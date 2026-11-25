//! Finds Half-Life installs with czero (Condition Zero) and/or cstrike (CS 1.6).
//! Windows: every Steam library. Mac: Steam inside CrossOver bottles. Linux: every library of
//! the native Steam (its usual folder, and the Flatpak and Snap ones).

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Install {
    pub hl: PathBuf,
    /// "windows", "crossover" or "linux" (the native Linux game: hl_linux and .so files)
    pub kind: String,
    pub bottle: Option<String>,
    pub games: Vec<String>,
    /// A Loner-Strike dedicated server folder (servers.rs), not a Steam game
    pub dedicated: bool,
}

/// A copy of the game from before Valve's 2013 update (an old retail or repacked one), which
/// Loner-Strike's mods (ReGameDLL, Metamod-P, AMX Mod X 1.10) can't load on: its own game
/// (hl.exe there) without the SDL2.dll that update added. Dedicated server folders aren't asked.
pub fn too_old(hl: &Path) -> bool {
    // (a Linux copy has hl_linux, and SDL2 under its own name)
    hl.join("hl.exe").is_file() && !hl.join("SDL2.dll").is_file() && !hl.join("hl_linux").is_file()
}

pub const TOO_OLD: &str = "This copy of the game is older than Loner-Strike supports (from before Valve's 2013 update), so its mods can't load on it. Use the current Steam version of Counter-Strike 1.6 or Condition Zero.";


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
        out.extend(library_paths(&text));
    }
    out
}

