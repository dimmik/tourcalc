//! The Menu tab's catalogue: the dishes and what goes into them, what is bought every day,
//! and the products themselves.
//!
//! One thing is open for editing at a time, and which one lives on the page
//! (`MenuState::editing`): a save redraws the tab, and the editor must not close under the
//! hand that saved it by surprise - it closes because the save said so.
//!
//! Rows of ingredients are signals each, keyed: typing an amount changes that row's signal
//! and nothing else, so the input keeps its focus.

use crate::edit::MenuEdit;
use crate::i18n::t;
use crate::menu::{folded, found, meal_name, search_box, when_name, MenuState};
use crate::queue::Operation;
use leptos::prelude::*;
use tc_core::menu::{Dish, Eaters, Ingredient, Meal, Menu, Product, Unit, When};

#[component]
pub fn Catalogue(
    menu: Menu,
    state: MenuState,
    apply: Callback<Operation>,
    /// What a portion comes to in this tour, said under the dishes.
    note: String,
) -> impl IntoView {
    // What a search looks at: a dish's name, meals and what goes in; a product's name, unit,
    // place and who it is for; the daily list's products.
    let product_name = |id: &str| menu.product(id).map(|p| p.name.clone()).unwrap_or_default();
    let dish_hays: Vec<String> = menu
        .dishes
        .iter()
        .map(|d| {
            let meals = d.meals.iter().map(|m| meal_name(*m)).collect::<Vec<_>>().join(" ");
            let what = d.ingredients.iter().map(|i| product_name(&i.product)).collect::<Vec<_>>().join(" ");
            folded(&format!("{} {meals} {what}", d.name))
        })
        .collect();
    let product_hays: Vec<String> = menu
        .products
        .iter()
        .map(|p| {
            let place = menu.places.iter().find(|x| x.id == p.place).map(|x| x.name.clone()).unwrap_or_default();
            folded(&format!("{} {} {place} {} {}", p.name, unit_label(p), eaters_name(p.eaters), p.category.as_deref().unwrap_or("")))
        })
        .collect();
    let place_hays: Vec<String> = menu.places.iter().map(|p| folded(&p.name)).collect();
    let daily_hay = folded(&menu.daily.iter().map(|i| product_name(&i.product)).collect::<Vec<_>>().join(" "));

    let row_shown = move |hay: String| move || if found(&state.cat_find.get(), &hay) { "" } else { "none" };
    let dishes = menu
        .dishes
        .iter()
        .zip(dish_hays.clone())
        .map(|(dish, hay)| view! {
            <div style:display=row_shown(hay)>
                <DishRow menu=menu.clone() dish=dish.clone() state=state apply=apply />
            </div>
        })
        .collect_view();
    let products = menu
        .products
        .iter()
        .zip(product_hays.clone())
        .map(|(p, hay)| view! {
            <div style:display=row_shown(hay)>
                <ProductRow menu=menu.clone() product=p.clone() state=state apply=apply />
            </div>
        })
        .collect_view();

    // A section is open as the reader left it - or, while searching, when something in it
    // matches: folded or not, that is where the answer is.
    let searching = move || !state.cat_find.get().trim().is_empty();
    let dishes_shown = Memo::new(move |_| {
        if searching() { dish_hays.iter().any(|h| found(&state.cat_find.get(), h)) } else { state.dishes_open.get() }
    });
    let products_shown = Memo::new(move |_| {
        if searching() { product_hays.iter().any(|h| found(&state.cat_find.get(), h)) } else { state.products_open.get() }
    });
    let places_shown = {
        let hays = place_hays.clone();
        Memo::new(move |_| {
            if searching() { hays.iter().any(|h| found(&state.cat_find.get(), h)) } else { state.places_open.get() }
        })
    };
    let places = menu
        .places
        .iter()
        .zip(place_hays)
        .map(|(p, hay)| view! {
            <div style:display=row_shown(hay)>
                <PlaceRow menu=menu.clone() place=p.clone() state=state apply=apply />
            </div>
        })
        .collect_view();
    let place_count = menu.places.len();
    let menu_for_new_place = menu.clone();
    let daily_shown = Memo::new(move |_| {
        if searching() { found(&state.cat_find.get(), &daily_hay) } else { state.daily_open.get() }
    });
    let blank_product = Product {
        place: menu.places.first().map(|p| p.id.clone()).unwrap_or_default(),
        ..Product::default()
    };
    let menu_for_new_product = menu.clone();

    let dish_count = menu.dishes.len();
    // Each section empties on its own - "start the dishes over" is the usual wish, and it
    // must not take the products and the daily list along. Edits that exist already: the
    // dishes one by one, the daily list as nothing.
    let dish_ids: Vec<String> = menu.dishes.iter().map(|d| d.id.clone()).collect();
    let clear_dishes = move |_| {
        if crate::menu::ask(t().menu.clear_dishes_q) {
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::All(dish_ids.iter().cloned().map(MenuEdit::RemoveDish).collect())));
        }
    };
    let has_daily = !menu.daily.is_empty();
    let clear_daily = move |_| {
        if crate::menu::ask(t().menu.clear_daily_q) {
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::Daily(Vec::new())));
        }
    };
    let product_count = menu.products.len();
    // A section's heading folds it; "+ Dish" opens it, since the new one is drawn inside.
    let fold = |open: RwSignal<bool>, shown: Memo<bool>, title: &'static str, count: Option<usize>, per: Option<&'static str>| view! {
        <button type="button" class="tcw-cat-fold" aria-expanded=move || shown.get().to_string()
                on:click=move |_| open.update(|o| *o = !*o)>
            <span class="tcw-cat-caret">{move || if shown.get() { "▾" } else { "▸" }}</span>
            <h3>{title}</h3>
            {count.map(|n| view! { <span class="tcn-tab-badge">{n}</span> })}
            {per.map(|p| view! { <span class="tcw-cat-per">{p}</span> })}
        </button>
    };

    view! {
        {search_box(state.cat_find, t().menu.find_in_catalogue)}
        <section class="tcw-cat is-dishes">
            <div class="tcw-cat-head">
                {fold(state.dishes_open, dishes_shown, t().menu.dishes, Some(dish_count), Some(t().menu.per_portion_title))}
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| {
                            state.cat_find.set(String::new());
                            state.dishes_open.set(true);
                            state.editing.set(Some(String::new()));
                        }>
                    {t().menu.new_dish}
                </button>
            </div>
            // Hidden rather than left out: folding and unfolding redraws nothing.
            <div style:display=move || if dishes_shown.get() { "" } else { "none" }>
                <p class="tcw-food-note">{note}</p>
                <Show when=move || state.editing.get().as_deref() == Some("")>
                    <NewDishes state=state apply=apply />
                </Show>
                {dishes}
                {(dish_count > 0).then(|| view! {
                    <div class="tcw-cat-clear">
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger" on:click=clear_dishes.clone()>
                            {t().menu.clear_dishes}
                        </button>
                    </div>
                })}
            </div>
        </section>

        <section class="tcw-cat is-daily">
            <div class="tcw-cat-head">
                {fold(state.daily_open, daily_shown, t().menu.daily_title, None, Some(t().menu.per_portion_day_title))}
            </div>
            <Show when=move || daily_shown.get()>
                <p class="tcw-food-note">{t().menu.daily_note}</p>
                <DailyRow menu=menu.clone() state=state apply=apply />
                {has_daily.then(|| view! {
                    <div class="tcw-cat-clear">
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger" on:click=clear_daily>
                            {t().menu.clear_daily}
                        </button>
                    </div>
                })}
            </Show>
        </section>

        <section class="tcw-cat is-products">
            <div class="tcw-cat-head">
                {fold(state.products_open, products_shown, t().menu.products, Some(product_count), None)}
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| {
                            state.cat_find.set(String::new());
                            state.products_open.set(true);
                            state.editing.set(Some(NEW_PRODUCT.to_owned()));
                        }>
                    {t().menu.new_product}
                </button>
            </div>
            <div style:display=move || if products_shown.get() { "" } else { "none" }>
                <Show when=move || state.editing.get().as_deref() == Some(NEW_PRODUCT)>
                    <ProductEditor menu=menu_for_new_product.clone() product=blank_product.clone() state=state apply=apply />
                </Show>
                {products}
            </div>
        </section>

        <section class="tcw-cat is-places">
            <div class="tcw-cat-head">
                {fold(state.places_open, places_shown, t().menu.places_title, Some(place_count), None)}
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| {
                            state.cat_find.set(String::new());
                            state.places_open.set(true);
                            state.editing.set(Some(NEW_PLACE.to_owned()));
                        }>
                    {t().menu.add_place}
                </button>
            </div>
            <div style:display=move || if places_shown.get() { "" } else { "none" }>
                <Show when=move || state.editing.get().as_deref() == Some(NEW_PLACE)>
                    <PlaceEditor menu=menu_for_new_place.clone() place=tc_core::menu::Place::default() state=state apply=apply />
                </Show>
                {places}
            </div>
        </section>
    }
}

