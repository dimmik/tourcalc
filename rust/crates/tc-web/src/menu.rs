//! The Menu tab: what is cooked on which day, and the shopping it takes.
//!
//! The model and the arithmetic are `tc_core::menu`; this is the starter catalogue a tour
//! gets when its menu is first switched on, and the two screens - the plan by days, and
//! the shopping by places, each with whoever does it, a tick per product, and the expense
//! it was recorded as.

use crate::edit::{MenuEdit, SpendingDraft, SplitChange};
use crate::i18n::{t, Lang};
use crate::queue::Operation;
use crate::tour::Dialog;
use crate::ui::money_in;
use leptos::prelude::*;
use tc_core::menu::{Dish, Eaters, Eating, Ingredient, Meal, Menu, Place, Product, Slot, Split, Unit, When};
use tc_core::{PersonId, Tour};

/// Which of the tab's three views is open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Plan,
    Shopping,
    Catalogue,
}

impl View {
    /// How the view is named at the end of an address: `/tour/x/menu/shopping`, so that a
    /// link to the shopping opens the shopping. The catalogue is the Menu on screen, and its
    /// dishes in the address - `/menu/menu` reads like a typo.
    pub fn part(self) -> &'static str {
        match self {
            View::Plan => "plan",
            View::Shopping => "shopping",
            View::Catalogue => "dishes",
        }
    }

    pub fn of_part(part: &str) -> Option<View> {
        [View::Plan, View::Shopping, View::Catalogue].into_iter().find(|v| v.part().eq_ignore_ascii_case(part))
    }
}

/// Text as a search compares it: case and "ё" aside.
pub(crate) fn folded(text: &str) -> String {
    text.to_lowercase().replace('ё', "е")
}

/// Whether every word typed is somewhere in `hay` (already [`folded`]); nothing typed is
/// everything.
pub(crate) fn found(query: &str, hay: &str) -> bool {
    folded(query).split_whitespace().all(|word| hay.contains(word))
}

/// A search box, as the People tab has it.
pub(crate) fn search_box(text: RwSignal<String>, placeholder: &'static str) -> impl IntoView {
    view! {
        <div class="tcn-search tcw-food-search">
            <span class="tcn-search-icon">"🔎"</span>
            <input type="text" placeholder=placeholder
                   prop:value=move || text.get()
                   on:input=move |ev| text.set(event_target_value(&ev)) />
            <Show when=move || !text.get().is_empty()>
                <button type="button" class="tcn-search-clear" title=t().people.clear
                        on:click=move |_| text.set(String::new())>"✕"</button>
            </Show>
        </div>
    }
}

/// How the shopping list is split into cards.
#[derive(Clone, Copy, PartialEq)]
pub enum Group {
    None,
    Place,
    Category,
    /// A card per buyer, and one of what nobody has taken yet.
    Person,
}

impl Group {
    const ALL: [Group; 4] = [Group::None, Group::Place, Group::Category, Group::Person];

    fn name(self) -> &'static str {
        match self {
            Group::None => t().menu.group_none,
            Group::Place => t().menu.group_place,
            Group::Category => t().menu.group_category,
            Group::Person => t().menu.group_person,
        }
    }
}

/// The order of the shopping list.
#[derive(Clone, Copy, PartialEq)]
pub enum Sort {
    /// As the catalogue has them: the meat together, the dairy together.
    Catalogue,
    Name,
    Place,
    Buyer,
    Category,
    /// What is still to buy on top.
    NotBought,
}

impl Sort {
    const ALL: [Sort; 6] = [Sort::Catalogue, Sort::Name, Sort::Place, Sort::Category, Sort::Buyer, Sort::NotBought];

    fn name(self) -> &'static str {
        let m = &t().menu;
        match self {
            Sort::Catalogue => m.sort_catalogue,
            Sort::Name => m.sort_name,
            Sort::Place => m.sort_place,
            Sort::Buyer => m.sort_buyer,
            Sort::Category => m.sort_category,
            Sort::NotBought => m.sort_not_bought,
        }
    }
}

/// What is open on the tab, owned by the page so that a save does not reset it.
#[derive(Clone, Copy)]
pub struct MenuState {
    pub view: RwSignal<View>,
    /// The dish, product or daily list open for editing in the catalogue: its id, "" for a
    /// new one, `None` for nothing.
    pub editing: RwSignal<Option<String>>,
    /// Which of the catalogue's sections are open. The products - thirty-odd lines looked
    /// up now and then - start folded.
    pub dishes_open: RwSignal<bool>,
    pub sort: RwSignal<Sort>,
    /// What is typed into the search boxes of the shopping and of the catalogue.
    pub shop_find: RwSignal<String>,
    pub cat_find: RwSignal<String>,
    /// The shopping in one list, or a card per place or per category.
    pub group: RwSignal<Group>,
    pub daily_open: RwSignal<bool>,
    pub products_open: RwSignal<bool>,
    pub places_open: RwSignal<bool>,
    pub methods_open: RwSignal<bool>,
    /// Who this phone is: the shopping shows theirs.
    pub me: RwSignal<Option<String>>,
    /// Other people's shopping too, although somebody is picked. Without it the shopping
    /// shows theirs and nobody's - nobody's so that there is something to take.
    pub everybody: RwSignal<bool>,
    /// What is ticked as bought, hidden: what is left to buy, without the struck-out lines.
    /// Remembered on the device for the tour (`settings::hide_bought`) when ticked or unticked
    /// ([`MenuState::set_hide_bought`]); a shopping link's `bought=` sets it for this time only.
    pub hide_bought: RwSignal<bool>,
    /// Whose state this is - the tour the remembered settings are kept under.
    tour: StoredValue<String>,
    /// The shopping's sort and grouping opened from their one-line summary.
    pub arrange_open: RwSignal<bool>,
    /// The days the shopping is for, when only some - Friday's market before the trip, the
    /// rest on Saturday. Empty: the whole trip.
    pub days: RwSignal<Vec<u32>>,
}

impl MenuState {
    pub fn new(tour: &str) -> MenuState {
        MenuState {
            view: RwSignal::new(View::Plan),
            editing: RwSignal::new(None),
            dishes_open: RwSignal::new(true),
            sort: RwSignal::new(Sort::Catalogue),
            shop_find: RwSignal::new(String::new()),
            cat_find: RwSignal::new(String::new()),
            group: RwSignal::new(Group::None),
            daily_open: RwSignal::new(true),
            products_open: RwSignal::new(false),
            places_open: RwSignal::new(false),
            methods_open: RwSignal::new(true),
            me: RwSignal::new(crate::settings::me(tour)),
            everybody: RwSignal::new(false),
            hide_bought: RwSignal::new(crate::settings::hide_bought(tour)),
            tour: StoredValue::new(tour.to_owned()),
            arrange_open: RwSignal::new(false),
            days: RwSignal::new(Vec::new()),
        }
    }

    /// "Hide what is bought" ticked or unticked: shown so, and remembered for the tour.
    pub fn set_hide_bought(&self, hide: bool) {
        crate::settings::remember_hide_bought(&self.tour.get_value(), hide);
        self.hide_bought.set(hide);
    }

    /// What a shopping link asks (`?buyer=<person>&others=0|1&bought=0|1`, see
    /// [`shopping_link`]): whose shopping, kept as this phone's "Shopping by" as if picked
    /// there - the link is handed to the one who shops - whether the others' lines show, and
    /// whether what is bought does. Whether it asked anything.
    pub fn take_link(&self, tour: &str, search: &str) -> bool {
        let Asks { buyer, others, bought } = link_asks(search);
        if let Some(buyer) = &buyer {
            crate::settings::set_me(tour, Some(buyer));
            self.me.set(Some(buyer.clone()));
            self.view.set(View::Shopping);
        }
        if let Some(others) = others {
            self.everybody.set(others);
        }
        if let Some(bought) = bought {
            self.hide_bought.set(!bought);
        }
        buyer.is_some() || others.is_some() || bought.is_some()
    }
}

/// The address that opens this tour's shopping with `buyer` picked, and the others' lines
/// and the bought ones shown or hidden: the tour's share link (it signs in whoever opens it)
/// with the view and the three on the end.
pub(crate) fn shopping_link(code: &str, tour: &str, buyer: &str, others: bool, bought: bool) -> String {
    format!(
        "/goto/{code}/{tour}/menu/{}?buyer={}&others={}&bought={}",
        View::Shopping.part(),
        escaped(buyer),
        u8::from(others),
        u8::from(bought)
    )
}

/// What a shopping link's query names: the buyer, whether the others' lines show, whether
/// the bought ones do - each if it does.
#[derive(Debug, Default, PartialEq)]
struct Asks {
    buyer: Option<String>,
    others: Option<bool>,
    bought: Option<bool>,
}

/// Text as it can stand in a query: letters, digits and `-_.~` as they are, every other
/// byte as `%XX`. By hand rather than through the browser's `encodeURIComponent`, so that
/// the tests run off the browser too.
fn escaped(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// [`escaped`] undone - and a `+` as a space, as forms send it.
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a shopping link's query asks - see [`Asks`].
fn link_asks(search: &str) -> Asks {
    let mut asks = Asks::default();
    for pair in search.trim_start_matches('?').split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "buyer" if !value.is_empty() => asks.buyer = Some(unescaped(value)),
            "others" => asks.others = Some(value == "1"),
            "bought" => asks.bought = Some(value == "1"),
            _ => {}
        }
    }
    asks
}

// ---- the starter catalogue ------------------------------------------------------------

const MARKET: &str = "market";
const SUPERMARKET: &str = "supermarket";
const DELIVERY: &str = "delivery";

/// Where a product typed straight into a dish is bought, until somebody moves it in
/// Products. The supermarket: chosen on 30.09 as a start, to be settled with the group - to
/// change it, change this.
pub const NEW_PRODUCT_PLACE: &str = SUPERMARKET;

/// The place a new product typed into a dish gets in this menu: [`NEW_PRODUCT_PLACE`] - by
/// its id, or, in a menu that made its own, by its name in either language - else the menu's
/// first place, else none.
pub fn place_for_new(menu: &Menu) -> String {
    if menu.places.iter().any(|p| p.id == NEW_PRODUCT_PLACE) {
        return NEW_PRODUCT_PLACE.to_owned();
    }
    let names: Vec<String> = PLACES
        .iter()
        .filter(|(id, ..)| *id == NEW_PRODUCT_PLACE)
        .flat_map(|(_, en, ru)| [en.to_lowercase(), ru.to_lowercase()])
        .collect();
    menu.places
        .iter()
        .find(|p| names.contains(&p.name.trim().to_lowercase()))
        .or(menu.places.first())
        .map(|p| p.id.clone())
        .unwrap_or_default()
}

/// Both languages side by side: it is data, written into the tour in the language of
/// whoever switches the menu on, and a pair per row keeps the two from drifting apart.
const PLACES: &[(&str, &str, &str)] = &[
    (MARKET, "Market", "Рынок"),
    (SUPERMARKET, "Supermarket", "Супермаркет"),
    (DELIVERY, "Delivery", "Доставка"),
];

use Eaters::{Everyone as ALL, FullWeight as FULL, Others as KIDS};
use Unit::{Gram as G, Millilitre as ML, Piece as PCS};

/// id, English, Russian, unit, where, for whom, and a unit of its own in both languages.
///
/// Taken on 30.09.2026 from a tour the group calibrated by hand ("Август на Дунае"): the
/// amounts, the units and what is in it are theirs.
type ProductRow = (&'static str, &'static str, &'static str, Unit, &'static str, Eaters, Option<(&'static str, &'static str)>);
const PRODUCTS: &[ProductRow] = &[
    ("meat", "Meat for the grill", "Мясо на гриль", G, MARKET, ALL, None),
    ("lamb", "Lamb", "Баранина", G, MARKET, ALL, None),
    ("fish", "Fish", "Рыба", G, MARKET, ALL, None),
    ("mince", "Mince", "Фарш", G, MARKET, ALL, None),
    ("potatoes", "Potatoes", "Картошка", G, MARKET, ALL, None),
    ("carrots", "Carrots", "Морковь", G, MARKET, ALL, None),
    ("onions", "Onions", "Лук", G, MARKET, ALL, None),
    ("salad", "Salad vegetables", "Овощи на салат", G, MARKET, ALL, None),
    ("herbs", "Herbs", "Зелень", G, MARKET, ALL, None),
    ("fruit", "Fruit", "Фрукты", G, MARKET, ALL, None),
    ("lemons", "Lemons", "Лимоны", PCS, MARKET, ALL, None),
    ("cheese", "Cheese", "Сыр", G, MARKET, ALL, None),
    ("rice", "Rice", "Рис", G, SUPERMARKET, ALL, None),
    ("pasta", "Pasta", "Макароны", G, SUPERMARKET, ALL, None),
    ("oats", "Oats", "Овсянка", G, SUPERMARKET, ALL, None),
    ("muesli", "Muesli", "Мюсли", G, SUPERMARKET, ALL, None),
    ("eggs", "Eggs", "Яйца", PCS, SUPERMARKET, ALL, None),
    ("milk", "Milk", "Молоко", ML, SUPERMARKET, ALL, None),
    ("yoghurt", "Yoghurt", "Йогурт", G, SUPERMARKET, ALL, None),
    ("butter", "Butter", "Сливочное масло", G, SUPERMARKET, ALL, None),
    ("oil", "Oil", "Растительное масло", ML, SUPERMARKET, ALL, None),
    ("bread", "Bread", "Хлеб", G, SUPERMARKET, ALL, None),
    ("ham", "Ham, sausage, pečenica", "Ветчина, колбаса, печеница", G, SUPERMARKET, ALL, None),
    ("nuts", "Nuts", "Орехи", G, SUPERMARKET, ALL, None),
    ("snacks", "Snacks - olives, pickles", "Закуски - Оливки, соленья", G, SUPERMARKET, ALL, None),
    ("coffee", "Coffee and tea", "Кофе и чай", G, SUPERMARKET, ALL, None),
    ("wine", "Wine", "Вино", PCS, SUPERMARKET, FULL, Some(("btl", "бут."))),
    ("juice", "Juice", "Сок", PCS, SUPERMARKET, KIDS, Some(("pack", "пакет"))),
    ("mineral", "Mineral water", "Минералка", PCS, DELIVERY, KIDS, Some(("btl", "бут."))),
    ("water", "Drinking water", "Питьевая вода", PCS, DELIVERY, ALL, Some(("5 l jug", "5л фляга"))),
    ("beer", "Beer", "Пиво", PCS, DELIVERY, FULL, Some(("can", "банка"))),
];

/// The starter's categories, by product - the aisles a list is usually read in.
fn starter_category(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "meat" | "lamb" | "fish" | "mince" | "ham" => ("Meat and fish", "Мясо и рыба"),
        "potatoes" | "carrots" | "onions" | "salad" | "herbs" | "fruit" | "lemons" => ("Vegetables and fruit", "Овощи и фрукты"),
        "cheese" | "eggs" | "milk" | "yoghurt" | "butter" => ("Dairy and eggs", "Молочное и яйца"),
        "rice" | "pasta" | "oats" | "muesli" | "oil" | "nuts" | "snacks" | "coffee" => ("Groceries", "Бакалея"),
        "bread" => ("Bread", "Хлеб"),
        "juice" | "mineral" | "water" => ("Drinks", "Напитки"),
        "wine" | "beer" => ("Alcohol", "Алкоголь"),
        _ => return None,
    })
}

/// id, English, Russian, meals, and per portion: product and amount.
type DishRow = (&'static str, &'static str, &'static str, &'static [Meal], &'static [(&'static str, f64)]);
const DISHES: &[DishRow] = &[
    ("omelette", "Omelette", "Омлет", &[Meal::Breakfast], &[("eggs", 3.0), ("milk", 50.0), ("butter", 10.0)]),
    ("porridge", "Porridge", "Каша", &[Meal::Breakfast], &[("oats", 60.0), ("milk", 200.0), ("butter", 10.0)]),
    ("yoghurt", "Yoghurt and muesli", "Йогурт с мюсли", &[Meal::Breakfast], &[("yoghurt", 200.0), ("muesli", 50.0), ("fruit", 100.0)]),
    ("sandwiches", "Sandwiches", "Бутерброды", &[Meal::Breakfast, Meal::Lunch], &[("bread", 100.0), ("cheese", 50.0), ("ham", 60.0), ("butter", 10.0)]),
    ("grill", "Grilled meat", "Быстромясо на гриле", &[Meal::Dinner], &[("meat", 300.0), ("potatoes", 200.0)]),
    ("lamb_steaks", "Lamb steaks", "Стейки баранины", &[Meal::Dinner], &[("lamb", 300.0), ("potatoes", 200.0)]),
    ("fish", "Fish", "Рыба", &[Meal::Dinner], &[("fish", 300.0), ("rice", 70.0), ("lemons", 0.25)]),
    ("plov", "Plov", "Плов", &[Meal::Dinner], &[("rice", 100.0), ("lamb", 150.0), ("carrots", 120.0), ("onions", 60.0), ("oil", 20.0)]),
    // Eating out: nothing to buy, and never put on the plan by itself - see `Menu::fill`.
    ("restaurant", "Restaurant", "Ресторан", &[Meal::Breakfast, Meal::Lunch, Meal::Dinner], &[]),
];

/// Every day, whatever is cooked: per portion, or per head for the adults' and the
/// children's drinks. Wine with dinner - arriving for Friday dinner and leaving after
/// Sunday breakfast is two evenings of it.
const DAILY: &[(&str, f64, When)] = &[
    ("salad", 150.0, When::With(Meal::Dinner)),
    ("herbs", 20.0, When::With(Meal::Breakfast)),
    ("fruit", 150.0, When::With(Meal::Breakfast)),
    ("bread", 60.0, When::EveryDay),
    ("snacks", 40.0, When::With(Meal::Dinner)),
    ("coffee", 15.0, When::With(Meal::Breakfast)),
    ("water", 0.2, When::EveryDay),
    ("wine", 1.0, When::With(Meal::Dinner)),
    ("juice", 1.0, When::With(Meal::Dinner)),
    ("mineral", 1.0, When::With(Meal::Dinner)),
    ("beer", 2.0, When::With(Meal::Dinner)),
];

