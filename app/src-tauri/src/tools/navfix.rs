//! Nav fixer (Beta): the bot nav fixes we used to do by hand (navpack/navarea.py and
//! navlink.py on cs_rats2), as edits the Tools page sends one at a time.
//!
//! - `GET  /api/tools/navfix/:name`          open the map's nav for editing (or the open one)
//! - `POST /api/tools/navfix/:name/edit`     one edit: add, delete, reshape, link, unlink,
//!   attr, place (body `{op, ...}`, see [`Op`])
//! - `POST /api/tools/navfix/:name/undo` / `redo`
//! - `POST /api/tools/navfix/:name/save`     `<nav>.part` then rename, `<nav>.bak` the first
//!   time; refused while the game has the map loaded or the file changed on disk
//! - `POST /api/tools/navfix/:name/reload`   drop the edits and read the file again
//! - `POST /api/tools/navfix/:name/rebuild`  rebuild the bots' map knowledge (.lsv) for the
//!   saved nav on a hidden server (a setup job, shown with the app's job progress)
//!
//! Every answer is the whole editing state: the nav as the viewer's `/nav` route sends it,
//! which areas bots can reach from the spawns, the problems found, and undo/redo labels.
//!
//! The edits are pure functions over [`Doc`] (areas behind `Arc`, so an undo step costs a
//! list of pointers, not a copy of the nav); the routes keep one editing session per nav.

use super::mapinfo::MapFile;
use super::{blocking, cache, find_map, json_answer, locate, Done, Fail};
use crate::goldsrc::{self, bsp, hull::MapHulls, lsv, nav};
use crate::goldsrc::nav::{Area, Nav};
use crate::server::Shared;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const NO_NAV: &str = "This map has no bot nav file yet. Play it with bots once and the game makes one, then fix it here.";
const NOT_PANEL: &str = "That has to come from the Loner-Strike window.";
const LOADED: &str = "The game has this map loaded right now. Change to another map (or close the game) and save again: the game keeps the nav it loaded and would carry on with the old one.";
const CHANGED: &str = "The nav file changed on disk since you opened it (the game may have learned it again). Reload to see the new one; your edits here would be lost.";

/// Most undo steps kept per map.
const MAX_UNDO: usize = 300;
/// The most areas a nav may grow to here (the brain's knowledge stops at 10,000: lsv::MAX_AREAS).
const MAX_AREAS: usize = 20_000;
/// Coordinates stay inside the GoldSrc world.
const WORLD: f32 = 32768.0;
/// Problems listed at most (the rest are counted).
const MAX_PROBLEMS: usize = 200;

// ---- the document ---------------------------------------------------------------------------

static GEN: AtomicU64 = AtomicU64::new(1);

/// The nav being edited. `gen` names this exact version: undo back to the saved version and
/// it's clean again.
#[derive(Debug, Clone)]
pub struct Doc {
    pub bsp_size: u32,
    pub places: Vec<String>,
    pub areas: Vec<Arc<Area>>,
    pub gen: u64,
}

impl Doc {
    pub fn from_nav(n: Nav) -> Doc {
        Doc { bsp_size: n.bsp_size, places: n.places, areas: n.areas.into_iter().map(Arc::new).collect(), gen: GEN.fetch_add(1, Ordering::Relaxed) }
    }

    pub fn to_nav(&self) -> Nav {
        Nav { version: nav::NAV_VERSION, bsp_size: self.bsp_size, places: self.places.clone(), areas: self.areas.iter().map(|a| (**a).clone()).collect() }
    }

    fn pos(&self, id: u32) -> Result<usize, String> {
        self.areas.iter().position(|a| a.id == id).ok_or_else(|| format!("There's no area {id} in this nav (it may have been deleted)."))
    }

    /// A copy with a new version stamp, for an edit to change.
    fn next(&self) -> Doc {
        Doc { gen: GEN.fetch_add(1, Ordering::Relaxed), ..self.clone() }
    }
}

/// The 2D box of an area, whichever way round its corners were saved.
fn rect(a: &Area) -> ([f32; 2], [f32; 2]) {
    ([a.lo[0].min(a.hi[0]), a.lo[1].min(a.hi[1])], [a.lo[0].max(a.hi[0]), a.lo[1].max(a.hi[1])])
}

/// Which side of `a` the area `b` is on (0 N -y, 1 E +x, 2 S +y, 3 W -x): the axis they're
/// further apart on. The same rule as navarea.py / navlink.py, so their fixes come out the same.
pub fn direction(a: &Area, b: &Area) -> usize {
    let (alo, ahi) = rect(a);
    let (blo, bhi) = rect(b);
    let dx = (blo[0] - ahi[0]).max(alo[0] - bhi[0]);
    let dy = (blo[1] - ahi[1]).max(alo[1] - bhi[1]);
    if dy >= dx {
        if blo[1] >= ahi[1] {
            2
        } else {
            0
        }
    } else if blo[0] >= ahi[0] {
        1
    } else {
        3
    }
}

fn links_to(a: &Area, id: u32) -> bool {
    a.conns.iter().any(|d| d.contains(&id))
}

// ---- the floor under a point --------------------------------------------------------------

/// What the edits ask of the map: the floor near a height, and whether a standing player fits.
pub trait Ground {
    /// The floor height under (x, y), looking from a little above `z` to a little below it.
    fn floor(&self, x: f32, y: f32, z: f32) -> Option<f32>;
    /// Whether a standing player fits with their feet at `p`.
    fn fits(&self, p: [f32; 3]) -> bool;
    /// What happens to a player walking from `from` (feet, just inside an area's edge) out
    /// to `to`: where they land, falling no further than [`MAX_FALL`].
    fn step_off(&self, _from: [f32; 3], _to: [f32; 2]) -> Step {
        Step::Wall
    }
}

/// See [`Ground::step_off`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    /// A wall or railing is in the way, or a player doesn't fit there.
    Wall,
    /// Nothing to land on (a pit deeper than a bot survives, or the void).
    Fall,
    /// Lands with the feet at this height.
    Land(f32),
}

/// The furthest a bot falls and lives at full health: the game's fall damage is
/// (speed - 500) * 100 / 600 * 1.25, so 100 damage at 980 units/s, a 600-unit drop at sv_gravity 800.
pub const MAX_FALL: f32 = 590.0;

/// How far above and below the drawn height the floor is looked for.
const SNAP_UP: f32 = 18.0;
const SNAP_DOWN: f32 = 64.0;

impl Ground for MapHulls {
    fn floor(&self, x: f32, y: f32, z: f32) -> Option<f32> {
        // (a point, not the player's box: a 10-unit beam's top is found exactly)
        let t = self.trace(0, [x, y, z + SNAP_UP], [x, y, z - SNAP_DOWN]);
        (!t.start_solid && t.fraction < 1.0 && t.normal[2] >= 0.7).then_some(t.end[2])
    }
    fn fits(&self, p: [f32; 3]) -> bool {
        // head room over the middle, with a point trace (a nav area over stairs sits under the
        // steps' edges, so starting inside one counts as a step; somewhere solid at every
        // height tried isn't something to report: the area may just be drawn a little low).
        // Not the player's box: an area's edge can run up to a wall or through a vent grate
        // (cs_rats2 areas 65, 146, 273), so the box at its middle can rightly be stuck while
        // players stand in most of the area.
        for up in [1.0, 10.0, SNAP_UP] {
            let from = [p[0], p[1], p[2] + up];
            let t = self.trace(0, from, [p[0], p[1], p[2] + up + 128.0]);
            if !t.start_solid {
                return up + t.fraction * 128.0 >= 70.0;
            }
        }
        true
    }
    fn step_off(&self, from: [f32; 3], to: [f32; 2]) -> Step {
        // nothing at shin or head height between the edge and the spot past it (point traces:
        // an area can run right up to a wall, so the player's box at the edge may be stuck)
        for up in [20.0, 56.0] {
            let t = self.trace(0, [from[0], from[1], from[2] + up], [to[0], to[1], from[2] + up]);
            if t.fraction < 1.0 || t.all_solid {
                return Step::Wall;
            }
        }
        // then the standing player's box (hull 1, origin 36 above the feet) falls from a step
        // up, so a step up onto the next area counts as walking on. Close to a wall the box
        // doesn't fit at that exact spot (the player would be a few units over): a point then.
        let top = from[2] + 36.0 + SNAP_UP;
        let mut t = self.trace(1, [to[0], to[1], top], [to[0], to[1], from[2] + 36.0 - MAX_FALL]);
        if t.start_solid {
            t = self.trace(0, [to[0], to[1], from[2] + SNAP_UP], [to[0], to[1], from[2] - MAX_FALL]);
            t.end[2] += 36.0;
        }
        if t.start_solid {
            Step::Wall
        } else if t.fraction < 1.0 && t.normal[2] >= 0.7 {
            Step::Land(t.end[2] - 36.0)
        } else {
            // (the void, a pit too deep, or a slope too steep to stand on: they slide off it)
            Step::Fall
        }
    }
}

/// The four corner heights (nw, ne, se, sw) of a flat box drawn at `z`, each dropped to the
/// floor. The middle is found first; a corner whose floor is more than a step away from the
/// middle's (it fell off the edge of a beam) takes the middle's height. None: no floor there.
fn snap_corners(g: &dyn Ground, lo: [f32; 2], hi: [f32; 2], z: f32) -> Option<[f32; 4]> {
    let mid = g.floor((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, z)?;
    // (a unit in from each edge: the edge itself is where the floor's brush ends)
    let ix = ((hi[0] - lo[0]) / 2.0).min(1.0);
    let iy = ((hi[1] - lo[1]) / 2.0).min(1.0);
    let at = |x: f32, y: f32| g.floor(x, y, mid).filter(|h| (h - mid).abs() <= SNAP_UP).unwrap_or(mid);
    Some([at(lo[0] + ix, lo[1] + iy), at(hi[0] - ix, lo[1] + iy), at(hi[0] - ix, hi[1] - iy), at(lo[0] + ix, hi[1] - iy)])
}

// ---- edits ----------------------------------------------------------------------------------

fn yes() -> bool {
    true
}

/// One edit, as the page sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    /// A new flat area over the box x × y at height z (dropped to the floor with `snap`).
    /// `place`: a place name ("" none); left out, it takes the nearest named area's place.
    Add {
        x: [f32; 2],
        y: [f32; 2],
        z: f32,
        #[serde(default)]
        attr: u8,
        #[serde(default)]
        place: Option<String>,
        #[serde(default = "yes")]
        snap: bool,
    },
    /// Areas and every link to them.
    Delete { ids: Vec<u32> },
    /// An area moved or resized to the box x × y (at height z, or where it was).
    Reshape {
        id: u32,
        x: [f32; 2],
        y: [f32; 2],
        #[serde(default)]
        z: Option<f32>,
        #[serde(default = "yes")]
        snap: bool,
    },
    /// a → b, and b → a unless `oneWay` (then b → a goes: a drop you can't climb back up).
    Link {
        a: u32,
        b: u32,
        #[serde(default, rename = "oneWay")]
        one_way: bool,
    },
    /// a → b goes, and b → a with `both`.
    Unlink {
        a: u32,
        b: u32,
        #[serde(default = "yes")]
        both: bool,
    },
    /// Attribute bits (crouch 1, jump 2, precise 4, no jump 8) set and cleared on areas.
    Attr {
        ids: Vec<u32>,
        #[serde(default)]
        set: u8,
        #[serde(default)]
        clear: u8,
    },
    /// The place name of areas ("" for none); a new name joins the nav's place list.
    Place { ids: Vec<u32>, name: String },
    /// Several edits as one step (one Undo takes them all back): "Fix 2" on the problems list.
    /// `label` names the step ("Fix 2 problems").
    Batch {
        ops: Vec<Op>,
        #[serde(default)]
        label: Option<String>,
    },
}

/// The most edits one batch takes.
const MAX_BATCH: usize = 500;

/// What an edit did.
#[derive(Debug)]
pub struct Edited {
    pub doc: Doc,
    /// For the undo button: "Add area 1144".
    pub label: String,
    /// Things worth saying ("Couldn't find the floor under it: kept the height you drew at").
    pub notes: Vec<String>,
    /// The area an add made.
    pub new_id: Option<u32>,
}

const ATTR_MASK: u8 = nav::ATTR_CROUCH | nav::ATTR_JUMP | nav::ATTR_PRECISE | nav::ATTR_NO_JUMP;

fn check_box(x: [f32; 2], y: [f32; 2], z: Option<f32>) -> Result<([f32; 2], [f32; 2]), String> {
    let all = [x[0], x[1], y[0], y[1], z.unwrap_or(0.0)];
    if all.iter().any(|v| !v.is_finite() || v.abs() > WORLD) {
        return Err("That's outside the map.".into());
    }
    let lo = [x[0].min(x[1]), y[0].min(y[1])];
    let hi = [x[0].max(x[1]), y[0].max(y[1])];
    if hi[0] - lo[0] < 1.0 || hi[1] - lo[1] < 1.0 {
        return Err("That area is too small: draw it at least a unit wide each way.".into());
    }
    if hi[0] - lo[0] > 16384.0 || hi[1] - lo[1] > 16384.0 {
        return Err("That area is bigger than any map.".into());
    }
    Ok((lo, hi))
}

/// A place name the game can save: letters, digits and _-. up to 63 characters.
fn place_ok(name: &str) -> bool {
    name.len() <= 63 && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
}