/// One row of the new-dishes list: a name and its meals.
#[derive(Clone, Copy)]
struct NewDish {
    key: u32,
    name: RwSignal<String>,
    meals: RwSignal<Vec<Meal>>,
}

impl NewDish {
    fn blank(next: StoredValue<u32>) -> NewDish {
        let key = next.get_value();
        next.set_value(key + 1);
        // Dinner, as a new dish always started: most of what a group cooks is dinner.
        NewDish { key, name: RwSignal::new(String::new()), meals: RwSignal::new(vec![Meal::Dinner]) }
    }
}

/// "+ Dish": dishes by the list, as currencies are added - a blank row at the end, a name
/// and its meals in each. Saved, they are in the catalogue with nothing in them yet, to be
/// filled one by one - or by several people at once. One dish alone opens its editor, since
/// that one is about to be filled.
#[component]
fn NewDishes(state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let next = StoredValue::new(0u32);
    let rows = RwSignal::new(vec![NewDish::blank(next)]);
    let error = RwSignal::new(None::<&'static str>);
    let spare = move || {
        if rows.with_untracked(|r| r.last().is_none_or(|l| !l.name.get_untracked().trim().is_empty())) {
            rows.update(|r| r.push(NewDish::blank(next)));
        }
    };
    let save = move |_| {
        let named: Vec<NewDish> = rows
            .get_untracked()
            .into_iter()
            .filter(|r| !r.name.get_untracked().trim().is_empty())
            .collect();
        if named.iter().any(|r| r.meals.get_untracked().is_empty()) {
            error.set(Some(t().menu.meal_needed));
            return;
        }
        let dishes: Vec<Dish> = named
            .iter()
            .map(|r| Dish {
                id: crate::edit::new_id(),
                name: r.name.get_untracked().trim().to_owned(),
                meals: r.meals.get_untracked(),
                meal: None,
                ingredients: Vec::new(),
            })
            .collect();
        state.editing.set(match dishes.as_slice() {
            [one] => Some(one.id.clone()),
            _ => None,
        });
        if !dishes.is_empty() {
            apply.run(Operation::Menu(MenuEdit::All(dishes.into_iter().map(MenuEdit::PutDish).collect())));
        }
    };
    view! {
        <div class="tcn-card tcw-cat-edit">
            <Show when=move || error.get().is_some()>
                <div class="tcn-errors">{move || error.get().unwrap_or_default()}</div>
            </Show>
            <For each=move || rows.get() key=|r| r.key let:row>
                <div class="tcw-newdish">
                    <input class="tcn-input tcw-cat-name" type="text" placeholder=t().menu.dish_name
                           prop:value=move || row.name.get()
                           on:input=move |ev| {
                               row.name.set(event_target_value(&ev));
                               spare();
                           } />
                    <span class="tcw-newdish-meals">
                        {Meal::ALL.into_iter().map(|meal| {
                            let on = move || row.meals.get().contains(&meal);
                            view! {
                                <button type="button" class=format!("tcw-meal tcw-meal-pick {}", meal_class(meal))
                                        class:is-on=on aria-pressed=move || on().to_string()
                                        on:click=move |_| row.meals.update(|m| {
                                            if m.contains(&meal) {
                                                m.retain(|x| *x != meal);
                                            } else {
                                                m.push(meal);
                                                m.sort_by_key(|x| Meal::ALL.iter().position(|y| y == x));
                                            }
                                        })>
                                    {meal_name(meal)}
                                </button>
                            }
                        }).collect_view()}
                    </span>
                    <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-x" title=t().menu.remove
                            style:visibility=move || if rows.with(|r| r.last().map(|l| l.key)) == Some(row.key) { "hidden" } else { "visible" }
                            on:click=move |_| {
                                rows.update(|r| r.retain(|x| x.key != row.key));
                                spare();
                            }>
                        "✕"
                    </button>
                </div>
            </For>
            <p class="tcw-food-note tcw-ingr-note">{t().menu.new_dishes_note}</p>
            <div class="tcw-cat-actions">
                <span class="tcw-cat-gap"></span>
                <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| state.editing.set(None)>
                    {t().dialogs.cancel}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" on:click=save>
                    {t().dialogs.save}
                </button>
            </div>
        </div>
    }
}

/// What `editing` says for a product being added - a dish being added is "".
const NEW_PRODUCT: &str = "+product";
/// ... and for a place being added; a place being edited is "@" and its id, since a place's
/// id could be a product's.
const NEW_PLACE: &str = "+place";
/// What `editing` says for the daily list.
const DAILY: &str = "+daily";
/// The unit select's value for "a unit of its own".
const OWN: &str = "+own";

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Gram => t().menu.grams,
        Unit::Millilitre => t().menu.millilitres,
        Unit::Piece => t().menu.pieces,
    }
}

