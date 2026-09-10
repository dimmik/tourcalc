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

use crate::dialogs::{CurrenciesDialog, PersonDialog, SpendingDialog, TourDialog, VersionsDialog};
use crate::edit::{PersonDraft, SpendingDraft};
use crate::people::PeopleTab;
use crate::push::PushBell;
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
pub enum Tab {
    Balance,
    People,
    Expenses,
    Stats,
}

/// Which dialog is open, if any.
#[derive(Clone)]
pub enum Dialog {
    Spending(SpendingDraft),
    Person(PersonDraft),
    /// The tour's own properties: name, length, archived, settling up.
    Tour(crate::edit::TourDraft),
    /// The currencies and their rates.
    Currencies,
    /// What this tour used to be, and putting one of those back.
    Versions,
}

/// How old what is on screen is, and a way to ask for newer.
///
/// The app answers the question a reader has when the numbers look wrong - "am I looking at
/// something stale?" - and it is worth answering out loud, because this client will happily
/// show a tour it stored days ago when there is no network.
#[component]
fn Freshness(reload: Callback<()>, status: RwSignal<Status>) -> impl IntoView {
    view! {
        <span class="tcn-refresh-note">
            {move || match status.get() {
                Status::Synced => "✓ from the server".to_owned(),
                Status::Idle => "from the server".to_owned(),
                Status::Checking => "local copy — asking the server…".to_owned(),
                Status::Waiting(0) => "⚠ local copy — the server did not answer".to_owned(),
                Status::Waiting(n) => format!("⚠ local copy — {n} waiting to be sent"),
                Status::Failed(_) => "✕ the server did not answer".to_owned(),
            }}
        </span>
        <button type="button" class="tcn-hero-link" title="Ask the server for the latest"
                on:click=move |_| reload.run(())>
            "↻ refresh"
        </button>
    }
}

/// The link that signs somebody in and opens this tour.
///
/// It carries the hashed access code, which is what makes it work at all: whoever opens it
/// gets a token for that code and sees the tour without being told anything else. That also
/// means it is the whole of the security here - a link is an invitation.
#[component]
fn ShareLink(tour: Tour) -> impl IntoView {
    let code = tour
        .extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("AccessCodeMD5"))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
        .to_owned();
    let said = RwSignal::new(false);

    let href = format!("/goto/{}/{}", code, tour.id);

    view! {
        <button type="button" class="tcn-hero-link"
                title="Copy a link that opens this tour"
                on:click=move |_| {
                    let full = web_sys::window()
                        .and_then(|w| w.location().origin().ok())
                        .map(|o| format!("{o}{href}"))
                        .unwrap_or_else(|| href.clone());
                    copy_to_clipboard(&full);
                    said.set(true);
                }>
            {move || if said.get() { "link copied" } else { "share link" }}
        </button>
    }
}

