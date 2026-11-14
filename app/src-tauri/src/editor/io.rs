//! Entity I/O (agent P5, Hammer parity): what triggers what. GoldSrc entities talk by name: an
//! entity's `target` (and `killtarget`, `pathtarget`, a beam's ends, trigger_changetarget's new
//! target…) names others' `targetname`; a `master` names the multisource (or another master)
//! that must be on before it works; a multi_manager fires every name in its own keys, each after
//! its delay (Hammer adds "#1", "#2" to fire one name twice). The graph here is the one source for
//! the page's I/O panel and arrows, the live check for broken links, and MCP editor_io.

use super::doc::{EntityId, MapDocument};
use std::collections::{BTreeMap, BTreeSet};

/// Keys that name another entity, and what the link does.
pub const LINK_KEYS: &[(&str, Kind)] = &[
    ("target", Kind::Fires),
    ("killtarget", Kind::Kills),
    ("master", Kind::Master),
    ("pathtarget", Kind::Fires),
    ("m_iszNewTarget", Kind::Names),
    ("LightningStart", Kind::Names),
    ("LightningEnd", Kind::Names),
    ("LaserTarget", Kind::Names),
    ("combattarget", Kind::Fires),
];

/// multi_manager keys that are its own settings, not names to fire.
const MM_OWN: &[&str] = &["classname", "targetname", "origin", "spawnflags", "wait", "master", "angles", "angle"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// sets it off (target, pathtarget, a multi_manager's names)
    Fires,
    /// removes it (killtarget)
    Kills,
    /// it only works while this is on (master)
    Master,
    /// points at it (a beam's ends, trigger_changetarget's new target)
    Names,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Fires => "fires",
            Kind::Kills => "kills",
            Kind::Master => "master",
            Kind::Names => "names",
        }
    }
}

/// One link: `from` names `name` through `key` (a multi_manager's own key is the name).
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub from: EntityId,
    pub key: String,
    pub name: String,
    pub kind: Kind,
    /// a multi_manager's delay in seconds
    pub delay: Option<f64>,
    /// the entities named that (empty: broken)
    pub to: Vec<EntityId>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub links: Vec<Link>,
    /// targetname -> entities with it
    pub names: BTreeMap<String, Vec<EntityId>>,
}

impl Graph {
    pub fn broken(&self) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(|l| l.to.is_empty())
    }

    /// Everything an entity sets off, directly or down the line (not itself), and everything
    /// that leads to it: its chain, for highlighting.
    pub fn chain(&self, id: EntityId) -> (BTreeSet<EntityId>, BTreeSet<EntityId>) {
        let walk = |down: bool| {
            let mut seen = BTreeSet::new();
            let mut todo = vec![id];
            while let Some(x) = todo.pop() {
                for l in &self.links {
                    let next: Vec<EntityId> = if down {
                        if l.from == x { l.to.clone() } else { vec![] }
                    } else if l.to.contains(&x) {
                        vec![l.from]
                    } else {
                        vec![]
                    };
                    for n in next {
                        if n != id && seen.insert(n) {
                            todo.push(n);
                        }
                    }
                }
            }
            seen
        };
        (walk(true), walk(false))
    }
}

/// "door#2" fires "door" (Hammer's way to fire one name twice from a multi_manager).
fn mm_name(key: &str) -> &str {
    match key.rsplit_once('#') {
        Some((n, k)) if !n.is_empty() && k.chars().all(|c| c.is_ascii_digit()) => n,
        _ => key,
    }
}

