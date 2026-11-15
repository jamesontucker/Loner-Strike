//! HUD & View: the engine nodes (Y). Hide HUD Parts, Set Field of View, Set Weapon View Model,
//! Flashlight, Night Vision, Silent Footsteps, Scoreboard Tag, Scoreboard Team Color, Force
//! Spectate, Play Soundtrack. All server side, through the game's own switches (CBasePlayer
//! fields by name from AMX Mod X's gamedata, so the offsets stay the game's) and messages.
//!
//! Kept out of gen.rs on purpose (others edit it): gen.rs calls `is_action`/`action` for the
//! node code, `blocks` for the helpers and hooks, `init` inside plugin_init, `precache` inside
//! plugin_precache; graph.rs calls `problems`.

use super::{model_text, pawn_str, Gen, Out};
use crate::tools::studio::graph::{Graph, Node, NodeDef, Problem, View};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) const KINDS: [&str; 10] = [
    "eng.hide_hud",
    "eng.fov",
    "eng.view_model",
    "eng.flashlight",
    "eng.nightvision",
    "eng.footsteps",
    "eng.score_tag",
    "eng.score_color",
    "eng.spectate",
    "eng.soundtrack",
];

/// The weapons the view model hook follows (all of the game's, as they deploy).
const WEAPON_CLASSES: [&str; 29] = [
    "knife", "glock18", "usp", "p228", "deagle", "elite", "fiveseven", "m3", "xm1014", "mac10", "tmp", "mp5navy", "ump45", "p90", "galil", "famas", "ak47", "m4a1", "sg552",
    "aug", "scout", "awp", "g3sg1", "sg550", "m249", "hegrenade", "flashbang", "smokegrenade", "c4",
];

/// Get Bone Position / Get Attachment Position (pure: Entities, in this file too).
const POINT_KINDS: [&str; 2] = ["ent.bone_pos", "ent.attachment_pos"];

fn uses_points(g: &Graph) -> bool {
    g.nodes.iter().any(|n| POINT_KINDS.contains(&n.kind.as_str()))
}

/// A Get Bone / Attachment Position output: the bone looked up by name (the attachment by number)
/// in the model the entity really has, read from its file (cached); Found is whether it has it.
pub(super) fn pure_value(gen: &Gen, node: &str, n: &Node, def: &'static NodeDef, pin: &str) -> Option<String> {
    if !POINT_KINDS.contains(&def.kind.as_str()) {
        return None;
    }
    let ent = gen.need(node, "ent", "entity");
    let (what, kind) = if def.kind == "ent.bone_pos" {
        let b = gen.text(n, def, "bone");
        let b: String = b.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.".contains(*c)).take(32).collect();
        (pawn_str(&b), 0)
    } else {
        ((gen.num(n, def, "attachment").round() as i64).clamp(0, 3).to_string(), 1)
    };
    let call = if kind == 0 { format!("lsg_mp_bone({ent}, {what})") } else { format!("lsg_mp_att({ent}, {what})") };
    Some(match pin {
        "found" => format!("({call} >= 0 ? 1 : 0)"),
        "angles" => format!("lsg_mp_vec({ent}, {call}, {kind}, 1)"),
        _ => format!("lsg_mp_vec({ent}, {call}, {kind}, 0)"),
    })
}

pub(super) fn is_action(kind: &str) -> bool {
    KINDS.contains(&kind)
}

fn uses(g: &Graph, k: &str) -> bool {
    g.nodes.iter().any(|n| n.kind == k)
}

fn any(g: &Graph) -> bool {
    g.nodes.iter().any(|n| is_action(&n.kind))
}

/// A path setting as a model under models/ (empty when it isn't one).
fn model_of(n: &Node, id: &str) -> String {
    let s = n.params.get(id).and_then(Value::as_str).unwrap_or("");
    if s.is_empty() {
        return String::new();
    }
    let m = model_text(s);
    if m == "models/w_c4.mdl" && !s.eq_ignore_ascii_case("models/w_c4.mdl") {
        String::new()
    } else {
        m
    }
}

/// Every model the view model nodes name, in one list (index + 1 is what a player keeps).
fn view_models(g: &Graph) -> Vec<String> {
    let mut set = BTreeSet::new();
    for n in g.nodes.iter().filter(|n| n.kind == "eng.view_model") {
        for k in ["model", "pmodel"] {
            let m = model_of(n, k);
            if !m.is_empty() {
                set.insert(m);
            }
        }
    }
    set.into_iter().collect()
}

