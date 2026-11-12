//! The repository itself: what is in it, one file, and a batch of changes as one commit.
//!
//! A trait because the store is tested against [`Memory`] - a repository in a `HashMap` -
//! and only the real thing, [`GitHub`], needs a network and a token.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// The branch as it is: the commit it points at, and every file under it with git's id for
/// its contents.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub head: String,
    /// When the head commit was made, if the repository says - for the help page.
    pub head_at: Option<String>,
    pub files: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Put { path: String, text: String },
    Delete { path: String },
}

impl Change {
    pub fn path(&self) -> &str {
        match self {
            Change::Put { path, .. } | Change::Delete { path } => path,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum RemoteError {
    /// The branch is no longer where the commit was going to go: somebody else wrote.
    Moved,
    /// GitHub's rate limit: wait this long (if it said) before asking again. Asking on
    /// regardless can get the token banned.
    Limited(Option<Duration>),
    Failed(String),
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteError::Moved => write!(f, "the branch moved"),
            RemoteError::Limited(Some(d)) => write!(f, "GitHub's rate limit: wait {} s", d.as_secs()),
            RemoteError::Limited(None) => write!(f, "GitHub's rate limit"),
            RemoteError::Failed(why) => write!(f, "{why}"),
        }
    }
}

#[async_trait::async_trait]
pub trait Remote: Send + Sync {
    async fn snapshot(&self) -> Result<Snapshot, RemoteError>;
    /// A file's contents, by git's id for them.
    async fn read(&self, blob: &str) -> Result<String, RemoteError>;
    /// The changes as one commit on top of `parent`, and the branch moved to it - only if it
    /// is still at `parent`; otherwise [`RemoteError::Moved`] and nothing is changed.
    /// Answers the new commit.
    async fn commit(&self, parent: &str, changes: &[Change], message: &str) -> Result<String, RemoteError>;
    /// "owner/name, branch main" - for the status, and for the administrator only.
    fn describe(&self) -> String;
    /// Whether this is the repository inside the server (`GitHubRepo=memory`), not GitHub:
    /// the status says so to everyone, or "written" would read as "written to GitHub".
    fn emulated(&self) -> bool {
        false
    }
    /// Where a commit can be looked at, if anywhere.
    fn commit_url(&self, _commit: &str) -> Option<String> {
        None
    }
}

/// Shared: the tests open a second store on the same repository, as a restarted server would.
#[async_trait::async_trait]
impl<T: Remote> Remote for std::sync::Arc<T> {
    async fn snapshot(&self) -> Result<Snapshot, RemoteError> {
        (**self).snapshot().await
    }
    async fn read(&self, blob: &str) -> Result<String, RemoteError> {
        (**self).read(blob).await
    }
    async fn commit(&self, parent: &str, changes: &[Change], message: &str) -> Result<String, RemoteError> {
        (**self).commit(parent, changes, message).await
    }
    fn describe(&self) -> String {
        (**self).describe()
    }
    fn emulated(&self) -> bool {
        (**self).emulated()
    }
    fn commit_url(&self, commit: &str) -> Option<String> {
        (**self).commit_url(commit)
    }
}

/// A repository in memory, for the tests: it counts commits, can be moved under the
/// store's feet, and can be told to fail.
#[derive(Default)]
pub struct Memory {
    inner: Mutex<MemoryRepo>,
}

#[derive(Default)]
struct MemoryRepo {
    commits: u64,
    files: HashMap<String, String>,
    fail: Option<String>,
    limited: Option<Option<Duration>>,
    /// The next commit is made, but its answer is lost - a timeout after GitHub did it.
    lose_answer: bool,
    reads: u64,
}

impl Memory {
    /// One commit already there, as GitHub needs for its tree API: a repository made with a
    /// README.
    pub fn new() -> Memory {
        let m = Memory::default();
        {
            let mut r = m.inner.lock().unwrap();
            r.commits = 1;
            r.files.insert("README.md".into(), "data\n".into());
        }
        m
    }

    pub fn head(&self) -> String {
        format!("c{}", self.inner.lock().unwrap().commits)
    }

    pub fn commits(&self) -> u64 {
        self.inner.lock().unwrap().commits
    }

    pub fn reads(&self) -> u64 {
        self.inner.lock().unwrap().reads
    }

