//! Engine effects, sounds and spawning (the effects agent): Dynamic Light, Decal, Break Into
//! Gibs, Beam Ring; Play Sound 2D / at Location / on Entity, Loop Sound, Stop Sound; Spawn
//! Entity (an allowlist of the game's classes) and Spawn Weapon On Ground; and the transform every
//! spawn node takes (At entity with a local offset and a rotation offset, or At position;
//! Rotation, Velocity, Scale), used by the other spawn nodes' code in gen.rs too.
//!
//! Kept out of gen.rs (others edit it), like engine.rs: gen.rs calls `is_action`/`action`,
//! `blocks`, `init`, `precache`/`needs_precache`; graph.rs calls `problems`.
//!
//! Only the engine's crash points are enforced (entity slots with a reserve, temp-entity
//! messages per frame, precached files): hitting one skips that one effect or spawn, logs it,
//! and the debugger shows what was skipped and why. Counts people may want more of (loops at
//! once, gibs) are node settings, and Problems only warns when they're raised a lot.

use super::{float_lit, pawn_str, Gen, Out};
use crate::tools::studio::graph::{Graph, Node, NodeDef, Problem, View};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) const KINDS: [&str; 11] = [
    "fx.dlight",
    "fx.decal",
    "fx.gibs",
    "fx.ring",
    "snd.play_2d",
    "snd.play_at",
    "snd.play_on",
    "snd.loop",
    "snd.stop",
    "ent.spawn_entity",
    "ent.spawn_weapon",
];

/// The older sound nodes these replace (still working in saved graphs, left out of the list).
#[allow(dead_code)]
pub(crate) const SUPERSEDED: [(&str, &str); 3] = [("game.play_sound", "Play Sound 2D"), ("fx.sound_at", "Play Sound at Location"), ("fx.sound_to", "Play Sound 2D")];

/// The spawn nodes that take the shared transform (At entity / At position, Rotation, Velocity,
/// Scale, Local offset, Rotation offset): their own, and gen.rs's.
pub(crate) const SPAWNERS: [&str; 7] = ["ent.spawn_prop", "creature.spawn", "npc.spawn", "npc.spawn_use", "comp.create", "ent.spawn_entity", "ent.spawn_weapon"];

/// Spawn Entity's classes: effects, sounds and markers the game makes safely at any time, with
/// the files each one loads when it spawns (loaded with the map instead: loading a file later
/// stops the server). Never players, worldspawn, game_* server controls, anything that runs
/// commands or changes the map, or brush entities (they need a model from the map).
pub(crate) const SPAWN_CLASSES: [(&str, &str, &[&str]); 23] = [
    ("env_sprite", "Sprite (env_sprite: model=sprites/...spr)", &[]),
    ("env_glow", "Glow (env_glow: model=sprites/...spr)", &[]),
    ("cycler_sprite", "Animated sprite or model (cycler_sprite: model=...)", &[]),
    ("env_spark", "Sparks (env_spark)", &["sound/buttons/spark1.wav", "sound/buttons/spark2.wav", "sound/buttons/spark3.wav", "sound/buttons/spark4.wav", "sound/buttons/spark5.wav", "sound/buttons/spark6.wav"]),
    ("env_explosion", "Explosion (env_explosion: iMagnitude=100)", &[]),
    ("env_shake", "Screen shake (env_shake)", &[]),
    ("env_fade", "Screen fade (env_fade)", &[]),
    ("env_render", "Render changer (env_render)", &[]),
    ("env_blood", "Blood spray (env_blood)", &[]),
    ("env_funnel", "Particle funnel (env_funnel)", &["sprites/flare6.spr"]),
    ("env_beverage", "Soda can chute (env_beverage)", &["models/can.mdl", "sound/weapons/g_bounce3.wav"]),
    ("env_shooter", "Model shooter (env_shooter: shootmodel=models/...mdl)", &["sound/debris/glass1.wav", "sound/debris/glass2.wav", "sound/debris/glass3.wav", "sound/debris/wood1.wav", "sound/debris/wood2.wav", "sound/debris/wood3.wav", "sound/debris/metal1.wav", "sound/debris/metal2.wav", "sound/debris/metal3.wav", "sound/debris/flesh1.wav", "sound/debris/flesh2.wav", "sound/debris/flesh3.wav", "sound/debris/concrete1.wav", "sound/debris/concrete2.wav", "sound/debris/concrete3.wav"]),
    ("gibshooter", "Gib shooter (gibshooter)", &["models/hgibs.mdl"]),
    ("env_beam", "Beam (env_beam: texture=sprites/laserbeam.spr)", &[]),
    ("env_laser", "Laser (env_laser: texture=sprites/laserbeam.spr)", &[]),
    ("ambient_generic", "Sound (ambient_generic: message=ambience/x.wav)", &[]),
    ("info_target", "Target point (info_target)", &[]),
    ("path_corner", "Path stop (path_corner)", &[]),
    ("trigger_relay", "Relay (trigger_relay)", &[]),
    ("multi_manager", "Multi manager (multi_manager)", &[]),
    ("armoury_entity", "Weapon on the ground (armoury_entity: item=0..29)", &[]),
    ("item_healthkit", "Health kit (item_healthkit)", &["models/w_medkit.mdl", "sound/items/smallmedkit1.wav"]),
    ("item_battery", "Battery (item_battery)", &["models/w_battery.mdl", "sound/items/gunpickup2.wav"]),
];

/// Keys Spawn Entity never sets (the node's own pins do, or they'd change what it is).
const BLOCKED_KEYS: [&str; 6] = ["classname", "origin", "angles", "model_index", "spawnflags_override", "globalname"];

/// Spawn Weapon On Ground's items: (id, label, armoury item, weapon classname for its ammo).
pub(crate) const WEAPON_ITEMS: [(&str, &str, i64, &str); 30] = [
    ("mp5navy", "MP5", 0, "weapon_mp5navy"), ("tmp", "TMP", 1, "weapon_tmp"), ("p90", "P90", 2, "weapon_p90"), ("mac10", "MAC-10", 3, "weapon_mac10"),
    ("ak47", "AK-47", 4, "weapon_ak47"), ("sg552", "SG 552", 5, "weapon_sg552"), ("m4a1", "M4A1", 6, "weapon_m4a1"), ("aug", "AUG", 7, "weapon_aug"),
    ("scout", "Scout", 8, "weapon_scout"), ("g3sg1", "G3SG1", 9, "weapon_g3sg1"), ("awp", "AWP", 10, "weapon_awp"), ("m3", "M3 shotgun", 11, "weapon_m3"),
    ("xm1014", "XM1014 shotgun", 12, "weapon_xm1014"), ("m249", "M249", 13, "weapon_m249"), ("flashbang", "Flashbang", 14, ""), ("hegrenade", "HE grenade", 15, ""),
    ("kevlar", "Kevlar", 16, ""), ("assault", "Kevlar and helmet", 17, ""), ("smokegrenade", "Smoke grenade", 18, ""), ("shield", "Tactical shield", 19, ""),
    ("famas", "FAMAS", 20, "weapon_famas"), ("sg550", "SG 550", 21, "weapon_sg550"), ("galil", "Galil", 22, "weapon_galil"), ("ump45", "UMP 45", 23, "weapon_ump45"),
    ("glock18", "Glock", 24, "weapon_glock18"), ("usp", "USP", 25, "weapon_usp"), ("elite", "Dual Elites", 26, "weapon_elite"), ("fiveseven", "Five-SeveN", 27, "weapon_fiveseven"),
    ("p228", "P228", 28, "weapon_p228"), ("deagle", "Desert Eagle", 29, "weapon_deagle"),
];

/// The world model each item lies as (loaded with the map: the game may not have it loaded).
fn item_model(id: &str) -> String {
    match id {
        "kevlar" => "models/w_kevlar.mdl".into(),
        "assault" => "models/w_assault.mdl".into(),
        "shield" => "models/w_shield.mdl".into(),
        "mp5navy" => "models/w_mp5.mdl".into(),
        x => format!("models/w_{x}.mdl"),
    }
}

/// Decal choices: (id, label, the decal names (one picked at random)).
pub(crate) const DECALS: [(&str, &str, &[&str]); 8] = [
    ("shot", "Bullet hole", &["{shot1", "{shot2", "{shot3", "{shot4", "{shot5"]),
    ("bigshot", "Big bullet hole", &["{bigshot1", "{bigshot2", "{bigshot3", "{bigshot4", "{bigshot5"]),
    ("scorch", "Scorch mark", &["{scorch1", "{scorch2", "{scorch3"]),
    ("smscorch", "Small scorch", &["{smscorch1", "{smscorch2", "{smscorch3"]),
    ("blood", "Blood", &["{blood1", "{blood2", "{blood3", "{blood4", "{blood5", "{blood6", "{blood7", "{blood8"]),
    ("yblood", "Yellow blood", &["{yblood1", "{yblood2", "{yblood3", "{yblood4", "{yblood5", "{yblood6"]),
    ("break", "Broken glass", &["{break1", "{break2", "{break3"]),
    ("lambda", "Lambda", &["{lambda01", "{lambda02", "{lambda03", "{lambda04", "{lambda05", "{lambda06"]),
];

