//! Tourcalc as a Cloudflare Worker.
//!
//! The Worker hands every request it gets - the API, the text pages - to one Durable Object,
//! and the object is the server: the same axum router `tc-server` builds (`api::routes`,
//! `text::routes`), over the server's in-memory store. The client's files never reach here:
//! Cloudflare serves them as static assets (`wrangler.toml`).
//!
//! What the process's memory is to the server, the object's memory is here; what would be
//! lost when it stops is kept in the object's own storage (SQLite): every tour and version a
//! request changed is written there before its answer goes, and an object that wakes up reads
//! them all back. `POST /seed` (with the master key) adds tours from a seed file - the format
//! `tc-server import` reads.

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tc_core::{Tour, TourId};
use tc_server::store::{InMemoryStore, Stale, TourStore};
use tower::ServiceExt;
use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // One object, one server: it is the only place the tours are, and the one writer.
    let stub = env.durable_object("STORE")?.id_from_name("server")?.get_stub()?;
    stub.fetch_with_request(req).await
}

fn fail(e: impl std::fmt::Display) -> Error {
    Error::RustError(e.to_string())
}

/// The server's in-memory store, and a note of every tour or version a request changed - so
/// that the object can write just those to its storage once the request is answered.
struct Tracked {
    inner: Arc<InMemoryStore>,
    changed: Arc<Mutex<HashSet<String>>>,
}

impl Tracked {
    fn mark(&self, id: &str) {
        self.changed.lock().unwrap_or_else(|p| p.into_inner()).insert(id.to_owned());
    }
}

#[async_trait::async_trait]
impl TourStore for Tracked {
    async fn get(&self, id: &TourId) -> Option<Arc<Tour>> {
        self.inner.get(id).await
    }
    async fn list(&self, codes: Option<&[String]>, allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync)) -> Vec<Arc<Tour>> {
        self.inner.list(codes, allowed).await
    }
    async fn page(
        &self,
        codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
        from: usize,
        count: usize,
    ) -> (Vec<Arc<Tour>>, usize) {
        self.inner.page(codes, allowed, from, count).await
    }
    async fn count(&self, codes: Option<&[String]>, allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync)) -> usize {
        self.inner.count(codes, allowed).await
    }
    async fn access_codes(&self, ids: &[String]) -> Vec<(String, String)> {
        self.inner.access_codes(ids).await
    }
    async fn state_of(&self, id: &TourId) -> Option<(String, String)> {
        self.inner.state_of(id).await
    }
    async fn store(&self, tour: Tour) {
        let id = tour.id.as_str().to_owned();
        self.inner.store(tour).await;
        self.mark(&id);
    }
    async fn remove(&self, id: &TourId) -> bool {
        let gone = self.inner.remove(id).await;
        self.mark(id.as_str());
        gone
    }
    async fn replace(
        &self,
        id: &TourId,
        expected_state: &str,
        next: Tour,
        version_of: &(dyn for<'a> Fn(&'a Tour) -> Option<Tour> + Sync),
    ) -> std::result::Result<(), Stale> {
        // The version kept is a tour of its own, under its own id: noted as it is made.
        let kept = Mutex::new(Vec::new());
        let noting = |t: &Tour| {
            let v = version_of(t);
            if let Some(v) = &v {
                kept.lock().unwrap_or_else(|p| p.into_inner()).push(v.id.as_str().to_owned());
            }
            v
        };
        let done = self.inner.replace(id, expected_state, next, &noting).await;
        if done.is_ok() {
            self.mark(id.as_str());
            for v in kept.into_inner().unwrap_or_else(|p| p.into_inner()) {
                self.mark(&v);
            }
        }
        done
    }
    async fn recomment(&self, version: &TourId, comment: &str) {
        self.inner.recomment(version, comment).await;
        self.mark(version.as_str());
    }
    async fn versions(&self, id: &TourId, from: usize, count: usize) -> (Vec<Arc<Tour>>, usize) {
        self.inner.versions(id, from, count).await
    }
    fn status(&self, admin: bool) -> serde_json::Value {
        let mut s = serde_json::json!({ "kind": "CloudflareDurableObject" });
        if admin {
            s["inMemory"] = self.inner.len().into();
        }
        s
    }
}

/// The running server: the router, and what the object needs to keep its storage in step.
struct App {
    router: axum::Router,
    tours: Arc<InMemoryStore>,
    changed: Arc<Mutex<HashSet<String>>>,
    /// Ids in the order they were first stored - the order of the tour list.
    order: RefCell<Vec<String>>,
    known: RefCell<HashSet<String>>,
}

