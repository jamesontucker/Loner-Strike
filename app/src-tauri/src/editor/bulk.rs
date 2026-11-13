//! Editing many things at once (agent P5): duplicate with new target names (links inside the
//! copied set follow the copies), find and replace across the map (textures, classnames, keys,
//! values), select similar / touching / inside, and line up or spread out objects.
//! Each `cmd_*` is one undo step; on error nothing changes.

use super::doc::{BrushId, Entity, EntityId, GroupId, MapDocument};
use super::geom::brush_bounds;
use super::history::{Created, History, Tx};
use super::mapfile::fmt_num;
use super::math::{Bounds, Vec3};
use super::ops::{self, OpResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Keys whose value names another entity (by its targetname).
pub const LINK_KEYS: &[&str] = &["target", "killtarget", "master", "netname", "LightningStart", "LightningEnd", "m_iszNewTarget", "changetarget", "parent", "landmark"];

fn axis(v: Vec3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn with_axis(i: usize, d: f64) -> Vec3 {
    Vec3 { x: if i == 0 { d } else { 0.0 }, y: if i == 1 { d } else { 0.0 }, z: if i == 2 { d } else { 0.0 } }
}

/// Every targetname in the map.
pub fn all_names(doc: &MapDocument) -> HashSet<String> {
    doc.entities.values().filter_map(|e| e.get("targetname")).filter(|n| !n.is_empty()).map(|n| n.to_string()).collect()
}

/// The next free name after `name`: a trailing number counts up (door1 -> door2), else _2, _3...
pub fn next_name(name: &str, taken: &HashSet<String>) -> String {
    let digits = name.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    let (stem, start, width) = if digits > 0 && digits < name.len() {
        let (s, d) = name.split_at(name.len() - digits);
        (s.to_string(), d.parse::<u64>().unwrap_or(1) + 1, d.len())
    } else if digits > 0 {
        // all digits: count the whole name up
        (String::new(), name.parse::<u64>().unwrap_or(0) + 1, name.len())
    } else {
        (format!("{name}_"), 2, 1)
    };
    let mut n = start;
    loop {
        let cand = format!("{stem}{n:0width$}");
        if !taken.contains(&cand) {
            return cand;
        }
        n += 1;
    }
}

/// multi_manager keys are target names, sometimes with "#n" to fire one twice.
fn mm_name(key: &str) -> &str {
    key.split('#').next().unwrap_or(key)
}

/// Gives copied entities new target names and points their links at the new names when the
/// thing they name was copied too (a copied button opens the copied door; a link to something
/// that wasn't copied stays as it was). `taken` grows with the names handed out.
pub fn rename_copies(copies: &mut [Entity], taken: &mut HashSet<String>) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for e in copies.iter() {
        if let Some(n) = e.get("targetname").filter(|n| !n.is_empty()) {
            if !map.contains_key(n) {
                let new = next_name(n, taken);
                taken.insert(new.clone());
                map.insert(n.to_string(), new);
            }
        }
    }
    if map.is_empty() {
        return map;
    }
    for e in copies.iter_mut() {
        let mm = e.classname() == "multi_manager";
        for (k, v) in e.keys.iter_mut() {
            if k == "targetname" || LINK_KEYS.contains(&k.as_str()) {
                if let Some(n) = map.get(v.as_str()) {
                    *v = n.clone();
                }
            } else if mm && k != "classname" && k != "origin" && k != "wait" && k != "spawnflags" {
                if let Some(n) = map.get(mm_name(k)) {
                    let rest = &k[mm_name(k).len()..];
                    *k = format!("{n}{rest}");
                }
            }
        }
    }
    map
}

fn move_origin(e: &mut Entity, d: Vec3) {
    if let Some(o) = e.origin() {
        let n = o + d;
        e.set("origin", &format!("{} {} {}", fmt_num(n.x), fmt_num(n.y), fmt_num(n.z)));
    }
}

/// Duplicate: copies moved by `offset` (zero for "in place"), brushes of a copied entity go
/// with its copy, groups kept off the copies. With `rename`, copies get new target names.
pub fn cmd_duplicate(doc: &mut MapDocument, h: &mut History, brushes: &[BrushId], entities: &[EntityId], offset: Vec3, rename: bool) -> OpResult<Created> {
    if brushes.is_empty() && entities.is_empty() {
        return Err("Pick something to duplicate first.".into());
    }
    if [offset.x, offset.y, offset.z].iter().any(|x| !x.is_finite() || x.abs() > 65536.0) {
        return Err("That's too far to move a copy.".into());
    }
    let mut copies: Vec<(EntityId, Entity)> = Vec::new();
    for id in entities {
        let e = doc.entities.get(id).cloned().ok_or("That entity no longer exists.")?;
        if e.classname() == "worldspawn" {
            continue;
        }
        copies.push((*id, e));
    }
    if rename {
        let mut taken = all_names(doc);
        let mut ents: Vec<Entity> = copies.iter().map(|(_, e)| e.clone()).collect();
        rename_copies(&mut ents, &mut taken);
        for (c, e) in copies.iter_mut().zip(ents) {
            c.1 = e;
        }
    }
    // brushes of picked brush entities come along even when only the entity was named
    let mut bs: BTreeSet<BrushId> = brushes.iter().copied().collect();
    for (id, _) in &copies {
        bs.extend(doc.brushes_of(Some(*id)).map(|(b, _)| b));
    }
    let src = doc.clone();
    let mut tx = Tx::new(doc, "Duplicate");
    let mut out = Created::default();
    let mut new_entity = HashMap::new();
    for (id, mut e) in copies {
        let has_brushes = src.brushes_of(Some(id)).next().is_some();
        if !has_brushes {
            move_origin(&mut e, offset);
        }
        e.group = None;
        let nid = tx.add_entity(e);
        new_entity.insert(id, nid);
        out.entities.push(nid);
    }
    for id in bs {
        let b = src.brushes.get(&id).ok_or("That brush no longer exists.")?;
        let mut nb = ops::translate(b, offset, true)?;
        nb.entity = b.entity.map(|e| new_entity.get(&e).copied().unwrap_or(e));
        nb.group = None;
        out.brushes.push(tx.add_brush(nb));
    }
    tx.commit(h);
    Ok(out)
}

// ---- find and replace ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    /// texture names on faces
    Texture,
    /// entity classnames
    Classname,
    /// key names (rename a key)
    Key,
    /// key values (all keys, or just `key`)
    Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FindSpec {
    pub field: Field,
    pub find: String,
    #[serde(default)]
    pub replace: String,
    /// for Value: only this key
    #[serde(default)]
    pub key: Option<String>,
    /// the whole name/value must match (else: contains)
    #[serde(default = "yes")]
    pub whole: bool,
    #[serde(default)]
    pub case: bool,
    /// only in these (else the whole map)
    #[serde(default)]
    pub brushes: Option<Vec<u64>>,
    #[serde(default)]
    pub entities: Option<Vec<u64>>,
}

fn yes() -> bool {
    true
}

impl FindSpec {
    fn matches(&self, s: &str) -> bool {
        if self.find.is_empty() {
            return false;
        }
        match (self.whole, self.case) {
            (true, true) => s == self.find,
            (true, false) => s.eq_ignore_ascii_case(&self.find),
            (false, true) => s.contains(&self.find),
            (false, false) => s.to_ascii_lowercase().contains(&self.find.to_ascii_lowercase()),
        }
    }
    fn replaced(&self, s: &str) -> String {
        if self.whole {
            return self.replace.clone();
        }
        if self.case {
            return s.replace(&self.find, &self.replace);
        }
        // case-blind replace of every occurrence
        let low = s.to_ascii_lowercase();
        let pat = self.find.to_ascii_lowercase();
        let mut out = String::new();
        let mut i = 0;
        while let Some(j) = low[i..].find(&pat) {
            out.push_str(&s[i..i + j]);
            out.push_str(&self.replace);
            i += j + pat.len();
        }
        out.push_str(&s[i..]);
        out
    }
    fn in_brushes(&self, id: BrushId) -> bool {
        self.brushes.as_ref().map_or(true, |l| l.contains(&id.0))
    }
    fn in_entities(&self, id: EntityId) -> bool {
        self.entities.as_ref().map_or(true, |l| l.contains(&id.0))
    }
    fn scoped(&self) -> bool {
        self.brushes.is_some() || self.entities.is_some()
    }
}

/// What a search found: the brushes and entities it's in, and how many places.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Found {
    pub brushes: Vec<u64>,
    pub entities: Vec<u64>,
    pub count: usize,
}

