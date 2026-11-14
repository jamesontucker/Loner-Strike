//! P5: a map's editable source in its community package, and Remix.
//!
//! With "Include the editable source" on, a map package gets `source/`:
//!   - `<map>.lsmap`, the editor project (its WAD list pointing at the game's WADs and the one below);
//!   - `<map>.map`, a Valve 220 export for Hammer, J.A.C.K. and TrenchBroom;
//!   - `<map>_custom.wad`, only the textures the map uses that aren't the game's own (never
//!     Valve's or CS 1.6's: those every player has);
//!   - `README-source.txt`.
//! "Remix this map" on a community map with source reads that folder back (checked: sizes, a
//! plain JSON project, a plain WAD of textures, nothing else), saves it as a new project
//! `<map>_remix` (never over one that's there) with the credit, and opens it in the editor.

use crate::editor::doc::{MapDocument, RemixCredit};
use crate::editor::mapfile;
use crate::goldsrc::{wad, wadwrite};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The most a shared project may be (the editor opens up to 64 MB).
pub const MAX_PROJECT: u64 = 64 << 20;
/// The most the custom texture file may be.
pub const MAX_SOURCE_WAD: u64 = 64 << 20;
/// A project bigger than this in brushes or entities isn't one the app made.
const MAX_BRUSHES: usize = 200_000;
const MAX_ENTITIES: usize = 50_000;

/// What goes in a package's source/ folder.
#[derive(Debug, Default)]
pub struct Source {
    /// (path inside the package, bytes)
    pub files: Vec<(String, Vec<u8>)>,
    /// the custom textures shipped in the WAD
    pub textures: Vec<String>,
    /// textures the map uses that aren't the game's and weren't found in any WAD here
    pub missing: Vec<String>,
    /// the project's own remix credit (it was remixed from a community map)
    pub remix_of: Option<RemixCredit>,
}

/// A WAD named by a document, as a plain file name (`C:\x\mine.wad` -> `mine.wad`, a UNC
/// `\\host\share\a.wad` -> `a.wad`); None when nothing safe is left (no share, drive or folder
/// ever survives).
pub fn wad_base_name(w: &str) -> Option<String> {
    let base = w.rsplit(['\\', '/']).next().unwrap_or("").trim();
    let ok = !base.is_empty()
        && base.len() <= 64
        && base.to_ascii_lowercase().ends_with(".wad")
        && !base.starts_with('.')
        && base.chars().all(|c| c.is_ascii_alphanumeric() || "_-. ".contains(c));
    ok.then(|| base.to_string())
}

/// A key value that's a path on someone's computer (a drive, a UNC share, a leading slash, ~).
fn absolute_path(v: &str) -> bool {
    let b = v.as_bytes();
    v.starts_with('/') || v.starts_with('\\') || v.starts_with("~/") || v.starts_with("~\\") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
}

/// The one sanitizer for a shared project, both ways (P5, security review): what Publish puts in
/// a package and what Remix takes from one. The editor settings come from an allowlist (no FGD
/// path: it's a file on the author's computer, or a UNC share on the remixer's); every WAD,
/// in the WAD list and in the texture table, is a plain file name the editor then looks up only
/// in its own folders (the project's, the game's); an entity key that holds an absolute path
/// keeps only the part from the game folder on (or the file name).
pub fn sanitize(doc: &mut MapDocument) {
    let old = std::mem::take(&mut doc.editor);
    doc.editor = crate::editor::doc::EditorSettings { vis_sets: old.vis_sets, cordon: old.cordon, remix_of: old.remix_of, ..Default::default() };
    let mut wads: Vec<String> = Vec::new();
    for w in doc.world.wads.iter().filter_map(|w| wad_base_name(w)) {
        if !wads.iter().any(|x| x.eq_ignore_ascii_case(&w)) {
            wads.push(w);
        }
    }
    doc.world.wads = wads;
    for m in doc.materials.values_mut() {
        m.wad = m.wad.as_deref().and_then(wad_base_name);
    }
    let rel = |v: &str| -> String {
        let norm = v.replace('\\', "/");
        let low = norm.to_ascii_lowercase();
        for g in ["/cstrike/", "/czero/", "/valve/", "/cstrike_downloads/", "/czero_downloads/", "/valve_downloads/"] {
            if let Some(i) = low.rfind(g) {
                return norm[i + g.len()..].to_string();
            }
        }
        norm.rsplit('/').next().unwrap_or("").to_string()
    };
    for (_, v) in doc.world.keys.iter_mut() {
        if absolute_path(v) {
            *v = rel(v);
        }
    }
    for e in doc.entities.values_mut() {
        for (_, v) in e.keys.iter_mut() {
            if absolute_path(v) {
                *v = rel(v);
            }
        }
    }
}

