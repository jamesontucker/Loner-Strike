//! P5's nodes: player input (Player Pressed Button, Is Holding Button, Player Command, Tell Player
//! To Bind), cooldowns (Start Cooldown, Is Ready, Cooldown Time Left, Reset Cooldown), Picked Up and
//! Dropped, weapon tuning (Infinite Ammo, Fire Rate, Recoil) and saving (Saved Text, Save Text,
//! Server Value, Set Server Value, Top Players).
//!
//! Kept out of gen.rs on purpose (others edit it): gen.rs calls `pure_value` for the pure nodes,
//! `is_event`/`is_action`/`action` for the node code, `helpers` and `init` and `handlers` for the
//! plugin around them, `end` in plugin_end, `uses_saved_numbers` for the shared saved values;
//! graph.rs calls `problems`. Nothing here is written into a plugin with none of these nodes.
//!
//! Costs, only when a node needs them: FM_PlayerPreThink (a button diff per player per frame) for
//! Player Pressed Button; one client command per Player Command (rate-limited per player); a touch
//! on weaponbox / armoury_entity plus Ham_AddPlayerItem for Picked Up; FM_SetModel for Dropped;
//! Ham_Weapon_PrimaryAttack on the guns for Infinite Ammo / Fire Rate / Recoil, CurWeapon for the
//! spare ammo. Graph text reaches the plugin only as numbers, registry choices or cleaned names.

use super::*;
use crate::tools::studio::graph::Problem;

/// The event nodes (their handlers are written here, not from EVENTS).
pub(super) const EVENT_KINDS: [&str; 4] = ["in.pressed", "in.command", "item.picked_up", "item.dropped"];
/// The flow (action) nodes.
pub(super) const ACTION_KINDS: [&str; 9] = ["in.tell_bind", "cd.start", "cd.reset", "wpn.infinite_ammo", "wpn.fire_rate", "wpn.recoil", "save.text_set", "save.server_set", "save.top"];
/// The pure nodes.
pub(super) const PURE_KINDS: [&str; 5] = ["in.holding", "cd.ready", "cd.left", "save.text_get", "save.server_get"];

/// Every node kind of this block (the every-node test covers them from every_node_p5.graph.json).
pub(crate) fn kinds() -> Vec<&'static str> {
    EVENT_KINDS.iter().chain(ACTION_KINDS.iter()).chain(PURE_KINDS.iter()).copied().collect()
}

pub(super) fn is_event(kind: &str) -> bool {
    EVENT_KINDS.contains(&kind)
}

pub(super) fn is_action(kind: &str) -> bool {
    ACTION_KINDS.contains(&kind)
}

/// Any node of this block in the graph.
pub(super) fn used(g: &Graph) -> bool {
    g.nodes.iter().any(|n| kinds().contains(&n.kind.as_str()))
}

fn has(g: &Graph, kinds: &[&str]) -> bool {
    g.nodes.iter().any(|n| kinds.contains(&n.kind.as_str()))
}

/// Do these nodes keep numbers in the mode's saved file (gen.rs's Saved Value helpers)?
pub(super) fn uses_saved_numbers(g: &Graph) -> bool {
    has(g, &["save.server_get", "save.server_set", "save.top"])
}

fn uses_saved_text(g: &Graph) -> bool {
    has(g, &["save.text_get", "save.text_set", "save.server_get", "save.server_set", "save.top"])
}

fn uses_weapons(g: &Graph) -> bool {
    has(g, &["wpn.infinite_ammo", "wpn.fire_rate", "wpn.recoil"])
}

/// A button choice as the game's IN_ bits.
fn button_bits(b: &str) -> i64 {
    match b {
        "attack" => 1,
        "jump" => 2,
        "duck" => 4,
        "forward" => 8,
        "back" => 16,
        "use" => 32,
        "left" => 512,
        "right" => 1024,
        "attack2" => 2048,
        "walk" => 4096,
        "reload" => 8192,
        _ => 1 | 2 | 4 | 8 | 16 | 32 | 512 | 1024 | 2048 | 4096 | 8192,
    }
}

/// A Player Command's name: ls_ and small letters only (anything else becomes ls_command), so a
/// graph can never register or shadow another command.
pub(crate) fn command_name(s: &str) -> String {
    let ok = s.len() <= 31 && s.starts_with("ls_") && s.len() > 3 && s[3..].chars().all(|c| c.is_ascii_lowercase());
    if ok { s.to_string() } else { "ls_command".into() }
}

/// A key name for Tell Player To Bind: letters, digits and _ only (f, mouse3, kp_ins).
fn key_name(s: &str) -> String {
    let k: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(16).collect();
    if k.is_empty() { "f".into() } else { k.to_ascii_lowercase() }
}

/// The cooldowns in the graph: (ability name, scope) pairs, each its own array.
fn cooldowns(g: &Graph) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for n in g.nodes.iter().filter(|n| n.kind.starts_with("cd.")) {
        let name = name_text(n.params.get("name").and_then(Value::as_str).unwrap_or("dash"));
        let name = if name.is_empty() { "dash".to_string() } else { name };
        let scope = match n.params.get("scope").and_then(Value::as_str).unwrap_or("player") {
            "team" => "team",
            "global" => "global",
            _ => "player",
        };
        if !out.iter().any(|(a, b)| *a == name && *b == scope) {
            out.push((name, scope));
        }
    }
    out
}

fn cd_index(g: &Graph, n: &Node) -> usize {
    let name = name_text(n.params.get("name").and_then(Value::as_str).unwrap_or("dash"));
    let name = if name.is_empty() { "dash".to_string() } else { name };
    let scope = match n.params.get("scope").and_then(Value::as_str).unwrap_or("player") {
        "team" => "team",
        "global" => "global",
        _ => "player",
    };
    cooldowns(g).iter().position(|(a, b)| *a == name && *b == scope).unwrap_or(0)
}

