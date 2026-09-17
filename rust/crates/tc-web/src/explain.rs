//! Tapping a number to find out where it came from.
//!
//! The app's own idea, and the best one in it: every figure on screen is the end of an
//! arithmetic nobody watched happen, and a group settling up at the end of a trip will
//! sooner disbelieve the number than the assumption behind it. So each of them opens into
//! the working - what was added, what was left out, and one sentence saying why.
//!
//! Always on here. In the C# it is a setting, off by default, because it was added late and
//! the author did not want to change what everybody already saw; a port has no such debt.
//!
//! The explanations are ported rather than invented: they say what the code actually does,
//! and rewriting them from a fresh reading of the same code would only add a second opinion
//! to keep in step with the first.

use crate::icon::Icon;
use crate::ui::money;
use leptos::prelude::*;
use tc_core::Cents;

/// One line of an explanation: something, and how much of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
    pub label: String,
    /// A note beside the label - a percentage, a count, who paid.
    pub note: String,
    pub value: String,
    /// Drawn heavier: the figure the rest adds up to.
    pub strong: bool,
}

impl Fact {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Fact {
        Fact {
            label: label.into(),
            note: String::new(),
            value: value.into(),
            strong: false,
        }
    }

    pub fn money(label: impl Into<String>, amount: Cents) -> Fact {
        Fact::new(label, money(amount))
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Fact {
        self.note = note.into();
        self
    }

    pub fn strong(mut self) -> Fact {
        self.strong = true;
        self
    }

    /// A note on the value's side of the row - a share, usually.
    pub fn with_value_note(mut self, note: impl Into<String>) -> Fact {
        self.value = format!("{} {}", self.value, note.into());
        self
    }
}

/// What a number opens into: a title, the figure restated, the working, and the sentence.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Explanation {
    pub title: String,
    /// The value again, in full, at the top - so the sheet says what it is about before it
    /// says anything else.
    pub headline: String,
    pub facts: Vec<Fact>,
    /// Sections of facts under their own small heading, for an explanation with two halves.
    pub sections: Vec<(String, Vec<Fact>)>,
    /// The sentence that says why, in words rather than numbers.
    pub note: String,
}