/// Break Into Gibs' stock models and materials: (id, label, model) and (id, label, break flag, sound).
pub(crate) const GIB_MODELS: [(&str, &str, &str); 8] = [
    ("wood", "Wood planks", "models/woodgibs.mdl"),
    ("glass", "Glass shards", "models/glassgibs.mdl"),
    ("metal", "Metal plates", "models/metalplategibs.mdl"),
    ("cinder", "Concrete chunks", "models/cindergibs.mdl"),
    ("ceiling", "Ceiling tiles", "models/ceilinggibs.mdl"),
    ("computer", "Computer parts", "models/computergibs.mdl"),
    ("rock", "Rocks", "models/rockgibs.mdl"),
    ("flesh", "Flesh", "models/fleshgibs.mdl"),
];
pub(crate) const MATERIALS: [(&str, &str, i64, &str); 6] = [
    ("none", "None", 0, ""),
    ("glass", "Glass", 0x01, "debris/bustglass1.wav"),
    ("metal", "Metal", 0x02, "debris/bustmetal1.wav"),
    ("flesh", "Flesh", 0x04, "debris/bustflesh1.wav"),
    ("wood", "Wood", 0x08, "debris/bustcrate1.wav"),
    ("concrete", "Concrete", 0x40, "debris/bustconcrete1.wav"),
];

pub(super) fn is_action(kind: &str) -> bool {
    KINDS.contains(&kind)
}

fn uses(g: &Graph, k: &str) -> bool {
    g.nodes.iter().any(|n| n.kind == k)
}

/// A text setting: the node's, else the registry's default, else `d`.
fn s<'a>(n: &'a Node, id: &str, d: &'a str) -> &'a str {
    if let Some(v) = n.params.get(id).and_then(Value::as_str) {
        return v;
    }
    crate::tools::studio::graph::registry().get(&n.kind).and_then(|def| def.param(id)).and_then(|p| p.default.as_str()).unwrap_or(d)
}

/// A sound path under sound/ (.wav), as the game names it (without sound/): empty if it isn't one.
pub(crate) fn sound_path(p: &str) -> String {
    let t: String = p.trim().replace('\\', "/").chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    let t = t.strip_prefix("sound/").unwrap_or(&t).to_string();
    if t.to_ascii_lowercase().ends_with(".wav") && !t.contains("..") && !t.starts_with('/') && t.len() > 4 { t } else { String::new() }
}

/// A model path (models/...mdl) or empty.
fn model_path(p: &str) -> String {
    let t: String = p.trim().replace('\\', "/").chars().filter(|c| c.is_ascii_alphanumeric() || "_-./".contains(*c)).take(64).collect();
    if t.starts_with("models/") && t.to_ascii_lowercase().ends_with(".mdl") && !t.contains("..") { t } else { String::new() }
}

/// Spawn Entity's key/value text: one pair a line (or separated by |), key=value. Keys are
/// letters, digits and _; values lose quotes, carets and control characters (127 at most).
pub(crate) fn spawn_keys(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in text.split(['\n', '|']).take(32) {
        let Some((k, v)) = part.split_once('=') else { continue };
        let k: String = k.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(32).collect();
        if k.is_empty() || BLOCKED_KEYS.contains(&k.to_ascii_lowercase().as_str()) {
            continue;
        }
        let v: String = v.trim().chars().filter(|c| !c.is_control() && *c != '"' && *c != '^').take(127).collect();
        out.push((k, v));
    }
    out
}

/// The files a key value names (models/sprites by path, sounds under sound/): loaded with the map.
fn key_files(keys: &[(String, String)]) -> (Vec<String>, Vec<String>) {
    let (mut models, mut sounds) = (Vec::new(), Vec::new());
    for (_, v) in keys {
        let l = v.to_ascii_lowercase();
        if (l.starts_with("models/") && l.ends_with(".mdl")) || (l.starts_with("sprites/") && l.ends_with(".spr")) {
            if !v.contains("..") {
                models.push(v.clone());
            }
        } else if l.ends_with(".wav") {
            let p = sound_path(v);
            if !p.is_empty() {
                sounds.push(p);
            }
        }
    }
    (models, sounds)
}

fn class_of(n: &Node) -> Option<&'static (&'static str, &'static str, &'static [&'static str])> {
    let c = s(n, "class", "env_sprite");
    SPAWN_CLASSES.iter().find(|x| x.0 == c)
}

/// Every model and sound a node in the graph loads with the map (models/sprites, sounds without sound/).
fn files(g: &Graph) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut m, mut snd) = (BTreeSet::new(), BTreeSet::new());
    for n in &g.nodes {
        match n.kind.as_str() {
            "snd.play_2d" | "snd.play_at" | "snd.play_on" | "snd.loop" => {
                let p = sound_path(s(n, "sound", "ambience/wind1.wav"));
                if !p.is_empty() {
                    snd.insert(p);
                }
            }
            "snd.stop" => {
                snd.insert("common/null.wav".into());
            }
            "fx.gibs" => {
                let custom = model_path(s(n, "custom", ""));
                let stock = GIB_MODELS.iter().find(|x| x.0 == s(n, "model", "wood")).map(|x| x.2).unwrap_or("models/woodgibs.mdl");
                m.insert(if custom.is_empty() { stock.to_string() } else { custom });
                if let Some(mat) = MATERIALS.iter().find(|x| x.0 == s(n, "material", "wood")).filter(|x| !x.3.is_empty()) {
                    snd.insert(mat.3.to_string());
                }
            }
            "fx.ring" => {
                m.insert("sprites/shockwave.spr".into());
                m.insert("sprites/laserbeam.spr".into());
            }
            "ent.spawn_entity" => {
                if let Some(c) = class_of(n) {
                    for f in c.2 {
                        match f.strip_prefix("sound/") {
                            Some(x) => snd.insert(x.to_string()),
                            None => m.insert(f.to_string()),
                        };
                    }
                }
                let (km, ks) = key_files(&spawn_keys(s(n, "keys", "")));
                m.extend(km);
                snd.extend(ks);
            }
            "ent.spawn_weapon" => {
                m.insert(item_model(s(n, "item", "ak47")));
            }
            _ => {}
        }
    }
    (m, snd)
}

// ---- the transform every spawn node takes ------------------------------------------------

/// Whether a spawn node's place comes from the shared transform pins (a position, or an
/// entity wired into its At pin with an offset). `at_pin` is the node's entity pin.
pub(super) fn place_wired(gen: &Gen, id: &str, at_pin: &str) -> bool {
    let w = |p: &str| gen.v.data_source(id, p).is_some();
    w("pos") || (w(at_pin) && w("offset"))
}

/// Code that finds where a spawn node makes its thing, into `o` (a Float:o[3] already
/// declared): the entity wired into `at_pin` (with Local offset), else At position, else
/// `fallback` (the node's own way, like the entity with a name). It is an expression that's
/// true when a place was found; it also remembers the source entity for the rotation.
pub(super) fn place(gen: &Gen, id: &str, at_pin: &str, fallback: &str) -> String {
    let w = |p: &str| gen.v.data_source(id, p).is_some();
    let ent = if w(at_pin) { gen.need(id, at_pin, "entity") } else { "0".into() };
    let pos = if w("pos") { gen.need(id, "pos", "vector") } else { "-1".into() };
    let ofs = if w("offset") { gen.need(id, "offset", "vector") } else { "-1".into() };
    let (n, def) = gen.node(id);
    if w("pos") || (w(at_pin) && w("offset")) {
        format!("lsg_xf_at({ent}, {pos}, {ofs}, o)")
    } else if w(at_pin) && copies(n, def) {
        // (the node's own way, remembering the entity to face its way)
        format!("lsg_xf_from({ent}, {fallback})")
    } else {
        fallback.to_string()
    }
}

/// The line that turns, pushes and sizes a spawned entity (`e`) after it's made, or nothing
/// when no transform pin is wired and nothing is copied from an At entity. `scale_ok`: models
/// and sprites (players and brush things aren't scaled). `yaw_param`: an older yaw setting
/// (Spawn NPC's Facing), added as a yaw offset so saved graphs face the same way.
pub(super) fn apply(gen: &Gen, n: &Node, def: &'static NodeDef, id: &str, e: &str, at_pin: &str, scale_ok: bool, yaw_param: Option<&str>) -> Option<String> {
    let w = |p: &str| gen.v.data_source(id, p).is_some();
    let scale_set = w("scale") || (gen.num(n, def, "scale") - 1.0).abs() > 1e-6;
    let copy = w(at_pin) && copies(n, def);
    // (an NPC's Facing already turns it; it's added on top only when the facing comes from here)
    let yaw = yaw_param.map(|k| gen.num(n, def, k)).filter(|y| y.abs() > 1e-6).filter(|_| copy || w("angles_offset"));
    if !(w("angles") || w("angles_offset") || w("vel") || (scale_ok && scale_set) || copy) {
        return None;
    }
    let rot = if w("angles") { gen.need(id, "angles", "vector") } else { "-1".into() };
    let rot_ofs = match (w("angles_offset"), yaw) {
        (true, Some(y)) => format!("lsg_vadd({}, lsg_vec(0.0, {}, 0.0), 1.0)", gen.need(id, "angles_offset", "vector"), float_lit(y)),
        (true, None) => gen.need(id, "angles_offset", "vector"),
        (false, Some(y)) => format!("lsg_vec(0.0, {}, 0.0)", float_lit(y)),
        (false, None) => "-1".into(),
    };
    let vel = if w("vel") { gen.need(id, "vel", "vector") } else { "-1".into() };
    let scale = if !scale_ok { "1.0".to_string() } else if w("scale") { gen.need(id, "scale", "number") } else { float_lit(gen.num(n, def, "scale")) };
    Some(format!("\tlsg_xf_set({e}, {}, {rot}, {rot_ofs}, {vel}, {scale})", if copy { 1 } else { 0 }))
}

