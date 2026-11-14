//! The map document: the one source of truth for the brush editor. The 3D view only draws it;
//! every change goes through `history::Change` so it can be undone.
//!
//! Saved as project JSON (`to_json`/`from_json`, versioned) and exported/imported as a GoldSrc
//! Valve 220 .map (`mapfile`).

use super::math::{Plane, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Bump when the JSON layout changes; `from_json` upgrades older versions.
pub const DOC_VERSION: u32 = 1;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}
id_type!(BrushId);
id_type!(EntityId);
id_type!(GroupId);

/// Valve 220 texture alignment: texture pixel coordinates are
/// `u = p . u_axis / scale_u + u_offset`, `v = p . v_axis / scale_v + v_offset`.
/// `rotation` is kept for the UI and the file (the axes already include it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TexAlign {
    pub u_axis: Vec3,
    pub u_offset: f64,
    pub v_axis: Vec3,
    pub v_offset: f64,
    pub rotation: f64,
    pub scale_u: f64,
    pub scale_v: f64,
}

impl TexAlign {
    /// Texture pixel coordinates of a point.
    pub fn uv(&self, p: Vec3) -> (f64, f64) {
        (
            p.dot(self.u_axis) / nonzero(self.scale_u) + self.u_offset,
            p.dot(self.v_axis) / nonzero(self.scale_v) + self.v_offset,
        )
    }
}

pub(crate) fn nonzero(s: f64) -> f64 {
    if s.abs() < 1e-9 {
        1.0
    } else {
        s
    }
}

/// One side of a brush: a plane given by three points (as in a .map file, clockwise seen from
/// outside) plus its texture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub points: [Vec3; 3],
    pub texture: String,
    pub align: TexAlign,
}

impl Face {
    /// The face's plane; None when its points are in a line (an invalid face).
    pub fn plane(&self) -> Option<Plane> {
        Plane::from_map_points(&self.points)
    }
}

/// A convex brush: the intersection of the back half-spaces of its faces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    pub faces: Vec<Face>,
    /// The brush entity that owns it (func_wall, trigger_...); None = part of the world.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
}

/// An entity's key-values, in file order. `classname` is one of them.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Entity {
    pub keys: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
}

impl Entity {
    pub fn new(classname: &str) -> Entity {
        Entity { keys: vec![("classname".into(), classname.into())], group: None }
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    pub fn classname(&self) -> &str {
        self.get("classname").unwrap_or("")
    }
    /// Set a key, keeping its place if it already exists.
    pub fn set(&mut self, key: &str, value: &str) {
        match self.keys.iter_mut().find(|(k, _)| k == key) {
            Some(kv) => kv.1 = value.to_string(),
            None => self.keys.push((key.to_string(), value.to_string())),
        }
    }
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let i = self.keys.iter().position(|(k, _)| k == key)?;
        Some(self.keys.remove(i).1)
    }
    /// `origin` as a vector, when present and well formed.
    pub fn origin(&self) -> Option<Vec3> {
        let v: Vec<f64> = self.get("origin")?.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        (v.len() == 3).then(|| Vec3 { x: v[0], y: v[1], z: v[2] })
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    /// P5: the prefab it was placed from (the prefab's permanent uid), when it was one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<GroupId>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    /// Parity A: a TrenchBroom layer (written back as one), not a group
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub layer: bool,
    /// Parity A: TrenchBroom's "omit from export": what's in it isn't exported or compiled
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub omit_export: bool,
}

/// What we know about a texture: its size (for fitting and alignment) and where it comes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wad: Option<String>,
}

/// worldspawn, minus `classname` / `mapversion` (always written) and `wad` (kept as a list).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct WorldSettings {
    /// WAD paths exactly as the `wad` key lists them (e.g. `\half-life\valve\halflife.wad`).
    pub wads: Vec<String>,
    /// Every other worldspawn key (skyname, message, light, sounds...), in order.
    pub keys: Vec<(String, String)>,
}

impl WorldSettings {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    /// The WAD file names without folders, lower-cased (what the game actually looks for).
    pub fn wad_names(&self) -> Vec<String> {
        self.wads
            .iter()
            .map(|w| w.rsplit(['\\', '/']).next().unwrap_or(w).to_ascii_lowercase())
            .filter(|w| !w.is_empty())
            .collect()
    }
    /// The `wad` key value (paths joined with `;`).
    pub fn wad_key(&self) -> String {
        let mut s = self.wads.join(";");
        if !s.is_empty() {
            s.push(';');
        }
        s
    }
    pub fn set_wad_key(&mut self, value: &str) {
        self.wads = value.split(';').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    }
}

/// A named set of things to show or hide together (the Outline's visibility sets).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct VisSet {
    pub name: String,
    #[serde(default)]
    pub brushes: Vec<BrushId>,
    #[serde(default)]
    pub entities: Vec<EntityId>,
}

