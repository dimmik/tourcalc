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

/// The geometry, in the viewBox's own units, so the ring scales with the box and nothing
/// here has to know how many pixels it ends up as.
///
/// The radius leaves room for the stroke: a stroke straddles the path, so the ring reaches
/// r + half of it, and the chosen band is both thicker and pushed out. At r = 54 with a
/// 20-wide stroke that came to 67 against a half-box of 60, and the viewport cut the circle
/// into a square - plain in a picture, invisible in the numbers.
const R: f64 = 40.0;
const STROKE: f64 = 18.0;
/// The chosen category steps forward, and is drawn wider so the things inside it have room.
const CHOSEN_R: f64 = 42.0;
const CHOSEN_STROKE: f64 = 22.0;
/// The white hair between the things inside a chosen category, in hundredths of a degree.
const GAP: i64 = 90;
const CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * R;

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
    /// What is inside it, largest first. Empty for a row that is only itself.
    pub inside: Vec<Row>,
}

impl Row {
    pub fn leaf(key: String, label: String, amount: Cents) -> Row {
        Row {
            key,
            label,
            amount,
            inside: Vec::new(),
        }
    }

    pub fn has_inside(&self) -> bool {
        !self.inside.is_empty()
    }
}

/// One wedge: its row, how much of the circle it takes in hundredths of a degree, and the
/// colour it is drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    pub row: Row,
    pub hundredths: i64,
    pub colour: String,
}