/// The solid spawn nodes with "If the spot is blocked" (their Moved output needs its own slot).
pub(crate) const STORE: [&str; 8] = ["ent.spawn_prop", "creature.spawn", "comp.create", "ent.spawn_entity", "ent.spawn_weapon", "npc.spawn", "npc.spawn_use", "proj.spawn"];

/// How big a solid spawn is for the free-spot check: 0 small (a weapon, an item), 1 medium (a
/// prop, a component), 2 tall (a creature); -1 no check (it isn't solid).
pub(super) fn hull_size(n: &Node, def: &NodeDef) -> i64 {
    match def.kind.as_str() {
        "ent.spawn_prop" if !n.params.get("solid").and_then(Value::as_bool).unwrap_or(true) => -1,
        "ent.spawn_prop" | "comp.create" => 1,
        "creature.spawn" => 2,
        // T1fix: an NPC stands like a person; an interactable is small; a projectile isn't solid
        "npc.spawn" => 2,
        "npc.spawn_use" => 0,
        "proj.spawn" => -1,
        "ent.spawn_weapon" => 0,
        "ent.spawn_entity" => match n.params.get("class").and_then(Value::as_str).unwrap_or("env_sprite") {
            "armoury_entity" | "item_healthkit" | "item_battery" | "env_beverage" => 0,
            "cycler_sprite" => 1,
            _ => -1,
        },
        _ => -1,
    }
}

/// Spawn Failure (graph.rs BUILTIN_ENUMS "ls_spawn_fail"): why a spawn node didn't make its thing.
pub(crate) const WHY_BLOCKED: i64 = 1;
pub(crate) const WHY_SLOTS: i64 = 2;
pub(crate) const WHY_LIMIT: i64 = 3;
pub(crate) const WHY_MISSING: i64 = 4;
pub(crate) const WHY_RETRIES: i64 = 5;
pub(crate) const WHY_BAD: i64 = 6;

/// "If the spot is blocked" (Unreal's Spawn Collision Handling): 0 always spawn, ignore
/// collisions; 1 try a nearby spot, but always spawn; 2 try a nearby spot, don't spawn if still
/// blocked; 3 don't spawn if blocked; 4 retry every N seconds up to M tries.
pub(super) fn blocked_mode(gen: &Gen, n: &Node, def: &'static NodeDef) -> i64 {
    match gen.choice(n, def, "blocked") {
        "force" => 0,
        "nearby_always" => 1,
        "exact" => 3,
        "retry" => 4,
        _ => 2,
    }
}

/// A solid spawn node's body (the spawn outcome standard, docs/notes/FX.md): checks `pre` in
/// order (each a condition, its Failure Reason and the debugger's words), finds the spot
/// (`place`, into `o`), handles a blocked spot as "If the spot is blocked" says, makes the thing
/// with `create` (an expression using `o`; `create_why` when it gives 0), turns and pushes it
/// (`apply`). Outputs: Then (right away, whatever happened), Spawned / Failed (once each, maybe
/// later with Retry), Entity, Success, Moved, Failure Reason.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_flow(
    gen: &Gen,
    id: &str,
    n: &Node,
    def: &'static NodeDef,
    b: &mut Out,
    pre: &[(String, i64, &str)],
    place: &str,
    create: &str,
    create_why: (i64, &str),
    apply: Option<String>,
    exit: &dyn Fn(&str) -> String,
    err: &dyn Fn(&str) -> String,
    next: &dyn Fn(&str) -> Option<String>,
    after: &mut Vec<String>,
) {
    let i = gen.index(id);
    let size = hull_size(n, def);
    let mode = blocked_mode(gen, n, def);
    let maxd = float_lit(gen.num(n, def, "max_dist").clamp(16.0, 1024.0));
    b.line(format!("\tnew Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == {i}"));
    b.line(format!("\tif (!lsgRe)\n\t\tg_lsgXrN[{i}] = 0\n\tg_lsgXrIn = -1"));
    let mut chain = String::new();
    for (k, (c, why, _)) in pre.iter().enumerate() {
        chain.push_str(&format!("\t{}if (!({c}))\n\t\tlsgWhy = {why}\n", if k == 0 { "" } else { "else " }));
    }
    // (Retry keeps the first try's spot: it waits for that spot to clear, not one that moves)
    let place = if mode == 4 { format!("(lsgRe ? lsg_xr_get({i}, o) : lsg_xr_put({i}, ({place}) ? 1 : 0, o))") } else { place.to_string() };
    chain.push_str(&format!("\t{}if (!({place}))\n\t\tlsgWhy = {WHY_BAD}\n", if pre.is_empty() { "" } else { "else " }));
    chain.push_str(&format!("\telse if (!lsg_xf_free(o, {size}, {mode}, {maxd}, lsgMv))\n\t\tlsgWhy = {WHY_BLOCKED}\n"));
    chain.push_str(&format!("\telse if (!(e = {create}))\n\t\tlsgWhy = {}", create_why.0));
    b.line(chain);
    if let Some(l) = apply {
        b.line(l);
    }
    if mode == 4 {
        let tries = gen.num(n, def, "retry_tries").round().clamp(0.0, 1000.0) as i64;
        let every = float_lit(gen.num(n, def, "retry_every").clamp(0.1, 60.0));
        b.line(format!("\tnew bool:lsgWait = !e && lsgWhy == {WHY_BLOCKED} && ({tries} == 0 || ++g_lsgXrN[{i}] < {tries})\n\tif (lsgWait)\n\t\tset_task({every}, \"lsg_xr{i}\", LSG_TASK + 410000 + {i}, ctx, LSG_CTX)\n\telse if (!e && lsgWhy == {WHY_BLOCKED})\n\t\tlsgWhy = {WHY_RETRIES}"));
        after.push(format!("public lsg_xr{i}(ctx[])\n{{\n\tg_lsgXrIn = {i}\n\t{}(ctx)\n}}", gen.pin_fname(id, "exec")));
    } else {
        b.line("\tnew bool:lsgWait = false");
    }
    b.line(format!("\tg_lsgOut[{i}] = e"));
    for (pin, val) in [("out", "e".to_string()), ("ok", "e ? 1 : 0".into()), ("moved", "e ? lsgMv : 0".into()), ("reason", "e ? 0 : lsgWhy".into())] {
        if let Some(k) = gen.out_index(id, pin) {
            b.line(format!("\tg_lsgO[{k}] = {val}"));
        }
    }
    // the debugger: what happened, in words
    if gen.debug {
        let mut words: Vec<(i64, String)> = pre.iter().map(|(_, w, m)| (*w, m.to_string())).collect();
        words.push((WHY_BAD, "No place: no such entity, or nothing wired into At.".into()));
        words.push((WHY_BLOCKED, "The spot is blocked (a player, a wall or something solid there) and no free spot was found.".into()));
        words.push((WHY_RETRIES, "The spot stayed blocked: every retry was used up.".into()));
        words.push((create_why.0, create_why.1.to_string()));
        let mut seen = std::collections::BTreeSet::new();
        let cases: String = words.iter().filter(|(w, _)| seen.insert(*w)).map(|(w, m)| format!("\t\t\tcase {w}: {}\n", err(m))).collect();
        let (moved, spawned) = (exit("moved to a nearby free spot"), exit("spawned"));
        b.line(format!("\tif (e)\n\t{{\n\t\tif (lsgMv)\n\t{moved}\n\t\telse\n\t{spawned}\n\t}}\n\telse if (lsgWait)\n\t{}\n\telse\n\t{{\n\t\tswitch (lsgWhy)\n\t\t{{\n{cases}\t\t}}\n\t}}", exit("blocked: trying again")));
    }
    // Spawned / Failed: once each; Then: right away (not again on a retry)
    let spawned = next("spawned").map(|c| format!("\n\t\t{c}")).unwrap_or_default();
    let failed = next("failed").map(|c| format!("\n\t\t{c}")).unwrap_or_default();
    if !spawned.is_empty() || !failed.is_empty() {
        b.line(format!("\tif (e)\n\t{{{spawned}\n\t}}\n\telse if (!lsgWait)\n\t{{{failed}\n\t}}"));
    }
    if let Some(c) = next("then") {
        b.line(format!("\tif (!lsgRe)\n\t\t{c}"));
    }
    if !gen.debug && spawned.is_empty() && failed.is_empty() {
        b.line("\t#pragma unused lsgWait");
    }
}

/// The older spawn nodes: saved graphs have no "Face the way At entity faces" setting, and for
/// them it stays off (they make the same plugin as before); new ones get it on.
const LEGACY_SPAWNERS: [&str; 5] = ["ent.spawn_prop", "creature.spawn", "npc.spawn", "npc.spawn_use", "comp.create"];

/// Does a spawn node turn its thing the way its At entity faces?
pub(super) fn copies(n: &Node, def: &NodeDef) -> bool {
    n.params.get("copy_rot").and_then(Value::as_bool).unwrap_or(!LEGACY_SPAWNERS.contains(&def.kind.as_str()))
}

