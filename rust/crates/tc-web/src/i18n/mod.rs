//! Which language the interface speaks, and every word it says in each.
//!
//! **Chosen once, at start.** The reader's own choice if they made one (kept on the device),
//! otherwise the first of the browser's languages that the app speaks, otherwise English.
//! Changing it reloads the page: a switch that happens once in somebody's life is not worth
//! wrapping four hundred strings in reactive closures so that it can happen without one.
//!
//! **Checked by the compiler.** Every text lives in a field of [`Texts`], and each language
//! is a `const` of that struct - so a field one language forgot is a build error, not an
//! English word turning up in the middle of a Russian screen. A text with something in it
//! ("owes 47 124") is a field holding a function, which is how the word order gets to
//! differ between languages.
//!
//! **What stays in English**, deliberately: whatever the server writes (push notifications,
//! the lines on a tour's versions, the text pages), the change log, and the names people
//! gave to their own tours, people and categories.

mod en;
mod ru;

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Ru];

    /// The tag that goes into `<html lang>` and into storage.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
        }
    }

    /// Its own name, in itself: what the switch shows, so that somebody who landed in a
    /// language they cannot read can still find theirs.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        // "ru-RU", "en-GB": the language is what comes before the region.
        let primary = code.split(['-', '_']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    fn texts(self) -> &'static Texts {
        match self {
            Lang::En => &en::TEXTS,
            Lang::Ru => &ru::TEXTS,
        }
    }
}

const KEY: &str = "__tcw_lang";

static CURRENT: OnceLock<Lang> = OnceLock::new();

/// The language of this page.
///
/// English in the tests, which run natively, where there is no browser to ask - and which
/// check the English wording.
pub fn lang() -> Lang {
    if cfg!(not(target_arch = "wasm32")) {
        return Lang::En;
    }
    *CURRENT.get_or_init(|| chosen().unwrap_or_else(from_browser))
}

/// Every word, in the language of this page.
pub fn t() -> &'static Texts {
    lang().texts()
}

/// What the reader picked, if anything: `None` is "whatever the browser says".
pub fn chosen() -> Option<Lang> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .and_then(|code| Lang::from_code(&code))
}

/// The first of the browser's languages this app speaks.
pub fn from_browser() -> Lang {
    let Some(navigator) = web_sys::window().map(|w| w.navigator()) else {
        return Lang::En;
    };
    let listed: Vec<String> = navigator
        .languages()
        .iter()
        .filter_map(|v| v.as_string())
        .collect();
    first_spoken(listed.iter().map(String::as_str).chain(navigator.language().as_deref()))
}

/// The first language in `wanted` that the app speaks, English when none is.
pub fn first_spoken<'a>(wanted: impl IntoIterator<Item = &'a str>) -> Lang {
    wanted
        .into_iter()
        .find_map(Lang::from_code)
        .unwrap_or(Lang::En)
}

/// Keeps the reader's choice - `None` goes back to following the browser - and reloads, so
/// the page comes back in it.
pub fn choose(lang: Option<Lang>) {
    let Some(window) = web_sys::window() else { return };
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = match lang {
            Some(l) => storage.set_item(KEY, l.code()),
            None => storage.remove_item(KEY),
        };
    }
    let _ = window.location().reload();
}

/// Tells the document which language it is in: screen readers, hyphenation and the
/// browser's own "translate this page?" all go by it.
pub fn mark_document() {
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let _ = root.set_attribute("lang", lang().code());
    }
}

/// Russian counts in three forms: 1 трата, 2 траты, 5 трат - and 11 трат, 21 трата.
pub fn ru_plural(n: i64, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let n = n.unsigned_abs();
    match (n % 10, n % 100) {
        (1, r) if r != 11 => one,
        (2..=4, r) if !(12..=14).contains(&r) => few,
        _ => many,
    }
}

/// English counts in two.
pub fn en_plural(n: i64, one: &'static str, other: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        other
    }
}

/// Every text the interface shows, one field each. See the module note.
pub struct Texts {
    pub shell: ShellTexts,
    pub settings: SettingsTexts,
}

/// The frame around every screen: the bar, signing in, the interface switch.
pub struct ShellTexts {
    pub tour_list: &'static str,
    pub back_to_tours: &'static str,
    pub back_to_tour: &'static str,
    pub back_to: fn(&str) -> String,
    pub help_title: &'static str,
    pub settings_title: &'static str,
    pub menu: &'static str,
    pub help: &'static str,
    pub help_hint: &'static str,
    pub settings: &'static str,
    pub log_out: &'static str,
    pub a_tour_changed: &'static str,
    pub open: &'static str,
    pub dismiss: &'static str,
    pub signing_in: &'static str,
    pub switch_interface: &'static str,
    pub full_hint: &'static str,
    pub mini_hint: &'static str,
    pub login_lead: &'static str,
    pub access_code: &'static str,
    pub your_code: &'static str,
    pub logging_in: &'static str,
    pub log_in: &'static str,
    pub no_code: &'static str,
}

pub struct SettingsTexts {
    pub title: &'static str,
    pub language: &'static str,
    pub language_desc: &'static str,
    /// "as the browser (English)": the choice that follows the browser, naming what it
    /// currently comes to.
    pub language_auto: fn(&str) -> String,
    pub min_debt: &'static str,
    pub min_debt_desc: &'static str,
    pub accent: &'static str,
    pub accent_desc: &'static str,
    pub accent_other: &'static str,
    /// The seven preset colours, in `accent::PRESETS` order.
    pub accent_names: [&'static str; 7],
    pub saved: &'static str,
    pub device: &'static str,
    pub check: &'static str,
    pub check_desc: &'static str,
    pub check_on_return: &'static str,
    pub check_minute: &'static str,
    pub check_seconds: fn(u32) -> String,
    pub elsewhere: &'static str,
    pub interface: &'static str,
    pub interface_desc: &'static str,
    pub notifications: &'static str,
    pub notifications_desc: &'static str,
    pub explain: &'static str,
    pub explain_desc: &'static str,
    pub how_it_counts: &'static str,
    pub how_it_counts_desc: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_language_the_app_speaks_wins() {
        assert_eq!(first_spoken(["de-DE", "ru-RU", "en"]), Lang::Ru);
        assert_eq!(first_spoken(["en-GB", "ru"]), Lang::En);
        assert_eq!(first_spoken(["de", "fr"]), Lang::En);
        assert_eq!(first_spoken([]), Lang::En);
    }

    #[test]
    fn russian_counts_in_three_forms() {
        let form = |n| ru_plural(n, "трата", "траты", "трат");
        assert_eq!(
            [1, 2, 5, 11, 12, 14, 21, 22, 25, 101, 111, 0].map(form),
            ["трата", "траты", "трат", "трат", "трат", "трат", "трата", "траты", "трат", "трата", "трат", "трат"]
        );
    }
}
