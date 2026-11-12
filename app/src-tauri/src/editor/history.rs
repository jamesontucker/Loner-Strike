//! Undo/redo. Every edit is a `Change`: the before and after of each brush, entity, group or
//! world setting it touched, so it can be applied and reverted exactly. Edits are made in a
//! `Tx` (transaction) that applies as it goes and either commits to the `History` or rolls
//! back. The `cmd_*` functions are the editor's commands, built on that.

use super::doc::{Brush, BrushId, Entity, EntityId, Group, GroupId, MapDocument, TexAlign, WorldSettings};
use super::geom::{brush_bounds, brush_polygons};
use super::math::{Affine, Plane, Vec3};
use super::ops::{self, OpResult};
use super::mapfile::fmt_num;

#[derive(Clone, Debug, PartialEq)]
enum Item {
    Brush { id: BrushId, before: Option<Brush>, after: Option<Brush> },
    Entity { id: EntityId, before: Option<Entity>, after: Option<Entity> },
    Group { id: GroupId, before: Option<Group>, after: Option<Group> },
    World { before: WorldSettings, after: WorldSettings },
}

/// One undoable step.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub label: String,
    /// Consecutive changes with the same key fold into one undo step (a face drag sends many
    /// small moves; undo takes back the whole drag).
    pub merge_key: Option<String>,
    items: Vec<Item>,
    serial: u64,
}

fn put<K: Ord + Copy, V: Clone>(map: &mut std::collections::BTreeMap<K, V>, id: K, v: &Option<V>) {
    match v {
        Some(v) => {
            map.insert(id, v.clone());
        }
        None => {
            map.remove(&id);
        }
    }
}

impl Change {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn apply_items(doc: &mut MapDocument, items: &[Item], forward: bool) {
        let mut apply = |it: &Item| match it {
            Item::Brush { id, before, after } => put(&mut doc.brushes, *id, if forward { after } else { before }),
            Item::Entity { id, before, after } => put(&mut doc.entities, *id, if forward { after } else { before }),
            Item::Group { id, before, after } => put(&mut doc.groups, *id, if forward { after } else { before }),
            Item::World { before, after } => doc.world = if forward { after.clone() } else { before.clone() },
        };
        if forward {
            items.iter().for_each(&mut apply);
        } else {
            items.iter().rev().for_each(&mut apply);
        }
        doc.revision += 1;
    }

    pub fn apply(&self, doc: &mut MapDocument) {
        Change::apply_items(doc, &self.items, true);
    }
    pub fn revert(&self, doc: &mut MapDocument) {
        Change::apply_items(doc, &self.items, false);
    }

    /// Fold a later change into this one (keep our befores, take its afters).
    fn absorb(&mut self, later: Change) {
        for it in later.items {
            record(&mut self.items, it);
        }
        self.items.retain(|it| !unchanged(it));
    }

    pub fn touched_brushes(&self) -> Vec<BrushId> {
        self.items.iter().filter_map(|i| if let Item::Brush { id, .. } = i { Some(*id) } else { None }).collect()
    }
}

fn unchanged(it: &Item) -> bool {
    match it {
        Item::Brush { before, after, .. } => before == after,
        Item::Entity { before, after, .. } => before == after,
        Item::Group { before, after, .. } => before == after,
        Item::World { before, after } => before == after,
    }
}

/// Add an item, merging with an earlier one for the same object.
fn record(items: &mut Vec<Item>, it: Item) {
    for old in items.iter_mut() {
        match (old, &it) {
            (Item::Brush { id: a, after, .. }, Item::Brush { id: b, after: new, .. }) if a == b => {
                *after = new.clone();
                return;
            }
            (Item::Entity { id: a, after, .. }, Item::Entity { id: b, after: new, .. }) if a == b => {
                *after = new.clone();
                return;
            }
            (Item::Group { id: a, after, .. }, Item::Group { id: b, after: new, .. }) if a == b => {
                *after = new.clone();
                return;
            }
            (Item::World { after, .. }, Item::World { after: new, .. }) => {
                *after = new.clone();
                return;
            }
            _ => {}
        }
    }
    items.push(it);
}

