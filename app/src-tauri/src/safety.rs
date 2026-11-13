//! One safety net for every file the app changes in a game folder: before a tool overwrites or
//! deletes a file, the file as it was is kept, so any change can be undone, any earlier version
//! put back, and Loner-Strike's own files put back the way it ships them.
//!
//! - Kept in the app's data folder under `backups/files/<name>-<hash>/`: `index.json` (the
//!   path, and one entry per change: when, which tool, why, size, checksum, whether the file
//!   was there at all) and `blobs/<sha256>` (each content once). The last [`KEEP`] versions
//!   of a file stay, and always the very first one (the file before Loner-Strike's tools ever
//!   changed it). All of it together stays under [`CAP`]: the oldest versions go first.
//! - A version is "the file just before a change". Putting one back keeps the file as it is
//!   now first, so a restore can be undone the same way.
//! - Only files inside a game folder are kept or written: the folders the app registered
//!   ([`use_game`]) or a Half-Life folder (one with `valve/` in it). Never a path with `..`, a
//!   shortcut (symlink), or anything outside them; never the backups themselves.
//! - Writes are whole or not at all (a temporary file renamed over the old one).
//! - The routes (`/api/safety/*`) list versions, compare, restore, undo the last save, restore
//!   Loner-Strike's defaults and clear old backups. Every POST answered while a version was
//!   kept says which (`X-LS-Undo: <from>-<to>`), so the page can offer "Undo".
//!
//! Writers: [`save`] / [`remove`] when the app writes the file itself; [`before`] as a
//! one-line hook in a writer that already writes its own way (never fails the save).

use crate::server::Shared;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Versions kept per file (besides the first-ever one)
pub const KEEP: usize = 20;
/// All backups together stay under this
pub const CAP: u64 = 2_000_000_000;
/// A file bigger than this isn't kept (its change is still listed, but can't be undone)
pub const MAX_FILE: u64 = 300_000_000;
/// Text bigger than this isn't compared line by line
const MAX_TEXT: usize = 2_000_000;
/// Where the files live inside the store
const FILES: &str = "files";

/// One version of a file: how it was just before a change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Version {
    pub seq: u64,
    /// seconds since 1970
    pub time: f64,
    pub tool: String,
    pub reason: String,
    /// false: the file wasn't there (putting this back deletes it)
    pub existed: bool,
    pub size: u64,
    /// the content's sha256; None when it wasn't there or was too big to keep
    #[serde(default)]
    pub sha: Option<String>,
    /// the file before Loner-Strike's tools first changed it (never pruned)
    #[serde(default)]
    pub original: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    /// the full path
    pub path: String,
    /// the path inside its game folder ("cstrike/maps/de_dust2.nav")
    pub rel: String,
    pub versions: Vec<Version>,
}

pub struct Store {
    root: PathBuf,
    roots: RwLock<Vec<PathBuf>>,
    lock: Mutex<u64>,
    keep: usize,
    cap: u64,
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Text compared the way a person sees it: \r\n read as \n, the end's blank lines ignored.
fn same_text(a: &[u8], b: &[u8]) -> bool {
    if a == b {
        return true;
    }
    let norm = |x: &[u8]| {
        let mut v: Vec<u8> = Vec::with_capacity(x.len());
        for (i, &c) in x.iter().enumerate() {
            if !(c == b'\r' && x.get(i + 1) == Some(&b'\n')) {
                v.push(c);
            }
        }
        while v.last().is_some_and(|c| *c == b'\n' || *c == b'\r') {
            v.pop();
        }
        v
    };
    is_text(a) && is_text(b) && norm(a) == norm(b)
}

/// Looks like a text file (no NUL bytes, not huge).
pub fn is_text(b: &[u8]) -> bool {
    b.len() <= MAX_TEXT && !b.contains(&0)
}

/// GoldSrc text is Latin-1: every byte one character.
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Lines added and taken away going from `a` to `b` (as a multiset: moved lines don't count).
pub fn line_change(a: &[u8], b: &[u8]) -> Option<(usize, usize)> {
    if !is_text(a) || !is_text(b) {
        return None;
    }
    let (a, b) = (latin1(a), latin1(b));
    let mut count: BTreeMap<&str, i64> = BTreeMap::new();
    for l in a.lines() {
        *count.entry(l.trim_end()).or_default() -= 1;
    }
    for l in b.lines() {
        *count.entry(l.trim_end()).or_default() += 1;
    }
    let added = count.values().filter(|v| **v > 0).map(|v| *v as usize).sum();
    let removed = count.values().filter(|v| **v < 0).map(|v| (-*v) as usize).sum();
    Some((added, removed))
}

/// A Half-Life folder: where `valve/` (or the dedicated server's program) is.
fn is_hl_root(p: &Path) -> bool {
    p.join("valve").is_dir() || p.join("hlds.exe").is_file() || p.join("hlds_run").is_file() || p.join("hl.exe").is_file()
}

/// `p` with every folder that exists made canonical (shortcuts followed), and the parts that
/// don't exist yet added on. None for a relative path or one with `..` / `.` in it.
fn resolve(p: &Path) -> Option<PathBuf> {
    if !p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return None;
    }
    let mut existing = p.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if existing.exists() {
            break;
        }
        rest.push(existing.file_name()?.to_os_string());
        existing = existing.parent()?.to_path_buf();
    }
    let mut out = crate::sys::canonical(&existing).ok()?;
    for r in rest.into_iter().rev() {
        out.push(r);
    }
    Some(out)
}

fn safe_name(s: &str) -> String {
    let tail: Vec<&str> = s.rsplit('/').take(2).collect();
    let mut n: String = tail.into_iter().rev().collect::<Vec<_>>().join("_").chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    n.truncate(60);
    n.trim_start_matches('.').to_string()
}

/// A store key: only these characters, no way out of the store
pub fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 90 && !k.starts_with('.') && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

impl Store {
    pub fn new(root: PathBuf) -> Store {
        Store { root, roots: RwLock::new(Vec::new()), lock: Mutex::new(0), keep: KEEP, cap: CAP }
    }

    #[cfg(test)]
    fn with_limits(root: PathBuf, keep: usize, cap: u64) -> Store {
        Store { keep, cap, ..Store::new(root) }
    }

    #[allow(dead_code)] // part of the API for writers (Game Mode Studio, the editor), and tests
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A folder whose files may be kept and written (a game folder).
    pub fn allow_root(&self, p: &Path) {
        if let Ok(c) = crate::sys::canonical(p) {
            let mut r = self.roots.write().unwrap();
            if !r.contains(&c) {
                r.push(c);
            }
        }
    }

