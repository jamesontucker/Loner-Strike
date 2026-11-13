//! The prefab library's routes (`/api/tools/editor/prefabs*`): the starter prefabs that come
//! with the app and the player's own in `<app dir>/editor/prefabs/` (`<id>.map`, `<id>.json`,
//! `<id>.png`), saving a selection, renaming, deleting, a package to share (zip) and
//! installing one (from a file, or from the community browser's Try it: `install_dir`).
//! Placing goes through the editor's `cmd` (`place_prefab`) so it's one undo step.

use super::{blocking, forbidden, Ed, NO_DOC};
use crate::editor::doc::MapDocument;
use crate::editor::history;
use crate::editor::prefab::{self, PrefabMeta};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_MINE: usize = 2000;
const BUILTIN: &str = "builtin:";
const MINE: &str = "mine:";
const THUMB_W: u32 = 192;
const THUMB_H: u32 = 144;

pub fn lib_dir(editor_dir: &Path) -> PathBuf {
    editor_dir.join("prefabs")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn card(id: &str, m: &PrefabMeta, builtin: bool) -> Value {
    json!({
        "id": id, "uid": m.uid, "name": m.name, "tags": m.tags, "author": m.author, "description": m.description, "builtin": builtin,
        "brushes": m.brushes, "entities": m.entities, "size": m.size, "textures": m.textures, "wads": m.wads, "created": m.created,
        "thumb": format!("/api/tools/editor/prefab/{}/thumb.png", id.replace(':', "~")),
    })
}

/// The player's own prefabs: (id without the "mine:" prefix, details), newest first.
/// P5: one without a valid uid (made before uids, or edited by hand), or with one another
/// prefab already has, gets a new one, written into its .json.
pub fn mine(dir: &Path) -> Vec<(String, PrefabMeta)> {
    let mut out: Vec<(String, PrefabMeta)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = prefab::builtins().into_iter().map(|(_, m, _)| m.uid).collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    // (the oldest file keeps a shared uid: sorted by name, then the rest)
    files.sort();
    for p in files {
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()).map(String::from) else { continue };
        if p.extension().and_then(|x| x.to_str()) != Some("json") || !prefab::valid_id(&stem) || !dir.join(format!("{stem}.map")).is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(mut m) = serde_json::from_str::<PrefabMeta>(&text) else { continue };
        if !crate::editor::doc::valid_uid(&m.uid) || seen.contains(&m.uid) {
            m.uid = crate::editor::doc::new_uid();
            if let Ok(t) = serde_json::to_string_pretty(&m) {
                let _ = write_file(&p, t.as_bytes());
            }
        }
        seen.insert(m.uid.clone());
        out.push((stem, m));
        if out.len() >= MAX_MINE {
            break;
        }
    }
    out.sort_by(|a, b| b.1.created.cmp(&a.1.created).then(a.1.name.cmp(&b.1.name)));
    out
}

pub fn list(dir: &Path) -> Value {
    let mut items: Vec<Value> = mine(dir).iter().map(|(id, m)| card(&format!("{MINE}{id}"), m, false)).collect();
    items.extend(prefab::builtins().iter().map(|(id, m, _)| card(&format!("{BUILTIN}{id}"), m, true)));
    json!({"prefabs": items, "folder": dir.to_string_lossy(), "pending": pending(dir)})
}

/// A prefab's pieces and details by id ("builtin:stairs", "mine:my_room").
pub fn load(dir: &Path, id: &str) -> Result<(MapDocument, PrefabMeta), String> {
    // P5: "uid:<uuid>": by the prefab's permanent id (a placed group's prefab, a package)
    if let Some(u) = id.strip_prefix("uid:") {
        return load(dir, &id_of_uid(dir, u).ok_or("That prefab isn't in your library any more.")?);
    }
    if let Some(b) = id.strip_prefix(BUILTIN) {
        return prefab::builtins().into_iter().find(|(i, _, _)| *i == b).map(|(_, m, f)| (f, m)).ok_or_else(|| "That prefab isn't there any more.".to_string());
    }
    let own = id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("That prefab isn't there any more.")?;
    let map = std::fs::read_to_string(dir.join(format!("{own}.map"))).map_err(|_| "That prefab isn't there any more.".to_string())?;
    let meta = std::fs::read_to_string(dir.join(format!("{own}.json"))).ok();
    prefab::from_files(&map, meta.as_deref(), own)
}

/// P5: the library id ("mine:…" or "builtin:…") of the prefab with this uid.
pub fn id_of_uid(dir: &Path, uid: &str) -> Option<String> {
    let u = uid.trim().to_ascii_lowercase();
    if !crate::editor::doc::valid_uid(&u) {
        return None;
    }
    mine(dir).into_iter().find(|(_, m)| m.uid == u).map(|(id, _)| format!("{MINE}{id}")).or_else(|| prefab::builtins().into_iter().find(|(_, m, _)| m.uid == u).map(|(id, _, _)| format!("{BUILTIN}{id}")))
}

/// A free id for a name (`crate`, `crate_2`, ...).
fn free_id(dir: &Path, name: &str) -> String {
    let base = prefab::slug(name);
    let taken = |id: &str| dir.join(format!("{id}.map")).exists() || dir.join(format!("{id}.json")).exists();
    if !taken(&base) {
        return base;
    }
    (2..10_000).map(|n| format!("{base}_{n}")).find(|id| !taken(id)).unwrap_or_else(|| format!("{base}_{}", now_secs()))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't save the prefab ({e}).")
    })
}

/// Saves pieces as a NEW prefab in the player's library; its id ("mine:..."). P5: a uid
/// that's missing, not a v4 UUID, a starter prefab's or one already in the library gets a new
/// one: an existing prefab is only ever changed by `save_over` (an explicit replace or update).
pub fn save(dir: &Path, frag: &MapDocument, meta: &PrefabMeta, thumb: Option<&[u8]>) -> Result<String, String> {
    if mine(dir).len() >= MAX_MINE {
        return Err("Your prefab library is full (2000). Delete some first.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make the prefab folder ({e})."))?;
    let mut meta = meta.clone();
    let taken = prefab::builtins().into_iter().any(|(_, m, _)| m.uid == meta.uid) || mine(dir).iter().any(|(_, m)| m.uid == meta.uid);
    if !crate::editor::doc::valid_uid(&meta.uid) || taken {
        meta.uid = crate::editor::doc::new_uid();
    }
    let id = free_id(dir, &meta.name);
    write_prefab(dir, &id, frag, &meta, thumb)?;
    Ok(format!("{MINE}{id}"))
}

