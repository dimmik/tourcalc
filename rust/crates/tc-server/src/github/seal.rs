//! What a tour looks like in a public repository: a name that says nothing, and contents
//! nobody without the key can read or change unnoticed.
//!
//! One key, `GitHubStoreKey` - 32 random bytes, base64 - and two keys made from it, so that
//! the one that names files is never the one that encrypts them:
//!
//! - **names**: a file is `t/<HMAC(id)>` (a tour) or `v/<HMAC(tour)>/<HMAC(version)>`, so the
//!   ids in `/goto/…` links cannot be matched with files;
//! - **sealing**: the tour's JSON, deflated, then XChaCha20-Poly1305 with a random nonce of its
//!   own. The path is part of what is authenticated, so a file moved under another name - one
//!   tour's contents put where another's were - does not open.
//!
//! A file is one line of base64: `TCE1`, a key number (for changing the key one day), the
//! nonce, the ciphertext. Text rather than bytes because GitHub's tree API takes a file's
//! contents as a string, and that is what lets a whole batch be one request.

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::Sha256;

const MAGIC: &[u8; 4] = b"TCE1";
/// The key a file was sealed with. One for now; a second is how a key would be changed -
/// new files sealed with it, old ones still opened with the first.
const KEY_ID: u8 = 1;
const NONCE: usize = 24;

/// Where the push subscriptions are: one file, all of them.
pub const SUBSCRIPTIONS: &str = "s";

pub struct Keys {
    seal: XChaCha20Poly1305,
    names: [u8; 32],
}

/// The setting is missing, is not base64, or is not 32 bytes.
#[derive(Debug, PartialEq)]
pub struct BadKey(pub String);

/// A file that does not open: not ours, damaged, moved, or sealed with another key.
#[derive(Debug, PartialEq)]
pub struct Unreadable(pub String);

impl Keys {
    pub fn from_setting(b64: &str) -> Result<Keys, BadKey> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|e| BadKey(format!("GitHubStoreKey is not base64: {e}")))?;
        let master: [u8; 32] = raw.try_into().map_err(|r: Vec<u8>| {
            BadKey(format!(
                "GitHubStoreKey is {} bytes, it must be 32 (`openssl rand -base64 32`)",
                r.len()
            ))
        })?;
        Ok(Keys {
            seal: XChaCha20Poly1305::new(&derive(&master, b"tourcalc seal").into()),
            names: derive(&master, b"tourcalc names"),
        })
    }

    /// 32 hex characters that stand for `id` and nothing else, and say nothing about it.
    fn name(&self, kind: &str, id: &str) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.names).expect("any length");
        mac.update(kind.as_bytes());
        mac.update(b":");
        mac.update(id.as_bytes());
        hex(&mac.finalize().into_bytes()[..16])
    }

    pub fn tour_path(&self, id: &str) -> String {
        format!("t/{}", self.name("tour", id))
    }

    /// Under its tour, so that the history of one tour is one directory.
    pub fn version_path(&self, tour: &str, version: &str) -> String {
        format!("v/{}/{}", self.name("tour", tour), self.name("version", version))
    }

    /// The file's contents, as they go into the repository.
    pub fn seal(&self, path: &str, plain: &[u8]) -> String {
        let packed = miniz_oxide::deflate::compress_to_vec(plain, 6);
        let mut nonce = [0u8; NONCE];
        use p256::elliptic_curve::rand_core::RngCore;
        p256::elliptic_curve::rand_core::OsRng.fill_bytes(&mut nonce);
        let sealed = self
            .seal
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: &packed, aad: &aad(path) })
            .expect("XChaCha20-Poly1305 does not fail on a message this size");
        let mut out = Vec::with_capacity(MAGIC.len() + 1 + NONCE + sealed.len());
        out.extend_from_slice(MAGIC);
        out.push(KEY_ID);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        let mut text = base64::engine::general_purpose::STANDARD.encode(out);
        text.push('\n');
        text
    }

    pub fn open(&self, path: &str, text: &str) -> Result<Vec<u8>, Unreadable> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(text.trim())
            .map_err(|_| Unreadable(format!("{path}: not base64")))?;
        let head = MAGIC.len() + 1 + NONCE;
        if raw.len() < head || &raw[..MAGIC.len()] != MAGIC {
            return Err(Unreadable(format!("{path}: not a sealed tour")));
        }
        if raw[MAGIC.len()] != KEY_ID {
            return Err(Unreadable(format!("{path}: sealed with key {}", raw[MAGIC.len()])));
        }
        let nonce = XNonce::from_slice(&raw[MAGIC.len() + 1..head]);
        let packed = self
            .seal
            .decrypt(nonce, Payload { msg: &raw[head..], aad: &aad(path) })
            .map_err(|_| Unreadable(format!("{path}: does not open - another key, or not this file")))?;
        miniz_oxide::inflate::decompress_to_vec(&packed)
            .map_err(|_| Unreadable(format!("{path}: opens, but does not inflate")))
    }
}

