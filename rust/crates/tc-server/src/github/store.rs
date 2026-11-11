//! The store: memory first, a cache on disk under it, and the repository catching up.
//!
//! - **Reading** never waits for GitHub: a tour is in memory, or it is read from the cache
//!   on disk - the same sealed files the repository has - and kept in memory again.
//! - **Writing** changes memory and the cache at once and marks the file changed; every
//!   `flush_every` all changed files go to the repository as **one commit**. Stopping the
//!   server writes what is left; a crash loses at most that much.
//! - **Memory has a ceiling** (`memory_bytes`): a tour not touched for longest goes back to
//!   the disk when the hot ones weigh more. Versions are never held whole - only what their
//!   list shows (an id, a date, a comment) - because every save keeps one and they are most
//!   of what there is.
//! - **Starting** reads the branch's list of files and takes from GitHub only the ones the
//!   cache does not have as they are: git's id for a file is a hash of its contents, so a
//!   file on disk is checked without downloading it.
//!
//! Subscriptions are one more file, `s`, the whole list of them.

use super::remote::{Change, Remote, RemoteError};
use super::seal::{self, Keys};
use crate::fields;
use crate::store::{Stale, TourStore};
use crate::subscriptions::{Subscription, SubscriptionStore};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tc_core::{Tour, TourId};

pub struct Settings {
    pub flush_every: Duration,
    pub memory_bytes: usize,
    pub cache_dir: PathBuf,
}

/// Cheap to clone: every clone is the same store. The server holds one as its tours and one
/// as its subscriptions, and the writer in the background holds a third.
#[derive(Clone)]
pub struct GitHubStore {
    core: Arc<Core>,
}

struct Core {
    keys: Keys,
    remote: Box<dyn Remote>,
    settings: Settings,
    inner: Mutex<Inner>,
    /// One commit at a time: the timer's and the one at shutdown must not both take the
    /// same changes.
    writing: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Inner {
    /// Every tour and version there is: where its file is, and what lists and checks need.
    index: HashMap<TourId, Meta>,
    /// The order tours were met in - by when they were made, then as they come.
    order: Vec<TourId>,
    hot: HashMap<TourId, Hot>,
    hot_bytes: usize,
    tick: u64,
    /// Files changed and not yet in the repository: what is to be done, and the change's
    /// number, so that a change made while a commit is under way is not taken for written
    /// when it ends.
    changed: HashMap<String, (Pending, u64)>,
    change_no: u64,
    /// The branch as last seen: its commit and its files.
    head: String,
    in_repo: HashMap<String, String>,
    subscriptions: HashMap<String, Vec<Subscription>>,
    /// The repository's file of subscriptions does not open with this key: it is not
    /// written over - the subscriptions made meanwhile live in memory only.
    subscriptions_unreadable: bool,
    stats: Stats,
}

/// A changed file, until it is in the repository.
enum Pending {
    /// In the cache on disk, which is where the commit takes it from: memory holds only the
    /// name, so a long time without GitHub does not fill it.
    OnDisk,
    /// The cache could not take it (a full disk): kept here, so that it is not lost.
    Text(String),
    Deleted,
}

/// The least time between two commits. GitHub allows 500 requests that make something in an
/// hour (and 80 a minute), the web interface's included; a commit is three of them. Every
/// 30 s at most is 360 an hour - room left for somebody working in the repository by hand.
const MIN_GAP: Duration = Duration::from_secs(30);
/// The longest wait after failures in a row (they double from [`MIN_GAP`]).
const MAX_BACKOFF: Duration = Duration::from_secs(15 * 60);

struct Meta {
    path: String,
    /// The tour without what is heavy in it - people, spendings, currencies, the menu: what
    /// a list, a count and a versions screen read.
    stub: Arc<Tour>,
}

struct Hot {
    tour: Arc<Tour>,
    bytes: usize,
    used: u64,
}

#[derive(Default)]
struct Stats {
    started: String,
    files_at_start: usize,
    downloaded_at_start: usize,
    evictions: u64,
    last_written: Option<String>,
    last_commit: Option<String>,
    last_error: Option<(String, String)>,
    /// The writer's next tick.
    next_write: Option<SystemTime>,
    /// The last commit tried, done or not - for [`MIN_GAP`].
    last_attempt: Option<SystemTime>,
    /// Failures in a row, and no commit before this: they double, and a rate limit says how
    /// long itself.
    failures: u32,
    not_before: Option<SystemTime>,
}

/// What [`Inner::stub`] keeps of a tour's extras.
const STUB_FIELDS: [&str; 11] = [
    fields::STATE,
    fields::ACCESS_CODE,
    fields::IS_VERSION,
    fields::VERSION_FOR,
    fields::VERSIONED_AT,
    fields::VERSION_COMMENT,
    fields::CREATED_AT,
    fields::ARCHIVED,
    fields::FINALIZING,
    tc_core::extras::MENU_TEMPLATE,
    tc_core::extras::DURATION,
];

/// A parsed tour weighs about twice its JSON (measured on the seed's tours: 666 K for 327 K).
fn weight(json_len: usize) -> usize {
    json_len * 2
}

fn ours(path: &str) -> bool {
    path == seal::SUBSCRIPTIONS || path.starts_with("t/") || path.starts_with("v/")
}

impl GitHubStore {
    /// Reads the branch, brings the cache on disk up to it, and opens every file.
    pub async fn open(keys: Keys, remote: Box<dyn Remote>, settings: Settings) -> Result<GitHubStore, String> {
        let snap = remote.snapshot().await.map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&settings.cache_dir)
            .map_err(|e| format!("GitHubCacheDir {}: {e}", settings.cache_dir.display()))?;

