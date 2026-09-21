//! Talking to the server, and remembering who we are.
//!
//! The same endpoints the Blazor client calls, because the server has to keep answering
//! both while the port is unfinished.

use gloo_net::http::Request;
use tc_core::Tour;

/// Where the token is kept between visits.
///
/// The Blazor client keeps its token in a cookie; this one uses localStorage, which is
/// simpler and is not sent with every request for a stylesheet. Nothing else reads it, so
/// the two can differ - but note that they therefore do **not** share a login: opening the
/// Blazor UI after this one asks for the code again. That is fine while both exist and is
/// worth remembering when they are compared side by side.
const TOKEN_KEY: &str = "__tcw_token";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

pub fn token() -> Option<String> {
    storage()?
        .get_item(TOKEN_KEY)
        .ok()
        .flatten()
        .filter(|t| !t.is_empty())
}

pub fn set_token(token: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(TOKEN_KEY, token);
    }
}

/// Whether anybody is signed in on this device.
pub fn signed_in() -> bool {
    token().is_some()
}

thread_local! {
    /// The app's "somebody is signed in", so that a request finding the login gone can say
    /// so to the screen. Set once by the app; absent in a test.
    static SIGNED_IN: std::cell::Cell<Option<leptos::prelude::RwSignal<bool>>> =
        const { std::cell::Cell::new(None) };
    /// Whether the sign-in screen is up because a login ran out, rather than because
    /// nobody had signed in.
    static EXPIRED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Hands this module the app's sign-in signal. Called once, by the app.
pub fn reports_sign_in_on(signal: leptos::prelude::RwSignal<bool>) {
    SIGNED_IN.with(|s| s.set(Some(signal)));
}

/// The server has refused the token - it expired, or the server's key changed. Forgets it,
/// puts the sign-in screen up, and answers with the words to show.
///
/// Only the token goes. What is waiting to be sent stays: typing the same code again picks
/// up where the reader left off, and the edits go out then.
pub fn expired() -> String {
    use leptos::prelude::*;
    if let Some(s) = storage() {
        let _ = s.remove_item(TOKEN_KEY);
    }
    EXPIRED.with(|e| e.set(true));
    if let Some(signal) = SIGNED_IN.with(|s| s.get()) {
        signal.try_set(false);
    }
    "The login has expired. Enter the access code again.".into()
}

/// Whether the last sign-out was a login running out. Asked once, by the sign-in screen,
/// which then says so instead of greeting a stranger.
pub fn take_expired() -> bool {
    EXPIRED.with(|e| e.replace(false))
}

/// Forgets the token, and everything this device kept about the tours it opened. Nothing on
/// the server to tell: it never knew.
///
/// Deliberately more than [`expired`] does. Leaving on purpose is leaving a device; a login
/// that simply ran out is the same person, and their unsent edits wait for them.
pub fn log_out() {
    if let Some(s) = storage() {
        let _ = s.remove_item(TOKEN_KEY);
    }
    crate::queue::forget_everything();
    crate::push::forget_bells();
}

/// Whose fault it is that a request came to nothing.
///
/// "The server did not answer" and "the server answered, and the answer was no" are
/// different facts, and the screen used to tell both of them the same way - so a tour that
/// was being refused with a 409 read as a tour nobody could reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    /// Nothing came back at all: no network, a server that is down, a proxy that dropped
    /// the request. Trying again later is the only answer.
    Unreachable,
    /// The server answered with an error. 4xx is the server saying no to this request -
    /// trying the same thing again will get the same answer; 5xx is the server in trouble,
    /// which is not about the request at all.
    Answered(u16),
    /// The answer arrived and this client could not make sense of it. Ours to fix.
    Ours,
}

impl Trouble {
    pub fn unreachable(self) -> bool {
        self == Trouble::Unreachable
    }

    /// Whether the server refused this request: 4xx, and nothing to be gained by repeating
    /// it unchanged.
    pub fn refusal(self) -> Option<u16> {
        match self {
            Trouble::Answered(code) if (400..500).contains(&code) => Some(code),
            _ => None,
        }
    }

    /// Whether the server broke: 5xx, which says nothing about the request.
    pub fn broke(self) -> Option<u16> {
        match self {
            Trouble::Answered(code) if code >= 500 => Some(code),
            _ => None,
        }
    }

    /// The short of it, for a line with no room: "no answer", "refused 409", "server 500".
    pub fn shortly(self) -> String {
        match self {
            Trouble::Unreachable => "no answer".to_owned(),
            Trouble::Ours => "unreadable answer".to_owned(),
            Trouble::Answered(code) if code >= 500 => format!("server {code}"),
            Trouble::Answered(code) => format!("refused {code}"),
        }
    }
}

/// What went wrong: whose fault, and the words the screen will show.
#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub why: Trouble,
    pub said: String,
}

