//! The tourcalc server as a Cloudflare Worker - a prototype.
//!
//! What it answers to find out whether the move off the droplet holds:
//! - `GET /api/Info/version` - the Worker alone, nothing behind it;
//! - `GET /api/Tour/:id` - the tour from the GitHub store, through a Durable Object that keeps
//!   the branch's listing and the files read, as the server keeps them in memory and on disk;
//! - `PUT /api/Tour/:id` - a tour stored: sealed and kept in the object at once, written to
//!   GitHub by the object's alarm 30 s later, all waiting changes in one commit - as the
//!   server does it;
//! - `GET /calc/:id` - the tour with its balances and settlement worked out in the object
//!   (30 s of CPU per request); `GET /calc-here/:id` - in the Worker (10 ms on the free plan);
//! - `GET /push-test?n=N` - N push notifications signed and encrypted as the server does it
//!   (`web-push-native`), sent to a push service that turns them away;
//! - `GET /status` - what waits, the last commit, the last error.
//!
//! It writes to its own branch (`GITHUB_BRANCH`, made from `master` when missing), never to
//! the one the beta writes to. The answers' `X-Tc-*` headers say what each cost in GitHub
//! requests; the CPU time is in the Workers logs (`wrangler tail`).

use std::cell::RefCell;
use std::collections::HashMap;

use worker::wasm_bindgen::JsValue;
use worker::*;

// The store's sealing as the server has it - the same file, not a copy.
#[path = "../../../rust/crates/tc-server/src/github/seal.rs"]
#[allow(dead_code)]
mod seal;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/api/Info/version", |_, _| {
            Response::from_json(&serde_json::json!({ "commit": "cf-proto", "kind": "cloudflare-worker" }))
        })
        .get_async("/api/Tour/:id", |req, ctx| async move { to_store(req, &ctx.env).await })
        .put_async("/api/Tour/:id", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/calc/:id", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/push-test", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/status", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/calc-here/:id", |_req, ctx| async move {
            let id = ctx.param("id").cloned().unwrap_or_default();
            let url = format!("https://store/api/Tour/{id}");
            let mut answer = to_store(Request::new(&url, Method::Get)?, &ctx.env).await?;
            if answer.status_code() != 200 {
                return Ok(answer);
            }
            let text = answer.text().await?;
            let mut out = Response::from_json(&work_out(&text)?)?;
            out.headers_mut().set("x-tc-worked-out", "worker")?;
            Ok(out)
        })
        .run(req, env)
        .await
}

/// One object for the whole store: it is the server's memory, and the one writer.
async fn to_store(req: Request, env: &Env) -> Result<Response> {
    let stub = env.durable_object("STORE")?.id_from_name("store")?.get_stub()?;
    stub.fetch_with_request(req).await
}

fn fail(e: impl std::fmt::Display) -> Error {
    Error::RustError(e.to_string())
}

/// The balances and the settlement, as the Balance tab shows them - the heaviest thing a
/// request does today.
fn work_out(json: &str) -> Result<serde_json::Value> {
    let tour = tc_core::domain::Tour::from_json(json).map_err(fail)?;
    let balances = tc_core::calc::calculate(&tour, tc_core::calc::Options { with_planned: false });
    let settlement = tc_core::calc::suggest_settlement(&tour).map_err(|e| fail(format!("{e:?}")))?;
    Ok(serde_json::json!({
        "name": tour.name,
        "people": balances.per_person.len(),
        "spendings": tour.spendings.len(),
        "debt": format!("{:?}", balances.total_abs_debt()),
        "transfers": settlement.len(),
        "jsonBytes": json.len(),
    }))
}

/// The branch as last read or written: path -> blob sha.
struct Listing {
    head: String,
    files: HashMap<String, String>,
    read_at: f64,
}

#[durable_object]
pub struct Store {
    state: State,
    env: Env,
    listing: RefCell<Option<Listing>>,
}

/// How long a listing read from GitHub is taken as the branch. The server reads it once and
/// then knows every change, being the only writer; this one also looks again now and then.
const LISTING_FOR_MS: f64 = 600_000.0;
/// Changes wait this long and go in one commit - the server's `GitHubFlushSeconds`.
const FLUSH_AFTER: std::time::Duration = std::time::Duration::from_secs(30);
/// What waits for the next commit, path -> sealed text: one value, read and written whole.
const PENDING: &str = "pending";

