//! Tours kept in MongoDB, in the collection the C# server already writes.
//!
//! Same database, same collection, same documents: a tour written here is one the C# reads
//! and the other way round. That is the whole requirement, and it is the reason this is not
//! simply "serialise the struct" - two things about the stored shape have to be reproduced
//! rather than invented.
//!
//! **The id.** The C# driver maps a member called `Id` to `_id` by convention, so a tour's
//! document is keyed by the tour id and also carries `GUID` with the same value.
//!
//! **The dates.** `DateCreated`, `DateVersioned` and `SpendingDate` are `DateTime` in the
//! C# model, so they are stored as BSON dates and not as text. This crate holds them as
//! strings, because that is what they are in JSON everywhere else - so they are converted
//! at the edge, in both directions. Getting this wrong would not fail: it would quietly
//! write strings into a database the C# then refuses to read.

use crate::store::{Stale, TourStore};
use bson::{doc, Document};
use futures_util::TryStreamExt;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection};
use std::sync::Arc;
use tc_core::{Tour, TourId};

/// Fields the C# model declares as `DateTime`, wherever they appear in a document.
const DATES: &[&str] = &["DateCreated", "DateVersioned", "SpendingDate"];

pub struct MongoStore {
    tours: Collection<Document>,
}

impl MongoStore {
    /// Connects, and checks the connection by asking for a tour that does not exist -
    /// exactly as the C# constructor does, and for the same reason: a database that cannot
    /// be reached should say so at startup and not on somebody's first save.
    pub async fn connect(url: &str, username: &str, password: &str) -> Result<MongoStore, String> {
        // The names the C# hardcodes. They are arguments only so that the tests can each
        // have a database to themselves.
        Self::connect_to(url, username, password, "tour", "Tours").await
    }

    pub async fn connect_to(
        url: &str,
        username: &str,
        password: &str,
        database: &str,
        collection: &str,
    ) -> Result<MongoStore, String> {
        let uri = if url.contains("://") {
            // A whole connection string, taken as it is.
            url.to_owned()
        } else {
            // The C# builds one from three settings, url-encoding the credentials.
            let enc =
                |s: &str| -> String { form_urlencoded::byte_serialize(s.as_bytes()).collect() };
            format!(
                "mongodb+srv://{}:{}@{url}?connect=replicaSet",
                enc(username),
                enc(password)
            )
        };

        let options = ClientOptions::parse(&uri)
            .await
            .map_err(|e| format!("MongoDB connection string: {e}"))?;
        let client = Client::with_options(options).map_err(|e| format!("MongoDB: {e}"))?;

        let store = MongoStore {
            tours: client.database(database).collection::<Document>(collection),
        };
        store
            .tours
            .find_one(doc! { "_id": "none" })
            .await
            .map_err(|e| format!("MongoDB is not answering: {e}"))?;
        Ok(store)
    }

    /// The stored document, as it is. For the tests that check its shape - what the C#
    /// would read is the whole claim of this module, and it is worth checking against the
    /// database rather than against the function that writes it.
    pub async fn raw_document_for_tests(&self, id: &str) -> Option<Document> {
        self.tours.find_one(doc! { "_id": id }).await.ok().flatten()
    }

    /// Empties the collection. Tests only, and it does what it says.
    pub async fn wipe_everything_for_tests(&self) {
        let _ = self.tours.delete_many(doc! {}).await;
    }

    async fn all(&self, filter: Document) -> Vec<Arc<Tour>> {
        let found = match self.tours.find(filter).await {
            Ok(cursor) => cursor.try_collect::<Vec<Document>>().await,
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                return Vec::new();
            }
        };
        match found {
            Ok(docs) => docs
                .iter()
                .filter_map(|d| to_tour(d).map(Arc::new))
                .collect(),
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                Vec::new()
            }
        }
    }
}

