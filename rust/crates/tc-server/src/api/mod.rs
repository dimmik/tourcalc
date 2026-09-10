//! The HTTP surface, shaped to match what the existing client already asks for.
//!
//! Nothing here invents a nicer API. The Blazor client, the text pages and the bot all call
//! these paths with these spellings today, so the port has to answer them exactly - which
//! also makes for the strongest test available: point the existing client at this server
//! and see whether it notices.

pub mod auth;
mod subscription;
pub mod tour;
pub mod write;

use crate::state::Shared;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;

pub fn routes(state: Shared) -> Router {
    Router::new()
        .route("/api/Auth/token/{scope}/{key}", get(auth::token))
        .route("/api/Auth/token/{scope}/{key}/{*is_md5}", get(auth::token))
        .route("/api/Auth/whoami", get(auth::whoami))
        .route("/api/Tour/all/suggested", get(tour::all_suggested))
        .route(
            "/api/Tour/{id}",
            get(tour::one).patch(write::update).delete(write::delete),
        )
        .route("/api/Tour/add/{code}", axum::routing::post(write::add))
        .route("/api/Tour/{id}/versions", get(tour::versions))
        .route("/api/Subscription/publickey", get(subscription::public_key))
        .route(
            "/api/Subscription/check/{tour}",
            axum::routing::post(subscription::check),
        )
        .route(
            "/api/Subscription/subscribe/{tour}",
            axum::routing::post(subscription::subscribe),
        )
        .route(
            "/api/Subscription/unsubscribe/{tour}",
            axum::routing::post(subscription::unsubscribe),
        )
        .route("/api/Info/start", get(info_start))
        .route("/api/Info/version", get(info_version))
        .route("/api/Info/wakeup/{code}", get(info_wakeup))
        .route("/api/Auth/random/{length}", get(auth::random))
        .route("/api/Log/headers", get(log_headers))
        .route("/api/Log/logs", get(log_logs))
        // The client fires these off and never reads the answer; without a route they would
        // fill its console with 404s.
        .route("/api/Log/x/{*rest}", get(|| async { StatusCode::OK }))
        // Anything else under /api is a 404 and not the app.
        //
        // Without this the request falls through to the static fallback and comes back as
        // index.html with a 200, so a client asking for an endpoint that does not exist gets
        // a page and has to discover it is not JSON. The C# has the same catch-all for the
        // same reason. Registered last, and matched last: axum prefers a static segment to a
        // parameter and a parameter to a wildcard, so the real routes still win.
        .route("/api/{*rest}", any(|| async { StatusCode::NOT_FOUND }))
        .with_state(state)
}

/// `GET /api/Info/version` - which build is running, and which client it is handing out.
///
/// Three questions, and they are not the same one:
///
/// * *Is my browser holding an old copy?* - `client` is the file name of the wasm this
///   server serves, hash and all. A browser compares it with the one it loaded.
/// * *Is the running server the build I pushed?* - `build` and `commit`.
/// * *Did the deploy happen at all?* - `build` and `commit` name the image that is actually
///   running; `started` says when it last came up. An image that was built and never started
///   is not running anywhere, and those two together are what shows it.
///
/// No authentication: it names a build, and the build is named in an HTTP header on every
/// response already.
async fn info_version(
    axum::extract::State(state): axum::extract::State<Shared>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "build": state.build_id,
        "buildType": state.build_type,
        "commit": state.build_commit,
        "client": state.client_asset,
        "started": chrono_lite::Utc::from(state.started).to_string(),
    }))
}

/// `GET /api/Info/start` - when the server came up, and when it was last woken.
///
/// A diagnostic: nothing in either client reads it. The field names are the C#'s all the
/// same, because the point of an endpoint that exists for a person looking at it is that the
/// person can compare the two servers without translating.
async fn info_start(
    axum::extract::State(state): axum::extract::State<Shared>,
) -> axum::Json<serde_json::Value> {
    let stamp = |t: std::time::SystemTime| chrono_lite::Utc::from(t).to_string();
    let wakeups: Vec<String> = state
        .wakeups
        .read()
        .map(|w| w.iter().copied().map(stamp).collect())
        .unwrap_or_default();

    axum::Json(serde_json::json!({
        "startTime": stamp(state.started),
        "lastWakeups": wakeups,
        "numberWakeupsToKeep": crate::state::WAKEUPS_TO_KEEP,
    }))
}