/// The cell of one cooldown for this node's player / team.
fn cd_cell(gen: &Gen, node: &str, n: &Node) -> String {
    let g = g_of(gen);
    let k = cd_index(g, n);
    let scope = cooldowns(g).get(k).map(|c| c.1).unwrap_or("player");
    let wired = |p: &str| gen.v.data_source(node, p).is_some();
    match scope {
        "player" => {
            let p = if wired("player") { gen.need(node, "player", "player") } else { "0".into() };
            format!("g_lsgCd{k}[clamp({p}, 0, 32)]")
        }
        "team" => {
            let t = if wired("team") { gen.need(node, "team", "team") } else { "0".into() };
            format!("g_lsgCd{k}[clamp({t}, 0, 3)]")
        }
        _ => format!("g_lsgCd{k}[0]"),
    }
}

/// A pure node's output, or None for other kinds.
pub(super) fn pure_value(gen: &Gen, node: &str, n: &Node, def: &'static NodeDef, pin: &str) -> Option<String> {
    if !PURE_KINDS.contains(&def.kind.as_str()) {
        return None;
    }
    let key = |k: &str| pawn_str(&name_text(gen.text(n, def, k)));
    Some(match def.kind.as_str() {
        "in.holding" => format!("(lsg_p5_holding({}, {}) ? 1 : 0)", gen.need(node, "player", "player"), button_bits(gen.choice(n, def, "button"))),
        "cd.ready" => format!("({} <= get_gametime() ? 1 : 0)", cd_cell(gen, node, n)),
        "cd.left" => format!("floatmax({} - get_gametime(), 0.0)", cd_cell(gen, node, n)),
        "save.text_get" => format!("lsg_p5_text_get({}, {})", gen.need(node, "player", "player"), key("key")),
        "save.server_get" => {
            if pin == "text" {
                format!("lsg_p5_stext_get({})", key("key"))
            } else {
                format!("lsg_p5_snum_get({})", key("key"))
            }
        }
        _ => "0".into(),
    })
}

/// The node's code (inside its function), before Then.
#[allow(clippy::too_many_arguments)]
pub(super) fn action(
    gen: &Gen,
    id: &str,
    n: &Node,
    def: &'static NodeDef,
    b: &mut Out,
    inp: &dyn Fn(&str, &str) -> String,
    exit: &dyn Fn(&str) -> String,
    next: &dyn Fn(&str) -> Option<String>,
) {
    let wired = |pin: &str| gen.v.data_source(id, pin).is_some();
    let ex = |b: &mut Out, r: &str| {
        let e = exit(r);
        if !e.is_empty() {
            b.line(e);
        }
    };
    let key = |k: &str| pawn_str(&name_text(gen.text(n, def, k)));
    let on = |k: &str| if gen.boolean(n, def, k) { "true" } else { "false" };
    match def.kind.as_str() {
        "in.tell_bind" => {
            let line = format!("bind {} ^\"{}^\"", key_name(gen.text(n, def, "key")), command_name(gen.text(n, def, "command")));
            let say = format!("client_print(p, print_chat, \"To use it, open the console (~) and type: {line}\")");
            if wired("player") {
                b.line(format!("\tnew p = {}\n\tif (lsg_ok(p) && !is_user_bot(p))\n\t\t{say}", inp("player", "player")));
            } else {
                b.line(format!("\tfor (new p = 1; p <= MaxClients; p++)\n\t\tif (is_user_connected(p) && !is_user_bot(p))\n\t\t\t{say}"));
            }
            ex(b, "shown");
        }
        "cd.start" => {
            let cell = cd_cell(gen, id, n);
            b.line(format!("\t{cell} = get_gametime() + floatmax({}, 0.0)", inp("secs", "number")));
            ex(b, "started");
        }
        "cd.reset" => {
            let cell = cd_cell(gen, id, n);
            b.line(format!("\t{cell} = 0.0"));
            ex(b, "reset");
        }
        "wpn.infinite_ammo" => {
            let what = match gen.choice(n, def, "what") {
                "clip" => 1,
                "reserve" => 2,
                _ => 3,
            };
            let v = if gen.boolean(n, def, "on") { what } else { 0 };
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t{{\n\t\tg_lsgP5Ammo[p] = {v}\n\t\tg_lsgP5AmmoReset[p] = {}\n\t\tlsg_p5_fill(p)\n\t}}", inp("player", "player"), on("reset")));
            ex(b, if v == 0 { "off" } else { "on" });
        }
        "wpn.fire_rate" | "wpn.recoil" => {
            let (arr, reset, lo, hi) = if def.kind == "wpn.fire_rate" { ("g_lsgP5Rate", "g_lsgP5RateReset", "0.1", "10.0") } else { ("g_lsgP5Kick", "g_lsgP5KickReset", "0.0", "5.0") };
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t{{\n\t\t{arr}[p] = floatclamp({}, {lo}, {hi})\n\t\t{reset}[p] = {}\n\t}}", inp("player", "player"), inp("mult", "number"), on("reset")));
            ex(b, "set");
        }
        "save.text_set" => {
            b.line(format!("\tlsg_p5_text_set({}, {}, {})", inp("player", "player"), key("key"), inp("text", "text")));
            ex(b, "saved");
        }
        "save.server_set" => {
            if gen.choice(n, def, "what") == "text" {
                let t = if wired("text") { inp("text", "text") } else { "\"\"".into() };
                b.line(format!("\tlsg_p5_stext_set({}, {t})", key("key")));
            } else {
                b.line(format!("\tlsg_p5_snum_set({}, {})", key("key"), inp("value", "number")));
            }
            ex(b, "saved");
        }
        "save.top" => {
            let most = (gen.num(n, def, "count").round() as i64).clamp(1, 1000);
            let low = if gen.choice(n, def, "order") == "low" { "true" } else { "false" };
            b.line(format!("\tnew count = lsg_p5_top({}, {most}, {low})", key("key")));
            let set = |pin: &str, what: &str| -> String {
                match (gen.out_index(id, pin), gen.tout_index(id, pin)) {
                    (_, Some(t)) => format!("\t\tcopy(g_lsgOT[{t}], 63, {what})"),
                    (Some(c), _) => format!("\t\tg_lsgO[{c}] = _:{what}"),
                    _ => String::new(),
                }
            };
            b.line("\tfor (new r = 0; r < count; r++)\n\t{");
            for l in [set("rank", "float(r + 1)"), set("name", "g_lsgP5TopName[r]"), set("value", "g_lsgP5TopVal[r]")] {
                if !l.is_empty() {
                    b.line(l);
                }
            }
            match next("each") {
                Some(c) => b.line(format!("\t\t{c}")),
                None => b.line("\t\tcontinue"),
            }
            b.line("\t}");
            ex(b, "listed");
        }
        _ => {}
    }
}

