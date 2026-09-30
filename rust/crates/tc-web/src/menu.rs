//! The Menu tab: what is cooked on which day, and the shopping it takes.
//!
//! The model and the arithmetic are `tc_core::menu`; this is the starter catalogue a tour
//! gets when its menu is first switched on, and the two screens - the plan by days, and
//! the shopping by places, each with whoever does it, a tick per product, and the expense
//! it was recorded as.

use crate::edit::{MenuEdit, SpendingDraft};
use crate::i18n::{t, Lang};
use crate::queue::Operation;
use crate::tour::Dialog;
use crate::ui::money_in;
use leptos::prelude::*;
use tc_core::menu::{Dish, Eaters, Eating, Ingredient, Meal, Menu, Place, Product, Slot, Unit, When};
use tc_core::{PersonId, Tour};

/// Which of the tab's three views is open.
#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Plan,
    Shopping,
    Catalogue,
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
    pub daily_open: RwSignal<bool>,
    pub products_open: RwSignal<bool>,
    /// Who this phone is: the shopping shows theirs.
    pub me: RwSignal<Option<String>>,
    /// Other people's shopping too, although somebody is picked. Without it the shopping
    /// shows theirs and nobody's - nobody's so that there is something to take.
    pub everybody: RwSignal<bool>,
}

impl MenuState {
    pub fn new(tour: &str) -> MenuState {
        MenuState {
            view: RwSignal::new(View::Plan),
            editing: RwSignal::new(None),
            dishes_open: RwSignal::new(true),
            daily_open: RwSignal::new(true),
            products_open: RwSignal::new(false),
            me: RwSignal::new(crate::settings::me(tour)),
            everybody: RwSignal::new(false),
        }
    }
}

// ---- the starter catalogue ------------------------------------------------------------

const MARKET: &str = "market";
const SUPERMARKET: &str = "supermarket";
const DELIVERY: &str = "delivery";

/// Both languages side by side: it is data, written into the tour in the language of
/// whoever switches the menu on, and a pair per row keeps the two from drifting apart.
const PLACES: &[(&str, &str, &str)] = &[
    (MARKET, "Market", "Рынок"),
    (SUPERMARKET, "Supermarket", "Супермаркет"),
    (DELIVERY, "Delivery", "Доставка"),
];

use Eaters::{Everyone as ALL, FullWeight as FULL, Others as KIDS};
use Unit::{Gram as G, Millilitre as ML, Piece as PCS};

