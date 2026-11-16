//! The foundation: Metamod-P, AMX Mod X (base + Counter-Strike addon) and ReGameDLL
//! (Loner-Strike's fork: the bots' changes). Dedicated servers also get REHLDS (Loner-Strike's
//! fork of the engine: voice awareness for the bots) and ReAPI.
//! Every version is the one Loner-Strike
//! is tested with, pinned by checksum.

use super::fetch;
use super::record::Record;
use super::{Ctx, TempDir};
use regex::Regex;
use std::path::Path;

pub struct Pinned {
    pub url: &'static str,
    pub sha256: &'static str,
    pub what: &'static str,
}

pub const METAMOD: Pinned = Pinned {
    url: "https://github.com/Bots-United/metamod-p/releases/download/v1.21p109/metamod-p-v1.21p109-win32.tar.xz",
    sha256: "564838d9a42032ee3bc44eeb2616896e0789c3ce8d7a033d81f3f795b67e31ed",
    what: "Metamod-P",
};
pub const AMXX_BASE: Pinned = Pinned {
    url: "https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-base-windows.zip",
    sha256: "ca13b474741f5f0b035507969cb853b7eac9052614402114801e21c4dc2f2ebd",
    what: "AMX Mod X",
};
pub const AMXX_CSTRIKE: Pinned = Pinned {
    url: "https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-cstrike-windows.zip",
    sha256: "47f358b4dd40c0259576bbde9790442dee4b642fe31bfed0421180b81429d4aa",
    what: "AMX Mod X's Counter-Strike module",
};
/// Loner-Strike's fork of ReGameDLL (github.com/jamesontucker/ReGameDLL_CS, branch loner-strike)
pub const REGAMEDLL: Pinned = Pinned {
    url: "https://github.com/jamesontucker/ReGameDLL_CS/releases/download/5.30.0.814-ls23/regamedll-bin-5.30.0.814-ls23.zip",
    sha256: "0facd6895e32a132b0fdac2992e243a70ab85b3e0c6628c7739ab180f45794f2",
    what: "ReGameDLL",
};
/// Loner-Strike's fork of ReHLDS (github.com/jamesontucker/ReHLDS, branch loner-strike): the
/// engine for dedicated servers
pub const REHLDS: Pinned = Pinned {
    url: "https://github.com/jamesontucker/ReHLDS/releases/download/3.15.0.896-ls8/rehlds-bin-3.15.0.896-ls8.zip",
    sha256: "95973fca8731b89e95def1168bc3a8ea0a2b72e2a0bd00c89a1859e9cdfca721",
    what: "ReHLDS",
};
pub const REAPI: Pinned = Pinned {
    url: "https://github.com/rehlds/ReAPI/releases/download/5.29.0.358/reapi-bin-5.29.0.358.zip",
    sha256: "f33a7435540bea8706db3fa948e51a85b511f5b18c383b97402a204dc5419195",
    what: "ReAPI",
};
pub const METAMOD_DLL: &str = r"addons\metamod\dlls\metamod.dll";

/// AMX Mod X files a player may have customised: never replaced if present, whoever put
/// them there (plugins.ini with the player's own plugins, users.ini with admins added in the
/// game, amxx.cfg). Only missing ones come from the download; core::plugins adds Loner-Strike's
/// lines to plugins.ini.
const KEEP_IF_PRESENT: &[&str] = &["addons/amxmodx/configs/"];

/// Every file under root as (full path, '/'-separated relative path), sorted.
pub fn tree(root: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    for full in crate::archive::walk(root) {
        if let Ok(rel) = full.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel.rsplit('/').next() == Some(".DS_Store") {
                continue;
            }
            out.push((full.clone(), rel));
        }
    }
    out
}

fn get(p: &Pinned, ctx: &Ctx, tmp: &TempDir, sub: &str) -> Result<std::path::PathBuf, String> {
    let file = fetch::download(p.url, Some(p.sha256), p.what, ctx.progress)?;
    ctx.progress.detail(&format!("Unpacking {}", p.what));
    fetch::unpack(&file, &tmp.0.join(sub))
}

