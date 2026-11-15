//! Engine coverage nodes (EC): the approved gaps of docs/notes/EngineCoverage.md (1-8, 10-15)
//! and NPC moves. Deal Damage, Get/Set Entity Property, Big HUD Text, Colored Chat, Player
//! Movement State, Push, Jump Rules, Grenade Thrown/Exploded, Set Grenade Fuse, Play Animation,
//! Attach To, Detach, On Map Entity Fired, Remove Map Objectives, Map Type, Weapon Switched,
//! Voice Rules, Voice Override, Play Radio, In Map Zone, Entered/Left Map Zone, Show Info Window,
//! NPC Move To, NPC Follow Path, NPC Stop.
//!
//! A child of gen.rs, kept out of it on purpose (others edit it): gen.rs calls `is_pure`/`pure`
//! for the values, `is_event`/`is_action`/`action` in node_code, `blocks` for the stocks,
//! `init` inside plugin_init and `handlers` for the hooks; graph.rs calls `problems`. Nothing
//! here is written into a plugin that has none of these nodes. Settings reach the plugin only as
//! numbers, the registry's own choices, or cleaned and quoted text: graph text never becomes code
//! or a server command.
//!
//! Comfort limits (attachments, looping animations, NPC moves at once, hooked map entity classes)
//! are #defines with plain defaults; past one, the call is skipped (the debugger says so), the
//! server keeps going.

use super::*;
use crate::tools::studio::graph::Problem;

pub(crate) const PURE: [&str; 7] = ["ent.prop_get", "player.move_state", "game.map_type", "player.in_zone", "player.has_flag", "game.vote_count", "hostage.is_following"];
pub(crate) const EVENTS: [&str; 6] = ["event.grenade_thrown", "event.grenade_exploded", "event.map_fired", "event.weapon_switched", "event.zone_enter", "event.zone_leave"];
pub(crate) const ACTIONS: [&str; 24] = [
    "game.round_end_rules", "game.start_vote", "hostage.follow", "hostage.stop", "botctl.add", "botctl.remove",
    "damage.deal", "ent.prop_set", "msg.big_hud", "msg.color_chat", "player.push", "player.jump_rules", "ent.grenade_fuse", "ent.play_anim", "ent.attach", "ent.detach",
    "game.remove_objectives", "game.voice_rules", "player.voice_override", "msg.radio", "msg.info_window", "npc.move_to", "npc.follow_path", "npc.stop",
];

pub(super) fn is_pure(k: &str) -> bool {
    PURE.contains(&k)
}
pub(super) fn is_event(k: &str) -> bool {
    EVENTS.contains(&k)
}
pub(super) fn is_action(k: &str) -> bool {
    ACTIONS.contains(&k)
}
fn mine(k: &str) -> bool {
    is_pure(k) || is_event(k) || is_action(k)
}
/// Any of these nodes in the graph (they use the vector helpers).
pub(super) fn used(g: &Graph) -> bool {
    g.nodes.iter().any(|n| mine(&n.kind))
}
fn has(g: &Graph, k: &str) -> bool {
    g.nodes.iter().any(|n| n.kind == k)
}
/// The NPC move nodes need gm_botnav for routes (plugins.ini).
pub(crate) fn needs_botnav(g: &Graph) -> bool {
    has(g, "npc.move_to") || has(g, "npc.follow_path")
}

const WEAPONS: [&str; 30] = [
    "knife", "glock18", "usp", "p228", "deagle", "elite", "fiveseven", "m3", "xm1014", "mac10", "tmp", "mp5navy", "ump45", "p90", "galil", "famas", "ak47", "m4a1", "sg552", "aug", "scout",
    "awp", "g3sg1", "sg550", "m249", "hegrenade", "flashbang", "smokegrenade", "c4", "shield",
];

fn field_id(f: &str) -> i32 {
    match f {
        "rendermode" => 0,
        "renderfx" => 1,
        "color" => 2,
        "amount" => 3,
        "skin" => 4,
        "body" => 5,
        "scale" => 6,
        "gravity" => 7,
        "movetype" => 8,
        "solid" => 9,
        "health" => 10,
        _ => 11,
    }
}

/// A Pick: (the field it belongs to, the engine's number).
fn pick_of(p: &str) -> (&'static str, i32) {
    match p {
        "normal" => ("rendermode", 0),
        "color" => ("rendermode", 1),
        "texture" => ("rendermode", 2),
        "glow" => ("rendermode", 3),
        "solid" => ("rendermode", 4),
        "additive" => ("rendermode", 5),
        "fx_none" => ("renderfx", 0),
        "fx_pulse" => ("renderfx", 1),
        "fx_pulsefast" => ("renderfx", 2),
        "fx_fadeslow" => ("renderfx", 5),
        "fx_strobe" => ("renderfx", 9),
        "fx_flicker" => ("renderfx", 12),
        "fx_distort" => ("renderfx", 15),
        "fx_hologram" => ("renderfx", 16),
        "fx_glowshell" => ("renderfx", 19),
        "mv_none" => ("movetype", 0),
        "mv_step" => ("movetype", 4),
        "mv_fly" => ("movetype", 5),
        "mv_toss" => ("movetype", 6),
        "mv_noclip" => ("movetype", 8),
        "mv_flymissile" => ("movetype", 9),
        "mv_bounce" => ("movetype", 10),
        "so_not" => ("solid", 0),
        "so_trigger" => ("solid", 1),
        _ => ("solid", 2),
    }
}

fn dmg_bits(k: &str) -> i64 {
    match k {
        "bullet" => 2,
        "slash" => 4,
        "burn" => 8,
        "freeze" => 16,
        "fall" => 32,
        "blast" => 64,
        "shock" => 256,
        "crush" => 1,
        "poison" => 131072,
        _ => 0,
    }
}