impl DurableObject for Store {
    fn new(state: State, env: Env) -> Self {
        Store { state, env, listing: RefCell::new(None) }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let path = req.path();
        let mut asked = 0u32;
        if path == "/status" {
            return self.status().await;
        }
        if path == "/push-test" {
            let n = req.url()?.query_pairs().find(|(k, _)| k == "n").and_then(|(_, v)| v.parse().ok()).unwrap_or(1);
            return push_test(n).await;
        }
        let keys = self.keys()?;
        let (calc, id) = match path.strip_prefix("/api/Tour/") {
            Some(id) => (false, id.to_owned()),
            None => match path.strip_prefix("/calc/") {
                Some(id) => (true, id.to_owned()),
                None => return Response::error("not here", 404),
            },
        };
        let file = keys.tour_path(&id);

        if req.method() == Method::Put {
            let json = req.text().await?;
            // What the server checks before it keeps anything: that it is a tour, and this one.
            let tour = tc_core::domain::Tour::from_json(&json).map_err(fail)?;
            if tour.id.as_str() != id {
                return Response::error("the id in the body is another tour's", 400);
            }
            let sealed = keys.seal(&file, json.as_bytes());
            let mut pending = self.pending().await;
            pending.insert(file, sealed);
            self.state.storage().put(PENDING, &pending).await?;
            if self.state.storage().get_alarm().await?.is_none() {
                self.state.storage().set_alarm(FLUSH_AFTER).await?;
            }
            let mut r = Response::from_json(&serde_json::json!({ "stored": id, "pending": pending.len() }))?;
            r.headers_mut().set("x-tc-github-requests", "0")?;
            return Ok(r);
        }

        // A change not written yet is the tour as it is.
        let pending = self.pending().await;
        let (text, from) = match pending.get(&file) {
            Some(t) => (t.clone(), "pending"),
            None => {
                self.fresh(&mut asked).await?;
                let sha = self.listing.borrow().as_ref().and_then(|l| l.files.get(&file).cloned());
                let Some(sha) = sha else {
                    return Response::error("no such tour", 404);
                };
                let (text, kept) = self.blob(&sha, &mut asked).await?;
                (text, if kept { "storage" } else { "github" })
            }
        };
        let plain = keys.open(&file, &text).map_err(|e| fail(e.0))?;
        let json = String::from_utf8(plain).map_err(fail)?;
        let mut answer = if calc {
            Response::from_json(&work_out(&json)?)?
        } else {
            let mut r = Response::ok(json)?;
            r.headers_mut().set("content-type", "application/json")?;
            r
        };
        let h = answer.headers_mut();
        h.set("x-tc-github-requests", &asked.to_string())?;
        h.set("x-tc-from", from)?;
        if calc {
            h.set("x-tc-worked-out", "durable-object")?;
        }
        Ok(answer)
    }

    /// The commit: everything waiting, in one, on top of the branch's head.
    async fn alarm(&self) -> Result<Response> {
        let pending = self.pending().await;
        if pending.is_empty() {
            return Response::ok("nothing waits");
        }
        let mut asked = 0u32;
        let outcome = self.commit(&pending, &mut asked).await;
        let storage = self.state.storage();
        match outcome {
            Ok(head) => {
                // Only what was written: a change made while the commit was on its way waits on.
                let mut now = self.pending().await;
                now.retain(|path, text| pending.get(path) != Some(text));
                storage.put(PENDING, &now).await?;
                if !now.is_empty() {
                    storage.set_alarm(FLUSH_AFTER).await?;
                }
                let note = serde_json::json!({
                    "head": head, "files": pending.len(), "githubRequests": asked,
                    "at": Date::now().to_string(),
                });
                storage.put("last-commit", &note).await?;
                storage.delete("last-error").await?;
            }
            Err(e) => {
                storage.put("last-error", &e.to_string()).await?;
                storage.set_alarm(FLUSH_AFTER).await?;
            }
        }
        Response::ok("done")
    }
}

