//! What the deeper checks share: every brush's planes, polygons, bounds, contents and owner,
//! worked out once per run, plus a coarse grid to find the brushes near a point or box quickly
//! (br_valley has ~3000 brushes; each check only ever looks at neighbors).

use crate::editor::doc::{Brush, BrushId, Entity, EntityId, GroupId, MapDocument};
use crate::editor::geom::{brush_polygons, clip_polygon_back, polygon_area, Polygon};
use crate::editor::math::{v3, Bounds, Plane, Vec3};
use std::collections::{HashMap, HashSet};

/// What a brush is made of, from its textures (as the compilers decide it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    Solid,
    Sky,
    /// Blocks players, not light or sight.
    Clip,
    Water,
    /// The rotation center of a brush entity; the compiler removes it.
    Origin,
    /// HINT/SKIP only: cuts the BSP, isn't solid.
    Hint,
}

/// Textures that are never drawn (compiler tools, triggers).
pub fn never_drawn(tex: &str) -> bool {
    let t = tex.to_ascii_lowercase();
    super::TOOL_TEXTURES.contains(&t.as_str()) || matches!(t.as_str(), "clip" | "aaatrigger" | "origin" | "sky")
}

pub fn is_sky(tex: &str) -> bool {
    tex.eq_ignore_ascii_case("sky")
}

/// Textures the compiler turns into lights (lights.rad's usual names).
pub fn texlight(tex: &str) -> bool {
    let t = tex.to_ascii_lowercase();
    // "~light", "+0~light", "-0~light": the ~ marks a light texture in Valve's naming
    t.contains("light") || t.contains("lght") || t.starts_with('~') || ((t.starts_with('+') || t.starts_with('-')) && t.get(2..3) == Some("~"))
}

pub fn contents(b: &Brush) -> Contents {
    let mut out = Contents::Solid;
    let mut skies = 0;
    let mut hints = 0;
    for f in &b.faces {
        let t = f.texture.to_ascii_lowercase();
        if t.starts_with('!') || t.starts_with("water") {
            return Contents::Water;
        }
        match t.as_str() {
            "origin" => return Contents::Origin,
            "clip" => out = Contents::Clip,
            "sky" => skies += 1,
            "hint" | "skip" | "solidhint" => hints += 1,
            _ => {}
        }
    }
    if out == Contents::Clip {
        return out;
    }
    if hints == b.faces.len() {
        return Contents::Hint;
    }
    if skies == b.faces.len() {
        return Contents::Sky;
    }
    out
}

/// Brush entities the compiler folds into the world.
pub fn merged_into_world(class: &str) -> bool {
    matches!(class, "func_detail" | "func_group")
}

/// Brush entities players collide with.
pub fn solid_class(class: &str) -> bool {
    matches!(
        class,
        "func_wall"
            | "func_wall_toggle"
            | "func_breakable"
            | "func_pushable"
            | "func_door"
            | "func_door_rotating"
            | "func_plat"
            | "func_platrot"
            | "func_train"
            | "func_tracktrain"
            | "func_vehicle"
            | "func_rotating"
            | "func_pendulum"
            | "func_button"
            | "func_rot_button"
            | "momentary_rot_button"
            | "momentary_door"
            | "func_guntarget"
            | "func_conveyor"
            | "func_tank"
            | "func_tankmortar"
            | "func_tanklaser"
            | "func_tankrocket"
    ) || merged_into_world(class)
}

/// Brush entities that move (their hidden faces show when they do).
pub fn moving_class(class: &str) -> bool {
    // (a func_breakable goes away and shows what's behind it; func_pushable moves)
    solid_class(class) && !matches!(class, "func_wall" | "func_detail" | "func_group" | "func_conveyor")
}

