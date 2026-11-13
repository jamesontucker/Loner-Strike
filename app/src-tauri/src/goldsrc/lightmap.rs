//! Lightmaps: each lit face has a small RGB picture, one sample ("luxel") per 16 texture
//! units, stored in the lighting lump. We pack them into one atlas (1 px of repeated edge
//! around each, so filtering doesn't bleed) and give every vertex a `uv2`.
//!
//! Light styles: a face can have up to four pictures, one per light style it's lit by
//! (`styles[0..4]`, 255 = no more), stored one after another. The game adds them up every
//! frame, each scaled by its style's current brightness (flicker, pulse, switched off). So
//! the atlas has *planes*, stacked top to bottom, each `height` rows: plane k holds every
//! face's k-th picture (black where a face has fewer), then one plane of style numbers
//! (R, G, B = styles 0, 1, 2 of the face at that spot) and, only when some face has four,
//! one more (R = style 3). The viewer's shader does the adding. See docs/TOOLS-RENDER.md.
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
    /// the height of one plane (what `uv2` is relative to)
    pub height: u32,
    /// pictures per face at most (1-4): planes 0..maps hold them
    pub maps: u32,
    /// all planes: picture planes, then the style-number plane(s)
    pub planes: u32,
    /// `width * height * planes` RGB pixels
    pub rgb: Vec<u8>,
    pub placed: std::collections::HashMap<usize, Placed>,
}

impl Atlas {
    /// The height of the whole picture (every plane).
    pub fn full_height(&self) -> u32 {
        self.height * self.planes
    }

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

/// The planes needed for `maps` pictures per face: the pictures, then one plane of style
/// numbers (two when a face can have four styles).
pub fn planes_for(maps: u32) -> u32 {
    maps + if maps >= 4 { 2 } else { 1 }
}

/// A face's lightmaps (one per style, back to back) and their styles, as many as fit in the
/// lighting lump; None if it has none.
fn samples<'a>(bsp: &'a Bsp, f: &Face, ext: &Extents) -> Option<(&'a [u8], [u8; 4], u32)> {
    if f.styles[0] == 255 || f.light_ofs < 0 {
        return None;
    }
    let at = f.light_ofs as usize;
    let one = (ext.w * ext.h * 3) as usize;
    let want = f.styles.iter().take_while(|&&s| s != 255).count();
    // a damaged face may claim more pictures than the lump holds: keep the ones that are there
    let have = (1..=want).rev().find(|&n| at.checked_add(n * one).is_some_and(|e| e <= bsp.lighting.len()))?;
    let mut styles = [255u8; 4];
    styles[..have].copy_from_slice(&f.styles[..have]);
    Some((&bsp.lighting[at..at + have * one], styles, have as u32))
}

