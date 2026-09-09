//! What changed between two states of a tour, in words.
//!
//! Every save keeps the state it replaced, and each kept copy carries a line saying what the
//! save did: "P 'Вася' added", "Spending: такси -> такси в аэропорт". The line is not
//! decoration - it is the whole of what makes a list of versions usable, because "14:32,
//! 14:35, 14:41" tells nobody which one to restore.
//!
//! It doubles as the decision whether to keep a version at all: a save that changed nothing
//! the description can name is a save that leaves no trace. The C# works the same way and
//! this is ported from it, wording included, so that a tour's history reads the same
//! whichever server wrote it.

use crate::fields;
use tc_core::{Kind, Person, Spending, Tour};

/// The line for a save, or `None` when nothing worth recording changed.
pub fn describe_change(old: &Tour, new: &Tour) -> Option<String> {
    // Somebody was removed: name whoever is in the old list and not the new one.
    if old.persons.len() > new.persons.len() {
        let gone = old
            .persons
            .iter()
            .filter(|p| !new.persons.iter().any(|q| q.id == p.id))
            .next_back();
        return Some(format!("P '{}' deleted", gone.map_or("--", |p| &p.name)));
    }
    // Somebody was added: the C# names the last of the new list, which is where an addition
    // lands.
    if old.persons.len() < new.persons.len() {
        let added = new.persons.last();
        return Some(format!("P '{}' added", added.map_or("--", |p| &p.name)));
    }

    let old_real: Vec<&Spending> = real(old);
    let new_real: Vec<&Spending> = real(new);

    if old_real.len() < new_real.len() {
        let s = new_real.last().copied();
        return Some(format!(
            "S '{}{}' from {} to {} added",
            s.map_or("--", |s| s.description.as_str()),
            money(s, new),
            s.map_or("na".to_owned(), |s| name_in(new, &s.from)),
            s.map_or("some", |s| if s.split == tc_core::Split::Everyone {
                "All"
            } else {
                "some"
            })
        ));
    }
    if old_real.len() > new_real.len() {
        let gone = old_real
            .iter()
            .filter(|s| !new_real.iter().any(|t| t.id == s.id))
            .next_back()
            .copied();
        return Some(format!(
            "S '{}{}' deleted",
            gone.map_or("--", |s| s.description.as_str()),
            money(gone, old)
        ));
    }

    if fields::bool_of(old, fields::ARCHIVED) != fields::bool_of(new, fields::ARCHIVED) {
        return Some(if fields::bool_of(new, fields::ARCHIVED) {
            "Moved to archive".to_owned()
        } else {
            "Restored from archive".to_owned()
        });
    }

    // Same number of everything: look for what was edited in place.
    let mut changed = String::new();
    describe_people(old, new, &mut changed);
    describe_spendings(old, new, &mut changed);
    if old.name != new.name {
        changed += &format!("Tour Name {} -> {}", old.name, new.name);
    }
    let was_finalizing = fields::bool_of(old, fields::FINALIZING);
    let is_finalizing = fields::bool_of(new, fields::FINALIZING);
    if was_finalizing != is_finalizing {
        changed += &format!("Tour Finalizing flag: {was_finalizing} -> {is_finalizing}");
    }

    if changed.trim().is_empty() {
        return None;
    }
    Some(format!("Changed: {changed}"))
}

/// The spendings a person actually entered. Planned payments are the settlement's own and
/// are rewritten on every edit, so counting them would make every save look like a change.
fn real(tour: &Tour) -> Vec<&Spending> {
    tour.spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned)
        .collect()
}

fn name_in(tour: &Tour, id: &tc_core::PersonId) -> String {
    tour.person(id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "na".to_owned())
}

/// The amount in brackets, with the currency only when the tour has more than one - the C#
/// makes the same distinction, and on a single-currency tour the name would be noise.
fn money(s: Option<&Spending>, tour: &Tour) -> String {
    match s {
        None => " (0)".to_owned(),
        Some(s) if tour.currencies.len() > 1 => {
            format!(" ({} {})", s.amount.0, s.currency.name)
        }
        Some(s) => format!(" ({})", s.amount.0),
    }
}

/// People edited in place: the two lists are paired by id, so a rename is seen as a rename
/// rather than as one person leaving and another arriving.
fn describe_people(old: &Tour, new: &Tour, out: &mut String) {
    for (was, is) in paired(&old.persons, &new.persons, |p: &Person| p.id.as_str()) {
        if was.name != is.name {
            out.push_str(&format!("Person: {} -> {}; ", was.name, is.name));
        }
        if was.weight != is.weight {
            out.push_str(&format!(
                "{} Weight: {} -> {}; ",
                was.name, was.weight, is.weight
            ));
        }
        if was.parent != is.parent {
            let before = was
                .parent
                .as_ref()
                .map_or("None".to_owned(), |p| name_in(old, p));
            let after = is
                .parent
                .as_ref()
                .map_or("None".to_owned(), |p| name_in(new, p));
            out.push_str(&format!("{} Parent: {before} -> {after}; ", was.name));
        }
    }
}

fn describe_spendings(old: &Tour, new: &Tour, out: &mut String) {
    let was_all = real(old);
    let is_all = real(new);
    for (was, is) in paired(&was_all, &is_all, |s: &&Spending| s.id.as_str()) {
        if was.description != is.description {
            out.push_str(&format!(
                "Spending: {} -> {}; ",
                was.description, is.description
            ));
        }
        if was.amount != is.amount {
            out.push_str(&format!(
                "{}: {} -> {}; ",
                was.description, was.amount.0, is.amount.0
            ));
        }
        let was_all_of_them = was.split == tc_core::Split::Everyone;
        let is_all_of_them = is.split == tc_core::Split::Everyone;
        if was_all_of_them != is_all_of_them {
            out.push_str(&format!(
                "{} toAll: {was_all_of_them} -> {is_all_of_them}; ",
                was.description
            ));
        }
        if was.from != is.from {
            out.push_str(&format!(
                "{} From: {} -> {}; ",
                was.description,
                name_in(old, &was.from),
                name_in(new, &is.from)
            ));
        }
        if recipients(was) != recipients(is) {
            out.push_str(&format!(
                "{} To Count: {} -> {}; ",
                was.description,
                recipients(was),
                recipients(is)
            ));
        }
        if was.category != is.category {
            out.push_str(&format!(
                "{} Type: {} -> {}; ",
                was.description, was.category, is.category
            ));
        }
        if was.currency != is.currency {
            out.push_str(&format!(
                " {} ({} {}) Currency: {} -> {}",
                was.description,
                was.amount.0,
                was.currency.name,
                was.currency.name,
                is.currency.name
            ));
        }
    }
}

fn recipients(s: &Spending) -> usize {
    match &s.split {
        tc_core::Split::Everyone => 0,
        tc_core::Split::Equally(to) | tc_core::Split::ByWeight(to) => to.len(),
    }
}

/// Items present in both lists, paired by id.
///
/// The C# sorts both by id and zips them, which pairs the wrong things the moment one list
/// has an item the other has not. Pairing by id is what that code was reaching for, and it
/// cannot mislabel a change as somebody else's.
fn paired<'a, T, F>(old: &'a [T], new: &'a [T], id: F) -> Vec<(&'a T, &'a T)>
where
    F: Fn(&T) -> &str,
{
    old.iter()
        .filter_map(|was| new.iter().find(|is| id(is) == id(was)).map(|is| (was, is)))
        .collect()
}
