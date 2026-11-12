//! BSP30 (Half-Life / Counter-Strike 1.6 / Condition Zero) map files: the header's 15 lumps
//! and the ones the viewer draws from. Layouts (little-endian):
//! - header: i32 version (30), then 15 × (i32 offset, i32 length)
//! - planes 20: normal f32×3, dist f32, type i32
//! - vertices 12: f32×3
//! - edges 4: u16 v0, u16 v1
//! - surfedges 4: i32 (negative = the edge walked backwards)
//! - faces 20: u16 plane, u16 side, i32 first surfedge, u16 surfedge count, u16 texinfo,
//!   u8 styles[4], i32 light offset (-1 = none)
//! - texinfo 40: f32 s[4], f32 t[4] (xyz + offset), u32 miptex, u32 flags
//! - textures: u32 count, i32 offsets[count] (from the lump's start, -1 = none), each a
//!   miptex: name[16], u32 width, u32 height, u32 mip offsets[4] (0 = pixels are in a WAD)
//! - models 64: mins f32×3, maxs f32×3, origin f32×3, i32 heads[4], i32 visleafs,
//!   i32 first face, i32 face count
//! - lighting: RGB bytes, faces point into it

use super::{cname, Error, Le, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const VERSION: u32 = 30;
pub const LUMPS: usize = 15;
pub const HEADER_SIZE: usize = 4 + LUMPS * 8;

pub const LUMP_ENTITIES: usize = 0;
pub const LUMP_PLANES: usize = 1;
pub const LUMP_TEXTURES: usize = 2;
pub const LUMP_VERTICES: usize = 3;
pub const LUMP_TEXINFO: usize = 6;
pub const LUMP_FACES: usize = 7;
pub const LUMP_LIGHTING: usize = 8;
pub const LUMP_EDGES: usize = 12;
pub const LUMP_SURFEDGES: usize = 13;
pub const LUMP_MODELS: usize = 14;

const WHAT: &str = "map file";
/// Most edges one face may have (the compilers stop well below this).
pub const MAX_FACE_EDGES: usize = 64;
/// Most bytes of embedded texture records one map may keep in memory.
pub const MAX_EMBEDDED: usize = 64 << 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub dist: f32,
    pub kind: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    pub plane: u16,
    pub side: u16,
    pub first_edge: i32,
    pub num_edges: u16,
    pub texinfo: u16,
    pub styles: [u8; 4],
    pub light_ofs: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexInfo {
    pub s: [f32; 4],
    pub t: [f32; 4],
    pub miptex: u32,
    pub flags: u32,
}

impl TexInfo {
    /// Texture-space (s, t) of a point, in texels (not yet divided by the texture size).
    pub fn st(&self, p: [f32; 3]) -> (f32, f32) {
        let d = |v: &[f32; 4]| p[0] * v[0] + p[1] * v[1] + p[2] * v[2] + v[3];
        (d(&self.s), d(&self.t))
    }
}

/// A texture the map uses. `embedded` is the whole miptex record (header + pixels +
/// palette) when the map carries the texture itself; None when it's in a WAD.
#[derive(Debug, Clone, PartialEq)]
pub struct MipTex {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub embedded: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub origin: [f32; 3],
    pub heads: [i32; 4],
    pub first_face: i32,
    pub num_faces: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lump {
    pub offset: u32,
    pub length: u32,
}

/// The header: version and where each lump is. Every lump is checked to lie in the file.
pub fn header(b: &[u8]) -> Result<[Lump; LUMPS]> {
    let r = Le::new(b, WHAT);
    if b.len() < HEADER_SIZE {
        return Err(Error::damaged(WHAT, "it's shorter than a map header"));
    }
    let version = r.u32(0)?;
    if version != VERSION {
        let detail = match &b[0..4] {
            b"IBSP" | b"VBSP" => "it's a Quake 2 / Source map".to_string(),
            _ if version == 29 => "it's a Quake map".to_string(),
            _ => format!("map version {version}, not 30"),
        };
        return Err(Error::NotFormat { what: WHAT, detail });
    }
    let mut lumps = [Lump::default(); LUMPS];
    for (i, l) in lumps.iter_mut().enumerate() {
        let (o, n) = (r.i32(4 + i * 8)?, r.i32(8 + i * 8)?);
        if o < 0 || n < 0 || (o as u64 + n as u64) > b.len() as u64 {
            return Err(Error::damaged(WHAT, format!("part {i} points outside the file")));
        }
        *l = Lump { offset: o as u32, length: n as u32 };
    }
    Ok(lumps)
}

fn lump<'a>(b: &'a [u8], l: Lump) -> &'a [u8] {
    // header() checked the range
    &b[l.offset as usize..(l.offset + l.length) as usize]
}

