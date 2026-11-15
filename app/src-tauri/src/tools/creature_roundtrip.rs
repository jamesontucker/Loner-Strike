//! The shared-creature round trip on real game folders (QA, ignored by default):
//!   LS_RT_AUTHOR=<a Half-Life folder> LS_RT_TARGET=<another> LS_RT_OUT=<scratch folder> \
//!     cargo test creature_round_trip -- --ignored --nocapture
//! publishes a Game Mode Studio mode that spawns the author's own type (qa_crawler, from a
//! runner) at each player spawn, as the app's Publish builds it; unpacks the package the way a
//! Community install does and installs its creature block into the target game under the
//! uploader's gb<id> namespace; writes the packaged graph to <out>/qa_rt.graph.json for the
//! Studio to put in the target game. `LS_RT_STEP=uninstall` takes the block out again.
use std::path::{Path, PathBuf};

const MEMBER: i64 = 1234567;
const PKG: &str = "mod-9990001";

#[test]
#[ignore]
fn creature_round_trip() {
    let (Ok(author), Ok(target), Ok(out)) = (std::env::var("LS_RT_AUTHOR"), std::env::var("LS_RT_TARGET"), std::env::var("LS_RT_OUT")) else { return };
    let (author, target, out) = (PathBuf::from(author), PathBuf::from(target), PathBuf::from(out));
    let tgame = target.join("cstrike");
    if std::env::var("LS_RT_STEP").as_deref() == Ok("uninstall") {
        let before = std::fs::read_to_string(tgame.join("addons/amxmodx/configs/gamemodes/creatures.ini")).unwrap();
        let removed = super::creaturepack::uninstall_pack(&tgame, PKG).unwrap();
        let after = std::fs::read_to_string(tgame.join("addons/amxmodx/configs/gamemodes/creatures.ini")).unwrap_or_default();
        println!("removed files: {removed:?}");
        println!("--- before ---\n{before}\n--- after ---\n{after}");
        return;
    }
    // the author's own type
    let ini = author.join("cstrike/addons/amxmodx/configs/gamemodes/creatures.ini");
    std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&ini).unwrap_or_default();
    if !text.contains("[qa_crawler]") {
        text.push_str("\n[qa_crawler]\nbase = runner\nname = QA Crawler\nhp = 77\n");
        std::fs::write(&ini, &text).unwrap();
    }
    // a mode that spawns it where each player spawns
    let graphs = out.join("graphs");
    let lib = out.join("lib");
    let pkgs = out.join("packages");
    for d in [&graphs, &lib, &pkgs] {
        std::fs::create_dir_all(d).unwrap();
    }
    let graph = serde_json::json!({
        "schema": super::studio::graph::SCHEMA, "version": super::studio::graph::VERSION,
        "mode": {"id": "qa_rt", "uid": "7d1c0e7a-5b8e-4a55-9a10-2f6d3a4c9e01", "name": "QA Crawlers", "description": "A crawler where each player spawns"},
        "variables": [], "comments": [],
        "nodes": [
            {"id": "e", "type": "event.player_spawned", "pos": {"x": 0, "y": 0}, "params": {}},
            {"id": "cs", "type": "creature.spawn", "pos": {"x": 300, "y": 0}, "params": {"kind": "qa_crawler"}}
        ],
        "edges": [
            {"id": "w1", "from": {"node": "e", "pin": "then"}, "to": {"node": "cs", "pin": "exec"}},
            {"id": "w2", "from": {"node": "e", "pin": "player"}, "to": {"node": "cs", "pin": "at_ent"}}
        ]
    });
    std::fs::write(graphs.join("qa_rt.graph.json"), serde_json::to_string_pretty(&graph).unwrap()).unwrap();
    let req: super::community::PublishReq = serde_json::from_value(serde_json::json!({"title": "QA Crawlers", "summary": "Round trip", "author": "QA", "bots": "ok", "member": MEMBER})).unwrap();
    let p = super::community::build_mode_package(&graphs, &lib, "qa_rt", &req, &pkgs, None, Some((&author, "cstrike"))).unwrap();
    println!("package {} ({} bytes): {:?}", p.file, p.size, p.contents);
    println!("index creatures: {}", serde_json::to_string(&p.item.creatures).unwrap());
    // unpack it as a Community install does, and put the block in the target game
    let src = out.join("unpacked");
    let _ = std::fs::remove_dir_all(&src);
    crate::archive::extract_strict(&pkgs.join(&p.file), &src, 64 << 20).unwrap();
    let (_, pack) = super::creaturepack::find_pack(&src).expect("the package carries creatures/creatures.ini");
    println!("--- creatures/creatures.ini ---\n{pack}");
    let ns = super::creaturepack::namespace_of(MEMBER).unwrap();
    let done = super::creaturepack::install_pack(&tgame, PKG, &pack, Some(&src), &ns).unwrap();
    println!("installed: {done:?}");
    // the packaged graph (its creature renamed) for the Studio to install in the target game
    let g = find_graph(&src).expect("a graph in the package");
    std::fs::copy(&g, out.join("qa_rt.graph.json")).unwrap();
    println!("graph: {}", std::fs::read_to_string(out.join("qa_rt.graph.json")).unwrap().lines().filter(|l| l.contains("kind")).collect::<Vec<_>>().join(" "));
}

fn find_graph(root: &Path) -> Option<PathBuf> {
    crate::archive::walk(root).into_iter().find(|p| p.to_string_lossy().ends_with(".graph.json"))
}
