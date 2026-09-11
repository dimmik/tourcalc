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

/// The balances list, as the interface shows it.
///
/// Different from the raw per-person debt, and worth its own check because the difference
/// is easy to get wrong in exactly the way that looks plausible: children hidden, family
/// payments left out of the netting. The numbers are the ones the C# client renders for
/// this tour.
#[test]
fn balances_list_matches_the_app() {
    let (_, tour, _) = cases()
        .into_iter()
        .find(|(name, _, _)| name == "zscph2y")
        .expect("the Ural tour");

    let transfers = suggest_settlement(&tour).expect("converges");
    let rows = tc_core::settlement_summary(&tour, &transfers, tour.min_meaningful(tc_core::MINIMUM_MEANINGFUL));

    let named: Vec<(String, i64)> = rows
        .iter()
        .map(|(id, amount)| {
            let name = tour.person(id).map(|p| p.name.clone()).unwrap_or_default();
            (name, amount.0)
        })
        .collect();

    assert_eq!(
        named,
        vec![
            ("Саша О.".to_owned(), 38_457),
            ("Женя К.".to_owned(), 28_389),
            ("Дима Т.".to_owned(), 3_403),
            ("Длинное и весьма длинное такое вот имя".to_owned(), -916),
            ("Андрей".to_owned(), -5_055),
            ("Хомяк".to_owned(), -10_197),
            ("Паша".to_owned(), -54_081),
        ]
    );
}

/// Spendings carry a date, and it is readable without a date library.
#[test]
fn spendings_have_a_day() {
    let (_, tour, _) = cases()
        .into_iter()
        .find(|(name, _, _)| name == "zscph2y")
        .expect("the Ural tour");

    let days: Vec<&str> = tour.spendings.iter().filter_map(|s| s.day()).collect();
    assert_eq!(days.len(), tour.spendings.len(), "every spending has a day");
    assert!(days
        .iter()
        .all(|d| d.len() == 10 && d.as_bytes()[4] == b'-'));

    // ISO stamps sort as text, which is the whole reason the interface can group and order
    // by them without parsing anything.
    let mut sorted = days.clone();
    sorted.sort();
    assert_eq!(sorted.first(), Some(&"2021-06-10"));
}

/// The breakdown behind a figure adds up to that figure - for every person of every tour.
///
/// The one property a breakdown has to have. A list of lines that does not sum to the total
/// printed above it is worse than showing nothing, because the reader will add it up.
///
/// Both sides are checked against `calculate`, which is itself checked against the C#
/// above, so this pins the itemised view to the same numbers without a second golden file.
#[test]
fn the_breakdown_adds_up_to_the_totals() {
    for (file, tour, _) in cases() {
        let balances = calculate(&tour, Options::default());
        for b in &balances.per_person {
            let who = tour.person(&b.person).expect("person");
            let it = tc_core::breakdown(&tour, &b.person, Options::default());

            assert_eq!(
                it.paid_total(),
                b.spent,
                "{file}: what {} paid, itemised, is what they paid",
                who.name
            );
            assert_eq!(
                it.charged_total(),
                b.received,
                "{file}: what {} was charged, itemised, is what they were charged",
                who.name
            );
        }
    }
}

/// Every line of a breakdown points at a spending that is really in the tour.
///
/// Except the rounding line, which points at none - that is what `Option` is saying, and
/// the test says the same thing in the other direction: nothing else is allowed to be
/// unattributable.
#[test]
fn breakdown_lines_point_at_real_spendings() {
    for (file, tour, _) in cases() {
        for p in &tour.persons {
            let it = tc_core::breakdown(&tour, &p.id, Options::default());

            for line in it.paid.iter() {
                let id = line.spending.as_ref().expect("a payment has a spending");
                assert!(
                    tour.spendings.iter().any(|s| &s.id == id),
                    "{file}: {} paid for something that is in the tour",
                    p.name
                );
            }

            let unattributed = it.charged.iter().filter(|l| l.spending.is_none()).count();
            assert!(
                unattributed <= 1,
                "{file}: at most one line - the rounding - belongs to no spending"
            );
        }
    }
}

