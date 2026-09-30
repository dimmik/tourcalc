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
}

/// So much of a product for one portion - or, for a product counted by heads, one person.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Ingredient {
    pub product: String,
    pub amount: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Dish {
    pub id: String,
    pub name: String,
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
    pub places: Vec<Place>,
    pub products: Vec<Product>,
    pub dishes: Vec<Dish>,
    /// Bought every day whatever is cooked: vegetables, bread, water, wine.
    pub daily: Vec<Ingredient>,
    pub plan: Vec<Slot>,
    pub errands: Vec<Errand>,
}

/// One place's shopping: who goes, what is in the basket already, and the expense it was
/// recorded as. Made the first time any of that is said, so a place nobody has touched has
/// none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Errand {
    pub place: String,
    /// The person who does this shopping, if anybody has taken it.
    pub who: Option<String>,
    /// Products ticked off as bought.
    pub bought: Vec<String>,
    /// The expense this shopping was recorded as. Only a pointer: whether it is still
    /// there is the tour's say - deleted, it is "not recorded" again.
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
        serde_json::from_value(value.clone()).ok()
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

    pub fn product(&self, id: &str) -> Option<&Product> {
        self.products.iter().find(|p| p.id == id)
    }

    pub fn dish(&self, id: &str) -> Option<&Dish> {
        self.dishes.iter().find(|d| d.id == id)
    }

    pub fn dishes_for(&self, meal: Meal) -> impl Iterator<Item = &Dish> {
        self.dishes.iter().filter(move |d| d.meal == Some(meal))
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
    pub fn set_days(&mut self, days: u32) {
        self.days = days.max(1);
        self.fill();
    }

    /// Gives every planned day that has no slot for a meal the next dish for that meal in
    /// turn, so that five dinners are five different dinners while there are five to choose
    /// from. A slot somebody emptied stays empty.
    pub fn fill(&mut self) {
        for meal in Meal::ALL {
            let choice: Vec<String> = self.dishes_for(meal).map(|d| d.id.clone()).collect();
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

    pub fn errand(&self, place: &str) -> Option<&Errand> {
        self.errands.iter().find(|e| e.place == place)
    }

    /// The place's errand, made if it is not there yet.
    pub fn errand_mut(&mut self, place: &str) -> &mut Errand {
        match self.errands.iter().position(|e| e.place == place) {
            Some(i) => &mut self.errands[i],
            None => {
                self.errands.push(Errand { place: place.to_owned(), ..Errand::default() });
                self.errands.last_mut().expect("just pushed")
            }
        }
    }

    /// Ticks a product off at a place, or back on.
    pub fn set_bought(&mut self, place: &str, product: &str, bought: bool) {
        let errand = self.errand_mut(place);
        errand.bought.retain(|p| p != product);
        if bought {
            errand.bought.push(product.to_owned());
        }
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
            add(ingredient, f64::from(self.days));
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
        Product { id: id.into(), name: id.into(), unit: Unit::Gram, place: "market".into(), eaters }
    }

    fn menu() -> Menu {
        let mut m = Menu {
            on: true,
            days: 2,
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
                    meal: Some(Meal::Dinner),
                    ingredients: vec![
                        Ingredient { product: "lamb".into(), amount: 200.0 },
                        Ingredient { product: "rice".into(), amount: 100.0 },
                    ],
                },
                Dish {
                    id: "steak".into(),
                    name: "Steak".into(),
                    meal: Some(Meal::Dinner),
                    ingredients: vec![Ingredient { product: "lamb".into(), amount: 300.0 }],
                },
            ],
            daily: vec![
                Ingredient { product: "wine".into(), amount: 350.0 },
                Ingredient { product: "juice".into(), amount: 500.0 },
            ],
            plan: vec![],
            errands: vec![],
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

    #[test]
    fn ticking_off_twice_is_one_tick_and_unticking_takes_it_off() {
        let mut m = menu();
        assert!(m.errand("market").is_none());
        m.set_bought("market", "lamb", true);
        m.set_bought("market", "lamb", true);
        assert_eq!(m.errand("market").map(|e| e.bought.clone()), Some(vec!["lamb".to_owned()]));
        m.set_bought("market", "lamb", false);
        assert_eq!(m.errand("market").map(|e| e.bought.len()), Some(0));
        assert_eq!(m.errands.len(), 1);
    }
}