/// P5: replaces one of the player's prefabs (the Save dialog's "Replace it", or an update the
/// player confirmed): same id and uid, the new pieces and details.
pub fn save_over(dir: &Path, own: &str, frag: &MapDocument, meta: &PrefabMeta, thumb: Option<&[u8]>) -> Result<String, String> {
    let (_, old) = mine(dir).into_iter().find(|(i, _)| i == own).ok_or("The prefab to replace isn't there any more.")?;
    let mut meta = meta.clone();
    meta.uid = old.uid;
    if meta.origin.is_none() {
        meta.origin = old.origin;
    }
    meta.exports = old.exports;
    if thumb.is_none() {
        let _ = std::fs::remove_file(dir.join(format!("{own}.png")));
    }
    write_prefab(dir, own, frag, &meta, thumb)?;
    Ok(format!("{MINE}{own}"))
}

fn write_prefab(dir: &Path, id: &str, frag: &MapDocument, meta: &PrefabMeta, thumb: Option<&[u8]>) -> Result<(), String> {
    let (map, js) = prefab::to_files(frag, meta)?;
    if let Some(png) = thumb {
        prefab::check_png(png)?;
        write_file(&dir.join(format!("{id}.png")), png)?;
    }
    write_file(&dir.join(format!("{id}.map")), map.as_bytes())?;
    write_file(&dir.join(format!("{id}.json")), js.as_bytes())?;
    Ok(())
}

// ---- updates waiting for the player's yes (P5) ----------------------------------------------

/// Packages that would update a prefab (same uid, same source) wait here until the player says.
fn pending_dir(lib: &Path) -> PathBuf {
    lib.join(".pending")
}

/// What an update changes, in words.
fn changes(old: &PrefabMeta, new: &PrefabMeta) -> Vec<String> {
    let mut c = Vec::new();
    if old.name != new.name {
        c.push(format!("name “{}” → “{}”", old.name, new.name));
    }
    if (old.brushes, old.entities) != (new.brushes, new.entities) {
        c.push(format!("{} brushes, {} entities → {} brushes, {} entities", old.brushes, old.entities, new.brushes, new.entities));
    }
    if old.size != new.size {
        c.push(format!("size {} → {}", old.size.map(|x| x.to_string()).join(" × "), new.size.map(|x| x.to_string()).join(" × ")));
    }
    let added: Vec<&String> = new.textures.iter().filter(|t| !old.textures.contains(t)).collect();
    let gone: Vec<&String> = old.textures.iter().filter(|t| !new.textures.contains(t)).collect();
    if !added.is_empty() || !gone.is_empty() {
        c.push(format!("textures{}{}", if added.is_empty() { String::new() } else { format!(" + {}", added.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) }, if gone.is_empty() { String::new() } else { format!(" − {}", gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) }));
    }
    if old.description != new.description {
        c.push("description".into());
    }
    if c.is_empty() {
        c.push("nothing you can see (the same pieces)".into());
    }
    c
}

/// The prefab a waiting update may still change: the library id it was staged for, still
/// holding that uid, and still from the same source the update came from. Otherwise the
/// update is dropped (the prefab was deleted, recreated, or replaced by one from elsewhere).
fn pending_target(lib: &Path, uid: &str) -> Result<(String, PrefabMeta), String> {
    let d = pending_dir(lib).join(uid);
    let t: Value = std::fs::read_to_string(d.join("target.json")).ok().and_then(|x| serde_json::from_str(&x).ok()).ok_or("That update can't be checked.")?;
    let id = t["id"].as_str().filter(|x| prefab::valid_id(x)).ok_or("That update can't be checked.")?;
    let origin: prefab::Origin = serde_json::from_value(t["origin"].clone()).map_err(|_| "That update can't be checked.".to_string())?;
    let (own, old) = mine(lib).into_iter().find(|(_, m)| m.uid == uid).ok_or("The prefab it updates isn't in your library any more.")?;
    if own != id || !prefab::same_source(&old, &origin) {
        return Err(format!("“{}” isn't the prefab this update was for any more, so it wasn't changed.", old.name));
    }
    Ok((own, old))
}

/// The updates waiting: [{uid, id, name, changes}]. Ones whose prefab no longer matches are dropped.
pub fn pending(lib: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(pending_dir(lib)).into_iter().flatten().flatten().take(200) {
        let uid = e.file_name().to_string_lossy().to_string();
        let d = e.path();
        if !crate::editor::doc::valid_uid(&uid) {
            let _ = std::fs::remove_dir_all(&d);
            continue;
        }
        let Ok((id, old)) = pending_target(lib, &uid) else {
            let _ = std::fs::remove_dir_all(&d);
            continue;
        };
        let (Ok(map), Ok(js)) = (std::fs::read_to_string(d.join("prefab.map")), std::fs::read_to_string(d.join("prefab.json"))) else { continue };
        let Ok((_, new)) = prefab::from_files(&map, Some(&js), "prefab") else { continue };
        out.push(json!({"uid": uid, "id": format!("{MINE}{id}"), "name": old.name, "changes": changes(&old, &new)}));
    }
    out
}

/// The player said yes (apply) or no to a waiting update.
pub fn settle(lib: &Path, uid: &str, apply: bool) -> Result<Value, String> {
    if !crate::editor::doc::valid_uid(uid) {
        return Err("That update isn't there any more.".into());
    }
    let d = pending_dir(lib).join(uid);
    if !d.is_dir() {
        return Err("That update isn't there any more.".into());
    }
    let r = if apply {
        (|| {
            let (own, old) = pending_target(lib, uid)?;
            let map = std::fs::read_to_string(d.join("prefab.map")).map_err(|_| "That update couldn't be read.".to_string())?;
            let js = std::fs::read_to_string(d.join("prefab.json")).ok();
            let (frag, mut meta) = prefab::from_files(&map, js.as_deref(), &own)?;
            meta.origin = old.origin.clone();
            let png = std::fs::read(d.join("prefab.png")).ok().filter(|b| prefab::check_png(b).is_ok());
            save_over(lib, &own, &frag, &meta, png.as_deref()).map(|id| json!({"ok": true, "updated": id, "name": meta.name}))
        })()
    } else {
        Ok(json!({"ok": true, "kept": true}))
    };
    let _ = std::fs::remove_dir_all(&d);
    r
}

