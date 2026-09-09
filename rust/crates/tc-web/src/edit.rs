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
        }
    }

    pub fn of(spending: &Spending) -> SpendingDraft {
        let (everyone, to) = match &spending.split {
            Split::Everyone => (true, Vec::new()),
            Split::Equally(to) | Split::ByWeight(to) => (false, to.clone()),
        };
        SpendingDraft {
            id: Some(spending.id.clone()),
            description: spending.description.clone(),
            category: spending.category.clone(),
            amount: spending.amount,
            from: spending.from.clone(),
            everyone,
            to,
        }
    }

    /// Why this cannot be saved, if it cannot.
    pub fn problem(&self) -> Option<&'static str> {
        if self.amount.0 == 0 {
            return Some("How much was it?");
        }
        if !self.everyone && self.to.is_empty() {
            return Some("Who was it for?");
        }
        if self.from.as_str().is_empty() {
            return Some("Who paid?");
        }
        None
    }
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
        Split::Equally(draft.to.clone())
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
            // What the form had before "everyone" was switched on is kept, as the stored
            // format does.
            if matches!(split, Split::Everyone) {
                existing.remembered_split = Some(match &existing.split {
                    Split::Everyone => existing
                        .remembered_split
                        .clone()
                        .unwrap_or(Split::Equally(Vec::new())),
                    other => other.clone(),
                });
            }
            existing.split = split;
        }
        // Not there: this is the add. Replaying it again finds the spending and updates it
        // instead of adding a second one.
        None => next.spendings.push(Spending {
            id,
            description: draft.description.clone(),
            category: draft.category.clone(),
            amount: draft.amount,
            currency: tour.currency().clone(),
            from: draft.from.clone(),
            split,
            remembered_split: None,
            kind: Kind::Real,
            extras: Default::default(),
        }),
    }
    next
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
