//! The demo manager (Tools > Look, Beta): the player's demos (`record` in the game, and the
//! HLTV recordings of the servers on this computer), what's in them, rename, delete, play.
//!
//! - `GET  /api/tools/demos`                    every demo: where, name, size, date, map, length
//! - `GET  /api/tools/demos/detail?at=&name=`   a closer look: players seen, server name, ...
//! - `POST /api/tools/demos/rename` `{at, name, to}`  (never over another file)
//! - `POST /api/tools/demos/delete` `{at, name}`
//! - `POST /api/tools/demos/play`   `{at, name}`  starts the game with `+playdemo` (a server's
//!   HLTV demo is copied into the game first); when the game is open already, says how instead
//! - `POST /api/tools/demos/replay` `{at, name}`  the recording player's path, as a recording
//!   the Replay tool opens (a CSV in the app's tools/recordings folder)
//!
//! `at` says which folder: `game:cstrike`, `game:czero` (the player's game) or `server:<id>`
//! (a server's folder, where its HLTV writes `hltv-<YYMMDDhhmm>-<map>.dem`). Names are plain
//! `.dem` file names, never paths.
//!
//! The file (GoldSrc demo protocol 5, as Valve's engine writes it; checked on real demos):
//! - header, 544 bytes: `"HLDEMO\0\0"`, i32 demo protocol (5), i32 network protocol (48 on
//!   Steam), char[260] map, char[260] game folder, u32 map CRC, i32 directory offset
//! - directory: i32 count, then 92-byte entries: i32 type, char[64] title ("LOADING",
//!   "Playback"), i32 flags, i32 CD track, f32 seconds, i32 frames, i32 offset, i32 length
//! - frames: u8 type, f32 time, i32 frame number, then by type: 0/1 a network frame (464 bytes
//!   of client state, i32 length, the server's messages), 2 start (nothing), 3 a console
//!   command (64), 4 client data (32), 5 end of segment, 6 an event (84), 7 weapon animation
//!   (8), 8 a sound (i32 channel, i32 length, sample name, 16 more), 9 a demo buffer (i32
//!   length, data).
//! - The client state starts with f32 time and the engine's ref_params: view origin at +4,
//!   view angles at +16, spectator at +80, on ground at +84, velocity at +92, origin at +104.
//! - Player names: the userinfo strings in the messages (`\name\<name>\`), found by looking for
//!   that text rather than decoding every message.

use super::{blocking, json_answer, Fail};
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const HEADER_SIZE: usize = 544;
const ENTRY_SIZE: usize = 92;
const MAX_ENTRIES: i32 = 1024;
/// A network frame's client state before its message length.
const NET_STATE: usize = 464;
/// The biggest message a frame may carry (the engine's is 64 KB).
const MAX_MSG: usize = 1 << 17;
/// The biggest demo we look inside (a full match is 100-300 MB).
const MAX_SCAN: u64 = 1 << 30;
/// Player names kept.
const MAX_NAMES: usize = 64;
/// Path samples a second for the Replay tool.
const PATH_RATE: f32 = 10.0;

const NO_GAME: &str = "Set up the game on this computer first.";
const GONE: &str = "That demo isn't there any more.";

// ---- reading ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub kind: i32,
    pub title: String,
    pub seconds: f32,
    pub frames: i32,
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub demo_protocol: i32,
    pub net_protocol: i32,
    pub map: String,
    pub game: String,
    pub map_crc: u32,
    pub segments: Vec<Segment>,
}

impl Header {
    /// The playback's length in seconds (the "Playback" segment's, else the longest).
    pub fn seconds(&self) -> f32 {
        self.segments.iter().find(|s| s.title.eq_ignore_ascii_case("playback")).or_else(|| self.segments.iter().max_by(|a, b| a.seconds.total_cmp(&b.seconds))).map(|s| s.seconds.max(0.0)).unwrap_or(0.0)
    }
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    crate::rcon::latin1(&b[..end])
}

/// Reads a demo's header and directory. Never panics: a file that isn't a demo, or is cut
/// short, is an error in words.
pub fn read_header<R: Read + Seek>(r: &mut R, len: u64) -> Result<Header, String> {
    let mut h = [0u8; HEADER_SIZE];
    r.read_exact(&mut h).map_err(|_| "It's too short to be a demo.".to_string())?;
    if &h[0..6] != b"HLDEMO" {
        return Err("It isn't a Half-Life demo.".into());
    }
    let demo_protocol = i32_at(&h, 8);
    let net_protocol = i32_at(&h, 12);
    let map = cstr(&h[16..276]);
    let game = cstr(&h[276..536]);
    let map_crc = i32_at(&h, 536) as u32;
    let dir = i32_at(&h, 540);
    let mut segments = Vec::new();
    if dir > 0 && (dir as u64) + 4 <= len {
        r.seek(SeekFrom::Start(dir as u64)).map_err(|e| e.to_string())?;
        let mut n = [0u8; 4];
        r.read_exact(&mut n).map_err(|_| "Its list of parts is cut short.".to_string())?;
        let n = i32::from_le_bytes(n);
        if !(0..=MAX_ENTRIES).contains(&n) || dir as u64 + 4 + n as u64 * ENTRY_SIZE as u64 > len {
            return Err("Its list of parts is damaged.".into());
        }
        let mut e = vec![0u8; n as usize * ENTRY_SIZE];
        r.read_exact(&mut e).map_err(|_| "Its list of parts is cut short.".to_string())?;
        for c in e.chunks_exact(ENTRY_SIZE) {
            segments.push(Segment {
                kind: i32_at(c, 0),
                title: cstr(&c[4..68]),
                seconds: f32_at(c, 76),
                frames: i32_at(c, 80),
                offset: i32_at(c, 84) as u32,
                length: i32_at(c, 88) as u32,
            });
        }
    }
    Ok(Header { demo_protocol, net_protocol, map, game, map_crc, segments })
}

