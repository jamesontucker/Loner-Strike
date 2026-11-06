//! "Get more" beyond maps: weapon skins, player models, sounds, sprays, sprites and
//! HUD/menu art from GameBanana's Counter-Strike 1.6 sections.
//!
//! Unlike maps, these replace game files (models/v_ak47.mdl...). They go into the game
//! folder the user plays (czero or cstrike), only from a strict allow-list per section
//! (models, sounds, sprites, images, the sprite/HUD text files and two menu layout files:
//! nothing that runs code or changes settings). Every file replaced is backed up first,
//! and the record keeps the backups stacked: removing a download puts back exactly what
//! was there before it, even when a later download replaced the same file.
//!
//! Record: <app dir>/content/record.json, backups in <app dir>/content/backup/<key>/.

use crate::archive::{self, ext_of};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct Section {
    pub key: &'static str,
    pub label: &'static str,
    /// GameBanana item type: Mod, Sound or Spray
    pub kind: &'static str,
    /// Category filters; id 0 means "everything of this type for CS 1.6"
    pub cats: &'static [(i64, &'static str)],
}

pub const CS16_GAME: i64 = 4254;

pub const SECTIONS: &[Section] = &[
    Section { key: "maps", label: "Maps", kind: "Mod", cats: crate::gamebanana::CATEGORIES },
    Section {
        key: "skins",
        label: "Weapon skins",
        kind: "Mod",
        cats: &[
            (8310, "All skins"), (8320, "Rifles"), (8316, "Pistols"), (8314, "SMGs"), (8330, "Shotguns"),
            (8350, "M249"), (8311, "Knives"), (8318, "Equipment"), (8329, "Arms"), (8322, "Packs"),
            (8338, "Map props"), (8357, "Other"),
        ],
    },
    Section { key: "players", label: "Player models", kind: "Mod", cats: &[(12761, "Player models")] },
    Section {
        key: "sounds",
        label: "Sounds",
        kind: "Sound",
        cats: &[
            (0, "All sounds"), (409, "Packs"), (2200, "AK-47"), (2203, "M4A1"), (2208, "AWP"), (415, "Desert Eagle"),
            (2201, "AUG"), (2516, "Dual Elites"), (2202, "FAMAS"), (2212, "Five-Seven"), (2216, "G3/SG-1"),
            (2214, "Galil"), (2209, "Glock"), (2173, "Grenades"), (5143, "Knife"), (2215, "M249"), (2230, "M3"),
            (2213, "MAC-10"), (2204, "MP5"), (2206, "P90"), (2229, "Scout"), (2308, "Sig 550"), (2211, "Sig 552"),
            (3808, "Sig P228"), (3807, "TMP"), (2205, "UMP45"), (2210, "USP"), (3809, "XM1014"),
            (2217, "Other weapons"), (2538, "C4"), (2016, "Ambience"), (5460, "Footsteps"), (3804, "Steps"),
            (2052, "Player/Death"), (413, "Radio"), (2207, "Round win"), (5251, "Pick up"), (2420, "Spray"),
            (2422, "User interface"), (401, "Game startup"), (408, "Other"),
        ],
    },
    Section {
        key: "sprays",
        label: "Sprays",
        kind: "Spray",
        cats: &[
            (0, "All sprays"), (610, "Logos"), (628, "Funny"), (617, "Anime art"), (626, "Flags"), (2716, "Music/Bands"),
            (2301, "Weapons"), (2692, "Decoys"), (2549, "Girls"), (622, "Other"),
        ],
    },
    Section {
        key: "sprites",
        label: "Effects & sprites",
        kind: "Mod",
        cats: &[
            (1247, "All sprites"), (1249, "Muzzle flash"), (1248, "Scopes"), (1251, "Blood"), (1262, "Bullet holes"),
            (1263, "Smoke"), (1255, "Explosions"), (1252, "C4 LEDs"), (1260, "Voice icons"), (1256, "Radio icons"),
            (1259, "Pain indicators"), (1264, "Shadows"), (1258, "Packs"), (1253, "Other"),
        ],
    },
    Section {
        key: "gui",
        label: "HUD & menus",
        kind: "Mod",
        cats: &[(1828, "HUDs"), (1831, "Menu backgrounds"), (1839, "Headshot icons"), (1825, "Other")],
    },
];