impl Store {
    fn keys(&self) -> Result<seal::Keys> {
        let key = self.env.secret("STORE_KEY")?.to_string();
        seal::Keys::from_setting(&key).map_err(|e| fail(e.0))
    }

    async fn pending(&self) -> HashMap<String, String> {
        self.state.storage().get::<HashMap<String, String>>(PENDING).await.ok().flatten().unwrap_or_default()
    }

    async fn status(&self) -> Result<Response> {
        let storage = self.state.storage();
        let last: Option<serde_json::Value> = storage.get("last-commit").await.ok().flatten();
        let error: Option<String> = storage.get("last-error").await.ok().flatten();
        let alarm = storage.get_alarm().await?;
        Response::from_json(&serde_json::json!({
            "pending": self.pending().await.len(),
            "alarmAt": alarm,
            "lastCommit": last,
            "lastError": error,
            "listed": self.listing.borrow().as_ref().map(|l| l.files.len()),
        }))
    }

    async fn github(&self, method: Method, path: &str, body: Option<serde_json::Value>, asked: &mut u32) -> Result<(u16, serde_json::Value)> {
        let repo = self.env.var("GITHUB_REPO")?.to_string();
        let token = self.env.secret("GITHUB_TOKEN")?.to_string();
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
        let url = format!("https://api.github.com/repos/{repo}{path}");
        let mut resp = Fetch::Request(Request::new_with_init(&url, &init)?).send().await?;
        *asked += 1;
        let status = resp.status_code();
        let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
        Ok((status, json))
    }

    /// A 2xx answer, or GitHub's status and message - never the token.
    async fn ok(&self, method: Method, path: &str, body: Option<serde_json::Value>, asked: &mut u32) -> Result<serde_json::Value> {
        let (status, json) = self.github(method, path, body, asked).await?;
        if !(200..300).contains(&status) {
            let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
            return Err(fail(format!("GitHub answered {status} to {path}: {message}")));
        }
        Ok(json)
    }