/// Whether any spawn node in the graph uses the shared transform (the helpers are added then).
pub(crate) fn xf_used(g: &Graph) -> bool {
    // (every solid spawn node checks its spot: the helpers come with any spawn node)
    if g.nodes.iter().any(|n| STORE.contains(&n.kind.as_str())) {
        return true;
    }
    let wired = |id: &str, pin: &str| g.edges.iter().any(|e| e.to.node == id && e.to.pin == pin);
    g.nodes.iter().filter(|n| SPAWNERS.contains(&n.kind.as_str())).any(|n| {
        let at = if n.kind == "ent.spawn_prop" { "at" } else { "at_ent" };
        let legacy = LEGACY_SPAWNERS.contains(&n.kind.as_str());
        ["pos", "offset", "angles", "angles_offset", "vel", "scale"].iter().any(|p| wired(&n.id, p))
            || n.params.get("scale").and_then(Value::as_f64).is_some_and(|x| (x - 1.0).abs() > 1e-6)
            || (wired(&n.id, at) && n.params.get("copy_rot").and_then(Value::as_bool).unwrap_or(!legacy))
            || !legacy
    })
}

// ---- the nodes ---------------------------------------------------------------------------

/// Who a sound or message is for: 0 everyone, 1 a player, 2 a team (t=1, ct=2).
fn who(gen: &Gen, n: &Node, def: &'static NodeDef, id: &str) -> (i64, String) {
    match gen.choice(n, def, "who") {
        "player" => (1, gen.need(id, "player", "player")),
        "t" => (2, "1".into()),
        "ct" => (2, "2".into()),
        _ => (0, "0".into()),
    }
}

