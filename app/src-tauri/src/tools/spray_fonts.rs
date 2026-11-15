//! The fonts installed on this computer, for the spray composer's lettering (macOS, Windows,
//! and Linux too). No font library: the font folders are listed and each file's `name` table
//! is read for its family and style, which is all the picker needs.
//!
//! - `GET /api/tools/spray/fonts`: the families, sorted, with their styles.
//! - `GET /api/tools/spray/font?family=…`: that family's regular face as a single TTF/OTF
//!   (a face inside a .ttc collection is copied out into a font of its own, since browsers
//!   don't all load collections). Only families found by the scan are served, so this can't
//!   be used to read other files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One face: where it is and its place in the file (collections hold several).
#[derive(Debug, Clone)]
pub struct Face {
    pub path: PathBuf,
    pub index: u32,
    pub family: String,
    pub style: String,
}

/// The folders fonts are installed in on this system.
fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home = dirs::home_dir();
    if cfg!(target_os = "macos") {
        v.extend(["/System/Library/Fonts", "/System/Library/Fonts/Supplemental", "/Library/Fonts"].map(PathBuf::from));
        if let Some(h) = &home {
            v.push(h.join("Library/Fonts"));
        }
    } else if cfg!(windows) {
        let win = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        v.push(win.join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            v.push(PathBuf::from(l).join(r"Microsoft\Windows\Fonts"));
        }
    } else {
        v.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        if let Some(h) = &home {
            v.push(h.join(".local/share/fonts"));
            v.push(h.join(".fonts"));
        }
    }
    v
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Where each face's table directory starts (one for a plain font, several for a .ttc).
fn face_offsets(b: &[u8]) -> Vec<usize> {
    match b.get(0..4) {
        Some(b"ttcf") => {
            let n = be32(b, 8).unwrap_or(0).min(64) as usize;
            (0..n).filter_map(|i| be32(b, 12 + i * 4).map(|o| o as usize)).collect()
        }
        Some([0, 1, 0, 0]) | Some(b"OTTO") | Some(b"true") => vec![0],
        _ => vec![],
    }
}

/// (offset, length) of a table in the face whose directory starts at `dir`.
fn table(b: &[u8], dir: usize, tag: &[u8; 4]) -> Option<(usize, usize)> {
    let n = be16(b, dir + 4)? as usize;
    (0..n.min(256)).find_map(|i| {
        let rec = dir + 12 + i * 16;
        (b.get(rec..rec + 4)? == tag).then(|| Some((be32(b, rec + 8)? as usize, be32(b, rec + 12)? as usize)))?
    })
}

/// A name from the `name` table: typographic family/subfamily (16/17) if there, else 1/2.
/// Windows-platform English names first (UTF-16), then Mac Roman ones.
fn names(b: &[u8], dir: usize) -> Option<(String, String)> {
    let (off, len) = table(b, dir, b"name")?;
    let t = b.get(off..off + len)?;
    let count = be16(t, 2)? as usize;
    let strings = be16(t, 4)? as usize;
    let mut found: BTreeMap<(u16, u8), String> = BTreeMap::new();
    for i in 0..count.min(2048) {
        let r = 6 + i * 12;
        let (platform, encoding, lang, id) = (be16(t, r)?, be16(t, r + 2)?, be16(t, r + 4)?, be16(t, r + 6)?);
        let (l, o) = (be16(t, r + 8)? as usize, be16(t, r + 10)? as usize);
        if ![1, 2, 16, 17].contains(&id) {
            continue;
        }
        let Some(raw) = t.get(strings + o..strings + o + l) else { continue };
        let (rank, text) = match (platform, encoding) {
            (3, 0 | 1 | 10) => {
                let u: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                (if lang == 0x409 { 0 } else { 2 }, String::from_utf16_lossy(&u))
            }
            (1, 0) if lang == 0 => (1, raw.iter().map(|&c| if c.is_ascii() { c as char } else { '?' }).collect()),
            (0, _) => {
                let u: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                (3, String::from_utf16_lossy(&u))
            }
            _ => continue,
        };
        let text = text.trim().to_string();
        if !text.is_empty() {
            found.entry((id, rank)).or_insert(text);
        }
    }
    let pick = |ids: [u16; 2]| ids.iter().find_map(|id| (0..4).find_map(|r| found.get(&(*id, r)).cloned()));
    Some((pick([16, 1])?, pick([17, 2]).unwrap_or_else(|| "Regular".into())))
}