fn need(path: std::path::PathBuf, what: &str) -> Result<std::path::PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("The {what} download is missing a file Loner-Strike needs ({}).", path.file_name().unwrap_or_default().to_string_lossy()))
    }
}

/// The KEEP_IF_PRESENT files a game has now ('/'-separated, lower case: Windows and Mac
/// folders ignore case).
fn present(game: &Path) -> Vec<String> {
    KEEP_IF_PRESENT.iter().flat_map(|k| tree(&game.join(k)).into_iter().map(move |(_, rel)| format!("{k}{rel}").to_lowercase())).collect()
}

/// A file the game already had that Loner-Strike didn't put there.
fn theirs(rec: &Record, rel: &str) -> bool {
    rec.exists(rel) && !rec.ours(rel)
}

/// Whether a file contains `needle` (to recognise ReGameDLL and Metamod builds).
fn mentions(rec: &Record, rel: &str, needle: &[u8]) -> bool {
    rec.full(rel).ok().and_then(|p| std::fs::read(p).ok()).map(|d| d.windows(needle.len()).any(|w| w == needle)).unwrap_or(false)
}

/// Downloads every foundation archive into the download cache before anything in the game is
/// changed, so a download that fails (no internet, a checksum that doesn't match) leaves the
/// game as it was instead of half updated. The steps then unpack them from the cache.
pub fn prefetch(rec: &Record, ctx: &Ctx) -> Result<(), String> {
    let mut all = vec![&AMXX_BASE, &AMXX_CSTRIKE, &REGAMEDLL];
    if !theirs(rec, "addons/metamod/dlls/metamod.dll") {
        all.insert(0, &METAMOD);
    }
    for p in all {
        fetch::download(p.url, Some(p.sha256), p.what, ctx.progress)?;
    }
    Ok(())
}

pub fn metamod(rec: &mut Record, ctx: &Ctx, tmp: &TempDir) -> Result<(), String> {
    // a server that already runs Metamod (often Metamod-R) keeps its own
    if theirs(rec, "addons/metamod/dlls/metamod.dll") {
        ctx.progress.note("Kept the Metamod that was already there.");
    } else {
        let mm = get(&METAMOD, ctx, tmp, "mm")?;
        rec.copy(&need(mm.join("addons/metamod/dlls/metamod.dll"), "Metamod-P")?, "addons/metamod/dlls/metamod.dll")?;
    }
    let plugins_ini = rec.read("addons/metamod/plugins.ini").unwrap_or_default();
    if !plugins_ini.to_lowercase().contains("amxmodx_mm.dll") {
        let sep = if plugins_ini.trim().is_empty() { "" } else { "\r\n" };
        let text = format!("{}{sep}win32 addons\\amxmodx\\dlls\\amxmodx_mm.dll\r\n", plugins_ini.trim_end_matches(['\r', '\n']));
        rec.write("addons/metamod/plugins.ini", &text)?;
    }
    Ok(())
}

pub fn amxmodx(rec: &mut Record, ctx: &Ctx, tmp: &TempDir) -> Result<(), String> {
    // what's there before this step (the Counter-Strike addon's plugins.ini still replaces the
    // base one it just put in on a first install)
    let kept = present(&rec.game);
    for (p, sub) in [(&AMXX_BASE, "amxx_base"), (&AMXX_CSTRIKE, "amxx_cstrike")] {
        let src = get(p, ctx, tmp, sub)?;
        ctx.progress.detail("Copying AMX Mod X into the game");
        for (full, rel) in tree(&src) {
            if !rel.starts_with("addons/") {
                continue;
            }
            if kept.contains(&rel.to_lowercase()) {
                continue;
            }
            rec.copy(&full, &rel)?;
        }
    }
    // Ham Sandwich with the special-bot queue fix (resources/windows/hamsandwich/README.md).
    // Without it hl.exe uses freed memory on map changes: real Windows crashes on it, Wine
    // on the Mac happens to survive it, but it's the same Windows game there, so CrossOver
    // installs get the fixed module too. (A server on Linux has its own .so.)
    if cfg!(windows) || ctx.install.kind == "crossover" {
        // the app's resources, or the repo's own copy in a dev build
        let rel = Path::new("windows").join("hamsandwich").join("hamsandwich_amxx.dll");
        let mut places = vec![ctx.res.join(&rel)];
        if cfg!(debug_assertions) {
            places.push(super::repo_root().join("app/src-tauri/resources").join(&rel));
        }
        if let Some(patched) = places.into_iter().find(|p| p.is_file()) {
            rec.copy(&patched, "addons/amxmodx/modules/hamsandwich_amxx.dll")?;
        } else {
            ctx.progress.note("Loner-Strike's fixed Ham Sandwich module is missing from the app, so the game may crash on map changes. Reinstalling the app should fix it.");
        }
    }
    Ok(())
}

