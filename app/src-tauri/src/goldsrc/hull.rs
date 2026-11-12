//! Collision traces through a map's BSP trees, as the engine does them
//! (SV_RecursiveHullCheck): hull 0 (a point, from the drawing nodes and leaves) and hull 1
//! (a standing player, 32x32x72, from the clip nodes). Solid brush entities (walls, doors,
//! breakables...) are traced too, each with its own tree moved by its origin.
//!
//! Used for the `probe` route (the floor under a point), the ladders the nav builds from
//! `func_ladder` brushes, and later the nav fixer and the surf lab. A port of
//! loner-strike-sim/surfsim.py's Hull.

use super::nav::{LadderBox, LineTrace};
use super::{Error, Le, Result};
use std::collections::HashMap;

const WHAT: &str = "map file";
const BSP_VERSION: u32 = 30;
const DIST_EPSILON: f32 = 0.031_25;

pub const CONTENTS_EMPTY: i32 = -1;
pub const CONTENTS_SOLID: i32 = -2;

/// Half the standing player's height: hull 1's origin is this far above the feet.
pub const PLAYER_HALF_HEIGHT: f32 = 36.0;

/// Brush entities that block players and bullets when they're where the map put them.
const SOLID_CLASSES: &[&str] = &[
    "func_wall",
    "func_door",
    "func_door_rotating",
    "func_breakable",
    "func_pushable",
    "func_rotating",
    "func_train",
    "func_plat",
    "func_wall_toggle",
    "func_conveyor",
    "func_tracktrain",
    "func_guntarget",
];

/// A word for a contents value.
pub fn contents_name(c: i32) -> &'static str {
    match c {
        -1 => "empty",
        -2 => "solid",
        -3 => "water",
        -4 => "slime",
        -5 => "lava",
        -6 => "sky",
        -7 => "origin",
        -8 => "clip",
        -14..=-9 => "current",
        -15 => "translucent",
        -16 => "ladder",
        _ => "other",
    }
}

#[derive(Debug, Clone, Copy)]
struct Plane {
    normal: [f32; 3],
    dist: f32,
    /// 0-2: axial (x, y, z), so the distance is one coordinate.
    kind: u32,
}

impl Plane {
    fn dist_to(&self, p: [f32; 3]) -> f32 {
        if self.kind < 3 {
            p[self.kind as usize] - self.dist
        } else {
            self.normal[0] * p[0] + self.normal[1] * p[1] + self.normal[2] * p[2] - self.dist
        }
    }
}

/// A tree node: a plane and two children, each a node index (>= 0) or a contents value (< 0).
#[derive(Debug, Clone, Copy)]
struct Node {
    plane: u32,
    children: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub end: [f32; 3],
    /// The surface hit (zero if nothing was).
    pub normal: [f32; 3],
    pub all_solid: bool,
    pub start_solid: bool,
    pub in_open: bool,
    pub in_water: bool,
}

/// One hull's tree.
#[derive(Debug, Clone)]
struct Tree {
    nodes: Vec<Node>,
}

/// Recursion deeper than this means a looping (damaged) tree: treated as solid.
const MAX_DEPTH: u32 = 1024;

struct Walk<'a> {
    planes: &'a [Plane],
    tree: &'a Tree,
    head: i32,
    t: Trace,
}

impl Tree {
    fn contents(&self, planes: &[Plane], mut num: i32, p: [f32; 3]) -> i32 {
        let mut steps = 0;
        while num >= 0 {
            steps += 1;
            if steps > MAX_DEPTH {
                return CONTENTS_SOLID;
            }
            let n = self.nodes[num as usize];
            let d = planes[n.plane as usize].dist_to(p);
            num = if d >= 0.0 { n.children[0] } else { n.children[1] };
        }
        num
    }
}

