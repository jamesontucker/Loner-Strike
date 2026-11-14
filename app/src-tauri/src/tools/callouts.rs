//! The callout editor (Tools, Beta): names places on a map's bot nav ("Catwalk", "Apartments"),
//! the names bots say on the radio and that bot chat and voice commands understand
//! (gm_botchat turns "UpperTunnel" into "upper tunnel", knows "cat" and "apps", and so on).
//!
//! - `GET  /api/tools/callouts/:name`          the nav's areas (top-down boxes) and places
//! - `GET  /api/tools/callouts/:name/suggest?from=usual|other|marks`  names to try: the usual
//!   callouts for a well-known map (navpack/callouts.py's rules), the names on the same
//!   map's nav in the other game folder (Condition Zero's hand-named ones; navpack/names.py),
//!   or names from the map's own zones and spawns (BombsiteA, CTSpawn, ...) for unnamed areas
//! - `POST /api/tools/callouts/:name`          `{stamp, changes: [{id, place}]}` saves
//!
//! Only place names change: area ids, corners and links stay as they are, so the bots' map
//! knowledge (.lsv, keyed to ids and centres) still fits. The nav is written to `.part`,
//! read back and checked, then renamed over the old one (kept once as `.nav.bak`).

use super::doctor::{map_in_use, save_file, IN_USE};
use super::mapinfo::MapFile;
use super::{blocking, find_map, json_answer, locate, Done, Fail};
use crate::goldsrc::{lsv, nav};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const NO_NAV: &str = "This map has no bot nav yet, so there's nothing to name. Use \"Teach bots this map\" in the map doctor first.";
const CHANGED: &str = "The nav changed on disk since you opened it (bots may have learned the map again). Reload to see it, then make your changes again.";
/// The most places a nav may have (the game reads a 16-bit count; real maps have 10 to 40).
const MAX_PLACES: usize = 250;

/// The places bots have a voice line for (BotChatter.db "Place" entries, CS 1.6 and CZ): a
/// fallback when the game's own file can't be read.
const VOICE: &[&str] = &[
    "BombsiteA", "BombsiteB", "BombsiteC", "Hostages", "HostageRescueZone", "VipRescueZone", "CTSpawn", "TSpawn", "Bridge", "Middle", "House",
    "Apartment", "Apartments", "Market", "Sewers", "Tunnel", "Ducts", "Village", "Roof", "Upstairs", "Downstairs", "Basement", "Crawlspace",
    "Kitchen", "Inside", "Outside", "Tower", "WineCellar", "Garage", "Courtyard", "Water", "FrontDoor", "BackDoor", "SideDoor", "BackWay",
    "FrontYard", "BackYard", "SideYard", "Lobby", "Vault", "Elevator", "DoubleDoors", "SecurityDoors", "LongHall", "SideHall", "FrontHall",
    "BackHall", "MainHall", "FarSide", "Windows", "Window", "Attic", "StorageRoom", "ProjectorRoom", "MeetingRoom", "ConferenceRoom",
    "ComputerRoom", "BigOffice", "LittleOffice", "Dumpster", "Airplane", "Underground", "Bunker", "Mines", "Front", "Back", "Rear", "Side",
    "Ramp", "Underpass", "Overpass", "Stairs", "Ladder", "Gate", "GateHouse", "LoadingDock", "GuardHouse", "Entrance", "VendingMachines",
    "Loft", "Balcony", "Alley", "BackAlley", "SideAlley", "FrontRoom", "BackRoom", "SideRoom", "Crates", "Truck", "Bedroom", "FamilyRoom",
    "Bathroom", "LivingRoom", "Den", "Office", "Atrium", "Entryway", "Foyer", "Stairwell", "Fence", "Deck", "Porch", "Patio", "Wall",
];

/// Callouts players use that bot chat knows (gm_botchat casual_place and its aliases), beyond
/// the voice list: bots write these in chat but have no voice line for them.
const CHAT: &[&str] = &[
    "Catwalk", "Long", "Short", "Pit", "OutsideLong", "LongDoors", "TopofMid", "MidDoors", "UpperTunnel", "LowerTunnel", "Tunnels", "ARamp",
    "Banana", "Library", "Boiler", "Arch", "Hut", "Secret", "Vents", "Tracks", "Heaven", "Hell", "Connector", "Jungle", "Palace", "Construction",
];

// ---- names ------------------------------------------------------------------------------