    /// The checked full path and its game folder, or why it can't be kept.
    pub fn check(&self, path: &Path) -> Result<(PathBuf, PathBuf), String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let bad = || format!("Loner-Strike only changes files inside your game folders, so it won't touch {name}.");
        let full = resolve(path).ok_or_else(bad)?;
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!("{name} is a shortcut to somewhere else, so Loner-Strike won't change it."));
        }
        if std::fs::metadata(&full).is_ok_and(|m| m.is_dir()) {
            return Err(bad());
        }
        if let Some(store) = resolve(&self.root) {
            if full.starts_with(&store) {
                return Err(bad());
            }
        }
        let registered = self.roots.read().unwrap().iter().filter(|r| full.starts_with(r) && full != **r).max_by_key(|r| r.as_os_str().len()).cloned();
        let root = registered.or_else(|| full.ancestors().skip(1).find(|a| is_hl_root(a)).map(Path::to_path_buf)).ok_or_else(bad)?;
        Ok((full, root))
    }

    fn key_of(full: &Path, rel: &str) -> String {
        let id = sha(full.to_string_lossy().as_bytes());
        format!("{}-{}", safe_name(rel), &id[..12])
    }

    fn dir(&self, key: &str) -> PathBuf {
        self.root.join(FILES).join(key)
    }

    fn read_index(&self, key: &str) -> Option<Index> {
        if !valid_key(key) {
            return None;
        }
        let text = std::fs::read(self.dir(key).join("index.json")).ok()?;
        serde_json::from_slice(&text).ok()
    }

    fn write_index(&self, key: &str, ix: &Index) -> Result<(), String> {
        crate::sys::write_atomic(&self.dir(key).join("index.json"), serde_json::to_string_pretty(ix).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't keep a copy ({e})."))
    }

    fn next_seq(last: &mut u64) -> u64 {
        let t = (now() * 1_000_000.0) as u64;
        *last = t.max(*last + 1);
        *last
    }

    /// The newest version number given out (0: none yet this run).
    pub fn last_seq(&self) -> u64 {
        *self.lock.lock().unwrap()
    }

    /// Keeps the file as it is now (under the lock). Returns the version's number.
    fn keep_locked(&self, last: &mut u64, tool: &str, reason: &str, full: &Path, root: &Path) -> Result<u64, String> {
        let rel = full.strip_prefix(root).unwrap_or(full).to_string_lossy().replace('\\', "/");
        let key = Store::key_of(full, &rel);
        let mut ix = self.read_index(&key).unwrap_or_default();
        ix.path = full.to_string_lossy().to_string();
        ix.rel = rel;
        let meta = std::fs::metadata(full).ok().filter(|m| m.is_file());
        let mut v = Version { seq: Store::next_seq(last), time: now(), tool: tool.into(), reason: reason.into(), existed: meta.is_some(), size: meta.as_ref().map(|m| m.len()).unwrap_or(0), sha: None, original: ix.versions.is_empty() };
        if let Some(m) = &meta {
            if m.len() <= MAX_FILE {
                let bytes = std::fs::read(full).map_err(|e| format!("Couldn't keep a copy of {} ({e}).", full.file_name().unwrap_or_default().to_string_lossy()))?;
                let h = sha(&bytes);
                let blob = self.dir(&key).join("blobs").join(&h);
                if !blob.is_file() {
                    crate::sys::write_atomic(&blob, &bytes).map_err(|e| format!("Couldn't keep a copy ({e})."))?;
                }
                v.size = bytes.len() as u64;
                v.sha = Some(h);
            }
        }
        let seq = v.seq;
        ix.versions.push(v);
        self.prune_index(&key, &mut ix);
        self.write_index(&key, &ix)?;
        self.enforce_cap();
        Ok(seq)
    }

    /// Keeps the file as it is now, before the caller changes it. Ok(seq) of the version.
    pub fn keep(&self, tool: &str, reason: &str, path: &Path) -> Result<u64, String> {
        let (full, root) = self.check(path)?;
        let mut last = self.lock.lock().unwrap();
        self.keep_locked(&mut last, tool, reason, &full, &root)
    }

    /// Writes `bytes` whole over `path`, the file as it was kept first. Returns the version kept.
    pub fn save(&self, tool: &str, reason: &str, path: &Path, bytes: &[u8]) -> Result<u64, String> {
        let (full, root) = self.check(path)?;
        let mut last = self.lock.lock().unwrap();
        let seq = self.keep_locked(&mut last, tool, reason, &full, &root)?;
        write_whole(&full, bytes)?;
        Ok(seq)
    }

    /// Deletes `path`, kept first. Ok(None) when it wasn't there.
    #[allow(dead_code)] // part of the API for writers (Game Mode Studio, the editor), and tests
    pub fn remove(&self, tool: &str, reason: &str, path: &Path) -> Result<Option<u64>, String> {
        let (full, root) = self.check(path)?;
        if !full.is_file() {
            return Ok(None);
        }
        let mut last = self.lock.lock().unwrap();
        let seq = self.keep_locked(&mut last, tool, reason, &full, &root)?;
        delete(&full)?;
        Ok(Some(seq))
    }

    /// Puts `bytes` (None: no file) at a kept file's path, keeping the file as it is first.
    fn put(&self, last: &mut u64, ix: &Index, bytes: Option<&[u8]>, tool: &str, reason: &str) -> Result<u64, String> {
        let (full, root) = self.check(Path::new(&ix.path))?;
        let seq = self.keep_locked(last, tool, reason, &full, &root)?;
        match bytes {
            Some(b) => write_whole(&full, b)?,
            None => delete(&full)?,
        }
        Ok(seq)
    }

    /// A version's content: Ok(None) when the file wasn't there then.
    pub fn version_bytes(&self, key: &str, seq: u64) -> Result<Option<Vec<u8>>, String> {
        let ix = self.read_index(key).ok_or(GONE)?;
        let v = ix.versions.iter().find(|v| v.seq == seq).ok_or(GONE)?;
        self.bytes_of(key, v)
    }

    fn bytes_of(&self, key: &str, v: &Version) -> Result<Option<Vec<u8>>, String> {
        if !v.existed {
            return Ok(None);
        }
        let h = v.sha.as_deref().ok_or("That version was too big to keep a copy of.")?;
        let b = std::fs::read(self.dir(key).join("blobs").join(h)).map_err(|_| GONE.to_string())?;
        if sha(&b) != h {
            return Err("That kept copy is damaged, so it can't be put back.".into());
        }
        Ok(Some(b))
    }

    /// Puts a version back (the file as it is now kept first). Returns the new version.
    pub fn restore(&self, key: &str, seq: u64, tool: &str) -> Result<u64, String> {
        let mut last = self.lock.lock().unwrap();
        let ix = self.read_index(key).ok_or(GONE)?;
        let v = ix.versions.iter().find(|v| v.seq == seq).ok_or(GONE)?.clone();
        let bytes = self.bytes_of(key, &v)?;
        self.put(&mut last, &ix, bytes.as_deref(), tool, "Before putting back an earlier version")
    }

    /// Writes given content to a kept file (Loner-Strike's default), keeping it first.
    pub fn put_key(&self, key: &str, bytes: Option<&[u8]>, tool: &str, reason: &str) -> Result<u64, String> {
        let mut last = self.lock.lock().unwrap();
        let ix = self.read_index(key).ok_or(GONE)?;
        self.put(&mut last, &ix, bytes, tool, reason)
    }

    /// Undoes every change kept with a number in (from, to]: each file goes back to how it
    /// was before the first of them. Returns the files put back.
    pub fn undo(&self, from: u64, to: u64) -> Result<Vec<String>, String> {
        let mut last = self.lock.lock().unwrap();
        let mut done = Vec::new();
        for (key, ix) in self.all() {
            let Some(v) = ix.versions.iter().filter(|v| v.seq > from && v.seq <= to).min_by_key(|v| v.seq).cloned() else { continue };
            let bytes = self.bytes_of(&key, &v)?;
            self.put(&mut last, &ix, bytes.as_deref(), "undo", "Before undoing a save")?;
            done.push(ix.rel.clone());
        }
        if done.is_empty() {
            return Err("There's nothing to undo any more.".into());
        }
        Ok(done)
    }

    /// Every kept file: (key, index), newest change first.
    pub fn all(&self) -> Vec<(String, Index)> {
        let mut v: Vec<(String, Index)> = std::fs::read_dir(self.root.join(FILES))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let k = e.file_name().to_string_lossy().to_string();
                let ix = self.read_index(&k)?;
                Some((k, ix))
            })
            .collect();
        v.sort_by(|a, b| b.1.versions.last().map(|x| x.seq).cmp(&a.1.versions.last().map(|x| x.seq)));
        v
    }

    pub fn index(&self, key: &str) -> Option<Index> {
        self.read_index(key)
    }

    /// The key a path is kept under (whether or not it has been).
    #[allow(dead_code)] // part of the API for writers (Game Mode Studio, the editor), and tests
    pub fn key_for(&self, path: &Path) -> Option<String> {
        let (full, root) = self.check(path).ok()?;
        let rel = full.strip_prefix(&root).unwrap_or(&full).to_string_lossy().replace('\\', "/");
        Some(Store::key_of(&full, &rel))
    }

    /// The last `keep` versions, the original, and the blobs they use; the rest goes.
    fn prune_index(&self, key: &str, ix: &mut Index) {
        let n = ix.versions.len();
        if n > self.keep + 1 {
            let cut = n - self.keep;
            let mut i = 0;
            ix.versions.retain(|v| {
                i += 1;
                v.original || i > cut
            });
        }
        self.gc_blobs(key, ix);
    }

    fn gc_blobs(&self, key: &str, ix: &Index) {
        let used: HashSet<&str> = ix.versions.iter().filter_map(|v| v.sha.as_deref()).collect();
        for e in std::fs::read_dir(self.dir(key).join("blobs")).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if !used.contains(n.as_str()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    /// (bytes on disk, files kept, versions kept)
    pub fn usage(&self) -> (u64, usize, usize) {
        let mut bytes = 0;
        let mut files = 0;
        let mut versions = 0;
        for (key, ix) in self.all() {
            files += 1;
            versions += ix.versions.len();
            for e in std::fs::read_dir(self.dir(&key).join("blobs")).into_iter().flatten().flatten() {
                bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        (bytes, files, versions)
    }

    /// Over the cap: the oldest versions go (never a file's original or its newest).
    fn enforce_cap(&self) {
        let (mut bytes, _, _) = self.usage();
        if bytes <= self.cap {
            return;
        }
        let mut all = self.all();
        let mut cands: Vec<(u64, usize, usize)> = Vec::new(); // (seq, file, version)
        for (fi, (_, ix)) in all.iter().enumerate() {
            let n = ix.versions.len();
            for (vi, v) in ix.versions.iter().enumerate() {
                if !v.original && vi + 1 < n {
                    cands.push((v.seq, fi, vi));
                }
            }
        }
        cands.sort();
        let mut drop: Vec<HashSet<u64>> = vec![HashSet::new(); all.len()];
        for (seq, fi, vi) in cands {
            if bytes <= self.cap {
                break;
            }
            let ix = &all[fi].1;
            let v = &ix.versions[vi];
            // only the blob's last user frees its bytes
            if let Some(h) = &v.sha {
                if !ix.versions.iter().any(|o| o.seq != seq && !drop[fi].contains(&o.seq) && o.sha.as_ref() == Some(h)) {
                    bytes = bytes.saturating_sub(v.size);
                }
            }
            drop[fi].insert(seq);
        }
        for (fi, (key, ix)) in all.iter_mut().enumerate() {
            if drop[fi].is_empty() {
                continue;
            }
            ix.versions.retain(|v| !drop[fi].contains(&v.seq));
            let _ = self.write_index(key, ix);
            self.gc_blobs(key, ix);
        }
    }

    /// "Clear old backups": every file keeps its original and its newest version. Returns the
    /// bytes freed.
    pub fn clear_old(&self) -> u64 {
        let _l = self.lock.lock().unwrap();
        let (before, _, _) = self.usage();
        for (key, mut ix) in self.all() {
            let n = ix.versions.len();
            let mut i = 0;
            ix.versions.retain(|v| {
                i += 1;
                v.original || i == n
            });
            let _ = self.write_index(&key, &ix);
            self.gc_blobs(&key, &ix);
        }
        before.saturating_sub(self.usage().0)
    }

    /// Forgets a file's backups entirely (it isn't there and nobody wants it back).
    #[cfg(test)]
    fn forget(&self, key: &str) {
        if valid_key(key) {
            let _ = std::fs::remove_dir_all(self.dir(key));
        }
    }
}

const GONE: &str = "That version isn't kept any more.";

fn write_whole(full: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = full.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::sys::make_writable(full);
    crate::sys::write_atomic(full, bytes).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Loner-Strike isn't allowed to change {name}."),
        _ => format!("Couldn't save {name} ({e})."),
    })
}

fn delete(full: &Path) -> Result<(), String> {
    match std::fs::remove_file(full) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Couldn't remove {} ({e}).", full.file_name().unwrap_or_default().to_string_lossy())),
    }
}

