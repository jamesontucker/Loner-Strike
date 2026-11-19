//! 1.0.1: what the map editor keeps of a map's builds (tools/compile.rs and the Compile panel).
//!
//! - The last compile and the last playtest of each map, on disk: a text log ("Open the full log")
//!   and a short summary (the panel shows its errors when it opens).
//! - What bots last learned of each map: a playtest after a compile offers to teach them again
//!   when the map's layout changed since (the nav would be out of date), and skips teaching when
//!   they already know this version.
//!
//! Map knowledge is matched by a fingerprint of the compiled source's layout (brushes, the
//! textures that change how players move, entities but not lights), never by file times: every
//! compile writes a new .bsp, so a .bsp is always newer than its nav.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Where the logs live: `<app's compile folder>/logs/<map>/{compile,playtest}.{txt,json}`.
pub fn logs_dir() -> PathBuf {
    crate::localdata::dir("tools/compile").join("logs")
}

/// What bots learned of each map (and what each map was compiled from).
pub fn nav_book() -> PathBuf {
    crate::localdata::dir("tools/compile").join("nav-learned.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A map's folder name (lower case), None for anything that isn't a map name.
fn folder(map: &str) -> Option<String> {
    super::compile::valid_map_name(map).then(|| map.to_ascii_lowercase())
}

/// Which log: the compile's or the playtest's.
pub fn which_ok(which: &str) -> bool {
    matches!(which, "compile" | "playtest")
}

/// Keeps a log and its summary as the map's last `which` ("compile" or "playtest"), replacing
/// the one before. The summary gets `when` (seconds since 1970) and `log` (the text's path).
pub fn keep(dir: &Path, map: &str, which: &str, text: &str, mut summary: Value) -> Result<PathBuf, String> {
    let f = folder(map).ok_or("That isn't a map name.")?;
    if !which_ok(which) {
        return Err("Unknown log.".into());
    }
    let d = dir.join(f);
    std::fs::create_dir_all(&d).map_err(|e| format!("Couldn't keep the log ({e})."))?;
    let txt = d.join(format!("{which}.txt"));
    crate::sys::write_atomic(&txt, text.as_bytes()).map_err(|e| format!("Couldn't keep the log ({e})."))?;
    if let Some(o) = summary.as_object_mut() {
        o.entry("when").or_insert(json!(now_secs()));
        o.insert("log".into(), json!(txt.to_string_lossy()));
        o.insert("map".into(), json!(map));
    }
    crate::sys::write_atomic(&d.join(format!("{which}.json")), serde_json::to_string(&summary).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't keep the log ({e})."))?;
    Ok(txt)
}

/// The map's last compile and playtest summaries (null when there's none, or its log is gone).
pub fn last(dir: &Path, map: &str) -> Value {
    let Some(f) = folder(map) else { return json!({"compile": null, "playtest": null}) };
    let read = |which: &str| -> Value {
        let d = dir.join(&f);
        if !d.join(format!("{which}.txt")).is_file() {
            return Value::Null;
        }
        std::fs::read_to_string(d.join(format!("{which}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
    };
    json!({"compile": read("compile"), "playtest": read("playtest")})
}

/// The text log's path when there is one.
pub fn log_path(dir: &Path, map: &str, which: &str) -> Option<PathBuf> {
    let p = dir.join(folder(map)?).join(format!("{which}.txt"));
    (which_ok(which) && p.is_file()).then_some(p)
}

/// "2026-11-18 14:03 UTC" for a log's first line.
pub fn utc(secs: u64) -> String {
    let secs = secs as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil-from-days (Howard Hinnant), as sys::stamp
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, rem % 3600 / 60)
}

/// "45 s", "3 min 5 s", "1 h 2 min".
pub fn duration(s: u64) -> String {
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min {} s", s / 60, s % 60)
    } else {
        format!("{} h {} min", s / 3600, s % 3600 / 60)
    }
}

/// The hidden server's console without its graphics driver's report (MoltenVK lists every
/// Vulkan extension and GPU feature, twice, under CrossOver): the lines that say something.
pub fn learner_console(text: &str) -> String {
    let mut out = Vec::new();
    let mut in_report = false;
    for l in text.lines() {
        if l.contains("[mvk-") {
            in_report = true;
            continue;
        }
        if in_report && (l.starts_with('\t') || l.starts_with(' ')) {
            continue;
        }
        in_report = false;
        out.push(l);
    }
    out.join("\n")
}

// ---- what bots know of each map --------------------------------------------------------------

/// Textures that change how players move or where the nav can go: kept in the fingerprint
/// (retexturing anything else doesn't make bots' knowledge old).
fn moves_players(tex: &str) -> bool {
    let t = tex.to_ascii_lowercase();
    t.starts_with('!') || t.starts_with('*') || t.starts_with("water") || t.starts_with("{")
        || ["clip", "null", "nullnoclip", "sky", "origin", "hint", "skip", "aaatrigger", "bevel", "solidhint", "clipbevel", "clipbevelbrush", "translucent"].contains(&t.as_str())
}

/// A fingerprint of a compile's `.map` text, for what matters to bots finding their way: brush
/// planes, movement textures and entities (lights, the WAD list and texture alignment left out).
pub fn nav_fingerprint(map_text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    let mut depth = 0i32;
    // (an entity's lines, held until its classname is known)
    let mut ent: Vec<String> = Vec::new();
    let mut class = String::new();
    let flush = |h: &mut Sha256, ent: &mut Vec<String>, class: &mut String| {
        let c = class.to_ascii_lowercase();
        if !(c.starts_with("light") || c == "env_glow" || c == "env_sprite" || c.starts_with("ambient_")) {
            for l in ent.iter() {
                h.update(l.as_bytes());
                h.update(b"\n");
            }
        }
        ent.clear();
        class.clear();
    };
    for raw in map_text.lines() {
        let l = raw.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        if l == "{" {
            depth += 1;
            ent.push("{".into());
            continue;
        }
        if l == "}" {
            depth -= 1;
            ent.push("}".into());
            if depth <= 0 {
                depth = 0;
                flush(&mut h, &mut ent, &mut class);
            }
            continue;
        }
        if depth == 1 && l.starts_with('"') {
            let parts: Vec<&str> = l.split('"').collect();
            // "key" "value" -> ["", key, " ", value, ""]
            let (k, v) = (parts.get(1).copied().unwrap_or(""), parts.get(3).copied().unwrap_or(""));
            if k.eq_ignore_ascii_case("classname") {
                class = v.to_string();
            }
            if k.eq_ignore_ascii_case("wad") || k.eq_ignore_ascii_case("mapversion") || k.starts_with('_') {
                continue;
            }
            ent.push(format!("\"{k}\" \"{v}\""));
            continue;
        }
        if l.starts_with('(') {
            // ( x y z ) ( x y z ) ( x y z ) TEXTURE [alignment...]
            let mut closed = 0;
            let mut end = l.len();
            for (i, c) in l.char_indices() {
                if c == ')' {
                    closed += 1;
                    if closed == 3 {
                        end = i + 1;
                        break;
                    }
                }
            }
            let tex = l[end..].split_whitespace().next().unwrap_or("");
            let tex = if moves_players(tex) { tex.to_ascii_lowercase() } else { "-".into() };
            ent.push(format!("{} {tex}", l[..end].split_whitespace().collect::<Vec<_>>().join(" ")));
            continue;
        }
        ent.push(l.to_string());
    }
    flush(&mut h, &mut ent, &mut class);
    h.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn file_sha(p: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let meta = std::fs::metadata(p).ok()?;
    if meta.len() > 512 << 20 {
        return None;
    }
    let b = std::fs::read(p).ok()?;
    Some(Sha256::digest(&b).iter().take(16).map(|b| format!("{b:02x}")).collect())
}

fn read_book(file: &Path) -> serde_json::Map<String, Value> {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn write_book(file: &Path, book: &serde_json::Map<String, Value>) {
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = crate::sys::write_atomic(file, serde_json::to_string(book).unwrap_or_default().as_bytes());
}

/// Notes that `map` was compiled from `fp` into `bsp` (the copy in the game's maps folder), so a
/// nav later learned from that .bsp is known to match `fp`.
pub fn note_compiled(file: &Path, map: &str, fp: &str, bsp: &Path) {
    let Some(sha) = file_sha(bsp) else { return };
    let mut book = read_book(file);
    let key = map.to_ascii_lowercase();
    let mut e = book.get(&key).cloned().unwrap_or_else(|| json!({}));
    e["compiled"] = json!({"fp": fp, "bsp": sha});
    book.insert(key, e);
    // (a few hundred maps at most; the oldest learned ones go first past that)
    if book.len() > 500 {
        if let Some(k) = book.keys().next().cloned() {
            book.remove(&k);
        }
    }
    write_book(file, &book);
}

/// Notes that bots learned `map` from `bsp`, their knowledge in `nav`: matched to the compile
/// that made that .bsp (unknown when it wasn't compiled here, or compiled again since).
pub fn note_learned(file: &Path, map: &str, bsp: &Path, nav: &Path) {
    let (Some(bsp_sha), Some(nav_sha)) = (file_sha(bsp), file_sha(nav)) else { return };
    let mut book = read_book(file);
    let key = map.to_ascii_lowercase();
    let mut e = book.get(&key).cloned().unwrap_or_else(|| json!({}));
    let fp = (e["compiled"]["bsp"].as_str() == Some(bsp_sha.as_str())).then(|| e["compiled"]["fp"].clone()).unwrap_or(Value::Null);
    e["learned"] = json!({"fp": fp, "nav": nav_sha, "when": now_secs()});
    book.insert(key, e);
    write_book(file, &book);
}

/// The nav beside a map's .bsp (any case), as the game finds it.
pub fn nav_beside(bsp: &Path, map: &str) -> Option<PathBuf> {
    let dir = bsp.parent()?;
    let want = format!("{}.nav", map.to_ascii_lowercase());
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase() == want))
}

/// Bots' knowledge of `map` against a compile from `fp`: {state, learnedAt}, state is
/// "none" (no nav), "current" (learned from this layout), "changed" (learned from an older
/// layout) or "unknown" (a nav from elsewhere: Valve's, another tool, or replaced since).
pub fn nav_state(file: &Path, map: &str, fp: &str, nav: Option<&Path>) -> Value {
    let Some(nav) = nav.filter(|p| p.is_file()) else { return json!({"state": "none"}) };
    let book = read_book(file);
    let e = book.get(&map.to_ascii_lowercase()).cloned().unwrap_or(Value::Null);
    let learned = &e["learned"];
    let when = learned["when"].clone();
    if learned["nav"].as_str().is_none() || learned["nav"].as_str() != file_sha(nav).as_deref() {
        return json!({"state": "unknown"});
    }
    match learned["fp"].as_str() {
        Some(f) if f == fp => json!({"state": "current", "learnedAt": when}),
        Some(_) => json!({"state": "changed", "learnedAt": when}),
        None => json!({"state": "unknown", "learnedAt": when}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &str = "// game: Counter-Strike\n{\n\"classname\" \"worldspawn\"\n\"wad\" \"/a/halflife.wad\"\n\"mapversion\" \"220\"\n{\n( 0 0 0 ) ( 0 1 0 ) ( 1 0 0 ) CRATE01 [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1\n( 0 0 64 ) ( 1 0 64 ) ( 0 1 64 ) CLIP [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1\n}\n}\n{\n\"classname\" \"info_player_start\"\n\"origin\" \"0 0 32\"\n}\n{\n\"classname\" \"light\"\n\"origin\" \"0 0 48\"\n\"_light\" \"255 255 255 200\"\n}\n";

    #[test]
    fn the_fingerprint_follows_the_layout_not_the_looks() {
        let fp = nav_fingerprint(ROOM);
        // the same layout on another computer (WAD paths), retextured, realigned, lights moved: the same
        let other = ROOM.replace("/a/halflife.wad", "C:\\x\\cs.wad").replace("CRATE01 [ 1 0 0 0 ]", "BRICK [ 1 0 0 16 ]").replace("\"0 0 48\"", "\"32 0 48\"").replace("200\"", "50\"");
        assert_eq!(nav_fingerprint(&other), fp);
        // a wall moved, a spawn moved, a clip brush made into a normal one: changed
        assert_ne!(nav_fingerprint(&ROOM.replace("( 0 0 64 ) ( 1 0 64 )", "( 0 0 96 ) ( 1 0 96 )")), fp);
        assert_ne!(nav_fingerprint(&ROOM.replace("\"0 0 32\"", "\"64 0 32\"")), fp);
        assert_ne!(nav_fingerprint(&ROOM.replace("CLIP", "BRICK")), fp);
        // water is kept too
        assert_ne!(nav_fingerprint(&ROOM.replace("CRATE01", "!waterblue")), fp);
    }

    #[test]
    fn nav_state_tells_current_changed_none_and_unknown() {
        let t = crate::sys::make_temp_dir(&std::env::temp_dir(), "ls-buildlog-nav-").unwrap();
        let book = t.join("book.json");
        let maps = t.join("maps");
        std::fs::create_dir_all(&maps).unwrap();
        let bsp = maps.join("qa_room.bsp");
        std::fs::write(&bsp, b"bsp v1").unwrap();
        // no nav yet
        assert_eq!(nav_state(&book, "qa_room", "fp1", nav_beside(&bsp, "qa_room").as_deref())["state"], "none");
        // a nav from elsewhere (Valve's, another tool): unknown
        std::fs::write(maps.join("QA_Room.nav"), b"nav from elsewhere").unwrap();
        let nav = nav_beside(&bsp, "qa_room").expect("any case");
        assert_eq!(nav_state(&book, "qa_room", "fp1", Some(&nav))["state"], "unknown");
        // compiled here, then learned from that .bsp: current for that layout, changed for another
        note_compiled(&book, "qa_room", "fp1", &bsp);
        std::fs::write(&nav, b"learned nav").unwrap();
        note_learned(&book, "qa_room", &bsp, &nav);
        let s = nav_state(&book, "qa_room", "fp1", Some(&nav));
        assert_eq!(s["state"], "current");
        assert!(s["learnedAt"].as_u64().unwrap() > 0);
        assert_eq!(nav_state(&book, "QA_ROOM", "fp2", Some(&nav))["state"], "changed");
        // compiled again (a new .bsp) and learned before the book knew: unknown, never "current"
        std::fs::write(&bsp, b"bsp v2").unwrap();
        note_learned(&book, "qa_room", &bsp, &nav);
        assert_eq!(nav_state(&book, "qa_room", "fp1", Some(&nav))["state"], "unknown");
        // the nav replaced by hand after learning: unknown
        note_compiled(&book, "qa_room", "fp3", &bsp);
        note_learned(&book, "qa_room", &bsp, &nav);
        assert_eq!(nav_state(&book, "qa_room", "fp3", Some(&nav))["state"], "current");
        std::fs::write(&nav, b"someone else's nav").unwrap();
        assert_eq!(nav_state(&book, "qa_room", "fp3", Some(&nav))["state"], "unknown");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn logs_are_kept_per_map_and_replaced() {
        let t = crate::sys::make_temp_dir(&std::env::temp_dir(), "ls-buildlog-logs-").unwrap();
        let d = t.as_path();
        assert_eq!(last(d, "qa_room"), json!({"compile": null, "playtest": null}));
        let p = keep(d, "qa_room", "compile", "first", json!({"outcome": "failed", "error": "leak"})).unwrap();
        keep(d, "qa_room", "compile", "second", json!({"outcome": "ok"})).unwrap();
        keep(d, "qa_room", "playtest", "steps", json!({"outcome": "ok"})).unwrap();
        keep(d, "other_map", "compile", "other", json!({"outcome": "stopped"})).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "second");
        let l = last(d, "QA_Room");
        assert_eq!(l["compile"]["outcome"], "ok");
        assert_eq!(l["compile"]["map"], "qa_room", "the name it was kept under");
        assert_eq!(l["playtest"]["outcome"], "ok");
        assert!(l["compile"]["when"].as_u64().unwrap() > 0);
        assert_eq!(log_path(d, "qa_room", "playtest").unwrap(), d.join("qa_room/playtest.txt"));
        // never outside the logs folder, never another file
        assert!(keep(d, "../x", "compile", "", json!({})).is_err());
        assert!(keep(d, "qa_room", "secrets", "", json!({})).is_err());
        assert!(log_path(d, "qa_room", "../../x").is_none());
        // a log deleted by hand: no summary pointing at nothing
        std::fs::remove_file(d.join("other_map/compile.txt")).unwrap();
        assert_eq!(last(d, "other_map")["compile"], Value::Null);
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn the_learner_console_without_the_graphics_report() {
        let text = "Protocol version 48\nERROR: Failed to load 'maps/x.nav' file navigation map!\n\tNavigation file not found or access denied.\n[mvk-info] MoltenVK version 1.2.10\n\tThe following 111 Vulkan extensions are supported:\n\tVK_KHR_16bit_storage v1\n\t\tGPU Family Metal 3\nMaster server communication disabled.\n";
        let c = learner_console(text);
        assert!(c.contains("Navigation file not found"), "a tab line of the server's own is kept");
        assert!(!c.contains("Vulkan") && !c.contains("GPU Family") && !c.contains("mvk"), "{c}");
        assert!(c.contains("Master server communication disabled."));
        assert_eq!(duration(42), "42 s");
        assert_eq!(duration(185), "3 min 5 s");
        assert_eq!(duration(3720), "1 h 2 min");
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
    }
}