fn is_font_file(p: &Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("ttf" | "otf" | "ttc"))
}

/// Every face in the font folders (hidden families, whose names start with "." on macOS, left out).
pub fn scan() -> Vec<Face> {
    let mut faces = Vec::new();
    let mut stack: Vec<(PathBuf, u8)> = font_dirs().into_iter().map(|d| (d, 0)).collect();
    let mut files = 0;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() && depth < 4 {
                stack.push((p, depth + 1));
            } else if ft.is_file() && is_font_file(&p) && files < 5000 {
                files += 1;
                // big files are mostly CJK collections: fine to read once (tens of MB at most)
                let Ok(meta) = e.metadata() else { continue };
                if meta.len() > 80 << 20 {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&p) else { continue };
                for (i, dir_at) in face_offsets(&bytes).into_iter().enumerate() {
                    if let Some((family, style)) = names(&bytes, dir_at) {
                        if !family.starts_with('.') {
                            faces.push(Face { path: p.clone(), index: i as u32, family, style });
                        }
                    }
                }
            }
        }
    }
    faces
}

static CACHE: Mutex<Option<Vec<Face>>> = Mutex::new(None);

/// The scan, done once per run (the picker asks again only after a restart of the app).
pub fn faces() -> Vec<Face> {
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    c.get_or_insert_with(scan).clone()
}

/// Families (sorted, case-insensitive) with their styles.
pub fn families(faces: &[Face]) -> Vec<(String, Vec<String>)> {
    let mut m: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for f in faces {
        let e = m.entry(f.family.to_lowercase()).or_insert_with(|| (f.family.clone(), vec![]));
        if !e.1.contains(&f.style) {
            e.1.push(f.style.clone());
        }
    }
    m.into_values().collect()
}

/// The face to draw a family with: Regular (or Book, Roman, Medium…) before bold or italic.
pub fn regular<'a>(faces: &'a [Face], family: &str) -> Option<&'a Face> {
    let rank = |s: &str| {
        let s = s.to_lowercase();
        ["regular", "normal", "book", "roman", "plain", "standard", "medium"].iter().position(|w| s == *w).unwrap_or(if s.contains("italic") || s.contains("oblique") { 20 } else if s.contains("bold") || s.contains("black") { 15 } else { 10 })
    };
    faces.iter().filter(|f| f.family.eq_ignore_ascii_case(family)).min_by_key(|f| (rank(&f.style), f.path.clone()))
}

/// The face as a font file of its own: the file as it is, or (from a collection) its tables
/// copied into a new single font.
pub fn face_bytes(face: &Face) -> Result<Vec<u8>, String> {
    let b = std::fs::read(&face.path).map_err(|e| format!("Couldn't read the font: {e}"))?;
    let offs = face_offsets(&b);
    let dir = *offs.get(face.index as usize).ok_or("That font file has changed. Restart Loner-Strike to list fonts again.")?;
    if b.get(0..4) != Some(b"ttcf") {
        return Ok(b);
    }
    single_font(&b, dir).ok_or_else(|| "Couldn't read that font.".into())
}