/// Deletes one of the player's prefabs, and any update waiting for it.
pub fn delete(lib: &Path, own: &str) -> Result<(), String> {
    if !prefab::valid_id(own) {
        return Err("That prefab isn't there any more.".into());
    }
    let uid = mine(lib).into_iter().find(|(i, _)| i == own).map(|(_, m)| m.uid);
    let mut any = false;
    for ext in ["map", "json", "png"] {
        any |= std::fs::remove_file(lib.join(format!("{own}.{ext}"))).is_ok();
    }
    if let Some(u) = uid.filter(|u| crate::editor::doc::valid_uid(u)) {
        let _ = std::fs::remove_dir_all(pending_dir(lib).join(u));
    }
    if any {
        Ok(())
    } else {
        Err("That prefab isn't there any more.".into())
    }
}

/// What a package (an unpacked folder) brought: prefab names and WAD files beside them.
#[derive(Debug, Default)]
pub struct Installed {
    pub prefabs: Vec<String>,
    pub ids: Vec<String>,
    pub wads: Vec<PathBuf>,
    pub skipped: Vec<String>,
    /// P5: updates of prefabs you have from the same source, waiting for a yes (`pending`)
    pub waiting: Vec<String>,
    /// P5: prefabs with the uid of one you have but from somewhere else: added as copies
    pub copies: Vec<String>,
}

/// Installs every prefab in an unpacked package or download: each `.map` (with the `.json`
/// and `.png` of the same name beside it, when there are) becomes one of the player's
/// prefabs. WAD files are listed for the caller to put where the game looks.
pub fn install_dir(lib: &Path, src: &Path, author_hint: &str, origin: &prefab::Origin) -> Result<Installed, String> {
    let mut got = Installed::default();
    let files = crate::archive::walk(src);
    for p in &files {
        let low = p.to_string_lossy().to_lowercase();
        if low.ends_with(".wad") {
            got.wads.push(p.clone());
            continue;
        }
        if !low.ends_with(".map") {
            continue;
        }
        if got.prefabs.len() >= 200 {
            break;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("prefab").to_string();
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
        if size > prefab::MAX_PREFAB_MAP as u64 {
            got.skipped.push(format!("{stem}: too big"));
            continue;
        }
        let Ok(bytes) = std::fs::read(p) else { continue };
        let text: String = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => e.into_bytes().iter().map(|b| *b as char).collect(),
        };
        let js = crate::caps::read_text(&p.with_extension("json"), "prefab info", 1 << 20).ok(); // [Caps]
        match prefab::from_files(&text, js.as_deref(), &stem) {
            Ok((frag, mut meta)) => {
                if meta.author.is_empty() {
                    meta.author = prefab::clean_text(author_hint, 48);
                }
                if meta.created == 0 {
                    meta.created = now_secs();
                }
                let png = crate::caps::read_file(&p.with_extension("png"), "picture", 1 << 20).ok().filter(|b| prefab::check_png(b).is_ok()); // [Caps]
                // P5: identity isn't permission. A prefab you have with this uid is only
                // updated when the package is from the same source, and only after you say yes
                // (it waits in .pending); from anywhere else it's added as a copy (a new uid).
                meta.origin = Some(origin.clone());
                if let Some((own, old)) = mine(lib).into_iter().find(|(_, m)| !meta.uid.is_empty() && m.uid == meta.uid) {
                    if prefab::same_source(&old, origin) {
                        let d = pending_dir(lib).join(&meta.uid);
                        let _ = std::fs::remove_dir_all(&d);
                        std::fs::create_dir_all(&d).map_err(|e| format!("Couldn't keep the update ({e})."))?;
                        let (map, js) = prefab::to_files(&frag, &meta)?;
                        write_file(&d.join("prefab.map"), map.as_bytes())?;
                        write_file(&d.join("prefab.json"), js.as_bytes())?;
                        if let Some(png) = &png {
                            write_file(&d.join("prefab.png"), png)?;
                        }
                        // what it may update: this prefab, from this source (checked again on yes)
                        let target = json!({"id": own, "origin": origin});
                        write_file(&d.join("target.json"), target.to_string().as_bytes())?;
                        got.waiting.push(old.name.clone());
                        got.ids.push(format!("{MINE}{own}"));
                        continue;
                    }
                    meta.uid = crate::editor::doc::new_uid();
                    got.copies.push(format!("“{}” was added as a copy: it has the same id as your “{}” but comes from somewhere else, so yours wasn't changed.", meta.name, old.name));
                }
                let id = save(lib, &frag, &meta, png.as_deref())?;
                got.prefabs.push(meta.name.clone());
                got.ids.push(id);
            }
            Err(e) => got.skipped.push(format!("{stem}: {e}")),
        }
    }
    if got.prefabs.is_empty() && got.waiting.is_empty() {
        let why = got.skipped.first().cloned().map(|s| format!(" ({s})")).unwrap_or_default();
        return Err(format!("There's no prefab (.map) in it the editor can use{why}."));
    }
    Ok(got)
}

/// Copies a package's WADs into the game folder, never over a file that's there.
pub fn install_wads(wads: &[PathBuf], game_dir: &Path) -> Vec<String> {
    let mut added = Vec::new();
    for w in wads {
        let Some(name) = w.file_name().and_then(|n| n.to_str()) else { continue };
        if !crate::content::safe_part(name) {
            continue;
        }
        let dest = game_dir.join(name);
        if dest.exists() {
            continue;
        }
        // [Caps] size checked before reading; only the header is needed
        let ok = std::fs::metadata(w).is_ok_and(|m| m.len() <= crate::goldsrc::MAX_WAD) && std::fs::File::open(w).and_then(|f| { use std::io::Read; let mut h = [0u8; 4]; f.take(4).read_exact(&mut h).map(|_| h) }).is_ok_and(|h| &h == b"WAD3");
        if ok && std::fs::copy(w, &dest).is_ok() {
            added.push(name.to_string());
        }
    }
    added
}

