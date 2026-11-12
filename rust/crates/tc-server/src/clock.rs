//! The time now, wherever the server runs.
//!
//! `std::time::SystemTime::now()` panics on `wasm32-unknown-unknown` - there is no clock
//! below the standard library there - and the same router runs in a Cloudflare Worker
//! (`cloudflare/worker`). JavaScript has one, so that is what a Worker asks. Everything else
//! about `SystemTime` (adding, comparing, `UNIX_EPOCH`) works on that target as it is.

use std::time::SystemTime;

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> SystemTime {
    SystemTime::now()
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_millis(js_sys::Date::now() as u64)
}

/// Runs a task beside the request that started it: on tokio's pool natively, on the
/// JavaScript event loop in a Worker (where it lives as long as the request's context does).
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(task: impl std::future::Future<Output = ()> + Send + 'static) {
    tokio::spawn(task);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn(task: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(task);
}
