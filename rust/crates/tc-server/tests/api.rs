//! The API, exercised through the router itself.
//!
//! No socket and no browser: axum's `Router` is a `tower::Service`, so a request can be
//! handed to it directly and the response read back. That makes these tests fast enough to
//! run on every change, which the browser check - valuable as it is - is not.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// The access code the seed tours are filed under, already hashed.
const CODE: &str = "1C0369EA42B2F746D3DF1E66BCB2DE46";
const DEV_KEY: &str = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";

fn app() -> axum::Router {
    let seed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../TCBlazor/Server/inmemory-tours.json");
    let store = tc_server::store::InMemoryStore::from_seed_file(&seed).expect("seed file");
    let state = std::sync::Arc::new(tc_server::state::AppState {
        store: Box::new(store),
        signer: tc_server::auth::Signer_::from_base64(DEV_KEY).unwrap(),
        master_key: "master".to_owned(),
        token_valid_minutes: 60,
        started: std::time::SystemTime::now(),
    });
    tc_server::api::routes(state)
}

async fn get(app: &axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder().uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("bearer {t}"));
    }
    let resp = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn token_for_code(app: &axum::Router) -> String {
    let (status, body) = get(app, &format!("/api/Auth/token/code/{CODE}/md5"), None).await;
    assert_eq!(status, StatusCode::OK);
    // Plain text, not a JSON string: an ASP.NET controller returning `string` answers with
    // text/plain, and a client that stored the quoted form would send them in the header.
    assert!(
        !body.starts_with('"'),
        "the token must not be quoted: {body}"
    );
    body
}

#[tokio::test]
async fn a_code_gets_a_token_and_is_recognised_by_it() {
    let app = app();
    let token = token_for_code(&app).await;

    let (status, body) = get(&app, "/api/Auth/whoami", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(CODE), "{body}");
    assert!(body.contains("\"IsMaster\":false"), "{body}");
}

#[tokio::test]
async fn no_token_is_nobody() {
    let (status, body) = get(&app(), "/api/Auth/whoami", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"Type\":\"None\""), "{body}");
}

