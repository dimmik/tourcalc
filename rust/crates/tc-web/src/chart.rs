//! What the money went on, as a ring you can take apart.
//!
//! The app draws a pie with a legend beside it and, under the legend, a link that unfolds
//! every subcategory at once. Two things about that were worth changing rather than
//! porting. The circle was decoration - nothing on it could be pressed, and the row of
//! category chips under the totals repeated the legend in words, as a filter. And "show the
//! subcategories" answers a question nobody asks: subcategories are a question about *one*
//! category, never about all of them at once.
//!
//! So here the ring is the filter, and going into a category is going into that category:
//!
//! * a slice and its row in the list are the same control - press either;
//! * what is chosen lights up, everything else fades, and the middle of the ring says what
//!   it is worth;
//! * a row with something inside it has a chevron, and pressing that redraws the ring on
//!   the inside of it, with a crumb back out.
//!
//! Two things are still ported exactly, because the two clients stand side by side and a
//! tour must not change colour when you switch between them: the colour sequence, and the
//! arithmetic that turns money into angles.

use crate::ui::money;
use leptos::prelude::*;
use tc_core::Cents;

/// The app's seed, from `CSG`.
const SEED: (u8, u8, u8) = (0x35, 0x66, 0xee);

/// What separates a category from its subcategory. The app's default.
pub const SUBCAT: char = '/';

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

/// One line of the chart: what it is called, what it is worth, and whether there is
/// anything inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// What choosing this row means - a category, a head of several, or a person's name.
    pub key: String,
    pub label: String,
    pub amount: Cents,
    /// Whether pressing the chevron leads anywhere.
    pub children: bool,
}

/// One wedge: its row, how much of the circle it takes in hundredths of a degree, and the
/// colour it is drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub row: Row,
    pub hundredths: i64,
    pub colour: String,
}

/// The head of a name: "Food" out of "Food / lunch", and "Taxi" out of "Taxi".
pub fn head_of(name: &str) -> &str {
    name.split(SUBCAT).next().unwrap_or(name).trim()
}

/// Whether a category belongs to what the reader has chosen.
///
/// Nothing chosen is everything; a full name is itself; a head is everything under it. The
/// totals under the chart ask this, which is what makes the ring a filter rather than a
/// picture beside one.
pub fn matches(chosen: &str, category: &str) -> bool {
    chosen.is_empty() || chosen == category.trim() || chosen == head_of(category)
}

/// The top level of a set of categories: everything under one head added up under it.
///
/// A head covering exactly one entry keeps that entry's full name - collapsing "Taxi /
/// airport" to "Taxi" when there is no other Taxi says something untrue - and, having
/// nothing to open, gets no chevron.
pub fn by_head(data: &[(String, Cents)]) -> Vec<Row> {
    let mut heads: Vec<(String, Vec<&(String, Cents)>)> = Vec::new();
    for item in data {
        let head = head_of(&item.0).to_owned();
        match heads.iter_mut().find(|(h, _)| *h == head) {
            Some((_, items)) => items.push(item),
            None => heads.push((head, vec![item])),
        }
    }
    let mut rows: Vec<Row> = heads
        .into_iter()
        .map(|(head, items)| {
            let amount = Cents(items.iter().map(|(_, c)| c.0).sum());
            match items.as_slice() {
                [only] => Row {
                    key: only.0.trim().to_owned(),
                    label: only.0.trim().to_owned(),
                    amount,
                    children: false,
                },
                _ => Row {
                    key: head.clone(),
                    label: head,
                    amount,
                    children: true,
                },
            }
        })
        .collect();
    rows.sort_by_key(|r| -r.amount.0);
    rows
}