/// The menu a tour starts with: three places, the usual products and dishes, and a plan
/// that takes the dishes in turn - every amount and every dish there to be changed.
pub fn starter(days: u32) -> Menu {
    let ru = crate::i18n::lang() == Lang::Ru;
    let pick = |en: &str, ru_: &str| if ru { ru_.to_owned() } else { en.to_owned() };
    let mut menu = Menu { methods: Default::default(), rest: Default::default(), arrive: None, leave: None,
        on: true,
        days: days.max(1),
        start: None,
        places: PLACES
            .iter()
            .map(|(id, en, r)| Place { id: (*id).into(), name: pick(en, r), ..Place::default() })
            .collect(),
        products: PRODUCTS
            .iter()
            .map(|(id, en, r, unit, place, eaters, own)| Product {
                id: (*id).into(),
                name: pick(en, r),
                unit: *unit,
                place: (*place).into(),
                eaters: *eaters,
                own_unit: own.map(|(en, r)| pick(en, r)),
                category: starter_category(id).map(|(en, r)| pick(en, r)),
                ..Product::default()
            })
            .collect(),
        dishes: DISHES
            .iter()
            .map(|(id, en, r, meals, items)| Dish { methods: Default::default(), rest: Default::default(),
                id: (*id).into(),
                name: pick(en, r),
                meals: meals.to_vec(),
                meal: None,
                ingredients: items
                    .iter()
                    .map(|(product, amount)| Ingredient { product: (*product).into(), amount: *amount, ..Ingredient::default() })
                    .collect(),
            })
            .collect(),
        daily: DAILY
            .iter()
            .map(|(product, amount, when)| Ingredient { product: (*product).into(), amount: *amount, when: *when, ..Ingredient::default() })
            .collect(),
        plan: Vec::new(),
        purchases: Vec::new(),
        day_notes: Vec::new(),
        expense_category: None,
    };
    menu.fill();
    menu
}

// ---- amounts -----------------------------------------------------------------------------

/// So much of a product, as a shop sells it: grams to 5 under 100 and to 10 above,
/// kilograms and litres to a tenth, pieces whole - to the nearest, but never more than 5 %
/// short, since the point is to have enough: 10.1 g of salt (2 g a portion, 5.05 portions)
/// is 10 g, not 20.
pub fn quantity_of(amount: f64, product: &Product) -> String {
    match product.own_unit.as_deref().filter(|u| !u.trim().is_empty()) {
        Some(own) => format!("{}\u{a0}{}", to_step(amount, 1.0), own.trim()),
        None => quantity(amount, product.unit),
    }
}

/// So much of a product as it is bought: whole packs, if it comes in them - "2 × 1 l" for
/// 1.9 l of milk, "2 × stack of 10" for 13 lavash - else as [`quantity_of`] says.
pub fn to_buy(amount: f64, product: &Product) -> String {
    match product.pack_size() {
        Some(size) => {
            let name = product
                .pack_name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| quantity_of(size, product));
            (t().menu.pack_times)(tc_core::menu::packs(amount, size), &name)
        }
        None => quantity_of(amount, product),
    }
}

/// To the nearest step, but never short by more than 5 %: 10.1 g is 10 g - that 0.1 is the
/// noise of weights like 35, and rounding it up would add a whole step, doubling 10 g - while
/// 348 g is still 350.
fn to_step(amount: f64, step: f64) -> f64 {
    let below = (amount / step).floor() * step;
    if below > 0.0 && amount - below <= (amount * 0.05).min(step / 2.0) {
        below
    } else {
        (amount / step).ceil() * step
    }
}

pub fn quantity(amount: f64, unit: Unit) -> String {
    let m = &t().menu;
    let tenths = |x: f64| {
        let v = to_step(x, 0.1);
        let text = if (v * 10.0).round() % 10.0 == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
        text.replace('.', &crate::ui::decimal().to_string())
    };
    let grams = |x: f64| to_step(x, if x < 100.0 { 5.0 } else { 10.0 });
    match unit {
        Unit::Piece => format!("{}\u{a0}{}", to_step(amount, 1.0), m.pieces),
        Unit::Gram if amount >= 1000.0 => format!("{}\u{a0}{}", tenths(amount / 1000.0), m.kilograms),
        Unit::Millilitre if amount >= 1000.0 => format!("{}\u{a0}{}", tenths(amount / 1000.0), m.litres),
        Unit::Gram => format!("{}\u{a0}{}", grams(amount), m.grams),
        Unit::Millilitre => format!("{}\u{a0}{}", grams(amount), m.millilitres),
    }
}

/// "every day", "with dinner" - which days a daily item is bought for.
pub(crate) fn when_name(when: When) -> &'static str {
    match when {
        When::EveryDay => t().menu.when_every_day,
        When::With(Meal::Breakfast) => t().menu.when_breakfast,
        When::With(Meal::Lunch) => t().menu.when_lunch,
        When::With(Meal::Dinner) => t().menu.when_dinner,
    }
}

/// "with breakfast and dinner" - the meals a row of the daily list is bought with, as
/// [`tc_core::menu::daily_groups`] puts them together.
pub(crate) fn whens_name(whens: &[When]) -> &'static str {
    let has = |m: Meal| whens.contains(&When::With(m));
    match (has(Meal::Breakfast), has(Meal::Lunch), has(Meal::Dinner)) {
        (true, true, true) => t().menu.when_every_meal,
        (true, true, false) => t().menu.when_breakfast_lunch,
        (true, false, true) => t().menu.when_breakfast_dinner,
        (false, true, true) => t().menu.when_lunch_dinner,
        _ => whens.first().map_or(t().menu.when_every_day, |w| when_name(*w)),
    }
}

/// The date so many days after "2026-11-13", as (year, month 1-12, day). Plain arithmetic
/// on the civil calendar - the browser's clock and zone have nothing to say about it.
pub fn date_after(start: &str, days: u32) -> Option<(i64, u32, u32)> {
    let mut parts = start.trim().split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(civil(days_from_civil(y, m, d) + i64::from(days)))
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// "Friday, 13 November".
fn day_date((y, m, d): (i64, u32, u32)) -> String {
    // 1970-01-01 was a Thursday; Monday is 0.
    let weekday = (days_from_civil(y, m, d) + 3).rem_euclid(7) as usize;
    (t().menu.dated)(t().menu.weekdays[weekday], d, t().menu.months[(m - 1) as usize])
}

pub(crate) fn meal_name(meal: Meal) -> &'static str {
    match meal {
        Meal::Breakfast => t().menu.breakfast,
        Meal::Lunch => t().menu.lunch,
        Meal::Dinner => t().menu.dinner,
    }
}

// ---- the template ------------------------------------------------------------------------

/// Whether this tour is an access code's menu template rather than a trip.
pub fn is_template(tour: &Tour) -> bool {
    tc_core::extras::bool_of(&tour.extras, tc_core::extras::MENU_TEMPLATE)
}

fn code_of(tour: &Tour) -> String {
    tc_core::extras::str_of(&tour.extras, tc_core::extras::ACCESS_CODE)
}

/// Whose a template is, as its row in the tour list says: the trips under its code, in the
/// list's order - "for: Danube, Shipka and 5 more". With two codes on a device there are
/// two templates of one name, and this is what tells them apart.
pub fn template_owners(template: &Tour, trips: &[Tour]) -> String {
    let code = code_of(template);
    let names: Vec<&str> = trips
        .iter()
        .filter(|t| !is_template(t) && code_of(t) == code)
        .map(|t| t.name.trim())
        .collect();
    match names.len() {
        0 => t().list.template_for_none.to_owned(),
        1 | 2 => (t().list.template_for)(&names.join(", "), 0),
        n => (t().list.template_for)(&names[..2].join(", "), n - 2),
    }
}

/// The templates filed under the same access code as `tour`, among `tours`, by name: a code
/// can have several, each a menu of its own ("Походное меню", "Новый год"). Sorted by name, so
/// the choice reads the same on every device, whatever order the server listed them in.
fn templates_among(tours: &[Tour], tour: &Tour) -> Vec<Tour> {
    let code = code_of(tour);
    let mut found: Vec<Tour> = tours
        .iter()
        .filter(|t| is_template(t) && t.id != tour.id && code_of(t) == code)
        .cloned()
        .collect();
    found.sort_by_key(|t| t.name.trim().to_lowercase());
    found
}

thread_local! {
    /// When this page last read the tour list from the server, in `now_millis` time.
    static LIST_READ_AT: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// How long a list read from the server stays good for opening a template panel without asking
/// again.
const LIST_FRESH_MS: f64 = 60_000.0;

/// Keeps the tour list the server just sent, and notes when it did.
fn remember_list(tours: &[Tour]) {
    crate::queue::cache_list(tours);
    LIST_READ_AT.with(|at| at.set(Some(crate::queue::now_millis())));
}

fn list_is_fresh() -> bool {
    LIST_READ_AT.with(|at| at.get()).is_some_and(|read| crate::queue::now_millis() - read < LIST_FRESH_MS)
}

/// The code's template tours, from the server - or, without a network, from the list this
/// device saw last. The list is kept, so the tour dialog knows about a template made here.
///
/// An empty catalogue opens the take panel by itself, and so every time the tour is opened
/// with nothing in it; one request a minute is enough for that, so a list read less than a
/// minute ago is used as it stands. A template saved here refreshes it (see `remember_list`).
async fn find_templates(tour: &Tour) -> Result<Vec<Tour>, crate::api::Failed> {
    if list_is_fresh() {
        if let Some(tours) = crate::queue::cached_list() {
            return Ok(templates_among(&tours, tour));
        }
    }
    match crate::api::tours().await {
        Ok(tours) => {
            remember_list(&tours);
            Ok(templates_among(&tours, tour))
        }
        Err(e) if crate::api::looks_offline(&e) => {
            Ok(crate::queue::cached_list().map(|tours| templates_among(&tours, tour)).unwrap_or_default())
        }
        Err(e) => Err(e),
    }
}

/// Says something that needs no answer.
fn ask_nothing(message: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.alert_with_message(message);
    }
}

pub(crate) fn ask(question: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(question).ok())
        .unwrap_or(false)
}

/// Which of the template bar's two panels is open: one at a time.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Closed,
    Take,
    Save,
}

/// What a template is called in a list: its name without the blanks around it, or the generic
/// title when it has none - a template a client made without asking for a name.
fn template_label(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() { t().menu.template_title.to_owned() } else { name.to_owned() }
}

/// The name a new template is saved under, if one was typed: blanks around it do not count.
fn new_template_name(typed: &str) -> Option<String> {
    let name = typed.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The template a typed name would take over, if one is already there: the same name as the
/// lists show it, blanks cut and case ignored. Saving under it would make two of one name.
fn taken_by<'a>(typed: &str, templates: &'a [Tour]) -> Option<&'a Tour> {
    let typed = typed.trim().to_lowercase();
    templates.iter().find(|found| template_label(&found.name).to_lowercase() == typed)
}