/// A point on the recording player's path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub t: f32,
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub angles: [f32; 3],
    pub on_ground: bool,
    pub spectator: bool,
}

/// What a walk through the frames finds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Details {
    pub names: Vec<String>,
    pub server: Option<String>,
    pub max_players: Option<u8>,
    pub frames: u64,
    pub commands: u64,
    /// the recording player's path (playback part, PATH_RATE a second)
    pub path: Vec<Sample>,
    /// where reading stopped early, in words
    pub problem: Option<String>,
}

/// The names in userinfo strings (`\name\<name>\`) in a message, in order.
fn names_in(msg: &[u8], out: &mut Vec<String>) {
    const KEY: &[u8] = b"\\name\\";
    let mut i = 0;
    while i + KEY.len() <= msg.len() {
        if &msg[i..i + KEY.len()] == KEY {
            let s = i + KEY.len();
            let e = msg[s..].iter().position(|&c| c == b'\\' || c == 0).map(|k| s + k).unwrap_or(msg.len());
            let name = String::from_utf8(msg[s..e].to_vec()).unwrap_or_else(|_| crate::rcon::latin1(&msg[s..e]));
            let name = name.trim().to_string();
            if !name.is_empty() && name.len() <= 64 && !out.contains(&name) && out.len() < MAX_NAMES {
                out.push(name);
            }
            i = e;
        } else {
            i += 1;
        }
    }
}

/// The server's name from svc_serverinfo (11, protocol 48: i32 protocol, i32 spawn count,
/// i32 map CRC, 16 bytes of client.dll hash, u8 max players, u8 player number, u8 ..., then
/// the game folder, the server's name, the map file).
fn server_info(msg: &[u8]) -> Option<(String, u8)> {
    let at = msg.windows(5).position(|w| w == [11, 48, 0, 0, 0])?;
    let b = msg.get(at + 1..)?;
    let maxp = *b.get(28)?;
    let rest = b.get(31..)?;
    let gd_end = rest.iter().position(|&c| c == 0)?;
    let rest = &rest[gd_end + 1..];
    let name_end = rest.iter().position(|&c| c == 0)?;
    let name = String::from_utf8(rest[..name_end].to_vec()).unwrap_or_else(|_| crate::rcon::latin1(&rest[..name_end]));
    Some((name.trim().to_string(), maxp))
}

