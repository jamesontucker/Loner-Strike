//! Tests for the deeper checks: a small purpose-built map for each, the check's fix through
//! `history::cmd_fix` (undoable), and a run over our own maps in maps-src.

use super::*;
use crate::editor::doc::{Brush, Entity, Material};
use crate::editor::history::{cmd_fix, FixIt, History};
use crate::editor::math::{v3, Vec3};
use crate::editor::ops::make_box;

fn opts(map: &str) -> Options {
    Options { map: map.into(), ..Options::default() }
}

fn codes(v: &[Issue]) -> Vec<&'static str> {
    v.iter().map(|i| i.code).collect()
}

fn ent(d: &mut MapDocument, class: &str, o: Vec3) -> EntityId {
    let mut e = Entity::new(class);
    e.set("origin", &format!("{} {} {}", o.x, o.y, o.z));
    d.insert_entity(e)
}

fn tie(d: &mut MapDocument, b: Brush, class: &str) -> (EntityId, BrushId) {
    let e = d.insert_entity(Entity::new(class));
    let id = d.insert_brush(Brush { entity: Some(e), ..b });
    (e, id)
}

/// A sealed 512 x 512 x 256 room (inside), 16-unit walls, a light and two spawns on the floor.
fn room() -> MapDocument {
    let mut d = MapDocument::new();
    d.world.set_wad_key("halflife.wad");
    let (lo, hi, t) = (v3(-256.0, -256.0, 0.0), v3(256.0, 256.0, 256.0), 16.0);
    let tex = "CRATE";
    d.insert_brush(make_box(v3(lo.x - t, lo.y - t, lo.z - t), v3(hi.x + t, hi.y + t, lo.z), tex)); // floor
    d.insert_brush(make_box(v3(lo.x - t, lo.y - t, hi.z), v3(hi.x + t, hi.y + t, hi.z + t), tex)); // ceiling
    d.insert_brush(make_box(v3(lo.x - t, lo.y - t, lo.z), v3(lo.x, hi.y + t, hi.z), tex)); // west
    d.insert_brush(make_box(v3(hi.x, lo.y - t, lo.z), v3(hi.x + t, hi.y + t, hi.z), tex)); // east
    d.insert_brush(make_box(v3(lo.x, lo.y - t, lo.z), v3(hi.x, lo.y, hi.z), tex)); // south
    d.insert_brush(make_box(v3(lo.x, hi.y, lo.z), v3(hi.x, hi.y + t, hi.z), tex)); // north
    ent(&mut d, "light", v3(0.0, 0.0, 200.0));
    ent(&mut d, "info_player_start", v3(-128.0, 0.0, 36.0));
    ent(&mut d, "info_player_deathmatch", v3(128.0, 0.0, 36.0));
    d.add_material(Material { name: "CRATE".into(), width: 64, height: 64, wad: None });
    d
}

/// Apply every fix offered for issues with this code, as one undo step.
fn fix_all(d: &mut MapDocument, h: &mut History, issues: &[Issue], code: &str) -> usize {
    let list: Vec<FixIt> = issues
        .iter()
        .filter(|i| i.code == code)
        .filter_map(|i| i.fix.filter(|f| !f.starts_with("place:")).map(|f| FixIt { fix: f.into(), brush: i.brush.map(BrushId), entity: i.entity.map(EntityId), texture: i.texture.clone(), face: i.face, data: i.fix_data.clone() }))
        .collect();
    assert!(!list.is_empty(), "no fix for {code}");
    cmd_fix(d, h, &list, None).unwrap()
}

fn problems(d: &MapDocument, map: &str) -> Vec<Issue> {
    deeper(d, &opts(map)).into_iter().filter(|i| i.level != Level::Info).collect()
}

#[test]
fn a_clean_room_has_no_warnings() {
    let d = room();
    let p = problems(&d, "ls_test");
    assert!(p.is_empty(), "{p:?}");
    // its outside faces face the void: counted as faces nobody sees, which the compiler drops itself
    let all = deeper(&d, &opts("ls_test"));
    let h = all.iter().find(|i| i.code == "hidden-faces").expect("hidden faces");
    assert!(h.message.contains("compiler drops these itself"), "{}", h.message);
}

