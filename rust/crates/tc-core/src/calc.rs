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

use crate::domain::{Kind, Person, Spending, Split, Tour};
use crate::ids::{PersonId, SpendingId};
use crate::money::Cents;

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
    // `counted` borrows from `tour` - it is a list of references, not of copies. No
    // spending is duplicated anywhere in this function.
    let counted: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(opts.with_planned))
        .collect();

    calculate_over(tour, &counted)
}

/// The calculation proper, over an explicit list of spendings to count.
///
/// Split out from `calculate` because the settlement below has to add payments as it goes
/// and recompute over tour spendings *plus* its own - and it does that without copying the
/// tour, by passing a list of references that points into both. That the two lists have
/// different owners is invisible here: a `&Spending` is a `&Spending` whoever holds the
/// value it points at, as long as it outlives this call - which is what the borrow checker
/// verifies at the call site.
fn calculate_over(tour: &Tour, counted: &[&Spending]) -> Balances {
    if tour.persons.is_empty() {
        return Balances::default();
    }

    let total_weight = tour.total_weight();

    let mut per_person: Vec<PersonBalance> = tour
        .persons
        .iter()
        .map(|p| PersonBalance {
            person: p.id.clone(),
            spent: spent_by(p, tour, counted),
            received: received_by(p, tour, counted, total_weight),
        })
        .collect();

    settle_rounding(&mut per_person);

    Balances { per_person }
}

/// The sum of what this person paid for.
fn spent_by(person: &Person, tour: &Tour, counted: &[&Spending]) -> Cents {
    counted
        .iter()
        .filter(|s| s.from == person.id)
        .map(|s| tour.amount_in_current(s))
        .sum()
}

/// The sum of what was spent on this person.
///
/// Each spending's share is computed scaled up by `MAGNITUDE` and rounded once, which is
/// what keeps a tour's shares adding back up to the amounts that were actually paid.
fn received_by(person: &Person, tour: &Tour, counted: &[&Spending], total_weight: i64) -> Cents {
    let mut scaled_total: i128 = 0;

    for s in counted {
        let amount = tour.amount_in_current(s).0 as i128;

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
    /// The amount as stored - **not** necessarily in the currency the tour is shown in.
    ///
    /// A payment suggested now is made in the tour's current currency, but one already
    /// stored can be in any of the tour's currencies: one of the fixtures carries a planned
    /// payment in RSD on a tour displayed in EUR. Use `Tour::amount_in_current` to show it.
    pub amount: Cents,
    pub currency: crate::domain::Currency,
    /// What the app writes on the row: "Family 'x' -> 'y'" or "X 'x' -> 'y'".
    pub description: String,
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

/// Works out the payments that square everyone up.
///
/// Two passes, in this order, because the C# implementation does the same and the two have
/// to agree payment for payment:
///
/// 1. **Families.** Anyone paid for by somebody else settles with their payer first, so a
///    family shows up as one payment instead of four.
/// 2. **Cross payments.** Repeatedly take the deepest creditor and the largest debtor and
///    move as much as fits between them. That zeroes at least one of the two, so the
///    problem shrinks every round.
///
/// The subtlety - and the thing an earlier version of this file got wrong - is that each
/// payment is *appended to the spendings and everything recomputed*, rather than subtracted
/// from the balances. It matters because `settle_rounding` runs at the end of every
/// calculation and can hand the odd cent to a different person once the payments are in.
/// Adjusting balances directly gives a settlement that is just as valid and not the same,
/// and "just as valid" is no use to somebody comparing the screen with what it said before.
///
/// A tour that already carries planned payments - one loaded from storage - keeps them:
/// they take part in the sums and come back in the result, exactly as in C#.
pub fn suggest_settlement(tour: &Tour) -> Result<Vec<Transfer>, CalcError> {
    // Owned, and local to this call. `counted` below points into both this and `tour`.
    let mut planned: Vec<Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind == Kind::Planned)
        .cloned()
        .collect();

    suggest_families(tour, &mut planned);

    for _ in 0..=MAX_ITERATIONS {
        let balances = calculate_over(tour, &with_planned(tour, &planned));

        let Some((creditor, debtor)) = pick_pair(tour, &balances) else {
            return Ok(planned.iter().map(Transfer::of).collect());
        };

        let credit = -creditor.debt();
        let debt = debtor.debt();
        planned.push(payment(
            tour,
            debtor.person.clone(),
            creditor.person.clone(),
            if credit > debt { debt } else { credit },
            "X",
        ));
    }

    Err(CalcError::DidNotConverge {
        max: MAX_ITERATIONS,
    })
}

/// Chooses who pays whom next, or `None` when nobody owes anybody.
///
/// Deepest creditor and largest debtor, with two preferences on top - both inherited from
/// the C# implementation, and both of which looked dead until the data proved otherwise:
///
/// * a creditor whose group is marked with a leading "r" is paid back before the others;
/// * a debtor from the creditor's own group is left for last, because people who travel
///   together settle up between themselves anyway.
///
/// The seed data has two people sharing the group "araaa", and that is the whole reason the
/// suggested payments differ if this is left out.
fn pick_pair<'a>(
    tour: &Tour,
    balances: &'a Balances,
) -> Option<(&'a PersonBalance, &'a PersonBalance)> {
    let group_of = |b: &PersonBalance| tour.person(&b.person).and_then(|p| p.group.clone());
    let preferred = |b: &PersonBalance| {
        tour.person(&b.person)
            .is_some_and(Person::is_preferred_creditor)
    };

    let creditors: Vec<&PersonBalance> = balances.creditors().collect();
    let creditor = *creditors
        .iter()
        .find(|b| preferred(b))
        .or_else(|| creditors.first())?;

    let creditor_group = group_of(creditor);
    let debtors: Vec<&PersonBalance> = balances.debtors().collect();
    // Same group only counts when both actually have one: C# hands everyone a fresh random
    // id when the data has none, so two people with no group are never each other's group.
    let outside = |b: &PersonBalance| match (&creditor_group, group_of(b)) {
        (Some(a), Some(c)) => *a != c,
        _ => true,
    };
    let debtor = *debtors
        .iter()
        .find(|b| outside(b))
        .or_else(|| debtors.first())?;

    Some((creditor, debtor))
}