/// A transaction: edits apply to the document right away and are recorded. Finish with
/// `commit` (one undo step) or `rollback` (as if nothing happened). Dropping a Tx without
/// either also rolls back, so a failed command never leaves half an edit behind.
pub struct Tx<'a> {
    doc: &'a mut MapDocument,
    change: Change,
    done: bool,
}

impl<'a> Tx<'a> {
    pub fn new(doc: &'a mut MapDocument, label: &str) -> Tx<'a> {
        Tx { doc, change: Change { label: label.into(), merge_key: None, items: Vec::new(), serial: 0 }, done: false }
    }
    pub fn merge_key(mut self, key: &str) -> Self {
        self.change.merge_key = Some(key.into());
        self
    }
    pub fn doc(&self) -> &MapDocument {
        self.doc
    }

    pub fn add_brush(&mut self, b: Brush) -> BrushId {
        let id = self.doc.new_brush_id();
        self.set_brush(id, b);
        id
    }
    pub fn set_brush(&mut self, id: BrushId, b: Brush) {
        let before = self.doc.brushes.insert(id, b.clone());
        record(&mut self.change.items, Item::Brush { id, before, after: Some(b) });
    }
    pub fn remove_brush(&mut self, id: BrushId) -> Option<Brush> {
        let before = self.doc.brushes.remove(&id)?;
        record(&mut self.change.items, Item::Brush { id, before: Some(before.clone()), after: None });
        Some(before)
    }
    pub fn add_entity(&mut self, e: Entity) -> EntityId {
        let id = self.doc.new_entity_id();
        self.set_entity(id, e);
        id
    }
    pub fn set_entity(&mut self, id: EntityId, e: Entity) {
        let before = self.doc.entities.insert(id, e.clone());
        record(&mut self.change.items, Item::Entity { id, before, after: Some(e) });
    }
    pub fn remove_entity(&mut self, id: EntityId) -> Option<Entity> {
        let before = self.doc.entities.remove(&id)?;
        record(&mut self.change.items, Item::Entity { id, before: Some(before.clone()), after: None });
        Some(before)
    }
    pub fn add_group(&mut self, g: Group) -> GroupId {
        let id = self.doc.new_group_id();
        self.set_group(id, g);
        id
    }
    pub fn set_group(&mut self, id: GroupId, g: Group) {
        let before = self.doc.groups.insert(id, g.clone());
        record(&mut self.change.items, Item::Group { id, before, after: Some(g) });
    }
    pub fn remove_group(&mut self, id: GroupId) -> Option<Group> {
        let before = self.doc.groups.remove(&id)?;
        record(&mut self.change.items, Item::Group { id, before: Some(before.clone()), after: None });
        Some(before)
    }
    pub fn set_world(&mut self, w: WorldSettings) {
        let before = std::mem::replace(&mut self.doc.world, w.clone());
        record(&mut self.change.items, Item::World { before, after: w });
    }

    pub fn commit(mut self, history: &mut History) -> bool {
        self.done = true;
        let mut change = std::mem::replace(&mut self.change, Change { label: String::new(), merge_key: None, items: Vec::new(), serial: 0 });
        change.items.retain(|it| !unchanged(it));
        if change.is_empty() {
            return false;
        }
        self.doc.revision += 1;
        history.push_applied(change);
        true
    }

    pub fn rollback(mut self) {
        self.undo_now();
    }

    fn undo_now(&mut self) {
        if !self.done {
            self.done = true;
            self.change.revert(self.doc);
        }
    }
}

impl Drop for Tx<'_> {
    fn drop(&mut self) {
        self.undo_now();
    }
}

/// The undo and redo stacks.
#[derive(Debug)]
pub struct History {
    undo: Vec<Change>,
    redo: Vec<Change>,
    limit: usize,
    next_serial: u64,
    saved_serial: u64,
}

