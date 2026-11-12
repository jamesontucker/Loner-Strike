//! Ready-made bot navigation (the nav pack). Loner-Strike builds a map's .nav, with place names
//! where it can (the stock maps, named from Condition Zero's own navs), and the bot brain's map
//! knowledge for it (.lsv: sightlines, cover, wallbangs), and publishes them with a list. For
//! every map a person has, in the exact version a pair was made from (the size of the map file,
//! which every nav records), the app puts the pair in the game's maps folder, so the bots know
//! the map the first time it loads instead of learning it first.
//!
//! A nav with place names already there is left alone: one made by hand, or by Valve for
//! Condition Zero, is better than anything generated. A nav without names (one the bots
//! learned) is replaced when the pack has one, and its map knowledge with it: map knowledge is
//! made for one nav. Servers get the files from the game's maps folder with the rest (navs.rs).

use crate::sys;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

const LIST_URL: &str = "https://dl.lonerstrike.com/navs/list.json";
const FILE_BASE: &str = "https://dl.lonerstrike.com/navs/";
/// The most a pack file can be (dust2's nav is about 700 KB, its map knowledge a few MB)
const MAX_FILE: u64 = 64 * 1024 * 1024;
const GAMES: [&str; 2] = ["cstrike", "czero"];

/// One map in the pack list.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct PackMap {
    /// the map's name, lower case
    pub map: String,
    /// the size of the .bsp the pair was made from
    pub bsp: u64,
    pub nav_sha256: String,
    pub lsv_sha256: String,
    /// earlier pack navs for this map (sha256): a player who has one gets this one instead,
    /// though it has place names (ours do, on the stock maps; a hand-made one never matches)
    #[serde(default)]
    pub replaces: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct PackList {
    pub maps: Vec<PackMap>,
}

/// What to do for one map in one game.
#[derive(Debug, PartialEq)]
pub enum Step {
    /// no nav there yet: the pack's pair goes in
    Add,
    /// a learned nav (no place names) or an older pack one: the pack's pair replaces it
    Replace,
    /// the pack's nav is there but its map knowledge is missing, unfinished or broken: only that
    /// goes in (the nav keeps its date, so it isn't sent to every server again)
    Knowledge,
}

/// A map knowledge file the brain finished (any version: the brain itself rebuilds one written
/// by another version, and a finished one is never replaced - no back and forth with it).
/// ReGameDLL cs_bot_ls.cpp FileHead: magic "LSV2", version, key, count, stage (4: done).
pub fn lsv_done(b: &[u8]) -> bool {
    let word = |i: usize| b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    word(0) == Some(0x3256_534C) && word(16) == Some(4)
}

/// The place names in a nav: the count of its place directory, right after the header (version
/// 5 on; ReGameDLL game_shared/bot/nav_file.cpp: magic, version, bsp size, then the places).
pub fn place_count(b: &[u8]) -> Option<u16> {
    let head = crate::navs::parse_head(b)?;
    if head.version < 5 {
        return Some(0);
    }
    let at = 12;
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn sha_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// A file in `dir` named <stem>.<ext>, whatever its case.
fn find(dir: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    let want = format!("{stem}.{ext}").to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.to_ascii_lowercase() == want)
    })
}

/// What the pack would do for this map in this maps folder (None: nothing).
pub fn step_for(maps_dir: &Path, m: &PackMap) -> Option<Step> {
    let bsp = find(maps_dir, &m.map, "bsp")?;
    // (only the exact version it was made from)
    if std::fs::metadata(&bsp).ok()?.len() != m.bsp {
        return None;
    }
    let Some(nav) = find(maps_dir, &m.map, "nav") else {
        return Some(Step::Add);
    };
    let b = std::fs::read(&nav).ok()?;
    // (already this one: its map knowledge too, unless that's missing or unfinished)
    if sha_hex(&b) == m.nav_sha256 {
        let lsv_ok = find(maps_dir, &m.map, "lsv").and_then(|p| std::fs::read(p).ok()).is_some_and(|x| lsv_done(&x) || sha_hex(&x) == m.lsv_sha256);
        return if lsv_ok { None } else { Some(Step::Knowledge) };
    }
    // (an earlier pack nav of ours: the new one goes in)
    let have = sha_hex(&b);
    if m.replaces.iter().any(|h| h.eq_ignore_ascii_case(&have)) {
        return Some(Step::Replace);
    }
    // (one with place names: made by hand, or by Valve - left alone; a broken one is replaced)
    match place_count(&b) {
        Some(n) if n > 0 => None,
        _ => Some(Step::Replace),
    }
}

