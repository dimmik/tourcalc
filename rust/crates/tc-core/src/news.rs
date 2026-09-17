//! What changed in a tour, in words for somebody's phone.
//!
//! The push used to carry the line written on the version - "S 'Еда (1200)' from Паша to All
//! added" - which is the right line for a history and a poor one for a notification: it is
//! the C#'s wording, kept word for word so a tour's history reads the same whichever server
//! wrote it, and it was never meant to be read on a lock screen. So the two are separate.
//! The server's `versions` module stays as it is; this says the same thing the way a person
//! would.
//!
//! Only the words differ. Which saves are worth telling anybody about is still decided by
//! the server's `versions::describe_change` - a save that changed nothing it can name tells
//! nobody.
//!
//! Here rather than in the server so that both ends say it the same way: the server puts
//! it in a push, and a tour page that finds somebody else's change puts it on screen. The
//! page compares the copy it had with the one it just fetched, so it can say what happened
//! whichever server - or whichever client - made the change.

use crate::extras;
use crate::{Kind, Person, Spending, SpendingId, Split, Tour};

/// How many edits one notification names before it just counts the rest.
const NAMED: usize = 3;

/// The notification for a save: the tour's name, then what happened to it.
///
/// `None` when nothing here recognises the change; the caller falls back to the version's
/// line rather than tell nobody.
pub fn describe(old: &Tour, new: &Tour) -> Option<String> {
    news(old, new).map(|what| format!("{} — {what}", new.name))
}

fn news(old: &Tour, new: &Tour) -> Option<String> {
    // People joining or leaving. Named by whoever is in one list and not the other, so two
    // at once are both named rather than "the last one" twice.
    let gone: Vec<&Person> = old
        .persons
        .iter()
        .filter(|p| new.person(&p.id).is_none())
        .collect();
    let joined: Vec<&Person> = new
        .persons
        .iter()
        .filter(|p| old.person(&p.id).is_none())
        .collect();
    if !joined.is_empty() || !gone.is_empty() {
        let mut said = Vec::new();
        if !joined.is_empty() {
            said.push(format!("{} joined", names(&joined)));
        }
        if !gone.is_empty() {
            said.push(format!("{} removed", names(&gone)));
        }
        return Some(said.join("; "));
    }

    let was = real(old);
    let is = real(new);
    let added: Vec<&Spending> = is
        .iter()
        .copied()
        .filter(|s| !was.iter().any(|w| w.id == s.id))
        .collect();
    let deleted: Vec<&Spending> = was
        .iter()
        .copied()
        .filter(|s| !is.iter().any(|i| i.id == s.id))
        .collect();
    if let [one] = added.as_slice() {
        return Some(format!(
            "New expense {}: {}, paid by {}, {}",
            quoted(one),
            amount(one, new),
            name_in(new, &one.from),
            for_whom(one, new)
        ));
    }
    if added.len() > 1 {
        return Some(format!(
            "{} new expenses: {}",
            added.len(),
            listed(
                added
                    .iter()
                    .map(|s| format!("{} {}", quoted(s), amount(s, new)))
            )
        ));
    }
    if let [one] = deleted.as_slice() {
        return Some(format!(
            "Expense {} ({}) deleted",
            quoted(one),
            amount(one, old)
        ));
    }
    if deleted.len() > 1 {
        return Some(format!(
            "{} expenses deleted: {}",
            deleted.len(),
            listed(deleted.iter().map(|s| quoted(s)))
        ));
    }

    let archived = extras::bool_of(&new.extras, extras::ARCHIVED);
    if extras::bool_of(&old.extras, extras::ARCHIVED) != archived {
        return Some(
            if archived {
                "Moved to the archive"
            } else {
                "Back from the archive"
            }
            .to_owned(),
        );
    }

    // Nothing added or taken away: what was edited in place.
    let mut edits = Vec::new();
    if old.name != new.name {
        edits.push(format!("renamed from “{}”", old.name));
    }
    let settling = extras::bool_of(&new.extras, extras::FINALIZING);
    if extras::bool_of(&old.extras, extras::FINALIZING) != settling {
        edits.push(
            if settling {
                "settling up started"
            } else {
                "settling up stopped"
            }
            .to_owned(),
        );
    }
    people_edits(old, new, &mut edits);
    spending_edits(&was, &is, old, new, &mut edits);

    if edits.is_empty() {
        return None;
    }
    let first = listed(edits.into_iter());
    // A sentence, not a fragment: the first letter up.
    let mut chars = first.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
}

