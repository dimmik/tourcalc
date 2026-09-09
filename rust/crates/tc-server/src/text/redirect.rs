//! Sending a text browser to the text interface instead of the app it cannot run.
//!
//! A convenience, not the mechanism. The honest signal is the `<noscript>` block in the
//! page itself, which keys off the missing capability and so covers every browser with
//! scripting off, including ones nobody has heard of. Matching on the user agent only
//! recognises names it has been told about, and such a list ages - which is why the list is
//! configuration rather than code, and why it is short.

/// Where a text browser asking for this path should be sent, if anywhere.
///
/// Only `GET`, and only paths the app would have handled: `/api` and `/_` are somebody
/// else's, and `/t` is already there.
pub fn target_for(method: &str, path: &str, user_agent: &str, agents: &[String]) -> Option<String> {
    if !method.eq_ignore_ascii_case("GET") {
        return None;
    }
    let agent = user_agent.to_lowercase();
    if !agents.iter().any(|a| agent.contains(&a.to_lowercase())) {
        return None;
    }
    if path == "/t" || path.starts_with("/t/") || path.starts_with("/api") || path.starts_with("/_")
    {
        return None;
    }

    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    Some(match parts.as_slice() {
        [] | ["tourlist"] => "/t".to_owned(),
        // The share link carries the sign-in, and the text pages take the same shape.
        ["goto", rest @ ..] if !rest.is_empty() => format!("/t/goto/{}", rest.join("/")),
        // The app's per-section routes have text counterparts, so a link into one lands on
        // the matching page rather than at the top.
        ["tour", id, section @ ..] => {
            let suffix = match section.first() {
                Some(&"persons") => "/people",
                Some(&"spendings") => "/spend",
                _ => "",
            };
            format!("/t/{id}{suffix}")
        }
        // Anything else the app would have handled: the tour list is the way in.
        _ => "/t".to_owned(),
    })
}

/// The agent names to look for, from the configured list.
///
/// "elinks" is deliberately not among the defaults: "links" is a substring of it, so the
/// shorter name already catches ELinks.
pub fn parse_agents(configured: &str) -> Vec<String> {
    configured
        .split([';', ','])
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(|a| a.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agents() -> Vec<String> {
        parse_agents("lynx;w3m;links")
    }

    #[test]
    fn a_text_browser_is_sent_to_the_text_pages() {
        let go = |path: &str| target_for("GET", path, "Lynx/2.9.0", &agents());
        assert_eq!(go("/"), Some("/t".to_owned()));
        assert_eq!(go("/tourlist"), Some("/t".to_owned()));
        assert_eq!(go("/tour/abc"), Some("/t/abc".to_owned()));
        assert_eq!(go("/tour/abc/persons"), Some("/t/abc/people".to_owned()));
        assert_eq!(go("/tour/abc/spendings"), Some("/t/abc/spend".to_owned()));
        assert_eq!(go("/goto/CODE/abc"), Some("/t/goto/CODE/abc".to_owned()));
    }

    #[test]
    fn everybody_else_is_left_alone() {
        let ua = "Mozilla/5.0 (X11) Firefox/140.0";
        assert_eq!(target_for("GET", "/", ua, &agents()), None);
        // ELinks is caught by "links" without a rule of its own.
        assert!(target_for("GET", "/", "ELinks/0.17", &agents()).is_some());
    }

    #[test]
    fn what_is_not_the_apps_is_not_touched() {
        let ua = "w3m/0.5.3";
        for path in ["/t", "/t/abc", "/api/Tour/abc", "/_framework/blazor.js"] {
            assert_eq!(target_for("GET", path, ua, &agents()), None, "{path}");
        }
        // A form post is never redirected: it would arrive as a GET and lose the body.
        assert_eq!(target_for("POST", "/", ua, &agents()), None);
    }
}
