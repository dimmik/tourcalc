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

/// What the tour list is ordered by.
///
/// The server answers newest first, which is the order this starts on and the one the app
/// has always had. The rest are for a list long enough that "where is it" is a real
/// question: the tour somebody touched this morning, a name they half remember, the trip
/// that cost the most.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// The day the tour was made.
    Made,
    /// The last time anybody saved it - which the stored state id begins with.
    Changed,
    Name,
    /// Everything spent on it, as the row shows.
    Spent,
}

impl Order {
    pub fn label(self) -> &'static str {
        match self {
            Order::Made => "New",
            Order::Changed => "Touched",
            Order::Name => "Name",
            Order::Spent => "Spent",
        }
    }

    /// What each way round means for this key, said in the button's title.
    pub fn ways(self) -> (&'static str, &'static str) {
        match self {
            Order::Made => ("newest first", "oldest first"),
            Order::Changed => ("changed most recently first", "longest untouched first"),
            Order::Name => ("Z to A", "A to Z"),
            Order::Spent => ("biggest first", "smallest first"),
        }
    }

    fn stored(self) -> &'static str {
        match self {
            Order::Made => "made",
            Order::Changed => "changed",
            Order::Name => "name",
            Order::Spent => "spent",
        }
    }

    fn of(text: &str) -> Option<Order> {
        match text {
            "made" => Some(Order::Made),
            "changed" => Some(Order::Changed),
            "name" => Some(Order::Name),
            "spent" => Some(Order::Spent),
            _ => None,
        }
    }

    pub const ALL: [Order; 4] = [Order::Made, Order::Changed, Order::Name, Order::Spent];
}

/// Presses a key: the one already on turns round, another takes over pointing the way its
/// first meaning does. Remembered for this device either way.
pub fn choose(by: RwSignal<Order>, downwards: RwSignal<bool>, which: Order) {
    if by.get_untracked() == which {
        downwards.update(|d| *d = !*d);
    } else {
        by.set(which);
        downwards.set(true);
    }
    crate::settings::remember_tour_order(by.get_untracked().stored(), downwards.get_untracked());
}

/// Everything spent on a tour, from what the server worked out for the list.
fn spent_on(tour: &Tour) -> i64 {
    tour.persons
        .iter()
        .filter_map(|p| p.extras.0.get("SpentInCents").and_then(|v| v.as_i64()))
        .sum()
}

/// When a tour was last saved. The state id is written "2026-09-20 10:00:00 .a1b2…", so the
/// front of it is a time, and comparing two of them as text compares two times.
fn touched_at(tour: &Tour) -> String {
    tc_core::extras::str_of(&tour.extras, tc_core::extras::STATE)
}

fn made_at(tour: &Tour) -> String {
    tc_core::extras::str_of(&tour.extras, tc_core::extras::CREATED_AT)
}

/// Puts the list in order. `downwards` is the first way round in [`Order::ways`].
pub fn order_tours(tours: &mut [Tour], by: Order, downwards: bool) {
    // The id last, so that two tours made the same day do not swap places between draws.
    match by {
        Order::Made => tours.sort_by(|a, b| {
            (made_at(a), a.id.as_str()).cmp(&(made_at(b), b.id.as_str()))
        }),
        Order::Changed => tours.sort_by(|a, b| {
            (touched_at(a), a.id.as_str()).cmp(&(touched_at(b), b.id.as_str()))
        }),
        Order::Name => tours.sort_by(|a, b| {
            (a.name.to_lowercase(), a.id.as_str()).cmp(&(b.name.to_lowercase(), b.id.as_str()))
        }),
        Order::Spent => tours.sort_by(|a, b| {
            (spent_on(a), a.id.as_str()).cmp(&(spent_on(b), b.id.as_str()))
        }),
    }
    if downwards {
        tours.reverse();
    }
}