pub fn graph(doc: &MapDocument) -> Graph {
    let mut g = Graph::default();
    for (id, e) in &doc.entities {
        if let Some(n) = e.get("targetname").filter(|n| !n.is_empty()) {
            g.names.entry(n.to_string()).or_default().push(*id);
        }
    }
    for (id, e) in &doc.entities {
        let mut add = |key: &str, name: &str, kind: Kind, delay: Option<f64>| {
            let name = name.trim();
            if name.is_empty() {
                return;
            }
            let to = g.names.get(name).cloned().unwrap_or_default();
            g.links.push(Link { from: *id, key: key.to_string(), name: name.to_string(), kind, delay, to });
        };
        for (k, kind) in LINK_KEYS {
            if let Some(v) = e.get(k) {
                add(k, v, *kind, None);
            }
        }
        // an NPC's patrol stops (NPC-editor): names it walks between
        if e.classname() == "ls_npc" {
            for w in e.get("targets").unwrap_or("").split_whitespace() {
                add("targets", w, Kind::Names, None);
            }
        }
        if e.classname() == "multi_manager" {
            for (k, v) in &e.keys {
                if MM_OWN.contains(&k.as_str()) || LINK_KEYS.iter().any(|(l, _)| l == k) {
                    continue;
                }
                add(k, mm_name(k), Kind::Fires, v.trim().parse::<f64>().ok());
            }
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;

    fn ent(d: &mut MapDocument, id: u64, keys: &[(&str, &str)]) {
        let mut e = Entity::new(keys[0].1);
        for (k, v) in &keys[1..] {
            e.set(k, v);
        }
        d.entities.insert(EntityId(id), e);
    }

    #[test]
    fn a_deathrun_trap_chain() {
        let mut d = MapDocument::default();
        ent(&mut d, 1, &[("classname", "func_button"), ("target", "trap1_mm"), ("master", "trap_lock")]);
        ent(&mut d, 2, &[("classname", "multi_manager"), ("targetname", "trap1_mm"), ("trap1_door", "0"), ("trap1_hurt", "0.5"), ("trap1_hurt#1", "3"), ("trap1_sound", "0")]);
        ent(&mut d, 3, &[("classname", "func_door"), ("targetname", "trap1_door")]);
        ent(&mut d, 4, &[("classname", "trigger_hurt"), ("targetname", "trap1_hurt"), ("killtarget", "trap1_door")]);
        ent(&mut d, 5, &[("classname", "multisource"), ("targetname", "trap_lock")]);
        let g = graph(&d);
        // the button fires the manager and waits on the lock
        assert!(g.links.iter().any(|l| l.from == EntityId(1) && l.kind == Kind::Fires && l.to == vec![EntityId(2)]));
        assert!(g.links.iter().any(|l| l.from == EntityId(1) && l.kind == Kind::Master && l.to == vec![EntityId(5)]));
        // the manager fans out with delays; "#1" fires the same name again
        let mm: Vec<_> = g.links.iter().filter(|l| l.from == EntityId(2)).collect();
        assert_eq!(mm.len(), 4);
        assert!(mm.iter().any(|l| l.key == "trap1_hurt#1" && l.name == "trap1_hurt" && l.delay == Some(3.0) && l.to == vec![EntityId(4)]));
        // trap1_sound is named by nothing: broken
        let broken: Vec<_> = g.broken().map(|l| (l.from, l.name.clone())).collect();
        assert_eq!(broken, vec![(EntityId(2), "trap1_sound".to_string())]);
        // the chain from the button: the manager, the door, the hurt; up from the door: the hurt (kills it), the manager, the button
        let (down, _) = g.chain(EntityId(1));
        assert_eq!(down, [2, 3, 4, 5].into_iter().map(EntityId).collect());
        let (_, up) = g.chain(EntityId(3));
        assert_eq!(up, [1, 2, 4].into_iter().map(EntityId).collect());
        assert_eq!(mm_name("door#12"), "door");
        assert_eq!(mm_name("#3"), "#3");
        assert_eq!(mm_name("a#b"), "a#b");
    }

    /// NPC-editor: NPCs and interactables in plain map I/O.
    #[test]
    fn npcs_and_interactables_fire_their_targets() {
        let mut d = MapDocument::default();
        ent(&mut d, 1, &[("classname", "ls_interactable"), ("targetname", "lever"), ("target", "gate")]);
        ent(&mut d, 2, &[("classname", "ls_npc"), ("name", "Joe"), ("target", "alarm"), ("targets", "p1 p2")]);
        ent(&mut d, 3, &[("classname", "func_door"), ("targetname", "gate")]);
        ent(&mut d, 4, &[("classname", "ambient_generic"), ("targetname", "alarm")]);
        ent(&mut d, 5, &[("classname", "info_target"), ("targetname", "p1")]);
        ent(&mut d, 6, &[("classname", "info_target"), ("targetname", "p2")]);
        let g = graph(&d);
        assert!(g.links.iter().any(|l| l.from == EntityId(1) && l.kind == Kind::Fires && l.to == vec![EntityId(3)]));
        assert!(g.links.iter().any(|l| l.from == EntityId(2) && l.kind == Kind::Fires && l.to == vec![EntityId(4)]));
        let patrol: Vec<_> = g.links.iter().filter(|l| l.from == EntityId(2) && l.key == "targets").map(|l| (l.kind, l.to.clone())).collect();
        assert_eq!(patrol, vec![(Kind::Names, vec![EntityId(5)]), (Kind::Names, vec![EntityId(6)])]);
        assert!(g.broken().next().is_none());
    }
}