/// The wedges, largest first, coloured in sequence.
pub fn slices(rows: &[Row], magic: f64) -> Vec<Slice> {
    let mut rows: Vec<Row> = rows.to_vec();
    if rows.is_empty() {
        return Vec::new();
    }

    // All zeroes still deserve a circle: every name gets an equal wedge and says why.
    let total: i64 = rows.iter().map(|r| r.amount.0.abs()).sum();
    let total = if total == 0 {
        for row in &mut rows {
            row.label = format!("{} [=0]", row.label);
        }
        rows.len() as i64
    } else {
        total
    };
    let weight = |row: &Row| if row.amount.0 == 0 { 1 } else { row.amount.0.abs() };

    let mut angles: Vec<i64> = rows
        .iter()
        .map(|r| (weight(r) as f64 * 36000.0 / total as f64) as i64)
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

    let mut wedges: Vec<(i64, Row)> = angles.into_iter().zip(rows).collect();
    // Stable, so slices of the same size keep the order they were given in.
    wedges.sort_by(|a, b| b.0.cmp(&a.0));
    wedges
        .into_iter()
        .zip(colours(SEED, magic))
        .map(|((hundredths, row), colour)| Slice {
            row,
            hundredths,
            colour,
        })
        .collect()
}

/// Where a wedge starts and how long it is, on a circle of this circumference.
///
/// SVG rather than the `conic-gradient` the app uses, for one reason: a gradient has no
/// parts, and these have to be pressable. A dashed circle has one element per wedge.
pub fn arc(hundredths: i64, before: i64, circumference: f64) -> (f64, f64) {
    let length = hundredths as f64 / 36000.0 * circumference;
    let offset = before as f64 / 36000.0 * circumference;
    (length, offset)
}

