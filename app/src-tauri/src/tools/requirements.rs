//! A Game Mode Studio mode's requirements (agent MCP): the maps it plays on and the content it
//! needs, so the person making it, the community browser and the app all know "this mode plays
//! on these maps and needs these models and sounds".
//!
//! Kept beside the graph as `<graphs>/<id>.requirements.json` (not in the graph: the Studio's own
//! format stays as X has it; docs/notes/X.md "Requirements"):
//! ```json
//! { "schema": "loner-strike/mode-requirements", "version": 1,
//!   "defaultMap": "ls_outpost", "maps": ["ls_outpost", "de_dust2"],
//!   "links": { "models/props/crate.mdl": {"model": "Mod", "id": 12345, "name": "Crate pack"} } }
//! ```
//! Everything else is worked out when asked: where each map comes from (the game's own, a
//! community item, or only on this computer), the content the graph uses (every model, sound and
//! sprite path in its nodes that isn't a file every player has), whether it's here, a suggested
//! link when an installed community item has the file, and the components, creature types and
//! bot brain it needs (read only).
//!   GET  /api/tools/requirements/:id     the requirements with all that
//!   POST /api/tools/requirements/:id     {defaultMap, maps, links}: saved, then the same answer

use crate::server::Shared;
use crate::tools::studio::graph::{self, Graph};
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "loner-strike/mode-requirements";
const MAX_MAPS: usize = 24;
const MAX_LINKS: usize = 128;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Saved {
    pub schema: String,
    pub version: u32,
    pub default_map: String,
    pub maps: Vec<String>,
    /// content path (lower case, "/") -> the community or GameBanana item that provides it
    pub links: BTreeMap<String, Link>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Link {
    /// "Mod" (GameBanana's model) or "Spray"
    pub model: String,
    pub id: i64,
    pub name: String,
}

fn path_of(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.requirements.json"))
}

pub fn load(dir: &Path, id: &str) -> Saved {
    std::fs::read(path_of(dir, id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A content path as the game reads it: "/" separated, lower case, no "..", one of the kinds.
pub fn clean_path(p: &str) -> Option<String> {
    let p = p.trim().replace('\\', "/").to_ascii_lowercase();
    let p = p.trim_start_matches('/').to_string();
    if p.is_empty() || p.len() > 160 || p.contains("..") || p.contains(':') || !p.bytes().all(|c| c.is_ascii_graphic()) {
        return None;
    }
    kind_of(&p).map(|_| p)
}

/// model, sound or sprite, by extension.
pub fn kind_of(p: &str) -> Option<&'static str> {
    let l = p.to_ascii_lowercase();
    if l.ends_with(".mdl") {
        Some("model")
    } else if l.ends_with(".wav") || l.ends_with(".mp3") {
        Some("sound")
    } else if l.ends_with(".spr") {
        Some("sprite")
    } else {
        None
    }
}

/// Every model, sound and sprite path the graph's nodes name (in any setting, so new nodes
/// count too), as the game reads them (sounds under sound/).
pub fn content_of(g: &Graph) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut look = |v: &Value| {
        if let Some(s) = v.as_str() {
            for part in s.split(|c: char| c.is_whitespace() || c == '|' || c == ',' || c == ';' || c == '=') {
                let Some(mut p) = clean_path(part) else { continue };
                if kind_of(&p) == Some("sound") && !p.starts_with("sound/") {
                    p = format!("sound/{p}");
                }
                out.insert(p);
            }
        }
    };
    for n in &g.nodes {
        for v in n.params.values() {
            look(v);
        }
    }
    out
}

/// Where a map comes from: the game's own, one Loner-Strike ships (every player has it, QA4-B),
/// a community item (its key), or only here.
pub fn map_source(name: &str, community: &HashMap<String, Value>) -> (&'static str, Option<String>) {
    if crate::maps::is_stock_file(&format!("maps/{name}.bsp")) {
        return ("base", None);
    }
    if crate::maps::ships_with_app(name) {
        return ("loner", None);
    }
    match community.get(&format!("maps/{}.bsp", name.to_ascii_lowercase())) {
        Some(who) => ("community", who["key"].as_str().map(String::from)),
        None => ("local", None),
    }
}

/// What a client of the Studio sees: the saved requirements and everything worked out.
pub struct Here<'a> {
    pub game_dir: Option<&'a Path>,
    /// is a file every player has (the game's own)
    pub stock: &'a dyn Fn(&str) -> bool,
    pub community: &'a HashMap<String, Value>,
    pub maps_here: &'a [String],
}

pub fn answer(g: &Graph, saved: &Saved, h: &Here) -> Value {
    let map = |name: &str| {
        let (source, key) = map_source(name, h.community);
        let have = h.maps_here.iter().any(|m| m.eq_ignore_ascii_case(name));
        let mut v = json!({"name": name, "source": source, "have": have});
        if let Some(k) = key {
            v["key"] = json!(k);
        }
        if source == "local" {
            v["warning"] = json!("Share this map first so players can get it.");
        }
        v
    };
    let content: Vec<Value> = content_of(g)
        .into_iter()
        .filter(|p| !(h.stock)(p))
        .map(|p| {
            let have = h.game_dir.is_some_and(|d| d.join(&p).is_file());
            let link = saved.links.get(&p).cloned();
            let suggest = h.community.get(&p).map(|who| {
                let key = who["key"].as_str().unwrap_or("");
                let (model, id) = key.split_once(':').map(|(m, i)| (m, i.parse::<i64>().unwrap_or(0))).unwrap_or(("", 0));
                json!({"model": if model.eq_ignore_ascii_case("spray") { "Spray" } else { "Mod" }, "id": id, "name": who["name"]})
            });
            json!({"path": p, "kind": kind_of(&p), "have": have, "link": link, "suggest": if link.is_none() { suggest } else { None }})
        })
        .collect();
    let needs = crate::tools::studio::gen::needs(g);
    let kit = crate::tools::studio::kit::kit_of(g);
    let mut warnings: Vec<String> = vec![];
    let unlinked: Vec<&str> = content.iter().filter(|c| c["link"].is_null()).filter_map(|c| c["path"].as_str()).collect();
    if !unlinked.is_empty() {
        warnings.push(format!("Not linked to a community or GameBanana item, so players may not have them: {}.", unlinked.join(", ")));
    }
    let lost: Vec<&str> = content.iter().filter(|c| c["link"].is_null() && c["have"] == false).filter_map(|c| c["path"].as_str()).collect();
    let maps: Vec<Value> = saved.maps.iter().map(|m| map(m)).collect();
    let local: Vec<&str> = maps.iter().filter(|m| m["source"] == "local").filter_map(|m| m["name"].as_str()).collect();
    if !local.is_empty() {
        warnings.push(format!("Only on this computer (share them first so players can get them): {}.", local.join(", ")));
    }
    if let Some(w) = custom_creatures_warning(&needs.creatures) {
        warnings.push(w);
    }
    json!({
        "defaultMap": if saved.default_map.is_empty() { Value::Null } else { map(&saved.default_map) },
        "maps": maps,
        "content": content,
        "known": {"components": kit["components"], "creatures": needs.creatures, "brain": !needs.creatures.is_empty(), "componentsPlugin": needs.components},
        // what it needs beyond the game, for the package and the community page (QA4-G F6)
        "needs": mode_needs(g),
        "brainHere": h.game_dir.map(brain_here),
        "warnings": warnings,
        // can't be found anywhere: not here, not the game's own, not linked (Publish stops on these)
        "missing": lost,
        "installedMaps": h.maps_here.iter().map(|m| map(m)).collect::<Vec<_>>(),
    })
}

/// The bot-brain features a mode can use, the brain version each needs (`ls_mode_ver` for bots,
/// `ls_creature_ver` for creatures) and how players see it.
const FEATURES: [(&str, &str, &str, u8); 5] = [
    ("creatures", "Creatures (zombies, the wave spawner's types)", "creature", 1),
    ("creature_control_2", "Creature State, Target, Speed and Tier", "creature", 2),
    ("bot_orders", "Orders to bots (Go To, Hold, Follow, a bot's nature)", "mode", 1),
    ("bots_in_modes", "Bots that play the mode (the Bots nodes: goals, offers, phases)", "mode", 1),
    ("influence", "Personality influence on bot orders", "mode", 2),
];

/// What a mode needs beyond the game (QA4-G F6): Loner-Strike's components plugin and which map
/// components, the creature types, and the bot-brain features with the lowest brain versions that
/// have them. From the graph (Macros and Functions expanded) and its map kit.
pub fn mode_needs(g: &Graph) -> Value {
    let ex = crate::tools::studio::expand::expand(g);
    let nodes = &ex.graph.nodes;
    let n = crate::tools::studio::gen::needs(g);
    let kit = crate::tools::studio::kit::kit_of(g);
    let components: Vec<Value> = kit["components"].as_array().into_iter().flatten().filter_map(|c| c["classname"].as_str()).map(|c| json!(c)).collect();
    let has = |f: &dyn Fn(&str) -> bool| nodes.iter().any(|x| f(&x.kind));
    // an influence other than 50 ("as usual"), or wired in
    let influence = nodes.iter().any(|x| {
        x.kind.starts_with("bot") && (x.params.get("influence").and_then(Value::as_f64).is_some_and(|v| (v - 50.0).abs() > 0.01) || ex.graph.edges.iter().any(|e| e.to.node == x.id && e.to.pin == "influence"))
    });
    let mut used: Vec<&str> = vec![];
    if !n.creatures.is_empty() || has(&|k| k.starts_with("creature.")) {
        used.push("creatures");
    }
    if has(&|k| ["creature.state", "creature.target", "creature.set_speed", "creature.set_tier"].contains(&k)) {
        used.push("creature_control_2");
    }
    if has(&|k| k.starts_with("bot.")) {
        used.push("bot_orders");
    }
    if has(&|k| k.starts_with("bots.")) {
        used.push("bots_in_modes");
    }
    if influence {
        used.push("influence");
    }
    let need = |brain: &str| FEATURES.iter().filter(|f| f.2 == brain && used.contains(&f.0)).map(|f| f.3).max().unwrap_or(0);
    json!({
        "componentsPlugin": n.components,
        "components": components,
        "creatures": n.creatures,
        "modeVer": need("mode"),
        "creatureVer": need("creature"),
        "features": FEATURES.iter().filter(|f| used.contains(&f.0)).map(|f| json!({"id": f.0, "name": f.1, "brain": f.2, "ver": f.3})).collect::<Vec<_>>(),
    })
}

/// The bot brain in a game folder, as the versions its `dlls/mp.dll` has (0: none): read from the
/// strings each version adds (no running game needed). `ls_mode_ver` 1 and 2 ("[influence 0-100]" in
/// ls_mode's help), `ls_creature_ver` 1 and 2 (the speed/tier help's "0.2..3").
pub fn brain_here(game_dir: &Path) -> Value {
    let b = std::fs::read(game_dir.join("dlls/mp.dll")).or_else(|_| std::fs::read(game_dir.join("dlls/cs.so"))).unwrap_or_default();
    let has = |s: &str| b.windows(s.len()).any(|w| w == s.as_bytes());
    let mode = if has("[influence 0-100]") { 2 } else if has("ls_mode_ver") { 1 } else { 0 };
    let creature = if has("ls_creature_ver") { if has("0.2..3") { 2 } else { 1 } } else { 0 };
    json!({"modeVer": mode, "creatureVer": creature})
}

/// A request to save: checked and cleaned.
pub fn cleaned(req: &Value) -> Result<Saved, String> {
    let name_ok = |m: &str| crate::maps::safe_arg(m) && m.len() <= 31;
    let default_map = req["defaultMap"].as_str().unwrap_or("").trim().to_string();
    if !default_map.is_empty() && !name_ok(&default_map) {
        return Err("That isn't a map name.".into());
    }
    let mut maps: Vec<String> = vec![];
    for m in req["maps"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::trim) {
        if !name_ok(m) {
            return Err(format!("{m} isn't a map name."));
        }
        if !maps.iter().any(|x| x.eq_ignore_ascii_case(m)) {
            maps.push(m.to_string());
        }
    }
    if maps.len() > MAX_MAPS {
        return Err(format!("At most {MAX_MAPS} recommended maps."));
    }
    let mut links = BTreeMap::new();
    for (p, l) in req["links"].as_object().into_iter().flatten() {
        let path = clean_path(p).ok_or_else(|| format!("{p} isn't a model, sound or sprite path."))?;
        if l.is_null() {
            continue;
        }
        let model = match l["model"].as_str().unwrap_or("Mod") {
            "Mod" | "mod" => "Mod",
            "Spray" | "spray" => "Spray",
            _ => return Err("A link is a GameBanana Mod or Spray page.".into()),
        };
        let id = l["id"].as_i64().filter(|i| *i > 0 && *i < 100_000_000).ok_or("A link needs the page's number.")?;
        let name: String = l["name"].as_str().unwrap_or("").chars().filter(|c| !c.is_control()).take(80).collect();
        links.insert(path, Link { model: model.into(), id, name });
    }
    if links.len() > MAX_LINKS {
        return Err("That's too many links.".into());
    }
    Ok(Saved { schema: SCHEMA.into(), version: 1, default_map, maps, links })
}

// ---- routes ----------------------------------------------------------------------------------

fn read_graph(dir: &Path, id: &str) -> Result<Graph, String> {
    if !graph::valid_mode_id(id) {
        return Err("That isn't a mode id.".into());
    }
    let text = std::fs::read_to_string(dir.join(format!("{id}.graph.json"))).map_err(|_| "There's no graph with that id.".to_string())?;
    graph::parse(&text)
}

fn maps_in(game_dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(game_dir.join("maps"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".bsp").map(String::from)))
        .filter(|n| crate::maps::safe_arg(n))
        .collect();
    v.sort();
    v
}