/// id, English, Russian, unit, where, for whom.
const PRODUCTS: &[(&str, &str, &str, Unit, &str, Eaters)] = &[
    ("meat", "Meat for the grill (pork, chicken)", "Мясо на гриль (свинина, курица)", G, MARKET, ALL),
    ("lamb", "Lamb", "Баранина", G, MARKET, ALL),
    ("fish", "Fish", "Рыба", G, MARKET, ALL),
    ("mince", "Mince", "Фарш", G, MARKET, ALL),
    ("potatoes", "Potatoes", "Картошка", G, MARKET, ALL),
    ("carrots", "Carrots", "Морковь", G, MARKET, ALL),
    ("onions", "Onions", "Лук", G, MARKET, ALL),
    ("salad", "Salad vegetables", "Овощи на салат", G, MARKET, ALL),
    ("herbs", "Herbs", "Зелень", G, MARKET, ALL),
    ("fruit", "Fruit", "Фрукты", G, MARKET, ALL),
    ("lemons", "Lemons", "Лимоны", PCS, MARKET, ALL),
    ("cheese", "Cheese", "Сыр", G, MARKET, ALL),
    ("rice", "Rice", "Рис", G, SUPERMARKET, ALL),
    ("pasta", "Pasta", "Макароны", G, SUPERMARKET, ALL),
    ("tomatoes", "Tinned tomatoes", "Томаты в банке", G, SUPERMARKET, ALL),
    ("oats", "Oats", "Овсянка", G, SUPERMARKET, ALL),
    ("muesli", "Muesli", "Мюсли", G, SUPERMARKET, ALL),
    ("eggs", "Eggs", "Яйца", PCS, SUPERMARKET, ALL),
    ("milk", "Milk", "Молоко", ML, SUPERMARKET, ALL),
    ("yoghurt", "Yoghurt", "Йогурт", G, SUPERMARKET, ALL),
    ("butter", "Butter", "Сливочное масло", G, SUPERMARKET, ALL),
    ("oil", "Oil", "Растительное масло", ML, SUPERMARKET, ALL),
    ("bread", "Bread", "Хлеб", G, SUPERMARKET, ALL),
    ("ham", "Ham, sausage", "Ветчина, колбаса", G, SUPERMARKET, ALL),
    ("nuts", "Nuts", "Орехи", G, SUPERMARKET, ALL),
    ("snacks", "Olives, pickles", "Оливки, соленья", G, SUPERMARKET, ALL),
    ("coffee", "Coffee and tea", "Кофе и чай", G, SUPERMARKET, ALL),
    ("wine", "Wine", "Вино", PCS, SUPERMARKET, FULL),
    ("juice", "Juice", "Сок", ML, SUPERMARKET, KIDS),
    ("mineral", "Mineral water", "Минералка", ML, DELIVERY, KIDS),
    ("water", "Drinking water", "Питьевая вода", ML, DELIVERY, ALL),
    ("cooking_water", "Water for cooking", "Вода для готовки", ML, DELIVERY, ALL),
];

/// id, English, Russian, meal, and per portion: product and amount.
type DishRow = (&'static str, &'static str, &'static str, &'static [Meal], &'static [(&'static str, f64)]);
const DISHES: &[DishRow] = &[
    ("omelette", "Omelette", "Омлет", &[Meal::Breakfast], &[("eggs", 3.0), ("milk", 50.0), ("butter", 10.0)]),
    ("porridge", "Porridge", "Каша", &[Meal::Breakfast], &[("oats", 60.0), ("milk", 200.0), ("butter", 10.0)]),
    ("yoghurt", "Yoghurt and muesli", "Йогурт с мюсли", &[Meal::Breakfast], &[("yoghurt", 200.0), ("muesli", 50.0), ("fruit", 100.0)]),
    ("sandwiches", "Sandwiches", "Бутерброды", &[Meal::Breakfast, Meal::Lunch], &[("bread", 100.0), ("cheese", 50.0), ("ham", 60.0), ("butter", 10.0)]),
    ("fruit_nuts", "Fruit and nuts", "Фрукты и орехи", &[Meal::Lunch], &[("fruit", 250.0), ("nuts", 40.0)]),
    ("grill", "Grilled meat", "Быстромясо на гриле", &[Meal::Dinner], &[("meat", 300.0), ("potatoes", 200.0)]),
    ("lamb_steaks", "Lamb steaks", "Стейки баранины", &[Meal::Dinner], &[("lamb", 300.0), ("potatoes", 200.0)]),
    ("fish", "Fish", "Рыба", &[Meal::Dinner], &[("fish", 300.0), ("rice", 70.0), ("lemons", 0.25)]),
    ("plov", "Plov", "Плов", &[Meal::Dinner], &[("rice", 100.0), ("lamb", 150.0), ("carrots", 120.0), ("onions", 60.0), ("oil", 20.0)]),
    ("pasta", "Pasta bolognese", "Паста болоньезе", &[Meal::Dinner], &[("pasta", 100.0), ("mince", 120.0), ("tomatoes", 100.0), ("onions", 30.0)]),
];

