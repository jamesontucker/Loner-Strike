//! Prefabs: a saved piece of a map (brushes and entities with their textures) to place again
//! in any map. On disk a prefab is two files side by side, `<id>.map` (a small Valve 220 .map,
//! readable by any GoldSrc editor) and `<id>.json` (name, tags, author, size, textures and the
//! WADs they come from), plus an optional `<id>.png` thumbnail. The same three files under
//! `prefabs/` make a community package (docs/COMMUNITY.md, "Prefab packages").
//!
//! Prefabs are stored centred: the middle of their footprint at x = y = 0 and their bottom at
//! z = 0 (whole units), so `placement` can stand them on a floor, hang them from a ceiling or
//! put them against a wall, with their corner on the grid.

use super::doc::{Brush, Entity, Material, MapDocument};
use super::geom::brush_bounds;
use super::mapfile;
use super::math::{v3, Bounds, Vec3};
use super::ops;
use serde::{Deserialize, Serialize};

/// The prefab JSON's format; a newer one is refused with a readable message.
pub const PREFAB_FORMAT: u32 = 1;
/// Point entities count as a box this big (most are about player size or smaller).
const POINT_HALF: f64 = 8.0;
pub const MAX_PREFAB_MAP: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrefabMeta {
    #[serde(default)]
    pub format: u32,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// seconds since 1970
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub brushes: usize,
    #[serde(default)]
    pub entities: usize,
    /// width, depth, height in units
    #[serde(default)]
    pub size: [f64; 3],
    #[serde(default)]
    pub textures: Vec<String>,
    /// WAD file names the textures come from (added to a map's list when placed)
    #[serde(default)]
    pub wads: Vec<String>,
    /// texture sizes, so alignment is right before the map's WADs are read
    #[serde(default)]
    pub materials: Vec<Material>,
}

/// Cleans what a person typed: one line, a length cap.
pub fn clean_text(s: &str, max: usize) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

pub fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags.iter().flat_map(|t| t.split(',')) {
        let t = clean_text(t, 24).to_lowercase();
        if !t.is_empty() && !out.contains(&t) && out.len() < 12 {
            out.push(t);
        }
    }
    out
}