/// Brush entities nobody sees (trigger volumes, zones, ladders).
pub fn invisible_class(class: &str, e: Option<&Entity>) -> bool {
    if class.starts_with("trigger_")
        || matches!(
            class,
            "func_buyzone" | "func_bomb_target" | "func_hostage_rescue" | "func_vip_safetyzone" | "func_escapezone" | "func_ladder" | "func_mortar_field" | "func_monsterclip" | "func_traincontrols" | "env_bubbles"
        )
    {
        return true;
    }
    // drawn see-through at 0 (a common way to hide a brush entity)
    e.is_some_and(|e| e.get("rendermode").is_some_and(|m| m.trim() != "0" && !m.trim().is_empty()) && e.get("renderamt").is_none_or(|a| a.trim().parse::<f64>().unwrap_or(0.0) <= 0.0))
}

pub struct SBrush<'a> {
    pub id: BrushId,
    pub b: &'a Brush,
    /// One per face (None for a face whose points are in a line).
    pub planes: Vec<Option<Plane>>,
    /// One per face (empty when the face doesn't touch the solid).
    pub polys: Vec<Polygon>,
    pub bounds: Bounds,
    pub entity: Option<EntityId>,
    pub class: &'a str,
    /// World brush or one the compiler folds into the world (func_detail, func_group).
    pub world: bool,
    pub contents: Contents,
    /// In a hidden visgroup.
    pub hidden: bool,
    /// Drawn at all (not a trigger, zone, tool brush...).
    pub drawn: bool,
}

impl SBrush<'_> {
    /// The brush entity it belongs to for z-fighting purposes (None = the world).
    pub fn owner(&self) -> Option<EntityId> {
        if self.world {
            None
        } else {
            self.entity
        }
    }
    /// Blocks a player standing there.
    pub fn blocks_players(&self) -> bool {
        matches!(self.contents, Contents::Solid | Contents::Sky | Contents::Clip) && (self.entity.is_none() || solid_class(self.class))
    }
    /// Blocks light and sight (an opaque solid; see `Scan::is_opaque` for see-through walls).
    pub fn opaque(&self) -> bool {
        matches!(self.contents, Contents::Solid | Contents::Sky) && (self.entity.is_none() || self.world || self.class == "func_wall")
    }
    /// Seals the map from the void (the compiler's view: world brushes only).
    pub fn seals(&self) -> bool {
        matches!(self.contents, Contents::Solid | Contents::Sky) && (self.entity.is_none() || self.class == "func_group")
    }
    /// A face that's drawn in the game.
    pub fn face_drawn(&self, i: usize) -> bool {
        self.drawn && !self.polys[i].is_empty() && !never_drawn(&self.b.faces[i].texture)
    }
    pub fn vertices(&self) -> impl Iterator<Item = &Vec3> {
        self.polys.iter().flatten()
    }
}

pub struct Scan<'a> {
    pub doc: &'a MapDocument,
    pub brushes: Vec<SBrush<'a>>,
    pub index: HashMap<BrushId, usize>,
    /// Brushes of see-through entities (rendermode set): not opaque.
    pub see_through: HashSet<usize>,
    cell: f64,
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
    big: Vec<u32>,
    /// Every brush together.
    pub bounds: Bounds,
}

fn hidden_groups(doc: &MapDocument) -> HashSet<GroupId> {
    let mut out = HashSet::new();
    for (id, _) in &doc.groups {
        let mut g = Some(*id);
        let mut steps = 0;
        while let Some(gid) = g {
            let Some(grp) = doc.groups.get(&gid) else { break };
            if grp.hidden {
                out.insert(*id);
                break;
            }
            g = grp.parent;
            steps += 1;
            if steps > 64 {
                break;
            }
        }
    }
    out
}

