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
    // Somebody removed, or added: everybody who is in one list and not the other. The C#
    // named only the last of the new list, so two companions added in one save read as one
    // - "P 'Эмма' added" for Эмма and Артём. No C# server writes these any more, and one of
    // them the same is still "P 'Эмма' added".
    if old.persons.len() > new.persons.len() {
        let gone = quoted(old.persons.iter().filter(|p| !new.persons.iter().any(|q| q.id == p.id)));
        return Some(format!("P {gone} deleted"));
    }
    if old.persons.len() < new.persons.len() {
        let added = quoted(new.persons.iter().filter(|p| !old.persons.iter().any(|q| q.id == p.id)));
        return Some(format!("P {added} added"));
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
            .rfind(|s| !new_real.iter().any(|t| t.id == s.id))
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

/// The line for what a save did to the menu, or `None` when it did nothing to it.
///
/// Apart from [`describe_change`] because it is apart everywhere else: it goes into the
/// version's line, so that a change of dinner can be found and undone, but it tells nobody -
/// a tick on the shopping list is not news for everybody's phone.
pub fn describe_menu(old: &Tour, new: &Tour) -> Option<String> {
    use tc_core::menu::{Meal, Menu};
    let was = Menu::of(old);
    let is = Menu::of(new);
    if was == is {
        return None;
    }
    let (was, is) = match (was, is) {
        (None, Some(is)) => return Some(if is.on { "Menu on".to_owned() } else { "Menu added".to_owned() }),
        (Some(_), None) => return Some("Menu removed".to_owned()),
        (Some(was), Some(is)) => (was, is),
        (None, None) => return None,
    };
    let mut said: Vec<String> = Vec::new();
    if was.on != is.on {
        said.push(if is.on { "on" } else { "off" }.to_owned());
    }
    if was.days != is.days {
        said.push(format!("days {} -> {}", was.days, is.days));
    }
    if was.start != is.start {
        said.push(format!("starts {}", is.start.as_deref().unwrap_or("-")));
    }
    for day in 1..=was.day_notes.len().max(is.day_notes.len()) as u32 {
        if was.day_note(day) != is.day_note(day) {
            said.push(format!("day {day}: {}", is.day_note(day).unwrap_or("-")));
        }
    }
    // The plan, over the days both have: a changed number of days is said above, and the
    // days it adds are filled in by themselves.
    let meal = |m: Meal| match m {
        Meal::Breakfast => "breakfast",
        Meal::Lunch => "lunch",
        Meal::Dinner => "dinner",
    };
    // Several dishes a meal: "Plov + Salad".
    let dish = |menu: &Menu, day: u32, m: Meal| {
        let names: Vec<&str> = menu.dishes_on(day, m).iter().map(|d| d.name.as_str()).collect();
        if names.is_empty() { "nothing".to_owned() } else { names.join(" + ") }
    };
    let mut plan: Vec<String> = Vec::new();
    for day in 1..=was.days.min(is.days) {
        for m in Meal::ALL {
            let (a, b) = (dish(&was, day, m), dish(&is, day, m));
            if a != b {
                plan.push(format!("{} day {day} {a} -> {b}", meal(m)));
            }
        }
    }
    said.extend(few(plan));
    // The shopping: ticks and who buys.
    let product = |id: &str| is.product(id).or_else(|| was.product(id)).map_or(id.to_owned(), |p| p.name.clone());
    let person = |id: &Option<String>| {
        id.as_deref()
            .and_then(|id| new.persons.iter().find(|p| p.id.as_str() == id))
            .map_or("nobody".to_owned(), |p| p.name.clone())
    };
    let mut bought = Vec::new();
    let mut unbought = Vec::new();
    let mut buyers = Vec::new();
    for p in &is.purchases {
        let before = was.purchase(&p.product).cloned().unwrap_or_default();
        if p.bought && !before.bought {
            bought.push(product(&p.product));
        }
        if !p.bought && before.bought {
            unbought.push(product(&p.product));
        }
        if p.who != before.who {
            buyers.push(format!("{} -> {}", product(&p.product), person(&p.who)));
        }
    }
    if !bought.is_empty() {
        said.push(format!("bought {}", few(bought).join(", ")));
    }
    if !unbought.is_empty() {
        said.push(format!("not bought {}", few(unbought).join(", ")));
    }
    said.extend(few(buyers));
    // The catalogue: what was added, taken out or changed, by name.
    catalogue(&mut said, "dish", &was.dishes, &is.dishes, |d| (d.id.as_str(), d.name.as_str()));
    catalogue(&mut said, "product", &was.products, &is.products, |p| (p.id.as_str(), p.name.as_str()));
    catalogue(&mut said, "place", &was.places, &is.places, |p| (p.id.as_str(), p.name.as_str()));
    if was.daily != is.daily {
        said.push("daily list changed".to_owned());
    }
    if said.is_empty() {
        // Something only the tour's own bookkeeping sees - still worth a version.
        said.push("changed".to_owned());
    }
    Some(format!("Menu: {}", said.join("; ")))
}

/// Three of a list and how many more: a whole day re-planned is not a paragraph.
fn few(mut items: Vec<String>) -> Vec<String> {
    if items.len() > 3 {
        let more = items.len() - 3;
        items.truncate(3);
        items.push(format!("and {more} more"));
    }
    items
}

/// Added, removed and changed items of one kind, paired by id.
fn catalogue<T: PartialEq>(said: &mut Vec<String>, kind: &str, was: &[T], is: &[T], key: impl Fn(&T) -> (&str, &str)) {
    for item in is {
        let (id, name) = key(item);
        match was.iter().find(|w| key(w).0 == id) {
            None => said.push(format!("{kind} added: {name}")),
            Some(w) if w != item => said.push(format!("{kind} changed: {name}")),
            Some(_) => {}
        }
    }
    for item in was {
        let (id, name) = key(item);
        if !is.iter().any(|i| key(i).0 == id) {
            said.push(format!("{kind} removed: {name}"));
        }
    }
}

/// The spendings a person actually entered. Planned payments are the settlement's own and
/// are rewritten on every edit, so counting them would make every save look like a change.
fn real(tour: &Tour) -> Vec<&Spending> {
    tour.spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned)
        .collect()
}