impl Default for History {
    fn default() -> Self {
        History::new(500)
    }
}

impl History {
    pub fn new(limit: usize) -> History {
        History { undo: Vec::new(), redo: Vec::new(), limit: limit.max(1), next_serial: 1, saved_serial: 0 }
    }

    /// Record a change that is already applied to the document.
    fn push_applied(&mut self, mut change: Change) {
        self.redo.clear();
        if let (Some(last), Some(key)) = (self.undo.last_mut(), change.merge_key.as_ref()) {
            if last.merge_key.as_ref() == Some(key) && last.serial != self.saved_serial {
                last.absorb(change);
                if last.is_empty() {
                    self.undo.pop();
                }
                return;
            }
        }
        change.serial = self.next_serial;
        self.next_serial += 1;
        self.undo.push(change);
        if self.undo.len() > self.limit {
            let extra = self.undo.len() - self.limit;
            self.undo.drain(..extra);
        }
    }

    /// Apply a change built elsewhere and record it.
    pub fn run(&mut self, doc: &mut MapDocument, change: Change) {
        change.apply(doc);
        self.push_applied(change);
    }

    /// Undo the last step; returns its label.
    pub fn undo(&mut self, doc: &mut MapDocument) -> Option<String> {
        let c = self.undo.pop()?;
        c.revert(doc);
        let label = c.label.clone();
        self.redo.push(c);
        Some(label)
    }

    pub fn redo(&mut self, doc: &mut MapDocument) -> Option<String> {
        let c = self.redo.pop()?;
        c.apply(doc);
        let label = c.label.clone();
        self.undo.push(c);
        Some(label)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Labels for the Edit menu ("Undo Hollow").
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|c| c.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|c| c.label.as_str())
    }
    /// Call after saving; `is_dirty` is then false until the next change (or undo/redo away).
    pub fn mark_saved(&mut self) {
        self.saved_serial = self.undo.last().map_or(0, |c| c.serial);
    }
    pub fn is_dirty(&self) -> bool {
        self.undo.last().map_or(0, |c| c.serial) != self.saved_serial
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.saved_serial = 0;
    }
}

// ---------------------------------------------------------------------------------------
// Commands. Each one is a single undo step; on error nothing changes.

fn get_brush(doc: &MapDocument, id: BrushId) -> OpResult<Brush> {
    doc.brushes.get(&id).cloned().ok_or_else(|| "That brush no longer exists.".to_string())
}

pub fn cmd_add_brush(doc: &mut MapDocument, h: &mut History, b: Brush, label: &str) -> BrushId {
    let mut tx = Tx::new(doc, label);
    let id = tx.add_brush(b);
    tx.commit(h);
    id
}

/// Delete brushes and entities (an entity's brushes go with it).
pub fn cmd_delete(doc: &mut MapDocument, h: &mut History, brushes: &[BrushId], entities: &[EntityId]) {
    let mut tx = Tx::new(doc, "Delete");
    for id in brushes {
        tx.remove_brush(*id);
    }
    for e in entities {
        let owned: Vec<BrushId> = tx.doc().brushes_of(Some(*e)).map(|(id, _)| id).collect();
        for id in owned {
            tx.remove_brush(id);
        }
        tx.remove_entity(*e);
    }
    tx.commit(h);
}

