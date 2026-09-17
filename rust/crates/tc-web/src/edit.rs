//! Changing a tour, and saving it.
//!
//! Every edit is the same shape: take the tour that is on screen, produce a changed one,
//! and PATCH the whole thing. That is how the app has always saved - the API has no
//! finer-grained calls - and it is why the functions here take `&Tour` and return `Tour`
//! rather than mutating in place: the caller keeps what it had until the server has agreed,
//! and can put it back if it has not.

use tc_core::{Cents, Kind, Person, PersonId, Spending, SpendingId, Split, Tour};

/// A new id, in the style the app uses: short, lowercase, URL-safe.
///
/// The C# builds one from the clock plus a little randomness. So does this, with the clock
/// the browser has; ids only have to be unique within a tour, and two edits a millisecond
/// apart on the same device are not a thing that happens.
pub fn new_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let ms = js_sys::Date::now() as u64;
    let noise = (js_sys::Math::random() * 1024.0) as u64;
    let mut n = ms.wrapping_mul(1024).wrapping_add(noise);
    let mut out = String::new();
    for _ in 0..7 {
        out.push(ALPHABET[(n % ALPHABET.len() as u64) as usize] as char);
        n /= ALPHABET.len() as u64;
    }
    out
}

/// What the spending dialog collects.
///
/// Serialisable because it is also what sits in the offline queue: an edit is remembered as
/// the intention behind it, so that it can be replayed onto a tour that has changed since.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SpendingDraft {
    /// Which spending this is about.
    ///
    /// `None` only while the dialog is open. The id is settled when the edit is *recorded*,
    /// not when it is applied - otherwise replaying a queued "add" a second time would
    /// invent a second id and leave two copies of one taxi fare. Assigning it up front makes
    /// the operation say exactly one thing, however many times it is carried out.
    pub id: Option<SpendingId>,
    pub description: String,
    pub category: String,
    pub amount: Cents,
    pub from: PersonId,
    pub everyone: bool,
    pub to: Vec<PersonId>,
    /// Whether the people in `to` share it by weight, or equally. By weight unless somebody
    /// says otherwise - the app's own default, and the one every expense entered through it
    /// carries. This form used to have no such choice and saved every partial split as
    /// equal, so a trip's expenses for "the grown-ups" ignored the weights they were given,
    /// and editing one entered in the app quietly changed how it was divided.
    ///
    /// Defaulted when read back: an edit queued before the field existed is by weight.
    #[serde(default = "by_weight")]
    pub by_weight: bool,
    /// The day it happened, `YYYY-MM-DD`. Empty means "leave whatever is stored", which is
    /// what an expense entered without touching the field should do - the app fills it in
    /// with today when the spending is new.
    #[serde(default)]
    pub date: String,
    /// A colour marks a row out in the list: a tourist tax for the whole group, an
    /// unexpected fine. Empty is the ordinary case.
    #[serde(default)]
    pub colour: String,
    /// Which of the tour's currencies the amount is in. Empty leaves what is stored, which
    /// is what an edit that never touched the field should do.
    #[serde(default)]
    pub currency_id: String,
}

fn by_weight() -> bool {
    true
}

/// The split a draft describes, for these people.
fn split_for(by_weight: bool, to: Vec<PersonId>) -> Split {
    if by_weight {
        Split::ByWeight(to)
    } else {
        Split::Equally(to)
    }
}

