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

// Whether this app is already on the device, asked of the browser rather than guessed.
//
// The manifest has to name itself under `related_applications` for this to answer at all -
// the call was built for "is my *native* app installed", and listing your own manifest is
// how it was extended to answer for the web app. Relative, so that it means this origin
// wherever this is running: production, beta and a laptop are three different addresses.
// Both manifest addresses are named: an app installed while this domain served the other
// client was installed from the old one, and it is the installed copy's own manifest URL
// that is matched against.
//
// It is the answer to the case that looks exactly like "cannot install" and is not: a
// browser that has the app installed already stops offering to install it, and the tab
// asking the question is still an ordinary tab, so nothing about *it* has changed. Chrome
// includes the web app itself in this list; Firefox and Safari do not have the call at all,
// which is not an error - they simply cannot say.
export async function tcw_installed_elsewhere() {
    if (!navigator.getInstalledRelatedApps) return false;
    try {
        const apps = await navigator.getInstalledRelatedApps();
        return apps.some((app) => app.platform === 'webapp');
    } catch (e) {
        return false;
    }
}

// What the browser's own conditions look like from inside the page.
//
// Not decoration: a phone cannot be opened in a debugger, and "the browser is not offering
// it" is three sentences of guesswork without this. Each of these is one of the things a
// browser weighs, and the answer is short enough to read off a screen and say over the
// telephone.
export async function tcw_why() {
    const said = [];
    said.push('offer supported: ' + (('onbeforeinstallprompt' in window) ? 'yes' : 'no'));
    said.push('worker in control: ' + (navigator.serviceWorker && navigator.serviceWorker.controller ? 'yes' : 'no'));
    said.push('secure: ' + (window.isSecureContext ? 'yes' : 'no'));

    // The tag, and the file, separately. They answer different questions: a browser only
    // reads a manifest the page points at, but a page that points at nothing while the file
    // is perfectly well served is a page that is not the one this server sent - a copy kept
    // by the browser from some earlier build. That is worth knowing and cannot be guessed.
    const link = document.querySelector('link[rel=manifest]');
    // Whether the tag is the one the page was sent with or the one it put back: on a device
    // that strips it, that is the difference between "installable again" and "still not".
    const mine = link && link.dataset && link.dataset.tcw === 'page';
    said.push('link: ' + (link ? link.getAttribute('href') + (mine ? '' : ' (restored)') : 'MISSING'));

    try {
        const answer = await fetch('/manifest.webmanifest');
        const manifest = await answer.json();
        const icons = (manifest.icons || []).length;
        said.push('file: ' + answer.status + ' ' + (manifest.display || 'no display') + ' ' + icons + ' icons');
    } catch (e) {
        said.push('file: unreadable');
    }

    // Which build this page actually is. The server names the one it serves at
    // /api/Info/version; if the two differ, the browser is holding an old copy.
    const script = [...document.scripts].map((s) => s.textContent).join(' ');
    const named = script.match(/tc-web-[0-9a-f]{8}/);
    said.push('client: ' + (named ? named[0].slice(7) : 'unknown'));

    return said.join(' · ');
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
    #[wasm_bindgen(js_name = tcw_installed_elsewhere)]
    async fn installed_elsewhere() -> wasm_bindgen::JsValue;
    #[wasm_bindgen(js_name = tcw_why)]
    async fn why() -> wasm_bindgen::JsValue;
    async fn tcw_install() -> wasm_bindgen::JsValue;
}

/// What the setting can say, in the order it decides.
#[derive(Clone, Copy, PartialEq)]
pub enum State {
    /// This *is* the installed app. Nothing to offer.
    Installed,
    /// The app is on this device, but this is a tab rather than it.
    Elsewhere,
    /// The browser has offered, and the offer is still in hand.
    Offered,
    /// Safari, which installs from the share sheet and says nothing to the page.
    ByHand,
    /// Nothing on offer: the browser has not decided yes, or has decided no.
    No,
}

pub fn state(on_the_device: bool) -> State {
    if tcw_running_installed() {
        State::Installed
    } else if on_the_device {
        State::Elsewhere
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
    // Asked once, because the answer cannot change while this page is open: installing it
    // from here opens a different window, and uninstalling happens elsewhere entirely.
    let on_the_device = RwSignal::new(false);
    let now = RwSignal::new(state(false));
    leptos::task::spawn_local(async move {
        if installed_elsewhere().await.as_bool().unwrap_or(false) {
            on_the_device.set(true);
            now.set(state(true));
        }
    });
    // The offer does not always arrive before this screen does - the browser weighs it up
    // in its own time - so the answer is asked for again while the page is open rather
    // than settled once and left wrong.
    // And taken down with the screen: Settings is left as often as it is opened.
    if let Ok(asking) = leptos::prelude::set_interval_with_handle(
        move || {
            let Some(was) = now.try_get_untracked() else {
                return;
            };
            let fresh = state(on_the_device.try_get_untracked().unwrap_or(false));
            if fresh != was {
                now.try_set(fresh);
            }
        },
        std::time::Duration::from_millis(1200),
    ) {
        on_cleanup(move || asking.clear());
    }
    let outcome: RwSignal<Option<&'static str>> = RwSignal::new(None);
    // Asked for whether it is needed or not, because it is needed exactly when nobody can
    // ask for it: on somebody else's phone, with no way to look inside.
    let checks = RwSignal::new(String::new());
    leptos::task::spawn_local(async move {
        checks.set(why().await.as_string().unwrap_or_default());
    });

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
            now.set(state(on_the_device.get_untracked()));
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
                    State::Elsewhere => view! {
                        <div class="tcn-hint" style="margin-top:6px">
                            "Already on this device — open it from the home screen rather
                             than here. A browser that has it installed stops offering to
                             install it, which is why there is no button."
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
                            "This browser is not offering it. Three reasons are possible:
                             it does not install web apps at all; it has not decided yet, and
                             this line follows it when it does; or the app is on this device
                             already — a browser that has it installed stops offering, so
                             look for Tourcalc on the home screen before looking for a bug."
                        </div>
                    }.into_any(),
                    State::Offered => ().into_any(),
                }}}
                {move || outcome.get().map(|said| view! {
                    <div class="tcn-hint" style="margin-top:6px">{said}</div>
                })}
                // Only where it answers something: with a button on screen, nothing is
                // wrong and a row of diagnostics is clutter.
                <Show when=move || now.get() == State::No && !checks.get().is_empty()>
                    <div class="tcn-hint" style="margin-top:6px; opacity:.75">
                        "Checked here — " {move || checks.get()}
                    </div>
                </Show>
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
