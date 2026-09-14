//! The icons, drawn rather than typed.
//!
//! An icon-only control must not depend on the system font. The app learned this from ⏻
//! (U+23FB), which has no coverage on Android and rendered as an empty box, leaving the
//! logout button blank. The same applies to every glyph this client was using for a caret or
//! a close button: they happen to work in a desktop browser and are somebody's tofu box
//! elsewhere.
//!
//! Same paths as the C#'s `TcIcon`, so both interfaces draw the same shapes at the same
//! weight.

use leptos::prelude::*;

#[component]
pub fn Icon(name: &'static str) -> impl IntoView {
    view! {
        <svg class="tcn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
             aria-hidden="true" focusable="false"
             inner_html=paths(name)>
        </svg>
    }
}

/// The shapes of one icon. Written as markup rather than as elements because they are
/// constants - there is nothing to bind, and a `match` over strings is the whole component.
fn paths(name: &str) -> &'static str {
    match name {
        "refresh" => {
            r#"<polyline points="23 4 23 10 17 10"/>
               <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>"#
        }
        "logout" => {
            r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
               <polyline points="16 17 21 12 16 7"/>
               <line x1="21" y1="12" x2="9" y2="12"/>"#
        }
        "settings" => {
            r#"<circle cx="12" cy="12" r="3"/>
               <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>"#
        }
        "edit" => r#"<path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z"/>"#,
        "help" => {
            r#"<circle cx="12" cy="12" r="10"/>
               <path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3"/>
               <line x1="12" y1="17" x2="12.01" y2="17"/>"#
        }
        "plus" => {
            r#"<line x1="12" y1="5" x2="12" y2="19"/>
               <line x1="5" y1="12" x2="19" y2="12"/>"#
        }
        "chevron-right" => r#"<polyline points="9 18 15 12 9 6"/>"#,
        "chevron-down" => r#"<polyline points="6 9 12 15 18 9"/>"#,
        "more" => {
            r#"<circle cx="5" cy="12" r="1"/>
               <circle cx="12" cy="12" r="1"/>
               <circle cx="19" cy="12" r="1"/>"#
        }
        "arrow-right" => {
            r#"<line x1="4" y1="12" x2="19" y2="12"/>
               <polyline points="13 6 19 12 13 18"/>"#
        }
        "close" => {
            r#"<line x1="18" y1="6" x2="6" y2="18"/>
               <line x1="6" y1="6" x2="18" y2="18"/>"#
        }
        "bell" => {
            r#"<path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/>
               <path d="M13.73 21a2 2 0 0 1-3.46 0"/>"#
        }
        _ => "",
    }
}
