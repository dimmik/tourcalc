//! One tour: the header figures, the settlement, and the two lists that can be edited.
//!
//! **The arithmetic happens here, in the browser.** The server sends the tour as stored -
//! spendings, people, weights - and `tc-core` works out who owes whom on this side. It is
//! the same `suggest_settlement` the server would run, compiled to wasm instead of to a
//! native binary.
//!
//! That is the whole argument for the rewrite in one file: the offline client has to do
//! this itself, and so the alternative is two implementations of the same money that must
//! agree forever.

use crate::dialogs::{PersonDialog, SpendingDialog};
use crate::edit::{PersonDraft, SpendingDraft};
use crate::queue::{self, Operation};
use crate::sync::{self, Status};
use crate::ui::{avatar_colour, initials, money, name_of};
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::{
    settlement_summary, split_family, suggest_settlement, Cents, Kind, Person, PersonId, Spending,
    Split, Tour, Transfer,
};

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

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Balance,
    People,
    Expenses,
}

/// Which dialog is open, if any.
#[derive(Clone)]
enum Dialog {
    Spending(SpendingDraft),
    Person(PersonDraft),
}

/// Says whether anything is still waiting to reach the server.
///
/// Quiet when there is nothing to say: an app that announces "saved" after every keystroke
/// teaches people to ignore it, and then it cannot tell them the one thing that matters.
#[component]
fn SyncLine(status: RwSignal<Status>, reload: Callback<()>, tour_id: String) -> impl IntoView {
    view! {
        {move || match status.get() {
            Status::Idle | Status::Synced => ().into_any(),
            Status::Waiting(n) => {
                // Naming the edits rather than counting them: "2 changes waiting" invites
                // the question this can answer directly.
                let what = queue::pending(&tour_id)
                    .iter()
                    .map(|op| op.describe())
                    .collect::<Vec<_>>()
                    .join(", ");
                view! {
                    <div class="tcn-section" style="padding-bottom:0">
                        <div class="tcn-chip tcn-chip-amber">
                            {if n == 0 {
                                "Offline — showing what this device had last".to_owned()
                            } else {
                                format!("Offline — saved here, waiting to be sent: {what}")
                            }}
                        </div>
                    </div>
                }.into_any()
            }
            Status::Failed(why) => view! {
                <div class="tcn-section" style="padding-bottom:0">
                    <div class="tcn-errors">
                        {why}
                        <button type="button" class="tcn-btn tcn-btn-sm" style="margin-left:10px"
                                on:click=move |_| reload.run(())>
                            "Try again"
                        </button>
                    </div>
                </div>
            }.into_any(),
        }}
    }
}

/// What a delete button asks for.
#[derive(Clone)]
enum Removal {
    Spending(Spending),
    Person(Person),
}

#[component]
pub fn TourPage(id: String) -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);
    let status = RwSignal::new(Status::Idle);

    // Arriving is the same operation as saving: drain whatever is queued, then show what
    // the server has with anything still waiting applied on top. So a reload after an
    // offline edit finishes the job by itself.
    let load = Callback::new({
        let id = id.clone();
        move |_: ()| {
            let id = id.clone();
            spawn_local(async move {
                let (tour, st) = sync::push(&id).await;
                status.set(st);
                set_state.set(match tour {
                    Some(t) => Load::Ready(queue::with_pending(&t)),
                    None => Load::Failed(
                        "This tour has not been opened on this device before, and there is \
                         no connection to fetch it."
                            .into(),
                    ),
                });
            });
        }
    });
    load.run(());

    view! {
        {move || match state.get() {
            Load::Loading => view! { <div class="tcn-loading">"Loading the tour…"</div> }.into_any(),
            Load::Failed(why) => view! {
                <div class="tcn-section">
                    <div class="tcn-errors">{why}</div>
                    <a class="tcn-btn" href="/">"Go to my tours"</a>
                </div>
            }.into_any(),
            Load::Ready(tour) => view! {
                <TourView tour=tour reload=load status=status />
            }.into_any(),
        }}
    }
}

