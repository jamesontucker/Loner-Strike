//! Spray decals: any picture turned into the `tempdecal.wad` Counter-Strike 1.6 and
//! Condition Zero send to the server as the player's spray.
//!
//! What the game expects (checked against the tempdecal.wad the game itself writes, and the
//! community's spray guides):
//! - A WAD3 file with one lump, a decal (type 0x40): a miptex with four mip levels, then a
//!   256-colour palette. The game names its own lump `LOGO`; community tools use `{LOGO`
//!   (the `{` marks a see-through texture). Only the first lump is read, so either works; we
//!   write `{LOGO`.
//! - How it's drawn is decided by palette colour 255:
//!   - pure blue (0, 0, 255): a colour spray. Indexes 0-254 are its colours and index 255 is
//!     see-through (hard edges). If colour 255 isn't pure blue a colour spray draws wrong, so
//!     it's always set, even when nothing is see-through.
//!   - anything else: a one-colour spray ("indexalpha", what the game makes from its logos):
//!     every pixel is painted in colour 255, with the index as its opacity (0 = clear,
//!     255 = solid). Colours 0-254 don't matter; the game writes them black.
//! - Size: sides are multiples of 16, at most 256, and width x height at most 12288 pixels
//!   (the smallest of the limits the guides give: 128x96, 96x128, 112x112 is too big).
//!
//! The player's choice in Options > Multiplayer rewrites tempdecal.wad from `logos/*.bmp`,
//! so the community makes the file read-only; the Tools page does that too (spray.rs).

use super::{cname, Error, Le, Result};
use image::RgbaImage;
use std::collections::HashMap;

/// The lump name we write (the game reads whichever lump comes first).
pub const LUMP_NAME: &str = "{LOGO";
/// WAD3 lump type of a decal.
pub const TYPE_DECAL: u8 = 0x40;
/// Width x height limit.
pub const MAX_PIXELS: u32 = 12288;
pub const MAX_SIDE: u32 = 256;
pub const STEP: u32 = 16;
/// Palette colour 255 that makes a spray a colour one.
pub const KEY_BLUE: [u8; 3] = [0, 0, 255];

/// How the spray looks in game.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Look {
    /// The picture's own colours (up to 255 of them).
    Colour,
    /// One paint colour; dark parts of the picture are painted (light ones with `invert`).
    Mono { paint: [u8; 3], invert: bool },
}

/// Which parts of the picture become see-through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clear {
    /// Nothing (only the padding around a picture that isn't the spray's shape).
    None,
    /// The picture's own transparency (PNG, WebP, TGA).
    Alpha,
    /// The colour of the top-left corner, and colours close to it (a plain background).
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// The size budget in pixels (at most MAX_PIXELS).
    pub max_pixels: u32,
    pub look: Look,
    pub clear: Clear,
    /// Floyd-Steinberg dithering for colour sprays (photos).
    pub dither: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { max_pixels: MAX_PIXELS, look: Look::Colour, clear: Clear::Alpha, dither: false }
    }
}

/// A spray ready to write: one palette index per pixel, rows from the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Spray {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub palette: Vec<[u8; 3]>,
}

impl Spray {
    /// A colour spray (palette 255 pure blue) rather than a one-colour one.
    pub fn is_colour(&self) -> bool {
        self.palette.get(255) == Some(&KEY_BLUE)
    }

    /// The spray as the game draws it, as RGBA (see-through parts have alpha 0).
    pub fn to_rgba(&self) -> Vec<u8> {
        let colour = self.is_colour();
        let paint = self.palette.get(255).copied().unwrap_or([0, 0, 0]);
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &i in &self.pixels {
            if colour {
                let c = self.palette.get(i as usize).copied().unwrap_or([0, 0, 0]);
                out.extend([c[0], c[1], c[2], if i == 255 { 0 } else { 255 }]);
            } else {
                out.extend([paint[0], paint[1], paint[2], i]);
            }
        }
        out
    }

