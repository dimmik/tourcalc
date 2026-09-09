//! Turning things into HTML, by hand.
//!
//! No template engine: these pages are small, there are ten of them, and a dependency that
//! generates HTML from a template language would be larger than the interface it renders.
//! What matters is that [`esc`] is used on everything that came from a person, and that is
//! easier to see in a function that builds a string than in a template.

use tc_core::Cents;

/// Text that will not be read as markup.
///
/// Tour names, people's names and descriptions are all typed by somebody, and a person who
/// calls themselves `<script>` should appear on the page under that name rather than run.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Money the way the text pages write it: grouped by an ordinary space.
///
/// Not the narrow no-break space the app uses. lynx renders that as a question mark in a
/// terminal that is not certain of its own encoding, and a number reading "1?234" is worse
/// than one that might wrap.
pub fn money(amount: Cents) -> String {
    let digits = amount.0.unsigned_abs().to_string();
    let mut out = String::new();
    if amount.0 < 0 {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

/// Caps a cell so a table still fits an eighty-column terminal.
///
/// lynx sizes each column to its widest cell and, when the total will not fit, abandons the
/// table and spills the cells across the line - taking every other row's alignment with it.
/// One very long name was enough to do that to the expense list in the C#, so the columns
/// that carry names are bounded.
pub fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1).max(1)).collect();
    format!("{kept}…")
}

/// The shell every page is drawn in.
pub fn page(title: &str, body: &str) -> axum::response::Html<String> {
    axum::response::Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title} · Tourcalc text</title>
<style>
body {{ font-family: monospace, monospace; margin: 1em auto; max-width: 60em; padding: 0 1em; }}
table {{ border-collapse: collapse; width: 100%; margin-bottom: 1em; }}
th, td {{ text-align: left; padding: 2px 8px 2px 0; vertical-align: top; }}
th {{ border-bottom: 1px solid #999; }}
td.n, th.n {{ text-align: right; white-space: nowrap; }}
nav a {{ margin-right: 1em; }}
.muted {{ color: #666; }}
hr {{ border: 0; border-top: 1px solid #ccc; }}
</style>
</head>
<body>
<p class="muted">Tourcalc &mdash; text interface. <a href="/">Full app</a></p>
<hr />
{body}
<hr />
<p class="muted"><a href="/t">Tours</a> <a href="/t/logout">Log out</a></p>
</body>
</html>"#,
        title = esc(title),
    ))
}
