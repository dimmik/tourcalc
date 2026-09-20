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
    Ok(Json(state.subscriptions.has(&tour, &sub).await))
}

pub async fn subscribe(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(tour): Path<String>,
    Json(sub): Json<Subscription>,
) -> Result<String, ApiError> {
    seen_by(&state, &auth, &tour).await?;
    state.subscriptions.add(&tour, sub).await;
    Ok("OK".into())
}

pub async fn unsubscribe(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(tour): Path<String>,
    Json(sub): Json<Subscription>,
) -> Result<String, ApiError> {
    seen_by(&state, &auth, &tour).await?;
    state.subscriptions.remove(&tour, &sub).await;
    Ok("OK".into())
}

/// Which of the reader's tours this browser is subscribed to - the bells in the tour list.
///
/// One request for the whole list, whatever its length: asking `check` for each tour would
/// be a request per row, each reading its tour whole. Only tours this token may see are
/// named; an endpoint alone must not tell anybody which tours exist.
pub async fn mine(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Json(sub): Json<Subscription>,
) -> Json<Vec<String>> {
    let tours = state.subscriptions.tours_of(&sub).await;
    if tours.is_empty() {
        return Json(Vec::new());
    }
    let seen = state
        .store
        .access_codes(&tours)
        .await
        .into_iter()
        .filter(|(_, code)| auth.may_see(code))
        .map(|(id, _)| id)
        .collect();
    Json(seen)
}

/// Only for a tour this token may see.
///
/// The C# marks the controller `[Authorize]` and stops there, so anybody holding any token
/// may subscribe to any tour id they can guess. A notification carries the tour's name and
/// what changed, so that is a leak; checking costs the one lookup the handler would do
/// anyway.
async fn seen_by(state: &Shared, auth: &crate::auth::AuthData, tour: &str) -> Result<(), ApiError> {
    // `state_of` rather than `get`: it answers with the access code and the state id, which
    // in a database is two fields rather than a whole tour with every expense in it. The
    // bell on a tour page asks this on every open.
    state
        .store
        .state_of(&tc_core::TourId::new(tour.to_owned()))
        .await
        .filter(|(code, _)| auth.may_see(code))
        .map(|_| ())
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {tour}")))
}
