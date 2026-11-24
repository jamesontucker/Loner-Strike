//! Keeps Loner-Strike's fixed Ham Sandwich in place on this computer (agent MCP). The official
//! module uses freed memory when the map changes and crashes the game and servers
//! (resources/windows/hamsandwich/README.md). Setup puts the fixed one in, and a Linux server's
//! unit puts it back before every start (remote.rs); here the same for local play and local
//! Windows and Mac servers: before every Play and every local server start the module's
//! hash is checked, and when it isn't ours (an AMX Mod X update, a package copied in by hand)
//! the file found is kept once beside it (`.replaced-<date>`) and ours goes back.
//! Setup's own copies never put another Ham Sandwich over ours (`keep_ours`, used by
//! setup/record.rs).

use crate::crashes::{FIXED_HAM_LINUX, FIXED_HAM_WINDOWS};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Where the module lives in a game folder on this computer (the Windows build: games and
/// servers here and in CrossOver; the Linux build for the native Linux game).
#[allow(dead_code)] // (tests)
pub const REL: &str = crate::gamebin::HAM;
/// The module's file name, which a kept copy starts with
fn file_name_of(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}
#[cfg(test)]
fn file_name() -> &'static str {
    file_name_of(REL)
}

/// The fixed build's hash for a module path (.so: the Linux build)
fn fixed_hash(rel: &str) -> &'static str {
    if rel.ends_with(".so") { FIXED_HAM_LINUX } else { FIXED_HAM_WINDOWS }
}

/// The app's resources folder, set once at start (main.rs).
static RES: OnceLock<PathBuf> = OnceLock::new();

pub fn init(res: PathBuf) {
    let _ = RES.set(res);
}

/// The fixed DLL: the app's resources, else LONER_STRIKE_RESOURCES or the repo (dev builds).
#[cfg(test)]
fn fixed_dll() -> Option<PathBuf> {
    fixed_from(crate::gamebin::HAM_FIXED_RES)
}

fn fixed_from(res_rel: &str) -> Option<PathBuf> {
    let rel = Path::new(res_rel);
    let mut places: Vec<PathBuf> = RES.get().map(|r| r.join(rel)).into_iter().collect();
    places.extend(std::env::var_os("LONER_STRIKE_RESOURCES").map(|r| PathBuf::from(r).join(rel)));
    if cfg!(any(debug_assertions, test)) {
        places.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join(rel));
    }
    places.into_iter().find(|p| p.is_file())
}

fn sha256(p: &Path) -> Option<String> {
    let b = std::fs::read(p).ok()?;
    Some(Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect())
}

/// Is this path a Ham Sandwich module (Windows or Linux build)?
pub fn is_ham(rel: &str) -> bool {
    let name = rel.rsplit(['/', '\\']).next().unwrap_or(rel).to_ascii_lowercase();
    name == "hamsandwich_amxx.dll" || name == "hamsandwich_amxx_i386.so"
}

/// Does this file hold one of our fixed builds?
pub fn is_fixed(p: &Path) -> bool {
    sha256(p).is_some_and(|h| h == FIXED_HAM_WINDOWS || h == FIXED_HAM_LINUX)
}

/// For an install that's about to write `dst` (a Ham Sandwich module) from `src`: false when
/// that would put another build over ours (ours stays).
pub fn keep_ours(dst: &Path, src: &Path) -> bool {
    dst.is_file() && is_fixed(dst) && !is_fixed(src)
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// no AMX Mod X module here (nothing to do)
    NotThere,
    Ours,
    /// put back; the other one kept at this path (None when it was kept before)
    Restored(Option<PathBuf>),
}

fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // civil date from days (Howard Hinnant)
    let z = (secs / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}{m:02}{d:02}")
}

/// Ours in `game_dir` (a game folder: cstrike or czero), from `fixed`. Fast when it's ours
/// already: one hash of a small file.
#[cfg(test)]
pub fn ensure_in(game_dir: &Path, fixed: &Path) -> Result<Outcome, String> {
    ensure_at(game_dir, REL, fixed)
}

/// The same for the module at `rel` (each set of the game's binaries has its own: gamebin.rs)
fn ensure_at(game_dir: &Path, rel: &str, fixed: &Path) -> Result<Outcome, String> {
    let want = fixed_hash(rel);
    let file_name = || file_name_of(rel);
    let p = game_dir.join(rel);
    let Some(found) = sha256(&p) else { return Ok(Outcome::NotThere) };
    if found == want {
        return Ok(Outcome::Ours);
    }
    if sha256(fixed).as_deref() != Some(want) {
        return Err("Loner-Strike's fixed Ham Sandwich is missing from the app. Reinstalling the app should fix it.".into());
    }
    // the one found, kept once (the same file kept before isn't kept again)
    let dir = p.parent().unwrap_or(game_dir);
    let kept_before = std::fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        n.starts_with(&format!("{}.replaced-", file_name())) && sha256(&e.path()).as_deref() == Some(found.as_str())
    });
    let mut backup = None;
    if !kept_before {
        let mut b = dir.join(format!("{}.replaced-{}", file_name(), today()));
        for n in 2..100 {
            if !b.exists() {
                break;
            }
            b = dir.join(format!("{}.replaced-{}-{n}", file_name(), today()));
        }
        std::fs::copy(&p, &b).map_err(|e| format!("Couldn't keep a copy of the Ham Sandwich module that was there ({e})."))?;
        backup = Some(b);
    }
    let bytes = std::fs::read(fixed).map_err(|e| format!("Couldn't read the fixed Ham Sandwich ({e})."))?;
    crate::sys::write_atomic(&p, &bytes).map_err(|e| format!("Couldn't put the fixed Ham Sandwich back ({e}). Is the game running?"))?;
    Ok(Outcome::Restored(backup))
}