pub fn section(key: &str) -> Option<&'static Section> {
    SECTIONS.iter().find(|s| s.key == key)
}

/// Record key: the GameBanana item and the game folder ("Mod:223124:1a2b3c4d"), so the
/// same item can't be installed twice through two sections, and installing it into
/// CS 1.6 and Condition Zero keeps two records and two sets of backups.
pub fn record_key(sec: &Section, id: i64, game: &Path) -> String {
    format!("{}:{id}:{}", sec.kind, crate::sounds::folder_tag(game))
}

/// The GameBanana id in a record key.
pub fn key_id(key: &str) -> i64 {
    key.split(':').nth(1).and_then(|n| n.parse().ok()).unwrap_or(0)
}

// ---- what may go where -------------------------------------------------------------

const ROOTS: &[&str] = &["models", "sound", "sprites", "gfx", "resource", "logos", "media"];
const LAYOUTS: &[&str] = &["resource/backgroundlayout.txt", "resource/backgroundloadinglayout.txt"];

/// The allow-list: may `dest` (relative to the game folder, any case) come from a download in this section?
pub fn allowed(section: &str, dest: &str) -> bool {
    let d = dest.to_lowercase();
    let p: Vec<&str> = d.split('/').collect();
    let ext = ext_of(p.last().unwrap_or(&""));
    let top = p[0];
    let image = ext == ".tga" || ext == ".bmp";
    let vgui = top == "gfx" && p.len() >= 3 && p[1] == "vgui" && image;
    let sprite = top == "sprites" && p.len() >= 2 && (ext == ".spr" || (ext == ".txt" && p.len() == 2));
    match section {
        "skins" | "players" => (top == "models" && p.len() >= 2 && ext == ".mdl") || (top == "sound" && p.len() >= 2 && ext == ".wav") || (top == "sprites" && p.len() >= 2 && ext == ".spr") || vgui,
        "sounds" => (top == "sound" && p.len() >= 2 && ext == ".wav") || d == "media/gamestartup.mp3",
        "sprays" => d == "tempdecal.wad" || (top == "logos" && p.len() == 2 && ext == ".bmp"),
        "sprites" => sprite || d == "decals.wad",
        "gui" => {
            sprite
                || vgui
                || LAYOUTS.contains(&d.as_str())
                || (top == "resource" && ext == ".tga" && (p.len() == 2 || (p.len() == 3 && p[1] == "background")))
        }
        _ => false,
    }
}

/// File and folder names we're willing to write: plain characters, no dot-files, no "..".
pub fn safe_part(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 64
        && !p.starts_with('.')
        && !p.ends_with(' ')
        && !p.ends_with('.')
        && p.chars().all(|c| c.is_ascii_alphanumeric() || " _.-()+!#&',".contains(c))
}

/// Files already in the game, by lower-case file name, for downloads that come as
/// loose files ("ak47-1.wav" goes where the game's own ak47-1.wav is).
#[derive(Default)]
pub struct Existing {
    by_name: HashMap<String, Vec<String>>,
}

impl Existing {
    /// Indexes models/sound/sprites/gfx/resource in the game folder and its fallbacks.
    pub fn scan(dirs: &[PathBuf]) -> Existing {
        let mut e = Existing::default();
        for d in dirs {
            for top in ["models", "sound", "sprites", "gfx", "resource"] {
                let root = d.join(top);
                for f in archive::walk(&root) {
                    let parts = archive::rel_parts(d, &f);
                    e.add(&parts.join("/"));
                }
            }
        }
        e
    }

    pub fn add(&mut self, rel: &str) {
        let name = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
        let v = self.by_name.entry(name).or_default();
        if !v.iter().any(|x| x.eq_ignore_ascii_case(rel)) {
            v.push(rel.to_string());
        }
    }