    /// How many palette colours the picture uses (one-colour sprays: 1).
    pub fn colours_used(&self) -> usize {
        if !self.is_colour() {
            return 1;
        }
        let mut seen = [false; 256];
        for &i in &self.pixels {
            seen[i as usize] = true;
        }
        seen[..255].iter().filter(|&&s| s).count()
    }
}

/// Whether a spray of this size works in game.
pub fn valid_size(w: u32, h: u32) -> bool {
    (STEP..=MAX_SIDE).contains(&w) && (STEP..=MAX_SIDE).contains(&h) && w.is_multiple_of(STEP) && h.is_multiple_of(STEP) && w * h <= MAX_PIXELS
}

/// The spray size for a picture of `w` x `h`: the valid size closest to its shape, as big as
/// the budget allows. (The picture is then fitted inside, see-through padding making up
/// the small difference in shape.)
pub fn fit_size(w: u32, h: u32, max_pixels: u32) -> (u32, u32) {
    let budget = max_pixels.clamp(STEP * STEP, MAX_PIXELS);
    let aspect = (w.max(1) as f64 / h.max(1) as f64).ln();
    let mut best = (STEP, STEP, f64::MAX);
    for sw in (STEP..=MAX_SIDE).step_by(STEP as usize) {
        for sh in (STEP..=MAX_SIDE).step_by(STEP as usize) {
            if sw * sh > budget {
                continue;
            }
            let shape = ((sw as f64 / sh as f64).ln() - aspect).abs();
            let unused = 1.0 - (sw * sh) as f64 / budget as f64;
            let cost = shape + 0.15 * unused;
            if cost < best.2 - 1e-9 {
                best = (sw, sh, cost);
            }
        }
    }
    (best.0, best.1)
}

/// Turns a picture into a spray.
pub fn make(src: &RgbaImage, opts: &Options) -> Spray {
    let mut src = src.clone();
    match opts.clear {
        Clear::None => src.pixels_mut().for_each(|p| p.0[3] = 255),
        Clear::Alpha => {}
        Clear::Background => {
            let key = src.get_pixel(0, 0).0;
            for p in src.pixels_mut() {
                let close = (0..3).all(|c| (p.0[c] as i32 - key[c] as i32).abs() <= 40);
                if close {
                    p.0[3] = 0;
                }
            }
        }
    }
    let (w, h) = fit_size(src.width(), src.height(), opts.max_pixels);
    let img = fit_into(&src, w, h);
    match opts.look {
        Look::Colour => colour_spray(&img, opts.dither),
        Look::Mono { paint, invert } => mono_spray(&img, paint, invert),
    }
}

/// The picture scaled to fit inside `w` x `h`, centred on a clear background.
fn fit_into(src: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    let scale = (w as f64 / src.width().max(1) as f64).min(h as f64 / src.height().max(1) as f64);
    let nw = ((src.width() as f64 * scale).round() as u32).clamp(1, w);
    let nh = ((src.height() as f64 * scale).round() as u32).clamp(1, h);
    // scale with alpha premultiplied so clear areas don't bleed their hidden colour in
    let mut pre = src.clone();
    for p in pre.pixels_mut() {
        let a = p.0[3] as u32;
        for c in 0..3 {
            p.0[c] = ((p.0[c] as u32 * a + 127) / 255) as u8;
        }
    }
    let scaled = image::imageops::resize(&pre, nw, nh, image::imageops::FilterType::Lanczos3);
    let mut out = RgbaImage::new(w, h);
    let (ox, oy) = ((w - nw) / 2, (h - nh) / 2);
    for (x, y, p) in scaled.enumerate_pixels() {
        let mut q = p.0;
        if q[3] > 0 {
            for c in 0..3 {
                q[c] = ((q[c] as u32 * 255 + q[3] as u32 / 2) / q[3] as u32).min(255) as u8;
            }
        }
        out.put_pixel(x + ox, y + oy, image::Rgba(q));
    }
    out
}

