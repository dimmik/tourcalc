//! Tourcalc's client, in Rust.
//!
//! Phase 3: read-only, and deliberately the same screens the Blazor client shows, reusing
//! its stylesheet unchanged. If the two look alike, that is the point - what is being
//! measured is what the browser has to download to get there, not a new design.

mod api;
mod list;
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
    Tour(String),
    /// A share link: an access code and the tour it opens.
    Goto(String, String),
    Unknown(String),
}

fn current_route() -> Route {
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".into());
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        [] | ["tourlist"] => Route::List,
        ["tour", id] => Route::Tour((*id).to_owned()),
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
    let (route, set_route) = signal(current_route());

    // A share link is not a screen: it exchanges the code for a token and then goes where
    // it was pointing. Done once, when that is the route we arrived on.
    if let Route::Goto(code, id) = route.get_untracked() {
        spawn_local(async move {
            match api::log_in_with_md5(&code).await {
                Ok(()) => go(&format!("/tour/{id}"), set_route),
                Err(_) => go("/", set_route),
            }
        });
    }

    view! {
        <div class="tcn-shell">
            <header class="tcn-topbar">
                <a class="tcn-brand" href="/" title="Tour list">"🧭"</a>
                <a class="tcn-topbar-title" href="/">"Tourcalc"</a>
                <div class="tcn-topbar-actions">
                    <span class="tcn-chip" title="This interface is written in Rust">"rust"</span>
                </div>
            </header>
            <main class="tcn-main">
                {move || match route.get() {
                    Route::List => view! { <list::TourListPage /> }.into_any(),
                    Route::Tour(id) => view! { <tour::TourPage id=id /> }.into_any(),
                    Route::Goto(_, _) => {
                        view! { <div class="tcn-loading">"Signing in…"</div> }.into_any()
                    }
                    Route::Unknown(path) => view! {
                        <div class="tcn-section">
                            <div class="tcn-errors">"Nothing here: " {path}</div>
                            <a class="tcn-btn" href="/">"Go to my tours"</a>
                        </div>
                    }.into_any(),
                }}
            </main>
        </div>
    }
}
