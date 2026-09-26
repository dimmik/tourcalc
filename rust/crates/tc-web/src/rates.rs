//! "Today's rate": a currency's worth filled in from the market rate, in the currencies dialog.
//!
//! The browser asks `open.er-api.com` itself - not through our server: the source's terms let
//! anyone read it but not pass it on, and it needs no key. One request, in dollars, serves
//! every currency (a cross rate is a division), and it is kept for as long as the source says
//! it is current - it updates once a day.
//!
//! Nothing here changes the tour. It works out numbers for the dialog's rows, which somebody
//! then saves or not. Everything but the request is plain functions, tested natively.
//!
//! **Which currency is which.** Tours name currencies however they liked - `EUR`, `Eur`,
//! `Din`, `EURc` - and the `id` is whatever the C# made of the first name. So a currency is
//! recognised by its `id` first and its name second: an ISO code, a synonym, or either with a
//! trailing `c` for a hundredth ("EURc" - from before a currency could have cents itself).
//!
//! **Against what.** The cheapest other currency the source knows keeps its worth; the rest
//! are worked out from it. Rounded to four significant figures - 117,5 dinars to the euro, not
//! 117,563 - which is closer than anybody settling a trip cares about.
//!
//! **Worths and rates.** A tour stores each currency's *worth*, an integer on a scale of its own
//! (the dinar 1 000, the euro 117 500); only the ratio means anything. The dialog shows rates
//! instead - "1 EUR = 117,5 RSD" - and chooses the scale itself (`room`), so nobody has to be
//! asked to multiply anything.

use std::collections::HashMap;

/// The source, as its terms ask to be credited.
pub const SOURCE_NAME: &str = "ExchangeRate-API";
pub const SOURCE_LINK: &str = "https://www.exchangerate-api.com";
const URL: &str = "https://open.er-api.com/v6/latest/USD";
const CACHE: &str = "__tcw_rates_USD";

/// A currency as the source knows it: its ISO code, and how many of the tour's units make one
/// of it - 1 for "EUR", 100 for "EURc".
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub iso: String,
    pub per: i64,
}

/// The currencies the source knows, kept with the app (`currency_codes.txt`): what the
/// currencies dialog suggests when a currency is being named.
const CODES: &str = include_str!("currency_codes.txt");

pub fn known_codes() -> Vec<&'static str> {
    CODES
        .lines()
        .filter(|l| !l.starts_with('#'))
        .flat_map(str::split_whitespace)
        .collect()
}

fn is_known_code(code: &str) -> bool {
    static KNOWN: std::sync::OnceLock<std::collections::HashSet<&'static str>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| known_codes().into_iter().collect()).contains(code)
}

thread_local! {
    static SUGGESTIONS: std::cell::OnceCell<Vec<(String, String)>> = const { std::cell::OnceCell::new() };
}

/// The euro's countries: its code says none of them, and somebody typing "Герм" means it.
/// With Montenegro and Kosovo, which use it without being in the eurozone.
const EURO_COUNTRIES: &[&str] = &[
    "AT", "BE", "HR", "CY", "EE", "FI", "FR", "DE", "GR", "IE", "IT", "LV", "LT", "LU", "MT",
    "NL", "PT", "SK", "SI", "ES", "ME", "XK",
];

/// The countries a code is the currency of, as region codes: the first two letters of an ISO
/// code are its country's - BY for BYN, BD for BDT - except for the currencies several
/// countries share, listed here so that "Ecuador" finds the dollar and "Senegal" the franc.
/// Other X… codes belong to no country (XDR).
fn countries_of(code: &str) -> Vec<&str> {
    match code {
        "EUR" => EURO_COUNTRIES.to_vec(),
        "USD" => vec!["US", "EC", "SV", "PA", "TL", "PR", "FM", "MH", "PW", "TC", "VG", "BQ"],
        "XOF" => vec!["BJ", "BF", "CI", "GW", "ML", "NE", "SN", "TG"],
        "XAF" => vec!["CM", "CF", "TD", "CG", "GQ", "GA"],
        "XCD" => vec!["AG", "DM", "GD", "KN", "LC", "VC", "AI", "MS"],
        "XPF" => vec!["PF", "NC", "WF"],
        "AUD" => vec!["AU", "NR", "KI", "TV"],
        "NZD" => vec!["NZ", "CK", "NU", "PN", "TK"],
        "CHF" => vec!["CH", "LI"],
        "DKK" => vec!["DK", "GL"],
        _ if code.starts_with('X') => Vec::new(),
        _ => vec![&code[..2]],
    }
}

