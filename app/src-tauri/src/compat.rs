//! Compatibility across 1.x (docs/notes/Compat.md): every file Loner-Strike saves or shares carries
//! a format version (a missing one is the 1.0 format, so files made before versions were written stay
//! valid), and a file from a newer format is refused whole, never half-loaded, with the app version
//! that made it so the person knows what to update to. The 1.0 samples in tests/compat/1.0 are
//! permanent: they must keep loading (and the 1.0 plugin API keep existing) in every 1.x.

/// This app's version, written into what it saves (`saved_by` / `app`).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A version string as a file may carry it (digits, dots, letters, - and +; short), else None.
pub fn clean_version(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|v| !v.is_empty() && v.len() <= 24 && v.bytes().all(|c| c.is_ascii_alphanumeric() || b".-+".contains(&c)))
}

/// The message for a file a newer Loner-Strike made: what it is, and the version to update to (the
/// one that saved it, when the file says).
pub fn newer(what: &str, saved_by: Option<&str>) -> String {
    match clean_version(saved_by) {
        Some(v) => format!("This {what} was made by Loner-Strike {v}, which is newer than this app ({APP_VERSION}). Update Loner-Strike to {v} or newer to open it."),
        None => format!("This {what} was made by a newer Loner-Strike than this one ({APP_VERSION}). Update Loner-Strike to the newest version to open it."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frozen 1.0 samples (tests/compat/1.0): read, never changed.
    fn fixture(rel: &str) -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/compat/1.0/").to_string() + rel).unwrap_or_else(|e| panic!("tests/compat/1.0/{rel}: {e}"))
    }

    #[test]
    fn compat_newer_messages() {
        let m = newer("map project", Some("1.4.2"));
        assert!(m.contains("Loner-Strike 1.4.2") && m.contains("Update Loner-Strike to 1.4.2 or newer"), "{m}");
        let m = newer("map project", Some("1.4\"; rm -rf"));
        assert!(m.contains("the newest version") && !m.contains("rm"), "{m}");
        assert!(newer("prefab", None).contains("the newest version"));
    }

    // ---- 1. map editor projects ---------------------------------------------------------------

    #[test]
    fn compat_1_0_map_project_loads() {
        let doc = crate::editor::doc::MapDocument::from_json(&fixture("editor/project.json")).expect("the 1.0 project opens");
        assert_eq!(doc.brushes.len(), 2);
        assert_eq!(doc.entities.len(), 2);
        assert_eq!(doc.groups.len(), 1);
        assert!(doc.entities.values().any(|e| e.classname() == "info_player_start"));
        assert_eq!(doc.world.wad_names(), vec!["halflife.wad", "cstrike.wad"]);
        // and it saves again in the current format, with who saved it
        let again: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        assert_eq!(again["version"], crate::editor::doc::DOC_VERSION);
        assert_eq!(again["saved_by"], APP_VERSION);
    }

    #[test]
    fn compat_newer_map_project_is_refused_whole() {
        let mut v: serde_json::Value = serde_json::from_str(&fixture("editor/project.json")).unwrap();
        v["version"] = serde_json::json!(crate::editor::doc::DOC_VERSION + 1);
        v["saved_by"] = serde_json::json!("1.7.0");
        let e = crate::editor::doc::MapDocument::from_json(&v.to_string()).unwrap_err();
        assert!(e.contains("Update Loner-Strike to 1.7.0 or newer"), "{e}");
        // a project from before saved_by was written: still refused, with the general advice
        v.as_object_mut().unwrap().remove("saved_by");
        assert!(crate::editor::doc::MapDocument::from_json(&v.to_string()).unwrap_err().contains("newest version"));
    }

    // ---- 2. shared creations ------------------------------------------------------------------

    #[test]
    fn compat_1_0_community_index_reads() {
        let idx = crate::tools::community::parse_index(fixture("community/index.json").as_bytes()).expect("the 1.0 list reads");
        assert_eq!(idx.items.len(), 3, "a map, a mode and a creature");
        let mut v: serde_json::Value = serde_json::from_str(&fixture("community/index.json")).unwrap();
        v["schema"] = serde_json::json!(crate::tools::community::SCHEMA + 1);
        let e = crate::tools::community::parse_index(v.to_string().as_bytes()).unwrap_err();
        assert!(e.contains("newer") && e.contains("Update Loner-Strike"), "{e}");
    }

    #[test]
    fn compat_1_0_mode_kit_installs() {
        let kit: serde_json::Value = serde_json::from_str(&fixture("community/mode-kit.json")).unwrap();
        let dir = crate::sys::test_root().join(format!("compat-kit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = crate::tools::studio::kit::install_kit_in(&dir, &kit).expect("the 1.0 kit installs");
        assert!(p.is_file());
        let mut newer_kit = kit.clone();
        newer_kit["version"] = serde_json::json!(99);
        newer_kit["app"] = serde_json::json!("1.9.0");
        let e = crate::tools::studio::kit::install_kit_in(&dir, &newer_kit).unwrap_err();
        assert!(e.contains("1.9.0"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compat_1_0_creature_pack_installs() {
        let text = fixture("community/creatures.ini");
        let p = crate::tools::creaturepack::parse_pack(&text).expect("the 1.0 pack reads");
        assert_eq!(p.types, ["gb123.ghoul"]);
        let game = crate::sys::test_root().join(format!("compat-cp-{}", std::process::id())).join("cstrike");
        let _ = std::fs::remove_dir_all(game.parent().unwrap());
        std::fs::create_dir_all(game.join("addons/amxmodx/configs")).unwrap();
        crate::tools::creaturepack::install_pack(&game, "Mod-1234", &text, None, "gb123").expect("the 1.0 pack installs");
        let ini = std::fs::read_to_string(game.join(crate::tools::creatures::FILE_REL)).unwrap();
        assert!(ini.contains("[gb123.ghoul]") && ini.contains("hp = 150"), "{ini}");
        // a pack in a newer format: refused, nothing written
        let newer_pack = text.replacen("; format: 1", "; format: 2 (Loner-Strike 1.6.0)", 1);
        let e = crate::tools::creaturepack::parse_pack(&newer_pack).unwrap_err();
        assert!(e.contains("1.6.0"), "{e}");
        let _ = std::fs::remove_dir_all(game.parent().unwrap());
    }

    // ---- 3. compiled modes and the server side ------------------------------------------------

    /// The natives and forwards a plugin compiled with 1.0's includes may call: every one is
    /// still declared, with the same signature (a removed one stays, deprecated).
    #[test]
    fn compat_1_0_plugin_api_still_exists() {
        let now = plugin_api();
        let frozen = fixture("server/plugin_api.txt");
        let missing: Vec<&str> = frozen.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).filter(|l| !now.contains(&l.to_string())).collect();
        assert!(missing.is_empty(), "the 1.0 plugin API changed (names and signatures never change; add, don't edit):\n{}", missing.join("\n"));
    }

    /// `native ...;` and `forward ...;` declarations of loner.inc and ls_components.inc, one per
    /// line, whitespace squeezed.
    pub(crate) fn plugin_api() -> Vec<String> {
        let inc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/scripting/include/");
        let mut out = Vec::new();
        for f in ["loner.inc", "ls_components.inc"] {
            let text = std::fs::read_to_string(format!("{inc}{f}")).unwrap();
            let mut cur: Option<String> = None;
            for line in text.lines() {
                let l = line.split("//").next().unwrap_or("").trim();
                if cur.is_none() && (l.starts_with("native ") || l.starts_with("forward ")) {
                    cur = Some(String::new());
                }
                if let Some(c) = cur.as_mut() {
                    c.push(' ');
                    c.push_str(l);
                    if l.ends_with(';') || l.ends_with(')') {
                        let sig = c.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(';').to_string();
                        out.push(format!("{f}: {sig}"));
                        cur = None;
                    }
                }
            }
        }
        out
    }

    /// Wrote the 1.0 map project sample (run once for 1.0; never again: the sample is frozen).
    #[test]
    #[ignore]
    fn compat_write_fixtures() {
        if std::env::var_os("LS_COMPAT_SNAPSHOT").is_none() {
            return;
        }
        use crate::editor::doc::{Entity, Group, MapDocument};
        use crate::editor::math::v3;
        let mut d = MapDocument::new();
        d.world.set_wad_key("\\half-life\\valve\\halflife.wad;\\half-life\\cstrike\\cstrike.wad");
        d.world.keys.push(("skyname".into(), "desert".into()));
        let g = d.insert_group(Group { name: "Crates".into(), ..Default::default() });
        d.insert_brush(crate::editor::ops::make_box(v3(-256.0, -256.0, -16.0), v3(256.0, 256.0, 0.0), "CRETE2_FLR"));
        let door = d.insert_entity({
            let mut e = Entity::new("func_door");
            e.set("speed", "100");
            e
        });
        let mut b = crate::editor::ops::make_box(v3(0.0, 0.0, 0.0), v3(64.0, 8.0, 112.0), "DOOR1");
        b.entity = Some(door);
        b.group = Some(g);
        d.insert_brush(b);
        let mut e = Entity::new("info_player_start");
        e.set("origin", "0 0 36");
        e.set("angles", "0 90 0");
        d.insert_entity(e);
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/compat/1.0/editor/project.json"), d.to_json()).unwrap();
        // a mode map kit as the Studio shares it (from the markers sample graph)
        let g = crate::tools::studio::graph::parse(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tools/studio/testdata/markers.graph.json")).unwrap()).unwrap();
        let mut kit = crate::tools::studio::kit::kit_of(&g);
        kit["mode"]["uid"] = serde_json::json!("6f1c2d3e-4a5b-4c6d-8e7f-001122334455");
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/compat/1.0/community/mode-kit.json"), serde_json::to_string_pretty(&kit).unwrap()).unwrap();
    }

    /// Writes the snapshot (run once, for 1.0: LS_COMPAT_SNAPSHOT=1 cargo test compat_write_api -- --ignored).
    #[test]
    #[ignore]
    fn compat_write_api() {
        if std::env::var_os("LS_COMPAT_SNAPSHOT").is_none() {
            return;
        }
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/compat/1.0/server/plugin_api.txt");
        let head = "# The plugin API Loner-Strike 1.0 ships (loner.inc, ls_components.inc): frozen. Plugins built\n# with 1.0's includes call these; every 1.x must still declare each one exactly like this.\n";
        std::fs::write(p, format!("{head}{}\n", plugin_api().join("\n"))).unwrap();
    }

    /// The bot brain features a mode can need, with the brain version that has each: an old brain
    /// is told apart from a new one, so the app can say which update a mode needs.
    #[test]
    fn compat_brain_versions_known() {
        let needs = crate::tools::requirements::mode_needs(&crate::tools::studio::graph::parse(&fixture("server/mode_avoid.graph.json")).unwrap());
        assert_eq!(needs["modeVer"], 4, "{needs}");
    }
}
