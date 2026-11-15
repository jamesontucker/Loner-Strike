//! The engine coverage nodes (EC): what each compiles to, only what's used is hooked, the
//! Problems checks, the snapshot, and the real amxxpc compile.

use super::super::generate;
use crate::tools::studio::graph::{self, parse};
use std::path::{Path, PathBuf};

const G: &str = include_str!("../../testdata/every_node_ec.graph.json");

fn snapshot(name: &str, got: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tools/studio/testdata").join(name);
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() || !path.is_file() {
        std::fs::write(&path, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap();
    assert!(want == got, "{name} changed (UPDATE_SNAPSHOTS=1 to accept)");
}

#[test]
fn coverage_nodes_generate() {
    let g = parse(G).unwrap();
    let core = graph::validate_core(&g);
    assert!(core.is_empty(), "{core:#?}");
    let errs: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level == "error").collect();
    assert!(errs.is_empty(), "{errs:#?}");
    // every node of this block is in the graph
    for k in super::PURE.iter().chain(super::EVENTS.iter()).chain(super::ACTIONS.iter()) {
        assert!(g.nodes.iter().any(|n| n.kind == *k), "every_node_ec lacks {k}");
    }
    let r = generate(&g, false);
    let src = &r.source;
    let _ = std::fs::write(std::env::temp_dir().join("every_node_ec_last.sma"), src);
    for n in g.nodes.iter().filter(|n| !crate::tools::studio::graph::registry().get(&n.kind).unwrap().pure) {
        assert!(r.spans.iter().any(|s| s.node == n.id), "{} isn't in the plugin", n.id);
    }
    // Deal Damage: burn (8), head (1), armor ignored, the game's own TakeDamage
    assert!(src.contains(", 8, 1, true)") && src.contains("ExecuteHamB(Ham_TakeDamage, t, a, a, dmg, bits)"), "damage");
    // properties: glow shell is renderfx 19; color from the settings
    assert!(src.contains("lsg_ec_set(") && src.contains(", 1, 1.0, 19, lsg_vec(") && src.contains(", 2, 1.0, -1, lsg_vec(0.0, 200.0, 255.0))"), "props");
    assert!(src.contains("lsg_ec_get(") && src.contains(", 10)") && src.contains("lsg_ec_getcolor("));
    // Big HUD Text typed out; colored chat: the tag in green, team color of a player, a value cleaned
    assert!(src.contains("set_dhudmessage(255, 80, 40, -1.0, 0.3, 2, 0.5, 3.0, 0.05, 0.5)"), "dhud");
    assert!(src.contains("client_print_color(") && src.contains("\"^4[EC]^1 ^3%s\"") && src.contains("lsg_ec_sender("), "chat");
    assert!(src.contains("\"^4%s\", lsgM)") || src.contains("print_team_default, \"^4%s\""), "green");
    assert!(src.contains("lsg_ec_clean(lsgM)"));
    // the per-frame message limit is there
    assert!(src.contains("#define LSG_EC_MSGS 24") && src.contains("lsg_ec_msg_ok()"));
    // movement state, zones, map type
    assert!(src.contains("lsg_ec_mv(") && src.contains("lsg_ec_mvf(") && src.contains("lsg_ec_inzone(") && src.contains(", 2)") && src.contains("lsg_ec_mtype()"));
    // push, jumps
    assert!(src.contains("lsg_ec_push(") && src.contains("lsg_ec_jump(") && src.contains(", 12, 2, 268.0)") && src.contains("lsg_ec_jump(0, 18, 0, 268.0)"), "jumps");
    assert!(src.contains("register_forward(FM_CmdStart, \"lsg_ec_cmd\", 0)") && src.contains("register_forward(FM_PlayerPreThink, \"lsg_ec_think\", 0)"));
    // grenades
    assert!(src.contains("register_forward(FM_SetModel, \"lsg_ec_setmodel\", 1)") && src.contains("RegisterHam(Ham_Think, \"grenade\", \"lsg_ec_gthink\", 0)") && src.contains("if (t == 1)"));
    assert!(src.contains("lsg_ec_fuse("));
    // animation, attach
    assert!(src.contains("lsg_ec_anim(") && src.contains("\"idle\", -1, 1.0, true, true, 1.5)") && src.contains("register_forward(FM_StartFrame, \"lsg_ec_frame\", 0)"));
    assert!(src.contains("lsg_ec_attach(") && src.contains(", 0, false)") && src.contains("lsg_ec_detach("));
    // map entity fired: any name, and door1 only
    assert!(src.contains("lsg_ec_hook_use()") && src.contains("RegisterHamFromEntity(Ham_Use, e, \"lsg_ec_used\", 1)") && src.contains("if (equal(tn, \"door1\"))"));
    // weapon switched: every weapon's deploy
    assert!(src.contains("RegisterHam(Ham_Item_Deploy, \"weapon_ak47\", \"lsg_ec_deploy\", 1)") && src.contains("lsg_ec_wname(was)"));
    // objectives (bomb, hostages, rescue, VIP, escape = 62), voice, radio, info window
    assert!(src.contains("lsg_ec_noobj(62)") && src.contains("g_lsgEcVoice = 3") && src.contains("register_forward(FM_Voice_SetClientListening, \"lsg_ec_voice\", 0)"));
    assert!(src.contains("\"%!MRAD_GO\", \"#Go_go_go\", false)") && src.contains("\"%!MRAD_terwin\", \"#Terrorists_Win\", true)"));
    assert!(src.contains("lsg_ec_motd(") && src.contains("show_motd(id, buf, title)") && src.contains("&lt;"));
    // zones: entered and left, from the game's zone signals
    assert!(src.contains("cs_get_user_mapzones(p)") && src.contains("(z & ~was) & 1") && src.contains("(was & ~z) & 2"));
    // NPC moves: run 220, stay; a path array; a path_track chain looping; the answers
    assert!(src.contains("lsg_ec_npc_to(") && src.contains(", 220.0, 6)") && src.contains("lsg_ec_npc_path(") && src.contains("\"path1\", 90.0, 1)"));
    assert!(src.contains("public ls_on_npc_arrived(npc, point, last)") && src.contains("public ls_on_npc_move_failed(npc, point)") && src.contains("lsg_ec_npc_keep(lsgNpc, "));
    assert!(src.contains("lsg_ec_npc_stop("));
    // round end rules (both: elimination and every time-out), votes, admin flag (f = 32), hostages, bots
    assert!(src.contains("set_cvar_string(\"mp_round_infinite\", \"afhijk\")"), "round end");
    assert!(src.contains("lsg_ec_vote_begin(") && src.contains("menu_additem(m, \"Rifles\")") && src.contains("lsg_ec_vote_show(m, 3, 0, true, 10.0)") && src.contains("public lsg_ec_vote_end()") && src.contains("case -1: "), "vote");
    assert!(src.contains("callfunc_begin(\"gm_pubserver_voting\", \"gm_pubserver.amxx\")") && src.contains("float(g_lsgEcVoteN[clamp(floatround(0.0), 0, 8)])"));
    assert!(src.contains("& 32) ? 1 : 0)") && src.contains("lsg_ec_hfollow(") && src.contains("lsg_ec_hleader("));
    assert!(src.contains("lsg_ec_bots_add(clamp(floatround(2.0), 1, 31), 2, 2)") && src.contains("lsg_ec_bots_remove(clamp(floatround(1.0), 1, 31), 1)") && !src.contains("server_cmd(\"bot_add %"));
    snapshot("every_node_ec.sma", src);
    snapshot("every_node_ec.debug.sma", &generate(&g, true).source);
    // a mode without them has none of it
    let plain = generate(&parse(include_str!("../../testdata/markers.graph.json")).unwrap(), false).source;
    assert!(!plain.contains("lsg_ec_"));
}

/// Only what's used is hooked: a lone Colored Chat needs no hooks at all.
#[test]
fn only_what_is_used_is_hooked() {
    let g = graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "cc_only", "name": "CC"},
        "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "cc", "type": "msg.color_chat", "params": {"text": "x^1y", "color": "red"}}],
        "edges": [{"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "cc", "pin": "exec"}}]})).unwrap();
    let src = generate(&g, false).source;
    assert!(src.contains("print_team_red"));
    for h in ["lsg_ec_cmd\"", "lsg_ec_setmodel\"", "\tlsg_ec_hook_use()", "lsg_ec_deploy\"", "lsg_ec_voice\"", "lsg_ec_frame\"", "lsg_ec_tick\"", "ls_on_npc_arrived"] {
        assert!(!src.contains(h), "{h} hooked for nothing");
    }
}

/// Graph text never becomes code: a hostile tag or info window text stays in a string.
#[test]
fn hostile_text_stays_text() {
    let evil = "\"); server_cmd(\"quit\"); //^4";
    let g = graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "evil", "name": "E"},
        "nodes": [{"id": "ps", "type": "event.player_spawned"}, {"id": "cc", "type": "msg.color_chat", "params": {"tag": evil, "text": evil}},
                  {"id": "m", "type": "msg.info_window", "params": {"title": evil, "text": evil}}, {"id": "an", "type": "ent.play_anim", "params": {"sequence": evil}}],
        "edges": [{"id": "w1", "from": {"node": "ps", "pin": "then"}, "to": {"node": "cc", "pin": "exec"}}, {"id": "w2", "from": {"node": "cc", "pin": "then"}, "to": {"node": "m", "pin": "exec"}},
                  {"id": "w3", "from": {"node": "ps", "pin": "player"}, "to": {"node": "m", "pin": "player"}}, {"id": "w4", "from": {"node": "m", "pin": "then"}, "to": {"node": "an", "pin": "exec"}},
                  {"id": "w5", "from": {"node": "ps", "pin": "player"}, "to": {"node": "an", "pin": "ent"}}]})).unwrap();
    assert!(graph::validate(&g).iter().any(|p| p.level == "error"), "the checks refuse it");
    let src = generate(&g, false).source;
    assert!(!src.contains("server_cmd(\"quit"), "{src}");
}

