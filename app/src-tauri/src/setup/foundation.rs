//! The foundation: Metamod-P, AMX Mod X (base + Counter-Strike addon) and ReGameDLL
//! (Loner-Strike's fork: the bots' changes). Dedicated servers also get REHLDS (Loner-Strike's
//! fork of the engine: voice awareness for the bots), ReAPI and relocalizebugfix.
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
    url: "https://github.com/jamesontucker/ReGameDLL_CS/releases/download/5.30.0.814-ls13/regamedll-bin-5.30.0.814-ls13.zip",
    sha256: "475b1ce22707ba69feb5f00ae072dad3a29357c75bb05f23d0f229e147b89faa",
    what: "ReGameDLL",
};
/// Loner-Strike's fork of ReHLDS (github.com/jamesontucker/ReHLDS, branch loner-strike): the
/// engine for dedicated servers
pub const REHLDS: Pinned = Pinned {
    url: "https://github.com/jamesontucker/ReHLDS/releases/download/3.15.0.896-ls5/rehlds-bin-3.15.0.896-ls5.zip",
    sha256: "ea18c64730f8b9df88b2510e446f6de7d186b50224b3b5ecc76daf68b56f49a8",
    what: "ReHLDS",
};
pub const REAPI: Pinned = Pinned {
    url: "https://github.com/rehlds/ReAPI/releases/download/5.29.0.358/reapi-bin-5.29.0.358.zip",
    sha256: "f33a7435540bea8706db3fa948e51a85b511f5b18c383b97402a204dc5419195",
    what: "ReAPI",
};
pub const RELOCALIZEBUGFIX: Pinned = Pinned {
    url: "https://github.com/rehlds/relocalizebugfix/releases/download/v2.8/relocalizebugfix-v2.8.zip",
    sha256: "262c4f021e4863fd772fcdbe48689f171426d33321ff0f2c9787eb083cacb1b0",
    what: "relocalizebugfix",
};
pub const METAMOD_DLL: &str = r"addons\metamod\dlls\metamod.dll";

/// AMX Mod X files a player may have customised: never replaced if present
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

/// A file the game already had that Loner-Strike didn't put there.
fn theirs(rec: &Record, rel: &str) -> bool {
    rec.exists(rel) && !rec.ours(rel)
}

/// Whether a file contains `needle` (to recognise ReGameDLL and Metamod builds).
fn mentions(rec: &Record, rel: &str, needle: &[u8]) -> bool {
    rec.full(rel).ok().and_then(|p| std::fs::read(p).ok()).map(|d| d.windows(needle.len()).any(|w| w == needle)).unwrap_or(false)
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
    for (p, sub) in [(&AMXX_BASE, "amxx_base"), (&AMXX_CSTRIKE, "amxx_cstrike")] {
        let src = get(p, ctx, tmp, sub)?;
        ctx.progress.detail("Copying AMX Mod X into the game");
        for (full, rel) in tree(&src) {
            if !rel.starts_with("addons/") {
                continue;
            }
            if KEEP_IF_PRESENT.iter().any(|k| rel.starts_with(k)) && rec.exists(&rel) && !rec.ours(&rel) {
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

/// ReGameDLL replaces the game's own server code (mp.dll); the original is backed up.
pub fn regamedll(rec: &mut Record, ctx: &Ctx, tmp: &TempDir) -> Result<(), String> {
    let rg = get(&REGAMEDLL, ctx, tmp, "rg")?;
    // a server that already runs ReGameDLL (maybe a newer one) keeps its own
    if theirs(rec, "dlls/mp.dll") && mentions(rec, "dlls/mp.dll", b"ReGameDLL") {
        ctx.progress.note("Kept the ReGameDLL that was already there.");
    } else {
        rec.copy(&need(rg.join("bin/win32/cstrike/dlls/mp.dll"), "ReGameDLL")?, "dlls/mp.dll")?;
    }
    for cfg in ["game.cfg", "game_init.cfg"] {
        if !rec.exists(cfg) {
            rec.copy(&need(rg.join("bin/win32/cstrike").join(cfg), "ReGameDLL")?, cfg)?;
        }
    }
    Ok(())
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
}