/// What is left after the head: "lunch" out of "Food / lunch", and the whole of a name with
/// no head to speak of.
pub fn tail_of(name: &str) -> String {
    match name.split_once(SUBCAT) {
        Some((_, tail)) => tail.trim().to_owned(),
        None => name.trim().to_owned(),
    }
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
                [only] => Row::leaf(only.0.trim().to_owned(), only.0.trim().to_owned(), amount),
                _ => {
                    // The children are carried, not just counted: choosing the head divides
                    // its arc into them, and they are named by their tail - the head is
                    // already the row they sit under.
                    let mut inside: Vec<Row> = items
                        .iter()
                        .map(|(name, amount)| {
                            Row::leaf(name.trim().to_owned(), tail_of(name), *amount)
                        })
                        .collect();
                    inside.sort_by_key(|r| -r.amount.0);
                    Row {
                        key: head.clone(),
                        label: head,
                        amount,
                        inside,
                    }
                }
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
                <div class="tcw-ring" style="position:relative; flex:0 0 auto">
                    <svg viewBox="0 0 120 120" style="width:100%; height:100%; display:block"
                         role="img" aria-label="What the money went on">
                        // The whole ring is redrawn when the choice changes, because the
                        // choice changes what the ring is made of: the chosen category is
                        // not one arc any more but one arc per thing inside it.
                        {move || {
                            let picked = chosen.get();
                            let mut before = 0i64;
                            let mut arcs: Vec<AnyView> = Vec::new();
                            for s in slices.get_value() {
                                let start = before;
                                before += s.hundredths;
                                let mine = picked == s.row.key;
                                let faded = !picked.is_empty() && !mine;

                                if mine && s.row.has_inside() {
                                    arcs.extend(inside_arcs(&s, start, chosen));
                                } else {
                                    let (length, offset) = arc(s.hundredths, start, CIRCUMFERENCE);
                                    let key = s.row.key.clone();
                                    arcs.push(
                                        view! {
                                            <circle cx="60" cy="60" r=R fill="none"
                                                    stroke=s.colour.clone()
                                                    stroke-width=STROKE
                                                    stroke-dasharray=dashes(length)
                                                    stroke-dashoffset=format!("{:.3}", -offset)
                                                    transform="rotate(-90 60 60)"
                                                    opacity=if faded { ".3" } else { "1" }
                                                    style="cursor:pointer; transition:opacity .12s ease"
                                                    on:click=move |_| pick(chosen, &key)>
                                                <title>{arc_title(&s.row, s.hundredths)}</title>
                                            </circle>
                                        }
                                        .into_any(),
                                    );
                                }
                            }
                            arcs
                        }}
                    </svg>
                    // The hole does the work the "Total" line used to do, and says what the
                    // chosen slice is worth when there is one.
                    <div style="position:absolute; inset:0; display:flex; flex-direction:column;
                                align-items:center; justify-content:center; text-align:center;
                                pointer-events:none; padding:0 24px">
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
                            let shown = s.row.label.clone();
                            let children = s.row.inside.clone();
                            let parent_colour = s.colour.clone();
                            let open = key.clone();
                            let unit_children = unit_rows.clone();
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
                                            on:click=move |_| pick(chosen, &picked)>
                                        <span style=format!(
                                            "flex:0 0 auto; width:10px; height:10px; \
                                             border-radius:3px; background:{colour}")></span>
                                        <span style="flex:1 1 auto; min-width:0">
                                            <span style="display:block; overflow:hidden;
                                                         text-overflow:ellipsis; white-space:nowrap;
                                                         font-weight:600; font-size:13px">
                                                {shown}
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
                                    {s.row.has_inside().then(|| view! {
                                        <button type="button" class="tcn-btn tcn-btn-sm"
                                                style="flex:0 0 auto; padding:4px 7px"
                                                title=format!("Show only {}", s.row.label)
                                                aria-label=format!("Show only {}", s.row.label)
                                                on:click=move |_| into.run(inside.clone())>
                                            <crate::icon::Icon name="chevron-right" />
                                        </button>
                                    })}
                                </div>

                                // What is inside it, once it is chosen: the same division the
                                // ring has just made, in words. The ring shows the shape of
                                // it; this says which is which.
                                {move || {
                                    if chosen.get() != open || children.is_empty() {
                                        return ().into_any();
                                    }
                                    let whole: i64 = children.iter().map(|c| c.amount.0.abs()).sum();
                                    let unit = unit_children.clone();
                                    let parent = parent_colour.clone();
                                    let n = children.len();
                                    children
                                        .iter()
                                        .enumerate()
                                        .map(|(i, child)| {
                                            let share = if whole == 0 {
                                                0.0
                                            } else {
                                                child.amount.0.abs() as f64 * 100.0 / whole as f64
                                            };
                                            let unit = unit.clone();
                                            view! {
                                                <div style="display:flex; align-items:center;
                                                            gap:7px; padding:3px 7px 3px 24px">
                                                    <span style=format!(
                                                        "flex:0 0 auto; width:8px; height:8px; \
                                                         border-radius:2px; background:{}",
                                                        shade(&parent, i, n))></span>
                                                    <span style="flex:1 1 auto; min-width:0;
                                                                 overflow:hidden;
                                                                 text-overflow:ellipsis;
                                                                 white-space:nowrap; font-size:12px">
                                                        {child.label.clone()}
                                                    </span>
                                                    <span style="flex:0 0 auto; font-size:12px">
                                                        {money(child.amount)}
                                                        {(!unit.is_empty())
                                                            .then(|| view! { <small>"\u{a0}" {unit}</small> })}
                                                    </span>
                                                    <span class="tcn-hint"
                                                          style="margin:0; font-size:11px; flex:0 0 auto">
                                                        {format!("{share:.0}%")}
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()
                                        .into_any()
                                }}
                            }
                        })
                        .collect_view()}
                </div>
            </div>
        </div>
    }
}

/// Choosing, and un-choosing by choosing again.
fn pick(chosen: RwSignal<String>, key: &str) {
    let key = key.to_owned();
    chosen.update(|c| {
        if *c == key {
            c.clear()
        } else {
            *c = key
        }
    })
}

fn dashes(length: f64) -> String {
    format!("{length:.3} {:.3}", CIRCUMFERENCE - length)
}

fn arc_title(row: &Row, hundredths: i64) -> String {
    format!(
        "{}, {:.1}%",
        row.label,
        hundredths as f64 * 100.0 / 36000.0
    )
}