/// The shared stocks (only stocks: unused ones aren't compiled) and the tables the hooks need.
pub(super) fn helpers(g: &Graph, o: &mut Out) {
    if !used(g) {
        return;
    }
    o.line("// ---- player input, cooldowns, pickups, weapon tuning, saving (P5) ----");
    let cds = cooldowns(g);
    for (k, (name, scope)) in cds.iter().enumerate() {
        let size = match *scope {
            "player" => 33,
            "team" => 4,
            _ => 1,
        };
        o.line(format!("stock Float:g_lsgCd{k}[{size}] // cooldown {} ({scope})", comment_text(name)));
    }
    o.line(PAWN_INPUT);
    if uses_weapons(g) {
        o.line(PAWN_WEAPONS);
    }
    if uses_saved_text(g) {
        let most = g.nodes.iter().filter(|n| n.kind == "save.top").map(|n| n.params.get("count").and_then(Value::as_f64).unwrap_or(10.0).round().clamp(1.0, 1000.0) as usize).max().unwrap_or(1);
        o.line(format!("#define LSG_P5TOP {most}"));
        o.line(PAWN_SAVED);
    }
}

/// Inside plugin_init.
pub(super) fn init(gen: &Gen, o: &mut Out) {
    let g = g_of(gen);
    if !used(g) {
        return;
    }
    let live = |k: &str| gen.live.iter().any(|id| gen.v.defs[*id].kind == k);
    if live("in.pressed") {
        o.line("\tregister_forward(FM_PlayerPreThink, \"lsg_p5_think\", 0)");
    }
    for id in gen.live.iter().filter(|id| gen.v.defs[**id].kind == "in.command") {
        let cmd = command_name(gen.text(gen.v.nodes[id], gen.v.defs[id], "command"));
        o.line(format!("\tregister_clcmd({}, \"lsg_p5cmd{}\")", pawn_str(&cmd), gen.index(id)));
    }
    if live("item.picked_up") {
        o.line("\tRegisterHam(Ham_Touch, \"weaponbox\", \"lsg_p5_touch\", 0)\n\tRegisterHam(Ham_Touch, \"armoury_entity\", \"lsg_p5_touch\", 0)\n\tRegisterHam(Ham_Touch, \"armoury_entity\", \"lsg_p5_touched\", 1)\n\tRegisterHamPlayer(Ham_AddPlayerItem, \"lsg_p5_added\", 1)");
    }
    if live("item.dropped") {
        o.line("\tregister_forward(FM_SetModel, \"lsg_p5_setmodel\", 1)");
    }
    if !cooldowns(g).is_empty() || uses_weapons(g) {
        o.line("\tregister_forward(FM_ClientDisconnect, \"lsg_p5_left\", 0)");
    }
    if uses_weapons(g) {
        for w in GUNS {
            o.line(format!("\tRegisterHam(Ham_Weapon_PrimaryAttack, \"weapon_{w}\", \"lsg_p5_fire\", 0)\n\tRegisterHam(Ham_Weapon_PrimaryAttack, \"weapon_{w}\", \"lsg_p5_fired\", 1)"));
        }
        o.line("\tregister_event(\"CurWeapon\", \"lsg_p5_curweapon\", \"be\", \"1=1\")\n\tRegisterHamPlayer(Ham_Killed, \"lsg_p5_died\", 1)");
    }
    if uses_saved_text(g) {
        o.line("\tlsg_p5_text_load()\n\tset_task(5.0, \"lsg_p5_text_flush\", LSG_TASK - 610, _, _, \"b\")\n\tRegisterHamPlayer(Ham_Spawn, \"lsg_p5_seen\", 1)");
    }
}

