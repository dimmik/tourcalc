//! The small presentation decisions, ported from the app's `NewUi` helpers.
//!
//! They are ported rather than reinvented because the stylesheet is reused as it is: the
//! avatars have to come out the same colour, or two interfaces on the same data would look
//! gratuitously different.

use tc_core::{Cents, Person};

/// An amount, grouped the way the app groups it: 1 234 567.
///
/// `Cents` already knows how to print itself; this exists so the call sites read the same
/// as the C# ones.
pub fn money(c: Cents) -> String {
    c.to_string()
}

/// Two letters for the avatar.
///
/// The C# version reaches for something more distinctive when two people in the same tour
/// would collide ("Родители" and "Рома" both starting РО). That is not ported yet: this is
/// the plain first-two-letters case, which is what it does for everybody else.
pub fn initials(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return "?".into();
    }
    n.chars()
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect::<String>()
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
