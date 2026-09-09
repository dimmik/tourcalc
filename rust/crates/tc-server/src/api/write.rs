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
use crate::fields;
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::Json;
use tc_core::{Tour, TourId};

/// `PATCH /api/Tour/{id}` - store the tour as given, and answer with its id.
///
/// Two different operations arrive here, as they do in the C#. Sending a tour saves it, and
/// sending a *version* of it restores that version - which is why the version check comes
/// first and skips the soft lock: a restore is deliberately overwriting whatever is there.
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
        .await
        .filter(|t| auth.may_see(&fields::access_code(t)))
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {id}")))?;

    let mut incoming = Tour::from_json(&body.to_string())
        .map_err(|e| ApiError::BadRequest(format!("could not read the tour: {e}")))?;

    // A version's own record may not be written to: it is what was, and editing it would
    // make it a record of nothing.
    if fields::is_version(&stored) && !state.version_editable {
        return Err(ApiError::Forbidden("Versions are not editable".into()));
    }

    let restoring = fields::is_version(&incoming)
        && incoming.id.as_str() != stored.id.as_str()
        && !fields::is_version(&stored);

    if restoring {
        // The restored copy becomes the tour again, and the state it replaces is kept with
        // a line saying what happened - otherwise a restore is the one change in a tour's
        // history that leaves no trace of what it undid.
        let when = fields::str_of(&incoming, fields::VERSIONED_AT);
        fields::set(&mut incoming, fields::IS_VERSION, false.into());
        fields::set(
            &mut incoming,
            fields::INTERNAL_VERSION_COMMENT,
            format!("Tour Restored to {when}").into(),
        );
    } else {
        let sent_state = fields::str_of(&incoming, fields::STATE);
        let stored_state = fields::str_of(&stored, fields::STATE);
        if stored_state != sent_state {
            return Err(ApiError::Conflict(format!(
                "You are trying to override newer version of tour ({stored_state})"
            )));
        }
    }

    // The id and the access code come from what is stored, never from the body.
    incoming.id = tour_id.clone();
    fields::set(&mut incoming, fields::STATE, new_state_guid().into());
    fields::set(
        &mut incoming,
        fields::ACCESS_CODE,
        fields::access_code(&stored).into(),
    );
    // Asked for by whoever is saving, and never stored on the tour itself.
    let asked_comment = fields::str_of(&incoming, fields::INTERNAL_VERSION_COMMENT);
    fields::remove(&mut incoming, fields::INTERNAL_VERSION_COMMENT);

    // What this save did, in words. One decision, used for three things: whether to keep a
    // version at all, what to write on it, and what to tell the people subscribed.
    let change = if asked_comment.is_empty() {
        crate::versions::describe_change(&stored, &incoming)
    } else {
        Some(asked_comment.clone())
    };

    let keep_versions = state.versioning;
    let comment_for_version = change.clone();
    let make_version = |previous: &Tour| -> Option<Tour> {
        if !keep_versions {
            return None;
        }
        // A save that changed nothing anybody can name leaves no version. Without that,
        // every reopened tour and every re-saved form would add a line to the history.
        Some(version_of(previous, comment_for_version.clone()?))
    };

    // The check and the write are one step; see `TourStore::replace`.
    match state
        .store
        .replace(
            &tour_id,
            &fields::str_of(&stored, fields::STATE),
            incoming.clone(),
            &make_version,
        )
        .await
    {
        Ok(()) => {
            if let Some(what) = change {
                state.announce(&id, format!("{} : {what}", incoming.name));
            }
            Ok(id)
        }
        Err(crate::store::Stale(now)) => Err(ApiError::Conflict(format!(
            "You are trying to override newer version of tour ({now})"
        ))),
    }
}