/// The event handlers (after plugin_init).
pub(super) fn handlers(gen: &Gen, o: &mut Out) {
    let v = &gen.v;
    let of = |k: &str| -> Vec<&str> { gen.live.iter().copied().filter(|id| v.defs[*id].kind == k).collect() };
    let presses = of("in.pressed");
    if !presses.is_empty() {
        o.line("// Player Pressed Button: what changed since the last frame (it arrives with the player's ping)\npublic lsg_p5_think(id)\n{\n\tif (!is_user_alive(id))\n\t\treturn FMRES_IGNORED\n\tnew now = pev(id, pev_button), was = pev(id, pev_oldbuttons)\n\tif (now == was)\n\t\treturn FMRES_IGNORED\n\tnew down = now & ~was, up = was & ~now, bot = is_user_bot(id)");
        for id in &presses {
            let (n, d) = gen.node(id);
            let bits = button_bits(gen.choice(n, d, "button"));
            let edge = if gen.choice(n, d, "when") == "released" { "up" } else { "down" };
            let bots = if gen.boolean(n, d, "bots") { String::new() } else { " && !bot".into() };
            let only = if v.data_source(id, "only").is_some() { format!(" && lsg_p5_only(id, {})", gen.need(id, "only", "player")) } else { String::new() };
            o.line(format!("\tif (({edge} & {bits}){bots}{only})\n\t{{\n\t\tnew ctx[LSG_CTX]\n\t\tctx[0] = id // player\n\t\tlsg_p5_btn_name({edge} & {bits}, g_lsgTxt[0], 63) // button\n\t\t{}(ctx)\n\t}}", gen.fname(id)));
        }
        o.line("\t#pragma unused bot\n\t#pragma unused down\n\t#pragma unused up\n\treturn FMRES_IGNORED\n}\n");
    }
    for id in of("in.command") {
        let (n, d) = gen.node(id);
        let rate = gen.num(n, d, "rate").clamp(1.0, 1000.0);
        let gap = float_lit(1.0 / rate);
        o.line(format!(
            "// Player Command {}: the rest of the line is only ever text, never run; at most {} a second per player\npublic lsg_p5cmd{}(id)\n{{\n\tstatic Float:last[33]\n\tif (id < 1 || id > MaxClients)\n\t\treturn PLUGIN_HANDLED\n\tnew Float:t = get_gametime()\n\tif (t >= last[id] && t - last[id] < {gap})\n\t\treturn PLUGIN_HANDLED\n\tlast[id] = t\n\tnew ctx[LSG_CTX]\n\tctx[0] = id // player\n\tread_args(g_lsgTxt[0], 63) // args\n\tremove_quotes(g_lsgTxt[0])\n\tlsg_p5_plain(g_lsgTxt[0])\n\t{}(ctx)\n\treturn PLUGIN_HANDLED\n}}\n",
            comment_text(&command_name(gen.text(n, d, "command"))),
            rate.round() as i64,
            gen.index(id),
            gen.fname(id)
        ));
    }
    let picks = of("item.picked_up");
    if !picks.is_empty() {
        let calls: String = picks.iter().map(|id| format!("\n\tlsg_p5_pick_call(id, item, {})", gen.index(id))).collect();
        let cases: String = picks.iter().map(|id| format!("\n\t\tcase {}: {}(ctx)", gen.index(id), gen.fname(id))).collect();
        o.line(format!("// Picked Up: a weapon (the touch that came just before it was added) or armor from the ground\npublic lsg_p5_touch(ent, id)\n{{\n\tif (id >= 1 && id <= MaxClients && is_user_alive(id))\n\t{{\n\t\tg_lsgP5TouchAt[id] = get_gametime()\n\t\tg_lsgP5Armor[id] = get_user_armor(id)\n\t}}\n\treturn HAM_IGNORED\n}}\n\npublic lsg_p5_added(id, item)\n{{\n\tif (id < 1 || id > MaxClients || !pev_valid(item) || get_gametime() != g_lsgP5TouchAt[id])\n\t\treturn HAM_IGNORED\n\tnew item_name[32]\n\tpev(item, pev_classname, item_name, charsmax(item_name))\n\tif (equal(item_name, \"weapon_\", 7))\n\t\tlsg_p5_picked(id, item_name[7])\n\telse\n\t\tlsg_p5_picked(id, item_name)\n\treturn HAM_IGNORED\n}}\n\npublic lsg_p5_touched(ent, id)\n{{\n\tif (id >= 1 && id <= MaxClients && is_user_alive(id) && get_gametime() == g_lsgP5TouchAt[id] && get_user_armor(id) > g_lsgP5Armor[id])\n\t\tlsg_p5_picked(id, \"kevlar\")\n\treturn HAM_IGNORED\n}}\n\nlsg_p5_picked(id, const item[])\n{{\n\tif (g_lsgP5Picking)\n\t\treturn\n\tg_lsgP5Picking = true;{calls}\n\tg_lsgP5Picking = false\n}}\n\nlsg_p5_pick_call(id, const item[], node)\n{{\n\tnew ctx[LSG_CTX]\n\tctx[0] = id\n\tcopy(g_lsgTxt[0], 63, item)\n\tswitch (node)\n\t{{{cases}\n\t}}\n}}\n"));
    }
    let drops = of("item.dropped");
    if !drops.is_empty() {
        let mut body = String::new();
        let drop_cases: String = drops.iter().map(|id| format!("\n\t\tcase {}: {}(ctx)", gen.index(id), gen.fname(id))).collect();
        for id in &drops {
            let (n, d) = gen.node(id);
            let cond = if gen.boolean(n, d, "death") { "is_user_connected(owner)" } else { "is_user_alive(owner)" };
            body.push_str(&format!("\n\tif ({cond})\n\t\tlsg_p5_drop_call(owner, w, {})", gen.index(id)));
        }
        o.line(format!("// Dropped: a weaponbox gets its model with its owner set (G, Drop Weapon, a death)\npublic lsg_p5_setmodel(ent, const model[])\n{{\n\tif (!pev_valid(ent))\n\t\treturn FMRES_IGNORED\n\tnew cls[12]\n\tpev(ent, pev_classname, cls, charsmax(cls))\n\tif (!equal(cls, \"weaponbox\"))\n\t\treturn FMRES_IGNORED\n\tnew owner = pev(ent, pev_owner)\n\tif (owner < 1 || owner > MaxClients || !is_user_connected(owner) || !equal(model, \"models/w_\", 9))\n\t\treturn FMRES_IGNORED\n\tnew w[24]\n\tcopy(w, charsmax(w), model[9])\n\tnew dot = contain(w, \".\")\n\tif (dot > 0)\n\t\tw[dot] = 0\n\tif (equal(w, \"backpack\"))\n\t\tcopy(w, charsmax(w), \"c4\"){body}\n\treturn FMRES_IGNORED\n}}\n\nlsg_p5_drop_call(owner, const w[], node)\n{{\n\tnew ctx[LSG_CTX]\n\tctx[0] = owner\n\tcopy(g_lsgTxt[0], 63, w)\n\tswitch (node)\n\t{{{drop_cases}\n\t}}\n}}\n"));
    }
    let g = g_of(gen);
    if !cooldowns(g).is_empty() || uses_weapons(g) {
        let mut f = String::from("// a slot that's left starts clean for whoever gets it next\npublic lsg_p5_left(id)\n{\n\tif (id < 1 || id > MaxClients)\n\t\treturn FMRES_IGNORED");
        for (k, (_, scope)) in cooldowns(g).iter().enumerate() {
            if *scope == "player" {
                f.push_str(&format!("\n\tg_lsgCd{k}[id] = 0.0"));
            }
        }
        if uses_weapons(g) {
            f.push_str("\n\tlsg_p5_wclear(id)");
        }
        f.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(f);
    }
}