/// Move/rotate/scale brushes and point entities together. Point entity origins follow the
/// transform; `angle`/`angles` yaw follows rotations about z.
pub fn cmd_transform(
    doc: &mut MapDocument,
    h: &mut History,
    brushes: &[BrushId],
    entities: &[EntityId],
    t: &Affine,
    lock: bool,
    label: &str,
    merge_key: Option<&str>,
) -> OpResult<()> {
    let mut tx = Tx::new(doc, label);
    if let Some(k) = merge_key {
        tx = tx.merge_key(k);
    }
    for id in brushes {
        let b = get_brush(tx.doc(), *id)?;
        tx.set_brush(*id, ops::transform(&b, t, lock)?);
    }
    let yaw = (t.m.r[1].x).atan2(t.m.r[0].x).to_degrees();
    let pure_z_rotation = t.m.r[2].approx_eq(Vec3::Z, 1e-9) && (t.m.det() - 1.0).abs() < 1e-9;
    for id in entities {
        let Some(mut e) = tx.doc().entities.get(id).cloned() else { continue };
        let has_brushes = tx.doc().brushes_of(Some(*id)).next().is_some();
        if let (Some(o), false) = (e.origin(), has_brushes) {
            let n = t.apply(o);
            e.set("origin", &format!("{} {} {}", fmt_num(n.x), fmt_num(n.y), fmt_num(n.z)));
        }
        if pure_z_rotation && yaw.abs() > 1e-9 {
            if let Some(a) = e.get("angle").and_then(|a| a.parse::<f64>().ok()) {
                if a >= 0.0 {
                    e.set("angle", &fmt_num((a + yaw).rem_euclid(360.0)));
                }
            } else if let Some(v) = e.get("angles").map(|s| s.split_whitespace().filter_map(|x| x.parse::<f64>().ok()).collect::<Vec<_>>()) {
                if v.len() == 3 {
                    e.set("angles", &format!("{} {} {}", fmt_num(v[0]), fmt_num((v[1] + yaw).rem_euclid(360.0)), fmt_num(v[2])));
                }
            }
        }
        tx.set_entity(*id, e);
    }
    tx.commit(h);
    Ok(())
}

/// Push/pull a face. Pass the same `drag_id` for every step of one drag so undo takes back
/// the whole drag at once.
pub fn cmd_move_face(doc: &mut MapDocument, h: &mut History, id: BrushId, face: usize, distance: f64, lock: bool, drag_id: Option<&str>) -> OpResult<()> {
    let b = get_brush(doc, id)?;
    let nb = ops::move_face(&b, face, distance, lock)?;
    let mut tx = Tx::new(doc, "Move face");
    if let Some(k) = drag_id {
        tx = tx.merge_key(k);
    }
    tx.set_brush(id, nb);
    tx.commit(h);
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    Back,
    Front,
    Both,
}

/// Clip brushes by a plane. Returns the ids of the pieces.
pub fn cmd_clip(doc: &mut MapDocument, h: &mut History, ids: &[BrushId], plane: &Plane, keep: Keep, texture: &str) -> OpResult<Vec<BrushId>> {
    let mut tx = Tx::new(doc, "Clip");
    let mut out = Vec::new();
    for id in ids {
        let b = get_brush(tx.doc(), *id)?;
        let (back, front) = ops::clip(&b, plane, texture);
        let mut pieces: Vec<Brush> = Vec::new();
        if keep != Keep::Front {
            pieces.extend(back);
        }
        if keep != Keep::Back {
            pieces.extend(front);
        }
        let mut it = pieces.into_iter();
        match it.next() {
            Some(first) => {
                tx.set_brush(*id, first);
                out.push(*id);
            }
            None => {
                tx.remove_brush(*id);
            }
        }
        for p in it {
            out.push(tx.add_brush(p));
        }
    }
    tx.commit(h);
    Ok(out)
}

pub fn cmd_hollow(doc: &mut MapDocument, h: &mut History, id: BrushId, thickness: f64) -> OpResult<Vec<BrushId>> {
    let b = get_brush(doc, id)?;
    let walls = ops::hollow(&b, thickness)?;
    let mut tx = Tx::new(doc, "Hollow");
    tx.remove_brush(id);
    let ids = walls.into_iter().map(|w| tx.add_brush(Brush { entity: b.entity, group: b.group, ..w })).collect();
    tx.commit(h);
    Ok(ids)
}

