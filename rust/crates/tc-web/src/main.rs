//! Tourcalc's client, in Rust.
//!
//! Phase 3: read-only, and deliberately the same screens the Blazor client shows, reusing
//! its stylesheet unchanged. If the two look alike, that is the point - what is being
//! measured is what the browser has to download to get there, not a new design.

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
mod queue;
mod sync;
mod tour;
mod ui;

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
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        [] | ["tourlist"] => Route::List,
        ["help"] => Route::Help,
        ["tour", id] => Route::Tour((*id).to_owned(), Landing::Balance),
        ["tour", id, "persons"] => Route::Tour((*id).to_owned(), Landing::People),
        ["tour", id, "spendings"] => Route::Tour((*id).to_owned(), Landing::Expenses),
        ["tour", id, "stats"] => Route::Tour((*id).to_owned(), Landing::Stats),
        ["tour", id, "spending", "add"] => Route::Tour((*id).to_owned(), Landing::AddSpending),
        ["goto", code, id] => Route::Goto((*code).to_owned(), (*id).to_owned()),
        _ => Route::Unknown(path),
    }
}

/// Moves to another screen without reloading the page.
fn go(route_to: &str, set_route: WriteSignal<Route>) {
    if let Some(w) = web_sys::window() {
        let _ = w
            .history()
            .map(|h| h.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(route_to)));
    }
    set_route.set(current_route());
}

#[component]
fn App() -> impl IntoView {
    // Which interface, shared by every screen: the switch is in the header and both the list
    // and the tour read it.
    let mode = RwSignal::new(mode::stored());
    provide_context(mode);

    let (route, set_route) = signal(current_route());
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
