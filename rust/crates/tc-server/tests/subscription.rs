//! Subscribing to a tour's changes, and being told about them.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

const CODE: &str = "1C0369EA42B2F746D3DF1E66BCB2DE46";
const DEV_KEY: &str = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";

fn state_with(push: Box<dyn tc_server::push::Notifier>) -> tc_server::state::Shared {
    let seed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../TCBlazor/Server/inmemory-tours.json");
    let store = tc_server::store::InMemoryStore::from_seed_file(&seed).expect("seed file");
    Arc::new(tc_server::state::AppState {
        store: Box::new(store),
        subscriptions: Box::new(tc_server::subscriptions::InMemorySubscriptions::default()),
        push,
        signer: tc_server::auth::Signer_::from_base64(DEV_KEY).unwrap(),
        master_key: "master".to_owned(),
        token_valid_minutes: 60,
        started: std::time::SystemTime::now(),
        versioning: true,
        version_editable: false,
        max_tours_per_code: -1,
        wakeup_code: "secCode".into(),
        wakeup_pre_delay_min: 0,
        wakeup_post_delay_min: 0,
        wakeups: Default::default(),
    })
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("bearer {t}"));
    }
    let req = match body {
        Some(v) => req
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn token(app: &axum::Router) -> String {
    let (status, body) = send(
        app,
        "GET",
        &format!("/api/Auth/token/code/{CODE}/md5"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

fn subscription(url: &str) -> serde_json::Value {
    serde_json::json!({ "Url": url, "P256dh": "key", "Auth": "auth" })
}

/// Subscribing, asking whether one is subscribed, and stopping.
#[tokio::test]
async fn a_browser_can_subscribe_and_stop() {
    let app = tc_server::api::routes(state_with(Box::new(tc_server::push::Silent)));
    let token = token(&app).await;
    let sub = subscription("https://push.example.org/a");

    let (status, body) = send(
        &app,
        "POST",
        "/api/Subscription/check/zscph2y",
        Some(&token),
        Some(sub.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "false", "nobody is subscribed yet");

    let (status, body) = send(
        &app,
        "POST",
        "/api/Subscription/subscribe/zscph2y",
        Some(&token),
        Some(sub.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "OK");

    let (_, body) = send(
        &app,
        "POST",
        "/api/Subscription/check/zscph2y",
        Some(&token),
        Some(sub.clone()),
    )
    .await;
    assert_eq!(body, "true");

    send(
        &app,
        "POST",
        "/api/Subscription/unsubscribe/zscph2y",
        Some(&token),
        Some(sub.clone()),
    )
    .await;
    let (_, body) = send(
        &app,
        "POST",
        "/api/Subscription/check/zscph2y",
        Some(&token),
        Some(sub),
    )
    .await;
    assert_eq!(body, "false");
}

/// A subscription is only for a tour the token may see.
///
/// The C# checks that the caller has *a* token and no more, so anybody could subscribe to
/// any tour id they guessed - and a notification carries the tour's name and what changed.
#[tokio::test]
async fn nobody_subscribes_to_somebody_elses_tour() {
    let app = tc_server::api::routes(state_with(Box::new(tc_server::push::Silent)));
    let sub = subscription("https://push.example.org/a");

    let (status, _) = send(
        &app,
        "POST",
        "/api/Subscription/subscribe/zscph2y",
        None,
        Some(sub.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no token, no tour");

    let (_, other) = send(
        &app,
        "GET",
        "/api/Auth/token/code/some-other-code",
        None,
        None,
    )
    .await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/Subscription/subscribe/zscph2y",
        Some(&other),
        Some(sub),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "another code, no tour");
}

/// The public key is what a browser needs before it can subscribe at all.
#[tokio::test]
async fn the_public_key_is_public() {
    let push = tc_server::push::WebPush::new("the-public-key", "the-private-key", "mailto:x")
        .expect("keys");
    let app = tc_server::api::routes(state_with(Box::new(push)));

    let (status, body) = send(&app, "GET", "/api/Subscription/publickey", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "the-public-key");
}

/// A save tells the subscribers what changed.
///
/// The notifier here only records what it was asked to send; that the message is really
/// encrypted and signed is the business of the unit test in `push`, and that it really
/// arrives is `a_notification_reaches_the_push_service` below.
#[tokio::test]
async fn saving_a_tour_tells_the_subscribers() {
    // A local type, so the trait can be implemented for it: `impl Trait for Arc<Recorder>`
    // is not allowed when both halves come from elsewhere.
    #[derive(Default, Clone)]
    struct Recorder(Arc<std::sync::Mutex<Vec<String>>>);

    #[async_trait::async_trait]
    impl tc_server::push::Notifier for Recorder {
        fn public_key(&self) -> &str {
            "k"
        }
        async fn notify(
            &self,
            subscribers: Vec<tc_server::subscriptions::Subscription>,
            _tour: &str,
            message: &str,
        ) {
            for _ in subscribers {
                self.0.lock().unwrap().push(message.to_owned());
            }
        }
    }

    let recorder = Recorder::default();
    let state = state_with(Box::new(recorder.clone()));
    let app = tc_server::api::routes(Arc::clone(&state));
    let token = token(&app).await;

    send(
        &app,
        "POST",
        "/api/Subscription/subscribe/zscph2y",
        Some(&token),
        Some(subscription("https://push.example.org/a")),
    )
    .await;

    let (_, body) = send(&app, "GET", "/api/Tour/zscph2y", Some(&token), None).await;
    let mut tour: serde_json::Value = serde_json::from_str(&body).unwrap();
    tour["Name"] = "Renamed for the neighbours".into();
    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::OK);

    // The telling is spawned so that a slow push service cannot slow a save down.
    for _ in 0..50 {
        if !recorder.0.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let said = recorder.0.lock().unwrap().clone();
    assert_eq!(said.len(), 1, "one subscriber, one notification");
    assert!(
        said[0].starts_with("Renamed for the neighbours : Changed: Tour Name"),
        "it says which tour and what changed: {}",
        said[0]
    );

    // And a save that changes nothing tells nobody.
    let (_, body) = send(&app, "GET", "/api/Tour/zscph2y", Some(&token), None).await;
    let tour: serde_json::Value = serde_json::from_str(&body).unwrap();
    send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(recorder.0.lock().unwrap().len(), 1, "still just the one");
}

/// The whole way: a save, an encrypted notification, and a push service that receives it.
///
/// The "push service" is a listener started by the test, so this exercises the real
/// preparation and the real HTTP request - everything except somebody else's TLS.
#[cfg(feature = "push")]
#[tokio::test]
async fn a_notification_reaches_the_push_service() {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    // A push service that writes down what it is sent: the content-encoding and the body.
    type Delivered = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;
    let seen: Delivered = Arc::default();
    let taking = Arc::clone(&seen);
    let service = axum::Router::new().route(
        "/send/{id}",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                let taking = Arc::clone(&taking);
                async move {
                    let encoding = headers
                        .get("content-encoding")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    taking.lock().unwrap().push((encoding, body.to_vec()));
                    StatusCode::CREATED
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, service).await.unwrap() });

    let push = tc_server::push::WebPush::new(
        "BIPKQK0SfrIR_RJduu4HuHrq4BdjUH2_qhtq5dKXyIvpuCjy9Q-85pgPJlAdpKzagaFLqEYxHAApCjaV-vwrWj8",
        "UBMrYX_hYpDz_1c-iGrSFS-v-3TtAX7fi68EooymclU",
        "mailto:tourcalc@example.org",
    )
    .expect("keys");

    let state = state_with(Box::new(push));
    let app = tc_server::api::routes(Arc::clone(&state));
    let token = token(&app).await;

    let secret = p256::SecretKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
    let sub = serde_json::json!({
        "Url": format!("http://127.0.0.1:{port}/send/one"),
        "P256dh": b64.encode(secret.public_key().to_sec1_bytes()),
        "Auth": b64.encode([3u8; 16]),
    });
    send(
        &app,
        "POST",
        "/api/Subscription/subscribe/zscph2y",
        Some(&token),
        Some(sub),
    )
    .await;

    let (_, body) = send(&app, "GET", "/api/Tour/zscph2y", Some(&token), None).await;
    let mut tour: serde_json::Value = serde_json::from_str(&body).unwrap();
    tour["Name"] = "Somebody changed this".into();
    send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;

    for _ in 0..100 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let arrived = seen.lock().unwrap().clone();
    assert_eq!(arrived.len(), 1, "the push service was sent one thing");
    let (encoding, body) = &arrived[0];
    assert_eq!(encoding, "aes128gcm");
    assert!(!body.is_empty());
    assert!(
        !String::from_utf8_lossy(body).contains("Somebody changed this"),
        "the push service cannot read what it carries"
    );
}