/// Problems: a Pick of another property, a push with nothing to push along, an NPC move with
/// nowhere to go, a looping path with Finished wired, a web address in the info window.
#[test]
fn coverage_problems() {
    let mk = |t: &str, p: serde_json::Value| graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "pr", "name": "P"},
        "nodes": [{"id": "ps", "type": "event.player_spawned"}, {"id": "n", "type": t, "params": p}],
        "edges": [{"id": "w1", "from": {"node": "ps", "pin": "then"}, "to": {"node": "n", "pin": "exec"}}]})).unwrap();
    let has = |g: &graph::Graph, lvl: &str, s: &str| graph::validate(g).iter().any(|p| p.level == lvl && p.node.as_deref() == Some("n") && p.message.contains(s));
    assert!(has(&mk("ent.prop_set", serde_json::json!({"field": "rendermode", "pick": "fx_glowshell"})), "error", "own values"));
    assert!(!has(&mk("ent.prop_set", serde_json::json!({"field": "renderfx", "pick": "fx_glowshell"})), "error", "own values"));
    assert!(has(&mk("player.push", serde_json::json!({"along": "dir"})), "error", "Direction"));
    assert!(has(&mk("npc.move_to", serde_json::json!({})), "error", "To entity"));
    assert!(has(&mk("npc.follow_path", serde_json::json!({})), "error", "path_track"));
    assert!(has(&mk("msg.info_window", serde_json::json!({"text": "see www.example.com"})), "warning", "plain text"));
    assert!(has(&mk("player.jump_rules", serde_json::json!({"nojump": true, "air": 2})), "warning", "never start"));
    assert!(has(&mk("game.remove_objectives", serde_json::json!({"buy": true})), "warning", "nobody can buy"));
}

/// The graph with every one of these nodes compiles with the real amxxpc, both builds, no
/// warnings. Skipped without the game's compiler.
#[test]
fn coverage_compiles_with_amxxpc() {
    let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
    if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
        eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
        return;
    }
    let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
    let work = std::env::temp_dir().join(format!("ls_ec_amxxpc_{}", std::process::id()));
    let inc = work.join("include");
    crate::tools::studio::write_includes(&inc).unwrap();
    let g = parse(G).unwrap();
    for debug in [false, true] {
        let out = generate(&g, debug);
        let file = format!("studio_every_node_ec{}.sma", if debug { "_dbg" } else { "" });
        let src = work.join(&file);
        std::fs::write(&src, &out.source).unwrap();
        let (ok, output) = crate::tools::studio::run_amxxpc(&install, &scripting, &src, &inc, &work.join(file.replace(".sma", ".amxx"))).unwrap();
        let diags = crate::tools::studio::parse_compiler(&output, &file);
        assert!(ok, "{file} didn't compile:\n{output}");
        assert!(diags.is_empty(), "{file} has warnings:\n{output}");
    }
    let _ = std::fs::remove_dir_all(&work);
}
