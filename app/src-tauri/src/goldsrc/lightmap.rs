//! Lightmaps: each lit face has a small RGB picture, one sample ("luxel") per 16 texture
//! units, stored in the lighting lump. We pack the style-0 pictures into one atlas (1 px of
//! repeated edge around each, so filtering doesn't bleed) and give every vertex a `uv2`.
//!
//! Extents, as the engine works them out: s/t of each corner (texinfo), then
//! `lo = floor(min/16)`, `hi = ceil(max/16)`, size = `hi - lo + 1` luxels per side. Luxel i's
//! centre sits at texture coordinate `(lo + i) * 16`.

use super::bsp::{Bsp, Face};

pub const LUXEL: f32 = 16.0;
/// Bigger than any face the compilers make (they split faces at 256 units → 17 luxels);
/// a face claiming more is treated as unlit.
const MAX_SIDE: u32 = 64;
const PAD: u32 = 1;
const MAX_ATLAS: u32 = 4096;
/// A white block in the corner for faces without a lightmap (the engine draws those
/// full-bright).
const WHITE: u32 = 4;

/// One face's lightmap size and where its luxel grid starts in texture space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extents {
    pub lo: [f32; 2],
    pub w: u32,
    pub h: u32,
}

/// The lightmap extents of a face from its corners' texture coordinates.
pub fn extents(st: &[(f32, f32)]) -> Option<Extents> {
    if st.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for &(s, t) in st {
        // doubles, like the engine, so a corner sitting exactly on a luxel line rounds the same
        lo = [lo[0].min(s as f64), lo[1].min(t as f64)];
        hi = [hi[0].max(s as f64), hi[1].max(t as f64)];
    }
    let l = [(lo[0] / 16.0).floor(), (lo[1] / 16.0).floor()];
    let h = [(hi[0] / 16.0).ceil(), (hi[1] / 16.0).ceil()];
    let (w, hh) = (h[0] - l[0] + 1.0, h[1] - l[1] + 1.0);
    if !(w.is_finite() && hh.is_finite()) || w < 1.0 || hh < 1.0 || w > MAX_SIDE as f64 || hh > MAX_SIDE as f64 {
        return None;
    }
    Some(Extents { lo: [l[0] as f32, l[1] as f32], w: w as u32, h: hh as u32 })
}

/// Where a face's lightmap went in the atlas (its luxels start at x, y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub x: u32,
    pub y: u32,
    pub ext: Extents,
}

/// The packed atlas: RGB rows, and per face (by BSP face index) where it is.
#[derive(Debug, Clone)]
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub placed: std::collections::HashMap<usize, Placed>,
}

impl Atlas {
    /// A vertex's atlas coordinate (0..1) from its texture-space (s, t). Faces without a
    /// lightmap get the white block's middle.
    pub fn uv2(&self, face: usize, s: f32, t: f32) -> [f32; 2] {
        let (w, h) = (self.width as f32, self.height as f32);
        match self.placed.get(&face) {
            Some(p) => [
                (p.x as f32 + 0.5 + s / LUXEL - p.ext.lo[0]) / w,
                (p.y as f32 + 0.5 + t / LUXEL - p.ext.lo[1]) / h,
            ],
            None => [(WHITE as f32 / 2.0) / w, (WHITE as f32 / 2.0) / h],
        }
    }
}

/// The style-0 lightmap of a face, if it has one that fits in the lighting lump.
fn samples<'a>(bsp: &'a Bsp, f: &Face, ext: &Extents) -> Option<&'a [u8]> {
    if f.styles[0] == 255 || f.light_ofs < 0 {
        return None;
    }
    let at = f.light_ofs as usize;
    let n = (ext.w * ext.h * 3) as usize;
    bsp.lighting.get(at..at.checked_add(n)?)
}

