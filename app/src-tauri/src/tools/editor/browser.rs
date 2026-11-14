//! The map editor's Content Browser (agent P5; docs/notes/Assets.md 3.2): every model, sprite and
//! sound in the game's folders (where the game reads them, first one wins, like the game) and
//! next to the project, each with where it came from: the game itself, a GameBanana download
//! (Get more's record: the page), or yours. Sprites are decoded for thumbnails and the animated
//! preview, sounds described (rate, bits, length, and what GoldSrc can't play).
//!   GET /api/tools/editor/content/list?kind=model|sprite|sound
//!   GET /api/tools/editor/content/sprite.png?folder=&path=&frame=
//!   GET /api/tools/editor/content/sprite/info?folder=&path=
//!   GET /api/tools/editor/content/sound/info?folder=&path=
//! Paths from the page are only used when they're clean asset paths (assets.rs `clean_rel`)
//! inside one of the game's folders, read through the same checks as the rest (no links, the
//! right type, size caps).

use super::assets::{clean_rel, vet};
use super::{blocking, Ed};
use crate::goldsrc::spr;
use crate::mapcopy::Finder;
use crate::server::Shared;
use crate::tools::doctor;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const MAX_ITEMS: usize = 6000;
const MAX_DEPTH: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Model,
    Sprite,
    Sound,
}

impl Kind {
    fn parse(s: &str) -> Option<Kind> {
        match s {
            "model" => Some(Kind::Model),
            "sprite" => Some(Kind::Sprite),
            "sound" => Some(Kind::Sound),
            _ => None,
        }
    }
    fn top(self) -> &'static str {
        match self {
            Kind::Model => "models",
            Kind::Sprite => "sprites",
            Kind::Sound => "sound",
        }
    }
    fn takes(self, low: &str) -> bool {
        match self {
            Kind::Model => low.ends_with(".mdl"),
            Kind::Sprite => low.ends_with(".spr"),
            Kind::Sound => low.ends_with(".wav") || low.ends_with(".mp3"),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Kind::Model => "model",
            Kind::Sprite => "sprite",
            Kind::Sound => "sound",
        }
    }
}

/// The folders the game reads (its own, its downloads, cstrike for CZ, valve), by name.
fn game_folders(hl: &Path, game: &str) -> Vec<(String, PathBuf)> {
    doctor::game_path(hl, game).into_iter().map(|d| (d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), d)).collect()
}

/// A model's companion files (`xT.mdl` textures, `x01.mdl` sequence groups) aren't models of their own.
fn companion(low: &str, names: &HashSet<String>) -> bool {
    // byte-safe: file names can hold any characters, so never slice a str mid-character
    let Some(stem) = low.strip_suffix(".mdl") else { return false };
    if let Some(base) = stem.strip_suffix('t') {
        if names.contains(&format!("{base}.mdl")) {
            return true;
        }
    }
    let b = stem.as_bytes();
    // the last two bytes are ASCII digits, so the cut before them is a character boundary
    if b.len() > 2 && b[b.len() - 2..].iter().all(|c| c.is_ascii_digit()) && names.contains(&format!("{}.mdl", &stem[..stem.len() - 2])) {
        return true;
    }
    false
}

/// Every file of a kind under `<dir>/<top>/` (relative paths, forward slashes).
fn walk(dir: &Path, kind: Kind, budget: usize) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.join(kind.top()), kind.top().to_string(), 0usize)];
    while let Some((d, rel, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let entries: Vec<(String, PathBuf, bool, u64)> = rd
            .flatten()
            .map(|e| {
                let ft = e.file_type().ok();
                let md = e.metadata().ok();
                (e.file_name().to_string_lossy().to_string(), e.path(), ft.is_some_and(|t| t.is_dir()), md.map(|m| m.len()).unwrap_or(0))
            })
            .collect();
        let names: HashSet<String> = entries.iter().filter(|e| !e.2).map(|e| e.0.to_lowercase()).collect();
        for (name, path, is_dir, size) in entries {
            let r = format!("{rel}/{name}");
            if is_dir {
                if depth < MAX_DEPTH && !path.is_symlink() {
                    stack.push((path, r, depth + 1));
                }
                continue;
            }
            let low = name.to_lowercase();
            if !kind.takes(&low) || (kind == Kind::Model && companion(&low, &names)) || clean_rel(&r).is_none() {
                continue;
            }
            out.push((r, size));
            if out.len() >= budget {
                return out;
            }
        }
    }
    out
}

/// Who installed a file: Get more's record (path → page).
struct Downloads {
    owners: HashMap<String, (String, String)>,
    pages: HashMap<String, Value>,
}