/// The node's code (inside its function, before it goes on).
#[allow(clippy::too_many_arguments)]
pub(super) fn action(gen: &Gen, id: &str, n: &Node, def: &'static NodeDef, b: &mut Out, inp: &dyn Fn(&str, &str) -> String, exit: &dyn Fn(&str) -> String, err: &dyn Fn(&str) -> String) {
    let debug = gen.debug;
    let wired = |pin: &str| gen.v.data_source(id, pin).is_some();
    let ex = |b: &mut Out, r: &str| {
        let e = exit(r);
        if !e.is_empty() {
            b.line(e);
        }
    };
    // a player, or everyone when Player isn't wired
    let each = |b: &mut Out, call: &str| {
        if wired("player") {
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t\t{call}", inp("player", "player")));
        } else {
            b.line(format!("\tfor (new p = 1; p <= MaxClients; p++)\n\t\tif (is_user_connected(p))\n\t\t\t{call}"));
        }
    };
    let on = |k: &str| if gen.boolean(n, def, k) { "true" } else { "false" };
    match def.kind.as_str() {
        "eng.hide_hud" => {
            let mut mask = 0;
            for (k, bit) in [("weapons", 1), ("flashlight", 2), ("radar", 8), ("timer", 16), ("money", 32), ("crosshair", 64)] {
                if gen.boolean(n, def, k) {
                    mask |= bit;
                }
            }
            each(b, &format!("lsg_eng_hud(p, {mask}, {})", if gen.boolean(n, def, "reset") { "false" } else { "true" }));
            ex(b, if mask == 0 { "shown" } else { "hidden" });
        }
        "eng.fov" => {
            let fov = if wired("fov") { format!("clamp(floatround({}), 10, 170)", inp("fov", "number")) } else { (gen.num(n, def, "fov").round() as i64).clamp(10, 170).to_string() };
            b.line(format!("\tnew f = {fov}"));
            each(b, &format!("lsg_eng_fov(p, f, {})", if gen.boolean(n, def, "reset") { "false" } else { "true" }));
            ex(b, "set");
        }
        "eng.view_model" => {
            let list = view_models(super::g_of(gen));
            let k_of = |m: &str| list.iter().position(|x| x == m).map(|i| i + 1).unwrap_or(0);
            let (vm, pm) = (k_of(&model_of(n, "model")), k_of(&model_of(n, "pmodel")));
            let w = gen.choice(n, def, "weapon");
            let csw = if w == "current" { pawn_str("-") } else { pawn_str(&format!("weapon_{w}")) };
            let p = inp("player", "player");
            let call = if gen.boolean(n, def, "restore") { format!("lsg_eng_vm({p}, {csw}, 0, 0)") } else { format!("lsg_eng_vm({p}, {csw}, {vm}, {pm})") };
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Not done: nobody there, nothing in hand, or that model isn't on the server."), exit("set")));
            } else {
                b.line(format!("\t{call}"));
            }
        }
        "eng.flashlight" => {
            let what = match gen.choice(n, def, "do") {
                "off" => 0,
                "allow" => 2,
                "deny" => 3,
                _ => 1,
            };
            let bat = (gen.num(n, def, "battery").round() as i64).clamp(-1, 100);
            let call = format!("lsg_eng_light({}, {what}, {bat})", inp("player", "player"));
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Not done: that player isn't alive."), exit("done")));
            } else {
                b.line(format!("\t{call}"));
            }
        }
        "eng.nightvision" => {
            let what = match gen.choice(n, def, "do") {
                "take" => 0,
                "on" => 2,
                "off" => 3,
                _ => 1,
            };
            let call = format!("lsg_eng_nvg({}, {what})", inp("player", "player"));
            if debug {
                b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Not done: nobody there."), exit("done")));
            } else {
                b.line(format!("\t{call}"));
            }
        }
        "eng.footsteps" => {
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t\tg_lsgQuiet[p] = {}", inp("player", "player"), on("on")));
            ex(b, if gen.boolean(n, def, "on") { "silent" } else { "heard" });
        }
        "eng.score_tag" => {
            let tag = match gen.choice(n, def, "tag") {
                "none" => 0,
                "dead" => 1,
                "bomb" => 2,
                _ => 4,
            };
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t\tlsg_eng_tag(p, {tag}, {})", inp("player", "player"), if gen.boolean(n, def, "reset") { "false" } else { "true" }));
            ex(b, "done");
        }
        "eng.score_color" => {
            let t = match gen.choice(n, def, "team") {
                "t" => 1,
                "ct" => 2,
                "spec" => 3,
                _ => 0,
            };
            b.line(format!("\tnew p = {}\n\tif (lsg_ok(p))\n\t\tlsg_eng_color(p, {t})", inp("player", "player")));
            ex(b, "done");
        }
        "eng.spectate" => {
            let p = inp("player", "player");
            if gen.choice(n, def, "do") == "release" {
                b.line(format!("\tnew p = {p}\n\tif (lsg_ok(p))\n\t\tg_lsgSpec[p] = 0"));
                ex(b, "released");
            } else {
                let t = if wired("target") { inp("target", "player") } else { "0".into() };
                let call = format!("lsg_eng_spec({p}, {t})");
                if debug {
                    b.line(format!("\tif (!{call})\n\t\t{}\n\telse\n\t{}", err("Not done: the watcher is alive (only the dead and spectators can be made to watch), or Target isn't there."), exit("watching")));
                } else {
                    b.line(format!("\t{call}"));
                }
            }
        }
        "eng.soundtrack" => {
            let track: i64 = gen.choice(n, def, "track").parse().unwrap_or(2);
            let stop = gen.choice(n, def, "do") == "stop";
            let call = if stop { "lsg_eng_cd(p, 0, false)".to_string() } else { format!("lsg_eng_cd(p, {}, {})", track.clamp(2, 30), on("loop")) };
            each(b, &call);
            ex(b, if stop { "stopped" } else { "playing" });
        }
        _ => {}
    }
}

