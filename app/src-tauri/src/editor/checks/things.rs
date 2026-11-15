//! Entity checks: things stuck in solid, ladders that don't touch, lighting basics, and
//! Counter-Strike gameplay sanity by map prefix (spawns, buy zones, objectives,
//! info_map_parameters).

use super::scan::{at, box_hits_brush, point_in_brush, Scan};
use super::{issue, FixData, Issue, Level};
use crate::editor::doc::{Entity, EntityId};
use crate::editor::math::{v3, Bounds, Vec3};

/// The player's standing hull around a spawn's origin.
pub const PLAYER_MIN: Vec3 = v3(-16.0, -16.0, -36.0);
pub const PLAYER_MAX: Vec3 = v3(16.0, 16.0, 36.0);

/// What box an entity takes up in the game (relative to its origin), and how deep an overlap
/// counts as stuck.
fn hull_of(class: &str) -> Option<(Vec3, Vec3, f64)> {
    Some(match class {
        "info_player_start" | "info_player_deathmatch" | "info_vip_start" => (PLAYER_MIN, PLAYER_MAX, 0.5),
        "hostage_entity" | "monster_scientist" => (v3(-16.0, -16.0, 0.0), v3(16.0, 16.0, 72.0), 1.0),
        c if c == "armoury_entity" || c.starts_with("weapon_") || c.starts_with("item_") || c.starts_with("ammo_") => (v3(-16.0, -16.0, 0.0), v3(16.0, 16.0, 16.0), 1.0),
        _ => return None,
    })
}

fn is_spawn(class: &str) -> bool {
    matches!(class, "info_player_start" | "info_player_deathmatch")
}