/// The balances list of a tour whose settlement leaves dust, against what the app shows.
///
/// The companion to `balances_list_matches_the_app`, and the one that has an opinion: on
/// this tour three people are owed real money *and* have a two-cent payment to make, and the
/// app reports all three as settled. Netting what they pay against what they receive would
/// list them as creditors of 5 055, 10 197 and 15 626 - plausible, tidy, and not what the
/// reader sees in the app they already use.
///
/// The numbers are read off the running C# client for this tour.
#[test]
fn dust_does_not_turn_a_creditor_into_a_line_item() {
    let (_, tour, _) = cases()
        .into_iter()
        .find(|(name, _, _)| name == "hs3huvy")
        .expect("the unfinished Ural tour");

    let transfers = suggest_settlement(&tour).expect("converges");
    let named: Vec<(String, i64)> = tc_core::settlement_summary(&tour, &transfers, tour.min_meaningful(tc_core::MINIMUM_MEANINGFUL))
        .iter()
        .map(|(id, amount)| {
            (
                tour.person(id).map(|p| p.name.clone()).unwrap_or_default(),
                amount.0,
            )
        })
        .collect();

    assert_eq!(
        named,
        vec![
            ("Женя К.".to_owned(), 28_389),
            ("Дима Т.".to_owned(), 3_403),
            ("Дима А.".to_owned(), -916),
        ],
        "three people owed money are settled: their whole obligation is two cents"
    );
}

/// What counts as noise does not change when the reader switches currency.
///
/// A threshold is a number of coins, and coins are not the same size in every currency. The
/// app scales it by the cheapest currency's rate over the current one, so "ignore under 49"
/// means the same amount of money whichever currency the tour is being read in - rather than
/// meaning half a euro on one screen and a third of a dinar on the next.
#[test]
fn the_threshold_follows_the_currency() {
    let (_, tour, _) = cases()
        .into_iter()
        .find(|(name, _, _)| name == "mcurrzscph2y")
        .expect("the multi-currency tour");

    assert!(tour.currencies.len() > 1, "this fixture has several");

    let cheapest = tour.currencies.iter().map(|c| c.rate).min().unwrap();
    let dearest = tour.currencies.iter().map(|c| c.rate).max().unwrap();
    assert_ne!(cheapest, dearest, "and they are worth different amounts");

    // Read in the cheapest currency, the threshold is the setting itself.
    let in_cheap = tour
        .currencies
        .iter()
        .find(|c| c.rate == cheapest)
        .map(|c| {
            let mut t = tour.clone();
            t.current_currency = c.id.clone();
            t.min_meaningful(49)
        })
        .unwrap();
    assert_eq!(in_cheap.0, 49);

    // Read in a dearer one, the same money is fewer of its coins.
    let in_dear = tour
        .currencies
        .iter()
        .find(|c| c.rate == dearest)
        .map(|c| {
            let mut t = tour.clone();
            t.current_currency = c.id.clone();
            t.min_meaningful(49)
        })
        .unwrap();
    assert!(in_dear.0 < 49, "{} is not fewer than 49", in_dear.0);
    assert_eq!(in_dear.0, 49 * cheapest as i64 / dearest as i64);

    // A tour with one currency has nothing to scale by.
    let (_, single, _) = cases()
        .into_iter()
        .find(|(name, _, _)| name == "zscph2y")
        .expect("the single-currency tour");
    assert_eq!(single.min_meaningful(49).0, 49);
    assert_eq!(single.min_meaningful(0).0, 0);
}