/// The copy of a state that is kept when it is replaced.
pub fn version_of(previous: &Tour, comment: String) -> Tour {
    let mut version = previous.clone();
    // Its own record, pointing at the tour it belongs to.
    version.id = TourId::new(new_version_id());
    fields::set(&mut version, fields::IS_VERSION, true.into());
    fields::set(
        &mut version,
        fields::VERSION_FOR,
        previous.id.as_str().into(),
    );
    fields::set(
        &mut version,
        fields::VERSIONED_AT,
        fields::now_stamp().into(),
    );
    fields::set(&mut version, fields::VERSION_COMMENT, comment.into());
    version
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
        .list(&|t: &Tour| auth.may_see(&fields::access_code(t)))
        .await;
    if !auth.is_master {
        if mine.is_empty() {
            return Err(ApiError::Forbidden(
                "Only admin can create first tour for a code".into(),
            ));
        }
        // A limit on how many tours one code may hold, off by default. An administrator is
        // not subject to it, which is the point of asking one.
        let most = state.max_tours_per_code;
        if most >= 0 && mine.len() as i64 >= most {
            return Err(ApiError::Forbidden(format!(
                "You can create up to {most} tours per code. To add more please ask administrator"
            )));
        }
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
    fields::set(&mut tour, fields::ACCESS_CODE, target_code.into());
    fields::set(&mut tour, fields::STATE, new_state_guid().into());
    fields::set(&mut tour, fields::CREATED_AT, fields::now_stamp().into());
    // Nobody creates a tour that is already somebody's history.
    fields::remove(&mut tour, fields::IS_VERSION);
    fields::remove(&mut tour, fields::VERSION_FOR);

    state.store.store(tour).await;
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
            .list(&|t: &Tour| auth.may_see(&fields::access_code(t)))
            .await;
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
        .await
        .filter(|t| auth.may_see(&fields::access_code(t)))
        .ok_or_else(|| ApiError::NotFound(format!("no tour with id {id}")))?;

    // The versions go with it: they are that tour's history and belong to nobody else.
    let (versions, _) = state.store.versions(&tour_id, 0, usize::MAX).await;
    for v in versions {
        state.store.remove(&v.id).await;
    }
    state.store.remove(&tour_id).await;
    Ok(id)
}

// --- ids, in the shapes the C# writes -----------------------------------------------------

/// The soft lock's value: a timestamp and a random part, in the shape the C# writes.
///
/// Nothing parses it - the only thing anybody does with it is compare it for equality - but
/// keeping the shape means a tour saved here is indistinguishable from one saved there.
pub fn new_state_guid() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        // The C# writes it in UTC+3, and a timestamp that reads differently would look like
        // a bug to anybody comparing two records.
        + 3 * 3600;
    format!("{} .{}", super::stamp(now), random_hex(32))
}

/// The id a suggested payment would have if it were stored.
///
/// The C# gives every generated payment an id that is the MD5 of who pays, the description,
/// the amount and who is paid - so it is the same id every time the settlement is worked out
/// from the same tour. That is what lets a page show a payment, and a form post afterwards
/// name the very same one, without either of them being stored in between.
pub fn transfer_id(t: &tc_core::Transfer) -> String {
    crate::auth::code_md5(&format!(
        "{}{}{}{}",
        t.from.as_str(),
        t.description,
        t.amount.0,
        t.to.as_str()
    ))
}

/// A short id for something new inside a tour, in the style of the existing ones.
pub fn new_spending_id() -> String {
    new_tour_id()
}

/// A version's own id. The C# uses a GUID here, and nothing reads it but the store.
fn new_version_id() -> String {
    format!(
        "{}-{}-{}-{}-{}",
        random_hex(8),
        random_hex(4),
        random_hex(4),
        random_hex(4),
        random_hex(12)
    )
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

pub fn random_bytes(n: usize) -> Vec<u8> {
    use p256::elliptic_curve::rand_core::RngCore;
    let mut buf = vec![0u8; n];
    // The same source the signing key uses; no extra dependency for a handful of bytes.
    p256::elliptic_curve::rand_core::OsRng.fill_bytes(&mut buf);
    buf
}