impl SpendingDraft {
    /// A blank draft, with the sensible defaults the app starts from.
    pub fn new(tour: &Tour) -> SpendingDraft {
        SpendingDraft {
            id: None,
            description: String::new(),
            category: last_category(tour),
            amount: Cents::ZERO,
            from: tour
                .persons
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| PersonId::new("")),
            everyone: true,
            to: Vec::new(),
            by_weight: true,
            date: today(),
            colour: String::new(),
            // The one the amounts are being read in, not the tour's own: if the screen is
            // in euro, what somebody is typing in is almost certainly euro too. The app
            // starts a new expense the same way.
            currency_id: tour.currency().id.as_str().to_owned(),
        }
    }

    pub fn of(spending: &Spending) -> SpendingDraft {
        let (everyone, to) = match &spending.split {
            Split::Everyone => (true, Vec::new()),
            Split::Equally(to) | Split::ByWeight(to) => (false, to.clone()),
        };
        // For an expense for everyone, the choice it would go back to if "everyone" were
        // switched off - which is what the stored format remembers.
        let by_weight = match &spending.split {
            Split::Everyone => !matches!(spending.remembered_split, Some(Split::Equally(_))),
            Split::Equally(_) => false,
            Split::ByWeight(_) => true,
        };
        SpendingDraft {
            id: Some(spending.id.clone()),
            description: spending.description.clone(),
            category: spending.category.clone(),
            amount: spending.amount,
            from: spending.from.clone(),
            everyone,
            to,
            by_weight,
            date: spending.day().unwrap_or_default().to_owned(),
            colour: tc_core::extras::str_of(&spending.extras, COLOUR),
            currency_id: spending.currency.id.as_str().to_owned(),
        }
    }

    /// Why this cannot be saved, if it cannot.
    /// Everything wrong with it, in the app's words and all at once.
    ///
    /// All at once because they are independent: told only the first, somebody fixes it,
    /// presses save, and is told the next one. The app lists them together and so does this.
    ///
    /// "What for" is on the list, which it was not before: an expense with no description is
    /// a line in the list that says only how much, and a week later nobody knows what it
    /// was.
    pub fn problems(&self) -> Vec<&'static str> {
        let mut wrong = Vec::new();
        if self.amount.0 == 0 {
            wrong.push("Amount should not be 0");
        }
        if self.description.trim().is_empty() {
            wrong.push("Please specify what the money went for");
        }
        if !self.everyone && self.to.is_empty() {
            wrong.push("Pick who this expense is for, or turn on “everyone”");
        }
        if self.from.as_str().is_empty() {
            wrong.push("Pick who paid");
        }
        wrong
    }
}

/// The field the app keeps a row's colour in.
pub const COLOUR: &str = "Color";
/// And the date, which `tc-core` reads but does not model as a field of its own.
pub const SPENDING_DATE: &str = "SpendingDate";

/// Today, as the app writes a date: `YYYY-MM-DD`.
pub fn today() -> String {
    let iso = js_sys::Date::new_0().to_iso_string();
    let text: String = iso.into();
    text.chars().take(10).collect()
}

/// The category the last real spending used - what the app offers first.
fn last_category(tour: &Tour) -> String {
    tour.spendings
        .iter()
        .rev()
        .find(|s| s.kind == Kind::Real && !s.category.trim().is_empty())
        .map(|s| s.category.clone())
        .unwrap_or_default()
}

/// Every category already used in this tour, for the picker.
pub fn categories(tour: &Tour) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for s in &tour.spendings {
        let c = s.category.trim();
        if !c.is_empty() && !seen.iter().any(|x| x == c) {
            seen.push(c.to_owned());
        }
    }
    seen.sort();
    seen
}

/// The tour with this spending added or replaced.
pub fn put_spending(tour: &Tour, draft: &SpendingDraft) -> Tour {
    let mut next = tour.clone();

    // Planned payments are the settlement's own; they are recomputed from the real
    // spendings, so an edit must not leave the old ones lying about.
    next.spendings.retain(|s| s.kind != Kind::Planned);

    let split = if draft.everyone {
        Split::Everyone
    } else {
        split_for(draft.by_weight, draft.to.clone())
    };

    let id = draft
        .id
        .clone()
        .unwrap_or_else(|| SpendingId::new(String::new()));

    match next.spendings.iter_mut().find(|s| s.id == id) {
        Some(existing) => {
            existing.description = draft.description.clone();
            existing.category = draft.category.clone();
            existing.amount = draft.amount;
            existing.from = draft.from.clone();
            set_date(existing, &draft.date);
            set_colour(existing, &draft.colour);
            // What the form had before "everyone" was switched on is kept, as the stored
            // format does.
            if matches!(split, Split::Everyone) {
                let chosen = match (&existing.split, &existing.remembered_split) {
                    (Split::Equally(to) | Split::ByWeight(to), _) => to.clone(),
                    (Split::Everyone, Some(Split::Equally(to) | Split::ByWeight(to))) => to.clone(),
                    _ => Vec::new(),
                };
                existing.remembered_split = Some(split_for(draft.by_weight, chosen));
            }
            existing.split = split;
            set_currency(existing, tour, &draft.currency_id);
        }
        // Not there: this is the add. Replaying it again finds the spending and updates it
        // instead of adding a second one.
        None => {
            let mut fresh = Spending {
                id,
                description: draft.description.clone(),
                category: draft.category.clone(),
                amount: draft.amount,
                currency: tour
                    .currencies
                    .iter()
                    .find(|c| c.id.as_str() == draft.currency_id)
                    .cloned()
                    .unwrap_or_else(|| tour.currency().clone()),
                from: draft.from.clone(),
                remembered_split: matches!(split, Split::Everyone)
                    .then(|| split_for(draft.by_weight, Vec::new())),
                split,
                kind: Kind::Real,
                extras: Default::default(),
            };
            // A new expense always carries a date: without one the app reads it as the day
            // the record was created, which is not what somebody entering last Tuesday's
            // taxi means.
            let when = if draft.date.is_empty() {
                today()
            } else {
                draft.date.clone()
            };
            set_date(&mut fresh, &when);
            set_colour(&mut fresh, &draft.colour);
            next.spendings.push(fresh);
        }
    }
    next
}