impl Failed {
    pub fn unreachable(said: impl Into<String>) -> Failed {
        Failed { why: Trouble::Unreachable, said: said.into() }
    }

    pub fn answered(status: u16, said: impl Into<String>) -> Failed {
        Failed { why: Trouble::Answered(status), said: said.into() }
    }

    /// The answer came and we could not read it.
    pub fn ours(said: impl Into<String>) -> Failed {
        Failed { why: Trouble::Ours, said: said.into() }
    }
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.said)
    }
}

/// Why a save did not happen - the caller has to tell these apart.
///
/// A conflict is resolvable: replay the pending operations on top of whatever is there now.
/// Being offline is not a failure at all, only a "later". Everything else is a real error.
#[derive(Debug)]
pub enum SaveError {
    Conflict,
    Offline(String),
    Other(Failed),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Conflict => f.write_str("somebody else changed this tour"),
            SaveError::Offline(e) => write!(f, "no connection: {e}"),
            SaveError::Other(e) => f.write_str(&e.said),
        }
    }
}

/// Exchanges a code somebody typed for a token.
///
/// `scope` is "code" for an access code and "admin" for the master key - the same two the
/// C# server accepts, because it is the same endpoint.
pub async fn log_in(scope: &str, key: &str) -> Result<(), Failed> {
    let url = format!("/api/Auth/token/{scope}/{}", urlencode(key));
    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;
    match resp.status() {
        200 => {
            let token = resp
                .text()
                .await
                .map_err(|e| Failed::ours(format!("could not read the token: {e}")))?;
            set_token(token.trim());
            Ok(())
        }
        401 => Err(Failed::answered(
            401,
            if scope == "admin" {
                "That is not the master key."
            } else {
                "That code was not accepted."
            },
        )),
        s => Err(Failed::answered(s, format!("The server answered {s}"))),
    }
}

/// Percent-encodes what has to survive being a path segment.
///
/// Codes are typed by people and can hold anything; a slash would otherwise become part of
/// the route and the server would see a different request entirely.
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Exchanges an access code - already hashed, as it comes in a share link - for a token.
pub async fn log_in_with_md5(code_md5: &str) -> Result<(), Failed> {
    let url = format!("/api/Auth/token/code/{code_md5}/md5");
    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;
    if !resp.ok() {
        let s = resp.status();
        return Err(Failed::answered(s, format!("the server refused the code ({s})")));
    }
    // text/plain, not JSON - see the note in tc-server's auth endpoint.
    let token = resp
        .text()
        .await
        .map_err(|e| Failed::ours(format!("could not read the token: {e}")))?;
    set_token(token.trim());
    Ok(())
}

/// What a read says when the server has no such tour for this login.
pub const NOT_FOUND: &str = "no such tour, or the link is for a different access code";

/// Whether a failure was the network rather than the server.
pub fn looks_offline(failed: &Failed) -> bool {
    failed.why.unreachable()
}

async fn get(url: &str) -> Result<String, Failed> {
    let mut req = Request::get(url);
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;
    let status = resp.status();
    match status {
        200 => resp
            .text()
            .await
            .map_err(|e| Failed::ours(format!("could not read the answer: {e}"))),
        404 => Err(Failed::answered(404, NOT_FOUND)),
        401 => Err(Failed::answered(401, expired())),
        s => Err(Failed::answered(s, format!("the server answered {s}"))),
    }
}

/// Which build the server is, and which client it hands out. Raw text: the caller parses
/// it, and a version check that fails to parse should not be a broken screen.
pub async fn info_version() -> Result<String, Failed> {
    get("/api/Info/version").await
}

/// One tour, in full.
pub async fn tour(id: &str) -> Result<Tour, Failed> {
    let body = get(&format!("/api/Tour/{id}")).await?;
    // Parsed by tc-core - the same code the server used to write it, and the same code that
    // will do the arithmetic on it in a moment.
    Tour::from_json(&body).map_err(|e| Failed::ours(format!("could not read the tour: {e}")))
}

/// The tour's `StateGUID` alone - which changes on every save, so it is how an open page
/// finds out that somebody else saved without fetching the tour to compare.
pub async fn tour_state(id: &str) -> Result<String, Failed> {
    get(&format!("/api/Tour/{id}/state"))
        .await
        .map(|s| s.trim().to_owned())
}