impl Explanation {
    pub fn new(title: impl Into<String>) -> Explanation {
        Explanation {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn headline(mut self, text: impl Into<String>) -> Explanation {
        self.headline = text.into();
        self
    }

    pub fn facts(mut self, facts: Vec<Fact>) -> Explanation {
        self.facts = facts;
        self
    }

    pub fn section(mut self, title: impl Into<String>, facts: Vec<Fact>) -> Explanation {
        if !facts.is_empty() {
            self.sections.push((title.into(), facts));
        }
        self
    }

    pub fn note(mut self, text: impl Into<String>) -> Explanation {
        self.note = text.into();
        self
    }
}

/// Where the open explanation lives. One at a time, and any number of numbers can put one
/// there - which is why it is a context rather than a signal per figure.
pub type Open = RwSignal<Option<Explanation>>;

/// Wraps a value so it can be tapped.
///
/// The figure itself is the button: nothing is added beside it, because a page where every
/// number carries a little "i" is a page of little "i"s.
#[component]
pub fn Explain(
    /// Built when the number is tapped and not before: an explanation walks the whole tour,
    /// and a list of forty rows would otherwise do that forty times per render.
    what: Callback<(), Explanation>,
    #[prop(optional)] class: &'static str,
    children: Children,
) -> impl IntoView {
    let open = use_context::<Open>();

    view! {
        <button type="button" class=format!("tcn-explain {class}")
                title="Where does this come from?"
                on:click=move |ev| {
                    // The number may sit inside a row that opens on click; explaining it is
                    // not asking for that.
                    ev.stop_propagation();
                    if let Some(open) = open {
                        open.set(Some(what.run(())));
                    }
                }>
            {children()}
        </button>
    }
}

/// What an explanation says: the headline, the facts, the sections and the sentence. Drawn
/// in the sheet a number opens, and in place under an expense that is unfolded.
#[component]
pub fn ExplanationBody(explanation: Explanation) -> impl IntoView {
    let e = explanation;
    let facts = |list: Vec<Fact>| {
        view! {
            <div class="tcn-facts">
                {list
                    .into_iter()
                    .map(|f| {
                        let strong = f.strong;
                        view! {
                            <div>
                                {if strong {
                                    view! { <b>{f.label.clone()}</b> }.into_any()
                                } else {
                                    f.label.clone().into_any()
                                }}
                                {(!f.note.is_empty())
                                    .then(|| view! { <span class="tcn-hint">" " {f.note}</span> })}
                            </div>
                            <div>
                                {if strong {
                                    view! { <b>{f.value.clone()}</b> }.into_any()
                                } else {
                                    f.value.clone().into_any()
                                }}
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        }
    };
    view! {
        {(!e.headline.is_empty()).then(|| view! {
            <div class="tcn-explain-headline">{e.headline.clone()}</div>
        })}
        {(!e.facts.is_empty()).then(|| facts(e.facts.clone()))}
        {e.sections
            .into_iter()
            .map(|(title, list)| {
                view! {
                    <div class="tcn-label" style="margin-top:14px">{title}</div>
                    {facts(list)}
                }
            })
            .collect_view()}
        {(!e.note.is_empty()).then(|| view! {
            <div class="tcn-explain-note">{e.note.clone()}</div>
        })}
    }
}

/// The sheet an explanation opens into. Mounted once, near the top of the page.
#[component]
pub fn ExplainSheet(open: Open) -> impl IntoView {
    view! {
        {move || open.get().map(|e| {
            view! {
                <div class="tcn-modal tcn-sheet" on:click=move |_| open.set(None)>
                    <div class="tcn-modal-card" on:click=|ev| ev.stop_propagation()>
                        <div class="tcn-sheet-head">
                            <div class="tcn-sheet-title">{e.title.clone()}</div>
                            <button type="button" class="tcn-sheet-x" title="Close"
                                    on:click=move |_| open.set(None)>
                                <Icon name="close" />
                            </button>
                        </div>
                        <div class="tcn-sheet-body">
                            <ExplanationBody explanation=e.clone() />
                        </div>
                        <div class="tcn-sheet-foot">
                            <span style="flex:1 1 auto"></span>
                            <button type="button" class="tcn-btn tcn-btn-primary"
                                    on:click=move |_| open.set(None)>"Got it"</button>
                        </div>
                    </div>
                </div>
            }
        })}
    }
}

/// A share as the app writes one: one decimal place, and never "NaN%".
pub fn percent(part: Cents, whole: Cents) -> String {
    if whole.0 == 0 {
        return "0%".to_owned();
    }
    format!("{:.1}%", part.0 as f64 * 100.0 / whole.0 as f64)
}

/// "1 entry" / "7 entries" - the app counts in words where a bare number would read oddly.
pub fn entries(n: usize) -> String {
    if n == 1 {
        "1 entry".to_owned()
    } else {
        format!("{n} entries")
    }
}

// --- what each figure in the app opens into ------------------------------------------------
//
// One function per number, each taking the tour and answering the question that number
// raises. They live together rather than beside the screens that use them because they are
// the same kind of thing - the working behind a figure - and because reading them in a row
// is how you notice two of them disagreeing.

use tc_core::{Kind, Person, PersonId, Spending, Split, Tour, Transfer};

fn unit_of(tour: &Tour) -> String {
    if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    }
}

fn with_unit(amount: Cents, tour: &Tour) -> String {
    let unit = unit_of(tour);
    if unit.is_empty() {
        money(amount)
    } else {
        format!("{} {unit}", money(amount))
    }
}

fn name_of(tour: &Tour, id: &PersonId) -> String {
    tour.person(id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "n/a".to_owned())
}

/// What counts as spending: an entry with a category that is not an uncounted draft. A
/// payback has no category, which is exactly how the arithmetic tells it from an expense.
fn counts_as_spending(s: &Spending) -> bool {
    s.kind.counts(false) && !s.category.trim().is_empty()
}

/// Total spent: what was added up, by category, and what was left out.
pub fn total_spent(tour: &Tour) -> Explanation {
    let counted: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| counts_as_spending(s))
        .collect();
    let total: Cents = counted.iter().map(|s| tour.amount_in_current(s)).sum();
    let real = tour
        .spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned)
        .count();
    let skipped = real - counted.len();

    let mut by_category: Vec<(String, Cents)> = Vec::new();
    for s in &counted {
        let key = s.category.trim().to_owned();
        let amount = tour.amount_in_current(s);
        match by_category.iter_mut().find(|(c, _)| c == &key) {
            Some(g) => g.1 += amount,
            None => by_category.push((key, amount)),
        }
    }
    by_category.sort_by_key(|(_, sum)| -sum.0);

    let mut note = "Adds up every expense that has a category and is not a draft.".to_owned();
    if skipped > 0 {
        note.push_str(&format!(
            " {} left out — paybacks and drafts do not count as spending.",
            if skipped == 1 {
                "1 entry is".to_owned()
            } else {
                format!("{skipped} entries are")
            }
        ));
    }

    Explanation::new("Total spent")
        .headline(with_unit(total, tour))
        .facts(
            by_category
                .into_iter()
                .map(|(cat, sum)| Fact::money(cat, sum).with_note(percent(sum, total)))
                .collect(),
        )
        .note(note)
}

/// People: everybody, their weight, and what a weight does.
pub fn people(tour: &Tour) -> Explanation {
    let total_weight = tour.total_weight();
    let facts = tour
        .persons
        .iter()
        .map(|p| {
            let note = p
                .parent
                .as_ref()
                .map(|id| format!("· paid for by {}", name_of(tour, id)))
                .unwrap_or_default();
            let share = if total_weight == 0 {
                0
            } else {
                p.weight as i64 * 100 / total_weight
            };
            Fact::new(p.name.clone(), format!("{}", p.weight))
                .with_note(note)
                .with_value_note(format!("{share}%"))
        })
        .collect();

    let one_share = if total_weight == 0 {
        0
    } else {
        100 * 100 / total_weight
    };
    Explanation::new("People in this tour")
        .headline(format!("{} people", tour.persons.len()))
        .facts(facts)
        .note(format!(
            "Weights decide how a shared expense is split: total weight here is \
             {total_weight}, so someone on 100 carries {one_share}% of anything shared by \
             everyone."
        ))
}

/// Recorded expenses: how many of what, and the span they cover.
pub fn expenses(tour: &Tour) -> Explanation {
    let real: Vec<&Spending> = tour
        .spendings
        .iter()
        .filter(|s| s.kind != Kind::Planned)
        .collect();
    let counted = real.iter().filter(|s| counts_as_spending(s)).count();
    let drafts = real
        .iter()
        .filter(|s| matches!(s.kind, Kind::Draft { .. }))
        .count();
    let paybacks = real
        .iter()
        .filter(|s| s.description.starts_with("X "))
        .count();
    let family = real
        .iter()
        .filter(|s| s.description.starts_with("Family "))
        .count();

    let mut facts = vec![
        Fact::new("Counted as spending", counted.to_string()),
        Fact::new("Drafts", drafts.to_string()),
        Fact::new("Paybacks (X …)", paybacks.to_string()),
        Fact::new("Inside families", family.to_string()),
    ];

    if !real.is_empty() {
        let mut days: Vec<&str> = real.iter().filter_map(|s| s.day()).collect();
        days.sort();
        if let (Some(first), Some(last)) = (days.first(), days.last()) {
            facts.push(Fact::new("First", (*first).to_owned()));
            facts.push(Fact::new("Last", (*last).to_owned()));
        }
        let sum: Cents = real.iter().map(|s| tour.amount_in_current(s)).sum();
        facts.push(Fact::money(
            "Average entry",
            Cents(sum.0 / real.len() as i64),
        ));
    }

    Explanation::new("Recorded expenses")
        .headline(entries(real.len()))
        .facts(facts)
}

/// Left to settle: which payments make it up, and what is too small to chase.
pub fn left_to_settle(tour: &Tour, between: &[Transfer]) -> Explanation {
    let total: Cents = between
        .iter()
        .map(|t| tour.convert(t.amount, &t.currency))
        .sum();

    if between.is_empty() {
        return Explanation::new("Left to settle")
            .headline(with_unit(Cents::ZERO, tour))
            .note("Everyone is square — no payments are left between the participants.");
    }

    Explanation::new("Left to settle")
        .headline(with_unit(total, tour))
        .facts(
            between
                .iter()
                .map(|t| {
                    Fact::money(
                        format!("{} → {}", name_of(tour, &t.from), name_of(tour, &t.to)),
                        tour.convert(t.amount, &t.currency),
                    )
                })
                .collect(),
        )
        .note(format!(
            "{} would even everyone out. Anything under {} is treated as settled.",
            if between.len() == 1 {
                "1 payment".to_owned()
            } else {
                format!("{} payments", between.len())
            },
            with_unit(crate::settings::threshold(tour), tour),
        ))
}

/// One person's weight: what it is a share of.
pub fn person_weight(tour: &Tour, person: &Person) -> Explanation {
    let total = tour.total_weight();
    let share = if total == 0 {
        0.0
    } else {
        person.weight as f64 * 100.0 / total as f64
    };

    Explanation::new(format!("{}: weight {}", person.name, person.weight))
        .facts(vec![
            Fact::new("Weight", person.weight.to_string()),
            Fact::new("Total weight in the tour", total.to_string()),
            Fact::new("Share of anything shared by all", format!("{share:.1}%")),
        ])
        .note(format!(
            "Out of every 1 000 spent on the whole group, {} lands on {}.",
            money(Cents((share * 10.0).round() as i64)),
            person.name
        ))
}

/// The tour's total weight: everybody's, and what a weight decides.
pub fn total_weight(tour: &Tour) -> Explanation {
    let total = tour.total_weight();
    Explanation::new("Total weight")
        .headline(total.to_string())
        .facts(
            tour.persons
                .iter()
                .map(|p| {
                    let share = if total == 0 {
                        0
                    } else {
                        p.weight as i64 * 100 / total
                    };
                    Fact::new(p.name.clone(), p.weight.to_string())
                        .with_value_note(format!("{share}%"))
                })
                .collect(),
        )
        .note(
            "Every expense shared by everyone is divided in these proportions. A child on 50 \
             costs half of what an adult on 100 does.",
        )
}

/// Where one person's balance comes from, and how much of it is theirs.
pub fn person_balance(
    tour: &Tour,
    person: &Person,
    balances: &tc_core::Balances,
    all_transfers: &[Transfer],
) -> Explanation {
    let spent = balances
        .get(&person.id)
        .map(|b| b.spent)
        .unwrap_or_default();
    let charged = balances
        .get(&person.id)
        .map(|b| b.received)
        .unwrap_or_default();
    let own = balances
        .get(&person.id)
        .map(|b| b.debt())
        .unwrap_or_default();
    let family = tc_core::will_pay(tour, all_transfers, &person.id, Cents::ZERO);

    let kids: Vec<&Person> = tour
        .persons
        .iter()
        .filter(|k| k.parent.as_ref() == Some(&person.id))
        .collect();

    let mut facts = vec![
        Fact::money("Paid for the group", spent),
        Fact::money("Charged for their part", charged),
        Fact::money("Charged − paid", own),
    ];
    for kid in &kids {
        let kid_debt = balances.get(&kid.id).map(|b| b.debt()).unwrap_or_default();
        facts.push(Fact::money(
            format!("{} (paid for by {})", kid.name, person.name),
            kid_debt,
        ));
    }
    facts.push(Fact::money("Hands over at settle-up", family).strong());

    let too_small = crate::settings::threshold(tour);
    let mut note = if own.abs() <= too_small {
        format!(
            "Anything under {} counts as settled, so this shows as square.",
            with_unit(too_small, tour)
        )
    } else if own.0 > 0 {
        format!(
            "{} used more than they paid for, so the difference is owed to the others.",
            person.name
        )
    } else {
        format!(
            "{} paid more than they used, so the group owes them the difference.",
            person.name
        )
    };
    if (family - own).abs() > too_small {
        note.push_str(&format!(
            " {} of that is {}'s own; the rest belongs to the people they pay for, and is \
             settled through them.",
            money(own),
            person.name
        ));
    }
    note.push_str(
        " The three cells below — Paid, Charged and Balance — open the full itemised lists.",
    );

    Explanation::new(format!("{}: where this comes from", person.name))
        .facts(facts)
        .note(note)
}

/// One suggested payment: who is on each side of it, and that nobody chose it.
pub fn transfer(tour: &Tour, t: &Transfer, balances: &tc_core::Balances) -> Explanation {
    let mut facts = vec![
        Fact::new("Pays", name_of(tour, &t.from)),
        Fact::new("Receives", name_of(tour, &t.to)),
    ];
    for id in [&t.from, &t.to] {
        if let Some(p) = tour.person(id) {
            let b = balances.get(id);
            facts.push(Fact::money(
                format!("{} paid in total", p.name),
                b.map(|b| b.spent).unwrap_or_default(),
            ));
            facts.push(Fact::money(
                format!("{} was charged", p.name),
                b.map(|b| b.received).unwrap_or_default(),
            ));
        }
    }

    Explanation::new("Why this payment")
        .headline(with_unit(tour.convert(t.amount, &t.currency), tour))
        .facts(facts)
        .note(
            "Nobody chose this payment — it is one of the transfers the app picked to square \
             everyone up with as few payments as possible. It has not happened yet: “Mark \
             paid” is what records it, after which both balances move towards zero.",
        )
}

/// One expense: who paid, when, and what each person's share of it came to.
pub fn spending(tour: &Tour, s: &Spending) -> Explanation {
    let amount = tour.amount_in_current(s);
    let receivers: Vec<&Person> = match &s.split {
        Split::Everyone => tour.persons.iter().collect(),
        Split::Equally(to) | Split::ByWeight(to) => {
            tour.persons.iter().filter(|p| to.contains(&p.id)).collect()
        }
    };
    let weight: i64 = receivers.iter().map(|p| p.weight as i64).sum();
    let by_weight = !matches!(s.split, Split::Equally(_));

    let mut facts = vec![
        Fact::new("Paid by", name_of(tour, &s.from)),
        Fact::new("When", pretty_stamp(s.when().unwrap_or_default())),
    ];
    if !s.category.trim().is_empty() {
        facts.push(Fact::new("Category", s.category.clone()));
    }
    if tour.currencies.len() > 1 && s.currency.id != tour.currency().id {
        facts.push(Fact::new(
            "Entered as",
            format!("{} {}", money(s.amount), s.currency.name),
        ));
    }
    if let Kind::Draft { counted } = s.kind {
        facts.push(Fact::new(
            "Draft",
            if counted {
                "counted in the balances"
            } else {
                "not counted yet"
            },
        ));
    }

    // Each share from the calculator itself: an equal split is equal whatever the weights,
    // and this used to divide every split by weight - disagreeing, on exactly the expenses
    // somebody opens to check, with the balance it was explaining.
    let mut shares: Vec<&&Person> = receivers.iter().collect();
    shares.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.name.cmp(&b.name)));
    let split = shares
        .into_iter()
        .map(|p| {
            let share = tc_core::share(tour, s, &p.id).unwrap_or(Cents::ZERO);
            let fact = Fact::money(p.name.clone(), share);
            if by_weight {
                fact.with_note(format!("weight {}", p.weight))
            } else {
                fact
            }
        })
        .collect();

    // Who it was not for, by name: on an expense for five of ten, "which five" is the
    // question, and the other five are the quicker way to see the answer.
    let mut left_out: Vec<&Person> = tour
        .persons
        .iter()
        .filter(|p| !receivers.iter().any(|r| r.id == p.id))
        .collect();
    left_out.sort_by(|a, b| a.name.cmp(&b.name));
    let left_out = left_out
        .into_iter()
        .map(|p| Fact::new(p.name.clone(), "—"))
        .collect();

    let note = match (&s.split, receivers.len()) {
        (Split::Everyone, _) => "Shared by everyone in the tour, split by weight.".to_owned(),
        (_, 1) => "Charged to one person only.".to_owned(),
        (Split::Equally(_), n) => format!("Charged to {n} people, in equal shares."),
        (_, n) => format!("Charged to {n} people, split by weight (total weight {weight})."),
    };

    Explanation::new(match crate::ui::as_service_transfer(&s.description) {
        Some((from, to)) => format!("{from} → {to}"),
        None if s.description.trim().is_empty() => "(no description)".to_owned(),
        None => s.description.clone(),
    })
    .headline(with_unit(amount, tour))
    .section(
        if matches!(s.split, Split::Everyone) {
            format!("Split between everyone ({})", receivers.len())
        } else {
            format!("Split between {} of {}", receivers.len(), tour.persons.len())
        },
        split,
    )
    .section("Not in this one", left_out)
    .facts(facts)
    .note(note)
}

/// People who carry the same share of one expense: said once, with their names after it.
///
/// An expense unfolded in the list used to give every person a line of their own - ten
/// lines of "63" for a dinner split equally. The figure is what differs; the names are what
/// shares it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShareGroup {
    pub share: Cents,
    /// Their weight, when the split goes by weight - and so the reason the figures differ.
    pub weight: Option<i32>,
    pub names: Vec<String>,
}