/// A place name the game and bot chat both take: a letter, then letters, digits or `_`
/// (bot chat reads `_` and capitals as word breaks), at most 31 characters.
pub fn valid_place(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty() && b.len() <= 31 && b[0].is_ascii_alphabetic() && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

/// The places in a BotChatter.db with a voice line (a `.wav` before the entry's `End`): not
/// the ones added for names of our own (teach_places), nor Loner-Strike's CS 1.6 file's.
fn spoken_places(text: &str) -> Vec<String> {
    let (mut out, mut open): (Vec<String>, Option<&str>) = (Vec::new(), None);
    for l in text.lines().map(str::trim) {
        let mut w = l.split_whitespace();
        match (w.next(), w.next(), open) {
            (Some("Place"), Some(n), None) if n != "=" && valid_place(n) => open = Some(n),
            (Some("End"), None, _) => open = None,
            (_, _, Some(n)) if l.to_ascii_lowercase().ends_with(".wav") => {
                if !out.iter().any(|o: &String| o == n) {
                    out.push(n.to_string());
                }
            }
            _ => {}
        }
    }
    out
}

/// The voice list from the game's BotChatter.db (places with a voice line), else the built-in one.
fn voice_places(hl: &Path, game: &str) -> Vec<String> {
    for g in [game, "cstrike", "czero"] {
        if let Ok(text) = crate::sys::read_latin1(&hl.join(g).join("BotChatter.db")) {
            let v = spoken_places(&text);
            if v.len() >= 10 {
                return v;
            }
        }
    }
    VOICE.iter().map(|s| s.to_string()).collect()
}

/// Sets areas' place names (`""` clears one) and rebuilds the place list: names still in use
/// keep their order, new ones follow in the order they're first used; unused ones go.
/// Returns how many areas changed. Unknown ids and bad names are errors (nothing changes).
pub fn apply_changes(n: &mut nav::Nav, changes: &[(u32, String)]) -> Result<usize, String> {
    let idx = n.index_of();
    let mut names: Vec<String> = n.areas.iter().map(|a| n.place_name(a).to_string()).collect();
    let mut changed = 0;
    for (id, place) in changes {
        let Some(&i) = idx.get(id) else { return Err(format!("There's no area {id} on this nav.")) };
        if !place.is_empty() && !valid_place(place) {
            return Err(format!("\"{place}\" can't be a place name: use letters and numbers (and _), starting with a letter, up to 31."));
        }
        if names[i] != *place {
            names[i] = place.clone();
            changed += 1;
        }
    }
    let mut places: Vec<String> = n.places.iter().filter(|p| names.iter().any(|x| x == *p)).cloned().collect();
    for x in &names {
        if !x.is_empty() && !places.contains(x) {
            places.push(x.clone());
        }
    }
    if places.len() > MAX_PLACES {
        return Err(format!("That's {} places; a nav can hold {MAX_PLACES}.", places.len()));
    }
    for (a, x) in n.areas.iter_mut().zip(&names) {
        a.place = if x.is_empty() { 0 } else { places.iter().position(|p| p == x).map(|i| i as u16 + 1).unwrap_or(0) };
    }
    n.places = places;
    Ok(changed)
}

/// The nav written and read back: everything but the names must come back the same, and
/// the brain's key with it (so its map knowledge still fits).
pub fn write_checked(before: &nav::Nav, after: &nav::Nav) -> Result<Vec<u8>, String> {
    let bytes = after.write();
    let back = nav::Nav::parse(&bytes).map_err(|e| format!("The new nav didn't read back ({e}). Nothing was saved."))?;
    let same = back.areas.len() == before.areas.len()
        && back.areas.iter().zip(&before.areas).all(|(a, b)| a.id == b.id && a.lo == b.lo && a.hi == b.hi && a.conns == b.conns && a.hiding == b.hiding)
        && back.areas.iter().zip(&after.areas).all(|(a, b)| back.place_name(a) == after.place_name(b));
    if !same || lsv::nav_key(&back) != lsv::nav_key(before) {
        return Err("The new nav didn't come back the same. Nothing was saved.".into());
    }
    Ok(bytes)
}

// ---- the game's place list --------------------------------------------------------------

/// ReGameDLL keeps a nav's place name only when the game's BotChatter.db has a `Place <Name>`
/// entry for it (nav_file.cpp PlaceDirectory::Load -> BotPhraseManager::NameToID, matched
/// without case): any other name is read as no place. Bots then never say it, the Location
/// message (where someone is) never carries it, and bot chat never hears it. So every name a
/// nav uses goes into BotChatter.db as an entry of its own with no voice line: the radio skips
/// the place (the rest of the line is said), and the Location message and bot chat use it.
pub const CHATTER: &str = "BotChatter.db";
/// (only under test: studio::guard_real_game lets everything through otherwise)
const UNDER_TEST: &str = "Under test, Callouts doesn't save into your real game. Point LS_TEST_GAME at a copy (a sim server) for that.";

/// The `Place <Name>` entries in a BotChatter.db (not `Place = <Name>` conditions inside a
/// phrase), lower case.
fn chatter_places(text: &str) -> std::collections::HashSet<String> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            (w.next()? == "Place").then(|| w.next()).flatten().filter(|n| *n != "=").map(|n| n.to_ascii_lowercase())
        })
        .collect()
}

/// The text to append to a BotChatter.db so it lists `names` (`None`: it does already).
pub fn chatter_additions(text: &str, names: &[String]) -> Option<(String, Vec<String>)> {
    let have = chatter_places(text);
    let mut add: Vec<String> = Vec::new();
    for n in names {
        if valid_place(n) && !have.contains(&n.to_ascii_lowercase()) && !add.iter().any(|a| a.eq_ignore_ascii_case(n)) {
            add.push(n.clone());
        }
    }
    if add.is_empty() {
        return None;
    }
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        out.push_str(nl);
    }
    out.push_str(&format!("{nl}// Place names from Loner-Strike's callout editor (no voice line: bots say them in chat){nl}"));
    for n in &add {
        out.push_str(&format!("Place {n}{nl}End{nl}{nl}"));
    }
    Some((out, add))
}

/// Makes `<game_dir>/BotChatter.db` list `names`. Ok(the names added). Err: there's no such
/// file (the game then keeps only its built-in names), or it couldn't be written. `history`:
/// keep the old file in the app's History first (the player's own game; not a server's copy).
pub fn teach_places(game_dir: &Path, names: &[String], history: bool) -> Result<Vec<String>, String> {
    // (under test: never into a real Half-Life install; LS_TEST_GAME points at a copy)
    super::studio::guard_real_game(game_dir).map_err(|_| UNDER_TEST.to_string())?;
    let p = game_dir.join(CHATTER);
    let Ok(bytes) = std::fs::read(&p) else { return Err(format!("There's no {CHATTER} in {}.", game_dir.display())) };
    let text: String = bytes.iter().map(|&c| c as char).collect();
    let Some((more, added)) = chatter_additions(&text, names) else { return Ok(Vec::new()) };
    if history {
        crate::safety::before("Callouts", &format!("Added {} to {CHATTER}", added.join(", ")), &p);
    }
    let mut out = bytes;
    out.extend(more.bytes());
    let part = p.with_file_name(format!("{CHATTER}.part"));
    crate::sys::make_writable(&p);
    std::fs::write(&part, &out).and_then(|_| crate::sys::rename_over(&part, &p)).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        format!("{CHATTER} couldn't be saved ({e}).")
    })?;
    Ok(added)
}