/// The textures a document's brushes use (lower case, without the editor's own special names).
pub fn used_textures(doc: &MapDocument) -> BTreeSet<String> {
    doc.brushes.values().flat_map(|b| b.faces.iter().map(|f| f.texture.to_ascii_lowercase())).filter(|t| !t.is_empty() && t.len() <= 15).collect()
}

/// Builds the source folder for map `name` from the project at `project`: the game is in `hl`
/// (WADs are found in the project's folder, then the game's folders).
pub fn build(project: &Path, hl: &Path, czero: bool, name: &str) -> Result<Source, String> {
    if !project.extension().is_some_and(|e| e.eq_ignore_ascii_case("lsmap")) {
        return Err("The editable source has to be a Loner-Strike map project (.lsmap).".into());
    }
    let len = std::fs::metadata(project).map_err(|_| "The map's project file isn't there any more. Pick it again.".to_string())?.len();
    if len > MAX_PROJECT {
        return Err("The map's project is over 64 MB, too big to share as source.".into());
    }
    let (mut doc, is_project) = super::editor::load_file(project)?;
    if !is_project {
        return Err("The editable source has to be a Loner-Strike map project (.lsmap).".into());
    }
    let remix_of = doc.editor.remix_of.clone();
    // the textures: the game's own WADs (by name) are left to the player's game; every other
    // texture the map uses goes into one WAD of its own
    let finder = crate::mapcopy::Finder::default();
    let mut dirs: Vec<PathBuf> = project.parent().map(|d| vec![d.to_path_buf()]).unwrap_or_default();
    dirs.extend(wad::search_dirs(hl, czero));
    let names = doc.world.wad_names();
    let game = if czero { "czero" } else { "cstrike" };
    let stock_names = super::doctor::stock_wad_names(game);
    let stock = wad::WadSet::open(&finder, &dirs, &stock_names);
    let custom_names: Vec<String> = names.iter().filter(|n| !super::doctor::is_stock(n, game)).cloned().collect();
    let custom = wad::WadSet::open(&finder, &dirs, &custom_names);
    let mut lumps: Vec<wadwrite::Lump> = Vec::new();
    let (mut textures, mut missing) = (Vec::new(), Vec::new());
    let mut size: u64 = 0;
    for t in used_textures(&doc) {
        if stock.wads.iter().any(|(_, w)| w.as_ref().is_ok_and(|w| w.has_texture(&t))) {
            continue;
        }
        match custom.read(&t).map_err(|e| e.to_string())? {
            Some(rec) => {
                size += rec.len() as u64;
                if size > MAX_SOURCE_WAD {
                    return Err("The map's own textures are over 64 MB together, too big to share as source.".into());
                }
                lumps.push(wadwrite::Lump { name: t.clone(), kind: wadwrite::TYPE_MIPTEX, data: rec });
                textures.push(t);
            }
            None => missing.push(t),
        }
    }
    // the project and the .map name the game's WADs it uses, plus the one with its own textures
    let wad_name = format!("{name}_custom.wad");
    let mut wads: Vec<String> = names.iter().filter(|n| super::doctor::is_stock(n, game)).cloned().collect();
    if !lumps.is_empty() {
        wads.push(wad_name.clone());
    }
    doc.world.wads = wads;
    // (no path from the author's computer goes out; a shared project gets no id of its own, so a
    // remix of it is never taken for the author's)
    sanitize(&mut doc);
    let map_text = mapfile::write_map(&doc, &mapfile::WriteOptions::default())?;
    let (map_bytes, _, _) = mapfile::encode_for_game(&map_text);
    let mut files = vec![(format!("source/{name}.lsmap"), doc.to_json().into_bytes()), (format!("source/{name}.map"), map_bytes)];
    if !lumps.is_empty() {
        files.push((format!("source/{wad_name}"), wadwrite::write_lumps(&lumps)));
    }
    files.push(("source/README-source.txt".into(), readme(name, !lumps.is_empty(), &wad_name).into_bytes()));
    Ok(Source { files, textures, missing, remix_of })
}