#[durable_object]
pub struct Store {
    state: State,
    env: Env,
    app: RefCell<Option<Arc<App>>>,
}

impl DurableObject for Store {
    fn new(state: State, env: Env) -> Self {
        Store { state, env, app: RefCell::new(None) }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let app = self.app().await?;
        if req.path() == "/seed" && req.method() == Method::Post {
            return self.seed(&app, &mut req).await;
        }
        let request = to_axum(&mut req).await?;
        let answer = app.router.clone().oneshot(request).await.map_err(fail)?;
        // Kept before the answer goes: an answered save is a stored one.
        self.keep(&app).await?;
        from_axum(answer).await
    }
}

impl Store {
    /// The server, made on the first request after the object woke: every tour read back
    /// from the object's storage.
    async fn app(&self) -> Result<Arc<App>> {
        if let Some(app) = self.app.borrow().as_ref() {
            return Ok(app.clone());
        }
        let storage = self.state.storage();
        let order: Vec<String> = storage.get("order").await.ok().flatten().unwrap_or_default();
        let mut texts = std::collections::HashMap::new();
        let all = storage.list().await?;
        for entry in all.entries() {
            let entry: worker::js_sys::Array = entry.map_err(|e| fail(format!("{e:?}")))?.into();
            if let (Some(key), Some(text)) = (entry.get(0).as_string(), entry.get(1).as_string()) {
                if let Some(id) = key.strip_prefix("t:") {
                    texts.insert(id.to_owned(), text);
                }
            }
        }
        let tours = Arc::new(InMemoryStore::empty());
        let mut known = HashSet::new();
        let mut listed = Vec::new();
        // In the order they came; anything the order misses (it should not) after them.
        let mut ids: Vec<String> = order.into_iter().filter(|id| texts.contains_key(id)).collect();
        let rest: Vec<String> = texts.keys().filter(|id| !ids.contains(id)).cloned().collect();
        ids.extend(rest);
        for id in ids {
            if let Ok(tour) = Tour::from_json(&texts[&id]) {
                tours.store(tour).await;
                known.insert(id.clone());
                listed.push(id);
            }
        }
        let changed = Arc::new(Mutex::new(HashSet::new()));
        let store = Tracked { inner: tours.clone(), changed: changed.clone() };
        let state = self.server_state(Box::new(store))?;
        let router = axum::Router::new()
            .merge(tc_server::api::routes(state.clone()))
            .merge(tc_server::text::routes().with_state(state))
            .route("/service-worker.js", axum::routing::get(tc_server::retire_the_old_service_worker));
        let app = Arc::new(App { router, tours, changed, order: RefCell::new(listed), known: RefCell::new(known) });
        *self.app.borrow_mut() = Some(app.clone());
        Ok(app)
    }

    fn var(&self, name: &str) -> Option<String> {
        self.env.secret(name).map(|s| s.to_string()).or_else(|_| self.env.var(name).map(|v| v.to_string())).ok().filter(|v| !v.trim().is_empty())
    }

    /// What `main` puts together from the environment, from the Worker's variables and
    /// secrets instead.
    fn server_state(&self, store: Box<dyn TourStore>) -> Result<tc_server::state::Shared> {
        let mut cfg = tc_server::config::Config::from_env();
        let flag = |name: &str, default: bool| self.var(name).and_then(|v| v.trim().parse().ok()).unwrap_or(default);
        cfg.master_key = self.var("MasterKey").ok_or_else(|| fail("MasterKey is not set"))?;
        cfg.private_key_b64 = self.var("AuthPrivateECDSAKey").ok_or_else(|| fail("AuthPrivateECDSAKey is not set"))?;
        cfg.build_type = self.var("BUILD_TYPE").unwrap_or_else(|| "cloudflare".into());
        cfg.build_id = self.var("BUILD_ID").unwrap_or_else(|| "dev".into());
        cfg.build_commit = self.var("BUILD_COMMIT").unwrap_or_default();
        cfg.versioning = flag("TourVersioning", true);
        cfg.menu_versioning = flag("TourMenuVersioning", false);
        cfg.version_editable = flag("TourVersionEditable", false);
        let signer = tc_server::auth::Signer_::from_base64(&cfg.private_key_b64).map_err(fail)?;
        Ok(Arc::new(tc_server::state::AppState {
            store,
            // Kept in memory only: lost when the object sleeps. Nothing is sent anyway - push
            // needs reqwest, which this build goes without.
            subscriptions: Box::new(tc_server::subscriptions::InMemorySubscriptions::default()),
            push: Box::new(tc_server::push::Silent),
            signer,
            master_key: cfg.master_key.clone(),
            token_valid_minutes: cfg.token_valid_minutes,
            started: tc_server::clock::now(),
            versioning: cfg.versioning,
            version_editable: cfg.version_editable,
            menu_versioning: cfg.menu_versioning,
            max_tours_per_code: cfg.max_tours_per_code,
            wakeup_code: cfg.wakeup_code.clone(),
            wakeup_pre_delay_min: cfg.wakeup_pre_delay_min,
            wakeup_post_delay_min: cfg.wakeup_post_delay_min,
            build_type: cfg.build_type.clone(),
            build_id: cfg.build_id.clone(),
            build_commit: cfg.build_commit.clone(),
            client_asset: self.var("CLIENT_ASSET"),
            wakeups: Default::default(),
            forgotten_subscriptions: Default::default(),
        }))
    }

