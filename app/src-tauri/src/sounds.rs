//! The Sounds page: round-end music, Quake sounds and a few classic sound packs, for the
//! player's own game or a dedicated server (X-Target).
//!
//! Round-end music is the roundmusic add-on (mods/roundmusic). It has no track list: at
//! every map load it scans
//!     sound/roundsound/t_win/    played when the Terrorists win
//!     sound/roundsound/ct_win/   played when the Counter-Terrorists win
//!     sound/roundsound/          played for either team
//! and uses each .mp3/.wav it finds (not sub-folders, not names with spaces, at most 16 a
//! folder). So a track is turned off by moving it into an `off` folder beside it, and every
//! change applies from the next map. Clients download the tracks when they join.
//!
//! Quake sounds are AMX Mod X's miscstats.amxx, the "quakesounds" add-on of the Game Mode
//! Menu. Add-ons are switched with gm_addon while the game runs; when it doesn't, this
//! writes the same two files the plugin would (data/addons.txt, configs/plugins-gmaddons.ini).
//! For a server on another machine those two files are brought back from it after a
//! switch, and whether an add-on is on comes from the running server itself.
//!
//! Sound packs: a short list of GameBanana items, checked by hand. The ones that replace
//! the game's own .wav files go through the usual GameBanana install (your own game; it
//! backs up what it replaces). The round-end music packs are copied in as tracks for the
//! game or server being managed, recorded the same way so removing one takes its tracks out.

use crate::archive;
use crate::game::{CmdError, Game};
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The plugin precaches at most this many tracks per folder (a random pick of them if there are more).
pub const PER_FOLDER: usize = 16;
/// Refused above this: a client downloads every track when it joins.
pub const MAX_TRACK: u64 = 8 * 1024 * 1024;
/// A good size for one track (shown as advice).
pub const GOOD_TRACK: u64 = 1024 * 1024;
/// Past this much music in total, joining takes noticeably longer for friends.
pub const WARN_TOTAL: u64 = 12 * 1024 * 1024;
/// File names the plugin can read (its buffer is 64 characters).
const MAX_NAME: usize = 60;
const OFF: &str = "off";
const USER_AGENT: &str = "LonerStrike/1.0 (local map browser)";
const DOWNLOAD_HOSTS: &[&str] = &["gamebanana.com", "files.gamebanana.com"];
const MAX_PACK_DOWNLOAD: u64 = 64 * 1024 * 1024;

/// (id, sub-folder of sound/roundsound, label)
pub const POOLS: &[(&str, &str, &str)] = &[("t", "t_win", "Terrorists win"), ("ct", "ct_win", "Counter-Terrorists win"), ("any", "", "Either team")];

fn pool_sub(pool: &str) -> Option<&'static str> {
    POOLS.iter().find(|p| p.0 == pool).map(|p| p.1)
}

/// sound/roundsound/<pool>[/off] inside a game folder.
pub fn pool_dir(game: &Path, pool: &str, off: bool) -> Option<PathBuf> {
    let sub = pool_sub(pool)?;
    let mut d = game.join("sound").join("roundsound");
    if !sub.is_empty() {
        d.push(sub);
    }
    if off {
        d.push(OFF);
    }
    Some(d)
}

fn pool_rel(pool: &str, off: bool, name: &str) -> String {
    let sub = pool_sub(pool).unwrap_or("");
    let mut parts = vec!["sound", "roundsound"];
    if !sub.is_empty() {
        parts.push(sub);
    }
    if off {
        parts.push(OFF);
    }
    parts.push(name);
    parts.join("/")
}

fn sound_kind(name: &str) -> Option<&'static str> {
    let low = name.to_lowercase();
    if low.ends_with(".mp3") {
        Some("mp3")
    } else if low.ends_with(".wav") {
        Some("wav")
    } else {
        None
    }
}

/// A track name we accept from a request: one plain file name with a sound extension.
/// (Files are then looked up by exact name in the folder, never joined blindly.)
pub fn request_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name != ".."
        && sound_kind(name).is_some()
}

/// The file called exactly `name` directly inside `dir` (a regular file, not a link).
pub fn find_in(dir: &Path, name: &str) -> Option<PathBuf> {
    if !request_name_ok(name) {
        return None;
    }
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        if e.file_name().to_string_lossy() == name {
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            return meta.file_type().is_file().then(|| e.path());
        }
    }
    None
}

/// A name the plugin can use, made from any file name: lower case, letters, digits and _.
pub fn safe_track_name(stem: &str, kind: &str) -> String {
    let mut out = String::new();
    for c in stem.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '_' };
        if c == '_' && (out.is_empty() || out.ends_with('_')) {
            continue;
        }
        out.push(c);
    }
    let mut s: String = out.trim_matches('_').chars().take(MAX_NAME - 8).collect();
    while s.ends_with('_') {
        s.pop();
    }
    if s.is_empty() {
        s = "track".into();
    }
    format!("{s}.{kind}")
}

/// `name`, or name_2, name_3... so nothing on or off in that pool is overwritten.
fn unique_name(game: &Path, pool: &str, name: &str, taken: &[String]) -> String {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "mp3"));
    let free = |n: &str| {
        !taken.iter().any(|t| t.eq_ignore_ascii_case(n))
            && [false, true].iter().all(|off| pool_dir(game, pool, *off).map(|d| !exists_ci(&d, n)).unwrap_or(true))
    };
    if free(name) {
        return name.to_string();
    }
    (2..1000).map(|i| format!("{stem}_{i}.{ext}")).find(|n| free(n)).unwrap_or_else(|| format!("{stem}_{}.{ext}", crate::sys::now() as u64))
}

fn exists_ci(dir: &Path, name: &str) -> bool {
    std::fs::read_dir(dir).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))).unwrap_or(false)
}

// ---- checking sound files ---------------------------------------------------------------

/// Whether the bytes look like the kind of file the name says, and anything the game may
/// not like about it. Err: not that kind of file at all.
pub fn check_sound(kind: &str, head: &[u8]) -> Result<Option<String>, String> {
    match kind {
        "mp3" => {
            let id3 = head.starts_with(b"ID3");
            let frame = head.len() >= 2 && head[0] == 0xFF && head[1] & 0xE0 == 0xE0;
            if id3 || frame {
                Ok(None)
            } else {
                Err("isn't an mp3 file".into())
            }
        }
        "wav" => {
            if head.len() < 12 || &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
                return Err("isn't a wav file".into());
            }
            // walk the chunks to "fmt "
            let mut i = 12;
            while i + 8 <= head.len() {
                let id = &head[i..i + 4];
                let len = u32::from_le_bytes([head[i + 4], head[i + 5], head[i + 6], head[i + 7]]) as usize;
                if id == b"fmt " && i + 8 + 16 <= head.len() {
                    let f = &head[i + 8..];
                    let format = u16::from_le_bytes([f[0], f[1]]);
                    let channels = u16::from_le_bytes([f[2], f[3]]);
                    let rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                    let bits = u16::from_le_bytes([f[14], f[15]]);
                    if format != 1 || !(bits == 8 || bits == 16) {
                        return Err("is a compressed wav; the game only plays plain 8 or 16-bit wav (or use mp3)".into());
                    }
                    if channels != 1 || !(rate == 11025 || rate == 22050) {
                        return Ok(Some(format!(
                            "is a {} wav at {rate} Hz; the game plays mono 11025 or 22050 Hz wav best, so it may not play. mp3 is safer.",
                            if channels == 1 { "mono" } else { "stereo" }
                        )));
                    }
                    return Ok(None);
                }
                i += 8 + len + (len & 1);
            }
            Err("isn't a wav file the game can read".into())
        }
        _ => Err("isn't an mp3 or wav file".into()),
    }
}