        let mut texts: Vec<(String, String)> = Vec::new();
        let mut downloaded = 0;
        for (path, blob) in snap.files.iter().filter(|(p, _)| ours(p)) {
            let local = settings.cache_dir.join(path);
            let text = match std::fs::read_to_string(&local) {
                Ok(t) if seal::blob_sha(t.as_bytes()) == *blob => t,
                _ => {
                    let t = remote.read(blob).await.map_err(|e| e.to_string())?;
                    write_file(&local, &t)?;
                    downloaded += 1;
                    t
                }
            };
            texts.push((path.clone(), text));
        }
        // What the repository no longer has is gone here too: the repository is the truth.
        prune(&settings.cache_dir, &settings.cache_dir, &snap.files);

        let mut inner = Inner { head: snap.head.clone(), in_repo: snap.files.clone(), ..Inner::default() };
        let mut tours: Vec<(String, Tour, usize)> = Vec::new();
        let mut opened = 0;
        for (path, text) in &texts {
            let plain = match keys.open(path, text) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("GitHub store: {}", e.0);
                    if path == seal::SUBSCRIPTIONS {
                        inner.subscriptions_unreadable = true;
                    }
                    continue;
                }
            };
            opened += 1;
            if path == seal::SUBSCRIPTIONS {
                match serde_json::from_slice(&plain) {
                    Ok(subs) => inner.subscriptions = subs,
                    Err(e) => tracing::warn!("GitHub store: the subscriptions do not read: {e}"),
                }
                continue;
            }
            match std::str::from_utf8(&plain).map_err(|e| e.to_string()).and_then(|s| Tour::from_json(s).map_err(|e| e.to_string())) {
                Ok(tour) => tours.push((path.clone(), tour, plain.len())),
                Err(e) => tracing::warn!("GitHub store: {path} opens, but is not a tour: {e}"),
            }
        }
        // Not one file opens: that is the wrong key, not damaged files. Starting would show no
        // tours, and the first subscription would write over the file of them - lost for good.
        if opened == 0 && !texts.is_empty() {
            return Err(format!(
                "GitHubStoreKey opens none of the {} file(s) in the repository: it is not the key \
                 they were sealed with. Nothing was changed.",
                texts.len()
            ));
        }
        // The order a list comes back in, as the seed file has it: oldest first.
        tours.sort_by(|a, b| {
            (fields::str_of(&a.1, fields::CREATED_AT), a.1.id.as_str().to_owned())
                .cmp(&(fields::str_of(&b.1, fields::CREATED_AT), b.1.id.as_str().to_owned()))
        });
        let hot_cap = settings.memory_bytes;
        for (path, tour, len) in tours {
            let keep_hot = path.starts_with("t/") && inner.hot_bytes + weight(len) <= hot_cap;
            inner.know(path, tour, len, keep_hot);
        }
        inner.stats = Stats {
            started: fields::now_stamp(),
            files_at_start: texts.len(),
            downloaded_at_start: downloaded,
            ..Stats::default()
        };
        tracing::info!(
            "GitHub store: {} tour(s), {} version(s), {} file(s) - {} downloaded, the rest from the cache",
            inner.index.values().filter(|m| !fields::is_version(&m.stub)).count(),
            inner.index.values().filter(|m| fields::is_version(&m.stub)).count(),
            texts.len(),
            downloaded
        );
        Ok(GitHubStore {
            core: Arc::new(Core { keys, remote, settings, inner: Mutex::new(inner), writing: Default::default() }),
        })
    }

    /// The writer: every `flush_every`, whatever changed goes to the repository; and every
    /// ten minutes a line in the log about memory, to see whether the ceiling is right.
    pub fn start_writing(&self) -> tokio::task::JoinHandle<()> {
        let store = self.clone();
        tokio::spawn(async move {
            let every = store.core.settings.flush_every;
            let mut last_note = SystemTime::now();
            loop {
                store.lock().stats.next_write = Some(SystemTime::now() + every);
                tokio::time::sleep(every).await;
                if store.due(SystemTime::now()) {
                    let _ = store.flush().await;
                }
                if last_note.elapsed().unwrap_or_default() >= Duration::from_secs(600) {
                    last_note = SystemTime::now();
                    let i = store.lock();
                    tracing::info!(
                        "GitHub store: {} tour(s) in memory, {} K of {} K; {} sent back to disk since start",
                        i.hot.len(),
                        i.hot_bytes / 1024,
                        store.core.settings.memory_bytes / 1024,
                        i.stats.evictions
                    );
                }
            }
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        crate::lock::mutex(&self.core.inner)
    }

    /// Everything changed, to the repository, as one commit. If the branch moved meanwhile,
    /// it is read again and the commit made on top of it: a file is written whole, so the
    /// last one written is the one that stays.
    ///
    /// Whenever asked - the writer asks only when [`Self::due`] says so, the shutdown always.
    pub async fn flush(&self) -> Result<(), String> {
        let _one = self.core.writing.lock().await;
        if self.lock().changed.is_empty() {
            return Ok(());
        }
        // After a failure the branch is read again first. A commit whose answer was lost, or
        // a file deleted by hand, would otherwise be asked for again - and refused, with every
        // change after it, for good.
        let mut fresh = self.lock().stats.failures > 0;
        for _ in 0..3 {
            if fresh {
                match self.core.remote.snapshot().await {
                    Ok(snap) => {
                        let mut i = self.lock();
                        i.head = snap.head;
                        i.in_repo = snap.files;
                    }
                    Err(e) => return Err(self.failed(e)),
                }
            }
            let Some((changes, numbers)) = self.batch() else {
                // Nothing left that the branch does not have already.
                let mut i = self.lock();
                i.stats.failures = 0;
                i.stats.not_before = None;
                i.stats.last_error = None;
                return Ok(());
            };
            // Says how many, and nothing about what.
            let message = format!("{} file(s)", changes.len());
            let head = {
                let mut i = self.lock();
                i.stats.last_attempt = Some(SystemTime::now());
                i.head.clone()
            };
            match self.core.remote.commit(&head, &changes, &message).await {
                Ok(commit) => {
                    let mut i = self.lock();
                    i.head = commit.clone();
                    for c in &changes {
                        match c {
                            Change::Put { path, text } => {
                                i.in_repo.insert(path.clone(), seal::blob_sha(text.as_bytes()));
                            }
                            Change::Delete { path } => {
                                i.in_repo.remove(path);
                            }
                        }
                        if i.changed.get(c.path()).map(|(_, no)| *no) == numbers.get(c.path()).copied() {
                            i.changed.remove(c.path());
                        }
                    }
                    i.stats.last_written = Some(fields::now_stamp());
                    i.stats.last_commit = Some(commit);
                    i.stats.last_error = None;
                    i.stats.failures = 0;
                    i.stats.not_before = None;
                    return Ok(());
                }
                Err(RemoteError::Moved) => {
                    tracing::warn!("GitHub store: the branch moved; reading it again");
                    fresh = true;
                }
                Err(e) => return Err(self.failed(e)),
            }
        }
        Err(self.failed(RemoteError::Failed("the branch kept moving".into())))
    }

    /// What goes in the next commit: every changed file - its contents from the cache - and
    /// the change's number. A delete of a file the branch does not have is dropped here: made
    /// and deleted between two commits, or gone already.
    fn batch(&self) -> Option<(Vec<Change>, HashMap<String, u64>)> {
        let plan: Vec<(String, Option<Option<String>>, u64)> = {
            let mut i = self.lock();
            let gone: Vec<String> = i
                .changed
                .iter()
                .filter(|(p, (w, _))| matches!(w, Pending::Deleted) && !i.in_repo.contains_key(*p))
                .map(|(p, _)| p.clone())
                .collect();
            for p in gone {
                i.changed.remove(&p);
            }
            i.changed
                .iter()
                .map(|(p, (w, no))| {
                    let what = match w {
                        Pending::Deleted => None,
                        Pending::OnDisk => Some(None),
                        Pending::Text(t) => Some(Some(t.clone())),
                    };
                    (p.clone(), what, *no)
                })
                .collect()
        };
        // The files are read without the lock. One changed again meanwhile is read as it is
        // now, under the older number: written now, and once more with the next commit.
        let mut changes = Vec::new();
        let mut numbers = HashMap::new();
        for (path, what, no) in plan {
            let change = match what {
                None => Change::Delete { path: path.clone() },
                Some(Some(text)) => Change::Put { path: path.clone(), text },
                Some(None) => match std::fs::read_to_string(self.core.settings.cache_dir.join(&path)) {
                    Ok(text) => Change::Put { path: path.clone(), text },
                    Err(e) => {
                        // Deleted since - a newer change says so - or the cache lost it.
                        tracing::warn!("GitHub store: {path} is not in the cache ({e}); it waits");
                        continue;
                    }
                },
            };
            numbers.insert(path, no);
            changes.push(change);
        }
        (!changes.is_empty()).then_some((changes, numbers))
    }

    /// Whether the writer may commit now: something to write, [`MIN_GAP`] since the last
    /// try, and the wait after a failure over.
    pub fn due(&self, now: SystemTime) -> bool {
        let i = self.lock();
        !i.changed.is_empty() && now >= i.earliest_commit()
    }

    fn failed(&self, e: RemoteError) -> String {
        let why = e.to_string();
        let mut i = self.lock();
        i.stats.failures += 1;
        let doubled = MIN_GAP.saturating_mul(1 << (i.stats.failures - 1).min(10)).min(MAX_BACKOFF);
        let wait = match e {
            // As long as GitHub says, a minute if it does not - and never less than doubling.
            RemoteError::Limited(said) => said.unwrap_or(Duration::from_secs(60)).max(doubled),
            _ => doubled,
        };
        i.stats.not_before = Some(SystemTime::now() + wait);
        i.stats.last_error = Some((fields::now_stamp(), why.clone()));
        tracing::warn!("GitHub store: not written - {why}; next try in {} s", wait.as_secs());
        why
    }

    /// The store's state for the help page: what anybody may know, and with `admin` the rest.
    pub fn status(&self, admin: bool) -> serde_json::Value {
        // Walking the cache takes a while with thousands of files: not under the lock, which
        // every read and write of a tour waits for.
        let cache_bytes = if admin { dir_size(&self.core.settings.cache_dir) } else { 0 };
        let i = self.lock();
        let tick = i.stats.next_write.unwrap_or_else(SystemTime::now);
        let next_at = if i.changed.is_empty() { tick } else { tick.max(i.earliest_commit()) };
        let next = i
            .stats
            .next_write
            .and(next_at.duration_since(SystemTime::now()).ok())
            .map(|d| d.as_secs());
        let mut v = serde_json::json!({
            "kind": "GitHub",
            "emulated": self.core.remote.emulated(),
            "pending": i.changed.len(),
            "lastWritten": i.stats.last_written,
            "nextWriteInSeconds": next,
            "lastError": i.stats.last_error.as_ref().map(|(at, why)| serde_json::json!({ "at": at, "message": why })),
        });
        if admin {
            let tours = i.index.values().filter(|m| !fields::is_version(&m.stub)).count();
            let extra = serde_json::json!({
                "repository": self.core.remote.describe(),
                "lastCommit": i.stats.last_commit.as_ref().or(Some(&i.head)),
                "lastCommitUrl": self.core.remote.commit_url(i.stats.last_commit.as_deref().unwrap_or(&i.head)),
                "tours": tours,
                "versions": i.index.len() - tours,
                "inMemory": i.hot.len(),
                "memoryBytes": i.hot_bytes,
                "memoryLimitBytes": self.core.settings.memory_bytes,
                "sentToDisk": i.stats.evictions,
                "cacheBytes": cache_bytes,
                "started": i.stats.started,
                "filesAtStart": i.stats.files_at_start,
                "downloadedAtStart": i.stats.downloaded_at_start,
                "flushSeconds": self.core.settings.flush_every.as_secs(),
            });
            if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
                o.extend(e.clone());
            }
        }
        v
    }

    /// The subscriptions, kept in the same repository.
    pub fn subscriptions(&self) -> GitHubSubscriptions {
        GitHubSubscriptions { store: self.clone() }
    }

    /// Seals a tour, puts it in the cache and memory, and marks its file changed.
    fn put(&self, i: &mut Inner, tour: Tour) {
        let json = match tour.to_json() {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("GitHub store: tour {} does not write as JSON: {e}", tour.id.as_str());
                return;
            }
        };
        let path = if fields::is_version(&tour) {
            self.core.keys.version_path(&fields::str_of(&tour, fields::VERSION_FOR), tour.id.as_str())
        } else {
            self.core.keys.tour_path(tour.id.as_str())
        };
        // Moved - a version now said to be of another tour: the old file goes.
        if let Some(old) = i.index.get(&tour.id).map(|m| m.path.clone()).filter(|p| *p != path) {
            self.mark(i, &old, None);
        }
        let text = self.core.keys.seal(&path, json.as_bytes());
        self.mark(i, &path, Some(text));
        let hot = !fields::is_version(&tour);
        i.know(path, tour, json.len(), hot);
        i.evict(self.core.settings.memory_bytes);
    }

    /// A file changed: on disk now, in the repository with the next commit.
    fn mark(&self, i: &mut Inner, path: &str, text: Option<String>) {
        let local = self.core.settings.cache_dir.join(path);
        let pending = match text {
            Some(t) => match write_file(&local, &t) {
                Ok(()) => Pending::OnDisk,
                Err(e) => {
                    tracing::warn!("GitHub store: {e}; kept in memory until written");
                    Pending::Text(t)
                }
            },
            None => {
                let _ = std::fs::remove_file(&local);
                Pending::Deleted
            }
        };
        i.change_no += 1;
        let no = i.change_no;
        i.changed.insert(path.to_owned(), (pending, no));
    }

    /// A tour by id: from memory, or from the disk - and then kept in memory.
    fn load(&self, i: &mut Inner, id: &TourId) -> Option<Arc<Tour>> {
        i.tick += 1;
        let tick = i.tick;
        if let Some(h) = i.hot.get_mut(id) {
            h.used = tick;
            return Some(h.tour.clone());
        }
        let path = i.index.get(id)?.path.clone();
        let tour = self.read_disk(&path)?;
        let len = tour.to_json().map(|j| j.len()).unwrap_or(0);
        let tour = Arc::new(tour);
        // A version is read when somebody restores it, once: not worth a place in memory.
        if !path.starts_with("v/") {
            i.hot_bytes += weight(len);
            i.hot.insert(id.clone(), Hot { tour: tour.clone(), bytes: weight(len), used: tick });
            i.evict(self.core.settings.memory_bytes);
        }
        Some(tour)
    }

    fn read_disk(&self, path: &str) -> Option<Tour> {
        let local = self.core.settings.cache_dir.join(path);
        let text = std::fs::read_to_string(&local)
            .map_err(|e| tracing::error!("GitHub store: {} is not in the cache: {e}", local.display()))
            .ok()?;
        let plain = self.core.keys.open(path, &text).map_err(|e| tracing::error!("GitHub store: {}", e.0)).ok()?;
        let json = String::from_utf8(plain).ok()?;
        Tour::from_json(&json).map_err(|e| tracing::error!("GitHub store: {path}: {e}")).ok()
    }

    fn save_subscriptions(&self, i: &mut Inner) {
        if i.subscriptions_unreadable {
            tracing::warn!(
                "GitHub store: subscriptions not written - the file of them in the repository does not \
                 open with this key, and writing would lose it; kept in memory"
            );
            return;
        }
        let json = serde_json::to_vec(&i.subscriptions).unwrap_or_default();
        let text = self.core.keys.seal(seal::SUBSCRIPTIONS, &json);
        self.mark(i, seal::SUBSCRIPTIONS, Some(text));
    }
}

