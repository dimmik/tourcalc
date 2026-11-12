//! Tourcalc as a Cloudflare Worker.
//!
//! The Worker hands every request it gets - the API, the text pages - to one Durable Object,
//! and the object is the server: the same axum router `tc-server` builds (`api::routes`,
//! `text::routes`), over the server's in-memory store. The client's files never reach here:
//! Cloudflare serves them as static assets (`wrangler.toml`).
//!
//! Where the tours are, as with the server's GitHub store (`rust/github-store.md`):
//! - **the repository is the truth**: a branch of its own (`GITHUB_BRANCH`) in `GITHUB_REPO`,
//!   every file sealed with `STORE_KEY`, laid out as the server lays it out (`t/`, `v/`);
//! - **the object's SQLite is the cache**, and the list of what is not written yet: a request's
//!   changes are kept there before its answer goes, and the object's alarm writes them to the
//!   branch 30 s later, everything waiting in one commit;
//! - an object with nothing in its storage starts from the branch, read whole as one archive.
//!
//! `POST /seed` (with the master key) adds tours from a seed file - the format `tc-server
//! import` reads.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tc_core::{Tour, TourId};
use tc_server::github::seal::Keys;
use tc_server::store::{InMemoryStore, Stale, TourStore};
use tower::ServiceExt;
use worker::wasm_bindgen::JsValue;
use worker::*;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // One object, one server: the one place the tours are, and the one writer. Another name
    // (`STORE_OBJECT`) is an object with nothing in it - how a start from the branch is tried.
    let name = env.var("STORE_OBJECT").map(|v| v.to_string()).unwrap_or_else(|_| "server".into());
    let stub = env.durable_object("STORE")?.id_from_name(&name)?.get_stub()?;
    stub.fetch_with_request(req).await
}

fn fail(e: impl std::fmt::Display) -> Error {
    Error::RustError(e.to_string())
}

fn locked<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Changes wait this long and go in one commit - the server's `GitHubFlushSeconds`.
const WRITE_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// What the help page says about the store, shared between the object and the store it hands
/// the router: the same fields the server's GitHub store gives (`/api/Info/storage`).
#[derive(Default)]
struct Written {
    pending: usize,
    last_written: Option<String>,
    head: Option<String>,
    last_error: Option<String>,
    next_write_at: Option<f64>,
}

/// The server's in-memory store, and a note of every tour or version a request changed - so
/// that the object can keep just those once the request is answered.
struct Tracked {
    inner: Arc<InMemoryStore>,
    changed: Arc<Mutex<HashSet<String>>>,
    written: Arc<Mutex<Written>>,
    repo: String,
}

impl Tracked {
    fn mark(&self, id: &str) {
        locked(&self.changed).insert(id.to_owned());
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
                locked(&kept).push(v.id.as_str().to_owned());
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
        let w = locked(&self.written);
        let next = w.next_write_at.map(|at| ((at - Date::now().as_millis() as f64) / 1000.0).max(0.0).round() as u64);
        let mut s = serde_json::json!({
            "kind": "GitHub",
            "emulated": false,
            "pending": w.pending,
            "lastWritten": w.last_written,
            "nextWriteInSeconds": next,
            "lastError": w.last_error.as_ref().map(|m| serde_json::json!({ "message": m })),
        });
        if admin {
            s["repo"] = self.repo.clone().into();
            s["head"] = w.head.clone().into();
            s["inMemory"] = self.inner.len().into();
            s["host"] = "Cloudflare Durable Object".into();
        }
        s
    }
}

/// The running server: the router, and what the object needs to keep its storage in step.
struct App {
    router: axum::Router,
    tours: Arc<InMemoryStore>,
    changed: Arc<Mutex<HashSet<String>>>,
    written: Arc<Mutex<Written>>,
    /// Ids in the order they were first stored - the order of the tour list.
    order: RefCell<Vec<String>>,
    known: RefCell<HashSet<String>>,
}