/// Helpers and hooks, once, when any of these nodes is used.
pub(super) fn blocks(g: &Graph, o: &mut Out) {
    if uses_points(g) {
        o.line(POINTS);
    }
    if !any(g) {
        return;
    }
    let vms = view_models(g);
    let list = if vms.is_empty() { "\"\"".to_string() } else { vms.iter().map(|m| pawn_str(m)).collect::<Vec<_>>().join(", ") };
    o.line(format!("#define LSG_VMN {}\nstock const g_lsgVmList[LSG_VMN + 1][] = {{ \"\", {list} }}", vms.len().max(1)));
    o.line(RUNTIME);
}

/// Inside plugin_init.
pub(super) fn init(g: &Graph, o: &mut Out) {
    if !any(g) {
        return;
    }
    let spawn = uses(g, "eng.hide_hud") || uses(g, "eng.fov") || uses(g, "eng.score_tag");
    if spawn {
        o.line("\tRegisterHamPlayer(Ham_Spawn, \"lsg_eng_spawn\", 1)\n\tRegisterHamPlayer(Ham_Killed, \"lsg_eng_dead\", 1)");
    }
    if uses(g, "eng.hide_hud") || uses(g, "eng.fov") {
        o.line("\tregister_event(\"ResetHUD\", \"lsg_eng_reset\", \"be\")");
    }
    if uses(g, "eng.fov") || uses(g, "eng.spectate") {
        o.line("\tset_task(0.25, \"lsg_eng_tick\", LSG_TASK - 520, _, _, \"b\")");
    }
    if uses(g, "eng.view_model") {
        for w in WEAPON_CLASSES {
            o.line(format!("\tRegisterHam(Ham_Item_Deploy, \"weapon_{w}\", \"lsg_eng_deploy\", 1)"));
        }
    }
    if uses(g, "eng.flashlight") {
        o.line("\tregister_impulse(100, \"lsg_eng_impulse\")");
    }
    if uses(g, "eng.footsteps") {
        o.line("\tregister_forward(FM_PlayerPreThink, \"lsg_eng_think\", 0)");
    }
    if uses(g, "eng.score_tag") {
        o.line("\tregister_message(get_user_msgid(\"ScoreAttrib\"), \"lsg_eng_attr\")");
    }
    if uses(g, "eng.score_color") {
        o.line("\tregister_message(get_user_msgid(\"TeamInfo\"), \"lsg_eng_teaminfo\")");
    }
    o.line("\tregister_forward(FM_ClientDisconnect, \"lsg_eng_left\", 0)");
}

/// Inside plugin_precache: the view models, only those on the server (a missing file would stop
/// the map loading; the node then leaves the weapon as it is).
pub(super) fn precache(g: &Graph) -> Vec<String> {
    view_models(g).iter().map(|m| format!("\tlsg_pre_model({})", pawn_str(m))).collect()
}

pub(super) fn needs_precache(g: &Graph) -> bool {
    !view_models(g).is_empty()
}

