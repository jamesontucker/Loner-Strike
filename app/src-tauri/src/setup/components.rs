//! Catalog components: game modes and add-ons made by the community.
//!
//! A component (catalog/components.json, catalog/parts/*.json) can have:
//!   vendor   folder (under the app's resources) mirroring the game folder: the mod's
//!            sources with Loner-Strike's bot patches, its configs and language files
//!   compile  plugins to build: {"src": path under scripting/, "out": name, "include": dir or null}
//!   fetch    resources from the original project, each file checked by sha256:
//!            {"archive": url, "strip": "prefix/inside/archive/", "files": {"rel/path": sha256}}
//!            {"url": url, "to": "rel/path", "sha256": "..."}
//!            a file's value can also be {"from": "path/in/archive", "sha256": "..."}
//!   patches  unified diffs applied to fetched files (mods whose code has no license)
//!   maps     maps from GameBanana: {"mod": id, "file": id, "maps": [names]}
//!   dirs     empty folders the mod expects
//! Models, sounds and maps that already exist are never replaced.

use super::record::{inside, Record};
use super::{core, fetch, foundation::tree, patch, Ctx, TempDir};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

/// components.json plus one file per packaged mod in catalog/parts/ (those replace
/// the entry with the same id, in its place).
pub fn load_catalog(res: &Path) -> Result<Vec<Value>, String> {
    let main = res.join("catalog").join("components.json");
    let text = std::fs::read_to_string(&main).map_err(|e| format!("Loner-Strike's catalog is missing ({e})."))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("Loner-Strike's catalog is damaged ({e})."))?;
    let mut comps: Vec<Value> = v["components"].as_array().cloned().unwrap_or_default();
    let parts = res.join("catalog").join("parts");
    let mut names: Vec<String> = std::fs::read_dir(&parts)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".json")).collect())
        .unwrap_or_default();
    names.sort();
    for name in names {
        let t = std::fs::read_to_string(parts.join(&name)).map_err(|e| format!("Couldn't read the catalog ({name}: {e})."))?;
        let part: Value = serde_json::from_str(&t).map_err(|e| format!("Loner-Strike's catalog is damaged ({name}: {e})."))?;
        match comps.iter().position(|c| c["id"] == part["id"]) {
            Some(i) => comps[i] = part,
            None => comps.push(part),
        }
    }
    Ok(comps)
}