    pub fn files(&self) -> HashMap<String, String> {
        self.inner.lock().unwrap().files.clone()
    }

    /// Somebody else commits: the branch moves on.
    pub fn move_on(&self, path: &str, text: &str) {
        let mut r = self.inner.lock().unwrap();
        r.commits += 1;
        r.files.insert(path.into(), text.into());
    }

    pub fn fail_with(&self, why: Option<&str>) {
        self.inner.lock().unwrap().fail = why.map(str::to_owned);
    }

    /// Commits are refused as over the rate limit.
    pub fn limit(&self, wait: Option<Option<Duration>>) {
        self.inner.lock().unwrap().limited = wait;
    }

    pub fn lose_next_answer(&self) {
        self.inner.lock().unwrap().lose_answer = true;
    }

    /// Somebody deletes a file by hand: the branch moves on without it.
    pub fn delete_by_hand(&self, path: &str) {
        let mut r = self.inner.lock().unwrap();
        r.commits += 1;
        r.files.remove(path);
    }
}

#[async_trait::async_trait]
impl Remote for Memory {
    async fn snapshot(&self) -> Result<Snapshot, RemoteError> {
        let r = self.inner.lock().unwrap();
        if let Some(why) = &r.fail {
            return Err(RemoteError::Failed(why.clone()));
        }
        Ok(Snapshot {
            head: format!("c{}", r.commits),
            head_at: None,
            files: r.files.iter().map(|(p, t)| (p.clone(), super::seal::blob_sha(t.as_bytes()))).collect(),
        })
    }

    async fn read(&self, blob: &str) -> Result<String, RemoteError> {
        let mut r = self.inner.lock().unwrap();
        r.reads += 1;
        r.files
            .values()
            .find(|t| super::seal::blob_sha(t.as_bytes()) == blob)
            .cloned()
            .ok_or_else(|| RemoteError::Failed(format!("no blob {blob}")))
    }

    async fn commit(&self, parent: &str, changes: &[Change], _message: &str) -> Result<String, RemoteError> {
        let mut r = self.inner.lock().unwrap();
        if let Some(why) = &r.fail {
            return Err(RemoteError::Failed(why.clone()));
        }
        if let Some(wait) = r.limited {
            return Err(RemoteError::Limited(wait));
        }
        if parent != format!("c{}", r.commits) {
            return Err(RemoteError::Moved);
        }
        // Stricter than GitHub may be: a file that is not there cannot be deleted. The store
        // must not ask - one change refused would hold back every change after it.
        if let Some(gone) = changes.iter().find(|c| matches!(c, Change::Delete { path } if !r.files.contains_key(path))) {
            return Err(RemoteError::Failed(format!("no file {} to delete", gone.path())));
        }
        for c in changes {
            match c {
                Change::Put { path, text } => {
                    r.files.insert(path.clone(), text.clone());
                }
                Change::Delete { path } => {
                    r.files.remove(path);
                }
            }
        }
        r.commits += 1;
        if std::mem::take(&mut r.lose_answer) {
            return Err(RemoteError::Failed("timed out".into()));
        }
        Ok(format!("c{}", r.commits))
    }

    fn describe(&self) -> String {
        "memory".into()
    }