/// Before Play or a local server start: ours back in place if something replaced it. Never
/// stops the start; says what it did in the diagnostics. Some(note) when it put ours back.
/// (On Linux both the native module and the Windows one Proton's game loads.)
pub fn before_start(game_dir: &Path, what: &str) -> Option<String> {
    let mut said = None;
    for f in crate::gamebin::FLAVORS {
        if let Some(note) = before_start_at(game_dir, what, f.ham, f.ham_fixed_res) {
            said.get_or_insert(note);
        }
    }
    said
}

fn before_start_at(game_dir: &Path, what: &str, rel: &str, res_rel: &str) -> Option<String> {
    let fixed = fixed_from(res_rel)?;
    // (a loaded DLL can't be replaced on Windows: with the game open, say so instead of failing)
    let p = game_dir.join(rel);
    if what == "Play" && p.is_file() && !is_fixed(&p) && crate::sys::game_running() {
        crate::diagnostics::log_error("ham sandwich", &format!("{what}: not the fixed one, but the game is open"));
        return Some("Loner-Strike's fixed Ham Sandwich goes back in the next time the game starts from a closed game.".into());
    }
    match ensure_at(game_dir, rel, &fixed) {
        Ok(Outcome::Restored(kept)) => {
            let note = "Restored Loner-Strike's fixed Ham Sandwich.".to_string();
            let where_ = kept.map(|k| format!(" The one found was kept as {}.", k.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())).unwrap_or_default();
            crate::diagnostics::log_error("ham sandwich", &format!("{what}: {note}{where_}"));
            Some(note)
        }
        Ok(_) => None,
        Err(why) => {
            crate::diagnostics::log_error("ham sandwich", &format!("{what}: {why}"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-hamguard-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("addons/amxmodx/modules")).unwrap();
        d
    }

    fn fixed() -> PathBuf {
        fixed_dll().expect("the repo's fixed Ham Sandwich")
    }

    #[test]
    fn the_official_module_is_replaced_before_play_and_kept_once() {
        let g = game("official");
        let p = g.join(REL);
        std::fs::write(&p, b"the official Ham Sandwich").unwrap();
        let r = ensure_in(&g, &fixed()).unwrap();
        let Outcome::Restored(Some(kept)) = r else { panic!("{r:?}") };
        assert!(is_fixed(&p), "ours is back");
        assert_eq!(std::fs::read(&kept).unwrap(), b"the official Ham Sandwich");
        assert!(kept.file_name().unwrap().to_string_lossy().starts_with(&format!("{}.replaced-", file_name())));
        // the same official one again: put right, not kept a second time
        std::fs::write(&p, b"the official Ham Sandwich").unwrap();
        assert_eq!(ensure_in(&g, &fixed()).unwrap(), Outcome::Restored(None));
        let backups = std::fs::read_dir(p.parent().unwrap()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".replaced-")).count();
        assert_eq!(backups, 1);
        let _ = std::fs::remove_dir_all(&g);
    }

    #[test]
    fn ours_is_left_alone_and_no_module_is_nothing_to_do() {
        let g = game("ours");
        let p = g.join(REL);
        std::fs::copy(fixed(), &p).unwrap();
        let before = std::fs::metadata(&p).unwrap().modified().unwrap();
        assert_eq!(ensure_in(&g, &fixed()).unwrap(), Outcome::Ours);
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before, "not written");
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1, "no backup");
        std::fs::remove_file(&p).unwrap();
        assert_eq!(ensure_in(&g, &fixed()).unwrap(), Outcome::NotThere);
        let _ = std::fs::remove_dir_all(&g);
    }

    #[test]
    fn an_install_that_ships_a_ham_module_keeps_ours() {
        let g = game("install");
        let p = g.join(REL);
        std::fs::copy(fixed(), &p).unwrap();
        let theirs = g.join("theirs.dll");
        std::fs::write(&theirs, b"a package's Ham Sandwich").unwrap();
        assert!(is_ham("addons/amxmodx/modules/hamsandwich_amxx.dll") && is_ham("addons\\amxmodx\\modules\\HAMSANDWICH_AMXX_I386.SO"));
        assert!(!is_ham("addons/amxmodx/modules/fun_amxx.dll"));
        assert!(keep_ours(&p, &theirs), "ours stays");
        assert!(!keep_ours(&p, &fixed()), "ours over ours is fine");
        std::fs::remove_file(&p).unwrap();
        assert!(!keep_ours(&p, &theirs), "nothing there yet: a first install may put one in (setup puts ours after it)");
        let _ = std::fs::remove_dir_all(&g);
    }
}