/// Reads fixed-size records from a lump; a length that isn't a whole number of records is
/// a damaged file.
fn records<T>(b: &[u8], name: &'static str, size: usize, mut each: impl FnMut(Le, usize) -> Result<T>) -> Result<Vec<T>> {
    if b.len() % size != 0 {
        return Err(Error::damaged(WHAT, format!("its {name} list has a broken length")));
    }
    let r = Le::new(b, WHAT);
    (0..b.len() / size).map(|i| each(r, i * size)).collect()
}

pub fn planes(b: &[u8]) -> Result<Vec<Plane>> {
    records(b, "planes", 20, |r, o| Ok(Plane { normal: r.vec3(o)?, dist: r.f32(o + 12)?, kind: r.i32(o + 16)? }))
}

pub fn vertices(b: &[u8]) -> Result<Vec<[f32; 3]>> {
    records(b, "vertices", 12, |r, o| r.vec3(o))
}

pub fn edges(b: &[u8]) -> Result<Vec<[u16; 2]>> {
    records(b, "edges", 4, |r, o| Ok([r.u16(o)?, r.u16(o + 2)?]))
}

pub fn surfedges(b: &[u8]) -> Result<Vec<i32>> {
    records(b, "surface edges", 4, |r, o| r.i32(o))
}

pub fn faces(b: &[u8]) -> Result<Vec<Face>> {
    records(b, "faces", 20, |r, o| {
        Ok(Face {
            plane: r.u16(o)?,
            side: r.u16(o + 2)?,
            first_edge: r.i32(o + 4)?,
            num_edges: r.u16(o + 8)?,
            texinfo: r.u16(o + 10)?,
            styles: r.bytes(o + 12, 4)?.try_into().unwrap(),
            light_ofs: r.i32(o + 16)?,
        })
    })
}

pub fn texinfo(b: &[u8]) -> Result<Vec<TexInfo>> {
    records(b, "texture mappings", 40, |r, o| {
        let v = |at: usize| -> Result<[f32; 4]> { Ok([r.f32(at)?, r.f32(at + 4)?, r.f32(at + 8)?, r.f32(at + 12)?]) };
        Ok(TexInfo { s: v(o)?, t: v(o + 16)?, miptex: r.u32(o + 32)?, flags: r.u32(o + 36)? })
    })
}

pub fn models(b: &[u8]) -> Result<Vec<Model>> {
    records(b, "models", 64, |r, o| {
        Ok(Model {
            mins: r.vec3(o)?,
            maxs: r.vec3(o + 12)?,
            origin: r.vec3(o + 24)?,
            heads: [r.i32(o + 36)?, r.i32(o + 40)?, r.i32(o + 44)?, r.i32(o + 48)?],
            first_face: r.i32(o + 56)?,
            num_faces: r.i32(o + 60)?,
        })
    })
}