/// What waits to be written, by path in the branch: the tour to write there (`Some(id)`) or
/// nothing (`None` - a removal), and a counter, so that a change made while its commit was on
/// its way is not taken as written.
type Pending = BTreeMap<String, (Option<String>, u64)>;

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

    /// The commit: everything waiting, in one, on top of the branch's head.
    async fn alarm(&self) -> Result<Response> {
        let app = self.app().await?;
        let pending = self.pending().await;
        if pending.is_empty() {
            return Response::ok("nothing waits");
        }
        let storage = self.state.storage();
        let outcome = self.commit(&pending).await;
        let mut w = locked(&app.written);
        match outcome {
            Ok(head) => {
                // Only what was written: a change made meanwhile has a newer counter and waits.
                let mut now = self.pending().await;
                now.retain(|path, (_, n)| pending.get(path).map(|(_, m)| m) != Some(n));
                storage.put("pending", &now).await?;
                let at = tc_server::api::chrono_lite::Utc::from(tc_server::clock::now()).to_string();
                storage.put("head", &head).await?;
                storage.put("last-written", &at).await?;
                storage.delete("last-error").await?;
                w.pending = now.len();
                w.head = Some(head);
                w.last_written = Some(at);
                w.last_error = None;
                w.next_write_at = None;
                if !now.is_empty() {
                    storage.set_alarm(WRITE_AFTER).await?;
                    w.next_write_at = Some(Date::now().as_millis() as f64 + WRITE_AFTER.as_millis() as f64);
                }
            }
            Err(e) => {
                let message = e.to_string();
                storage.put("last-error", &message).await?;
                storage.set_alarm(WRITE_AFTER).await?;
                w.last_error = Some(message);
                w.next_write_at = Some(Date::now().as_millis() as f64 + WRITE_AFTER.as_millis() as f64);
            }
        }
        Response::ok("done")
    }
}

