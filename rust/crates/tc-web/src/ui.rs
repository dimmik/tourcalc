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