/// Puts text on the clipboard.
///
/// Reached through `js_sys` rather than `web_sys`: the clipboard sits on `Navigator`, which
/// would mean turning on another web-sys feature for one call, and this crate is measured
/// by what it weighs.
fn copy_to_clipboard(text: &str) {
    use wasm_bindgen::JsCast as _;
    let Some(window) = web_sys::window() else {
        return;
    };
    let navigator = js_sys::Reflect::get(&window, &"navigator".into()).ok();
    let clipboard = navigator
        .and_then(|n| js_sys::Reflect::get(&n, &"clipboard".into()).ok())
        .filter(|c| !c.is_undefined());
    let Some(clipboard) = clipboard else { return };
    let write = js_sys::Reflect::get(&clipboard, &"writeText".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    if let Some(write) = write {
        let _ = write.call1(&clipboard, &text.into());
    }
}

/// Which currency the amounts are shown in.
///
/// Only the tour's own list, and only when there is more than one: changing it is a change
/// to the tour, so it goes through the queue like any other edit and everyone sees it.
#[component]
fn CurrencyPicker(tour: Tour, apply: Callback<Operation>) -> impl IntoView {
    let current = tour.current_currency.as_str().to_owned();
    view! {
        <span>"·"</span>
        <span>"show in "</span>
        <select class="tcn-input" style="width:auto;padding:2px 6px"
                on:change=move |ev| apply.run(Operation::SetCurrency(event_target_value(&ev)))>
            {tour
                .currencies
                .iter()
                .map(|c| {
                    let id = c.id.as_str().to_owned();
                    let selected = id == current;
                    view! { <option value=id selected=selected>{c.name.clone()}</option> }
                })
                .collect_view()}
        </select>
    }
}

/// Says whether anything is still waiting to reach the server.
///
/// Quiet when there is nothing to say: an app that announces "saved" after every keystroke
/// teaches people to ignore it, and then it cannot tell them the one thing that matters.
#[component]
fn SyncLine(status: RwSignal<Status>, reload: Callback<()>, tour_id: String) -> impl IntoView {
    view! {
        {move || match status.get() {
            Status::Idle | Status::Synced | Status::Checking => ().into_any(),
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
pub enum Removal {
    Spending(Spending),
    Person(Person),
}

#[component]
pub fn TourPage(id: String, landing: crate::Landing) -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);
    let status = RwSignal::new(Status::Idle);
    // Which tab is open lives here, above the screen that is rebuilt whenever the tour is
    // reloaded - which happens after every edit. Kept inside, it meant that saving an
    // expense answered by throwing the reader back to Balance.
    let tab = RwSignal::new(tab_of(landing));

    // A tour opened on this device before is drawn from what we have, at once, and the
    // server is asked in the background - the way the app itself does it. Waiting for the
    // answer first is a blank screen for as long as the network takes, to show numbers that
    // are almost always the ones already in hand. What arrives replaces it, and the line
    // under the title says which of the two is on screen.
    if let Some(known) = queue::cached(&id) {
        set_state.set(Load::Ready(queue::with_pending(&known)));
        status.set(Status::Checking);
    }

    // Arriving is the same operation as saving: drain whatever is queued, then show what
    // the server has with anything still waiting applied on top. So a reload after an
    // offline edit finishes the job by itself.
    let load = Callback::new({
        let id = id.clone();
        move |_: ()| {
            let id = id.clone();
            // Refreshing by hand says so too, rather than leaving "from the server" up
            // while the server is being asked again.
            if matches!(state.get_untracked(), Load::Ready(_)) {
                status.set(Status::Checking);
            }
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
                <TourView tour=tour reload=load status=status landing=landing tab=tab />
            }.into_any(),
        }}
    }
}

#[component]
fn TourView(
    tour: Tour,
    reload: Callback<()>,
    status: RwSignal<Status>,
    /// Which tab the address asked for, and whether it asked for the expense dialog too.
    landing: crate::Landing,
    /// Which tab is open. Owned by the page above, so that it survives a redraw.
    tab: RwSignal<Tab>,
) -> impl IntoView {
    // Every avatar on this screen can now tell one Дима from another.
    provide_context(crate::ui::Peers(
        tour.persons.iter().map(|p| p.name.clone()).collect(),
    ));

    let dialog: RwSignal<Option<Dialog>> = RwSignal::new(None);
    let tour_id = tour.id.as_str().to_owned();

    // A link straight to "record an expense" opens with the dialog already up: that is what
    // the app's own /tour/x/spending/add does, and it is how the button on a phone's home
    // screen gets somebody to a keypad in one tap.
    if landing == crate::Landing::AddSpending {
        dialog.set(Some(Dialog::Spending(SpendingDraft::new(&tour))));
    }

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
        // Dividing in whole cents leaves dust, and the settlement dutifully proposes moving
        // it: on one of the tours here, six separate payments of two cents. The app does not
        // show them and neither does this - below the threshold is not money anybody is
        // going to hand over. They are only dropped from the *display*: the People tab is
        // given the settlement entire, because "has this person anything at all to pay"
        // is a question about the dust too.
        let worth_showing =
            |t: &&Transfer| tour.convert(t.amount, &t.currency).abs() > crate::settings::threshold(&tour);
        (
            f.into_iter().filter(worth_showing).cloned().collect(),
            b.into_iter().filter(worth_showing).cloned().collect(),
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
    let tour_for_share = tour.clone();
    let tour_for_rename = tour.clone();
    let tour_for_currency = tour.clone();
    let unit_stats = unit.clone();
    let real_for_stats = real.clone();
    let tour_for_stats = tour.clone();
    let tour_for_balance = tour.clone();
    let tour_for_people = tour.clone();
    let unit_people = unit.clone();
    // The People tab wants every payment, family ones included: what somebody hands over
    // covers the people they pay for, and that is a different figure from their own debt.
    let all_transfers = transfers.clone();
    let all_for_balance = transfers.clone();
    let tour_for_expenses = tour.clone();
    let tour_for_fab = tour.clone();
    let tour_for_dialog = tour.clone();
    let tour_for_versions = tour.clone();
    let tour_for_explain = tour.clone();
    let tour_for_explain2 = tour.clone();
    let tour_for_explain3 = tour.clone();
    let tour_for_explain4 = tour.clone();
    let tour_for_currencies = tour.clone();
    let tour_id_for_bell = tour_id.clone();
    let fin = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);
    let arch = tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED);

    // One explanation open at a time, wherever it was asked for.
    let explaining: crate::explain::Open = RwSignal::new(None);
    provide_context(explaining);

    let mode = use_context::<RwSignal<crate::mode::UiMode>>()
        .unwrap_or_else(|| RwSignal::new(crate::mode::UiMode::Full));

    // The small interface is the same tour drawn as rows. Everything above it - the sync,
    // the queue, the dialogs - is shared, so this is a choice of surface and not of app.
    let mini_tour = tour.clone();
    let mini_dialog_tour = tour.clone();

    // Read once, not tracked: the page is rebuilt when the switch moves (see `main`), which
    // is what a change of interface should do anyway - every open dialog and every folded
    // row belongs to the surface being left.
    if mode.get_untracked() == crate::mode::UiMode::Mini {
        return view! {
            <crate::mini::MiniTour tour=mini_tour reload=reload status=status tab=tab
                                   apply=apply dialog=dialog delete=delete />
            <crate::mini::MiniDialogs tour=mini_dialog_tour dialog=dialog
                                      close=close apply=apply />
        }
        .into_any();
    }

    view! {
        <div class="tcn-hero">
            <div class="tcn-hero-top">
                <div class="tcn-hero-name">{title.clone()}</div>
            </div>
            <div class="tcn-hero-sub">
                <ShareLink tour=tour_for_share.clone() />
                <span>"·"</span>
                <button type="button" class="tcn-hero-link"
                        on:click={
                            let t = tour_for_rename.clone();
                            move |_| dialog.set(Some(Dialog::Tour(crate::edit::TourDraft::of(&t))))
                        }>
                    "edit"
                </button>
                <span>"·"</span>
                <button type="button" class="tcn-hero-link"
                        on:click=move |_| dialog.set(Some(Dialog::Versions))>
                    "versions"
                </button>
                <span>"·"</span>
                <button type="button" class="tcn-hero-link"
                        on:click=move |_| dialog.set(Some(Dialog::Currencies))>
                    "currencies"
                </button>
                {(tour_for_currency.currencies.len() > 1).then(|| view! {
                    <CurrencyPicker tour=tour_for_currency.clone() apply=apply />
                })}
                <PushBell tour_id=tour_id_for_bell.clone() />
            </div>
            <div class="tcn-hero-sub">
                <Freshness reload=reload status=status />
                {(fin || arch).then(|| view! {
                    <span>
                        {fin.then(|| view! {
                            <span class="tcn-chip tcn-chip-amber"
                                  title="Everyone sees the payments to make">"settling up"</span>
                        })}
                        {arch.then(|| view! {
                            <span class="tcn-chip" title="Hidden from the default list">"archived"</span>
                        })}
                    </span>
                })}
            </div>
            <div class="tcn-hero-metrics">
                <Metric label="Total spent" value=money(total_spent) unit=unit_metrics.clone()
                        what={
                            let t = tour_for_explain.clone();
                            Callback::new(move |()| crate::explain::total_spent(&t))
                        } />
                <Metric label="People" value=people.to_string() unit=String::new()
                        what={
                            let t = tour_for_explain2.clone();
                            Callback::new(move |()| crate::explain::people(&t))
                        } />
                <Metric label="Expenses" value=expenses.to_string() unit=String::new()
                        what={
                            let t = tour_for_explain3.clone();
                            Callback::new(move |()| crate::explain::expenses(&t))
                        } />
                <Metric label="Left to settle" value=money(left_to_settle) unit=unit_metrics.clone()
                        what={
                            let t = tour_for_explain4.clone();
                            let between = between.clone();
                            Callback::new(move |()| crate::explain::left_to_settle(&t, &between))
                        } />
            </div>
        </div>

        <crate::explain::ExplainSheet open=explaining />

        <SyncLine status=status reload=reload tour_id=tour_id.clone() />

        <div class="tcn-section" style="padding-bottom:0">
            <TabButton tab=tab mine=Tab::Balance label="Balance" count=Some(between.len()) />
            <TabButton tab=tab mine=Tab::People label="People" count=Some(people) />
            <TabButton tab=tab mine=Tab::Expenses label="Expenses" count=Some(expenses) />
            <TabButton tab=tab mine=Tab::Stats label="Stats" count=None />
        </div>

        <Show when=move || tab.get() == Tab::Balance>
            <BalanceTab tour=tour_for_balance.clone() apply=apply
                        all_for_summary=all_for_balance.clone() between=between.clone()
                        family=family.clone() unit=unit_balance.clone() />
        </Show>

        <Show when=move || tab.get() == Tab::People>
            <PeopleTab tour=tour_for_people.clone() transfers=all_transfers.clone()
                       unit=unit_people.clone() dialog=dialog delete=delete />
        </Show>

        <Show when=move || tab.get() == Tab::Expenses>
            <ExpensesTab tour=tour_for_expenses.clone() spendings=real.clone()
                         unit=unit_expenses.clone() dialog=dialog delete=delete />
        </Show>

        <Show when=move || tab.get() == Tab::Stats>
            <StatsTab tour=tour_for_stats.clone() spendings=real_for_stats.clone()
                      unit=unit_stats.clone() />
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
                Dialog::Tour(draft) => view! {
                    <TourDialog draft=draft on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Currencies => view! {
                    <CurrenciesDialog tour=tour_for_currencies.clone() on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Versions => view! {
                    <VersionsDialog tour=tour_for_versions.clone() on_close=close />
                }.into_any(),
            })
        }}
    }
    .into_any()
}