fn falloff(gen: &Gen, n: &Node, def: &'static NodeDef) -> &'static str {
    match gen.choice(n, def, "falloff") {
        "everywhere" => "0.0",
        "large" => "0.4",
        "small" => "1.25",
        _ => "0.8",
    }
}

fn channel(gen: &Gen, n: &Node, def: &'static NodeDef) -> i64 {
    match gen.choice(n, def, "channel") {
        "weapon" => 1,
        "voice" => 2,
        "item" => 3,
        "body" => 4,
        "stream" => 5,
        "static" => 6,
        _ => 0,
    }
}

/// The node's code (inside its function, before it goes on).
#[allow(clippy::too_many_arguments)]
pub(super) fn action(gen: &Gen, id: &str, n: &Node, def: &'static NodeDef, b: &mut Out, inp: &dyn Fn(&str, &str) -> String, exit: &dyn Fn(&str) -> String, err: &dyn Fn(&str) -> String, next: &dyn Fn(&str) -> Option<String>, after: &mut Vec<String>) {
    // (the spawn nodes go on to Then or Failed themselves; the rest go on to Then here)
    if !STORE.contains(&def.kind.as_str()) {
        action_inner(gen, id, n, def, b, inp, exit, err, next, after);
        if let Some(c) = next("then") {
            b.line(format!("\t{c}"));
        }
        return;
    }
    action_inner(gen, id, n, def, b, inp, exit, err, next, after);
}

#[allow(clippy::too_many_arguments)]
fn action_inner(gen: &Gen, id: &str, n: &Node, def: &'static NodeDef, b: &mut Out, inp: &dyn Fn(&str, &str) -> String, exit: &dyn Fn(&str) -> String, err: &dyn Fn(&str) -> String, next: &dyn Fn(&str) -> Option<String>, after: &mut Vec<String>) {
    let debug = gen.debug;
    let i = gen.index(id);
    let wired = |pin: &str| gen.v.data_source(id, pin).is_some();
    let num = |p: &str| if wired(p) { inp(p, "number") } else { float_lit(gen.num(n, def, p)) };
    // a call that answers 1 (done) or 0 (skipped), and what the debugger says either way
    let done = |b: &mut Out, call: &str, why: &str, ok: &str| {
        if debug {
            b.line(format!("\tif (!({call}))\n\t\t{}\n\telse\n\t{}", err(why), exit(ok)));
        } else {
            b.line(format!("\t{call}"));
        }
    };
    let pos_or = |pin: &str| if wired(pin) { inp(pin, "vector") } else { "-1".into() };
    let ent_or = |pin: &str| if wired(pin) { inp(pin, "entity") } else { "0".into() };
    // (effect messages a frame: the mode's "messages" limit, limits.rs; gen.rs's lsg_msg_room)
    let lim = gen.limit_expr("messages");
    let rgb = || format!("{}, {}, {}", gen.num(n, def, "r").round().clamp(0.0, 255.0) as i64, gen.num(n, def, "g").round().clamp(0.0, 255.0) as i64, gen.num(n, def, "b").round().clamp(0.0, 255.0) as i64);
    match def.kind.as_str() {
        "fx.dlight" => {
            let call = format!("lsg_fx_dlight({}, {}, {}, {}, {}, {}, {lim})", ent_or("follow"), pos_or("pos"), rgb(), num("radius"), num("life"), num("decay"));
            done(b, &call, "Skipped: no position and nothing to follow, or too many effects this frame (see the server log).", "lit");
        }
        "fx.decal" => {
            let names = DECALS.iter().find(|d| d.0 == gen.choice(n, def, "decal")).map(|d| d.2).unwrap_or(DECALS[0].2);
            let list = names.iter().map(|x| pawn_str(x)).collect::<Vec<_>>().join(", ");
            b.line(format!("\tstatic const lsgD[][] = {{ {list} }}"));
            let call = format!("lsg_fx_decal({}, lsgD[random_num(0, sizeof lsgD - 1)], {}, {lim})", pos_or("pos"), num("reach"));
            done(b, &call, "No decal: no surface within reach, the decal isn't in the game's decals.wad, or too many effects this frame.", "placed");
        }
        "fx.gibs" => {
            let custom = model_path(s(n, "custom", ""));
            let model = if custom.is_empty() { GIB_MODELS.iter().find(|x| x.0 == gen.choice(n, def, "model")).map(|x| x.2).unwrap_or("models/woodgibs.mdl").to_string() } else { custom };
            let mat = MATERIALS.iter().find(|x| x.0 == gen.choice(n, def, "material")).unwrap_or(&MATERIALS[4]);
            let at = if wired("at") { inp("at", "entity") } else { "0".into() };
            let call = format!(
                "lsg_fx_gibs({at}, {}, {}, {}, {}, clamp(floatround({}), 1, 255), {}, {}, {}, {lim})",
                pos_or("pos"),
                pawn_str(&model),
                mat.2,
                pawn_str(mat.3),
                num("count"),
                num("speed"),
                num("life"),
                num("size")
            );
            done(b, &call, "No gibs: no position, the model isn't loaded with the map, or too many effects this frame.", "broken");
        }
        "fx.ring" => {
            let at = if wired("at") { inp("at", "entity") } else { "0".into() };
            let call = format!("lsg_fx_ring({at}, {}, {}, {}, {}, clamp(floatround({}), 1, 255), {lim})", pos_or("pos"), num("radius"), rgb(), num("life"), num("width"));
            done(b, &call, "No ring: no position, or too many effects this frame.", "shown");
        }
        "snd.play_2d" => {
            let (w, who) = who(gen, n, def, id);
            let call = format!("lsg_snd_2d({w}, {who}, {}, {}, clamp(floatround({}), 50, 255), {lim})", pawn_str(&sound_path(gen.text(n, def, "sound"))), num("volume"), num("pitch"));
            done(b, &call, "Not played: the sound isn't on the server (loaded with the map), or nobody to hear it.", "played");
        }
        "snd.play_at" => {
            let call = format!("lsg_snd_at({}, {}, {}, {}, {}, clamp(floatround({}), 50, 255))", ent_or("at"), pos_or("pos"), pawn_str(&sound_path(gen.text(n, def, "sound"))), num("volume"), falloff(gen, n, def), num("pitch"));
            done(b, &call, "Not played: no position, or the sound isn't on the server (loaded with the map).", "played");
        }
        "snd.play_on" => {
            let call = format!("lsg_snd_on({}, {}, {}, {}, {}, clamp(floatround({}), 50, 255))", inp("ent", "entity"), channel(gen, n, def), pawn_str(&sound_path(gen.text(n, def, "sound"))), num("volume"), falloff(gen, n, def), num("pitch"));
            done(b, &call, "Not played: nothing there, or the sound isn't on the server (loaded with the map).", "played");
        }
        "snd.loop" => {
            // (how many at once: the mode's comfort limit, limits.rs "loop_sounds")
            let most = gen.limit_expr("loop_sounds");
            let call = format!(
                "(g_lsgOut[{i}] = lsg_snd_loop({}, {}, {}, {}, {}, clamp(floatround({}), 50, 255), {}, {most}))",
                ent_or("on"),
                pos_or("pos"),
                pawn_str(&sound_path(gen.text(n, def, "sound"))),
                num("volume"),
                falloff(gen, n, def),
                num("pitch"),
                num("every")
            );
            done(b, &call, "Not looping: no position or entity, the sound isn't on the server, the most loops at once are playing, or the map is near its entity limit.", "looping");
        }
        "snd.stop" => {
            let call = match gen.choice(n, def, "what") {
                "entity" => format!("lsg_snd_stop_on({}, {})", inp("ent", "entity"), channel(gen, n, def)),
                "player" => format!("lsg_snd_stop_player({})", inp("player", "player")),
                _ => format!("lsg_snd_stop_loop({})", inp("loop", "entity")),
            };
            done(b, &call, "Nothing stopped: no such loop, entity or player.", "stopped");
        }
        "ent.spawn_entity" => {
            let Some(c) = class_of(n) else {
                if debug {
                    b.line(format!("\t{}", err("That class isn't one Spawn Entity can make (see Problems for the list).")));
                }
                if let Some(f) = next("failed") {
                    b.line(format!("\t{f}"));
                }
                return;
            };
            let keys = spawn_keys(gen.text(n, def, "keys"));
            let (models, sounds) = key_files(&keys);
            // every file it loads must be loaded with the map already (else: skipped, not a crash)
            let mut need: Vec<String> = c.2.iter().map(|f| f.strip_prefix("sound/").map(|x| format!("lsg_have_snd({})", pawn_str(x))).unwrap_or_else(|| format!("lsg_have({})", pawn_str(f)))).collect();
            need.extend(models.iter().map(|m| format!("lsg_have({})", pawn_str(m))));
            need.extend(sounds.iter().map(|m| format!("lsg_have_snd({})", pawn_str(m))));
            let have = if need.is_empty() { "true".to_string() } else { need.join(" && ") };
            let fallback = if wired("at_ent") { format!("lsg_ent_origin({}, o)", inp("at_ent", "entity")) } else { "false".into() };
            let place = place(gen, id, "at_ent", &fallback);
            let kv = keys.iter().map(|(k, v)| format!("{}, {}", pawn_str(k), pawn_str(v))).collect::<Vec<_>>();
            // (the keys go in one call: a short helper with up to 32 pairs)
            let create = format!("lsg_spawn_kv({}, o{}{})", pawn_str(c.0), if kv.is_empty() { "" } else { ", " }, kv.join(", "));
            let pre = [(have, WHY_MISSING, "Not made: a file it needs isn't loaded with the map."), ("lsg_ent_room(\"Spawn Entity\")".to_string(), WHY_SLOTS, "Not made: the server is out of entity slots.")];
            spawn_flow(gen, id, n, def, b, &pre, &place, &create, (WHY_BAD, "The game didn't make it (its keys may be wrong for that class)."), apply(gen, n, def, id, "e", "at_ent", true, None), exit, err, next, after);
        }
        "ent.spawn_weapon" => {
            let item = WEAPON_ITEMS.iter().find(|x| x.0 == gen.choice(n, def, "item")).unwrap_or(&WEAPON_ITEMS[4]);
            let ammo = if wired("ammo") { format!("floatround({})", inp("ammo", "number")) } else { (gen.num(n, def, "ammo").round() as i64).clamp(-1, 1000).to_string() };
            let fallback = if wired("at_ent") { format!("lsg_ent_origin({}, o)", inp("at_ent", "entity")) } else { "false".into() };
            let place = place(gen, id, "at_ent", &fallback);
            let create = format!("lsg_weapon_drop({}, {}, o, {ammo}, {})", item.2, pawn_str(item.3), num("respawn"));
            let pre = [(format!("lsg_have({})", pawn_str(&item_model(item.0))), WHY_MISSING, "Not made: its model isn't loaded with the map."), ("lsg_ent_room(\"Spawn Weapon On Ground\")".to_string(), WHY_SLOTS, "Not made: the server is out of entity slots.")];
            spawn_flow(gen, id, n, def, b, &pre, &place, &create, (WHY_BAD, "The game didn't make it."), apply(gen, n, def, id, "e", "at_ent", true, None), exit, err, next, after);
        }
        _ => {}
    }
}

/// The helpers and hooks (once, when a node here or a spawn node is used).
pub(super) fn blocks(g: &Graph, o: &mut Out) {
    if !g.nodes.iter().any(|n| is_action(&n.kind)) && !xf_used(g) {
        return;
    }
    o.line(RUNTIME);
    if uses(g, "ent.spawn_weapon") {
        o.line(WEAPONS_RUNTIME);
    }
}

/// After gen.rs's creature helpers (it uses their pay table): Spawn Creature at a place.
pub(super) fn creature_block(g: &Graph, o: &mut Out) {
    if xf_used(g) && uses(g, "creature.spawn") {
        o.line(CREATURE_O2);
    }
}

/// Inside plugin_init.
pub(super) fn init(g: &Graph, o: &mut Out) {
    if uses(g, "ent.spawn_weapon") {
        o.line("\tRegisterHam(Ham_Touch, \"armoury_entity\", \"lsg_wd_touch_pre\", 0)\n\tRegisterHam(Ham_Touch, \"armoury_entity\", \"lsg_wd_touch\", 1)");
    }
}

/// Inside plugin_precache: only files that are there (a missing one is logged and the node skips).
pub(super) fn precache(g: &Graph) -> Vec<String> {
    let (m, snd) = files(g);
    let mut out: Vec<String> = m.iter().map(|x| format!("\tlsg_pre_model({})", pawn_str(x))).collect();
    out.extend(snd.iter().map(|x| format!("\tlsg_pre_sound({})", pawn_str(x))));
    out
}

pub(super) fn needs_precache(g: &Graph) -> bool {
    let (m, snd) = files(g);
    !m.is_empty() || !snd.is_empty()
}

fn err(node: &str, msg: &str) -> Problem {
    Problem { level: "error", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

fn warn(node: &str, msg: &str) -> Problem {
    Problem { level: "warning", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

/// The Problems checks.
pub(crate) fn problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    let num = |n: &Node, def: &NodeDef, k: &str, d: f64| super::super::graph::param(n, def, k).and_then(Value::as_f64).unwrap_or(d);
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        let wired = |pin: &str| v.data_source(&n.id, pin).is_some();
        // (the superseded sound nodes aren't flagged: saved graphs and templates stay clean; the
        // node list leaves them out and their help names the new one)
        // the shared transform: what a spawn node can't use
        if SPAWNERS.contains(&def.kind.as_str()) {
            let brushy = def.kind == "comp.create";
            if brushy && (wired("scale") || (num(n, def, "scale", 1.0) - 1.0).abs() > 1e-6) {
                p.push(warn(&n.id, "Scale isn't used here: a component is a zone or a brush thing, not a model."));
            }
            if brushy && wired("vel") {
                p.push(warn(&n.id, "Velocity isn't used here: components stay where they're made."));
            }
            if wired("pos") && wired(if def.kind == "ent.spawn_prop" { "at" } else { "at_ent" }) {
                p.push(warn(&n.id, "Both At entity and At position are wired: At position wins (Local offset works with At entity)."));
            }
        }
        match def.kind.as_str() {
            "ent.spawn_entity" => {
                let c = n.params.get("class").and_then(Value::as_str).unwrap_or("env_sprite");
                if !SPAWN_CLASSES.iter().any(|x| x.0 == c) {
                    let list = SPAWN_CLASSES.iter().map(|x| x.0).collect::<Vec<_>>().join(", ");
                    p.push(err(&n.id, &format!("Spawn Entity can't make {c}. It makes only these (players, worldspawn, game_* server controls, anything that runs commands or changes the map, and brush entities are never allowed): {list}.")));
                }
                let text = n.params.get("keys").and_then(Value::as_str).unwrap_or("");
                let kept = spawn_keys(text);
                let given = text.split(['\n', '|']).filter(|x| x.contains('=')).count();
                if kept.len() < given {
                    p.push(warn(&n.id, "Some keys were left out: classname, origin and angles come from the node, and a key is letters, digits and _ only."));
                }
                if c == "env_beam" || c == "env_laser" {
                    if !kept.iter().any(|(k, v)| k.eq_ignore_ascii_case("texture") && v.ends_with(".spr")) {
                        p.push(err(&n.id, "A beam needs texture=sprites/...spr in Keys, or it removes itself."));
                    }
                }
            }
            "fx.gibs" if num(n, def, "count", 8.0) > 64.0 => p.push(warn(&n.id, "Over 64 gibs at once can make slower computers stutter (the game draws up to 255).")),
            "snd.play_2d" | "snd.play_at" | "snd.play_on" | "snd.loop" => {
                let snd = n.params.get("sound").and_then(Value::as_str).unwrap_or("ambience/wind1.wav");
                if sound_path(snd).is_empty() {
                    p.push(err(&n.id, "Sound is a .wav file under sound/ (like ambience/wind1.wav, or custom/mine.wav)."));
                }
            }
            _ => {}
        }
    }
}

const RUNTIME: &str = r#"// ---- effects, sounds and spawning (fx.rs) ----
// the engine's crash points are gen.rs's shared guards: lsg_msg_room (effect messages a frame,
// the mode's "messages" limit) and lsg_ent_room (entity slots with a reserve)
stock g_lsgXfSrc

stock bool:lsg_xf_from(src, bool:found)
{
	g_lsgXfSrc = src
	return found
}

// a prop at a place (Spawn Prop with the shared transform)
stock lsg_prop_o(const Float:o[3], const model[], solid)
{
	if (!lsg_have(model))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_prop")
	engfunc(EngFunc_SetModel, e, model)
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, solid ? SOLID_BBOX : SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	if (solid)
		engfunc(EngFunc_SetSize, e, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 32.0})
	return e
}

stock bool:lsg_have_snd(const p[])
{
	new full[128]
	formatex(full, charsmax(full), "sound/%s", p)
	return lsg_have(full) || lsg_have(p)
}

stock lsg_wcoord(const Float:o[3])
{
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
}

// where a spawn goes: an entity's spot (plus an offset along its own forward, right and up),
// or a position; remembers the entity for its rotation
stock bool:lsg_xf_at(src, pos, ofs, Float:o[3])
{
	g_lsgXfSrc = 0
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		return true
	}
	if (!lsg_ent_origin(src, o))
		return false
	g_lsgXfSrc = src
	if (ofs >= 0)
	{
		new Float:a[3], Float:f[3], Float:r[3], Float:u[3]
		pev(src, is_user_alive(src) ? pev_v_angle : pev_angles, a)
		engfunc(EngFunc_MakeVectors, a)
		global_get(glb_v_forward, f)
		global_get(glb_v_right, r)
		global_get(glb_v_up, u)
		for (new k = 0; k < 3; k++)
			o[k] += f[k] * lsg_vc(ofs, 0) + r[k] * lsg_vc(ofs, 1) + u[k] * lsg_vc(ofs, 2)
	}
	return true
}

// turn, push and size a new entity: Rotation, else the At entity's facing (if copied) plus the
// rotation offset; Velocity (a still thing falls and slides from there); Scale (models, sprites)
stock lsg_xf_set(e, copy, rot, rot_ofs, vel, Float:scale)
{
	if (e <= 0 || !pev_valid(e))
		return
	new Float:a[3], bool:turn = false
	if (rot >= 0)
	{
		lsg_vto(rot, a)
		turn = true
	}
	else if (copy && g_lsgXfSrc > 0 && pev_valid(g_lsgXfSrc))
	{
		pev(g_lsgXfSrc, pev_angles, a)
		// (a player's model pitch isn't where they look: keep their heading)
		if (is_user_alive(g_lsgXfSrc))
			a[0] = 0.0
		turn = true
	}
	else
		pev(e, pev_angles, a)
	if (rot_ofs >= 0)
	{
		for (new k = 0; k < 3; k++)
			a[k] += lsg_vc(rot_ofs, k)
		turn = true
	}
	if (turn)
	{
		set_pev(e, pev_angles, a)
		if (pev(e, pev_flags) & FL_MONSTER)
			set_pev(e, pev_ideal_yaw, a[1])
	}
	if (vel >= 0)
	{
		new Float:v[3]
		lsg_vto(vel, v)
		if (pev(e, pev_movetype) == MOVETYPE_NONE)
			set_pev(e, pev_movetype, MOVETYPE_TOSS)
		set_pev(e, pev_velocity, v)
	}
	if (scale > 0.0 && scale != 1.0)
		set_pev(e, pev_scale, floatclamp(scale, 0.05, 10.0))
}

// ---- effects
stock g_lsgDlFollow[33], Float:g_lsgDlEnd[33], g_lsgDl[33][5], g_lsgDlN, g_lsgDlLim[33]

public lsg_fx_dl_tick(t)
{
	// (the task's id: its slot plus the base)
	new k = clamp(t - (LSG_TASK - 200000), 0, 31)
	new e = g_lsgDlFollow[k]
	if (e <= 0 || !pev_valid(e) || get_gametime() >= g_lsgDlEnd[k])
	{
		remove_task(k + LSG_TASK - 200000)
		g_lsgDlFollow[k] = 0
		return
	}
	new Float:o[3]
	if (!lsg_ent_origin(e, o) || !lsg_msg_room("Dynamic Light", 1, 0, g_lsgDlLim[k]))
		return
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_DLIGHT)
	lsg_wcoord(o)
	write_byte(g_lsgDl[k][0])
	write_byte(g_lsgDl[k][1])
	write_byte(g_lsgDl[k][2])
	write_byte(g_lsgDl[k][3])
	write_byte(2)
	write_byte(g_lsgDl[k][4])
	message_end()
}

