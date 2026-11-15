//! WAD3 texture files and miptex decoding.
//! - header 12: "WAD3", i32 entry count, i32 directory offset
//! - directory entry 32: i32 offset, i32 size on disk, i32 size, u8 type, u8 compression,
//!   u16 padding, name[16]
//! - miptex (type 0x43; decals 0x40 use the same layout): name[16], u32 width, u32 height,
//!   u32 mip offsets[4], the pixels (palette indices), then after the smallest mip u16
//!   colour count and 256 RGB colours.
//!
//! Textures whose name starts with `{` are see-through where the palette index is 255.

use super::{cname, Error, Le, Result};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const WHAT: &str = "texture file";
const TYPE_MIPTEX: u8 = 0x43;
const TYPE_DECAL: u8 = 0x40;
/// Most entries one WAD may list (halflife.wad has ~3000).
const MAX_ENTRIES: usize = 1 << 16;
/// Biggest single texture record we read.
const MAX_ENTRY: u32 = 32 << 20;

/// A decoded texture: RGBA, row by row from the top.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Decodes a miptex record (from a map or a WAD) to RGBA. `name` decides `{` transparency
/// (the record's own name is used when `name` is empty).
pub fn decode_miptex(rec: &[u8], name: &str) -> Result<Rgba> {
    let what = "texture";
    let r = Le::new(rec, what);
    let own = cname(r.bytes(0, 16)?);
    let name = if name.is_empty() { own.as_str() } else { name };
    let (w, h) = (r.u32(16)?, r.u32(20)?);
    if w == 0 || h == 0 || w > super::MAX_TEX_SIDE || h > super::MAX_TEX_SIDE {
        return Err(Error::damaged(what, format!("its size is {w}×{h}")));
    }
    let off0 = r.u32(24)? as usize;
    if off0 == 0 {
        return Err(Error::damaged(what, "its pixels aren't in it"));
    }
    let (wu, hu) = (w as usize, h as usize);
    let pixels = r.bytes(off0, wu * hu)?;
    // the palette follows the smallest mip; when that offset is off, assume the usual layout
    let off3 = r.u32(36)? as usize;
    let after3 = off3.checked_add((wu / 8) * (hu / 8)).filter(|&a| off3 != 0 && a + 2 + 768 <= rec.len());
    let pal_at = after3.unwrap_or(off0 + wu * hu + wu * hu / 4 + wu * hu / 16 + wu * hu / 64) + 2;
    let palette = r.bytes(pal_at, 768)?;
    let clear = name.starts_with('{');
    let mut out = Vec::with_capacity(wu * hu * 4);
    for &i in pixels {
        if clear && i == 255 {
            out.extend([0, 0, 0, 0]);
        } else {
            let c = &palette[i as usize * 3..i as usize * 3 + 3];
            out.extend([c[0], c[1], c[2], 255]);
        }
    }
    Ok(Rgba { width: w, height: h, pixels: out })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry {
    pub offset: u32,
    pub disk_size: u32,
    pub size: u32,
    pub kind: u8,
    pub compression: u8,
}

/// An open WAD: its directory (names lower-cased), read once; textures read on demand.
#[derive(Debug)]
pub struct Wad {
    pub path: PathBuf,
    pub entries: HashMap<String, Entry>,
    size: u64,
}

impl Wad {
    pub fn open(path: &Path) -> Result<Wad> {
        let mut f = std::fs::File::open(path)?;
        let size = f.metadata()?.len();
        if size > super::MAX_WAD {
            return Err(Error::TooBig { what: WHAT, size, max: super::MAX_WAD });
        }
        let mut head = [0u8; 12];
        f.read_exact(&mut head).map_err(|_| Error::damaged(WHAT, "it's shorter than its header"))?;
        if &head[0..4] != b"WAD3" {
            let detail = if &head[0..4] == b"WAD2" { "it's a Quake texture file".to_string() } else { "no WAD3 header".to_string() };
            return Err(Error::NotFormat { what: WHAT, detail });
        }
        let r = Le::new(&head, WHAT);
        let (n, dir) = (r.i32(4)?, r.i32(8)?);
        if n < 0 || n as usize > MAX_ENTRIES || dir < 0 || dir as u64 + n as u64 * 32 > size {
            return Err(Error::damaged(WHAT, "its directory points outside the file"));
        }
        let mut d = vec![0u8; n as usize * 32];
        f.seek(SeekFrom::Start(dir as u64))?;
        f.read_exact(&mut d)?;
        let r = Le::new(&d, WHAT);
        let mut entries = HashMap::with_capacity(n as usize);
        for i in 0..n as usize {
            let o = i * 32;
            let e = Entry {
                offset: r.u32(o)?,
                disk_size: r.u32(o + 4)?,
                size: r.u32(o + 8)?,
                kind: r.u8(o + 12)?,
                compression: r.u8(o + 13)?,
            };
            let name = cname(r.bytes(o + 16, 16)?).to_lowercase();
            // the first of a repeated name wins, as in the engine
            entries.entry(name).or_insert(e);
        }
        Ok(Wad { path: path.to_path_buf(), entries, size })
    }

    pub fn has_texture(&self, name: &str) -> bool {
        self.entries.get(&name.to_lowercase()).is_some_and(|e| e.kind == TYPE_MIPTEX || e.kind == TYPE_DECAL)
    }

    /// A texture's miptex record, or None when this WAD doesn't have it.
    pub fn read_miptex(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(e) = self.entries.get(&name.to_lowercase()) else { return Ok(None) };
        if e.kind != TYPE_MIPTEX && e.kind != TYPE_DECAL {
            return Ok(None);
        }
        if e.compression != 0 {
            return Err(Error::NotFormat { what: WHAT, detail: format!("{name} is compressed") });
        }
        if e.disk_size > MAX_ENTRY || e.offset as u64 + e.disk_size as u64 > self.size {
            return Err(Error::damaged(WHAT, format!("{name} points outside the file")));
        }
        let mut f = std::fs::File::open(&self.path)?;
        f.seek(SeekFrom::Start(e.offset as u64))?;
        let mut b = vec![0u8; e.disk_size as usize];
        f.read_exact(&mut b)?;
        Ok(Some(b))
    }
}

/// The game folders WADs are looked for in, in order: the player's game first, then
/// cstrike, valve and czero (base names only, never a path from a map or a request).
pub fn search_dirs(hl: &Path, czero: bool) -> Vec<PathBuf> {
    let order: &[&str] = if czero { &["czero", "cstrike", "valve", "czero_downloads", "cstrike_downloads"] } else { &["cstrike", "valve", "czero", "cstrike_downloads", "czero_downloads"] };
    order.iter().map(|g| hl.join(g)).filter(|d| d.is_dir()).collect()
}

/// The WADs a map names, each found (and opened) or not.
pub struct WadSet {
    pub wads: Vec<(String, std::result::Result<Wad, String>)>,
}

/// Where a texture comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Embedded,
    /// the WAD's base name as the map names it
    Wad(String),
    Missing,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Embedded => "embedded".into(),
            Source::Wad(w) => w.clone(),
            Source::Missing => "missing".into(),
        }
    }
}