/// A file-name-safe id from a name ("Crate stack #2" -> "crate_stack_2").
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('_') && !s.is_empty() {
            s.push('_');
        }
    }
    let s: String = s.trim_end_matches('_').chars().take(48).collect();
    if s.is_empty() {
        "prefab".into()
    } else {
        s
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// What a prefab takes up: its brushes and a small box round each point entity.
pub fn bounds(frag: &MapDocument) -> Bounds {
    let mut b = Bounds::empty();
    for br in frag.brushes.values() {
        let bb = brush_bounds(br);
        if !bb.is_empty() {
            b = b.union(&bb);
        }
    }
    let owned: std::collections::HashSet<_> = frag.brushes.values().filter_map(|br| br.entity).collect();
    for (id, e) in &frag.entities {
        if owned.contains(id) {
            continue;
        }
        if let Some(o) = e.origin() {
            b.add(o - v3(POINT_HALF, POINT_HALF, POINT_HALF));
            b.add(o + v3(POINT_HALF, POINT_HALF, POINT_HALF));
        }
    }
    b
}

fn shift_entity(e: &mut Entity, d: Vec3) {
    if let Some(o) = e.origin() {
        let n = o + d;
        e.set("origin", &format!("{} {} {}", mapfile::fmt_num(n.x), mapfile::fmt_num(n.y), mapfile::fmt_num(n.z)));
    }
}

/// Moves a fragment so its footprint's middle is at x = y = 0 and its bottom at z = 0 (by
/// whole units, so a prefab made on the grid stays on it).
pub fn centre(frag: &mut MapDocument) -> Result<(), String> {
    let b = bounds(frag);
    if b.is_empty() {
        return Err("That prefab is empty.".into());
    }
    let c = b.center();
    let d = v3(-c.x.round(), -c.y.round(), -b.min.z.round());
    if d == Vec3::ZERO {
        return Ok(());
    }
    let ids: Vec<_> = frag.brushes.keys().copied().collect();
    for id in ids {
        let nb = ops::translate(&frag.brushes[&id], d, true)?;
        frag.brushes.insert(id, Brush { entity: frag.brushes[&id].entity, group: None, ..nb });
    }
    // (a brush entity's origin key, when it has one, is its turning centre: it moves too)
    for e in frag.entities.values_mut() {
        shift_entity(e, d);
    }
    Ok(())
}

/// Facts for the JSON: counts, size, textures and their WADs.
pub fn describe(frag: &MapDocument, name: &str, tags: &[String], author: &str, created: u64) -> PrefabMeta {
    let b = bounds(frag);
    let size = if b.is_empty() { [0.0; 3] } else { let s = b.size(); [s.x.round(), s.y.round(), s.z.round()] };
    let mut textures: Vec<String> = Vec::new();
    for br in frag.brushes.values() {
        for f in &br.faces {
            if !textures.iter().any(|t| t.eq_ignore_ascii_case(&f.texture)) {
                textures.push(f.texture.clone());
            }
        }
    }
    textures.sort_by_key(|t| t.to_ascii_lowercase());
    let materials: Vec<Material> = textures.iter().filter_map(|t| frag.material(t).cloned()).collect();
    let mut wads: Vec<String> = Vec::new();
    for w in materials.iter().filter_map(|m| m.wad.as_deref()).chain(frag.world.wads.iter().map(String::as_str)) {
        let base = w.rsplit(['\\', '/']).next().unwrap_or(w).to_ascii_lowercase();
        if !base.is_empty() && base.ends_with(".wad") && !wads.contains(&base) {
            wads.push(base);
        }
    }
    PrefabMeta {
        format: PREFAB_FORMAT,
        name: clean_text(name, 64),
        tags: clean_tags(tags),
        author: clean_text(author, 48),
        description: String::new(),
        created,
        brushes: frag.brushes.len(),
        entities: frag.entities.len(),
        size,
        textures,
        wads,
        materials,
    }
}

/// The two files of a prefab: (.map text, .json text).
pub fn to_files(frag: &MapDocument, meta: &PrefabMeta) -> Result<(String, String), String> {
    let mut f = frag.clone();
    f.world.wads = meta.wads.clone();
    f.world.keys.retain(|(k, _)| k == "comment");
    if f.world.keys.is_empty() {
        f.world.keys.push(("comment".into(), format!("Loner-Strike prefab: {}", meta.name)));
    }
    let map = mapfile::write_map(&f, &mapfile::WriteOptions::default())?;
    let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    Ok((map, json))
}

/// Reads a prefab's .map (and its JSON, when there is one; a plain .map gets one made up).
pub fn from_files(map: &str, json: Option<&str>, fallback_name: &str) -> Result<(MapDocument, PrefabMeta), String> {
    if map.len() > MAX_PREFAB_MAP {
        return Err("That prefab is too big (over 8 MB).".into());
    }
    let mut frag = mapfile::read_map(map).map_err(|e| format!("That prefab's .map couldn't be read: {e}"))?;
    if frag.brushes.is_empty() && frag.entities.is_empty() {
        return Err("That prefab is empty.".into());
    }
    if frag.brushes.len() > 50_000 || frag.entities.len() > 10_000 {
        return Err("That prefab is too big.".into());
    }
    // groups don't come along: placing makes one group of the lot
    frag.groups.clear();
    for b in frag.brushes.values_mut() {
        b.group = None;
    }
    for e in frag.entities.values_mut() {
        e.group = None;
    }
    let meta = match json {
        Some(j) => {
            let m: PrefabMeta = serde_json::from_str(j).map_err(|e| format!("That prefab's details couldn't be read: {e}"))?;
            if m.format > PREFAB_FORMAT {
                return Err("This prefab was made by a newer Loner-Strike. Update the app to use it.".into());
            }
            m
        }
        None => describe(&frag, fallback_name, &[], "", 0),
    };
    for m in &meta.materials {
        if m.width > 0 && m.height > 0 && m.width <= 4096 && m.height <= 4096 && !m.name.is_empty() && m.name.len() <= 15 {
            frag.add_material(m.clone());
        }
    }
    // pieces made elsewhere (a GameBanana .map) are centred like ours
    centre(&mut frag)?;
    let mut meta = meta;
    let fresh = describe(&frag, &meta.name, &meta.tags, &meta.author, meta.created);
    meta.name = if fresh.name.is_empty() { clean_text(fallback_name, 64) } else { fresh.name };
    meta.tags = fresh.tags;
    meta.author = fresh.author;
    meta.description = clean_text(&meta.description, 400);
    meta.brushes = fresh.brushes;
    meta.entities = fresh.entities;
    meta.size = fresh.size;
    meta.textures = fresh.textures;
    if meta.wads.is_empty() {
        meta.wads = fresh.wads;
    } else {
        meta.wads = meta.wads.iter().map(|w| w.rsplit(['\\', '/']).next().unwrap_or(w).to_ascii_lowercase()).filter(|w| w.ends_with(".wad") && w.len() <= 64).collect();
    }
    meta.format = PREFAB_FORMAT;
    Ok((frag, meta))
}

fn snap(x: f64, grid: f64) -> f64 {
    if grid > 0.0 {
        (x / grid).round() * grid
    } else {
        x.round()
    }
}

/// Where a prefab goes when dropped at `point` on a surface facing `normal`: the offset to
/// add to its pieces. Across the surface its middle follows the mouse with its corner on the
/// grid; along the surface's main axis it sits on it: on a floor, under a ceiling, against a wall.
pub fn placement(b: &Bounds, point: Vec3, normal: Vec3, grid: f64) -> Vec3 {
    let n = if normal.is_finite() && normal.len() > 0.5 { normal } else { v3(0.0, 0.0, 1.0) };
    let a = n.abs().major_axis();
    let get = |v: Vec3, i: usize| [v.x, v.y, v.z][i];
    let mut out = [0.0; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let (lo, hi, p) = (get(b.min, i), get(b.max, i), get(point, i));
        if i == a {
            let surface = (p * 1000.0).round() / 1000.0;
            *o = if get(n, i) >= 0.0 { surface - lo } else { surface - hi };
        } else {
            let half = (hi - lo) / 2.0;
            *o = snap(p - half, grid) - lo;
        }
    }
    v3(out[0], out[1], out[2])
}

/// How far a prefab turns about z when it goes on a surface: on a wall, so its front (+y, the
/// side away from its back: a ladder's rungs, a doorway's face) points out of the wall; on
/// floors and ceilings not at all. Whole quarter turns for axis-aligned walls.
pub fn wall_yaw(normal: Vec3) -> f64 {
    let n = if normal.is_finite() && normal.len() > 0.5 { normal.normalized() } else { return 0.0 };
    if n.z.abs() >= 0.7 {
        return 0.0;
    }
    let yaw = n.y.atan2(n.x).to_degrees() - 90.0;
    let q = (yaw / 90.0).round() * 90.0;
    let yaw = if (yaw - q).abs() < 1.0 { q } else { yaw };
    yaw.rem_euclid(360.0)
}

/// A box turned about z (round the origin): the box round its turned corners.
pub fn turned_bounds(b: &Bounds, yaw: f64) -> Bounds {
    if yaw.rem_euclid(360.0).abs() < 1e-9 {
        return *b;
    }
    let m = super::math::Mat3::rotation(v3(0.0, 0.0, 1.0), yaw);
    let mut out = Bounds::empty();
    for k in 0..8 {
        let c = v3(if k & 1 != 0 { b.max.x } else { b.min.x }, if k & 2 != 0 { b.max.y } else { b.min.y }, if k & 4 != 0 { b.max.z } else { b.min.z });
        out.add(m.mul_vec(c));
    }
    out
}

/// The whole move for dropping a prefab: turned (to face out of a wall, plus `extra_yaw`
/// degrees the player asked for), then placed on the surface (`placement`). Also the turn used.
pub fn place_transform(b: &Bounds, point: Vec3, normal: Vec3, grid: f64, extra_yaw: f64, face_out: bool) -> (super::math::Affine, f64) {
    use super::math::{Affine, Mat3};
    let extra = if extra_yaw.is_finite() { extra_yaw } else { 0.0 };
    let yaw = ((if face_out { wall_yaw(normal) } else { 0.0 }) + extra).rem_euclid(360.0);
    let turned = turned_bounds(b, yaw);
    let off = placement(&turned, point, normal, grid);
    let turn = if yaw.abs() < 1e-9 { Affine::IDENTITY } else { Affine::around(Mat3::rotation(v3(0.0, 0.0, 1.0), yaw), Vec3::ZERO) };
    (turn.then(&Affine::translate(off)), yaw)
}

// ---- the starter prefabs -------------------------------------------------------------------

fn bx(f: &mut MapDocument, a: [f64; 3], b: [f64; 3], tex: &str) -> super::doc::BrushId {
    f.insert_brush(ops::make_box(v3(a[0], a[1], a[2]), v3(b[0], b[1], b[2]), tex))
}

fn tie(f: &mut MapDocument, ids: &[super::doc::BrushId], class: &str, keys: &[(&str, &str)]) {
    let mut e = Entity::new(class);
    for (k, v) in keys {
        e.set(k, v);
    }
    let eid = f.insert_entity(e);
    for id in ids {
        if let Some(b) = f.brushes.get_mut(id) {
            b.entity = Some(eid);
        }
    }
}

fn point(f: &mut MapDocument, class: &str, at: [f64; 3], keys: &[(&str, &str)]) {
    let mut e = Entity::new(class);
    e.set("origin", &format!("{} {} {}", at[0], at[1], at[2]));
    for (k, v) in keys {
        e.set(k, v);
    }
    f.insert_entity(e);
}

/// Prefabs that come with the app, made here from boxes (so they're always valid), all with
/// textures from halflife.wad: (id, name, tags, pieces).
pub fn builtins() -> Vec<(&'static str, PrefabMeta, MapDocument)> {
    let mut out = Vec::new();
    let mut add = |id: &'static str, name: &str, tags: &[&str], desc: &str, f: MapDocument| {
        let mut f = f;
        f.world.wads = vec!["halflife.wad".into()];
        let _ = centre(&mut f);
        let mut m = describe(&f, name, &tags.iter().map(|t| t.to_string()).collect::<Vec<_>>(), "Loner-Strike", 0);
        m.description = desc.into();
        out.push((id, m, f));
    };

    // doorway: a wall 192 wide with a 64 x 112 opening and a sliding door in it
    let mut f = MapDocument::new();
    bx(&mut f, [-96.0, -8.0, 0.0], [-32.0, 8.0, 128.0], "OUT_WALL5");
    bx(&mut f, [32.0, -8.0, 0.0], [96.0, 8.0, 128.0], "OUT_WALL5");
    bx(&mut f, [-32.0, -8.0, 112.0], [32.0, 8.0, 128.0], "OUT_WALL5");
    let door = bx(&mut f, [-32.0, -2.0, 0.0], [32.0, 2.0, 112.0], "LAB1_DOOR1");
    tie(&mut f, &[door], "func_door", &[("angle", "-1"), ("speed", "100"), ("wait", "4"), ("lip", "8")]);
    add("doorway", "Doorway", &["door", "wall"], "A wall with a door that slides up when you walk into it.", f);

    // stairs: 8 steps of 16 x 16, 64 wide (players walk up 18 units)
    let mut f = MapDocument::new();
    for i in 0..8 {
        let y = i as f64 * 16.0;
        bx(&mut f, [-32.0, y, 0.0], [32.0, y + 16.0, (i + 1) as f64 * 16.0], "CRETE3_FLR04");
    }
    add("stairs", "Stairs", &["stairs", "floor"], "Eight 16-unit steps, 64 wide: a player walks up them.", f);

    // crate stack: two crates side by side, one on top, a small one beside them
    let mut f = MapDocument::new();
    bx(&mut f, [-64.0, -32.0, 0.0], [0.0, 32.0, 64.0], "CRATE01");
    bx(&mut f, [0.0, -32.0, 0.0], [64.0, 32.0, 64.0], "CRATE02");
    bx(&mut f, [-32.0, -32.0, 64.0], [32.0, 32.0, 128.0], "CRATE01");
    bx(&mut f, [64.0, -16.0, 0.0], [96.0, 16.0, 32.0], "CRATE05");
    add("crate_stack", "Crate stack", &["crate", "cover"], "Cover: three big crates and a small one to jump up on.", f);

    // ladder: the climbable volume plus the rungs you see
    let mut f = MapDocument::new();
    let vol = bx(&mut f, [-16.0, -4.0, 0.0], [16.0, 4.0, 192.0], "AAATRIGGER");
    tie(&mut f, &[vol], "func_ladder", &[]);
    let rungs = bx(&mut f, [-16.0, 4.0, 0.0], [16.0, 6.0, 192.0], "{LADDER1");
    tie(&mut f, &[rungs], "func_illusionary", &[("rendermode", "4"), ("renderamt", "255")]);
    add("ladder", "Ladder", &["ladder", "climb"], "A 192-unit ladder: put its back against a wall.", f);

    // window: a wall with an opening and breakable glass in it
    let mut f = MapDocument::new();
    bx(&mut f, [-96.0, -8.0, 0.0], [96.0, 8.0, 48.0], "OUT_WALL5");
    bx(&mut f, [-96.0, -8.0, 112.0], [96.0, 8.0, 128.0], "OUT_WALL5");
    bx(&mut f, [-96.0, -8.0, 48.0], [-48.0, 8.0, 112.0], "OUT_WALL5");
    bx(&mut f, [48.0, -8.0, 48.0], [96.0, 8.0, 112.0], "OUT_WALL5");
    let glass = bx(&mut f, [-48.0, -1.0, 48.0], [48.0, 1.0, 112.0], "GLASS_MED");
    tie(&mut f, &[glass], "func_breakable", &[("material", "0"), ("health", "20"), ("rendermode", "2"), ("renderamt", "110")]);
    add("window", "Window", &["window", "glass", "wall"], "A wall with a pane of glass that breaks when shot.", f);

    // bombsite kit: the bomb zone with cover crates
    let mut f = MapDocument::new();
    let zone = bx(&mut f, [-128.0, -128.0, 0.0], [128.0, 128.0, 128.0], "AAATRIGGER");
    tie(&mut f, &[zone], "func_bomb_target", &[]);
    bx(&mut f, [-96.0, 32.0, 0.0], [-32.0, 96.0, 64.0], "CRATE01");
    bx(&mut f, [32.0, -96.0, 0.0], [96.0, -32.0, 64.0], "CRATE02");
    bx(&mut f, [48.0, -80.0, 64.0], [80.0, -48.0, 96.0], "CRATE05");
    add("bombsite", "Bombsite kit", &["bomb", "objective", "de_"], "A bomb zone (Terrorists plant here) with crates for cover.", f);

    // buy zone with four Counter-Terrorist spawns
    let mut f = MapDocument::new();
    let buy = bx(&mut f, [-192.0, -128.0, 0.0], [192.0, 128.0, 128.0], "AAATRIGGER");
    tie(&mut f, &[buy], "func_buyzone", &[("team", "2")]);
    for x in [-96.0, -32.0, 32.0, 96.0] {
        point(&mut f, "info_player_start", [x, -48.0, 36.0], &[("angle", "90")]);
    }
    for x in [-64.0, 0.0, 64.0] {
        point(&mut f, "info_player_start", [x, 32.0, 36.0], &[("angle", "90")]);
    }
    add(
        "buyzone_ct",
        "Buy zone + CT spawns",
        &["spawn", "buy", "ct"],
        "A Counter-Terrorist buy zone with seven spawns facing north. For Terrorists use info_player_deathmatch and team 1.",
        f,
    );

    // light fixture: a lamp to hang from a ceiling, with the light under it
    let mut f = MapDocument::new();
    bx(&mut f, [-16.0, -16.0, 0.0], [16.0, 16.0, 4.0], "~LIGHT3A");
    bx(&mut f, [-20.0, -20.0, 4.0], [20.0, 20.0, 8.0], "METAL_BORD01");
    point(&mut f, "light", [0.0, 0.0, -24.0], &[("_light", "255 236 200 200")]);
    add("light_fixture", "Light fixture", &["light", "ceiling"], "A ceiling lamp with a light under it. Drop it on a ceiling.", f);

    out
}

