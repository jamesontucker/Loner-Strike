//! The Tools cache: processed map files (mesh bundle, lightmap, textures, info) kept on
//! disk so a map opens fast the second time.
//!
//! Layout: `<app cache>/tools/<map, lower case>/<key>/<file>`, where the key is the map
//! file's size and modified time plus the parser version. A newer key for the same map
//! removes the older ones. Results that also depend on texture files (textures, info) put a
//! stamp of those in their file name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

/// Where the Tools cache lives.
pub fn root() -> PathBuf {
    if cfg!(test) {
        // tests never touch the real cache
        return std::env::temp_dir().join(format!("ls-tools-cache-test-{}", std::process::id()));
    }
    crate::sys::cache_root().join("tools")
}

/// A file's version as far as the cache cares: size and modified time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stamp {
    pub size: u64,
    pub mtime: u64,
}

impl Stamp {
    pub fn of(path: &Path) -> std::io::Result<Stamp> {
        let m = std::fs::metadata(path)?;
        let mtime = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        Ok(Stamp { size: m.len(), mtime })
    }

    /// The cache key for this file version and the current parsers.
    pub fn key(&self) -> String {
        format!("{}-{}-p{}", self.size, self.mtime, crate::goldsrc::PARSER_VERSION)
    }
}

/// The folder for one version of a map's processed files.
pub fn map_dir(root: &Path, map: &str, stamp: &Stamp) -> PathBuf {
    root.join(map.to_lowercase()).join(stamp.key())
}

/// A short, stable hash of some strings (texture-file stamps, ETags).
pub fn short_hash(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0]);
    }
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// The ETag for a cached file: its folder key and name (the folder changes with the map
/// file, the name with the texture files).
pub fn etag(dir: &Path, file: &str) -> String {
    format!("\"{}\"", short_hash(&[&dir.to_string_lossy(), file]))
}

/// A file name for any text (texture names hold `{ ! + ~`, and `*` on some maps, which
/// Windows doesn't allow): hex of its lower-case bytes.
pub fn safe_file(name: &str, ext: &str) -> String {
    let hex: String = name.to_lowercase().bytes().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.{ext}")
}

fn lock_for(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut m = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if m.len() > 256 {
        m.retain(|_, l| Arc::strong_count(l) > 1);
    }
    m.entry(dir.to_path_buf()).or_default().clone()
}

/// `files[want]` from `dir`, building all of `files` together when it isn't there (one build
/// at a time per folder; others asking meanwhile wait and then read). Saving is best effort:
/// a full disk still answers. Starting a new version of a map clears its older ones.
pub fn get_or_build<E>(dir: &Path, files: &[&str], want: usize, build: impl FnOnce() -> Result<Vec<Vec<u8>>, E>) -> Result<Vec<u8>, E> {
    let lock = lock_for(dir);
    let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(b) = std::fs::read(dir.join(files[want])) {
        return Ok(b);
    }
    let built = build()?;
    if !dir.is_dir() {
        prune_others(dir);
    }
    if std::fs::create_dir_all(dir).is_ok() {
        for (name, bytes) in files.iter().zip(&built) {
            crate::sys::write_atomic(&dir.join(name), bytes).ok();
        }
    }
    Ok(built.into_iter().nth(want).unwrap_or_default())
}

/// Removes the other versions next to `dir` (same map, older file or parser).
fn prune_others(dir: &Path) {
    let (Some(parent), Some(own)) = (dir.parent(), dir.file_name()) else { return };
    for e in std::fs::read_dir(parent).into_iter().flatten().flatten() {
        if e.file_name() != own && e.path().is_dir() {
            std::fs::remove_dir_all(e.path()).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_once_and_prunes_old_versions() {
        let root = std::env::temp_dir().join(format!("ls-tools-cache-{}", std::process::id()));
        let old = map_dir(&root, "De_Dust2", &Stamp { size: 1, mtime: 2 });
        let new = map_dir(&root, "de_dust2", &Stamp { size: 3, mtime: 4 });
        assert!(old.ends_with(format!("de_dust2/1-2-p{}", crate::goldsrc::PARSER_VERSION)));
        let mut calls = 0;
        let mut get = |dir: &Path, want: usize| {
            get_or_build::<String>(dir, &["a", "b"], want, || {
                calls += 1;
                Ok(vec![b"A".to_vec(), b"B".to_vec()])
            })
            .unwrap()
        };
        assert_eq!(get(&old, 0), b"A");
        assert_eq!(get(&old, 1), b"B");
        assert_eq!(get(&new, 1), b"B");
        drop(get);
        assert_eq!(calls, 2);
        assert!(!old.exists() && new.join("a").is_file());
        // a failed build saves nothing
        let other = map_dir(&root, "x", &Stamp { size: 0, mtime: 0 });
        assert_eq!(get_or_build::<&str>(&other, &["a"], 0, || Err("no")), Err("no"));
        assert!(!other.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn names_and_tags() {
        assert_eq!(safe_file("{Fence", "png"), "7b66656e6365.png");
        assert_eq!(short_hash(&["a", "b"]).len(), 16);
        assert_ne!(short_hash(&["ab", ""]), short_hash(&["a", "b"]));
        let d = Path::new("/x/y");
        assert!(etag(d, "mesh").starts_with('"'));
        assert_ne!(etag(d, "mesh"), etag(d, "lightmap"));
    }
}
