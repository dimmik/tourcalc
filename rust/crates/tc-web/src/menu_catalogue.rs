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
use crate::menu::{meal_name, when_name, MenuState};
use crate::queue::Operation;
use leptos::prelude::*;
use tc_core::menu::{Dish, Eaters, Ingredient, Meal, Menu, Product, Unit, When};

#[component]
pub fn Catalogue(menu: Menu, state: MenuState, apply: Callback<Operation>) -> impl IntoView {
    let dishes = menu
        .dishes
        .iter()
        .map(|dish| view! { <DishRow menu=menu.clone() dish=dish.clone() state=state apply=apply /> })
        .collect_view();
    let products = menu
        .products
        .iter()
        .map(|p| view! { <ProductRow menu=menu.clone() product=p.clone() state=state apply=apply /> })
        .collect_view();
    let blank_dish = Dish { meals: vec![Meal::Dinner], ..Dish::default() };
    let blank_product = Product {
        place: menu.places.first().map(|p| p.id.clone()).unwrap_or_default(),
        ..Product::default()
    };
    let menu_for_new_dish = menu.clone();
    let menu_for_new_product = menu.clone();

    view! {
        <section class="tcw-cat">
            <div class="tcw-cat-head">
                <h3>{t().menu.dishes}</h3>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| state.editing.set(Some(String::new()))>
                    {t().menu.new_dish}
                </button>
            </div>
            <Show when=move || state.editing.get().as_deref() == Some("")>
                <DishEditor menu=menu_for_new_dish.clone() dish=blank_dish.clone() state=state apply=apply />
            </Show>
            {dishes}
        </section>

        <section class="tcw-cat">
            <div class="tcw-cat-head">
                <h3>{t().menu.daily_title}</h3>
            </div>
            <p class="tcw-food-note">{t().menu.daily_note}</p>
            <DailyRow menu=menu.clone() state=state apply=apply />
        </section>

        <section class="tcw-cat">
            <div class="tcw-cat-head">
                <h3>{t().menu.products}</h3>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click=move |_| state.editing.set(Some(NEW_PRODUCT.to_owned()))>
                    {t().menu.new_product}
                </button>
            </div>
            <Show when=move || state.editing.get().as_deref() == Some(NEW_PRODUCT)>
                <ProductEditor menu=menu_for_new_product.clone() product=blank_product.clone() state=state apply=apply />
            </Show>
            {products}
        </section>
    }
}

/// What `editing` says for a product being added - a dish being added is "".
const NEW_PRODUCT: &str = "+product";
/// What `editing` says for the daily list.
const DAILY: &str = "+daily";

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Gram => t().menu.grams,
        Unit::Millilitre => t().menu.millilitres,
        Unit::Piece => t().menu.pieces,
    }
}

fn eaters_name(eaters: Eaters) -> &'static str {
    match eaters {
        Eaters::Everyone => t().menu.eaters_everyone,
        Eaters::FullWeight => t().menu.eaters_full,
        Eaters::Others => t().menu.eaters_others,
    }
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
            Some(format!("{} {}\u{a0}{}{when}", p.name.to_lowercase(), number(i.amount), unit_name(p.unit)))
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
    let meals = dish.meals.iter().map(|m| meal_name(*m)).collect::<Vec<_>>().join(" · ");
    let what = summary(&menu, &dish.ingredients);
    let editor_dish = dish.clone();
    view! {
        <Show when=open.clone()
              fallback={
                  let id = id.clone();
                  let name = dish.name.clone();
                  let meals = meals.clone();
                  let what = what.clone();
                  move || {
                      let id = id.clone();
                      view! {
                          <div class="tcw-cat-row">
                              <div class="tcw-cat-main">
                                  <b>{name.clone()}</b>
                                  <span class="tcw-cat-meals">{meals.clone()}</span>
                                  <small class="tcw-cat-what">{what.clone()}</small>
                              </div>
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

/// A list of product-and-amount rows, with "+" and "✕".
#[component]
fn Ingredients(menu: Menu, rows: RwSignal<Vec<Row>>, next: StoredValue<u32>, per_day: bool) -> impl IntoView {
    let products: Vec<(String, String, Unit, Eaters)> = menu
        .products
        .iter()
        .map(|p| (p.id.clone(), p.name.clone(), p.unit, p.eaters))
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
                                format!("{} {whom}", unit_name(*unit))
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
    let error = RwSignal::new(None::<&'static str>);
    let sure = RwSignal::new(false);
    let id = dish.id.clone();

    let save = {
        let id = id.clone();
        move |_| {
            let name = name.get_untracked().trim().to_owned();
            if name.is_empty() {
                error.set(Some(t().menu.name_needed));
                return;
            }
            let meals = meals.get_untracked();
            if meals.is_empty() {
                error.set(Some(t().menu.meal_needed));
                return;
            }
            let dish = Dish {
                id: if id.is_empty() { crate::edit::new_id() } else { id.clone() },
                name,
                meals,
                meal: None,
                ingredients: ingredients_of(&rows.get_untracked()),
            };
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::PutDish(dish)));
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

    let meal_boxes = Meal::ALL
        .into_iter()
        .map(|meal| {
            view! {
                <label class="tcn-switchline tcw-cat-meal">
                    <input type="checkbox" prop:checked=move || meals.get().contains(&meal)
                           on:change=move |ev| {
                               let on = event_target_checked(&ev);
                               meals.update(|m| {
                                   m.retain(|x| *x != meal);
                                   if on {
                                       m.push(meal);
                                       m.sort_by_key(|x| Meal::ALL.iter().position(|y| y == x));
                                   }
                               });
                           } />
                    {meal_name(meal)}
                </label>
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
            <Ingredients menu=menu.clone() rows=rows next=next per_day=false />
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
                let save = move |_| {
                    state.editing.set(None);
                    apply.run(Operation::Menu(MenuEdit::Daily(ingredients_of(&rows.get_untracked()))));
                };
                view! {
                    <div class="tcn-card tcw-cat-edit">
                        <Ingredients menu=menu.clone() rows=rows next=next per_day=true />
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
    let facts = format!("{} · {} · {}", unit_name(product.unit), place, eaters_name(product.eaters));
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
    let place = RwSignal::new(product.place.clone());
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
            let product = Product {
                id: if id.is_empty() { crate::edit::new_id() } else { id.clone() },
                name,
                unit: unit.get_untracked(),
                place: place.get_untracked(),
                eaters: eaters.get_untracked(),
            };
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::PutProduct(product)));
        }
    };
    let remove = {
        let id = id.clone();
        move |_| {
            state.editing.set(None);
            apply.run(Operation::Menu(MenuEdit::RemoveProduct(id.clone())));
        }
    };

    let units = [Unit::Gram, Unit::Millilitre, Unit::Piece]
        .into_iter()
        .map(|u| view! { <option value=unit_name(u) selected=move || unit.get() == u>{unit_name(u)}</option> })
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
                        if let Some(u) = [Unit::Gram, Unit::Millilitre, Unit::Piece].into_iter().find(|u| unit_name(*u) == v) {
                            unit.set(u);
                        }
                    }>{units}</select>
                </label>
                <label>
                    <span>{t().menu.place}</span>
                    <select class="tcn-input" on:change=move |ev| place.set(event_target_value(&ev))>{places}</select>
                </label>
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
                    view! { <span class="tcn-hint">{t().menu.in_use}</span> }.into_any()
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
}