#[async_trait::async_trait]
impl TourStore for MongoStore {
    async fn get(&self, id: &TourId) -> Option<Arc<Tour>> {
        match self.tours.find_one(doc! { "_id": id.as_str() }).await {
            Ok(found) => found.as_ref().and_then(to_tour).map(Arc::new),
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                None
            }
        }
    }

    async fn list(&self, allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync)) -> Vec<Arc<Tour>> {
        // Versions are that tour's history and never appear in a list. The filter is on the
        // database side because a tour with a long history would otherwise be read whole,
        // once per version, to be thrown away here.
        let filter = doc! { "IsVersion": { "$ne": true } };
        self.all(filter)
            .await
            .into_iter()
            .filter(|t| allowed(t))
            .collect()
    }

    async fn store(&self, tour: Tour) {
        let Some(document) = to_document(&tour) else {
            tracing::error!("could not turn a tour into a document: {}", tour.id);
            return;
        };
        let id = tour.id.as_str().to_owned();
        if let Err(e) = self
            .tours
            .replace_one(doc! { "_id": &id }, document)
            .upsert(true)
            .await
        {
            tracing::error!("MongoDB write failed for {id}: {e}");
        }
    }

    async fn remove(&self, id: &TourId) -> bool {
        match self.tours.delete_one(doc! { "_id": id.as_str() }).await {
            Ok(result) => result.deleted_count > 0,
            Err(e) => {
                tracing::error!("MongoDB delete failed: {e}");
                false
            }
        }
    }

    async fn replace(
        &self,
        id: &TourId,
        expected_state: &str,
        next: Tour,
        version_of: &(dyn for<'a> Fn(&'a Tour) -> Option<Tour> + Sync),
    ) -> Result<(), Stale> {
        // The soft lock, done by the database: the update only matches while StateGUID is
        // still what the caller saw. Two saves at once cannot both match, so one of them is
        // told - which is the same guarantee the in-memory store gets from its write lock,
        // without this process having to be the only one writing.
        let Some(previous) = self.get(id).await else {
            return Err(Stale(String::new()));
        };
        let Some(document) = to_document(&next) else {
            return Err(Stale(String::new()));
        };

        let matched = self
            .tours
            .replace_one(
                doc! { "_id": id.as_str(), "StateGUID": expected_state },
                document,
            )
            .await
            .map_err(|e| {
                tracing::error!("MongoDB write failed: {e}");
                Stale(String::new())
            })?;

        if matched.matched_count == 0 {
            // Somebody was quicker. Read back what is there now, so the answer can say so.
            let now = self
                .get(id)
                .await
                .map(|t| crate::fields::str_of(&t, crate::fields::STATE))
                .unwrap_or_default();
            return Err(Stale(now));
        }

        // The version is written after the tour rather than with it. Two writes, and no
        // transaction: a version needs a replica set to be written atomically alongside,
        // and the worst this can do is lose a history entry that nothing depends on. The
        // tour itself - the thing that matters - is one atomic operation either way.
        if let Some(version) = version_of(&previous) {
            self.store(version).await;
        }
        Ok(())
    }

    async fn versions(&self, id: &TourId, from: usize, count: usize) -> (Vec<Arc<Tour>>, usize) {
        let mut mine = self
            .all(doc! { "IsVersion": true, "VersionFor_Id": id.as_str() })
            .await;

        mine.sort_by(|a, b| {
            crate::fields::str_of(b, crate::fields::VERSIONED_AT)
                .cmp(&crate::fields::str_of(a, crate::fields::VERSIONED_AT))
        });

        let total = mine.len();
        (mine.into_iter().skip(from).take(count).collect(), total)
    }
}

// --- the shape on disk --------------------------------------------------------------------

/// A stored document as a tour.
pub fn to_tour(document: &Document) -> Option<Tour> {
    let mut value: serde_json::Value = bson::from_document(document.clone()).ok()?;
    dates_to_text(&mut value);
    // `_id` is Mongo's; the tour's own id is `Id`/`GUID`, which travel in the document too.
    if let Some(obj) = value.as_object_mut() {
        obj.remove("_id");
    }
    Tour::from_json(&value.to_string()).ok()
}

