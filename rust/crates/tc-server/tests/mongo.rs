//! The MongoDB store, against a real MongoDB.
//!
//! Skipped unless `TC_TEST_MONGO` names one - `mongodb://127.0.0.1:27099`, say. A test that
//! quietly passes when there is no database to talk to would be worse than no test: the
//! whole point of these is that the driver, the filters and the document shape are exercised
//! against the real thing rather than against my idea of it.
//!
//! ```sh
//! TC_TEST_MONGO=mongodb://127.0.0.1:27099 cargo test -p tc-server --features mongo --test mongo
//! ```

#![cfg(feature = "mongo")]

use tc_core::{Tour, TourId};
use tc_server::fields;
use tc_server::mongo::MongoStore;
use tc_server::store::TourStore;

fn fixture() -> Tour {
    let json = include_str!("../../../fixtures/zscph2y.tour.json");
    Tour::from_json(json).expect("fixture")
}

/// A store on a database of its own.
///
/// Its own, because these run at the same time in one process and the first version of this
/// had them all on one collection, each emptying it as it started - five tests failing in
/// five different ways, none of them about MongoDB.
async fn store(name: &str) -> Option<MongoStore> {
    let url = std::env::var("TC_TEST_MONGO").ok()?;
    let store = MongoStore::connect_to(&url, "", "", &format!("tc_test_{name}"), "Tours")
        .await
        .expect("connects");
    store.wipe_everything_for_tests().await;
    Some(store)
}

macro_rules! store_or_skip {
    ($name:literal) => {
        match store($name).await {
            Some(s) => s,
            None => {
                eprintln!("no TC_TEST_MONGO: skipped");
                return;
            }
        }
    };
}

#[tokio::test]
async fn a_tour_survives_the_database() {
    let store = store_or_skip!("a_tour_survives_the_database");
    let tour = fixture();

    store.store(tour.clone()).await;
    let back = store.get(&tour.id).await.expect("it is there");

    assert_eq!(back.name, tour.name);
    assert_eq!(back.persons.len(), tour.persons.len());
    assert_eq!(back.spendings.len(), tour.spendings.len());
    assert_eq!(
        tc_core::calculate(&back, tc_core::Options::default()),
        tc_core::calculate(&tour, tc_core::Options::default()),
        "the same money, to the cent"
    );
    // The dates went in as dates and came back as text this crate can read.
    let created = fields::str_of(&back, fields::CREATED_AT);
    assert!(
        created.starts_with("20") && created.contains('T'),
        "a date came back as {created:?}"
    );
}

#[tokio::test]
async fn the_soft_lock_is_the_databases_job() {
    let store = store_or_skip!("the_soft_lock_is_the_databases_job");
    let mut tour = fixture();
    fields::set(&mut tour, fields::STATE, "state-one".into());
    store.store(tour.clone()).await;

    let mut next = tour.clone();
    next.name = "Changed".into();
    fields::set(&mut next, fields::STATE, "state-two".into());

    // Saving against the state that is stored works.
    store
        .replace(&tour.id, "state-one", next.clone(), &|_| None)
        .await
        .expect("the first save wins");
    assert_eq!(store.get(&tour.id).await.unwrap().name, "Changed");

    // Saving against a state that has moved on does not, and says what is there now.
    let mut theirs = tour.clone();
    theirs.name = "Theirs".into();
    let outcome = store
        .replace(&tour.id, "state-one", theirs, &|_| None)
        .await;
    let stale = outcome.expect_err("the second save is told no");
    assert_eq!(stale.0, "state-two", "and told what it is now");
    assert_eq!(store.get(&tour.id).await.unwrap().name, "Changed");
}

#[tokio::test]
async fn versions_are_kept_and_kept_out_of_the_list() {
    let store = store_or_skip!("versions_are_kept_and_kept_out_of_the_list");
    let mut tour = fixture();
    fields::set(&mut tour, fields::STATE, "one".into());
    store.store(tour.clone()).await;

    let mut next = tour.clone();
    next.name = "After".into();
    fields::set(&mut next, fields::STATE, "two".into());

    store
        .replace(&tour.id, "one", next, &|previous| {
            let mut version = previous.clone();
            version.id = TourId::new("a-version");
            fields::set(&mut version, fields::IS_VERSION, true.into());
            fields::set(
                &mut version,
                fields::VERSION_FOR,
                previous.id.as_str().into(),
            );
            fields::set(&mut version, fields::VERSIONED_AT, "2026-01-01".into());
            fields::set(&mut version, fields::VERSION_COMMENT, "renamed".into());
            Some(version)
        })
        .await
        .expect("saved");

    let (versions, total) = store.versions(&tour.id, 0, 50).await;
    assert_eq!(total, 1);
    assert_eq!(
        versions[0].name,
        fixture().name,
        "the state before the save"
    );

    // And the list is tours only - a version is history, not a tour.
    let listed = store.list(None, &|_| true).await;
    assert_eq!(listed.len(), 1, "one tour, not two");
    assert!(!fields::is_version(&listed[0]));
}

