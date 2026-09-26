//! Edits that have not reached the server yet, and the tour they were made on.
//!
//! Until now an edit went straight out as a PATCH and the screen waited for the answer.
//! That is fine on a train platform with signal and useless without one, which is the
//! situation this app is actually used in - somebody adding a taxi fare in a country where
//! their data does not work.
//!
//! So an edit is written down instead of sent: it changes the copy in the browser, joins a
//! queue, and the queue is drained whenever the network allows. Two things follow, and they
//! are the reason this is a queue of *operations* rather than of finished tours:
//!
//! * a conflict can be resolved rather than reported. If somebody else saved first, the
//!   operations are replayed on top of *their* tour, and both sets of edits survive.
//! * the tour on screen is never a guess. It is the last one from the server with the
//!   pending operations applied - the same computation the sync will do.

use crate::i18n::t;
use crate::edit::{self, CurrencyDraft, PaymentDraft, PersonDraft, SpendingDraft, TourDraft};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use tc_core::{PersonId, SpendingId, Tour};

/// One thing somebody did.
///
/// Deliberately the intent ("this person's weight is now 50") and not the result ("here is
/// the whole tour"). A result cannot be replayed onto a tour that has moved on; an intent
/// can.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    PutSpending(SpendingDraft),
    RemoveSpending(SpendingId),
    PutPerson(PersonDraft),
    RemovePerson(PersonId),
    Rename(String),
    /// Which of the tour's currencies the amounts are shown in. A property of the tour and
    /// not of the reader, as it has always been - everybody sees the same figures.
    SetCurrency(String),
    /// The tour's own properties: its name, its length, whether it is archived or being
    /// settled up.
    EditTour(TourDraft),
    /// The currencies themselves, and which of them the totals are worked out in.
    SetCurrencies {
        kept: Vec<CurrencyDraft>,
        main: String,
    },
    /// One of the suggested payments, recorded as having happened.
    RecordPayment(PaymentDraft),
}

impl Operation {
    /// What this edit was about, if carrying it out onto `tour` would drop it because
    /// somebody else deleted the thing it edits - see [`SpendingDraft::editing`].
    pub fn lost_on(&self, tour: &Tour) -> Option<String> {
        match self {
            Operation::PutSpending(d) if d.editing => {
                let id = d.id.as_ref()?;
                (!tour.spendings.iter().any(|s| &s.id == id))
                    .then(|| format!("“{}”", short(&d.description)))
            }
            Operation::PutPerson(d) if d.editing => {
                let id = d.id.as_ref()?;
                (!tour.persons.iter().any(|p| &p.id == id)).then(|| short(&d.name))
            }
            _ => None,
        }
    }

    /// The tour as it would be with this operation carried out.
    pub fn apply(&self, tour: &Tour) -> Tour {
        match self {
            Operation::PutSpending(d) => edit::put_spending(tour, d),
            Operation::RemoveSpending(id) => edit::remove_spending(tour, id),
            Operation::PutPerson(d) => edit::put_person(tour, d),
            Operation::RemovePerson(id) => edit::remove_person(tour, id),
            Operation::Rename(name) => {
                let mut next = tour.clone();
                next.name = name.clone();
                next
            }
            Operation::SetCurrency(id) => {
                let mut next = tour.clone();
                if next.currencies.iter().any(|c| c.id.as_str() == id) {
                    next.current_currency = tc_core::CurrencyId::new(id.clone());
                }
                next
            }
            Operation::EditTour(draft) => edit::put_tour(tour, draft),
            Operation::SetCurrencies { kept, main } => edit::put_currencies(tour, kept, main),
            Operation::RecordPayment(draft) => edit::record_payment(tour, draft),
        }
    }

