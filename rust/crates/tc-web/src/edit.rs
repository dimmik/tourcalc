//! Changing a tour, and saving it.
//!
//! Every edit is the same shape: take the tour that is on screen, produce a changed one,
//! and PATCH the whole thing. That is how the app has always saved - the API has no
//! finer-grained calls - and it is why the functions here take `&Tour` and return `Tour`
//! rather than mutating in place: the caller keeps what it had until the server has agreed,
//! and can put it back if it has not.

use crate::i18n::t;
use tc_core::{Cents, Kind, Person, PersonId, Spending, SpendingId, Split, Tour};

/// A new id, in the style the app uses: short, lowercase, URL-safe.
///
/// The C# builds one from the clock plus a little randomness. So does this, with the clock
/// the browser has; ids only have to be unique within a tour, and two edits a millisecond
/// apart on the same device are not a thing that happens.
pub fn new_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    // The tests run natively, where there is no browser to ask: the system clock and a
    // counter instead, which is all an id in a test needs to be.
    let (ms, noise) = if cfg!(target_arch = "wasm32") {
        (js_sys::Date::now() as u64, (js_sys::Math::random() * 1024.0) as u64)
    } else {
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        (ms, COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 1024)
    };
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
    /// Whether this edits an expense that was already there, as opposed to adding one.
    ///
    /// Replayed onto a tour where that expense is gone - somebody deleted it while this
    /// edit waited for a network - an edit is dropped rather than bringing the expense back.
    /// Without it the two cases looked alike: "no spending with this id" was taken to mean
    /// "not added yet", and a deleted taxi fare returned with the edit. False when read from
    /// an edit queued before the field existed, which is how those were treated anyway.
    #[serde(default)]
    pub editing: bool,
    /// Whether `amount` is hundredths - the currency's cents as they were when the edit was
    /// made. Waiting in the queue while somebody switched that currency's cents, it would be
    /// read in the other unit: 12 € as 0,12 € or 1 200 €. `None` for an edit queued before
    /// this was recorded - taken as it is, as it always was.
    #[serde(default)]
    pub in_cents: Option<bool>,
}

