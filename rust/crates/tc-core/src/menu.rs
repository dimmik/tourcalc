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
//! at 100 eats one portion, a child at 35 a third of one. Two kinds of product are only for
//! some: what the grown-ups have (wine), at weight 100 and over ([`GROWN_UP`]), and what the
//! children have (juice) - counted by weight among them too: a child at 50 gets half a
//! juice, a couple written as one at 200 two bottles.

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
    /// Everybody at the full weight or above, by weight, nothing for the others. Wine: a
    /// couple written as one person at 200 drinks two people's.
    FullWeight,
    /// Everybody below the full weight, by weight. Juice: a child at 50 drinks half of what
    /// a teenager at 100 would - were the teenager not already a grown-up by weight.
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
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
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
    /// What one pack holds, in the product's unit - 1000 for a kilogram of prawns, 10 for
    /// a stack of lavash - if it is only sold in packs: the shopping is then whole packs (see
    /// [`packs`]), counted on what the whole trip needs, not dish by dish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack: Option<f64>,
    /// The pack's word - "bottle 0.7", "bowl" - if it has one; else the list says the size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_name: Option<String>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// How many packs of `size` buy `amount`: up to a whole one, but not one more for the last
/// 5 % - 4.01 lemons are 4 - and never none.
pub fn packs(amount: f64, size: f64) -> f64 {
    if !(size > 0.0) || !(amount > 0.0) {
        return 0.0;
    }
    let below = (amount / size + 1e-9).floor();
    if below >= 1.0 && amount - below * size <= amount * 0.05 {
        below
    } else {
        (amount / size - 1e-9).ceil().max(1.0)
    }
}

impl Product {
    /// The pack size, if it is sold in packs of a size that means anything.
    pub fn pack_size(&self) -> Option<f64> {
        self.pack.filter(|s| s.is_finite() && *s > 0.0)
    }
}

/// So much of a product for one portion - or, for a product counted by heads, one person.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Ingredient {
    pub product: String,
    pub amount: f64,
    /// What the amount is for: a portion, by default; a head, a fire, the trip, a day.
    #[serde(skip_serializing_if = "Per::is_portion")]
    pub per: Per,
    /// On the daily list: which days it is bought for. Nothing in a dish - written only when
    /// it is not the default, so a dish's ingredients stay two fields.
    #[serde(skip_serializing_if = "When::is_every_day")]
    pub when: When,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
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

/// What an ingredient's amount is for. Most food is per portion, by weight; some things
/// are not shared out by weight at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Per {
    /// One portion, by weight - a child at 35 eats a third of it - of the product's eaters.
    #[default]
    Portion,
    /// One person of the product's eaters, however much they weigh: a bowl of noodles, a
    /// lavash, a juice box. A couple written as one at 200 is two.
    Head,
    /// One fire - a meal cooked on the grill, whatever is on it: charcoal, firelighters,
    /// foil. Two dishes grilled at one dinner share the fire, so a meal takes the largest of
    /// its dishes' amounts, not their sum.
    Fire,
    /// The whole trip, once, if the dish is cooked at all (or, on the daily list, if any day
    /// is): the syringe for the sausages, salt, ketchup.
    Trip,
    /// Each day it is cooked on (or the daily list's days), whoever is there: bin bags.
    Day,
}

impl Per {
    pub const ALL: [Per; 5] = [Per::Portion, Per::Head, Per::Fire, Per::Trip, Per::Day];

    pub fn is_portion(&self) -> bool {
        *self == Per::Portion
    }

    /// Whether the product's eaters count: portions and heads are somebody's, a fire, the
    /// trip and a day are nobody's.
    pub fn by_people(&self) -> bool {
        matches!(self, Per::Portion | Per::Head)
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
    /// How it is cooked - the grill, a campfire - by [`Method`] id: what that needs is the
    /// method's, bought once a meal however many dishes share it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// A way of cooking - the grill on charcoal, a campfire - and what it takes each time it is
/// lit: charcoal and firelighters, or firewood. Dishes say they are cooked so; a meal with
/// any of them lights it once, so meat and prawns at one dinner are one grill, and a grill
/// at lunch and another at dinner are two. Its amounts are "per use" (`Per::Fire`) unless
/// said otherwise: per portion, if more people need more charcoal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Method {
    pub id: String,
    pub name: String,
    pub ingredients: Vec<Ingredient>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// One meal of one day: what is cooked for it, in order - plov, and a salad, and mulled
/// wine. No dishes is "nothing cooked" - eating out, say - which is not the same as a day
/// nobody has planned yet: that one has no slot at all, and is filled in when the days are.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SlotWire", into = "SlotWire")]
pub struct Slot {
    /// From 1.
    pub day: u32,
    pub meal: Meal,
    pub dishes: Vec<String>,
}

impl Slot {
    /// One dish, or nothing.
    pub fn one(day: u32, meal: Meal, dish: Option<String>) -> Slot {
        Slot { day, meal, dishes: dish.into_iter().collect() }
    }
}

/// A slot as stored. A meal had one dish before it could have several, and a client from
/// then reads `Dish` and writes `Dish` only - so the first dish is always written there, and
/// `Dishes` only when there are more. Read back, `Dishes` wins when it is there: a client
/// that knows only `Dish` drops it on saving, and then `Dish` is the whole story.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SlotWire {
    day: u32,
    meal: Meal,
    #[serde(default)]
    dish: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dishes: Vec<String>,
}

impl From<SlotWire> for Slot {
    fn from(w: SlotWire) -> Slot {
        let dishes = if w.dishes.is_empty() { w.dish.into_iter().collect() } else { w.dishes };
        Slot { day: w.day, meal: w.meal, dishes }
    }
}

impl From<Slot> for SlotWire {
    fn from(s: Slot) -> SlotWire {
        SlotWire {
            day: s.day,
            meal: s.meal,
            dish: s.dishes.first().cloned(),
            dishes: if s.dishes.len() > 1 { s.dishes } else { Vec::new() },
        }
    }
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
    /// How things are cooked - see [`Method`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<Method>,
    /// Bought every day whatever is cooked: vegetables, bread, water, wine.
    pub daily: Vec<Ingredient>,
    pub plan: Vec<Slot>,
    pub purchases: Vec<Purchase>,
    /// A word for a day, by its number from 1 - "arrival", "radial hikes", "leaving" - beside
    /// its date or number. Kept for days beyond the trip's length too: shortened and grown
    /// again, a day finds its word where it left it.
    pub day_notes: Vec<String>,
    /// The category expenses from the shopping go under, if the group renamed it from the
    /// default ("Shopping"); each expense adds its products' own category below it - see
    /// [`Menu::expense_category`].
    pub expense_category: Option<String>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// One product's shopping: who buys and pays for it, where, whether it is bought, and the
/// expense it was recorded as. Made the first time any of that is said, so a product nobody has
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
    /// Where it is bought, as whoever buys it says - "Lidl", "delivery" - if not at the
    /// product's own place from the catalogue. Text, not a catalogue place: one person's
    /// "Lidl" is nobody else's business, and it does not go back into the catalogue.
    pub place: Option<String>,
    /// Ticked for the whole trip. The tick and the expenses are each other's business only
    /// in the group's head: ticked by hand, never by recording - the usual wine is bought and
    /// recorded, the special one is brought from home, and only then is all the wine ticked -
    /// and unticking forgets no expense.
    pub bought: bool,
    /// Ticked for these days only - the meat for Friday, the rest on Saturday's market. It is
    /// bought once these cover every day it is needed on, or `bought` says so.
    pub bought_days: Vec<u32>,
    /// The expense it was recorded as for the whole trip - one receipt can be several
    /// products', so several can point at one expense. Only a pointer: whether the expense is
    /// still there is the tour's say, and deleted, the product is "not recorded" again.
    pub spending: Option<String>,
    /// Recorded for some days only, each part as its own expense.
    pub parts: Vec<Part>,
    /// A word about it for whoever buys it - "Perlenbacher Gold", "Olga decides".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Parts of it split off as lines of their own - coffee: a pack from Sasha's shelf at
    /// home, the rest bought by Sergey. Each has its buyer, place, tick and expense; the
    /// purchase itself is the rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub splits: Vec<Split>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

/// So much of a product split off its purchase as a line of its own, for the whole trip.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Split {
    pub id: String,
    pub who: Option<String>,
    /// Where it is got, as whoever gets it says: "From home", "Metro".
    pub place: Option<String>,
    /// In the product's unit.
    pub amount: f64,
    pub bought: bool,
    /// The expense it was recorded as, as with the purchase's.
    pub spending: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Whatever a later version wrote here that this one does not know: kept as it was, so
    /// that saving through this version does not wipe it - see `nuances.md`.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

impl Purchase {
    /// What the split-off lines take between them.
    pub fn split_off(&self) -> f64 {
        self.splits.iter().map(|s| s.amount.max(0.0)).sum()
    }

