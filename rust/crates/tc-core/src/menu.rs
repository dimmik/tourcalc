//! Food for a trip: what is cooked on which day, and what that means to buy.
//!
//! A tour is usually a group away together, cooking. The menu is a small catalogue - places
//! to buy at, products, dishes made of them - and a plan: for each day, which dish is
//! breakfast, lunch and dinner. The shopping list is worked out from the plan and the
//! people, never stored: add a child or change a dinner and the list follows.
//!
//! It lives inside the tour, under `Menu` - one more field a server or client that does not
//! know it carries along untouched (see [`crate::Extras`]). So it is synced, queued offline,
//! shared by the tour's code and copied with the tour like everything else, and nothing
//! about the money reads it.
//!
//! **Portions are weights.** The weight that divides the money divides the food: somebody
//! at 100 eats one portion, a child at 35 a third of one. Two kinds of product are counted
//! by heads instead - what only the full weights have (wine: nobody pours a third of a glass
//! for a child) and what only everybody else has (juice). "Full" is the weight most people
//! in the tour have ([`common_weight`]), as the "×100" on the People tab.

use crate::domain::Extras;
use crate::Tour;
use serde::{Deserialize, Serialize};

/// Where in the tour's extras the menu is kept.
pub const MENU: &str = "Menu";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Meal {
    Breakfast,
    Lunch,
    Dinner,
}

impl Meal {
    pub const ALL: [Meal; 3] = [Meal::Breakfast, Meal::Lunch, Meal::Dinner];
}

/// Who a product is bought for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Eaters {
    /// Everybody, by weight: a child at 35 counts as 0.35 of a portion.
    #[default]
    Everyone,
    /// One each for everybody at the full weight, nothing for the others. Wine.
    FullWeight,
    /// One each for everybody below the full weight. Juice.
    Others,
}

/// What a product's amounts are counted in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    #[default]
    Gram,
    Millilitre,
    Piece,
}

/// Somewhere things are bought: a market, a supermarket, a delivery.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Place {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub unit: Unit,
    /// The [`Place`] it is usually bought at.
    pub place: String,
    pub eaters: Eaters,
    /// A unit of the group's own, as text - "btl" for wine, "can" for beer - counted like
    /// pieces: amounts per portion may be fractions, the shopping is whole ones. `unit` is
    /// then `Piece`, so that a reader that does not know this field still counts it right.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub own_unit: Option<String>,
    /// What kind of thing it is - "Drinks", "Spices" - as the group names it: to group and
    /// sort the shopping by. Free text, offered from what the tour already has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// So much of a product for one portion - or, for a product counted by heads, one person.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Ingredient {
    pub product: String,
    pub amount: f64,
    /// On the daily list: which days it is bought for. Nothing in a dish - written only when
    /// it is not the default, so a dish's ingredients stay two fields.
    #[serde(skip_serializing_if = "When::is_every_day")]
    pub when: When,
}

/// Which days a daily item is bought for - counted from the plan, so that arriving on
/// Friday for dinner and leaving on Sunday after breakfast is two evenings of wine and three
/// days of water, with nothing to keep in step by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum When {
    /// Every day something is cooked on.
    #[default]
    EveryDay,
    /// Every day with this meal planned.
    With(Meal),
}