/// Writes the day, keeping the time part the stored value had.
///
/// The field is a full timestamp in the data - `2021-08-14T09:12:33.4Z` - and the form only
/// offers a day. Replacing the whole thing with midnight would quietly reorder the list of
/// a day's expenses, which are sorted by it.
fn set_date(spending: &mut Spending, day: &str) {
    if day.is_empty() {
        return;
    }
    // The time from whatever the expense is dated by - `SpendingDate`, or for one entered
    // before that existed, `DateCreated`. Reading only the first stamped every save of an
    // old expense at midnight, and it sank below the ones entered later that same day.
    let existing = spending.when().unwrap_or_default().to_owned();
    let rest = existing.get(10..).unwrap_or("");
    let stamp = if rest.is_empty() {
        format!("{day}T00:00:00")
    } else {
        format!("{day}{rest}")
    };
    tc_core::extras::set(&mut spending.extras, SPENDING_DATE, stamp.into());
}

/// Puts the expense in the currency the form chose.
///
/// Only when the tour actually has that one: a currency it does not list is not a currency,
/// and writing it would make the amount unconvertible - the arithmetic treats such an expense
/// as already being in whatever is on screen, which is how a tour comes to show one figure in
/// dinars and a different one in euro.
fn set_currency(spending: &mut Spending, tour: &Tour, id: &str) {
    if let Some(c) = tour.currencies.iter().find(|c| c.id.as_str() == id) {
        spending.currency = c.clone();
    }
}

/// The people of a tour in the order the app lists them: by name, with anybody paid for
/// sitting under whoever pays for them.
///
/// The C# sorts on the parent's name with the child's appended, which is what puts the
/// family together and keeps it in one piece.
pub fn sorted_people(tour: &Tour) -> Vec<tc_core::Person> {
    fn key(p: &tc_core::Person, tour: &Tour, depth: usize) -> String {
        if depth > 50 {
            return String::new();
        }
        match p.parent.as_ref().and_then(|id| tour.person(id)) {
            Some(parent) => format!("{}{}", key(parent, tour, depth + 1), p.name),
            None => p.name.clone(),
        }
    }
    let mut people = tour.persons.clone();
    people.sort_by_key(|p| key(p, tour, 0));
    people
}

fn set_colour(spending: &mut Spending, colour: &str) {
    if colour.trim().is_empty() {
        tc_core::extras::remove(&mut spending.extras, COLOUR);
    } else {
        tc_core::extras::set(&mut spending.extras, COLOUR, colour.trim().into());
    }
}

pub fn remove_spending(tour: &Tour, id: &SpendingId) -> Tour {
    let mut next = tour.clone();
    next.spendings
        .retain(|s| &s.id != id && s.kind != Kind::Planned);
    next
}

/// What the person dialog collects.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersonDraft {
    pub id: Option<PersonId>,
    pub name: String,
    pub weight: i32,
    pub parent: Option<PersonId>,
}

