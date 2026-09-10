//! The help page.
//!
//! The app's own document, taken as it is - the same thing that happened with the two
//! stylesheets, and for the same reason. It is prose about what the buttons do, written
//! against what the app actually does rather than what it was meant to do, and rewriting it
//! from memory would have produced something less true and no shorter.
//!
//! Static markup, so it is included rather than built out of view macros: 250 lines of
//! `<section>` and `<p>` expressed as Rust would be harder to compare with the original,
//! which is the thing that has to stay true when either side changes.

use leptos::prelude::*;

#[component]
pub fn HelpPage() -> impl IntoView {
    view! {
        <div class="tcn-main" inner_html=include_str!("help.html")></div>
    }
}
