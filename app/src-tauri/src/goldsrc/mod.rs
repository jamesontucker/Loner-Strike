//! Readers for GoldSrc's files (BSP30 maps, WAD3 texture files, entity text) and the
//! processed mesh the Tools page draws. Clean-room: written from the formats and our own
//! Python tools in loner-strike-sim, not from Valve's SDK.
//!
//! Every offset that comes from a file is checked before it's used: a damaged or cut-off
//! file gives an Error the page can show, never a panic.

pub mod bsp;
pub mod entities;
pub mod lightmap;
pub mod mesh;
pub mod wad;

use std::fmt;

/// Bumped whenever a parser or the bundle changes what it produces: cached results made by
/// an older version are rebuilt.
pub const PARSER_VERSION: u32 = 1;

/// The biggest map file we read (the biggest stock map is ~30 MB).
pub const MAX_BSP: u64 = 64 << 20;
/// The biggest texture file we open (halflife.wad is ~20 MB).
pub const MAX_WAD: u64 = 256 << 20;
/// The biggest texture side we decode (GoldSrc itself stops at 512; some tools allow more).
pub const MAX_TEX_SIDE: u32 = 1024;

/// What went wrong reading a file, in words the page can show.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Not the format we expected (`what` says which): e.g. a Quake or Source map.
    NotFormat { what: &'static str, detail: String },
    /// The file is cut off or its numbers point outside it.
    Damaged { what: &'static str, detail: String },
    /// Bigger than we're willing to read.
    TooBig { what: &'static str, size: u64, max: u64 },
    /// Couldn't read it from the disk.
    Io(String),
}

impl Error {
    pub fn damaged(what: &'static str, detail: impl Into<String>) -> Error {
        Error::Damaged { what, detail: detail.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFormat { what, detail } => write!(f, "This {what} isn't one Counter-Strike 1.6 can read ({detail})."),
            Error::Damaged { what, detail } => write!(f, "This {what} is damaged ({detail}). Downloading it again usually fixes that."),
            Error::TooBig { what, size, max } => {
                write!(f, "This {what} is too big to open here ({} MB; the limit is {} MB).", size >> 20, max >> 20)
            }
            Error::Io(e) => write!(f, "Couldn't read the file: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Little-endian reads from a byte slice, each one bounds-checked. `what` names the file
/// kind for the error ("map file", "texture file").
#[derive(Clone, Copy)]
pub struct Le<'a> {
    pub b: &'a [u8],
    pub what: &'static str,
}

impl<'a> Le<'a> {
    pub fn new(b: &'a [u8], what: &'static str) -> Le<'a> {
        Le { b, what }
    }

    pub fn len(&self) -> usize {
        self.b.len()
    }

    pub fn bytes(&self, at: usize, n: usize) -> Result<&'a [u8]> {
        at.checked_add(n)
            .and_then(|end| self.b.get(at..end))
            .ok_or_else(|| Error::damaged(self.what, format!("{n} bytes at {at} are past its end ({} bytes)", self.b.len())))
    }

    fn arr<const N: usize>(&self, at: usize) -> Result<[u8; N]> {
        Ok(self.bytes(at, N)?.try_into().unwrap())
    }

    pub fn u8(&self, at: usize) -> Result<u8> {
        Ok(self.arr::<1>(at)?[0])
    }
    pub fn u16(&self, at: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(self.arr(at)?))
    }
    pub fn i16(&self, at: usize) -> Result<i16> {
        Ok(i16::from_le_bytes(self.arr(at)?))
    }
    pub fn u32(&self, at: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.arr(at)?))
    }
    pub fn i32(&self, at: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(self.arr(at)?))
    }
    pub fn f32(&self, at: usize) -> Result<f32> {
        Ok(f32::from_le_bytes(self.arr(at)?))
    }
    pub fn vec3(&self, at: usize) -> Result<[f32; 3]> {
        Ok([self.f32(at)?, self.f32(at + 4)?, self.f32(at + 8)?])
    }
}

/// A fixed-size, NUL-padded name (texture names, WAD entries) as text: Latin-1, stopping
/// at the first NUL.
pub fn cname(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect()
}

/// Reads a whole file, refusing ones over `max`.
pub fn read_capped(path: &std::path::Path, what: &'static str, max: u64) -> Result<Vec<u8>> {
    let size = std::fs::metadata(path)?.len();
    if size > max {
        return Err(Error::TooBig { what, size, max });
    }
    Ok(std::fs::read(path)?)
}

/// A PNG from RGBA (`alpha`) or RGB pixels, rows from the top.
pub fn png(width: u32, height: u32, pixels: &[u8], alpha: bool) -> Result<Vec<u8>> {
    use image::ImageEncoder;
    let kind = if alpha { image::ExtendedColorType::Rgba8 } else { image::ExtendedColorType::Rgb8 };
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Adaptive)
        .write_image(pixels, width, height, kind)
        .map_err(|e| Error::Io(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_checked() {
        let r = Le::new(&[1, 0, 0, 0, 0, 0, 128, 63], "map file");
        assert_eq!(r.u32(0), Ok(1));
        assert_eq!(r.f32(4), Ok(1.0));
        assert!(r.u32(6).is_err());
        assert!(r.bytes(usize::MAX, 2).is_err());
        let e = r.u16(8).unwrap_err().to_string();
        assert!(e.starts_with("This map file is damaged"), "{e}");
    }

    #[test]
    fn names_stop_at_nul() {
        assert_eq!(cname(b"{fence\0junk"), "{fence");
        assert_eq!(cname(b"abc"), "abc");
    }
}
pub mod hull;
pub mod nav;
pub mod lsv;