/// The nearest spot where the box is free (up first: most things are sunk into a floor),
/// in whole units. `avoid` are other boxes it mustn't overlap (other spawns).
pub fn free_spot(scan: &Scan, o: Vec3, min: Vec3, max: Vec3, tol: f64, ignore: Option<EntityId>, avoid: &[Bounds]) -> Option<Vec3> {
    let free = |p: Vec3| {
        let (a, b) = (p + min, p + max);
        scan.box_blocked(a, b, tol.min(0.01), ignore).is_none() && !avoid.iter().any(|x| x.min.x < b.x && x.max.x > a.x && x.min.y < b.y && x.max.y > a.y && x.min.z < b.z && x.max.z > a.z)
    };
    let base = v3(o.x.round(), o.y.round(), o.z.round());
    for dz in 0..=72 {
        let p = base + v3(0.0, 0.0, dz as f64);
        if free(p) {
            return Some(p);
        }
    }
    for r in [8.0, 16.0, 24.0, 32.0, 48.0, 64.0, 96.0, 128.0] {
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            for dz in [0.0, 8.0, 16.0, 36.0] {
                let p = base + v3((a.cos() * r).round(), (a.sin() * r).round(), dz);
                if free(p) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// The distance down to the first walkable floor (a surface sloped less than ~45 degrees) under
/// the point, within `max`.
pub fn floor_below(scan: &Scan, p: Vec3, max: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for i in scan.near(p - v3(0.0, 0.0, max), p, 0.0) {
        let b = &scan.brushes[i];
        if !b.blocks_players() {
            continue;
        }
        // ray p + t*(0,0,-1) against the convex brush
        let (mut t0, mut t1) = (0.0f64, max);
        let mut entry = None;
        let mut miss = false;
        for pl in b.planes.iter().flatten() {
            let denom = -pl.normal.z;
            let num = pl.dist - pl.normal.dot(p);
            if denom.abs() < 1e-9 {
                if num < 0.0 {
                    miss = true;
                    break;
                }
                continue;
            }
            let t = num / denom;
            if denom < 0.0 {
                if t > t0 {
                    t0 = t;
                    entry = Some(pl.normal);
                }
            } else if t < t1 {
                t1 = t;
            }
        }
        if miss || t0 > t1 {
            continue;
        }
        if entry.is_some_and(|n| n.z >= 0.7) && best.is_none_or(|x| t0 < x) {
            best = Some(t0);
        }
    }
    best
}

fn ent_issue(id: EntityId, level: Level, code: &'static str, msg: String, p: Vec3) -> Issue {
    Issue { entity: Some(id.0), at: Some(at(p)), ..issue(level, code, msg) }
}

/// Spawns, items and lights in solid; buried buttons.
pub fn stuck(scan: &Scan, out: &mut Vec<Issue>) {
    let spawns: Vec<Bounds> = scan.doc.entities.values().filter(|e| is_spawn(e.classname())).filter_map(|e| e.origin()).map(|o| Bounds { min: o + PLAYER_MIN, max: o + PLAYER_MAX }).collect();
    for (id, e) in &scan.doc.entities {
        let cls = e.classname();
        let Some(o) = e.origin() else {
            if cls == "func_button" {
                buried_button(scan, *id, out);
            }
            continue;
        };
        if matches!(cls, "light" | "light_spot") {
            if scan.point_in_opaque(o, 0.5).is_some() {
                let to = free_spot(scan, o, v3(-1.0, -1.0, -1.0), v3(1.0, 1.0, 1.0), 0.0, None, &[]);
                out.push(Issue {
                    fix_data: to.map(|p| FixData { to: Some(at(p)), ..Default::default() }),
                    fix: to.and(super::fix_for("light-in-solid")),
                    hint: Some("A light inside a brush lights nothing. Move it out into the room.".into()),
                    ..ent_issue(*id, Level::Warning, "light-in-solid", format!("A {cls} is inside a solid brush, so it gives no light."), o)
                });
            }
            continue;
        }
        let Some((min, max, tol)) = hull_of(cls) else { continue };
        if scan.box_blocked(o + min, o + max, tol, None).is_none() {
            continue;
        }
        // other spawns are off limits for a spawn (but not its own box)
        let avoid: Vec<Bounds> = if is_spawn(cls) { spawns.iter().filter(|b| !(b.min - (o + min)).approx_eq(Vec3::ZERO, 1e-6)).copied().collect() } else { Vec::new() };
        let to = free_spot(scan, o, min, max, tol, None, &avoid);
        let what = if is_spawn(cls) { "Players spawning here are stuck." } else { "It may fall out of the world or be unreachable." };
        out.push(Issue {
            fix_data: to.map(|p| FixData { to: Some(at(p)), ..Default::default() }),
            fix: to.and(super::fix_for("stuck-in-solid")),
            hint: Some(match to {
                Some(p) => format!("Move it to free space moves it to {} {} {}.", p.x, p.y, p.z),
                None => "No free spot nearby: move it by hand.".into(),
            }),
            ..ent_issue(*id, if is_spawn(cls) { Level::Error } else { Level::Warning }, "stuck-in-solid", format!("{cls} overlaps a solid brush. {what}"), o)
        });
    }
}

fn entity_bounds(scan: &Scan, id: EntityId) -> Bounds {
    let mut bb = Bounds::empty();
    for b in scan.brushes.iter().filter(|b| b.entity == Some(id)) {
        bb = bb.union(&b.bounds);
    }
    bb
}

fn buried_button(scan: &Scan, id: EntityId, out: &mut Vec<Issue>) {
    let bb = entity_bounds(scan, id);
    if bb.is_empty() {
        return;
    }
    let c = bb.center();
    let inside = scan.near(c, c, 0.0).into_iter().any(|i| {
        let b = &scan.brushes[i];
        b.entity != Some(id) && b.blocks_players() && b.world && point_in_brush(c, b, 1.0)
    });
    if inside {
        out.push(Issue {
            fix: None,
            hint: Some("Pull it out so at least half of it sticks out of the wall; players press what they can reach.".into()),
            ..ent_issue(id, Level::Warning, "stuck-in-solid", "A func_button is buried in a solid brush; players can't reach it.".into(), c)
        });
    }
}

/// func_ladder that isn't against a wall, or doesn't reach a floor at the bottom or the top.
pub fn ladders(scan: &Scan, out: &mut Vec<Issue>) {
    for (id, e) in &scan.doc.entities {
        if e.classname() != "func_ladder" {
            continue;
        }
        let bb = entity_bounds(scan, *id);
        if bb.is_empty() {
            continue;
        }
        let solid = |i: usize| {
            let b = &scan.brushes[i];
            b.entity != Some(*id) && b.blocks_players()
        };
        // a wall within 2 units of the ladder's sides
        let (wmin, wmax) = (bb.min - v3(2.0, 2.0, 0.0), bb.max + v3(2.0, 2.0, 0.0));
        let wall = scan.near(wmin, wmax, 0.0).into_iter().any(|i| solid(i) && box_hits_brush(wmin, wmax, &scan.brushes[i], 0.01));
        // walkable surfaces near the foot and the top
        let h = bb.max.z - bb.min.z;
        let walkable = |zlo: f64, zhi: f64, pad: f64| -> bool {
            let (min, max) = (v3(bb.min.x - pad, bb.min.y - pad, zlo), v3(bb.max.x + pad, bb.max.y + pad, zhi));
            scan.near(min, max, 0.0).into_iter().filter(|i| solid(*i)).any(|i| {
                let b = &scan.brushes[i];
                b.polys.iter().zip(&b.planes).any(|(poly, pl)| {
                    let Some(pl) = pl else { return false };
                    if pl.normal.z < 0.7 || poly.is_empty() {
                        return false;
                    }
                    let mut pb = Bounds::empty();
                    poly.iter().for_each(|p| pb.add(*p));
                    pb.max.z >= min.z && pb.max.z <= max.z && pb.min.x < max.x && pb.max.x > min.x && pb.min.y < max.y && pb.max.y > min.y
                })
            })
        };
        let floor = walkable(bb.min.z - 48.0, bb.min.z + h * 0.5, 16.0);
        let top = h < 96.0 || walkable(bb.max.z - 72.0, bb.max.z + 24.0, 40.0);
        let mut problems = Vec::new();
        if !wall {
            problems.push("isn't against a wall");
        }
        if !floor {
            problems.push("doesn't reach the floor");
        }
        if !top {
            problems.push("has no ledge to step off at the top");
        }
        if problems.is_empty() {
            continue;
        }
        // a ladder a few units off the wall still climbs: only a note on its own
        let level = if floor && top { Level::Info } else { Level::Warning };
        out.push(Issue {
            hint: Some("Ladders work when they touch the wall (overlap it a unit or two), start at the floor and end level with the ledge above.".into()),
            ..ent_issue(*id, level, "ladder-gap", format!("A func_ladder {}.", problems.join(" and ")), bb.center())
        });
    }
}

/// Sky with no light_environment.
pub fn sky_light(scan: &Scan, out: &mut Vec<Issue>) {
    let sky = scan.brushes.iter().find(|b| b.drawn && b.b.faces.iter().any(|f| super::scan::is_sky(&f.texture)));
    let env = scan.doc.entities.values().any(|e| e.classname() == "light_environment");
    if let (Some(b), false) = (sky, env) {
        out.push(Issue {
            brush: Some(b.id.0),
            at: Some(at(b.bounds.center())),
            hint: Some("light_environment is the sun: it lights everything under the sky. Place one anywhere inside the map.".into()),
            ..issue(Level::Warning, "no-sun", "The map has sky but no light_environment, so nothing outdoors is lit by the sky.".into())
        });
    }
}

/// The map's prefix decides what it needs; mode maps (Loner-Strike modes and the community's
/// usual ones) are checked by their mode kits instead.
pub fn quiet_prefix(map: &str) -> bool {
    let m = map.to_ascii_lowercase();
    ["ls_", "br_", "deathrun_", "dr_", "fun_", "surf_", "jail_", "jb_", "ba_jail"].iter().any(|p| m.starts_with(p))
}

/// Counter-Strike gameplay sanity.
pub fn gameplay(scan: &Scan, map: &str, out: &mut Vec<Issue>) {
    let doc = scan.doc;
    if quiet_prefix(map) || doc.world.get("ls_modes").is_some_and(|v| !v.trim().is_empty()) {
        return;
    }
    let m = map.to_ascii_lowercase();
    let prefix = m.split('_').next().unwrap_or("");
    let objective_map = matches!(prefix, "de" | "cs" | "as" | "es");
    let of = |c: &str| doc.entities.iter().filter(move |(_, e)| e.classname() == c).collect::<Vec<_>>();
    let (ct, t) = (of("info_player_start"), of("info_player_deathmatch"));
    // enough spawns (the "no spawns" lines come from the basic checks)
    for (list, team, cls) in [(&ct, "CT", "info_player_start"), (&t, "T", "info_player_deathmatch")] {
        if objective_map && !list.is_empty() && list.len() < 10 {
            out.push(Issue {
                entity: Some(list[0].0 .0),
                at: list[0].1.origin().map(at),
                place: None,
                fix: Some(if team == "CT" { "place:info_player_start" } else { "place:info_player_deathmatch" }),
                hint: Some("A full server has 16 players a side; most maps have 10 to 20 spawns per team.".into()),
                ..issue(Level::Warning, "few-spawns", format!("Only {} {team} spawn{} ({cls}); about 10 or more is usual.", list.len(), if list.len() == 1 { "" } else { "s" }))
            });
        }
    }
    // spawns on top of each other, and floating ones
    let all: Vec<(EntityId, &Entity, Vec3)> = ct.iter().chain(t.iter()).filter_map(|(id, e)| e.origin().map(|o| (**id, *e, o))).collect();
    let boxes: Vec<Bounds> = all.iter().map(|(_, _, o)| Bounds { min: *o + PLAYER_MIN, max: *o + PLAYER_MAX }).collect();
    let mut overlapping = 0;
    for (k, (id, _, o)) in all.iter().enumerate() {
        let hit = all.iter().enumerate().any(|(j, (_, _, p))| j < k && (o.x - p.x).abs() < 32.0 && (o.y - p.y).abs() < 32.0 && (o.z - p.z).abs() < 72.0);
        if hit {
            overlapping += 1;
            if overlapping <= 30 {
                let avoid: Vec<Bounds> = boxes.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, b)| *b).collect();
                let to = free_spot(scan, *o, PLAYER_MIN, PLAYER_MAX, 0.5, None, &avoid);
                out.push(Issue {
                    fix_data: to.map(|p| FixData { to: Some(at(p)), ..Default::default() }),
                    fix: to.and(super::fix_for("stuck-in-solid")),
                    hint: Some("Spawns need 32 units between them, or players spawn inside each other.".into()),
                    ..ent_issue(*id, Level::Warning, "spawns-overlap", "Two spawns overlap: players spawn stuck in each other.".into(), *o)
                });
            }
            continue;
        }
        if scan.box_blocked(*o + PLAYER_MIN, *o + PLAYER_MAX, 0.5, None).is_some() {
            continue; // the stuck check says so
        }
        let foot = *o + v3(0.0, 0.0, PLAYER_MIN.z + 0.5);
        let on_floor = [v3(0.0, 0.0, 0.0), v3(15.0, 15.0, 0.0), v3(-15.0, 15.0, 0.0), v3(15.0, -15.0, 0.0), v3(-15.0, -15.0, 0.0)].iter().any(|d| floor_below(scan, foot + *d, 64.5).is_some());
        if !on_floor {
            out.push(Issue {
                hint: Some("Put spawns 36 units above a floor (the editor's entity tool does when you click the floor).".into()),
                ..ent_issue(*id, Level::Warning, "spawn-no-floor", "A spawn has no walkable floor under it (within 64 units).".into(), *o)
            });
        }
    }
    // buy zones
    let zones: Vec<(EntityId, i64)> = of("func_buyzone").iter().map(|(id, e)| (**id, e.get("team").and_then(|v| v.trim().parse().ok()).unwrap_or(0))).collect();
    if zones.is_empty() {
        if objective_map {
            out.push(issue(Level::Info, "no-buyzone", "No func_buyzone: the game makes a buy area around each team's spawns. Add zones to control where buying works.".into()));
        }
    } else {
        for (list, team, num) in [(&ct, "CT", 2), (&t, "T", 1)] {
            let outside: Vec<&(&EntityId, &Entity)> = list
                .iter()
                .filter(|(_, e)| {
                    let Some(o) = e.origin() else { return false };
                    !zones.iter().filter(|(_, tm)| *tm == 0 || *tm == num).any(|(zid, _)| scan.brushes.iter().filter(|b| b.entity == Some(*zid)).any(|b| point_in_brush(o, b, 0.0)))
                })
                .collect();
            if let Some((id, e)) = outside.first() {
                out.push(Issue {
                    hint: Some("Stretch a func_buyzone over the spawns (team 2 = CT, 1 = T, 0 = both).".into()),
                    ..ent_issue(**id, Level::Warning, "spawn-no-buyzone", format!("{} {team} spawn{} outside every {team} buy zone.", outside.len(), if outside.len() == 1 { " is" } else { "s are" }), e.origin().unwrap_or_default())
                });
            }
        }
    }
    // objectives
    let has = |c: &[&str]| doc.entities.values().any(|e| c.contains(&e.classname()));
    match prefix {
        "de" if !has(&["func_bomb_target", "info_bomb_target"]) => {
            out.push(Issue { hint: Some("Draw a brush where the bomb can be planted and tie it to func_bomb_target (two sites is usual).".into()), ..issue(Level::Warning, "no-bombsite", "A de_ map with no bomb site (func_bomb_target).".into()) })
        }
        "cs" => {
            if !has(&["hostage_entity", "monster_scientist"]) {
                out.push(Issue { fix: Some("place:hostage_entity"), ..issue(Level::Warning, "no-hostages", "A cs_ map with no hostages (hostage_entity).".into()) });
            }
            if !has(&["func_hostage_rescue", "info_hostage_rescue"]) {
                out.push(issue(Level::Info, "no-rescue", "No hostage rescue zone: the CT spawns count as one. Add func_hostage_rescue to choose where.".into()));
            }
        }
        "as" => {
            if !has(&["info_vip_start"]) {
                out.push(Issue { fix: Some("place:info_vip_start"), ..issue(Level::Warning, "no-vip", "An as_ map with no VIP start (info_vip_start).".into()) });
            }
            if !has(&["func_vip_safetyzone"]) {
                out.push(issue(Level::Warning, "no-vip-zone", "An as_ map with no VIP escape zone (func_vip_safetyzone).".into()));
            }
        }
        "es" if !has(&["func_escapezone"]) => out.push(issue(Level::Warning, "no-escape", "An es_ map with no escape zone (func_escapezone).".into())),
        _ => {}
    }
    // info_map_parameters
    let params = of("info_map_parameters");
    if params.len() > 1 {
        out.push(Issue { entity: Some(params[1].0 .0), ..issue(Level::Warning, "map-parameters", format!("{} info_map_parameters; the game reads only one.", params.len())) });
    }
    if let Some((id, e)) = params.first() {
        if let Some(b) = e.get("buying") {
            if !matches!(b.trim(), "0" | "1" | "2" | "3") {
                out.push(Issue { entity: Some(id.0), ..issue(Level::Warning, "map-parameters", format!("info_map_parameters: buying \"{b}\" isn't 0-3 (0 everyone, 1 CTs only, 2 Ts only, 3 nobody).")) });
            }
        }
        if let Some(r) = e.get("bombradius") {
            if r.trim().parse::<f64>().map_or(true, |v| !(1.0..=2048.0).contains(&v)) {
                out.push(Issue { entity: Some(id.0), ..issue(Level::Warning, "map-parameters", format!("info_map_parameters: bomb radius \"{r}\" isn't between 1 and 2048 (500 is the default).")) });
            }
        }
    }
}
