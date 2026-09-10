//! The same tour, one line per thing.
//!
//! For an old phone, a narrow screen, or anybody who would rather see twenty rows than six
//! cards. It is not a different app and not a cut-down one: the arithmetic is the same
//! `tc-core`, the edits go through the same queue, and the dialogs are the roomy interface's
//! own. Only the reading surface is redrawn, so nothing here can disagree with the other one
//! about what a number means.
//!
//! The stylesheet is the app's `mini.css`, unchanged - the same 141 rules the Blazor client
//! uses. That is the second time this port has got a screen's worth of design for nothing,
//! and the reason both are worth reusing rather than reinventing.

use crate::dialogs::{CurrenciesDialog, PersonDialog, SpendingDialog, TourDialog, VersionsDialog};
use crate::edit::{PersonDraft, SpendingDraft};
use crate::icon::Icon;
use crate::queue::Operation;
use crate::sync::Status;
use crate::tour::{Dialog, Removal, Tab};
use crate::ui::{money, name_of};
use leptos::prelude::*;
use tc_core::{
    calculate, settlement_summary, split_family, suggest_settlement, will_pay, Cents, Kind,
    Options, Person, PersonId, Spending, Split, Tour, Transfer,
};

/// Sorting the expense list. The same two the roomy list offers.
#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Date,
    Amount,
}

#[component]
pub fn MiniTour(
    tour: Tour,
    reload: Callback<()>,
    status: RwSignal<Status>,
    apply: Callback<Operation>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
    /// Owned by the page, so an edit does not send the reader back to the first tab - and
    /// so that switching between the two interfaces leaves the reader on the same tab.
    tab: RwSignal<Tab>,
) -> impl IntoView {
    let show_family = RwSignal::new(false);

    let transfers = suggest_settlement(&tour).unwrap_or_default();
    let too_small = crate::settings::threshold(&tour);
    let worth_showing = |t: &&Transfer| tour.convert(t.amount, &t.currency).abs() > too_small;
    let (family, between): (Vec<Transfer>, Vec<Transfer>) = {
        let (f, b) = split_family(&transfers);
        (
            f.into_iter().filter(worth_showing).cloned().collect(),
            b.into_iter().filter(worth_showing).cloned().collect(),
        )
    };
    let mut between = between;
    between.sort_by_key(|t| -tour.convert(t.amount, &t.currency).0);

    let real: Vec<Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false))
        .cloned()
        .collect();
    let total_spent: Cents = real
        .iter()
        .filter(|s| !s.category.trim().is_empty())
        .map(|s| tour.amount_in_current(s))
        .sum();
    let left: Cents = between
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();
    let unit = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let people = tour.persons.len();
    let expenses = real.len();
    let settled = between.is_empty();
    let fin = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);
    let arch = tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED);

    let for_head = tour.clone();
    let for_balance = tour.clone();
    let for_people = tour.clone();
    let for_expenses = tour.clone();
    let for_stats = tour.clone();
    let for_spend = tour.clone();
    let all_transfers = transfers.clone();
    let transfers_for_people = transfers.clone();
    let unit_facts = unit.clone();
    let unit_expenses = unit.clone();
    let unit_stats = unit.clone();

    view! {
        <div class="tcm-head">
            <div class="tcm-row tcm-head-top">
                <span class="tcm-title" title=for_head.name.clone()>{for_head.name.clone()}</span>
                {fin.then(|| view! {
                    <span class="tcm-tag is-amber" title="The tour is being settled up">"settling"</span>
                })}
                {arch.then(|| view! {
                    <span class="tcm-tag" title="Archived, hidden from the default list">"arch"</span>
                })}
                <span class="tcm-spacer"></span>
                <button type="button" class="tcm-btn is-primary" title="Record an expense"
                        on:click={
                            let t = for_spend.clone();
                            move |_| dialog.set(Some(Dialog::Spending(SpendingDraft::new(&t))))
                        }>
                    "+ spend"
                </button>
                <button type="button" class="tcm-btn" title="Reload from the server"
                        on:click=move |_| reload.run(())>
                    <Icon name="refresh" />
                </button>
            </div>

            <div class="tcm-row tcm-head-facts">
                <span title="Everything that counts as spending">
                    <b class="tcm-money">{money(total_spent)}</b>
                    {(!unit_facts.is_empty()).then(|| format!(" {unit_facts}"))}
                </span>
                <span class="tcm-dot">"·"</span>
                <span title="People on this tour">{people} " p"</span>
                <span class="tcm-dot">"·"</span>
                <span title="Recorded expenses">{expenses} " e"</span>
                <span class="tcm-dot">"·"</span>
                {if settled {
                    view! {
                        <span class="tcm-ok" title="No payments are left between the participants">
                            "settled ✓"
                        </span>
                    }.into_any()
                } else {
                    view! {
                        <span title="What the suggested payments come to">
                            "left " <b class="tcm-money is-neg">{money(left)}</b>
                        </span>
                    }.into_any()
                }}
            </div>

            <div class="tcm-row tcm-head-meta">
                <span class="tcm-hint">
                    {move || match status.get() {
                        Status::Synced | Status::Idle => "from the server".to_owned(),
                        Status::Checking => "local copy, asking…".to_owned(),
                        Status::Waiting(0) => "local copy".to_owned(),
                        Status::Waiting(n) => format!("{n} waiting to be sent"),
                        Status::Failed(_) => "server did not answer".to_owned(),
                    }}
                </span>
                <span class="tcm-spacer"></span>
                <button type="button" class="tcm-btn"
                        on:click={
                            let t = for_head.clone();
                            move |_| dialog.set(Some(Dialog::Tour(crate::edit::TourDraft::of(&t))))
                        }>"edit"</button>
                <button type="button" class="tcm-btn"
                        on:click=move |_| dialog.set(Some(Dialog::Currencies))>"cur"</button>
                <button type="button" class="tcm-btn"
                        on:click=move |_| dialog.set(Some(Dialog::Versions))>"vers"</button>
            </div>
        </div>

        <nav class="tcm-tabs" role="tablist" aria-label="Tour sections">
            <MiniTab tab=tab mine=Tab::Balance label="bal".into()
                     count=(!between.is_empty()).then_some(between.len()) />
            <MiniTab tab=tab mine=Tab::People label="people".into() count=Some(people) />
            <MiniTab tab=tab mine=Tab::Expenses label="spend".into() count=Some(expenses) />
            <MiniTab tab=tab mine=Tab::Stats label="stats".into() count=None />
        </nav>

        <div class="tcm-panel" role="tabpanel">
            <Show when=move || tab.get() == Tab::Balance>
                <MiniBalance tour=for_balance.clone() between=between.clone() family=family.clone()
                             all_transfers=all_transfers.clone() show_family=show_family
                             apply=apply />
            </Show>
            <Show when=move || tab.get() == Tab::People>
                <MiniPeople tour=for_people.clone() all_transfers=transfers_for_people.clone()
                            dialog=dialog delete=delete />
            </Show>
            <Show when=move || tab.get() == Tab::Expenses>
                <MiniExpenses tour=for_expenses.clone() real=real.clone()
                              unit=unit_expenses.clone() dialog=dialog delete=delete />
            </Show>
            <Show when=move || tab.get() == Tab::Stats>
                <MiniStats tour=for_stats.clone() unit=unit_stats.clone() />
            </Show>
        </div>
    }
}