/// Who carries how much of an expense, largest share first, and who is not in it at all.
/// The shares are the calculator's own ([`tc_core::share`]).
pub fn share_groups(tour: &Tour, s: &Spending) -> (Vec<ShareGroup>, Vec<String>) {
    let by_weight = !matches!(s.split, Split::Equally(_));
    let mut groups: Vec<ShareGroup> = Vec::new();
    let mut left_out: Vec<String> = Vec::new();
    for p in &tour.persons {
        let Some(share) = tc_core::share(tour, s, &p.id) else {
            left_out.push(p.name.clone());
            continue;
        };
        let weight = by_weight.then_some(p.weight);
        match groups
            .iter_mut()
            .find(|g| g.share == share && g.weight == weight)
        {
            Some(g) => g.names.push(p.name.clone()),
            None => groups.push(ShareGroup {
                share,
                weight,
                names: vec![p.name.clone()],
            }),
        }
    }
    for g in &mut groups {
        g.names.sort();
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.share));
    left_out.sort();
    (groups, left_out)
}

/// A day in the expense list: what was spent that day, largest first.
pub fn day(tour: &Tour, label: &str, rows: &[Spending]) -> Explanation {
    let mut sorted: Vec<&Spending> = rows.iter().collect();
    sorted.sort_by_key(|s| -tour.amount_in_current(s).0);
    let total: Cents = sorted
        .iter()
        .filter(|s| counts_as_spending(s))
        .map(|s| tour.amount_in_current(s))
        .sum();

    let mut e = Explanation::new(label.to_owned());
    // A day whose entries are all paybacks counts as nothing spent, and "0" at the top of
    // the sheet reads as an error rather than as an answer.
    if !total.is_zero() {
        e = e.headline(with_unit(total, tour));
    }
    e.facts(
        sorted
            .into_iter()
            .map(|s| {
                Fact::money(s.description.clone(), tour.amount_in_current(s))
                    .with_note(name_of(tour, &s.from))
            })
            .collect(),
    )
}