impl PersonDraft {
    pub fn new() -> PersonDraft {
        PersonDraft {
            id: None,
            name: String::new(),
            // A whole share. The app's default, and the reason weights are in hundredths.
            weight: 100,
            parent: None,
        }
    }

    pub fn of(p: &Person) -> PersonDraft {
        PersonDraft {
            id: Some(p.id.clone()),
            name: p.name.clone(),
            weight: p.weight,
            parent: p.parent.clone(),
        }
    }

    pub fn problem(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("A name, please.");
        }
        if self.weight <= 0 {
            return Some("A weight has to be more than nothing.");
        }
        None
    }
}

pub fn put_person(tour: &Tour, draft: &PersonDraft) -> Tour {
    let mut next = tour.clone();
    next.spendings.retain(|s| s.kind != Kind::Planned);

    let id = draft
        .id
        .clone()
        .unwrap_or_else(|| PersonId::new(String::new()));

    match next.persons.iter_mut().find(|p| p.id == id) {
        Some(existing) => {
            existing.name = draft.name.trim().to_owned();
            existing.weight = draft.weight;
            existing.parent = draft.parent.clone();
        }
        None => next.persons.push(Person {
            id,
            name: draft.name.trim().to_owned(),
            weight: draft.weight,
            parent: draft.parent.clone(),
            group: None,
            extras: Default::default(),
        }),
    }
    next
}

/// Removes a person, and everything that would now point at nobody.
///
/// A spending they paid for cannot stay - it would be money from nowhere - and neither can
/// they remain on the receiving end of one, or in somebody's "paid for by".
pub fn remove_person(tour: &Tour, id: &PersonId) -> Tour {
    let mut next = tour.clone();
    next.persons.retain(|p| &p.id != id);
    for p in next.persons.iter_mut() {
        if p.parent.as_ref() == Some(id) {
            p.parent = None;
        }
    }
    next.spendings
        .retain(|s| &s.from != id && s.kind != Kind::Planned);
    for s in next.spendings.iter_mut() {
        if let Split::Equally(to) | Split::ByWeight(to) = &mut s.split {
            to.retain(|p| p != id);
        }
    }
    // A spending for nobody in particular is a spending for everybody.
    for s in next.spendings.iter_mut() {
        if matches!(&s.split, Split::Equally(to) | Split::ByWeight(to) if to.is_empty()) {
            s.split = Split::Everyone;
        }
    }
    next
}

// --- the tour itself ----------------------------------------------------------------------

/// What the tour dialog collects: the properties that are not people and not expenses.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TourDraft {
    pub name: String,
    /// How many days the trip lasted - what the per-day figures divide by.
    pub days: i64,
    /// Hidden from the default list.
    pub archived: bool,
    /// Everybody sees the payments to make. It changes nothing in the arithmetic; it says
    /// out loud that the tour is being wound up.
    pub finalizing: bool,
}

impl TourDraft {
    pub fn of(tour: &Tour) -> TourDraft {
        TourDraft {
            name: tour.name.clone(),
            days: tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION).unwrap_or(5),
            archived: tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED),
            finalizing: tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING),
        }
    }

    pub fn problem(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("A tour needs a name.");
        }
        if self.days <= 0 {
            return Some("A tour lasts at least a day.");
        }
        None
    }
}

pub fn put_tour(tour: &Tour, draft: &TourDraft) -> Tour {
    let mut next = tour.clone();
    next.name = draft.name.trim().to_owned();
    tc_core::extras::set(
        &mut next.extras,
        tc_core::extras::DURATION,
        draft.days.into(),
    );
    tc_core::extras::set(
        &mut next.extras,
        tc_core::extras::ARCHIVED,
        draft.archived.into(),
    );
    tc_core::extras::set(
        &mut next.extras,
        tc_core::extras::FINALIZING,
        draft.finalizing.into(),
    );
    next
}

/// What the currencies dialog collects.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CurrencyDraft {
    pub id: String,
    pub name: String,
    /// What one unit is worth on any scale you like - only the ratio matters.
    pub rate: i32,
}