impl Store {
    /// The server, made on the first request after the object woke: every tour read back from
    /// the object's storage - or, when that is empty, from the branch.
    async fn app(&self) -> Result<Arc<App>> {
        if let Some(app) = self.app.borrow().as_ref() {
            return Ok(app.clone());
        }
        let storage = self.state.storage();
        let mut texts = HashMap::new();
        let all = storage.list().await?;
        for entry in all.entries() {
            let entry: worker::js_sys::Array = entry.map_err(|e| fail(format!("{e:?}")))?.into();
            if let (Some(key), Some(text)) = (entry.get(0).as_string(), entry.get(1).as_string()) {
                if let Some(id) = key.strip_prefix("t:") {
                    texts.insert(id.to_owned(), text);
                }
            }
        }
        let mut order: Vec<String> = storage.get("order").await.ok().flatten().unwrap_or_default();
        if texts.is_empty() {
            // Nothing here: a new object, or its storage lost. The branch has it all - and
            // what came from it is written already: its head is noted, nothing waits.
            let (head, files) = self.from_branch().await?;
            for (id, text) in files {
                storage.put(&format!("t:{id}"), &text).await?;
                order.push(id.clone());
                texts.insert(id, text);
            }
            storage.put("order", &order).await?;
            if let Some(head) = head {
                storage.put("head", &head).await?;
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
            if known.contains(&id) {
                continue;
            }
            if let Ok(tour) = Tour::from_json(&texts[&id]) {
                tours.store(tour).await;
                known.insert(id.clone());
                listed.push(id);
            }
        }

        let written = Arc::new(Mutex::new(Written {
            pending: self.pending().await.len(),
            last_written: storage.get("last-written").await.ok().flatten(),
            head: storage.get("head").await.ok().flatten(),
            last_error: storage.get("last-error").await.ok().flatten(),
            next_write_at: storage.get_alarm().await.ok().flatten().map(|t| t as f64),
        }));
        let changed = Arc::new(Mutex::new(HashSet::new()));
        let store = Tracked {
            inner: tours.clone(),
            changed: changed.clone(),
            written: written.clone(),
            repo: format!("{}@{}", self.var("GITHUB_REPO").unwrap_or_default(), self.var("GITHUB_BRANCH").unwrap_or_default()),
        };
        let state = self.server_state(Box::new(store))?;
        let router = axum::Router::new()
            .merge(tc_server::api::routes(state.clone()))
            .merge(tc_server::text::routes().with_state(state))
            .route("/service-worker.js", axum::routing::get(tc_server::retire_the_old_service_worker));
        let app = Arc::new(App { router, tours, changed, written, order: RefCell::new(listed), known: RefCell::new(known) });
        *self.app.borrow_mut() = Some(app.clone());

        // Never written to the branch (the tours were here before it was): all of it waits.
        if app.written.lock().map(|w| w.head.is_none()).unwrap_or(false) {
            let ids: Vec<String> = app.order.borrow().clone();
            locked(&app.changed).extend(ids);
            self.keep(&app).await?;
        }
        Ok(app)
    }

    fn var(&self, name: &str) -> Option<String> {
        self.env
            .secret(name)
            .map(|s| s.to_string())
            .or_else(|_| self.env.var(name).map(|v| v.to_string()))
            .ok()
            .filter(|v| !v.trim().is_empty())
    }

    fn keys(&self) -> Result<Keys> {
        Keys::from_setting(&self.var("STORE_KEY").ok_or_else(|| fail("STORE_KEY is not set"))?).map_err(|e| fail(e.0))
    }

    /// Where a tour or a version lives in the branch - as the server's GitHub store has it.
    fn path_of(&self, keys: &Keys, json: &str) -> Option<String> {
        let tour = Tour::from_json(json).ok()?;
        Some(if tc_server::fields::is_version(&tour) {
            keys.version_path(&tc_server::fields::str_of(&tour, tc_server::fields::VERSION_FOR), tour.id.as_str())
        } else {
            keys.tour_path(tour.id.as_str())
        })
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

    async fn pending(&self) -> Pending {
        self.state.storage().get::<Pending>("pending").await.ok().flatten().unwrap_or_default()
    }

    /// Writes what the last request changed to the object's storage, and lists it for the
    /// branch.
    async fn keep(&self, app: &App) -> Result<()> {
        let ids: Vec<String> = locked(&app.changed).drain().collect();
        if ids.is_empty() {
            return Ok(());
        }
        let keys = self.keys()?;
        let storage = self.state.storage();
        let mut pending = self.pending().await;
        let mut counter: u64 = storage.get("counter").await.ok().flatten().unwrap_or(0);
        let mut reordered = false;
        for id in ids {
            counter += 1;
            let key = format!("t:{id}");
            match app.tours.get(&TourId::new(id.clone())).await {
                Some(tour) => {
                    let json = tour.to_json().map_err(fail)?;
                    if let Some(path) = self.path_of(&keys, &json) {
                        pending.insert(path, (Some(id.clone()), counter));
                    }
                    storage.put(&key, &json).await?;
                    if app.known.borrow_mut().insert(id.clone()) {
                        app.order.borrow_mut().push(id);
                        reordered = true;
                    }
                }
                None => {
                    // Its path is in what was kept of it.
                    let old: Option<String> = storage.get(&key).await.ok().flatten();
                    if let Some(path) = old.and_then(|j| self.path_of(&keys, &j)) {
                        pending.insert(path, (None, counter));
                    }
                    storage.delete(&key).await?;
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
        storage.put("counter", counter).await?;
        storage.put("pending", &pending).await?;
        let mut w = locked(&app.written);
        w.pending = pending.len();
        if storage.get_alarm().await?.is_none() {
            storage.set_alarm(WRITE_AFTER).await?;
            w.next_write_at = Some(Date::now().as_millis() as f64 + WRITE_AFTER.as_millis() as f64);
        }
        Ok(())
    }

    /// Tours from a seed file - a JSON array, as `tc-server import` reads - added or written
    /// over. Only with the master key.
    async fn seed(&self, app: &App, req: &mut Request) -> Result<Response> {
        let key = req.headers().get("x-master-key")?.unwrap_or_default();
        if key.is_empty() || self.var("MasterKey").as_deref() != Some(key.as_str()) {
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
            locked(&app.changed).insert(id);
            added += 1;
        }
        self.keep(app).await?;
        Response::from_json(&serde_json::json!({ "stored": added, "inMemory": app.tours.len() }))
    }

    // --- the branch -------------------------------------------------------------------------

    async fn github(&self, method: Method, path: &str, body: Option<serde_json::Value>) -> Result<(u16, serde_json::Value)> {
        let mut resp = self.github_raw(method, &format!("/repos/{}{path}", self.repo()?), body).await?;
        let status = resp.status_code();
        let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
        Ok((status, json))
    }

    async fn github_raw(&self, method: Method, path: &str, body: Option<serde_json::Value>) -> Result<Response> {
        let token = self.var("GITHUB_TOKEN").ok_or_else(|| fail("GITHUB_TOKEN is not set"))?;
        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", token.trim()))?;
        headers.set("Accept", "application/vnd.github+json")?;
        headers.set("X-GitHub-Api-Version", "2022-11-28")?;
        headers.set("User-Agent", "tourcalc")?;
        let mut init = RequestInit::new();
        init.with_method(method).with_headers(headers);
        if let Some(b) = body {
            init.with_body(Some(JsValue::from_str(&b.to_string())));
        }
        Fetch::Request(Request::new_with_init(&format!("https://api.github.com{path}"), &init)?).send().await
    }

    fn repo(&self) -> Result<String> {
        self.var("GITHUB_REPO").ok_or_else(|| fail("GITHUB_REPO is not set"))
    }

    fn branch(&self) -> Result<String> {
        self.var("GITHUB_BRANCH").ok_or_else(|| fail("GITHUB_BRANCH is not set"))
    }

    /// A 2xx answer, or GitHub's status and message - never the token.
    async fn ok(&self, method: Method, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let (status, json) = self.github(method, path, body).await?;
        if !(200..300).contains(&status) {
            let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
            return Err(fail(format!("GitHub answered {status} to {path}: {message}")));
        }
        Ok(json)
    }

    fn sha(json: &serde_json::Value, pointer: &str) -> String {
        json.pointer(pointer).and_then(|s| s.as_str()).unwrap_or_default().to_owned()
    }

    /// The branch's head; a branch of its own made when it is not there yet - with nothing
    /// in it but a note, not from `master`: another store's files are not this one's.
    async fn head(&self) -> Result<String> {
        let branch = self.branch()?;
        let (status, r) = self.github(Method::Get, &format!("/git/ref/heads/{branch}"), None).await?;
        if status == 200 {
            return Ok(Self::sha(&r, "/object/sha"));
        }
        let note = "Tourcalc's tours, sealed - written by the Cloudflare Worker (cloudflare/worker \
                    in dimmik/tourcalc). Nothing here reads without the store key.\n";
        let tree = self
            .ok(Method::Post, "/git/trees", Some(serde_json::json!({ "tree": [{ "path": "README.md", "mode": "100644", "type": "blob", "content": note }] })))
            .await?;
        let commit = self
            .ok(Method::Post, "/git/commits", Some(serde_json::json!({ "message": "A store of its own", "tree": Self::sha(&tree, "/sha"), "parents": [] })))
            .await?;
        let sha = Self::sha(&commit, "/sha");
        self.ok(Method::Post, "/git/refs", Some(serde_json::json!({ "ref": format!("refs/heads/{branch}"), "sha": sha }))).await?;
        Ok(sha)
    }

    /// Everything waiting as one commit: the head, its tree, a tree on top of it, the commit,
    /// the branch moved - five requests however many files. The branch moved by somebody else
    /// meanwhile: once more on top of where it is now.
    async fn commit(&self, pending: &Pending) -> Result<String> {
        let keys = self.keys()?;
        let storage = self.state.storage();
        let mut entries = Vec::new();
        for (path, (id, _)) in pending {
            match id {
                Some(id) => {
                    let Some(json) = storage.get::<String>(&format!("t:{id}")).await.ok().flatten() else { continue };
                    let text = keys.seal(path, json.as_bytes());
                    entries.push(serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "content": text }));
                }
                None => entries.push(serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "sha": null })),
            }
        }
        let branch = self.branch()?;
        let mut last = String::new();
        for _ in 0..2 {
            let head = self.head().await?;
            let base = Self::sha(&self.ok(Method::Get, &format!("/git/commits/{head}"), None).await?, "/tree/sha");
            let (status, tree) =
                self.github(Method::Post, "/git/trees", Some(serde_json::json!({ "base_tree": base, "tree": entries }))).await?;
            if status == 422 {
                // A removal of a file the branch does not have: written without the removals.
                entries.retain(|e| e.get("sha").is_none());
                continue;
            }
            if !(200..300).contains(&status) {
                return Err(fail(format!("GitHub answered {status} to a tree: {}", tree.get("message").and_then(|m| m.as_str()).unwrap_or(""))));
            }
            let message = format!("{} file(s)", entries.len());
            let commit = self
                .ok(Method::Post, "/git/commits", Some(serde_json::json!({ "message": message, "tree": Self::sha(&tree, "/sha"), "parents": [head] })))
                .await?;
            let sha = Self::sha(&commit, "/sha");
            let (status, r) =
                self.github(Method::Patch, &format!("/git/refs/heads/{branch}"), Some(serde_json::json!({ "sha": sha, "force": false }))).await?;
            if (200..300).contains(&status) {
                return Ok(sha);
            }
            last = format!("GitHub answered {status} to moving the branch: {}", r.get("message").and_then(|m| m.as_str()).unwrap_or(""));
        }
        Err(fail(last))
    }

    /// The branch's head and every tour and version in it, opened: `(id, json)`. Read as one
    /// archive of that very commit - one request however many files, where a request a file
    /// would run into the Worker's limit of requests out (50 on the free plan).
    async fn from_branch(&self) -> Result<(Option<String>, Vec<(String, String)>)> {
        let branch = self.branch()?;
        let (status, r) = self.github(Method::Get, &format!("/git/ref/heads/{branch}"), None).await?;
        if status != 200 {
            return Ok((None, Vec::new()));
        }
        let head = Self::sha(&r, "/object/sha");
        let mut resp = self.github_raw(Method::Get, &format!("/repos/{}/tarball/{head}", self.repo()?), None).await?;
        if resp.status_code() != 200 {
            return Err(fail(format!("GitHub answered {} to the archive of {branch}", resp.status_code())));
        }
        let gz = resp.bytes().await?;
        let keys = self.keys()?;
        let mut out = Vec::new();
        let mut unreadable = 0;
        for (path, bytes) in untar(&gunzip(&gz)?) {
            if !(path.starts_with("t/") || path.starts_with("v/")) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes).into_owned();
            match keys.open(&path, &text).ok().and_then(|p| String::from_utf8(p).ok()) {
                Some(json) => match Tour::from_json(&json) {
                    Ok(t) => out.push((t.id.as_str().to_owned(), json)),
                    Err(_) => unreadable += 1,
                },
                None => unreadable += 1,
            }
        }
        if out.is_empty() && unreadable > 0 {
            return Err(fail(format!("STORE_KEY opens none of the {unreadable} file(s) in {branch}: not the key they were sealed with")));
        }
        Ok((Some(head), out))
    }
}