/// The texture lump. A missing entry (offset -1) or one that points outside the lump
/// becomes a nameless 16×16 texture rather than failing the whole map. With `pixels` the
/// embedded textures keep their miptex record (for decoding); without, only the sizes.
pub fn textures(b: &[u8], pixels: bool) -> Result<Vec<MipTex>> {
    if b.is_empty() {
        return Ok(Vec::new());
    }
    let r = Le::new(b, WHAT);
    let count = r.u32(0)? as usize;
    if count > 1 << 16 || 4 + count * 4 > b.len() {
        return Err(Error::damaged(WHAT, "its texture list has a broken length"));
    }
    let blank = || MipTex { name: String::new(), width: 16, height: 16, embedded: None };
    let mut out = Vec::with_capacity(count);
    let mut kept = 0usize;
    for i in 0..count {
        let at = r.i32(4 + i * 4)?;
        if at < 0 || at as usize + 40 > b.len() {
            out.push(blank());
            continue;
        }
        let at = at as usize;
        let name = cname(r.bytes(at, 16)?);
        let (w, h) = (r.u32(at + 16)?, r.u32(at + 20)?);
        if w == 0 || h == 0 || w > super::MAX_TEX_SIDE || h > super::MAX_TEX_SIDE {
            out.push(MipTex { name, ..blank() });
            continue;
        }
        let mip0 = r.u32(at + 24)?;
        // keep only what the record needs (header, four mips, palette), never the rest of
        // the lump: many entries pointing at the start mustn't each copy the whole lump
        let need = 40 + (w as usize * h as usize) * 85 / 64 + 2 + 768 + 2;
        let end = (at + need).min(b.len());
        let embedded = if mip0 == 0 {
            None
        } else if pixels && kept + (end - at) <= MAX_EMBEDDED {
            kept += end - at;
            Some(b[at..end].to_vec())
        } else {
            // without pixels (or past the cap): still say it's embedded, with just the header
            Some(b[at..at + 40].to_vec())
        };
        out.push(MipTex { name, width: w, height: h, embedded });
    }
    Ok(out)
}

/// The map parts the viewer uses, parsed and checked.
#[derive(Debug, Clone)]
pub struct Bsp {
    pub entities: String,
    pub planes: Vec<Plane>,
    pub vertices: Vec<[f32; 3]>,
    pub edges: Vec<[u16; 2]>,
    pub surfedges: Vec<i32>,
    pub faces: Vec<Face>,
    pub texinfo: Vec<TexInfo>,
    pub textures: Vec<MipTex>,
    pub models: Vec<Model>,
    pub lighting: Vec<u8>,
}

impl Bsp {
    /// Parses a whole map file. `pixels`: keep embedded textures' pixels.
    pub fn parse(b: &[u8], pixels: bool) -> Result<Bsp> {
        let h = header(b)?;
        let l = |i: usize| lump(b, h[i]);
        let bsp = Bsp {
            entities: crate::rcon::latin1(trim_nul(l(LUMP_ENTITIES))),
            planes: planes(l(LUMP_PLANES))?,
            vertices: vertices(l(LUMP_VERTICES))?,
            edges: edges(l(LUMP_EDGES))?,
            surfedges: surfedges(l(LUMP_SURFEDGES))?,
            faces: faces(l(LUMP_FACES))?,
            texinfo: texinfo(l(LUMP_TEXINFO))?,
            textures: textures(l(LUMP_TEXTURES), pixels)?,
            models: models(l(LUMP_MODELS))?,
            lighting: l(LUMP_LIGHTING).to_vec(),
        };
        if bsp.models.is_empty() {
            return Err(Error::damaged(WHAT, "it has no world model"));
        }
        Ok(bsp)
    }

    pub fn open(path: &Path, pixels: bool) -> Result<Bsp> {
        Bsp::parse(&super::read_capped(path, WHAT, super::MAX_BSP)?, pixels)
    }

    /// A face's corners in order, or None when it points outside the map's lists (that
    /// face is skipped; the rest of the map still draws).
    pub fn face_points(&self, f: &Face) -> Option<Vec<[f32; 3]>> {
        let n = f.num_edges as usize;
        if !(3..=MAX_FACE_EDGES).contains(&n) || f.first_edge < 0 {
            return None;
        }
        let first = f.first_edge as usize;
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let e = *self.surfedges.get(first + k)?;
            let v = if e >= 0 { self.edges.get(e as usize)?[0] } else { self.edges.get(e.unsigned_abs() as usize)?[1] };
            out.push(*self.vertices.get(v as usize)?);
        }
        Some(out)
    }

    /// A face's normal (the plane's, turned round for back faces).
    pub fn face_normal(&self, f: &Face) -> Option<[f32; 3]> {
        let n = self.planes.get(f.plane as usize)?.normal;
        Some(if f.side != 0 { [-n[0], -n[1], -n[2]] } else { n })
    }

    /// The texture a face uses (its texinfo and the texture), if both exist.
    pub fn face_texture(&self, f: &Face) -> Option<(&TexInfo, usize)> {
        let ti = self.texinfo.get(f.texinfo as usize)?;
        let t = ti.miptex as usize;
        (t < self.textures.len()).then_some((ti, t))
    }
}

