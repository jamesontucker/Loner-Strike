//! A map's faces as triangles, ready for the GPU, in the LSMB bundle (TOOLS-PLAN.md 3.3).
//!
//! Bundle layout (little-endian): `"LSMB"`, u32 version, u32 JSON length, the JSON header
//! (UTF-8, padded with spaces so the data starts 4-byte aligned), then the sections. Each
//! section in the header is `{name, offset, length, type, stride, count}`: `offset` counts
//! from the start of the bundle (so `new Float32Array(buf, offset, length / 4)` works),
//! `length` is in bytes, `stride` is components per element, `count` the elements.
//!
//! Sections: positions f32×3, normals f32×3, uv f32×2 (divided by the texture's size),
//! uv2 f32×2 (lightmap atlas, 0..1), indices u32 (into the vertex arrays, whole bundle),
//! faceOfTriangle u32 (BSP face index per triangle). All in GoldSrc units and axes.
//!
//! Draws (`draws` in the header) are ranges of `indices`: `{model, texture, start, count,
//! flags, entityIndex}` with start/count in indices (so `geometry.addGroup(start, count)`),
//! sorted by model, then texture name. `flags` is a bit set, named in `flagBits`.

use super::bsp::Bsp;
use super::entities::Entity;
use super::lightmap::{self, Atlas};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const BUNDLE_VERSION: u32 = 1;

pub const FLAG_SKY: u32 = 1;
pub const FLAG_WATER: u32 = 2;
pub const FLAG_TRIGGER: u32 = 4;
pub const FLAG_TRANSPARENT: u32 = 8;

/// Tool textures the compilers sometimes leave in (drawn only with the Triggers layer on).
const TOOL_TEXTURES: &[&str] = &["aaatrigger", "clip", "null", "origin", "hint", "skip", "bevel", "solidhint", "noclip"];

/// What a texture name (and the entity using it) says about how to draw it.
pub fn flags_for(texture: &str, classname: &str) -> u32 {
    let t = texture.to_lowercase();
    let mut f = 0;
    if t == "sky" {
        f |= FLAG_SKY;
    }
    if t.starts_with('!') || t.starts_with('*') || t.starts_with("water") {
        f |= FLAG_WATER;
    }
    if TOOL_TEXTURES.contains(&t.as_str()) || classname.starts_with("trigger_") {
        f |= FLAG_TRIGGER;
    }
    if t.starts_with('{') {
        f |= FLAG_TRANSPARENT;
    }
    f
}

