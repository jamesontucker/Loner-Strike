//! GoldSrc sprites (.spr, version 2): the header, the 256-color palette and the frames, decoded to
//! RGBA for thumbnails and the editor's animated preview (agent P5). Frame groups are flattened
//! (each picture in a group is one frame). Every size is checked before anything is read.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexFormat {
    Normal,
    Additive,
    IndexAlpha,
    AlphaTest,
}

impl TexFormat {
    pub fn name(self) -> &'static str {
        match self {
            TexFormat::Normal => "normal",
            TexFormat::Additive => "additive",
            TexFormat::IndexAlpha => "indexalpha",
            TexFormat::AlphaTest => "alphatest",
        }
    }
}

/// How the sprite faces the camera (the `type` field).
pub fn orientation_name(t: i32) -> &'static str {
    match t {
        0 => "parallel upright",
        1 => "facing upright",
        2 => "parallel",
        3 => "oriented",
        4 => "parallel oriented",
        _ => "unknown",
    }
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// palette indexes, width × height
    pub pixels: Vec<u8>,
}

pub struct Sprite {
    pub orientation: i32,
    pub format: TexFormat,
    pub width: u32,
    pub height: u32,
    pub palette: Vec<[u8; 3]>,
    pub frames: Vec<Frame>,
}

#[derive(Debug, PartialEq)]
pub struct SprError(pub &'static str);

impl fmt::Display for SprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

const MAX_SIDE: u32 = 1024;
const MAX_FRAMES: usize = 1000;
const MAX_PIXELS: u64 = 32 << 20;

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], SprError> {
        let end = self.at.checked_add(n).ok_or(SprError("damaged"))?;
        let s = self.b.get(self.at..end).ok_or(SprError("cut short"))?;
        self.at = end;
        Ok(s)
    }
    fn i32(&mut self) -> Result<i32, SprError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u16(&mut self) -> Result<u16, SprError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
}

/// Reads a sprite file (`first_only`: only the first frame's pixels, for thumbnails).
pub fn parse(bytes: &[u8], first_only: bool) -> Result<Sprite, SprError> {
    let mut r = Rd { b: bytes, at: 0 };
    if r.take(4)? != b"IDSP" {
        return Err(SprError("not a sprite"));
    }
    if r.i32()? != 2 {
        return Err(SprError("not a Half-Life sprite (version 2)"));
    }
    let orientation = r.i32()?;
    let format = match r.i32()? {
        0 => TexFormat::Normal,
        1 => TexFormat::Additive,
        2 => TexFormat::IndexAlpha,
        3 => TexFormat::AlphaTest,
        _ => return Err(SprError("unknown sprite format")),
    };
    r.take(4)?; // bounding radius
    let (w, h) = (r.i32()?, r.i32()?);
    let count = r.i32()?;
    r.take(8)?; // beam length, sync type
    if w <= 0 || h <= 0 || w as u32 > MAX_SIDE || h as u32 > MAX_SIDE || count <= 0 || count as usize > MAX_FRAMES {
        return Err(SprError("sizes out of range"));
    }
    let colors = r.u16()? as usize;
    if colors == 0 || colors > 256 {
        return Err(SprError("bad palette"));
    }
    let pal = r.take(colors * 3)?;
    let mut palette: Vec<[u8; 3]> = pal.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    palette.resize(256, [0, 0, 0]);
    let mut frames = Vec::new();
    let mut total: u64 = 0;
    let mut read_one = |r: &mut Rd| -> Result<Frame, SprError> {
        r.take(8)?; // origin
        let (fw, fh) = (r.i32()?, r.i32()?);
        if fw <= 0 || fh <= 0 || fw as u32 > MAX_SIDE || fh as u32 > MAX_SIDE {
            return Err(SprError("frame size out of range"));
        }
        total += fw as u64 * fh as u64;
        if total > MAX_PIXELS {
            return Err(SprError("too many pixels"));
        }
        let px = r.take(fw as usize * fh as usize)?.to_vec();
        Ok(Frame { width: fw as u32, height: fh as u32, pixels: px })
    };
    for _ in 0..count {
        let kind = r.i32()?;
        if kind == 0 {
            frames.push(read_one(&mut r)?);
        } else {
            let n = r.i32()?;
            if n <= 0 || n as usize > MAX_FRAMES {
                return Err(SprError("bad frame group"));
            }
            r.take(n as usize * 4)?; // intervals
            for _ in 0..n {
                frames.push(read_one(&mut r)?);
                if frames.len() > MAX_FRAMES {
                    return Err(SprError("too many frames"));
                }
            }
        }
        if first_only && !frames.is_empty() {
            break;
        }
    }
    Ok(Sprite { orientation, format, width: w as u32, height: h as u32, palette, frames })
}