fn single_font(b: &[u8], dir: usize) -> Option<Vec<u8>> {
    let n = be16(b, dir + 4)? as usize;
    if n == 0 || n > 256 {
        return None;
    }
    let mut tables = Vec::with_capacity(n);
    for i in 0..n {
        let rec = dir + 12 + i * 16;
        let rec_bytes = b.get(rec..rec + 16)?;
        let (off, len) = (be32(b, rec + 8)? as usize, be32(b, rec + 12)? as usize);
        tables.push((rec_bytes, b.get(off..off.checked_add(len)?)?));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b.get(dir..dir + 12)?);
    let mut at = 12 + 16 * n;
    let mut data = Vec::new();
    for (rec, t) in &tables {
        out.extend_from_slice(&rec[0..8]); // tag, checksum
        out.extend_from_slice(&(at as u32).to_be_bytes());
        out.extend_from_slice(&(t.len() as u32).to_be_bytes());
        data.extend_from_slice(t);
        while data.len() % 4 != 0 {
            data.push(0);
        }
        at = 12 + 16 * n + data.len();
    }
    out.extend_from_slice(&data);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny font: a `name` table (Windows English family + style) and one other table.
    fn font(family: &str, style: &str) -> Vec<u8> {
        let enc = |s: &str| s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect::<Vec<u8>>();
        let (f, s) = (enc(family), enc(style));
        let mut name = vec![];
        name.extend(0u16.to_be_bytes());
        name.extend(2u16.to_be_bytes());
        name.extend((6u16 + 24).to_be_bytes());
        for (id, len, off) in [(1u16, f.len(), 0usize), (2, s.len(), f.len())] {
            for v in [3u16, 1, 0x409, id, len as u16, off as u16] {
                name.extend(v.to_be_bytes());
            }
        }
        name.extend(&f);
        name.extend(&s);
        let other = b"abcdefg".to_vec();
        let n = 2u16;
        let mut out = vec![0, 1, 0, 0];
        out.extend(n.to_be_bytes());
        out.extend([0u8; 6]);
        let start = 12 + 16 * 2;
        let name_at = start;
        let other_at = start + ((name.len() + 3) & !3);
        for (tag, at, t) in [(b"name", name_at, &name), (b"zzzz", other_at, &other)] {
            out.extend(tag);
            out.extend(0u32.to_be_bytes());
            out.extend((at as u32).to_be_bytes());
            out.extend((t.len() as u32).to_be_bytes());
        }
        out.extend(&name);
        out.resize(other_at, 0);
        out.extend(&other);
        out
    }

    #[test]
    fn reads_names_and_copies_faces_out_of_collections() {
        let a = font("Test Sans", "Regular");
        let b = font("Test Serif", "Bold");
        assert_eq!(names(&a, 0), Some(("Test Sans".into(), "Regular".into())));
        // a collection of both, faces at their own offsets
        let mut ttc = b"ttcf".to_vec();
        ttc.extend([0, 1, 0, 0]);
        ttc.extend(2u32.to_be_bytes());
        let base = 12 + 8;
        ttc.extend((base as u32).to_be_bytes());
        ttc.extend(((base + a.len()) as u32).to_be_bytes());
        // the faces' table offsets are from the start of the file: shift them
        for (f, at) in [(&a, base), (&b, base + a.len())] {
            let mut g = f.clone();
            for i in 0..2 {
                let rec = 12 + i * 16 + 8;
                let o = be32(&g, rec).unwrap() as usize + at;
                g[rec..rec + 4].copy_from_slice(&(o as u32).to_be_bytes());
            }
            ttc.extend(g);
        }
        let offs = face_offsets(&ttc);
        assert_eq!(offs.len(), 2);
        assert_eq!(names(&ttc, offs[1]), Some(("Test Serif".into(), "Bold".into())));
        let one = single_font(&ttc, offs[1]).unwrap();
        assert_eq!(face_offsets(&one), vec![0]);
        assert_eq!(names(&one, 0), Some(("Test Serif".into(), "Bold".into())));
        let (o, l) = table(&one, 0, b"zzzz").unwrap();
        assert_eq!(&one[o..o + l], b"abcdefg");
        // nonsense is no font
        assert!(face_offsets(b"hello").is_empty());
        assert_eq!(names(&[0, 1, 0, 0, 0, 9], 0), None);
    }

    #[test]
    fn regular_face_first() {
        let f = |style: &str| Face { path: PathBuf::from(format!("/f/{style}.ttf")), index: 0, family: "Fam".into(), style: style.into() };
        let faces = vec![f("Bold"), f("Italic"), f("Regular"), f("Light")];
        assert_eq!(regular(&faces, "fam").unwrap().style, "Regular");
        assert_eq!(families(&faces), vec![("Fam".to_string(), vec!["Bold".into(), "Italic".into(), "Regular".into(), "Light".into()])]);
        assert!(regular(&faces, "Other").is_none());
    }

    #[test]
    fn this_computer_has_fonts() {
        // the machine running the tests (a Mac or Windows build box) has some installed
        if cfg!(any(target_os = "macos", windows)) {
            let all = scan();
            assert!(!all.is_empty(), "no fonts found in {:?}", font_dirs());
            let fam = families(&all);
            let r = regular(&all, &fam[0].0).unwrap();
            let bytes = face_bytes(r).unwrap();
            assert_eq!(face_offsets(&bytes), vec![0]);
        }
    }
}