fn readme(name: &str, has_wad: bool, wad_name: &str) -> String {
    let mut t = String::new();
    t.push_str(&format!("The editable source of {name}.\r\n\r\n"));
    t.push_str(&format!("{name}.lsmap  the Loner-Strike map editor's project: in Loner-Strike, Community > the map's page > Remix this map\r\n"));
    t.push_str(&format!("{name}.map    the same map as a Valve 220 .map, for Hammer, J.A.C.K. and TrenchBroom\r\n"));
    if has_wad {
        t.push_str(&format!("{wad_name}  the map's own textures (the game's own ones come with the game)\r\n"));
    }
    t.push_str("\r\nRemix it, and credit the original when you share yours (Loner-Strike's Publish does that for you).\r\n");
    t
}

/// The source folder of an unpacked community package, checked: one project (plain JSON, within
/// the limits), at most one texture WAD (only textures), the .map and a text file, nothing else.
pub fn read_package(src: &Path) -> Result<(MapDocument, Option<(String, Vec<u8>)>), String> {
    let refuse = |why: &str| Err(format!("This map's editable source can't be used: {why}"));
    let dir = std::fs::read_dir(src).ok().and_then(|rd| rd.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("source")).map(|e| e.path()));
    let Some(dir) = dir else { return refuse("the package has no source folder.") };
    let (mut project, mut texture) = (None, None);
    for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        let meta = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        if !meta.is_file() {
            return refuse(&format!("{name} isn't a plain file."));
        }
        let low = name.to_ascii_lowercase();
        if low.ends_with(".lsmap") {
            if project.is_some() {
                return refuse("there's more than one project in it.");
            }
            if meta.len() > MAX_PROJECT {
                return refuse("its project is over 64 MB.");
            }
            project = Some(p);
        } else if low.ends_with(".wad") {
            if texture.is_some() {
                return refuse("there's more than one texture file in it.");
            }
            if meta.len() > MAX_SOURCE_WAD {
                return refuse("its texture file is over 64 MB.");
            }
            texture = Some((name, p));
        } else if !(low.ends_with(".map") || low.ends_with(".txt")) {
            return refuse(&format!("{name} isn't a map source file."));
        }
    }
    let Some(project) = project else { return refuse("it has no project (.lsmap).") };
    let text = std::fs::read_to_string(&project).map_err(|_| "This map's editable source can't be used: its project isn't text.".to_string())?;
    let mut doc = MapDocument::from_json(&text).map_err(|e| format!("This map's editable source can't be used: {e}"))?;
    // (nothing from the package may point outside: no FGD path, no WAD folder, no UNC share)
    sanitize(&mut doc);
    if doc.brushes.len() > MAX_BRUSHES || doc.entities.len() > MAX_ENTITIES {
        return refuse("its project is bigger than any the editor makes.");
    }
    let wad = match texture {
        None => None,
        Some((name, p)) => {
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            let lumps = wadwrite::read_lumps(&bytes).map_err(|_| "This map's editable source can't be used: its texture file is damaged.".to_string())?;
            if lumps.iter().any(|l| l.kind != wadwrite::TYPE_MIPTEX) {
                return refuse("its texture file holds something other than textures.");
            }
            if !safe_file_name(&name) {
                return refuse("its texture file has an odd name.");
            }
            Some((name, bytes))
        }
    };
    Ok((doc, wad))
}

