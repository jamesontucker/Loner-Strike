//! Writing GoldSrc sprites (.spr, version 2) for the Sprite maker (T1fix): one or more pictures
//! become the frames of one sprite, sharing a 256-color palette made by the spray maker's median
//! cut (wadwrite::quantise, shared). Read back by spr.rs in the tests.
//!
//! Render formats, as the engine draws them:
//! - normal: the colors as they are; clear areas are black.
//! - additive: added to what's behind (black disappears): clear areas are black, colors are
//!   premultiplied by their opacity, so soft edges fade.
//! - alphatest: palette index 255 is see-through (its color is the transparent color); every other
//!   pixel is solid.
//! - indexalpha: one color (the palette's last entry) with each pixel's index as its opacity.

use super::spr::TexFormat;
use super::wadwrite::{fit_into, nearest_index, quantise};
use image::RgbaImage;
use std::collections::HashMap;

/// The biggest frame side the maker writes (GoldSrc's software and GL renderers both draw
/// sprites up to 256; many mods use 512, the most this writes).
pub const MAX_SIDE: u32 = 512;
/// Frames in one sprite.
pub const MAX_FRAMES: usize = 64;

/// How each picture fills the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fit {
    /// all of it, centered, clear around it
    Contain,
    /// the frame filled, the overflow cut off
    Cover,
    /// stretched to the frame
    Stretch,
}

pub struct Options {
    pub width: u32,
    pub height: u32,
    /// the `type` field: 0 parallel upright, 1 facing upright, 2 parallel, 3 oriented, 4 parallel oriented
    pub orientation: i32,
    pub format: TexFormat,
    pub fit: Fit,
    /// a color that counts as clear (besides the picture's own transparency)
    pub key: Option<[u8; 3]>,
    /// alphatest's see-through color (palette index 255); indexalpha's tint
    pub tint: [u8; 3],
}

/// Why sprite settings can't be used (None: they can).
pub fn size_problem(w: u32, h: u32, frames: usize) -> Option<String> {
    if w < 8 || h < 8 || w > MAX_SIDE || h > MAX_SIDE || w % 8 != 0 || h % 8 != 0 {
        return Some(format!("The size must be a multiple of 8, from 8 to {MAX_SIDE}."));
    }
    if frames == 0 {
        return Some("Add a picture.".into());
    }
    if frames > MAX_FRAMES {
        return Some(format!("A sprite holds {MAX_FRAMES} frames at most."));
    }
    None
}

/// One picture fitted into the frame (alpha kept; the key color made clear first).
fn fitted(src: &RgbaImage, o: &Options) -> RgbaImage {
    let mut src = src.clone();
    if let Some(k) = o.key {
        for p in src.pixels_mut() {
            if (0..3).all(|c| (p.0[c] as i32 - k[c] as i32).abs() <= 24) {
                p.0[3] = 0;
            }
        }
    }
    match o.fit {
        Fit::Contain => fit_into(&src, o.width, o.height),
        Fit::Stretch => image::imageops::resize(&src, o.width, o.height, image::imageops::FilterType::Lanczos3),
        Fit::Cover => {
            let scale = (o.width as f64 / src.width().max(1) as f64).max(o.height as f64 / src.height().max(1) as f64);
            let nw = ((src.width() as f64 * scale).ceil() as u32).max(o.width);
            let nh = ((src.height() as f64 * scale).ceil() as u32).max(o.height);
            let big = image::imageops::resize(&src, nw, nh, image::imageops::FilterType::Lanczos3);
            image::imageops::crop_imm(&big, (nw - o.width) / 2, (nh - o.height) / 2, o.width, o.height).to_image()
        }
    }
}

/// A finished sprite: the file's bytes and what's in it.
pub struct Made {
    pub bytes: Vec<u8>,
    pub colors: usize,
}

