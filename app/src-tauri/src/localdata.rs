//! Where the app's big folders live. On Windows the app's folder (`sys::app_dir()`) is in
//! Roaming AppData, which a domain profile copies to a server at every sign-in and sign-out,
//! and which some backup tools treat as settings. Downloads, voice and chat models, steamcmd,
//! backups and compile work are gigabytes, so they live in Local AppData
//! (`%LOCALAPPDATA%\gg.lonerstrike.app\data`); small settings and records stay in Roaming.
//! On the Mac and Linux both are the same folder and nothing moves.
//!
//! Earlier versions kept everything in Roaming: `start()` moves the big folders over once.
//! A plain rename when both are on one drive (instant); otherwise a copy in the background,
//! checked byte for byte, and only then the Roaming copy removed. Until a folder's copy is
//! checked the app keeps using the Roaming one, so nothing is half there. An interrupted move
//! picks up at the next start; a file that differs between the two is never thrown away
//! (the Local one is kept in place, the Roaming one goes to `data\from-roaming`).

use std::path::{Path, PathBuf};

/// The big folders, relative to the app's folder.
pub const BIG: &[&str] = &[
    "backups",
    "cache",
    "control-log",
    "editor/backups",
    "smarter-chat",
    "steamcmd",
    "voice",
    "tools/community/packages",
    "tools/compile",
    "tools/recordings",
    "tools/sdhlt",
    "tools/wad-backups",
];

/// Where the big folders go: Local AppData on Windows, the app's folder elsewhere (and in
/// test builds pointed at a scratch folder).
pub fn local_root() -> PathBuf {
    #[cfg(test)]
    if std::env::var_os("LS_APP_DIR").is_some() {
        return crate::sys::app_dir();
    }
    if cfg!(windows) {
        if let Some(d) = dirs::data_local_dir() {
            return d.join("gg.lonerstrike.app").join("data");
        }
    }
    crate::sys::app_dir()
}

/// One of the big folders (`rel` from `BIG`, with `/`), wherever it is right now.
pub fn dir(rel: &str) -> PathBuf {
    pick(&crate::sys::app_dir(), &local_root(), rel)
}

/// `base.join(rel)`, sent to Local AppData when that's one of the big folders of the app's
/// own folder. Any other base (a dev server's scratch app folder, a project folder) is
/// joined as it is.
pub fn join(base: &Path, rel: &str) -> PathBuf {
    let app = crate::sys::app_dir();
    let Ok(under) = base.strip_prefix(&app) else { return base.join(rel) };
    let mut full: Vec<String> = under.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    full.extend(rel.split('/').map(str::to_string));
    let full = full.join("/");
    match BIG.iter().find(|b| full == **b || full.starts_with(&format!("{b}/"))) {
        Some(b) => at(&pick(&app, &local_root(), b), full[b.len()..].trim_start_matches('/')),
        None => base.join(rel),
    }
}

/// The Local copy once it exists (or when there's nothing in Roaming); the Roaming one while
/// it's still the only one.
fn pick(roaming: &Path, local: &Path, rel: &str) -> PathBuf {
    let l = at(local, rel);
    if roaming == local {
        return l;
    }
    let r = at(roaming, rel);
    if l.exists() || !r.exists() {
        l
    } else {
        r
    }
}

fn at(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|s| !s.is_empty()).fold(root.to_path_buf(), |p, s| p.join(s))
}

/// Move the big folders out of Roaming. The renames happen here, before anything uses the
/// folders; copies (Roaming on another drive or a server) run in the background.
pub fn start() {
    let (roaming, local) = (crate::sys::app_dir(), local_root());
    if roaming == local || !roaming.is_dir() {
        return;
    }
    let left = rename_all(&roaming, &local);
    if left.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Some(_lock) = Lock::take(&local) else { return };
        for rel in left {
            if let Err(e) = migrate_one(&roaming, &local, rel) {
                crate::diagnostics::log_error("localdata", &format!("Moving {rel} to Local AppData: {e}"));
            }
        }
    });
}

