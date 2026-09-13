//! Tourcalc's server.
//!
//! Split into a library with a thin binary on top so that the tests can build a router and
//! talk to it directly, without a socket or a browser.

pub mod api;
pub mod auth;
pub mod config;
pub mod fields;
#[cfg(feature = "mongo")]
pub mod mongo;
pub mod push;
pub mod state;
pub mod store;
pub mod subscriptions;
pub mod text;
pub mod versions;

/// The address the C# client's service worker lives at, answered with one that removes
/// itself.
///
/// A browser that ever opened the Blazor app is still holding that worker, and it serves
/// its own cached `index.html` for every navigation - so the day this server takes over the
/// domain, a returning reader is handed the *old* app out of their own browser and nothing
/// on the network can talk them out of it. The worker only steps aside if the script at its
/// own address changes, and everything this server does not recognise is answered with the
/// app's page: HTML, where a script was expected, which the browser refuses - leaving the
/// old worker exactly where it was.
///
/// So this address answers with a real script whose whole job is to unregister itself and
/// reload whatever windows it has. After that the page loads from the network, registers
/// this client's own worker (`/sw.js`), and the changeover is done.
///
/// Its own caches are left alone - `tcw-` is this client's prefix - so the swap costs the
/// reader nothing but one reload. Safe to delete once no browser can still be holding a
/// Blazor worker, which is to say: not soon.
pub async fn retire_the_old_service_worker() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        RETIRE_SCRIPT,
    )
}

const RETIRE_SCRIPT: &str = r#"// This is not a service worker. It is how the one that used
// to live here is taken off the air: see `retire_the_old_service_worker` in the server.
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil((async () => {
    await self.registration.unregister();
    const names = await caches.keys();
    // Not the ones belonging to the client that is taking over.
    await Promise.all(names.filter((n) => !n.startsWith('tcw-')).map((n) => caches.delete(n)));
    const windows = await self.clients.matchAll({ type: 'window' });
    for (const w of windows) {
        w.navigate(w.url);
    }
})()));
"#;

/// The wasm file a built index.html names, hash and all.
///
/// Found from the `_bg.wasm` end and read backwards: the page names the glue script
/// (`tc-web-<hash>.js`) before the module, so looking forwards from the first `tc-web-`
/// and stopping at the first `.wasm` picks up the whole import statement in between - which
/// it did, and the endpoint reported it.
pub fn wasm_named_in(page: &str) -> Option<&str> {
    let end = page.find("_bg.wasm")? + "_bg.wasm".len();
    let start = page[..end].rfind("tc-web-")?;
    Some(&page[start..end])
}

#[cfg(test)]
mod version_tests {
    #[test]
    fn the_client_is_named_by_the_page_that_loads_it() {
        let page = r#"<script type="module">
import init, * as bindings from '/tc-web-7cacd9a9839aeea6.js';
const wasm = await init({ module_or_path: '/tc-web-7cacd9a9839aeea6_bg.wasm' });
</script>
<link rel="preload" href="/tc-web-7cacd9a9839aeea6_bg.wasm" as="fetch">"#;
        assert_eq!(
            super::wasm_named_in(page),
            Some("tc-web-7cacd9a9839aeea6_bg.wasm")
        );
        assert_eq!(super::wasm_named_in("<html>nothing here</html>"), None);
    }
}