/// The library folders in libraryfolders.vdf's text: `"path" "D:\\Lib"` (Steam since 2021)
/// and the older `"1" "D:\\Lib"` (numbered keys whose value is a path).
pub fn library_paths(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in text.lines() {
        let t = line.trim();
        let mut q = t.split('"').skip(1).step_by(2);
        let (Some(k), Some(v)) = (q.next(), q.next()) else { continue };
        let numbered = !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit());
        let looks_like_path = v.contains(":\\") || v.starts_with("\\\\") || v.starts_with('/');
        if k.eq_ignore_ascii_case("path") || (numbered && looks_like_path) {
            let v = v.replace("\\\\", "\\");
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

#[cfg(test)]
mod library_tests {
    #[test]
    fn both_libraryfolders_formats() {
        let new = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        assert_eq!(super::library_paths(new), vec![r"C:\Program Files (x86)\Steam".to_string(), r"D:\SteamLibrary".to_string()]);
        let old = "\"LibraryFolders\"\n{\n\t\"TimeNextStatsReport\"\t\t\"1600000000\"\n\t\"ContentStatsID\"\t\t\"-123\"\n\t\"1\"\t\t\"E:\\\\Games\\\\Steam\"\n}";
        assert_eq!(super::library_paths(old), vec![r"E:\Games\Steam".to_string()]);
        let unicode = "\"1\"\n{\n\t\"path\"\t\t\"D:\\\\Игры\\\\Steam\"\n}";
        assert_eq!(super::library_paths(unicode), vec![r"D:\Игры\Steam".to_string()]);
    }
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

/// The Steam folders on Linux: the usual one (~/.steam/steam links to it), Flatpak's and
/// Snap's. The same folder reached two ways counts once.
#[cfg(target_os = "linux")]
pub fn linux_steam_roots() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    let mut out: Vec<PathBuf> = vec![];
    for rel in [".steam/steam", ".local/share/Steam", ".steam/root", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/.steam/steam", "snap/steam/common/.local/share/Steam"] {
        let p = home.join(rel);
        if !p.join("steamapps").is_dir() {
            continue;
        }
        let real = std::fs::canonicalize(&p).unwrap_or(p);
        if !out.contains(&real) {
            out.push(real);
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn scan() -> Vec<Install> {
    let mut found = Vec::new();
    for root in linux_steam_roots() {
        for lib in libraries(&root) {
            let lib = std::fs::canonicalize(&lib).unwrap_or_else(|_| PathBuf::from(&lib));
            push(&mut found, lib.join("steamapps/common/Half-Life"), "linux", None);
        }
    }
    found
}

/// How Steam is started on Linux for a game in `hl`: Flatpak's Steam through flatpak, else the
/// `steam` command.
#[cfg(target_os = "linux")]
fn linux_steam_command(hl: &Path) -> std::process::Command {
    if hl.to_string_lossy().contains("/.var/app/com.valvesoftware.Steam/") {
        let mut c = std::process::Command::new("flatpak");
        c.args(["run", "com.valvesoftware.Steam"]);
        c
    } else {
        std::process::Command::new("steam")
    }
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
    #[cfg(target_os = "linux")]
    {
        let mut cmd = linux_steam_command(hl);
        cmd.arg(url);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start Steam ({e})."))
    }
}

pub fn launch(hl: &Path, appid: &str, args: &[String]) -> Result<(), String> {
    // on Windows these go through a shell (Steam's URL handler): plain characters only
    if !appid.chars().all(|c| c.is_ascii_digit()) || !args.iter().all(|a| a.chars().all(|c| c.is_ascii_alphanumeric() || "+-_.:/".contains(c))) {
        return Err("Those launch options can't be used.".into());
    }
    if crate::backup::restoring_game(hl) {
        return Err(crate::backup::RESTORING_GAME.into());
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
    #[cfg(target_os = "linux")]
    {
        // Steam that's already running is handed the launch and this one quits
        let mut cmd = linux_steam_command(hl);
        cmd.arg("-applaunch").arg(appid).args(args);
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        cmd.spawn().map(crate::sys::reap).map_err(|e| format!("Couldn't start Steam ({e}). Is Steam installed?"))
    }
}

/// The Steam name of who's signed in to Steam here, to fill in for shared control (they can
/// change it). Steam's own last-used name, else the most recent account's in loginusers.vdf.
#[cfg(windows)]
pub fn persona_name() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok()?;
    let clean = |s: String| Some(s.trim().chars().filter(|c| !c.is_control()).take(32).collect::<String>()).filter(|s| !s.is_empty());
    if let Some(n) = key.get_value::<String, _>("LastGameNameUsed").ok().and_then(clean) {
        return Some(n);
    }
    let root = PathBuf::from(key.get_value::<String, _>("SteamPath").ok()?.replace('/', "\\"));
    let text = std::fs::read_to_string(root.join("config").join("loginusers.vdf")).ok()?;
    persona_in(&text).and_then(clean)
}

#[cfg(target_os = "linux")]
pub fn persona_name() -> Option<String> {
    let clean = |s: String| Some(s.trim().chars().filter(|c| !c.is_control()).take(32).collect::<String>()).filter(|s| !s.is_empty());
    linux_steam_roots().into_iter().find_map(|r| std::fs::read_to_string(r.join("config/loginusers.vdf")).ok().and_then(|t| persona_in(&t)).and_then(clean))
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn persona_name() -> Option<String> {
    None
}

/// The Steam account this computer's Steam last signed in with, as an in-game SteamID
/// ("STEAM_0:0:12345"), from loginusers.vdf: the Steam that has the game at `hl` first
/// (.../steamapps/common/Half-Life), then this computer's usual Steam folders (Linux's,
/// Windows' from the registry, the Mac's CrossOver bottles).
pub fn own_steamid(hl: Option<&Path>) -> Option<String> {
    let mut roots: Vec<PathBuf> = hl.and_then(|h| h.ancestors().nth(3)).map(Path::to_path_buf).into_iter().collect();
    roots.extend(steam_roots_here());
    roots.into_iter().find_map(|r| std::fs::read_to_string(r.join("config").join("loginusers.vdf")).ok().and_then(|t| steamid_in(&t)))
}

fn steam_roots_here() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        linux_steam_roots()
    }
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Valve\\Steam")
            .ok()
            .and_then(|k| k.get_value::<String, _>("SteamPath").ok())
            .map(|p| PathBuf::from(p.replace('/', "\\")))
            .into_iter()
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        let Some(home) = dirs::home_dir() else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(home.join("Library/Application Support/CrossOver/Bottles")) else { return Vec::new() };
        entries
            .flatten()
            .flat_map(|b| ["Program Files (x86)/Steam", "Program Files/Steam"].map(|s| b.path().join("drive_c").join(s)))
            .filter(|p| p.is_dir())
            .collect()
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// The SteamID of the account marked MostRecent "1" (else the first) in loginusers.vdf, whose
/// accounts are keyed by their 64-bit id ("76561198...").
pub(crate) fn steamid_in(vdf: &str) -> Option<String> {
    let (mut first, mut current, mut recent) = (None, None::<u64>, None);
    for line in vdf.lines() {
        let t = line.trim();
        if let Some(id) = t.strip_prefix('"').and_then(|r| r.strip_suffix('"')).filter(|r| r.len() == 17 && r.bytes().all(|b| b.is_ascii_digit())).and_then(|r| r.parse::<u64>().ok()) {
            first.get_or_insert(id);
            current = Some(id);
        }
        let key = t.strip_prefix("\"MostRecent\"").map(|r| r.trim().trim_matches('"').to_string());
        if key.as_deref() == Some("1") {
            recent = current;
        }
    }
    steam2(recent.or(first)?)
}

/// 76561198056158890 -> STEAM_0:0:47946581
pub(crate) fn steam2(id64: u64) -> Option<String> {
    let account = id64.checked_sub(76_561_197_960_265_728).filter(|a| *a > 0 && *a <= u32::MAX as u64)?;
    Some(format!("STEAM_0:{}:{}", account & 1, account >> 1))
}

/// The PersonaName of the account marked MostRecent "1" (else the first) in loginusers.vdf.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn persona_in(vdf: &str) -> Option<String> {
    let value = |line: &str, key: &str| -> Option<String> {
        let rest = line.trim().strip_prefix(&format!("\"{key}\""))?.trim();
        Some(rest.trim_matches('"').to_string())
    };
    let (mut first, mut current, mut recent) = (None, None::<String>, None);
    for line in vdf.lines() {
        if let Some(n) = value(line, "PersonaName") {
            first.get_or_insert(n.clone());
            current = Some(n);
        }
        if value(line, "MostRecent").as_deref() == Some("1") {
            recent = current.clone();
        }
    }
    recent.or(first)
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

#[cfg(test)]
mod too_old_tests {

    /// The owner's SteamID from loginusers.vdf: the MostRecent account, else the first
    #[test]
    fn own_steamid_from_loginusers() {
        let vdf = "\"users\"\n{\n\t\"76561197960265729\"\n\t{\n\t\t\"PersonaName\"\t\t\"other\"\n\t\t\"MostRecent\"\t\t\"0\"\n\t}\n\t\"76561198056158890\"\n\t{\n\t\t\"PersonaName\"\t\t\"nanoshrine\"\n\t\t\"MostRecent\"\t\t\"1\"\n\t}\n}\n";
        assert_eq!(super::steamid_in(vdf).as_deref(), Some("STEAM_0:0:47946581"));
        assert_eq!(super::steamid_in(&vdf.replace("\"MostRecent\"\t\t\"1\"", "\"MostRecent\"\t\t\"0\"")).as_deref(), Some("STEAM_0:1:0"));
        assert_eq!(super::steamid_in("nothing here"), None);
        assert_eq!(super::steam2(76561197960265728), None);
    }
    #[test]
    fn spots_a_pre_2013_copy() {
        let d = std::env::temp_dir().join(format!("ls-tooold-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // a dedicated server's folder (no hl.exe): never asked
        assert!(!super::too_old(&d));
        std::fs::write(d.join("hl.exe"), b"").unwrap();
        assert!(super::too_old(&d), "the game without SDL2.dll: from before 2013");
        std::fs::write(d.join("SDL2.dll"), b"").unwrap();
        assert!(!super::too_old(&d));
        let _ = std::fs::remove_dir_all(&d);
    }
}