fn colour_spray(img: &RgbaImage, dither: bool) -> Spray {
    let solid = |p: &image::Rgba<u8>| p.0[3] >= 128;
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for p in img.pixels().filter(|p| solid(p)) {
        *counts.entry([p.0[0], p.0[1], p.0[2]]).or_default() += 1;
    }
    let mut palette = quantise(&counts.into_iter().collect::<Vec<_>>(), 255);
    // all-grey colours could read as a one-colour spray to some engines (Xash): tint one
    // unused entry
    let grey = palette.iter().all(|c| c[0] == c[1] && c[1] == c[2]);
    palette.resize(255, [0, 0, 0]);
    if grey {
        palette[254] = [1, 0, 0];
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut pixels = vec![255u8; w * h];
    let mut cache: HashMap<[u8; 3], u8> = HashMap::new();
    let mut nearest = |c: [u8; 3]| -> u8 { *cache.entry(c).or_insert_with(|| nearest_index(&palette, c)) };
    // error carried to the next pixels (dithering), per channel
    let mut err = vec![[0i32; 3]; if dither { w * h } else { 0 }];
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x as u32, y as u32);
            if !solid(p) {
                continue;
            }
            let at = y * w + x;
            let mut want = [p.0[0], p.0[1], p.0[2]];
            if dither {
                for c in 0..3 {
                    want[c] = (want[c] as i32 + err[at][c] / 16).clamp(0, 255) as u8;
                }
            }
            let i = nearest(want);
            pixels[at] = i;
            if dither {
                let got = palette[i as usize];
                let e: [i32; 3] = std::array::from_fn(|c| want[c] as i32 - got[c] as i32);
                let mut spread = |dx: isize, dy: usize, k: i32| {
                    let nx = x as isize + dx;
                    if nx < 0 || nx as usize >= w || y + dy >= h {
                        return;
                    }
                    let n = (y + dy) * w + nx as usize;
                    for c in 0..3 {
                        err[n][c] += e[c] * k;
                    }
                };
                spread(1, 0, 7);
                spread(-1, 1, 3);
                spread(0, 1, 5);
                spread(1, 1, 1);
            }
        }
    }
    palette.push(KEY_BLUE);
    Spray { width: w as u32, height: h as u32, pixels, palette }
}

fn mono_spray(img: &RgbaImage, paint: [u8; 3], invert: bool) -> Spray {
    let pixels = img
        .pixels()
        .map(|p| {
            let lum = (p.0[0] as u32 * 299 + p.0[1] as u32 * 587 + p.0[2] as u32 * 114) / 1000;
            let ink = if invert { lum } else { 255 - lum };
            ((ink * p.0[3] as u32 + 127) / 255) as u8
        })
        .collect();
    let mut palette = vec![[0u8, 0, 0]; 255];
    // pure blue would make it a colour spray
    palette.push(if paint == KEY_BLUE { [0, 0, 254] } else { paint });
    Spray { width: img.width(), height: img.height(), pixels, palette }
}

fn dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    // weighted towards green, like the eye
    let d = |i: usize| (a[i] as i32 - b[i] as i32).pow(2) as u32;
    2 * d(0) + 4 * d(1) + 3 * d(2)
}