/// A box to compile on its own (Hammer's cordon), in GoldSrc units.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct CordonBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// How the cordon is sealed and what it keeps.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct CordonSettings {
    #[serde(default)]
    pub boxes: Vec<CordonBox>,
    /// compile only the boxes
    #[serde(default)]
    pub on: bool,
    /// "sky" (default) or "black"
    #[serde(default)]
    pub seal: String,
    /// lights outside the boxes are compiled too
    #[serde(default)]
    pub keep_lights: bool,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct EditorSettings {
    /// a mapper's own FGD file, read on top of ours
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fgd: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vis_sets: Vec<VisSet>,
    #[serde(default, skip_serializing_if = "CordonSettings::is_empty")]
    pub cordon: CordonSettings,
    /// P5: the project's permanent id (a UUID v4): autosave/recovery, backups and the recent
    /// list follow the project by it when it's renamed or moved
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    /// P5: the community map this project was remixed from (Remix this map), credited when the
    /// remix is published
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remix_of: Option<RemixCredit>,
}

/// P5: where a remixed map came from: the community creation's GameBanana page, its name and
/// who made it ("A remix of X by Y").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct RemixCredit {
    /// "Mod" (a GameBanana mod page)
    pub model: String,
    pub gb: i64,
    /// the original's permanent id (its project uid), "" when it had none
    pub uid: String,
    pub name: String,
    pub author: String,
}

/// P5: a new random UUID (version 4), as text.
pub fn new_uid() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// P5: a UUID v4 in lower case (the only form uids are kept in).
pub fn valid_uid(u: &str) -> bool {
    let b = u.as_bytes();
    u.len() == 36
        && u.char_indices().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == '-' } else { c.is_ascii_digit() || ('a'..='f').contains(&c) })
        && b[14] == b'4'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// P5: a fixed uid made from a name (the starter prefabs: the same on every computer).
pub fn stable_uid(seed: &str) -> String {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325;
    let mut c: u64 = 0x8422_2325_cbf2_9ce4;
    for x in seed.bytes() {
        a = (a ^ x as u64).wrapping_mul(0x100_0000_01b3);
        c = (c ^ x as u64).wrapping_mul(0x100_0000_01b3).rotate_left(17);
    }
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&a.to_be_bytes());
    b[8..].copy_from_slice(&c.to_be_bytes());
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

impl CordonSettings {
    pub fn is_empty(&self) -> bool {
        self == &CordonSettings::default()
    }
}

