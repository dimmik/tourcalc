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
use crate::subscriptions::{Subscription, SubscriptionStore};
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
    /// The connection, kept so that the subscriptions can share it rather than opening a
    /// second one to the same database.
    client: Client,
    database: String,
}

impl MongoStore {
    /// These fields of these tours, and nothing else - `(id, values in the order asked)`,
    /// an absent field as an empty string. Both spellings of each, because some documents
    /// are camelCase throughout (see `fields`).
    async fn just(&self, ids: &[String], keys: &[&str]) -> Vec<(String, Vec<String>)> {
        if ids.is_empty() {
            return Vec::new();
        }
        let mut projection = Document::new();
        for key in keys {
            let lower = format!("{}{}", key[..1].to_ascii_lowercase(), &key[1..]);
            projection.insert(*key, 1);
            projection.insert(lower, 1);
        }
        let found = match self
            .tours
            .find(doc! { "_id": { "$in": ids.to_vec() } })
            .projection(projection)
            .await
        {
            Ok(cursor) => cursor.try_collect::<Vec<Document>>().await,
            Err(e) => Err(e),
        };
        match found {
            Ok(docs) => docs
                .iter()
                .filter_map(|d| {
                    let id = d.get_str("_id").ok()?.to_owned();
                    let values = keys
                        .iter()
                        .map(|key| {
                            d.iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                                .and_then(|(_, v)| v.as_str())
                                .unwrap_or_default()
                                .to_owned()
                        })
                        .collect();
                    Some((id, values))
                })
                .collect(),
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                Vec::new()
            }
        }
    }

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
        let uri = connection_string(url, username, password);

        let options = ClientOptions::parse(&uri)
            .await
            .map_err(|e| format!("MongoDB connection string: {e}"))?;
        let client = Client::with_options(options).map_err(|e| format!("MongoDB: {e}"))?;

        let store = MongoStore {
            tours: client.database(database).collection::<Document>(collection),
            client: client.clone(),
            database: database.to_owned(),
        };
        store
            .tours
            .find_one(doc! { "_id": "none" })
            .await
            .map_err(|e| format!("MongoDB is not answering: {e}"))?;
        store.make_sure_of_the_indexes().await;
        Ok(store)
    }

    /// The stored document, as it is. For the tests that check its shape - what the C#
    /// would read is the whole claim of this module, and it is worth checking against the
    /// database rather than against the function that writes it.
    pub async fn raw_document_for_tests(&self, id: &str) -> Option<Document> {
        self.tours.find_one(doc! { "_id": id }).await.ok().flatten()
    }

    /// Puts a document in as it stands, without going through [`Tour`]. Tests only: it is
    /// how a document written by the C# app - which this port has to read - gets into the
    /// database without a C# app to write it.
    pub async fn insert_raw_for_tests(&self, doc: Document) {
        let _ = self.tours.insert_one(doc).await;
    }

    /// Puts a subscription document in as the C# writes one. Tests only: reading what the
    /// app stored is half the claim, and there is no app here to store it.
    pub async fn insert_raw_subscription_for_tests(&self, doc: Document) {
        let _ = self
            .client
            .database(&self.database)
            .collection::<Document>("NSubscriptions")
            .insert_one(doc)
            .await;
    }

    /// Empties the collections. Tests only, and it does what it says - subscriptions
    /// included, or a test that runs twice finds what the first run left.
    /// Puts a document in as it stands, for the tests that are about what is already in
    /// somebody's database rather than about what this server writes.
    pub async fn raw_insert_for_tests(&self, document: Document) {
        let _ = self.tours.insert_one(document).await;
    }

    pub async fn wipe_everything_for_tests(&self) {
        let _ = self.tours.delete_many(doc! {}).await;
        let _ = self
            .client
            .database(&self.database)
            .collection::<Document>("NSubscriptions")
            .delete_many(doc! {})
            .await;
    }

    /// Subscriptions, in the collection the C# writes them to, on this same connection.
    /// The indexes every query here depends on, created if they are not there.
    ///
    /// `create_index` on an index that exists does nothing, so this runs on every start and
    /// costs one round trip. Without it a new database - a move, a second instance, a
    /// restore from a dump - answers every list by reading every tour, and nothing says so:
    /// it is simply slow, in a way that looks like the network.
    async fn make_sure_of_the_indexes(&self) {
        use mongodb::IndexModel;
        let tours = [
            // The list: this code's tours, versions excluded.
            doc! { tc_core::extras::ACCESS_CODE: 1, "IsVersion": 1 },
            // A tour's own history.
            doc! { "VersionFor_Id": 1 },
        ];
        for keys in tours {
            let named = format!("{keys:?}");
            if let Err(e) = self
                .tours
                .create_index(IndexModel::builder().keys(keys).build())
                .await
            {
                // Not fatal: a reader without rights to create indexes still serves tours,
                // slowly, and saying so is more use than refusing to start.
                tracing::warn!("could not create the index on {named}: {e}");
            }
        }
        let subscriptions = self.subscriptions().subscriptions;
        for keys in [doc! { "TourId": 1 }, doc! { "Subscription.Url": 1 }] {
            let named = format!("{keys:?}");
            if let Err(e) = subscriptions
                .create_index(IndexModel::builder().keys(keys).build())
                .await
            {
                tracing::warn!("could not create the index on {named}: {e}");
            }
        }
    }

    pub fn subscriptions(&self) -> MongoSubscriptions {
        MongoSubscriptions {
            // "NSubscriptions" is the C#'s own name for it.
            subscriptions: self
                .client
                .database(&self.database)
                .collection::<Document>("NSubscriptions"),
        }
    }

    /// Which tours a list is about: never a version, and - when the caller's codes are
    /// known - only theirs. Without it every list request reads every tour in the database,
    /// whole, to keep the handful that belong to the reader. The C# hands the same
    /// condition to the driver.
    fn list_filter(&self, codes: Option<&[String]>) -> Document {
        let mut filter = doc! { "IsVersion": { "$ne": true } };
        if let Some(codes) = codes {
            filter.insert(tc_core::extras::ACCESS_CODE, doc! { "$in": codes.to_vec() });
        }
        filter
    }

    async fn all(&self, filter: Document) -> Vec<Arc<Tour>> {
        self.all_without(filter, Document::new()).await
    }

    /// The same, leaving out the fields the caller has no use for.
    async fn all_without(&self, filter: Document, projection: Document) -> Vec<Arc<Tour>> {
        let found = match self.tours.find(filter).projection(projection).await {
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

    async fn list(
        &self,
        codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
    ) -> Vec<Arc<Tour>> {
        self.all(self.list_filter(codes))
            .await
            .into_iter()
            .filter(|t| allowed(t))
            .collect()
    }

    async fn page(
        &self,
        codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
        from: usize,
        count: usize,
    ) -> (Vec<Arc<Tour>>, usize) {
        // Two reads, both small. The first asks for nothing but each tour's id and the day
        // it was made, which is what the order is by - newest first, as the C# list was.
        //
        // The order cannot be left to the database: `sort` matches one spelling of a field,
        // and three of the tours in the seed - and whatever else the C# wrote in its
        // camelCase years - spell every field the other way. It cannot be left out either:
        // without an order two pages can share a tour and miss another. So the ids are put
        // in order here, where both spellings are read, and only the page is fetched whole.
        let filter = self.list_filter(codes);
        let mut order: Vec<(String, String, String)> = match self
            .tours
            .find(filter)
            .projection(doc! {
                "_id": 1,
                "GUID": 1,
                "Id": 1,
                "guid": 1,
                "id": 1,
                tc_core::extras::CREATED_AT: 1,
                "dateCreated": 1,
            })
            .await
        {
            Ok(cursor) => match cursor.try_collect::<Vec<Document>>().await {
                Ok(docs) => docs
                    .iter()
                    .filter_map(|d| {
                        let key = d.get_str("_id").ok()?.to_owned();
                        let field = |name: &str| {
                            d.iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                                .and_then(|(_, v)| v.as_str())
                                .map(|s| s.to_owned())
                        };
                        // The tour's own id, which is what a link points at and what the
                        // reader sees as "this tour"; `_id` is the document's.
                        let tour = field("GUID").or_else(|| field("Id")).unwrap_or_else(|| key.clone());
                        let made = d
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(tc_core::extras::CREATED_AT))
                            .map(|(_, v)| stamp_of(v))
                            .unwrap_or_default();
                        Some((made, tour, key))
                    })
                    .collect(),
                Err(e) => {
                    tracing::error!("MongoDB read failed: {e}");
                    return (Vec::new(), 0);
                }
            },
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                return (Vec::new(), 0);
            }
        };
        // Newest first; a tour with no date made is oldest, and the id breaks a tie so that
        // the same page is the same page twice.
        order.sort_by(|a, b| b.cmp(a));
        // One row per tour, not per document. A database can hold two documents that call
        // themselves the same tour - a copy made without a new id, a restore from long ago -
        // and counting those separately is what put the same tour on the list two and three
        // times: the page came back short (the second copy folded into the first), the count
        // said there were more, and the client asked for the rest.
        let mut seen = std::collections::HashSet::new();
        order.retain(|(_, tour, _)| seen.insert(tour.clone()));
        let total = order.len();

        let wanted: Vec<String> = order
            .into_iter()
            .skip(from)
            .take(count)
            .map(|(_, _, key)| key)
            .collect();
        if wanted.is_empty() {
            return (Vec::new(), total);
        }
        // Read by document id and put back in the order asked for. Keyed by `_id` and not
        // by the tour's own id, because those are not always the same string.
        let docs = match self.tours.find(doc! { "_id": { "$in": wanted.clone() } }).await {
            Ok(cursor) => cursor.try_collect::<Vec<Document>>().await.unwrap_or_default(),
            Err(e) => {
                tracing::error!("MongoDB read failed: {e}");
                Vec::new()
            }
        };
        let mut found: std::collections::HashMap<String, Arc<Tour>> = docs
            .iter()
            .filter_map(|d| {
                let key = d.get_str("_id").ok()?.to_owned();
                Some((key, Arc::new(to_tour(d)?)))
            })
            .collect();
        let page = wanted
            .iter()
            .filter_map(|key| found.remove(key))
            .filter(|t| allowed(t))
            .collect();
        (page, total)
    }

    async fn count(
        &self,
        codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
    ) -> usize {
        // Counted by the database when the question is about particular codes - which is
        // every time it is asked, since an administrator is never counted. Without codes the
        // filter alone cannot say who may see what, so that case reads them as `list` does.
        let Some(codes) = codes else {
            return self.list(None, allowed).await.len();
        };
        let filter = doc! {
            "IsVersion": { "$ne": true },
            tc_core::extras::ACCESS_CODE: { "$in": codes.to_vec() },
        };
        match self.tours.count_documents(filter).await {
            Ok(n) => n as usize,
            Err(e) => {
                tracing::error!("MongoDB count failed: {e}");
                self.list(Some(codes), allowed).await.len()
            }
        }
    }

    async fn access_codes(&self, ids: &[String]) -> Vec<(String, String)> {
        // Just the code: these are tours somebody subscribed to, and reading them whole to
        // look at one field would make the tour list pay for every spending in them.
        self.just(ids, &[tc_core::extras::ACCESS_CODE])
            .await
            .into_iter()
            .map(|(id, mut values)| (id, values.remove(0)))
            .collect()
    }

    async fn state_of(&self, id: &TourId) -> Option<(String, String)> {
        // Asked every half minute by every open tour page: two short strings, never the tour.
        let mut found = self
            .just(
                &[id.as_str().to_owned()],
                &[tc_core::extras::ACCESS_CODE, tc_core::extras::STATE],
            )
            .await;
        let (_, mut values) = found.pop()?;
        let state = values.pop()?;
        let code = values.pop()?;
        Some((code, state))
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
                still_at(id.as_str(), expected_state),
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
        // Without what is in them: a version is a whole copy of the tour, and the screen
        // that lists them shows a date and a line of text. A tour saved a hundred times
        // used to be read back a hundred times over, spendings and all, to draw ten rows.
        //
        // The sorting and the paging stay here rather than in the database, because the
        // field they go by is written in two spellings (see `fields`) and one `sort` cannot
        // follow both. What is read is now small enough for that to be cheap.
        let filter = doc! {
            "$or": [
                { "IsVersion": true, "VersionFor_Id": id.as_str() },
                { "isVersion": true, "versionFor_Id": id.as_str() },
            ]
        };
        let mut mine = self
            .all_without(filter, doc! { "Spendings": 0, "Persons": 0, "spendings": 0, "persons": 0 })
            .await;

        mine.sort_by(|a, b| {
            crate::fields::str_of(b, crate::fields::VERSIONED_AT)
                .cmp(&crate::fields::str_of(a, crate::fields::VERSIONED_AT))
        });

        let total = mine.len();
        (mine.into_iter().skip(from).take(count).collect(), total)
    }
}