/// The credits of a prefab's assets, from the game's records (T's credits.rs): the models,
/// sprites and sounds its entities name and the WADs it uses. None without a game.
pub fn prefab_credits(hl: &Path, game: &str, frag: &MapDocument, meta: &PrefabMeta) -> Vec<crate::tools::credits::AssetCredit> {
    let text = crate::editor::mapfile::write_map(frag, &Default::default()).unwrap_or_default();
    crate::tools::credits::credits_for_entities(hl, game, &text, &meta.wads)
}

/// A package to share: `prefabs/<id>.map|json|png`, README.txt and loner-strike.json.
/// `credits`: who made the assets it uses (`prefab_credits`), written into the prefab's .json
/// and the package's item (P5; T's list service checks them), and listed in README.txt.
pub fn package(dir: &Path, id: &str, out: &Path, credits: Option<Vec<crate::tools::credits::AssetCredit>>) -> Result<(), String> {
    let (frag, mut meta) = load(dir, id)?;
    if let Some(c) = credits {
        meta.credits = c;
    }
    // P5: where it came from stays here (load already leaves it out); record this export, so
    // importing this file again is the same source (it can update after asking)
    meta.origin = None;
    meta.exports.clear();
    if let Some(own) = id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)) {
        let path = dir.join(format!("{own}.json"));
        if let Some(mut m) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<PrefabMeta>(&t).ok()) {
            let out_dir = out.parent().and_then(|p| std::fs::canonicalize(p).ok());
            if let Some(full) = out_dir.zip(out.file_name()).map(|(d, n)| d.join(n).to_string_lossy().to_string()) {
                m.exports.retain(|e| *e != full);
                m.exports.insert(0, full);
                m.exports.truncate(5);
                if let Ok(t) = serde_json::to_string_pretty(&m) {
                    let _ = write_file(&path, t.as_bytes());
                }
            }
        }
    }
    let slug = id.split(':').nth(1).unwrap_or("prefab");
    let (map, js) = prefab::to_files(&frag, &meta)?;
    let png = match id.strip_prefix(MINE).map(|x| dir.join(format!("{x}.png"))).and_then(|p| std::fs::read(p).ok()) {
        Some(p) => p,
        None => prefab::thumbnail(&frag, THUMB_W, THUMB_H),
    };
    let readme = format!(
        "{}\r\n\r\nA map editor prefab made with Loner-Strike ({} brushes, {} entities, {} x {} x {} units).\r\n\
         Textures: {}\r\nWADs: {}\r\n\r\nLoner-Strike: Tools > Map editor > Prefabs > Add from a file, or Try it in Tools > Community.\r\n\
         Other editors: prefabs/{slug}.map is a plain Valve 220 .map (copy and paste it into J.A.C.K. or TrenchBroom).\r\n",
        meta.name, meta.brushes, meta.entities, meta.size[0], meta.size[1], meta.size[2], meta.textures.join(", "), meta.wads.join(", ")
    );
    // (who made what it uses)
    let mut readme = readme;
    if !meta.credits.is_empty() {
        readme.push_str("\r\nCredits:\r\n");
        for c in &meta.credits {
            let who: Vec<String> = c.authors.iter().map(|a| if a.role.is_empty() { a.name.clone() } else { format!("{} ({})", a.name, a.role) }).collect();
            readme.push_str(&format!("- {}{}: {}{}\r\n", if c.name.is_empty() { c.files.join(", ") } else { c.name.clone() }, if c.page.is_empty() { String::new() } else { format!(" <{}>", c.page) }, who.join(", "), if c.permission == "granted" { format!(" (used with permission: {})", c.note) } else { String::new() }));
        }
    }
    let manifest = json!({"schema": 1, "app": env!("CARGO_PKG_VERSION"), "item": {"kind": "prefab", "name": meta.name, "author": meta.author, "summary": meta.description, "tags": meta.tags, "credits": meta.credits,
        "prefab": {"uid": meta.uid, "files": [format!("prefabs/{slug}.map")], "brushes": meta.brushes, "entities": meta.entities, "size": meta.size, "wads": meta.wads}}});
    let part = out.with_extension("part");
    let write = || -> Result<(), String> {
        let f = std::fs::File::create(&part).map_err(|e| format!("Couldn't write the package ({e})."))?;
        let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in [
            (format!("prefabs/{slug}.map"), map.into_bytes()),
            (format!("prefabs/{slug}.json"), js.into_bytes()),
            (format!("prefabs/{slug}.png"), png),
            ("README.txt".into(), readme.into_bytes()),
            ("loner-strike.json".into(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()),
        ] {
            z.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(&body).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, out).map_err(|e| format!("Couldn't save the package ({e})."))
}

/// Installs a file the player picked: a package (zip/rar/7z) or a single .map.
fn install_file(lib: &Path, work: &Path, file: &Path, game_dir: Option<&Path>) -> Result<Value, String> {
    let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = crate::sys::make_temp_dir(work, "prefab-").map_err(|e| e.to_string())?;
    let res = (|| {
        let src = tmp.join("x");
        std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
        if crate::archive::is_archive(&name) {
            let copy = tmp.join(format!("in{}", crate::archive::ext_of(&name)));
            std::fs::copy(file, &copy).map_err(|e| format!("Couldn't read the file ({e})."))?;
            crate::archive::extract(&copy, &src, 512 << 20)?;
        } else if name.to_lowercase().ends_with(".map") {
            std::fs::copy(file, src.join(&name)).map_err(|e| format!("Couldn't read the file ({e})."))?;
            for side in ["json", "png"] {
                let _ = std::fs::copy(file.with_extension(side), src.join(Path::new(&name).with_extension(side)));
            }
        } else {
            return Err("Pick a prefab package (.zip) or a .map file.".into());
        }
        let got = install_dir(lib, &src, "", &prefab::Origin::file(file))?;
        let wads = game_dir.map(|g| install_wads(&got.wads, g)).unwrap_or_default();
        Ok(json!({"ok": true, "prefabs": got.prefabs, "ids": got.ids, "wads": wads, "skipped": got.skipped, "waiting": got.waiting, "copies": got.copies, "pending": pending(lib)}))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

// ---- routes ------------------------------------------------------------------------------

async fn api_list(Extension(ed): Extension<Ed>) -> Response {
    blocking(move || Ok(list(&lib_dir(&ed.dir)))).await
}

async fn api_thumb(Extension(ed): Extension<Ed>, UrlPath(id): UrlPath<String>) -> Response {
    let id = id.replacen('~', ":", 1);
    let r = tokio::task::spawn_blocking(move || {
        let dir = lib_dir(&ed.dir);
        if let Some(own) = id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)) {
            if let Ok(b) = std::fs::read(dir.join(format!("{own}.png"))) {
                return Some(b);
            }
        }
        load(&dir, &id).ok().map(|(f, _)| prefab::thumbnail(&f, THUMB_W, THUMB_H))
    })
    .await
    .ok()
    .flatten();
    match r {
        Some(b) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-cache")], b).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[derive(Deserialize)]
struct SaveReq {
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    brushes: Vec<u64>,
    #[serde(default)]
    entities: Vec<u64>,
    /// a PNG of the view (base64, a data: URL is fine)
    thumb: Option<String>,
    /// P5: the id of one of your prefabs this replaces (it keeps its uid; "Replace it" in Save)
    #[serde(default)]
    replace: Option<String>,
}

async fn api_save(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let name = prefab::clean_text(&req.name, 64);
        if name.is_empty() {
            return Err("Give the prefab a name.".into());
        }
        if req.brushes.is_empty() && req.entities.is_empty() {
            return Err("Pick what goes in the prefab first.".into());
        }
        let thumb = match req.thumb.as_deref().map(|t| t.rsplit(',').next().unwrap_or(t).trim()) {
            Some(t) if !t.is_empty() => {
                if t.len() > 1_500_000 {
                    return Err("The thumbnail is too big.".into());
                }
                Some(base64::engine::general_purpose::STANDARD.decode(t).map_err(|_| "The thumbnail couldn't be read.".to_string())?)
            }
            _ => None,
        };
        let mut frag = {
            let g = ed.lock();
            let s = g.as_ref().ok_or(NO_DOC)?;
            let f = history::fragment(&s.doc, &super::bids(&req.brushes), &super::eids(&req.entities));
            let mut f = f;
            f.world.wads = s.doc.world.wads.clone();
            f
        };
        if frag.brushes.is_empty() && frag.entities.is_empty() {
            return Err("Those pieces aren't in the map any more.".into());
        }
        prefab::centre(&mut frag)?;
        let mut meta = prefab::describe(&frag, &name, &req.tags, &req.author, now_secs());
        // only the WADs the prefab's textures come from, when that's known
        if frag.materials.values().all(|m| m.wad.is_some()) && !frag.materials.is_empty() {
            let used: Vec<String> = frag.materials.values().filter_map(|m| m.wad.as_deref()).map(|w| w.rsplit(['\\', '/']).next().unwrap_or(w).to_ascii_lowercase()).collect();
            meta.wads.retain(|w| used.contains(w));
        }
        meta.description = prefab::clean_text(&req.description, 400);
        let dir = lib_dir(&ed.dir);
        meta.origin = Some(prefab::Origin::local());
        // Replace it (asked in the Save dialog): the same prefab, new pieces
        let id = match &req.replace {
            Some(r) => save_over(&dir, r.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("Only your own prefabs can be replaced.")?, &frag, &meta, thumb.as_deref())?,
            None => save(&dir, &frag, &meta, thumb.as_deref())?,
        };
        Ok(json!({"ok": true, "id": id, "name": meta.name}))
    })
    .await
}

#[derive(Deserialize)]
struct EditReq {
    id: String,
    name: Option<String>,
    tags: Option<Vec<String>>,
    description: Option<String>,
}

async fn api_edit(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<EditReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let own = req.id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("Only your own prefabs can be changed. Save a copy first.")?.to_string();
        let path = dir.join(format!("{own}.json"));
        let mut m: PrefabMeta = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("That prefab isn't there any more.")?;
        if let Some(n) = req.name {
            let n = prefab::clean_text(&n, 64);
            if n.is_empty() {
                return Err("Give the prefab a name.".into());
            }
            m.name = n;
        }
        if let Some(t) = req.tags {
            m.tags = prefab::clean_tags(&t);
        }
        if let Some(d) = req.description {
            m.description = prefab::clean_text(&d, 400);
        }
        write_file(&path, serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?.as_bytes())?;
        Ok(json!({"ok": true}))
    })
    .await
}