fn err(node: &str, msg: &str) -> Problem {
    Problem { level: "error", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

fn warn(node: &str, msg: &str) -> Problem {
    Problem { level: "warning", node: Some(node.to_string()), pin: None, edge: None, message: msg.into() }
}

/// The Problems checks.
pub(crate) fn problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        match def.kind.as_str() {
            "eng.view_model" => {
                let restore = n.params.get("restore").and_then(Value::as_bool).unwrap_or(false);
                let m = n.params.get("model").and_then(Value::as_str).unwrap_or("models/v_knife.mdl");
                if !restore && model_of(n, "model").is_empty() {
                    p.push(err(&n.id, "View model is a model file: models/...mdl (like models/v_knife.mdl)."));
                } else if !restore && !m.to_ascii_lowercase().rsplit('/').next().unwrap_or("").starts_with("v_") {
                    p.push(warn(&n.id, "A view model is usually models/v_...mdl: other models may float, show arms wrong or not animate."));
                }
                let pm = n.params.get("pmodel").and_then(Value::as_str).unwrap_or("");
                if !pm.is_empty() && model_of(n, "pmodel").is_empty() {
                    p.push(err(&n.id, "Model others see is a model file (models/p_...mdl) or empty."));
                }
            }
            "eng.score_color" => p.push(warn(&n.id, "Only how the player is shown changes: radar, name tags and their own buy menu follow the shown team; damage, spawns and winning keep the real one.")),
            "eng.hide_hud" => {
                let none = ["crosshair", "money", "timer", "radar", "weapons", "flashlight"].iter().all(|k| !super::super::graph::param(n, def, k).and_then(Value::as_bool).unwrap_or(false));
                if none {
                    p.push(warn(&n.id, "Nothing is ticked: this shows the whole HUD again."));
                }
            }
            "eng.fov" => {
                let f = super::super::graph::param(n, def, "fov").and_then(Value::as_f64).unwrap_or(90.0);
                if v.data_source(&n.id, "fov").is_none() && !(60.0..=120.0).contains(&f) {
                    p.push(warn(&n.id, "A field of view under 60 or over 120 can make people feel sick in long play."));
                }
            }
            _ => {}
        }
    }
}

const POINTS: &str = r##"// ---- Get Bone / Attachment Position: a model's bones by name and attachments by number, from
// the entity's own model file (the header's tables), cached per model; -1 when it has none
stock Trie:g_lsgMp = Invalid_Trie

stock lsg_mp_model(ent, m[], len)
{
	m[0] = 0
	if (ent < 1 || !pev_valid(ent))
		return 0
	pev(ent, pev_model, m, len)
	new n = strlen(m)
	return n > 4 && equali(m[n - 4], ".mdl") && pev(ent, pev_modelindex) > 0
}

stock lsg_mp_bone(ent, const name[])
{
	new m[96], key[140], k
	if (!name[0] || !lsg_mp_model(ent, m, charsmax(m)))
		return -1
	formatex(key, charsmax(key), "b|%s|%s", m, name)
	if (g_lsgMp == Invalid_Trie)
		g_lsgMp = TrieCreate()
	if (TrieGetCell(g_lsgMp, key, k))
		return k
	k = lsg_mp_read(m, name, -1)
	TrieSetCell(g_lsgMp, key, k)
	return k
}

stock lsg_mp_att(ent, n)
{
	new m[96], key[140], k
	if (n < 0 || n > 3 || !lsg_mp_model(ent, m, charsmax(m)))
		return -1
	formatex(key, charsmax(key), "a|%s|%d", m, n)
	if (g_lsgMp == Invalid_Trie)
		g_lsgMp = TrieCreate()
	if (TrieGetCell(g_lsgMp, key, k))
		return k
	k = lsg_mp_read(m, "", n)
	TrieSetCell(g_lsgMp, key, k)
	return k
}

// a bone's index by name (att -1), or the attachment number if the model has it
stock lsg_mp_read(const m[], const name[], att)
{
	new f = fopen(m, "rb", true)
	if (!f)
		return -1
	new id, ver, n, off, k = -1
	fread(f, id, BLOCK_INT)
	fread(f, ver, BLOCK_INT)
	if (id == 0x54534449 && ver == 10)
	{
		fseek(f, att >= 0 ? 212 : 140, SEEK_SET)
		fread(f, n, BLOCK_INT)
		fread(f, off, BLOCK_INT)
		if (att >= 0)
			k = att < n ? att : -1
		else
		{
			new nm[33]
			for (new i = 0; i < n && i < 128; i++)
			{
				fseek(f, off + i * 112, SEEK_SET)
				fread_blocks(f, nm, 32, BLOCK_CHAR)
				nm[32] = 0
				if (equali(nm, name))
				{
					k = i
					break
				}
			}
		}
	}
	fclose(f)
	return k
}

// where (which 0) or which way (1) a bone (kind 0) or attachment (1) is now; the entity's own
// place when it has none
stock lsg_mp_vec(ent, k, kind, which)
{
	new Float:o[3], Float:a[3]
	if (k >= 0 && pev_valid(ent))
	{
		if (kind)
			engfunc(EngFunc_GetAttachment, ent, k, o, a)
		else
			engfunc(EngFunc_GetBonePosition, ent, k, o, a)
	}
	else if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		pev(ent, pev_angles, a)
	}
	return lsg_vfrom(which ? a : o)
}
"##;

const RUNTIME: &str = r##"// ---- HUD & View (engine nodes): the game's own switches, kept through what resets them ----
stock g_lsgHud[33], g_lsgHudSet[33], bool:g_lsgHudKeep[33], g_lsgFov[33], bool:g_lsgFovKeep[33]
stock g_lsgVm[33][33], g_lsgPm[33][33], bool:g_lsgNoLight[33], bool:g_lsgQuiet[33]
stock g_lsgTag[33], bool:g_lsgTagKeep[33], g_lsgShowTeam[33], g_lsgSpec[33]
stock const g_lsgTeamWord[][] = { "UNASSIGNED", "TERRORIST", "CT", "SPECTATOR" }