/// The total under a filtered list: only what counts, by category.
pub fn shown_total(tour: &Tour, shown: &[Spending]) -> Explanation {
    let mut by_category: Vec<(String, Cents, usize)> = Vec::new();
    for s in shown.iter().filter(|s| counts_as_spending(s)) {
        let key = s.category.trim().to_owned();
        let amount = tour.amount_in_current(s);
        match by_category.iter_mut().find(|(c, _, _)| c == &key) {
            Some(g) => {
                g.1 += amount;
                g.2 += 1;
            }
            None => by_category.push((key, amount, 1)),
        }
    }
    by_category.sort_by_key(|(_, sum, _)| -sum.0);
    let total: Cents = by_category.iter().map(|(_, sum, _)| *sum).sum();

    Explanation::new("Spending in the shown expenses")
        .headline(with_unit(total, tour))
        .facts(
            by_category
                .into_iter()
                .map(|(cat, sum, n)| Fact::money(cat, sum).with_note(entries(n)))
                .collect(),
        )
        .note(
            "Only entries that count as spending are added up here — a payback or an \
             uncounted draft moves money without spending it. Filters apply, so this is not \
             necessarily the tour total.",
        )
}

/// What was left out of a total, and why leaving it out is right.
pub fn uncounted(tour: &Tour, rows: &[Spending]) -> Explanation {
    let out: Vec<&Spending> = rows
        .iter()
        .filter(|s| s.kind != Kind::Planned && !counts_as_spending(s))
        .collect();

    // A payback: money handed over to clear a debt, which was counted when the expense that
    // created the debt was recorded.
    let settling: Vec<&&Spending> = out
        .iter()
        .filter(|s| s.category.trim().is_empty())
        .collect();
    let drafts: Vec<&&Spending> = out
        .iter()
        .filter(|s| matches!(s.kind, Kind::Draft { counted: false }))
        .collect();

    let line = |s: &Spending| {
        Fact::money(s.description.clone(), tour.amount_in_current(s))
            .with_note(name_of(tour, &s.from))
    };
    let sum =
        |list: &[&&Spending]| -> Cents { list.iter().map(|s| tour.amount_in_current(s)).sum() };

    let mut e = Explanation::new("Not counted as spending");
    if !settling.is_empty() {
        e = e
            .section(
                format!("Settling up · {}", with_unit(sum(&settling), tour)),
                settling.iter().map(|s| line(s)).collect(),
            )
            .note(
                "Money handed from one person to another to clear a debt. It was already \
                 counted when the original expense was recorded — counting it again would \
                 double it.",
            );
    }
    if !drafts.is_empty() {
        e = e.section(
            format!("Drafts · {}", with_unit(sum(&drafts), tour)),
            drafts.iter().map(|s| line(s)).collect(),
        );
    }
    e
}