    fn emulated(&self) -> bool {
        true
    }
}

/// GitHub's REST API - the Git Data part of it, which writes a batch of files as one commit
/// in four requests however many files there are.
#[cfg(feature = "github")]
pub struct GitHub {
    http: reqwest::Client,
    /// `https://api.github.com/repos/owner/name`
    base: String,
    repo: String,
    branch: String,
    token: String,
}

#[cfg(feature = "github")]
impl GitHub {
    pub fn new(repo: &str, branch: &str, token: &str) -> Result<GitHub, String> {
        if repo.split('/').count() != 2 || repo.split('/').any(str::is_empty) {
            return Err(format!("GitHubRepo is {repo:?}; it must be owner/name"));
        }
        if token.trim().is_empty() {
            return Err("GitHubToken is not set".into());
        }
        let http = reqwest::Client::builder()
            .user_agent("tourcalc")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(GitHub {
            http,
            base: format!("https://api.github.com/repos/{repo}"),
            repo: repo.to_owned(),
            branch: branch.to_owned(),
            token: token.trim().to_owned(),
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// The answer as JSON, or what went wrong said with the status - and never the token.
    async fn send(&self, rb: reqwest::RequestBuilder, body: Option<serde_json::Value>) -> Result<(u16, serde_json::Value), RemoteError> {
        let rb = match body {
            Some(b) => rb.header("Content-Type", "application/json").body(b.to_string()),
            None => rb,
        };
        let resp = rb.send().await.map_err(|e| RemoteError::Failed(format!("GitHub: {}", e.without_url())))?;
        let status = resp.status().as_u16();
        let header = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok());
        let (retry_after, remaining, reset) = (header("retry-after"), header("x-ratelimit-remaining"), header("x-ratelimit-reset"));
        let text = resp.text().await.unwrap_or_default();
        let json = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
        if let Some(wait) = rate_limited(status, retry_after, remaining, reset, message, now_secs()) {
            return Err(RemoteError::Limited(wait));
        }
        Ok((status, json))
    }

    fn failed(what: &str, status: u16, json: &serde_json::Value) -> RemoteError {
        let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
        RemoteError::Failed(format!("GitHub answered {status} to {what}: {message}"))
    }

    /// A commit's tree, and when it was made.
    async fn tree_of(&self, commit: &str) -> Result<(String, Option<String>), RemoteError> {
        let (status, json) = self.send(self.request(reqwest::Method::GET, &format!("/git/commits/{commit}")), None).await?;
        if status != 200 {
            return Err(Self::failed("reading a commit", status, &json));
        }
        let tree = json
            .pointer("/tree/sha")
            .and_then(|s| s.as_str())
            .map(str::to_owned)
            .ok_or_else(|| RemoteError::Failed("GitHub: a commit without a tree".into()))?;
        Ok((tree, json.pointer("/committer/date").and_then(|s| s.as_str()).map(str::to_owned)))
    }
}

/// Whether an answer is GitHub's rate limit, and how long it says to wait - as its
/// documentation has it: `retry-after` if there is one, else the reset time when nothing is
/// left; a limit that says neither is waited out by the caller (a minute at least).
#[cfg_attr(not(feature = "github"), allow(dead_code))]
fn rate_limited(status: u16, retry_after: Option<u64>, remaining: Option<u64>, reset: Option<u64>, message: &str, now: u64) -> Option<Option<Duration>> {
    let limit = status == 429
        || (status == 403
            && (retry_after.is_some() || remaining == Some(0) || message.to_ascii_lowercase().contains("rate limit")));
    if !limit {
        return None;
    }
    Some(match (retry_after, remaining, reset) {
        (Some(s), _, _) => Some(Duration::from_secs(s)),
        (None, Some(0), Some(at)) => Some(Duration::from_secs(at.saturating_sub(now))),
        _ => None,
    })
}

#[cfg(feature = "github")]
fn now_secs() -> u64 {
    crate::clock::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(feature = "github")]
#[async_trait::async_trait]
impl Remote for GitHub {
    async fn snapshot(&self) -> Result<Snapshot, RemoteError> {
        let (status, json) = self
            .send(self.request(reqwest::Method::GET, &format!("/git/ref/heads/{}", self.branch)), None)
            .await?;
        if status == 404 || status == 409 {
            return Err(RemoteError::Failed(format!(
                "GitHub: no branch {} in {} - the repository needs one commit to start from \
                 (make it with a README)",
                self.branch, self.repo
            )));
        }
        if status != 200 {
            return Err(Self::failed("reading the branch", status, &json));
        }
        let head = json
            .pointer("/object/sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| RemoteError::Failed("GitHub: a branch without a commit".into()))?
            .to_owned();
        let (tree, head_at) = self.tree_of(&head).await?;
        let (status, json) = self
            .send(self.request(reqwest::Method::GET, &format!("/git/trees/{tree}?recursive=1")), None)
            .await?;
        if status != 200 {
            return Err(Self::failed("reading the tree", status, &json));
        }
        if json.get("truncated").and_then(|t| t.as_bool()) == Some(true) {
            return Err(RemoteError::Failed("GitHub: the tree is too big to list in one answer".into()));
        }
        let files = json
            .get("tree")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("blob"))
            .filter_map(|e| Some((e.get("path")?.as_str()?.to_owned(), e.get("sha")?.as_str()?.to_owned())))
            .collect();
        Ok(Snapshot { head, head_at, files })
    }

    async fn read(&self, blob: &str) -> Result<String, RemoteError> {
        let (status, json) = self.send(self.request(reqwest::Method::GET, &format!("/git/blobs/{blob}")), None).await?;
        if status != 200 {
            return Err(Self::failed("reading a file", status, &json));
        }
        let content: String = json
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(content)
            .map_err(|_| RemoteError::Failed("GitHub: a file that is not base64".into()))?;
        String::from_utf8(bytes).map_err(|_| RemoteError::Failed("GitHub: a file that is not text".into()))
    }

    async fn commit(&self, parent: &str, changes: &[Change], message: &str) -> Result<String, RemoteError> {
        let (base_tree, _) = self.tree_of(parent).await?;
        let entries: Vec<serde_json::Value> = changes
            .iter()
            .map(|c| match c {
                Change::Put { path, text } => serde_json::json!({
                    "path": path, "mode": "100644", "type": "blob", "content": text
                }),
                Change::Delete { path } => serde_json::json!({
                    "path": path, "mode": "100644", "type": "blob", "sha": null
                }),
            })
            .collect();
        let (status, json) = self
            .send(
                self.request(reqwest::Method::POST, "/git/trees"),
                Some(serde_json::json!({ "base_tree": base_tree, "tree": entries })),
            )
            .await?;
        if status != 201 {
            return Err(Self::failed("writing the tree", status, &json));
        }
        let tree = json.get("sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        let (status, json) = self
            .send(
                self.request(reqwest::Method::POST, "/git/commits"),
                Some(serde_json::json!({ "message": message, "tree": tree, "parents": [parent] })),
            )
            .await?;
        if status != 201 {
            return Err(Self::failed("writing the commit", status, &json));
        }
        let commit = json.get("sha").and_then(|s| s.as_str()).unwrap_or_default().to_owned();
        // Not forced: if the branch moved since `parent`, this is not a fast-forward and
        // GitHub refuses it - which is the check that nothing somebody else wrote is lost.
        let (status, json) = self
            .send(
                self.request(reqwest::Method::PATCH, &format!("/git/refs/heads/{}", self.branch)),
                Some(serde_json::json!({ "sha": commit, "force": false })),
            )
            .await?;
        match status {
            200 => Ok(commit),
            409 | 422 => Err(RemoteError::Moved),
            _ => Err(Self::failed("moving the branch", status, &json)),
        }
    }

    fn describe(&self) -> String {
        format!("{}, branch {}", self.repo, self.branch)
    }

    fn commit_url(&self, commit: &str) -> Option<String> {
        Some(format!("https://github.com/{}/commit/{commit}", self.repo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_memory_repository_refuses_a_commit_on_a_moved_branch() {
        let m = Memory::new();
        let head = m.snapshot().await.unwrap().head;
        let put = Change::Put { path: "t/a".into(), text: "x\n".into() };
        let next = m.commit(&head, std::slice::from_ref(&put), "one").await.unwrap();
        assert_eq!(m.commit(&head, &[put], "again").await, Err(RemoteError::Moved));
        let snap = m.snapshot().await.unwrap();
        assert_eq!(snap.head, next);
        assert_eq!(m.read(&snap.files["t/a"]).await.unwrap(), "x\n");
    }

    #[test]
    fn a_rate_limit_is_told_from_a_refusal() {
        let s = |n| Some(Duration::from_secs(n));
        // Not a limit: no permission is a 403 too.
        assert_eq!(rate_limited(403, None, Some(4000), None, "Resource not accessible by personal access token", 0), None);
        assert_eq!(rate_limited(422, None, None, None, "", 0), None);
        assert_eq!(rate_limited(403, Some(60), None, None, "", 0), Some(s(60)));
        assert_eq!(rate_limited(429, None, None, None, "", 0), Some(None));
        assert_eq!(rate_limited(403, None, Some(0), Some(1_000), "API rate limit exceeded", 900), Some(s(100)));
        assert_eq!(rate_limited(403, None, None, None, "You have exceeded a secondary rate limit", 0), Some(None));
    }
}
