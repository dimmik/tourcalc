//! The People tab: families, the cards that open, and the itemised list behind every figure.
//!
//! Two ideas from the app are worth naming, because both are about the same thing - not
//! making the reader take a number on trust.
//!
//! **Families.** Somebody can be paid for by somebody else: a child, or anyone who does not
//! settle up themselves. They are drawn *inside* the block of whoever pays for them, behind
//! a bar that folds them away, so a family reads as one entry with its own total rather than
//! as people scattered down a list who happen to share a surname.
//!
//! **Breakdowns.** Each person shows three figures - what they paid, what their share cost,
//! and the difference - and each of the three opens the spendings it is made of. That is why
//! [`tc_core::breakdown`] exists: the numbers have to be explainable, and an explanation
//! that does not add up to the number above it is worse than none.

use crate::i18n::t;
use crate::edit::PersonDraft;
use crate::icon::Icon;
use crate::tour::{Dialog, Removal};
use crate::ui::{avatar_colour, initials, money, money_in, name_of};
use leptos::prelude::*;
use tc_core::{
    breakdown, calculate, settlement_for, will_pay, Cents, Options, Person, PersonId, Tour,
    Transfer,
};

/// A person who settles up, and everyone settled for through them.
#[derive(Clone)]
struct Family {
    head: Person,
    kids: Vec<Person>,
    /// The weight of the whole family - what the payer's share is actually worked out from.
    weight: i32,
}

/// The list, grouped.
///
/// "Paid for by" can be a chain, so the head is found by walking it to the end: whoever has
/// no one above them is who hands money over. The walk is bounded because the data cannot be
/// trusted not to contain a loop - a person marked as paid for by someone they pay for would
/// otherwise hang the tab, and a tour that renders oddly beats one that does not render.
fn families(tour: &Tour) -> Vec<Family> {
    let parent_of = |p: &Person| -> Option<&Person> {
        p.parent
            .as_ref()
            .and_then(|id| tour.person(id))
            // Somebody cannot be their own payer, whatever the file says.
            .filter(|up| up.id != p.id)
    };

    let head_of = |p: &Person| -> PersonId {
        let mut cur = p;
        for _ in 0..50 {
            match parent_of(cur) {
                Some(up) => cur = up,
                None => break,
            }
        }
        cur.id.clone()
    };

    // "Together" and then the name, as the app sorts - see `edit::order_key`.
    let by_name = |a: &Person, b: &Person| {
        crate::edit::order_key(a).cmp(&crate::edit::order_key(b))
    };

    let mut heads: Vec<&Person> = tour
        .persons
        .iter()
        .filter(|p| parent_of(p).is_none())
        .collect();
    heads.sort_by(|a, b| by_name(a, b));

    heads
        .into_iter()
        .map(|head| {
            let mut kids: Vec<Person> = tour
                .persons
                .iter()
                .filter(|p| parent_of(p).is_some() && head_of(p) == head.id)
                .cloned()
                .collect();
            kids.sort_by(|a, b| by_name(a, b));

            Family {
                weight: head.weight + kids.iter().map(|k| k.weight).sum::<i32>(),
                head: head.clone(),
                kids,
            }
        })
        .collect()
}

/// Somebody who could pay the next bill: whoever settles for a family, what the family owes,
/// and the bill for everyone that would leave them square.
#[derive(Clone, Debug, PartialEq)]
pub struct NextPayer {
    pub head: Person,
    pub owes: Cents,
    /// A bill split among everybody by weight, paid by this family, moves their balance by
    /// the bill less their own share of it: `bill × (1 − w/W)`. Evened out at
    /// `owes × W / (W − w)`, rounded down to a whole unit of the currency the tour is read in
    /// so that it never overshoots - a whole euro, a whole dinar, since amounts are stored in
    /// the smallest unit a currency counts in. None when the family is everybody - there is
    /// nobody else for them to pay for - and when that comes to less than one whole unit: a
    /// debt of 60 cents asks nobody to pay "a bill of up to about 0".
    pub evens_at: Option<Cents>,
}

/// Who had better pay next, the biggest debt first: the families that owe more than the
/// reader's threshold.
///
/// The biggest debtor paying the next shared bill is what keeps the settle-up short - their
/// debt shrinks, and nobody else's grows past what their own share adds. The amount is what
/// the card of whoever settles for the family says, so the two cannot disagree.
pub fn next_to_pay(tour: &Tour, transfers: &[Transfer], too_small: Cents) -> Vec<NextPayer> {
    let total = tour.persons.iter().map(|p| p.weight as i64).sum::<i64>();
    let whole = tour.currency().stored_per_whole().max(1) as i128;
    let mut next: Vec<NextPayer> = families(tour)
        .into_iter()
        .filter_map(|fam| {
            let owes = will_pay(tour, transfers, &fam.head.id, too_small);
            if owes <= too_small {
                return None;
            }
            let others = total - fam.weight as i64;
            let evens_at = (others > 0)
                .then(|| owes.0 as i128 * total as i128 / others as i128 / whole * whole)
                .filter(|bill| *bill > 0)
                .map(|bill| Cents(bill as i64));
            Some(NextPayer { head: fam.head, owes, evens_at })
        })
        .collect();
    next.sort_by(|a, b| b.owes.cmp(&a.owes));
    next
}

/// Which of the three figures a sheet is explaining.
#[derive(Clone, Copy, PartialEq)]
pub enum Which {
    Paid,
    Charged,
    Balance,
}

impl Which {
    fn label(self) -> &'static str {
        match self {
            Which::Paid => t().people.paid,
            Which::Charged => t().people.charged,
            Which::Balance => t().people.balance,
        }
    }
}