pub fn find(doc: &MapDocument, spec: &FindSpec) -> Found {
    let mut f = Found::default();
    match spec.field {
        Field::Texture => {
            for (id, b) in &doc.brushes {
                // a scoped search also takes brushes of picked entities
                let inside = spec.in_brushes(*id) || (spec.scoped() && b.entity.is_some_and(|e| spec.in_entities(e) && spec.entities.is_some()));
                if !inside {
                    continue;
                }
                let n = b.faces.iter().filter(|fc| spec.matches(&fc.texture)).count();
                if n > 0 {
                    f.brushes.push(id.0);
                    f.count += n;
                }
            }
        }
        Field::Classname | Field::Key | Field::Value => {
            for (id, e) in &doc.entities {
                if !spec.in_entities(*id) {
                    continue;
                }
                let n = match spec.field {
                    Field::Classname => usize::from(spec.matches(e.classname())),
                    Field::Key => e.keys.iter().filter(|(k, _)| k != "classname" && spec.matches(k)).count(),
                    _ => e.keys.iter().filter(|(k, v)| k != "classname" && spec.key.as_deref().map_or(true, |want| want == k) && spec.matches(v)).count(),
                };
                if n > 0 {
                    f.entities.push(id.0);
                    f.count += n;
                }
            }
        }
    }
    f
}

fn check_text(field: Field, s: &str) -> OpResult<()> {
    if s.chars().any(|c| c == '"' || c == '\n' || c == '\r') {
        return Err("Names and values can't have quotes or line breaks.".into());
    }
    match field {
        Field::Texture => {
            if s.is_empty() || s.len() > 15 || !s.is_ascii() || s.contains(char::is_whitespace) {
                return Err("A texture name is 1 to 15 letters, digits or symbols, without spaces.".into());
            }
        }
        Field::Classname | Field::Key => {
            if s.is_empty() || s.contains(char::is_whitespace) {
                return Err("Names can't be empty or have spaces.".into());
            }
            if s.len() > 63 {
                return Err("That name is too long (63 characters at most).".into());
            }
        }
        Field::Value => {
            if s.len() > 1023 {
                return Err("That value is too long (1023 characters at most).".into());
            }
        }
    }
    Ok(())
}