fn people_edits(old: &Tour, new: &Tour, edits: &mut Vec<String>) {
    for was in &old.persons {
        let Some(is) = new.person(&was.id) else {
            continue;
        };
        if was.name != is.name {
            edits.push(format!("{} is now {}", was.name, is.name));
        }
        if was.weight != is.weight {
            edits.push(format!(
                "{}’s share {} → {}",
                is.name, was.weight, is.weight
            ));
        }
        if was.parent != is.parent {
            edits.push(match &is.parent {
                Some(p) => format!("{} is paid for by {}", is.name, name_in(new, p)),
                None => format!("{} pays for themselves", is.name),
            });
        }
    }
}

fn spending_edits(
    was_all: &[&Spending],
    is_all: &[&Spending],
    old: &Tour,
    new: &Tour,
    edits: &mut Vec<String>,
) {
    for was in was_all {
        let Some(is) = is_all.iter().find(|s| s.id == was.id) else {
            continue;
        };
        if was.description != is.description {
            edits.push(format!("{} renamed to {}", quoted(was), quoted(is)));
        }
        if was.amount != is.amount || was.currency.name != is.currency.name {
            edits.push(format!(
                "{} {} → {}",
                quoted(is),
                amount(was, old),
                amount(is, new)
            ));
        }
        if was.from != is.from {
            edits.push(format!(
                "{} now paid by {}",
                quoted(is),
                name_in(new, &is.from)
            ));
        }
        if !same_split(&was.split, &is.split) {
            edits.push(format!("{} now {}", quoted(is), for_whom(is, new)));
        }
        if was.category != is.category {
            edits.push(if is.category.trim().is_empty() {
                format!("{} has no category now", quoted(is))
            } else {
                format!("{} moved to {}", quoted(is), is.category)
            });
        }
    }
}

/// The expenses a reader should have pointed out after this change: the new ones, and the
/// ones edited in place. Deleted ones are not here - there is no row left to point at.
pub fn touched_spendings(old: &Tour, new: &Tour) -> Vec<SpendingId> {
    let was = real(old);
    real(new)
        .into_iter()
        .filter(|is| match was.iter().find(|w| w.id == is.id) {
            None => true,
            Some(w) => {
                w.description != is.description
                    || w.amount != is.amount
                    || w.currency.name != is.currency.name
                    || w.from != is.from
                    || !same_split(&w.split, &is.split)
                    || w.category != is.category
            }
        })
        .map(|s| s.id.clone())
        .collect()
}

/// What changed, without the tour's name in front - for a screen that already shows it.
pub fn what_changed(old: &Tour, new: &Tour) -> Option<String> {
    news(old, new)
}

/// The first few, then how many more: a notification is a line, not a report.
fn listed(items: impl Iterator<Item = String>) -> String {
    let all: Vec<String> = items.collect();
    if all.len() <= NAMED {
        return all.join("; ");
    }
    format!(
        "{}; and {} more",
        all[..NAMED].join("; "),
        all.len() - NAMED
    )
}