// Hide HUD Parts: our bits in the game's m_iHideHUD (it sends HideWeapon and the crosshair itself)
stock lsg_eng_hud(id, mask, bool:keep)
{
	g_lsgHud[id] = mask
	g_lsgHudKeep[id] = keep
	lsg_eng_hud_put(id)
}

stock lsg_eng_hud_put(id)
{
	if (!is_user_connected(id))
		return
	new cur = get_ent_data(id, "CBasePlayer", "m_iHideHUD")
	cur = (cur & ~g_lsgHudSet[id]) | g_lsgHud[id]
	g_lsgHudSet[id] = g_lsgHud[id]
	set_ent_data(id, "CBasePlayer", "m_iHideHUD", cur)
}

// Set Field of View: the game's m_iFOV (it sends SetFOV); 0 is the game's own
stock lsg_eng_fov(id, fov, bool:keep)
{
	g_lsgFov[id] = fov == 90 ? 0 : fov
	g_lsgFovKeep[id] = keep
	lsg_eng_fov_put(id, true)
}

stock lsg_eng_fov_put(id, bool:now)
{
	if (!is_user_connected(id))
		return
	new want = g_lsgFov[id] ? g_lsgFov[id] : 90
	new cur = get_ent_data(id, "CBasePlayer", "m_iFOV")
	// (a scope zoomed in: left alone, and this comes back once it's off)
	if (!now && cur != 90 && cur != want)
		return
	if (cur != want)
	{
		set_ent_data(id, "CBasePlayer", "m_iFOV", want)
		set_pev(id, pev_fov, float(want))
	}
}

// Set Weapon View Model: per player and weapon, the model to show (index into g_lsgVmList)
stock lsg_eng_vm(id, const wname[], vm, pm)
{
	if (!lsg_ok(id))
		return 0
	new csw
	if (wname[0] == '-')
		csw = is_user_alive(id) ? get_user_weapon(id) : 0
	else
		csw = get_weaponid(wname)
	if (csw <= 0 || csw > 32)
		return 0
	if ((vm && !lsg_eng_vm_ok(vm)) || (pm && !lsg_eng_vm_ok(pm)))
		return 0
	g_lsgVm[id][csw] = vm
	g_lsgPm[id][csw] = pm
	// the one in hand: now (back to its own: deployed again)
	if (is_user_alive(id) && get_user_weapon(id) == csw)
	{
		new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
		if (!vm && !pm && w > 0)
			ExecuteHamB(Ham_Item_Deploy, w)
		else
			lsg_eng_vm_put(id, csw)
	}
	return 1
}

stock bool:lsg_eng_vm_ok(k)
{
	return k >= 1 && k <= LSG_VMN && g_lsgHave != Invalid_Trie && TrieKeyExists(g_lsgHave, g_lsgVmList[k])
}

stock lsg_eng_vm_put(id, csw)
{
	if (g_lsgVm[id][csw])
		set_pev(id, pev_viewmodel2, g_lsgVmList[g_lsgVm[id][csw]])
	if (g_lsgPm[id][csw])
		set_pev(id, pev_weaponmodel2, g_lsgVmList[g_lsgPm[id][csw]])
}

public lsg_eng_deploy(w)
{
	if (!pev_valid(w))
		return HAM_IGNORED
	new id = get_ent_data_entity(w, "CBasePlayerItem", "m_pPlayer")
	if (!lsg_ok(id))
		return HAM_IGNORED
	new csw = cs_get_weapon_id(w)
	if (csw > 0 && csw <= 32)
		lsg_eng_vm_put(id, csw)
	return HAM_IGNORED
}

// Flashlight: 0 off, 1 on, 2 allow, 3 deny; battery -1 leaves it
stock lsg_eng_light(id, what, bat)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return 0
	if (what == 2)
		g_lsgNoLight[id] = false
	else if (what == 3)
		g_lsgNoLight[id] = true
	if (bat >= 0)
		set_ent_data(id, "CBasePlayer", "m_iFlashBattery", clamp(bat, 0, 100))
	new bool:lit = (pev(id, pev_effects) & EF_DIMLIGHT) != 0
	if (what == 1)
		lit = true
	else if (what == 0 || what == 3)
		lit = false
	set_pev(id, pev_effects, lit ? (pev(id, pev_effects) | EF_DIMLIGHT) : (pev(id, pev_effects) & ~EF_DIMLIGHT))
	set_ent_data_float(id, "CBasePlayer", "m_flFlashLightTime", get_gametime() + (lit ? 1.2 : 0.2))
	message_begin(MSG_ONE, get_user_msgid("Flashlight"), _, id)
	write_byte(lit ? 1 : 0)
	write_byte(get_ent_data(id, "CBasePlayer", "m_iFlashBattery"))
	message_end()
	return 1
}