fn downloads(app: &Path, game_dir: &Path) -> Downloads {
    let recs = crate::content::Records::new(app);
    let all = recs.for_game(game_dir);
    Downloads { owners: recs.owners(game_dir), pages: all.into_iter().collect() }
}

pub fn list(hl: &Path, game: &str, app: &Path, project: Option<&Path>, kind: Kind) -> Value {
    let dl = downloads(app, &hl.join(game));
    // (T: files a Loner-Strike Community creation installed, listed under Community)
    let community = crate::tools::credits::community_files_in(app);
    let mut seen: HashSet<String> = HashSet::new();
    let mut items = Vec::new();
    let mut pages: HashMap<String, (String, String, usize)> = HashMap::new();
    let mut places: Vec<(String, PathBuf, bool)> = game_folders(hl, game).into_iter().map(|(n, d)| (n, d, false)).collect();
    if let Some(p) = project.filter(|p| p.is_dir()) {
        places.push(("project".into(), p.to_path_buf(), true));
    }
    for (folder, dir, mine) in places {
        for (rel, size) in walk(&dir, kind, MAX_ITEMS.saturating_sub(items.len())) {
            let low = rel.to_lowercase();
            if !seen.insert(low.clone()) {
                continue; // (the game uses the first one it finds)
            }
            let (source, page) = if mine {
                ("mine", None)
            } else if let Some(who) = community.get(&low) {
                ("community", Some(json!({"key": who["key"], "name": who["name"], "url": who["url"], "author": who["author"]})))
            } else if let Some((key, name)) = dl.owners.get(&low) {
                let url = dl.pages.get(key).and_then(|r| r["url"].as_str()).unwrap_or("").to_string();
                let e = pages.entry(key.clone()).or_insert((name.clone(), url.clone(), 0));
                e.2 += 1;
                ("downloaded", Some(json!({"key": key, "name": name, "url": url, "author": dl.pages.get(key).and_then(|r| r["author"].as_str()).unwrap_or("")})))
            } else if doctor::is_stock(&rel, game) || !folder.ends_with("_downloads") {
                // the game's own folders (Steam's files; there's no list of them, so files copied
                // in by hand count too), not a download folder without a record
                ("game", None)
            } else {
                ("other", None)
            };
            let name = rel.rsplit('/').next().unwrap_or(&rel);
            let stem = name.rsplit_once('.').map(|x| x.0).unwrap_or(name);
            items.push(json!({"kind": kind.name(), "folder": folder, "path": rel, "name": stem, "size": size, "source": source, "page": page}));
        }
        if items.len() >= MAX_ITEMS {
            break;
        }
    }
    let mut groups: Vec<Value> = pages.into_iter().map(|(key, (name, url, n))| json!({"key": key, "name": name, "url": url, "count": n})).collect();
    groups.sort_by(|a, b| a["name"].as_str().unwrap_or("").to_lowercase().cmp(&b["name"].as_str().unwrap_or("").to_lowercase()));
    json!({"items": items, "downloads": groups, "capped": items.len() >= MAX_ITEMS})
}

/// The file a request names, vetted (clean path, in that folder, no links, its real type).
fn resolve(hl: &Path, game: &str, project: Option<&Path>, folder: &str, rel: &str) -> Option<PathBuf> {
    let rel = clean_rel(rel)?;
    let root = if folder == "project" { project?.to_path_buf() } else { game_folders(hl, game).into_iter().find(|(n, _)| n.eq_ignore_ascii_case(folder))?.1 };
    let p = Finder::default().find(&root, &rel)?;
    vet(&root, &p, &rel)
}

#[derive(Deserialize)]
struct ListQ {
    kind: String,
}

#[derive(Deserialize)]
struct FileQ {
    folder: String,
    path: String,
    #[serde(default)]
    frame: usize,
}

fn context(st: &Shared, ed: &Ed) -> Option<(PathBuf, String, PathBuf, Option<PathBuf>)> {
    let g = st.local_game()?;
    let game = if g.is_czero() { "czero" } else { "cstrike" }.to_string();
    let project = ed.lock().as_ref().and_then(|s| s.project.as_deref().and_then(Path::parent).map(Path::to_path_buf));
    Some((g.hl.clone(), game, crate::sys::app_dir(), project))
}

async fn api_list(State(st): State<Shared>, Extension(ed): Extension<Ed>, Query(q): Query<ListQ>) -> Response {
    blocking(move || {
        let kind = Kind::parse(&q.kind).ok_or("Which kind: model, sprite or sound?")?;
        let Some((hl, game, app, project)) = context(&st, &ed) else { return Ok(json!({"items": [], "downloads": [], "game": false})) };
        let mut v = list(&hl, &game, &app, project.as_deref(), kind);
        v["game"] = json!(true);
        Ok(v)
    })
    .await
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error": "That file isn't there, or it can't be read."}))).into_response()
}