/// The quick part: each big folder that's only in Roaming is renamed into Local. Returns the
/// ones still to do (a rename across drives fails, and a folder in both places needs a merge).
fn rename_all(roaming: &Path, local: &Path) -> Vec<&'static str> {
    let mut left = Vec::new();
    for rel in BIG {
        let (r, l) = (at(roaming, rel), at(local, rel));
        if !r.exists() {
            continue;
        }
        if !l.exists() {
            if let Some(parent) = l.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::rename(&r, &l).is_ok() {
                continue;
            }
        }
        left.push(*rel);
    }
    left
}

/// The slow part for one folder: copy, check, swap in, then remove the Roaming copy. Safe to
/// stop at any point: Roaming is only removed after the Local copy is checked and in place.
fn migrate_one(roaming: &Path, local: &Path, rel: &str) -> Result<(), String> {
    let (r, l) = (at(roaming, rel), at(local, rel));
    if !r.exists() {
        return Ok(());
    }
    if !l.exists() {
        let partial = l.with_file_name(format!("{}.moving", l.file_name().unwrap_or_default().to_string_lossy()));
        // a copy from an interrupted start: Roaming is still whole, so start again
        if partial.exists() {
            std::fs::remove_dir_all(&partial).map_err(|e| format!("couldn't clear the unfinished copy: {e}"))?;
        }
        copy_tree(&r, &partial)?;
        if let Err(e) = same_tree(&r, &partial) {
            let _ = std::fs::remove_dir_all(&partial);
            return Err(format!("the copy didn't match ({e}); Roaming kept"));
        }
        std::fs::rename(&partial, &l).map_err(|e| format!("couldn't put the copy in place: {e}"))?;
    }
    merge(&r, &l, &at(local, "from-roaming").join(rel.replace('/', "_")))?;
    let _ = remove_empty_dirs(&r);
    Ok(())
}

/// Everything left in Roaming `r` goes into Local `l`: a file that's the same in both is
/// removed from Roaming, one Local lacks is copied, checked and removed, and one that
/// differs is moved to `conflicts` (Local's own stays where it is).
fn merge(r: &Path, l: &Path, conflicts: &Path) -> Result<(), String> {
    let mut failed = Vec::new();
    for (sub, is_file) in walk(r)? {
        if !is_file {
            continue;
        }
        let (src, dst) = (r.join(&sub), l.join(&sub));
        let to = if !dst.exists() {
            dst
        } else if same_file(&src, &dst).unwrap_or(false) {
            if std::fs::remove_file(&src).is_err() {
                failed.push(sub.display().to_string());
            }
            continue;
        } else {
            conflicts.join(&sub)
        };
        let ok = to.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok())
            && std::fs::copy(&src, &to).is_ok()
            && same_file(&src, &to).unwrap_or(false);
        if !ok || std::fs::remove_file(&src).is_err() {
            failed.push(sub.display().to_string());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} file(s) stay in Roaming for the next start: {}", failed.len(), failed.into_iter().take(5).collect::<Vec<_>>().join(", ")))
    }
}