/// A gzip file's contents.
fn gunzip(gz: &[u8]) -> Result<Vec<u8>> {
    if gz.len() < 18 || gz[0] != 0x1f || gz[1] != 0x8b || gz[2] != 8 {
        return Err(fail("the archive is not gzip"));
    }
    let flags = gz[3];
    let mut at = 10;
    if flags & 4 != 0 {
        at += 2 + u16::from_le_bytes([gz[at], gz[at + 1]]) as usize;
    }
    for bit in [8u8, 16] {
        if flags & bit != 0 {
            while at < gz.len() && gz[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    miniz_oxide::inflate::decompress_to_vec(&gz[at..gz.len() - 8]).map_err(|_| fail("the archive does not inflate"))
}

/// The files in a tar archive, as `(path, contents)`, the first directory of each path (the
/// one GitHub names after the repository and the commit) left off.
fn untar(tar: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut at = 0;
    while at + 512 <= tar.len() {
        let head = &tar[at..at + 512];
        if head.iter().all(|b| *b == 0) {
            break;
        }
        let text = |r: std::ops::Range<usize>| String::from_utf8_lossy(&head[r]).trim_end_matches('\0').to_owned();
        let size = usize::from_str_radix(text(124..136).trim(), 8).unwrap_or(0);
        let kind = head[156];
        let prefix = text(345..500);
        let name = if prefix.is_empty() { text(0..100) } else { format!("{prefix}/{}", text(0..100)) };
        let body = at + 512;
        if (kind == b'0' || kind == 0) && body + size <= tar.len() {
            if let Some((_, path)) = name.split_once('/') {
                files.push((path.to_owned(), tar[body..body + size].to_vec()));
            }
        }
        at = body + size.div_ceil(512) * 512;
    }
    files
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