impl Inner {
    /// The earliest a commit may be tried: [`MIN_GAP`] after the last, and after the wait a
    /// failure set.
    fn earliest_commit(&self) -> SystemTime {
        let gap = self.stats.last_attempt.map(|t| t + MIN_GAP).unwrap_or(SystemTime::UNIX_EPOCH);
        gap.max(self.stats.not_before.unwrap_or(SystemTime::UNIX_EPOCH))
    }

    /// Takes a tour into the index - and into memory, if `hot`.
    fn know(&mut self, path: String, tour: Tour, json_len: usize, hot: bool) {
        let id = tour.id.clone();
        if !self.index.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.index.insert(id.clone(), Meta { path, stub: Arc::new(stub(&tour)) });
        if let Some(old) = self.hot.remove(&id) {
            self.hot_bytes -= old.bytes;
        }
        if hot {
            self.tick += 1;
            let bytes = weight(json_len);
            self.hot_bytes += bytes;
            self.hot.insert(id, Hot { tour: Arc::new(tour), bytes, used: self.tick });
        }
    }

    fn forget(&mut self, id: &TourId) -> Option<String> {
        if let Some(h) = self.hot.remove(id) {
            self.hot_bytes -= h.bytes;
        }
        self.order.retain(|x| x != id);
        self.index.remove(id).map(|m| m.path)
    }