#[test]
fn z_fighting_between_an_entity_and_the_world() {
    let mut d = room();
    d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE"));
    let (_, wall) = tie(&mut d, make_box(v3(16.0, 16.0, 32.0), v3(48.0, 48.0, 64.0), "CRATE"), "func_wall");
    // two world brushes sharing a plane are fine (the compiler merges them)
    d.insert_brush(make_box(v3(-128.0, -128.0, 0.0), v3(-64.0, -64.0, 64.0), "CRATE"));
    d.insert_brush(make_box(v3(-128.0, -128.0, 32.0), v3(-64.0, -64.0, 64.0), "CRATE"));
    let issues = deeper(&d, &opts("ls_test"));
    let zf: Vec<&Issue> = issues.iter().filter(|i| i.code == "z-fighting").collect();
    assert_eq!(zf.len(), 1, "{zf:?}");
    assert_eq!(zf[0].brush, Some(wall.0));
    assert!(zf[0].at.is_some() && zf[0].hint.is_some());
    assert!(zf[0].more_fixes.iter().any(|m| m.fix == "null-face") && zf[0].more_fixes.iter().any(|m| m.fix == "to-world"));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "z-fighting");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"z-fighting"));
    h.undo(&mut d);
    assert!(codes(&deeper(&d, &opts("ls_test"))).contains(&"z-fighting"));
    // skipped when one face is NULL or the brush is a trigger
    let mut b = d.brushes[&wall].clone();
    for f in &mut b.faces {
        f.texture = "AAATRIGGER".into();
    }
    d.brushes.insert(wall, b);
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"z-fighting"));
}

#[test]
fn stuck_spawns_items_and_lights_move_to_free_space() {
    let mut d = room();
    let sp = ent(&mut d, "info_player_deathmatch", v3(0.0, 128.0, 30.0)); // 6 units into the floor
    let gun = ent(&mut d, "armoury_entity", v3(0.0, -128.0, -8.0));
    let lamp = ent(&mut d, "light", v3(-264.0, 0.0, 128.0)); // inside the west wall
    let issues = deeper(&d, &opts("ls_test"));
    let stuck: Vec<&Issue> = issues.iter().filter(|i| i.code == "stuck-in-solid").collect();
    assert_eq!(stuck.len(), 2, "{stuck:?}");
    let s = stuck.iter().find(|i| i.entity == Some(sp.0)).unwrap();
    assert_eq!(s.level, Level::Error);
    assert_eq!(s.fix_data.as_ref().unwrap().to, Some([0.0, 128.0, 36.0]));
    assert!(stuck.iter().any(|i| i.entity == Some(gun.0)));
    let l = issues.iter().find(|i| i.code == "light-in-solid").unwrap();
    assert_eq!(l.entity, Some(lamp.0));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "stuck-in-solid");
    fix_all(&mut d, &mut h, &issues, "light-in-solid");
    assert_eq!(d.entities[&sp].get("origin"), Some("0 128 36"));
    let left = codes(&deeper(&d, &opts("ls_test")));
    assert!(!left.contains(&"stuck-in-solid") && !left.contains(&"light-in-solid"), "{left:?}");
    // a spawn standing on the floor exactly isn't stuck
    assert!(!codes(&deeper(&room(), &opts("ls_test"))).contains(&"stuck-in-solid"));
}

#[test]
fn micro_gaps_and_off_grid_brushes_snap() {
    let mut d = room();
    d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE"));
    d.insert_brush(make_box(v3(64.5, 0.0, 0.0), v3(128.0, 64.0, 64.0), "CRATE"));
    let issues = deeper(&d, &opts("ls_test"));
    let gap = issues.iter().find(|i| i.code == "micro-gap").expect("gap");
    assert!((gap.fix_data.as_ref().unwrap().dist.unwrap() - 0.5).abs() < 1e-6);
    assert!(issues.iter().any(|i| i.code == "off-grid"));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "micro-gap");
    let after = deeper(&d, &opts("ls_test"));
    assert!(!codes(&after).contains(&"micro-gap") && !codes(&after).contains(&"off-grid"), "{:?}", codes(&after));
    // off the grid without a gap: snap
    let mut d = room();
    d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.25, 64.0, 64.0), "CRATE"));
    let issues = deeper(&d, &opts("ls_test"));
    fix_all(&mut d, &mut h, &issues, "off-grid");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"off-grid"));
    // brushes that touch, or are a unit or more apart, are fine
    let mut d = room();
    d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE"));
    d.insert_brush(make_box(v3(64.0, 0.0, 0.0), v3(128.0, 64.0, 64.0), "CRATE"));
    d.insert_brush(make_box(v3(129.0, 0.0, 0.0), v3(160.0, 64.0, 64.0), "CRATE"));
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"micro-gap"));
}