/// Every code the source knows, with what it is called - its name and its country's, in the
/// reader's language first and then in the other: "BYN", "белорусский рубль · Беларусь ·
/// Belarusian Ruble · Belarus". A suggestion list matches the words as well as the code, so
/// "Белар", "Belar" and "Бангла" find theirs, "рубль" finds both roubles, and nobody has to
/// look up that the Belarusian rouble is BYN. The names are the browser's own
/// (`Intl.DisplayNames`); where it has none, the code alone. Worked out once.
pub fn suggestions() -> Vec<(String, String)> {
    SUGGESTIONS.with(|s| {
        s.get_or_init(|| {
            let mine = crate::i18n::lang();
            let order: Vec<&str> = std::iter::once(mine.code())
                .chain(crate::i18n::Lang::ALL.iter().map(|l| l.code()).filter(|c| *c != mine.code()))
                .collect();
            let namers: Vec<_> = order
                .iter()
                .map(|lang| (display_names("currency", lang), display_names("region", lang)))
                .collect();
            known_codes()
                .into_iter()
                .map(|code| {
                    let mut parts: Vec<String> = Vec::new();
                    for (currency, region) in &namers {
                        let name = currency.as_ref().and_then(|n| n(code));
                        let places: Vec<String> = countries_of(code)
                            .into_iter()
                            .filter_map(|r| region.as_ref().and_then(|n| n(r)))
                            .collect();
                        for part in name.into_iter().chain((!places.is_empty()).then(|| places.join(", "))) {
                            if !parts.contains(&part) {
                                parts.push(part);
                            }
                        }
                    }
                    (code.to_owned(), parts.join(" · "))
                })
                .collect()
        })
        .clone()
    })
}

/// The suggestions a name typed and left matches - by code, currency or country, in either
/// language, any case: for when the list under the box did not help (some browsers match its
/// codes only). At most `limit`.
pub fn matching(text: &str, limit: usize) -> Vec<(String, String)> {
    let wanted = text.trim().to_lowercase();
    if wanted.chars().count() < 2 {
        return Vec::new();
    }
    suggestions()
        .into_iter()
        .filter(|(code, label)| format!("{code} {label}").to_lowercase().contains(&wanted))
        .take(limit)
        .collect()
}

/// The browser's names of currencies or of regions, in `lang`.
#[cfg(target_arch = "wasm32")]
fn display_names(kind: &str, lang: &str) -> Option<impl Fn(&str) -> Option<String>> {
    use wasm_bindgen::{JsCast, JsValue};
    let intl = js_sys::Reflect::get(&js_sys::global(), &"Intl".into()).ok()?;
    let ctor: js_sys::Function = js_sys::Reflect::get(&intl, &"DisplayNames".into()).ok()?.dyn_into().ok()?;
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"type".into(), &kind.into()).ok()?;
    let locales = js_sys::Array::of1(&lang.into());
    let namer = js_sys::Reflect::construct(&ctor, &js_sys::Array::of2(&locales, &options)).ok()?;
    let of: js_sys::Function = js_sys::Reflect::get(&namer, &"of".into()).ok()?.dyn_into().ok()?;
    Some(move |code: &str| {
        of.call1(&namer, &JsValue::from_str(code))
            .ok()?
            .as_string()
            // Where it does not know the currency, it says the code back.
            .filter(|n| n != code)
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn display_names(_kind: &str, _lang: &str) -> Option<fn(&str) -> Option<String>> {
    None
}

/// Names that are not ISO codes but mean one.
const SYNONYMS: &[(&str, &str)] = &[
    ("DIN", "RSD"),
    ("ДИН", "RSD"),
    ("ДИНАР", "RSD"),
    ("LEV", "BGN"),
    ("ЛЕВ", "BGN"),
    ("KM", "BAM"),
    ("КМ", "BAM"),
    ("РУБ", "RUB"),
    ("₽", "RUB"),
    ("ЕВРО", "EUR"),
    ("EURO", "EUR"),
    ("€", "EUR"),
    ("$", "USD"),
];

/// What one piece of text - an id or a name - says the currency is, if anything.
fn unit_of(text: &str) -> Option<Unit> {
    let up = text.trim().to_uppercase();
    let whole = |s: &str| -> Option<String> {
        if let Some((_, iso)) = SYNONYMS.iter().find(|(k, _)| *k == s) {
            return Some((*iso).to_owned());
        }
        // A code only if it is one the source knows: "ABC" or "Fun" is somebody's own currency,
        // to be told "not found" when named - not a code the button then has no rate for.
        is_known_code(s).then(|| s.to_owned())
    };
    if let Some(iso) = whole(&up) {
        return Some(Unit { iso, per: 1 });
    }
    // "EURc", "BAMc", "LEVc", "EUR¢": the hundredth of what is before it.
    let base = up.strip_suffix('C').or_else(|| up.strip_suffix('¢'))?;
    whole(base.trim()).map(|iso| Unit { iso, per: 100 })
}

/// What a currency may be, most likely first: by its id, then by its name.
pub fn units_of(id: &str, name: &str) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    for u in [unit_of(id), unit_of(name)].into_iter().flatten() {
        if !out.contains(&u) {
            out.push(u);
        }
    }
    out
}

/// The id a new currency gets from its name: the ISO code when the name is one ("EUR", "Din"
/// → "RSD"), so that the next tour's EUR is the same `EUR`. Not for a hundredth - "EURc" is
/// what the "with cents" box replaces - and not when the code is taken.
pub fn id_for_new(name: &str, taken: &[&str]) -> Option<String> {
    let unit = unit_of(name).filter(|u| u.per == 1)?;
    (!taken.iter().any(|t| t.eq_ignore_ascii_case(&unit.iso))).then_some(unit.iso)
}

/// The source's answer: how many of each currency one dollar buys, and when it was so.
#[derive(Clone, Debug, PartialEq)]
pub struct Rates {
    pub per_usd: HashMap<String, f64>,
    /// When the source last updated, seconds since the epoch - "rate of 25.09".
    pub updated: i64,
    /// When it will next - until then there is no point asking again.
    pub next: i64,
}

pub fn parse(json: &str) -> Option<Rates> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("result")?.as_str()? != "success" {
        return None;
    }
    let per_usd: HashMap<String, f64> = v
        .get("rates")?
        .as_object()?
        .iter()
        .filter_map(|(k, r)| r.as_f64().filter(|r| *r > 0.0).map(|r| (k.clone(), r)))
        .collect();
    Some(Rates {
        per_usd,
        updated: v.get("time_last_update_unix").and_then(|t| t.as_i64()).unwrap_or(0),
        next: v.get("time_next_update_unix").and_then(|t| t.as_i64()).unwrap_or(0),
    })
}

