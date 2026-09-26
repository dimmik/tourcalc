//! The two dialogs: an expense and a person.
//!
//! This is where a Rust UI stops looking like a rendering exercise. A form is a pile of
//! little pieces of state that several closures read and write - a change handler, a submit
//! handler, the view itself - and in a language without a garbage collector that question
//! ("who owns the draft?") has to be answered rather than assumed.
//!
//! The answer here is Leptos's: state lives in signals, which are cheap handles to a store
//! the framework owns. A signal is `Copy`, so a closure can take one by `move` without
//! taking anything away from anybody else. That is what keeps `Rc<RefCell<..>>` - and the
//! "already mutably borrowed" panics that come with it - out of this file entirely.

use crate::i18n::t;
use crate::api;
use crate::edit::{self, PersonDraft, SpendingDraft};
use crate::edit::{CurrencyDraft, TourDraft};
use crate::icon::Icon;
use crate::queue::Operation;
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::{Cents, PersonId, Tour};

/// Wraps a dialog in the app's modal shell.
#[component]
fn Modal(
    title: String,
    on_close: Callback<()>,
    children: Children,
    #[prop(optional)] footer: Option<ViewFn>,
) -> impl IntoView {
    // Somebody else's change, said inside the form it is waiting for: a line over the
    // screen would sit on top of this form's own buttons.
    let others = use_context::<crate::others::Others>();

    // Escape closes it, wherever the cursor is. A dialog with a text box in it swallows
    // every key that lands in the box, so the listener goes on the document and is taken
    // off with the dialog - the same thing the × and the mask do, and the thing a keyboard
    // expects of any window that covers the screen.
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        use wasm_bindgen::JsCast;
        let escape = leptos::__reexports::send_wrapper::SendWrapper::new(
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                move |ev: web_sys::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        on_close.run(());
                    }
                },
            ),
        );
        let _ = document
            .add_event_listener_with_callback("keydown", escape.as_ref().unchecked_ref());
        let document = leptos::__reexports::send_wrapper::SendWrapper::new(document);
        on_cleanup(move || {
            let _ = document.remove_event_listener_with_callback(
                "keydown",
                escape.as_ref().unchecked_ref(),
            );
        });
    }

    view! {
        <div class="tcn-modal"
             // Closing on *click* and not on mousedown: releasing the button over the mask
             // after a drag that started inside the dialog is not "click outside".
             on:click=move |_| on_close.run(())>
            <div class="tcn-modal-card" on:click=|ev| ev.stop_propagation()>
                <div class="tcn-modal-head">
                    <div class="tcn-modal-title">{title}</div>
                    <button type="button" class="tcn-modal-x" on:click=move |_| on_close.run(())>
                        <Icon name="close" />
                    </button>
                </div>
                {others.map(|o| view! { <crate::others::WaitingLine others=o /> })}
                <div class="tcn-modal-body">{children()}</div>
                {footer.map(|f| view! { <div class="tcn-modal-foot">{f.run()}</div> })}
            </div>
        </div>
    }
}

