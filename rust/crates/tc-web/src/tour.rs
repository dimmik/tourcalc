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
/// How the last refresh went. The app's four states, and its wording.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    None,
    Updated,
    UpToDate,
    Failed,
}

/// Everything the freshness line needs, owned by the page so that it survives the redraw
/// that follows every edit.
#[derive(Clone, Copy)]
pub struct Refresh {
    /// A request to the server is in flight.
    pub busy: RwSignal<bool>,
    /// How the last one went, cleared a couple of seconds later.
    pub outcome: RwSignal<Outcome>,
    /// The last attempt failed and none has succeeded since.
    pub stale: RwSignal<bool>,
    /// Whether what is on screen came from the server rather than from this device.
    pub fresh: RwSignal<bool>,
    /// When this device's copy was stored, in milliseconds since the epoch.
    pub stored_at: RwSignal<f64>,
    /// Bumped every half minute, so "3 min ago" stops being a lie without anybody asking.
    pub tick: RwSignal<u32>,
}

/// A cheap "did anything actually change" stamp. The app's fingerprint, field for field:
/// counts and totals catch any real edit, and nothing here needs to catch more than that.
fn fingerprint(tour: &Tour) -> String {
    let total: i64 = tour.spendings.iter().map(|s| s.amount.0).sum();
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        tour.name,
        tour.persons.len(),
        tour.spendings.len(),
        total,
        tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION).unwrap_or(0),
        tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING),
        tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED),
        tour.current_currency.as_str(),
    )
}

/// How long ago, in words. Absolute times look identical after two refreshes a minute
/// apart, which is exactly when somebody is looking at this line.
fn ago(stored_at: f64) -> String {
    let seconds = (crate::queue::now_millis() - stored_at) / 1000.0;
    if seconds < 45.0 {
        "just now".to_owned()
    } else if seconds < 3600.0 {
        format!("{} min ago", (seconds / 60.0) as i64)
    } else if seconds < 86_400.0 {
        format!("{} h ago", (seconds / 3600.0) as i64)
    } else {
        crate::ui::local_stamp(stored_at)
    }
}