fn zone_bit(z: &str) -> i32 {
    match z {
        "bomb" => 2,
        "rescue" => 4,
        "escape" => 8,
        "vip" => 16,
        _ => 1,
    }
}

fn gren(t: &str) -> i32 {
    match t {
        "he" => 1,
        "flash" => 2,
        "smoke" => 3,
        _ => 0,
    }
}

/// A radio line: (sound, radio text or announcer center text, is the announcer).
fn radio(l: &str) -> (&'static str, &'static str, bool) {
    match l {
        "fallback" => ("%!MRAD_FALLBACK", "#Team_fall_back", false),
        "sticktog" => ("%!MRAD_STICKTOG", "#Stick_together_team", false),
        "getinpos" => ("%!MRAD_GETINPOS", "#Get_in_position_and_wait", false),
        "stormfront" => ("%!MRAD_STORMFRONT", "#Storm_the_front", false),
        "reportin" => ("%!MRAD_REPORTIN", "#Report_in_team", false),
        "affirm" => ("%!MRAD_AFFIRM", "#Affirmative", false),
        "roger" => ("%!MRAD_ROGER", "#Roger_that", false),
        "enemyspot" => ("%!MRAD_ENEMYSPOT", "#Enemy_spotted", false),
        "backup" => ("%!MRAD_BACKUP", "#Need_backup", false),
        "clear" => ("%!MRAD_CLEAR", "#Sector_clear", false),
        "inpos" => ("%!MRAD_INPOS", "#In_position", false),
        "reportingin" => ("%!MRAD_REPRTINGIN", "#Reporting_in", false),
        "blow" => ("%!MRAD_BLOW", "#Get_out_of_there", false),
        "negative" => ("%!MRAD_NEGATIVE", "#Negative", false),
        "enemydown" => ("%!MRAD_ENEMYDOWN", "#Enemy_down", false),
        "coverme" => ("%!MRAD_COVERME", "#Cover_me", false),
        "takepoint" => ("%!MRAD_TAKEPOINT", "#You_take_the_point", false),
        "position" => ("%!MRAD_POSITION", "#Hold_this_position", false),
        "regroup" => ("%!MRAD_REGROUP", "#Regroup_team", false),
        "followme" => ("%!MRAD_FOLLOWME", "#Follow_me", false),
        "takingfire" => ("%!MRAD_HITASSIST", "#Taking_fire", false),
        "fireinhole" => ("%!MRAD_FIREINHOLE", "#Fire_in_the_hole", false),
        "a_terwin" => ("%!MRAD_terwin", "#Terrorists_Win", true),
        "a_ctwin" => ("%!MRAD_ctwin", "#CTs_Win", true),
        "a_draw" => ("%!MRAD_rounddraw", "#Round_Draw", true),
        "a_bombpl" => ("%!MRAD_BOMBPL", "#Bomb_Planted", true),
        "a_bombdef" => ("%!MRAD_BOMBDEF", "#Bomb_Defused", true),
        "a_rescued" => ("%!MRAD_rescued", "#All_Hostages_Rescued", true),
        "a_letsgo" => ("%!MRAD_LETSGO", "", true),
        "a_locknload" => ("%!MRAD_LOCKNLOAD", "", true),
        "a_moveout" => ("%!MRAD_MOVEOUT", "", true),
        _ => ("%!MRAD_GO", "#Go_go_go", false),
    }
}

/// A pure node's output as an expression.
pub(super) fn pure(gen: &Gen, node: &str, pin: &str) -> String {
    let (n, def) = gen.node(node);
    let i = |p: &str, t: &str| gen.need(node, p, t);
    match def.kind.as_str() {
        "ent.prop_get" => {
            let e = i("ent", "entity");
            if pin == "color" { format!("lsg_ec_getcolor({e})") } else { format!("lsg_ec_get({e}, {})", field_id(gen.choice(n, def, "field"))) }
        }
        "player.move_state" => {
            let p = i("player", "player");
            let w = ["ground", "duck", "water", "level", "ladder", "speed", "vspeed"].iter().position(|x| *x == pin).unwrap_or(0);
            if w == 3 || w >= 5 { format!("lsg_ec_mvf({p}, {w})") } else { format!("lsg_ec_mv({p}, {w})") }
        }
        "game.map_type" => match pin {
            "type" => "lsg_ec_mtype()".into(),
            "bomb" => "(lsg_ec_mhas(1) ? 1 : 0)".into(),
            "hostage" => "(lsg_ec_mhas(2) ? 1 : 0)".into(),
            "vip" => "(lsg_ec_mhas(3) ? 1 : 0)".into(),
            _ => "(lsg_ec_mhas(4) ? 1 : 0)".into(),
        },
        "player.has_flag" => {
            let f = gen.choice(n, def, "flag").bytes().next().unwrap_or(b'c');
            format!("(lsg_ok({p}) && (get_user_flags({p}) & {}) ? 1 : 0)", 1i64 << (f.clamp(b'a', b'z') - b'a'), p = i("player", "player"))
        }
        "game.vote_count" => format!("float(g_lsgEcVoteN[clamp(floatround({}), 0, 8)])", i("option", "number")),
        "hostage.is_following" => {
            let h = i("hostage", "entity");
            if pin == "leader" { format!("lsg_ec_hleader({h})") } else { format!("(lsg_ec_hleader({h}) > 0 ? 1 : 0)") }
        }
        "player.in_zone" => format!("lsg_ec_inzone({}, {})", i("player", "player"), zone_bit(gen.choice(n, def, "zone"))),
        _ => "0".into(),
    }
}

/// The NPC move nodes in the graph, in order (their number in the dispatch).
fn npc_nodes<'g>(gen: &'g Gen) -> Vec<&'g str> {
    gen.live.iter().copied().filter(|id| matches!(gen.v.defs[*id].kind.as_str(), "npc.move_to" | "npc.follow_path")).collect()
}

