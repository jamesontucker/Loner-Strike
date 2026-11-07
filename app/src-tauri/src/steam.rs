//! Finds Half-Life installs with czero (Condition Zero) and/or cstrike (CS 1.6).
//! Windows: every Steam library. Mac: Steam inside CrossOver bottles.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Install {
    pub hl: PathBuf,
    /// "windows" or "crossover"
    pub kind: String,
    pub bottle: Option<String>,
    pub games: Vec<String>,
    /// A Loner-Strike dedicated server folder (servers.rs), not a Steam game
    pub dedicated: bool,
}

fn games_in(hl: &Path) -> Vec<String> {
    ["czero", "cstrike"]
        .iter()
        .filter(|g| hl.join(g).join("liblist.gam").is_file())
        .map(|g| g.to_string())
        .collect()
}

/// Library folders listed in Steam's libraryfolders.vdf, plus the main one.
fn libraries(steam_root: &Path) -> Vec<String> {
    let mut out = vec![steam_root.to_string_lossy().to_string()];
    if let Ok(text) = std::fs::read_to_string(steam_root.join("steamapps").join("libraryfolders.vdf")) {
        for line in text.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("\"path\"") {
                let v = rest.trim().trim_matches('"').replace("\\\\", "\\");
                out.push(v);
            }
        }
    }
    out
}

/// The scan, plus Half-Life folders added by hand in Settings.
pub fn find_installs() -> Vec<Install> {
    let mut found = scan();
    for dir in crate::app_api::load_settings().game_folders {
        if let Some(i) = crate::prefs::custom_install(&dir) {
            if !found.iter().any(|f| same_dir(&f.hl, &i.hl)) {
                found.push(i);
            }
        }
    }
    found
}

/// A folder for comparing: Windows paths match whatever their case and slashes (the
/// registry gives c:/program files (x86)/steam, Steam's own list C:\Program Files (x86)\Steam).
pub fn same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    if cfg!(windows) {
        let key = |p: &std::path::Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
        key(a) == key(b)
    } else {
        a == b
    }
}

fn push(found: &mut Vec<Install>, hl: PathBuf, kind: &str, bottle: Option<String>) {
    if hl.is_dir() && !found.iter().any(|i| same_dir(&i.hl, &hl)) {
        let games = games_in(&hl);
        found.push(Install { hl, kind: kind.into(), bottle, games, dedicated: false });
    }
}

#[cfg(target_os = "macos")]
fn scan() -> Vec<Install> {
    let mut found = Vec::new();
    let Some(home) = dirs::home_dir() else { return found };
    let bottles = home.join("Library/Application Support/CrossOver/Bottles");
    let Ok(entries) = std::fs::read_dir(&bottles) else { return found };
    for entry in entries.flatten() {
        let bottle = entry.file_name().to_string_lossy().to_string();
        let drive_c = entry.path().join("drive_c");
        for steam in ["Program Files (x86)/Steam", "Program Files/Steam"] {
            let root = drive_c.join(steam);
            if !root.is_dir() {
                continue;
            }
            for lib in libraries(&root) {
                // Windows paths inside the bottle: only C: maps to drive_c
                let lib = if lib.len() > 3 && lib.as_bytes()[1] == b':' {
                    if !lib.to_lowercase().starts_with('c') {
                        continue;
                    }
                    drive_c.join(lib[3..].replace('\\', "/"))
                } else {
                    PathBuf::from(lib)
                };
                push(&mut found, lib.join("steamapps/common/Half-Life"), "crossover", Some(bottle.clone()));
            }
        }
    }
    found
}

#[cfg(windows)]
fn scan() -> Vec<Install> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let mut roots: Vec<PathBuf> = Vec::new();
    for (hive, key) in [(HKEY_CURRENT_USER, "Software\\Valve\\Steam"), (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Valve\\Steam")] {
        if let Ok(k) = RegKey::predef(hive).open_subkey(key) {
            for name in ["SteamPath", "InstallPath"] {
                if let Ok(v) = k.get_value::<String, _>(name) {
                    // (SteamPath is written with forward slashes)
                    roots.push(PathBuf::from(v.replace('/', "\\")));
                }
            }
        }
    }
    roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
    let mut found = Vec::new();
    for root in roots {
        for lib in libraries(&root) {
            push(&mut found, PathBuf::from(lib).join("steamapps").join("common").join("Half-Life"), "windows", None);
        }
    }
    found
}

#[cfg(not(any(target_os = "macos", windows)))]
fn scan() -> Vec<Install> {
    Vec::new()
}

