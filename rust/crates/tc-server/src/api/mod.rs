//! The HTTP surface, shaped to match what the existing client already asks for.
//!
//! Nothing here invents a nicer API. The Blazor client, the text pages and the bot all call
//! these paths with these spellings today, so the port has to answer them exactly - which
//! also makes for the strongest test available: point the existing client at this server
//! and see whether it notices.

pub mod auth;
pub mod tour;
pub mod write;

use crate::state::Shared;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
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
        .route("/api/Info/start", get(info_start))
        // The client fires these off and never reads the answer; without a route they would
        // fill its console with 404s.
        .route("/api/Log/x/{*rest}", get(|| async { StatusCode::OK }))
        .with_state(state)
}

async fn info_start(
    axum::extract::State(state): axum::extract::State<Shared>,
) -> axum::Json<serde_json::Value> {
    let started: chrono_lite::Utc = state.started.into();
    axum::Json(serde_json::json!({
        "StartupTime": started.to_string(),
        "WakeupTime": serde_json::Value::Null,
    }))
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