fn vote_nodes<'g>(gen: &'g Gen) -> Vec<&'g str> {
    gen.live.iter().copied().filter(|id| gen.v.defs[*id].kind == "game.start_vote").collect()
}

/// The node's code (inside its function, before it goes on).
#[allow(clippy::too_many_arguments)]
pub(super) fn action(gen: &Gen, id: &str, n: &Node, def: &'static NodeDef, b: &mut Out, inp: &dyn Fn(&str, &str) -> String, exit: &dyn Fn(&str) -> String, err: &dyn Fn(&str) -> String) {
    let debug = gen.debug;
    let wired = |pin: &str| gen.v.data_source(id, pin).is_some();
    let on = |k: &str| if gen.boolean(n, def, k) { "true" } else { "false" };
    let num = |k: &str| float_lit(gen.num(n, def, k));
    // a call that can be refused: the debugger says why
    let checked = |b: &mut Out, call: String, why: &str, ok: &str| {
        if debug {
            b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err(why), exit(ok)));
        } else {
            b.line(format!("\t{call}"));
        }
    };
    let ex = |b: &mut Out, r: &str| {
        let e = exit(r);
        if !e.is_empty() {
            b.line(e);
        }
    };
    let who = || if wired("player") { inp("player", "player") } else { "0".into() };
    match def.kind.as_str() {
        "damage.deal" => {
            let a = if wired("attacker") { inp("attacker", "player") } else { "0".into() };
            let zone = ["generic", "head", "chest", "stomach", "leftarm", "rightarm", "leftleg", "rightleg"].iter().position(|z| *z == gen.choice(n, def, "zone")).unwrap_or(0);
            let call = format!("lsg_ec_damage({}, {a}, floatclamp({}, 0.0, 10000.0), {}, {zone}, {})", inp("target", "entity"), inp("amount", "number"), dmg_bits(gen.choice(n, def, "kind")), on("armor"));
            checked(b, call, "Not hurt: the target isn't there, is dead, or can't be hurt.", "hurt");
        }
        "ent.prop_set" => {
            let f = gen.choice(n, def, "field");
            let (pf, pv) = pick_of(gen.choice(n, def, "pick"));
            let pick = if pf == f { pv } else { -1 };
            let color = if wired("color") {
                inp("color", "vector")
            } else {
                let c = |k: &str| float_lit(gen.num(n, def, k).clamp(0.0, 255.0));
                format!("lsg_vec({}, {}, {})", c("r"), c("g"), c("b"))
            };
            let call = format!("lsg_ec_set({}, {}, {}, {pick}, {color})", inp("ent", "entity"), field_id(f), inp("value", "number"));
            checked(b, call, "Not set: the entity isn't there, or that property can't change on it (a player's movement or collision, a brush's movement, a scale on a player).", "set");
        }
        "msg.big_hud" => {
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 100));
            let args = text_args(gen, &text, gen.input(id, "value"));
            let c = |k: &str| gen.num(n, def, k).round().clamp(0.0, 255.0) as i64;
            let (fx, fxt, fin) = match gen.choice(n, def, "effect") {
                "flicker" => (1, "1.0", "0.1"),
                "type" => (2, "0.5", "0.05"),
                _ => (0, "0.0", "0.2"),
            };
            b.line(format!("\tif (lsg_msg_room(\"Big HUD Text\", 1, {}, {}))\n\t{{\n\t\tset_dhudmessage({}, {}, {}, {}, {}, {fx}, {fxt}, {}, {fin}, 0.5)\n\t\tshow_dhudmessage({}, {args})\n\t}}", who(), gen.limit_expr("messages"), c("r"), c("g"), c("b"), num("x"), num("y"), num("seconds"), who()));
            ex(b, "shown");
        }
        "msg.color_chat" => {
            let text = pawn_str(&safe_text(gen.text(n, def, "text"), 120));
            let tag = safe_text(gen.text(n, def, "tag"), 24);
            let args = text_args(gen, &text, gen.input(id, "value"));
            let (code, sender) = match gen.choice(n, def, "color") {
                "red" => ("^3", "print_team_red".to_string()),
                "blue" => ("^3", "print_team_blue".into()),
                "gray" => ("^3", "print_team_grey".into()),
                "green" => ("^4", "print_team_default".into()),
                "normal" => ("^1", "print_team_default".into()),
                _ => ("^3", if wired("of") { format!("lsg_ec_sender({})", inp("of", "player")) } else { "print_team_default".into() }),
            };
            // (the tag is safe text: no quotes, carets or backslashes; % doubled for the format)
            let head = if tag.is_empty() { String::new() } else { format!("^4{}^1 ", tag.replace('%', "%%")) };
            b.line(format!("\tif (lsg_msg_room(\"Colored Chat\", 1, {}, {}))\n\t{{\n\t\tnew lsgM[190]\n\t\tformatex(lsgM, charsmax(lsgM), {args})\n\t\tlsg_ec_clean(lsgM)\n\t\tclient_print_color({}, {sender}, \"{head}{code}%s\", lsgM)\n\t}}", who(), gen.limit_expr("messages"), who()));
            ex(b, "said");
        }
        "player.push" => {
            let along = ["aim", "facing", "dir", "away", "up"].iter().position(|x| *x == gen.choice(n, def, "along")).unwrap_or(0);
            let d = if wired("dir") { inp("dir", "vector") } else { "-1".into() };
            let p = if wired("point") { inp("point", "vector") } else { "-1".into() };
            let call = format!("lsg_ec_push({}, {along}, {d}, {p}, floatclamp({}, 0.0, 3000.0), {}, {})", inp("target", "entity"), inp("strength", "number"), num("lift"), if gen.choice(n, def, "how") == "replace" { "true" } else { "false" });
            checked(b, call, "Not pushed: nothing there, a dead player, or no Direction / Point wired in for that choice.", "pushed");
        }
        "player.jump_rules" => {
            let air = gen.num(n, def, "air").round().clamp(0.0, 10.0) as i64;
            let flags = [("nojump", 1), ("noduck", 2), ("bhop", 4), ("reset_death", 8), ("reset_round", 16)].iter().filter(|(k, _)| gen.boolean(n, def, k)).map(|(_, b)| b).sum::<i32>();
            b.line(format!("\tlsg_ec_jump({}, {flags}, {air}, {})", who(), num("power")));
            ex(b, "set");
        }
        "ent.grenade_fuse" => {
            let call = format!("lsg_ec_fuse({}, floatclamp({}, 0.0, 60.0))", inp("grenade", "entity"), inp("secs", "number"));
            checked(b, call, "Not set: that isn't a thrown grenade (or it went off already).", "set");
        }
        "ent.play_anim" => {
            let seq = if wired("number") { format!("floatround({})", inp("number", "number")) } else { "-1".into() };
            let call = format!("lsg_ec_anim({}, {}, {seq}, {}, {}, {}, {}, {})", inp("ent", "entity"), pawn_str(&name_text(gen.text(n, def, "sequence"))), num("rate"), on("restart"), on("loop"), num("length"), gen.limit_expr("ec_anim_loops"));
            checked(b, call, "Not played: not a model entity (players are animated by the game), no sequence by that name, or too many looping already (Limits).", "playing");
        }
        "ent.attach" => {
            let pt = match gen.choice(n, def, "point") {
                "a0" => 0,
                "a1" => 1,
                "a2" => 2,
                "a3" => 3,
                _ => -1,
            };
            let off = if wired("offset") { inp("offset", "vector") } else { "-1".into() };
            let call = format!("lsg_ec_attach({}, {}, {off}, {pt}, {}, {})", inp("ent", "entity"), inp("to", "entity"), on("on_death"), gen.limit_expr("ec_attach"));
            checked(b, call, "Not attached: a player can't be attached, the parent isn't there, it's itself, or too many are attached already (Limits).", "attached");
        }
        "ent.detach" => {
            b.line(format!("\tlsg_ec_detach({}, {})", inp("ent", "entity"), on("fall")));
            ex(b, "detached");
        }
        "game.remove_objectives" => {
            let mask = [("buy", 1), ("bomb", 2), ("hostages", 4), ("rescue", 8), ("vip", 16), ("escape", 32)].iter().filter(|(k, _)| gen.boolean(n, def, k)).map(|(_, b)| b).sum::<i32>();
            b.line(format!("\tlsg_ec_noobj({mask})"));
            ex(b, "removed");
        }
        "game.voice_rules" => {
            let w = ["all", "team", "life", "team_life", "near"].iter().position(|x| *x == gen.choice(n, def, "who")).unwrap_or(0);
            b.line(format!("\tg_lsgEcVoice = {w}\n\tg_lsgEcVoiceDist = {}", num("dist")));
            ex(b, "set");
        }
        "player.voice_override" => {
            let h = match gen.choice(n, def, "how") {
                "heard" => 1,
                "silent" => 2,
                "hears" => 3,
                _ => 0,
            };
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t\tg_lsgEcVoiceOv[p] = {h}", inp("player", "player")));
            ex(b, "set");
        }
        "msg.radio" => {
            let (snd, txt, ann) = radio(gen.choice(n, def, "line"));
            let team = match gen.choice(n, def, "to") {
                "t" => 1,
                "ct" => 2,
                _ => 0,
            };
            let from = if wired("from") { inp("from", "player") } else { "0".into() };
            let txt = if gen.boolean(n, def, "text") { txt } else { "" };
            b.line(format!("\tif (lsg_msg_room(\"Play Radio\", 2, {}, {}))\n\t\tlsg_ec_radio({}, {team}, {from}, {}, {}, {})", who(), gen.limit_expr("messages"), who(), pawn_str(snd), pawn_str(txt), if ann { "true" } else { "false" }));
            ex(b, "played");
        }
        "msg.info_window" => {
            let more = if wired("more") { inp("more", "text") } else { "\"\"".into() };
            let call = format!("lsg_ec_motd({}, {}, {}, {more})", inp("player", "player"), pawn_str(&safe_text(gen.text(n, def, "title"), 40)), pawn_str(&safe_text(gen.text(n, def, "text"), 900)));
            checked(b, call, "Not shown: nobody there, or a bot.", "shown");
        }
        "npc.move_to" | "npc.follow_path" => {
            let k = npc_nodes(gen).iter().position(|x| *x == id).unwrap_or(0);
            let run = gen.choice(n, def, "speed") == "run";
            let speed = if run { "220.0" } else { "90.0" };
            let mut flags = if run { 4 } else { 0 };
            let stay = gen.boolean(n, def, "stay");
            if stay {
                flags |= 2;
            }
            let npc = inp("npc", "entity");
            let call = if def.kind == "npc.move_to" {
                let to = if wired("to") { inp("to", "entity") } else { "0".into() };
                let pos = if wired("pos") { inp("pos", "vector") } else { "-1".into() };
                format!("lsg_ec_npc_to({npc}, {to}, {pos}, {speed}, {flags})")
            } else {
                if gen.boolean(n, def, "loop") {
                    flags |= 1;
                }
                let track = name_text(gen.text(n, def, "track"));
                let pts = if wired("points") { inp("points", "array<vector>") } else { "-1".into() };
                format!("lsg_ec_npc_path({npc}, {pts}, {}, {speed}, {flags})", pawn_str(&track))
            };
            // started: kept with this flow's values for Arrived / Failed; refused: Failed now
            let fail = gen.v.exec_target(id, "failed").map(|(t, tp)| format!("{}(ctx)", gen.pin_fname(t, tp)));
            b.line(format!("\tnew lsgNpc = {npc}\n\tif ({call})\n\t\tlsg_ec_npc_keep(lsgNpc, {k}, ctx, {})", gen.limit_expr("ec_npc_moves")));
            if debug {
                b.line(format!("\telse\n\t{{\n\t\t{}", err("Not moving: not an NPC (or dead), gm_components isn't running, or no points.")));
                if let Some(f) = &fail {
                    b.line(format!("\t\t{f}"));
                }
                b.line("\t}");
                b.line(exit("moving"));
            } else if let Some(f) = &fail {
                b.line(format!("\telse\n\t\t{f}"));
            }
        }
        "game.round_end_rules" => {
            let f = match gen.choice(n, def, "rule") {
                "elim" => "f",
                "time" => "ahijk",
                "both" => "afhijk",
                _ => "0",
            };
            b.line(format!("\tset_cvar_string(\"mp_round_infinite\", {})", pawn_str(f)));
            ex(b, "set");
        }
        "game.start_vote" => {
            let k = vote_nodes(gen).iter().position(|x| *x == id).unwrap_or(0);
            let count = gen.num(n, def, "count").round().clamp(2.0, 8.0) as usize;
            let who = ["all", "t", "ct", "alive"].iter().position(|x| *x == gen.choice(n, def, "who")).unwrap_or(0);
            let mut body = format!("\tif (lsg_ec_vote_begin({k}, ctx))\n\t{{\n\t\tnew m = menu_create({}, \"lsg_ec_vote_pick\")", pawn_str(&safe_text(gen.text(n, def, "question"), 60)));
            for o in 1..=count {
                body.push_str(&format!("\n\t\tmenu_additem(m, {})", pawn_str(&safe_text(gen.text(n, def, &format!("o{o}")), 30))));
            }
            body.push_str(&format!("\n\t\tlsg_ec_vote_show(m, {count}, {who}, {}, {})\n\t}}", if gen.boolean(n, def, "bots") { "true" } else { "false" }, num("secs")));
            b.line(body);
            let none = gen.v.exec_target(id, "none").map(|(t, tp)| format!("{}(ctx)", gen.pin_fname(t, tp)));
            b.line(format!("\telse\n\t{{\n\t\t{}\n\t}}", [if debug { err("Not started: another vote is on (this mode's or the server's map vote).") } else { String::new() }, none.unwrap_or_default()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join("\n\t\t")));
            ex(b, "voting");
        }
        "hostage.follow" => {
            let call = format!("lsg_ec_hfollow({}, {})", inp("hostage", "entity"), inp("player", "player"));
            checked(b, call, "Not following: not a hostage (or a rescued or dead one), or the leader isn't alive.", "following");
        }
        "hostage.stop" => {
            let call = format!("lsg_ec_hfollow({}, 0)", inp("hostage", "entity"));
            checked(b, call, "Not a hostage (or a rescued or dead one).", "stopped");
        }
        "botctl.add" => {
            let team = match gen.choice(n, def, "team") {
                "t" => 1,
                "ct" => 2,
                _ => 0,
            };
            let skill: i64 = gen.choice(n, def, "skill").parse().unwrap_or(-1);
            let call = format!("lsg_ec_bots_add(clamp(floatround({}), 1, 31), {team}, {skill})", inp("count", "number"));
            checked(b, call, "No bots added: no free slot (one is kept for a person).", "added");
        }
        "botctl.remove" => {
            let team = match gen.choice(n, def, "team") {
                "t" => 1,
                "ct" => 2,
                _ => 0,
            };
            let call = format!("lsg_ec_bots_remove(clamp(floatround({}), 1, 31), {team})", inp("count", "number"));
            checked(b, call, "No bots removed: none on that team.", "removed");
        }
        "npc.stop" => {
            b.line(format!("\tlsg_ec_npc_stop({}, {})", inp("npc", "entity"), on("stay")));
            ex(b, "stopped");
        }
        _ => {}
    }
}