fn read_head(p: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut buf = vec![0u8; 4096];
    let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    buf.truncate(n);
    buf
}

// ---- listing ----------------------------------------------------------------------------

fn list_dir(dir: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = std::fs::symlink_metadata(e.path()).ok()?;
            (meta.file_type().is_file() && !name.starts_with('.') && sound_kind(&name).is_some()).then_some((name, meta.len()))
        })
        .collect();
    v.sort_by_key(|(n, _)| n.to_lowercase());
    v
}

/// Every track of the game's round-end music, by pool, on and off.
pub fn tracks(game: &Path, owners: &HashMap<String, (String, String)>) -> Value {
    let mut pools = Vec::new();
    let mut total_on = 0u64;
    for (id, _, label) in POOLS {
        let mut list = Vec::new();
        let mut on_count = 0;
        for off in [false, true] {
            let Some(dir) = pool_dir(game, id, off) else { continue };
            for (name, size) in list_dir(&dir) {
                if !off {
                    total_on += size;
                    on_count += 1;
                }
                let pack = owners.get(&pool_rel(id, false, &name).to_lowercase()).or_else(|| owners.get(&pool_rel(id, true, &name).to_lowercase()));
                let problem = if name.contains(' ') {
                    Some("The name has a space, so the game skips it. Remove it and add it again.")
                } else if name.len() > MAX_NAME {
                    Some("The name is too long for the game. Remove it and add it again.")
                } else {
                    None
                };
                list.push(json!({
                    "name": name, "size": size, "on": !off, "kind": sound_kind(&name),
                    "pack": pack.map(|p| p.1.clone()), "problem": problem,
                }));
            }
        }
        pools.push(json!({"id": id, "label": label, "tracks": list, "on": on_count, "over": on_count > PER_FOLDER}));
    }
    json!({"pools": pools, "total": total_on, "limit": PER_FOLDER, "good": GOOD_TRACK, "warn_total": WARN_TOTAL, "max": MAX_TRACK})
}

/// Turns a track on or off (moves it in or out of the `off` folder), or moves it to another pool.
pub fn move_track(game: &Path, pool: &str, name: &str, off: bool, to_pool: &str, to_off: bool) -> Result<(), String> {
    let from = pool_dir(game, pool, off).ok_or("Unknown list.")?;
    let src = find_in(&from, name).ok_or("That track isn't there any more.")?;
    let dest_dir = pool_dir(game, to_pool, to_off).ok_or("Unknown list.")?;
    if dest_dir == from {
        return Ok(());
    }
    if exists_ci(&dest_dir, name) {
        return Err("There's already a track with that name there.".into());
    }
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    std::fs::rename(&src, dest_dir.join(name)).map_err(|e| format!("Couldn't move it ({e})."))
}

pub fn remove_track(game: &Path, pool: &str, name: &str, off: bool) -> Result<(), String> {
    let dir = pool_dir(game, pool, off).ok_or("Unknown list.")?;
    let p = find_in(&dir, name).ok_or("That track isn't there any more.")?;
    std::fs::remove_file(&p).map_err(|e| format!("Couldn't remove it ({e})."))?;
    if off {
        let _ = std::fs::remove_dir(&dir); // only if empty
    }
    Ok(())
}

/// Copies files the player picked into a pool. Returns (added names, notes).
pub fn add_tracks(game: &Path, pool: &str, files: &[PathBuf]) -> Result<(Vec<String>, Vec<String>), String> {
    let dir = pool_dir(game, pool, false).ok_or("Unknown list.")?;
    let mut added = Vec::new();
    let mut notes = Vec::new();
    for f in files {
        let shown = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let Some(kind) = sound_kind(&shown) else {
            notes.push(format!("{shown} isn't an mp3 or wav file."));
            continue;
        };
        let meta = match std::fs::metadata(f) {
            Ok(m) if m.is_file() => m,
            _ => {
                notes.push(format!("Couldn't read {shown}."));
                continue;
            }
        };
        if meta.len() > MAX_TRACK {
            notes.push(format!("{shown} is {}; tracks over {} aren't added (everyone downloads them when they join).", mb(meta.len()), mb(MAX_TRACK)));
            continue;
        }
        match check_sound(kind, &read_head(f)) {
            Err(why) => {
                notes.push(format!("{shown} {why}."));
                continue;
            }
            Ok(Some(warn)) => notes.push(format!("{shown} {warn}")),
            Ok(None) => {}
        }
        let stem = shown.rsplit_once('.').map(|(s, _)| s).unwrap_or(&shown);
        let name = unique_name(game, pool, &safe_track_name(stem, kind), &added);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        crate::content::replace_with(f, &dir.join(&name)).map_err(|e| format!("Couldn't copy {shown} ({e})."))?;
        if meta.len() > GOOD_TRACK {
            notes.push(format!("{name} is {}. Under 1 MB a track keeps joining quick for friends.", mb(meta.len())));
        }
        added.push(name);
    }
    Ok((added, notes))
}

fn mb(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / 1048576.0)
    } else {
        format!("{} KB", (n / 1024).max(1))
    }
}

// ---- add-ons (Quake sounds, round-end music) ------------------------------------------------

/// Pawn's parse(): words, or "quoted strings".
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            chars.next();
        }
        let Some(&c) = chars.peek() else { break };
        let mut w = String::new();
        if c == '"' {
            chars.next();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                w.push(c);
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                w.push(c);
                chars.next();
            }
        }
        out.push(w);
    }
    out
}

/// configs/gamemodes/addons.ini: [(id, plugin files)] in file order.
pub fn addon_list(ini: &str) -> Vec<(String, Vec<String>)> {
    ini.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with('/'))
        .filter_map(|l| {
            let w = words(l);
            let id = w.first()?.clone();
            let plugins: Vec<String> = w.get(2)?.split_whitespace().map(String::from).collect();
            (!id.is_empty() && !plugins.is_empty()).then_some((id, plugins))
        })
        .collect()
}