/// Every entry under `root` (relative path, is a file). A link isn't followed or copied: it
/// makes the move fail, so Roaming is kept.
fn walk(root: &Path) -> Result<Vec<(PathBuf, bool)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(sub) = stack.pop() {
        let rd = std::fs::read_dir(root.join(&sub)).map_err(|e| format!("{}: {e}", root.join(&sub).display()))?;
        for e in rd {
            let e = e.map_err(|e| e.to_string())?;
            let t = e.file_type().map_err(|e| e.to_string())?;
            let rel = sub.join(e.file_name());
            if t.is_symlink() {
                return Err(format!("{} is a link", rel.display()));
            } else if t.is_dir() {
                out.push((rel.clone(), false));
                stack.push(rel);
            } else {
                out.push((rel, true));
            }
        }
    }
    Ok(out)
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for (sub, is_file) in walk(from)? {
        let dst = to.join(&sub);
        if is_file {
            std::fs::copy(from.join(&sub), &dst).map_err(|e| format!("{}: {e}", sub.display()))?;
        } else {
            std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The copy holds every file of the original with the same bytes.
fn same_tree(a: &Path, b: &Path) -> Result<(), String> {
    for (sub, is_file) in walk(a)? {
        let ok = if is_file { same_file(&a.join(&sub), &b.join(&sub)).unwrap_or(false) } else { b.join(&sub).is_dir() };
        if !ok {
            return Err(format!("{} differs", sub.display()));
        }
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let (ma, mb) = (std::fs::metadata(a)?, std::fs::metadata(b)?);
    if ma.len() != mb.len() || !mb.is_file() {
        return Ok(false);
    }
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    let (mut ba, mut bb) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let n = fa.read(&mut ba)?;
        if n == 0 {
            return Ok(true);
        }
        fb.read_exact(&mut bb[..n])?;
        if ba[..n] != bb[..n] {
            return Ok(false);
        }
    }
}

/// Removes `dir` and the folders under it that are empty (files are left alone).
fn remove_empty_dirs(dir: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)?.flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = remove_empty_dirs(&e.path());
        }
    }
    std::fs::remove_dir(dir)
}

/// One copy of the app moves at a time (a second launch before the first opened its window).
struct Lock(PathBuf);

