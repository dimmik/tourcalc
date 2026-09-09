//! What every request handler is given.
//!
//! One of these exists for the life of the process and every connection shares it, which is
//! why it is behind an `Arc` and why everything inside it has to be `Send + Sync`. On the
//! browser side the same idea is spelled `Rc` - one thread, no atomics needed - and the
//! compiler is what keeps the two from being confused: an `Rc` has no `Send`, so it cannot
//! reach a thread pool even by accident.

use crate::auth::Signer_;
use crate::store::TourStore;
use std::sync::Arc;

pub struct AppState {
    pub store: Box<dyn TourStore>,
    pub signer: Signer_,
    pub master_key: String,
    pub token_valid_minutes: i64,
    pub started: std::time::SystemTime,
}

pub type Shared = Arc<AppState>;
