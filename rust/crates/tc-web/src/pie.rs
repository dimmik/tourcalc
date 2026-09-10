//! The pie chart, ported from the app's own component (`dimmik/simple-blazor-piechart`).
//!
//! There is no drawing here and no library: a circle painted with a CSS `conic-gradient`,
//! and a list beside it. That is what the original does, and it is the reason the original
//! is forty lines - a pie chart is one gradient and some arithmetic about angles.
//!
//! Two things are ported exactly rather than reinvented, because the two clients stand side
//! by side and a tour should not change colour when you switch between them:
//!
//! * **The colours.** A seed colour, multiplied by a magic number, the low three bytes taken
//!   as the next colour, repeat. It is a nonsense that produces a pleasant sequence, and the
//!   number that drives it is a setting in the app - so it is read from the same place.
//! * **The angles.** In hundredths of a degree, truncated, with the remainder given to the
//!   largest slice, so the circle always closes.

use crate::ui::money;
use leptos::prelude::*;
use tc_core::Cents;

/// The app's seed, from `CSG`.
const SEED: (u8, u8, u8) = (0x35, 0x66, 0xee);

/// What separates a category from its subcategory, and what marks a name that stands for
/// several. The app's defaults.
const SUBCAT: char = '/';
const MORE: &str = " [+]";

/// The colours, in order, from a seed and a magic multiplier.
///
/// The multiplication is done on the colour read as one number - blue lowest, red highest -
/// and the result is cut back to three bytes. Anything above that is thrown away, which is
/// what makes the sequence wander rather than run off into white.
pub fn colours(seed: (u8, u8, u8), magic: f64) -> impl Iterator<Item = String> {
    let (mut r, mut g, mut b) = seed;
    std::iter::from_fn(move || {
        let here = format!("#{r:02X}{g:02X}{b:02X}");
        let n = (b as i64) | ((g as i64) << 8) | ((r as i64) << 16);
        // Truncated, not rounded: C#'s `(int)` on a double, and the colours differ if it is
        // not.
        let next = (n as f64 * magic) as i64;
        b = (next & 0xFF) as u8;
        g = ((next >> 8) & 0xFF) as u8;
        r = ((next >> 16) & 0xFF) as u8;
        Some(here)
    })
}

/// One wedge: its name, how much of the circle it takes in hundredths of a degree, its
/// colour, and the money it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub name: String,
    pub hundredths: i64,
    pub colour: String,
    pub amount: Cents,
}

/// Everything with the same head - "Food / lunch" and "Food / dinner" - added up under it,
/// marked so it is clear the name stands for more than itself.
///
/// A head that covers only one entry keeps that entry's full name: collapsing "Food / lunch"
/// to "Food" when there is no other Food says something untrue.
fn aggregated(data: &[(String, Cents)]) -> Vec<(String, Cents)> {
    let mut heads: Vec<(String, Vec<&(String, Cents)>)> = Vec::new();
    for item in data {
        let head = item.0.split(SUBCAT).next().unwrap_or("").trim().to_owned();
        match heads.iter_mut().find(|(h, _)| *h == head) {
            Some((_, items)) => items.push(item),
            None => heads.push((head, vec![item])),
        }
    }
    heads
        .into_iter()
        .map(|(head, items)| {
            let sum = Cents(items.iter().map(|(_, c)| c.0).sum());
            match items.as_slice() {
                [only] if items.len() == 1 => (only.0.clone(), sum),
                _ => (format!("{head}{MORE}"), sum),
            }
        })
        .collect()
}

/// Whether collapsing would change anything - which is the only case where offering to
/// expand makes sense.
pub fn has_subcategories(data: &[(String, Cents)]) -> bool {
    aggregated(data).len() < data.len()
}

/// The wedges, largest first, coloured in sequence.
pub fn slices(data: &[(String, Cents)], magic: f64) -> Vec<Slice> {
    let mut data: Vec<(String, Cents)> = data.to_vec();
    if data.is_empty() {
        data.push(("n/a".to_owned(), Cents::ZERO));
    }

    // All zeroes still deserve a circle: every name gets an equal wedge and says why.
    let total: i64 = data.iter().map(|(_, c)| c.0.abs()).sum();
    let (data, total) = if total == 0 {
        let evened: Vec<(String, Cents)> = data
            .iter()
            .map(|(name, _)| (format!("{name} [=0]"), Cents(1)))
            .collect();
        let n = evened.len() as i64;
        (evened, n)
    } else {
        (data, total)
    };

    let mut angles: Vec<i64> = data
        .iter()
        .map(|(_, c)| (c.0.abs() as f64 * 36000.0 / total as f64) as i64)
        .collect();
    // Truncation loses up to a hundredth of a degree per slice, and a circle with a gap in
    // it looks like a bug rather than like rounding. The biggest slice takes the shortfall,
    // where it is least visible.
    let short = 36000 - angles.iter().sum::<i64>();
    if short > 0 {
        if let Some(biggest) = angles
            .iter()
            .enumerate()
            .max_by_key(|(_, a)| **a)
            .map(|(i, _)| i)
        {
            angles[biggest] += short;
        }
    }

    let mut wedges: Vec<(i64, String, Cents)> = angles
        .into_iter()
        .zip(data)
        .map(|(a, (name, amount))| (a, name, amount))
        .collect();
    // Stable, so slices of the same size keep the order they were given in.
    wedges.sort_by(|a, b| b.0.cmp(&a.0));

    wedges
        .into_iter()
        .zip(colours(SEED, magic))
        .map(|((hundredths, name, amount), colour)| Slice {
            name,
            hundredths,
            colour,
            amount,
        })
        .collect()
}

