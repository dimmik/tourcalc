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

use crate::edit::{self, PersonDraft, SpendingDraft};
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
    view! {
        <div class="tcn-modal"
             // Closing on *click* and not on mousedown: releasing the button over the mask
             // after a drag that started inside the dialog is not "click outside".
             on:click=move |_| on_close.run(())>
            <div class="tcn-modal-card" on:click=|ev| ev.stop_propagation()>
                <div class="tcn-modal-head">
                    <div class="tcn-modal-title">{title}</div>
                    <button type="button" class="tcn-modal-x" on:click=move |_| on_close.run(())>
                        "✕"
                    </button>
                </div>
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
    on_close: Callback<()>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let people = tour.persons.clone();
    let known_categories = edit::categories(&tour);
    let editing = draft.id.is_some();

    // One signal per field. Each is `Copy`, so the handlers below can each take their own
    // without any of them owning the form.
    let description = RwSignal::new(draft.description.clone());
    let category = RwSignal::new(draft.category.clone());
    let amount = RwSignal::new(if draft.amount.0 == 0 {
        String::new()
    } else {
        draft.amount.0.to_string()
    });
    let from = RwSignal::new(draft.from.as_str().to_owned());
    let everyone = RwSignal::new(draft.everyone);
    let to = RwSignal::new(draft.to.clone());
    let error = RwSignal::new(String::new());
    let saving = RwSignal::new(false);

    let base = draft.clone();
    let submit = {
        let tour = tour.clone();
        move |_| {
            let mut d = base.clone();
            d.description = description.get();
            d.category = category.get();
            d.amount = Cents(amount.get().trim().parse::<i64>().unwrap_or(0));
            d.from = PersonId::new(from.get());
            d.everyone = everyone.get();
            d.to = to.get();

            if let Some(why) = d.problem() {
                error.set(why.to_owned());
                return;
            }

            let next = edit::put_spending(&tour, &d);
            error.set(String::new());
            saving.set(true);
            spawn_local(async move {
                match edit::save(&next).await {
                    Ok(()) => on_saved.run(()),
                    Err(e) => {
                        error.set(e);
                        saving.set(false);
                    }
                }
            });
        }
    };

    let title = if editing {
        "Edit expense"
    } else {
        "New expense"
    };
    let footer = {
        let submit = submit.clone();
        ViewFn::from(move || {
            let submit = submit.clone();
            view! {
                <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                <button type="button" class="tcn-btn tcn-btn-primary"
                        disabled=move || saving.get()
                        on:click=submit.clone()>
                    {move || if saving.get() { "Saving…" } else { "Save" }}
                </button>
            }
        })
    };

    view! {
        <Modal title=title.to_owned() on_close=on_close footer=footer>
            <div class="tcn-field">
                <div class="tcn-label">"Amount"</div>
                <input class="tcn-input" type="number" inputmode="numeric" placeholder="0"
                       prop:value=move || amount.get()
                       on:input=move |ev| amount.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"What for"</div>
                <input class="tcn-input" type="text" placeholder="Dinner, tickets, taxi…"
                       prop:value=move || description.get()
                       on:input=move |ev| description.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Category"</div>
                <input class="tcn-input" type="text" list="tcw-categories"
                       prop:value=move || category.get()
                       on:input=move |ev| category.set(event_target_value(&ev)) />
                <datalist id="tcw-categories">
                    {known_categories
                        .iter()
                        .map(|c| view! { <option value=c.clone()></option> })
                        .collect_view()}
                </datalist>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Paid by"</div>
                <select class="tcn-input" on:change=move |ev| from.set(event_target_value(&ev))>
                    {people
                        .iter()
                        .map(|p| {
                            let id = p.id.as_str().to_owned();
                            view! {
                                <option value=id.clone() selected=move || from.get() == id>
                                    {p.name.clone()}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </div>

            <div class="tcn-field">
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || everyone.get()
                           on:change=move |ev| everyone.set(event_target_checked(&ev)) />
                    "Shared by everyone"
                </label>
            </div>

            <Show when=move || !everyone.get()>
                <div class="tcn-field">
                    <div class="tcn-label">"Split between"</div>
                    {tour
                        .persons
                        .iter()
                        .map(|p| {
                            let id = p.id.clone();
                            let checked_id = id.clone();
                            view! {
                                <label class="tcn-switchline">
                                    <input type="checkbox"
                                           prop:checked=move || to.get().contains(&checked_id)
                                           on:change={
                                               let id = id.clone();
                                               move |ev| {
                                                   let on = event_target_checked(&ev);
                                                   to.update(|list| {
                                                       list.retain(|x| x != &id);
                                                       if on { list.push(id.clone()) }
                                                   });
                                               }
                                           } />
                                    {p.name.clone()}
                                </label>
                            }
                        })
                        .collect_view()}
                </div>
            </Show>

            <Show when=move || !error.get().is_empty()>
                <div class="tcn-errors">{move || error.get()}</div>
            </Show>
        </Modal>
    }
}

/// Adding or changing a person.
#[component]
pub fn PersonDialog(
    tour: Tour,
    draft: PersonDraft,
    on_close: Callback<()>,
    on_saved: Callback<()>,
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
    let saving = RwSignal::new(false);

    // Somebody cannot pay for themselves, and a payer who is paid for by another would
    // make a chain the settlement deliberately does not follow.
    let candidates: Vec<_> = tour
        .persons
        .iter()
        .filter(|p| Some(&p.id) != myself.as_ref())
        .filter(|p| p.parent.is_none())
        .cloned()
        .collect();

    let base = draft.clone();
    let submit = {
        let tour = tour.clone();
        move |_| {
            let mut d = base.clone();
            d.name = name.get();
            d.weight = weight.get().trim().parse::<i32>().unwrap_or(0);
            let p = parent.get();
            d.parent = (!p.is_empty()).then(|| PersonId::new(p));

            if let Some(why) = d.problem() {
                error.set(why.to_owned());
                return;
            }

            let next = edit::put_person(&tour, &d);
            error.set(String::new());
            saving.set(true);
            spawn_local(async move {
                match edit::save(&next).await {
                    Ok(()) => on_saved.run(()),
                    Err(e) => {
                        error.set(e);
                        saving.set(false);
                    }
                }
            });
        }
    };

    let title = if editing { "Edit person" } else { "Add person" };
    let footer = {
        let submit = submit.clone();
        ViewFn::from(move || {
            let submit = submit.clone();
            view! {
                <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                <button type="button" class="tcn-btn tcn-btn-primary"
                        disabled=move || saving.get()
                        on:click=submit.clone()>
                    {move || if saving.get() { "Saving…" } else { "Save" }}
                </button>
            }
        })
    };

    view! {
        <Modal title=title.to_owned() on_close=on_close footer=footer>
            <div class="tcn-field">
                <div class="tcn-label">"Name"</div>
                <input class="tcn-input" type="text"
                       prop:value=move || name.get()
                       on:input=move |ev| name.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Weight"</div>
                <input class="tcn-input" type="number" inputmode="numeric"
                       prop:value=move || weight.get()
                       on:input=move |ev| weight.set(event_target_value(&ev)) />
                <div class="tcn-hint">
                    "100 is a whole share. A child who eats half as much can be 50."
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Paid for by"</div>
                <select class="tcn-input" on:change=move |ev| parent.set(event_target_value(&ev))>
                    <option value="" selected=move || parent.get().is_empty()>
                        "pays for themselves"
                    </option>
                    {candidates
                        .iter()
                        .map(|p| {
                            let id = p.id.as_str().to_owned();
                            view! {
                                <option value=id.clone() selected=move || parent.get() == id>
                                    {p.name.clone()}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </div>

            <Show when=move || !error.get().is_empty()>
                <div class="tcn-errors">{move || error.get()}</div>
            </Show>
        </Modal>
    }
}