/// What server code (mp.dll) a game has, as setup sees it (QA4-CORE F4).
#[derive(Debug, PartialEq, Eq)]
pub enum ServerCode {
    /// Loner-Strike's ReGameDLL, any version (its bot cvars, `bot_ls_...`): setup puts the
    /// pinned one in, as an update
    OurBrain,
    /// another ReGameDLL (a mod pack's, the stock one) that Loner-Strike didn't put there: kept,
    /// and the player is told the bots won't have Loner-Strike's brain until it's replaced
    OtherReGameDll,
    /// the game's own (Valve's), one setup wrote, or none: replaced, the original backed up
    Replace,
}

pub fn server_code(rec: &Record) -> ServerCode {
    const MP: &str = "dlls/mp.dll";
    if mentions(rec, MP, b"bot_ls_") {
        ServerCode::OurBrain
    } else if theirs(rec, MP) && mentions(rec, MP, b"ReGameDLL") {
        ServerCode::OtherReGameDll
    } else {
        ServerCode::Replace
    }
}

/// ReGameDLL replaces the game's own server code (mp.dll); the original is backed up. Another
/// ReGameDLL is kept (see ServerCode); Loner-Strike's own brain of any version is updated.
pub fn regamedll(rec: &mut Record, ctx: &Ctx, tmp: &TempDir) -> Result<(), String> {
    let rg = get(&REGAMEDLL, ctx, tmp, "rg")?;
    match server_code(rec) {
        ServerCode::OtherReGameDll => {
            ctx.progress.note(KEPT_OTHER_REGAMEDLL);
            ctx.progress.kept_other_brain();
        }
        code => {
            let was_ours_elsewhere = code == ServerCode::OurBrain && !rec.ours("dlls/mp.dll");
            rec.copy(&need(rg.join("bin/win32/cstrike/dlls/mp.dll"), "ReGameDLL")?, "dlls/mp.dll")?;
            if was_ours_elsewhere {
                ctx.progress.note("Updated Loner-Strike's bot brain (its ReGameDLL) to the current one. The old one is backed up.");
            }
        }
    }
    for cfg in ["game.cfg", "game_init.cfg"] {
        if !rec.exists(cfg) {
            rec.copy(&need(rg.join("bin/win32/cstrike").join(cfg), "ReGameDLL")?, cfg)?;
        }
    }
    Ok(())
}

pub const KEPT_OTHER_REGAMEDLL: &str =
    "Kept the ReGameDLL that was already there. It isn't Loner-Strike's, so bots won't have Loner-Strike's brain until it's replaced.";

/// Puts Loner-Strike's ReGameDLL in place of another one setup kept (the setup done screen's
/// "Use Loner-Strike's"): the old one is backed up, like every file setup replaces.
pub fn use_our_brain(rec: &mut Record, ctx: &Ctx, tmp: &TempDir) -> Result<(), String> {
    let rg = get(&REGAMEDLL, ctx, tmp, "rg")?;
    rec.begin("foundation");
    rec.copy(&need(rg.join("bin/win32/cstrike/dlls/mp.dll"), "ReGameDLL")?, "dlls/mp.dll")?;
    rec.save()
}