/// Inside plugin_end.
pub(super) fn end(g: &Graph, o: &mut Out) {
    if uses_saved_text(g) {
        o.line("\tlsg_p5_text_flush()");
    }
}

pub(super) fn needs_end(g: &Graph) -> bool {
    uses_saved_text(g)
}

const GUNS: [&str; 24] = [
    "glock18", "usp", "p228", "deagle", "elite", "fiveseven", "m3", "xm1014", "mac10", "tmp", "mp5navy", "ump45", "p90", "galil", "famas", "ak47", "m4a1", "sg552", "aug", "scout", "awp", "g3sg1", "sg550", "m249",
];

const PAWN_INPUT: &str = r#"stock bool:g_lsgP5Picking, Float:g_lsgP5TouchAt[33], g_lsgP5Armor[33]

stock bool:lsg_p5_holding(id, bits)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return false
	return (pev(id, pev_button) & bits) != 0
}

// Only this player: none wired in (0) means nobody, so nothing runs
stock bool:lsg_p5_only(id, who)
	return who > 0 && id == who

// the first button of these bits, by name
stock lsg_p5_btn_name(bits, out[], len)
{
	static const b[] = { 1, 2, 4, 8, 16, 32, 512, 1024, 2048, 4096, 8192 }
	static const names[][] = { "attack", "jump", "duck", "forward", "back", "use", "left", "right", "attack2", "walk", "reload" }
	for (new k = 0; k < sizeof b; k++)
	{
		if (bits & b[k])
		{
			copy(out, len, names[k])
			return
		}
	}
	out[0] = 0
}

// a command's arguments as plain text: control characters out (it's only ever text)
stock lsg_p5_plain(s[])
{
	for (new i = 0; s[i]; i++)
		if (s[i] < 32)
			s[i] = ' '
	trim(s)
}"#;

const PAWN_WEAPONS: &str = r#"stock g_lsgP5Ammo[33], bool:g_lsgP5AmmoReset[33], Float:g_lsgP5Rate[33] = { 1.0, ... }, bool:g_lsgP5RateReset[33], Float:g_lsgP5Kick[33] = { 1.0, ... }, bool:g_lsgP5KickReset[33]
stock g_lsgP5Clip[33], Float:g_lsgP5Punch[33][3]
// the most spare ammo of each weapon (CSW_ numbers), as the game gives it
stock const g_lsgP5MaxBp[31] = { 0, 52, 0, 90, 0, 32, 0, 100, 90, 0, 120, 100, 100, 90, 90, 90, 100, 120, 30, 120, 200, 32, 90, 120, 90, 0, 35, 90, 90, 0, 100 }

stock lsg_p5_wclear(id)
{
	g_lsgP5Ammo[id] = 0
	g_lsgP5Rate[id] = 1.0
	g_lsgP5Kick[id] = 1.0
}

stock lsg_p5_fill(id)
{
	if (!is_user_alive(id) || !(g_lsgP5Ammo[id] & 2))
		return
	new clip, ammo, w = get_user_weapon(id, clip, ammo)
	if (w > 0 && w < sizeof g_lsgP5MaxBp && g_lsgP5MaxBp[w] > 0)
		cs_set_user_bpammo(id, w, g_lsgP5MaxBp[w])
}

public lsg_p5_curweapon(id)
{
	if (g_lsgP5Ammo[id] & 2)
		lsg_p5_fill(id)
}

public lsg_p5_fire(ent)
{
	new id = get_ent_data_entity(ent, "CBasePlayerItem", "m_pPlayer")
	if (id < 1 || id > MaxClients)
		return HAM_IGNORED
	if (g_lsgP5Ammo[id] & 1)
		g_lsgP5Clip[id] = cs_get_weapon_ammo(ent)
	if (g_lsgP5Kick[id] != 1.0)
		pev(id, pev_punchangle, g_lsgP5Punch[id])
	return HAM_IGNORED
}

