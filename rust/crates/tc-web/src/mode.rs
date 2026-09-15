//! Which of the two interfaces is on screen.
//!
//! Kept in `localStorage` under the name the app already uses, so a browser that has been
//! set to the small interface in the Blazor client opens this one the same way.
//!
//! There is no Classic here. The C# keeps it working and untouched for the people who were
//! using it before the redesign; a port has nobody in that position.

use leptos::prelude::*;

const KEY: &str = "__tc_ui_mode";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiMode {
    /// Cards, avatars, room to breathe.
    Full,
    /// One line per thing: an old phone, a narrow screen, twenty rows instead of six.
    Mini,
}

impl UiMode {
    fn stored_as(self) -> &'static str {
        match self {
            UiMode::Full => "new",
            UiMode::Mini => "mini",
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// What this browser was last set to. The roomy one unless it says otherwise - including
/// for anybody whose stored value is the C#'s "old", since Classic is not ported.
pub fn stored() -> UiMode {
    match storage()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .as_deref()
    {
        Some("mini") => UiMode::Mini,
        _ => UiMode::Full,
    }
}

pub fn remember(mode: UiMode) {
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, mode.stored_as());
    }
}

/// Tells the stylesheet which interface is on.
///
/// Two classes, because the two stylesheets are written against two different elements.
/// `tcm-shell` on the shell is what shrinks the top bar and gives the rows the whole width;
/// `tcm-on` on the body is what tightens the forms, which mini does not redraw - it borrows
/// the roomy ones and takes the padding off. Both are the app's own names and the app's own
/// placement: the rules are in `mini.css`, unchanged, and this is the switch they were
/// written for.
///
/// (The app also puts `tcn-on` on the body for both interfaces. That one only paints the
/// background behind the shell, and the shell here is opaque and full height, so there is
/// nothing for it to do.)
pub fn on_the_body(mode: UiMode) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    let _ = body
        .class_list()
        .toggle_with_force("tcm-on", mode == UiMode::Mini);
}

/// The switch, small enough to sit in a header.
#[component]
pub fn ModeSwitch(mode: RwSignal<UiMode>) -> impl IntoView {
    // The app's control: both interfaces named, the one you are in lit up. A single link
    // that said "mini" left it ambiguous whether that was where you were or where it would
    // take you - and it is the sort of thing a reader should not have to test.
    let pick = move |what: UiMode| {
        move |_| {
            remember(what);
            mode.set(what);
        }
    };
    view! {
        <span class="tcn-uiswitch" title="Switch the interface">
            <button type="button" class="tcn-uiswitch-opt"
                    class:is-active=move || mode.get() == UiMode::Full
                    title="Full interface - the roomy view"
                    on:click=pick(UiMode::Full)>
                "Full"
            </button>
            <button type="button" class="tcn-uiswitch-opt"
                    class:is-active=move || mode.get() == UiMode::Mini
                    title="Mini interface - one line per thing, for a small screen or a slow device"
                    on:click=pick(UiMode::Mini)>
                "Mini"
            </button>
        </span>
    }
}