/// The place of the nearest area with one, within 384 units (navarea.py's `place_from`).
fn nearby_place(doc: &Doc, at: [f32; 3]) -> u16 {
    doc.areas
        .iter()
        .filter(|a| a.place != 0)
        .map(|a| {
            let c = a.closest_point(at);
            ((c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2) + (c[2] - at[2]).powi(2), a.place)
        })
        .filter(|(d, _)| *d <= 384.0 * 384.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
        .unwrap_or(0)
}

/// Lays a box on the floor: corner heights from the ground (or flat at `z`).
fn lay(a: &mut Area, lo: [f32; 2], hi: [f32; 2], z: f32, snap: bool, ground: Option<&dyn Ground>, notes: &mut Vec<String>) {
    let h = match (snap, ground) {
        (true, Some(g)) => snap_corners(g, lo, hi, z).unwrap_or_else(|| {
            notes.push("Couldn't find the floor under it, so it's flat at the height you drew it.".into());
            [z; 4]
        }),
        (true, None) => {
            notes.push("The map couldn't be read, so it's flat at the height you drew it.".into());
            [z; 4]
        }
        _ => [z; 4],
    };
    a.lo = [lo[0], lo[1], h[0]];
    a.ne_z = h[1];
    a.hi = [hi[0], hi[1], h[2]];
    a.sw_z = h[3];
    if let Some(g) = ground {
        let c = a.center();
        if snap && a.attr & nav::ATTR_CROUCH == 0 && !g.fits([c[0], c[1], a.z_at(c[0], c[1])]) {
            notes.push("A standing player doesn't fit there. Mark it crouch if it's a vent or a low gap.".into());
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Applies one edit. The document isn't changed: the result is a new version (or why not).
pub fn apply(doc: &Doc, op: &Op, ground: Option<&dyn Ground>) -> Result<Edited, String> {
    // several edits, one step: each on the last one's result, all or none
    if let Op::Batch { ops, label } = op {
        if ops.is_empty() || ops.len() > MAX_BATCH || ops.iter().any(|o| matches!(o, Op::Batch { .. })) {
            return Err("That edit isn't one the nav fixer knows.".into());
        }
        let mut cur: Option<Edited> = None;
        let mut notes = Vec::new();
        for o in ops {
            let e = apply(cur.as_ref().map(|e| &e.doc).unwrap_or(doc), o, ground)?;
            notes.extend(e.notes.iter().cloned());
            cur = Some(e);
        }
        let last = cur.expect("at least one edit");
        let label: String = label.as_deref().map(|l| l.chars().filter(|c| !c.is_control()).take(60).collect::<String>()).filter(|l| !l.trim().is_empty()).unwrap_or_else(|| format!("{} edits", ops.len()));
        notes.dedup();
        return Ok(Edited { doc: last.doc, label, notes, new_id: last.new_id });
    }
    let mut d = doc.next();
    let mut notes = Vec::new();
    let mut new_id = None;
    let label = match op {
        Op::Add { x, y, z, attr, place, snap } => {
            if d.areas.len() >= MAX_AREAS {
                return Err(format!("This nav has {MAX_AREAS} areas, the most the fixer makes."));
            }
            let (lo, hi) = check_box(*x, *y, Some(*z))?;
            let id = d.areas.iter().map(|a| a.id).max().unwrap_or(0).checked_add(1).ok_or("This nav's area numbers have run out.")?;
            let mut a = Area { id, attr: attr & ATTR_MASK, lo: [0.0; 3], hi: [0.0; 3], ne_z: 0.0, sw_z: 0.0, conns: Default::default(), hiding: vec![], approach: vec![], encounters: vec![], place: 0 };
            lay(&mut a, lo, hi, *z, *snap, ground, &mut notes);
            a.place = match place {
                Some(name) => place_index(&mut d, name)?,
                None => nearby_place(doc, a.center()),
            };
            d.areas.push(Arc::new(a));
            new_id = Some(id);
            format!("Add area {id}")
        }
        Op::Delete { ids } => {
            let gone: HashSet<u32> = ids.iter().copied().collect();
            if gone.is_empty() {
                return Err("Pick the areas to delete first.".into());
            }
            for id in &gone {
                d.pos(*id)?;
            }
            // (their hiding spots go with them: encounter data that names one goes too)
            let spots: HashSet<u32> = d.areas.iter().filter(|a| gone.contains(&a.id)).flat_map(|a| a.hiding.iter().map(|h| h.id)).collect();
            d.areas.retain(|a| !gone.contains(&a.id));
            for a in d.areas.iter_mut() {
                let touches = a.conns.iter().flatten().any(|i| gone.contains(i))
                    || a.approach.iter().any(|p| gone.contains(&p.prev) || gone.contains(&p.next))
                    || a.encounters.iter().any(|e| gone.contains(&e.from) || gone.contains(&e.to) || e.spots.iter().any(|s| spots.contains(&s.id)));
                if !touches {
                    continue;
                }
                let m = Arc::make_mut(a);
                for c in m.conns.iter_mut() {
                    c.retain(|i| !gone.contains(i));
                }
                m.approach.retain(|p| !gone.contains(&p.prev) && !gone.contains(&p.next));
                m.encounters.retain(|e| !gone.contains(&e.from) && !gone.contains(&e.to));
                for e in m.encounters.iter_mut() {
                    e.spots.retain(|s| !spots.contains(&s.id));
                }
            }
            let mut list: Vec<u32> = gone.into_iter().collect();
            list.sort_unstable();
            if list.len() == 1 {
                format!("Delete area {}", list[0])
            } else {
                format!("Delete {}", plural(list.len(), "area", "areas"))
            }
        }
        Op::Reshape { id, x, y, z, snap } => {
            let i = d.pos(*id)?;
            let (lo, hi) = check_box(*x, *y, *z)?;
            let a = Arc::make_mut(&mut d.areas[i]);
            let (olo, ohi) = rect(a);
            let oz = a.center()[2];
            let nz = z.unwrap_or(a.lo[2].min(a.hi[2]).min(a.ne_z).min(a.sw_z));
            lay(a, lo, hi, nz, *snap, ground, &mut notes);
            let same_size = ((hi[0] - lo[0]) - (ohi[0] - olo[0])).abs() < 0.01 && ((hi[1] - lo[1]) - (ohi[1] - olo[1])).abs() < 0.01;
            if same_size {
                // moved: its hiding spots move with it
                let dz = a.center()[2] - oz;
                for h in a.hiding.iter_mut() {
                    h.pos = [h.pos[0] + lo[0] - olo[0], h.pos[1] + lo[1] - olo[1], h.pos[2] + dz];
                }
                format!("Move area {id}")
            } else {
                let before = a.hiding.len();
                a.hiding.retain(|h| h.pos[0] >= lo[0] && h.pos[0] <= hi[0] && h.pos[1] >= lo[1] && h.pos[1] <= hi[1]);
                if a.hiding.len() < before {
                    notes.push(format!("{} outside the new size went.", plural(before - a.hiding.len(), "hiding spot", "hiding spots")));
                }
                format!("Resize area {id}")
            }
        }
        Op::Link { a, b, one_way } => {
            if a == b {
                return Err("Pick two different areas to link.".into());
            }
            let (ia, ib) = (d.pos(*a)?, d.pos(*b)?);
            let had = (links_to(&d.areas[ia], *b), links_to(&d.areas[ib], *a));
            if had.0 && (had.1 != *one_way) {
                return Err(if *one_way { format!("Area {a} already drops to {b}.") } else { format!("Areas {a} and {b} are already linked both ways.") });
            }
            let dir = direction(&d.areas[ia], &d.areas[ib]);
            if !had.0 {
                Arc::make_mut(&mut d.areas[ia]).conns[dir].push(*b);
            }
            if *one_way {
                for c in Arc::make_mut(&mut d.areas[ib]).conns.iter_mut() {
                    c.retain(|i| i != a);
                }
                format!("One-way drop from {a} to {b}")
            } else {
                if !had.1 {
                    let back = direction(&d.areas[ib], &d.areas[ia]);
                    Arc::make_mut(&mut d.areas[ib]).conns[back].push(*a);
                }
                format!("Link {a} and {b}")
            }
        }
        Op::Unlink { a, b, both } => {
            // (b may be missing: that's the broken link being removed)
            let (ia, ib) = (d.pos(*a)?, d.pos(*b).ok());
            let back = *both && a != b && ib.is_some_and(|ib| links_to(&d.areas[ib], *a));
            if !links_to(&d.areas[ia], *b) && !back {
                return Err(format!("Areas {a} and {b} aren't linked."));
            }
            for c in Arc::make_mut(&mut d.areas[ia]).conns.iter_mut() {
                c.retain(|i| i != b);
            }
            if let (true, Some(ib)) = (*both && a != b, ib) {
                for c in Arc::make_mut(&mut d.areas[ib]).conns.iter_mut() {
                    c.retain(|i| i != a);
                }
                format!("Unlink {a} and {b}")
            } else {
                format!("Unlink {a} → {b}")
            }
        }
        Op::Attr { ids, set, clear } => {
            if ids.is_empty() {
                return Err("Pick the areas first.".into());
            }
            for id in ids {
                let i = d.pos(*id)?;
                let a = Arc::make_mut(&mut d.areas[i]);
                a.attr = (a.attr & !(clear & ATTR_MASK)) | (set & ATTR_MASK);
            }
            let what = [(nav::ATTR_CROUCH, "crouch"), (nav::ATTR_JUMP, "jump"), (nav::ATTR_PRECISE, "precise"), (nav::ATTR_NO_JUMP, "no jump")];
            let names = |bits: u8| what.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| *n).collect::<Vec<_>>().join(", ");
            let whose = if ids.len() == 1 { format!("area {}", ids[0]) } else { plural(ids.len(), "area", "areas") };
            match (set & ATTR_MASK, clear & ATTR_MASK) {
                (s, 0) if s != 0 => format!("Mark {whose} {}", names(s)),
                (0, c) if c != 0 => format!("Unmark {} on {whose}", names(c)),
                _ => format!("Change how bots move on {whose}"),
            }
        }
        // (handled above)
        Op::Batch { .. } => return Err("That edit isn't one the nav fixer knows.".into()),
        Op::Place { ids, name } => {
            if ids.is_empty() {
                return Err("Pick the areas first.".into());
            }
            let p = place_index(&mut d, name)?;
            for id in ids {
                let i = d.pos(*id)?;
                Arc::make_mut(&mut d.areas[i]).place = p;
            }
            let whose = if ids.len() == 1 { format!("area {}", ids[0]) } else { plural(ids.len(), "area", "areas") };
            if name.trim().is_empty() {
                format!("Clear the place of {whose}")
            } else {
                format!("Name {whose} {}", name.trim())
            }
        }
    };
    Ok(Edited { doc: d, label, notes, new_id })
}

/// A place name's number (1-based), added to the list if it's new; 0 for "".
fn place_index(d: &mut Doc, name: &str) -> Result<u16, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(0);
    }
    if !place_ok(name) {
        return Err("Place names can use letters, numbers, _ - and . (no spaces), up to 63 characters. The game shows them on the radar, like BombsiteA or Long.".into());
    }
    if let Some(i) = d.places.iter().position(|p| p == name) {
        return Ok(i as u16 + 1);
    }
    if d.places.len() >= u16::MAX as usize - 1 {
        return Err("This nav has too many place names.".into());
    }
    d.places.push(name.to_string());
    Ok(d.places.len() as u16)
}

// ---- checks ---------------------------------------------------------------------------------

/// Per area: whether bots get there from a spawn and back.
pub const REACH_OK: u8 = 0;
pub const REACH_NEVER: u8 = 1;
pub const REACH_TRAP: u8 = 2;
/// Bots only get there by falling or stumbling in (off an edge with no link), and can walk
/// back out: they never go there on purpose.
pub const REACH_FALLS: u8 = 3;

#[derive(Debug, Clone)]
pub struct Problem {
    pub kind: &'static str,
    pub text: String,
    pub hint: String,
    pub areas: Vec<u32>,
    pub at: [f32; 3],
    /// A one-click fix: (button label, the edit).
    pub fix: Option<(String, Value)>,
}

impl Problem {
    fn json(&self) -> Value {
        json!({"kind": self.kind, "text": self.text, "hint": self.hint, "areas": self.areas, "at": self.at,
               "fix": self.fix.as_ref().map(|(label, op)| json!({"label": label, "op": op}))})
    }
}

#[derive(Debug, Clone)]
pub struct Checked {
    pub reach: Vec<u8>,
    pub problems: Vec<Problem>,
    /// Problems past MAX_PROBLEMS.
    pub more: usize,
    /// Areas bots start in (indices).
    pub spawn_areas: Vec<usize>,
    pub note: Option<String>,
}

/// The area a spawn point (a player's origin, 36 above the feet) stands in, or the nearest.
fn spawn_area(n: &Nav, p: [f32; 3]) -> Option<usize> {
    if let Some(i) = n.area_at(p, 200.0) {
        return Some(i);
    }
    let feet = [p[0], p[1], p[2] - 36.0];
    n.areas
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let c = a.closest_point(feet);
            (i, (c[0] - feet[0]).powi(2) + (c[1] - feet[1]).powi(2) + (c[2] - feet[2]).powi(2))
        })
        .filter(|(_, d)| *d <= 256.0 * 256.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn mid(n: &Nav, list: &[usize]) -> [f32; 3] {
    let mut s = [0.0f32; 3];
    for &i in list {
        let c = n.areas[i].center();
        for k in 0..3 {
            s[k] += c[k];
        }
    }
    let k = list.len().max(1) as f32;
    [s[0] / k, s[1] / k, s[2] / k]
}

/// Groups of areas that touch each other through links (either way), biggest first.
fn clusters(n: &Nav, list: &[usize], index: &HashMap<u32, usize>) -> Vec<Vec<usize>> {
    let want: HashSet<usize> = list.iter().copied().collect();
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for &i in list {
        for id in n.areas[i].conns.iter().flatten() {
            if let Some(&j) = index.get(id) {
                if want.contains(&j) {
                    adj.entry(i).or_default().push(j);
                    adj.entry(j).or_default().push(i);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &s in list {
        if !seen.insert(s) {
            continue;
        }
        let mut group = vec![s];
        let mut k = 0;
        while k < group.len() {
            for &j in adj.get(&group[k]).map(|v| v.as_slice()).unwrap_or(&[]) {
                if seen.insert(j) {
                    group.push(j);
                }
            }
            k += 1;
        }
        group.sort_unstable();
        out.push(group);
    }
    out.sort_by(|a, b| b.len().cmp(&a.len()).then(a[0].cmp(&b[0])));
    out
}

fn bfs(starts: &[usize], adj: &[Vec<usize>]) -> Vec<bool> {
    let mut seen = vec![false; adj.len()];
    let mut q: VecDeque<usize> = VecDeque::new();
    for &s in starts {
        if !seen[s] {
            seen[s] = true;
            q.push_back(s);
        }
    }
    while let Some(i) = q.pop_front() {
        for &j in &adj[i] {
            if !seen[j] {
                seen[j] = true;
                q.push_back(j);
            }
        }
    }
    seen
}

fn ids(n: &Nav, list: &[usize]) -> Vec<u32> {
    list.iter().map(|&i| n.areas[i].id).collect()
}

/// 1.0.1: where a group of areas is, so two problems of the same kind read differently:
/// "areas 12, 48 and 3 more near Long A". The place is the group's own (the commonest), else
/// the nearest named area's within 384 units.
fn where_is(n: &Nav, list: &[usize]) -> String {
    let mut text = if list.len() == 1 {
        format!("area {}", n.areas[list[0]].id)
    } else {
        let first: Vec<String> = list.iter().take(3).map(|&i| n.areas[i].id.to_string()).collect();
        let rest = list.len().saturating_sub(3);
        if rest == 0 {
            match first.split_last() {
                Some((last, head)) => format!("areas {} and {last}", head.join(", ")),
                None => String::new(),
            }
        } else {
            format!("areas {} and {rest} more", first.join(", "))
        }
    };
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for &i in list {
        if n.areas[i].place != 0 {
            *counts.entry(n.areas[i].place).or_default() += 1;
        }
    }
    let own = counts.into_iter().max_by_key(|&(p, c)| (c, std::cmp::Reverse(p))).map(|(p, _)| p);
    let place = own.or_else(|| {
        let at = mid(n, list);
        n.areas
            .iter()
            .filter(|a| a.place != 0)
            .map(|a| {
                let c = a.center();
                ((c[0] - at[0]).powi(2) + (c[1] - at[1]).powi(2) + (c[2] - at[2]).powi(2), a.place)
            })
            .filter(|(d, _)| *d <= 384.0 * 384.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    });
    if let Some(name) = place.and_then(|p| n.places.get(p as usize - 1)) {
        let pretty = name.replace('_', " ");
        text.push_str(if own.is_some() { " in " } else { " near " });
        text.push_str(&pretty);
    }
    text
}

/// A ladder's ends and the areas at each (indices), as the game picks them
/// (nav::build_ladders follows the game's BuildLadders). A ladder it leaves without a top is
/// reported ("A ladder has no area at its top"); in the sim's cs_rats2 recordings those are
/// ladders bots hardly ever climb (thin plates, a vent grate, floors with no area).
#[derive(Debug, Clone)]
pub struct LadderEnds {
    pub bottom: [f32; 3],
    pub top: [f32; 3],
    pub at_bottom: Vec<usize>,
    pub at_top: Vec<usize>,
}

pub fn ladder_ends(built: &[nav::Ladder], boxes: &[nav::LadderBox]) -> Vec<LadderEnds> {
    boxes
        .iter()
        .map(|bx| {
            let (cx, cy) = ((bx.mins[0] + bx.maxs[0]) / 2.0, (bx.mins[1] + bx.maxs[1]) / 2.0);
            let (bottom, top) = ([cx, cy, bx.mins[2]], [cx, cy, bx.maxs[2]]);
            let l = built.iter().find(|l| l.entity == bx.entity);
            let at_bottom: Vec<usize> = l.and_then(|l| l.bottom_area).into_iter().collect();
            let mut at_top: Vec<usize> = l.map(|l| [l.top_forward, l.top_left, l.top_right, l.top_behind].into_iter().flatten().collect()).unwrap_or_default();
            at_top.sort_unstable();
            at_top.dedup();
            LadderEnds { bottom, top, at_bottom, at_top }
        })
        .collect()
}

/// Areas by 256-unit cell, for finding the area under a point without trying every one.
struct AreaGrid {
    cells: HashMap<(i32, i32), Vec<usize>>,
}

const CELL: f32 = 256.0;

impl AreaGrid {
    fn new(n: &Nav) -> AreaGrid {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, a) in n.areas.iter().enumerate() {
            let (lo, hi) = rect(a);
            if !(lo[0].is_finite() && lo[1].is_finite() && hi[0].is_finite() && hi[1].is_finite()) || hi[0] - lo[0] > WORLD || hi[1] - lo[1] > WORLD {
                continue;
            }
            let (x0, y0, x1, y1) = ((lo[0] / CELL).floor() as i32, (lo[1] / CELL).floor() as i32, (hi[0] / CELL).floor() as i32, (hi[1] / CELL).floor() as i32);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        AreaGrid { cells }
    }

    /// The highest area under `p` within `beneath` (the same rule as Nav::area_at).
    fn area_at(&self, n: &Nav, p: [f32; 3], beneath: f32) -> Option<usize> {
        let key = ((p[0] / CELL).floor() as i32, (p[1] / CELL).floor() as i32);
        let mut best: Option<(usize, f32)> = None;
        for &i in self.cells.get(&key).map(|v| v.as_slice()).unwrap_or(&[]) {
            let a = &n.areas[i];
            if !a.contains_xy(p[0], p[1]) {
                continue;
            }
            let z = a.z_at(p[0], p[1]);
            if z > p[2] + 5.0 || z < p[2] - beneath {
                continue;
            }
            if best.map(|(_, bz)| z > bz).unwrap_or(true) {
                best = Some((i, z));
            }
        }
        best.map(|(i, _)| i)
    }
}

/// How far past an area's edge the spots tried are: first where a player's middle has to be
/// before they drop off (half the player's width and a little more), then further on across
/// floor the nav left out.
const PAST_EDGE_STEPS: [f32; 4] = [20.0, 48.0, 80.0, 112.0];
/// Room between the spots tried along an edge.
const EDGE_STEP: f32 = 32.0;

/// Where bots end up stepping off each side of an area with no link to follow: the areas
/// they land in (indices), and whether that's a drop (more than a step down) or a walk.
/// The game's path finding only follows links and ladders, but bots leave the nav all the
/// time (strafing in a fight, dodging, pushed by others, getting unstuck), and then fall or
/// walk wherever the map lets them. In the sim's cs_rats2 recordings that's how bots get
/// into 39 areas no link leads to: off the beams and shelves onto the floor below.
fn step_offs(n: &Nav, grid: &AreaGrid, i: usize, g: &dyn Ground) -> Vec<(usize, bool)> {
    let a = &n.areas[i];
    let (lo, hi) = rect(a);
    let mut out: Vec<(usize, bool)> = Vec::new();
    let along = |from: f32, to: f32| -> Vec<f32> {
        let len = to - from;
        if len <= EDGE_STEP {
            return vec![(from + to) / 2.0];
        }
        let k = (len / EDGE_STEP).ceil() as usize;
        (0..=k).map(|j| from + 8.0_f32.min(len / 2.0) + (len - 16.0_f32.min(len)) * j as f32 / k as f32).collect()
    };
    // (inset a few units from the edge, where the floor's brush still is; then out past the
    // edge, straight out from each side and diagonally from each corner)
    let inset = 4.0_f32.min((hi[0] - lo[0]) / 2.0).min((hi[1] - lo[1]) / 2.0);
    let mut tries: Vec<([f32; 2], [f32; 2])> = Vec::new();
    for x in along(lo[0], hi[0]) {
        tries.push(([x, lo[1] + inset], [0.0, -1.0]));
        tries.push(([x, hi[1] - inset], [0.0, 1.0]));
    }
    for y in along(lo[1], hi[1]) {
        tries.push(([lo[0] + inset, y], [-1.0, 0.0]));
        tries.push(([hi[0] - inset, y], [1.0, 0.0]));
    }
    let d = std::f32::consts::FRAC_1_SQRT_2;
    for (cx, cy, dx, dy) in [(lo[0] + inset, lo[1] + inset, -d, -d), (hi[0] - inset, lo[1] + inset, d, -d), (hi[0] - inset, hi[1] - inset, d, d), (lo[0] + inset, hi[1] - inset, -d, d)] {
        tries.push(([cx, cy], [dx, dy]));
    }
    for (from, dir) in tries {
        let z = a.z_at(from[0], from[1]);
        // a little past the edge first; when that's floor with no area on it (a gap the nav
        // left out), a bit further, as a player carries on walking across it; past a drop or
        // a pit, further on is where a running jump across it lands
        for far in PAST_EDGE_STEPS {
            let to = [from[0] + dir[0] * (far + inset), from[1] + dir[1] * (far + inset)];
            let land = match g.step_off([from[0], from[1], z], to) {
                Step::Wall => break,
                Step::Fall => continue,
                Step::Land(h) => h,
            };
            let drop = z - land > SNAP_UP;
            // (look a step above where the feet land: the area's own floor can sit a little higher)
            match grid.area_at(n, [to[0], to[1], land + SNAP_UP], SNAP_UP + 24.0) {
                Some(j) if j == i => {}
                Some(j) => {
                    if let Some(seen) = out.iter_mut().find(|(k, _)| *k == j) {
                        seen.1 &= drop;
                    } else {
                        out.push((j, drop));
                    }
                }
                None if !drop => continue,
                None => {}
            }
            if !drop {
                break;
            }
            // (dropped: a running jump may still clear it to something level further on)
        }
    }
    out
}

/// Everything the auto-check finds: areas bots can't reach from any spawn (or can't get back
/// from), links to areas that aren't there, areas a player can't stand in, ladders without an
/// area at an end, areas with no size, ids used twice.
pub fn check(n: &Nav, spawns: &[[f32; 3]], ladders: &[LadderEnds], ground: Option<&dyn Ground>) -> Checked {
    let count = n.areas.len();
    let index = n.index_of();
    let mut problems = Vec::new();
    let mut note = None;

    // the walking graph: links, plus ladders both ways between their bottom and top areas
    let mut fwd: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut back: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (i, a) in n.areas.iter().enumerate() {
        for id in a.conns.iter().flatten() {
            if let Some(&j) = index.get(id) {
                fwd[i].push(j);
                back[j].push(i);
            }
        }
    }
    for l in ladders {
        for &b in &l.at_bottom {
            for &t in &l.at_top {
                fwd[b].push(t);
                back[t].push(b);
                fwd[t].push(b);
                back[b].push(t);
            }
        }
    }
    let mut starts: Vec<usize> = spawns.iter().filter_map(|&p| spawn_area(n, p)).collect();
    starts.sort_unstable();
    starts.dedup();
    if starts.is_empty() && count > 0 {
        // no spawns found: the biggest connected group stands in for "the map"
        let all: Vec<usize> = (0..count).collect();
        if let Some(big) = clusters(n, &all, &index).into_iter().next() {
            starts = big;
        }
        note = Some(if spawns.is_empty() { "No spawn points were found in this map, so the biggest connected part of the nav counts as the main map." } else { "No area is under this map's spawn points, so the biggest connected part of the nav counts as the main map." }.to_string());
    }
    // stepping off an edge where no link goes: bots don't plan it, but they end up there
    let mut loose: Vec<Vec<(usize, bool)>> = vec![Vec::new(); count];
    if let Some(g) = ground {
        let grid = AreaGrid::new(n);
        for (i, out) in loose.iter_mut().enumerate() {
            *out = step_offs(n, &grid, i, g).into_iter().filter(|(j, _)| !fwd[i].contains(j)).collect();
        }
    }
    let planned = bfs(&starts, &fwd);
    let mut any: Vec<Vec<usize>> = fwd.clone();
    for (i, out) in loose.iter().enumerate() {
        any[i].extend(out.iter().map(|(j, _)| *j));
    }
    let reached = bfs(&starts, &any);
    // (the way back is planned: a bot that fell somewhere only gets out along links)
    let returns = bfs(&starts, &back);
    let mut reach = vec![REACH_OK; count];
    let (mut never, mut trapped, mut falls) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..count {
        if !reached[i] {
            reach[i] = REACH_NEVER;
            never.push(i);
        } else if !returns[i] {
            reach[i] = REACH_TRAP;
            trapped.push(i);
        } else if !planned[i] {
            reach[i] = REACH_FALLS;
            falls.push(i);
        }
    }
    for g in clusters(n, &never, &index) {
        let text = if g.len() == 1 { format!("Bots can't reach {} from any spawn", where_is(n, &g)) } else { format!("Bots can't reach {} areas from any spawn: {}", g.len(), where_is(n, &g)) };
        problems.push(Problem {
            kind: "unreachable",
            text,
            hint: "Link them to the walkable area next to them (Link, then click one of these and one that's reachable). Delete them if nobody can really stand there.".into(),
            areas: ids(n, &g),
            at: mid(n, &g),
            fix: None,
        });
    }
    for g in clusters(n, &trapped, &index) {
        let text = if g.len() == 1 { format!("Bots that get to {} can't walk back out", where_is(n, &g)) } else { format!("Bots that get to these {} areas can't walk back out: {}", g.len(), where_is(n, &g)) };
        problems.push(Problem {
            kind: "trap",
            text,
            hint: "Bots only get there by dropping down (a one-way link, or falling off an edge). Fine for a pit you really can't climb out of; otherwise link them back to where they lead.".into(),
            areas: ids(n, &g),
            at: mid(n, &g),
            fix: None,
        });
    }
    for g in clusters(n, &falls, &index) {
        let text = if g.len() == 1 { format!("Bots only get to {} by falling in", where_is(n, &g)) } else { format!("Bots only get to these {} areas by falling in: {}", g.len(), where_is(n, &g)) };
        // the link bots would need: from an area they plan to reach to where they fall or walk in
        let way_in = (0..count).filter(|&i| planned[i]).find_map(|i| loose[i].iter().find(|(j, _)| g.contains(j)).map(|&(j, drop)| (i, j, drop)));
        problems.push(Problem {
            kind: "falls",
            text,
            hint: "No link leads there, so bots never go there on purpose; they only land there falling off an edge (or stumbling in). Link them to the area they fall from (a one-way drop) or walk in from.".into(),
            areas: ids(n, &g),
            at: mid(n, &g),
            fix: way_in.map(|(i, j, drop)| {
                let (a, b) = (n.areas[i].id, n.areas[j].id);
                let label = if drop { format!("Add a drop from {a} to {b}") } else { format!("Link {a} and {b}") };
                (label, json!({"op": "link", "a": a, "b": b, "oneWay": drop}))
            }),
        });
    }

    // links to areas that aren't there, and to themselves
    for a in &n.areas {
        for id in a.conns.iter().flatten() {
            if *id == a.id {
                problems.push(Problem {
                    kind: "self",
                    text: format!("Area {} links to itself", a.id),
                    hint: "Harmless, but it makes the bots' paths odd. Remove it.".into(),
                    areas: vec![a.id],
                    at: a.center(),
                    fix: Some(("Remove the link".into(), json!({"op": "unlink", "a": a.id, "b": a.id, "both": false}))),
                });
            } else if !index.contains_key(id) {
                problems.push(Problem {
                    kind: "dangling",
                    text: format!("Area {} links to area {id}, which isn't in the nav", a.id),
                    hint: "The game can't follow a link to an area that isn't there. Remove it.".into(),
                    areas: vec![a.id],
                    at: a.center(),
                    fix: Some(("Remove the link".into(), json!({"op": "unlink", "a": a.id, "b": id, "both": false}))),
                });
            }
        }
    }
    // the same id twice: the game finds only one of them
    let mut seen_ids = HashMap::new();
    for (i, a) in n.areas.iter().enumerate() {
        if let Some(&first) = seen_ids.get(&a.id) {
            problems.push(Problem {
                kind: "duplicate",
                text: format!("Two areas are both number {}", a.id),
                hint: "The game only finds one of them, so links to the other go astray. Delete one and draw it again.".into(),
                areas: vec![a.id],
                at: mid(n, &[first, i]),
                fix: None,
            });
        } else {
            seen_ids.insert(a.id, i);
        }
    }
    // no size
    for a in &n.areas {
        let (lo, hi) = rect(a);
        if hi[0] - lo[0] < 0.5 || hi[1] - lo[1] < 0.5 {
            problems.push(Problem {
                kind: "flat",
                text: format!("Area {} has no size", a.id),
                hint: "Bots can't stand on an area with no width. Delete it.".into(),
                areas: vec![a.id],
                at: a.center(),
                fix: Some(("Delete it".into(), json!({"op": "delete", "ids": [a.id]}))),
            });
        }
    }
    // a standing player doesn't fit (crouch areas are crouched through on purpose)
    if let Some(g) = ground {
        let mut cramped = Vec::new();
        for (i, a) in n.areas.iter().enumerate() {
            // (crouch areas are crouched through on purpose; jump areas are mid-jump)
            if a.attr & (nav::ATTR_CROUCH | nav::ATTR_JUMP) != 0 {
                continue;
            }
            let c = a.center();
            if !g.fits([c[0], c[1], a.z_at(c[0], c[1])]) {
                cramped.push(i);
            }
        }
        for i in cramped {
            let a = &n.areas[i];
            problems.push(Problem {
                kind: "cramped",
                text: format!("A player can't stand in area {}", a.id),
                hint: "Something's in the way of a standing player there. If it's a vent or a low gap, mark it crouch; if it's inside a wall, delete it.".into(),
                areas: vec![a.id],
                at: a.center(),
                fix: Some(("Mark it crouch".into(), json!({"op": "attr", "ids": [a.id], "set": nav::ATTR_CROUCH}))),
            });
        }
    }
    // ladders without an area at an end
    for l in ladders {
        if l.at_bottom.is_empty() {
            problems.push(Problem {
                kind: "ladder",
                text: "A ladder has no area at its foot".into(),
                hint: "Bots can't get on it. Draw an area on the floor where the ladder starts.".into(),
                areas: ids(n, &l.at_top),
                at: l.bottom,
                fix: None,
            });
        }
        if l.at_top.is_empty() {
            problems.push(Problem {
                kind: "ladder",
                text: "A ladder has no area at its top".into(),
                hint: "Bots climb it but can't get off. Draw an area where players step off at the top.".into(),
                areas: ids(n, &l.at_bottom),
                at: l.top,
                fix: None,
            });
        }
    }
    let more = problems.len().saturating_sub(MAX_PROBLEMS);
    problems.truncate(MAX_PROBLEMS);
    Checked { reach, problems, more, spawn_areas: starts, note }
}

// ---- saving ---------------------------------------------------------------------------------

/// Writes `bytes` over `path` in one step (`<path>.part` then rename), keeping the file that
/// was there as `<path>.bak` the first time. Returns whether a backup was made now.
pub fn write_nav(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<bool> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    // the app-wide history too (safety.rs): the .bak is only the very first version
    crate::safety::before("Nav fixer", &format!("Saved {name}"), path);
    let part = path.with_file_name(format!("{name}.part"));
    let bak = path.with_file_name(format!("{name}.bak"));
    let r = (|| {
        std::fs::write(&part, bytes)?;
        let backed = if path.is_file() && !bak.exists() {
            std::fs::copy(path, &bak)?;
            true
        } else {
            false
        };
        std::fs::rename(&part, path)?;
        Ok(backed)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    r
}

/// The .lsv next to a nav: (stage, whether it was built for this nav). None if there's none.
fn knowledge_of(nav_path: &std::path::Path, n: &Nav) -> Option<(u32, bool)> {
    use std::io::Read;
    let p = sibling_lsv(nav_path)?;
    let mut head = [0u8; 20];
    std::fs::File::open(p).ok()?.read_exact(&mut head).ok()?;
    let word = |i: usize| u32::from_le_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
    if word(0) != lsv::LSV_MAGIC {
        return Some((0, false));
    }
    let fits = word(4) == lsv::LSV_VERSION && word(8) == lsv::nav_key(n) && word(12) as usize == n.areas.len();
    Some((word(16), fits))
}

/// `<map>.lsv` beside `<map>.nav`, whatever its case.
fn sibling_lsv(nav_path: &std::path::Path) -> Option<PathBuf> {
    let stem = nav_path.file_stem()?.to_string_lossy().to_string();
    crate::mapcopy::Finder::default().find(nav_path.parent()?, &format!("{stem}.lsv"))
}

/// Roughly how long the hidden server takes to rebuild the knowledge for `areas` areas, in
/// seconds: starting it, then the brain's work (about 11 s for de_dust2's 737 areas with
/// nobody on; it grows with the square of the areas: every pair is looked at).
pub fn rebuild_seconds(areas: usize) -> u64 {
    let k = areas as f64 / 737.0;
    (45.0 + 11.0 * k * k).round() as u64
}

fn about(secs: u64) -> String {
    if secs < 90 {
        "about a minute".into()
    } else {
        format!("about {} minutes", (secs + 30) / 60)
    }
}

// ---- sessions -------------------------------------------------------------------------------

struct Session {
    nav_path: PathBuf,
    /// The file as it was when read or last saved (a change on disk refuses a save).
    disk: Option<cache::Stamp>,
    doc: Doc,
    saved_gen: u64,
    undo: Vec<(String, Doc)>,
    redo: Vec<(String, Doc)>,
    used: std::time::Instant,
}

fn sessions() -> &'static Mutex<Vec<Session>> {
    static S: OnceLock<Mutex<Vec<Session>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// The map's files and facts the editor needs, kept for the last two map versions.
#[derive(Clone)]
struct MapCtx {
    hulls: Option<Arc<MapHulls>>,
    hull_error: Option<String>,
    spawns: Vec<[f32; 3]>,
    bsp_size: Option<u64>,
}

fn map_ctx(m: &MapFile) -> Done<MapCtx> {
    static MEMO: OnceLock<Mutex<Vec<(PathBuf, cache::Stamp, MapCtx)>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(&m.bsp).map_err(|_| Fail::no_map())?;
    if let Some(c) = memo.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == m.bsp && e.1 == stamp) {
        return Ok(c.2.clone());
    }
    let (hulls, hull_error) = match goldsrc::read_capped(&m.bsp, "map file", goldsrc::MAX_BSP).and_then(|b| MapHulls::parse(&b)) {
        Ok(h) => (Some(Arc::new(h)), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let spawns = bsp::read_lumps(&m.bsp, &[bsp::LUMP_ENTITIES])
        .map(|l| goldsrc::entities::parse(&crate::rcon::latin1(&l[0])))
        .unwrap_or_default()
        .iter()
        .filter(|e| matches!(e.classname(), "info_player_start" | "info_player_deathmatch"))
        .filter_map(|e| e.origin())
        .collect();
    let c = MapCtx { hulls, hull_error, spawns, bsp_size: Some(stamp.size) };
    let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
    list.push((m.bsp.clone(), stamp, c.clone()));
    if list.len() > 2 {
        list.remove(0);
    }
    Ok(c)
}

fn nav_path(m: &MapFile) -> Done<PathBuf> {
    m.sibling("nav").ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, NO_NAV))
}

fn read_doc(p: &std::path::Path) -> Done<Doc> {
    let b = goldsrc::read_capped(p, "navigation file", nav::MAX_NAV)?;
    Ok(Doc::from_nav(Nav::parse(&b)?))
}

/// Runs `f` on the map's session (opened from the file if there's none, or `fresh`).
fn with_session<T>(m: &MapFile, fresh: bool, f: impl FnOnce(&mut Session) -> Done<T>) -> Done<T> {
    let p = nav_path(m)?;
    let mut list = sessions().lock().unwrap_or_else(|e| e.into_inner());
    if fresh {
        list.retain(|s| s.nav_path != p);
    }
    let i = match list.iter().position(|s| s.nav_path == p) {
        Some(i) => i,
        None => {
            let doc = read_doc(&p)?;
            // (four maps open at most: the one used longest ago goes)
            if list.len() >= 4 {
                if let Some(old) = list.iter().enumerate().min_by_key(|(_, s)| s.used).map(|(i, _)| i) {
                    list.remove(old);
                }
            }
            list.push(Session { disk: cache::Stamp::of(&p).ok(), saved_gen: doc.gen, doc, nav_path: p, undo: vec![], redo: vec![], used: std::time::Instant::now() });
            list.len() - 1
        }
    };
    let s = &mut list[i];
    s.used = std::time::Instant::now();
    f(s)
}

/// What the page gets back after anything.
struct View {
    doc: Doc,
    nav_path: PathBuf,
    dirty: bool,
    undo: Option<String>,
    redo: Option<String>,
    steps: usize,
}

fn view(s: &Session) -> View {
    View {
        doc: s.doc.clone(),
        nav_path: s.nav_path.clone(),
        dirty: s.doc.gen != s.saved_gen,
        undo: s.undo.last().map(|u| u.0.clone()),
        redo: s.redo.last().map(|u| u.0.clone()),
        steps: s.undo.len(),
    }
}

fn state_json(m: &MapFile, v: &View, ctx: &MapCtx, extra: Value, can_rebuild: Result<(), String>) -> Value {
    let n = v.doc.to_nav();
    let ground: Option<&dyn Ground> = ctx.hulls.as_deref().map(|h| h as &dyn Ground);
    let ladders = ctx.hulls.as_ref().map(|h| nav::build_ladders(&n, &h.ladders, h.as_ref())).unwrap_or_default();
    let ends = ctx.hulls.as_ref().map(|h| ladder_ends(&ladders, &h.ladders)).unwrap_or_default();
    let checked = check(&n, &ctx.spawns, &ends, ground);
    let mut navj = super::overlays::nav_json(&n, ctx.bsp_size, &ladders);
    navj["warning"] = json!(ctx.hull_error.as_ref().map(|e| format!("The map's shape couldn't be read, so heights aren't snapped to the floor and ladders aren't shown: {e}")));
    let know = knowledge_of(&v.nav_path, &n);
    let secs = rebuild_seconds(n.areas.len());
    let mut out = json!({
        "map": m.name,
        "file": v.nav_path.file_name().map(|f| f.to_string_lossy().to_string()),
        "nav": navj,
        "reach": checked.reach,
        "spawnAreas": checked.spawn_areas.iter().map(|&i| n.areas[i].id).collect::<Vec<_>>(),
        "problems": checked.problems.iter().map(Problem::json).collect::<Vec<_>>(),
        "moreProblems": checked.more,
        "checkNote": checked.note,
        "dirty": v.dirty,
        "undo": v.undo,
        "redo": v.redo,
        "steps": v.steps,
        "backup": v.nav_path.with_file_name(format!("{}.bak", v.nav_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default())).is_file(),
        "knowledge": know.map(|(stage, fits)| json!({"stage": stage, "done": stage >= 4, "fits": fits})),
        "rebuild": {"seconds": secs, "about": about(secs), "can": can_rebuild.is_ok(), "why": can_rebuild.err(), "job": REBUILD_KIND},
    });
    if let (Value::Object(o), Value::Object(e)) = (&mut out, extra) {
        o.extend(e);
    }
    out
}

/// Whether this computer can rebuild knowledge (Err: why not, in plain words).
fn rebuild_ready(st: &Shared, m: &MapFile) -> Result<(), String> {
    let client = crate::servers::client(st).ok_or("Set up the game on this computer first.")?;
    if client.kind == "crossover" && !std::path::Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    brain_dll(m).map(|_| ())
}

fn state_for(st: &Shared, m: &MapFile, v: View, extra: Value) -> Done<Value> {
    let ctx = map_ctx(m)?;
    Ok(state_json(m, &v, &ctx, extra, rebuild_ready(st, m)))
}

/// Whether the game on this computer has `map` loaded right now.
fn map_loaded(game: &crate::game::Game, map: &str) -> bool {
    let s = game.state();
    s["online"] == json!(true) && s["state"]["map"].as_str().is_some_and(|m| m.eq_ignore_ascii_case(map))
}

/// Saves the session's nav (see the module notes). `loaded`: whether the game has the map.
fn save_session(s: &mut Session, loaded: bool) -> Done<Value> {
    if loaded {
        return Err(Fail::new(StatusCode::CONFLICT, LOADED));
    }
    if s.disk.is_some() && cache::Stamp::of(&s.nav_path).ok() != s.disk {
        return Err(Fail::new(StatusCode::CONFLICT, CHANGED));
    }
    let n = s.doc.to_nav();
    let bytes = n.write();
    // (never write something the game would refuse)
    Nav::parse(&bytes).map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, format!("The nav came out wrong, so nothing was saved ({e}).")))?;
    let backed = write_nav(&s.nav_path, &bytes).map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, format!("The nav couldn't be saved in the maps folder ({e}).")))?;
    s.disk = cache::Stamp::of(&s.nav_path).ok();
    s.saved_gen = s.doc.gen;
    let know = knowledge_of(&s.nav_path, &n);
    let stale = matches!(know, Some((_, false)));
    let bak = format!("{}.bak", s.nav_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    let mut msg = format!("Saved {}.", s.nav_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    if backed {
        msg.push_str(&format!(" The nav you started from is kept as {bak}."));
    }
    if stale {
        msg.push_str(" The bots' map knowledge was made for the old nav: rebuild it, or play the map and the bots rebuild it themselves in the first minutes.");
    }
    Ok(json!({"saved": true, "message": msg, "backedUp": backed, "knowledgeStale": stale}))
}

// ---- routes ---------------------------------------------------------------------------------

fn panel(h: &HeaderMap) -> Done<()> {
    if crate::panel_api::from_panel(h) {
        Ok(())
    } else {
        Err(Fail::new(StatusCode::FORBIDDEN, NOT_PANEL))
    }
}

async fn api_open(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let v = with_session(&m, false, |s| Ok(view(s)))?;
                state_for(&st, &m, v, json!({}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_edit(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap, body: axum::body::Bytes) -> Response {
    let r = match (find_map(&st, &name), panel(&h)) {
        (Err(f), _) | (_, Err(f)) => Err(f),
        (Ok((game, name)), Ok(())) => {
            blocking(move || {
                let op: Op = serde_json::from_slice(&body[..body.len().min(64 << 10)]).map_err(|_| Fail::new(StatusCode::BAD_REQUEST, "That edit isn't one the nav fixer knows."))?;
                let m = locate(&game, &name)?;
                let ctx = map_ctx(&m)?;
                let ground: Option<&dyn Ground> = ctx.hulls.as_deref().map(|h| h as &dyn Ground);
                let (v, extra) = with_session(&m, false, |s| {
                    let e = apply(&s.doc, &op, ground).map_err(|msg| Fail::new(StatusCode::OK, msg))?;
                    let old = std::mem::replace(&mut s.doc, e.doc);
                    s.undo.push((e.label.clone(), old));
                    if s.undo.len() > MAX_UNDO {
                        s.undo.remove(0);
                    }
                    s.redo.clear();
                    Ok((view(s), json!({"did": e.label, "notes": e.notes, "newId": e.new_id})))
                })?;
                state_for(&st, &m, v, extra)
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_undo(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    history(st, name, h, true).await
}

async fn api_redo(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    history(st, name, h, false).await
}

async fn history(st: Shared, name: String, h: HeaderMap, undo: bool) -> Response {
    let r = match (find_map(&st, &name), panel(&h)) {
        (Err(f), _) | (_, Err(f)) => Err(f),
        (Ok((game, name)), Ok(())) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let (v, extra) = with_session(&m, false, |s| {
                    let (from, to) = if undo { (&mut s.undo, &mut s.redo) } else { (&mut s.redo, &mut s.undo) };
                    let Some((label, doc)) = from.pop() else {
                        return Err(Fail::new(StatusCode::OK, if undo { "Nothing to undo." } else { "Nothing to redo." }));
                    };
                    let cur = std::mem::replace(&mut s.doc, doc);
                    to.push((label.clone(), cur));
                    Ok((view(s), json!({"did": if undo { format!("Undid: {label}") } else { format!("Redid: {label}") }})))
                })?;
                state_for(&st, &m, v, extra)
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_save(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    let r = match (find_map(&st, &name), panel(&h)) {
        (Err(f), _) | (_, Err(f)) => Err(f),
        (Ok((game, name)), Ok(())) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let loaded = map_loaded(&game, &m.name);
                let (v, extra) = with_session(&m, false, |s| {
                    let said = save_session(s, loaded)?;
                    Ok((view(s), said))
                })?;
                state_for(&st, &m, v, extra)
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_reload(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    let r = match (find_map(&st, &name), panel(&h)) {
        (Err(f), _) | (_, Err(f)) => Err(f),
        (Ok((game, name)), Ok(())) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let v = with_session(&m, true, |s| Ok(view(s)))?;
                state_for(&st, &m, v, json!({"did": "Read the nav file again"}))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_rebuild(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    let r = match (find_map(&st, &name), panel(&h)) {
        (Err(f), _) | (_, Err(f)) => Err(f),
        (Ok((game, name)), Ok(())) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                if map_loaded(&game, &m.name) {
                    return Err(Fail::new(StatusCode::OK, "The game has this map loaded right now: the bots there are already rebuilding its knowledge. Change map first to rebuild it here."));
                }
                let dirty = with_session(&m, false, |s| Ok(s.doc.gen != s.saved_gen))?;
                if dirty {
                    return Err(Fail::new(StatusCode::OK, "Save the nav first: the knowledge is built for the saved file."));
                }
                let client = crate::servers::client(&st).ok_or_else(|| Fail::new(StatusCode::OK, "Set up the game on this computer first."))?;
                let p = nav_path(&m)?;
                let job = start_rebuild(client, m, p).map_err(|e| Fail::new(StatusCode::OK, e))?;
                Ok(json!({"ok": true, "job": job}))
            })
            .await
        }
    };
    json_answer(r)
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/navfix/:name", get(api_open))
        .route("/api/tools/navfix/:name/edit", post(api_edit))
        .route("/api/tools/navfix/:name/undo", post(api_undo))
        .route("/api/tools/navfix/:name/redo", post(api_redo))
        .route("/api/tools/navfix/:name/save", post(api_save))
        .route("/api/tools/navfix/:name/reload", post(api_reload))
        .route("/api/tools/navfix/:name/rebuild", post(api_rebuild))
}

// ---- rebuilding the knowledge ---------------------------------------------------------------
//
// The bot brain (our ReGameDLL) builds a map's knowledge by itself while bots are on the map,
// and starts again whenever the nav it loads isn't the one the .lsv was made for. So the
// rebuild is: the app's hidden server (navs.rs LocalLearner's folder, made the same way) with
// the brain from this computer's game in place of Valve's server code, the map and the new nav
// copied in, one bot, nobody on (the brain works ten times faster then). When the brain logs
// "map knowledge for <map>:" the .lsv goes next to the nav, checked against it.

/// The setup job's kind while knowledge is rebuilt.
pub const REBUILD_KIND: &str = "knowfix";

/// The bot brain's server code in this computer's game (the `gamedll` liblist.gam names when
/// it's ours, else dlls/mp.dll), checked to be our ReGameDLL.
fn brain_dll(m: &MapFile) -> Result<PathBuf, String> {
    let gdir = m.hl.join(&m.game);
    let f = crate::mapcopy::Finder::default();
    let p = f.find(&gdir, crate::gamebin::MP).ok_or("The bot brain isn't installed in your game. Open Mods and click Repair, then try again.")?;
    let b = std::fs::read(&p).map_err(|e| format!("The bot brain couldn't be read ({e})."))?;
    let needle = b"lonerstrike_mapknow";
    if !b.windows(needle.len()).any(|w| w == needle) {
        return Err("Your game has a different server DLL than Loner-Strike's bot brain, so it can't build map knowledge. Open Mods and click Repair, then try again.".into());
    }
    Ok(p)
}

/// The DLL a game folder's liblist.gam loads (relative, forward slashes).
fn liblist_dll(game_dir: &std::path::Path) -> String {
    let text = std::fs::read(game_dir.join("liblist.gam")).map(|b| crate::rcon::latin1(&b)).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            let key = crate::gamebin::LIBLIST_KEY;
            let rest = l.strip_prefix(key).filter(|r| r.starts_with([' ', '\t']))?;
            Some(rest.trim().trim_matches('"').replace('\\', "/"))
        })
        .find(|p| !p.is_empty() && !p.contains(".."))
        .unwrap_or_else(|| crate::gamebin::MP.into())
}

/// Puts the hidden server's own DLL back however the run ends.
struct DllSwap {
    dll: PathBuf,
    keep: PathBuf,
}

impl Drop for DllSwap {
    fn drop(&mut self) {
        if self.keep.is_file() {
            let _ = std::fs::rename(&self.keep, &self.dll);
        }
    }
}

fn free_port() -> Result<u16, String> {
    let taken: HashSet<u16> = crate::servers::load().iter().filter(|e| e.remote.is_none()).map(|e| e.port).collect();
    (27140..27190)
        .find(|p| !taken.contains(p) && std::net::UdpSocket::bind(("0.0.0.0", *p)).is_ok())
        .ok_or_else(|| "No free port for the hidden server.".to_string())
}

#[cfg(unix)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setpriority only changes the new process's own scheduling, between fork and exec
    unsafe {
        cmd.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, 15);
            Ok(())
        });
    }
}

#[cfg(windows)]
fn low_priority(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000 | 0x0000_4000);
}

/// New text in a log since `from` (moved on to its end; at most 1 MB at a time).
fn read_new(path: &std::path::Path, from: &mut u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *from {
        *from = 0;
    }
    if f.seek(SeekFrom::Start(*from)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = f.take(1 << 20).read_to_end(&mut buf);
    *from += buf.len() as u64;
    crate::rcon::latin1(&buf)
}

/// Starts the rebuild job for the saved nav at `nav_path`. Returns the job id.
fn start_rebuild(client: crate::steam::Install, m: MapFile, nav_path: PathBuf) -> Result<String, String> {
    use crate::navs::Learner;
    if client.kind == "crossover" && !std::path::Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        return Err("CrossOver isn't installed in Applications. The hidden server runs through it, like the game.".into());
    }
    let brain = brain_dll(&m)?;
    let n = Nav::parse(&crate::caps::read_file(&nav_path, "saved nav", crate::goldsrc::nav::MAX_NAV)?).map_err(|e| e.to_string())?; // [Caps]
    let root = crate::servers::root_for(&client)?;
    let first = !crate::servers::hlds_in(&root.join(".base")).is_file();
    let secs = rebuild_seconds(n.areas.len());
    let steps = vec![
        crate::setup::server_step(&m.game, "knowfix-prep", if first { "Getting a hidden server ready (downloads Valve's server the first time, about 1 GB)" } else { "Getting a hidden server ready" }, 1.0),
        crate::setup::server_step(&m.game, "knowfix-run", format!("Bots are studying {}'s new nav ({})", m.name, about(secs)), 3.0),
    ];
    let mut learner = crate::navs::LocalLearner::new(client.clone())?;
    let game = m.game.clone();
    crate::setup::start_job(REBUILD_KIND, steps, move |job| {
        let r = job
            .run(&format!("{game}/knowfix-prep"), || learner.prepare(job, std::slice::from_ref(&game)))
            .and_then(|_| job.run(&format!("{game}/knowfix-run"), || study(job, &client, &root.join(".nav-learner"), &m, &nav_path, &n, &brain, secs)));
        learner.finish();
        r
    })
}

/// The hidden server builds the knowledge; the .lsv goes next to the nav.
#[allow(clippy::too_many_arguments)]
fn study(job: &crate::setup::Job, client: &crate::steam::Install, dir: &std::path::Path, m: &MapFile, nav_path: &std::path::Path, n: &Nav, brain: &std::path::Path, secs: u64) -> Result<(), String> {
    use crate::setup::Progress;
    use std::time::{Duration, Instant};
    let gdir = dir.join(&m.game);
    let maps = gdir.join("maps");
    std::fs::create_dir_all(&maps).map_err(|e| e.to_string())?;
    // the brain in place of Valve's server code, put back however this ends
    let dll = gdir.join(liblist_dll(&gdir));
    let keep = dll.with_extension("dll.ls-valve");
    if dll.is_file() && !keep.is_file() {
        std::fs::rename(&dll, &keep).map_err(|e| format!("The hidden server's files couldn't be changed ({e})."))?;
    }
    let _swap = DllSwap { dll: dll.clone(), keep };
    std::fs::copy(brain, &dll).map_err(|e| format!("The bot brain couldn't be copied ({e})."))?;
    // bots and the brain on (Valve's server has no ReGameDLL game.cfg)
    let cfg = gdir.join("server.cfg");
    let mut text = std::fs::read_to_string(&cfg).unwrap_or_default();
    text.push_str("\n// the brain builds map knowledge\nbot_enable 1\nbot_ls_fight 1\nbot_quota 1\n");
    std::fs::write(&cfg, text).map_err(|e| e.to_string())?;

    // the map, what it needs, and the new nav; no old knowledge
    let f = crate::mapcopy::Finder::default();
    let bsp = maps.join(format!("{}.bsp", m.name));
    let stock = dir.parent().map(|r| r.join(".base").join(&m.game).join("maps").join(format!("{}.bsp", m.name)).is_file()).unwrap_or(false);
    std::fs::copy(&m.bsp, &bsp).map_err(|e| format!("The map couldn't be copied ({e})."))?;
    if let Some(text) = crate::navs::entity_text(&m.bsp) {
        for rel in crate::navs::referenced_files(&text) {
            if [gdir.clone(), dir.join("cstrike"), dir.join("valve")].iter().any(|d| f.find(d, &rel).is_some()) {
                continue;
            }
            // where the game looks (a map it downloaded has its files in a _downloads folder)
            if let Some(src) = super::doctor::game_path(&m.hl, &m.game).iter().find_map(|d| f.find(d, &rel)) {
                let dest = gdir.join(&rel);
                if let Some(d) = dest.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::copy(src, dest);
            }
        }
    }
    for ext in ["nav", "lsv"] {
        while let Some(p) = crate::mapcopy::Finder::default().find(&maps, &format!("{}.{ext}", m.name)) {
            std::fs::remove_file(&p).map_err(|_| "An old file in the hidden server's folder couldn't be cleared.".to_string())?;
        }
    }
    let nav_copy = maps.join(format!("{}.nav", m.name));
    let lsv_out = maps.join(format!("{}.lsv", m.name));
    std::fs::copy(nav_path, &nav_copy).map_err(|e| format!("The nav couldn't be copied ({e})."))?;

    // start it
    let port = free_port()?;
    let password: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..20).map(|_| rng.sample(rand::distributions::Alphanumeric) as char).collect()
    };
    let hlds = crate::servers::hlds_in(dir);
    if !hlds.is_file() {
        return Err("The hidden server's files are missing.".into());
    }
    let log_path = dir.join("knowledge-server.log");
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    crate::navs::write_learn_map(&dir.join(&m.game), &m.name)?;
    let spawn = |log: std::fs::File| -> Result<std::process::Child, String> {
        let mut cmd = crate::servers::hlds_command(&client.kind, client.bottle.as_deref(), dir, true)
            .map_err(|_| "The hidden server has to be inside the game's CrossOver bottle.".to_string())?;
        low_priority(&mut cmd);
        cmd.args(crate::navs::learn_args(&m.game, port, &password))
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log);
        cmd.spawn().map_err(|e| format!("The hidden server didn't start ({e})."))
    };
    let mut child = spawn(log)?;
    crate::sys::tie_to_app(&child);
    // (the hidden servers' retry rules: crate::tools::health::retry_reason_with; this one works for
    // minutes without answering, so a hang is only Valve's SDL3.dll line)
    let (mut steam, mut hang) = (0u32, 0u32);

    let logs = [dir.join("qconsole.log"), gdir.join("qconsole.log"), log_path, gdir.join("lonerstrike_mapknow.log")];
    let mut from: Vec<u64> = logs.iter().map(|p| std::fs::metadata(p).map(|x| x.len()).unwrap_or(0)).collect();
    from[2] = 0;
    let mut started = Instant::now();
    let limit = Duration::from_secs((secs * 6).max(15 * 60));
    let done_line = format!("map knowledge for {}:", m.name.to_lowercase());
    let mut text = String::new();
    let result = loop {
        std::thread::sleep(Duration::from_secs(1));
        for (p, at) in logs.iter().zip(from.iter_mut()) {
            text.push_str(&read_new(p, at));
        }
        let lower = crate::tools::health::after_restart(&text).to_lowercase();
        if lower.contains(&done_line) {
            break Ok(());
        }
        let closed = matches!(child.try_wait(), Ok(Some(_)));
        if let Some(why) = crate::tools::health::retry_reason_with(&text, closed, started.elapsed(), steam, hang, None) {
            if why == "steam" {
                steam += 1;
            } else {
                hang += 1;
            }
            job.detail(&crate::tools::health::retry_detail(why));
            if let Err(e) = crate::tools::health::restart_hidden(&mut child, dir, &logs[2], why, &spawn) {
                break Err(e);
            }
            started = Instant::now();
            continue;
        }
        if lower.contains("host_error") || lower.contains("fatal error") {
            break Err(crate::tools::health::closed_why(&text, "the hidden server stopped with an error loading the map."));
        }
        if closed {
            break Err(crate::tools::health::closed_why(&text, "the hidden server closed while loading the map."));
        }
        if started.elapsed() > limit {
            break Err(format!("it took longer than {} minutes.", limit.as_secs() / 60));
        }
        // the brain saves its progress as it goes: say which stage it's on
        let stage = std::fs::File::open(&lsv_out).ok().and_then(|mut fh| {
            use std::io::Read;
            let mut head = [0u8; 20];
            fh.read_exact(&mut head).ok()?;
            Some(u32::from_le_bytes([head[16], head[17], head[18], head[19]]))
        });
        job.detail(&match stage {
            Some(s) if (s as usize) < lsv::STAGES.len() - 1 => format!("Working out {} ({} of 4)", ["sightlines", "cover", "angles to watch", "exposure"][s as usize], s + 1),
            _ => format!("Loading the map ({} s)", started.elapsed().as_secs()),
        });
    };
    let _ = crate::rcon::Rcon::at(password, "127.0.0.1", port).send("quit");
    for _ in 0..25 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    crate::sys::stop_child(child, Duration::from_secs(3));
    let finish = || -> Result<(), String> {
        result.map_err(|why| format!("The knowledge couldn't be built: {why}"))?;
        let b = crate::caps::read_file(&lsv_out, "knowledge file", crate::goldsrc::lsv::MAX_LSV).map_err(|e| if e.contains("too big") { e } else { "The knowledge couldn't be built: the hidden server didn't save it.".to_string() })?; // [Caps]
        let k = lsv::Lsv::parse(&b).map_err(|e| format!("The knowledge came out damaged ({e})."))?;
        if k.head.key != lsv::nav_key(n) || k.head.stage < 4 {
            return Err("The knowledge that came out isn't for this nav. Try again.".into());
        }
        let dest = sibling_lsv(nav_path).unwrap_or_else(|| nav_path.with_extension("lsv"));
        let name = dest.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let part = dest.with_file_name(format!("{name}.part"));
        crate::safety::before("Nav fixer", &format!("Learned the map into {name}"), &dest);
        std::fs::write(&part, &b).and_then(|_| std::fs::rename(&part, &dest)).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("The knowledge couldn't be saved in the maps folder ({e}).")
        })?;
        Ok(())
    };
    let r = finish();
    for p in [nav_copy, lsv_out] {
        let _ = std::fs::remove_file(p);
    }
    if !stock {
        let _ = std::fs::remove_file(bsp);
    }
    r
}

// ---- tests ----------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::nav::{Approach, Encounter, HidingSpot, SpotOrder};

    fn area(id: u32, x: [f32; 2], y: [f32; 2], z: f32) -> Area {
        Area { id, attr: 0, lo: [x[0], y[0], z], hi: [x[1], y[1], z], ne_z: z, sw_z: z, conns: Default::default(), hiding: vec![], approach: vec![], encounters: vec![], place: 0 }
    }

    /// A row of three rooms joined both ways (1 - 2 - 3) and a fourth on its own; a spawn in 1.
    fn rooms() -> Doc {
        let mut a = area(1, [0.0, 100.0], [0.0, 100.0], 0.0);
        let mut b = area(2, [100.0, 200.0], [0.0, 100.0], 0.0);
        let mut c = area(3, [200.0, 300.0], [0.0, 100.0], 0.0);
        let d = area(4, [500.0, 600.0], [0.0, 100.0], 0.0);
        a.conns[1] = vec![2];
        b.conns[3] = vec![1];
        b.conns[1] = vec![3];
        c.conns[3] = vec![2];
        a.place = 1;
        b.hiding = vec![HidingSpot { id: 9, pos: [150.0, 50.0, 0.0], flags: nav::SPOT_IN_COVER }];
        a.encounters = vec![Encounter { from: 3, from_dir: 1, to: 2, to_dir: 1, spots: vec![SpotOrder { id: 9, t: 10 }] }];
        a.approach = vec![Approach { here: 1, prev: 3, prev_to_here: 1, next: 2, here_to_next: 1 }];
        Doc::from_nav(Nav { version: 5, bsp_size: 4242, places: vec!["Mid".into()], areas: vec![a, b, c, d].into_iter().collect() })
    }

    const SPAWN: [f32; 3] = [50.0, 50.0, 36.0];

    fn run(doc: &Doc, op: Value) -> Doc {
        let op: Op = serde_json::from_value(op).unwrap();
        apply(doc, &op, None).unwrap().doc
    }

    fn conns(d: &Doc, id: u32) -> Vec<Vec<u32>> {
        d.areas[d.pos(id).unwrap()].conns.to_vec()
    }

    /// A flat floor at z 0 everywhere, a beam at z 256 over x 0..10, a wall over x >= 1000.
    struct Floor;
    impl Ground for Floor {
        fn floor(&self, x: f32, _y: f32, z: f32) -> Option<f32> {
            if x >= 1000.0 {
                return None;
            }
            let top = if (0.0..=10.0).contains(&x) { 256.0 } else { 0.0 };
            (top <= z + SNAP_UP && top >= z - SNAP_DOWN).then_some(top)
        }
        fn fits(&self, p: [f32; 3]) -> bool {
            p[0] < 900.0
        }
    }

    #[test]
    fn add_round_trips_through_the_file() {
        let d = rooms();
        let e = apply(&d, &serde_json::from_value(json!({"op": "add", "x": [700, 800], "y": [100, 0], "z": 8, "snap": false})).unwrap(), None).unwrap();
        assert_eq!(e.new_id, Some(5));
        assert_eq!(e.label, "Add area 5");
        let n = e.doc.to_nav();
        let back = Nav::parse(&n.write()).unwrap();
        assert_eq!(back, n);
        let a = &back.areas[4];
        assert_eq!((a.lo, a.hi, a.ne_z, a.sw_z), ([700.0, 0.0, 8.0], [800.0, 100.0, 8.0], 8.0, 8.0));
        // the old document is untouched (undo keeps it)
        assert_eq!(d.areas.len(), 4);
        // its place: the nearest named area's, unless one is given
        assert_eq!(a.place, 0, "area 1 (Mid) is too far");
        let near = run(&d, json!({"op": "add", "x": [0, 50], "y": [100, 150], "z": 0, "snap": false}));
        assert_eq!(near.areas[4].place, 1);
        let named = run(&d, json!({"op": "add", "x": [0, 50], "y": [100, 150], "z": 0, "place": "Long", "snap": false}));
        assert_eq!((named.areas[4].place, named.places.clone()), (2, vec!["Mid".to_string(), "Long".to_string()]));
        // bad boxes say so
        for bad in [json!({"op": "add", "x": [0, 0.5], "y": [0, 10], "z": 0}), json!({"op": "add", "x": [0, 1e9], "y": [0, 10], "z": 0}), json!({"op": "add", "x": [0, 10], "y": [0, 10], "z": f32::MAX})] {
            assert!(apply(&d, &serde_json::from_value(bad.clone()).unwrap(), None).is_err(), "{bad}");
        }
    }

    #[test]
    fn add_snaps_to_the_floor() {
        let d = rooms();
        // drawn a little above the beam: every corner lands on it
        let e = apply(&d, &serde_json::from_value(json!({"op": "add", "x": [0, 10], "y": [0, 80], "z": 250})).unwrap(), Some(&Floor)).unwrap();
        let a = &e.doc.areas[4];
        assert_eq!([a.lo[2], a.ne_z, a.hi[2], a.sw_z], [256.0; 4]);
        assert!(e.notes.is_empty(), "{:?}", e.notes);
        // half off the beam: the middle decides, corners off the edge take its height
        let e = apply(&d, &serde_json::from_value(json!({"op": "add", "x": [4, 30], "y": [0, 80], "z": 6})).unwrap(), Some(&Floor)).unwrap();
        let a = &e.doc.areas[4];
        assert_eq!([a.lo[2], a.ne_z, a.hi[2], a.sw_z], [0.0; 4]);
        // nothing below: flat where drawn, and said
        let e = apply(&d, &serde_json::from_value(json!({"op": "add", "x": [1000, 1100], "y": [0, 80], "z": 40})).unwrap(), Some(&Floor)).unwrap();
        assert_eq!(e.doc.areas[4].lo[2], 40.0);
        assert!(e.notes[0].contains("Couldn't find the floor"), "{:?}", e.notes);
        // somewhere a player doesn't fit
        let e = apply(&d, &serde_json::from_value(json!({"op": "add", "x": [900, 950], "y": [0, 80], "z": 0})).unwrap(), Some(&Floor)).unwrap();
        assert!(e.notes.iter().any(|n| n.contains("doesn't fit")), "{:?}", e.notes);
    }

    /// QA4-T2 L1: several fixes in one click are one edit (one Undo), all or none.
    #[test]
    fn a_batch_is_one_step() {
        let d = rooms();
        let op = json!({"op": "batch", "label": "Fix 2 problems", "ops": [{"op": "link", "a": 3, "b": 4}, {"op": "link", "a": 1, "b": 4, "oneWay": true}]});
        let e = apply(&d, &serde_json::from_value(op).unwrap(), None).unwrap();
        assert_eq!(e.label, "Fix 2 problems");
        assert!(conns(&e.doc, 3).iter().flatten().any(|&x| x == 4) && conns(&e.doc, 1).iter().flatten().any(|&x| x == 4));
        assert!(!conns(&d, 1).iter().flatten().any(|&x| x == 4), "the old document is untouched");
        // one bad edit: nothing changes
        let bad = json!({"op": "batch", "ops": [{"op": "link", "a": 3, "b": 4}, {"op": "link", "a": 3, "b": 999}]});
        assert!(apply(&d, &serde_json::from_value(bad).unwrap(), None).is_err());
        for bad in [json!({"op": "batch", "ops": []}), json!({"op": "batch", "ops": [{"op": "batch", "ops": [{"op": "link", "a": 3, "b": 4}]}]})] {
            assert!(apply(&d, &serde_json::from_value(bad.clone()).unwrap(), None).is_err(), "{bad}");
        }
    }

    #[test]
    fn link_and_unlink_both_ways_and_one_way() {
        let d = rooms();
        let l = run(&d, json!({"op": "link", "a": 3, "b": 4}));
        assert_eq!(conns(&l, 3)[1], vec![4]);
        assert_eq!(conns(&l, 4)[3], vec![3]);
        assert!(apply(&l, &serde_json::from_value(json!({"op": "link", "a": 4, "b": 3})).unwrap(), None).unwrap_err().contains("already linked"));
        let n = Nav::parse(&l.to_nav().write()).unwrap();
        assert!(n.one_way_links().is_empty());

        // one way: a drop from 4 to 3 (3's way back goes)
        let o = run(&l, json!({"op": "link", "a": 4, "b": 3, "oneWay": true}));
        assert_eq!(conns(&o, 4)[3], vec![3]);
        assert!(conns(&o, 3).iter().all(|c| !c.contains(&4)));
        let n = Nav::parse(&o.to_nav().write()).unwrap();
        assert_eq!(n.one_way_links(), vec![(4, 3)]);
        assert!(apply(&o, &serde_json::from_value(json!({"op": "link", "a": 4, "b": 3, "oneWay": true})).unwrap(), None).is_err());
        // and back to both ways
        let both = run(&o, json!({"op": "link", "a": 4, "b": 3}));
        assert!(Nav::parse(&both.to_nav().write()).unwrap().one_way_links().is_empty());

        // unlink one direction, then both
        let u = run(&l, json!({"op": "unlink", "a": 3, "b": 4, "both": false}));
        assert_eq!(Nav::parse(&u.to_nav().write()).unwrap().one_way_links(), vec![(4, 3)]);
        let u = run(&l, json!({"op": "unlink", "a": 3, "b": 4}));
        assert_eq!(conns(&u, 3), conns(&d, 3));
        assert_eq!(conns(&u, 4), conns(&d, 4));
        assert!(apply(&u, &serde_json::from_value(json!({"op": "unlink", "a": 3, "b": 4})).unwrap(), None).is_err());
        assert!(apply(&d, &serde_json::from_value(json!({"op": "link", "a": 3, "b": 3})).unwrap(), None).is_err());
        assert!(apply(&d, &serde_json::from_value(json!({"op": "link", "a": 3, "b": 99})).unwrap(), None).unwrap_err().contains("no area 99"));
    }

    #[test]
    fn delete_takes_every_link_with_it() {
        let d = rooms();
        let x = run(&d, json!({"op": "delete", "ids": [2]}));
        let n = Nav::parse(&x.to_nav().write()).unwrap();
        assert_eq!(n.areas.iter().map(|a| a.id).collect::<Vec<_>>(), vec![1, 3, 4]);
        assert!(n.dangling_links().is_empty());
        assert!(n.areas.iter().all(|a| a.conns.iter().flatten().all(|&i| i != 2)));
        // area 1's encounter and approach named area 2 and its hiding spot: gone too
        assert!(n.areas[0].encounters.is_empty() && n.areas[0].approach.is_empty());
        // untouched areas are the same objects (undo steps share them)
        assert!(Arc::ptr_eq(&x.areas[2], &d.areas[3]));
        assert!(apply(&d, &serde_json::from_value(json!({"op": "delete", "ids": [77]})).unwrap(), None).is_err());
    }

    #[test]
    fn reshape_attr_and_place() {
        let d = rooms();
        let m = run(&d, json!({"op": "reshape", "id": 2, "x": [110, 210], "y": [10, 110], "snap": false}));
        let a = &m.areas[1];
        assert_eq!((a.lo, a.hi), ([110.0, 10.0, 0.0], [210.0, 110.0, 0.0]));
        assert_eq!(a.hiding[0].pos, [160.0, 60.0, 0.0], "moved with it");
        let r = apply(&d, &serde_json::from_value(json!({"op": "reshape", "id": 2, "x": [100, 120], "y": [0, 100], "snap": false})).unwrap(), None).unwrap();
        assert!(r.doc.areas[1].hiding.is_empty() && r.label == "Resize area 2" && !r.notes.is_empty());

        let c = run(&d, json!({"op": "attr", "ids": [1, 2], "set": nav::ATTR_CROUCH | nav::ATTR_JUMP}));
        assert_eq!(c.areas[0].attr, 3);
        let c = run(&c, json!({"op": "attr", "ids": [1], "clear": nav::ATTR_JUMP, "set": 0}));
        assert_eq!((c.areas[0].attr, c.areas[1].attr), (1, 3));
        assert_eq!(run(&d, json!({"op": "attr", "ids": [1], "set": 0xF0})).areas[0].attr, 0, "unknown bits ignored");

        let p = run(&d, json!({"op": "place", "ids": [3, 4], "name": "BombsiteB"}));
        let n = Nav::parse(&p.to_nav().write()).unwrap();
        assert_eq!(n.place_name(&n.areas[2]), "BombsiteB");
        assert_eq!(n.place_name(&n.areas[3]), "BombsiteB");
        assert_eq!(n.places.len(), 2);
        let p = run(&p, json!({"op": "place", "ids": [3], "name": "Mid"}));
        assert_eq!((p.areas[2].place, p.places.len()), (1, 2));
        assert_eq!(run(&p, json!({"op": "place", "ids": [3], "name": ""})).areas[2].place, 0);
        assert!(apply(&d, &serde_json::from_value(json!({"op": "place", "ids": [3], "name": "Bomb site"})).unwrap(), None).is_err());
    }

    /// A ledge at z 128 over x < 100 and a floor at 0 past it; `wall` puts a wall at its edge.
    struct Ledge {
        wall: bool,
    }
    impl Ground for Ledge {
        fn floor(&self, x: f32, _y: f32, z: f32) -> Option<f32> {
            let top = if x < 100.0 { 128.0 } else { 0.0 };
            (top <= z + SNAP_UP && top >= z - SNAP_DOWN).then_some(top)
        }
        fn fits(&self, _p: [f32; 3]) -> bool {
            true
        }
        fn step_off(&self, from: [f32; 3], to: [f32; 2]) -> Step {
            let top = if to[0] < 100.0 { 128.0 } else { 0.0 };
            if (self.wall && from[0] < 100.0 && to[0] >= 100.0) || top > from[2] + SNAP_UP {
                Step::Wall
            } else if from[2] - top > MAX_FALL {
                Step::Fall
            } else {
                Step::Land(top)
            }
        }
    }

    /// Bots get to areas no link leads to by stepping off edges; the check says so, and
    /// offers the link that's missing.
    #[test]
    fn falling_in_counts_as_getting_there() {
        let mut up = area(1, [0.0, 100.0], [0.0, 100.0], 128.0);
        let mut down = area(2, [100.0, 200.0], [0.0, 100.0], 0.0);
        // stairs back up from 2 to 1, but no drop from 1 to 2
        down.conns[3] = vec![1];
        up.conns = Default::default();
        let n = Nav { version: 5, bsp_size: 1, places: vec![], areas: vec![up, down] };
        let spawn = [50.0, 50.0, 128.0 + 36.0];
        // no map to go by: only links count
        assert_eq!(check(&n, &[spawn], &[], None).reach, vec![REACH_OK, REACH_NEVER]);
        // off the ledge: bots land in 2 and walk back up the stairs
        let c = check(&n, &[spawn], &[], Some(&Ledge { wall: false }));
        assert_eq!(c.reach, vec![REACH_OK, REACH_FALLS]);
        let p = c.problems.iter().find(|p| p.kind == "falls").expect("a falls problem");
        assert_eq!(p.text, "Bots only get to area 2 by falling in");
        let (label, op) = p.fix.clone().unwrap();
        assert_eq!(label, "Add a drop from 1 to 2");
        assert_eq!(op, json!({"op": "link", "a": 1, "b": 2, "oneWay": true}));
        // with that drop linked, it's an ordinary area
        let mut fixed = n.clone();
        fixed.areas[0].conns[1] = vec![2];
        assert_eq!(check(&fixed, &[spawn], &[], Some(&Ledge { wall: false })).reach, vec![REACH_OK, REACH_OK]);
        // without the stairs, a bot that falls in is stuck there
        let mut pit = n.clone();
        pit.areas[1].conns = Default::default();
        assert_eq!(check(&pit, &[spawn], &[], Some(&Ledge { wall: false })).reach, vec![REACH_OK, REACH_TRAP]);
        // a wall along the edge: nobody gets down
        assert_eq!(check(&n, &[spawn], &[], Some(&Ledge { wall: true })).reach, vec![REACH_OK, REACH_NEVER]);
        // floor the nav left out between two areas is walked across
        let a = area(1, [0.0, 50.0], [0.0, 100.0], 0.0);
        let mut b = area(2, [130.0, 200.0], [0.0, 100.0], 0.0);
        b.conns[3] = vec![1];
        let gap = Nav { version: 5, bsp_size: 1, places: vec![], areas: vec![a, b] };
        let flat = Ledge { wall: false };
        struct Flat<'a>(&'a Ledge);
        impl Ground for Flat<'_> {
            fn floor(&self, _x: f32, _y: f32, _z: f32) -> Option<f32> {
                Some(0.0)
            }
            fn fits(&self, p: [f32; 3]) -> bool {
                self.0.fits(p)
            }
            fn step_off(&self, _from: [f32; 3], _to: [f32; 2]) -> Step {
                Step::Land(0.0)
            }
        }
        let c = check(&gap, &[[25.0, 50.0, 36.0]], &[], Some(&Flat(&flat)));
        assert_eq!(c.reach, vec![REACH_OK, REACH_FALLS]);
        assert_eq!(c.problems[0].fix.as_ref().unwrap().0, "Link 1 and 2");
    }

    #[test]
    fn islands_and_traps() {
        let d = rooms();
        let c = check(&d.to_nav(), &[SPAWN], &[], None);
        assert_eq!(c.reach, vec![REACH_OK, REACH_OK, REACH_OK, REACH_NEVER]);
        assert_eq!(c.problems[0].kind, "unreachable");
        assert_eq!(c.problems[0].text, "Bots can't reach area 4 from any spawn");
        assert_eq!(c.spawn_areas, vec![0]);
        // a drop into 4 with no way back: a trap
        let o = run(&d, json!({"op": "link", "a": 3, "b": 4, "oneWay": true}));
        let c = check(&o.to_nav(), &[SPAWN], &[], None);
        assert_eq!(c.reach[3], REACH_TRAP);
        assert_eq!(c.problems[0].kind, "trap");
        // linked both ways: all fine
        let ok = run(&d, json!({"op": "link", "a": 3, "b": 4}));
        let c = check(&ok.to_nav(), &[SPAWN], &[], None);
        assert!(c.reach.iter().all(|&r| r == REACH_OK) && c.problems.is_empty(), "{:?}", c.problems);
        // no spawns: the biggest group counts, and it says so
        let c = check(&d.to_nav(), &[], &[], None);
        assert_eq!(c.reach, vec![REACH_OK, REACH_OK, REACH_OK, REACH_NEVER]);
        assert!(c.note.is_some());
        // two islands of 2 and 1 come as two problems, the bigger first
        let mut n = d.to_nav();
        n.areas.push(area(5, [600.0, 700.0], [0.0, 100.0], 0.0));
        n.areas[3].conns[1] = vec![5];
        n.areas[4].conns[3] = vec![4];
        n.areas.push(area(6, [900.0, 950.0], [0.0, 100.0], 0.0));
        let c = check(&n, &[SPAWN], &[], None);
        assert_eq!(c.problems.iter().filter(|p| p.kind == "unreachable").map(|p| p.areas.clone()).collect::<Vec<_>>(), vec![vec![4, 5], vec![6]]);
        assert_eq!(c.problems[0].text, "Bots can't reach 2 areas from any spawn: areas 4 and 5");
    }

    #[test]
    fn checks_find_the_rest() {
        let mut n = rooms().to_nav();
        n.areas[0].conns[0].push(77);
        n.areas[1].conns[0].push(2);
        n.areas.push(area(3, [700.0, 700.2], [0.0, 10.0], 0.0));
        let ladder = LadderEnds { bottom: [0.0, 0.0, 0.0], top: [0.0, 0.0, 128.0], at_bottom: vec![0], at_top: vec![] };
        let c = check(&n, &[SPAWN], &[ladder], Some(&Floor));
        let kinds: Vec<&str> = c.problems.iter().map(|p| p.kind).collect();
        for k in ["dangling", "self", "duplicate", "flat", "ladder"] {
            assert!(kinds.contains(&k), "{k} in {kinds:?}");
        }
        let fix = c.problems.iter().find(|p| p.kind == "dangling").unwrap().fix.clone().unwrap();
        let op: Op = serde_json::from_value(fix.1).unwrap();
        let fixed = apply(&Doc::from_nav(n.clone()), &op, None).unwrap().doc.to_nav();
        assert!(fixed.dangling_links().is_empty());
        // a player doesn't fit in area 4 (x >= 900 in Floor): unless it's a crouch area
        let mut n = rooms().to_nav();
        n.areas[3].lo[0] = 900.0;
        n.areas[3].hi[0] = 990.0;
        assert!(check(&n, &[SPAWN], &[], Some(&Floor)).problems.iter().any(|p| p.kind == "cramped"));
        n.areas[3].attr = nav::ATTR_CROUCH;
        assert!(!check(&n, &[SPAWN], &[], Some(&Floor)).problems.iter().any(|p| p.kind == "cramped"));
    }

    #[test]
    fn saving_keeps_a_backup_once_and_refuses_when_it_should() {
        let dir = std::env::temp_dir().join(format!("ls-navfix-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("cs_test.nav");
        let original = rooms().to_nav().write();
        std::fs::write(&p, &original).unwrap();
        let doc = read_doc(&p).unwrap();
        let mut s = Session { disk: cache::Stamp::of(&p).ok(), saved_gen: doc.gen, doc, nav_path: p.clone(), undo: vec![], redo: vec![], used: std::time::Instant::now() };
        s.doc = run(&s.doc, json!({"op": "link", "a": 3, "b": 4}));
        assert_eq!(save_session(&mut s, true).unwrap_err().msg, LOADED);
        let said = save_session(&mut s, false).unwrap();
        assert_eq!(said["backedUp"], true);
        assert_eq!(std::fs::read(dir.join("cs_test.nav.bak")).unwrap(), original);
        assert_eq!(Nav::parse(&std::fs::read(&p).unwrap()).unwrap(), s.doc.to_nav());
        assert!(!dir.join("cs_test.nav.part").exists());
        assert_eq!(s.doc.gen, s.saved_gen);
        // the second save keeps the first backup
        s.doc = run(&s.doc, json!({"op": "delete", "ids": [1]}));
        assert_eq!(save_session(&mut s, false).unwrap()["backedUp"], false);
        assert_eq!(std::fs::read(dir.join("cs_test.nav.bak")).unwrap(), original);
        // changed on disk behind its back: refused
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&p, &original).unwrap();
        s.doc = run(&s.doc, json!({"op": "delete", "ids": [2]}));
        assert_eq!(save_session(&mut s, false).unwrap_err().msg, CHANGED);
        // knowledge made for another nav is called stale
        let mut fake = vec![0u8; 20];
        fake[0..4].copy_from_slice(&lsv::LSV_MAGIC.to_le_bytes());
        fake[4..8].copy_from_slice(&lsv::LSV_VERSION.to_le_bytes());
        fake[16..20].copy_from_slice(&4u32.to_le_bytes());
        std::fs::write(dir.join("cs_test.lsv"), &fake).unwrap();
        s.disk = cache::Stamp::of(&p).ok();
        let said = save_session(&mut s, false).unwrap();
        assert_eq!(said["knowledgeStale"], true, "{said}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn direction_matches_navarea_py() {
        let a = area(1, [0.0, 10.0], [0.0, 50.0], 0.0);
        assert_eq!(direction(&a, &area(2, [0.0, 10.0], [-50.0, 0.0], 0.0)), 0);
        assert_eq!(direction(&a, &area(2, [0.0, 10.0], [50.0, 90.0], 0.0)), 2);
        assert_eq!(direction(&a, &area(2, [10.0, 40.0], [0.0, 50.0], 0.0)), 1);
        assert_eq!(direction(&a, &area(2, [-40.0, 0.0], [0.0, 50.0], 0.0)), 3);
        // overlapping: the axis they're further apart on, ties to north/south
        assert_eq!(direction(&a, &area(2, [-5.0, 5.0], [-5.0, 5.0], 0.0)), 0);
        assert_eq!(rebuild_seconds(737), 56);
        assert_eq!(about(rebuild_seconds(586)), "about a minute");
        assert_eq!(about(rebuild_seconds(4000)), "about 6 minutes");
    }

    /// The raw cs_rats2 nav the learner made (navpack/raw/cs_rats2.nav, gzipped), before the fix.
    fn rats2_raw() -> Doc {
        use std::io::Read;
        let mut b = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("testdata/cs_rats2_raw.nav.gz")[..]).read_to_end(&mut b).unwrap();
        Doc::from_nav(Nav::parse(&b).unwrap())
    }

    /// sha256 of navpack/fixes/cs_rats2.nav, the fix made by hand with navarea.py.
    const RATS2_FIXED_SHA: &str = "707694c7a54c024aae18aebf831a2840013195accca443bee17b9c9324dc9a0e";

    /// The cs_rats2 fix (navpack/fixes/cs_rats2.json, made with navarea.py) as edits, the way
    /// the page sends them: the beam areas, their links and the one-way drop.
    fn rats2_fix(raw: &Doc, ground: Option<&dyn Ground>) -> Doc {
        let spec: Value = serde_json::from_slice(include_bytes!("testdata/cs_rats2_fix.json")).unwrap();
        let mut d = raw.clone();
        let first = d.areas.iter().map(|a| a.id).max().unwrap() + 1;
        let specs = spec["areas"].as_array().unwrap();
        for a in specs {
            let op = json!({"op": "add", "x": a["x"], "y": a["y"], "z": a["z"], "snap": ground.is_some()});
            d = apply(&d, &serde_json::from_value(op).unwrap(), ground).unwrap().doc;
        }
        let id = |v: &Value| v.as_u64().map(|n| n as u32).unwrap_or_else(|| first + v.as_str().unwrap()[3..].parse::<u32>().unwrap());
        for (i, a) in specs.iter().enumerate() {
            let me = first + i as u32;
            for v in a["links"].as_array().unwrap() {
                d = run(&d, json!({"op": "link", "a": me, "b": id(v)}));
            }
            for v in a.get("drops_to").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
                d = run(&d, json!({"op": "link", "a": me, "b": id(&v), "oneWay": true}));
            }
        }
        d
    }

    /// The fix made with these edits on the raw nav is the hand-made file, byte for byte.
    #[test]
    fn rats2_fix_matches_the_hand_made_one() {
        use sha2::{Digest, Sha256};
        let raw = rats2_raw();
        assert_eq!(raw.areas.len(), 574);
        let d = rats2_fix(&raw, None);
        let bytes = d.to_nav().write();
        let sha: String = Sha256::digest(&bytes).iter().map(|x| format!("{x:02x}")).collect();
        assert_eq!(sha, RATS2_FIXED_SHA, "the edits should give navarea.py's file exactly");
        let n = Nav::parse(&bytes).unwrap();
        assert_eq!(n.areas.len(), 586);
        assert_eq!(n.one_way_links().into_iter().filter(|(a, _)| *a >= 1144).collect::<Vec<_>>(), vec![(1155, 2)]);
        assert!(n.dangling_links().is_empty());
        // undone (deleted again): back to the raw nav exactly
        let back = run(&d, json!({"op": "delete", "ids": (1144..=1155).collect::<Vec<u32>>()}));
        assert_eq!(back.to_nav().write(), raw.to_nav().write());
        // the beams without the ladder between them: the far one is reached only through the
        // drop's start, so it's an island with the near one's links in place
        let c = check(&n, &[[-900.0, 600.0, 292.0]], &[], None);
        let at = |id: u32| c.reach[n.index_of()[&id]];
        assert_eq!(at(1144), at(274));
        assert_eq!(at(1150), REACH_NEVER);
    }

    /// LS_TEST_SIM=<loner-strike-sim>: the same against the files there.
    #[test]
    fn real_rats2_fix() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let sim = PathBuf::from(sim);
        let raw = read_doc(&sim.join("navpack/raw/cs_rats2.nav")).unwrap();
        assert_eq!(raw.to_nav().write(), rats2_raw().to_nav().write(), "the fixture is the raw nav");
        let fixed = std::fs::read(sim.join("navpack/fixes/cs_rats2.nav")).unwrap();
        assert_eq!(rats2_fix(&raw, None).to_nav().write(), fixed);
    }

    /// LS_TEST_SIM + LS_TEST_GAME (a Half-Life folder with cs_rats2.bsp): drawn on the real map
    /// with the floor probe, the new areas land on the beams (z 256 and 448) and check out.
    #[test]
    fn real_rats2_on_the_map() {
        let (Ok(sim), Ok(hl)) = (std::env::var("LS_TEST_SIM"), std::env::var("LS_TEST_GAME")) else { return };
        let bsp = crate::mapcopy::Finder::default().find(&PathBuf::from(&hl).join("cstrike/maps"), "cs_rats2.bsp").expect("cs_rats2.bsp");
        let hulls = MapHulls::parse(&std::fs::read(&bsp).unwrap()).unwrap();
        let sim = PathBuf::from(sim);
        let raw = rats2_raw();
        let want = Nav::parse(&std::fs::read(sim.join("navpack/fixes/cs_rats2.nav")).unwrap()).unwrap();
        let d = rats2_fix(&raw, Some(&hulls));
        let n = d.to_nav();
        assert_eq!(n.areas.len(), want.areas.len());
        for (a, b) in n.areas.iter().zip(&want.areas).skip(raw.areas.len()) {
            assert_eq!((a.id, a.conns.clone()), (b.id, b.conns.clone()));
            assert_eq!((a.lo[0], a.lo[1], a.hi[0], a.hi[1]), (b.lo[0], b.lo[1], b.hi[0], b.hi[1]));
            for (z, w) in [a.lo[2], a.ne_z, a.hi[2], a.sw_z].iter().zip([b.lo[2], b.ne_z, b.hi[2], b.sw_z]) {
                assert!((z - w).abs() <= 1.0, "area {}: floor {z}, the hand fix has {w}", a.id);
            }
        }
        let ents = goldsrc::entities::parse(&crate::rcon::latin1(&bsp::read_lumps(&bsp, &[bsp::LUMP_ENTITIES]).unwrap()[0]));
        let spawns: Vec<[f32; 3]> = ents.iter().filter(|e| matches!(e.classname(), "info_player_start" | "info_player_deathmatch")).filter_map(|e| e.origin()).collect();
        let ends = |n: &Nav| ladder_ends(&nav::build_ladders(n, &hulls.ladders, &hulls), &hulls.ladders);
        let before = check(&raw.to_nav(), &spawns, &ends(&raw.to_nav()), Some(&hulls));
        let after = check(&n, &spawns, &ends(&n), Some(&hulls));
        let new_reach: Vec<u8> = after.reach[raw.areas.len()..].to_vec();
        let count = |c: &Checked, r: u8| c.reach.iter().filter(|&&x| x == r).count();
        println!("raw: {} unreachable, {} trapped; {} problems {:?}", count(&before, REACH_NEVER), count(&before, REACH_TRAP), before.problems.len(), before.problems.iter().map(|p| &p.text).take(12).collect::<Vec<_>>());
        println!("fixed: {} unreachable, {} trapped; {} problems; new areas reach {new_reach:?}", count(&after, REACH_NEVER), count(&after, REACH_TRAP), after.problems.len());
        // the first beam (1144-1147) hangs off area 274; the second (1148-1155) only drops
        // into area 2, so it's an island unless something leads up to it
        assert!(new_reach[..4].iter().all(|&r| r == REACH_OK), "{new_reach:?}");
        assert!(!after.problems.iter().any(|p| p.kind == "cramped" && p.areas.iter().any(|&id| id >= 1144)), "a player fits on the beams");
    }

    /// LS_TEST_SIM + LS_TEST_GAME: the reach check against where bots really went. Every area
    /// a bot stood in during the sim's rats2_v5 recordings (5 matches of 12 bots) has to be
    /// one the check says bots get to. Before the step-off model, 120 areas of the raw nav were
    /// "unreachable" and bots stood in 39 of them: they fall off the beams and shelves.
    #[test]
    fn real_rats2_reach_matches_the_recordings() {
        let (Ok(sim), Ok(hl)) = (std::env::var("LS_TEST_SIM"), std::env::var("LS_TEST_GAME")) else { return };
        let bsp = crate::mapcopy::Finder::default().find(&PathBuf::from(&hl).join("cstrike/maps"), "cs_rats2.bsp").expect("cs_rats2.bsp");
        let hulls = MapHulls::parse(&std::fs::read(&bsp).unwrap()).unwrap();
        let ents = goldsrc::entities::parse(&crate::rcon::latin1(&bsp::read_lumps(&bsp, &[bsp::LUMP_ENTITIES]).unwrap()[0]));
        let spawns: Vec<[f32; 3]> = ents.iter().filter(|e| matches!(e.classname(), "info_player_start" | "info_player_deathmatch")).filter_map(|e| e.origin()).collect();
        // where every bot stood: S rows (x y z of the player's origin)
        let mut stood: Vec<[f32; 3]> = Vec::new();
        for e in std::fs::read_dir(PathBuf::from(&sim).join("results/rats2_v5")).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with('W') || !name.ends_with(".csv") {
                continue;
            }
            for line in std::fs::read_to_string(e.path()).unwrap().lines() {
                let f: Vec<&str> = line.split(',').collect();
                if f.len() >= 7 && f[0] == "S" {
                    if let (Ok(x), Ok(y), Ok(z)) = (f[4].parse(), f[5].parse(), f[6].parse()) {
                        stood.push([x, y, z]);
                    }
                }
            }
        }
        assert!(stood.len() > 100_000, "{} samples", stood.len());
        for which in ["raw", "fixes"] {
            let n = Nav::parse(&std::fs::read(PathBuf::from(&sim).join(format!("navpack/{which}/cs_rats2.nav"))).unwrap()).unwrap();
            let ends = ladder_ends(&nav::build_ladders(&n, &hulls.ladders, &hulls), &hulls.ladders);
            let t = std::time::Instant::now();
            let c = check(&n, &spawns, &ends, Some(&hulls));
            let took = t.elapsed();
            let mut visits = vec![0u32; n.areas.len()];
            for p in &stood {
                if let Some(a) = n.area_at(*p, 120.0) {
                    visits[a] += 1;
                }
            }
            let count = |r: u8| c.reach.iter().filter(|&&x| x == r).count();
            let missed: Vec<(u32, u32)> = (0..n.areas.len()).filter(|&i| c.reach[i] == REACH_NEVER && visits[i] > 0).map(|i| (n.areas[i].id, visits[i])).collect();
            println!(
                "{which}: {} areas: {} can't be reached, {} only by falling in, {} can't get back; bots stood in {} areas; check took {took:?}; problems {:?}",
                n.areas.len(), count(REACH_NEVER), count(REACH_FALLS), count(REACH_TRAP), visits.iter().filter(|&&v| v > 0).count(),
                c.problems.iter().map(|p| p.text.as_str()).collect::<Vec<_>>()
            );
            assert!(missed.is_empty(), "{which}: bots stood in areas the check says nobody reaches (id, samples): {missed:?}");
            assert!(count(REACH_NEVER) <= 10, "{which}: {} unreachable", count(REACH_NEVER));
        }
    }

    /// The routes over a game folder with the fixture room and a nav: open, edit, undo, redo,
    /// reload; edits only from the app's own pages; bad edits answer in words.
    #[tokio::test]
    async fn routes_edit_and_undo() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;
        let root = std::env::temp_dir().join(format!("ls-navfix-routes-{}", std::process::id()));
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::write(hl.join("cstrike/maps/Room.bsp"), crate::goldsrc::bsp::fixture::build()).unwrap();
        std::fs::write(hl.join("cstrike/maps/Room.nav"), rooms().to_nav().write()).unwrap();
        let g = Arc::new(crate::game::Game::with_app_dir(&hl, "cstrike", &root.join("app")));
        let st: Shared = Arc::new(crate::server::AppState {
            current: std::sync::RwLock::new(Some(g)),
            hosts: std::sync::RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: std::sync::RwLock::new(vec![]),
            token: std::sync::RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let call = |method: &str, uri: &str, body: Value, panel: bool| {
            let mut req = Request::builder().method(method).uri(uri).header("host", "127.0.0.1:8027").header("content-type", "application/json");
            if panel {
                req = req.header("x-panel", "1");
            }
            let req = req.body(Body::from(body.to_string())).unwrap();
            let st = st.clone();
            async move {
                let res = routes().with_state(st).oneshot(req).await.unwrap();
                let status = res.status();
                let b = axum::body::to_bytes(res.into_body(), 64 << 20).await.unwrap();
                (status, serde_json::from_slice::<Value>(&b).unwrap())
            }
        };
        let (_, v) = call("GET", "/api/tools/navfix/room", json!({}), false).await;
        assert_eq!(v["nav"]["areas"].as_array().unwrap().len(), 4, "{v}");
        assert_eq!((v["dirty"].clone(), v["undo"].clone()), (json!(false), Value::Null));
        assert_eq!(v["reach"].as_array().unwrap().len(), 4);
        assert!(v["rebuild"]["seconds"].as_u64().unwrap() > 0);

        let add = json!({"op": "add", "x": [10, 60], "y": [10, 60], "z": 0, "snap": false});
        let (s, v) = call("POST", "/api/tools/navfix/room/edit", add.clone(), false).await;
        assert_eq!((s, v["error"].as_str()), (StatusCode::OK, Some(NOT_PANEL)));
        let (_, v) = call("POST", "/api/tools/navfix/room/edit", add, true).await;
        assert_eq!(v["newId"], 5, "{v}");
        assert_eq!((v["dirty"].clone(), v["undo"].clone(), v["did"].clone()), (json!(true), json!("Add area 5"), json!("Add area 5")));
        assert_eq!(v["nav"]["areas"].as_array().unwrap().len(), 5);
        let (_, v) = call("POST", "/api/tools/navfix/room/edit", json!({"op": "link", "a": 5, "b": 5}), true).await;
        assert!(v["error"].as_str().unwrap().contains("two different"));
        let (s, v) = call("POST", "/api/tools/navfix/room/edit", json!({"op": "explode"}), true).await;
        assert_eq!((s, v["error"].is_string()), (StatusCode::BAD_REQUEST, true));

        let (_, v) = call("POST", "/api/tools/navfix/room/undo", json!({}), true).await;
        assert_eq!((v["nav"]["areas"].as_array().unwrap().len(), v["dirty"].clone(), v["redo"].clone()), (4, json!(false), json!("Add area 5")));
        let (_, v) = call("POST", "/api/tools/navfix/room/undo", json!({}), true).await;
        assert_eq!(v["error"], "Nothing to undo.");
        let (_, v) = call("POST", "/api/tools/navfix/room/redo", json!({}), true).await;
        assert_eq!((v["nav"]["areas"].as_array().unwrap().len(), v["dirty"].clone()), (5, json!(true)));
        // the session is kept between requests; reload drops it
        let (_, v) = call("GET", "/api/tools/navfix/room", json!({}), false).await;
        assert_eq!(v["nav"]["areas"].as_array().unwrap().len(), 5);
        let (_, v) = call("POST", "/api/tools/navfix/room/reload", json!({}), true).await;
        assert_eq!((v["nav"]["areas"].as_array().unwrap().len(), v["dirty"].clone()), (4, json!(false)));
        let (_, v) = call("GET", "/api/tools/navfix/de_nothere", json!({}), false).await;
        assert!(v["error"].is_string());
        std::fs::remove_dir_all(&root).ok();
    }
}