#[component]
fn ExpensesTab(
    tour: Tour,
    spendings: Vec<Spending>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let by_amount = RwSignal::new(false);
    let newest_first = RwSignal::new(true);
    let chosen: RwSignal<Vec<String>> = RwSignal::new(Vec::new());

    let categories = {
        let mut cs: Vec<String> = spendings
            .iter()
            .map(|s| s.category.trim().to_owned())
            .filter(|c| !c.is_empty())
            .collect();
        cs.sort();
        cs.dedup();
        cs
    };

    let name_of_id = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <div class="tcn-search">
                    <span class="tcn-search-icon">"🔎"</span>
                    <input type="text" placeholder="Search by description, payer or category"
                           prop:value=move || search.get()
                           on:input=move |ev| search.set(event_target_value(&ev)) />
                    <Show when=move || !search.get().is_empty()>
                        <button type="button" class="tcn-search-clear" title="Clear"
                                on:click=move |_| search.set(String::new())>"✕"</button>
                    </Show>
                </div>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        class:tcn-btn-primary=move || !by_amount.get()
                        on:click=move |_| {
                            if by_amount.get() { by_amount.set(false) } else { newest_first.update(|d| *d = !*d) }
                        }>
                    "Date " {move || if by_amount.get() { "" } else if newest_first.get() { "↓" } else { "↑" }}
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        class:tcn-btn-primary=move || by_amount.get()
                        on:click=move |_| {
                            if by_amount.get() { newest_first.update(|d| *d = !*d) } else { by_amount.set(true) }
                        }>
                    "Amount " {move || if by_amount.get() { if newest_first.get() { "↓" } else { "↑" } } else { "" }}
                </button>
            </div>

            {(!categories.is_empty()).then(|| view! {
                <div class="tcn-chips" style="margin-bottom: 10px;">
                    {categories
                        .iter()
                        .map(|c| {
                            let c = c.clone();
                            let mine = c.clone();
                            view! {
                                <span class="tcn-chip tcn-filter-chip"
                                      class:is-on=move || chosen.get().contains(&mine)
                                      on:click={
                                          let c = c.clone();
                                          move |_| chosen.update(|list| {
                                              if let Some(i) = list.iter().position(|x| x == &c) {
                                                  list.remove(i);
                                              } else {
                                                  list.push(c.clone());
                                              }
                                          })
                                      }>
                                    {c.clone()}
                                </span>
                            }
                        })
                        .collect_view()}
                    <Show when=move || !chosen.get().is_empty()>
                        <span class="tcn-chip tcn-filter-chip"
                              on:click=move |_| chosen.set(Vec::new())>"clear ×"</span>
                    </Show>
                </div>
            })}

            {move || {
                let needle = search.get().trim().to_lowercase();
                let cats = chosen.get();
                let mut shown: Vec<Spending> = spendings
                    .iter()
                    .filter(|s| cats.is_empty() || cats.iter().any(|c| c == s.category.trim()))
                    .filter(|s| {
                        needle.is_empty()
                            || s.description.to_lowercase().contains(&needle)
                            || s.category.to_lowercase().contains(&needle)
                            || name_of_id(&s.from).to_lowercase().contains(&needle)
                    })
                    .cloned()
                    .collect();

                if by_amount.get() {
                    shown.sort_by_key(|s| tour.amount_in_current(s).0);
                } else {
                    // ISO stamps sort as text - see `Spending::when`.
                    shown.sort_by(|a, b| a.when().unwrap_or("").cmp(b.when().unwrap_or("")));
                }
                if newest_first.get() {
                    shown.reverse();
                }

                // What the header counts and what this list counts have to agree, and the
                // difference is worth naming rather than hiding: a payback has no category
                // and is not spending, however much money moved.
                let counted: Cents = shown
                    .iter()
                    .filter(|s| !s.category.trim().is_empty())
                    .map(|s| tour.amount_in_current(s))
                    .sum();
                let uncounted: Cents = shown
                    .iter()
                    .filter(|s| s.category.trim().is_empty())
                    .map(|s| tour.amount_in_current(s))
                    .sum();

                if shown.is_empty() {
                    return view! {
                        <div class="tcn-empty">
                            <span class="tcn-empty-icon">"🔍"</span>
                            <div class="tcn-empty-title">"Nothing matches the filter"</div>
                        </div>
                    }.into_any();
                }

                // Grouped by day when in date order; a list sorted by amount has no days.
                let mut groups: Vec<(String, Vec<Spending>)> = Vec::new();
                if by_amount.get() {
                    groups.push((String::new(), shown.clone()));
                } else {
                    for s in &shown {
                        let day = s.day().unwrap_or("").to_owned();
                        match groups.last_mut() {
                            Some((d, list)) if *d == day => list.push(s.clone()),
                            _ => groups.push((day, vec![s.clone()])),
                        }
                    }
                }

                let unit_here = unit.clone();
                let unit_days = unit.clone();
                let tour_here = tour.clone();
                let tour_for_sums = tour.clone();
                let tour_for_uncounted = tour.clone();
                let tour_for_days = tour.clone();
                let shown_for_sums: Vec<Spending> = shown.iter().map(|s| (*s).clone()).collect();
                let shown_for_uncounted = shown_for_sums.clone();
                view! {
                    <div class="tcn-summary">
                        <span><b>{shown.len()}</b>" expenses"</span>
                        <span>
                            "spent "
                            <crate::explain::Explain what={
                                let tour = tour_for_sums.clone();
                                let rows = shown_for_sums.clone();
                                Callback::new(move |()| crate::explain::shown_total(&tour, &rows))
                            }>
                                <b>{money(counted)}</b>" " {unit_here.clone()}
                            </crate::explain::Explain>
                        </span>
                        {(!uncounted.is_zero()).then(|| view! {
                            <span class="tcn-hint">
                                "+ "
                                <crate::explain::Explain what={
                                    let tour = tour_for_uncounted.clone();
                                    let rows = shown_for_uncounted.clone();
                                    Callback::new(move |()| crate::explain::uncounted(&tour, &rows))
                                }>
                                    {money(uncounted)} " uncounted"
                                </crate::explain::Explain>
                            </span>
                        })}
                    </div>

                    {groups
                        .into_iter()
                        .map(|(day, list)| {
                            let day_total: Cents = list
                                .iter()
                                .filter(|s| !s.category.trim().is_empty())
                                .map(|s| tour_here.amount_in_current(s))
                                .sum();
                            let unit = unit_days.clone();
                            let unit_head = unit_days.clone();
                            let tour = tour_here.clone();
                            let tour_for_days = tour_for_days.clone();
                            view! {
                                <div class="tcn-daygroup">
                                    {(!day.is_empty()).then(|| view! {
                                        <div class="tcn-dayhead">
                                            <span>{pretty_day(&day)}</span>
                                            <span class="tcn-daysum">
                                                <crate::explain::Explain what={
                                                    let tour = tour_for_days.clone();
                                                    let label = pretty_day(&day);
                                                    let rows = list.clone();
                                                    Callback::new(move |()| {
                                                        crate::explain::day(&tour, &label, &rows)
                                                    })
                                                }>
                                                    {money(day_total)}
                                                    {(!unit_head.is_empty())
                                                        .then(|| view! { "\u{a0}" {unit_head.clone()} })}
                                                </crate::explain::Explain>
                                            </span>
                                        </div>
                                    })}
                                    <div class="tcn-list">
                                        {list
                                            .iter()
                                            .map(|s| view! {
                                                <ExpenseRow spending=s.clone() tour=tour.clone()
                                                            unit=unit.clone() dialog=dialog delete=delete />
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
                }.into_any()
            }}
        </div>
    }
}

/// "2021-08-18" as "18.08.2021", which is how the app writes dates.
fn pretty_day(iso: &str) -> String {
    match (iso.get(0..4), iso.get(5..7), iso.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}.{m}.{y}"),
        _ => iso.to_owned(),
    }
}

#[component]
fn ExpenseRow(
    spending: Spending,
    tour: Tour,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let who = name_of(tour.person(&spending.from));
    let shown = tour.amount_in_current(&spending);
    let original = (spending.currency.id != tour.currency().id && tour.currencies.len() > 1)
        .then(|| format!("{} {}", money(spending.amount), spending.currency.name));
    let for_edit = spending.clone();
    let for_delete = spending.clone();
    let for_why = spending.clone();
    let tour_for_why = tour.clone();
    // A payment the app recorded reads as a payment; anything a person typed is theirs.
    let description = match crate::ui::as_service_transfer(&spending.description) {
        Some((from, to)) => format!("{from} → {to}"),
        None if spending.description.trim().is_empty() => "(no description)".to_owned(),
        None => spending.description.clone(),
    };
    let category = spending.category.trim().to_owned();
    // A colour somebody chose marks the row out. What the app writes itself when a payment
    // is recorded - "lightgreen", "lightgray" - is not a choice and does not light up.
    let colour = tc_core::extras::str_of(&spending.extras, crate::edit::COLOUR);
    let marked = crate::ui::is_marked(&colour);
    let mark_style = crate::ui::mark_style(&colour);
    let to_whom = match &spending.split {
        Split::Everyone => "everyone".to_owned(),
        Split::Equally(to) | Split::ByWeight(to) => to
            .iter()
            .map(|id| name_of(tour.person(id)))
            .collect::<Vec<_>>()
            .join(", "),
    };

    let alone =
        matches!(&spending.split, Split::Equally(to) | Split::ByWeight(to) if to.len() == 1);

    view! {
        <div class="tcn-settle" class:tcn-sp-marked=move || marked style=mark_style>
            <div class="tcn-settle-flow">
                <Avatar name=who.clone() />
                <span class="tcn-settle-who"
                      title=if alone { "Charged to one person only." } else { "" }>
                    {description}
                    <small class="tcn-hint">
                        " · " {who.clone()}
                        {(!category.is_empty()).then(|| format!(" · {category}"))}
                        " · for " {to_whom}
                    </small>
                </span>
            </div>
            <div class="tcn-settle-amount">
                <crate::explain::Explain what={
                    let tour = tour_for_why.clone();
                    let s = for_why.clone();
                    Callback::new(move |()| crate::explain::spending(&tour, &s))
                }>
                    {money(shown)}
                    {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                </crate::explain::Explain>
                {original.map(|o| view! { <small class="tcn-hint">" (" {o} ")"</small> })}
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
}

/// Where the money went: by category or by person, with the per-person and per-day figures
/// the app shows.
///
/// No pie chart. The C# version draws one; it is the part of that screen that carries the
/// least and would cost the most here, so the numbers are drawn as bars instead - same
/// information, no library.
#[component]
fn StatsTab(tour: Tour, spendings: Vec<Spending>, unit: String) -> impl IntoView {
    let by_category = RwSignal::new(true);
    // The tour's own length, which the tour dialog sets - not the span of the expenses.
    // They are different questions: a trip is eight days whether or not anybody spent
    // anything on the middle three. The box stays editable, because trying another number
    // is the point of the per-day figure. Falling back to the span keeps an old tour that
    // never had a length from dividing by nothing.
    let days = RwSignal::new(
        tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION)
            .filter(|d| *d > 0)
            .unwrap_or_else(|| days_of(&spendings))
            .to_string(),
    );

    // Only what counts as spending: a payback moves money without anybody spending it.
    let counted: Vec<&Spending> = spendings
        .iter()
        .filter(|s| !s.category.trim().is_empty())
        .collect();

    let total: Cents = counted.iter().map(|s| tour.amount_in_current(s)).sum();

    // "Per person" means per whole share, so a child counted at half does not read as a
    // person here - which is why it is weights and not heads.
    let total_weight: i64 = tour.persons.iter().map(|p| p.weight as i64).sum();
    let whole_share = 100i64;
    let per_share = if total_weight > 0 {
        Cents(total.0 * whole_share / total_weight)
    } else {
        Cents::ZERO
    };

    let tour_for_rows = tour.clone();
    let counted_owned: Vec<Spending> = counted.into_iter().cloned().collect();

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || by_category.get()
                      on:click=move |_| by_category.set(true)>"By category"</span>
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || !by_category.get()
                      on:click=move |_| by_category.set(false)>"By person"</span>
            </div>

            {
                let unit = unit.clone();
                move || {
                    let d = days.get().trim().parse::<i64>().unwrap_or(1).max(1);
                    let unit = unit.clone();
                    view! {
                        <div class="tcn-statgrid">
                            <div class="tcn-statcard">
                                <div class="tcn-statcard-label">"Total"</div>
                                <div class="tcn-statcard-value">
                                    {money(total)} <small>"\u{a0}" {unit.clone()}</small>
                                </div>
                            </div>
                            <div class="tcn-statcard">
                                <div class="tcn-statcard-label">"Per person (100 w)"</div>
                                <div class="tcn-statcard-value">
                                    {money(per_share)} <small>"\u{a0}" {unit.clone()}</small>
                                </div>
                            </div>
                            <div class="tcn-statcard">
                                <div class="tcn-statcard-label">"Per person per day"</div>
                                <div class="tcn-statcard-value">
                                    {money(Cents(per_share.0 / d))} <small>"\u{a0}" {unit.clone()}</small>
                                </div>
                                <div class="tcn-statcard-extra">
                                    "over "
                                    <input class="tcn-input tcn-daysinput" type="number" min="1"
                                           prop:value=move || days.get()
                                           on:input=move |ev| days.set(event_target_value(&ev)) />
                                    " days"
                                </div>
                            </div>
                        </div>
                    }
                }
            }

            {move || {
                let rows = if by_category.get() {
                    group_by(&counted_owned, &tour_for_rows, |s, _| {
                        s.category.trim().to_owned()
                    })
                } else {
                    group_by(&counted_owned, &tour_for_rows, |s, t| {
                        name_of(t.person(&s.from))
                    })
                };
                let biggest = rows.iter().map(|(_, c)| c.0).max().unwrap_or(1).max(1);
                let unit = unit.clone();
                let for_pie = rows.clone();
                let pie_unit = unit.clone();
                view! {
                    // The same numbers as the list below it, drawn as the app draws them.
                    // A proportion is what the eye reads first and the list is what answers
                    // "how much exactly", so both, in that order.
                    {(!for_pie.is_empty())
                        .then(|| view! {
                            <crate::pie::PieChart data=for_pie unit=pie_unit />
                        })}
                    <div class="tcn-section-title" style="margin-top:14px">
                        {move || if by_category.get() { "By category" } else { "By person" }}
                    </div>
                    <div class="tcn-card" style="padding: 12px;">
                        {rows
                            .into_iter()
                            .map(|(label, amount)| {
                                let share = if total.0 > 0 { amount.0 * 100 / total.0 } else { 0 };
                                let width = amount.0 * 100 / biggest;
                                let unit = unit.clone();
                                view! {
                                    <div style="margin-bottom: 12px;">
                                        <div class="tcn-bal-row">
                                            <span class="tcn-bal-name">{label}</span>
                                            <span class="tcn-hint">{share}"%"</span>
                                            <span class="tcn-bal-amount">
                                                {money(amount)}
                                                {(!unit.is_empty())
                                                    .then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                            </span>
                                        </div>
                                        // A plain proportion bar. `tcn-balancebar` is the
                                        // two-sided one - money owed to the left of a
                                        // middle line, owed to you on the right - and a
                                        // share of a total has no such middle, so borrowing
                                        // it drew every category as a dot.
                                        <div style="height:6px;border-radius:999px;background:var(--tcn-surface-2);overflow:hidden">
                                            <div style=format!(
                                                "height:100%;width:{width}%;border-radius:999px;background:var(--tcn-primary)")>
                                            </div>
                                        </div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
            }}
        </div>
    }
}

/// Which tab an address asks for.
pub fn tab_of(landing: crate::Landing) -> Tab {
    match landing {
        crate::Landing::People => Tab::People,
        crate::Landing::Expenses | crate::Landing::AddSpending => Tab::Expenses,
        crate::Landing::Stats => Tab::Stats,
        crate::Landing::Balance => Tab::Balance,
    }
}

/// Adds the spendings up under whatever key the caller picks, largest first.
fn group_by(
    spendings: &[Spending],
    tour: &Tour,
    key: impl Fn(&Spending, &Tour) -> String,
) -> Vec<(String, Cents)> {
    let mut rows: Vec<(String, Cents)> = Vec::new();
    for s in spendings {
        let k = key(s, tour);
        let amount = tour.amount_in_current(s);
        match rows.iter_mut().find(|(label, _)| label == &k) {
            Some((_, sum)) => *sum += amount,
            None => rows.push((k, amount)),
        }
    }
    rows.sort_by_key(|(_, amount)| -amount.0);
    rows
}

/// How many days the tour covers, from the first spending to the last.
fn days_of(spendings: &[Spending]) -> i64 {
    let mut days: Vec<&str> = spendings.iter().filter_map(|s| s.day()).collect();
    days.sort();
    days.dedup();
    // Distinct days with something on them, which is closer to "how long were we there"
    // than counting the calendar between the first and the last.
    days.len().max(1) as i64
}

#[component]
fn BalanceTab(
    tour: Tour,
    /// Recording one of these as paid is an edit like any other.
    apply: Callback<Operation>,
    /// Every suggested payment, including the ones too small to show: the summary needs to
    /// see them to decide who has anything left to pay.
    all_for_summary: Vec<Transfer>,
    between: Vec<Transfer>,
    family: Vec<Transfer>,
    unit: String,
) -> impl IntoView {
    let name_by = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };
    // The whole settlement, dust and all: `settlement_summary` applies the app's own
    // rule for what counts and what is too small to mention.
    let rows = settlement_summary(&tour, &all_for_summary, crate::settings::threshold(&tour));
    let balances_for_bal = tc_core::calculate(&tour, tc_core::Options::default());
    let tour_for_bal = tour.clone();
    let all_for_bal = all_for_summary.clone();
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
                            {transfer_rows(&tour, &between, &name_by, &unit, Some(apply))}
                        </div>
                    }.into_any()
                }
            }

            {
                let unit = unit.clone();
                let name_by = name_by.clone();
                // Folded away to start with, as in the app. Who inside a family owes whom is
                // the household's own arithmetic; the list people came for is the one above.
                let open = RwSignal::new(false);
                (!family.is_empty()).then(move || view! {
                    <div class="tcn-section-title" style="margin-top:18px">
                        <span class="tcn-btn tcn-btn-ghost tcn-btn-sm"
                              on:click=move |_| open.update(|o| *o = !*o)>
                            {move || if open.get() {
                                view! { <crate::icon::Icon name="chevron-down" /> }
                            } else {
                                view! { <crate::icon::Icon name="chevron-right" /> }
                            }}
                            " Inside families"
                        </span>
                        <span class="tcn-count">{family.len()}</span>
                    </div>
                    // No button on the rows: a payment between a child and whoever pays for
                    // them is family business, and the app does not offer to record it either.
                    <Show when=move || open.get()>
                        <div class="tcn-list">
                            {transfer_rows(&tour, &family, &name_by, &unit, None)}
                        </div>
                    </Show>
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
                                                <crate::explain::Explain what={
                                                    let t = tour_for_bal.clone();
                                                    let id = who_id.clone();
                                                    let b = balances_for_bal.clone();
                                                    let all = all_for_bal.clone();
                                                    Callback::new(move |()| match t.person(&id) {
                                                        Some(p) => crate::explain::person_balance(&t, p, &b, &all),
                                                        None => crate::explain::Explanation::new("Balance"),
                                                    })
                                                }>
                                                    {if owes { "owes " } else { "gets " }}
                                                    {money(shown)}
                                                    {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                                </crate::explain::Explain>
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
    tour: &Tour,
    list: &[Transfer],
    name_by: &impl Fn(&PersonId) -> String,
    unit: &str,
    // Where "this has happened" goes, if the row may say so. Family payments settle
    // themselves and are shown without the button, as in the app.
    paid: Option<Callback<Operation>>,
) -> impl IntoView {
    let balances = tc_core::calculate(tour, tc_core::Options::default());
    list.iter()
        .map(|t| {
            let tour_for_why = tour.clone();
            let balances = balances.clone();
            let from = name_by(&t.from);
            let to = name_by(&t.to);
            let unit = unit.to_owned();
            let recording = t.clone();
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
                        <crate::explain::Explain what={
                            let tour = tour_for_why.clone();
                            let t = recording.clone();
                            let balances = balances.clone();
                            Callback::new(move |()| crate::explain::transfer(&tour, &t, &balances))
                        }>
                            {money(recording.amount)}
                            {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                        </crate::explain::Explain>
                    </div>
                    {paid.map(|apply| {
                        let from = from.clone();
                        let to = to.clone();
                        let amount = money(recording.amount);
                        view! {
                            <button type="button" class="tcn-btn tcn-btn-sm"
                                    title="Record that this money has changed hands"
                                    on:click=move |_| {
                                        // Recording a payment is not undoable in one click -
                                        // it becomes an ordinary entry in the list - so it
                                        // asks first, naming what it is about to write down.
                                        let question = format!(
                                            "Record that {from} paid {to} {amount}?"
                                        );
                                        let agreed = web_sys::window()
                                            .and_then(|w| w.confirm_with_message(&question).ok())
                                            .unwrap_or(false);
                                        if !agreed {
                                            return;
                                        }
                                        let mut draft = crate::edit::PaymentDraft::of(&recording);
                                        // The id is settled here, so replaying the queued
                                        // operation twice records one payment.
                                        draft.id = Some(tc_core::SpendingId::new(crate::edit::new_id()));
                                        apply.run(Operation::RecordPayment(draft));
                                    }>
                                "Mark paid"
                            </button>
                        }
                    })}
                </div>
            }
        })
        .collect_view()
}

#[component]
fn TabButton(
    tab: RwSignal<Tab>,
    mine: Tab,
    label: &'static str,
    /// Absent where a number would mean nothing - "Stats 0" says only that nobody thought
    /// about it.
    count: Option<usize>,
) -> impl IntoView {
    view! {
        <button type="button"
                class="tcn-btn"
                class:tcn-btn-primary=move || tab.get() == mine
                style="margin-right:6px"
                on:click=move |_| tab.set(mine)>
            {label}
            {count.map(|c| view! { " " <span class="tcn-count">{c}</span> })}
        </button>
    }
}

#[component]
fn Metric(
    label: &'static str,
    value: String,
    unit: String,
    /// Tapping the figure opens the working behind it.
    what: Callback<(), crate::explain::Explanation>,
) -> impl IntoView {
    view! {
        <div class="tcn-metric">
            <div class="tcn-metric-label">{label}</div>
            <div class="tcn-metric-value">
                <crate::explain::Explain what=what>
                    {value}
                    {(!unit.is_empty()).then(|| view! { <small>"\u{a0}" {unit}</small> })}
                </crate::explain::Explain>
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