    pub fn split(&self, id: &str) -> Option<&Split> {
        self.splits.iter().find(|s| s.id == id)
    }

    pub fn split_mut(&mut self, id: &str) -> Option<&mut Split> {
        self.splits.iter_mut().find(|s| s.id == id)
    }

    /// Takes the rest away: the last split-off line becomes the purchase, with its buyer,
    /// place, tick, expense and note - Sergey does not buy coffee after all, Sasha brings it
    /// all. Its amount is then whatever the trip needs. The rest's expense stays in the tour.
    /// Nothing, with no line to take its place.
    pub fn drop_rest(&mut self) {
        let Some(last) = self.splits.pop() else { return };
        self.who = last.who;
        self.place = last.place;
        self.bought = last.bought;
        self.bought_days.clear();
        self.spending = last.spending;
        self.parts.clear();
        self.note = last.note;
    }
}

/// Some of a product, recorded as an expense for these days of the plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Part {
    /// The days it is for, by number from 1, in order.
    pub days: Vec<u32>,
    /// The expense. `None` only in what a beta of 4 October 2026 wrote, where a part was a
    /// tick as well: [`Menu::of`] reads such a part as ticked days.
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
    /// Heads at the full weight or above - said on the plan, not counted with.
    pub full: f64,
    /// Heads below it.
    pub others: f64,
    /// Their weights over 100: what the grown-ups' and the children's products are bought
    /// for. Counted in heads, a child at 35 got a whole juice, and two people written as one
    /// at 200 one bottle of wine between them.
    pub full_portions: f64,
    pub others_portions: f64,
    /// The full weight itself.
    pub full_weight: i32,
    /// People, for what is counted per head: everybody, the grown-ups, the others. A couple
    /// written as one at 200 is two, somebody at 0 - not eating - none.
    pub heads: f64,
    pub full_heads: f64,
    pub others_heads: f64,
}

impl Eating {
    pub fn of(tour: &Tour) -> Eating {
        let full_weight = GROWN_UP;
        let mut e = Eating {
            portions: 0.0,
            full: 0.0,
            others: 0.0,
            full_portions: 0.0,
            others_portions: 0.0,
            full_weight,
            heads: 0.0,
            full_heads: 0.0,
            others_heads: 0.0,
        };
        for p in &tour.persons {
            let share = f64::from(p.weight.max(0)) / 100.0;
            let heads = if p.weight <= 0 { 0.0 } else { f64::from((p.weight / 100).max(1)) };
            e.portions += share;
            e.heads += heads;
            if p.weight >= full_weight {
                e.full += 1.0;
                e.full_portions += share;
                e.full_heads += heads;
            } else {
                e.others += 1.0;
                e.others_portions += share;
                e.others_heads += heads;
            }
        }
        e
    }

    pub fn times(&self, eaters: Eaters) -> f64 {
        match eaters {
            Eaters::Everyone => self.portions,
            Eaters::FullWeight => self.full_portions,
            Eaters::Others => self.others_portions,
        }
    }