// ---- thumbnails ----------------------------------------------------------------------------

/// A colour per texture that reads like it (crates brown, glass blue, triggers orange...).
fn tex_colour(t: &str) -> Option<[f32; 3]> {
    let l = t.to_ascii_lowercase();
    if ["null", "skip", "hint", "clip", "origin", "bevel", "solidhint"].contains(&l.as_str()) {
        return None;
    }
    Some(if l.starts_with("aaatrigger") {
        [0.95, 0.55, 0.2]
    } else if l == "sky" {
        [0.45, 0.65, 0.9]
    } else if l.contains("crate") {
        [0.62, 0.45, 0.26]
    } else if l.contains("glass") || l.contains("water") {
        [0.55, 0.75, 0.85]
    } else if l.contains("light") {
        [1.0, 0.95, 0.75]
    } else if l.contains("ladder") || l.contains("metal") || l.contains("grate") {
        [0.55, 0.57, 0.6]
    } else if l.contains("door") {
        [0.5, 0.42, 0.36]
    } else {
        let h = l.bytes().fold(7u32, |a, c| a.wrapping_mul(31).wrapping_add(c as u32));
        let k = (h % 1000) as f32 / 1000.0;
        [0.55 + 0.2 * k, 0.53 + 0.12 * (1.0 - k), 0.48 + 0.1 * k]
    })
}