/// "'Артём', 'Эмма'" - each name in the quotes the history's lines use; "'--'" for nobody,
/// which is what the C# wrote when it could not find one.
fn quoted<'a>(people: impl Iterator<Item = &'a tc_core::Person>) -> String {
    let names: Vec<String> = people.map(|p| format!("'{}'", p.name)).collect();
    if names.is_empty() {
        "'--'".to_owned()
    } else {
        names.join(", ")
    }
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
            format!(" ({} {})", amount(s, tour), s.currency.name)
        }
        Some(s) => format!(" ({})", amount(s, tour)),
    }
}

/// An expense's amount as its currency counts it: 44.00 for a currency with cents, whose
/// amounts are hundredths - printed raw, it read "4400 Eur" in every version and notification.
/// Whether it has cents is the tour's say, not the copy inside the expense, which can be stale.
/// Plain digits otherwise, as these lines always had.
fn amount(s: &Spending, tour: &Tour) -> String {
    let n = s.amount.0;
    if !tour.counts_cents(&s.currency.id) {
        return n.to_string();
    }
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", n.unsigned_abs() / 100, n.unsigned_abs() % 100)
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
                was.description,
                amount(was, old),
                amount(is, new)
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
        // By id: the copy of the currency inside an expense also changes when its worth or its
        // cents do, and compared whole it announced "Currency: Eur -> Eur".
        if was.currency.id != is.currency.id {
            out.push_str(&format!(
                " {} ({} {}) Currency: {} -> {}",
                was.description,
                amount(was, old),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A euro with cents, one expense of 44.00 in it, and the copy of the currency inside the
    /// expense as the tour had it.
    fn tour(amount: i64, copy_rate: i32) -> Tour {
        let json = serde_json::json!({
            "Id": "t", "Name": "t", "Persons": [{"GUID": "p", "Name": "P", "Weight": 100}],
            "Currencies": [
                {"_id": "RSD", "Name": "RSD", "CurrencyRate": 1000},
                {"_id": "EUR", "Name": "Eur", "CurrencyRate": 117_500, "WithCents": true}
            ],
            "TourCurrencyId": "RSD",
            "Spendings": [
                {"GUID": "a", "Description": "44", "Type": "Food", "AmountInCents": amount,
                 "FromGuid": "p", "ToAll": true, "ToGuid": [],
                 "Currency": {"_id": "EUR", "Name": "Eur", "CurrencyRate": copy_rate}}
            ]
        });
        Tour::from_json(&json.to_string()).expect("tour")
    }

    #[test]
    fn an_amount_with_cents_is_written_with_its_cents() {
        let said = describe_change(&tour(4400, 117_500), &tour(4445, 117_500)).expect("a change");
        assert!(said.contains("44: 44.00 -> 44.45"), "{said}");
    }

    /// The copy inside the expense is refreshed with the worth; the currency is the same one.
    /// Two companions added in one save are both named; the C# named only the last.
    #[test]
    fn everybody_added_or_removed_at_once_is_named() {
        let mut two_more = tour(4400, 117_500);
        for (id, name) in [("emma", "Эмма"), ("artem", "Артём")] {
            let mut p = two_more.persons[0].clone();
            p.id = tc_core::PersonId::new(id);
            p.name = name.into();
            two_more.persons.push(p);
        }
        let before = tour(4400, 117_500);
        assert_eq!(describe_change(&before, &two_more).as_deref(), Some("P 'Эмма', 'Артём' added"));
        assert_eq!(describe_change(&two_more, &before).as_deref(), Some("P 'Эмма', 'Артём' deleted"));
        let mut one_more = before.clone();
        one_more.persons.push(two_more.persons[1].clone());
        assert_eq!(describe_change(&before, &one_more).as_deref(), Some("P 'Эмма' added"), "one reads as before");
    }

    fn with_menu(t: &Tour, edit: impl FnOnce(&mut tc_core::menu::Menu)) -> Tour {
        use tc_core::menu::{Dish, Ingredient, Meal, Menu, Place, Product};
        let mut menu = Menu::of(t).unwrap_or_else(|| {
            let mut m = Menu {
                on: true,
                days: 2,
                places: vec![Place { id: "m".into(), name: "Market".into() }],
                products: vec![Product { id: "lamb".into(), name: "Lamb".into(), place: "m".into(), ..Product::default() }],
                ..Menu::default()
            };
            for (id, name) in [("plov", "Plov"), ("fish", "Fish")] {
                m.dishes.push(Dish {
                    id: id.into(),
                    name: name.into(),
                    meals: vec![Meal::Dinner],
                    meal: None,
                    ingredients: vec![Ingredient { product: "lamb".into(), amount: 1.0, ..Ingredient::default() }],
                });
            }
            m.fill();
            m
        });
        edit(&mut menu);
        let mut next = t.clone();
        menu.put(&mut next);
        next
    }

    /// A dinner changed and a product ticked are said, by name; the money's line has
    /// nothing to say about either.
    #[test]
    fn a_menu_change_is_said_apart_from_the_money() {
        use tc_core::menu::{Meal, Slot};
        let before = with_menu(&tour(4400, 117_500), |_| {});
        let after = with_menu(&before, |m| {
            m.set(Slot::one(2, Meal::Dinner, Some("plov".into())));
            m.purchase_mut("lamb").bought = true;
        });
        assert_eq!(
            describe_menu(&before, &after).as_deref(),
            Some("Menu: dinner day 2 Fish -> Plov; bought Lamb")
        );
        assert_eq!(describe_change(&before, &after), None, "no line, so no notification");
        assert_eq!(describe_menu(&before, &before), None);
        assert_eq!(describe_menu(&tour(4400, 117_500), &before).as_deref(), Some("Menu on"));
        let renamed = with_menu(&before, |m| m.dishes[0].name = "Pilaf".into());
        assert_eq!(describe_menu(&before, &renamed).as_deref(), Some("Menu: dinner day 1 Plov -> Pilaf; dish changed: Pilaf"));
    }

    #[test]
    fn a_new_worth_is_not_a_change_of_currency() {
        let said = describe_change(&tour(4400, 117_000), &tour(4445, 117_500)).unwrap_or_default();
        assert!(!said.contains("Currency:"), "{said}");
    }
}