/// The stocks, once, when any of these nodes is used.
pub(super) fn blocks(g: &Graph, o: &mut Out) {
    if !used(g) {
        return;
    }
    o.line(RUNTIME);
    if needs_botnav(g) || has(g, "npc.stop") {
        o.line(NPC_RUNTIME);
    }
    if has(g, "game.start_vote") || has(g, "game.vote_count") {
        o.line(VOTE_RUNTIME);
    }
}

/// Inside plugin_init: the hooks only the nodes in the graph need.
pub(super) fn init(gen: &Gen, o: &mut Out) {
    let g = super::g_of(gen);
    if !used(g) {
        return;
    }
    let live = |k: &str| gen.live.iter().any(|id| gen.v.defs[id].kind == k);
    if live("player.jump_rules") {
        o.line("\tregister_forward(FM_CmdStart, \"lsg_ec_cmd\", 0)\n\tregister_forward(FM_PlayerPreThink, \"lsg_ec_think\", 0)\n\tRegisterHamPlayer(Ham_Killed, \"lsg_ec_dead\", 1)\n\tregister_event(\"HLTV\", \"lsg_ec_round\", \"a\", \"1=0\", \"2=0\")");
    }
    if live("event.grenade_thrown") || live("event.grenade_exploded") || live("ent.grenade_fuse") {
        o.line("\tregister_forward(FM_SetModel, \"lsg_ec_setmodel\", 1)");
    }
    if live("event.grenade_exploded") {
        o.line("\tRegisterHam(Ham_Think, \"grenade\", \"lsg_ec_gthink\", 0)");
    }
    if live("event.map_fired") {
        o.line(format!("\tlsg_ec_hook_use({})", gen.limit_expr("ec_map_hooks")));
    }
    if live("event.weapon_switched") {
        for w in WEAPONS.iter().filter(|w| **w != "shield") {
            o.line(format!("\tRegisterHam(Ham_Item_Deploy, \"weapon_{w}\", \"lsg_ec_deploy\", 1)"));
        }
    }
    if live("game.voice_rules") || live("player.voice_override") {
        o.line("\tregister_forward(FM_Voice_SetClientListening, \"lsg_ec_voice\", 0)");
    }
    if live("ent.attach") {
        o.line("\tregister_forward(FM_StartFrame, \"lsg_ec_frame\", 0)");
    }
    if live("ent.play_anim") || live("event.zone_enter") || live("event.zone_leave") {
        o.line("\tset_task(0.1, \"lsg_ec_tick\", LSG_TASK - 610, _, _, \"b\")");
    }
    o.line("\tregister_forward(FM_ClientDisconnect, \"lsg_ec_left\", 0)");
}