impl CurrencyDraft {
    pub fn of(c: &tc_core::Currency) -> CurrencyDraft {
        CurrencyDraft {
            id: c.id.as_str().to_owned(),
            name: c.name.clone(),
            rate: c.rate,
        }
    }

    pub fn is_blank(&self) -> bool {
        self.name.trim().is_empty()
    }
}

/// Why this set of currencies cannot be saved, if it cannot.
///
/// The rate ends up as a divisor, and two currencies with one name are two things nobody
/// can tell apart afterwards. The C# says the same two things in the same words.
pub fn currency_problems(kept: &[CurrencyDraft]) -> Vec<String> {
    let mut problems = Vec::new();
    if kept.is_empty() {
        problems.push("A tour needs at least one currency".to_owned());
        return problems;
    }
    for c in kept.iter().filter(|c| c.rate <= 0) {
        problems.push(format!("“{}” needs a worth above zero", c.name.trim()));
    }
    for (i, c) in kept.iter().enumerate() {
        let name = c.name.trim();
        if kept[..i].iter().any(|other| other.name.trim() == name) {
            problems.push(format!(
                "“{name}” is listed more than once — names have to be unique"
            ));
        }
    }
    problems
}

/// The tour with these currencies, and this one as the main.
///
/// Ids are kept through a rename on purpose: an expense is matched to its currency by id, so
/// keeping it is what lets a currency be renamed without re-reading old expenses in another.
pub fn put_currencies(tour: &Tour, kept: &[CurrencyDraft], main: &str) -> Tour {
    let mut next = tour.clone();
    next.currencies = kept
        .iter()
        .map(|c| {
            // Whatever else the stored currency carried travels on: the one being edited is
            // found by id, and a currency the tour has never seen starts empty.
            let mut fresh = tour
                .currencies
                .iter()
                .find(|old| old.id.as_str() == c.id)
                .cloned()
                .unwrap_or_else(|| tc_core::Currency {
                    id: tc_core::CurrencyId::new(new_id()),
                    name: String::new(),
                    rate: 100,
                    extras: Default::default(),
                });
            fresh.name = c.name.trim().to_owned();
            fresh.rate = c.rate;
            fresh
        })
        .collect();

    let main = next
        .currencies
        .iter()
        .find(|c| c.id.as_str() == main)
        .or_else(|| next.currencies.first())
        .map(|c| c.id.clone());
    if let Some(id) = main {
        next.current_currency = id;
    }
    // A settlement worked out in the old rates is not one in the new ones.
    next.spendings.retain(|s| s.kind != Kind::Planned);
    next
}

/// A suggested payment, recorded as having happened.
///
/// It becomes an ordinary spending from one person to one other, with **no category** -
/// which is what makes it a payback and not money spent on the tour, in the arithmetic and
/// in every total the app shows.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PaymentDraft {
    pub id: Option<SpendingId>,
    pub description: String,
    pub amount: Cents,
    pub currency_id: String,
    pub from: PersonId,
    pub to: PersonId,
}

impl PaymentDraft {
    pub fn of(t: &tc_core::Transfer) -> PaymentDraft {
        PaymentDraft {
            id: None,
            description: t.description.clone(),
            amount: t.amount,
            currency_id: t.currency.id.as_str().to_owned(),
            from: t.from.clone(),
            to: t.to.clone(),
        }
    }
}

pub fn record_payment(tour: &Tour, draft: &PaymentDraft) -> Tour {
    let mut next = tour.clone();
    // The suggested payments are worked out afresh from the real ones, so the old set goes.
    next.spendings.retain(|s| s.kind != Kind::Planned);

    let id = draft
        .id
        .clone()
        .unwrap_or_else(|| SpendingId::new(new_id()));
    if next.spendings.iter().any(|s| s.id == id) {
        return next;
    }

    let currency = tour
        .currencies
        .iter()
        .find(|c| c.id.as_str() == draft.currency_id)
        .unwrap_or_else(|| tour.currency())
        .clone();

    let mut payment = Spending {
        id,
        description: draft.description.clone(),
        category: String::new(),
        amount: draft.amount,
        currency,
        from: draft.from.clone(),
        split: Split::Equally(vec![draft.to.clone()]),
        remembered_split: None,
        kind: Kind::Real,
        extras: Default::default(),
    };
    set_date(&mut payment, &today());
    // The colour the app marks a recorded payment with: green for one it suggested between
    // two people, grey for a family one.
    let colour = if draft.description.starts_with('X') {
        "lightgreen"
    } else {
        "lightgray"
    };
    set_colour(&mut payment, colour);

    next.spendings.push(payment);
    next
}