/// A tour as a document to store.
pub fn to_document(tour: &Tour) -> Option<Document> {
    let text = tour.to_json().ok()?;
    let mut value: serde_json::Value = serde_json::from_str(&text).ok()?;
    text_to_dates(&mut value);
    if let Some(obj) = value.as_object_mut() {
        // What the C# driver keys the document by.
        obj.insert("_id".into(), tour.id.as_str().into());
    }
    bson::to_document(&value).ok()
}

/// Turns the stored BSON dates into the ISO text this crate keeps them as.
///
/// `bson::from_document` renders a date as `{"$date": {"$numberLong": "..."}}` - so this
/// walks the value and puts back a plain string wherever one of the date fields carries
/// that shape.
fn dates_to_text(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let is_date = DATES.iter().any(|d| d.eq_ignore_ascii_case(&key));
                if is_date {
                    if let Some(millis) = millis_of(&map[&key]) {
                        map[&key] = serde_json::Value::String(iso_from_millis(millis));
                        continue;
                    }
                }
                dates_to_text(map.get_mut(&key).expect("just listed"));
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(dates_to_text),
        _ => {}
    }
}

/// The other direction: text into something BSON will store as a date.
fn text_to_dates(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let is_date = DATES.iter().any(|d| d.eq_ignore_ascii_case(&key));
                if is_date {
                    if let Some(text) = map[&key].as_str() {
                        if let Some(millis) = millis_from_iso(text) {
                            map[&key] = serde_json::json!({
                                "$date": { "$numberLong": millis.to_string() }
                            });
                            continue;
                        }
                    }
                }
                text_to_dates(map.get_mut(&key).expect("just listed"));
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(text_to_dates),
        _ => {}
    }
}

/// Milliseconds out of whichever `$date` shape the encoder produced.
///
/// Three of them exist and this has met two: `bson` writes an ISO string for a date inside
/// the representable range and `{"$numberLong": "..."}` for one outside it, while a plain
/// number is the older canonical form. Reading only the number was enough to make every
/// date come back empty - and to make one test fail while the test that checks the *stored*
/// shape passed, which is what pointed at the reading rather than the writing.
fn millis_of(value: &serde_json::Value) -> Option<i64> {
    match value.get("$date")? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(text) => millis_from_iso(text),
        other => other.get("$numberLong")?.as_str()?.parse().ok(),
    }
}

/// `2026-09-09T18:24:50.000Z` from milliseconds since the epoch.
fn iso_from_millis(millis: i64) -> String {
    let bson_date = bson::DateTime::from_millis(millis);
    bson_date.try_to_rfc3339_string().unwrap_or_default()
}

/// Milliseconds since the epoch from any of the date shapes the app has ever written.
///
/// The stored data is not consistent: `2022-07-03T07:40:20.4829583+03:00` from the C#,
/// `2021-06-10T15:55:15.466Z` from the browser, and `2026-09-09 21:24:50` from this server.
/// All three have to survive a trip through the database, so all three are read here rather
/// than one being declared correct.
pub fn millis_from_iso(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.len() < 19 {
        return None;
    }
    let bytes = text.as_bytes();
    let num = |from: usize, to: usize| -> Option<i64> { text.get(from..to)?.parse().ok() };

    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    // A space or a T between the date and the time; both are written.
    if bytes[10] != b'T' && bytes[10] != b' ' {
        return None;
    }
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    let second = num(17, 19)?;

    let rest = &text[19..];
    let fraction = rest
        .strip_prefix('.')
        .map(|f| {
            let digits: String = f.chars().take_while(|c| c.is_ascii_digit()).collect();
            // However many digits were written, milliseconds are what is kept.
            let millis: String = digits.chars().chain("000".chars()).take(3).collect();
            millis.parse::<i64>().unwrap_or(0)
        })
        .unwrap_or(0);

    // An offset, if one is written: the instant is what is stored, not the local reading.
    let offset_minutes = match rest.rfind(['+', '-']) {
        Some(at) => {
            let sign = if rest.as_bytes()[at] == b'-' { -1 } else { 1 };
            let zone = &rest[at + 1..];
            let (h, m) = match zone.split_once(':') {
                Some((h, m)) => (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?),
                None if zone.len() == 4 => (zone[..2].parse().ok()?, zone[2..].parse().ok()?),
                None => (zone.parse::<i64>().ok()?, 0),
            };
            sign * (h * 60 + m)
        }
        None => 0,
    };

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    Some(seconds * 1_000 + fraction)
}