fn nearest_index(palette: &[[u8; 3]], c: [u8; 3]) -> u8 {
    let mut best = (0usize, u32::MAX);
    for (i, &p) in palette.iter().enumerate() {
        let d = dist(p, c);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0 as u8
}

/// Median cut: at most `n` colours standing for the weighted colours given. Fewer distinct
/// colours than `n` come back exactly.
pub fn quantise(colours: &[([u8; 3], u32)], n: usize) -> Vec<[u8; 3]> {
    if colours.is_empty() || n == 0 {
        return Vec::new();
    }
    if colours.len() <= n {
        let mut v: Vec<[u8; 3]> = colours.iter().map(|c| c.0).collect();
        v.sort();
        return v;
    }
    let mut boxes: Vec<Vec<([u8; 3], u32)>> = vec![colours.to_vec()];
    while boxes.len() < n {
        // split the box with the most spread x weight
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(_, b)| widest(b).1 as u64 * b.iter().map(|x| x.1 as u64).sum::<u64>())
            .map(|(i, _)| i);
        let Some(i) = pick else { break };
        let mut b = boxes.swap_remove(i);
        let (ch, _) = widest(&b);
        b.sort_by_key(|x| x.0[ch]);
        let total: u64 = b.iter().map(|x| x.1 as u64).sum();
        let mut acc = 0u64;
        let mut cut = 1;
        for (k, x) in b.iter().enumerate() {
            acc += x.1 as u64;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let mut out: Vec<[u8; 3]> = boxes
        .iter()
        .map(|b| {
            let total: u64 = b.iter().map(|x| x.1 as u64).sum::<u64>().max(1);
            std::array::from_fn(|c| ((b.iter().map(|x| x.0[c] as u64 * x.1 as u64).sum::<u64>() + total / 2) / total) as u8)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The channel with the biggest range in a box, and that range.
fn widest(b: &[([u8; 3], u32)]) -> (usize, u8) {
    (0..3)
        .map(|c| {
            let lo = b.iter().map(|x| x.0[c]).min().unwrap_or(0);
            let hi = b.iter().map(|x| x.0[c]).max().unwrap_or(0);
            (c, hi - lo)
        })
        .max_by_key(|&(c, r)| (r, std::cmp::Reverse(c)))
        .unwrap_or((0, 0))
}

/// The four mip levels (full, 1/2, 1/4, 1/8). One-colour sprays average the opacity; colour
/// ones take the top-left pixel of each block.
fn mips(s: &Spray) -> [Vec<u8>; 4] {
    let colour = s.is_colour();
    std::array::from_fn(|level| {
        let k = 1usize << level;
        let (w, h) = (s.width as usize / k, s.height as usize / k);
        let mut v = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                if colour {
                    v.push(s.pixels[y * k * s.width as usize + x * k]);
                } else {
                    let mut sum = 0u32;
                    for dy in 0..k {
                        for dx in 0..k {
                            sum += s.pixels[(y * k + dy) * s.width as usize + x * k + dx] as u32;
                        }
                    }
                    v.push((sum / (k * k) as u32) as u8);
                }
            }
        }
        v
    })
}

/// The tempdecal.wad bytes for a spray. The size must be valid (`valid_size`).
pub fn write_wad(s: &Spray) -> Result<Vec<u8>> {
    if !valid_size(s.width, s.height) || s.pixels.len() != (s.width * s.height) as usize || s.palette.len() != 256 {
        return Err(Error::NotFormat { what: "spray", detail: format!("{}x{} isn't a size sprays can be", s.width, s.height) });
    }
    let levels = mips(s);
    let mut lump = Vec::new();
    let mut name = [0u8; 16];
    name[..LUMP_NAME.len()].copy_from_slice(LUMP_NAME.as_bytes());
    lump.extend(name);
    lump.extend(s.width.to_le_bytes());
    lump.extend(s.height.to_le_bytes());
    let mut at = 40u32;
    for l in &levels {
        lump.extend(at.to_le_bytes());
        at += l.len() as u32;
    }
    for l in &levels {
        lump.extend(l);
    }
    lump.extend(256u16.to_le_bytes());
    for c in &s.palette {
        lump.extend(c);
    }
    while lump.len() % 4 != 0 {
        lump.push(0);
    }
    let mut out = b"WAD3".to_vec();
    out.extend(1i32.to_le_bytes());
    out.extend((12 + lump.len() as i32).to_le_bytes());
    out.extend(&lump);
    out.extend(12u32.to_le_bytes());
    out.extend((lump.len() as u32).to_le_bytes());
    out.extend((lump.len() as u32).to_le_bytes());
    out.extend([TYPE_DECAL, 0, 0, 0]);
    out.extend(name);
    Ok(out)
}

/// Reads the spray back from a tempdecal.wad (ours, the game's or anyone's): its first lump.
pub fn read_wad(bytes: &[u8]) -> Result<Spray> {
    let what = "spray file";
    let r = Le::new(bytes, what);
    if r.bytes(0, 4)? != b"WAD3" {
        return Err(Error::NotFormat { what, detail: "not a WAD3 file".into() });
    }
    let count = r.i32(4)?;
    let dir = r.u32(8)? as usize;
    if count < 1 {
        return Err(Error::damaged(what, "it has no picture in it"));
    }
    let at = r.u32(dir)? as usize;
    let size = r.u32(dir + 8)? as usize;
    let lump = Le::new(r.bytes(at, size)?, what);
    let _ = cname(lump.bytes(0, 16)?);
    let (w, h) = (lump.u32(16)?, lump.u32(20)?);
    if w == 0 || h == 0 || w > MAX_SIDE * 2 || h > MAX_SIDE * 2 || w % 8 != 0 || h % 8 != 0 {
        return Err(Error::damaged(what, format!("its picture size {w}x{h} is wrong")));
    }
    let off0 = lump.u32(24)? as usize;
    let off3 = lump.u32(36)? as usize;
    let pixels = lump.bytes(off0, (w * h) as usize)?.to_vec();
    let pal_at = off3 + ((w / 8) * (h / 8)) as usize;
    let n = lump.u16(pal_at)? as usize;
    if n != 256 {
        return Err(Error::damaged(what, format!("its palette has {n} colours, not 256")));
    }
    let raw = lump.bytes(pal_at + 2, 768)?;
    let palette = raw.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    Ok(Spray { width: w, height: h, pixels, palette })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)))
    }

    #[test]
    fn size_rules() {
        assert!(valid_size(128, 96));
        assert!(valid_size(96, 128));
        assert!(valid_size(256, 48));
        assert!(!valid_size(112, 112), "12544 pixels is over the limit");
        assert!(!valid_size(100, 64), "not a multiple of 16");
        assert!(!valid_size(272, 16), "too wide");
        assert!(!valid_size(0, 16));
        assert_eq!(fit_size(1000, 1000, MAX_PIXELS), (96, 96));
        assert_eq!(fit_size(640, 480, MAX_PIXELS), (128, 96));
        assert_eq!(fit_size(480, 640, MAX_PIXELS), (96, 128));
        assert_eq!(fit_size(64, 64, 4096), (64, 64));
        // every shape and budget lands on a valid size
        for (w, h) in [(1, 1), (1920, 1080), (10, 2000), (5000, 3), (333, 777)] {
            for b in [0, 256, 4096, 8192, MAX_PIXELS, 99999] {
                let (sw, sh) = fit_size(w, h, b);
                assert!(valid_size(sw, sh), "{w}x{h} budget {b} -> {sw}x{sh}");
            }
        }
        let (sw, sh) = fit_size(1920, 1080, MAX_PIXELS);
        assert!(sw > sh && sw * sh >= 10000, "{sw}x{sh}");
    }

    #[test]
    fn quantise_keeps_few_colours_exact_and_caps_many() {
        let few = vec![([255, 0, 0], 10), ([0, 255, 0], 3), ([0, 0, 0], 1)];
        assert_eq!(quantise(&few, 255), vec![[0, 0, 0], [0, 255, 0], [255, 0, 0]]);
        let many: Vec<([u8; 3], u32)> = (0..4096u32).map(|i| ([(i % 16 * 16) as u8, (i / 16 % 16 * 16) as u8, (i / 256 * 16) as u8], 1 + i % 5)).collect();
        let p = quantise(&many, 255);
        assert!(p.len() <= 255 && p.len() > 200, "{}", p.len());
        // two clusters become two colours near their middles
        let two = vec![([10, 10, 10], 5), ([12, 12, 12], 5), ([240, 240, 240], 5), ([250, 250, 250], 5)];
        let p = quantise(&two, 2);
        assert_eq!(p.len(), 2);
        assert!(p[0][0] < 20 && p[1][0] > 230, "{p:?}");
        assert!(quantise(&[], 255).is_empty());
    }

    #[test]
    fn colour_spray_palette_and_transparency() {
        // left half red, right half see-through, 64x64
        let img = picture(64, 64, |x, _| if x < 32 { [200, 20, 20, 255] } else { [0, 255, 0, 0] });
        let s = make(&img, &Options { max_pixels: 4096, ..Options::default() });
        assert_eq!((s.width, s.height), (64, 64));
        assert_eq!(s.palette.len(), 256);
        assert_eq!(s.palette[255], KEY_BLUE);
        assert!(s.is_colour());
        assert_eq!(s.pixels[0 * 64 + 63], 255, "clear side is index 255");
        let red = s.pixels[10 * 64 + 5];
        assert_ne!(red, 255);
        assert_eq!(s.palette[red as usize], [200, 20, 20]);
        let rgba = s.to_rgba();
        assert_eq!(&rgba[(10 * 64 + 5) * 4..][..4], &[200, 20, 20, 255]);
        assert_eq!(rgba[63 * 4 + 3], 0);
        assert_eq!(s.colours_used(), 1);

        // no clear: alpha ignored, all solid
        let s = make(&img, &Options { max_pixels: 4096, clear: Clear::None, ..Options::default() });
        assert!(s.pixels.iter().all(|&i| i != 255));

        // background: the corner's colour goes
        let img = picture(64, 64, |x, y| if (20..44).contains(&x) && (20..44).contains(&y) { [0, 0, 0, 255] } else { [250, 250, 250, 255] });
        let s = make(&img, &Options { max_pixels: 4096, clear: Clear::Background, ..Options::default() });
        assert_eq!(s.pixels[0], 255);
        assert_ne!(s.pixels[32 * 64 + 32], 255);
        // a black-and-white picture isn't mistaken for a one-colour spray
        assert!(s.palette[..255].iter().any(|c| c[0] != c[1] || c[1] != c[2]));
    }

    #[test]
    fn padding_is_clear_and_photos_quantise() {
        // a wide photo-like gradient: padded top and bottom, under 256 colours
        let img = picture(400, 100, |x, y| [(x % 256) as u8, (y * 2) as u8, ((x + y) % 256) as u8, 255]);
        for dither in [false, true] {
            let s = make(&img, &Options { dither, ..Options::default() });
            assert!(valid_size(s.width, s.height), "{}x{}", s.width, s.height);
            assert!(s.colours_used() <= 255);
            assert!(s.colours_used() > 50);
        }
        let s = make(&picture(300, 100, |_, _| [9, 9, 200, 255]), &Options::default());
        // 300x100 fits a 3:1 size exactly or nearly; any padding is clear, the middle isn't
        assert_ne!(s.pixels[(s.height / 2 * s.width + s.width / 2) as usize], 255);
    }

    #[test]
    fn mono_spray_is_a_gradient() {
        let img = picture(64, 64, |x, _| {
            let v = (x * 4) as u8;
            [v, v, v, 255]
        });
        let s = make(&img, &Options { max_pixels: 4096, look: Look::Mono { paint: [255, 120, 24], invert: false }, clear: Clear::Alpha, dither: false });
        assert!(!s.is_colour());
        assert_eq!(s.palette[255], [255, 120, 24]);
        // dark is painted, light is clear
        assert!(s.pixels[0] > 240, "{}", s.pixels[0]);
        assert!(s.pixels[63] < 20, "{}", s.pixels[63]);
        assert_eq!(&s.to_rgba()[..4], &[255, 120, 24, s.pixels[0]]);
        let inv = make(&img, &Options { max_pixels: 4096, look: Look::Mono { paint: KEY_BLUE, invert: true }, clear: Clear::Alpha, dither: false });
        assert!(inv.pixels[0] < 20);
        assert_eq!(inv.palette[255], [0, 0, 254], "pure blue would turn it into a colour spray");
        assert!(!inv.is_colour());
    }

    #[test]
    fn wad_round_trip() {
        let img = picture(200, 150, |x, y| [(x * 7 % 256) as u8, (y * 3 % 256) as u8, 90, if x < 10 { 0 } else { 255 }]);
        for look in [Look::Colour, Look::Mono { paint: [10, 200, 30], invert: false }] {
            let s = make(&img, &Options { look, ..Options::default() });
            let wad = write_wad(&s).unwrap();
            assert_eq!(&wad[..4], b"WAD3");
            assert_eq!(wad.len() % 4, 0);
            let back = read_wad(&wad).unwrap();
            assert_eq!(back, s);
            // the directory entry: one decal lump named {LOGO
            let dir = u32::from_le_bytes(wad[8..12].try_into().unwrap()) as usize;
            assert_eq!(wad[dir + 12], TYPE_DECAL);
            assert_eq!(cname(&wad[dir + 16..dir + 32]), LUMP_NAME);
            // the lump size is what the game's own file has for the same size
            let px = (s.width * s.height) as usize;
            let lump = 40 + px * 85 / 64 + 2 + 768;
            assert_eq!(u32::from_le_bytes(wad[dir + 4..dir + 8].try_into().unwrap()) as usize, (lump + 3) / 4 * 4);
        }
    }

    /// The same layout as the tempdecal.wad the game writes (64x64 "LOGO", 6296 bytes).
    #[test]
    fn reads_the_games_layout() {
        let s = Spray { width: 64, height: 64, pixels: (0..4096).map(|i| (i % 256) as u8).collect(), palette: { let mut p = vec![[0u8; 3]; 255]; p.push([255, 120, 24]); p } };
        let wad = write_wad(&s).unwrap();
        assert_eq!(wad.len(), 6296);
        assert_eq!(read_wad(&wad).unwrap(), s);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(read_wad(b"").is_err());
        assert!(read_wad(b"WAD2\x01\0\0\0\x0c\0\0\0").is_err());
        let s = make(&picture(32, 32, |_, _| [1, 2, 3, 255]), &Options::default());
        let wad = write_wad(&s).unwrap();
        for cut in [5, 12, 40, 100, wad.len() - 40] {
            assert!(read_wad(&wad[..cut]).is_err(), "cut at {cut}");
        }
        let bad = Spray { width: 100, height: 100, pixels: vec![0; 10000], palette: vec![[0; 3]; 256] };
        assert!(write_wad(&bad).is_err());
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> reads the real tempdecal.wad files there.
    #[test]
    fn real_sprays() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        for g in ["cstrike", "czero", "valve"] {
            let p = std::path::Path::new(&hl).join(g).join("tempdecal.wad");
            let Ok(b) = std::fs::read(&p) else { continue };
            let s = read_wad(&b).unwrap();
            println!("{g}: {}x{} colour={} paint={:?}", s.width, s.height, s.is_colour(), s.palette[255]);
            if valid_size(s.width, s.height) {
                assert_eq!(read_wad(&write_wad(&s).unwrap()).unwrap(), s);
            }
        }
    }
    /// Opt-in: LS_SPRAY_SAMPLE=<picture> LS_TEST_DUMP=<folder> saves the picture as each kind
    /// of spray (PNG, as the game draws it) to look at.
    #[test]
    fn sample_picture() {
        let (Ok(src), Ok(out)) = (std::env::var("LS_SPRAY_SAMPLE"), std::env::var("LS_TEST_DUMP")) else { return };
        let img = image::open(&src).unwrap().to_rgba8();
        let kinds = [
            ("colour", Options::default()),
            ("dither", Options { dither: true, ..Options::default() }),
            ("background", Options { clear: Clear::Background, ..Options::default() }),
            ("mono", Options { look: Look::Mono { paint: [255, 120, 24], invert: false }, ..Options::default() }),
        ];
        for (name, o) in kinds {
            let s = make(&img, &o);
            let png = super::super::png(s.width, s.height, &s.to_rgba(), true).unwrap();
            std::fs::write(std::path::Path::new(&out).join(format!("spray-{name}.png")), png).unwrap();
            println!("{name}: {}x{}, {} colours", s.width, s.height, s.colours_used());
        }
    }
}