#[component]
fn MiniTab(tab: RwSignal<Tab>, mine: Tab, label: String, count: Option<usize>) -> impl IntoView {
    view! {
        <button type="button" role="tab" class="tcm-tab"
                class:is-active=move || tab.get() == mine
                aria-selected=move || (tab.get() == mine).to_string()
                on:click=move |_| tab.set(mine)>
            {label}
            {count.map(|n| format!(" {n}"))}
        </button>
    }
}

// --- balance ------------------------------------------------------------------------------

#[component]
fn MiniBalance(
    tour: Tour,
    between: Vec<Transfer>,
    family: Vec<Transfer>,
    all_transfers: Vec<Transfer>,
    show_family: RwSignal<bool>,
    apply: Callback<Operation>,
) -> impl IntoView {
    let name = {
        let tour = tour.clone();
        move |id: &PersonId| name_of(tour.person(id))
    };
    let balances = settlement_summary(&tour, &all_transfers, crate::settings::threshold(&tour));
    let has_real = tour.spendings.iter().any(|s| s.kind == Kind::Real);
    let empty = between.is_empty();
    let family_count = family.len();

    let name_for_balances = name.clone();
    let tour_for_balances = tour.clone();
    let name_for_family = name.clone();
    let tour_for_family = tour.clone();

    view! {
        {if empty {
            view! {
                <div class="tcm-empty">
                    "Everyone is square."
                    {(!has_real).then_some(" Add the first expense and the split shows up here.")}
                </div>
            }.into_any()
        } else {
            let rows = between
                .iter()
                .map(|t| {
                    let from = name(&t.from);
                    let to = name(&t.to);
                    let amount = tour.convert(t.amount, &t.currency);
                    let recording = t.clone();
                    let question = format!("Record that {from} paid {to} {}?", money(amount));
                    view! {
                        <div class="tcm-row">
                            // Who hands the money over recedes, who ends up with it is the
                            // strong one - the direction is then readable without reading.
                            <span class="tcm-main">
                                <span class="tcm-name is-payer">{from}</span>
                                <span class="tcm-arrow">"→"</span>
                                <span class="tcm-name is-payee">{to}</span>
                            </span>
                            <span class="tcm-money">{money(amount)}</span>
                            <button type="button" class="tcm-act" aria-label="Mark paid"
                                    title="Record that this money has changed hands"
                                    on:click=move |_| {
                                        let agreed = web_sys::window()
                                            .and_then(|w| w.confirm_with_message(&question).ok())
                                            .unwrap_or(false);
                                        if !agreed {
                                            return;
                                        }
                                        let mut draft = crate::edit::PaymentDraft::of(&recording);
                                        draft.id = Some(tc_core::SpendingId::new(crate::edit::new_id()));
                                        apply.run(Operation::RecordPayment(draft));
                                    }>
                                "✓"
                            </button>
                        </div>
                    }
                })
                .collect_view();
            view! {
                <div class="tcm-caption is-band">
                    "who pays whom " <span class="tcm-count">{between.len()}</span>
                    <span class="tcm-spacer"></span>
                    <span class="tcm-hint">"tap ✓ to record"</span>
                </div>
                <div class="tcm-list">{rows}</div>
            }.into_any()
        }}

        {(!balances.is_empty()).then(|| {
            let rows = balances
                .iter()
                .filter_map(|(id, amount)| {
                    tour_for_balances.person(id).map(|p| (p.name.clone(), *amount))
                })
                .map(|(who, amount)| view! {
                    <div class="tcm-row">
                        <span class="tcm-main"><span class="tcm-name">{who}</span></span>
                        <span class="tcm-half is-pos">
                            {(amount.0 < 0).then(|| money(amount.abs()))}
                        </span>
                        <span class="tcm-half is-neg">
                            {(amount.0 > 0).then(|| money(amount))}
                        </span>
                    </div>
                })
                .collect_view();
            let _ = &name_for_balances;
            view! {
                <div class="tcm-caption is-band">
                    "balances"
                    <span class="tcm-spacer"></span>
                    <span class="tcm-hh">"gets"</span><span class="tcm-hh">"owes"</span>
                </div>
                <div class="tcm-list tcm-2side">{rows}</div>
            }
        })}

        {(family_count > 0).then(|| view! {
            <button type="button" class="tcm-fold"
                    on:click=move |_| show_family.update(|f| *f = !*f)>
                {move || if show_family.get() { "▾" } else { "▸" }}
                " inside families " <span class="tcm-count">{family_count}</span>
            </button>
            <Show when=move || show_family.get()>
                <div class="tcm-list">
                    {family
                        .iter()
                        .map(|t| view! {
                            <div class="tcm-row">
                                <span class="tcm-main">
                                    <span class="tcm-name">{name_for_family(&t.from)}</span>
                                    <span class="tcm-arrow">"→"</span>
                                    <span class="tcm-name">{name_for_family(&t.to)}</span>
                                </span>
                                <span class="tcm-money">
                                    {money(tour_for_family.convert(t.amount, &t.currency))}
                                </span>
                            </div>
                        })
                        .collect_view()}
                </div>
            </Show>
        })}
    }
}