/// The place names in a nav file's place table (it's at the start: only that much is read).
pub fn nav_place_names(nav_file: &Path) -> Vec<String> {
    use std::io::Read;
    let mut b = Vec::new();
    if std::fs::File::open(nav_file).and_then(|f| f.take(64 * 1024).read_to_end(&mut b)).is_err() {
        return Vec::new();
    }
    let u16_at = |i: usize| b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as usize);
    let u32_at = |i: usize| b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    if u32_at(0) != Some(nav::NAV_MAGIC) || u32_at(4).is_none_or(|v| v < 5) {
        return Vec::new();
    }
    let Some(count) = u16_at(12) else { return Vec::new() };
    let (mut at, mut out) = (14, Vec::new());
    for _ in 0..count {
        let Some(len) = u16_at(at).filter(|l| (1..=256).contains(l)) else { break };
        let Some(raw) = b.get(at + 2..at + 2 + len) else { break };
        out.push(raw[..len - 1].iter().take_while(|&&c| c != 0).map(|&c| c as char).collect());
        at += 2 + len;
    }
    out
}

/// A server got a nav (navs.rs): its game's BotChatter.db learns the nav's place names, so the
/// game keeps them. Quiet: a server without the file keeps the game's own names, as before.
pub fn teach_server(server_dir: &Path, game: &str, nav_file: &Path) {
    let names = nav_place_names(nav_file);
    if !names.is_empty() {
        let _ = teach_places(&server_dir.join(game), &names, false);
    }
}

// ---- suggestions ------------------------------------------------------------------------

/// A rule: (new name, names it may replace (empty: any), x, y, z ranges as (lo, hi), either
/// end open). The first rule that fits an area wins (navpack/callouts.py).
type Range = Option<(Option<f32>, Option<f32>)>;
struct Rule {
    name: &'static str,
    olds: &'static [&'static str],
    x: Range,
    y: Range,
    z: Range,
}

const fn rule(name: &'static str, olds: &'static [&'static str], x: Range, y: Range, z: Range) -> Rule {
    Rule { name, olds, x, y, z }
}

fn rules(map: &str) -> &'static [Rule] {
    const DUST2: &[Rule] = &[
        rule("OutsideLong", &["SideDoor"], None, None, None),
        rule("Pit", &["Side"], None, Some((None, Some(650.0))), Some((None, Some(-48.0)))),
        rule("Long", &["Side"], None, None, None),
        rule("Catwalk", &["Middle"], Some((Some(100.0), None)), Some((Some(1450.0), None)), None),
        rule("MidDoors", &["DoubleDoors"], None, None, None),
        rule("UpperTunnel", &["Tunnel"], None, None, Some((Some(16.0), None))),
        rule("LowerTunnel", &["Tunnel"], None, None, Some((None, Some(16.0)))),
    ];
    const INFERNO: &[Rule] = &[rule("Banana", &["BackHall"], None, None, None), rule("Apartments", &["Upstairs"], None, None, None)];
    match map.to_lowercase().as_str() {
        "de_dust2" => DUST2,
        "de_inferno" => INFERNO,
        _ => &[],
    }
}

fn inside(v: f32, r: Range) -> bool {
    r.is_none_or(|(lo, hi)| lo.is_none_or(|l| v >= l) && hi.is_none_or(|h| v < h))
}

/// An area's middle and its average floor height (as the python tools take it).
fn mid(a: &nav::Area) -> (f32, f32, f32) {
    ((a.lo[0] + a.hi[0]) / 2.0, (a.lo[1] + a.hi[1]) / 2.0, (a.lo[2] + a.hi[2] + a.ne_z + a.sw_z) / 4.0)
}

/// Names from the map's own zones and spawns, for areas without a name: bomb sites (BombsiteA,
/// BombsiteB... in the game's order), the hostage rescue zone, VIP escape, the CT and T spawns
/// (the box round each team's spawn points) and hostages. Zones first, then spawns.
pub fn names_from_marks(n: &nav::Nav, marks: &[super::overview::Mark]) -> Vec<(u32, String)> {
    // (name, x0, y0, x1, y1, z0, z1): a nav area whose middle is inside gets the name
    let mut boxes: Vec<(String, [f32; 6])> = Vec::new();
    for m in marks {
        let name = match (m.kind, m.label.as_deref()) {
            ("bomb", Some(l)) => format!("Bombsite{l}"),
            ("rescue", _) => "HostageRescueZone".into(),
            ("vip", _) => "VipRescueZone".into(),
            _ => continue,
        };
        boxes.push((name, [m.lo[0] - 16.0, m.lo[1] - 16.0, m.hi[0] + 16.0, m.hi[1] + 16.0, m.lo[2] - 64.0, m.hi[2] + 64.0]));
    }
    for (kind, name) in [("ct", "CTSpawn"), ("t", "TSpawn")] {
        let pts: Vec<&[f32; 3]> = marks.iter().filter(|m| m.kind == kind).map(|m| &m.lo).collect();
        if pts.is_empty() {
            continue;
        }
        let f = |i: usize, lo: bool| pts.iter().map(|p| p[i]).fold(if lo { f32::MAX } else { f32::MIN }, |a, b| if lo { a.min(b) } else { a.max(b) });
        // spawn points stand 36 units over the floor
        boxes.push((name.into(), [f(0, true) - 160.0, f(1, true) - 160.0, f(0, false) + 160.0, f(1, false) + 160.0, f(2, true) - 100.0, f(2, false) + 24.0]));
    }
    for m in marks.iter().filter(|m| m.kind == "hostage") {
        boxes.push(("Hostages".into(), [m.lo[0] - 96.0, m.lo[1] - 96.0, m.lo[0] + 96.0, m.lo[1] + 96.0, m.lo[2] - 100.0, m.lo[2] + 40.0]));
    }
    let mut out = Vec::new();
    for a in &n.areas {
        if a.place != 0 {
            continue;
        }
        let (x, y, z) = mid(a);
        if let Some((name, _)) = boxes.iter().find(|(_, b)| x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3] && z >= b[4] && z <= b[5]) {
            out.push((a.id, name.clone()));
        }
    }
    out
}