/// Why no rate: said as it is, and the worth left alone.
#[derive(Clone, Debug, PartialEq)]
pub enum Why {
    /// Not a currency anything here recognises ("Chips") - its name.
    Unknown(String),
    /// Recognised, but the source has no rate for it - its code.
    NotAtSource(String),
    /// No other currency the source knows, to work it out against.
    NothingToCompare,
    /// No answer from the source, and none kept from before.
    Offline,
    /// The number would not fit.
    OutOfRange,
}

/// A row of the dialog, as far as rates go.
#[derive(Clone, Debug)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub rate: i64,
    /// Already one of the tour's currencies, not one being added: only such a worth means
    /// anything - a new row's is the default until somebody changes it.
    pub saved: bool,
}

/// Of `(row, worth, saved)`, the one the others are worked out from: the cheapest of the
/// tour's own currencies, and of the new ones only when there are no others. A currency just
/// added sits at the default 10 000, which says nothing about what it is worth - taken for
/// the base, it lost its own button (a PLN added beside a euro at 1 000 000).
fn cheapest_of(candidates: impl Iterator<Item = (usize, i64, bool)> + Clone) -> Option<usize> {
    let saved = candidates.clone().filter(|(_, _, s)| *s).min_by_key(|(_, rate, _)| *rate);
    saved
        .or_else(|| candidates.min_by_key(|(_, rate, _)| *rate))
        .map(|(i, _, _)| i)
}

/// Which unit of `row` the source has, or why none.
fn resolve(row: &Row, rates: &Rates) -> Result<Unit, Why> {
    let all = units_of(&row.id, &row.name);
    let Some(first) = all.first() else {
        return Err(Why::Unknown(row.name.trim().to_owned()));
    };
    all.iter()
        .find(|u| rates.per_usd.contains_key(&u.iso))
        .cloned()
        .ok_or_else(|| Why::NotAtSource(first.iso.clone()))
}

/// The row every other is worked out against: the cheapest the source knows, other than
/// `group` (the currency asked about, in any of its units).
pub fn reference(rows: &[Row], group: &str, rates: &Rates) -> Option<usize> {
    let known: Vec<(usize, i64, bool)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.rate > 0 && !r.name.trim().is_empty())
        .filter_map(|(i, r)| resolve(r, rates).ok().map(|u| (i, r, u)))
        .filter(|(_, _, u)| u.iso != group)
        .map(|(i, r, _)| (i, r.rate, r.saved))
        .collect();
    cheapest_of(known.into_iter())
}

/// What goes where the button would be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// "Today's rate".
    Button,
    /// The one the others are worked out from - said, so its missing button is not a puzzle.
    Base,
    /// Nothing: a blank row, or nothing to work it out against.
    Nothing,
}

/// Whether a row gets the button: not the one the others are worked out against, and not in
/// a tour where there is nothing else to work it out against. Decided without the rates -
/// the button is there before anybody asks the source.
pub fn place(rows: &[Row], at: usize) -> Place {
    let Some(row) = rows.get(at) else { return Place::Nothing };
    if row.name.trim().is_empty() {
        return Place::Nothing;
    }
    let known: Vec<(usize, i64, bool, String)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.rate > 0 && !r.name.trim().is_empty())
        .filter_map(|(i, r)| {
            units_of(&r.id, &r.name).first().map(|u| (i, r.rate, r.saved, u.iso.clone()))
        })
        .collect();
    let mine = units_of(&row.id, &row.name).first().map(|u| u.iso.clone());
    // Alone, or only beside its own cents: nothing to work it out against. An unrecognised
    // one ("Chips") beside others keeps the button - pressed, it says it is not recognised.
    if !known.iter().any(|(_, _, _, iso)| Some(iso) != mine.as_ref()) {
        return Place::Nothing;
    }
    // The one the others are worked out against - chosen as `reference` chooses it.
    let base = cheapest_of(known.iter().map(|(i, rate, saved, _)| (*i, *rate, *saved)));
    if base == Some(at) {
        Place::Base
    } else {
        Place::Button
    }
}