// ---- the app's store ------------------------------------------------------------------------

static STORE: OnceLock<RwLock<Arc<Store>>> = OnceLock::new();

fn default_root() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join(format!("ls-safety-test-{}", std::process::id()))
    } else {
        crate::sys::app_dir().join("backups")
    }
}

/// The app's store.
pub fn store() -> Arc<Store> {
    STORE.get_or_init(|| RwLock::new(Arc::new(Store::new(default_root())))).read().unwrap().clone()
}

/// The player's game is this one: its folder may be kept and written, and the backups go in
/// its app folder (the app's own, or a temporary one for the dev server).
pub fn use_game(g: &crate::game::Game) {
    if g.remote.is_some() {
        return;
    }
    let root = g.app.join("backups");
    let lock = STORE.get_or_init(|| RwLock::new(Arc::new(Store::new(root.clone()))));
    if lock.read().unwrap().root != root {
        let old = lock.read().unwrap().roots.read().unwrap().clone();
        let s = Store::new(root);
        *s.roots.write().unwrap() = old;
        *lock.write().unwrap() = Arc::new(s);
    }
    let s = store();
    s.allow_root(&g.hl);
    s.allow_root(&g.dir);
}

/// Hook for a writer that writes its own way: keeps the file as it is now, before the change.
/// Never fails the save: a path outside the game folders (a file the player picked) is left
/// alone, and a copy that couldn't be made is only logged.
pub fn before(tool: &str, reason: &str, path: &Path) -> Option<u64> {
    let s = store();
    if s.check(path).is_err() {
        return None;
    }
    match s.keep(tool, reason, path) {
        Ok(seq) => Some(seq),
        Err(e) => {
            eprintln!("backups: {e}");
            None
        }
    }
}