fn safe_file_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c)) && !n.starts_with('.')
}

/// Saves a remix: the project as `<dir>/<name>_remix.lsmap` (or `_remix2`... never over a file
/// that's there) with the credit and a fresh id, its WAD beside it. The project's path.
pub fn save_remix(dir: &Path, name: &str, mut doc: MapDocument, wad: Option<(String, Vec<u8>)>, credit: RemixCredit) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make the remix folder ({e})."))?;
    let base: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(24).collect();
    let base = if base.is_empty() { "map".to_string() } else { base };
    let mut stem = format!("{base}_remix");
    let mut n = 2;
    while dir.join(format!("{stem}.lsmap")).exists() {
        stem = format!("{base}_remix{n}");
        n += 1;
        if n > 999 {
            return Err("There are too many remixes of this map already.".into());
        }
    }
    if let Some((wad_name, bytes)) = wad {
        // (its own folder's WAD: a second remix keeps the first one's if they're the same)
        let target = dir.join(&wad_name);
        if !target.exists() || std::fs::read(&target).ok().as_deref() != Some(&bytes[..]) {
            let target = if target.exists() { dir.join(format!("{stem}_{wad_name}")) } else { target };
            if target != dir.join(&wad_name) {
                // the project names the WAD by file name: point it at the copy kept for this remix
                let new = target.file_name().unwrap().to_string_lossy().to_string();
                doc.world.wads = doc.world.wads.iter().map(|w| if w.eq_ignore_ascii_case(&wad_name) { new.clone() } else { w.clone() }).collect();
            }
            std::fs::write(&target, &bytes).map_err(|e| format!("Couldn't save the remix's textures ({e})."))?;
        }
    }
    doc.editor.uid = Some(crate::editor::doc::new_uid());
    doc.editor.remix_of = Some(credit);
    let path = dir.join(format!("{stem}.lsmap"));
    std::fs::write(&path, doc.to_json()).map_err(|e| format!("Couldn't save the remix ({e})."))?;
    Ok(path)
}

