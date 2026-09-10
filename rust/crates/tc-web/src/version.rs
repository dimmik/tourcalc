//! Which build is running, here and there.
//!
//! The app's own answer to this was the build date at the foot of the page, and a date
//! answers only one of the three questions a person actually has when the screen looks
//! wrong:
//!
//! * **Is my browser holding an old copy?** Browsers cache, and this one installs a service
//!   worker on top of that. Answered by comparing the wasm file this page loaded with the
//!   one the server names in the page it serves now - the name carries a hash of the
//!   contents, so two builds cannot share it and one build cannot lose it.
//! * **Is the server running what I pushed?** Answered by the build stamp and the commit,
//!   which the pipeline puts into the image.
//! * **Did the deploy happen at all?** Answered by how long the server has been up. A new
//!   image that nothing restarted is still the old server, and a date on a page could not
//!   tell you that.
//!
//! Only the first has a button, because it is the only one a reader can do anything about.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
export async function tcw_update() {
    // Everything the browser is keeping of this app, in the order that matters: the caches
    // the service worker filled, then the worker itself, then a reload that has nothing
    // left to serve it but the network.
    try {
        if (window.caches) {
            const keys = await caches.keys();
            await Promise.all(keys.map((k) => caches.delete(k)));
        }
    } catch (e) {}
    try {
        if (navigator.serviceWorker) {
            const regs = await navigator.serviceWorker.getRegistrations();
            await Promise.all(regs.map((r) => r.unregister()));
        }
    } catch (e) {}
    location.reload();
}
"#)]
extern "C" {
    fn tcw_update();
}

/// Throws away everything cached and starts again from the server.
pub fn update_now() {
    tcw_update();
}

/// The wasm this page actually loaded, read out of the page that loaded it.
///
/// Trunk writes the name into a `<link rel="preload">`, which is still in the head long
/// after the module has started - and it is the built page's own word for what it asked
/// for, rather than this code's guess at what it might have been called.
pub fn running_client() -> Option<String> {
    let href = web_sys::window()?
        .document()?
        .query_selector("link[href$='_bg.wasm']")
        .ok()
        .flatten()?
        .get_attribute("href")?;
    Some(href.trim_start_matches('/').to_owned())
}

/// What the server says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deployment {
    pub build: String,
    pub build_type: String,
    pub commit: String,
    /// The client it is handing out, which may not be the one this browser is running.
    pub client: String,
    pub started: String,
}

pub async fn deployment() -> Option<Deployment> {
    let text = crate::api::info_version().await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let field = |name: &str| {
        v.get(name)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    Some(Deployment {
        build: field("build"),
        build_type: field("buildType"),
        commit: field("commit"),
        client: field("client"),
        started: field("started"),
    })
}

/// Whether the server is handing out a client this browser is not running.
///
/// `None` while nobody has asked, and when the server did not answer: an unreachable server
/// is not evidence of an old browser, and saying so would send people to press a button that
/// cannot help them.
pub fn is_stale(server: &Deployment, running: &Option<String>) -> Option<bool> {
    let (Some(running), false) = (running.as_deref(), server.client.is_empty()) else {
        return None;
    };
    Some(server.client != running)
}

/// The bar that finds you: one line, only when there is something to do about it.
#[component]
pub fn UpdateBar() -> impl IntoView {
    let stale = RwSignal::new(false);
    let check = move || {
        spawn_local(async move {
            let running = running_client();
            if let Some(server) = deployment().await {
                if is_stale(&server, &running) == Some(true) {
                    stale.set(true);
                }
            }
        });
    };
    check();

    // And again whenever somebody comes back to the tab. Since navigating stopped reloading
    // the page, a check that runs once per document runs about as often as the browser is
    // restarted - which for an app kept open on a phone is never. Coming back to it is the
    // moment a person is about to use it, and one small request is a fair price for not
    // handing them a stale screen.
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let on_visible = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_: web_sys::Event| {
                let visible = web_sys::window()
                    .and_then(|w| w.document())
                    .is_some_and(|d| !d.hidden());
                if visible && !stale.get_untracked() {
                    check();
                }
            },
        );
        use wasm_bindgen::JsCast;
        let _ = document.add_event_listener_with_callback(
            "visibilitychange",
            on_visible.as_ref().unchecked_ref(),
        );
        on_visible.forget();
    }

    view! {
        // The app's own shape for "something is off and here is the button", borrowed from
        // the offline line so this reads as part of the app rather than as a browser
        // notification bolted on top.
        <Show when=move || stale.get()>
            <div class="tcn-section" style="padding-bottom:0">
                <div class="tcn-chip tcn-chip-amber">
                    "A newer version is on the server"
                    <button type="button" class="tcn-btn tcn-btn-sm" style="margin-left:10px"
                            on:click=move |_| update_now()>
                        "Update"
                    </button>
                </div>
            </div>
        </Show>
    }
}

