//! Handing out and reading back tokens.

use super::{ApiError, Bearer};
use crate::auth::{code_md5, AuthData};
use crate::state::Shared;
use axum::extract::{Path, State};
use axum::Json;

/// `GET /api/Auth/token/{scope}/{key}[/{anything}]`
///
/// The trailing segment means "the key is already an MD5" - the app puts the word `md5`
/// there, and the C# route only checks whether anything is present at all.
pub async fn token(
    State(state): State<Shared>,
    path: Path<Vec<String>>,
) -> Result<String, ApiError> {
    let parts = &path.0;
    let scope = parts.first().cloned().unwrap_or_default();
    let key = parts.get(1).cloned().unwrap_or_default();
    let already_md5 = parts.len() > 2 && !parts[2].is_empty();

    let auth = match scope.as_str() {
        "admin" => {
            if key == state.master_key && !state.master_key.is_empty() {
                AuthData::master()
            } else {
                return Err(ApiError::NotAuthenticated("Wrong Master Key".into()));
            }
        }
        "code" => AuthData::for_code_md5(if already_md5 { key } else { code_md5(&key) }),
        _ => {
            return Err(ApiError::NotAuthenticated(
                "Wrong scope. Please try 'code' or 'admin'.".into(),
            ))
        }
    };

    // Plain text, not JSON.
    //
    // An ASP.NET controller returning `string` does not produce a JSON string: the
    // StringOutputFormatter gets there first and writes the value as text/plain with no
    // quotes. Answering with `"eyJ..."` instead put the quotes into the client's stored
    // token and every later request went out as `Authorization: bearer "eyJ...`, which this
    // server then refused - a 404 on the tour, three steps away from the actual mistake.
    Ok(state.signer.issue(&scope, &auth, state.token_valid_minutes))
}

/// `GET /api/Auth/whoami` - what the token says, or the anonymous default.
pub async fn whoami(Bearer(auth): Bearer) -> Json<AuthData> {
    Json(auth)
}

/// `GET /api/Auth/random/{length}` - random bytes, base64, for making a key with.
///
/// Not a secret-issuing endpoint: it hands out entropy, not authority. The cap is the C#'s.
pub async fn random(Path(length): Path<usize>) -> Result<String, ApiError> {
    const MOST: usize = 8192;
    if length > MOST {
        return Err(ApiError::Forbidden(format!(
            "Length should be up to {MOST} bytes. You specified {length}"
        )));
    }
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(crate::api::write::random_bytes(length)))
}
