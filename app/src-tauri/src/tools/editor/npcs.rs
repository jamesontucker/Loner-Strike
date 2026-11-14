//! NPC-editor: the map editor's Checks for NPCs and interactables. A missing model file is the
//! asset check's (missing-file); here: an NPC nobody can tell by name, and a point
//! ls_interactable with nothing drawn (players can't see what to use).

use crate::editor::checks::{issue, Issue, Level};
use crate::editor::doc::MapDocument;

pub fn issues(doc: &MapDocument) -> Vec<Issue> {
    let mut out = Vec::new();
    for (id, e) in &doc.entities {
        let empty = |k: &str| e.get(k).map(str::trim).unwrap_or("").is_empty();
        match e.classname() {
            "ls_npc" if empty("name") && empty("targetname") => {
                out.push(Issue { entity: Some(id.0), ..issue(Level::Warning, "npc-no-name", "An NPC has no name: give it a Label (what players see) or a Name.".into()) });
            }
            "ls_interactable" if empty("model") => {
                out.push(Issue { entity: Some(id.0), ..issue(Level::Info, "interactable-no-model", "An interactable has no model, so players see nothing to use there. Pick a model, or use a brush (ls_interactable_brush).".into()) });
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::doc::Entity;

    #[test]
    fn npcs_without_a_name_and_invisible_interactables_are_flagged() {
        let mut d = MapDocument::default();
        let add = |d: &mut MapDocument, keys: &[(&str, &str)]| {
            let mut e = Entity::new(keys[0].1);
            for (k, v) in &keys[1..] {
                e.set(k, v);
            }
            d.insert_entity(e);
        };
        add(&mut d, &[("classname", "ls_npc"), ("model", "models/barney.mdl")]);
        add(&mut d, &[("classname", "ls_npc"), ("name", "Old Joe")]);
        add(&mut d, &[("classname", "ls_npc"), ("targetname", "joe")]);
        add(&mut d, &[("classname", "ls_interactable"), ("prompt", "Press E to read")]);
        add(&mut d, &[("classname", "ls_interactable"), ("model", "models/w_battery.mdl")]);
        let found = issues(&d);
        let codes: Vec<&str> = found.iter().map(|i| i.code).collect();
        assert_eq!(codes.iter().filter(|c| **c == "npc-no-name").count(), 1, "{codes:?}");
        assert_eq!(codes.iter().filter(|c| **c == "interactable-no-model").count(), 1, "{codes:?}");
        assert!(found.iter().all(|i| i.entity.is_some()));
    }
}
