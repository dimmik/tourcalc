//! The small presentation decisions, ported from the app's `NewUi` helpers.
//!
//! They are ported rather than reinvented because the stylesheet is reused as it is: the
//! avatars have to come out the same colour, or two interfaces on the same data would look
//! gratuitously different.

use leptos::prelude::*;
use tc_core::{Cents, Person};

/// How long a confirmation stays on screen: long enough to read, short enough that it is
/// gone before it stops being news.
pub const CONFIRMED_FOR: std::time::Duration = std::time::Duration::from_secs(4);

/// A line that says something went right and then goes away by itself - "copied".
///
/// Kept apart from the error line on purpose: an error stays until it is dealt with, a
/// confirmation that stayed would still be saying "copied" an hour later, about nothing in
/// particular. Saying it again restarts the clock rather than being cut short by the first
/// one's timer: every `say` is numbered, and only the latest may clear the line.
#[derive(Clone, Copy)]
pub struct Brief {
    text: RwSignal<String>,
    said: StoredValue<u32>,
}

impl Brief {
    pub fn new() -> Self {
        Brief { text: RwSignal::new(String::new()), said: StoredValue::new(0) }
    }

    pub fn say(self, text: impl Into<String>) {
        let n = self.said.try_get_value().unwrap_or(0).wrapping_add(1);
        self.said.try_set_value(n);
        self.text.try_set(text.into());
        set_timeout(
            move || {
                if self.said.try_get_value() == Some(n) {
                    self.text.try_set(String::new());
                }
            },
            CONFIRMED_FOR,
        );
    }

    /// Takes it away now - when the next thing on the same line is an error.
    pub fn hush(self) {
        self.said.try_update_value(|n| *n = n.wrapping_add(1));
        self.text.try_set(String::new());
    }

    pub fn get(self) -> String {
        self.text.get()
    }

    pub fn is_on(self) -> bool {
        !self.text.with(String::is_empty)
    }
}

/// The name of the currency a tour's amounts are shown in, to write beside them.
///
/// Only on a tour with more than one currency, as in the app: that is where "which
/// currency is this" is a real question. A tour with a single currency usually has the
/// placeholder "coin" as that currency, while the money was really euros or roubles -
/// naming it would be naming something wrong.
///
/// Every amount on such a tour is in this one currency, whatever it was entered in, and
/// is labelled with it - the balances and people included, which the app left bare under
/// a header that read "65 910 RSD".
pub fn unit(tour: &tc_core::Tour) -> String {
    if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    }
}

/// Puts the cursor in `node` once it is on the screen - for a form that opens: somebody who
/// opened "add a person" starts typing the name at once, and the letters went nowhere.
/// After the frame, so that whatever shows the form has laid it out.
pub fn focus_when_shown(node: NodeRef<leptos::html::Input>) {
    Effect::new(move |_| {
        if let Some(el) = node.get() {
            request_animation_frame(move || {
                let _ = el.focus();
                caret_to_end(&el);
            });
        }
    });
}

/// In a dialog just opened, the box to type in first: the one marked `data-first`, else the
/// first text box of its body - not a checkbox, not something disabled.
pub fn focus_first_in(root: &web_sys::Element) {
    use wasm_bindgen::JsCast;
    let pick = root.query_selector("[data-first]").ok().flatten().or_else(|| {
        root.query_selector(
            ".tcn-modal-body input:not([type=checkbox]):not([type=radio]):not([disabled]), \
             .tcn-modal-body textarea",
        )
        .ok()
        .flatten()
    });
    if let Some(el) = pick.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
        let _ = el.focus();
        caret_to_end(&el);
    }
}

/// In a box that already says something - a name being edited - the cursor after it, where
/// typing goes on, and not before it. Boxes that have no caret (numbers) are left as they are.
fn caret_to_end(el: &web_sys::HtmlElement) {
    use wasm_bindgen::JsCast;
    if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
        let end = input.value().encode_utf16().count() as u32;
        let _ = input.set_selection_range(end, end);
    }
}