impl Lock {
    fn take(local: &Path) -> Option<Lock> {
        let _ = std::fs::create_dir_all(local);
        let p = local.join(".moving.lock");
        // one left by a copy that was ended part way
        let stale = std::fs::metadata(&p).and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|e| e.as_secs() > 6 * 3600));
        if stale {
            let _ = std::fs::remove_file(&p);
        }
        std::fs::OpenOptions::new().write(true).create_new(true).open(&p).ok().map(|_| Lock(p))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(root: &Path, files: &[(&str, &str)]) {
        for (p, body) in files {
            let f = root.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, body).unwrap();
        }
    }

    fn scratch(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let d = std::env::temp_dir().join(format!("ls-localdata-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        (d.clone(), d.join("Roaming/Loner-Strike"), d.join("Local/gg.lonerstrike.app/data"))
    }

    #[test]
    fn big_folders_move_and_settings_stay() {
        let (d, r, l) = scratch("move");
        tree(&r, &[("settings.json", "{}"), ("servers.json", "[]"), ("voice/model.bin", "weights"), ("backups/auto/s1/a.zip", "zip"), ("editor/recent.json", "[]"), ("editor/backups/m~1.lsmap", "map"), ("tools/compile/made.json", "{}"), ("tools/studio/graphs/g.json", "{}")]);
        // what the app uses before the move: Roaming's
        assert_eq!(pick(&r, &l, "voice"), r.join("voice"));
        let left = rename_all(&r, &l);
        assert!(left.is_empty(), "{left:?}");
        assert_eq!(std::fs::read_to_string(l.join("voice/model.bin")).unwrap(), "weights");
        assert_eq!(std::fs::read_to_string(l.join("backups/auto/s1/a.zip")).unwrap(), "zip");
        assert_eq!(std::fs::read_to_string(l.join("editor/backups/m~1.lsmap")).unwrap(), "map");
        assert!(l.join("tools/compile/made.json").is_file());
        // small things stay in Roaming
        for keep in ["settings.json", "servers.json", "editor/recent.json", "tools/studio/graphs/g.json"] {
            assert!(r.join(keep).is_file(), "{keep}");
            assert!(!l.join(keep).exists(), "{keep}");
        }
        assert!(!r.join("voice").exists() && !r.join("editor/backups").exists());
        assert_eq!(pick(&r, &l, "voice"), l.join("voice"));
        // a folder that never existed is made in Local
        assert_eq!(pick(&r, &l, "steamcmd"), l.join("steamcmd"));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_copy_is_checked_before_roaming_goes() {
        let (d, r, l) = scratch("copy");
        tree(&r, &[("voice/a.bin", "aaaa"), ("voice/sub/b.bin", "bb"), ("voice/empty/.keep", "")]);
        // an unfinished copy from a start that was ended: thrown away and done again
        tree(&l, &[("voice.moving/a.bin", "a")]);
        migrate_one(&r, &l, "voice").unwrap();
        assert_eq!(std::fs::read_to_string(l.join("voice/a.bin")).unwrap(), "aaaa");
        assert_eq!(std::fs::read_to_string(l.join("voice/sub/b.bin")).unwrap(), "bb");
        assert!(l.join("voice/empty/.keep").is_file());
        assert!(!l.join("voice.moving").exists());
        assert!(!r.join("voice").exists(), "Roaming removed after the check");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_bad_copy_keeps_roaming() {
        let (d, r, l) = scratch("bad");
        tree(&r, &[("cache/x", "12345")]);
        let p = l.join("cache.moving");
        copy_tree(&r.join("cache"), &p).unwrap();
        std::fs::write(p.join("x"), "12399").unwrap();
        assert!(same_tree(&r.join("cache"), &p).is_err());
        std::fs::write(p.join("x"), "1234").unwrap();
        assert!(same_tree(&r.join("cache"), &p).is_err());
        std::fs::remove_file(p.join("x")).unwrap();
        assert!(same_tree(&r.join("cache"), &p).is_err());
        assert!(r.join("cache/x").is_file());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn an_interrupted_move_finishes_without_losing_anything() {
        let (d, r, l) = scratch("resume");
        // stopped after the copy went in place but before Roaming was emptied, and the app
        // has written to Local since
        tree(&r, &[("backups/same.zip", "same"), ("backups/only-roaming.zip", "old"), ("backups/changed.json", "roaming version")]);
        tree(&l, &[("backups/same.zip", "same"), ("backups/changed.json", "local version"), ("backups/new.zip", "new")]);
        assert_eq!(rename_all(&r, &l), vec!["backups"]);
        migrate_one(&r, &l, "backups").unwrap();
        assert_eq!(std::fs::read_to_string(l.join("backups/same.zip")).unwrap(), "same");
        assert_eq!(std::fs::read_to_string(l.join("backups/only-roaming.zip")).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(l.join("backups/changed.json")).unwrap(), "local version");
        assert_eq!(std::fs::read_to_string(l.join("from-roaming/backups/changed.json")).unwrap(), "roaming version");
        assert_eq!(std::fs::read_to_string(l.join("backups/new.zip")).unwrap(), "new");
        assert!(!r.join("backups").exists());
        // and again: nothing to do
        migrate_one(&r, &l, "backups").unwrap();
        std::fs::remove_dir_all(d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_stops_the_copy() {
        let (d, r, l) = scratch("link");
        tree(&r, &[("cache/a", "a")]);
        std::os::unix::fs::symlink("/etc", r.join("cache/etc")).unwrap();
        assert!(migrate_one(&r, &l, "cache").is_err());
        assert!(r.join("cache/a").is_file());
        assert!(!l.join("cache").exists());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn one_mover_at_a_time() {
        let (d, _, l) = scratch("lock");
        let a = Lock::take(&l).unwrap();
        assert!(Lock::take(&l).is_none());
        drop(a);
        assert!(Lock::take(&l).is_some());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn join_sends_only_big_folders_of_the_app_folder() {
        let app = crate::sys::app_dir();
        let other = std::env::temp_dir().join("ls-localdata-other-app");
        assert_eq!(join(&other, "cache"), other.join("cache"));
        assert_eq!(join(&app, "settings.json"), app.join("settings.json"));
        assert_eq!(join(&app.join("editor"), "backups"), dir("editor/backups"));
        assert_eq!(join(&app.join("tools"), "compile"), dir("tools/compile"));
        assert_eq!(join(&app, "tools/studio"), app.join("tools/studio"));
        assert_eq!(join(&app, "cache/downloads"), dir("cache").join("downloads"));
    }
}