/// Under the catalogue: take a catalogue from the starter set or from one of the code's
/// templates, or save this catalogue as one of them - over an existing one, or as a new one.
///
/// The template is a tour of its own under the same code, marked `IsMenuTemplate`, with
/// nobody on it: the one thing every client already stores, lists and keeps versions of. On
/// the template itself this says what it is instead. A code can have several templates, each
/// under its own name, so both panels list them all.
#[component]
fn TemplateBar(tour: Tour, menu: Menu, apply: Callback<Operation>, open_take: bool) -> impl IntoView {
    // The template says what it is where a trip says what its portions come to.
    if is_template(&tour) {
        return ().into_any();
    }
    let panel = RwSignal::new(if open_take { Panel::Take } else { Panel::Closed });
    // None while the list is being read; then the templates, or why they could not be read.
    let templates = RwSignal::new(None::<Result<Vec<Tour>, String>>);
    // The id of the template a save replaces; None saves a new one.
    let target = RwSignal::new(None::<String>);
    let name = RwSignal::new(String::new());
    let name_error = RwSignal::new(None::<String>);
    let said = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let tour = StoredValue::new(tour);
    let menu = StoredValue::new(menu);

    // The list is read each time a panel opens, so a template made on another device is there.
    let load = move || {
        busy.set(true);
        templates.set(None);
        leptos::task::spawn_local(async move {
            let found = find_templates(&tour.get_value()).await.map_err(|e| e.to_string());
            templates.set(Some(found));
            busy.set(false);
        });
    };
    if open_take {
        load();
    }

    let open = move |which: Panel| {
        said.set(String::new());
        name_error.set(None);
        if panel.get_untracked() == which {
            panel.set(Panel::Closed);
        } else {
            panel.set(which);
            load();
        }
    };

    // A catalogue taken: the starter set, or a template. Asked first, and the question names it.
    let take = move |source: Option<Tour>| {
        let (label, catalogue) = match source {
            None => (t().menu.starter_set.to_owned(), starter(menu.get_value().days).as_template()),
            Some(found) => match Menu::of(&found) {
                Some(found_menu) => (template_label(&found.name), found_menu.as_template()),
                None => {
                    said.set(t().menu.template_no_menu.into());
                    return;
                }
            },
        };
        if ask(&(t().menu.template_take_q)(&label)) {
            panel.set(Panel::Closed);
            apply.run(Operation::Menu(MenuEdit::TakeTemplate(catalogue)));
        }
    };

    // What a save said: the panel closes when it went through, and stays open to be fixed if not.
    let saved = move |text: String, ok: bool| {
        if ok {
            panel.set(Panel::Closed);
        }
        said.set(text);
        busy.set(false);
    };

    let save = move |_| {
        let catalogue = menu.get_value().as_template();
        let tour = tour.get_value();
        match target.get_untracked() {
            Some(id) => {
                let existing = templates
                    .get_untracked()
                    .and_then(Result::ok)
                    .and_then(|found| found.into_iter().find(|found_tour| found_tour.id.as_str() == id));
                let Some(existing) = existing else { return };
                let label = template_label(&existing.name);
                if !ask(&(t().menu.template_replace_q)(&label)) {
                    return;
                }
                busy.set(true);
                let op = Operation::Menu(MenuEdit::TakeTemplate(catalogue));
                leptos::task::spawn_local(async move {
                    match crate::sync::record(existing.id.as_str(), op).await.1 {
                        crate::sync::Status::Failed(e) => saved(e.to_string(), false),
                        crate::sync::Status::Waiting(_) => saved(t().menu.template_saved_here.into(), true),
                        _ => saved(t().menu.template_saved.into(), true),
                    }
                });
            }
            None => {
                let Some(new_name) = new_template_name(&name.get_untracked()) else {
                    name_error.set(Some(t().menu.template_name_needed.to_owned()));
                    return;
                };
                let listed = templates.get_untracked().and_then(Result::ok).unwrap_or_default();
                if let Some(taken) = taken_by(&new_name, &listed) {
                    // Not a second template of that name: the save is the replace it names.
                    target.set(Some(taken.id.as_str().to_owned()));
                    name_error.set(Some((t().menu.template_name_taken)(&template_label(&taken.name))));
                    return;
                }
                name_error.set(None);
                busy.set(true);
                leptos::task::spawn_local(async move {
                    let mut body = crate::edit::new_tour_body(&new_name);
                    if let (Some(obj), Ok(value)) = (body.as_object_mut(), serde_json::to_value(&catalogue)) {
                        obj.insert(tc_core::extras::MENU_TEMPLATE.into(), true.into());
                        obj.insert(tc_core::menu::MENU.into(), value);
                    }
                    let code = code_of(&tour);
                    match crate::api::add_tour(body, crate::api::Pile::Hashed(&code)).await {
                        Ok(_) => {
                            if let Ok(tours) = crate::api::tours().await {
                                remember_list(&tours);
                            }
                            name.set(String::new());
                            saved(t().menu.template_saved.into(), true);
                        }
                        Err(e) => saved(e.to_string(), false),
                    }
                });
            }
        }
    };

    view! {
        <div class="tcw-food-template">
            <div class="tcw-food-template-title">{t().list.menu_templates}</div>
            <p class="tcw-food-note">{t().menu.template_about}</p>
            <div class="tcw-food-template-buttons">
                <button type="button" class="tcn-btn tcn-btn-sm"
                        aria-expanded=move || (panel.get() == Panel::Take).to_string()
                        on:click=move |_| open(Panel::Take)>{t().menu.template_take}</button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        aria-expanded=move || (panel.get() == Panel::Save).to_string()
                        on:click=move |_| open(Panel::Save)>{t().menu.template_save}</button>
            </div>
            <Show when=move || panel.get() == Panel::Take>
                <div class="tcw-food-choices">
                    <p class="tcw-food-note">{t().menu.take_pick}</p>
                    {move || match templates.get() {
                        None => view! { <p class="tcw-food-note" role="status">{t().menu.template_loading}</p> }.into_any(),
                        Some(Err(e)) => view! { <div class="tcn-errors">{e}</div> }.into_any(),
                        Some(Ok(found)) => view! {
                            <div class="tcw-food-template-buttons">
                                <button type="button" class="tcn-btn tcn-btn-sm"
                                        on:click=move |_| take(None)>{t().menu.starter_set}</button>
                                {found
                                    .into_iter()
                                    .map(|template| {
                                        let label = template_label(&template.name);
                                        view! {
                                            <button type="button" class="tcn-btn tcn-btn-sm"
                                                    on:click=move |_| take(Some(template.clone()))>{label}</button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }.into_any(),
                    }}
                </div>
            </Show>
            <Show when=move || panel.get() == Panel::Save>
                <fieldset class="tcw-food-choices">
                    <legend class="tcw-food-note">{t().menu.save_pick}</legend>
                    {move || match templates.get() {
                        None => view! { <p class="tcw-food-note" role="status">{t().menu.template_loading}</p> }.into_any(),
                        Some(Err(e)) => view! { <div class="tcn-errors">{e}</div> }.into_any(),
                        Some(Ok(found)) => found
                            .into_iter()
                            .map(|template| {
                                let id = template.id.as_str().to_owned();
                                let input_id = format!("menu-template-{id}");
                                let label = (t().menu.replace_choice)(&template_label(&template.name));
                                let shown = id.clone();
                                let picked = id;
                                view! {
                                    <label class="tcw-food-choice">
                                        <input type="radio" name="menu-template-target" id=input_id
                                               prop:checked=move || target.get().as_deref() == Some(shown.as_str())
                                               on:change=move |_| target.set(Some(picked.clone())) />
                                        {label}
                                    </label>
                                }
                            })
                            .collect_view()
                            .into_any(),
                    }}
                    <label class="tcw-food-choice">
                        <input type="radio" name="menu-template-target" id="menu-template-new"
                               prop:checked=move || target.get().is_none()
                               on:change=move |_| target.set(None) />
                        {t().menu.new_template_choice}
                    </label>
                    <label class="tcw-food-choice">
                        {t().menu.new_template_label}
                        <input type="text" class="tcn-input" id="menu-template-name"
                               placeholder=t().menu.new_template_placeholder
                               prop:value=move || name.get()
                               on:input=move |ev| name.set(event_target_value(&ev))
                               on:focus=move |_| target.set(None) />
                    </label>
                    <Show when=move || name_error.get().is_some()>
                        <div class="tcn-errors">{move || name_error.get().unwrap_or_default()}</div>
                    </Show>
                </fieldset>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                        prop:disabled=move || busy.get() on:click=save>{t().menu.template_save_go}</button>
            </Show>
            <Show when=move || !said.get().is_empty()>
                <p class="tcw-food-note" role="status">{move || said.get()}</p>
            </Show>
        </div>
    }
    .into_any()
}

// ---- moving a menu to another access code -------------------------------------------------

/// What "Copy menu" puts on the clipboard: the catalogue, marked as such, so that a paste can
/// tell a menu from any other JSON.
const MENU_MARK: &str = "TourcalcMenu";

fn menu_as_text(menu: &Menu) -> String {
    serde_json::json!({ MENU_MARK: 1, tc_core::menu::MENU: menu.as_template() }).to_string()
}

/// The menu in pasted text: what "Copy menu" wrote, a whole tour's JSON from the list's "Copy
/// JSON", or a bare menu. Something with no dishes and no products is not taken for one -
/// an empty catalogue in place of a real one is not what anybody pasting meant.
fn menu_from_text(text: &str) -> Option<Menu> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let inner = value
        .as_object()?
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(tc_core::menu::MENU))
        .map(|(_, v)| v.clone())
        .unwrap_or(value);
    // Read the way a tour's own menu is read, older spellings and all.
    let tour = Tour::from_json(&serde_json::json!({ "Id": "pasted", "Name": "", tc_core::menu::MENU: inner }).to_string()).ok()?;
    Menu::of(&tour)
        .filter(|m| !m.dishes.is_empty() || !m.products.is_empty())
        .map(|m| m.as_template())
}

/// Under the catalogue, after the template: the catalogue as text to send to another access
/// code, and a box to paste one into.
///
/// Through the clipboard and a box rather than between codes directly: a device holds only
/// the codes' hashes, and the people of one code have no business in another's tours - a
/// message with the menu in it is how one group hands it to the next.
#[component]
fn TransferBar(menu: Menu, apply: Callback<Operation>) -> impl IntoView {
    let said = crate::ui::Brief::new();
    let pasting = RwSignal::new(false);
    let pasted = RwSignal::new(String::new());
    let trouble = RwSignal::new(None::<&'static str>);
    let text = StoredValue::new(menu_as_text(&menu));

    let take = move |_| match menu_from_text(&pasted.get_untracked()) {
        None => trouble.set(Some(t().menu.not_a_menu)),
        Some(found) => {
            trouble.set(None);
            if ask(t().menu.paste_q) {
                pasting.set(false);
                pasted.set(String::new());
                apply.run(Operation::Menu(MenuEdit::TakeTemplate(found)));
            }
        }
    };

    view! {
        <div class="tcw-food-template">
            <div class="tcw-food-template-title">{t().menu.transfer_title}</div>
            <p class="tcw-food-note">{t().menu.transfer_about}</p>
            <div class="tcw-food-template-buttons">
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| {
                            crate::tour::copy_to_clipboard(&text.get_value());
                            said.say(t().menu.menu_copied);
                        }>
                    {move || if said.is_on() { said.get() } else { t().menu.copy_menu.to_owned() }}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm" aria-expanded=move || pasting.get().to_string()
                        on:click=move |_| pasting.update(|p| *p = !*p)>
                    {t().menu.paste_menu}
                </button>
            </div>
            <Show when=move || pasting.get()>
                <div class="tcw-food-paste">
                    <textarea class="tcn-input" rows="3" placeholder=t().menu.paste_here
                              prop:value=move || pasted.get()
                              on:input=move |ev| pasted.set(event_target_value(&ev))></textarea>
                    <Show when=move || trouble.get().is_some()>
                        <div class="tcn-errors">{move || trouble.get().unwrap_or_default()}</div>
                    </Show>
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                            prop:disabled=move || pasted.get().trim().is_empty()
                            on:click=take>
                        {t().menu.take_pasted}
                    </button>
                </div>
            </Show>
        </div>
    }
}

/// The catalogue emptied - dishes, products, the daily list - for a group that would rather
/// start from nothing than from the starter. The places stay: they are where the group
/// shops, not what it eats. The edit is "take this template" with an empty one, so the plan
/// and what was bought go the way they go then.
#[component]
fn ClearBar(menu: Menu, apply: Callback<Operation>) -> impl IntoView {
    let places = StoredValue::new(menu.places.clone());
    view! {
        <div class="tcw-food-template">
            <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                    on:click=move |_| {
                        if ask(t().menu.clear_q) {
                            let empty = Menu { on: true, places: places.get_value(), ..Menu::default() };
                            apply.run(Operation::Menu(MenuEdit::TakeTemplate(empty)));
                        }
                    }>
                {t().menu.clear_catalogue}
            </button>
        </div>
    }
}

// ---- the tab -----------------------------------------------------------------------------

#[component]
pub fn MenuTab(
    tour: Tour,
    state: MenuState,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let Some(menu) = Menu::shown(&tour) else {
        return ().into_any();
    };
    let eating = Eating::of(&tour);
    let days = menu.days;
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));

    let portions = format!("{:.1}", eating.portions)
        .trim_end_matches(".0")
        .replace('.', &crate::ui::decimal().to_string());
    let note = (t().menu.portions_note)(&portions, eating.full as usize, eating.others as usize, eating.full_weight);
    // A template has nobody on it, so no portions to speak of: it says what it is instead.
    let note_for_catalogue = if is_template(&tour) { t().menu.template_is_this.to_owned() } else { note.clone() };

    let tour_for_shopping = tour.clone();
    let menu_for_shopping = menu.clone();
    let menu_for_catalogue = menu.clone();
    let tour_for_template = tour.clone();
    let menu_for_template = menu.clone();
    // A template has no trip to plan or shop for: only its catalogue is shown.
    let template = is_template(&tour);
    if template {
        state.view.set(View::Catalogue);
    }
    // A menu with nothing in its catalogue says so, and says where dishes come from.
    let nudge = !template && menu.dishes.is_empty() && menu.products.is_empty();

    view! {
        <div class="tcw-food">
            <div class="tcw-food-bar" style:display=move || if template { "none" } else { "" }>
                <div class="tcw-seg" role="group">
                    <button type="button" class:is-on=move || state.view.get() == View::Plan
                            on:click=move |_| state.view.set(View::Plan)>{t().menu.plan}</button>
                    <button type="button" class:is-on=move || state.view.get() == View::Shopping
                            on:click=move |_| state.view.set(View::Shopping)>{t().menu.shopping}</button>
                    <button type="button" class:is-on=move || state.view.get() == View::Catalogue
                            on:click=move |_| state.view.set(View::Catalogue)>{t().menu.catalogue}</button>
                </div>
                // The plan's length - in the shopping it only pushed the list down.
                <div class="tcw-food-days" style:display=move || if state.view.get() == View::Shopping { "none" } else { "" }>
                    <span>{t().menu.days}</span>
                    <button type="button" class="tcn-btn tcn-btn-sm" title=t().menu.fewer_days
                            disabled=days <= 1
                            on:click=move |_| change(MenuEdit::Days(days.saturating_sub(1).max(1)))>"−"</button>
                    <b>{days}</b>
                    <button type="button" class="tcn-btn tcn-btn-sm" title=t().menu.more_days
                            on:click=move |_| change(MenuEdit::Days(days + 1))>"+"</button>
                </div>
            </div>
            // What the amounts mean for this tour - in the plan; in the catalogue it is said
            // under the dishes, where the amounts per portion are. The shopping goes without:
            // on a phone it pushed the list off the first screen.
            <Show when=move || state.view.get() == View::Plan>
                <p class="tcw-food-note">{note.clone()}</p>
            </Show>
            <Show when=move || nudge && state.view.get() != View::Catalogue>
                <div class="tcw-food-template">
                    <p class="tcw-food-note">{t().menu.empty_elsewhere}</p>
                    <div class="tcw-food-template-buttons">
                        <button type="button" class="tcn-btn tcn-btn-sm"
                                on:click=move |_| state.view.set(View::Catalogue)>{t().menu.open_catalogue}</button>
                    </div>
                </div>
            </Show>

            <Show when=move || state.view.get() == View::Plan>
                <Plan menu=menu.clone() apply=apply />
            </Show>
            <Show when=move || state.view.get() == View::Shopping>
                // Built again when the days change: every amount on it is for them.
                {
                    let tour = tour_for_shopping.clone();
                    let menu = menu_for_shopping.clone();
                    move || {
                        state.days.track();
                        view! {
                            <Shopping tour=tour.clone() menu=menu.clone() eating=eating
                                      state=state apply=apply dialog=dialog />
                        }
                    }
                }
            </Show>
            <Show when=move || state.view.get() == View::Catalogue>
                <Show when=move || nudge>
                    <p class="tcw-food-note">{t().menu.empty_here}</p>
                </Show>
                <crate::menu_catalogue::Catalogue menu=menu_for_catalogue.clone() state=state apply=apply
                                                  note=note_for_catalogue.clone() />
                <TemplateBar tour=tour_for_template.clone() menu=menu_for_template.clone() apply=apply
                             open_take=nudge />
                <TransferBar menu=menu_for_template.clone() apply=apply />
                <ClearBar menu=menu_for_template.clone() apply=apply />
            </Show>
        </div>
    }
    .into_any()
}

/// The days, each with its three meals, and what is bought every day whatever is cooked.
#[component]
fn Plan(menu: Menu, apply: Callback<Operation>) -> impl IntoView {
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));
    let days = menu.days;
    let arrival = menu.arrival();
    let departure = menu.departure();
    let index = |m: Meal| Meal::ALL.iter().position(|x| *x == m).unwrap_or(0);
    // Inside the trip: not before arriving on the first day, not after leaving on the last.
    let trip_menu = StoredValue::new(menu.clone());
    let inside = move |day: u32, meal: Meal| trip_menu.with_value(|m| m.in_trip(day, meal));

    // When the trip starts and ends, and the first day's date.
    let bound = |current: Meal, first: bool| {
        Meal::ALL
            .into_iter()
            .map(|meal| {
                let label = if first { t().menu.from_meal[index(meal)] } else { t().menu.to_meal[index(meal)] };
                view! { <option value=meal_name(meal) selected=meal == current>{label}</option> }
            })
            .collect_view()
    };
    let chosen_meal = |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        Meal::ALL.into_iter().find(|m| meal_name(*m) == v)
    };
    let start = menu.start.clone().unwrap_or_default();
    let trip = view! {
        <div class="tcn-card tcw-food-day tcw-food-trip">
            <label class="tcw-food-meal">
                <span>{t().menu.start_date}</span>
                <input class="tcn-input" type="date" prop:value=start
                       on:change=move |ev| {
                           let v = event_target_value(&ev);
                           change(MenuEdit::Start((!v.is_empty()).then_some(v)));
                       } />
            </label>
            <label class="tcw-food-meal">
                <span>{t().menu.first_day}</span>
                <select class="tcn-input" on:change=move |ev| if let Some(m) = chosen_meal(ev) { change(MenuEdit::Arrive(m)) }>
                    {bound(arrival, true)}
                </select>
            </label>
            <label class="tcw-food-meal">
                <span>{t().menu.last_day}</span>
                <select class="tcn-input" on:change=move |ev| if let Some(m) = chosen_meal(ev) { change(MenuEdit::Leave(m)) }>
                    {bound(departure, false)}
                </select>
            </label>
        </div>
    };

    // What each meal has now, dishes since deleted left out - what the selects show, and what
    // an edit of one of them starts from.
    let plan = StoredValue::new(menu.clone());
    let current = move |day: u32, meal: Meal| -> Vec<String> {
        plan.with_value(|m| m.dishes_on(day, meal).iter().map(|d| d.id.clone()).collect())
    };
    let put = move |day: u32, meal: Meal, dishes: Vec<String>| change(MenuEdit::Meals(vec![Slot { day, meal, dishes }]));

    // A dish for one meal on every day at once - "yoghurt every morning" - on the days the
    // trip has that meal. The main dish only: what was added beside it on a day stays.
    let every_day = Meal::ALL
        .into_iter()
        .map(|meal| {
            let options = menu
                .dishes_for(meal)
                .map(|d| view! { <option value=d.id.clone()>{d.name.clone()}</option> })
                .collect_view();
            view! {
                <label class="tcw-food-meal">
                    <span class=format!("tcw-meal {}", crate::menu_catalogue::meal_class(meal))>{meal_name(meal)}</span>
                    <select class="tcn-input"
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                if v == "?" { return; }
                                let slots = (1..=days)
                                    .filter(|d| inside(*d, meal))
                                    .map(|day| {
                                        let mut dishes = current(day, meal);
                                        match (v.is_empty(), dishes.is_empty()) {
                                            // "Nothing cooked" is nothing, extras and all.
                                            (true, _) => dishes.clear(),
                                            (false, true) => dishes.push(v.clone()),
                                            (false, false) => dishes[0] = v.clone(),
                                        }
                                        Slot { day, meal, dishes }
                                    })
                                    .collect();
                                change(MenuEdit::Meals(slots));
                            }>
                        <option value="?" selected=true>{t().menu.pick}</option>
                        <option value="">{t().menu.nothing}</option>
                        {options}
                    </select>
                </label>
            }
        })
        .collect_view();

    let day_cards = (1..=days)
        .map(|day| {
            // The meals outside the trip are not shown: a quiet word in their place - "arriving
            // for dinner" where the breakfast and lunch of the first day would be.
            let away = |meals: Vec<Meal>, word: &'static str| {
                let first = meals.first().copied()?;
                Some(view! {
                    <div class="tcw-food-away" style=format!("grid-row: {} / span {}", index(first) + 2, meals.len())>{word}</div>
                })
            };
            let before = if day == 1 {
                away(Meal::ALL.into_iter().filter(|m| index(*m) < index(arrival)).collect(), t().menu.arriving_for[index(arrival)])
            } else {
                None
            };
            let after = if day == days {
                away(Meal::ALL.into_iter().filter(|m| index(*m) > index(departure)).collect(), t().menu.leaving_after[index(departure)])
            } else {
                None
            };
            let meals = Meal::ALL
                .into_iter()
                .filter(|meal| inside(day, *meal))
                .map(|meal| {
                    let now = current(day, meal);
                    let options = |chosen: &str| {
                        menu.dishes_for(meal)
                            .map(|d| {
                                let on = d.id == chosen;
                                view! { <option value=d.id.clone() selected=on>{d.name.clone()}</option> }
                            })
                            .collect_view()
                    };
                    let main = now.first().cloned().unwrap_or_default();
                    // The dishes beside the main one: plov, and a salad, and mulled wine.
                    let extras = now
                        .iter()
                        .enumerate()
                        .skip(1)
                        .map(|(at, id)| view! {
                            <div class="tcw-food-extra">
                                <select class="tcn-input"
                                        on:change=move |ev| {
                                            let mut dishes = current(day, meal);
                                            if at < dishes.len() {
                                                dishes[at] = event_target_value(&ev);
                                                put(day, meal, dishes);
                                            }
                                        }>
                                    {options(id)}
                                </select>
                                <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-x" title=t().menu.remove
                                        on:click=move |_| {
                                            let mut dishes = current(day, meal);
                                            if at < dishes.len() {
                                                dishes.remove(at);
                                                put(day, meal, dishes);
                                            }
                                        }>
                                    "✕"
                                </button>
                            </div>
                        })
                        .collect_view();
                    // "+": the first of the meal's dishes not on it yet; a meal with nothing
                    // cooked has nothing to add beside.
                    let next = menu.dishes_for(meal).map(|d| d.id.clone()).find(|id| !now.contains(id));
                    let can_add = !now.is_empty() && next.is_some();
                    // Not chosen yet - a day just added: marked, and "— not chosen —" rather than
                    // "— nothing —", which is a choice.
                    let unplanned = menu.unplanned(day, meal);
                    view! {
                        <div class=format!("tcw-food-meal tcw-food-slot {}", crate::menu_catalogue::meal_class(meal))
                             class:is-unplanned=unplanned title=unplanned.then_some(t().menu.unplanned_title)>
                            <span class=format!("tcw-meal {}", crate::menu_catalogue::meal_class(meal))>{meal_name(meal)}</span>
                            <div class="tcw-food-dishes">
                                <select class="tcn-input" aria-label=meal_name(meal)
                                        on:change=move |ev| {
                                            let v = event_target_value(&ev);
                                            if v == "?" { return; }
                                            let mut dishes = current(day, meal);
                                            match (v.is_empty(), dishes.is_empty()) {
                                                (true, _) => dishes.clear(),
                                                (false, true) => dishes.push(v),
                                                (false, false) => dishes[0] = v,
                                            }
                                            put(day, meal, dishes);
                                        }>
                                    {unplanned.then(|| view! { <option value="?" selected=true>{t().menu.unplanned}</option> })}
                                    <option value="" selected=main.is_empty() && !unplanned>{t().menu.nothing}</option>
                                    {options(&main)}
                                </select>
                                {extras}
                                {can_add.then(|| view! {
                                    <button type="button" class="tcn-btn tcn-btn-sm tcw-food-more"
                                            on:click=move |_| {
                                                let mut dishes = current(day, meal);
                                                if let Some(id) = next.clone() {
                                                    dishes.push(id);
                                                    put(day, meal, dishes);
                                                }
                                            }>
                                        {t().menu.more_dish}
                                    </button>
                                })}
                            </div>
                        </div>
                    }
                })
                .collect_view();
            let (title, small) = match menu.start.as_deref().and_then(|s| date_after(s, day - 1)) {
                Some(date) => (day_date(date), Some((t().menu.day_n)(day))),
                None => ((t().menu.day)(day), None),
            };
            // The day's own word - "arrival", "radial hikes" - after its name; "✎" writes it.
            let note = menu.day_note(day).unwrap_or_default().to_owned();
            let writing = RwSignal::new(false);
            let typed = RwSignal::new(note.clone());
            let was = StoredValue::new(note.clone());
            let done = move || {
                writing.set(false);
                let now = typed.get_untracked().trim().to_owned();
                if now != was.get_value() {
                    change(MenuEdit::DayNote { day, note: now });
                }
            };
            view! {
                <div class="tcn-card tcw-food-day">
                    <div class="tcw-food-dayname">
                        {title}
                        {small.map(|s| view! { " " <small>{s}</small> })}
                        <Show when=move || !writing.get()
                              fallback=move || view! {
                                  <input class="tcn-input tcw-day-note-input" type="text" autofocus=true
                                         placeholder=t().menu.day_note_hint
                                         prop:value=move || typed.get()
                                         on:input=move |ev| typed.set(event_target_value(&ev))
                                         on:blur=move |_| done()
                                         on:keydown=move |ev| match ev.key().as_str() {
                                             "Enter" => done(),
                                             "Escape" => writing.set(false),
                                             _ => {}
                                         } />
                              }>
                            {(!note.is_empty()).then(|| view! { <span class="tcw-day-note">" — " {note.clone()}</span> })}
                            <button type="button" class="tcw-day-note-edit" title=t().menu.day_note_title
                                    on:click=move |_| writing.set(true)>
                                {if note.is_empty() { t().menu.day_note_add } else { "✎" }}
                            </button>
                        </Show>
                    </div>
                    {before}
                    {meals}
                    {after}
                </div>
            }
        })
        .collect_view();

    let daily = tc_core::menu::daily_groups(&menu.daily)
        .into_iter()
        .filter_map(|(i, whens)| menu.product(&i.product).map(|p| (p, whens)))
        .map(|(p, whens)| {
            let mut notes: Vec<&str> = Vec::new();
            match p.eaters {
                Eaters::Everyone => {}
                Eaters::FullWeight => notes.push(t().menu.for_full),
                Eaters::Others => notes.push(t().menu.for_others),
            }
            if !whens.iter().all(When::is_every_day) {
                notes.push(whens_name(&whens));
            }
            if notes.is_empty() { p.name.clone() } else { format!("{} ({})", p.name, notes.join(", ")) }
        })
        .collect::<Vec<_>>()
        .join(" · ");

    view! {
        {trip}
        <div class="tcn-card tcw-food-day tcw-food-all">
            <div class="tcw-food-dayname">{t().menu.every_day}</div>
            {every_day}
        </div>
        <div class="tcw-food-days-list">{day_cards}</div>
        <p class="tcw-food-note"><b>{t().menu.daily}</b> " " {daily}</p>
    }
}

