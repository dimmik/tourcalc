//! Who spent what, who owes whom.
//!
//! **This is the file to read if you want to see what ownership is for.**
//!
//! The C# calculator starts like this:
//!
//! ```csharp
//! public TourCalculator(Tour tour, Currency currency) {
//!     // clone
//!     CurrentTour = JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(tour));
//! ```
//!
//! The whole tour is serialised to a JSON string and parsed back, purely to obtain a copy
//! nobody else holds - because the calculator then mutates it, and mutating the caller's
//! tour would be a disaster. The copy is not an optimisation; it is a fence.
//!
//! Here the fence is the signature. `calculate` takes `&Tour` - a shared borrow, which
//! cannot be mutated through, and the compiler enforces that rather than trusting us. So
//! there is nothing to protect the caller from, and the copy disappears: not optimised
//! away, simply never needed.
//!
//! The same idea decides the return type. Instead of handing back a mutated tour, we
//! return a separate `Balances` that the caller owns outright. Input borrowed, output
//! owned - "who is allowed to change this" is answered by the types, once, at the border.

use crate::domain::{Person, Split, Tour};
use crate::ids::PersonId;
use crate::money::{convert, Cents};

/// Fixed-point scale used while dividing a spending between people.
///
/// Shares are divided in whole units scaled up by this, and rounded down at the very end,
/// so that rounding happens once per person instead of once per spending. The C# code uses
/// the same constant (`_magnitude_`); the arithmetic below is deliberately identical to it,
/// because the two implementations have to agree to the cent.
const MAGNITUDE: i128 = 10_000_000;

/// What one person ended up with.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonBalance {
    pub person: PersonId,
    /// What they paid out.
    pub spent: Cents,
    /// What was spent on them.
    pub received: Cents,
}

impl PersonBalance {
    /// Positive - owes money; negative - is owed money.
    pub fn debt(&self) -> Cents {
        self.received - self.spent
    }
}

/// The result of a calculation: owned by the caller, unrelated to the tour it came from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Balances {
    pub per_person: Vec<PersonBalance>,
}

impl Balances {
    pub fn get(&self, id: &PersonId) -> Option<&PersonBalance> {
        self.per_person.iter().find(|b| &b.person == id)
    }

    /// Everyone who is owed money, deepest credit first.
    ///
    /// Returns an iterator that borrows `self`. The `+ '_` is the compiler asking us to
    /// promise the iterator will not outlive the `Balances` it walks over - which is the
    /// entire content of lifetimes in a codebase like this one.
    pub fn creditors(&self) -> impl Iterator<Item = &PersonBalance> + '_ {
        let mut v: Vec<&PersonBalance> =
            self.per_person.iter().filter(|b| b.debt().0 < 0).collect();
        v.sort_by_key(|b| b.debt().0);
        v.into_iter()
    }

    /// Everyone who owes, largest debt first.
    pub fn debtors(&self) -> impl Iterator<Item = &PersonBalance> + '_ {
        let mut v: Vec<&PersonBalance> =
            self.per_person.iter().filter(|b| b.debt().0 > 0).collect();
        v.sort_by_key(|b| -b.debt().0);
        v.into_iter()
    }

    pub fn total_abs_debt(&self) -> Cents {
        self.per_person.iter().map(|b| b.debt().abs()).sum()
    }
}

/// Whether planned transfers take part in the sums.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub with_planned: bool,
}

/// Adds up a tour.
///
/// Borrows the tour and owns nothing of it; returns a fresh `Balances`.
pub fn calculate(tour: &Tour, opts: Options) -> Balances {
    if tour.persons.is_empty() {
        return Balances::default();
    }

    let total_weight = tour.total_weight();
    let base_rate = tour.currency().rate;

    // `counted` borrows from `tour` too - it is a list of references, not of copies. No
    // spending is duplicated anywhere in this function.
    let counted: Vec<&crate::domain::Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(opts.with_planned))
        .collect();

    let mut per_person: Vec<PersonBalance> = tour
        .persons
        .iter()
        .map(|p| PersonBalance {
            person: p.id.clone(),
            spent: spent_by(p, &counted, base_rate),
            received: received_by(p, tour, &counted, base_rate, total_weight),
        })
        .collect();

    settle_rounding(&mut per_person);

    Balances { per_person }
}

/// The sum of what this person paid for.
fn spent_by(person: &Person, counted: &[&crate::domain::Spending], base_rate: i32) -> Cents {
    counted
        .iter()
        .filter(|s| s.from == person.id)
        .map(|s| convert(s.amount, s.currency.rate, base_rate))
        .sum()
}

/// The sum of what was spent on this person.
///
/// Each spending's share is computed scaled up by `MAGNITUDE` and rounded once, which is
/// what keeps a tour's shares adding back up to the amounts that were actually paid.
fn received_by(
    person: &Person,
    tour: &Tour,
    counted: &[&crate::domain::Spending],
    base_rate: i32,
    total_weight: i64,
) -> Cents {
    let mut scaled_total: i128 = 0;

    for s in counted {
        let amount = convert(s.amount, s.currency.rate, base_rate).0 as i128;

        // Three arms, and the compiler will not let a fourth appear unnoticed: adding a
        // variant to `Split` breaks this match until it is handled.
        let share = match &s.split {
            Split::Everyone => amount * person.weight as i128 * MAGNITUDE / total_weight as i128,

            Split::ByWeight(to) if to.contains(&person.id) => {
                let group_weight: i64 = to
                    .iter()
                    .filter_map(|id| tour.person(id))
                    .map(|p| p.weight as i64)
                    .sum();
                let group_weight = if group_weight == 0 { 1 } else { group_weight };
                amount * person.weight as i128 * MAGNITUDE / group_weight as i128
            }

            Split::Equally(to) if to.contains(&person.id) => {
                amount * MAGNITUDE / to.len().max(1) as i128
            }

            // Not one of theirs.
            _ => continue,
        };

        scaled_total += share;
    }

    Cents(((scaled_total + MAGNITUDE / 2) / MAGNITUDE) as i64)
}

