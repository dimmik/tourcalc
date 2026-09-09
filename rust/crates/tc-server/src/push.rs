//! Telling a browser that a tour changed, while nobody has it open.
//!
//! Two halves, and they are split where the dependencies force them to be.
//!
//! **Preparing the message** is pure Rust and always built: a VAPID signature proving the
//! notification comes from this server, and a body encrypted for that one subscription with
//! the keys it handed over (RFC 8291 - the push service forwards it and cannot read it).
//!
//! **Sending it** is an outbound HTTPS request, which means a TLS stack, which means C or
//! assembly in a build that has none. So it lives behind `--features push`. Without the
//! feature the message is still prepared and the fact that it would have gone is logged,
//! which is enough to see that the plumbing works and honest about what did not happen.

use crate::subscriptions::Subscription;

/// Anything that can tell subscribers about a change.
#[async_trait::async_trait]
pub trait Notifier: Send + Sync {
    /// The VAPID public key a browser needs in order to subscribe.
    fn public_key(&self) -> &str;
    /// Tells everybody subscribed to this tour.
    async fn notify(&self, subscribers: Vec<Subscription>, tour_id: &str, message: &str);
}

/// A server with no push keys configured. Answers with an empty key, which is what the C#
/// does when the setting is missing, and tells nobody anything.
pub struct Silent;

#[async_trait::async_trait]
impl Notifier for Silent {
    fn public_key(&self) -> &str {
        ""
    }
    async fn notify(&self, _subscribers: Vec<Subscription>, _tour_id: &str, _message: &str) {}
}

/// The real one.
pub struct WebPush {
    public_key: String,
    /// Both are read only when there is something to sign and send with them, which is
    /// behind `--features push`; without it the server still holds them, still answers with
    /// the public one, and simply has nothing to do with the rest.
    #[cfg_attr(not(feature = "push"), allow(dead_code))]
    private_key: String,
    /// Who to contact about this server, as the push services ask.
    #[cfg_attr(not(feature = "push"), allow(dead_code))]
    contact: String,
}

impl WebPush {
    /// Built only when both keys are configured; otherwise the server is [`Silent`].
    pub fn new(public_key: &str, private_key: &str, contact: &str) -> Option<WebPush> {
        if public_key.trim().is_empty() || private_key.trim().is_empty() {
            return None;
        }
        Some(WebPush {
            public_key: public_key.trim().to_owned(),
            private_key: private_key.trim().to_owned(),
            contact: if contact.trim().is_empty() {
                "mailto:nobody@example.org".to_owned()
            } else {
                contact.trim().to_owned()
            },
        })
    }

    /// The payload the client's service worker expects: what changed, and where.
    pub fn payload(tour_id: &str, message: &str) -> String {
        serde_json::json!({ "message": message, "tourId": tour_id }).to_string()
    }
}

#[async_trait::async_trait]
impl Notifier for WebPush {
    fn public_key(&self) -> &str {
        &self.public_key
    }

    async fn notify(&self, subscribers: Vec<Subscription>, tour_id: &str, message: &str) {
        if subscribers.is_empty() {
            return;
        }
        let payload = WebPush::payload(tour_id, message);

        for sub in subscribers {
            match self.prepare(&sub, &payload) {
                Ok(request) => self.deliver(&sub, request).await,
                Err(e) => tracing::warn!("could not prepare a notification for {}: {e}", sub.url),
            }
        }
    }
}