/// The buttons that choose it, in both interfaces: one per key, and pressing the one that
/// is already on turns it round - the same gesture the expense list has.
#[component]
pub fn OrderPicker(by: RwSignal<Order>, downwards: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="tcw-order">
            {Order::ALL
                .into_iter()
                .map(|which| {
                    let (down, up) = which.ways();
                    view! {
                        <button type="button" class="tcn-btn tcn-btn-sm"
                                class:tcn-btn-primary=move || by.get() == which
                                title=move || {
                                    if by.get() == which && downwards.get() {
                                        format!("{down} — click for {up}")
                                    } else if by.get() == which {
                                        format!("{up} — click for {down}")
                                    } else {
                                        format!("Order by: {down}")
                                    }
                                }
                                on:click=move |_| choose(by, downwards, which)>
                            {which.label()}
                            <Show when=move || by.get() == which>
                                {move || if downwards.get() { " ↓" } else { " ↑" }}
                            </Show>
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
pub fn TourListPage() -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);
    let search = RwSignal::new(String::new());
    let adding = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_code = RwSignal::new(String::new());
    let new_json = RwSignal::new(String::new());
    let show_archived = RwSignal::new(false);
    let trouble = RwSignal::new(String::new());
    let busy = RwSignal::new(false);

    // The list this device saw last, drawn before the server is asked. Coming back from a
    // tour is the commonest move in the app, and it should not be a blank screen for the
    // length of a request to show the same seven names again.
    if let Some(known) = crate::queue::cached_list() {
        set_state.set(Load::Ready(known));
    }

    let load = Callback::new(move |_: ()| {
        spawn_local(async move {
            match api::tours().await {
                Ok(ts) => {
                    crate::queue::cache_list(&ts);
                    set_state.set(Load::Ready(ts));
                }
                // A failure with something already on screen keeps it: an old list is more
                // use than an error page, and the tours in it still open.
                Err(e) => {
                    if !matches!(state.get_untracked(), Load::Ready(_)) {
                        set_state.set(Load::Failed(e.to_string()));
                    }
                }
            }
        });
    });
    load.run(());

    // Which of these tours ring on this device. Drawn from last time at once, then asked -
    // once for the whole list, and not at all by a browser that never subscribed.
    let bells = crate::push::list_bells();

    let mode = use_context::<RwSignal<crate::mode::UiMode>>()
        .unwrap_or_else(|| RwSignal::new(crate::mode::UiMode::Full));

    let create = Callback::new(move |()| {
        let name = new_name.get().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let code = new_code.get().trim().to_owned();
        let pasted = new_json.get().trim().to_owned();
        trouble.set(String::new());

        // A pasted tour starts the new one off: its people, its expenses, its currencies.
        // Anything that says *which* tour it was - the id, the access code, the state - is
        // dropped, because this is a new tour and not that one.
        let body = if pasted.is_empty() {
            serde_json::json!({ "Name": name, "Persons": [], "Spendings": [] })
        } else {
            match serde_json::from_str::<serde_json::Value>(&pasted) {
                Ok(mut v) => {
                    if let Some(obj) = v.as_object_mut() {
                        for key in [
                            "Id",
                            "GUID",
                            "AccessCodeMD5",
                            "StateGUID",
                            "IsVersion",
                            "VersionFor_Id",
                            "VersionComment",
                            "DateVersioned",
                        ] {
                            obj.remove(key);
                        }
                        obj.insert("Name".into(), name.clone().into());
                    }
                    v
                }
                Err(e) => {
                    trouble.set(format!("That does not look like a tour: {e}"));
                    return;
                }
            }
        };

        busy.set(true);
        spawn_local(async move {
            match api::add_tour(body, api::Pile::Typed(&code)).await {
                Ok(_id) => {
                    adding.set(false);
                    new_name.set(String::new());
                    new_code.set(String::new());
                    new_json.set(String::new());
                    load.run(());
                }
                Err(e) => trouble.set(e.to_string()),
            }
            busy.set(false);
        });
    });

    // Clones a tour, with or without what was spent on it. The second is what a group
    // going on the same trip again wants: the same people, the same weights, the same
    // currencies, and none of last year's taxis.
    let clone = Callback::new(move |(tour, strip): (Tour, bool)| {
        trouble.set(String::new());
        busy.set(true);
        spawn_local(async move {
            // The list carries no spendings - the server strips them - so the tour is
            // fetched whole before being copied.
            let whole = match api::tour(tour.id.as_str()).await {
                Ok(t) => t,
                Err(e) => {
                    trouble.set(e.to_string());
                    busy.set(false);
                    return;
                }
            };
            let Some(mut body) = whole
                .to_json()
                .ok()
                .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
            else {
                trouble.set("could not read that tour".into());
                busy.set(false);
                return;
            };
            if let Some(obj) = body.as_object_mut() {
                obj.insert("Name".into(), format!("clone of {}", tour.name).into());
                for key in ["Id", "GUID", "StateGUID"] {
                    obj.remove(key);
                }
                if strip {
                    obj.insert("Spendings".into(), serde_json::json!([]));
                }
            }
            // Next to the original: an ordinary reader's copy goes under their own code
            // whatever is asked, and an administrator's under the original's.
            let pile = tc_core::extras::str_of(&tour.extras, tc_core::extras::ACCESS_CODE);
            match api::add_tour(body, api::Pile::Hashed(&pile)).await {
                Ok(_) => load.run(()),
                Err(e) => trouble.set(e.to_string()),
            }
            busy.set(false);
        });
    });

    // The tour as JSON, on the clipboard - what the box above pastes back.
    let copy_json = Callback::new(move |tour: Tour| {
        trouble.set(String::new());
        spawn_local(async move {
            let whole = match api::tour(tour.id.as_str()).await {
                Ok(t) => t,
                Err(e) => {
                    trouble.set(e.to_string());
                    return;
                }
            };
            // Without the suggested payments: they are worked out from the rest, and a copy
            // that carried them would look like a tour where somebody had already paid.
            let mut copy = whole;
            copy.spendings.retain(|s| s.kind != tc_core::Kind::Planned);
            let Ok(text) = copy.to_json() else {
                trouble.set("could not write that tour out".into());
                return;
            };
            match copy_to_clipboard(&text).await {
                Ok(()) => trouble.set("Copied — the JSON is on the clipboard.".into()),
                Err(e) => trouble.set(e.to_string()),
            }
        });
    });

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
                Err(e) => trouble.set(e.to_string()),
            }
        });
    });

    // How the list is ordered, remembered on this device. The server already answers newest
    // first, so that is where this starts and what it means for nobody to have chosen.
    let (stored_by, stored_way) = crate::settings::tour_order()
        .and_then(|(by, way)| Order::of(&by).map(|by| (by, way)))
        .unwrap_or((Order::Made, true));
    let order_by = RwSignal::new(stored_by);
    let downwards = RwSignal::new(stored_way);

    // The small interface carries its own bar and its own create form, so the roomy ones
    // are not drawn at all - which is a thing a screenshot catches and a driver does not:
    // the rows were right and the page had two of everything above them.
    let full = move || mode.get() == crate::mode::UiMode::Full;

    view! {
        <Show when=full>
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
                <label class="tcn-switchline" style="margin:0 0 0 10px">
                    <input type="checkbox" prop:checked=move || show_archived.get()
                           on:change=move |ev| show_archived.set(event_target_checked(&ev)) />
                    "Show archived"
                </label>
                <OrderPicker by=order_by downwards=downwards />
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
                        <div class="tcn-field-label">"Tour JSON (optional)"</div>
                        <textarea class="tcn-input" rows="3"
                                  placeholder="paste an exported tour to start from it"
                                  prop:value=move || new_json.get()
                                  on:input=move |ev| new_json.set(event_target_value(&ev))></textarea>
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
                    <button type="button" class="tcn-btn tcn-btn-primary"
                            prop:disabled=move || busy.get() on:click=move |_| create.run(())>
                        "Create tour"
                    </button>
                </div>
            </Show>

        </div>
        </Show>

        // Whatever went wrong is worth saying in either interface.
        <Show when=move || !trouble.get().is_empty()>
            <div class="tcn-section" style="padding-bottom:0">
                <div class="tcn-errors">{move || trouble.get()}</div>
            </div>
        </Show>

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
            Load::Ready(mut tours) if mode.get() == crate::mode::UiMode::Mini => {
                order_tours(&mut tours, order_by.get(), downwards.get());
                view! {
                <crate::mini::MiniList tours=tours search=search show_archived=show_archived
                                       order_by=order_by downwards=downwards
                                       adding=adding new_name=new_name new_code=new_code
                                       new_json=new_json busy=busy
                                       create=create bells=bells
                                       remove=remove clone_it=clone copy_json=copy_json />
                }.into_any()
            }
            Load::Ready(mut tours) => {
                order_tours(&mut tours, order_by.get(), downwards.get());
                let needle = search.get().trim().to_lowercase();
                let shown: Vec<Tour> = tours
                    .into_iter()
                    // Archived tours are out of the way until asked for - that is what
                    // archiving is.
                    .filter(|t| {
                        // Archiving is about the default list, not about the search: asking
                        // for a name by typing it is asking for that tour, and answering
                        // "nothing matches" because it was archived two years ago is a lie
                        // the reader has no way to see through. Each row says "archived"
                        // for itself.
                        show_archived.get()
                            || !needle.is_empty()
                            || !tc_core::extras::bool_of(&t.extras, tc_core::extras::ARCHIVED)
                    })
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
                                    .map(|t| view! {
                                        <Row tour=t.clone() remove=remove clone_it=clone
                                             copy_json=copy_json bells=bells />
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    }.into_any()
                }
            }
        }}

        // Edits waiting for a tour that is not in this list - deleted by somebody, or not
        // opened by this code - have no row to hang "not sent yet" on. Without this line they
        // were on the device and nowhere on screen.
        {move || {
            crate::queue::changes();
            let Load::Ready(tours) = state.get() else {
                return ().into_any();
            };
            let orphans: Vec<(String, String)> = crate::queue::tours_with_pending()
                .into_iter()
                .filter(|id| !tours.iter().any(|t| t.id.as_str() == id))
                .map(|id| {
                    let name = crate::queue::cached(&id)
                        .map(|t| t.name)
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or_else(|| id.clone());
                    (id, name)
                })
                .collect();
            if orphans.is_empty() {
                return ().into_any();
            }
            let title = match orphans.len() {
                1 => "Edits waiting for 1 tour that is not in your list:".to_owned(),
                n => format!("Edits waiting for {n} tours that are not in your list:"),
            };
            view! {
                <div class="tcn-section">
                    <div class="tcn-chip tcn-chip-amber tcw-wraps">
                        {title}
                        {orphans
                            .into_iter()
                            .map(|(id, name)| view! {
                                " " <a href=format!("/tour/{id}")>{name}</a>
                            })
                            .collect_view()}
                        " — open it to send them again or discard them."
                    </div>
                </div>
            }.into_any()
        }}
    }
}