/// Carve the cutter out of every brush it overlaps (the cutter stays). Returns how many
/// brushes were cut.
pub fn cmd_carve(doc: &mut MapDocument, h: &mut History, cutter: BrushId, targets: &[BrushId]) -> OpResult<usize> {
    let c = get_brush(doc, cutter)?;
    let cb = brush_bounds(&c);
    let mut tx = Tx::new(doc, "Carve");
    let mut cut = 0;
    for id in targets.iter().filter(|t| **t != cutter) {
        let b = get_brush(tx.doc(), *id)?;
        if !brush_bounds(&b).overlaps(&cb) {
            continue;
        }
        let pieces = ops::subtract(&b, &c);
        if pieces.len() == 1 && pieces[0] == b {
            continue;
        }
        cut += 1;
        tx.remove_brush(*id);
        for p in pieces {
            tx.add_brush(Brush { entity: b.entity, group: b.group, ..p });
        }
    }
    tx.commit(h);
    Ok(cut)
}

pub fn cmd_merge(doc: &mut MapDocument, h: &mut History, ids: &[BrushId]) -> OpResult<(BrushId, bool)> {
    let bs = ids.iter().map(|i| get_brush(doc, *i)).collect::<OpResult<Vec<_>>>()?;
    let m = ops::merge(&bs)?;
    let mut tx = Tx::new(doc, "Merge");
    for id in &ids[1..] {
        tx.remove_brush(*id);
    }
    tx.set_brush(ids[0], m.brush);
    tx.commit(h);
    Ok((ids[0], m.exact))
}

/// Snap brush corners to the grid.
pub fn cmd_snap(doc: &mut MapDocument, h: &mut History, ids: &[BrushId], grid: f64) -> OpResult<()> {
    let mut tx = Tx::new(doc, "Snap to grid");
    for id in ids {
        let b = get_brush(tx.doc(), *id)?;
        tx.set_brush(*id, ops::snap_vertices(&b, grid)?);
    }
    tx.commit(h);
    Ok(())
}

/// Apply a texture to faces (`None` face index = every face of the brush).
pub fn cmd_set_texture(doc: &mut MapDocument, h: &mut History, faces: &[(BrushId, Option<usize>)], texture: &str) -> OpResult<()> {
    let mut tx = Tx::new(doc, "Apply texture");
    for (id, face) in faces {
        let mut b = get_brush(tx.doc(), *id)?;
        match face {
            Some(i) => b.faces.get_mut(*i).ok_or("That face doesn't exist.")?.texture = texture.into(),
            None => b.faces.iter_mut().for_each(|f| f.texture = texture.into()),
        }
        tx.set_brush(*id, b);
    }
    tx.commit(h);
    Ok(())
}

/// Change face alignment with a helper (fit, shift, rotate...). `f` gets the current
/// alignment, the face polygon and the texture size. Pass a drag id to fold a drag into one
/// undo step.
pub fn cmd_align(
    doc: &mut MapDocument,
    h: &mut History,
    faces: &[(BrushId, usize)],
    drag_id: Option<&str>,
    f: impl Fn(&TexAlign, &[Vec3], (f64, f64), Vec3) -> TexAlign,
) -> OpResult<()> {
    let mut tx = Tx::new(doc, "Align texture");
    if let Some(k) = drag_id {
        tx = tx.merge_key(k);
    }
    for (id, i) in faces {
        let mut b = get_brush(tx.doc(), *id)?;
        let polys = brush_polygons(&b);
        let face = b.faces.get_mut(*i).ok_or("That face doesn't exist.")?;
        let normal = face.plane().map(|p| p.normal).unwrap_or(Vec3::Z);
        let size = tx.doc().texture_size(&face.texture);
        face.align = f(&face.align, &polys[*i], size, normal);
        tx.set_brush(*id, b);
    }
    tx.commit(h);
    Ok(())
}

