//! One tour: the header figures and the Balance tab.
//!
//! **The arithmetic happens here, in the browser.** The server sends the tour as stored -
//! spendings, people, weights - and `tc-core` works out who owes whom on this side. It is
//! the same `calculate` and the same `suggest_settlement` the server would run, compiled to
//! wasm instead of to a native binary.
//!
//! That is the whole argument for the rewrite in one file: the offline client has to do
//! this itself, and so the alternative is two implementations of the same money that must
//! agree forever.

use crate::api;
use crate::ui::{avatar_colour, initials, money, name_of};
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::{settlement_summary, split_family, suggest_settlement, Cents, PersonId, Tour};

/// A screen that is waiting, has something, or has failed.
///
/// Three states, so the view has to handle all three: `match` will not compile until it
/// does, and "forgot to draw the error case" is a whole genre of bug that cannot happen.
#[derive(Clone)]
pub enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[component]
pub fn TourPage(id: String) -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);

    // Fetching is asynchronous and the browser has one thread, so the future is spawned
    // rather than awaited: the view renders "loading" immediately and is told to update
    // when the answer arrives.
    {
        let id = id.clone();
        spawn_local(async move {
            set_state.set(match api::tour(&id).await {
                Ok(t) => Load::Ready(t),
                Err(e) => Load::Failed(e),
            });
        });
    }

    view! {
        {move || match state.get() {
            Load::Loading => view! { <div class="tcn-loading">"Loading the tour…"</div> }.into_any(),
            Load::Failed(why) => view! {
                <div class="tcn-section">
                    <div class="tcn-errors">{why}</div>
                    <a class="tcn-btn" href="/">"Go to my tours"</a>
                </div>
            }.into_any(),
            Load::Ready(tour) => view! { <TourView tour=tour /> }.into_any(),
        }}
    }
}

