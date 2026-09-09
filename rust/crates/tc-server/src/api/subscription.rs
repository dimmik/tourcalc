//! `api/Subscription/*` - who wants telling when a tour changes.

use super::{ApiError, Bearer};
use crate::state::Shared;
use crate::subscriptions::Subscription;
use axum::extract::{Path, State};
use axum::Json;

/// The key a browser needs to subscribe at all. Public, and meant to be.
pub async fn public_key(State(state): State<Shared>) -> String {
    state.push.public_key().to_owned()
}

pub async fn check(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(tour): Path<String>,
    Json(sub): Json<Subscription>,
) -> Result<Json<bool>, ApiError> {
    seen_by(&state, &auth, &tour).await?;
    Ok(Json(state.subscriptions.has(&tour, &sub)))
}

pub async fn subscribe(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(tour): Path<String>,
    Json(sub): Json<Subscription>,
) -> Result<String, ApiError> {
    seen_by(&state, &auth, &tour).await?;
    state.subscriptions.add(&tour, sub);
    Ok("OK".into())
}

pub async fn unsubscribe(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(tour): Path<String>,
    Json(sub): Json<Subscription>,
) -> Result<String, ApiError> {
    seen_by(&state, &auth, &tour).await?;
    state.subscriptions.remove(&tour, &sub);
    Ok("OK".into())
}

/// Only for a tour this token may see.
///
/// The C# marks the controller `[Authorize]` and stops there, so anybody holding any token
/// may subscribe to any tour id they can guess. A notification carries the tour's name and
/// what changed, so that is a leak; checking costs the one lookup the handler would do
/// anyway.
async fn seen_by(state: &Shared, auth: &crate::auth::AuthData, tour: &str) -> Result<(), ApiError> {
    state
        .store
        .get(&tc_core::TourId::new(tour.to_owned()))
        .await
        .filter(|t| auth.may_see(&crate::fields::access_code(t)))
        .map(|_| ())
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {tour}")))
}