impl<'a> Scan<'a> {
    pub fn new(doc: &'a MapDocument) -> Scan<'a> {
        let hidden = hidden_groups(doc);
        let mut brushes = Vec::with_capacity(doc.brushes.len());
        let mut see_through = HashSet::new();
        for (id, b) in &doc.brushes {
            let polys = brush_polygons(b);
            if polys.iter().filter(|p| !p.is_empty()).count() < 4 {
                continue; // broken: the basic checks report it
            }
            let mut bounds = Bounds::empty();
            for p in polys.iter().flatten() {
                bounds.add(*p);
            }
            let ent = b.entity.and_then(|e| doc.entities.get(&e));
            let class = ent.map(|e| e.classname()).unwrap_or("");
            let world = b.entity.is_none() || merged_into_world(class);
            let contents = contents(b);
            let in_hidden = b.group.is_some_and(|g| hidden.contains(&g)) || ent.and_then(|e| e.group).is_some_and(|g| hidden.contains(&g));
            let drawn = !matches!(contents, Contents::Clip | Contents::Origin | Contents::Hint) && !(b.entity.is_some() && invisible_class(class, ent));
            if ent.is_some_and(|e| e.get("rendermode").is_some_and(|m| !m.trim().is_empty() && m.trim() != "0")) && class == "func_wall" {
                see_through.insert(brushes.len());
            }
            brushes.push(SBrush {
                id: *id,
                b,
                planes: b.faces.iter().map(|f| f.plane()).collect(),
                polys,
                bounds,
                entity: b.entity,
                class,
                world,
                contents,
                hidden: in_hidden,
                drawn,
            });
        }
        let mut all = Bounds::empty();
        for b in &brushes {
            all = all.union(&b.bounds);
        }
        let cell = 256.0;
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        let mut big = Vec::new();
        for (i, b) in brushes.iter().enumerate() {
            let (lo, hi) = (cell_of(b.bounds.min, cell), cell_of(b.bounds.max, cell));
            let n = (hi.0 - lo.0 + 1) as i64 * (hi.1 - lo.1 + 1) as i64 * (hi.2 - lo.2 + 1) as i64;
            if n > 512 {
                big.push(i as u32);
                continue;
            }
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        grid.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        let index = brushes.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        Scan { doc, brushes, index, see_through, cell, grid, big, bounds: all }
    }

    /// Brushes whose bounds come within `pad` of the box (indexes into `brushes`, sorted).
    pub fn near(&self, min: Vec3, max: Vec3, pad: f64) -> Vec<usize> {
        let (min, max) = (min - v3(pad, pad, pad), max + v3(pad, pad, pad));
        let (lo, hi) = (cell_of(min, self.cell), cell_of(max, self.cell));
        let mut out: Vec<usize> = Vec::new();
        let n = (hi.0 - lo.0 + 1) as i64 * (hi.1 - lo.1 + 1) as i64 * (hi.2 - lo.2 + 1) as i64;
        let mut push = |i: u32| {
            let b = &self.brushes[i as usize].bounds;
            if b.min.x <= max.x && b.max.x >= min.x && b.min.y <= max.y && b.max.y >= min.y && b.min.z <= max.z && b.max.z >= min.z {
                out.push(i as usize);
            }
        };
        if n > 4096 {
            for i in 0..self.brushes.len() {
                push(i as u32);
            }
            return out;
        }
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    if let Some(v) = self.grid.get(&(x, y, z)) {
                        v.iter().for_each(|i| push(*i));
                    }
                }
            }
        }
        self.big.iter().for_each(|i| push(*i));
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn is_opaque(&self, i: usize) -> bool {
        self.brushes[i].opaque() && !self.see_through.contains(&i)
    }

    /// Does the box overlap something that blocks players, deeper than `tol`?
    pub fn box_blocked(&self, min: Vec3, max: Vec3, tol: f64, ignore: Option<EntityId>) -> Option<usize> {
        self.near(min, max, 0.0).into_iter().find(|i| {
            let b = &self.brushes[*i];
            b.blocks_players() && (ignore.is_none() || b.entity != ignore) && box_hits_brush(min, max, b, tol)
        })
    }

    /// Is the point inside an opaque solid, deeper than `tol`?
    pub fn point_in_opaque(&self, p: Vec3, tol: f64) -> Option<usize> {
        self.near(p, p, 0.0).into_iter().find(|i| self.is_opaque(*i) && point_in_brush(p, &self.brushes[*i], tol))
    }
}

