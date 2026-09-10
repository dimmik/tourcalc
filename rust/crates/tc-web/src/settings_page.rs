//! The settings screen.
//!
//! Two settings, out of the app's eleven, and the two that change what a reader sees rather
//! than which interface they are in. The rest are accounted for on the page itself: it is
//! better to say why something is missing than to let somebody look for it.

use crate::accent;
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

    view! {
        <div class="tcn-section">
            <div class="tcn-section-title">"Settings"</div>

            <div class="tcn-card" style="padding: 14px;">
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">"Ignore debts smaller than"</div>
                        <div class="tcn-setdesc">
                            "Anything below this is treated as settled, so rounding leftovers
                             of a few coins stop showing up as debts. On a tour with several
                             currencies it is scaled, so the same amount of money counts as
                             noise whichever one you are reading in."
                        </div>
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
                        <div class="tcn-setname">"Accent colour"</div>
                        <div class="tcn-setdesc">
                            "The header, the buttons and the highlights. Everything else stays
                             the neutral grey it is now, so this only changes the accent, never
                             the text. The last sample opens the colour picker if none of the
                             seven is the one you want; a very light pick is darkened a little
                             so the white text on top of it stays readable."
                        </div>
                    </div>
                </div>
                <div class="tcn-accents" style="margin-top:10px" role="radiogroup"
                     aria-label="Accent colour">
                    {accent::PRESETS
                        .iter()
                        .map(|preset| {
                            let on = move || current_accent() == preset.id;
                            view! {
                                <button type="button" class="tcn-accent-dot" class:is-on=on
                                        role="radio" aria-checked=move || on().to_string()
                                        title=preset.label aria-label=preset.label
                                        style=format!("background: linear-gradient(120deg, {}, {})",
                                                      preset.from, preset.to)
                                        on:click=move |_| {
                                            accent::apply(preset.id);
                                            save(Box::new(move |s| s.accent = preset.id.to_owned()));
                                        }></button>
                            }
                        })
                        .collect_view()}
                    <span class="tcn-accent-wrap" title="Any other colour"
                          class:is-on=move || !accent::is_preset(&current_accent())>
                        <input type="color" class="tcn-accent-custom" role="radio"
                               aria-label="Any other colour"
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
                    <div class="tcn-hint" style="margin-top:10px">
                        "Saved in this browser. Settings live here and not on the server, so
                         they follow the device rather than the tour."
                    </div>
                </Show>
            </div>
        </div>

        <div class="tcn-section">
            <div class="tcn-section-title">"Elsewhere"</div>
            <div class="tcn-card" style="padding: 14px;">
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">"Interface"</div>
                        <div class="tcn-setdesc">
                            "Full is the roomy view; Mini draws it one line per thing. The
                             switch is in the header, next to the help."
                        </div>
                    </div>
                </div>
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">"Notifications"</div>
                        <div class="tcn-setdesc">
                            "A bell on each tour rather than one switch for all of them —
                             being told about the flat-share is not the same as being told
                             about last year's trip."
                        </div>
                    </div>
                </div>
                <div class="tcn-setrow">
                    <div class="tcn-settext">
                        <div class="tcn-setname">"Tap any number for details"</div>
                        <div class="tcn-setdesc">
                            "Always on here. It is what makes a figure checkable, and a
                             setting for it is a setting for “show me less”."
                        </div>
                    </div>
                </div>
            </div>
            <a class="tcn-card tcn-helplink" href="/help" style="margin-top:10px">
                <crate::icon::Icon name="help" />
                <div class="tcn-settext">
                    <div class="tcn-setname">"How Tourcalc counts"</div>
                    <div class="tcn-setdesc">
                        "What a weight is, why a total can be smaller than the sum of the
                         expenses, what “uncounted” means, and every other word on screen."
                    </div>
                </div>
            </a>
        </div>
    }
}
