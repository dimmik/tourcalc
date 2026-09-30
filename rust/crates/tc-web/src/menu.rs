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
use tc_core::menu::{Dish, Eaters, Eating, Ingredient, Meal, Menu, Place, Product, Slot, Unit};
use tc_core::{PersonId, Tour};

/// What is open on the tab, owned by the page so that a save does not reset it.
#[derive(Clone, Copy)]
pub struct MenuState {
    pub shopping: RwSignal<bool>,
    /// Who this phone is: the shopping shows theirs.
    pub me: RwSignal<Option<String>>,
    /// Other people's shopping too, although somebody is picked. Without it the shopping
    /// shows theirs and nobody's - nobody's so that there is something to take.
    pub everybody: RwSignal<bool>,
}

impl MenuState {
    pub fn new(tour: &str) -> MenuState {
        MenuState {
            shopping: RwSignal::new(false),
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
    ("wine", "Wine", "Вино", ML, SUPERMARKET, FULL),
    ("juice", "Juice", "Сок", ML, SUPERMARKET, KIDS),
    ("mineral", "Mineral water", "Минералка", ML, DELIVERY, KIDS),
    ("water", "Drinking water", "Питьевая вода", ML, DELIVERY, ALL),
    ("cooking_water", "Water for cooking", "Вода для готовки", ML, DELIVERY, ALL),
];

/// id, English, Russian, meal, and per portion: product and amount.
type DishRow = (&'static str, &'static str, &'static str, Meal, &'static [(&'static str, f64)]);
const DISHES: &[DishRow] = &[
    ("omelette", "Omelette", "Омлет", Meal::Breakfast, &[("eggs", 3.0), ("milk", 50.0), ("butter", 10.0)]),
    ("porridge", "Porridge", "Каша", Meal::Breakfast, &[("oats", 60.0), ("milk", 200.0), ("butter", 10.0)]),
    ("yoghurt", "Yoghurt and muesli", "Йогурт с мюсли", Meal::Breakfast, &[("yoghurt", 200.0), ("muesli", 50.0), ("fruit", 100.0)]),
    ("sandwiches", "Sandwiches", "Бутерброды", Meal::Lunch, &[("bread", 100.0), ("cheese", 50.0), ("ham", 60.0), ("butter", 10.0)]),
    ("fruit_nuts", "Fruit and nuts", "Фрукты и орехи", Meal::Lunch, &[("fruit", 250.0), ("nuts", 40.0)]),
    ("grill", "Grilled meat", "Быстромясо на гриле", Meal::Dinner, &[("meat", 300.0), ("potatoes", 200.0)]),
    ("lamb_steaks", "Lamb steaks", "Стейки баранины", Meal::Dinner, &[("lamb", 300.0), ("potatoes", 200.0)]),
    ("fish", "Fish", "Рыба", Meal::Dinner, &[("fish", 300.0), ("rice", 70.0), ("lemons", 0.25)]),
    ("plov", "Plov", "Плов", Meal::Dinner, &[("rice", 100.0), ("lamb", 150.0), ("carrots", 120.0), ("onions", 60.0), ("oil", 20.0)]),
    ("pasta", "Pasta bolognese", "Паста болоньезе", Meal::Dinner, &[("pasta", 100.0), ("mince", 120.0), ("tomatoes", 100.0), ("onions", 30.0)]),
];

/// Every day, whatever is cooked: per portion, or per head for wine and the children's
/// drinks.
const DAILY: &[(&str, f64)] = &[
    ("salad", 300.0),
    ("herbs", 20.0),
    ("fruit", 200.0),
    ("bread", 150.0),
    ("snacks", 40.0),
    ("coffee", 15.0),
    ("water", 1500.0),
    ("cooking_water", 1000.0),
    ("wine", 350.0),
    ("juice", 500.0),
    ("mineral", 500.0),
];

/// The menu a tour starts with: three places, the usual products and dishes, and a plan
/// that takes the dishes in turn - every amount and every dish there to be changed.
pub fn starter(days: u32) -> Menu {
    let ru = crate::i18n::lang() == Lang::Ru;
    let pick = |en: &str, ru_: &str| if ru { ru_.to_owned() } else { en.to_owned() };
    let mut menu = Menu {
        on: true,
        days: days.max(1),
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
            })
            .collect(),
        dishes: DISHES
            .iter()
            .map(|(id, en, r, meal, items)| Dish {
                id: (*id).into(),
                name: pick(en, r),
                meal: Some(*meal),
                ingredients: items
                    .iter()
                    .map(|(product, amount)| Ingredient { product: (*product).into(), amount: *amount })
                    .collect(),
            })
            .collect(),
        daily: DAILY
            .iter()
            .map(|(product, amount)| Ingredient { product: (*product).into(), amount: *amount })
            .collect(),
        plan: Vec::new(),
        errands: Vec::new(),
    };
    menu.fill();
    menu
}

// ---- amounts -----------------------------------------------------------------------------

/// So much of a product, as a shop sells it: grams up to 10, kilograms and litres to a
/// tenth, pieces whole - always rounded up, since the point is to have enough.
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

fn meal_name(meal: Meal) -> &'static str {
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

    view! {
        <div class="tcw-food">
            <div class="tcw-food-bar">
                <div class="tcw-seg" role="group">
                    <button type="button" class:is-on=move || !state.shopping.get()
                            on:click=move |_| state.shopping.set(false)>{t().menu.plan}</button>
                    <button type="button" class:is-on=move || state.shopping.get()
                            on:click=move |_| state.shopping.set(true)>{t().menu.shopping}</button>
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

            <Show when=move || !state.shopping.get()>
                <Plan menu=menu.clone() apply=apply />
            </Show>
            <Show when=move || state.shopping.get()>
                <Shopping tour=tour_for_shopping.clone() menu=menu_for_shopping.clone() eating=eating
                          state=state apply=apply dialog=dialog />
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

    // A dish for one meal on every day at once - "yoghurt every morning".
    let every_day = Meal::ALL
        .into_iter()
        .map(|meal| {
            let options = menu
                .dishes_for(meal)
                .map(|d| view! { <option value=d.id.clone()>{d.name.clone()}</option> })
                .collect_view();
            view! {
                <label class="tcw-food-meal">
                    <span>{meal_name(meal)}</span>
                    <select class="tcn-input"
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                if v == "?" { return; }
                                let dish = (!v.is_empty()).then_some(v);
                                change(MenuEdit::Meals((1..=days).map(|day| Slot { day, meal, dish: dish.clone() }).collect()));
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
                            <span>{meal_name(meal)}</span>
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
            view! {
                <div class="tcn-card tcw-food-day">
                    <div class="tcw-food-dayname">{(t().menu.day)(day)}</div>
                    {meals}
                </div>
            }
        })
        .collect_view();

    let daily = menu
        .daily
        .iter()
        .filter_map(|i| menu.product(&i.product))
        .map(|p| match p.eaters {
            Eaters::Everyone => p.name.clone(),
            Eaters::FullWeight => format!("{} ({})", p.name, t().menu.for_full),
            Eaters::Others => format!("{} ({})", p.name, t().menu.for_others),
        })
        .collect::<Vec<_>>()
        .join(" · ");

    view! {
        <div class="tcn-card tcw-food-day tcw-food-all">
            <div class="tcw-food-dayname">{t().menu.every_day}</div>
            {every_day}
        </div>
        <div class="tcw-food-days-list">{day_cards}</div>
        <p class="tcw-food-note"><b>{t().menu.daily}</b> " " {daily}</p>
    }
}

/// The shopping, place by place: who goes, a tick per product, and the expense.
#[component]
fn Shopping(
    tour: Tour,
    menu: Menu,
    eating: Eating,
    state: MenuState,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    // Owned, so that nothing below borrows the menu it came from.
    let needs: Vec<(Product, f64)> = menu
        .shopping(&eating)
        .into_iter()
        .map(|n| (n.product.clone(), n.amount))
        .collect();
    let tour_id = tour.id.as_str().to_owned();
    // Whoever can do the shopping: people who pay for themselves, not the ones paid for.
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

    if needs.is_empty() {
        return view! { {who_am_i} <div class="tcn-empty">{t().menu.nothing_to_buy}</div> }.into_any();
    }

    let places = menu
        .places
        .iter()
        .filter(|place| needs.iter().any(|(p, _)| p.place == place.id))
        .map(|place| {
            let errand = menu.errand(&place.id).cloned().unwrap_or_default();
            let lines: Vec<(Product, f64, bool)> = needs
                .iter()
                .filter(|(p, _)| p.place == place.id)
                .map(|(p, amount)| (p.clone(), *amount, errand.bought.contains(&p.id)))
                .collect();
            let tour = tour.clone();
            let shoppers = shoppers.clone();
            let place = place.clone();
            let spending = errand.spending.clone();
            let who = errand.who.clone();
            let mine = {
                let who = who.clone();
                move || match (state.me.get(), who.as_deref()) {
                    (Some(me), Some(who)) if !state.everybody.get() => who == me,
                    _ => true,
                }
            };
            view! {
                <Show when=mine.clone()>
                    <Errand tour=tour.clone() place=place.clone() lines=lines.clone()
                            who=who.clone() spending=spending.clone()
                            shoppers=shoppers.clone() apply=apply dialog=dialog />
                </Show>
            }
        })
        .collect_view();

    // Somebody picked and every place taken by others: say so, rather than an empty screen.
    let theirs = {
        let menu = menu.clone();
        let needs_places: Vec<String> = menu
            .places
            .iter()
            .filter(|p| needs.iter().any(|(n, _)| n.place == p.id))
            .map(|p| p.id.clone())
            .collect();
        move || match state.me.get() {
            Some(me) if !state.everybody.get() => needs_places.iter().any(|p| {
                let who = menu.errand(p).and_then(|e| e.who.as_deref());
                who.is_none() || who == Some(me.as_str())
            }),
            _ => true,
        }
    };

    view! {
        {who_am_i}
        <Show when=move || !theirs()>
            <div class="tcn-empty">{t().menu.nothing_mine}</div>
        </Show>
        {places}
    }
    .into_any()
}

/// One place's shopping.
#[component]
fn Errand(
    tour: Tour,
    place: Place,
    lines: Vec<(Product, f64, bool)>,
    who: Option<String>,
    spending: Option<String>,
    shoppers: Vec<(String, String)>,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let change = move |edit: MenuEdit| apply.run(Operation::Menu(edit));
    let bought = lines.iter().filter(|(_, _, b)| *b).count();
    let total = lines.len();
    let all_bought = bought == total;

    let place_id = place.id.clone();
    let chosen = who.clone().unwrap_or_default();
    let options = shoppers
        .iter()
        .map(|(id, name)| {
            let on = *id == chosen;
            view! { <option value=id.clone() selected=on>{name.clone()}</option> }
        })
        .collect_view();
    let pick_shopper = {
        let place = place_id.clone();
        move |ev: leptos::ev::Event| {
            let v = event_target_value(&ev);
            change(MenuEdit::Shopper { place: place.clone(), who: (!v.is_empty()).then_some(v) });
        }
    };

    let rows = lines
        .into_iter()
        .map(|(product, amount, ticked)| {
            let place = place_id.clone();
            let id = product.id.clone();
            view! {
                <label class="tcw-buy" class:is-bought=ticked>
                    <input type="checkbox" prop:checked=ticked
                           on:change=move |ev| change(MenuEdit::Bought {
                               place: place.clone(),
                               product: id.clone(),
                               bought: event_target_checked(&ev),
                           }) />
                    <span class="tcw-buy-name">{product.name.clone()}</span>
                    <span class="tcw-buy-amount">{quantity(amount, product.unit)}</span>
                </label>
            }
        })
        .collect_view();

    // The expense: there, with what it came to, or a button to record it. Only a pointer is
    // kept, so an expense deleted since is "not recorded" again.
    let recorded = spending
        .as_deref()
        .and_then(|id| tour.spendings.iter().find(|s| s.id.as_str() == id));
    let unit = crate::ui::unit(&tour);
    let foot = match recorded {
        Some(s) => {
            let amount = money_in(tour.convert(s.amount, &s.currency), &unit);
            view! {
                <div class="tcw-errand-foot is-done">
                    "✓ " {(t().menu.recorded)(&s.description, &amount)}
                </div>
            }
            .into_any()
        }
        None => {
            let draft = {
                let payer = who.clone().map(PersonId::new);
                let mut d = match &payer {
                    Some(p) => SpendingDraft::paid_by(&tour, p),
                    None => SpendingDraft::new(&tour),
                };
                d.description = (t().menu.errand_description)(&place.name);
                d.category = t().menu.category.to_owned();
                d.errand = Some(place_id.clone());
                d
            };
            view! {
                <div class="tcw-errand-foot">
                    <span class="tcn-hint">{t().menu.not_recorded}</span>
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            class:tcn-btn-primary=all_bought
                            on:click=move |_| dialog.set(Some(Dialog::Spending(draft.clone())))>
                        {t().menu.record}
                    </button>
                </div>
            }
            .into_any()
        }
    };

    view! {
        <div class="tcn-card tcw-errand">
            <div class="tcw-errand-head">
                <b class="tcw-errand-place">{place.name.clone()}</b>
                <span class="tcw-errand-count">{(t().menu.bought_of)(bought, total)}</span>
                <label class="tcw-errand-who">
                    <span>{t().menu.shopper}</span>
                    <select class="tcn-input" on:change=pick_shopper>
                        <option value="" selected=chosen.is_empty()>{t().menu.nobody_yet}</option>
                        {options}
                    </select>
                </label>
            </div>
            <div class="tcw-buy-list">{rows}</div>
            {foot}
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

    /// Recorded from the market's shopping, the expense is the market's; deleted, the
    /// market is "not recorded" again because the pointer leads nowhere.
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
            errand: Some(MARKET.into()),
        };
        let after = crate::edit::put_spending(&t, &d);
        let menu = Menu::of(&after).expect("menu");
        assert_eq!(menu.errand(MARKET).and_then(|e| e.spending.clone()).as_deref(), Some("s1"));
        assert!(after.spendings.iter().any(|s| s.id.as_str() == "s1"));
        // The same expense edited later, from the Expenses tab, leaves the pointer alone.
        let mut plain = SpendingDraft::of(after.spendings.iter().find(|s| s.id.as_str() == "s1").unwrap());
        plain.amount = tc_core::Cents(5000);
        let edited = crate::edit::put_spending(&after, &plain);
        assert_eq!(Menu::of(&edited).and_then(|m| m.errand(MARKET).and_then(|e| e.spending.clone())).as_deref(), Some("s1"));
    }

    #[test]
    fn quantities_are_rounded_up_to_what_a_shop_sells() {
        assert_eq!(quantity(1234.0, Unit::Gram), format!("1{}3\u{a0}{}", crate::ui::decimal(), t().menu.kilograms));
        assert_eq!(quantity(2000.0, Unit::Millilitre), format!("2\u{a0}{}", t().menu.litres));
        assert_eq!(quantity(333.0, Unit::Gram), format!("340\u{a0}{}", t().menu.grams));
        assert_eq!(quantity(2.25, Unit::Piece), format!("3\u{a0}{}", t().menu.pieces));
    }
}