    /// Writes what the last request changed to the object's storage.
    async fn keep(&self, app: &App) -> Result<()> {
        let ids: Vec<String> = app.changed.lock().unwrap_or_else(|p| p.into_inner()).drain().collect();
        if ids.is_empty() {
            return Ok(());
        }
        let storage = self.state.storage();
        let mut reordered = false;
        for id in ids {
            match app.tours.get(&TourId::new(id.clone())).await {
                Some(tour) => {
                    storage.put(&format!("t:{id}"), tour.to_json().map_err(fail)?).await?;
                    if app.known.borrow_mut().insert(id.clone()) {
                        app.order.borrow_mut().push(id);
                        reordered = true;
                    }
                }
                None => {
                    storage.delete(&format!("t:{id}")).await?;
                    if app.known.borrow_mut().remove(&id) {
                        app.order.borrow_mut().retain(|x| x != &id);
                        reordered = true;
                    }
                }
            }
        }
        if reordered {
            let order = app.order.borrow().clone();
            storage.put("order", order).await?;
        }
        Ok(())
    }

    /// Tours from a seed file - a JSON array, as `tc-server import` reads - added or written
    /// over. Only with the master key.
    async fn seed(&self, app: &App, req: &mut Request) -> Result<Response> {
        let key = req.headers().get("x-master-key")?.unwrap_or_default();
        if self.var("MasterKey").as_deref() != Some(key.as_str()) || key.is_empty() {
            return Response::error("no", 403);
        }
        let values: Vec<serde_json::Value> = serde_json::from_str(&req.text().await?).map_err(fail)?;
        let mut seen = HashSet::new();
        let mut added = 0;
        for v in values {
            let Ok(tour) = Tour::from_json(&v.to_string()) else { continue };
            let id = tour.id.as_str().to_owned();
            // The first of a repeated id wins, as the server's own seed reading has it.
            if id.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            app.tours.store(tour).await;
            app.changed.lock().unwrap_or_else(|p| p.into_inner()).insert(id);
            added += 1;
        }
        self.keep(app).await?;
        Response::from_json(&serde_json::json!({ "stored": added, "inMemory": app.tours.len() }))
    }
}

/// The Worker's request as the router takes it.
async fn to_axum(req: &mut Request) -> Result<http::Request<axum::body::Body>> {
    let url = req.url()?;
    let uri = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_owned(),
    };
    let mut builder = http::Request::builder().method(req.method().to_string().as_str()).uri(uri);
    for (name, value) in req.headers() {
        builder = builder.header(name, value);
    }
    let body = req.bytes().await.unwrap_or_default();
    builder.body(axum::body::Body::from(body)).map_err(fail)
}

/// The router's answer as the Worker gives it.
async fn from_axum(answer: http::Response<axum::body::Body>) -> Result<Response> {
    use http_body_util::BodyExt;
    let (parts, body) = answer.into_parts();
    let bytes = body.collect().await.map_err(fail)?.to_bytes();
    let headers = Headers::new();
    for (name, value) in parts.headers.iter() {
        if let Ok(v) = value.to_str() {
            headers.append(name.as_str(), v)?;
        }
    }
    Ok(Response::from_bytes(bytes.to_vec())?.with_status(parts.status.as_u16()).with_headers(headers))
}