fn get(url: &str) -> Result<Vec<u8>, String> {
    let r = sys::http().get(url).timeout(Duration::from_secs(60)).send().map_err(|e| format!("couldn't reach the download server ({e})"))?;
    if !r.status().is_success() {
        return Err(format!("the download server said {}", r.status()));
    }
    let mut out = Vec::new();
    r.take(MAX_FILE).read_to_end(&mut out).map_err(|e| format!("the download stopped ({e})"))?;
    Ok(out)
}

/// Writes `bytes` to `dest` in one step (a running game never reads half a file).
fn write_whole(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // (a read-only file there can't be replaced on Windows; and the rename is retried there
    // while an antivirus scan holds the file)
    if dest.exists() {
        sys::make_writable(dest);
    }
    sys::write_atomic(dest, bytes)
}

/// Puts the pair in: the nav and its map knowledge, both checked against the list.
fn install(maps_dir: &Path, m: &PackMap, step: &Step, fetch: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Result<(), String> {
    let nav = if *step == Step::Knowledge { Vec::new() } else { fetch(&format!("{FILE_BASE}{}.nav", m.map))? };
    if *step != Step::Knowledge && sha_hex(&nav) != m.nav_sha256 {
        return Err(format!("{}.nav didn't download whole", m.map));
    }
    let lsv = fetch(&format!("{FILE_BASE}{}.lsv", m.map))?;
    if sha_hex(&lsv) != m.lsv_sha256 {
        return Err(format!("{}.lsv didn't download whole", m.map));
    }
    // (over a differently written name of the same file, not beside it)
    let nav_dest = find(maps_dir, &m.map, "nav").unwrap_or_else(|| maps_dir.join(format!("{}.nav", m.map)));
    let lsv_dest = find(maps_dir, &m.map, "lsv").unwrap_or_else(|| maps_dir.join(format!("{}.lsv", m.map)));
    // (not while a server sync is copying navs: it would send half a pair)
    let _placing = crate::navs::PLACING.lock().unwrap_or_else(|p| p.into_inner());
    // (map knowledge first: a nav without its own knowledge would have the brain rebuild it)
    write_whole(&lsv_dest, &lsv).map_err(|e| format!("couldn't save {}.lsv ({e})", m.map))?;
    if *step != Step::Knowledge {
        write_whole(&nav_dest, &nav).map_err(|e| format!("couldn't save {}.nav ({e})", m.map))?;
    }
    Ok(())
}

/// What a run did.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub added: Vec<String>,
    pub replaced: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// Every map in the list against this game install's maps folders.
pub fn run_with(hl: &Path, list: &PackList, fetch: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Outcome {
    let mut out = Outcome::default();
    for game in GAMES {
        let dir = hl.join(game).join("maps");
        if !dir.is_dir() {
            continue;
        }
        for m in &list.maps {
            let Some(step) = step_for(&dir, m) else { continue };
            match install(&dir, m, &step, fetch) {
                Ok(()) => match step {
                    Step::Add => out.added.push(m.map.clone()),
                    Step::Replace | Step::Knowledge => out.replaced.push(m.map.clone()),
                },
                Err(e) => out.failed.push((m.map.clone(), e)),
            }
        }
    }
    out
}

/// The published list, then every map this computer's game has (in the background: at setup,
/// at start, and after maps or mods come in).
pub fn run(hl: &Path) -> Result<Outcome, String> {
    let list: PackList = serde_json::from_slice(&get(LIST_URL)?).map_err(|e| format!("the nav list couldn't be read ({e})"))?;
    Ok(run_with(hl, &list, &get))
}

/// In the background: once a few minutes after the app starts, then every six hours (new maps
/// from a mod or Get maps are picked up then; nothing is written while the game is running a
/// map that's being replaced, as the files go in whole). Quiet when the list can't be reached.
pub fn start_loop() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(180));
        loop {
            if let Some(hl) = crate::navs::local_hl() {
                if let Ok(out) = run(&hl) {
                    if !out.added.is_empty() || !out.replaced.is_empty() || !out.failed.is_empty() {
                        eprintln!("nav pack: added {:?}, replaced {:?}, failed {:?}", out.added, out.replaced, out.failed);
                        for (map, why) in &out.failed {
                            crate::diagnostics::log_error("navpack", &format!("{map}: {why}"));
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav(version: u32, bsp: u32, places: u16) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0xFEEDFACEu32.to_le_bytes());
        b.extend_from_slice(&version.to_le_bytes());
        b.extend_from_slice(&bsp.to_le_bytes());
        b.extend_from_slice(&places.to_le_bytes());
        b.resize(2048, 0);
        b
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-navpack-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("cstrike/maps")).unwrap();
        d
    }

    #[test]
    fn reads_place_counts() {
        assert_eq!(place_count(&nav(5, 100, 23)), Some(23));
        assert_eq!(place_count(&nav(5, 100, 0)), Some(0));
        assert_eq!(place_count(&nav(4, 100, 9)), Some(0), "before version 5 there are no places");
        assert_eq!(place_count(b"not a nav"), None);
    }

    #[test]
    fn decides_by_version_and_names() {
        let hl = dir("steps");
        let maps = hl.join("cstrike/maps");
        std::fs::write(maps.join("de_x.bsp"), vec![0u8; 500]).unwrap();
        let packed = nav(5, 500, 12);
        let m = PackMap { map: "de_x".into(), bsp: 500, nav_sha256: sha_hex(&packed), lsv_sha256: sha_hex(b"lsv"), replaces: vec![] };
        assert_eq!(step_for(&maps, &m), Some(Step::Add), "no nav yet");
        std::fs::write(maps.join("de_x.nav"), nav(5, 500, 0)).unwrap();
        assert_eq!(step_for(&maps, &m), Some(Step::Replace), "a learned nav without names");
        std::fs::write(maps.join("de_x.nav"), nav(5, 500, 7)).unwrap();
        assert_eq!(step_for(&maps, &m), None, "a named nav stays");
        std::fs::write(maps.join("de_x.nav"), &packed).unwrap();
        assert_eq!(step_for(&maps, &m), Some(Step::Knowledge), "the pack's nav but not its map knowledge");
        let mut done = vec![0u8; 64];
        done[0..4].copy_from_slice(&0x3256_534Cu32.to_le_bytes());
        done[16..20].copy_from_slice(&4u32.to_le_bytes());
        std::fs::write(maps.join("de_x.lsv"), &done).unwrap();
        assert_eq!(step_for(&maps, &m), None, "a finished map knowledge of its own (any version) stays");
        done[16..20].copy_from_slice(&2u32.to_le_bytes());
        std::fs::write(maps.join("de_x.lsv"), &done).unwrap();
        assert_eq!(step_for(&maps, &m), Some(Step::Knowledge), "an unfinished one is topped up");
        std::fs::write(maps.join("de_x.lsv"), b"lsv").unwrap();
        assert_eq!(step_for(&maps, &m), None, "already the pack's pair");
        let other = PackMap { bsp: 501, ..m.clone() };
        assert_eq!(step_for(&maps, &other), None, "another version of the map");
        // an update to the pack: our earlier named nav is replaced, a hand-made named one isn't
        let old_ours = nav(5, 500, 9);
        std::fs::write(maps.join("de_x.nav"), &old_ours).unwrap();
        let newer = PackMap { nav_sha256: sha_hex(&nav(5, 500, 10)), replaces: vec![sha_hex(&old_ours).to_uppercase()], ..m.clone() };
        assert_eq!(step_for(&maps, &newer), Some(Step::Replace), "an earlier pack nav of ours");
        std::fs::write(maps.join("de_x.nav"), nav(5, 500, 11)).unwrap();
        assert_eq!(step_for(&maps, &newer), None, "a hand-made named nav still stays");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn installs_checked_pairs_only() {
        let hl = dir("install");
        let maps = hl.join("cstrike/maps");
        std::fs::write(maps.join("DE_Y.bsp"), vec![0u8; 300]).unwrap();
        let packed = nav(5, 300, 4);
        let good = PackMap { map: "de_y".into(), bsp: 300, nav_sha256: sha_hex(&packed), lsv_sha256: sha_hex(b"knowledge"), replaces: vec![] };
        let list = PackList { maps: vec![good.clone()] };
        let p2 = packed.clone();
        let fetch = move |url: &str| -> Result<Vec<u8>, String> { Ok(if url.ends_with(".nav") { p2.clone() } else { b"knowledge".to_vec() }) };
        let out = run_with(&hl, &list, &fetch);
        assert_eq!(out.added, vec!["de_y".to_string()]);
        assert_eq!(std::fs::read(maps.join("de_y.nav")).unwrap(), packed);
        assert_eq!(std::fs::read(maps.join("de_y.lsv")).unwrap(), b"knowledge");
        // (a second run: nothing to do)
        assert_eq!(run_with(&hl, &list, &fetch), Outcome::default());
        // (a damaged download is refused and nothing is written)
        std::fs::remove_file(maps.join("de_y.nav")).unwrap();
        std::fs::remove_file(maps.join("de_y.lsv")).unwrap();
        let bad = |_: &str| -> Result<Vec<u8>, String> { Ok(b"garbage".to_vec()) };
        let out = run_with(&hl, &list, &bad);
        assert_eq!(out.failed.len(), 1);
        assert!(find(&maps, "de_y", "nav").is_none() && find(&maps, "de_y", "lsv").is_none());
        let _ = std::fs::remove_dir_all(&hl);
    }
}