// --- people -------------------------------------------------------------------------------

#[component]
fn MiniPeople(
    tour: Tour,
    all_transfers: Vec<Transfer>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let open: RwSignal<Option<String>> = RwSignal::new(None);
    let count = tour.persons.len();
    let searchable = count > 8;
    let total_weight = tour.total_weight();

    // The weight almost everybody shares is not worth a column of ink - only the people who
    // differ from it are worth marking.
    let common = common_weight(&tour);
    let balances = calculate(&tour, Options::default());

    view! {
        <div class="tcm-bar">
            <button type="button" class="tcm-btn"
                    on:click=move |_| dialog.set(Some(Dialog::Person(PersonDraft::new())))>
                "+ person"
            </button>
            <Show when=move || { searchable }>
                <input class="tcm-find" type="text" placeholder="find"
                       prop:value=move || search.get()
                       on:input=move |ev| search.set(event_target_value(&ev)) />
            </Show>
            <span class="tcm-spacer"></span>
            <span class="tcm-hint" title="Every shared expense is split in these proportions">
                "Σw " {total_weight}
            </span>
        </div>

        {if count == 0 {
            view! {
                <div class="tcm-empty">"Nobody here yet — “+ person” adds the first one."</div>
            }.into_any()
        } else {
            view! {
                <div class="tcm-caption is-band">
                    {count} " people"
                    <span class="tcm-spacer"></span>
                    <span class="tcm-hh">"gets"</span><span class="tcm-hh">"owes"</span>
                </div>
                <div class="tcm-list tcm-2side">
                    {move || {
                        let needle = search.get().trim().to_lowercase();
                        let hit = |p: &Person| {
                            needle.is_empty() || p.name.to_lowercase().contains(&needle)
                        };
                        tour.persons
                            .iter()
                            .filter(|p| p.parent.is_none())
                            .flat_map(|head| {
                                let kids: Vec<&Person> = tour
                                    .persons
                                    .iter()
                                    .filter(|k| k.parent.as_ref() == Some(&head.id))
                                    .collect();
                                // A family is shown whole if anybody in it matches.
                                let shown = hit(head) || kids.iter().any(|k| hit(k));
                                let family_weight =
                                    head.weight + kids.iter().map(|k| k.weight).sum::<i32>();
                                let mut rows: Vec<(Person, usize, i32)> = Vec::new();
                                if shown {
                                    rows.push((head.clone(), kids.len(), family_weight));
                                    rows.extend(kids.iter().map(|k| ((*k).clone(), 0, 0)));
                                }
                                rows
                            })
                            .map(|(person, covers, family_weight)| view! {
                                <MiniPerson person=person covers=covers
                                            family_weight=family_weight common=common
                                            tour=tour.clone() balances=balances.clone()
                                            all_transfers=all_transfers.clone() open=open
                                            dialog=dialog delete=delete />
                            })
                            .collect_view()
                    }}
                </div>
            }.into_any()
        }}
    }
}