#[test]
fn hidden_faces_count_and_null_them_all() {
    let mut d = room();
    // a func_wall flat against the west wall: its back face is hidden and the compiler keeps it
    tie(&mut d, make_box(v3(-256.0, -32.0, 0.0), v3(-240.0, 32.0, 64.0), "CRATE"), "func_wall");
    let issues = deeper(&d, &opts("ls_test"));
    let h = issues.iter().find(|i| i.code == "hidden-faces").expect("hidden");
    assert!(h.message.contains("saves about"), "{}", h.message);
    let mut hist = History::default();
    fix_all(&mut d, &mut hist, &issues, "hidden-faces");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"hidden-faces"));
    assert!(d.brushes.values().any(|b| b.faces.iter().any(|f| f.texture == "NULL")));
    // inside faces stay
    let floor_top = d.brushes.values().next().unwrap().faces.iter().find(|f| f.plane().is_some_and(|p| p.normal.z > 0.9)).unwrap();
    assert_eq!(floor_top.texture, "CRATE");
    hist.undo(&mut d);
    assert!(!d.brushes.values().any(|b| b.faces.iter().any(|f| f.texture == "NULL")));
}

#[test]
fn texture_seams_align_with_the_neighbor() {
    let mut d = room();
    let a = d.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 32.0), "CRATE"));
    let mut b = make_box(v3(64.0, 0.0, 0.0), v3(128.0, 64.0, 32.0), "CRATE");
    let top = b.faces.iter().position(|f| f.plane().is_some_and(|p| p.normal.z > 0.9)).unwrap();
    b.faces[top].align.u_offset += 5.0;
    let bid = d.insert_brush(b);
    let issues = deeper(&d, &opts("ls_test"));
    let seam = issues.iter().find(|i| i.code == "texture-seam").expect("seam");
    let fd = seam.fix_data.as_ref().unwrap();
    assert!((seam.brush == Some(bid.0) && fd.other_brush == Some(a.0)) || (seam.brush == Some(a.0) && fd.other_brush == Some(bid.0)));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "texture-seam");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"texture-seam"));
    // a whole texture width apart lines up
    let mut d2 = room();
    d2.insert_brush(make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 32.0), "CRATE"));
    let mut b = make_box(v3(64.0, 0.0, 0.0), v3(128.0, 64.0, 32.0), "CRATE");
    b.faces[top].align.u_offset += 64.0;
    d2.insert_brush(b);
    assert!(!codes(&deeper(&d2, &opts("ls_test"))).contains(&"texture-seam"));
}

#[test]
fn stretched_textures_with_adjustable_limits() {
    let mut d = room();
    let mut b = make_box(v3(0.0, 0.0, 0.0), v3(64.0, 64.0, 64.0), "CRATE");
    b.faces[0].align.scale_u = 0.05;
    b.faces[1].align.scale_v = 6.0;
    d.insert_brush(b);
    let issues = deeper(&d, &opts("ls_test"));
    assert_eq!(issues.iter().filter(|i| i.code == "stretched-texture").count(), 2);
    let loose = Options { min_scale: 0.01, max_scale: 8.0, ..opts("ls_test") };
    assert!(!codes(&deeper(&d, &loose)).contains(&"stretched-texture"));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "stretched-texture");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"stretched-texture"));
}

#[test]
fn ladders_must_touch_a_wall_and_reach_floor_and_top() {
    let mut d = room();
    // against the west wall, floor to a ledge at 128 (the ledge is a block on the wall)
    d.insert_brush(make_box(v3(-256.0, 64.0, 0.0), v3(-160.0, 160.0, 128.0), "CRATE"));
    tie(&mut d, make_box(v3(-256.0, 0.0, 0.0), v3(-248.0, 32.0, 128.0), "CLIP"), "func_ladder");
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"ladder-gap"));
    // one hanging in the middle of the room
    let (bad, _) = tie(&mut d, make_box(v3(0.0, 0.0, 64.0), v3(8.0, 32.0, 200.0), "CLIP"), "func_ladder");
    let issues = deeper(&d, &opts("ls_test"));
    let l: Vec<&Issue> = issues.iter().filter(|i| i.code == "ladder-gap").collect();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].entity, Some(bad.0));
    assert!(l[0].message.contains("isn't against a wall") && l[0].message.contains("doesn't reach the floor"), "{}", l[0].message);
}

