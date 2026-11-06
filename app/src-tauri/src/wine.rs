//! Running Windows programs on a Mac: the game, the plugin compiler and dedicated
//! servers all run in the same Wine bottle as Steam. Two ways to have one:
//! - CrossOver (paid): bottles in ~/Library/Application Support/CrossOver/Bottles,
//!   run with `wine --bottle <name>`
//! - Whisky (free): bottles in ~/Library/Containers/com.isaacmarovitz.Whisky/Bottles
//!   (or wherever the player put them), run with Whisky's Wine and WINEPREFIX=<bottle>
//!
//! An install's `kind` is "crossover", "whisky" or "windows"; its `bottle` is the
//! CrossOver bottle's name, or the Whisky bottle's folder.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CROSSOVER_WINE: &str = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";
const WHISKY_ID: &str = "com.isaacmarovitz.Whisky";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// Whisky's Wine (newer builds call it wine64, older ones wine).
pub fn whisky_wine() -> Option<PathBuf> {
    let bin = home().join("Library/Application Support").join(WHISKY_ID).join("Libraries/Wine/bin");
    ["wine64", "wine"].iter().map(|w| bin.join(w)).find(|p| p.is_file())
}

/// Whisky's bottles: the default folder, plus any the player keeps elsewhere
/// (listed in Whisky's settings).
pub fn whisky_bottles() -> Vec<PathBuf> {
    let container = home().join("Library/Containers").join(WHISKY_ID);
    let mut out: Vec<PathBuf> = std::fs::read_dir(container.join("Bottles"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("drive_c").is_dir())
        .collect();
    // other bottle locations are file:// URLs in Whisky's property lists
    for plist in [container.join("BottleVM.plist"), container.join("Data/Library/Preferences").join(format!("{WHISKY_ID}.plist"))] {
        for path in plist_paths(&plist) {
            if path.join("drive_c").is_dir() && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// file:// paths in a property list (binary or XML), via macOS's plutil.
fn plist_paths(plist: &Path) -> Vec<PathBuf> {
    if !plist.is_file() {
        return vec![];
    }
    let Some((true, json, _)) = crate::sys::run_output(
        {
            let mut c = crate::sys::command("/usr/bin/plutil");
            c.args(["-convert", "json", "-o", "-"]).arg(plist);
            c
        },
        std::time::Duration::from_secs(5),
    ) else {
        return vec![];
    };
    let mut out = Vec::new();
    for part in json.split('"') {
        let p = part.replace("\\/", "/");
        if let Some(rest) = p.strip_prefix("file://") {
            let decoded = percent_decode(rest.trim_end_matches('/'));
            out.push(PathBuf::from(decoded));
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Which bottle a path is in: ("crossover", name) or ("whisky", bottle folder).
pub fn bottle_of(path: &Path) -> Option<(String, String)> {
    let s = path.to_string_lossy();
    let i = s.find("/drive_c/").or_else(|| s.ends_with("/drive_c").then(|| s.len() - 8))?;
    let prefix = &s[..i];
    if let Some(j) = prefix.find("/CrossOver/Bottles/") {
        let name = &prefix[j + 19..];
        return (!name.is_empty() && !name.contains('/')).then(|| ("crossover".to_string(), name.to_string()));
    }
    Some(("whisky".to_string(), prefix.to_string()))
}

/// .../drive_c/Some Folder/x.exe -> C:\Some Folder\x.exe
pub fn windows_path(p: &Path) -> Option<String> {
    let s = p.to_string_lossy();
    let i = s.find("/drive_c/")?;
    Some(format!("C:\\{}", s[i + 9..].replace('/', "\\")))
}

/// What's missing to run things in this kind of bottle, if anything.
pub fn missing(kind: &str) -> Option<&'static str> {
    match kind {
        "crossover" if !Path::new(CROSSOVER_WINE).is_file() => Some("CrossOver isn't installed in Applications."),
        "whisky" if whisky_wine().is_none() => Some("Whisky's Wine isn't installed. Open Whisky once so it finishes setting itself up."),
        _ => None,
    }
}

/// A command that runs a Windows program (`exe`, a Mac path or C:\ path) in the
/// install's bottle; on Windows, the program itself. `wait_children` keeps
/// CrossOver's launcher running until everything the program started has finished.
pub fn command(kind: &str, bottle: Option<&str>, exe: impl AsRef<OsStr>, wait_children: bool) -> Result<Command, String> {
    if let Some(why) = missing(kind) {
        return Err(why.into());
    }
    match kind {
        "crossover" => {
            let mut c = crate::sys::command(CROSSOVER_WINE);
            c.arg("--bottle").arg(bottle.unwrap_or("Steam"));
            if wait_children {
                c.arg("--wait-children");
            }
            c.arg(exe);
            Ok(c)
        }
        "whisky" => {
            let prefix = bottle.ok_or("Couldn't tell which Whisky bottle the game is in.")?;
            let mut c = crate::sys::command(whisky_wine().unwrap_or_default());
            c.env("WINEPREFIX", prefix).env("WINEDEBUG", "-all").arg(exe);
            Ok(c)
        }
        _ => Ok(crate::sys::command(exe)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottles_from_paths() {
        let cx = Path::new("/Users/x/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steam.exe");
        assert_eq!(bottle_of(cx), Some(("crossover".into(), "Steam".into())));
        let wh = Path::new("/Users/x/Library/Containers/com.isaacmarovitz.Whisky/Bottles/1A2B/drive_c/Program Files (x86)/Steam");
        assert_eq!(bottle_of(wh), Some(("whisky".into(), "/Users/x/Library/Containers/com.isaacmarovitz.Whisky/Bottles/1A2B".into())));
        let custom = Path::new("/Volumes/Games/My Bottle/drive_c/Loner-Strike Servers/a/hlds.exe");
        assert_eq!(bottle_of(custom).unwrap().1, "/Volumes/Games/My Bottle");
        assert_eq!(bottle_of(Path::new("/Users/x/Games/hlds")), None);
        assert_eq!(windows_path(cx).unwrap(), "C:\\Program Files (x86)\\Steam\\steam.exe");
        assert_eq!(percent_decode("/Volumes/My%20Games/B"), "/Volumes/My Games/B");
    }
}