/// The usual callouts for this map: changes the rules make.
pub fn usual_callouts(map: &str, n: &nav::Nav) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for a in &n.areas {
        let now = n.place_name(a);
        let (x, y, z) = mid(a);
        if let Some(r) = rules(map).iter().find(|r| (r.olds.is_empty() || r.olds.contains(&now)) && inside(x, r.x) && inside(y, r.y) && inside(z, r.z)) {
            if r.name != now {
                out.push((a.id, r.name.to_string()));
            }
        }
    }
    out
}

/// Names from another nav of the same map (navpack/names.py): each area takes the place of
/// the named area under its middle (nearest in height), else of the nearest named middle
/// within `max_dist`; then an unnamed area whose named neighbours all agree takes their
/// name, until nothing changes. Only areas without a name now get one (a name someone gave an
/// area is better than one carried over).
pub fn names_from(target: &nav::Nav, source: &nav::Nav, max_dist: f32) -> Vec<(u32, String)> {
    let src: Vec<(&nav::Area, &str)> = source.areas.iter().map(|a| (a, source.place_name(a))).filter(|(_, p)| !p.is_empty()).collect();
    let mut new: Vec<String> = target
        .areas
        .iter()
        .map(|a| {
            let (x, y, z) = mid(a);
            let mut best: Option<(f32, &str)> = None;
            for (b, p) in &src {
                let (lx, hx) = (b.lo[0].min(b.hi[0]), b.lo[0].max(b.hi[0]));
                let (ly, hy) = (b.lo[1].min(b.hi[1]), b.lo[1].max(b.hi[1]));
                if lx - 8.0 <= x && x <= hx + 8.0 && ly - 8.0 <= y && y <= hy + 8.0 {
                    let dz = (mid(b).2 - z).abs();
                    if dz < 48.0 && best.is_none_or(|(d, _)| dz < d) {
                        best = Some((dz, p));
                    }
                }
            }
            if best.is_none() {
                for (b, p) in &src {
                    let (bx, by, bz) = mid(b);
                    let d = (bx - x).hypot(by - y);
                    if d < max_dist && (bz - z).abs() < 48.0 && best.is_none_or(|(e, _)| d < e) {
                        best = Some((d, p));
                    }
                }
            }
            best.map(|(_, p)| p.to_string()).unwrap_or_default()
        })
        .collect();
    // the gaps: neighbours (either way) all agreeing on one place
    let idx = target.index_of();
    let mut nb: Vec<Vec<usize>> = vec![Vec::new(); target.areas.len()];
    for (i, a) in target.areas.iter().enumerate() {
        for c in a.conns.iter().flatten() {
            if let Some(&j) = idx.get(c) {
                nb[i].push(j);
                nb[j].push(i);
            }
        }
    }
    loop {
        let mut changed = false;
        for i in 0..new.len() {
            if !new[i].is_empty() {
                continue;
            }
            let mut ps: Vec<&String> = nb[i].iter().map(|&j| &new[j]).filter(|p| !p.is_empty()).collect();
            ps.sort();
            ps.dedup();
            if ps.len() == 1 {
                new[i] = ps[0].clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    target.areas.iter().zip(new).filter(|(a, p)| !p.is_empty() && a.place == 0).map(|(a, p)| (a.id, p)).collect()
}

// ---- reading ----------------------------------------------------------------------------

fn load_nav(p: &Path) -> Done<nav::Nav> {
    Ok(nav::Nav::parse(&crate::goldsrc::read_capped(p, "navigation file", nav::MAX_NAV)?)?)
}

fn nav_path(m: &MapFile) -> Done<PathBuf> {
    m.sibling("nav").ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, NO_NAV))
}

/// A stamp of the nav file (size and time), so a save can tell it changed underneath.
fn stamp(p: &Path) -> String {
    super::cache::Stamp::of(p).map(|s| s.key()).unwrap_or_default()
}

/// The same map's nav in the other game folder, when it has place names.
fn other_nav(m: &MapFile) -> Option<(String, PathBuf)> {
    let other = if m.game == "czero" { "cstrike" } else { "czero" };
    let p = crate::mapcopy::Finder::default().find(&m.hl.join(other), &format!("maps/{}.nav", m.name))?;
    let n = load_nav(&p).ok()?;
    n.areas.iter().any(|a| a.place != 0).then(|| (other.to_string(), p))
}

/// Whether the knowledge next to the map fits this nav (None: there's none, or it can't be read).
fn knowledge_fits(m: &MapFile, n: &nav::Nav) -> Option<bool> {
    let p = m.sibling("lsv")?;
    let mut b = [0u8; 16];
    use std::io::Read;
    std::fs::File::open(p).ok()?.read_exact(&mut b).ok()?;
    let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    (u(0) == lsv::LSV_MAGIC).then(|| u(8) == lsv::nav_key(n) && u(12) as usize == n.areas.len())
}

/// The page's view of the nav: places, and each area as `[id, x0, y0, x1, y1, z, place]`
/// (corners sorted, z the average floor height, place 1-based into `places`, 0 none).
pub fn view(m: &MapFile, n: &nav::Nav, nav_file: &Path) -> Value {
    let areas: Vec<Value> = n
        .areas
        .iter()
        .map(|a| {
            let (_, _, z) = mid(a);
            let r = |v: f32| (v * 10.0).round() / 10.0;
            json!([a.id, r(a.lo[0].min(a.hi[0])), r(a.lo[1].min(a.hi[1])), r(a.lo[0].max(a.hi[0])), r(a.lo[1].max(a.hi[1])), r(z), a.place])
        })
        .collect();
    let size = std::fs::metadata(&m.bsp).ok().map(|x| x.len());
    json!({
        "map": m.name, "game": m.game,
        "file": nav_file.file_name().map(|f| f.to_string_lossy().to_string()),
        "version": n.version,
        "matchesMap": size.map(|s| n.version < 4 || s == n.bsp_size as u64),
        "places": n.places, "areas": areas,
        "voice": voice_places(&m.hl, &m.game), "chat": CHAT,
        "knowledge": knowledge_fits(m, n),
        "stamp": stamp(nav_file),
        "usual": !rules(&m.name).is_empty(),
        "other": other_nav(m).map(|(g, _)| g),
        "backup": nav_file.with_file_name(format!("{}.bak", nav_file.file_name().unwrap_or_default().to_string_lossy())).is_file(),
    })
}

fn changes_json(c: &[(u32, String)]) -> Value {
    Value::Array(c.iter().map(|(id, p)| json!({"id": id, "place": p})).collect())
}

/// The player's game learns the nav's names (CS 1.6 and Condition Zero each read their own
/// BotChatter.db, and Condition Zero plays CS 1.6's maps too). Words for the save message.
fn teach_game(m: &MapFile, places: &[String]) -> String {
    let mut added: Vec<String> = Vec::new();
    let mut found = false;
    for g in ["cstrike", "czero"] {
        let dir = m.hl.join(g);
        if !dir.join(CHATTER).is_file() {
            continue;
        }
        found = true;
        match teach_places(&dir, places, true) {
            Ok(a) => {
                for n in a {
                    if !added.contains(&n) {
                        added.push(n);
                    }
                }
            }
            Err(e) => return format!(" {e} The game won't know the new names until it can be."),
        }
    }
    if !found {
        let known = voice_places(&m.hl, &m.game);
        if places.iter().all(|p| known.iter().any(|v| v.eq_ignore_ascii_case(p))) {
            return String::new();
        }
        return format!(" This game has no {CHATTER}, so playing here it keeps only the standard place names (servers learn the new ones with the nav).");
    }
    match added.len() {
        0 => String::new(),
        1 => format!(" {} is new to the game, so it's added to its place list ({CHATTER}): the game reads it when it next starts.", added[0]),
        _ => format!(" {} are new to the game, so they're added to its place list ({CHATTER}): the game reads them when it next starts.", crate::navs::names(&added)),
    }
}

/// Saves `changes` to the map's nav. `seen`: the stamp the page loaded.
pub fn save(m: &MapFile, seen: &str, changes: &[(u32, String)]) -> Result<Value, String> {
    super::studio::guard_real_game(&m.hl).map_err(|_| UNDER_TEST.to_string())?;
    let p = m.sibling("nav").ok_or(NO_NAV)?;
    if stamp(&p) != seen {
        return Err(CHANGED.into());
    }
    let before = load_nav(&p).map_err(|f| f.msg)?;
    let fit_before = knowledge_fits(m, &before);
    let mut after = before.clone();
    let changed = apply_changes(&mut after, changes)?;
    if changed == 0 {
        return Ok(json!({"ok": true, "changed": 0, "message": "Nothing to save: the names are the same.", "stamp": stamp(&p)}));
    }
    let bytes = write_checked(&before, &after)?;
    let backed = save_file(&p, &bytes).map_err(|e| format!("The nav couldn't be saved in the maps folder ({e})."))?;
    let mut msg = format!("Saved {changed} area name{}.", if changed == 1 { "" } else { "s" });
    if before.version < nav::NAV_VERSION {
        msg.push_str(&format!(" The nav was an old version ({}); it's saved as version 5, which the game reads.", before.version));
    }
    msg.push_str(match fit_before {
        Some(true) => " The bots' map knowledge still fits: it's tied to the areas, not their names.",
        Some(false) => " The bots' map knowledge was made for a different nav already; they work it out again the next time the map is played with bots.",
        None => "",
    });
    if backed {
        msg.push_str(" The old nav is kept next to it as .nav.bak.");
    }
    msg.push_str(&teach_game(m, &after.places));
    msg.push_str(" Servers get the new names with the next sync.");
    Ok(json!({"ok": true, "changed": changed, "message": msg, "stamp": stamp(&p), "knowledge": knowledge_fits(m, &after)}))
}

// ---- routes -----------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn api_get(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let p = nav_path(&m)?;
                let n = load_nav(&p)?;
                let mut v = view(&m, &n, &p);
                v["inUse"] = json!(map_in_use(&game, &m.name));
                Ok(v)
            })
            .await
        }
    };
    json_answer(r)
}

