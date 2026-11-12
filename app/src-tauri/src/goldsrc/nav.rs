//! The CS bot navigation mesh (`maps/<map>.nav`, version 5 and the older ones the game still
//! loads): walkable areas with their corner heights, connections per direction, hiding spots,
//! approach and encounter data, and place names. Read in full and written back byte for byte
//! (the nav fixer in phase 2 edits it).
//!
//! Ladders aren't in the file: the game rebuilds them from the map's `func_ladder` brushes
//! every time it loads a nav. [`build_ladders`] does the same with our own hull traces.
//!
//! Directions are the game's: 0 north (-y), 1 east (+x), 2 south (+y), 3 west (-x).

use super::{Error, Le, Result};

pub const NAV_MAGIC: u32 = 0xFEED_FACE;
/// The newest version the game writes (and the one we write).
pub const NAV_VERSION: u32 = 5;
/// Navs bigger than this aren't real (de_dust2's is ~200 KB; the biggest seen ~3 MB).
pub const MAX_NAV: u64 = 32 << 20;

const WHAT: &str = "navigation file";

/// Area attribute bits.
pub const ATTR_CROUCH: u8 = 0x01;
pub const ATTR_JUMP: u8 = 0x02;
pub const ATTR_PRECISE: u8 = 0x04;
pub const ATTR_NO_JUMP: u8 = 0x08;

/// Hiding spot flag bits.
pub const SPOT_IN_COVER: u8 = 0x01;
pub const SPOT_GOOD_SNIPER: u8 = 0x02;
pub const SPOT_IDEAL_SNIPER: u8 = 0x04;

#[derive(Debug, Clone, PartialEq)]
pub struct HidingSpot {
    pub id: u32,
    pub pos: [f32; 3],
    pub flags: u8,
}

/// One way into an area, as the nav generator worked it out (where you came from, how).
#[derive(Debug, Clone, PartialEq)]
pub struct Approach {
    pub here: u32,
    pub prev: u32,
    pub prev_to_here: u8,
    pub next: u32,
    pub here_to_next: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpotOrder {
    pub id: u32,
    /// 0-255: how far along the path the spot comes into view.
    pub t: u8,
}

/// The hiding spots seen along one path through an area.
#[derive(Debug, Clone, PartialEq)]
pub struct Encounter {
    pub from: u32,
    pub from_dir: u8,
    pub to: u32,
    pub to_dir: u8,
    pub spots: Vec<SpotOrder>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    pub id: u32,
    pub attr: u8,
    /// The north-west corner (as saved: the game doesn't sort them).
    pub lo: [f32; 3],
    /// The south-east corner.
    pub hi: [f32; 3],
    /// Heights of the other two corners: north-east (hi.x, lo.y) and south-west (lo.x, hi.y).
    pub ne_z: f32,
    pub sw_z: f32,
    /// Connected area ids, per direction (N, E, S, W).
    pub conns: [Vec<u32>; 4],
    pub hiding: Vec<HidingSpot>,
    pub approach: Vec<Approach>,
    pub encounters: Vec<Encounter>,
    /// 1-based index into [`Nav::places`]; 0 = no place.
    pub place: u16,
}

impl Area {
    /// The centre the game uses (and the brain's nav key): the middle of the two saved corners.
    pub fn center(&self) -> [f32; 3] {
        [(self.lo[0] + self.hi[0]) / 2.0, (self.lo[1] + self.hi[1]) / 2.0, (self.lo[2] + self.hi[2]) / 2.0]
    }

    /// The floor height at (x, y), clamped to the area (CNavArea::GetZ).
    pub fn z_at(&self, x: f32, y: f32) -> f32 {
        let dx = self.hi[0] - self.lo[0];
        let dy = self.hi[1] - self.lo[1];
        if dx == 0.0 || dy == 0.0 {
            return self.ne_z;
        }
        let u = ((x - self.lo[0]) / dx).clamp(0.0, 1.0);
        let v = ((y - self.lo[1]) / dy).clamp(0.0, 1.0);
        let north = self.lo[2] + u * (self.ne_z - self.lo[2]);
        let south = self.sw_z + u * (self.hi[2] - self.sw_z);
        north + v * (south - north)
    }

