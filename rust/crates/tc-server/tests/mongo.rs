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
    let listed = store.list(&|_| true).await;
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
