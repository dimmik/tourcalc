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

use crate::edit::PersonDraft;
use crate::tour::{Dialog, Removal};
use crate::ui::{avatar_colour, initials, money, name_of};
use leptos::prelude::*;
use tc_core::{
    breakdown, calculate, settlement_for, will_pay, Cents, Options, Person, PersonId, Tour,
    Transfer, MINIMUM_MEANINGFUL,
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

    let by_name = |a: &Person, b: &Person| a.name.to_lowercase().cmp(&b.name.to_lowercase());

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

/// Which of the three figures a sheet is explaining.
#[derive(Clone, Copy, PartialEq)]
enum Which {
    Paid,
    Charged,
    Balance,
}

impl Which {
    fn label(self) -> &'static str {
        match self {
            Which::Paid => "Paid",
            Which::Charged => "Charged",
            Which::Balance => "Balance",
        }
    }
}

/// A debt in words, so that a minus sign never has to be interpreted.
fn direction(amount: Cents) -> &'static str {
    if amount.0 > 0 {
        "owes"
    } else if amount.0 < 0 {
        "gets"
    } else {
        "settled"
    }
}

/// The same, with the amount: what the chip on a card says.
fn balance_words(amount: Cents) -> String {
    match amount.0 {
        0 => "settled".to_owned(),
        n if n > 0 => format!("owes {}", money(amount)),
        _ => format!("gets {}", money(Cents(-amount.0))),
    }
}

/// Amounts below the meaningful threshold are rounding noise, and are shown as nothing.
fn meaningful(amount: Cents) -> Cents {
    if amount.abs().0 > MINIMUM_MEANINGFUL {
        amount
    } else {
        Cents::ZERO
    }
}