/// Creature types a mode spawns that aren't the game's own: a package only names them (their
/// creatures.ini sections aren't in it), so players have to make them in the Creature Studio.
/// ("all": the wave spawner's own; controller and stukabat are Last Stand's round-table names a
/// stand-in plays until the game has them.)
pub fn custom_creatures(names: &[String]) -> Vec<String> {
    names.iter().filter(|n| *n != "all" && !["controller", "stukabat"].contains(&n.as_str()) && crate::tools::creatures::builtin(n).is_none()).cloned().collect()
}

fn custom_creatures_warning(names: &[String]) -> Option<String> {
    let c = custom_creatures(names);
    (!c.is_empty()).then(|| format!("Custom creatures aren't included yet: players need to make {} themselves (Creature Studio).", c.join(", ")))
}

/// Everything about a mode's requirements on this computer.
pub fn report(st: &Shared, id: &str) -> Result<Value, String> {
    let dir = crate::tools::studio::graphs_dir();
    let g = read_graph(&dir, id)?;
    let saved = load(&dir, id);
    let game = st.local_game();
    let community = crate::tools::credits::community_files_in(&crate::sys::app_dir());
    let maps_here = game.as_ref().map(|g| maps_in(&g.dir)).unwrap_or_default();
    let stock = |p: &str| game.as_ref().is_some_and(|g| crate::tools::doctor::is_stock(p, &g.name));
    let mut v = answer(&g, &saved, &Here { game_dir: game.as_ref().map(|g| g.dir.as_path()), stock: &stock, community: &community, maps_here: &maps_here });
    v["id"] = json!(id);
    Ok(v)
}