/// liblist.gam with gamedll pointing at Metamod, or None if it already does.
pub fn point_at_metamod(lib: &str) -> Option<String> {
    let re = Regex::new(r#"(?m)^gamedll\s+"([^"]*)""#).unwrap();
    match re.captures(lib) {
        None => Some(format!("{}\r\ngamedll \"{METAMOD_DLL}\"\r\n", lib.trim_end_matches(['\r', '\n']))),
        Some(c) => {
            let g = c.get(1).unwrap();
            if g.as_str().eq_ignore_ascii_case(METAMOD_DLL) {
                None
            } else {
                Some(format!("{}{}{}", &lib[..g.start()], METAMOD_DLL, &lib[g.end()..]))
            }
        }
    }
}

/// Points the game at Metamod.
pub fn liblist(rec: &mut Record) -> Result<(), String> {
    let lib = rec.read("liblist.gam").ok_or_else(|| no_liblist(&rec.game))?;
    if let Some(new) = point_at_metamod(&lib) {
        rec.write("liblist.gam", &new)?;
    }
    Ok(())
}

pub fn no_liblist(game: &Path) -> String {
    format!("{} has no liblist.gam, so it doesn't look like a Counter-Strike folder.", game.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liblist_rewrite() {
        let lib = "game \"Counter-Strike\"\r\ngamedll \"dlls\\mp.dll\"\r\ngamedll_linux \"dlls/cs.so\"\r\n";
        assert_eq!(
            point_at_metamod(lib).unwrap(),
            "game \"Counter-Strike\"\r\ngamedll \"addons\\metamod\\dlls\\metamod.dll\"\r\ngamedll_linux \"dlls/cs.so\"\r\n"
        );
        assert_eq!(point_at_metamod(&point_at_metamod(lib).unwrap()), None);
        assert_eq!(point_at_metamod("game \"x\"\n\n").unwrap(), "game \"x\"\r\ngamedll \"addons\\metamod\\dlls\\metamod.dll\"\r\n");
    }

    /// QA4-CORE F4 (and QA4-A's CZ trap): Loner-Strike's brain of any version is updated, another
    /// ReGameDLL is kept (and said), Valve's or one setup wrote is replaced; the same in both games
    #[test]
    fn which_server_code_is_kept() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "brain-").unwrap();
        for game in ["cstrike", "czero"] {
            let dir = root.join(game);
            std::fs::create_dir_all(dir.join("dlls")).unwrap();
            let put = |b: &[u8]| std::fs::write(dir.join("dlls/mp.dll"), b).unwrap();
            let rec = Record::open(&dir);
            put(b"MZ...ReGameDLL version: 5.30.0.841-dev...bot_ls_lines\0");
            assert_eq!(server_code(&rec), ServerCode::OurBrain, "an older Loner-Strike brain left behind: updated");
            put(b"MZ...ReGameDLL version: 5.21.0.556-dev...");
            assert_eq!(server_code(&rec), ServerCode::OtherReGameDll, "another ReGameDLL: kept");
            put(b"MZ...Counter-Strike...");
            assert_eq!(server_code(&rec), ServerCode::Replace, "Valve's own: replaced");
            // a stock ReGameDLL an earlier setup put there itself (CZ on this Mac, QA4-A): replaced
            let mut rec = Record::open(&dir);
            put(b"MZ...ReGameDLL version: 5.30.0.814-dev...");
            rec.begin("foundation");
            rec.copy(&dir.join("dlls/mp.dll"), "dlls/mp.dll").unwrap();
            assert_eq!(server_code(&rec), ServerCode::Replace, "{game}: one setup wrote, without the brain");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The done screen hears which game kept another ReGameDLL (and only once)
    #[test]
    fn a_kept_regamedll_reaches_the_done_screen() {
        let job = crate::setup::job_for_test("install", &[]);
        job.set_game_for_test("czero");
        let p: &dyn crate::setup::Progress = &*job;
        p.note(KEPT_OTHER_REGAMEDLL);
        p.kept_other_brain();
        p.kept_other_brain();
        let v = job.view();
        assert_eq!(v.other_brain, ["czero"]);
        assert!(v.notes[0].starts_with("Counter-Strike: Condition Zero: Kept the ReGameDLL") && v.notes[0].contains("won't have Loner-Strike's brain"), "{:?}", v.notes);
        assert!(serde_json::to_value(&v).unwrap()["other_brain"] == serde_json::json!(["czero"]));
        assert!(serde_json::to_value(crate::setup::JobView::idle()).unwrap().get("other_brain").is_none());
    }

    /// AMX Mod X's own configs a game already has (admins, the player's plugins) are kept on a
    /// repair: only these are skipped when the download is copied in
    #[test]
    fn existing_configs_are_kept() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "configs-").unwrap();
        let cfg = root.join("addons/amxmodx/configs");
        std::fs::create_dir_all(cfg.join("maps")).unwrap();
        std::fs::write(cfg.join("Plugins.ini"), "admin.amxx\r\n").unwrap();
        std::fs::write(cfg.join("users.ini"), "\"STEAM_0:1:2\" \"\" \"abcdefghijklmnopqrstu\" \"ce\"\r\n").unwrap();
        std::fs::write(cfg.join("maps/de_dust2.cfg"), "").unwrap();
        std::fs::create_dir_all(root.join("addons/amxmodx/plugins")).unwrap();
        std::fs::write(root.join("addons/amxmodx/plugins/admin.amxx"), "").unwrap();
        let mut kept = present(&root);
        kept.sort();
        assert_eq!(kept, ["addons/amxmodx/configs/maps/de_dust2.cfg", "addons/amxmodx/configs/plugins.ini", "addons/amxmodx/configs/users.ini"]);
        assert!(present(&root.join("nothing-here")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A fresh install's server code is the pinned brain (ls23: 5.30.0.1318-dev): the pinned
    /// ReGameDLL through the app's own download path (mirror first, SHA-256 checked) into a
    /// scratch game, as setup's ReGameDLL step puts it in. Network:
    /// `LS_APP_DIR=<scratch> cargo test -- --ignored pinned_brain_installs`.
    #[test]
    #[ignore]
    fn pinned_brain_installs() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "brain-install-").unwrap();
        let hl = root.join("Half-Life");
        let game = hl.join("cstrike");
        std::fs::create_dir_all(game.join("dlls")).unwrap();
        std::fs::write(game.join("liblist.gam"), "game \"Counter-Strike\"\r\ngamedll \"dlls\\mp.dll\"\r\n").unwrap();
        std::fs::write(game.join("dlls/mp.dll"), b"MZ Valve's own server code").unwrap();
        let install = crate::steam::Install { hl: hl.clone(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec!["cstrike".into()], dedicated: false };
        let res = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ctx = Ctx { install: &install, res: &res, progress: &crate::setup::Quiet, store: None };
        let mut rec = Record::open(&game);
        rec.begin("foundation");
        let tmp = TempDir::new().unwrap();
        regamedll(&mut rec, &ctx, &tmp).unwrap();
        let dll = std::fs::read(game.join("dlls/mp.dll")).unwrap();
        let has = |s: &[u8]| dll.windows(s.len()).any(|w| w == s);
        assert!(has(b"5.30.0.1318-dev") && has(b"bot_ls_"), "the pinned brain");
        assert!(REGAMEDLL.url.ends_with("regamedll-bin-5.30.0.814-ls23.zip"));
        // Valve's was backed up, and is what an uninstall puts back
        assert!(rec.ours("dlls/mp.dll"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The pinned ReHLDS through the app's own download path (the mirror first, the SHA-256
    /// checked) has both engines in it. Network: `cargo test -- --ignored pinned_rehlds`.
    #[test]
    #[ignore]
    fn pinned_rehlds_downloads_with_both_engines() {
        let file = crate::setup::fetch::download(REHLDS.url, Some(REHLDS.sha256), REHLDS.what, &crate::setup::Quiet).unwrap();
        let tmp = crate::setup::TempDir::new().unwrap();
        crate::setup::fetch::unpack(&file, &tmp.0).unwrap();
        assert!(tmp.0.join("bin/win32/swds.dll").is_file());
        assert!(tmp.0.join("bin/linux32/engine_i486.so").is_file());
    }
}