    /// What the status line says is waiting.
    ///
    /// Says what the edit is about and not whether it adds or changes: since the id is
    /// settled when the operation is recorded, "adding" and "editing" are the same
    /// operation carried out against a tour that either has that id already or does not.
    /// An earlier version guessed from `id.is_some()` and called every add an edit - the
    /// sort of thing that keeps compiling perfectly.
    pub fn describe(&self) -> String {
        match self {
            Operation::PutSpending(d) => format!("“{}”", short(&d.description)),
            Operation::RemoveSpending(_) => t().queue.expense_removed.into(),
            Operation::PutPerson(d) => short(&d.name),
            Operation::RemovePerson(_) => t().queue.person_removed.into(),
            Operation::Rename(name) => (t().queue.renamed)(&short(name)),
            Operation::SetCurrency(id) => (t().queue.amounts_in)(id.as_str()),
            Operation::EditTour(d) => (t().queue.the_tour)(&short(&d.name)),
            Operation::SetCurrencies { .. } => t().queue.the_currencies.into(),
            Operation::RecordPayment(d) => (t().queue.paid)(&short(&d.description)),
        }
    }
}

fn short(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return t().queue.no_description.into();
    }
    if s.chars().count() > 24 {
        format!("{}…", s.chars().take(24).collect::<String>())
    } else {
        s.to_owned()
    }
}

pub(crate) fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn queue_key(tour: &str) -> String {
    format!("__tcw_queue_{tour}")
}

fn tour_key(tour: &str) -> String {
    format!("__tcw_tour_{tour}")
}

fn stamp_key(tour: &str) -> String {
    format!("__tcw_tour_at_{tour}")
}

fn refused_key(tour: &str) -> String {
    format!("__tcw_refused_{tour}")
}

/// How many times in a row the server may refuse a queue before it stops being retried by
/// itself. A refusal is an answer, not a missing network - the tour is gone, the login no
/// longer covers it, the server will not take the tour as it is - and asking again every
/// twenty seconds for ever changes nothing. Three, so that one odd answer is not the end.
pub const GIVE_UP_AFTER: u32 = 3;

/// The server's last refusal of this tour's queue, and how many in a row there have been.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Refused {
    pub times: u32,
    pub why: String,
}

impl Refused {
    /// Whether to stop sending this queue until somebody says otherwise.
    pub fn given_up(&self) -> bool {
        self.times >= GIVE_UP_AFTER
    }
}

/// How the server has answered this tour's queue lately; `None` when it has not refused.
pub fn refused(tour: &str) -> Option<Refused> {
    let text = storage()?.get_item(&refused_key(tour)).ok().flatten()?;
    serde_json::from_str(&text).ok()
}

/// Whether sending this tour's queue has been given up on. See [`GIVE_UP_AFTER`].
pub fn given_up(tour: &str) -> bool {
    refused(tour).is_some_and(|r| r.given_up())
}

/// Notes one more refusal.
pub fn was_refused(tour: &str, why: &str) {
    let Some(s) = storage() else { return };
    let mut r = refused(tour).unwrap_or_default();
    r.times = r.times.saturating_add(1);
    r.why = why.to_owned();
    if let Ok(text) = serde_json::to_string(&r) {
        let _ = s.set_item(&refused_key(tour), &text);
    }
    changed();
}

/// Forgets the refusals: the queue went through, was thrown away, or is to be tried again.
pub fn not_refused(tour: &str) {
    let Some(s) = storage() else { return };
    if s.get_item(&refused_key(tour)).ok().flatten().is_some() {
        let _ = s.remove_item(&refused_key(tour));
        changed();
    }
}

fn lost_key(tour: &str) -> String {
    format!("__tcw_lost_{tour}")
}