    /// Over the ceiling: the tours not touched for longest go - they are on disk already. The
    /// last one stays, however big: a tour bigger than the ceiling is still read.
    fn evict(&mut self, cap: usize) {
        while self.hot_bytes > cap && self.hot.len() > 1 {
            let Some(oldest) = self.hot.iter().min_by_key(|(_, h)| h.used).map(|(id, _)| id.clone()) else {
                break;
            };
            if let Some(h) = self.hot.remove(&oldest) {
                self.hot_bytes -= h.bytes;
                self.stats.evictions += 1;
            }
        }
    }

    fn stub(&self, id: &TourId) -> Option<Arc<Tour>> {
        self.index.get(id).map(|m| m.stub.clone())
    }
}

/// What a list needs of a tour - its name, who it belongs to, its dates and flags - without
/// people, spendings, currencies or the menu.
fn stub(tour: &Tour) -> Tour {
    let mut s = tour.clone();
    s.persons.clear();
    s.spendings.clear();
    s.currencies.clear();
    s.extras.0.retain(|k, _| STUB_FIELDS.iter().any(|f| f.eq_ignore_ascii_case(k)));
    s
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // Whole or not at all: written beside, then put in place.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Removes the cache's files that the repository does not have.
fn prune(root: &Path, dir: &Path, keep: &HashMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            prune(root, &p, keep);
            let _ = std::fs::remove_dir(&p); // only if empty
        } else if let Ok(rel) = p.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if !keep.contains_key(&rel) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() { dir_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }
        })
        .sum()
}