public lsg_p5_fired(ent)
{
	new id = get_ent_data_entity(ent, "CBasePlayerItem", "m_pPlayer")
	if (id < 1 || id > MaxClients)
		return HAM_IGNORED
	if ((g_lsgP5Ammo[id] & 1) && g_lsgP5Clip[id] > 0 && cs_get_weapon_ammo(ent) < g_lsgP5Clip[id])
		cs_set_weapon_ammo(ent, g_lsgP5Clip[id])
	if (g_lsgP5Rate[id] != 1.0)
	{
		new Float:next = get_ent_data_float(ent, "CBasePlayerWeapon", "m_flNextPrimaryAttack")
		if (next > 0.0)
			set_ent_data_float(ent, "CBasePlayerWeapon", "m_flNextPrimaryAttack", next / g_lsgP5Rate[id])
	}
	if (g_lsgP5Kick[id] != 1.0)
	{
		new Float:now[3]
		pev(id, pev_punchangle, now)
		for (new k = 0; k < 3; k++)
			now[k] = g_lsgP5Punch[id][k] + (now[k] - g_lsgP5Punch[id][k]) * g_lsgP5Kick[id]
		set_pev(id, pev_punchangle, now)
	}
	return HAM_IGNORED
}

public lsg_p5_died(id)
{
	if (id < 1 || id > MaxClients)
		return HAM_IGNORED
	if (g_lsgP5AmmoReset[id])
		g_lsgP5Ammo[id] = 0
	if (g_lsgP5RateReset[id])
		g_lsgP5Rate[id] = 1.0
	if (g_lsgP5KickReset[id])
		g_lsgP5Kick[id] = 1.0
	return HAM_IGNORED
}"#;

/// Saved text (per player and server-wide) and Top Players: like Saved Value's file (a line
/// "key<TAB>text" each in addons/amxmodx/data/studio/<mode>.savedtext, written within 5 s, again at
/// the map's end, through a .new file renamed over the old one so a crash leaves one whole file).
/// Players' names are kept there too (@nm@<SteamID>), so Top Players can name offline players.
const PAWN_SAVED: &str = r#"stock Trie:g_lsgP5Txt = Invalid_Trie, bool:g_lsgP5TxtDirty
stock g_lsgP5TopName[LSG_P5TOP][32], Float:g_lsgP5TopVal[LSG_P5TOP]

stock lsg_p5_text_path(out[], len, const ext[])
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(out, len, "%s/studio/%s.savedtext%s", dir, LSG_MODE, ext)
}

stock lsg_p5_text_load()
{
	g_lsgP5Txt = TrieCreate()
	new path[192], line[200], key[128]
	lsg_p5_text_path(path, charsmax(path), "")
	if (!file_exists(path))
		lsg_p5_text_path(path, charsmax(path), ".new")
	new f = fopen(path, "rt")
	if (!f)
		return
	while (fgets(f, line, charsmax(line)))
	{
		new len = strlen(line)
		while (len > 0 && (line[len - 1] == '^n' || line[len - 1] == '^r'))
			line[--len] = 0
		new tab = contain(line, "^t")
		if (tab <= 0)
			continue
		line[tab] = 0
		copy(key, charsmax(key), line)
		TrieSetString(g_lsgP5Txt, key, line[tab + 1])
	}
	fclose(f)
}

public lsg_p5_text_flush()
{
	if (!g_lsgP5TxtDirty || g_lsgP5Txt == Invalid_Trie)
		return
	new path[192], tmp[200], dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_p5_text_path(path, charsmax(path), "")
	lsg_p5_text_path(tmp, charsmax(tmp), ".new")
	new f = fopen(tmp, "wt")
	if (!f)
		return
	new Snapshot:snap = TrieSnapshotCreate(g_lsgP5Txt), key[128], val[64]
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, key, charsmax(key))
		TrieGetString(g_lsgP5Txt, key, val, charsmax(val))
		fprintf(f, "%s^t%s^n", key, val)
	}
	TrieSnapshotDestroy(snap)
	fclose(f)
	if (!rename_file(tmp, path, 1))
	{
		delete_file(path)
		rename_file(tmp, path, 1)
	}
	g_lsgP5TxtDirty = false
}

// text for the file: one line, no tabs or control characters, at most 63 letters
stock lsg_p5_clean(const s[], out[], len)
{
	copy(out, len, s)
	for (new i = 0; out[i]; i++)
		if (out[i] < 32)
			out[i] = ' '
}

stock lsg_p5_text_get(id, const key[])
{
	new s[64], k[96]
	if (g_lsgP5Txt == Invalid_Trie || !lsg_ok(id))
		return s
	lsg_saved_key(id, key, k, charsmax(k))
	TrieGetString(g_lsgP5Txt, k, s, charsmax(s))
	return s
}

stock lsg_p5_text_set(id, const key[], const text[])
{
	if (g_lsgP5Txt == Invalid_Trie || !lsg_ok(id))
		return
	new k[96], v[64]
	lsg_saved_key(id, key, k, charsmax(k))
	lsg_p5_clean(text, v, charsmax(v))
	TrieSetString(g_lsgP5Txt, k, v)
	g_lsgP5TxtDirty = true
}

stock lsg_p5_stext_get(const key[])
{
	new s[64], k[96]
	formatex(k, charsmax(k), "%s@@server", key)
	if (g_lsgP5Txt != Invalid_Trie)
		TrieGetString(g_lsgP5Txt, k, s, charsmax(s))
	return s
}

stock lsg_p5_stext_set(const key[], const text[])
{
	if (g_lsgP5Txt == Invalid_Trie)
		return
	new k[96], v[64]
	formatex(k, charsmax(k), "%s@@server", key)
	lsg_p5_clean(text, v, charsmax(v))
	TrieSetString(g_lsgP5Txt, k, v)
	g_lsgP5TxtDirty = true
}