/// The chosen category, drawn as the things inside it.
///
/// They take the arc their parent had, split in its proportions, in shades of its colour and
/// with a hair of white between them. The ring still adds up to the whole tour - which is the
/// difference between this and going inside, where the children get the whole circle.
fn inside_arcs(slice: &Slice, start: i64, chosen: RwSignal<String>) -> Vec<AnyView> {
    let children = &slice.row.inside;
    let whole: i64 = children.iter().map(|c| c.amount.0.abs()).sum();
    let n = children.len();
    let mut at = start;
    let mut left = slice.hundredths;
    let mut out: Vec<AnyView> = Vec::new();

    for (i, child) in children.iter().enumerate() {
        // The last one takes what is left, so the parent's arc is covered exactly however
        // the division rounded.
        let span = if i + 1 == n {
            left
        } else if whole > 0 {
            (child.amount.0.abs() as f64 / whole as f64 * slice.hundredths as f64) as i64
        } else {
            slice.hundredths / n as i64
        };
        let (length, offset) = arc(span, at, CIRCUMFERENCE);
        // A hair of white, and only where there is room for one: on a slice of two degrees a
        // gap is the whole slice.
        let drawn = if span > GAP * 2 {
            arc(span - GAP, at, CIRCUMFERENCE).0
        } else {
            length
        };
        let colour = shade(&slice.colour, i, n);
        let key = slice.row.key.clone();
        out.push(
            view! {
                <circle cx="60" cy="60" r=CHOSEN_R fill="none" stroke=colour
                        stroke-width=CHOSEN_STROKE
                        stroke-dasharray=dashes(drawn)
                        stroke-dashoffset=format!("{:.3}", -offset)
                        transform="rotate(-90 60 60)"
                        style="cursor:pointer"
                        on:click=move |_| pick(chosen, &key)>
                    <title>{arc_title(child, span)}</title>
                </circle>
            }
            .into_any(),
        );

        // Its name along the band, where the band is long enough to hold it.
        if worth_labelling(span, &child.label) {
            let id = format!("tcw-arc-{}-{i}", slice.row.key.replace(' ', "-"));
            let href = format!("#{id}");
            out.push(
                view! {
                    <defs>
                        <path id=id fill="none"
                              d=label_path(at, span - GAP, CHOSEN_R) />
                    </defs>
                    <text font-size="6.5" font-weight="600" fill="#fff"
                          style="pointer-events:none; letter-spacing:.2px">
                        <textPath href=href startOffset="50%" text-anchor="middle"
                                  dominant-baseline="middle">
                            {child.label.clone()}
                        </textPath>
                    </text>
                }
                .into_any(),
            );
        }
        at += span;
        left -= span;
    }
    out
}

/// The path a label runs along: the middle of the band, between the two angles a wedge
/// covers.
///
/// Written so the text comes out the right way up. Following the arc in the direction of
/// travel puts the letters outside-up on the top of the circle and upside-down at the
/// bottom, so a wedge whose middle is in the bottom half gets its path drawn backwards.
pub fn label_path(start: i64, span: i64, r: f64) -> String {
    let point = |hundredths: i64| {
        let a = (hundredths as f64 / 100.0 - 90.0).to_radians();
        (60.0 + r * a.cos(), 60.0 + r * a.sin())
    };
    let (x0, y0) = point(start);
    let (x1, y1) = point(start + span);
    let large = if span > 18000 { 1 } else { 0 };
    let middle = (start + span / 2) % 36000;
    if (9000..27000).contains(&middle) {
        // Bottom half: the same arc, walked the other way.
        format!("M {x1:.2} {y1:.2} A {r} {r} 0 {large} 0 {x0:.2} {y0:.2}")
    } else {
        format!("M {x0:.2} {y0:.2} A {r} {r} 0 {large} 1 {x1:.2} {y1:.2}")
    }
}

/// Whether a wedge is worth writing on: long enough for the word to fit along it, and a word
/// short enough to try. Everything else is named in the list beside the ring, which is where
/// names belong.
pub fn worth_labelling(span: i64, label: &str) -> bool {
    span >= 4200 && label.chars().count() <= 11
}