#[tokio::test]
async fn deleting_takes_it_away() {
    let store = store_or_skip!("deleting_takes_it_away");
    let tour = fixture();
    store.store(tour.clone()).await;

    assert!(store.remove(&tour.id).await);
    assert!(store.get(&tour.id).await.is_none());
    assert!(!store.remove(&tour.id).await, "and not twice");
}

/// What the C# would read: the document is keyed by `_id` and its dates are BSON dates.
///
/// This is the compatibility claim the whole module rests on, so it is checked against what
/// is actually in the database rather than against the mapping function.
#[tokio::test]
async fn the_document_is_the_shape_the_csharp_writes() {
    let store = store_or_skip!("the_document_is_the_shape_the_csharp_writes");
    let tour = fixture();
    store.store(tour.clone()).await;

    let raw = store
        .raw_document_for_tests(tour.id.as_str())
        .await
        .expect("the document");

    assert_eq!(raw.get_str("_id").ok(), Some(tour.id.as_str()));
    assert_eq!(
        raw.get_str("GUID").ok(),
        Some(tour.id.as_str()),
        "the C# writes both"
    );
    assert!(
        matches!(raw.get("DateCreated"), Some(bson::Bson::DateTime(_))),
        "DateCreated is a date and not a string: {:?}",
        raw.get("DateCreated")
    );
    let spending = raw
        .get_array("Spendings")
        .expect("spendings")
        .first()
        .and_then(|s| s.as_document())
        .expect("a spending");
    assert!(
        matches!(spending.get("SpendingDate"), Some(bson::Bson::DateTime(_))),
        "so is a spending's date: {:?}",
        spending.get("SpendingDate")
    );
}

/// A currency is known by the id the app's driver stores, through the database.
///
/// The C# maps its `Id` property to the `_id` element and ignores elements it does not know,
/// so a currency in the database is identified by `_id`. This port read `Id` instead, and on
/// a real tour whose dinars were stamped with an older id that meant no conversion at all -
/// the euro view came out sixty times too large while the app had it right.
///
/// The test that would have caught it has to run against MongoDB, because that is the only
/// place the two names for the same thing exist side by side. The file-backed store has no
/// `_id` anywhere, which is exactly why comparing the two clients on it agreed.
#[tokio::test]
async fn a_currency_keeps_its_identity_through_the_database() {
    let store = store_or_skip!("a_currency_keeps_its_identity_through_the_database");

    // As the app leaves it: the currency was named Din when the expense was entered, and is
    // called RSD now. The driver wrote its identity to `_id` and never wrote an `Id`.
    let din = bson::doc! { "_id": "Din", "Name": "RSD", "CurrencyRate": 1000 };
    let eur = bson::doc! { "_id": "Eur", "Name": "Eur", "CurrencyRate": 117000 };
    let stamped = bson::doc! { "_id": "Din", "Name": "Din", "CurrencyRate": 1000 };
    store
        .insert_raw_for_tests(bson::doc! {
            "_id": "renamed",
            "GUID": "renamed",
            "Name": "a renamed currency",
            "TourCurrencyId": "Eur",
            "Currencies": [din, eur],
            "Persons": [ { "GUID": "p1", "Name": "Ann", "Weight": 100 } ],
            "Spendings": [ {
                "GUID": "s1", "Description": "dinars", "Type": "food",
                "AmountInCents": 11700, "Currency": stamped,
                "FromGuid": "p1", "ToAll": true, "ToGuid": []
            } ],
        })
        .await;

    let tour = store
        .get(&TourId::new("renamed".to_owned()))
        .await
        .expect("the tour");

    assert_eq!(
        tour.currencies[0].id.as_str(),
        "Din",
        "the identity is what the driver stored, not the name it happens to carry now"
    );
    assert_eq!(
        tour.amount_in_current(&tour.spendings[0]),
        tc_core::Cents(100),
        "11700 dinars at 1000 against the euro at 117000 - not 11700, which is what \
         'this tour has no such currency' would have left it as"
    );

    // And saving it back leaves a document the app still reads the same way.
    store.store((*tour).clone()).await;
    let raw = store
        .raw_document_for_tests("renamed")
        .await
        .expect("the document");
    let first = raw
        .get_array("Currencies")
        .expect("currencies")
        .first()
        .and_then(|c| c.as_document())
        .expect("a currency");
    assert_eq!(
        first.get_str("_id").ok(),
        Some("Din"),
        "unchanged for the app"
    );
    assert_eq!(
        first.get_str("Id").ok(),
        Some("Din"),
        "and the same under the other name, which is what this port reads"
    );
    let currency = raw.get_document("Currency").ok();
    if let Some(currency) = currency {
        assert_eq!(
            currency.get_str("_id").ok().or(currency.get_str("Id").ok()),
            Some("Eur"),
            "the tour's own currency says what TourCurrencyId says"
        );
    }
}