stock Float:lsg_p5_snum_get(const key[])
{
	new k[96], Float:v
	formatex(k, charsmax(k), "%s@@server", key)
	if (g_lsgSaved == Invalid_Trie || !TrieGetCell(g_lsgSaved, k, v))
		return 0.0
	return v
}

stock lsg_p5_snum_set(const key[], Float:value)
{
	if (g_lsgSaved == Invalid_Trie)
		return
	new k[96]
	formatex(k, charsmax(k), "%s@@server", key)
	TrieSetCell(g_lsgSaved, k, value)
	g_lsgSavedDirty = true
}

// a player's name for Top Players, kept by their SteamID (bots and LAN players are saved by name already)
public lsg_p5_seen(id)
{
	if (!is_user_alive(id) || is_user_bot(id) || g_lsgP5Txt == Invalid_Trie)
		return HAM_IGNORED
	new who[44], k[64], name[32], had[32]
	get_user_authid(id, who, charsmax(who))
	if (!equal(who, "STEAM_", 6))
		return HAM_IGNORED
	formatex(k, charsmax(k), "@nm@%s", who)
	get_user_name(id, name, charsmax(name))
	lsg_p5_clean(name, name, charsmax(name))
	if (!TrieGetString(g_lsgP5Txt, k, had, charsmax(had)) || !equal(had, name))
	{
		TrieSetString(g_lsgP5Txt, k, name)
		g_lsgP5TxtDirty = true
	}
	return HAM_IGNORED
}

// the best `most` players by a saved value ("key@<SteamID>" or "key@name:<name>"), into
// g_lsgP5TopName / g_lsgP5TopVal; how many there are
stock lsg_p5_top(const key[], most, bool:low)
{
	most = min(most, LSG_P5TOP)
	if (g_lsgSaved == Invalid_Trie)
		return 0
	new Snapshot:snap = TrieSnapshotCreate(g_lsgSaved), k[128], Float:v, count, klen = strlen(key)
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, k, charsmax(k))
		// this key's, and a player's (server values are key@@server)
		if (!equal(k, key, klen) || k[klen] != '@' || k[klen + 1] == '@')
			continue
		TrieGetCell(g_lsgSaved, k, v)
		// where it goes in the list so far (the worst one drops off a full list)
		new at = count
		while (at > 0 && (low ? v < g_lsgP5TopVal[at - 1] : v > g_lsgP5TopVal[at - 1]))
			at--
		if (at >= most)
			continue
		for (new j = min(count, most - 1); j > at; j--)
		{
			g_lsgP5TopVal[j] = g_lsgP5TopVal[j - 1]
			copy(g_lsgP5TopName[j], 31, g_lsgP5TopName[j - 1])
		}
		g_lsgP5TopVal[at] = v
		new who[96]
		copy(who, charsmax(who), k[klen + 1])
		if (equal(who, "name:", 5))
			copy(g_lsgP5TopName[at], 31, who[5])
		else
		{
			new nk[110]
			formatex(nk, charsmax(nk), "@nm@%s", who)
			if (g_lsgP5Txt == Invalid_Trie || !TrieGetString(g_lsgP5Txt, nk, g_lsgP5TopName[at], 31))
				copy(g_lsgP5TopName[at], 31, who)
		}
		if (count < most)
			count++
	}
	TrieSnapshotDestroy(snap)
	return count
}"#;

