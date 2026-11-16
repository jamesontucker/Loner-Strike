//! A model entity's Collision (map editor, Content Browser models): the key `ls_collision` on a
//! model entity (cycler_sprite and the like) says what it blocks once compiled:
//! - `none`: nothing (walk through; what a cycler_sprite always did)
//! - `box`: players only: an invisible CLIP brush fitted to the model's box
//! - `box_bullets`: players and bullets: the same box as a func_wall with NULL faces
//! - `custom`: no box; you draw your own brushes
//!
//! The box is made only when the map is compiled, from the entity's origin, yaw and scale and
//! the model's bounding box (its first sequence's, else the header's), so it always follows the
//! model and never shows up as a brush you'd edit. A map without the key compiles as before.

use super::doc::{Entity, MapDocument};
use super::math::{v3, Vec3};
use super::ops::make_box;

pub const KEY: &str = "ls_collision";

/// A model's bounding box, from the .mdl: the first sequence's bbmin/bbmax, else the header's
/// clipping box, else its movement box. None for a file that isn't a studio model.
pub fn model_box(b: &[u8]) -> Option<([f32; 3], [f32; 3])> {
    let f = |at: usize| -> Option<f32> { b.get(at..at + 4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])) };
    let i = |at: usize| -> Option<i32> { b.get(at..at + 4).map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]])) };
    let v = |at: usize| -> Option<[f32; 3]> { Some([f(at)?, f(at + 4)?, f(at + 8)?]) };
    if b.get(0..4)? != b"IDST" || i(4)? != 10 {
        return None;
    }
    let ok = |lo: [f32; 3], hi: [f32; 3]| lo.iter().chain(hi.iter()).all(|x| x.is_finite() && x.abs() < 65536.0) && (0..3).all(|k| hi[k] > lo[k]);
    // (the first sequence: numseq at 164, seqindex at 168; bbmin at 96 in a 176-byte seqdesc)
    if let (Some(n), Some(at)) = (i(164), i(168)) {
        if n > 0 && at > 0 {
            let at = at as usize;
            if let (Some(lo), Some(hi)) = (v(at + 96), v(at + 108)) {
                if ok(lo, hi) {
                    return Some((lo, hi));
                }
            }
        }
    }
    for at in [112, 88] {
        if let (Some(lo), Some(hi)) = (v(at), v(at + 12)) {
            if ok(lo, hi) {
                return Some((lo, hi));
            }
        }
    }
    None
}

fn num(e: &Entity, k: &str) -> Option<f64> {
    e.get(k).and_then(|v| v.trim().parse::<f64>().ok()).filter(|x| x.is_finite())
}

