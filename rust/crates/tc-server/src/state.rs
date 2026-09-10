//! What every request handler is given.
//!
//! One of these exists for the life of the process and every connection shares it, which is
//! why it is behind an `Arc` and why everything inside it has to be `Send + Sync`. On the
//! browser side the same idea is spelled `Rc` - one thread, no atomics needed - and the
//! compiler is what keeps the two from being confused: an `Rc` has no `Send`, so it cannot
//! reach a thread pool even by accident.

use crate::auth::Signer_;
use crate::store::TourStore;
use std::sync::{Arc, RwLock};

pub struct AppState {
    pub store: Box<dyn TourStore>,
    /// Who has asked to be told when a tour changes, and what does the telling.
    pub subscriptions: Box<dyn crate::subscriptions::SubscriptionStore>,
    pub push: Box<dyn crate::push::Notifier>,
    pub signer: Signer_,
    pub master_key: String,
    pub token_valid_minutes: i64,
    pub started: std::time::SystemTime,

    /// The settings a handler needs to answer a request. Copied out of [`crate::config`] at
    /// startup rather than read from the environment per request: configuration that can
    /// change under a running server is configuration nobody can reason about.
    pub max_tours_per_code: i64,
    /// Whether saving keeps the state it replaced, and whether a kept state may be written
    /// to. Both are the C#'s switches, with its defaults.
    pub versioning: bool,
    pub version_editable: bool,
    pub wakeup_code: String,
    pub wakeup_pre_delay_min: u64,
    pub wakeup_post_delay_min: u64,

    /// What this deployment calls itself, and the client it is serving: the file name of
    /// the wasm named by index.html. The name carries a hash of the contents, so it is the
    /// one identifier that cannot be out of step with what is actually there - and it is
    /// what a browser compares its own against to know whether it is holding an old copy.
    pub build_type: String,
    pub build_id: String,
    pub build_commit: String,
    pub client_asset: Option<String>,

    /// When the server was last woken, newest last. The C# keeps the same short list and
    /// shows it on the startup-info page.
    pub wakeups: RwLock<Vec<std::time::SystemTime>>,
}

/// How many wake-up times are worth keeping. The C#'s number, and for the same reason:
/// it is a diagnostic, not a history.
pub const WAKEUPS_TO_KEEP: usize = 15;

impl AppState {
    /// Tells whoever subscribed to this tour that it changed.
    ///
    /// Best-effort and out of the way of the save: the tour is already stored by the time
    /// this runs, and a push service that is slow or down must not make somebody's edit
    /// slow or failed.
    pub fn announce(self: &Arc<Self>, tour_id: &str, message: String) {
        let subscribers = self.subscriptions.for_tour(tour_id);
        if subscribers.is_empty() {
            return;
        }
        let state = Arc::clone(self);
        let tour_id = tour_id.to_owned();
        tokio::spawn(async move {
            state.push.notify(subscribers, &tour_id, &message).await;
        });
    }

    /// Records a wake-up, dropping the oldest when the list is full.
    pub fn woke_up(&self) {
        let Ok(mut list) = self.wakeups.write() else {
            return;
        };
        if list.len() >= WAKEUPS_TO_KEEP {
            list.remove(0);
        }
        list.push(std::time::SystemTime::now());
    }
}

pub type Shared = Arc<AppState>;
