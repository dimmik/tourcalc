//! Every tour this access code may see.
//!
//! The list endpoint sends the tours without their spendings - the screen shows neither -
//! so unlike the tour page there is nothing to calculate here: what each person spent has
//! already been worked out server-side and travels with the tour.

use crate::api;
use crate::tour::Load;
use crate::ui::money;
use leptos::prelude::*;
use leptos::task::spawn_local;
use tc_core::{Cents, Tour};

#[component]
pub fn TourListPage() -> impl IntoView {
    let (state, set_state) = signal(Load::Loading);
    let search = RwSignal::new(String::new());
    let adding = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_code = RwSignal::new(String::new());
    let new_json = RwSignal::new(String::new());
    let show_archived = RwSignal::new(false);
    let trouble = RwSignal::new(String::new());
    let busy = RwSignal::new(false);

    // The list this device saw last, drawn before the server is asked. Coming back from a
    // tour is the commonest move in the app, and it should not be a blank screen for the
    // length of a request to show the same seven names again.
    if let Some(known) = crate::queue::cached_list() {
        set_state.set(Load::Ready(known));
    }

    let load = Callback::new(move |_: ()| {
        spawn_local(async move {
            match api::tours().await {
                Ok(ts) => {
                    crate::queue::cache_list(&ts);
                    set_state.set(Load::Ready(ts));
                }
                // A failure with something already on screen keeps it: an old list is more
                // use than an error page, and the tours in it still open.
                Err(e) => {
                    if !matches!(state.get_untracked(), Load::Ready(_)) {
                        set_state.set(Load::Failed(e));
                    }
                }
            }
        });
    });
    load.run(());

    let mode = use_context::<RwSignal<crate::mode::UiMode>>()
        .unwrap_or_else(|| RwSignal::new(crate::mode::UiMode::Full));

    let create = Callback::new(move |()| {
        let name = new_name.get().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let code = new_code.get().trim().to_owned();
        let pasted = new_json.get().trim().to_owned();
        trouble.set(String::new());

        // A pasted tour starts the new one off: its people, its expenses, its currencies.
        // Anything that says *which* tour it was - the id, the access code, the state - is
        // dropped, because this is a new tour and not that one.
        let body = if pasted.is_empty() {
            serde_json::json!({ "Name": name, "Persons": [], "Spendings": [] })
        } else {
            match serde_json::from_str::<serde_json::Value>(&pasted) {
                Ok(mut v) => {
                    if let Some(obj) = v.as_object_mut() {
                        for key in [
                            "Id",
                            "GUID",
                            "AccessCodeMD5",
                            "StateGUID",
                            "IsVersion",
                            "VersionFor_Id",
                            "VersionComment",
                            "DateVersioned",
                        ] {
                            obj.remove(key);
                        }
                        obj.insert("Name".into(), name.clone().into());
                    }
                    v
                }
                Err(e) => {
                    trouble.set(format!("That does not look like a tour: {e}"));
                    return;
                }
            }
        };

        busy.set(true);
        spawn_local(async move {
            match api::add_tour(body, &code).await {
                Ok(_id) => {
                    adding.set(false);
                    new_name.set(String::new());
                    new_code.set(String::new());
                    new_json.set(String::new());
                    load.run(());
                }
                Err(e) => trouble.set(e),
            }
            busy.set(false);
        });
    });

    // Clones a tour, with or without what was spent on it. The second is what a group
    // going on the same trip again wants: the same people, the same weights, the same
    // currencies, and none of last year's taxis.
    let clone = Callback::new(move |(tour, strip): (Tour, bool)| {
        trouble.set(String::new());
        busy.set(true);
        spawn_local(async move {
            // The list carries no spendings - the server strips them - so the tour is
            // fetched whole before being copied.
            let whole = match api::tour(tour.id.as_str()).await {
                Ok(t) => t,
                Err(e) => {
                    trouble.set(e);
                    busy.set(false);
                    return;
                }
            };
            let Some(mut body) = whole
                .to_json()
                .ok()
                .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
            else {
                trouble.set("could not read that tour".into());
                busy.set(false);
                return;
            };
            if let Some(obj) = body.as_object_mut() {
                obj.insert("Name".into(), format!("clone of {}", tour.name).into());
                for key in ["Id", "GUID", "StateGUID"] {
                    obj.remove(key);
                }
                if strip {
                    obj.insert("Spendings".into(), serde_json::json!([]));
                }
            }
            // No access code: an ordinary reader gets their own, which is where the
            // original came from anyway.
            match api::add_tour(body, "").await {
                Ok(_) => load.run(()),
                Err(e) => trouble.set(e),
            }
            busy.set(false);
        });
    });

    // The tour as JSON, on the clipboard - what the box above pastes back.
    let copy_json = Callback::new(move |tour: Tour| {
        trouble.set(String::new());
        spawn_local(async move {
            let whole = match api::tour(tour.id.as_str()).await {
                Ok(t) => t,
                Err(e) => {
                    trouble.set(e);
                    return;
                }
            };
            // Without the suggested payments: they are worked out from the rest, and a copy
            // that carried them would look like a tour where somebody had already paid.
            let mut copy = whole;
            copy.spendings.retain(|s| s.kind != tc_core::Kind::Planned);
            let Ok(text) = copy.to_json() else {
                trouble.set("could not write that tour out".into());
                return;
            };
            match copy_to_clipboard(&text).await {
                Ok(()) => trouble.set("Copied — the JSON is on the clipboard.".into()),
                Err(e) => trouble.set(e),
            }
        });
    });

    let remove = Callback::new(move |tour: Tour| {
        let question = format!("Delete '{}' and everything in it?", tour.name);
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message(&question).ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        trouble.set(String::new());
        spawn_local(async move {
            match api::delete_tour(tour.id.as_str()).await {
                Ok(()) => load.run(()),
                Err(e) => trouble.set(e),
            }
        });
    });

    // The small interface carries its own bar and its own create form, so the roomy ones
    // are not drawn at all - which is a thing a screenshot catches and a driver does not:
    // the rows were right and the page had two of everything above them.
    let full = move || mode.get() == crate::mode::UiMode::Full;

    view! {
        <Show when=full>
        <div class="tcn-section">
            <div class="tcn-toolbar">
                <div class="tcn-search">
                    <span class="tcn-search-icon">"🔎"</span>
                    <input type="text" placeholder="Search"
                           prop:value=move || search.get()
                           on:input=move |ev| search.set(event_target_value(&ev)) />
                    <Show when=move || !search.get().is_empty()>
                        <button type="button" class="tcn-search-clear" title="Clear"
                                on:click=move |_| search.set(String::new())>"✕"</button>
                    </Show>
                </div>
                <button type="button" class="tcn-btn tcn-btn-primary"
                        on:click=move |_| adding.update(|a| *a = !*a)>
                    "+ New tour"
                </button>
                <label class="tcn-switchline" style="margin:0 0 0 10px">
                    <input type="checkbox" prop:checked=move || show_archived.get()
                           on:change=move |ev| show_archived.set(event_target_checked(&ev)) />
                    "Show archived"
                </label>
            </div>

            <Show when=move || adding.get()>
                <div class="tcn-form">
                    <div class="tcn-field">
                        <div class="tcn-field-label">"Tour name"</div>
                        <input class="tcn-input" type="text" placeholder="Alps 2026"
                               prop:value=move || new_name.get()
                               on:input=move |ev| new_name.set(event_target_value(&ev)) />
                    </div>
                    <div class="tcn-field">
                        <div class="tcn-field-label">"Tour JSON (optional)"</div>
                        <textarea class="tcn-input" rows="3"
                                  placeholder="paste an exported tour to start from it"
                                  prop:value=move || new_json.get()
                                  on:input=move |ev| new_json.set(event_target_value(&ev))></textarea>
                    </div>
                    <div class="tcn-field">
                        <div class="tcn-field-label">"Access code"</div>
                        <input class="tcn-input" type="text"
                               placeholder="only an administrator may choose one"
                               prop:value=move || new_code.get()
                               on:input=move |ev| new_code.set(event_target_value(&ev)) />
                        <div class="tcn-hint">
                            "Left empty - and for anybody but an administrator, always - the
                             tour joins the code you are signed in with."
                        </div>
                    </div>
                    <button type="button" class="tcn-btn tcn-btn-primary"
                            prop:disabled=move || busy.get() on:click=move |_| create.run(())>
                        "Create tour"
                    </button>
                </div>
            </Show>

        </div>
        </Show>

        // Whatever went wrong is worth saying in either interface.
        <Show when=move || !trouble.get().is_empty()>
            <div class="tcn-section" style="padding-bottom:0">
                <div class="tcn-errors">{move || trouble.get()}</div>
            </div>
        </Show>

        {move || match state.get() {
            Load::Loading => {
                view! { <div class="tcn-loading">"Loading your tours…"</div> }.into_any()
            }
            Load::Failed(why) => view! {
                <div class="tcn-section">
                    <div class="tcn-errors">{why}</div>
                    <p class="tcn-hint">
                        "Opening a tour link signs you in - ask whoever shared the tour to send it again."
                    </p>
                </div>
            }.into_any(),
            Load::Ready(tours) if mode.get() == crate::mode::UiMode::Mini => view! {
                <crate::mini::MiniList tours=tours search=search show_archived=show_archived
                                       adding=adding new_name=new_name new_code=new_code
                                       new_json=new_json busy=busy
                                       create=create
                                       remove=remove clone_it=clone copy_json=copy_json />
            }.into_any(),
            Load::Ready(tours) => {
                let needle = search.get().trim().to_lowercase();
                let shown: Vec<Tour> = tours
                    .into_iter()
                    // Archived tours are out of the way until asked for - that is what
                    // archiving is.
                    .filter(|t| {
                        show_archived.get()
                            || !tc_core::extras::bool_of(&t.extras, tc_core::extras::ARCHIVED)
                    })
                    .filter(|t| {
                        needle.is_empty()
                            || t.name.to_lowercase().contains(&needle)
                            // Searching by who is on it, which is how a trip gets found
                            // when its name is "поездка".
                            || t.persons.iter().any(|p| p.name.to_lowercase().contains(&needle))
                    })
                    .collect();

                if shown.is_empty() {
                    view! {
                        <div class="tcn-section">
                            <div class="tcn-empty">
                                <span class="tcn-empty-icon">"🧭"</span>
                                <div class="tcn-empty-title">
                                    {if needle.is_empty() { "No tours yet" } else { "Nothing matches" }}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="tcn-section">
                            <div class="tcn-section-title">
                                "Your tours " <span class="tcn-count">{shown.len()}</span>
                            </div>
                            <div class="tcn-tourgrid">
                                {shown
                                    .iter()
                                    .map(|t| view! {
                                        <Row tour=t.clone() remove=remove clone_it=clone
                                             copy_json=copy_json />
                                    })
                                    .collect_view()}
                            </div>
                        </div>
                    }.into_any()
                }
            }
        }}
    }
}