/// The pictures as one sprite. The settings must pass `size_problem`.
pub fn make(pictures: &[RgbaImage], o: &Options) -> Result<Made, String> {
    if let Some(p) = size_problem(o.width, o.height, pictures.len()) {
        return Err(p);
    }
    let frames: Vec<RgbaImage> = pictures.iter().map(|p| fitted(p, o)).collect();
    let mut palette: Vec<[u8; 3]>;
    let mut pixels: Vec<Vec<u8>> = Vec::with_capacity(frames.len());
    match o.format {
        TexFormat::IndexAlpha => {
            // a ramp of the tint; each pixel's index is its opacity times its brightness
            palette = (0..256u32).map(|i| std::array::from_fn(|c| ((o.tint[c] as u32 * i + 127) / 255) as u8)).collect();
            palette[255] = o.tint;
            for f in &frames {
                pixels.push(
                    f.pixels()
                        .map(|p| {
                            let lum = (p.0[0] as u32 * 30 + p.0[1] as u32 * 59 + p.0[2] as u32 * 11) / 100;
                            ((p.0[3] as u32 * lum.max(1) + 127) / 255) as u8
                        })
                        .collect(),
                );
            }
        }
        _ => {
            let alphatest = o.format == TexFormat::AlphaTest;
            // what each pixel looks like (None: see-through, alphatest only)
            let look = |p: &image::Rgba<u8>| -> Option<[u8; 3]> {
                let a = p.0[3] as u32;
                if alphatest {
                    return (a >= 128).then_some([p.0[0], p.0[1], p.0[2]]);
                }
                // normal and additive: clear goes black, soft edges darken
                Some(std::array::from_fn(|c| ((p.0[c] as u32 * a + 127) / 255) as u8))
            };
            let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
            for f in &frames {
                for p in f.pixels() {
                    if let Some(c) = look(p) {
                        *counts.entry(c).or_insert(0) += 1;
                    }
                }
            }
            let mut colours: Vec<([u8; 3], u32)> = counts.into_iter().collect();
            colours.sort();
            let room = if alphatest { 255 } else { 256 };
            palette = quantise(&colours, room);
            if palette.is_empty() {
                palette.push([0, 0, 0]);
            }
            let used = palette.clone();
            palette.resize(256, [0, 0, 0]);
            if alphatest {
                palette[255] = o.tint;
            }
            for f in &frames {
                pixels.push(f.pixels().map(|p| look(p).map_or(255, |c| nearest_index(&used, c))).collect());
            }
        }
    }
    let colors = {
        let mut seen = [false; 256];
        pixels.iter().flatten().for_each(|&i| seen[i as usize] = true);
        seen.iter().filter(|s| **s).count()
    };
    Ok(Made { bytes: write(o.orientation, o.format, o.width, o.height, &palette, &pixels), colors })
}