#[test]
fn lighting_basics() {
    let mut d = room();
    // a sky ceiling and no sun
    let ceiling = *d.brushes.keys().nth(1).unwrap();
    let mut b = d.brushes[&ceiling].clone();
    b.faces.iter_mut().for_each(|f| f.texture = "sky".into());
    d.brushes.insert(ceiling, b);
    assert!(codes(&deeper(&d, &opts("ls_test"))).contains(&"no-sun"));
    ent(&mut d, "light_environment", v3(0.0, 0.0, 128.0));
    assert!(!codes(&deeper(&d, &opts("ls_test"))).contains(&"no-sun"));
    // a room with no light at all
    let mut dark = room();
    let lamp = *dark.entities.iter().find(|(_, e)| e.classname() == "light").unwrap().0;
    dark.entities.remove(&lamp);
    let issues = deeper(&dark, &opts("ls_test"));
    let r = issues.iter().find(|i| i.code == "dark-room").expect("dark room");
    assert_eq!(r.fix, Some("place:light"));
    assert!(r.at.is_some());
    // a light texture counts as a light
    let floor = *dark.brushes.keys().next().unwrap();
    let mut b = dark.brushes[&floor].clone();
    b.faces.iter_mut().for_each(|f| f.texture = "~LIGHT3A".into());
    dark.brushes.insert(floor, b);
    assert!(!codes(&deeper(&dark, &opts("ls_test"))).contains(&"dark-room"));
    // a leaking map says nothing about rooms or the void
    let mut open = room();
    let north = *open.brushes.keys().nth(5).unwrap();
    open.brushes.remove(&north);
    let c = codes(&deeper(&open, &opts("ls_test")));
    assert!(!c.contains(&"dark-room"));
}

#[test]
fn gameplay_by_map_prefix() {
    let mut d = room();
    ent(&mut d, "info_player_start", v3(-128.0, 8.0, 36.0)); // overlaps the first CT spawn
    let mut p = Entity::new("info_map_parameters");
    p.set("buying", "7");
    d.insert_entity(p);
    let de = codes(&deeper(&d, &opts("de_test")));
    for want in ["few-spawns", "spawns-overlap", "no-bombsite", "map-parameters", "no-buyzone"] {
        assert!(de.contains(&want), "missing {want}: {de:?}");
    }
    let cs = codes(&deeper(&d, &opts("cs_test")));
    assert!(cs.contains(&"no-hostages") && cs.contains(&"no-rescue") && !cs.contains(&"no-bombsite"));
    assert!(codes(&deeper(&d, &opts("as_test"))).contains(&"no-vip"));
    assert!(codes(&deeper(&d, &opts("es_test"))).contains(&"no-escape"));
    // mode maps are quiet (their kits check them)
    for quiet in ["ls_x", "br_x", "deathrun_x", "fun_x", "surf_x", "jail_x"] {
        let c = codes(&deeper(&d, &opts(quiet)));
        assert!(!c.iter().any(|x| ["few-spawns", "spawns-overlap", "no-bombsite", "map-parameters"].contains(x)), "{quiet}: {c:?}");
    }
    // the overlap's fix moves the spawn clear of the other one
    let issues = deeper(&d, &opts("de_test"));
    let mut h = History::default();
    fix_all(&mut d, &mut h, &issues, "spawns-overlap");
    assert!(!codes(&deeper(&d, &opts("de_test"))).contains(&"spawns-overlap"));
    // a floating spawn
    let mut f = room();
    ent(&mut f, "info_player_start", v3(0.0, 0.0, 200.0));
    assert!(codes(&deeper(&f, &opts("de_test"))).contains(&"spawn-no-floor"));
    // buy zones: spawns outside their team's zone
    let mut z = room();
    let (zone, _) = tie(&mut z, make_box(v3(-200.0, -64.0, 0.0), v3(-64.0, 64.0, 128.0), "AAATRIGGER"), "func_buyzone");
    let mut ze = z.entities[&zone].clone();
    ze.set("team", "2");
    z.entities.insert(zone, ze);
    let c = deeper(&z, &opts("de_test"));
    let nb: Vec<&Issue> = c.iter().filter(|i| i.code == "spawn-no-buyzone").collect();
    assert_eq!(nb.len(), 1, "{nb:?}");
    assert!(nb[0].message.contains(" T spawn"));
}