/// Replace everywhere the search matches; returns how many places changed.
pub fn cmd_replace(doc: &mut MapDocument, h: &mut History, spec: &FindSpec) -> OpResult<usize> {
    if spec.find.is_empty() {
        return Err("Type what to find first.".into());
    }
    let found = find(doc, spec);
    if found.count == 0 {
        return Ok(0);
    }
    let label = match spec.field {
        Field::Texture => "Replace textures",
        Field::Classname => "Replace classnames",
        Field::Key => "Rename keys",
        Field::Value => "Replace values",
    };
    let src = doc.clone();
    let mut tx = Tx::new(doc, label);
    let mut n = 0;
    match spec.field {
        Field::Texture => {
            for id in &found.brushes {
                let mut b = src.brushes[&BrushId(*id)].clone();
                for f in b.faces.iter_mut() {
                    if spec.matches(&f.texture) {
                        let t = spec.replaced(&f.texture);
                        check_text(Field::Texture, &t)?;
                        f.texture = t;
                        n += 1;
                    }
                }
                tx.set_brush(BrushId(*id), b);
            }
        }
        _ => {
            for id in &found.entities {
                let mut e = src.entities[&EntityId(*id)].clone();
                let mut keys = Vec::with_capacity(e.keys.len());
                for (k, v) in std::mem::take(&mut e.keys) {
                    let (mut k2, mut v2) = (k.clone(), v.clone());
                    match spec.field {
                        Field::Classname if k == "classname" && spec.matches(&v) => {
                            v2 = spec.replaced(&v);
                            check_text(Field::Classname, &v2)?;
                            n += 1;
                        }
                        Field::Key if k != "classname" && spec.matches(&k) => {
                            k2 = spec.replaced(&k);
                            check_text(Field::Key, &k2)?;
                            n += 1;
                        }
                        Field::Value if k != "classname" && spec.key.as_deref().map_or(true, |want| want == k) && spec.matches(&v) => {
                            v2 = spec.replaced(&v);
                            check_text(Field::Value, &v2)?;
                            n += 1;
                        }
                        _ => {}
                    }
                    // a renamed key that lands on one already there replaces it
                    if let Some(i) = keys.iter().position(|(a, _): &(String, String)| *a == k2) {
                        keys.remove(i);
                    }
                    keys.push((k2, v2));
                }
                e.keys = keys;
                tx.set_entity(EntityId(*id), e);
            }
        }
    }
    tx.commit(h);
    Ok(n)
}

// ---- select similar / touching / inside ----------------------------------------------

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "by", rename_all = "lowercase")]
pub enum Similar {
    /// brushes with any of these textures (or those the picked brushes use)
    Texture {
        #[serde(default)]
        textures: Vec<String>,
    },
    /// entities of the picked entities' classes (world brushes: every world brush)
    Class,
    /// entities with the same value of `key` as a picked one
    Value { key: String },
}

/// The selection grown to everything like it (the picked things included).
pub fn similar(doc: &MapDocument, brushes: &[BrushId], entities: &[EntityId], by: &Similar) -> (Vec<BrushId>, Vec<EntityId>) {
    let mut ob: BTreeSet<BrushId> = BTreeSet::new();
    let mut oe: BTreeSet<EntityId> = BTreeSet::new();
    match by {
        Similar::Texture { textures } => {
            let mut want: HashSet<String> = textures.iter().map(|t| t.to_ascii_lowercase()).collect();
            if want.is_empty() {
                let mut ids: Vec<BrushId> = brushes.to_vec();
                for e in entities {
                    ids.extend(doc.brushes_of(Some(*e)).map(|(b, _)| b));
                }
                for id in ids {
                    if let Some(b) = doc.brushes.get(&id) {
                        want.extend(b.faces.iter().map(|f| f.texture.to_ascii_lowercase()));
                    }
                }
            }
            for (id, b) in &doc.brushes {
                if b.faces.iter().any(|f| want.contains(&f.texture.to_ascii_lowercase())) {
                    match b.entity {
                        Some(e) => {
                            oe.insert(e);
                        }
                        None => {
                            ob.insert(*id);
                        }
                    }
                }
            }
        }
        Similar::Class => {
            let classes: HashSet<&str> = entities.iter().filter_map(|e| doc.entities.get(e)).map(|e| e.classname()).collect();
            let world = brushes.iter().any(|b| doc.brushes.get(b).is_some_and(|b| b.entity.is_none()));
            let tied: HashSet<&str> = brushes.iter().filter_map(|b| doc.brushes.get(b)?.entity).filter_map(|e| doc.entities.get(&e)).map(|e| e.classname()).collect();
            for (id, e) in &doc.entities {
                if classes.contains(e.classname()) || tied.contains(e.classname()) {
                    oe.insert(*id);
                }
            }
            if world {
                ob.extend(doc.brushes_of(None).map(|(b, _)| b));
            }
        }
        Similar::Value { key } => {
            let vals: HashSet<&str> = entities.iter().filter_map(|e| doc.entities.get(e)?.get(key)).collect();
            for (id, e) in &doc.entities {
                if e.get(key).is_some_and(|v| vals.contains(v)) {
                    oe.insert(*id);
                }
            }
        }
    }
    (ob.into_iter().collect(), oe.into_iter().collect())
}