    /// Where the game keeps a file of this name, if there's exactly one place.
    fn unique(&self, name: &str, under: &str) -> Result<Option<String>, ()> {
        let hits: Vec<&String> = self
            .by_name
            .get(&name.to_lowercase())
            .map(|v| v.iter().filter(|r| r.to_lowercase().starts_with(&format!("{under}/"))).collect())
            .unwrap_or_default();
        match hits.len() {
            0 => Ok(None),
            1 => Ok(Some(hits[0].clone())),
            _ => Err(()),
        }
    }
}

/// [(index into rels, destination relative to the game folder)] and what was left out.
pub fn plan(section: &str, rels: &[Vec<String>], existing: &Existing) -> (Vec<(usize, String)>, Vec<String>) {
    let (mut plan, mut seen, mut skipped) = (Vec::new(), HashSet::new(), Vec::new());
    for (index, parts) in rels.iter().enumerate() {
        if parts.is_empty() {
            continue;
        }
        let rel = parts.join("/");
        let low: Vec<String> = parts.iter().map(|p| p.to_lowercase()).collect();
        let name = low.last().unwrap().clone();
        let file = parts.last().unwrap().clone();
        let ext = ext_of(&name).to_string();
        let parent = if low.len() >= 2 { Some(low[low.len() - 2].as_str()) } else { None };

        // inside a game folder layout: the outermost root folder that gives an allowed path
        let mut dest = (0..low.len() - 1).filter(|&i| ROOTS.contains(&low[i].as_str())).find_map(|i| {
            let d = std::iter::once(low[i].clone()).chain(parts[i + 1..].iter().cloned()).collect::<Vec<_>>().join("/");
            allowed(section, &d).then_some(d)
        });
        let mut note = "";
        if dest.is_none() {
            // loose files
            let near = |under: &str| existing.unique(&name, under);
            dest = match ext.as_str() {
                _ if name == "tempdecal.wad" || name == "decals.wad" => Some(name.clone()),
                ".mdl" if ["v_", "p_", "w_"].iter().any(|p| name.starts_with(p)) => Some(format!("models/{file}")),
                ".mdl" => match near("models") {
                    Ok(Some(d)) => Some(d),
                    Ok(None) => parent
                        .filter(|p| safe_part(p) && (name == format!("{p}.mdl") || name == format!("{p}t.mdl")))
                        .map(|p| format!("models/player/{p}/{file}")),
                    Err(()) => {
                        note = " (the game has several files with that name)";
                        None
                    }
                },
                ".spr" => match near("sprites") {
                    Ok(Some(d)) => Some(d),
                    Ok(None) => Some(format!("sprites/{file}")),
                    Err(()) => Some(format!("sprites/{file}")),
                },
                ".wav" => match near("sound") {
                    Ok(d) => d,
                    Err(()) => {
                        note = " (the game has several sounds with that name)";
                        None
                    }
                },
                ".bmp" if section == "sprays" => Some(format!("logos/{file}")),
                ".tga" | ".bmp" => near("resource").ok().flatten().or_else(|| near("gfx").ok().flatten()),
                ".txt" if name == "backgroundlayout.txt" || name == "backgroundloadinglayout.txt" => {
                    Some(format!("resource/{file}"))
                }
                ".txt" if name == "hud.txt" || name.starts_with("weapon_") => Some(format!("sprites/{file}")),
                _ => None,
            };
        }
        let Some(dest) = dest.filter(|d| allowed(section, d)) else {
            if !archive::is_archive(&name) {
                skipped.push(format!("{rel}{note}"));
            }
            continue;
        };
        if !dest.split('/').all(safe_part) {
            skipped.push(format!("{rel} (name has characters the app doesn't install)"));
            continue;
        }
        if !seen.insert(dest.to_lowercase()) {
            skipped.push(format!("{rel} (another copy in the download was used)"));
            continue;
        }
        plan.push((index, dest));
    }
    skipped.truncate(40);
    (plan, skipped)
}