/// A child's colour: the parent's, lighter or darker by where it sits in the family.
///
/// The hue is kept, so the whole family still reads as one block of the ring - which is the
/// point of dividing it in place rather than replacing it.
pub fn shade(parent: &str, i: usize, n: usize) -> String {
    let Some(rgb) = crate::ui::parse_hex(parent) else {
        return parent.to_owned();
    };
    if n < 2 {
        return parent.to_owned();
    }
    let (h, s, l) = crate::ui::to_hsl(rgb);
    let step = 0.34 / (n - 1) as f64;
    let lightness = (l - 0.17 + step * i as f64).clamp(0.22, 0.82);
    crate::ui::from_hsl(h, s, lightness)
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
        assert!(top[0].has_inside(), "there is something to go into");
        assert_eq!(
            top[0].inside.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            vec!["dinner", "lunch"],
            "carried with it, largest first, named by their tails"
        );
        // One child and no siblings keeps its own name: calling it "Taxi" would say there is
        // more under it than there is - and there is nothing to open.
        assert_eq!(top[1].key, "Taxi / airport");
        assert!(!top[1].has_inside());
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
        // A stroke straddles the path, so a band reaches its radius plus half its width -
        // and the chosen one is both wider and pushed out. Exceed the half-box and the
        // viewport clips the circle into a square, which is what it did at r = 54.
        const HALF_BOX: f64 = 60.0;
        assert!(R + STROKE / 2.0 <= HALF_BOX, "the plain ring");
        assert!(
            CHOSEN_R + CHOSEN_STROKE / 2.0 <= HALF_BOX,
            "and the chosen band, which is the one that overflowed"
        );
    }

    #[test]
    fn a_family_keeps_its_hue() {
        // Dividing a category in place only works if the pieces still read as that
        // category: same hue, different lightness.
        let kids: Vec<String> = (0..4).map(|i| shade("#3566EE", i, 4)).collect();
        let hue = |c: &str| crate::ui::to_hsl(crate::ui::parse_hex(c).unwrap()).0.round();
        assert!(
            kids.iter().all(|c| hue(c) == hue("#3566EE")),
            "the hue is the family name: {kids:?}"
        );
        assert_eq!(
            kids.len(),
            kids.iter().collect::<std::collections::HashSet<_>>().len(),
            "and they still have to be told apart: {kids:?}"
        );
        // On its own there is nothing to spread, and nothing to change.
        assert_eq!(shade("#3566EE", 0, 1), "#3566EE");
    }

    #[test]
    fn a_label_is_written_the_right_way_up() {
        // Following the arc in the direction of travel reads correctly on the top of the
        // circle and upside-down at the bottom, so the bottom half is walked backwards. The
        // sweep flag is what says which way round it went.
        let top = label_path(0, 6000, 42.0);
        let bottom = label_path(15000, 6000, 42.0);
        assert!(top.contains(" 1 "), "top half runs with the arc: {top}");
        assert!(bottom.contains(" 0 "), "bottom half runs against it: {bottom}");
        // Half a circle or more needs the large-arc flag, or SVG draws the short way round.
        // Taken over the top, where the direction is the plain one - a wide wedge low down
        // is walked backwards like any other, and would say "1 0".
        let wide_over_the_top = label_path(27000, 20000, 42.0);
        assert!(
            wide_over_the_top.contains(" 1 1 "),
            "large arc, drawn forwards: {wide_over_the_top}"
        );
        assert!(label_path(0, 20000, 42.0).contains(" 1 0 "), "and backwards low down");
    }

    #[test]
    fn only_a_wedge_with_room_gets_a_name_on_it() {
        // A name on a sliver is a name on top of its neighbours. Everything is in the list
        // beside the ring either way.
        assert!(worth_labelling(9000, "готовка"));
        assert!(!worth_labelling(600, "готовка"), "two degrees of arc");
        assert!(
            !worth_labelling(18000, "a category with a very long name"),
            "half the circle is still not enough for that"
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