impl When {
    pub fn is_every_day(&self) -> bool {
        *self == When::EveryDay
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Dish {
    pub id: String,
    pub name: String,
    /// The meals it can be - breakfast, lunch, dinner, one or several: an omelette is
    /// breakfast, sandwiches can be breakfast or lunch.
    pub meals: Vec<Meal>,
    /// A menu written before a dish could be more than one meal says it here; [`Menu::of`]
    /// folds it into `meals`, and it is never written back.
    #[serde(skip_serializing)]
    pub meal: Option<Meal>,
    pub ingredients: Vec<Ingredient>,
}

/// One meal of one day. `dish: None` is "nothing cooked" - eating out, say - which is not
/// the same as a day nobody has planned yet: that one has no slot at all, and is filled in
/// when the days are.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Slot {
    /// From 1.
    pub day: u32,
    pub meal: Meal,
    #[serde(default)]
    pub dish: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Menu {
    /// Whether the tour shows its menu. Switching it off keeps the plan, so switching it
    /// back on finds it as it was.
    pub on: bool,
    pub days: u32,
    /// The first day's date, "2026-11-13", if anybody gave it: the days are then called by
    /// their dates. Only a label - nothing is counted from it.
    pub start: Option<String>,
    pub places: Vec<Place>,
    pub products: Vec<Product>,
    pub dishes: Vec<Dish>,
    /// Bought every day whatever is cooked: vegetables, bread, water, wine.
    pub daily: Vec<Ingredient>,
    pub plan: Vec<Slot>,
    pub purchases: Vec<Purchase>,
}

/// One product's shopping: who buys and pays for it, whether it is bought, and the expense
/// it was recorded as. Made the first time any of that is said, so a product nobody has
/// touched has none.
///
/// Per product, not per place: the market is often two people's - one takes the meat, the
/// other the vegetables. A place's "everybody: Dima" is only all its products at once.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Purchase {
    pub product: String,
    /// Who buys it and pays for it, if anybody has taken it.
    pub who: Option<String>,
    pub bought: bool,
    /// The expense it was recorded as - one receipt can be several products', so several
    /// can point at one expense. Only a pointer: whether the expense is still there is the
    /// tour's say, and deleted, the product is "not recorded" again.
    pub spending: Option<String>,
}

/// So much of one product for the whole trip.
#[derive(Clone, Debug, PartialEq)]
pub struct Need<'a> {
    pub product: &'a Product,
    pub amount: f64,
}

/// How many are eating, in the three ways products are counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eating {
    /// Everybody's weight over 100.
    pub portions: f64,
    /// Heads at the full weight or above.
    pub full: f64,
    /// Heads below it.
    pub others: f64,
    /// The full weight itself.
    pub full_weight: i32,
}

impl Eating {
    pub fn of(tour: &Tour) -> Eating {
        let full_weight = common_weight(tour);
        let mut e = Eating { portions: 0.0, full: 0.0, others: 0.0, full_weight };
        for p in &tour.persons {
            e.portions += f64::from(p.weight.max(0)) / 100.0;
            if p.weight >= full_weight {
                e.full += 1.0;
            } else {
                e.others += 1.0;
            }
        }
        e
    }

    pub fn times(&self, eaters: Eaters) -> f64 {
        match eaters {
            Eaters::Everyone => self.portions,
            Eaters::FullWeight => self.full,
            Eaters::Others => self.others,
        }
    }
}