#[test]
fn heavy_views_pick_far_apart_spots() {
    let pts = vec![(v3(0.0, 0.0, 0.0), 3000), (v3(10.0, 0.0, 0.0), 2900), (v3(2000.0, 0.0, 0.0), 2600), (v3(4000.0, 0.0, 0.0), 100), (v3(5000.0, 0.0, 0.0), 100), (v3(6000.0, 0.0, 0.0), 100), (v3(7000.0, 0.0, 0.0), 100), (v3(8000.0, 0.0, 0.0), 100)];
    let mut out = Vec::new();
    views::heavy_views(&pts, HEAVY_PVS, "test", &mut out);
    assert_eq!(out.len(), 2);
    // heavy everywhere: one line for the whole map
    let open: Vec<(Vec3, usize)> = (0..10).map(|i| (v3(i as f64 * 1000.0, 0.0, 0.0), 5000 + i)).collect();
    let mut one = Vec::new();
    views::heavy_views(&open, HEAVY_PVS, "test", &mut one);
    assert_eq!(one.len(), 1);
    assert!(one[0].message.contains("whole map"));
    assert!(out[0].message.contains("3000") && out[0].at == Some([0.0, 0.0, 0.0]));
    // the sampled estimate sees the room's faces
    let d = room();
    let sc = scan::Scan::new(&d);
    let f = views::Field::build(&sc).unwrap();
    assert!(!f.leaks);
    let v = views::sampled_views(&sc, &f, &Default::default());
    assert!(!v.is_empty() && v.iter().all(|(_, n)| *n >= 6), "{v:?}");
}

#[test]
fn reads_vis_from_a_compiled_map() {
    // the starter map's BSP from the compile tests, when it's there
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/testdata");
    let Some(bsp) = walk(&p).into_iter().find(|f| f.extension().is_some_and(|e| e == "bsp")) else { return };
    let bytes = std::fs::read(bsp).unwrap();
    if let Ok(v) = views::pvs_counts(&bytes) {
        assert!(v.iter().all(|(_, n)| *n < 1_000_000));
    }
}

fn walk(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            let q = e.path();
            if q.is_dir() {
                out.extend(walk(&q));
            } else {
                out.push(q);
            }
        }
    }
    out
}

/// Real findings on our maps, left for their builders (see docs/notes/Checks.md).
/// (map, code, at most): checked by hand on 2026-11-13, all real.
const KNOWN: &[(&str, &str, usize)] = &[
    // ls_outpost: the floodlight's light is inside the mast (func_detail), the red route
    // light inside its own ~SPOTRED block: both give no light
    ("ls_outpost", "light-in-solid", 2),
    // (br_valley v6, 71f7c21e: only notes left)
];

/// Our own maps (and the last compile's VIS data when there is one): no warnings or problems
/// from the deeper checks, and fast. `LS_CHECKS_MAPS=1 cargo test our_maps -- --nocapture`
/// prints every finding.
#[test]
fn our_maps_pass() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps-src");
    for name in ["ls_outpost", "br_valley"] {
        let p = root.join(name).join(format!("{name}.map"));
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let d = crate::editor::mapfile::read_map(&text).unwrap();
        let t = std::time::Instant::now();
        let all = deeper(&d, &opts(name));
        let ms = t.elapsed().as_millis();
        if std::env::var("LS_CHECKS_MAPS").is_ok() {
            eprintln!("{name}: {} brushes, {ms} ms", d.brushes.len());
            let mut by: std::collections::BTreeMap<&str, usize> = Default::default();
            for i in &all {
                *by.entry(i.code).or_default() += 1;
                if i.level != Level::Info || by[i.code] <= 3 {
                    eprintln!("  {:?} {} {} at {:?} brush {:?} entity {:?}", i.level, i.code, i.message, i.at, i.brush, i.entity);
                }
            }
            eprintln!("  counts: {by:?}");
        }
        let known = |code: &str| KNOWN.iter().find(|(m, c, _)| *m == name && *c == code).map(|k| k.2);
        let count = |code: &str| all.iter().filter(|i| i.code == code).count();
        let bad: Vec<String> = all.iter().filter(|i| i.level != Level::Info).filter(|i| known(i.code).is_none_or(|n| count(i.code) > n)).map(|i| format!("{} {} at {:?}", i.code, i.message, i.at)).collect();
        assert!(bad.is_empty(), "{name}: {bad:#?}");
        assert!(all.iter().filter(|i| i.level != Level::Info).all(|i| i.at.is_some() || i.brush.is_some() || i.entity.is_some()), "{name}: a finding with no location");
        timing(&name, ms);
    }
}