impl<'a> Walk<'a> {
    fn rec(&mut self, num: i32, p1f: f32, p2f: f32, p1: [f32; 3], p2: [f32; 3], depth: u32) -> bool {
        if num < 0 || depth > MAX_DEPTH {
            let num = if depth > MAX_DEPTH { CONTENTS_SOLID } else { num };
            if num != CONTENTS_SOLID {
                self.t.all_solid = false;
                if num == CONTENTS_EMPTY {
                    self.t.in_open = true;
                } else {
                    self.t.in_water = true;
                }
            } else {
                self.t.start_solid = true;
            }
            return true;
        }
        let n = self.tree.nodes[num as usize];
        let pl = self.planes[n.plane as usize];
        let t1 = pl.dist_to(p1);
        let t2 = pl.dist_to(p2);
        if t1 >= 0.0 && t2 >= 0.0 {
            return self.rec(n.children[0], p1f, p2f, p1, p2, depth + 1);
        }
        if t1 < 0.0 && t2 < 0.0 {
            return self.rec(n.children[1], p1f, p2f, p1, p2, depth + 1);
        }
        let mut frac = if t1 < 0.0 { (t1 + DIST_EPSILON) / (t1 - t2) } else { (t1 - DIST_EPSILON) / (t1 - t2) };
        frac = frac.clamp(0.0, 1.0);
        let lerp = |f: f32| [p1[0] + f * (p2[0] - p1[0]), p1[1] + f * (p2[1] - p1[1]), p1[2] + f * (p2[2] - p1[2])];
        let mut midf = p1f + (p2f - p1f) * frac;
        let mut mid = lerp(frac);
        let side = if t1 >= 0.0 { 0 } else { 1 };
        if !self.rec(n.children[side], p1f, midf, p1, mid, depth + 1) {
            return false;
        }
        if self.tree.contents(self.planes, n.children[side ^ 1], mid) != CONTENTS_SOLID {
            return self.rec(n.children[side ^ 1], midf, p2f, mid, p2, depth + 1);
        }
        if self.t.all_solid {
            return false;
        }
        self.t.normal = if side == 0 { pl.normal } else { [-pl.normal[0], -pl.normal[1], -pl.normal[2]] };
        while self.tree.contents(self.planes, self.head, mid) == CONTENTS_SOLID {
            frac -= 0.1;
            if frac < 0.0 {
                self.t.fraction = midf;
                self.t.end = mid;
                return false;
            }
            midf = p1f + (p2f - p1f) * frac;
            mid = lerp(frac);
        }
        self.t.fraction = midf;
        self.t.end = mid;
        false
    }
}

/// A solid brush entity: its model's tree heads, where it is, and a box around it (grown by
/// the player hull for hull 1).
#[derive(Debug, Clone)]
struct Solid {
    heads: [i32; 2],
    origin: [f32; 3],
    mins: [f32; 3],
    maxs: [f32; 3],
}

#[derive(Debug, Clone)]
struct Model {
    mins: [f32; 3],
    maxs: [f32; 3],
    heads: [i32; 2],
}

/// The map's hull 0 and hull 1 trees, its solid brush entities and its ladders.
#[derive(Debug, Clone)]
pub struct MapHulls {
    planes: Vec<Plane>,
    trees: [Tree; 2],
    world: [i32; 2],
    solids: Vec<Solid>,
    /// `func_ladder` boxes, for [`super::nav::build_ladders`].
    pub ladders: Vec<LadderBox>,
}

fn lump(r: &Le, i: usize, size: usize, name: &str) -> Result<(usize, usize)> {
    let off = r.u32(4 + i * 8)? as usize;
    let len = r.u32(8 + i * 8)? as usize;
    r.bytes(off, len)?;
    if size > 0 && len % size != 0 {
        return Err(Error::damaged(WHAT, format!("its {name} list is {len} bytes, not a whole number of entries")));
    }
    Ok((off, if size > 0 { len / size } else { len }))
}