/// `23.08.2021 15:04` from whatever shape the timestamp was stored in.
pub fn pretty_stamp(when: &str) -> String {
    let day: Vec<&str> = when.get(..10).unwrap_or_default().split('-').collect();
    let time = when.get(11..16).unwrap_or_default();
    match day.as_slice() {
        [y, m, d] if time.is_empty() => format!("{d}.{m}.{y}"),
        [y, m, d] => format!("{d}.{m}.{y} {time}"),
        _ => when.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tour() -> Tour {
        Tour::from_json(include_str!("../../../fixtures/hs3huvy.tour.json")).expect("fixture")
    }

    /// Ten people on one dinner come out as one line per figure, not one per person.
    #[test]
    fn the_same_share_is_said_once() {
        let tour = tour();
        let mut s = tour
            .spendings
            .iter()
            .find(|s| s.kind == Kind::Real)
            .unwrap()
            .clone();
        s.split = Split::Everyone;
        let (groups, left_out) = share_groups(&tour, &s);
        assert!(left_out.is_empty());
        assert_eq!(
            groups.iter().map(|g| g.names.len()).sum::<usize>(),
            tour.persons.len()
        );
        let weights: std::collections::HashSet<i32> =
            tour.persons.iter().map(|p| p.weight).collect();
        assert_eq!(groups.len(), weights.len(), "one line per weight: {groups:?}");
        assert!(groups.windows(2).all(|w| w[0].share >= w[1].share));

        let three: Vec<PersonId> = tour.persons.iter().take(3).map(|p| p.id.clone()).collect();
        s.split = Split::Equally(three);
        let (groups, left_out) = share_groups(&tour, &s);
        assert_eq!(groups.len(), 1, "equal is one figure whatever the weights");
        assert_eq!(groups[0].weight, None);
        assert_eq!(groups[0].names.len(), 3);
        assert_eq!(left_out.len(), tour.persons.len() - 3);
    }
}