/// Walks every frame of every part: names, the server, the player's path.
pub fn scan<R: Read + Seek>(r: &mut R, h: &Header) -> Details {
    let mut d = Details::default();
    let mut buf = vec![0u8; NET_STATE + 4];
    let mut msg: Vec<u8> = Vec::new();
    // the last tenth of a second sampled
    let mut last_bucket = i64::MIN;
    for (si, seg) in h.segments.iter().enumerate() {
        if r.seek(SeekFrom::Start(seg.offset as u64)).is_err() {
            d.problem = Some("Part of the demo is missing.".into());
            break;
        }
        let playback = seg.title.eq_ignore_ascii_case("playback") || (si > 0 && h.segments.len() == 2);
        let mut br = BufReader::with_capacity(1 << 16, &mut *r);
        let mut left = seg.length as u64;
        let fail = |d: &mut Details, why: &str| d.problem = Some(why.to_string());
        loop {
            if left < 9 {
                break;
            }
            let mut fh = [0u8; 9];
            if br.read_exact(&mut fh).is_err() {
                fail(&mut d, "The demo ends early: it may not have finished recording.");
                break;
            }
            left -= 9;
            let kind = fh[0];
            let t = f32_at(&fh, 1);
            d.frames += 1;
            let skip = |br: &mut BufReader<&mut R>, n: u64, left: &mut u64| -> bool {
                if n > *left {
                    return false;
                }
                *left -= n;
                std::io::copy(&mut br.by_ref().take(n), &mut std::io::sink()).map(|c| c == n).unwrap_or(false)
            };
            let ok = match kind {
                5 => break,
                2 => true,
                3 => {
                    d.commands += 1;
                    skip(&mut br, 64, &mut left)
                }
                4 => skip(&mut br, 32, &mut left),
                6 => skip(&mut br, 84, &mut left),
                7 => skip(&mut br, 8, &mut left),
                8 => {
                    let mut a = [0u8; 8];
                    if left < 8 || br.read_exact(&mut a).is_err() {
                        false
                    } else {
                        left -= 8;
                        let n = i32_at(&a, 4);
                        (0..=4096).contains(&n) && skip(&mut br, n as u64 + 16, &mut left)
                    }
                }
                9 => {
                    let mut a = [0u8; 4];
                    if left < 4 || br.read_exact(&mut a).is_err() {
                        false
                    } else {
                        left -= 4;
                        let n = i32_at(&a, 0);
                        (0..=MAX_MSG as i32).contains(&n) && skip(&mut br, n as u64, &mut left)
                    }
                }
                0 | 1 => {
                    if left < buf.len() as u64 || br.read_exact(&mut buf).is_err() {
                        false
                    } else {
                        left -= buf.len() as u64;
                        let n = i32_at(&buf, NET_STATE);
                        if !(0..=MAX_MSG as i32).contains(&n) || n as u64 > left {
                            false
                        } else {
                            msg.resize(n as usize, 0);
                            if br.read_exact(&mut msg).is_err() {
                                false
                            } else {
                                left -= n as u64;
                                names_in(&msg, &mut d.names);
                                if d.server.is_none() {
                                    if let Some((name, maxp)) = server_info(&msg) {
                                        d.server = Some(name);
                                        d.max_players = Some(maxp);
                                    }
                                }
                                if playback && t.is_finite() && ((t * PATH_RATE + 1e-3).floor() as i64) > last_bucket {
                                    let v3 = |at: usize| [f32_at(&buf, at), f32_at(&buf, at + 4), f32_at(&buf, at + 8)];
                                    let origin = v3(104);
                                    if origin.iter().all(|c| c.is_finite() && c.abs() < 32768.0) {
                                        d.path.push(Sample { t, origin, velocity: v3(92), angles: v3(16), on_ground: i32_at(&buf, 84) != 0, spectator: i32_at(&buf, 80) != 0 });
                                        last_bucket = (t * PATH_RATE + 1e-3).floor() as i64;
                                    }
                                }
                                true
                            }
                        }
                    }
                }
                _ => false,
            };
            if !ok {
                fail(&mut d, "Part of the demo is damaged; what's shown is from the part before it.");
                break;
            }
        }
        if d.problem.is_some() {
            break;
        }
    }
    d
}

// ---- where demos are ----------------------------------------------------------------------

/// A demo's file name as the page sends it: a plain .dem name, nothing else (as hltv.rs).
pub fn safe_name(name: &str) -> bool {
    name.len() <= 128 && name.len() > 4 && name.to_lowercase().ends_with(".dem") && !name.contains(['/', '\\', ':', '\0']) && !name.starts_with('.') && !name.chars().any(|c| c.is_control())
}

/// A name the game's command line takes (`+playdemo <name>`): letters, digits, `_ - .`.
pub fn playable_name(name: &str) -> bool {
    safe_name(name) && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

/// One folder demos are in.
#[derive(Debug, Clone)]
pub struct Place {
    /// "game:cstrike", "server:<id>"
    pub at: String,
    pub label: String,
    /// the game folder name the demo plays in ("cstrike", "czero")
    pub game: String,
    pub dirs: Vec<PathBuf>,
    pub hltv: bool,
    pub server: Option<String>,
}

fn game_label(g: &str) -> &'static str {
    if g == "czero" {
        "Condition Zero"
    } else {
        "CS 1.6"
    }
}

/// The player's game folders (CS 1.6, Condition Zero) and each server's folder on this
/// computer (servers on other machines keep their demos there; their page fetches them).
pub fn places(hl: Option<&Path>) -> Vec<Place> {
    let mut out = Vec::new();
    if let Some(hl) = hl {
        for g in ["cstrike", "czero"] {
            let d = hl.join(g);
            if d.is_dir() {
                out.push(Place { at: format!("game:{g}"), label: format!("Your game ({})", game_label(g)), game: g.into(), dirs: vec![d], hltv: false, server: None });
            }
        }
    }
    for e in crate::servers::load().into_iter().filter(|e| e.remote.is_none()) {
        let mut dirs = vec![e.dir.join(&e.game), e.dir.clone()];
        if e.game != "cstrike" {
            dirs.push(e.dir.join("cstrike"));
        }
        out.push(Place { at: format!("server:{}", e.id), label: format!("Server: {}", e.name), game: e.game.clone(), dirs, hltv: true, server: Some(e.id.clone()) });
    }
    out
}

fn find(places: &[Place], at: &str, name: &str) -> Option<(Place, PathBuf)> {
    if !safe_name(name) {
        return None;
    }
    let p = places.iter().find(|p| p.at == at)?;
    let path = p.dirs.iter().map(|d| d.join(name)).find(|f| f.is_file())?;
    Some((p.clone(), path))
}

fn header_of(path: &Path) -> Result<Header, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    read_header(&mut f, len)
}

