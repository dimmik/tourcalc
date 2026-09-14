//! Phase-0 spike: the Balance tab of a real tour, on real data, in Leptos.
//!
//! The point is a number - how large a bundle a Rust web app of this shape actually is -
//! so this is deliberately not a hello-world: it parses the same 199 KB tour the C# app
//! serves, runs the same arithmetic, and renders the same two lists the app's Balance tab
//! shows. Whatever `serde_json` and `tc-core` cost, they are in the measurement.

use leptos::prelude::*;
use tc_core::{calculate, suggest_settlement, Options, PersonId, Tour};

/// Baked into the binary at compile time, so the spike needs no server to run.
const FIXTURE: &str = include_str!("../../../fixtures/zscph2y.tour.json");

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Balance,
    People,
}

#[component]
fn App() -> impl IntoView {
    // Parsed once. `tour` owns its data; everything below borrows from it.
    let tour = Tour::from_json(FIXTURE).expect("fixture parses");

    // A signal is Leptos's answer to "who may change this and who needs to know".
    // `set_tab` is Copy and cheap to hand to a closure - which is what lets the click
    // handler below capture it by `move` without any Rc<RefCell<..>> ceremony.
    let (tab, set_tab) = signal(Tab::Balance);

    let name_of = {
        let tour = tour.clone();
        move |id: &PersonId| {
            tour.person(id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "n/a".to_owned())
        }
    };

    let balances = calculate(&tour, Options::default());
    let transfers = suggest_settlement(&tour).expect("converges");

    let total: i64 = tour.spendings.iter().map(|s| s.amount.0).sum();

    let rows: Vec<_> = balances
        .per_person
        .iter()
        .map(|b| (name_of(&b.person), b.spent.0, b.received.0, b.debt().0))
        .collect();

    let payments: Vec<_> = transfers
        .iter()
        .map(|t| (name_of(&t.from), name_of(&t.to), t.amount.0))
        .collect();

    view! {
        <header class="top">
            <span class="brand">"🧭"</span>
            <span class="title">{tour.name.clone()}</span>
        </header>

        <div class="stats">
            <div><b>{tour.persons.len()}</b>" people"</div>
            <div><b>{tour.spendings.len()}</b>" expenses"</div>
            <div><b>{total}</b>" total"</div>
        </div>

        <nav class="tabs">
            <button
                class:active=move || tab.get() == Tab::Balance
                on:click=move |_| set_tab.set(Tab::Balance)
            >"Balance"</button>
            <button
                class:active=move || tab.get() == Tab::People
                on:click=move |_| set_tab.set(Tab::People)
            >"People"</button>
        </nav>

        <Show when=move || tab.get() == Tab::Balance>
            <ul class="list">
                {payments
                    .iter()
                    .map(|(from, to, amount)| {
                        view! {
                            <li>
                                <span>{from.clone()}" → "{to.clone()}</span>
                                <b>{*amount}</b>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </Show>

        <Show when=move || tab.get() == Tab::People>
            <ul class="list">
                {rows
                    .iter()
                    .map(|(name, spent, received, debt)| {
                        let owes = *debt > 0;
                        view! {
                            <li>
                                <span>{name.clone()}</span>
                                <small>{*spent}" spent · "{*received}" share"</small>
                                <b class:owes=owes>{*debt}</b>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </Show>
    }
}