/// The entry in components.json itself (parts don't repeat its notes, e.g. bot_patched).
pub fn base_entry(res: &Path, id: &str) -> Option<Value> {
    let text = std::fs::read_to_string(res.join("catalog").join("components.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v["components"].as_array()?.iter().find(|c| c["id"] == id).cloned()
}

pub fn strs(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect()
}

pub fn installable(c: &Value) -> bool {
    matches!(c["kind"].as_str(), Some("mode") | Some("addon")) && c["installable"].as_bool() == Some(true)
}

/// Games a component works in (both when it doesn't say).
pub fn games_of(c: &Value) -> Vec<String> {
    match c.get("games") {
        Some(g) if g.is_array() => strs(g),
        _ => super::GAMES.iter().map(|(g, _)| g.to_string()).collect(),
    }
}

fn copy_vendor(rec: &mut Record, ctx: &Ctx, vendor: &str) -> Result<(), String> {
    let root = inside(ctx.res, vendor).ok_or("The catalog names a folder outside Loner-Strike's files.")?;
    if !root.is_dir() {
        return Err(format!("Loner-Strike's files are incomplete ({vendor} is missing). Reinstalling the app should fix it."));
    }
    for (full, rel) in tree(&root) {
        rec.copy(&full, &rel)?;
    }
    Ok(())
}

/// Copies one fetched file, unless the player already has a file there.
#[cfg(test)]
fn place_resource(rec: &mut Record, src: &Path, rel: &str, sha: Option<&str>) -> Result<bool, String> {
    place_resource_as(rec, src, rel, sha, false)
}

/// `patched`: our bot fixes go onto this file, so it's always the downloaded original
/// (the player's own copy is backed up, not patched: it may be another version)
fn place_resource_as(rec: &mut Record, src: &Path, rel: &str, sha: Option<&str>, patched: bool) -> Result<bool, String> {
    if let Some(s) = sha {
        if !fetch::sha256(src).map(|h| h.eq_ignore_ascii_case(s)).unwrap_or(false) {
            return Err(format!("{rel} changed upstream (its checksum differs), so setup stopped to keep your game safe."));
        }
    }
    if !patched && rec.exists(rel) && !rec.ours(rel) {
        return Ok(false);
    }
    rec.copy(src, rel)?;
    Ok(true)
}

pub(crate) fn fetch_all(rec: &mut Record, ctx: &Ctx, comp: &Value, tmp: &TempDir) -> Result<(), String> {
    let name = comp["name"].as_str().unwrap_or("this mode");
    // the files our patches change
    let patched: std::collections::HashSet<String> = strs(&comp["patches"])
        .iter()
        .filter_map(|p| inside(ctx.res, p))
        .filter_map(|p| crate::sys::read_latin1(&p).ok())
        .filter_map(|t| patch::parse(&t).ok())
        .flat_map(|files| files.into_iter().map(|(rel, _)| rel.to_lowercase()))
        .collect();
    let items = comp["fetch"].as_array().cloned().unwrap_or_default();
    for (i, item) in items.iter().enumerate() {
        ctx.progress.fraction(0.6 * i as f64 / items.len().max(1) as f64);
        let what = if items.len() > 1 { format!("files for {name} ({} of {})", i + 1, items.len()) } else { format!("files for {name}") };
        if let Some(url) = item["archive"].as_str() {
            let file = fetch::download(url, item["sha256"].as_str(), &what, ctx.progress)?;
            ctx.progress.detail(&format!("Unpacking {what}"));
            let where_ = fetch::unpack(&file, &tmp.0.join(i.to_string()))?;
            let strip = item["strip"].as_str().unwrap_or("");
            for (rel, spec) in item["files"].as_object().into_iter().flatten() {
                let (origin, sha) = match spec {
                    Value::Object(o) => (o.get("from").and_then(|v| v.as_str()).unwrap_or(rel).to_string(), o.get("sha256").and_then(|v| v.as_str())),
                    v => (rel.clone(), v.as_str()),
                };
                let src = inside(&where_, &format!("{strip}{origin}")).filter(|p| p.is_file());
                let Some(src) = src else {
                    return Err(format!("{rel} is missing from the download of {name}."));
                };
                place_resource_as(rec, &src, rel, sha, patched.contains(&rel.to_lowercase()))?;
            }
        } else if let (Some(url), Some(to)) = (item["url"].as_str(), item["to"].as_str()) {
            let sha = item["sha256"].as_str();
            let file = fetch::download(url, sha, &what, ctx.progress)?;
            place_resource_as(rec, &file, to, sha, patched.contains(&to.to_lowercase()))?;
        }
    }
    Ok(())
}

fn apply_patches(rec: &mut Record, ctx: &Ctx, comp: &Value) -> Result<(), String> {
    for p in strs(&comp["patches"]) {
        let path = inside(ctx.res, &p).ok_or("The catalog names a patch outside Loner-Strike's files.")?;
        let text = crate::sys::read_latin1(&path).map_err(|e| format!("Loner-Strike's files are incomplete ({p}: {e})."))?;
        let files = patch::parse(&text).map_err(|e| format!("{p}: {e}"))?;
        for (rel, hunks) in files {
            let Some(text) = rec.read(&rel) else {
                return Err(format!("{p} needs {rel}, which wasn't downloaded."));
            };
            let new = patch::apply(&text, &hunks).map_err(|e| format!("Loner-Strike's bot fixes for {rel} don't fit the downloaded file ({e})."))?;
            rec.write(&rel, &new)?;
        }
    }
    Ok(())
}

/// Installs the component's GameBanana maps and remembers the ones it put in, so uninstalling
/// the component removes them again. A map it put in earlier that the list no longer has (one
/// replaced because its download was missing files) comes out again, and one it put in that the
/// player has since removed (Get maps) isn't fetched again. `base`..`base + span` is the part of
/// the step's progress bar the downloads fill.
pub(crate) fn maps(rec: &mut Record, ctx: &Ctx, comp: &Value, base: f64, span: f64) {
    let Some(list) = comp["maps"].as_array().filter(|l| !l.is_empty()) else { return };
    let Some(store) = ctx.store.as_ref() else { return };
    let id = comp["id"].as_str().unwrap_or_default().to_string();
    let mut ours = rec.map_mods(&id);
    let installed = |m: i64| store.records().contains_key(&m.to_string());
    // (0.1.2 noted a map as ours before its download had worked: the first time through, only
    // what really came in stays noted, so a download that failed then is tried again)
    let checked_key = "maps_checked";
    let mut checked: Vec<String> = rec.data.extra.get(checked_key).map(strs).unwrap_or_default();
    if !checked.contains(&id) {
        ours.retain(|m| installed(*m));
        checked.push(id.clone());
        rec.data.extra.insert(checked_key.into(), Value::from(checked));
    }
    let listed: Vec<i64> = list.iter().filter_map(|m| m["mod"].as_i64()).collect();
    let dropped: Vec<i64> = ours.iter().copied().filter(|m| !listed.contains(m)).collect();
    for m in dropped {
        if store.uninstall(m).is_ok() || !installed(m) {
            ours.retain(|x| *x != m);
        }
    }
    rec.data.maps.insert(id.clone(), Value::from(ours.clone()));
    let maps_dir = store.maps_dir();
    let total = list.len();
    for (i, m) in list.iter().enumerate() {
        let names = strs(&m["maps"]);
        let (Some(mod_id), Some(file_id)) = (m["mod"].as_i64(), m["file"].as_i64()) else { continue };
        let have = names.iter().all(|n| maps_dir.join(format!("{n}.bsp")).exists());
        // (there already, or put in by us once and removed by the player since)
        if have || (ours.contains(&mod_id) && !installed(mod_id)) {
            continue;
        }
        ctx.progress.detail(&format!("Getting the map {} ({} of {total})", names.join(", "), i + 1));
        ctx.progress.fraction(base + span * i as f64 / total as f64);
        // (the player's own copy of the same download is never claimed)
        let theirs = installed(mod_id);
        let job = store.start_install(mod_id, file_id);
        let deadline = std::time::Instant::now() + Duration::from_secs(1800);
        let ok = loop {
            let j = store.job_list().into_iter().find(|j| j.id == job);
            match j {
                Some(j) if j.state == "done" => break true,
                Some(j) if j.state == "error" => {
                    ctx.progress.note(&format!("Couldn't install the map {}: {}", names.join(", "), j.message));
                    break false;
                }
                None => break false,
                _ if std::time::Instant::now() > deadline => {
                    ctx.progress.note(&format!("Gave up waiting for the map {}.", names.join(", ")));
                    break false;
                }
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        };
        if ok && !theirs && installed(mod_id) && !ours.contains(&mod_id) {
            ours.push(mod_id);
            rec.data.maps.insert(id.clone(), Value::from(ours.clone()));
        }
    }
}

/// Installs one component. Ok(false) if a plugin didn't compile (it won't load).
pub fn install_component(rec: &mut Record, ctx: &Ctx, comp: &Value) -> Result<bool, String> {
    let id = comp["id"].as_str().unwrap_or_default();
    rec.begin(id);
    if let Some(v) = comp["vendor"].as_str() {
        ctx.progress.detail("Copying the mod's files");
        copy_vendor(rec, ctx, v)?;
    }
    for d in strs(&comp["dirs"]) {
        if let Some(p) = inside(&rec.game, &d) {
            let _ = std::fs::create_dir_all(p);
        }
    }
    let tmp = TempDir::new()?;
    fetch_all(rec, ctx, comp, &tmp)?;
    drop(tmp);
    if comp["patches"].as_array().map(|a| !a.is_empty()).unwrap_or(false) {
        ctx.progress.detail("Applying Loner-Strike's bot fixes");
        apply_patches(rec, ctx, comp)?;
    }
    let mut ok = true;
    let builds = comp["compile"].as_array().cloned().unwrap_or_default();
    for (i, c) in builds.iter().enumerate() {
        let (Some(src), Some(out)) = (c["src"].as_str(), c["out"].as_str()) else { continue };
        ctx.progress.detail(&format!("Compiling {out} ({} of {})", i + 1, builds.len()));
        ctx.progress.fraction(0.6 + 0.2 * i as f64 / builds.len() as f64);
        ok = core::compile_plugin(rec, ctx, src, out, c["include"].as_str())? && ok;
    }
    maps(rec, ctx, comp, 0.8, 0.2);
    // a mode whose plugin didn't build stays out of the menu (it would load its settings
    // without its plugin); setup says it failed, and running it again tries again
    if ok {
        rec.done(id, Value::from(super::shipped_version(ctx.res, id)))?;
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_are_checked_and_players_files_kept() {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "res-").unwrap();
        let game = root.join("cstrike");
        std::fs::create_dir_all(game.join("sound")).unwrap();
        std::fs::write(game.join("sound/mine.wav"), b"the player's own").unwrap();
        let src = root.join("download.wav");
        std::fs::write(&src, b"fetched").unwrap();
        let sha = fetch::sha256(&src).unwrap();
        let mut rec = Record::open(&game);
        rec.begin("mode-x");
        let err = place_resource(&mut rec, &src, "sound/new.wav", Some(&"0".repeat(64))).unwrap_err();
        assert!(err.contains("changed upstream"), "{err}");
        assert!(!place_resource(&mut rec, &src, "sound/mine.wav", Some(&sha)).unwrap());
        assert_eq!(std::fs::read(game.join("sound/mine.wav")).unwrap(), b"the player's own");
        assert!(place_resource(&mut rec, &src, "sound/new.wav", Some(&sha)).unwrap());
        // a file that's ours gets replaced on a repair
        assert!(place_resource(&mut rec, &src, "sound/new.wav", Some(&sha)).unwrap());
        assert!(rec.prepare("../outside.txt").is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