/// An amount followed by its unit, when there is one: "47 124 RSD".
pub fn money_in(c: Cents, unit: &str) -> String {
    if unit.is_empty() {
        money(c)
    } else {
        format!("{}\u{a0}{unit}", money(c))
    }
}

/// An amount of the tour on screen, in the currency it is being read in: 1 234 567, or
/// 1 234,56 when that currency has cents.
///
/// Whether it has cents is set once, by the tour page, when it gets its tour
/// ([`show_cents_for`]) - there is only ever one tour on that page, and every figure on it is
/// in that tour's currency, so threading a flag through eighty call sites would say the same
/// thing eighty times. Where an amount is in some *other* currency - a row of the tour list,
/// "entered as 12 EUR" - [`amount`] is called with that currency's own flag instead.
pub fn money(c: Cents) -> String {
    amount(c, DISPLAY_CENTS.with(|d| d.get()))
}

/// An amount in a currency with or without cents, in the reader's decimal separator.
pub fn amount(c: Cents, cents: bool) -> String {
    tc_core::units::format(c, cents, decimal())
}

/// "3,50" in Russian, "3.50" in English.
pub fn decimal() -> char {
    match crate::i18n::lang() {
        crate::i18n::Lang::En => '.',
        crate::i18n::Lang::Ru => ',',
    }
}

thread_local! {
    static DISPLAY_CENTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Tells [`money`] whether this tour's figures are hundredths. Called by the tour page for
/// the tour it is showing, every time it is given one.
pub fn show_cents_for(tour: &tc_core::Tour) {
    DISPLAY_CENTS.with(|d| d.set(tour.shows_cents()));
}

/// The two or three letters on an avatar.
///
/// Normally the first letter of each of the first two words - "Женя К." is ЖК - and the
/// first two letters of a single-word name. It reaches for something more distinctive only
/// when another name in the same tour would produce the same label, which is how "Родители"
/// and "Рома" both ended up as РО.
///
/// `peers` is everybody else on the tour. Pass an empty slice where they are not to hand:
/// the first candidate is what the app shows in that case too.
pub fn initials_among(name: &str, peers: &[String]) -> String {
    let n = name.trim();
    if n.is_empty() {
        return "?".into();
    }

    let mine = candidates(n);
    let taken: Vec<String> = peers
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case(n))
        // Only each peer's *first* choice is taken: the app does not chase collisions
        // between second choices, and neither does this.
        .filter_map(|p| candidates(p).into_iter().next())
        .collect();

    mine.iter()
        .find(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(c)))
        .or_else(|| mine.first())
        .cloned()
        .unwrap_or_else(|| "?".into())
}

/// Everybody on the tour being shown, so that two people whose names start alike do not
/// wear the same two letters.
///
/// Passed as context rather than as a parameter through six layers of view. This is what
/// context is for: ambient information about *where* something is being drawn, which every
/// avatar wants and none of the components in between care about. The C# does the same thing
/// by handing `Initials` the whole tour.
#[derive(Clone)]
pub struct Peers(pub Vec<String>);

/// The initials for a name, avoiding the other people on the tour if any are known.
pub fn initials(name: &str) -> String {
    match use_context::<Peers>() {
        Some(peers) => initials_among(name, &peers.0),
        None => initials_among(name, &[]),
    }
}