/// What the ciphertext is bound to besides the key: the format and the file's own path.
fn aad(path: &str) -> Vec<u8> {
    let mut a = Vec::with_capacity(MAGIC.len() + 1 + path.len());
    a.extend_from_slice(MAGIC);
    a.push(KEY_ID);
    a.extend_from_slice(path.as_bytes());
    a
}

fn derive(master: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(master).expect("any length");
    mac.update(label);
    mac.finalize().into_bytes().into()
}

/// The id git gives these contents - what a tree lists for a file - so that a file already on
/// disk is known to be the one in the repository without downloading it.
pub fn blob_sha(contents: &[u8]) -> String {
    use sha1::Digest;
    let mut h = sha1::Sha1::new();
    h.update(format!("blob {}\0", contents.len()).as_bytes());
    h.update(contents);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    const OTHER: &str = "HxwdHhkaGxwVFhcYERITFA0ODxAJCgsMBQYHCAECAwQ=";

    #[test]
    fn a_sealed_tour_opens_as_it_was() {
        let k = Keys::from_setting(KEY).unwrap();
        let plain = br#"{"Name":"Ural","Spendings":[]}"#.repeat(50);
        let path = k.tour_path("zscph2y");
        let text = k.seal(&path, &plain);
        assert!(text.ends_with('\n') && !text.trim().contains('\n'), "one line");
        assert!(!text.contains("Ural"));
        assert_eq!(k.open(&path, &text).unwrap(), plain);
    }

    #[test]
    fn the_same_tour_sealed_twice_is_two_different_files() {
        let k = Keys::from_setting(KEY).unwrap();
        let path = k.tour_path("a");
        assert_ne!(k.seal(&path, b"same"), k.seal(&path, b"same"), "a nonce of its own");
    }

    #[test]
    fn another_key_or_another_path_does_not_open_it() {
        let k = Keys::from_setting(KEY).unwrap();
        let text = k.seal(&k.tour_path("a"), b"secret");
        assert!(Keys::from_setting(OTHER).unwrap().open(&k.tour_path("a"), &text).is_err());
        // Moved: one tour's contents put where another's are.
        assert!(k.open(&k.tour_path("b"), &text).is_err());
        // Changed by a byte.
        let mut raw = base64::engine::general_purpose::STANDARD.decode(text.trim()).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 1;
        let tampered = base64::engine::general_purpose::STANDARD.encode(raw);
        assert!(k.open(&k.tour_path("a"), &tampered).is_err());
        assert!(k.open(&k.tour_path("a"), "hello").is_err());
    }

    #[test]
    fn names_say_nothing_and_stay_the_same() {
        let k = Keys::from_setting(KEY).unwrap();
        let p = k.tour_path("zscph2y");
        assert_eq!(p, k.tour_path("zscph2y"));
        assert_ne!(p, k.tour_path("zscph2z"));
        assert!(!p.contains("zscph2y"));
        assert_eq!(p.len(), "t/".len() + 32);
        let v = k.version_path("zscph2y", "v1");
        assert!(v.starts_with(&format!("v/{}/", &p[2..])), "a tour's versions under its name");
        assert_ne!(Keys::from_setting(OTHER).unwrap().tour_path("zscph2y"), p);
    }

    #[test]
    fn a_key_must_be_32_bytes_of_base64() {
        assert!(Keys::from_setting("not base64!").is_err());
        assert!(Keys::from_setting("AAEC").is_err());
        assert!(Keys::from_setting(&format!("  {KEY}\n")).is_ok());
    }

    #[test]
    fn the_blob_id_is_gits() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(blob_sha(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
    }
}
