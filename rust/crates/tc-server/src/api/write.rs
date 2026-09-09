//! Changing tours.
//!
//! The client sends the whole tour back, which is how the app has always saved: the API has
//! no "add a spending" call any more (those routes are commented out in the C# controller),
//! only a PATCH of everything. It works because a tour is small and because the client is
//! the one that edits it.
//!
//! Two rules make that safe enough, and both are inherited rather than invented:
//!
//! * **the soft lock.** Every stored tour carries a `StateGUID`. A save must present the
//!   one it read, or it is refused with 409 - somebody else has written in the meantime and
//!   the loser would silently erase their work.
//! * **the body is not trusted for anything about access.** The id comes from the URL and
//!   the access code from the stored record, so a save cannot move a tour into somebody
//!   else's pile.

use super::{ApiError, Bearer};
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::Json;
use tc_core::{Tour, TourId};

/// `PATCH /api/Tour/{id}` - store the tour as given, and answer with its id.
pub async fn update(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<String, ApiError> {
    let tour_id = TourId::new(id.clone());
    let stored = state
        .store
        .get(&tour_id)
        .filter(|t| auth.may_see(super::tour::access_code_of(t)))
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {id}")))?;

    let mut incoming = Tour::from_json(&body.to_string())
        .map_err(|e| ApiError::BadRequest(format!("could not read the tour: {e}")))?;

    // Restoring an old version is a different operation with different rules, and versions
    // are not kept here yet. Refusing plainly beats saving it as if it were an edit.
    if extra_bool(&incoming, "IsVersion") {
        return Err(ApiError::BadRequest(
            "restoring a version is not supported by this server yet".into(),
        ));
    }

    let stored_state = extra_str(&stored, "StateGUID");
    let sent_state = extra_str(&incoming, "StateGUID");
    if stored_state != sent_state {
        return Err(ApiError::Conflict(format!(
            "You are trying to override newer version of tour ({stored_state})"
        )));
    }

    // The id and the access code come from what is stored, never from the body.
    incoming.id = tour_id;
    set_extra(&mut incoming, "StateGUID", new_state_guid().into());
    set_extra(
        &mut incoming,
        "AccessCodeMD5",
        super::tour::access_code_of(&stored).into(),
    );

    state.store.store(incoming);
    Ok(id)
}

/// `POST /api/Tour/add/{accessCode}` - create a tour and answer with its new id.
pub async fn add(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(code): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<String, ApiError> {
    // An administrator may open a new pile; everybody else may only add to one that already
    // has something in it. Otherwise a stray code would quietly become a new account.
    let mine: Vec<_> = state
        .store
        .list(&|t: &Tour| auth.may_see(super::tour::access_code_of(t)));
    if !auth.is_master && mine.is_empty() {
        return Err(ApiError::Forbidden(
            "Only admin can create first tour for a code".into(),
        ));
    }

    let target_code = if auth.is_master {
        crate::auth::code_md5(&code)
    } else {
        auth.access_codes()
            .next()
            .ok_or_else(|| {
                ApiError::Forbidden("No valid access code associated with this token".into())
            })?
            .to_owned()
    };

    let mut tour = Tour::from_json(&body.to_string())
        .map_err(|e| ApiError::BadRequest(format!("could not read the tour: {e}")))?;

    let id = new_tour_id();
    tour.id = TourId::new(id.clone());
    set_extra(&mut tour, "AccessCodeMD5", target_code.into());
    set_extra(&mut tour, "StateGUID", new_state_guid().into());

    state.store.store(tour);
    Ok(id)
}

/// `DELETE /api/Tour/{id}` - remove a tour and answer with its id.
pub async fn delete(
    State(state): State<Shared>,
    Bearer(auth): Bearer,
    Path(id): Path<String>,
) -> Result<String, ApiError> {
    // The last tour under a code is kept: deleting it would leave the code with nothing,
    // and then only an administrator could ever put something back.
    if !auth.is_master {
        let mine = state
            .store
            .list(&|t: &Tour| auth.may_see(super::tour::access_code_of(t)));
        if mine.len() <= 1 {
            return Err(ApiError::Forbidden(
                "Only admin can delete last tour for a code".into(),
            ));
        }
    }

    let tour_id = TourId::new(id.clone());
    state
        .store
        .get(&tour_id)
        .filter(|t| auth.may_see(super::tour::access_code_of(t)))
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {id}")))?;

    state.store.remove(&tour_id);
    Ok(id)
}

// --- the bits of a tour this crate keeps in `Extras` --------------------------------------
//
// `StateGUID` and `AccessCodeMD5` are about storage and access rather than about money, so
// `tc-core` does not model them and they travel in the carry-through bag. Reading them is
// case-insensitive for the same reason as everywhere else: the stored data is not
// consistent about it.

fn extra_str(tour: &Tour, key: &str) -> String {
    tour.extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
        .to_owned()
}

fn extra_bool(tour: &Tour, key: &str) -> bool {
    tour.extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_bool())
        .unwrap_or(false)
}

fn set_extra(tour: &mut Tour, key: &str, value: serde_json::Value) {
    // Replace whatever spelling is already there, so a camelCase tour does not end up with
    // both `stateGUID` and `StateGUID`.
    let existing: Option<String> = tour
        .extras
        .0
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned();
    let key = existing.unwrap_or_else(|| key.to_owned());
    tour.extras.0.insert(key, value);
}

/// The soft lock's value: a timestamp and a random part, in the shape the C# writes.
///
/// Nothing parses it - the only thing anybody does with it is compare it for equality - but
/// keeping the shape means a tour saved here is indistinguishable from one saved there.
fn new_state_guid() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        // The C# writes it in UTC+3, and a timestamp that reads differently would look like
        // a bug to anybody comparing two records.
        + 3 * 3600;
    format!("{} .{}", super::stamp(now), random_hex(32))
}

/// A short, URL-safe id for a new tour, in the style of the existing ones.
fn new_tour_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    random_bytes(7)
        .iter()
        .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
        .collect()
}

fn random_hex(chars: usize) -> String {
    random_bytes(chars.div_ceil(2))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .chars()
        .take(chars)
        .collect()
}

fn random_bytes(n: usize) -> Vec<u8> {
    use p256::elliptic_curve::rand_core::RngCore;
    let mut buf = vec![0u8; n];
    // The same source the signing key uses; no extra dependency for a handful of bytes.
    p256::elliptic_curve::rand_core::OsRng.fill_bytes(&mut buf);
    buf
}
