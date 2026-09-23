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

#[derive(Clone, Copy, PartialEq, Debug)]
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
/// How the last refresh went.
///
/// A failure carries what kind it was. "The server did not answer" and "the server answered
/// 409" are not the same news: the first is the network and waiting may fix it, the second
/// is the server saying no to this very request, and a tour that could not be saved read as
/// a tour nobody could reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    None,
    Updated,
    UpToDate,
    Failed(crate::api::Trouble),
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

/// What the reader has done to the expense list and to the ring: the search box, the sort,
/// the categories picked out, which slice is open. Owned by the page for the same reason
/// the open tab is - the screen below is rebuilt after every edit, and a filter that saving
/// an expense throws away is a filter nobody can use while editing.
#[derive(Clone, Copy)]
pub struct Sifting {
    /// The expense list's search box.
    pub search: RwSignal<String>,
    /// Sort by amount rather than by date.
    pub by_amount: RwSignal<bool>,
    /// Which way round that sort goes.
    pub newest_first: RwSignal<bool>,
    /// The categories the reader has picked out of the list, empty for all of them.
    pub chosen: RwSignal<Vec<String>>,
    /// Stats: by category rather than by person.
    pub by_category: RwSignal<bool>,
    /// Stats: what is chosen on the ring - a category, a head of several, or a person.
    pub ring: RwSignal<String>,
    /// Stats: which head we are inside, if any.
    pub drill: RwSignal<Option<String>>,
    /// Expenses: the one expense unfolded to show its details, by id. One at a time, and
    /// kept here so that an edit elsewhere on the page does not fold it back up.
    pub unfolded: RwSignal<Option<String>>,
}

impl Sifting {
    fn new() -> Self {
        Self {
            search: RwSignal::new(String::new()),
            by_amount: RwSignal::new(false),
            newest_first: RwSignal::new(true),
            chosen: RwSignal::new(Vec::new()),
            by_category: RwSignal::new(true),
            ring: RwSignal::new(String::new()),
            drill: RwSignal::new(None),
            unfolded: RwSignal::new(None),
        }
    }
}

/// What went wrong behind a status, for the line that has to say it.
fn trouble_in(status: &Status) -> crate::api::Trouble {
    match status {
        Status::Failed(f) => f.why,
        // Waiting is the network by definition, and nothing else here is a failure at all.
        _ => crate::api::Trouble::Unreachable,
    }
}

/// What says whether the server's tour has changed: its `StateGUID`, which every save moves.
///
/// The fingerprint below is the app's, and it misses any edit that keeps the counts and the
/// total - a new description, another payer, a date - so a refresh that had just brought
/// one in answered "nothing newer". It stays for a tour with no state, which only a tour
/// never saved by a server can be.
fn fingerprint(tour: &Tour) -> String {
    let state = tc_core::extras::str_of(&tour.extras, tc_core::extras::STATE);
    if state.is_empty() {
        counts_and_totals(tour)
    } else {
        state
    }
}