/// Starts the game through Steam: Steam inside CrossOver on a Mac, Steam on Windows.
/// `args` go to the game (e.g. +map de_dust2 to start a server straight away).
/// Hands Steam a steam:// link (in the game's bottle on a Mac). A running game is told
/// by Steam to act on it.
pub fn open_steam_url(hl: &Path, url: &str) -> Result<(), String> {
    // it goes through a shell on Windows: plain characters only
    if !url.starts_with("steam://") || !url.chars().all(|c| c.is_ascii_alphanumeric() || ":/._-".contains(c)) {
        return Err("That address can't be opened.".into());
    }
    #[cfg(target_os = "macos")]
    {
        let s = hl.to_string_lossy().to_string();
        let i = s.find("/Bottles/").ok_or("The game isn't in a CrossOver bottle.")?;
        let rest = &s[i + 9..];
        let bottle = rest.split('/').next().unwrap_or_default().to_string();
        let drive_c = format!("{}/drive_c/", &s[..i + 9 + bottle.len()]);
        let steam_root = hl.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).ok_or("Couldn't find Steam.")?;
        let rel = steam_root.to_string_lossy().strip_prefix(&drive_c).map(String::from).ok_or("Couldn't find Steam.")?;
        let steam_exe = format!("C:\\{}\\steam.exe", rel.replace('/', "\\"));
        let mut cmd = std::process::Command::new("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine");
        cmd.args(["--bottle", &bottle, &steam_exe, url]);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start CrossOver ({e})."))
    }
    #[cfg(windows)]
    {
        let _ = hl;
        // (no console window flashing up: sys::command hides it)
        let mut cmd = crate::sys::command("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start Steam ({e})."))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (hl, url);
        Err("Launching isn't supported on this system.".into())
    }
}

pub fn launch(hl: &Path, appid: &str, args: &[String]) -> Result<(), String> {
    // on Windows these go through a shell (Steam's URL handler): plain characters only
    if !appid.chars().all(|c| c.is_ascii_digit()) || !args.iter().all(|a| a.chars().all(|c| c.is_ascii_alphanumeric() || "+-_.:/".contains(c))) {
        return Err("Those launch options can't be used.".into());
    }
    #[cfg(target_os = "macos")]
    {
        // .../Bottles/<bottle>/drive_c/<steam>/steamapps/common/Half-Life
        let s = hl.to_string_lossy().to_string();
        let i = s.find("/Bottles/").ok_or("The game isn't in a CrossOver bottle.")?;
        let rest = &s[i + 9..];
        let bottle = rest.split('/').next().unwrap_or_default().to_string();
        let drive_c = format!("{}/drive_c/", &s[..i + 9 + bottle.len()]);
        let steam_root = hl.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).ok_or("Couldn't find Steam.")?;
        let rel = steam_root.to_string_lossy().strip_prefix(&drive_c).map(String::from).ok_or("Couldn't find Steam.")?;
        let steam_exe = format!("C:\\{}\\steam.exe", rel.replace('/', "\\"));
        let mut cmd = std::process::Command::new("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine");
        cmd.args(["--bottle", &bottle, &steam_exe, "-applaunch", appid]).args(args);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start CrossOver ({e})."))
    }
    #[cfg(windows)]
    {
        let _ = hl;
        // Steam itself with -applaunch, like on a Mac (no shell, no console window, no
        // "launch with these options?" question); Steam's URL handler if it can't be found
        if let Some(steam) = steam_exe() {
            let mut cmd = crate::sys::command(&steam);
            cmd.arg("-applaunch").arg(appid).args(args);
            return cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start Steam ({e})."));
        }
        let mut cmd = crate::sys::command("cmd");
        let url = format!("steam://run/{}//{}", appid, args.join(" "));
        cmd.args(["/C", "start", "", &url]);
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start Steam ({e})."))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (hl, appid, args);
        Err("Launching isn't supported on this system.".into())
    }
}

/// steam.exe, from the registry (SteamExe, or InstallPath\\steam.exe).
#[cfg(windows)]
fn steam_exe() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let exe = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok().and_then(|k| k.get_value::<String, _>("SteamExe").ok()).map(|v| PathBuf::from(v.replace('/', "\\")));
    let exe = exe.or_else(|| {
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam").ok().and_then(|k| k.get_value::<String, _>("InstallPath").ok()).map(|v| PathBuf::from(v).join("steam.exe"))
    });
    exe.filter(|p| p.is_file())
}
