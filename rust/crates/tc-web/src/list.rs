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

    spawn_local(async move {
        set_state.set(match api::tours().await {
            Ok(ts) => Load::Ready(ts),
            Err(e) => Load::Failed(e),
        });
    });

    view! {
        {move || match state.get() {
            Load::Loading => view! { <div class="tcn-loading">"Loading your tours…"</div> }.into_any(),
            Load::Failed(why) => view! {
                <div class="tcn-section">
                    <div class="tcn-errors">{why}</div>
                    <p class="tcn-hint">
                        "Opening a tour link signs you in - ask whoever shared the tour to send it again."
                    </p>
                </div>
            }.into_any(),
            Load::Ready(tours) if tours.is_empty() => view! {
                <div class="tcn-section">
                    <div class="tcn-allsettled">
                        <div class="tcn-allsettled-icon">"🧭"</div>
                        <div class="tcn-allsettled-title">"No tours yet"</div>
                        <div class="tcn-allsettled-sub">"Nothing is filed under this access code."</div>
                    </div>
                </div>
            }.into_any(),
            Load::Ready(tours) => view! {
                <div class="tcn-section">
                    <div class="tcn-section-title">
                        "Your tours " <span class="tcn-count">{tours.len()}</span>
                    </div>
                    <div class="tcn-tourgrid">
                        {tours.iter().map(|t| view! { <Row tour=t.clone() /> }).collect_view()}
                    </div>
                </div>
            }.into_any(),
        }}
    }
}

#[component]
fn Row(tour: Tour) -> impl IntoView {
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
        </div>
    }
}