/// `rel` under `base`, matching the case of what's already on disk (the game is
/// Windows software: models/V_AK47.mdl and models/v_ak47.mdl are the same file).
pub fn resolve(base: &Path, rel: &str) -> (PathBuf, String, bool) {
    let mut path = base.to_path_buf();
    let mut out = Vec::new();
    let mut exists = true;
    for part in rel.split('/') {
        let found = if exists {
            std::fs::read_dir(&path)
                .ok()
                .and_then(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).find(|n| n.eq_ignore_ascii_case(part)))
        } else {
            None
        };
        let name = found.unwrap_or_else(|| {
            exists = false;
            part.to_string()
        });
        path.push(&name);
        out.push(name);
    }
    (path, out.join("/"), exists)
}

// ---- the record ------------------------------------------------------------------------

pub struct Records {
    pub dir: PathBuf,
}

impl Records {
    pub fn new(app: &Path) -> Records {
        Records { dir: app.join("content") }
    }

    fn path(&self) -> PathBuf {
        self.dir.join("record.json")
    }

    pub fn load(&self) -> Map<String, Value> {
        let rec = crate::sys::read_json_map(&self.path());
        // records from before keys named their game folder ("Mod:123"); saved that way next time.
        // Their backups stay where they are: each file entry names its own backup path.
        rec.into_iter()
            .map(|(k, r)| match (k.matches(':').count(), r["game"].as_str()) {
                (1, Some(g)) => (format!("{k}:{}", crate::sounds::folder_tag(Path::new(g))), r),
                _ => (k, r),
            })
            .collect()
    }

    pub fn save(&self, rec: &Map<String, Value>) -> std::io::Result<()> {
        crate::sys::write_atomic(&self.path(), serde_json::to_string_pretty(rec).unwrap_or_default().as_bytes())
    }

    fn backup_root(&self, key: &str) -> PathBuf {
        self.dir.join("backup").join(key.replace(':', "-"))
    }

    /// Installed downloads for this game folder.
    pub fn for_game(&self, game: &Path) -> Map<String, Value> {
        let g = game.to_string_lossy();
        self.load().into_iter().filter(|(_, r)| r["game"] == *g).collect()
    }

    /// Which installed download (key, name) currently owns each file in this game folder (the latest wins).
    pub fn owners(&self, game: &Path) -> HashMap<String, (String, String)> {
        let mut recs: Vec<(String, Value)> = self.for_game(game).into_iter().collect();
        recs.sort_by_key(|(_, r)| r["seq"].as_i64().unwrap_or(0));
        let mut out = HashMap::new();
        for (k, r) in recs {
            for f in r["files"].as_array().into_iter().flatten() {
                if let Some(p) = f["path"].as_str() {
                    out.insert(p.to_lowercase(), (k.clone(), r["name"].as_str().unwrap_or("").to_string()));
                }
            }
        }
        out
    }