/// `amount`, recorded as hundredths or not, in the unit `currency` counts in now.
fn in_units_now(amount: Cents, recorded: Option<bool>, tour: &Tour, currency: &tc_core::CurrencyId) -> Cents {
    match recorded {
        Some(was) if was != tour.counts_cents(currency) => {
            if was {
                tc_core::money::convert(amount, 1, 100)
            } else {
                Cents(amount.0.saturating_mul(100))
            }
        }
        _ => amount,
    }
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
            editing: false,
            in_cents: None,
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
            editing: true,
            in_cents: None,
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
            wrong.push(t().checks.amount_zero);
        }
        if self.description.trim().is_empty() {
            wrong.push(t().checks.no_description);
        }
        if !self.everyone && self.to.is_empty() {
            wrong.push(t().checks.for_nobody);
        }
        if self.from.as_str().is_empty() {
            wrong.push(t().checks.no_payer);
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
            let currency = if draft.currency_id.is_empty() {
                existing.currency.id.clone()
            } else {
                tc_core::CurrencyId::new(draft.currency_id.clone())
            };
            existing.amount = in_units_now(draft.amount, draft.in_cents, tour, &currency);
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
        // Not there, and it was: somebody else deleted it. Their delete stands.
        None if draft.editing => return tour.clone(),
        // Not there: this is the add. Replaying it again finds the spending and updates it
        // instead of adding a second one.
        None => {
            let currency = tour
                .currencies
                .iter()
                .find(|c| c.id.as_str() == draft.currency_id)
                .cloned()
                .unwrap_or_else(|| tour.currency().clone());
            let mut fresh = Spending {
                id,
                description: draft.description.clone(),
                category: draft.category.clone(),
                amount: in_units_now(draft.amount, draft.in_cents, tour, &currency.id),
                currency,
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
    /// Whether this edits somebody already in the tour. See [`SpendingDraft::editing`].
    #[serde(default)]
    pub editing: bool,
}

impl PersonDraft {
    pub fn new() -> PersonDraft {
        PersonDraft {
            id: None,
            name: String::new(),
            // A whole share. The app's default, and the reason weights are in hundredths.
            weight: 100,
            parent: None,
            editing: false,
        }
    }

    pub fn of(p: &Person) -> PersonDraft {
        PersonDraft {
            id: Some(p.id.clone()),
            name: p.name.clone(),
            weight: p.weight,
            parent: p.parent.clone(),
            editing: true,
        }
    }

    pub fn problem(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some(t().checks.no_name);
        }
        if self.weight <= 0 {
            return Some(t().checks.no_weight);
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
        // Removed by somebody else while this waited: their removal stands.
        None if draft.editing => return tour.clone(),
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

/// Removes a person - unless an expense still holds them, and then nothing changes.
///
/// The screen asks before recording this (see `tc_core::removal`), so a refusal here is
/// the rare case of a queued removal replayed onto a tour where somebody has since recorded
/// an expense paid by them: that expense is kept, and so are they.
pub fn remove_person(tour: &Tour, id: &PersonId) -> Tour {
    tc_core::removal::without_person(tour, id).unwrap_or_else(|| tour.clone())
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
            return Some(t().checks.tour_no_name);
        }
        if self.days <= 0 {
            return Some(t().checks.tour_no_days);
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
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CurrencyDraft {
    pub id: String,
    pub name: String,
    /// What one unit is worth on any scale you like - only the ratio matters. The dialog shows
    /// it as a rate against another currency and chooses the scale itself (`rates::room`).
    pub rate: i64,
    /// Amounts are hundredths, read and written with a decimal part. See `tc_core::units`.
    #[serde(default)]
    pub cents: bool,
    /// A new currency whose cents nobody has chosen yet: they follow its worth
    /// (`tc_core::units::cents_by_default`) until somebody ticks or unticks the box.
    #[serde(default)]
    pub cents_auto: bool,
    /// Switching cents on: fold this currency's "EURc" into it.
    #[serde(default)]
    pub absorb: bool,
}

impl CurrencyDraft {
    pub fn of(c: &tc_core::Currency) -> CurrencyDraft {
        CurrencyDraft {
            id: c.id.as_str().to_owned(),
            name: c.name.clone(),
            rate: c.rate,
            cents: c.with_cents(),
            cents_auto: false,
            absorb: false,
        }
    }

    /// A row for a currency to be added: cents decided by its worth until somebody decides.
    /// Worth 10 000 to start: a currency added as the cheapest leaves room for the others to
    /// come out to four figures and, with cents, two zeros after them - a mark or a lev at
    /// sixty times the dinar is 601 100 - without "today's rate" having to offer to multiply.
    pub fn blank() -> CurrencyDraft {
        CurrencyDraft {
            rate: 10_000,
            cents_auto: true,
            ..Default::default()
        }
    }

    /// Whether this currency will have cents: what was chosen, or - for a new one nobody
    /// has decided about - what its worth against the others suggests.
    pub fn effective_cents(&self, all: &[CurrencyDraft]) -> bool {
        if !self.cents_auto {
            return self.cents;
        }
        let others: Vec<i64> = all
            .iter()
            .filter(|c| !c.is_blank() && !std::ptr::eq(*c, self))
            .map(|c| c.rate)
            .collect();
        tc_core::units::cents_by_default(self.rate, &self.name, &others)
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
        problems.push(t().checks.no_currency.to_owned());
        return problems;
    }
    for c in kept.iter().filter(|c| c.rate <= 0) {
        problems.push((t().checks.rate_zero)(c.name.trim()));
    }
    for (i, c) in kept.iter().enumerate() {
        let name = c.name.trim();
        if kept[..i].iter().any(|other| other.name.trim() == name) {
            problems.push((t().checks.duplicate_currency)(name));
        }
    }
    problems
}

/// The tour with these currencies, and this one as the main.
///
/// Ids are kept through a rename on purpose: an expense is matched to its currency by id, so
/// keeping it is what lets a currency be renamed without re-reading old expenses in another.
pub fn put_currencies(tour: &Tour, kept: &[CurrencyDraft], main: &str) -> Tour {
    // A plan that cannot be carried out was refused by the dialog before it got here; a
    // queued one replayed against a tour that has since changed leaves the tour as it is.
    plan_currencies(tour, kept, main).tour
}

/// Gives every currency being added its id now, when the edit is recorded: the ISO code when
/// its name is one and it is free, a fresh id otherwise.
///
/// Decided when the edit is carried out, a save that reached the server but whose answer was
/// lost was replayed as a second addition - the `EUR` taken by the first, the second got a
/// random id, and the tour had two euros. The same reason spendings get theirs up front.
pub fn settle_new_ids(tour: &Tour, kept: &mut [CurrencyDraft]) {
    let mut taken: Vec<String> = tour
        .currencies
        .iter()
        .map(|c| c.id.as_str().to_owned())
        .chain(kept.iter().map(|c| c.id.clone()).filter(|id| !id.is_empty()))
        .collect();
    for c in kept.iter_mut().filter(|c| c.id.is_empty()) {
        let refs: Vec<&str> = taken.iter().map(String::as_str).collect();
        c.id = crate::rates::id_for_new(&c.name, &refs).unwrap_or_else(new_id);
        taken.push(c.id.clone());
    }
}

/// What saving the currencies dialog will do, worked out before it is done - so the dialog
/// can say it, and refuse what cannot be done.
#[derive(Clone, Debug)]
pub struct CurrencyPlan {
    pub tour: Tour,
    /// A removed currency's expenses, moved into another: (from, into, how many).
    pub moved: Vec<(String, String, usize)>,
    /// Expenses with cents rounded to whole units by switching a currency's cents off.
    pub rounded: usize,
    /// An "EURc" folded into its currency: (the EURc, the currency, how many expenses).
    pub absorbed: Vec<(String, String, usize)>,
}

/// The tour with these currencies, and this one as the main.
///
/// In this order, each step on what the last one left:
///
/// 1. a currency that is gone takes its expenses into the cheapest one left, at the worths
///    the tour had **before** this edit - they used to be read, silently, in the main
///    currency instead. Before, not after: the dialog may have rescaled every worth ("today's
///    rate" multiplying by 10, or a dinar retyped from 100 to 1 000), and the removed
///    currency's worth is only known on the old scale - read against a new one, a 410-dinar
///    coffee came out at 41;
/// 2. names and worths as the dialog has them; a new currency starts with the cents it was
///    given, and has nothing to convert;
/// 3. a currency whose cents were switched has its expenses and the worths rescaled
///    (`tc_core::units::switch_cents`), which keeps every figure;
/// 4. an "EURc" beside a euro that now has cents of its own is folded into it.
pub fn plan_currencies(
    tour: &Tour,
    kept: &[CurrencyDraft],
    main: &str,
) -> CurrencyPlan {
    use tc_core::units::{cents_sibling, cheapest, move_spendings, switch_cents};
    let mut next = tour.clone();
    let mut plan = CurrencyPlan {
        tour: Tour { spendings: Vec::new(), persons: Vec::new(), ..tour.clone() },
        moved: Vec::new(),
        rounded: 0,
        absorbed: Vec::new(),
    };

    // 1. What is gone, and had expenses in it - on the tour's own worths, one scale.
    let gone: Vec<tc_core::Currency> = tour
        .currencies
        .iter()
        .filter(|old| !kept.iter().any(|c| c.id == old.id.as_str()))
        .cloned()
        .collect();
    let mut orphans: Vec<tc_core::Currency> = Vec::new();
    for old in gone {
        if !next.spendings.iter().any(|s| s.currency.id == old.id) {
            continue;
        }
        // Money into money: of what is left, the cheapest currency that is a currency - chips
        // cheaper than the dinar are not where a removed euro's expenses belong. Only when
        // nothing left is recognisable, the cheapest of whatever there is.
        let left: Vec<&tc_core::Currency> = next
            .currencies
            .iter()
            .filter(|c| c.id != old.id && kept.iter().any(|k| k.id == c.id.as_str()))
            .collect();
        let money: Vec<&&tc_core::Currency> = left
            .iter()
            .filter(|c| !crate::rates::units_of(c.id.as_str(), &c.name).is_empty())
            .collect();
        let into = money
            .iter()
            .min_by_key(|c| c.rate)
            .map(|c| (**c).clone())
            .or_else(|| left.iter().min_by_key(|c| c.rate).map(|c| (*c).clone()));
        match into {
            Some(into) => {
                let n = move_spendings(&mut next, &old.id, &into.id);
                plan.moved.push((old.name.clone(), into.name.clone(), n));
            }
            // Nothing of the old tour is kept - only new currencies: see after step 2.
            None => orphans.push(old),
        }
    }

    // 2. What is kept, as named and worth now - cents as they were, for the moment. A new
    // currency named by its code gets the code as its id (`rates::id_for_new`).
    let mut new_ids: Vec<tc_core::CurrencyId> = Vec::new();
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
                .unwrap_or_else(|| {
                    let taken: Vec<&str> = tour
                        .currencies
                        .iter()
                        .map(|x| x.id.as_str())
                        .chain(new_ids.iter().map(|x| x.as_str()))
                        .collect();
                    // Settled when the edit was recorded (`settle_new_ids`), so a replay finds
                    // the currency it added; an edit queued before that is settled here.
                    let id = if c.id.is_empty() {
                        crate::rates::id_for_new(&c.name, &taken).unwrap_or_else(new_id)
                    } else {
                        c.id.clone()
                    };
                    let mut new = tc_core::Currency {
                        id: tc_core::CurrencyId::new(id),
                        name: String::new(),
                        rate: 10_000,
                        extras: Default::default(),
                    };
                    new.set_with_cents(c.effective_cents(kept));
                    new_ids.push(new.id.clone());
                    new
                });
            fresh.name = c.name.trim().to_owned();
            fresh.rate = c.rate;
            fresh
        })
        .collect();

    // Every old currency replaced by new ones at once: there is no old worth to go by on the
    // other side, so the new one's is taken as it is - the best there is.
    for old in orphans {
        let Some(into) = cheapest(&next, None).cloned() else { continue };
        next.currencies.push(old.clone());
        let n = move_spendings(&mut next, &old.id, &into.id);
        next.currencies.retain(|c| c.id != old.id);
        plan.moved.push((old.name.clone(), into.name.clone(), n));
    }

    // 3. Cents switched on or off where there were already expenses to convert.
    for c in kept {
        let id = tc_core::CurrencyId::new(c.id.clone());
        if c.id.is_empty() || new_ids.contains(&id) {
            continue;
        }
        let done = switch_cents(&mut next, &id, c.cents);
        plan.rounded += done.rounded;
    }

    // 4. An EURc beside a euro that now counts cents itself.
    for c in kept.iter().filter(|c| c.cents && c.absorb) {
        let id = tc_core::CurrencyId::new(c.id.clone());
        let Some(sibling) = cents_sibling(&next, &id).cloned() else { continue };
        let n = move_spendings(&mut next, &sibling.id, &id);
        next.currencies.retain(|x| x.id != sibling.id);
        let into = next.currencies.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_default();
        plan.absorbed.push((sibling.name.clone(), into, n));
    }

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
    plan.tour = next;
    plan
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
    /// As on [`SpendingDraft::in_cents`].
    #[serde(default)]
    pub in_cents: Option<bool>,
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
            in_cents: Some(t.currency.with_cents()),
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
        amount: in_units_now(draft.amount, draft.in_cents, tour, &currency.id),
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
            editing: false,
            in_cents: None,
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

#[cfg(test)]
mod currency_tests {
    use super::*;

    /// A tour in dinars with euros and euro cents, as the tours of 2025-2026 are.
    fn tour() -> Tour {
        let json = serde_json::json!({
            "Id": "t", "Name": "t", "Persons": [{"GUID": "p", "Name": "P", "Weight": 100}],
            "Currencies": [
                {"_id": "RSD", "Name": "RSD", "CurrencyRate": 1000},
                {"_id": "EUR", "Name": "EUR", "CurrencyRate": 117000},
                {"_id": "EURc", "Name": "EURc", "CurrencyRate": 1170}
            ],
            "TourCurrencyId": "RSD",
            "Spendings": [
                {"GUID": "a", "Description": "dinner", "Type": "Food", "AmountInCents": 12,
                 "FromGuid": "p", "ToAll": true, "ToGuid": [],
                 "Currency": {"_id": "EUR", "Name": "EUR", "CurrencyRate": 117000}},
                {"GUID": "b", "Description": "coffee", "Type": "Food", "AmountInCents": 350,
                 "FromGuid": "p", "ToAll": true, "ToGuid": [],
                 "Currency": {"_id": "EURc", "Name": "EURc", "CurrencyRate": 1170}}
            ]
        });
        Tour::from_json(&json.to_string()).expect("tour")
    }

    fn drafts(t: &Tour) -> Vec<CurrencyDraft> {
        t.currencies.iter().map(CurrencyDraft::of).collect()
    }

    fn in_dinars(t: &Tour) -> Vec<Cents> {
        t.spendings.iter().map(|s| t.convert(s.amount, &s.currency)).collect()
    }

    /// An edit of 12 whole euros waited in the queue while somebody gave the euro cents: it
    /// lands as 12,00 €, not 0,12 €. And the other way round.
    #[test]
    fn a_queued_amount_follows_a_switch_of_cents() {
        let t = tour(); // EUR without cents
        let mut draft = SpendingDraft::of(&t.spendings[0]); // 12 EUR
        draft.in_cents = Some(false);
        let mut with_cents = t.clone();
        tc_core::units::switch_cents(&mut with_cents, &tc_core::CurrencyId::new("EUR"), true);
        assert_eq!(put_spending(&with_cents, &draft).spendings[0].amount, Cents(1200));
        // Recorded in hundredths, applied where the euro has none any more.
        draft.amount = Cents(1250);
        draft.in_cents = Some(true);
        assert_eq!(put_spending(&t, &draft).spendings[0].amount, Cents(13));
        // Queued before this was recorded: as it is.
        draft.in_cents = None;
        assert_eq!(put_spending(&with_cents, &draft).spendings[0].amount, Cents(1250));
    }

    /// A save whose answer was lost is replayed: the euro it added is found, not added again.
    #[test]
    fn a_replayed_addition_adds_one_currency() {
        let t = tour();
        let mut kept = drafts(&t);
        kept.retain(|c| c.id != "EUR");
        kept.push(CurrencyDraft { name: "EUR".into(), rate: 117_500, ..CurrencyDraft::blank() });
        settle_new_ids(&t, &mut kept);
        assert!(kept.iter().all(|c| !c.id.is_empty()), "every currency has its id before it is saved");
        let once = put_currencies(&t, &kept, "RSD");
        let twice = put_currencies(&once, &kept, "RSD");
        let euros = twice.currencies.iter().filter(|c| c.name == "EUR").count();
        assert_eq!(euros, 1, "{:?}", twice.currencies.iter().map(|c| c.id.as_str()).collect::<Vec<_>>());
    }

    /// Chips cheaper than the dinar: a removed euro's expenses still go into dinars.
    #[test]
    fn a_removed_currency_goes_into_money_not_chips() {
        let mut t = tour();
        t.currencies.push(tc_core::Currency {
            id: tc_core::CurrencyId::new("chips"),
            name: "Chips".into(),
            rate: 1,
            extras: Default::default(),
        });
        let mut kept = drafts(&t);
        kept.retain(|c| c.id != "EUR");
        let plan = plan_currencies(&t, &kept, "RSD");
        assert_eq!(plan.moved, vec![("EUR".to_owned(), "RSD".to_owned(), 1)]);
    }

    /// Every worth multiplied by ten ("today's rate") and the EURc removed, in one save: the
    /// coffee in EURc is still 410 dinars, not 41.
    #[test]
    fn removing_a_currency_while_rescaling_keeps_its_expenses() {
        let t = tour();
        let before = in_dinars(&t);
        let mut kept = drafts(&t);
        kept.retain(|c| c.id != "EURc");
        for c in kept.iter_mut() {
            c.rate *= 10;
        }
        let plan = plan_currencies(&t, &kept, "RSD");
        assert_eq!(in_dinars(&plan.tour), before);
        assert_eq!(plan.moved, vec![("EURc".to_owned(), "RSD".to_owned(), 1)]);
    }

    #[test]
    fn a_new_currency_named_by_its_code_is_keyed_by_it() {
        let t = tour();
        let mut kept = drafts(&t);
        for (name, rate) in [("bam", 60_000), ("Eur", 117_000), ("Chips", 5)] {
            kept.push(CurrencyDraft { name: name.into(), rate, ..CurrencyDraft::blank() });
        }
        let ids: Vec<String> = plan_currencies(&t, &kept, "RSD")
            .tour
            .currencies
            .iter()
            .map(|c| c.id.as_str().to_owned())
            .collect();
        assert_eq!(&ids[..4], ["RSD", "EUR", "EURc", "BAM"]);
        // EUR is taken; chips are no code: both get an id of the usual kind.
        assert!(ids[4] != "EUR" && ids[4].len() == 7, "{ids:?}");
        assert!(ids[5].len() == 7, "{ids:?}");
    }

    #[test]
    fn giving_eur_cents_and_folding_eurc_in_keeps_every_figure() {
        let t = tour();
        let before = in_dinars(&t);
        let mut kept = drafts(&t);
        kept[1].cents = true;
        kept[1].absorb = true;
        let plan = plan_currencies(&t, &kept, "RSD");
        assert_eq!(plan.absorbed, vec![("EURc".to_owned(), "EUR".to_owned(), 1)]);
        assert_eq!(plan.rounded, 0);
        let after = plan.tour;
        assert_eq!(after.currencies.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["RSD", "EUR"]);
        assert!(after.currencies[1].with_cents());
        assert_eq!(after.spendings.iter().map(|s| s.amount.0).collect::<Vec<_>>(), [1200, 350]);
        assert!(after.spendings.iter().all(|s| s.currency.id.as_str() == "EUR"));
        assert_eq!(in_dinars(&after), before);
    }

    #[test]
    fn a_removed_currency_takes_its_expenses_into_the_cheapest_one_left() {
        let t = tour();
        let before = in_dinars(&t);
        let kept: Vec<CurrencyDraft> = drafts(&t).into_iter().filter(|c| c.id != "EUR").collect();
        let plan = plan_currencies(&t, &kept, "RSD");
        assert_eq!(plan.moved, vec![("EUR".to_owned(), "RSD".to_owned(), 1)]);
        assert_eq!(plan.tour.spendings[0].currency.id.as_str(), "RSD");
        assert_eq!(in_dinars(&plan.tour), before);
    }

    #[test]
    fn a_new_currency_starts_with_the_cents_its_worth_suggests() {
        let t = tour();
        let mut kept = drafts(&t);
        let mut usd = CurrencyDraft::blank();
        usd.name = "USD".into();
        usd.rate = 108000;
        kept.push(usd);
        let plan = plan_currencies(&t, &kept, "RSD");
        assert!(plan.tour.currencies.iter().find(|c| c.name == "USD").unwrap().with_cents());
    }

    #[test]
    fn a_queued_edit_from_before_cents_still_reads() {
        let old = r#"{"id":"EUR","name":"EUR","rate":117000}"#;
        let d: CurrencyDraft = serde_json::from_str(old).expect("reads");
        assert!(!d.cents && !d.cents_auto && !d.absorb);
    }
}