public lsg_eng_impulse(id)
	return lsg_ok(id) && g_lsgNoLight[id] ? PLUGIN_HANDLED_MAIN : PLUGIN_CONTINUE

// Night Vision: 0 take, 1 give, 2 on, 3 off
stock lsg_eng_nvg(id, what)
{
	if (!lsg_ok(id))
		return 0
	if (what == 1 || what == 2)
		cs_set_user_nvg(id, 1)
	new bool:on = what == 2
	if (what == 0 || what == 3 || what == 2)
	{
		set_ent_data(id, "CBasePlayer", "m_bNightVisionOn", on)
		if (!is_user_bot(id))
		{
			message_begin(MSG_ONE, get_user_msgid("NVGToggle"), _, id)
			write_byte(on ? 1 : 0)
			message_end()
		}
	}
	if (what == 0)
		cs_set_user_nvg(id, 0)
	return 1
}

// Silent Footsteps: the game waits for the next step sound
public lsg_eng_think(id)
{
	if (id >= 1 && id <= 32 && g_lsgQuiet[id])
		set_pev(id, pev_flTimeStepSound, 999)
	return FMRES_IGNORED
}

// Scoreboard Tag: 1 dead, 2 bomb, 4 VIP (0: the game's own), put back whenever the game sends one
stock lsg_eng_tag(id, tag, bool:keep)
{
	g_lsgTag[id] = tag
	g_lsgTagKeep[id] = keep
	message_begin(MSG_ALL, get_user_msgid("ScoreAttrib"))
	write_byte(id)
	write_byte(lsg_eng_real_attr(id) | tag)
	message_end()
}

stock lsg_eng_real_attr(id)
{
	new f = is_user_alive(id) ? 0 : 1
	if (user_has_weapon(id, CSW_C4))
		f |= 2
	if (cs_get_user_vip(id))
		f |= 4
	return f
}

public lsg_eng_attr(msg, dest, to)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= 32 && g_lsgTag[id])
		set_msg_arg_int(2, ARG_BYTE, get_msg_arg_int(2) | g_lsgTag[id])
	return PLUGIN_CONTINUE
}

// Scoreboard Team Color: what everyone's game is told the team is (0: the real one)
stock lsg_eng_color(id, t)
{
	g_lsgShowTeam[id] = t
	new real = _:cs_get_user_team(id)
	message_begin(MSG_ALL, get_user_msgid("TeamInfo"))
	write_byte(id)
	write_string(g_lsgTeamWord[t ? t : clamp(real, 0, 3)])
	message_end()
}

public lsg_eng_teaminfo(msg, dest, to)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= 32 && g_lsgShowTeam[id])
		set_msg_arg_string(2, g_lsgTeamWord[g_lsgShowTeam[id]])
	return PLUGIN_CONTINUE
}

// Force Spectate: through Target's eyes, kept there
stock lsg_eng_spec(id, target)
{
	if (!lsg_ok(id) || is_user_alive(id) || !lsg_ok(target) || target == id)
		return 0
	g_lsgSpec[id] = target
	lsg_eng_spec_put(id)
	return 1
}

stock lsg_eng_spec_put(id)
{
	new t = g_lsgSpec[id]
	if (!lsg_ok(t) || is_user_alive(id))
	{
		g_lsgSpec[id] = 0
		return
	}
	if (pev(id, pev_iuser2) != t || pev(id, pev_iuser1) != 4)
	{
		set_ent_data_entity(id, "CBasePlayer", "m_hObserverTarget", t)
		set_pev(id, pev_iuser1, 4)
		set_pev(id, pev_iuser2, t)
	}
}

// Play Soundtrack: as a map's trigger_cdaudio does it (the player's own music files)
stock lsg_eng_cd(id, track, bool:loop)
{
	if (!is_user_connected(id) || is_user_bot(id))
		return
	if (!track)
		client_cmd(id, "cd stop")
	else
		client_cmd(id, "cd %s %d", loop ? "loop" : "play", track)
}

// a quarter second: the field of view after a scope; the spectated stay on their target
public lsg_eng_tick()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p))
			continue
		if (g_lsgFov[p] && is_user_alive(p))
			lsg_eng_fov_put(p, false)
		if (g_lsgSpec[p])
			lsg_eng_spec_put(p)
	}
}

// the HUD redrawn (a respawn, a reconnect's first frame): ours again
public lsg_eng_reset(id)
{
	if (g_lsgHud[id])
		lsg_eng_hud_put(id)
	if (g_lsgFov[id])
		lsg_eng_fov_put(id, true)
}

// death/respawn: what's set to end there ends, the rest is put back
public lsg_eng_dead(id)
	lsg_eng_ends(id)

