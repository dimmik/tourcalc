//! Every fixture, checked against what the C# implementation makes of it.
//!
//! The expected files are not written by hand and not transcribed from a screen: they are
//! produced by `rust/tools/GoldenDump`, which loads the same tours and runs the real
//! `TourCalculator` over them. Regenerate with
//!
//! ```text
//! dotnet run --project rust/tools/GoldenDump -- TCBlazor/Server/inmemory-tours.json rust/fixtures
//! ```
//!
//! One test per tour would be tidier in the output, but the fixtures are data rather than
//! code, so this walks the directory instead - a new tour becomes covered by dropping two
//! files in, with nothing to remember to add here.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use tc_core::{calculate, suggest_settlement, Options, Tour};

#[derive(Debug, Deserialize)]
struct Expected {
    #[serde(rename = "tourId")]
    tour_id: String,
    name: String,
    currency: ExpectedCurrency,
    #[serde(rename = "isMultiCurrency")]
    is_multi_currency: bool,
    persons: Vec<ExpectedPerson>,
    transfers: Vec<ExpectedTransfer>,
}

#[derive(Debug, Deserialize)]
struct ExpectedCurrency {
    id: String,
    name: String,
    rate: i32,
}

#[derive(Debug, Deserialize)]
struct ExpectedPerson {
    guid: String,
    name: String,
    weight: i32,
    spent: i64,
    received: i64,
    debt: i64,
}

#[derive(Debug, Deserialize)]
struct ExpectedTransfer {
    #[serde(rename = "fromName")]
    from_name: String,
    #[serde(rename = "toName")]
    to_name: String,
    amount: i64,
    description: String,
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Every `<id>.tour.json` that has an `<id>.expected.json` beside it.
fn cases() -> Vec<(String, Tour, Expected)> {
    let dir = fixtures_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("fixtures directory")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".expected.json").map(str::to_owned)
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures found in {}", dir.display());

    names
        .into_iter()
        .map(|name| {
            let tour_json = std::fs::read_to_string(dir.join(format!("{name}.tour.json")))
                .unwrap_or_else(|e| panic!("reading {name}.tour.json: {e}"));
            let expected_json = std::fs::read_to_string(dir.join(format!("{name}.expected.json")))
                .unwrap_or_else(|e| panic!("reading {name}.expected.json: {e}"));
            let tour = Tour::from_json(&tour_json)
                .unwrap_or_else(|e| panic!("parsing {name}.tour.json: {e}"));
            let expected: Expected = serde_json::from_str(&expected_json)
                .unwrap_or_else(|e| panic!("parsing {name}.expected.json: {e}"));
            (name, tour, expected)
        })
        .collect()
}

/// The tour reads back as the same tour: id, name, people, and the currency in force.
#[test]
fn reads_every_fixture() {
    for (name, tour, expected) in cases() {
        assert_eq!(tour.id.as_str(), expected.tour_id, "[{name}] tour id");
        assert_eq!(tour.name, expected.name, "[{name}] tour name");
        assert_eq!(
            tour.persons.len(),
            expected.persons.len(),
            "[{name}] number of people"
        );

        let c = tour.currency();
        assert_eq!(c.id.as_str(), expected.currency.id, "[{name}] currency id");
        assert_eq!(c.name, expected.currency.name, "[{name}] currency name");
        assert_eq!(c.rate, expected.currency.rate, "[{name}] currency rate");
        assert_eq!(
            tour.currencies.len() > 1,
            expected.is_multi_currency,
            "[{name}] multi-currency"
        );
    }
}

/// Spent, received and debt for every person of every tour, to the cent.
#[test]
fn person_totals_match_csharp() {
    for (name, tour, expected) in cases() {
        let got = calculate(&tour, Options::default());

        for want in &expected.persons {
            let id = tc_core::PersonId::new(want.guid.clone());
            let person = tour
                .person(&id)
                .unwrap_or_else(|| panic!("[{name}] no person {}", want.name));
            assert_eq!(
                person.weight, want.weight,
                "[{name}] weight of {}",
                want.name
            );

            let b = got
                .get(&id)
                .unwrap_or_else(|| panic!("[{name}] no balance for {}", want.name));
            assert_eq!(b.spent.0, want.spent, "[{name}] {} spent", want.name);
            assert_eq!(
                b.received.0, want.received,
                "[{name}] {} received",
                want.name
            );
            assert_eq!(b.debt().0, want.debt, "[{name}] {} debt", want.name);
        }
    }
}

/// The suggested settlement, payment for payment and in the same order.
///
/// This is the assertion that caught the divergence phase 0 shipped with: an earlier
/// version settled everyone correctly while choosing different pairs, and only comparing
/// the actual list showed it.
#[test]
fn settlement_matches_csharp() {
    for (name, tour, expected) in cases() {
        let transfers = suggest_settlement(&tour)
            .unwrap_or_else(|e| panic!("[{name}] settlement did not converge: {e}"));

        let name_of = |id: &tc_core::PersonId| {
            tour.person(id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "n/a".to_owned())
        };

        let got: Vec<(String, String, i64, String)> = transfers
            .iter()
            .map(|t| {
                (
                    name_of(&t.from),
                    name_of(&t.to),
                    t.amount.0,
                    t.description.clone(),
                )
            })
            .collect();

        let want: Vec<(String, String, i64, String)> = expected
            .transfers
            .iter()
            .map(|t| {
                (
                    t.from_name.clone(),
                    t.to_name.clone(),
                    t.amount,
                    t.description.clone(),
                )
            })
            .collect();

        assert_eq!(got, want, "[{name}] suggested payments");
    }
}

/// Whatever the rounding does, the books balance - on every tour.
#[test]
fn credits_equal_debits() {
    for (name, tour, _) in cases() {
        let b = calculate(&tour, Options::default());
        let sum: i64 = b.per_person.iter().map(|p| p.debt().0).sum();
        assert_eq!(sum, 0, "[{name}] everything owed is owed to somebody");
    }
}

/// Making the suggested payments leaves nobody owing anything.
///
/// Checked by recomputing with the payments in place, which is the same arithmetic the
/// settlement loop uses to decide it is finished. Subtracting the payments from the
/// balances by hand instead looks equivalent and is not: `settle_rounding` runs at the end
/// of every calculation, so the odd cent can land on a different person in the two views -
/// on one of these fixtures the C# answer is off by 3 that way too.
#[test]
fn settlement_squares_everyone_up() {
    for (name, tour, _) in cases() {
        let transfers = suggest_settlement(&tour).expect("converges");
        let after = tc_core::balances_after(&tour, &transfers);

        for b in &after.per_person {
            assert_eq!(
                b.debt().0,
                0,
                "[{name}] {} still has {} outstanding",
                b.person,
                b.debt().0
            );
        }
    }
}

/// The suggested payments move exactly as much money as the C# implementation moves.
///
/// Weaker than `settlement_matches_csharp` and kept because it fails differently: if the
/// pairs ever drift again, the totals staying equal says the arithmetic is still right and
/// only the choice of who pays whom has changed.
#[test]
fn settlement_totals_match_csharp() {
    for (name, tour, expected) in cases() {
        let transfers = suggest_settlement(&tour).expect("converges");
        let got: i64 = transfers.iter().map(|t| t.amount.0).sum();
        let want: i64 = expected.transfers.iter().map(|t| t.amount).sum();
        assert_eq!(got, want, "[{name}] total moved");
    }
}
