//! P5: the final Studio check's installer (docs/notes/FinalCheck.md). scripts/finalcheck.mjs runs it to
//! put Studio modes into a sim server copy the way the Studio's Compile does (the generated source,
//! the include files, the mode's line in modes.ini and its plugin list); the script compiles them.
//!
//! `LS_FC_GAME=<sim>/cstrike LS_FC_GRAPHS=a.graph.json,b.graph.json [LS_FC_DEBUG=a,b] \
//!    cargo test finalcheck_install -- --ignored --nocapture`
//!
//! One line per graph on stdout: `FC {"graph", "id", "plugin", "version", "installed", "errors",
//! "warnings", "precache"}`. A graph with errors isn't installed (its errors are the result: the
//! precache flood graph must be refused). LS_FC_DEBUG names the mode ids built with the debugger
//! (their node runs are written to data/studio/<id>.events), "all" for every graph.

use super::*;

#[test]
#[ignore]
fn finalcheck_install() {
    let Ok(game) = std::env::var("LS_FC_GAME") else {
        panic!("set LS_FC_GAME to a sim server's cstrike folder")
    };
    let am = PathBuf::from(game).join("addons/amxmodx");
    let scripting = am.join("scripting");
    write_includes(&scripting.join("studio/include")).expect("the include files");
    let debug_ids = std::env::var("LS_FC_DEBUG").unwrap_or_default();
    let debug_ids: Vec<&str> = debug_ids.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    for path in std::env::var("LS_FC_GRAPHS").unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let version = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["version"].as_u64());
        let g = match graph::parse(&text) {
            Ok(g) => g,
            Err(e) => {
                println!("FC {}", json!({"graph": path, "version": version, "installed": false, "errors": [e]}));
                continue;
            }
        };
        let problems = graph::validate(&g);
        let of = |level: &str| problems.iter().filter(|p| p.level == level).map(|p| p.message.clone()).collect::<Vec<_>>();
        let plugin = gen::plugin_name(&g.mode.id);
        let mut line = json!({"graph": path, "id": g.mode.id, "plugin": plugin, "version": version, "installed": false,
            "errors": of("error"), "warnings": of("warning"), "precache": limits::precache_count(&g), "nodes": g.nodes.len()});
        if !graph::has_errors(&problems) {
            let debug = debug_ids.iter().any(|d| *d == "all" || *d == g.mode.id);
            let out = gen::generate(&g, debug);
            // (an earlier install's debugger flag: the plugin list must name the plugin alone for
            // register_mode to know the mode as the Studio's)
            let pini = am.join(format!("configs/gamemodes/{}.plugins.ini", g.mode.id));
            if let Ok(t) = std::fs::read_to_string(&pini) {
                let _ = std::fs::write(&pini, t.replace(&format!("{plugin}.amxx debug"), &format!("{plugin}.amxx")));
            }
            let wrote = std::fs::write(scripting.join(format!("{plugin}.sma")), &out.source).map_err(|e| e.to_string()).and_then(|_| register_mode(&am.join("configs"), &g, &plugin));
            match wrote {
                Ok(()) => {
                    if debug {
                        // (as the Studio's Test run does: AMXX's traces name the plugin's lines)
                        let t = std::fs::read_to_string(&pini).unwrap_or_default().replace(&format!("{plugin}.amxx"), &format!("{plugin}.amxx debug"));
                        std::fs::write(&pini, t).expect("the plugin list");
                    }
                    line["installed"] = json!(true);
                    line["debug"] = json!(debug);
                }
                Err(e) => line["errors"] = json!([e]),
            }
        }
        println!("FC {line}");
    }
}