struct AddonFiles {
    ini: PathBuf,
    on: PathBuf,
    plugins_ini: PathBuf,
    plugins: PathBuf,
}

fn addon_files(g: &Game) -> AddonFiles {
    let am = g.dir.join("addons").join("amxmodx");
    AddonFiles {
        ini: am.join("configs/gamemodes/addons.ini"),
        on: g.data.join("addons.txt"),
        plugins_ini: am.join("configs/plugins-gmaddons.ini"),
        plugins: am.join("plugins"),
    }
}

fn read_text(p: &Path) -> String {
    crate::sys::read_latin1(p).unwrap_or_default()
}

/// {installed, on} for an add-on of the Game Mode Menu.
fn addon_state(f: &AddonFiles, id: &str) -> Value {
    let list = addon_list(&read_text(&f.ini));
    let installed = list.iter().find(|(i, _)| i.eq_ignore_ascii_case(id)).map(|(_, p)| p.iter().all(|p| f.plugins.join(p).is_file())).unwrap_or(false);
    let on = read_text(&f.on).lines().any(|l| l.trim().eq_ignore_ascii_case(id));
    json!({"installed": installed, "on": installed && on})
}

/// What the plugin writes when an add-on is switched (write_addons in gamemodes.sma):
/// the add-ons that are on, and the plugins AMX Mod X loads for them on the next map.
pub fn write_addons_offline(ini: &str, on_now: &str, id: &str, on: bool) -> Result<(String, String), String> {
    let list = addon_list(ini);
    if !list.iter().any(|(i, _)| i.eq_ignore_ascii_case(id)) {
        return Err("That add-on isn't installed.".into());
    }
    let was: Vec<String> = on_now.lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect();
    let mut txt = String::new();
    let mut plugins = String::from("; Generated by gamemodes.amxx - toggle add-ons from the in-game menu\n");
    for (i, p) in &list {
        let enabled = if i.eq_ignore_ascii_case(id) { on } else { was.contains(&i.to_lowercase()) };
        if enabled {
            txt.push_str(&format!("{i}\n"));
            for name in p {
                plugins.push_str(&format!("{name}\n"));
            }
        }
    }
    Ok((txt, plugins))
}

/// The add-on files the plugin rewrites, relative to the game folder (pulled back from a
/// server on another machine after a switch).
const ADDON_FILES: &[&str] = &["addons/amxmodx/data/gamemodes/addons.txt", "addons/amxmodx/configs/plugins-gmaddons.ini"];

fn set_addon(g: &Game, id: &str, on: bool) -> Result<&'static str, String> {
    let remote = crate::servers::entry_for(g);
    match g.command("gm_addon", &[id.to_string(), if on { "1" } else { "0" }.to_string()]) {
        Ok(_) => {
            // a server on another machine: the plugin saved the switch there; this
            // computer's copy takes it too (recorded as sent, so the old one never goes
            // back up over it)
            // (the switch is made either way; if the files can't be fetched now, the next
            // switch with the server off fetches them before writing)
            if let Some(e) = &remote {
                let _ = crate::servers::pull_files(e, ADDON_FILES);
            }
            Ok("live")
        }
        Err(CmdError::Bad(e)) => Err(e),
        Err(CmdError::Game(_)) => {
            // not running: write what the plugin would, it reads them when the game starts.
            // On another machine, from what the machine has (add-ons switched from the Fun
            // page are only there); the change then goes up with the next sync
            if let Some(e) = &remote {
                crate::servers::pull_files(e, ADDON_FILES).map_err(|err| format!("Couldn't reach the server's machine to switch it ({err})."))?;
            }
            let f = addon_files(g);
            let (txt, plugins) = write_addons_offline(&read_text(&f.ini), &read_text(&f.on), id, on)?;
            std::fs::create_dir_all(&g.data).map_err(|e| e.to_string())?;
            crate::sys::write_latin1(&f.on, &txt).map_err(|e| e.to_string())?;
            crate::sys::write_latin1(&f.plugins_ini, &plugins).map_err(|e| e.to_string())?;
            Ok("saved")
        }
    }
}

/// For a server on another machine that's running: whether each add-on is on, as the
/// plugin says (this computer's addons.txt can be behind the machine's).
fn live_addons(g: &Game) -> Option<HashMap<String, bool>> {
    g.remote.as_ref()?;
    let st = g.state();
    if st["online"] != json!(true) {
        return None;
    }
    Some(st["addons"].as_array()?.iter().filter_map(|a| Some((a["id"].as_str()?.to_lowercase(), a["on"].as_i64().unwrap_or(0) != 0))).collect())
}

/// A dedicated server's sounds changed: the next map takes them in (the server list says
/// so until then).
fn sounds_changed(g: &Game) {
    if let Some(id) = crate::servers::id_of(g) {
        crate::servers::set_pending_restart(&id, crate::servers::Pending::Sounds);
    }
}

// ---- classic sound packs ------------------------------------------------------------------

pub struct Pack {
    /// GameBanana Sound id
    pub id: i64,
    /// the download (file id) that was checked
    pub file: i64,
    pub name: &'static str,
    pub author: &'static str,
    /// "music": mp3s copied in as round-end tracks; "replace": the usual sounds install
    pub kind: &'static str,
    /// for music packs: the start of each track's file name
    pub slug: &'static str,
    /// for music packs: the download's kind of archive
    pub ext: &'static str,
    pub about: &'static str,
}

/// Checked on GameBanana (Counter-Strike 1.6 sounds, 2026-11): each item exists, is a Sound
/// with one clean download, and its archive holds the files described here.
pub const PACKS: &[Pack] = &[
    Pack {
        id: 14773, file: 189134, name: "Rammstein CT and T win pack", author: "K3PT3L3N", kind: "music", slug: "rammstein", ext: ".zip",
        about: "Six Rammstein clips of about 20 seconds (2010): three for each team's win. About 0.5 MB each.",
    },
    Pack {
        id: 82836, file: 1549188, name: "End Round Music Cues", author: "OffTheVoid", kind: "music", slug: "cues", ext: ".rar",
        about: "Six 8-second music stings, three for each team. About 0.3 MB each.",
    },
    Pack {
        id: 57946, file: 707945, name: "End Round sound", author: "opera", kind: "music", slug: "endround", ext: ".zip",
        about: "Five songs not tied to a team, so they play for either side. About 0.5 MB each.",
    },
    Pack {
        id: 90672, file: 1738096, name: "Kill sounds pack (UT recreation)", author: "Mike Isakov", kind: "replace", slug: "", ext: "",
        about: "Unreal Tournament style announcer. Replaces the Quake sounds the game already has with the same names: first blood, double kill, killing spree, rampage, monster kill, godlike and headshot.",
    },
    Pack {
        id: 36543, file: 167738, name: "CS:GO CT and T End Round Sounds", author: "MatasM82", kind: "replace", slug: "", ext: "",
        about: "Replaces the radio's \"Counter-Terrorists win\" and \"Terrorists win\" lines with CS:GO's round-end music.",
    },
    Pack {
        id: 21823, file: 182451, name: "CT Win Radio Sound (SWAT Team Wins)", author: "Ghost968373", kind: "replace", slug: "", ext: "",
        about: "Replaces the radio's \"Counter-Terrorists win\" line with \"SWAT team wins\".",
    },
    Pack {
        id: 25095, file: 179180, name: "SWAT Radio Voices", author: "Xander9393", kind: "replace", slug: "", ext: "",
        about: "A new voice for the radio commands (like Go go go, Enemy spotted and Fire in the hole).",
    },
    Pack {
        id: 31143, file: 173142, name: "CS:GO Radio & Bot Chatter", author: "hellmonja", kind: "replace", slug: "", ext: "",
        about: "CS:GO's radio commands and bot chatter. A large download (about 18 MB).",
    },
];