    /// How many times an amount `per` this is bought for these eaters, at one meal or day.
    pub fn times_per(&self, eaters: Eaters, per: Per) -> f64 {
        match (per, eaters) {
            (Per::Portion, _) => self.times(eaters),
            (Per::Head, Eaters::Everyone) => self.heads,
            (Per::Head, Eaters::FullWeight) => self.full_heads,
            (Per::Head, Eaters::Others) => self.others_heads,
            (Per::Fire | Per::Trip | Per::Day, _) => 1.0,
        }
    }
}

/// Who pays for a product, by whom it is for: `None` for everybody - the tour's own "for
/// everyone, by weight" - and otherwise the people at the full weight or above, or those
/// below it, by weight as it was bought. Wine bought for the grown-ups is the grown-ups'
/// expense; recorded "for everyone", the children paid a share of it.
pub fn payers_for(tour: &Tour, eaters: Eaters) -> Option<Vec<crate::PersonId>> {
    let full = GROWN_UP;
    let grown = match eaters {
        Eaters::Everyone => return None,
        Eaters::FullWeight => true,
        Eaters::Others => false,
    };
    Some(tour.persons.iter().filter(|p| (p.weight >= full) == grown).map(|p| p.id.clone()).collect())
}

/// From what weight on somebody is a grown-up for the menu: wine, beer, the adults' share.
/// A weight, not "the most common weight in the tour": in a group of mostly children at 35
/// that made everybody at 35 a grown-up, wine and all. 100 is one portion, and a couple
/// written as one at 200 is two grown-ups' worth.
pub const GROWN_UP: i32 = 100;

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
        // A part without an expense was a tick for its days.
        for p in &mut menu.purchases {
            let ticks: Vec<u32> = p.parts.iter().filter(|x| x.spending.is_none()).flat_map(|x| x.days.clone()).collect();
            if !ticks.is_empty() {
                p.bought_days.extend(ticks);
                p.bought_days.sort_unstable();
                p.bought_days.dedup();
                p.parts.retain(|x| x.spending.is_some());
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
    /// The category an expense for these products goes into: "Shopping" - or whatever the
    /// group named it - and the products' own category under it when they share one,
    /// "Shopping/Alcohol". Several categories in one receipt are just "Shopping".
    pub fn expense_category(&self, root_by_default: &str, products: &[&Product]) -> String {
        let root = self.expense_category.as_deref().map(str::trim).filter(|r| !r.is_empty()).unwrap_or(root_by_default);
        let own = |p: &&Product| p.category.as_deref().map(str::trim).unwrap_or("").to_owned();
        match products.first().map(own) {
            Some(first) if !first.is_empty() && products.iter().all(|p| own(p) == first) => format!("{root}/{first}"),
            _ => root.to_owned(),
        }
    }

    pub fn as_template(&self) -> Menu {
        Menu {
            on: true,
            places: self.places.clone(),
            products: self.products.clone(),
            dishes: self.dishes.clone(),
            methods: self.methods.clone(),
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
        self.methods = template.methods.clone();
        self.daily = template.daily.clone();
        let dishes = &self.dishes;
        // A meal keeps the dishes still there; one left with none of its own goes back to
        // unplanned. One that was "nothing" stays nothing.
        self.plan.retain_mut(|s| {
            let had = !s.dishes.is_empty();
            s.dishes.retain(|id| dishes.iter().any(|d| &d.id == id));
            !had || !s.dishes.is_empty()
        });
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

    /// What is cooked for this meal of this day - the main dish, the first.
    pub fn dish_on(&self, day: u32, meal: Meal) -> Option<&Dish> {
        self.dishes_on(day, meal).into_iter().next()
    }

    /// Everything cooked for this meal of this day, in order; a dish since deleted is left
    /// out.
    pub fn dishes_on(&self, day: u32, meal: Meal) -> Vec<&Dish> {
        self.slot(day, meal)
            .map(|s| s.dishes.iter().filter_map(|id| self.dish(id)).collect())
            .unwrap_or_default()
    }

    /// Puts dishes, or nothing, on one meal of one day.
    pub fn set(&mut self, slot: Slot) {
        match self.plan.iter_mut().find(|s| s.day == slot.day && s.meal == slot.meal) {
            Some(s) => s.dishes = slot.dishes,
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
            self.plan.retain(|s| !(s.day == was_last && s.dishes.is_empty() && index(s.meal) > index(leaving)));
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
                    self.plan.push(Slot::one(day, meal, Some(dish)));
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

    /// The word written for a day, if any.
    pub fn day_note(&self, day: u32) -> Option<&str> {
        let at = usize::try_from(day).ok()?.checked_sub(1)?;
        self.day_notes.get(at).map(|n| n.trim()).filter(|n| !n.is_empty())
    }

    /// Writes a day's word; an empty one takes it away.
    pub fn set_day_note(&mut self, day: u32, note: &str) {
        let Some(at) = usize::try_from(day).ok().and_then(|d| d.checked_sub(1)) else { return };
        if self.day_notes.len() <= at {
            self.day_notes.resize(at + 1, String::new());
        }
        self.day_notes[at] = note.trim().to_owned();
        while self.day_notes.last().is_some_and(|n| n.is_empty()) {
            self.day_notes.pop();
        }
    }

    /// A dish as another one is now - its meals and what goes in - under a new id and name,
    /// right after it in the list: plov with buckwheat starts as plov. Nothing, if there is
    /// no such dish or the id is taken already (a copy replayed is not a second copy).
    pub fn copy_dish(&mut self, from: &str, id: &str, name: &str) {
        if self.dishes.iter().any(|d| d.id == id) {
            return;
        }
        let Some(at) = self.dishes.iter().position(|d| d.id == from) else {
            return;
        };
        let copy = Dish { id: id.to_owned(), name: name.to_owned(), ..self.dishes[at].clone() };
        self.dishes.insert(at + 1, copy);
    }

    /// Takes a dish out of the catalogue, and out of the plan: the meals it was on are
    /// "nothing" until somebody picks another, rather than quietly something else.
    pub fn remove_dish(&mut self, id: &str) {
        self.dishes.retain(|d| d.id != id);
        for slot in &mut self.plan {
            slot.dishes.retain(|d| d != id);
        }
    }

    pub fn method(&self, id: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.id == id)
    }

    /// Adds a way of cooking, or replaces the one with its id.
    pub fn put_method(&mut self, method: Method) {
        match self.methods.iter_mut().find(|m| m.id == method.id) {
            Some(m) => *m = method,
            None => self.methods.push(method),
        }
    }

    /// Takes a way of cooking away, and out of the dishes cooked so.
    pub fn remove_method(&mut self, id: &str) {
        self.methods.retain(|m| m.id != id);
        for dish in &mut self.dishes {
            dish.methods.retain(|m| m != id);
        }
    }

    /// The ways of cooking one meal of one day lights, each once, in the menu's order.
    pub fn methods_on(&self, day: u32, meal: Meal) -> Vec<&Method> {
        let dishes = self.dishes_on(day, meal);
        self.methods.iter().filter(|m| dishes.iter().any(|d| d.methods.contains(&m.id))).collect()
    }

    /// The ways of cooking a product goes into, and how many times each is lit over these
    /// days - "Grill ×2" under the charcoal.
    pub fn methods_for(&self, product: &str, days: &[u32]) -> Vec<(&Method, u32)> {
        let mut out: Vec<(&Method, u32)> = Vec::new();
        for &day in days.iter().filter(|&&d| d >= 1 && d <= self.days) {
            for meal in Meal::ALL {
                for m in self.methods_on(day, meal) {
                    if !m.ingredients.iter().any(|i| i.product == product) {
                        continue;
                    }
                    match out.iter_mut().find(|(x, _)| x.id == m.id) {
                        Some((_, n)) => *n += 1,
                        None => out.push((m, 1)),
                    }
                }
            }
        }
        out
    }

    /// Every ingredient list there is: the dishes', the ways of cooking's and the daily one.
    fn all_ingredients(&self) -> impl Iterator<Item = &Ingredient> {
        self.dishes
            .iter()
            .flat_map(|d| d.ingredients.iter())
            .chain(self.methods.iter().flat_map(|m| m.ingredients.iter()))
            .chain(self.daily.iter())
    }

    /// Adds a product, or replaces the one with its id.
    pub fn put_product(&mut self, product: Product) {
        match self.products.iter_mut().find(|p| p.id == product.id) {
            Some(p) => *p = product,
            None => self.products.push(product),
        }
    }

    /// Whether a product is counted by people anywhere - per portion or per head - rather than
    /// only per fire, trip or day: charcoal is nobody's in particular, and its line does not
    /// say "everybody".
    pub fn by_people(&self, product: &str) -> bool {
        self.all_ingredients().any(|i| i.product == product && i.per.by_people())
    }

    /// Whether any dish, way of cooking or the daily list has it - such a product is not to
    /// be removed.
    pub fn uses(&self, product: &str) -> bool {
        self.all_ingredients().any(|i| i.product == product)
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

    /// Where a product is bought: where its purchase says, else its place in the catalogue.
    pub fn bought_at(&self, product: &Product) -> String {
        match self.purchase(&product.id).and_then(|p| p.place.as_deref()) {
            Some(place) => place.to_owned(),
            None => self.places.iter().find(|p| p.id == product.place).map(|p| p.name.clone()).unwrap_or_default(),
        }
    }

    /// Says where these products are bought. Blank, or the product's own place, is that
    /// place again - so a typed "Market" does not stay behind when the catalogue's market is
    /// renamed.
    pub fn set_bought_at(&mut self, products: &[String], place: &str) {
        let place = place.trim();
        for id in products {
            let own = self
                .products
                .iter()
                .find(|p| p.id == *id)
                .and_then(|p| self.places.iter().find(|x| x.id == p.place))
                .map(|x| x.name.trim().to_lowercase());
            let text = (!place.is_empty() && own.as_deref() != Some(place.to_lowercase().as_str())).then(|| place.to_owned());
            self.purchase_mut(id).place = text;
        }
    }

    /// Every place there is to shop at: the catalogue's, in its order, and then the ones
    /// typed in the shopping, as first said - each once, whatever its case.
    pub fn shop_places(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let typed = self.purchases.iter().filter_map(|p| p.place.clone());
        for name in self.places.iter().map(|p| p.name.clone()).chain(typed) {
            if !names.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
                names.push(name);
            }
        }
        names
    }

    /// The products bought at a place, in the catalogue's order.
    pub fn products_at<'a>(&'a self, place: &'a str) -> impl Iterator<Item = &'a Product> + 'a {
        self.products.iter().filter(move |p| p.place == place)
    }

    /// Why a product is on the list: the planned dishes it goes into, with how many times
    /// each is cooked, and when it is bought besides, if it is on the daily list.
    pub fn sources(&self, product: &str) -> (Vec<(&Dish, u32)>, Option<When>) {
        self.sources_on(product, &self.all_days())
    }

    /// The same for these days of the plan alone: the omelette cooked on days 2 and 3 is
    /// "×1" when shopping for day 2.
    pub fn sources_on(&self, product: &str, days: &[u32]) -> (Vec<(&Dish, u32)>, Option<When>) {
        let mut dishes: Vec<(&Dish, u32)> = Vec::new();
        for &day in days.iter().filter(|&&d| d >= 1 && d <= self.days) {
            for meal in Meal::ALL {
                for dish in self.dishes_on(day, meal) {
                    if !dish.ingredients.iter().any(|i| i.product == product) {
                        continue;
                    }
                    match dishes.iter_mut().find(|(d, _)| d.id == dish.id) {
                        Some((_, n)) => *n += 1,
                        None => dishes.push((dish, 1)),
                    }
                }
            }
        }
        (dishes, self.daily.iter().find(|i| i.product == product).map(|i| i.when))
    }

    /// How many of the planned days a daily item is bought for: the days anything is cooked
    /// on, or the days a given meal is.
    pub fn days_for(&self, when: When) -> u32 {
        (1..=self.days).filter(|&day| self.daily_on(when, day)).count() as u32
    }

    /// Whether what is bought "every day" or "with dinner" is bought for this day: on a day
    /// something is cooked, or that meal is.
    pub fn daily_on(&self, when: When, day: u32) -> bool {
        match when {
            When::EveryDay => Meal::ALL.iter().any(|m| self.dish_on(day, *m).is_some()),
            When::With(meal) => self.dish_on(day, meal).is_some(),
        }
    }

    /// Every day of the plan, 1 to the last.
    pub fn all_days(&self) -> Vec<u32> {
        (1..=self.days).collect()
    }

    /// The days a product is needed on: a dish with it planned, or it on the daily list.
    pub fn days_needing(&self, product: &str) -> Vec<u32> {
        let trip = self.trip_days(product);
        (1..=self.days)
            .filter(|&day| {
                trip.contains(&day)
                    || self.on_day(day).iter().any(|i| i.product == product && i.per != Per::Trip)
            })
            .collect()
    }

    /// The days what is bought once for the trip is bought for: the first one ticked, once it
    /// is ticked for any - it is bought then; until then every day it is there for, so that
    /// salt not bought with Friday's shopping is on Saturday's list and not forgotten.
    pub fn trip_days(&self, product: &str) -> Vec<u32> {
        let there: Vec<u32> = self
            .all_days()
            .into_iter()
            .filter(|&day| self.on_day(day).iter().any(|i| i.product == product && i.per == Per::Trip))
            .collect();
        let covered = match self.purchase(product) {
            Some(p) if p.bought => there.first().copied(),
            Some(p) => there.iter().copied().find(|d| p.bought_days.contains(d)),
            None => None,
        };
        match covered {
            Some(day) => vec![day],
            None => there,
        }
    }

    /// The days a product is ticked for: all of them if it was ticked for the whole trip,
    /// else those ticked one by one, in order. Expenses do not count: see [`Purchase::bought`].
    pub fn covered_days(&self, product: &str) -> Vec<u32> {
        match self.purchase(product) {
            Some(p) if p.bought => self.all_days(),
            Some(p) => p.bought_days.clone(),
            None => Vec::new(),
        }
    }

    /// The days a product is recorded for, by its parts' expenses that `exists` says are
    /// still in the tour; all of them, if its whole-trip expense is.
    pub fn recorded_days(&self, product: &str, exists: impl Fn(&str) -> bool) -> Vec<u32> {
        let Some(p) = self.purchase(product) else { return Vec::new() };
        if p.spending.as_deref().is_some_and(&exists) {
            return self.all_days();
        }
        let mut days: Vec<u32> = p
            .parts
            .iter()
            .filter(|x| x.spending.as_deref().is_some_and(&exists))
            .flat_map(|x| x.days.iter().copied())
            .collect();
        days.sort_unstable();
        days.dedup();
        days
    }

    /// Ticks a product, or unticks it, for these days only. Unticked for some days, a product
    /// ticked for the whole trip stays ticked for the rest of them. Its expenses stay as they
    /// are either way.
    pub fn set_bought_on(&mut self, product: &str, days: &[u32], bought: bool) {
        let needing = self.days_needing(product);
        let p = self.purchase_mut(product);
        if bought {
            p.bought_days.extend_from_slice(days);
        } else {
            if p.bought {
                p.bought = false;
                p.bought_days = needing;
            }
            p.bought_days.retain(|d| !days.contains(d));
        }
        p.bought_days.sort_unstable();
        p.bought_days.dedup();
    }

    /// Records the part of a product bought for these days as this expense. It ticks
    /// nothing; every part whose expense is gone, for whatever days, goes - a part is only a
    /// record now, and one without its expense says nothing.
    pub fn record_on(&mut self, product: &str, days: &[u32], spending: &str, exists: impl Fn(&str) -> bool) {
        let mut days = days.to_vec();
        days.sort_unstable();
        days.dedup();
        let p = self.purchase_mut(product);
        p.parts.retain(|x| x.spending.as_deref().is_some_and(&exists));
        p.parts.push(Part { days, spending: Some(spending.to_owned()) });
    }

    /// The first meal of the first day not set to nothing, and the last one of the last day -
    /// where the trip starts and ends, as [`Menu::arrive`] and [`Menu::leave`] left it.
    /// Breakfast and dinner when the whole day is nothing.
    ///
    /// Not "the first meal with a dish": a tour whose catalogue has no dinner yet - every
    /// tour that has just switched its menu on - would forget an arrival for dinner at once,
    /// and the choice would jump back to breakfast.
    pub fn arrival(&self) -> Meal {
        Meal::ALL.into_iter().find(|m| !self.is_nothing(1, *m)).unwrap_or(Meal::Breakfast)
    }

    pub fn departure(&self) -> Meal {
        Meal::ALL.into_iter().rev().find(|m| !self.is_nothing(self.days, *m)).unwrap_or(Meal::Dinner)
    }

    /// Set to "— nothing —": a slot with no dishes. A meal with no slot is still to be filled.
    fn is_nothing(&self, day: u32, meal: Meal) -> bool {
        self.slot(day, meal).is_some_and(|s| s.dishes.is_empty())
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
                self.set(Slot::one(day, meal, None));
            } else if self.slot(day, meal).is_some_and(|s| s.dishes.is_empty()) {
                // Emptied by an earlier arrival or departure: let `fill` choose again.
                self.plan.retain(|s| !(s.day == day && s.meal == meal));
            }
        }
        self.fill();
    }

    /// Everything to buy for the planned days, in the catalogue's order. A product nobody
    /// needs is left out, and so is an ingredient whose product is not in the catalogue.
    pub fn shopping(&self, eating: &Eating) -> Vec<Need<'_>> {
        self.shopping_on(eating, &self.all_days())
    }

    /// Everything to buy for these days of the plan alone - the shopping before Friday's
    /// market, when Sunday's is bought on Saturday.
    pub fn shopping_on(&self, eating: &Eating, days: &[u32]) -> Vec<Need<'_>> {
        let mut amounts: Vec<f64> = vec![0.0; self.products.len()];
        let at = |id: &str| self.products.iter().position(|p| p.id == id);
        // Once for the trip, the largest amount any dish or the daily list says, if any day
        // it is bought for is among these - see `trip_days`.
        for (p, product) in self.products.iter().enumerate() {
            let most = self
                .all_days()
                .into_iter()
                .flat_map(|day| self.on_day(day))
                .filter(|i| i.product == product.id && i.per == Per::Trip)
                .map(|i| i.amount)
                .fold(0.0, f64::max);
            if most > 0.0 && self.trip_days(&product.id).iter().any(|d| days.contains(d)) {
                amounts[p] += most;
            }
        }
        for &day in days.iter().filter(|&&d| d >= 1 && d <= self.days) {
            // A fire a meal, and a day a day: the largest of what the dishes say, not the sum.
            let mut daily_most: Vec<(usize, f64)> = Vec::new();
            let most = |list: &mut Vec<(usize, f64)>, p: usize, amount: f64| match list.iter_mut().find(|(q, _)| *q == p) {
                Some((_, a)) => *a = a.max(amount),
                None => list.push((p, amount)),
            };
            for meal in Meal::ALL {
                let mut fire: Vec<(usize, f64)> = Vec::new();
                for dish in self.dishes_on(day, meal) {
                    for i in &dish.ingredients {
                        let Some(p) = at(&i.product) else { continue };
                        match i.per {
                            Per::Portion | Per::Head => {
                                amounts[p] += i.amount * eating.times_per(self.products[p].eaters, i.per)
                            }
                            Per::Fire => most(&mut fire, p, i.amount),
                            Per::Day => most(&mut daily_most, p, i.amount),
                            Per::Trip => {}
                        }
                    }
                }
                for (p, amount) in fire {
                    amounts[p] += amount;
                }
                // Each way of cooking once a meal, whatever is on it.
                for method in self.methods_on(day, meal) {
                    for i in &method.ingredients {
                        let Some(p) = at(&i.product) else { continue };
                        match i.per {
                            Per::Portion | Per::Head => {
                                amounts[p] += i.amount * eating.times_per(self.products[p].eaters, i.per)
                            }
                            Per::Fire => amounts[p] += i.amount,
                            Per::Day => most(&mut daily_most, p, i.amount),
                            Per::Trip => {}
                        }
                    }
                }
            }
            for i in self.daily.iter().filter(|i| self.daily_on(i.when, day)) {
                let Some(p) = at(&i.product) else { continue };
                match i.per {
                    Per::Portion | Per::Head => amounts[p] += i.amount * eating.times_per(self.products[p].eaters, i.per),
                    // On the daily list a fire is a day with the meal: there is no dish to
                    // share it with.
                    Per::Fire | Per::Day => most(&mut daily_most, p, i.amount),
                    Per::Trip => {}
                }
            }
            for (p, amount) in daily_most {
                amounts[p] += amount;
            }
        }
        self.products
            .iter()
            .zip(amounts)
            .filter(|(_, amount)| *amount > 0.0)
            .map(|(product, amount)| Need { product, amount })
            .collect()
    }