/// Who wants telling, kept where the C# keeps them.
///
/// The whole reason this exists: the in-memory version forgets everybody when the process
/// stops, and this process stops every time a new image is pulled. A reader who turned
/// notifications on would quietly stop being told, and nothing on their screen would say
/// so - the browser still holds a subscription the server has never heard of.
///
/// The document is the C#'s: `{ TourId, Subscription: { Url, P256dh, Auth } }` in
/// `NSubscriptions`, keyed by nothing in particular - it looks them up by the pair. Same
/// shape both ways, so the two servers can hand the collection back and forth.
pub struct MongoSubscriptions {
    pub(crate) subscriptions: Collection<Document>,
}

impl MongoSubscriptions {
    /// The pair that identifies one: a tour and the endpoint the browser gave us. The keys
    /// are not part of it - a browser renews those for the same endpoint, and the C#
    /// compares the same way.
    fn mine(tour: &str, sub: &Subscription) -> Document {
        doc! { "TourId": tour, "Subscription.Url": &sub.url }
    }

    fn as_document(tour: &str, sub: &Subscription) -> Document {
        doc! {
            "TourId": tour,
            "Subscription": {
                "Url": &sub.url,
                "P256dh": &sub.p256dh,
                "Auth": &sub.auth,
            },
        }
    }