fn problem(level: &'static str, node: &str, msg: &str) -> Problem {
    Problem { level, node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

/// The Problems checks. Comfort limits (Most per second, Most places) only warn when raised a lot;
/// a command name that isn't ls_ and letters is an error (it's a security rule, not a comfort one).
pub(crate) fn problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    let num = |n: &Node, k: &str, d: f64| n.params.get(k).and_then(Value::as_f64).unwrap_or(d);
    let txt = |n: &Node, k: &str, d: &str| n.params.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let commands: Vec<String> = g.nodes.iter().filter(|n| n.kind == "in.command").map(|n| txt(n, "command", "ls_dash")).collect();
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        let wired = |pin: &str| v.data_source(&n.id, pin).is_some();
        match def.kind.as_str() {
            "in.command" | "in.tell_bind" => {
                let c = txt(n, "command", "ls_dash");
                if command_name(&c) != c {
                    p.push(problem("error", &n.id, "Command is ls_ and small letters only (like ls_dash), at most 31 characters: it can't be any of the game's own commands."));
                }
                if def.kind == "in.command" {
                    let r = num(n, "rate", 5.0);
                    if r > 20.0 {
                        p.push(problem("warning", &n.id, &format!("Most per second is {r}: a player holding a key bound to it can run the flow that often. 5 is the usual; raise it only if the flow is cheap.")));
                    }
                    if commands.iter().filter(|x| **x == c).count() > 1 {
                        p.push(problem("warning", &n.id, "Another Player Command has this command: both run each time."));
                    }
                } else if !commands.contains(&c) {
                    p.push(problem("warning", &n.id, "No Player Command in this mode listens for this command: the bind would do nothing."));
                }
            }
            "in.pressed" => {
                if let Some((src, _)) = v.data_source(&n.id, "only") {
                    if v.defs.get(src).is_some_and(|d| d.event || !d.pure) {
                        p.push(problem("warning", &n.id, "Only this player: wire a player from a variable (set when they're picked), not from another flow's output; that one isn't known here, so nothing would run."));
                    }
                }
                if txt(n, "button", "use") == "any" && txt(n, "when", "pressed") == "released" {
                    p.push(problem("warning", &n.id, "Any button, released: this runs every time a player lets go of a move key too, many times a second while they play."));
                }
            }
            "cd.start" | "cd.ready" | "cd.left" | "cd.reset" => match txt(n, "scope", "player").as_str() {
                "team" if !wired("team") => p.push(problem("warning", &n.id, "For each team, but no Team wired in: this is one cooldown for everyone (like For: Everyone).")),
                "global" => {}
                "team" => {}
                _ if !wired("player") => p.push(problem("warning", &n.id, "For each player, but no Player wired in: this is one cooldown for everyone (like For: Everyone).")),
                _ => {}
            },
            "save.top" => {
                let most = num(n, "count", 10.0);
                if most > 50.0 {
                    p.push(problem("warning", &n.id, &format!("Most is {most}: Each runs that many times in one go, and the list is worked out from the whole save file. 10 is the usual.")));
                }
                let key = txt(n, "key", "wins");
                if !g.nodes.iter().any(|m| matches!(m.kind.as_str(), "save.set" | "save.add") && m.params.get("key").and_then(Value::as_str).unwrap_or("wins") == key) {
                    p.push(problem("warning", &n.id, &format!("Nothing in this mode saves \"{key}\" (Save Value or Add To Saved with that name): the list stays empty.")));
                }
            }
            "wpn.fire_rate" => {
                if !wired("mult") && num(n, "mult", 2.0) > 4.0 {
                    p.push(problem("warning", &n.id, "Over 4 times as fast: some guns stop animating right and bots aim worse with it."));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::tools::studio::graph::{parse, registry};
    use serde_json::json;

    pub const EVERY_NODE_P5: &str = include_str!("../testdata/every_node_p5.graph.json");

    /// Every node of this block, in its own graph: it checks clean, every flow node is in the
    /// plugin, and the source has the hooks each needs (and nothing else's).
    #[test]
    fn every_p5_node_generates() {
        let g = parse(EVERY_NODE_P5).unwrap();
        let probs = super::super::super::graph::validate_core(&g);
        assert!(probs.is_empty(), "{probs:#?}");
        let used: BTreeSet<&str> = g.nodes.iter().map(|n| n.kind.as_str()).collect();
        for k in kinds() {
            assert!(used.contains(k), "every_node_p5.graph.json lacks {k}");
        }
        for debug in [false, true] {
            let r = generate(&g, debug);
            for n in g.nodes.iter().filter(|n| { let d = registry().get(&n.kind).unwrap(); !d.pure && !d.reroute }) {
                assert!(r.spans.iter().any(|s| s.node == n.id), "{} isn't in the plugin", n.id);
            }
            let s = &r.source;
            assert!(s.contains("register_forward(FM_PlayerPreThink, \"lsg_p5_think\", 0)"));
            assert!(s.contains("register_clcmd(\"ls_dash\", \"lsg_p5cmd"));
            assert!(s.contains("RegisterHamPlayer(Ham_AddPlayerItem, \"lsg_p5_added\", 1)") && s.contains("register_forward(FM_SetModel, \"lsg_p5_setmodel\", 1)"));
            assert!(s.contains("RegisterHam(Ham_Weapon_PrimaryAttack, \"weapon_ak47\", \"lsg_p5_fired\", 1)"));
            assert!(s.contains("lsg_p5_text_load()") && s.contains("lsg_saved_load()"), "saved text and the shared saved numbers");
            assert!(s.contains("To use it, open the console (~) and type: bind f ^\"ls_dash^\""));
        }
    }

    /// Nothing of this block is written into a plugin that has none of its nodes.
    #[test]
    fn other_modes_dont_change() {
        let g = parse(super::super::tests::EVERY_NODE_3).unwrap();
        let s = generate(&g, false).source;
        assert!(!s.contains("lsg_p5_"), "a P5 helper leaked into every_node_3");
    }

    /// Player Command: only ls_ and small letters become a command; the arguments are text that's
    /// never run, and each player is rate-limited.
    #[test]
    fn player_commands_are_safe() {
        assert_eq!(command_name("ls_dash"), "ls_dash");
        for bad in ["quit", "ls_", "ls_Dash", "ls_dash;quit", "say", "ls_d4sh", "amx_kick", &format!("ls_{}", "a".repeat(40))] {
            assert_eq!(command_name(bad), "ls_command", "{bad}");
        }
        let g = parse(EVERY_NODE_P5).unwrap();
        let s = generate(&g, false).source;
        let h = &s[s.find("public lsg_p5cmd").unwrap()..];
        let h = &h[..h.find("\n}\n").unwrap()];
        assert!(h.contains("read_args(g_lsgTxt[0], 63)") && h.contains("lsg_p5_plain(g_lsgTxt[0])") && h.contains("return PLUGIN_HANDLED"));
        assert!(h.contains("t - last[id] <"), "rate-limited per player");
        assert!(!h.contains("server_cmd") && !h.contains("client_cmd"), "never run");
        // a hostile command name is refused by the checks and never reaches register_clcmd as typed
        let mut v: Value = serde_json::from_str(EVERY_NODE_P5).unwrap();
        for n in v["nodes"].as_array_mut().unwrap() {
            if n["type"] == "in.command" {
                n["params"]["command"] = json!("quit\"); server_cmd(\"quit");
            }
        }
        let g2 = parse(&v.to_string()).unwrap();
        assert!(super::super::super::graph::validate(&g2).iter().any(|p| p.level == "error" && p.message.contains("ls_ and small letters")));
        let s2 = generate(&g2, false).source;
        assert!(!s2.contains("server_cmd(\"quit"));
    }
}