/// One thing that moves as a whole: a world brush, or an entity with its brushes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Obj {
    Brush(BrushId),
    Entity(EntityId),
}

fn obj_bounds(doc: &MapDocument, o: Obj) -> Option<Bounds> {
    match o {
        Obj::Brush(id) => doc.brushes.get(&id).map(brush_bounds),
        Obj::Entity(id) => {
            let e = doc.entities.get(&id)?;
            let mut b = Bounds::empty();
            let mut any = false;
            for (_, br) in doc.brushes_of(Some(id)) {
                b = b.union(&brush_bounds(br));
                any = true;
            }
            if !any {
                let o = e.origin()?;
                b.add(o);
            }
            Some(b)
        }
    }
}

/// The objects a selection is made of (brushes of picked entities move with them).
pub fn objects(doc: &MapDocument, brushes: &[BrushId], entities: &[EntityId]) -> Vec<Obj> {
    let ents: HashSet<EntityId> = entities.iter().copied().collect();
    let mut out: Vec<Obj> = entities.iter().filter(|e| doc.entities.contains_key(e)).map(|e| Obj::Entity(*e)).collect();
    for b in brushes {
        match doc.brushes.get(b) {
            Some(br) if br.entity.is_none() => out.push(Obj::Brush(*b)),
            Some(br) => {
                let e = br.entity.unwrap_or(EntityId(0));
                if !ents.contains(&e) && !out.contains(&Obj::Entity(e)) {
                    out.push(Obj::Entity(e));
                }
            }
            None => {}
        }
    }
    out
}

fn sel_bounds(doc: &MapDocument, objs: &[Obj]) -> Option<Bounds> {
    let mut b = Bounds::empty();
    for o in objs {
        b = b.union(&obj_bounds(doc, *o)?);
    }
    (!b.is_empty()).then_some(b)
}

/// Things (not picked) whose box touches (`inside` false) or sits wholly inside (`inside`
/// true) the box round the selection.
pub fn touching(doc: &MapDocument, brushes: &[BrushId], entities: &[EntityId], inside: bool) -> (Vec<BrushId>, Vec<EntityId>) {
    let objs = objects(doc, brushes, entities);
    let Some(sb) = sel_bounds(doc, &objs) else { return (vec![], vec![]) };
    let picked: HashSet<Obj> = objs.into_iter().collect();
    let eps = 1e-3;
    let hit = |b: &Bounds| {
        if inside {
            b.min.x >= sb.min.x - eps && b.min.y >= sb.min.y - eps && b.min.z >= sb.min.z - eps && b.max.x <= sb.max.x + eps && b.max.y <= sb.max.y + eps && b.max.z <= sb.max.z + eps
        } else {
            b.min.x <= sb.max.x + eps && b.max.x >= sb.min.x - eps && b.min.y <= sb.max.y + eps && b.max.y >= sb.min.y - eps && b.min.z <= sb.max.z + eps && b.max.z >= sb.min.z - eps
        }
    };
    let mut ob = vec![];
    let mut oe = vec![];
    for (id, b) in doc.brushes_of(None) {
        if !picked.contains(&Obj::Brush(id)) && hit(&brush_bounds(b)) {
            ob.push(id);
        }
    }
    for id in doc.entities.keys() {
        if doc.entities[id].classname() == "worldspawn" || picked.contains(&Obj::Entity(*id)) {
            continue;
        }
        if obj_bounds(doc, Obj::Entity(*id)).is_some_and(|b| !b.is_empty() && hit(&b)) {
            oe.push(*id);
        }
    }
    (ob, oe)
}

// ---- align and distribute ------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Min,
    Centre,
    Max,
}

fn move_obj(tx: &mut Tx, o: Obj, d: Vec3, lock: bool) -> OpResult<()> {
    if d.x == 0.0 && d.y == 0.0 && d.z == 0.0 {
        return Ok(());
    }
    match o {
        Obj::Brush(id) => {
            let b = tx.doc().brushes.get(&id).cloned().ok_or("That brush no longer exists.")?;
            tx.set_brush(id, ops::translate(&b, d, lock)?);
        }
        Obj::Entity(id) => {
            let ids: Vec<BrushId> = tx.doc().brushes_of(Some(id)).map(|(b, _)| b).collect();
            for bid in &ids {
                let b = tx.doc().brushes[bid].clone();
                tx.set_brush(*bid, ops::translate(&b, d, lock)?);
            }
            if ids.is_empty() {
                let mut e = tx.doc().entities.get(&id).cloned().ok_or("That entity no longer exists.")?;
                move_origin(&mut e, d);
                tx.set_entity(id, e);
            }
        }
    }
    Ok(())
}