stock lsg_fx_dlight(follow, pos, r, g, b, Float:radius, Float:life, Float:decay, lim)
{
	new rad = clamp(floatround(radius / 10.0), 1, 255), dec = clamp(floatround(decay / 10.0), 0, 255)
	if (follow > 0 && pev_valid(follow))
	{
		// following: a short light each tenth of a second where it is (32 at once at most)
		new k = g_lsgDlN++ % 32
		remove_task(k + LSG_TASK - 200000)
		g_lsgDlFollow[k] = follow
		g_lsgDlEnd[k] = get_gametime() + floatclamp(life, 0.1, 60.0)
		// (as the message wants them: radius, r, g, b, then the decay after the life)
		g_lsgDl[k][0] = rad
		g_lsgDl[k][1] = r
		g_lsgDl[k][2] = g
		g_lsgDl[k][3] = b
		g_lsgDl[k][4] = dec
		g_lsgDlLim[k] = lim
		lsg_fx_dl_tick(k + LSG_TASK - 200000)
		set_task(0.1, "lsg_fx_dl_tick", k + LSG_TASK - 200000, _, _, "b")
		return 1
	}
	if (pos < 0 || !lsg_msg_room("Dynamic Light", 1, 0, lim))
		return 0
	new Float:o[3]
	lsg_vto(pos, o)
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_DLIGHT)
	lsg_wcoord(o)
	write_byte(rad)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(dec)
	message_end()
	return 1
}

// a decal on the nearest surface within reach (traced six ways from the point)
stock lsg_fx_decal(pos, const name[], Float:reach, lim)
{
	if (pos < 0)
		return 0
	new d = engfunc(EngFunc_DecalIndex, name)
	if (d < 0)
		return 0
	new Float:o[3], Float:end[3], Float:hit[3], Float:best = 2.0, Float:at[3], who = 0
	static const Float:DIRS[6][3] = { {0.0, 0.0, -1.0}, {0.0, 0.0, 1.0}, {1.0, 0.0, 0.0}, {-1.0, 0.0, 0.0}, {0.0, 1.0, 0.0}, {0.0, -1.0, 0.0} }
	lsg_vto(pos, o)
	reach = floatclamp(reach, 8.0, 512.0)
	for (new k = 0; k < 6; k++)
	{
		for (new j = 0; j < 3; j++)
			end[j] = o[j] + DIRS[k][j] * reach
		engfunc(EngFunc_TraceLine, o, end, IGNORE_MONSTERS, 0, 0)
		new Float:f
		get_tr2(0, TR_flFraction, f)
		if (f < best)
		{
			best = f
			get_tr2(0, TR_vecEndPos, hit)
			at = hit
			who = max(get_tr2(0, TR_pHit), 0)
		}
	}
	if (best >= 1.0 || !lsg_msg_room("Decal", 1, 0, lim))
		return 0
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	if (who > 0)
	{
		write_byte(d > 255 ? TE_DECALHIGH : TE_DECAL)
		lsg_wcoord(at)
		write_byte(d & 255)
		write_short(who)
	}
	else
	{
		write_byte(d > 255 ? TE_WORLDDECALHIGH : TE_WORLDDECAL)
		lsg_wcoord(at)
		write_byte(d & 255)
	}
	message_end()
	return 1
}

// pieces of a model flying out from a point (or an entity's middle), with the material's sound
stock lsg_fx_gibs(at, pos, const model[], flags, const snd[], count, Float:speed, Float:life, Float:size, lim)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	if (!lsg_have(model) || !lsg_msg_room("Break Into Gibs", 2, 0, lim))
		return 0
	new Float:sz = floatclamp(size, 1.0, 512.0)
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_BREAKMODEL)
	lsg_wcoord(o)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, 0.0)
	engfunc(EngFunc_WriteCoord, 0.0)
	engfunc(EngFunc_WriteCoord, floatclamp(speed, 0.0, 2000.0) * 0.5)
	write_byte(clamp(floatround(speed / 10.0), 0, 255))
	write_short(engfunc(EngFunc_ModelIndex, model))
	write_byte(count)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(flags)
	message_end()
	if (snd[0] && lsg_have_snd(snd))
		engfunc(EngFunc_EmitAmbientSound, 0, o, snd, 1.0, ATTN_NORM, 0, PITCH_NORM)
	return 1
}

// a shockwave ring growing from a point (or an entity's spot)
stock lsg_fx_ring(at, pos, Float:radius, r, g, b, Float:life, width, lim)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	new spr = lsg_have("sprites/shockwave.spr") ? engfunc(EngFunc_ModelIndex, "sprites/shockwave.spr") : (lsg_have("sprites/laserbeam.spr") ? engfunc(EngFunc_ModelIndex, "sprites/laserbeam.spr") : 0)
	if (!spr || !lsg_msg_room("Beam Ring", 1, 0, lim))
		return 0
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_BEAMCYLINDER)
	lsg_wcoord(o)
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2] + floatclamp(radius, 8.0, 4096.0))
	write_short(spr)
	write_byte(0)
	write_byte(0)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(width)
	write_byte(0)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(200)
	write_byte(0)
	message_end()
	return 1
}

// ---- sounds (map sounds are best mono: a stereo .wav is heard in the middle, not where it is)
// heard as is, not from a place: a player, a team (1 T, 2 CT) or everyone; volume and pitch go
// through the game's speaker command (v = volume %, p = pitch %)
stock lsg_snd_2d(who, arg, const snd[], Float:vol, pitch, lim)
{
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	new name[72], n = 0
	copy(name, charsmax(name), snd)
	new len = strlen(name)
	if (len > 4 && equali(name[len - 4], ".wav"))
		name[len - 4] = 0
	new v = clamp(floatround(vol * 100.0), 0, 100)
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		if ((who == 1 && p != arg) || (who == 2 && get_user_team(p) != arg))
			continue
		if (!lsg_msg_room("Play Sound 2D", 1, p, lim))
			continue
		client_cmd(p, "spk ^"(v%d p%d) %s^"", v, pitch, name)
		n++
	}
	return n > 0 || who == 0
}

// at a place (a position, else an entity's spot, staying there)
stock lsg_snd_at(at, pos, const snd[], Float:vol, Float:attn, pitch)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	engfunc(EngFunc_EmitAmbientSound, 0, o, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	return 1
}