fn modified(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

fn list(places: &[Place]) -> Value {
    let mut rows: Vec<(u64, Value)> = Vec::new();
    for p in places {
        let mut seen = std::collections::HashSet::new();
        for d in &p.dirs {
            for f in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                let Ok(meta) = f.metadata() else { continue };
                if !meta.is_file() || !safe_name(&name) || !seen.insert(name.to_lowercase()) {
                    continue;
                }
                let when = modified(&meta);
                let mut v = json!({"at": p.at, "where": p.label, "game": p.game, "hltv": p.hltv || name.to_lowercase().starts_with("hltv-"),
                                   "name": name, "size": meta.len(), "modified": when, "playable": playable_name(&name)});
                match header_of(&f.path()) {
                    Ok(h) => {
                        v["map"] = json!(h.map);
                        v["seconds"] = json!(h.seconds());
                        v["protocol"] = json!(h.net_protocol);
                        v["demoGame"] = json!(h.game);
                    }
                    Err(e) => v["error"] = json!(e),
                }
                rows.push((when, v));
            }
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    json!({"demos": rows.into_iter().map(|r| r.1).collect::<Vec<_>>(),
           "places": places.iter().map(|p| json!({"at": p.at, "label": p.label, "folder": p.dirs.first().map(|d| d.display().to_string())})).collect::<Vec<_>>(),
           "gameOpen": crate::sys::game_running()})
}

fn detail(path: &Path) -> Result<Value, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let h = read_header(&mut f, len)?;
    let d = if len <= MAX_SCAN { scan(&mut f, &h) } else { Details { problem: Some("This demo is too big to look inside.".into()), ..Default::default() } };
    let pov = d.path.iter().filter(|s| !s.spectator).count();
    Ok(json!({
        "map": h.map, "game": h.game, "protocol": h.net_protocol, "demoProtocol": h.demo_protocol,
        "seconds": h.seconds(),
        "parts": h.segments.iter().map(|s| json!({"title": s.title, "seconds": s.seconds, "frames": s.frames})).collect::<Vec<_>>(),
        "names": d.names, "server": d.server, "maxPlayers": d.max_players,
        "frames": d.frames, "problem": d.problem,
        // a path worth replaying: the recorder was playing, not spectating, for a while
        "canReplay": pov >= 10,
    }))
}

// ---- changing things ----------------------------------------------------------------------

/// Whether HLTV may still be writing this demo (its server is up and it changed lately).
fn maybe_recording(p: &Place, path: &Path) -> bool {
    let recent = std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 90);
    p.server.as_deref().is_some_and(crate::servers::running) && recent
}

fn rename(places: &[Place], at: &str, name: &str, to: &str) -> Result<Value, String> {
    let (p, path) = find(places, at, name).ok_or(GONE)?;
    let to = if to.to_lowercase().ends_with(".dem") { to.trim().to_string() } else { format!("{}.dem", to.trim()) };
    if !safe_name(&to) {
        return Err("That name can't be used. Use letters, numbers, - and _.".into());
    }
    if to == name {
        return Ok(json!({"ok": true, "name": to}));
    }
    if maybe_recording(&p, &path) {
        return Err("HLTV may still be recording this demo. Stop the server's HLTV first.".into());
    }
    let dest = path.with_file_name(&to);
    // a different file of that name (not just this one in other letter case)
    if dest.exists() && !to.eq_ignore_ascii_case(name) {
        return Err(format!("There's already a demo called {to}."));
    }
    std::fs::rename(&path, &dest).map_err(|e| format!("Couldn't rename it ({e})."))?;
    Ok(json!({"ok": true, "name": to, "message": format!("Renamed to {to}.")}))
}

fn delete(places: &[Place], at: &str, name: &str) -> Result<Value, String> {
    let (p, path) = find(places, at, name).ok_or(GONE)?;
    if maybe_recording(&p, &path) {
        return Err("HLTV may still be recording this demo. Stop the server's HLTV first.".into());
    }
    std::fs::remove_file(&path).map_err(|e| format!("Couldn't delete it ({e})."))?;
    Ok(json!({"ok": true, "message": format!("Deleted {name}.")}))
}

/// What to type in the game's console to watch a demo.
fn console_hint(name: &str) -> String {
    let stem = name.strip_suffix(".dem").or_else(|| name.strip_suffix(".DEM")).unwrap_or(name);
    format!("playdemo {stem}")
}