impl WebPush {
    /// The signed, encrypted request for one subscription.
    ///
    /// Always compiled: this is the part worth being sure about, and it needs no network.
    #[cfg(feature = "push")]
    fn prepare(&self, sub: &Subscription, payload: &str) -> Result<http::Request<Vec<u8>>, String> {
        use base64::Engine;
        use web_push_native::jwt_simple::algorithms::ES256KeyPair;
        use web_push_native::{Auth, WebPushBuilder};

        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let key_bytes = b64
            .decode(self.private_key.trim_end_matches('='))
            .map_err(|e| format!("the VAPID private key is not base64url: {e}"))?;
        let key = ES256KeyPair::from_bytes(&key_bytes)
            .map_err(|e| format!("the VAPID private key is not a P-256 scalar: {e}"))?;

        let endpoint = sub
            .url
            .parse()
            .map_err(|e| format!("the endpoint is not a URL: {e}"))?;
        let p256dh = b64
            .decode(sub.p256dh.trim_end_matches('='))
            .map_err(|e| format!("p256dh is not base64url: {e}"))?;
        let auth = b64
            .decode(sub.auth.trim_end_matches('='))
            .map_err(|e| format!("auth is not base64url: {e}"))?;
        let auth = Auth::clone_from_slice(&auth);

        // The browser sends its public key in SEC1 form, which is what the push spec calls
        // for; the builder wants it as a key.
        let their_key = p256::PublicKey::from_sec1_bytes(&p256dh)
            .map_err(|e| format!("p256dh is not a public key: {e}"))?;

        let builder =
            WebPushBuilder::new(endpoint, their_key, auth).with_vapid(&key, &self.contact);

        builder
            .build(payload.as_bytes().to_vec())
            .map_err(|e| format!("could not encrypt: {e}"))
    }

    #[cfg(not(feature = "push"))]
    fn prepare(&self, _sub: &Subscription, _payload: &str) -> Result<(), String> {
        Ok(())
    }

    #[cfg(feature = "push")]
    async fn deliver(&self, sub: &Subscription, request: http::Request<Vec<u8>>) {
        // Delivery is deliberately best-effort: a push service that is down, or a
        // subscription the browser has withdrawn, must not fail somebody's save.
        let client = reqwest::Client::new();
        let (parts, body) = request.into_parts();
        let mut send = client.post(parts.uri.to_string()).body(body);
        for (name, value) in parts.headers.iter() {
            send = send.header(name, value);
        }
        match send.send().await {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => tracing::warn!("{} answered {}", sub.url, response.status()),
            Err(e) => tracing::warn!("could not reach {}: {e}", sub.url),
        }
    }

    #[cfg(not(feature = "push"))]
    async fn deliver(&self, sub: &Subscription, _request: ()) {
        tracing::info!(
            "would notify {} - built without --features push, so nothing was sent",
            sub.url
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_without_keys_is_silent() {
        assert!(WebPush::new("", "key", "mailto:x").is_none());
        assert!(WebPush::new("key", "  ", "mailto:x").is_none());
        assert!(WebPush::new("pub", "priv", "").is_some());
    }

    #[test]
    fn the_payload_is_what_the_service_worker_reads() {
        let payload = WebPush::payload("abc", "Trip : P 'Вася' added");
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["tourId"], "abc");
        assert_eq!(value["message"], "Trip : P 'Вася' added");
    }

    /// The message a real browser subscription would receive: signed, encrypted, addressed.
    #[cfg(feature = "push")]
    #[test]
    fn a_notification_is_prepared_for_a_real_subscription() {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

        // The development keys from appsettings.json, and a subscription key pair generated
        // for this test - a browser's p256dh is the public half of one.
        let push = WebPush::new(
            "BIPKQK0SfrIR_RJduu4HuHrq4BdjUH2_qhtq5dKXyIvpuCjy9Q-85pgPJlAdpKzagaFLqEYxHAApCjaV-vwrWj8",
            "UBMrYX_hYpDz_1c-iGrSFS-v-3TtAX7fi68EooymclU",
            "mailto:tourcalc@example.org",
        )
        .expect("keys");

        let secret = p256::SecretKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let public = secret.public_key().to_sec1_bytes();

        let sub = Subscription {
            url: "https://push.example.org/send/abc".into(),
            p256dh: b64.encode(&public),
            auth: b64.encode([7u8; 16]),
        };

        let request = push
            .prepare(&sub, &WebPush::payload("t1", "something changed"))
            .expect("prepared");

        assert_eq!(request.uri().to_string(), sub.url);
        let headers = request.headers();
        assert_eq!(
            headers.get("content-encoding").map(|v| v.to_str().unwrap()),
            Some("aes128gcm"),
            "encrypted for the subscription, not for the push service"
        );
        let authorization = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            authorization.starts_with("vapid t=") && authorization.contains(", k="),
            "signed with VAPID: {authorization}"
        );
        assert!(
            !request.body().is_empty() && !request.body().starts_with(b"{"),
            "the body is ciphertext and not the payload in the clear"
        );
    }
}