/// The shopping, place by place and product by product: who buys and pays for each, a
/// tick when it is bought, and the expense it was recorded as.
#[component]
fn Shopping(
    tour: Tour,
    menu: Menu,
    eating: Eating,
    state: MenuState,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let tour_id = tour.id.as_str().to_owned();
    // Whoever can buy: people who pay for themselves, not the ones paid for.
    let shoppers: Vec<(String, String)> = crate::edit::sorted_people(&tour)
        .into_iter()
        .filter(|p| p.parent.is_none())
        .map(|p| (p.id.as_str().to_owned(), p.name.clone()))
        .collect();
    forget_unknown_me(state, &shoppers);

    let me_options = {
        let chosen = state.me.get_untracked().unwrap_or_default();
        shoppers
            .iter()
            .map(|(id, name)| {
                let on = *id == chosen;
                view! { <option value=id.clone() selected=on>{name.clone()}</option> }
            })
            .collect_view()
    };
    // A link to this list as it is now - whose, and with the others' lines or without - for
    // that person's phone. Through the tour's share link, which also signs them in; a tour
    // without one has nothing to offer.
    let code = StoredValue::new(crate::tour::share_code(&tour));
    let link_said = crate::ui::Brief::new();
    let copy_link = {
        let tour_id = StoredValue::new(tour_id.clone());
        move |_| {
            let Some(me) = state.me.get_untracked() else { return };
            let path = shopping_link(
                &code.get_value(),
                &tour_id.get_value(),
                &me,
                state.everybody.get_untracked(),
                !state.hide_bought.get_untracked(),
            );
            crate::tour::copy_to_clipboard(&crate::tour::full_address(&path));
            link_said.say(t().sync.link_copied);
        }
    };
    let who_am_i = view! {
        <div class="tcw-food-me">
            <label>
                <span>{t().menu.who_am_i}</span>
                <select class="tcn-input"
                        on:change={
                            let tour_id = tour_id.clone();
                            move |ev| {
                                let v = event_target_value(&ev);
                                let me = (!v.is_empty()).then_some(v);
                                crate::settings::set_me(&tour_id, me.as_deref());
                                state.me.set(me);
                            }
                        }>
                    <option value="" selected=state.me.get_untracked().is_none()>{t().menu.nobody}</option>
                    {me_options}
                </select>
            </label>
            // By the select, as an icon: a row of its own pushed the list down.
            <Show when=move || state.me.get().is_some() && !code.with_value(String::is_empty)>
                <button type="button" class="tcn-chip tcw-food-link" title=t().menu.shopping_link_hint
                        aria-label=t().menu.shopping_link_hint on:click=copy_link.clone()>
                    {move || if link_said.is_on() { link_said.get() } else { t().menu.shopping_link.to_owned() }}
                </button>
            </Show>
            <Show when=move || state.me.get().is_some()>
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || !state.everybody.get()
                           on:change=move |ev| state.everybody.set(!event_target_checked(&ev)) />
                    {t().menu.mine_only}
                </label>
            </Show>
            <label class="tcn-switchline">
                <input type="checkbox" prop:checked=move || state.hide_bought.get()
                       on:change=move |ev| state.set_hide_bought(event_target_checked(&ev)) />
                {t().menu.hide_bought}
            </label>
        </div>
    };

    let categories = menu.categories();
    let category_line = expense_category_line(&menu, apply);
    // The days the shopping is for: all, or a few - one tap picks a day, more add to it.
    // Days picked before the plan was made shorter are not days of it any more.
    let plan_days = menu.days;
    let in_plan = move |v: Vec<u32>| -> Vec<u32> { v.into_iter().filter(|d| (1..=plan_days).contains(d)).collect() };
    let day_chips = (menu.days > 1).then(|| {
        let total = menu.days as usize;
        let chips = menu
            .all_days()
            .into_iter()
            .map(|day| {
                let on = in_plan(state.days.get_untracked()).contains(&day);
                view! {
                    <button type="button" class="tcn-chip tcn-filter-chip" class:is-on=on
                            on:click=move |_| state.days.update(|v| {
                                v.retain(|d| (1..=plan_days).contains(d));
                                if v.contains(&day) {
                                    v.retain(|d| *d != day);
                                } else {
                                    v.push(day);
                                    v.sort_unstable();
                                }
                                if v.len() == total {
                                    v.clear();
                                }
                            })>
                        {day_short(&menu, day)}
                    </button>
                }
            })
            .collect_view();
        let all = in_plan(state.days.get_untracked()).is_empty();
        view! {
            <div class="tcw-shop-days tcn-chips">
                <span class="tcw-shop-days-label">{t().menu.shopping_for}</span>
                <button type="button" class="tcn-chip tcn-filter-chip" class:is-on=all
                        on:click=move |_| state.days.set(Vec::new())>
                    {t().menu.days_all}
                </button>
                {chips}
            </div>
        }
    });
    let places = menu.shop_places();
    let place_options = places.iter().map(|p| view! { <option value=p.clone()></option> }).collect_view();
    let lines = shopping_lines(&tour, &menu, &eating, &in_plan(state.days.get_untracked()), &shoppers);
    if lines.is_empty() {
        return view! { {who_am_i} <div class="tcn-empty">{t().menu.nothing_to_buy}</div> }.into_any();
    }

    // Somebody picked and everything taken by others, or nothing found: say so, rather than
    // an empty screen.
    let rows: Vec<(Option<String>, String, bool)> = lines.iter().map(|l| (l.who.clone(), l.hay.clone(), l.ticked)).collect();
    let anything_shown = {
        let rows = rows.clone();
        move || rows.iter().any(|(who, hay, ticked)| visible_line(state, who.as_deref(), hay, *ticked))
    };
    // With nothing on screen, why: the search, everything bought, everything others'.
    let all_bought = {
        let rows = StoredValue::new(rows);
        move || rows.with_value(|rows| rows.iter().any(|(who, hay, _)| wanted(state, who.as_deref(), hay)))
    };

    let sort_options = Sort::ALL
        .into_iter()
        .map(|s| view! { <option value=s.name() selected=move || state.sort.get() == s>{s.name()}</option> })
        .collect_view();
    // The days too: picked now and then, and their line pushed the list down. Folded, the line
    // still says which, so a list for two days does not pass for the whole trip's.
    let days_said = (menu.days > 1).then(|| {
        let picked = in_plan(state.days.get_untracked());
        if picked.is_empty() { t().menu.days_all.to_owned() } else { days_label(&menu, &picked) }
    });
    let some_days = days_said.is_some() && !in_plan(state.days.get_untracked()).is_empty();
    // Chosen once and seldom again: a line saying what they are, opening to them.
    let arrange = view! {
        <button type="button" class="tcw-food-arrange" aria-expanded=move || state.arrange_open.get().to_string()
                on:click=move |_| state.arrange_open.update(|o| *o = !*o)>
            <span class="tcw-food-arrange-gear">"⚙"</span>
            <span>{move || format!("{} · {}", state.sort.get().name(), state.group.get().name())}</span>
            {days_said.map(|d| view! { <span>"·"</span> <span class:tcw-food-arrange-days=some_days>{d}</span> })}
            <span class="tcw-food-arrange-caret">{move || if state.arrange_open.get() { "▴" } else { "▾" }}</span>
        </button>
        <div style:display=move || if state.arrange_open.get() { "" } else { "none" }>
        <div class="tcw-food-me">
            <label>
                <span>{t().menu.sort}</span>
                <select class="tcn-input" on:change=move |ev| {
                    let v = event_target_value(&ev);
                    if let Some(s) = Sort::ALL.into_iter().find(|s| s.name() == v) {
                        state.sort.set(s);
                    }
                }>{sort_options}</select>
            </label>
            <label>
                <span>{t().menu.group}</span>
                <select class="tcn-input" on:change=move |ev| {
                    let v = event_target_value(&ev);
                    if let Some(g) = Group::ALL.into_iter().find(|g| g.name() == v) {
                        state.group.set(g);
                    }
                }>
                    {Group::ALL.into_iter().map(|g| view! {
                        <option value=g.name() selected=move || state.group.get() == g>{g.name()}</option>
                    }).collect_view()}
                </select>
            </label>
        </div>
        {day_chips}
        </div>
    };

    // Redrawn when the order or the grouping changes - nothing is being typed into here.
    let cards = move || {
        let names: Vec<(String, String)> = shoppers.clone();
        let mut lines = lines.clone();
        sort_lines(&mut lines, state.sort.get(), &names);
        let group = state.group.get();
        if group == Group::Category {
            let mut titles: Vec<String> = categories.clone();
            titles.push(String::new());
            titles
                .into_iter()
                .filter_map(|category| {
                    let here: Vec<Line> = lines.iter().filter(|l| l.category == category).cloned().collect();
                    let title = if category.is_empty() { t().menu.no_category.to_owned() } else { category };
                    (!here.is_empty()).then(|| view! {
                        <ListCard tour=tour.clone() title=title lines=here shoppers=shoppers.clone()
                                  state=state apply=apply dialog=dialog />
                    })
                })
                .collect_view()
                .into_any()
        } else if group == Group::Person {
            // A buyer who has left the tour counts as nobody: the product is to share out again.
            let mut buyers: Vec<Option<(String, String)>> = shoppers.iter().cloned().map(Some).collect();
            buyers.push(None);
            buyers
                .into_iter()
                .filter_map(|buyer| {
                    let here: Vec<Line> = lines
                        .iter()
                        .filter(|l| {
                            let who = l.who.as_deref().filter(|w| shoppers.iter().any(|(id, _)| id == w));
                            who == buyer.as_ref().map(|(id, _)| id.as_str())
                        })
                        .cloned()
                        .collect();
                    let title = buyer.map(|(_, name)| name).unwrap_or_else(|| t().menu.no_buyer.to_owned());
                    (!here.is_empty()).then(|| view! {
                        <ListCard tour=tour.clone() title=title lines=here shoppers=shoppers.clone()
                                  state=state apply=apply dialog=dialog />
                    })
                })
                .collect_view()
                .into_any()
        } else if group == Group::Place {
            // Where the shopping says, not the catalogue: Sasha's beer from Lidl is Lidl's.
            let mut titles = places.clone();
            titles.push(String::new());
            titles
                .into_iter()
                .filter_map(|place| {
                    let here: Vec<Line> = lines.iter().filter(|l| l.place.to_lowercase() == place.to_lowercase()).cloned().collect();
                    let title = if place.is_empty() { t().menu.no_place.to_owned() } else { place };
                    (!here.is_empty()).then(|| view! {
                        <ListCard tour=tour.clone() title=title lines=here shoppers=shoppers.clone()
                                  state=state apply=apply dialog=dialog />
                    })
                })
                .collect_view()
                .into_any()
        } else {
            view! {
                <ListCard tour=tour.clone() title=t().menu.category.to_owned() lines=lines shoppers=shoppers.clone()
                          state=state apply=apply dialog=dialog />
            }
            .into_any()
        }
    };

    view! {
        {who_am_i}
        {arrange}
        <datalist id=PLACES_LIST>{place_options}</datalist>
        {search_box(state.shop_find, t().menu.find_product)}
        <Show when=move || !anything_shown()>
            <div class="tcn-empty">
                {move || if !state.shop_find.get().trim().is_empty() {
                    t().menu.nothing_found
                } else if all_bought() {
                    t().menu.nothing_unbought
                } else {
                    t().menu.nothing_mine
                }}
            </div>
        </Show>
        {cards}
        {category_line}
    }
    .into_any()
}

