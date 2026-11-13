//! Hammer/Worldcraft `.rmf` and J.A.C.K. `.jmf` import (agent P5). Binary formats, read with
//! bounds checks (never panics, readable errors). Layouts follow the MIT-licensed
//! sledge-formats readers (WorldcraftRmfFormat.cs, JackhamerJmfFormat.cs) and the TWHL specs.
//!
//! What comes over: brushes (planes from each face's vertices, Valve 220 texture axes, or
//! Quake-style for RMF before 2.2), entities with their keys and spawnflags, point entity
//! origins, brush entities with their brushes, groups (nested), visgroups as visibility sets,
//! RMF/JMF paths as path_corner/path_track entities linked by target.
//! What doesn't (said in the notes): cameras, background images, the cordon box, JMF patch
//! meshes (Jackhammer's curved surfaces), per-object colours, hidden visgroups stay shown.

use super::doc::{Brush, BrushId, Entity, EntityId, Face, GroupId, Group, MapDocument, TexAlign, VisSet};
use super::geom::map_points_for_plane;
use super::math::{Plane, Vec3};
use super::texture::from_standard;
use std::collections::HashMap;

const MAX_COUNT: usize = 1_000_000;

/// What an import made, and what it left out (plain words for the page).
#[derive(Debug)]
pub struct Imported {
    pub doc: MapDocument,
    pub notes: Vec<String>,
}

struct R<'a> {
    b: &'a [u8],
    at: usize,
    what: &'static str,
}

type Res<T> = Result<T, String>;

