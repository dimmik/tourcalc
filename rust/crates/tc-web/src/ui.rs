//! The small presentation decisions, ported from the app's `NewUi` helpers.
//!
//! They are ported rather than reinvented because the stylesheet is reused as it is: the
//! avatars have to come out the same colour, or two interfaces on the same data would look
//! gratuitously different.

use leptos::prelude::use_context;
use tc_core::{Cents, Person};

/// An amount, grouped the way the app groups it: 1 234 567.
///
/// `Cents` already knows how to print itself; this exists so the call sites read the same
/// as the C# ones.
pub fn money(c: Cents) -> String {
    c.to_string()
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