/// Adding or changing an expense.
#[component]
pub fn SpendingDialog(
    tour: Tour,
    draft: SpendingDraft,
    /// Whether this draft is what somebody was typing when they last left the form, rather
    /// than a fresh one. The form says so, and offers to start blank.
    #[prop(optional)]
    carried_over: bool,
    on_close: Callback<()>,
    /// Where the finished edit goes. The dialog does not save: it says what was meant, and
    /// the page writes that down and gets it to the server when it can.
    on_apply: Callback<Operation>,
) -> impl IntoView {
    // In the app's order: a family stays together, and children sit under whoever pays for
    // them.
    let people_paid = edit::sorted_people(&tour);
    let people_split = people_paid.clone();
    let headcount = tour.persons.len();
    let currencies = tour.currencies.clone();
    let editing = draft.id.is_some();
    // The categories this tour already uses, plus the one the app always offers.
    let known_categories = {
        let mut list = edit::categories(&tour);
        if !list.iter().any(|c| c == "Common") {
            list.push("Common".to_owned());
        }
        list
    };

    // One signal per field. Each is `Copy`, so the handlers below can each take their own
    // without any of them owning the form.
    let description = RwSignal::new(draft.description.clone());
    let category = RwSignal::new(draft.category.clone());
    // Whether amounts in a currency are hundredths - read and written as "3,50".
    let tour_here = StoredValue::new(tour.clone());
    let cents_in = move |currency: &str| {
        tour_here.with_value(|t| t.counts_cents(&tc_core::CurrencyId::new(currency)))
    };
    let cents_preview = cents_in;
    let currency_name = move |currency: &str| {
        tour_here.with_value(|t| {
            t.currencies
                .iter()
                .find(|c| c.id.as_str() == currency)
                .unwrap_or_else(|| t.currency())
                .name
                .clone()
        })
    };
    let amount = RwSignal::new(if draft.amount.0 == 0 {
        String::new()
    } else {
        // As it would be typed: the decimal part, no thousands gaps.
        crate::ui::amount(draft.amount, cents_in(&draft.currency_id)).replace('\u{202f}', "")
    });
    let from = RwSignal::new(draft.from.as_str().to_owned());
    let everyone = RwSignal::new(draft.everyone);
    let by_weight = RwSignal::new(draft.by_weight);
    let to = RwSignal::new(draft.to.clone());
    let date = RwSignal::new(if draft.date.is_empty() {
        edit::today()
    } else {
        draft.date.clone()
    });
    let colour = RwSignal::new(draft.colour.clone());
    let currency = RwSignal::new(draft.currency_id.clone());
    let errors: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    // A new expense opens in the category last used, and says so: it is a guess, and one
    // that is wrong often enough that it has to look different from a choice.
    let guessed = RwSignal::new(!editing && !draft.category.trim().is_empty());
    let more = RwSignal::new(false);
    let adding = RwSignal::new(false);
    let fresh_category = RwSignal::new(String::new());
    // Opening the box is asking to type in it. The effect re-runs when the input is
    // mounted, not only when the flag turns: `NodeRef` is itself a signal, and at the
    // moment "+ new" is pressed the element does not exist yet.
    let new_category_box = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if adding.get() {
            if let Some(el) = new_category_box.get() {
                let _ = el.focus();
            }
        }
    });
    let add_category = move |()| {
        let name = fresh_category.get().trim().to_owned();
        if name.is_empty() {
            return;
        }
        category.set(name);
        guessed.set(false);
        fresh_category.set(String::new());
        adding.set(false);
    };

    let base = draft.clone();
    // The form as it stands, whether it is being saved or left behind. Cloned rather than
    // shared: it holds nothing but signals, which are `Copy`.
    let current = std::sync::Arc::new(move || {
        let mut d = base.clone();
        d.description = description.get_untracked();
        d.category = category.get_untracked();
        d.amount = tc_core::units::parse_amount(
            &amount.get_untracked(),
            cents_in(&currency.get_untracked()),
        )
        .unwrap_or(Cents::ZERO);
        d.from = PersonId::new(from.get_untracked());
        d.everyone = everyone.get_untracked();
        d.by_weight = by_weight.get_untracked();
        d.to = to.get_untracked();
        d.date = date.get_untracked();
        d.colour = colour.get_untracked();
        d.currency_id = currency.get_untracked();
        d
    });

    // Left without saving, what was typed stays on the device - see `crate::drafts`. Only
    // for a new expense: an edit that was abandoned is the expense as it already is.
    let tour_id = StoredValue::new(tour.id.as_str().to_owned());
    let blank = StoredValue::new(SpendingDraft::new(&tour));
    let carried = RwSignal::new(carried_over);
    let close = Callback::new({
        let current = current.clone();
        move |()| {
            if !editing {
                crate::drafts::keep_spending(&tour_id.get_value(), &current());
            }
            on_close.run(());
        }
    });
    // Starting blank: the form as a new expense opens, and the kept draft forgotten.
    let start_blank = move |_| {
        let fresh = blank.get_value();
        description.set(fresh.description.clone());
        category.set(fresh.category.clone());
        amount.set(String::new());
        from.set(fresh.from.as_str().to_owned());
        everyone.set(fresh.everyone);
        by_weight.set(fresh.by_weight);
        to.set(fresh.to.clone());
        date.set(edit::today());
        colour.set(fresh.colour.clone());
        currency.set(fresh.currency_id.clone());
        errors.set(Vec::new());
        crate::drafts::forget_spending(&tour_id.get_value());
        carried.set(false);
    };

    let submit = move |_| {
        let mut d = current();

        // What the amount box says, read the way the currency counts: an amount that cannot
        // be read is not "0", it is a mistake worth naming.
        let typed = tc_core::units::parse_amount(&amount.get_untracked(), cents_in(&currency.get_untracked()));
        let mut wrong: Vec<String> = d.problems().into_iter().map(String::from).collect();
        use tc_core::units::AmountError;
        let unreadable = match typed {
            Err(AmountError::NotANumber) => Some(t().checks.amount_not_a_number.to_owned()),
            Err(AmountError::TooManyDecimals) => Some(t().checks.amount_too_many_decimals.to_owned()),
            Err(AmountError::NoCentsHere) => Some((t().checks.amount_no_cents)(&currency_name(&currency.get_untracked()))),
            _ => None,
        };
        if let Some(why) = unreadable {
            // Instead of "should not be 0", which is what an unreadable amount came to.
            wrong.retain(|w| w != t().checks.amount_zero);
            wrong.insert(0, why);
        }
        if !wrong.is_empty() {
            errors.set(wrong);
            return;
        }
        // A new spending gets its id here, where the edit is decided - see the note on
        // `SpendingDraft::id`.
        if d.id.is_none() {
            d.id = Some(tc_core::SpendingId::new(edit::new_id()));
            d.editing = false;
        }
        crate::drafts::forget_spending(&tour_id.get_value());
        on_apply.run(Operation::PutSpending(d));
    };

    let title = if editing {
        t().dialogs.edit_expense
    } else {
        t().dialogs.new_expense
    };
    let footer = {
        let submit = submit.clone();
        ViewFn::from(move || {
            let submit = submit.clone();
            view! {
                // Beside the button that produced them, not at the top of a body that may be
                // scrolled somewhere else entirely.
                <Show when=move || !errors.get().is_empty()>
                    <div class="tcn-errors" role="alert" style="flex:1 1 100%; margin:0 0 8px">
                        {move || errors.get()
                            .into_iter()
                            .map(|e| view! { <div>{e}</div> })
                            .collect_view()}
                    </div>
                </Show>
                <span style="flex:1 1 auto"></span>
                <button type="button" class="tcn-btn" on:click=move |_| close.run(())>{t().dialogs.cancel}</button>
                <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit.clone()>
                    {t().dialogs.save}
                </button>
            }
        })
    };

    view! {
        <Modal title=title.to_owned() on_close=close footer=footer>
            // Picked up from the last time this form was open and left.
            <Show when=move || carried.get()>
                <div class="tcn-chip tcn-chip-amber tcw-wraps tcw-carried">
                    {t().dialogs.carried}
                    <button type="button" class="tcn-btn tcn-btn-sm" on:click=start_blank>
                        {t().dialogs.start_blank}
                    </button>
                </div>
            </Show>
            <div class="tcn-amount">
                <div class="tcn-label">{t().dialogs.amount}</div>
                <div class="tcn-amount-row">
                    <input class="tcn-amount-input" type="text" inputmode="decimal" placeholder="0"
                           prop:value=move || amount.get()
                           on:input=move |ev| amount.set(event_target_value(&ev)) />
                    // Which currency this one is in. On a tour with one it is a label, not a
                    // question - and without it an expense paid in marks went into the books
                    // as dinars, which no arithmetic downstream can undo.
                    {if currencies.len() > 1 {
                        view! {
                            <select class="tcn-amount-curr"
                                    on:change=move |ev| currency.set(event_target_value(&ev))>
                                {currencies
                                    .iter()
                                    .map(|c| {
                                        let id = c.id.as_str().to_owned();
                                        let mine = id.clone();
                                        view! {
                                            <option value=id
                                                    selected=move || currency.get() == mine>
                                                {c.name.clone()}
                                            </option>
                                        }
                                    })
                                    .collect_view()}
                            </select>
                        }.into_any()
                    } else {
                        view! {
                            <span class="tcn-amount-curr is-static">
                                {currencies.first().map(|c| c.name.clone()).unwrap_or_default()}
                            </span>
                        }.into_any()
                    }}
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.what_for}</div>
                <input class="tcn-input" type="text" placeholder=t().dialogs.what_for_example
                       prop:value=move || description.get()
                       on:input=move |ev| description.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.paid_by}</div>
                <div class="tcn-pchips">
                    {people_paid
                        .iter()
                        .map(|p| {
                            let id = p.id.as_str().to_owned();
                            let mine = id.clone();
                            let child = p.parent.is_some();
                            view! {
                                // Somebody paid for by another - a child, usually - is the
                                // quiet one on both rows: it is rare for them to be the
                                // payer, and on a tour of ten with four of them, all
                                // fourteen looking alike is what makes the row hard to read.
                                // Quiet, not out of reach: it does happen, and the chip
                                // works like any other.
                                <button type="button" class="tcn-pchip"
                                        class:is-child=child
                                        class:is-on=move || from.get() == mine
                                        on:click=move |_| from.set(id.clone())>
                                    <crate::tour::Avatar name=p.name.clone() />
                                    <span class="tcn-pchip-name">
                                        {if child { "⤷ " } else { "" }}
                                        {p.name.clone()}
                                    </span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label tcn-label-row">
                    <span>{t().dialogs.split_between}</span>
                    <label class="tcn-switchline">
                        <input type="checkbox" prop:checked=move || everyone.get()
                               on:change=move |ev| everyone.set(event_target_checked(&ev)) />
                        {t().dialogs.everyone}
                    </label>
                </div>
                <Show when=move || everyone.get()>
                    <div class="tcn-hint">
                        {(t().dialogs.shared_by_all)(headcount)}
                    </div>
                </Show>
                <Show when=move || !everyone.get()>
                    <div class="tcn-pchips">
                        {people_split
                            .iter()
                            .map(|p| {
                                let id = p.id.clone();
                                let mine = id.clone();
                                let child = p.parent.is_some();
                                view! {
                                    <button type="button" class="tcn-pchip"
                                            class:is-child=child
                                            class:is-on=move || to.get().contains(&mine)
                                            on:click={
                                                let id = id.clone();
                                                move |_| to.update(|list| {
                                                    if let Some(at) =
                                                        list.iter().position(|x| x == &id)
                                                    {
                                                        list.remove(at);
                                                    } else {
                                                        list.push(id.clone());
                                                    }
                                                })
                                            }>
                                        <crate::tour::Avatar name=p.name.clone() />
                                        <span class="tcn-pchip-name">
                                            {if child { "⤷ " } else { "" }}
                                            {p.name.clone()}
                                        </span>
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                    <div class="tcn-hint">
                        {move || if to.get().is_empty() {
                            view! { <span>{t().dialogs.pick_who}</span> }.into_any()
                        } else {
                            view! {
                                <span>{(t().dialogs.selected)(to.get().len())}</span>
                                <button type="button" class="tcn-linkbtn"
                                        on:click=move |_| to.set(Vec::new())>
                                    {t().dialogs.clear}
                                </button>
                            }.into_any()
                        }}
                    </div>
                    <div class="tcn-hint">
                        {move || if by_weight.get() {
                            t().dialogs.by_weight_note
                        } else {
                            t().dialogs.equally_note
                        }}
                    </div>
                </Show>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.category}</div>
                <div class="tcn-chips">
                    {move || {
                        let mut names = known_categories.clone();
                        // Whatever this expense is already in belongs on the list, even if
                        // nothing else in the tour uses it.
                        let now = category.get();
                        if !now.trim().is_empty() && !names.iter().any(|c| *c == now) {
                            names.push(now);
                        }
                        names
                            .into_iter()
                            .map(|name| {
                                let mine = name.clone();
                                let guess = name.clone();
                                let picked = name.clone();
                                view! {
                                    <span class="tcn-chip tcn-filter-chip"
                                          class:is-on=move || {
                                              category.get() == mine && !guessed.get()
                                          }
                                          class:is-guess=move || {
                                              category.get() == guess && guessed.get()
                                          }
                                          on:click=move |_| {
                                              guessed.set(false);
                                              category.update(|c| {
                                                  if *c == picked {
                                                      c.clear()
                                                  } else {
                                                      *c = picked.clone()
                                                  }
                                              });
                                          }>
                                        {name}
                                    </span>
                                }
                            })
                            .collect_view()
                    }}
                    <span class="tcn-chip tcn-filter-chip"
                          on:click=move |_| adding.update(|a| *a = !*a)>
                        {t().dialogs.new_category}
                    </span>
                </div>
                <Show when=move || guessed.get()>
                    <div class="tcn-hint" style="margin-top:4px">
                        {t().dialogs.last_used}
                    </div>
                </Show>
                <Show when=move || adding.get()>
                    <div class="tcn-row" style="margin-top:8px">
                        <input class="tcn-input" style="flex:1 1 140px" type="text"
                               node_ref=new_category_box
                               placeholder=t().dialogs.category_name
                               prop:value=move || fresh_category.get()
                               on:input=move |ev| fresh_category.set(event_target_value(&ev))
                               on:keyup=move |ev: web_sys::KeyboardEvent| {
                                   if ev.key() == "Enter" {
                                       add_category(());
                                   }
                               } />
                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                                on:click=move |_| add_category(())>
                            {t().dialogs.add}
                        </button>
                    </div>
                </Show>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.date}</div>
                // A date input rather than free text: every browser that runs this has one,
                // and it spells the day out in whatever order the reader's locale uses while
                // handing back the same YYYY-MM-DD the data is stored in.
                <input class="tcn-input tcn-input-date" type="date"
                       prop:value=move || date.get()
                       on:input=move |ev| date.set(event_target_value(&ev)) />
                <div class="tcn-hint">
                    {t().dialogs.date_note}
                </div>
            </div>

            // Everything that is usually left alone, out of the way but one tap off.
            <button type="button" class="tcn-btn tcn-btn-ghost tcn-btn-block"
                    on:click=move |_| more.update(|m| *m = !*m)>
                {move || if more.get() {
                    view! { <crate::icon::Icon name="chevron-down" /> }
                } else {
                    view! { <crate::icon::Icon name="chevron-right" /> }
                }}
                {t().dialogs.more_options}
            </button>

            <Show when=move || more.get()>
                <div class="tcn-field" style="margin-top:8px">
                    <div class="tcn-row" style="flex-wrap:wrap; gap:6px 8px">
                        <span class="tcn-label" style="margin:0">{t().dialogs.split_chosen}</span>
                        <span class="tcw-splitway" role="radiogroup"
                              aria-label=t().dialogs.split_how>
                            <button type="button" role="radio" class="tcw-splitway-opt"
                                    class:is-on=move || by_weight.get()
                                    aria-checked=move || by_weight.get().to_string()
                                    on:click=move |_| by_weight.set(true)>
                                {t().dialogs.by_weight}
                            </button>
                            <button type="button" role="radio" class="tcw-splitway-opt"
                                    class:is-on=move || !by_weight.get()
                                    aria-checked=move || (!by_weight.get()).to_string()
                                    on:click=move |_| by_weight.set(false)>
                                {t().dialogs.equally}
                            </button>
                        </span>
                    </div>
                    <div class="tcn-hint" style="margin-top:2px">
                        {move || match (everyone.get(), by_weight.get()) {
                            (true, _) => t().dialogs.split_everyone_note,
                            (false, true) => t().dialogs.split_weight_note,
                            (false, false) => t().dialogs.split_equal_note,
                        }}
                    </div>
                </div>
                <div class="tcn-field" style="margin-top:8px">
                    <div class="tcn-row">
                        <span class="tcn-label" style="margin:0">{t().dialogs.colour}</span>
                        <input type="color" class="tcn-colour"
                               prop:value=move || {
                                   let c = colour.get();
                                   if c.is_empty() { "#ffd54f".to_owned() } else { c }
                               }
                               on:input=move |ev| colour.set(event_target_value(&ev)) />
                        <Show when=move || crate::ui::is_marked(&colour.get())>
                            <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-ghost"
                                    on:click=move |_| colour.set(String::new())>
                                {t().dialogs.reset}
                            </button>
                        </Show>
                    </div>
                    <div class="tcn-hint" style="margin-top:2px">
                        {t().dialogs.colour_note}
                    </div>
                    // What you picked, as the list will show it: a colour is chosen for how
                    // it looks there, not for how it looks in a colour picker.
                    <div class="tcn-settle" style=move || format!(
                        "margin-top:8px;{}", crate::ui::mark_style(&colour.get()))>
                        <div class="tcn-settle-flow">
                            <span class="tcn-settle-who">
                                {move || {
                                    let what = description.get();
                                    if what.trim().is_empty() {
                                        t().dialogs.this_expense.to_owned()
                                    } else {
                                        what
                                    }
                                }}
                            </span>
                        </div>
                        <div class="tcn-settle-amount">
                            {move || {
                                let cents = cents_preview(&currency.get());
                                crate::ui::amount(
                                    tc_core::units::parse_amount(&amount.get(), cents).unwrap_or(Cents::ZERO),
                                    cents,
                                )
                            }}
                        </div>
                    </div>
                </div>
            </Show>

        </Modal>
    }
}

/// Adding or changing a person.
#[component]
pub fn PersonDialog(
    tour: Tour,
    draft: PersonDraft,
    /// As in [`SpendingDialog`]: this is what was being typed when the form was last left.
    #[prop(optional)]
    carried_over: bool,
    on_close: Callback<()>,
    on_apply: Callback<Operation>,
) -> impl IntoView {
    let editing = draft.id.is_some();
    let myself = draft.id.clone();

    let name = RwSignal::new(draft.name.clone());
    let weight = RwSignal::new(draft.weight.to_string());
    let parent = RwSignal::new(
        draft
            .parent
            .as_ref()
            .map(|p| p.as_str().to_owned())
            .unwrap_or_default(),
    );
    let error = RwSignal::new(String::new());

    // Somebody cannot pay for themselves, and a payer who is paid for by another would
    // make a chain the settlement deliberately does not follow.
    let candidates: Vec<_> = edit::sorted_people(&tour)
        .into_iter()
        .filter(|p| Some(&p.id) != myself.as_ref())
        .filter(|p| p.parent.is_none())
        .collect();

    // What a weight is actually for, in the four answers anybody ever gives. The app offers
    // 100/75/50/25; 75 is nobody, and the three below a full share are the ones a tour
    // really has - a teenager, a child, a small child. The box underneath still takes any
    // number, so nothing is lost by not offering it as a chip.
    let presets = [100, 50, 35, 25].into_iter().zip(t().dialogs.weight_presets);

    let base = draft.clone();
    let current = std::sync::Arc::new(move || {
        let mut d = base.clone();
        d.name = name.get_untracked();
        d.weight = weight.get_untracked().trim().parse::<i32>().unwrap_or(0);
        let p = parent.get_untracked();
        d.parent = (!p.is_empty()).then(|| PersonId::new(p));
        d
    });

    let tour_id = StoredValue::new(tour.id.as_str().to_owned());
    let carried = RwSignal::new(carried_over);
    let close = Callback::new({
        let current = current.clone();
        move |()| {
            if !editing {
                crate::drafts::keep_person(&tour_id.get_value(), &current());
            }
            on_close.run(());
        }
    });
    let start_blank = move |_| {
        let fresh = PersonDraft::new();
        name.set(fresh.name.clone());
        weight.set(fresh.weight.to_string());
        parent.set(String::new());
        error.set(String::new());
        crate::drafts::forget_person(&tour_id.get_value());
        carried.set(false);
    };

    let submit = move |_| {
        let mut d = current();

        if let Some(why) = d.problem() {
            error.set(why.to_owned());
            return;
        }
        if d.id.is_none() {
            d.id = Some(PersonId::new(edit::new_id()));
            d.editing = false;
        }
        crate::drafts::forget_person(&tour_id.get_value());
        on_apply.run(Operation::PutPerson(d));
    };

    let title = if editing { t().dialogs.edit_person } else { t().dialogs.add_person };
    let footer = {
        let submit = submit.clone();
        ViewFn::from(move || {
            let submit = submit.clone();
            view! {
                // Beside the button that produced it. There was nowhere at all for this
                // before: saving a person with no name simply did nothing.
                <Show when=move || !error.get().is_empty()>
                    <div class="tcn-errors" role="alert" style="flex:1 1 100%; margin:0 0 8px">
                        {move || error.get()}
                    </div>
                </Show>
                <span style="flex:1 1 auto"></span>
                <button type="button" class="tcn-btn" on:click=move |_| close.run(())>{t().dialogs.cancel}</button>
                <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit.clone()>
                    {if editing { t().dialogs.save } else { t().dialogs.add_person }}
                </button>
            }
        })
    };

    view! {
        <Modal title=title.to_owned() on_close=close footer=footer>
            <Show when=move || carried.get()>
                <div class="tcn-chip tcn-chip-amber tcw-wraps tcw-carried">
                    {t().dialogs.carried}
                    <button type="button" class="tcn-btn tcn-btn-sm" on:click=start_blank>
                        {t().dialogs.start_blank}
                    </button>
                </div>
            </Show>
            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.name}</div>
                <div class="tcn-namerow">
                    // The avatar is the app's, and it is not decoration: two people called
                    // Дима get different colours and different initials, and this is where
                    // the reader finds out which one they are typing.
                    <span class="tcn-avatar"
                          style=move || format!("background:{}", crate::ui::avatar_colour(&name.get()))>
                        {move || crate::ui::initials(&name.get())}
                    </span>
                    <input class="tcn-input" type="text" placeholder=t().dialogs.who_joins
                           prop:value=move || name.get()
                           on:input=move |ev| name.set(event_target_value(&ev)) />
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.share}</div>
                <div class="tcn-chips">
                    {presets
                        .map(|(w, label)| {
                            view! {
                                <span class="tcn-chip tcn-filter-chip"
                                      class:is-on=move || {
                                          weight.get().trim().parse::<i32>() == Ok(w)
                                      }
                                      on:click=move |_| weight.set(w.to_string())>
                                    {label}
                                </span>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="tcn-row" style="margin-top:8px">
                    <span class="tcn-hint" style="margin:0">{t().dialogs.custom}</span>
                    <input class="tcn-input" style="width:100px" type="number" inputmode="numeric"
                           min="0"
                           prop:value=move || weight.get()
                           on:input=move |ev| weight.set(event_target_value(&ev)) />
                </div>
                <div class="tcn-hint">
                    {t().dialogs.share_note}
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.paid_for_by}</div>
                <div class="tcn-pchips">
                    <button type="button" class="tcn-pchip"
                            class:is-on=move || parent.get().is_empty()
                            on:click=move |_| parent.set(String::new())>
                        <span class="tcn-pchip-name">{t().dialogs.pays_for_self}</span>
                    </button>
                    {candidates
                        .iter()
                        .map(|p| {
                            let id = p.id.as_str().to_owned();
                            let mine = id.clone();
                            view! {
                                <button type="button" class="tcn-pchip"
                                        class:is-on=move || parent.get() == mine
                                        on:click=move |_| parent.set(id.clone())>
                                    <crate::tour::Avatar name=p.name.clone() />
                                    <span class="tcn-pchip-name">{p.name.clone()}</span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="tcn-hint">
                    {t().dialogs.paid_for_note}
                </div>
            </div>

        </Modal>
    }
}

/// The tour's own properties.
///
/// Replaces the rename box: renaming turned out to be one of four things this dialog is
/// for, and the other three had nowhere to live.
#[component]
pub fn TourDialog(
    draft: TourDraft,
    on_close: Callback<()>,
    on_apply: Callback<Operation>,
) -> impl IntoView {
    let name = RwSignal::new(draft.name.clone());
    let days = RwSignal::new(draft.days.to_string());
    let archived = RwSignal::new(draft.archived);
    let finalizing = RwSignal::new(draft.finalizing);
    let error = RwSignal::new(String::new());

    let submit = move |_| {
        let d = TourDraft {
            name: name.get(),
            days: days.get().trim().parse().unwrap_or(0),
            archived: archived.get(),
            finalizing: finalizing.get(),
        };
        if let Some(why) = d.problem() {
            error.set(why.to_owned());
            return;
        }
        on_apply.run(Operation::EditTour(d));
    };

    let footer = ViewFn::from(move || {
        view! {
            <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>{t().dialogs.cancel}</button>
            <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit>{t().dialogs.save}</button>
        }
    });

    view! {
        <Modal title=t().dialogs.edit_tour.to_owned() on_close=on_close footer=footer>
            <Show when=move || !error.get().is_empty()>
                <div class="tcn-errors">{move || error.get()}</div>
            </Show>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.name}</div>
                <input class="tcn-input" type="text"
                       prop:value=move || name.get()
                       on:input=move |ev| name.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.days}</div>
                <input class="tcn-input" type="number" min="1" inputmode="numeric"
                       prop:value=move || days.get()
                       on:input=move |ev| days.set(event_target_value(&ev)) />
                <div class="tcn-hint">{t().dialogs.days_note}</div>
            </div>

            <div class="tcn-field">
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || finalizing.get()
                           on:change=move |ev| finalizing.set(event_target_checked(&ev)) />
                    {t().dialogs.settling}
                    <span class="tcn-hint">{t().dialogs.settling_note}</span>
                </label>
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || archived.get()
                           on:change=move |ev| archived.set(event_target_checked(&ev)) />
                    {t().dialogs.archived}
                    <span class="tcn-hint">{t().dialogs.archived_note}</span>
                </label>
            </div>
        </Modal>
    }
}

/// The currencies of a tour, and which one the totals are worked out in.
///
/// The list always ends with a blank row, so there is nothing to "add" and nothing to
/// forget to add - the C# grew that after a separate Add button silently dropped whatever
/// had been typed into it when Save was pressed instead.
#[component]
pub fn CurrenciesDialog(
    tour: Tour,
    on_close: Callback<()>,
    on_apply: Callback<Operation>,
) -> impl IntoView {
    let with_blank = |mut rows: Vec<CurrencyDraft>| -> Vec<CurrencyDraft> {
        while rows.last().is_some_and(|c| c.is_blank()) {
            rows.pop();
        }
        rows.push(CurrencyDraft::blank());
        rows
    };

    let rows = RwSignal::new(with_blank(
        tour.currencies.iter().map(CurrencyDraft::of).collect(),
    ));
    // The tour as stored: the page may be showing it in another currency (`show_in`), and
    // what this dialog saves as the main currency must be the real one.
    let tour = crate::show_in::stored(&tour);
    let main = RwSignal::new(tour.currency().id.as_str().to_owned());
    let problems: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let original = StoredValue::new(tour.clone());
    // The worth being typed, before the box is left: which row, and what it says so far. The
    // rows themselves take it on `change`, since rewriting them redraws the list; this is only
    // for the "with cents" box beside it, which should follow the typing - 6 000 no, 60 000
    // yes, back to 6 000 no - as long as nobody has ticked or unticked it by hand.
    let typing: RwSignal<Option<(usize, i32)>> = RwSignal::new(None);

    // "Today's rate" (`crate::rates`): what each row was told - the worth it had, or why there
    // is no rate - and the question in flight, and the offer to multiply every worth first.
    let rate_notes: RwSignal<Vec<(usize, RateNote)>> = RwSignal::new(Vec::new());
    let asking: RwSignal<Option<usize>> = RwSignal::new(None);
    let raise_offer: RwSignal<Option<(usize, i32)>> = RwSignal::new(None);
    let raised: RwSignal<Option<i32>> = RwSignal::new(None);
    let raise_declined = RwSignal::new(false);
    let fetched: StoredValue<Option<crate::rates::Rates>> = StoredValue::new(None);
    let rate_rows = move || -> Vec<crate::rates::Row> {
        let all = rows.get_untracked();
        all.iter()
            .map(|c| crate::rates::Row {
                id: c.id.clone(),
                name: c.name.clone(),
                rate: c.rate,
                cents: c.effective_cents(&all),
            })
            .collect()
    };
    let note_for = move |at: usize, note: RateNote| {
        rate_notes.update(|all| {
            all.retain(|(r, _)| *r != at);
            all.push((at, note));
        });
    };
    // `quiet`: a row nobody asked about says nothing when it has no rate (chips).
    let work_out = move |at: usize, quiet: bool| {
        let Some(rates) = fetched.get_value() else { return };
        match crate::rates::refine(&rate_rows(), at, &rates) {
            Ok(changes) => {
                let date = rate_date(rates.updated);
                let before = rows.get_untracked();
                typing.set(None);
                rows.update(|all| {
                    for (i, worth) in &changes {
                        if let Some(row) = all.get_mut(*i) {
                            row.rate = *worth;
                        }
                    }
                });
                for (i, _) in changes {
                    let was = before.get(i).map(|c| c.rate).unwrap_or_default();
                    note_for(i, RateNote::Was(was, date.clone()));
                }
            }
            Err(why) if !quiet => note_for(at, RateNote::Failed(rate_failure(&why))),
            Err(_) => {}
        }
    };
    let ask_rate = move |at: usize| {
        if asking.get_untracked().is_some() {
            return;
        }
        asking.set(Some(at));
        raise_offer.set(None);
        spawn_local(async move {
            let got = crate::rates::fetch().await;
            asking.set(None);
            match got {
                Err(why) => note_for(at, RateNote::Failed(rate_failure(&why))),
                Ok(rates) => {
                    let offer = if raise_declined.get_untracked() {
                        None
                    } else {
                        crate::rates::raise_for(&rate_rows(), at, &rates)
                    };
                    fetched.set_value(Some(rates));
                    match offer {
                        Some(factor) => raise_offer.set(Some((at, factor))),
                        None => work_out(at, false),
                    }
                }
            }
        });
    };
    let answer_raise = move |yes: bool| {
        let Some((at, factor)) = raise_offer.get_untracked() else { return };
        raise_offer.set(None);
        if yes {
            typing.set(None);
            rows.update(|all| {
                for row in all.iter_mut().filter(|c| !c.is_blank()) {
                    row.rate = row.rate.saturating_mul(factor);
                }
            });
            raised.update(|r| *r = Some(r.unwrap_or(1).saturating_mul(factor)));
            // The worths shown as "was" are the old scale now.
            rate_notes.set(Vec::new());
            // Every currency the source knows, not only the one asked about: a worth that was
            // already off (the dinar set to 100 beside a lev at 60 100) would otherwise be
            // multiplied as it was, and "made exact" would be true of one row only.
            work_out(at, false);
            let all = rate_rows();
            for i in (0..all.len()).filter(|i| *i != at && crate::rates::has_button(&all, *i)) {
                work_out(i, true);
            }
        } else {
            raise_declined.set(true);
            work_out(at, false);
        }
    };

    // What saving would do to the expenses, said before it is done: a removed currency's
    // expenses converted, amounts rounded by switching cents off, an EURc folded in.
    let kept_now = move || -> Vec<CurrencyDraft> {
        let all = rows.get();
        all.iter()
            .filter(|c| !c.is_blank())
            .map(|c| CurrencyDraft { cents: c.effective_cents(&all), cents_auto: false, ..c.clone() })
            .collect()
    };
    let plan = Memo::new(move |_| {
        let kept = kept_now();
        let main = main.get();
        original.with_value(|tour| {
            {
                let p = edit::plan_currencies(tour, &kept, &main);
                let mut notes: Vec<String> = Vec::new();
                for (from, into, n) in &p.moved {
                    notes.push((t().dialogs.will_move)(*n, from, into));
                }
                if p.rounded > 0 {
                    notes.push((t().dialogs.will_round)(p.rounded));
                }
                for (c, into, n) in &p.absorbed {
                    notes.push((t().dialogs.will_absorb)(*n, c, into));
                }
                notes
            }
        })
    });

    let submit = move |_| {
        let kept = kept_now();
        let mut found = edit::currency_problems(&kept);
        if !found.is_empty() {
            problems.set(found);
            return;
        }
        let chosen = if kept.iter().any(|c| c.id == main.get()) {
            main.get()
        } else {
            kept.first().map(|c| c.id.clone()).unwrap_or_default()
        };
        on_apply.run(Operation::SetCurrencies { kept, main: chosen });
    };

    let footer = ViewFn::from(move || {
        view! {
            <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>{t().dialogs.cancel}</button>
            <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit>{t().dialogs.save}</button>
        }
    });

    view! {
        <Modal title=(t().dialogs.currencies_of)(&tour.name) on_close=on_close footer=footer>
            <Show when=move || !problems.get().is_empty()>
                <div class="tcn-errors">
                    {move || problems.get().into_iter().map(|p| view! { <div>{p}</div> }).collect_view()}
                </div>
            </Show>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.main_currency}</div>
                <div class="tcn-chips">
                    {move || rows.get()
                        .into_iter()
                        .filter(|c| !c.is_blank())
                        .map(|c| {
                            let id = c.id.clone();
                            let mine = id.clone();
                            view! {
                                <span class="tcn-chip tcn-filter-chip"
                                      class:is-on=move || main.get() == mine
                                      on:click=move |_| main.set(id.clone())>
                                    {c.name.clone()}
                                </span>
                            }
                        })
                        .collect_view()}
                </div>
                <div class="tcn-hint">
                    {t().dialogs.main_currency_note}
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">{t().dialogs.rates}</div>
                <div class="tcn-hint" style="margin: 0 0 8px 0">
                    {t().dialogs.rates_note}
                </div>

                {move || {
                    let all = rows.get();
                    let for_rates = rate_rows();
                    all.into_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let blank = c.is_blank();
                        let rate_button = crate::rates::has_button(&for_rates, i);
                        let refined = move || rate_notes.with(|n| {
                            n.iter().any(|(r, x)| *r == i && matches!(x, RateNote::Was(..)))
                        });
                        view! {
                            <div class="tcn-currow">
                                <input class="tcn-input tcn-cur-name" type="text"
                                       placeholder=if blank { t().dialogs.add_currency } else { "" }
                                       prop:value=c.name.clone()
                                       on:input=move |ev| {
                                           let text = event_target_value(&ev);
                                           rows.update(|all| {
                                               if let Some(row) = all.get_mut(i) {
                                                   row.name = text;
                                               }
                                               // Always exactly one empty row at the end:
                                               // typing into the last one makes the next.
                                               while all.last().is_some_and(|c| c.is_blank()) {
                                                   all.pop();
                                               }
                                               all.push(CurrencyDraft::blank());
                                           });
                                       } />
                                <span class="tcn-cur-worth-label">{t().dialogs.worth}</span>
                                <input class="tcn-input tcn-cur-rate" type="number" min="1"
                                       class:tcw-refined=refined
                                       prop:value=c.rate
                                       on:input=move |ev| {
                                           let rate = event_target_value(&ev).trim().parse().unwrap_or(0);
                                           typing.set(Some((i, rate)));
                                       }
                                       on:change=move |ev| {
                                           let rate = event_target_value(&ev).trim().parse().unwrap_or(0);
                                           rows.update(|all| {
                                               if let Some(row) = all.get_mut(i) {
                                                   row.rate = rate;
                                               }
                                           });
                                       } />
                                {if blank {
                                    view! { <span class="tcn-currow-spacer"></span> }.into_any()
                                } else {
                                    view! {
                                        <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                                                title=t().dialogs.remove
                                                on:click=move |_| {
                                                    rows.update(|all| { all.remove(i); });
                                                    // They were said of rows by place.
                                                    rate_notes.set(Vec::new());
                                                    raise_offer.set(None);
                                                }>
                                            "✕"
                                        </button>
                                    }.into_any()
                                }}
                            </div>
                            {(!blank).then(|| {
                                // Follows the worth as it is typed, until chosen by hand.
                                let cents_now = move || {
                                    let mut all = rows.get();
                                    if let Some((at, rate)) = typing.get() {
                                        if let Some(row) = all.get_mut(at) {
                                            row.rate = rate;
                                        }
                                    }
                                    all.get(i).map(|row| row.effective_cents(&all)).unwrap_or(false)
                                };
                                // An EURc to fold in: only when this currency is being given
                                // cents now, and there is one beside it that is still kept.
                                let had_cents = original.with_value(|t| {
                                    t.currencies.iter().any(|x| x.id.as_str() == c.id && x.with_cents())
                                });
                                let sibling = original.with_value(|t| {
                                    tc_core::units::cents_sibling(t, &tc_core::CurrencyId::new(c.id.clone()))
                                        .map(|s| (s.id.as_str().to_owned(), s.name.clone()))
                                })
                                .filter(|(sid, _)| rows.get_untracked().iter().any(|r| &r.id == sid));
                                // Read once, untracked: tracked here, every keystroke in a worth
                                // redrew the whole list - the box lost what was typed and the
                                // focus with it.
                                let offer = (untrack(cents_now) && !had_cents && !c.id.is_empty())
                                    .then_some(sibling)
                                    .flatten();
                                let name = c.name.clone();
                                view! {
                                    <div class="tcw-cur-cents">
                                        <label class="tcn-switchline" title=t().dialogs.with_cents_hint>
                                            <input type="checkbox" prop:checked=cents_now
                                                   on:change=move |ev| {
                                                       let on = event_target_checked(&ev);
                                                       rows.update(|all| {
                                                           if let Some(row) = all.get_mut(i) {
                                                               row.cents = on;
                                                               row.cents_auto = false;
                                                               // Folding the EURc in is what
                                                               // somebody giving EUR cents means.
                                                               row.absorb = on;
                                                           }
                                                       });
                                                   } />
                                            {t().dialogs.with_cents}
                                        </label>
                                        {offer.map(|(_, sname)| view! {
                                            <label class="tcn-switchline">
                                                <input type="checkbox" prop:checked=c.absorb
                                                       on:change=move |ev| {
                                                           let on = event_target_checked(&ev);
                                                           rows.update(|all| {
                                                               if let Some(row) = all.get_mut(i) {
                                                                   row.absorb = on;
                                                               }
                                                           });
                                                       } />
                                                {(t().dialogs.absorb)(&sname, &name)}
                                            </label>
                                        })}
                                        {rate_button.then(|| view! {
                                            <button type="button" class="tcn-linkbtn tcw-rate-btn"
                                                    disabled=move || asking.get().is_some()
                                                    on:click=move |_| ask_rate(i)>
                                                {move || if asking.get() == Some(i) {
                                                    t().dialogs.rate_asking
                                                } else {
                                                    t().dialogs.rate_button
                                                }}
                                            </button>
                                        })}
                                    </div>
                                    {move || rate_notes.get().into_iter().find(|(r, _)| *r == i).map(|(_, note)| match note {
                                        RateNote::Was(was, date) => view! {
                                            <div class="tcw-rate-note">
                                                {(t().dialogs.rate_was)(&crate::ui::amount(Cents(was as i64), false), &date)}
                                                " "
                                                <a href=crate::rates::SOURCE_LINK target="_blank" rel="noopener"
                                                   title=t().dialogs.rate_source_hint>
                                                    {crate::rates::SOURCE_NAME}
                                                </a>
                                            </div>
                                        }.into_any(),
                                        RateNote::Failed(why) => view! {
                                            <div class="tcw-rate-note is-failed">{why}</div>
                                        }.into_any(),
                                    })}
                                    {move || raise_offer.get().filter(|(at, _)| *at == i).map(|(_, factor)| view! {
                                        <div class="tcn-chip tcn-chip-amber tcw-wraps tcw-raise">
                                            <div>{(t().dialogs.raise_offer)(factor)}</div>
                                            <div class="tcw-raise-buttons">
                                                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-primary"
                                                        on:click=move |_| answer_raise(true)>
                                                    {t().dialogs.raise_yes}
                                                </button>
                                                <button type="button" class="tcn-btn tcn-btn-sm"
                                                        on:click=move |_| answer_raise(false)>
                                                    {t().dialogs.raise_no}
                                                </button>
                                            </div>
                                        </div>
                                    })}
                                }
                            })}
                        }
                    })
                    .collect_view()
                }}

                {move || raised.get().map(|f| view! {
                    <div class="tcn-hint">{(t().dialogs.raised)(f)}</div>
                })}

                {move || {
                    let notes = plan.get();
                    (!notes.is_empty()).then(|| view! {
                        <div class="tcn-chip tcn-chip-amber tcw-wraps" style="margin-top:8px">
                            {notes.into_iter().map(|n| view! { <div>{n}</div> }).collect_view()}
                        </div>
                    })
                }}

                <div class="tcn-hint">
                    {t().dialogs.rename_note}
                </div>
            </div>
        </Modal>
    }
}

/// What "today's rate" said of a row: the worth it had before, and the date of the rate - or
/// why there is none.
#[derive(Clone, Debug)]
enum RateNote {
    Was(i32, String),
    Failed(String),
}

fn rate_failure(why: &crate::rates::Why) -> String {
    use crate::rates::Why;
    let d = &t().dialogs;
    match why {
        Why::Unknown(name) => (d.rate_unknown)(name),
        Why::NotAtSource(code) => (d.rate_not_at_source)(code),
        Why::NothingToCompare => d.rate_nothing_to_compare.to_owned(),
        Why::Offline => d.rate_offline.to_owned(),
        Why::OutOfRange => d.rate_out_of_range.to_owned(),
    }
}

/// The day of a rate, on the reader's calendar.
fn rate_date(unix: i64) -> String {
    if cfg!(not(target_arch = "wasm32")) || unix <= 0 {
        return String::new();
    }
    let day = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(unix as f64 * 1000.0));
    (t().dialogs.rate_date)(day.get_date(), day.get_month() + 1)
}

/// What this tour used to be.
///
/// Restoring **adds a copy** rather than overwriting: the tour on screen stays exactly as it
/// is, and the version arrives as a separate tour named after the moment it was taken. That
/// is what the app does, and it is the right way round - somebody looking through the
/// history is not necessarily asking to lose today's work.
#[component]
pub fn VersionsDialog(tour: Tour, on_close: Callback<()>) -> impl IntoView {
    let state: RwSignal<Option<Result<Vec<Tour>, String>>> = RwSignal::new(None);
    let busy = RwSignal::new(false);
    let note = RwSignal::new(String::new());

    {
        let id = tour.id.as_str().to_owned();
        spawn_local(async move {
            state.set(Some(api::versions(&id).await.map_err(|e| e.said)));
        });
    }

    let tour_name = tour.name.clone();
    let restore = {
        let code = tour
            .extras
            .0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("AccessCodeMD5"))
            .and_then(|(_, v)| v.as_str())
            .unwrap_or("")
            .to_owned();
        let tour_name = tour_name.clone();
        Callback::new(move |version: Tour| {
            let question = t().dialogs.restore_question;
            let agreed = web_sys::window()
                .and_then(|w| w.confirm_with_message(question).ok())
                .unwrap_or(false);
            if !agreed {
                return;
            }

            busy.set(true);
            let code = code.clone();
            let tour_name = tour_name.clone();
            spawn_local(async move {
                // The list has no people and no expenses in it - the server strips them -
                // so the version is fetched whole by its own id before being sent back.
                let whole = match api::tour(version.id.as_str()).await {
                    Ok(t) => t,
                    Err(e) => {
                        note.set(e.to_string());
                        busy.set(false);
                        return;
                    }
                };
                let when = version_stamp(&version);
                let comment = version_comment(&version);
                let mut body: serde_json::Value = match whole
                    .to_json()
                    .ok()
                    .and_then(|j| serde_json::from_str(&j).ok())
                {
                    Some(v) => v,
                    None => {
                        note.set(t().dialogs.cannot_read_version.into());
                        busy.set(false);
                        return;
                    }
                };
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "Name".into(),
                        (t().dialogs.restored_name)(&tour_name, &when, &comment).into(),
                    );
                    // It is a tour now, not a version of one.
                    obj.insert("IsVersion".into(), false.into());
                    obj.insert("VersionFor_Id".into(), "".into());
                }

                match api::add_tour(body, api::Pile::Hashed(&code)).await {
                    Ok(_) => note.set(t().dialogs.restored.into()),
                    Err(e) => note.set(e.to_string()),
                }
                busy.set(false);
            });
        })
    };

    let footer = ViewFn::from(move || {
        view! {
            <span style="flex:1 1 auto"></span>
            <button type="button" class="tcn-btn tcn-btn-primary"
                    on:click=move |_| on_close.run(())>{t().dialogs.got_it}</button>
        }
    });

    view! {
        <Modal title=(t().dialogs.versions_of)(&tour_name) on_close=on_close footer=footer>
            <Show when=move || !note.get().is_empty()>
                <div class="tcn-chip tcn-chip-amber">{move || note.get()}</div>
            </Show>

            {move || match state.get() {
                None => view! { <div class="tcn-loading">{t().dialogs.loading_versions}</div> }.into_any(),
                Some(Err(why)) => view! { <div class="tcn-errors">{why}</div> }.into_any(),
                Some(Ok(list)) if list.is_empty() => view! {
                    <div class="tcn-empty">
                        <div class="tcn-empty-title">{t().dialogs.no_versions}</div>
                        <div>{t().dialogs.no_versions_note}</div>
                    </div>
                }.into_any(),
                Some(Ok(list)) => {
                    let total = list.len();
                    view! {
                        <div class="tcn-brk">
                            {list
                                .into_iter()
                                .enumerate()
                                .map(|(i, v)| {
                                    let restoring = v.clone();
                                    view! {
                                        <div class="tcn-brk-row">
                                            <div class="tcn-brk-main">
                                                <div class="tcn-brk-title">
                                                    {format!("{}. ", total - i)}
                                                    <b>{version_comment(&v)}</b>
                                                </div>
                                                <div class="tcn-brk-sub">{version_stamp(&v)}</div>
                                            </div>
                                            <div class="tcn-brk-amt">
                                                <button type="button" class="tcn-btn tcn-btn-sm"
                                                        prop:disabled=move || busy.get()
                                                        on:click=move |_| restore.run(restoring.clone())>
                                                    {t().dialogs.restore}
                                                </button>
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }.into_any()
                }
            }}
        </Modal>
    }
}

fn version_comment(v: &Tour) -> String {
    let text = tc_core::extras::str_of(&v.extras, tc_core::extras::VERSION_COMMENT);
    if text.trim().is_empty() {
        "changed".to_owned()
    } else {
        text
    }
}

fn version_stamp(v: &Tour) -> String {
    let text = tc_core::extras::str_of(&v.extras, tc_core::extras::VERSIONED_AT);
    crate::ui::local_stamp_of(&version_iso(&text))
}

/// A version's time as an instant the browser can read, to be shown on the reader's clock -
/// it used to be printed as stored, an hour ahead for somebody at UTC+2.
///
/// Three kinds of stamp are stored:
///
/// - with a zone and a real fraction (`…:12.345Z`, the C#'s `…+03:00`): taken as written;
/// - with no zone at all (`2026-09-09 21:24:50`, this server before 25.09.2026, in a file):
///   the wall clock at UTC+3;
/// - `…:SSZ` - the same old stamp after a trip through MongoDB, which took it for UTC and
///   gives it back with no fraction (or `.000Z`, as some writers spell it). Those were
///   written to the second; new ones never fall on a whole second (see the server's
///   `fields::now_stamp`), so this is them, and they too are UTC+3 wall clock.
fn version_iso(text: &str) -> String {
    let iso = text.trim().replacen(' ', "T", 1);
    if iso.len() < 19 {
        return iso;
    }
    if matches!(&iso[19..], "Z" | ".000Z") {
        return format!("{}+03:00", &iso[..19]);
    }
    let time = &iso[19..];
    let zoned = time.contains('Z') || time.contains('+') || time.contains('-');
    if zoned {
        iso
    } else {
        format!("{iso}+03:00")
    }
}

#[cfg(test)]
mod version_time_tests {
    use super::version_iso;

    #[test]
    fn a_stamp_with_a_zone_is_read_as_written_and_one_without_as_utc_plus_three() {
        assert_eq!(version_iso("2026-09-25T18:48:12.345Z"), "2026-09-25T18:48:12.345Z");
        // An old stamp of this server, back from MongoDB: UTC+3 wall clock after all.
        assert_eq!(version_iso("2026-09-25T21:48:12.000Z"), "2026-09-25T21:48:12+03:00");
        assert_eq!(version_iso("2026-09-25T21:48:12Z"), "2026-09-25T21:48:12+03:00");
        assert_eq!(version_iso("2022-07-03T07:40:20.4829583+03:00"), "2022-07-03T07:40:20.4829583+03:00");
        assert_eq!(version_iso("2026-09-09 21:24:50"), "2026-09-09T21:24:50+03:00");
    }
}