#[component]
pub fn PeopleTab(
    tour: Tour,
    /// Every suggested payment, family ones included: what somebody hands over at settle-up
    /// is not the same number as what they owe, and both are shown.
    transfers: Vec<Transfer>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    // Which people are opened out, and which families have their members showing. Ids
    // rather than indices: the list is rebuilt on every edit, and a position means nothing
    // across two versions of a tour.
    let open: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let kids_open: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let sheet: RwSignal<Option<(Which, Person)>> = RwSignal::new(None);

    let fams = families(&tour);
    let count = tour.persons.len();
    let everyone: Vec<String> = tour
        .persons
        .iter()
        .map(|p| p.id.as_str().to_owned())
        .collect();

    let tour_for_sheet = tour.clone();
    let transfers_for_sheet = transfers.clone();
    let unit_for_sheet = unit.clone();

    view! {
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <button type="button" class="tcn-btn tcn-btn-primary"
                        on:click=move |_| dialog.set(Some(Dialog::Person(PersonDraft::new())))>
                    "+ Add person"
                </button>
                <Show when=move || { count > 1 }>
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            on:click={
                                let everyone = everyone.clone();
                                move |_| {
                                    let all = everyone.clone();
                                    // One button for both directions: anybody open means
                                    // the next press closes.
                                    if open.get().is_empty() {
                                        kids_open.set(all.clone());
                                        open.set(all);
                                    } else {
                                        open.set(Vec::new());
                                    }
                                }
                            }>
                        {move || if open.get().is_empty() { "Expand all" } else { "Collapse all" }}
                    </button>
                </Show>
                <span class="tcn-chip">{count} " people"</span>
                <span class="tcn-chip">"total weight " {tour.total_weight()}</span>
            </div>

            <Show when=move || { count > 0 }>
                <div class="tcn-hint" style="margin: -4px 2px 10px 2px">
                    <b>"Paid"</b>" — what they put in · "<b>"Charged"</b>
                    " — what their part of the spending cost · "<b>"Balance"</b>
                    " — the difference. Tap any of the three for the itemised list."
                </div>
            </Show>
        </div>

        {if fams.is_empty() {
            view! {
                <div class="tcn-section">
                    <div class="tcn-empty">
                        <span class="tcn-empty-icon">"👥"</span>
                        <div class="tcn-empty-title">"No participants yet"</div>
                        <div>"Add everyone who takes part - then you can start recording expenses."</div>
                    </div>
                </div>
            }.into_any()
        } else {
            view! {
                <div class="tcn-section">
                    <div class="tcn-list tcn-people-list">
                        {fams
                            .into_iter()
                            .map(|fam| view! {
                                <FamilyBlock tour=tour.clone() fam=fam transfers=transfers.clone()
                                             open=open kids_open=kids_open sheet=sheet
                                             dialog=dialog delete=delete />
                            })
                            .collect_view()}
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
                    <span>{move || if showing.get() { "⌄" } else { "›" }}</span>
                    <span class="tcn-familybar-names">{names.clone()}</span>
                    <span class="tcn-familybar-note">"paid for by " {head_name.clone()}</span>
                </button>
            </Show>

            <Show when=move || { covers > 0 && showing.get() }>
                <div class="tcn-family-kids">
                    {kids
                        .iter()
                        .map(|kid| view! {
                            <PersonBlock tour=tour.clone() person=kid.clone() covers=0
                                         family_weight=0 transfers=transfers.clone()
                                         open=open sheet=sheet dialog=dialog delete=delete />
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
    let debt = meaningful(
        balances
            .get(&person.id)
            .map(|b| b.debt())
            .unwrap_or_default(),
    );

    // What actually changes hands. For somebody who pays for others it carries their
    // people too, which is a different number from their own debt - hence both being shown.
    // Zero, not the threshold: the headline says whether there is anything at all to
    // do, and a two-cent payment counts as something to do - it is `meaningful` just
    // below that then reports it as "settled".
    let will_pay = meaningful(will_pay(&tour, &transfers, &person.id, Cents::ZERO));

    // Somebody who is paid for hands nothing over themselves; showing them as "settled"
    // while they plainly owe something was confusing, so they show their own debt.
    let shown = if is_child { debt } else { will_pay };
    let split_family = !is_child && (will_pay - debt).abs().0 > MINIMUM_MEANINGFUL;

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
    let words = balance_words(shown);
    let paid_by = person
        .parent
        .as_ref()
        .and_then(|p| tour.person(p))
        .map(|p| p.name.clone());

    let name = person.name.clone();
    let weight = person.weight;
    let colour = avatar_colour(&person.name);
    let letters = initials(&person.name);

    let for_edit = person.clone();
    let for_delete = person.clone();
    let for_spend = person.clone();
    let for_stats = person.clone();
    let toggle_row = toggle.clone();
    let words_row = words.clone();
    let name_row = name.clone();
    let colour_row = colour.clone();
    let letters_row = letters.clone();
    let tour_for_spend = tour.clone();

    view! {
        <div class="tcn-person" class:is-child=move || is_child
             class:is-compact=move || !is_open.get()>
            <Show when=move || !is_open.get()
                  fallback=move || {
                      // The roomy head is the handle: clicking it folds the card back down.
                      // It cannot be a <button> - the figures inside it are buttons
                      // themselves, and a button inside a button is not markup a browser
                      // accepts - so the caret beside it is what a keyboard reaches for.
                      let words = words.clone();
                      let name = name.clone();
                      let colour = colour.clone();
                      let letters = letters.clone();
                      let paid_by = paid_by.clone();
                      let toggle = toggle.clone();
                      let fold = toggle.clone();
                      view! {
                          <div class="tcn-person-head is-handle" on:click=toggle>
                              <span class="tcn-avatar" style=format!("background:{colour}")>
                                  {letters}
                              </span>
                              <div class="tcn-person-id">
                                  <div class="tcn-person-name">{name}</div>
                                  <div class="tcn-person-meta" on:click=|ev| ev.stop_propagation()>
                                      <span>"weight " <b>{weight}</b></span>
                                      {paid_by.map(|n| view! { <span>"paid by " <b>{n}</b></span> })}
                                      {(covers > 0).then(|| view! {
                                          <span>"pays for " <b>{covers}</b></span>
                                          <span title="The weight the family carries between them">
                                              "family weight " <b>{family_weight}</b>
                                          </span>
                                      })}
                                  </div>
                              </div>
                              <div class="tcn-person-owe" on:click=|ev| ev.stop_propagation()>
                                  <span class=format!("tcn-chip {chip_class}")>{words}</span>
                                  {split_family.then(|| view! {
                                      <span class="tcn-person-own"
                                            title="Their own debt, before the people they pay for">
                                          "own: " {balance_words(debt)}
                                      </span>
                                  })}
                              </div>
                              <button type="button" class="tcn-person-fold" title="Collapse"
                                      on:click=move |ev| { ev.stop_propagation(); fold(ev); }>
                                  "⌄"
                              </button>
                          </div>
                      }
                  }>
                <div class="tcn-person-row">
                    <button type="button" class="tcn-person-open"
                            aria-expanded=move || is_open.get().to_string()
                            on:click=toggle_row.clone()>
                        <span class="tcn-avatar tcn-avatar-sm"
                              style=format!("background:{colour_row}")>{letters_row.clone()}</span>
                        <span class="tcn-person-id">
                            <span class="tcn-person-name">{name_row.clone()}</span>
                            // "weight" costs about forty pixels the name wants: a weight is
                            // a multiplier, and that is what the × says. A folded family
                            // shows what the family weighs instead - the payer's own weight
                            // says little while the people it covers are out of sight.
                            {if covers == 0 {
                                view! {
                                    <span class="tcn-person-meta" title=format!("Weight {weight}")>
                                        "×" {weight}
                                    </span>
                                }.into_any()
                            } else {
                                view! {
                                    <span class="tcn-chip tcn-chip-kids"
                                          title=format!("Pays for {covers}, {family_weight} of weight between them")>
                                        "👥" {covers} " ×" {family_weight}
                                    </span>
                                }.into_any()
                            }}
                        </span>
                        <span class=format!("tcn-chip {chip_class}")>{words_row.clone()}</span>
                        <span class="tcn-person-caret">
                            {move || if is_open.get() { "⌄" } else { "›" }}
                        </span>
                    </button>
                </div>
            </Show>

            <Show when=move || is_open.get()>
                <div class="tcn-person-stats">
                    <Stat which=Which::Paid person=for_stats.clone() sheet=sheet
                          amount=balances.get(&for_stats.id).map(|b| b.spent).unwrap_or_default()
                          own=None />
                    <Stat which=Which::Charged person=for_stats.clone() sheet=sheet
                          amount=balances.get(&for_stats.id).map(|b| b.received).unwrap_or_default()
                          own=None />
                    <Stat which=Which::Balance person=for_stats.clone() sheet=sheet
                          amount=shown own=split_family.then_some(debt) />
                </div>
                <div class="tcn-person-actions">
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                            on:click={
                                let tour = tour_for_spend.clone();
                                let who = for_spend.clone();
                                move |_| {
                                    let mut draft = crate::edit::SpendingDraft::new(&tour);
                                    draft.from = who.id.clone();
                                    dialog.set(Some(Dialog::Spending(draft)));
                                }
                            }>
                        "💸 Spend"
                    </button>
                    <button type="button" class="tcn-btn tcn-btn-sm"
                            on:click={
                                let who = for_edit.clone();
                                move |_| dialog.set(Some(Dialog::Person(PersonDraft::of(&who))))
                            }>
                        "Edit"
                    </button>
                    <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                            on:click={
                                let who = for_delete.clone();
                                move |_| delete.run(Removal::Person(who.clone()))
                            }>
                        "Delete"
                    </button>
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
) -> impl IntoView {
    let tint = match which {
        Which::Paid => "is-paid",
        Which::Charged => "",
        Which::Balance if amount.0 > 0 => "is-owing",
        Which::Balance if amount.0 < 0 => "is-owed",
        Which::Balance => "",
    };

    view! {
        <div class="tcn-stat">
            <span class="tcn-stat-label">{which.label()}</span>
            <button type="button" class=format!("tcn-statbtn {tint}")
                    on:click=move |_| sheet.set(Some((which, person.clone())))>
                {(which == Which::Balance).then(|| view! {
                    // The chip on the same card says "gets 5 055" while this cell used to
                    // say "-5 055": one number, two languages, five centimetres apart.
                    <span class="tcn-statbtn-dir">{direction(amount)}</span>
                })}
                {if which == Which::Balance {
                    if amount.is_zero() { String::new() } else { money(amount.abs()) }
                } else {
                    money(amount)
                }}
                {own.map(|d| view! {
                    <span class="tcn-statbtn-note">
                        "own: " {direction(d)} " "
                        {(!d.is_zero()).then(|| money(d.abs()))}
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
        Which::Paid => format!("{} paid {} {}", person.name, money(paid), unit),
        Which::Charged => format!("{} was charged {} {}", person.name, money(charged), unit),
        Which::Balance => {
            // The itemised view ignores dust, which can flip somebody the card calls
            // "settled" into somebody collecting five thousand. Both are the app's.
            let owed = will_pay(&tour, &transfers, &person.id, Cents(MINIMUM_MEANINGFUL));
            format!(
                "{} will {} {} {}",
                person.name,
                if owed.0 >= 0 { "pay" } else { "collect" },
                money(owed.abs()),
                unit
            )
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
                    <button type="button" class="tcn-sheet-x" title="Close"
                            on:click=move |_| close.run(())>"✕"</button>
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
                            on:click=move |_| close.run(())>"Got it"</button>
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
        return view! { <div class="tcn-empty">"Nothing recorded yet."</div> }.into_any();
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
                        format!("{:.1}% of {}", line.amount.0 as f64 * 100.0 / w.0 as f64, money(w))
                    });
                    view! {
                        <div class="tcn-brk-row">
                            <div class="tcn-brk-main">
                                <div class="tcn-brk-title">{line.description}</div>
                                <div class="tcn-brk-sub">
                                    {(!line.category.trim().is_empty()).then(|| view! {
                                        <span class="tcn-chip tcn-chip-primary">{line.category}</span>
                                    })}
                                    {from.map(|n| view! { <span>"from " <b>{n}</b></span> })}
                                </div>
                            </div>
                            <div class="tcn-brk-amt">
                                {money(line.amount)} " " <small>{unit.clone()}</small>
                                {share.map(|s| view! { <div class="tcn-brk-note">{s}</div> })}
                                <div class="tcn-brk-note">"running " {money(running)}</div>
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
    let (paying, rows) = settlement_for(&tour, &transfers, &person.id, Cents(MINIMUM_MEANINGFUL));
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
                {if paying { "Needs to pay" } else { "Will collect" }}
            </div>
            <div class="tcn-brk-summary-value">
                {money(total)} " " <small>{unit.clone()}</small>
            </div>
        </div>

        {if empty {
            view! {
                <div class="tcn-empty" style="margin-top:10px">"Nothing left to settle."</div>
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
                                        {if paying { "to " } else { "from " }}<b>{other}</b>
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