/// Which way a polygon's corners go around `n`: positive = counter-clockwise seen from the
/// side `n` points to (Newell's method, fine for any convex face).
fn winding(pts: &[[f32; 3]], n: [f32; 3]) -> f32 {
    let mut a = [0.0f32; 3];
    for i in 0..pts.len() {
        let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
        a[0] += (p[1] - q[1]) * (p[2] + q[2]);
        a[1] += (p[2] - q[2]) * (p[0] + q[0]);
        a[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    a[0] * n[0] + a[1] * n[1] + a[2] * n[2]
}

/// The built mesh: the bundle's bytes and the lightmap atlas made with it (its uv2 point
/// into this atlas, so the two are cached together).
pub struct Built {
    pub bundle: Vec<u8>,
    pub atlas: Atlas,
    pub triangles: usize,
}

struct Draw {
    model: usize,
    texture: usize,
    start: usize,
    count: usize,
    flags: u32,
    entity: Option<usize>,
}

/// Turns the map into the bundle. Faces that point outside the map's lists are skipped;
/// a map with nothing drawable at all is an error.
pub fn build(bsp: &Bsp, ents: &[Entity], map: &str) -> super::Result<Built> {
    // which entity uses each brush model (worldspawn is model 0)
    let mut owner: BTreeMap<usize, usize> = BTreeMap::new();
    if ents.first().is_some_and(|e| e.classname() == "worldspawn") {
        owner.insert(0, 0);
    }
    for (i, e) in ents.iter().enumerate() {
        if let Some(m) = e.brush_model().filter(|m| *m > 0) {
            owner.entry(m).or_insert(i);
        }
    }

    // faces per model, grouped by texture (by name, then index, for a stable order)
    let mut groups: Vec<(usize, Vec<(usize, Vec<usize>)>)> = Vec::new();
    for (mi, m) in bsp.models.iter().enumerate() {
        if m.first_face < 0 || m.num_faces <= 0 {
            continue;
        }
        let (first, n) = (m.first_face as usize, m.num_faces as usize);
        let Some(end) = first.checked_add(n).filter(|&e| e <= bsp.faces.len()) else { continue };
        let mut by_tex: BTreeMap<(String, usize), Vec<usize>> = BTreeMap::new();
        for fi in first..end {
            if let Some((_, t)) = bsp.face_texture(&bsp.faces[fi]) {
                by_tex.entry((bsp.textures[t].name.to_lowercase(), t)).or_default().push(fi);
            }
        }
        groups.push((mi, by_tex.into_iter().map(|((_, t), f)| (t, f)).collect()));
    }
    let all: Vec<usize> = groups.iter().flat_map(|(_, g)| g.iter().flat_map(|(_, f)| f.iter().copied())).collect();
    let atlas = lightmap::pack(bsp, &all);

    let mut pos: Vec<f32> = Vec::new();
    let mut nrm: Vec<f32> = Vec::new();
    let mut uv: Vec<f32> = Vec::new();
    let mut uv2: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut tri_face: Vec<u32> = Vec::new();
    let mut draws: Vec<Draw> = Vec::new();
    let mut model_ranges: Vec<(usize, usize, usize)> = Vec::new();
    for (mi, texs) in &groups {
        let m_start = idx.len();
        let entity = owner.get(mi).copied();
        let classname = entity.and_then(|e| ents.get(e)).map(|e| e.classname()).unwrap_or("");
        for (t, faces) in texs {
            let tex = &bsp.textures[*t];
            let (tw, th) = (tex.width.max(1) as f32, tex.height.max(1) as f32);
            let start = idx.len();
            for &fi in faces {
                let f = &bsp.faces[fi];
                let (Some(pts), Some(n), Some((ti, _))) = (bsp.face_points(f), bsp.face_normal(f), bsp.face_texture(f)) else { continue };
                if pts.iter().flatten().any(|c| !c.is_finite()) {
                    continue;
                }
                let base = (pos.len() / 3) as u32;
                for p in &pts {
                    let (s, tt) = ti.st(*p);
                    pos.extend(p);
                    nrm.extend(n);
                    uv.extend([s / tw, tt / th]);
                    uv2.extend(atlas.uv2(fi, s, tt));
                }
                // triangles counter-clockwise seen from the front (three.js's front side),
                // whichever way the map stored the corners
                let flip = winding(&pts, n) < 0.0;
                for k in 1..pts.len() as u32 - 1 {
                    if flip {
                        idx.extend([base, base + k + 1, base + k]);
                    } else {
                        idx.extend([base, base + k, base + k + 1]);
                    }
                    tri_face.push(fi as u32);
                }
            }
            if idx.len() > start {
                draws.push(Draw { model: *mi, texture: *t, start, count: idx.len() - start, flags: flags_for(&tex.name, classname), entity });
            }
        }
        if idx.len() > m_start {
            model_ranges.push((*mi, m_start, idx.len() - m_start));
        }
    }
    if idx.is_empty() {
        return Err(super::Error::damaged("map file", "it has no faces to draw"));
    }

    let flag_names = json!({"sky": FLAG_SKY, "water": FLAG_WATER, "trigger": FLAG_TRIGGER, "transparent": FLAG_TRANSPARENT});
    let draws_json: Vec<Value> = draws
        .iter()
        .map(|d| json!({"model": d.model, "texture": bsp.textures[d.texture].name, "start": d.start, "count": d.count, "flags": d.flags, "entityIndex": d.entity}))
        .collect();
    let models_json: Vec<Value> = model_ranges
        .iter()
        .map(|&(mi, start, count)| {
            let m = &bsp.models[mi];
            let e = owner.get(&mi).and_then(|&i| ents.get(i).map(|e| (i, e)));
            let mut o = json!({"index": mi, "start": start, "count": count, "mins": m.mins, "maxs": m.maxs,
                               "entityIndex": e.map(|(i, _)| i), "classname": e.map(|(_, e)| e.classname()),
                               "origin": e.and_then(|(_, e)| if mi == 0 { None } else { e.origin() }).unwrap_or([0.0; 3])});
            if let Some((_, e)) = e {
                if let Some(r) = e.get("rendermode").and_then(|v| v.trim().parse::<i32>().ok()) {
                    o["renderMode"] = json!(r);
                }
                if let Some(a) = e.get("renderamt").and_then(|v| v.trim().parse::<i32>().ok()) {
                    o["renderAmt"] = json!(a);
                }
            }
            o
        })
        .collect();
    let textures_json: Vec<Value> = bsp
        .textures
        .iter()
        .map(|t| json!({"name": t.name, "width": t.width, "height": t.height, "embedded": t.embedded.is_some()}))
        .collect();
    let world = &bsp.models[0];
    let header = json!({
        "format": "LSMB", "version": BUNDLE_VERSION, "parser": super::PARSER_VERSION, "map": map,
        "bounds": {"mins": world.mins, "maxs": world.maxs},
        "vertexCount": pos.len() / 3, "indexCount": idx.len(), "triangleCount": tri_face.len(),
        "draws": draws_json, "models": models_json, "textures": textures_json,
        "lightmap": {"width": atlas.width, "height": atlas.height},
        "flagBits": flag_names,
    });
    let sections = vec![
        Section::f32("positions", 3, &pos),
        Section::f32("normals", 3, &nrm),
        Section::f32("uv", 2, &uv),
        Section::f32("uv2", 2, &uv2),
        Section::u32("indices", 1, &idx),
        Section::u32("faceOfTriangle", 1, &tri_face),
    ];
    let triangles = tri_face.len();
    Ok(Built { bundle: write_bundle(header, &sections), atlas, triangles })
}

/// One raw section of a bundle.
pub struct Section {
    pub name: &'static str,
    pub kind: &'static str,
    pub stride: usize,
    pub bytes: Vec<u8>,
}

impl Section {
    pub fn f32(name: &'static str, stride: usize, v: &[f32]) -> Section {
        Section { name, kind: "f32", stride, bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
    }
    pub fn u32(name: &'static str, stride: usize, v: &[u32]) -> Section {
        Section { name, kind: "u32", stride, bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
    }
    pub fn u16(name: &'static str, stride: usize, v: &[u16]) -> Section {
        Section { name, kind: "u16", stride, bytes: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
    }
    pub fn u8(name: &'static str, stride: usize, v: &[u8]) -> Section {
        Section { name, kind: "u8", stride, bytes: v.to_vec() }
    }
    fn elem(&self) -> usize {
        match self.kind {
            "f32" | "u32" => 4,
            "u16" => 2,
            _ => 1,
        }
    }
}

/// Writes an LSMB bundle: `header` gets a `sections` list with absolute offsets. Shared
/// with the other bundles (nav knowledge, replays) so they decode the same way.
pub fn write_bundle(mut header: Value, sections: &[Section]) -> Vec<u8> {
    let align = |n: usize| (n + 3) & !3;
    // offsets depend on the header's length, which depends on the offsets: grow the guess
    // until the header fits in front of the data
    let mut data_start = 12 + 256;
    loop {
        let mut at = data_start;
        let list: Vec<Value> = sections
            .iter()
            .map(|s| {
                let o = json!({"name": s.name, "offset": at, "length": s.bytes.len(), "type": s.kind, "stride": s.stride,
                               "count": s.bytes.len() / s.elem() / s.stride.max(1)});
                at = align(at + s.bytes.len());
                o
            })
            .collect();
        header["sections"] = Value::Array(list);
        let text = serde_json::to_string(&header).unwrap_or_default();
        if 12 + text.len() <= data_start {
            let mut out = Vec::with_capacity(at);
            out.extend(b"LSMB");
            out.extend(BUNDLE_VERSION.to_le_bytes());
            out.extend(((data_start - 12) as u32).to_le_bytes());
            out.extend(text.as_bytes());
            out.resize(data_start, b' ');
            for s in sections {
                out.extend(&s.bytes);
                out.resize(align(out.len()), 0);
            }
            return out;
        }
        data_start = align(12 + text.len() + 64);
    }
}

/// Reads a bundle back (tests; the page has its own decoder).
#[cfg(test)]
pub fn read_bundle(b: &[u8]) -> (Value, std::collections::HashMap<String, Vec<u8>>) {
    assert_eq!(&b[0..4], b"LSMB");
    let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let header: Value = serde_json::from_slice(b[12..12 + n].trim_ascii_end()).unwrap();
    let mut out = std::collections::HashMap::new();
    for s in header["sections"].as_array().unwrap() {
        let (o, l) = (s["offset"].as_u64().unwrap() as usize, s["length"].as_u64().unwrap() as usize);
        assert_eq!(o % 4, 0);
        out.insert(s["name"].as_str().unwrap().to_string(), b[o..o + l].to_vec());
    }
    (header, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::{bsp::fixture, entities};

    fn f32s(b: &[u8]) -> Vec<f32> {
        b.chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
    }
    fn u32s(b: &[u8]) -> Vec<u32> {
        b.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
    }

    #[test]
    fn flags() {
        assert_eq!(flags_for("SKY", "worldspawn"), FLAG_SKY);
        assert_eq!(flags_for("!waterblue", ""), FLAG_WATER);
        assert_eq!(flags_for("AAATRIGGER", ""), FLAG_TRIGGER);
        assert_eq!(flags_for("brick", "trigger_hurt"), FLAG_TRIGGER);
        assert_eq!(flags_for("{fence", "func_wall"), FLAG_TRANSPARENT);
        assert_eq!(flags_for("skybox_wall", ""), 0);
    }

    #[test]
    fn cube_room_bundle() {
        let bsp = Bsp::parse(&fixture::build(), false).unwrap();
        let ents = entities::parse(&bsp.entities);
        let built = build(&bsp, &ents, "room").unwrap();
        assert_eq!(built.triangles, 14);
        let (h, s) = read_bundle(&built.bundle);
        assert_eq!(h["version"], 1);
        assert_eq!(h["vertexCount"], 28);
        assert_eq!(h["indexCount"], 42);
        assert_eq!(h["lightmap"]["width"], built.atlas.width);
        let pos = f32s(&s["positions"]);
        let nrm = f32s(&s["normals"]);
        let uv = f32s(&s["uv"]);
        let uv2 = f32s(&s["uv2"]);
        let idx = u32s(&s["indices"]);
        let tf = u32s(&s["faceOfTriangle"]);
        assert_eq!((pos.len(), nrm.len(), uv.len(), uv2.len(), idx.len(), tf.len()), (84, 84, 56, 56, 42, 14));
        assert!(idx.iter().all(|&i| (i as usize) < 28));
        assert!(uv2.iter().all(|&v| (0.0..=1.0).contains(&v)));
        // draws: world brick (4 walls), world {grate (floor + ceiling), model 1 brick
        let d = h["draws"].as_array().unwrap();
        assert_eq!(d.len(), 3);
        assert_eq!((d[0]["model"].as_u64(), d[0]["texture"].as_str(), d[0]["count"].as_u64()), (Some(0), Some("brick"), Some(24)));
        assert_eq!((d[1]["texture"].as_str(), d[1]["start"].as_u64(), d[1]["count"].as_u64()), (Some("{grate"), Some(24), Some(12)));
        assert_eq!(d[1]["flags"].as_u64(), Some(FLAG_TRANSPARENT as u64));
        assert_eq!((d[2]["model"].as_u64(), d[2]["entityIndex"].as_u64(), d[2]["start"].as_u64()), (Some(1), Some(1), Some(36)));
        let m = h["models"].as_array().unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[1]["classname"], "func_wall");
        assert_eq!(m[1]["renderMode"], 4);
        assert_eq!(m[1]["renderAmt"], 200);
        assert_eq!((m[1]["start"].as_u64(), m[1]["count"].as_u64()), (Some(36), Some(6)));
        // the walls' triangles come from faces 2..5, the floor's from 0
        assert!(tf[0..8].iter().all(|f| (2..6).contains(f)));
        assert!(tf[8..12].iter().all(|f| [0, 1].contains(f)));
        assert_eq!(tf[12..14], [6, 6]);
        // first vertex of the floor: (-128,-128,-128), normal up, uv = (s/16, t/16) = (-8, 8)
        let v0 = idx[24] as usize;
        assert_eq!(&pos[v0 * 3..v0 * 3 + 3], &[-128.0, -128.0, -128.0]);
        assert_eq!(&nrm[v0 * 3..v0 * 3 + 3], &[0.0, 0.0, 1.0]);
        assert_eq!(&uv[v0 * 2..v0 * 2 + 2], &[-8.0, 8.0]);
        // every triangle is counter-clockwise seen from the side its normal points to
        let mut signs = Vec::new();
        for t in 0..14 {
            let p = |k: usize| {
                let i = idx[t * 3 + k] as usize;
                [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]]
            };
            let (a, b, c) = (p(0), p(1), p(2));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cr = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let i = idx[t * 3] as usize;
            signs.push((cr[0] * nrm[i * 3] + cr[1] * nrm[i * 3 + 1] + cr[2] * nrm[i * 3 + 2]).signum());
        }
        assert!(signs.iter().all(|s| *s > 0.0), "{signs:?}");
    }

    #[test]
    fn bundle_alignment_with_odd_sections() {
        let b = write_bundle(json!({"x": 1}), &[Section::u8("a", 1, &[1, 2, 3]), Section::u16("b", 1, &[7]), Section::f32("c", 2, &[1.0, 2.0])]);
        let (h, s) = read_bundle(&b);
        assert_eq!(s["a"], vec![1, 2, 3]);
        assert_eq!(s["b"], vec![7, 0]);
        assert_eq!(h["sections"][2]["count"], 1);
        assert_eq!((12 + u32::from_le_bytes(b[8..12].try_into().unwrap())) % 4, 0);
        // a header too long for the first guess still fits
        let big = json!({"pad": "x".repeat(5000)});
        let (h, s) = read_bundle(&write_bundle(big, &[Section::u32("i", 1, &[42])]));
        assert_eq!(h["pad"].as_str().unwrap().len(), 5000);
        assert_eq!(s["i"], 42u32.to_le_bytes().to_vec());
    }

    #[test]
    fn nothing_to_draw_is_an_error() {
        let mut bsp = Bsp::parse(&fixture::build(), false).unwrap();
        bsp.models[0].num_faces = 0;
        bsp.models[1].first_face = 1000;
        assert!(build(&bsp, &[], "room").is_err());
    }
}