fn play(places: &[Place], game: &crate::game::Game, at: &str, name: &str) -> Result<Value, String> {
    let (p, path) = find(places, at, name).ok_or(GONE)?;
    if !playable_name(name) {
        return Err("The game can't be started with a demo named like this. Rename it to letters, numbers, - and _ first.".into());
    }
    // a server's demo goes into the player's game folder first (the game plays from there)
    let mut copied = false;
    if p.server.is_some() {
        let dest_dir = game.hl.join(&p.game);
        let dest = dest_dir.join(name);
        let same = std::fs::metadata(&dest).ok().zip(std::fs::metadata(&path).ok()).is_some_and(|(a, b)| a.len() == b.len());
        if dest.exists() && !same {
            return Err(format!("Your game already has a different demo called {name}. Rename one of them first."));
        }
        if !same {
            if maybe_recording(&p, &path) {
                return Err("HLTV is still recording this demo. Stop the server's HLTV first, or watch an earlier one.".into());
            }
            let part = dest.with_file_name(format!("{name}.part"));
            std::fs::copy(&path, &part).map_err(|e| format!("Couldn't copy the demo into your game ({e})."))?;
            crate::sys::rename_over(&part, &dest).map_err(|e| format!("Couldn't copy the demo into your game ({e})."))?;
            copied = true;
        }
    }
    let hint = console_hint(name);
    if crate::sys::game_running() {
        return Ok(json!({"ok": false, "gameOpen": true, "copied": copied, "command": hint,
            "message": format!("The game is open already. Open its console (the ~ key) and type: {hint}")}));
    }
    let appid = if p.game == "czero" { "80" } else { "10" };
    let stem = hint.trim_start_matches("playdemo ").to_string();
    crate::steam::launch(&game.hl, appid, &["-condebug".into(), "+playdemo".into(), stem])?;
    Ok(json!({"ok": true, "copied": copied, "message": "Starting the game with your demo…"}))
}

/// The recording player's path as a recording the Replay tool reads (gm_simrec's CSV: a
/// player line and a sample line per tenth of a second).
pub fn replay_csv(name: &str, d: &Details) -> String {
    let clean: String = name.chars().filter(|c| *c != ',' && !c.is_control()).collect();
    let mut out = format!("P,1,{},0\n", if clean.is_empty() { "Demo" } else { &clean });
    let t0 = d.path.first().map(|s| s.t).unwrap_or(0.0);
    for s in d.path.iter().filter(|s| !s.spectator) {
        out.push_str(&format!(
            "S,{:.2},1,0,{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},0,{},0,0,0,100,0,0,0\n",
            s.t - t0,
            s.origin[0],
            s.origin[1],
            s.origin[2],
            s.velocity[0],
            s.velocity[1],
            s.velocity[2],
            s.angles[1],
            s.angles[0],
            s.on_ground as u8
        ));
    }
    out
}

/// Keeps only `[A-Za-z0-9_-]` (anything else becomes `-`), at most 64 characters; `empty`
/// when nothing is left. For text from a demo (its file name, the map its header names) that
/// goes into a file name: a crafted header must never reach outside the folder.
pub fn file_part(s: &str, dash_ok: bool, empty: &str) -> String {
    let v: String = s.chars().take(64).map(|c| if c.is_ascii_alphanumeric() || c == '-' || (c == '_' && !dash_ok) { c } else { '-' }).collect();
    let v = v.trim_matches('-').to_string();
    if v.is_empty() {
        empty.to_string()
    } else {
        v
    }
}

/// Where a demo's path goes as a recording: `<dir>/demo-<stem>_<map>_1.csv` (the Replay tool
/// finds the map in the name). None if it wouldn't be a plain file right in `dir`.
pub fn replay_file(dir: &Path, name: &str, map: &str) -> Option<PathBuf> {
    // the stem keeps no '_' (the map comes after the first one)
    let stem = file_part(name.trim_end_matches(".dem").trim_end_matches(".DEM"), true, "demo");
    let map = file_part(map, false, "unknown");
    let file = dir.join(format!("demo-{stem}_{map}_1.csv"));
    (file.parent() == Some(dir)).then_some(file)
}

fn to_replay(places: &[Place], at: &str, name: &str) -> Result<Value, String> {
    let (_, path) = find(places, at, name).ok_or(GONE)?;
    let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len > MAX_SCAN {
        return Err("This demo is too big to look inside.".into());
    }
    let h = read_header(&mut f, len)?;
    let d = scan(&mut f, &h);
    if d.path.iter().filter(|s| !s.spectator).count() < 10 {
        return Err("This demo has no player walking around in it (an HLTV demo follows a camera, not a player).".into());
    }
    let who = d.names.first().cloned().unwrap_or_else(|| "You".into());
    let dir = crate::sys::app_dir().join("tools").join("recordings");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the recordings folder ({e})."))?;
    let file = replay_file(&dir, name, &h.map).ok_or("This demo's name or map can't be used for a file.")?;
    crate::sys::write_atomic(&file, replay_csv(&who, &d).as_bytes()).map_err(|e| format!("Couldn't save it ({e})."))?;
    let id = super::cache::short_hash(&[&file.to_string_lossy()]);
    Ok(json!({"ok": true, "id": id, "map": h.map, "message": "Opening the path in Replay…"}))
}

// ---- routes -------------------------------------------------------------------------------

fn places_for(st: &Shared) -> Vec<Place> {
    places(st.local_game().as_deref().map(|g| g.hl.as_path()))
}

async fn api_list(State(st): State<Shared>) -> Response {
    let r = blocking(move || {
        if st.local_game().is_none() {
            return Err(Fail::new(StatusCode::CONFLICT, NO_GAME));
        }
        Ok(list(&places_for(&st)))
    })
    .await;
    json_answer(r)
}