#[component]
fn Row(
    tour: Tour,
    remove: Callback<Tour>,
    clone_it: Callback<(Tour, bool)>,
    copy_json: Callback<Tour>,
) -> impl IntoView {
    let href = format!("/tour/{}", tour.id);
    let people = tour.persons.len();

    // The list carries what the server already worked out, in `SpentInCents` on each
    // person. It is not modelled by tc-core - the arithmetic recomputes it rather than
    // trusting it - so it is read out of the fields that came along for the ride.
    let spent: i64 = tour
        .persons
        .iter()
        .filter_map(|p| p.extras.0.get("SpentInCents").and_then(|v| v.as_i64()))
        .sum();

    let currency = if tour.currencies.len() > 1 {
        tour.currency().name.clone()
    } else {
        String::new()
    };

    let for_delete = tour.clone();
    let for_clone = tour.clone();
    let for_clone_bare = tour.clone();
    let for_json = tour.clone();
    let archived = tc_core::extras::bool_of(&tour.extras, tc_core::extras::ARCHIVED);
    // Which tours are being settled up is the thing you look for in a list of them: it says
    // which one is asking for something to be done. The app marks it here, the small
    // interface marks it, and this list did not.
    let settling = tc_core::extras::bool_of(&tour.extras, tc_core::extras::FINALIZING);

    view! {
        <div class="tcn-tour" class:is-archived=move || archived>
            <a class="tcn-tour-name" href=href>{tour.name.clone()}</a>
            <div class="tcn-tour-meta">
                <span>{people} " people"</span>
                <span>"·"</span>
                {settling.then(|| view! {
                    <span class="tcn-chip tcn-chip-amber"
                          title="Everyone can see what to pay whom">
                        "settling up"
                    </span>
                })}
                {archived.then(|| view! {
                    <span class="tcn-chip" title="Hidden from the default list">"archived"</span>
                })}
                <span title="Everything spent on this tour">
                    {money(Cents(spent))}
                    {(!currency.is_empty()).then(|| view! { "\u{a0}" {currency} })}
                </span>
            </div>
            <div class="tcn-tour-actions">
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click={
                            let t = for_clone.clone();
                            move |_| clone_it.run((t.clone(), false))
                        }>
                    "Clone"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        title="The same people and currencies, none of the expenses"
                        on:click={
                            let t = for_clone_bare.clone();
                            move |_| clone_it.run((t.clone(), true))
                        }>
                    "Clone w/o expenses"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm"
                        on:click={
                            let t = for_json.clone();
                            move |_| copy_json.run(t.clone())
                        }>
                    "Copy JSON"
                </button>
                <button type="button" class="tcn-btn tcn-btn-sm tcn-btn-danger"
                        on:click=move |_| remove.run(for_delete.clone())>
                    "Delete"
                </button>
            </div>
        </div>
    }
}

/// Puts text on the clipboard.
///
/// Through the async clipboard when the browser allows it - which needs a secure origin and
/// a gesture, both true here - and otherwise by the old trick of selecting a hidden textarea,
/// which works everywhere and is why this is not three lines.
async fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let promise = clipboard.write_text(text);
        if wasm_bindgen_futures::JsFuture::from(promise).await.is_ok() {
            return Ok(());
        }
    }

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let area = document
        .create_element("textarea")
        .map_err(|_| "could not make a textarea".to_owned())?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .map_err(|_| "not a textarea".to_owned())?;
    area.set_value(text);
    let body = document.body().ok_or("no body")?;
    body.append_child(&area)
        .map_err(|_| "could not add it".to_owned())?;
    area.select();
    let copied = document
        .dyn_into::<web_sys::HtmlDocument>()
        .ok()
        .and_then(|d| d.exec_command("copy").ok())
        .unwrap_or(false);
    let _ = body.remove_child(&area);

    if copied {
        Ok(())
    } else {
        Err("This browser would not let the page copy it.".into())
    }
}