impl WadSet {
    /// Finds each named WAD in `dirs` (whatever its case) and reads its directory.
    pub fn open(finder: &crate::mapcopy::Finder, dirs: &[PathBuf], names: &[String]) -> WadSet {
        let wads = names
            .iter()
            .map(|n| {
                let found = dirs.iter().find_map(|d| finder.find(d, n));
                let w = match found {
                    None => Err("missing".to_string()),
                    Some(p) => Wad::open(&p).map_err(|e| e.to_string()),
                };
                (n.clone(), w)
            })
            .collect();
        WadSet { wads }
    }

    /// The WADs a map names that aren't on this computer (or can't be read).
    pub fn missing(&self) -> Vec<String> {
        self.wads.iter().filter(|(_, w)| w.is_err()).map(|(n, _)| n.clone()).collect()
    }

    /// Which named WAD has the texture first.
    pub fn source_of(&self, name: &str) -> Source {
        self.wads
            .iter()
            .find(|(_, w)| w.as_ref().is_ok_and(|w| w.has_texture(name)))
            .map(|(n, _)| Source::Wad(n.clone()))
            .unwrap_or(Source::Missing)
    }

    /// The texture's miptex record from the first WAD that has it.
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        for w in self.wads.iter().filter_map(|(_, w)| w.as_ref().ok()) {
            if let Some(b) = w.read_miptex(name)? {
                return Ok(Some(b));
            }
        }
        Ok(None)
    }
}