/// ctx[...] or g_lsgTxt[...] for an event pin.
fn slot(kind: &str, pin: &str) -> Option<(bool, usize)> {
    event_slot(registry_def(kind), pin)
}

fn fill(o: &mut String, kind: &str, vals: &[(&str, &str)]) {
    for (pin, read) in vals {
        match slot(kind, pin) {
            Some((false, i)) => o.push_str(&format!("\n\tctx[{i}] = {read} // {pin}")),
            Some((true, i)) => o.push_str(&format!("\n\tcopy(g_lsgTxt[{i}], 63, {read}) // {pin}")),
            None => {}
        }
    }
}

/// The hooks' handlers and the always-there publics.
pub(super) fn handlers(gen: &Gen, o: &mut Out) {
    let g = super::g_of(gen);
    if !used(g) {
        return;
    }
    let v = &gen.v;
    let nodes = |k: &str| -> Vec<&str> { gen.live.iter().copied().filter(|id| v.defs[*id].kind == k).collect() };
    let live = |k: &str| !nodes(k).is_empty();
    let choice = |id: &str, p: &str| gen.choice(v.nodes[id], v.defs[id], p);

    // grenades
    let thrown = nodes("event.grenade_thrown");
    let boom = nodes("event.grenade_exploded");
    if live("event.grenade_thrown") || live("event.grenade_exploded") || live("ent.grenade_fuse") {
        let mut t = String::from("public lsg_ec_setmodel(e, const model[])\n{\n\tnew t = lsg_ec_gtype(e, model)\n\tif (t <= 0)\n\t\treturn FMRES_IGNORED");
        if !thrown.is_empty() {
            t.push_str("\n\tnew ctx[LSG_CTX], Float:o[3]\n\tpev(e, pev_origin, o)");
            let k = "event.grenade_thrown";
            fill(&mut t, k, &[("player", "g_lsgEcGOwner[e]"), ("grenade", "e"), ("pos", "lsg_vfrom(o)"), ("kind", "g_lsgEcGren[t]")]);
            for id in &thrown {
                let want = gren(choice(id, "type"));
                let call = format!("{}(ctx)", gen.fname(id));
                t.push_str(&if want == 0 { format!("\n\t{call}") } else { format!("\n\tif (t == {want})\n\t\t{call}") });
            }
        }
        t.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(t);
    }
    if !boom.is_empty() {
        let mut t = String::from("public lsg_ec_gthink(e)\n{\n\tnew t = lsg_ec_gdue(e)\n\tif (!t)\n\t\treturn HAM_IGNORED\n\tnew ctx[LSG_CTX], Float:o[3]\n\tpev(e, pev_origin, o)");
        fill(&mut t, "event.grenade_exploded", &[("player", "g_lsgEcGOwner[e]"), ("grenade", "e"), ("pos", "lsg_vfrom(o)"), ("kind", "g_lsgEcGren[t]")]);
        for id in &boom {
            let want = gren(choice(id, "type"));
            let call = format!("{}(ctx)", gen.fname(id));
            t.push_str(&if want == 0 { format!("\n\t{call}") } else { format!("\n\tif (t == {want})\n\t\t{call}") });
        }
        t.push_str("\n\treturn HAM_IGNORED\n}\n");
        o.line(t);
    }
    // map entity fired
    let fired = nodes("event.map_fired");
    if !fired.is_empty() {
        let mut t = String::from("public lsg_ec_used(ent, caller, activator, kind, Float:value)\n{\n\t#pragma unused kind\n\t#pragma unused value\n\tif (ent <= MaxClients || !pev_valid(ent) || lsg_ec_twice(ent, caller))\n\t\treturn HAM_IGNORED\n\tnew tn[33]\n\tpev(ent, pev_targetname, tn, charsmax(tn))\n\tif (!tn[0])\n\t\treturn HAM_IGNORED\n\tnew ctx[LSG_CTX]");
        fill(&mut t, "event.map_fired", &[("name", "tn"), ("entity", "ent"), ("activator", "(activator >= 1 && activator <= MaxClients ? activator : 0)"), ("caller", "(caller > 0 ? caller : 0)")]);
        for id in &fired {
            let name = name_text(gen.text(v.nodes[id], v.defs[id], "name"));
            let call = format!("{}(ctx)", gen.fname(id));
            t.push_str(&if name.is_empty() { format!("\n\t{call}") } else { format!("\n\tif (equal(tn, {}))\n\t\t{call}", pawn_str(&name)) });
        }
        t.push_str("\n\treturn HAM_IGNORED\n}\n");
        o.line(t);
    }
    // weapon switched
    let sw = nodes("event.weapon_switched");
    if !sw.is_empty() {
        let mut t = String::from("public lsg_ec_deploy(w)\n{\n\tif (!pev_valid(w))\n\t\treturn HAM_IGNORED\n\tnew id = get_ent_data_entity(w, \"CBasePlayerItem\", \"m_pPlayer\")\n\tif (!lsg_ok(id))\n\t\treturn HAM_IGNORED\n\tnew csw = cs_get_weapon_id(w), was = g_lsgEcLastW[id]\n\tif (csw == was && w == g_lsgEcLastE[id])\n\t\treturn HAM_IGNORED\n\tg_lsgEcLastW[id] = csw\n\tg_lsgEcLastE[id] = w\n\tnew ctx[LSG_CTX]");
        fill(&mut t, "event.weapon_switched", &[("player", "id"), ("weapon", "w"), ("new", "lsg_ec_wname(csw)"), ("old", "lsg_ec_wname(was)")]);
        for id in &sw {
            t.push_str(&format!("\n\t{}(ctx)", gen.fname(id)));
        }
        t.push_str("\n\treturn HAM_IGNORED\n}\n");
        o.line(t);
    }
    // the tick: looping animations, map zones
    if live("ent.play_anim") || live("event.zone_enter") || live("event.zone_leave") {
        let mut t = String::from("public lsg_ec_tick()\n{\n\tlsg_ec_anim_tick()");
        let ins = nodes("event.zone_enter");
        let outs = nodes("event.zone_leave");
        if !ins.is_empty() || !outs.is_empty() {
            t.push_str("\n\tg_lsgEcZoneTurn = !g_lsgEcZoneTurn\n\tif (g_lsgEcZoneTurn)\n\t\treturn\n\tfor (new p = 1; p <= MaxClients; p++)\n\t{\n\t\tnew z = is_user_alive(p) ? cs_get_user_mapzones(p) : 0, was = g_lsgEcZone[p]\n\t\tg_lsgEcZone[p] = z\n\t\tif (z == was)\n\t\t\tcontinue\n\t\tnew ctx[LSG_CTX]");
            for (kind, list, cond) in [("event.zone_enter", &ins, "(z & ~was)"), ("event.zone_leave", &outs, "(was & ~z)")] {
                for id in list.iter() {
                    let mut f = String::new();
                    fill(&mut f, kind, &[("player", "p")]);
                    t.push_str(&format!("\n\t\tif ({cond} & {})\n\t\t{{{}\n\t\t\t{}(ctx)\n\t\t}}", zone_bit(choice(id, "zone")), f.replace("\n\t", "\n\t\t\t"), gen.fname(id)));
                }
            }
            t.push_str("\n\t}");
        }
        t.push_str("\n}\n");
        o.line(t);
    }
    // Start Vote: the result runs the winner's output (or Tie, No votes) with the flow's own values
    let votes = vote_nodes(gen);
    if !votes.is_empty() {
        let target = |id: &str, pin: &str| v.exec_target(id, pin).map(|(t, tp)| format!("{}(ctx)", gen.pin_fname(t, tp)));
        let mut t = String::from("public lsg_ec_vote_end()\n{\n\tnew k = g_lsgEcVoteNode, w = lsg_ec_vote_close()\n\tnew ctx[LSG_CTX]\n\tfor (new i = 0; i < LSG_CTX; i++)\n\t\tctx[i] = g_lsgEcVoteCtx[i]\n\tswitch (k)\n\t{");
        for (k, id) in votes.iter().enumerate() {
            let mut cases = Vec::new();
            for o in 1..=8 {
                if let Some(c) = target(id, &format!("w{o}")) {
                    cases.push(format!("case {o}: {c}"));
                }
            }
            if let Some(c) = target(id, "tie") {
                cases.push(format!("case -1: {c}"));
            }
            if let Some(c) = target(id, "none") {
                cases.push(format!("case 0: {c}"));
            }
            if !cases.is_empty() {
                t.push_str(&format!("\n\t\tcase {k}:\n\t\t{{\n\t\t\tswitch (w)\n\t\t\t{{\n\t\t\t\t{}\n\t\t\t}}\n\t\t}}", cases.join("\n\t\t\t\t")));
            }
        }
        t.push_str("\n\t}\n\t#pragma unused ctx\n\t#pragma unused w\n}\n");
        o.line(t);
    }
    // NPC moves: Arrived / At point / Finished / Failed with the flow's own values
    let npcs = npc_nodes(gen);
    if !npcs.is_empty() {
        let target = |id: &str, pin: &str| v.exec_target(id, pin).map(|(t, tp)| format!("{}(ctx)", gen.pin_fname(t, tp)));
        let mut a = String::from("public ls_on_npc_arrived(npc, point, last)\n{\n\t#pragma unused point\n\tnew s = lsg_ec_npc_find(npc)\n\tif (s < 0)\n\t\treturn\n\tnew ctx[LSG_CTX], k = g_lsgEcNpcNode[s]\n\tfor (new i = 0; i < LSG_CTX; i++)\n\t\tctx[i] = g_lsgEcNpcCtx[s][i]\n\tif (last)\n\t\tg_lsgEcNpcEnt[s] = 0\n\tswitch (k)\n\t{");
        let mut f = String::from("public ls_on_npc_move_failed(npc, point)\n{\n\t#pragma unused point\n\tnew s = lsg_ec_npc_find(npc)\n\tif (s < 0)\n\t\treturn\n\tnew ctx[LSG_CTX], k = g_lsgEcNpcNode[s]\n\tfor (new i = 0; i < LSG_CTX; i++)\n\t\tctx[i] = g_lsgEcNpcCtx[s][i]\n\tg_lsgEcNpcEnt[s] = 0\n\tswitch (k)\n\t{");
        for (k, id) in npcs.iter().enumerate() {
            let mut body = Vec::new();
            if v.defs[*id].kind == "npc.move_to" {
                if let Some(c) = target(id, "arrived") {
                    body.push(format!("if (last)\n\t\t\t\t{c}"));
                }
            } else {
                if let Some(c) = target(id, "point") {
                    body.push(c);
                }
                if let Some(c) = target(id, "finished") {
                    body.push(format!("if (last)\n\t\t\t\t{c}"));
                }
            }
            if !body.is_empty() {
                a.push_str(&format!("\n\t\tcase {k}:\n\t\t{{\n\t\t\t{}\n\t\t}}", body.join("\n\t\t\t")));
            }
            if let Some(c) = target(id, "failed") {
                f.push_str(&format!("\n\t\tcase {k}: {c}"));
            }
        }
        a.push_str("\n\t}\n\t#pragma unused ctx\n}\n");
        f.push_str("\n\t}\n\t#pragma unused ctx\n}\n");
        o.line(a);
        o.line(f);
    }
}