/// Nudges one person so that what is owed equals what is due.
///
/// Dividing in whole cents leaves a remainder: the sum of everybody's credit need not equal
/// the sum of everybody's debt, and a settlement built on that would never converge. The
/// C# code does the same thing in `DealWithRoundErrors`, and the choice of who absorbs the
/// remainder is reproduced exactly - first anyone the difference would bring to zero, then
/// the largest creditor or the largest debtor, whichever is larger.
fn settle_rounding(per_person: &mut [PersonBalance]) {
    let credit: i64 = per_person
        .iter()
        .map(|b| b.debt().0)
        .filter(|d| *d < 0)
        .sum::<i64>();
    let debit: i64 = per_person
        .iter()
        .map(|b| b.debt().0)
        .filter(|d| *d > 0)
        .sum::<i64>();
    let diff = -credit - debit;
    if diff == 0 {
        return;
    }

    // `iter().position(...)` finds an index rather than a reference: taking a shared borrow
    // and then mutating through it is precisely what the borrow checker forbids, so we look
    // first and mutate second.
    let target = per_person
        .iter()
        .position(|b| b.debt().0 + diff == 0)
        .or_else(|| {
            let max_credit = per_person.iter().map(|b| -b.debt().0).max().unwrap_or(0);
            let max_debit = per_person.iter().map(|b| b.debt().0).max().unwrap_or(0);
            if max_credit > max_debit {
                per_person.iter().position(|b| -b.debt().0 == max_credit)
            } else {
                per_person.iter().position(|b| b.debt().0 == max_debit)
            }
        });

    if let Some(i) = target {
        per_person[i].received += Cents(diff);
    }
}

/// One suggested payment from one person to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Transfer {
    pub from: PersonId,
    pub to: PersonId,
    pub amount: Cents,
}

#[derive(Debug, PartialEq)]
pub enum CalcError {
    /// The settlement loop did not run out of debtors in time. In C# this is a bare
    /// `throw new Exception(...)`; here the caller can see it in the type and decide.
    DidNotConverge { max: usize },
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalcError::DidNotConverge { max } => {
                write!(f, "settlement did not converge in {max} iterations")
            }
        }
    }
}

const MAX_ITERATIONS: usize = 500;

/// Works out the smallest set of payments that squares everyone up.
///
/// Each round picks the deepest creditor and the largest debtor and moves as much as it
/// can between them; that zeroes at least one of the two, so the loop shrinks the problem
/// every time. Family members are settled with their payer first, so that a family shows
/// up as one payment rather than four.
pub fn suggest_settlement(tour: &Tour) -> Result<Vec<Transfer>, CalcError> {
    let mut transfers = family_transfers(tour);

    for _ in 0..MAX_ITERATIONS {
        let balances = calculate_with(tour, &transfers);

        let (Some(creditor), Some(debtor)) =
            (balances.creditors().next(), balances.debtors().next())
        else {
            return Ok(transfers);
        };

        let credit = -creditor.debt();
        let debt = debtor.debt();
        transfers.push(Transfer {
            from: debtor.person.clone(),
            to: creditor.person.clone(),
            amount: if credit > debt { debt } else { credit },
        });
    }

    Err(CalcError::DidNotConverge {
        max: MAX_ITERATIONS,
    })
}

/// A child's debt is moved onto whoever pays for them, before anything else is settled.
fn family_transfers(tour: &Tour) -> Vec<Transfer> {
    let balances = calculate(tour, Options { with_planned: true });

    tour.persons
        .iter()
        .filter_map(|p| {
            let parent = p.parent.as_ref()?;
            // A parent who is not on the tour is not a parent.
            tour.person(parent)?;
            // Someone who is themselves paid for does not pay for others: that would be a
            // chain, and the C# code cuts those too.
            if tour
                .person(parent)
                .and_then(|pp| pp.parent.as_ref())
                .is_some()
            {
                return None;
            }
            let debt = balances.get(&p.id)?.debt();
            if debt.is_zero() {
                return None;
            }
            Some(Transfer {
                from: p.id.clone(),
                to: parent.clone(),
                amount: debt,
            })
        })
        .collect()
}

/// Recomputes balances as if the given transfers had been made.
fn calculate_with(tour: &Tour, transfers: &[Transfer]) -> Balances {
    let mut balances = calculate(tour, Options { with_planned: true });
    for t in transfers {
        if let Some(b) = balances.per_person.iter_mut().find(|b| b.person == t.from) {
            b.spent += t.amount;
        }
        if let Some(b) = balances.per_person.iter_mut().find(|b| b.person == t.to) {
            b.received += t.amount;
        }
    }
    balances
}