/// What a product is counted in, as the lists say it: its own unit if it has one.
fn unit_label(p: &Product) -> String {
    match p.own_unit.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(own) => own.to_owned(),
        None => unit_name(p.unit).to_owned(),
    }
}

/// The class that colours a meal: the same three colours in the plan and here.
pub(crate) fn meal_class(meal: Meal) -> &'static str {
    match meal {
        Meal::Breakfast => "is-breakfast",
        Meal::Lunch => "is-lunch",
        Meal::Dinner => "is-dinner",
    }
}

fn eaters_name(eaters: Eaters) -> &'static str {
    match eaters {
        Eaters::Everyone => t().menu.eaters_everyone,
        Eaters::FullWeight => t().menu.eaters_full,
        Eaters::Others => t().menu.eaters_others,
    }
}

/// Where a product is used, by name: "Plov, Fish, Every day" - so that "cannot delete it"
/// says what to change first.
fn used_in(menu: &Menu, product: &str) -> String {
    let mut places: Vec<String> = menu
        .dishes
        .iter()
        .filter(|d| d.ingredients.iter().any(|i| i.product == product))
        .map(|d| d.name.clone())
        .collect();
    if menu.daily.iter().any(|i| i.product == product) {
        places.push(t().menu.daily_title.to_owned());
    }
    places.join(", ")
}