/// The .spr bytes: the header, the palette (256 colors) and single frames (origin at the
/// middle). `pixels` are palette indexes, width × height each.
pub fn write(orientation: i32, format: TexFormat, w: u32, h: u32, palette: &[[u8; 3]], frames: &[Vec<u8>]) -> Vec<u8> {
    let mut b = Vec::with_capacity(64 + 768 + frames.len() * (16 + (w * h) as usize));
    let i32le = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    b.extend_from_slice(b"IDSP");
    i32le(&mut b, 2);
    i32le(&mut b, orientation.clamp(0, 4));
    i32le(&mut b, match format {
        TexFormat::Normal => 0,
        TexFormat::Additive => 1,
        TexFormat::IndexAlpha => 2,
        TexFormat::AlphaTest => 3,
    });
    let radius = ((w as f32 / 2.0).powi(2) + (h as f32 / 2.0).powi(2)).sqrt();
    b.extend_from_slice(&radius.to_le_bytes());
    i32le(&mut b, w as i32);
    i32le(&mut b, h as i32);
    i32le(&mut b, frames.len() as i32);
    b.extend_from_slice(&0f32.to_le_bytes()); // beam length
    i32le(&mut b, 0); // sync type: synchronized
    b.extend_from_slice(&256u16.to_le_bytes());
    for i in 0..256 {
        b.extend_from_slice(&palette.get(i).copied().unwrap_or([0, 0, 0]));
    }
    for px in frames {
        i32le(&mut b, 0); // a single frame
        i32le(&mut b, -(w as i32) / 2);
        i32le(&mut b, h as i32 / 2);
        i32le(&mut b, w as i32);
        i32le(&mut b, h as i32);
        b.extend_from_slice(px);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::super::spr;
    use super::*;

    fn opts(format: TexFormat) -> Options {
        Options { width: 16, height: 8, orientation: 2, format, fit: Fit::Stretch, key: None, tint: [0, 0, 255] }
    }

    /// Left half red, right half clear (or `right` when given).
    fn pic(right: Option<[u8; 4]>) -> RgbaImage {
        RgbaImage::from_fn(32, 16, |x, _| image::Rgba(if x < 16 { [255, 0, 0, 255] } else { right.unwrap_or([0, 0, 0, 0]) }))
    }

    #[test]
    fn writes_what_the_reader_reads_back() {
        let m = make(&[pic(None), pic(Some([0, 255, 0, 255])), pic(Some([0, 0, 255, 255]))], &opts(TexFormat::Normal)).unwrap();
        let s = spr::parse(&m.bytes, false).unwrap();
        assert_eq!((s.orientation, s.format, s.width, s.height, s.frames.len()), (2, TexFormat::Normal, 16, 8, 3));
        let (w, h, px) = s.rgba(1).unwrap();
        assert_eq!((w, h), (16, 8));
        assert_eq!(&px[0..4], &[255, 0, 0, 255], "left: red");
        assert_eq!(&px[(15 * 4)..(16 * 4)], &[0, 255, 0, 255], "right of frame 2: green");
        // frame 1's clear half is black in normal sprites
        assert_eq!(&s.rgba(0).unwrap().2[(15 * 4)..(16 * 4)], &[0, 0, 0, 255]);
        assert!(m.colors >= 3);
        // every orientation the format has
        for t in 0..5 {
            let o = Options { orientation: t, ..opts(TexFormat::Additive) };
            assert_eq!(spr::parse(&make(&[pic(None)], &o).unwrap().bytes, true).unwrap().orientation, t);
        }
    }

    #[test]
    fn alphatest_and_indexalpha_transparency() {
        let m = make(&[pic(None)], &opts(TexFormat::AlphaTest)).unwrap();
        let s = spr::parse(&m.bytes, false).unwrap();
        assert_eq!(s.palette[255], [0, 0, 255], "index 255 is the transparent color");
        assert_eq!(s.frames[0].pixels[15], 255, "a clear pixel is index 255");
        assert_ne!(s.frames[0].pixels[0], 255);
        assert_eq!(s.rgba(0).unwrap().2[15 * 4 + 3], 0);
        // a key color counts as clear too
        let o = Options { key: Some([0, 255, 0]), ..opts(TexFormat::AlphaTest) };
        let k = spr::parse(&make(&[pic(Some([0, 255, 0, 255]))], &o).unwrap().bytes, false).unwrap();
        assert_eq!(k.frames[0].pixels[15], 255);
        // indexalpha: opaque bright red is a high index, clear is 0, the tint is the last color
        let ia = spr::parse(&make(&[pic(None)], &Options { tint: [255, 200, 0], ..opts(TexFormat::IndexAlpha) }).unwrap().bytes, false).unwrap();
        assert_eq!(ia.palette[255], [255, 200, 0]);
        assert_eq!(ia.frames[0].pixels[15], 0);
        assert!(ia.frames[0].pixels[0] > 50);
    }

    #[test]
    fn many_colors_fit_one_palette() {
        let rainbow = RgbaImage::from_fn(64, 64, |x, y| image::Rgba([(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255]));
        let o = Options { width: 64, height: 64, ..opts(TexFormat::Normal) };
        let m = make(&[rainbow.clone(), image::imageops::flip_horizontal(&rainbow)], &o).unwrap();
        let s = spr::parse(&m.bytes, false).unwrap();
        assert!(m.colors <= 256 && m.colors > 100, "{}", m.colors);
        assert_eq!(s.frames.len(), 2);
        // cover and contain keep the frame size
        for fit in [Fit::Cover, Fit::Contain] {
            let o = Options { width: 32, height: 16, fit, ..opts(TexFormat::Normal) };
            let s = spr::parse(&make(&[rainbow.clone()], &o).unwrap().bytes, true).unwrap();
            assert_eq!((s.frames[0].width, s.frames[0].height), (32, 16));
        }
    }

    #[test]
    fn refuses_bad_sizes_and_counts() {
        assert!(size_problem(16, 8, 1).is_none());
        for (w, h) in [(12, 8), (0, 8), (8, 4), (520, 8), (1024, 1024)] {
            assert!(size_problem(w, h, 1).is_some(), "{w}x{h}");
        }
        assert!(size_problem(8, 8, 0).is_some() && size_problem(8, 8, MAX_FRAMES + 1).is_some());
        assert!(make(&[], &opts(TexFormat::Normal)).is_err());
        assert!(make(&[pic(None)], &Options { width: 10, ..opts(TexFormat::Normal) }).is_err());
    }
}