/// The weight almost everybody shares.
///
/// On a tie - two at 100 and two at 50 - the full share, 100, if it is one of them, and
/// otherwise the heavier. `max_by_key` alone took whichever came last, so the same tour
/// marked "Anna ×100, Boris ×100" or "Vera ×50, Gosha ×50" depending on the order the
/// people were added in, and the first made the full shares look like the odd ones.
pub fn common_weight(tour: &Tour) -> i32 {
    let mut counts: Vec<(i32, usize)> = Vec::new();
    for p in &tour.persons {
        match counts.iter_mut().find(|(w, _)| *w == p.weight) {
            Some((_, n)) => *n += 1,
            None => counts.push((p.weight, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(w, n)| (n, w == 100, w))
        .map(|(w, _)| w)
        .unwrap_or(100)
}

impl Menu {
    /// The tour's menu, if it has ever had one. Unreadable counts as none: a menu written by
    /// some later version this one cannot follow is not worth refusing the tour over.
    pub fn of(tour: &Tour) -> Option<Menu> {
        let value = value_of(&tour.extras)?;
        let mut menu: Menu = serde_json::from_value(value.clone()).ok()?;
        for dish in &mut menu.dishes {
            if let Some(meal) = dish.meal.take() {
                if !dish.meals.contains(&meal) {
                    dish.meals.push(meal);
                }
            }
        }
        Some(menu)
    }

    /// The tour's menu if it is switched on.
    pub fn shown(tour: &Tour) -> Option<Menu> {
        Menu::of(tour).filter(|m| m.on)
    }

    /// Writes this menu into the tour.
    pub fn put(&self, tour: &mut Tour) {
        if let Ok(value) = serde_json::to_value(self) {
            crate::extras::set(&mut tour.extras, MENU, value);
        }
    }

    /// What a template keeps of this menu: the catalogue - places, products, dishes, the daily
    /// list. Not the plan, the days or what was bought: those are one trip's, and the next
    /// trip is another length with other people buying.
    pub fn as_template(&self) -> Menu {
        Menu {
            on: true,
            places: self.places.clone(),
            products: self.products.clone(),
            dishes: self.dishes.clone(),
            daily: self.daily.clone(),
            ..Menu::default()
        }
    }

    /// Takes a template's catalogue in place of this one's.
    ///
    /// The plan stays where its dishes are still there - a template saved from this group's
    /// last trip mostly has the same ones - and a meal whose dish the template does not have
    /// goes back to unplanned. What is bought stays for the products the template has.
    pub fn take_catalogue(&mut self, template: &Menu) {
        self.places = template.places.clone();
        self.products = template.products.clone();
        self.dishes = template.dishes.clone();
        self.daily = template.daily.clone();
        let dishes = &self.dishes;
        self.plan.retain(|s| s.dish.as_ref().is_none_or(|id| dishes.iter().any(|d| &d.id == id)));
        let products = &self.products;
        self.purchases.retain(|p| products.iter().any(|x| x.id == p.product));
    }

    pub fn product(&self, id: &str) -> Option<&Product> {
        self.products.iter().find(|p| p.id == id)
    }

    pub fn dish(&self, id: &str) -> Option<&Dish> {
        self.dishes.iter().find(|d| d.id == id)
    }

    pub fn dishes_for(&self, meal: Meal) -> impl Iterator<Item = &Dish> {
        self.dishes.iter().filter(move |d| d.meals.contains(&meal))
    }

    pub fn slot(&self, day: u32, meal: Meal) -> Option<&Slot> {
        self.plan.iter().find(|s| s.day == day && s.meal == meal)
    }

    /// What is cooked for this meal of this day.
    pub fn dish_on(&self, day: u32, meal: Meal) -> Option<&Dish> {
        self.slot(day, meal)
            .and_then(|s| s.dish.as_deref())
            .and_then(|id| self.dish(id))
    }

    /// Puts a dish, or nothing, on one meal of one day.
    pub fn set(&mut self, slot: Slot) {
        match self.plan.iter_mut().find(|s| s.day == slot.day && s.meal == slot.meal) {
            Some(s) => s.dish = slot.dish,
            None => self.plan.push(slot),
        }
    }

    /// How many days are planned. Days taken off keep their plan, so that 5 → 3 → 5 is
    /// where it started; new days are filled in ([`Menu::fill`]).
    ///
    /// The departure moves with the last day: leaving after breakfast on day 3 of 3, a
    /// fourth day makes day 3 a whole day and day 4 the one left after breakfast - rather
    /// than a day in the middle of the trip with no lunch or dinner.
    pub fn set_days(&mut self, days: u32) {
        let leaving = self.departure();
        let was_last = self.days;
        self.days = days.max(1);
        if self.days != was_last && leaving != Meal::Dinner {
            let index = |m: Meal| Meal::ALL.iter().position(|x| *x == m).unwrap_or(0);
            self.plan.retain(|s| !(s.day == was_last && s.dish.is_none() && index(s.meal) > index(leaving)));
            self.fill();
            self.leave(leaving);
        } else {
            self.fill();
        }
    }

    /// Gives every planned day that has no slot for a meal the next dish for that meal in
    /// turn, so that five dinners are five different dinners while there are five to choose
    /// from. A slot somebody emptied stays empty.
    pub fn fill(&mut self) {
        for meal in Meal::ALL {
            // A dish with nothing in it - "Restaurant" - is chosen, never taken in turn: it
            // would make every third dinner one eaten out.
            let choice: Vec<String> = self
                .dishes_for(meal)
                .filter(|d| !d.ingredients.is_empty())
                .map(|d| d.id.clone())
                .collect();
            if choice.is_empty() {
                continue;
            }
            for day in 1..=self.days {
                if self.slot(day, meal).is_none() {
                    let dish = choice[(day as usize - 1) % choice.len()].clone();
                    self.plan.push(Slot { day, meal, dish: Some(dish) });
                }
            }
        }
        self.plan.sort_by_key(|s| (s.day, Meal::ALL.iter().position(|m| *m == s.meal)));
    }

    /// Adds a dish, or replaces the one with its id.
    pub fn put_dish(&mut self, dish: Dish) {
        match self.dishes.iter_mut().find(|d| d.id == dish.id) {
            Some(d) => *d = dish,
            None => self.dishes.push(dish),
        }
    }

    /// Takes a dish out of the catalogue, and out of the plan: the meals it was on are
    /// "nothing" until somebody picks another, rather than quietly something else.
    pub fn remove_dish(&mut self, id: &str) {
        self.dishes.retain(|d| d.id != id);
        for slot in &mut self.plan {
            if slot.dish.as_deref() == Some(id) {
                slot.dish = None;
            }
        }
    }

    /// Adds a product, or replaces the one with its id.
    pub fn put_product(&mut self, product: Product) {
        match self.products.iter_mut().find(|p| p.id == product.id) {
            Some(p) => *p = product,
            None => self.products.push(product),
        }
    }

    /// Whether any dish or the daily list has it - such a product is not to be removed.
    pub fn uses(&self, product: &str) -> bool {
        self.daily.iter().any(|i| i.product == product)
            || self.dishes.iter().any(|d| d.ingredients.iter().any(|i| i.product == product))
    }

    /// Adds a place, or renames the one with its id.
    pub fn put_place(&mut self, place: Place) {
        match self.places.iter_mut().find(|p| p.id == place.id) {
            Some(p) => *p = place,
            None => self.places.push(place),
        }
    }

    /// Whether any product is bought there - such a place is not to be removed.
    pub fn place_used(&self, id: &str) -> bool {
        self.products.iter().any(|p| p.place == id)
    }

    /// Takes a place nothing is bought at out of the list; a used one stays.
    pub fn remove_place(&mut self, id: &str) {
        if !self.place_used(id) {
            self.places.retain(|p| p.id != id);
        }
    }

    /// The categories the products have, in the catalogue's order, each once.
    pub fn categories(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for c in self.products.iter().filter_map(|p| p.category.as_deref()).map(str::trim) {
            if !c.is_empty() && !seen.iter().any(|s| s == c) {
                seen.push(c.to_owned());
            }
        }
        seen
    }

    /// Takes an unused product out of the catalogue; a used one stays.
    pub fn remove_product(&mut self, id: &str) {
        if !self.uses(id) {
            self.products.retain(|p| p.id != id);
            self.purchases.retain(|p| p.product != id);
        }
    }

    pub fn purchase(&self, product: &str) -> Option<&Purchase> {
        self.purchases.iter().find(|p| p.product == product)
    }

    /// The product's purchase, made if it is not there yet.
    pub fn purchase_mut(&mut self, product: &str) -> &mut Purchase {
        match self.purchases.iter().position(|p| p.product == product) {
            Some(i) => &mut self.purchases[i],
            None => {
                self.purchases.push(Purchase { product: product.to_owned(), ..Purchase::default() });
                self.purchases.last_mut().expect("just pushed")
            }
        }
    }

    /// The products bought at a place, in the catalogue's order.
    pub fn products_at<'a>(&'a self, place: &'a str) -> impl Iterator<Item = &'a Product> + 'a {
        self.products.iter().filter(move |p| p.place == place)
    }

    /// Why a product is on the list: the planned dishes it goes into, with how many times
    /// each is cooked, and when it is bought besides, if it is on the daily list.
    pub fn sources(&self, product: &str) -> (Vec<(&Dish, u32)>, Option<When>) {
        let mut dishes: Vec<(&Dish, u32)> = Vec::new();
        for day in 1..=self.days {
            for meal in Meal::ALL {
                let Some(dish) = self.dish_on(day, meal) else { continue };
                if !dish.ingredients.iter().any(|i| i.product == product) {
                    continue;
                }
                match dishes.iter_mut().find(|(d, _)| d.id == dish.id) {
                    Some((_, n)) => *n += 1,
                    None => dishes.push((dish, 1)),
                }
            }
        }
        (dishes, self.daily.iter().find(|i| i.product == product).map(|i| i.when))
    }

    /// How many of the planned days a daily item is bought for: the days anything is cooked
    /// on, or the days a given meal is.
    pub fn days_for(&self, when: When) -> u32 {
        (1..=self.days)
            .filter(|&day| match when {
                When::EveryDay => Meal::ALL.iter().any(|m| self.dish_on(day, *m).is_some()),
                When::With(meal) => self.dish_on(day, meal).is_some(),
            })
            .count() as u32
    }

    /// The first meal cooked on the first day, and the last one on the last day - where the
    /// trip starts and ends, as the plan has it. Breakfast and dinner when nothing is cooked.
    pub fn arrival(&self) -> Meal {
        Meal::ALL.into_iter().find(|m| self.dish_on(1, *m).is_some()).unwrap_or(Meal::Breakfast)
    }

    pub fn departure(&self) -> Meal {
        Meal::ALL.into_iter().rev().find(|m| self.dish_on(self.days, *m).is_some()).unwrap_or(Meal::Dinner)
    }

    /// Arriving for this meal: the first day's meals before it are nothing, and the ones
    /// from it on get a dish if they had none.
    pub fn arrive(&mut self, meal: Meal) {
        self.bound(1, meal, |m, at| m < at);
    }

    /// Leaving after this meal: the last day's meals after it are nothing.
    pub fn leave(&mut self, meal: Meal) {
        self.bound(self.days, meal, |m, at| m > at);
    }

    fn bound(&mut self, day: u32, at: Meal, outside: fn(usize, usize) -> bool) {
        let index = |m: Meal| Meal::ALL.iter().position(|x| *x == m).unwrap_or(0);
        for meal in Meal::ALL {
            if outside(index(meal), index(at)) {
                self.set(Slot { day, meal, dish: None });
            } else if self.slot(day, meal).is_some_and(|s| s.dish.is_none()) {
                // Emptied by an earlier arrival or departure: let `fill` choose again.
                self.plan.retain(|s| !(s.day == day && s.meal == meal));
            }
        }
        self.fill();
    }

    /// Everything to buy for the planned days, in the catalogue's order. A product nobody
    /// needs is left out, and so is an ingredient whose product is not in the catalogue.
    pub fn shopping(&self, eating: &Eating) -> Vec<Need<'_>> {
        let mut amounts: Vec<f64> = vec![0.0; self.products.len()];
        let mut add = |ingredient: &Ingredient, times: f64| {
            if let Some(i) = self.products.iter().position(|p| p.id == ingredient.product) {
                amounts[i] += ingredient.amount * times * eating.times(self.products[i].eaters);
            }
        };
        for day in 1..=self.days {
            for meal in Meal::ALL {
                if let Some(dish) = self.dish_on(day, meal) {
                    for ingredient in &dish.ingredients {
                        add(ingredient, 1.0);
                    }
                }
            }
        }
        for ingredient in &self.daily {
            add(ingredient, f64::from(self.days_for(ingredient.when)));
        }
        self.products
            .iter()
            .zip(amounts)
            .filter(|(_, amount)| *amount > 0.0)
            .map(|(product, amount)| Need { product, amount })
            .collect()
    }
}

fn value_of(extras: &Extras) -> Option<&serde_json::Value> {
    extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(MENU))
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour(weights: &[i32]) -> Tour {
        let persons: Vec<serde_json::Value> = weights
            .iter()
            .enumerate()
            .map(|(i, w)| serde_json::json!({"GUID": format!("p{i}"), "Name": format!("P{i}"), "Weight": w}))
            .collect();
        Tour::from_json(&serde_json::json!({"Id": "t", "Name": "t", "Persons": persons}).to_string())
            .expect("tour")
    }

    fn product(id: &str, eaters: Eaters) -> Product {
        Product { id: id.into(), name: id.into(), unit: Unit::Gram, place: "market".into(), eaters, own_unit: None, category: None }
    }

    fn menu() -> Menu {
        let mut m = Menu {
            on: true,
            days: 2,
            start: None,
            places: vec![Place { id: "market".into(), name: "Market".into() }],
            products: vec![
                product("lamb", Eaters::Everyone),
                product("rice", Eaters::Everyone),
                product("wine", Eaters::FullWeight),
                product("juice", Eaters::Others),
            ],
            dishes: vec![
                Dish {
                    id: "plov".into(),
                    name: "Plov".into(),
                    meals: vec![Meal::Dinner],
                    meal: None,
                    ingredients: vec![
                        Ingredient { product: "lamb".into(), amount: 200.0, ..Ingredient::default() },
                        Ingredient { product: "rice".into(), amount: 100.0, ..Ingredient::default() },
                    ],
                },
                Dish {
                    id: "steak".into(),
                    name: "Steak".into(),
                    meals: vec![Meal::Dinner],
                    meal: None,
                    ingredients: vec![Ingredient { product: "lamb".into(), amount: 300.0, ..Ingredient::default() }],
                },
            ],
            daily: vec![
                Ingredient { product: "wine".into(), amount: 350.0, ..Ingredient::default() },
                Ingredient { product: "juice".into(), amount: 500.0, ..Ingredient::default() },
            ],
            plan: vec![],
            purchases: vec![],
        };
        m.fill();
        m
    }

    fn amount(needs: &[Need], id: &str) -> f64 {
        needs.iter().find(|n| n.product.id == id).map_or(0.0, |n| n.amount)
    }

    /// Two adults and a child at 50: 2.5 portions of dinner, wine for two, juice for one.
    #[test]
    fn portions_are_weights_and_wine_is_for_the_full_weight() {
        let eating = Eating::of(&tour(&[100, 100, 50]));
        assert_eq!((eating.portions, eating.full, eating.others, eating.full_weight), (2.5, 2.0, 1.0, 100));
        let m = menu();
        let needs = m.shopping(&eating);
        // Plov on day 1, steak on day 2: (200 + 300) × 2.5.
        assert_eq!(amount(&needs, "lamb"), 1250.0);
        assert_eq!(amount(&needs, "rice"), 250.0);
        assert_eq!(amount(&needs, "wine"), 350.0 * 2.0 * 2.0);
        assert_eq!(amount(&needs, "juice"), 500.0 * 2.0);
    }

    /// Everybody at 100: nobody below the full weight, so no juice on the list at all.
    #[test]
    fn nobody_below_the_full_weight_means_no_juice() {
        let m = menu();
        let needs = m.shopping(&Eating::of(&tour(&[100, 100])));
        assert!(needs.iter().all(|n| n.product.id != "juice"));
    }

    /// Mostly children: the full weight is theirs, and the two at 100 are above it - still
    /// full weights, still wine.
    #[test]
    fn the_full_weight_is_the_most_common_one() {
        let eating = Eating::of(&tour(&[35, 35, 35, 100, 100]));
        assert_eq!((eating.full_weight, eating.full, eating.others), (35, 5.0, 0.0));
    }

    #[test]
    fn dinners_take_turns_and_an_emptied_one_stays_empty() {
        let mut m = menu();
        let dinners = |m: &Menu| (1..=m.days).map(|d| m.dish_on(d, Meal::Dinner).map(|x| x.id.clone())).collect::<Vec<_>>();
        assert_eq!(dinners(&m), [Some("plov".into()), Some("steak".into())]);
        m.set(Slot { day: 2, meal: Meal::Dinner, dish: None });
        m.set_days(3);
        assert_eq!(dinners(&m), [Some("plov".into()), None, Some("plov".into())]);
        // Fewer days keep the plan of the ones taken off.
        m.set_days(1);
        m.set_days(3);
        assert_eq!(dinners(&m), [Some("plov".into()), None, Some("plov".into())]);
    }

    /// The menu goes into the tour's extras and comes back out of its JSON, as a server
    /// that knows nothing about it would pass it along.
    #[test]
    fn a_menu_survives_the_tour_json() {
        let mut t = tour(&[100]);
        assert!(Menu::of(&t).is_none());
        menu().put(&mut t);
        let back = Tour::from_json(&t.to_json().expect("json")).expect("tour");
        assert_eq!(Menu::of(&back), Some(menu()));
        assert!(Menu::shown(&back).is_some());
    }

    /// Rice is on the list because of the plov on day 1 - and only while plov is planned.
    #[test]
    fn a_product_says_which_dishes_it_is_for() {
        let mut m = menu();
        let (dishes, daily) = m.sources("rice");
        assert_eq!(dishes.iter().map(|(d, n)| (d.id.as_str(), *n)).collect::<Vec<_>>(), [("plov", 1)]);
        assert!(daily.is_none());
        assert_eq!(m.sources("wine").1, Some(When::EveryDay));
        m.set(Slot { day: 1, meal: Meal::Dinner, dish: Some("steak".into()) });
        assert!(m.sources("rice").0.is_empty());
        let eating = Eating::of(&tour(&[100]));
        assert!(m.shopping(&eating).iter().all(|n| n.product.id != "rice"));
    }

    /// A dish from before `meals` - `"Meal": "Dinner"` - is read as a dinner, and written
    /// back with `Meals` only.
    #[test]
    fn a_dish_of_one_meal_is_read_as_a_dish_of_several() {
        let mut t = tour(&[100]);
        let mut json = serde_json::to_value(menu()).expect("json");
        for d in json["Dishes"].as_array_mut().expect("dishes") {
            d.as_object_mut().expect("dish").remove("Meals");
            d["Meal"] = "Dinner".into();
        }
        tc_core_set(&mut t, json);
        let m = Menu::of(&t).expect("menu");
        assert!(m.dishes.iter().all(|d| d.meals == [Meal::Dinner] && d.meal.is_none()));
        let written = serde_json::to_value(&m).expect("json");
        assert!(written["Dishes"][0].get("Meal").is_none());
        assert_eq!(m.dishes_for(Meal::Dinner).count(), 2);
    }

    fn tc_core_set(t: &mut Tour, menu: serde_json::Value) {
        crate::extras::set(&mut t.extras, MENU, menu);
    }

    /// A dish taken out leaves its meals empty; a product still in a dish is not taken out.
    #[test]
    fn removing_a_dish_empties_its_meals_and_a_used_product_stays() {
        let mut m = menu();
        m.remove_dish("plov");
        assert!(m.dish("plov").is_none());
        assert_eq!(m.slot(1, Meal::Dinner).map(|s| s.dish.clone()), Some(None));
        m.remove_product("lamb");
        assert!(m.product("lamb").is_some(), "the steak has lamb");
        m.remove_product("rice");
        assert!(m.product("rice").is_none(), "nothing has rice once the plov is gone");
    }

    /// Friday for dinner to Sunday after breakfast: three days, two dinners - two evenings of
    /// wine - and juice for all three days something is cooked.
    #[test]
    fn a_weekend_is_two_evenings_of_wine_and_three_days_of_juice() {
        let mut m = menu();
        m.dishes.push(Dish {
            id: "porridge".into(),
            name: "Porridge".into(),
            meals: vec![Meal::Breakfast],
            meal: None,
            ingredients: vec![Ingredient { product: "rice".into(), amount: 50.0, ..Ingredient::default() }],
        });
        m.daily[0].when = When::With(Meal::Dinner);
        m.set_days(3);
        m.arrive(Meal::Dinner);
        m.leave(Meal::Breakfast);
        assert_eq!((m.arrival(), m.departure()), (Meal::Dinner, Meal::Breakfast));
        assert!(m.dish_on(1, Meal::Breakfast).is_none() && m.dish_on(1, Meal::Dinner).is_some());
        assert!(m.dish_on(3, Meal::Breakfast).is_some() && m.dish_on(3, Meal::Dinner).is_none());
        assert_eq!(m.days_for(When::With(Meal::Dinner)), 2);
        assert_eq!(m.days_for(When::EveryDay), 3);
        let eating = Eating::of(&tour(&[100, 100, 50]));
        let needs = m.shopping(&eating);
        assert_eq!(amount(&needs, "wine"), 350.0 * 2.0 * 2.0, "two adults, two evenings");
        assert_eq!(amount(&needs, "juice"), 500.0 * 3.0, "one child, three days");
        // A fourth day: Sunday is a whole day now, and the departure is Monday's.
        m.set_days(4);
        assert!(m.dish_on(3, Meal::Dinner).is_some() && m.dish_on(4, Meal::Dinner).is_none());
        assert_eq!(m.departure(), Meal::Breakfast);
        // Arriving for breakfast after all fills the first day's breakfast again.
        m.arrive(Meal::Breakfast);
        assert!(m.dish_on(1, Meal::Breakfast).is_some());
    }

    /// "When" is written only for a daily item that has one: a dish's ingredients and an
    /// every-day item stay as they were.
    #[test]
    fn when_is_written_only_when_it_says_something() {
        let plain = serde_json::to_value(Ingredient { product: "rice".into(), amount: 1.0, ..Ingredient::default() }).unwrap();
        assert!(plain.get("When").is_none());
        let wine = Ingredient { product: "wine".into(), amount: 1.0, when: When::With(Meal::Dinner) };
        let back: Ingredient = serde_json::from_value(serde_json::to_value(&wine).unwrap()).unwrap();
        assert_eq!(back, wine);
    }

    #[test]
    fn a_place_something_is_bought_at_stays_and_categories_come_once() {
        let mut m = menu();
        m.put_place(Place { id: "shop".into(), name: "Shop".into() });
        m.put_place(Place { id: "shop".into(), name: "Corner shop".into() });
        assert_eq!(m.places.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Market", "Corner shop"]);
        m.remove_place("market");
        assert!(m.places.iter().any(|p| p.id == "market"), "lamb is bought there");
        m.remove_place("shop");
        assert!(m.places.iter().all(|p| p.id != "shop"));
        m.products[0].category = Some("Meat".into());
        m.products[1].category = Some(" Meat ".into());
        m.products[2].category = Some("Drinks".into());
        assert_eq!(m.categories(), ["Meat", "Drinks"]);
    }

    #[test]
    fn an_empty_dish_is_never_planned_by_itself() {
        let mut m = menu();
        m.dishes.push(Dish { id: "out".into(), name: "Restaurant".into(), meals: vec![Meal::Dinner], ..Dish::default() });
        m.plan.clear();
        m.set_days(6);
        assert!((1..=6).all(|d| m.dish_on(d, Meal::Dinner).is_some_and(|x| x.id != "out")));
    }

    #[test]
    fn a_purchase_is_made_once_and_only_when_something_is_said() {
        let mut m = menu();
        assert!(m.purchase("lamb").is_none());
        m.purchase_mut("lamb").bought = true;
        m.purchase_mut("lamb").who = Some("p0".into());
        assert_eq!(m.purchases.len(), 1);
        assert_eq!(m.purchase("lamb").map(|p| (p.bought, p.who.clone())), Some((true, Some("p0".into()))));
        assert_eq!(m.products_at("market").count(), 4);
    }

    /// A template is the catalogue only; taking it keeps the plan where its dishes survive.
    #[test]
    fn a_template_is_the_catalogue_and_taking_it_keeps_what_still_fits() {
        let mut trip = menu();
        trip.purchase_mut("lamb").bought = true;
        trip.purchase_mut("rice").bought = true;
        let template = trip.as_template();
        assert!(template.on && template.plan.is_empty() && template.purchases.is_empty());
        assert_eq!((template.days, template.start.clone()), (0, None));
        assert_eq!(template.dishes, trip.dishes);

        // The group dropped steak and rice from the template since.
        let mut newer = template.clone();
        newer.dishes.retain(|d| d.id != "steak");
        newer.products.retain(|p| p.id != "rice");
        newer.dishes[0].ingredients.retain(|i| i.product != "rice");

        trip.take_catalogue(&newer);
        assert_eq!(trip.dishes, newer.dishes);
        assert_eq!(trip.dish_on(1, Meal::Dinner).map(|d| d.id.as_str()), Some("plov"), "plov stays planned");
        assert!(trip.slot(2, Meal::Dinner).is_none(), "the steak's dinner is unplanned again");
        assert!(trip.purchase("lamb").is_some_and(|p| p.bought), "still bought");
        assert!(trip.purchase("rice").is_none(), "a product the template dropped takes its purchase along");
        assert_eq!(trip.days, 2, "the trip keeps its length");
    }
}