/// "rice 100 g · lamb 150 g" - what goes in, as the list shows it.
fn summary(menu: &Menu, items: &[Ingredient]) -> String {
    if items.is_empty() {
        return t().menu.no_ingredients.to_owned();
    }
    items
        .iter()
        .filter_map(|i| {
            let p = menu.product(&i.product)?;
            let when = match i.when {
                When::EveryDay => String::new(),
                w => format!(" {}", when_name(w)),
            };
            Some(format!("{} {}\u{a0}{}{when}", p.name.to_lowercase(), number(i.amount), unit_label(p)))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// An amount as typed and shown: no ".0", the reader's decimal mark.
fn number(x: f64) -> String {
    let text = if x.fract() == 0.0 { format!("{x:.0}") } else { format!("{x}") };
    text.replace('.', &crate::ui::decimal().to_string())
}

fn parse_number(text: &str) -> Option<f64> {
    text.trim().replace(',', ".").parse::<f64>().ok().filter(|x| x.is_finite() && *x > 0.0)
}

#[component]
fn DishRow(menu: Menu, dish: Dish, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let id = dish.id.clone();
    let open = {
        let id = id.clone();
        move || state.editing.get().as_deref() == Some(id.as_str())
    };
    let meals: Vec<Meal> = dish.meals.clone();
    let what = summary(&menu, &dish.ingredients);
    let editor_dish = dish.clone();
    let copy_name = (t().menu.copy_of)(&dish.name);
    view! {
        <Show when=open.clone()
              fallback={
                  let id = id.clone();
                  let name = dish.name.clone();
                  let meals = meals.clone();
                  let what = what.clone();
                  let copy_name = copy_name.clone();
                  move || {
                      let id = id.clone();
                      let from = id.clone();
                      let copy_name = copy_name.clone();
                      view! {
                          <div class="tcw-cat-row">
                              <div class="tcw-cat-main">
                                  <b>{name.clone()}</b>
                                  <span class="tcw-cat-meals">
                                      {meals.iter().map(|m| view! {
                                          <span class=format!("tcw-meal {}", meal_class(*m))>{meal_name(*m)}</span>
                                      }).collect_view()}
                                  </span>
                                  <small class="tcw-cat-what">{what.clone()}</small>
                              </div>
                              // A copy, opened at once: what is copied is usually to be changed.
                              <button type="button" class="tcn-btn tcn-btn-sm" title=t().menu.copy_dish_title
                                      on:click=move |_| {
                                          let id = crate::edit::new_id();
                                          state.editing.set(Some(id.clone()));
                                          apply.run(Operation::Menu(MenuEdit::CopyDish {
                                              from: from.clone(),
                                              id,
                                              name: copy_name.clone(),
                                          }));
                                      }>
                                  {t().menu.copy_dish}
                              </button>
                              <button type="button" class="tcn-btn tcn-btn-sm"
                                      on:click=move |_| state.editing.set(Some(id.clone()))>
                                  {t().menu.edit}
                              </button>
                          </div>
                      }
                  }
              }>
            <DishEditor menu=menu.clone() dish=editor_dish.clone() state=state apply=apply />
        </Show>
    }
}

/// One row of an ingredient list being edited.
#[derive(Clone, Copy)]
struct Row {
    key: u32,
    product: RwSignal<String>,
    amount: RwSignal<String>,
    /// On the daily list only.
    when: RwSignal<When>,
}

const WHENS: [When; 4] = [When::EveryDay, When::With(Meal::Breakfast), When::With(Meal::Lunch), When::With(Meal::Dinner)];

/// Rows for these ingredients, keyed from 0.
fn rows_of(items: &[Ingredient]) -> (RwSignal<Vec<Row>>, StoredValue<u32>) {
    let rows: Vec<Row> = items
        .iter()
        .enumerate()
        .map(|(i, item)| Row {
            key: i as u32,
            product: RwSignal::new(item.product.clone()),
            amount: RwSignal::new(number(item.amount)),
            when: RwSignal::new(item.when),
        })
        .collect();
    let next = StoredValue::new(rows.len() as u32);
    (RwSignal::new(rows), next)
}

/// The ingredients as the rows say them; rows without a product or a number are skipped.
fn ingredients_of(rows: &[Row]) -> Vec<Ingredient> {
    rows.iter()
        .filter_map(|r| {
            let product = r.product.get_untracked();
            let amount = parse_number(&r.amount.get_untracked())?;
            (!product.is_empty()).then_some(Ingredient { product, amount, when: r.when.get_untracked() })
        })
        .collect()
}

/// A product typed into an ingredient list rather than picked from the catalogue: its name,
/// its unit and how much of it. Always one blank one at the end, as with currencies - start
/// typing in it and the next appears.
#[derive(Clone, Copy)]
struct NewRow {
    key: u32,
    name: RwSignal<String>,
    amount: RwSignal<String>,
    unit: RwSignal<Unit>,
    /// A unit of its own, as with a product; `None` - one of the three.
    own: RwSignal<Option<String>>,
    /// A unit of its own being typed, as against picked from the list.
    typing: RwSignal<bool>,
    when: RwSignal<When>,
}

impl NewRow {
    fn blank(next: StoredValue<u32>) -> NewRow {
        let key = next.get_value();
        next.set_value(key + 1);
        NewRow {
            key,
            name: RwSignal::new(String::new()),
            amount: RwSignal::new(String::new()),
            unit: RwSignal::new(Unit::Gram),
            own: RwSignal::new(None),
            typing: RwSignal::new(false),
            when: RwSignal::new(When::EveryDay),
        }
    }

    fn is_blank(&self) -> bool {
        self.name.get_untracked().trim().is_empty() && self.amount.get_untracked().trim().is_empty()
    }

    fn typed(&self) -> Typed {
        Typed {
            name: self.name.get_untracked(),
            amount: self.amount.get_untracked(),
            unit: self.unit.get_untracked(),
            own: self.own.get_untracked(),
            when: self.when.get_untracked(),
        }
    }
}

/// The rows for new products: one blank.
fn new_rows(next: StoredValue<u32>) -> RwSignal<Vec<NewRow>> {
    RwSignal::new(vec![NewRow::blank(next)])
}

/// A blank row at the end, whatever was typed into the last one.
fn keep_spare(fresh: RwSignal<Vec<NewRow>>, next: StoredValue<u32>) {
    if fresh.with_untracked(|r| r.last().is_none_or(|last| !last.is_blank())) {
        fresh.update(|r| r.push(NewRow::blank(next)));
    }
}

/// A new-product row as typed.
struct Typed {
    name: String,
    amount: String,
    unit: Unit,
    own: Option<String>,
    when: When,
}

/// The products typed as new, and the ingredients they make.
///
/// A name the menu already has is that product - "rice" typed again is the rice, not a
/// second one - and so is a name typed twice. A new one is bought at [`place_for_new`], by
/// everybody, with no category: what Products is there to change.
fn new_products(menu: &Menu, typed: &[Typed]) -> Result<(Vec<Product>, Vec<Ingredient>), String> {
    let mut made: Vec<Product> = Vec::new();
    let mut items = Vec::new();
    for row in typed {
        let name = row.name.trim();
        if name.is_empty() && row.amount.trim().is_empty() {
            continue;
        }
        if name.is_empty() {
            return Err(t().menu.name_needed.into());
        }
        let Some(amount) = parse_number(&row.amount) else {
            return Err(t().menu.amount_needed.into());
        };
        let own = row.own.as_deref().map(str::trim);
        if own == Some("") {
            return Err(t().menu.unit_needed.into());
        }
        let key = name.to_lowercase();
        let known = menu
            .products
            .iter()
            .chain(made.iter())
            .find(|p| p.name.trim().to_lowercase() == key);
        // The one there is, in the unit it has: "rice, pcs, 2" against rice counted in grams
        // would be two grams of rice, so it is said rather than done.
        if let Some(p) = known {
            let theirs = p.own_unit.as_deref().map(str::trim).filter(|u| !u.is_empty());
            let same = match (own, theirs) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase(),
                (None, None) => p.unit == row.unit,
                _ => false,
            };
            if !same {
                return Err((t().menu.unit_differs)(p.name.trim(), &unit_label(p)));
            }
        }
        let product = match known.map(|p| p.id.clone()) {
            Some(id) => id,
            None => {
                let id = crate::edit::new_id();
                made.push(Product {
                    id: id.clone(),
                    name: name.to_owned(),
                    // A unit of its own counts like pieces - see `Product::own_unit`.
                    unit: if own.is_some() { Unit::Piece } else { row.unit },
                    place: crate::menu::place_for_new(menu),
                    eaters: Eaters::Everyone,
                    own_unit: own.map(str::to_owned),
                    category: None,
                });
                id
            }
        };
        items.push(Ingredient { product, amount, when: row.when });
    }
    Ok((made, items))
}

/// The edit, with the products it needs made first - one edit, so it is never half done.
fn with_products(made: Vec<Product>, edit: MenuEdit) -> MenuEdit {
    if made.is_empty() {
        edit
    } else {
        MenuEdit::All(made.into_iter().map(MenuEdit::PutProduct).chain([edit]).collect())
    }
}

/// A list of product-and-amount rows, with "+" and "✕".
#[component]
fn Ingredients(
    menu: Menu,
    rows: RwSignal<Vec<Row>>,
    next: StoredValue<u32>,
    fresh: RwSignal<Vec<NewRow>>,
    per_day: bool,
) -> impl IntoView {
    // The units of its own the tour's products already have, offered for a new one too.
    let mut known_units: Vec<String> = Vec::new();
    for p in &menu.products {
        if let Some(u) = p.own_unit.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            if !known_units.iter().any(|k| k == u) {
                known_units.push(u.to_owned());
            }
        }
    }
    let known_units = StoredValue::new(known_units);
    let place = crate::menu::place_for_new(&menu);
    let place_name = menu.places.iter().find(|p| p.id == place).map(|p| p.name.clone()).unwrap_or_default();
    let when_select = move |when: RwSignal<When>| {
        per_day.then(|| {
            let options = WHENS
                .into_iter()
                .map(|w| view! {
                    <option value=when_name(w) selected=when.get_untracked() == w>{when_name(w)}</option>
                })
                .collect_view();
            view! {
                " "
                <select class="tcw-ingr-when" title=t().menu.when_title
                        on:change=move |ev| {
                            let v = event_target_value(&ev);
                            if let Some(w) = WHENS.into_iter().find(|w| when_name(*w) == v) {
                                when.set(w);
                            }
                        }>
                    {options}
                </select>
            }
        })
    };
    let products: Vec<(String, String, String, Eaters)> = menu
        .products
        .iter()
        .map(|p| (p.id.clone(), p.name.clone(), unit_label(p), p.eaters))
        .collect();
    let products = StoredValue::new(products);
    let add = move |_| {
        let key = next.get_value();
        next.set_value(key + 1);
        rows.update(|r| {
            r.push(Row {
                key,
                product: RwSignal::new(String::new()),
                amount: RwSignal::new(String::new()),
                when: RwSignal::new(When::EveryDay),
            })
        });
    };
    view! {
        <div class="tcw-ingr">
            <For each=move || rows.get() key=|r| r.key let:row>
                {
                    let options = products.with_value(|ps| {
                        ps.iter()
                            .map(|(id, name, _, _)| {
                                let on = *id == row.product.get_untracked();
                                view! { <option value=id.clone() selected=on>{name.clone()}</option> }
                            })
                            .collect_view()
                    });
                    // "g per portion", "ml per adult": what the number means for this product.
                    let per = move || {
                        let chosen = row.product.get();
                        products.with_value(|ps| {
                            ps.iter().find(|(id, ..)| *id == chosen).map(|(_, _, unit, eaters)| {
                                let whom = match eaters {
                                    Eaters::Everyone => t().menu.per_portion,
                                    Eaters::FullWeight => t().menu.per_adult,
                                    Eaters::Others => t().menu.per_child,
                                };
                                format!("{unit} {whom}")
                            })
                        })
                        .unwrap_or_default()
                    };
                    view! {
                        <div class="tcw-ingr-row">
                            <select class="tcn-input"
                                    on:change=move |ev| row.product.set(event_target_value(&ev))>
                                <option value="" selected=row.product.get_untracked().is_empty()>
                                    {t().menu.pick}
                                </option>
                                {options}
                            </select>
                            <input class="tcn-input tcw-ingr-amount" type="text" inputmode="decimal"
                                   prop:value=move || row.amount.get()
                                   on:input=move |ev| row.amount.set(event_target_value(&ev)) />
                            <span class="tcw-ingr-per">
                                {per}
                                {per_day.then(|| {
                                    let options = WHENS
                                        .into_iter()
                                        .map(|w| view! {
                                            <option value=when_name(w) selected=row.when.get_untracked() == w>{when_name(w)}</option>
                                        })
                                        .collect_view();
                                    view! {
                                        " "
                                        <select class="tcw-ingr-when" title=t().menu.when_title
                                                on:change=move |ev| {
                                                    let v = event_target_value(&ev);
                                                    if let Some(w) = WHENS.into_iter().find(|w| when_name(*w) == v) {
                                                        row.when.set(w);
                                                    }
                                                }>
                                            {options}
                                        </select>
                                    }
                                })}
                            </span>
                            <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-x" title=t().menu.remove
                                    on:click=move |_| rows.update(|r| r.retain(|x| x.key != row.key))>
                                "✕"
                            </button>
                        </div>
                    }
                }
            </For>
            <For each=move || fresh.get() key=|r| r.key let:row>
                {
                    let units = [Unit::Gram, Unit::Millilitre, Unit::Piece]
                        .into_iter()
                        .map(|u| view! {
                            <option value=unit_name(u) selected=row.own.get_untracked().is_none() && row.unit.get_untracked() == u>
                                {unit_name(u)}
                            </option>
                        })
                        .collect_view();
                    let known = known_units.with_value(|ks| {
                        ks.iter()
                            .map(|u| view! { <option value=format!("{OWN}{u}")>{u.clone()}</option> })
                            .collect_view()
                    });
                    view! {
                        <div class="tcw-ingr-row tcw-ingr-new">
                            <input class="tcn-input tcw-ingr-name" type="text" placeholder=t().menu.typed_product
                                   prop:value=move || row.name.get()
                                   on:input=move |ev| {
                                       row.name.set(event_target_value(&ev));
                                       keep_spare(fresh, next);
                                   } />
                            <input class="tcn-input tcw-ingr-amount" type="text" inputmode="decimal"
                                   prop:value=move || row.amount.get()
                                   on:input=move |ev| {
                                       row.amount.set(event_target_value(&ev));
                                       keep_spare(fresh, next);
                                   } />
                            <span class="tcw-ingr-per">
                                <Show when=move || row.typing.get()
                                      fallback=move || view! {
                                          <select class="tcw-ingr-when" title=t().menu.unit
                                                  on:change=move |ev| {
                                                      let v = event_target_value(&ev);
                                                      if v == OWN {
                                                          row.own.set(Some(String::new()));
                                                          row.typing.set(true);
                                                      } else if let Some(k) = v.strip_prefix(OWN) {
                                                          row.own.set(Some(k.to_owned()));
                                                      } else if let Some(u) = [Unit::Gram, Unit::Millilitre, Unit::Piece].into_iter().find(|u| unit_name(*u) == v) {
                                                          row.unit.set(u);
                                                          row.own.set(None);
                                                      }
                                                  }>
                                              {units.clone()}
                                              {known.clone()}
                                              <option value=OWN>{t().menu.own_unit}</option>
                                          </select>
                                      }>
                                    <input class="tcw-ingr-when tcw-ingr-own" type="text" placeholder=t().menu.unit
                                           prop:value=move || row.own.get().unwrap_or_default()
                                           on:input=move |ev| row.own.set(Some(event_target_value(&ev))) />
                                </Show>
                                " " {t().menu.per_portion}
                                {when_select(row.when)}
                            </span>
                            <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-x" title=t().menu.remove
                                    style:visibility=move || if fresh.with(|r| r.last().map(|l| l.key)) == Some(row.key) { "hidden" } else { "visible" }
                                    on:click=move |_| {
                                        fresh.update(|r| r.retain(|x| x.key != row.key));
                                        keep_spare(fresh, next);
                                    }>
                                "✕"
                            </button>
                        </div>
                    }
                }
            </For>
            <Show when=move || { fresh.track(); fresh.with_untracked(|r| r.iter().any(|x| !x.name.get().trim().is_empty())) }>
                <p class="tcw-food-note tcw-ingr-note">{(t().menu.new_product_note)(&place_name)}</p>
            </Show>
            <button type="button" class="tcn-btn tcn-btn-sm tcw-ingr-add" on:click=add>
                {t().menu.add_ingredient}
            </button>
        </div>
    }
}

#[component]
fn DishEditor(menu: Menu, dish: Dish, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let is_new = dish.id.is_empty();
    let name = RwSignal::new(dish.name.clone());
    let meals = RwSignal::new(dish.meals.clone());
    let (rows, next) = rows_of(&dish.ingredients);
    let fresh = new_rows(next);
    let error = RwSignal::new(None::<String>);
    let sure = RwSignal::new(false);
    let id = dish.id.clone();
    let menu_for_save = StoredValue::new(menu.clone());

    let save = {
        let id = id.clone();
        move |_| {
            let name = name.get_untracked().trim().to_owned();
            if name.is_empty() {
                error.set(Some(t().menu.name_needed.into()));
                return;
            }
            let meals = meals.get_untracked();
            if meals.is_empty() {
                error.set(Some(t().menu.meal_needed.into()));
                return;
            }
            let typed: Vec<Typed> = fresh.get_untracked().iter().map(NewRow::typed).collect();
            let (made, more) = match menu_for_save.with_value(|m| new_products(m, &typed)) {
                Ok(x) => x,
                Err(why) => {
                    error.set(Some(why));
                    return;
                }
            };
            let mut ingredients = ingredients_of(&rows.get_untracked());
            ingredients.extend(more);
            let dish = Dish {
                id: if id.is_empty() { crate::edit::new_id() } else { id.clone() },
                name,
                meals,
                meal: None,
                ingredients,
            };
            state.editing.set(None);
            apply.run(Operation::Menu(with_products(made, MenuEdit::PutDish(dish))));
        }
    };
    let remove = {
        let id = id.clone();
        move |_| {
            if !sure.get_untracked() {
                sure.set(true);
                return;
            }
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::RemoveDish(id.clone())));
        }
    };

    // Chips that are pressed, as the payer and the categories are - no box inside. Pressed,
    // a chip is the meal's own badge, the one the dish shows in the list.
    let meal_boxes = Meal::ALL
        .into_iter()
        .map(|meal| {
            let on = move || meals.get().contains(&meal);
            view! {
                <button type="button" class=format!("tcw-meal tcw-meal-pick {}", meal_class(meal))
                        class:is-on=on
                        aria-pressed=move || on().to_string()
                        on:click=move |_| meals.update(|m| {
                            if m.contains(&meal) {
                                m.retain(|x| *x != meal);
                            } else {
                                m.push(meal);
                                m.sort_by_key(|x| Meal::ALL.iter().position(|y| y == x));
                            }
                        })>
                    {meal_name(meal)}
                </button>
            }
        })
        .collect_view();

    view! {
        <div class="tcn-card tcw-cat-edit">
            <Show when=move || error.get().is_some()>
                <div class="tcn-errors">{move || error.get().unwrap_or_default()}</div>
            </Show>
            <input class="tcn-input tcw-cat-name" type="text" placeholder=t().menu.dish_name
                   prop:value=move || name.get()
                   on:input=move |ev| name.set(event_target_value(&ev)) />
            <div class="tcw-cat-meals-edit">{meal_boxes}</div>
            <Ingredients menu=menu.clone() rows=rows next=next fresh=fresh per_day=false />
            <div class="tcw-cat-actions">
                {(!is_new).then(|| view! {
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger" on:click=remove.clone()>
                        {move || if sure.get() { t().menu.delete_sure } else { t().menu.delete }}
                    </button>
                })}
                <span class="tcw-cat-gap"></span>
                <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| state.editing.set(None)>
                    {t().dialogs.cancel}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" on:click=save>
                    {t().dialogs.save}
                </button>
            </div>
        </div>
    }
}