/// The app's own "did anything actually change" stamp, field for field.
fn counts_and_totals(tour: &Tour) -> String {
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

/// What went wrong, in the words the line has room for.
///
/// The three answers are different things to do about it: wait (nobody answered), look
/// (the server refused this request and will refuse it again), or wait differently (the
/// server itself is in trouble, and the request was fine).
fn said_of(why: crate::api::Trouble) -> String {
    use crate::api::Trouble;
    match why {
        Trouble::Unreachable => "server did not answer".to_owned(),
        Trouble::Ours => "could not read the server's answer".to_owned(),
        Trouble::Answered(404) => "this tour is not on the server".to_owned(),
        Trouble::Answered(409) => "the server would not take the change (409)".to_owned(),
        Trouble::Answered(403) => "the server would not allow it (403)".to_owned(),
        Trouble::Answered(code) if code >= 500 => format!("the server is in trouble ({code})"),
        Trouble::Answered(code) => format!("the server refused it ({code})"),
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
            Outcome::Failed(why) => (
                format!("✕ {} — showing the local copy", said_of(why)),
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
    let said = crate::ui::Brief::new();

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
                    said.say("link copied");
                }>
            {move || if said.is_on() { "link copied" } else { "share link" }}
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
fn SyncLine(status: RwSignal<Status>, reload: Callback<bool>, tour_id: String) -> impl IntoView {
    let tour_for_status = tour_id.clone();
    let tour_for_buttons = StoredValue::new(tour_id.clone());
    view! {
        // What is waiting is a fact about this device, not about the last request: it is
        // read from the queue, and the queue says when it changes. Tied to the request, the
        // line went out for as long as any load said "checking" - which is every load, and
        // one runs after every edit.
        {move || {
            queue::changes();
            // Naming the edits rather than counting them: "2 changes waiting" invites the
            // question this can answer directly. Three names and a count: with eight
            // waiting the line ran off the side of a phone, and "and 5 more" is what the
            // ninth one is worth anyway.
            let waiting: Vec<String> =
                queue::pending(&tour_id).iter().map(|op| op.describe()).collect();
            if waiting.is_empty() {
                return ().into_any();
            }
            let what = match waiting.len() {
                0..=3 => waiting.join(", "),
                n => format!("{}, and {} more", waiting[..3].join(", "), n - 3),
            };
            // The server has said no often enough that nothing sends these by itself any
            // more: the reader decides whether to try again or let them go.
            if let Some(refused) = queue::refused(&tour_id).filter(|r| r.given_up()) {
                let count = waiting.len();
                return view! {
                    <div class="tcn-section" style="padding-bottom:0">
                        <div class="tcn-errors tcw-wraps">
                            {format!(
                                "The server did not take {}: {what}. It said: {}",
                                if count == 1 { "this edit".to_owned() } else { format!("these {count} edits") },
                                refused.why
                            )}
                            <div class="tcw-refused-actions">
                                <button type="button" class="tcn-btn tcn-btn-sm"
                                        on:click=move |_| {
                                            queue::not_refused(&tour_for_buttons.get_value());
                                            reload.run(true);
                                        }>
                                    "Try again"
                                </button>
                                <button type="button" class="tcn-btn tcn-btn-sm"
                                        on:click=move |_| {
                                            let sure = web_sys::window()
                                                .and_then(|w| w.confirm_with_message(
                                                    "Throw away the edits that were not sent? \
                                                     They exist only on this device."
                                                ).ok())
                                                .unwrap_or(false);
                                            if sure {
                                                queue::discard(&tour_for_buttons.get_value());
                                                reload.run(true);
                                            }
                                        }>
                                    "Discard them"
                                </button>
                            </div>
                        </div>
                    </div>
                }.into_any();
            }
            view! {
                <div class="tcn-section" style="padding-bottom:0">
                    <div class="tcn-chip tcn-chip-amber tcw-wraps">
                        {format!("Saved here, waiting to be sent: {what}")}
                    </div>
                </div>
            }.into_any()
        }}
        // Edits that reached the server after what they edited had been deleted there.
        // The delete stood; this says so once, until dismissed.
        {move || {
            queue::changes();
            let lost = queue::lost(&tour_for_buttons.get_value());
            if lost.is_empty() {
                return ().into_any();
            }
            let what = match lost.len() {
                1 => format!("{} was deleted by somebody else, so your edit to it was dropped.", lost[0]),
                _ => format!(
                    "{} were deleted by somebody else, so your edits to them were dropped.",
                    lost.join(", ")
                ),
            };
            view! {
                <div class="tcn-section" style="padding-bottom:0">
                    <div class="tcn-chip tcn-chip-amber tcw-wraps">
                        {what}
                        <button type="button" class="tcw-others-close" aria-label="Dismiss"
                                on:click=move |_| queue::set_lost(&tour_for_buttons.get_value(), None)>
                            "×"
                        </button>
                    </div>
                </div>
            }.into_any()
        }}
        {move || match status.get() {
            // Nothing of this device's is waiting and the server was not reached: the copy
            // on screen is what this device had.
            Status::Waiting(0) => view! {
                <div class="tcn-section" style="padding-bottom:0">
                    <div class="tcn-chip tcn-chip-amber tcw-wraps">
                        "Offline — showing what this device had last"
                    </div>
                </div>
            }.into_any(),
            Status::Idle | Status::Synced | Status::Checking | Status::Waiting(_) => {
                ().into_any()
            }
            // Said above, with what to do about it.
            Status::Failed(_) if queue::given_up(&tour_for_status) => ().into_any(),
            Status::Failed(why) => { let why = why.to_string(); view! {
                <div class="tcn-section" style="padding-bottom:0">
                    <div class="tcn-errors">
                        {why}
                        <button type="button" class="tcn-btn tcn-btn-sm" style="margin-left:10px"
                                on:click=move |_| reload.run(true)>
                            "Try again"
                        </button>
                    </div>
                </div>
            }.into_any() }
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
    // An address that names a tab wins; otherwise the tab this tour was last on, and only
    // then the one it opens on.
    let asked_for = !matches!(landing, crate::Landing::Unsaid);
    let left_on = (!asked_for).then(|| tab_left_on(&id)).flatten();
    let tab = RwSignal::new(left_on.unwrap_or_else(|| tab_of(landing)));
    // Whether that was an answer or a placeholder. The app settles this once and leaves it:
    // a reader who has moved to another tab does not want the next refresh moving them back.
    let tab_settled = RwSignal::new(asked_for || left_on.is_some());
    {
        let id = id.clone();
        Effect::new(move |_| leaving_tab(&id, tab.get()));
    }
    let settle_tab = move |tour: &Tour| {
        if !tab_settled.get_untracked() {
            tab.set(opens_on(tour));
            tab_settled.set(true);
        }
    };

    // What the reader has filtered the list down to, owned here for the same reason.
    let sifting = Sifting::new();
    // And the same for the People tab: whose card is open, what is in its search box.
    let people_state = crate::people::People::new(&id);

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
    //
    // Taken down with the page. Without `on_cleanup` every tour ever opened left a timer
    // behind, waking the tab to bump a signal of a screen that is gone - four tours, four
    // timers, still ticking on the tour list.
    let tick = refresh.tick;
    if let Ok(clock) = leptos::prelude::set_interval_with_handle(
        move || {
            tick.try_update(|t| *t = t.wrapping_add(1));
        },
        std::time::Duration::from_secs(30),
    ) {
        on_cleanup(move || clock.clear());
    }

    // A tour opened on this device before is drawn from what we have, at once, and the
    // server is asked in the background - the way the app itself does it. Waiting for the
    // answer first is a blank screen for as long as the network takes, to show numbers that
    // are almost always the ones already in hand. What arrives replaces it, and the line
    // under the title says which of the two is on screen.
    // The name in the top bar, set as soon as there is one to set - from the local copy
    // before the server has answered, which is the whole point of having kept it.
    let where_we_are = use_context::<crate::place::Current>();
    let show_name = move |tour: &Tour| {
        if let Some(place) = where_we_are {
            crate::place::name_is(place, tour.id.as_str(), &tour.name);
        }
    };

    // Leaving the tour closes whatever was open on it.
    if let Some(editing) = use_context::<crate::Editing>() {
        on_cleanup(move || {
            editing.0.try_set(false);
        });
    }

    // Who else is changing this tour: asked while the page is open, see `others`. Owned
    // here, above the view that every load rebuilds, and handed down as context.
    let others = crate::others::Others::new();
    provide_context(others);

    if let Some(known) = queue::cached(&id) {
        others.showing(&known);
        show_name(&known);
        settle_tab(&known);
        set_state.set(Load::Ready(queue::with_pending(&known)));
        status.set(Status::Checking);
    }

    // Arriving is the same operation as saving: drain whatever is queued, then show what
    // the server has with anything still waiting applied on top. So a reload after an
    // offline edit finishes the job by itself.
    // `true` means a reader pressed refresh. Arriving on the tour and saving an edit reload
    // it too, and neither of those should announce itself: the spinner and the green "✓
    // server has nothing newer" are an answer to a question, and nobody asked one. The app
    // draws the same line and sets that state only from its refresh button.
    let load = Callback::new({
        let id = id.clone();
        move |asked: bool| {
            if refresh.busy.get_untracked() {
                others.missed();
                return;
            }
            // A fetch `others` asked for only fetches: it must never become a second send of
            // a queue that a save is already sending.
            let quiet = others.fetch_only();
            others.loading.set(true);
            let id = id.clone();
            if asked {
                refresh.busy.set(true);
            }
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
                let (tour, st) = if quiet {
                    sync::fetch(&id).await
                } else {
                    sync::push(&id).await
                };
                let answered = matches!(st, Status::Idle | Status::Synced);
                // What went wrong, kept before the status is handed over to the screen.
                let why = trouble_in(&st);
                status.set(st);

                if let Some(t) = &tour {
                    if answered {
                        refresh.fresh.set(true);
                        refresh.stored_at.set(queue::now_millis());
                    }
                    if asked {
                        refresh.outcome.set(if !answered {
                            Outcome::Failed(why)
                        } else if fingerprint(t) == before {
                            Outcome::UpToDate
                        } else {
                            Outcome::Updated
                        });
                    }
                    // The warning is not a verdict on one press: it says the copy on screen
                    // is this device's, and it goes as soon as anything arrives from the
                    // server, whoever asked for it.
                    refresh.stale.set(!answered);
                } else {
                    if asked {
                        refresh.outcome.set(Outcome::Failed(why));
                    }
                    refresh.stale.set(true);
                }

                if let Some(t) = &tour {
                    show_name(t);
                    settle_tab(t);
                }
                if let Some(t) = &tour {
                    others.showing(t);
                }
                match &tour {
                    Some(t) if answered => others.landed(t),
                    _ => others.missed(),
                }
                others.loading.try_set(false);
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
                let hold = if asked {
                    (450.0 - elapsed).max(0.0) as u64
                } else {
                    0
                };
                leptos::prelude::set_timeout(
                    move || refresh.busy.set(false),
                    std::time::Duration::from_millis(hold),
                );

                // Then the verdict fades and the line goes back to saying where the copy
                // came from. A failure deserves a longer look than a success.
                let mine = refresh.outcome.get_untracked();
                let shown = if matches!(mine, Outcome::Failed(_)) { 5000 } else { 2200 };
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
    load.run(false);
    others.watch(id.clone(), load, status);

    view! {
        <crate::others::OthersLine others=others />
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
                          refresh=refresh sifting=sifting people=people_state />
            }.into_any(),
        }}
    }
}

#[component]
fn TourView(
    tour: Tour,
    reload: Callback<bool>,
    status: RwSignal<Status>,
    /// Which tab the address asked for, and whether it asked for the expense dialog too.
    landing: crate::Landing,
    /// Which tab is open. Owned by the page above, so that it survives a redraw.
    tab: RwSignal<Tab>,
    /// The state of the last refresh, likewise owned above.
    refresh: Refresh,
    /// What the list and the ring are filtered to, likewise owned above.
    sifting: Sifting,
    /// What is open and typed on the People tab, likewise owned above.
    people: crate::people::People,
) -> impl IntoView {
    // Every avatar on this screen can now tell one Дима from another.
    provide_context(crate::ui::Peers(
        tour.persons.iter().map(|p| p.name.clone()).collect(),
    ));

    let dialog: RwSignal<Option<Dialog>> = RwSignal::new(None);
    let tour_id = tour.id.as_str().to_owned();

    // The page above has to know when a form is open, to keep somebody else's change from
    // replacing the tour under it.
    let others = use_context::<crate::others::Others>();
    // The page above needs it to keep somebody else's change from replacing the tour under
    // an open form; the app above that needs it for a tapped notification, which would
    // otherwise walk out of the form and take what is typed in it with it.
    let editing = use_context::<crate::Editing>();
    Effect::new(move |_| {
        let open = dialog.with(|d| d.is_some());
        if let Some(o) = others {
            o.editing.set(open);
        }
        if let Some(e) = editing {
            e.0.set(open);
        }
    });

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
            // Counted before the form closes: closing it is what makes `others` look again,
            // and this save moves the state as surely as anybody else's would.
            if let Some(o) = others {
                o.saving.update(|n| *n += 1);
            }
            dialog.set(None);
            spawn_local(async move {
                let (_, st) = sync::record(&tour_id, op).await;
                if let Some(o) = others {
                    o.saving.try_update(|n| *n = n.saturating_sub(1));
                }
                status.set(st);
                reload.run(false);
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

    // What the app counts as spending - a payback is not an expense - worked out by the
    // same function the list's figure comes from, so the two cannot drift apart.
    let total_spent: Cents = tc_core::spent_on_expenses(&tour);

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

    let how_many_people = tour.persons.len();
    let expenses = real.len();
    let title = tour.name.clone();

    // Deleting is the one edit with no dialog, so it does its own asking.
    let tour_for_delete = StoredValue::new(tour.clone());
    let delete = Callback::new(move |what: Removal| {
        let question = match &what {
            Removal::Spending(s) => format!("Delete '{}'?", s.description),
            Removal::Person(p) => {
                // Somebody an expense still holds is not removed - see tc_core::removal -
                // and the reader is told which expenses, rather than asked a question whose
                // answer would be refused.
                let tour = tour_for_delete.get_value();
                let holds = tc_core::removal::what_holds(&tour, &p.id);
                if !holds.is_empty() {
                    if let Some(w) = web_sys::window() {
                        let _ = w.alert_with_message(&holds.explain(&p.name));
                    }
                    return;
                }
                // What goes with them, in the order somebody would notice it: the money
                // first, then the family.
                let mut also = Vec::new();
                match tc_core::removal::shared_in(&tour, &p.id) {
                    0 => {}
                    1 => also.push(
                        "their part of 1 shared expense goes to the others on it".to_owned(),
                    ),
                    n => also.push(format!(
                        "their part of {n} shared expenses goes to the others on them"
                    )),
                }
                match tc_core::removal::children_of(&tour, &p.id) {
                    0 => {}
                    1 => also.push("1 person leaves their family".to_owned()),
                    n => also.push(format!("{n} people leave their family")),
                }
                match also.len() {
                    0 => format!("Delete '{}'?", p.name),
                    _ => format!("Delete '{}'? Then {}.", p.name, also.join(", and ")),
                }
            }
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
                        on:click=move |_| reload.run(true)>
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
                <Metric label="People" value=how_many_people.to_string() unit=String::new()
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
            <TabButton tab=tab mine=Tab::People label="People" count=Some(how_many_people) />
            <TabButton tab=tab mine=Tab::Expenses label="Expenses" count=Some(expenses) />
            <TabButton tab=tab mine=Tab::Stats label="Stats" count=None />
        </nav>

        <Show when=move || tab.get() == Tab::Balance>
            <BalanceTab tour=tour_for_balance.clone() apply=apply
                        all_for_summary=all_for_balance.clone() between=between.clone()
                        family=family.clone() unit=unit_balance.clone() />
        </Show>

        <Show when=move || tab.get() == Tab::People>
            <PeopleTab tour=tour_for_people.clone() people=people
                       transfers=all_transfers.clone()
                       unit=unit_people.clone() dialog=dialog delete=delete />
        </Show>

        <Show when=move || tab.get() == Tab::Expenses>
            <ExpensesTab tour=tour_for_expenses.clone() spendings=real.clone()
                         unit=unit_expenses.clone() dialog=dialog delete=delete
                         sifting=sifting />
        </Show>

        <Show when=move || tab.get() == Tab::Stats>
            <StatsTab tour=tour_for_stats.clone() spendings=real_for_stats.clone()
                      unit=unit_stats.clone() sifting=sifting />
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
                Dialog::Spending(draft) => {
                    // A blank new expense opens where the last one was left off.
                    let (draft, carried) = crate::drafts::carry_spending(&tour, draft);
                    view! {
                        <SpendingDialog tour=tour draft=draft carried_over=carried
                                        on_close=close on_apply=apply />
                    }.into_any()
                }
                Dialog::Person(draft) => {
                    let (draft, carried) = crate::drafts::carry_person(&tour, draft);
                    view! {
                        <PersonDialog tour=tour draft=draft carried_over=carried
                                      on_close=close on_apply=apply />
                    }.into_any()
                }
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
    sifting: Sifting,
) -> impl IntoView {
    let Sifting { search, by_amount, newest_first, chosen, unfolded, .. } = sifting;

    let categories = categories_in_order(&spendings);

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

                order_rows(&mut shown, by_amount.get(), newest_first.get(), &tour);

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
                                                            unit=unit.clone() dialog=dialog delete=delete
                                                            unfolded=unfolded />
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

/// The categories the filter row offers, in the order the app puts them in.
///
/// The app sorts them with `OrderBy`, which asks the culture and not the code points:
/// "бухать" comes before "Гнездо" there and after it under a plain byte-order sort, which
/// is how the two lists came to disagree. Folding case in the key is what that difference
/// amounts to on this data.
fn categories_in_order(spendings: &[Spending]) -> Vec<String> {
    let mut cs: Vec<String> = spendings
        .iter()
        .map(|s| s.category.trim().to_owned())
        .filter(|c| !c.is_empty())
        .collect();
    cs.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    cs.dedup();
    cs
}

/// Put the expenses in the order the toolbar asks for.
///
/// Sorted the way round that is wanted rather than sorted and then reversed: every expense
/// on a day carries the same stamp, and reversing turns the order they were entered in
/// upside down - which is why one day read one way here and the other way in the app.
/// `sort_by` is stable, so equal keys keep the order the tour stores them in, whichever way
/// the list faces - exactly what `OrderBy` and `OrderByDescending` leave behind.
pub fn order_rows(shown: &mut [Spending], by_amount: bool, newest_first: bool, tour: &Tour) {
    let facing = |ord: std::cmp::Ordering| if newest_first { ord.reverse() } else { ord };
    if by_amount {
        shown.sort_by(|a, b| facing(tour.amount_in_current(a).0.cmp(&tour.amount_in_current(b).0)));
    } else {
        // ISO stamps sort as text - see `Spending::when`.
        shown.sort_by(|a, b| facing(a.when().unwrap_or("").cmp(b.when().unwrap_or(""))));
    }
}

#[component]
fn ExpenseRow(
    spending: Spending,
    tour: Tour,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
    /// Which expense is showing its details; see [`Sifting::unfolded`].
    unfolded: RwSignal<Option<String>>,
) -> impl IntoView {
    let who = name_of(tour.person(&spending.from));
    // Tapping the expense unfolds what it is - who paid, when, who it is for and what each
    // of them carries, who it leaves out - without opening a form to find out. The edit is
    // a button; the row itself is for reading.
    let my_id = spending.id.as_str().to_owned();
    let is_open = {
        let my_id = my_id.clone();
        Memo::new(move |_| unfolded.with(|u| u.as_deref() == Some(my_id.as_str())))
    };
    let fold = {
        let my_id = my_id.clone();
        move || {
            unfolded.update(|u| {
                *u = if u.as_deref() == Some(my_id.as_str()) {
                    None
                } else {
                    Some(my_id.clone())
                };
            })
        }
    };
    let for_details = spending.clone();
    let tour_for_details = tour.clone();
    let shown = tour.amount_in_current(&spending);
    let original = (spending.currency.id != tour.currency().id && tour.currencies.len() > 1)
        .then(|| format!("{} {}", money(spending.amount), spending.currency.name));
    let for_edit = spending.clone();
    let for_delete = spending.clone();
    let for_why = spending.clone();
    let tour_for_why = tour.clone();
    let others = use_context::<crate::others::Others>();
    let lit_id = spending.id.as_str().to_owned();
    let lit = move || others.is_some_and(|o| o.lit(&lit_id));
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
    // Who it was for. Almost every expense is for everybody, so saying so on every row is
    // the one thing on the line that never carries information - while the handful that are
    // *not* (the restaurant half the party went to, the wine the children are not drinking)
    // are exactly what somebody scanning the list is looking for. So the common case says
    // nothing and the exception is marked.
    let whose: Vec<String> = match &spending.split {
        Split::Everyone => Vec::new(),
        Split::Equally(to) | Split::ByWeight(to) => {
            let mut names: Vec<String> = to.iter().map(|id| name_of(tour.person(id))).collect();
            names.sort();
            names
        }
    };
    // A payback is for one person by definition - marking that as an exception says
    // nothing. The app names it instead, and so does this.
    let service = crate::ui::as_service_transfer(&spending.description).map(|_| {
        if spending.description.starts_with("Family ") {
            "inside family"
        } else {
            "payback"
        }
    });
    let some_of_them = !whose.is_empty() && service.is_none();
    // And the common case says so too, quietly: in the grey line with the payer, not on a
    // chip. It is the answer to a question somebody does ask - "is this one everybody's?" -
    // and the row should not make them open the expense to find out. Where it does not
    // belong is beside the exceptions, competing with them for the eye.
    let everyone = whose.is_empty() && service.is_none();
    // Two names fit on a row; nine do not, and "4 of 9" is the thing worth knowing at a
    // glance anyway. The full list is on the row's tooltip either way.
    let equally = matches!(spending.split, Split::Equally(_)) && whose.len() > 1;
    let for_chip = match whose.len() {
        0 => String::new(),
        1..=2 => format!("for {}", whose.join(", ")),
        n => format!("for {n} of {}", tour.persons.len()),
    };
    // By weight is the rule; equal shares are the exception, and the chip says so - the
    // app marks them the same way.
    let for_chip = if equally {
        format!("{for_chip} · equally")
    } else {
        for_chip
    };
    let for_title = if some_of_them {
        let mut why = format!("For {}. Tap for who carries how much.", whose.join(", "));
        if whose.len() == 1 {
            why = format!("Charged to {} alone.", whose[0]);
        } else if matches!(&spending.split, Split::Equally(_)) {
            why.push_str(" Split equally.");
        }
        why
    } else {
        String::new()
    };

    // Green for a payback, cyan for a transfer inside a family - the app's own colours, on
    // the row rather than only in a chip. A row the calculator wrote for itself is the one
    // kind of line in the list that is not somebody's expense, and that is worth seeing
    // without reading.
    let family = service == Some("inside family");
    view! {
        <div class="tcn-settle"
             class:tcw-kind=move || service.is_some()
             class:tcw-payback=move || service.is_some() && !family
             class:tcw-family=move || family
             class:tcw-lit=lit
             class:tcw-unfolded=move || is_open.get()
             class:tcn-sp-marked=move || marked style=mark_style>
            <div class="tcn-settle-flow tcw-unfold" role="button" tabindex="0"
                 aria-expanded=move || is_open.get().to_string()
                 title="Details"
                 on:click={
                     let fold = fold.clone();
                     move |_| fold()
                 }
                 on:keydown={
                     let fold = fold.clone();
                     move |ev: leptos::ev::KeyboardEvent| {
                         if ev.key() == "Enter" || ev.key() == " " {
                             ev.prevent_default();
                             fold();
                         }
                     }
                 }>
                <Avatar name=who.clone() />
                <span class="tcn-settle-who">
                    {description}
                    <small class="tcn-hint">
                        " · " {who.clone()}
                        {everyone.then(|| " · for everyone")}
                    </small>
                </span>
                // Outside the name, not inside it: that span ellipsises a long description,
                // and a chip put in with it is the first thing the ellipsis eats.
                {service.map(|what| view! {
                    <span class="tcn-chip" style="flex:0 0 auto">{what}</span>
                })}
                {(!category.is_empty()).then(|| view! {
                    <span class="tcn-chip tcn-chip-primary" style="flex:0 0 auto">
                        {category.clone()}
                    </span>
                })}
                {some_of_them.then(|| view! {
                    <span class="tcn-chip tcn-chip-amber tcw-for-chip" style="flex:0 0 auto"
                          title=for_title.clone()>
                        {for_chip.clone()}
                        <span class="tcw-caret" aria-hidden="true">
                            {move || if is_open.get() { "▴" } else { "▾" }}
                        </span>
                    </span>
                })}
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
            <Show when=move || is_open.get()>
                <div class="tcw-details">
                    <ExpenseDetails tour=tour_for_details.clone() spending=for_details.clone() />
                </div>
            </Show>
        </div>
    }
}

/// What an unfolded expense says: who paid and when on one line, then each share once with
/// the names that carry it, then who is not in it.
#[component]
fn ExpenseDetails(tour: Tour, spending: Spending) -> impl IntoView {
    let payer = name_of(tour.person(&spending.from));
    let mut meta: Vec<String> = vec![crate::explain::pretty_stamp(
        spending.when().unwrap_or_default(),
    )];
    if !spending.category.trim().is_empty() {
        meta.push(spending.category.trim().to_owned());
    }
    if tour.currencies.len() > 1 && spending.currency.id != tour.currency().id {
        meta.push(format!(
            "entered as {} {}",
            money(spending.amount),
            spending.currency.name
        ));
    }
    if let tc_core::Kind::Draft { counted } = spending.kind {
        meta.push(if counted { "draft, counted" } else { "draft, not counted" }.to_owned());
    }
    let (groups, left_out) = crate::explain::share_groups(&tour, &spending);
    let how = match &spending.split {
        Split::Everyone => "everyone, by weight".to_owned(),
        Split::Equally(_) => format!("{} of {}, equally", groups.iter().map(|g| g.names.len()).sum::<usize>(), tour.persons.len()),
        Split::ByWeight(_) => format!("{} of {}, by weight", groups.iter().map(|g| g.names.len()).sum::<usize>(), tour.persons.len()),
    };

    view! {
        <div class="tcw-det-meta">
            <b>{payer}</b>" paid · " {meta.join(" · ")}
        </div>
        <div class="tcw-det-how">{how}</div>
        <div class="tcw-shares">
            {groups
                .into_iter()
                .map(|g| {
                    let each = g.names.len() > 1;
                    view! {
                        <span class="tcw-share-amt">{money(g.share)}</span>
                        <span class="tcw-share-names">
                            {(each || g.weight.is_some()).then(|| view! {
                                <span class="tcw-share-tag">
                                    {each.then_some("each")}
                                    {(each && g.weight.is_some()).then_some(" · ")}
                                    {g.weight.map(|w| format!("w{w}"))}
                                </span>
                            })}
                            {g.names.join(", ")}
                        </span>
                    }
                })
                .collect_view()}
        </div>
        {(!left_out.is_empty()).then(|| view! {
            <div class="tcw-det-out">"Not in it: " {left_out.join(", ")}</div>
        })}
    }
}

/// Where the money went: by category or by person, with the per-person and per-day figures
/// the app shows.
///
/// No pie chart. The C# version draws one; it is the part of that screen that carries the
/// least and would cost the most here, so the numbers are drawn as bars instead - same
/// information, no library.
#[component]
fn StatsTab(tour: Tour, spendings: Vec<Spending>, unit: String, sifting: Sifting) -> impl IntoView {
    // What the reader has chosen on the ring - a category, a head of several, or a person -
    // and which of them we are inside, if any. Both live with the page rather than in the
    // chart: the totals below are the other half of the same question, and an edit made
    // from this screen must not answer by clearing it.
    let by_category = sifting.by_category;
    let chosen = sifting.ring;
    let drill = sifting.drill;
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

    let total_weight = match tour.persons.iter().map(|p| p.weight as i64).sum::<i64>() {
        0 => 100,
        w => w,
    };

    let by_cat = group_by(&counted, &tour, |s, _| s.category.trim().to_owned());
    let by_person = group_by(&counted, &tour, |s, t| name_of(t.person(&s.from)));

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

    // Memos rather than closures: they are read from a dozen places in the view, and a
    // closure read twice is a closure moved twice.
    // What the cards are about. A category, or all of them - and in the by-person view
    // always all of them: "per person" of what one person paid is not a number anybody
    // wants, and the title below says which it is rather than leaving it to be guessed.
    let filter = Memo::new(move |_| {
        if by_category.get() {
            chosen.get()
        } else {
            String::new()
        }
    });

    let for_total = counted.clone();
    let tour_for_total = tour.clone();
    let cat_total = Memo::new(move |_| {
        let chosen = filter.get();
        Cents(
            for_total
                .iter()
                .filter(|s| crate::chart::matches(&chosen, &s.category))
                .map(|s| tour_for_total.amount_in_current(s).0)
                .sum(),
        )
    });

    let for_weight = counted.clone();
    let tour_for_weight = tour.clone();
    let per_person = Memo::new(move |_| {
        let chosen = filter.get();
        // The weight the money was split across, averaged over the expenses it covers.
        // "Per person" for something only three people shared is not the figure it is for
        // something the whole tour did.
        let weights: Vec<i64> = for_weight
            .iter()
            .filter(|s| crate::chart::matches(&chosen, &s.category))
            .map(|s| match &s.split {
                Split::Everyone => tour_for_weight.persons.iter().map(|p| p.weight as i64).sum(),
                Split::Equally(to) | Split::ByWeight(to) => to
                    .iter()
                    .filter_map(|id| tour_for_weight.person(id))
                    .map(|p| p.weight as i64)
                    .sum(),
            })
            .collect();
        // With nothing chosen the answer is the tour's own weight, not the average of how
        // its expenses happened to be split - that is the app's rule, and taking the average
        // here put "per person" out by a tenth without anything on screen looking wrong.
        let weight = if chosen.is_empty() || weights.is_empty() {
            total_weight
        } else {
            (weights.iter().sum::<i64>() as f64 / weights.len() as f64).round() as i64
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

    // What the ring shows: the categories folded by head, or the people - and, once the
    // reader has gone into one, what is inside that one. Inside a category, rows are named
    // by their tail: the crumb above already says "Food".
    let for_rows = counted.clone();
    let tour_for_rows2 = tour.clone();
    let cats = by_cat.clone();
    let people = by_person.clone();
    let rows = move || {
        let inside = drill.get();
        match (by_category.get(), inside) {
            (true, None) => crate::chart::by_head(&cats),
            (true, Some(head)) => cats
                .iter()
                .filter(|(name, _)| crate::chart::head_of(name) == head)
                .map(|(name, amount)| {
                    crate::chart::Row::leaf(
                        name.clone(),
                        crate::chart::tail_of(name),
                        *amount,
                    )
                })
                .collect(),
            (false, None) => people
                .iter()
                .map(|(who, amount)| {
                    // What one person's money went on, carried with them: choosing them
                    // divides their arc into it, and the chevron gives it the whole circle.
                    let theirs: Vec<crate::chart::Row> =
                        categories_of(&for_rows, &tour_for_rows2, who)
                            .into_iter()
                            .map(|(name, amount)| {
                                crate::chart::Row::leaf(name.clone(), name, amount)
                            })
                            .collect();
                    crate::chart::Row {
                        key: who.clone(),
                        label: who.clone(),
                        amount: *amount,
                        inside: if theirs.len() > 1 { theirs } else { Vec::new() },
                    }
                })
                .collect(),
            (false, Some(who)) => categories_of(&for_rows, &tour_for_rows2, &who)
                .into_iter()
                // Their categories as they are, not folded by head: what one person's money
                // went on is the question, and folding "Food / lunch" and "Food / dinner"
                // back into "Food" is how it stops being answered. One level, too - the
                // crumb says whose money this is, and a second would need a second crumb to
                // climb back out of.
                .map(|(name, amount)| crate::chart::Row::leaf(name.clone(), name, amount))
                .collect(),
        }
    };
    let leave = Callback::new(move |()| {
        drill.set(None);
        chosen.set(String::new());
    });
    let enter = Callback::new(move |key: String| {
        drill.set(Some(key));
        chosen.set(String::new());
    });
    let nothing = counted.is_empty();
    let unit_for_pie = unit.clone();
    let unit_for_cards = unit.clone();
    let rates_for_table = rates.clone();

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                // Changing what the ring is about starts from the top of it. Otherwise
                // switching while inside "Food" asked the other view for the inside of a
                // person by that name, and drew an empty circle.
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || by_category.get()
                      on:click=move |_| {
                          by_category.set(true);
                          drill.set(None);
                          chosen.set(String::new());
                      }>"By category"</span>
                <span class="tcn-chip tcn-filter-chip" class:is-on=move || !by_category.get()
                      on:click=move |_| {
                          by_category.set(false);
                          drill.set(None);
                          chosen.set(String::new());
                      }>"By person"</span>
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
                        <crate::chart::Composition rows=rows() unit=unit_for_pie.clone()
                                                   chosen=chosen crumb=drill.get()
                                                   into=enter out=leave />
                    }}

                    <div class="tcn-section-title" style="margin-top:14px">
                        "Totals · "
                        {move || {
                            let what = filter.get();
                            if what.is_empty() {
                                "everything".to_owned()
                            } else {
                                what
                            }
                        }}
                    </div>

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

thread_local! {
    /// Which tab the reader was on, and which tour it was.
    ///
    /// Above the page rather than in it, because switching between the roomy and the small
    /// interface builds the page again from nothing - and did it by putting the reader back
    /// on Expenses. Somebody who opens Balance and switches interface to read it wants
    /// Balance in the other one, not the expense list.
    ///
    /// Only for as long as the app is open: a reload starts the tour where it opens.
    static LAST_TAB: std::cell::RefCell<Option<(String, Tab)>> =
        const { std::cell::RefCell::new(None) };
}

/// The tab this tour was left on, if it is still the tour that was left.
fn tab_left_on(tour: &str) -> Option<Tab> {
    LAST_TAB.with(|last| {
        last.borrow()
            .as_ref()
            .filter(|(was, _)| was == tour)
            .map(|(_, tab)| *tab)
    })
}

fn leaving_tab(tour: &str, tab: Tab) {
    LAST_TAB.with(|last| *last.borrow_mut() = Some((tour.to_owned(), tab)));
}

/// Which tab an address asks for.
pub fn tab_of(landing: crate::Landing) -> Tab {
    match landing {
        crate::Landing::People => Tab::People,
        crate::Landing::Expenses | crate::Landing::AddSpending => Tab::Expenses,
        crate::Landing::Stats => Tab::Stats,
        crate::Landing::Balance => Tab::Balance,
        // Until the tour is here to say. It never shows: the tab is settled the moment
        // there is a tour to settle it from, cached copy included.
        crate::Landing::Unsaid => Tab::Expenses,
    }
}

/// Which tab a tour opens on when the address did not say.
///
/// A tour being settled up opens on the payments, because that is the whole of what anybody
/// is doing with it; any other one opens where expenses are added, because that is the whole
/// of what anybody is doing with *it*.
///
/// Archived does not come into it. The app asks
/// `(RawTour.IsFinalizing && !RawTour.IsArchived)` (`TourPageNew.razor:339`), so an archived
/// tour still being settled opened on the expenses - while the list beside it showed the
/// amber "settling up" chip, which ignores archiving. Two screens, two answers about one
/// tour, and the first person to meet it read the page as broken.
///
/// Here archiving means one thing only: **the tour is out of the default list**. Inside a
/// tour it decides nothing; the badge in the header says it is archived and that is all.
/// The second deliberate departure from the app, after the settlement threshold.
pub fn opens_on(tour: &Tour) -> Tab {
    if tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING) {
        Tab::Balance
    } else {
        Tab::Expenses
    }
}

/// What one person's money went on, as categories and amounts.
fn categories_of(counted: &[Spending], tour: &Tour, who: &str) -> Vec<(String, Cents)> {
    let theirs: Vec<Spending> = counted
        .iter()
        .filter(|s| name_of(tour.person(&s.from)) == who)
        .cloned()
        .collect();
    group_by(&theirs, tour, |s, _| s.category.trim().to_owned())
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
    let threshold = crate::settings::threshold(&tour);
    let rows = settlement_summary(&tour, &all_for_summary, threshold);
    // What the settlement worked out and the screen does not show: payments smaller than
    // the threshold. They are why a tour can say "everyone is settled up" and still have
    // somebody sitting on a balance - the balance adds them up, the list leaves them out,
    // and side by side that reads as a contradiction. So they are named.
    let dust: Vec<Transfer> = {
        let (_, between_all) = tc_core::split_family(&all_for_summary);
        between_all
            .into_iter()
            .filter(|t| {
                let shown = tour.convert(t.amount, &t.currency);
                shown.0 != 0 && shown.abs() <= threshold
            })
            .cloned()
            .collect()
    };
    let dust_total: Cents = dust
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency).abs())
        .sum();
    let dust_rows = dust.clone();
    let dust_open = RwSignal::new(false);
    // Its own copies, because the block that draws them outlives the one that names them.
    let tour_for_dust = tour.clone();
    let names_for_dust = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };
    let unit_for_dust = unit.clone();
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
                                {if !has_real {
                                    "Add the first expense and the split will show up here.".to_owned()
                                } else if dust.is_empty() {
                                    "No payments are left between the participants.".to_owned()
                                } else {
                                    // The balances below are not zero, and this is why -
                                    // as long as there are any. A reader who has raised the
                                    // threshold high enough has none, and then there is
                                    // nothing to point at.
                                    format!(
                                        "What is left is too small to chase: {} under {} each, {} in all.{}",
                                        match dust.len() {
                                            1 => "one payment".to_owned(),
                                            n => format!("{n} payments"),
                                        },
                                        money(threshold),
                                        money(dust_total),
                                        if rows.is_empty() {
                                            ""
                                        } else {
                                            " That is what the balances below add up to."
                                        },
                                    )
                                }}
                            </div>
                            <Show when=move || !dust.is_empty()>
                                <button type="button" class="tcn-btn tcn-btn-sm"
                                        style="margin-top:10px"
                                        on:click=move |_| dust_open.update(|o| *o = !*o)>
                                    {move || if dust_open.get() {
                                        "Hide the small ones"
                                    } else {
                                        "Show the small ones"
                                    }}
                                </button>
                            </Show>
                        </div>
                        <Show when=move || dust_open.get()>
                            <div class="tcn-list" style="margin-top:12px">
                                {transfer_rows(&tour_for_dust, &dust_rows, &names_for_dust,
                                               &unit_for_dust, Some(apply))}
                            </div>
                        </Show>
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
pub fn Avatar(name: String) -> impl IntoView {
    view! {
        <span class="tcn-avatar tcn-avatar-sm" style=format!("background:{}", avatar_colour(&name))>
            {initials(&name)}
        </span>
    }
}

#[cfg(test)]
mod what_went_wrong {
    use super::said_of;
    use crate::api::Trouble;

    /// Each kind says something a reader can act on, and no two say the same thing.
    #[test]
    fn every_kind_of_trouble_says_its_own_thing() {
        let said = [
            said_of(Trouble::Unreachable),
            said_of(Trouble::Answered(404)),
            said_of(Trouble::Answered(409)),
            said_of(Trouble::Answered(500)),
            said_of(Trouble::Answered(418)),
            said_of(Trouble::Ours),
        ];
        let mut sorted = said.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), said.len(), "no two read alike: {said:?}");

        assert!(said[0].contains("did not answer"));
        assert!(said[1].contains("not on the server"));
        assert!(said[2].contains("409"));
        assert!(said[3].contains("in trouble"));
        assert!(said[4].contains("refused"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour() -> Tour {
        Tour::from_json(include_str!("../../../fixtures/zscph2y.tour.json")).expect("fixture")
    }

    fn flagged(archived: bool, settling: bool) -> Tour {
        let mut t = tour();
        tc_core::extras::set(&mut t.extras, tc_core::extras::ARCHIVED, archived.into());
        tc_core::extras::set(&mut t.extras, tc_core::extras::FINALIZING, settling.into());
        t
    }

    /// Archiving is about the list of tours and nothing else. A tour being settled up opens
    /// on the payments whether or not it has been put away - the app checks both flags here
    /// and lands an archived one on the expenses, while its own list still calls the tour
    /// "settling up".
    #[test]
    fn archiving_does_not_decide_which_tab_a_tour_opens_on() {
        assert_eq!(opens_on(&flagged(false, true)), Tab::Balance);
        assert_eq!(opens_on(&flagged(true, true)), Tab::Balance, "archived as well");
        assert_eq!(opens_on(&flagged(true, false)), Tab::Expenses);
        assert_eq!(opens_on(&flagged(false, false)), Tab::Expenses);
    }

    /// The app's chip row reads "бухать/вино … Гнездо/ещё … Треш и угар". A byte-order sort
    /// puts every capital before every small letter and answers "Гнездо, Треш, бухать",
    /// which is the list the reader saw side by side with the app's and asked about.
    #[test]
    fn the_categories_are_in_the_order_the_app_offers_them() {
        let cats = categories_in_order(&tour().spendings);
        let folded: Vec<String> = cats.iter().map(|c| c.to_lowercase()).collect();
        let mut wanted = folded.clone();
        wanted.sort();
        assert_eq!(folded, wanted, "ignoring case, the chips are in order: {cats:?}");
        assert!(
            cats.iter().all(|c| !c.trim().is_empty()),
            "an expense with no category is not a chip"
        );
    }

    /// Expenses entered through the new form all carry the same midnight stamp, so the
    /// sort decides nothing between them and what is left is the order the tour stores
    /// them in. The app keeps it - its `OrderByDescending` is stable - and so must this, or
    /// the same day reads one way here and the other way there.
    #[test]
    fn expenses_sharing_a_stamp_stay_in_the_order_the_tour_keeps_them() {
        let t = tour();
        // What the form writes: a day and no time, three in a row on the same one.
        let mut rows: Vec<Spending> = t.spendings.iter().take(3).cloned().collect();
        for s in rows.iter_mut() {
            s.extras.0.insert(
                crate::edit::SPENDING_DATE.to_owned(),
                serde_json::Value::String("2026-09-11".to_owned()),
            );
        }
        let stored: Vec<String> = rows.iter().map(|s| s.id.as_str().to_owned()).collect();

        for newest_first in [true, false] {
            let mut mine = rows.clone();
            order_rows(&mut mine, false, newest_first, &t);
            let shown: Vec<String> = mine.iter().map(|s| s.id.as_str().to_owned()).collect();
            assert_eq!(shown, stored, "newest_first = {newest_first}");
        }
    }

    /// And the days themselves do turn round.
    #[test]
    fn which_way_up_the_list_is_is_still_the_readers_choice() {
        let t = tour();
        let mut newest = t.spendings.clone();
        order_rows(&mut newest, false, true, &t);
        let mut oldest = t.spendings.clone();
        order_rows(&mut oldest, false, false, &t);
        assert!(
            newest.first().unwrap().when() >= oldest.first().unwrap().when(),
            "the two orders do not start on the same end"
        );
        assert_ne!(
            newest.first().unwrap().when(),
            newest.last().unwrap().when(),
            "the fixture spans more than one day, or this test proves nothing"
        );
    }
}