#[component]
fn TourView(tour: Tour) -> impl IntoView {
    // Everything below is computed once, here, from a borrowed tour. No clone of the tour
    // is made to calculate over it - which is the point `tc-core::calc` is written to make.
    let transfers = suggest_settlement(&tour).unwrap_or_default();

    let currency = tour.currency().name.clone();
    let multi = tour.currencies.len() > 1;
    let currency_label = if multi { currency } else { String::new() };

    let total_spent: Cents = tour
        .spendings
        .iter()
        // What the app counts as spending: a payback has no category and is not an expense.
        .filter(|s| !s.category.trim().is_empty() && s.kind.counts(false))
        .map(|s| tour.amount_in_current(s))
        .sum();

    let expenses = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .count();

    // Family transfers are shown apart from the rest, as in the app.
    let (family, between) = split_family(&transfers);

    let left_to_settle: Cents = between
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();

    let name_by = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };

    let people = tour.persons.len();
    let title = tour.name.clone();

    view! {
        <div class="tcn-hero">
            <div class="tcn-hero-top">
                <div class="tcn-hero-name">{title}</div>
            </div>
            <div class="tcn-hero-sub">
                <span>"from server"</span>
            </div>
            <div class="tcn-hero-metrics">
                <Metric label="Total spent" value=money(total_spent) unit=currency_label.clone() />
                <Metric label="People" value=people.to_string() unit=String::new() />
                <Metric label="Expenses" value=expenses.to_string() unit=String::new() />
                <Metric label="Left to settle" value=money(left_to_settle) unit=currency_label.clone() />
            </div>
        </div>

        <div class="tcn-section">
            {
                let count = between.len();
                let name_by = name_by.clone();
                let unit = currency_label.clone();
                if count == 0 {
                    view! {
                        <div class="tcn-allsettled">
                            <div class="tcn-allsettled-icon">"🎉"</div>
                            <div class="tcn-allsettled-title">"Everyone is settled up"</div>
                            <div class="tcn-allsettled-sub">
                                "No payments are left between the participants."
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="tcn-section-title">
                            "Who pays whom " <span class="tcn-count">{count}</span>
                        </div>
                        <div class="tcn-hint" style="margin: -4px 2px 10px 2px">
                            "Nothing here is paid yet — these are the payments that would square everyone up."
                        </div>
                        <div class="tcn-list">
                            {between
                                .iter()
                                .map(|t| {
                                    let from = name_by(&t.from);
                                    let to = name_by(&t.to);
                                    view! {
                                        <div class="tcn-settle">
                                            <div class="tcn-settle-flow">
                                                <Avatar name=from.clone() />
                                                <span class="tcn-settle-who">{from.clone()}</span>
                                                <span class="tcn-settle-arrow">"→"</span>
                                                <Avatar name=to.clone() />
                                                <span class="tcn-settle-who">{to.clone()}</span>
                                            </div>
                                            <div class="tcn-settle-amount">
                                                {money(t.amount)}
                                                {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit.clone()}</small> })}
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }.into_any()
                }
            }

            {
                let name_by = name_by.clone();
                let unit = currency_label.clone();
                (!family.is_empty()).then(|| view! {
                    <div class="tcn-section-title" style="margin-top:18px">
                        "Inside families " <span class="tcn-count">{family.len()}</span>
                    </div>
                    <div class="tcn-list">
                        {family
                            .iter()
                            .map(|t| {
                                let from = name_by(&t.from);
                                let to = name_by(&t.to);
                                view! {
                                    <div class="tcn-settle">
                                        <div class="tcn-settle-flow">
                                            <Avatar name=from.clone() />
                                            <span class="tcn-settle-who">{from.clone()}</span>
                                            <span class="tcn-settle-arrow">"→"</span>
                                            <Avatar name=to.clone() />
                                            <span class="tcn-settle-who">{to.clone()}</span>
                                        </div>
                                        <div class="tcn-settle-amount">
                                            {money(t.amount)}
                                            {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit.clone()}</small> })}
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                })
            }

            <div class="tcn-section-title" style="margin-top:18px">"Balances"</div>
            <div class="tcn-card" style="padding: 12px;">
                {
                    let name_by = name_by.clone();
                    let unit = currency_label.clone();
                    let rows = settlement_summary(&tour, &between);
                    // The bar is drawn to the largest balance, so the widths compare.
                    let scale = rows.iter().map(|(_, a)| a.abs().0).max().unwrap_or(1).max(1);
                    rows.into_iter()
                        .map(|(who_id, amount)| {
                            let who = name_by(&who_id);
                            let owes = amount.0 > 0;
                            let shown = amount.abs();
                            let width = (shown.0 as f64 / scale as f64 * 100.0).round();
                            let unit = unit.clone();
                            view! {
                                <div style="margin-bottom: 12px;">
                                    <div class="tcn-bal-row">
                                        <Avatar name=who.clone() />
                                        <span class="tcn-bal-name">{who.clone()}</span>
                                        <span class=if owes { "tcn-bal-amount tcn-neg" } else { "tcn-bal-amount tcn-pos" }>
                                            {if owes { "owes " } else { "gets " }}
                                            {money(shown)}
                                            {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                        </span>
                                    </div>
                                    <div class="tcn-balancebar">
                                        <div class="tcn-balancebar-neg">
                                            {(!owes).then(|| view! { <i style=format!("width:{width}%")></i> })}
                                        </div>
                                        <div class="tcn-balancebar-mid"></div>
                                        <div class="tcn-balancebar-pos">
                                            {owes.then(|| view! { <i style=format!("width:{width}%")></i> })}
                                        </div>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()
                }
            </div>
        </div>
    }
}

#[component]
fn Metric(label: &'static str, value: String, unit: String) -> impl IntoView {
    view! {
        <div class="tcn-metric">
            <div class="tcn-metric-label">{label}</div>
            <div class="tcn-metric-value">
                {value}
                {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
            </div>
        </div>
    }
}

#[component]
fn Avatar(name: String) -> impl IntoView {
    view! {
        <span class="tcn-avatar tcn-avatar-sm" style=format!("background:{}", avatar_colour(&name))>
            {initials(&name)}
        </span>
    }
}