fn cell_of(p: Vec3, cell: f64) -> (i32, i32, i32) {
    let c = |x: f64| (x / cell).floor().clamp(-1e6, 1e6) as i32;
    (c(p.x), c(p.y), c(p.z))
}

pub fn point_in_brush(p: Vec3, b: &SBrush, tol: f64) -> bool {
    b.planes.iter().flatten().all(|pl| pl.distance(p) < -tol)
}

/// Does the box overlap the brush by more than `tol` (separating axis test: the box's axes,
/// the brush's planes and the cross products of their edges)?
pub fn box_hits_brush(min: Vec3, max: Vec3, b: &SBrush, tol: f64) -> bool {
    let bb = &b.bounds;
    if bb.max.x <= min.x + tol || bb.min.x >= max.x - tol || bb.max.y <= min.y + tol || bb.min.y >= max.y - tol || bb.max.z <= min.z + tol || bb.min.z >= max.z - tol {
        return false;
    }
    for pl in b.planes.iter().flatten() {
        let n = pl.normal;
        let sup = v3(if n.x > 0.0 { min.x } else { max.x }, if n.y > 0.0 { min.y } else { max.y }, if n.z > 0.0 { min.z } else { max.z });
        if pl.distance(sup) >= -tol {
            return false;
        }
    }
    // edge x axis directions (only matters for slanted brushes)
    let corners = [
        v3(min.x, min.y, min.z),
        v3(max.x, min.y, min.z),
        v3(min.x, max.y, min.z),
        v3(max.x, max.y, min.z),
        v3(min.x, min.y, max.z),
        v3(max.x, min.y, max.z),
        v3(min.x, max.y, max.z),
        v3(max.x, max.y, max.z),
    ];
    for poly in &b.polys {
        for k in 0..poly.len() {
            let e = poly[(k + 1) % poly.len()] - poly[k];
            for a in [Vec3::X, Vec3::Y, Vec3::Z] {
                let ax = e.cross(a);
                if ax.len() < 1e-6 {
                    continue;
                }
                let ax = ax.normalized();
                let (mut bl, mut bh) = (f64::MAX, f64::MIN);
                for v in b.vertices() {
                    let d = v.dot(ax);
                    bl = bl.min(d);
                    bh = bh.max(d);
                }
                let (mut cl, mut ch) = (f64::MAX, f64::MIN);
                for c in &corners {
                    let d = c.dot(ax);
                    cl = cl.min(d);
                    ch = ch.max(d);
                }
                if bh <= cl + tol || ch <= bl + tol {
                    return false;
                }
            }
        }
    }
    true
}

/// The part of convex polygon `a` inside convex polygon `b` (seen along `b`'s normal `nb`):
/// `a` clipped by the planes through `b`'s edges.
pub fn overlap(a: &[Vec3], b: &[Vec3], nb: Vec3) -> Polygon {
    let mut out: Polygon = a.to_vec();
    for k in 0..b.len() {
        if out.len() < 3 {
            return Vec::new();
        }
        let d = b[(k + 1) % b.len()] - b[k];
        let outward = d.cross(nb).normalized();
        if outward == Vec3::ZERO {
            continue;
        }
        out = clip_polygon_back(&out, &Plane::from_point_normal(b[k], outward));
    }
    out
}

pub fn overlap_area(a: &[Vec3], b: &[Vec3], nb: Vec3) -> (f64, Polygon) {
    let o = overlap(a, b, nb);
    (polygon_area(&o), o)
}

/// Pairs of brushes whose bounds come within `pad` of each other (i < j).
pub fn pairs(scan: &Scan, pad: f64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (i, b) in scan.brushes.iter().enumerate() {
        for j in scan.near(b.bounds.min, b.bounds.max, pad) {
            if j > i {
                out.push((i, j));
            }
        }
    }
    out
}

/// A world-space position as the JSON-friendly array the UI takes.
pub fn at(p: Vec3) -> [f64; 3] {
    let r = |x: f64| (x * 10.0).round() / 10.0;
    [r(p.x), r(p.y), r(p.z)]
}
