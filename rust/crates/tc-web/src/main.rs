//! Tourcalc's client, in Rust.
//!
//! Phase 3: read-only, and deliberately the same screens the Blazor client shows, reusing
//! its stylesheet unchanged. If the two look alike, that is the point - what is being
//! measured is what the browser has to download to get there, not a new design.

mod accent;
mod api;
mod dialogs;
mod edit;
mod explain;
mod help;
mod icon;
mod list;
mod login;
mod mini;
mod mode;
mod people;
mod push;
mod settings;
mod settings_page;
mod queue;
mod sync;
mod tour;
mod ui;
mod version;

use leptos::prelude::*;
use leptos::task::spawn_local;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
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
        ["tour", id] => Route::Tour((*id).to_owned(), Landing::Balance),
        ["tour", id, "persons"] => Route::Tour((*id).to_owned(), Landing::People),
        ["tour", id, "spendings"] => Route::Tour((*id).to_owned(), Landing::Expenses),
        ["tour", id, "stats"] => Route::Tour((*id).to_owned(), Landing::Stats),
        ["tour", id, "spending", "add"] => Route::Tour((*id).to_owned(), Landing::AddSpending),
        ["goto", code, id] => Route::Goto((*code).to_owned(), (*id).to_owned()),
        _ => Route::Unknown(path.to_owned()),
    }
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

#[component]
fn App() -> impl IntoView {
    // Which interface, shared by every screen: the switch is in the header and both the list
    // and the tour read it.
    let mode = RwSignal::new(mode::stored());
    provide_context(mode);

    // What this browser is set to, and the colour it is painted in. Applied before anything
    // is drawn, so the page does not flash the default first.
    let settings: settings::Shared = RwSignal::new(settings::stored());
    accent::apply(&settings.get_untracked().accent);
    provide_context(settings);

    let (route, set_route) = signal(current_route());
    intercept_links(set_route);
    // Whether anybody is signed in on this device. A signal rather than a check in the
    // view, so that signing in or out redraws without a reload.
    let signed_in = RwSignal::new(api::signed_in());

    // A share link is not a screen: it exchanges the code for a token and then goes where
    // it was pointing. Done once, when that is the route we arrived on.
    if let Route::Goto(code, id) = route.get_untracked() {
        spawn_local(async move {
            match api::log_in_with_md5(&code).await {
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
        <div class="tcn-shell">
            <header class="tcn-topbar">
                <a class="tcn-brand" href="/" title="Tour list">"🧭"</a>
                <a class="tcn-topbar-title" href="/">"Tourcalc"</a>
                <div class="tcn-topbar-actions">
                    <span class="tcn-chip" title="This interface is written in Rust">"rust"</span>
                    <mode::ModeSwitch mode=mode />
                    <a class="tcn-iconbtn" href="/help" title="What everything here means"
                       aria-label="Help">
                        <icon::Icon name="help" />
                    </a>
                    <a class="tcn-iconbtn" href="/settings" title="Settings" aria-label="Settings">
                        <icon::Icon name="settings" />
                    </a>
                    <Show when=move || signed_in.get()>
                        <button type="button" class="tcn-iconbtn" title="Log out" aria-label="Log out"
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
            <main class="tcn-main">
                {move || match (signed_in.get(), route.get()) {
                    // Nothing is readable without a code, so the sign-in screen stands in
                    // front of every route except the share link, which signs in by itself.
                    (false, Route::Goto(_, _)) => {
                        view! { <div class="tcn-loading">"Signing in…"</div> }.into_any()
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
                            view! { <div class="tcn-loading">"Signing in…"</div> }.into_any()
                        }
                        Route::Help => ().into_any(),
                        Route::Settings => view! {
                            <settings_page::SettingsPage settings=settings />
                        }.into_any(),
                        Route::Unknown(path) => view! {
                            <div class="tcn-section">
                                <div class="tcn-errors">"Nothing here: " {path}</div>
                                <a class="tcn-btn" href="/">"Go to my tours"</a>
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
        assert!(matches!(route_of("/tour/abc"), Route::Tour(id, Landing::Balance) if id == "abc"));
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
    fn a_share_link_is_left_to_the_browser() {
        // The interceptor asks exactly this before it swallows a click. A share link
        // exchanges its code for a token while the app is starting, so it has to start:
        // turning it into a change of signal would land on a tour nobody is signed in for.
        assert!(matches!(route_of("/goto/CODE/tourid"), Route::Goto(..)));
    }
}