/// The labels this name could wear, best first.
fn candidates(n: &str) -> Vec<String> {
    let upper = |s: String| s.chars().flat_map(char::to_uppercase).collect::<String>();
    let parts: Vec<&str> = n.split([' ', '-', '_']).filter(|p| !p.is_empty()).collect();

    if parts.len() >= 2 {
        let a: Vec<char> = parts[0].chars().collect();
        let b: Vec<char> = parts[1].chars().collect();
        let mut out = vec![upper(format!("{}{}", a[0], b[0]))];
        if b.len() >= 2 {
            out.push(upper(format!("{}{}{}", a[0], b[0], b[1])));
        }
        return out;
    }

    let w: Vec<char> = parts
        .first()
        .map(|p| p.chars().collect())
        .unwrap_or_default();
    if w.len() <= 1 {
        return vec![upper(w.into_iter().collect())];
    }
    let mut out = vec![
        upper(w[..2].iter().collect()),
        upper(format!("{}{}", w[0], w[w.len() - 1])),
    ];
    if w.len() >= 3 {
        out.push(upper(w[..3].iter().collect()));
    }
    out
}

/// The avatar's colour, derived from the name so that a person keeps theirs.
///
/// Reproduces the C# hash exactly, including the wrapping multiply - hence `wrapping_mul`
/// rather than `*`, which would panic in a debug build on overflow. Rust makes that choice
/// explicit where C# hides it behind `unchecked`.
///
/// The characters are UTF-16 code units, again to match: a Rust `char` is a whole scalar
/// value, and iterating those would give different numbers for anything outside the basic
/// plane.
pub fn avatar_colour(seed: &str) -> String {
    let mut hash: i32 = 17;
    for unit in seed.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(unit as i32);
    }
    let hue = hash.unsigned_abs() % 360;
    let hue2 = (hue + 28) % 360;
    format!("linear-gradient(135deg, hsl({hue} 62% 52%), hsl({hue2} 66% 42%))")
}