fn trim_nul(b: &[u8]) -> &[u8] {
    let end = b.iter().rposition(|&c| c != 0).map(|i| i + 1).unwrap_or(0);
    &b[..end]
}

/// Reads just some lumps of a map file from the disk (the texture route needs the
/// textures and entities, not the 30 MB of the rest). Same checks as `header`.
pub fn read_lumps(path: &Path, which: &[usize]) -> Result<Vec<Vec<u8>>> {
    let mut f = std::fs::File::open(path)?;
    let size = f.metadata()?.len();
    if size > super::MAX_BSP {
        return Err(Error::TooBig { what: WHAT, size, max: super::MAX_BSP });
    }
    let mut head = vec![0u8; HEADER_SIZE];
    f.read_exact(&mut head).map_err(|_| Error::damaged(WHAT, "it's shorter than a map header"))?;
    let r = Le::new(&head, WHAT);
    let version = r.u32(0)?;
    if version != VERSION {
        return Err(Error::NotFormat { what: WHAT, detail: format!("map version {version}, not 30") });
    }
    let mut out = Vec::with_capacity(which.len());
    for &i in which {
        if i >= LUMPS {
            return Err(Error::damaged(WHAT, format!("there's no part {i}")));
        }
        let (o, n) = (r.i32(4 + i * 8)?, r.i32(8 + i * 8)?);
        if o < 0 || n < 0 || o as u64 + n as u64 > size {
            return Err(Error::damaged(WHAT, format!("part {i} points outside the file")));
        }
        f.seek(SeekFrom::Start(o as u64))?;
        let mut buf = vec![0u8; n as usize];
        f.read_exact(&mut buf)?;
        out.push(buf);
    }
    Ok(out)
}

/// A tiny map built in memory, for tests (and the start of the phase 2 writer): a cube room
/// seen from inside, 256 units a side, centred on the origin. Walls use "brick" (16×16,
/// embedded in the map), floor and ceiling "{grate" (16×16, from test.wad). Model 1 is a
/// func_wall: one 64×64 face at z=0 facing up, textured "brick". Every face has a lightmap.
#[cfg(test)]
pub mod fixture {
    use super::*;

    pub const ENTITIES: &str = "{\n\"classname\" \"worldspawn\"\n\"wad\" \"\\\\sierra\\\\half-life\\\\cstrike\\\\test.wad;missing.wad\"\n\"skyname\" \"desert\"\n\"message\" \"Test Room\"\n}\n{\n\"classname\" \"func_wall\"\n\"model\" \"*1\"\n\"rendermode\" \"4\"\n\"renderamt\" \"200\"\n}\n{\n\"classname\" \"info_player_start\"\n\"origin\" \"0 0 -92\"\n\"angles\" \"0 90 0\"\n}\n{\n\"classname\" \"info_player_deathmatch\"\n\"origin\" \"64 0 -92\"\n\"angle\" \"180\"\n}\n";

    pub struct Builder {
        pub planes: Vec<Plane>,
        pub vertices: Vec<[f32; 3]>,
        pub edges: Vec<[u16; 2]>,
        pub surfedges: Vec<i32>,
        pub faces: Vec<Face>,
        pub texinfo: Vec<TexInfo>,
        pub lighting: Vec<u8>,
    }

    impl Builder {
        /// Adds a face from its corners (in the order that faces `normal`), with a lightmap
        /// of the right size filled with `light`.
        pub fn face(&mut self, pts: &[[f32; 3]], normal: [f32; 3], texinfo: u16, light: u8) {
            let dist = normal[0] * pts[0][0] + normal[1] * pts[0][1] + normal[2] * pts[0][2];
            let plane = self.planes.len() as u16;
            self.planes.push(Plane { normal, dist, kind: 5 });
            let first_edge = self.surfedges.len() as i32;
            for k in 0..pts.len() {
                let a = self.vertices.len() as u16;
                self.vertices.push(pts[k]);
                let e = self.edges.len() as i32;
                // edges point to the next corner's vertex, added on the following pass:
                // simpler to store both ends here
                self.edges.push([a, 0]);
                self.surfedges.push(e);
            }
            let n = pts.len();
            let base = self.vertices.len() - n;
            for k in 0..n {
                let e = (self.edges.len() - n) + k;
                self.edges[e][1] = (base + (k + 1) % n) as u16;
            }
            // lightmap size from the extents, as the engine works it out
            let ti = self.texinfo[texinfo as usize];
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for p in pts {
                let (s, t) = ti.st(*p);
                lo = [lo[0].min(s), lo[1].min(t)];
                hi = [hi[0].max(s), hi[1].max(t)];
            }
            let w = ((hi[0] / 16.0).ceil() - (lo[0] / 16.0).floor()) as usize + 1;
            let h = ((hi[1] / 16.0).ceil() - (lo[1] / 16.0).floor()) as usize + 1;
            let light_ofs = self.lighting.len() as i32;
            self.lighting.extend(std::iter::repeat(light).take(w * h * 3));
            self.faces.push(Face { plane, side: 0, first_edge, num_edges: n as u16, texinfo, styles: [0, 255, 255, 255], light_ofs });
        }
    }