fn err(node: &str, msg: &str) -> Problem {
    Problem { level: "error", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}
fn warn(node: &str, msg: &str) -> Problem {
    Problem { level: "warning", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

/// The Problems checks.
pub(crate) fn problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    let prm = |n: &Node, def: &NodeDef, k: &str| super::super::graph::param(n, def, k).cloned().unwrap_or(Value::Null);
    let wired = |n: &Node, pin: &str| v.data_source(&n.id, pin).is_some();
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        match def.kind.as_str() {
            "ent.prop_set" => {
                let f = prm(n, def, "field");
                let f = f.as_str().unwrap_or("");
                let pk = prm(n, def, "pick");
                let (pf, _) = pick_of(pk.as_str().unwrap_or(""));
                if matches!(f, "rendermode" | "renderfx" | "movetype" | "solid") && pf != f {
                    p.push(err(&n.id, "Pick one of this property's own values (the Pick list names the property first: Render mode, Effect, Movement, Collision)."));
                }
                if f == "movetype" {
                    p.push(warn(&n.id, "Movement on a map's brush entity (a door, a train) or a player is refused: they need their own. Use No Clip or Freeze for players."));
                }
            }
            "damage.deal" => {
                let a = prm(n, def, "amount").as_f64().unwrap_or(0.0);
                if !wired(n, "amount") && a <= 0.0 {
                    p.push(warn(&n.id, "Amount 0 hurts nobody."));
                }
            }
            "player.push" => {
                let al = prm(n, def, "along");
                match al.as_str() {
                    Some("dir") if !wired(n, "dir") => p.push(err(&n.id, "Along The Direction needs a direction wired into Direction.")),
                    Some("away") if !wired(n, "point") => p.push(err(&n.id, "Away from the Point needs a position wired into Away from point.")),
                    _ => {}
                }
            }
            "player.jump_rules" => {
                if prm(n, def, "nojump").as_bool() == Some(true) && (prm(n, def, "bhop").as_bool() == Some(true) || prm(n, def, "air").as_f64().unwrap_or(0.0) > 0.0) {
                    p.push(warn(&n.id, "Jumping is blocked, so air jumps and bunny hops never start."));
                }
            }
            "ent.play_anim" => {
                let s = prm(n, def, "sequence");
                if !wired(n, "number") && s.as_str().unwrap_or("").is_empty() {
                    p.push(err(&n.id, "Name a sequence, or wire a number into Sequence number."));
                }
            }
            "npc.move_to" => {
                if !wired(n, "to") && !wired(n, "pos") {
                    p.push(err(&n.id, "Wire an entity into To entity or a position into Or point."));
                }
            }
            "npc.follow_path" => {
                let t = prm(n, def, "track");
                if !wired(n, "points") && t.as_str().unwrap_or("").is_empty() {
                    p.push(err(&n.id, "Wire an array of positions into Points, or name the first path_track."));
                }
                if prm(n, def, "loop").as_bool() == Some(true) && v.exec_target(&n.id, "finished").is_some() {
                    p.push(warn(&n.id, "With Loop on, Finished never runs."));
                }
            }
            "game.voice_rules" => {
                if prm(n, def, "who").as_str() == Some("near") && prm(n, def, "dist").as_f64().unwrap_or(0.0) <= 0.0 {
                    p.push(warn(&n.id, "Only within the distance with distance 0 lets everyone hear everyone."));
                }
            }
            "game.remove_objectives" => {
                if prm(n, def, "buy").as_bool() == Some(true) {
                    p.push(warn(&n.id, "With the buy zones gone nobody can buy (a far away stand-in keeps the game from buying at spawn). Use Buying to choose where instead."));
                }
            }
            "game.start_vote" => {
                let c = prm(n, def, "count").as_f64().unwrap_or(2.0).round().clamp(2.0, 8.0) as usize;
                for k in 1..=c {
                    if prm(n, def, &format!("o{k}")).as_str().unwrap_or("").trim().is_empty() {
                        p.push(err(&n.id, &format!("Answer {k} is empty (Answers is {c}).")));
                    }
                }
                for k in (c + 1)..=8 {
                    if v.exec_target(&n.id, &format!("w{k}")).is_some() {
                        p.push(warn(&n.id, &format!("Output {k} is wired but there are only {c} answers: it never runs.")));
                    }
                }
            }
            "botctl.add" => {
                let c = prm(n, def, "count").as_f64().unwrap_or(1.0);
                if !wired(n, "count") && c > 10.0 {
                    p.push(warn(&n.id, "That's a lot of bots at once: the host's server may not have the slots (only free ones are used)."));
                }
            }
            "msg.info_window" => {
                let t = prm(n, def, "text");
                let low = t.as_str().unwrap_or("").to_ascii_lowercase();
                if low.contains("http") || low.contains("www.") {
                    p.push(warn(&n.id, "The info window shows plain text: a web address stays words, never a link or a page."));
                }
            }
            _ => {}
        }
    }
}

const RUNTIME: &str = include_str!("coverage.pawn");
const NPC_RUNTIME: &str = include_str!("coverage_npc.pawn");
const VOTE_RUNTIME: &str = include_str!("coverage_vote.pawn");

#[cfg(test)]
mod tests;