/// Name, or a placeholder when the person is gone.
pub fn name_of(p: Option<&Person>) -> String {
    p.map(|p| p.name.clone()).unwrap_or_else(|| "n/a".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour_in(names: &[&str], current: &str) -> tc_core::Tour {
        tc_core::Tour {
            id: tc_core::TourId::new("t"),
            name: "t".into(),
            persons: Vec::new(),
            spendings: Vec::new(),
            currencies: names
                .iter()
                .map(|n| tc_core::Currency {
                    id: tc_core::CurrencyId::new(*n),
                    name: (*n).to_owned(),
                    rate: 100,
                    extras: Default::default(),
                })
                .collect(),
            current_currency: tc_core::CurrencyId::new(current),
            extras: Default::default(),
        }
    }

    #[test]
    fn a_tour_of_several_currencies_names_the_one_it_is_shown_in() {
        assert_eq!(unit(&tour_in(&["RSD", "EUR"], "EUR")), "EUR");
    }

    #[test]
    fn a_tour_of_one_currency_or_none_names_nothing() {
        // The one currency is usually the placeholder "coin", not what the money was.
        assert_eq!(unit(&tour_in(&["coin"], "coin")), "");
        assert_eq!(unit(&tour_in(&[], "")), "");
        assert_eq!(money_in(Cents(47_124), ""), "47\u{202f}124");
        assert_eq!(money_in(Cents(47_124), "RSD"), "47\u{202f}124\u{a0}RSD");
    }

    #[test]
    fn a_generated_payment_reads_as_one() {
        assert_eq!(
            as_service_transfer("X 'Женя К.' -> 'Паша'"),
            Some(("Женя К.".to_owned(), "Паша".to_owned()))
        );
        assert_eq!(
            as_service_transfer("Family 'Олежка' -> 'Саша О.'"),
            Some(("Олежка".to_owned(), "Саша О.".to_owned()))
        );
        // Anything a person typed is left exactly as they typed it.
        assert_eq!(as_service_transfer("Ужин в 'Прадо' -> вкусно"), None);
        assert_eq!(as_service_transfer("X 'a' -> 'b' and then some"), None);
        assert_eq!(as_service_transfer("такси"), None);
    }

    #[test]
    fn only_a_chosen_colour_marks_a_row() {
        assert!(is_marked("#ffd54f"));
        assert!(is_marked("#fd5"));
        // What the app writes when a payment is recorded: not a mark.
        assert!(!is_marked("lightgreen"));
        assert!(!is_marked("lightgray"));
        assert!(!is_marked(""));
        assert!(!is_marked("#12345"));
    }

    #[test]
    fn a_mark_paints_and_does_not_only_declare() {
        // The bug this holds: the two custom properties were set, the class was added, and
        // the row stayed white, because the stylesheet paints `.tcn-sp.tcn-sp-marked` and
        // these rows are not `.tcn-sp`.
        let style = mark_style("#ffd54f");
        assert!(style.contains("--tcn-mark-bg:"), "{style}");
        assert!(
            style.contains("background:#"),
            "a mark that only declares a colour is not a mark: {style}"
        );
        assert!(style.contains("border-color:#"), "{style}");
        assert_eq!(mark_style("lightgreen"), "", "not a colour, not a mark");
    }

    #[test]
    fn a_colour_keeps_its_hue_and_loses_its_lightness() {
        let style = mark_style("#ffd54f");
        assert!(style.contains("--tcn-mark-line:#"), "{style}");
        assert!(style.contains("--tcn-mark-bg:#"), "{style}");
        // The background is the pale one and the line the dark one - that is the whole
        // point of computing two.
        let line = style.split("--tcn-mark-line:").nth(1).unwrap()[..7].to_owned();
        let bg = style.split("--tcn-mark-bg:").nth(1).unwrap()[..7].to_owned();
        let lightness = |hex: &str| {
            let rgb = parse_hex(hex).unwrap();
            to_hsl(rgb).2
        };
        assert!(lightness(&bg) > lightness(&line), "{line} vs {bg}");
    }

    #[test]
    fn grey_becomes_a_neutral_outline_and_not_a_dusty_pink() {
        let style = mark_style("#808080");
        assert!(style.contains("--tcn-mark-line:#525252"), "{style}");
    }

    #[test]
    fn initials_are_two_letters() {
        assert_eq!(initials("Андрей"), "АН");
        assert_eq!(initials("bob"), "BO");
        assert_eq!(initials(" "), "?");
    }

    /// The colours have to match the C# ones, or the same person would come out a
    /// different colour in the two interfaces on the same tour.
    ///
    /// The expected values are what `NewUi.AvatarColor` computes - worked out from its
    /// algorithm rather than copied from this implementation's output, which would only
    /// prove that it agrees with itself.
    #[test]
    fn avatar_colour_matches_csharp() {
        assert_eq!(
            avatar_colour("Андрей"),
            "linear-gradient(135deg, hsl(282 62% 52%), hsl(310 66% 42%))"
        );
        assert_eq!(
            avatar_colour("Паша"),
            "linear-gradient(135deg, hsl(322 62% 52%), hsl(350 66% 42%))"
        );
        assert_eq!(
            avatar_colour("Женя К."),
            "linear-gradient(135deg, hsl(354 62% 52%), hsl(22 66% 42%))"
        );
        assert_eq!(
            avatar_colour(""),
            "linear-gradient(135deg, hsl(17 62% 52%), hsl(45 66% 42%))"
        );
    }
}

/// Whether a spending's colour was chosen by a person.
///
/// Only a `#hex` counts. "lightgreen" and "lightgray" are written by the app itself when a
/// payment is marked paid, and a row that lit up for that would be telling the reader
/// something nobody meant.
pub fn is_marked(colour: &str) -> bool {
    parse_hex(colour).is_some()
}

/// The inline style a marked row is drawn with.
///
/// The chosen hue is kept but its lightness is not: a colour picked to be recognisable is
/// not one that reads as text on a white row. Ported from the C# so the same colour marks
/// the same way in both interfaces.
///
/// It paints as well as declaring. The stylesheet's rule is `.tcn-sp.tcn-sp-marked`, and the
/// expense rows here are `.tcn-settle` - so setting the two custom properties and the class
/// left the row exactly as pale as before, which is what a spending with a colour on it
/// looked like: not coloured at all. The properties stay for anything that does match that
/// rule; the paint is spelled out so it does not depend on a class combination in a
/// stylesheet this client only borrows. Found in a screenshot, after a DOM check had said
/// the mark was there - the attribute was, the colour was not.
pub fn mark_style(colour: &str) -> String {
    let Some(rgb) = parse_hex(colour) else {
        return String::new();
    };
    let (h, s, _) = to_hsl(rgb);
    // Black, white and grey carry no hue to keep; they become a heavy neutral outline
    // rather than the dusty pink that clamping a hue-less colour would produce.
    let flat = s < 0.05;
    let line = if flat {
        from_hsl(0.0, 0.0, 0.32)
    } else {
        from_hsl(h, s.clamp(0.30, 0.85), 0.40)
    };
    let bg = if flat {
        from_hsl(0.0, 0.0, 0.93)
    } else {
        from_hsl(h, s.clamp(0.25, 0.80), 0.945)
    };
    format!(
        "--tcn-mark-line:{line};--tcn-mark-bg:{bg};\
         background:{bg};border-color:{line};"
    )
}

/// A moment in time, written in the reader's own clock: "10.09.2026 21:52".
///
/// Not the same job as [`crate::explain::pretty_stamp`], which slices an ISO string and
/// leaves it alone - right for a spending, whose stamp is the wall clock of whoever entered
/// it, and wrong for an instant. The server says when it started in UTC, and slicing that
/// showed a reader in Belgrade a time two hours behind their own watch.
pub fn local_stamp(millis: f64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(millis));
    format!(
        "{:02}.{:02}.{} {:02}:{:02}",
        d.get_date(),
        d.get_month() + 1,
        d.get_full_year(),
        d.get_hours(),
        d.get_minutes()
    )
}