#[derive(Deserialize)]
struct SuggestQuery {
    from: Option<String>,
}

async fn api_suggest(State(st): State<Shared>, UrlPath(name): UrlPath<String>, q: Option<Query<SuggestQuery>>) -> Response {
    let from = q.and_then(|q| q.0.from).unwrap_or_default();
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let n = load_nav(&nav_path(&m)?)?;
                let (changes, words) = match from.as_str() {
                    "usual" => (usual_callouts(&m.name, &n), "the usual callouts for this map"),
                    "other" => {
                        let (g, p) = other_nav(&m).ok_or_else(|| Fail::new(StatusCode::OK, "There's no named nav for this map in the other game folder."))?;
                        let src = load_nav(&p)?;
                        let words = if g == "czero" { "Condition Zero's nav of this map" } else { "CS 1.6's nav of this map" };
                        (names_from(&n, &src, 64.0), words)
                    }
                    "marks" => {
                        let bsp = crate::goldsrc::bsp::Bsp::open(&m.bsp, false)?;
                        let ents = crate::goldsrc::entities::parse(&bsp.entities);
                        (names_from_marks(&n, &super::overview::find_marks(&bsp, &ents)), "the map's bomb sites, spawns and zones")
                    }
                    _ => return Err(Fail::new(StatusCode::BAD_REQUEST, "Ask for from=usual, from=other or from=marks.")),
                };
                let msg = if changes.is_empty() {
                    if from == "marks" {
                        format!("Nothing to add: no unnamed area is in {words}.")
                    } else if from == "usual" {
                        format!("Nothing to change: the names already match {words}.")
                    } else {
                        format!("Nothing to add: the areas {words} can name have names already.")
                    }
                } else {
                    format!("{} area{} named from {words}. Look them over, then save.", changes.len(), if changes.len() == 1 { "" } else { "s" })
                };
                Ok(json!({"changes": changes_json(&changes), "message": msg}))
            })
            .await
        }
    };
    json_answer(r)
}

