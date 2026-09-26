//! A currency "with cents": amounts kept in hundredths, read and written with a decimal part.
//!
//! The stored field is called `AmountInCents`, but what it has always held is whole units of
//! the spending's currency - 171 000 is a hundred and seventy-one thousand roubles. A tour in
//! euros had no way to say 3,50, and the answer people found on the road was a second
//! currency, "EURc", worth a hundredth of the first. That is what this module replaces: one
//! currency, marked `WithCents`, whose amounts are hundredths, and which the interface reads
//! and writes as "3,50".
//!
//! Amounts stay integers. A currency's worth is always the worth of a **whole** unit - a euro
//! is 117 000 beside a dinar at 1 000, cents or not, which is what the currencies dialog shows
//! and what an exchange rate gives - and `Tour::convert` divides by a hundred for a currency
//! whose amounts are hundredths. So switching cents on or off changes the expenses (×100,
//! ÷100) and never a worth.
//!
//! An earlier version kept the worth of a *cent* instead, so the euro read 1 170 in the
//! dialog, and switching had to rescale the other currencies to stay exact - which is how a
//! tour's dinars went from 1 000 to 100 000 in one click.

use crate::domain::{extras, Currency, Tour};
use crate::ids::CurrencyId;
use crate::money::{convert, Cents};

impl Currency {
    /// Whether this currency's amounts are hundredths, shown with a decimal part.
    pub fn with_cents(&self) -> bool {
        extras::bool_of(&self.extras, extras::WITH_CENTS)
    }

    /// How many stored units make a whole one: 100 for a currency with cents, 1 otherwise.
    pub fn stored_per_whole(&self) -> i64 {
        if self.with_cents() {
            100
        } else {
            1
        }
    }

    pub fn set_with_cents(&mut self, on: bool) {
        if on {
            extras::set(&mut self.extras, extras::WITH_CENTS, true.into());
        } else {
            // Taken out rather than stored as `false`: a currency that never had cents looks
            // exactly as it always did, to this client and to the C# one.
            let key = self
                .extras
                .0
                .keys()
                .find(|k| k.eq_ignore_ascii_case(extras::WITH_CENTS))
                .cloned();
            if let Some(key) = key {
                self.extras.0.remove(&key);
            }
        }
    }
}

impl Tour {
    /// Whether the figures of this tour - which are in the currency it is being read in -
    /// are hundredths.
    pub fn shows_cents(&self) -> bool {
        self.currency().with_cents()
    }

    /// Whether amounts entered in `currency` are hundredths. A currency the tour does not
    /// list is read in the tour's own currency, as `convert` reads it.
    pub fn counts_cents(&self, currency: &CurrencyId) -> bool {
        self.currencies
            .iter()
            .find(|c| &c.id == currency)
            .unwrap_or_else(|| self.currency())
            .with_cents()
    }
}

/// An amount as the reader sees it: groups of three split by a thin space, and - for a
/// currency with cents - two decimals after `decimal` (`,` in Russian, `.` in English).
pub fn format(amount: Cents, cents: bool, decimal: char) -> String {
    if !cents {
        return amount.to_string();
    }
    let n = amount.0;
    let whole = Cents((n.unsigned_abs() / 100) as i64);
    let frac = n.unsigned_abs() % 100;
    let sign = if n < 0 { "-" } else { "" };
    format!("{sign}{whole}{decimal}{frac:02}")
}

/// What is wrong with a typed amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    Empty,
    NotANumber,
    /// "1,234" in a currency with cents: three digits after the separator. Refused rather
    /// than read as a thousands separator - the comma is the decimal separator here.
    TooManyDecimals,
    /// "3,50" in a currency without cents.
    NoCentsHere,
}

