//! GoldSrc .map files (what Hammer/J.A.C.K./TrenchBroom export and the HLT compilers read).
//! We always write Valve 220 (`"mapversion" "220"`, texture axes in brackets) and read both
//! Valve 220 and the older Quake "standard" face format. TrenchBroom groups/layers
//! (`func_group` with `_tb_type`) come in as editor groups.

use super::doc::{Brush, Entity, EntityId, Face, Group, GroupId, MapDocument, TexAlign};
use super::math::{Plane, Vec3};
use super::texture::from_standard;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Biggest .map we'll read (a huge GoldSrc map is a few MB).
pub const MAX_MAP_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Str(String),
    Word(String),
    Sym(char),
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
    peeked: Option<(Tok, usize)>,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Lexer { s: s.as_bytes(), i: 0, line: 1, peeked: None }
    }

    fn skip_space(&mut self) {
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'\n' {
                self.line += 1;
                self.i += 1;
            } else if c.is_ascii_whitespace() {
                self.i += 1;
            } else if c == b'/' && self.s.get(self.i + 1) == Some(&b'/') {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn lex(&mut self) -> Result<Option<(Tok, usize)>, String> {
        self.skip_space();
        if self.i >= self.s.len() {
            return Ok(None);
        }
        let line = self.line;
        let c = self.s[self.i];
        let standalone = |s: &[u8], i: usize| s.get(i + 1).map_or(true, |n| n.is_ascii_whitespace());
        let tok = match c {
            b'(' | b')' | b'[' | b']' => {
                self.i += 1;
                Tok::Sym(c as char)
            }
            // A brace only counts on its own: texture names like {FENCE start with one.
            b'{' | b'}' if standalone(self.s, self.i) => {
                self.i += 1;
                Tok::Sym(c as char)
            }
            b'"' => {
                let start = self.i + 1;
                let mut j = start;
                while j < self.s.len() && self.s[j] != b'"' {
                    if self.s[j] == b'\n' {
                        self.line += 1;
                    }
                    j += 1;
                }
                if j >= self.s.len() {
                    return Err(format!("Line {line}: a quote is never closed."));
                }
                self.i = j + 1;
                Tok::Str(String::from_utf8_lossy(&self.s[start..j]).into_owned())
            }
            _ => {
                let start = self.i;
                while self.i < self.s.len() {
                    let c = self.s[self.i];
                    if c.is_ascii_whitespace() || matches!(c, b'(' | b')' | b'[' | b']' | b'"') {
                        break;
                    }
                    self.i += 1;
                }
                Tok::Word(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
            }
        };
        Ok(Some((tok, line)))
    }

    fn next(&mut self) -> Result<Option<(Tok, usize)>, String> {
        match self.peeked.take() {
            Some(t) => Ok(Some(t)),
            None => self.lex(),
        }
    }

    fn peek(&mut self) -> Result<Option<&(Tok, usize)>, String> {
        if self.peeked.is_none() {
            self.peeked = self.lex()?;
        }
        Ok(self.peeked.as_ref())
    }

    fn expect_sym(&mut self, c: char) -> Result<(), String> {
        match self.next()? {
            Some((Tok::Sym(x), _)) if x == c => Ok(()),
            Some((t, line)) => Err(format!("Line {line}: expected '{c}' but found {}.", describe(&t))),
            None => Err(format!("The file ends early (expected '{c}').")),
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        match self.next()? {
            Some((Tok::Word(w), line)) => w
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("Line {line}: '{w}' isn't a number.")),
            Some((t, line)) => Err(format!("Line {line}: expected a number but found {}.", describe(&t))),
            None => Err("The file ends in the middle of a brush.".into()),
        }
    }

    fn point(&mut self) -> Result<Vec3, String> {
        self.expect_sym('(')?;
        let p = Vec3 { x: self.number()?, y: self.number()?, z: self.number()? };
        self.expect_sym(')')?;
        Ok(p)
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Str(s) => format!("\"{s}\""),
        Tok::Word(w) => format!("'{w}'"),
        Tok::Sym(c) => format!("'{c}'"),
    }
}

struct RawEntity {
    keys: Vec<(String, String)>,
    brushes: Vec<Brush>,
}

fn parse_face(lx: &mut Lexer) -> Result<Face, String> {
    let points = [lx.point()?, lx.point()?, lx.point()?];
    let (texture, tex_line) = match lx.next()? {
        Some((Tok::Word(w), l)) | Some((Tok::Str(w), l)) => (w, l),
        Some((t, l)) => return Err(format!("Line {l}: expected a texture name but found {}.", describe(&t))),
        None => return Err("The file ends in the middle of a brush.".into()),
    };
    let normal = Plane::from_map_points(&points).map(|p| p.normal).unwrap_or(Vec3::Z);
    let align = if matches!(lx.peek()?, Some((Tok::Sym('['), _))) {
        let mut axis = || -> Result<(Vec3, f64), String> {
            lx.expect_sym('[')?;
            let v = Vec3 { x: lx.number()?, y: lx.number()?, z: lx.number()? };
            let off = lx.number()?;
            lx.expect_sym(']')?;
            Ok((v, off))
        };
        let (u, uo) = axis()?;
        let (v, vo) = axis()?;
        let rotation = lx.number()?;
        let (su, sv) = (lx.number()?, lx.number()?);
        TexAlign { u_axis: u, u_offset: uo, v_axis: v, v_offset: vo, rotation, scale_u: su, scale_v: sv }
    } else {
        let (xo, yo, rot, sx, sy) = (lx.number()?, lx.number()?, lx.number()?, lx.number()?, lx.number()?);
        from_standard(normal, xo, yo, rot, sx, sy)
    };
    // Quake 2 / TrenchBroom extras (content flags, surface flags, value) on the same line.
    while let Some((Tok::Word(w), l)) = lx.peek()? {
        if *l != tex_line || w.parse::<f64>().is_err() {
            break;
        }
        lx.next()?;
    }
    Ok(Face { points, texture, align })
}

fn parse_entities(text: &str) -> Result<Vec<RawEntity>, String> {
    let mut lx = Lexer::new(text);
    let mut out = Vec::new();
    while let Some((tok, line)) = lx.next()? {
        if tok != Tok::Sym('{') {
            return Err(format!("Line {line}: expected '{{' to start an entity but found {}.", describe(&tok)));
        }
        let mut ent = RawEntity { keys: Vec::new(), brushes: Vec::new() };
        loop {
            match lx.next()? {
                Some((Tok::Str(k), _)) => match lx.next()? {
                    Some((Tok::Str(v), _)) => ent.keys.push((k, v)),
                    Some((t, l)) => return Err(format!("Line {l}: key \"{k}\" has no value (found {}).", describe(&t))),
                    None => return Err("The file ends in the middle of an entity.".into()),
                },
                Some((Tok::Sym('{'), _)) => {
                    let mut faces = Vec::new();
                    loop {
                        match lx.peek()? {
                            Some((Tok::Sym('}'), _)) => {
                                lx.next()?;
                                break;
                            }
                            Some((Tok::Sym('('), _)) => faces.push(parse_face(&mut lx)?),
                            Some((Tok::Word(w), l)) if w.starts_with("patchDef") || w.starts_with("brushDef") => {
                                return Err(format!("Line {l}: '{w}' is a Quake 3 / Doom 3 feature, not GoldSrc."));
                            }
                            Some((t, l)) => return Err(format!("Line {l}: unexpected {} in a brush.", describe(t))),
                            None => return Err("The file ends in the middle of a brush.".into()),
                        }
                    }
                    ent.brushes.push(Brush { faces, entity: None, group: None });
                }
                Some((Tok::Sym('}'), _)) => break,
                Some((t, l)) => return Err(format!("Line {l}: unexpected {} in an entity.", describe(&t))),
                None => return Err("The file ends in the middle of an entity.".into()),
            }
        }
        out.push(ent);
    }
    Ok(out)
}

fn key<'a>(keys: &'a [(String, String)], k: &str) -> Option<&'a str> {
    keys.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

/// Read a .map file into a new document.
pub fn read_map(text: &str) -> Result<MapDocument, String> {
    if text.len() > MAX_MAP_BYTES {
        return Err("This .map file is too big to open.".into());
    }
    let raw = parse_entities(text)?;
    if raw.is_empty() {
        return Err("This .map file has no entities.".into());
    }
    let mut doc = MapDocument::new();
    let world_i = raw.iter().position(|e| key(&e.keys, "classname") == Some("worldspawn"));

    // TrenchBroom groups and layers first, so everything else can point at them.
    let mut tb_groups: HashMap<String, GroupId> = HashMap::new();
    let is_tb_group = |e: &RawEntity| {
        key(&e.keys, "classname") == Some("func_group") && matches!(key(&e.keys, "_tb_type"), Some("_tb_group" | "_tb_layer"))
    };
    for e in raw.iter().filter(|e| is_tb_group(e)) {
        let name = key(&e.keys, "_tb_name").unwrap_or("Group").to_string();
        // a layer's flags come along (QA EditorCoverage TB-1, TB-2): hidden and locked as the
        // editor's own, "omit from export" kept for export and compile
        let on = |k: &str| key(&e.keys, k) == Some("1");
        let layer = key(&e.keys, "_tb_type") == Some("_tb_layer");
        let gid = doc.insert_group(Group {
            name,
            layer,
            hidden: layer && on("_tb_layer_hidden"),
            locked: layer && on("_tb_layer_locked"),
            omit_export: layer && on("_tb_layer_omit_from_export"),
            ..Default::default()
        });
        if let Some(tid) = key(&e.keys, "_tb_id") {
            tb_groups.insert(tid.to_string(), gid);
        }
    }
    let parent_of = |keys: &[(String, String)]| {
        key(keys, "_tb_group").or_else(|| key(keys, "_tb_layer")).and_then(|t| tb_groups.get(t).copied())
    };
    for e in raw.iter().filter(|e| is_tb_group(e)) {
        if let (Some(tid), Some(parent)) = (key(&e.keys, "_tb_id"), parent_of(&e.keys)) {
            let gid = tb_groups[tid];
            if gid != parent {
                if let Some(g) = doc.groups.get_mut(&gid) {
                    g.parent = Some(parent);
                }
            }
        }
    }

    for (i, e) in raw.into_iter().enumerate() {
        if Some(i) == world_i {
            for (k, v) in e.keys {
                match k.as_str() {
                    // (TrenchBroom's own world keys, like _tb_def, stay with TrenchBroom: TB-4)
                    "classname" | "mapversion" => {}
                    k if k.starts_with("_tb_") => {}
                    "wad" => doc.world.set_wad_key(&v),
                    _ => doc.world.keys.push((k, v)),
                }
            }
            for b in e.brushes {
                doc.insert_brush(b);
            }
        } else if is_tb_group(&e) {
            let gid = key(&e.keys, "_tb_id").and_then(|t| tb_groups.get(t).copied());
            for mut b in e.brushes {
                b.group = gid;
                doc.insert_brush(b);
            }
        } else {
            let group = parent_of(&e.keys);
            let keys = e.keys.into_iter().filter(|(k, _)| !k.starts_with("_tb_")).collect();
            let id: EntityId = doc.insert_entity(Entity { keys, group });
            for mut b in e.brushes {
                b.entity = Some(id);
                b.group = group;
                doc.insert_brush(b);
            }
        }
    }
    Ok(doc)
}

#[derive(Clone, Debug, Default)]
pub struct WriteOptions {
    /// Write groups as TrenchBroom `func_group`s so they survive a trip through TrenchBroom.
    /// Off by default: the HLT compilers would then see extra func_group entities.
    pub trenchbroom_groups: bool,
    /// Leave out what's in a layer marked "omit from export" (TrenchBroom's own rule): File ›
    /// Export, Share and remix and compiles. Off for copying a selection.
    pub leave_out_omitted: bool,
}

impl WriteOptions {
    /// File › Export, a shared map's source and prefabs: groups and layers kept for TrenchBroom,
    /// "omit from export" layers left out.
    pub fn export() -> WriteOptions {
        WriteOptions { trenchbroom_groups: true, leave_out_omitted: true }
    }
    /// What the compilers get: no editor groups, "omit from export" layers left out.
    pub fn compile() -> WriteOptions {
        WriteOptions { trenchbroom_groups: false, leave_out_omitted: true }
    }
}

/// Format a number the way map files like it: whole numbers without a dot, others with up to
/// six decimals.
pub fn fmt_num(x: f64) -> String {
    if (x - x.round()).abs() < 1e-9 {
        let r = x.round();
        return format!("{}", if r == 0.0 { 0.0 } else { r });
    }
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

fn check_text(what: &str, s: &str) -> Result<(), String> {
    if s.contains('"') || s.contains('\n') || s.contains('\r') {
        Err(format!("{what} \"{s}\" can't contain quotes or line breaks in a .map file."))
    } else {
        Ok(())
    }
}

fn write_keys(out: &mut String, keys: &[(String, String)]) -> Result<(), String> {
    for (k, v) in keys {
        check_text("The key", k)?;
        check_text("The value", v)?;
        let _ = writeln!(out, "\"{k}\" \"{v}\"");
    }
    Ok(())
}

fn write_brush(out: &mut String, n: usize, b: &Brush) -> Result<(), String> {
    let _ = writeln!(out, "// brush {n}\n{{");
    for f in &b.faces {
        let t = &f.texture;
        if t.is_empty() || t.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '(' | ')' | '[' | ']')) {
            return Err(format!("The texture name \"{t}\" can't be written to a .map file."));
        }
        for p in &f.points {
            let _ = write!(out, "( {} {} {} ) ", fmt_num(p.x), fmt_num(p.y), fmt_num(p.z));
        }
        let a = &f.align;
        let _ = writeln!(
            out,
            "{t} [ {} {} {} {} ] [ {} {} {} {} ] {} {} {}",
            fmt_num(a.u_axis.x),
            fmt_num(a.u_axis.y),
            fmt_num(a.u_axis.z),
            fmt_num(a.u_offset),
            fmt_num(a.v_axis.x),
            fmt_num(a.v_axis.y),
            fmt_num(a.v_axis.z),
            fmt_num(a.v_offset),
            fmt_num(a.rotation),
            fmt_num(a.scale_u),
            fmt_num(a.scale_v)
        );
    }
    out.push_str("}\n");
    Ok(())
}

