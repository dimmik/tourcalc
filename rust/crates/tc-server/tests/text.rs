//! The text interface, driven the way a text browser drives it: links and form posts.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

const CODE: &str = "1C0369EA42B2F746D3DF1E66BCB2DE46";
const DEV_KEY: &str = "aSXx0m1XH4K1GfIYR8mi7/XrSWGCH30Eqn074DhewZo=";
const TOUR: &str = "hs3huvy";

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
        versioning: true,
        version_editable: false,
        max_tours_per_code: -1,
        wakeup_code: "secCode".into(),
        wakeup_pre_delay_min: 0,
        wakeup_post_delay_min: 0,
        wakeups: Default::default(),
    });
    tc_server::text::routes().with_state(state)
}

struct Answer {
    status: StatusCode,
    body: String,
    location: String,
    cookie: String,
}

async fn go(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: &str,
    form: Option<&str>,
) -> Answer {
    let mut req = Request::builder().method(method).uri(uri);
    if !cookie.is_empty() {
        req = req.header("cookie", cookie);
    }
    let req = match form {
        Some(body) => req
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body.to_owned()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };

    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    let location = header("location");
    // Only the name=value part travels back, as a browser sends it.
    let cookie = header("set-cookie")
        .split(';')
        .next()
        .unwrap_or("")
        .to_owned();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    Answer {
        status,
        location,
        cookie,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

/// Signing in the way a share link does it, and the cookie it leaves.
async fn signed_in(app: &axum::Router) -> String {
    let answer = go(app, "GET", &format!("/t/goto/{CODE}/{TOUR}"), "", None).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    assert_eq!(answer.location, format!("/t/{TOUR}"));
    assert!(answer.cookie.starts_with("tc_text="), "{}", answer.cookie);
    answer.cookie
}

/// A reader with no cookie is sent to the login form, not given a bare 401.
///
/// A text browser shows a 401 as an error page with no way out of it, so the one thing this
/// interface must not do is answer with a status and no words.
#[tokio::test]
async fn a_stranger_is_sent_to_the_login_form() {
    let app = app();
    for path in ["/t", "/t/hs3huvy", "/t/hs3huvy/people", "/t/hs3huvy/stats"] {
        let answer = go(&app, "GET", path, "", None).await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER, "{path}");
        assert!(
            answer.location.starts_with("/t/login?next="),
            "{path} -> {}",
            answer.location
        );
    }
}

/// The share link signs the reader in and drops them on the tour.
#[tokio::test]
async fn a_share_link_is_the_way_in() {
    let app = app();
    let cookie = signed_in(&app).await;

    let answer = go(&app, "GET", &format!("/t/{TOUR}"), &cookie, None).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("Поход Урал Август 2021"));
    assert!(answer.body.contains("Who pays whom"));
    assert!(answer.body.contains("Mark paid"));
}

/// The list holds the tours of that code, and links to each.
#[tokio::test]
async fn the_index_lists_what_the_code_may_see() {
    let app = app();
    let cookie = signed_in(&app).await;

    let answer = go(&app, "GET", "/t", &cookie, None).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains(&format!("href=\"/t/{TOUR}\"")));
}

/// A tour is not there for somebody signed in with a different code.
///
/// Every tour in the seed is filed under the same code, so this signs in with another one
/// rather than looking for a tour that does not exist - which is the same rule seen from
/// the other side, and the side that matters.
#[tokio::test]
async fn another_codes_tour_is_not_found() {
    let app = app();

    let stranger = go(
        &app,
        "POST",
        "/t/login",
        "",
        Some("Code=some-other-code&Scope=code"),
    )
    .await;
    let cookie = stranger.cookie;

    let answer = go(&app, "GET", &format!("/t/{TOUR}"), &cookie, None).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);

    // And their own list is empty rather than everybody else's.
    let list = go(&app, "GET", "/t", &cookie, None).await;
    assert!(list
        .body
        .contains("No tours are visible with this access code."));
}

/// A person added through the form is on the page afterwards.
#[tokio::test]
async fn a_person_can_be_added_and_removed() {
    let app = app();
    let cookie = signed_in(&app).await;

    let answer = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/people/edit"),
        &cookie,
        Some("Name=%D0%A2%D0%B5%D1%81%D1%82&Weight=50&ParentId="),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);

    let people = go(&app, "GET", &format!("/t/{TOUR}/people"), &cookie, None).await;
    assert!(people.body.contains("Тест"), "the new person is listed");

    // And out again.
    let id = people
        .body
        .split("Тест")
        .nth(1)
        .and_then(|rest| rest.split("people/edit/").nth(1))
        .and_then(|rest| rest.split('"').next())
        .expect("an edit link for the new person")
        .to_owned();

    let gone = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/people/delete/{id}"),
        &cookie,
        Some(""),
    )
    .await;
    assert_eq!(gone.status, StatusCode::SEE_OTHER);

    let people = go(&app, "GET", &format!("/t/{TOUR}/people"), &cookie, None).await;
    assert!(!people.body.contains("Тест"));
}