/// Reads what somebody typed into an amount box.
///
/// Both `.` and `,` are the decimal separator - which one a phone's keyboard offers depends
/// on the phone's language, not on ours. Spaces of any kind inside the number are ignored, so
/// "1 234,56" pasted from the app itself reads back. A minus sign in front is allowed:
/// negative expenses are how a refund is recorded.
pub fn parse_amount(text: &str, cents: bool) -> Result<Cents, AmountError> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{202f}' && *c != '\u{a0}')
        .collect();
    if cleaned.is_empty() {
        return Err(AmountError::Empty);
    }
    let (negative, digits) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, cleaned.as_str()),
    };
    let mut parts = digits.splitn(2, ['.', ',']);
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    if frac.is_some_and(|f| f.contains(['.', ','])) {
        return Err(AmountError::NotANumber);
    }
    let all_digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if !all_digits(whole) || !frac.is_none_or(all_digits) || (whole.is_empty() && frac.is_none_or(str::is_empty)) {
        return Err(AmountError::NotANumber);
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| AmountError::NotANumber)?
    };
    let value = match (frac, cents) {
        (None, false) => whole,
        (Some(""), false) => whole,
        (Some(_), false) => return Err(AmountError::NoCentsHere),
        (None, true) | (Some(""), true) => whole.checked_mul(100).ok_or(AmountError::NotANumber)?,
        (Some(f), true) if f.len() > 2 => return Err(AmountError::TooManyDecimals),
        (Some(f), true) => {
            let hundredths: i64 = if f.len() == 1 { f.parse::<i64>().unwrap_or(0) * 10 } else { f.parse().unwrap_or(0) };
            whole
                .checked_mul(100)
                .and_then(|w| w.checked_add(hundredths))
                .ok_or(AmountError::NotANumber)?
        }
    };
    Ok(Cents(if negative { -value } else { value }))
}

/// Moves every expense in currency `from` into currency `to`, converting each amount by the
/// worths the tour lists. Returns how many moved.
///
/// Used where a currency goes away and what was entered in it must not: removing a currency
/// (its expenses go into the cheapest one left), and folding an "EURc" into a euro that now
/// has cents of its own. Before this, an expense whose currency had been removed was quietly
/// read in the tour's main currency - and every figure in the tour changed without a word.
pub fn move_spendings(tour: &mut Tour, from: &CurrencyId, to: &CurrencyId) -> usize {
    let (Some(source), Some(target)) = (
        tour.currencies.iter().find(|c| &c.id == from).cloned(),
        tour.currencies.iter().find(|c| &c.id == to).cloned(),
    ) else {
        return 0;
    };
    // Worths are per whole unit; amounts are in stored units - hundredths where there are
    // cents - on both sides.
    let num = source.rate as i128 * target.stored_per_whole() as i128;
    let den = target.rate as i128 * source.stored_per_whole() as i128;
    let mut moved = 0;
    for s in tour.spendings.iter_mut().filter(|s| &s.currency.id == from) {
        s.amount = crate::money::convert_ratio(s.amount, num, den);
        s.currency = target.clone();
        moved += 1;
    }
    moved
}

/// The currency the others are worth something against: the one whose unit is worth least.
/// `except` is left out - the one being removed, say.
pub fn cheapest<'a>(tour: &'a Tour, except: Option<&CurrencyId>) -> Option<&'a Currency> {
    tour.currencies
        .iter()
        .filter(|c| Some(&c.id) != except)
        .min_by_key(|c| c.rate)
}

/// What switching a currency's cents did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Switched {
    /// Expenses whose amount changed form (×100 or ÷100).
    pub changed: usize,
    /// Of those, switching off: the ones that had cents, now rounded to whole units.
    pub rounded: usize,
}

/// Switches `id` to amounts in hundredths (`on`) or back to whole units, keeping every figure
/// of the tour what it was.
///
/// Only the expenses change: ×100 on, ÷100 off - the second rounded half away from zero, the
/// one step that can lose money, and counted so the reader can be told. No worth changes: a
/// worth is always that of a whole unit.
pub fn switch_cents(tour: &mut Tour, id: &CurrencyId, on: bool) -> Switched {
    let Some(at) = tour.currencies.iter().position(|c| &c.id == id) else {
        return Switched::default();
    };
    if tour.currencies[at].with_cents() == on {
        return Switched::default();
    }
    tour.currencies[at].set_with_cents(on);
    let current = tour.currencies[at].clone();

    let mut done = Switched::default();
    for s in tour.spendings.iter_mut().filter(|s| &s.currency.id == id) {
        if on {
            s.amount = Cents(s.amount.0 * 100);
        } else {
            if s.amount.0 % 100 != 0 {
                done.rounded += 1;
            }
            s.amount = convert(s.amount, 1, 100);
        }
        s.currency = current.clone();
        done.changed += 1;
    }
    done
}

/// The "EURc" of a currency: another currency of the tour named or identified like this one
/// with a `c` after it - how a hundredth was spelled before currencies could have cents.
pub fn cents_sibling<'a>(tour: &'a Tour, of: &CurrencyId) -> Option<&'a Currency> {
    let this = tour.currencies.iter().find(|c| &c.id == of)?;
    let names = [this.id.as_str(), this.name.trim()];
    tour.currencies.iter().find(|c| {
        c.id != this.id
            && [c.id.as_str(), c.name.trim()].iter().any(|n| {
                names.iter().any(|base| {
                    !base.is_empty()
                        && n.len() == base.len() + 1
                        && n.to_lowercase().starts_with(&base.to_lowercase())
                        && n.to_lowercase().ends_with('c')
                })
            })
    })
}