/// The `conic-gradient` the circle is painted with: each colour from where the last one
/// stopped to where this one does.
pub fn gradient(slices: &[Slice]) -> String {
    let mut at = 0i64;
    let stops: Vec<String> = slices
        .iter()
        .map(|s| {
            let from = at as f64 / 100.0;
            at += s.hundredths;
            let to = at as f64 / 100.0;
            format!("{} {from:.2}deg {to:.2}deg", s.colour)
        })
        .collect();
    format!("conic-gradient({})", stops.join(", "))
}

#[component]
pub fn PieChart(data: Vec<(String, Cents)>, unit: String) -> impl IntoView {
    // Details start hidden, as in the app: the head of a category is usually the answer,
    // and the subcategories are what you open when it is not.
    let detailed = RwSignal::new(false);
    let has_subs = has_subcategories(&data);
    let magic = crate::settings::piechart_magic();

    let slices_now = Memo::new(move |_| {
        let source = if detailed.get() || !has_subs {
            data.clone()
        } else {
            aggregated(&data)
        };
        slices(&source, magic)
    });

    view! {
        <div class="tcn-card tcn-chartbox" style="padding:14px; margin-bottom:14px;
                    display:flex; gap:14px; align-items:flex-start; flex-wrap:wrap">
            <div style=move || format!(
                "flex:0 0 auto; width:160px; aspect-ratio:1; border-radius:100%; background:{}",
                gradient(&slices_now.get()))>
            </div>
            <div style="flex:1 1 200px; min-width:180px; font-size:12px">
                {move || {
                    let unit = unit.clone();
                    slices_now.get()
                        .into_iter()
                        .map(|s| {
                            let percent = s.hundredths as f64 * 100.0 / 36000.0;
                            let unit = unit.clone();
                            view! {
                                // Inline rather than classes: the stylesheet belongs to the
                                // Blazor client and is taken unchanged, and four rules for
                                // a legend are not worth editing somebody else's file for.
                                <div style="display:flex; align-items:center; gap:6px;
                                            padding:2px 0">
                                    <span style=format!(
                                        "width:10px; height:10px; border-radius:3px; \
                                         flex:0 0 auto; background:{}", s.colour)></span>
                                    <b style="overflow:hidden; text-overflow:ellipsis;
                                              white-space:nowrap">{s.name}</b>
                                    <span class="tcn-hint" style="margin:0">
                                        {format!("{percent:.1}%")}
                                    </span>
                                    <span style="margin-left:auto; white-space:nowrap">
                                        {money(s.amount)}
                                        {(!unit.is_empty())
                                            .then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                    </span>
                                </div>
                            }
                        })
                        .collect_view()
                }}
                <Show when=move || has_subs>
                    <span class="tcn-hero-link" style="cursor:pointer"
                          on:click=move |_| detailed.update(|d| *d = !*d)>
                        {move || if detailed.get() { "Hide details" } else { "Show details" }}
                    </span>
                </Show>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colours_are_the_apps_colours() {
        // Taken from the app's own generator, seed 0x3566ee and the magic 1.63 its settings
        // default to. The point of the port is that the same tour comes out the same colour
        // in both clients, and only this test can hold that.
        let first: Vec<String> = colours(SEED, 1.63).take(6).collect();
        assert_eq!(
            first,
            vec![
                "#3566EE", "#570B9D", "#8DE24A", "#E74554", "#78F8AF", "#C52EF4"
            ]
        );
    }

    #[test]
    fn the_wedges_close_the_circle() {
        let data = vec![
            ("a".to_owned(), Cents(100)),
            ("b".to_owned(), Cents(33)),
            ("c".to_owned(), Cents(33)),
            ("d".to_owned(), Cents(33)),
        ];
        let wedges = slices(&data, 1.63);
        assert_eq!(
            wedges.iter().map(|s| s.hundredths).sum::<i64>(),
            36000,
            "a gap in the circle reads as a bug, not as rounding"
        );
        assert_eq!(wedges[0].name, "a", "largest first");

        // Nothing spent anywhere still draws something, and says what it is.
        let nothing = vec![("a".to_owned(), Cents::ZERO), ("b".to_owned(), Cents::ZERO)];
        let wedges = slices(&nothing, 1.63);
        assert_eq!(wedges.iter().map(|s| s.hundredths).sum::<i64>(), 36000);
        assert!(wedges[0].name.ends_with("[=0]"), "{}", wedges[0].name);
    }

    #[test]
    fn a_category_with_several_children_stands_for_them() {
        let data = vec![
            ("Food / lunch".to_owned(), Cents(100)),
            ("Food / dinner".to_owned(), Cents(200)),
            ("Taxi / airport".to_owned(), Cents(50)),
        ];
        assert!(has_subcategories(&data));
        let rolled = aggregated(&data);
        assert_eq!(
            rolled,
            vec![
                ("Food [+]".to_owned(), Cents(300)),
                // One child and no siblings keeps its own name: calling it "Taxi" would say
                // there is more under it than there is.
                ("Taxi / airport".to_owned(), Cents(50)),
            ]
        );

        // Flat names are not subcategories, and offering to expand them would open nothing.
        let flat = vec![
            ("Food".to_owned(), Cents(1)),
            ("Taxi".to_owned(), Cents(1)),
        ];
        assert!(!has_subcategories(&flat));
    }

    #[test]
    fn the_gradient_runs_from_where_the_last_one_stopped() {
        let data = vec![("a".to_owned(), Cents(3)), ("b".to_owned(), Cents(1))];
        let css = gradient(&slices(&data, 1.63));
        assert_eq!(
            css,
            "conic-gradient(#3566EE 0.00deg 270.00deg, #570B9D 270.00deg 360.00deg)"
        );
    }
}