#[component]
fn DailyRow(menu: Menu, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let what = summary(&menu, &menu.daily);
    let items = menu.daily.clone();
    view! {
        <Show when=move || state.editing.get().as_deref() == Some(DAILY)
              fallback={
                  let what = what.clone();
                  move || view! {
                      <div class="tcw-cat-row">
                          <div class="tcw-cat-main"><small class="tcw-cat-what">{what.clone()}</small></div>
                          <button type="button" class="tcn-btn tcn-btn-sm"
                                  on:click=move |_| state.editing.set(Some(DAILY.to_owned()))>
                              {t().menu.edit}
                          </button>
                      </div>
                  }
              }>
            {
                let (rows, next) = rows_of(&items);
                let fresh = new_rows(next);
                let error = RwSignal::new(None::<String>);
                let menu_for_save = StoredValue::new(menu.clone());
                let save = move |_| {
                    let typed: Vec<Typed> = fresh.get_untracked().iter().map(NewRow::typed).collect();
                    let (made, more) = match menu_for_save.with_value(|m| new_products(m, &typed)) {
                        Ok(x) => x,
                        Err(why) => {
                            error.set(Some(why));
                            return;
                        }
                    };
                    let mut items = ingredients_of(&rows.get_untracked());
                    items.extend(more);
                    state.editing.set(None);
                    apply.run(Operation::Menu(with_products(made, MenuEdit::Daily(items))));
                };
                view! {
                    <div class="tcn-card tcw-cat-edit">
                        <Show when=move || error.get().is_some()>
                            <div class="tcn-errors">{move || error.get().unwrap_or_default()}</div>
                        </Show>
                        <Ingredients menu=menu.clone() rows=rows next=next fresh=fresh per_day=true />
                        <div class="tcw-cat-actions">
                            <span class="tcw-cat-gap"></span>
                            <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| state.editing.set(None)>
                                {t().dialogs.cancel}
                            </button>
                            <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" on:click=save>
                                {t().dialogs.save}
                            </button>
                        </div>
                    </div>
                }
            }
        </Show>
    }
}