    /// Everything that goes into a day: its dishes' ingredients and the daily list's for it.
    fn on_day(&self, day: u32) -> Vec<&Ingredient> {
        let mut out: Vec<&Ingredient> = Vec::new();
        for meal in Meal::ALL {
            for dish in self.dishes_on(day, meal) {
                out.extend(dish.ingredients.iter());
            }
            for method in self.methods_on(day, meal) {
                out.extend(method.ingredients.iter());
            }
        }
        out.extend(self.daily.iter().filter(|i| self.daily_on(i.when, day)));
        out
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
        Product { id: id.into(), name: id.into(), unit: Unit::Gram, place: "market".into(), eaters, own_unit: None, category: None, ..Product::default() }
    }

    fn menu() -> Menu {
        let mut m = Menu { methods: Default::default(), rest: Default::default(),
            on: true,
            days: 2,
            start: None,
            places: vec![Place { id: "market".into(), name: "Market".into(), ..Place::default() }],
            products: vec![
                product("lamb", Eaters::Everyone),
                product("rice", Eaters::Everyone),
                product("wine", Eaters::FullWeight),
                product("juice", Eaters::Others),
            ],
            dishes: vec![
                Dish { methods: Default::default(), rest: Default::default(),
                    id: "plov".into(),
                    name: "Plov".into(),
                    meals: vec![Meal::Dinner],
                    meal: None,
                    ingredients: vec![
                        Ingredient { product: "lamb".into(), amount: 200.0, ..Ingredient::default() },
                        Ingredient { product: "rice".into(), amount: 100.0, ..Ingredient::default() },
                    ],
                },
                Dish { methods: Default::default(), rest: Default::default(),
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
            day_notes: vec![],
            expense_category: None,
        };
        m.fill();
        m
    }

    fn item(product: &str, amount: f64, per: Per) -> Ingredient {
        Ingredient { product: product.into(), amount, per, ..Ingredient::default() }
    }

    /// The Zlatibor weekend: grilled meat and prawns on Friday's fire, sausages on
    /// Saturday's - with the syringe once for the trip - and noodles a bowl a child.
    fn grill() -> Menu {
        let mut m = Menu { on: true, days: 3, ..Menu::default() };
        for id in ["meat", "prawns", "sausage", "charcoal", "syringe", "noodles", "bags", "salt"] {
            m.products.push(product(id, if id == "noodles" { Eaters::Others } else { Eaters::Everyone }));
        }
        let dish = |id: &str, items: Vec<Ingredient>| Dish { methods: Default::default(), rest: Default::default(), id: id.into(), name: id.into(), meals: vec![Meal::Dinner], meal: None, ingredients: items };
        m.dishes = vec![
            dish("meat", vec![item("meat", 370.0, Per::Portion), item("charcoal", 3.0, Per::Fire)]),
            dish("prawns", vec![item("prawns", 100.0, Per::Portion), item("charcoal", 1.0, Per::Fire)]),
            dish("sausages", vec![item("sausage", 260.0, Per::Portion), item("syringe", 1.0, Per::Trip), item("charcoal", 2.0, Per::Fire)]),
            dish("lunch", vec![item("noodles", 1.0, Per::Head)]),
        ];
        m.daily = vec![item("bags", 0.2, Per::Day), item("salt", 1.0, Per::Trip)];
        m.plan = vec![
            Slot { day: 1, meal: Meal::Dinner, dishes: vec!["meat".into(), "prawns".into()] },
            Slot { day: 2, meal: Meal::Lunch, dishes: vec!["lunch".into()] },
            Slot { day: 2, meal: Meal::Dinner, dishes: vec!["sausages".into()] },
            Slot { day: 3, meal: Meal::Breakfast, dishes: vec!["lunch".into()] },
        ];
        m
    }

    /// One fire a meal - the largest of its dishes', 3 on Friday and 2 on Saturday; the
    /// syringe and the salt once; bags a day for three days; noodles a bowl a child, twice.
    #[test]
    fn a_fire_a_meal_a_head_a_trip_and_a_day() {
        let eating = Eating::of(&tour(&[100, 100, 50, 35, 200]));
        assert_eq!((eating.heads, eating.full_heads, eating.others_heads), (6.0, 4.0, 2.0));
        let m = grill();
        let needs = m.shopping(&eating);
        assert_eq!(amount(&needs, "charcoal"), 5.0);
        assert_eq!(amount(&needs, "syringe"), 1.0);
        assert_eq!(amount(&needs, "salt"), 1.0);
        assert!((amount(&needs, "bags") - 0.6).abs() < 1e-9);
        assert_eq!(amount(&needs, "noodles"), 4.0, "two children, two meals");
        assert!((amount(&needs, "meat") - 370.0 * 4.85).abs() < 1e-9);
        // Shopping a day at a time: the trip's salt on every day's list until it is ticked for
        // one - then on that day's only; the syringe on day 2's, its dish's.
        fn day<'a>(m: &'a Menu, e: &Eating, d: u32) -> Vec<Need<'a>> {
            m.shopping_on(e, &[d])
        }
        assert_eq!((amount(&day(&m, &eating, 1), "salt"), amount(&day(&m, &eating, 2), "salt")), (1.0, 1.0));
        assert_eq!(amount(&m.shopping_on(&eating, &[1, 2]), "salt"), 1.0, "once, however many days");
        assert_eq!((amount(&day(&m, &eating, 1), "syringe"), amount(&day(&m, &eating, 2), "syringe")), (0.0, 1.0));
        assert_eq!((amount(&day(&m, &eating, 1), "charcoal"), amount(&day(&m, &eating, 2), "charcoal")), (3.0, 2.0));
        assert_eq!(m.days_needing("salt"), [1, 2, 3]);
        let mut ticked = m.clone();
        ticked.set_bought_on("salt", &[2], true);
        assert_eq!(ticked.days_needing("salt"), [2]);
        assert_eq!((amount(&day(&ticked, &eating, 1), "salt"), amount(&day(&ticked, &eating, 2), "salt")), (0.0, 1.0));
        assert_eq!(m.days_needing("bags"), [1, 2, 3]);
    }

