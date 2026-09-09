//! Reading tours.
//!
//! The arithmetic here is not here: it is `tc-core`, the same crate the browser compiles
//! into its wasm. That is the whole reason the rewrite is worth doing at all - one
//! implementation of who owes whom, used by the server, the offline client, the text pages
//! and the bot alike.

use super::{ApiError, Bearer};
use crate::state::Shared;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use tc_core::{Tour, TourId};

/// The shape the client expects a list in. C#'s field names, so C#'s spelling.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct TourList {
    pub tours: Vec<serde_json::Value>,
    pub total_count: usize,
    pub from: usize,
    pub count: usize,
    pub requested_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct Paging {
    #[serde(default)]
    pub from: usize,
    #[serde(default = "fifty")]
    pub count: usize,
    #[serde(default)]
    pub code: String,
}

fn fifty() -> usize {
    50
}

/// `GET /api/Tour/{id}` - one tour, as stored.
pub async fn one(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tour = state
        .store
        .get(&TourId::new(id.clone()))
        // A tour the bearer may not see is reported as missing rather than as forbidden:
        // saying "it exists but not for you" would leak which ids are real.
        .filter(|t| auth.may_see(access_code_of(t)))
        .ok_or_else(|| ApiError::NotFound(format!("No tour with id={id}")))?;

    Ok(Json(to_value(&tour)))
}

/// `GET /api/Tour/all/suggested` - every visible tour, settled, without its spendings.
///
/// The C# version strips the spendings and the per-person breakdowns before answering,
/// because the list screen shows neither and they are most of the payload. Same here.
pub async fn all_suggested(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Query(paging): Query<Paging>,
) -> Json<TourList> {
    // An admin may narrow the list to one access code; everybody else gets their own.
    let wanted_code = if auth.is_master && !paging.code.is_empty() {
        Some(crate::auth::code_md5(&paging.code))
    } else {
        None
    };

    let visible = state.store.list(&|t: &Tour| {
        let code = access_code_of(t);
        match &wanted_code {
            Some(c) => code == c,
            None => auth.may_see(code),
        }
    });

    let total = visible.len();
    let page: Vec<serde_json::Value> = visible
        .iter()
        .skip(paging.from)
        .take(paging.count)
        .map(|tour| {
            let transfers = tc_core::suggest_settlement(tour).unwrap_or_default();
            let balances = tc_core::calculate(tour, tc_core::Options::default());
            let mut value = to_value(tour);

            // The list needs the totals, not the detail.
            if let Some(obj) = value.as_object_mut() {
                obj.insert("Spendings".into(), serde_json::json!([]));
                if let Some(persons) = obj.get_mut("Persons").and_then(|p| p.as_array_mut()) {
                    for p in persons.iter_mut() {
                        let Some(p) = p.as_object_mut() else { continue };
                        // Fill in what the calculator worked out, the way the C# does by
                        // returning a calculated tour.
                        if let Some(id) = p.get("GUID").and_then(|v| v.as_str()) {
                            if let Some(b) = balances.get(&tc_core::PersonId::new(id)) {
                                p.insert("SpentInCents".into(), b.spent.0.into());
                                p.insert("ReceivedInCents".into(), b.received.0.into());
                            }
                        }
                        p.insert("SpentSendingInfo".into(), serde_json::json!([]));
                        p.insert("ReceivedSendingInfo".into(), serde_json::json!([]));
                    }
                }
                obj.insert(
                    "SuggestedPaymentsCount".into(),
                    serde_json::json!(transfers.len()),
                );
            }
            value
        })
        .collect();

    Json(TourList {
        count: page.len(),
        tours: page,
        total_count: total,
        from: paging.from,
        requested_count: paging.count,
    })
}

/// `GET /api/Tour/{id}/versions` - always empty for now.
///
/// Versions are kept by the storage layer, which this server does not have yet. An empty
/// list is what a tour with no history returns anyway, so the client renders correctly
/// rather than erroring; when storage arrives this stops being a stub.
pub async fn versions(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(id): Path<String>,
) -> Result<Json<TourList>, ApiError> {
    state
        .store
        .get(&TourId::new(id.clone()))
        .filter(|t| auth.may_see(access_code_of(t)))
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {id}")))?;

    Ok(Json(TourList {
        tours: Vec::new(),
        total_count: 0,
        from: 0,
        count: 0,
        requested_count: 0,
    }))
}

/// The tour, as JSON in the stored shape.
fn to_value(tour: &Tour) -> serde_json::Value {
    tour.to_json()
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Which pile of tours this one belongs to.
///
/// `AccessCodeMD5` is not modelled by `tc-core` - it is about who may look, not about the
/// money - so it rides along in `Extras` and is read from there.
///
/// Case-insensitively, because the stored tours are not consistent about it: three of the
/// eight in the seed file spell every field in camelCase, so this one is `accessCodeMD5`
/// there. Looking only for the capitalised spelling made those tours invisible to everyone,
/// which is the sort of thing that shows up as "two tours missing from the list" and not as
/// an error anywhere.
fn access_code_of(tour: &Tour) -> &str {
    tour.extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("AccessCodeMD5"))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
}
