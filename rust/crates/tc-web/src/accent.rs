//! The colour the app is painted in.
//!
//! Six presets and anything else. A preset is only a name: the stylesheet carries a
//! `[data-tcn-accent="teal"]` block for each one, so setting the attribute is the whole of
//! it. A colour of somebody's own has to be turned into the same five custom properties by
//! hand, and that is what the rest of this file does - ported from the app's own function,
//! including the part that matters most.
//!
//! Which is: white text sits on the accent - the top bar, the buttons, the floating button -
//! so a colour too light for it is darkened until it clears 4.5:1 against white. The hue and
//! the saturation are left exactly as chosen. Without that, picking a nice yellow makes the
//! app unreadable rather than yellow.

use crate::ui::{from_hsl, parse_hex, to_hsl};

/// The presets, in the order the app shows them: the name, the label, and the two ends of
/// the gradient the sample is drawn in. The colours are here only to paint the samples -
/// what a preset actually means lives in the stylesheet.
pub const PRESETS: &[Preset] = &[
    Preset { id: "indigo", label: "Indigo (default)", from: "#4f46e5", to: "#6d28d9" },
    Preset { id: "blue", label: "Ocean", from: "#2563eb", to: "#226c91" },
    Preset { id: "teal", label: "Teal", from: "#0f766e", to: "#226881" },
    Preset { id: "green", label: "Forest", from: "#15803d", to: "#1b6a5a" },
    Preset { id: "plum", label: "Plum", from: "#7e22ce", to: "#8d358d" },
    Preset { id: "crimson", label: "Crimson", from: "#b91c1c", to: "#922a51" },
    Preset { id: "graphite", label: "Graphite", from: "#475569", to: "#425367" },
];

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub from: &'static str,
    pub to: &'static str,
}

pub fn is_preset(name: &str) -> bool {
    PRESETS.iter().any(|p| p.id == name)
}

/// What the colour picker opens on: the colour chosen by hand, or the first stop of the
/// preset in use - so it starts from what is on screen rather than from black.
pub fn swatch(current: &str) -> String {
    PRESETS
        .iter()
        .find(|p| p.id == current)
        .map(|p| p.from.to_owned())
        .unwrap_or_else(|| current.to_owned())
}

/// Paints the page. A preset by name, anything else as a colour.
pub fn apply(name: &str) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    const VARS: &[&str] = &[
        "--tcn-primary",
        "--tcn-primary-dark",
        "--tcn-primary-soft",
        "--tcn-accent-2",
        "--tcn-accent-shadow",
    ];
    let style = body.style();

    let Some(rgb) = parse_hex(name) else {
        // A preset: drop anything set by hand and let the stylesheet answer.
        for v in VARS {
            let _ = style.remove_property(v);
        }
        let _ = body.set_attribute(
            "data-tcn-accent",
            if name.is_empty() { "indigo" } else { name },
        );
        return;
    };

    let (h, s, l) = to_hsl(rgb);
    let l = readable(h, s, l);
    // The second gradient stop is a neighbouring hue and carries white text too, so it goes
    // through the same check.
    let h2 = h + 25.0;
    let l2 = readable(h2, s, (l - 0.05).max(0.0));
    // A grey has no hue to keep: floor its saturation at zero, or the pale tint comes out
    // pink instead of grey.
    let soft = if s < 0.05 { 0.0 } else { s.max(0.45) };

    let primary = from_hsl(h, s, l);
    let _ = body.set_attribute("data-tcn-accent", "custom");
    let set = |name: &str, value: &str| {
        let _ = style.set_property(name, value);
    };
    set("--tcn-primary", &primary);
    set("--tcn-primary-dark", &from_hsl(h, s, (l - 0.06).max(0.0)));
    set("--tcn-primary-soft", &from_hsl(h, soft, 0.965));
    set("--tcn-accent-2", &from_hsl(h2, s, l2));
    if let Some((r, g, b)) = parse_hex(&primary) {
        set("--tcn-accent-shadow", &format!("rgba({r},{g},{b},.28)"));
    }
}

/// Darkens a colour until white text on it clears 4.5:1.
fn readable(h: f64, s: f64, mut l: f64) -> f64 {
    while l > 0.06 && contrast_with_white(h, s, l) < 4.5 {
        l -= 0.01;
    }
    l
}

fn contrast_with_white(h: f64, s: f64, l: f64) -> f64 {
    let Some((r, g, b)) = parse_hex(&from_hsl(h, s, l)) else {
        return 21.0;
    };
    let channel = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
    1.05 / (luminance + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_too_light_for_white_text_is_darkened() {
        // Pure yellow: white on it is unreadable, and picking it should make the app yellow
        // rather than unusable.
        let (h, s, l) = to_hsl(parse_hex("#ffff00").unwrap());
        let fixed = readable(h, s, l);
        assert!(fixed < l, "{fixed} is not darker than {l}");
        assert!(contrast_with_white(h, s, fixed) >= 4.5);

        // Something already dark enough is left exactly as chosen.
        let (h, s, l) = to_hsl(parse_hex("#4f46e5").unwrap());
        assert_eq!(readable(h, s, l), l);
    }

    #[test]
    fn the_presets_are_names_and_not_colours() {
        assert!(is_preset("teal"));
        assert!(!is_preset("#00bcd4"));
        assert!(parse_hex("teal").is_none(), "a name is not a colour");
    }
}