/// What the list says about a tour's settlement, if anything.
///
/// The chip is about the tour's *state*, not its arithmetic. A tour nobody has put into
/// settle-up mode says nothing at all, however its balances happen to stand: everyone may be
/// square today because each of them paid a hundred, and tomorrow somebody buys dinner. The
/// question a list is scanned for is "is this trip finished?", and only a tour being settled
/// up is answering it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Settle {
    /// Being settled up, and there is nothing left to hand over.
    Square,
    /// Being settled up, with this much still to go - `None` from a server too old to
    /// work it out, which leaves the chip saying only that the tour is being settled.
    Owing(Option<i64>),
}

pub fn settle_state(settling: bool, left: Option<i64>) -> Option<Settle> {
    if !settling {
        return None;
    }
    Some(match left {
        Some(0) => Settle::Square,
        Some(left) if left > 0 => Settle::Owing(Some(left)),
        // A negative figure would mean the settlement owes somebody money, which it cannot;
        // treat it the way an unknown one is treated rather than printing nonsense.
        _ => Settle::Owing(None),
    })
}

#[component]
fn Row(
    tour: Tour,
    remove: Callback<Tour>,
    clone_it: Callback<(Tour, bool)>,
    copy_json: Callback<Tour>,
    bells: crate::push::Bells,
) -> impl IntoView {
    let href = format!("/tour/{}", tour.id);
    let people = tour.persons.len();

    // The list carries what the server already worked out, in `SpentInCents` on each
    // person. It is not modelled by tc-core - the arithmetic recomputes it rather than
    // trusting it - so it is read out of the fields that came along for the ride.
    // What the tour cost, as the server worked it out - the same figure the tour's own
    // screen shows. An older server does not send it; then this falls back to what it
    // always did, which adds the paybacks in and reads a little high.
    let spent: i64 = tour
        .extras
        .0
        .get("TotalSpentInCents")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| {
            tour.persons
                .iter()
                .filter_map(|p| p.extras.0.get("SpentInCents").and_then(|v| v.as_i64()))
                .sum()
        });

    // What is still owed between the participants, as the server worked it out. `None` from
    // a server too old to send it - and then the row says nothing rather than guessing, since
    // the list is answered without the spendings and there is nothing here to add up.
    let left_to_settle: Option<i64> = tour
        .extras
        .0
        .get("LeftToSettleInCents")
        .and_then(|v| v.as_i64());

    let currency = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let for_delete = tour.clone();
    let for_clone = tour.clone();
    let for_clone_bare = tour.clone();
    let for_json = tour.clone();
    // Edits made on this device that have not reached the server - the tour page says so
    // while you are in it, and the list said nothing at all, so a tour with a day's worth
    // of expenses waiting looked like any other. Read through the counter the sender bumps,
    // so the chip goes when the queue does rather than at the next redraw.
    let waiting = {
        let id = tour.id.as_str().to_owned();
        move || {
            crate::queue::changes();
            crate::queue::pending(&id).len()
        }
    };
    let archived = tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED);
    // Which tours are being settled up is the thing you look for in a list of them: it says
    // which one is asking for something to be done. The app marks it here, the small
    // interface marks it, and this list did not.
    let settling = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);
    let state = settle_state(settling, left_to_settle);

    view! {
        <div class="tcn-tour" class:is-archived=move || archived>
            <a class="tcn-tour-name" href=href>
                {tour.name.clone()}
                <crate::push::ListBell bells=bells tour=tour.id.as_str().to_owned() />
            </a>
            <div class="tcn-tour-meta">
                <span>{people} " people"</span>
                <span>"·"</span>
                {match state {
                    Some(Settle::Owing(_)) => view! {
                        <span class="tcn-chip tcn-chip-amber"
                              title="Everyone can see what to pay whom">
                            "settling up"
                        </span>
                    }.into_any(),
                    Some(Settle::Square) => view! {
                        <span class="tcn-chip tcn-chip-green"
                              title="Everybody has paid: nothing is left to hand over">
                            "all square"
                        </span>
                    }.into_any(),
                    None => ().into_any(),
                }}
                {archived.then(|| view! {
                    <span class="tcn-chip" title="Hidden from the default list">"archived"</span>
                })}
                <Show when={
                    let waiting = waiting.clone();
                    move || waiting() > 0
                }>
                    <span class="tcn-chip tcn-chip-amber"
                          title="Saved on this device and not yet sent to the server">
                        {let waiting = waiting.clone();
                         let id = tour.id.as_str().to_owned();
                         // Refused by the server and no longer retried: open the tour to
                         // see why and decide.
                         move || match (waiting(), crate::queue::given_up(&id)) {
                            (1, true) => "1 not accepted".to_owned(),
                            (n, true) => format!("{n} not accepted"),
                            (1, false) => "1 not sent yet".to_owned(),
                            (n, false) => format!("{n} not sent yet"),
                        }}
                    </span>
                </Show>
                <span title="Everything spent on this tour">
                    {money(Cents(spent))}
                    {(!currency.is_empty()).then(|| view! { "\u{a0}" {currency} })}
                </span>
                // How much of it is still owed, beside the chip that says so.
                {match state {
                    Some(Settle::Owing(Some(left))) => view! {
                        // The dot earns its keep here: two money figures side by side, and
                        // without it they read as one number in two halves.
                        <span>"·"</span>
                        <span title="Still to be handed over between the participants">
                            {money(Cents(left))} " to settle"
                        </span>
                    }.into_any(),
                    _ => ().into_any(),
                }}
            </div>
            <div class="tcn-tour-actions">
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click={
                            let t = for_clone.clone();
                            move |_| clone_it.run((t.clone(), false))
                        }>
                    "Clone"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        title="The same people and currencies, none of the expenses"
                        on:click={
                            let t = for_clone_bare.clone();
                            move |_| clone_it.run((t.clone(), true))
                        }>
                    "Clone w/o expenses"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click={
                            let t = for_json.clone();
                            move |_| copy_json.run(t.clone())
                        }>
                    "Copy JSON"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                        on:click=move |_| remove.run(for_delete.clone())>
                    "Delete"
                </button>
            </div>
        </div>
    }
}