/// The shopping's lines for these picked days (none - the whole trip): a product each, and
/// a line for each part split off it - what the shopping shows, here and in Mini.
fn shopping_lines(tour: &Tour, menu: &Menu, eating: &Eating, picked: &[u32], shoppers: &[(String, String)]) -> Vec<Line> {
    let places = menu.shop_places();
    let categories = menu.categories();
    let picked: Vec<u32> = picked.to_vec();
    let place_order = |name: &str| places.iter().position(|p| p.to_lowercase() == name.to_lowercase()).unwrap_or(usize::MAX);
    // The days it is for: all of them, or those picked above the list.
    let all_days = menu.all_days();
    let subset = !picked.is_empty() && picked.len() < all_days.len();
    let chosen: Vec<u32> = if subset { picked.clone() } else { all_days.clone() };
    let unit = crate::ui::unit(&tour);
    let need_of = |id: &str, days: &[u32]| {
        menu.shopping_on(&eating, days).iter().find(|n| n.product.id == id).map_or(0.0, |n| n.amount)
    };
    // What the whole trip needs, counted once for the list rather than again for every line.
    let whole_trip = menu.shopping_on(&eating, &all_days);
    let whole_of = |id: &str| whole_trip.iter().find(|n| n.product.id == id).map_or(0.0, |n| n.amount);
    // What the days need - and, over some days, a product with a split-off line not ticked yet
    // although these days need none of it: Sasha's coffee from home is on Friday's list too,
    // where there is no breakfast, until it is packed.
    let mut needs = menu.shopping_on(&eating, &chosen);
    if subset {
        for n in &whole_trip {
            let waiting = menu.purchase(&n.product.id).is_some_and(|p| p.splits.iter().any(|x| !x.bought));
            if waiting && !needs.iter().any(|x| x.product.id == n.product.id) {
                needs.push(tc_core::menu::Need { product: n.product, amount: 0.0 });
            }
        }
        needs.sort_by_key(|n| menu.products.iter().position(|p| p.id == n.product.id));
    }
    // Everything to buy, owned, with what has been said about each.
    needs
        .into_iter()
        .flat_map(|n| {
            let (dishes, daily) = menu.sources_on(&n.product.id, &chosen);
            let mut why: Vec<String> = dishes.iter().map(|(d, times)| format!("{} ×{times}", d.name)).collect();
            why.extend(menu.methods_for(&n.product.id, &chosen).iter().map(|(m, times)| format!("{} ×{times}", m.name)));
            // Each of its items on the daily list - with breakfast, with dinner - and "per trip"
            // once. Only the first was said: dried fruit with breakfast and with dinner read
            // "with breakfast ×2", though both were counted.
            use tc_core::menu::Per;
            if daily.iter().any(|i| i.per == Per::Trip) {
                why.push(t().menu.per_trip.to_owned());
            }
            // Per day (or per fire, which on the daily list is a day too) is bought once a day,
            // however many meals it is with: "per day ×3", not "with breakfast ×3 · with
            // dinner ×3" over 0.2 a day.
            let once_a_day: Vec<_> = daily.iter().filter(|i| matches!(i.per, Per::Day | Per::Fire)).collect();
            if !once_a_day.is_empty() {
                let times = chosen.iter().filter(|&&day| once_a_day.iter().any(|i| menu.daily_on(i.when, day))).count();
                if times > 0 {
                    why.push(format!("{} ×{times}", t().menu.per_day));
                }
            }
            for item in daily.iter().filter(|i| matches!(i.per, Per::Portion | Per::Head)) {
                let times = chosen.iter().filter(|&&day| menu.daily_on(item.when, day)).count();
                let said = format!("{} ×{times}", when_name(item.when));
                // None of these days: a split-off line shown for being not packed yet.
                if times > 0 && !why.contains(&said) {
                    why.push(said);
                }
            }
            let place_name = menu.bought_at(n.product);
            let purchase = menu.purchase(&n.product.id).cloned().unwrap_or_default();
            let buyer = purchase
                .who
                .as_deref()
                .and_then(|w| shoppers.iter().find(|(id, _)| id == w))
                .map(|(_, name)| name.clone())
                .unwrap_or_default();
            let category = n.product.category.as_deref().map(str::trim).unwrap_or("").to_owned();
            // Ticked and recorded, a day at a time or for the whole trip - each on its own: a
            // tick is the group's word that it is bought, an expense what was paid.
            let id = n.product.id.as_str();
            let needed = menu.days_needing(id);
            let covered = menu.covered_days(id);
            // An expense, if it is still in the tour: deleted, what it was for is not recorded.
            let exists = |s: &str| tour.spendings.iter().any(|x| x.id.as_str() == s);
            let recorded_days = menu.recorded_days(id, exists);
            let whole = purchase.spending.as_deref().is_some_and(exists);
            // The days the list is for, of those it is needed on.
            let here: Vec<u32> = if subset { needed.iter().copied().filter(|d| chosen.contains(d)).collect() } else { needed.clone() };
            let tick_days = if subset { here.clone() } else { Vec::new() };
            let ticked = if subset {
                here.iter().all(|d| covered.contains(d))
            } else {
                purchase.bought || (!needed.is_empty() && needed.iter().all(|d| covered.contains(d)))
            };
            // What "Record" is for: the days no expense has paid for - over the whole trip, the
            // whole trip if none has, else the rest, or Friday would be paid twice.
            let unpaid: Vec<u32> = here.iter().copied().filter(|d| !recorded_days.contains(d)).collect();
            let days = if !subset && recorded_days.is_empty() { Vec::new() } else { unpaid.clone() };
            let paid = !here.is_empty() && unpaid.is_empty();
            let spending = if whole {
                purchase.spending.clone()
            } else if paid {
                purchase
                    .parts
                    .iter()
                    .filter(|p| p.spending.as_deref().is_some_and(exists) && here.iter().all(|d| p.days.contains(d)))
                    .find_map(|p| p.spending.clone())
            } else {
                None
            };
            let recorded_parts = paid && spending.is_none();
            // Every recorded part, on whatever days are picked - "recorded for Fri 13: 3 000" over
            // Saturday too - but the one the line's own expense already shows; and, ticked for
            // some days, how much is left for the others.
            let parts_note = {
                // Every day it is needed on is the whole trip's amount.
                let total = whole_of(id);
                let mut bits: Vec<String> = purchase
                    .parts
                    .iter()
                    .filter(|p| p.spending.as_deref().is_some_and(exists) && p.spending != spending)
                    .map(|p| {
                        let its: Vec<u32> = p.days.iter().copied().filter(|d| needed.contains(d)).collect();
                        let share = if total > 0.0 { (need_of(id, &its) / total * 100.0).round() as i64 } else { 0 };
                        let money = p
                            .spending
                            .as_deref()
                            .and_then(|sid| tour.spendings.iter().find(|s| s.id.as_str() == sid))
                            .map(|s| money_in(tour.convert(s.amount, &s.currency), &unit))
                            .unwrap_or_default();
                        (t().menu.recorded_part)(&days_label(&menu, &its), share, &money)
                    })
                    .collect();
                let left: Vec<u32> = needed.iter().copied().filter(|d| !covered.contains(d)).collect();
                if !purchase.bought && !covered.is_empty() && !left.is_empty() {
                    bits.push((t().menu.left_for)(&to_buy(need_of(id, &left), n.product), &days_label(&menu, &left)));
                }
                bits.join(" · ")
            };
            let hay_of = |buyer: &str, place: &str, note: &str| {
                folded(&format!("{} {place} {category} {buyer} {note} {} {}", n.product.name, whom(n.product.eaters), why.join(" ")))
            };
            let name_of = |who: Option<&str>| {
                who.and_then(|w| shoppers.iter().find(|(id, _)| id == w)).map(|(_, name)| name.clone()).unwrap_or_default()
            };
            // The rest, after what is split off - over some days, its share of their amount.
            let whole = whole_of(id);
            let rest = if whole > 0.0 { (whole - purchase.split_off()).max(0.0) / whole } else { 1.0 };
            let own_place = menu.places.iter().find(|p| p.id == n.product.place).map(|p| p.name.clone()).unwrap_or_default();
            let note = purchase.note.clone().unwrap_or_default();
            let base = Line {
                product: n.product.clone(),
                amount: n.amount * rest,
                total: n.amount,
                by_people: menu.by_people(&n.product.id),
                split: None,
                splits: purchase.splits.len(),
                who: purchase.who.clone(),
                hay: hay_of(&buyer, &place_name, &note),
                note,
                purchase: purchase.clone(),
                why: why.join(" · "),
                place_order: place_order(&place_name),
                place: place_name,
                own_place: own_place.clone(),
                category_order: categories.iter().position(|c| *c == category).unwrap_or(usize::MAX),
                category: category.clone(),
                order: menu.products.iter().position(|p| p.id == n.product.id).unwrap_or(usize::MAX),
                days,
                tick_days,
                ticked,
                spending,
                recorded_parts,
                parts_note,
            };
            // Each split-off line is for the whole trip: over some days, there until ticked.
            let mut out: Vec<Line> = purchase
                .splits
                .iter()
                .filter(|x| !subset || !x.bought)
                .map(|x| {
                    let place = x.place.clone().unwrap_or_else(|| own_place.clone());
                    let note = x.note.clone().unwrap_or_default();
                    Line {
                        amount: x.amount,
                        total: x.amount,
                        split: Some(x.id.clone()),
                        who: x.who.clone(),
                        hay: hay_of(&name_of(x.who.as_deref()), &place, &note),
                        note,
                        place_order: place_order(&place),
                        place,
                        days: Vec::new(),
                        tick_days: Vec::new(),
                        ticked: x.bought,
                        spending: x.spending.clone().filter(|s| exists(s)),
                        recorded_parts: false,
                        parts_note: String::new(),
                        ..base.clone()
                    }
                })
                .collect();
            // Nothing of it needed over these days: only its split-off lines.
            if n.amount > 0.0 {
                out.insert(0, base);
            }
            out
        })
        .collect()
}

/// A list's count, of what is on screen: "2 of 47 bought", "2 of 15 bought · 15 of 47 shown"
/// with lines hidden, and the picked person's share after it with everybody's showing. A row
/// is (who buys it, what a search looks at, whether it is ticked).
fn tally_text(state: MenuState, rows: &[(Option<String>, String, bool)], shoppers: &[(String, String)]) -> String {
    let total = rows.len();
    let shown: Vec<&(Option<String>, String, bool)> =
        rows.iter().filter(|(who, hay, _)| wanted(state, who.as_deref(), hay)).collect();
    let bought = shown.iter().filter(|r| r.2).count();
    let count = if shown.len() == total {
        (t().menu.bought_of)(bought, total)
    } else {
        (t().menu.shown_bought_of)(bought, shown.len(), total)
    };
    let me = state.me.get().filter(|_| state.everybody.get());
    let theirs: Vec<bool> = shown.iter().filter(|r| r.0.is_some() && r.0 == me).map(|r| r.2).collect();
    match me.and_then(|me| shoppers.iter().find(|(id, _)| *id == me).map(|(_, name)| name.clone())) {
        Some(name) if !theirs.is_empty() => format!(
            "{count} · {}",
            (t().menu.theirs_bought_of)(&name, theirs.iter().filter(|b| **b).count(), theirs.len())
        ),
        _ => count,
    }
}

/// The shopping in Mini: a line a product - tick, name, how much, who buys it - under "Shopping
/// by" and the days. The roomy list's own lines ([`shopping_lines`]) and count, so the two
/// cannot disagree; who buys, where, notes and expenses are set in Full.
#[component]
pub fn MiniShopping(tour: Tour, state: MenuState, apply: Callback<Operation>, mode: RwSignal<crate::mode::UiMode>) -> impl IntoView {
    let Some(menu) = Menu::shown(&tour) else {
        return view! { <div class="tcm-empty">{t().menu.nothing_to_buy}</div> }.into_any();
    };
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));
    let eating = Eating::of(&tour);
    let tour_id = tour.id.as_str().to_owned();
    let shoppers: Vec<(String, String)> = crate::edit::sorted_people(&tour)
        .into_iter()
        .filter(|p| p.parent.is_none())
        .map(|p| (p.id.as_str().to_owned(), p.name.clone()))
        .collect();
    forget_unknown_me(state, &shoppers);
    let plan_days = menu.days;
    let in_plan = move |v: Vec<u32>| -> Vec<u32> { v.into_iter().filter(|d| (1..=plan_days).contains(d)).collect() };

    let me_options = {
        let chosen = state.me.get_untracked().unwrap_or_default();
        shoppers
            .iter()
            .map(|(id, name)| view! { <option value=id.clone() selected=*id == chosen>{name.clone()}</option> })
            .collect_view()
    };
    let bar = view! {
        <div class="tcm-bar">
            <span class="tcm-hint">{t().menu.who_am_i}</span>
            <select class="tcm-input" on:change=move |ev| {
                let v = event_target_value(&ev);
                let me = (!v.is_empty()).then_some(v);
                crate::settings::set_me(&tour_id, me.as_deref());
                state.me.set(me);
            }>
                <option value="" selected=state.me.get_untracked().is_none()>{t().menu.nobody}</option>
                {me_options}
            </select>
            <Show when=move || state.me.get().is_some()>
                <label class="tcm-check">
                    <input type="checkbox" prop:checked=move || !state.everybody.get()
                           on:change=move |ev| state.everybody.set(!event_target_checked(&ev)) />
                    {t().menu.mine_only}
                </label>
            </Show>
            <label class="tcm-check">
                <input type="checkbox" prop:checked=move || state.hide_bought.get()
                       on:change=move |ev| state.set_hide_bought(event_target_checked(&ev)) />
                {t().menu.hide_bought}
            </label>
        </div>
    };
    let days = (menu.days > 1).then(|| {
        let total = menu.days as usize;
        let chips = menu
            .all_days()
            .into_iter()
            .map(|day| view! {
                <button type="button" class="tcm-btn" class:is-on=move || in_plan(state.days.get()).contains(&day)
                        on:click=move |_| state.days.update(|v| {
                            v.retain(|d| (1..=plan_days).contains(d));
                            if v.contains(&day) {
                                v.retain(|d| *d != day);
                            } else {
                                v.push(day);
                                v.sort_unstable();
                            }
                            if v.len() == total {
                                v.clear();
                            }
                        })>
                    {day_short(&menu, day)}
                </button>
            })
            .collect_view();
        view! {
            <div class="tcm-bar">
                <span class="tcm-hint">{t().menu.shopping_for}</span>
                <button type="button" class="tcm-btn" class:is-on=move || in_plan(state.days.get()).is_empty()
                        on:click=move |_| state.days.set(Vec::new())>
                    {t().menu.days_all}
                </button>
                {chips}
            </div>
        }
    });

    // Redrawn when the days change: the amounts are theirs.
    let list = move || {
        let lines = shopping_lines(&tour, &menu, &eating, &in_plan(state.days.get()), &shoppers);
        if lines.is_empty() {
            return view! { <div class="tcm-empty">{t().menu.nothing_to_buy}</div> }.into_any();
        }
        let rows: Vec<(Option<String>, String, bool)> = lines.iter().map(|l| (l.who.clone(), l.hay.clone(), l.ticked)).collect();
        let tally = {
            let shoppers = shoppers.clone();
            move || tally_text(state, &rows, &shoppers)
        };
        let items = lines
            .iter()
            .map(|line| {
                let id = line.product.id.clone();
                let split = line.split.clone();
                let days = line.tick_days.clone();
                let tick = move |ev: leptos::ev::Event| {
                    let bought = event_target_checked(&ev);
                    change(match &split {
                        Some(s) => MenuEdit::SplitChange { product: id.clone(), id: s.clone(), change: SplitChange::Bought(bought) },
                        None if days.is_empty() => MenuEdit::Bought { product: id.clone(), bought },
                        None => MenuEdit::BoughtOn { product: id.clone(), days: days.clone(), bought },
                    });
                };
                let who = line.who.clone();
                let buyer = who
                    .as_deref()
                    .and_then(|w| shoppers.iter().find(|(id, _)| id == w))
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| "—".to_owned());
                let visible = {
                    let (who, hay, ticked) = (who.clone(), line.hay.clone(), line.ticked);
                    move || visible_line(state, who.as_deref(), &hay, ticked)
                };
                let is_mine = {
                    let who = who.clone();
                    move || state.everybody.get() && state.me.get().is_some_and(|me| who.as_deref() == Some(me.as_str()))
                };
                let name = if line.split.is_some() {
                    format!("{} · {}", line.product.name, t().menu.split_part)
                } else {
                    line.product.name.clone()
                };
                let title = [line.place.as_str(), line.why.as_str(), line.note.as_str()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                view! {
                    <div class="tcm-item tcm-buy" class:is-dim=line.ticked class:is-mine=is_mine
                         style:display=move || if visible() { "" } else { "none" }>
                        <div class="tcm-row">
                            <input type="checkbox" class="tcm-buy-tick" prop:checked=line.ticked
                                   title=t().menu.bought on:change=tick />
                            <span class="tcm-main" title=title><span class="tcm-name">{name}</span></span>
                            <span class="tcm-buy-amount">{amount_text(line)}</span>
                            <span class="tcm-buy-who">{buyer}</span>
                        </div>
                    </div>
                }
            })
            .collect_view();
        view! {
            <div class="tcm-caption">{tally}</div>
            <div class="tcm-list">{items}</div>
        }
        .into_any()
    };

    // The search is Full's own, shared: typed there, it narrows the list here too - so it is
    // here to see and to clear.
    let find = view! {
        <div class="tcm-bar">
            <input class="tcm-find" type="text" placeholder=t().menu.find_product
                   prop:value=move || state.shop_find.get()
                   on:input=move |ev| state.shop_find.set(event_target_value(&ev)) />
            <Show when=move || !state.shop_find.get().is_empty()>
                <button type="button" class="tcm-btn" title=t().menu.clear_find
                        on:click=move |_| state.shop_find.set(String::new())>"✕"</button>
            </Show>
        </div>
    };

    view! {
        {bar}
        {days}
        {find}
        {list}
        <div class="tcm-bar">
            <span class="tcm-hint">{t().menu.mini_in_full}</span>
            <span class="tcm-spacer"></span>
            <button type="button" class="tcm-btn" on:click=move |_| {
                crate::mode::remember(crate::mode::UiMode::Full);
                state.view.set(View::Shopping);
                mode.set(crate::mode::UiMode::Full);
            }>{t().menu.mini_open_full}</button>
        </div>
    }
    .into_any()
}

/// Under the shopping: what its expenses are filed under, and a way to rename that. Seldom
/// changed, so a line of small print rather than a field - findable where the expenses are
/// made, out of the way of the list.
fn expense_category_line(menu: &Menu, apply: Callback<Operation>) -> impl IntoView {
    let root = menu.expense_category.clone().unwrap_or_else(|| t().menu.expense_root.to_owned());
    let renaming = RwSignal::new(false);
    let was = menu.expense_category.clone().unwrap_or_default();
    let typed = RwSignal::new(was.clone());
    // Closed first, so that the blur that follows the box going away finds it closed and
    // saves nothing more: Escape is "never mind", and Enter has saved already. Saved only
    // when it changed - opened and closed again, the box is no edit.
    let done = move || {
        if !renaming.get_untracked() {
            return;
        }
        renaming.set(false);
        let now = typed.get_untracked();
        if now.trim() != was.trim() {
            apply.run(Operation::Menu(MenuEdit::ExpenseCategory(now)));
        }
    };
    let was_again = menu.expense_category.clone().unwrap_or_default();
    let cancel = move || {
        typed.set(was_again.clone());
        renaming.set(false);
    };
    view! {
        <p class="tcw-food-note tcw-shop-category">
            {t().menu.expense_category_is} " «" <b>{root}</b> "/…» · "
            <Show when=move || !renaming.get()
                  fallback=move || view! {
                      <input class="tcn-input tcw-shop-category-input" type="text" autofocus=true
                             placeholder=t().menu.expense_root
                             prop:value=move || typed.get()
                             on:input=move |ev| typed.set(event_target_value(&ev))
                             on:blur={let done = done.clone(); move |_| done()}
                             on:keydown={
                                 let done = done.clone();
                                 let cancel = cancel.clone();
                                 move |ev| match ev.key().as_str() {
                                     "Enter" => done(),
                                     "Escape" => cancel(),
                                     _ => {}
                                 }
                             } />
                      <span class="tcn-hint">{t().menu.expense_category_hint}</span>
                  }>
                <button type="button" class="tcn-linkbtn" on:click=move |_| renaming.set(true)>
                    {t().menu.expense_category_change}
                </button>
            </Show>
        </p>
    }
}

/// The places to pick from in a product's "where": the catalogue's and the ones typed.
const PLACES_LIST: &str = "tcw-shop-places";

/// One product on the list.
#[derive(Clone)]
struct Line {
    product: Product,
    /// What this line buys: a split-off line's amount, or what the days need less those.
    amount: f64,
    /// What the days need, all lines of the product together.
    total: f64,
    /// A split-off line's id; `None` - the purchase itself, the rest.
    split: Option<String>,
    /// How many lines are split off the product.
    splits: usize,
    /// Who buys this line, and the note for them.
    who: Option<String>,
    note: String,
    /// Counted by people somewhere, so whom it is for means something - see `Menu::by_people`.
    by_people: bool,
    purchase: tc_core::menu::Purchase,
    /// What it is for: "Plov ×1 · Fish ×2", "every day".
    why: String,
    /// Everything a search looks at - name, place, buyer, dishes - [`folded`].
    hay: String,
    /// Where it is bought - its purchase's word, else its catalogue place - and that place's
    /// and the product's own place in the catalogue.
    place: String,
    /// The product's place in the catalogue - where it goes back to when "where" is emptied.
    own_place: String,
    place_order: usize,
    /// Its category, "" for none, and that category's place among the tour's.
    category: String,
    category_order: usize,
    order: usize,
    /// The days a record is for now: those no expense has paid for. Empty: the whole trip.
    days: Vec<u32>,
    /// The days a tick is for: those picked over the list that it is needed on. Empty: the
    /// whole trip.
    tick_days: Vec<u32>,
    /// Bought for the days the list is for.
    ticked: bool,
    /// The expense those days were recorded as.
    spending: Option<String>,
    /// Every day the list is for recorded, by more than one expense.
    recorded_parts: bool,
    /// "bought for Fri 13: 30 % · 2 000 · 2,3 kg left for Sat 14, Sun 15"; empty when it was
    /// not bought a day at a time.
    parts_note: String,
}