/// Whether a new currency should have cents unless somebody says otherwise: when its unit is
/// worth at least thirty of the cheapest currency's - a euro against dinars (≈ 117), a mark
/// against dinars (≈ 60). A tour's only or cheapest currency has nothing to compare with, and
/// goes by its name instead: the ones people count in cents.
pub fn cents_by_default(rate: i64, name: &str, others: &[i64]) -> bool {
    match others.iter().copied().filter(|r| *r > 0).min() {
        Some(cheapest) if cheapest < rate => rate >= 30 * cheapest,
        _ => {
            let n = name.trim().to_uppercase();
            ["EUR", "USD", "BAM", "BGN", "GBP", "CHF", "ЕВРО", "EURO", "KM", "LEV"]
                .iter()
                .any(|c| n == *c)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour(currencies: &[(&str, i64, bool)], spendings: &[(&str, i64)]) -> Tour {
        let mut json = serde_json::json!({
            "Id": "t", "Name": "t", "Persons": [{"GUID": "p", "Name": "P", "Weight": 100}],
            "Currencies": [], "Currency": {"Name": currencies[0].0, "CurrencyRate": currencies[0].1},
            "Spendings": []
        });
        for (name, rate, cents) in currencies {
            let mut c = serde_json::json!({"_id": name, "Name": name, "CurrencyRate": rate});
            if *cents {
                c["WithCents"] = true.into();
            }
            json["Currencies"].as_array_mut().unwrap().push(c);
        }
        for (i, (cur, amount)) in spendings.iter().enumerate() {
            let rate = currencies.iter().find(|c| c.0 == *cur).map(|c| c.1).unwrap_or(100);
            json["Spendings"].as_array_mut().unwrap().push(serde_json::json!({
                "GUID": format!("s{i}"), "Description": "x", "Type": "Food",
                "AmountInCents": amount, "FromGuid": "p", "ToAll": true, "ToGuid": [],
                "Currency": {"_id": cur, "Name": cur, "CurrencyRate": rate}
            }));
        }
        Tour::from_json(&json.to_string()).expect("tour")
    }

    #[test]
    fn a_currency_with_cents_shows_two_decimals_in_the_readers_separator() {
        assert_eq!(format(Cents(350), true, ','), "3,50");
        assert_eq!(format(Cents(123456), true, '.'), "1\u{202f}234.56");
        assert_eq!(format(Cents(-5), true, ','), "-0,05");
        assert_eq!(format(Cents(123456), false, ','), "123\u{202f}456");
    }

    #[test]
    fn a_point_and_a_comma_both_mean_the_decimal_part() {
        for typed in ["3,50", "3.50", "3,5", "3.5", " 3,50 "] {
            assert_eq!(parse_amount(typed, true), Ok(Cents(350)), "{typed}");
        }
        assert_eq!(parse_amount("1 234,56", true), Ok(Cents(123456)));
        assert_eq!(parse_amount("1\u{202f}234.56", true), Ok(Cents(123456)));
        assert_eq!(parse_amount("12", true), Ok(Cents(1200)));
        assert_eq!(parse_amount("12,", true), Ok(Cents(1200)));
        assert_eq!(parse_amount(",5", true), Ok(Cents(50)));
        assert_eq!(parse_amount("-3,50", true), Ok(Cents(-350)));
        assert_eq!(parse_amount("1500", false), Ok(Cents(1500)));
    }

    #[test]
    fn what_cannot_be_an_amount_is_refused_by_name() {
        assert_eq!(parse_amount("", true), Err(AmountError::Empty));
        assert_eq!(parse_amount("   ", false), Err(AmountError::Empty));
        assert_eq!(parse_amount("1,234", true), Err(AmountError::TooManyDecimals));
        assert_eq!(parse_amount("3,50", false), Err(AmountError::NoCentsHere));
        assert_eq!(parse_amount("abc", true), Err(AmountError::NotANumber));
        assert_eq!(parse_amount("1,2,3", true), Err(AmountError::NotANumber));
        assert_eq!(parse_amount(",", true), Err(AmountError::NotANumber));
    }

    #[test]
    fn switching_cents_on_keeps_every_figure_and_every_worth() {
        // The tours of 2025-2026: dinars at 1 000, euros at 117 000.
        let mut t = tour(&[("RSD", 1000, false), ("BAMc", 600, false), ("EUR", 117000, false)], &[("EUR", 12), ("RSD", 500)]);
        let before: Vec<Cents> = t.spendings.iter().map(|s| t.convert(s.amount, &s.currency)).collect();
        let done = switch_cents(&mut t, &CurrencyId::new("EUR"), true);
        assert_eq!(done, Switched { changed: 1, rounded: 0 });
        assert_eq!(t.spendings[0].amount, Cents(1200));
        // A worth is the worth of a whole euro, cents or not: nothing moves.
        assert_eq!(t.currencies.iter().map(|c| c.rate).collect::<Vec<_>>(), vec![1000, 600, 117000]);
        assert!(t.currencies[2].with_cents());
        let after: Vec<Cents> = t.spendings.iter().map(|s| t.convert(s.amount, &s.currency)).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn a_tour_read_in_a_currency_with_cents_is_read_in_cents() {
        let mut t = tour(&[("EUR", 117000, true), ("RSD", 1000, false)], &[("RSD", 1174)]);
        t.current_currency = CurrencyId::new("EUR");
        // 1 174 dinars at 1 000 against 117 000 are 10,03 euros - 1 003 cents.
        assert_eq!(t.convert(t.spendings[0].amount, &t.spendings[0].currency), Cents(1003));
        // And the other way: 3,50 € in dinars.
        t.current_currency = CurrencyId::new("RSD");
        let eur = t.currencies[0].clone();
        assert_eq!(t.convert(Cents(350), &eur), Cents(410));
    }

    #[test]
    fn switching_cents_off_rounds_and_says_how_many() {
        let mut t = tour(&[("RSD", 1000, false), ("EUR", 117000, true)], &[("EUR", 350), ("EUR", 1200)]);
        let done = switch_cents(&mut t, &CurrencyId::new("EUR"), false);
        assert_eq!(done, Switched { changed: 2, rounded: 1 });
        assert_eq!(t.spendings.iter().map(|s| s.amount).collect::<Vec<_>>(), vec![Cents(4), Cents(12)]);
        assert_eq!(t.currencies.iter().map(|c| c.rate).collect::<Vec<_>>(), vec![1000, 117000]);
        assert!(!t.currencies[1].with_cents());
    }

    #[test]
    fn the_threshold_counts_the_smallest_coin() {
        // Dinars at 1 000, euro cents at 1 170 a cent: the dinar is the smallest coin, and
        // read in euros 49 dinars are 41 cents.
        let mut t = tour(&[("RSD", 1000, false), ("EUR", 117000, true)], &[]);
        assert_eq!(t.min_meaningful(49), Cents(49));
        t.current_currency = CurrencyId::new("EUR");
        assert_eq!(t.min_meaningful(49), Cents(41));
    }

    #[test]
    fn a_removed_currencys_expenses_go_into_the_cheapest_at_its_worth() {
        let mut t = tour(&[("RSD", 100, false), ("EUR", 11745, false)], &[("EUR", 12)]);
        let cheapest = cheapest(&t, Some(&CurrencyId::new("EUR"))).unwrap().id.clone();
        assert_eq!(cheapest, CurrencyId::new("RSD"));
        assert_eq!(move_spendings(&mut t, &CurrencyId::new("EUR"), &cheapest), 1);
        assert_eq!(t.spendings[0].amount, Cents(1409));
        assert_eq!(t.spendings[0].currency.id, CurrencyId::new("RSD"));
    }

    #[test]
    fn eurc_is_found_beside_eur_by_id_or_name() {
        let t = tour(&[("RSD", 1000, false), ("EUR", 117000, false), ("EURc", 1170, false)], &[]);
        assert_eq!(cents_sibling(&t, &CurrencyId::new("EUR")).map(|c| c.name.as_str()), Some("EURc"));
        assert!(cents_sibling(&t, &CurrencyId::new("RSD")).is_none());
        let t = tour(&[("Eur", 117000, false), ("Eurc", 1170, false)], &[]);
        assert_eq!(cents_sibling(&t, &CurrencyId::new("Eur")).map(|c| c.name.as_str()), Some("Eurc"));
    }

    #[test]
    fn cents_by_default_from_thirty_times_the_cheapest() {
        assert!(cents_by_default(11745, "EUR", &[100]));
        assert!(cents_by_default(6000, "BAM", &[100]));
        assert!(!cents_by_default(2900, "X", &[100]));
        assert!(!cents_by_default(100, "RSD", &[11745]));
        assert!(cents_by_default(100, "EUR", &[]));
        assert!(!cents_by_default(100, "RSD", &[]));
    }
}