/// A form that cannot be saved comes back as the form, with the reason on it.
#[tokio::test]
async fn a_bad_form_comes_back_saying_why() {
    let app = app();
    let cookie = signed_in(&app).await;

    let answer = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/people/edit"),
        &cookie,
        Some("Name=&Weight=100"),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "not a redirect, and not a 500"
    );
    assert!(answer.body.contains("A name, please."));
    assert!(answer.body.contains("<form"), "the form is still there");

    let answer = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/spend/edit"),
        &cookie,
        Some("Amount=0&Description=nothing&ToAll=true"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("How much was it?"));
}

/// An expense for particular people keeps all of them.
///
/// The checkboxes send the same name repeatedly, and the usual form deserialiser keeps only
/// the last value - which would quietly turn "for these four" into "for that one".
#[tokio::test]
async fn an_expense_for_several_people_keeps_them_all() {
    let app = app();
    let cookie = signed_in(&app).await;

    let form = go(&app, "GET", &format!("/t/{TOUR}/spend/edit"), &cookie, None).await;
    let ids: Vec<String> = form
        .body
        .split("name=\"ToGuid\" value=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next().map(|s| s.to_owned()))
        .take(3)
        .collect();
    assert_eq!(ids.len(), 3, "three people to pick from");

    let body = format!(
        "Amount=900&Description=Split+three+ways&FromGuid={}&Type=Test&{}",
        ids[0],
        ids.iter()
            .map(|id| format!("ToGuid={id}"))
            .collect::<Vec<_>>()
            .join("&")
    );
    let answer = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/spend/edit"),
        &cookie,
        Some(&body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);

    let list = go(
        &app,
        "GET",
        &format!("/t/{TOUR}/spend?q=Split+three+ways"),
        &cookie,
        None,
    )
    .await;
    assert!(list.body.contains("Split three ways"));
    assert!(
        list.body.contains("<td>3 people</td>"),
        "all three recipients survived the form"
    );
}

/// One of the suggested payments can be recorded as having happened.
#[tokio::test]
async fn a_payment_can_be_marked_paid() {
    let app = app();
    let cookie = signed_in(&app).await;

    let tour = go(&app, "GET", &format!("/t/{TOUR}"), &cookie, None).await;
    let before = tour.body.matches("Mark paid").count();
    assert!(before > 0, "there is something to settle");

    let id = tour
        .body
        .split("name=\"transferId\" value=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a payment to mark")
        .to_owned();

    let answer = go(
        &app,
        "POST",
        &format!("/t/{TOUR}/markpaid"),
        &cookie,
        Some(&format!("transferId={id}")),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    assert!(
        answer.location.contains("recorded="),
        "the page says what was recorded: {}",
        answer.location
    );

    // The payback is now an expense of its own, with no category - so it is a payment
    // between two people and not money spent on the tour.
    let spend = go(&app, "GET", &format!("/t/{TOUR}/spend"), &cookie, None).await;
    assert!(spend.body.contains("&#39;"), "a payback names both people");
}

/// A name that looks like markup is shown, not run.
#[tokio::test]
async fn a_name_cannot_be_markup() {
    let app = app();
    let cookie = signed_in(&app).await;

    go(
        &app,
        "POST",
        &format!("/t/{TOUR}/people/edit"),
        &cookie,
        Some("Name=%3Cscript%3Ealert(1)%3C%2Fscript%3E&Weight=100"),
    )
    .await;

    let people = go(&app, "GET", &format!("/t/{TOUR}/people"), &cookie, None).await;
    assert!(
        !people.body.contains("<script>alert(1)</script>"),
        "the page must not carry it as markup"
    );
    assert!(
        people.body.contains("&lt;script&gt;"),
        "it is shown as the name somebody typed"
    );
}

/// Logging out drops the cookie.
#[tokio::test]
async fn logging_out_takes_the_cookie_away() {
    let app = app();
    let cookie = signed_in(&app).await;

    let answer = go(&app, "GET", "/t/logout", &cookie, None).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    assert_eq!(answer.location, "/t/login");
    assert_eq!(answer.cookie, "tc_text=", "emptied");
}

/// The login form takes an access code, and refuses one it does not know.
#[tokio::test]
async fn the_login_form_works_and_says_no() {
    let app = app();

    let answer = go(&app, "POST", "/t/login", "", Some("Code=&Scope=code")).await;
    assert!(answer.location.contains("failed=1"), "an empty code is no");

    let answer = go(
        &app,
        "POST",
        "/t/login",
        "",
        Some("Code=not-the-master-key&Scope=admin"),
    )
    .await;
    assert!(answer.location.contains("failed=1"));

    // The access code itself is accepted - it is hashed, so any code is "valid" and simply
    // shows whichever tours are filed under it.
    let answer = go(
        &app,
        "POST",
        "/t/login",
        "",
        Some("Code=whatever&Scope=code"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER);
    assert!(answer.cookie.starts_with("tc_text="));
}