    /// The grill as a way of cooking: lit once a meal however many dishes are on it, twice
    /// for a grilled lunch and dinner; per portion when said; taken away, out of its dishes.
    #[test]
    fn a_way_of_cooking_is_lit_once_a_meal() {
        let eating = Eating::of(&tour(&[100, 100]));
        let mut m = grill();
        for d in &mut m.dishes {
            d.ingredients.retain(|i| i.per != Per::Fire);
            if d.id != "lunch" {
                d.methods = vec!["grill".into()];
            }
        }
        m.methods.push(Method {
            id: "grill".into(),
            name: "Grill".into(),
            ingredients: vec![item("charcoal", 2.5, Per::Fire), item("salt", 10.0, Per::Portion)],
            ..Method::default()
        });
        let needs = m.shopping(&eating);
        assert_eq!(amount(&needs, "charcoal"), 5.0, "Friday's dinner and Saturday's");
        assert_eq!(m.methods_for("charcoal", &m.all_days()).iter().map(|(x, n)| (x.id.as_str(), *n)).collect::<Vec<_>>(), [("grill", 2)]);
        assert!(m.uses("charcoal"));
        m.set(Slot { day: 2, meal: Meal::Lunch, dishes: vec!["meat".into()] });
        assert_eq!(amount(&m.shopping(&eating), "charcoal"), 7.5, "a grilled lunch is another fire");
        m.remove_method("grill");
        assert!(m.dishes.iter().all(|d| d.methods.is_empty()) && !m.uses("charcoal"));
    }

