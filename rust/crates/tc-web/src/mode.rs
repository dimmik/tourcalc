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