#[component]
fn TourView(tour: Tour, reload: Callback<()>, status: RwSignal<Status>) -> impl IntoView {
    let tab = RwSignal::new(Tab::Balance);
    let dialog: RwSignal<Option<Dialog>> = RwSignal::new(None);
    let tour_id = tour.id.as_str().to_owned();

    // Every edit takes the same road: write it down, try to send it, redraw from whatever
    // came back. Nothing here waits for the network to decide what to show.
    let apply = {
        let tour_id = tour_id.clone();
        Callback::new(move |op: Operation| {
            let tour_id = tour_id.clone();
            dialog.set(None);
            spawn_local(async move {
                let (_, st) = sync::record(&tour_id, op).await;
                status.set(st);
                reload.run(());
            });
        })
    };

    // Computed from a borrowed tour. No copy of it is made to calculate over - which is the
    // point `tc-core::calc` is written to make.
    let transfers = suggest_settlement(&tour).unwrap_or_default();
    let (family, between): (Vec<Transfer>, Vec<Transfer>) = {
        let (f, b) = split_family(&transfers);
        (
            f.into_iter().cloned().collect(),
            b.into_iter().cloned().collect(),
        )
    };

    let unit = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let total_spent: Cents = tour
        .spendings
        .iter()
        // What the app counts as spending: a payback has no category and is not an expense.
        .filter(|s| !s.category.trim().is_empty() && s.kind.counts(false))
        .map(|s| tour.amount_in_current(s))
        .sum();

    let real: Vec<Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .cloned()
        .collect();

    let left_to_settle: Cents = between
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();

    let people = tour.persons.len();
    let expenses = real.len();
    let title = tour.name.clone();

    // Deleting is the one edit with no dialog, so it does its own asking.
    let delete = Callback::new(move |what: Removal| {
        let question = match &what {
            Removal::Spending(s) => format!("Delete '{}'?", s.description),
            Removal::Person(p) => format!("Delete '{}'?", p.name),
        };
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message(&question).ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        apply.run(match &what {
            Removal::Spending(s) => Operation::RemoveSpending(s.id.clone()),
            Removal::Person(p) => Operation::RemovePerson(p.id.clone()),
        });
    });

    let close = Callback::new(move |_: ()| dialog.set(None));

    // Each `Show` body is its own closure, and a `String` is not `Copy`: one clone per
    // place that needs it, made here where it is obvious, rather than a borrow that would
    // have to outlive them all. This is the tax the compiler charges for knowing that no
    // two of these can be reading a value somebody else is changing.
    let unit_metrics = unit.clone();
    let unit_balance = unit.clone();
    let unit_expenses = unit.clone();
    let tour_for_balance = tour.clone();
    let tour_for_people = tour.clone();
    let tour_for_expenses = tour.clone();
    let tour_for_fab = tour.clone();
    let tour_for_dialog = tour.clone();

    view! {
        <div class="tcn-hero">
            <div class="tcn-hero-top">
                <div class="tcn-hero-name">{title}</div>
            </div>
            <div class="tcn-hero-sub"><span>"from server"</span></div>
            <div class="tcn-hero-metrics">
                <Metric label="Total spent" value=money(total_spent) unit=unit_metrics.clone() />
                <Metric label="People" value=people.to_string() unit=String::new() />
                <Metric label="Expenses" value=expenses.to_string() unit=String::new() />
                <Metric label="Left to settle" value=money(left_to_settle) unit=unit_metrics.clone() />
            </div>
        </div>

        <SyncLine status=status reload=reload tour_id=tour_id.clone() />

        <div class="tcn-section" style="padding-bottom:0">
            <TabButton tab=tab mine=Tab::Balance label="Balance" count=between.len() />
            <TabButton tab=tab mine=Tab::People label="People" count=people />
            <TabButton tab=tab mine=Tab::Expenses label="Expenses" count=expenses />
        </div>

        <Show when=move || tab.get() == Tab::Balance>
            <BalanceTab tour=tour_for_balance.clone() between=between.clone()
                        family=family.clone() unit=unit_balance.clone() />
        </Show>

        <Show when=move || tab.get() == Tab::People>
            <PeopleTab tour=tour_for_people.clone() dialog=dialog delete=delete />
        </Show>

        <Show when=move || tab.get() == Tab::Expenses>
            <ExpensesTab tour=tour_for_expenses.clone() spendings=real.clone()
                         unit=unit_expenses.clone() dialog=dialog delete=delete />
        </Show>

        <button type="button" class="tcn-btn tcn-btn-primary tcn-fab"
                on:click={
                    let tour = tour_for_fab.clone();
                    move |_| dialog.set(Some(Dialog::Spending(SpendingDraft::new(&tour))))
                }>
            "+ Spend"
        </button>

        {move || {
            let tour = tour_for_dialog.clone();
            dialog.get().map(|d| match d {
                Dialog::Spending(draft) => view! {
                    <SpendingDialog tour=tour draft=draft on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Person(draft) => view! {
                    <PersonDialog tour=tour draft=draft on_close=close on_apply=apply />
                }.into_any(),
            })
        }}
    }
}

#[component]
fn PeopleTab(
    tour: Tour,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let rows = tour.persons.clone();
    let by_id = tour.clone();

    view! {
        <div class="tcn-section">
            <div class="tcn-section-title">
                "People " <span class="tcn-count">{rows.len()}</span>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary" style="margin-left:10px"
                        on:click=move |_| dialog.set(Some(Dialog::Person(PersonDraft::new())))>
                    "Add person"
                </button>
            </div>
            <div class="tcn-card" style="padding: 12px;">
                {rows
                    .iter()
                    .map(|p| {
                        let paid_by = p
                            .parent
                            .as_ref()
                            .and_then(|id| by_id.person(id))
                            .map(|pp| pp.name.clone());
                        let for_edit = p.clone();
                        let for_delete = p.clone();
                        view! {
                            <div class="tcn-bal-row" style="margin-bottom:10px">
                                <Avatar name=p.name.clone() />
                                <span class="tcn-bal-name">
                                    {p.name.clone()}
                                    {paid_by.map(|n| view! {
                                        <small class="tcn-hint">" · paid for by " {n}</small>
                                    })}
                                </span>
                                <span class="tcn-hint">"weight " {p.weight}</span>
                                <button type="button" class="tcn-btn tcn-btn-sm"
                                        on:click=move |_| dialog.set(Some(
                                            Dialog::Person(PersonDraft::of(&for_edit))))>
                                    "Edit"
                                </button>
                                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                                        on:click={
                                            let p = for_delete.clone();
                                            move |_| delete.run(Removal::Person(p.clone()))
                                        }>
                                    "✕"
                                </button>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
fn ExpensesTab(
    tour: Tour,
    spendings: Vec<Spending>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let name_of_id = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };

    view! {
        <div class="tcn-section">
            <div class="tcn-section-title">
                "Expenses " <span class="tcn-count">{spendings.len()}</span>
            </div>
            <div class="tcn-list">
                {spendings
                    .iter()
                    .rev()
                    .map(|s| {
                        let who = name_of_id(&s.from);
                        let shown = tour.amount_in_current(s);
                        let unit = unit.clone();
                        let for_edit = s.clone();
                        let for_delete = s.clone();
                        let description = if s.description.trim().is_empty() {
                            "(no description)".to_owned()
                        } else {
                            s.description.clone()
                        };
                        let category = s.category.trim().to_owned();
                        let everyone = matches!(s.split, Split::Everyone);
                        view! {
                            <div class="tcn-settle">
                                <div class="tcn-settle-flow">
                                    <Avatar name=who.clone() />
                                    <span class="tcn-settle-who">
                                        {description}
                                        <small class="tcn-hint">
                                            " · " {who.clone()}
                                            {(!category.is_empty()).then(|| format!(" · {category}"))}
                                            {everyone.then(|| " · everyone".to_owned())}
                                        </small>
                                    </span>
                                </div>
                                <div class="tcn-settle-amount">
                                    {money(shown)}
                                    {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                    <button type="button" class="tcn-btn tcn-btn-sm" style="margin-left:10px"
                                            on:click=move |_| dialog.set(Some(
                                                Dialog::Spending(SpendingDraft::of(&for_edit))))>
                                        "Edit"
                                    </button>
                                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                                            on:click={
                                                let s = for_delete.clone();
                                                move |_| delete.run(Removal::Spending(s.clone()))
                                            }>
                                        "✕"
                                    </button>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[component]
fn BalanceTab(
    tour: Tour,
    between: Vec<Transfer>,
    family: Vec<Transfer>,
    unit: String,
) -> impl IntoView {
    let name_by = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };
    let rows = settlement_summary(&tour, &between.iter().collect::<Vec<_>>());
    let has_real = tour.spendings.iter().any(|s| s.kind == Kind::Real);
    let names_for_rows = name_by.clone();

    view! {
        <div class="tcn-section">
            {
                let unit = unit.clone();
                let name_by = name_by.clone();
                if between.is_empty() {
                    view! {
                        <div class="tcn-allsettled">
                            <div class="tcn-allsettled-icon">"🎉"</div>
                            <div class="tcn-allsettled-title">"Everyone is settled up"</div>
                            <div class="tcn-allsettled-sub">
                                {if has_real {
                                    "No payments are left between the participants."
                                } else {
                                    "Add the first expense and the split will show up here."
                                }}
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="tcn-section-title">
                            "Who pays whom " <span class="tcn-count">{between.len()}</span>
                        </div>
                        <div class="tcn-hint" style="margin: -4px 2px 10px 2px">
                            "Nothing here is paid yet — these are the payments that would square everyone up."
                        </div>
                        <div class="tcn-list">
                            {transfer_rows(&between, &name_by, &unit)}
                        </div>
                    }.into_any()
                }
            }

            {
                let unit = unit.clone();
                let name_by = name_by.clone();
                (!family.is_empty()).then(|| view! {
                    <div class="tcn-section-title" style="margin-top:18px">
                        "Inside families " <span class="tcn-count">{family.len()}</span>
                    </div>
                    <div class="tcn-list">{transfer_rows(&family, &name_by, &unit)}</div>
                })
            }

            {(!rows.is_empty()).then(|| {
                // The bar is drawn to the largest balance, so the widths compare.
                let scale = rows.iter().map(|(_, a)| a.abs().0).max().unwrap_or(1).max(1);
                view! {
                    <div class="tcn-section-title" style="margin-top:18px">"Balances"</div>
                    <div class="tcn-card" style="padding: 12px;">
                        {rows
                            .iter()
                            .map(|(who_id, amount)| {
                                let who = names_for_rows(who_id);
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
                            .collect_view()}
                    </div>
                }
            })}
        </div>
    }
}

fn transfer_rows(
    list: &[Transfer],
    name_by: &impl Fn(&PersonId) -> String,
    unit: &str,
) -> impl IntoView {
    list.iter()
        .map(|t| {
            let from = name_by(&t.from);
            let to = name_by(&t.to);
            let unit = unit.to_owned();
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
                        {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                    </div>
                </div>
            }
        })
        .collect_view()
}

#[component]
fn TabButton(tab: RwSignal<Tab>, mine: Tab, label: &'static str, count: usize) -> impl IntoView {
    view! {
        <button type="button"
                class="tcn-btn"
                class:tcn-btn-primary=move || tab.get() == mine
                style="margin-right:6px"
                on:click=move |_| tab.set(mine)>
            {label} " " <span class="tcn-count">{count}</span>
        </button>
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
