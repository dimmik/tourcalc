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

use crate::edit::{self, CurrencyDraft, PaymentDraft, PersonDraft, SpendingDraft, TourDraft};
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
            Operation::RemoveSpending(_) => "an expense removed".into(),
            Operation::PutPerson(d) => short(&d.name),
            Operation::RemovePerson(_) => "somebody removed".into(),
            Operation::Rename(name) => format!("renamed to “{}”", short(name)),
            Operation::SetCurrency(id) => format!("amounts in {id}"),
            Operation::EditTour(d) => format!("the tour: {}", short(&d.name)),
            Operation::SetCurrencies { .. } => "the currencies".into(),
            Operation::RecordPayment(d) => format!("paid: {}", short(&d.description)),
        }
    }
}

fn short(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "no description".into();
    }
    if s.chars().count() > 24 {
        format!("{}…", s.chars().take(24).collect::<String>())
    } else {
        s.to_owned()
    }
}

fn storage() -> Option<web_sys::Storage> {
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

/// One key for the whole list, not one per tour: it is a screen, not a set of documents.
const LIST_KEY: &str = "__tcw_tourlist";

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
            date: "2021-08-14".into(),
            colour: String::new(),
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
}