/// Line up every picked object with the selection's lowest edge, middle or highest edge on
/// one axis (0 x, 1 y, 2 z).
pub fn cmd_align_objects(doc: &mut MapDocument, h: &mut History, brushes: &[BrushId], entities: &[EntityId], ax: usize, edge: Edge, lock: bool) -> OpResult<usize> {
    if ax > 2 {
        return Err("That edit wasn't understood.".into());
    }
    let objs = objects(doc, brushes, entities);
    if objs.len() < 2 {
        return Err("Pick two or more things to line up.".into());
    }
    let sb = sel_bounds(doc, &objs).ok_or("Something picked has no place in the map.")?;
    let target = match edge {
        Edge::Min => axis(sb.min, ax),
        Edge::Max => axis(sb.max, ax),
        Edge::Centre => axis(sb.center(), ax),
    };
    let moves: Vec<(Obj, f64)> = objs
        .iter()
        .filter_map(|o| {
            let b = obj_bounds(doc, *o)?;
            let at = match edge {
                Edge::Min => axis(b.min, ax),
                Edge::Max => axis(b.max, ax),
                Edge::Centre => axis(b.center(), ax),
            };
            Some((*o, target - at))
        })
        .collect();
    let mut tx = Tx::new(doc, "Line up");
    let mut n = 0;
    for (o, d) in moves {
        if d.abs() > 1e-9 {
            move_obj(&mut tx, o, with_axis(ax, d), lock)?;
            n += 1;
        }
    }
    tx.commit(h);
    Ok(n)
}

/// Spread the picked objects out so the gaps between them are equal on one axis (the first
/// and last stay put).
pub fn cmd_distribute(doc: &mut MapDocument, h: &mut History, brushes: &[BrushId], entities: &[EntityId], ax: usize, lock: bool) -> OpResult<usize> {
    if ax > 2 {
        return Err("That edit wasn't understood.".into());
    }
    let objs = objects(doc, brushes, entities);
    if objs.len() < 3 {
        return Err("Pick three or more things to spread out.".into());
    }
    let mut items: Vec<(Obj, f64, f64)> = objs.iter().filter_map(|o| obj_bounds(doc, *o).map(|b| (*o, axis(b.min, ax), axis(b.max, ax)))).collect();
    items.sort_by(|a, b| (a.1 + a.2).partial_cmp(&(b.1 + b.2)).unwrap_or(std::cmp::Ordering::Equal));
    let first = items[0].1;
    let last = items[items.len() - 1].2;
    let sizes: f64 = items.iter().map(|i| i.2 - i.1).sum();
    let gap = (last - first - sizes) / (items.len() - 1) as f64;
    let mut at = first;
    let mut tx = Tx::new(doc, "Spread out");
    let mut n = 0;
    for (o, lo, hi) in items {
        let d = (at - lo).round_to_grid();
        if d.abs() > 1e-9 {
            move_obj(&mut tx, o, with_axis(ax, d), lock)?;
            n += 1;
        }
        at += (hi - lo) + gap;
    }
    tx.commit(h);
    Ok(n)
}

/// Spreading out lands on whole units when it's within a hair of one (no 0.0000001 noise).
trait Tidy {
    fn round_to_grid(self) -> f64;
}
impl Tidy for f64 {
    fn round_to_grid(self) -> f64 {
        let r = (self * 1024.0).round() / 1024.0;
        if (r - r.round()).abs() < 1e-6 {
            r.round()
        } else {
            r
        }
    }
}

// ---- many entities' keys -----------------------------------------------------------------

/// Sets (or, with an empty value, removes) one key on every listed entity, as one undo step.
pub fn cmd_set_key_many(doc: &mut MapDocument, h: &mut History, entities: &[EntityId], key: &str, value: &str) -> OpResult<usize> {
    let key = key.trim();
    if entities.is_empty() {
        return Err("Pick some entities first.".into());
    }
    if key.is_empty() || key.len() > 63 || key.contains(['"', '\n', '\r']) || key.contains(char::is_whitespace) {
        return Err("A key is 1 to 63 characters, without spaces or quotes.".into());
    }
    if value.len() > 1023 || value.contains(['"', '\n', '\r']) {
        return Err("A value is at most 1023 characters, without quotes or line breaks.".into());
    }
    if key == "classname" && value.trim().is_empty() {
        return Err("Every entity needs a classname.".into());
    }
    let mut changed = Vec::new();
    for id in entities {
        let mut e = doc.entities.get(id).cloned().ok_or("That entity no longer exists.")?;
        let before = e.get(key).map(str::to_string);
        if value.is_empty() {
            e.remove(key);
        } else {
            e.set(key, value);
        }
        if e.get(key).map(str::to_string) != before {
            changed.push((*id, e));
        }
    }
    if changed.is_empty() {
        return Ok(0);
    }
    let n = changed.len();
    let mut tx = Tx::new(doc, &format!("Set {key} on {n}"));
    for (id, e) in changed {
        tx.set_entity(id, e);
    }
    tx.commit(h);
    Ok(n)
}

// ---- groups (outliner) ----------------------------------------------------------------

pub fn cmd_rename_group(doc: &mut MapDocument, h: &mut History, id: GroupId, name: &str) -> OpResult<()> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 || name.contains(['"', '\n', '\r']) {
        return Err("A group name is 1 to 64 characters, without quotes.".into());
    }
    let mut g = doc.groups.get(&id).cloned().ok_or("That group no longer exists.")?;
    if g.name == name {
        return Ok(());
    }
    g.name = name.to_string();
    let mut tx = Tx::new(doc, "Rename group");
    tx.set_group(id, g);
    tx.commit(h);
    Ok(())
}

