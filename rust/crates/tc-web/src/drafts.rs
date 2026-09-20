//! What somebody was typing when they left a form.
//!
//! The form is a dialog, so looking anything up means closing it: who paid for the taxi
//! last time, what the hotel was called, whether Вася is already on the tour. Until now
//! that threw away everything typed so far, and the second attempt started from nothing.
//!
//! So a form that is closed without saving leaves its draft on the device, and the next
//! time the same kind of form is opened for the same tour it picks up where it was left -
//! saying so, with one button to start blank instead. Saving forgets it: what was typed has
//! become an expense.

use crate::edit::{PersonDraft, SpendingDraft};
use tc_core::Tour;

/// How long a left-behind draft is worth offering. Long enough for a meal and an argument
/// about who pays; short enough that next week's expense does not open with last week's
/// words in it.
const FRESH_FOR: f64 = 2.0 * 60.0 * 60.0 * 1000.0;

fn key(kind: &str, tour: &str) -> String {
    format!("__tcw_draft_{kind}_{tour}")
}

fn keep<T: serde::Serialize>(kind: &str, tour: &str, draft: &T) {
    let Some(s) = crate::queue::storage() else {
        return;
    };
    let Ok(what) = serde_json::to_value(draft) else {
        return;
    };
    let text = serde_json::json!({ "at": crate::queue::now_millis(), "draft": what }).to_string();
    let _ = s.set_item(&key(kind, tour), &text);
}

fn kept<T: serde::de::DeserializeOwned>(kind: &str, tour: &str) -> Option<T> {
    let s = crate::queue::storage()?;
    let text = s.get_item(&key(kind, tour)).ok()??;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let at = value.get("at")?.as_f64()?;
    if crate::queue::now_millis() - at > FRESH_FOR {
        let _ = s.remove_item(&key(kind, tour));
        return None;
    }
    serde_json::from_value(value.get("draft")?.clone()).ok()
}

fn forget(kind: &str, tour: &str) {
    if let Some(s) = crate::queue::storage() {
        let _ = s.remove_item(&key(kind, tour));
    }
}

/// Whether there is anything in this expense worth coming back to. A form somebody opened
/// and closed at once is not a draft, and offering it back would be noise.
pub fn worth_keeping_spending(draft: &SpendingDraft) -> bool {
    !draft.description.trim().is_empty() || draft.amount.0 != 0 || !draft.to.is_empty()
}

pub fn worth_keeping_person(draft: &PersonDraft) -> bool {
    !draft.name.trim().is_empty()
}

pub fn keep_spending(tour: &str, draft: &SpendingDraft) {
    if worth_keeping_spending(draft) {
        keep("spending", tour, draft);
    } else {
        forget("spending", tour);
    }
}

pub fn kept_spending(tour: &str) -> Option<SpendingDraft> {
    kept("spending", tour)
}

pub fn forget_spending(tour: &str) {
    forget("spending", tour);
}

pub fn keep_person(tour: &str, draft: &PersonDraft) {
    if worth_keeping_person(draft) {
        keep("person", tour, draft);
    } else {
        forget("person", tour);
    }
}

pub fn kept_person(tour: &str) -> Option<PersonDraft> {
    kept("person", tour)
}

pub fn forget_person(tour: &str) {
    forget("person", tour);
}

/// Whether this is a form somebody has only just opened, as opposed to one filled in for
/// them: "spend for Вася" arrives with the payer and the person already chosen, and putting
/// last night's taxi in its place would undo that.
fn untouched_spending(draft: &SpendingDraft) -> bool {
    draft.description.trim().is_empty() && draft.amount.0 == 0 && draft.to.is_empty()
}

/// The form a new expense should open with: what was left behind, if there is anything and
/// the form is otherwise blank. Also says whether that is what happened, so the form can.
pub fn carry_spending(tour: &Tour, draft: SpendingDraft) -> (SpendingDraft, bool) {
    if draft.id.is_some() || !untouched_spending(&draft) {
        return (draft, false);
    }
    match kept_spending(tour.id.as_str()) {
        Some(kept) => (kept, true),
        None => (draft, false),
    }
}

/// The same for somebody being added to the tour.
pub fn carry_person(tour: &Tour, draft: PersonDraft) -> (PersonDraft, bool) {
    if draft.id.is_some() || !draft.name.trim().is_empty() {
        return (draft, false);
    }
    match kept_person(tour.id.as_str()) {
        Some(kept) => (kept, true),
        None => (draft, false),
    }
}