async fn api_sprite_png(State(st): State<Shared>, Extension(ed): Extension<Ed>, Query(q): Query<FileQ>) -> Response {
    let r = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
        let (hl, game, _, project) = context(&st, &ed)?;
        let p = resolve(&hl, &game, project.as_deref(), &q.folder, &q.path)?;
        let bytes = crate::caps::read_file(&p, "sprite", 8 << 20).ok()?;
        let s = spr::parse(&bytes, q.frame == 0).ok()?;
        let (w, h, px) = s.rgba(q.frame.min(s.frames.len().saturating_sub(1)))?;
        crate::goldsrc::png(w, h, &px, true).ok()
    })
    .await
    .ok()
    .flatten();
    match r {
        Some(png) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=300")], png).into_response(),
        None => not_found(),
    }
}

async fn api_sprite_info(State(st): State<Shared>, Extension(ed): Extension<Ed>, Query(q): Query<FileQ>) -> Response {
    blocking(move || {
        let (hl, game, _, project) = context(&st, &ed).ok_or("Set up the game first.")?;
        let p = resolve(&hl, &game, project.as_deref(), &q.folder, &q.path).ok_or("That sprite isn't there, or it can't be read.")?;
        let bytes = crate::caps::read_file(&p, "sprite", 8 << 20)?;
        let s = spr::parse(&bytes, false).map_err(|e| format!("That sprite can't be read ({e})."))?;
        Ok(json!({"frames": s.frames.len(), "width": s.width, "height": s.height, "format": s.format.name(), "orientation": spr::orientation_name(s.orientation), "size": bytes.len()}))
    })
    .await
}

/// What a sound file is, and whether GoldSrc plays it (8/16-bit PCM up to 44.1 kHz).
pub fn sound_info(bytes: &[u8], mp3: bool) -> Value {
    if mp3 {
        return json!({"format": "mp3", "note": "MP3s play only through the music commands (mp3 play), not ambient_generic or entity sounds."});
    }
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return json!({"format": "unknown", "warn": "This isn't a WAV file the game can read."});
    }
    let (mut fmt, mut ch, mut rate, mut bits, mut data) = (0u16, 0u16, 0u32, 0u16, 0u32);
    let mut cues = false;
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = at + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            fmt = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
            ch = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
            rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
            bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
        } else if id == b"data" {
            data = len as u32;
        } else if id == b"cue " {
            cues = true;
        }
        at = body.saturating_add(len + (len & 1));
    }
    let per_sec = rate as u64 * ch.max(1) as u64 * (bits.max(8) as u64 / 8);
    let seconds = if per_sec > 0 { data as f64 / per_sec as f64 } else { 0.0 };
    let mut warn = Vec::new();
    if fmt != 1 {
        warn.push("not plain PCM (the game plays only uncompressed WAVs)");
    }
    if bits != 8 && bits != 16 {
        warn.push("not 8- or 16-bit");
    }
    if rate > 44100 {
        warn.push("over 44.1 kHz");
    }
    if ch > 1 {
        warn.push("stereo (the game plays it, but in the world it isn't placed: use mono for map sounds)");
    }
    json!({"format": "wav", "rate": rate, "bits": bits, "channels": ch, "seconds": (seconds * 100.0).round() / 100.0, "loops": cues,
        "warn": if warn.is_empty() { Value::Null } else { json!(format!("Check it: {}.", warn.join("; "))) }})
}