/// Every day, whatever is cooked: per portion, or per head for wine and the children's
/// drinks. Wine with dinner - arriving for Friday dinner and leaving after Sunday
/// breakfast is two evenings of it.
const DAILY: &[(&str, f64, When)] = &[
    ("salad", 300.0, When::EveryDay),
    ("herbs", 20.0, When::EveryDay),
    ("fruit", 200.0, When::EveryDay),
    ("bread", 150.0, When::EveryDay),
    ("snacks", 40.0, When::EveryDay),
    ("coffee", 15.0, When::EveryDay),
    ("water", 1500.0, When::EveryDay),
    ("cooking_water", 1000.0, When::EveryDay),
    // Half a bottle each.
    ("wine", 0.5, When::With(Meal::Dinner)),
    ("juice", 500.0, When::EveryDay),
    ("mineral", 500.0, When::EveryDay),
];

/// The menu a tour starts with: three places, the usual products and dishes, and a plan
/// that takes the dishes in turn - every amount and every dish there to be changed.
pub fn starter(days: u32) -> Menu {
    let ru = crate::i18n::lang() == Lang::Ru;
    let pick = |en: &str, ru_: &str| if ru { ru_.to_owned() } else { en.to_owned() };
    let mut menu = Menu {
        on: true,
        days: days.max(1),
        start: None,
        places: PLACES
            .iter()
            .map(|(id, en, r)| Place { id: (*id).into(), name: pick(en, r) })
            .collect(),
        products: PRODUCTS
            .iter()
            .map(|(id, en, r, unit, place, eaters)| Product {
                id: (*id).into(),
                name: pick(en, r),
                unit: *unit,
                place: (*place).into(),
                eaters: *eaters,
                // Wine by the bottle: nobody buys 5 600 ml of it.
                own_unit: (*id == "wine").then(|| pick("btl", "бут.")),
            })
            .collect(),
        dishes: DISHES
            .iter()
            .map(|(id, en, r, meals, items)| Dish {
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
            .map(|(product, amount, when)| Ingredient { product: (*product).into(), amount: *amount, when: *when })
            .collect(),
        plan: Vec::new(),
        purchases: Vec::new(),
    };
    menu.fill();
    menu
}

// ---- amounts -----------------------------------------------------------------------------

/// So much of a product, as a shop sells it: grams up to 10, kilograms and litres to a
/// tenth, pieces whole - always rounded up, since the point is to have enough.
pub fn quantity_of(amount: f64, product: &Product) -> String {
    match product.own_unit.as_deref().filter(|u| !u.trim().is_empty()) {
        Some(own) => format!("{}\u{a0}{}", amount.ceil(), own.trim()),
        None => quantity(amount, product.unit),
    }
}

pub fn quantity(amount: f64, unit: Unit) -> String {
    let m = &t().menu;
    let tenths = |x: f64| {
        let v = (x * 10.0).ceil() / 10.0;
        let text = if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
        text.replace('.', &crate::ui::decimal().to_string())
    };
    match unit {
        Unit::Piece => format!("{}\u{a0}{}", amount.ceil(), m.pieces),
        Unit::Gram if amount >= 1000.0 => format!("{}\u{a0}{}", tenths(amount / 1000.0), m.kilograms),
        Unit::Millilitre if amount >= 1000.0 => format!("{}\u{a0}{}", tenths(amount / 1000.0), m.litres),
        Unit::Gram => format!("{}\u{a0}{}", (amount / 10.0).ceil() * 10.0, m.grams),
        Unit::Millilitre => format!("{}\u{a0}{}", (amount / 10.0).ceil() * 10.0, m.millilitres),
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

    let tour_for_shopping = tour.clone();
    let menu_for_shopping = menu.clone();
    let menu_for_catalogue = menu.clone();

    view! {
        <div class="tcw-food">
            <div class="tcw-food-bar">
                <div class="tcw-seg" role="group">
                    <button type="button" class:is-on=move || state.view.get() == View::Plan
                            on:click=move |_| state.view.set(View::Plan)>{t().menu.plan}</button>
                    <button type="button" class:is-on=move || state.view.get() == View::Shopping
                            on:click=move |_| state.view.set(View::Shopping)>{t().menu.shopping}</button>
                    <button type="button" class:is-on=move || state.view.get() == View::Catalogue
                            on:click=move |_| state.view.set(View::Catalogue)>{t().menu.catalogue}</button>
                </div>
                <div class="tcw-food-days">
                    <span>{t().menu.days}</span>
                    <button type="button" class="tcn-btn tcn-btn-sm" title=t().menu.fewer_days
                            disabled=days <= 1
                            on:click=move |_| change(MenuEdit::Days(days.saturating_sub(1).max(1)))>"−"</button>
                    <b>{days}</b>
                    <button type="button" class="tcn-btn tcn-btn-sm" title=t().menu.more_days
                            on:click=move |_| change(MenuEdit::Days(days + 1))>"+"</button>
                </div>
            </div>
            <p class="tcw-food-note">{note}</p>

            <Show when=move || state.view.get() == View::Plan>
                <Plan menu=menu.clone() apply=apply />
            </Show>
            <Show when=move || state.view.get() == View::Shopping>
                <Shopping tour=tour_for_shopping.clone() menu=menu_for_shopping.clone() eating=eating
                          state=state apply=apply dialog=dialog />
            </Show>
            <Show when=move || state.view.get() == View::Catalogue>
                <crate::menu_catalogue::Catalogue menu=menu_for_catalogue.clone() state=state apply=apply />
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
    let inside = move |day: u32, meal: Meal| {
        !(day == 1 && index(meal) < index(arrival)) && !(day == days && index(meal) > index(departure))
    };

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

    // A dish for one meal on every day at once - "yoghurt every morning" - on the days the
    // trip has that meal.
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
                                let dish = (!v.is_empty()).then_some(v);
                                change(MenuEdit::Meals((1..=days).filter(|d| inside(*d, meal)).map(|day| Slot { day, meal, dish: dish.clone() }).collect()));
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
            let meals = Meal::ALL
                .into_iter()
                .map(|meal| {
                    let chosen = menu.dish_on(day, meal).map(|d| d.id.clone()).unwrap_or_default();
                    let options = menu
                        .dishes_for(meal)
                        .map(|d| {
                            let on = d.id == chosen;
                            view! { <option value=d.id.clone() selected=on>{d.name.clone()}</option> }
                        })
                        .collect_view();
                    view! {
                        <label class="tcw-food-meal">
                            <span class=format!("tcw-meal {}", crate::menu_catalogue::meal_class(meal))>{meal_name(meal)}</span>
                            <select class="tcn-input"
                                    on:change=move |ev| {
                                        let v = event_target_value(&ev);
                                        let dish = (!v.is_empty()).then_some(v);
                                        change(MenuEdit::Meals(vec![Slot { day, meal, dish }]));
                                    }>
                                <option value="" selected=chosen.is_empty()>{t().menu.nothing}</option>
                                {options}
                            </select>
                        </label>
                    }
                })
                .collect_view();
            let (title, small) = match menu.start.as_deref().and_then(|s| date_after(s, day - 1)) {
                Some(date) => (day_date(date), Some((t().menu.day_n)(day))),
                None => ((t().menu.day)(day), None),
            };
            view! {
                <div class="tcn-card tcw-food-day">
                    <div class="tcw-food-dayname">
                        {title}
                        {small.map(|s| view! { " " <small>{s}</small> })}
                    </div>
                    {meals}
                </div>
            }
        })
        .collect_view();

    let daily = menu
        .daily
        .iter()
        .filter_map(|i| menu.product(&i.product).map(|p| (p, i.when)))
        .map(|(p, when)| {
            let mut notes: Vec<&str> = Vec::new();
            match p.eaters {
                Eaters::Everyone => {}
                Eaters::FullWeight => notes.push(t().menu.for_full),
                Eaters::Others => notes.push(t().menu.for_others),
            }
            if let When::With(meal) = when {
                notes.push(when_name(When::With(meal)));
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
            <Show when=move || state.me.get().is_some()>
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || !state.everybody.get()
                           on:change=move |ev| state.everybody.set(!event_target_checked(&ev)) />
                    {t().menu.mine_only}
                </label>
            </Show>
        </div>
    };

    // Everything to buy, owned, with what has been said about each.
    let lines: Vec<Line> = menu
        .shopping(&eating)
        .into_iter()
        .map(|n| {
            let (dishes, daily) = menu.sources(&n.product.id);
            let mut why: Vec<String> = dishes.iter().map(|(d, times)| format!("{} ×{times}", d.name)).collect();
            if let Some(when) = daily {
                why.push(format!("{} ×{}", when_name(when), menu.days_for(when)));
            }
            Line {
                product: n.product.clone(),
                amount: n.amount,
                purchase: menu.purchase(&n.product.id).cloned().unwrap_or_default(),
                why: why.join(" · "),
            }
        })
        .collect();
    if lines.is_empty() {
        return view! { {who_am_i} <div class="tcn-empty">{t().menu.nothing_to_buy}</div> }.into_any();
    }

    // Somebody picked and everything taken by others: say so, rather than an empty screen.
    let anything_shown = {
        let whos: Vec<Option<String>> = lines.iter().map(|l| l.purchase.who.clone()).collect();
        move || whos.iter().any(|who| shown(state, who.as_deref()))
    };

    let places = menu
        .places
        .iter()
        .filter_map(|place| {
            let here: Vec<Line> = lines.iter().filter(|l| l.product.place == place.id).cloned().collect();
            (!here.is_empty()).then(|| view! {
                <PlaceCard tour=tour.clone() place=place.clone() lines=here shoppers=shoppers.clone()
                           state=state apply=apply dialog=dialog />
            })
        })
        .collect_view();

    view! {
        {who_am_i}
        <Show when=move || !anything_shown()>
            <div class="tcn-empty">{t().menu.nothing_mine}</div>
        </Show>
        {places}
    }
    .into_any()
}

/// One product on the list.
#[derive(Clone)]
struct Line {
    product: Product,
    amount: f64,
    purchase: tc_core::menu::Purchase,
    /// What it is for: "Plov ×1 · Fish ×2", "every day".
    why: String,
}

/// Whether a product bought by `who` is on screen: everybody's are, unless somebody is
/// picked as "me" and others' are hidden - then theirs and nobody's, nobody's so that there
/// is something to take.
fn shown(state: MenuState, who: Option<&str>) -> bool {
    match (state.me.get(), who) {
        (Some(me), Some(who)) if !state.everybody.get() => who == me,
        _ => true,
    }
}

/// A new expense for these products, paid by whoever buys them - or, when nobody or
/// several people do, by whom this phone records expenses for.
fn expense_for(tour: &Tour, who: Option<&str>, description: String, products: Vec<String>) -> SpendingDraft {
    let mut d = match who {
        Some(p) => SpendingDraft::paid_by(tour, &PersonId::new(p)),
        None => SpendingDraft::new(tour),
    };
    d.description = description;
    d.category = t().menu.category.to_owned();
    d.purchases = products;
    d
}

/// One place's shopping.
#[component]
fn PlaceCard(
    tour: Tour,
    place: Place,
    lines: Vec<Line>,
    shoppers: Vec<(String, String)>,
    state: MenuState,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));
    let bought = lines.iter().filter(|l| l.purchase.bought).count();
    let total = lines.len();
    let unit = crate::ui::unit(&tour);
    let recorded = |l: &Line| {
        l.purchase
            .spending
            .as_deref()
            .and_then(|id| tour.spendings.iter().find(|s| s.id.as_str() == id))
            .cloned()
    };

    // "Everybody: Dima" - every product here at once.
    let all_ids: Vec<String> = lines.iter().map(|l| l.product.id.clone()).collect();
    let everybody_options = shoppers
        .iter()
        .map(|(id, name)| view! { <option value=id.clone()>{name.clone()}</option> })
        .collect_view();
    let everybody = move |ev: leptos::ev::Event| {
        let v = event_target_value(&ev);
        if v == "?" {
            return;
        }
        change(MenuEdit::Buyer { products: all_ids.clone(), who: (!v.is_empty()).then_some(v) });
    };

    // Bought, not recorded, and on screen: one receipt for all of them.
    let unrecorded: Vec<Line> = lines
        .iter()
        .filter(|l| l.purchase.bought && recorded(l).is_none())
        .cloned()
        .collect();
    let together = {
        let tour = tour.clone();
        let place_name = place.name.clone();
        move || {
            let these: Vec<&Line> = unrecorded.iter().filter(|l| shown(state, l.purchase.who.as_deref())).collect();
            if these.len() < 2 {
                return None;
            }
            let first = these[0].purchase.who.clone();
            let who = if these.iter().all(|l| l.purchase.who == first) { first } else { state.me.get() };
            let names: Vec<String> = these.iter().map(|l| l.product.name.to_lowercase()).collect();
            let description = if names.len() > 3 {
                format!("{}: {} +{}", place_name, names[..3].join(", "), names.len() - 3)
            } else {
                format!("{}: {}", place_name, names.join(", "))
            };
            let draft = expense_for(&tour, who.as_deref(), description, these.iter().map(|l| l.product.id.clone()).collect());
            let n = these.len();
            Some(view! {
                <div class="tcw-errand-foot">
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                            on:click=move |_| dialog.set(Some(Dialog::Spending(draft.clone())))>
                        {(t().menu.record_together)(n)}
                    </button>
                </div>
            })
        }
    };

    let visible_here = {
        let whos: Vec<Option<String>> = lines.iter().map(|l| l.purchase.who.clone()).collect();
        move || whos.iter().any(|who| shown(state, who.as_deref()))
    };

    let rows = lines
        .iter()
        .map(|line| {
            let id = line.product.id.clone();
            let who = line.purchase.who.clone();
            let ticked = line.purchase.bought;
            let options = shoppers
                .iter()
                .map(|(pid, name)| {
                    let on = who.as_deref() == Some(pid.as_str());
                    view! { <option value=pid.clone() selected=on>{name.clone()}</option> }
                })
                .collect_view();
            let pick = {
                let id = id.clone();
                move |ev: leptos::ev::Event| {
                    let v = event_target_value(&ev);
                    change(MenuEdit::Buyer { products: vec![id.clone()], who: (!v.is_empty()).then_some(v) });
                }
            };
            let tick = {
                let id = id.clone();
                move |ev: leptos::ev::Event| change(MenuEdit::Bought { product: id.clone(), bought: event_target_checked(&ev) })
            };
            // The expense: there, with what it came to - its own, or a receipt shared with
            // other products - or a button to record it.
            let expense = match recorded(line) {
                Some(s) => {
                    let amount = money_in(tour.convert(s.amount, &s.currency), &unit);
                    let shared = lines
                        .iter()
                        .filter(|l| l.purchase.spending.as_deref() == Some(s.id.as_str()))
                        .count()
                        > 1;
                    let text = if shared { (t().menu.in_shared)(&amount) } else { format!("✓ {amount}") };
                    view! { <span class="tcw-buy-done" title=s.description.clone()>{text}</span> }.into_any()
                }
                None => {
                    let draft = expense_for(
                        &tour,
                        who.as_deref(),
                        format!("{}: {}", place.name, line.product.name.to_lowercase()),
                        vec![id.clone()],
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
                move || shown(state, who.as_deref())
            };
            // Hidden rather than left out, so that picking "me" does not rebuild the list.
            view! {
                    <div class="tcw-buy" class:is-bought=ticked
                         style:display=move || if visible() { "" } else { "none" }>
                        <input type="checkbox" class="tcw-buy-tick" prop:checked=ticked
                               title=t().menu.bought on:change=tick />
                        <span class="tcw-buy-name">
                            <span class="tcw-buy-title">{line.product.name.clone()}</span>
                            <small class="tcw-buy-why">{line.why.clone()}</small>
                        </span>
                        <span class="tcw-buy-amount">{quantity_of(line.amount, &line.product)}</span>
                        <select class="tcn-input tcw-buy-who" title=t().menu.buyer on:change=pick>
                            <option value="" selected=who.is_none()>{t().menu.nobody_yet}</option>
                            {options}
                        </select>
                        <span class="tcw-buy-expense">{expense}</span>
                    </div>
            }
        })
        .collect_view();

    view! {
            <div class="tcn-card tcw-errand"
                 style:display=move || if visible_here() { "" } else { "none" }>
                <div class="tcw-errand-head">
                    <b class="tcw-errand-place">{place.name.clone()}</b>
                    <span class="tcw-errand-count">{(t().menu.bought_of)(bought, total)}</span>
                    <label class="tcw-errand-who">
                        <span>{t().menu.everybody}</span>
                        <select class="tcn-input" on:change=everybody>
                            <option value="?" selected=true>{t().menu.pick}</option>
                            <option value="">{t().menu.nobody_yet}</option>
                            {everybody_options}
                        </select>
                    </label>
                </div>
                <div class="tcw-buy-list">{rows}</div>
                {together}
            </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Switched on, the tour gets the starter for as many days as it lasts; off and on
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

    /// One receipt for the meat and the lamb: both point at it, and both are bought.
    #[test]
    fn an_expense_recorded_from_the_shopping_is_its_expense() {
        let t = switched(&tour(), true);
        // Built whole: `paid_by` asks the browser for today's date.
        let d = SpendingDraft {
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
        };
        let after = crate::edit::put_spending(&t, &d);
        let menu = Menu::of(&after).expect("menu");
        for product in ["meat", "lamb"] {
            let p = menu.purchase(product).expect("a purchase");
            assert_eq!((p.spending.as_deref(), p.bought), (Some("s1"), true), "{product}");
        }
        assert!(after.spendings.iter().any(|s| s.id.as_str() == "s1"));
        // The same expense edited later, from the Expenses tab, leaves the pointer alone.
        let mut plain = SpendingDraft::of(after.spendings.iter().find(|s| s.id.as_str() == "s1").unwrap());
        plain.amount = tc_core::Cents(5000);
        let edited = crate::edit::put_spending(&after, &plain);
        assert_eq!(Menu::of(&edited).and_then(|m| m.purchase("lamb").and_then(|p| p.spending.clone())).as_deref(), Some("s1"));
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

    #[test]
    fn an_own_unit_is_counted_in_whole_ones() {
        let wine = Product { own_unit: Some("бут.".into()), unit: Unit::Piece, ..Product::default() };
        assert_eq!(quantity_of(5.6, &wine), "6\u{a0}бут.");
        let blank = Product { own_unit: Some("  ".into()), unit: Unit::Millilitre, ..Product::default() };
        assert_eq!(quantity_of(500.0, &blank), quantity(500.0, Unit::Millilitre), "an empty unit is none");
    }

    #[test]
    fn quantities_are_rounded_up_to_what_a_shop_sells() {
        assert_eq!(quantity(1234.0, Unit::Gram), format!("1{}3\u{a0}{}", crate::ui::decimal(), t().menu.kilograms));
        assert_eq!(quantity(2000.0, Unit::Millilitre), format!("2\u{a0}{}", t().menu.litres));
        assert_eq!(quantity(333.0, Unit::Gram), format!("340\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(2.25, Unit::Piece), format!("3\u{a0}{}", t().menu.pieces));
    }
}