#[async_trait::async_trait]
impl TourStore for GitHubStore {
    async fn get(&self, id: &TourId) -> Option<Arc<Tour>> {
        let mut i = self.lock();
        self.load(&mut i, id)
    }

    async fn list(
        &self,
        _codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
    ) -> Vec<Arc<Tour>> {
        let mut i = self.lock();
        let ids: Vec<TourId> = i
            .order
            .iter()
            .filter(|id| i.stub(id).is_some_and(|s| !fields::is_version(&s) && allowed(&s)))
            .cloned()
            .collect();
        ids.iter().filter_map(|id| self.load(&mut i, id)).collect()
    }

    async fn page(
        &self,
        _codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
        from: usize,
        count: usize,
    ) -> (Vec<Arc<Tour>>, usize) {
        // Chosen and sorted on what lists need, and only the page read whole - as the
        // in-memory store orders it: newest first by the day it was made, then the id.
        let mut i = self.lock();
        let mut stubs: Vec<Arc<Tour>> = i
            .order
            .iter()
            .filter_map(|id| i.stub(id))
            .filter(|s| !fields::is_version(s) && allowed(s))
            .collect();
        stubs.sort_by(|a, b| {
            let made = |t: &Arc<Tour>| fields::str_of(t, fields::CREATED_AT);
            (made(b), b.id.as_str().to_owned()).cmp(&(made(a), a.id.as_str().to_owned()))
        });
        let total = stubs.len();
        let page: Vec<Arc<Tour>> = stubs
            .iter()
            .skip(from)
            .take(count)
            .filter_map(|s| self.load(&mut i, &s.id))
            .collect();
        (page, total)
    }

    async fn count(
        &self,
        _codes: Option<&[String]>,
        allowed: &(dyn for<'a> Fn(&'a Tour) -> bool + Sync),
    ) -> usize {
        let i = self.lock();
        i.index.values().filter(|m| !fields::is_version(&m.stub) && allowed(&m.stub)).count()
    }

    async fn access_codes(&self, ids: &[String]) -> Vec<(String, String)> {
        let i = self.lock();
        ids.iter()
            .filter_map(|id| i.stub(&TourId::new(id.clone())).map(|s| (id.clone(), fields::access_code(&s))))
            .collect()
    }

    async fn state_of(&self, id: &TourId) -> Option<(String, String)> {
        let s = self.lock().stub(id)?;
        Some((fields::access_code(&s), fields::str_of(&s, fields::STATE)))
    }

    async fn store(&self, tour: Tour) {
        let mut i = self.lock();
        self.put(&mut i, tour);
    }

    async fn remove(&self, id: &TourId) -> bool {
        let mut i = self.lock();
        match i.forget(id) {
            Some(path) => {
                self.mark(&mut i, &path, None);
                true
            }
            None => false,
        }
    }

    async fn replace(
        &self,
        id: &TourId,
        expected_state: &str,
        next: Tour,
        version_of: &(dyn for<'a> Fn(&'a Tour) -> Option<Tour> + Sync),
    ) -> Result<(), Stale> {
        // One lock around reading, comparing and both writes, as in memory.
        let mut i = self.lock();
        let stored = self.load(&mut i, id).ok_or_else(|| Stale(String::new()))?;
        let stored_state = fields::str_of(&stored, fields::STATE);
        if stored_state != expected_state {
            return Err(Stale(stored_state));
        }
        if let Some(version) = version_of(&stored) {
            self.put(&mut i, version);
        }
        self.put(&mut i, next);
        Ok(())
    }

    async fn versions(&self, id: &TourId, from: usize, count: usize) -> (Vec<Arc<Tour>>, usize) {
        // Without their contents, as MongoDB answers: the list is a date and a comment.
        let i = self.lock();
        let mut mine: Vec<Arc<Tour>> = i
            .order
            .iter()
            .rev()
            .filter_map(|v| i.stub(v))
            .filter(|s| fields::is_version(s) && fields::str_of(s, fields::VERSION_FOR) == id.as_str())
            .collect();
        mine.sort_by_key(|v| std::cmp::Reverse(fields::str_of(v, fields::VERSIONED_AT)));
        let total = mine.len();
        (mine.into_iter().skip(from).take(count).collect(), total)
    }

    async fn flush(&self) {
        let _ = GitHubStore::flush(self).await;
    }

    fn status(&self, admin: bool) -> serde_json::Value {
        GitHubStore::status(self, admin)
    }
}