/// Packs the lightmaps of `faces` (BSP face indices) into an atlas. Shelf packing, tallest
/// first; the atlas is a power of two wide and high (WebGL mipmaps), at most 4096 wide. Faces
/// that don't fit (only on absurd maps) or have no lightmap use the white block.
pub fn pack(bsp: &Bsp, faces: &[usize]) -> Atlas {
    struct Item<'a> {
        face: usize,
        ext: Extents,
        data: &'a [u8],
        styles: [u8; 4],
        maps: u32,
    }
    let mut items: Vec<Item> = Vec::new();
    for &i in faces {
        let Some(f) = bsp.faces.get(i) else { continue };
        let Some((ti, _)) = bsp.face_texture(f) else { continue };
        let Some(pts) = bsp.face_points(f) else { continue };
        let st: Vec<(f32, f32)> = pts.iter().map(|p| ti.st(*p)).collect();
        let Some(ext) = extents(&st) else { continue };
        if let Some((data, styles, maps)) = samples(bsp, f, &ext) {
            items.push(Item { face: i, ext, data, styles, maps });
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
    let maps = spots.iter().map(|&(k, _, _)| items[k].maps).max().unwrap_or(1).clamp(1, 4);
    let planes = planes_for(maps);
    let plane = (width * height * 3) as usize;
    let mut rgb = vec![0u8; plane * planes as usize];
    // the white block: full light, style 0 only
    let ids = maps as usize * plane;
    for py in 0..WHITE {
        for px in 0..WHITE {
            let o = ((py * width + px) * 3) as usize;
            rgb[o..o + 3].copy_from_slice(&[255, 255, 255]);
            rgb[ids + o..ids + o + 3].copy_from_slice(&[0, 255, 255]);
            if maps >= 4 {
                rgb[ids + plane + o] = 255;
            }
        }
    }
    let mut placed = std::collections::HashMap::with_capacity(spots.len());
    for (k, x0, y0) in spots {
        let it = &items[k];
        let (w, h) = (it.ext.w, it.ext.h);
        let one = (w * h * 3) as usize;
        let st = it.styles;
        // copy with the border: each padded pixel takes its nearest luxel
        for py in 0..h + 2 * PAD {
            let sy = py.saturating_sub(PAD).min(h - 1);
            for px in 0..w + 2 * PAD {
                let sx = px.saturating_sub(PAD).min(w - 1);
                let s = ((sy * w + sx) * 3) as usize;
                let o = (((y0 + py) * width + x0 + px) * 3) as usize;
                for m in 0..it.maps as usize {
                    let at = m * plane + o;
                    rgb[at..at + 3].copy_from_slice(&it.data[m * one + s..m * one + s + 3]);
                }
                rgb[ids + o..ids + o + 3].copy_from_slice(&st[..3]);
                if maps >= 4 {
                    rgb[ids + plane + o] = st[3];
                }
            }
        }
        placed.insert(it.face, Placed { x: x0 + PAD, y: y0 + PAD, ext: it.ext });
    }
    Atlas { width, height, maps, planes, rgb, placed }
}

/// The brightness of light style `'m'` (normal light): letters go `(c - 'a') * 22`.
pub const STYLE_NORMAL: u32 = 264;

/// What a ray from `start` to `end` first hits among `faces`: (face, fraction along, point).
/// Faces are tried by their plane and polygon, from either side (as the game's light trace).
pub fn first_hit(bsp: &Bsp, faces: impl Iterator<Item = usize>, start: [f32; 3], end: [f32; 3], skip: impl Fn(usize) -> bool) -> Option<(usize, f32, [f32; 3])> {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut best: Option<(usize, f32, [f32; 3])> = None;
    for fi in faces {
        let Some(f) = bsp.faces.get(fi) else { continue };
        let Some(pl) = bsp.planes.get(f.plane as usize) else { continue };
        let (d0, d1) = (dot(pl.normal, start) - pl.dist, dot(pl.normal, end) - pl.dist);
        if (d0 >= 0.0) == (d1 >= 0.0) || d0 == d1 {
            continue;
        }
        let frac = d0 / (d0 - d1);
        if best.is_some_and(|b| b.1 <= frac) || skip(fi) {
            continue;
        }
        let p = [start[0] + (end[0] - start[0]) * frac, start[1] + (end[1] - start[1]) * frac, start[2] + (end[2] - start[2]) * frac];
        let Some(pts) = bsp.face_points(f) else { continue };
        // inside the (convex) polygon: on the same side of every edge
        let n = pl.normal;
        let mut sign = 0f32;
        let inside = (0..pts.len()).all(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let q = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
            let c = [e[1] * q[2] - e[2] * q[1], e[2] * q[0] - e[0] * q[2], e[0] * q[1] - e[1] * q[0]];
            let side = dot(c, n);
            if side.abs() < 1e-3 {
                return true;
            }
            if sign == 0.0 {
                sign = side.signum();
            }
            side.signum() == sign
        });
        if inside {
            best = Some((fi, frac, p));
        }
    }
    best
}

