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

/// Forgets the token. Nothing on the server to tell: it never knew.
pub fn log_out() {
    if let Some(s) = storage() {
        let _ = s.remove_item(TOKEN_KEY);
    }
    crate::queue::forget_list();
}

/// What went wrong, in the words the screen will show.
pub type Failed = String;

/// Why a save did not happen - the caller has to tell these apart.
///
/// A conflict is resolvable: replay the pending operations on top of whatever is there now.
/// Being offline is not a failure at all, only a "later". Everything else is a real error.
#[derive(Debug)]
pub enum SaveError {
    Conflict,
    Offline(String),
    Other(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Conflict => f.write_str("somebody else changed this tour"),
            SaveError::Offline(e) => write!(f, "no connection: {e}"),
            SaveError::Other(e) => f.write_str(e),
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
        .map_err(|e| format!("could not reach the server: {e}"))?;
    match resp.status() {
        200 => {
            let token = resp
                .text()
                .await
                .map_err(|e| format!("could not read the token: {e}"))?;
            set_token(token.trim());
            Ok(())
        }
        401 => Err(if scope == "admin" {
            "That is not the master key.".to_owned()
        } else {
            "That code was not accepted.".to_owned()
        }),
        s => Err(format!("The server answered {s}")),
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
        .map_err(|e| format!("could not reach the server: {e}"))?;
    if !resp.ok() {
        return Err(format!("the server refused the code ({})", resp.status()));
    }
    // text/plain, not JSON - see the note in tc-server's auth endpoint.
    let token = resp
        .text()
        .await
        .map_err(|e| format!("could not read the token: {e}"))?;
    set_token(token.trim());
    Ok(())
}

/// Whether a failure was the network rather than the server.
pub fn looks_offline(message: &str) -> bool {
    message.contains("could not reach the server")
}

async fn get(url: &str) -> Result<String, Failed> {
    let mut req = Request::get(url);
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("could not reach the server: {e}"))?;
    match resp.status() {
        200 => resp
            .text()
            .await
            .map_err(|e| format!("could not read the answer: {e}")),
        404 => Err("no such tour, or the link is for a different access code".into()),
        s => Err(format!("the server answered {s}")),
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
    Tour::from_json(&body).map_err(|e| format!("could not read the tour: {e}"))
}

/// Every tour this token may see.
///
/// The list endpoint answers with the tours stripped of their spendings, so what comes back
/// is enough to name and count them and no more; the balances shown there are the ones the
/// server worked out.
pub async fn tours() -> Result<Vec<Tour>, Failed> {
    let body = get("/api/Tour/all/suggested?from=0&count=50").await?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("could not read the list: {e}"))?;
    let items = value
        .get("Tours")
        .and_then(|t| t.as_array())
        .ok_or("the list came back in an unexpected shape")?;
    Ok(items
        .iter()
        .filter_map(|v| Tour::from_json(&v.to_string()).ok())
        .collect())
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
        .map_err(|e| SaveError::Other(format!("could not write the tour: {e}")))?;

    let mut req = Request::patch(&format!("/api/Tour/{}", tour.id));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| SaveError::Other(format!("could not build the request: {e}")))?
        .send()
        .await
        // A failed fetch is what being offline looks like from here; the browser does not
        // say more than that, and it does not need to.
        .map_err(|e| SaveError::Offline(e.to_string()))?;

    match resp.status() {
        200 => Ok(()),
        409 => Err(SaveError::Conflict),
        404 => Err(SaveError::Other(
            "This tour is gone, or the login no longer covers it.".into(),
        )),
        s => {
            let detail = resp.text().await.unwrap_or_default();
            Err(SaveError::Other(format!(
                "The server answered {s}. {detail}"
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
        serde_json::from_str(&body).map_err(|e| format!("could not read the versions: {e}"))?;
    let items = value
        .get("Tours")
        .and_then(|t| t.as_array())
        .ok_or("the versions came back in an unexpected shape")?;
    Ok(items
        .iter()
        .filter_map(|v| Tour::from_json(&v.to_string()).ok())
        .collect())
}

/// Adds a whole tour, as JSON. What restoring a version and cloning both do.
pub async fn add_tour(body: serde_json::Value, code: &str) -> Result<String, Failed> {
    let mut req = Request::post(&format!(
        "/api/Tour/add/{}",
        if code.is_empty() { "-" } else { code }
    ));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("could not build the request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("could not reach the server: {e}"))?;

    match resp.status() {
        200 => resp
            .text()
            .await
            .map(|s| s.trim().to_owned())
            .map_err(|e| format!("could not read the answer: {e}")),
        403 => Err("Only an administrator can start the first tour under a code.".into()),
        s => Err(format!("The server answered {s}")),
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
        .map_err(|e| format!("could not reach the server: {e}"))?;
    match resp.status() {
        200 => Ok(()),
        403 => Err(
            "The last tour under an access code can only be deleted by an administrator.".into(),
        ),
        s => Err(format!("The server answered {s}")),
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
    let body = post_subscription("check", tour, sub).await?;
    Ok(body.trim() == "true")
}

pub async fn push_subscribe(tour: &str, sub: &PushSubscription) -> Result<(), Failed> {
    post_subscription("subscribe", tour, sub).await.map(|_| ())
}

pub async fn push_unsubscribe(tour: &str, sub: &PushSubscription) -> Result<(), Failed> {
    post_subscription("unsubscribe", tour, sub)
        .await
        .map(|_| ())
}

async fn post_subscription(
    what: &str,
    tour: &str,
    sub: &PushSubscription,
) -> Result<String, Failed> {
    let body = serde_json::to_string(sub).map_err(|e| format!("could not write it down: {e}"))?;

    let mut req = Request::post(&format!("/api/Subscription/{what}/{tour}"));
    if let Some(t) = token() {
        req = req.header("Authorization", &format!("bearer {t}"));
    }
    let resp = req
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| format!("could not build the request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("could not reach the server: {e}"))?;

    match resp.status() {
        200 => resp
            .text()
            .await
            .map_err(|e| format!("could not read the answer: {e}")),
        s => Err(format!("The server answered {s}")),
    }
}