impl EditorSettings {
    pub fn is_empty(&self) -> bool {
        self == &EditorSettings::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapDocument {
    pub version: u32,
    pub world: WorldSettings,
    pub brushes: BTreeMap<BrushId, Brush>,
    pub entities: BTreeMap<EntityId, Entity>,
    #[serde(default)]
    pub groups: BTreeMap<GroupId, Group>,
    /// Known textures by lower-case name. Filled from the WADs by the caller; not saved in .map.
    #[serde(default)]
    pub materials: BTreeMap<String, Material>,
    next_id: u64,
    /// The editor's own settings kept with the project (not in the .map): visibility sets,
    /// cordon boxes, a custom FGD. Not part of undo.
    #[serde(default, skip_serializing_if = "EditorSettings::is_empty")]
    pub editor: EditorSettings,
    /// Bumped on every change, so the view can tell when to redraw. Not saved.
    #[serde(skip)]
    pub revision: u64,
    /// P5 (perf): the brushes and entities changed since the editor last looked (history notes
    /// them as it applies edits), so it needn't compare or copy the whole map after each
    /// command. Not saved, never part of equality.
    #[serde(skip)]
    pub touched: Touched,
}

/// What changed since the last `take_touched` (P5, perf).
#[derive(Clone, Debug, Default)]
pub struct Touched {
    pub brushes: BTreeSet<BrushId>,
    pub entities: BTreeSet<EntityId>,
}

impl PartialEq for Touched {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Default for MapDocument {
    fn default() -> Self {
        MapDocument::new()
    }
}

impl MapDocument {
    pub fn new() -> MapDocument {
        MapDocument {
            version: DOC_VERSION,
            world: WorldSettings::default(),
            brushes: BTreeMap::new(),
            entities: BTreeMap::new(),
            groups: BTreeMap::new(),
            materials: BTreeMap::new(),
            next_id: 1,
            editor: EditorSettings::default(),
            revision: 0,
            touched: Touched::default(),
        }
    }

    /// P5 (perf): a brush changed (and the entities it left or joined: their brush lists did too).
    pub fn touch_brush(&mut self, id: BrushId, owners: [Option<EntityId>; 2]) {
        self.touched.brushes.insert(id);
        self.touched.entities.extend(owners.into_iter().flatten());
    }
    pub fn touch_entity(&mut self, id: EntityId) {
        self.touched.entities.insert(id);
    }
    pub fn take_touched(&mut self) -> Touched {
        std::mem::take(&mut self.touched)
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn new_brush_id(&mut self) -> BrushId {
        BrushId(self.take_id())
    }
    pub fn new_entity_id(&mut self) -> EntityId {
        EntityId(self.take_id())
    }
    pub fn new_group_id(&mut self) -> GroupId {
        GroupId(self.take_id())
    }

    /// Add a brush directly (no undo). Editing code should go through `history`.
    pub fn insert_brush(&mut self, b: Brush) -> BrushId {
        let id = self.new_brush_id();
        self.brushes.insert(id, b);
        id
    }
    pub fn insert_entity(&mut self, e: Entity) -> EntityId {
        let id = self.new_entity_id();
        self.entities.insert(id, e);
        id
    }
    pub fn insert_group(&mut self, g: Group) -> GroupId {
        let id = self.new_group_id();
        self.groups.insert(id, g);
        id
    }

    /// Brushes that belong to an entity (None = world brushes), in id order.
    pub fn brushes_of(&self, entity: Option<EntityId>) -> impl Iterator<Item = (BrushId, &Brush)> {
        self.brushes.iter().filter(move |(_, b)| b.entity == entity).map(|(id, b)| (*id, b))
    }

    pub fn material(&self, name: &str) -> Option<&Material> {
        self.materials.get(&name.to_ascii_lowercase())
    }
    pub fn add_material(&mut self, m: Material) {
        self.materials.insert(m.name.to_ascii_lowercase(), m);
    }
    /// Texture size for alignment helpers; 64x64 when unknown (the size of most GoldSrc tiles).
    pub fn texture_size(&self, name: &str) -> (f64, f64) {
        self.material(name).map(|m| (m.width.max(1) as f64, m.height.max(1) as f64)).unwrap_or((64.0, 64.0))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<MapDocument, String> {
        let raw: serde_json::Value = serde_json::from_str(s).map_err(|e| format!("This project file isn't valid JSON: {e}"))?;
        let version = raw.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version == 0 {
            return Err("This doesn't look like a Loner-Strike map project.".into());
        }
        if version > DOC_VERSION as u64 {
            return Err(format!(
                "This project was saved by a newer Loner-Strike (format {version}); update the app to open it."
            ));
        }
        // Older versions would be upgraded here, one step at a time.
        let mut doc: MapDocument =
            serde_json::from_value(raw).map_err(|e| format!("This project file is damaged: {e}"))?;
        doc.version = DOC_VERSION;
        // Never hand out an id that's already taken, even if the file was edited by hand.
        let max_id = doc
            .brushes
            .keys()
            .map(|i| i.0)
            .chain(doc.entities.keys().map(|i| i.0))
            .chain(doc.groups.keys().map(|i| i.0))
            .max()
            .unwrap_or(0);
        doc.next_id = doc.next_id.max(max_id + 1);
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_versions() {
        let mut d = MapDocument::new();
        d.world.set_wad_key("\\half-life\\valve\\halflife.wad; c:/x/cstrike.wad;");
        d.world.keys.push(("skyname".into(), "desert".into()));
        let g = d.insert_group(Group { name: "Stairs".into(), ..Default::default() });
        let mut e = Entity::new("info_player_start");
        e.set("origin", "0 0 36");
        e.group = Some(g);
        d.insert_entity(e);
        let s = d.to_json();
        let back = MapDocument::from_json(&s).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.world.wad_names(), vec!["halflife.wad", "cstrike.wad"]);
        assert_eq!(back.world.wad_key(), "\\half-life\\valve\\halflife.wad;c:/x/cstrike.wad;");
        assert!(MapDocument::from_json("{}").is_err());
        assert!(MapDocument::from_json(&s.replace("\"version\": 1", "\"version\": 99")).unwrap_err().contains("newer"));
        assert!(MapDocument::from_json("not json").is_err());
    }

    #[test]
    fn ids_never_repeat_after_load() {
        let mut d = MapDocument::new();
        d.insert_entity(Entity::new("light"));
        let s = d.to_json().replace("\"next_id\": 2", "\"next_id\": 1");
        let mut back = MapDocument::from_json(&s).unwrap();
        assert_eq!(back.new_entity_id(), EntityId(2));
    }

    #[test]
    fn entity_keys() {
        let mut e = Entity::new("light");
        e.set("origin", "1 2 3");
        e.set("classname", "light_spot");
        assert_eq!(e.keys[0], ("classname".into(), "light_spot".into()));
        assert_eq!(e.origin(), Some(Vec3 { x: 1.0, y: 2.0, z: 3.0 }));
        assert_eq!(e.remove("origin").as_deref(), Some("1 2 3"));
        assert_eq!(e.origin(), None);
    }
}