/// Writes a file whole, kept first.
#[allow(dead_code)] // part of the API for writers (Game Mode Studio, the editor), and tests
pub fn save(tool: &str, reason: &str, path: &Path, bytes: &[u8]) -> Result<u64, String> {
    store().save(tool, reason, path, bytes)
}

/// Deletes a file, kept first.
#[allow(dead_code)] // part of the API for writers (Game Mode Studio, the editor), and tests
pub fn remove(tool: &str, reason: &str, path: &Path) -> Result<Option<u64>, String> {
    store().remove(tool, reason, path)
}

// ---- Loner-Strike's defaults ------------------------------------------------------------------

/// Where a file's default comes from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultKind {
    /// a file Loner-Strike ships (its resources)
    Shipped,
    /// the file before Loner-Strike's tools first changed it (maybe: not there at all)
    Original,
}

const GM: &str = "addons/amxmodx/configs/gamemodes/";

/// Loner-Strike's shipped version of a file, shaped for what's installed, if it ships one.
pub fn shipped(res: &Path, path: &Path) -> Option<Vec<u8>> {
    let p = path.to_string_lossy().replace('\\', "/");
    let at = p.find(GM)?;
    let rel = &p[at + GM.len()..];
    if rel.is_empty() || rel.split('/').any(|x| x.is_empty() || x == "..") {
        return None;
    }
    let src = rel.split('/').fold(res.join("plugins").join("configs").join("gamemodes"), |d, x| d.join(x));
    let text = crate::sys::read_latin1(&src).ok()?;
    let gm_dir = PathBuf::from(&p[..at + GM.len() - 1]);
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let mode_ok = |m: &str| crate::setup::core::CORE_MODES.contains(&m) || gm_dir.join(format!("{m}.cfg")).is_file();
    let addon_ok = |a: &str| std::fs::read_dir(gm_dir.join("addons")).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().starts_with(&format!("{a}.")));
    let current = if base == "modes.ini" { crate::sys::read_latin1(path).unwrap_or_default() } else { String::new() };
    let shaped = crate::setup::core::shape_config(base, text, &mode_ok, &addon_ok, &current, &gm_dir);
    Some(shaped.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect())
}

/// A kept file's default: (kind, content; None = no file).
fn default_of(s: &Store, res: Option<&Path>, key: &str, ix: &Index) -> Option<(DefaultKind, Option<Vec<u8>>)> {
    if let Some(b) = res.and_then(|r| shipped(r, Path::new(&ix.path))) {
        return Some((DefaultKind::Shipped, Some(b)));
    }
    let first = ix.versions.iter().find(|v| v.original)?;
    Some((DefaultKind::Original, s.bytes_of(key, first).ok()?))
}

fn differs(path: &Path, want: &Option<Vec<u8>>) -> bool {
    let now = std::fs::read(path).ok();
    match (&now, want) {
        (None, None) => false,
        (Some(a), Some(b)) => !same_text(a, b),
        _ => true,
    }
}

/// What part of the game a file belongs to, for grouping.
pub fn area(rel: &str) -> &'static str {
    let r = rel.to_ascii_lowercase();
    let base = r.rsplit('/').next().unwrap_or(&r);
    if base.starts_with("people.ini") || base.starts_with("regular_names") || base.starts_with("names.ini") || base.starts_with("botprofile") || base.starts_with("botchat") || base.starts_with("botplay") {
        "Bot people"
    } else if r.contains("/maps/") || r.contains("/overviews/") || base.ends_with(".nav") || base.ends_with(".lsv") || base.ends_with(".bsp") || base.ends_with(".res") {
        "Maps & nav"
    } else if r.contains("amxmodx/plugins/") || r.contains("amxmodx/scripting/") || base.starts_with("plugins") {
        "Plugins"
    } else if r.contains(GM) || r.contains("data/gamemodes/") {
        "Game modes"
    } else if base.ends_with(".wad") || r.contains("/decals") || r.contains("/gfx/") {
        "Sprays & textures"
    } else {
        "Settings files"
    }
}

// ---- routes -----------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error with its backups. Try again.".into())))
}