/// Packs the lightmaps of `faces` (BSP face indices) into an atlas. Shelf packing, tallest
/// first; the atlas is a power of two wide and high (WebGL mipmaps), at most 4096 wide. Faces
/// that don't fit (only on absurd maps) or have no lightmap use the white block.
pub fn pack(bsp: &Bsp, faces: &[usize]) -> Atlas {
    struct Item<'a> {
        face: usize,
        ext: Extents,
        data: &'a [u8],
    }
    let mut items: Vec<Item> = Vec::new();
    for &i in faces {
        let Some(f) = bsp.faces.get(i) else { continue };
        let Some((ti, _)) = bsp.face_texture(f) else { continue };
        let Some(pts) = bsp.face_points(f) else { continue };
        let st: Vec<(f32, f32)> = pts.iter().map(|p| ti.st(*p)).collect();
        let Some(ext) = extents(&st) else { continue };
        if let Some(data) = samples(bsp, f, &ext) {
            items.push(Item { face: i, ext, data });
        }
    }
    items.sort_by(|a, b| b.ext.h.cmp(&a.ext.h).then(b.ext.w.cmp(&a.ext.w)).then(a.face.cmp(&b.face)));
    let area: u64 = items.iter().map(|it| ((it.ext.w + 2 * PAD) * (it.ext.h + 2 * PAD)) as u64).sum::<u64>() + (WHITE * WHITE) as u64;
    let mut width = 256u32;
    while width < MAX_ATLAS && (width as u64 * width as u64) < area * 5 / 4 {
        width *= 2;
    }
    // shelves: the white block starts the first one
    let (mut x, mut y, mut shelf) = (WHITE, 0u32, WHITE);
    let mut spots: Vec<(usize, u32, u32)> = Vec::with_capacity(items.len());
    for (k, it) in items.iter().enumerate() {
        let (w, h) = (it.ext.w + 2 * PAD, it.ext.h + 2 * PAD);
        if x + w > width {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        if y + h > MAX_ATLAS * 2 {
            break; // out of room: the rest stay white
        }
        spots.push((k, x, y));
        x += w;
        shelf = shelf.max(h);
    }
    let used = (y + shelf).max(WHITE);
    let height = used.next_power_of_two();
    let mut rgb = vec![0u8; (width * height * 3) as usize];
    for py in 0..WHITE {
        for px in 0..WHITE {
            let o = ((py * width + px) * 3) as usize;
            rgb[o..o + 3].copy_from_slice(&[255, 255, 255]);
        }
    }
    let mut placed = std::collections::HashMap::with_capacity(spots.len());
    for (k, x0, y0) in spots {
        let it = &items[k];
        let (w, h) = (it.ext.w, it.ext.h);
        // copy with the border: each padded pixel takes its nearest luxel
        for py in 0..h + 2 * PAD {
            let sy = py.saturating_sub(PAD).min(h - 1);
            for px in 0..w + 2 * PAD {
                let sx = px.saturating_sub(PAD).min(w - 1);
                let s = ((sy * w + sx) * 3) as usize;
                let o = (((y0 + py) * width + x0 + px) * 3) as usize;
                rgb[o..o + 3].copy_from_slice(&it.data[s..s + 3]);
            }
        }
        placed.insert(it.face, Placed { x: x0 + PAD, y: y0 + PAD, ext: it.ext });
    }
    Atlas { width, height, rgb, placed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture;

    #[test]
    fn extents_match_the_engine() {
        // 0..256 → luxels 0..16 → 17 wide
        let e = extents(&[(0.0, 0.0), (256.0, 0.0), (256.0, 64.0)]).unwrap();
        assert_eq!((e.lo, e.w, e.h), ([0.0, 0.0], 17, 5));
        // -8..8 → floor(-0.5) = -1, ceil(0.5) = 1 → 3
        let e = extents(&[(-8.0, -8.0), (8.0, 8.0)]).unwrap();
        assert_eq!((e.lo, e.w, e.h), ([-1.0, -1.0], 3, 3));
        assert!(extents(&[(0.0, 0.0), (5000.0, 0.0)]).is_none());
        assert!(extents(&[(f32::NAN, 0.0)]).is_none());
        assert!(extents(&[]).is_none());
    }

    #[test]
    fn packs_the_cube_room() {
        let bsp = Bsp::parse(&fixture::build(), false).unwrap();
        let all: Vec<usize> = (0..bsp.faces.len()).collect();
        let a = pack(&bsp, &all);
        assert_eq!(a.placed.len(), 7);
        assert!(a.width.is_power_of_two() && a.height.is_power_of_two());
        assert_eq!(a.rgb.len(), (a.width * a.height * 3) as usize);
        // the white block
        assert_eq!(&a.rgb[0..3], &[255, 255, 255]);
        // the floor (light 100) and its padding
        let p = a.placed[&0];
        let px = |x: u32, y: u32| a.rgb[((y * a.width + x) * 3) as usize];
        assert_eq!(px(p.x, p.y), 100);
        assert_eq!(px(p.x - 1, p.y - 1), 100);
        assert_eq!(px(p.x + p.ext.w, p.y + p.ext.h), 100);
        assert_eq!((p.ext.w, p.ext.h), (17, 17));
        // no two faces overlap (with padding)
        let rects: Vec<_> = a.placed.values().map(|p| (p.x - 1, p.y - 1, p.x + p.ext.w + 1, p.y + p.ext.h + 1)).collect();
        for (i, r) in rects.iter().enumerate() {
            assert!(r.0 >= WHITE || r.1 >= WHITE, "over the white block");
            for q in &rects[i + 1..] {
                assert!(r.2 <= q.0 || q.2 <= r.0 || r.3 <= q.1 || q.3 <= r.1, "{r:?} {q:?}");
            }
        }
        // a corner's uv2 lands on the middle of its luxel
        let uv = a.uv2(0, -128.0, 128.0);
        let s_lo = p.ext.lo[0];
        assert_eq!(uv[0], (p.x as f32 + 0.5 + -8.0 - s_lo) / a.width as f32);
        // unlit faces point at the white block
        assert_eq!(a.uv2(99, 0.0, 0.0), [2.0 / a.width as f32, 2.0 / a.height as f32]);
    }

    #[test]
    fn short_lighting_leaves_faces_white() {
        let mut bsp = Bsp::parse(&fixture::build(), false).unwrap();
        bsp.lighting.truncate(10);
        let a = pack(&bsp, &[0, 1]);
        assert!(a.placed.is_empty());
        bsp.faces[0].styles[0] = 255;
        assert!(pack(&bsp, &[0]).placed.is_empty());
    }
}