/// The row the dialog reads every other rate against - "1 EUR = 117,5 **RSD**": the one
/// `place` calls the base, and when no currency is recognised at all, the cheapest of the
/// tour's own (chips against chips still have a rate). `None` only for a list of blank rows.
pub fn display_base(rows: &[Row]) -> Option<usize> {
    if let Some(base) = (0..rows.len()).find(|i| place(rows, *i) == Place::Base) {
        return Some(base);
    }
    cheapest_of(
        rows.iter()
            .enumerate()
            .filter(|(_, r)| r.rate > 0 && !r.name.trim().is_empty())
            .map(|(i, r)| (i, r.rate, r.saved)),
    )
}

/// The power of ten to multiply every worth by before today's rate for row `at` is worked
/// out: its base at 10 000 at least, so the rate comes out to four figures (see [`room`]).
pub fn refine_room(rows: &[Row], at: usize, rates: &Rates) -> i64 {
    let Some(unit) = rows.get(at).and_then(|r| resolve(r, rates).ok()) else { return 1 };
    let Some(base) = reference(rows, &unit.iso, rates) else { return 1 };
    let worths: Vec<i64> = rows.iter().filter(|r| !r.name.trim().is_empty()).map(|r| r.rate).collect();
    room(&worths, &[rows[base].rate as f64], 10_000.0, false)
}

#[cfg(test)]
pub fn has_button(rows: &[Row], at: usize) -> bool {
    place(rows, at) == Place::Button
}

/// `x` to four significant figures, as a whole number - and never below 1.
pub fn round4(x: f64) -> i64 {
    if !x.is_finite() || x <= 0.0 {
        return 0;
    }
    let digits = x.log10().floor() as i32 + 1;
    let scale = 10f64.powi((digits - 4).max(0));
    (((x / scale).round() * scale) as i64).max(1)
}

/// The most a worth may be: far below `i64::MAX`, so that `Tour::convert`, which multiplies
/// two of them in `i128`, and any rescaling here stay well clear of it.
pub const MAX_WORTH: i64 = 1_000_000_000_000_000;

