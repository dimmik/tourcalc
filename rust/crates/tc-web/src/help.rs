//! The help page.
//!
//! The app's own document, taken as it is - the same thing that happened with the two
//! stylesheets, and for the same reason. It is prose about what the buttons do, written
//! against what the app actually does rather than what it was meant to do, and rewriting it
//! from memory would have produced something less true and no shorter.
//!
//! One file per language (`help.html`, `help.ru.html`), picked by `i18n::lang`: prose is
//! translated as prose, not assembled from fields.
//!
//! Static markup, so it is included rather than built out of view macros: 250 lines of
//! `<section>` and `<p>` expressed as Rust would be harder to compare with the original,
//! which is the thing that has to stay true when either side changes.

use leptos::prelude::*;

#[component]
pub fn HelpPage() -> impl IntoView {
    view! {
        <div class="tcn-main" inner_html=match crate::i18n::lang() {
            crate::i18n::Lang::En => include_str!("help.html"),
            crate::i18n::Lang::Ru => include_str!("help.ru.html"),
        }></div>
        // Where the app kept its build date. The question it was there to answer is asked
        // properly here instead - see `crate::version`.
        <crate::version::AboutBuild />
        // What changed and why, newest first. Folded: it is for the reader who wonders
        // why something looks different today, not for everybody who opens Help. Kept as
        // markup for the same reason as the page above, and added to with every change a
        // reader could notice - see CLAUDE.md.
        <div class="tcn-main" inner_html=with_latest(include_str!("changes.html"))></div>
    }
}

/// The change log with its newest entry shown on its own above the fold, so that "what is
/// new" is there without opening the log. Taken from the log itself - the first entry under
/// the first date, which is the newest because entries go on top - so there is nothing to
/// write twice and nothing to fall out of step.
fn with_latest(log: &str) -> String {
    match latest(log) {
        Some((day, entry)) => log.replacen(
            "<details",
            &format!(
                "<div class=\"tcw-latest\"><div class=\"tcw-latest-head\">Latest change · {day}</div>\
                 <div>{entry}</div></div><details"
            ),
            1,
        ),
        None => log.to_owned(),
    }
}

/// The first date heading and the markup of the first entry under it.
fn latest(log: &str) -> Option<(&str, &str)> {
    let day_at = log.find("<h3>")? + "<h3>".len();
    let day = &log[day_at..day_at + log[day_at..].find("</h3>")?];
    let entry_at = day_at + log[day_at..].find("<li>")? + "<li>".len();
    let entry = &log[entry_at..entry_at + log[entry_at..].find("</li>")?];
    Some((day.trim(), entry.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_entry_is_the_first_under_the_first_date() {
        let log = "<details><summary>Change log</summary>\n<h3>27 September 2026</h3>\n<ul>\n    \
                   <li><b>New.</b> Newest.</li>\n    <li><b>Older.</b></li>\n</ul>\n\
                   <h3>26 September 2026</h3>\n<ul><li>Oldest.</li></ul></details>";
        assert_eq!(latest(log), Some(("27 September 2026", "<b>New.</b> Newest.")));
        let shown = with_latest(log);
        assert!(shown.starts_with("<div class=\"tcw-latest\">"), "above the fold: {shown}");
        assert!(shown.contains("Latest change · 27 September 2026"));
        assert_eq!(shown.matches("<details").count(), 1);
    }

    /// The real log parses, and its newest entry is a bold title and its why.
    #[test]
    fn the_real_log_has_a_latest_entry() {
        let (day, entry) = latest(include_str!("changes.html")).expect("a dated entry");
        assert!(day.ends_with("2026"), "{day}");
        assert!(entry.starts_with("<b>"), "{entry}");
    }
}