/// The light at the spot a ray from `start` to `end` first meets a lit surface of the world,
/// as the game samples it for models standing there: the nearest luxel of each of the face's
/// styles, scaled by the style's brightness (`style(n)`, 264 = normal), added up, `>> 8` and
/// capped at 255. None when the ray meets nothing; black for a face without a lightmap.
pub fn light_point(bsp: &Bsp, start: [f32; 3], end: [f32; 3], style: impl Fn(u8) -> u32) -> Option<[u32; 3]> {
    let world = bsp.models.first()?;
    let (first, n) = (world.first_face.max(0) as usize, world.num_faces.max(0) as usize);
    let faces = first..(first + n).min(bsp.faces.len());
    // faces with no lightmap of their own (sky, water, tool textures: "special") aren't tried
    let special = |fi: usize| bsp.faces.get(fi).and_then(|f| bsp.texinfo.get(f.texinfo as usize)).map_or(true, |ti| ti.flags & 1 != 0);
    let mut faces: Vec<usize> = faces.collect();
    loop {
        let (fi, _, p) = first_hit(bsp, faces.iter().copied(), start, end, special)?;
        let f = &bsp.faces[fi];
        let (ti, _) = bsp.face_texture(f)?;
        let pts = bsp.face_points(f)?;
        let st: Vec<(f32, f32)> = pts.iter().map(|q| ti.st(*q)).collect();
        let Some(ext) = extents(&st) else {
            faces.retain(|&x| x != fi);
            continue;
        };
        let (s, t) = ti.st(p);
        let (ds, dt) = (s / LUXEL - ext.lo[0], t / LUXEL - ext.lo[1]);
        if ds < -0.01 || dt < -0.01 || ds > (ext.w - 1) as f32 + 0.01 || dt > (ext.h - 1) as f32 + 0.01 {
            // outside its light grid (the game skips it and keeps looking)
            faces.retain(|&x| x != fi);
            continue;
        }
        let Some((data, styles, maps)) = samples(bsp, f, &ext) else { return Some([0, 0, 0]) };
        let (x, y) = ((ds.round().max(0.0) as u32).min(ext.w - 1), (dt.round().max(0.0) as u32).min(ext.h - 1));
        let one = (ext.w * ext.h * 3) as usize;
        let at = ((y * ext.w + x) * 3) as usize;
        let mut sum = [0u32; 3];
        for m in 0..maps as usize {
            let v = style(styles[m]);
            for c in 0..3 {
                sum[c] += data[m * one + at + c] as u32 * v;
            }
        }
        return Some(sum.map(|c| (c >> 8).min(255)));
    }
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
        assert_eq!((a.maps, a.planes), (1, 2));
        assert_eq!(a.rgb.len(), (a.width * a.height * 3 * 2) as usize);
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
    fn styles_go_into_their_own_planes() {
        let mut bsp = Bsp::parse(&fixture::build(), false).unwrap();
        // the floor gets a second picture (style 32, a switchable light) at the end of the lump
        let ofs = bsp.faces[0].light_ofs as usize;
        let one = 17 * 17 * 3;
        let mut second = vec![7u8; one];
        second[0] = 9;
        let at = bsp.lighting.len();
        bsp.lighting.extend_from_slice(&bsp.lighting[ofs..ofs + one].to_vec());
        bsp.lighting.extend_from_slice(&second);
        bsp.faces[0].light_ofs = at as i32;
        bsp.faces[0].styles = [0, 32, 255, 255];
        let a = pack(&bsp, &[0, 1]);
        assert_eq!((a.maps, a.planes), (2, 3));
        assert_eq!(a.full_height(), a.height * 3);
        let plane = (a.width * a.height * 3) as usize;
        let p = a.placed[&0];
        let o = ((p.y * a.width + p.x) * 3) as usize;
        assert_eq!(a.rgb[o], 100); // style 0
        assert_eq!(a.rgb[plane + o], 9); // style 32's picture, first luxel
        assert_eq!(a.rgb[plane + o + 3], 7);
        assert_eq!(&a.rgb[2 * plane + o..2 * plane + o + 3], &[0, 32, 255]); // the face's styles
        // the other face has one picture: black in plane 1, styles 0, none, none
        let q = a.placed[&1];
        let o = ((q.y * a.width + q.x) * 3) as usize;
        assert_eq!(a.rgb[plane + o], 0);
        assert_eq!(&a.rgb[2 * plane + o..2 * plane + o + 3], &[0, 255, 255]);
        // the white block is style 0
        assert_eq!(&a.rgb[2 * plane..2 * plane + 3], &[0, 255, 255]);
        // a face claiming four pictures with room for only two keeps two
        bsp.faces[0].styles = [0, 32, 33, 34];
        let a = pack(&bsp, &[0]);
        assert_eq!(a.maps, 2);
        assert_eq!(planes_for(4), 6);
    }

    #[test]
    fn light_under_a_point() {
        let bsp = Bsp::parse(&fixture::build(), false).unwrap();
        // straight down from the middle of the room onto the floor (light 100, style 0 normal)
        let l = light_point(&bsp, [0.0, 0.0, 0.0], [0.0, 0.0, -2048.0], |_| STYLE_NORMAL).unwrap();
        assert_eq!(l, [103, 103, 103]); // 100 * 264 >> 8
        // style off: dark
        assert_eq!(light_point(&bsp, [0.0, 0.0, 0.0], [0.0, 0.0, -2048.0], |_| 0).unwrap(), [0, 0, 0]);
        // a ray that leaves the room through nothing
        assert!(light_point(&bsp, [0.0, 0.0, 0.0], [0.0, 0.0, 0.5], |_| STYLE_NORMAL).is_none());
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