impl<'a> R<'a> {
    fn new(b: &'a [u8], what: &'static str) -> Self {
        R { b, at: 0, what }
    }
    fn short(&self) -> String {
        format!("This {} file ends too soon (at byte {}). It may be damaged or from a newer editor.", self.what, self.at)
    }
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if n > self.b.len() - self.at.min(self.b.len()) {
            return Err(self.short());
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Res<()> {
        self.take(n).map(|_| ())
    }
    fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Res<i16> {
        let s = self.take(2)?;
        Ok(i16::from_le_bytes([s[0], s[1]]))
    }
    fn i32(&mut self) -> Res<i32> {
        let s = self.take(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Res<f64> {
        let s = self.take(4)?;
        let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64;
        if !v.is_finite() {
            return Err(format!("This {} file has a broken number at byte {}.", self.what, self.at - 4));
        }
        Ok(v)
    }
    fn f64(&mut self) -> Res<f64> {
        let s = self.take(8)?;
        Ok(f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    fn vec(&mut self) -> Res<Vec3> {
        Ok(Vec3 { x: self.f32()?, y: self.f32()?, z: self.f32()? })
    }
    fn count(&mut self, what: &str) -> Res<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > MAX_COUNT || n as usize > self.b.len() {
            return Err(format!("This {} file has an impossible {what} count ({n}) at byte {}.", self.what, self.at - 4));
        }
        Ok(n as usize)
    }
    /// Fixed-width text, cut at the first zero.
    fn fixed(&mut self, n: usize) -> Res<String> {
        let s = self.take(n)?;
        let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
        Ok(String::from_utf8_lossy(&s[..end]).into_owned())
    }
    /// RMF: a byte length, then the text (with its zero).
    fn bstr(&mut self) -> Res<String> {
        let n = self.u8()? as usize;
        self.fixed(n)
    }
    /// JMF: an i32 length, then the text.
    fn istr(&mut self) -> Res<String> {
        let n = self.i32()?;
        if n <= 0 {
            return Ok(String::new());
        }
        if n as usize > 1 << 20 {
            return Err(format!("This {} file has an impossible text length at byte {}.", self.what, self.at - 4));
        }
        self.fixed(n as usize)
    }
    fn done(&self) -> bool {
        self.at >= self.b.len()
    }
}

/// A face as the editors store it: its corners and texture.
struct RawFace {
    verts: Vec<Vec3>,
    /// a plane from the file, when it has one (JMF), else from the corners
    plane: Option<Plane>,
    texture: String,
    align: Option<TexAlign>,
    /// Quake-style (RMF before 2.2): xshift, yshift, rotation, xscale, yscale
    standard: Option<[f64; 5]>,
}

/// Newell's normal of a polygon (either winding).
fn newell(v: &[Vec3]) -> Vec3 {
    let mut n = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    for i in 0..v.len() {
        let a = v[i];
        let b = v[(i + 1) % v.len()];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    n
}

/// Tidies float noise from 32-bit files: 63.99998 -> 64.
fn tidy(x: f64) -> f64 {
    let r = x.round();
    if (x - r).abs() < 1e-3 {
        r
    } else {
        (x * 1024.0).round() / 1024.0
    }
}

/// A brush from raw faces: planes facing out from the brush's middle, in our three-point form.
fn make_brush(faces: Vec<RawFace>) -> Option<Brush> {
    let all: Vec<Vec3> = faces.iter().flat_map(|f| f.verts.iter().copied()).collect();
    if all.len() < 4 || faces.len() < 4 {
        return None;
    }
    let n = all.len() as f64;
    let mid = Vec3 { x: all.iter().map(|p| p.x).sum::<f64>() / n, y: all.iter().map(|p| p.y).sum::<f64>() / n, z: all.iter().map(|p| p.z).sum::<f64>() / n };
    let mut out = Vec::new();
    for f in faces {
        let plane = match f.plane {
            Some(p) if p.normal.len() > 0.5 => p,
            _ => {
                if f.verts.len() < 3 {
                    continue;
                }
                let nrm = newell(&f.verts);
                if nrm.len() < 1e-9 {
                    continue;
                }
                let c = f.verts.iter().fold(Vec3 { x: 0.0, y: 0.0, z: 0.0 }, |a, p| a + *p) * (1.0 / f.verts.len() as f64);
                Plane::from_point_normal(c, nrm.normalized())
            }
        };
        // outward: the brush's middle must be behind the plane
        let plane = if plane.distance(mid) > 0.0 { plane.flipped() } else { plane };
        // snap nearly-axis normals and whole distances (32-bit floats in the file)
        let nn = Vec3 { x: tidy(plane.normal.x), y: tidy(plane.normal.y), z: tidy(plane.normal.z) };
        let plane = if nn.len() > 0.999 && nn.len() < 1.001 { Plane::new(nn.normalized(), tidy(plane.dist)) } else { plane };
        let points = map_points_for_plane(&plane).map(|p| Vec3 { x: tidy(p.x), y: tidy(p.y), z: tidy(p.z) });
        let align = match (f.align, f.standard) {
            (Some(a), _) => a,
            (None, Some([xs, ys, rot, sx, sy])) => from_standard(plane.normal, xs, ys, rot, sx, sy),
            _ => super::texture::world_aligned(plane.normal),
        };
        let texture = if f.texture.is_empty() { "AAATRIGGER".to_string() } else { f.texture };
        out.push(Face { points, texture, align });
    }
    if out.len() < 4 {
        return None;
    }
    Some(Brush { faces: out, entity: None, group: None })
}

fn valve(u: Vec3, us: f64, v: Vec3, vs: f64, rot: f64, su: f64, sv: f64) -> TexAlign {
    TexAlign {
        u_axis: u,
        u_offset: us,
        v_axis: v,
        v_offset: vs,
        rotation: rot,
        scale_u: if su.abs() < 1e-6 { 1.0 } else { su },
        scale_v: if sv.abs() < 1e-6 { 1.0 } else { sv },
    }
}

/// Builds the document as things are found; ids handed out in file order.
struct Builder {
    doc: MapDocument,
    /// file visgroup id -> set index
    vis: HashMap<i32, usize>,
    bad_brushes: usize,
}

impl Builder {
    fn new() -> Self {
        Builder { doc: MapDocument::new(), vis: HashMap::new(), bad_brushes: 0 }
    }
    fn add_vis(&mut self, id: i32, name: &str) {
        let i = self.doc.editor.vis_sets.len();
        self.doc.editor.vis_sets.push(VisSet { name: if name.is_empty() { format!("Visgroup {id}") } else { name.to_string() }, brushes: vec![], entities: vec![] });
        self.vis.insert(id, i);
    }
    fn group(&mut self, parent: Option<GroupId>, n: usize) -> GroupId {
        let id = self.doc.new_group_id();
        self.doc.groups.insert(id, Group { name: format!("Group {n}"), parent, hidden: false, locked: false });
        id
    }
    fn brush(&mut self, faces: Vec<RawFace>, entity: Option<EntityId>, group: Option<GroupId>, vis: &[i32]) -> Option<BrushId> {
        let Some(mut b) = make_brush(faces) else {
            self.bad_brushes += 1;
            return None;
        };
        b.entity = entity;
        b.group = if entity.is_some() { None } else { group };
        let id = self.doc.new_brush_id();
        self.doc.brushes.insert(id, b);
        for v in vis {
            if let Some(i) = self.vis.get(v) {
                self.doc.editor.vis_sets[*i].brushes.push(id);
            }
        }
        Some(id)
    }
    fn entity(&mut self, class: &str, keys: Vec<(String, String)>, flags: i32, origin: Option<Vec3>, group: Option<GroupId>, vis: &[i32]) -> EntityId {
        let mut e = Entity::new(class);
        for (k, v) in keys {
            if k.is_empty() || k == "classname" {
                continue;
            }
            e.set(&k, &v);
        }
        if flags != 0 && e.get("spawnflags").is_none() {
            e.set("spawnflags", &flags.to_string());
        }
        if let Some(o) = origin {
            e.set("origin", &format!("{} {} {}", fmt(o.x), fmt(o.y), fmt(o.z)));
        }
        e.group = group;
        let id = self.doc.new_entity_id();
        self.doc.entities.insert(id, e);
        for v in vis {
            if let Some(i) = self.vis.get(v) {
                self.doc.editor.vis_sets[*i].entities.push(id);
            }
        }
        id
    }
    fn world(&mut self, keys: Vec<(String, String)>) {
        for (k, v) in keys {
            match k.as_str() {
                "classname" | "mapversion" | "" => {}
                "wad" => self.doc.world.set_wad_key(&v),
                _ => self.doc.world.keys.push((k, v)),
            }
        }
    }
    fn finish(mut self, mut notes: Vec<String>) -> Imported {
        if self.bad_brushes > 0 {
            notes.push(format!("{} brush{} couldn't be read as a closed shape and {} left out.", self.bad_brushes, if self.bad_brushes == 1 { "" } else { "es" }, if self.bad_brushes == 1 { "was" } else { "were" }));
        }
        self.doc.editor.vis_sets.retain(|v| !v.brushes.is_empty() || !v.entities.is_empty());
        self.doc.revision += 1;
        Imported { doc: self.doc, notes }
    }
}

fn fmt(x: f64) -> String {
    super::mapfile::fmt_num(tidy(x))
}

// ---- RMF ---------------------------------------------------------------------------------

/// Reads a Hammer/Worldcraft .rmf (versions 0.8 to 2.2).
pub fn read_rmf(bytes: &[u8]) -> Res<Imported> {
    let mut r = R::new(bytes, ".rmf");
    let ver = (r.f32()? * 10.0).round() as i32;
    if ![8, 9, 14, 16, 18, 22].contains(&ver) {
        return Err(format!("This isn't a Hammer .rmf file this editor knows (version {:.1}).", ver as f64 / 10.0));
    }
    if r.take(3)? != b"RMF" {
        return Err("This isn't a Hammer .rmf file (no RMF mark).".into());
    }
    let mut b = Builder::new();
    let mut notes = Vec::new();
    if ver > 9 {
        let n = r.count("visgroup")?;
        for _ in 0..n {
            let name = r.fixed(128)?;
            r.skip(4)?; // colour
            let id = r.i32()?;
            let visible = r.u8()? != 0;
            r.skip(3)?;
            b.add_vis(id, &name);
            if !visible {
                notes.push(format!("Visgroup “{name}” was hidden in Hammer; it's shown here (hide it in Outline > visibility sets)."));
            }
        }
    }
    let kind = r.bstr()?;
    if kind != "CMapWorld" {
        return Err("This .rmf file doesn't start with the world.".into());
    }
    let mut groups = 0usize;
    let mut paths = 0usize;
    rmf_object_body(&mut r, ver, &mut b, &mut groups, "CMapWorld", None, None, &mut notes)?;
    if !r.done() {
        // the DOCINFO block (cameras) follows; nothing in it we keep
        if r.b.len() - r.at >= 8 && &r.b[r.at..r.at + 7] == b"DOCINFO" {
            notes.push("Hammer's saved cameras aren't brought over.".into());
        }
    }
    let _ = &mut paths;
    Ok(b.finish(notes))
}

fn rmf_base(r: &mut R, ver: i32) -> Res<Vec<i32>> {
    let vis = if ver <= 9 {
        let n = r.count("visgroup")?;
        r.take(n)?.iter().filter(|x| **x > 0).map(|x| *x as i32).collect()
    } else {
        let v = r.i32()?;
        if v > 0 {
            vec![v]
        } else {
            vec![]
        }
    };
    r.skip(3)?; // colour
    Ok(vis)
}

fn rmf_entity_data(r: &mut R, ver: i32) -> Res<(String, i32, Vec<(String, String)>)> {
    let class = r.bstr()?;
    r.skip(4)?;
    let flags = r.i32()?;
    let n = r.count("key")?;
    let mut keys = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        let k = r.bstr()?;
        let v = r.bstr()?;
        keys.push((k, v));
    }
    if ver >= 16 {
        r.skip(12)?;
    }
    Ok((class, flags, keys))
}

fn rmf_face(r: &mut R, ver: i32) -> Res<RawFace> {
    let texture = r.fixed(if ver <= 8 { 16 } else if ver <= 16 { 40 } else { 260 })?;
    let (mut align, mut standard) = (None, None);
    let (u, us, v, vs, rot);
    if ver <= 18 {
        rot = r.f32()?;
        us = r.f32()?;
        vs = r.f32()?;
        u = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
        v = u;
    } else {
        u = r.vec()?;
        us = r.f32()?;
        v = r.vec()?;
        vs = r.f32()?;
        rot = r.f32()?;
    }
    let su = r.f32()?;
    let sv = r.f32()?;
    if ver <= 9 {
    } else if ver <= 16 {
        r.skip(4)?;
    } else {
        r.skip(16)?;
    }
    let n = r.count("corner")?;
    let mut verts = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        verts.push(r.vec()?);
    }
    r.skip(36)?; // the three plane points (we use the corners)
    if ver <= 18 {
        standard = Some([us, vs, rot, if su == 0.0 { 1.0 } else { su }, if sv == 0.0 { 1.0 } else { sv }]);
    } else {
        align = Some(valve(u, us, v, vs, rot, su, sv));
    }
    Ok(RawFace { verts, plane: None, texture, align, standard })
}

#[allow(clippy::too_many_arguments)]
fn rmf_object_body(r: &mut R, ver: i32, b: &mut Builder, groups: &mut usize, kind: &str, group: Option<GroupId>, entity: Option<EntityId>, notes: &mut Vec<String>) -> Res<()> {
    let vis = rmf_base(r, ver)?;
    let n = r.count("object")?;
    match kind {
        "CMapWorld" => {
            for _ in 0..n {
                rmf_object(r, ver, b, groups, None, None, notes)?;
            }
            let (_, _, keys) = rmf_entity_data(r, ver)?;
            b.world(keys);
            if ver <= 9 {
                let ng = r.count("visgroup")?;
                for _ in 0..ng {
                    let name = r.fixed(128)?;
                    r.skip(4)?;
                    let id = r.u8()? as i32;
                    r.skip(3)?;
                    b.add_vis(id, &name);
                }
                // (old files list visgroups after the objects that use them: those links are lost)
            }
            if ver >= 14 {
                let np = r.count("path")?;
                for _ in 0..np {
                    rmf_path(r, b)?;
                }
                if np > 0 {
                    notes.push(format!("{np} path{} became path_corner/path_track entities linked by target.", if np == 1 { "" } else { "s" }));
                }
            }
        }
        "CMapGroup" => {
            *groups += 1;
            let g = b.group(group, *groups);
            for _ in 0..n {
                rmf_object(r, ver, b, groups, Some(g), None, notes)?;
            }
        }
        "CMapSolid" => {
            if n > 0 {
                return Err("This .rmf file has a brush with things inside it, which Hammer never writes.".into());
            }
            let nf = r.count("face")?;
            let mut faces = Vec::with_capacity(nf.min(64));
            for _ in 0..nf {
                faces.push(rmf_face(r, ver)?);
            }
            b.brush(faces, entity, group, &vis);
        }
        "CMapEntity" => {
            // the brushes come first, then the entity's data
            let mut solids: Vec<Vec<RawFace>> = Vec::new();
            let mut solid_vis = Vec::new();
            for _ in 0..n {
                let k = r.bstr()?;
                if k != "CMapSolid" {
                    return Err(format!("This .rmf file has a {k} inside an entity, which this editor can't read."));
                }
                let sv = rmf_base(r, ver)?;
                if r.count("object")? > 0 {
                    return Err("This .rmf file has a brush with things inside it.".into());
                }
                let nf = r.count("face")?;
                let mut faces = Vec::with_capacity(nf.min(64));
                for _ in 0..nf {
                    faces.push(rmf_face(r, ver)?);
                }
                solids.push(faces);
                solid_vis.push(sv);
            }
            let (class, flags, keys) = rmf_entity_data(r, ver)?;
            r.skip(2)?;
            let origin = r.vec()?;
            r.skip(4)?;
            let point = solids.is_empty();
            let e = b.entity(&class, keys, flags, point.then_some(origin), group, &vis);
            for (faces, sv) in solids.into_iter().zip(solid_vis) {
                b.brush(faces, Some(e), None, &sv);
            }
        }
        _ => return Err(format!("This .rmf file has a {kind}, which this editor can't read.")),
    }
    Ok(())
}

fn rmf_object(r: &mut R, ver: i32, b: &mut Builder, groups: &mut usize, group: Option<GroupId>, entity: Option<EntityId>, notes: &mut Vec<String>) -> Res<()> {
    let kind = r.bstr()?;
    match kind.as_str() {
        "CMapGroup" | "CMapSolid" | "CMapEntity" => rmf_object_body(r, ver, b, groups, &kind, group, entity, notes),
        other => Err(format!("This .rmf file has a {other}, which this editor can't read.")),
    }
}

struct PathNode {
    name: String,
    pos: Vec3,
    keys: Vec<(String, String)>,
}

/// A path's nodes as entities, each targeting the next (circular: the last the first;
/// ping-pong: Hammer writes it one way, so do we).
fn path_entities(b: &mut Builder, base: &str, class: &str, circular: bool, nodes: Vec<PathNode>) {
    let class = if class.is_empty() { "path_corner" } else { class };
    let names: Vec<String> = nodes.iter().enumerate().map(|(i, n)| if n.name.is_empty() { format!("{base}{:02}", i + 1) } else { n.name.clone() }).collect();
    let count = nodes.len();
    for (i, n) in nodes.into_iter().enumerate() {
        let mut keys = vec![("targetname".to_string(), names[i].clone())];
        let next = if i + 1 < count { Some(i + 1) } else if circular && count > 1 { Some(0) } else { None };
        if let Some(j) = next {
            keys.push(("target".to_string(), names[j].clone()));
        }
        keys.extend(n.keys);
        b.entity(class, keys, 0, Some(n.pos), None, &[]);
    }
}

fn rmf_path(r: &mut R, b: &mut Builder) -> Res<()> {
    let name = r.fixed(128)?;
    let class = r.fixed(128)?;
    let dir = r.i32()?;
    let n = r.count("path node")?;
    let mut nodes = Vec::new();
    for _ in 0..n {
        let pos = r.vec()?;
        let _id = r.i32()?;
        let nname = r.fixed(128)?;
        let nk = r.count("key")?;
        let mut keys = Vec::new();
        for _ in 0..nk {
            let k = r.bstr()?;
            let v = r.bstr()?;
            keys.push((k, v));
        }
        nodes.push(PathNode { name: nname, pos, keys });
    }
    path_entities(b, &name, &class, dir == 1, nodes);
    Ok(())
}

// ---- JMF ---------------------------------------------------------------------------------

/// Reads a J.A.C.K. .jmf (versions 121 and 122).
pub fn read_jmf(bytes: &[u8]) -> Res<Imported> {
    let mut r = R::new(bytes, ".jmf");
    if r.take(4)? != b"JHMF" {
        return Err("This isn't a J.A.C.K. .jmf file (no JHMF mark).".into());
    }
    let ver = r.i32()?;
    if ver != 121 && ver != 122 {
        return Err(format!("This .jmf file is version {ver}; this editor reads versions 121 and 122."));
    }
    let mut notes = Vec::new();
    let ne = r.count("export path")?;
    for _ in 0..ne {
        r.istr()?;
    }
    if ver >= 122 {
        let mut any = false;
        for _ in 0..3 {
            any |= !r.istr()?.is_empty();
            r.f64()?;
            r.skip(4 * 5 + 4)?; // luminance, filter, invert, offset x/y, padding
        }
        if any {
            notes.push("Background pictures in the flat views aren't brought over.".into());
        }
    }
    let mut b = Builder::new();
    // groups: id, parent, flags, count, colour
    let ng = r.count("group")?;
    let mut raw = Vec::new();
    for _ in 0..ng {
        let id = r.i32()?;
        let parent = r.i32()?;
        r.skip(12)?;
        raw.push((id, parent));
    }
    let mut gid: HashMap<i32, GroupId> = HashMap::new();
    let mut left = raw.clone();
    let mut n = 0usize;
    while !left.is_empty() {
        let before = left.len();
        left.retain(|(id, parent)| {
            let top = *parent == 0 || parent == id;
            if top || gid.contains_key(parent) {
                n += 1;
                let p = if top { None } else { gid.get(parent).copied() };
                let g = b.group(p, n);
                gid.insert(*id, g);
                false
            } else {
                true
            }
        });
        if left.len() == before {
            // a loop or a missing parent: put the rest at the top
            for (id, _) in left.drain(..) {
                n += 1;
                let g = b.group(None, n);
                gid.insert(id, g);
            }
        }
    }
    let nv = r.count("visgroup")?;
    for _ in 0..nv {
        let name = r.istr()?;
        let id = r.i32()?;
        r.skip(4)?;
        let visible = r.u8()? != 0;
        b.add_vis(id, &name);
        if !visible {
            notes.push(format!("Visgroup “{name}” was hidden in J.A.C.K.; it's shown here."));
        }
    }
    r.skip(24)?; // cordon box
    let nc = r.count("camera")?;
    r.skip(nc * (24 + 4 + 4))?;
    if nc > 0 {
        notes.push("J.A.C.K.'s saved cameras aren't brought over.".into());
    }
    let np = r.count("path")?;
    for _ in 0..np {
        let class = r.istr()?;
        let name = r.istr()?;
        let dir = r.i32()?;
        r.skip(8)?; // flags, colour
        let nn = r.count("path node")?;
        let mut nodes = Vec::new();
        for _ in 0..nn {
            let nname = r.istr()?;
            let fire = r.istr()?;
            let pos = r.vec()?;
            let ang = r.vec()?;
            let flags = r.i32()?;
            r.skip(4)?;
            let nk = r.count("key")?;
            let mut keys = Vec::new();
            if !fire.is_empty() {
                keys.push(("message".to_string(), fire));
            }
            if ang.x != 0.0 || ang.y != 0.0 || ang.z != 0.0 {
                keys.push(("angles".to_string(), format!("{} {} {}", fmt(ang.x), fmt(ang.y), fmt(ang.z))));
            }
            if flags != 0 {
                keys.push(("spawnflags".to_string(), flags.to_string()));
            }
            for _ in 0..nk {
                let k = r.istr()?;
                let v = r.istr()?;
                keys.push((k, v));
            }
            nodes.push(PathNode { name: nname, pos, keys });
        }
        path_entities(&mut b, &name, &class, dir == 1, nodes);
    }
    if np > 0 {
        notes.push(format!("{np} path{} became path_corner/path_track entities linked by target.", if np == 1 { "" } else { "s" }));
    }
    let mut patches = 0usize;
    let mut entities = 0usize;
    while !r.done() {
        entities += 1;
        if entities > MAX_COUNT {
            return Err("This .jmf file has too many entities.".into());
        }
        let class = r.istr()?;
        let origin = r.vec()?;
        let _flags = r.i32()?;
        let group = r.i32()?;
        let _root = r.i32()?;
        r.skip(4)?;
        for _ in 0..13 {
            r.istr()?;
        }
        let spawnflags = r.i32()?;
        r.vec()?; // angles (also in the keys)
        r.skip(4 + 4 + 4 + 4)?; // rendering, fx colour, render mode, render fx
        r.i16()?;
        r.i16()?;
        r.skip(4 + 4 * 3 + 28)?; // sequence, framerate, scale, radius, unknown
        let nk = r.count("key")?;
        let mut keys = Vec::new();
        for _ in 0..nk {
            let k = r.istr()?;
            let v = r.istr()?;
            keys.push((k, v));
        }
        let nvis = r.count("visgroup")?;
        let mut vis = Vec::new();
        for _ in 0..nvis {
            vis.push(r.i32()?);
        }
        let ns = r.count("brush")?;
        let mut solids = Vec::new();
        for _ in 0..ns {
            let npatch = r.count("patch")?;
            r.skip(4)?; // flags
            let sgroup = r.i32()?;
            r.skip(4 + 4)?; // root group, colour
            let nsv = r.count("visgroup")?;
            let mut svis = Vec::new();
            for _ in 0..nsv {
                svis.push(r.i32()?);
            }
            let nf = r.count("face")?;
            let mut faces = Vec::new();
            for _ in 0..nf {
                r.skip(4)?; // render flags
                let nvert = r.count("corner")?;
                let u = r.vec()?;
                let us = r.f32()?;
                let v = r.vec()?;
                let vs = r.f32()?;
                let su = r.f32()?;
                let sv = r.f32()?;
                let rot = r.f32()?;
                r.skip(16 + 4)?; // unknown, surface flags
                let texture = r.fixed(64)?;
                let normal = r.vec()?;
                let dist = r.f32()?;
                r.skip(4)?;
                let mut verts = Vec::new();
                for _ in 0..nvert {
                    verts.push(r.vec()?);
                    r.skip(12)?; // texture coordinates
                }
                let plane = (normal.len() > 0.5).then(|| Plane::new(normal.normalized(), dist));
                faces.push(RawFace { verts, plane, texture, align: Some(valve(u, us, v, vs, rot, su, sv)), standard: None });
            }
            for _ in 0..npatch {
                r.skip(4 + 4)?; // width, height
                r.skip(12 + 4 + 12 + 4 + 4 + 4 + 4 + 16 + 4 + 64)?; // surface properties
                r.skip(4)?;
                r.skip(32 * 32 * 36)?;
            }
            patches += npatch;
            solids.push((faces, sgroup, svis));
        }
        let g = gid.get(&group).copied();
        if class == "worldspawn" {
            b.world(keys);
            for (faces, sg, sv) in solids {
                b.brush(faces, None, gid.get(&sg).copied(), &sv);
            }
        } else {
            let point = solids.is_empty();
            let e = b.entity(&class, keys, spawnflags, point.then_some(origin), g, &vis);
            for (faces, _, sv) in solids {
                b.brush(faces, Some(e), None, &sv);
            }
        }
    }
    if patches > 0 {
        notes.push(format!("{patches} curved surface{} (J.A.C.K. patches) can't be used in GoldSrc maps and {} left out.", if patches == 1 { "" } else { "s" }, if patches == 1 { "was" } else { "were" }));
    }
    Ok(b.finish(notes))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::editor::geom::{brush_bounds, is_valid};

    // ---- hand-built files from the format docs -----------------------------------------

    #[derive(Default)]
    pub struct W(pub Vec<u8>);
    impl W {
        pub fn f(&mut self, x: f32) -> &mut Self {
            self.0.extend_from_slice(&x.to_le_bytes());
            self
        }
        pub fn i(&mut self, x: i32) -> &mut Self {
            self.0.extend_from_slice(&x.to_le_bytes());
            self
        }
        pub fn b(&mut self, x: &[u8]) -> &mut Self {
            self.0.extend_from_slice(x);
            self
        }
        pub fn v(&mut self, x: [f32; 3]) -> &mut Self {
            self.f(x[0]).f(x[1]).f(x[2])
        }
        pub fn fixed(&mut self, s: &str, n: usize) -> &mut Self {
            let mut b = s.as_bytes().to_vec();
            b.resize(n, 0);
            self.b(&b)
        }
        /// RMF string: byte length (with the zero), text, zero
        pub fn bs(&mut self, s: &str) -> &mut Self {
            self.0.push(s.len() as u8 + 1);
            self.b(s.as_bytes()).b(&[0])
        }
        /// JMF string: i32 length, text, zero
        pub fn is(&mut self, s: &str) -> &mut Self {
            if s.is_empty() {
                return self.i(0);
            }
            self.i(s.len() as i32 + 1).b(s.as_bytes()).b(&[0])
        }
    }

    /// A box's faces as (outward normal, corners clockwise seen from outside, like Hammer).
    fn box_faces(min: [f32; 3], max: [f32; 3]) -> Vec<([f32; 3], Vec<[f32; 3]>)> {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        vec![
            ([0.0, 0.0, 1.0], vec![[x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1]]),
            ([0.0, 0.0, -1.0], vec![[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]]),
            ([1.0, 0.0, 0.0], vec![[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]]),
            ([-1.0, 0.0, 0.0], vec![[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]]),
            ([0.0, 1.0, 0.0], vec![[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]]),
            ([0.0, -1.0, 0.0], vec![[x0, y0, z0], [x0, y0, z1], [x1, y0, z1], [x1, y0, z0]]),
        ]
    }

    fn rmf_solid(w: &mut W, min: [f32; 3], max: [f32; 3], tex: &str, vis: i32) {
        w.bs("CMapSolid").i(vis).b(&[200, 100, 50]).i(0);
        let faces = box_faces(min, max);
        w.i(faces.len() as i32);
        for (n, verts) in faces {
            w.fixed(tex, 260);
            // u, shift, v, shift, rotation, scales
            let (u, v) = if n[2] != 0.0 { ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0]) } else if n[0] != 0.0 { ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]) } else { ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]) };
            w.v(u).f(8.0).v(v).f(4.0).f(0.0).f(1.0).f(1.0).b(&[0; 16]);
            w.i(verts.len() as i32);
            for p in &verts {
                w.v(*p);
            }
            w.v(verts[0]).v(verts[1]).v(verts[2]);
        }
    }

    fn rmf_kv(w: &mut W, class: &str, flags: i32, kv: &[(&str, &str)]) {
        w.bs(class).b(&[0; 4]).i(flags).i(kv.len() as i32);
        for (k, v) in kv {
            w.bs(k).bs(v);
        }
        w.b(&[0; 12]);
    }

    pub fn sample_rmf() -> Vec<u8> {
        let mut w = W::default();
        w.f(2.2).b(b"RMF");
        // two visgroups, the second hidden
        w.i(2);
        w.fixed("Walls", 128).b(&[1, 2, 3, 0]).i(1).b(&[1, 0, 0, 0]);
        w.fixed("Lights", 128).b(&[1, 2, 3, 0]).i(2).b(&[0, 0, 0, 0]);
        w.bs("CMapWorld").i(0).b(&[0, 0, 0]);
        // objects: a solid, a group (a solid + a light), a brush entity
        w.i(3);
        rmf_solid(&mut w, [0.0, 0.0, 0.0], [64.0, 64.0, 16.0], "CRATE1", 1);
        w.bs("CMapGroup").i(0).b(&[0, 0, 0]).i(2);
        rmf_solid(&mut w, [128.0, 0.0, 0.0], [192.0, 32.0, 64.0], "WALL", 1);
        w.bs("CMapEntity").i(2).b(&[0, 0, 0]).i(0);
        rmf_kv(&mut w, "light", 0, &[("_light", "255 255 128 200")]);
        w.b(&[0; 2]).v([16.0, 16.0, 48.0]).b(&[0; 4]);
        w.bs("CMapEntity").i(0).b(&[0, 0, 0]).i(1);
        rmf_solid(&mut w, [0.0, 128.0, 0.0], [32.0, 160.0, 96.0], "DOOR", 0);
        rmf_kv(&mut w, "func_door", 1, &[("targetname", "door1"), ("speed", "100")]);
        w.b(&[0; 2]).v([0.0, 0.0, 0.0]).b(&[0; 4]);
        // world data
        rmf_kv(&mut w, "worldspawn", 0, &[("wad", "\\half-life\\valve\\halflife.wad;\\cstrike\\cstrike.wad"), ("skyname", "desert")]);
        // one path of two nodes, circular
        w.i(1);
        w.fixed("train", 128).fixed("path_corner", 128).i(1).i(2);
        w.v([0.0, 0.0, 128.0]).i(1).fixed("", 128).i(1).bs("speed").bs("50");
        w.v([256.0, 0.0, 128.0]).i(2).fixed("", 128).i(0);
        w.b(b"DOCINFO\0").f(0.2).i(0).i(0);
        w.0
    }

    #[test]
    fn rmf_from_the_spec() {
        let im = read_rmf(&sample_rmf()).unwrap();
        let d = &im.doc;
        assert_eq!(d.brushes.len(), 3);
        assert!(d.brushes.values().all(is_valid), "every brush is a closed convex shape");
        let crate_b = d.brushes.values().find(|b| b.faces[0].texture == "CRATE1").unwrap();
        let bb = brush_bounds(crate_b);
        assert_eq!((bb.min.x, bb.min.y, bb.min.z, bb.max.x, bb.max.y, bb.max.z), (0.0, 0.0, 0.0, 64.0, 64.0, 16.0));
        assert_eq!(crate_b.faces[0].align.u_offset, 8.0);
        assert_eq!(crate_b.faces[0].align.v_offset, 4.0);
        // the group holds the wall and the light
        assert_eq!(d.groups.len(), 1);
        let g = *d.groups.keys().next().unwrap();
        let wall = d.brushes.values().find(|b| b.faces[0].texture == "WALL").unwrap();
        assert_eq!(wall.group, Some(g));
        let light = d.entities.values().find(|e| e.classname() == "light").unwrap();
        assert_eq!(light.group, Some(g));
        assert_eq!(light.get("origin"), Some("16 16 48"));
        // the door keeps its brush, keys and spawnflags, and has no origin
        let (did, door) = d.entities.iter().find(|(_, e)| e.classname() == "func_door").unwrap();
        assert_eq!(door.get("spawnflags"), Some("1"));
        assert_eq!(door.get("origin"), None);
        assert_eq!(d.brushes.values().filter(|b| b.entity == Some(*did)).count(), 1);
        // world: WADs and sky
        assert_eq!(d.world.wads.len(), 2);
        assert_eq!(d.world.get("skyname"), Some("desert"));
        // visgroups as sets
        let walls = d.editor.vis_sets.iter().find(|v| v.name == "Walls").unwrap();
        assert_eq!(walls.brushes.len(), 2);
        assert_eq!(d.editor.vis_sets.iter().find(|v| v.name == "Lights").unwrap().entities.len(), 1);
        // the path: two linked nodes, round in a circle
        let nodes: Vec<&Entity> = d.entities.values().filter(|e| e.classname() == "path_corner").collect();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].get("targetname"), Some("train01"));
        assert_eq!(nodes[0].get("target"), Some("train02"));
        assert_eq!(nodes[1].get("target"), Some("train01"));
        assert_eq!(nodes[0].get("speed"), Some("50"));
        assert!(im.notes.iter().any(|n| n.contains("Lights")), "the hidden visgroup is mentioned");
        assert!(im.notes.iter().any(|n| n.contains("cameras")));
        // and it writes out as a .map that reads back the same
        let text = crate::editor::mapfile::write_map(d, &Default::default()).unwrap();
        let back = crate::editor::mapfile::read_map(&text).unwrap();
        assert_eq!(back.brushes.len(), 3);
    }

    #[test]
    fn rmf_damaged_files_say_so() {
        let full = sample_rmf();
        for cut in [0, 3, 7, 40, 200, full.len() / 2, full.len() - 30] {
            let e = read_rmf(&full[..cut]);
            assert!(e.is_err(), "cut at {cut}");
        }
        let mut bad = full.clone();
        bad[4] = b'X';
        assert!(read_rmf(&bad).unwrap_err().contains("RMF"));
        // a huge count doesn't allocate or panic
        let mut huge = full.clone();
        huge[7..11].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(read_rmf(&huge).is_err());
    }

    fn jmf_face(w: &mut W, n: [f32; 3], verts: &[[f32; 3]], tex: &str) {
        w.i(0).i(verts.len() as i32);
        w.v([1.0, 0.0, 0.0]).f(2.0).v([0.0, 0.0, -1.0]).f(3.0).f(0.5).f(0.5).f(0.0).b(&[0; 16]).i(0).fixed(tex, 64);
        let d = n[0] * verts[0][0] + n[1] * verts[0][1] + n[2] * verts[0][2];
        w.v(n).f(d).i(0);
        for p in verts {
            w.v(*p).v([0.0, 0.0, 0.0]);
        }
    }

    fn jmf_entity(w: &mut W, class: &str, origin: [f32; 3], group: i32, kv: &[(&str, &str)], vis: &[i32], solids: &[([f32; 3], [f32; 3], i32)]) {
        w.is(class).v(origin).i(0).i(group).i(group).b(&[0; 4]);
        for _ in 0..13 {
            w.is("");
        }
        w.i(0).v([0.0, 0.0, 0.0]).i(0).b(&[0; 4]).i(0).i(0);
        w.b(&[0; 4]); // body, skin (i16 each)
        w.i(0).f(0.0).f(1.0).f(0.0).b(&[0; 28]);
        w.i(kv.len() as i32);
        for (k, v) in kv {
            w.is(k).is(v);
        }
        w.i(vis.len() as i32);
        for v in vis {
            w.i(*v);
        }
        w.i(solids.len() as i32);
        for (min, max, g) in solids {
            w.i(0).i(0).i(*g).i(*g).b(&[0; 4]).i(0);
            let faces = box_faces(*min, *max);
            w.i(faces.len() as i32);
            for (n, verts) in faces {
                jmf_face(w, n, &verts, "BRICK");
            }
        }
    }

    pub fn sample_jmf(version: i32) -> Vec<u8> {
        let mut w = W::default();
        w.b(b"JHMF").i(version).i(1).is("C:\\maps\\x.map");
        if version >= 122 {
            for _ in 0..3 {
                w.is("").0.extend_from_slice(&1.0f64.to_le_bytes());
                w.i(0).i(0).i(0).i(0).i(0).b(&[0; 4]);
            }
        }
        // groups: 7 at the top, 9 inside 7
        w.i(2).i(9).i(7).i(0).i(1).b(&[0; 4]).i(7).i(0).i(0).i(2).b(&[0; 4]);
        w.i(1).is("Detail").i(5).b(&[0; 4]).b(&[1]);
        w.v([-4096.0, -4096.0, -4096.0]).v([4096.0, 4096.0, 4096.0]);
        w.i(1).v([0.0, 0.0, 0.0]).v([1.0, 0.0, 0.0]).i(0).b(&[0; 4]);
        w.i(0); // paths
        jmf_entity(&mut w, "worldspawn", [0.0; 3], 0, &[("wad", "a.wad;b.wad"), ("skyname", "night")], &[], &[([0.0, 0.0, 0.0], [32.0, 32.0, 32.0], 9), ([64.0, 0.0, 0.0], [96.0, 32.0, 32.0], 0)]);
        jmf_entity(&mut w, "info_player_start", [8.0, 8.0, 36.0], 7, &[("angle", "90")], &[5], &[]);
        jmf_entity(&mut w, "func_wall", [0.0; 3], 0, &[("rendermode", "4")], &[], &[([0.0, 64.0, 0.0], [16.0, 80.0, 64.0], 0)]);
        w.0
    }

    #[test]
    fn jmf_from_the_spec() {
        for version in [121, 122] {
            let im = read_jmf(&sample_jmf(version)).unwrap();
            let d = &im.doc;
            assert_eq!(d.brushes.len(), 3, "v{version}");
            assert!(d.brushes.values().all(is_valid));
            assert_eq!(d.world.wads, vec!["a.wad", "b.wad"]);
            assert_eq!(d.world.get("skyname"), Some("night"));
            // nested groups: 9 inside 7
            assert_eq!(d.groups.len(), 2);
            let inner = d.groups.values().find(|g| g.parent.is_some()).unwrap();
            assert!(d.groups.contains_key(&inner.parent.unwrap()));
            let spawn = d.entities.values().find(|e| e.classname() == "info_player_start").unwrap();
            assert_eq!(spawn.get("origin"), Some("8 8 36"));
            assert!(spawn.group.is_some());
            let (wid, _) = d.entities.iter().find(|(_, e)| e.classname() == "func_wall").unwrap();
            assert_eq!(d.brushes.values().filter(|b| b.entity == Some(*wid)).count(), 1);
            assert_eq!(d.editor.vis_sets.len(), 1);
            assert_eq!(d.editor.vis_sets[0].name, "Detail");
            let b = d.brushes.values().find(|b| b.entity.is_none() && b.group.is_some()).unwrap();
            assert_eq!(b.faces[0].align.scale_u, 0.5);
            assert!(im.notes.iter().any(|n| n.contains("cameras")));
        }
    }

    #[test]
    fn jmf_damaged_files_say_so() {
        let full = sample_jmf(121);
        for cut in [0, 4, 9, 30, full.len() / 3, full.len() - 7] {
            assert!(read_jmf(&full[..cut]).is_err(), "cut at {cut}");
        }
        assert!(read_jmf(b"JHMF\x7b\x00\x00\x00").is_err());
        assert!(read_jmf(b"NOPE").unwrap_err().contains("JHMF"));
    }

    /// A real Hammer file when one is on this computer (opt-in):
    /// `LS_TEST_RMF=<file.rmf> cargo test real_rmf -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_rmf() {
        let Ok(p) = std::env::var("LS_TEST_RMF") else { return };
        let im = read_rmf(&std::fs::read(p).unwrap()).unwrap();
        let bad = im.doc.brushes.values().filter(|b| !is_valid(b)).count();
        println!("{} brushes ({bad} not valid), {} entities, {} groups, {} visgroups; notes: {:?}", im.doc.brushes.len(), im.doc.entities.len(), im.doc.groups.len(), im.doc.editor.vis_sets.len(), im.notes);
        assert_eq!(bad, 0);
        crate::editor::mapfile::write_map(&im.doc, &Default::default()).unwrap();
    }
}