    /// The branch's head; the branch made from `master` if it is not there yet.
    async fn head(&self, asked: &mut u32) -> Result<String> {
        let branch = self.env.var("GITHUB_BRANCH")?.to_string();
        let (status, r) = self.github(Method::Get, &format!("/git/ref/heads/{branch}"), None, asked).await?;
        if status == 200 {
            return Ok(r.pointer("/object/sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned());
        }
        let m = self.ok(Method::Get, "/git/ref/heads/master", None, asked).await?;
        let sha = m.pointer("/object/sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        self.ok(Method::Post, "/git/refs", Some(serde_json::json!({ "ref": format!("refs/heads/{branch}"), "sha": sha })), asked)
            .await?;
        Ok(sha)
    }

    async fn tree_of(&self, commit: &str, asked: &mut u32) -> Result<String> {
        let c = self.ok(Method::Get, &format!("/git/commits/{commit}"), None, asked).await?;
        Ok(c.pointer("/tree/sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned())
    }

    /// The listing, read again when it is old.
    async fn fresh(&self, asked: &mut u32) -> Result<()> {
        let stale = match &*self.listing.borrow() {
            Some(l) => Date::now().as_millis() as f64 - l.read_at > LISTING_FOR_MS,
            None => true,
        };
        if stale {
            let head = self.head(asked).await?;
            let tree = self.tree_of(&head, asked).await?;
            let t = self.ok(Method::Get, &format!("/git/trees/{tree}?recursive=1"), None, asked).await?;
            let files = t
                .get("tree")
                .and_then(|t| t.as_array())
                .into_iter()
                .flatten()
                .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("blob"))
                .filter_map(|e| Some((e.get("path")?.as_str()?.to_owned(), e.get("sha")?.as_str()?.to_owned())))
                .collect();
            *self.listing.borrow_mut() = Some(Listing { head, files, read_at: Date::now().as_millis() as f64 });
        }
        Ok(())
    }

    /// Everything waiting as one commit: the head, its tree, a tree on top of it with the
    /// files in it, the commit, the branch moved - five requests however many files.
    async fn commit(&self, pending: &HashMap<String, String>, asked: &mut u32) -> Result<String> {
        let head = self.head(asked).await?;
        let base = self.tree_of(&head, asked).await?;
        let entries: Vec<serde_json::Value> = pending
            .iter()
            .map(|(path, text)| serde_json::json!({ "path": path, "mode": "100644", "type": "blob", "content": text }))
            .collect();
        let tree = self.ok(Method::Post, "/git/trees", Some(serde_json::json!({ "base_tree": base, "tree": entries })), asked).await?;
        let tree = tree.get("sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        let message = format!("{} file(s) from the Cloudflare prototype", pending.len());
        let c = self
            .ok(Method::Post, "/git/commits", Some(serde_json::json!({ "message": message, "tree": tree, "parents": [head] })), asked)
            .await?;
        let commit = c.get("sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        let branch = self.env.var("GITHUB_BRANCH")?.to_string();
        self.ok(Method::Patch, &format!("/git/refs/heads/{branch}"), Some(serde_json::json!({ "sha": commit, "force": false })), asked)
            .await?;
        // The listing follows without asking GitHub, and the files written are kept by sha.
        let storage = self.state.storage();
        for (path, text) in pending {
            let sha = seal::blob_sha(text.as_bytes());
            storage.put(&format!("blob:{sha}"), text).await?;
            if let Some(l) = self.listing.borrow_mut().as_mut() {
                l.files.insert(path.clone(), sha);
                l.head = commit.clone();
            }
        }
        Ok(commit)
    }

    /// A file by its blob sha: from the object's storage if read before, else from GitHub and
    /// kept - the server's disk cache. The sha names the contents, so a kept one is never stale.
    async fn blob(&self, sha: &str, asked: &mut u32) -> Result<(String, bool)> {
        let storage = self.state.storage();
        let key = format!("blob:{sha}");
        if let Ok(Some(text)) = storage.get::<String>(&key).await {
            return Ok((text, true));
        }
        let b = self.ok(Method::Get, &format!("/git/blobs/{sha}"), None, asked).await?;
        let content: String =
            b.get("content").and_then(|c| c.as_str()).unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(content).map_err(|_| fail("GitHub: not base64"))?;
        let text = String::from_utf8(bytes).map_err(|_| fail("GitHub: not text"))?;
        storage.put(&key, &text).await?;
        Ok((text, false))
    }
}

/// N notifications made as the server makes them - VAPID-signed, encrypted for the browser's
/// key - and sent to a push service, which turns them away: no subscription is real here.
/// A key pair of its own each time, never the server's.
async fn push_test(n: usize) -> Result<Response> {
    use p256::elliptic_curve::rand_core::OsRng;
    use web_push_native::jwt_simple::algorithms::ES256KeyPair;
    use web_push_native::{Auth, WebPushBuilder};

    let vapid = ES256KeyPair::generate();
    let browser = p256::SecretKey::random(&mut OsRng).public_key();
    let auth = Auth::clone_from_slice(&[7u8; 16]);
    let payload = serde_json::json!({ "title": "Викендица", "body": "Дима: Хлеб, 450 RSD", "tag": "tour" }).to_string();
    let mut prepared = Vec::with_capacity(n);
    for i in 0..n {
        let endpoint = format!("https://fcm.googleapis.com/fcm/send/tc-proto-not-a-subscription-{i}");
        let builder = WebPushBuilder::new(endpoint.parse().map_err(fail)?, browser, auth).with_vapid(&vapid, "mailto:push@example.org");
        prepared.push(builder.build(payload.as_bytes().to_vec()).map_err(fail)?);
    }
    let mut answers: HashMap<u16, usize> = HashMap::new();
    for request in prepared {
        let (parts, body) = request.into_parts();
        let headers = Headers::new();
        for (name, value) in parts.headers.iter() {
            headers.set(name.as_str(), value.to_str().unwrap_or_default())?;
        }
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(js_bytes(&body)));
        let status = match Fetch::Request(Request::new_with_init(&parts.uri.to_string(), &init)?).send().await {
            Ok(r) => r.status_code(),
            Err(_) => 0,
        };
        *answers.entry(status).or_default() += 1;
    }
    Response::from_json(&serde_json::json!({ "made": n, "answers": answers }))
}

fn js_bytes(bytes: &[u8]) -> JsValue {
    worker::js_sys::Uint8Array::from(bytes).into()
}