    fn as_subscription(document: &Document) -> Option<Subscription> {
        let sub = document.get_document("Subscription").ok()?;
        let text = |key: &str| sub.get_str(key).unwrap_or_default().to_owned();
        let url = text("Url");
        (!url.is_empty()).then(|| Subscription {
            url,
            p256dh: text("P256dh"),
            auth: text("Auth"),
        })
    }
}

#[async_trait::async_trait]
impl SubscriptionStore for MongoSubscriptions {
    async fn add(&self, tour: &str, sub: Subscription) {
        // Replacing rather than inserting: subscribing twice from the same browser is one
        // subscription, and the keys may be the renewed ones. `upsert` makes the first time
        // and every time after it the same operation.
        let update = doc! { "$set": Self::as_document(tour, &sub) };
        if let Err(e) = self
            .subscriptions
            .update_one(Self::mine(tour, &sub), update)
            .upsert(true)
            .await
        {
            tracing::error!("could not store a subscription: {e}");
        }
    }

    async fn remove(&self, tour: &str, sub: &Subscription) {
        if let Err(e) = self.subscriptions.delete_one(Self::mine(tour, sub)).await {
            tracing::error!("could not remove a subscription: {e}");
        }
    }

    async fn has(&self, tour: &str, sub: &Subscription) -> bool {
        matches!(
            self.subscriptions.find_one(Self::mine(tour, sub)).await,
            Ok(Some(_))
        )
    }

