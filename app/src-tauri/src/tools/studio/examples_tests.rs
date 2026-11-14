//! The five example modes (templates made by make_examples.py): each passes the quick check with
//! no errors or warnings, has notes, generates both builds, and compiles with the real amxxpc
//! with no warnings when the CrossOver game is there.

use super::{gen, graph, parse_compiler, run_amxxpc, write_includes};
use std::path::{Path, PathBuf};

const EXAMPLES: [(&str, &str); 5] = [
    ("infection", include_str!("../../../../ui/src/views/tools/studio/templates/infection.graph.json")),
    ("gun_game_lite", include_str!("../../../../ui/src/views/tools/studio/templates/gun-game-lite.graph.json")),
    ("king_of_the_hill", include_str!("../../../../ui/src/views/tools/studio/templates/king-of-the-hill.graph.json")),
    ("one_in_the_chamber", include_str!("../../../../ui/src/views/tools/studio/templates/one-in-the-chamber.graph.json")),
    ("juggernaut", include_str!("../../../../ui/src/views/tools/studio/templates/juggernaut.graph.json")),
];

#[test]
fn examples_check_clean_and_explain_themselves() {
    for (id, text) in EXAMPLES {
        let g = graph::parse(text).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(g.mode.id, id);
        let problems = graph::validate(&g);
        assert!(problems.is_empty(), "{id}: {problems:?}");
        assert!((25..=90).contains(&g.nodes.len()), "{id}: {} nodes (an example stays small)", g.nodes.len());
        assert!(g.comments.len() >= 3, "{id}: every part has a note");
        for debug in [false, true] {
            let out = gen::generate(&g, debug);
            assert!(out.source.contains("plugin_init"), "{id}: no plugin");
        }
    }
}

#[test]
fn examples_compile_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls-studio-examples-{}", std::process::id()));
    let inc = work.join("include");
    write_includes(&inc).unwrap();
    for (id, text) in EXAMPLES {
        let g = graph::parse(text).unwrap();
        for debug in [false, true] {
            let out = gen::generate(&g, debug);
            let file = format!("studio_{id}{}.sma", if debug { "_dbg" } else { "" });
            let src = work.join(&file);
            std::fs::write(&src, &out.source).unwrap();
            let (ok, output) = run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
            assert!(ok, "{file} didn't compile:\n{output}");
            assert!(parse_compiler(&output, &file).is_empty(), "{file} has warnings:\n{output}");
        }
    }
    let _ = std::fs::remove_dir_all(&work);
}
