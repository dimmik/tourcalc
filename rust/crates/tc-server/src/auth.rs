//! Who is asking, and what they may see.
//!
//! The token has to be **the same token** the C# server issues: same signature, same
//! claims, same spelling. A browser holding a login from the old server must keep working
//! against this one, and vice versa - that is the only way the two can be run side by side
//! while the port is finished.
//!
//! Signing is done with `p256` rather than a JWT library, for two reasons. The key on disk
//! is a bare 32-byte scalar, which most libraries will not take without a detour through
//! PKCS#8; and a JWT is three base64url segments and a signature, which is short enough to
//! write out and read.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use md5::Digest as _;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// What a token says about its bearer.
///
/// Serialised into the token verbatim as the `AuthDataJson` claim, so the field names are
/// C#'s and not up for improvement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthData {
    #[serde(rename = "Type")]
    pub kind: String,
    #[serde(rename = "IsMaster")]
    pub is_master: bool,
    #[serde(rename = "AccessCodeMD5")]
    pub access_code_md5: String,
}

impl Default for AuthData {
    /// Nobody, with no access to anything.
    fn default() -> Self {
        AuthData {
            kind: "None".to_owned(),
            is_master: false,
            access_code_md5: String::new(),
        }
    }
}

impl AuthData {
    pub fn master() -> Self {
        AuthData {
            kind: "Master".to_owned(),
            is_master: true,
            access_code_md5: String::new(),
        }
    }

    pub fn for_code_md5(md5: impl Into<String>) -> Self {
        AuthData {
            kind: "AccessCode".to_owned(),
            is_master: false,
            access_code_md5: md5.into(),
        }
    }

    /// One token can carry several codes, separated by ';'.
    pub fn access_codes(&self) -> impl Iterator<Item = &str> {
        self.access_code_md5
            .split(';')
            .filter(|s| !s.trim().is_empty())
    }

    /// Whether this bearer may see a tour filed under `tour_code`.
    pub fn may_see(&self, tour_code: &str) -> bool {
        self.is_master || self.access_codes().any(|c| c == tour_code)
    }
}

/// The access code, hashed the way the app has always hashed it.
///
/// Uppercase hex of the MD5 of the **ASCII** bytes. MD5 is not protecting anything here -
/// the hash is an identifier for "which pile of tours", and the codes themselves are shared
/// out loud - but the spelling has to match, because these strings are stored on the tours.
pub fn code_md5(code: &str) -> String {
    let mut h = md5::Md5::new();
    // ASCII, as in the C#: anything outside it is already lost there, and a different
    // encoding here would hash the same code to a different pile.
    let ascii: Vec<u8> = code
        .chars()
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .collect();
    h.update(ascii);
    h.finalize().iter().map(|b| format!("{b:02X}")).collect()
}

/// The key that signs tokens: a raw 32-byte P-256 scalar, base64 in the configuration.
#[derive(Clone)]
pub struct Signer_ {
    signing: SigningKey,
}