/// Puts text on the clipboard.
///
/// Through the async clipboard when the browser allows it - which needs a secure origin and
/// a gesture, both true here - and otherwise by the old trick of selecting a hidden textarea,
/// which works everywhere and is why this is not three lines.
async fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let promise = clipboard.write_text(text);
        if wasm_bindgen_futures::JsFuture::from(promise).await.is_ok() {
            return Ok(());
        }
    }

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let area = document
        .create_element("textarea")
        .map_err(|_| "could not make a textarea".to_owned())?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .map_err(|_| "not a textarea".to_owned())?;
    area.set_value(text);
    let body = document.body().ok_or("no body")?;
    body.append_child(&area)
        .map_err(|_| "could not add it".to_owned())?;
    area.select();
    let copied = document
        .dyn_into::<web_sys::HtmlDocument>()
        .ok()
        .and_then(|d| d.exec_command("copy").ok())
        .unwrap_or(false);
    let _ = body.remove_child(&area);

    if copied {
        Ok(())
    } else {
        Err("This browser would not let the page copy it.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chip says what state the tour is in, not what its balances happen to be.
    #[test]
    fn a_tour_nobody_is_settling_up_says_nothing() {
        // Even when it is square: everyone may have paid a hundred each, and tomorrow
        // somebody buys dinner.
        assert_eq!(settle_state(false, Some(0)), None);
        assert_eq!(settle_state(false, Some(15_628)), None);
        assert_eq!(settle_state(false, None), None);
    }

    #[test]
    fn a_tour_being_settled_says_how_much_is_left_or_that_there_is_none() {
        assert_eq!(settle_state(true, Some(0)), Some(Settle::Square));
        assert_eq!(settle_state(true, Some(138)), Some(Settle::Owing(Some(138))));
    }

    /// An older server does not work the figure out; the chip still says the tour is being
    /// settled, which is what it said before the figure existed.
    #[test]
    fn without_a_figure_the_chip_still_says_it_is_being_settled() {
        assert_eq!(settle_state(true, None), Some(Settle::Owing(None)));
        // The settlement cannot owe anybody money; a negative reads as "not known".
        assert_eq!(settle_state(true, Some(-5)), Some(Settle::Owing(None)));
    }

    fn tour(id: &str, name: &str, made: &str, touched: &str, spent: i64) -> Tour {
        let mut t = Tour::from_json(include_str!("../../../fixtures/zscph2y.tour.json"))
            .expect("fixture");
        t.id = tc_core::TourId::new(id);
        t.name = name.to_owned();
        tc_core::extras::set(&mut t.extras, tc_core::extras::CREATED_AT, made.into());
        tc_core::extras::set(&mut t.extras, tc_core::extras::STATE, touched.into());
        // What the list is told each person spent; the row adds them up.
        for (i, p) in t.persons.iter_mut().enumerate() {
            let theirs = if i == 0 { spent } else { 0 };
            p.extras.0.insert("SpentInCents".into(), theirs.into());
        }
        t
    }

    fn some() -> Vec<Tour> {
        vec![
            tour("a", "Альпы", "2021-06-01", "2026-09-20 10:00:00 .aa", 300),
            tour("b", "Байкал", "2026-01-01", "2021-07-01 10:00:00 .bb", 100),
            tour("c", "carpathians", "2024-03-03", "2024-03-03 10:00:00 .cc", 200),
        ]
    }

    fn ids(tours: &[Tour]) -> Vec<&str> {
        tours.iter().map(|t| t.id.as_str()).collect()
    }

    #[test]
    fn newest_first_is_what_the_list_opens_on() {
        let mut tours = some();
        order_tours(&mut tours, Order::Made, true);
        assert_eq!(ids(&tours), ["b", "c", "a"]);
        order_tours(&mut tours, Order::Made, false);
        assert_eq!(ids(&tours), ["a", "c", "b"], "and the other way round");
    }

    #[test]
    fn the_other_keys_order_by_what_they_say() {
        let mut tours = some();
        order_tours(&mut tours, Order::Changed, true);
        assert_eq!(ids(&tours), ["a", "c", "b"], "saved most recently first");

        order_tours(&mut tours, Order::Spent, true);
        assert_eq!(ids(&tours), ["a", "c", "b"], "biggest first");

        order_tours(&mut tours, Order::Name, false);
        // Plain Unicode order, so Latin comes before Cyrillic and case does not decide:
        // "carpathians", then "Альпы", then "Байкал".
        assert_eq!(ids(&tours), ["c", "a", "b"]);
    }

    /// Two tours made the same day do not swap places between one draw and the next.
    #[test]
    fn a_tie_is_broken_the_same_way_every_time() {
        let mut tours = vec![
            tour("y", "One", "2026-01-01", "x", 0),
            tour("x", "Two", "2026-01-01", "x", 0),
        ];
        order_tours(&mut tours, Order::Made, true);
        let first = ids(&tours).join(",");
        order_tours(&mut tours, Order::Made, true);
        assert_eq!(ids(&tours).join(","), first);
    }

    /// What is remembered is read back as the same choice.
    #[test]
    fn a_stored_key_reads_back() {
        for which in Order::ALL {
            assert_eq!(Order::of(which.stored()), Some(which));
        }
        assert_eq!(Order::of("whatever"), None);
    }
}
