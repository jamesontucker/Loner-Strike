//! The Starter library (make_starter.py; X2): every premade checks clean as a library item, the two
//! test modes that use all of them check clean and generate, and they compile with amxxpc.

use super::*;
use std::path::{Path, PathBuf};

const A: &str = include_str!("testdata/starter_library_a.graph.json");
const B: &str = include_str!("testdata/starter_library_b.graph.json");

fn starters() -> Vec<(&'static str, Value)> {
    builtin_library::BUILTIN.iter().filter(|(n, _)| n.starts_with("st_")).map(|(n, t)| (*n, serde_json::from_str(t).unwrap())).collect()
}

#[test]
fn starter_items_check_clean_and_say_what_they_are() {
    let items = starters();
    assert!(items.len() >= 10, "{}", items.len());
    for (name, v) in &items {
        check_library_item(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        // the help (what to wire in, what bots do) rides along as a comment over the root graph
        let root = v["root"].as_str().unwrap();
        let help = v["comments"].as_array().unwrap().iter().find(|c| c["graph"] == root && c["id"] == "help").unwrap_or_else(|| panic!("{name}: no help"));
        assert!(help["text"].as_str().unwrap().contains("Bots"), "{name}: the help doesn't say what bots do");
        assert!(v["category"].as_str().unwrap().starts_with("Starter: "), "{name}");
        // stock files only: no models, sprites or sounds outside the game's own folders
        let text = v.to_string();
        assert!(!text.contains("custom/"), "{name} uses a custom file");
    }
    // the app lists them (api_library adds the built-in ones; MCP's studio_library reads it)
    assert!(builtin_library::BUILTIN.iter().any(|(n, _)| *n == "run_dialog"));
}

#[test]
fn starter_test_modes_check_clean_and_generate() {
    for (part, text) in [("a", A), ("b", B)] {
        let g = graph::parse(text).unwrap();
        let p: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level != "info").collect();
        assert!(p.is_empty(), "{part}: {p:#?}");
        for debug in [false, true] {
            let src = gen::generate(&g, debug).source;
            assert!(src.contains("public plugin_init()"), "{part}");
            // what each premade there brings (premades waiting for other agents' nodes aren't in yet)
            let has = |f: &str| g.functions.iter().any(|x| x.id == f) || g.macros.iter().any(|x| x.id == f);
            for (f, want) in [("heal_take", "loot"), ("jp_launch", "jumppad"), ("mine_step", "\"mine\""), ("ks_kill", "is on a killing spree!"), ("lv_lose", "Lives left: "), ("cz_make", "capture"), ("hp_mark", "has the hot potato!"), ("fb_throw", "sprites/fexplo.spr"), ("dash_go", "weapons/knife_slash1.wav")] {
                if has(f) {
                    assert!(src.contains(want), "{part} {f}: {want}");
                }
            }
        }
    }
}

#[test]
fn starter_test_modes_compile_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-starter-amxxpc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let inc = work.join("include");
    write_includes(&inc).unwrap();
    for (part, text) in [("a", A), ("b", B)] {
        for debug in [false, true] {
            let g = graph::parse(text).unwrap();
            let out = gen::generate(&g, debug);
            let file = format!("starter_{part}{}.sma", if debug { "_dbg" } else { "" });
            let src = work.join(&file);
            std::fs::write(&src, &out.source).unwrap();
            let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
            assert!(ok, "{file} didn't compile:\n{output}");
            assert!(parse_compiler(&output, &file).is_empty(), "{file} has warnings:\n{output}");
        }
    }
    let _ = std::fs::remove_dir_all(&work);
}
