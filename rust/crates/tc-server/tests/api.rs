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
    app_with(|_| {})
}

/// The same server with one setting changed, for the tests that are about a setting.
fn app_with(tweak: impl FnOnce(&mut tc_server::state::AppState)) -> axum::Router {
    let seed = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../TCBlazor/Server/inmemory-tours.json");
    let store = tc_server::store::InMemoryStore::from_seed_file(&seed).expect("seed file");
    let state = tc_server::state::AppState {
        store: Box::new(store),
        subscriptions: Box::new(tc_server::subscriptions::InMemorySubscriptions::default()),
        push: Box::new(tc_server::push::Silent),
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
        build_type: "test".to_owned(),
        build_id: "test".to_owned(),
        build_commit: String::new(),
        client_asset: None,
        wakeups: Default::default(),
    };
    let mut state = state;
    tweak(&mut state);
    tc_server::api::routes(std::sync::Arc::new(state))
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

/// A token for the administrator, who is not subject to the rules the codes are.
async fn token_for_admin(app: &axum::Router) -> String {
    let (status, body) = get(app, "/api/Auth/token/admin/master", None).await;
    assert_eq!(status, StatusCode::OK);
    body
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

/// Saving keeps what it replaced, with a line saying what the save did.
#[tokio::test]
async fn a_save_keeps_the_state_it_replaced() {
    let app = app();
    let token = token_for_code(&app).await;

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    let was = tour["Name"].as_str().unwrap().to_owned();
    tour["Name"] = "Renamed once".into();
    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get(&app, "/api/Tour/zscph2y/versions", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(list["TotalCount"], 1, "one save, one version: {body}");
    let version = &list["Tours"][0];
    assert_eq!(version["Name"], was, "the version holds what was replaced");
    assert!(
        version["VersionComment"]
            .as_str()
            .unwrap_or_default()
            .contains(&was),
        "the comment names the change: {}",
        version["VersionComment"]
    );
    // The list screen shows a date and a comment, so the contents are left out.
    assert_eq!(version["Persons"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(version["Spendings"].as_array().map(|a| a.len()), Some(0));
}

/// A save that changed nothing anybody can name leaves no version.
///
/// Otherwise every reopened tour and every re-saved form would add a line to the history,
/// and the history would be useless by the end of the week.
#[tokio::test]
async fn saving_the_same_tour_again_keeps_nothing() {
    let app = app();
    let token = token_for_code(&app).await;

    let tour = fetch_tour(&app, &token, "zscph2y").await;
    let (status, _) = send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = get(&app, "/api/Tour/zscph2y/versions", Some(&token)).await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        list["TotalCount"], 0,
        "nothing changed, nothing kept: {body}"
    );
}

/// Restoring a version puts the old state back, and says so in the history.
#[tokio::test]
async fn a_version_can_be_restored() {
    let app = app();
    let token = token_for_code(&app).await;

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    let original = tour["Name"].as_str().unwrap().to_owned();
    tour["Name"] = "A name nobody wanted".into();
    send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;

    // Take the kept copy and send it back as it is: that is what restoring is.
    let (_, body) = get(&app, "/api/Tour/zscph2y/versions", Some(&token)).await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    let version_id = list["Tours"][0]["GUID"].as_str().unwrap().to_owned();

    let mut restore = fetch_tour(&app, &token, "zscph2y").await;
    // A real client sends the version record back as it came. `Id` and `GUID` are one field
    // in the C# - `Id` is a property over `GUID` - so both carry the version's own id, and a
    // body that set only one of them would not be a restore at all.
    restore["Id"] = version_id.clone().into();
    restore["GUID"] = version_id.into();
    restore["IsVersion"] = true.into();
    restore["Name"] = original.clone().into();

    let (status, _) = send(
        &app,
        "PATCH",
        "/api/Tour/zscph2y",
        Some(&token),
        Some(restore),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let back = fetch_tour(&app, &token, "zscph2y").await;
    assert_eq!(back["Name"], serde_json::Value::from(original));
    assert_eq!(
        back["IsVersion"],
        serde_json::Value::Bool(false),
        "what came back is the tour, not a version of it"
    );

    let (_, body) = get(&app, "/api/Tour/zscph2y/versions", Some(&token)).await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list["TotalCount"], 2, "the undone state is kept too");
    assert!(
        list["Tours"][0]["VersionComment"]
            .as_str()
            .unwrap_or_default()
            .starts_with("Tour Restored to"),
        "the restore names itself: {body}"
    );
}

/// A version is history, not a tour: it never shows up in anybody's list.
#[tokio::test]
async fn versions_are_not_tours() {
    let app = app();
    let token = token_for_code(&app).await;

    let (_, before) = get(&app, "/api/Tour/all/suggested", Some(&token)).await;
    let before: serde_json::Value = serde_json::from_str(&before).unwrap();
    let count = before["TotalCount"].as_u64().unwrap();

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    tour["Name"] = "Renamed".into();
    send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;

    let (_, after) = get(&app, "/api/Tour/all/suggested", Some(&token)).await;
    let after: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(after["TotalCount"].as_u64().unwrap(), count);
}

/// Deleting a tour takes its history with it.
#[tokio::test]
async fn deleting_a_tour_deletes_its_versions() {
    let app = app();
    let token = token_for_code(&app).await;

    let mut tour = fetch_tour(&app, &token, "zscph2y").await;
    tour["Name"] = "About to go".into();
    send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(tour)).await;

    let (status, _) = send(&app, "DELETE", "/api/Tour/zscph2y", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);

    // Nothing of it is left to be listed, and asking for its versions is a 404 like any
    // other unknown tour.
    let (status, _) = get(&app, "/api/Tour/zscph2y/versions", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// An endpoint that does not exist is a 404, and not the app's index page.
///
/// It fell through to the static fallback before, so a client that asked for a misspelled
/// endpoint got HTML with a 200 and had to work out for itself that it was not JSON.
#[tokio::test]
async fn an_unknown_endpoint_is_not_the_app() {
    let app = app();
    let token = token_for_code(&app).await;

    let (status, _) = get(&app, "/api/Tour/all/suggestedX", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // And the routes that do exist still answer, which is the half a catch-all can break.
    let (status, _) = get(&app, "/api/Tour/all/suggested", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
}

/// Two saves of the same tour at the same moment: one wins, the other is told.
///
/// This is what the soft lock is for, and checking the state id in the handler and writing
/// afterwards does not provide it - between those two steps the other request can land, and
/// then one client's work is gone with both told they succeeded. The check and the write are
/// one step in the store, and this is the test that would fail if they were ever separated
/// again.
#[tokio::test]
async fn two_saves_at_once_do_not_lose_one() {
    let app = app();
    let token = token_for_code(&app).await;

    // Both start from the same state, as two people with the tour open would.
    let base = fetch_tour(&app, &token, "zscph2y").await;
    let mut mine = base.clone();
    let mut theirs = base;
    mine["Name"] = "Mine".into();
    theirs["Name"] = "Theirs".into();

    let first = {
        let app = app.clone();
        let token = token.clone();
        tokio::spawn(async move {
            send(&app, "PATCH", "/api/Tour/zscph2y", Some(&token), Some(mine)).await
        })
    };
    let second = {
        let app = app.clone();
        let token = token.clone();
        tokio::spawn(async move {
            send(
                &app,
                "PATCH",
                "/api/Tour/zscph2y",
                Some(&token),
                Some(theirs),
            )
            .await
        })
    };

    let (a, _) = first.await.unwrap();
    let (b, _) = second.await.unwrap();

    let mut outcomes = [a, b];
    outcomes.sort_by_key(|s| s.as_u16());
    assert_eq!(
        outcomes,
        [StatusCode::OK, StatusCode::CONFLICT],
        "exactly one save may win, and the other has to be told"
    );

    // And what is stored is one of the two, not a mixture.
    let after = fetch_tour(&app, &token, "zscph2y").await;
    let name = after["Name"].as_str().unwrap();
    assert!(name == "Mine" || name == "Theirs", "stored: {name}");
}

/// Random bytes come back base64, and an unreasonable length is refused.
#[tokio::test]
async fn random_bytes_are_bytes() {
    let app = app();

    let (status, body) = get(&app, "/api/Auth/random/32", None).await;
    assert_eq!(status, StatusCode::OK);
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .expect("base64");
    assert_eq!(raw.len(), 32);

    // Twice is not the same twice.
    let (_, again) = get(&app, "/api/Auth/random/32", None).await;
    assert_ne!(body, again);

    let (status, _) = get(&app, "/api/Auth/random/9000", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// The wake-up endpoint answers only to its own code.
#[tokio::test]
async fn a_wakeup_needs_the_word() {
    let app = app();

    let (status, body) = get(&app, "/api/Info/wakeup/nonsense", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "wrong code");

    // The real code is accepted, and the wake-up is recorded where the startup info shows
    // it. The delays are zero in these tests; in a deployment they are what holds the
    // instance open.
    let (status, body) = get(&app, "/api/Info/wakeup/secCode", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");

    let (_, info) = get(&app, "/api/Info/start", None).await;
    let info: serde_json::Value = serde_json::from_str(&info).unwrap();
    assert_eq!(
        info["lastWakeups"].as_array().map(|a| a.len()),
        Some(1),
        "the wake-up is on the record: {info}"
    );
}

/// One access code may be capped, and an administrator is not subject to the cap.
#[tokio::test]
async fn a_code_can_be_limited_to_so_many_tours() {
    let app = app_with(|state| state.max_tours_per_code = 2);
    let token = token_for_code(&app).await;

    // The seed gives this code two tours already, so the next one is one too many.
    let (status, body) = send(
        &app,
        "POST",
        "/api/Tour/add/whatever",
        Some(&token),
        Some(serde_json::json!({ "Name": "One too many" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("up to 2 tours"), "{body}");

    // An administrator is asked precisely so that they can say yes.
    let admin = token_for_admin(&app).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/Tour/add/somecode",
        Some(&admin),
        Some(serde_json::json!({ "Name": "Allowed" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

/// The three questions a person has when the screen looks wrong, and the one endpoint that
/// answers them: which client this server hands out, which build it is, and since when it
/// has been running. No token: it names a build, and every response names it in a header
/// already.
#[tokio::test]
async fn the_server_says_which_build_it_is_and_which_client_it_serves() {
    let app = app_with(|state| {
        state.build_id = "20260910-150000".to_owned();
        state.build_commit = "7dfb7d3a91".to_owned();
        state.client_asset = Some("tc-web-b77d06b34d9563dc_bg.wasm".to_owned());
    });

    let (status, body) = get(&app, "/api/Info/version", None).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["build"], "20260910-150000");
    assert_eq!(v["commit"], "7dfb7d3a91");
    assert_eq!(v["client"], "tc-web-b77d06b34d9563dc_bg.wasm");
    assert!(
        v["started"].as_str().is_some_and(|s| s.ends_with('Z')),
        "the start time is there, and it is a time: {v}"
    );
}