/// A worth read as a rate: how many of the reference one unit of it is - "117,5" for the euro
/// against the dinar. Four significant figures, a whole number from 1 000 up, `decimal` as the
/// reader writes it; no thousands gaps, as it goes into a box to be typed over.
pub fn rate_text(worth: i64, against: i64, decimal: char) -> String {
    if against <= 0 || worth <= 0 {
        return String::new();
    }
    let x = worth as f64 / against as f64;
    if x >= 1000.0 {
        return format!("{}", x.round() as i64);
    }
    let digits = x.log10().floor() as i32 + 1;
    let decimals = (4 - digits).max(0) as usize;
    let mut s = format!("{x:.decimals$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s.replace('.', &decimal.to_string())
}

/// A rate as somebody typed it: "117,5", "117.5", "0,0085", "19 000". `None` for anything
/// that is not a positive number.
pub fn parse_rate(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{202f}' && *c != '\u{a0}')
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.matches('.').count() > 1 {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|x| x.is_finite() && *x > 0.0)
}

/// The power of ten to multiply every worth by, so that each of `wanted` - worths about to be
/// written, unrounded - is at least `floor`, and (when `exact`) a whole number: a rate typed
/// as "1,2226" against a dinar at 100 is 122,26, which only ×100 keeps. 1 when nothing needs
/// it; never so much that `worths` would pass [`MAX_WORTH`].
///
/// The ratios stay what they were, and nobody sees the worths, so this is done without asking.
pub fn room(worths: &[i64], wanted: &[f64], floor: f64, exact: bool) -> i64 {
    let largest = worths
        .iter()
        .map(|w| *w as f64)
        .chain(wanted.iter().copied())
        .fold(0.0_f64, f64::max);
    let short = |f: f64| {
        wanted.iter().any(|w| {
            let x = w * f;
            x < floor || (exact && (x - x.round()).abs() > x * 1e-9)
        })
    };
    let mut f: i64 = 1;
    while short(f as f64) && largest * (f as f64) * 10.0 <= MAX_WORTH as f64 && f < 1_000_000_000_000 {
        f *= 10;
    }
    f
}

/// What one of `unit` is worth when one of `base` is worth `base_worth`, unrounded.
fn worth_against(unit: &Unit, base: &Unit, base_worth: i64, rates: &Rates) -> f64 {
    // One unit in dollars, over one unit of the base in dollars.
    let usd = |u: &Unit| 1.0 / (rates.per_usd[&u.iso] * u.per as f64);
    base_worth as f64 * usd(unit) / usd(base)
}

/// The worth "today's rates for all" gives the base: room for four figures and more.
const NORMAL_BASE: f64 = 10_000.0;

/// "Today's rates for all": every worth set afresh.
#[derive(Clone, Debug, PartialEq)]
pub struct Normalized {
    /// `(row, new worth)` for every row that changes.
    pub worths: Vec<(usize, i64)>,
    /// The row that is now the base: the cheapest currency by today's market, not by the
    /// worths it had - a lira that strengthened past the euro stops being the base.
    pub base: usize,
    /// Rows the source does not know (chips), kept in proportion to `kept_against` - a
    /// currency of the tour whose old worth meant something. `None` when there was none.
    pub unknown: Vec<usize>,
    pub kept_against: Option<usize>,
}

/// Every known currency at today's rate, the cheapest of them at 10 000, and every unknown one
/// moved in proportion, so that chips are worth what they were against the money.
///
/// What the worths were before does not matter for the known ones: added at the default 10 000
/// or years old, they all come out of the one answer. It is what makes adding the currencies
/// and pressing one button enough.
pub fn normalize(rows: &[Row], rates: &Rates) -> Result<Normalized, Why> {
    let live = |r: &Row| !r.name.trim().is_empty();
    let known: Vec<(usize, Unit)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| live(r))
        .filter_map(|(i, r)| resolve(r, rates).ok().map(|u| (i, u)))
        .collect();
    if known.is_empty() {
        return Err(Why::NothingToCompare);
    }
    // One unit of each in dollars; the cheapest is the base.
    let usd = |u: &Unit| 1.0 / (rates.per_usd[&u.iso] * u.per as f64);
    let (base, base_unit) = known
        .iter()
        .min_by(|(_, a), (_, b)| usd(a).total_cmp(&usd(b)))
        .cloned()
        .expect("not empty");
    let ratio = |u: &Unit| usd(u) / usd(&base_unit);

    // The unknown ones keep their proportion to a currency whose old worth meant something:
    // the cheapest of the tour's own known ones, as `reference` would choose.
    let kept_against = cheapest_of(
        known
            .iter()
            .map(|(i, _)| (*i, rows[*i].rate, rows[*i].saved))
            .filter(|(_, rate, saved)| *saved && *rate > 0),
    );
    let unknown: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(i, r)| live(r) && !known.iter().any(|(k, _)| k == i))
        .map(|(i, _)| i)
        .collect();

    // The base at 10 000 - more, when an unknown currency far cheaper than it would come out
    // below four figures (chips a hundred thousand to the dinar), rather than refusing.
    let mut base_worth = NORMAL_BASE;
    if let Some(k) = kept_against {
        let (_, ku) = known.iter().find(|(i, _)| *i == k).expect("known");
        let per_base = ratio(ku) / rows[k].rate as f64;
        let smallest = unknown
            .iter()
            .map(|i| rows[*i].rate as f64 * per_base)
            .filter(|w| *w > 0.0)
            .fold(f64::INFINITY, f64::min);
        while base_worth * smallest < 1000.0 && base_worth < 1e12 {
            base_worth *= 10.0;
        }
    }

    let mut worths: Vec<(usize, i64)> = Vec::new();
    let mut put = |i: usize, w: f64| -> Result<(), Why> {
        let w = if i == base { w.round() as i64 } else { round4(w) };
        if !(1..=MAX_WORTH).contains(&w) {
            return Err(Why::OutOfRange);
        }
        if w != rows[i].rate {
            worths.push((i, w));
        }
        Ok(())
    };
    for (i, u) in &known {
        put(*i, base_worth * ratio(u))?;
    }
    if let Some(k) = kept_against {
        let (_, ku) = known.iter().find(|(i, _)| *i == k).expect("known");
        let scale = base_worth * ratio(ku) / rows[k].rate as f64;
        for i in &unknown {
            put(*i, rows[*i].rate as f64 * scale)?;
        }
    }
    worths.sort_by_key(|(i, _)| *i);
    Ok(Normalized { worths, base, unknown, kept_against })
}

/// Today's worth of the currency in row `at` - and of its other units in the tour (EUR and
/// EURc both) - worked out against the cheapest other currency: `(row, new worth)`.
pub fn refine(rows: &[Row], at: usize, rates: &Rates) -> Result<Vec<(usize, i64)>, Why> {
    let row = rows.get(at).ok_or(Why::NothingToCompare)?;
    let unit = resolve(row, rates)?;
    let base_at = reference(rows, &unit.iso, rates).ok_or(Why::NothingToCompare)?;
    let base_row = &rows[base_at];
    let base = resolve(base_row, rates)?;
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.name.trim().is_empty() || i == base_at {
            continue;
        }
        let Ok(u) = resolve(r, rates) else { continue };
        if u.iso != unit.iso {
            continue;
        }
        let worth = round4(worth_against(&u, &base, base_row.rate, rates));
        if !(1..=MAX_WORTH).contains(&worth) {
            return Err(Why::OutOfRange);
        }
        out.push((i, worth));
    }
    Ok(out)
}

/// The rates: kept from earlier while the source says they are current, else asked for. An
/// answer kept from earlier is still better than none when the source cannot be reached.
#[cfg(target_arch = "wasm32")]
pub async fn fetch() -> Result<Rates, Why> {
    let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
    let kept = storage
        .as_ref()
        .and_then(|s| s.get_item(CACHE).ok().flatten())
        .and_then(|text| parse(&text));
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if let Some(r) = kept.as_ref().filter(|r| now < r.next) {
        return Ok(r.clone());
    }
    let asked = async {
        let resp = gloo_net::http::Request::get(URL)
            .abort_signal(deadline().as_ref())
            .send()
            .await
            .ok()?;
        if !resp.ok() {
            return None;
        }
        let text = resp.text().await.ok()?;
        parse(&text).map(|r| (r, text))
    }
    .await;
    match asked {
        Some((rates, text)) => {
            if let Some(s) = storage {
                let _ = s.set_item(CACHE, &text);
            }
            Ok(rates)
        }
        None => kept.ok_or(Why::Offline),
    }
}

