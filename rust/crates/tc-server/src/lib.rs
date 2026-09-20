//! Tourcalc's server.
//!
//! Split into a library with a thin binary on top so that the tests can build a router and
//! talk to it directly, without a socket or a browser.

pub mod api;
pub mod auth;
pub mod config;
pub mod lock;
pub mod fields;
#[cfg(feature = "mongo")]
pub mod mongo;
/// The words for a change, shared with the browser client - see [`tc_core::news`].
pub use tc_core::news;
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
        // With a throwaway parameter on the end, because the browser's own cache can still
        // be holding the page the old worker was built around - and this navigation would
        // read it. The app splits the query off the path, so it lands where it would have.
        const fresh = w.url + (w.url.includes('?') ? '&' : '?') + 'sw=' + Date.now();
        w.navigate(fresh);
    }
})()));
"#;

/// How long a browser may keep what it just asked for.
///
/// Two answers, and the file name decides which. The build renames what it compiles after
/// its own contents - `tc-web-163cb1d90a8bff90_bg.wasm` - so that file can never change
/// meaning and may be kept for a year. Everything else keeps its name from one build to the
/// next: the page, the manifest, the icons, the service worker. Those must be revalidated,
/// or a browser that has been here before goes on using last year's copy - which is exactly
/// how a manifest from the *previous* client survived a server being replaced under it.
///
/// `no-cache` does not mean "do not store": it means "ask before using", which with a
/// `Last-Modified` is one cheap 304.
pub fn cache_for(path: &str, is_api: bool, is_document: bool) -> &'static str {
    if is_api || is_document {
        return "no-cache";
    }
    if named_after_its_contents(path) {
        return "public, max-age=31536000, immutable";
    }
    "no-cache"
}

/// Whether a file's name carries a hash of what is in it, the way the build writes them:
/// `something-<at least eight hex digits>.<extension>`.
fn named_after_its_contents(path: &str) -> bool {
    let Some(name) = path.rsplit('/').next() else {
        return false;
    };
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    if extension.is_empty() || !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let Some((_, hash)) = stem.rsplit_once('-') else {
        return false;
    };
    // wasm-bindgen writes the module as `<name>-<hash>_bg.wasm`, so the hash is not always
    // the end of the name.
    let hash = hash.strip_suffix("_bg").unwrap_or(hash);
    hash.len() >= 8 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Whether this build should wear the beta icon.
///
/// The build stamps `BUILD_TYPE` into the image - `betaR (branch)` off a beta branch,
/// `prodR (prod)` off prod - and it is already read, already handed to the client and
/// already on the build page. So which icon to serve is a question that has been answered
/// before it was asked: no second build argument, no second manifest, nothing for the page
/// to know. A beta container serves `dist/beta/favicon.svg` at `/favicon.svg`, and every
/// address on the page stays what it was.
///
/// `na` is a build nobody stamped - a developer's own. Those get the real icon, and
/// `BUILD_TYPE=beta ./rust/build-and-run.sh` is how to see the other one.
pub fn wears_the_beta_skin(build_type: &str) -> bool {
    build_type.trim_start().to_ascii_lowercase().starts_with("beta")
}

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
mod beta_tests {
    use super::wears_the_beta_skin;

    #[test]
    fn only_a_beta_build_wears_the_beta_icon() {
        // What the workflow actually stamps, both ways round.
        assert!(wears_the_beta_skin("betaR (beta/save-tour-in-help)"));
        assert!(!wears_the_beta_skin("prodR (prod)"));
        // A developer's own build, and somebody setting it by hand to have a look.
        assert!(!wears_the_beta_skin("na"));
        assert!(wears_the_beta_skin("beta"));
        assert!(wears_the_beta_skin("Beta"));
        // "prod" is not "beta" however it is written, and neither is nothing.
        assert!(!wears_the_beta_skin(""));
        assert!(!wears_the_beta_skin("prod-beta"));
    }
}

#[cfg(test)]
mod cache_tests {
    use super::cache_for;

    #[test]
    fn what_the_build_renames_may_be_kept_and_the_rest_may_not() {
        for kept in [
            "/tc-web-163cb1d90a8bff90_bg.wasm",
            "/tc-web-163cb1d90a8bff90.js",
            "/newui-4dc956e92343512b.css",
        ] {
            assert_eq!(
                cache_for(kept, false, false),
                "public, max-age=31536000, immutable",
                "{kept} is named after its contents"
            );
        }

        // The ones that keep their names. A stale manifest is not a cosmetic problem: it is
        // what a browser reads to decide what the installed app is called and what its icon
        // is, and it had one from the previous client for as long as its own cache allowed.
        for asked in [
            "/manifest.json",
            "/sw.js",
            "/favicon.svg",
            "/icon-192.png",
            "/icon-180.png",
        ] {
            assert_eq!(cache_for(asked, false, false), "no-cache", "{asked}");
        }

        assert_eq!(cache_for("/api/Tour/abc", true, false), "no-cache");
        assert_eq!(cache_for("/", false, true), "no-cache");
        // Eight hex digits at least, and it has to look like a name and an extension.
        assert_eq!(cache_for("/snippets/inline-abc123.js", false, false), "no-cache");
        assert_eq!(cache_for("/some-file.png", false, false), "no-cache");
    }
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
