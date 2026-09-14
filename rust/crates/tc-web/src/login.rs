//! Signing in.
//!
//! There are no user accounts. A tour belongs to an access code, and anybody who knows the
//! code sees the tours filed under it - so the code is what gets shared, and typing it is
//! the whole of logging in. `admin:` in front of the master key signs in as administrator.
//!
//! Deliberately a screen and not a dialog, for the reason the app learned the hard way: a
//! modal with nothing behind it and no way to dismiss is a trap, especially in a standalone
//! window with no address bar.

use crate::api;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn SignIn(on_done: Callback<()>) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);

    let submit = move |_| {
        let typed = code.get().trim().to_owned();
        if typed.is_empty() {
            return;
        }
        error.set(String::new());
        busy.set(true);
        spawn_local(async move {
            // "admin:key" is the administrator's way in; anything else is an access code.
            let outcome = match typed.split_once(':') {
                Some((scope, key)) if scope.eq_ignore_ascii_case("admin") => {
                    api::log_in("admin", key).await
                }
                _ => api::log_in("code", &typed).await,
            };
            match outcome {
                Ok(()) => on_done.run(()),
                Err(e) => {
                    error.set(e);
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <div class="tcn-signin">
            <div class="tcn-signin-card">
                <div class="tcn-signin-logo">"🧭"</div>
                <h1 class="tcn-signin-title">"Tourcalc"</h1>
                <p class="tcn-login-lead">"Enter the access code for your tour to continue."</p>

                <Show when=move || !error.get().is_empty()>
                    <div class="tcn-errors">{move || error.get()}</div>
                </Show>

                <div class="tcn-field">
                    <div class="tcn-label">"Access code"</div>
                    <input class="tcn-input tcn-login-input" type="text" placeholder="your code"
                           autocomplete="off" autocapitalize="off" spellcheck="false"
                           prop:disabled=move || busy.get()
                           prop:value=move || code.get()
                           on:input=move |ev| code.set(event_target_value(&ev))
                           on:keyup=move |ev: web_sys::KeyboardEvent| {
                               if ev.key() == "Enter" {
                                   submit(());
                               }
                           } />
                </div>

                <button type="button" class="tcn-btn tcn-btn-primary tcn-btn-block"
                        prop:disabled=move || busy.get() || code.get().trim().is_empty()
                        on:click=move |_| submit(())>
                    {move || if busy.get() { "Logging in…" } else { "Log in" }}
                </button>

                <div class="tcn-hint" style="margin-top:12px">
                    "No code at hand? Opening a tour link signs you in by itself — ask
                     whoever shares the tour to send it again."
                </div>
            </div>
        </div>
    }
}