/// Set (or with an empty value, remove) an entity key.
pub fn cmd_set_key(doc: &mut MapDocument, h: &mut History, id: EntityId, key: &str, value: &str) -> OpResult<()> {
    let mut e = doc.entities.get(&id).cloned().ok_or("That entity no longer exists.")?;
    if key.is_empty() || key.contains('"') || value.contains('"') {
        return Err("Keys and values can't contain quotes.".into());
    }
    if value.is_empty() {
        if key == "classname" {
            return Err("Every entity needs a classname.".into());
        }
        e.remove(key);
    } else {
        e.set(key, value);
    }
    let mut tx = Tx::new(doc, &format!("Set {key}"));
    tx.set_entity(id, e);
    tx.commit(h);
    Ok(())
}

pub fn cmd_add_entity(doc: &mut MapDocument, h: &mut History, e: Entity) -> EntityId {
    let mut tx = Tx::new(doc, &format!("Add {}", e.classname()));
    let id = tx.add_entity(e);
    tx.commit(h);
    id
}

/// Turn world brushes into a brush entity ("tie to entity": func_wall, func_door...).
pub fn cmd_tie_to_entity(doc: &mut MapDocument, h: &mut History, ids: &[BrushId], classname: &str) -> OpResult<EntityId> {
    if ids.is_empty() {
        return Err("Pick some brushes first.".into());
    }
    let mut tx = Tx::new(doc, &format!("Tie to {classname}"));
    let eid = tx.add_entity(Entity::new(classname));
    for id in ids {
        let b = get_brush(tx.doc(), *id)?;
        tx.set_brush(*id, Brush { entity: Some(eid), ..b });
    }
    tx.commit(h);
    Ok(eid)
}

/// Send a brush entity's brushes back to the world and remove the entity.
pub fn cmd_move_to_world(doc: &mut MapDocument, h: &mut History, entity: EntityId) {
    let mut tx = Tx::new(doc, "Move to world");
    let owned: Vec<(BrushId, Brush)> = tx.doc().brushes_of(Some(entity)).map(|(i, b)| (i, b.clone())).collect();
    for (id, b) in owned {
        tx.set_brush(id, Brush { entity: None, ..b });
    }
    tx.remove_entity(entity);
    tx.commit(h);
}

pub fn cmd_set_world(doc: &mut MapDocument, h: &mut History, w: WorldSettings) {
    let mut tx = Tx::new(doc, "Map settings");
    tx.set_world(w);
    tx.commit(h);
}

/// Put brushes and entities in a new group.
pub fn cmd_group(doc: &mut MapDocument, h: &mut History, name: &str, brushes: &[BrushId], entities: &[EntityId]) -> OpResult<GroupId> {
    let mut tx = Tx::new(doc, "Group");
    let gid = tx.add_group(Group { name: name.into(), ..Default::default() });
    for id in brushes {
        let b = get_brush(tx.doc(), *id)?;
        tx.set_brush(*id, Brush { group: Some(gid), ..b });
    }
    for id in entities {
        let mut e = tx.doc().entities.get(id).cloned().ok_or("That entity no longer exists.")?;
        e.group = Some(gid);
        tx.set_entity(*id, e);
    }
    tx.commit(h);
    Ok(gid)
}

