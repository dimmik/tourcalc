//! The tourcalc server as a Cloudflare Worker - a prototype, reading only.
//!
//! What it answers to find out whether the move off the droplet holds:
//! - `GET /api/Info/version` - the Worker alone, nothing behind it;
//! - `GET /api/Tour/:id` - the tour from the GitHub store (`dimmik/tc-strg-test`), through a
//!   Durable Object that keeps the branch's listing and the files read, as the server keeps
//!   them in memory and on disk;
//! - `GET /calc/:id` - the same tour with its balances and the settlement worked out in the
//!   Durable Object (30 s of CPU per request);
//! - `GET /calc-here/:id` - worked out in the Worker itself (10 ms of CPU on the free plan).
//!
//! The answer's `X-Tc-*` headers say what it cost: GitHub requests made, files from the
//! object's own storage. The CPU time is in the Workers logs (`wrangler tail`).

use std::cell::RefCell;
use std::collections::HashMap;

use worker::*;

// The store's sealing as the server has it - the same file, not a copy.
#[path = "../../../rust/crates/tc-server/src/github/seal.rs"]
#[allow(dead_code)]
mod seal;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/api/Info/version", |_, _| {
            Response::from_json(&serde_json::json!({
                "commit": "cf-proto",
                "kind": "cloudflare-worker",
            }))
        })
        .get_async("/api/Tour/:id", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/calc/:id", |req, ctx| async move { to_store(req, &ctx.env).await })
        .get_async("/calc-here/:id", |_req, ctx| async move {
            let id = ctx.param("id").cloned().unwrap_or_default();
            let url = format!("https://store/api/Tour/{id}");
            let mut answer = to_store(Request::new(&url, Method::Get)?, &ctx.env).await?;
            if answer.status_code() != 200 {
                return Ok(answer);
            }
            let text = answer.text().await?;
            let mut out = Response::from_json(&work_out(&text)?)?;
            for (k, v) in answer.headers() {
                if k.starts_with("x-tc-") {
                    out.headers_mut().set(&k, &v)?;
                }
            }
            out.headers_mut().set("x-tc-worked-out", "worker")?;
            Ok(out)
        })
        .run(req, env)
        .await
}

/// One object for the whole store: it is the server's memory, and one writer.
async fn to_store(req: Request, env: &Env) -> Result<Response> {
    let stub = env.durable_object("STORE")?.id_from_name("store")?.get_stub()?;
    stub.fetch_with_request(req).await
}

/// The balances and the settlement, as the Balance tab shows them - the heaviest thing a
/// request does today.
fn work_out(json: &str) -> Result<serde_json::Value> {
    let tour = tc_core::domain::Tour::from_json(json).map_err(|e| Error::RustError(e.to_string()))?;
    let balances = tc_core::calc::calculate(&tour, tc_core::calc::Options { with_planned: false });
    let settlement = tc_core::calc::suggest_settlement(&tour).map_err(|e| Error::RustError(format!("{e:?}")))?;
    Ok(serde_json::json!({
        "name": tour.name,
        "people": balances.per_person.len(),
        "debt": format!("{:?}", balances.total_abs_debt()),
        "transfers": settlement.len(),
    }))
}

/// The branch as last read: path -> blob sha.
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

/// How long a listing is taken as the branch: the server reads it once at the start and then
/// knows every change, being the only writer; read-only, this one looks again now and then.
const LISTING_FOR_MS: f64 = 60_000.0;