public lsg_eng_spawn(id)
{
	if (!is_user_alive(id))
		return
	lsg_eng_ends(id)
	if (g_lsgHud[id])
		lsg_eng_hud_put(id)
	if (g_lsgFov[id])
		lsg_eng_fov_put(id, true)
}

stock lsg_eng_ends(id)
{
	if (id < 1 || id > 32)
		return
	if (g_lsgHud[id] && !g_lsgHudKeep[id])
	{
		g_lsgHud[id] = 0
		lsg_eng_hud_put(id)
	}
	if (g_lsgFov[id] && !g_lsgFovKeep[id])
	{
		g_lsgFov[id] = 0
		lsg_eng_fov_put(id, true)
	}
	if (g_lsgTag[id] && !g_lsgTagKeep[id])
		g_lsgTag[id] = 0
}

// leaving: nothing carries over to whoever gets the slot next
public lsg_eng_left(id)
{
	if (id < 1 || id > 32)
		return FMRES_IGNORED
	g_lsgHud[id] = 0
	g_lsgHudSet[id] = 0
	g_lsgFov[id] = 0
	g_lsgNoLight[id] = false
	g_lsgQuiet[id] = false
	g_lsgTag[id] = 0
	g_lsgShowTeam[id] = 0
	g_lsgSpec[id] = 0
	for (new k = 0; k < 33; k++)
	{
		g_lsgVm[id][k] = 0
		g_lsgPm[id][k] = 0
	}
	for (new p = 1; p <= 32; p++)
		if (g_lsgSpec[p] == id)
			g_lsgSpec[p] = 0
	return FMRES_IGNORED
}
"##;

#[cfg(test)]
mod tests {
    use super::super::generate;
    use crate::tools::studio::graph::{self, parse};

    const G: &str = include_str!("../testdata/every_node_4.graph.json");