// on an entity's channel: it follows the entity
stock lsg_snd_on(e, chan, const snd[], Float:vol, Float:attn, pitch)
{
	if (e <= 0 || !pev_valid(e) || !snd[0] || !lsg_have_snd(snd))
		return 0
	emit_sound(e, chan, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	return 1
}

// a looping sound: an emitter at a spot, or riding on an entity; the file loops by itself if it
// has loop points, else it's played again every Every seconds
stock g_lsgLoops
public lsg_snd_again(t)
{
	new e = t - (LSG_TASK - 300000)
	if (e <= MaxClients || !pev_valid(e))
	{
		remove_task(t)
		return
	}
	new snd[72]
	pev(e, pev_noise, snd, charsmax(snd))
	emit_sound(e, CHAN_STATIC, snd, entity_get_float(e, EV_FL_fuser1), entity_get_float(e, EV_FL_fuser2), 0, pev(e, pev_iuser1))
}

stock lsg_snd_loop(on, pos, const snd[], Float:vol, Float:attn, pitch, Float:every, most)
{
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	if (g_lsgLoops >= most)
	{
		log_amx("[%s] Loop Sound skipped: %d are playing (the mode's Loop sounds at once limit)", LSG_MODE, most)
		return 0
	}
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(on, o))
		return 0
	if (!lsg_ent_room("Loop Sound"))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_loop")
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_effects, EF_NODRAW)
	if (pos < 0 && on > 0)
	{
		set_pev(e, pev_movetype, MOVETYPE_FOLLOW)
		set_pev(e, pev_aiment, on)
	}
	else
		set_pev(e, pev_movetype, MOVETYPE_NONE)
	set_pev(e, pev_noise, snd)
	set_pev(e, pev_fuser1, floatclamp(vol, 0.0, 1.0))
	set_pev(e, pev_fuser2, attn)
	set_pev(e, pev_iuser1, pitch)
	emit_sound(e, CHAN_STATIC, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	if (every > 0.0)
		set_task(floatclamp(every, 0.5, 600.0), "lsg_snd_again", e + LSG_TASK - 300000, _, _, "b")
	g_lsgLoops++
	return e
}

stock lsg_snd_stop_loop(e)
{
	if (e <= MaxClients || !pev_valid(e))
		return 0
	new cls[24]
	pev(e, pev_classname, cls, charsmax(cls))
	if (!equal(cls, "ls_studio_loop"))
		return 0
	new snd[72]
	pev(e, pev_noise, snd, charsmax(snd))
	emit_sound(e, CHAN_STATIC, snd, 0.0, ATTN_NORM, SND_STOP, PITCH_NORM)
	remove_task(e + LSG_TASK - 300000)
	engfunc(EngFunc_RemoveEntity, e)
	if (g_lsgLoops > 0)
		g_lsgLoops--
	return 1
}

stock lsg_snd_stop_on(e, chan)
{
	if (e <= 0 || !pev_valid(e) || !lsg_have_snd("common/null.wav"))
		return 0
	emit_sound(e, chan, "common/null.wav", 0.0, ATTN_NORM, SND_STOP, PITCH_NORM)
	return 1
}

stock lsg_snd_stop_player(p)
{
	if (!lsg_ok(p))
		return 0
	client_cmd(p, "stopsound")
	return 1
}

// ---- spawning: a class from Spawn Entity's list, its keys, then spawned
stock lsg_spawn_kv(const cls[], const Float:o[3], const k1[] = "", const v1[] = "", ...)
{
	new e = create_entity(cls)
	if (e <= 0)
		return 0
	engfunc(EngFunc_SetOrigin, e, o)
	if (k1[0])
		DispatchKeyValue(e, k1, v1)
	// (the rest, in pairs)
	new n = numargs(), key[64], val[128]
	for (new a = 4; a + 1 < n; a += 2)
	{
		lsg_argstr(a, key, charsmax(key))
		lsg_argstr(a + 1, val, charsmax(val))
		if (key[0])
			DispatchKeyValue(e, key, val)
	}
	DispatchSpawn(e)
	return pev_valid(e) && !(pev(e, pev_flags) & FL_KILLME) ? e : 0
}

stock lsg_argstr(a, s[], len)
{
	new i = 0
	for (; i < len; i++)
	{
		s[i] = getarg(a, i)
		if (!s[i])
			break
	}
	s[min(i, len)] = 0
}

// ---- solid spawns: is the spot free (a hull the size of the thing, players and walls count)?
// If not: the nearest free spot within reach (rings round it, in sight of it), or nothing.
stock g_lsgXrN[LSG_N], g_lsgXrIn = -1, Float:g_lsgXrO[LSG_N][3]

// (Retry: the spot of the first try, kept)
stock lsg_xr_put(i, ok, const Float:o[3])
{
	if (ok)
	{
		g_lsgXrO[i][0] = o[0]
		g_lsgXrO[i][1] = o[1]
		g_lsgXrO[i][2] = o[2]
	}
	return ok
}

stock lsg_xr_get(i, Float:o[3])
{
	o[0] = g_lsgXrO[i][0]
	o[1] = g_lsgXrO[i][1]
	o[2] = g_lsgXrO[i][2]
	return 1
}

stock bool:lsg_xf_clear(const Float:o[3], size)
{
	new Float:c[3]
	c[0] = o[0]
	c[1] = o[1]
	c[2] = o[2] + (size >= 2 ? 37.0 : 19.0)
	engfunc(EngFunc_TraceHull, c, c, DONT_IGNORE_MONSTERS, size >= 2 ? HULL_HUMAN : HULL_HEAD, 0, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid)
}

// mode: 0 always spawn (no check), 1 a nearby spot or in place anyway, 2 a nearby spot or not at
// all, 3 exactly there or not at all, 4 exactly there (the node retries)
stock bool:lsg_xf_free(Float:o[3], size, mode, Float:maxd, &moved)
{
	moved = 0
	if (size < 0 || mode == 0 || lsg_xf_clear(o, size))
		return true
	if (mode >= 3)
		return false
	new Float:c[3], Float:from[3], Float:to[3], Float:f
	from[0] = o[0]
	from[1] = o[1]
	from[2] = o[2] + 18.0
	for (new Float:r = 16.0; r <= maxd; r += 16.0)
	{
		new steps = clamp(floatround(r / 6.0), 8, 32)
		for (new k = 0; k < steps; k++)
		{
			new Float:ang = 6.2831853 * float(k) / float(steps)
			for (new z = 0; z < 3; z++)
			{
				c[0] = o[0] + floatcos(ang) * r
				c[1] = o[1] + floatsin(ang) * r
				c[2] = o[2] + float(z) * 18.0
				if (!lsg_xf_clear(c, size))
					continue
				to[0] = c[0]
				to[1] = c[1]
				to[2] = c[2] + 18.0
				engfunc(EngFunc_TraceLine, from, to, IGNORE_MONSTERS, 0, 0)
				get_tr2(0, TR_flFraction, f)
				if (f < 1.0)
					continue
				o[0] = c[0]
				o[1] = c[1]
				o[2] = c[2]
				moved = 1
				return true
			}
		}
	}
	// (mode 1: no free spot near, so in place anyway)
	return mode == 1
}
"#;

/// Spawn Creature at a place (the shared transform), as lsg_creature_x does at a named one.
const CREATURE_O2: &str = r#"stock lsg_creature_o2(const kind[], const Float:o[3], tier, Float:hp, pay)
{
	if (!get_cvar_pointer("ls_creature_ver"))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	if (tier > 0)
	{
		new v[4]
		num_to_str(tier, v, charsmax(v))
		DispatchKeyValue(ent, "tier", v)
	}
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	if (!is_valid_ent(ent) || (entity_get_int(ent, EV_INT_flags) & FL_KILLME))
		return 0
	if (hp > 0.0)
	{
		entity_set_float(ent, EV_FL_health, hp)
		entity_set_float(ent, EV_FL_max_health, hp)
	}
	if (ent < sizeof g_lsgPay)
		g_lsgPay[ent] = clamp(pay, 0, 16000)
	g_lsgCrAt = -1.0
	return ent
}
"#;

const WEAPONS_RUNTIME: &str = r#"// Spawn Weapon On Ground: an armoury_entity (picked up like a map's weapon), with its own reserve
// ammo given on pickup and, if set, back after some seconds (else at the next round, as the game does)
#define LSG_WD_MARK 76531
stock g_lsgWdWas[33]

stock lsg_weapon_drop(item, const wclass[], const Float:o[3], ammo, Float:respawn)
{
	new e = create_entity("armoury_entity")
	if (e <= 0)
		return 0
	new v[8]
	num_to_str(item, v, charsmax(v))
	DispatchKeyValue(e, "item", v)
	DispatchKeyValue(e, "count", "1")
	engfunc(EngFunc_SetOrigin, e, o)
	DispatchSpawn(e)
	if (!pev_valid(e))
		return 0
	set_pev(e, pev_iuser3, LSG_WD_MARK)
	set_pev(e, pev_iuser4, wclass[0] && ammo >= 0 ? ammo + 1 : 0)
	set_pev(e, pev_fuser4, floatmax(respawn, 0.0))
	set_pev(e, pev_noise1, wclass)
	return e
}

public lsg_wd_touch_pre(e, p)
{
	if (p >= 1 && p <= MaxClients && pev(e, pev_iuser3) == LSG_WD_MARK)
		g_lsgWdWas[p] = (pev(e, pev_effects) & EF_NODRAW) ? 1 : 0
}