impl DurableObject for Store {
    fn new(state: State, env: Env) -> Self {
        Store { state, env, listing: RefCell::new(None) }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();
        let (calc, id) = match path.strip_prefix("/api/Tour/") {
            Some(id) => (false, id.to_owned()),
            None => match path.strip_prefix("/calc/") {
                Some(id) => (true, id.to_owned()),
                None => return Response::error("not here", 404),
            },
        };
        let key = self.env.secret("STORE_KEY")?.to_string();
        let keys = seal::Keys::from_setting(&key).map_err(|e| Error::RustError(e.0))?;
        let mut asked = 0u32;
        if self.stale() {
            *self.listing.borrow_mut() = Some(self.list(&mut asked).await?);
        }
        let file = keys.tour_path(&id);
        let (sha, head) = {
            let l = self.listing.borrow();
            let l = l.as_ref().expect("just read");
            (l.files.get(&file).cloned(), l.head.clone())
        };
        let Some(sha) = sha else {
            return Response::error("no such tour", 404);
        };
        let (text, kept) = self.blob(&sha, &mut asked).await?;
        let plain = keys.open(&file, &text).map_err(|e| Error::RustError(e.0))?;
        let json = String::from_utf8(plain).map_err(|e| Error::RustError(e.to_string()))?;
        let mut answer = if calc {
            Response::from_json(&work_out(&json)?)?
        } else {
            let mut r = Response::ok(json)?;
            r.headers_mut().set("content-type", "application/json")?;
            r
        };
        let h = answer.headers_mut();
        h.set("x-tc-github-requests", &asked.to_string())?;
        h.set("x-tc-from-storage", if kept { "1" } else { "0" })?;
        h.set("x-tc-head", &head[..7.min(head.len())])?;
        if calc {
            h.set("x-tc-worked-out", "durable-object")?;
        }
        Ok(answer)
    }
}

impl Store {
    fn stale(&self) -> bool {
        match &*self.listing.borrow() {
            Some(l) => Date::now().as_millis() as f64 - l.read_at > LISTING_FOR_MS,
            None => true,
        }
    }

    async fn github(&self, path: &str, asked: &mut u32) -> Result<serde_json::Value> {
        let repo = self.env.var("GITHUB_REPO")?.to_string();
        let token = self.env.secret("GITHUB_TOKEN")?.to_string();
        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", token.trim()))?;
        headers.set("Accept", "application/vnd.github+json")?;
        headers.set("X-GitHub-Api-Version", "2022-11-28")?;
        headers.set("User-Agent", "tourcalc")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Get).with_headers(headers);
        let url = format!("https://api.github.com/repos/{repo}{path}");
        let mut resp = Fetch::Request(Request::new_with_init(&url, &init)?).send().await?;
        *asked += 1;
        let status = resp.status_code();
        let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
        if status != 200 {
            // The status and GitHub's message - never the token.
            let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
            return Err(Error::RustError(format!("GitHub answered {status}: {message}")));
        }
        Ok(json)
    }

    /// The branch's files, in three requests: the ref, its commit, the tree.
    async fn list(&self, asked: &mut u32) -> Result<Listing> {
        let branch = self.env.var("GITHUB_BRANCH")?.to_string();
        let r = self.github(&format!("/git/ref/heads/{branch}"), asked).await?;
        let head = r.pointer("/object/sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        let c = self.github(&format!("/git/commits/{head}"), asked).await?;
        let tree = c.pointer("/tree/sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        let t = self.github(&format!("/git/trees/{tree}?recursive=1"), asked).await?;
        let files = t
            .get("tree")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("blob"))
            .filter_map(|e| Some((e.get("path")?.as_str()?.to_owned(), e.get("sha")?.as_str()?.to_owned())))
            .collect();
        Ok(Listing { head, files, read_at: Date::now().as_millis() as f64 })
    }

    /// A file by its blob sha: from the object's storage if read before, else from GitHub and
    /// kept - the server's disk cache. The sha names the contents, so a kept one is never stale.
    async fn blob(&self, sha: &str, asked: &mut u32) -> Result<(String, bool)> {
        let storage = self.state.storage();
        let key = format!("blob:{sha}");
        if let Ok(Some(text)) = storage.get::<String>(&key).await {
            return Ok((text, true));
        }
        let b = self.github(&format!("/git/blobs/{sha}"), asked).await?;
        let content: String = b
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(content)
            .map_err(|_| Error::RustError("GitHub: a file that is not base64".into()))?;
        let text = String::from_utf8(bytes).map_err(|_| Error::RustError("GitHub: a file that is not text".into()))?;
        storage.put(&key, &text).await?;
        Ok((text, false))
    }
}