/// The same, from the ISO stamp a server sends.
pub fn local_stamp_of(iso: &str) -> String {
    let millis = js_sys::Date::parse(iso);
    if millis.is_nan() {
        return iso.to_owned();
    }
    local_stamp(millis)
}

pub fn parse_hex(colour: &str) -> Option<(u8, u8, u8)> {
    let c = colour.trim().strip_prefix('#')?;
    let c: String = match c.len() {
        // #abc is #aabbcc
        3 => c.chars().flat_map(|ch| [ch, ch]).collect(),
        6 => c.to_owned(),
        _ => return None,
    };
    Some((
        u8::from_str_radix(&c[0..2], 16).ok()?,
        u8::from_str_radix(&c[2..4], 16).ok()?,
        u8::from_str_radix(&c[4..6], 16).ok()?,
    ))
}

pub fn to_hsl((r, g, b): (u8, u8, u8)) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn from_hsl(h: f64, s: f64, l: f64) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (((h % 360.0) + 360.0) % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let byte = |v: f64| ((v * 255.0).round().clamp(0.0, 255.0)) as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r + m), byte(g + m), byte(b + m))
}

/// A description the calculator wrote for itself, split into who and whom.
///
/// `X 'Женя К.' -> 'Паша'` and `Family 'Олежка' -> 'Саша О.'` are what a recorded payment
/// carries. Generated rows sit alongside the ones people typed, so the quotes and the arrow
/// are shown as a payment rather than as a description - without ever rewriting anything a
/// human wrote.
///
/// Done by hand rather than with a regular expression: the shape is fixed, and a regex crate
/// is 300 KB of wasm for one pattern.
pub fn as_service_transfer(description: &str) -> Option<(String, String)> {
    let rest = description
        .strip_prefix("X '")
        .or_else(|| description.strip_prefix("Family '"))?;
    let (from, rest) = rest.split_once("' -> '")?;
    let to = rest.strip_suffix('\'')?;
    if from.is_empty() || to.is_empty() {
        return None;
    }
    Some((from.to_owned(), to.to_owned()))
}