/// Tests here and in community.rs: a Half-Life folder with CS 1.6's cstrike.wad (holding "stockwall") and the mapper's own
/// mine.wad (holding "brick"); a project in `proj/` using both, plus "lost" found nowhere.
#[cfg(test)]
pub(crate) fn fixture_project(root: &Path) -> (PathBuf, PathBuf) {
    use crate::goldsrc::bsp::fixture;
    let hl = root.join("Half-Life");
    std::fs::create_dir_all(hl.join("cstrike")).unwrap();
    std::fs::create_dir_all(hl.join("valve")).unwrap();
    std::fs::write(hl.join("cstrike/cstrike.wad"), wad::build_wad(&[("stockwall", fixture::miptex("stockwall", false))])).unwrap();
    std::fs::write(hl.join("cstrike/mine.wad"), wad::build_wad(&[("brick", fixture::miptex("brick", false)), ("unused", fixture::miptex("unused", false))])).unwrap();
    let text = "{\n\"classname\" \"worldspawn\"\n\"wad\" \"\\\\half-life\\\\cstrike\\\\cstrike.wad;C:\\\\maps\\\\mine.wad\"\n\"mapversion\" \"220\"\n{\n\
( -64 -64 64 ) ( -64 64 64 ) ( 64 64 64 ) BRICK [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1\n\
( -64 64 -64 ) ( -64 -64 -64 ) ( 64 -64 -64 ) STOCKWALL [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1\n\
( -64 -64 -64 ) ( -64 64 -64 ) ( -64 64 64 ) lost [ 0 1 0 0 ] [ 0 0 -1 0 ] 0 1 1\n\
( 64 64 -64 ) ( 64 -64 -64 ) ( 64 -64 64 ) BRICK [ 0 1 0 0 ] [ 0 0 -1 0 ] 0 1 1\n\
( 64 64 -64 ) ( 64 64 64 ) ( -64 64 64 ) BRICK [ 1 0 0 0 ] [ 0 0 -1 0 ] 0 1 1\n\
( 64 -64 64 ) ( 64 -64 -64 ) ( -64 -64 -64 ) BRICK [ 1 0 0 0 ] [ 0 0 -1 0 ] 0 1 1\n}\n}\n\
{\n\"classname\" \"info_player_start\"\n\"origin\" \"0 0 100\"\n}\n";
    let doc = mapfile::read_map(text).unwrap();
    let proj = root.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let p = proj.join("aim_qa.lsmap");
    std::fs::write(&p, doc.to_json()).unwrap();
    (hl, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-mapsource-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn source_has_the_project_a_map_and_only_the_maps_own_textures() {
        let root = tmp("build");
        let (hl, project) = super::fixture_project(&root);
        let s = build(&project, &hl, false, "aim_qa").unwrap();
        let names: Vec<&str> = s.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["source/aim_qa.lsmap", "source/aim_qa.map", "source/aim_qa_custom.wad", "source/README-source.txt"]);
        assert_eq!(s.textures, ["brick"], "only the map's own, used texture: not CS 1.6's, not an unused one");
        assert_eq!(s.missing, ["lost"]);
        let wad_bytes = &s.files[2].1;
        let lumps = wadwrite::read_lumps(wad_bytes).unwrap();
        assert_eq!(lumps.iter().map(|l| l.name.to_ascii_lowercase()).collect::<Vec<_>>(), ["brick"]);
        // the project and the .map name the game's WAD and the new one, nothing from the author's disk
        let doc = MapDocument::from_json(std::str::from_utf8(&s.files[0].1).unwrap()).unwrap();
        assert_eq!(doc.world.wads, ["cstrike.wad", "aim_qa_custom.wad"]);
        assert!(doc.editor.uid.is_none());
        let map = String::from_utf8_lossy(&s.files[1].1).to_string();
        assert!(map.contains("\"wad\" \"cstrike.wad;aim_qa_custom.wad;\"") && map.contains("\"mapversion\" \"220\""), "{map}");
        assert!(mapfile::read_map(&map).is_ok(), "the .map reads back (Hammer, J.A.C.K., TrenchBroom)");
        // not a project: refused
        assert!(build(&root.join("x.map"), &hl, false, "x").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// QA4-T2: Condition Zero's own texture files are the game's on a CZ map (left out, named as
    /// they are) but a mapper's on a CS 1.6 one (packed into the map's own WAD), whether or not
    /// CZ is installed on the author's computer
    #[test]
    fn czero_texture_files_are_stock_on_czero_maps_only() {
        use crate::goldsrc::bsp::fixture;
        for cz_installed in [true, false] {
            let root = tmp(if cz_installed { "czwad" } else { "czwad-nocz" });
            let (hl, project) = super::fixture_project(&root);
            // the CZ texture file next to the project (as an author without CZ would have it) or in czero/
            let dir = if cz_installed { hl.join("czero") } else { project.parent().unwrap().to_path_buf() };
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("czde_aztec.wad"), wad::build_wad(&[("brick", fixture::miptex("brick", false))])).unwrap();
            let (mut doc, _) = super::super::editor::load_file(&project).unwrap();
            doc.world.wads = vec!["cstrike.wad".into(), "czde_aztec.wad".into()];
            std::fs::write(&project, doc.to_json()).unwrap();
            if cz_installed {
                let s = build(&project, &hl, true, "de_qa").unwrap();
                assert!(s.textures.is_empty(), "CZ's own texture on a CZ map: {:?}", s.textures);
                let doc = MapDocument::from_json(std::str::from_utf8(&s.files[0].1).unwrap()).unwrap();
                assert_eq!(doc.world.wads, ["cstrike.wad", "czde_aztec.wad"]);
                assert!(!s.files.iter().any(|(n, _)| n.ends_with("_custom.wad")));
            }
            let s = build(&project, &hl, false, "de_qa").unwrap();
            assert_eq!(s.textures, ["brick"], "a CZ texture on a CS 1.6 map goes in the map's own WAD");
            let doc = MapDocument::from_json(std::str::from_utf8(&s.files[0].1).unwrap()).unwrap();
            assert_eq!(doc.world.wads, ["cstrike.wad", "de_qa_custom.wad"]);
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn a_remix_comes_back_with_its_credit_and_never_overwrites() {
        let root = tmp("remix");
        let (hl, project) = super::fixture_project(&root);
        let s = build(&project, &hl, false, "aim_qa").unwrap();
        let pkg = root.join("pkg");
        for (n, b) in &s.files {
            let p = pkg.join(n);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b).unwrap();
        }
        let (doc, wad) = read_package(&pkg).unwrap();
        assert_eq!(wad.as_ref().unwrap().0, "aim_qa_custom.wad");
        let credit = RemixCredit { model: "Mod".into(), gb: 777, uid: String::new(), name: "Aim QA".into(), author: "jam".into() };
        let dir = root.join("remixes");
        let a = save_remix(&dir, "aim_qa", doc.clone(), wad.clone(), credit.clone()).unwrap();
        let b = save_remix(&dir, "aim_qa", doc.clone(), wad, credit.clone()).unwrap();
        assert_eq!(a.file_name().unwrap(), "aim_qa_remix.lsmap");
        assert_eq!(b.file_name().unwrap(), "aim_qa_remix2.lsmap", "never over the first");
        let (back, _) = super::super::editor::load_file(&a).unwrap();
        assert_eq!(back.editor.remix_of.as_ref(), Some(&credit));
        assert!(back.editor.uid.as_deref().is_some_and(crate::editor::doc::valid_uid));
        assert_eq!(back.brushes.len(), doc.brushes.len(), "the same map");
        assert!(dir.join("aim_qa_custom.wad").is_file());
        // the remix's own source carries the credit on
        let again = build(&a, &hl, false, "aim_qa_remix").unwrap();
        assert_eq!(again.remix_of.as_ref().map(|c| c.gb), Some(777));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// What a path on someone's computer looks like in the shared JSON or .map text.
    fn leaks(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for pat in ["/Users/", "Users\\\\", "C:\\\\", "c:\\\\", "\\\\\\\\", "\"/", "\"~", "\"\\\\", "\\\\host", "Users\\"] {
            if text.contains(pat) {
                out.push(pat.to_string());
            }
        }
        // a drive letter at the start of any string value ("X:\\..." or "X:/...")
        let b = text.as_bytes();
        for i in 0..b.len().saturating_sub(4) {
            if b[i] == b'"' && b[i + 1].is_ascii_alphabetic() && b[i + 2] == b':' && (b[i + 3] == b'\\' || b[i + 3] == b'/') {
                out.push(String::from_utf8_lossy(&b[i..(i + 12).min(b.len())]).to_string());
            }
        }
        out
    }

    /// A project with paths from the author's computer everywhere they can be.
    fn leaky(doc: &mut MapDocument) {
        doc.editor.fgd = Some("/Users/jam/fgds/mine.fgd".into());
        doc.world.wads = vec!["\\\\evilhost\\share\\cstrike.wad".into(), "C:\\Users\\jam\\maps\\mine.wad".into(), "../../secret.wad".into(), "~/x/y.wad".into()];
        for m in doc.materials.values_mut() {
            m.wad = Some("\\\\evilhost\\share\\mine.wad".into());
        }
        doc.world.keys.push(("skyname".into(), "C:\\Users\\jam\\sky\\des".into()));
        if let Some(e) = doc.entities.values_mut().next() {
            e.keys.push(("model".into(), "C:\\Users\\jam\\Half-Life\\cstrike\\models\\x.mdl".into()));
            e.keys.push(("noise".into(), "\\\\evilhost\\share\\a.wav".into()));
        }
    }

    #[test]
    fn a_published_project_carries_no_path_from_the_authors_computer() {
        let root = tmp("leak-out");
        let (hl, project) = super::fixture_project(&root);
        let (mut doc, _) = super::super::editor::load_file(&project).unwrap();
        leaky(&mut doc);
        std::fs::write(&project, doc.to_json()).unwrap();
        let s = build(&project, &hl, false, "aim_qa").unwrap();
        for (name, body) in s.files.iter().filter(|(n, _)| n.ends_with(".lsmap") || n.ends_with(".map")) {
            let text = String::from_utf8_lossy(body);
            assert!(leaks(&text).is_empty(), "{name}: {:?}", leaks(&text));
        }
        let shared = MapDocument::from_json(std::str::from_utf8(&s.files[0].1).unwrap()).unwrap();
        assert!(shared.editor.fgd.is_none() && shared.editor.uid.is_none());
        assert!(shared.world.wads.iter().all(|w| !w.contains(['/', '\\'])), "{:?}", shared.world.wads);
        let model = shared.entities.values().flat_map(|e| e.keys.iter()).find(|(k, _)| k == "model").map(|(_, v)| v.clone());
        assert_eq!(model.as_deref(), Some("models/x.mdl"), "the part from the game folder on");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_remix_takes_no_path_from_the_package() {
        let root = tmp("leak-in");
        let src = root.join("pkg/source");
        std::fs::create_dir_all(&src).unwrap();
        let mut doc = mapfile::read_map("{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"info_player_start\"\n\"origin\" \"0 0 0\"\n}\n").unwrap();
        doc.materials.insert("brick".into(), crate::editor::doc::Material { name: "brick".into(), width: 64, height: 64, wad: None });
        leaky(&mut doc);
        doc.editor.fgd = Some("\\\\evilhost\\share\\x.fgd".into());
        std::fs::write(src.join("m.lsmap"), doc.to_json()).unwrap();
        assert!(leaks(&doc.to_json()).len() >= 3, "the check sees them: {:?}", leaks(&doc.to_json()));
        let (back, _) = read_package(&root.join("pkg")).unwrap();
        assert!(back.editor.fgd.is_none());
        assert_eq!(back.world.wads, ["cstrike.wad", "mine.wad", "secret.wad", "y.wad"]);
        assert!(back.materials.values().all(|m| m.wad.as_deref() == Some("mine.wad")));
        assert!(leaks(&back.to_json()).is_empty(), "{:?}", leaks(&back.to_json()));
        let path = save_remix(&root.join("remixes"), "m", back, None, RemixCredit::default()).unwrap();
        let saved = std::fs::read_to_string(path).unwrap();
        assert!(leaks(&saved).is_empty(), "{:?}", leaks(&saved));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_source_is_refused_with_a_reason() {
        let root = tmp("bad");
        let src = root.join("x/source");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("m.lsmap"), "{not json").unwrap();
        assert!(read_package(&root.join("x")).unwrap_err().contains("can't be used"));
        std::fs::write(src.join("m.lsmap"), r#"{"version": 99}"#).unwrap();
        assert!(read_package(&root.join("x")).unwrap_err().contains("newer"));
        std::fs::write(src.join("m.lsmap"), crate::editor::doc::MapDocument::new().to_json()).unwrap();
        std::fs::write(src.join("run.exe"), b"MZ").unwrap();
        assert!(read_package(&root.join("x")).unwrap_err().contains("run.exe"));
        std::fs::remove_file(src.join("run.exe")).unwrap();
        std::fs::write(src.join("t.wad"), b"WAD3junk").unwrap();
        assert!(read_package(&root.join("x")).unwrap_err().contains("damaged"));
        std::fs::remove_file(src.join("t.wad")).unwrap();
        assert!(read_package(&root.join("x")).is_ok());
        assert!(read_package(&root.join("none")).unwrap_err().contains("no source folder"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