/// Every tour this token may see.
///
/// The list endpoint answers with the tours stripped of their spendings, so what comes back
/// is enough to name and count them and no more; the balances shown there are the ones the
/// server worked out.
pub async fn tours() -> Result<Vec<Tour>, Failed> {
    // A page at a time, until the server says that was all. One page of fifty used to be
    // the whole list, and an administrator with more saw the first fifty and no word that
    // anything was missing.
    const PAGE: usize = 200;
    let mut all = Vec::new();
    // What the server has handed over, which is not the same as what could be read: a tour
    // this client cannot parse still takes up a place in the server's list, and counting by
    // the parsed ones would ask for the next page one short and show a tour twice.
    let mut handed_over = 0usize;
    loop {
        let body = get(&format!(
            "/api/Tour/all/suggested?from={handed_over}&count={PAGE}"
        ))
        .await?;
        let value: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| Failed::ours(format!("could not read the list: {e}")))?;
        let items = value
            .get("Tours")
            .and_then(|t| t.as_array())
            .ok_or_else(|| Failed::ours("the list came back in an unexpected shape"))?;
        let total = value
            .get("TotalCount")
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as usize;
        handed_over += items.len();
        all.extend(
            items
                .iter()
                .filter_map(|v| Tour::from_json(&v.to_string()).ok()),
        );
        // An empty page ends it too, whatever the total says: a list that shrank while it
        // was being read must not be asked for forever.
        if items.is_empty() || handed_over >= total {
            return Ok(all);
        }
    }
}

/// Sends the whole tour back.
///
/// The soft lock lives in the tour's own `StateGUID`, which travels with it: the server
/// compares what arrives against what it holds and refuses a save built on a stale read.
/// A 409 is therefore not a failure to report and forget - it means somebody else wrote
/// while this one was reading, and `sync` answers it by replaying the queued operations on
/// top of what they saved.
pub async fn save_tour(tour: &Tour) -> Result<(), SaveError> {
    let body = tour
        .to_json()
        .map_err(|e| SaveError::Other(Failed::ours(format!("could not write the tour: {e}"))))?;

    let mut req = Request::patch(&format!("/api/Tour/{}", tour.id));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| SaveError::Other(Failed::ours(format!("could not build the request: {e}"))))?
        .send()
        .await
        // A failed fetch is what being offline looks like from here; the browser does not
        // say more than that, and it does not need to.
        .map_err(|e| SaveError::Offline(e.to_string()))?;

    match resp.status() {
        200 => Ok(()),
        409 => Err(SaveError::Conflict),
        401 => Err(SaveError::Other(Failed::answered(401, expired()))),
        404 => Err(SaveError::Other(Failed::answered(
            404,
            "This tour is gone, or the login no longer covers it.",
        ))),
        s => {
            let detail = resp.text().await.unwrap_or_default();
            Err(SaveError::Other(Failed::answered(
                s,
                format!("The server answered {s}. {detail}"),
            )))
        }
    }
}

/// The versions of a tour: what it used to be, newest first.
///
/// Without their contents - the server strips those, because the list shows a date and a
/// line saying what changed. Restoring one asks for it by its own id.
pub async fn versions(id: &str) -> Result<Vec<Tour>, Failed> {
    let body = get(&format!("/api/Tour/{id}/versions")).await?;
    let value: serde_json::Value =
        serde_json::from_str(&body)
            .map_err(|e| Failed::ours(format!("could not read the versions: {e}")))?;
    let items = value
        .get("Tours")
        .and_then(|t| t.as_array())
        .ok_or_else(|| Failed::ours("the versions came back in an unexpected shape"))?;
    Ok(items
        .iter()
        .filter_map(|v| Tour::from_json(&v.to_string()).ok())
        .collect())
}

/// Which access code a new tour is filed under.
///
/// Only an administrator's request is read for it - everybody else's tours go under their
/// own code whatever the address says - but an administrator's has to name one, and it
/// matters which form it is in.
pub enum Pile<'a> {
    /// A code somebody typed: the server hashes it.
    Typed(&'a str),
    /// A tour's own `AccessCodeMD5`, for a copy that goes next to it. Hashing that again
    /// used to file the copy under md5(md5), where nobody would ever see it.
    Hashed(&'a str),
}

/// Adds a whole tour, as JSON. What creating, cloning and restoring a version all do.
pub async fn add_tour(body: serde_json::Value, pile: Pile<'_>) -> Result<String, Failed> {
    // "-" stands in for "none": a reader's request is not read for it, and an administrator
    // who names none is told so.
    let url = match pile {
        Pile::Typed(code) if code.trim().is_empty() => "/api/Tour/add/-".to_owned(),
        Pile::Typed(code) => format!("/api/Tour/add/{}", urlencode(code.trim())),
        Pile::Hashed(md5) if md5.trim().is_empty() => "/api/Tour/add/-".to_owned(),
        Pile::Hashed(md5) => format!("/api/Tour/add/{}/md5", urlencode(md5.trim())),
    };
    let mut req = Request::post(&url);
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| Failed::ours(format!("could not build the request: {e}")))?
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;

    match resp.status() {
        200 => resp
            .text()
            .await
            .map(|s| s.trim().to_owned())
            .map_err(|e| Failed::ours(format!("could not read the answer: {e}"))),
        401 => Err(Failed::answered(401, expired())),
        403 => Err(Failed::answered(
            403,
            "Only an administrator can start the first tour under a code.",
        )),
        400 => Err(Failed::answered(
            400,
            resp.text().await.unwrap_or_else(|_| "The server answered 400".into()),
        )),
        s => Err(Failed::answered(s, format!("The server answered {s}"))),
    }
}