/// A balance as a sign and an amount: what they put in less what their share cost, which is
/// how the legend defines it. "+6 870" comes to them, "−3 205" is theirs to pay, and nothing
/// either way is "settled". `debt` counts the other way (positive is owing) - hence the flip.
///
/// Signs, not words: "gets 6 870" and "owes 3 205" took a column of their own, and a phone on
/// its side had room for the numbers but not for the words. The words have not gone - they
/// are the title and the label a screen reader says ([`balance_words`]). The sign once meant
/// the debt here ("−5 055" beside "gets 5 055"); one rule for every place ends that.
fn signed(debt: Cents) -> String {
    match debt.0 {
        0 => t().people.settled.to_owned(),
        n if n > 0 => format!("\u{2212}{}", money(debt)),
        _ => format!("+{}", money(Cents(-debt.0))),
    }
}

fn signed_in(debt: Cents, unit: &str) -> String {
    if debt.is_zero() || unit.is_empty() {
        signed(debt)
    } else {
        format!("{}\u{a0}{unit}", signed(debt))
    }
}

/// The same in words, with the amount and its currency: said, not shown - see [`signed`].
fn balance_words(amount: Cents, unit: &str) -> String {
    match amount.0 {
        0 => t().people.settled.to_owned(),
        n if n > 0 => format!("{} {}", t().people.owes, money_in(amount, unit)),
        _ => format!("{} {}", t().people.gets, money_in(Cents(-amount.0), unit)),
    }
}

/// Amounts below what this reader calls meaningful are rounding noise, shown as nothing.
fn meaningful(amount: Cents, too_small: Cents) -> Cents {
    if amount.abs() > too_small {
        amount
    } else {
        Cents::ZERO
    }
}

/// What the reader has done to the People tab: whose card is open, whose family is showing,
/// which sheet of numbers is up, and the search box.
///
/// Owned by the tour page rather than by this tab, for the reason [`crate::tour::Sifting`]
/// is: the screen below the page is rebuilt after every edit, and a card that saving an
/// expense folds back up is a card nobody can read while editing.
#[derive(Clone, Copy)]
pub struct People {
    /// Who is opened out, and whose family is showing - by id, since the list is rebuilt on
    /// every edit and a position means nothing across two versions of a tour.
    pub open: RwSignal<Vec<String>>,
    pub kids_open: RwSignal<Vec<String>>,
    pub sheet: RwSignal<Option<(Which, Person)>>,
    pub search: RwSignal<String>,
}

impl People {
    pub fn new() -> People {
        People {
            open: RwSignal::new(Vec::new()),
            kids_open: RwSignal::new(Vec::new()),
            sheet: RwSignal::new(None),
            search: RwSignal::new(String::new()),
        }
    }
}

/// Under this many people the list fits on a screen anyway, and a search box is clutter.
/// One number for both interfaces: at exactly eight, one of them had the box and the other
/// did not.
pub const SEARCH_FROM: usize = 8;