/// The ring, the list, and the way in and out.
#[component]
pub fn Composition(
    rows: Vec<Row>,
    unit: String,
    /// What the reader has chosen - shared with the totals under the chart. "" is all of it.
    chosen: RwSignal<String>,
    /// Where we are, if we are inside something.
    crumb: Option<String>,
    /// The reader wants to see the inside of this row.
    into: Callback<String>,
    /// ...and back out again.
    out: Callback<()>,
) -> impl IntoView {
    let magic = crate::settings::piechart_magic();
    let slices = StoredValue::new(slices(&rows, magic));
    let total = Cents(rows.iter().map(|r| r.amount.0).sum());

    // The geometry, in the viewBox's own units, so the ring scales with the box and nothing
    // here has to know how many pixels it ends up as.
    //
    // The radius leaves room for the stroke: a stroke straddles the path, so the ring
    // reaches r + half of it, and the chosen slice is drawn thicker still. At r = 54 that
    // came to 67 against a half-box of 60, and the viewport cut the circle into a square -
    // visible immediately in a picture, and not at all in the numbers.
    const R: f64 = 44.0;
    const STROKE: f64 = 20.0;
    const CHOSEN_STROKE: f64 = 26.0;
    const CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * R;

    let unit_centre = unit.clone();
    let unit_rows = unit.clone();

    let chosen_amount = move || {
        let key = chosen.get();
        slices
            .get_value()
            .iter()
            .find(|s| s.row.key == key)
            .map(|s| (s.row.label.clone(), s.row.amount, s.hundredths))
    };

    view! {
        <div class="tcn-card" style="padding:14px; margin-bottom:14px">
            {crumb.clone().map(|name| view! {
                <div class="tcn-chips" style="margin-bottom:10px">
                    <button type="button" class="tcn-chip tcn-filter-chip"
                            on:click=move |_| out.run(())>
                        "↑ all of them"
                    </button>
                    <span class="tcn-chip is-on">{name}</span>
                </div>
            })}

            <div style="display:flex; gap:16px; align-items:center; flex-wrap:wrap">
                <div style="position:relative; flex:0 0 auto; width:150px; height:150px">
                    <svg viewBox="0 0 120 120" style="width:100%; height:100%; display:block"
                         role="img" aria-label="What the money went on">
                        {
                            let mut before = 0i64;
                            slices.get_value()
                                .into_iter()
                                .map(|s| {
                                    let (length, offset) = arc(s.hundredths, before, CIRCUMFERENCE);
                                    before += s.hundredths;
                                    let key = s.row.key.clone();
                                    let picked = key.clone();
                                    let mine = key.clone();
                                    let label = format!(
                                        "{}, {:.1}%",
                                        s.row.label,
                                        s.hundredths as f64 * 100.0 / 36000.0
                                    );
                                    view! {
                                        <circle cx="60" cy="60" r=R fill="none"
                                                stroke=s.colour.clone()
                                                stroke-width=move || {
                                                    // The chosen one steps forward. Nothing
                                                    // moves, so nothing is measured wrongly
                                                    // because of it.
                                                    if chosen.get() == mine {
                                                        CHOSEN_STROKE.to_string()
                                                    } else {
                                                        STROKE.to_string()
                                                    }
                                                }
                                                stroke-dasharray=format!(
                                                    "{length:.3} {:.3}", CIRCUMFERENCE - length)
                                                stroke-dashoffset=format!("{:.3}", -offset)
                                                transform="rotate(-90 60 60)"
                                                opacity=move || {
                                                    let c = chosen.get();
                                                    if c.is_empty() || c == key { "1" } else { ".3" }
                                                }
                                                style="cursor:pointer; transition:stroke-width .12s ease, opacity .12s ease"
                                                aria-label=label
                                                on:click=move |_| {
                                                    chosen.update(|c| {
                                                        if *c == picked {
                                                            c.clear()
                                                        } else {
                                                            *c = picked.clone()
                                                        }
                                                    })
                                                } />
                                    }
                                })
                                .collect_view()
                        }
                    </svg>
                    // The hole does the work the "Total" line used to do, and says what the
                    // chosen slice is worth when there is one.
                    <div style="position:absolute; inset:0; display:flex; flex-direction:column;
                                align-items:center; justify-content:center; text-align:center;
                                pointer-events:none; padding:0 22px">
                        <div style="font-size:15px; font-weight:700; line-height:1.15">
                            {move || match chosen_amount() {
                                Some((_, amount, _)) => money(amount),
                                None => money(total),
                            }}
                        </div>
                        <div class="tcn-hint" style="margin:2px 0 0; font-size:11px;
                                                     overflow:hidden; text-overflow:ellipsis;
                                                     white-space:nowrap; max-width:100%">
                            {move || match chosen_amount() {
                                Some((label, _, hundredths)) => format!(
                                    "{label} · {:.1}%", hundredths as f64 * 100.0 / 36000.0),
                                None => format!("total {}", unit_centre.clone()),
                            }}
                        </div>
                    </div>
                </div>

                <div style="flex:1 1 210px; min-width:0; display:flex; flex-direction:column; gap:2px">
                    {slices.get_value()
                        .into_iter()
                        .map(|s| {
                            let percent = s.hundredths as f64 * 100.0 / 36000.0;
                            let key = s.row.key.clone();
                            let picked = key.clone();
                            let lit = key.clone();
                            let inside = key.clone();
                            let unit = unit_rows.clone();
                            let colour = s.colour.clone();
                            let bar = s.colour.clone();
                            view! {
                                <div style="display:flex; align-items:center; gap:4px">
                                    <button type="button" class="tcn-compo-row"
                                            aria-pressed=move || (chosen.get() == key).to_string()
                                            style=move || format!(
                                                "flex:1 1 auto; min-width:0; display:flex; \
                                                 align-items:center; gap:7px; border:0; \
                                                 background:{}; border-radius:8px; \
                                                 padding:5px 7px; cursor:pointer; \
                                                 font:inherit; text-align:left",
                                                if chosen.get() == lit {
                                                    "var(--tcn-surface-2)"
                                                } else {
                                                    "transparent"
                                                })
                                            on:click=move |_| {
                                                chosen.update(|c| {
                                                    if *c == picked { c.clear() } else { *c = picked.clone() }
                                                })
                                            }>
                                        <span style=format!(
                                            "flex:0 0 auto; width:10px; height:10px; \
                                             border-radius:3px; background:{colour}")></span>
                                        <span style="flex:1 1 auto; min-width:0">
                                            <span style="display:block; overflow:hidden;
                                                         text-overflow:ellipsis; white-space:nowrap;
                                                         font-weight:600; font-size:13px">
                                                {s.row.label.clone()}
                                            </span>
                                            // The proportion, drawn. On a phone the ring is
                                            // small and the list is what actually gets read.
                                            <span style="display:block; height:4px; margin-top:3px;
                                                         border-radius:999px;
                                                         background:var(--tcn-surface-2)">
                                                <span style=format!(
                                                    "display:block; height:100%; width:{percent:.1}%; \
                                                     border-radius:999px; background:{bar}")></span>
                                            </span>
                                        </span>
                                        <span style="flex:0 0 auto; text-align:right; font-size:12px">
                                            <span style="display:block; font-weight:600">
                                                {money(s.row.amount)}
                                                {(!unit.is_empty())
                                                    .then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                            </span>
                                            <span class="tcn-hint" style="margin:0; font-size:11px">
                                                {format!("{percent:.1}%")}
                                            </span>
                                        </span>
                                    </button>
                                    {s.row.children.then(|| view! {
                                        <button type="button" class="tcn-btn tcn-btn-sm"
                                                style="flex:0 0 auto; padding:4px 7px"
                                                title=format!("What is inside {}", s.row.label)
                                                aria-label=format!("What is inside {}", s.row.label)
                                                on:click=move |_| into.run(inside.clone())>
                                            <crate::icon::Icon name="chevron-right" />
                                        </button>
                                    })}
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(pairs: &[(&str, i64)]) -> Vec<(String, Cents)> {
        pairs
            .iter()
            .map(|(name, amount)| ((*name).to_owned(), Cents(*amount)))
            .collect()
    }

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
        let data = by_head(&rows(&[("a", 100), ("b", 33), ("c", 33), ("d", 33)]));
        let wedges = slices(&data, 1.63);
        assert_eq!(
            wedges.iter().map(|s| s.hundredths).sum::<i64>(),
            36000,
            "a gap in the circle reads as a bug, not as rounding"
        );
        assert_eq!(wedges[0].row.label, "a", "largest first");

        // Nothing spent anywhere still draws something, and says what it is.
        let nothing = by_head(&rows(&[("a", 0), ("b", 0)]));
        let wedges = slices(&nothing, 1.63);
        assert_eq!(wedges.iter().map(|s| s.hundredths).sum::<i64>(), 36000);
        assert!(wedges[0].row.label.ends_with("[=0]"), "{:?}", wedges[0].row);
    }

    #[test]
    fn a_head_with_several_children_stands_for_them() {
        let data = rows(&[
            ("Food / lunch", 100),
            ("Food / dinner", 200),
            ("Taxi / airport", 50),
        ]);
        let top = by_head(&data);
        assert_eq!(top[0].key, "Food");
        assert_eq!(top[0].amount, Cents(300));
        assert!(top[0].children, "there is something to go into");
        // One child and no siblings keeps its own name: calling it "Taxi" would say there is
        // more under it than there is - and there is nothing to open.
        assert_eq!(top[1].key, "Taxi / airport");
        assert!(!top[1].children);
    }

    #[test]
    fn choosing_a_head_chooses_everything_under_it() {
        // This is what makes the ring a filter: the totals below ask this question of every
        // spending, and a head has to answer for its children.
        assert!(matches("Food", "Food / lunch"));
        assert!(matches("Food", "Food"));
        assert!(!matches("Food", "Taxi / airport"));
        assert!(matches("", "anything at all"));
        assert!(matches("Taxi / airport", "Taxi / airport"));
        assert!(
            !matches("Taxi / airport", "Taxi / home"),
            "a full name is itself, not its head"
        );
    }

    #[test]
    fn the_ring_fits_inside_its_box() {
        // A stroke straddles the path, so the ring reaches r + half of it - and the chosen
        // slice is drawn thicker. Exceed the half-box and the viewport clips the circle into
        // a square, which is what it did at r = 54.
        const HALF_BOX: f64 = 60.0;
        assert!(
            44.0 + 26.0 / 2.0 <= HALF_BOX,
            "the widest stroke has to stay inside the viewBox"
        );
    }

    #[test]
    fn the_wedges_are_laid_end_to_end() {
        // Each arc starts where the last one stopped, and the last one closes the circle.
        let c = 100.0;
        let (first_len, first_off) = arc(9000, 0, c);
        let (second_len, second_off) = arc(27000, 9000, c);
        assert_eq!((first_len, first_off), (25.0, 0.0));
        assert_eq!((second_len, second_off), (75.0, 25.0));
    }
}
