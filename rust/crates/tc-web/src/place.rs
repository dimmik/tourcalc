//! Where the reader was last doing something, kept between screens and between visits.
//!
//! Help and Settings are not places you go *to* so much as places you step aside into: what
//! you want afterwards is the tour you came from, not the list of every tour you have. The
//! app had no answer for that - the bar said "Tourcalc" and the only way back was the list
//! and then the tour - so the way back is remembered here instead.
//!
//! In `localStorage`, so it also survives the page being reloaded, which on a phone is
//! something the browser decides, not the reader: come back to a backgrounded tab an hour
//! later, and the tour you were in is still named at the top.
//!
//! Rust-only, hence the `__tcw_` name: the Blazor client knows nothing about it, and a
//! browser that goes back and forth between the two is none the worse for the extra key.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

const KEY: &str = "__tcw_place";

/// What the top of the screen is about.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    /// Your tours - where somebody who has not opened one yet is, and where leaving a tour
    /// by the list puts you back.
    List,
    /// One tour. The name is carried along with the id because the bar has to write it
    /// before anything has been fetched - the whole point of remembering it.
    Tour { id: String, name: String },
}

impl Place {
    /// What the bar calls it.
    pub fn label(&self) -> String {
        match self {
            Place::List => "Tourcalc".to_owned(),
            // A tour whose name has not arrived yet is still better described by the app's
            // own name than by an empty bar.
            Place::Tour { name, .. } if name.trim().is_empty() => "Tourcalc".to_owned(),
            Place::Tour { name, .. } => name.clone(),
        }
    }

    /// What the tooltip promises while the reader is somewhere else. The bar writes
    /// "Tourcalc" for the list because that is what the list is called at the top of the
    /// screen, but "Back to Tourcalc" is not a sentence about anything.
    pub fn back_to(&self) -> String {
        match self {
            Place::List => "Back to your tours".to_owned(),
            Place::Tour { name, .. } if name.trim().is_empty() => "Back to the tour".to_owned(),
            Place::Tour { name, .. } => format!("Back to {name}"),
        }
    }

    /// Where clicking it goes.
    pub fn href(&self) -> String {
        match self {
            Place::List => "/".to_owned(),
            Place::Tour { id, .. } => format!("/tour/{id}"),
        }
    }
}

/// The one every screen reads. Set by the router as the reader moves, and by the tour as
/// soon as it knows what it is called.
pub type Current = RwSignal<Place>;

/// The stored shape. The list is stored as nothing at all - an absent key and a browser
/// that has never been here are the same situation, and both mean the list.
#[derive(Serialize, Deserialize)]
struct Written {
    id: String,
    name: String,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn from_text(text: &str) -> Place {
    match serde_json::from_str::<Written>(text) {
        Ok(w) if !w.id.is_empty() => Place::Tour {
            id: w.id,
            name: w.name,
        },
        _ => Place::List,
    }
}

fn to_text(place: &Place) -> Option<String> {
    match place {
        Place::List => None,
        Place::Tour { id, name } => serde_json::to_string(&Written {
            id: id.clone(),
            name: name.clone(),
        })
        .ok(),
    }
}

/// Where this browser was when it was last here.
pub fn stored() -> Place {
    storage()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .map(|text| from_text(&text))
        .unwrap_or(Place::List)
}

fn remember(place: &Place) {
    let Some(s) = storage() else { return };
    match to_text(place) {
        Some(text) => {
            let _ = s.set_item(KEY, &text);
        }
        None => {
            let _ = s.remove_item(KEY);
        }
    }
}

fn move_to(current: Current, place: Place) {
    if current.get_untracked() == place {
        return;
    }
    remember(&place);
    current.set(place);
}

/// The reader is looking at the list, so that is now the way back.
pub fn went_to_the_list(current: Current) {
    move_to(current, Place::List);
}

/// The reader opened a tour. The name is whatever is already known - usually nothing, for
/// about as long as it takes to read the copy in this browser - and `name_is` fills it in.
pub fn went_to_a_tour(current: Current, id: &str) {
    if let Place::Tour { id: already, .. } = current.get_untracked() {
        if already == id {
            return;
        }
    }
    move_to(
        current,
        Place::Tour {
            id: id.to_owned(),
            name: String::new(),
        },
    );
}

/// The tour turns out to be called this. Ignored if the reader has moved on since - a slow
/// answer about a tour nobody is in any more must not rename the bar.
pub fn name_is(current: Current, id: &str, name: &str) {
    if let Place::Tour { id: here, .. } = current.get_untracked() {
        if here == id {
            move_to(
                current,
                Place::Tour {
                    id: id.to_owned(),
                    name: name.to_owned(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remembered_tour_survives_the_round_trip() {
        let place = Place::Tour {
            id: "abc".into(),
            name: "Хижины и рюкзаки".into(),
        };
        let text = to_text(&place).expect("a tour is written down");
        assert_eq!(from_text(&text), place);
    }

    #[test]
    fn nothing_written_down_means_the_list() {
        assert_eq!(to_text(&Place::List), None);
        // Whatever else may be in that key - a value from an older version, somebody's
        // experiment in the console - is not a tour, and the list is the safe answer.
        assert_eq!(from_text(""), Place::List);
        assert_eq!(from_text(r#"{"id":"","name":"x"}"#), Place::List);
    }

    #[test]
    fn the_way_back_is_described_by_where_it_leads() {
        assert_eq!(Place::List.back_to(), "Back to your tours");
        assert_eq!(
            Place::Tour {
                id: "abc".into(),
                name: "Карелия".into()
            }
            .back_to(),
            "Back to Карелия"
        );
    }

    #[test]
    fn a_tour_whose_name_has_not_arrived_is_still_a_way_back_to_it() {
        let waiting = Place::Tour {
            id: "abc".into(),
            name: String::new(),
        };
        assert_eq!(waiting.label(), "Tourcalc");
        assert_eq!(waiting.href(), "/tour/abc");
    }
}