public lsg_wd_touch(e, p)
{
	if (p < 1 || p > MaxClients || pev(e, pev_iuser3) != LSG_WD_MARK || g_lsgWdWas[p] || !(pev(e, pev_effects) & EF_NODRAW))
		return
	// picked up just now
	new ammo = pev(e, pev_iuser4) - 1
	if (ammo >= 0)
	{
		new w[24]
		pev(e, pev_noise1, w, charsmax(w))
		new id = get_weaponid(w)
		if (id > 0)
			cs_set_user_bpammo(p, id, ammo)
	}
	new Float:back = entity_get_float(e, EV_FL_fuser4)
	if (back > 0.0)
		set_task(back, "lsg_wd_back", e + LSG_TASK - 400000)
}

public lsg_wd_back(t)
{
	new e = t - (LSG_TASK - 400000)
	if (pev_valid(e) && pev(e, pev_iuser3) == LSG_WD_MARK)
		ExecuteHamB(Ham_CS_Restart, e)
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_keys_keep_only_safe_pairs() {
        let k = spawn_keys("model=sprites/glow01.spr|classname=player\nrendercolor=255 0 0\nbad key=1\nmessage=say \"hi\"^x\norigin=1 2 3");
        assert_eq!(k, vec![("model".into(), "sprites/glow01.spr".into()), ("rendercolor".into(), "255 0 0".into()), ("badkey".into(), "1".into()), ("message".into(), "say hix".into())]);
        let (m, s) = key_files(&[("model".into(), "sprites/glow01.spr".into()), ("message".into(), "ambience/wind1.wav".into()), ("x".into(), "models/../../x.mdl".into())]);
        assert_eq!(m, vec!["sprites/glow01.spr"]);
        assert_eq!(s, vec!["ambience/wind1.wav"]);
    }

    #[test]
    fn no_command_or_server_control_class_is_spawnable() {
        for (c, _, _) in SPAWN_CLASSES {
            assert!(!c.starts_with("game_") && !c.starts_with("point_") && !c.starts_with("func_") && c != "player" && c != "worldspawn" && !c.contains("changelevel"), "{c}");
        }
    }

    /// The code of one node in a generated plugin (its span).
    fn code_of(src: &str, spans: &[super::super::Span], id: &str) -> String {
        let lines: Vec<&str> = src.lines().collect();
        spans.iter().filter(|s| s.node == id).map(|s| lines[s.from.saturating_sub(1)..s.to.min(lines.len())].join("\n")).collect::<Vec<_>>().join("\n")
    }

    /// Every spawn node takes the shared transform: At entity + Local offset, At position,
    /// Rotation / Rotation offset, Velocity, Scale (not for components); saved graphs' own pins
    /// (a prop's At, an NPC's Facing) still work.
    #[test]
    fn every_spawn_node_takes_the_transform() {
        let g = crate::tools::studio::graph::parse(include_str!("../testdata/every_node_fx.graph.json")).unwrap();
        let r = super::super::generate(&g, false);
        let c = |id: &str| code_of(&r.source, &r.spans, id);
        // at an entity with an offset, pushed: a spawned entity, a weapon, a prop, a creature
        for id in ["se", "sw", "pr", "cr"] {
            let code = c(id);
            assert!(code.contains("lsg_xf_at(ctx[0], -1, lsg_vec(32.0, 0.0, 8.0), o)"), "{id}: {code}");
            assert!(code.contains("lsg_xf_set("), "{id}: {code}");
        }
        assert!(c("se").contains(", 2.0)"), "Spawn Entity's Scale: {}", c("se"));
        assert!(c("cr").contains("lsg_creature_o2(\"zombie\", o,"), "{}", c("cr"));
        assert!(c("pr").contains("lsg_prop_o(o, "), "{}", c("pr"));
        // (a saved graph's prop at an entity, no new setting: its spot, not turned; the spot checked)
        assert!(c("pr3").contains("lsg_ent_origin(ctx[0], o)") && c("pr3").contains("lsg_xf_free(o, 1, 2, 128.0, lsgMv)") && !c("pr3").contains("lsg_xf_set("), "{}", c("pr3"));
        // at a position, sized
        assert!(c("pr2").contains("lsg_xf_at(0, ") && c("pr2").contains(", 1.5)"), "{}", c("pr2"));
        // an NPC: Facing is a yaw offset on top of the At entity's facing
        assert!(c("np").contains("lsg_vec(0.0, 45.0, 0.0)"), "{}", c("np"));
        // an interactable at a position, turned
        assert!(c("iu").contains("lsg_xf_at(0, ") && c("iu").contains("lsg_xf_set("), "{}", c("iu"));
        // a component turns, never scaled or pushed
        let cc = c("cc");
        assert!(cc.contains("lsg_vec(0.0, 90.0, 0.0), -1, -1, 1.0)"), "{cc}");
        // Spawn Entity only makes its class, with its keys, after checking its files
        assert!(c("se").contains("lsg_spawn_kv(\"env_sprite\", o, \"model\", \"sprites/glow01.spr\""), "{}", c("se"));
        assert!(c("se").contains("lsg_have(\"sprites/glow01.spr\")"), "{}", c("se"));
        assert!(c("se3").contains("lsg_have_snd(\"buttons/spark1.wav\")"), "{}", c("se3"));
        // and its files are loaded with the map
        assert!(r.source.contains("lsg_pre_model(\"sprites/glow01.spr\")") && r.source.contains("lsg_pre_sound(\"buttons/spark1.wav\")"));
        assert!(r.source.contains("lsg_pre_model(\"models/w_awp.mdl\")") && r.source.contains("lsg_pre_sound(\"custom/fx_sting.wav\")"));
    }

    /// If the spot is blocked: the nearest free spot (Moved), Don't spawn (Failed), Retry (a timer
    /// runs the node again, 0 tries: until clear); non-solid things aren't checked.
    #[test]
    fn blocked_spots_move_fail_or_retry() {
        let g = crate::tools::studio::graph::parse(include_str!("../testdata/every_node_fx.graph.json")).unwrap();
        let r = super::super::generate(&g, false);
        let c = |id: &str| code_of(&r.source, &r.spans, id);
        // each way of handling a blocked spot (mode 0-4), at the bot itself
        assert!(c("pforce").contains("lsg_xf_free(o, 1, 0, "), "{}", c("pforce"));
        assert!(c("palways").contains("lsg_xf_free(o, 1, 1, 64.0, lsgMv)"), "{}", c("palways"));
        assert!(c("pnear").contains("lsg_xf_free(o, 0, 2, 96.0, lsgMv)") && c("pnear").contains("e ? lsgMv : 0"), "{}", c("pnear"));
        assert!(c("pfail").contains("lsg_xf_free(o, 1, 3, "), "{}", c("pfail"));
        let cr = c("cretry");
        assert!(cr.contains("lsg_xf_free(o, 2, 4, ") && cr.contains("(0 == 0 || ++g_lsgXrN[") && cr.contains("lsgWhy = 5"), "{cr}");
        assert!(r.source.contains("public lsg_xr") && r.source.contains("set_task(1.0, \"lsg_xr"));
        // Then isn't run again by a retry
        assert!(cr.contains("if (!lsgRe)"), "{cr}");
        // a retry waits for the first try's spot (not the entity's spot now)
        assert!(cr.contains("(lsgRe ? lsg_xr_get("), "{cr}");
        // a sprite isn't solid: never moved; a non-solid prop neither
        assert!(c("se").contains("lsg_xf_free(o, -1, "), "{}", c("se"));
        assert!(c("pr").contains("lsg_xf_free(o, -1, "), "{}", c("pr"));
    }

    /// The fallback chain: exactly at the bot, and on Failed at a point; the reason goes on.
    #[test]
    fn a_failed_spawn_can_fall_back() {
        let g = crate::tools::studio::graph::parse(include_str!("../testdata/every_node_fx.graph.json")).unwrap();
        let r = super::super::generate(&g, false);
        let code = code_of(&r.source, &r.spans, "pfail");
        let b_fn = code_of(&r.source, &r.spans, "pB").lines().find(|l| l.trim_end().ends_with("(ctx[])")).map(|l| l.trim().trim_end_matches("(ctx[])").trim_start_matches("public ").to_string()).unwrap();
        assert!(code.contains(&format!("else if (!lsgWait)\n\t{{\n\t\t{b_fn}(ctx)")), "{code}");
        // Failure Reason is a cell the Switch on Enum reads (the Spawn Failure enum)
        assert!(code.contains("? 0 : lsgWhy"), "{code}");
        let p = crate::tools::studio::graph::validate_core(&g);
        assert!(p.is_empty(), "{p:#?}");
    }

    /// A class off the list never becomes code; Problems lists what's allowed.
    #[test]
    fn spawn_entity_refuses_other_classes() {
        let text = include_str!("../testdata/every_node_fx.graph.json").replace("\"class\": \"env_spark\"", "\"class\": \"point_servercommand\"");
        let g = crate::tools::studio::graph::parse(&text).unwrap();
        let p = crate::tools::studio::graph::validate(&g);
        assert!(p.iter().any(|x| x.level == "error" && x.message.contains("can't make point_servercommand") && x.message.contains("env_sprite")), "{p:#?}");
        let src = super::super::generate(&g, false).source;
        assert!(!src.contains("point_servercommand"));
    }

    #[test]
    fn sound_paths() {
        assert_eq!(sound_path("sound/ambience/wind1.wav"), "ambience/wind1.wav");
        assert_eq!(sound_path("custom/x.wav"), "custom/x.wav");
        assert_eq!(sound_path("../x.wav"), "");
        assert_eq!(sound_path("x.mp3"), "");
    }
}