/// Write the document as a Valve 220 .map.
pub fn write_map(doc: &MapDocument, opts: &WriteOptions) -> Result<String, String> {
    let mut out = String::new();
    // "omit from export": a group in (or under) a layer marked so is left out with what's in it
    let omitted = |g: Option<GroupId>| {
        let mut g = g;
        for _ in 0..64 {
            let Some(id) = g else { return false };
            let Some(x) = doc.groups.get(&id) else { return false };
            if x.omit_export {
                return opts.leave_out_omitted;
            }
            g = x.parent;
        }
        false
    };
    let tb = opts.trenchbroom_groups && doc.groups.iter().any(|(id, _)| !omitted(Some(*id)));
    // Brushes whose owner entity is gone are written as world brushes rather than lost.
    let owner = |b: &Brush| b.entity.filter(|e| doc.entities.contains_key(e));
    let brush_out = |b: &Brush| !omitted(b.group) && !owner(b).and_then(|e| doc.entities.get(&e)).is_some_and(|e| omitted(e.group));
    // the key a member uses to name its group or layer (TrenchBroom reads both)
    let member_key = |g: GroupId| if doc.groups.get(&g).is_some_and(|x| x.layer) { "_tb_layer" } else { "_tb_group" };

    out.push_str("// entity 0\n{\n");
    let mut wk = vec![("classname".to_string(), "worldspawn".to_string()), ("mapversion".to_string(), "220".to_string())];
    if !doc.world.wads.is_empty() {
        wk.push(("wad".into(), doc.world.wad_key()));
    }
    // (never TrenchBroom's own world keys: _tb_def and the like, TB-4)
    wk.extend(doc.world.keys.iter().filter(|(k, _)| !matches!(k.as_str(), "classname" | "mapversion" | "wad") && !k.starts_with("_tb_")).cloned());
    write_keys(&mut out, &wk)?;
    let mut n = 0;
    for b in doc.brushes.values().filter(|b| brush_out(b) && owner(b).is_none() && !(tb && b.group.is_some())) {
        write_brush(&mut out, n, b)?;
        n += 1;
    }
    out.push_str("}\n");

    let mut en = 1;
    if tb {
        for (gid, g) in doc.groups.iter().filter(|(id, _)| !omitted(Some(**id))) {
            let _ = writeln!(out, "// entity {en}\n{{");
            en += 1;
            let mut keys = vec![
                ("classname".to_string(), "func_group".to_string()),
                ("_tb_type".into(), if g.layer { "_tb_layer" } else { "_tb_group" }.into()),
                ("_tb_name".into(), g.name.clone()),
                ("_tb_id".into(), gid.0.to_string()),
            ];
            if g.layer {
                // (TrenchBroom's layers are top level; their flags go with them)
                for (on, k) in [(g.hidden, "_tb_layer_hidden"), (g.locked, "_tb_layer_locked"), (g.omit_export, "_tb_layer_omit_from_export")] {
                    if on {
                        keys.push((k.into(), "1".into()));
                    }
                }
            } else if let Some(p) = g.parent.filter(|p| doc.groups.contains_key(p)) {
                keys.push((member_key(p).into(), p.0.to_string()));
            }
            write_keys(&mut out, &keys)?;
            for (bn, b) in doc.brushes.values().filter(|b| owner(b).is_none() && b.group == Some(*gid)).enumerate() {
                write_brush(&mut out, bn, b)?;
            }
            out.push_str("}\n");
        }
    }
    for (id, e) in doc.entities.iter().filter(|(_, e)| !omitted(e.group)) {
        let _ = writeln!(out, "// entity {en}\n{{");
        en += 1;
        let mut keys = e.keys.clone();
        if tb {
            if let Some(g) = e.group.filter(|g| doc.groups.contains_key(g)) {
                keys.push((member_key(g).into(), g.0.to_string()));
            }
        }
        write_keys(&mut out, &keys)?;
        for (bn, b) in doc.brushes.values().filter(|b| owner(b) == Some(*id)).enumerate() {
            write_brush(&mut out, bn, b)?;
        }
        out.push_str("}\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity A (EditorCoverage TB-1..TB-4): a TrenchBroom map with layers and nested groups
    /// comes in with its layers' flags, goes out (File › Export) with its groups and layers and
    /// without the "omit from export" layer, comes back in the same, and the compilers get
    /// neither groups nor the omitted layer nor TrenchBroom's world keys.
    #[test]
    fn trenchbroom_layers_round_trip() {
        let src = include_str!("../../../ui/tests/fixtures/trenchbroom/tb_valve_layers.map");
        let d = read_map(src).unwrap();
        let by = |d: &MapDocument, n: &str| d.groups.iter().find(|(_, g)| g.name == n).map(|(id, g)| (*id, g.clone())).unwrap_or_else(|| panic!("group {n}"));
        let (details, g) = by(&d, "Details");
        assert!(g.layer && g.hidden && g.locked && !g.omit_export, "TB-1: {g:?}");
        let (notes, g) = by(&d, "Notes");
        assert!(g.layer && g.omit_export && !g.hidden, "TB-2: {g:?}");
        let (crates, g) = by(&d, "Crates");
        assert!(!g.layer && g.parent == Some(details));
        assert_eq!(by(&d, "Inner").1.parent, Some(crates));
        assert!(!d.world.keys.iter().any(|(k, _)| k.starts_with("_tb_")), "TB-4: {:?}", d.world.keys);
        let in_notes = d.brushes.values().filter(|b| b.group == Some(notes)).count();
        assert!(in_notes > 0);

        // File › Export: groups and layers with their flags, the Notes layer left out
        let out = write_map(&d, &WriteOptions::export()).unwrap();
        assert!(!out.contains("_tb_def") && !out.contains("\"Notes\""));
        let back = read_map(&out).unwrap();
        assert_eq!(back.groups.len(), d.groups.len() - 1, "every group but the omitted layer");
        assert_eq!(back.brushes.len(), d.brushes.len() - in_notes);
        assert_eq!(back.entities.len(), d.entities.len());
        let g = by(&back, "Details").1;
        assert!(g.layer && g.hidden && g.locked);
        let (bcrates, g) = by(&back, "Crates");
        assert_eq!(g.parent, Some(by(&back, "Details").0));
        assert_eq!(by(&back, "Inner").1.parent, Some(bcrates));
        // ...and a second trip keeps the same groups, flags and nesting (ids may be renumbered)
        let shape = |d: &MapDocument| {
            let mut v: Vec<_> = d.groups.values().map(|g| (g.name.clone(), g.layer, g.hidden, g.locked, g.parent.and_then(|p| d.groups.get(&p)).map(|p| p.name.clone()))).collect();
            v.sort();
            v
        };
        let again = read_map(&write_map(&back, &WriteOptions::export()).unwrap()).unwrap();
        assert_eq!(shape(&again), shape(&back));
        assert_eq!(again.brushes.len(), back.brushes.len());

        // the compilers: no func_group from the editor, no _tb_ keys, no omitted brushes
        let c = write_map(&d, &WriteOptions::compile()).unwrap();
        assert!(!c.contains("_tb_") && !c.contains("func_group"));
        assert_eq!(read_map(&c).unwrap().brushes.len(), d.brushes.len() - in_notes);
        // a copied selection keeps everything (the omitted layer's brushes too)
        assert_eq!(read_map(&write_map(&d, &WriteOptions::default()).unwrap()).unwrap().brushes.len(), d.brushes.len());
    }
    use crate::editor::geom::{brush_volume, is_valid};
    use crate::editor::math::v3;
    use crate::editor::ops::{make_box, make_cylinder, rotate};

    fn sample_doc() -> MapDocument {
        let mut d = MapDocument::new();
        d.world.set_wad_key("\\half-life\\valve\\halflife.wad;\\half-life\\cstrike\\cstrike.wad");
        d.world.keys.push(("skyname".into(), "desert".into()));
        d.insert_brush(make_box(v3(-256.0, -256.0, -16.0), v3(256.0, 256.0, 0.0), "{FENCE"));
        let cyl = make_cylinder(v3(0.0, 0.0, 0.0), 30.0, 64.0, 12, 0.0, "PIPE~1").unwrap();
        d.insert_brush(rotate(&cyl, v3(1.0, 1.0, 0.0), 17.0, Vec3::ZERO, true).unwrap());
        let mut door = Entity::new("func_door");
        door.set("angle", "-1");
        door.set("targetname", "lift door");
        let id = d.insert_entity(door);
        let mut b = make_box(v3(0.0, 0.0, 0.0), v3(8.0, 64.0, 96.0), "+0BUTTON");
        b.entity = Some(id);
        d.insert_brush(b);
        let mut spawn = Entity::new("info_player_start");
        spawn.set("origin", "0 0 36");
        d.insert_entity(spawn);
        d
    }

    #[test]
    fn round_trip_is_stable() {
        let d = sample_doc();
        let text = write_map(&d, &WriteOptions::default()).unwrap();
        assert!(text.contains("\"mapversion\" \"220\""));
        assert!(text.contains("{FENCE [ "));
        let back = read_map(&text).unwrap();
        assert_eq!(back.brushes.len(), 3);
        assert_eq!(back.entities.len(), 2);
        assert_eq!(back.world, d.world);
        let text2 = write_map(&back, &WriteOptions::default()).unwrap();
        assert_eq!(text, text2);
        for (a, b) in d.brushes.values().zip(back.brushes.values()) {
            assert_eq!(a.faces.len(), b.faces.len());
            assert!(is_valid(b));
            assert!((brush_volume(a) - brush_volume(b)).abs() < 1e-2);
            for (fa, fb) in a.faces.iter().zip(&b.faces) {
                assert_eq!(fa.texture, fb.texture);
                assert!(fa.align.u_axis.approx_eq(fb.align.u_axis, 1e-6));
            }
        }
        let door = back.entities.values().find(|e| e.classname() == "func_door").unwrap();
        assert_eq!(door.get("targetname"), Some("lift door"));
        let (did, _) = back.entities.iter().find(|(_, e)| e.classname() == "func_door").unwrap();
        assert_eq!(back.brushes_of(Some(*did)).count(), 1);
    }

    #[test]
    fn reads_hammer_style_and_standard_format() {
        let text = r#"
// Game: Counter-Strike
{
"classname" "worldspawn"
"wad" "\sierra\half-life\valve\halflife.wad"
"mapversion" "220"
// brush 0
{
( -64 -64 64 ) ( -64 64 64 ) ( 64 64 64 ) {BLUE [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1
( -64 64 -64 ) ( -64 -64 -64 ) ( 64 -64 -64 ) {BLUE [ 1 0 0 0 ] [ 0 -1 0 0 ] 0 1 1
( -64 -64 -64 ) ( -64 64 -64 ) ( -64 64 64 ) C1A0_W1 [ 0 1 0 0 ] [ 0 0 -1 0 ] 0 1 1
( 64 64 -64 ) ( 64 -64 -64 ) ( 64 -64 64 ) C1A0_W1 [ 0 1 0 0 ] [ 0 0 -1 0 ] 0 1 1
( 64 64 -64 ) ( 64 64 64 ) ( -64 64 64 ) C1A0_W1 [ 1 0 0 0 ] [ 0 0 -1 0 ] 0 1 1
( 64 -64 64 ) ( 64 -64 -64 ) ( -64 -64 -64 ) C1A0_W1 [ 1 0 0 0 ] [ 0 0 -1 0 ] 0 1 1
}
// brush 1 (standard format, with Quake 2 extras)
{
( 0 0 128 ) ( 0 32 128 ) ( 32 32 128 ) CRATE 8 -4 90 0.5 0.5 0 0 0
( 0 32 96 ) ( 0 0 96 ) ( 32 0 96 ) CRATE 0 0 0 1 1
( 0 0 96 ) ( 0 32 96 ) ( 0 32 128 ) CRATE 0 0 0 1 1
( 32 32 96 ) ( 32 0 96 ) ( 32 0 128 ) CRATE 0 0 0 1 1
( 32 32 96 ) ( 32 32 128 ) ( 0 32 128 ) CRATE 0 0 0 1 1
( 32 0 128 ) ( 32 0 96 ) ( 0 0 96 ) CRATE 0 0 0 1 1
}
}
{
"classname" "light"
"origin" "0 0 32"
"_light" "255 255 128 200"
}
"#;
        let d = read_map(text).unwrap();
        assert_eq!(d.world.wads, vec!["\\sierra\\half-life\\valve\\halflife.wad"]);
        assert_eq!(d.brushes.len(), 2);
        assert!(d.brushes.values().all(is_valid));
        let b0 = d.brushes.values().next().unwrap();
        assert_eq!(b0.faces[0].texture, "{BLUE");
        assert!((brush_volume(b0) - 128f64.powi(3)).abs() < 1e-6);
        let crate_top = &d.brushes.values().nth(1).unwrap().faces[0];
        assert_eq!(crate_top.align.rotation, 90.0);
        assert!(crate_top.align.u_axis.approx_eq(Vec3::Y, 1e-12));
        assert_eq!((crate_top.align.u_offset, crate_top.align.scale_u), (8.0, 0.5));
        assert_eq!(d.entities.values().next().unwrap().get("_light"), Some("255 255 128 200"));
    }

    #[test]
    fn trenchbroom_groups() {
        let mut d = sample_doc();
        let g = d.insert_group(Group { name: "Floor".into(), ..Default::default() });
        let sub = d.insert_group(Group { name: "Bits".into(), parent: Some(g), ..Default::default() });
        let first = *d.brushes.keys().next().unwrap();
        d.brushes.get_mut(&first).unwrap().group = Some(sub);
        let opts = WriteOptions { trenchbroom_groups: true, ..Default::default() };
        let text = write_map(&d, &opts).unwrap();
        assert!(text.contains("\"_tb_type\" \"_tb_group\""));
        let back = read_map(&text).unwrap();
        assert_eq!(back.groups.len(), 2);
        let bits = back.groups.values().find(|g| g.name == "Bits").unwrap();
        assert!(bits.parent.is_some());
        assert_eq!(back.brushes.values().filter(|b| b.group.is_some()).count(), 1);
        assert!(back.entities.values().all(|e| e.classname() != "func_group"));
        // Flattened by default: no func_group for the compilers.
        assert!(!write_map(&d, &WriteOptions::default()).unwrap().contains("func_group"));
    }

    #[test]
    fn bad_input_is_a_readable_error() {
        for (text, needle) in [
            ("{ \"classname\" \"worldspawn\"", "ends"),
            ("{ \"classname\" }", "no value"),
            ("{ \"classname\" \"worldspawn\" { ( 0 0 0 ) ( 1 0 0 ) ( x 0 0 ) T 0 0 0 1 1 } }", "isn't a number"),
            ("{ \"classname\" \"worldspawn\" { patchDef2 { } } }", "Quake 3"),
            ("\"key\" \"value\"", "expected '{'"),
            ("{ \"classname\" \"worldspawn }\n", "never closed"),
            ("", "no entities"),
            ("{ \"a\" \"b\" { ( 0 0 0 ) ( 1 0 0 ) ( 0 1 0 ) }", "texture name"),
        ] {
            let e = read_map(text).unwrap_err();
            assert!(e.contains(needle), "{text:?} -> {e}");
        }
        let mut d = MapDocument::new();
        d.insert_brush(make_box(Vec3::ZERO, v3(8.0, 8.0, 8.0), "has space"));
        assert!(write_map(&d, &WriteOptions::default()).is_err());
        let mut d = MapDocument::new();
        let mut e = Entity::new("info_target");
        e.set("message", "say \"hi\"");
        d.insert_entity(e);
        assert!(write_map(&d, &WriteOptions::default()).is_err());
    }

    #[test]
    fn truncated_files_never_panic() {
        let text = write_map(&sample_doc(), &WriteOptions::default()).unwrap();
        for (i, _) in text.char_indices().step_by(3) {
            let _ = read_map(&text[..i]);
        }
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt_num(64.0), "64");
        assert_eq!(fmt_num(-0.0), "0");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(1.0 / 3.0), "0.333333");
        assert_eq!(fmt_num(-1e-12), "0");
        assert_eq!(fmt_num(31.9999999999), "32");
    }
}

// ---- the game's code page ------------------------------------------------------------------

/// The Windows code page a .map's text is written in for the compiler (GoldSrc reads entity
/// text as bytes and shows them in the player's own code page).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodePage {
    /// Western (the default)
    Cp1252,
    /// Cyrillic
    Cp1251,
}

impl CodePage {
    pub fn name(self) -> &'static str {
        match self {
            CodePage::Cp1252 => "Western (Windows-1252)",
            CodePage::Cp1251 => "Cyrillic (Windows-1251)",
        }
    }

    /// The byte for a character, if this code page has it.
    pub fn byte(self, c: char) -> Option<u8> {
        let u = c as u32;
        if u < 0x80 {
            return Some(u as u8);
        }
        // the 0x80-0x9F block (and a few others) differ between the two
        const W1252: [(char, u8); 27] = [
            ('€', 0x80), ('‚', 0x82), ('ƒ', 0x83), ('„', 0x84), ('…', 0x85), ('†', 0x86), ('‡', 0x87), ('ˆ', 0x88), ('‰', 0x89),
            ('Š', 0x8A), ('‹', 0x8B), ('Œ', 0x8C), ('Ž', 0x8E), ('‘', 0x91), ('’', 0x92), ('“', 0x93), ('”', 0x94), ('•', 0x95),
            ('–', 0x96), ('—', 0x97), ('˜', 0x98), ('™', 0x99), ('š', 0x9A), ('›', 0x9B), ('œ', 0x9C), ('ž', 0x9E), ('Ÿ', 0x9F),
        ];
        const W1251: [(char, u8); 22] = [
            ('Ђ', 0x80), ('Ѓ', 0x81), ('‚', 0x82), ('ѓ', 0x83), ('„', 0x84), ('…', 0x85), ('‘', 0x91), ('’', 0x92), ('“', 0x93),
            ('”', 0x94), ('•', 0x95), ('–', 0x96), ('—', 0x97), ('™', 0x99), ('Ё', 0xA8), ('ё', 0xB8), ('№', 0xB9), ('«', 0xAB),
            ('»', 0xBB), ('Є', 0xAA), ('є', 0xBA), ('Ї', 0xAF),
        ];
        match self {
            CodePage::Cp1252 => {
                if (0xA0..=0xFF).contains(&u) {
                    Some(u as u8)
                } else {
                    W1252.iter().find(|(k, _)| *k == c).map(|(_, b)| *b)
                }
            }
            CodePage::Cp1251 => {
                if (0x0410..=0x044F).contains(&u) {
                    Some((u - 0x0410 + 0xC0) as u8)
                } else if c == '\u{A0}' || c == '§' || c == '©' || c == '®' || c == '°' || c == '±' || c == 'µ' || c == '¶' || c == '·' || c == '¬' || c == '¦' || c == '¤' {
                    Some(u as u8)
                } else {
                    W1251.iter().find(|(k, _)| *k == c).map(|(_, b)| *b)
                }
            }
        }
    }
}

/// A .map's text as the bytes the compiler gets: Western (cp1252) unless the text needs
/// Cyrillic (cp1251). Characters neither has become `?`, and are listed (each once) so the
/// player can be told.
pub fn encode_for_game(text: &str) -> (Vec<u8>, CodePage, Vec<char>) {
    let fits = |cp: CodePage| text.chars().all(|c| cp.byte(c).is_some());
    let cp = if fits(CodePage::Cp1252) {
        CodePage::Cp1252
    } else if fits(CodePage::Cp1251) {
        CodePage::Cp1251
    } else {
        // the one that keeps more of the text
        let lost = |cp: CodePage| text.chars().filter(|c| cp.byte(*c).is_none()).count();
        if lost(CodePage::Cp1251) < lost(CodePage::Cp1252) { CodePage::Cp1251 } else { CodePage::Cp1252 }
    };
    let mut bad: Vec<char> = Vec::new();
    let bytes = text
        .chars()
        .map(|c| {
            cp.byte(c).unwrap_or_else(|| {
                if !bad.contains(&c) {
                    bad.push(c);
                }
                b'?'
            })
        })
        .collect();
    (bytes, cp, bad)
}

#[cfg(test)]
mod code_page_tests {
    use super::*;

    #[test]
    fn entity_text_in_the_games_code_page() {
        // plain text stays as it is
        let (b, cp, bad) = encode_for_game("\"message\" \"Hello\"");
        assert_eq!((b.as_slice(), cp, bad.len()), (b"\"message\" \"Hello\"".as_slice(), CodePage::Cp1252, 0));
        // Western accents and quotes: cp1252
        let (b, cp, _) = encode_for_game("Café – “ok”");
        assert_eq!(cp, CodePage::Cp1252);
        assert_eq!(b, vec![b'C', b'a', b'f', 0xE9, b' ', 0x96, b' ', 0x93, b'o', b'k', 0x94]);
        // Russian: cp1251, and it reads back the way the importer reads it
        let (b, cp, bad) = encode_for_game("Фаза - #1 Ёё №");
        assert_eq!(cp, CodePage::Cp1251);
        assert!(bad.is_empty());
        assert_eq!(&b[..4], &[0xD4, 0xE0, 0xE7, 0xE0]);
        assert_eq!(crate::editor::rmf::decode_text(&b), "Фаза - #1 Ёё №");
        // what neither has (Chinese, emoji) becomes ? and is listed once
        let (b, _, bad) = encode_for_game("go 去去 🙂");
        assert_eq!(b, b"go ?? ?");
        assert_eq!(bad, vec!['去', '🙂']);
    }
}