#[derive(Deserialize)]
struct Which {
    #[serde(default)]
    at: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    to: String,
}

async fn api_detail(State(st): State<Shared>, Query(q): Query<Which>) -> Response {
    let r = blocking(move || {
        let places = places_for(&st);
        let (_, path) = find(&places, &q.at, &q.name).ok_or_else(|| Fail::new(StatusCode::OK, GONE))?;
        detail(&path).map_err(|e| Fail::new(StatusCode::OK, e))
    })
    .await;
    json_answer(r)
}

async fn act(st: Shared, h: HeaderMap, f: impl FnOnce(&Shared, &[Place]) -> Result<Value, String> + Send + 'static) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let r = blocking(move || {
        let places = places_for(&st);
        f(&st, &places).map_err(|e| Fail::new(StatusCode::OK, e))
    })
    .await;
    json_answer(r)
}

async fn api_rename(State(st): State<Shared>, h: HeaderMap, Json(q): Json<Which>) -> Response {
    act(st, h, move |_, p| rename(p, &q.at, &q.name, &q.to)).await
}

async fn api_delete(State(st): State<Shared>, h: HeaderMap, Json(q): Json<Which>) -> Response {
    act(st, h, move |_, p| delete(p, &q.at, &q.name)).await
}

async fn api_play(State(st): State<Shared>, h: HeaderMap, Json(q): Json<Which>) -> Response {
    act(st, h, move |st, p| {
        let game = st.local_game().ok_or(NO_GAME)?;
        play(p, &game, &q.at, &q.name)
    })
    .await
}

async fn api_replay(State(st): State<Shared>, h: HeaderMap, Json(q): Json<Which>) -> Response {
    act(st, h, move |_, p| to_replay(p, &q.at, &q.name)).await
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/demos", get(api_list))
        .route("/api/tools/demos/detail", get(api_detail))
        .route("/api/tools/demos/rename", post(api_rename))
        .route("/api/tools/demos/delete", post(api_delete))
        .route("/api/tools/demos/play", post(api_play))
        .route("/api/tools/demos/replay", post(api_replay))
}

#[cfg(test)]
pub mod fixture {
    /// A small demo the way the engine writes one: header, a LOADING part with the server's
    /// info and two players' userinfo, a Playback part with a console command, client data
    /// and network frames walking along x, then the directory.
    pub fn build() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(b"HLDEMO\0\0");
        b.extend(5i32.to_le_bytes());
        b.extend(48i32.to_le_bytes());
        let mut name = [0u8; 260];
        name[..8].copy_from_slice(b"de_dust2");
        b.extend(name);
        let mut game = [0u8; 260];
        game[..7].copy_from_slice(b"cstrike");
        b.extend(game);
        b.extend(0x1234_5678u32.to_le_bytes());
        b.extend(0i32.to_le_bytes()); // directory offset, filled in below
        assert_eq!(b.len(), super::HEADER_SIZE);

        let frame = |b: &mut Vec<u8>, kind: u8, t: f32, n: i32| {
            b.push(kind);
            b.extend(t.to_le_bytes());
            b.extend(n.to_le_bytes());
        };
        let net = |b: &mut Vec<u8>, t: f32, n: i32, origin: [f32; 3], msg: &[u8]| {
            frame(b, 1, t, n);
            let mut state = vec![0u8; super::NET_STATE];
            state[0..4].copy_from_slice(&t.to_le_bytes());
            for k in 0..3 {
                state[4 + k * 4..8 + k * 4].copy_from_slice(&(origin[k]).to_le_bytes()); // view origin
                state[104 + k * 4..108 + k * 4].copy_from_slice(&origin[k].to_le_bytes());
            }
            state[16..20].copy_from_slice(&5.0f32.to_le_bytes()); // pitch
            state[20..24].copy_from_slice(&90.0f32.to_le_bytes()); // yaw
            state[84..88].copy_from_slice(&1i32.to_le_bytes()); // on ground
            state[92..96].copy_from_slice(&250.0f32.to_le_bytes()); // moving along x
            b.extend(state);
            b.extend((msg.len() as i32).to_le_bytes());
            b.extend(msg);
        };

        let load_at = b.len();
        let mut info = vec![11u8];
        info.extend(48i32.to_le_bytes());
        info.extend(7i32.to_le_bytes());
        info.extend(0x1234_5678u32.to_le_bytes());
        info.extend([0u8; 16]);
        info.extend([20u8, 1, 0]);
        info.extend(b"cstrike\0My \"Test\" Server\0maps/de_dust2.bsp\0\0");
        info.extend(b"\x0d\x00\x01\x00\x00\x00\\model\\gign\\name\\Alice\\topcolor\\0\0");
        info.extend(b"\x0d\x01\x02\x00\x00\x00\\*bot\\1\\name\\[LS] Bob\0");
        net(&mut b, 100.0, 1, [0.0; 3], &info);
        frame(&mut b, 5, 100.1, 2);
        let load_len = b.len() - load_at;