#[component]
fn ProductRow(menu: Menu, product: Product, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let id = product.id.clone();
    let open = {
        let id = id.clone();
        move || state.editing.get().as_deref() == Some(id.as_str())
    };
    let place = menu
        .places
        .iter()
        .find(|p| p.id == product.place)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let mut facts = format!("{} · {} · {}", unit_label(&product), place, eaters_name(product.eaters));
    if let Some(c) = product.category.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        facts = format!("{c} · {facts}");
    }
    let editor_product = product.clone();
    view! {
        <Show when=open.clone()
              fallback={
                  let id = id.clone();
                  let name = product.name.clone();
                  let facts = facts.clone();
                  move || {
                      let id = id.clone();
                      view! {
                          <div class="tcw-cat-row">
                              <div class="tcw-cat-main">
                                  <b>{name.clone()}</b>
                                  <small class="tcw-cat-what">{facts.clone()}</small>
                              </div>
                              <button type="button" class="tcn-btn tcn-btn-sm"
                                      on:click=move |_| state.editing.set(Some(id.clone()))>
                                  {t().menu.edit}
                              </button>
                          </div>
                      }
                  }
              }>
            <ProductEditor menu=menu.clone() product=editor_product.clone() state=state apply=apply />
        </Show>
    }
}

#[component]
fn ProductEditor(menu: Menu, product: Product, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let is_new = product.id.is_empty();
    let used = !is_new && menu.uses(&product.id);
    let name = RwSignal::new(product.name.clone());
    let unit = RwSignal::new(product.unit);
    // Text when the product has a unit of its own; `None` - one of the three.
    let own = RwSignal::new(product.own_unit.clone().filter(|u| !u.trim().is_empty()));
    // A new unit being typed, as against one picked from the list - only then the text box.
    let typing = RwSignal::new(false);
    // The unit it had: changed, the amounts in dishes and on the daily list stay the numbers
    // they were - 500 ml of juice becomes 500 packs - and the editor says so.
    let was = (product.unit, product.own_unit.clone().filter(|u| !u.trim().is_empty()));
    let unit_changed = move || !is_new && (unit.get(), own.get().map(|u| u.trim().to_owned())) != was;
    let place = RwSignal::new(product.place.clone());
    // A new place being typed: its name. `None` - one of the tour's places.
    let new_place = RwSignal::new(None::<String>);
    let category = RwSignal::new(product.category.clone().map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()));
    // A new category being typed, as with units.
    let new_category = RwSignal::new(false);
    let eaters = RwSignal::new(product.eaters);
    let error = RwSignal::new(None::<&'static str>);
    let id = product.id.clone();

    let save = {
        let id = id.clone();
        move |_| {
            let name = name.get_untracked().trim().to_owned();
            if name.is_empty() {
                error.set(Some(t().menu.name_needed));
                return;
            }
            let own_unit = own.get_untracked().map(|u| u.trim().to_owned());
            if own_unit.as_deref() == Some("") {
                error.set(Some(t().menu.unit_needed));
                return;
            }
            let category = category.get_untracked().map(|c| c.trim().to_owned());
            if new_category.get_untracked() && category.as_deref().unwrap_or("").is_empty() {
                error.set(Some(t().menu.category_needed));
                return;
            }
            // A place typed in is made here, with the product, in one edit.
            let fresh = match new_place.get_untracked().map(|n| n.trim().to_owned()) {
                Some(n) if n.is_empty() => {
                    error.set(Some(t().menu.place_needed));
                    return;
                }
                Some(n) => Some(tc_core::menu::Place { id: crate::edit::new_id(), name: n }),
                None => None,
            };
            let product = Product {
                id: if id.is_empty() { crate::edit::new_id() } else { id.clone() },
                name,
                // A unit of its own counts like pieces - see `Product::own_unit`.
                unit: if own_unit.is_some() { Unit::Piece } else { unit.get_untracked() },
                place: fresh.as_ref().map(|p| p.id.clone()).unwrap_or_else(|| place.get_untracked()),
                eaters: eaters.get_untracked(),
                own_unit,
                category: category.filter(|c| !c.is_empty()),
            };
            state.editing.set(None);
            let edit = match fresh {
                Some(p) => MenuEdit::All(vec![MenuEdit::PutPlace(p), MenuEdit::PutProduct(product)]),
                None => MenuEdit::PutProduct(product),
            };
            apply.run(Operation::Menu(edit));
        }
    };
    let remove = {
        let id = id.clone();
        move |_| {
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::RemoveProduct(id.clone())));
        }
    };

    // The units of its own the tour's products already have - "btl", "5 l jug" - offered
    // like the three, the way an expense's categories offer the ones already used.
    let mut known: Vec<String> = Vec::new();
    for p in &menu.products {
        if let Some(u) = p.own_unit.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            if !known.iter().any(|k| k == u) {
                known.push(u.to_owned());
            }
        }
    }
    let known_options = known
        .into_iter()
        .map(|u| {
            let value = format!("{OWN}{u}");
            let is = u.clone();
            view! { <option value=value selected=move || !typing.get() && own.get().as_deref() == Some(is.as_str())>{u}</option> }
        })
        .collect_view();
    let units = [Unit::Gram, Unit::Millilitre, Unit::Piece]
        .into_iter()
        .map(|u| view! {
            <option value=unit_name(u) selected=move || own.get().is_none() && unit.get() == u>{unit_name(u)}</option>
        })
        .collect_view();
    let places = menu
        .places
        .iter()
        .map(|p| {
            let pid = p.id.clone();
            let on = pid == product.place;
            view! { <option value=pid selected=on>{p.name.clone()}</option> }
        })
        .collect_view();
    let categories = menu
        .categories()
        .into_iter()
        .map(|c| {
            let is = c.clone();
            let value = format!("{OWN}{c}");
            view! { <option value=value selected=move || !new_category.get() && category.get().as_deref() == Some(is.as_str())>{c}</option> }
        })
        .collect_view();
    let whom = [Eaters::Everyone, Eaters::FullWeight, Eaters::Others]
        .into_iter()
        .map(|e| view! { <option value=eaters_name(e) selected=move || eaters.get() == e>{eaters_name(e)}</option> })
        .collect_view();

    view! {
        <div class="tcn-card tcw-cat-edit">
            <Show when=move || error.get().is_some()>
                <div class="tcn-errors">{move || error.get().unwrap_or_default()}</div>
            </Show>
            <input class="tcn-input tcw-cat-name" type="text" placeholder=t().menu.product_name
                   prop:value=move || name.get()
                   on:input=move |ev| name.set(event_target_value(&ev)) />
            <div class="tcw-cat-fields">
                <label>
                    <span>{t().menu.unit}</span>
                    <select class="tcn-input" on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if v == OWN {
                            own.set(Some(String::new()));
                            typing.set(true);
                        } else if let Some(known) = v.strip_prefix(OWN) {
                            own.set(Some(known.to_owned()));
                            typing.set(false);
                        } else if let Some(u) = [Unit::Gram, Unit::Millilitre, Unit::Piece].into_iter().find(|u| unit_name(*u) == v) {
                            unit.set(u);
                            own.set(None);
                            typing.set(false);
                        }
                    }>
                        {units}
                        {known_options}
                        <option value=OWN selected=move || typing.get()>{t().menu.own_unit}</option>
                    </select>
                </label>
                <Show when=unit_changed.clone()>
                    <div class="tcn-hint tcw-cat-warn">{t().menu.unit_changed}</div>
                </Show>
                <Show when=move || typing.get()>
                    <label>
                        <span>{t().menu.own_unit_name}</span>
                        <input class="tcn-input" type="text" placeholder=t().menu.own_unit_placeholder
                               prop:value=move || own.get().unwrap_or_default()
                               on:input=move |ev| own.set(Some(event_target_value(&ev))) />
                    </label>
                </Show>
                <label>
                    <span>{t().menu.place}</span>
                    <select class="tcn-input" on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if v == OWN {
                            new_place.set(Some(String::new()));
                        } else {
                            new_place.set(None);
                            place.set(v);
                        }
                    }>
                        {places}
                        <option value=OWN>{t().menu.new_place}</option>
                    </select>
                </label>
                <Show when=move || new_place.get().is_some()>
                    <label>
                        <span>{t().menu.place_name}</span>
                        <input class="tcn-input" type="text" placeholder=t().menu.place_placeholder
                               prop:value=move || new_place.get().unwrap_or_default()
                               on:input=move |ev| new_place.set(Some(event_target_value(&ev))) />
                    </label>
                </Show>
                <label>
                    <span>{t().menu.product_category}</span>
                    <select class="tcn-input" on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if v == OWN {
                            category.set(Some(String::new()));
                            new_category.set(true);
                        } else if let Some(c) = v.strip_prefix(OWN) {
                            category.set(Some(c.to_owned()));
                            new_category.set(false);
                        } else {
                            category.set(None);
                            new_category.set(false);
                        }
                    }>
                        <option value="" selected=move || !new_category.get() && category.get().is_none()>{t().menu.no_category}</option>
                        {categories}
                        <option value=OWN selected=move || new_category.get()>{t().menu.new_category}</option>
                    </select>
                </label>
                <Show when=move || new_category.get()>
                    <label>
                        <span>{t().menu.category_name}</span>
                        <input class="tcn-input" type="text" placeholder=t().menu.category_placeholder
                               prop:value=move || category.get().unwrap_or_default()
                               on:input=move |ev| category.set(Some(event_target_value(&ev))) />
                    </label>
                </Show>
                <label>
                    <span>{t().menu.for_whom}</span>
                    <select class="tcn-input" on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if let Some(e) = [Eaters::Everyone, Eaters::FullWeight, Eaters::Others].into_iter().find(|e| eaters_name(*e) == v) {
                            eaters.set(e);
                        }
                    }>{whom}</select>
                </label>
            </div>
            <div class="tcw-cat-actions">
                {(!is_new).then(|| if used {
                    view! { <span class="tcn-hint">{(t().menu.in_use)(&used_in(&menu, &product.id))}</span> }.into_any()
                } else {
                    view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger" on:click=remove.clone()>
                            {t().menu.delete}
                        </button>
                    }.into_any()
                })}
                <span class="tcw-cat-gap"></span>
                <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| state.editing.set(None)>
                    {t().dialogs.cancel}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" on:click=save>
                    {t().dialogs.save}
                </button>
            </div>
        </div>
    }
}

