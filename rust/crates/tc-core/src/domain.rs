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

/// The fields of the stored JSON this crate does not model.
///
/// A tour written by the C# app carries more than the arithmetic needs - when it was
/// created, its sync metadata, the cached per-person spending breakdowns. Dropping them on
/// the way through would quietly damage everybody's data the first time this code writes a
/// tour back, so they are carried along untouched instead: read in, written out, never
/// looked at.
///
/// `#[serde(flatten)]` on the wire structs is what collects them: whatever a struct did not
/// claim by name ends up here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Extras(pub serde_json::Map<String, serde_json::Value>);

#[derive(Debug, Clone, PartialEq)]
pub struct Currency {
    pub id: CurrencyId,
    pub name: String,
    /// Rate against the tour's base, times 100 - as it is stored today.
    pub rate: i32,
    pub extras: Extras,
}

impl Default for Currency {
    fn default() -> Self {
        Currency {
            id: CurrencyId::new("coin"),
            name: "coin".to_owned(),
            rate: 100,
            extras: Extras::default(),
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
    /// People who travel together and square up between themselves anyway.
    ///
    /// The settlement avoids proposing a payment inside a group when it can pay somebody
    /// outside instead. C# gives every person a fresh random id here when the data has
    /// none, so "no group" has to mean "in nobody else's group" - which is what `None`
    /// means below, since two `None`s are never the same group.
    pub group: Option<String>,
    pub extras: Extras,
}

impl Person {
    /// Whether two people count as travelling together.
    pub fn same_group_as(&self, other: &Person) -> bool {
        match (&self.group, &other.group) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// The settlement pays these creditors back first. Inherited from C#'s `_PGS_ = "r"`;
    /// what the letter means is not recorded anywhere, and the data in hand does not use it.
    pub fn is_preferred_creditor(&self) -> bool {
        self.group.as_deref().is_some_and(|g| g.starts_with('r'))
    }
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
    /// How the money is actually divided. This is what the arithmetic reads.
    pub split: Split,
    /// What the form had selected before "everyone" was switched on.
    ///
    /// Dead weight as far as the sums go, and not dead at all to the person editing: switch
    /// "everyone" back off and the old selection is still there. The stored data keeps it -
    /// spendings in the fixtures carry `ToAll: true` together with a list of names and a
    /// weighting flag - so this crate keeps it too, in its own field rather than smuggled
    /// into `split`, which stays exactly three cases with no impossible fourth.
    ///
    /// Found by the round-trip test, not by reading the C# source.
    pub remembered_split: Option<Split>,
    pub kind: Kind,
    pub extras: Extras,
}

impl Spending {
    /// When it happened, as the stored ISO stamp - "2021-08-18T12:38:04.123Z".
    ///
    /// Not modelled as a date, and deliberately so: nothing here does arithmetic on time.
    /// The interface groups by day and sorts by it, and an ISO stamp sorts correctly as
    /// text, so a date library would buy nothing and cost a dependency in the browser.
    ///
    /// `SpendingDate` is what the app writes when somebody picks a date; older spendings
    /// only have `DateCreated`, which is what the C# getter falls back to.
    pub fn when(&self) -> Option<&str> {
        for key in ["SpendingDate", "DateCreated"] {
            let found = self
                .extras
                .0
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .and_then(|(_, v)| v.as_str())
                .filter(|s| !s.is_empty());
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// The day it happened, as "2021-08-18": the first ten characters of the stamp.
    pub fn day(&self) -> Option<&str> {
        self.when().filter(|s| s.len() >= 10).map(|s| &s[..10])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tour {
    pub id: TourId,
    pub name: String,
    pub persons: Vec<Person>,
    pub spendings: Vec<Spending>,
    pub currencies: Vec<Currency>,
    pub current_currency: CurrencyId,
    pub extras: Extras,
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

    /// What a spending is worth in the currency the tour is being shown in.
    ///
    /// Three rules, all of them inherited rather than chosen, because the stored data was
    /// written under them (`Spending.AmountInCurrentCurrency` in the C# source):
    ///
    /// 1. A tour with one currency or none converts nothing.
    /// 2. The rate used is the tour's rate for that currency **id**, not the rate stored on
    ///    the spending - those go stale, and the tour's list is the authority.
    /// 3. A spending in a currency the tour does not list is treated as being in the current
    ///    one, and passes through untouched.
    pub fn amount_in_current(&self, spending: &Spending) -> Cents {
        self.convert(spending.amount, &spending.currency)
    }

    /// The same conversion for an amount that is not attached to a spending - a suggested
    /// payment, say, which carries its own currency.
    pub fn convert(&self, amount: Cents, from: &Currency) -> Cents {
        if self.currencies.len() <= 1 {
            return amount;
        }
        let current = self.currency();
        let from = self
            .currencies
            .iter()
            .find(|c| c.id == from.id)
            .unwrap_or(current);
        if from.id == current.id {
            return amount;
        }
        crate::money::convert(amount, from.rate, current.rate)
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
        let mut value: serde_json::Value = serde_json::from_str(s)?;
        wire::normalise_case(&mut value);
        let w: wire::Tour = serde_json::from_value(value)?;
        Ok(w.into())
    }

    /// Writes the tour back in the shape the C# app reads.
    ///
    /// The point of the exercise is that this is not "our format": a tour written here has
    /// to be readable by the running application, by the bot and by whatever is already in
    /// somebody's browser. So it goes back out through the same `wire` types it came in
    /// through, unmodelled fields and all.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let w: wire::Tour = self.into();
        serde_json::to_string_pretty(&w)
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
    use serde_json::Value;

    /// Fixes the spelling of the field names before they reach serde.
    ///
    /// Three of the eight tours in the seed file are written in camelCase (`id`, `persons`,
    /// `accessCodeMD5`) and the rest in PascalCase. Newtonsoft reads both without noticing,
    /// because it matches property names case-insensitively; serde matches them exactly and
    /// would quietly return an empty tour for the camelCase ones.
    ///
    /// So any key that matches a field this module knows about, whatever its case, is
    /// renamed to the spelling the structs below declare. Keys nobody claims are left
    /// exactly as they were, because they travel on into `Extras` and back out again.
    ///
    /// This is the same hazard the plan flagged for swapping Newtonsoft for
    /// System.Text.Json in the C# app, found here first because this port hit it first.
    pub fn normalise_case(value: &mut Value) {
        const TOUR: &[&str] = &[
            "Id",
            "GUID",
            "Name",
            "Persons",
            "Spendings",
            "Currencies",
            "TourCurrencyId",
        ];
        const PERSON: &[&str] = &["GUID", "Name", "Weight", "ParentId", "GroupId"];
        const SPENDING: &[&str] = &[
            "GUID",
            "Description",
            "Type",
            "AmountInCents",
            "Currency",
            "FromGuid",
            "ToGuid",
            "ToAll",
            "IsPartialWeighted",
            "Planned",
            "IsDryRun",
            "IncludeDryRunInCalc",
        ];
        const CURRENCY: &[&str] = &["Id", "Name", "CurrencyRate"];

        rename(value, TOUR);
        for person in array_at(value, "Persons") {
            rename(person, PERSON);
        }
        for currency in array_at(value, "Currencies") {
            rename(currency, CURRENCY);
        }
        for spending in array_at(value, "Spendings") {
            rename(spending, SPENDING);
            if let Some(c) = spending.get_mut("Currency") {
                rename(c, CURRENCY);
            }
        }
    }

    fn rename(value: &mut Value, canonical: &[&str]) {
        let Some(obj) = value.as_object_mut() else {
            return;
        };
        for want in canonical {
            if obj.contains_key(*want) {
                continue;
            }
            let found = obj.keys().find(|k| k.eq_ignore_ascii_case(want)).cloned();
            if let Some(k) = found {
                if let Some(v) = obj.remove(&k) {
                    obj.insert((*want).to_owned(), v);
                }
            }
        }
    }

    fn array_at<'a>(value: &'a mut Value, key: &str) -> impl Iterator<Item = &'a mut Value> {
        value
            .get_mut(key)
            .and_then(|v| v.as_array_mut())
            .map(|a| a.iter_mut())
            .unwrap_or_default()
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Tour {
        /// Tours stored before `Id` existed only carry `GUID`; the C# property maps one
        /// onto the other, so both spellings appear in real data and both must read.
        #[serde(default, rename = "Id")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, rename = "GUID")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub guid: Option<String>,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub persons: Vec<Person>,
        #[serde(default)]
        pub spendings: Vec<Spending>,
        /// Old tours have no currency list at all, hence `Option` and a default below.
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub currencies: Option<Vec<Currency>>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tour_currency_id: Option<String>,
        /// Everything this struct did not name. See [`Extras`].
        #[serde(flatten)]
        pub rest: serde_json::Map<String, serde_json::Value>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_id: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub group_id: Option<String>,
        /// Everything this struct did not name. See [`Extras`].
        #[serde(flatten)]
        pub rest: serde_json::Map<String, serde_json::Value>,
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
        #[serde(skip_serializing_if = "Option::is_none")]
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
        /// Everything this struct did not name. See [`Extras`].
        #[serde(flatten)]
        pub rest: serde_json::Map<String, serde_json::Value>,
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
        /// Everything this struct did not name. See [`Extras`].
        #[serde(flatten)]
        pub rest: serde_json::Map<String, serde_json::Value>,
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
                extras: Extras(c.rest),
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
                group: p.group_id,
                extras: Extras(p.rest),
            }
        }
    }

    impl From<Spending> for super::Spending {
        fn from(s: Spending) -> super::Spending {
            // Here the four flags collapse into three cases, once, in one place.
            let to: Vec<PersonId> = s.to_guid.into_iter().map(PersonId::new).collect();
            let chosen = if s.is_partial_weighted {
                Split::ByWeight(to)
            } else {
                Split::Equally(to)
            };
            // "Everyone" wins over the selection; the selection is kept aside.
            let (split, remembered_split) = if s.to_all {
                (Split::Everyone, Some(chosen))
            } else {
                (chosen, None)
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
                remembered_split,
                kind,
                extras: Extras(s.rest),
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
                id: TourId::new(t.id.or(t.guid).unwrap_or_default()),
                name: t.name,
                persons: t.persons.into_iter().map(Into::into).collect(),
                spendings: t.spendings.into_iter().map(Into::into).collect(),
                currencies,
                current_currency: current,
                extras: Extras(t.rest),
            }
        }
    }
}

// --- and back out again ----------------------------------------------------------------
//
// One rule worth stating before the code: **a field the source did not have is not written
// back as null.** The C# model fills an absent field with its default, and some of those
// defaults are generated rather than constant - `Person.GroupId` is a fresh Guid. Writing
// an explicit null instead overwrites that default with nothing, and the settlement then
// dies on `GroupId.StartsWith(...)`. Hence `skip_serializing_if` on every Option above:
// absent stays absent.
//
// Written by hand rather than derived, because the shapes differ: one `Split` becomes three
// separate booleans plus a list, and `Kind` becomes another three. Deriving cannot know
// that, and a round trip through the wrong shape would be read by the C# app as a different
// spending.

impl From<&Currency> for wire::Currency {
    fn from(c: &Currency) -> wire::Currency {
        wire::Currency {
            id: Some(c.id.as_str().to_owned()),
            name: c.name.clone(),
            currency_rate: c.rate,
            rest: c.extras.0.clone(),
        }
    }
}

impl From<&Person> for wire::Person {
    fn from(p: &Person) -> wire::Person {
        wire::Person {
            guid: p.id.as_str().to_owned(),
            name: p.name.clone(),
            weight: p.weight,
            // "Pays for themselves" is written as null, which is what the C# model holds
            // by default. The stored data also contains "" for the same thing - both come
            // back as `None`, because C# tests this with IsNullOrWhiteSpace - and writing
            // one where the other stood changes nothing that anything reads.
            parent_id: p.parent.as_ref().map(|id| id.as_str().to_owned()),
            group_id: p.group.clone(),
            rest: p.extras.0.clone(),
        }
    }
}

impl From<&Spending> for wire::Spending {
    fn from(s: &Spending) -> wire::Spending {
        // On the wire the selection is written whether or not it is in force, so that
        // switching "everyone" off in the app finds it again.
        let selection = match &s.split {
            Split::Everyone => s.remembered_split.as_ref(),
            other => Some(other),
        };
        let (to_guid, partial) = match selection {
            Some(Split::ByWeight(to)) => {
                (to.iter().map(|id| id.as_str().to_owned()).collect(), true)
            }
            Some(Split::Equally(to)) => {
                (to.iter().map(|id| id.as_str().to_owned()).collect(), false)
            }
            Some(Split::Everyone) | None => (Vec::new(), false),
        };
        let to_all = matches!(s.split, Split::Everyone);
        let (planned, dry_run, counted) = match s.kind {
            Kind::Real => (false, false, false),
            Kind::Draft { counted } => (false, true, counted),
            Kind::Planned => (true, false, false),
        };
        wire::Spending {
            guid: s.id.as_str().to_owned(),
            description: s.description.clone(),
            kind_name: s.category.clone(),
            amount_in_cents: s.amount.0,
            currency: Some((&s.currency).into()),
            from_guid: s.from.as_str().to_owned(),
            to_guid,
            to_all,
            is_partial_weighted: partial,
            planned,
            is_dry_run: dry_run,
            include_dry_run_in_calc: counted,
            rest: s.extras.0.clone(),
        }
    }
}

impl From<&Tour> for wire::Tour {
    fn from(t: &Tour) -> wire::Tour {
        wire::Tour {
            id: Some(t.id.as_str().to_owned()),
            guid: Some(t.id.as_str().to_owned()),
            name: t.name.clone(),
            persons: t.persons.iter().map(Into::into).collect(),
            spendings: t.spendings.iter().map(Into::into).collect(),
            currencies: Some(t.currencies.iter().map(Into::into).collect()),
            tour_currency_id: Some(t.current_currency.as_str().to_owned()),
            rest: t.extras.0.clone(),
        }
    }
}