#[cfg(test)]
mod split_tests {
    use super::*;

    fn tour() -> Tour {
        Tour::from_json(include_str!("../../../fixtures/zscph2y.tour.json")).expect("fixture")
    }

    /// Built by hand: `SpendingDraft::new` asks the browser for today's date.
    fn partial(t: &Tour) -> SpendingDraft {
        SpendingDraft {
            id: Some(SpendingId::new("split-test")),
            description: "for some".into(),
            category: "Еда".into(),
            amount: Cents(1000),
            from: t.persons[0].id.clone(),
            everyone: false,
            to: t.persons.iter().take(3).map(|p| p.id.clone()).collect(),
            by_weight: true,
            date: "2021-08-14".into(),
            colour: String::new(),
            currency_id: t.currency().id.as_str().to_owned(),
        }
    }

    fn written(t: &Tour) -> serde_json::Value {
        let json: serde_json::Value = serde_json::from_str(&t.to_json().unwrap()).unwrap();
        json["Spendings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["GUID"] == "split-test")
            .unwrap()
            .clone()
    }

    /// A new expense for some of the people is shared by weight, as in the app - and is
    /// written with the flag the app reads that from.
    #[test]
    fn a_partial_split_is_by_weight_unless_asked() {
        let t = tour();
        let next = put_spending(&t, &partial(&t));
        assert!(matches!(
            next.spendings.iter().find(|s| s.id.as_str() == "split-test").unwrap().split,
            Split::ByWeight(_)
        ));
        assert_eq!(written(&next)["IsPartialWeighted"], true);

        let mut equal = partial(&t);
        equal.by_weight = false;
        let next = put_spending(&t, &equal);
        assert_eq!(written(&next)["IsPartialWeighted"], false);
    }

    /// Opening an expense and saving it keeps how it was divided, both ways - and through
    /// "everyone" and back.
    #[test]
    fn an_edit_keeps_the_way_it_was_divided() {
        let t = tour();
        for by_weight in [true, false] {
            let mut d = partial(&t);
            d.by_weight = by_weight;
            let once = put_spending(&t, &d);
            let s = once.spendings.iter().find(|s| s.id.as_str() == "split-test").unwrap();
            let again = SpendingDraft::of(s);
            assert_eq!(again.by_weight, by_weight);

            let mut all = again.clone();
            all.everyone = true;
            let everyone = put_spending(&once, &all);
            let s = everyone.spendings.iter().find(|s| s.id.as_str() == "split-test").unwrap();
            let back = SpendingDraft::of(s);
            assert!(back.everyone);
            assert_eq!(back.by_weight, by_weight, "remembered through everyone");
            assert_eq!(written(&everyone)["IsPartialWeighted"], by_weight);
        }
    }

    /// Saving an old expense - dated only by when it was created - keeps its time of day.
    #[test]
    fn a_saved_expense_keeps_its_time_of_day() {
        let t = tour();
        let mut s = t.spendings.iter().find(|s| s.kind == Kind::Real).unwrap().clone();
        tc_core::extras::remove(&mut s.extras, SPENDING_DATE);
        tc_core::extras::set(&mut s.extras, "DateCreated", "2021-08-17T11:39:04Z".into());
        set_date(&mut s, "2021-08-17");
        assert_eq!(s.when(), Some("2021-08-17T11:39:04Z"));
        set_date(&mut s, "2021-08-18");
        assert_eq!(s.when(), Some("2021-08-18T11:39:04Z"));
    }

    /// An edit queued before the choice existed reads as by weight.
    #[test]
    fn an_old_queued_draft_is_by_weight() {
        let t = tour();
        let mut value = serde_json::to_value(partial(&t)).unwrap();
        value.as_object_mut().unwrap().remove("by_weight");
        let d: SpendingDraft = serde_json::from_value(value).unwrap();
        assert!(d.by_weight);
    }
}