impl Sprite {
    /// A frame as RGBA, as the game would draw it over a dark background: additive sprites get
    /// their brightness as alpha, alphatest makes index 255 clear, indexalpha uses the last
    /// palette color with the index as alpha.
    pub fn rgba(&self, frame: usize) -> Option<(u32, u32, Vec<u8>)> {
        let f = self.frames.get(frame)?;
        let mut out = Vec::with_capacity(f.pixels.len() * 4);
        for &i in &f.pixels {
            let [r, g, b] = self.palette[i as usize];
            let (c, a) = match self.format {
                TexFormat::Normal => ([r, g, b], 255),
                TexFormat::Additive => ([r, g, b], r.max(g).max(b)),
                TexFormat::AlphaTest => ([r, g, b], if i == 255 { 0 } else { 255 }),
                TexFormat::IndexAlpha => (self.palette[255], i),
            };
            out.extend_from_slice(&[c[0], c[1], c[2], a]);
        }
        Some((f.width, f.height, out))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A tiny sprite: `frames` single frames of 2×2 (plus one group of two when `group`).
    pub fn sample(format: i32, frames: i32, group: bool) -> Vec<u8> {
        let mut b = b"IDSP".to_vec();
        for v in [2, 2, format] {
            b.extend_from_slice(&(v as i32).to_le_bytes());
        }
        b.extend_from_slice(&1.0f32.to_le_bytes());
        for v in [2, 2, frames + group as i32] {
            b.extend_from_slice(&(v as i32).to_le_bytes());
        }
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&256u16.to_le_bytes());
        for i in 0..256u32 {
            b.extend_from_slice(&[i as u8, (255 - i) as u8, 7]);
        }
        let frame = |b: &mut Vec<u8>, px: [u8; 4]| {
            b.extend_from_slice(&[0; 8]);
            b.extend_from_slice(&2i32.to_le_bytes());
            b.extend_from_slice(&2i32.to_le_bytes());
            b.extend_from_slice(&px);
        };
        for k in 0..frames {
            b.extend_from_slice(&0i32.to_le_bytes());
            frame(&mut b, [k as u8, 255, 10, 0]);
        }
        if group {
            b.extend_from_slice(&1i32.to_le_bytes());
            b.extend_from_slice(&2i32.to_le_bytes());
            b.extend_from_slice(&[0; 8]);
            frame(&mut b, [1, 1, 1, 1]);
            frame(&mut b, [2, 2, 2, 2]);
        }
        b
    }

    #[test]
    fn decodes_frames_groups_and_formats() {
        let s = parse(&sample(3, 2, true), false).unwrap();
        assert_eq!((s.frames.len(), s.width, s.format), (4, 2, TexFormat::AlphaTest));
        let (_, _, px) = s.rgba(0).unwrap();
        assert_eq!(&px[4..8], &[255, 0, 7, 0], "index 255 is clear in alphatest");
        assert_eq!(&px[0..4], &[0, 255, 7, 255]);
        let a = parse(&sample(1, 1, false), false).unwrap();
        assert_eq!(a.rgba(0).unwrap().2[3], 255, "additive: brightness as alpha");
        let ia = parse(&sample(2, 1, false), false).unwrap();
        assert_eq!(&ia.rgba(0).unwrap().2[4..8], &[255, 0, 7, 255], "indexalpha: last color, index as alpha");
        assert_eq!(parse(&sample(0, 3, false), true).unwrap().frames.len(), 1);
    }

    #[test]
    fn refuses_damaged_or_huge() {
        let ok = sample(0, 1, false);
        assert!(parse(&ok[..ok.len() - 1], false).is_err());
        assert!(parse(b"IDSQ\x02\0\0\0", false).is_err());
        let mut big = ok.clone();
        big[20..24].copy_from_slice(&100_000i32.to_le_bytes());
        assert!(parse(&big, false).is_err());
        let mut many = ok.clone();
        many[28..32].copy_from_slice(&1_000_000i32.to_le_bytes());
        assert!(parse(&many, false).is_err());
    }
}