#[derive(Deserialize)]
struct UpdateReq {
    uid: String,
    /// true: update the prefab; false: keep yours and drop the update
    apply: bool,
}

/// P5: the player's answer to a waiting update (Library: "Update <name>?").
async fn api_update(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<UpdateReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || settle(&lib_dir(&ed.dir), &req.uid, req.apply)).await
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn api_delete(Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<IdReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let own = req.id.strip_prefix(MINE).filter(|x| prefab::valid_id(x)).ok_or("The starter prefabs can't be deleted.")?;
        delete(&dir, own)?; // P5: and its waiting update
        Ok(json!({"ok": true}))
    })
    .await
}

async fn api_export(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap, Json(req): Json<IdReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let dir = lib_dir(&ed.dir);
        let (frag, meta) = load(&dir, &req.id)?;
        let slug = req.id.split(':').nth(1).unwrap_or("prefab").to_string();
        let Some(out) = super::dialog(&st, super::Pick::ExportPrefab, &format!("{slug}-prefab"))? else { return Ok(json!({"cancelled": true})) };
        // P5: the credits of what it uses (needs the game; without one, what the prefab had)
        let credits = st.local_game().map(|g| prefab_credits(&g.hl, if g.is_czero() { "czero" } else { "cstrike" }, &frag, &meta));
        package(&dir, &req.id, &out, credits)?;
        Ok(json!({"ok": true, "file": out.file_name().map(|n| n.to_string_lossy().to_string()), "name": meta.name}))
    })
    .await
}