/// The balances as they would stand once the given payments have been made.
///
/// The app needs this to show what is left to settle, and it is also the only honest way to
/// check a settlement: subtracting the payments from the balances by hand is not the same
/// arithmetic, because `settle_rounding` runs at the end of every calculation and can put
/// the odd cent on a different person once the payments are in.
pub fn balances_after(tour: &Tour, transfers: &[Transfer]) -> Balances {
    let payments: Vec<Spending> = transfers
        .iter()
        .map(|t| {
            let mut sp = payment(tour, t.from.clone(), t.to.clone(), t.amount, "X");
            // Keep the transfer's own currency: rebuilding it in the tour's current one
            // would silently revalue a payment stored in another.
            sp.currency = t.currency.clone();
            sp
        })
        .collect();
    calculate_over(tour, &with_planned(tour, &payments))
}

/// Everything that counts, from the tour and from the payments suggested so far.
///
/// The returned references point into two different owners, and neither is copied. Both
/// outlive the borrow, which is all the compiler needs to know.
fn with_planned<'a>(tour: &'a Tour, planned: &'a [Spending]) -> Vec<&'a Spending> {
    tour.spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned && s.kind.counts(false))
        .chain(planned.iter())
        .collect()
}

/// Moves each dependant's balance onto whoever pays for them.
fn suggest_families(tour: &Tour, planned: &mut Vec<Spending>) {
    let balances = calculate_over(tour, &with_planned(tour, planned));

    // Who has a payer, owes or is owed something, and whose payer is actually on the tour.
    let dependants: Vec<&Person> = tour
        .persons
        .iter()
        .filter(|p| p.parent.is_some())
        .filter(|p| {
            balances
                .get(&p.id)
                .map(|b| !b.debt().is_zero())
                .unwrap_or(false)
        })
        .filter(|p| {
            p.parent
                .as_ref()
                .is_some_and(|id| tour.person(id).is_some())
        })
        .collect();

    // Break chains: somebody who is themselves paid for by another dependant is left out,
    // so that a debt is never moved twice.
    let payers_in_list: Vec<&PersonId> = dependants.iter().map(|p| &p.id).collect();
    let dependants: Vec<&Person> = dependants
        .iter()
        .filter(|p| {
            p.parent
                .as_ref()
                .is_some_and(|parent| !payers_in_list.contains(&parent))
        })
        .copied()
        .collect();

    for d in dependants {
        let parent = d.parent.clone().expect("filtered above");
        let debt = balances.get(&d.id).expect("filtered above").debt();
        let name_of = |id: &PersonId| {
            tour.person(id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "n/a".to_owned())
        };
        let description = format!("Family '{}' -> '{}'", name_of(&d.id), name_of(&parent));
        let mut sp = payment(tour, d.id.clone(), parent, debt, "Family");
        sp.description = description;
        planned.push(sp);
    }
}

/// A suggested payment, shaped like the spending the C# calculator appends.
fn payment(tour: &Tour, from: PersonId, to: PersonId, amount: Cents, tag: &str) -> Spending {
    let name_of = |id: &PersonId| {
        tour.person(id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "n/a".to_owned())
    };
    Spending {
        id: SpendingId::new(String::new()),
        description: format!("{tag} '{}' -> '{}'", name_of(&from), name_of(&to)),
        // Empty on purpose: a settling payment is not an expense, and the app leans on the
        // empty category to tell the two apart.
        category: String::new(),
        amount,
        currency: tour.currency().clone(),
        from,
        split: Split::Equally(vec![to]),
        remembered_split: None,
        kind: Kind::Planned,
        extras: Default::default(),
    }
}

impl Transfer {
    fn of(s: &Spending) -> Transfer {
        Transfer {
            from: s.from.clone(),
            to: match &s.split {
                Split::Equally(to) | Split::ByWeight(to) => {
                    to.first().cloned().unwrap_or_else(|| PersonId::new(""))
                }
                Split::Everyone => PersonId::new(""),
            },
            amount: s.amount,
            currency: s.currency.clone(),
            description: s.description.clone(),
        }
    }
}
