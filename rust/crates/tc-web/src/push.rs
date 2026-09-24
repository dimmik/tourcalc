//! Asking the browser to notify this device when a tour changes.
//!
//! Three parties and none of them trusts the others, which is why this is more than a
//! button. The browser makes a subscription - an endpoint at a push service plus two keys -
//! after the reader grants permission. The server is told about it and encrypts for those
//! keys, so the push service carries what it cannot read. The service worker receives the
//! message and shows it, because a page that is closed cannot.
//!
//! What this file does is the first part: permission, the subscription, and telling the
//! server. The encryption is `tc-server`'s and the showing is `sw.js`'s.

use crate::i18n::t;
use crate::api;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Whether this browser can do push at all, and whether it is already on for this tour.
#[derive(Clone, Copy, PartialEq)]
enum Bell {
    /// No service worker or no push support - an old browser, or an insecure origin.
    Impossible,
    Off,
    On,
    /// Finding out what is true - the state the button starts in, before the browser and the
    /// server have been asked.
    Checking,
    /// Asked, and no answer: the server is down or did not reply in time. Not the same as
    /// off, and it must not look like it - a reader who sees "off" believes it.
    Unknown,
    /// Turning it on or off right now.
    Working,
    /// The browser, or a server with no keys, said no.
    Refused,
}

/// How long a question about notifications may take before the answer is "can't tell".
/// A server that is down behind a proxy usually fails at once; one that hangs would leave
/// a spinner, or an old answer, looking like the truth for as long as it hangs.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(5);

/// Why turning notifications on or off did not happen.
enum Failure {
    /// Somebody said no: the browser, the reader, or a server without keys.
    Refused(String),
    /// Nobody answered, so what is true now is not known.
    NoAnswer(String),
}

#[component]
pub fn PushBell(tour_id: String) -> impl IntoView {
    let state = RwSignal::new(Bell::Checking);
    // Which question is the latest: a timer for an old one must not overrule a newer answer.
    let asked = StoredValue::new(0u32);
    let tour_of = StoredValue::new(tour_id);

    // What is true: does this browser support it, and is this device subscribed to this
    // tour? Asked when the page opens, and again when a "can't tell" is clicked.
    let check = move || {
        let id = tour_of.get_value();
        let n = asked.get_value() + 1;
        asked.set_value(n);
        state.set(Bell::Checking);
        spawn_local(async move {
            let found = match existing_subscription().await {
                None if supported() => Bell::Off,
                None => Bell::Impossible,
                Some(sub) => match api::push_check(&id, &sub).await {
                    Ok(known) => {
                        // What the tour page found out is what the list should show next.
                        rings_for(&id, known);
                        if known {
                            Bell::On
                        } else {
                            Bell::Off
                        }
                    }
                    Err(why) => {
                        leptos::logging::warn!("push: could not check: {why}");
                        Bell::Unknown
                    }
                },
            };
            if asked.try_get_value() == Some(n) {
                state.try_set(found);
            }
        });
        set_timeout(
            move || {
                if asked.try_get_value() == Some(n)
                    && state.try_get_untracked() == Some(Bell::Checking)
                {
                    state.try_set(Bell::Unknown);
                }
            },
            PATIENCE,
        );
    };
    check();

    let click = move |_| {
        let was = state.get();
        if matches!(was, Bell::Unknown) {
            check();
            return;
        }
        let id = tour_of.get_value();
        let n = asked.get_value() + 1;
        asked.set_value(n);
        state.set(Bell::Working);
        spawn_local(async move {
            let outcome = match was {
                Bell::On => turn_off(&id).await.map(|()| Bell::Off),
                _ => turn_on(&id).await.map(|()| Bell::On),
            };
            if let Ok(now) = &outcome {
                rings_for(&id, *now == Bell::On);
            }
            let next = match outcome {
                Ok(next) => next,
                Err(Failure::Refused(why)) => {
                    leptos::logging::warn!("push: {why}");
                    Bell::Refused
                }
                Err(Failure::NoAnswer(why)) => {
                    leptos::logging::warn!("push: {why}");
                    Bell::Unknown
                }
            };
            if asked.try_get_value() == Some(n) {
                state.try_set(next);
            }
        });
    };

    view! {
        <Show when=move || state.get() != Bell::Impossible>
            // The app's own pill, and its own faces: an outline while it is off, solid white
            // once you are subscribed, dashed while what is true is not known.
            <button type="button" class="tcn-bell"
                    class:is-on=move || state.get() == Bell::On
                    class:is-unknown=move || matches!(state.get(), Bell::Checking | Bell::Unknown)
                    prop:disabled=move || matches!(state.get(), Bell::Working | Bell::Checking)
                    title=move || match state.get() {
                        Bell::On => t().device.bell_on_hint,
                        Bell::Off => t().device.bell_off_hint,
                        Bell::Checking => t().device.bell_checking_hint,
                        Bell::Unknown => t().device.bell_unknown_hint,
                        Bell::Refused => t().device.bell_refused_hint,
                        Bell::Working | Bell::Impossible => "",
                    }
                    on:click=click>
                {move || match state.get() {
                    Bell::On => t().device.bell_on,
                    Bell::Off => t().device.bell_off,
                    Bell::Checking => t().device.bell_checking,
                    Bell::Unknown => t().device.bell_unknown,
                    Bell::Working => "⏳ …",
                    Bell::Refused => t().device.bell_refused,
                    Bell::Impossible => "",
                }}
            </button>
        </Show>
    }
}