/// A currency the app has stored in MongoDB is identified by `_id`, not by `Id`.
///
/// The MongoDB driver maps the C#'s `Id` property to the `_id` element and is registered to
/// ignore every element it does not know, so a document carrying both is read by the app as
/// its `_id`. Reading the other one is how a real tour's euro view came out sixty times too
/// big: its expenses were stamped `Din`, the tour's list said `_id: "Din"` with `Id: "RSD"`
/// written beside it, nothing matched, and "a currency the tour does not list" means "no
/// conversion at all".
#[test]
fn a_currency_is_known_by_the_id_the_app_stores() {
    let json = r#"{
        "Id": "t1", "Name": "renamed currency",
        "TourCurrencyId": "Eur",
        "Currencies": [
            {"Id": "RSD", "Name": "RSD", "CurrencyRate": 1000, "_id": "Din"},
            {"Id": "Eur", "Name": "Eur", "CurrencyRate": 117000, "_id": "Eur"}
        ],
        "Persons": [{"GUID": "p1", "Name": "Ann", "Weight": 100}],
        "Spendings": [
            {"GUID": "s1", "Description": "dinars", "Type": "food", "AmountInCents": 11700,
             "Currency": {"Id": "Din", "Name": "Din", "CurrencyRate": 1000, "_id": "Din"},
             "FromGuid": "p1", "ToAll": true, "ToGuid": []}
        ]
    }"#;
    let tour = tc_core::Tour::from_json(json).expect("a tour");

    assert_eq!(
        tour.currencies[0].id.as_str(),
        "Din",
        "the identity is the one the driver wrote, not the one beside it"
    );
    assert_eq!(
        tour.amount_in_current(&tour.spendings[0]),
        tc_core::Cents(100),
        "11700 dinars at 1000 against the euro at 117000 - and not 11700, which is what \
         'the tour has no such currency' would have given"
    );

    // Written back, the two names for it agree, so the next reader of either finds the same
    // currency. Before this, saving from here left a document the app read one way and this
    // port read the other.
    let out: serde_json::Value = serde_json::from_str(&tour.to_json().unwrap()).unwrap();
    let stored = &out["Currencies"][0];
    assert_eq!(stored["Id"], "Din");
    assert_eq!(stored["_id"], "Din");
    assert_eq!(stored["Name"], "RSD", "the name is untouched: only the id was confused");
}

/// The same tour as the app's own JSON has it - `Id` and no `_id` - still reads by `Id`.
#[test]
fn a_currency_from_the_apps_json_is_known_by_its_id() {
    let json = r#"{
        "Id": "t2", "Name": "plain",
        "TourCurrencyId": "Eur",
        "Currencies": [
            {"Id": "Din", "Name": "RSD", "CurrencyRate": 1000},
            {"Id": "Eur", "Name": "Eur", "CurrencyRate": 117000}
        ],
        "Persons": [{"GUID": "p1", "Name": "Ann", "Weight": 100}],
        "Spendings": [
            {"GUID": "s1", "Description": "dinars", "Type": "food", "AmountInCents": 11700,
             "Currency": {"Id": "Din", "Name": "RSD", "CurrencyRate": 1000},
             "FromGuid": "p1", "ToAll": true, "ToGuid": []}
        ]
    }"#;
    let tour = tc_core::Tour::from_json(json).expect("a tour");
    assert_eq!(tour.currencies[0].id.as_str(), "Din");
    assert_eq!(tour.amount_in_current(&tour.spendings[0]), tc_core::Cents(100));
}

/// The `Currency` written beside `TourCurrencyId` says the same thing as it.
///
/// It is the C#'s computed view of which currency a tour is in, and on that side it is not
/// read-only: the property's setter changes `TourCurrencyId` and drops the suggested
/// payments. A tour saved from here used to carry whatever that field said when it arrived,
/// which after any currency change was the wrong one - two fields, two answers, and which a
/// reader believes depends on the order it reads them in.
#[test]
fn the_tours_own_currency_agrees_with_the_id_beside_it() {
    let json = r#"{
        "Id": "t3", "Name": "changed its mind",
        "TourCurrencyId": "Eur",
        "Currency": {"CurrencyRate": 1000, "Name": "RSD", "_id": "Din"},
        "Currencies": [
            {"Id": "RSD", "Name": "RSD", "CurrencyRate": 1000, "_id": "Din"},
            {"Id": "Eur", "Name": "Eur", "CurrencyRate": 117000, "_id": "Eur"}
        ],
        "Persons": [], "Spendings": []
    }"#;
    let tour = tc_core::Tour::from_json(json).expect("a tour");
    let out: serde_json::Value = serde_json::from_str(&tour.to_json().unwrap()).unwrap();

    assert_eq!(out["TourCurrencyId"], "Eur");
    assert_eq!(out["Currency"]["Id"], "Eur");
    assert_eq!(out["Currency"]["Name"], "Eur");
    assert_eq!(out["Currency"]["CurrencyRate"], 117000);
    assert_eq!(
        out["Currency"]["_id"], "Eur",
        "a document that came from the database keeps the name the driver reads"
    );
}