/// The shopping list in the order asked for; the catalogue's order breaks every tie, so
/// that the meat stays together within "Dima's".
fn sort_lines(lines: &mut [Line], sort: Sort, shoppers: &[(String, String)]) {
    let buyer = |l: &Line| {
        l.who.as_deref().and_then(|w| shoppers.iter().find(|(id, _)| id == w)).map(|(_, name)| name.to_lowercase())
    };
    match sort {
        Sort::Catalogue => lines.sort_by_key(|l| l.order),
        Sort::Name => lines.sort_by_key(|l| (l.product.name.to_lowercase().replace('ё', "е"), l.order)),
        Sort::Place => lines.sort_by_key(|l| (l.place_order, l.order)),
        Sort::Category => lines.sort_by_key(|l| (l.category_order, l.order)),
        // Nobody's last: those are the ones still to share out.
        Sort::Buyer => lines.sort_by_key(|l| (buyer(l).is_none(), buyer(l), l.order)),
        Sort::NotBought => lines.sort_by_key(|l| (l.ticked, l.order)),
    }
}

/// Who a product is for, as a product's line says it: everybody, the adults, the children.
fn whom(eaters: Eaters) -> &'static str {
    match eaters {
        Eaters::Everyone => t().menu.for_everyone,
        Eaters::FullWeight => t().menu.for_full,
        Eaters::Others => t().menu.for_others,
    }
}

/// "Shopping by" that is nobody who can buy - somebody removed since, a child, a link with
/// a wrong id - is taken as nobody: otherwise "— nobody —" shows in the select while the list
/// keeps only nobody's lines. Not written back: the phone's choice stays as it was.
fn forget_unknown_me(state: MenuState, shoppers: &[(String, String)]) {
    if state.me.get_untracked().is_some_and(|me| !shoppers.iter().any(|(id, _)| *id == me)) {
        state.me.set(None);
    }
}

/// Whether a line is on screen: [`wanted`], and not hidden as bought.
fn visible_line(state: MenuState, who: Option<&str>, hay: &str, ticked: bool) -> bool {
    wanted(state, who, hay) && !(ticked && state.hide_bought.get())
}

/// Whether a line is among those asked for: its buyer's (see [`shown`]) and matching the
/// search. What the count counts - "2 of 15 bought" says how much of them is done, the
/// bought hidden or not.
fn wanted(state: MenuState, who: Option<&str>, hay: &str) -> bool {
    shown(state, who) && found(&state.shop_find.get(), hay)
}

/// Whether a line bought by `who` is on screen: everybody's are, unless somebody is picked
/// as "me" and others' are hidden - then theirs and nobody's, nobody's so that there is
/// something to take.
fn shown(state: MenuState, who: Option<&str>) -> bool {
    match (state.me.get(), who) {
        (Some(me), Some(who)) if !state.everybody.get() => who == me,
        _ => true,
    }
}

/// A new expense for these products, paid by whoever buys them - or, when nobody or
/// several people do, by whom this phone records expenses for. Filed under the shopping's
/// category and the products' own ("Shopping/Alcohol"), and shared by whom they are for, by
/// weight - as they were bought: the grown-ups' wine by the grown-ups. The lines are all for
/// the same people -
/// the shared receipt is split by that first (see `by_whom`).
fn expense_for(tour: &Tour, who: Option<&str>, description: String, lines: &[&Line]) -> SpendingDraft {
    let mut d = match who {
        Some(p) => SpendingDraft::paid_by(tour, &PersonId::new(p)),
        None => SpendingDraft::new(tour),
    };
    d.description = description;
    let menu = Menu::of(tour).unwrap_or_default();
    let products: Vec<&Product> = lines.iter().map(|l| &l.product).collect();
    d.category = menu.expense_category(t().menu.expense_root, &products);
    if let Some(payers) = lines.first().and_then(|l| tc_core::menu::payers_for(tour, l.product.eaters)) {
        d.everyone = false;
        d.to = payers;
    }
    d.purchases = lines.iter().map(|l| l.product.id.clone()).collect();
    d.days_of = lines.iter().map(|l| l.days.clone()).collect();
    d.splits = lines.iter().map(|l| l.split.clone().unwrap_or_default()).collect();
    d
}

/// The days a receipt is for, if all of it is for the same ones - "Shopping for Sat 14 – Tue
/// 17"; else none, and it is plain "Shopping".
fn receipt_days(lines: &[&Line]) -> Vec<u32> {
    match lines.split_first() {
        Some((first, rest)) if rest.iter().all(|l| l.days == first.days) => first.days.clone(),
        _ => Vec::new(),
    }
}

/// "Shopping", or "Shopping for Fri 13" when it is for some days only.
fn shopping_prefix(tour: &Tour, days: &[u32]) -> String {
    let root = shopping_root(tour);
    match (days.is_empty(), Menu::of(tour)) {
        (false, Some(menu)) => (t().menu.root_for_days)(&root, &days_label(&menu, days)),
        _ => root,
    }
}

/// A day, short: "Fri 13" when the plan has dates, else "day 3".
fn day_short(menu: &Menu, day: u32) -> String {
    match menu.start.as_deref().and_then(|s| date_after(s, day.saturating_sub(1))) {
        Some((y, m, d)) => {
            let weekday = (days_from_civil(y, m, d) + 3).rem_euclid(7) as usize;
            format!("{} {d}", t().menu.weekdays_short[weekday])
        }
        None => (t().menu.day_n)(day),
    }
}

/// Days, short, with a run of three or more as one: "Fri 13, Sat 14", "day 2 – day 5".
fn days_label(menu: &Menu, days: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < days.len() {
        let mut j = i;
        while j + 1 < days.len() && days[j + 1] == days[j] + 1 {
            j += 1;
        }
        if j - i >= 2 {
            out.push(format!("{} – {}", day_short(menu, days[i]), day_short(menu, days[j])));
        } else {
            out.extend(days[i..=j].iter().map(|d| day_short(menu, *d)));
        }
        i = j + 1;
    }
    out.join(", ")
}

/// What an expense from the shopping is described as starting with - "Shopping: milk, bread"
/// - the same word its category starts with. Not the place: who bought what where is the
/// shopping list's business, and the expenses list reads better with one word for all of them.
fn shopping_root(tour: &Tour) -> String {
    Menu::of(tour)
        .and_then(|m| m.expense_category)
        .map(|r| r.trim().to_owned())
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| t().menu.expense_root.to_owned())
}

/// Lines by whom they are for - everybody, the grown-ups, the children - in that order: one
/// receipt is shared one way, so a shared one with wine and bread in it is two expenses.
fn by_whom<'a>(lines: &[&'a Line]) -> Vec<(Eaters, Vec<&'a Line>)> {
    [Eaters::Everyone, Eaters::FullWeight, Eaters::Others]
        .into_iter()
        .map(|e| (e, lines.iter().copied().filter(|l| l.product.eaters == e).collect::<Vec<_>>()))
        .filter(|(_, ls)| !ls.is_empty())
        .collect()
}

/// A card of the shopping: one place's, or everything in one list.
/// Who buys a line: the purchase's buyer, or a split-off line's.
fn buyer_edit(product: &str, split: Option<&str>, who: Option<String>) -> MenuEdit {
    match split {
        Some(id) => MenuEdit::SplitChange { product: product.to_owned(), id: id.to_owned(), change: SplitChange::Who(who) },
        None => MenuEdit::Buyer { products: vec![product.to_owned()], who },
    }
}

/// An amount as typed for a line: packs, if the product comes in them - "1" is a pack of 250 g
/// of coffee - else its own unit; and back.
fn typed_amount(text: &str, product: &Product) -> Option<f64> {
    let x = text.trim().replace(',', ".").parse::<f64>().ok().filter(|a| a.is_finite() && *a > 0.0)?;
    Some(x * product.pack_size().unwrap_or(1.0))
}

fn amount_unit(product: &Product) -> String {
    match product.pack_size() {
        Some(size) => product
            .pack_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("× {}", quantity_of(size, product))),
        None => crate::menu_catalogue::unit_label(product),
    }
}

/// What a line buys, at its right: whole packs, or "all split off" for a rest with nothing
/// left; under a rest, "of" the whole. A split-off line's amount is tapped to change it.
/// How much a line buys, as its row says it.
fn amount_text(line: &Line) -> String {
    // A split-off line is what somebody brings, as said - half a pack of coffee is 125 g, not
    // "1 × pack"; in packs only when it is whole ones.
    let whole_packs = line.product.pack_size().is_some_and(|size| {
        let n = line.amount / size;
        n >= 1.0 && (n - n.round()).abs() < 1e-6
    });
    if line.amount <= 0.0 {
        t().menu.all_split.to_owned()
    } else if line.split.is_some() && !whole_packs {
        // As typed: 125 g is 125, not the shop's 120.
        format!("{}\u{a0}{}", crate::menu_catalogue::number((line.amount * 100.0).round() / 100.0), crate::menu_catalogue::unit_label(&line.product))
    } else {
        to_buy(line.amount, &line.product)
    }
}

fn amount_cell(line: &Line, change: impl Fn(MenuEdit) + Copy + Send + Sync + 'static) -> AnyView {
    let text = amount_text(line);
    let Some(split) = line.split.clone() else {
        let of = (line.total > line.amount + 1e-9).then(|| (t().menu.of_total)(&quantity_of(line.total, &line.product)));
        return view! { {text} {of.map(|o| view! { <small class="tcw-buy-total">{o}</small> })} }.into_any();
    };
    let typing = RwSignal::new(false);
    let id = line.product.id.clone();
    let product = line.product.clone();
    let unit = amount_unit(&line.product);
    let value = crate::menu_catalogue::number(((line.amount / line.product.pack_size().unwrap_or(1.0)) * 1000.0).round() / 1000.0);
    view! {
        <Show when=move || typing.get()
              fallback=move || view! {
                  <button type="button" class="tcw-buy-place tcw-buy-qty" on:click=move |_| typing.set(true)>{text.clone()}</button>
              }>
            {
                let (id, split, product, value, unit) = (id.clone(), split.clone(), product.clone(), value.clone(), unit.clone());
                view! {
                    <input type="text" class="tcn-input tcw-share-amount" inputmode="decimal" autofocus=true
                           prop:value=value
                           on:change=move |ev| {
                               typing.set(false);
                               if let Some(x) = typed_amount(&event_target_value(&ev), &product) {
                                   change(MenuEdit::SplitChange { product: id.clone(), id: split.clone(), change: SplitChange::Amount(x) });
                               }
                           }
                           on:blur=move |_| typing.set(false)
                           on:keydown=move |ev| if ev.key() == "Escape" { typing.set(false) } />
                    <small class="tcw-share-unit">{unit}</small>
                }
            }
        </Show>
    }
    .into_any()
}

/// Under a line's name, words that turn into boxes where they are, as "where" does: the note
/// ("+ note", or the note itself), "split" - who and how much, as a line of its own - and,
/// on a split product, taking a part or the rest away. Saved field by field, as the
/// shopping's other choices are; taking away asks first when it has an expense.
fn line_acts(
    line: &Line,
    recorded_as: Option<String>,
    rest_recorded: Option<String>,
    shoppers: &[(String, String)],
    change: impl Fn(MenuEdit) + Copy + Send + Sync + 'static,
) -> AnyView {
    let id = line.product.id.clone();
    let split = line.split.clone();
    let note = line.note.clone();
    let product = line.product.clone();
    let noting = RwSignal::new(false);
    let splitting = RwSignal::new(false);
    let sure = |money: &Option<String>| money.as_deref().is_none_or(|m| ask(&(t().menu.recorded_stays)(m)));

    let note_word = {
        let note = note.clone();
        move || {
            let has = !note.is_empty();
            let text = if has { note.clone() } else { t().menu.note_add.to_owned() };
            view! {
                <button type="button" class="tcw-buy-place tcw-buy-note" class:is-own=has title=t().menu.note_title
                        on:click=move |_| noting.set(true)>{text}</button>
            }
        }
    };
    let note_box = {
        let (id, split, note) = (id.clone(), split.clone(), note.clone());
        move || {
            let (id, split) = (id.clone(), split.clone());
            view! {
                <input type="text" class="tcn-input tcw-buy-at" autofocus=true prop:value=note.clone()
                       placeholder=t().menu.note_placeholder
                       on:change=move |ev| {
                           noting.set(false);
                           let note = event_target_value(&ev);
                           change(match &split {
                               Some(s) => MenuEdit::SplitChange { product: id.clone(), id: s.clone(), change: SplitChange::Note(note) },
                               None => MenuEdit::Note { product: id.clone(), note },
                           });
                       }
                       on:blur=move |_| noting.set(false)
                       on:keydown=move |ev| if ev.key() == "Escape" { noting.set(false) } />
            }
        }
    };
    // The rest only: split some off. A box for how much, a pick of who, and "Split off".
    let split_form = {
        let id = id.clone();
        let product = product.clone();
        let options = shoppers.to_vec();
        move || {
            let typed = RwSignal::new(String::new());
            let to = RwSignal::new(String::new());
            let (id, product, product2) = (id.clone(), product.clone(), product.clone());
            let options = options
                .iter()
                .map(|(pid, name)| view! { <option value=pid.clone()>{name.clone()}</option> })
                .collect_view();
            view! {
                <span class="tcw-share">
                    <select class="tcn-input" title=t().menu.buyer on:change=move |ev| to.set(event_target_value(&ev))>
                        <option value="" selected=true>{t().menu.nobody_yet}</option>
                        {options}
                    </select>
                    <input type="text" class="tcn-input tcw-share-amount" inputmode="decimal" autofocus=true
                           placeholder="1" prop:value=move || typed.get()
                           on:input=move |ev| typed.set(event_target_value(&ev))
                           on:keydown=move |ev| if ev.key() == "Escape" { splitting.set(false) } />
                    <small class="tcw-share-unit">{amount_unit(&product)}</small>
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            prop:disabled=move || typed_amount(&typed.get(), &product2).is_none()
                            on:click=move |_| {
                                let Some(x) = typed_amount(&typed.get_untracked(), &product) else { return };
                                let who = Some(to.get_untracked()).filter(|w| !w.is_empty());
                                splitting.set(false);
                                let split = Split { id: crate::edit::new_id(), who, amount: x, ..Split::default() };
                                change(MenuEdit::SplitOff { product: id.clone(), split });
                            }>
                        {t().menu.split_off}
                    </button>
                    <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-x" title=t().dialogs.cancel
                            on:click=move |_| splitting.set(false)>"✕"</button>
                </span>
            }
        }
    };
    let take_away = match &split {
        Some(s) => {
            let (id, s) = (id.clone(), s.clone());
            Some(view! {
                <button type="button" class="tcw-buy-place tcw-buy-drop" title=t().menu.remove_part_title
                        on:click=move |_| if sure(&recorded_as) { change(MenuEdit::RemoveSplit { product: id.clone(), id: s.clone() }) }>
                    {t().menu.remove_part}
                </button>
            }.into_any())
        }
        None if line.splits > 0 => {
            let id = id.clone();
            Some(view! {
                <button type="button" class="tcw-buy-place tcw-buy-drop" title=t().menu.remove_rest_title
                        on:click=move |_| if sure(&rest_recorded) { change(MenuEdit::DropRest { product: id.clone() }) }>
                    {t().menu.remove_rest}
                </button>
            }.into_any())
        }
        None => None,
    };
    let is_rest = split.is_none();
    view! {
        <small class="tcw-buy-acts">
            <Show when=move || !noting.get() fallback=note_box>{note_word()}</Show>
            {is_rest.then(|| view! {
                " · "
                <Show when=move || splitting.get()
                      fallback=move || view! {
                          <button type="button" class="tcw-buy-place" title=t().menu.split_title
                                  on:click=move |_| splitting.set(true)>{t().menu.split_word}</button>
                      }>
                    {split_form.clone()}
                </Show>
            })}
            {take_away.map(|b| view! { " · " {b} })}
        </small>
    }
    .into_any()
}

