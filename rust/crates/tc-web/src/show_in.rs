//! "Show amounts in": which currency this reader reads a tour in - on this device only.
//!
//! The tour has a main currency, chosen in the currencies dialog and saved with the tour, and
//! everyone sees the tour in it unless they choose otherwise. Choosing otherwise is this: it
//! is remembered here, per tour, and changes nothing on the server - no save, no version, no
//! notification. It used to be an edit of the tour, so reading it in roubles told everybody
//! subscribed that "amounts are in RUB" now, and moved the tour's own currency under them.
//!
//! Remembered rather than forgotten on every open, because somebody who reads a tour in their
//! own currency reads it that way every time. The page says so beside the picker ("main -
//! EUR · reset") so that it is never mistaken for the tour's own figures. A remembered
//! currency the tour no longer has is simply not used.
//!
//! Everything on the page reads `Tour::currency()`, so showing the tour in another currency
//! is a copy of it with `current_currency` swapped - and the main one noted in the copy's
//! extras for the one place that must not take the swap for the real thing: the currencies
//! dialog, which saves the main currency.

use tc_core::{CurrencyId, Tour};

const KEY: &str = "__tcw_show_in_";
/// On the copy shown: the tour's own main currency, which the copy's `current_currency` is
/// not. Never saved - edits go through the queue against the stored tour.
const MAIN: &str = "__tcwMainCurrency";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// The currency this device reads `tour_id` in, if it chose one.
pub fn chosen(tour_id: &str) -> Option<String> {
    storage()
        .and_then(|s| s.get_item(&format!("{KEY}{tour_id}")).ok().flatten())
        .filter(|c| !c.is_empty())
}

/// Remembers the choice - `None` is back to the tour's main currency.
pub fn choose(tour_id: &str, currency: Option<&str>) {
    let Some(s) = storage() else { return };
    let key = format!("{KEY}{tour_id}");
    let _ = match currency.filter(|c| !c.is_empty()) {
        Some(c) => s.set_item(&key, c),
        None => s.remove_item(&key),
    };
}

/// The tour as this reader sees it: in the chosen currency when the tour has it and it is not
/// the main one already.
pub fn view_of(tour: &Tour, chosen: Option<&str>) -> Tour {
    let Some(id) = chosen.filter(|c| {
        *c != tour.current_currency.as_str() && tour.currencies.iter().any(|x| x.id.as_str() == *c)
    }) else {
        return tour.clone();
    };
    let mut shown = tour.clone();
    tc_core::extras::set(&mut shown.extras, MAIN, tour.current_currency.as_str().into());
    shown.current_currency = CurrencyId::new(id);
    shown
}

/// The tour's own main currency, whatever it is being shown in.
pub fn main_of(tour: &Tour) -> CurrencyId {
    let noted = tc_core::extras::str_of(&tour.extras, MAIN);
    if noted.is_empty() {
        tour.current_currency.clone()
    } else {
        CurrencyId::new(noted)
    }
}

/// The tour as stored, from a copy made by [`view_of`].
pub fn stored(tour: &Tour) -> Tour {
    let mut t = tour.clone();
    t.current_currency = main_of(tour);
    t.extras.0.remove(MAIN);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour() -> Tour {
        let json = serde_json::json!({
            "Id": "t", "Name": "t", "Persons": [],
            "Currencies": [
                {"_id": "RSD", "Name": "RSD", "CurrencyRate": 100},
                {"_id": "EUR", "Name": "EUR", "CurrencyRate": 11745}
            ],
            "TourCurrencyId": "RSD", "Spendings": []
        });
        Tour::from_json(&json.to_string()).expect("tour")
    }

    #[test]
    fn showing_in_another_currency_keeps_the_main_one_for_the_dialog() {
        let t = tour();
        let main = t.current_currency.clone();
        let other = t
            .currencies
            .iter()
            .find(|c| c.id != main)
            .map(|c| c.id.as_str().to_owned());
        let other = other.expect("a second currency");
        let shown = view_of(&t, Some(&other));
        assert_eq!(shown.current_currency.as_str(), other);
        assert_eq!(main_of(&shown), main);
        assert_eq!(stored(&shown), t);
    }

    #[test]
    fn a_currency_the_tour_does_not_have_is_not_used() {
        let t = tour();
        assert_eq!(view_of(&t, Some("XYZ")), t);
        assert_eq!(view_of(&t, None), t);
        assert_eq!(main_of(&t), t.current_currency);
    }
}