pub fn save(st: &Shared, id: &str, req: &Value) -> Result<Value, String> {
    let dir = crate::tools::studio::graphs_dir();
    read_graph(&dir, id)?;
    let s = cleaned(req)?;
    let text = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&path_of(&dir, id), text.as_bytes()).map_err(|e| format!("Couldn't save the requirements ({e})."))?;
    report(st, id)
}

fn answer_json(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

async fn api_get(State(st): State<Shared>, UrlPath(id): UrlPath<String>) -> (StatusCode, Json<Value>) {
    answer_json(tokio::task::spawn_blocking(move || report(&st, &id)).await.unwrap_or_else(|_| Err("That didn't finish.".into())))
}

async fn api_set(State(st): State<Shared>, UrlPath(id): UrlPath<String>, h: HeaderMap, Json(req): Json<Value>) -> (StatusCode, Json<Value>) {
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    answer_json(tokio::task::spawn_blocking(move || save(&st, &id, &req)).await.unwrap_or_else(|_| Err("That didn't finish.".into())))
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/requirements/:id", get(api_get).post(api_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_list_components_creatures_and_the_brain_versions() {
        // nothing beyond the game
        let plain = mode_needs(&g(&[("msg.chat", "text", "hi")]));
        assert_eq!((plain["modeVer"].as_u64(), plain["creatureVer"].as_u64(), plain["features"].as_array().map(Vec::len)), (Some(0), Some(0), Some(0)));
        // a creature, creature control 2 and a bot order: the highest version each brain needs
        let gr = g(&[("creature.spawn", "kind", "zombie"), ("creature.set_tier", "tier", "3"), ("bot.hold", "secs", "5")]);
        let n = mode_needs(&gr);
        assert_eq!(n["creatures"], json!(["zombie"]));
        assert_eq!((n["creatureVer"].as_u64(), n["modeVer"].as_u64()), (Some(2), Some(1)));
        let ids: Vec<&str> = n["features"].as_array().unwrap().iter().filter_map(|f| f["id"].as_str()).collect();
        assert_eq!(ids, vec!["creatures", "creature_control_2", "bot_orders"]);
        // a personality influence other than 50: bots version 2
        let inf = g(&[("bot.follow", "influence", "x")]);
        let mut infg = inf.clone();
        infg.nodes[0].params.insert("influence".into(), json!(20));
        assert_eq!(mode_needs(&infg)["modeVer"], 2);
        let mut same = inf;
        same.nodes[0].params.insert("influence".into(), json!(50));
        assert_eq!(mode_needs(&same)["modeVer"], 1);
    }

    #[test]
    fn custom_creatures_are_named_as_not_included() {
        let names = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(custom_creatures_warning(&names(&["zombie", "all", "gonarch", "controller"])), None);
        let w = custom_creatures_warning(&names(&["zombie", "qa_runner_", "my_brute_2"])).unwrap();
        assert_eq!(w, "Custom creatures aren't included yet: players need to make qa_runner_, my_brute_2 themselves (Creature Studio).");
        // the report a Publish check shows
        let gr = g(&[("creature.spawn", "kind", "qa_runner_")]);
        let v = answer(&gr, &Default::default(), &Here { game_dir: None, stock: &|_: &str| false, community: &Default::default(), maps_here: &[] });
        assert!(v["warnings"].as_array().unwrap().iter().any(|x| x.as_str().is_some_and(|s| s.contains("qa_runner_ themselves"))), "{v}");
    }

    #[test]
    fn the_brain_here_from_its_mp_dll() {
        let dir = std::env::temp_dir().join(format!("ls-brain-here-{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join("dlls"));
        assert_eq!(brain_here(&dir), json!({"modeVer": 0, "creatureVer": 0}));
        std::fs::write(dir.join("dlls/mp.dll"), b"..ls_mode_ver..ls_creature_ver..").unwrap();
        assert_eq!(brain_here(&dir), json!({"modeVer": 1, "creatureVer": 1}));
        std::fs::write(dir.join("dlls/mp.dll"), b"..ls_mode_ver..ls_mode goal <who> [influence 0-100]..ls_creature_ver..<0.2..3>").unwrap();
        assert_eq!(brain_here(&dir), json!({"modeVer": 2, "creatureVer": 2}));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn g(params: &[(&str, &str, &str)]) -> Graph {
        let nodes: Vec<Value> = params.iter().enumerate().map(|(i, (kind, k, v))| json!({"id": format!("n{i}"), "type": kind, "pos": {"x": 0, "y": 0}, "params": {*k: v}})).collect();
        graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "req_test", "name": "R"}, "nodes": nodes, "edges": [], "variables": [], "comments": []})).unwrap()
    }

    #[test]
    fn content_is_found_in_any_node_setting() {
        let gr = g(&[("ent.spawn_prop", "model", "models/Props/Crate.mdl"), ("fx.sound_at", "sound", "ambience/wind.wav"), ("comp.create", "keys", "sprite=sprites/glow01.spr|x=1"), ("msg.chat", "text", "hello.world")]);
        let c: Vec<String> = content_of(&gr).into_iter().collect();
        assert_eq!(c, vec!["models/props/crate.mdl", "sound/ambience/wind.wav", "sprites/glow01.spr"]);
        assert_eq!(clean_path("../../x.mdl"), None);
        assert_eq!(clean_path("C:\\x.mdl"), None);
    }

    #[test]
    fn maps_and_content_where_they_come_from() {
        let gr = g(&[("ent.spawn_prop", "model", "models/crate.mdl"), ("ent.spawn_prop", "model", "models/player.mdl"), ("fx.sound_at", "sound", "my/beep.wav")]);
        let mut community = HashMap::new();
        community.insert("maps/de_lonely.bsp".to_string(), json!({"key": "mod:101", "name": "de_lonely"}));
        community.insert("models/crate.mdl".to_string(), json!({"key": "mod:77", "name": "Crate pack"}));
        let saved = cleaned(&json!({"defaultMap": "de_dust2", "maps": ["de_dust2", "de_lonely", "my_test"], "links": {"sound/my/beep.wav": null}})).unwrap();
        let stock = |p: &str| p == "models/player.mdl";
        let here = ["de_dust2".to_string(), "de_lonely".to_string(), "my_test".to_string()];
        let v = answer(&gr, &saved, &Here { game_dir: None, stock: &stock, community: &community, maps_here: &here });
        assert_eq!(v["defaultMap"]["source"], "base");
        // a map Loner-Strike ships is every player's: no "share it first" (QA4-B)
        assert_eq!(map_source("ls_outpost", &HashMap::new()), ("loner", None));
        assert_eq!(map_source("br_valley", &HashMap::new()), ("loner", None));
        assert_eq!(map_source("my_own_map", &HashMap::new()), ("local", None));
        assert_eq!(v["maps"][1]["source"], "community");
        assert_eq!(v["maps"][1]["key"], "mod:101");
        assert_eq!(v["maps"][2]["source"], "local");
        assert_eq!(v["maps"][2]["warning"], "Share this map first so players can get it.");
        let content = v["content"].as_array().unwrap();
        assert_eq!(content.len(), 2, "the game's own model isn't listed: {content:?}");
        let crate_ = content.iter().find(|c| c["path"] == "models/crate.mdl").unwrap();
        assert_eq!(crate_["suggest"]["id"], 77, "an installed community item has it");
        assert_eq!(v["missing"], json!(["models/crate.mdl", "sound/my/beep.wav"]), "nothing here, nothing linked");
        assert_eq!(v["warnings"].as_array().unwrap().len(), 2);
        // linked: no longer missing
        let saved = cleaned(&json!({"maps": [], "links": {"models/crate.mdl": {"model": "Mod", "id": 77, "name": "Crate pack"}, "sound/my/beep.wav": {"model": "Mod", "id": 5}}})).unwrap();
        let v = answer(&gr, &saved, &Here { game_dir: None, stock: &stock, community: &community, maps_here: &here });
        assert_eq!(v["missing"], json!([]));
        assert!(v["content"][0]["suggest"].is_null(), "linked: no suggestion");
    }

    #[test]
    fn bad_requests_are_refused() {
        assert!(cleaned(&json!({"defaultMap": "de_dust2;quit"})).is_err());
        assert!(cleaned(&json!({"maps": ["a b"]})).is_err());
        assert!(cleaned(&json!({"links": {"../../evil.mdl": {"model": "Mod", "id": 1}}})).is_err());
        assert!(cleaned(&json!({"links": {"models/x.mdl": {"model": "Skin", "id": 1}}})).is_err());
        assert!(cleaned(&json!({"links": {"models/x.mdl": {"model": "Mod", "id": -3}}})).is_err());
        let s = cleaned(&json!({"maps": ["de_dust2", "DE_DUST2"]})).unwrap();
        assert_eq!(s.maps, vec!["de_dust2"], "no repeats");
    }
}