fn pack(id: i64) -> Option<&'static Pack> {
    PACKS.iter().find(|p| p.id == id)
}

/// Short and stable per game folder, so a music pack can be on the game and a server at once.
pub fn folder_tag(game: &Path) -> String {
    let mut h: u32 = 0x811c9dc5;
    for b in game.to_string_lossy().bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("{h:08x}")
}

pub fn music_key(id: i64, game: &Path) -> String {
    format!("Roundsound:{id}:{}", folder_tag(game))
}

/// Which pool a track from a pack belongs in, from its name and folder ("Ctwin4.mp3", "t_win/x.mp3").
pub fn pool_for(parts: &[String]) -> &'static str {
    let norm = |s: &str| s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    let file = parts.last().map(|f| norm(f.rsplit_once('.').map(|(s, _)| s).unwrap_or(f))).unwrap_or_default();
    let dir = parts.len().checked_sub(2).map(|i| norm(&parts[i])).unwrap_or_default();
    let digit_after = |s: &str, p: &str| s.strip_prefix(p).map(|r| r.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true)).unwrap_or(false);
    let ct = |s: &str| s.starts_with("ctwin") || s.starts_with("counterterroristwin") || digit_after(s, "ct");
    let t = |s: &str| s.starts_with("twin") || s.starts_with("terwin") || s.starts_with("terroristwin") || s.starts_with("ttwin") || digit_after(s, "t") || digit_after(s, "tt");
    if ct(&file) || (!t(&file) && ct(&dir)) {
        "ct"
    } else if t(&file) || t(&dir) {
        "t"
    } else if file.contains("ctwin") {
        "ct"
    } else if file.contains("terwin") || file.contains("twin") {
        "t"
    } else {
        "any"
    }
}

/// [(source, destination relative to the game)] for the mp3/wav files of an unpacked music pack.
pub fn plan_music(game: &Path, slug: &str, root: &Path) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    let mut taken: HashMap<&str, Vec<String>> = HashMap::new();
    for f in archive::walk(root) {
        let parts = archive::rel_parts(root, &f);
        let Some(file) = parts.last() else { continue };
        let Some(kind) = sound_kind(file) else { continue };
        let size = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
        if size > MAX_TRACK {
            skipped.push(format!("{file} ({}, too big)", mb(size)));
            continue;
        }
        if check_sound(kind, &read_head(&f)).is_err() {
            skipped.push(format!("{file} (not a sound the game plays)"));
            continue;
        }
        let pool = pool_for(&parts);
        let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
        let name = safe_track_name(&format!("{slug}_{stem}"), kind);
        let list = taken.entry(pool).or_default();
        let name = unique_name(game, pool, &name, list);
        list.push(name.clone());
        out.push((f, pool_rel(pool, false, &name)));
    }
    (out, skipped)
}

#[derive(Clone, serde::Serialize)]
struct MusicJob {
    id: String,
    #[serde(rename = "mod")]
    mod_id: i64,
    state: String,
    message: String,
    got: u64,
    size: u64,
    #[serde(skip)]
    ended: Option<f64>,
}

static MUSIC_JOB: Mutex<Option<MusicJob>> = Mutex::new(None);
/// One change to the content record at a time from here.

fn job_update(f: impl FnOnce(&mut MusicJob)) {
    if let Some(j) = MUSIC_JOB.lock().unwrap().as_mut() {
        f(j);
    }
}

fn download(url: &str, out: &Path) -> Result<(), String> {
    use std::io::{Read, Write};
    let net = || "Couldn't reach GameBanana. Check your internet connection.".to_string();
    let mut r = crate::sys::http().get(url).header("User-Agent", USER_AGENT).timeout(Duration::from_secs(600)).send().map_err(|_| net())?;
    let host = r.url().host_str().unwrap_or("").to_lowercase();
    if !DOWNLOAD_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}"))) {
        return Err(format!("The download was redirected somewhere unexpected ({host})."));
    }
    if !r.status().is_success() {
        return Err(net());
    }
    let size = r.content_length().unwrap_or(0);
    job_update(|j| j.size = size);
    let mut f = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 128 * 1024];
    let mut got = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|_| net())?;
        if n == 0 {
            break;
        }
        got += n as u64;
        if got > MAX_PACK_DOWNLOAD {
            return Err("That download is bigger than expected, so it was stopped.".into());
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        job_update(|j| j.got = got);
    }
    Ok(())
}