// --- the bells in the tour list ----------------------------------------------------------

/// Which tours this browser is subscribed to, as far as the list knows: `None` until it has
/// found out, then the ids that ring - every other tour is known not to.
pub type Bells = RwSignal<Option<Vec<String>>>;

/// What one row of the list says about notifications on this device.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ring {
    /// Not found out: the browser or the server has still to answer, or could not.
    Unknown,
    Off,
    On,
}

pub fn ring_of(known: Option<&[String]>, tour: &str) -> Ring {
    match known {
        None => Ring::Unknown,
        Some(tours) if tours.iter().any(|t| t == tour) => Ring::On,
        Some(_) => Ring::Off,
    }
}

/// The list's bells: the last answer at once, then the real one - or "can't tell" when the
/// question fails or takes longer than [`PATIENCE`]. The last answer is only a guess about
/// now, and a stopped server must not leave it looking like one it just gave.
pub fn list_bells() -> Bells {
    let bells: Bells = RwSignal::new(remembered_bells());
    let answered = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let answered = answered.clone();
        spawn_local(async move {
            let found = subscribed_tours().await;
            answered.set(true);
            // `try_`: the reader may have opened a tour before the answer came.
            bells.try_set(found);
        });
    }
    set_timeout(
        move || {
            if !answered.get() {
                bells.try_set(None);
            }
        },
        PATIENCE,
    );
    bells
}

/// The bell after a tour's name in the list, in both interfaces.
#[component]
pub fn ListBell(bells: Bells, tour: String) -> impl IntoView {
    let ring = Memo::new(move |_| bells.with(|b| ring_of(b.as_deref(), &tour)));
    view! {
        <span class="tcw-bell"
              class:is-on=move || ring.get() == Ring::On
              class:is-off=move || ring.get() == Ring::Off
              class:is-unknown=move || ring.get() == Ring::Unknown
              title=move || match ring.get() {
                  Ring::On => t().device.ring_on,
                  Ring::Off => t().device.ring_off,
                  Ring::Unknown => t().device.ring_unknown,
              }>
            {move || if ring.get() == Ring::Off { "🔕" } else { "🔔" }}
        </span>
    }
}

/// The last answer, so the list draws its bells with the rows instead of a moment after.
/// The key being there at all is what says "found out": an empty list is a real answer.
const BELLS_KEY: &str = "__tcw_bells";