    /// Whether (x, y) is inside the area's 2D extent, edges included.
    pub fn contains_xy(&self, x: f32, y: f32) -> bool {
        x >= self.lo[0] && x <= self.hi[0] && y >= self.lo[1] && y <= self.hi[1]
    }

    /// The point on the area closest to `p` (CNavArea::GetClosestPointOnArea).
    pub fn closest_point(&self, p: [f32; 3]) -> [f32; 3] {
        let x = p[0].clamp(self.lo[0].min(self.hi[0]), self.hi[0].max(self.lo[0]));
        let y = p[1].clamp(self.lo[1].min(self.hi[1]), self.hi[1].max(self.lo[1]));
        [x, y, self.z_at(x, y)]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Nav {
    pub version: u32,
    /// The size of the .bsp the nav was made for (version 4+; 0 before).
    pub bsp_size: u32,
    /// Place names (version 5), in file order: an area's `place` is 1 + an index here.
    pub places: Vec<String>,
    pub areas: Vec<Area>,
}

impl Nav {
    /// Reads a whole .nav. Never panics: a damaged file gives a readable Error.
    pub fn parse(b: &[u8]) -> Result<Nav> {
        let r = Le::new(b, WHAT);
        let mut p = Cursor { r, at: 0 };
        if b.len() < 8 || p.r.u32(0)? != NAV_MAGIC {
            return Err(Error::NotFormat { what: WHAT, detail: "it doesn't start like a bot nav file".into() });
        }
        p.at = 4;
        let version = p.u32()?;
        if version == 0 || version > NAV_VERSION {
            return Err(Error::NotFormat { what: WHAT, detail: format!("version {version}; the game reads 1 to {NAV_VERSION}") });
        }
        let bsp_size = if version >= 4 { p.u32()? } else { 0 };
        let mut places = Vec::new();
        if version >= 5 {
            let n = p.u16()?;
            for _ in 0..n {
                let len = p.u16()? as usize;
                if len == 0 || len > 256 {
                    return Err(Error::damaged(WHAT, format!("a place name {len} bytes long")));
                }
                let raw = p.bytes(len)?;
                places.push(raw[..len - 1].iter().take_while(|&&c| c != 0).map(|&c| c as char).collect());
            }
        }
        let count = p.u32()? as usize;
        // (the smallest area is 47 bytes: don't reserve for a count the file can't hold)
        if count > b.len() / 40 + 1 {
            return Err(Error::damaged(WHAT, format!("it says {count} areas but is only {} bytes", b.len())));
        }
        let mut areas = Vec::with_capacity(count);
        for i in 0..count {
            areas.push(read_area(&mut p, version).map_err(|e| match e {
                Error::Damaged { what, detail } => Error::Damaged { what, detail: format!("area {} of {count}: {detail}", i + 1) },
                e => e,
            })?);
        }
        Ok(Nav { version, bsp_size, places, areas })
    }

    /// Writes the nav as version 5 (what the game writes). A version-5 file read by `parse`
    /// comes back byte for byte.
    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(64 + self.areas.len() * 120);
        put32(&mut o, NAV_MAGIC);
        put32(&mut o, NAV_VERSION);
        put32(&mut o, self.bsp_size);
        put16(&mut o, self.places.len() as u16);
        for name in &self.places {
            let bytes: Vec<u8> = name.chars().take(255).map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
            put16(&mut o, bytes.len() as u16 + 1);
            o.extend_from_slice(&bytes);
            o.push(0);
        }
        put32(&mut o, self.areas.len() as u32);
        for a in &self.areas {
            put32(&mut o, a.id);
            o.push(a.attr);
            for v in a.lo.iter().chain(a.hi.iter()).chain([a.ne_z, a.sw_z].iter()) {
                o.extend_from_slice(&v.to_le_bytes());
            }
            for d in &a.conns {
                put32(&mut o, d.len() as u32);
                for id in d {
                    put32(&mut o, *id);
                }
            }
            o.push(a.hiding.len().min(255) as u8);
            for s in a.hiding.iter().take(255) {
                put32(&mut o, s.id);
                for v in s.pos {
                    o.extend_from_slice(&v.to_le_bytes());
                }
                o.push(s.flags);
            }
            o.push(a.approach.len().min(16) as u8);
            for ap in a.approach.iter().take(16) {
                put32(&mut o, ap.here);
                put32(&mut o, ap.prev);
                o.push(ap.prev_to_here);
                put32(&mut o, ap.next);
                o.push(ap.here_to_next);
            }
            put32(&mut o, a.encounters.len() as u32);
            for e in &a.encounters {
                put32(&mut o, e.from);
                o.push(e.from_dir);
                put32(&mut o, e.to);
                o.push(e.to_dir);
                o.push(e.spots.len().min(255) as u8);
                for s in e.spots.iter().take(255) {
                    put32(&mut o, s.id);
                    o.push(s.t);
                }
            }
            put16(&mut o, a.place);
        }
        o
    }

    /// An area's place name ("" for none).
    pub fn place_name(&self, a: &Area) -> &str {
        if a.place == 0 {
            return "";
        }
        self.places.get(a.place as usize - 1).map(|s| s.as_str()).unwrap_or("")
    }

    /// File-order index of an area id.
    pub fn index_of(&self) -> std::collections::HashMap<u32, usize> {
        self.areas.iter().enumerate().map(|(i, a)| (a.id, i)).collect()
    }

    /// The area under `pos` (within `beneath` units below it, the highest such), as the game's
    /// CNavAreaGrid::GetNavArea.
    pub fn area_at(&self, pos: [f32; 3], beneath: f32) -> Option<usize> {
        let test_z = pos[2] + 5.0;
        let mut best: Option<(usize, f32)> = None;
        for (i, a) in self.areas.iter().enumerate() {
            if !a.contains_xy(pos[0], pos[1]) {
                continue;
            }
            let z = a.z_at(pos[0], pos[1]);
            if z > test_z || z < pos[2] - beneath {
                continue;
            }
            if best.map(|(_, bz)| z > bz).unwrap_or(true) {
                best = Some((i, z));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Connections that only go one way (a drop you can't climb back up), as (from, to) ids.
    pub fn one_way_links(&self) -> Vec<(u32, u32)> {
        let back: std::collections::HashSet<(u32, u32)> =
            self.areas.iter().flat_map(|a| a.conns.iter().flatten().map(move |&b| (a.id, b))).collect();
        let mut out: Vec<(u32, u32)> = back.iter().filter(|&&(a, b)| !back.contains(&(b, a))).copied().collect();
        out.sort_unstable();
        out
    }

    /// Ids that areas connect to but no area has (a damaged or hand-edited nav).
    pub fn dangling_links(&self) -> Vec<(u32, u32)> {
        let ids = self.index_of();
        let mut out = Vec::new();
        for a in &self.areas {
            for d in &a.conns {
                for id in d {
                    if !ids.contains_key(id) {
                        out.push((a.id, *id));
                    }
                }
            }
        }
        out
    }
}

fn put16(o: &mut Vec<u8>, v: u16) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn put32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}

struct Cursor<'a> {
    r: Le<'a>,
    at: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8> {
        let v = self.r.u8(self.at)?;
        self.at += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16> {
        let v = self.r.u16(self.at)?;
        self.at += 2;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32> {
        let v = self.r.u32(self.at)?;
        self.at += 4;
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32> {
        let v = self.r.f32(self.at)?;
        self.at += 4;
        Ok(v)
    }
    fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let v = self.r.bytes(self.at, n)?;
        self.at += n;
        Ok(v)
    }
    /// A count of items each at least `min` bytes: refused if the rest of the file can't hold them.
    fn count_fits(&self, n: usize, min: usize, what: &str) -> Result<()> {
        if n.saturating_mul(min) > self.r.len().saturating_sub(self.at) {
            return Err(Error::damaged(WHAT, format!("{n} {what} don't fit in what's left of the file")));
        }
        Ok(())
    }
}

fn read_area(p: &mut Cursor, version: u32) -> Result<Area> {
    let id = p.u32()?;
    let attr = p.u8()?;
    let lo = p.vec3()?;
    let hi = p.vec3()?;
    let ne_z = p.f32()?;
    let sw_z = p.f32()?;
    let mut conns: [Vec<u32>; 4] = Default::default();
    for d in conns.iter_mut() {
        let n = p.u32()? as usize;
        p.count_fits(n, 4, "connections")?;
        d.reserve(n);
        for _ in 0..n {
            d.push(p.u32()?);
        }
    }
    let nh = p.u8()?;
    let mut hiding = Vec::with_capacity(nh as usize);
    for _ in 0..nh {
        if version == 1 {
            hiding.push(HidingSpot { id: 0, pos: p.vec3()?, flags: SPOT_IN_COVER });
        } else {
            hiding.push(HidingSpot { id: p.u32()?, pos: p.vec3()?, flags: p.u8()? });
        }
    }
    let na = p.u8()?;
    if na > 16 {
        return Err(Error::damaged(WHAT, format!("{na} approach areas (at most 16)")));
    }
    let mut approach = Vec::with_capacity(na as usize);
    for _ in 0..na {
        approach.push(Approach { here: p.u32()?, prev: p.u32()?, prev_to_here: p.u8()?, next: p.u32()?, here_to_next: p.u8()? });
    }
    let ne = p.u32()? as usize;
    let mut encounters = Vec::new();
    if version < 3 {
        // old encounter paths: read and dropped, as the game does (and no place follows)
        p.count_fits(ne, 33, "encounter paths")?;
        for _ in 0..ne {
            p.bytes(4 + 4 + 24)?;
            let sc = p.u8()? as usize;
            p.bytes(sc * 16)?;
        }
        return Ok(Area { id, attr, lo, hi, ne_z, sw_z, conns, hiding, approach, encounters, place: 0 });
    }
    p.count_fits(ne, 11, "encounter paths")?;
    encounters.reserve(ne);
    for _ in 0..ne {
        let from = p.u32()?;
        let from_dir = p.u8()?;
        let to = p.u32()?;
        let to_dir = p.u8()?;
        let sc = p.u8()?;
        let mut spots = Vec::with_capacity(sc as usize);
        for _ in 0..sc {
            spots.push(SpotOrder { id: p.u32()?, t: p.u8()? });
        }
        encounters.push(Encounter { from, from_dir, to, to_dir, spots });
    }
    let place = if version >= 5 { p.u16()? } else { 0 };
    Ok(Area { id, attr, lo, hi, ne_z, sw_z, conns, hiding, approach, encounters, place })
}

// ---- ladders ------------------------------------------------------------------------------

/// The game's nav generation step (25 units).
const STEP: f32 = 25.0;
const HALF_HUMAN_WIDTH: f32 = 16.0;
const HUMAN_HEIGHT: f32 = 72.0;

/// A `func_ladder`'s box as the engine links it: the brush model's bounds plus the entity's
/// origin, grown by 1 unit each way (absmin/absmax).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LadderBox {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    /// The entity's index in the map's entity list (for the inspector).
    pub entity: usize,
}

/// A line trace through the map's solid parts: `(fraction, start_solid)`. The ladder's own
/// brush never blocks (the game skips it).
pub trait LineTrace {
    fn line(&self, from: [f32; 3], to: [f32; 3]) -> (f32, bool);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ladder {
    pub entity: usize,
    pub top: [f32; 3],
    pub bottom: [f32; 3],
    pub length: f32,
    /// The side a climber is on (0 N, 1 E, 2 S, 3 W): away from the wall the ladder is on.
    pub dir: u8,
    /// File-order area indices (not ids) the ladder joins.
    pub bottom_area: Option<usize>,
    pub top_forward: Option<usize>,
    pub top_left: Option<usize>,
    pub top_right: Option<usize>,
    pub top_behind: Option<usize>,
    /// Too high off the floor to climb on to.
    pub dangling: bool,
}

fn add_dir(v: &mut [f32; 3], dir: u8, amount: f32) {
    match dir {
        0 => v[1] -= amount,
        1 => v[0] += amount,
        2 => v[1] += amount,
        _ => v[0] -= amount,
    }
}
fn opposite(dir: u8) -> u8 {
    (dir + 2) % 4
}
fn left(dir: u8) -> u8 {
    (dir + 3) % 4
}
fn right(dir: u8) -> u8 {
    (dir + 1) % 4
}

fn first_area_in_direction(nav: &Nav, tr: &dyn LineTrace, start: [f32; 3], dir: u8, range: f32, beneath: f32) -> Option<usize> {
    let mut pos = start;
    let end = (range / STEP + 0.5) as i32;
    for _ in 1..=end {
        add_dir(&mut pos, dir, STEP);
        if tr.line(start, pos).0 != 1.0 {
            break;
        }
        if let Some(a) = nav.area_at(pos, beneath) {
            return Some(a);
        }
    }
    None
}

/// The nearest area to `pos` (CNavAreaGrid::GetNearestNavArea with anyZ): the one beneath it,
/// else the closest by distance from the floor height under it.
fn nearest_area(nav: &Nav, tr: &dyn LineTrace, pos: [f32; 3]) -> Option<usize> {
    if let Some(a) = nav.area_at(pos, 120.0) {
        return Some(a);
    }
    // (the game's GetGroundHeight samples several start heights to find layered floors; one
    // trace down from just above is close enough for picking the nearest area)
    let from = [pos[0], pos[1], pos[2] + 1.0];
    let to = [pos[0], pos[1], pos[2] - 9999.9];
    let (f, solid) = tr.line(from, to);
    if solid {
        return None;
    }
    let src = [pos[0], pos[1], from[2] + (to[2] - from[2]) * f + 36.0];
    let mut best: Option<(usize, f32)> = None;
    for (i, a) in nav.areas.iter().enumerate() {
        let c = a.closest_point(src);
        let d = (c[0] - src[0]).powi(2) + (c[1] - src[1]).powi(2) + (c[2] - src[2]).powi(2);
        if d < 100_000_000.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// The ladders the game would build for this nav from the map's `func_ladder` boxes
/// (BuildLadders in the game's nav code: facing from a trace past the rungs, top and bottom
/// pulled in past blockages, then the areas at the bottom and on three sides of the top).
pub fn build_ladders(nav: &Nav, boxes: &[LadderBox], tr: &dyn LineTrace) -> Vec<Ladder> {
    let mut out = Vec::new();
    for bx in boxes {
        let cx = (bx.mins[0] + bx.maxs[0]) / 2.0;
        let cy = (bx.mins[1] + bx.maxs[1]) / 2.0;
        let mut top = [cx, cy, bx.maxs[2]];
        let mut bottom = [cx, cy, bx.mins[2]];
        let xs = bx.maxs[0] - bx.mins[0];
        let ys = bx.maxs[1] - bx.mins[1];
        let dir = if xs > ys {
            let from = [bottom[0], bottom[1] + STEP, bottom[2] + STEP];
            let to = [top[0], top[1] + STEP, top[2] - STEP];
            let (f, s) = tr.line(from, to);
            if f != 1.0 || s {
                0
            } else {
                2
            }
        } else {
            let from = [bottom[0] + STEP, bottom[1], bottom[2] + STEP];
            let to = [top[0] + STEP, top[1], top[2] - STEP];
            let (f, s) = tr.line(from, to);
            if f != 1.0 || s {
                3
            } else {
                1
            }
        };
        let along = [top[0] - bottom[0], top[1] - bottom[1], top[2] - bottom[2]];
        let length = (along[0] * along[0] + along[1] * along[1] + along[2] * along[2]).sqrt();
        let unit = if length > 0.0 { [along[0] / length, along[1] / length, along[2] / length] } else { [0.0, 0.0, 1.0] };
        let clear = |on: [f32; 3]| {
            let mut o = on;
            add_dir(&mut o, dir, 32.0);
            let (f, s) = tr.line(on, o);
            f == 1.0 && !s
        };
        let (b0, t0) = (bottom, top);
        let mut t = 0.0;
        while t <= length {
            let on = [b0[0] + t * unit[0], b0[1] + t * unit[1], b0[2] + t * unit[2]];
            if clear(on) {
                bottom = on;
                break;
            }
            t += 10.0;
        }
        t = 0.0;
        while t <= length {
            let on = [t0[0] - t * unit[0], t0[1] - t * unit[1], t0[2] - t * unit[2]];
            if clear(on) {
                top = on;
                break;
            }
            t += 10.0;
        }
        let length = ((top[0] - bottom[0]).powi(2) + (top[1] - bottom[1]).powi(2) + (top[2] - bottom[2]).powi(2)).sqrt();

        let mut c = [bottom[0], bottom[1], bottom[2] + STEP];
        add_dir(&mut c, dir, HALF_HUMAN_WIDTH);
        let bottom_area = nearest_area(nav, tr, c);

        let mut c = [top[0], top[1], top[2] + STEP];
        add_dir(&mut c, dir, HALF_HUMAN_WIDTH);
        let near = 75.0;
        let not_bottom = |a: Option<usize>| a.filter(|&a| Some(a) != bottom_area);
        let top_forward = not_bottom(first_area_in_direction(nav, tr, c, opposite(dir), near, 120.0));
        let top_left = not_bottom(first_area_in_direction(nav, tr, c, left(dir), near, 120.0));
        let top_right = not_bottom(first_area_in_direction(nav, tr, c, right(dir), near, 120.0));
        let top_behind = not_bottom(first_area_in_direction(nav, tr, c, dir, 2.0 * near, 120.0));

        let mut top_z: Option<f32> = None;
        for a in [top_forward, top_left, top_right, top_behind].into_iter().flatten() {
            let z = nav.areas[a].closest_point(top)[2];
            if top_z.map(|tz| z > tz).unwrap_or(true) {
                top_z = Some(z);
            }
        }
        if let Some(z) = top_z {
            top[2] = z;
        }
        let dangling = bottom_area.map(|a| bottom[2] - nav.areas[a].closest_point(bottom)[2] > HUMAN_HEIGHT).unwrap_or(false);
        out.push(Ladder { entity: bx.entity, top, bottom, length, dir, bottom_area, top_forward, top_left, top_right, top_behind, dangling });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(id: u32, lo: [f32; 3], hi: [f32; 3]) -> Area {
        Area { id, attr: 0, lo, hi, ne_z: lo[2], sw_z: hi[2], conns: Default::default(), hiding: vec![], approach: vec![], encounters: vec![], place: 0 }
    }

    /// Three areas with everything the format has, written and read back.
    pub(crate) fn sample() -> Nav {
        let mut a = area(1, [0.0, 0.0, 0.0], [100.0, 100.0, 0.0]);
        a.conns[1] = vec![2];
        a.attr = ATTR_CROUCH | ATTR_JUMP;
        a.hiding = vec![HidingSpot { id: 7, pos: [10.0, 20.0, 0.0], flags: SPOT_IN_COVER | SPOT_GOOD_SNIPER }];
        a.approach = vec![Approach { here: 1, prev: 3, prev_to_here: 2, next: 2, here_to_next: 0 }];
        a.encounters = vec![Encounter { from: 3, from_dir: 0, to: 2, to_dir: 1, spots: vec![SpotOrder { id: 7, t: 128 }] }];
        a.place = 1;
        let mut b = area(2, [100.0, 0.0, 0.0], [200.0, 100.0, 16.0]);
        b.conns[3] = vec![1];
        b.conns[2] = vec![3];
        b.ne_z = 4.0;
        b.sw_z = 12.0;
        b.place = 2;
        let mut c = area(3, [100.0, 100.0, -64.0], [200.0, 180.0, -64.0]);
        c.conns[0] = vec![2, 1];
        Nav { version: 5, bsp_size: 123_456, places: vec!["BombsiteA".into(), "Long".into()], areas: vec![a, b, c] }
    }

    #[test]
    fn round_trip() {
        let n = sample();
        let bytes = n.write();
        let back = Nav::parse(&bytes).unwrap();
        assert_eq!(back, n);
        assert_eq!(back.write(), bytes);
        assert_eq!(back.place_name(&back.areas[1]), "Long");
        assert_eq!(back.place_name(&back.areas[2]), "");
    }

    #[test]
    fn cut_off_files_are_errors() {
        let bytes = sample().write();
        for cut in [0, 3, 7, 11, 20, 40, bytes.len() / 2, bytes.len() - 1] {
            assert!(Nav::parse(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let mut bad = bytes.clone();
        bad[4] = 9;
        assert!(Nav::parse(&bad).unwrap_err().to_string().contains("version 9"));
        // a huge connection count doesn't allocate or panic
        let mut huge = bytes.clone();
        let at = 4 + 4 + 4 + 2 + (2 + 10) + (2 + 5) + 4 + 4 + 1 + 32;
        huge[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Nav::parse(&huge).is_err());
    }

    #[test]
    fn heights_and_lookup() {
        let n = sample();
        let b = &n.areas[1];
        assert_eq!(b.z_at(100.0, 0.0), 0.0);
        assert_eq!(b.z_at(200.0, 0.0), 4.0);
        assert_eq!(b.z_at(100.0, 100.0), 12.0);
        assert_eq!(b.z_at(200.0, 100.0), 16.0);
        assert_eq!(n.area_at([50.0, 50.0, 40.0], 120.0), Some(0));
        assert_eq!(n.area_at([150.0, 150.0, 0.0], 120.0), Some(2));
        assert_eq!(n.area_at([150.0, 150.0, -100.0], 120.0), None);
        assert_eq!(n.dangling_links(), vec![]);
    }

    struct Open;
    impl LineTrace for Open {
        fn line(&self, _: [f32; 3], _: [f32; 3]) -> (f32, bool) {
            (1.0, false)
        }
    }
    /// A wall at y >= 210, the ladder on its north face.
    struct WallSouth;
    impl LineTrace for WallSouth {
        fn line(&self, from: [f32; 3], to: [f32; 3]) -> (f32, bool) {
            if from[1] >= 210.0 {
                return (0.0, true);
            }
            if to[1] >= 210.0 {
                return ((210.0 - from[1]) / (to[1] - from[1]), false);
            }
            (1.0, false)
        }
    }

    #[test]
    fn ladders() {
        // a floor at z 0 north of the wall, and one on top of it at z 128
        let mut lower = area(10, [0.0, 100.0, 0.0], [200.0, 210.0, 0.0]);
        lower.conns = Default::default();
        let upper = area(11, [0.0, 210.0, 128.0], [200.0, 400.0, 128.0]);
        let nav = Nav { version: 5, bsp_size: 0, places: vec![], areas: vec![lower, upper] };
        let bx = LadderBox { mins: [80.0, 202.0, -1.0], maxs: [120.0, 210.0, 129.0], entity: 4 };
        let l = &build_ladders(&nav, &[bx], &WallSouth)[0];
        assert_eq!(l.dir, 0, "{l:?}");
        assert_eq!(l.bottom_area, Some(0));
        assert_eq!(l.entity, 4);
        // in the open, no wall: the other way, and still finds the floor beneath
        let l = &build_ladders(&nav, &[bx], &Open)[0];
        assert_eq!(l.dir, 2);
        assert_eq!(l.bottom_area, Some(0));
    }

    /// LS_TEST_SIM=<loner-strike-sim folder>: the fixed cs_rats2 nav reads and writes back exactly.
    #[test]
    fn real_navs() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let sim = std::path::PathBuf::from(sim);
        for rel in ["navpack/fixes/cs_rats2.nav", "navpack/out/de_dust2.nav"] {
            let b = std::fs::read(sim.join(rel)).unwrap();
            let n = Nav::parse(&b).unwrap();
            assert_eq!(n.write(), b, "{rel}");
            assert!(n.dangling_links().is_empty(), "{rel}");
        }
        let n = Nav::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.nav")).unwrap()).unwrap();
        assert_eq!(n.areas.len(), 586);
        assert!(!n.one_way_links().is_empty());
    }
}