        let play_at = b.len();
        frame(&mut b, 2, 0.0, 3);
        frame(&mut b, 3, 0.0, 4);
        let mut cmd = [0u8; 64];
        cmd[..7].copy_from_slice(b"+attack");
        b.extend(cmd);
        frame(&mut b, 4, 0.0, 5);
        b.extend([0u8; 32]);
        for i in 0..40 {
            let t = i as f32 * 0.05;
            net(&mut b, t, 6 + i, [t * 250.0, 64.0, -100.0], if i == 10 { b"\\name\\Alice\\\0".as_slice() } else { b"\x01".as_slice() });
        }
        frame(&mut b, 8, 1.0, 50);
        b.extend(6i32.to_le_bytes());
        b.extend(10i32.to_le_bytes());
        b.extend(b"a.wav\0zz\0\0\0");
        b.extend([0u8; 15]);
        frame(&mut b, 5, 2.0, 51);
        let play_len = b.len() - play_at;

        let dir_at = b.len();
        b[540..544].copy_from_slice(&(dir_at as i32).to_le_bytes());
        b.extend(2i32.to_le_bytes());
        for (kind, title, secs, frames, at, len) in [(0, "LOADING", 0.0f32, 0, load_at, load_len), (1, "Playback", 1.95, 40, play_at, play_len)] {
            b.extend((kind as i32).to_le_bytes());
            let mut t = [0u8; 64];
            t[..title.len()].copy_from_slice(title.as_bytes());
            b.extend(t);
            b.extend(0i32.to_le_bytes());
            b.extend((-1i32).to_le_bytes());
            b.extend(secs.to_le_bytes());
            b.extend((frames as i32).to_le_bytes());
            b.extend((at as i32).to_le_bytes());
            b.extend((len as i32).to_le_bytes());
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_the_header_and_parts() {
        let b = fixture::build();
        let h = read_header(&mut Cursor::new(&b), b.len() as u64).unwrap();
        assert_eq!((h.demo_protocol, h.net_protocol), (5, 48));
        assert_eq!((h.map.as_str(), h.game.as_str(), h.map_crc), ("de_dust2", "cstrike", 0x1234_5678));
        assert_eq!(h.segments.len(), 2);
        assert_eq!(h.segments[1].title, "Playback");
        assert_eq!(h.segments[1].frames, 40);
        assert!((h.seconds() - 1.95).abs() < 1e-6);
    }

    #[test]
    fn walks_the_frames() {
        let b = fixture::build();
        let mut c = Cursor::new(&b);
        let h = read_header(&mut c, b.len() as u64).unwrap();
        let d = scan(&mut c, &h);
        assert_eq!(d.problem, None);
        assert_eq!(d.names, vec!["Alice".to_string(), "[LS] Bob".to_string()]);
        assert_eq!(d.server.as_deref(), Some("My \"Test\" Server"));
        assert_eq!(d.max_players, Some(20));
        assert_eq!(d.commands, 1);
        // 40 network frames at 20 a second, sampled at 10 a second
        assert_eq!(d.path.len(), 20, "{:?}", d.path.iter().map(|s| s.t).collect::<Vec<_>>());
        let last = d.path.last().unwrap();
        assert!((last.origin[0] - last.t * 250.0).abs() < 0.01 && last.origin[1] == 64.0);
        assert_eq!((last.angles[0], last.angles[1], last.on_ground, last.velocity[0]), (5.0, 90.0, true, 250.0));
        let csv = replay_csv("Alice", &d);
        assert!(csv.starts_with("P,1,Alice,0\nS,0.00,1,0,0.0,64.0,-100.0,250.0,0.0,0.0,90.0,5.0,0,1,"), "{csv}");
        let rec = crate::tools::replay::parse_recording(std::io::BufReader::new(csv.as_bytes())).unwrap();
        assert_eq!(rec.players.len(), 1);
    }

    #[test]
    fn damaged_demos_are_words_not_panics() {
        let b = fixture::build();
        assert!(read_header(&mut Cursor::new(&b[..100]), 100).is_err());
        let mut not = b.clone();
        not[0] = b'X';
        assert_eq!(read_header(&mut Cursor::new(&not), not.len() as u64).unwrap_err(), "It isn't a Half-Life demo.");
        // the directory points past the end
        let mut bad = b.clone();
        bad[540..544].copy_from_slice(&((b.len() - 10) as i32).to_le_bytes());
        assert!(read_header(&mut Cursor::new(&bad), bad.len() as u64).is_err());
        // cut in the middle of the playback: what came before is kept
        let h = read_header(&mut Cursor::new(&b), b.len() as u64).unwrap();
        let mut cut = b.clone();
        // the first network frame's client state: after start, a command and client data
        let state = h.segments[1].offset as usize + 9 + (9 + 64) + (9 + 32) + 9;
        cut[state + NET_STATE..state + NET_STATE + 4].copy_from_slice(&i32::MAX.to_le_bytes());
        let d = scan(&mut Cursor::new(&cut), &h);
        assert!(d.problem.is_some());
        assert_eq!(d.names.len(), 2);
        // every byte flipped in turn never panics
        for i in (0..b.len()).step_by(7) {
            let mut x = b.clone();
            x[i] ^= 0xA5;
            let len = x.len() as u64;
            if let Ok(h) = read_header(&mut Cursor::new(&x), len) {
                let _ = scan(&mut Cursor::new(&x), &h);
            }
        }
    }


    #[test]
    fn crafted_headers_stay_in_the_folder() {
        let dir = PathBuf::from("/tmp/recs");
        for map in ["../../x", "..\\..\\x", "/etc/passwd", "a/b", "", "..", "de_dust2\0evil", &"x".repeat(500)] {
            let f = replay_file(&dir, "my demo.dem", map).unwrap();
            assert_eq!(f.parent(), Some(dir.as_path()), "{map}");
            let n = f.file_name().unwrap().to_string_lossy().to_string();
            assert!(n.starts_with("demo-my-demo_") && n.ends_with("_1.csv") && n.len() < 160 && !n.contains(".."), "{n}");
        }
        assert_eq!(replay_file(&dir, "a.dem", "de_dust2").unwrap(), dir.join("demo-a_de_dust2_1.csv"));
        // a demo whose header names a path as its map: the header reads, the file stays put
        let mut b = fixture::build();
        b[16..32].copy_from_slice(b"../../../evil\0\0\0");
        let h = read_header(&mut Cursor::new(&b), b.len() as u64).unwrap();
        assert_eq!(h.map, "../../../evil");
        assert_eq!(replay_file(&dir, "x.dem", &h.map).unwrap(), dir.join("demo-x_evil_1.csv"));
        // names with line breaks or commas can't add rows to the recording
        let d = Details { path: vec![], ..Default::default() };
        assert_eq!(replay_csv("a,b\nS,1,2", &d), "P,1,abS12,0\n");
    }

    #[test]
    fn names_are_checked() {
        assert!(safe_name("my demo.dem") && safe_name("hltv-2611061830-de_dust2.dem") && safe_name("A.DEM"));
        assert!(!safe_name("../x.dem") && !safe_name("a/b.dem") && !safe_name(".dem") && !safe_name("x.txt") && !safe_name("a\u{7}.dem"));
        assert!(playable_name("my_demo-1.dem") && !playable_name("my demo.dem") && !playable_name("ç.dem"));
        assert_eq!(console_hint("abc.dem"), "playdemo abc");
    }

    #[test]
    fn files_in_places() {
        let root = std::env::temp_dir().join(format!("ls-demos-{}", std::process::id()));
        let dir = root.join("Half-Life/cstrike");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("one.dem"), fixture::build()).unwrap();
        std::fs::write(dir.join("broken.dem"), b"nope").unwrap();
        std::fs::write(dir.join("two.dem"), fixture::build()).unwrap();
        let places = vec![Place { at: "game:cstrike".into(), label: "Your game".into(), game: "cstrike".into(), dirs: vec![dir.clone()], hltv: false, server: None }];
        let v = list(&places);
        let demos = v["demos"].as_array().unwrap();
        assert_eq!(demos.len(), 3);
        let one = demos.iter().find(|d| d["name"] == "one.dem").unwrap();
        assert_eq!(one["map"], "de_dust2");
        assert!(demos.iter().find(|d| d["name"] == "broken.dem").unwrap()["error"].is_string());
        let d = detail(&dir.join("one.dem")).unwrap();
        assert_eq!(d["names"], json!(["Alice", "[LS] Bob"]));
        assert_eq!(d["canReplay"], true);
        // rename: not over another, not to a bad name, then fine
        assert!(rename(&places, "game:cstrike", "one.dem", "two").unwrap_err().contains("already"));
        assert!(rename(&places, "game:cstrike", "one.dem", "../x").is_err());
        assert_eq!(rename(&places, "game:cstrike", "one.dem", "first match").unwrap()["name"], "first match.dem");
        assert!(dir.join("first match.dem").is_file() && !dir.join("one.dem").exists());
        assert!(delete(&places, "game:cstrike", "one.dem").is_err());
        assert!(delete(&places, "game:nowhere", "two.dem").is_err());
        delete(&places, "game:cstrike", "two.dem").unwrap();
        assert!(!dir.join("two.dem").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> reads every demo in cstrike/ and czero/.
    #[test]
    fn real_demos() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        for g in ["cstrike", "czero"] {
            for f in std::fs::read_dir(PathBuf::from(&hl).join(g)).into_iter().flatten().flatten() {
                let p = f.path();
                if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dem")) {
                    continue;
                }
                let t = std::time::Instant::now();
                match detail(&p) {
                    Ok(v) => println!("{}: map {} {:.0}s, {} names, server {}, problem {} ({} ms)", p.display(), v["map"], v["seconds"].as_f64().unwrap_or(0.0), v["names"].as_array().map(|a| a.len()).unwrap_or(0), v["server"], v["problem"], t.elapsed().as_millis()),
                    Err(e) => println!("{}: ERROR {e}", p.display()),
                }
            }
        }
    }
}