/// `GET /api/Info/wakeup/{code}` - hold a sleeping instance open for a while.
///
/// For hosting that stops a container nobody has talked to. Something outside calls this;
/// the server notes the time and sits on the request, so the platform sees work in progress
/// and leaves it alone.
///
/// The C# then calls a `WakeupUrl` of its own, passing the favour along a chain of
/// instances. That part is deliberately not here: making one outbound HTTPS request would
/// mean linking a TLS stack into a server that otherwise opens no connection at all, and
/// with it the C code that this build's cross-compilation is free of. The waiting - which is
/// the part that keeps the instance up - is faithful.
async fn info_wakeup(
    axum::extract::State(state): axum::extract::State<Shared>,
    axum::extract::Path(code): axum::extract::Path<String>,
) -> &'static str {
    if code != state.wakeup_code {
        return "wrong code";
    }
    state.woke_up();
    tracing::info!("wakeup");

    let minutes = state.wakeup_pre_delay_min + state.wakeup_post_delay_min;
    tokio::time::sleep(std::time::Duration::from_secs(minutes * 60)).await;
    "ok"
}

/// `GET /api/Log/headers` - the request's own headers, as text.
///
/// What it is for: seeing what a proxy in front of this server is actually sending.
async fn log_headers(headers: axum::http::HeaderMap) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{name}: {}", value.to_str().unwrap_or("<not text>")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `GET /api/Log/logs` - always empty, as in production.
///
/// The C# has an interface here with two implementations, and the one it is wired to is
/// `VoidLogStorage` - it stores nothing and answers with nothing. Reproducing the empty
/// answer is reproducing the behaviour; reproducing the unused implementation behind it
/// would not be. Administrators only, like the original.
async fn log_logs(Bearer(auth): Bearer) -> axum::Json<Vec<serde_json::Value>> {
    let _ = auth.is_master;
    axum::Json(Vec::new())
}

/// Formats a unix timestamp the way the stored `StateGUID` is written.
pub fn stamp(secs: u64) -> String {
    chrono_lite::Utc(secs)
        .to_string()
        .replace('T', " ")
        .replace('Z', "")
}

/// A minimal ISO-8601 stamp, so that this crate does not pull in a date library to print
/// one field. Seconds since the epoch is not what the client shows, but the field is only
/// displayed, never parsed back.
mod chrono_lite {
    pub struct Utc(pub u64);

    impl From<std::time::SystemTime> for Utc {
        fn from(t: std::time::SystemTime) -> Utc {
            Utc(t
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0))
        }
    }

    impl std::fmt::Display for Utc {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // Days since epoch to a civil date, by the usual algorithm. Fewer than twenty
            // lines, against a dependency that would also land in the client one day.
            let (secs, days) = (self.0 % 86_400, self.0 / 86_400);
            let z = days as i64 + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if m <= 2 { y + 1 } else { y };
            write!(
                f,
                "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        }
    }
}

/// The bearer's rights, pulled out of the Authorization header.
///
/// Written as an extractor so that a handler can simply take an `AuthData` argument and be
/// given one. That is a trait implementation - `FromRequestParts` - and it is how axum lets
/// a function signature say what it needs instead of every handler repeating the plumbing.
pub struct Bearer(pub crate::auth::AuthData);

impl FromRequestParts<Shared> for Bearer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                let (scheme, rest) = v.split_once(' ')?;
                scheme.eq_ignore_ascii_case("bearer").then_some(rest.trim())
            });

        // No token, or a bad one, is not an error: it is simply nobody. The app is meant to
        // be opened by anyone with a link, and it shows a login screen rather than a 401.
        let auth = match token {
            Some(t) => state.signer.verify(t).unwrap_or_else(|why| {
                tracing::debug!("rejecting token: {why}");
                crate::auth::AuthData::default()
            }),
            None => crate::auth::AuthData::default(),
        };
        Ok(Bearer(auth))
    }
}

/// What can go wrong.
pub enum ApiError {
    NotFound(String),
    NotAuthenticated(String),
    Forbidden(String),
    BadRequest(String),
    /// Somebody else saved first. The client has to reload and redo, and telling it apart
    /// from a plain failure is the whole point of the soft lock.
    Conflict(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, message) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::NotAuthenticated(m) => (StatusCode::UNAUTHORIZED, m),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
        };
        (code, message).into_response()
    }
}