/// Subscriptions survive the server. That is the whole point of storing them.
///
/// The in-memory version forgets everybody when the process stops, and this process stops on
/// every deploy - so somebody who turned notifications on would quietly stop being told,
/// with nothing on their screen to say so.
#[tokio::test]
async fn a_subscription_outlives_the_server() {
    use tc_server::subscriptions::{Subscription, SubscriptionStore};

    let store = store_or_skip!("subs");
    let subs = store.subscriptions();
    subs.add("t1", sub("https://push.example/a")).await;
    subs.add("t1", sub("https://push.example/b")).await;
    subs.add("t2", sub("https://push.example/a")).await;

    // A second server, a fresh connection - which is what a restart looks like from here.
    let again = MongoStore::connect_to(
        &std::env::var("TC_TEST_MONGO").unwrap(),
        "",
        "",
        "tc_test_subs",
        "Tours",
    )
    .await
    .expect("connects")
    .subscriptions();

    let mut found: Vec<String> = again
        .for_tour("t1")
        .await
        .into_iter()
        .map(|s| s.url)
        .collect();
    found.sort();
    assert_eq!(found, ["https://push.example/a", "https://push.example/b"]);
    assert!(again.has("t1", &sub("https://push.example/a")).await);
    assert!(!again.has("t1", &sub("https://push.example/nobody")).await);

    // A subscription belongs to its tour and not to the server.
    assert_eq!(again.for_tour("t2").await.len(), 1);

    // Subscribing twice from the same browser is one subscription, and the renewed keys win.
    again
        .add(
            "t1",
            Subscription {
                url: "https://push.example/a".into(),
                p256dh: "renewed".into(),
                auth: "renewed".into(),
            },
        )
        .await;
    let mine = again.for_tour("t1").await;
    assert_eq!(mine.len(), 2, "still two: {mine:?}");
    assert_eq!(
        mine.iter()
            .find(|s| s.url == "https://push.example/a")
            .map(|s| s.p256dh.as_str()),
        Some("renewed")
    );

    again.remove("t1", &sub("https://push.example/a")).await;
    assert_eq!(again.for_tour("t1").await.len(), 1);
    assert!(!again.has("t1", &sub("https://push.example/a")).await);
}

/// The tour list's question - which tours is this browser subscribed to, and whose are
/// they - answered from the database without reading a tour whole.
#[tokio::test]
async fn a_browser_finds_its_subscribed_tours() {
    use tc_server::subscriptions::SubscriptionStore;

    let store = store_or_skip!("subs_mine");
    let tour = fixture();
    store.store(tour.clone()).await;

    let subs = store.subscriptions();
    subs.add(tour.id.as_str(), sub("https://push.example/a"))
        .await;
    subs.add("gone", sub("https://push.example/a")).await;
    subs.add("other", sub("https://push.example/b")).await;
    // Stored twice, as the C# can.
    store
        .insert_raw_subscription_for_tests(bson::doc! {
            "TourId": tour.id.as_str(),
            "Subscription": { "Url": "https://push.example/a", "P256dh": "p", "Auth": "a" },
        })
        .await;

    let tours = subs.tours_of(&sub("https://push.example/a")).await;
    let mut expected = vec!["gone".to_owned(), tour.id.as_str().to_owned()];
    expected.sort();
    assert_eq!(tours, expected);
    assert!(subs
        .tours_of(&sub("https://push.example/c"))
        .await
        .is_empty());

    // Only the tour that exists has a code, and it is the tour's.
    let codes = store.access_codes(&tours).await;
    assert_eq!(
        codes,
        [(tour.id.as_str().to_owned(), fields::access_code(&tour))]
    );
    assert!(!fields::access_code(&tour).is_empty());
}

