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

/// Up to three descriptions in quotes, and how many more: “Ужин”, “Такси” and 2 more.
pub fn quoted(what: &[String], blank: &str, more: &str) -> String {
    const NAMED: usize = 3;
    let names: Vec<String> = what
        .iter()
        .take(NAMED)
        .map(|d| match d.trim() {
            "" => format!("“{blank}”"),
            d => format!("“{d}”"),
        })
        .collect();
    match what.len().saturating_sub(NAMED) {
        0 => names.join(", "),
        n => format!("{} + {n} {more}", names.join(", ")),
    }
}

/// Every text the interface shows, one field each. See the module note.
pub struct Texts {
    pub explain: ExplainTexts,
    pub mini: MiniTexts,
    pub queue: QueueTexts,
    pub balance: BalanceTexts,
    pub expenses: ExpenseTexts,
    pub tour: TourTexts,
    pub sync: SyncTexts,
    pub dialogs: DialogTexts,
    pub people: PeopleTexts,
    pub list: ListTexts,
    pub chart: ChartTexts,
    pub checks: CheckTexts,
    pub errors: ErrorTexts,
    pub others: OthersTexts,
    pub build: BuildTexts,
    pub device: DeviceTexts,
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
    pub expired: &'static str,
}

pub struct SettingsTexts {
    pub title: &'static str,
    pub language: &'static str,
    pub language_desc: &'static str,
    pub language_short: &'static str,
    /// "as the browser (English)": the choice that follows the browser, naming what it
    /// currently comes to.
    pub language_auto: fn(&str) -> String,
    pub min_debt: &'static str,
    pub min_debt_desc: &'static str,
    pub min_debt_short: &'static str,
    pub accent: &'static str,
    pub accent_desc: &'static str,
    pub accent_short: &'static str,
    pub accent_other: &'static str,
    /// The seven preset colours, in `accent::PRESETS` order.
    pub accent_names: [&'static str; 7],
    pub saved: &'static str,
    pub device: &'static str,
    pub check: &'static str,
    pub check_desc: &'static str,
    pub check_short: &'static str,
    /// What opens the rest of a setting's description.
    pub more: &'static str,
    /// What folds it back.
    pub less: &'static str,
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

/// Installing the app, and the bell that subscribes this device to a tour.
pub struct DeviceTexts {
    pub installing: &'static str,
    pub install_dismissed: &'static str,
    pub install_gone: &'static str,
    pub install_failed: &'static str,
    pub install: &'static str,
    pub install_desc: &'static str,
    pub install_short: &'static str,
    pub installed_here: &'static str,
    pub installed_elsewhere: &'static str,
    pub by_hand_before: &'static str,
    pub by_hand_menu: &'static str,
    pub by_hand_after: &'static str,
    pub not_offered: &'static str,
    pub checked_here: &'static str,
    pub install_button: &'static str,
    pub cannot_install: &'static str,
    pub bell_on_hint: &'static str,
    pub bell_off_hint: &'static str,
    pub bell_checking_hint: &'static str,
    pub bell_unknown_hint: &'static str,
    pub bell_refused_hint: &'static str,
    pub bell_on: &'static str,
    pub bell_off: &'static str,
    pub bell_checking: &'static str,
    pub bell_unknown: &'static str,
    pub bell_refused: &'static str,
    pub ring_on: &'static str,
    pub ring_off: &'static str,
    pub ring_unknown: &'static str,
}

/// Which version is running: the mark in the bar, and the block at the foot of Help.
pub struct BuildTexts {
    pub checking: &'static str,
    pub latest: &'static str,
    pub update: &'static str,
    pub cant_tell: &'static str,
    pub why_asking: &'static str,
    pub why_latest: &'static str,
    pub why_stale: &'static str,
    pub why_unknown: &'static str,
    pub built_running: fn(&str, &str) -> String,
    pub built: fn(&str) -> String,
    pub not_pipeline_running: fn(&str) -> String,
    pub newer_on_server: &'static str,
    pub update_button: &'static str,
    pub this_build: &'static str,
    pub in_browser: &'static str,
    pub client_file: fn(&str) -> String,
    pub not_trunk: &'static str,
    pub on_server: &'static str,
    pub no_answer: &'static str,
    pub asking: &'static str,
    pub server_newer: fn(&str) -> String,
    pub server_same: fn(&str) -> String,
    pub server_unknown: &'static str,
    pub built_title: &'static str,
    pub not_by_pipeline: &'static str,
    pub commit: &'static str,
    pub running_since: &'static str,
    pub running_since_desc: &'static str,
    pub throw_away: &'static str,
    pub ask_again: &'static str,
}

/// Somebody else's change arriving on an open tour. What changed is said by `tc_core::news`,
/// in English, the same words the notification uses; only the frame around it is here.
pub struct OthersTexts {
    pub changed_an_expense: &'static str,
    pub gone: &'static str,
    pub my_tours: &'static str,
    pub updated: fn(&str) -> String,
    pub dismiss: &'static str,
    pub waiting_what: fn(&str) -> String,
    pub waiting: &'static str,
    pub kept_changing: fn(usize) -> String,
}

/// What a request that went wrong says. The technical tail (`{e}`) is the browser's own
/// words and stays as it comes.
pub struct ErrorTexts {
    pub expired: &'static str,
    pub no_answer: &'static str,
    pub unreadable: &'static str,
    pub server_code: fn(u16) -> String,
    pub refused_code: fn(u16) -> String,
    pub conflict: &'static str,
    pub no_connection: fn(&str) -> String,
    pub unreachable: fn(&str) -> String,
    pub cannot_read_token: fn(&str) -> String,
    pub not_master_key: &'static str,
    pub code_refused: &'static str,
    pub server_answered: fn(u16) -> String,
    pub code_refused_with: fn(u16) -> String,
    pub not_found: &'static str,
    pub cannot_read_answer: fn(&str) -> String,
    pub cannot_read_tour: fn(&str) -> String,
    pub cannot_read_list: fn(&str) -> String,
    pub odd_list: &'static str,
    pub cannot_write_tour: fn(&str) -> String,
    pub cannot_build_request: fn(&str) -> String,
    pub tour_gone: &'static str,
    pub server_answered_detail: fn(u16, &str) -> String,
    pub cannot_read_versions: fn(&str) -> String,
    pub odd_versions: &'static str,
    pub admin_first_tour: &'static str,
    pub admin_last_tour: &'static str,
    pub cannot_write_down: fn(&str) -> String,
}

/// What a form says is wrong before it will save.
pub struct CheckTexts {
    pub amount_zero: &'static str,
    pub no_description: &'static str,
    pub for_nobody: &'static str,
    pub no_payer: &'static str,
    pub no_name: &'static str,
    pub no_weight: &'static str,
    pub tour_no_name: &'static str,
    pub tour_no_days: &'static str,
    pub no_currency: &'static str,
    pub rate_zero: fn(&str) -> String,
    pub duplicate_currency: fn(&str) -> String,
    pub amount_not_a_number: &'static str,
    pub amount_too_many_decimals: &'static str,
    pub amount_no_cents: fn(&str) -> String,
}

/// The ring in Stats.
pub struct ChartTexts {
    pub all_of_them: &'static str,
    pub what_money_went_on: &'static str,
    pub total: &'static str,
    pub show_only: fn(&str) -> String,
}

/// The list of tours, in either interface.
pub struct ListTexts {
    pub order_new: &'static str,
    pub order_touched: &'static str,
    pub order_name: &'static str,
    pub order_spent: &'static str,
    pub newest_first: &'static str,
    pub oldest_first: &'static str,
    pub changed_first: &'static str,
    pub untouched_first: &'static str,
    pub z_to_a: &'static str,
    pub a_to_z: &'static str,
    pub biggest_first: &'static str,
    pub smallest_first: &'static str,
    pub click_for: fn(&str, &str) -> String,
    pub order_by: fn(&str) -> String,
    pub not_a_tour: fn(&str) -> String,
    pub cannot_read_tour: &'static str,
    pub clone_of: fn(&str) -> String,
    pub cannot_write_tour: &'static str,
    pub copied: &'static str,
    pub delete_question: fn(&str) -> String,
    pub search: &'static str,
    pub clear: &'static str,
    pub new_tour: &'static str,
    pub show_archived: &'static str,
    pub tour_name: &'static str,
    pub tour_name_example: &'static str,
    pub tour_json: &'static str,
    pub tour_json_hint: &'static str,
    pub access_code: &'static str,
    pub access_code_hint: &'static str,
    pub access_code_note: &'static str,
    pub create: &'static str,
    pub loading: &'static str,
    pub link_signs_in: &'static str,
    pub no_tours: &'static str,
    pub nothing_matches: &'static str,
    pub your_tours: &'static str,
    pub orphans: fn(usize) -> String,
    pub orphans_tail: &'static str,
    pub people: fn(usize) -> String,
    pub settling_hint: &'static str,
    pub settling: &'static str,
    pub square_hint: &'static str,
    pub square: &'static str,
    pub archived_hint: &'static str,
    pub archived: &'static str,
    pub waiting_hint: &'static str,
    pub not_accepted: fn(usize) -> String,
    pub not_sent: fn(usize) -> String,
    pub spent_hint: &'static str,
    pub left_hint: &'static str,
    pub to_settle: &'static str,
    pub clone: &'static str,
    pub clone_bare_hint: &'static str,
    pub clone_bare: &'static str,
    pub copy_json: &'static str,
    pub delete: &'static str,
    pub cannot_copy: &'static str,
}

/// The People tab, and the words for a balance that Balance and Mini use too.
///
/// Russian has no neutral past tense for "paid", and a card cannot know whose name it is
/// on; so the Russian words are nouns and present or future tenses, which are the same
/// for everyone.
pub struct PeopleTexts {
    pub paid: &'static str,
    pub charged: &'static str,
    pub balance: &'static str,
    pub owes: &'static str,
    pub gets: &'static str,
    pub settled: &'static str,
    pub add_person: &'static str,
    pub find: &'static str,
    pub clear: &'static str,
    pub compact_hint: &'static str,
    pub compact: &'static str,
    pub expand_all: &'static str,
    pub collapse_all: &'static str,
    pub total_weight: &'static str,
    pub legend_paid: &'static str,
    pub legend_charged: &'static str,
    pub legend_balance: &'static str,
    pub nobody_yet: &'static str,
    pub nobody_yet_hint: &'static str,
    pub nobody_called: fn(&str) -> String,
    pub shorter: &'static str,
    pub paid_for_by: &'static str,
    pub weight: &'static str,
    pub paid_by: &'static str,
    pub pays_for: &'static str,
    pub family_weight_hint: &'static str,
    pub family_weight: &'static str,
    pub own_hint: &'static str,
    pub own: &'static str,
    pub collapse: &'static str,
    pub weight_hint: fn(i64) -> String,
    pub family_hint: fn(usize, i64) -> String,
    pub spend_for: fn(&str) -> String,
    pub spend: &'static str,
    pub edit: &'static str,
    pub delete: &'static str,
    pub sheet_paid: fn(&str, &str) -> String,
    pub sheet_charged: fn(&str, &str) -> String,
    pub sheet_will_pay: fn(&str, &str) -> String,
    pub sheet_will_collect: fn(&str, &str) -> String,
    pub close: &'static str,
    pub got_it: &'static str,
    pub nothing_recorded: &'static str,
    pub share_of: fn(f64, &str) -> String,
    pub from: &'static str,
    pub running: &'static str,
    pub needs_to_pay: &'static str,
    pub will_collect: &'static str,
    pub nothing_left: &'static str,
    pub to: &'static str,
}

/// The forms: an expense, a person, the tour, its currencies, its versions.
pub struct DialogTexts {
    pub cancel: &'static str,
    pub save: &'static str,
    pub carried: &'static str,
    pub start_blank: &'static str,
    pub edit_expense: &'static str,
    pub new_expense: &'static str,
    pub amount: &'static str,
    pub what_for: &'static str,
    pub what_for_example: &'static str,
    pub paid_by: &'static str,
    pub split_between: &'static str,
    pub everyone: &'static str,
    pub shared_by_all: fn(usize) -> String,
    pub pick_who: &'static str,
    pub selected: fn(usize) -> String,
    pub clear: &'static str,
    pub by_weight_note: &'static str,
    pub equally_note: &'static str,
    pub category: &'static str,
    pub new_category: &'static str,
    pub last_used: &'static str,
    /// The guess when there is nothing to guess from but the one category on offer.
    pub only_category: &'static str,
    pub category_name: &'static str,
    pub add: &'static str,
    pub date: &'static str,
    pub date_note: &'static str,
    pub more_options: &'static str,
    pub split_chosen: &'static str,
    pub split_how: &'static str,
    pub by_weight: &'static str,
    pub equally: &'static str,
    pub split_everyone_note: &'static str,
    pub split_weight_note: &'static str,
    pub split_equal_note: &'static str,
    pub colour: &'static str,
    pub reset: &'static str,
    pub colour_note: &'static str,
    pub this_expense: &'static str,
    pub weight_presets: [&'static str; 4],
    pub edit_person: &'static str,
    pub add_person: &'static str,
    pub name: &'static str,
    pub who_joins: &'static str,
    pub share: &'static str,
    pub custom: &'static str,
    pub share_note: &'static str,
    pub paid_for_by: &'static str,
    pub pays_for_self: &'static str,
    pub paid_for_note: &'static str,
    pub edit_tour: &'static str,
    pub days: &'static str,
    pub days_note: &'static str,
    pub settling: &'static str,
    pub settling_note: &'static str,
    pub archived: &'static str,
    pub archived_note: &'static str,
    pub currencies_of: fn(&str) -> String,
    pub main_currency: &'static str,
    pub main_currency_note: &'static str,
    pub rates: &'static str,
    pub rates_note: &'static str,
    pub add_currency: &'static str,
    pub worth: &'static str,
    pub remove: &'static str,
    pub rename_note: &'static str,
    pub with_cents: &'static str,
    pub with_cents_hint: &'static str,
    /// Fold "EURc" into "EUR": the EURc, then the currency.
    pub absorb: fn(&str, &str) -> String,
    /// How many expenses, from which currency, into which.
    pub will_move: fn(usize, &str, &str) -> String,
    pub will_round: fn(usize) -> String,
    pub will_absorb: fn(usize, &str, &str) -> String,
    /// The main currency gains or loses cents: the debt threshold is counted in them.
    pub will_threshold_cents: &'static str,
    pub will_threshold_whole: &'static str,
    /// "Today's rate": the button, while asking, and after - what the worth was, and the
    /// date of the rate (then the source's name, as a link).
    pub rate_button: &'static str,
    /// Where the button would be, on the currency the others are worked out from.
    pub rate_base: &'static str,
    pub rate_asking: &'static str,
    /// Every currency at once, the cheapest by the market at 10 000.
    pub rate_all: &'static str,
    /// An unknown currency (name) moved in proportion to a known one (name).
    pub rate_kept: fn(&str, &str) -> String,
    pub rate_was: fn(&str, &str) -> String,
    /// Day and month.
    pub rate_date: fn(u32, u32) -> String,
    /// On the source's link: who is asked, and what they see.
    pub rate_source_hint: &'static str,
    pub rate_unknown: fn(&str) -> String,
    pub rate_not_at_source: fn(&str) -> String,
    pub rate_nothing_to_compare: &'static str,
    pub rate_offline: &'static str,
    pub rate_out_of_range: &'static str,
    /// Multiply every worth by this? Because the base (name, worth) is too small for four
    /// figures, or a currency with cents (name, worth) does not end in two zeros.
    pub raise_base: fn(&str, &str, i32) -> String,
    pub raise_cents: fn(&str, &str, i32) -> String,
    pub raise_yes: &'static str,
    pub raise_no: &'static str,
    pub raised: fn(i32) -> String,
    pub restore_question: &'static str,
    pub cannot_read_version: &'static str,
    pub restored_name: fn(&str, &str, &str) -> String,
    pub restored: &'static str,
    pub got_it: &'static str,
    pub versions_of: fn(&str) -> String,
    pub loading_versions: &'static str,
    pub no_versions: &'static str,
    pub no_versions_note: &'static str,
    pub restore: &'static str,
}

/// Where the copy on screen came from, and what is waiting to be sent.
pub struct SyncTexts {
    pub just_now: &'static str,
    pub min_ago: fn(i64) -> String,
    pub h_ago: fn(i64) -> String,
    pub no_answer: &'static str,
    pub unreadable: &'static str,
    pub not_on_server: &'static str,
    pub conflict: &'static str,
    pub forbidden: &'static str,
    pub server_trouble: fn(u16) -> String,
    pub refused: fn(u16) -> String,
    pub asking: &'static str,
    pub new_data: &'static str,
    pub nothing_newer: &'static str,
    pub failed: fn(&str) -> String,
    pub stale: fn(&str) -> String,
    pub from_server: &'static str,
    pub local_copy: &'static str,
    pub stale_hint: fn(&str) -> String,
    pub share_hint: &'static str,
    pub link_copied: &'static str,
    pub share_link: &'static str,
    pub and_more: fn(&str, usize) -> String,
    pub not_taken: fn(usize, &str, &str) -> String,
    pub try_again: &'static str,
    pub discard_question: &'static str,
    pub discard: &'static str,
    pub waiting: fn(&str) -> String,
    pub lost_one: fn(&str) -> String,
    pub lost_many: fn(&str) -> String,
    pub dismiss: &'static str,
    pub offline: &'static str,
    pub never_opened: &'static str,
    pub loading: &'static str,
    pub go_to_tours: &'static str,
}

/// A tour's page: its header, its tabs, and what deleting from it asks.
pub struct TourTexts {
    pub delete_expense: fn(&str) -> String,
    pub delete_person: fn(&str) -> String,
    pub delete_person_then: fn(&str, &[String]) -> String,
    pub shares_go: fn(usize) -> String,
    pub leave_family: fn(usize) -> String,
    /// Why somebody cannot be removed: their name, what they paid, what is only for them.
    pub cannot_remove: fn(&str, &[String], &[String]) -> String,
    pub edit_tour: &'static str,
    pub reload: &'static str,
    pub settling_hint: &'static str,
    pub settling: &'static str,
    pub archived_hint: &'static str,
    pub archived: &'static str,
    pub currencies_hint: &'static str,
    pub currencies: &'static str,
    pub versions: &'static str,
    pub show_in_hint: &'static str,
    pub show_in: &'static str,
    /// The first choice in "show amounts in": the tour's own currency.
    pub show_in_main: fn(&str) -> String,
    /// Beside the picker while the tour is shown in another currency.
    pub main_is: fn(&str) -> String,
    pub reset: &'static str,
    pub total_spent: &'static str,
    pub people: &'static str,
    pub expenses: &'static str,
    pub left_to_settle: &'static str,
    pub sections: &'static str,
    pub tab_balance: &'static str,
    pub tab_people: &'static str,
    pub tab_expenses: &'static str,
    pub tab_stats: &'static str,
    pub spend: &'static str,
}

/// The Expenses tab and one expense's row.
pub struct ExpenseTexts {
    pub search: &'static str,
    pub clear: &'static str,
    pub date: &'static str,
    pub amount: &'static str,
    pub clear_filter: &'static str,
    pub nothing_matches: &'static str,
    pub count: fn(usize) -> String,
    pub spent: &'static str,
    pub from_to: [&'static str; 2],
    pub filtered_out_of: &'static str,
    pub settling_up: &'static str,
    pub draft: &'static str,
    pub drafts: &'static str,
    pub uncounted: &'static str,
    /// "Jan" … "Dec", as a day's heading writes the month.
    pub months: [&'static str; 12],
    pub today: &'static str,
    pub yesterday: &'static str,
    pub no_description: &'static str,
    pub inside_family: &'static str,
    pub payback: &'static str,
    pub for_names: fn(&str) -> String,
    pub for_n_of: fn(usize, usize) -> String,
    pub equally: &'static str,
    pub for_hint: fn(&str) -> String,
    pub alone_hint: fn(&str) -> String,
    pub split_equally: &'static str,
    pub details: &'static str,
    pub for_everyone: &'static str,
    pub edit: &'static str,
    pub entered_as: fn(&str, &str) -> String,
    pub draft_counted: &'static str,
    pub draft_not_counted: &'static str,
    pub everyone_by_weight: &'static str,
    pub n_of_equally: fn(usize, usize) -> String,
    pub n_of_by_weight: fn(usize, usize) -> String,
    pub paid: &'static str,
    pub each: &'static str,
    pub not_in_it: &'static str,
}

/// The Balance and Stats tabs.
pub struct BalanceTexts {
    pub all_settled: &'static str,
    pub first_expense: &'static str,
    pub no_payments_left: &'static str,
    /// How many small payments, under what, how much in all, and whether the balances
    /// below are made of them.
    pub dust: fn(usize, &str, &str, bool) -> String,
    pub hide_small: &'static str,
    pub show_small: &'static str,
    pub who_pays_whom: &'static str,
    pub not_paid_yet: &'static str,
    pub inside_families: &'static str,
    pub balances: &'static str,
    pub gets_back: &'static str,
    pub owes_money: &'static str,
    pub mark_paid_hint: &'static str,
    pub mark_paid_question: fn(&str, &str, &str) -> String,
    pub mark_paid: &'static str,
    pub by_category: &'static str,
    pub by_person: &'static str,
    pub nothing_to_chart: &'static str,
    pub nothing_to_chart_hint: &'static str,
    pub totals: &'static str,
    pub everything: &'static str,
    pub total: &'static str,
    pub per_person_w: fn(i64) -> String,
    pub per_person_day: &'static str,
    pub over: &'static str,
    pub days: &'static str,
    pub per_person: &'static str,
    pub per_day: &'static str,
}

/// How a waiting edit is named in "Saved here, waiting to be sent: …".
pub struct QueueTexts {
    pub expense_removed: &'static str,
    pub person_removed: &'static str,
    pub renamed: fn(&str) -> String,
    pub amounts_in: fn(&str) -> String,
    pub the_tour: fn(&str) -> String,
    pub the_currencies: &'static str,
    pub paid: fn(&str) -> String,
    pub no_description: &'static str,
    pub nothing_here: &'static str,
    pub go_to_tours: &'static str,
}

/// The small interface, which says the same things in fewer letters.
pub struct MiniTexts {
    pub settling_hint: &'static str,
    pub settling: &'static str,
    pub archived_hint: &'static str,
    pub archived: &'static str,
    pub spend_hint: &'static str,
    pub spend: &'static str,
    pub reload: &'static str,
    pub spent_hint: &'static str,
    pub people_hint: &'static str,
    pub people_short: &'static str,
    pub expenses_hint: &'static str,
    pub expenses_short: &'static str,
    pub settled_hint: &'static str,
    pub settled: &'static str,
    pub left_hint: &'static str,
    pub left: &'static str,
    pub from_server: &'static str,
    pub asking: &'static str,
    pub local_copy: &'static str,
    pub waiting: fn(usize) -> String,
    pub edit: &'static str,
    pub currencies: &'static str,
    pub versions: &'static str,
    pub tab_balance: &'static str,
    pub tab_people: &'static str,
    pub tab_expenses: &'static str,
    pub tab_stats: &'static str,
    pub in_unit: fn(&str) -> String,
    pub square: &'static str,
    pub first_expense: &'static str,
    pub dust: fn(usize, &str, &str) -> String,
    pub mark_paid_question: fn(&str, &str, &str) -> String,
    pub who_pays_whom: &'static str,
    pub tap_to_record: &'static str,
    pub inside_families: &'static str,
    pub balances: &'static str,
    pub gets: &'static str,
    pub owes: &'static str,
    pub add_person: &'static str,
    pub find: &'static str,
    pub weight_hint: &'static str,
    pub nobody: &'static str,
    pub people: &'static str,
    pub settled_through: fn(&str) -> String,
    pub settled_row: &'static str,
    pub paid: &'static str,
    pub charged: &'static str,
    pub own: &'static str,
    pub family_hint: &'static str,
    pub family: &'static str,
    pub weight: &'static str,
    pub paid_by: &'static str,
    pub spend_row: &'static str,
    pub delete: &'static str,
    pub date: &'static str,
    pub amount: &'static str,
    pub category: &'static str,
    pub all: &'static str,
    pub no_expenses: &'static str,
    pub nothing_matches_filter: &'static str,
    pub everyone: &'static str,
    pub n_of: fn(usize, usize) -> String,
    pub for_whom: &'static str,
    pub each: &'static str,
    pub nothing_to_count: &'static str,
    pub total: &'static str,
    pub per_person: &'static str,
    pub per_person_day: fn(i64) -> String,
    pub by_category: &'static str,
    pub by_payer: &'static str,
    pub in_group: &'static str,
    pub cancel: &'static str,
    pub add_tour: &'static str,
    pub show_archived: &'static str,
    pub saving: &'static str,
    pub tap_for: fn(&str, &str) -> String,
    pub order_by: fn(&str) -> String,
    pub tour_name: &'static str,
    pub code: &'static str,
    pub create: &'static str,
    pub tour_json: &'static str,
    pub no_tours: &'static str,
    pub nothing_matches: &'static str,
    pub unsent: fn(usize) -> String,
    pub days_short: &'static str,
    pub more: &'static str,
    pub clone: &'static str,
    pub clone_bare_hint: &'static str,
    pub clone_bare: &'static str,
    pub json: &'static str,
}

/// Explain: what every number opens into.
pub struct ExplainTexts {
    pub where_from: &'static str,
    pub close: &'static str,
    pub got_it: &'static str,
    pub entries: fn(usize) -> String,
    pub unknown_person: &'static str,
    pub total_spent: &'static str,
    pub total_note: &'static str,
    pub skipped: fn(usize) -> String,
    pub paid_for_by: fn(&str) -> String,
    pub people_title: &'static str,
    pub people_count: fn(usize) -> String,
    pub weights_note: fn(i64, i64) -> String,
    pub counted: &'static str,
    pub drafts: &'static str,
    pub paybacks: &'static str,
    pub inside_families: &'static str,
    pub first: &'static str,
    pub last: &'static str,
    pub average: &'static str,
    pub recorded: &'static str,
    pub left_to_settle: &'static str,
    pub square: &'static str,
    pub even_out: fn(usize, &str) -> String,
    pub weight_title: fn(&str, i32) -> String,
    pub weight: &'static str,
    pub total_weight_in_tour: &'static str,
    pub share_of_all: &'static str,
    pub out_of_1000: fn(&str, &str) -> String,
    pub total_weight: &'static str,
    pub total_weight_note: &'static str,
    pub paid_for_group: &'static str,
    pub charged_for_part: &'static str,
    pub charged_minus_paid: &'static str,
    pub kid_paid_for_by: fn(&str, &str) -> String,
    pub hands_over: &'static str,
    pub counts_as_settled: fn(&str) -> String,
    pub used_more: fn(&str) -> String,
    pub paid_more: fn(&str) -> String,
    pub own_part: fn(&str, &str) -> String,
    pub three_cells: &'static str,
    pub where_title: fn(&str) -> String,
    pub pays: &'static str,
    pub receives: &'static str,
    pub paid_in_total: fn(&str) -> String,
    pub was_charged: fn(&str) -> String,
    pub why_payment: &'static str,
    pub nobody_chose: &'static str,
    pub paid_by: &'static str,
    pub when: &'static str,
    pub category: &'static str,
    pub entered_as: &'static str,
    pub draft: &'static str,
    pub draft_counted: &'static str,
    pub draft_not_counted: &'static str,
    pub weight_n: fn(i32) -> String,
    pub shared_everyone: &'static str,
    pub one_person: &'static str,
    pub n_equal: fn(usize) -> String,
    pub n_weight: fn(usize, i64) -> String,
    pub no_description: &'static str,
    pub split_everyone: fn(usize) -> String,
    pub split_some: fn(usize, usize) -> String,
    pub not_in_this: &'static str,
    pub shown_title: &'static str,
    pub shown_note: &'static str,
    pub not_counted_title: &'static str,
    pub settling_section: fn(&str) -> String,
    pub settling_note: &'static str,
    pub drafts_section: fn(&str) -> String,
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