#[derive(Deserialize)]
struct Change {
    id: u32,
    place: String,
}

#[derive(Deserialize)]
struct SaveReq {
    stamp: String,
    changes: Vec<Change>,
}

async fn api_save(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>, Json(req): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                if map_in_use(&game, &m.name) {
                    return Err(Fail::new(StatusCode::CONFLICT, IN_USE));
                }
                let changes: Vec<(u32, String)> = req.changes.into_iter().take(100_000).map(|c| (c.id, c.place.trim().to_string())).collect();
                save(&m, &req.stamp, &changes).map_err(|e| Fail::new(StatusCode::OK, e))
            })
            .await
        }
    };
    json_answer(r)
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/callouts/:name", get(api_get).post(api_save))
        .route("/api/tools/callouts/:name/suggest", get(api_suggest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(id: u32, x: f32, y: f32, z: f32, place: u16) -> nav::Area {
        nav::Area { id, attr: 0, lo: [x, y, z], hi: [x + 100.0, y + 100.0, z], ne_z: z, sw_z: z, conns: Default::default(), hiding: vec![], approach: vec![], encounters: vec![], place }
    }

    /// A row of four areas, linked in a line: Side, Side, (none), Tunnel.
    fn row() -> nav::Nav {
        let mut a: Vec<nav::Area> = (0..4).map(|i| area(10 + i, i as f32 * 100.0, 0.0, 0.0, [1, 1, 0, 2][i as usize])).collect();
        for i in 0..3 {
            a[i].conns[1] = vec![11 + i as u32];
            a[i + 1].conns[3] = vec![10 + i as u32];
        }
        nav::Nav { version: 5, bsp_size: 1234, places: vec!["Side".into(), "Tunnel".into(), "Unused".into()], areas: a }
    }

    fn names(n: &nav::Nav) -> Vec<&str> {
        n.areas.iter().map(|a| n.place_name(a)).collect()
    }

    #[test]
    fn names_from_the_maps_zones_and_spawns() {
        use super::super::overview::Mark;
        // four areas at x 0..400: a bomb zone over the first, CT spawns over the third, the
        // second is named already, the last is left alone
        let mut n = row();
        n.areas[0].place = 0;
        let marks = vec![
            Mark { kind: "bomb", lo: [10.0, 10.0, -10.0], hi: [90.0, 90.0, 60.0], label: Some("A".into()) },
            Mark { kind: "bomb", lo: [100.0, 0.0, 0.0], hi: [200.0, 100.0, 60.0], label: Some("B".into()) },
            Mark { kind: "ct", lo: [250.0, 50.0, 36.0], hi: [250.0, 50.0, 36.0], label: None },
            Mark { kind: "t", lo: [5000.0, 0.0, 36.0], hi: [5000.0, 0.0, 36.0], label: None },
        ];
        let c = names_from_marks(&n, &marks);
        assert_eq!(c, vec![(10, "BombsiteA".to_string()), (12, "CTSpawn".to_string())]);
    }

    #[test]
    fn rename_round_trip() {
        let before = row();
        let mut after = before.clone();
        // rename "Side" to "Long" (both its areas), name the gap, clear the tunnel
        let changed = apply_changes(&mut after, &[(10, "Long".into()), (11, "Long".into()), (12, "Catwalk".into()), (13, "".into())]).unwrap();
        assert_eq!(changed, 4);
        assert_eq!(names(&after), ["Long", "Long", "Catwalk", ""]);
        // unused names go; new ones follow in first-use order
        assert_eq!(after.places, ["Long", "Catwalk"]);
        let bytes = write_checked(&before, &after).unwrap();
        let back = nav::Nav::parse(&bytes).unwrap();
        assert_eq!(names(&back), ["Long", "Long", "Catwalk", ""]);
        assert_eq!(lsv::nav_key(&back), lsv::nav_key(&before));
        assert_eq!(back.areas.iter().map(|a| (a.id, a.lo, a.hi, a.conns.clone())).collect::<Vec<_>>(), before.areas.iter().map(|a| (a.id, a.lo, a.hi, a.conns.clone())).collect::<Vec<_>>());
        // and back again
        let mut again = back.clone();
        apply_changes(&mut again, &[(10, "Side".into()), (11, "Side".into()), (12, "".into()), (13, "Tunnel".into())]).unwrap();
        assert_eq!(names(&again), names(&before));
        // existing names keep their order
        assert_eq!(again.places, ["Side", "Tunnel"]);
    }

    #[test]
    fn bad_changes_are_refused() {
        let mut n = row();
        assert!(apply_changes(&mut n, &[(99, "Long".into())]).unwrap_err().contains("no area 99"));
        for bad in ["9lives", "has space", "x".repeat(32).as_str(), "semi;colon", "ümlaut"] {
            assert!(apply_changes(&mut n, &[(10, bad.into())]).is_err(), "{bad}");
        }
        assert_eq!(names(&n), names(&row()));
        assert!(valid_place("BombsiteA") && valid_place("Long_A") && valid_place("A"));
        // same name: nothing changes
        assert_eq!(apply_changes(&mut n, &[(10, "Side".into())]).unwrap(), 0);
    }

    #[test]
    fn usual_callouts_follow_the_rules() {
        let mut n = row();
        n.places = vec!["Side".into(), "Tunnel".into()];
        // area 13 (x 300-400) is Tunnel at z 0 (< 16): lower tunnel; the Side ones at y 0-100
        // (< 650) but z 0 isn't below -48: Long
        let c = usual_callouts("de_dust2", &n);
        assert_eq!(c, vec![(10, "Long".to_string()), (11, "Long".to_string()), (13, "LowerTunnel".to_string())]);
        n.areas[0].lo[2] = -100.0;
        n.areas[0].hi[2] = -100.0;
        n.areas[0].ne_z = -100.0;
        n.areas[0].sw_z = -100.0;
        assert_eq!(usual_callouts("DE_DUST2", &n)[0], (10, "Pit".to_string()));
        assert!(usual_callouts("de_whatever", &n).is_empty());
    }

    #[test]
    fn names_carry_over_from_another_nav() {
        // a named nav with two big areas; a generated one with smaller areas under them,
        // one out of reach that's linked only to Long areas
        let mut src = nav::Nav { version: 5, bsp_size: 1, places: vec!["Long".into(), "Pit".into()], areas: vec![area(1, 0.0, 0.0, 0.0, 1), area(2, 100.0, 0.0, 0.0, 2)] };
        src.areas[0].hi = [100.0, 100.0, 0.0];
        let mut t = nav::Nav { version: 5, bsp_size: 1, places: vec![], areas: vec![area(5, 10.0, 10.0, 0.0, 0), area(6, 120.0, 10.0, 0.0, 0), area(7, 10.0, 900.0, 0.0, 0)] };
        for a in &mut t.areas {
            a.hi = [a.lo[0] + 20.0, a.lo[1] + 20.0, 0.0];
        }
        t.areas[2].conns[0] = vec![5];
        let c = names_from(&t, &src, 64.0);
        assert_eq!(c, vec![(5, "Long".to_string()), (6, "Pit".to_string()), (7, "Long".to_string())]);
        // too high above: no name from the area, none from the nearest middle
        t.areas[1].lo[2] = 200.0;
        t.areas[1].hi[2] = 200.0;
        t.areas[1].ne_z = 200.0;
        t.areas[1].sw_z = 200.0;
        assert!(!names_from(&t, &src, 64.0).iter().any(|(id, _)| *id == 6));
    }

    #[test]
    fn tests_never_save_into_the_real_game() {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let real = home.join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life");
        let m = MapFile { name: "de_dust2".into(), bsp: real.join("cstrike/maps/de_dust2.bsp"), game: "cstrike".into(), hl: real.clone(), czero: false };
        assert!(save(&m, "", &[]).unwrap_err().contains("real game"));
        assert!(teach_places(&real.join("cstrike"), &["Zork".to_string()], false).unwrap_err().contains("real game"));
    }

    #[test]
    fn save_writes_a_backup_and_refuses_a_changed_file() {
        let root = std::env::temp_dir().join(format!("ls-callouts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        let bsp = hl.join("cstrike/maps/de_cal.bsp");
        std::fs::write(&bsp, crate::goldsrc::bsp::fixture::build()).unwrap();
        let navp = hl.join("cstrike/maps/de_cal.nav");
        let original = row().write();
        std::fs::write(&navp, &original).unwrap();
        // knowledge made for this nav
        let mut head = Vec::new();
        for v in [lsv::LSV_MAGIC, lsv::LSV_VERSION, lsv::nav_key(&row()), 4, 4] {
            head.extend(v.to_le_bytes());
        }
        std::fs::write(hl.join("cstrike/maps/de_cal.lsv"), &head).unwrap();
        let m = MapFile { name: "de_cal".into(), bsp, game: "cstrike".into(), hl: hl.clone(), czero: false };
        let n = load_nav(&navp).unwrap();
        let v = view(&m, &n, &navp);
        assert_eq!(v["areas"][0], json!([10, 0.0, 0.0, 100.0, 100.0, 0.0, 1]));
        assert_eq!(v["knowledge"], true);
        assert!(v["voice"].as_array().unwrap().len() > 50);
        let st = v["stamp"].as_str().unwrap().to_string();

        assert!(save(&m, "stale", &[(10, "Long".into())]).unwrap_err().contains("changed on disk"));
        let out = save(&m, &st, &[(10, "Long".into())]).unwrap();
        assert_eq!(out["changed"], 1);
        assert!(out["message"].as_str().unwrap().contains("still fits"), "{out}");
        assert_eq!(out["knowledge"], true);
        assert_eq!(std::fs::read(hl.join("cstrike/maps/de_cal.nav.bak")).unwrap(), original);
        assert!(!hl.join("cstrike/maps/de_cal.nav.part").exists());
        let saved = load_nav(&navp).unwrap();
        assert_eq!(saved.place_name(&saved.areas[0]), "Long");
        // a second save keeps the first backup
        let st2 = out["stamp"].as_str().unwrap().to_string();
        save(&m, &st2, &[(11, "Long".into())]).unwrap();
        assert_eq!(std::fs::read(hl.join("cstrike/maps/de_cal.nav.bak")).unwrap(), original);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_game_learns_new_place_names() {
        // a CZ-style file: places with and without voice lines, a phrase with a place condition
        let db = "BaseDir \"radio\\bot\"\r\n\r\nPlace BombsiteA\r\n\tradio\\bot\\a.wav\r\nEnd\r\n\r\nPlace Catwalkish\r\nEnd\r\n\r\nChatter Help\r\n\tPlace = Bridge\r\n\tbridge.wav\r\nEnd";
        let names: Vec<String> = ["bombsitea", "Catwalk", "ZorkBase", "Bridge", "catwalk", "not valid"].iter().map(|s| s.to_string()).collect();
        let (more, added) = chatter_additions(db, &names).unwrap();
        // known ones (any case) stay out; a "Place = X" condition isn't an entry; one of each
        assert_eq!(added, ["Catwalk", "ZorkBase", "Bridge"]);
        assert!(more.starts_with("\r\n\r\n//"), "{more:?}");
        assert!(more.contains("\r\nPlace Catwalk\r\nEnd\r\n") && more.contains("\r\nPlace ZorkBase\r\nEnd\r\n"), "{more:?}");
        let both = format!("{db}{more}");
        assert!(chatter_additions(&both, &names).is_none());
        let have = chatter_places(&both);
        for n in ["bombsitea", "catwalkish", "catwalk", "zorkbase", "bridge"] {
            assert!(have.contains(n), "{n}");
        }
        // the radio list is only the places with a voice line: not the ones added here
        assert_eq!(spoken_places(&both), ["BombsiteA"]);

        // on disk: a game folder's file, then a server getting a nav
        let root = std::env::temp_dir().join(format!("ls-callouts-chatter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("srv/cstrike/maps")).unwrap();
        assert!(teach_places(&root.join("srv/czero"), &names, false).unwrap_err().contains("There's no BotChatter.db"));
        let file = root.join("srv/cstrike/BotChatter.db");
        std::fs::write(&file, "Place BombsiteA\nEnd\n").unwrap();
        let mut n = row();
        n.places = vec!["Side".into(), "Tunnel".into(), "BombsiteA".into()];
        let navp = root.join("srv/cstrike/maps/de_x.nav");
        std::fs::write(&navp, n.write()).unwrap();
        assert_eq!(nav_place_names(&navp), ["Side", "Tunnel", "BombsiteA"]);
        teach_server(&root.join("srv"), "cstrike", &navp);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("Place BombsiteA\nEnd\n\n//"), "{text}");
        assert!(text.contains("\nPlace Side\nEnd\n") && text.contains("\nPlace Tunnel\nEnd\n") && !text.contains("Place BombsiteA\nEnd\n\nPlace BombsiteA"), "{text}");
        // a second time adds nothing
        teach_server(&root.join("srv"), "cstrike", &navp);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        // not a nav, an old nav: no names
        std::fs::write(&navp, b"nonsense").unwrap();
        assert!(nav_place_names(&navp).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_adds_new_names_to_the_games_place_list() {
        let root = std::env::temp_dir().join(format!("ls-callouts-teach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(hl.join("czero")).unwrap();
        let bsp = hl.join("cstrike/maps/de_cal.bsp");
        std::fs::write(&bsp, crate::goldsrc::bsp::fixture::build()).unwrap();
        let navp = hl.join("cstrike/maps/de_cal.nav");
        std::fs::write(&navp, row().write()).unwrap();
        let voice: String = VOICE.iter().map(|v| format!("Place {v}\nEnd\n\n")).collect();
        std::fs::write(hl.join("cstrike/BotChatter.db"), &voice).unwrap();
        std::fs::write(hl.join("czero/BotChatter.db"), &voice).unwrap();
        let m = MapFile { name: "de_cal".into(), bsp, game: "cstrike".into(), hl: hl.clone(), czero: false };
        // "Side" and "Tunnel" are in the voice list; "Catwalk" and "ZorkBase" aren't
        let out = save(&m, &stamp(&navp), &[(10, "Catwalk".into()), (12, "ZorkBase".into())]).unwrap();
        let msg = out["message"].as_str().unwrap();
        assert!(msg.contains("Catwalk and ZorkBase are new to the game"), "{msg}");
        for g in ["cstrike", "czero"] {
            let text = std::fs::read_to_string(hl.join(g).join("BotChatter.db")).unwrap();
            assert!(text.contains("\nPlace Catwalk\nEnd\n") && text.contains("\nPlace ZorkBase\nEnd\n"), "{g}: {text}");
            assert_eq!(text.matches("Place Side\n").count(), 1);
        }
        // known names only: nothing to say about the place list
        let out = save(&m, &out["stamp"].as_str().unwrap().to_string(), &[(10, "Side".into())]).unwrap();
        assert!(!out["message"].as_str().unwrap().contains("BotChatter"), "{out}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The real game: names de_dust2's cstrike nav from the czero one, without saving.
    /// `LS_TEST_GAME=<Half-Life folder> cargo test real_callouts -- --nocapture`
    #[test]
    fn real_callouts() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        for map in ["de_dust2", "de_inferno", "cs_office"] {
            let f = crate::mapcopy::Finder::default();
            let (Some(a), Some(b)) = (f.find(&hl.join("cstrike"), &format!("maps/{map}.nav")), f.find(&hl.join("czero"), &format!("maps/{map}.nav"))) else { continue };
            let (a, b) = (load_nav(&a).unwrap(), load_nav(&b).unwrap());
            let c = names_from(&a, &b, 64.0);
            let u = usual_callouts(map, &a);
            let mut after = a.clone();
            apply_changes(&mut after, &c).unwrap();
            write_checked(&a, &after).unwrap();
            println!("{map}: {} areas, {} named now, {} from czero, {} usual; places after {:?}", a.areas.len(), a.areas.iter().filter(|x| x.place != 0).count(), c.len(), u.len(), after.places);
        }
    }
}