/// The entity lump's text as key/value lists (just enough here; entities.rs has the full one).
fn entities(text: &str) -> Vec<HashMap<String, String>> {
    let mut out = Vec::new();
    let mut cur: Option<HashMap<String, String>> = None;
    let mut quoted: Vec<String> = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                cur = Some(HashMap::new());
                quoted.clear();
            }
            '}' => {
                if let Some(e) = cur.take() {
                    out.push(e);
                }
            }
            '"' => {
                let s: String = chars.by_ref().take_while(|&c| c != '"').collect();
                if let Some(e) = cur.as_mut() {
                    quoted.push(s);
                    if quoted.len() == 2 {
                        let v = quoted.pop().unwrap();
                        let k = quoted.pop().unwrap();
                        e.entry(k).or_insert(v);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn vec3_of(s: Option<&String>) -> [f32; 3] {
    let mut v = [0.0; 3];
    if let Some(s) = s {
        for (i, p) in s.split_whitespace().take(3).enumerate() {
            v[i] = p.parse().unwrap_or(0.0);
        }
    }
    v
}

impl MapHulls {
    /// Reads the trees from a whole .bsp file. Every index in them is checked.
    pub fn parse(b: &[u8]) -> Result<MapHulls> {
        let r = Le::new(b, WHAT);
        if b.len() < 124 {
            return Err(Error::damaged(WHAT, "it's too short to be a map"));
        }
        let v = r.u32(0)?;
        if v != BSP_VERSION {
            return Err(Error::NotFormat { what: WHAT, detail: format!("BSP version {v}, not 30") });
        }
        let (po, pn) = lump(&r, 1, 20, "plane")?;
        let mut planes = Vec::with_capacity(pn);
        for i in 0..pn {
            let at = po + i * 20;
            planes.push(Plane { normal: r.vec3(at)?, dist: r.f32(at + 12)?, kind: r.u32(at + 16)? });
        }
        let (lo, ln) = lump(&r, 10, 28, "leaf")?;
        let mut leaf_contents = Vec::with_capacity(ln);
        for i in 0..ln {
            let c = r.i32(lo + i * 28)?;
            leaf_contents.push(if c < 0 { c } else { CONTENTS_SOLID });
        }
        let (no, nn) = lump(&r, 5, 24, "node")?;
        let mut nodes0 = Vec::with_capacity(nn);
        for i in 0..nn {
            let at = no + i * 24;
            let plane = r.u32(at)?;
            let mut children = [0i32; 2];
            for (k, ch) in children.iter_mut().enumerate() {
                let c = r.i16(at + 4 + k * 2)? as i32;
                *ch = if c >= 0 {
                    c
                } else {
                    let leaf = (-1 - c) as usize;
                    *leaf_contents.get(leaf).ok_or_else(|| Error::damaged(WHAT, format!("node {i} points at leaf {leaf} of {ln}")))?
                };
            }
            nodes0.push(Node { plane, children });
        }
        let (co, cn) = lump(&r, 9, 8, "clip node")?;
        let mut nodes1 = Vec::with_capacity(cn);
        for i in 0..cn {
            let at = co + i * 8;
            nodes1.push(Node { plane: r.u32(at)?, children: [r.i16(at + 4)? as i32, r.i16(at + 6)? as i32] });
        }
        for (name, nodes) in [("node", &nodes0), ("clip node", &nodes1)] {
            for (i, n) in nodes.iter().enumerate() {
                if n.plane as usize >= planes.len() || n.children.iter().any(|&c| c >= 0 && c as usize >= nodes.len()) {
                    return Err(Error::damaged(WHAT, format!("{name} {i} points outside the map's lists")));
                }
            }
        }
        let (mo, mn) = lump(&r, 14, 64, "model")?;
        if mn == 0 {
            return Err(Error::damaged(WHAT, "it has no world model"));
        }
        let mut models = Vec::with_capacity(mn);
        for i in 0..mn {
            let at = mo + i * 64;
            let heads = [r.i32(at + 36)?, r.i32(at + 40)?];
            for (h, len) in heads.iter().zip([nodes0.len(), nodes1.len()]) {
                if *h >= 0 && *h as usize >= len {
                    return Err(Error::damaged(WHAT, format!("model {i} starts outside its tree")));
                }
            }
            models.push(Model { mins: r.vec3(at)?, maxs: r.vec3(at + 12)?, heads });
        }
        let (eo, el) = lump(&r, 0, 0, "entity")?;
        let text: String = b[eo..eo + el].iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
        let mut solids = Vec::new();
        let mut ladders = Vec::new();
        for (index, e) in entities(&text).iter().enumerate() {
            let class = e.get("classname").map(|s| s.as_str()).unwrap_or("");
            let Some(mi) = e.get("model").and_then(|m| m.strip_prefix('*')).and_then(|m| m.parse::<usize>().ok()) else { continue };
            let Some(m) = models.get(mi).filter(|_| mi > 0) else { continue };
            let o = vec3_of(e.get("origin"));
            let mins = [m.mins[0] + o[0], m.mins[1] + o[1], m.mins[2] + o[2]];
            let maxs = [m.maxs[0] + o[0], m.maxs[1] + o[1], m.maxs[2] + o[2]];
            if class == "func_ladder" {
                ladders.push(LadderBox {
                    mins: [mins[0] - 1.0, mins[1] - 1.0, mins[2] - 1.0],
                    maxs: [maxs[0] + 1.0, maxs[1] + 1.0, maxs[2] + 1.0],
                    entity: index,
                });
            } else if SOLID_CLASSES.contains(&class) {
                solids.push(Solid { heads: m.heads, origin: o, mins, maxs });
            }
        }
        Ok(MapHulls { planes, trees: [Tree { nodes: nodes0 }, Tree { nodes: nodes1 }], world: models[0].heads, solids, ladders })
    }

    fn trace_one(&self, hull: usize, head: i32, start: [f32; 3], end: [f32; 3]) -> Trace {
        let t = Trace { fraction: 1.0, end, normal: [0.0; 3], all_solid: true, start_solid: false, in_open: false, in_water: false };
        let mut w = Walk { planes: &self.planes, tree: &self.trees[hull], head, t };
        w.rec(head, 0.0, 1.0, start, end, 0);
        let mut t = w.t;
        if t.all_solid {
            t.start_solid = true;
        }
        if t.fraction == 1.0 {
            t.end = end;
        }
        t
    }

    /// A trace from `start` to `end` with hull 0 (a point) or 1 (a standing player's box,
    /// positions are its centre): the world and the solid brush entities, the nearest hit.
    pub fn trace(&self, hull: usize, start: [f32; 3], end: [f32; 3]) -> Trace {
        let hull = hull.min(1);
        let mut best = self.trace_one(hull, self.world[hull], start, end);
        let grow = if hull == 1 { [16.0, 16.0, 36.0] } else { [0.0; 3] };
        for s in &self.solids {
            let out = (0..3).any(|i| start[i].max(end[i]) < s.mins[i] - grow[i] || start[i].min(end[i]) > s.maxs[i] + grow[i]);
            if out || s.heads[hull] < 0 {
                continue;
            }
            let s2 = [start[0] - s.origin[0], start[1] - s.origin[1], start[2] - s.origin[2]];
            let e2 = [end[0] - s.origin[0], end[1] - s.origin[1], end[2] - s.origin[2]];
            let mut t = self.trace_one(hull, s.heads[hull], s2, e2);
            if t.all_solid {
                t.end = start;
                return t;
            }
            if t.fraction < best.fraction {
                t.end = [t.end[0] + s.origin[0], t.end[1] + s.origin[1], t.end[2] + s.origin[2]];
                best = t;
            }
        }
        best
    }

    /// What's at a point (the world's hull 0): CONTENTS_* (-1 empty, -2 solid, -3 water...).
    pub fn contents(&self, p: [f32; 3]) -> i32 {
        self.trees[0].contents(&self.planes, self.world[0], p)
    }

    /// The floor a standing player would land on from `p` (their feet height), tracing hull 1
    /// down from feet height `p.z`. None if nothing's under it within `max_drop` or `p` is
    /// inside a wall.
    pub fn floor_below(&self, p: [f32; 3], max_drop: f32) -> Option<(f32, Trace)> {
        let start = [p[0], p[1], p[2] + PLAYER_HALF_HEIGHT];
        let end = [p[0], p[1], start[2] - max_drop];
        let t = self.trace(1, start, end);
        if t.start_solid || t.fraction >= 1.0 {
            return None;
        }
        Some((t.end[2] - PLAYER_HALF_HEIGHT, t))
    }

    /// The ceiling over a standing player at feet height `p.z` (where their head would stop).
    pub fn ceiling_above(&self, p: [f32; 3], max_rise: f32) -> Option<(f32, Trace)> {
        let start = [p[0], p[1], p[2] + PLAYER_HALF_HEIGHT];
        let end = [p[0], p[1], start[2] + max_rise];
        let t = self.trace(1, start, end);
        if t.start_solid || t.fraction >= 1.0 {
            return None;
        }
        Some((t.end[2] + PLAYER_HALF_HEIGHT, t))
    }
}

impl LineTrace for MapHulls {
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> (f32, bool) {
        let t = self.trace(0, from, to);
        (t.fraction, t.start_solid)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny map: a box room 0..256 on x and y, floor at z 0, ceiling at z 128, as both a
    /// point tree (nodes + leaves) and a clip tree (grown by the player hull), plus a
    /// func_ladder model and a func_wall model (a pillar at x 100..120 in the middle).
    pub(crate) fn room_bsp() -> Vec<u8> {
        // planes: x=0, x=256, y=0, y=256, z=0, z=128 (all normals +axis)
        let planes: Vec<([f32; 3], f32, u32)> = vec![
            ([1.0, 0.0, 0.0], 0.0, 0),
            ([1.0, 0.0, 0.0], 256.0, 0),
            ([0.0, 1.0, 0.0], 0.0, 1),
            ([0.0, 1.0, 0.0], 256.0, 1),
            ([0.0, 0.0, 1.0], 0.0, 2),
            ([0.0, 0.0, 1.0], 128.0, 2),
            // clip hull versions: grown inwards by 16 (x, y) and 36 (z)
            ([1.0, 0.0, 0.0], 16.0, 0),
            ([1.0, 0.0, 0.0], 240.0, 0),
            ([0.0, 1.0, 0.0], 16.0, 1),
            ([0.0, 1.0, 0.0], 240.0, 1),
            ([0.0, 0.0, 1.0], 36.0, 2),
            ([0.0, 0.0, 1.0], 92.0, 2),
            // the pillar (x 100..120) for the wall model
            ([1.0, 0.0, 0.0], 100.0, 0),
            ([1.0, 0.0, 0.0], 120.0, 0),
        ];
        // a chain: front child = keep testing, back child = solid, or the other way round
        // node k: plane k; for the "lo" planes (0,2,4) inside is front; for "hi" (1,3,5) inside is back
        let chain = |base: u32| -> Vec<(u32, i32, i32)> {
            vec![
                (base, 1, -2),
                (base + 1, -2, 2),
                (base + 2, 3, -2),
                (base + 3, -2, 4),
                (base + 4, 5, -2),
                (base + 5, -2, -1),
            ]
        };
        // point tree from nodes: leaf 0 solid, leaf 1 empty; children -1 => leaf 0, -2 => leaf 1
        let to_leaf = |c: i32| -> i16 {
            match c {
                -2 => -1,
                -1 => -2,
                c => c as i16,
            }
        };
        let mut nodes = Vec::new();
        for (p, a, b) in chain(0) {
            nodes.push((p, to_leaf(a), to_leaf(b)));
        }
        // the pillar for hull 0: node 6 (plane 12): x>=100 -> node 7, else empty; node 7 (plane 13): x>=120 empty else solid
        nodes.push((12, 7, -2));
        nodes.push((13, -2, -1));
        let mut clip: Vec<(u32, i16, i16)> = chain(6).into_iter().map(|(p, a, b)| (p, a as i16, b as i16)).collect();
        clip.push((12, 7, -1)); // pillar in hull 1 (not grown: fine for the test)
        clip.push((13, -1, -2));
        let leafs: Vec<i32> = vec![-2, -1];
        let ents = "{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"func_ladder\"\n\"model\" \"*1\"\n}\n{\n\"classname\" \"func_wall\"\n\"model\" \"*2\"\n}\n\0";
        // models: world (heads 0, 0), ladder (no tree), wall (heads 6, 6)
        let models: Vec<([f32; 3], [f32; 3], [i32; 2])> = vec![
            ([0.0, 0.0, 0.0], [256.0, 256.0, 128.0], [0, 0]),
            ([120.0, 250.0, 0.0], [140.0, 256.0, 100.0], [-1, -1]),
            ([100.0, 0.0, 0.0], [120.0, 256.0, 128.0], [6, 6]),
        ];
        let mut lumps: Vec<Vec<u8>> = vec![Vec::new(); 15];
        lumps[0] = ents.as_bytes().to_vec();
        for (n, d, k) in &planes {
            for v in n {
                lumps[1].extend_from_slice(&v.to_le_bytes());
            }
            lumps[1].extend_from_slice(&d.to_le_bytes());
            lumps[1].extend_from_slice(&k.to_le_bytes());
        }
        for (p, a, b) in &nodes {
            lumps[5].extend_from_slice(&p.to_le_bytes());
            lumps[5].extend_from_slice(&a.to_le_bytes());
            lumps[5].extend_from_slice(&b.to_le_bytes());
            lumps[5].extend_from_slice(&[0u8; 16]);
        }
        for (p, a, b) in &clip {
            lumps[9].extend_from_slice(&p.to_le_bytes());
            lumps[9].extend_from_slice(&a.to_le_bytes());
            lumps[9].extend_from_slice(&b.to_le_bytes());
        }
        for c in &leafs {
            lumps[10].extend_from_slice(&c.to_le_bytes());
            lumps[10].extend_from_slice(&[0u8; 24]);
        }
        for (mi, ma, h) in &models {
            for v in mi.iter().chain(ma.iter()).chain([0.0f32; 3].iter()) {
                lumps[14].extend_from_slice(&v.to_le_bytes());
            }
            lumps[14].extend_from_slice(&h[0].to_le_bytes());
            lumps[14].extend_from_slice(&h[1].to_le_bytes());
            lumps[14].extend_from_slice(&[0u8; 8]);
            lumps[14].extend_from_slice(&[0u8; 12]);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&30u32.to_le_bytes());
        let mut at = 4 + 15 * 8;
        let mut dir = Vec::new();
        for l in &lumps {
            dir.extend_from_slice(&(at as u32).to_le_bytes());
            dir.extend_from_slice(&(l.len() as u32).to_le_bytes());
            at += l.len();
        }
        out.extend_from_slice(&dir);
        for l in &lumps {
            out.extend_from_slice(l);
        }
        out
    }

    #[test]
    fn room() {
        let h = MapHulls::parse(&room_bsp()).unwrap();
        assert_eq!(h.contents([50.0, 50.0, 50.0]), CONTENTS_EMPTY);
        assert_eq!(h.contents([50.0, 50.0, -5.0]), CONTENTS_SOLID);
        assert_eq!(h.ladders.len(), 1);
        assert_eq!(h.ladders[0].mins, [119.0, 249.0, -1.0]);
        // the floor under a point, for a player
        let (floor, t) = h.floor_below([50.0, 50.0, 40.0], 4096.0).unwrap();
        assert!((floor - 0.0).abs() < 0.1, "{floor} {t:?}");
        assert_eq!(t.normal, [0.0, 0.0, 1.0]);
        let (ceil, _) = h.ceiling_above([50.0, 50.0, 10.0], 4096.0).unwrap();
        assert!((ceil - 128.0).abs() < 0.1, "{ceil}");
        // a point trace across the room stops at the far wall
        let t = h.trace(0, [10.0, 50.0, 50.0], [90.0, 50.0, 50.0]);
        assert_eq!(t.fraction, 1.0);
        let t = h.trace(0, [200.0, 50.0, 50.0], [300.0, 50.0, 50.0]);
        assert!((t.end[0] - 256.0).abs() < 0.1 && t.fraction < 1.0, "{t:?}");
        // ... and at the pillar (a func_wall)
        let t = h.trace(0, [10.0, 50.0, 50.0], [200.0, 50.0, 50.0]);
        assert!((t.end[0] - 100.0).abs() < 0.1, "{t:?}");
        assert_eq!(t.normal, [-1.0, 0.0, 0.0]);
        // inside the floor
        assert!(h.floor_below([50.0, 50.0, -100.0], 4096.0).is_none());
    }

    #[test]
    fn damaged_maps_are_errors() {
        let b = room_bsp();
        for cut in [0, 10, 100, 130, b.len() / 2, b.len() - 1] {
            assert!(MapHulls::parse(&b[..cut]).is_err(), "cut {cut}");
        }
        let mut bad = b.clone();
        bad[0] = 29;
        assert!(MapHulls::parse(&bad).is_err());
        // a node pointing at itself loops: traces still end
        let mut looped = b.clone();
        let nodes_at = u32::from_le_bytes(looped[4 + 5 * 8..4 + 5 * 8 + 4].try_into().unwrap()) as usize;
        looped[nodes_at + 4..nodes_at + 6].copy_from_slice(&0i16.to_le_bytes());
        let h = MapHulls::parse(&looped).unwrap();
        let _ = h.trace(0, [50.0, 50.0, 50.0], [60.0, 60.0, 60.0]);
        let _ = h.contents([50.0, 50.0, 50.0]);
    }

    /// LS_TEST_GAME=<Half-Life folder> and LS_TEST_SIM=<loner-strike-sim folder>: de_dust2's
    /// floors match its nav, and its ladders build.
    #[test]
    fn real_map() {
        let (Ok(game), Ok(sim)) = (std::env::var("LS_TEST_GAME"), std::env::var("LS_TEST_SIM")) else { return };
        let b = std::fs::read(std::path::Path::new(&game).join("cstrike/maps/de_dust2.bsp")).unwrap();
        let h = MapHulls::parse(&b).unwrap();
        let nav = super::super::nav::Nav::parse(&std::fs::read(std::path::Path::new(&sim).join("navpack/out/de_dust2.nav")).unwrap()).unwrap();
        // most areas' centres have a floor within a few units of the nav's height
        let mut close = 0;
        for a in &nav.areas {
            let c = a.center();
            if let Some((z, _)) = h.floor_below([c[0], c[1], c[2] + 20.0], 512.0) {
                if (z - a.z_at(c[0], c[1])).abs() < 20.0 {
                    close += 1;
                }
            }
        }
        assert!(close * 10 > nav.areas.len() * 8, "{close} of {}", nav.areas.len());
        assert!(h.ladders.is_empty());
        // de_nuke has ladders: each finds the floor at its foot and somewhere to step off at the top
        let b = std::fs::read(std::path::Path::new(&game).join("cstrike/maps/de_nuke.bsp")).unwrap();
        let h = MapHulls::parse(&b).unwrap();
        let nav = super::super::nav::Nav::parse(&std::fs::read(std::path::Path::new(&sim).join("navpack/out/de_nuke.nav")).unwrap()).unwrap();
        let ladders = super::super::nav::build_ladders(&nav, &h.ladders, &h);
        assert!(!ladders.is_empty());
        // (the two vent ladders end under breakable grates the game's traces hit too)
        assert!(ladders.iter().all(|l| l.bottom_area.is_some() && l.length > 100.0), "{ladders:?}");
        let joined = ladders.iter().filter(|l| l.top_forward.is_some() || l.top_left.is_some() || l.top_right.is_some()).count();
        assert!(joined >= 2, "{ladders:?}");
    }
}
