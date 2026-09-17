//! Finding out that somebody else changed the tour that is open here.
//!
//! A tour page used to learn about other people's edits only when it was reloaded, saved
//! to, or its refresh button was pressed - so two people at one table could look at two
//! different totals for as long as neither of them touched anything. Now the page asks
//! the server for the tour's `StateGUID` (a few bytes, see `GET /api/Tour/{id}/state`)
//! whenever it comes back into view, and every [`EVERY`] while it is on screen. A state
//! that is not the one this device last had means somebody saved.
//!
//! What happens then depends on what the reader is doing:
//!
//! 1. **Nothing in particular:** the tour is fetched and swapped in, with a line saying what
//!    changed and the touched expenses lit up for a moment.
//! 2. **A form is open:** nothing moves under it. A line says the tour changed and that
//!    what they save goes on top; the swap waits for the form to close.
//! 3. **Edits of their own are stuck in the queue:** that queue is sent, the ordinary way,
//!    which takes the other person's tour as its base.
//!
//! **This never sends anything on its own account.** Two sends of one queue at once can
//! each replay the same edit onto the other's result, so a watcher that sent would be a
//! way to add an expense twice. It fetches, and it keeps out of the way of any save or
//! load already in flight.

use crate::api;
use crate::queue;
use crate::sync::Status;
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::Tour;

/// How often an open, visible tour asks whether it is still current.
pub const EVERY: std::time::Duration = std::time::Duration::from_secs(25);

/// How long the touched expenses stay lit, and how long the line about them stays up.
const LIT: std::time::Duration = std::time::Duration::from_secs(4);
const SAID: std::time::Duration = std::time::Duration::from_secs(12);

#[derive(Clone, Copy)]
pub struct Others {
    /// A form is open on this page.
    pub editing: RwSignal<bool>,
    /// A save of this reader's is on its way; nothing is asked meanwhile, since their own
    /// save moves the state too and would be taken for somebody else's.
    pub saving: RwSignal<u32>,
    /// A load is in flight.
    pub loading: RwSignal<bool>,
    /// Somebody else's change is waiting for the form to close.
    pub behind: RwSignal<bool>,
    /// What changed, in words, while it is on screen.
    pub news: RwSignal<Option<String>>,
    /// The expenses to point out, briefly.
    pub touched: RwSignal<Vec<String>>,
    /// Set by a fetch this module asked for: the copy that was on screen before it, so what
    /// changed can be said once the new one lands. Also what makes that load fetch-only.
    pub before: StoredValue<Option<Tour>>,
    /// The server's copy this page is showing, before anything queued is applied. What the
    /// server's state is compared with - not the copy kept in storage, which every tab of
    /// this browser shares: a save from the tab next door updates that one, and this page
    /// would then think it had the newest tour while still drawing the old one.
    base: StoredValue<Option<Tour>>,
    /// Which announcement is the latest, so an old timer does not take down a newer line.
    said: StoredValue<u32>,
    asking: StoredValue<bool>,
}

impl Others {
    pub fn new() -> Self {
        Self {
            editing: RwSignal::new(false),
            saving: RwSignal::new(0),
            loading: RwSignal::new(false),
            behind: RwSignal::new(false),
            news: RwSignal::new(None),
            touched: RwSignal::new(Vec::new()),
            before: StoredValue::new(None),
            base: StoredValue::new(None),
            said: StoredValue::new(0),
            asking: StoredValue::new(false),
        }
    }

    /// Whether the load that is starting was asked for here - and so must fetch and never
    /// send. Cleared when that load lands ([`Self::landed`]) or does not ([`Self::missed`]).
    pub fn fetch_only(self) -> bool {
        self.before.with_value(|b| b.is_some())
    }

    /// Starts asking. Stops by itself when the page goes.
    pub fn watch(self, id: String, load: Callback<bool>, status: RwSignal<Status>) {
        let look = {
            let id = id.clone();
            move || self.look(id.clone(), load, status)
        };

        if let Ok(handle) = leptos::prelude::set_interval_with_handle(
            {
                let look = look.clone();
                move || look()
            },
            EVERY,
        ) {
            on_cleanup(move || handle.clear());
        }

        // Coming back to the tab - unlocking the phone, switching back from a chat - is when
        // somebody is about to look at the numbers, so that is when to ask first.
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            use wasm_bindgen::JsCast;
            let on_visible = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new({
                let look = look.clone();
                move |_: web_sys::Event| look()
            })
            .into_js_value();
            let _ = document
                .add_event_listener_with_callback("visibilitychange", on_visible.unchecked_ref());
            let held = leptos::__reexports::send_wrapper::SendWrapper::new((document, on_visible));
            on_cleanup(move || {
                let (document, on_visible) = held.take();
                let _ = document.remove_event_listener_with_callback(
                    "visibilitychange",
                    on_visible.unchecked_ref(),
                );
            });
        }

