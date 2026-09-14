//! The same screen as `spike-leptos`, in Dioxus, so the two can be compared on identical
//! work: same fixture, same arithmetic, same two lists, same tab switch.

use dioxus::prelude::*;
use tc_core::{calculate, suggest_settlement, Options, PersonId, Tour};

const FIXTURE: &str = include_str!("../../../fixtures/zscph2y.tour.json");

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Balance,
    People,
}

#[component]
fn App() -> Element {
    let tour = use_hook(|| Tour::from_json(FIXTURE).expect("fixture parses"));
    let mut tab = use_signal(|| Tab::Balance);

    let name_of = |id: &PersonId| {
        tour.person(id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "n/a".to_owned())
    };

    let balances = calculate(&tour, Options::default());
    let transfers = suggest_settlement(&tour).expect("converges");
    let total: i64 = tour.spendings.iter().map(|s| s.amount.0).sum();

    rsx! {
        header { class: "top",
            span { class: "brand", "🧭" }
            span { class: "title", "{tour.name}" }
        }
        div { class: "stats",
            div { b { "{tour.persons.len()}" } " people" }
            div { b { "{tour.spendings.len()}" } " expenses" }
            div { b { "{total}" } " total" }
        }
        nav { class: "tabs",
            button {
                class: if tab() == Tab::Balance { "active" },
                onclick: move |_| tab.set(Tab::Balance),
                "Balance"
            }
            button {
                class: if tab() == Tab::People { "active" },
                onclick: move |_| tab.set(Tab::People),
                "People"
            }
        }
        if tab() == Tab::Balance {
            ul { class: "list",
                for t in transfers.iter() {
                    li {
                        span { "{name_of(&t.from)} → {name_of(&t.to)}" }
                        b { "{t.amount}" }
                    }
                }
            }
        } else {
            ul { class: "list",
                for b in balances.per_person.iter() {
                    li {
                        span { "{name_of(&b.person)}" }
                        small { "{b.spent} spent · {b.received} share" }
                        b { class: if b.debt().0 > 0 { "owes" }, "{b.debt()}" }
                    }
                }
            }
        }
    }
}