    async fn for_tour(&self, tour: &str) -> Vec<Subscription> {
        let found = match self.subscriptions.find(doc! { "TourId": tour }).await {
            Ok(cursor) => cursor.try_collect::<Vec<Document>>().await,
            Err(e) => {
                tracing::error!("could not read the subscriptions: {e}");
                return Vec::new();
            }
        };
        let mut mine: Vec<Subscription> = match found {
            Ok(docs) => docs.iter().filter_map(Self::as_subscription).collect(),
            Err(e) => {
                tracing::error!("could not read the subscriptions: {e}");
                return Vec::new();
            }
        };
        // The C# collection can hold the same endpoint twice - its `AddSubscription` checks
        // first and inserts, and two saves at once both pass the check. Sending the same
        // notification twice is the reader's problem, so it is dealt with here rather than
        // relied upon not to happen. Sorted first, because `dedup` only looks at
        // neighbours and two copies need not be stored next to each other.
        mine.sort_by(|a, b| a.url.cmp(&b.url));
        mine.dedup_by(|a, b| a.is_same(b));
        mine
    }

    async fn tours_of(&self, sub: &Subscription) -> Vec<String> {
        // Only the tour ids: the keys are of no use to a list of bells. No index on the URL,
        // on purpose - the collection is a few hundred rows, a scan of it is a millisecond,
        // and an index would be this server changing the shape of a database it shares.
        let found = match self
            .subscriptions
            .find(doc! { "Subscription.Url": &sub.url })
            .projection(doc! { "TourId": 1, "_id": 0 })
            .await
        {
            Ok(cursor) => cursor.try_collect::<Vec<Document>>().await,
            Err(e) => Err(e),
        };
        let mut tours: Vec<String> = match found {
            Ok(docs) => docs
                .iter()
                .filter_map(|d| d.get_str("TourId").ok().map(str::to_owned))
                .collect(),
            Err(e) => {
                tracing::error!("could not read the subscriptions: {e}");
                return Vec::new();
            }
        };
        // The same pair can be stored twice by the C# (see `for_tour`).
        tours.sort();
        tours.dedup();
        tours
    }
}