/// The weight most people are on, which is the one not worth showing.
fn common_weight(tour: &Tour) -> i32 {
    let mut counts: Vec<(i32, usize)> = Vec::new();
    for p in &tour.persons {
        match counts.iter_mut().find(|(w, _)| *w == p.weight) {
            Some((_, n)) => *n += 1,
            None => counts.push((p.weight, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(w, _)| w)
        .unwrap_or(100)
}

#[component]
fn MiniPerson(
    person: Person,
    covers: usize,
    family_weight: i32,
    common: i32,
    tour: Tour,
    balances: tc_core::Balances,
    all_transfers: Vec<Transfer>,
    open: RwSignal<Option<String>>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let id = person.id.as_str().to_owned();
    let is_child = person.parent.is_some();
    let too_small = crate::settings::threshold(&tour);
    let hush = |amount: Cents| {
        if amount.abs() > too_small {
            amount
        } else {
            Cents::ZERO
        }
    };
    let own = hush(
        balances
            .get(&person.id)
            .map(|b| b.debt())
            .unwrap_or_default(),
    );
    let settle = hush(will_pay(&tour, &all_transfers, &person.id, Cents::ZERO));
    // Somebody who is paid for hands nothing over themselves, so their own figure is the
    // honest one to show; everybody else settles for their whole family.
    let shown = if is_child { own } else { settle };

    let paid = balances
        .get(&person.id)
        .map(|b| b.spent)
        .unwrap_or_default();
    let charged = balances
        .get(&person.id)
        .map(|b| b.received)
        .unwrap_or_default();
    let payer = person
        .parent
        .as_ref()
        .and_then(|p| tour.person(p))
        .map(|p| p.name.clone());

    let mine = id.clone();
    let is_open = Memo::new(move |_| open.get().as_deref() == Some(mine.as_str()));
    let toggle_id = id.clone();

    let for_edit = person.clone();
    let for_delete = person.clone();
    let for_spend = person.clone();
    let tour_for_spend = tour.clone();
    let name = person.name.clone();
    let weight = person.weight;
    let shows_family = covers > 0 && (settle - own).abs() > too_small;

    view! {
        <div class="tcm-item" class:is-child=move || is_child class:is-fam=move || is_child
             class:is-famhead=move || { covers > 0 }>
            <div class="tcm-row">
                <button type="button" class="tcm-main tcm-open"
                        aria-expanded=move || is_open.get().to_string()
                        on:click=move |_| {
                            let id = toggle_id.clone();
                            open.update(|o| {
                                *o = if o.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                            });
                        }>
                    {is_child.then(|| view! { <span class="tcm-tree" aria-hidden="true">"└"</span> })}
                    <span class="tcm-name">{name.clone()}</span>
                    {if covers > 0 {
                        view! {
                            <span class="tcm-tag"
                                  title=format!("Pays for {covers}, {family_weight} of weight between them")>
                                "+" {covers} " ×" {family_weight}
                            </span>
                        }.into_any()
                    } else if weight != common {
                        view! {
                            <span class="tcm-weight" title=format!("Weight {weight}")>
                                "×" {weight}
                            </span>
                        }.into_any()
                    } else {
                        ().into_any()
                    }}
                </button>
                {if shown.is_zero() {
                    let why = if is_child {
                        payer.clone().map(|p| format!("Settled through {p}"))
                            .unwrap_or_else(|| "Settled".to_owned())
                    } else {
                        "Settled".to_owned()
                    };
                    view! {
                        <span class="tcm-half"></span>
                        <span class="tcm-half tcm-ok" title=why>"✓"</span>
                    }.into_any()
                } else {
                    view! {
                        <span class="tcm-half is-pos">
                            {(shown.0 < 0).then(|| money(shown.abs()))}
                        </span>
                        <span class="tcm-half is-neg">
                            {(shown.0 > 0).then(|| money(shown))}
                        </span>
                    }.into_any()
                }}
            </div>

            <Show when=move || is_open.get()>
                <div class="tcm-sub">
                    <div class="tcm-facts-line">
                        <span>"paid " <b class="tcm-money">{money(paid)}</b></span>
                        <span class="tcm-dot">"·"</span>
                        <span>"charged " <b class="tcm-money">{money(charged)}</b></span>
                        <span class="tcm-dot">"·"</span>
                        <span>"own " <b class="tcm-money">{money(own)}</b></span>
                        {shows_family.then(|| view! {
                            <span class="tcm-dot">"·"</span>
                            <span title="Their own debt plus everyone they pay for">
                                "family " <b class="tcm-money">{money(settle)}</b>
                            </span>
                        })}
                        <span class="tcm-dot">"·"</span>
                        <span>"weight " <b>{weight}</b></span>
                        {payer.clone().map(|p| view! {
                            <span class="tcm-dot">"·"</span>
                            <span>"paid by " <b>{p}</b></span>
                        })}
                    </div>
                    <div class="tcm-fields">
                        <button type="button" class="tcm-btn is-primary"
                                on:click={
                                    let t = tour_for_spend.clone();
                                    let who = for_spend.clone();
                                    move |_| {
                                        let mut draft = SpendingDraft::new(&t);
                                        draft.from = who.id.clone();
                                        dialog.set(Some(Dialog::Spending(draft)));
                                    }
                                }>
                            "spend"
                        </button>
                        <button type="button" class="tcm-btn"
                                on:click={
                                    let who = for_edit.clone();
                                    move |_| dialog.set(Some(Dialog::Person(PersonDraft::of(&who))))
                                }>
                            "edit"
                        </button>
                        <button type="button" class="tcm-btn is-danger"
                                on:click={
                                    let who = for_delete.clone();
                                    move |_| delete.run(Removal::Person(who.clone()))
                                }>
                            "delete"
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// --- expenses -----------------------------------------------------------------------------

#[component]
fn MiniExpenses(
    tour: Tour,
    real: Vec<Spending>,
    unit: String,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let sort_by = RwSignal::new(Sort::Date);
    let newest_first = RwSignal::new(true);
    let category = RwSignal::new(String::new());
    let open: RwSignal<Option<String>> = RwSignal::new(None);

    let categories = {
        let mut cs: Vec<String> = real
            .iter()
            .map(|s| s.category.trim().to_owned())
            .filter(|c| !c.is_empty())
            .collect();
        cs.sort();
        cs.dedup();
        cs
    };
    let total = real.len();
    let toggle = move |which: Sort| {
        if sort_by.get() == which {
            newest_first.update(|d| *d = !*d);
        } else {
            sort_by.set(which);
            newest_first.set(true);
        }
    };

    view! {
        <div class="tcm-bar">
            <input class="tcm-find" type="text" placeholder="find"
                   prop:value=move || search.get()
                   on:input=move |ev| search.set(event_target_value(&ev)) />
            <button type="button" class="tcm-btn" class:is-on=move || sort_by.get() == Sort::Date
                    on:click=move |_| toggle(Sort::Date)>
                "date"
                {move || if sort_by.get() == Sort::Date {
                    if newest_first.get() { "↓" } else { "↑" }
                } else { "" }}
            </button>
            <button type="button" class="tcm-btn" class:is-on=move || sort_by.get() == Sort::Amount
                    on:click=move |_| toggle(Sort::Amount)>
                "amt"
                {move || if sort_by.get() == Sort::Amount {
                    if newest_first.get() { "↓" } else { "↑" }
                } else { "" }}
            </button>
            {(categories.len() > 1).then(|| view! {
                <select class="tcm-input tcm-input-xs" title="Category"
                        on:change=move |ev| category.set(event_target_value(&ev))>
                    <option value="">"all"</option>
                    {categories
                        .iter()
                        .map(|c| view! { <option value=c.clone()>{c.clone()}</option> })
                        .collect_view()}
                </select>
            })}
        </div>

        {move || {
            if total == 0 {
                return view! {
                    <div class="tcm-empty">"No expenses yet — “+ spend” records the first one."</div>
                }.into_any();
            }
            let needle = search.get().trim().to_lowercase();
            let wanted = category.get();
            let mut shown: Vec<&Spending> = real
                .iter()
                .filter(|s| wanted.is_empty() || s.category == wanted)
                .filter(|s| {
                    needle.is_empty()
                        || s.description.to_lowercase().contains(&needle)
                        || s.category.to_lowercase().contains(&needle)
                        || name_of(tour.person(&s.from)).to_lowercase().contains(&needle)
                })
                .collect();

            if shown.is_empty() {
                return view! { <div class="tcm-empty">"Nothing matches the filter."</div> }
                    .into_any();
            }

            match sort_by.get() {
                Sort::Date => shown.sort_by(|a, b| a.day().cmp(&b.day())),
                Sort::Amount => shown.sort_by_key(|s| tour.amount_in_current(s).0),
            }
            if newest_first.get() {
                shown.reverse();
            }

            let counted: Cents = shown
                .iter()
                .filter(|s| !s.category.trim().is_empty())
                .map(|s| tour.amount_in_current(s))
                .sum();

            let by_day = sort_by.get() == Sort::Date;
            let mut last_day = String::new();
            let rows = shown
                .into_iter()
                .map(|s| {
                    let day = s.day().unwrap_or_default().to_owned();
                    // Sorted by date the day rides in the first row's gutter, which is a
                    // line cheaper than a header and does not look like data. Sorted by
                    // amount there are no day blocks, so every row carries its own.
                    let label = if !by_day || day != last_day {
                        last_day = day.clone();
                        short_day(&day)
                    } else {
                        String::new()
                    };
                    let starts_day = by_day && !label.is_empty();
                    view! {
                        <MiniSpending spending=s.clone() tour=tour.clone() unit=unit.clone()
                                      date_label=label starts_day=starts_day open=open
                                      dialog=dialog delete=delete />
                    }
                })
                .collect_view();

            view! {
                <div class="tcm-caption">
                    {shown_count_label(&counted, total, &unit)}
                </div>
                <div class="tcm-list">{rows}</div>
            }.into_any()
        }}
    }
}

fn shown_count_label(counted: &Cents, total: usize, unit: &str) -> String {
    if unit.is_empty() {
        format!("{total} · {}", money(*counted))
    } else {
        format!("{total} · {} {unit}", money(*counted))
    }
}

/// `dd.MM` - what a day looks like in a gutter two characters wide.
fn short_day(day: &str) -> String {
    let parts: Vec<&str> = day.split('-').collect();
    match parts.as_slice() {
        [_, month, d] => format!("{d}.{month}"),
        _ => day.to_owned(),
    }
}

#[component]
fn MiniSpending(
    spending: Spending,
    tour: Tour,
    unit: String,
    date_label: String,
    starts_day: bool,
    open: RwSignal<Option<String>>,
    dialog: RwSignal<Option<Dialog>>,
    delete: Callback<Removal>,
) -> impl IntoView {
    let id = spending.id.as_str().to_owned();
    let mine = id.clone();
    let is_open = Memo::new(move |_| open.get().as_deref() == Some(mine.as_str()));
    let toggle_id = id.clone();

    let amount = tour.amount_in_current(&spending);
    let who = name_of(tour.person(&spending.from));
    // A payment the app recorded reads as a payment; anything a person typed is theirs.
    let description = match crate::ui::as_service_transfer(&spending.description) {
        Some((from, to)) => format!("{from} → {to}"),
        None if spending.description.trim().is_empty() => "(no description)".to_owned(),
        None => spending.description.clone(),
    };
    let for_whom = match &spending.split {
        Split::Everyone => "everyone".to_owned(),
        Split::Equally(to) | Split::ByWeight(to) => {
            let mut names: Vec<String> = to.iter().map(|id| name_of(tour.person(id))).collect();
            names.sort();
            names.join(", ")
        }
    };
    let category = spending.category.trim().to_owned();
    let entered_as = (tour.currencies.len() > 1 && spending.currency.id != tour.currency().id)
        .then(|| format!("{} {}", money(spending.amount), spending.currency.name));
    let colour = tc_core::extras::str_of(&spending.extras, crate::edit::COLOUR);
    let marked = crate::ui::is_marked(&colour);
    let mark_style = crate::ui::mark_style(&colour);
    let when = spending.when().unwrap_or_default().to_owned();

    let for_edit = spending.clone();
    let for_delete = spending.clone();
    let _ = unit;

    view! {
        <div class="tcm-item" class:is-daystart=move || starts_day
             class:tcm-marked=move || marked style=mark_style>
            <div class="tcm-row">
                <span class="tcm-date" title=when.clone()>{date_label}</span>
                <button type="button" class="tcm-main tcm-open"
                        aria-expanded=move || is_open.get().to_string()
                        on:click=move |_| {
                            let id = toggle_id.clone();
                            open.update(|o| {
                                *o = if o.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                            });
                        }>
                    <span class="tcm-name">{description}</span>
                    // The payer sits in a fixed column so the three columns line up down the
                    // list; a long name is cut, and the title carries it in full.
                    <span class="tcm-who" title=who.clone()>{who.clone()}</span>
                </button>
                <span class="tcm-money">{money(amount)}</span>
            </div>

            <Show when=move || is_open.get()>
                <div class="tcm-sub">
                    <div class="tcm-facts-line">
                        <span>{pretty_when(&when)}</span>
                        <span class="tcm-dot">"·"</span>
                        <span>"for " <b>{for_whom.clone()}</b></span>
                        {(!category.is_empty()).then(|| view! {
                            <span class="tcm-dot">"·"</span>
                            <span>{category.clone()}</span>
                        })}
                        {entered_as.clone().map(|e| view! {
                            <span class="tcm-dot">"·"</span>
                            <span>{e}</span>
                        })}
                    </div>
                    <div class="tcm-fields">
                        <button type="button" class="tcm-btn"
                                on:click={
                                    let s = for_edit.clone();
                                    move |_| dialog.set(Some(Dialog::Spending(SpendingDraft::of(&s))))
                                }>
                            "edit"
                        </button>
                        <button type="button" class="tcm-btn is-danger"
                                on:click={
                                    let s = for_delete.clone();
                                    move |_| delete.run(Removal::Spending(s.clone()))
                                }>
                            "delete"
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

/// `14.08.2021 09:12` out of whatever shape the timestamp was stored in.
fn pretty_when(when: &str) -> String {
    let day: String = when.chars().take(10).collect();
    let time: String = when.chars().skip(11).take(5).collect();
    let pretty = short_day(&day);
    let year: String = day.chars().take(4).collect();
    if time.is_empty() {
        format!("{pretty}.{year}")
    } else {
        format!("{pretty}.{year} {time}")
    }
}

// --- stats --------------------------------------------------------------------------------

#[component]
fn MiniStats(tour: Tour, unit: String) -> impl IntoView {
    let by_category = RwSignal::new(true);

    // Owned, because the closure that draws the bars outlives this function - the view is
    // re-run whenever the by-category switch moves.
    let counted: Vec<Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind.counts(false) && !s.category.trim().is_empty())
        .cloned()
        .collect();
    let total: Cents = counted.iter().map(|s| tour.amount_in_current(s)).sum();
    let total_weight = if tour.total_weight() == 0 {
        100
    } else {
        tour.total_weight()
    };
    // A "full share" person: a payer if anybody is paid for, otherwise the lightest one.
    let one_share = tour
        .persons
        .iter()
        .find(|p| p.parent.is_some())
        .and_then(|kid| kid.parent.as_ref())
        .and_then(|id| tour.person(id))
        .map(|p| p.weight)
        .or_else(|| {
            tour.persons
                .iter()
                .map(|p| p.weight)
                .filter(|w| *w > 0)
                .min()
        })
        .unwrap_or(100) as i64;
    let days = tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION)
        .filter(|d| *d > 0)
        .unwrap_or(1);
    let per_person = Cents(total.0 * one_share / total_weight);

    let empty = counted.is_empty();
    let tour_for_groups = tour.clone();

    view! {
        {if empty {
            view! {
                <div class="tcm-empty">
                    "Nothing to count yet — an expense needs a category to show up here."
                </div>
            }.into_any()
        } else {
            view! {
                <div class="tcm-statline">
                    <div class="tcm-statcell">
                        <b>{money(total)}</b>
                        <span>{if unit.is_empty() { "total".to_owned() } else { format!("total {unit}") }}</span>
                    </div>
                    <div class="tcm-statcell">
                        <b>{money(per_person)}</b><span>"per person"</span>
                    </div>
                    <div class="tcm-statcell">
                        <b>{money(Cents(per_person.0 / days))}</b>
                        <span>{format!("per person / day · {days} d")}</span>
                    </div>
                </div>

                <div class="tcm-bar">
                    <button type="button" class="tcm-btn" class:is-on=move || by_category.get()
                            on:click=move |_| by_category.set(true)>"by category"</button>
                    <button type="button" class="tcm-btn" class:is-on=move || !by_category.get()
                            on:click=move |_| by_category.set(false)>"by payer"</button>
                </div>

                <div class="tcm-list">
                    {move || {
                        let mut groups: Vec<(String, Cents, usize)> = Vec::new();
                        for s in &counted {
                            let key = if by_category.get() {
                                let c = s.category.trim();
                                if c.is_empty() { "—".to_owned() } else { c.to_owned() }
                            } else {
                                name_of(tour_for_groups.person(&s.from))
                            };
                            let amount = tour_for_groups.amount_in_current(s);
                            match groups.iter_mut().find(|(k, _, _)| k == &key) {
                                Some(g) => {
                                    g.1 += amount;
                                    g.2 += 1;
                                }
                                None => groups.push((key, amount, 1)),
                            }
                        }
                        groups.sort_by_key(|(_, sum, _)| -sum.0);
                        let top = groups.first().map(|(_, s, _)| s.0).unwrap_or(1).max(1);

                        groups
                            .into_iter()
                            .map(|(key, sum, n)| {
                                let width = sum.0 * 100 / top;
                                view! {
                                    <div class="tcm-row">
                                        <span class="tcm-main">
                                            <span class="tcm-name">{key}</span>
                                            <span class="tcm-hint">{n} " e"</span>
                                        </span>
                                        <span class="tcm-bar-cell" aria-hidden="true">
                                            <span class="tcm-bar-fill"
                                                  style=format!("width:{width}%")></span>
                                        </span>
                                        <span class="tcm-money">{money(sum)}</span>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            }.into_any()
        }}
    }
}

// --- the dialogs, which are the roomy interface's own -------------------------------------

/// Draws whichever dialog is open. The same components the Full interface uses: a dialog is
/// a form over the same draft, and there is nothing about it that a narrow screen changes.
#[component]
pub fn MiniDialogs(
    tour: Tour,
    dialog: RwSignal<Option<Dialog>>,
    close: Callback<()>,
    apply: Callback<Operation>,
) -> impl IntoView {
    view! {
        {move || {
            let tour = tour.clone();
            dialog.get().map(|d| match d {
                Dialog::Spending(draft) => view! {
                    <SpendingDialog tour=tour draft=draft on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Person(draft) => view! {
                    <PersonDialog tour=tour draft=draft on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Tour(draft) => view! {
                    <TourDialog draft=draft on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Currencies => view! {
                    <CurrenciesDialog tour=tour on_close=close on_apply=apply />
                }.into_any(),
                Dialog::Versions => view! {
                    <VersionsDialog tour=tour on_close=close />
                }.into_any(),
            })
        }}
    }
}

// --- the tour list, one line per tour ------------------------------------------------------

/// The same list, drawn as rows.
///
/// Everything that does not fit on the line - the flags, the clones, the delete - lives
/// behind the row's own "…" and costs nothing until asked for.
#[component]
pub fn MiniList(
    tours: Vec<Tour>,
    search: RwSignal<String>,
    show_archived: RwSignal<bool>,
    adding: RwSignal<bool>,
    new_name: RwSignal<String>,
    new_code: RwSignal<String>,
    new_json: RwSignal<String>,
    busy: RwSignal<bool>,
    create: Callback<()>,
    remove: Callback<Tour>,
    clone_it: Callback<(Tour, bool)>,
    copy_json: Callback<Tour>,
) -> impl IntoView {
    let more: RwSignal<Option<String>> = RwSignal::new(None);
    let total = tours.len();
    // Under this many the list fits on a screen and a find box is clutter.
    let searchable = total > 8;

    view! {
        <div class="tcm-bar">
            <Show when=move || { searchable }>
                <input class="tcm-find" type="text" placeholder="find"
                       prop:value=move || search.get()
                       on:input=move |ev| search.set(event_target_value(&ev)) />
            </Show>
            <button type="button" class="tcm-btn" class:is-on=move || adding.get()
                    on:click=move |_| adding.update(|a| *a = !*a)>
                {move || if adding.get() { "cancel" } else { "+ tour" }}
            </button>
            <label class="tcm-check" title="Show the archived tours too">
                <input type="checkbox" prop:checked=move || show_archived.get()
                       on:change=move |ev| show_archived.set(event_target_checked(&ev)) />
                "arch"
            </label>
            <span class="tcm-spacer"></span>
            <Show when=move || busy.get()>
                <span class="tcm-saving">"saving…"</span>
            </Show>
        </div>

        <Show when=move || adding.get()>
            <div class="tcm-sub">
                <div class="tcm-fields">
                    <input class="tcm-input" type="text" placeholder="tour name"
                           prop:value=move || new_name.get()
                           on:input=move |ev| new_name.set(event_target_value(&ev)) />
                    <input class="tcm-input tcm-input-xs" type="text" placeholder="code"
                           prop:value=move || new_code.get()
                           on:input=move |ev| new_code.set(event_target_value(&ev)) />
                    <button type="button" class="tcm-btn is-primary"
                            prop:disabled=move || busy.get()
                            on:click=move |_| create.run(())>"create"</button>
                </div>
                <textarea class="tcm-input" rows="2" placeholder="tour JSON (optional)"
                          prop:value=move || new_json.get()
                          on:input=move |ev| new_json.set(event_target_value(&ev))></textarea>
            </div>
        </Show>

        {move || {
            let needle = search.get().trim().to_lowercase();
            let shown: Vec<Tour> = tours
                .iter()
                .filter(|t| {
                    show_archived.get()
                        || !tc_core::extras::bool_of(&t.extras, tc_core::extras::ARCHIVED)
                })
                .filter(|t| {
                    needle.is_empty()
                        || t.name.to_lowercase().contains(&needle)
                        || t.persons.iter().any(|p| p.name.to_lowercase().contains(&needle))
                })
                .cloned()
                .collect();

            if shown.is_empty() {
                return view! {
                    <div class="tcm-empty">
                        {if needle.is_empty() {
                            "No tours under this code yet — “+ tour” makes the first one."
                        } else {
                            "Nothing matches."
                        }}
                    </div>
                }.into_any();
            }

            view! {
                <div class="tcm-list">
                    {shown
                        .into_iter()
                        .map(|tour| view! {
                            <MiniTourRow tour=tour more=more remove=remove
                                         clone_it=clone_it copy_json=copy_json />
                        })
                        .collect_view()}
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn MiniTourRow(
    tour: Tour,
    more: RwSignal<Option<String>>,
    remove: Callback<Tour>,
    clone_it: Callback<(Tour, bool)>,
    copy_json: Callback<Tour>,
) -> impl IntoView {
    let id = tour.id.as_str().to_owned();
    let mine = id.clone();
    let is_open = Memo::new(move |_| more.get().as_deref() == Some(mine.as_str()));
    let toggle_id = id.clone();

    let archived = tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED);
    let finalizing = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);
    let days = tc_core::extras::int_of(&tour.extras, tc_core::extras::DURATION).unwrap_or(0);
    let people = tour.persons.len();
    let names: Vec<String> = tour.persons.iter().map(|p| p.name.clone()).collect();
    let href = format!("/tour/{id}");

    let for_clone = tour.clone();
    let for_clone_bare = tour.clone();
    let for_json = tour.clone();
    let for_delete = tour.clone();

    view! {
        <div class="tcm-item">
            <div class="tcm-row">
                <a class="tcm-main tcm-name" href=href>
                    {tour.name.clone()}
                    {finalizing.then(|| view! { <span class="tcm-tag is-amber">"settling"</span> })}
                    {archived.then(|| view! { <span class="tcm-tag">"arch"</span> })}
                </a>
                <span class="tcm-facts" title=names.join(", ")>
                    <span>{people} "p"</span>
                    {(days > 0).then(|| view! { <span>{days} "d"</span> })}
                </span>
                <button type="button" class="tcm-more" class:is-on=move || is_open.get()
                        title="More about this tour" aria-label="More about this tour"
                        aria-expanded=move || is_open.get().to_string()
                        on:click=move |_| {
                            let id = toggle_id.clone();
                            more.update(|m| {
                                *m = if m.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                            });
                        }>
                    {move || if is_open.get() { "×" } else { "…" }}
                </button>
            </div>

            <Show when=move || is_open.get()>
                <div class="tcm-sub">
                    <div class="tcm-fields">
                        <button type="button" class="tcm-btn"
                                on:click={
                                    let t = for_clone.clone();
                                    move |_| clone_it.run((t.clone(), false))
                                }>"clone"</button>
                        <button type="button" class="tcm-btn" title="Clone without the expenses"
                                on:click={
                                    let t = for_clone_bare.clone();
                                    move |_| clone_it.run((t.clone(), true))
                                }>"clone∅"</button>
                        <button type="button" class="tcm-btn"
                                on:click={
                                    let t = for_json.clone();
                                    move |_| copy_json.run(t.clone())
                                }>"json"</button>
                        <button type="button" class="tcm-btn is-danger"
                                on:click={
                                    let t = for_delete.clone();
                                    move |_| remove.run(t.clone())
                                }>"delete"</button>
                    </div>
                </div>
            </Show>
        </div>
    }
}
