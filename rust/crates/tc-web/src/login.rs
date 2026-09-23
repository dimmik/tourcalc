//! Signing in.
//!
//! There are no user accounts. A tour belongs to an access code, and anybody who knows the
//! code sees the tours filed under it - so the code is what gets shared, and typing it is
//! the whole of logging in. `admin:` in front of the master key signs in as administrator.
//!
//! Deliberately a screen and not a dialog, for the reason the app learned the hard way: a
//! modal with nothing behind it and no way to dismiss is a trap, especially in a standalone
//! window with no address bar.

use crate::i18n::t;
use crate::api;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn SignIn(on_done: Callback<()>) -> impl IntoView {
    let code = RwSignal::new(String::new());
    // Here because a login ran out, not because nobody had signed in: say so, or a reader
    // who was reading a tour a moment ago is greeted like a stranger.
    let error = RwSignal::new(if api::take_expired() {
        "The login has expired. Enter the access code again — edits not sent yet are kept \
         and go out once you are signed in."
            .to_owned()
    } else {
        String::new()
    });
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
                    error.set(e.to_string());
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
                <p class="tcn-login-lead">{t().shell.login_lead}</p>

                <Show when=move || !error.get().is_empty()>
                    <div class="tcn-errors">{move || error.get()}</div>
                </Show>

                <div class="tcn-field">
                    <div class="tcn-label">{t().shell.access_code}</div>
                    <input class="tcn-input tcn-login-input" type="text" placeholder=t().shell.your_code
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
                    {move || if busy.get() { t().shell.logging_in } else { t().shell.log_in }}
                </button>

                <div class="tcn-hint" style="margin-top:12px">
                    {t().shell.no_code}
                </div>
            </div>
        </div>
    }
}