    /// Whole packs, with 5 % to spare, never none.
    #[test]
    fn packs_are_whole_with_five_per_cent_to_spare() {
        assert_eq!(packs(955.0, 1000.0), 1.0);
        assert_eq!(packs(1910.0, 1000.0), 2.0);
        assert_eq!(packs(13.0, 10.0), 2.0);
        assert_eq!(packs(4.01, 1.0), 4.0);
        assert_eq!(packs(4.3, 1.0), 5.0);
        assert_eq!(packs(0.2, 1.0), 1.0);
        assert_eq!(packs(1000.0, 1000.0), 1.0);
        assert_eq!(packs(0.0, 1000.0), 0.0);
        assert_eq!(packs(5.0, 0.0), 0.0);
    }

    /// A split-off line is taken off the rest; dropping the rest makes the last line the
    /// purchase. Nothing new is written where nothing was said, and what a later version
    /// wrote is kept.
    #[test]
    fn splits_and_fields_this_version_does_not_know() {
        let mut m = menu();
        let p = m.purchase_mut("rice");
        p.who = Some("sergey".into());
        p.spending = Some("e1".into());
        p.splits.push(Split { id: "s".into(), who: Some("sasha".into()), amount: 250.0, place: Some("From home".into()), bought: true, ..Split::default() });
        assert_eq!(p.split_off(), 250.0);
        let back: Menu = serde_json::from_value(serde_json::to_value(&m).expect("json")).expect("back");
        assert_eq!(back, m);
        let p = m.purchase_mut("rice");
        p.drop_rest();
        assert_eq!((p.who.as_deref(), p.place.as_deref(), p.bought, p.spending.as_deref()), (Some("sasha"), Some("From home"), true, None));
        assert!(p.splits.is_empty());

        let plain = serde_json::to_string(&menu()).expect("json");
        for field in ["\"Per\"", "\"Pack\"", "\"Note\"", "\"Splits\""] {
            assert!(!plain.contains(field), "{field}");
        }
        let mut later = serde_json::to_value(menu()).expect("json");
        later["Future"] = serde_json::json!(1);
        later["Products"][0]["Shelf"] = serde_json::json!("top");
        later["Dishes"][0]["Ingredients"][0]["Optional"] = serde_json::json!(true);
        let read: Menu = serde_json::from_value(later.clone()).expect("read");
        let again = serde_json::to_value(&read).expect("json");
        assert_eq!((&again["Future"], &again["Products"][0]["Shelf"], &again["Dishes"][0]["Ingredients"][0]["Optional"]),
                   (&later["Future"], &later["Products"][0]["Shelf"], &later["Dishes"][0]["Ingredients"][0]["Optional"]));
    }

    fn amount(needs: &[Need], id: &str) -> f64 {
        needs.iter().find(|n| n.product.id == id).map_or(0.0, |n| n.amount)
    }

    /// Two adults and a child at 50: 2.5 portions of dinner, wine for two, juice for half a
    /// child - by weight, as everything else.
    #[test]
    fn portions_are_weights_and_wine_is_for_the_full_weight() {
        let eating = Eating::of(&tour(&[100, 100, 50]));
        assert_eq!((eating.portions, eating.full, eating.others, eating.full_weight), (2.5, 2.0, 1.0, 100));
        assert_eq!((eating.full_portions, eating.others_portions), (2.0, 0.5));
        let m = menu();
        let needs = m.shopping(&eating);
        // Plov on day 1, steak on day 2: (200 + 300) × 2.5.
        assert_eq!(amount(&needs, "lamb"), 1250.0);
        assert_eq!(amount(&needs, "rice"), 250.0);
        assert_eq!(amount(&needs, "wine"), 350.0 * 2.0 * 2.0);
        assert_eq!(amount(&needs, "juice"), 500.0 * 2.0 * 0.5);
    }

