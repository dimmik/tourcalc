//! The help page.
//!
//! The app's own document, taken as it is - the same thing that happened with the two
//! stylesheets, and for the same reason. It is prose about what the buttons do, written
//! against what the app actually does rather than what it was meant to do, and rewriting it
//! from memory would have produced something less true and no shorter.
//!
//! One file per language (`help.html`, `help.ru.html`), picked by `i18n::lang`: prose is
//! translated as prose, not assembled from fields.
//!
//! Static markup, so it is included rather than built out of view macros: 250 lines of
//! `<section>` and `<p>` expressed as Rust would be harder to compare with the original,
//! which is the thing that has to stay true when either side changes.

use leptos::prelude::*;

#[component]
pub fn HelpPage() -> impl IntoView {
    view! {
        <div class="tcn-main" inner_html=match crate::i18n::lang() {
            crate::i18n::Lang::En => include_str!("help.html"),
            crate::i18n::Lang::Ru => include_str!("help.ru.html"),
        }></div>
        // Where the app kept its build date. The question it was there to answer is asked
        // properly here instead - see `crate::version`.
        <crate::version::AboutBuild />
        // What changed and why, newest first. Folded: it is for the reader who wonders
        // why something looks different today, not for everybody who opens Help. Kept as
        // markup for the same reason as the page above, and added to with every change a
        // reader could notice - see CLAUDE.md.
        <div class="tcn-main" inner_html=include_str!("changes.html")></div>
    }
}
