//! Removing somebody from a tour.
//!
//! One implementation for the app, the queue replaying it later, and the text pages. There
//! used to be two copies, and both did the same unkind thing: every expense the person had
//! paid for was deleted with them - money that was really spent, gone from the tour - and an
//! expense that was only for them became an expense for everybody, spread over people who
//! had nothing to do with it. The confirmation said "Delete 'Vasya'?" and nothing else.
//!
//! Now somebody who still carries money is not removed at all. The reader is told which
//! expenses hold them, and changing or deleting those is a decision they make one by one,
//! on screen, where they can see it.

use crate::{Kind, PersonId, Spending, Split, Tour};

/// The expenses that keep somebody in a tour, by description.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Holds {
    /// Paid for by them. Removing them would leave money from nowhere.
    pub paid: Vec<String>,
    /// For them alone. Removing them would leave an expense for nobody.
    pub only_for: Vec<String>,
}

impl Holds {
    pub fn is_empty(&self) -> bool {
        self.paid.is_empty() && self.only_for.is_empty()
    }

    /// Why `name` cannot be removed, and what to do about it - in words for a person.
    pub fn explain(&self, name: &str) -> String {
        let mut why = Vec::new();
        if !self.paid.is_empty() {
            why.push(format!(
                "{name} paid for {} ({})",
                counted(self.paid.len(), "expense", "expenses"),
                named(&self.paid)
            ));
        }
        if !self.only_for.is_empty() {
            why.push(format!(
                "{} only for {name} ({})",
                counted(self.only_for.len(), "expense is", "expenses are"),
                named(&self.only_for)
            ));
        }
        format!(
            "{name} cannot be removed yet: {}. Change who paid or who it is for, or delete \
             those expenses, and then remove {name}.",
            why.join(", and ")
        )
    }
}

