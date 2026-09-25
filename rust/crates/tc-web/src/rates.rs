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
//! are worked out from it. Rounded to four significant figures - 117 500, not 117 563 - which
//! is closer than anybody settling a trip cares about, and reads better. When the cheapest is
//! under 1 000 that is not four figures, so the dialog offers to multiply every worth by a
//! power of ten first (`raise_factor`): the ratios stay, the numbers get room.

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
        (s.chars().count() == 3 && s.chars().all(|c| c.is_ascii_uppercase())).then(|| s.to_owned())
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
    pub rate: i32,
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
    rows.iter()
        .enumerate()
        .filter(|(_, r)| r.rate > 0 && !r.name.trim().is_empty())
        .filter_map(|(i, r)| resolve(r, rates).ok().map(|u| (i, r.rate, u)))
        .filter(|(_, _, u)| u.iso != group)
        .min_by_key(|(_, rate, _)| *rate)
        .map(|(i, _, _)| i)
}

/// Whether a row gets the button: not the one the others are worked out against, and not in
/// a tour where there is nothing else to work it out against. Decided without the rates -
/// the button is there before anybody asks the source.
pub fn has_button(rows: &[Row], at: usize) -> bool {
    let Some(row) = rows.get(at) else { return false };
    if row.name.trim().is_empty() {
        return false;
    }
    let known: Vec<(usize, i32, String)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.rate > 0 && !r.name.trim().is_empty())
        .filter_map(|(i, r)| units_of(&r.id, &r.name).first().map(|u| (i, r.rate, u.iso.clone())))
        .collect();
    let mine = units_of(&row.id, &row.name).first().map(|u| u.iso.clone());
    // Alone, or only beside its own cents: nothing to work it out against. An unrecognised
    // one ("Chips") beside others keeps the button - pressed, it says it is not recognised.
    if !known.iter().any(|(_, _, iso)| Some(iso) != mine.as_ref()) {
        return false;
    }
    // The cheapest known currency is the one the others are worked out against.
    let base = known.iter().min_by_key(|(_, rate, _)| *rate).map(|(i, _, _)| *i);
    base != Some(at)
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

/// Multiplying every worth by this makes the smallest at least 1 000 - four significant
/// figures for everything worked out from it. `None` when there is no need, or the largest
/// would not fit.
pub fn raise_factor(rates: &[i32]) -> Option<i32> {
    let min = rates.iter().copied().filter(|r| *r > 0).min()?;
    let max = rates.iter().copied().max()?;
    let mut f: i64 = 1;
    while (min as i64) * f < 1000 {
        f *= 10;
    }
    (f > 1 && (max as i64) * f <= i32::MAX as i64).then_some(f as i32)
}

/// The multiplier to offer before working out row `at`: when the currency it would be worked
/// out against is worth under 1 000 (see [`raise_factor`]).
pub fn raise_for(rows: &[Row], at: usize, rates: &Rates) -> Option<i32> {
    let unit = resolve(rows.get(at)?, rates).ok()?;
    let base = rows[reference(rows, &unit.iso, rates)?].rate;
    let max = rows.iter().filter(|r| !r.name.trim().is_empty()).map(|r| r.rate).max()?;
    raise_factor(&[base, max])
}

/// Today's worth of the currency in row `at` - and of its other units in the tour (EUR and
/// EURc both) - worked out against the cheapest other currency: `(row, new worth)`.
pub fn refine(rows: &[Row], at: usize, rates: &Rates) -> Result<Vec<(usize, i32)>, Why> {
    let row = rows.get(at).ok_or(Why::NothingToCompare)?;
    let unit = resolve(row, rates)?;
    let base_at = reference(rows, &unit.iso, rates).ok_or(Why::NothingToCompare)?;
    let base_row = &rows[base_at];
    let base = resolve(base_row, rates)?;
    // One unit of the row, in dollars, over one unit of the base, in dollars.
    let usd = |u: &Unit| 1.0 / (rates.per_usd[&u.iso] * u.per as f64);
    let mut out = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        if r.name.trim().is_empty() || i == base_at {
            continue;
        }
        let Ok(u) = resolve(r, rates) else { continue };
        if u.iso != unit.iso {
            continue;
        }
        let worth = round4(base_row.rate as f64 * usd(&u) / usd(&base));
        if worth < 1 || worth > i32::MAX as i64 {
            return Err(Why::OutOfRange);
        }
        out.push((i, worth as i32));
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
        let resp = gloo_net::http::Request::get(URL).send().await.ok()?;
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

    fn row(id: &str, name: &str, rate: i32) -> Row {
        Row { id: id.into(), name: name.into(), rate }
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
        assert_eq!(units_of("abc", "EUR"), vec![u("ABC", 1), u("EUR", 1)]);
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
        assert_eq!(refine(&rows, 1, &r), Ok(vec![(1, round4(eur) as i32)]));
        let bam_cent = r.per_usd["RSD"] / r.per_usd["BAM"] * 10.0;
        assert_eq!(refine(&rows, 2, &r), Ok(vec![(2, round4(bam_cent) as i32)]));
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
        let r = sample();
        let rows = vec![row("RSD", "RSD", 1000), row("c1", "Chips", 5), row("XXQ", "XXQ", 7)];
        assert_eq!(refine(&rows, 1, &r), Err(Why::Unknown("Chips".into())));
        assert_eq!(refine(&rows, 2, &r), Err(Why::NotAtSource("XXQ".into())));
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

    #[test]
    fn a_small_base_is_offered_room() {
        assert_eq!(raise_factor(&[100, 11_800]), Some(10));
        assert_eq!(raise_factor(&[1, 120]), Some(1000));
        assert_eq!(raise_factor(&[1000, 117_000]), None);
        assert_eq!(raise_factor(&[5, i32::MAX / 10]), None, "would not fit");
        // Chips at 1 are cheaper, but the dinar is what the euro is worked out from.
        let r = sample();
        let rows = vec![row("c", "Chips", 1), row("RSD", "RSD", 100), row("EUR", "EUR", 11_800)];
        assert_eq!(raise_for(&rows, 2, &r), Some(10));
    }
}