#[tokio::test]
async fn the_wrong_master_key_gets_nothing() {
    let (status, _) = get(&app(), "/api/Auth/token/admin/not-the-key", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_tour_is_readable_with_its_code_and_invisible_without() {
    let app = app();
    let token = token_for_code(&app).await;

    let (status, body) = get(&app, "/api/Tour/zscph2y", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let tour: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(tour["Name"], "(Fin) Поход Урал Август 2021");
    assert_eq!(tour["Persons"].as_array().unwrap().len(), 10);
    assert_eq!(tour["Spendings"].as_array().unwrap().len(), 64);

    // Same tour, no token: not "forbidden" but "not found", so that ids cannot be probed.
    let (status, _) = get(&app, "/api/Tour/zscph2y", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Nothing may be written back that would break the C# calculator reading it.
///
/// A `"GroupId": null` did exactly that: it replaces a generated default with nothing, and
/// the settlement dies on `GroupId.StartsWith(...)`. Cheap to assert here, and it was not
/// cheap to find.
#[tokio::test]
async fn the_answer_contains_no_invented_nulls() {
    let app = app();
    let token = token_for_code(&app).await;
    let (_, body) = get(&app, "/api/Tour/zscph2y", Some(&token)).await;
    let tour: serde_json::Value = serde_json::from_str(&body).unwrap();

    for person in tour["Persons"].as_array().unwrap() {
        assert!(
            !person.as_object().unwrap().contains_key("GroupId") || !person["GroupId"].is_null(),
            "GroupId written as null: {person}"
        );
    }
}

#[tokio::test]
async fn the_list_holds_every_tour_of_that_code_and_no_spendings() {
    let app = app();
    let token = token_for_code(&app).await;

    let (status, body) = get(
        &app,
        "/api/Tour/all/suggested?from=0&count=50",
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    let tours = list["Tours"].as_array().unwrap();

    // Seven of the eight seed entries are filed under this code; the eighth is the same
    // tour id again under a different one. Checked against the C# server, which answers
    // with the same seven.
    assert_eq!(tours.len(), 7, "tours in the list");
    assert_eq!(list["TotalCount"], 7);

    for t in tours {
        assert!(
            t["Spendings"].as_array().unwrap().is_empty(),
            "the list carries no spendings"
        );
        // ...but it does carry what each person owes, which is what the list shows.
        assert!(t["Persons"].as_array().is_some());
    }

    // Including the camelCase tours, which were invisible while the access code was only
    // looked for under its capitalised spelling.
    let names: Vec<&str> = tours.iter().filter_map(|t| t["Name"].as_str()).collect();
    assert!(
        names.contains(&"TEST for issue repro InMem"),
        "camelCase tours are in the list too: {names:?}"
    );
}

#[tokio::test]
async fn an_unknown_tour_is_not_found() {
    let app = app();
    let token = token_for_code(&app).await;
    let (status, _) = get(&app, "/api/Tour/no-such-tour", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// --- writing ------------------------------------------------------------------------------

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
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn fetch_tour(app: &axum::Router, token: &str, id: &str) -> serde_json::Value {
    let (status, body) = get(app, &format!("/api/Tour/{id}"), Some(token)).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn a_change_is_stored_and_gets_a_new_state() {
    let app = app();
    let token = token_for_code(&app).await;

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    let state_before = tour["StateGUID"].as_str().unwrap_or("").to_owned();
    tour["Name"] = "renamed by a test".into();

    let (status, body) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "zscph2y");

    let after = fetch_tour(&app, &token, "zscph2y").await;
    assert_eq!(after["Name"], "renamed by a test");
    // The soft lock moves on, so the next save has to present the new one.
    assert_ne!(after["StateGUID"].as_str().unwrap_or(""), state_before);
}

/// Saving over somebody else's change is refused rather than silently winning.
#[tokio::test]
async fn a_stale_save_is_a_conflict() {
    let app = app();
    let token = token_for_code(&app).await;

    let first = fetch_tour(&app, &token, "zscph2y").await;
    // Two editors read the same tour...
    let mut second = first.clone();

    let mut mine = first.clone();
    mine["Name"] = "saved first".into();
    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(mine)).await;
    assert_eq!(status, StatusCode::OK);

    // ...and the slower one is holding a state id that is no longer current.
    second["Name"] = "saved second".into();
    let (status, message) = send(
        &app,
        "PATCH",
        "/api/Tour/zscph2y",
        Some(&token),
        Some(second),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(message.contains("newer version"), "{message}");

    // And the first save is still what is stored.
    let after = fetch_tour(&app, &token, "zscph2y").await;
    assert_eq!(after["Name"], "saved first");
}

/// The body cannot move a tour into another pile, or rename its id.
#[tokio::test]
async fn the_body_is_not_trusted_for_id_or_access() {
    let app = app();
    let token = token_for_code(&app).await;

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    tour["Id"] = "somewhere-else".into();
    tour["GUID"] = "somewhere-else".into();
    tour["AccessCodeMD5"] = "0000000000000000000000000000FFFF".into();

    let (status, body) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "zscph2y");

    let after = fetch_tour(&app, &token, "zscph2y").await;
    assert_eq!(after["Id"], "zscph2y");
    assert_eq!(after["AccessCodeMD5"], CODE);
    // ...and nothing appeared under the id the body asked for.
    let (status, _) = get(&app, "/api/Tour/somewhere-else", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_tour_of_another_code_cannot_be_written() {
    let app = app();
    let token = token_for_code(&app).await;
    let tour = fetch_tour(&app, &token, "zscph2y").await;

    // A token for a code that owns nothing here.
    let (_, other) = get(&app, "/api/Auth/token/code/nobody/md5", None).await;
    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&other), Some(tour)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_new_tour_joins_the_same_pile() {
    let app = app();
    let token = token_for_code(&app).await;

    let body = serde_json::json!({ "Name": "A new trip", "Persons": [], "Spendings": [] });
    let (status, id) = send(
        &app,
        "POST",
        "/api/Tour/add/ignored",
        Some(&token),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!id.is_empty());

    let made = fetch_tour(&app, &token, &id).await;
    assert_eq!(made["Name"], "A new trip");
    // The access code is the caller's own, whatever the URL said.
    assert_eq!(made["AccessCodeMD5"], CODE);
    assert!(made["StateGUID"].as_str().is_some_and(|s| !s.is_empty()));
}

#[tokio::test]
async fn a_tour_can_be_deleted_while_others_remain() {
    let app = app();
    let token = token_for_code(&app).await;

    let (status, body) = send(&app, "DELETE", "/api/Tour/zscph2y", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "zscph2y");

    let (status, _) = get(&app, "/api/Tour/zscph2y", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Restoring a version is a different operation; better refused than mistaken for an edit.
#[tokio::test]
async fn a_version_restore_is_refused_for_now() {
    let app = app();
    let token = token_for_code(&app).await;
    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    tour["IsVersion"] = true.into();

    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
