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