/// The subscriptions, one file in the same repository.
pub struct GitHubSubscriptions {
    store: GitHubStore,
}

#[async_trait::async_trait]
impl SubscriptionStore for GitHubSubscriptions {
    async fn add(&self, tour: &str, sub: Subscription) {
        let mut i = self.store.lock();
        let mine = i.subscriptions.entry(tour.to_owned()).or_default();
        // The same browser twice is one subscription, as in memory.
        if let Some(existing) = mine.iter_mut().find(|s| s.is_same(&sub)) {
            if *existing == sub {
                return;
            }
            *existing = sub;
        } else {
            mine.push(sub);
        }
        self.store.save_subscriptions(&mut i);
    }

    async fn remove(&self, tour: &str, sub: &Subscription) {
        let mut i = self.store.lock();
        let Some(mine) = i.subscriptions.get_mut(tour) else { return };
        let before = mine.len();
        mine.retain(|s| !s.is_same(sub));
        if mine.len() == before {
            return;
        }
        if mine.is_empty() {
            i.subscriptions.remove(tour);
        }
        self.store.save_subscriptions(&mut i);
    }

    async fn has(&self, tour: &str, sub: &Subscription) -> bool {
        self.store.lock().subscriptions.get(tour).is_some_and(|m| m.iter().any(|s| s.is_same(sub)))
    }

    async fn for_tour(&self, tour: &str) -> Vec<Subscription> {
        self.store.lock().subscriptions.get(tour).cloned().unwrap_or_default()
    }

    async fn tours_of(&self, sub: &Subscription) -> Vec<String> {
        let i = self.store.lock();
        let mut tours: Vec<String> = i
            .subscriptions
            .iter()
            .filter(|(_, subs)| subs.iter().any(|s| s.is_same(sub)))
            .map(|(t, _)| t.clone())
            .collect();
        tours.sort();
        tours
    }
}

#[cfg(test)]
mod tests {
    use super::super::remote::Memory;
    use super::*;

    const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

    fn fixture(id: &str) -> Tour {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/zscph2y.tour.json");
        let mut t = Tour::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
        t.id = TourId::new(id.to_owned());
        fields::set(&mut t, fields::STATE, "s1".into());
        t
    }

    /// A cache directory of its own, emptied when the test is done.
    struct Cache(PathBuf);
    impl Cache {
        fn new(name: &str) -> Cache {
            let p = std::env::temp_dir().join(format!("tc-github-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            Cache(p)
        }
    }
    impl Drop for Cache {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn open(repo: &Arc<Memory>, cache: &Cache, memory_bytes: usize) -> GitHubStore {
        let settings = Settings { flush_every: Duration::from_secs(10), memory_bytes, cache_dir: cache.0.clone() };
        GitHubStore::open(Keys::from_setting(KEY).unwrap(), Box::new(repo.clone()), settings).await.unwrap()
    }

    #[tokio::test]
    async fn tours_go_to_the_repository_sealed_in_one_commit_and_come_back() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("roundtrip");
        let store = open(&repo, &cache, 32 << 20).await;
        store.store(fixture("a")).await;
        store.store(fixture("b")).await;
        assert_eq!(repo.commits(), 1, "nothing written before the flush");
        assert_eq!(store.status(false)["pending"], 2);
        // The repository in memory says so, to everyone: "written" must not read as GitHub.
        assert_eq!(store.status(false)["emulated"], true);

        store.flush().await.unwrap();
        assert_eq!(repo.commits(), 2, "one commit for both");
        let files = repo.files();
        let tours: Vec<&String> = files.keys().filter(|p| p.starts_with("t/")).collect();
        assert_eq!(tours.len(), 2);
        assert!(files.values().all(|t| !t.contains("Ural") && !t.contains("zscph2y")));
        assert_eq!(store.status(false)["pending"], 0);

        // Another server, an empty cache: everything from the repository.
        let other = Cache::new("roundtrip-2");
        let again = open(&repo, &other, 32 << 20).await;
        let a = again.get(&TourId::new("a")).await.unwrap();
        assert_eq!(a.spendings.len(), fixture("a").spendings.len());
        assert_eq!(again.count(None, &|_| true).await, 2);
    }

    #[tokio::test]
    async fn a_start_with_the_cache_downloads_only_what_changed() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("cache");
        let store = open(&repo, &cache, 32 << 20).await;
        store.store(fixture("a")).await;
        store.store(fixture("b")).await;
        store.flush().await.unwrap();
        drop(store);

        let reads = repo.reads();
        let again = open(&repo, &cache, 32 << 20).await;
        assert_eq!(repo.reads(), reads, "all from the cache");
        assert_eq!(again.status(true)["downloadedAtStart"], 0);

        // Somebody else changed one tour.
        let mut b = fixture("b");
        b.name = "changed".into();
        let keys = Keys::from_setting(KEY).unwrap();
        let path = keys.tour_path("b");
        repo.move_on(&path, &keys.seal(&path, b.to_json().unwrap().as_bytes()));
        let third = open(&repo, &cache, 32 << 20).await;
        assert_eq!(third.status(true)["downloadedAtStart"], 1);
        assert_eq!(third.get(&TourId::new("b")).await.unwrap().name, "changed");
    }