fn names(people: &[&Person]) -> String {
    people
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn quoted(s: &Spending) -> String {
    let what = s.description.trim();
    if what.is_empty() {
        "an expense".to_owned()
    } else {
        format!("«{what}»")
    }
}

/// The amount the way the app prints it: grouped digits, and the currency named only when
/// the tour has more than one - on a single-currency tour the name is noise.
fn amount(s: &Spending, tour: &Tour) -> String {
    if tour.currencies.len() > 1 {
        format!("{} {}", s.amount, s.currency.name)
    } else {
        s.amount.to_string()
    }
}

/// The same people, the same way - in whatever order the form happened to list them.
fn same_split(a: &Split, b: &Split) -> bool {
    fn sorted(to: &[crate::PersonId]) -> Vec<&str> {
        let mut ids: Vec<&str> = to.iter().map(|p| p.as_str()).collect();
        ids.sort_unstable();
        ids
    }
    match (a, b) {
        (Split::Everyone, Split::Everyone) => true,
        (Split::Equally(x), Split::Equally(y)) | (Split::ByWeight(x), Split::ByWeight(y)) => {
            sorted(x) == sorted(y)
        }
        _ => false,
    }
}

fn for_whom(s: &Spending, tour: &Tour) -> String {
    match &s.split {
        Split::Everyone => "for everyone".to_owned(),
        Split::Equally(to) | Split::ByWeight(to) => match to.as_slice() {
            [] => "for nobody".to_owned(),
            [one] => format!("for {}", name_in(tour, one)),
            many => format!("for {} people", many.len()),
        },
    }
}

fn name_in(tour: &Tour, id: &crate::PersonId) -> String {
    tour.person(id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "somebody".to_owned())
}

/// What people entered. Planned payments are the settlement's and are rewritten on every
/// save, so they would make every save look like news.
fn real(tour: &Tour) -> Vec<&Spending> {
    tour.spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour() -> Tour {
        Tour::from_json(include_str!("../../../fixtures/zscph2y.tour.json")).expect("fixture")
    }

    /// A tour with one currency, so amounts are printed bare.
    fn single() -> Tour {
        let mut t = tour();
        t.currencies.truncate(1);
        t
    }

    fn first_real(t: &mut Tour) -> &mut Spending {
        t.spendings
            .iter_mut()
            .find(|s| s.kind != Kind::Planned)
            .expect("a spending")
    }

    #[test]
    fn a_new_expense_says_what_how_much_who_paid_and_for_whom() {
        let old = single();
        let mut new = old.clone();
        let mut s = old
            .spendings
            .iter()
            .find(|s| s.kind != Kind::Planned)
            .unwrap()
            .clone();
        s.id = crate::SpendingId::new("brand-new".to_owned());
        s.description = "Еда".into();
        s.amount = crate::Cents(1200);
        s.split = Split::Everyone;
        let payer = name_in(&new, &s.from);
        new.spendings.push(s);

        assert_eq!(
            describe(&old, &new).unwrap(),
            format!(
                "{} — New expense «Еда»: 1\u{202f}200, paid by {payer}, for everyone",
                new.name
            )
        );
    }

    #[test]
    fn a_deleted_expense_is_named() {
        let old = single();
        let mut new = old.clone();
        let gone = first_real(&mut new).clone();
        new.spendings.retain(|s| s.id != gone.id);
        let said = news(&old, &new).unwrap();
        assert!(said.starts_with("Expense «"), "{said}");
        assert!(said.ends_with(") deleted"), "{said}");
    }

    #[test]
    fn edits_in_place_are_listed_and_counted_past_three() {
        let old = single();
        let mut new = old.clone();
        new.name = "Урал".into();
        let s = first_real(&mut new);
        s.description = "Такси".into();
        s.amount = crate::Cents(s.amount.0 + 100);
        s.category = String::new();
        s.split = Split::Everyone;
        let said = news(&old, &new).unwrap();
        assert!(said.starts_with("Renamed from “"), "{said}");
        assert!(said.contains("renamed to «Такси»"), "{said}");
        assert!(
            said.ends_with("more"),
            "more than three are counted: {said}"
        );
    }

    #[test]
    fn people_joining_and_leaving_are_named() {
        let old = tour();
        let mut new = old.clone();
        let gone = new.persons.remove(0);
        let said = news(&old, &new).unwrap();
        assert_eq!(said, format!("{} removed", gone.name));
    }

    #[test]
    fn the_currency_is_named_when_there_is_more_than_one() {
        let mut old = tour();
        if old.currencies.len() < 2 {
            let mut other = old.currencies[0].clone();
            other.name = "EUR".into();
            old.currencies.push(other);
        }
        let mut new = old.clone();
        let s = first_real(&mut new);
        s.amount = crate::Cents(5);
        let currency = s.currency.name.clone();
        let said = news(&old, &new).unwrap();
        assert!(said.ends_with(&format!("→ 5 {currency}")), "{said}");
    }

    #[test]
    fn settling_up_and_the_archive_are_said_plainly() {
        let old = tour();
        let mut new = old.clone();
        extras::set(&mut new.extras, extras::ARCHIVED, true.into());
        assert_eq!(news(&old, &new).unwrap(), "Moved to the archive");

        let mut new = old.clone();
        let now = !extras::bool_of(&old.extras, extras::FINALIZING);
        extras::set(&mut new.extras, extras::FINALIZING, now.into());
        let said = news(&old, &new).unwrap();
        assert!(said.starts_with("Settling up "), "{said}");
    }

    #[test]
    fn the_same_people_in_another_order_is_not_a_change() {
        let mut old = single();
        let people: Vec<crate::PersonId> =
            old.persons.iter().take(3).map(|p| p.id.clone()).collect();
        first_real(&mut old).split = Split::Equally(people.clone());
        let mut new = old.clone();
        first_real(&mut new).split = Split::Equally(people.into_iter().rev().collect());
        assert_eq!(news(&old, &new), None);
    }

    #[test]
    fn the_rows_to_point_at_are_the_new_and_the_edited() {
        let old = single();
        let mut new = old.clone();
        let edited = first_real(&mut new);
        edited.amount = crate::Cents(edited.amount.0 + 1);
        let edited = edited.id.clone();
        let mut added = old
            .spendings
            .iter()
            .find(|s| s.kind != Kind::Planned)
            .unwrap()
            .clone();
        added.id = crate::SpendingId::new("brand-new".to_owned());
        new.spendings.push(added);
        assert_eq!(
            touched_spendings(&old, &new),
            vec![edited, crate::SpendingId::new("brand-new".to_owned())]
        );
        assert!(touched_spendings(&old, &old).is_empty());
    }

    #[test]
    fn a_save_that_changed_nothing_is_not_news() {
        assert_eq!(news(&tour(), &tour()), None);
    }
}