        // The form closed with somebody else's change waiting: ask again now. If a save of
        // this reader's started as it closed, `look` waits for it.
        Effect::new(move |_| {
            if !self.editing.get() && self.behind.get_untracked() {
                look();
            }
        });
    }

    fn look(self, id: String, load: Callback<bool>, status: RwSignal<Status>) {
        let hidden = web_sys::window()
            .and_then(|w| w.document())
            .is_none_or(|d| d.hidden());
        // `try_` throughout: a retry below can fire after the reader has left the page.
        let (Some(asking), Some(saving), Some(loading)) = (
            self.asking.try_get_value(),
            self.saving.try_get_untracked(),
            self.loading.try_get_untracked(),
        ) else {
            return;
        };
        if hidden || asking {
            return;
        }
        // Something of this reader's is moving. Try again shortly rather than at the next
        // tick, if a change is known to be waiting.
        if saving > 0 || loading {
            if self.behind.try_get_untracked() == Some(true) {
                set_timeout(
                    move || self.look(id, load, status),
                    std::time::Duration::from_millis(1500),
                );
            }
            return;
        }
        let Some(had) = self.base.try_get_value().flatten() else {
            return;
        };
        let known = tc_core::extras::str_of(&had.extras, tc_core::extras::STATE);

        self.asking.try_set_value(true);
        spawn_local(async move {
            let now = api::tour_state(&id).await;
            self.asking.try_set_value(false);
            let Ok(now) = now else {
                // Offline, or the server is down: nothing to learn, and the freshness line
                // already says so when it matters.
                return;
            };

            if now.is_empty() || now == known {
                // Already current. A change that was waiting has been brought in by a save
                // of this reader's in the meantime, which replays onto the newest tour; the
                // line about it can still be said.
                if self.behind.try_get_untracked() == Some(true) {
                    self.behind.try_set(false);
                    if let Some(text) = self.news.try_get_untracked().flatten() {
                        self.announce(Some(text), Vec::new());
                    }
                }
                return;
            }

            // 3. Their own edits are stuck: send them, which is what the refresh button
            //    does. Only when the queue is stuck, not while a save is merely under way.
            if !queue::pending(&id).is_empty() {
                if matches!(
                    status.try_get_untracked(),
                    Some(Status::Waiting(_) | Status::Failed(_))
                ) {
                    load.run(false);
                }
                return;
            }

            // 2. A form is open: say so, and wait for it to close.
            if self.editing.try_get_untracked() == Some(true) {
                if self.behind.try_get_untracked() != Some(true) {
                    let text = match api::tour(&id).await {
                        Ok(fresh) => tc_core::news::what_changed(&had, &fresh),
                        Err(_) => None,
                    };
                    self.news.try_set(text);
                    self.behind.try_set(true);
                }
                return;
            }

            // 1. Swap it in, and say what changed once it has landed.
            self.behind.try_set(false);
            self.before.try_set_value(Some(had));
            load.run(false);
        });
    }

    /// The server's copy the page has just drawn from - on arrival from storage, and after
    /// every load.
    pub fn showing(self, base: &Tour) {
        self.base.try_set_value(Some(base.clone()));
    }

    /// Called by the load with the tour it just put on screen.
    pub fn landed(self, fresh: &Tour) {
        let Some(had) = self.before.try_update_value(|b| b.take()).flatten() else {
            return;
        };
        let text = tc_core::news::what_changed(&had, fresh);
        let touched: Vec<String> = tc_core::news::touched_spendings(&had, fresh)
            .into_iter()
            .map(|s| s.as_str().to_owned())
            .collect();
        // A change news.rs has no words for - a currency's rate, a date - is still a change.
        let text = text.or_else(|| Some("somebody saved a change".to_owned()));
        self.announce(text, touched);
    }

    /// A load this module asked for did not land a tour: forget what it was for.
    pub fn missed(self) {
        self.before.try_set_value(None);
    }

    fn announce(self, text: Option<String>, touched: Vec<String>) {
        let n = self.said.try_get_value().unwrap_or(0).wrapping_add(1);
        self.said.try_set_value(n);
        self.news.try_set(text);
        self.touched.try_set(touched);
        set_timeout(
            move || {
                if self.said.try_get_value() == Some(n) {
                    self.touched.try_set(Vec::new());
                }
            },
            LIT,
        );
        set_timeout(
            move || {
                if self.said.try_get_value() == Some(n) && self.behind.try_get_untracked() != Some(true)
                {
                    self.news.try_set(None);
                }
            },
            SAID,
        );
    }

    /// Whether this expense should be lit up right now.
    pub fn lit(self, spending: &str) -> bool {
        self.touched.with(|t| t.iter().any(|s| s == spending))
    }
}

/// The line about somebody else's change once it is on screen: over the page, clear of the
/// + Spend button, and gone by itself.
#[component]
pub fn OthersLine(others: Others) -> impl IntoView {
    view! {
        <Show when=move || others.news.get().is_some() && !others.behind.get()>
            <div class="tcw-others" role="status">
                <span class="tcw-others-text">
                    {move || format!("Updated: {}", others.news.get().unwrap_or_default())}
                </span>
                <button type="button" class="tcw-others-close" aria-label="Dismiss"
                        on:click=move |_| others.news.set(None)>"×"</button>
            </div>
        </Show>
    }
}

/// The same news while a form is open, at the top of the form: nothing has moved under it,
/// and what it saves goes on top of the change.
#[component]
pub fn WaitingLine(others: Others) -> impl IntoView {
    view! {
        <Show when=move || others.behind.get()>
            <div class="tcw-others-waiting" role="status">
                {move || match others.news.get() {
                    Some(w) => format!("Somebody changed this tour: {w}. What you save goes on top of it."),
                    None => "Somebody changed this tour. What you save goes on top of it.".to_owned(),
                }}
            </div>
        </Show>
    }
}
