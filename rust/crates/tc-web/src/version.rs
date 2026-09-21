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
//!
//! The date is back, in the mark in the header - but next to the answer to the first
//! question rather than instead of it. A colour saying "current" and a date saying *how*
//! current are two different facts, and the date on its own never was the second one.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
export async function tcw_update() {
    // Everything the browser is keeping of this app, in the order that matters: the caches
    // the service worker filled, then a fresh copy of the worker itself, then a reload that
    // has nothing left to serve it but the network.
    //
    // The worker is updated, never unregistered. Its registration is what the push
    // subscription belongs to, and unregistering deletes the subscription with it - so this
    // button used to switch off notifications for every tour, silently: the bells said "not
    // notified", which was by then true, and nothing said why. A fresh sw.js takes over at
    // once anyway (`skipWaiting` and `clients.claim`), so unregistering bought nothing.
    try {
        if (window.caches) {
            const keys = await caches.keys();
            await Promise.all(keys.map((k) => caches.delete(k)));
        }
    } catch (e) {}
    try {
        if (navigator.serviceWorker) {
            const regs = await navigator.serviceWorker.getRegistrations();
            await Promise.all(regs.map((r) => r.update()));
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

/// Whether the server hands out a newer client than this browser is running.
///
/// One question, one answer, two places that show it: the mark in the header asks, and the
/// bar under it - which is the one with a button on it - reads. Provided by the app, so
/// that neither of them has to know about the other.
#[derive(Clone, Copy)]
pub struct Stale(pub RwSignal<bool>);

/// What the mark in the header is saying.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    /// Nobody has answered yet - the first second or so after the app starts.
    Asking,
    /// This browser is running what the server hands out.
    Latest,
    /// The server has a newer client than this browser is running.
    Stale,
    /// The server did not answer, which is not the same as "current" and must not look
    /// like it.
    Unknown,
}

impl Mark {
    fn word(self) -> &'static str {
        match self {
            Mark::Asking => "checking…",
            // Only when there is no date to put there instead; see `word` in `VersionMark`.
            Mark::Latest => "latest",
            Mark::Stale => "update",
            Mark::Unknown => "can't tell",
        }
    }

    fn why(self) -> &'static str {
        match self {
            Mark::Asking => "Asking the server which client it hands out…",
            Mark::Latest => "This browser is running the client the server hands out. Click to ask again.",
            Mark::Stale => "The server hands out a newer client than this browser is running. Click to throw away the cached copy and reload.",
            Mark::Unknown => "The server did not say which client it hands out. Click to ask again.",
        }
    }
}

/// What a build stamp with no zone on it means.
///
/// Stamps written before 2026-09-21 are `YYYYMMDD-HHmmss` and nothing else, on +03:00 -
/// which was where the author lived, and which the stamp never said. They are still on the
/// dated tags of every image published until then, so a rollback still has to be readable;
/// new ones end in `Z` and need no guessing. Nothing new is ever written on this offset.
const BEFORE_THE_Z: &str = "+03:00";

/// The build stamp as an instant: `20260921-143012Z` → `2026-09-21T14:30:12Z`.
///
/// `None` for anything that is not that shape - a build from somebody's machine says `dev`.
pub fn build_iso(build: &str) -> Option<String> {
    let (stamp, zone) = match build.strip_suffix('Z') {
        Some(stamp) => (stamp, "Z"),
        None => (build, BEFORE_THE_Z),
    };
    let (date, time) = stamp.split_once('-')?;
    if date.len() != 8 || time.len() != 6 {
        return None;
    }
    if !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}{zone}",
        &date[..4],
        &date[4..6],
        &date[6..],
        &time[..2],
        &time[2..4],
        &time[4..],
    ))
}

/// An instant in milliseconds, or `None` when the string is not one.
fn moment(iso: &str) -> Option<f64> {
    let millis = js_sys::Date::parse(iso);
    (!millis.is_nan()).then_some(millis)
}

/// When the running build was built, and failing that, when the server started.
///
/// Two different answers to "how old is this?", and the second is the weaker one: a restart
/// is not a deploy. It is here because a server built by hand has no build stamp at all, and
/// "since 14:41" still beats saying nothing.
fn built_at(server: &Deployment) -> Option<f64> {
    build_iso(&server.build)
        .as_deref()
        .and_then(moment)
        .or_else(|| moment(&server.started))
}