async fn api_import(State(st): State<Shared>, Extension(ed): Extension<Ed>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let game = st.local_game().map(|g| g.dir.clone());
    blocking(move || {
        let Some(file) = super::dialog(&st, super::Pick::ImportPrefab, "")? else { return Ok(json!({"cancelled": true})) };
        install_file(&lib_dir(&ed.dir), &ed.dir.join("work"), &file, game.as_deref())
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/editor/prefabs", get(api_list))
        .route("/api/tools/editor/prefab/:id/thumb.png", get(api_thumb))
        .route("/api/tools/editor/prefabs/save", post(api_save))
        .route("/api/tools/editor/prefabs/edit", post(api_edit))
        .route("/api/tools/editor/prefabs/delete", post(api_delete))
        .route("/api/tools/editor/prefabs/update", post(api_update)) // P5
        .route("/api/tools/editor/prefabs/export", post(api_export))
        .route("/api/tools/editor/prefabs/import", post(api_import))
}

/// For the community browser's Try it (tools/community.rs): installs the prefabs in an
/// unpacked download into the player's library, and its WADs into the game folder.
/// A community download. P5: `origin` is the creation's page and its uploader, so a later
/// version can update the prefabs it installed (after the player says yes in the library).
pub fn install_download(src: &Path, author: &str, game_dir: Option<&Path>, origin: &prefab::Origin) -> Result<Value, String> {
    let lib = lib_dir(&crate::sys::app_dir().join("editor"));
    let got = install_dir(&lib, src, author, origin)?;
    let wads = game_dir.map(|g| install_wads(&got.wads, g)).unwrap_or_default();
    Ok(json!({"prefabs": got.prefabs, "ids": got.ids, "wads": wads, "skipped": got.skipped, "waiting": got.waiting, "copies": got.copies}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-prefabs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A prefab whose entity uses a downloaded model: its package carries that model's credits
    /// (in the prefab's .json, the loner-strike.json item and README.txt), and installing it
    /// keeps them.
    #[test]
    fn package_carries_the_credits_of_what_it_uses() {
        use crate::editor::doc::Entity;
        use crate::tools::credits::{credits_for_entities_with, Ctx};
        let root = tmp("credits");
        let lib = lib_dir(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/models/props")).unwrap();
        std::fs::write(hl.join("cstrike/models/props/crate_x.mdl"), b"IDST\x0a\0\0\0crate").unwrap();
        // the prefab: a box and a cycler_sprite with the downloaded model
        let (mut frag, meta) = load(&lib, "builtin:crate_stack").unwrap();
        let mut e = Entity::new("cycler_sprite");
        e.keys.push(("model".into(), "models/props/crate_x.mdl".into()));
        e.keys.push(("origin".into(), "0 0 0".into()));
        frag.insert_entity(e);
        let id = save(&lib, &frag, &meta, None).unwrap();
        // Get more's record of the download, with the page's credits
        let mut ctx = Ctx::default();
        ctx.add_record(&json!({
            "url": "https://gamebanana.com/mods/4242", "name": "Wooden Crates", "author": "Uploader",
            "files": [{"path": "models/props/crate_x.mdl"}],
            "rights": {"page": "https://gamebanana.com/mods/4242", "model": "Mod", "id": 4242, "name": "Wooden Crates",
                "submitter": {"id": 7, "name": "Uploader"},
                "credits": [{"group": "Original Authors", "authors": [{"name": "CrateMaker", "role": "Modeler", "url": ""}]}],
                "checklist": {"install": "yes", "parts_gb": "yes", "parts_other": "yes"}, "licence": "", "ported": false}
        }));
        let (f2, m2) = load(&lib, &id).unwrap();
        let text = crate::editor::mapfile::write_map(&f2, &Default::default()).unwrap();
        let credits = credits_for_entities_with(&ctx, &hl, "cstrike", &text, &m2.wads);
        assert!(credits.iter().any(|c| c.files.iter().any(|f| f == "models/props/crate_x.mdl") && c.authors.iter().any(|a| a.name == "CrateMaker")), "{credits:?}");
        let zip = root.join("crate.zip");
        package(&lib, &id, &zip, Some(credits.clone())).unwrap();
        let un = root.join("un");
        std::fs::create_dir_all(&un).unwrap();
        crate::archive::extract(&zip, &un, 64 << 20).unwrap();
        let slug = id.strip_prefix(MINE).unwrap();
        let pj: Value = serde_json::from_slice(&std::fs::read(un.join(format!("prefabs/{slug}.json"))).unwrap()).unwrap();
        assert_eq!(pj["credits"][0]["page"], "https://gamebanana.com/mods/4242");
        let mf: Value = serde_json::from_slice(&std::fs::read(un.join("loner-strike.json")).unwrap()).unwrap();
        assert_eq!(mf["item"]["credits"][0]["authors"][0]["name"], "CrateMaker");
        let readme = std::fs::read_to_string(un.join("README.txt")).unwrap();
        assert!(readme.contains("Credits:") && readme.contains("CrateMaker"), "{readme}");
        // installed elsewhere: the credits stay with the prefab
        let lib2 = root.join("lib2");
        install_dir(&lib2, &un, "x", &prefab::Origin::file(&zip)).unwrap();
        assert_eq!(mine(&lib2)[0].1.credits, credits);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn library_save_list_load_delete_and_packages() {
        let root = tmp("lib");
        let lib = lib_dir(&root);
        let (frag, meta) = load(&lib, "builtin:crate_stack").unwrap();
        let png = prefab::thumbnail(&frag, 64, 48);
        let a = save(&lib, &frag, &meta, Some(&png)).unwrap();
        let b = save(&lib, &frag, &meta, None).unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("mine:crate_stack", "mine:crate_stack_2"));
        assert!(save(&lib, &frag, &meta, Some(b"nope")).is_err());
        let l = list(&lib);
        let items = l["prefabs"].as_array().unwrap();
        assert_eq!(items.iter().filter(|x| x["builtin"] == json!(false)).count(), 2);
        assert_eq!(items.iter().filter(|x| x["builtin"] == json!(true)).count(), 8);
        let (back, m) = load(&lib, &a).unwrap();
        assert_eq!(back.brushes.len(), frag.brushes.len());
        assert_eq!(m.name, "Crate stack");
        assert!(load(&lib, "mine:../../etc").is_err() && load(&lib, "builtin:nope").is_err() && load(&lib, "x").is_err());

        // a package round trip: zip -> unpack -> install into another library
        let zip = root.join("out.zip");
        package(&lib, &a, &zip, None).unwrap();
        let un = root.join("un");
        std::fs::create_dir_all(&un).unwrap();
        crate::archive::extract(&zip, &un, 64 << 20).unwrap();
        assert!(un.join("prefabs/crate_stack.map").is_file() && un.join("loner-strike.json").is_file());
        std::fs::write(un.join("prefabs/mytex.wad"), b"WAD3\0\0\0\0\0\0\0\0").unwrap();
        let lib2 = root.join("lib2");
        let community = prefab::Origin::community("mod:501", 77);
        let got = install_dir(&lib2, &un, "someone", &community).unwrap();
        assert_eq!(got.prefabs, vec!["Crate stack"]);
        assert_eq!(got.wads.len(), 1);
        assert!(lib2.join("crate_stack.png").is_file());
        let game = root.join("cstrike");
        std::fs::create_dir_all(&game).unwrap();
        assert_eq!(install_wads(&got.wads, &game), vec!["mytex.wad"]);
        // never over a file that's there
        assert!(install_wads(&got.wads, &game).is_empty());

        // a plain .map picked from the computer
        let raw = root.join("fence.map");
        std::fs::write(&raw, crate::editor::mapfile::write_map(&frag, &Default::default()).unwrap()).unwrap();
        let v = install_file(&lib2, &root.join("work"), &raw, None).unwrap();
        assert_eq!(v["prefabs"], json!(["fence"]));
        assert!(install_file(&lib2, &root.join("work"), &root.join("out.zip.nope"), None).is_err());
        // nothing usable
        let empty = root.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("readme.txt"), "hi").unwrap();
        assert!(install_dir(&lib2, &empty, "", &community).is_err());

        // P5: uids: a new one per saved prefab, kept in the package and by the install
        let ua = mine(&lib).into_iter().find(|(i, _)| format!("{MINE}{i}") == a).unwrap().1.uid;
        assert!(crate::editor::doc::valid_uid(&ua));
        let ub = mine(&lib).into_iter().find(|(i, _)| format!("{MINE}{i}") == b).unwrap().1.uid;
        assert_ne!(ua, ub, "a starter prefab saved twice: two prefabs");
        let pk: Value = serde_json::from_slice(&std::fs::read(un.join("loner-strike.json")).unwrap()).unwrap();
        assert_eq!(pk["item"]["prefab"]["uid"], json!(ua));
        assert_eq!(mine(&lib2).iter().find(|(_, m)| m.name == "Crate stack").unwrap().1.uid, ua);
        // P5 (identity isn't permission): the package records where it came from, never its own claim
        let in2 = || mine(&lib2).into_iter().find(|(_, m)| m.uid == ua).unwrap();
        assert_eq!(in2().1.origin, Some(community.clone()));
        let before = std::fs::read(lib2.join(format!("{}.map", in2().0))).unwrap();
        // the same creation again (a newer version): waits for a yes, nothing changed yet
        let n2 = mine(&lib2).len();
        let again = install_dir(&lib2, &un, "someone", &community).unwrap();
        assert_eq!((again.waiting.clone(), again.prefabs.len()), (vec!["Crate stack".to_string()], 0));
        assert_eq!(mine(&lib2).len(), n2);
        assert_eq!(std::fs::read(lib2.join(format!("{}.map", in2().0))).unwrap(), before);
        let p = pending(&lib2);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0]["uid"], json!(ua));
        assert!(p[0]["changes"].as_array().is_some_and(|c| !c.is_empty()));
        // no: yours stays; yes: updated in place, same id and uid
        settle(&lib2, &ua, false).unwrap();
        assert!(pending(&lib2).is_empty());
        install_dir(&lib2, &un, "someone", &community).unwrap();
        let id_before = in2().0;
        assert_eq!(settle(&lib2, &ua, true).unwrap()["updated"], json!(format!("{MINE}{id_before}")));
        assert_eq!((in2().0, mine(&lib2).len()), (id_before.clone(), n2));
        assert!(settle(&lib2, &ua, true).is_err(), "settled already");
        // a waiting update is tied to its prefab (library id) and its source, checked again on yes
        let keep_map = std::fs::read(lib2.join(format!("{id_before}.map"))).unwrap();
        let keep_json = std::fs::read_to_string(lib2.join(format!("{id_before}.json"))).unwrap();
        let held = root.join("held");
        let hold = || {
            install_dir(&lib2, &un, "someone", &community).unwrap();
            let _ = std::fs::remove_dir_all(&held);
            std::fs::create_dir_all(&held).unwrap();
            for f in ["prefab.map", "prefab.json", "target.json"] {
                std::fs::copy(pending_dir(&lib2).join(&ua).join(f), held.join(f)).unwrap();
            }
        };
        let put_back = || {
            std::fs::create_dir_all(pending_dir(&lib2).join(&ua)).unwrap();
            for f in ["prefab.map", "prefab.json", "target.json"] {
                std::fs::copy(held.join(f), pending_dir(&lib2).join(&ua).join(f)).unwrap();
            }
        };
        let write_as = |stem: &str, name: &str, origin: prefab::Origin| {
            let mut m: PrefabMeta = serde_json::from_str(&keep_json).unwrap();
            m.name = name.into();
            m.origin = Some(origin);
            std::fs::write(lib2.join(format!("{stem}.map")), &keep_map).unwrap();
            std::fs::write(lib2.join(format!("{stem}.json")), serde_json::to_string(&m).unwrap()).unwrap();
        };
        // 1. the prefab is deleted (its waiting update goes with it), then one with the same uid
        //    from a file is added: an update left behind is refused and dropped, the new one untouched
        hold();
        delete(&lib2, &id_before).unwrap();
        assert!(!pending_dir(&lib2).join(&ua).exists(), "deleting drops its waiting update");
        write_as("recreated", "Recreated", prefab::Origin::file(&root.join("elsewhere.map")));
        put_back();
        let e = settle(&lib2, &ua, true).unwrap_err();
        assert!(e.contains("isn't the prefab this update was for"), "{e}");
        assert!(!pending_dir(&lib2).join(&ua).exists());
        assert_eq!(std::fs::read(lib2.join("recreated.map")).unwrap(), keep_map);
        assert!(std::fs::read_to_string(lib2.join("recreated.json")).unwrap().contains("Recreated"));
        // recreated under the same library id from another source: still refused
        delete(&lib2, "recreated").unwrap();
        write_as(&id_before, "Same id, other source", prefab::Origin::community("mod:501", 999));
        put_back();
        assert!(pending(&lib2).iter().all(|p| p["uid"] != json!(ua)), "pending() drops it");
        assert!(!pending_dir(&lib2).join(&ua).exists());
        put_back();
        assert!(settle(&lib2, &ua, true).is_err());
        assert!(std::fs::read_to_string(lib2.join(format!("{id_before}.json"))).unwrap().contains("Same id, other source"));
        // 2. the original back (same id, same source): the held update applies
        std::fs::write(lib2.join(format!("{id_before}.json")), &keep_json).unwrap();
        put_back();
        assert_eq!(pending(&lib2).len(), 1);
        settle(&lib2, &ua, false).unwrap();
        // the same uid from another uploader, another page, or a file: a copy with a new uid, yours untouched
        for other in [prefab::Origin::community("mod:501", 78), prefab::Origin::community("mod:999", 77), prefab::Origin::file(&zip)] {
            let n = mine(&lib2).len();
            let got = install_dir(&lib2, &un, "x", &other).unwrap();
            assert_eq!(got.copies.len(), 1, "{other:?}");
            assert!(got.waiting.is_empty());
            assert_eq!(mine(&lib2).len(), n + 1);
            assert_eq!((in2().0, in2().1.origin), (id_before.clone(), Some(community.clone())));
        }
        // an export: importing that same file into the library it came from is the same source (asks)
        let exp = lib.join(format!("{}.json", a.strip_prefix(MINE).unwrap()));
        assert!(std::fs::read_to_string(&exp).unwrap().contains("out.zip"), "the export is recorded");
        let back = install_file(&lib, &root.join("work"), &zip, None).unwrap();
        assert_eq!(back["waiting"], json!(["Crate stack"]));
        // the package itself never carries where it came from or was exported to
        let shipped = std::fs::read_to_string(un.join("prefabs/crate_stack.json")).unwrap();
        assert!(!shipped.contains("origin") && !shipped.contains("exports"), "{shipped}");
        // a package claiming an origin: ignored
        let forged = shipped.replacen("{", "{\"origin\": {\"kind\": \"community\", \"page\": \"mod:501\", \"owner\": 77},", 1);
        std::fs::write(un.join("prefabs/crate_stack.json"), &forged).unwrap();
        let n = mine(&lib2).len();
        let got = install_dir(&lib2, &un, "x", &prefab::Origin::community("mod:666", 5)).unwrap();
        assert_eq!((got.copies.len(), mine(&lib2).len()), (1, n + 1));
        std::fs::write(un.join("prefabs/crate_stack.json"), &shipped).unwrap();
        // Save's Replace it: the same prefab, same uid
        let (f3, mut m3) = load(&lib, &a).unwrap();
        m3.name = "Crate stack v2".into();
        assert_eq!(save_over(&lib, a.strip_prefix(MINE).unwrap(), &f3, &m3, None).unwrap(), a);
        let now = mine(&lib).into_iter().find(|(i, _)| format!("{MINE}{i}") == a).unwrap().1;
        assert_eq!((now.uid.as_str(), now.name.as_str()), (ua.as_str(), "Crate stack v2"));
        // a plain save with a uid that's taken: a new prefab, the other untouched
        let c = save(&lib, &f3, &now, None).unwrap();
        assert_ne!(c, a);
        assert_ne!(mine(&lib).into_iter().find(|(i, _)| format!("{MINE}{i}") == c).unwrap().1.uid, ua);
        // by uid
        assert_eq!(load(&lib, &format!("uid:{ua}")).unwrap().1.name, "Crate stack v2");
        assert_eq!(id_of_uid(&lib, &ua).as_deref(), Some(a.as_str()));
        assert!(load(&lib, "uid:not-a-uid").is_err());
        assert!(id_of_uid(&lib, &prefab::builtins()[0].1.uid).unwrap().starts_with(BUILTIN));
        // a package with a broken uid gets a new one; an old prefab without one gets one on load,
        // written into its file; two with the same uid (copied by hand): the second gets a new one
        let js = std::fs::read_to_string(un.join("prefabs/crate_stack.json")).unwrap();
        std::fs::write(un.join("prefabs/crate_stack.json"), js.replace(&ua, "../../not-a-uuid")).unwrap();
        let lib3 = root.join("lib3");
        install_dir(&lib3, &un, "", &community).unwrap();
        let u3 = mine(&lib3)[0].1.uid.clone();
        assert!(crate::editor::doc::valid_uid(&u3) && u3 != ua);
        let old = std::fs::read_to_string(lib3.join("crate_stack.json")).unwrap().replace(&format!("\"uid\": \"{u3}\","), "");
        assert!(!old.contains("\"uid\""));
        std::fs::write(lib3.join("crate_stack.json"), &old).unwrap();
        std::fs::copy(lib3.join("crate_stack.map"), lib3.join("crate_copy.map")).unwrap();
        let m = mine(&lib3);
        assert!(m.iter().all(|(_, x)| crate::editor::doc::valid_uid(&x.uid)));
        assert!(std::fs::read_to_string(lib3.join("crate_stack.json")).unwrap().contains("\"uid\""));
        std::fs::write(lib3.join("crate_copy.json"), std::fs::read_to_string(lib3.join("crate_stack.json")).unwrap()).unwrap();
        let m = mine(&lib3);
        assert_eq!(m.len(), 2);
        assert_ne!(m[0].1.uid, m[1].1.uid);
        // placed: the group remembers it
        let mut d = MapDocument::new();
        let mut h = history::History::default();
        let (frag2, meta2) = load(&lib, &a).unwrap();
        assert_eq!(meta2.uid, ua);
        let (_, g) = history::cmd_place_prefab_from(&mut d, &mut h, &frag2, &crate::editor::math::Affine::translate(crate::editor::math::v3(0.0, 0.0, 0.0)), &meta2.name, &[], Some(&meta2.uid)).unwrap();
        assert_eq!(d.groups[&g].prefab.as_deref(), Some(ua.as_str()));

        // delete: only your own
        let gone = mine(&lib).len();
        std::fs::remove_file(lib.join(format!("{}.map", b.strip_prefix(MINE).unwrap()))).unwrap();
        assert_eq!(mine(&lib).len(), gone - 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
