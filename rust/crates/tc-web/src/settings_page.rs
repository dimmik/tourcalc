//! The settings screen.
//!
//! Two settings, out of the app's eleven, and the two that change what a reader sees rather
//! than which interface they are in - and one of this client's own, about how often an open
//! tour checks for other people's changes. The rest are accounted for on the page itself: it is
//! better to say why something is missing than to let somebody look for it.

use crate::accent;
use crate::i18n::{self, t, Lang};
use crate::settings::{self, Settings};
use leptos::prelude::*;

#[component]
pub fn SettingsPage(settings: settings::Shared) -> impl IntoView {
    let saved = RwSignal::new(false);

    // There is no Save button in the app either: a setting is written the moment it is
    // settled - the number when the box is left, the colour when the picker closes. Not on
    // every keystroke, which would file "1" and "10" on the way to 100, and not on every
    // frame of a drag through the colour wheel.
    let save = move |change: Box<dyn FnOnce(&mut Settings)>| {
        settings.update(|s| {
            change(s);
            settings::remember(s);
        });
        saved.set(true);
    };

    let current_accent = move || settings.get().accent;
    let check = RwSignal::new(settings::check_seconds());
    let tx = &t().settings;
    let chosen_lang = i18n::chosen();

    view! {
        <div class="tcn-section">
            <div class="tcn-section-title">{tx.title}</div>

            <div class="tcn-card" style="padding: 14px;">
                // First, because somebody who cannot read the rest is looking for exactly this.
                // The languages are named in themselves, so it can be found either way.
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">
                            {tx.language}
                            // English beside it, for somebody who landed here in a language
                            // they cannot read - and only then: in English it said
                            // "Language · Language".
                            {(i18n::lang() != Lang::En).then_some(" · Language")}
                        </div>
                        <SetDesc short=tx.language_short full=tx.language_desc />
                    </div>
                    <select class="tcn-input" id="language" style="width:auto; flex:0 0 auto"
                            aria-label="Language"
                            on:change=move |ev| {
                                i18n::choose(Lang::from_code(&event_target_value(&ev)));
                            }>
                        <option value="" selected=chosen_lang.is_none()>
                            {(tx.language_auto)(i18n::from_browser().name())}
                        </option>
                        {Lang::ALL
                            .into_iter()
                            .map(|l| view! {
                                <option value=l.code() selected=chosen_lang == Some(l)>{l.name()}</option>
                            })
                            .collect_view()}
                    </select>
                </div>

                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.min_debt}</div>
                        <SetDesc short=tx.min_debt_short full=tx.min_debt_desc />
                    </div>
                    <input class="tcn-input tcn-setnum" type="number" min="0"
                           prop:value=move || settings.get().minimum_meaningful_debt.to_string()
                           on:change=move |ev| {
                               let text = event_target_value(&ev);
                               // An empty box is nought, not a reason to keep the old number:
                               // clearing it is how somebody says "show me everything".
                               let value = text.trim().parse::<i64>().unwrap_or(0).max(0);
                               save(Box::new(move |s| s.minimum_meaningful_debt = value));
                           } />
                </div>

                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.accent}</div>
                        <SetDesc short=tx.accent_short full=tx.accent_desc />
                    </div>
                </div>
                <div class="tcn-accents" style="margin-top:10px" role="radiogroup"
                     aria-label=tx.accent>
                    {accent::PRESETS
                        .iter()
                        .zip(tx.accent_names)
                        .map(|(preset, label)| {
                            let on = move || current_accent() == preset.id;
                            view! {
                                <button type="button" class="tcn-accent-dot" class:is-on=on
                                        role="radio" aria-checked=move || on().to_string()
                                        title=label aria-label=label
                                        style=format!("background: linear-gradient(120deg, {}, {})",
                                                      preset.from, preset.to)
                                        on:click=move |_| {
                                            accent::apply(preset.id);
                                            save(Box::new(move |s| s.accent = preset.id.to_owned()));
                                        }></button>
                            }
                        })
                        .collect_view()}
                    <span class="tcn-accent-wrap" title=tx.accent_other
                          class:is-on=move || !accent::is_preset(&current_accent())>
                        <input type="color" class="tcn-accent-custom" role="radio"
                               aria-label=tx.accent_other
                               aria-checked=move || (!accent::is_preset(&current_accent())).to_string()
                               prop:value=move || accent::swatch(&current_accent())
                               // Dragging through the wheel repaints the page and writes
                               // nothing, so backing out of the picker leaves the setting
                               // alone; closing it is what saves.
                               on:input=move |ev| accent::apply(&event_target_value(&ev))
                               on:change=move |ev| {
                                   let hex = event_target_value(&ev);
                                   accent::apply(&hex);
                                   save(Box::new(move |s| s.accent = hex.clone()));
                               } />
                    </span>
                </div>

                <Show when=move || saved.get()>
                    <div class="tcn-hint" style="margin-top:10px">{tx.saved}</div>
                </Show>
            </div>
        </div>

        <div class="tcn-section">
            <div class="tcn-section-title">{tx.device}</div>
            <div class="tcn-card" style="padding: 14px;">
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.check}</div>
                        <SetDesc short=tx.check_short full=tx.check_desc />
                    </div>
                    <select class="tcn-input" id="check-seconds" style="width:auto; flex:0 0 auto"
                            aria-label=tx.check
                            on:change=move |ev| {
                                let seconds = event_target_value(&ev)
                                    .parse::<u32>()
                                    .unwrap_or(settings::CHECK_DEFAULT);
                                settings::remember_check_seconds(seconds);
                                check.set(seconds);
                                saved.set(true);
                            }>
                        {settings::CHECK_CHOICES
                            .iter()
                            .map(|&n| {
                                let label = match n {
                                    0 => tx.check_on_return.to_owned(),
                                    60 => tx.check_minute.to_owned(),
                                    n => (tx.check_seconds)(n),
                                };
                                view! {
                                    <option value=n.to_string()
                                            selected=move || check.get() == n>{label}</option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <crate::install::InstallSetting />
            </div>
        </div>

        <div class="tcn-section">
            <div class="tcn-section-title">{tx.elsewhere}</div>
            <div class="tcn-card" style="padding: 14px;">
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.interface}</div>
                        <div class="tcn-setdesc">{tx.interface_desc}</div>
                    </div>
                </div>
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.notifications}</div>
                        <div class="tcn-setdesc">{tx.notifications_desc}</div>
                    </div>
                </div>
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">{tx.explain}</div>
                        <div class="tcn-setdesc">{tx.explain_desc}</div>
                    </div>
                </div>
            </div>
            <a class="tcn-card tcn-helplink" href="/help" style="margin-top:10px">
                <crate::icon::Icon name="help" />
                <div class="tcn-settext">
                    <div class="tcn-setname">{tx.how_it_counts}</div>
                    <div class="tcn-setdesc">{tx.how_it_counts_desc}</div>
                </div>
            </a>
        </div>
    }
}

/// A setting's description on a phone-sized budget: one sentence, and the rest behind
/// "more". The whole paragraph at once made each row ten lines tall on a narrow screen,
/// which is where settings are mostly changed. Opening it swaps the sentence for the full
/// text rather than adding to it - the full text begins by saying the same thing - and
/// "less" at its end folds it back.
///
/// A button and a signal rather than `<details>`: a summary has to come first, so once the
/// text was open the only way to fold it was to find the line it had started on - and the
/// first version hid that line, so it could not be folded at all.
#[component]
pub fn SetDesc(short: &'static str, full: &'static str) -> impl IntoView {
    let open = RwSignal::new(false);
    let tx = &crate::i18n::t().settings;
    view! {
        <div class="tcn-setdesc">
            {move || if open.get() { full } else { short }}
            " "
            <button type="button" class="tcw-more" aria-expanded=move || open.get().to_string()
                    on:click=move |_| open.update(|o| *o = !*o)>
                {move || if open.get() { tx.less } else { tx.more }}
            </button>
        </div>
    }
}