    /// Copies the planned files into the game, backing up what they replace.
    /// Returns the record entry's files and the paths that replaced something.
    /// On any failure, everything done so far is undone.
    pub fn install(&self, game: &Path, key: &str, files: &[(PathBuf, String)]) -> Result<(Vec<Value>, Vec<String>), String> {
        let broot = self.backup_root(key);
        let _ = std::fs::remove_dir_all(&broot);
        let mut done: Vec<Value> = Vec::new();
        let mut replaced = Vec::new();
        let result = (|| -> Result<(), String> {
            for (src, dest) in files {
                let (target, actual, exists) = resolve(game, dest);
                if exists && archive::same_file(src, &target) {
                    continue; // already exactly this
                }
                let mut backup = Value::Null;
                if exists {
                    if !target.is_file() {
                        return Err(format!("{actual} is a folder in the game, so it wasn't replaced."));
                    }
                    let b = broot.join(&actual);
                    std::fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
                    std::fs::copy(&target, &b).map_err(|e| format!("Couldn't back up {actual} ({e})."))?;
                    backup = json!(format!("{}/{actual}", key.replace(':', "-")));
                }
                if let Some(d) = target.parent() {
                    std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                }
                // pushed before the copy so a half-written file is undone too
                done.push(json!({"path": actual, "backup": backup}));
                replace_with(src, &target).map_err(|e| format!("Couldn't write {actual} ({e})."))?;
                if exists {
                    replaced.push(actual);
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            for f in done.iter().rev() {
                self.put_back(game, f);
            }
            let _ = std::fs::remove_dir_all(&broot);
            return Err(e);
        }
        Ok((done, replaced))
    }

    /// Puts back what a file entry replaced (or removes the file if it was new).
    fn put_back(&self, game: &Path, f: &Value) {
        let Some(rel) = f["path"].as_str() else { return };
        let Some(target) = inside(game, rel) else { return };
        match f["backup"].as_str().and_then(|b| inside(&self.dir.join("backup"), b)) {
            Some(b) => {
                if replace_with(&b, &target).is_ok() {
                    let _ = std::fs::remove_file(&b);
                }
            }
            None => {
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::remove_file(target.with_extension("ls-tmp"));
                prune(target.parent(), game);
            }
        }
    }

    /// Undoes an install whose record couldn't be saved.
    pub fn uninstall_entry(&self, game: &Path, entry: &Value) -> Result<(), String> {
        for f in entry["files"].as_array().into_iter().flatten().rev() {
            self.put_back(game, f);
        }
        Ok(())
    }

    /// Removes an installed download. For each of its files: if a later download
    /// replaced it since, that download's backup becomes what this one had replaced
    /// (the disk is left alone); otherwise this download's backup is put back.
    pub fn uninstall(&self, key: &str) -> Result<Value, String> {
        let mut rec = self.load();
        let r = rec.remove(key).ok_or("That download isn't installed from the app.")?;
        let game = PathBuf::from(r["game"].as_str().unwrap_or(""));
        let seq = r["seq"].as_i64().unwrap_or(0);
        for f in r["files"].as_array().into_iter().flatten() {
            let Some(path) = f["path"].as_str() else { continue };
            // the next download above this one that also wrote this file
            let above = rec
                .iter()
                .filter(|(_, o)| o["game"] == r["game"] && o["seq"].as_i64().unwrap_or(0) > seq)
                .filter_map(|(k, o)| {
                    let i = o["files"].as_array()?.iter().position(|g| g["path"].as_str().map(|p| p.eq_ignore_ascii_case(path)).unwrap_or(false))?;
                    Some((o["seq"].as_i64().unwrap_or(0), k.clone(), i))
                })
                .min();
            match above {
                Some((_, k, i)) => {
                    let backups = self.dir.join("backup");
                    let theirs = rec[&k]["files"][i]["backup"].as_str().and_then(|b| inside(&backups, b));
                    if let Some(t) = &theirs {
                        let _ = std::fs::remove_file(t);
                    }
                    let mine = f["backup"].as_str().and_then(|b| inside(&backups, b));
                    let new_backup = match mine {
                        Some(m) => {
                            let dest = self.backup_root(&k).join(path);
                            let _ = std::fs::create_dir_all(dest.parent().unwrap());
                            if std::fs::rename(&m, &dest).is_err() {
                                let _ = std::fs::copy(&m, &dest);
                            }
                            json!(format!("{}/{path}", k.replace(':', "-")))
                        }
                        None => Value::Null,
                    };
                    rec.get_mut(&k).unwrap()["files"][i]["backup"] = new_backup;
                }
                None => self.put_back(&game, f),
            }
        }
        self.save(&rec).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(self.backup_root(key));
        Ok(r)
    }
}

/// Writes a copy of src at target, whatever target's permissions (copy to a temporary
/// file beside it, then rename over it). The result is always writable: archives
/// often carry read-only files, and the game folder has to stay ours to change.
pub fn replace_with(src: &Path, target: &Path) -> std::io::Result<()> {
    let tmp = target.with_extension("ls-tmp");
    std::fs::copy(src, &tmp)?;
    let mut perm = std::fs::metadata(&tmp)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o644);
    }
    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    std::fs::set_permissions(&tmp, perm)?;
    #[cfg(windows)]
    if let Ok(m) = std::fs::metadata(target) {
        // Windows won't rename over a read-only file
        let mut p = m.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        let _ = std::fs::set_permissions(target, p);
    }
    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// base/rel only if rel stays inside base.
fn inside(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    (out != base).then_some(out)
}

/// Removes folders we emptied, up to the game folder.
fn prune(mut d: Option<&Path>, stop: &Path) {
    while let Some(dir) = d {
        if dir == stop || !dir.starts_with(stop) || std::fs::remove_dir(dir).is_err() {
            return;
        }
        d = dir.parent();
    }
}

/// Files the installer (installer/record.py) put in the game: never replaced by downloads.
pub fn installer_files(game: &Path) -> HashSet<String> {
    std::fs::read_to_string(game.join("addons/loner-strike/record.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["files"].as_object().map(|o| o.keys().map(|k| k.replace('\\', "/").to_lowercase()).collect()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys;

    fn rels(list: &[&str]) -> Vec<Vec<String>> {
        list.iter().map(|r| archive::listed_parts(r)).collect()
    }

    #[test]
    fn allow_lists() {
        assert!(allowed("skins", "models/v_ak47.mdl"));
        assert!(allowed("skins", "sound/weapons/ak47-1.wav"));
        assert!(!allowed("skins", "models/v_ak47.dll"));
        assert!(!allowed("skins", "dlls/mp.dll"));
        assert!(!allowed("skins", "config.cfg"));
        assert!(!allowed("skins", "sprites/hud.txt"));
        assert!(allowed("sprites", "sprites/hud.txt"));
        assert!(!allowed("sprites", "sprites/x/hud.txt"));
        assert!(allowed("sprays", "tempdecal.wad"));
        assert!(allowed("sprays", "logos/cat.bmp"));
        assert!(!allowed("sprays", "cstrike.wad"));
        assert!(!allowed("sounds", "media/other.mp3"));
        assert!(allowed("gui", "resource/background/800_1_a_loading.tga"));
        assert!(allowed("gui", "resource/BackgroundLayout.txt"));
        assert!(!allowed("gui", "resource/GameMenu.res"));
        assert!(!allowed("gui", "commandmenu.txt"));
        assert!(!allowed("maps", "models/v_ak47.mdl"));
        assert!(!allowed("nope", "models/v_ak47.mdl"));
        assert!(safe_part("AWSM Draw.wav"));
        assert!(!safe_part(".."));
        assert!(!safe_part("a\"b"));
        assert!(!safe_part("x;quit"));
    }

    #[test]
    fn placement() {
        let mut ex = Existing::default();
        ex.add("sound/weapons/ak47-1.wav");
        ex.add("sprites/muzzleflash1.spr");
        ex.add("resource/background/800_1_a_loading.tga");
        ex.add("sound/a/dup.wav");
        ex.add("sound/b/dup.wav");
        let list = rels(&[
            "install.txt", "p_awp.mdl", "screenshots/Render.jpg", "v_awp.mdl", "w_awp.mdl", "ak47-1.wav", "dup.wav",
            "unknown.wav", "Pack/cstrike/models/player/vip/vip.mdl", "VIP/vipT.mdl", "evil.dll", "cstrike/sound/weapons/AWSM Draw.wav",
            "x/../../etc/passwd.mdl",
        ]);
        let (p, skipped) = plan("skins", &list, &ex);
        let dests: Vec<&str> = p.iter().map(|(_, d)| d.as_str()).collect();
        assert_eq!(
            dests,
            vec![
                "models/p_awp.mdl", "models/v_awp.mdl", "models/w_awp.mdl", "sound/weapons/ak47-1.wav",
                "models/player/vip/vip.mdl", "models/player/vip/vipT.mdl", "sound/weapons/AWSM Draw.wav"
            ]
        );
        assert!(skipped.iter().any(|s| s.starts_with("dup.wav (the game has several")));
        assert!(skipped.contains(&"unknown.wav".to_string()));
        assert!(skipped.contains(&"evil.dll".to_string()));

        let (p, _) = plan("sprays", &rels(&["READ ME.txt", "tempdecal.wad"]), &ex);
        assert_eq!(p, vec![(1, "tempdecal.wad".to_string())]);
        let (p, _) = plan("sprites", &rels(&["muzzleflash1.spr", "big/cstrike/decals.wad", "small/cstrike/decals.wad"]), &ex);
        assert_eq!(p.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), vec!["sprites/muzzleflash1.spr", "decals.wad"]);
        let (p, _) = plan(
            "gui",
            &rels(&["BG/cstrike/resource/BackgroundLayout.txt", "BG/cstrike/resource/background/1024_1_a_loading.tga", "BG/cstrike/resource/GameMenu.res", "800_1_a_loading.tga"]),
            &ex,
        );
        assert_eq!(
            p.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(),
            vec!["resource/BackgroundLayout.txt", "resource/background/1024_1_a_loading.tga", "resource/background/800_1_a_loading.tga"]
        );
    }

    fn put(p: &Path, data: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    #[test]
    fn stacked_backups() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "content-").unwrap();
        let game = root.join("czero");
        put(&game.join("models/V_AK47.mdl"), "original");
        let recs = Records::new(&root.join("app"));
        let tag = crate::sounds::folder_tag(&game);
        let k = |n: i64| format!("Mod:{n}:{tag}");
        let src = root.join("src");
        put(&src.join("a"), "skin A");
        put(&src.join("b"), "skin B");
        put(&src.join("snd"), "sound A");

        let install = |key: &str, files: Vec<(PathBuf, String)>, seq: i64| {
            let (f, replaced) = recs.install(&game, key, &files).unwrap();
            let mut rec = recs.load();
            rec.insert(key.into(), json!({"name": key, "game": game.to_string_lossy(), "seq": seq, "files": f}));
            recs.save(&rec).unwrap();
            replaced
        };
        let read = |rel: &str| std::fs::read_to_string(game.join(rel)).ok();

        // A replaces the original (matching its case) and adds a new sound
        let r = install(&k(1), vec![(src.join("a"), "models/v_ak47.mdl".into()), (src.join("snd"), "sound/new/x.wav".into())], 1);
        assert_eq!(r, vec!["models/V_AK47.mdl".to_string()]);
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("skin A"));
        // B replaces A
        install(&k(2), vec![(src.join("b"), "models/v_ak47.mdl".into())], 2);
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("skin B"));
        assert_eq!(recs.owners(&game)["models/v_ak47.mdl"].0, k(2));

        // removing A (underneath) keeps B on disk; removing B then brings back the original
        recs.uninstall(&k(1)).unwrap();
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("skin B"));
        assert_eq!(read("sound/new/x.wav"), None);
        assert!(!game.join("sound").exists(), "emptied folders are removed");
        recs.uninstall(&k(2)).unwrap();
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("original"));
        assert!(recs.load().is_empty());

        // the other order: A, B, remove B (A comes back), remove A (original)
        install(&k(1), vec![(src.join("a"), "models/v_ak47.mdl".into())], 3);
        install(&k(2), vec![(src.join("b"), "models/v_ak47.mdl".into())], 4);
        recs.uninstall(&k(2)).unwrap();
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("skin A"));
        recs.uninstall(&k(1)).unwrap();
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("original"));
        assert!(recs.uninstall(&k(1)).is_err());

        // read-only files (as archives often have) are still put back
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            put(&src.join("ro"), "read-only skin");
            std::fs::set_permissions(src.join("ro"), std::fs::Permissions::from_mode(0o444)).unwrap();
            install(&k(4), vec![(src.join("ro"), "models/v_ak47.mdl".into())], 5);
            recs.uninstall(&k(4)).unwrap();
            assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("original"));
        }

        // a failed install leaves nothing behind
        let bad = recs.install(&game, &k(3), &[(src.join("a"), "models/v_ak47.mdl".into()), (src.join("missing"), "models/v_m4a1.mdl".into())]);
        assert!(bad.is_err());
        assert_eq!(read("models/V_AK47.mdl").as_deref(), Some("original"));
        assert!(!game.join("models/v_m4a1.mdl").exists());
        // a record saved before keys named the game folder is read under the new key
        let mut old = Map::new();
        old.insert("Mod:9".into(), json!({"game": game.to_string_lossy(), "seq": 9, "files": []}));
        recs.save(&old).unwrap();
        assert!(recs.for_game(&game).contains_key(&k(9)));
        assert_eq!(key_id(&k(9)), 9);
        let _ = std::fs::remove_dir_all(&root);
    }
}
