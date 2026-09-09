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

    /// When the server was last woken, newest last. The C# keeps the same short list and
    /// shows it on the startup-info page.
    pub wakeups: RwLock<Vec<std::time::SystemTime>>,
}

/// How many wake-up times are worth keeping. The C#'s number, and for the same reason:
/// it is a diagnostic, not a history.
pub const WAKEUPS_TO_KEEP: usize = 15;

impl AppState {
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