/// Removes a tour altogether.
pub async fn delete_tour(id: &str) -> Result<(), Failed> {
    let mut req = Request::delete(&format!("/api/Tour/{id}"));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;
    match resp.status() {
        200 => Ok(()),
        401 => Err(Failed::answered(401, expired())),
        403 => Err(Failed::answered(
            403,
            "The last tour under an access code can only be deleted by an administrator.",
        )),
        s => Err(Failed::answered(s, format!("The server answered {s}"))),
    }
}

// --- push notifications -------------------------------------------------------------------

/// What a browser hands over when it subscribes: an endpoint at a push service and the two
/// keys a message for it has to be encrypted with. The server's field names.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PushSubscription {
    #[serde(rename = "Url")]
    pub url: String,
    #[serde(rename = "P256dh")]
    pub p256dh: String,
    #[serde(rename = "Auth")]
    pub auth: String,
}

/// The server's VAPID public key. Public, and meant to be: a browser needs it to subscribe.
pub async fn push_public_key() -> Result<String, Failed> {
    get("/api/Subscription/publickey")
        .await
        .map(|s| s.trim().to_owned())
}

pub async fn push_check(tour: &str, sub: &PushSubscription) -> Result<bool, Failed> {
    let body = post_subscription(&format!("check/{tour}"), sub).await?;
    Ok(body.trim() == "true")
}

pub async fn push_subscribe(tour: &str, sub: &PushSubscription) -> Result<(), Failed> {
    post_subscription(&format!("subscribe/{tour}"), sub)
        .await
        .map(|_| ())
}

pub async fn push_unsubscribe(tour: &str, sub: &PushSubscription) -> Result<(), Failed> {
    post_subscription(&format!("unsubscribe/{tour}"), sub)
        .await
        .map(|_| ())
}

/// The reader's tours this browser is subscribed to, in one request for the whole list.
pub async fn push_mine(sub: &PushSubscription) -> Result<Vec<String>, Failed> {
    let body = post_subscription("mine", sub).await?;
    serde_json::from_str(&body).map_err(|e| Failed::ours(format!("could not read the answer: {e}")))
}

async fn post_subscription(what: &str, sub: &PushSubscription) -> Result<String, Failed> {
    let body = serde_json::to_string(sub)
        .map_err(|e| Failed::ours(format!("could not write it down: {e}")))?;

    let mut req = Request::post(&format!("/api/Subscription/{what}"));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| Failed::ours(format!("could not build the request: {e}")))?
        .send()
        .await
        .map_err(|e| Failed::unreachable(format!("could not reach the server: {e}")))?;

    match resp.status() {
        200 => resp
            .text()
            .await
            .map_err(|e| Failed::ours(format!("could not read the answer: {e}"))),
        401 => Err(Failed::answered(401, expired())),
        s => Err(Failed::answered(s, format!("The server answered {s}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three kinds are told apart by what the caller can do about them.
    #[test]
    fn who_is_at_fault_is_part_of_the_failure() {
        let network = Failed::unreachable("could not reach the server: …");
        assert!(network.why.unreachable());
        assert!(looks_offline(&network));
        assert_eq!(network.why.refusal(), None);
        assert_eq!(network.why.broke(), None);

        let refused = Failed::answered(409, "somebody else changed this tour");
        assert!(!looks_offline(&refused), "a 409 is an answer, not a silence");
        assert_eq!(refused.why.refusal(), Some(409));
        assert_eq!(refused.why.broke(), None);

        let gone = Failed::answered(404, NOT_FOUND);
        assert_eq!(gone.why.refusal(), Some(404));

        let broken = Failed::answered(502, "The server answered 502");
        assert_eq!(broken.why.broke(), Some(502));
        assert_eq!(broken.why.refusal(), None);
        assert!(!looks_offline(&broken), "a gateway error is not being offline");

        let ours = Failed::ours("could not read the tour: …");
        assert!(!looks_offline(&ours));
        assert_eq!(ours.why.refusal(), None);
    }

    /// What is shown is what the failure said, wherever it is printed.
    #[test]
    fn a_failure_prints_what_it_said() {
        assert_eq!(
            Failed::answered(409, "somebody else changed this tour").to_string(),
            "somebody else changed this tour"
        );
        assert_eq!(Trouble::Unreachable.shortly(), "no answer");
        assert_eq!(Trouble::Answered(409).shortly(), "refused 409");
        assert_eq!(Trouble::Answered(503).shortly(), "server 503");
    }
}
