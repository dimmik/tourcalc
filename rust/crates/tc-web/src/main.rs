//! Tourcalc's client, in Rust.
//!
//! The same screens the Blazor client shows, on its stylesheet, and working without a
//! network: edits are queued on the device and sent when the server can be reached (see
//! `queue` and `sync`).

mod accent;
mod api;
mod dialogs;
mod drafts;
mod edit;
mod explain;
mod help;
mod i18n;
mod icon;
mod install;
mod list;
mod login;
mod mini;
mod mode;
mod others;
mod people;
mod place;
mod chart;
mod push;
mod rates;
mod settings;
mod settings_page;
mod show_in;
mod queue;
mod sync;
mod tour;
mod ui;
mod version;

use i18n::t;
use leptos::prelude::*;
use leptos::task::spawn_local;

fn main() {
    console_error_panic_hook::set_once();
    i18n::mark_document();
    leptos::mount::mount_to_body(App);
    // The page carries a plain "Starting…" of its own, because until this line runs there
    // is nothing in the body at all - and a client that never starts would otherwise be a
    // white screen with no word on it. See the note beside it in index.html.
    tcw_started();
}

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function tcw_started() {
    window.tcwStarted = true;
    const boot = document.getElementById('tcw-boot');
    if (boot) boot.remove();
    // The one automatic reload the boot screen allows itself is spent per start, not per
    // tab: see index.html.
    try { sessionStorage.removeItem('tcwRetried'); } catch (e) {}
}
"#)]
extern "C" {
    fn tcw_started();
}

/// Which screen the address bar is asking for.
///
/// Three routes is not enough to justify a routing library, and one of the things being
/// measured here is size, so this reads the path itself. When editing arrives and the
/// routes multiply, `leptos_router` is the obvious replacement.
#[derive(Clone, Debug, PartialEq)]
enum Route {
    List,
    /// What everything on screen means.
    Help,
    /// The two settings this client has.
    Settings,
    /// A tour, and which of its tabs the address asks for. The app's own deep links -
    /// /tour/x/persons and /tour/x/spendings - land on the matching tab, and
    /// /tour/x/spending/add opens the tour with the expense dialog already up.
    Tour(String, Landing),
    /// A share link: an access code and the tour it opens.
    Goto(String, String),
    Unknown(String),
}

/// Where in a tour a link points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landing {
    /// The address named a tour and nothing else, so which tab to open is the tour's to
    /// decide - see `tour::opens_on`.
    Unsaid,
    Balance,
    People,
    Expenses,
    Stats,
    /// Straight into "record an expense", which is what the app's own add link does.
    AddSpending,
}

fn current_route() -> Route {
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".into());
    route_of(&path)
}

fn route_of(path: &str) -> Route {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        [] | ["tourlist"] => Route::List,
        ["help"] => Route::Help,
        ["settings"] => Route::Settings,
        ["tour", id] => Route::Tour((*id).to_owned(), Landing::Unsaid),
        ["tour", id, "persons"] => Route::Tour((*id).to_owned(), Landing::People),
        ["tour", id, "spendings"] => Route::Tour((*id).to_owned(), Landing::Expenses),
        ["tour", id, "stats"] => Route::Tour((*id).to_owned(), Landing::Stats),
        ["tour", id, "spending", "add"] => Route::Tour((*id).to_owned(), Landing::AddSpending),
        ["goto", code, id] => Route::Goto((*code).to_owned(), (*id).to_owned()),
        _ => Route::Unknown(path.to_owned()),
    }
}

/// Whether a form is open somewhere on screen - a new expense, an edit, the currencies.
///
/// Above every page, because what must not disturb a half-typed form is not always on the
/// same page: a tapped notification about another tour would otherwise throw it away.
#[derive(Clone, Copy)]
pub struct Editing(pub RwSignal<bool>);

/// A tour the reader was asked to open - by tapping a notification - and what changed in it.
/// Set only while a form is open; otherwise the app simply goes there.
type AskedToOpen = RwSignal<Option<(String, String)>>;