/// A real Hammer map (ReGameDLL's test map, RMF), when it's on this computer
/// (`LS_CHECKS_RMF=<file.rmf>`): runs fast; `LS_CHECKS_MAPS=1` prints what it finds.
#[test]
fn a_hammer_import_runs_fast() {
    let Ok(p) = std::env::var("LS_CHECKS_RMF") else { return };
    let Ok(bytes) = std::fs::read(&p) else { return };
    let d = crate::editor::rmf::read_rmf(&bytes).unwrap().doc;
    let name = std::path::Path::new(&p).file_stem().unwrap().to_string_lossy().to_string();
    let t = std::time::Instant::now();
    let all = deeper(&d, &opts(&name));
    let ms = t.elapsed().as_millis();
    if std::env::var("LS_CHECKS_MAPS").is_ok() {
        eprintln!("{name}: {} brushes, {} entities, {ms} ms", d.brushes.len(), d.entities.len());
        let mut by: std::collections::BTreeMap<&str, usize> = Default::default();
        for i in &all {
            *by.entry(i.code).or_default() += 1;
            if by[i.code] <= 4 {
                eprintln!("  {:?} {} {} at {:?} brush {:?} entity {:?}", i.level, i.code, i.message, i.at, i.brush, i.entity);
            }
        }
        eprintln!("  counts: {by:?}");
    }
    timing(&name, ms);
}

/// Calibration for the heavy-view limits: the heaviest sampled and VIS views of our maps
/// (`cargo test --release heavy_view_numbers -- --ignored --nocapture`, with LS_CHECKS_BSP_DIR
/// the folder with their compiled .bsp files).
#[test]
#[ignore]
fn heavy_view_numbers() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps-src");
    for name in ["ls_outpost", "br_valley"] {
        let Ok(text) = std::fs::read_to_string(root.join(name).join(format!("{name}.map"))) else { continue };
        let d = crate::editor::mapfile::read_map(&text).unwrap();
        let sc = scan::Scan::new(&d);
        let f = views::Field::build(&sc).unwrap();
        let h = surfaces::hidden_faces(&sc, Some(&f));
        let mut v = views::sampled_views(&sc, &f, &h.set);
        v.sort_by(|a, b| b.1.cmp(&a.1));
        eprintln!("{name}: cell {} leaks {} samples {} top {:?}", f.s, f.leaks, v.len(), &v[..v.len().min(5)]);
        if let Ok(dir) = std::env::var("LS_CHECKS_BSP_DIR") {
            if let Ok(b) = std::fs::read(std::path::Path::new(&dir).join(format!("{name}.bsp"))) {
                let mut p = views::pvs_counts(&b).unwrap();
                p.sort_by(|a, b| b.1.cmp(&a.1));
                let med = p.get(p.len() / 2).map(|x| x.1);
                eprintln!("  pvs leaves {} median {:?} top {:?}", p.len(), med, &p[..p.len().min(5)]);
            }
        }
    }
}

/// Speed is checked only when asked (`LS_CHECKS_TIMING_TEST=1`, in a release build on a quiet
/// machine): a busy Mac or a debug build would make it flaky.
fn timing(name: &str, ms: u128) {
    if std::env::var("LS_CHECKS_TIMING_TEST").is_err() || cfg!(debug_assertions) {
        return;
    }
    assert!(ms < 1_500, "{name}: the checks took {ms} ms (release; ~370 ms expected for br_valley on a quiet machine)");
}