fn counted(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "“Taxi”, “Dinner”, “Museum” and 2 more" - the first few by name, so the reader knows
/// where to look without the message becoming the list.
fn named(what: &[String]) -> String {
    const NAMED: usize = 3;
    let names: Vec<String> = what
        .iter()
        .take(NAMED)
        .map(|d| match d.trim() {
            "" => "“no description”".to_owned(),
            d => format!("“{d}”"),
        })
        .collect();
    match what.len().saturating_sub(NAMED) {
        0 => names.join(", "),
        more => format!("{} and {more} more", names.join(", ")),
    }
}

/// Whether an expense is money somebody entered, as opposed to a payment the app proposed
/// and never stores for good.
fn is_money(s: &Spending) -> bool {
    s.kind != Kind::Planned
}

fn recipients(s: &Spending) -> Option<&[PersonId]> {
    match &s.split {
        Split::Everyone => None,
        Split::Equally(to) | Split::ByWeight(to) => Some(to),
    }
}

/// What keeps this person in the tour. Empty when nothing does.
pub fn what_holds(tour: &Tour, id: &PersonId) -> Holds {
    let mut holds = Holds::default();
    for s in tour.spendings.iter().filter(|s| is_money(s)) {
        if &s.from == id {
            holds.paid.push(s.description.clone());
        } else if recipients(s).is_some_and(|to| to.len() == 1 && &to[0] == id) {
            holds.only_for.push(s.description.clone());
        }
    }
    holds
}

/// How many expenses this person shares with others by name - the ones whose split changes
/// when they go, their part spread over whoever else is on it.
pub fn shared_in(tour: &Tour, id: &PersonId) -> usize {
    tour.spendings
        .iter()
        .filter(|s| is_money(s))
        .filter(|s| recipients(s).is_some_and(|to| to.len() > 1 && to.contains(id)))
        .count()
}

/// The tour without this person, or `None` when something still holds them (see
/// [`what_holds`]) - in which case nothing is changed at all.
///
/// Going, they leave nothing pointing at them: nobody's "paid for by" names them, no
/// expense lists them among the people it is for, and the payments the app had proposed -
/// worked out with them in it - are dropped, to be proposed again without them.
pub fn without_person(tour: &Tour, id: &PersonId) -> Option<Tour> {
    if !what_holds(tour, id).is_empty() {
        return None;
    }
    let mut next = tour.clone();
    next.persons.retain(|p| &p.id != id);
    for p in next.persons.iter_mut() {
        if p.parent.as_ref() == Some(id) {
            p.parent = None;
        }
    }
    next.spendings.retain(is_money);
    for s in next.spendings.iter_mut() {
        if let Split::Equally(to) | Split::ByWeight(to) = &mut s.split {
            to.retain(|p| p != id);
        }
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour() -> Tour {
        Tour::from_json(include_str!("../../../fixtures/zscph2y.tour.json")).expect("fixture")
    }

    #[test]
    fn somebody_who_paid_is_not_removed() {
        let t = tour();
        let payer = t
            .spendings
            .iter()
            .find(|s| is_money(s))
            .expect("an expense")
            .from
            .clone();
        let holds = what_holds(&t, &payer);
        assert!(!holds.paid.is_empty());
        assert!(without_person(&t, &payer).is_none(), "nothing is removed");

        let name = &t.person(&payer).unwrap().name;
        let told = holds.explain(name);
        assert!(told.contains("paid for"), "{told}");
        assert!(told.contains(name.as_str()), "{told}");
    }

    #[test]
    fn an_expense_only_for_somebody_keeps_them() {
        let mut t = tour();
        let who = t.persons[1].id.clone();
        let payer = t.persons[0].id.clone();
        // Nobody paid by them, so the only thing holding them is this.
        t.spendings.retain(|s| s.from != who);
        let mut mine = t.spendings[0].clone();
        mine.from = payer;
        mine.description = "Their souvenir".into();
        mine.split = Split::Equally(vec![who.clone()]);
        t.spendings.push(mine);

        let holds = what_holds(&t, &who);
        assert_eq!(holds.only_for, vec!["Their souvenir".to_owned()]);
        assert!(holds.paid.is_empty());
        assert!(without_person(&t, &who).is_none());
    }

    #[test]
    fn somebody_free_goes_and_leaves_nothing_pointing_at_them() {
        let mut t = tour();
        let who = t.persons[1].id.clone();
        let other = t.persons[0].id.clone();
        t.spendings.retain(|s| s.from != who);
        // Shared by name with somebody else, and somebody's parent.
        let mut shared = t.spendings[0].clone();
        shared.from = other.clone();
        shared.split = Split::ByWeight(vec![who.clone(), other.clone()]);
        t.spendings.push(shared);
        t.persons[0].parent = Some(who.clone());
        // Nothing for them alone, either: that would hold them too.
        t.spendings
            .retain(|s| !recipients(s).is_some_and(|to| to == [who.clone()]));
        assert!(what_holds(&t, &who).is_empty());
        assert!(shared_in(&t, &who) >= 1);

        let before = t.spendings.iter().filter(|s| is_money(s)).count();
        let next = without_person(&t, &who).expect("removed");
        assert!(next.person(&who).is_none());
        assert_eq!(next.spendings.len(), before, "no money went with them");
        assert!(next.persons.iter().all(|p| p.parent.as_ref() != Some(&who)));
        assert!(next
            .spendings
            .iter()
            .all(|s| recipients(s).is_none_or(|to| !to.contains(&who))));
    }

    #[test]
    fn the_explanation_names_a_few_and_counts_the_rest() {
        let holds = Holds {
            paid: vec!["A".into(), "B".into(), "C".into(), "D".into(), "E".into()],
            only_for: vec!["F".into()],
        };
        let told = holds.explain("Vasya");
        assert!(told.contains("paid for 5 expenses (“A”, “B”, “C” and 2 more)"), "{told}");
        assert!(told.contains("1 expense is only for Vasya (“F”)"), "{told}");
    }
}