    #[tokio::test]
    async fn the_soft_lock_holds_and_versions_are_listed_without_their_contents() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("lock");
        let store = open(&repo, &cache, 32 << 20).await;
        let id = TourId::new("a");
        store.store(fixture("a")).await;

        let mut next = fixture("a");
        fields::set(&mut next, fields::STATE, "s2".into());
        let keep = |t: &Tour| Some(crate::api::write::version_of(t, "first".into()));
        assert!(store.replace(&id, "stale", next.clone(), &keep).await.is_err());
        store.replace(&id, "s1", next, &keep).await.unwrap();

        let (kept, total) = store.versions(&id, 0, 10).await;
        assert_eq!(total, 1);
        assert!(kept[0].spendings.is_empty() && kept[0].persons.is_empty());
        assert_eq!(fields::str_of(&kept[0], fields::VERSION_COMMENT), "first");
        // Restoring one reads it whole.
        assert!(!store.get(&kept[0].id).await.unwrap().spendings.is_empty());
        assert_eq!(store.count(None, &|_| true).await, 1, "a version is not a tour");

        store.flush().await.unwrap();
        assert_eq!(repo.files().keys().filter(|p| p.starts_with("v/")).count(), 1);
        let again = open(&repo, &Cache::new("lock-2"), 32 << 20).await;
        assert_eq!(again.versions(&id, 0, 10).await.1, 1);
        assert_eq!(again.state_of(&id).await.unwrap().1, "s2");
    }