/// The long answer, for the help page: what is running here, what is running there, and
/// since when.
#[component]
pub fn AboutBuild() -> impl IntoView {
    let running = RwSignal::new(running_client());
    let server = RwSignal::new(None::<Deployment>);
    let asked = RwSignal::new(false);

    let check = move || {
        asked.set(false);
        spawn_local(async move {
            server.set(deployment().await);
            asked.set(true);
        });
    };
    check();

    let short = |name: &str| {
        // The hash is the identity; the rest of the file name is the same in every build.
        name.trim_start_matches("tc-web-")
            .trim_end_matches("_bg.wasm")
            .chars()
            .take(8)
            .collect::<String>()
    };

    view! {
        <div class="tcn-section">
            <div class="tcn-section-title">"This build"</div>
            <div class="tcn-card" style="padding:14px">
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">"In this browser"</div>
                        <div class="tcn-setdesc">
                            {move || match running.get() {
                                Some(name) => format!("client {}", short(&name)),
                                None => "not known — this page was not built by trunk".to_owned(),
                            }}
                        </div>
                    </div>
                </div>

                {move || {
                    let Some(s) = server.get() else {
                        return view! {
                            <div class="tcn-setrow">
                                <div class="tcn-settext">
                                    <div class="tcn-setname">"On the server"</div>
                                    <div class="tcn-setdesc">
                                        {move || if asked.get() {
                                            "the server did not answer"
                                        } else {
                                            "asking…"
                                        }}
                                    </div>
                                </div>
                            </div>
                        }.into_any();
                    };
                    let stale = is_stale(&s, &running.get_untracked());
                    let started = crate::explain::pretty_stamp(&s.started);
                    view! {
                        <div class="tcn-setrow">
                            <div class="tcn-settext">
                                <div class="tcn-setname">"On the server"</div>
                                <div class="tcn-setdesc">
                                    {match stale {
                                        Some(true) => format!(
                                            "client {} — newer than the one this browser is \
                                             running, so this browser is holding an old copy",
                                            short(&s.client)),
                                        Some(false) => format!(
                                            "client {} — the same one, so this browser is \
                                             current", short(&s.client)),
                                        None => "the client it serves is not known".to_owned(),
                                    }}
                                </div>
                            </div>
                        </div>
                        <div class="tcn-setrow">
                            <div class="tcn-settext">
                                <div class="tcn-setname">"Built"</div>
                                <div class="tcn-setdesc">
                                    {if s.build == "dev" {
                                        "not by the pipeline — this is a build from somebody's \
                                         machine".to_owned()
                                    } else {
                                        let commit = if s.commit.is_empty() {
                                            String::new()
                                        } else {
                                            format!(" · commit {}", &s.commit[..s.commit.len().min(7)])
                                        };
                                        format!("{}{commit} · {}", s.build, s.build_type)
                                    }}
                                </div>
                            </div>
                        </div>
                        <div class="tcn-setrow">
                            <div class="tcn-settext">
                                <div class="tcn-setname">"Running since"</div>
                                <div class="tcn-setdesc">
                                    {started}
                                    " — a new image that nothing restarted is still the old
                                     server, and this is the line that says so."
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }}

                <div class="tcn-chips" style="margin-top:12px">
                    <button type="button" class="tcn-btn tcn-btn-primary"
                            on:click=move |_| update_now()>
                        "Throw away the cached copy and reload"
                    </button>
                    <button type="button" class="tcn-btn" on:click=move |_| check()>
                        "Ask the server again"
                    </button>
                </div>
            </div>
        </div>
    }
}