/// Listens for the service worker saying which tour a tapped notification was about.
///
/// The worker used to move the window there itself, which a browser refuses for a window it
/// does not control - so a notification brought the app to the front and left it on
/// whatever screen it was on. Going there from inside the app also keeps what is on screen:
/// no reload, and a form somebody is in the middle of is asked about rather than thrown
/// away.
fn listen_for_the_worker(set_route: WriteSignal<Route>, editing: RwSignal<bool>, asked: AskedToOpen) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let container = window.navigator().service_worker();
    use wasm_bindgen::JsCast;
    let heard = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
        move |event: web_sys::MessageEvent| {
            let data = event.data();
            let field = |name: &str| {
                js_sys::Reflect::get(&data, &name.into())
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default()
            };
            if field("tc") != "open-tour" {
                return;
            }
            // Somebody heard it, so the worker does not fall back to reloading the app.
            if let Ok(port) = event.ports().get(0).dyn_into::<web_sys::MessagePort>() {
                let _ = port.post_message(&wasm_bindgen::JsValue::from_str("ok"));
            }
            let tour = field("tourId");
            if tour.is_empty() {
                return;
            }
            // Already reading it: the notification has done its job by bringing the app
            // to the front.
            if matches!(current_route(), Route::Tour(open, _) if open == tour) {
                return;
            }
            if editing.get_untracked() {
                asked.set(Some((tour, field("message"))));
            } else {
                go(&format!("/tour/{tour}"), set_route);
            }
        },
    )
    .into_js_value();
    let _ = container.add_event_listener_with_callback("message", heard.unchecked_ref());
    // Never removed: it lives as long as the app does.
}

/// Moves to another screen without reloading the page.
fn go(route_to: &str, set_route: WriteSignal<Route>) {
    if let Some(w) = web_sys::window() {
        let _ = w
            .history()
            .map(|h| h.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(route_to)));
        // A real navigation starts at the top of the page; halfway down a list of forty
        // expenses is not where the next screen begins.
        w.scroll_to_with_x_and_y(0.0, 0.0);
    }
    set_route.set(current_route());
}

/// Makes every internal link a change of signal instead of a page load.
///
/// This is the whole difference between switching tours in a quarter of a second and
/// switching them in a few milliseconds. Without it a plain `<a href>` is a real navigation:
/// the browser throws the app away and builds it again - fetch the document, fetch the glue,
/// compile a megabyte of wasm, mount, read local storage, ask for the tour - all to draw a
/// screen the app was already holding the data for. Measured on localhost with a warm cache,
/// that reboot is ~200 ms before the app even starts; the redraw it replaces is single
/// figures.
///
/// One listener on the document rather than a handler per link: links are written in a dozen
/// places, and a component that forgets the handler is a component that reloads the page.
fn intercept_links(set_route: WriteSignal<Route>) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let handler = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MouseEvent)>::new(
        move |ev: web_sys::MouseEvent| {
            // Everything the browser is expected to handle itself: the middle button opens a
            // tab, ctrl-click opens a tab, and a handler that has already dealt with the
            // click has the last word.
            if ev.default_prevented()
                || ev.button() != 0
                || ev.meta_key()
                || ev.ctrl_key()
                || ev.shift_key()
                || ev.alt_key()
            {
                return;
            }
            let Some(anchor) = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| el.closest("a").ok().flatten())
            else {
                return;
            };
            if anchor.has_attribute("target") || anchor.has_attribute("download") {
                return;
            }
            let href = anchor.get_attribute("href").unwrap_or_default();
            // Ours are paths. Anything else - another site, a mailto:, an anchor within the
            // page - belongs to the browser. `//host` is a URL, not a path.
            if !href.starts_with('/') || href.starts_with("//") {
                return;
            }
            // A share link is not a screen: it exchanges a code for a token on the way
            // through, and that happens when the app starts. Let it load properly.
            if matches!(route_of(&href), Route::Goto(_, _)) {
                return;
            }
            ev.prevent_default();
            go(&href, set_route);
        },
    );
    let _ = document
        .add_event_listener_with_callback("click", handler.as_ref().unchecked_ref());
    handler.forget();

    // Back and forward move through history without touching the document, so the app has
    // to notice by itself. Without this the buttons would look broken.
    if let Some(w) = web_sys::window() {
        let back = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_: web_sys::Event| set_route.set(current_route()),
        );
        let _ = w.add_event_listener_with_callback("popstate", back.as_ref().unchecked_ref());
        back.forget();
    }
}

/// How often the app tries a queue that is still waiting. Nothing is asked of the server
/// while every queue is empty, which is nearly always.
const RETRY_WAITING: std::time::Duration = std::time::Duration::from_secs(20);

