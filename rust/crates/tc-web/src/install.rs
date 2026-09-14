//! Putting the app on a phone's home screen as an app, not as a bookmark.
//!
//! A browser decides on its own whether a site may be installed - a manifest, an icon big
//! enough, a service worker, https - and when it decides yes it offers the page one event,
//! once, early. `index.html` catches that event and keeps it; this is what asks for it back.
//!
//! Why this exists at all, given that Chrome has its own menu item: because the menu item
//! is silent about *why* it is not there. A reader who taps "add to home screen" and gets a
//! bookmark cannot tell whether the app refuses to be installed, or is installed already,
//! or whether something in it is simply broken. So the setting says one of those three
//! things out loud, and a button is only one of the three.

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
export function tcw_offered() {
    return !!window.tcwInstall;
}

// Whether this is already the installed app rather than a tab. The media query is what
// every browser but Safari answers; `navigator.standalone` is Safari's own, older way.
export function tcw_running_installed() {
    // Installed a moment ago, from here or from the browser's own menu: the window this
    // happened in is still an ordinary tab, so no media query will ever say so.
    if (window.tcwInstalled) return true;
    try {
        if (window.matchMedia('(display-mode: standalone)').matches) return true;
        if (window.matchMedia('(display-mode: fullscreen)').matches) return true;
        if (window.matchMedia('(display-mode: minimal-ui)').matches) return true;
    } catch (e) { /* an old browser with no matchMedia is not an installed app */ }
    return window.navigator.standalone === true;
}

// Safari installs from the share sheet and has no event to offer, so there is nothing to
// press and the setting has to say so in words instead.
export function tcw_is_ios() {
    const ua = navigator.userAgent || '';
    const iOS = /iPad|iPhone|iPod/.test(ua);
    // An iPad has called itself a Mac since iPadOS 13; the touch points give it away.
    const iPadOS = navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1;
    return iOS || iPadOS;
}

export function tcw_mark_installed() {
    window.tcwInstalled = true;
}

export async function tcw_install() {
    const offer = window.tcwInstall;
    if (!offer) return 'gone';
    // The offer is good for one prompt, whatever the answer.
    window.tcwInstall = null;
    try {
        offer.prompt();
        const choice = await offer.userChoice;
        return choice && choice.outcome ? choice.outcome : 'dismissed';
    } catch (e) {
        return 'failed';
    }
}
"#)]
extern "C" {
    fn tcw_offered() -> bool;
    fn tcw_running_installed() -> bool;
    fn tcw_is_ios() -> bool;
    #[wasm_bindgen(js_name = tcw_mark_installed)]
    fn mark_installed();
    async fn tcw_install() -> wasm_bindgen::JsValue;
}

/// What the setting can say, in the order it decides.
#[derive(Clone, Copy, PartialEq)]
pub enum State {
    /// This *is* the installed app. Nothing to offer.
    Installed,
    /// The browser has offered, and the offer is still in hand.
    Offered,
    /// Safari, which installs from the share sheet and says nothing to the page.
    ByHand,
    /// Nothing on offer: the browser has not decided yes, or has decided no.
    No,
}

pub fn state() -> State {
    if tcw_running_installed() {
        State::Installed
    } else if tcw_offered() {
        State::Offered
    } else if tcw_is_ios() {
        State::ByHand
    } else {
        State::No
    }
}

/// The setting. Always a row, never an empty space: "nothing here" is the one answer that
/// cannot be told apart from a bug.
#[component]
pub fn InstallSetting() -> impl IntoView {
    let now = RwSignal::new(state());
    // The offer does not always arrive before this screen does - the browser weighs it up
    // in its own time - so the answer is asked for again while the page is open rather
    // than settled once and left wrong.
    leptos::prelude::set_interval(
        move || {
            let fresh = state();
            if fresh != now.get_untracked() {
                now.set(fresh);
            }
        },
        std::time::Duration::from_millis(1200),
    );
    let outcome: RwSignal<Option<&'static str>> = RwSignal::new(None);

    let ask = move |_| {
        outcome.set(None);
        leptos::task::spawn_local(async move {
            let answer = tcw_install().await.as_string().unwrap_or_default();
            if answer == "accepted" {
                // This tab stays a tab, so nothing about it changes to say what happened.
                mark_installed();
            }
            outcome.set(Some(match answer.as_str() {
                "accepted" => "Installing — the app will appear beside your others.",
                "dismissed" => "Not this time. The offer stays until the page is reloaded.",
                "gone" => "The browser has withdrawn the offer. Reload the page and try again.",
                _ => "The browser would not open the install dialogue.",
            }));
            now.set(state());
        });
    };

    view! {
        <div class="tcn-setrow">
            <div class="tcn-settext">
                <div class="tcn-setname">"Install on this device"</div>
                <div class="tcn-setdesc">
                    "Tourcalc as an app of its own: its own window with no address bar, its
                     own icon, and everything it has already downloaded, so it opens without
                     a network."
                </div>
                // Only until something has actually happened: after a press, what the
                // browser answered is the news, and the standing explanation underneath it
                // would be answering a question nobody is asking any more.
                {move || if outcome.get().is_some() { ().into_any() } else { match now.get() {
                    State::Installed => view! {
                        <div class="tcn-hint" style="margin-top:6px">
                            "Already installed — this is the installed app."
                        </div>
                    }.into_any(),
                    State::ByHand => view! {
                        <div class="tcn-hint" style="margin-top:6px">
                            "On iPhone and iPad it is done from the share menu: "
                            <b>"Share → Add to Home Screen"</b>
                            ". Safari does not let a page ask."
                        </div>
                    }.into_any(),
                    State::No => view! {
                        <div class="tcn-hint" style="margin-top:6px">
                            "This browser is not offering it. Either it does not install web
                             apps, or it has not decided yet — it makes up its own mind a
                             moment after the page loads, and this line follows it."
                        </div>
                    }.into_any(),
                    State::Offered => ().into_any(),
                }}}
                {move || outcome.get().map(|said| view! {
                    <div class="tcn-hint" style="margin-top:6px">{said}</div>
                })}
            </div>
            {move || match now.get() {
                State::Offered => view! {
                    <button type="button" class="tcn-btn tcn-btn-primary" on:click=ask>
                        "Install"
                    </button>
                }.into_any(),
                // Not an empty space: a reader who sees nothing cannot tell "this browser
                // will not" from "the button is broken".
                _ => view! { <span class="tcn-chip">"Cannot install"</span> }.into_any(),
            }}
        </div>
    }
}
