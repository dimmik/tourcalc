//! Money, and the one rule about it: never a floating point number.
//!
//! The C# app already stores amounts as whole cents in `long`, which is right. What it
//! does not do is stop you adding a person's weight to a person's debt - both are numbers.
//! `Cents` makes that a compile error while staying eight bytes with no wrapper at runtime.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Neg, Sub};

/// An amount in the smallest unit of some currency.
///
/// `Copy` is right here: eight bytes with nothing owned on the heap is cheaper to copy
/// than to borrow, and borrowing it would only add noise. That is the whole rule for
/// `Copy` - small, and owning nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);

    pub fn abs(self) -> Cents {
        Cents(self.0.abs())
    }
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl Add for Cents {
    type Output = Cents;
    fn add(self, rhs: Cents) -> Cents {
        Cents(self.0 + rhs.0)
    }
}

impl Sub for Cents {
    type Output = Cents;
    fn sub(self, rhs: Cents) -> Cents {
        Cents(self.0 - rhs.0)
    }
}

impl AddAssign for Cents {
    fn add_assign(&mut self, rhs: Cents) {
        self.0 += rhs.0;
    }
}

impl Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}

// Deliberately absent: Mul<Cents> for Cents. Money times money has no meaning, so the
// compiler should not let anyone write it. Scaling by a rate goes through `convert` below,
// where the rounding is spelled out.

impl std::iter::Sum for Cents {
    fn sum<I: Iterator<Item = Cents>>(iter: I) -> Cents {
        Cents(iter.map(|c| c.0).sum())
    }
}

impl std::fmt::Display for Cents {
    /// Thin-space groups, the way the app prints amounts: 1 234 567.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0;
        let digits = n.unsigned_abs().to_string();
        if n < 0 {
            f.write_str("-")?;
        }
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                f.write_str("\u{202f}")?;
            }
            f.write_str(&ch.to_string())?;
        }
        Ok(())
    }
}

/// Converts an amount between two currencies of the same tour.
///
/// The C# version does this in `double` and then rounds (`Spending.AmountInCurrentCurrency`).
/// Here the intermediate product is `i128`, so nothing is lost before the one rounding step
/// that we choose ourselves - half away from zero, matching `Math.Round`'s default.
pub fn convert(amount: Cents, from_rate: i32, to_rate: i32) -> Cents {
    if from_rate == to_rate || to_rate == 0 {
        return amount;
    }
    let num = amount.0 as i128 * from_rate as i128;
    let den = to_rate as i128;
    let half = den / 2;
    let rounded = if num >= 0 {
        (num + half) / den
    } else {
        (num - half) / den
    };
    Cents(rounded as i64)
}