impl Signer_ {
    pub fn from_base64(b64: &str) -> Result<Self, String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| format!("AuthPrivateECDSAKey is not base64: {e}"))?;
        let signing = SigningKey::from_slice(&bytes)
            .map_err(|e| format!("AuthPrivateECDSAKey is not a P-256 private key: {e}"))?;
        Ok(Signer_ { signing })
    }

    fn verifying(&self) -> VerifyingKey {
        *self.signing.verifying_key()
    }

    /// Builds the same JWT the C# server builds: ES256, issuer TourCalc, audience Users.
    pub fn issue(&self, scope: &str, auth: &AuthData, valid_minutes: i64) -> String {
        let header = serde_json::json!({ "alg": "ES256", "typ": "JWT" });
        let exp = now() + valid_minutes * 60;
        let claims = serde_json::json!({
            // The short name is what JwtSecurityTokenHandler writes for
            // ClaimTypes.NameIdentifier, and what its reader expects back.
            "nameid": scope,
            "AuthDataJson": serde_json::to_string(auth).unwrap_or_default(),
            "exp": exp,
            "iss": "TourCalc",
            "aud": "Users",
        });

        let signing_input = format!(
            "{}.{}",
            B64.encode(header.to_string()),
            B64.encode(claims.to_string())
        );
        let sig: Signature = self.signing.sign(signing_input.as_bytes());
        format!("{signing_input}.{}", B64.encode(sig.to_bytes()))
    }

    /// Reads a token back, or says why it will not have it.
    ///
    /// An unreadable token is not an error the caller has to handle - the app is readable
    /// by anyone with a link and simply shows nothing without a good one - so callers turn
    /// this into `AuthData::default()`. The reason is still returned, for the log.
    pub fn verify(&self, token: &str) -> Result<AuthData, String> {
        let mut parts = token.split('.');
        let (Some(h), Some(p), Some(s), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err("not three dot-separated segments".into());
        };

        let sig_bytes = B64.decode(s).map_err(|e| format!("signature: {e}"))?;
        let sig = Signature::from_slice(&sig_bytes).map_err(|e| format!("signature: {e}"))?;
        let signing_input = format!("{h}.{p}");
        self.verifying()
            .verify(signing_input.as_bytes(), &sig)
            .map_err(|_| "signature does not match".to_owned())?;

        let payload = B64.decode(p).map_err(|e| format!("payload: {e}"))?;
        let claims: serde_json::Value =
            serde_json::from_slice(&payload).map_err(|e| format!("payload: {e}"))?;

        if claims.get("iss").and_then(|v| v.as_str()) != Some("TourCalc") {
            return Err("wrong issuer".into());
        }
        if claims.get("aud").and_then(|v| v.as_str()) != Some("Users") {
            return Err("wrong audience".into());
        }
        // Five seconds of slack, matching the ClockSkew the C# server allows.
        let exp = claims.get("exp").and_then(|v| v.as_i64()).unwrap_or(0);
        if exp + 5 < now() {
            return Err("expired".into());
        }

        let json = claims
            .get("AuthDataJson")
            .and_then(|v| v.as_str())
            .ok_or("no AuthDataJson claim")?;
        serde_json::from_str(json).map_err(|e| format!("AuthDataJson: {e}"))
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The key from appsettings.json - a development key, published in the repository.
    const DEV_KEY: &str = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";

    #[test]
    fn md5_matches_the_stored_hashes() {
        // The access code of the seed tours, and the hash they are stored under.
        assert_eq!(code_md5("test"), "098F6BCD4621D373CADE4E832627B4F6");
        assert_eq!(code_md5(""), "D41D8CD98F00B204E9800998ECF8427E");
    }

    #[test]
    fn a_token_reads_back() {
        let s = Signer_::from_base64(DEV_KEY).unwrap();
        let auth = AuthData::for_code_md5("ABC");
        let token = s.issue("code", &auth, 60);
        assert_eq!(s.verify(&token).unwrap(), auth);
    }

    #[test]
    fn a_tampered_token_does_not() {
        let s = Signer_::from_base64(DEV_KEY).unwrap();
        let token = s.issue("code", &AuthData::for_code_md5("ABC"), 60);
        let mut bad = token.clone();
        bad.pop();
        bad.push('A');
        assert!(s.verify(&bad).is_err());
    }

    #[test]
    fn an_expired_token_does_not() {
        let s = Signer_::from_base64(DEV_KEY).unwrap();
        let token = s.issue("code", &AuthData::master(), -10);
        assert_eq!(s.verify(&token), Err("expired".to_owned()));
    }

    #[test]
    fn master_sees_everything_and_a_code_sees_its_own() {
        assert!(AuthData::master().may_see("whatever"));
        let a = AuthData::for_code_md5("AAA;BBB");
        assert!(a.may_see("AAA"));
        assert!(a.may_see("BBB"));
        assert!(!a.may_see("CCC"));
        assert!(!AuthData::default().may_see("AAA"));
    }
}