    /// A couple written as one person at 200 drinks two people's wine.
    #[test]
    fn a_couple_at_200_is_two_portions_of_wine() {
        let eating = Eating::of(&tour(&[200, 100, 100]));
        assert_eq!((eating.full, eating.full_portions), (3.0, 4.0));
        assert_eq!(amount(&menu().shopping(&eating), "wine"), 350.0 * 2.0 * 4.0);
    }

    /// Everybody at 100: nobody below the full weight, so no juice on the list at all.
    #[test]
    fn nobody_below_the_full_weight_means_no_juice() {
        let m = menu();
        let needs = m.shopping(&Eating::of(&tour(&[100, 100])));
        assert!(needs.iter().all(|n| n.product.id != "juice"));
    }

    /// Mostly children: still children. The grown-ups are the two at 100 - the most common
    /// weight, 35, made all five of them grown-ups, wine and all.
    #[test]
    fn grown_ups_are_100_and_over_however_many_children() {
        let eating = Eating::of(&tour(&[35, 35, 35, 100, 100]));
        assert_eq!((eating.full_weight, eating.full, eating.others), (100, 2.0, 3.0));
        assert!((eating.others_portions - 1.05).abs() < 1e-9);
    }

    #[test]
    fn dinners_take_turns_and_an_emptied_one_stays_empty() {
        let mut m = menu();
        let dinners = |m: &Menu| (1..=m.days).map(|d| m.dish_on(d, Meal::Dinner).map(|x| x.id.clone())).collect::<Vec<_>>();
        assert_eq!(dinners(&m), [Some("plov".into()), Some("steak".into())]);
        // A middle day's: on the last day an emptied dinner is where the trip ends, and a
        // day more moves it on (see `set_days`).
        m.set_days(3);
        m.set(Slot::one(2, Meal::Dinner, None));
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
        m.set(Slot::one(1, Meal::Dinner, Some("steak".into())));
        assert!(m.sources("rice").0.is_empty());
        // Steak on days 1 and 2: twice over the trip, once on day 2.
        assert_eq!(m.sources("lamb").0.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [2]);
        assert_eq!(m.sources_on("lamb", &[2]).0.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [1]);
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
        assert_eq!(m.slot(1, Meal::Dinner).map(|s| s.dishes.clone()), Some(Vec::new()));
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
        m.dishes.push(Dish { methods: Default::default(), rest: Default::default(),
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
        assert_eq!(amount(&needs, "juice"), 500.0 * 3.0 * 0.5, "a child at 50, three days");
        // A fourth day: Sunday is a whole day now, and the departure is Monday's.
        m.set_days(4);
        assert!(m.dish_on(3, Meal::Dinner).is_some() && m.dish_on(4, Meal::Dinner).is_none());
        assert_eq!(m.departure(), Meal::Breakfast);
        // Arriving for breakfast after all fills the first day's breakfast again.
        m.arrive(Meal::Breakfast);
        assert!(m.dish_on(1, Meal::Breakfast).is_some());
    }

    /// With no dishes yet there is nothing to cook, but where the trip starts and ends is
    /// still what was chosen.
    #[test]
    fn arrival_and_departure_stay_with_no_dishes_yet() {
        let mut m = Menu::default();
        m.set_days(3);
        m.arrive(Meal::Dinner);
        m.leave(Meal::Breakfast);
        assert_eq!((m.arrival(), m.departure()), (Meal::Dinner, Meal::Breakfast));
        let back: Menu = serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!((back.arrival(), back.departure()), (Meal::Dinner, Meal::Breakfast));
        // A day longer, the departure moves to the new last day.
        m.set_days(4);
        assert_eq!((m.arrival(), m.departure()), (Meal::Dinner, Meal::Breakfast));
        assert!(!m.is_nothing(3, Meal::Dinner));
    }

    /// "When" is written only for a daily item that has one: a dish's ingredients and an
    /// every-day item stay as they were.
    #[test]
    fn when_is_written_only_when_it_says_something() {
        let plain = serde_json::to_value(Ingredient { product: "rice".into(), amount: 1.0, ..Ingredient::default() }).unwrap();
        assert!(plain.get("When").is_none());
        let wine = Ingredient { product: "wine".into(), amount: 1.0, when: When::With(Meal::Dinner), ..Ingredient::default() };
        let back: Ingredient = serde_json::from_value(serde_json::to_value(&wine).unwrap()).unwrap();
        assert_eq!(back, wine);
    }

    #[test]
    fn a_place_something_is_bought_at_stays_and_categories_come_once() {
        let mut m = menu();
        m.put_place(Place { id: "shop".into(), name: "Shop".into(), ..Place::default() });
        m.put_place(Place { id: "shop".into(), name: "Corner shop".into(), ..Place::default() });
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

    /// A copy is the dish under a new name, right after it, planned nowhere; replayed, it is
    /// still one copy.
    #[test]
    fn a_copied_dish_sits_next_to_its_original() {
        let mut m = menu();
        m.copy_dish("plov", "plov2", "Plov (copy)");
        let ids: Vec<&str> = m.dishes.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["plov", "plov2", "steak"]);
        assert_eq!(m.dishes[1].name, "Plov (copy)");
        assert_eq!(m.dishes[1].ingredients, m.dishes[0].ingredients);
        assert!(m.plan.iter().all(|s| !s.dishes.iter().any(|d| d == "plov2")));
        m.copy_dish("plov", "plov2", "Plov (copy)");
        m.copy_dish("nothing", "x", "x");
        assert_eq!(m.dishes.len(), 3, "a replay or a missing original adds nothing");
    }

    /// A meal with several dishes buys for each of them, and says each is where a product
    /// goes.
    #[test]
    fn every_dish_of_a_meal_is_bought_for() {
        let mut m = menu();
        m.set(Slot { day: 1, meal: Meal::Dinner, dishes: vec!["plov".into(), "steak".into()] });
        let needs = m.shopping(&Eating::of(&tour(&[100])));
        // Day 1 plov (200) and steak (300), day 2 steak (300): lamb for one portion each.
        assert_eq!(amount(&needs, "lamb"), 800.0);
        let (sources, _) = m.sources("lamb");
        let names: Vec<(&str, u32)> = sources.iter().map(|(d, n)| (d.id.as_str(), *n)).collect();
        assert_eq!(names, vec![("plov", 1), ("steak", 2)]);
        // Deleting one leaves the other on that dinner.
        m.remove_dish("plov");
        assert_eq!(m.slot(1, Meal::Dinner).map(|s| s.dishes.clone()), Some(vec!["steak".to_owned()]));
    }

    /// The first dish is written where a client that knows one dish a meal reads it; the
    /// rest beside it. What such a client writes back - `Dish` alone - reads as that dish.
    #[test]
    fn a_meal_is_stored_so_that_one_dish_clients_still_read_it() {
        let several = Slot { day: 2, meal: Meal::Dinner, dishes: vec!["plov".into(), "salad".into()] };
        let json = serde_json::to_value(&several).expect("json");
        assert_eq!(json["Dish"], "plov");
        assert_eq!(json["Dishes"], serde_json::json!(["plov", "salad"]));
        assert_eq!(serde_json::from_value::<Slot>(json).expect("reads"), several);

        let one = serde_json::to_value(Slot::one(1, Meal::Lunch, Some("soup".into()))).expect("json");
        assert!(one.get("Dishes").is_none(), "one dish is written the old way");
        let old: Slot = serde_json::from_value(serde_json::json!({"Day": 1, "Meal": "Lunch", "Dish": "soup"})).expect("old");
        assert_eq!(old.dishes, vec!["soup".to_owned()]);
        let nothing: Slot = serde_json::from_value(serde_json::json!({"Day": 1, "Meal": "Lunch", "Dish": null})).expect("nothing");
        assert!(nothing.dishes.is_empty());
    }

    /// A day's word is set, read, taken away; the list does not grow with empty tails.
    #[test]
    fn a_day_has_a_word_of_its_own() {
        let mut m = menu();
        m.set_day_note(3, " leaving ");
        assert_eq!(m.day_note(3), Some("leaving"));
        assert_eq!((m.day_note(1), m.day_note(0), m.day_note(9)), (None, None, None));
        m.set_day_note(1, "arrival");
        m.set_day_note(3, "");
        assert_eq!(m.day_notes, vec!["arrival".to_owned()]);
        assert!(m.as_template().day_notes.is_empty(), "a template is a catalogue, not a trip");
    }

    #[test]
    fn a_purchase_says_where_it_is_bought_and_its_own_place_is_none() {
        let mut m = menu();
        let lamb = m.products[0].clone();
        assert_eq!(m.bought_at(&lamb), "Market");
        m.set_bought_at(&["lamb".into(), "rice".into()], " Lidl ");
        assert_eq!(m.bought_at(&lamb), "Lidl");
        assert_eq!(m.shop_places(), vec!["Market".to_owned(), "Lidl".to_owned()]);
        // The product's own place, in any case, is no place of the purchase's own.
        m.set_bought_at(&["lamb".into()], "market");
        assert_eq!(m.purchase("lamb").and_then(|p| p.place.clone()), None);
        m.set_bought_at(&["rice".into()], "");
        assert_eq!(m.purchase("rice").and_then(|p| p.place.clone()), None);
        // Nor is it the catalogue's: the place list stays as it was.
        assert_eq!(m.places.len(), 1);
    }

    #[test]
    fn wine_is_paid_by_the_grown_ups_and_juice_by_the_children() {
        let t = tour(&[100, 100, 50]);
        assert_eq!(payers_for(&t, Eaters::Everyone), None);
        let ids = |v: Option<Vec<crate::PersonId>>| v.unwrap().iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>();
        assert_eq!(ids(payers_for(&t, Eaters::FullWeight)), vec!["p0", "p1"]);
        assert_eq!(ids(payers_for(&t, Eaters::Others)), vec!["p2"]);
    }

    #[test]
    fn an_expense_goes_under_the_shopping_and_its_products_category() {
        let mut m = menu();
        let mut wine = m.products[2].clone();
        wine.category = Some("Alcohol".into());
        let mut beer = wine.clone();
        beer.id = "beer".into();
        let mut rice = m.products[1].clone();
        rice.category = Some("Groceries".into());
        assert_eq!(m.expense_category("Shopping", &[&wine]), "Shopping/Alcohol");
        assert_eq!(m.expense_category("Shopping", &[&wine, &beer]), "Shopping/Alcohol");
        assert_eq!(m.expense_category("Shopping", &[&wine, &rice]), "Shopping", "two categories, the root alone");
        assert_eq!(m.expense_category("Shopping", &[&m.products[0].clone()]), "Shopping", "no category of its own");
        m.expense_category = Some(" Food ".into());
        assert_eq!(m.expense_category("Shopping", &[&wine]), "Food/Alcohol");
        m.expense_category = Some("  ".into());
        assert_eq!(m.expense_category("Shopping", &[&wine]), "Shopping/Alcohol", "blank is the default");
    }

    /// Bought for day 1 only: day 1's share is covered, day 2's still to buy; recorded, the
    /// tick gives way to the expense; unticked, the part goes.
    #[test]
    fn a_product_is_bought_a_day_at_a_time() {
        let eating = Eating::of(&tour(&[100, 100, 50]));
        let mut m = menu();
        // Plov on day 1, steak on day 2: 200 and 300 of lamb a portion.
        assert_eq!(amount(&m.shopping_on(&eating, &[1]), "lamb"), 200.0 * 2.5);
        assert_eq!(amount(&m.shopping_on(&eating, &[2]), "lamb"), 300.0 * 2.5);
        assert_eq!(amount(&m.shopping_on(&eating, &[1, 2]), "lamb"), amount(&m.shopping(&eating), "lamb"));
        assert_eq!(m.days_needing("lamb"), vec![1, 2]);

        // Ticked twice for day 1: one day.
        m.set_bought_on("lamb", &[1], true);
        m.set_bought_on("lamb", &[1], true);
        assert_eq!(m.covered_days("lamb"), vec![1]);

        // Recorded for day 2: an expense, not a tick.
        let all = |_: &str| true;
        m.record_on("lamb", &[2], "s1", all);
        assert_eq!(m.covered_days("lamb"), vec![1]);
        assert_eq!(m.recorded_days("lamb", all), vec![2]);
        assert_eq!(m.recorded_days("lamb", |s| s != "s1"), Vec::<u32>::new(), "its expense deleted");

        // Unticked: the expense stays.
        m.set_bought_on("lamb", &[1, 2], false);
        assert!(m.covered_days("lamb").is_empty());
        assert_eq!(m.recorded_days("lamb", all), vec![2]);

        // Bought for the whole trip covers every day.
        m.purchase_mut("rice").bought = true;
        assert_eq!(m.covered_days("rice"), vec![1, 2]);

        // Ticked for the whole trip, then not for day 2: day 1 stays ticked, the whole trip's
        // expense where it was.
        let p = m.purchase_mut("lamb");
        (p.bought, p.spending) = (true, Some("s9".into()));
        m.set_bought_on("lamb", &[2], false);
        let p = m.purchase("lamb").expect("lamb");
        assert!(!p.bought);
        assert_eq!(p.spending.as_deref(), Some("s9"));
        assert_eq!(m.covered_days("lamb"), vec![1]);

        // A part recorded again for its days, its expense gone: one part, not two.
        m.record_on("lamb", &[2], "s2", |s| s != "s1");
        assert_eq!(m.purchase("lamb").expect("lamb").parts, vec![Part { days: vec![2], spending: Some("s2".into()) }]);
    }

    /// What a beta of 4 October wrote - a part with no expense, which was a tick - is read as
    /// ticked days; a part with one stays an expense.
    #[test]
    fn an_unrecorded_part_is_read_as_ticked_days() {
        let mut t = tour(&[100]);
        let mut m = menu();
        m.purchase_mut("lamb").parts = vec![
            Part { days: vec![1], spending: None },
            Part { days: vec![2], spending: Some("s1".into()) },
        ];
        m.put(&mut t);
        let back = Menu::of(&t).expect("menu");
        let p = back.purchase("lamb").expect("lamb");
        assert_eq!(p.bought_days, vec![1]);
        assert_eq!(p.parts, vec![Part { days: vec![2], spending: Some("s1".into()) }]);
    }
}