    /// The engine nodes: each one's code, the hooks only they need, the view models loaded with
    /// the map (and only those), and the Problems notes.
    #[test]
    fn engine_nodes_generate() {
        let g = parse(G).unwrap();
        let errs: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level == "error").collect();
        assert!(errs.is_empty(), "{errs:#?}");
        let src = generate(&g, false).source;
        // Hide HUD Parts: crosshair 64 + money 32, back on death; everyone: timer 16, radar 8, weapons 1, flashlight 2, kept
        assert!(src.contains("lsg_eng_hud(p, 96, false)") && src.contains("lsg_eng_hud(p, 27, true)"), "masks and keep");
        assert!(src.contains("for (new p = 1; p <= MaxClients; p++)\n\t\tif (is_user_connected(p))\n\t\t\tlsg_eng_hud(p, 27, true)"), "everyone when Player isn't wired");
        assert!(src.contains("set_ent_data(id, \"CBasePlayer\", \"m_iHideHUD\", cur)"), "the game's own switch, by name");
        assert!(src.contains("lsg_eng_fov(p, f, true)") && src.contains("new f = 100"));
        // view models: the list, the hook on every weapon, precache only those named
        assert!(src.contains("g_lsgVmList[LSG_VMN + 1][] = { \"\", \"models/p_usp.mdl\", \"models/v_knife.mdl\", \"models/v_usp.mdl\" }"), "{src}");
        assert!(src.contains("lsg_eng_vm(") && src.contains("\"-\", 2, 0)") && src.contains("\"weapon_usp\", 3, 1)") && src.contains("\"weapon_usp\", 0, 0)"));
        assert!(src.contains("RegisterHam(Ham_Item_Deploy, \"weapon_ak47\", \"lsg_eng_deploy\", 1)") && src.contains("RegisterHam(Ham_Item_Deploy, \"weapon_c4\", \"lsg_eng_deploy\", 1)"));
        assert!(src.contains("lsg_pre_model(\"models/v_knife.mdl\")") && src.contains("lsg_pre_model(\"models/p_usp.mdl\")"));
        assert!(src.contains("lsg_eng_light(") && src.contains(", 1, 50)") && src.contains(", 3, -1)") && src.contains("register_impulse(100, \"lsg_eng_impulse\")"));
        assert!(src.contains(", 1)\n") || src.contains("lsg_eng_nvg("));
        assert!(src.contains("g_lsgQuiet[p] = true") && src.contains("register_forward(FM_PlayerPreThink, \"lsg_eng_think\", 0)"));
        assert!(src.contains("lsg_eng_tag(p, 4, false)") && src.contains("register_message(get_user_msgid(\"ScoreAttrib\"), \"lsg_eng_attr\")"));
        assert!(src.contains("lsg_eng_color(p, 2)") && src.contains("register_message(get_user_msgid(\"TeamInfo\"), \"lsg_eng_teaminfo\")"));
        assert!(src.contains("lsg_eng_cd(p, 11, true)") && src.contains("lsg_eng_cd(p, 0, false)") && src.contains("client_cmd(id, \"cd %s %d\", loop ? \"loop\" : \"play\", track)"));
        assert!(src.contains("lsg_eng_spec(") && src.contains("g_lsgSpec[p] = 0"));
        // a mode without them has none of it
        let plain = generate(&parse(include_str!("../testdata/markers.graph.json")).unwrap(), false).source;
        assert!(!plain.contains("lsg_eng_"));
    }

    /// Get Bone / Attachment Position: the bone by name, the attachment by number, both looked up
    /// in the entity's own model file at run time; Found from the lookup; vectors through the ring.
    #[test]
    fn model_points_generate() {
        let g = parse(G).unwrap();
        let src = generate(&g, false).source;
        assert!(src.contains("lsg_mp_vec(ctx[0], lsg_mp_bone(ctx[0], \"Bip01 R Hand\"), 0, 0)"), "{src}");
        assert!(src.contains("(lsg_mp_bone(ctx[0], \"Bip01 R Hand\") >= 0 ? 1 : 0)"));
        assert!(src.contains("lsg_mp_vec(ctx[0], lsg_mp_bone(ctx[0], \"Bip01 R Hand\"), 0, 1)"));
        assert!(src.contains("lsg_mp_vec(ctx[0], lsg_mp_att(ctx[0], 1), 1, 0)"));
        assert!(src.contains("engfunc(EngFunc_GetBonePosition, ent, k, o, a)") && src.contains("if (id == 0x54534449 && ver == 10)"));
        // a bone name can't carry code
        let mut v: serde_json::Value = serde_json::from_str(G).unwrap();
        for n in v["nodes"].as_array_mut().unwrap() {
            if n["id"] == "bp" {
                n["params"]["bone"] = serde_json::json!("x\"); server_cmd(\"quit");
            }
        }
        let g2 = graph::from_value(v).unwrap();
        assert!(graph::validate(&g2).iter().any(|p| p.level == "error" && p.message.contains("bone's name")));
        let src2 = generate(&g2, false).source;
        assert!(!src2.contains("server_cmd(\"quit"));
    }

    /// Only what a graph uses is hooked: a soundtrack alone needs no spawn, deploy or message hooks.
    #[test]
    fn only_what_is_used_is_hooked() {
        let g = graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "cd_only", "name": "CD"},
            "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "cd", "type": "eng.soundtrack", "params": {"do": "play", "track": "5", "loop": false}}],
            "edges": [{"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "cd", "pin": "exec"}}]})).unwrap();
        let src = generate(&g, false).source;
        assert!(src.contains("lsg_eng_cd(p, 5, false)"));
        for h in ["lsg_eng_spawn\"", "lsg_eng_deploy\"", "ScoreAttrib\"), \"lsg", "register_impulse", "lsg_eng_tick\""] {
            assert!(!src.contains(h), "{h} hooked for nothing");
        }
        assert!(!src.contains("plugin_precache"), "nothing to load");
    }

    /// Problems: a view model that isn't a model file is an error; an odd one, a wide field of
    /// view, an empty Hide HUD and the team color's side effects are notes.
    #[test]
    fn engine_problems() {
        let mk = |t: &str, p: serde_json::Value| graph::from_value(serde_json::json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "pr", "name": "P"},
            "nodes": [{"id": "ps", "type": "event.player_spawned"}, {"id": "n", "type": t, "params": p}],
            "edges": [{"id": "w1", "from": {"node": "ps", "pin": "then"}, "to": {"node": "n", "pin": "exec"}}, {"id": "w2", "from": {"node": "ps", "pin": "player"}, "to": {"node": "n", "pin": "player"}}]})).unwrap();
        let has = |g: &graph::Graph, lvl: &str, s: &str| graph::validate(g).iter().any(|p| p.level == lvl && p.node.as_deref() == Some("n") && p.message.contains(s));
        assert!(has(&mk("eng.view_model", serde_json::json!({"model": "sound/x.wav"})), "error", "models/...mdl"));
        assert!(has(&mk("eng.view_model", serde_json::json!({"model": "models/w_ak47.mdl"})), "warning", "usually models/v_"));
        assert!(has(&mk("eng.view_model", serde_json::json!({"model": "models/v_ak47.mdl", "pmodel": "x.txt"})), "error", "Model others see"));
        assert!(!has(&mk("eng.view_model", serde_json::json!({"model": "", "restore": true})), "error", "models/"));
        assert!(has(&mk("eng.fov", serde_json::json!({"fov": 150})), "warning", "feel sick"));
        assert!(has(&mk("eng.hide_hud", serde_json::json!({"crosshair": false})), "warning", "Nothing is ticked"));
        assert!(has(&mk("eng.score_color", serde_json::json!({"team": "t"})), "warning", "buy menu"));
    }
}