fn install_music(g: &Game, p: &Pack) -> Result<String, String> {
    let key = music_key(p.id, &g.dir);
    if g.store.content.for_game(&g.dir).contains_key(&key) {
        return Err("That pack is already in.".into());
    }
    let work = g.app.join("cache").join("downloads");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let tmp = crate::sys::make_temp_dir(&work, "snd-").map_err(|e| e.to_string())?;
    let res = (|| {
        let dl = tmp.join(format!("download{}", p.ext));
        download(&format!("https://gamebanana.com/dl/{}", p.file), &dl)?;
        job_update(|j| {
            j.state = "unpacking".into();
            j.message = "Unpacking".into();
        });
        let x = tmp.join("x");
        std::fs::create_dir_all(&x).map_err(|e| e.to_string())?;
        archive::extract(&dl, &x, 256 * 1024 * 1024)?;
        let (plan, _) = plan_music(&g.dir, p.slug, &x);
        if plan.is_empty() {
            return Err("There were no tracks the game can play in that download.".into());
        }
        job_update(|j| j.message = "Copying tracks".into());
        let _g = crate::gamebanana::RECORDS.lock().unwrap_or_else(|e| e.into_inner());
        let (entries, _) = g.store.content.install(&g.dir, &key, &plan)?;
        let mut rec = g.store.content.load();
        let seq = rec.values().filter_map(|r| r["seq"].as_i64()).max().unwrap_or(0) + 1;
        let count = entries.len();
        rec.insert(
            key.clone(),
            json!({
                "section": "roundsound", "name": p.name, "author": p.author, "url": format!("https://gamebanana.com/sounds/{}", p.id),
                "file": p.file, "date": crate::sys::now() as i64, "seq": seq, "game": g.dir.to_string_lossy(), "files": entries,
            }),
        );
        if let Err(e) = g.store.content.save(&rec) {
            let _ = g.store.content.uninstall_entry(&g.dir, &rec[&key]);
            return Err(e.to_string());
        }
        Ok(format!("Added {count} tracks from {}. They play from the next map.", p.name))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

fn remove_music(g: &Game, p: &Pack) -> Result<(), String> {
    let key = music_key(p.id, &g.dir);
    if !g.store.content.for_game(&g.dir).contains_key(&key) {
        return Err("That pack isn't in.".into());
    }
    let _g = crate::gamebanana::RECORDS.lock().unwrap_or_else(|e| e.into_inner());
    let r = g.store.content.uninstall(&key)?;
    // tracks that were turned off sit in the off folder beside where they were put
    for f in r["files"].as_array().into_iter().flatten() {
        let Some(path) = f["path"].as_str() else { continue };
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 3 || parts[0] != "sound" || parts[1] != "roundsound" {
            continue;
        }
        let name = parts[parts.len() - 1];
        for (id, sub, _) in POOLS {
            let here = if sub.is_empty() { parts.len() == 3 } else { parts.len() == 4 && parts[2] == *sub };
            if here {
                if let Some(off) = pool_dir(&g.dir, id, true) {
                    if let Some(f) = find_in(&off, name) {
                        let _ = std::fs::remove_file(f);
                        let _ = std::fs::remove_dir(&off);
                    }
                }
            }
        }
    }
    Ok(())
}

// ---- requests -----------------------------------------------------------------------------

type Reply = (StatusCode, Json<Value>);

fn ok(v: Value) -> Reply {
    (StatusCode::OK, Json(v))
}

fn err(e: impl Into<String>) -> Reply {
    (StatusCode::OK, Json(json!({"error": e.into()})))
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

fn summary(g: &Game) -> Value {
    let components = crate::setup::record::Record::open(&g.dir).component_ids();
    let f = addon_files(g);
    let mut music_addon = addon_state(&f, "roundmusic");
    let mut quake = addon_state(&f, "quakesounds");
    if let Some(live) = live_addons(g) {
        for (v, id) in [(&mut music_addon, "roundmusic"), (&mut quake, "quakesounds")] {
            if let Some(on) = live.get(id) {
                v["on"] = json!(v["installed"] == json!(true) && *on);
            }
        }
    }
    let owners = g.store.content.owners(&g.dir);
    let mut music = tracks(&g.dir, &owners);
    let installed = components.iter().any(|c| c == "addon-roundmusic") || music_addon["installed"] == json!(true);
    music["installed"] = json!(installed);
    music["on"] = music_addon["on"].clone();
    // Condition Zero also reads Counter-Strike 1.6's folder, so its tracks play too
    if g.name == "czero" {
        let cs = g.hl.join("cstrike");
        music["fallback"] = json!(POOLS.iter().filter_map(|(id, _, _)| pool_dir(&cs, id, false)).map(|d| list_dir(&d).len()).sum::<usize>());
    }
    json!({
        "game": g.name, "hl": g.hl, "dedicated": g.dedicated,
        "setup": components.iter().any(|c| c == "core"),
        "music": music, "quake": quake, "announcer": crate::setup::core::announcer_of(&g.dir),
    })
}

async fn api_get(State(st): State<Shared>, helper: Option<axum::Extension<crate::control::Helper>>) -> Reply {
    let Some(g) = st.game() else { return err("No game found.") };
    let mut v = tokio::task::spawn_blocking(move || summary(&g)).await.unwrap_or_else(|_| json!({"error": "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening."}));
    // someone helping run it (control/) doesn't get this computer's folders
    if helper.is_some() {
        if let Some(o) = v.as_object_mut() {
            o.remove("hl");
        }
    }
    ok(v)
}

#[derive(Deserialize)]
struct TrackReq {
    action: String,
    pool: String,
    name: String,
    #[serde(default)]
    off: bool,
    #[serde(default)]
    to: Option<String>,
}

async fn api_track(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<TrackReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    if pool_sub(&req.pool).is_none() || !request_name_ok(&req.name) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown track."})));
    }
    let Some(g) = st.game() else { return err("No game found.") };
    let r = tokio::task::spawn_blocking(move || {
        let r = track_action(&g, &req);
        if r.is_ok() {
            sounds_changed(&g);
        }
        r
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(()) => ok(json!({"ok": true})),
        Err(e) => err(e),
    }
}

fn track_action(g: &Game, req: &TrackReq) -> Result<(), String> {
    match req.action.as_str() {
        "on" => move_track(&g.dir, &req.pool, &req.name, req.off, &req.pool, false),
        "off" => move_track(&g.dir, &req.pool, &req.name, req.off, &req.pool, true),
        "move" => match req.to.as_deref().filter(|t| pool_sub(t).is_some()) {
            Some(to) => move_track(&g.dir, &req.pool, &req.name, req.off, to, req.off),
            None => Err("Unknown list.".into()),
        },
        "remove" => remove_track(&g.dir, &req.pool, &req.name, req.off),
        _ => Err("Unknown action.".into()),
    }
}

/// Plays a track in the page: only files listed in the game's sound/roundsound folders.
async fn api_file(State(st): State<Shared>, headers: HeaderMap, Query(q): Query<HashMap<String, String>>) -> Response {
    let pool = q.get("pool").cloned().unwrap_or_default();
    let name = q.get("name").cloned().unwrap_or_default();
    let off = q.get("off").map(|v| v == "1").unwrap_or(false);
    let not_found = || (StatusCode::NOT_FOUND, "not found").into_response();
    let Some(g) = st.game() else { return not_found() };
    let Some(path) = pool_dir(&g.dir, &pool, off).and_then(|d| find_in(&d, &name)) else { return not_found() };
    let Some(kind) = sound_kind(&name) else { return not_found() };
    let data = match tokio::task::spawn_blocking(move || std::fs::read(path)).await {
        Ok(Ok(d)) if d.len() as u64 <= 4 * MAX_TRACK => d,
        _ => return not_found(),
    };
    let ctype = if kind == "mp3" { "audio/mpeg" } else { "audio/wav" };
    let len = data.len();
    // WebKit plays audio only from servers that answer byte ranges
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| parse_range(r, len));
    let base = Response::builder()
        .header(header::CONTENT_TYPE, ctype)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    match range {
        Some((a, b)) => base
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {a}-{b}/{len}"))
            .header(header::CONTENT_LENGTH, b - a + 1)
            .body(Body::from(data[a..=b].to_vec()))
            .unwrap_or_else(|_| not_found()),
        None => base.header(header::CONTENT_LENGTH, len).body(Body::from(data)).unwrap_or_else(|_| not_found()),
    }
}

/// "bytes=a-b", "bytes=a-" or "bytes=-n" within len: the inclusive range.
pub fn parse_range(r: &str, len: usize) -> Option<(usize, usize)> {
    let spec = r.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = match (a.trim(), b.trim()) {
        ("", n) => {
            let n: usize = n.parse().ok()?;
            (len.saturating_sub(n), len - 1)
        }
        (a, "") => (a.parse().ok()?, len - 1),
        (a, b) => (a.parse().ok()?, b.parse::<usize>().ok()?.min(len - 1)),
    };
    (a <= b && a < len).then_some((a, b))
}

#[derive(Deserialize)]
struct AddonReq {
    id: String,
    on: bool,
}

async fn api_addon(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<AddonReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    if !["quakesounds", "roundmusic"].contains(&req.id.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown add-on."})));
    }
    let Some(g) = st.game() else { return err("No game found.") };
    match tokio::task::spawn_blocking(move || set_addon(&g, &req.id, req.on)).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into())) {
        Ok(how) => {
            if let Some(g) = st.game() {
                let _ = tokio::task::spawn_blocking(move || sounds_changed(&g)).await;
            }
            ok(json!({"ok": true, "how": how}))
        }
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
struct AnnouncerReq {
    which: String,
}

/// The Quake sounds announcer for this game or server: its miscstats.ini (both sets of
/// sounds come with setup), from the next map.
async fn api_announcer(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<AnnouncerReq>) -> Reply {
    if !from_panel(&headers) {
        return forbidden();
    }
    let Some(g) = st.game() else { return err("No game found.") };
    let res = match crate::setup_api::resources_for(&st) {
        Ok(r) => r,
        Err(e) => return err(e),
    };
    let r = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let ini = crate::setup::core::announcer_ini(&res, &req.which)?;
        let data = g.dir.join("addons/amxmodx/data/gamemodes");
        std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
        crate::sys::write_atomic(&data.join("announcer.txt"), req.which.as_bytes()).map_err(|e| format!("Couldn't save it ({e})."))?;
        let cfg = g.dir.join("addons/amxmodx/configs/miscstats.ini");
        if cfg.is_file() {
            crate::sys::write_latin1(&cfg, &ini).map_err(|e| format!("Couldn't write miscstats.ini ({e})."))?;
        }
        crate::servers::mark_changed_dir(&g.dir);
        sounds_changed(&g);
        Ok(())
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into()));
    match r {
        Ok(()) => ok(json!({"ok": true})),
        Err(e) => err(e),
    }
}

async fn api_packs(State(st): State<Shared>) -> Reply {
    let (Some(g), local) = (st.game(), st.local_game()) else { return err("No game found.") };
    let v = tokio::task::spawn_blocking(move || {
        let mine = g.store.content.for_game(&g.dir);
        let theirs = local.as_ref().map(|l| l.store.content.for_game(&l.dir)).unwrap_or_default();
        let quake_here = local.as_ref().map(|l| l.dir.join("sound/misc/headshot.wav").is_file()).unwrap_or(false);
        let packs: Vec<Value> = PACKS
            .iter()
            .map(|p| {
                let installed = if p.kind == "music" { mine.contains_key(&music_key(p.id, &g.dir)) } else { local.as_ref().is_some_and(|l| theirs.contains_key(&crate::content::record_key(crate::content::section("sounds").unwrap(), p.id, &l.dir))) };
                let mut v = json!({
                    "id": p.id, "name": p.name, "author": p.author, "kind": p.kind, "about": p.about, "installed": installed,
                    "url": format!("https://gamebanana.com/sounds/{}", p.id),
                });
                if p.id == 90672 && !quake_here {
                    v["note"] = json!("Your game doesn't have the Quake sound files this replaces, so it may find nothing to install.");
                }
                v
            })
            .collect();
        let mut jobs: Vec<Value> = local
            .map(|l| l.store.job_list())
            .unwrap_or_default()
            .into_iter()
            .filter(|j| j.section == "sounds" && pack(j.mod_id).is_some())
            .map(|j| json!({"id": j.id, "mod": j.mod_id, "state": j.state, "message": j.message, "got": j.got, "size": j.size}))
            .collect();
        if let Some(j) = MUSIC_JOB.lock().unwrap().clone() {
            if j.ended.map(|e| crate::sys::now() - e < 600.0).unwrap_or(true) {
                jobs.push(serde_json::to_value(j).unwrap_or_default());
            }
        }
        json!({"packs": packs, "jobs": jobs})
    })
    .await
    .unwrap_or_else(|_| json!({"error": "Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening."}));
    ok(v)
}

#[derive(Deserialize)]
struct PackReq {
    id: i64,
    action: String,
}

async fn api_pack(State(st): State<Shared>, headers: HeaderMap, on: Option<axum::Extension<crate::server::OnComputer>>, phone: Option<axum::Extension<crate::server::OnPhone>>, Json(req): Json<PackReq>) -> Reply {
    // installing packs is for the owner (the computer or their phone); removing one only at
    // the computer
    if !from_panel(&headers) || !crate::server::owner(&on, &phone) || (on.is_none() && req.action != "install") {
        return forbidden();
    }
    let Some(p) = pack(req.id) else { return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown pack."}))) };
    if p.kind == "replace" {
        // these change what you hear, so they always go into your own game (like Get more)
        let Some(l) = st.local_game() else { return err("No game found.") };
        let r = tokio::task::spawn_blocking(move || match req.action.as_str() {
            "install" => l.store.start_section_install("sounds", p.id, p.file).map(|job| json!({"ok": true, "job": job})).map_err(|e| e.message()),
            "remove" => l.store.uninstall_section("sounds", p.id).map(|_| json!({"ok": true})).map_err(|e| e.message()),
            _ => Err("Unknown action.".into()),
        })
        .await
        .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
        return match r {
            Ok(v) => ok(v),
            Err(e) => err(e),
        };
    }
    let Some(g) = st.game() else { return err("No game found.") };
    match req.action.as_str() {
        "install" => {
            {
                let mut j = MUSIC_JOB.lock().unwrap();
                if j.as_ref().map(|j| j.ended.is_none()).unwrap_or(false) {
                    return err("Another pack is still being added. Try again in a moment.");
                }
                *j = Some(MusicJob {
                    id: format!("roundsound-{}-{}", p.id, crate::sys::now() as i64),
                    mod_id: p.id, state: "downloading".into(), message: "Downloading".into(), got: 0, size: 0, ended: None,
                });
            }
            let id = MUSIC_JOB.lock().unwrap().as_ref().map(|j| j.id.clone()).unwrap_or_default();
            std::thread::spawn(move || {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_music(&g, p))).unwrap_or_else(|_| Err("Something went wrong adding the pack.".into()));
                job_update(|j| {
                    j.ended = Some(crate::sys::now());
                    match r {
                        Ok(m) => {
                            sounds_changed(&g);
                            j.state = "done".into();
                            j.message = m;
                        }
                        Err(e) => {
                            j.state = "error".into();
                            j.message = e;
                        }
                    }
                });
            });
            ok(json!({"ok": true, "job": id}))
        }
        "remove" => match tokio::task::spawn_blocking(move || remove_music(&g, p).inspect(|_| sounds_changed(&g))).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into())) {
            Ok(()) => ok(json!({"ok": true})),
            Err(e) => err(e),
        },
        _ => err("Unknown action."),
    }
}