/// The URI to connect with, from the three settings the deployment already sets.
///
/// A whole connection string in `MongoDbUrl` is used as it is; a bare host is turned into
/// the `mongodb+srv://` form the C# builds, with the credentials url-encoded.
///
/// **Without the C#'s `?connect=replicaSet`.** That option is the old .NET driver's way of
/// being told the topology; this driver works it out, and rejects the option outright -
/// "connect is an invalid option". Copying the C#'s string verbatim meant the server would
/// not start against the deployment's own configuration, which is the sort of thing that is
/// only found by trying it.
pub fn connection_string(url: &str, username: &str, password: &str) -> String {
    if url.contains("://") {
        return url.to_owned();
    }
    let enc = |s: &str| -> String { form_urlencoded::byte_serialize(s.as_bytes()).collect() };
    if username.is_empty() {
        return format!("mongodb+srv://{url}");
    }
    format!("mongodb+srv://{}:{}@{url}", enc(username), enc(password))
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

/// The soft lock as a filter: which documents count as "still the state the caller read".
///
/// Not simply `{ StateGUID: expected }`, for two reasons, both of them old tours. A tour
/// written before this field existed has no `StateGUID` at all, and in MongoDB a missing
/// field does not equal `""` - so every save of such a tour matched nothing, was read as
/// somebody else having saved first, and came back 409 for ever. And a tour the C# wrote in
/// its camelCase years spells the field `stateGUID`, which is a different field again.
fn still_at(id: &str, expected: &str) -> Document {
    // Nothing read means nothing stored: the tour must still have no state in either
    // spelling. Written as two conditions rather than one list, so that a tour which *has*
    // a state cannot be overwritten by a save that carries none.
    let missing = |field: &str| {
        doc! { "$or": [
            { field: { "$exists": false } },
            { field: "" },
            { field: bson::Bson::Null },
        ] }
    };
    if expected.is_empty() {
        return doc! {
            "_id": id,
            "$and": [ missing("StateGUID"), missing("stateGUID") ],
        };
    }
    doc! {
        "_id": id,
        "$or": [ { "StateGUID": expected }, { "stateGUID": expected } ],
    }
}

/// A stored date as the text it is compared by. Mongo holds `DateCreated` as a BSON date
/// where this server wrote it and as a string where the C# did; both sort as text once the
/// date is written the ISO way round.
fn stamp_of(value: &bson::Bson) -> String {
    match value {
        bson::Bson::String(s) => s.clone(),
        bson::Bson::DateTime(d) => d.try_to_rfc3339_string().unwrap_or_default(),
        other => other.to_string(),
    }
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

    /// The deployment's own settings make a string this driver accepts.
    ///
    /// Written from the configuration file the server actually runs with: a bare Atlas host,
    /// a username, and a password that may be supplied by other means. The C#'s
    /// `?connect=replicaSet` is deliberately absent - see [`connection_string`].
    #[tokio::test]
    async fn the_deployments_settings_make_a_string_the_driver_accepts() {
        let uri = connection_string("cluster-name.example.mongodb.net", "mongo", "p@ss word");
        assert_eq!(
            uri, "mongodb+srv://mongo:p%40ss+word@cluster-name.example.mongodb.net",
            "credentials are url-encoded, and no legacy options are added"
        );

        // Parsing an srv URI looks the host up, so a made-up one cannot get past DNS. What
        // matters is *which* complaint comes back: about the name, not about an option.
        let complaint = mongodb::options::ClientOptions::parse(&uri)
            .await
            .expect_err("no such cluster")
            .to_string();
        assert!(
            !complaint.contains("invalid option"),
            "the options are accepted; only the lookup fails: {complaint}"
        );

        // A whole connection string is taken as it is.
        let plain = connection_string("mongodb://user:pass@localhost:27017", "ignored", "ignored");
        assert!(mongodb::options::ClientOptions::parse(&plain).await.is_ok());

        // And no credentials at all is a valid string too - a database that wants none.
        let open = connection_string("localhost.example.net", "", "");
        assert_eq!(open, "mongodb+srv://localhost.example.net");
    }

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
