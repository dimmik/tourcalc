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

/// What went wrong, in the words the screen will show.
pub type Failed = String;

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
/// while this screen was open, and the only honest answer is to reload and let the reader
/// see what changed.
pub async fn save_tour(tour: &Tour) -> Result<(), Failed> {
    let body = tour
        .to_json()
        .map_err(|e| format!("could not write the tour: {e}"))?;

    let mut req = Request::patch(&format!("/api/Tour/{}", tour.id));
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
        200 => Ok(()),
        409 => {
            Err("Somebody else changed this tour while it was open. Reload and try again.".into())
        }
        404 => Err("This tour is gone, or the login no longer covers it.".into()),
        s => {
            let detail = resp.text().await.unwrap_or_default();
            Err(format!("The server answered {s}. {detail}"))
        }
    }
}