    #[tokio::test]
    async fn over_the_ceiling_a_tour_goes_to_disk_and_comes_back() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("evict");
        // About one tour of this size.
        let store = open(&repo, &cache, 1 << 20).await;
        for id in ["a", "b", "c"] {
            store.store(fixture(id)).await;
        }
        let s = store.status(true);
        assert_eq!(s["inMemory"], 1, "{s}");
        assert_eq!(s["sentToDisk"], 2);
        assert!(s["memoryBytes"].as_u64().unwrap() <= 1 << 20);
        // Not in memory, still there - read from the disk.
        let a = store.get(&TourId::new("a")).await.unwrap();
        assert_eq!(a.spendings.len(), fixture("a").spendings.len());
        assert_eq!(store.list(None, &|_| true).await.len(), 3);
    }

    #[tokio::test]
    async fn a_moved_branch_is_read_again_and_written_on_top() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("moved");
        let store = open(&repo, &cache, 32 << 20).await;
        store.store(fixture("a")).await;
        repo.move_on("README.md", "somebody else\n");
        store.flush().await.unwrap();
        let files = repo.files();
        assert_eq!(files["README.md"], "somebody else\n", "theirs kept");
        assert!(files.keys().any(|p| p.starts_with("t/")), "ours written");
    }

    #[tokio::test]
    async fn a_failed_write_keeps_the_changes_and_says_so() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("failed");
        let store = open(&repo, &cache, 32 << 20).await;
        store.store(fixture("a")).await;
        repo.fail_with(Some("GitHub answered 503"));
        assert!(store.flush().await.is_err());
        let s = store.status(false);
        assert_eq!(s["pending"], 1);
        assert_eq!(s["lastError"]["message"], "GitHub answered 503");
        repo.fail_with(None);
        store.flush().await.unwrap();
        let s = store.status(false);
        assert_eq!(s["pending"], 0);
        assert!(s["lastError"].is_null());
    }

    #[tokio::test]
    async fn a_deleted_tour_leaves_the_repository() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("deleted");
        let store = open(&repo, &cache, 32 << 20).await;
        store.store(fixture("a")).await;
        store.flush().await.unwrap();
        assert!(store.remove(&TourId::new("a")).await);
        store.flush().await.unwrap();
        assert!(!repo.files().keys().any(|p| p.starts_with("t/")));
        // Made and deleted between two writes: GitHub hears nothing of it.
        let commits = repo.commits();
        store.store(fixture("b")).await;
        store.remove(&TourId::new("b")).await;
        store.flush().await.unwrap();
        assert_eq!(repo.commits(), commits);
    }

    #[tokio::test]
    async fn subscriptions_outlive_the_server() {
        let repo = Arc::new(Memory::new());
        let cache = Cache::new("subs");
        let store = open(&repo, &cache, 32 << 20).await;
        let sub = Subscription { url: "https://push.example/1".into(), p256dh: "k".into(), auth: "a".into() };
        let subs = store.subscriptions();
        subs.add("a", sub.clone()).await;
        subs.add("a", sub.clone()).await;
        store.flush().await.unwrap();
        let again = open(&repo, &Cache::new("subs-2"), 32 << 20).await.subscriptions();
        assert_eq!(again.for_tour("a").await, vec![sub.clone()]);
        assert_eq!(again.tours_of(&sub).await, vec!["a".to_owned()]);
    }

    const OTHER: &str = "HxwdHhkaGxwVFhcYERITFA0ODxAJCgsMBQYHCAECAwQ=";

    #[tokio::test]
    async fn the_wrong_key_does_not_start_and_changes_nothing() {
        let repo = Arc::new(Memory::new());
        let store = open(&repo, &Cache::new("key"), 32 << 20).await;
        store.store(fixture("a")).await;
        let sub = Subscription { url: "https://push.example/1".into(), p256dh: "k".into(), auth: "a".into() };
        store.subscriptions().add("a", sub.clone()).await;
        store.flush().await.unwrap();
        let before = repo.files();

        let settings = Settings { flush_every: Duration::from_secs(10), memory_bytes: 32 << 20, cache_dir: Cache::new("key-2").0.clone() };
        let wrong = GitHubStore::open(Keys::from_setting(OTHER).unwrap(), Box::new(repo.clone()), settings).await;
        assert!(wrong.err().unwrap().contains("GitHubStoreKey opens none of the 2 file(s)"));
        assert_eq!(repo.files(), before);
        // And an empty repository opens with any key: nothing to tell it by.
        assert!(open(&Arc::new(Memory::new()), &Cache::new("key-3"), 32 << 20).await.count(None, &|_| true).await == 0);
    }

    #[tokio::test]
    async fn subscriptions_that_do_not_open_are_not_written_over() {
        let repo = Arc::new(Memory::new());
        let store = open(&repo, &Cache::new("subs-key"), 32 << 20).await;
        store.store(fixture("a")).await;
        store.flush().await.unwrap();
        // The file of subscriptions, sealed with another key.
        let theirs = Keys::from_setting(OTHER).unwrap().seal(seal::SUBSCRIPTIONS, b"{}");
        repo.move_on(seal::SUBSCRIPTIONS, &theirs);

        let again = open(&repo, &Cache::new("subs-key-2"), 32 << 20).await;
        let sub = Subscription { url: "https://push.example/1".into(), p256dh: "k".into(), auth: "a".into() };
        again.subscriptions().add("a", sub.clone()).await;
        assert_eq!(again.subscriptions().for_tour("a").await, vec![sub], "kept in memory");
        again.flush().await.unwrap();
        assert_eq!(repo.files()[seal::SUBSCRIPTIONS], theirs);
    }

    #[tokio::test]
    async fn a_lost_answer_or_a_file_deleted_by_hand_does_not_hold_the_rest_back() {
        let repo = Arc::new(Memory::new());
        let store = open(&repo, &Cache::new("gone"), 32 << 20).await;
        let keys = Keys::from_setting(KEY).unwrap();
        for id in ["a", "b"] {
            store.store(fixture(id)).await;
        }
        store.flush().await.unwrap();

        // The commit is made, its answer lost: the delete would be asked for twice.
        assert!(store.remove(&TourId::new("a")).await);
        repo.lose_next_answer();
        assert!(store.flush().await.is_err());
        store.flush().await.unwrap();
        assert_eq!(store.status(false)["pending"], 0);
        assert!(!repo.files().contains_key(&keys.tour_path("a")));

        // Deleted by hand, and then here: nothing left to delete, and a change beside it goes.
        repo.delete_by_hand(&keys.tour_path("b"));
        assert!(store.remove(&TourId::new("b")).await);
        store.store(fixture("c")).await;
        store.flush().await.unwrap();
        assert_eq!(store.status(false)["pending"], 0);
        assert!(repo.files().contains_key(&keys.tour_path("c")));
    }

    #[tokio::test]
    async fn the_writer_waits_between_commits_and_longer_after_failures() {
        let repo = Arc::new(Memory::new());
        let store = open(&repo, &Cache::new("pace"), 32 << 20).await;
        let s = |n| Duration::from_secs(n);
        assert!(!store.due(SystemTime::now()), "nothing to write");
        store.store(fixture("a")).await;
        assert!(store.due(SystemTime::now()), "the first commit at once");
        store.flush().await.unwrap();

        store.store(fixture("b")).await;
        let now = SystemTime::now();
        assert!(!store.due(now + s(25)), "not within 30 s of the last");
        assert!(store.due(now + s(31)));

        // A rate limit: as long as GitHub says.
        repo.limit(Some(Some(s(120))));
        assert!(store.flush().await.is_err());
        let now = SystemTime::now();
        assert!(!store.due(now + s(115)));
        assert!(store.due(now + s(125)));
        assert!(store.status(false)["lastError"]["message"].as_str().unwrap().contains("rate limit"));

        // Failures in a row: the wait doubles.
        repo.limit(None);
        repo.fail_with(Some("GitHub answered 502"));
        assert!(store.flush().await.is_err());
        let now = SystemTime::now();
        assert!(!store.due(now + s(55)), "the second failure in a row: a minute");
        assert!(store.due(now + s(65)));

        repo.fail_with(None);
        store.flush().await.unwrap();
        let s2 = store.status(false);
        assert!(s2["lastError"].is_null() && s2["pending"] == 0, "{s2}");
    }

    #[tokio::test]
    async fn a_change_waiting_for_the_repository_is_not_held_in_memory() {
        let repo = Arc::new(Memory::new());
        let store = open(&repo, &Cache::new("pending"), 32 << 20).await;
        store.store(fixture("a")).await;
        assert!(store.lock().changed.values().all(|(p, _)| matches!(p, Pending::OnDisk)));
        store.flush().await.unwrap();
        let keys = Keys::from_setting(KEY).unwrap();
        let path = keys.tour_path("a");
        let back = keys.open(&path, &repo.files()[&path]).unwrap();
        assert_eq!(Tour::from_json(std::str::from_utf8(&back).unwrap()).unwrap().spendings.len(), fixture("a").spendings.len());
    }
}