/// The weight almost everybody shares - which Mini leaves unmarked, its column too narrow for
/// the same number on every line. The roomy People list marks everybody's: a line without
/// "×" read there as a person without a weight.
///
/// On a tie - two at 100 and two at 50 - the full share, 100, if it is one of them, and
/// otherwise the heavier. `max_by_key` alone took whichever came last, so the same tour
/// marked "Anna ×100, Boris ×100" or "Vera ×50, Gosha ×50" depending on the order the
/// people were added in, and the first made the full shares look like the odd ones.
pub fn common_weight(tour: &Tour) -> i32 {
    let mut counts: Vec<(i32, usize)> = Vec::new();
    for p in &tour.persons {
        match counts.iter_mut().find(|(w, _)| *w == p.weight) {
            Some((_, n)) => *n += 1,
            None => counts.push((p.weight, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(w, n)| (n, w == 100, w))
        .map(|(w, _)| w)
        .unwrap_or(100)
}

#[component]
pub fn PeopleTab(
    tour: Tour,
    /// What the reader has opened and typed here. Owned by the page above, which is not
    /// rebuilt when the tour is saved.
    people: People,
    /// Every suggested payment, family ones included: what somebody hands over at settle-up
    /// is not the same number as what they owe, and both are shown.
    transfers: Vec<Transfer>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let People {
        open,
        kids_open,
        sheet,
        search,
    } = people;

    let fams = families(&tour);
    // Whether anybody on screen is open: somebody opened and then searched out of the list,
    // or tucked into a folded family, is not a figure the legend can be explaining.
    let showing_figures = {
        let tour = tour.clone();
        let fams = fams.clone();
        Memo::new(move |_| {
            let opened = open.get();
            if opened.is_empty() {
                return false;
            }
            let needle = search.get().trim().to_lowercase();
            let unfolded = kids_open.get();
            matching(&tour, &fams, &needle).iter().any(|fam| {
                let head = fam.head.id.as_str();
                opened.iter().any(|id| id == head)
                    || (unfolded.iter().any(|id| id == head)
                        && fam.kids.iter().any(|k| opened.iter().any(|id| id == k.id.as_str())))
            })
        })
    };
    let count = tour.persons.len();
    let searchable = count >= SEARCH_FROM;
    let tour_for_search = tour.clone();
    let everyone: Vec<String> = tour
        .persons
        .iter()
        .map(|p| p.id.as_str().to_owned())
        .collect();

    let tour_for_sheet = tour.clone();
    let tour_for_weight = tour.clone();
    let transfers_for_sheet = transfers.clone();
    let unit_for_sheet = unit.clone();
    let next = next_to_pay(&tour, &transfers, crate::settings::threshold(&tour));
    // A tour being settled up has its payments on the Balance tab; a bill still to come is
    // not what anybody there is deciding.
    let settling = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);
    let unit_for_next = unit.clone();
    let tour_for_next = tour.clone();

    view! {
        // First on the tab the tour opens on: at a table with the bill coming, it is the one
        // thing looked for, and the legend below belongs with the list it explains.
        {(!settling && !next.is_empty()).then(|| view! {
            <div class="tcn-section">
                <NextPayerCard tour=tour_for_next next=next unit=unit_for_next dialog=dialog />
            </div>
        })}
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <button type="button" class="tcn-btn tcn-btn-primary"
                        on:click=move |_| dialog.set(Some(Dialog::Person(PersonDraft::new())))>
                    {t().people.add_person}
                </button>
                <Show when=move || { searchable }>
                    <div class="tcn-search">
                        <span class="tcn-search-icon">"🔎"</span>
                        <input type="text" placeholder=t().people.find
                               prop:value=move || search.get()
                               on:input=move |ev| search.set(event_target_value(&ev)) />
                        <Show when=move || !search.get().is_empty()>
                            <button type="button" class="tcn-search-clear" title=t().people.clear
                                    on:click=move |_| search.set(String::new())>"✕"</button>
                        </Show>
                    </div>
                </Show>
                <Show when=move || { count > 1 }>
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            on:click={
                                let everyone = everyone.clone();
                                move |_| {
                                    let all = everyone.clone();
                                    // One button for both directions: anything open at
                                    // all - a person's numbers or a family's children -
                                    // means the next press closes.
                                    if open.get().is_empty() && kids_open.get().is_empty() {
                                        kids_open.set(all.clone());
                                        open.set(all);
                                    } else {
                                        // Families close with everybody else, which is the
                                        // app's own wording for it. Clearing only the
                                        // people left the children hanging open under a
                                        // head that had just folded - so "collapse all"
                                        // collapsed all but the one list still on screen.
                                        kids_open.set(Vec::new());
                                        open.set(Vec::new());
                                    }
                                }
                            }>
                        {move || if open.get().is_empty() && kids_open.get().is_empty() {
                            t().people.expand_all
                        } else {
                            t().people.collapse_all
                        }}
                    </button>
                </Show>
                <span class="tcn-chip">{(t().list.people)(count)}</span>
                <span class="tcn-chip">
                    {t().people.total_weight} " "
                    <crate::explain::Explain what={
                        let t = tour_for_weight.clone();
                        Callback::new(move |()| crate::explain::total_weight(&t))
                    }>
                        {tour.total_weight()}
                    </crate::explain::Explain>
                </span>
            </div>



            // What the three figures mean - where they are on the screen: always on a wide
            // one, on a phone once somebody is open. It explained three words nobody could
            // see while the list was folded.
            <Show when=move || { count > 0 }>
                <div class="tcn-hint tcw-legend" class:is-shown=move || showing_figures.get()
                     style="margin: -4px 2px 10px 2px">
                    <b>{t().people.paid}</b>{t().people.legend_paid}<b>{t().people.charged}</b>
                    {t().people.legend_charged}<b>{t().people.balance}</b>
                    {t().people.legend_balance}
                </div>
            </Show>
        </div>

        {if fams.is_empty() {
            view! {
                <div class="tcn-section">
                    <div class="tcn-empty">
                        <span class="tcn-empty-icon">"👥"</span>
                        <div class="tcn-empty-title">{t().people.nobody_yet}</div>
                        <div>{t().people.nobody_yet_hint}</div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {
                <div class="tcn-section">
                    <div class="tcn-list tcn-people-list is-compact">
                        {move || {
                            let needle = search.get().trim().to_lowercase();
                            let shown = matching(&tour_for_search, &fams, &needle);
                            if shown.is_empty() {
                                return view! {
                                    <div class="tcn-empty">
                                        <div class="tcn-empty-title">
                                            {(t().people.nobody_called)(&search.get())}
                                        </div>
                                        <div>{t().people.shorter}</div>
                                    </div>
                                }.into_any();
                            }
                            shown
                                .into_iter()
                                .map(|fam| view! {
                                    <FamilyBlock tour=tour_for_search.clone() fam=fam
                                                 transfers=transfers.clone() open=open
                                                 kids_open=kids_open sheet=sheet
                                                 dialog=dialog delete=delete />
                                })
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </div>
            }.into_any()
        }}

        {move || sheet.get().map(|(which, person)| view! {
            <BreakdownSheet tour=tour_for_sheet.clone() person=person which=which
                            transfers=transfers_for_sheet.clone() unit=unit_for_sheet.clone()
                            close=Callback::new(move |_: ()| sheet.set(None)) />
        })}
    }
}

/// "Who pays next · Anna": the biggest debt, the bill that evens it out, and who comes after.
#[component]
fn NextPayerCard(
    tour: Tour,
    next: Vec<NextPayer>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
) -> impl IntoView {
    let first = next[0].clone();
    let then = next.get(1).cloned();
    // "Dima, you pay - I'll put it down": the expense is recorded from this phone in their
    // name, and the next "+ Spend" still starts from whoever holds the phone.
    let payer = first.head.id.clone();
    let record = move |_| dialog.set(Some(Dialog::Spending(
        crate::edit::SpendingDraft::paid_by(&tour, &payer),
    )));
    view! {
        <div class="tcw-next" title=t().people.next_hint>
            <div class="tcw-next-head">
                <span class="tcw-next-title">{t().people.next_title}</span>
                <b class="tcw-next-name">{first.head.name.clone()}</b>
            </div>
            <div>
                {(t().people.next_owes)(&money_in(first.owes, &unit))}
                {first.evens_at.map(|bill| {
                    format!(" {}", (t().people.next_evens)(&money_in(bill, &unit)))
                })}
            </div>
            {then.map(|n| view! {
                <div class="tcn-hint">
                    {(t().people.next_then)(&n.head.name, &money_in(n.owes, &unit))}
                </div>
            })}
            <div>
                <button type="button" class="tcn-btn tcn-btn-sm tcw-pays-for" on:click=record>
                    {(t().people.next_spend)(&first.head.name)}
                </button>
            </div>
        </div>
    }
}

#[component]
fn FamilyBlock(
    tour: Tour,
    fam: Family,
    transfers: Vec<Transfer>,
    open: RwSignal<Vec<String>>,
    kids_open: RwSignal<Vec<String>>,
    sheet: RwSignal<Option<(Which, Person)>>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let head_id = fam.head.id.as_str().to_owned();
    let covers = fam.kids.len();
    let names = fam
        .kids
        .iter()
        .map(|k| k.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let head_name = fam.head.name.clone();

    // A `Memo` rather than a closure: it is `Copy`, so every place in the view that asks
    // whether the family is showing gets the same cheap handle instead of its own clone.
    // A plain closure would be moved into the first one that used it.
    let showing = Memo::new({
        let head_id = head_id.clone();
        move |_| kids_open.get().iter().any(|id| id == &head_id)
    });
    let toggle_kids = {
        let head_id = head_id.clone();
        move |_| {
            let id = head_id.clone();
            kids_open.update(|v| match v.iter().position(|x| x == &id) {
                Some(i) => {
                    v.remove(i);
                }
                None => v.push(id),
            })
        }
    };

    let kids = fam.kids.clone();

    view! {
        <div class="tcn-family" class:has-kids=move || { covers > 0 }
             class:is-open=move || { covers > 0 && showing.get() }>
            <PersonBlock tour=tour.clone() person=fam.head.clone() covers=covers
                         family_weight=fam.weight transfers=transfers.clone()
                         open=open sheet=sheet dialog=dialog delete=delete />

            <Show when=move || { covers > 0 }>
                <button type="button" class="tcn-familybar" aria-expanded=move || showing.get().to_string()
                        on:click=toggle_kids.clone()>
                    <span>{move || if showing.get() { view!{<Icon name="chevron-down"/>} } else { view!{<Icon name="chevron-right"/>} }}</span>
                    <span class="tcn-familybar-names">{names.clone()}</span>
                    <span class="tcn-familybar-note">{t().people.paid_for_by} " " {head_name.clone()}</span>
                </button>
            </Show>

            <Show when=move || { covers > 0 && showing.get() }>
                <div class="tcn-family-kids">
                    {kids
                        .iter()
                        .map(|kid| view! {
                            <PersonBlock tour=tour.clone() person=kid.clone() covers=0
                                         family_weight=0 transfers=transfers.clone()
                                         open=open sheet=sheet
                                         dialog=dialog delete=delete />
                        })
                        .collect_view()}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn PersonBlock(
    tour: Tour,
    person: Person,
    /// How many people this person pays for. Zero for everybody else.
    covers: usize,
    family_weight: i32,
    transfers: Vec<Transfer>,
    open: RwSignal<Vec<String>>,
    sheet: RwSignal<Option<(Which, Person)>>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let id = person.id.as_str().to_owned();
    let is_child = person.parent.is_some();
    let balances = calculate(&tour, Options::default());
    let too_small = crate::settings::threshold(&tour);
    let debt = meaningful(
        balances
            .get(&person.id)
            .map(|b| b.debt())
            .unwrap_or_default(),
        too_small,
    );

    // What actually changes hands. For somebody who pays for others it carries their
    // people too, which is a different number from their own debt - hence both being shown.
    //
    // The reader's own threshold, not zero. Zero used to let a cent of rounding decide the
    // side: a person owed 15 628 had a two-cent payment to make, was therefore a payer of
    // two cents, and `meaningful` below turned that into "settled" - beside a Balance tab
    // listing the 15 628 coming to them. See `settlement_for`.
    let will_pay = meaningful(will_pay(&tour, &transfers, &person.id, too_small), too_small);

    // Somebody who is paid for hands nothing over themselves; showing them as "settled"
    // while they plainly owe something was confusing, so they show their own debt.
    let shown = if is_child { debt } else { will_pay };
    let split_family = !is_child && (will_pay - debt).abs() > too_small;

    // `Copy`, and so usable from every closure in the view without a clone each.
    let is_open = Memo::new({
        let id = id.clone();
        move |_| open.get().iter().any(|x| x == &id)
    });
    let toggle = {
        let id = id.clone();
        move |_| {
            let id = id.clone();
            open.update(|v| match v.iter().position(|x| x == &id) {
                Some(i) => {
                    v.remove(i);
                }
                None => v.push(id),
            })
        }
    };

    let chip_class = if is_child || shown.is_zero() {
        ""
    } else if shown.0 > 0 {
        "tcn-chip-red"
    } else {
        "tcn-chip-green"
    };
    let unit = crate::ui::unit(&tour);
    let words = balance_words(shown, &unit);
    let unit_for_stats = StoredValue::new(unit.clone());
    let paid = balances.get(&person.id).map(|b| b.spent).unwrap_or_default();
    let charged = balances.get(&person.id).map(|b| b.received).unwrap_or_default();
    let paid_by = person
        .parent
        .as_ref()
        .and_then(|p| tour.person(p))
        .map(|p| p.name.clone());

    let weight = person.weight;

    let for_edit = person.clone();
    let for_delete = person.clone();
    // Whom they pay for - only somebody who pays for themselves can pay for others.
    let payer = person.parent.is_none().then(|| person.id.clone());
    let person_for_why = person.clone();
    let person_for_pweight = person.clone();
    let tour_for_why = tour.clone();
    let tour_for_pweight = tour.clone();
    let balances_for_why = balances.clone();
    let transfers_for_why = transfers.clone();
    let for_spend = person.clone();
    // The row has its own 💸, so it needs its own copies to hand to the dialog.
    let for_compact_spend = person.clone();
    let for_stats = person.clone();
    let for_row_stats = person.clone();
    let for_row_edit = person.clone();
    let toggle_row = toggle;
    let words_row = words;
    let name_row = person.name.clone();
    let colour_row = avatar_colour(&person.name);
    let letters_row = initials(&person.name);
    let tour_for_spend = tour.clone();
    let tour_for_compact_spend = tour.clone();

    view! {
        <div class="tcn-person is-compact" class:is-child=move || is_child
             class:is-open=move || is_open.get()>
            // One row, whatever the width and whether or not the person is open: opening adds
            // what is below it and moves nothing in it. The roomy card this replaced swapped
            // the row for a different one - the name moved, the weight changed places, 💸
            // vanished from under the finger that had just found it.
            <div class="tcn-person-row">
                <button type="button" class="tcn-person-open"
                        aria-expanded=move || is_open.get().to_string()
                        on:click=toggle_row.clone()>
                    <span class="tcn-avatar tcn-avatar-sm"
                          style=format!("background:{colour_row}")>{letters_row.clone()}</span>
                    <span class="tcn-person-id">
                        <span class="tcn-person-name">{name_row.clone()}</span>
                        // Everybody's weight, the common one too: with it left out, a line
                        // with no "×" read as a person with no weight, and the reader had to
                        // open them to find out it was 100. A family shows what the family
                        // weighs - the payer's own is in the open block.
                        {if covers > 0 {
                            view! {
                                <span class="tcn-chip tcn-chip-kids"
                                      title=(t().people.family_hint)(covers, family_weight as i64)>
                                    "👥" {covers} " ×" {family_weight}
                                </span>
                            }.into_any()
                        } else {
                            view! {
                                <span class="tcn-person-meta" title=(t().people.weight_hint)(weight as i64)>
                                    "×" {weight}
                                </span>
                            }.into_any()
                        }}
                    </span>
                    <span class=format!("tcn-chip tcw-row-chip {chip_class}") title=words_row.clone()>
                        {signed_in(shown, &unit_for_stats.get_value())}
                    </span>
                    <span class="tcn-person-caret">
                        {move || if is_open.get() { view!{<Icon name="chevron-down"/>} } else { view!{<Icon name="chevron-right"/>} }}
                    </span>
                </button>
                // On a wide screen the row is a line of a table: the three figures and the
                // everyday buttons are in it, and nothing needs opening to be seen.
                <div class="tcw-row-figs">
                    <Stat which=Which::Paid person=for_row_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=paid own=None />
                    <Stat which=Which::Charged person=for_row_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=charged own=None />
                    // One line, like the chip it stands in for: the payer's own debt would make
                    // it two, so that one is in the open block's facts.
                    <Stat which=Which::Balance person=for_row_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=shown own=None muted=is_child />
                </div>
                // Edit last, so that it stands in one column down the list: somebody paid for
                // has no "+ Pays for…", and with Edit first theirs slid right into its place.
                <div class="tcw-row-acts">
                    {payer.clone().map(|who| view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcw-pays-for"
                                on:click=move |_| dialog.set(Some(Dialog::Dependants(who.clone())))>
                            {t().people.pays_for_button}
                        </button>
                    })}
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            on:click={
                                let who = for_row_edit.clone();
                                move |_| dialog.set(Some(Dialog::Person(PersonDraft::of(&who))))
                            }>
                        {t().people.edit}
                    </button>
                </div>
                <button type="button"
                        class="tcn-btn tcn-btn-sm tcn-btn-primary tcn-person-spend"
                        title=(t().people.spend_for)(&person.name)
                        aria-label=(t().people.spend_for)(&person.name)
                        on:click={
                            let tour = tour_for_compact_spend.clone();
                            let who = for_compact_spend.clone();
                            move |_| {
                                let draft = crate::edit::SpendingDraft::paid_by(&tour, &who.id);
                                dialog.set(Some(Dialog::Spending(draft)));
                            }
                        }>
                    "💸"
                </button>
            </div>

            <Show when=move || is_open.get()>
              // One block, so that where the row already carries the figures and the buttons,
              // Delete - all that is left - sits at the end of the facts' line instead of on a
              // line of its own under every open person.
              <div class="tcw-open">
                // What the roomy card's head used to carry, and the row has no room for.
                <div class="tcn-person-facts">
                    <span>
                        {t().people.weight} " "
                        <crate::explain::Explain what={
                            let t = tour_for_pweight.clone();
                            let who = person_for_pweight.clone();
                            Callback::new(move |()| crate::explain::person_weight(&t, &who))
                        }>
                            <b>{weight}</b>
                        </crate::explain::Explain>
                    </span>
                    {paid_by.clone().map(|n| view! { <span>{t().people.paid_by} " " <b>{n}</b></span> })}
                    {(covers > 0).then(|| view! {
                        <span>{t().people.pays_for} " " <b>{covers}</b></span>
                        <span title=t().people.family_weight_hint>
                            {t().people.family_weight} " " <b>{family_weight}</b>
                        </span>
                    })}
                    <span>
                        <crate::explain::Explain what={
                            let t = tour_for_why.clone();
                            let who = person_for_why.clone();
                            let b = balances_for_why.clone();
                            let all = transfers_for_why.clone();
                            Callback::new(move |()| crate::explain::person_balance(&t, &who, &b, &all))
                        }>
                            {t().people.why_balance}
                        </crate::explain::Explain>
                    </span>
                    // Where the row carries the figures: on a phone the Balance cell below
                    // says it already.
                    {split_family.then(|| view! {
                        <span class="tcw-table-only">
                            {t().people.own} " "
                            <b title=balance_words(debt, &unit_for_stats.get_value())>
                                {signed_in(debt, &unit_for_stats.get_value())}
                            </b>
                        </span>
                    })}
                </div>
                // On a phone: the figures and buttons the wide row already shows.
                <div class="tcn-person-stats tcw-narrow-only">
                    <Stat which=Which::Paid person=for_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=paid own=None />
                    <Stat which=Which::Charged person=for_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=charged own=None />
                    <Stat which=Which::Balance person=for_stats.clone() sheet=sheet unit=unit_for_stats.get_value()
                          amount=shown own=split_family.then_some(debt) muted=is_child />
                </div>
                <div class="tcn-person-actions">
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary tcw-narrow-only"
                            on:click={
                                let tour = tour_for_spend.clone();
                                let who = for_spend.clone();
                                move |_| {
                                    let draft = crate::edit::SpendingDraft::paid_by(&tour, &who.id);
                                    dialog.set(Some(Dialog::Spending(draft)));
                                }
                            }>
                        {t().people.spend}
                    </button>
                    <button type="button" class="tcn-btn tcn-btn-sm tcw-below-wide"
                            on:click={
                                let who = for_edit.clone();
                                move |_| dialog.set(Some(Dialog::Person(PersonDraft::of(&who))))
                            }>
                        {t().people.edit}
                    </button>
                    {payer.clone().map(|who| view! {
                        <button type="button" class="tcn-btn tcn-btn-sm tcw-pays-for tcw-below-wide"
                                on:click=move |_| dialog.set(Some(Dialog::Dependants(who.clone())))>
                            {t().people.pays_for_button}
                        </button>
                    })}
                    // Not in the wide row: a button that deletes somebody does not belong
                    // one slip away from Edit on every line of the list.
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                            on:click={
                                let who = for_delete.clone();
                                move |_| delete.run(Removal::Person(who.clone()))
                            }>
                        {t().people.delete}
                    </button>
                </div>
              </div>
            </Show>
        </div>
    }
}

/// One of the three figures on a card, and the button that opens the list behind it.
#[component]
fn Stat(
    which: Which,
    person: Person,
    amount: Cents,
    /// Their own debt, when it differs from what they will actually hand over.
    own: Option<Cents>,
    sheet: RwSignal<Option<(Which, Person)>>,
    unit: String,
    /// Somebody paid for: their balance is theirs to know, not theirs to settle, and it is
    /// drawn in grey - as their chip is, and as the help says - not red or green.
    #[prop(optional)]
    muted: bool,
) -> impl IntoView {
    let tint = match which {
        Which::Paid => "is-paid",
        Which::Charged => "",
        Which::Balance if muted => "",
        Which::Balance if amount.0 > 0 => "is-owing",
        Which::Balance if amount.0 < 0 => "is-owed",
        Which::Balance => "",
    };

    // What the sign means, for a reader who hovers and for one who listens.
    let said = (which == Which::Balance).then(|| balance_words(amount, &unit));

    view! {
        <div class="tcn-stat">
            <span class="tcn-stat-label">{which.label()}</span>
            <button type="button" class=format!("tcn-statbtn {tint}")
                    title=said.clone() aria-label=said
                    on:click=move |_| sheet.set(Some((which, person.clone())))>
                {if which == Which::Balance { signed(amount) } else { money(amount) }}
                {(!(which == Which::Balance && amount.is_zero()) && !unit.is_empty())
                    .then(|| view! { <small>"\u{a0}" {unit.clone()}</small> })}
                {own.map(|d| view! {
                    <span class="tcn-statbtn-note">
                        {t().people.own} " " {signed_in(d, &unit)}
                    </span>
                })}
            </button>
        </div>
    }
}

/// The itemised list behind one figure.
#[component]
fn BreakdownSheet(
    tour: Tour,
    person: Person,
    which: Which,
    transfers: Vec<Transfer>,
    unit: String,
    close: Callback<()>,
) -> impl IntoView {
    let it = breakdown(&tour, &person.id, Options::default());
    let balances = calculate(&tour, Options::default());
    let paid = balances
        .get(&person.id)
        .map(|b| b.spent)
        .unwrap_or_default();
    let charged = balances
        .get(&person.id)
        .map(|b| b.received)
        .unwrap_or_default();

    let title = match which {
        Which::Paid => (t().people.sheet_paid)(&person.name, &money_in(paid, &unit)),
        Which::Charged => (t().people.sheet_charged)(&person.name, &money_in(charged, &unit)),
        Which::Balance => {
            // The itemised view ignores dust, which can flip somebody the card calls
            // "settled" into somebody collecting five thousand. Both are the app's.
            let owed = will_pay(&tour, &transfers, &person.id, crate::settings::threshold(&tour));
            let amount = money_in(owed.abs(), &unit);
            if owed.0 >= 0 {
                (t().people.sheet_will_pay)(&person.name, &amount)
            } else {
                (t().people.sheet_will_collect)(&person.name, &amount)
            }
        }
    };

    let colour = avatar_colour(&person.name);
    let letters = initials(&person.name);

    view! {
        <div class="tcn-modal tcn-sheet" on:click=move |_| close.run(())>
            <div class="tcn-modal-card" on:click=|ev| ev.stop_propagation()>
                <div class="tcn-sheet-head">
                    <span class="tcn-avatar tcn-avatar-sm" style=format!("background:{colour}")>
                        {letters}
                    </span>
                    <div class="tcn-sheet-title">{title}</div>
                    <button type="button" class="tcn-sheet-x" title=t().people.close
                            on:click=move |_| close.run(())><Icon name="close" /></button>
                </div>

                <div class="tcn-sheet-body">
                    {match which {
                        Which::Balance => view! {
                            <SettleRows tour=tour.clone() person=person.clone()
                                        transfers=transfers.clone() unit=unit.clone() />
                        }.into_any(),
                        _ => {
                            let lines = if which == Which::Paid { it.paid } else { it.charged };
                            view! { <Lines tour=tour.clone() lines=lines unit=unit.clone()
                                           charged={which == Which::Charged} /> }.into_any()
                        }
                    }}
                </div>

                <div class="tcn-sheet-foot">
                    <span style="flex:1 1 auto"></span>
                    <button type="button" class="tcn-btn tcn-btn-primary"
                            on:click=move |_| close.run(())>{t().people.got_it}</button>
                </div>
            </div>
        </div>
    }
}

/// The spendings behind Paid or Charged, with a running total down the side.
#[component]
fn Lines(
    tour: Tour,
    lines: Vec<tc_core::Line>,
    unit: String,
    /// Charged shows who paid and what fraction of the spending this was; Paid does not.
    charged: bool,
) -> impl IntoView {
    if lines.is_empty() {
        return view! { <div class="tcn-empty">{t().people.nothing_recorded}</div> }.into_any();
    }

    let mut running = Cents::ZERO;
    let rows: Vec<_> = lines
        .into_iter()
        .map(|line| {
            running += line.amount;
            // What the whole spending came to, so a share can be shown as a share.
            let whole = line
                .spending
                .as_ref()
                .and_then(|id| tour.spendings.iter().find(|s| &s.id == id))
                .map(|s| tour.amount_in_current(s));
            let from = line
                .from
                .as_ref()
                .map(|id| name_of(tour.person(id)))
                .filter(|_| charged);
            (line, whole, from, running)
        })
        .collect();

    view! {
        <div class="tcn-brk">
            {rows
                .into_iter()
                .map(|(line, whole, from, running)| {
                    let share = whole.filter(|w| charged && !w.is_zero()).map(|w| {
                        (t().people.share_of)(line.amount.0 as f64 * 100.0 / w.0 as f64, &money(w))
                    });
                    view! {
                        <div class="tcn-brk-row">
                            <div class="tcn-brk-main">
                                <div class="tcn-brk-title">{line.description}</div>
                                <div class="tcn-brk-sub">
                                    {(!line.category.trim().is_empty()).then(|| view! {
                                        <span class="tcn-chip tcn-chip-primary">{line.category}</span>
                                    })}
                                    {from.map(|n| view! { <span>{t().people.from} " " <b>{n}</b></span> })}
                                </div>
                            </div>
                            <div class="tcn-brk-amt">
                                {money(line.amount)} " " <small>{unit.clone()}</small>
                                {share.map(|s| view! { <div class="tcn-brk-note">{s}</div> })}
                                <div class="tcn-brk-note">{t().people.running} " " {money(running)}</div>
                            </div>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

/// The payments this person makes or receives when the tour is settled.
#[component]
fn SettleRows(tour: Tour, person: Person, transfers: Vec<Transfer>, unit: String) -> impl IntoView {
    // The same call decides the direction and the rows, so the summary above them and the
    // list below can never say different things.
    let (paying, rows) =
        settlement_for(&tour, &transfers, &person.id, crate::settings::threshold(&tour));
    let total: Cents = rows
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();

    let rows: Vec<(String, Cents)> = rows
        .into_iter()
        .map(|t| {
            let other = if paying { &t.to } else { &t.from };
            (
                name_of(tour.person(other)),
                tour.convert(t.amount, &t.currency),
            )
        })
        .collect();
    let empty = rows.is_empty();

    view! {
        <div class="tcn-brk-summary" class:is-owing=move || paying class:is-owed=move || !paying>
            <div class="tcn-brk-summary-label">
                {if paying { t().people.needs_to_pay } else { t().people.will_collect }}
            </div>
            <div class="tcn-brk-summary-value">
                {money(total)} " " <small>{unit.clone()}</small>
            </div>
        </div>

        {if empty {
            view! {
                <div class="tcn-empty" style="margin-top:10px">{t().people.nothing_left}</div>
            }.into_any()
        } else {
            view! {
                <div class="tcn-brk" style="margin-top:10px">
                    {rows
                        .into_iter()
                        .map(|(other, amount)| view! {
                            <div class="tcn-brk-row" class:is-owing=move || paying
                                 class:is-owed=move || !paying>
                                <div class="tcn-brk-main">
                                    <div class="tcn-brk-title">
                                        <span class="tcn-avatar tcn-avatar-sm"
                                              style=format!("background:{}", avatar_colour(&other))>
                                            {initials(&other)}
                                        </span>
                                        {if paying { t().people.to } else { t().people.from }} " " <b>{other}</b>
                                    </div>
                                </div>
                                <div class="tcn-brk-amt">
                                    {money(amount)} " " <small>{unit.clone()}</small>
                                </div>
                            </div>
                        })
                        .collect_view()}
                </div>
            }.into_any()
        }}
    }
}

/// The families to draw for a search.
///
/// A family whose payer matches is shown whole - looking for "Саша" and being handed Саша
/// without the three people he pays for would be a strange answer. A family the payer does
/// not match keeps only the members who do.
fn matching(tour: &Tour, families: &[Family], needle: &str) -> Vec<Family> {
    if needle.is_empty() {
        return families.to_vec();
    }
    let hit = |p: &Person| -> bool {
        p.name.to_lowercase().contains(needle)
            // And by the name of whoever pays for them: you look for the family, not for
            // the child you had forgotten was part of it.
            || p.parent
                .as_ref()
                .and_then(|id| tour.person(id))
                .is_some_and(|payer| payer.name.to_lowercase().contains(needle))
    };

    families
        .iter()
        .filter_map(|fam| {
            if hit(&fam.head) {
                return Some(fam.clone());
            }
            let kids: Vec<Person> = fam.kids.iter().filter(|k| hit(k)).cloned().collect();
            (!kids.is_empty()).then(|| Family {
                head: fam.head.clone(),
                kids,
                weight: fam.weight,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tc_core::suggest_settlement;

    /// Anna paid 300 for everybody, and Boris gave her `back` of it.
    fn dinner(people: serde_json::Value, back: i64) -> Tour {
        let json = serde_json::json!({
            "Id": "t", "Name": "t",
            "Persons": people,
            "Spendings": [
                {"GUID": "a", "Description": "dinner", "Type": "Food", "AmountInCents": 30000,
                 "FromGuid": "anna", "ToAll": true, "ToGuid": []},
                {"GUID": "b", "Description": "", "Type": "", "AmountInCents": back,
                 "FromGuid": "boris", "ToAll": false, "ToGuid": ["anna"]}
            ]
        });
        Tour::from_json(&json.to_string()).expect("tour")
    }

    fn next(tour: &Tour) -> Vec<(String, i64, Option<i64>)> {
        let transfers = suggest_settlement(tour).expect("settles");
        next_to_pay(tour, &transfers, Cents(0))
            .into_iter()
            .map(|n| (n.head.name, n.owes.0, n.evens_at.map(|c| c.0)))
            .collect()
    }

    #[test]
    fn whoever_owes_the_most_is_first_and_whoever_paid_is_not_there() {
        let tour = dinner(
            serde_json::json!([
                {"GUID": "anna", "Name": "Anna", "Weight": 100},
                {"GUID": "boris", "Name": "Boris", "Weight": 100},
                {"GUID": "vera", "Name": "Vera", "Weight": 100},
                {"GUID": "gleb", "Name": "Gleb", "Weight": 100}
            ]),
            3000,
        );
        // 75 each, Boris 45 after the 30 he gave back. A bill of B for all four moves
        // whoever pays it by three quarters of B.
        let got = next(&tour);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].1, 7_500);
        assert_eq!(got[0].2, Some(10_000));
        assert_eq!(got[1].1, 7_500);
        assert_eq!(got[2], ("Boris".into(), 4_500, Some(6_000)));
    }

    /// A family owes as one, and is named by whoever pays for it; its bill is bigger, since
    /// more of it comes back to them as their own share.
    #[test]
    fn a_family_owes_as_one() {
        let tour = dinner(
            serde_json::json!([
                {"GUID": "anna", "Name": "Anna", "Weight": 100},
                {"GUID": "boris", "Name": "Boris", "Weight": 100},
                {"GUID": "kid", "Name": "Kid", "Weight": 100, "ParentId": "boris"},
                {"GUID": "vera", "Name": "Vera", "Weight": 100}
            ]),
            0,
        );
        // Boris's family: 150 of the 300. A bill of B moves them by B/2: 300 evens it.
        assert_eq!(
            next(&tour),
            vec![("Boris".into(), 15_000, Some(30_000)), ("Vera".into(), 7_500, Some(10_000))],
        );
    }

    /// Stored amounts are in the smallest unit a currency counts in. Rounding by a hundred
    /// regardless made a dinar bill of 1 645 read "about 1 600", and a debt of 60 cents
    /// "a bill of up to about 0".
    #[test]
    fn the_bill_is_rounded_to_a_whole_unit_of_the_currency() {
        let four = serde_json::json!([
            {"GUID": "anna", "Name": "Anna", "Weight": 100},
            {"GUID": "boris", "Name": "Boris", "Weight": 100},
            {"GUID": "vera", "Name": "Vera", "Weight": 100},
            {"GUID": "gleb", "Name": "Gleb", "Weight": 100}
        ]);
        let in_currency = |cents: bool, spent: i64| {
            let json = serde_json::json!({
                "Id": "t", "Name": "t", "Persons": four,
                "Currencies": [{"_id": "C", "Name": "C", "CurrencyRate": 10000, "WithCents": cents}],
                "TourCurrencyId": "C",
                "Spendings": [
                    {"GUID": "a", "Description": "dinner", "Type": "Food", "AmountInCents": spent,
                     "FromGuid": "anna", "ToAll": true, "ToGuid": []}
                ]
            });
            Tour::from_json(&json.to_string()).expect("tour")
        };
        // Dinars: 4 936 spent, 1 234 each; three quarters of a bill is 1 234 at 1 645.
        assert_eq!(next(&in_currency(false, 4_936))[0].2, Some(1_645));
        // Euros: 12,34 each; 16,45 is 16 whole euros.
        assert_eq!(next(&in_currency(true, 4_936))[0].2, Some(1_600));
        // 0,60 € each: 0,80 is not a bill anybody pays.
        assert_eq!(next(&in_currency(true, 240))[0].1, 60);
        assert_eq!(next(&in_currency(true, 240))[0].2, None);
    }

    /// The sign is the balance - put in less charged - so owing is minus, whatever `debt`
    /// counts in; and nothing either way says so in words.
    #[test]
    fn a_balance_is_a_sign_and_nothing_is_settled() {
        assert!(signed(Cents(320_500)).starts_with('\u{2212}'), "owes 3 205: minus");
        assert!(signed(Cents(-687_000)).starts_with('+'), "gets 6 870: plus");
        assert_eq!(signed(Cents(0)), t().people.settled);
        assert!(!signed_in(Cents(0), "RUB").contains("RUB"), "no unit on settled");
    }

    /// Two at 100 and two at 50: the full share is the common one, whichever order the
    /// people were added in; with no 100 in the tie, the heavier.
    #[test]
    fn a_tie_for_the_common_weight_goes_to_the_full_share() {
        let tour_of = |weights: &[i32]| {
            let people: Vec<serde_json::Value> = weights
                .iter()
                .enumerate()
                .map(|(i, w)| serde_json::json!({"GUID": format!("p{i}"), "Name": format!("P{i}"), "Weight": w}))
                .collect();
            Tour::from_json(&serde_json::json!({"Id": "t", "Name": "t", "Persons": people, "Spendings": []}).to_string())
                .expect("tour")
        };
        assert_eq!(common_weight(&tour_of(&[100, 100, 50, 50])), 100);
        assert_eq!(common_weight(&tour_of(&[50, 50, 100, 100])), 100);
        assert_eq!(common_weight(&tour_of(&[60, 60, 80, 80])), 80);
        assert_eq!(common_weight(&tour_of(&[50, 100, 50])), 50, "a majority is a majority");
    }

    #[test]
    fn nobody_is_suggested_when_nobody_owes() {
        let tour = dinner(serde_json::json!([{"GUID": "anna", "Name": "Anna", "Weight": 100}]), 0);
        assert!(next(&tour).is_empty());
    }
}