/// A small picture of a prefab from above and to the side (flat shaded, painter's order),
/// for prefabs with no thumbnail of their own. PNG, `w` x `h`, transparent background.
pub fn thumbnail(frag: &MapDocument, w: u32, h: u32) -> Vec<u8> {
    let cam = v3(1.0, -1.3, 1.0).normalized();
    let right = v3(1.3, 1.0, 0.0).normalized();
    let up = cam.cross(right).normalized();
    let up = if up.z < 0.0 { up * -1.0 } else { up };
    let light = v3(0.4, -0.6, 0.9).normalized();
    let mut polys: Vec<(f64, Vec<(f64, f64)>, [f32; 3], bool)> = Vec::new();
    for b in frag.brushes.values() {
        for f in ops::brush_mesh(b, |_| (64.0, 64.0)) {
            if f.normal.dot(cam) <= 0.0 || f.vertices.len() < 3 {
                continue;
            }
            let Some(c) = tex_colour(&f.texture) else { continue };
            // trigger volumes (buy zones, bomb sites, ladders) are outlines, so what's inside shows
            let outline = f.texture.to_ascii_lowercase().starts_with("aaatrigger");
            let shade = 0.5 + 0.5 * f.normal.dot(light).max(0.0) as f32;
            let depth = f.vertices.iter().map(|p| p.dot(cam)).sum::<f64>() / f.vertices.len() as f64;
            let pts = f.vertices.iter().map(|p| (p.dot(right), p.dot(up))).collect();
            polys.push((depth, pts, [c[0] * shade, c[1] * shade, c[2] * shade], outline));
        }
    }
    let owned: std::collections::HashSet<_> = frag.brushes.values().filter_map(|br| br.entity).collect();
    for (id, e) in &frag.entities {
        if owned.contains(id) {
            continue;
        }
        let Some(o) = e.origin() else { continue };
        let c = if e.classname().starts_with("light") { [1.0, 0.85, 0.3] } else if e.classname().starts_with("info_player") { [0.35, 0.8, 0.45] } else { [0.85, 0.4, 0.85] };
        let (x, y, r) = (o.dot(right), o.dot(up), 7.0);
        polys.push((o.dot(cam) + 1000.0, vec![(x, y + r), (x + r, y), (x, y - r), (x - r, y)], c, false));
    }
    let mut img = image::RgbaImage::new(w.max(1), h.max(1));
    if polys.is_empty() {
        return encode(&img);
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (_, pts, _, _) in &polys {
        for (x, y) in pts {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
            y0 = y0.min(*y);
            y1 = y1.max(*y);
        }
    }
    let pad = 6.0;
    let k = ((w as f64 - 2.0 * pad) / (x1 - x0).max(1.0)).min((h as f64 - 2.0 * pad) / (y1 - y0).max(1.0));
    let (ox, oy) = (w as f64 / 2.0 - (x0 + x1) / 2.0 * k, h as f64 / 2.0 + (y0 + y1) / 2.0 * k);
    polys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    // outlines last, over everything
    polys.sort_by_key(|p| p.3);
    for (_, pts, c, outline) in &polys {
        let sp: Vec<(f64, f64)> = pts.iter().map(|(x, y)| (ox + x * k, oy - y * k)).collect();
        if !outline {
            fill(&mut img, &sp, [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255]);
        }
        let edge = if *outline { [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255] } else { [(c[0] * 120.0) as u8, (c[1] * 120.0) as u8, (c[2] * 120.0) as u8, 255] };
        for i in 0..sp.len() {
            line(&mut img, sp[i], sp[(i + 1) % sp.len()], edge);
        }
    }
    encode(&img)
}

fn encode(img: &image::RgbaImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    out.into_inner()
}

/// Fills a convex polygon (scanlines through pixel centres).
fn fill(img: &mut image::RgbaImage, pts: &[(f64, f64)], c: [u8; 4]) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let ymin = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor().max(0.0) as i64;
    let ymax = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil().min(h as f64 - 1.0) as i64;
    for y in ymin..=ymax {
        let yc = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= yc && b.1 > yc) || (b.1 <= yc && a.1 > yc) {
                xs.push(a.0 + (yc - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        if xs.len() < 2 {
            continue;
        }
        let (l, r) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
        for x in (l.round() as i64).max(0)..(r.round() as i64).min(w) {
            img.put_pixel(x as u32, y as u32, image::Rgba(c));
        }
    }
}

fn line(img: &mut image::RgbaImage, a: (f64, f64), b: (f64, f64), c: [u8; 4]) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).clamp(1, 4096);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if x >= 0.0 && y >= 0.0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, image::Rgba(c));
        }
    }
}