/// Sends the queues of tours nobody is looking at: when the network comes back, when the app
/// comes back into view, and every [`RETRY_WAITING`] while anything is still waiting.
///
/// The open tour is left out: its own page sends for it, and it knows how to say what
/// changed afterwards. Two senders for one tour cannot collide in any case - `sync::push`
/// allows one at a time - but the page's is the one that redraws the screen.
fn send_what_is_waiting(route: ReadSignal<Route>) {
    // `Copy`, so the timer and the listener can each have it.
    let go = move || {
        let open = match route.try_get_untracked() {
            Some(Route::Tour(id, _)) => Some(id),
            _ => None,
        };
        // Nobody signed in: every answer would be "who are you?". The queue waits for the
        // code to be typed again.
        if !api::signed_in() {
            return;
        }
        for tour in queue::tours_with_pending() {
            // The open tour is its page's to send; one the server keeps refusing waits for
            // the reader to decide (see `queue::GIVE_UP_AFTER`).
            if Some(&tour) == open.as_ref() || queue::given_up(&tour) {
                continue;
            }
            leptos::task::spawn_local(async move {
                let _ = sync::push(&tour).await;
            });
        }
    };

    let Some(window) = web_sys::window() else {
        return;
    };
    use wasm_bindgen::JsCast;
    let listener = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
        move |_: web_sys::Event| {
            // Coming back into view counts too: a phone that woke up with a network again
            // raises no "online" at all.
            let hidden = web_sys::window()
                .and_then(|w| w.document())
                .is_some_and(|d| d.hidden());
            if !hidden {
                go();
            }
        },
    )
    .into_js_value();
    let _ = window.add_event_listener_with_callback("online", listener.unchecked_ref());
    if let Some(document) = window.document() {
        let _ = document.add_event_listener_with_callback("visibilitychange", listener.unchecked_ref());
    }
    // And on a timer, because the events cannot be relied on: a phone coming off flight
    // mode raises "online" sometimes and not others - in a browser told to go offline for a
    // test, `navigator.onLine` never changed at all - and a queue that waits for an event
    // that never comes waits until somebody presses refresh.
    leptos::prelude::set_interval(go, RETRY_WAITING);
    // Never removed: both live as long as the app does.
}