/// The game a computer-only request is about. These routes sit outside the router's
/// X-Target layer, so the header is read here with the same rules.
async fn target_game(st: &Shared, headers: &HeaderMap) -> Result<Option<Arc<Game>>, Reply> {
    let id = headers.get("x-target").and_then(|v| v.to_str().ok()).map(String::from).filter(|t| !t.is_empty() && t != "local");
    let Some(id) = id else { return Ok(st.game()) };
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || id.len() > 48 {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown server."}))));
    }
    Ok(tokio::task::spawn_blocking(move || crate::servers::game_for(&id, || None)).await.ok().flatten())
}

#[derive(Deserialize)]
struct AddReq {
    pool: String,
}

/// The app window only: the system's file picker, then copies the picked tracks in.
async fn api_add(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<AddReq>) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return forbidden();
    }
    if pool_sub(&req.pool).is_none() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown list."})));
    }
    let g = match target_game(&st, &headers).await {
        Ok(Some(g)) => g,
        Ok(None) => return err("No game found."),
        Err(r) => return r,
    };
    let Some(app) = st.app.get().cloned() else { return err("No window.") };
    let r = tokio::task::spawn_blocking(move || {
        let Some(picked) = app.dialog().file().set_title("Add round-end music").add_filter("Music (mp3, wav)", &["mp3", "wav", "MP3", "WAV"]).blocking_pick_files() else {
            return Ok(json!({"cancelled": true}));
        };
        let files: Vec<PathBuf> = picked.into_iter().filter_map(|p| p.into_path().ok()).take(40).collect();
        let (added, notes) = add_tracks(&g.dir, &req.pool, &files)?;
        if !added.is_empty() {
            sounds_changed(&g);
        }
        Ok::<Value, String>(json!({"ok": true, "added": added, "notes": notes}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into()));
    match r {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/sounds", get(api_get))
        .route("/api/sounds/track", post(api_track))
        .route("/api/sounds/file", get(api_file))
        .route("/api/sounds/addon", post(api_addon))
        .route("/api/sounds/announcer", post(api_announcer))
        .route("/api/sounds/packs", get(api_packs))
        .route("/api/sounds/pack", post(api_pack))
}

/// The app window only (the file picker opens on this computer).
pub fn local_routes() -> Router<Shared> {
    Router::new().route("/api/sounds/add", post(api_add))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), &format!("sounds-{tag}-")).unwrap()
    }

    fn put(p: &Path, data: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    fn wav(channels: u16, rate: u32, format: u16, bits: u16) -> Vec<u8> {
        let mut v = b"RIFF\0\0\0\0WAVE".to_vec();
        v.extend(b"LIST");
        v.extend(4u32.to_le_bytes());
        v.extend(b"INFO");
        v.extend(b"fmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(format.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        v.extend((rate * channels as u32 * bits as u32 / 8).to_le_bytes());
        v.extend((channels * bits / 8).to_le_bytes());
        v.extend(bits.to_le_bytes());
        v.extend(b"data\0\0\0\0");
        v
    }

    #[test]
    fn names_and_paths_stay_inside() {
        assert!(request_name_ok("rock_theme.mp3"));
        assert!(request_name_ok("Song (live).WAV"));
        for bad in ["../x.mp3", "a/b.mp3", "a\\b.mp3", ".hidden.mp3", "x.exe", "", "c:x.mp3", "x.mp3\0"] {
            assert!(!request_name_ok(bad), "{bad}");
        }
        assert_eq!(safe_track_name("My Song - Live!! (2004)", "mp3"), "my_song_live_2004.mp3");
        assert_eq!(safe_track_name("../../etc/passwd", "wav"), "etc_passwd.wav");
        assert_eq!(safe_track_name("Ünïcode ☃", "mp3"), "n_code.mp3");
        assert_eq!(safe_track_name("???", "mp3"), "track.mp3");
        assert!(safe_track_name(&"x".repeat(200), "mp3").len() <= MAX_NAME);

        let g = tmp("paths");
        assert_eq!(pool_dir(&g, "t", false).unwrap(), g.join("sound/roundsound/t_win"));
        assert_eq!(pool_dir(&g, "any", true).unwrap(), g.join("sound/roundsound/off"));
        assert!(pool_dir(&g, "../x", false).is_none());
        assert_eq!(pool_rel("ct", true, "a.mp3"), "sound/roundsound/ct_win/off/a.mp3");

        put(&g.join("sound/roundsound/t_win/a.mp3"), b"ID3");
        put(&g.join("sound/roundsound/secret.mp3"), b"ID3");
        put(&g.join("sound/roundsound/t_win/notes.txt"), b"x");
        let t = pool_dir(&g, "t", false).unwrap();
        assert!(find_in(&t, "a.mp3").is_some());
        assert!(find_in(&t, "A.mp3").is_none(), "exact names only");
        assert!(find_in(&t, "../secret.mp3").is_none());
        assert!(find_in(&t, "notes.txt").is_none());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(g.join("sound/roundsound/secret.mp3"), t.join("link.mp3")).unwrap();
            assert!(find_in(&t, "link.mp3").is_none(), "links aren't served");
        }
        let _ = std::fs::remove_dir_all(&g);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-", 10), Some((0, 9)));
        assert_eq!(parse_range("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(parse_range("bytes=2-50", 10), Some((2, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=10-", 10), None);
        assert_eq!(parse_range("bytes=5-2", 10), None);
        assert_eq!(parse_range("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_range("items=0-1", 10), None);
    }

    #[test]
    fn sound_checks() {
        assert_eq!(check_sound("mp3", b"ID3\x04"), Ok(None));
        assert_eq!(check_sound("mp3", &[0xFF, 0xFB, 0x90]), Ok(None));
        assert!(check_sound("mp3", b"<html>").is_err());
        assert_eq!(check_sound("wav", &wav(1, 22050, 1, 16)), Ok(None));
        assert!(check_sound("wav", &wav(2, 44100, 1, 16)).unwrap().unwrap().contains("stereo"));
        assert!(check_sound("wav", &wav(1, 22050, 2, 4)).is_err(), "ADPCM");
        assert!(check_sound("wav", b"RIFF").is_err());
    }

    #[test]
    fn tracks_on_off_move_add_remove() {
        let root = tmp("tracks");
        let game = root.join("czero");
        put(&game.join("sound/roundsound/t_win/a.mp3"), b"ID3aaaa");
        put(&game.join("sound/roundsound/ct_win/b.mp3"), b"ID3bb");
        put(&game.join("sound/roundsound/with space.mp3"), b"ID3");
        let owners = HashMap::from([("sound/roundsound/ct_win/b.mp3".to_string(), ("Roundsound:1:x".to_string(), "Pack".to_string()))]);
        let v = tracks(&game, &owners);
        assert_eq!(v["pools"][0]["tracks"][0]["name"], "a.mp3");
        assert_eq!(v["pools"][1]["tracks"][0]["pack"], "Pack");
        assert!(v["pools"][2]["tracks"][0]["problem"].is_string());
        assert_eq!(v["total"], 7 + 5 + 3);

        move_track(&game, "t", "a.mp3", false, "t", true).unwrap();
        assert!(game.join("sound/roundsound/t_win/off/a.mp3").is_file());
        let v = tracks(&game, &HashMap::new());
        assert_eq!(v["pools"][0]["tracks"][0]["on"], false);
        assert_eq!(v["pools"][0]["on"], 0);
        assert_eq!(v["total"], 5 + 3, "off tracks aren't downloaded");
        move_track(&game, "t", "a.mp3", true, "ct", true).unwrap();
        assert!(game.join("sound/roundsound/ct_win/off/a.mp3").is_file());
        move_track(&game, "ct", "a.mp3", true, "ct", false).unwrap();
        assert!(move_track(&game, "ct", "a.mp3", false, "ct", false).is_ok(), "already there");
        put(&game.join("sound/roundsound/t_win/a.mp3"), b"ID3");
        assert!(move_track(&game, "ct", "a.mp3", false, "t", false).is_err(), "never overwrites");
        assert!(move_track(&game, "ct", "../../../x.mp3", false, "t", false).is_err());

        // adding: safe names, no overwriting, size and kind checks
        let src = root.join("picked");
        put(&src.join("My Song.mp3"), b"ID3song");
        put(&src.join("a.mp3"), b"ID3again");
        put(&src.join("fake.mp3"), b"PK\x03\x04");
        put(&src.join("stereo.wav"), &wav(2, 44100, 1, 16));
        let (added, notes) = add_tracks(&game, "t", &[src.join("My Song.mp3"), src.join("a.mp3"), src.join("fake.mp3"), src.join("stereo.wav")]).unwrap();
        assert_eq!(added, vec!["my_song.mp3", "a_2.mp3", "stereo.wav"]);
        assert!(notes.iter().any(|n| n.starts_with("fake.mp3 isn't an mp3")));
        assert!(notes.iter().any(|n| n.contains("stereo")));
        assert_eq!(std::fs::read(game.join("sound/roundsound/t_win/a.mp3")).unwrap(), b"ID3", "the old one is kept");

        remove_track(&game, "t", "a_2.mp3", false).unwrap();
        assert!(!game.join("sound/roundsound/t_win/a_2.mp3").exists());
        assert!(remove_track(&game, "t", "a_2.mp3", false).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pack_tracks_by_team() {
        let p = |s: &str| archive::listed_parts(s);
        assert_eq!(pool_for(&p("Ctwin4.mp3")), "ct");
        assert_eq!(pool_for(&p("Twin5.mp3")), "t");
        assert_eq!(pool_for(&p("cstrike/sound/misc/ctwinnar1.mp3")), "ct");
        assert_eq!(pool_for(&p("cstrike/sound/misc/twinnar2.mp3")), "t");
        assert_eq!(pool_for(&p("round_sounds/bataljon.mp3")), "any");
        assert_eq!(pool_for(&p("round_sounds/track1.mp3")), "any", "a t at the start isn't enough");
        assert_eq!(pool_for(&p("pack/ct_win/song.mp3")), "ct");
        assert_eq!(pool_for(&p("pack/t_win/song.mp3")), "t");
        assert_eq!(pool_for(&p("pack/T/song.mp3")), "t");
        assert_eq!(pool_for(&p("terwin.wav")), "t");
        assert_eq!(pool_for(&p("ct1.mp3")), "ct");

        let root = tmp("pack");
        let game = root.join("czero");
        put(&game.join("sound/roundsound/ct_win/rammstein_ctwin4.mp3"), b"ID3old");
        let x = root.join("x");
        put(&x.join("Ctwin4.mp3"), b"ID3a");
        put(&x.join("Twin4.mp3"), b"ID3b");
        put(&x.join("readme.txt"), b"hi");
        put(&x.join("broken.mp3"), b"nope");
        let (plan, skipped) = plan_music(&game, "rammstein", &x);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        assert_eq!(dests, vec!["sound/roundsound/ct_win/rammstein_ctwin4_2.mp3", "sound/roundsound/t_win/rammstein_twin4.mp3"]);
        assert_eq!(skipped.len(), 1);
        assert_ne!(music_key(1, Path::new("/a/czero")), music_key(1, Path::new("/b/czero")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn addons_written_like_the_plugin() {
        let ini = "; comment\n\nhook         \"Grappling hook\"  \"csb_grappling_hook.amxx\"  \"Bind a key\"  \"\"\nquakesounds  \"Quake sounds\"    \"miscstats.amxx\"  \"First blood, multi kill...\"  \"amx_statscfgmenu\"\ndice \"Roll the Dice\" \"dice_main.amxx dice_items_global.amxx\" \"Type /rtd\" \"\"\nroundmusic   \"Round-end music\" \"roundmusic.amxx\"  \"Win music\"  \"say /music\"\n";
        let list = addon_list(ini);
        assert_eq!(list.len(), 4);
        assert_eq!(list[2], ("dice".to_string(), vec!["dice_main.amxx".to_string(), "dice_items_global.amxx".to_string()]));

        let (txt, plugins) = write_addons_offline(ini, "dice\r\n", "quakesounds", true).unwrap();
        assert_eq!(txt, "quakesounds\ndice\n");
        assert_eq!(plugins, "; Generated by gamemodes.amxx - toggle add-ons from the in-game menu\nmiscstats.amxx\ndice_main.amxx\ndice_items_global.amxx\n");
        let (txt, plugins) = write_addons_offline(ini, &txt, "dice", false).unwrap();
        assert_eq!(txt, "quakesounds\n");
        assert!(plugins.ends_with("miscstats.amxx\n"));
        let (txt, _) = write_addons_offline(ini, "gone\nquakesounds\n", "roundmusic", true).unwrap();
        assert_eq!(txt, "quakesounds\nroundmusic\n", "add-ons no longer listed are dropped, like the plugin does");
        assert!(write_addons_offline("hook \"Hook\" \"h.amxx\"\n", "", "quakesounds", true).is_err());
    }
}
