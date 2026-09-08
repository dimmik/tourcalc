//! The data a tour is made of.
//!
//! Two things are going on in this file and they pull in opposite directions.
//!
//! The first is modelling: say what a tour *is*, so that impossible states cannot be
//! written down. That is where `Split` comes from.
//!
//! The second is compatibility: there are tours in MongoDB and in people's browsers right
//! now, written by the C# app, and this has to read them byte for byte. That is where the
//! `wire` module comes from - a literal transcription of the old shape, converted on the
//! way in. Modelling wins in the code, compatibility wins on the wire, and the two meet in
//! exactly one place instead of leaking into everything.

use crate::ids::{CurrencyId, PersonId, SpendingId, TourId};
use crate::money::Cents;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct Currency {
    pub id: CurrencyId,
    pub name: String,
    /// Rate against the tour's base, times 100 - as it is stored today.
    pub rate: i32,
}

impl Default for Currency {
    fn default() -> Self {
        Currency {
            id: CurrencyId::new("coin"),
            name: "coin".to_owned(),
            rate: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    pub weight: i32,
    /// Who pays for this one. `None` - pays for themselves.
    ///
    /// In C# this is `string ParentId = null`, and every reader has to remember that null
    /// means something. `Option` says it in the type, and there is no null to forget about.
    pub parent: Option<PersonId>,
}

/// How one spending is divided. Three shapes, and no fourth.
///
/// The C# model spreads this over four independent fields - `ToAll`, `ToGuid`, `Weighted`,
/// `IsPartialWeighted` - which is eight combinations, of which three mean anything. The
/// other five exist, can be stored, and have to be mentally excluded by everyone reading
/// the code.
///
/// As an enum the impossible ones cannot be constructed, and `match` will not compile until
/// every real case is handled. That is what enums are for in Rust: not a list of constants,
/// but "one of these, each carrying its own data".
#[derive(Debug, Clone, PartialEq)]
pub enum Split {
    /// Everyone on the tour, in proportion to their weight.
    Everyone,
    /// These people, in equal shares.
    Equally(Vec<PersonId>),
    /// These people, in proportion to their weights.
    ByWeight(Vec<PersonId>),
}

/// Whether a spending counts, and why.
///
/// Replaces the `Planned` / `IsDryRun` / `IncludeDryRunInCalc` triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An expense somebody actually made.
    Real,
    /// A draft: entered, but only counted if the author says so.
    Draft { counted: bool },
    /// A transfer the app itself proposed to settle up.
    Planned,
}

impl Kind {
    /// Draft spendings are excluded unless marked; planned ones only when asked for.
    pub fn counts(self, with_planned: bool) -> bool {
        match self {
            Kind::Real => true,
            Kind::Draft { counted } => counted,
            Kind::Planned => with_planned,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spending {
    pub id: SpendingId,
    pub description: String,
    pub category: String,
    pub amount: Cents,
    pub currency: Currency,
    pub from: PersonId,
    pub split: Split,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tour {
    pub id: TourId,
    pub name: String,
    pub persons: Vec<Person>,
    pub spendings: Vec<Spending>,
    pub currencies: Vec<Currency>,
    pub current_currency: CurrencyId,
}

impl Tour {
    pub fn person(&self, id: &PersonId) -> Option<&Person> {
        self.persons.iter().find(|p| &p.id == id)
    }

    /// The currency amounts are shown in.
    ///
    /// Returns a borrow, not a copy. The C# property hands out a fresh `Currency` on every
    /// read - it has to, because callers could otherwise mutate the tour's own - and it is
    /// read once per amount conversion. Here `&self` already says the caller cannot mutate
    /// anything, so there is nothing to protect and nothing to copy.
    pub fn currency(&self) -> &Currency {
        self.currencies
            .iter()
            .find(|c| c.id == self.current_currency)
            .or_else(|| self.currencies.first())
            .unwrap_or(&DEFAULT_CURRENCY)
    }

    pub fn total_weight(&self) -> i64 {
        let sum: i64 = self.persons.iter().map(|p| p.weight as i64).sum();
        if sum == 0 {
            1
        } else {
            sum
        }
    }

    pub fn from_json(s: &str) -> Result<Tour, serde_json::Error> {
        let w: wire::Tour = serde_json::from_str(s)?;
        Ok(w.into())
    }
}

/// A tour with no currencies at all should not exist - `From<wire::Tour>` always puts one
/// in - but `currency()` returns a borrow, and a borrow has to point at something that
/// outlives the call. `LazyLock` gives us one instance, built on first use, living for the
/// whole program: exactly the lifetime a fallback needs.
static DEFAULT_CURRENCY: std::sync::LazyLock<Currency> =
    std::sync::LazyLock::new(Currency::default);

// ---------------------------------------------------------------------------------------
// The wire format: what the C# app writes, transcribed literally, and converted on the way
// in. Nothing outside this module needs to know that `ToAll` and `IsPartialWeighted` ever
// existed.
// ---------------------------------------------------------------------------------------
pub mod wire {
    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Tour {
        #[serde(default, rename = "Id")]
        pub id: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub persons: Vec<Person>,
        #[serde(default)]
        pub spendings: Vec<Spending>,
        /// Old tours have no currency list at all, hence `Option` and a default below.
        #[serde(default)]
        pub currencies: Option<Vec<Currency>>,
        #[serde(default)]
        pub tour_currency_id: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Person {
        #[serde(rename = "GUID")]
        pub guid: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub weight: i32,
        #[serde(default)]
        pub parent_id: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Spending {
        #[serde(rename = "GUID")]
        pub guid: String,
        #[serde(default)]
        pub description: String,
        #[serde(default, rename = "Type")]
        pub kind_name: String,
        #[serde(default)]
        pub amount_in_cents: i64,
        #[serde(default)]
        pub currency: Option<Currency>,
        #[serde(default)]
        pub from_guid: String,
        #[serde(default)]
        pub to_guid: Vec<String>,
        #[serde(default)]
        pub to_all: bool,
        #[serde(default)]
        pub is_partial_weighted: bool,
        #[serde(default)]
        pub planned: bool,
        #[serde(default)]
        pub is_dry_run: bool,
        #[serde(default)]
        pub include_dry_run_in_calc: bool,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Currency {
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default = "coin")]
        pub name: String,
        #[serde(default = "hundred")]
        pub currency_rate: i32,
    }

    fn coin() -> String {
        "coin".to_owned()
    }
    fn hundred() -> i32 {
        100
    }

    impl From<Currency> for super::Currency {
        fn from(c: Currency) -> super::Currency {
            // The old model lets Id be absent and falls back to Name - "keep current
            // contract", as the C# comment puts it. Kept, because the data relies on it.
            let id = c.id.unwrap_or_else(|| c.name.clone());
            super::Currency {
                id: CurrencyId::new(id),
                name: c.name,
                rate: c.currency_rate,
            }
        }
    }

    impl From<Person> for super::Person {
        fn from(p: Person) -> super::Person {
            super::Person {
                id: PersonId::new(p.guid),
                name: p.name,
                weight: p.weight,
                // An empty string is not a parent, it is the absence of one.
                parent: p
                    .parent_id
                    .filter(|s| !s.trim().is_empty())
                    .map(PersonId::new),
            }
        }
    }

    impl From<Spending> for super::Spending {
        fn from(s: Spending) -> super::Spending {
            // Here the four flags collapse into three cases, once, in one place.
            let split = if s.to_all {
                Split::Everyone
            } else {
                let to: Vec<PersonId> = s.to_guid.into_iter().map(PersonId::new).collect();
                if s.is_partial_weighted {
                    Split::ByWeight(to)
                } else {
                    Split::Equally(to)
                }
            };
            let kind = if s.planned {
                Kind::Planned
            } else if s.is_dry_run {
                Kind::Draft {
                    counted: s.include_dry_run_in_calc,
                }
            } else {
                Kind::Real
            };
            super::Spending {
                id: SpendingId::new(s.guid),
                description: s.description,
                category: s.kind_name,
                amount: Cents(s.amount_in_cents),
                currency: s.currency.map(Into::into).unwrap_or_default(),
                from: PersonId::new(s.from_guid),
                split,
                kind,
            }
        }
    }

    impl From<Tour> for super::Tour {
        fn from(t: Tour) -> super::Tour {
            let currencies: Vec<super::Currency> = t
                .currencies
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect();
            let currencies = if currencies.is_empty() {
                vec![super::Currency::default()]
            } else {
                currencies
            };
            let current = t
                .tour_currency_id
                .map(CurrencyId::new)
                .filter(|id| currencies.iter().any(|c| &c.id == id))
                .unwrap_or_else(|| currencies[0].id.clone());
            super::Tour {
                id: TourId::new(t.id),
                name: t.name,
                persons: t.persons.into_iter().map(Into::into).collect(),
                spendings: t.spendings.into_iter().map(Into::into).collect(),
                currencies,
                current_currency: current,
            }
        }
    }
}