/// "from server · 3 min ago", and what to say instead when the server did not answer.
///
/// The app answers the question a reader has when the numbers look wrong - "am I looking at
/// something stale?" - and it is worth answering out loud, because both clients will happily
/// show a tour they stored days ago when there is no network.
#[component]
fn Freshness(refresh: Refresh) -> impl IntoView {
    let label = move || {
        // Reading the tick is what makes "3 min ago" become "4 min ago" on its own.
        refresh.tick.get();
        let stored = refresh.stored_at.get();
        if refresh.busy.get() {
            return ("asking the server…".to_owned(), "");
        }
        match refresh.outcome.get() {
            Outcome::Updated => ("✓ new data received".to_owned(), "is-ok"),
            Outcome::UpToDate => ("✓ server has nothing newer".to_owned(), "is-ok"),
            Outcome::Failed => (
                "✕ server did not answer — showing the local copy".to_owned(),
                "is-bad",
            ),
            Outcome::None if refresh.stale.get() => {
                (format!("⚠ local copy · {}", ago(stored)), "is-stale")
            }
            Outcome::None => (
                format!(
                    "{} · {}",
                    if refresh.fresh.get() {
                        "from server"
                    } else {
                        "local copy"
                    },
                    ago(stored)
                ),
                "",
            ),
        }
    };

    view! {
        <span title=move || {
            let when = crate::ui::local_stamp(refresh.stored_at.get());
            if refresh.stale.get() {
                format!("The server could not be reached. This is the copy stored on this \
                         device at {when}.")
            } else {
                when
            }
        }>
            <span class="tcn-refresh-note" class=("is-ok", move || label().1 == "is-ok")
                  class=("is-bad", move || label().1 == "is-bad")
                  class=("is-stale", move || label().1 == "is-stale")>
                {move || label().0}
            </span>
        </span>
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
        <select class="tcn-input" style="width:auto; padding:4px 8px; font-size:13px;"
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

    let refresh = Refresh {
        busy: RwSignal::new(false),
        outcome: RwSignal::new(Outcome::None),
        stale: RwSignal::new(false),
        fresh: RwSignal::new(false),
        stored_at: RwSignal::new(queue::cached_at(&id).unwrap_or_else(queue::now_millis)),
        tick: RwSignal::new(0),
    };
    // The wording of "3 min ago" has to keep up with the clock, and nothing else on this
    // page changes to make it. Half a minute is the app's interval.
    let tick = refresh.tick;
    leptos::prelude::set_interval(
        move || tick.update(|t| *t = t.wrapping_add(1)),
        std::time::Duration::from_secs(30),
    );

    // A tour opened on this device before is drawn from what we have, at once, and the
    // server is asked in the background - the way the app itself does it. Waiting for the
    // answer first is a blank screen for as long as the network takes, to show numbers that
    // are almost always the ones already in hand. What arrives replaces it, and the line
    // under the title says which of the two is on screen.
    // The name in the top bar, set as soon as there is one to set - from the local copy
    // before the server has answered, which is the whole point of having kept it.
    let name_in_the_bar = use_context::<crate::PageTitle>();
    let show_name = move |tour: &Tour| {
        if let Some(t) = name_in_the_bar {
            t.0.set(Some((
                tour.name.clone(),
                format!("/tour/{}", tour.id.as_str()),
            )));
        }
    };

    if let Some(known) = queue::cached(&id) {
        show_name(&known);
        set_state.set(Load::Ready(queue::with_pending(&known)));
        status.set(Status::Checking);
    }

    // Arriving is the same operation as saving: drain whatever is queued, then show what
    // the server has with anything still waiting applied on top. So a reload after an
    // offline edit finishes the job by itself.
    let load = Callback::new({
        let id = id.clone();
        move |_: ()| {
            if refresh.busy.get_untracked() {
                return;
            }
            let id = id.clone();
            refresh.busy.set(true);
            refresh.outcome.set(Outcome::None);
            // What is on screen now, to tell "nothing has changed" from "here is the change".
            let before = match state.get_untracked() {
                Load::Ready(t) => fingerprint(&t),
                _ => String::new(),
            };
            let started = queue::now_millis();
            if matches!(state.get_untracked(), Load::Ready(_)) {
                status.set(Status::Checking);
            }
            spawn_local(async move {
                let (tour, st) = sync::push(&id).await;
                let answered = matches!(st, Status::Idle | Status::Synced);
                status.set(st);

                if let Some(t) = &tour {
                    if answered {
                        refresh.fresh.set(true);
                        refresh.stored_at.set(queue::now_millis());
                    }
                    let outcome = if !answered {
                        Outcome::Failed
                    } else if fingerprint(t) == before {
                        Outcome::UpToDate
                    } else {
                        Outcome::Updated
                    };
                    refresh.outcome.set(outcome);
                    // Anything from the server clears the warning, whoever asked for it.
                    refresh.stale.set(!answered);
                } else {
                    refresh.outcome.set(Outcome::Failed);
                    refresh.stale.set(true);
                }

                if let Some(t) = &tour {
                    show_name(t);
                }
                set_state.set(match tour {
                    Some(t) => Load::Ready(queue::with_pending(&t)),
                    None => Load::Failed(
                        "This tour has not been opened on this device before, and there is \
                         no connection to fetch it."
                            .into(),
                    ),
                });

                // A local server answers in milliseconds; hold the spinner long enough to
                // be seen, or a refresh looks like a button that does nothing.
                let elapsed = queue::now_millis() - started;
                let hold = (450.0 - elapsed).max(0.0) as u64;
                leptos::prelude::set_timeout(
                    move || refresh.busy.set(false),
                    std::time::Duration::from_millis(hold),
                );

                // Then the verdict fades and the line goes back to saying where the copy
                // came from. A failure deserves a longer look than a success.
                let mine = refresh.outcome.get_untracked();
                let shown = if mine == Outcome::Failed { 5000 } else { 2200 };
                leptos::prelude::set_timeout(
                    move || {
                        if refresh.outcome.get_untracked() == mine {
                            refresh.outcome.set(Outcome::None);
                        }
                    },
                    std::time::Duration::from_millis(shown + hold),
                );
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
                <TourView tour=tour reload=load status=status landing=landing tab=tab
                          refresh=refresh />
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
    /// The state of the last refresh, likewise owned above.
    refresh: Refresh,
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
            // Name, edit and reload on one line, as in the app: the two things you do *to*
            // the tour rather than inside it, next to what they act on.
            <div class="tcn-hero-top">
                <div class="tcn-hero-name">{title.clone()}</div>
                <button type="button" class="tcn-iconbtn" title="Edit the tour"
                        aria-label="Edit the tour"
                        on:click={
                            let t = tour_for_rename.clone();
                            move |_| dialog.set(Some(Dialog::Tour(crate::edit::TourDraft::of(&t))))
                        }>
                    <crate::icon::Icon name="edit" />
                </button>
                <button type="button" class="tcn-iconbtn" title="Reload from the server"
                        aria-label="Reload from the server"
                        prop:disabled=move || refresh.busy.get()
                        on:click=move |_| reload.run(())>
                    <span class:tcn-spin=move || refresh.busy.get()>
                        <crate::icon::Icon name="refresh" />
                    </span>
                </button>
            </div>

            {(fin || arch).then(|| view! {
                <div class="tcn-hero-badges">
                    {fin.then(|| view! {
                        <span class="tcn-chip tcn-chip-amber"
                              title="The tour is being settled up">"settling up"</span>
                    })}
                    {arch.then(|| view! {
                        <span class="tcn-chip"
                              title="The tour is archived and hidden from the default list">
                            "archived"
                        </span>
                    })}
                </div>
            })}

            // The same row, in the same order, as the app's: how fresh this is, then the
            // three things you can do with the tour as a whole, then the bell.
            <div class="tcn-hero-sub">
                <Freshness refresh=refresh />
                <span>"·"</span>
                <button type="button" class="tcn-hero-chip"
                        title="Edit the currencies of this tour"
                        on:click=move |_| dialog.set(Some(Dialog::Currencies))>
                    <crate::icon::Icon name="settings" />
                    " currencies"
                </button>
                <span>"·"</span>
                <ShareLink tour=tour_for_share.clone() />
                <span>"·"</span>
                <span class="tcn-legacy-btn">
                    <button type="button" class="tcn-hero-link"
                            on:click=move |_| dialog.set(Some(Dialog::Versions))>
                        "versions"
                    </button>
                </span>
                <PushBell tour_id=tour_id_for_bell.clone() />
            </div>

            {(tour_for_currency.currencies.len() > 1).then(|| view! {
                <div class="tcn-hero-sub" style="margin-top:8px">
                    <span title="Only changes what you see - the tour itself is not touched">
                        "show amounts in"
                    </span>
                    <CurrencyPicker tour=tour_for_currency.clone() apply=apply />
                </div>
            })}

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

        <nav class="tcn-tabs" role="tablist" aria-label="Tour sections">
            <TabButton tab=tab mine=Tab::Balance label="Balance" count=Some(between.len()) />
            <TabButton tab=tab mine=Tab::People label="People" count=Some(people) />
            <TabButton tab=tab mine=Tab::Expenses label="Expenses" count=Some(expenses) />
            <TabButton tab=tab mine=Tab::Stats label="Stats" count=None />
        </nav>

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
                let all_count = spendings.len();
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
                let shown_for_uncounted_label = shown_for_sums.clone();
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
                                    {money(uncounted)} " " {uncounted_label(&shown_for_uncounted_label)}
                                </crate::explain::Explain>
                            </span>
                        })}
                        // Which days this list covers, and whether it is all of them.
                        {span_of(&shown_for_sums).map(|(first, last)| view! {
                            <span>"from " <b>{first}</b> " to " <b>{last}</b></span>
                        })}
                        {(shown.len() != all_count).then(|| view! {
                            <span style="color: var(--tcn-primary)">
                                "filtered out of " {all_count}
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
                            let day_other: Cents = list
                                .iter()
                                .filter(|s| s.category.trim().is_empty())
                                .map(|s| tour_here.amount_in_current(s))
                                .sum();
                            let day_label = uncounted_label(&list);
                            let for_day_uncounted = list.clone();
                            let tour_for_day_uncounted = tour_here.clone();
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
                                                {(!day_other.is_zero()).then(|| view! {
                                                    <span class="tcn-hint">
                                                        "\u{a0}+ "
                                                        <crate::explain::Explain what={
                                                            let tour = tour_for_day_uncounted.clone();
                                                            let rows = for_day_uncounted.clone();
                                                            Callback::new(move |()| {
                                                                crate::explain::uncounted(&tour, &rows)
                                                            })
                                                        }>
                                                            {money(day_other)} " " {day_label}
                                                        </crate::explain::Explain>
                                                    </span>
                                                })}
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

/// The heading over a day's expenses, as the app writes it: "Today", "Yesterday", or
/// "9 Dec 2022". A bare 09.12.2022 is harder to place at a glance, and the two nearest days
/// are the ones somebody is usually looking for.
fn pretty_day(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (Some(y), Some(m), Some(d)) = (iso.get(0..4), iso.get(5..7), iso.get(8..10)) else {
        return iso.to_owned();
    };
    let today = js_sys::Date::new_0();
    let day_before = |back: f64| {
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
            today.get_time() - back * 86_400_000.0,
        ));
        format!(
            "{:04}-{:02}-{:02}",
            d.get_full_year(),
            d.get_month() + 1,
            d.get_date()
        )
    };
    let day = &iso[..10.min(iso.len())];
    if day == day_before(0.0) {
        return "Today".to_owned();
    }
    if day == day_before(1.0) {
        return "Yesterday".to_owned();
    }
    let month = m
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=12).contains(n))
        .map(|n| MONTHS[n - 1])
        .unwrap_or(m);
    // No leading zero on the day, as the invariant "d MMM yyyy" writes it.
    let d = d.trim_start_matches('0');
    format!("{d} {month} {y}")
}

/// What to call the money in a list that is not spending. The app's three answers: drafts
/// and transfers together are just "uncounted", drafts alone are drafts, and otherwise it is
/// somebody settling up. Naming it is the point - "uncounted" on its own begs the question.
fn uncounted_label(rows: &[Spending]) -> &'static str {
    let un: Vec<&Spending> = rows
        .iter()
        .filter(|s| s.category.trim().is_empty())
        .collect();
    let drafts = un
        .iter()
        .filter(|s| matches!(s.kind, Kind::Draft { counted: false }))
        .count();
    let transfers = un.len() - drafts;
    match (drafts, transfers) {
        (0, _) => "settling up",
        (_, 0) if drafts == 1 => "draft",
        (_, 0) => "drafts",
        _ => "uncounted",
    }
}

/// The first and last day the given expenses fall on, written as the app writes them.
fn span_of(shown: &[Spending]) -> Option<(String, String)> {
    let mut days: Vec<&str> = shown.iter().filter_map(|s| s.day()).collect();
    days.sort();
    let first = days.first()?;
    let last = days.last()?;
    let dotted = |iso: &str| match (iso.get(0..4), iso.get(5..7), iso.get(8..10)) {
        (Some(y), Some(m), Some(d)) => format!("{d}.{m}.{y}"),
        _ => iso.to_owned(),
    };
    Some((dotted(first), dotted(last)))
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
    // Which category the totals are about. "" is all of them, and it is what the app calls
    // "everything".
    let selected = RwSignal::new(String::new());
    // The tour's own length, which the tour dialog sets - not the span of the expenses.
    // They are different questions: a trip is eight days whether or not anybody spent
    // anything on the middle three. Four is the app's fallback for a tour that never had
    // one, and the box stays editable because trying another number is the point of the
    // per-day figure.
    let days = RwSignal::new(
        tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION)
            .filter(|d| *d > 0)
            // Five, because that is what the C# model defaults `Duration` to when a tour
            // has never had one written down - not the four the component falls back to,
            // which it only reaches for a tour that stored a zero. Four here made every
            // per-day figure disagree with the app's by a quarter.
            .unwrap_or(5)
            .to_string(),
    );

    // Only what counts as spending: a payback moves money without anybody spending it, and
    // an expense with no category is not in the statistics at all.
    let counted: Vec<Spending> = spendings
        .iter()
        .filter(|s| !s.category.trim().is_empty())
        .cloned()
        .collect();

    let total: Cents = counted.iter().map(|s| tour.amount_in_current(s)).sum();
    let total_weight = match tour.persons.iter().map(|p| p.weight as i64).sum::<i64>() {
        0 => 100,
        w => w,
    };

    let by_cat = group_by(&counted, &tour, |s, _| s.category.trim().to_owned());
    let by_person = group_by(&counted, &tour, |s, t| name_of(t.person(&s.from)));

    // How much weight the money in a category was split across, averaged over its expenses.
    // "Per person" for a category that only three people shared is not the same figure as
    // for one the whole tour did.
    let weight_of = |s: &Spending| -> i64 {
        match &s.split {
            Split::Everyone => tour.persons.iter().map(|p| p.weight as i64).sum(),
            Split::Equally(to) | Split::ByWeight(to) => to
                .iter()
                .filter_map(|id| tour.person(id))
                .map(|p| p.weight as i64)
                .sum(),
        }
    };
    let weight_by_cat: Vec<(String, i64)> = by_cat
        .iter()
        .map(|(name, _)| {
            let of_this: Vec<i64> = counted
                .iter()
                .filter(|s| s.category.trim() == name)
                .map(weight_of)
                .collect();
            let average = if of_this.is_empty() {
                0
            } else {
                // Rounded, as the C# rounds: the average of two people at 100 and one at 50
                // is a weight, not a fraction of one.
                (of_this.iter().sum::<i64>() as f64 / of_this.len() as f64).round() as i64
            };
            (name.clone(), average)
        })
        .collect();

    // A "full share" person: a parent if there are children, otherwise the lightest weight
    // anybody carries. What "per person" is per.
    let adult = tour
        .persons
        .iter()
        .find(|p| p.parent.is_some())
        .and_then(|child| child.parent.as_ref())
        .and_then(|id| tour.person(id))
        .map(|p| p.weight as i64)
        .or_else(|| {
            tour.persons
                .iter()
                .map(|p| p.weight as i64)
                .filter(|w| *w > 0)
                .min()
        })
        .unwrap_or(100);

    let mut category_names: Vec<String> = by_cat.iter().map(|(name, _)| name.clone()).collect();
    category_names.sort();

    // Memos rather than closures: they are read from a dozen places in the view, and a
    // closure read twice is a closure moved twice.
    let spent_by_cat = by_cat.clone();
    let cat_total = Memo::new(move |_| {
        let sel = selected.get();
        if sel.is_empty() {
            total
        } else {
            spent_by_cat
                .iter()
                .find(|(name, _)| *name == sel)
                .map(|(_, c)| *c)
                .unwrap_or(Cents::ZERO)
        }
    });
    let per_person = Memo::new(move |_| {
        let sel = selected.get();
        let weight = if sel.is_empty() {
            total_weight
        } else {
            weight_by_cat
                .iter()
                .find(|(name, _)| *name == sel)
                .map(|(_, w)| *w)
                .filter(|w| *w != 0)
                .unwrap_or(total_weight)
        };
        let weight = if weight == 0 { 100 } else { weight };
        Cents(cat_total.get().0 * adult / weight)
    });
    let per_day = Memo::new(move |_| {
        let d = days.get().trim().parse::<i64>().unwrap_or(1).max(1);
        Cents(per_person.get().0 / d)
    });

    // The same amount seen in another of the tour's currencies. The tour's own rates, which
    // are the authority - the ones stored on a spending go stale.
    // Name, rate, and whether it is the one the tour is being read in - the app marks that
    // column, and without the mark the table is four numbers with nothing to hold on to.
    let current_id = tour.current_currency.clone();
    let rates: Vec<(String, i64, bool)> = {
        let mut list: Vec<(String, i64, bool)> = tour
            .currencies
            .iter()
            .map(|c| (c.name.clone(), c.rate as i64, c.id == current_id))
            .collect();
        list.sort_by_key(|(_, rate, _)| -rate);
        list
    };
    let current_rate = tour.currency().rate as i64;
    let multi = tour.currencies.len() > 1;
    let in_currency = move |amount: Cents, rate: i64| {
        if rate == 0 {
            Cents::ZERO
        } else {
            // Rounded, not truncated: the C# does this in floating point and rounds when it
            // prints, and a cent of difference in a table beside the other client is the
            // kind of thing that makes somebody doubt both.
            Cents((amount.0 as f64 * current_rate as f64 / rate as f64).round() as i64)
        }
    };

    let slices = move || {
        if by_category.get() {
            by_cat.clone()
        } else {
            by_person.clone()
        }
    };
    let nothing = counted.is_empty();
    let unit_for_pie = unit.clone();
    let unit_for_cards = unit.clone();
    let rates_for_table = rates.clone();

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || by_category.get()
                      on:click=move |_| by_category.set(true)>"By category"</span>
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || !by_category.get()
                      on:click=move |_| by_category.set(false)>"By person"</span>
            </div>

            {if nothing {
                view! {
                    <div class="tcn-empty">
                        <span class="tcn-empty-icon">"📊"</span>
                        <div class="tcn-empty-title">"Nothing to chart yet"</div>
                        <div>"Expenses need a category before they show up in the statistics."</div>
                    </div>
                }.into_any()
            } else {
                view! {
                    {move || view! {
                        <crate::pie::PieChart data=slices() unit=unit_for_pie.clone() />
                    }}

                    <div class="tcn-section-title" style="margin-top:14px">"Totals"</div>

                    {(category_names.len() > 1).then(|| {
                        let names = category_names.clone();
                        view! {
                            <div class="tcn-chips" style="margin-bottom:10px">
                                <span class="tcn-chip tcn-filter-chip"
                                      class:is-on=move || selected.get().is_empty()
                                      on:click=move |_| selected.set(String::new())>
                                    "everything"
                                </span>
                                {names
                                    .into_iter()
                                    .map(|name| {
                                        let mine = name.clone();
                                        let picked = name.clone();
                                        view! {
                                            <span class="tcn-chip tcn-filter-chip"
                                                  class:is-on=move || selected.get() == mine
                                                  on:click=move |_| selected.set(picked.clone())>
                                                {name.clone()}
                                            </span>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })}

                    <div class="tcn-statgrid">
                        <div class="tcn-statcard">
                            <div class="tcn-statcard-label">"Total"</div>
                            <div class="tcn-statcard-value">
                                {move || money(cat_total.get())}
                                <small>"\u{a0}" {unit_for_cards.clone()}</small>
                            </div>
                        </div>
                        <div class="tcn-statcard">
                            <div class="tcn-statcard-label">{format!("Per person ({adult} w)")}</div>
                            <div class="tcn-statcard-value">
                                {move || money(per_person.get())}
                                <small>"\u{a0}" {unit_for_cards.clone()}</small>
                            </div>
                        </div>
                        <div class="tcn-statcard">
                            <div class="tcn-statcard-label">"Per person per day"</div>
                            <div class="tcn-statcard-value">
                                {move || money(per_day.get())}
                                <small>"\u{a0}" {unit_for_cards.clone()}</small>
                            </div>
                            <div class="tcn-statcard-extra">
                                "over "
                                <input class="tcn-input tcn-daysinput" type="number" min="1" max="50"
                                       prop:value=move || days.get()
                                       on:input=move |ev| days.set(event_target_value(&ev)) />
                                " days"
                            </div>
                        </div>
                    </div>

                    {multi.then(|| {
                        let rates = rates_for_table.clone();
                        let head = rates.clone();
                        let total_row = rates.clone();
                        let person_row = rates.clone();
                        let day_row = rates;
                        view! {
                            <div class="tcn-card tcn-scroll-x" style="margin-top:12px">
                                <table class="tcn-ratetable">
                                    <thead>
                                        <tr>
                                            <th></th>
                                            {head
                                                .into_iter()
                                                .map(|(name, _, main)| view! {
                                                    <th class:is-main=main>{name}</th>
                                                })
                                                .collect_view()}
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr>
                                            <td>"Total"</td>
                                            {total_row
                                                .into_iter()
                                                .map(|(_, rate, _)| view! {
                                                    <td>{move || money(in_currency(cat_total.get(), rate))}</td>
                                                })
                                                .collect_view()}
                                        </tr>
                                        <tr>
                                            <td>"Per person"</td>
                                            {person_row
                                                .into_iter()
                                                .map(|(_, rate, _)| view! {
                                                    <td>{move || money(in_currency(per_person.get(), rate))}</td>
                                                })
                                                .collect_view()}
                                        </tr>
                                        <tr>
                                            <td>"Per day"</td>
                                            {day_row
                                                .into_iter()
                                                .map(|(_, rate, _)| view! {
                                                    <td>{move || money(in_currency(per_day.get(), rate))}</td>
                                                })
                                                .collect_view()}
                                        </tr>
                                    </tbody>
                                </table>
                            </div>
                        }
                    })}
                }.into_any()
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
                        // Which side of the middle line means what. Two words, and without
                        // them the bar is a decoration.
                        <div style="display:flex; justify-content:space-between; font-size:11px;
                                    color: var(--tcn-faint); text-transform:uppercase;
                                    letter-spacing:.5px;">
                            <span>"gets money back"</span>
                            <span>"owes money"</span>
                        </div>
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
        <button type="button" role="tab" class="tcn-tab"
                class:is-active=move || tab.get() == mine
                aria-selected=move || (tab.get() == mine).to_string()
                on:click=move |_| tab.set(mine)>
            {label}
            {count.map(|c| view! { " " <span class="tcn-tab-badge">{c}</span> })}
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
