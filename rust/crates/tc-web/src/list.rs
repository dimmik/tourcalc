//! Every tour this access code may see.
//!
//! The list endpoint sends the tours without their spendings - the screen shows neither -
//! so unlike the tour page there is nothing to calculate here: what each person spent has
//! already been worked out server-side and travels with the tour.

use crate::api;
use crate::tour::Load;
use crate::ui::money;
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::{Cents, Tour};

#[component]
pub fn TourListPage() -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);
    let search = RwSignal::new(String::new());
    let adding = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_code = RwSignal::new(String::new());
    let trouble = RwSignal::new(String::new());

    let load = Callback::new(move |_: ()| {
        spawn_local(async move {
            set_state.set(match api::tours().await {
                Ok(ts) => Load::Ready(ts),
                Err(e) => Load::Failed(e),
            });
        });
    });
    load.run(());

    let create = move |_| {
        let name = new_name.get().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let code = new_code.get().trim().to_owned();
        trouble.set(String::new());
        spawn_local(async move {
            match api::create_tour(&name, &code).await {
                Ok(_id) => {
                    adding.set(false);
                    new_name.set(String::new());
                    new_code.set(String::new());
                    load.run(());
                }
                Err(e) => trouble.set(e),
            }
        });
    };

    let remove = Callback::new(move |tour: Tour| {
        let question = format!("Delete '{}' and everything in it?", tour.name);
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message(&question).ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        trouble.set(String::new());
        spawn_local(async move {
            match api::delete_tour(tour.id.as_str()).await {
                Ok(()) => load.run(()),
                Err(e) => trouble.set(e),
            }
        });
    });

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <div class="tcn-search">
                    <span class="tcn-search-icon">"🔎"</span>
                    <input type="text" placeholder="Search"
                           prop:value=move || search.get()
                           on:input=move |ev| search.set(event_target_value(&ev)) />
                    <Show when=move || !search.get().is_empty()>
                        <button type="button" class="tcn-search-clear" title="Clear"
                                on:click=move |_| search.set(String::new())>"✕"</button>
                    </Show>
                </div>
                <button type="button" class="tcn-btn tcn-btn-primary"
                        on:click=move |_| adding.update(|a| *a = !*a)>
                    "+ New tour"
                </button>
            </div>

            <Show when=move || adding.get()>
                <div class="tcn-form">
                    <div class="tcn-field">
                        <div class="tcn-field-label">"Tour name"</div>
                        <input class="tcn-input" type="text" placeholder="Alps 2026"
                               prop:value=move || new_name.get()
                               on:input=move |ev| new_name.set(event_target_value(&ev)) />
                    </div>
                    <div class="tcn-field">
                        <div class="tcn-field-label">"Access code"</div>
                        <input class="tcn-input" type="text"
                               placeholder="only an administrator may choose one"
                               prop:value=move || new_code.get()
                               on:input=move |ev| new_code.set(event_target_value(&ev)) />
                        <div class="tcn-hint">
                            "Left empty - and for anybody but an administrator, always - the
                             tour joins the code you are signed in with."
                        </div>
                    </div>
                    <button type="button" class="tcn-btn tcn-btn-primary" on:click=create>
                        "Create"
                    </button>
                </div>
            </Show>

            <Show when=move || !trouble.get().is_empty()>
                <div class="tcn-errors">{move || trouble.get()}</div>
            </Show>
        </div>

        {move || match state.get() {
            Load::Loading => {
                view! { <div class="tcn-loading">"Loading your tours…"</div> }.into_any()
            }
            Load::Failed(why) => view! {
                <div class="tcn-section">
                    <div class="tcn-errors">{why}</div>
                    <p class="tcn-hint">
                        "Opening a tour link signs you in - ask whoever shared the tour to send it again."
                    </p>
                </div>
            }.into_any(),
            Load::Ready(tours) => {
                let needle = search.get().trim().to_lowercase();
                let shown: Vec<Tour> = tours
                    .into_iter()
                    .filter(|t| {
                        needle.is_empty()
                            || t.name.to_lowercase().contains(&needle)
                            // Searching by who is on it, which is how a trip gets found
                            // when its name is "поездка".
                            || t.persons.iter().any(|p| p.name.to_lowercase().contains(&needle))
                    })
                    .collect();

                if shown.is_empty() {
                    view! {
                        <div class="tcn-section">
                            <div class="tcn-empty">
                                <span class="tcn-empty-icon">"🧭"</span>
                                <div class="tcn-empty-title">
                                    {if needle.is_empty() { "No tours yet" } else { "Nothing matches" }}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="tcn-section">
                            <div class="tcn-section-title">
                                "Your tours " <span class="tcn-count">{shown.len()}</span>
                            </div>
                            <div class="tcn-tourgrid">
                                {shown
                                    .iter()
                                    .map(|t| view! { <Row tour=t.clone() remove=remove /> })
                                    .collect_view()}
                            </div>
                        </div>
                    }.into_any()
                }
            }
        }}
    }
}

#[component]
fn Row(tour: Tour, remove: Callback<Tour>) -> impl IntoView {
    let href = format!("/tour/{}", tour.id);
    let people = tour.persons.len();

    // The list carries what the server already worked out, in `SpentInCents` on each
    // person. It is not modelled by tc-core - the arithmetic recomputes it rather than
    // trusting it - so it is read out of the fields that came along for the ride.
    let spent: i64 = tour
        .persons
        .iter()
        .filter_map(|p| p.extras.0.get("SpentInCents").and_then(|v| v.as_i64()))
        .sum();

    let currency = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let for_delete = tour.clone();

    view! {
        <div class="tcn-tour">
            <a class="tcn-tour-name" href=href>{tour.name.clone()}</a>
            <div class="tcn-tour-meta">
                <span>{people} " people"</span>
                <span>"·"</span>
                <span title="Everything spent on this tour">
                    {money(Cents(spent))}
                    {(!currency.is_empty()).then(|| view! { "\u{a0}" {currency} })}
                </span>
            </div>
            <div class="tcn-tour-actions">
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                        on:click=move |_| remove.run(for_delete.clone())>
                    "Delete"
                </button>
            </div>
        </div>
    }
}