#[component]
fn ListCard(
    tour: Tour,
    /// The place, the buyer or the category, or "Groceries" for the one list.
    title: String,
    lines: Vec<Line>,
    shoppers: Vec<(String, String)>,
    state: MenuState,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));
    let unit = crate::ui::unit(&tour);
    // Of the lines on screen, not of the card: with "hide the rest" or a search, "0 of 47"
    // counted lines nobody was looking at.
    // With everybody's lines showing and somebody picked, their share of them follows.
    let tally = {
        let rows: Vec<(Option<String>, String, bool)> =
            lines.iter().map(|l| (l.who.clone(), l.hay.clone(), l.ticked)).collect();
        let shoppers = shoppers.clone();
        move || tally_text(state, &rows, &shoppers)
    };
    let recorded = |l: &Line| {
        l.spending
            .as_deref()
            .and_then(|id| tour.spendings.iter().find(|s| s.id.as_str() == id))
            .cloned()
    };

    // "All to Dima" - every line here not bought yet and on screen, once asked. A bought or
    // recorded line keeps its buyer: the expense says who paid. A line hidden by the search or
    // by "Hide the others" is not "here" to whoever is looking. And it asks: 12.11.2026
    // somebody opening the list for the first time picked their own name here - it looks like
    // "who am I" - and the whole trip's shopping became theirs, Dima's recorded wine included.
    let open_lines: Vec<(String, Option<String>, Option<String>, String)> = lines
        .iter()
        .filter(|l| !l.ticked && l.spending.is_none() && !l.recorded_parts)
        .map(|l| (l.product.id.clone(), l.split.clone(), l.who.clone(), l.hay.clone()))
        .collect();
    let everybody_options = shoppers
        .iter()
        .map(|(id, name)| view! { <option value=id.clone()>{name.clone()}</option> })
        .collect_view();
    // Bumped after every pick, so that the select goes back to "choose…": it is an action,
    // not a setting, and a name left standing in it read as one.
    let picked = RwSignal::new(0u32);
    // The picker shows only after "Assign all…": a select in the card's head, always there,
    // read as "who am I" next to "Shopping by" - and one pick in it was the whole trip's.
    let assigning = RwSignal::new(false);
    let everybody = {
        let shoppers = shoppers.clone();
        move |ev: leptos::ev::Event| {
            let v = event_target_value(&ev);
            picked.update(|n| *n += 1);
            if v == "?" {
                return;
            }
            let who = (!v.is_empty()).then_some(v);
            let these: Vec<&(String, Option<String>, Option<String>, String)> = open_lines
                .iter()
                .filter(|(_, _, now, hay)| *now != who && visible_line(state, now.as_deref(), hay, false))
                .collect();
            if these.is_empty() {
                ask_nothing(t().menu.everybody_none);
                assigning.set(false);
                return;
            }
            let question = match &who {
                Some(id) => {
                    let name = shoppers.iter().find(|(p, _)| p == id).map(|(_, n)| n.as_str()).unwrap_or("?");
                    (t().menu.everybody_ask)(these.len(), name)
                }
                None => (t().menu.everybody_ask_nobody)(these.len()),
            };
            if !ask(&question) {
                return;
            }
            change(MenuEdit::All(these.iter().map(|(product, split, _, _)| buyer_edit(product, split.as_deref(), who.clone())).collect()));
            assigning.set(false);
        }
    };

    // Bought, not recorded, and on screen: one receipt for all of them.
    let unrecorded: Vec<Line> = lines
        .iter()
        .filter(|l| l.ticked && recorded(l).is_none() && !l.recorded_parts)
        .cloned()
        .collect();
    let together = {
        let tour = tour.clone();
        move || {
            let these: Vec<&Line> = unrecorded.iter().filter(|l| shown(state, l.who.as_deref())).collect();
            if these.len() < 2 {
                return None;
            }
            // One button a way of sharing: bread for everybody, wine for the grown-ups.
            let groups = by_whom(&these);
            let several = groups.len() > 1;
            let buttons = groups
                .into_iter()
                .map(|(eaters, these)| {
                    let first = these[0].who.clone();
                    let who = if these.iter().all(|l| l.who == first) { first } else { state.me.get() };
                    let prefix = shopping_prefix(&tour, &receipt_days(&these));
                    let names: Vec<String> = these.iter().map(|l| l.product.name.to_lowercase()).collect();
                    let description = if names.len() > 3 {
                        format!("{}: {} +{}", prefix, names[..3].join(", "), names.len() - 3)
                    } else {
                        format!("{}: {}", prefix, names.join(", "))
                    };
                    let draft = expense_for(&tour, who.as_deref(), description, &these);
                    let n = these.len();
                    let label = if several { (t().menu.record_together_for)(n, whom(eaters)) } else { (t().menu.record_together)(n) };
                    view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                                on:click=move |_| dialog.set(Some(Dialog::Spending(draft.clone())))>
                            {label}
                        </button>
                    }
                })
                .collect_view();
            Some(view! { <div class="tcw-errand-foot">{buttons}</div> })
        }
    };

    let visible_here = {
        let rows: Vec<(Option<String>, String, bool)> = lines.iter().map(|l| (l.who.clone(), l.hay.clone(), l.ticked)).collect();
        move || rows.iter().any(|(who, hay, ticked)| visible_line(state, who.as_deref(), hay, *ticked))
    };

    let rows = lines
        .iter()
        .map(|line| {
            let id = line.product.id.clone();
            let split = line.split.clone();
            let who = line.who.clone();
            let ticked = line.ticked;
            let options = shoppers
                .iter()
                .map(|(pid, name)| {
                    let on = who.as_deref() == Some(pid.as_str());
                    view! { <option value=pid.clone() selected=on>{name.clone()}</option> }
                })
                .collect_view();
            let pick = {
                let id = id.clone();
                let split = split.clone();
                move |ev: leptos::ev::Event| {
                    let v = event_target_value(&ev);
                    change(buyer_edit(&id, split.as_deref(), (!v.is_empty()).then_some(v)));
                }
            };
            // Where: a word under the name, which a tap turns into a box with the places said
            // so far. A box in the row itself was three letters wide on a phone - "Mar" - and,
            // with the catalogue's place as its grey hint, every row looked unset.
            let placing = RwSignal::new(false);
            let place_word = {
                let own = line.place != line.own_place;
                let label = if line.place.is_empty() { t().menu.where_buy.to_owned() } else { line.place.clone() };
                move || {
                    view! {
                        <button type="button" class="tcw-buy-place" class:is-own=own title=t().menu.place_change
                                on:click=move |_| placing.set(true)>{label.clone()}</button>
                    }
                }
            };
            let place_box = {
                let id = id.clone();
                let typed = if line.place != line.own_place { line.place.clone() } else { String::new() };
                let split = split.clone();
                let hint = if line.own_place.is_empty() { t().menu.where_buy.to_owned() } else { line.own_place.clone() };
                move || {
                    let id = id.clone();
                    let split = split.clone();
                    view! {
                        <input type="text" class="tcn-input tcw-buy-at" list=PLACES_LIST autofocus=true
                               prop:value=typed.clone() placeholder=hint.clone() title=t().menu.where_buy_title
                               on:change=move |ev| {
                                   placing.set(false);
                                   let place = event_target_value(&ev);
                                   change(match &split {
                                       Some(s) => MenuEdit::SplitChange { product: id.clone(), id: s.clone(), change: SplitChange::Place(place) },
                                       None => MenuEdit::BuyAt { products: vec![id.clone()], place },
                                   });
                               }
                               on:blur=move |_| placing.set(false)
                               on:keydown=move |ev| if ev.key() == "Escape" { placing.set(false) } />
                    }
                }
            };
            // For the whole trip, or for the days picked over the list.
            let tick = {
                let id = id.clone();
                let days = line.tick_days.clone();
                let split = split.clone();
                move |ev: leptos::ev::Event| {
                    let bought = event_target_checked(&ev);
                    if let Some(s) = &split {
                        change(MenuEdit::SplitChange { product: id.clone(), id: s.clone(), change: SplitChange::Bought(bought) });
                    } else if days.is_empty() {
                        change(MenuEdit::Bought { product: id.clone(), bought });
                    } else {
                        change(MenuEdit::BoughtOn { product: id.clone(), days: days.clone(), bought });
                    }
                }
            };
            // The expense: there, with what it came to - its own, or a receipt shared with
            // other products - or a button to record it.
            let expense = match recorded(line) {
                Some(s) => {
                    let amount = money_in(tour.convert(s.amount, &s.currency), &unit);
                    let shared = lines
                        .iter()
                        .filter(|l| l.spending.as_deref() == Some(s.id.as_str()))
                        .count()
                        > 1;
                    let text = if shared { (t().menu.in_shared)(&amount) } else { format!("✓ {amount}") };
                    view! { <span class="tcw-buy-done" title=s.description.clone()>{text}</span> }.into_any()
                }
                None if line.recorded_parts => {
                    view! { <span class="tcw-buy-done">{t().menu.in_parts}</span> }.into_any()
                }
                None => {
                    let draft = expense_for(
                        &tour,
                        who.as_deref(),
                        format!("{}: {}", shopping_prefix(&tour, &line.days), line.product.name.to_lowercase()),
                        &[line],
                    );
                    view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcw-buy-record"
                                class:tcn-btn-primary=ticked
                                on:click=move |_| dialog.set(Some(Dialog::Spending(draft.clone())))>
                            {t().menu.record}
                        </button>
                    }
                    .into_any()
                }
            };
            let visible = {
                let who = who.clone();
                let hay = line.hay.clone();
                move || visible_line(state, who.as_deref(), &hay, ticked)
            };
            let money_of = |sid: &str| {
                tour.spendings.iter().find(|s| s.id.as_str() == sid).map(|s| money_in(tour.convert(s.amount, &s.currency), &unit))
            };
            let recorded_as = line.spending.as_deref().and_then(money_of);
            let rest_recorded = line.purchase.spending.as_deref().and_then(money_of);
            let acts = line_acts(line, recorded_as, rest_recorded, &shoppers, change);
            let amount_cell = amount_cell(line, change);
            // "To: <the picked one>": one tap instead of finding them in the list - there when
            // somebody is picked in "Shopping by" and the product is not theirs yet. Named, not
            // "Mine": the one picked need not be the one holding the phone.
            let to_me = {
                let id = id.clone();
                let who = who.clone();
                let split = split.clone();
                let shoppers = shoppers.clone();
                move || {
                    let me = state.me.get().filter(|me| who.as_deref() != Some(me.as_str()))?;
                    let name = shoppers.iter().find(|(pid, _)| *pid == me).map(|(_, name)| name.clone())?;
                    let edit = buyer_edit(&id, split.as_deref(), Some(me));
                    Some(view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcw-buy-me" title=(t().menu.to_me_title)(&name)
                                on:click=move |_| change(edit.clone())>
                            {(t().menu.to_me)(&name)}
                        </button>
                    })
                }
            };
            // Whose the line is, marked while the others' are on screen too - hidden, they need
            // no marking.
            let is_mine = {
                let who = who.clone();
                move || state.everybody.get() && state.me.get().is_some_and(|me| who.as_deref() == Some(me.as_str()))
            };
            // Hidden rather than left out, so that picking "me" does not rebuild the list.
            view! {
                    <div class="tcw-buy" class:is-bought=ticked class:is-mine=is_mine
                         style:display=move || if visible() { "" } else { "none" }>
                        <input type="checkbox" class="tcw-buy-tick" prop:checked=ticked
                               title=t().menu.bought on:change=tick />
                        <span class="tcw-buy-name">
                            <span class="tcw-buy-title">{line.product.name.clone()}</span>
                            <small class="tcw-buy-why">
                                {(line.splits > 0).then(|| view! {
                                    <b class="tcw-buy-split">{if line.split.is_some() { t().menu.split_part } else { t().menu.split_rest }}</b>
                                    " · "
                                })}
                                <Show when=move || !placing.get() fallback=place_box>{place_word()}</Show>
                                " · " {line.by_people.then(|| if line.why.is_empty() { whom(line.product.eaters).to_owned() } else { format!("{} · ", whom(line.product.eaters)) })}
                                {line.why.clone()}
                            </small>
                            {(!line.parts_note.is_empty()).then(|| view! {
                                <small class="tcw-buy-parts">{line.parts_note.clone()}</small>
                            })}
                            {acts}
                        </span>
                        <span class="tcw-buy-amount">{amount_cell}</span>
                        <span class="tcw-buy-who">
                            <select class="tcn-input" title=t().menu.buyer on:change=pick>
                                <option value="" selected=who.is_none()>{t().menu.nobody_yet}</option>
                                {options}
                            </select>
                            {to_me}
                        </span>
                        <span class="tcw-buy-expense">{expense}</span>
                    </div>
            }
        })
        .collect_view();

    view! {
            <div class="tcn-card tcw-errand"
                 style:display=move || if visible_here() { "" } else { "none" }>
                <div class="tcw-errand-head">
                    <b class="tcw-errand-place">{title.clone()}</b>
                    <span class="tcw-errand-count">{tally}</span>
                    <button type="button" class="tcn-btn tcn-btn-sm tcw-errand-all"
                            style:display=move || if assigning.get() { "none" } else { "" }
                            on:click=move |_| assigning.set(true)>
                        {t().menu.assign_all}
                    </button>
                </div>
                <div class="tcw-errand-who" style:display=move || if assigning.get() { "" } else { "none" }>
                    <span>{t().menu.everybody}</span>
                    <select class="tcn-input" on:change=everybody
                            prop:value=move || { picked.track(); "?" }>
                        <option value="?" selected=true>{t().menu.pick}</option>
                        <option value="">{t().menu.nobody_yet}</option>
                        {everybody_options}
                    </select>
                    <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| assigning.set(false)>
                        {t().dialogs.cancel}
                    </button>
                </div>
                <div class="tcw-buy-list">{rows}</div>
                {together}
            </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shopping link reads back as the buyer and the switch it was made with, whatever is
    /// in the buyer's id; an address without them asks nothing.
    #[test]
    fn a_shopping_link_names_the_buyer_and_the_others() {
        for (buyer, others, bought) in [("p1", false, true), ("a b&c=d/é", true, false)] {
            let link = shopping_link("CODE", "tour1", buyer, others, bought);
            let (path, query) = link.split_once('?').expect("a query");
            assert_eq!(path, "/goto/CODE/tour1/menu/shopping");
            assert_eq!(
                link_asks(&format!("?{query}")),
                Asks { buyer: Some(buyer.to_owned()), others: Some(others), bought: Some(bought) }
            );
        }
        assert_eq!(link_asks(""), Asks::default());
        assert_eq!(link_asks("?others=1"), Asks { others: Some(true), ..Asks::default() });
        assert_eq!(link_asks("?bought=0"), Asks { bought: Some(false), ..Asks::default() });
        assert_eq!(link_asks("?buyer=&x=1"), Asks::default());
        assert_eq!(link_asks("?buyer=Sasha+O.%"), Asks { buyer: Some("Sasha O.%".to_owned()), ..Asks::default() });
    }

    /// What "Copy menu" writes reads back as the catalogue alone; so does a whole tour's JSON;
    /// anything else is not a menu.
    #[test]
    fn a_copied_menu_pastes_back_as_its_catalogue() {
        let mut trip = starter(3);
        trip.purchase_mut("rice").bought = true;
        let back = menu_from_text(&menu_as_text(&trip)).expect("a menu");
        assert_eq!(back, trip.as_template());
        assert!(back.plan.is_empty() && back.purchases.is_empty());

        let mut tour = Tour::from_json("{\"Id\": \"t\", \"Name\": \"t\"}").expect("tour");
        trip.put(&mut tour);
        let whole = tour.to_json().expect("json");
        assert_eq!(menu_from_text(&whole), Some(trip.as_template()), "a tour's JSON");

        assert_eq!(menu_from_text("hello"), None);
        assert_eq!(menu_from_text("{\"Name\": \"a tour without a menu\"}"), None);
        assert_eq!(menu_from_text(&menu_as_text(&Menu::default())), None, "an empty one");
    }

    /// Every dish and every daily line names a product the catalogue has - a typo there
    /// would quietly leave something off every shopping list.
    #[test]
    fn the_starter_names_only_products_it_has() {
        let m = starter(5);
        let known = |id: &str| m.product(id).is_some();
        for d in &m.dishes {
            for i in &d.ingredients {
                assert!(known(&i.product), "{} in {}", i.product, d.id);
            }
        }
        for i in &m.daily {
            assert!(known(&i.product), "{} daily", i.product);
        }
        for p in &m.products {
            assert!(m.places.iter().any(|x| x.id == p.place), "{} at {}", p.id, p.place);
        }
        assert_eq!(m.plan.len(), 15, "five days, three meals");
    }

    fn tour() -> Tour {
        Tour::from_json(&serde_json::json!({
            "Id": "t", "Name": "t",
            "Persons": [{"GUID": "a", "Name": "A", "Weight": 100}, {"GUID": "b", "Name": "B", "Weight": 35}]
        }).to_string()).expect("tour")
    }

    fn switched(tour: &Tour, on: bool) -> Tour {
        let mut d = crate::edit::TourDraft::of(tour);
        d.menu = Some(on);
        crate::edit::put_tour(tour, &d)
    }

    /// A tour whose menu was started from the starter, for the tests that need its dishes.
    fn from_starter(tour: &Tour) -> Tour {
        let mut started = tour.clone();
        starter(5).put(&mut started);
        started
    }

    /// Switched on, the tour gets an empty catalogue for as many days as it lasts; off and on
    /// again, the plan is as it was left.
    #[test]
    fn switching_the_menu_off_keeps_it() {
        let on = switched(&tour(), true);
        let menu = Menu::shown(&on).expect("a menu");
        assert_eq!(menu.days, 5, "the tour's length when it has none written");
        let edited = crate::edit::put_menu(&on, &MenuEdit::Days(2));
        let off = switched(&edited, false);
        assert!(Menu::shown(&off).is_none());
        assert_eq!(Menu::of(&off).map(|m| m.days), Some(2));
        assert_eq!(Menu::shown(&switched(&off, true)).map(|m| m.days), Some(2));
        // An edit queued before the menu existed says nothing about it.
        let mut old = crate::edit::TourDraft::of(&on);
        old.menu = None;
        assert!(Menu::shown(&crate::edit::put_tour(&on, &old)).is_some());
    }

    /// Switched on for the first time, the menu has no dishes, products or places: the
    /// catalogue is filled in later, and the plan is for the tour's days.
    #[test]
    fn switching_the_menu_on_starts_with_an_empty_catalogue() {
        let mut d = crate::edit::TourDraft::of(&tour());
        d.days = 3;
        d.menu = Some(true);
        let on = crate::edit::put_tour(&tour(), &d);
        let menu = Menu::shown(&on).expect("a menu");
        assert!(menu.on);
        assert!(menu.dishes.is_empty() && menu.products.is_empty() && menu.places.is_empty());
        assert_eq!(menu.days, 3);
    }

    /// Switched on for the first time with a template (an edit queued by an older client), the
    /// menu starts from the template's catalogue, planned for the tour's days; taking a
    /// template later keeps what still fits.
    #[test]
    fn a_template_starts_the_menu_and_can_be_taken_later() {
        let mut template = starter(3).as_template();
        template.dishes.retain(|d| d.id != "grill");
        let mut d = crate::edit::TourDraft::of(&tour());
        d.menu = Some(true);
        d.template = Some(template.clone());
        let on = crate::edit::put_tour(&tour(), &d);
        let menu = Menu::shown(&on).expect("a menu");
        assert_eq!(menu.dishes, template.dishes);
        assert_eq!(menu.days, 5);
        assert!(menu.plan.len() >= 5, "planned, not empty");
        assert!(menu.plan.iter().all(|s| !s.dishes.iter().any(|d| d == "grill")));

        // A tour started from the starter takes the template: the grill is gone from its plan,
        // and its dinners are planned again from what the template has.
        let started = from_starter(&tour());
        assert!(Menu::shown(&started).expect("a menu").plan.iter().any(|s| s.dishes.iter().any(|d| d == "grill")));
        let taken = crate::edit::put_menu(&started, &MenuEdit::TakeTemplate(template.clone()));
        let menu = Menu::shown(&taken).expect("a menu");
        assert_eq!(menu.dishes, template.dishes);
        assert!(menu.plan.iter().all(|s| !s.dishes.iter().any(|d| d == "grill")));
        assert_eq!(menu.plan.len(), Menu::shown(&started).expect("a menu").plan.len(), "no meal left unplanned");
    }

    /// Clearing is taking an empty template: no dishes, products or daily list, nothing
    /// planned or bought - and the places still there.
    #[test]
    fn clearing_the_catalogue_keeps_the_places() {
        let started = from_starter(&tour());
        let places = Menu::shown(&started).expect("a menu").places;
        let empty = Menu { on: true, places: places.clone(), ..Menu::default() };
        let cleared = crate::edit::put_menu(&started, &MenuEdit::TakeTemplate(empty));
        let m = Menu::shown(&cleared).expect("still a menu");
        assert!(m.dishes.is_empty() && m.products.is_empty() && m.daily.is_empty());
        assert!(m.plan.iter().all(|s| s.dishes.is_empty()) && m.purchases.is_empty());
        assert_eq!(m.places, places);
        assert_eq!(m.days, 5, "the trip keeps its length");
    }

    /// The dishes cleared on their own: the products and the daily list stay, and nothing is
    /// planned; the daily list cleared leaves the dishes alone.
    #[test]
    fn dishes_and_the_daily_list_clear_apart() {
        let started = from_starter(&tour());
        let before = Menu::shown(&started).expect("a menu");
        let all = MenuEdit::All(before.dishes.iter().map(|d| MenuEdit::RemoveDish(d.id.clone())).collect());
        let m = Menu::shown(&crate::edit::put_menu(&started, &all)).expect("a menu");
        assert!(m.dishes.is_empty());
        assert_eq!((m.products.len(), m.daily.len()), (before.products.len(), before.daily.len()));
        assert!(m.plan.iter().all(|s| s.dishes.is_empty()));

        let m = Menu::shown(&crate::edit::put_menu(&started, &MenuEdit::Daily(Vec::new()))).expect("a menu");
        assert!(m.daily.is_empty());
        assert_eq!(m.dishes, before.dishes);
    }

    /// A typed name is taken by a template of the list only when the names are the same, in any
    /// case and with blanks cut; a new name is not taken by a trip or another code's template.
    #[test]
    fn a_taken_name_is_the_template_that_has_it() {
        let make = |id: &str, name: &str| {
            Tour::from_json(&serde_json::json!({
                "Id": id, "Name": name, "AccessCodeMD5": "HIKE", "IsMenuTemplate": true
            }).to_string()).expect("tour")
        };
        let tours = vec![make("t1", "Новый год"), make("t2", "  Походное меню ")];
        assert_eq!(taken_by("  походное МЕНЮ ", &tours).map(|found| found.id.as_str()), Some("t2"));
        assert_eq!(taken_by("Новый год", &tours).map(|found| found.id.as_str()), Some("t1"));
        assert!(taken_by("Новый", &tours).is_none(), "a prefix is another name");
        assert!(taken_by("Новый год", &[]).is_none(), "nothing listed takes nothing");
    }

    /// A code can have several templates: all of them under its code, sorted by name - not the
    /// trips, the other codes' templates, or the template the list is asked from.
    #[test]
    fn a_code_has_all_its_templates_by_name() {
        let make = |id: &str, name: &str, code: &str, template: bool| {
            Tour::from_json(&serde_json::json!({
                "Id": id, "Name": name, "AccessCodeMD5": code, "IsMenuTemplate": template
            }).to_string()).expect("tour")
        };
        let tours = vec![
            make("t1", "Новый год", "HIKE", true),
            make("t2", "  походное меню ", "HIKE", true),
            make("t3", "Danube", "HIKE", false),
            make("t4", "Apple", "HIKE", true),
            make("t5", "Office", "WORK", true),
            make("t0", "Mine", "HIKE", true),
        ];
        let mine = make("t0", "Mine", "HIKE", true);
        let found: Vec<String> = templates_among(&tours, &mine).iter().map(|t| t.id.as_str().to_owned()).collect();
        assert_eq!(found, ["t4", "t1", "t2"]);
        let names: Vec<String> = templates_among(&tours, &make("t3", "Danube", "HIKE", false))
            .iter().map(|t| t.name.trim().to_owned()).collect();
        assert_eq!(names, ["Apple", "Mine", "Новый год", "походное меню"]);
        assert!(templates_among(&tours, &make("t9", "x", "NONE", false)).is_empty());
    }

    /// A template is listed by its name, blanks cut; one without a name has the generic title. A
    /// new template's name counts only if something other than blanks was typed.
    #[test]
    fn a_template_is_named_as_typed() {
        assert_eq!(template_label("  Новый год "), "Новый год");
        assert_eq!(template_label("   "), t().menu.template_title);
        assert_eq!(new_template_name("  Походное меню  "), Some("Походное меню".to_owned()));
        assert_eq!(new_template_name(" \t"), None);
    }

    /// A template says whose it is by the trips under its own code, not the other code's.
    #[test]
    fn a_template_is_known_by_its_codes_tours() {
        let make = |id: &str, name: &str, code: &str, template: bool| {
            Tour::from_json(&serde_json::json!({
                "Id": id, "Name": name, "AccessCodeMD5": code, "IsMenuTemplate": template
            }).to_string()).expect("tour")
        };
        let tours = vec![
            make("a", "Danube", "HIKE", false),
            make("b", "Office party", "WORK", false),
            make("c", "Shipka", "HIKE", false),
            make("d", "Ural", "HIKE", false),
            make("e", "Kayaks", "HIKE", false),
        ];
        let hike = make("t1", "template", "HIKE", true);
        let work = make("t2", "template", "WORK", true);
        let lonely = make("t3", "template", "NONE", true);
        assert_eq!(template_owners(&hike, &tours), (t().list.template_for)("Danube, Shipka", 2));
        assert_eq!(template_owners(&work, &tours), (t().list.template_for)("Office party", 0));
        assert_eq!(template_owners(&lonely, &tours), t().list.template_for_none);
    }

    /// Coffee split: a pack from Sasha's home and the rest by Sergey. Recording Sasha's line
    /// points his line at the expense, not the rest; taking the rest away leaves Sasha's line
    /// as the whole purchase, his expense with it.
    #[test]
    fn a_split_off_line_has_its_own_expense() {
        use crate::edit::{put_menu, put_spending, MenuEdit};
        let t = switched(&tour(), true);
        let t = put_menu(&t, &MenuEdit::Buyer { products: vec!["coffee".into()], who: Some("b".into()) });
        let split = Split { id: "s".into(), who: Some("a".into()), amount: 250.0, ..Split::default() };
        let t = put_menu(&t, &MenuEdit::SplitOff { product: "coffee".into(), split: split.clone() });
        let t = put_menu(&t, &MenuEdit::SplitOff { product: "coffee".into(), split });
        let t = put_menu(&t, &MenuEdit::SplitChange { product: "coffee".into(), id: "s".into(), change: SplitChange::Place("From home".into()) });
        // Built whole: `new` asks the browser for today's date.
        let d = SpendingDraft {
            entered_at: None,
            id: Some(tc_core::SpendingId::new("e1")),
            description: "Shopping: coffee".into(),
            category: "Shopping".into(),
            amount: tc_core::Cents(1500),
            from: PersonId::new("a"),
            everyone: true,
            to: Vec::new(),
            by_weight: true,
            date: "2026-11-07".into(),
            colour: String::new(),
            currency_id: t.currency().id.as_str().to_owned(),
            editing: false,
            in_cents: None,
            on_behalf: true,
            purchases: vec!["coffee".into()],
            days_of: vec![Vec::new()],
            splits: vec!["s".into()],
        };
        let t = put_spending(&t, &d);
        let p = Menu::of(&t).and_then(|m| m.purchase("coffee").cloned()).expect("coffee");
        assert_eq!(p.splits.len(), 1, "split off once, however often replayed");
        assert_eq!((p.spending.as_deref(), p.splits[0].spending.as_deref(), p.splits[0].place.as_deref()), (None, Some("e1"), Some("From home")));
        let t = put_menu(&t, &MenuEdit::DropRest { product: "coffee".into() });
        let p = Menu::of(&t).and_then(|m| m.purchase("coffee").cloned()).expect("coffee");
        assert_eq!((p.who.as_deref(), p.spending.as_deref(), p.splits.len()), (Some("a"), Some("e1"), 0));
    }

    /// One receipt for the meat and the lamb: both point at it, and both are bought.
    #[test]
    fn an_expense_recorded_from_the_shopping_is_its_expense() {
        let t = switched(&tour(), true);
        // Built whole: `paid_by` asks the browser for today's date.
        let d = SpendingDraft {
            entered_at: None,
            id: Some(tc_core::SpendingId::new("s1")),
            description: "Market: groceries".into(),
            category: "Groceries".into(),
            amount: tc_core::Cents(4200),
            from: PersonId::new("a"),
            everyone: true,
            to: Vec::new(),
            by_weight: true,
            date: "2026-09-30".into(),
            colour: String::new(),
            currency_id: t.currency().id.as_str().to_owned(),
            editing: false,
            in_cents: None,
            on_behalf: true,
            purchases: vec!["meat".into(), "lamb".into()],
            days_of: Vec::new(),
            splits: Vec::new(),
        };
        let after = crate::edit::put_spending(&t, &d);
        let menu = Menu::of(&after).expect("menu");
        for product in ["meat", "lamb"] {
            let p = menu.purchase(product).expect("a purchase");
            // Recorded, not ticked: the tick is given by hand.
            assert_eq!((p.spending.as_deref(), p.bought), (Some("s1"), false), "{product}");
        }
        assert!(after.spendings.iter().any(|s| s.id.as_str() == "s1"));
        // The same expense edited later, from the Expenses tab, leaves the pointer alone.
        let mut plain = SpendingDraft::of(after.spendings.iter().find(|s| s.id.as_str() == "s1").unwrap());
        plain.amount = tc_core::Cents(5000);
        let edited = crate::edit::put_spending(&after, &plain);
        assert_eq!(Menu::of(&edited).and_then(|m| m.purchase("lamb").and_then(|p| p.spending.clone())).as_deref(), Some("s1"));
    }

    /// Recorded for day 1 only: a part for day 1 points at it, and the product is not bought
    /// for the whole trip; the tick for day 1 it replaces is gone.
    #[test]
    fn an_expense_for_some_days_records_their_part() {
        let mut t = switched(&tour(), true);
        t = crate::edit::put_menu(&t, &MenuEdit::BoughtOn { product: "lamb".into(), days: vec![1], bought: true });
        let d = SpendingDraft {
            entered_at: None,
            id: Some(tc_core::SpendingId::new("s2")),
            description: "Shopping for day 1: lamb".into(),
            category: "Shopping".into(),
            amount: tc_core::Cents(2000),
            from: PersonId::new("a"),
            everyone: true,
            to: Vec::new(),
            by_weight: true,
            date: "2026-09-30".into(),
            colour: String::new(),
            currency_id: t.currency().id.as_str().to_owned(),
            editing: false,
            in_cents: None,
            on_behalf: true,
            purchases: vec!["lamb".into()],
            days_of: vec![vec![1]],
            splits: Vec::new(),
        };
        let after = crate::edit::put_spending(&t, &d);
        let m = Menu::of(&after).expect("menu");
        let p = m.purchase("lamb").cloned().expect("a purchase");
        assert!(!p.bought, "not the whole trip");
        assert_eq!(p.parts, vec![tc_core::menu::Part { days: vec![1], spending: Some("s2".into()) }]);
        assert_eq!(m.covered_days("lamb"), vec![1], "the tick is the one given by hand");
        // Unticked for the whole trip: the expense stays where it was.
        let none = crate::edit::put_menu(&after, &MenuEdit::Bought { product: "lamb".into(), bought: false });
        let m = Menu::of(&none).expect("menu");
        assert!(m.covered_days("lamb").is_empty());
        assert_eq!(m.purchase("lamb").expect("lamb").parts, p.parts);
        // One receipt: rice for the whole trip, the rest of the lamb for day 2. Each is
        // recorded as what it was bought for - rice not as a part for day 2.
        let mut both = d.clone();
        both.id = Some(tc_core::SpendingId::new("s3"));
        both.purchases = vec!["rice".into(), "lamb".into()];
        both.days_of = vec![Vec::new(), vec![2]];
        let m = Menu::of(&crate::edit::put_spending(&after, &both)).expect("menu");
        let rice = m.purchase("rice").expect("rice");
        assert!(!rice.bought && rice.parts.is_empty(), "recorded, not ticked");
        assert_eq!(rice.spending.as_deref(), Some("s3"));
        assert_eq!(m.recorded_days("lamb", |_| true), vec![1, 2]);
        assert_eq!(m.covered_days("lamb"), vec![1]);

        // Day 1's expense deleted: recording day 1 again replaces its part rather than adding
        // a second one beside it.
        let gone = crate::edit::remove_spending(&after, &tc_core::SpendingId::new("s2"));
        let mut again = d.clone();
        again.id = Some(tc_core::SpendingId::new("s4"));
        let m = Menu::of(&crate::edit::put_spending(&gone, &again)).expect("menu");
        assert_eq!(m.purchase("lamb").expect("lamb").parts, vec![tc_core::menu::Part { days: vec![1], spending: Some("s4".into()) }]);
    }

    #[test]
    fn a_day_is_called_by_its_date() {
        assert_eq!(date_after("2026-11-13", 0), Some((2026, 11, 13)));
        assert_eq!(date_after("2026-11-13", 2), Some((2026, 11, 15)));
        assert_eq!(date_after("2026-12-31", 1), Some((2027, 1, 1)));
        assert_eq!(date_after("2028-02-28", 1), Some((2028, 2, 29)), "a leap year");
        assert_eq!(date_after("", 0), None);
        // 13 November 2026 is a Friday.
        assert_eq!(day_date((2026, 11, 13)), (t().menu.dated)(t().menu.weekdays[4], 13, t().menu.months[10]));
    }

    /// Sold in packs: whole packs of the trip's total, named by the pack's word or its size.
    #[test]
    fn a_pack_is_bought_whole() {
        let milk = Product { unit: Unit::Millilitre, pack: Some(1000.0), ..Product::default() };
        assert_eq!(to_buy(1910.0, &milk), format!("2 × 1\u{a0}{}", t().menu.litres));
        let lavash = Product { unit: Unit::Piece, pack: Some(10.0), pack_name: Some("пачка по 10".into()), ..Product::default() };
        assert_eq!(to_buy(13.0, &lavash), "2 × пачка по 10");
        let lemons = Product { unit: Unit::Piece, ..Product::default() };
        assert_eq!(to_buy(4.01, &lemons), quantity(4.01, Unit::Piece), "no pack: as before");
    }

    #[test]
    fn an_own_unit_is_counted_in_whole_ones() {
        let wine = Product { own_unit: Some("бут.".into()), unit: Unit::Piece, ..Product::default() };
        assert_eq!(quantity_of(5.6, &wine), "6\u{a0}бут.");
        let blank = Product { own_unit: Some("  ".into()), unit: Unit::Millilitre, ..Product::default() };
        assert_eq!(quantity_of(500.0, &blank), quantity(500.0, Unit::Millilitre), "an empty unit is none");
    }

    /// Every word somewhere, case and "ё" aside; nothing typed finds everything.
    #[test]
    fn a_search_finds_every_word_anywhere() {
        let hay = folded("Шашлык Рынок Дима Т. Быстромясо на гриле ×1");
        assert!(found("", &hay));
        assert!(found("рынок дима", &hay));
        assert!(found("ГРИЛЕ", &hay));
        assert!(!found("рынок женя", &hay));
        assert!(found("ёлка", &folded("Елка")));
    }

    /// The next trip keeps the plan and the catalogue, not last year's shopping.
    #[test]
    fn a_copy_for_the_next_trip_has_nothing_bought() {
        let mut t = tour();
        let mut m = starter(3);
        m.purchase_mut("rice").bought = true;
        m.purchase_mut("rice").who = Some("a".into());
        m.purchase_mut("rice").spending = Some("s1".into());
        m.put(&mut t);
        let next = crate::edit::for_next_trip(&t);
        let copied = Menu::of(&next).expect("menu");
        assert!(copied.purchases.is_empty());
        assert_eq!((copied.days, copied.dishes.len(), copied.plan.len()), (m.days, m.dishes.len(), m.plan.len()));
    }

    #[test]
    fn quantities_are_rounded_to_what_a_shop_sells() {
        assert_eq!(quantity(1270.0, Unit::Gram), format!("1{}3\u{a0}{}", crate::ui::decimal(), t().menu.kilograms));
        // 34 g short of 1.234 kg is under 5 %: not a reason for another 100 g.
        assert_eq!(quantity(1234.0, Unit::Gram), format!("1{}2\u{a0}{}", crate::ui::decimal(), t().menu.kilograms));
        assert_eq!(quantity(2000.0, Unit::Millilitre), format!("2\u{a0}{}", t().menu.litres));
        assert_eq!(quantity(348.0, Unit::Gram), format!("350\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(333.0, Unit::Gram), format!("330\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(2.25, Unit::Piece), format!("3\u{a0}{}", t().menu.pieces));
        // Salt for an omelette, 2 g a portion, 5.05 portions: a day is 10 g, two are 20 -
        // not 20 and 30, as rounding 10.1 up to the next 10 made it.
        assert_eq!(quantity(10.1, Unit::Gram), format!("10\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(20.2, Unit::Gram), format!("20\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(15.0, Unit::Gram), format!("15\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(5.05, Unit::Piece), format!("5\u{a0}{}", t().menu.pieces));
    }

    /// A day added to the plan is a day more of the trip; the tour's length changed in its
    /// dialog leaves the plan alone.
    #[test]
    fn the_menus_days_are_the_tours_and_not_the_other_way() {
        let started = switched(&tour(), true);
        let longer = crate::edit::put_menu(&started, &MenuEdit::Days(7));
        assert_eq!(tc_core::extras::int_of(&longer.extras, tc_core::extras::DURATION), Some(7));
        assert_eq!(Menu::shown(&longer).expect("a menu").days, 7);

        let mut d = crate::edit::TourDraft::of(&longer);
        d.days = 3;
        let shorter = crate::edit::put_tour(&longer, &d);
        assert_eq!(tc_core::extras::int_of(&shorter.extras, tc_core::extras::DURATION), Some(3));
        assert_eq!(Menu::shown(&shorter).expect("a menu").days, 7, "the plan keeps its days");
    }
}