/// The date the header says when this browser is current: "21.09 14:30", in the reader's own
/// clock.
///
/// Without the year, which is the one part a reader of the *latest* build never needs, and
/// the header has no room for.
fn short_stamp(millis: f64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(millis));
    format!(
        "{:02}.{:02} {:02}:{:02}",
        d.get_date(),
        d.get_month() + 1,
        d.get_hours(),
        d.get_minutes()
    )
}

/// How long the mark says its word out loud before shrinking back to a dot. Long enough to
/// be read by somebody who has just opened the app, short enough not to sit in the way.
const SAID: std::time::Duration = std::time::Duration::from_secs(3);

/// Whether this browser is current, in the corner of the header.
///
/// A dot, always there, in the app's three colours: green for current, amber for "there is
/// a newer one", grey for "the server did not answer". It opens into a word when the answer
/// first arrives - opening the app is when the question is asked - and shrinks back to the
/// dot, except when there is something to do about it. Before this, the only answer the app
/// gave was the amber bar below, which said nothing at all when everything was in order.
#[component]
pub fn VersionMark() -> impl IntoView {
    let mark = RwSignal::new(Mark::Asking);
    let open = RwSignal::new(false);
    // What the server said last time it was asked, kept so that the mark can say *when* this
    // build was built rather than the word "latest" - the colour of the dot already says
    // that much, and a date says something the colour cannot.
    let server = RwSignal::new(None::<Deployment>);
    // Read here and not inside the check: `use_context` answers for the owner that is
    // running, and by the time the server has answered there is none - the bar would never
    // have heard a thing.
    let bar = use_context::<Stale>().map(|s| s.0);
    // Which question this answer belongs to, so that a timer from an old one cannot shut a
    // newer answer's word.
    let asked = StoredValue::new(0u32);

    let check = move || {
        let n = asked.get_value() + 1;
        asked.set_value(n);
        spawn_local(async move {
            let running = running_client();
            let said = deployment().await;
            let now = match &said {
                Some(there) => match is_stale(there, &running) {
                    Some(true) => Mark::Stale,
                    Some(false) => Mark::Latest,
                    None => Mark::Unknown,
                },
                None => Mark::Unknown,
            };
            if asked.try_get_value() != Some(n) {
                return;
            }
            server.try_set(said);
            // Said out loud when it is news: the first answer, or one that differs from what
            // the dot has been showing. Coming back to the tab every few minutes should not
            // set the header talking each time.
            let news = mark.try_get_untracked() != Some(now);
            mark.try_set(now);
            if let Some(bar) = bar {
                bar.try_set(now == Mark::Stale);
            }
            if news {
                open.try_set(true);
                set_timeout(
                    move || {
                        // "Update" stays open: it is the one that asks for something.
                        if asked.try_get_value() == Some(n)
                            && mark.try_get_untracked() != Some(Mark::Stale)
                        {
                            open.try_set(false);
                        }
                    },
                    SAID,
                );
            }
        });
    };
    check();

    // And again whenever somebody comes back to the tab, for the reason the bar below gives.
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let on_visible = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_: web_sys::Event| {
                let visible = web_sys::window()
                    .and_then(|w| w.document())
                    .is_some_and(|d| !d.hidden());
                if visible {
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

    // What the mark opens into. When this browser is current, that is the date of the build
    // it is running: "latest" repeats what the green dot has already said, and a date answers
    // the question a reader actually has - is this from just now, or from Tuesday?
    let word = move || match mark.get() {
        Mark::Latest => server
            .with(|s| s.as_ref().and_then(built_at))
            .map(short_stamp)
            .unwrap_or_else(|| Mark::Latest.word().to_owned()),
        other => other.word().to_owned(),
    };
    // The long form, for the tooltip: both dates in full, because "built" and "running since"
    // are different questions and the short word only has room for one of them.
    let why = move || {
        let detail = server.with(|s| {
            let Some(s) = s.as_ref() else {
                return String::new();
            };
            let built = build_iso(&s.build).as_deref().and_then(moment);
            let started = moment(&s.started);
            match (built, started) {
                (Some(b), Some(r)) => format!(
                    " Built {}, running since {}.",
                    crate::ui::local_stamp(b),
                    crate::ui::local_stamp(r)
                ),
                (Some(b), None) => format!(" Built {}.", crate::ui::local_stamp(b)),
                (None, Some(r)) => format!(
                    " Not built by the pipeline; running since {}.",
                    crate::ui::local_stamp(r)
                ),
                (None, None) => String::new(),
            }
        });
        format!("{}{detail}", mark.get().why())
    };

    view! {
        <button type="button" class="tcw-vmark"
                class:is-latest=move || mark.get() == Mark::Latest
                class:is-stale=move || mark.get() == Mark::Stale
                class:is-unknown=move || matches!(mark.get(), Mark::Unknown | Mark::Asking)
                class:is-open=move || open.get()
                title=why
                aria-label=why
                on:click=move |_| {
                    if mark.get_untracked() == Mark::Stale {
                        update_now();
                    } else {
                        // Say the answer out loud again, whatever it turns out to be.
                        mark.set(Mark::Asking);
                        open.set(true);
                        check();
                    }
                }>
            <span class="tcw-vmark-dot"></span>
            <span class="tcw-vmark-word">{word}</span>
        </button>
    }
}

/// The bar that finds you: one line, only when there is something to do about it.
#[component]
pub fn UpdateBar() -> impl IntoView {
    // What the mark in the header found out; it is the one that asks.
    let stale = use_context::<Stale>()
        .map(|s| s.0)
        .unwrap_or_else(|| RwSignal::new(false));

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
                                // Named as what it is. Both this and the commit below are
                                // seven or eight hex characters, and "client bc0e7da" invites
                                // exactly one question: why is that not the commit I pushed?
                                Some(name) => format!(
                                    "client file {} — a hash of the compiled client, not a commit",
                                    short(&name)
                                ),
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
                    let started = crate::ui::local_stamp_of(&s.started);
                    view! {
                        <div class="tcn-setrow">
                            <div class="tcn-settext">
                                <div class="tcn-setname">"On the server"</div>
                                <div class="tcn-setdesc">
                                    {match stale {
                                        Some(true) => format!(
                                            "client file {} — newer than the one this browser \
                                             is running, so this browser is holding an old copy",
                                            short(&s.client)),
                                        Some(false) => format!(
                                            "client file {} — the same one, so this browser is \
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
                                        // The stamp is kept as it is written - it is half of
                                        // the name of the image tag, and somebody rolling a
                                        // deploy back needs to type it. The date in front of
                                        // it is the same moment on the reader's own clock,
                                        // which is the one they can compare with "just now".
                                        let when = build_iso(&s.build)
                                            .as_deref()
                                            .and_then(moment)
                                            .map(crate::ui::local_stamp)
                                            .map(|w| format!("{w} · "))
                                            .unwrap_or_default();
                                        format!("{when}{}{commit} · {}", s.build, s.build_type)
                                    }}
                                </div>
                            </div>
                        </div>
                        <div class="tcn-setrow">
                            <div class="tcn-settext">
                                <div class="tcn-setname">"Running since"</div>
                                <div class="tcn-setdesc">
                                    {started}
                                    " — when this container last started. If the build above
                                     is not the one you pushed, nothing here has been
                                     restarted with it yet."
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_stamp_becomes_an_instant() {
        assert_eq!(
            build_iso("20260921-143012Z").as_deref(),
            Some("2026-09-21T14:30:12Z")
        );
    }

    #[test]
    fn a_stamp_from_before_the_z_is_read_on_the_offset_it_was_written_on() {
        // Every image published up to 2026-09-21 is tagged this way, and rolling one back
        // must not move its date by three hours.
        assert_eq!(
            build_iso("20260910-150000").as_deref(),
            Some("2026-09-10T15:00:00+03:00")
        );
    }

    #[test]
    fn anything_that_is_not_a_build_stamp_has_no_date() {
        // A build from somebody's machine, and a few shapes that are nearly right.
        for not_a_stamp in ["dev", "", "20260921", "2026092-1143012", "2026092x-143012"] {
            assert_eq!(build_iso(not_a_stamp), None, "{not_a_stamp}");
        }
    }
}