/// The box in the map: the model's box scaled, turned by the entity's yaw (the box of the turned
/// box when the yaw isn't a multiple of 90) and moved to its origin.
pub fn world_box(e: &Entity, b: ([f32; 3], [f32; 3])) -> Option<(Vec3, Vec3)> {
    let o: Vec<f64> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if o.len() != 3 {
        return None;
    }
    let s = num(e, "scale").filter(|s| *s > 0.0).unwrap_or(1.0);
    let yaw = e.get("angles").and_then(|a| a.split_whitespace().nth(1)?.parse::<f64>().ok()).or_else(|| num(e, "angle")).unwrap_or(0.0).to_radians();
    let (c, sn) = (yaw.cos(), yaw.sin());
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for &x in &[b.0[0], b.1[0]] {
        for &y in &[b.0[1], b.1[1]] {
            for &z in &[b.0[2], b.1[2]] {
                let (x, y, z) = (x as f64 * s, y as f64 * s, z as f64 * s);
                let p = [x * c - y * sn, x * sn + y * c, z];
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    // (whole units, at least one each way: the compilers want planes on the grid)
    let lo = v3((o[0] + lo[0]).floor(), (o[1] + lo[1]).floor(), (o[2] + lo[2]).floor());
    let hi = v3((o[0] + hi[0]).ceil().max(lo.x + 1.0), (o[1] + hi[1]).ceil().max(lo.y + 1.0), (o[2] + hi[2]).ceil().max(lo.z + 1.0));
    Some((lo, hi))
}

/// The document as the compilers get it: a box brush for every model entity whose Collision is
/// a box (`read` gives a model file's bytes by its game path), and notes for models it couldn't
/// read. A document without the key comes back unchanged.
pub fn with_boxes(doc: &MapDocument, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> (MapDocument, Vec<String>) {
    let mut d = doc.clone();
    let mut notes = Vec::new();
    let want: Vec<(Entity, bool)> = doc
        .entities
        .values()
        .filter_map(|e| match e.get(KEY).map(str::trim) {
            Some("box") => Some((e.clone(), false)),
            Some("box_bullets") => Some((e.clone(), true)),
            _ => None,
        })
        .collect();
    for (e, bullets) in want {
        let Some(model) = e.get("model").map(str::trim).filter(|m| m.to_ascii_lowercase().ends_with(".mdl")) else { continue };
        let Some(b) = read(model).as_deref().and_then(model_box) else {
            notes.push(format!("{model}: its collision box wasn't made (the model couldn't be read), so it blocks nothing."));
            continue;
        };
        let Some((lo, hi)) = world_box(&e, b) else { continue };
        if bullets {
            let mut w = Entity::new("func_wall");
            w.set("rendermode", "0");
            let id = d.insert_entity(w);
            let mut br = make_box(lo, hi, "NULL");
            br.entity = Some(id);
            d.insert_brush(br);
        } else {
            d.insert_brush(make_box(lo, hi, "CLIP"));
        }
    }
    (d, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny studio model: the header with a box, and one sequence with its own box.
    pub fn mdl(seq: Option<([f32; 3], [f32; 3])>, header: ([f32; 3], [f32; 3])) -> Vec<u8> {
        let mut b = vec![0u8; 244 + 176];
        b[0..4].copy_from_slice(b"IDST");
        b[4..8].copy_from_slice(&10i32.to_le_bytes());
        let put = |b: &mut Vec<u8>, at: usize, v: [f32; 3]| {
            for k in 0..3 {
                b[at + k * 4..at + k * 4 + 4].copy_from_slice(&v[k].to_le_bytes());
            }
        };
        put(&mut b, 112, header.0);
        put(&mut b, 124, header.1);
        if let Some(s) = seq {
            b[164..168].copy_from_slice(&1i32.to_le_bytes());
            b[168..172].copy_from_slice(&244i32.to_le_bytes());
            put(&mut b, 244 + 96, s.0);
            put(&mut b, 244 + 108, s.1);
        }
        b
    }

    #[test]
    fn the_box_comes_from_the_model_and_follows_the_entity() {
        let m = mdl(Some(([-8.0, -4.0, 0.0], [8.0, 4.0, 20.0])), ([-1.0; 3], [1.0; 3]));
        assert_eq!(model_box(&m), Some(([-8.0, -4.0, 0.0], [8.0, 4.0, 20.0])), "the sequence's box first");
        assert_eq!(model_box(&mdl(None, ([-2.0; 3], [2.0; 3]))), Some(([-2.0; 3], [2.0; 3])), "else the header's");
        assert_eq!(model_box(b"not a model at all"), None);
        let mut e = Entity::new("cycler_sprite");
        e.set("origin", "100 0 0");
        e.set("angles", "0 90 0");
        e.set("scale", "2");
        let (lo, hi) = world_box(&e, model_box(&m).unwrap()).unwrap();
        // turned 90: x and y swap; scaled twice
        assert_eq!((lo, hi), (v3(92.0, -16.0, 0.0), v3(108.0, 16.0, 40.0)));
    }

    #[test]
    fn each_collision_makes_its_brush_and_old_maps_compile_the_same() {
        let m = mdl(Some(([-8.0, -8.0, 0.0], [8.0, 8.0, 16.0])), ([0.0; 3], [0.0; 3]));
        let read = |p: &str| (p == "models/crate.mdl").then(|| m.clone());
        let mut d = MapDocument::new();
        for (k, v) in [("a", Some("box")), ("b", Some("box_bullets")), ("c", Some("none")), ("d", Some("custom")), ("e", None)] {
            let mut e = Entity::new("cycler_sprite");
            e.set("origin", "0 0 0");
            e.set("model", "models/crate.mdl");
            e.set("targetname", k);
            if let Some(v) = v {
                e.set(KEY, v);
            }
            d.insert_entity(e);
        }
        let (out, notes) = with_boxes(&d, &read);
        assert!(notes.is_empty());
        let clips: Vec<_> = out.brushes.values().filter(|b| b.faces.iter().all(|f| f.texture == "CLIP")).collect();
        let nulls: Vec<_> = out.brushes.values().filter(|b| b.faces.iter().all(|f| f.texture == "NULL")).collect();
        assert_eq!((clips.len(), nulls.len()), (1, 1), "one players-only box, one players-and-bullets box");
        assert!(clips[0].entity.is_none(), "CLIP in the world");
        assert_eq!(out.entities[&nulls[0].entity.unwrap()].classname(), "func_wall");
        // without the key: the document as it was
        let mut old = MapDocument::new();
        let mut e = Entity::new("cycler_sprite");
        e.set("model", "models/crate.mdl");
        e.set("origin", "0 0 0");
        old.insert_entity(e);
        assert_eq!(with_boxes(&old, &read).0, old);
        // a model that can't be read: a note, no brush
        let mut d2 = old.clone();
        let id = *d2.entities.keys().next().unwrap();
        d2.entities.get_mut(&id).unwrap().set(KEY, "box");
        d2.entities.get_mut(&id).unwrap().set("model", "models/gone.mdl");
        let (o2, n2) = with_boxes(&d2, &read);
        assert!(o2.brushes.is_empty() && n2.len() == 1, "{n2:?}");
    }

    /// A test map with each Collision, through the compile's own pass (for a compiler run):
    /// `LS_COLL_MDL=<a .mdl> LS_COLL_MAP=<out .map> cargo test write_collision_test_map -- --ignored`
    #[test]
    #[ignore]
    fn write_collision_test_map() {
        let mdl = std::fs::read(std::env::var("LS_COLL_MDL").unwrap()).unwrap();
        let mut d = MapDocument::new();
        for (a, b) in [((-512., -512., -16.), (512., 512., 0.)), ((-512., -512., 256.), (512., 512., 272.)), ((-528., -512., 0.), (-512., 512., 256.)), ((512., -512., 0.), (528., 512., 256.)), ((-512., -528., 0.), (512., -512., 256.)), ((-512., 512., 0.), (512., 528., 256.))] {
            d.insert_brush(make_box(v3(a.0, a.1, a.2), v3(b.0, b.1, b.2), "OUT_WALL5"));
        }
        for (x, c, yaw) in [(0, "box", 0), (120, "box_bullets", 45), (240, "none", 0), (360, "custom", 0)] {
            let mut e = Entity::new("cycler_sprite");
            e.set("origin", &format!("{x} 0 0"));
            e.set("angles", &format!("0 {yaw} 0"));
            e.set("model", "models/test.mdl");
            e.set(KEY, c);
            d.insert_entity(e);
        }
        for (cls, o) in [("info_player_start", "0 -300 40"), ("info_player_deathmatch", "0 300 40"), ("light", "0 0 200")] {
            let mut e = Entity::new(cls);
            e.set("origin", o);
            d.insert_entity(e);
        }
        let (out, notes) = with_boxes(&d, &|_| Some(mdl.clone()));
        assert!(notes.is_empty(), "{notes:?}");
        std::fs::write(std::env::var("LS_COLL_MAP").unwrap(), crate::editor::mapfile::write_map(&out, &crate::editor::mapfile::WriteOptions::compile()).unwrap()).unwrap();
    }
}