/// The store, with the player's game registered (the dev server sets the game without set_game).
fn ready(st: &Shared) -> Arc<Store> {
    if let Some(g) = st.local_game() {
        use_game(&g);
    }
    store()
}

/// `maps/cs_rats2.*` against "cstrike/maps/cs_rats2.nav": the pattern matches the whole path
/// or its end from a folder on (case ignored; `*` is any run of characters but '/').
pub fn path_matches(pattern: &str, rel: &str) -> bool {
    fn glob(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => glob(&p[1..], s) || (!s.is_empty() && s[0] != b'/' && glob(p, &s[1..])),
            (Some(a), Some(b)) => a == b && glob(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let p = pattern.to_ascii_lowercase().replace('\\', "/");
    let r = rel.to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    let p = p.trim_start_matches('/').as_bytes();
    glob(p, r.as_bytes()) || r.match_indices('/').any(|(i, _)| glob(p, &r.as_bytes()[i + 1..]))
}

fn file_json(s: &Store, res: Option<&Path>, key: &str, ix: &Index, with_changes: bool) -> Value {
    let path = Path::new(&ix.path);
    let current = std::fs::read(path).ok();
    let def = default_of(s, res, key, ix);
    let mut versions: Vec<Value> = Vec::new();
    // each version against what came after it: what that change did
    let mut after: Option<Vec<u8>> = current.clone();
    let mut after_exists = current.is_some();
    for v in ix.versions.iter().rev() {
        let mut j = json!({"seq": v.seq, "time": v.time, "tool": v.tool, "reason": v.reason, "existed": v.existed, "size": v.size, "original": v.original, "kept": !v.existed || v.sha.is_some()});
        let mine = if with_changes { s.bytes_of(key, v).ok().flatten() } else { None };
        if with_changes {
            j["created"] = json!(!v.existed && after_exists);
            j["deleted"] = json!(v.existed && !after_exists);
            if let (Some(a), Some(b)) = (&mine, &after) {
                if let Some((add, del)) = line_change(a, b) {
                    j["added"] = json!(add);
                    j["removed"] = json!(del);
                }
                j["same"] = json!(same_text(a, b));
            }
            j["text"] = json!(mine.as_deref().map(is_text).unwrap_or(false));
        }
        versions.push(j);
        after = mine;
        after_exists = v.existed;
    }
    json!({
        "id": key,
        "rel": ix.rel,
        "name": path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "area": area(&ix.rel),
        "exists": current.is_some(),
        "size": current.as_ref().map(|c| c.len()).unwrap_or(0),
        "text": current.as_deref().map(is_text).unwrap_or(true),
        "default": def.as_ref().map(|d| d.0),
        "changed": def.as_ref().map(|d| differs(path, &d.1)).unwrap_or(false),
        "defaultGone": def.as_ref().is_some_and(|d| d.1.is_none()),
        "versions": versions,
    })
}

#[derive(Deserialize)]
struct FilesQ {
    /// comma-separated path patterns (see path_matches)
    #[serde(default, rename = "match")]
    pattern: String,
    #[serde(default)]
    tool: String,
}

async fn api_files(State(st): State<Shared>, Query(q): Query<FilesQ>) -> Response {
    let s = ready(&st);
    let res = crate::setup_api::resources_for(&st).ok();
    blocking(move || {
        let pats: Vec<&str> = q.pattern.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
        let tools: Vec<&str> = q.tool.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
        let files: Vec<Value> = s
            .all()
            .into_iter()
            .filter(|(_, ix)| pats.iter().any(|p| path_matches(p, &ix.rel)) || ix.versions.iter().any(|v| tools.contains(&v.tool.as_str())))
            .take(200)
            .map(|(k, ix)| file_json(&s, res.as_deref(), &k, &ix, true))
            .collect();
        Ok(json!({"files": files}))
    })
    .await
}

#[derive(Deserialize)]
struct VersionQ {
    id: String,
    seq: u64,
}

/// A version's text and the file's text now, for comparing.
async fn api_version(State(st): State<Shared>, Query(q): Query<VersionQ>) -> Response {
    let s = ready(&st);
    blocking(move || {
        let ix = s.index(&q.id).ok_or(GONE)?;
        let then = s.version_bytes(&q.id, q.seq)?;
        let now = std::fs::read(&ix.path).ok();
        let text = |b: &Option<Vec<u8>>| b.as_deref().filter(|b| is_text(b)).map(latin1);
        let binary = then.as_deref().is_some_and(|b| !is_text(b)) || now.as_deref().is_some_and(|b| !is_text(b));
        Ok(json!({"then": text(&then), "now": text(&now), "thenExists": then.is_some(), "nowExists": now.is_some(), "binary": binary}))
    })
    .await
}

#[derive(Deserialize)]
struct RestoreReq {
    id: String,
    seq: u64,
}

async fn api_restore(State(st): State<Shared>, h: HeaderMap, Json(r): Json<RestoreReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let s = ready(&st);
    blocking(move || {
        let ix = s.index(&r.id).ok_or(GONE)?;
        let seq = s.restore(&r.id, r.seq, "restore")?;
        let name = Path::new(&ix.path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Ok(json!({"ok": true, "undo": seq, "message": format!("{name} is back the way it was. Its version from just now is kept too, if you change your mind.")}))
    })
    .await
}

#[derive(Deserialize)]
struct DefaultsReq {
    /// kept files (ids) and shipped files not kept yet (paths from /changed) to put back
    #[serde(default)]
    ids: Vec<String>,
}

/// Puts Loner-Strike's default back for each file.
async fn api_defaults(State(st): State<Shared>, h: HeaderMap, Json(r): Json<DefaultsReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let s = ready(&st);
    let res = crate::setup_api::resources_for(&st).ok();
    let game = st.local_game();
    blocking(move || {
        let mut done = Vec::new();
        let mut failed = Vec::new();
        for id in r.ids.iter().take(2000) {
            let out = if let Some(ix) = s.index(id) {
                match default_of(&s, res.as_deref(), id, &ix) {
                    Some((_, want)) if differs(Path::new(&ix.path), &want) => s.put_key(id, want.as_deref(), "defaults", "Before restoring Loner-Strike's default").map(|_| ix.rel.clone()),
                    Some(_) => continue,
                    None => Err(format!("{} has no default to go back to.", ix.rel)),
                }
            } else if let Some(rel) = id.strip_prefix("shipped:") {
                // a shipped settings file nobody has changed through the app yet
                let (Some(g), Some(res)) = (&game, &res) else { failed.push("Set up the game first.".to_string()); continue };
                match crate::setup::record::inside(&g.dir, &format!("{GM}{rel}")) {
                    Some(p) => match shipped(res, &p) {
                        Some(b) => s.save("defaults", "Before restoring Loner-Strike's default", &p, &b).map(|_| rel.to_string()),
                        None => Err(format!("Loner-Strike doesn't ship {rel}.")),
                    },
                    None => Err("That isn't a settings file.".into()),
                }
            } else {
                Err(GONE.to_string())
            };
            match out {
                Ok(rel) => done.push(rel),
                Err(e) => failed.push(e),
            }
        }
        let msg = match (done.len(), failed.first()) {
            (0, Some(e)) => return Err(e.clone()),
            (0, None) => "Everything was already the way Loner-Strike ships it.".to_string(),
            (1, None) => format!("{} is back to Loner-Strike's default.", done[0]),
            (n, None) => format!("{n} files are back to Loner-Strike's defaults."),
            (n, Some(e)) => format!("{n} files are back to Loner-Strike's defaults. {} couldn't be: {e}", failed.len()),
        };
        Ok(json!({"ok": true, "restored": done, "message": msg}))
    })
    .await
}

/// Everything that's different from Loner-Strike's defaults, for Settings.
async fn api_changed(State(st): State<Shared>) -> Response {
    let s = ready(&st);
    let res = crate::setup_api::resources_for(&st).ok();
    let game = st.local_game().filter(|g| g.remote.is_none());
    blocking(move || {
        let mut files: Vec<Value> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (k, ix) in s.all() {
            seen.insert(ix.path.clone());
            let j = file_json(&s, res.as_deref(), &k, &ix, false);
            if j["changed"] == json!(true) {
                files.push(j);
            }
        }
        // shipped settings files changed some other way (by hand, or before backups)
        if let (Some(g), Some(res)) = (&game, &res) {
            let src = res.join("plugins").join("configs").join("gamemodes");
            for (_, rel) in crate::setup::foundation::tree(&src) {
                let Some(p) = crate::setup::record::inside(&g.dir, &format!("{GM}{rel}")) else { continue };
                let full = resolve(&p).unwrap_or(p.clone());
                if seen.contains(&full.to_string_lossy().to_string()) || !p.is_file() {
                    continue;
                }
                let Some(want) = shipped(res, &p) else { continue };
                if differs(&p, &Some(want)) {
                    let full_rel = format!("{}/{GM}{rel}", g.name);
                    files.push(json!({"id": format!("shipped:{rel}"), "rel": full_rel, "name": rel.rsplit('/').next().unwrap_or(&rel), "area": area(&full_rel), "exists": true, "default": "shipped", "changed": true, "versions": []}));
                }
            }
        }
        let (bytes, kept, versions) = s.usage();
        Ok(json!({"files": files, "usage": {"bytes": bytes, "files": kept, "versions": versions, "cap": s.cap}}))
    })
    .await
}

#[derive(Deserialize)]
struct UndoReq {
    from: u64,
    to: u64,
}

async fn api_undo(State(st): State<Shared>, h: HeaderMap, Json(r): Json<UndoReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let s = ready(&st);
    blocking(move || {
        if r.to < r.from || r.to - r.from > 600_000_000 {
            return Err("That save is too old to undo from here. Use History to put back a version.".into());
        }
        let done = s.undo(r.from, r.to)?;
        Ok(json!({"ok": true, "restored": done, "message": if done.len() == 1 { format!("Undone: {} is back the way it was.", done[0]) } else { format!("Undone: {} files are back the way they were.", done.len()) }}))
    })
    .await
}

async fn api_clear(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let s = ready(&st);
    blocking(move || {
        let freed = s.clear_old();
        let (bytes, files, versions) = s.usage();
        Ok(json!({"ok": true, "freed": freed, "usage": {"bytes": bytes, "files": files, "versions": versions, "cap": s.cap}, "message": format!("Cleared {:.1} MB of old backups. Every file still has its original and its newest version.", freed as f64 / 1e6)}))
    })
    .await
}

/// Marks a POST that kept versions with `X-LS-Undo: <from>-<to>`, so the page can offer "Undo".
pub async fn mark_saves(req: Request, next: Next) -> Response {
    let tracked = req.method() == Method::POST && !req.uri().path().starts_with("/api/safety/");
    if !tracked {
        return next.run(req).await;
    }
    let before = store().last_seq();
    let mut res = next.run(req).await;
    let after = store().last_seq();
    if after > before {
        if let Ok(v) = HeaderValue::from_str(&format!("{before}-{after}")) {
            res.headers_mut().insert("x-ls-undo", v);
        }
    }
    res
}

/// The app window's listener only.
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/safety/files", get(api_files))
        .route("/api/safety/version", get(api_version))
        .route("/api/safety/changed", get(api_changed))
        .route("/api/safety/restore", post(api_restore))
        .route("/api/safety/defaults", post(api_defaults))
        .route("/api/safety/undo", post(api_undo))
        .route("/api/safety/clear", post(api_clear))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        root: PathBuf,
        hl: PathBuf,
        s: Store,
    }

    impl Drop for Fx {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fx(name: &str, keep: usize, cap: u64) -> Fx {
        let root = std::env::temp_dir().join(format!("ls-safety-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("valve")).unwrap();
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        let s = Store::with_limits(root.join("app/backups"), keep, cap);
        Fx { root, hl, s }
    }

    #[test]
    fn save_and_restore_round_trip() {
        let f = fx("round", KEEP, CAP);
        let p = f.hl.join("cstrike/userconfig.cfg");
        // a new file: its version says it wasn't there
        let s1 = f.s.save("crosshair", "Saved the crosshair", &p, b"cl_crosshair_color 255\n").unwrap();
        let s2 = f.s.save("crosshair", "Saved the crosshair", &p, b"cl_crosshair_color 50\n").unwrap();
        assert!(s2 > s1);
        let key = f.s.key_for(&p).unwrap();
        let ix = f.s.index(&key).unwrap();
        assert_eq!(ix.rel, "cstrike/userconfig.cfg");
        assert_eq!(ix.versions.len(), 2);
        assert!(!ix.versions[0].existed && ix.versions[0].original);
        assert_eq!(f.s.version_bytes(&key, s2).unwrap().unwrap(), b"cl_crosshair_color 255\n");
        // put the first back, then undo that restore
        let r = f.s.restore(&key, s2, "restore").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"cl_crosshair_color 255\n");
        f.s.restore(&key, r, "restore").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"cl_crosshair_color 50\n");
        // the original (no file) deletes it
        f.s.restore(&key, s1, "restore").unwrap();
        assert!(!p.exists());
        // and that's undoable too
        let last = f.s.index(&key).unwrap().versions.last().unwrap().seq;
        f.s.restore(&key, last, "restore").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"cl_crosshair_color 50\n");
        // no stray temporary files beside it
        let names: Vec<String> = std::fs::read_dir(p.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn remove_and_undo() {
        let f = fx("undo", KEEP, CAP);
        let a = f.hl.join("cstrike/maps/de_x.nav");
        let b = f.hl.join("cstrike/maps/de_x.lsv");
        std::fs::write(&a, b"nav one").unwrap();
        std::fs::write(&b, b"lsv one").unwrap();
        let from = f.s.last_seq();
        f.s.save("nav", "Saved the nav", &a, b"nav two").unwrap();
        f.s.save("nav", "Saved the nav", &a, b"nav three").unwrap();
        assert_eq!(f.s.remove("nav", "Old knowledge", &b).unwrap().is_some(), true);
        assert!(!b.exists());
        let to = f.s.last_seq();
        let mut done = f.s.undo(from, to).unwrap();
        done.sort();
        assert_eq!(done, vec!["cstrike/maps/de_x.lsv", "cstrike/maps/de_x.nav"]);
        // back to before the first change in the range, not the middle one
        assert_eq!(std::fs::read(&a).unwrap(), b"nav one");
        assert_eq!(std::fs::read(&b).unwrap(), b"lsv one");
        assert!(f.s.remove("nav", "x", &f.hl.join("cstrike/maps/none.nav")).unwrap().is_none());
    }

    #[test]
    fn prunes_but_keeps_the_original() {
        let f = fx("prune", 3, CAP);
        let p = f.hl.join("cstrike/maps/de_p.res");
        std::fs::write(&p, b"v0").unwrap();
        for i in 1..=10 {
            f.s.save("doctor", "x", &p, format!("v{i}").as_bytes()).unwrap();
        }
        let key = f.s.key_for(&p).unwrap();
        let ix = f.s.index(&key).unwrap();
        assert_eq!(ix.versions.len(), 4);
        assert!(ix.versions[0].original);
        assert_eq!(f.s.version_bytes(&key, ix.versions[0].seq).unwrap().unwrap(), b"v0");
        assert_eq!(f.s.version_bytes(&key, ix.versions[3].seq).unwrap().unwrap(), b"v9");
        // blobs of pruned versions are gone
        assert_eq!(std::fs::read_dir(f.s.dir(&key).join("blobs")).unwrap().count(), 4);
        // clear: original and newest stay
        assert!(f.s.clear_old() > 0);
        let ix = f.s.index(&key).unwrap();
        assert_eq!(ix.versions.len(), 2);
        assert!(ix.versions[0].original);
    }

    #[test]
    fn stays_under_the_cap() {
        let f = fx("cap", KEEP, 3_000);
        let a = f.hl.join("cstrike/a.wad");
        let b = f.hl.join("cstrike/b.wad");
        for i in 0..8u8 {
            f.s.save("textures", "x", &a, &vec![i; 1000]).unwrap();
            f.s.save("textures", "x", &b, &vec![i + 100; 1000]).unwrap();
        }
        let (bytes, files, _) = f.s.usage();
        assert_eq!(files, 2);
        assert!(bytes <= 3_000 + 1000, "{bytes}");
        // each still has its newest version and the original (no file at first)
        for p in [&a, &b] {
            let ix = f.s.index(&f.s.key_for(p).unwrap()).unwrap();
            assert!(ix.versions[0].original && !ix.versions[0].existed);
            let newest = ix.versions.last().unwrap();
            assert!(f.s.version_bytes(&f.s.key_for(p).unwrap(), newest.seq).unwrap().is_some());
        }
    }

    #[test]
    fn only_inside_game_folders() {
        let f = fx("paths", KEEP, CAP);
        let outside = f.root.join("elsewhere/x.cfg");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        assert!(f.s.save("t", "x", &outside, b"x").is_err());
        assert!(!outside.exists());
        assert!(f.s.save("t", "x", Path::new("cstrike/x.cfg"), b"x").is_err());
        assert!(f.s.save("t", "x", &f.hl.join("cstrike/../../elsewhere/x.cfg"), b"x").is_err());
        // the backups themselves
        std::fs::create_dir_all(f.s.root()).unwrap();
        assert!(f.s.save("t", "x", &f.s.root().join("x"), b"x").is_err());
        // a registered folder is fine
        f.s.allow_root(&f.root.join("elsewhere"));
        assert!(f.s.save("t", "x", &outside, b"x").is_ok());
        // a folder itself isn't a file
        assert!(f.s.save("t", "x", &f.hl.join("cstrike/maps"), b"x").is_err());
        #[cfg(unix)]
        {
            let target = f.root.join("secret.txt");
            std::fs::write(&target, b"secret").unwrap();
            let link = f.hl.join("cstrike/link.cfg");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(f.s.save("t", "x", &link, b"x").is_err());
            assert_eq!(std::fs::read(&target).unwrap(), b"secret");
            // a folder shortcut out of the game folder
            let out = f.root.join("outdir");
            std::fs::create_dir_all(&out).unwrap();
            std::os::unix::fs::symlink(&out, f.hl.join("cstrike/out")).unwrap();
            assert!(f.s.save("t", "x", &f.hl.join("cstrike/out/x.cfg"), b"x").is_err());
            assert!(!out.join("x.cfg").exists());
        }
        // keys can't leave the store
        assert!(!valid_key("../x") && !valid_key("a/b") && !valid_key("") && valid_key("de_x.nav-0123abcd"));
        assert!(f.s.version_bytes("../../etc", 1).is_err());
    }

    #[test]
    fn two_writers_at_once() {
        let f = Arc::new(fx("concurrent", 100, CAP));
        let p = f.hl.join("cstrike/people.ini");
        std::fs::write(&p, b"start").unwrap();
        let mut hs = Vec::new();
        for t in 0..2 {
            let f = f.clone();
            let p = p.clone();
            hs.push(std::thread::spawn(move || {
                for i in 0..25 {
                    f.s.save(&format!("w{t}"), "x", &p, format!("writer {t} save {i}").as_bytes()).unwrap();
                }
            }));
        }
        for h in hs {
            h.join().unwrap();
        }
        let key = f.s.key_for(&p).unwrap();
        let ix = f.s.index(&key).unwrap();
        assert_eq!(ix.versions.len(), 50);
        // numbers are unique and in order, every blob is readable and whole
        assert!(ix.versions.windows(2).all(|w| w[0].seq < w[1].seq));
        for v in &ix.versions {
            f.s.version_bytes(&key, v.seq).unwrap().unwrap();
        }
        assert_eq!(f.s.version_bytes(&key, ix.versions[0].seq).unwrap().unwrap(), b"start");
        let now = std::fs::read_to_string(&p).unwrap();
        assert!(now.starts_with("writer "), "{now}");
        f.s.forget(&key);
        assert!(f.s.index(&key).is_none());
    }

    #[test]
    fn shipped_defaults() {
        let f = fx("defaults", KEEP, CAP);
        let res = f.root.join("res");
        let src = res.join("plugins/configs/gamemodes");
        std::fs::create_dir_all(src.join("maps")).unwrap();
        std::fs::write(src.join("people.ini"), b"; people\r\nshipped\r\n").unwrap();
        std::fs::write(src.join("modes.ini"), b"; modes\r\nclassic \"Classic\"\r\nzombie \"Zombies\"\r\nknife \"Knife\"\r\n").unwrap();
        let gm = f.hl.join("cstrike").join(GM);
        std::fs::create_dir_all(&gm).unwrap();
        let people = gm.join("people.ini");
        std::fs::write(&people, b"; people\r\nmine\r\n").unwrap();
        assert_eq!(shipped(&res, &people).unwrap(), b"; people\r\nshipped\r\n");
        // modes.ini only lists the modes installed (zombie isn't)
        let modes = shipped(&res, &gm.join("modes.ini")).unwrap();
        assert_eq!(String::from_utf8(modes).unwrap(), "; modes\r\nclassic \"Classic\"\r\nknife \"Knife\"\r\n");
        std::fs::write(gm.join("zombie.cfg"), b"").unwrap();
        assert!(String::from_utf8(shipped(&res, &gm.join("modes.ini")).unwrap()).unwrap().contains("zombie"));
        assert!(shipped(&res, &f.hl.join("cstrike/userconfig.cfg")).is_none());
        assert!(shipped(&res, &gm.join("nothere.ini")).is_none());
        // the people tool saves; the default is the shipped one, and putting it back is undoable
        f.s.save("people", "Saved", &people, b"; people\r\nedited\r\n").unwrap();
        let key = f.s.key_for(&people).unwrap();
        let ix = f.s.index(&key).unwrap();
        let (kind, want) = default_of(&f.s, Some(&res), &key, &ix).unwrap();
        assert_eq!(kind, DefaultKind::Shipped);
        assert!(differs(&people, &want));
        f.s.put_key(&key, want.as_deref(), "defaults", "x").unwrap();
        assert_eq!(std::fs::read(&people).unwrap(), b"; people\r\nshipped\r\n");
        assert!(!differs(&people, &want));
        // line ends alone aren't a change
        std::fs::write(&people, b"; people\nshipped\n").unwrap();
        assert!(!differs(&people, &want));
        // a file Loner-Strike doesn't ship: its default is how it was first (here: not there)
        let cfg = f.hl.join("cstrike/userconfig.cfg");
        f.s.save("crosshair", "x", &cfg, b"x").unwrap();
        let k2 = f.s.key_for(&cfg).unwrap();
        let (kind, want) = default_of(&f.s, Some(&res), &k2, &f.s.index(&k2).unwrap()).unwrap();
        assert_eq!((kind, want), (DefaultKind::Original, None));
        f.s.put_key(&k2, None, "defaults", "x").unwrap();
        assert!(!cfg.exists());
    }

    #[test]
    fn hooks_keep_into_the_app_store() {
        let f = fx("hooks", KEEP, CAP);
        let p = f.hl.join("cstrike/userconfig.cfg");
        std::fs::write(&p, b"old").unwrap();
        let seq = before("Crosshair & config", "Saved userconfig.cfg", &p).unwrap();
        std::fs::write(&p, b"new").unwrap();
        let s = store();
        let key = s.key_for(&p).unwrap();
        assert_eq!(s.version_bytes(&key, seq).unwrap().unwrap(), b"old");
        // a file the player picked outside the game folders: left alone, the save goes on
        assert!(before("Bot people", "Exported", &f.root.join("people.ini")).is_none());
        let from = seq - 1;
        assert_eq!(s.undo(from, s.last_seq()).unwrap(), vec!["cstrike/userconfig.cfg"]);
        assert_eq!(std::fs::read(&p).unwrap(), b"old");
    }

    #[test]
    fn small_helpers() {
        assert!(path_matches("maps/cs_rats2.*", "cstrike/maps/cs_rats2.nav"));
        assert!(path_matches("MAPS/CS_RATS2.NAV", "cstrike/maps/cs_rats2.nav"));
        assert!(!path_matches("maps/cs_rats2.*", "cstrike/maps/cs_rats22/x.nav"));
        assert!(!path_matches("aps/cs_rats2.nav", "cstrike/maps/cs_rats2.nav"));
        assert!(path_matches("*.wad", "cstrike/pldecal.wad"));
        assert!(path_matches("cstrike/maps/*", "cstrike/maps/x.bsp"));
        assert!(!path_matches("", "cstrike/maps/x.bsp"));
        assert_eq!(line_change(b"a\nb\nc\n", b"a\nc\nd\ne\n"), Some((2, 1)));
        assert_eq!(line_change(b"a\0", b"a"), None);
        assert!(same_text(b"a\r\nb\r\n", b"a\nb"));
        assert!(!same_text(b"a\nb", b"a\nc"));
        assert_eq!(area("cstrike/addons/amxmodx/configs/gamemodes/people.ini"), "Bot people");
        assert_eq!(area("cstrike/maps/de_dust2.nav"), "Maps & nav");
        assert_eq!(area("cstrike/addons/amxmodx/configs/plugins-studio.ini"), "Plugins");
        assert_eq!(area("cstrike/addons/amxmodx/configs/gamemodes/knife.cfg"), "Game modes");
        assert_eq!(area("cstrike/pldecal.wad"), "Sprays & textures");
        assert_eq!(area("cstrike/userconfig.cfg"), "Settings files");
    }
}