#[component]
fn App() -> impl IntoView {
    // Which interface, shared by every screen: the switch is in the header and both the list
    // and the tour read it.
    let mode = RwSignal::new(mode::stored());
    provide_context(mode);
    Effect::new(move |_| mode::on_the_body(mode.get()));

    // What this browser is set to, and the colour it is painted in. Applied before anything
    // is drawn, so the page does not flash the default first.
    let settings: settings::Shared = RwSignal::new(settings::stored());
    accent::apply(&settings.get_untracked().accent);
    provide_context(settings);

    // Asked once by the mark in the header, shown there and on the bar under it.
    provide_context(version::Stale(RwSignal::new(false)));

    let (route, set_route) = signal(current_route());
    intercept_links(set_route);

    // Edits made without a network go out as soon as there is one, whatever screen the
    // reader is on - the tour list, Help, or the app left in the background. The tour page
    // watches for its own tour (see `others`); this is for every other one, and for the
    // times nothing on screen is watching at all.
    // What is waiting to be sent is drawn from the queue itself, which reports its changes
    // on this signal - see `queue::reports_changes_on`.
    queue::reports_changes_on(RwSignal::new(0));
    send_what_is_waiting(route);

    // A tapped notification, and whether the screen may move to it right now.
    let editing = Editing(RwSignal::new(false));
    provide_context(editing);
    let asked_to_open: AskedToOpen = RwSignal::new(None);
    listen_for_the_worker(set_route, editing.0, asked_to_open);

    // Whether the narrow-screen menu is open. On a wide screen there is no menu: the same
    // controls are simply a row, and this signal never does anything.
    let menu = RwSignal::new(false);

    // Where the reader is, which is not the same question as which screen is on: stepping
    // into Help or Settings does not leave the tour, it looks something up. Read from this
    // browser first, so a reload on one of those screens still knows the way back.
    let place: place::Current = RwSignal::new(place::stored());
    provide_context(place);
    Effect::new(move |_| match route.get() {
        Route::List => place::went_to_the_list(place),
        Route::Tour(id, _) => place::went_to_a_tour(place, &id),
        // Help, Settings, a share link on its way through, a path that means nothing: none
        // of them is a place, and all of them are somewhere you came from somewhere else.
        _ => {}
    });
    // The browser's own title, too: the app sets it to the tour's name, and it is what a
    // tab, a bookmark and a shared link are called. Ours said "Tourcalc" for every tour, so
    // three tabs of three tours were three of the same thing. Here it follows the screen
    // rather than the place - a tab called "Settings" is about settings.
    Effect::new(move |_| {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.set_title(&match route.get() {
                Route::Tour(..) => place.get().label(),
                Route::Help => format!("{} · Tourcalc", t().shell.help_title),
                Route::Settings => format!("{} · Tourcalc", t().shell.settings_title),
                _ => "Tourcalc".to_owned(),
            });
        }
    });
    // Whether anybody is signed in on this device. A signal rather than a check in the
    // view, so that signing in or out redraws without a reload.
    let signed_in = RwSignal::new(api::signed_in());
    // And a request that finds the login gone takes the reader to the sign-in screen,
    // instead of to an empty list that says they have no tours.
    api::reports_sign_in_on(signed_in);

    // What the bar is about. Somebody who is not signed in is shown none of it: the tour
    // they were in before is not theirs to be reminded of until the code is typed again.
    let named_at_the_top = Memo::new(move |_| {
        if signed_in.get() {
            place.get()
        } else {
            place::Place::List
        }
    });
    // Whether the bar is naming the screen you are on or the one you can go back to. The
    // second is a button in a way the first is not, and it is worth looking like one.
    let stepped_aside = Memo::new(move |_| {
        signed_in.get() && !matches!(route.get(), Route::List | Route::Tour(..))
    });

    // A share link is not a screen: it exchanges the code for a token and then goes where
    // it was pointing. Done once, when that is the route we arrived on.
    //
    // The link's code is added to the ones already held, not put in their place: somebody in
    // three companies sees the tours of all three, as they did in the app. It used to replace
    // them, and every link from one company hid the tours of the others. Should the server
    // refuse the lot, the link's code alone - which is what the app fell back to as well.
    if let Route::Goto(code, id) = route.get_untracked() {
        spawn_local(async move {
            let all = api::codes_with(&api::my_codes().await, &code);
            let signed = match api::log_in_with_md5(&all).await {
                Err(_) if all != code.trim().to_uppercase() => api::log_in_with_md5(&code).await,
                other => other,
            };
            match signed {
                Ok(()) => {
                    signed_in.set(true);
                    go(&format!("/tour/{id}"), set_route)
                }
                Err(_) => go("/", set_route),
            }
        });
    }

    let signed_in_now = Callback::new(move |_: ()| {
        signed_in.set(true);
        go("/", set_route);
    });

    view! {
        <div class="tcn-shell" class:tcm-shell=move || mode.get() == mode::UiMode::Mini>
            <header class="tcn-topbar">
                <a class="tcn-brand" href="/" title=t().shell.tour_list>"🧭"</a>
                // Whether this browser is running the client the server hands out.
                <version::VersionMark />
                <a class="tcn-topbar-title" class:tcw-back=move || stepped_aside.get()
                   href=move || named_at_the_top.get().href()
                   title=move || {
                       let place = named_at_the_top.get();
                       if stepped_aside.get() { place.back_to() } else { place.label() }
                   }>
                    {move || named_at_the_top.get().label()}
                </a>
                // Only on a narrow screen, where the controls become the panel below.
                <button type="button" class="tcn-iconbtn tcw-menu-btn" title=t().shell.menu
                        aria-label=t().shell.menu aria-expanded=move || menu.get().to_string()
                        on:click=move |_| menu.update(|m| *m = !*m)>
                    <icon::Icon name="more" />
                </button>
                <Show when=move || menu.get()>
                    // A tap anywhere else puts it away, which is what a menu is expected to
                    // do and what nothing else here would have done.
                    <div class="tcw-menu-backdrop" on:click=move |_| menu.set(false)></div>
                </Show>
                <div class="tcn-topbar-actions" class:tcw-open=move || menu.get()
                     on:click=move |_| menu.set(false)>
                    <mode::ModeSwitch mode=mode />
                    <a class="tcn-iconbtn" href="/help" title=t().shell.help_hint
                       aria-label=t().shell.help>
                        <icon::Icon name="help" />
                    </a>
                    <a class="tcn-iconbtn" href="/settings" title=t().shell.settings aria-label=t().shell.settings>
                        <icon::Icon name="settings" />
                    </a>
                    <Show when=move || signed_in.get()>
                        <button type="button" class="tcn-iconbtn" title=t().shell.log_out aria-label=t().shell.log_out
                                on:click=move |_| {
                                    api::log_out();
                                    signed_in.set(false);
                                    go("/", set_route);
                                }>
                            <icon::Icon name="logout" />
                        </button>
                    </Show>
                </div>
            </header>
            <version::UpdateBar />
            // A notification tapped while a form is open: the screen stays where it is and
            // says where it could go, so nothing half-typed is thrown away for it.
            <Show when=move || asked_to_open.get().is_some()>
                <div class="tcw-asked" role="status">
                    <span class="tcw-asked-text">
                        {move || {
                            let (_, what) = asked_to_open.get().unwrap_or_default();
                            if what.is_empty() {
                                t().shell.a_tour_changed.to_owned()
                            } else {
                                what
                            }
                        }}
                    </span>
                    <button type="button" class="tcw-asked-open"
                            on:click=move |_| {
                                if let Some((tour, _)) = asked_to_open.get_untracked() {
                                    asked_to_open.set(None);
                                    go(&format!("/tour/{tour}"), set_route);
                                }
                            }>
                        {t().shell.open}
                    </button>
                    <button type="button" class="tcw-others-close" aria-label=t().shell.dismiss
                            on:click=move |_| asked_to_open.set(None)>"×"</button>
                </div>
            </Show>
            <main class="tcn-main">
                {move || match (signed_in.get(), route.get()) {
                    // Nothing is readable without a code, so the sign-in screen stands in
                    // front of every route except the share link, which signs in by itself.
                    (false, Route::Goto(_, _)) => {
                        view! { <div class="tcn-loading">{t().shell.signing_in}</div> }.into_any()
                    }
                    // Help is readable without a code: somebody who has just been handed a
                    // link and does not know what any of this is starts here.
                    (_, Route::Help) => view! { <help::HelpPage /> }.into_any(),
                    (false, _) => view! { <login::SignIn on_done=signed_in_now /> }.into_any(),
                    (true, route) => match route {
                        Route::List => view! { <list::TourListPage /> }.into_any(),
                        Route::Tour(id, landing) => view! {
                            // Reading `mode` here is what rebuilds the page when the
                            // interface is switched: the tour draws itself one way or the
                            // other and nothing in it has to be reactive about which.
                            {move || {
                                let _ = mode.get();
                                view! { <tour::TourPage id=id.clone() landing=landing /> }
                            }}
                        }.into_any(),
                        Route::Goto(_, _) => {
                            view! { <div class="tcn-loading">{t().shell.signing_in}</div> }.into_any()
                        }
                        Route::Help => ().into_any(),
                        Route::Settings => view! {
                            <settings_page::SettingsPage settings=settings />
                        }.into_any(),
                        Route::Unknown(path) => view! {
                            <div class="tcn-section">
                                <div class="tcn-errors">{t().queue.nothing_here} {path}</div>
                                <a class="tcn-btn" href="/">{t().queue.go_to_tours}</a>
                            </div>
                        }.into_any(),
                    },
                }}
            </main>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_read_the_way_the_address_bar_writes_it() {
        assert_eq!(route_of("/"), Route::List);
        assert_eq!(route_of("/tourlist"), Route::List);
        assert_eq!(route_of("/settings"), Route::Settings);
        assert!(matches!(route_of("/tour/abc"), Route::Tour(id, Landing::Unsaid) if id == "abc"));
        assert!(matches!(
            route_of("/tour/abc/persons"),
            Route::Tour(_, Landing::People)
        ));
        assert!(matches!(
            route_of("/tour/abc/spending/add"),
            Route::Tour(_, Landing::AddSpending)
        ));
        // A query string or a fragment belongs to the page, not to the route: without this
        // a link with one on the end would be swallowed by the interceptor and land on
        // "nothing here".
        assert_eq!(route_of("/help?from=list"), Route::Help);
        assert_eq!(route_of("/help#weights"), Route::Help);
    }

    #[test]
    fn a_tour_on_its_own_leaves_the_tab_to_the_tour() {
        // "/tour/x" is not a request for the balances: it is a request for the tour, and
        // which tab that opens depends on whether it is being settled up. The three that do
        // name a section still mean what they say.
        assert!(matches!(
            route_of("/tour/abc"),
            Route::Tour(_, Landing::Unsaid)
        ));
        assert!(matches!(
            route_of("/tour/abc/spendings"),
            Route::Tour(_, Landing::Expenses)
        ));
    }

    #[test]
    fn a_share_link_is_left_to_the_browser() {
        // The interceptor asks exactly this before it swallows a click. A share link
        // exchanges its code for a token while the app is starting, so it has to start:
        // turning it into a change of signal would land on a tour nobody is signed in for.
        assert!(matches!(route_of("/goto/CODE/tourid"), Route::Goto(..)));
    }
}