/// Days since 1970-01-01 for a Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_date_shape_the_app_has_written_is_read() {
        // Midnight, 1970: the anchor.
        assert_eq!(millis_from_iso("1970-01-01T00:00:00Z"), Some(0));
        // The browser's shape, with milliseconds.
        assert_eq!(
            millis_from_iso("2021-06-10T15:55:15.466Z"),
            Some(1_623_340_515_466)
        );
        // The C#'s, with seven fractional digits and an offset - the offset moves the
        // instant, so this is 04:40:20 UTC and not 07:40:20. (These three numbers are what
        // Python's datetime makes of the same strings; the two I worked out by hand were
        // both wrong, one of them by exactly an hour, which is what hand-computed dates do.)
        assert_eq!(
            millis_from_iso("2022-07-03T07:40:20.4829583+03:00"),
            Some(1_656_823_220_482)
        );
        // This server's, with a space and no zone.
        assert_eq!(
            millis_from_iso("2026-09-09 21:24:50"),
            Some(1_788_989_090_000)
        );
        assert_eq!(millis_from_iso("not a date"), None);
        assert_eq!(millis_from_iso(""), None);
    }

    #[test]
    fn every_shape_of_stored_date_is_read() {
        let iso = serde_json::json!({ "$date": "2022-07-03T04:40:20.482Z" });
        let long = serde_json::json!({ "$date": { "$numberLong": "1656823220482" } });
        let plain = serde_json::json!({ "$date": 1_656_823_220_482i64 });
        for shape in [iso, long, plain] {
            assert_eq!(millis_of(&shape), Some(1_656_823_220_482), "{shape}");
        }
    }

    #[test]
    fn a_date_survives_the_round_trip() {
        for text in [
            "2021-06-10T15:55:15.466Z",
            "2022-07-03T07:40:20.4829583+03:00",
            "2026-09-09 21:24:50",
        ] {
            let millis = millis_from_iso(text).expect(text);
            let back = iso_from_millis(millis);
            assert_eq!(
                millis_from_iso(&back),
                Some(millis),
                "{text} -> {back} is the same instant"
            );
        }
    }

    #[test]
    fn a_tour_becomes_a_document_and_comes_back() {
        let json = include_str!("../../../fixtures/zscph2y.tour.json");
        let tour = Tour::from_json(json).expect("fixture");

        let document = to_document(&tour).expect("a document");
        assert_eq!(
            document.get_str("_id").ok(),
            Some(tour.id.as_str()),
            "keyed the way the C# driver keys it"
        );
        // The dates are dates, not text - which is the whole point of the mapping.
        assert!(
            matches!(document.get("DateCreated"), Some(bson::Bson::DateTime(_))),
            "DateCreated is a BSON date: {:?}",
            document.get("DateCreated")
        );

        let back = to_tour(&document).expect("a tour");
        assert_eq!(back.id, tour.id);
        assert_eq!(back.name, tour.name);
        assert_eq!(back.persons.len(), tour.persons.len());
        assert_eq!(back.spendings.len(), tour.spendings.len());
        assert_eq!(
            tc_core::calculate(&back, tc_core::Options::default()),
            tc_core::calculate(&tour, tc_core::Options::default()),
            "the same money after a trip through the database"
        );
    }
}