/// Ten seconds, then give up: on a phone on a bad network the request could otherwise hang for
/// minutes with "asking…" on every button, and the dialog would be closed before it answered.
/// `AbortSignal.timeout` is not in every browser this runs in; without it, no deadline.
#[cfg(target_arch = "wasm32")]
fn deadline() -> Option<web_sys::AbortSignal> {
    let ctor = js_sys::Reflect::get(&js_sys::global(), &"AbortSignal".into()).ok()?;
    js_sys::Reflect::has(&ctor, &"timeout".into())
        .unwrap_or(false)
        .then(|| web_sys::AbortSignal::timeout_with_u32(10_000))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch() -> Result<Rates, Why> {
    Err(Why::Offline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Rates {
        parse(include_str!("../../../fixtures/er-api.latest.usd.json")).expect("the sample parses")
    }

    fn row(id: &str, name: &str, rate: i64) -> Row {
        Row { id: id.into(), name: name.into(), rate, saved: true }
    }

    fn new_row(name: &str, rate: i64) -> Row {
        Row { saved: false, ..row("", name, rate) }
    }


    fn u(iso: &str, per: i64) -> Unit {
        Unit { iso: iso.into(), per }
    }

    #[test]
    fn the_sample_answer_is_read() {
        let r = sample();
        assert_eq!(r.per_usd["USD"], 1.0);
        for code in ["EUR", "RSD", "BAM", "BGN", "RUB"] {
            assert!(r.per_usd.contains_key(code), "{code}");
        }
        assert!(r.next > r.updated && r.updated > 0);
        assert_eq!(parse(r#"{"result":"error","error-type":"unsupported-code"}"#), None);
        assert_eq!(parse("not json"), None);
    }

    /// Every id and name the tours have used (see BACKLOG.md), and the synonyms.
    #[test]
    fn the_currencies_of_the_tours_are_recognised() {
        assert_eq!(units_of("coin", "coin"), vec![]);
        assert_eq!(units_of("EUR", "EUR"), vec![u("EUR", 1)]);
        assert_eq!(units_of("Eur", "Eur"), vec![u("EUR", 1)]);
        assert_eq!(units_of("RSD", "Din"), vec![u("RSD", 1)]);
        assert_eq!(units_of("Din", "RSD"), vec![u("RSD", 1)]);
        assert_eq!(units_of("EURc", "EURc"), vec![u("EUR", 100)]);
        assert_eq!(units_of("Eurc", "Eurc"), vec![u("EUR", 100)]);
        assert_eq!(units_of("BAMc", "BAMc"), vec![u("BAM", 100)]);
        assert_eq!(units_of("USD", "USD"), vec![u("USD", 1)]);
        assert_eq!(units_of("LEV", "LEV"), vec![u("BGN", 1)]);
        assert_eq!(units_of("LEVc", "LEVc"), vec![u("BGN", 100)]);
        assert_eq!(units_of("x7k2q3a", "KM"), vec![u("BAM", 1)]);
        assert_eq!(units_of("x7k2q3a", "руб"), vec![u("RUB", 1)]);
        assert_eq!(units_of("x7k2q3a", "Chips"), vec![]);
        // A three-letter id that is no currency, and a name that is: both, id first.
        // A three-letter id that is no currency the source knows, and a name that is: the name.
        assert_eq!(units_of("abc", "EUR"), vec![u("EUR", 1)]);
        assert_eq!(units_of("Fun", "Fun"), vec![]);
    }

    #[test]
    fn a_new_currency_named_by_its_code_gets_the_code_as_id() {
        assert_eq!(id_for_new("eur", &[]), Some("EUR".into()));
        assert_eq!(id_for_new("Din", &[]), Some("RSD".into()));
        assert_eq!(id_for_new("EUR", &["Eur"]), None);
        assert_eq!(id_for_new("EURc", &[]), None);
        assert_eq!(id_for_new("Chips", &[]), None);
    }

    #[test]
    fn four_significant_figures() {
        assert_eq!(round4(117_563.2), 117_600);
        assert_eq!(round4(60_114.0), 60_110);
        assert_eq!(round4(1_175.6), 1_176);
        assert_eq!(round4(117.56), 118);
        assert_eq!(round4(0.2), 1);
    }

    /// The tour of 2025-26: RSD 1 000, and the rest from it.
    #[test]
    fn worths_are_worked_out_against_the_cheapest() {
        let r = sample();
        let rows = vec![
            row("RSD", "RSD", 1000),
            row("Eur", "Eur", 11_800),
            row("BAMc", "BAMc", 6_000),
            row("", "", 100),
        ];
        let eur = r.per_usd["RSD"] / r.per_usd["EUR"] * 1000.0;
        assert_eq!(refine(&rows, 1, &r), Ok(vec![(1, round4(eur))]));
        let bam_cent = r.per_usd["RSD"] / r.per_usd["BAM"] * 10.0;
        assert_eq!(refine(&rows, 2, &r), Ok(vec![(2, round4(bam_cent))]));
        assert_eq!(reference(&rows, "EUR", &r), Some(0));
        // The cheapest is the base, whatever it is: BAMc at 600 is cheaper than the dinar.
        let mut cheap_bam = rows.clone();
        cheap_bam[2].rate = 600;
        assert_eq!(reference(&cheap_bam, "EUR", &r), Some(2));
    }

    #[test]
    fn a_currency_and_its_cents_are_refined_together() {
        let r = sample();
        let rows = vec![row("RSD", "RSD", 1000), row("EUR", "EUR", 117_000), row("EURc", "EURc", 1_170)];
        let got = refine(&rows, 2, &r).expect("rates");
        let eur = got.iter().find(|(i, _)| *i == 1).expect("EUR too").1 as f64;
        let cent = got.iter().find(|(i, _)| *i == 2).expect("EURc").1 as f64;
        assert!((eur / cent - 100.0).abs() < 0.2, "{eur} / {cent}");
        // EURc on its own, without EUR: worked out from the euro and divided.
        let alone = vec![row("RSD", "RSD", 1000), row("EURc", "EURc", 1_170)];
        assert_eq!(refine(&alone, 1, &r).expect("rates")[0].1 as f64, cent);
    }

    #[test]
    fn what_cannot_be_done_is_said() {
        let mut r = sample();
        let rows = vec![row("RSD", "RSD", 1000), row("c1", "Chips", 5), row("XXQ", "XXQ", 7), row("BAM", "BAM", 60_000)];
        assert_eq!(refine(&rows, 1, &r), Err(Why::Unknown("Chips".into())));
        assert_eq!(refine(&rows, 2, &r), Err(Why::Unknown("XXQ".into())), "no code the source knows");
        // One the kept list has, missing from today's answer.
        r.per_usd.remove("BAM");
        assert_eq!(refine(&rows, 3, &r), Err(Why::NotAtSource("BAM".into())));
        let alone = vec![row("EUR", "EUR", 100), row("EURc", "EURc", 1)];
        assert_eq!(refine(&alone, 0, &r), Err(Why::NothingToCompare));
        // Chips are cheaper than dinars, but unknown - the dinar is still the base.
        let chips_first = vec![row("c1", "Chips", 1), row("RSD", "RSD", 1000), row("EUR", "EUR", 1)];
        assert_eq!(reference(&chips_first, "EUR", &r), Some(1));
    }

    #[test]
    fn the_button_is_not_on_the_base_nor_alone() {
        let rows = vec![row("RSD", "RSD", 1000), row("Eur", "Eur", 11_800), row("c", "Chips", 5), row("", "", 100)];
        assert!(!has_button(&rows, 0), "the base");
        assert!(has_button(&rows, 1));
        assert!(has_button(&rows, 2), "says it does not know chips");
        assert!(!has_button(&rows, 3), "the blank row");
        assert!(!has_button(&[row("coin", "coin", 100)], 0));
        assert!(!has_button(&[row("EUR", "EUR", 100), row("EURc", "EURc", 1)], 0));
    }

    /// A currency just added sits at the default worth, which may well be the smallest in the
    /// tour - it is still not the base, and keeps its button.
    #[test]
    fn a_new_currency_is_never_the_base() {
        let r = sample();
        let rows = vec![
            row("EUR", "EUR", 1_000_000),
            row("RSD", "RSD", 850_000),
            new_row("PLN", 10_000),
        ];
        assert_eq!(place(&rows, 2), Place::Button, "the new PLN");
        assert_eq!(place(&rows, 1), Place::Base, "the dinar, saved");
        assert_eq!(reference(&rows, "PLN", &r), Some(1));
        // Nothing saved that the source knows: then a new one is the base after all.
        let all_new = vec![new_row("RSD", 10_000), new_row("EUR", 10_000)];
        assert_eq!(place(&all_new, 0), Place::Base);
        assert_eq!(place(&all_new, 1), Place::Button);
    }

    /// Every currency added at the default 10 000, one press: the dinar - the cheapest by the
    /// market - is the base at 10 000, the rest follow.
    #[test]
    fn all_at_the_default_and_one_press() {
        let r = sample();
        let rows = vec![
            new_row("EUR", 10_000),
            new_row("RSD", 10_000),
            new_row("BAM", 10_000),
            new_row("PLN", 10_000),
            new_row("", 10_000),
        ];
        let n = normalize(&rows, &r).expect("rates");
        assert_eq!(n.base, 1);
        let w = |i: usize| n.worths.iter().find(|(r, _)| *r == i).map(|(_, w)| *w).unwrap_or(rows[i].rate);
        assert_eq!(w(1), 10_000);
        assert_eq!(w(0), round4(10_000.0 * r.per_usd["RSD"] / r.per_usd["EUR"]));
        assert!(w(3) > 10_000);
        assert!(!n.worths.iter().any(|(i, _)| *i == 4), "the blank row is left alone");
    }

    /// The base is the cheapest by the market, whatever the worths said.
    #[test]
    fn a_currency_that_strengthened_stops_being_the_base() {
        let mut r = sample();
        // A lira worth more than a euro.
        r.per_usd.insert("TRY".into(), r.per_usd["EUR"] / 2.0);
        let rows = vec![row("TRY", "TRY", 1000), row("EUR", "EUR", 40_000)];
        let n = normalize(&rows, &r).expect("rates");
        assert_eq!(n.base, 1);
        assert_eq!(n.worths, vec![(0, 20_000), (1, 10_000)]);
    }

    /// Chips keep what they were worth against the dinar the tour already had.
    #[test]
    fn chips_keep_their_proportion() {
        let r = sample();
        let rows = vec![row("RSD", "RSD", 100), row("c", "Chips", 50), row("EUR", "EUR", 11_800)];
        let n = normalize(&rows, &r).expect("rates");
        assert_eq!(n.kept_against, Some(0));
        assert_eq!(n.unknown, vec![1]);
        assert!(n.worths.contains(&(0, 10_000)) && n.worths.contains(&(1, 5_000)), "{:?}", n.worths);
        assert_eq!(normalize(&[row("c", "Chips", 5)], &r), Err(Why::NothingToCompare));
        // Chips a hundred thousand to the dinar: the base rises instead of refusing.
        let tiny = vec![row("RSD", "RSD", 100_000), row("c", "Chips", 1), row("EUR", "EUR", 11_800_000)];
        let n = normalize(&tiny, &r).expect("rates");
        let chips = n.worths.iter().find(|(i, _)| *i == 1).expect("chips").1;
        assert!(chips >= 1000, "{chips}");
    }

    /// An old tour, the dinar at 100: ×100 first, so the rouble comes out as 1,223 and not 1,22.
    #[test]
    fn a_small_base_gets_room_before_the_rate() {
        let r = sample();
        let rows = vec![row("RUB", "RUB", 180), row("EUR", "EUR", 11_800), row("RSD", "RSD", 100)];
        assert_eq!(refine_room(&rows, 0, &r), 100);
        assert_eq!(display_base(&rows), Some(2));
        let chips = vec![row("a", "Chips", 5), row("b", "Tokens", 50)];
        assert_eq!(display_base(&chips), Some(0), "nothing recognised: the cheapest");
    }

    /// The list kept with the app is the source's own - refreshed with the sample, it says so.
    #[test]
    fn the_kept_list_is_the_sources() {
        let mut from_source: Vec<String> = sample().per_usd.keys().cloned().collect();
        from_source.sort();
        let kept: Vec<String> = known_codes().into_iter().map(String::from).collect();
        assert_eq!(kept, from_source);
        assert!(kept.iter().all(|c| c.len() == 3 && c.chars().all(|x| x.is_ascii_uppercase())));
        assert_eq!(suggestions().len(), kept.len());
    }

    #[test]
    fn a_code_says_its_country() {
        assert_eq!(countries_of("BYN"), vec!["BY"]);
        assert_eq!(countries_of("BDT"), vec!["BD"]);
        assert!(countries_of("EUR").contains(&"DE") && countries_of("EUR").contains(&"ME"));
        assert!(countries_of("USD").contains(&"EC"));
        assert!(countries_of("XOF").contains(&"SN"));
        assert!(countries_of("XDR").is_empty());
    }

    #[test]
    fn a_worth_reads_as_a_rate() {
        assert_eq!(rate_text(1_175_000, 10_000, ','), "117,5");
        assert_eq!(rate_text(12_226, 10_000, '.'), "1.223");
        assert_eq!(rate_text(11_800, 100, ','), "118");
        assert_eq!(rate_text(190_000_000, 10_000, ','), "19000");
        assert_eq!(rate_text(85, 10_000, ','), "0,0085");
        assert_eq!(rate_text(10_000, 10_000, ','), "1");
        assert_eq!(rate_text(5, 0, ','), "");
    }

    #[test]
    fn a_rate_is_read_as_typed() {
        assert_eq!(parse_rate("117,5"), Some(117.5));
        assert_eq!(parse_rate(" 117.5 "), Some(117.5));
        assert_eq!(parse_rate("19 000"), Some(19_000.0));
        assert_eq!(parse_rate("0,0085"), Some(0.0085));
        assert_eq!(parse_rate("1.234,5"), None);
        assert_eq!(parse_rate("0"), None);
        assert_eq!(parse_rate("-3"), None);
        assert_eq!(parse_rate("abc"), None);
    }

    /// Enough room is made without asking, and no more than is needed.
    #[test]
    fn room_is_made_quietly() {
        // 117,5 against a dinar at 100: 11 750, whole - nothing to do.
        assert_eq!(room(&[100, 11_800], &[11_750.0], 1.0, true), 1);
        // 1,2226 against the same dinar: 122,26 - ×100 keeps it whole.
        assert_eq!(room(&[100, 11_800], &[122.26], 1.0, true), 100);
        // A base under 10 000 before today's rate: up to 10 000.
        assert_eq!(room(&[100, 11_800], &[100.0], 10_000.0, false), 100);
        assert_eq!(room(&[10_000], &[10_000.0], 10_000.0, false), 1);
        // Never past the ceiling, whatever is asked.
        assert!(room(&[MAX_WORTH / 5], &[0.5], 10_000.0, false) < 10);
    }
}
