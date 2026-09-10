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

use crate::api;
use crate::edit::{self, PersonDraft, SpendingDraft};
use crate::edit::{CurrencyDraft, TourDraft};
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
    /// Where the finished edit goes. The dialog does not save: it says what was meant, and
    /// the page writes that down and gets it to the server when it can.
    on_apply: Callback<Operation>,
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
    let date = RwSignal::new(if draft.date.is_empty() {
        edit::today()
    } else {
        draft.date.clone()
    });
    let colour = RwSignal::new(draft.colour.clone());
    let error = RwSignal::new(String::new());

    let base = draft.clone();
    let submit = move |_| {
        let mut d = base.clone();
        d.description = description.get();
        d.category = category.get();
        d.amount = Cents(amount.get().trim().parse::<i64>().unwrap_or(0));
        d.from = PersonId::new(from.get());
        d.everyone = everyone.get();
        d.to = to.get();
        d.date = date.get();
        d.colour = colour.get();

        if let Some(why) = d.problem() {
            error.set(why.to_owned());
            return;
        }
        // A new spending gets its id here, where the edit is decided - see the note on
        // `SpendingDraft::id`.
        if d.id.is_none() {
            d.id = Some(tc_core::SpendingId::new(edit::new_id()));
        }
        on_apply.run(Operation::PutSpending(d));
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
                <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit.clone()>
                    "Save"
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
                <div class="tcn-label">"Date"</div>
                // A date input rather than free text: every browser that runs this has one,
                // and it spells the day out in whatever order the reader's locale uses while
                // handing back the same YYYY-MM-DD the data is stored in.
                <input class="tcn-input" type="date"
                       prop:value=move || date.get()
                       on:input=move |ev| date.set(event_target_value(&ev)) />
                <div class="tcn-hint">
                    "Only the day changes; an expense keeps its place among the ones entered
                     the same day."
                </div>
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

            <div class="tcn-field">
                <label class="tcn-switchline">
                    <input type="checkbox"
                           prop:checked=move || !colour.get().is_empty()
                           on:change=move |ev| colour.set(if event_target_checked(&ev) {
                               // The app's own default when a row is first marked; the
                               // colour input below changes it to anything else.
                               "#ffd54f".to_owned()
                           } else {
                               String::new()
                           }) />
                    <span class="tcn-label" style="margin:0">"Colour"</span>
                    <Show when=move || !colour.get().is_empty()>
                        <input type="color" style="margin-left:8px"
                               prop:value=move || colour.get()
                               on:input=move |ev| colour.set(event_target_value(&ev)) />
                    </Show>
                </label>
                <div class="tcn-hint">
                    "Marks the row out in the list — a tourist tax for the whole group, an
                     unexpected fine."
                </div>
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
    let candidates: Vec<_> = tour
        .persons
        .iter()
        .filter(|p| Some(&p.id) != myself.as_ref())
        .filter(|p| p.parent.is_none())
        .cloned()
        .collect();

    let base = draft.clone();
    let submit = move |_| {
        let mut d = base.clone();
        d.name = name.get();
        d.weight = weight.get().trim().parse::<i32>().unwrap_or(0);
        let p = parent.get();
        d.parent = (!p.is_empty()).then(|| PersonId::new(p));

        if let Some(why) = d.problem() {
            error.set(why.to_owned());
            return;
        }
        if d.id.is_none() {
            d.id = Some(PersonId::new(edit::new_id()));
        }
        on_apply.run(Operation::PutPerson(d));
    };

    let title = if editing { "Edit person" } else { "Add person" };
    let footer = {
        let submit = submit.clone();
        ViewFn::from(move || {
            let submit = submit.clone();
            view! {
                <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
                <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit.clone()>
                    "Save"
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
            <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
            <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit>"Save"</button>
        }
    });

    view! {
        <Modal title="Edit tour".to_owned() on_close=on_close footer=footer>
            <Show when=move || !error.get().is_empty()>
                <div class="tcn-errors">{move || error.get()}</div>
            </Show>

            <div class="tcn-field">
                <div class="tcn-label">"Name"</div>
                <input class="tcn-input" type="text"
                       prop:value=move || name.get()
                       on:input=move |ev| name.set(event_target_value(&ev)) />
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Length in days"</div>
                <input class="tcn-input" type="number" min="1" inputmode="numeric"
                       prop:value=move || days.get()
                       on:input=move |ev| days.set(event_target_value(&ev)) />
                <div class="tcn-hint">"What the per-day figures on the Stats tab divide by."</div>
            </div>

            <div class="tcn-field">
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || finalizing.get()
                           on:change=move |ev| finalizing.set(event_target_checked(&ev)) />
                    "Settling up "
                    <span class="tcn-hint">"— everyone sees the payments to make"</span>
                </label>
                <label class="tcn-switchline">
                    <input type="checkbox" prop:checked=move || archived.get()
                           on:change=move |ev| archived.set(event_target_checked(&ev)) />
                    "Archived "
                    <span class="tcn-hint">"— hidden from the default list"</span>
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
        rows.push(CurrencyDraft {
            id: String::new(),
            name: String::new(),
            rate: 100,
        });
        rows
    };

    let rows = RwSignal::new(with_blank(
        tour.currencies.iter().map(CurrencyDraft::of).collect(),
    ));
    let main = RwSignal::new(tour.currency().id.as_str().to_owned());
    let problems: RwSignal<Vec<String>> = RwSignal::new(Vec::new());

    let submit = move |_| {
        let kept: Vec<CurrencyDraft> = rows.get().into_iter().filter(|c| !c.is_blank()).collect();
        let found = edit::currency_problems(&kept);
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
            <button type="button" class="tcn-btn" on:click=move |_| on_close.run(())>"Cancel"</button>
            <button type="button" class="tcn-btn tcn-btn-primary" on:click=submit>"Save"</button>
        }
    });

    view! {
        <Modal title=format!("Currencies of {}", tour.name) on_close=on_close footer=footer>
            <Show when=move || !problems.get().is_empty()>
                <div class="tcn-errors">
                    {move || problems.get().into_iter().map(|p| view! { <div>{p}</div> }).collect_view()}
                </div>
            </Show>

            <div class="tcn-field">
                <div class="tcn-label">"Main currency"</div>
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
                    "Totals and balances are calculated in this one. It is a property of the
                     tour — changing it affects everyone. To change only what you see, use
                     “show in” in the header instead."
                </div>
            </div>

            <div class="tcn-field">
                <div class="tcn-label">"Rates"</div>
                <div class="tcn-hint" style="margin: 0 0 8px 0">
                    "“Worth” is what one unit is worth on any scale you like — only the ratio
                     matters. If one euro is 118 dinars, put 100 next to the dinar and 11800
                     next to the euro."
                </div>

                {move || rows.get()
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let blank = c.is_blank();
                        view! {
                            <div class="tcn-currow">
                                <input class="tcn-input tcn-cur-name" type="text"
                                       placeholder=if blank { "add a currency…" } else { "" }
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
                                               all.push(CurrencyDraft {
                                                   id: String::new(),
                                                   name: String::new(),
                                                   rate: 100,
                                               });
                                           });
                                       } />
                                <span class="tcn-cur-worth-label">"worth"</span>
                                <input class="tcn-input tcn-cur-rate" type="number" min="1"
                                       prop:value=c.rate
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
                                                title="Remove"
                                                on:click=move |_| rows.update(|all| { all.remove(i); })>
                                            "✕"
                                        </button>
                                    }.into_any()
                                }}
                            </div>
                        }
                    })
                    .collect_view()}

                <div class="tcn-hint">
                    "Renaming keeps the amounts: expenses stay attached to the currency they
                     were entered in, whatever you call it now. Removing one does not — those
                     expenses would be read in the main currency."
                </div>
            </div>
        </Modal>
    }
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
            state.set(Some(api::versions(&id).await));
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
            let question = "Restore this version? The tour stays as it is — the version is \
                 added as a separate copy.";
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
                        note.set(e);
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
                        note.set("could not read that version".into());
                        busy.set(false);
                        return;
                    }
                };
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "Name".into(),
                        format!("{tour_name} (v {when} before {comment})").into(),
                    );
                    // It is a tour now, not a version of one.
                    obj.insert("IsVersion".into(), false.into());
                    obj.insert("VersionFor_Id".into(), "".into());
                }

                match api::add_tour(body, &code).await {
                    Ok(_) => note.set("Restored as a new tour — it is in your list.".into()),
                    Err(e) => note.set(e),
                }
                busy.set(false);
            });
        })
    };

    let footer = ViewFn::from(move || {
        view! {
            <span style="flex:1 1 auto"></span>
            <button type="button" class="tcn-btn tcn-btn-primary"
                    on:click=move |_| on_close.run(())>"Got it"</button>
        }
    });

    view! {
        <Modal title=format!("Versions of “{tour_name}”") on_close=on_close footer=footer>
            <Show when=move || !note.get().is_empty()>
                <div class="tcn-chip tcn-chip-amber">{move || note.get()}</div>
            </Show>

            {move || match state.get() {
                None => view! { <div class="tcn-loading">"Loading versions…"</div> }.into_any(),
                Some(Err(why)) => view! { <div class="tcn-errors">{why}</div> }.into_any(),
                Some(Ok(list)) if list.is_empty() => view! {
                    <div class="tcn-empty">
                        <div class="tcn-empty-title">"No versions yet"</div>
                        <div>"A version is kept whenever a save changes something."</div>
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
                                                    "Restore"
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
    // Whatever shape it was written in, the day and the minute are what a person reads.
    text.replace('T', " ").chars().take(16).collect()
}