/// Checks a thumbnail the page sent: a real PNG, not huge.
pub fn check_png(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 1024 * 1024 || !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err("The thumbnail isn't a PNG picture under 1 MB.".into());
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|_| "The thumbnail couldn't be read.".to_string())?;
    if img.width() > 1024 || img.height() > 1024 {
        return Err("The thumbnail is too big.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::is_valid;
    use crate::editor::history::{self, History};

    fn room() -> MapDocument {
        let mut d = MapDocument::new();
        d.insert_brush(ops::make_box(v3(96.0, 32.0, 64.0), v3(160.0, 96.0, 128.0), "CRATE01"));
        d.add_material(Material { name: "CRATE01".into(), width: 128, height: 128, wad: Some("halflife.wad".into()) });
        let wall = d.insert_brush(ops::make_box(v3(160.0, 32.0, 64.0), v3(176.0, 96.0, 192.0), "GLASS_MED"));
        let mut e = Entity::new("func_breakable");
        e.set("health", "20");
        let eid = d.insert_entity(e);
        d.brushes.get_mut(&wall).unwrap().entity = Some(eid);
        let mut l = Entity::new("light");
        l.set("origin", "128 64 160");
        d.insert_entity(l);
        d
    }

    #[test]
    fn save_and_load_round_trip() {
        let d = room();
        let all_b: Vec<_> = d.brushes.keys().copied().collect();
        let all_e: Vec<_> = d.entities.keys().copied().collect();
        let mut frag = history::fragment(&d, &all_b, &all_e);
        centre(&mut frag).unwrap();
        let b = bounds(&frag);
        // footprint centred, bottom at 0
        assert_eq!(b.min.z, 0.0);
        assert_eq!((b.min.x + b.max.x).round(), 0.0);
        assert_eq!((b.min.y + b.max.y).round(), 0.0);
        let meta = describe(&frag, "  My   crate\nwall ", &["Cover, crate".into(), "cover".into()], "jam", 5);
        assert_eq!(meta.name, "My crate wall");
        assert_eq!(meta.tags, vec!["cover", "crate"]);
        assert_eq!(meta.size, [80.0, 64.0, 128.0]);
        assert_eq!((meta.brushes, meta.entities), (2, 2));
        assert_eq!(meta.textures, vec!["CRATE01", "GLASS_MED"]);
        assert_eq!(meta.wads, vec!["halflife.wad"]);
        let (map, json) = to_files(&frag, &meta).unwrap();
        assert!(map.contains("\"classname\" \"func_breakable\"") && map.contains("halflife.wad"));
        let (back, m2) = from_files(&map, Some(&json), "x").unwrap();
        assert_eq!(back.brushes.len(), 2);
        assert_eq!(back.entities.len(), 2);
        assert_eq!(m2.name, meta.name);
        assert_eq!(m2.size, meta.size);
        // texture sizes come back from the JSON
        assert_eq!(back.texture_size("crate01"), (128.0, 128.0));
        assert!(back.brushes.values().all(is_valid));
        // a brush entity keeps its brush
        let fb = back.entities.iter().find(|(_, e)| e.classname() == "func_breakable").map(|(i, _)| *i).unwrap();
        assert_eq!(back.brushes.values().filter(|b| b.entity == Some(fb)).count(), 1);
        // a plain .map (from GameBanana) gets centred and named
        let raw = mapfile::write_map(&d, &mapfile::WriteOptions::default()).unwrap();
        let (p, m3) = from_files(&raw, None, "fence_kit").unwrap();
        assert_eq!(m3.name, "fence_kit");
        assert_eq!(bounds(&p).min.z, 0.0);
        // bad files
        assert!(from_files("not a map", None, "x").is_err());
        assert!(from_files(&map, Some("{\"format\": 9, \"name\": \"x\"}"), "x").unwrap_err().contains("newer"));
        assert!(from_files("{\n\"classname\" \"worldspawn\"\n}\n", None, "x").unwrap_err().contains("empty"));
    }

    #[test]
    fn placement_on_floor_ceiling_and_wall() {
        let b = Bounds { min: v3(-40.0, -32.0, 0.0), max: v3(40.0, 32.0, 128.0) };
        // on a floor at z 64: bottom on it, corner on the 16 grid near the mouse
        let o = placement(&b, v3(103.0, 9.0, 64.0), v3(0.0, 0.0, 1.0), 16.0);
        assert_eq!(o.z, 64.0);
        assert_eq!((b.min.x + o.x) % 16.0, 0.0);
        assert_eq!((b.min.y + o.y) % 16.0, 0.0);
        assert!(((b.center().x + o.x) - 103.0).abs() <= 8.0);
        // under a ceiling at z 256: top touches it
        let o = placement(&b, v3(0.0, 0.0, 256.0), v3(0.0, 0.0, -1.0), 16.0);
        assert_eq!(b.max.z + o.z, 256.0);
        // against a wall facing -x at x 512: its +x side touches it
        let o = placement(&b, v3(512.0, 30.0, 70.0), v3(-1.0, 0.0, 0.0), 8.0);
        assert_eq!(b.max.x + o.x, 512.0);
        assert_eq!((b.min.z + o.z) % 8.0, 0.0);
        // no grid: whole units; a bad normal counts as a floor
        let o = placement(&b, v3(10.4, 0.0, 5.5), v3(f64::NAN, 0.0, 0.0), 0.0);
        assert_eq!(o.z, 5.5);
        assert_eq!(o.x.fract(), 0.0);
    }

    #[test]
    fn prefabs_face_out_of_walls() {
        // the ladder's rungs are on +y: on a wall facing -x they must end up on the -x side
        let (_, _, frag) = builtins().into_iter().find(|(id, _, _)| *id == "ladder").unwrap();
        let b = bounds(&frag);
        assert_eq!(wall_yaw(v3(0.0, 1.0, 0.0)), 0.0);
        assert_eq!(wall_yaw(v3(-1.0, 0.0, 0.0)), 90.0);
        assert_eq!(wall_yaw(v3(1.0, 0.0, 0.0)), 270.0);
        assert_eq!(wall_yaw(v3(0.0, -1.0, 0.0)), 180.0);
        assert_eq!(wall_yaw(v3(0.0, 0.0, 1.0)), 0.0);
        let (t, yaw) = place_transform(&b, v3(512.0, 40.0, 70.0), v3(-1.0, 0.0, 0.0), 8.0, 0.0, true);
        assert_eq!(yaw, 90.0);
        let mut d = MapDocument::new();
        let mut h = History::default();
        history::cmd_place_prefab_with(&mut d, &mut h, &frag, &t, "Ladder", &[]).unwrap();
        let placed = d.brushes.values().fold(Bounds::empty(), |a, br| a.union(&brush_bounds(br)));
        // flush against the wall (its back on x 512) and thin along x now
        assert_eq!(placed.max.x, 512.0);
        assert!(placed.size().x < placed.size().y);
        let rungs = d.brushes.values().find(|br| br.faces[0].texture == "{LADDER1").map(brush_bounds).unwrap();
        assert!(rungs.max.x < 512.0 - 7.0, "rungs face out of the wall: {rungs:?}");
        assert!(d.brushes.values().all(is_valid));
        // turned by hand on a floor: a quarter turn
        let (_, yaw) = place_transform(&b, v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), 16.0, 90.0, true);
        assert_eq!(yaw, 90.0);
        let tb = turned_bounds(&Bounds { min: v3(-40.0, -8.0, 0.0), max: v3(40.0, 8.0, 10.0) }, 90.0);
        assert!((tb.size().x - 16.0).abs() < 1e-9 && (tb.size().y - 80.0).abs() < 1e-9);
    }

    #[test]
    fn placing_is_one_grouped_step() {
        let mut d = MapDocument::new();
        d.world.wads = vec!["cstrike.wad".into()];
        let mut h = History::default();
        let (_, _, frag) = builtins().into_iter().find(|(id, _, _)| *id == "doorway").unwrap();
        let o = placement(&bounds(&frag), v3(100.0, 0.0, 32.0), v3(0.0, 0.0, 1.0), 16.0);
        let (made, g) = history::cmd_place_prefab(&mut d, &mut h, &frag, o, "Doorway", &["halflife.wad".into()]).unwrap();
        assert_eq!(made.brushes.len(), 4);
        assert_eq!(made.entities.len(), 1);
        assert_eq!(d.groups[&g].name, "Doorway");
        assert_eq!(d.world.wad_names(), vec!["cstrike.wad", "halflife.wad"]);
        // the world brushes and the door entity carry the group; the door's brush stays with its entity
        assert_eq!(d.brushes.values().filter(|b| b.group == Some(g)).count(), 3);
        assert_eq!(d.entities.values().filter(|e| e.group == Some(g)).count(), 1);
        let placed = Bounds { min: v3(f64::MAX, f64::MAX, f64::MAX), max: v3(f64::MIN, f64::MIN, f64::MIN) };
        let placed = d.brushes.values().fold(placed, |a, b| a.union(&brush_bounds(b)));
        assert_eq!(placed.min.z, 32.0);
        assert_eq!(placed.min.x % 16.0, 0.0);
        // one undo takes the lot back
        assert_eq!(h.undo(&mut d).as_deref(), Some("Place prefab"));
        assert!(d.brushes.is_empty() && d.entities.is_empty() && d.groups.is_empty());
        assert_eq!(d.world.wad_names(), vec!["cstrike.wad"]);
    }

    #[test]
    fn starter_prefabs_are_valid() {
        let all = builtins();
        assert_eq!(all.len(), 8);
        for (id, m, f) in &all {
            assert!(valid_id(id), "{id}");
            assert!(!m.name.is_empty() && !m.tags.is_empty() && !m.description.is_empty(), "{id}");
            assert!(f.brushes.values().all(is_valid), "{id}");
            let b = bounds(f);
            assert_eq!(b.min.z.min(0.0), b.min.z, "{id}");
            assert_eq!(m.wads, vec!["halflife.wad"], "{id}");
            let (map, json) = to_files(f, m).unwrap();
            let (back, _) = from_files(&map, Some(&json), id).unwrap();
            assert_eq!(back.brushes.len(), f.brushes.len(), "{id}");
            let png = thumbnail(f, 160, 120);
            assert!(check_png(&png).is_ok(), "{id}");
        }
    }

    #[test]
    fn names_and_ids() {
        assert_eq!(slug("Crate stack #2"), "crate_stack_2");
        assert_eq!(slug("!!!"), "prefab");
        assert_eq!(slug(&"a".repeat(80)).len(), 48);
        assert!(valid_id("crate_stack_2") && !valid_id("../x") && !valid_id("A") && !valid_id(""));
        assert!(check_png(b"nope").is_err());
    }
}
