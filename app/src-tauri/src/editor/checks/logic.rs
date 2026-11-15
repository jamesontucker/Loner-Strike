//! Stock entity checks (EngineCoverage item 9): what stops the game's own random triggers,
//! game_ rules, trains, turrets and wheels working, from how the game DLL reads their keys.
//! A missing key is 0 to the game, and for several of these 0 means "never" (a turret that
//! never turns, a counter with nothing to count to). Names that point at nothing are the I/O
//! graph's dangling-target check; this one flags the keys left empty and the wrong kind of
//! entity on the other end.

use super::{issue, Issue, Level};
use crate::editor::doc::{EntityId, MapDocument};
use std::collections::{HashMap, HashSet};

/// Brush entities that turn or ride a track round their origin brush.
const NEEDS_ORIGIN: &[&str] = &[
    "func_tracktrain", "momentary_rot_button", "func_platrot", "func_trackchange", "func_trackautochange", "func_tank", "func_tankmortar", "func_tanklaser",
];

pub fn stock(doc: &MapDocument, out: &mut Vec<Issue>) {
    // targetname -> classnames
    let mut names: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in doc.entities.values() {
        if let Some(n) = e.get("targetname").map(str::trim).filter(|n| !n.is_empty()) {
            names.entry(n).or_default().push(e.classname());
        }
    }
    let with_origin: HashSet<EntityId> = doc.brushes.values().filter(|b| b.faces.iter().any(|f| f.texture.eq_ignore_ascii_case("origin"))).filter_map(|b| b.entity).collect();
    let with_brushes: HashSet<EntityId> = doc.brushes.values().filter_map(|b| b.entity).collect();
    // what the momentary wheels drive, and the turrets' controls
    let driven: HashSet<&str> = doc.entities.values().filter(|e| e.classname() == "momentary_rot_button").filter_map(|e| e.get("target")).map(str::trim).collect();
    for (id, e) in &doc.entities {
        let cls = e.classname();
        let val = |k: &str| e.get(k).map(str::trim).filter(|v| !v.is_empty());
        let num = |k: &str| val(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let flags = num("spawnflags") as i64;
        let mut say = |level: Level, code: &'static str, msg: String| out.push(Issue { entity: Some(id.0), ..issue(level, code, msg) });
        // a name that must be a certain kind of entity
        let kind_of = |k: &str| val(k).and_then(|n| names.get(n));
        let wrong_kind = |k: &str, ok: &dyn Fn(&str) -> bool| kind_of(k).is_some_and(|cs| !cs.iter().any(|c| ok(c)));
        if NEEDS_ORIGIN.contains(&cls) && with_brushes.contains(id) && !with_origin.contains(id) {
            say(Level::Warning, "needs-origin", format!("{cls} has no origin brush: it turns round (or rides the track by) the middle of the map instead. Add a small brush with the ORIGIN texture."));
        }
        match cls {
            "trigger_random" | "trigger_random_time" | "trigger_random_unique" => {
                let n = num("target_count").clamp(0.0, 16.0) as usize;
                if n == 0 {
                    say(Level::Warning, "random-no-count", format!("{cls} has How many targets at 0, so it never picks anything."));
                } else {
                    let empty: Vec<String> = (0..n).filter(|i| val(&format!("target{i}")).is_none()).map(|i| (i + 1).to_string()).collect();
                    if !empty.is_empty() {
                        say(Level::Warning, "random-empty-target", format!("{cls} picks among {n} targets, but Target {} is empty: those picks set off nothing.", empty.join(", ")));
                    }
                }
                let timed = cls == "trigger_random_time" || (cls == "trigger_random" && flags & 8 != 0);
                if timed && (num("min_delay") <= 0.0 || num("max_delay") <= 0.0) {
                    say(Level::Info, "random-no-wait", format!("{cls} is timed but a wait is 0: it picks once and stops."));
                }
                if timed && flags & 1 == 0 && val("targetname").is_none() {
                    say(Level::Warning, "random-never-starts", format!("{cls} is timed, doesn't start on, and has no name: nothing can start it."));
                }
            }
            "trigger_setorigin" => {
                if val("target").is_none() {
                    say(Level::Warning, "setorigin-no-target", "trigger_setorigin moves nothing: name the entities in Moves.".into());
                }
                if val("copypointer").is_none() {
                    say(Level::Warning, "setorigin-no-copy", "trigger_setorigin has nowhere to move them: name an entity in Moves them to.".into());
                }
            }
            "env_global" if val("globalstate").is_none() => {
                say(Level::Error, "global-no-name", "env_global has no Global state name: the game removes it.".into());
            }
            "game_counter" => {
                if val("target").is_none() {
                    say(Level::Info, "counter-no-target", "game_counter sets off nothing when it reaches its limit.".into());
                }
                if num("health") == 0.0 && num("frags") == 0.0 {
                    say(Level::Info, "counter-zero-limit", "game_counter's Limit is 0 (the same as its start): it only fires when counted back down to 0.".into());
                }
            }
            "game_counter_set" | "game_team_set" | "game_player_team" if val("target").is_none() => {
                let what = if cls == "game_counter_set" { "game_counter" } else { "game_team_master" };
                say(Level::Warning, "rule-no-target", format!("{cls} needs a {what} as its target; it has none."));
            }
            "game_team_master" if val("triggerstate").is_none() && val("target").is_some() => {
                say(Level::Info, "teammaster-sends-off", "game_team_master's Sends is unset, so it sends Off: doors and buttons ignore that. Pick On or Toggle.".into());
            }
            "game_end" => {
                say(Level::Info, "ends-map", "game_end ends the map for everyone when it's set off.".into());
            }
            "game_zone_player" if ["intarget", "outtarget", "incount", "outcount"].iter().all(|k| val(k).is_none()) => {
                say(Level::Warning, "zone-no-targets", "game_zone_player sets off nothing: fill In, Out or a count.".into());
            }
            "momentary_rot_button" => {
                if num("distance") == 0.0 {
                    say(Level::Error, "wheel-no-distance", "momentary_rot_button has Turns at 0: it can't turn (and breaks what it drives).".into());
                }
                if val("target").is_none() {
                    say(Level::Info, "wheel-no-target", "momentary_rot_button drives nothing: point its target at a momentary_door.".into());
                }
            }
            "momentary_door" if !val("targetname").is_some_and(|n| driven.contains(n)) => {
                say(Level::Info, "momentary-undriven", "No momentary_rot_button drives this momentary_door, so it never moves.".into());
            }
            "func_tracktrain" => {
                if val("target").is_none() {
                    say(Level::Error, "train-no-track", "func_tracktrain has no First stop: it won't move. Name its first path_track.".into());
                } else if wrong_kind("target", &|c| c == "path_track") {
                    say(Level::Error, "train-not-track", "func_tracktrain's First stop isn't a path_track; the game stops it.".into());
                }
            }
            "path_track" => {
                if wrong_kind("target", &|c| c == "path_track") {
                    say(Level::Warning, "track-next-not-track", "This path_track's Next stop isn't a path_track: the track ends here.".into());
                }
                if val("targetname").is_none() {
                    say(Level::Warning, "track-unnamed", "path_track has no name, so no train or stop can lead to it.".into());
                }
            }
            "func_traincontrols" | "func_vehiclecontrols" | "func_tankcontrols" => {
                let (what, ok): (&str, fn(&str) -> bool) = match cls {
                    "func_traincontrols" => ("func_tracktrain", |c| c == "func_tracktrain"),
                    "func_vehiclecontrols" => ("func_vehicle", |c| c == "func_vehicle"),
                    _ => ("func_tank", |c| c.starts_with("func_tank") && c != "func_tankcontrols"),
                };
                if val("target").is_none() {
                    say(Level::Warning, "controls-no-target", format!("{cls} controls nothing: name its {what}."));
                } else if wrong_kind("target", &ok) {
                    say(Level::Warning, "controls-wrong-target", format!("{cls}'s target isn't a {what}."));
                }
                if cls == "func_tankcontrols" {
                    let tank_off = val("target").is_some_and(|n| {
                        doc.entities.values().any(|t| t.get("targetname").map(str::trim) == Some(n) && t.classname().starts_with("func_tank") && t.get("spawnflags").and_then(|f| f.trim().parse::<i64>().ok()).unwrap_or(0) & 32 == 0)
                    });
                    if tank_off {
                        say(Level::Warning, "tank-not-controllable", "The turret this func_tankcontrols names doesn't have Players can control it, so nobody can take it.".into());
                    }
                }
            }
            "func_tank" | "func_tankmortar" | "func_tanklaser" => {
                if num("yawrate") == 0.0 && num("pitchrate") == 0.0 {
                    say(Level::Warning, "tank-no-turn", format!("{cls} has both turn speeds at 0, so it never aims (30 is usual)."));
                }
                if flags & 1 != 0 && num("persistence") <= 0.0 {
                    say(Level::Warning, "tank-no-persist", format!("{cls} starts on but Keeps firing is 0, so it never fires on its own (1 is usual)."));
                }
                if cls == "func_tank" && num("bullet") == 0.0 {
                    say(Level::Warning, "tank-no-bullets", "func_tank's Bullets is None: it fires nothing.".into());
                }
                if cls == "func_tankmortar" && num("iMagnitude") == 0.0 {
                    say(Level::Warning, "tank-no-blast", "func_tankmortar's Explosion size is 0: its shots do nothing.".into());
                }
                if cls == "func_tanklaser" && (val("laserentity").is_none() || wrong_kind("laserentity", &|c| c == "env_laser")) {
                    say(Level::Error, "tank-no-laser", "func_tanklaser needs an env_laser named in Laser, or the game removes it.".into());
                }
            }
            "func_trackchange" | "func_trackautochange" => {
                let empty: Vec<&str> = [("train", "Train"), ("toptrack", "Top track"), ("bottomtrack", "Bottom track")].into_iter().filter(|(k, _)| val(k).is_none()).map(|(_, l)| l).collect();
                if !empty.is_empty() {
                    say(Level::Warning, "trackchange-missing", format!("{cls} needs {} filled in.", empty.join(", ")));
                }
            }
            "func_guntarget" => {
                if val("target").is_none() {
                    say(Level::Warning, "guntarget-no-path", "func_guntarget has no First stop, so it never moves or takes damage. Name a path_corner.".into());
                }
                if num("health") <= 0.0 {
                    say(Level::Info, "guntarget-no-health", "func_guntarget's Health is 0: shooting it never stops it.".into());
                }
            }
            _ => {}
        }
    }
}

/// A small room with every stock entity above working together (wired so a plugin can fire
/// them by name and read the results), for the real compile and sim check of item 9.
#[cfg(test)]
pub fn stock_test_map() -> MapDocument {
    use crate::editor::doc::Entity;
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;
    let mut d = crate::tools::editor::starter_map();
    let point = |d: &mut MapDocument, at: (f64, f64, f64), kv: &[(&str, &str)]| {
        let mut e = Entity::new(kv[0].1);
        e.set("origin", &format!("{} {} {}", at.0, at.1, at.2));
        for (k, v) in &kv[1..] {
            e.set(k, v);
        }
        d.insert_entity(e);
    };
    // a brush entity from boxes (min, max, texture)
    let solid = |d: &mut MapDocument, boxes: &[((f64, f64, f64), (f64, f64, f64), &str)], kv: &[(&str, &str)]| {
        let mut e = Entity::new(kv[0].1);
        for (k, v) in &kv[1..] {
            e.set(k, v);
        }
        let id = d.insert_entity(e);
        for (a, b, t) in boxes {
            let mut br = make_box(v3(a.0, a.1, a.2), v3(b.0, b.1, b.2), t);
            br.entity = Some(id);
            d.insert_brush(br);
        }
    };
    const W: &str = "OUT_WALL5";
    // random triggers into counters
    point(&mut d, (-200.0, 0.0, 32.0), &[("classname", "trigger_random"), ("targetname", "rnd"), ("target_count", "2"), ("target0", "cntA"), ("target1", "cntB")]);
    point(&mut d, (-200.0, 32.0, 32.0), &[("classname", "trigger_random_unique"), ("targetname", "rndu"), ("target_count", "2"), ("target0", "cntC"), ("target1", "cntD")]);
    point(&mut d, (-200.0, 64.0, 32.0), &[("classname", "trigger_random_time"), ("targetname", "rndt"), ("spawnflags", "1"), ("min_delay", "1"), ("max_delay", "1"), ("target_count", "1"), ("target0", "cntH")]);
    let mut y = -200.0;
    for c in ["cntA", "cntB", "cntC", "cntD", "cntE", "cntG", "cntH", "cntBT", "cntZin", "cntZcount", "cntTM"] {
        point(&mut d, (-150.0, y, 32.0), &[("classname", "game_counter"), ("targetname", c), ("health", "1000")]);
        y += 24.0;
    }
    point(&mut d, (-120.0, 0.0, 32.0), &[("classname", "game_counter"), ("targetname", "cntF"), ("health", "2"), ("target", "csetG")]);
    point(&mut d, (-120.0, 24.0, 32.0), &[("classname", "game_counter_set"), ("targetname", "cset"), ("frags", "7"), ("target", "cntE")]);
    point(&mut d, (-120.0, 48.0, 32.0), &[("classname", "game_counter_set"), ("targetname", "csetG"), ("frags", "5"), ("target", "cntG")]);
    // moves an info_target onto another
    point(&mut d, (0.0, 200.0, 64.0), &[("classname", "trigger_setorigin"), ("targetname", "so"), ("target", "mover"), ("copypointer", "dest"), ("offset", "0 0 16")]);
    point(&mut d, (0.0, 240.0, 64.0), &[("classname", "info_target"), ("targetname", "mover")]);
    point(&mut d, (100.0, 240.0, 64.0), &[("classname", "info_target"), ("targetname", "dest")]);
    // a global switch and the multisource that reads it
    point(&mut d, (40.0, 0.0, 32.0), &[("classname", "env_global"), ("targetname", "gl"), ("globalstate", "stk_g"), ("triggermode", "1"), ("initialstate", "0"), ("spawnflags", "1")]);
    point(&mut d, (40.0, 24.0, 32.0), &[("classname", "multisource"), ("targetname", "ms"), ("globalstate", "stk_g")]);
    // game rules
    point(&mut d, (80.0, 0.0, 32.0), &[("classname", "game_score"), ("targetname", "score"), ("points", "5")]);
    point(&mut d, (80.0, 24.0, 32.0), &[("classname", "game_player_hurt"), ("targetname", "hurt"), ("dmg", "10")]);
    point(&mut d, (80.0, 48.0, 32.0), &[("classname", "game_team_master"), ("targetname", "tm"), ("teamindex", "-1"), ("triggerstate", "1"), ("target", "cntTM"), ("spawnflags", "2")]);
    point(&mut d, (80.0, 72.0, 32.0), &[("classname", "game_team_set"), ("targetname", "ts"), ("target", "tm")]);
    point(&mut d, (80.0, 96.0, 32.0), &[("classname", "game_player_team"), ("targetname", "pteam"), ("target", "tm")]);
    point(&mut d, (80.0, 120.0, 32.0), &[("classname", "game_end"), ("targetname", "gend")]);
    // a track and the train on it, with its controls
    for (n, x, yy, next) in [("t1", -300.0, -300.0, "t2"), ("t2", -300.0, 300.0, "t3"), ("t3", -100.0, 300.0, "t1")] {
        point(&mut d, (x, yy, 8.0), &[("classname", "path_track"), ("targetname", n), ("target", next)]);
    }
    solid(&mut d, &[((-332.0, -332.0, 16.0), (-268.0, -268.0, 40.0), W), ((-308.0, -308.0, 16.0), (-292.0, -292.0, 32.0), "ORIGIN")], &[("classname", "func_tracktrain"), ("targetname", "train"), ("target", "t1"), ("speed", "100"), ("height", "16"), ("wheels", "32")]);
    solid(&mut d, &[((-332.0, -332.0, 40.0), (-268.0, -268.0, 72.0), "AAATRIGGER")], &[("classname", "func_traincontrols"), ("target", "train")]);
    // a wheel that opens a gate
    solid(&mut d, &[((0.0, -480.0, 32.0), (16.0, -464.0, 64.0), W), ((0.0, -480.0, 40.0), (16.0, -464.0, 56.0), "ORIGIN")], &[("classname", "momentary_rot_button"), ("targetname", "wheel"), ("target", "mdoor"), ("distance", "90"), ("speed", "50")]);
    solid(&mut d, &[((100.0, -500.0, 0.0), (164.0, -484.0, 96.0), W)], &[("classname", "momentary_door"), ("targetname", "mdoor"), ("angle", "-1"), ("speed", "100")]);
    solid(&mut d, &[((200.0, -500.0, 64.0), (216.0, -484.0, 80.0), W)], &[("classname", "button_target"), ("targetname", "btgt"), ("target", "cntBT")]);
    // a gun target on two corners
    point(&mut d, (300.0, 300.0, 72.0), &[("classname", "path_corner"), ("targetname", "pc1"), ("target", "pc2")]);
    point(&mut d, (300.0, 400.0, 72.0), &[("classname", "path_corner"), ("targetname", "pc2"), ("target", "pc1")]);
    solid(&mut d, &[((292.0, 292.0, 64.0), (308.0, 308.0, 80.0), W)], &[("classname", "func_guntarget"), ("targetname", "gt"), ("target", "pc1"), ("health", "50"), ("speed", "50")]);
    // turrets players control, and their controls
    let tank = |d: &mut MapDocument, x: f64, kv: &[(&str, &str)]| {
        solid(d, &[((x, 0.0, 32.0), (x + 32.0, 16.0, 48.0), W), ((x, 0.0, 32.0), (x + 16.0, 16.0, 48.0), "ORIGIN")], kv);
    };
    tank(&mut d, 200.0, &[("classname", "func_tank"), ("targetname", "tank"), ("yawrate", "30"), ("yawrange", "45"), ("pitchrate", "30"), ("pitchrange", "20"), ("bullet", "1"), ("firerate", "2"), ("persistence", "1"), ("spawnflags", "32")]);
    tank(&mut d, 260.0, &[("classname", "func_tankmortar"), ("targetname", "mortar"), ("yawrate", "30"), ("yawrange", "45"), ("iMagnitude", "50"), ("persistence", "1"), ("spawnflags", "32")]);
    tank(&mut d, 320.0, &[("classname", "func_tanklaser"), ("targetname", "ltank"), ("yawrate", "30"), ("yawrange", "45"), ("laserentity", "laser1"), ("persistence", "1"), ("spawnflags", "32")]);
    point(&mut d, (336.0, 8.0, 64.0), &[("classname", "env_laser"), ("targetname", "laser1"), ("texture", "sprites/laserbeam.spr"), ("LaserTarget", "info1"), ("BoltWidth", "8"), ("renderamt", "200"), ("rendercolor", "255 0 0")]);
    point(&mut d, (336.0, 100.0, 64.0), &[("classname", "info_target"), ("targetname", "info1")]);
    solid(&mut d, &[((200.0, 24.0, 0.0), (232.0, 56.0, 72.0), "AAATRIGGER")], &[("classname", "func_tankcontrols"), ("target", "tank")]);
    // chargers on the north wall, a turning lift, the CT zone
    solid(&mut d, &[((-100.0, 496.0, 32.0), (-68.0, 512.0, 64.0), W)], &[("classname", "func_healthcharger")]);
    solid(&mut d, &[((-40.0, 496.0, 32.0), (-8.0, 512.0, 64.0), W)], &[("classname", "func_recharge")]);
    solid(&mut d, &[((400.0, -400.0, 0.0), (464.0, -336.0, 16.0), W), ((424.0, -376.0, 0.0), (440.0, -360.0, 16.0), "ORIGIN")], &[("classname", "func_platrot"), ("targetname", "prot"), ("height", "64"), ("rotation", "90")]);
    solid(&mut d, &[((-480.0, -160.0, 0.0), (-288.0, 160.0, 128.0), "AAATRIGGER")], &[("classname", "game_zone_player"), ("targetname", "zone"), ("intarget", "cntZin"), ("incount", "cntZcount")]);
    // pickups in a row
    let mut x = -300.0;
    for c in ["item_healthkit", "item_battery", "item_kevlar", "item_assaultsuit", "item_thighpack", "item_longjump", "ammo_9mm", "ammo_buckshot", "ammo_556nato", "ammo_556natobox", "ammo_762nato", "ammo_45acp", "ammo_50ae", "ammo_338magnum", "ammo_57mm", "ammo_357sig"] {
        point(&mut d, (x, -440.0, 16.0), &[("classname", c)]);
        x += 40.0;
    }
    d
}

#[cfg(test)]
mod tests {
    use crate::editor::checks::run;
    use crate::editor::doc::{Entity, EntityId, MapDocument};

    fn ent(d: &mut MapDocument, kv: &[(&str, &str)]) -> EntityId {
        let mut e = Entity::new(kv[0].1);
        e.set("origin", "0 0 0");
        for (k, v) in &kv[1..] {
            e.set(k, v);
        }
        d.insert_entity(e)
    }

    /// The sim map uses every entity the way the checks want: no errors or warnings from them.
    #[test]
    fn the_stock_test_map_is_clean() {
        let d = super::stock_test_map();
        let mut v = Vec::new();
        super::stock(&d, &mut v);
        let bad: Vec<_> = v.iter().filter(|i| i.level != crate::editor::checks::Level::Info).map(|i| (i.code, i.message.clone())).collect();
        assert!(bad.is_empty(), "{bad:?}");
        let all = run(&d, None);
        assert!(all.iter().all(|i| i.level != crate::editor::checks::Level::Error), "{:?}", all.iter().filter(|i| i.level == crate::editor::checks::Level::Error).collect::<Vec<_>>());
    }

    #[test]
    fn stock_entities_say_what_stops_them() {
        let mut d = MapDocument::new();
        let rnd = ent(&mut d, &[("classname", "trigger_random"), ("targetname", "r"), ("target_count", "3"), ("target0", "a"), ("target2", "a")]);
        let rnd0 = ent(&mut d, &[("classname", "trigger_random_time")]);
        let so = ent(&mut d, &[("classname", "trigger_setorigin"), ("target", "a")]);
        let glob = ent(&mut d, &[("classname", "env_global")]);
        let set = ent(&mut d, &[("classname", "game_team_set")]);
        let end = ent(&mut d, &[("classname", "game_end")]);
        let tank = ent(&mut d, &[("classname", "func_tank"), ("spawnflags", "1")]);
        let laser = ent(&mut d, &[("classname", "func_tanklaser"), ("yawrate", "30"), ("laserentity", "a")]);
        let ctl = ent(&mut d, &[("classname", "func_tankcontrols"), ("target", "t2")]);
        ent(&mut d, &[("classname", "func_tank"), ("targetname", "t2"), ("yawrate", "30"), ("bullet", "1")]);
        let train = ent(&mut d, &[("classname", "func_tracktrain"), ("target", "a")]);
        let wheel = ent(&mut d, &[("classname", "momentary_rot_button")]);
        let ok = ent(&mut d, &[("classname", "trigger_random"), ("targetname", "ok"), ("target_count", "1"), ("target0", "a")]);
        ent(&mut d, &[("classname", "info_target"), ("targetname", "a")]);
        let v = run(&d, None);
        // (brush entities here have no brushes: leave that check out)
        let of = |id: EntityId| v.iter().filter(|i| i.entity == Some(id.0) && i.code != "empty-brush-entity").map(|i| i.code).collect::<Vec<_>>();
        assert_eq!(of(rnd), vec!["random-empty-target"]);
        assert!(v.iter().any(|i| i.entity == Some(rnd.0) && i.message.contains("Target 2 is empty")));
        assert!(of(rnd0).contains(&"random-no-count") && of(rnd0).contains(&"random-never-starts"), "{:?}", of(rnd0));
        assert_eq!(of(so), vec!["setorigin-no-copy"]);
        assert_eq!(of(glob), vec!["global-no-name"]);
        assert_eq!(of(set), vec!["rule-no-target"]);
        assert_eq!(of(end), vec!["ends-map"]);
        let t = of(tank);
        assert!(t.contains(&"tank-no-turn") && t.contains(&"tank-no-persist") && t.contains(&"tank-no-bullets"), "{t:?}");
        assert_eq!(of(laser), vec!["tank-no-laser"], "an info_target isn't a laser");
        assert_eq!(of(ctl), vec!["tank-not-controllable"]);
        assert_eq!(of(train), vec!["train-not-track"]);
        let w = of(wheel);
        assert!(w.contains(&"wheel-no-distance") && w.contains(&"wheel-no-target"), "{w:?}");
        assert!(of(ok).is_empty(), "{:?}", of(ok));
        // point entities named trigger_* aren't brush entities
        assert!(!v.iter().any(|i| i.code == "empty-brush-entity" && (i.entity == Some(rnd.0) || i.entity == Some(so.0))));
    }
}
