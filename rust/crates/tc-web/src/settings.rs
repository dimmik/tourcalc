//! What this browser has been set to.
//!
//! Kept in `localStorage` under the name the app already uses, with the field names the app
//! already writes - so a browser that has been through the Blazor client keeps its settings
//! here, and a browser that goes back keeps them there.
//!
//! Two of them, out of the app's eleven. The rest are either about the Classic interface
//! (which is not ported, and which the app's own form says the setting does nothing outside
//! of), or about something this client decided differently: Explain is always on, and push
//! is a bell on each tour rather than one switch for all of them.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

const KEY: &str = "__tc_ui_settings";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Below this, in the tour's smallest currency, a debt is treated as settled. The app's
    /// spelling, because the same browser's settings are read by both clients.
    #[serde(rename = "MinimumMeaningfulDebt", default = "default_threshold")]
    pub minimum_meaningful_debt: i64,

    /// A preset name - "indigo", "blue", "teal", "green", "plum", "crimson", "graphite" -
    /// or a `#rrggbb` of somebody's own choosing.
    #[serde(rename = "Accent_Colour", default = "default_accent")]
    pub accent: String,

    /// Everything else the app stores here, carried through untouched. A setting this client
    /// does not offer is still somebody's setting in the other one, and writing the file back
    /// without it would quietly reset their pie chart.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

fn default_threshold() -> i64 {
    tc_core::MINIMUM_MEANINGFUL
}

fn default_accent() -> String {
    "indigo".to_owned()
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            minimum_meaningful_debt: default_threshold(),
            accent: default_accent(),
            rest: serde_json::Map::new(),
        }
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn stored() -> Settings {
    storage()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn remember(settings: &Settings) {
    let (Some(s), Ok(text)) = (storage(), serde_json::to_string(settings)) else {
        return;
    };
    let _ = s.set_item(KEY, &text);
}

// --- how often an open tour asks whether somebody else changed it --------------------------

/// Kept apart from the settings above, under this client's own prefix: that object is
/// shared with the Blazor client, which writes it back from its own model and would drop a
/// field it does not know - quietly putting this back to the default.
const CHECK_KEY: &str = "__tcw_check_seconds";

/// The choices offered, in seconds. `0` is "only when I come back to the tab".
pub const CHECK_CHOICES: &[u32] = &[5, 10, 30, 60, 0];

/// Ten seconds: a change at the next table shows up while people are still talking about
/// it, and a tour left open on a phone costs a handful of tiny requests a minute.
pub const CHECK_DEFAULT: u32 = 10;

/// How often an open tour asks the server whether somebody else saved, in seconds; `0` for
/// never on a timer. Anything not on the list reads as the default, so a hand-edited "1"
/// cannot turn every open tab into a request a second.
pub fn check_seconds() -> u32 {
    storage()
        .and_then(|s| s.get_item(CHECK_KEY).ok().flatten())
        .and_then(|text| text.trim().parse::<u32>().ok())
        .filter(|n| CHECK_CHOICES.contains(n))
        .unwrap_or(CHECK_DEFAULT)
}

pub fn remember_check_seconds(seconds: u32) {
    if let Some(s) = storage() {
        let _ = s.set_item(CHECK_KEY, &seconds.to_string());
    }
}

fn compact_key(tour: &str) -> String {
    format!("__tcw_compact_{tour}")
}

/// Whether this tour's People tab is drawn one line per person on this device.
///
/// Per tour, because it is a property of the list and not of the reader: a trip of thirty
/// wants it, a weekend of four does not. Per device, because it is about the screen.
pub fn compact_people(tour: &str) -> bool {
    storage()
        .and_then(|s| s.get_item(&compact_key(tour)).ok().flatten())
        .is_some_and(|v| v == "1")
}

/// Remembers the choice. Only "on" is written: the default takes no room, and a device
/// that has seen fifty tours does not keep fifty keys saying "no".
pub fn remember_compact_people(tour: &str, compact: bool) {
    if let Some(s) = storage() {
        let _ = if compact {
            s.set_item(&compact_key(tour), "1")
        } else {
            s.remove_item(&compact_key(tour))
        };
    }
}

/// The settings every screen reads, and the one place they are written.
pub type Shared = RwSignal<Settings>;

/// The multiplier the pie chart's colours are generated with. Not a setting this client
/// offers - but it is one the app offers, it is carried through in `rest` like every other,
/// and reading it is what makes a tour the same colours in both clients.
pub fn piechart_magic() -> f64 {
    let stored = use_context::<Shared>()
        .map(|s| s.get_untracked())
        .unwrap_or_else(stored);
    stored
        .rest
        .get("Magic_Piechart_Color_Scheme_Number")
        .and_then(|v| v.as_i64())
        .filter(|n| *n > 0)
        .unwrap_or(1630) as f64
        / 1000.0
}

/// What this reader calls too small to bother with, in the currency the tour is shown in.
///
/// A tracked read: called from inside the closures that draw the balances, so changing the
/// setting redraws them rather than waiting for the next time the page is built.
pub fn threshold(tour: &tc_core::Tour) -> tc_core::Cents {
    let setting = use_context::<Shared>()
        .map(|s| s.get().minimum_meaningful_debt)
        .unwrap_or(tc_core::MINIMUM_MEANINGFUL);
    tour.min_meaningful(setting)
}