/// The state alone, read without the tour - and it is the tour's own.
#[tokio::test]
async fn the_state_is_read_without_the_tour() {
    let store = store_or_skip!("state_of");
    let tour = fixture();
    store.store(tour.clone()).await;

    let (code, state) = store.state_of(&tour.id).await.expect("it is there");
    assert_eq!(code, fields::access_code(&tour));
    assert_eq!(state, fields::str_of(&tour, fields::STATE));
    assert!(!state.is_empty());
    assert!(store
        .state_of(&TourId::new("nosuchtour".to_owned()))
        .await
        .is_none());
}

/// And the C#'s own documents are read, because it is the same collection.
#[tokio::test]
async fn a_subscription_the_app_stored_is_read_here() {
    use tc_server::subscriptions::SubscriptionStore;

    let store = store_or_skip!("subs_csharp");
    // What `MongoDbSubscriptionStorage.AddSubscription` inserts, field for field.
    store
        .insert_raw_subscription_for_tests(bson::doc! {
            "TourId": "abc",
            "Subscription": {
                "Url": "https://push.example/from-the-app",
                "P256dh": "p",
                "Auth": "a",
            },
        })
        .await;

    let subs = store.subscriptions();
    let mine = subs.for_tour("abc").await;
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].url, "https://push.example/from-the-app");
    assert_eq!(mine[0].p256dh, "p");
    assert!(
        subs.has("abc", &sub("https://push.example/from-the-app"))
            .await
    );
}

fn sub(url: &str) -> tc_server::subscriptions::Subscription {
    tc_server::subscriptions::Subscription {
        url: url.to_owned(),
        p256dh: "key".into(),
        auth: "auth".into(),
    }
}

/// A list asks the database for the codes it may see, not for every tour there is.
///
/// On a database with everybody's tours in it, reading the lot to keep a handful is the
/// difference between a list screen and a list screen that gets slower every year. The
/// filter is an optimisation, so the test is about what comes back being right *and* the
/// narrowing being real: a tour under another code is not in the answer even though
/// `allowed` would have taken it.
#[tokio::test]
async fn a_list_is_narrowed_by_the_database() {
    let store = store_or_skip!("list_by_code");

    let mut mine = fixture();
    mine.id = TourId::new("mine");
    fields::set(&mut mine, fields::ACCESS_CODE, "AAA".into());
    store.store(mine).await;

    let mut theirs = fixture();
    theirs.id = TourId::new("theirs");
    fields::set(&mut theirs, fields::ACCESS_CODE, "BBB".into());
    store.store(theirs).await;

    let codes = vec!["AAA".to_owned()];
    let listed = store.list(Some(&codes), &|_| true).await;
    assert_eq!(listed.len(), 1, "only the one code: {:?}", ids(&listed));
    assert_eq!(listed[0].id.as_str(), "mine");

    // And without narrowing, both - which is what an administrator gets.
    let all = store.list(None, &|_| true).await;
    assert_eq!(all.len(), 2, "{:?}", ids(&all));
}

fn ids(tours: &[std::sync::Arc<Tour>]) -> Vec<&str> {
    tours.iter().map(|t| t.id.as_str()).collect()
}

/// The rule on how many tours a code may hold asks for a number, and the database counts
/// rather than handing every tour over to be counted.
#[tokio::test]
async fn tours_are_counted_by_code() {
    let store = store_or_skip!("tours_are_counted_by_code");
    let tour = fixture();
    let code = fields::access_code(&tour);

    store.store(tour.clone()).await;
    let mut another = tour.clone();
    another.id = TourId::new("another");
    store.store(another).await;
    // A version is history, not a tour of the code.
    let version = tc_server::api::write::version_of(&tour, "kept".into());
    store.store(version).await;
    let mut elsewhere = tour.clone();
    elsewhere.id = TourId::new("elsewhere");
    fields::set(&mut elsewhere, fields::ACCESS_CODE, "SOMEBODY ELSE".into());
    store.store(elsewhere).await;

    let codes = vec![code.clone()];
    let count = store.count(Some(&codes), &|_| true).await;
    assert_eq!(count, 2);
    assert_eq!(count, store.list(Some(&codes), &|_| true).await.len());
}