/// Remove a group; its members move to the group's parent.
pub fn cmd_ungroup(doc: &mut MapDocument, h: &mut History, gid: GroupId) {
    let mut tx = Tx::new(doc, "Ungroup");
    let Some(g) = tx.remove_group(gid) else { return };
    let bs: Vec<(BrushId, Brush)> = tx.doc().brushes.iter().filter(|(_, b)| b.group == Some(gid)).map(|(i, b)| (*i, b.clone())).collect();
    for (id, b) in bs {
        tx.set_brush(id, Brush { group: g.parent, ..b });
    }
    let es: Vec<(EntityId, Entity)> = tx.doc().entities.iter().filter(|(_, e)| e.group == Some(gid)).map(|(i, e)| (*i, e.clone())).collect();
    for (id, mut e) in es {
        e.group = g.parent;
        tx.set_entity(id, e);
    }
    let gs: Vec<(GroupId, Group)> = tx.doc().groups.iter().filter(|(_, c)| c.parent == Some(gid)).map(|(i, c)| (*i, c.clone())).collect();
    for (id, mut c) in gs {
        c.parent = g.parent;
        tx.set_group(id, c);
    }
    tx.commit(h);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::geom::brush_volume;
    use crate::editor::math::{v3, Mat3};
    use crate::editor::ops::make_box;
    use crate::editor::texture;

    fn setup() -> (MapDocument, History, BrushId) {
        let mut d = MapDocument::new();
        let mut h = History::default();
        let id = cmd_add_brush(&mut d, &mut h, make_box(Vec3::ZERO, v3(64.0, 64.0, 64.0), "CRATE"), "Box");
        (d, h, id)
    }

    #[test]
    fn undo_redo_round_trip() {
        let (mut d, mut h, id) = setup();
        let start = d.clone();
        cmd_hollow(&mut d, &mut h, id, 8.0).unwrap();
        assert_eq!(d.brushes.len(), 6);
        let hollowed = d.clone();
        assert_eq!(h.undo_label(), Some("Hollow"));
        assert_eq!(h.undo(&mut d).as_deref(), Some("Hollow"));
        assert_eq!(d.brushes, start.brushes);
        assert_eq!(h.redo(&mut d).as_deref(), Some("Hollow"));
        assert_eq!(d.brushes, hollowed.brushes);
        h.undo(&mut d);
        h.undo(&mut d);
        assert!(d.brushes.is_empty());
        assert!(!h.can_undo());
        assert!(h.undo(&mut d).is_none());
        h.redo(&mut d);
        // A new edit clears redo.
        cmd_set_texture(&mut d, &mut h, &[(id, None)], "METAL").unwrap();
        assert!(!h.can_redo());
        assert!(d.brushes[&id].faces.iter().all(|f| f.texture == "METAL"));
    }

    #[test]
    fn failed_commands_change_nothing() {
        let (mut d, mut h, id) = setup();
        let before = d.brushes.clone();
        assert!(cmd_hollow(&mut d, &mut h, id, 40.0).is_err());
        assert!(cmd_move_face(&mut d, &mut h, id, 0, -200.0, false, None).is_err());
        let other = cmd_add_brush(&mut d, &mut h, make_box(v3(100.0, 0.0, 0.0), v3(110.0, 10.0, 10.0), "X"), "Box");
        // The second brush can't be flattened, so the first must not move either.
        let flat = Affine::around(Mat3::scale(v3(0.0, 1.0, 1.0)), Vec3::ZERO);
        assert!(cmd_transform(&mut d, &mut h, &[id, other], &[], &flat, false, "Scale", None).is_err());
        assert_eq!(d.brushes[&id], before[&id]);
        assert_eq!(h.undo_label(), Some("Box"));
        // A dropped transaction rolls back too.
        {
            let mut tx = Tx::new(&mut d, "Abandoned");
            tx.remove_brush(id);
        }
        assert!(d.brushes.contains_key(&id));
    }

    #[test]
    fn drags_fold_into_one_step() {
        let (mut d, mut h, id) = setup();
        for _ in 0..4 {
            cmd_move_face(&mut d, &mut h, id, 0, 8.0, false, Some("drag-1")).unwrap();
        }
        assert!((brush_bounds(&d.brushes[&id]).max.z - 96.0).abs() < 1e-9);
        h.undo(&mut d);
        assert!((brush_bounds(&d.brushes[&id]).max.z - 64.0).abs() < 1e-9);
        assert_eq!(h.undo_label(), Some("Box"));
    }

    #[test]
    fn dirty_tracking() {
        let (mut d, mut h, id) = setup();
        assert!(h.is_dirty());
        h.mark_saved();
        assert!(!h.is_dirty());
        cmd_move_face(&mut d, &mut h, id, 0, 8.0, false, Some("d")).unwrap();
        assert!(h.is_dirty());
        h.undo(&mut d);
        assert!(!h.is_dirty());
        h.redo(&mut d);
        h.mark_saved();
        // A drag after saving starts a new step rather than folding into the saved one.
        cmd_move_face(&mut d, &mut h, id, 0, 8.0, false, Some("d")).unwrap();
        assert!(h.is_dirty());
        h.undo(&mut d);
        assert!(!h.is_dirty());
    }

    #[test]
    fn transform_moves_entities_and_turns_them() {
        let (mut d, mut h, id) = setup();
        let mut spawn = Entity::new("info_player_deathmatch");
        spawn.set("origin", "32 0 36");
        spawn.set("angle", "90");
        let sid = cmd_add_entity(&mut d, &mut h, spawn);
        let rot = Affine::around(Mat3::rotation(Vec3::Z, 90.0), Vec3::ZERO);
        cmd_transform(&mut d, &mut h, &[id], &[sid], &rot, true, "Rotate", None).unwrap();
        assert_eq!(d.entities[&sid].get("origin"), Some("0 32 36"));
        assert_eq!(d.entities[&sid].get("angle"), Some("180"));
        h.undo(&mut d);
        assert_eq!(d.entities[&sid].get("origin"), Some("32 0 36"));
    }

    #[test]
    fn carve_merge_clip_and_entities() {
        let (mut d, mut h, id) = setup();
        let cutter = cmd_add_brush(&mut d, &mut h, make_box(v3(16.0, 16.0, -8.0), v3(48.0, 48.0, 72.0), "X"), "Box");
        assert_eq!(cmd_carve(&mut d, &mut h, cutter, &[id, cutter]).unwrap(), 1);
        assert_eq!(d.brushes.len(), 5);
        h.undo(&mut d);
        assert_eq!(d.brushes.len(), 2);

        let ids = cmd_clip(&mut d, &mut h, &[id], &Plane::new(Vec3::X, 32.0), Keep::Both, "CLIP").unwrap();
        assert_eq!(ids.len(), 2);
        let (mid, exact) = cmd_merge(&mut d, &mut h, &ids).unwrap();
        assert!(exact);
        assert!((brush_volume(&d.brushes[&mid]) - 64f64.powi(3)).abs() < 1e-6);

        let eid = cmd_tie_to_entity(&mut d, &mut h, &[mid], "func_wall").unwrap();
        assert_eq!(d.brushes[&mid].entity, Some(eid));
        cmd_set_key(&mut d, &mut h, eid, "rendermode", "4").unwrap();
        assert!(cmd_set_key(&mut d, &mut h, eid, "classname", "").is_err());
        cmd_move_to_world(&mut d, &mut h, eid);
        assert!(d.entities.is_empty() && d.brushes[&mid].entity.is_none());
        h.undo(&mut d);
        assert_eq!(d.entities[&eid].get("rendermode"), Some("4"));

        let gid = cmd_group(&mut d, &mut h, "Crates", &[mid], &[]).unwrap();
        assert_eq!(d.brushes[&mid].group, Some(gid));
        cmd_ungroup(&mut d, &mut h, gid);
        assert!(d.groups.is_empty() && d.brushes[&mid].group.is_none());
        cmd_delete(&mut d, &mut h, &[], &[eid]);
        assert!(!d.brushes.contains_key(&mid));
        h.undo(&mut d);
        assert!(d.brushes.contains_key(&mid));
    }

    #[test]
    fn align_command() {
        let (mut d, mut h, id) = setup();
        cmd_align(&mut d, &mut h, &[(id, 0)], None, |a, poly, (w, hgt), _| texture::fit(a, poly, w, hgt, 1.0, 1.0)).unwrap();
        assert!((d.brushes[&id].faces[0].align.scale_u - 1.0).abs() < 1e-9);
        let mut w = d.world.clone();
        w.keys.push(("skyname".into(), "night".into()));
        cmd_set_world(&mut d, &mut h, w);
        h.undo(&mut d);
        assert!(d.world.get("skyname").is_none());
    }
}