/// Puts things into a group (`None`: out of any group). Brushes of a brush entity follow the
/// entity; a group can't go inside itself or its own children.
pub fn cmd_move_to_group(doc: &mut MapDocument, h: &mut History, group: Option<GroupId>, brushes: &[BrushId], entities: &[EntityId], groups: &[GroupId]) -> OpResult<usize> {
    if let Some(g) = group {
        if !doc.groups.contains_key(&g) {
            return Err("That group no longer exists.".into());
        }
    }
    for gid in groups {
        // walking up from the target must never meet the group being moved
        let mut cur = group;
        for _ in 0..64 {
            match cur {
                Some(c) if c == *gid => return Err("A group can't go inside itself.".into()),
                Some(c) => cur = doc.groups.get(&c).and_then(|x| x.parent),
                None => break,
            }
        }
    }
    let src = doc.clone();
    let mut tx = Tx::new(doc, if group.is_some() { "Move into group" } else { "Take out of group" });
    let mut n = 0;
    let mut ents: BTreeSet<EntityId> = entities.iter().copied().collect();
    for b in brushes {
        let br = src.brushes.get(b).ok_or("That brush no longer exists.")?;
        match br.entity {
            Some(e) => {
                ents.insert(e);
            }
            None if br.group != group => {
                let mut nb = br.clone();
                nb.group = group;
                tx.set_brush(*b, nb);
                n += 1;
            }
            None => {}
        }
    }
    for e in ents {
        let en = src.entities.get(&e).ok_or("That entity no longer exists.")?;
        if en.group != group {
            let mut ne = en.clone();
            ne.group = group;
            tx.set_entity(e, ne);
            n += 1;
        }
    }
    for gid in groups {
        let g = src.groups.get(gid).ok_or("That group no longer exists.")?;
        if g.parent != group && Some(*gid) != group {
            let mut ng = g.clone();
            ng.parent = group;
            tx.set_group(*gid, ng);
            n += 1;
        }
    }
    tx.commit(h);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::math::v3;
    use crate::editor::ops::make_box;

    fn ent(keys: &[(&str, &str)]) -> Entity {
        Entity { keys: keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), group: None }
    }

    fn sample() -> (MapDocument, Vec<BrushId>, Vec<EntityId>) {
        let mut d = MapDocument::new();
        let mut h = History::default();
        let mut bs = vec![];
        for i in 0..3 {
            let mut b = make_box(v3(i as f64 * 100.0, 0.0, 0.0), v3(i as f64 * 100.0 + 32.0, 32.0, 32.0), if i == 1 { "CRATE1" } else { "WALL" });
            if i == 2 {
                b.faces[0].texture = "crate1".into();
            }
            bs.push(super::super::history::cmd_add_brush(&mut d, &mut h, b, "Box"));
        }
        let mut es = vec![];
        for e in [
            ent(&[("classname", "func_button"), ("target", "door1")]),
            ent(&[("classname", "func_door"), ("targetname", "door1")]),
            ent(&[("classname", "light"), ("origin", "0 0 64"), ("target", "outside")]),
            ent(&[("classname", "info_target"), ("targetname", "outside"), ("origin", "512 0 0")]),
            ent(&[("classname", "multi_manager"), ("targetname", "mm"), ("door1", "0"), ("door1#1", "2"), ("origin", "0 0 0")]),
        ] {
            es.push(super::super::history::cmd_add_entity(&mut d, &mut h, e));
        }
        (d, bs, es)
    }

    #[test]
    fn next_names_count_up() {
        let taken: HashSet<String> = ["door1", "door2", "lift", "lift_2", "007"].iter().map(|s| s.to_string()).collect();
        assert_eq!(next_name("door1", &taken), "door3");
        assert_eq!(next_name("lift", &taken), "lift_3");
        assert_eq!(next_name("gate", &taken), "gate_2");
        assert_eq!(next_name("007", &taken), "008");
        assert_eq!(next_name("cam09", &taken), "cam10");
    }

    #[test]
    fn duplicate_renames_and_keeps_links_inside_the_copy() {
        let (mut d, _, es) = sample();
        let mut h = History::default();
        // copy the button, the door and the light (its target "outside" isn't copied)
        let out = cmd_duplicate(&mut d, &mut h, &[], &[es[0], es[1], es[2], es[4]], v3(0.0, 0.0, 128.0), true).unwrap();
        assert_eq!(out.entities.len(), 4);
        let c: Vec<&Entity> = out.entities.iter().map(|id| &d.entities[id]).collect();
        assert_eq!(c[1].get("targetname"), Some("door2"));
        assert_eq!(c[0].get("target"), Some("door2"), "the copied button opens the copied door");
        assert_eq!(c[2].get("target"), Some("outside"), "a link to something not copied stays");
        assert_eq!(c[2].get("origin"), Some("0 0 192"));
        assert_eq!(c[3].get("targetname"), Some("mm_2"));
        assert_eq!(c[3].get("door2"), Some("0"));
        assert_eq!(c[3].get("door2#1"), Some("2"));
        // the originals are untouched
        assert_eq!(d.entities[&es[1]].get("targetname"), Some("door1"));
        assert_eq!(d.entities[&es[0]].get("target"), Some("door1"));
        // one undo step takes it all back
        h.undo(&mut d);
        assert_eq!(d.entities.len(), 5);
    }

    #[test]
    fn duplicate_without_renaming_and_in_place() {
        let (mut d, bs, es) = sample();
        let mut h = History::default();
        let out = cmd_duplicate(&mut d, &mut h, &[bs[0]], &[es[1]], Vec3::ZERO, false).unwrap();
        assert_eq!(out.brushes.len(), 1);
        assert_eq!(d.entities[&out.entities[0]].get("targetname"), Some("door1"));
        assert_eq!(brush_bounds(&d.brushes[&out.brushes[0]]), brush_bounds(&d.brushes[&bs[0]]));
        assert!(cmd_duplicate(&mut d, &mut h, &[], &[], Vec3::ZERO, true).is_err());
    }

    #[test]
    fn duplicate_takes_a_brush_entity_with_its_brushes() {
        let (mut d, bs, es) = sample();
        let mut h = History::default();
        super::super::history::cmd_tie_to_entity(&mut d, &mut h, &[bs[1]], "func_wall").unwrap();
        let wall = d.brushes[&bs[1]].entity.unwrap();
        let out = cmd_duplicate(&mut d, &mut h, &[], &[wall], v3(16.0, 0.0, 0.0), true).unwrap();
        assert_eq!(out.brushes.len(), 1);
        assert_eq!(d.brushes[&out.brushes[0]].entity, Some(out.entities[0]));
        let _ = es;
    }

    #[test]
    fn find_and_replace_textures() {
        let (mut d, bs, _) = sample();
        let mut h = History::default();
        let spec = FindSpec { field: Field::Texture, find: "crate1".into(), replace: "CRATE2".into(), key: None, whole: true, case: false, brushes: None, entities: None };
        let f = find(&d, &spec);
        assert_eq!(f.brushes, vec![bs[1].0, bs[2].0]);
        assert_eq!(f.count, 7);
        assert_eq!(cmd_replace(&mut d, &mut h, &spec).unwrap(), 7);
        assert!(d.brushes[&bs[1]].faces.iter().all(|f| f.texture == "CRATE2"));
        assert_eq!(d.brushes[&bs[2]].faces[0].texture, "CRATE2");
        assert_eq!(d.brushes[&bs[2]].faces[1].texture, "WALL");
        h.undo(&mut d);
        assert_eq!(d.brushes[&bs[1]].faces[0].texture, "CRATE1");
        // partial, in the selection only
        let spec = FindSpec { whole: false, find: "AT".into(), replace: "UMB".into(), brushes: Some(vec![bs[2].0]), ..spec };
        assert_eq!(cmd_replace(&mut d, &mut h, &spec).unwrap(), 1);
        assert_eq!(d.brushes[&bs[2]].faces[0].texture, "crUMBe1");
        assert_eq!(d.brushes[&bs[1]].faces[0].texture, "CRATE1");
        // too long a texture name: nothing changes
        let spec = FindSpec { field: Field::Texture, find: "WALL".into(), replace: "A_VERY_LONG_TEXTURE".into(), key: None, whole: true, case: false, brushes: None, entities: None };
        assert!(cmd_replace(&mut d, &mut h, &spec).is_err());
        assert_eq!(d.brushes[&bs[0]].faces[0].texture, "WALL");
    }

    #[test]
    fn find_and_replace_keys_values_and_classes() {
        let (mut d, _, es) = sample();
        let mut h = History::default();
        let v = FindSpec { field: Field::Value, find: "door1".into(), replace: "gate".into(), key: Some("target".into()), whole: true, case: true, brushes: None, entities: None };
        assert_eq!(find(&d, &v).entities, vec![es[0].0]);
        assert_eq!(cmd_replace(&mut d, &mut h, &v).unwrap(), 1);
        assert_eq!(d.entities[&es[0]].get("target"), Some("gate"));
        assert_eq!(d.entities[&es[1]].get("targetname"), Some("door1"), "only the key asked for");
        let c = FindSpec { field: Field::Classname, find: "func_door".into(), replace: "func_door_rotating".into(), ..v.clone() };
        assert_eq!(cmd_replace(&mut d, &mut h, &c).unwrap(), 1);
        assert_eq!(d.entities[&es[1]].classname(), "func_door_rotating");
        let k = FindSpec { field: Field::Key, find: "target".into(), replace: "killtarget".into(), key: None, ..v };
        assert_eq!(cmd_replace(&mut d, &mut h, &k).unwrap(), 2);
        assert_eq!(d.entities[&es[2]].get("killtarget"), Some("outside"));
        assert_eq!(d.entities[&es[2]].get("target"), None);
        assert_eq!(d.entities[&es[0]].keys[0].0, "classname", "key order kept");
    }

    #[test]
    fn select_similar() {
        let (d, bs, es) = sample();
        let (b, e) = similar(&d, &[bs[1]], &[], &Similar::Texture { textures: vec![] });
        assert_eq!(b, vec![bs[1], bs[2]]);
        assert!(e.is_empty());
        let (b, _) = similar(&d, &[], &[], &Similar::Texture { textures: vec!["wall".into()] });
        assert_eq!(b, vec![bs[0], bs[2]]);
        let (_, e) = similar(&d, &[], &[es[3]], &Similar::Class);
        assert_eq!(e, vec![es[3]]);
        let (b, e) = similar(&d, &[bs[0]], &[es[2]], &Similar::Class);
        assert_eq!(b, bs);
        assert_eq!(e, vec![es[2]]);
        let (_, e) = similar(&d, &[], &[es[3]], &Similar::Value { key: "targetname".into() });
        assert_eq!(e, vec![es[3]]);
        let (_, e) = similar(&d, &[], &[es[2]], &Similar::Value { key: "classname".into() });
        assert_eq!(e, vec![es[2]]);
    }

    #[test]
    fn touching_and_inside() {
        let (mut d, bs, _) = sample();
        let mut h = History::default();
        // a big box round the first two, and one touching the first
        let big = super::super::history::cmd_add_brush(&mut d, &mut h, make_box(v3(-8.0, -8.0, -8.0), v3(140.0, 40.0, 40.0), "WALL"), "Box");
        let (b, e) = touching(&d, &[big], &[], true);
        assert_eq!(b, vec![bs[0], bs[1]]);
        assert!(e.iter().all(|id| d.entities[id].origin().is_some()));
        let (b, _) = touching(&d, &[bs[0]], &[], false);
        assert_eq!(b, vec![big]);
    }

    #[test]
    fn align_and_distribute() {
        let (mut d, bs, _) = sample();
        let mut h = History::default();
        // raise the middle box, then line their bottoms up
        let t = ops::translate(&d.brushes[&bs[1]], v3(0.0, 40.0, 50.0), true).unwrap();
        d.brushes.insert(bs[1], t);
        assert_eq!(cmd_align_objects(&mut d, &mut h, &bs, &[], 2, Edge::Min, true).unwrap(), 1);
        assert!(bs.iter().all(|b| brush_bounds(&d.brushes[b]).min.z == 0.0));
        cmd_align_objects(&mut d, &mut h, &bs, &[], 1, Edge::Max, true).unwrap();
        assert!(bs.iter().all(|b| brush_bounds(&d.brushes[b]).max.y == 72.0));
        // spread: move the middle one off centre, then even the gaps
        let t = ops::translate(&d.brushes[&bs[1]], v3(-50.0, 0.0, 0.0), true).unwrap();
        d.brushes.insert(bs[1], t);
        cmd_distribute(&mut d, &mut h, &bs, &[], 0, true).unwrap();
        let xs: Vec<f64> = bs.iter().map(|b| brush_bounds(&d.brushes[b]).min.x).collect();
        assert_eq!(xs, vec![0.0, 100.0, 200.0]);
        assert!(cmd_distribute(&mut d, &mut h, &bs[..2], &[], 0, true).is_err());
        assert!(cmd_align_objects(&mut d, &mut h, &bs[..1], &[], 0, Edge::Min, true).is_err());
        // undo the spread: the off-centre place comes back
        h.undo(&mut d);
        assert_eq!(brush_bounds(&d.brushes[&bs[1]]).min.x, 50.0);
    }

    #[test]
    fn align_moves_point_entities_by_origin() {
        let (mut d, _, es) = sample();
        let mut h = History::default();
        cmd_align_objects(&mut d, &mut h, &[], &[es[2], es[3]], 2, Edge::Max, true).unwrap();
        assert_eq!(d.entities[&es[3]].get("origin"), Some("512 0 64"));
    }

    #[test]
    fn groups_rename_and_move() {
        let (mut d, bs, es) = sample();
        let mut h = History::default();
        let g = super::super::history::cmd_group(&mut d, &mut h, "Room", &[bs[0]], &[]).unwrap();
        let g2 = super::super::history::cmd_group(&mut d, &mut h, "Crates", &[bs[1]], &[]).unwrap();
        cmd_rename_group(&mut d, &mut h, g, "Hall").unwrap();
        assert_eq!(d.groups[&g].name, "Hall");
        assert!(cmd_rename_group(&mut d, &mut h, g, "  ").is_err());
        assert_eq!(cmd_move_to_group(&mut d, &mut h, Some(g), &[bs[2]], &[es[2]], &[g2]).unwrap(), 3);
        assert_eq!(d.brushes[&bs[2]].group, Some(g));
        assert_eq!(d.entities[&es[2]].group, Some(g));
        assert_eq!(d.groups[&g2].parent, Some(g));
        assert!(cmd_move_to_group(&mut d, &mut h, Some(g2), &[], &[], &[g]).is_err(), "no loops");
        cmd_move_to_group(&mut d, &mut h, None, &[bs[2]], &[], &[]).unwrap();
        assert_eq!(d.brushes[&bs[2]].group, None);
        h.undo(&mut d);
        assert_eq!(d.brushes[&bs[2]].group, Some(g));
    }

    #[test]
    fn set_a_key_on_many() {
        let (mut d, _, es) = sample();
        let mut h = History::default();
        assert_eq!(cmd_set_key_many(&mut d, &mut h, &[es[0], es[1], es[2]], "rendermode", "4").unwrap(), 3);
        assert!(es[..3].iter().all(|e| d.entities[e].get("rendermode") == Some("4")));
        // the same again changes nothing and makes no step
        assert_eq!(cmd_set_key_many(&mut d, &mut h, &[es[0], es[1]], "rendermode", "4").unwrap(), 0);
        // empty removes; one undo puts all of them back
        assert_eq!(cmd_set_key_many(&mut d, &mut h, &[es[0], es[1], es[2]], "rendermode", "").unwrap(), 3);
        h.undo(&mut d);
        assert!(es[..3].iter().all(|e| d.entities[e].get("rendermode") == Some("4")));
        assert!(cmd_set_key_many(&mut d, &mut h, &[es[0]], "bad key", "1").is_err());
        assert!(cmd_set_key_many(&mut d, &mut h, &[es[0]], "classname", "").is_err());
        assert!(cmd_set_key_many(&mut d, &mut h, &[es[0]], "message", "a \"quote\"").is_err());
    }
}