/// Edits that were dropped when they reached the server, because what they edited had
/// been deleted there in the meantime - by name, for the line that says so.
pub fn lost(tour: &str) -> Vec<String> {
    storage()
        .and_then(|s| s.get_item(&lost_key(tour)).ok().flatten())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Adds to what [`lost`] says; `None` forgets it, once the reader has seen it.
pub fn set_lost(tour: &str, what: Option<&[String]>) {
    let Some(s) = storage() else { return };
    match what {
        Some(names) if !names.is_empty() => {
            let mut all = lost(tour);
            all.extend(names.iter().cloned());
            if let Ok(text) = serde_json::to_string(&all) {
                let _ = s.set_item(&lost_key(tour), &text);
            }
        }
        Some(_) => return,
        None => {
            let _ = s.remove_item(&lost_key(tour));
        }
    }
    changed();
}

/// Throws away what is waiting for this tour - what the reader asks for when the server
/// will not take it.
pub fn discard(tour: &str) {
    not_refused(tour);
    set_pending(tour, &[]);
}

/// One key for the whole list, not one per tour: it is a screen, not a set of documents.
const LIST_KEY: &str = "__tcw_tourlist";

thread_local! {
    /// Bumped whenever anything is queued or sent, so a screen drawn from what is waiting -
    /// the tour's amber line, the list's "not sent yet" - follows the queue itself rather
    /// than the state of the last request. It used to follow the request: every load set
    /// "checking" for as long as it lasted, and the line vanished and came back with it.
    ///
    /// Set once by the app, because a signal belongs to the reactive root; before that, and
    /// in a test, there is none and the screens simply do not track it.
    static CHANGED: std::cell::Cell<Option<RwSignal<u32>>> = const { std::cell::Cell::new(None) };
}

/// Hands the queue the signal it reports changes on. Called once, by the app.
pub fn reports_changes_on(signal: RwSignal<u32>) {
    CHANGED.with(|c| c.set(Some(signal)));
}

/// Reads the counter, so the caller re-runs when a queue changes. Nothing to read before
/// the app has started.
pub fn changes() {
    if let Some(signal) = CHANGED.with(|c| c.get()) {
        signal.track();
    }
}

fn changed() {
    if let Some(signal) = CHANGED.with(|c| c.get()) {
        signal.try_update(|n| *n = n.wrapping_add(1));
    }
}

/// What is still waiting to be sent for this tour.
pub fn pending(tour: &str) -> Vec<Operation> {
    storage()
        .and_then(|s| s.get_item(&queue_key(tour)).ok().flatten())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn set_pending(tour: &str, ops: &[Operation]) {
    let Some(s) = storage() else { return };
    if ops.is_empty() {
        let _ = s.remove_item(&queue_key(tour));
    } else if let Ok(text) = serde_json::to_string(ops) {
        let _ = s.set_item(&queue_key(tour), &text);
    }
    changed();
}

/// Every tour with edits still waiting to be sent, by id.
///
/// Read from the keys themselves: a queue belongs to the tour it is named after, and a
/// separate list of them would be a second thing to keep in step.
pub fn tours_with_pending() -> Vec<String> {
    let Some(s) = storage() else {
        return Vec::new();
    };
    let prefix = queue_key("");
    let mut tours = Vec::new();
    for i in 0..s.length().unwrap_or(0) {
        let Ok(Some(key)) = s.key(i) else { continue };
        if let Some(tour) = key.strip_prefix(&prefix) {
            if !pending(tour).is_empty() {
                tours.push(tour.to_owned());
            }
        }
    }
    tours
}

pub fn push(tour: &str, op: Operation) {
    let mut ops = pending(tour);
    ops.push(op);
    set_pending(tour, &ops);
}

/// The last tour seen from the server, kept so the app opens without one.
pub fn cached(tour: &str) -> Option<Tour> {
    let text = storage()?.get_item(&tour_key(tour)).ok().flatten()?;
    Tour::from_json(&text).ok()
}

pub fn cache(tour: &Tour) {
    let (Some(s), Ok(text)) = (storage(), tour.to_json()) else {
        return;
    };
    let _ = s.set_item(&tour_key(tour.id.as_str()), &text);
    // When, as well as what. The line under the tour name says "from server · 3 min ago",
    // and without this the age of a copy stored days ago would have to be guessed at.
    let _ = s.set_item(&stamp_key(tour.id.as_str()), &now_millis().to_string());
}

/// When this device's copy was stored, in milliseconds since the epoch.
pub fn cached_at(tour: &str) -> Option<f64> {
    storage()?
        .get_item(&stamp_key(tour))
        .ok()
        .flatten()?
        .parse()
        .ok()
}

pub fn now_millis() -> f64 {
    js_sys::Date::now()
}

/// The tour list as the server last sent it, so the list opens on what it showed last time
/// instead of on nothing. It carries no spendings and is never edited here - only the
/// tours themselves are - so unlike a tour it needs no queue replayed over it.
pub fn cached_list() -> Option<Vec<Tour>> {
    let text = storage()?.get_item(LIST_KEY).ok().flatten()?;
    let items: Vec<serde_json::Value> = serde_json::from_str(&text).ok()?;
    Some(
        items
            .iter()
            .filter_map(|v| Tour::from_json(&v.to_string()).ok())
            .collect(),
    )
}

pub fn cache_list(tours: &[Tour]) {
    let Some(s) = storage() else { return };
    let items: Vec<serde_json::Value> = tours
        .iter()
        .filter_map(|t| t.to_json().ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    if let Ok(text) = serde_json::to_string(&items) {
        let _ = s.set_item(LIST_KEY, &text);
    }
}

/// Drops the remembered list. Called on the way out: the next person to sign in on this
/// device may hold a different access code, and the list is the one thing here that says
/// which tours exist. A cached tour needs its id to be asked for; a cached list hands the
/// ids over.
pub fn forget_list() {
    if let Some(s) = storage() {
        let _ = s.remove_item(LIST_KEY);
    }
}

/// Everything this device kept about somebody's tours: the cached copies, the queues, the
/// refusals, the dropped edits, the compact lists, the remembered list itself.
///
/// For the way out. A login that ran out keeps all of this on purpose - typing the same code
/// again sends what is waiting - but somebody pressing "log out" is leaving the device, and
/// what they leave behind is the next person's to find. Their own settings (the accent, how
/// often to check) are not theirs to lose and stay.
pub fn forget_everything() {
    let Some(s) = storage() else { return };
    const MINE: &[&str] = &[
        "__tcw_tour_",
        "__tcw_tour_at_",
        "__tcw_queue_",
        "__tcw_refused_",
        "__tcw_lost_",
        "__tcw_compact_",
    ];
    let mut doomed = Vec::new();
    for i in 0..s.length().unwrap_or(0) {
        let Ok(Some(key)) = s.key(i) else { continue };
        if MINE.iter().any(|prefix| key.starts_with(prefix)) {
            doomed.push(key);
        }
    }
    for key in doomed {
        let _ = s.remove_item(&key);
    }
    forget_list();
    changed();
}

/// The tour as the reader should see it: what the server last said, plus everything that
/// has not reached it yet.
pub fn with_pending(tour: &Tour) -> Tour {
    pending(tour.id.as_str())
        .iter()
        .fold(tour.clone(), |acc, op| op.apply(&acc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_core::Cents;

    fn tour() -> Tour {
        let json = include_str!("../../../fixtures/zscph2y.tour.json");
        Tour::from_json(json).expect("fixture")
    }

    fn add(id: &str, amount: i64) -> Operation {
        let t = tour();
        Operation::PutSpending(SpendingDraft {
            id: Some(SpendingId::new(id)),
            description: format!("added {id}"),
            category: "Еда".into(),
            amount: Cents(amount),
            from: t.persons[0].id.clone(),
            everyone: true,
            to: Vec::new(),
            by_weight: true,
            date: "2021-08-14".into(),
            colour: String::new(),
            currency_id: t.currency().id.as_str().to_owned(),
            editing: false,
            in_cents: None,
        })
    }

    /// The point of queuing intentions: they can be carried out on a tour that has moved on.
    #[test]
    fn operations_replay_onto_a_changed_tour() {
        let base = tour();
        let before = base.spendings.len();

        // Somebody else's edit lands first.
        let theirs = add("theirs", 1_500).apply(&base);

        // Ours were queued while that happened, and go on top.
        let ours = [add("mine-1", 100), add("mine-2", 200)];
        let merged = ours.iter().fold(theirs, |acc, op| op.apply(&acc));

        assert_eq!(merged.spendings.len(), before + 3, "nobody's edit was lost");
        for id in ["theirs", "mine-1", "mine-2"] {
            assert!(
                merged.spendings.iter().any(|s| s.id.as_str() == id),
                "{id} survived"
            );
        }
    }

    /// Carrying out the same recorded "add" twice must not leave two of them.
    ///
    /// It can happen: a save that reached the server and whose answer did not, and the queue
    /// is retried. The id is settled when the edit is recorded, so the second run finds the
    /// spending and updates it.
    #[test]
    fn replaying_an_add_twice_adds_one_thing() {
        let base = tour();
        let before = base.spendings.len();
        let op = add("once", 4_200);

        let once = op.apply(&base);
        let twice = op.apply(&once);

        assert_eq!(once.spendings.len(), before + 1);
        assert_eq!(twice.spendings.len(), before + 1, "no duplicate");
        assert_eq!(
            twice
                .spendings
                .iter()
                .filter(|s| s.id.as_str() == "once")
                .count(),
            1
        );
    }

    /// A delete that has already happened is not an error the second time round.
    #[test]
    fn replaying_a_delete_twice_is_harmless() {
        let base = tour();
        let victim = base.spendings[0].id.clone();
        let op = Operation::RemoveSpending(victim.clone());

        let once = op.apply(&base);
        let twice = op.apply(&once);

        assert!(!once.spendings.iter().any(|s| s.id == victim));
        assert_eq!(once.spendings.len(), twice.spendings.len());
    }

    /// The order of the queue is the order the edits were made in.
    #[test]
    fn later_edits_win_over_earlier_ones() {
        let base = tour();
        let mut first = match add("same", 100) {
            Operation::PutSpending(d) => d,
            _ => unreachable!(),
        };
        let applied = Operation::PutSpending(first.clone()).apply(&base);
        first.amount = Cents(900);
        let applied = Operation::PutSpending(first).apply(&applied);

        let found = applied
            .spendings
            .iter()
            .find(|s| s.id.as_str() == "same")
            .expect("still there");
        assert_eq!(found.amount.0, 900);
    }

    /// An edit made while somebody else deleted the expense does not bring it back.
    #[test]
    fn an_edit_of_something_deleted_meanwhile_is_dropped() {
        let base = tour();
        let victim = base.spendings[0].clone();
        let mut edit = SpendingDraft::of(&victim);
        edit.amount = Cents(123);
        let op = Operation::PutSpending(edit);

        // Somebody else deleted it first.
        let theirs = Operation::RemoveSpending(victim.id.clone()).apply(&base);
        assert!(op.lost_on(&theirs).is_some(), "said to be lost");
        let after = op.apply(&theirs);
        assert!(!after.spendings.iter().any(|s| s.id == victim.id), "the delete stood");

        // On a tour that still has it, the edit is an edit.
        assert!(op.lost_on(&base).is_none());
        let edited = op.apply(&base);
        let found = edited.spendings.iter().find(|s| s.id == victim.id).expect("there");
        assert_eq!(found.amount.0, 123);
    }

    /// An add is not an edit: queued twice, or replayed, it still adds.
    #[test]
    fn an_add_is_never_lost() {
        let base = tour();
        let op = add("brand-new", 500);
        assert!(op.lost_on(&base).is_none());
        assert!(op.apply(&base).spendings.iter().any(|s| s.id.as_str() == "brand-new"));
    }
}