/// What `editing` says for this place.
fn place_key(id: &str) -> String {
    format!("@{id}")
}

#[component]
fn PlaceRow(menu: Menu, place: tc_core::menu::Place, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let key = place_key(&place.id);
    let open = {
        let key = key.clone();
        move || state.editing.get().as_deref() == Some(key.as_str())
    };
    let count = menu.products.iter().filter(|p| p.place == place.id).count();
    let editor_place = place.clone();
    view! {
        <Show when=open.clone()
              fallback={
                  let key = key.clone();
                  let name = place.name.clone();
                  move || {
                      let key = key.clone();
                      view! {
                          <div class="tcw-cat-row">
                              <div class="tcw-cat-main">
                                  <b>{name.clone()}</b>
                                  <small class="tcw-cat-what">{(t().menu.products_here)(count)}</small>
                              </div>
                              <button type="button" class="tcn-btn tcn-btn-sm"
                                      on:click=move |_| state.editing.set(Some(key.clone()))>
                                  {t().menu.edit}
                              </button>
                          </div>
                      }
                  }
              }>
            <PlaceEditor menu=menu.clone() place=editor_place.clone() state=state apply=apply />
        </Show>
    }
}

#[component]
fn PlaceEditor(menu: Menu, place: tc_core::menu::Place, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let is_new = place.id.is_empty();
    let name = RwSignal::new(place.name.clone());
    let error = RwSignal::new(None::<&'static str>);
    // What is bought there, by name - why it cannot go.
    let here: Vec<String> = menu.products.iter().filter(|p| p.place == place.id).map(|p| p.name.clone()).collect();
    let id = place.id.clone();
    let save = {
        let id = id.clone();
        move |_| {
            let name = name.get_untracked().trim().to_owned();
            if name.is_empty() {
                error.set(Some(t().menu.name_needed));
                return;
            }
            let id = if id.is_empty() { crate::edit::new_id() } else { id.clone() };
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::PutPlace(tc_core::menu::Place { id, name })));
        }
    };
    let remove = {
        let id = id.clone();
        move |_| {
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::RemovePlace(id.clone())));
        }
    };
    view! {
        <div class="tcn-card tcw-cat-edit">
            <Show when=move || error.get().is_some()>
                <div class="tcn-errors">{move || error.get().unwrap_or_default()}</div>
            </Show>
            <input class="tcn-input tcw-cat-name" type="text" placeholder=t().menu.place_placeholder
                   prop:value=move || name.get()
                   on:input=move |ev| name.set(event_target_value(&ev)) />
            <div class="tcw-cat-actions">
                {(!is_new).then(|| if here.is_empty() {
                    view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger" on:click=remove.clone()>
                            {t().menu.delete}
                        </button>
                    }.into_any()
                } else {
                    view! { <span class="tcn-hint">{(t().menu.place_in_use)(&here.join(", "))}</span> }.into_any()
                })}
                <span class="tcw-cat-gap"></span>
                <button type="button" class="tcn-btn tcn-btn-sm" on:click=move |_| state.editing.set(None)>
                    {t().dialogs.cancel}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" on:click=save>
                    {t().dialogs.save}
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_amount_reads_with_a_comma_or_a_point_and_must_be_more_than_nothing() {
        assert_eq!(parse_number("0,25"), Some(0.25));
        assert_eq!(parse_number(" 120 "), Some(120.0));
        assert_eq!(parse_number("0"), None);
        assert_eq!(parse_number("много"), None);
    }

    fn typed(name: &str, amount: &str, unit: Unit, own: Option<&str>) -> Typed {
        Typed { name: name.into(), amount: amount.into(), unit, own: own.map(Into::into), when: When::EveryDay }
    }

    /// Pearl barley typed into the solyanka: a product at the supermarket, for everybody, and
    /// the dish's ingredient; rice typed again is the rice there is, and a name typed twice is
    /// one product.
    #[test]
    fn a_product_typed_into_a_dish_joins_the_catalogue_once() {
        let menu = crate::menu::starter(3);
        let rice = menu.products.iter().find(|p| p.id == "rice").expect("rice").clone();
        let (made, items) = new_products(
            &menu,
            &[
                typed("Перловка", "50", Unit::Gram, None),
                typed(&format!(" {} ", rice.name.to_uppercase()), "70", rice.unit, None),
                typed("перловка", "10", Unit::Gram, None),
                typed("Каперсы", "0,5", Unit::Gram, Some("банка")),
                typed("", "", Unit::Gram, None),
            ],
        )
        .expect("read");
        assert_eq!(made.len(), 2, "barley once, capers; no second rice");
        let barley = &made[0];
        assert_eq!((barley.name.as_str(), barley.unit, barley.eaters), ("Перловка", Unit::Gram, Eaters::Everyone));
        assert_eq!(barley.place, crate::menu::NEW_PRODUCT_PLACE);
        assert_eq!(barley.category, None);
        assert_eq!((made[1].unit, made[1].own_unit.as_deref()), (Unit::Piece, Some("банка")));
        let ids: Vec<&str> = items.iter().map(|i| i.product.as_str()).collect();
        assert_eq!(ids, vec![barley.id.as_str(), "rice", barley.id.as_str(), made[1].id.as_str()]);
        assert_eq!(items[3].amount, 0.5);

        assert!(new_products(&menu, &[typed("Перловка", "", Unit::Gram, None)]).is_err(), "no amount");
        // The rice there is, typed in pieces: said, not turned into two grams of rice.
        let why = new_products(&menu, &[typed(&rice.name, "2", Unit::Piece, None)]).expect_err("another unit");
        assert!(why.contains(rice.name.trim()), "{why}");
        // A new name twice in two units: the same.
        assert!(new_products(&menu, &[typed("Перловка", "50", Unit::Gram, None), typed("перловка", "1", Unit::Piece, None)]).is_err());
        assert!(new_products(&menu, &[typed("", "50", Unit::Gram, None)]).is_err(), "no name");
    }

    /// The supermarket where the menu has it - by id or by name - else the first place.
    #[test]
    fn a_new_product_is_bought_at_the_supermarket_or_the_first_place() {
        use tc_core::menu::Place;
        let mut menu = crate::menu::starter(1);
        assert_eq!(crate::menu::place_for_new(&menu), "supermarket");
        menu.places = vec![
            Place { id: "a".into(), name: "Рынок".into() },
            Place { id: "b".into(), name: " супермаркет ".into() },
        ];
        assert_eq!(crate::menu::place_for_new(&menu), "b");
        menu.places.truncate(1);
        assert_eq!(crate::menu::place_for_new(&menu), "a");
        menu.places.clear();
        assert_eq!(crate::menu::place_for_new(&menu), "");
    }
}