async fn api_sound_info(State(st): State<Shared>, Extension(ed): Extension<Ed>, Query(q): Query<FileQ>) -> Response {
    blocking(move || {
        let (hl, game, _, project) = context(&st, &ed).ok_or("Set up the game first.")?;
        let p = resolve(&hl, &game, project.as_deref(), &q.folder, &q.path).ok_or("That sound isn't there, or it can't be read.")?;
        let bytes = crate::caps::read_file(&p, "sound", 32 << 20)?;
        let mut v = sound_info(&bytes, q.path.to_ascii_lowercase().ends_with(".mp3"));
        v["size"] = json!(bytes.len());
        Ok(v)
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/content/list", get(api_list))
        .route("/api/tools/editor/content/sprite.png", get(api_sprite_png))
        .route("/api/tools/editor/content/sprite/info", get(api_sprite_info))
        .route("/api/tools/editor/content/sound/info", get(api_sound_info))
}

#[cfg(test)]
mod tests {
    #[test]
    fn companion_never_panics_on_non_ascii_names() {
        let names: HashSet<String> = ["zombie.mdl".to_string()].into_iter().collect();
        for n in ["é.mdl", "zé1.mdl", "日本.mdl", "日本01.mdl", "zombieT.mdl", "zombie01.mdl", "x", ".mdl", "é"] {
            let _ = companion(n, &names);
        }
        assert!(companion("zombiet.mdl", &names));
        assert!(companion("zombie01.mdl", &names));
        assert!(!companion("日本.mdl", &names));
    }
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-browser-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(p: &Path, b: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b).unwrap();
    }

    #[test]
    fn lists_by_kind_with_sources() {
        let root = tmp("list");
        let hl = root.join("Half-Life");
        let app = root.join("app");
        put(&hl.join("valve/models/barney.mdl"), b"IDST");
        put(&hl.join("valve/models/barneyT.mdl"), b"IDST");
        put(&hl.join("cstrike/models/custom/prop.mdl"), b"IDST");
        put(&hl.join("cstrike/models/barney.mdl"), b"IDST"); // the game's own folder first
        put(&hl.join("cstrike_downloads/sprites/fx.spr"), &spr::tests::sample(1, 2, false));
        put(&hl.join("cstrike/sound/ambience/x.wav"), b"RIFF");
        put(&hl.join("cstrike/models/bad name;rm.mdl"), b"IDST");
        // a GameBanana download owns models/custom/prop.mdl
        let rec = json!({"Mod:42:aaaa": {"game": hl.join("cstrike").to_string_lossy(), "name": "Crates", "url": "https://gamebanana.com/mods/42", "author": "Someone", "files": [{"path": "models/custom/prop.mdl"}]}});
        put(&app.join("content/record.json"), rec.to_string().as_bytes());
        let proj = root.join("proj");
        put(&proj.join("models/mine.mdl"), b"IDST");
        // (T) a community map installed models/community/tree.mdl
        put(&hl.join("cstrike/models/community/tree.mdl"), b"IDST");
        put(&app.join("installed_maps.json"), json!({"101": {"files": ["maps/de_lonely.bsp", "models/community/tree.mdl"]}}).to_string().as_bytes());
        put(&app.join("community/mod-101/meta.json"), json!({"kind": "map", "model": "Mod", "gb": 101, "name": "De Lonely", "author": "jam"}).to_string().as_bytes());
        let v = list(&hl, "cstrike", &app, Some(&proj), Kind::Model);
        let items = v["items"].as_array().unwrap();
        let by = |p: &str| items.iter().find(|i| i["path"] == p).cloned();
        assert_eq!(by("models/barney.mdl").unwrap()["folder"], "cstrike", "first folder wins");
        assert!(by("models/barneyT.mdl").is_none(), "texture companions aren't listed");
        assert_eq!(by("models/custom/prop.mdl").unwrap()["source"], "downloaded");
        assert_eq!(by("models/custom/prop.mdl").unwrap()["page"]["url"], "https://gamebanana.com/mods/42");
        assert_eq!(by("models/mine.mdl").unwrap()["source"], "mine");
        assert_eq!(by("models/community/tree.mdl").unwrap()["source"], "community", "(T) a community creation's file");
        assert_eq!(by("models/community/tree.mdl").unwrap()["page"]["author"], "jam");
        assert!(by("models/bad name;rm.mdl").is_none());
        assert_eq!(v["downloads"][0]["count"], 1);
        let s = list(&hl, "cstrike", &app, None, Kind::Sprite);
        assert_eq!(s["items"][0]["path"], "sprites/fx.spr");
        assert_eq!(list(&hl, "cstrike", &app, None, Kind::Sound)["items"][0]["path"], "sound/ambience/x.wav");
        // resolving only clean paths in a named folder
        assert!(resolve(&hl, "cstrike", None, "cstrike_downloads", "sprites/fx.spr").is_some());
        assert!(resolve(&hl, "cstrike", None, "cstrike_downloads", "../cstrike/sprites/fx.spr").is_none());
        assert!(resolve(&hl, "cstrike", None, "elsewhere", "sprites/fx.spr").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn describes_sounds() {
        let mut w = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&22050u32.to_le_bytes());
        w.extend_from_slice(&44100u32.to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&44100u32.to_le_bytes());
        let v = sound_info(&w, false);
        assert_eq!((v["rate"].as_u64(), v["bits"].as_u64(), v["seconds"].as_f64()), (Some(22050), Some(16), Some(1.0)));
        assert!(v["warn"].is_null());
        let mut st = w.clone();
        st[22] = 2; // stereo
        assert!(sound_info(&st, false)["warn"].as_str().unwrap().contains("stereo"));
        assert!(sound_info(b"nope", false)["warn"].is_string());
        assert!(sound_info(b"", true)["note"].is_string());
    }
}