/// Builds a WAD3 file from miptex records (tests and fixtures).
#[cfg(test)]
pub fn build_wad(records: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    let mut dir: Vec<u8> = Vec::new();
    for (name, rec) in records {
        let at = 12 + body.len() as u32;
        body.extend(rec);
        dir.extend(at.to_le_bytes());
        dir.extend((rec.len() as u32).to_le_bytes());
        dir.extend((rec.len() as u32).to_le_bytes());
        dir.extend([TYPE_MIPTEX, 0, 0, 0]);
        let mut n = [0u8; 16];
        n[..name.len()].copy_from_slice(name.as_bytes());
        dir.extend(n);
    }
    let mut out = b"WAD3".to_vec();
    out.extend((records.len() as i32).to_le_bytes());
    out.extend((12 + body.len() as i32).to_le_bytes());
    out.extend(body);
    out.extend(dir);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture::miptex;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-wad-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn decodes_with_transparency() {
        let rec = miptex("{grate", true);
        let t = decode_miptex(&rec, "").unwrap();
        assert_eq!((t.width, t.height), (16, 16));
        // pixel (x, y) = index (x + y), palette i = (i, i, i): nothing is index 255 here
        assert_eq!(&t.pixels[4 * 3..4 * 3 + 4], &[3, 3, 3, 255]);
        // make one pixel index 255: see-through for `{`, opaque grey otherwise
        let mut rec = rec;
        rec[40] = 255;
        assert_eq!(&decode_miptex(&rec, "").unwrap().pixels[0..4], &[0, 0, 0, 0]);
        assert_eq!(&decode_miptex(&rec, "brick").unwrap().pixels[0..4], &[255, 255, 255, 255]);
        // header only, or cut off: an error
        assert!(decode_miptex(&miptex("x", false), "").is_err());
        for n in [0, 20, 40, 300, rec.len() - 100] {
            assert!(decode_miptex(&rec[..n], "").is_err(), "{n}");
        }
    }

    #[test]
    fn wad_directory_and_lookup() {
        let d = temp("set");
        let cs = d.join("cstrike");
        let valve = d.join("valve");
        std::fs::create_dir_all(&cs).unwrap();
        std::fs::create_dir_all(&valve).unwrap();
        std::fs::write(valve.join("Test.WAD"), build_wad(&[("{GRATE", miptex("{grate", true)), ("other", miptex("other", true))])).unwrap();
        std::fs::write(cs.join("broken.wad"), b"WAD3\xff\xff\xff\x7f\0\0\0\0").unwrap();
        let w = Wad::open(&valve.join("Test.WAD")).unwrap();
        assert!(w.has_texture("{grate"));
        assert!(w.read_miptex("nope").unwrap().is_none());

        let f = crate::mapcopy::Finder::default();
        let dirs = search_dirs(&d, false);
        assert_eq!(dirs, vec![cs.clone(), valve.clone()]);
        let set = WadSet::open(&f, &dirs, &["broken.wad".into(), "test.wad".into(), "missing.wad".into()]);
        assert_eq!(set.missing(), vec!["broken.wad".to_string(), "missing.wad".to_string()]);
        assert_eq!(set.source_of("{Grate"), Source::Wad("test.wad".into()));
        assert_eq!(set.source_of("brick"), Source::Missing);
        let rec = set.read("{grate").unwrap().unwrap();
        let t = decode_miptex(&rec, "{grate").unwrap();
        assert_eq!(t.pixels.len(), 16 * 16 * 4);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn bad_wads_are_errors() {
        let d = temp("bad");
        let p = d.join("x.wad");
        for bytes in [&b"WAD2\0\0\0\0\0\0\0\0"[..], b"WAD3", b"WAD3\x01\0\0\0\xf0\xff\0\0"] {
            std::fs::write(&p, bytes).unwrap();
            assert!(Wad::open(&p).is_err());
        }
        // an entry pointing past the end
        let mut w = build_wad(&[("a", miptex("a", true))]);
        let dir = w.len() - 32;
        w[dir + 4..dir + 8].copy_from_slice(&(1u32 << 20).to_le_bytes());
        std::fs::write(&p, &w).unwrap();
        assert!(Wad::open(&p).unwrap().read_miptex("a").is_err());
        std::fs::remove_dir_all(&d).ok();
    }
}