    /// A 16×16 miptex record with all four mips and a palette where index i is (i, i, i),
    /// pixels = (x + y) % 256.
    pub fn miptex(name: &str, embedded: bool) -> Vec<u8> {
        let (w, h) = (16u32, 16u32);
        let mut b = vec![0u8; 16];
        b[..name.len()].copy_from_slice(name.as_bytes());
        b.extend(w.to_le_bytes());
        b.extend(h.to_le_bytes());
        if !embedded {
            b.extend([0u8; 16]);
            return b;
        }
        let sizes = [w * h, w * h / 4, w * h / 16, w * h / 64];
        let mut at = 40u32;
        for s in sizes {
            b.extend(at.to_le_bytes());
            at += s;
        }
        for y in 0..h {
            for x in 0..w {
                b.push(((x + y) % 256) as u8);
            }
        }
        for s in &sizes[1..] {
            b.extend(std::iter::repeat(7u8).take(*s as usize));
        }
        b.extend(256u16.to_le_bytes());
        for i in 0..256 {
            b.extend([i as u8, i as u8, i as u8]);
        }
        b.extend([0u8; 2]);
        b
    }

    pub fn build() -> Vec<u8> {
        let mut bd = Builder { planes: vec![], vertices: vec![], edges: vec![[0, 0]], surfedges: vec![], faces: vec![], texinfo: vec![], lighting: vec![] };
        // texinfo 0: brick on walls facing ±x (s along y, t along -z)
        // texinfo 1: brick on walls facing ±y (s along x)
        // texinfo 2: {grate on floor/ceiling (s along x, t along -y)
        bd.texinfo.push(TexInfo { s: [0.0, 1.0, 0.0, 0.0], t: [0.0, 0.0, -1.0, 0.0], miptex: 0, flags: 0 });
        bd.texinfo.push(TexInfo { s: [1.0, 0.0, 0.0, 0.0], t: [0.0, 0.0, -1.0, 0.0], miptex: 0, flags: 0 });
        bd.texinfo.push(TexInfo { s: [1.0, 0.0, 0.0, 0.0], t: [0.0, -1.0, 0.0, 0.0], miptex: 1, flags: 0 });
        let a = 128.0;
        // floor (z=-a, faces up), ceiling (z=a, faces down)
        bd.face(&[[-a, -a, -a], [a, -a, -a], [a, a, -a], [-a, a, -a]], [0.0, 0.0, 1.0], 2, 100);
        bd.face(&[[-a, -a, a], [-a, a, a], [a, a, a], [a, -a, a]], [0.0, 0.0, -1.0], 2, 60);
        // walls x = -a (faces +x), x = a (faces -x)
        bd.face(&[[-a, -a, -a], [-a, a, -a], [-a, a, a], [-a, -a, a]], [1.0, 0.0, 0.0], 0, 80);
        bd.face(&[[a, -a, -a], [a, -a, a], [a, a, a], [a, a, -a]], [-1.0, 0.0, 0.0], 0, 80);
        // walls y = -a (faces +y), y = a (faces -y)
        bd.face(&[[-a, -a, -a], [-a, -a, a], [a, -a, a], [a, -a, -a]], [0.0, 1.0, 0.0], 1, 80);
        bd.face(&[[-a, a, -a], [a, a, -a], [a, a, a], [-a, a, a]], [0.0, -1.0, 0.0], 1, 80);
        // model 1: the func_wall's face
        bd.face(&[[-32.0, -32.0, 0.0], [32.0, -32.0, 0.0], [32.0, 32.0, 0.0], [-32.0, 32.0, 0.0]], [0.0, 0.0, 1.0], 1, 200);

        let mut lumps: Vec<Vec<u8>> = vec![Vec::new(); LUMPS];
        lumps[LUMP_ENTITIES] = ENTITIES.as_bytes().to_vec();
        lumps[LUMP_ENTITIES].push(0);
        for p in &bd.planes {
            let l = &mut lumps[LUMP_PLANES];
            p.normal.iter().for_each(|f| l.extend(f.to_le_bytes()));
            l.extend(p.dist.to_le_bytes());
            l.extend(p.kind.to_le_bytes());
        }
        // textures
        let texs = [miptex("brick", true), miptex("{grate", false)];
        let l = &mut lumps[LUMP_TEXTURES];
        l.extend((texs.len() as u32).to_le_bytes());
        let mut at = 4 + 4 * texs.len() as u32;
        for t in &texs {
            l.extend(at.to_le_bytes());
            at += t.len() as u32;
        }
        texs.iter().for_each(|t| l.extend(t));
        for v in &bd.vertices {
            v.iter().for_each(|f| lumps[LUMP_VERTICES].extend(f.to_le_bytes()));
        }
        for t in &bd.texinfo {
            let l = &mut lumps[LUMP_TEXINFO];
            t.s.iter().chain(t.t.iter()).for_each(|f| l.extend(f.to_le_bytes()));
            l.extend(t.miptex.to_le_bytes());
            l.extend(t.flags.to_le_bytes());
        }
        for f in &bd.faces {
            let l = &mut lumps[LUMP_FACES];
            l.extend(f.plane.to_le_bytes());
            l.extend(f.side.to_le_bytes());
            l.extend(f.first_edge.to_le_bytes());
            l.extend(f.num_edges.to_le_bytes());
            l.extend(f.texinfo.to_le_bytes());
            l.extend(f.styles);
            l.extend(f.light_ofs.to_le_bytes());
        }
        lumps[LUMP_LIGHTING] = bd.lighting.clone();
        for e in &bd.edges {
            lumps[LUMP_EDGES].extend(e[0].to_le_bytes());
            lumps[LUMP_EDGES].extend(e[1].to_le_bytes());
        }
        for s in &bd.surfedges {
            lumps[LUMP_SURFEDGES].extend(s.to_le_bytes());
        }
        let model = |mins: [f32; 3], maxs: [f32; 3], first: i32, num: i32| {
            let mut m = Vec::new();
            mins.iter().chain(maxs.iter()).chain([0.0f32; 3].iter()).for_each(|f| m.extend(f.to_le_bytes()));
            [0i32, -1, -1, 0, 1, first, num].iter().for_each(|i| m.extend(i.to_le_bytes()));
            m
        };
        lumps[LUMP_MODELS].extend(model([-a; 3], [a; 3], 0, 6));
        lumps[LUMP_MODELS].extend(model([-32.0, -32.0, 0.0], [32.0, 32.0, 0.0], 6, 1));

        let mut out = Vec::new();
        out.extend(VERSION.to_le_bytes());
        let mut at = HEADER_SIZE as u32;
        for l in &lumps {
            out.extend(at.to_le_bytes());
            out.extend((l.len() as u32).to_le_bytes());
            at += (l.len() as u32 + 3) & !3;
        }
        for l in &lumps {
            out.extend(l);
            while out.len() % 4 != 0 {
                out.push(0);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_cube_room() {
        let b = fixture::build();
        let bsp = Bsp::parse(&b, true).unwrap();
        assert_eq!(bsp.faces.len(), 7);
        assert_eq!(bsp.models.len(), 2);
        assert_eq!(bsp.models[1].first_face, 6);
        assert_eq!(bsp.textures.len(), 2);
        assert_eq!(bsp.textures[0].name, "brick");
        assert!(bsp.textures[0].embedded.as_ref().is_some_and(|e| e.len() > 40 + 256));
        assert_eq!(bsp.textures[1].name, "{grate");
        assert!(bsp.textures[1].embedded.is_none());
        assert!(bsp.entities.starts_with("{\n\"classname\" \"worldspawn\""));
        assert!(!bsp.entities.ends_with('\0'));
        let f = &bsp.faces[0];
        let pts = bsp.face_points(f).unwrap();
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0], [-128.0, -128.0, -128.0]);
        assert_eq!(pts[1], [128.0, -128.0, -128.0]);
        assert_eq!(bsp.face_normal(f), Some([0.0, 0.0, 1.0]));
        // without pixels the embedded texture is still marked as embedded
        let lite = Bsp::parse(&b, false).unwrap();
        assert_eq!(lite.textures[0].embedded.as_ref().map(|e| e.len()), Some(40));
    }

    #[test]
    fn reads_single_lumps_from_disk() {
        let dir = std::env::temp_dir().join(format!("ls-bsp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("room.bsp");
        std::fs::write(&p, fixture::build()).unwrap();
        let l = read_lumps(&p, &[LUMP_TEXTURES, LUMP_ENTITIES]).unwrap();
        assert_eq!(textures(&l[0], true).unwrap().len(), 2);
        assert!(l[1].starts_with(b"{"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn damaged_files_say_so() {
        let b = fixture::build();
        // too short, wrong version, a lump past the end, a cut-off file
        assert!(matches!(Bsp::parse(&b[..20], false), Err(Error::Damaged { .. })));
        let mut q = b.clone();
        q[0] = 29;
        let e = Bsp::parse(&q, false).unwrap_err();
        assert!(matches!(e, Error::NotFormat { .. }), "{e}");
        assert!(e.to_string().contains("Quake"));
        let mut bad = b.clone();
        let at = 4 + LUMP_FACES * 8 + 4;
        bad[at..at + 4].copy_from_slice(&(1u32 << 30).to_le_bytes());
        assert!(matches!(Bsp::parse(&bad, false), Err(Error::Damaged { .. })));
        // every truncation gives an error, never a panic
        for n in (0..b.len()).step_by(7) {
            assert!(Bsp::parse(&b[..n], true).is_err());
        }
        // garbage anywhere never panics (it may parse or fail)
        let mut g = b.clone();
        for i in (HEADER_SIZE..g.len()).step_by(3) {
            g[i] = g[i].wrapping_mul(31).wrapping_add(7);
        }
        let _ = Bsp::parse(&g, true).map(|bsp| bsp.faces.iter().map(|f| bsp.face_points(f)).count());
    }

    #[test]
    fn embedded_textures_copy_only_their_record() {
        // 2000 entries all pointing at the same 16x16 embedded record: each keeps one
        // record's worth, not the rest of the lump
        let rec = fixture::miptex("brick", true);
        let n = 2000usize;
        let mut lump = (n as u32).to_le_bytes().to_vec();
        let at = 4 + 4 * n as u32;
        for _ in 0..n {
            lump.extend(at.to_le_bytes());
        }
        lump.extend(&rec);
        lump.extend(vec![0u8; 1 << 20]);
        let t = textures(&lump, true).unwrap();
        assert_eq!(t.len(), n);
        let need = 40 + 256 * 85 / 64 + 2 + 768 + 2;
        assert!(t.iter().all(|t| t.embedded.as_ref().is_some_and(|e| e.len() == need)));
        assert!(crate::goldsrc::wad::decode_miptex(t[5].embedded.as_ref().unwrap(), "").is_ok());
        // oversized sides become blank textures
        let mut big = rec.clone();
        big[16..20].copy_from_slice(&2048u32.to_le_bytes());
        let mut lump = 1u32.to_le_bytes().to_vec();
        lump.extend(8u32.to_le_bytes());
        lump.extend(&big);
        let t = textures(&lump, true).unwrap();
        assert_eq!((t[0].width, t[0].embedded.is_none()), (16, true));
    }

    #[test]
    fn bad_face_references_are_skipped() {
        let bsp = Bsp::parse(&fixture::build(), false).unwrap();
        let mut f = bsp.faces[0];
        f.first_edge = 10_000;
        assert!(bsp.face_points(&f).is_none());
        f = bsp.faces[0];
        f.num_edges = 2;
        assert!(bsp.face_points(&f).is_none());
        f.texinfo = 99;
        assert!(bsp.face_texture(&f).is_none());
    }
}