/// What the list shows before it has asked: the answer it got last time, if it ever got one.
pub fn remembered_bells() -> Option<Vec<String>> {
    storage()
        .and_then(|s| s.get_item(BELLS_KEY).ok().flatten())
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn remember_bells(tours: &[String]) {
    let Some(s) = storage() else { return };
    if let Ok(text) = serde_json::to_string(tours) {
        let _ = s.set_item(BELLS_KEY, &text);
    }
}

/// On the way out, with the list: the ids are the reader's.
pub fn forget_bells() {
    if let Some(s) = storage() {
        let _ = s.remove_item(BELLS_KEY);
    }
}

/// What the tour page found out about one tour, into the list's answer.
///
/// Only into an answer the list already has: one tour checked says nothing about the rest,
/// and starting a list from it would mark every other tour as not notified.
fn rings_for(tour: &str, on: bool) {
    let Some(mut bells) = remembered_bells() else {
        return;
    };
    bells.retain(|t| t != tour);
    if on {
        bells.push(tour.to_owned());
    }
    remember_bells(&bells);
}

/// Which of the reader's tours this browser is subscribed to.
///
/// A browser that never subscribed - most of them - finds that out from itself and asks the
/// server nothing. One that did asks once, for the whole list. `None` when the answer could
/// not be had: the list then says it cannot tell, and the remembered answer stays for next
/// time.
pub async fn subscribed_tours() -> Option<Vec<String>> {
    if !supported() {
        return Some(Vec::new());
    }
    let tours = match existing_subscription().await {
        None => Vec::new(),
        Some(sub) => api::push_mine(&sub).await.ok()?,
    };
    remember_bells(&tours);
    Some(tours)
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn supported() -> bool {
    web_sys::window().is_some_and(|w| {
        js_sys::Reflect::has(&w.navigator(), &"serviceWorker".into()).unwrap_or(false)
            && js_sys::Reflect::has(&w, &"PushManager".into()).unwrap_or(false)
    })
}

/// The push manager of the registered service worker, if there is one.
async fn manager() -> Option<web_sys::PushManager> {
    let window = web_sys::window()?;
    let ready = window.navigator().service_worker().ready().ok()?;
    let registration: web_sys::ServiceWorkerRegistration =
        JsFuture::from(ready).await.ok()?.dyn_into().ok()?;
    registration.push_manager().ok()
}

/// The subscription this browser already has for this origin, if any.
async fn existing_subscription() -> Option<api::PushSubscription> {
    let manager = manager().await?;
    let existing = JsFuture::from(manager.get_subscription().ok()?)
        .await
        .ok()?;
    if existing.is_null() || existing.is_undefined() {
        return None;
    }
    read_subscription(&existing)
}

async fn turn_on(tour_id: &str) -> Result<(), Failure> {
    let manager = manager()
        .await
        .ok_or_else(|| Failure::Refused("no service worker".into()))?;

    // The server's VAPID public key: the browser encrypts to it, so a notification can only
    // come from whoever holds the other half.
    let key = api::push_public_key().await.map_err(|e| Failure::NoAnswer(e.to_string()))?;
    if key.trim().is_empty() {
        return Err(Failure::Refused(
            "this server has no notification keys configured".into(),
        ));
    }

    let subscription = match existing_subscription().await {
        Some(sub) => sub,
        None => {
            let options = web_sys::PushSubscriptionOptionsInit::new();
            // Promising the notification will be shown to the reader. Chrome refuses a
            // subscription without it, and silent push is not what this is for anyway.
            options.set_user_visible_only(true);
            let bytes =
                js_sys::Uint8Array::from(base64url(&key).map_err(Failure::Refused)?.as_slice());
            options.set_application_server_key(&bytes);
            let promise = manager
                .subscribe_with_options(&options)
                .map_err(|e| Failure::Refused(format!("could not subscribe: {e:?}")))?;
            let value = JsFuture::from(promise)
                .await
                .map_err(|_| Failure::Refused("the browser refused notifications".into()))?;
            read_subscription(&value).ok_or_else(|| {
                Failure::Refused("the subscription came back in an odd shape".into())
            })?
        }
    };

    api::push_subscribe(tour_id, &subscription)
        .await
        .map_err(|e| Failure::NoAnswer(e.to_string()))
}

async fn turn_off(tour_id: &str) -> Result<(), Failure> {
    let Some(subscription) = existing_subscription().await else {
        return Ok(());
    };
    // Told to the server first: the browser's own subscription is shared by every tour on
    // this origin, so it is dropped only when nothing wants it.
    api::push_unsubscribe(tour_id, &subscription)
        .await
        .map_err(|e| Failure::NoAnswer(e.to_string()))
}

/// Reads the browser's subscription object into the shape the server stores.
fn read_subscription(value: &wasm_bindgen::JsValue) -> Option<api::PushSubscription> {
    let json = js_sys::Reflect::get(value, &"toJSON".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .and_then(|f| f.call0(value).ok())?;

    let endpoint = js_sys::Reflect::get(&json, &"endpoint".into())
        .ok()?
        .as_string()?;
    let keys = js_sys::Reflect::get(&json, &"keys".into()).ok()?;
    let get = |name: &str| -> String {
        js_sys::Reflect::get(&keys, &name.into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default()
    };

    Some(api::PushSubscription {
        url: endpoint,
        p256dh: get("p256dh"),
        auth: get("auth"),
    })
}

/// base64url without padding, which is how a VAPID key is written down.
fn base64url(text: &str) -> Result<Vec<u8>, String> {
    let mut cleaned: String = text
        .trim()
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    while !cleaned.len().is_multiple_of(4) {
        cleaned.push('=');
    }
    let window = web_sys::window().ok_or("no window")?;
    let raw = window
        .atob(&cleaned)
        .map_err(|_| "the server's key is not base64".to_owned())?;
    Ok(raw.chars().map(|c| c as u8).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Not having asked is not the same as having been told no.
    #[test]
    fn a_bell_is_unknown_until_the_answer_and_off_after_it() {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(ring_of(None, "a"), Ring::Unknown);
        assert_eq!(ring_of(Some(&ids(&[])), "a"), Ring::Off);
        assert_eq!(ring_of(Some(&ids(&["a", "b"])), "a"), Ring::On);
        assert_eq!(ring_of(Some(&ids(&["b"])), "a"), Ring::Off);
    }
}
