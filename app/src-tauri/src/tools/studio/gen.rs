//! Graph → Pawn. A checked graph becomes one AMX Mod X plugin (`studio_<mode id>.sma`) built on
//! `loner.inc`: one function per node (`lsg_<node id>`), called along the flow wires, with the
//! event's values (victim, attacker...) passed down in a small `ctx` array. Event handlers are
//! registered once per event kind and run every event node of that kind, in graph order.
//!
//! With `debug`, the plugin also carries a small runtime (below, `DEBUG_RUNTIME`) and every node
//! reports itself: `node_enter` (with its inputs), `node_exit` (with its result), `node_error`.
//! A node marked as a breakpoint freezes the world instead of running (`breakpoint_hit`,
//! `world_paused`) until the app says `resume`; then it runs and the flow carries on. Without
//! `debug` none of that is in the plugin (no cost on real servers).
//!
//! The channel is two plain files in the game's `addons/amxmodx/data/studio/`:
//! `<mode>.events` (the plugin appends one JSON object per line; emptied when the map loads) and
//! `<mode>.cmd` (the app writes commands, the plugin reads and deletes it, every 0.25 s).
//! Files work the same on Windows and under CrossOver (where the game's folder is an ordinary
//! folder on the Mac), need no ports, rcon password or network, and survive the app restarting.

use super::graph::{param, Graph, Node, NodeDef, View};
use serde::Serialize;
use serde_json::Value;

/// How many event values a flow can carry (victim, attacker...).
const CTX: usize = 4;

/// The plugin's file name (no extension) for a mode.
pub fn plugin_name(mode_id: &str) -> String {
    format!("studio_{mode_id}")
}

/// Which lines of the generated source belong to which node (1-based, inclusive), so a
/// compiler error can be shown on the node it came from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Span {
    pub node: String,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Generated {
    pub source: String,
    pub spans: Vec<Span>,
}

impl Generated {
    /// The node a source line belongs to.
    pub fn node_at(&self, line: usize) -> Option<&str> {
        self.spans.iter().find(|s| (s.from..=s.to).contains(&line)).map(|s| s.node.as_str())
    }
}

struct Out {
    text: String,
    lines: usize,
    spans: Vec<Span>,
}

impl Out {
    fn line(&mut self, s: impl AsRef<str>) {
        let s = s.as_ref();
        for l in s.split('\n') {
            self.text.push_str(l);
            self.text.push('\n');
            self.lines += 1;
        }
    }
    fn blank(&mut self) {
        self.line("");
    }
}

/// A Pawn string literal: AMX Mod X escapes with `^`; quotes and control characters never get
/// this far (the checks refuse them), but they're dropped here too.
pub fn pawn_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '^' => o.push_str("^^"),
            '"' => {}
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Event kinds: the Pawn registration, the handler and how it fills `ctx` from the event.
struct EventKind {
    kind: &'static str,
    handler: &'static str,
    register: &'static str,
    /// output pin -> (ctx slot, how it's read)
    values: &'static [(&'static str, usize, &'static str)],
}

const EVENTS: &[EventKind] = &[
    EventKind { kind: "event.round_started", handler: "lsg_on_round_started", register: "register_logevent(\"lsg_on_round_started\", 2, \"1=Round_Start\")", values: &[] },
    EventKind {
        kind: "event.player_killed",
        handler: "lsg_on_player_killed",
        register: "register_event(\"DeathMsg\", \"lsg_on_player_killed\", \"a\")",
        values: &[("victim", 0, "read_data(2)"), ("attacker", 1, "read_data(1)")],
    },
];

fn event_kind(kind: &str) -> Option<&'static EventKind> {
    EVENTS.iter().find(|e| e.kind == kind)
}

/// The function a node becomes.
fn fname(id: &str) -> String {
    format!("lsg_{id}")
}

/// The Pawn expression for a data input: the wired value, or the param it falls back to.
fn data_expr(v: &View, node: &str, def: &NodeDef, pin: &str) -> String {
    let n = v.nodes[node];
    if let Some((src, spin)) = v.data_source(node, pin) {
        if let Some(ev) = v.defs.get(src).and_then(|d| event_kind(&d.kind)) {
            if let Some((_, slot, _)) = ev.values.iter().find(|(p, _, _)| *p == spin) {
                return format!("ctx[{slot}]");
            }
        }
        return "0".into();
    }
    let ip = def.input(pin);
    match (ip.map(|p| p.ty.as_str()), ip.and_then(|p| p.param.as_deref())) {
        (Some("team"), Some(prm)) => team_const(param(n, def, prm).and_then(Value::as_str).unwrap_or("t")).into(),
        _ => "0".into(),
    }
}

fn team_const(t: &str) -> &'static str {
    match t {
        "ct" => "CS_TEAM_CT",
        _ => "CS_TEAM_T",
    }
}

fn num(n: &Node, def: &NodeDef, id: &str) -> f64 {
    param(n, def, id).and_then(Value::as_f64).unwrap_or(0.0)
}

fn text<'a>(n: &'a Node, def: &'a NodeDef, id: &str) -> &'a str {
    param(n, def, id).and_then(Value::as_str).unwrap_or("")
}

/// Turns a checked graph into the plugin's source. The graph must have passed
/// `graph::validate` without errors; nodes nothing runs are left out.
pub fn generate(g: &Graph, debug: bool) -> Generated {
    let v = View::new(g);
    let reached = v.reached_from();
    // the nodes that become functions: every event, and whatever their flow reaches
    let live: Vec<&str> = g
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| v.defs.get(id).is_some_and(|d| !d.reroute && (d.event || reached.contains_key(id))))
        .collect();
    let index = |id: &str| live.iter().position(|x| *x == id).unwrap_or(0);

    let mut o = Out { text: String::new(), lines: 0, spans: Vec::new() };
    o.line(format!("/*\n\t{} - made with Loner-Strike's Game Mode Studio (graph version {}).\n\tGenerated from {}.graph.json: change the graph and compile it again; edits here are lost.{}\n*/",
        g.mode.name, g.version, g.mode.id, if debug { "\n\tThis is a debug build (Playtest): it reports every node to the Studio." } else { "" }));
    o.blank();
    o.line("#include <amxmodx>\n#include <cstrike>\n#include <fakemeta>\n#include <loner>");
    o.blank();
    o.line(format!("#define LSG_MODE {}\n#define LSG_CTX {CTX}", pawn_str(&g.mode.id)));
    o.blank();

    if debug {
        o.line(format!("#define LSD_N {}", live.len().max(1)));
        let ids: Vec<String> = if live.is_empty() { vec![pawn_str("")] } else { live.iter().map(|id| pawn_str(id)).collect() };
        o.line(format!("new const g_lsdIds[LSD_N][] = {{ {} }}", ids.join(", ")));
        let bps: Vec<&str> = if live.is_empty() { vec!["false"] } else { live.iter().map(|id| if v.nodes[id].breakpoint { "true" } else { "false" }).collect() };
        o.line(format!("new bool:g_lsdBp[LSD_N] = {{ {} }}", bps.join(", ")));
        o.line(DEBUG_RUNTIME);
        o.blank();
    }

    // registration
    let kinds: Vec<&EventKind> = EVENTS.iter().filter(|k| live.iter().any(|id| v.defs[id].kind == k.kind)).collect();
    o.line("public plugin_init()\n{");
    o.line(format!("\tls_register_mode(LSG_MODE, {}, \"1.0\", \"Game Mode Studio\")", pawn_str(&g.mode.name)));
    if debug {
        o.line("\tlsd_init()");
    }
    for k in &kinds {
        o.line(format!("\t{}", k.register));
    }
    o.line("}");
    o.blank();

    // handlers
    for k in &kinds {
        o.line(format!("public {}()\n{{", k.handler));
        o.line("\tnew ctx[LSG_CTX]");
        for (pin, slot, read) in k.values {
            o.line(format!("\tctx[{slot}] = {read} // {pin}"));
        }
        for id in live.iter().filter(|id| v.defs[*id].kind == k.kind) {
            o.line(format!("\t{}(ctx)", fname(id)));
        }
        o.line("}");
        o.blank();
    }

    // nodes
    for id in &live {
        let def = v.defs[id];
        let n = v.nodes[id];
        let from = o.lines + 1;
        o.line(format!("// {} ({id})", def.title));
        o.line(format!("{}(const ctx[])\n{{", fname(id)));
        let mut body = Out { text: String::new(), lines: 0, spans: Vec::new() };
        let i = index(id);
        if debug {
            body.line("\tlsd_begin()");
            for ip in def.inputs.iter().filter(|p| p.ty != "exec") {
                let e = data_expr(&v, id, def, &ip.id);
                match ip.ty.as_str() {
                    "player" => body.line(format!("\tlsd_in_player({}, {e})", pawn_str(&ip.label))),
                    "team" => body.line(format!("\tlsd_in_team({}, {e})", pawn_str(&ip.label))),
                    _ => {}
                }
            }
            body.line(format!("\tif (lsd_enter({i}, ctx))\n\t\treturn"));
        }
        // calls the node wired to an output (nothing if it isn't wired)
        let next = |pin: &str| -> Option<String> { v.exec_target(id, pin).map(|(t, _)| format!("{}(ctx)", fname(t))) };
        let exit = |result: &str| format!("\tlsd_exit({i}, {})", pawn_str(result));
        match def.kind.as_str() {
            k if event_kind(k).is_some() => {
                if debug {
                    body.line(exit("ran"));
                }
                if let Some(c) = next("then") {
                    body.line(format!("\t{c}"));
                }
            }
            "cond.is_team" => {
                let p = data_expr(&v, id, def, "player");
                let t = data_expr(&v, id, def, "team");
                body.line(format!("\tnew bool:ok = 1 <= {p} <= MaxClients && is_user_connected({p}) && cs_get_user_team({p}) == {t}"));
                let (yes, no) = (next("true"), next("false"));
                if debug {
                    body.line(format!("\tlsd_exit({i}, g_lsdBool[_:ok])"));
                }
                match (yes, no) {
                    (Some(y), Some(n)) => body.line(format!("\tif (ok)\n\t\t{y}\n\telse\n\t\t{n}")),
                    (Some(y), None) => body.line(format!("\tif (ok)\n\t\t{y}")),
                    (None, Some(n)) => body.line(format!("\tif (!ok)\n\t\t{n}")),
                    (None, None) => {
                        if !debug {
                            body.line("\t#pragma unused ok");
                        }
                    }
                }
            }
            "player.give_weapon" => {
                let p = data_expr(&v, id, def, "player");
                let w = text(n, def, "weapon");
                let ammo = num(n, def, "ammo").round() as i64;
                if debug {
                    body.line(format!("\tif (ls_give_weapon({p}, {}, {ammo}))\n{}\n\telse\n\t\tlsd_error({i}, \"Not given: that player isn't alive.\")", pawn_str(w), exit("given").replacen('\t', "\t\t", 1)));
                } else {
                    body.line(format!("\tls_give_weapon({p}, {}, {ammo})", pawn_str(w)));
                }
                if let Some(c) = next("then") {
                    body.line(format!("\t{c}"));
                }
            }
            "score.end_round" => {
                let cmd = match text(n, def, "winner") {
                    "t" => "endround T",
                    "ct" => "endround CT",
                    _ => "endround",
                };
                body.line(format!("\tserver_cmd({})", pawn_str(cmd)));
                if debug {
                    body.line(exit(match text(n, def, "winner") {
                        "t" => "Terrorists win",
                        "ct" => "Counter-Terrorists win",
                        _ => "draw",
                    }));
                }
            }
            "msg.broadcast" => {
                let secs = num(n, def, "seconds");
                body.line(format!("\tls_announce(0, {secs:.1}, \"%s\", {})", pawn_str(text(n, def, "text"))));
                if debug {
                    body.line(exit("shown"));
                }
                if let Some(c) = next("then") {
                    body.line(format!("\t{c}"));
                }
            }
            _ => {}
        }
        if !body.text.contains("ctx") {
            body.line("\t#pragma unused ctx");
        }
        o.text.push_str(&body.text);
        o.lines += body.lines;
        o.line("}");
        o.spans.push(Span { node: id.to_string(), from, to: o.lines });
        o.blank();
    }

    if debug {
        // a paused node runs again from here when the app says resume
        if live.is_empty() {
            o.line("lsd_call(n, const ctx[])\n{\n\t#pragma unused n\n\t#pragma unused ctx\n}");
        } else {
            o.line("lsd_call(n, const ctx[])\n{\n\tswitch (n)\n\t{");
            for (i, id) in live.iter().enumerate() {
                o.line(format!("\t\tcase {i}: {}(ctx)", fname(id)));
            }
            o.line("\t}\n}");
        }
    }
    Generated { source: o.text, spans: o.spans }
}

/// The debug runtime (only in Playtest builds). Kept as plain Pawn so it reads like any plugin.
/// Every helper is a `stock`, so the ones a graph doesn't use cost nothing and warn nothing.
const DEBUG_RUNTIME: &str = r#"new const g_lsdBool[][] = { "false", "true" }
new const g_lsdTeams[][] = { "none", "T", "CT", "spectator" }
#define LSD_TASK 9475100
new g_lsdEvents[160], g_lsdCmd[160]
new g_lsdSeq, g_lsdTick, g_lsdRound
new g_lsdPhase[8] = "freeze"
new g_lsdIn[600]
new g_lsdPaused = -1, g_lsdSkip = -1
new g_lsdCtx[LSG_CTX]
new Float:g_lsdPausedAt
new g_lsdBotStop
new bool:g_lsdHeld[33], g_lsdWasFrozen[33], g_lsdCanShoot[33], Float:g_lsdWasDmg[33]

lsd_init()
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	formatex(g_lsdEvents, charsmax(g_lsdEvents), "%s/%s.events", dir, LSG_MODE)
	formatex(g_lsdCmd, charsmax(g_lsdCmd), "%s/%s.cmd", dir, LSG_MODE)
	new f = fopen(g_lsdEvents, "wt")
	if (f)
		fclose(f)
	register_event("HLTV", "lsd_new_round", "a", "1=0", "2=0")
	register_logevent("lsd_round_start", 2, "1=Round_Start")
	register_logevent("lsd_round_end", 2, "1=Round_End")
	set_task(0.25, "lsd_poll", LSD_TASK, _, _, "b")
	new map[32]
	get_mapname(map, charsmax(map))
	lsd_emitf("^"e^":^"session^",^"mode^":^"%s^",^"map^":^"%s^",^"nodes^":%d", LSG_MODE, map, LSD_N)
}

public plugin_end()
{
	// a map change while paused: the bots mustn't stay stopped
	if (g_lsdPaused >= 0)
		set_cvar_num("bot_stop", g_lsdBotStop)
}

public lsd_new_round()
{
	g_lsdRound++
	copy(g_lsdPhase, charsmax(g_lsdPhase), "freeze")
}

public lsd_round_start()
{
	copy(g_lsdPhase, charsmax(g_lsdPhase), "live")
}

public lsd_round_end()
{
	copy(g_lsdPhase, charsmax(g_lsdPhase), "ended")
}

stock lsd_emitf(const fmt[], any:...)
{
	static body[1400]
	vformat(body, charsmax(body), fmt, 2)
	new f = fopen(g_lsdEvents, "at")
	if (!f)
		return
	fprintf(f, "{^"s^":%d,^"t^":%.2f,%s}^n", ++g_lsdSeq, get_gametime(), body)
	fclose(f)
}

stock lsd_esc(const src[], dst[], len)
{
	new j = 0
	for (new i = 0; src[i] && j < len - 2; i++)
	{
		if (src[i] == '"' || src[i] == '\')
		{
			dst[j++] = '\'
			dst[j++] = src[i]
		}
		else if (src[i] >= 32)
			dst[j++] = src[i]
	}
	dst[j] = 0
}

stock lsd_pl(id, out[], len)
{
	if (id < 1 || id > MaxClients || !is_user_connected(id))
	{
		copy(out, len, "null")
		return
	}
	new name[32], esc[72], wpn[32]
	get_user_name(id, name, charsmax(name))
	lsd_esc(name, esc, charsmax(esc))
	new alive = is_user_alive(id) ? 1 : 0
	if (alive)
	{
		new w = get_user_weapon(id)
		if (w > 0)
		{
			get_weaponname(w, wpn, charsmax(wpn))
			replace(wpn, charsmax(wpn), "weapon_", "")
		}
	}
	new team = _:cs_get_user_team(id)
	if (team < 0 || team > 3)
		team = 0
	formatex(out, len, "{^"id^":%d,^"name^":^"%s^",^"team^":^"%s^",^"hp^":%d,^"alive^":%s,^"weapon^":^"%s^",^"bot^":%s}",
		id, esc, g_lsdTeams[team], alive ? get_user_health(id) : 0, g_lsdBool[alive], wpn, g_lsdBool[is_user_bot(id) ? 1 : 0])
}

stock lsd_begin()
{
	g_lsdIn[0] = 0
}

stock lsd_in_raw(const name[], const json[])
{
	if (g_lsdIn[0])
		add(g_lsdIn, charsmax(g_lsdIn), ",")
	new part[300]
	formatex(part, charsmax(part), "^"%s^":%s", name, json)
	add(g_lsdIn, charsmax(g_lsdIn), part)
}

stock lsd_in_player(const name[], id)
{
	new pl[240]
	lsd_pl(id, pl, charsmax(pl))
	lsd_in_raw(name, pl)
}

stock lsd_in_team(const name[], CsTeams:team)
{
	new t = _:team
	if (t < 0 || t > 3)
		t = 0
	new q[16]
	formatex(q, charsmax(q), "^"%s^"", g_lsdTeams[t])
	lsd_in_raw(name, q)
}

stock bool:lsd_enter(n, const ctx[])
{
	if (g_lsdSkip == n)
		g_lsdSkip = -1
	else if (g_lsdBp[n] && g_lsdPaused < 0)
	{
		g_lsdPaused = n
		for (new i = 0; i < LSG_CTX; i++)
			g_lsdCtx[i] = ctx[i]
		lsd_emitf("^"e^":^"breakpoint_hit^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
		lsd_freeze()
		return true
	}
	lsd_emitf("^"e^":^"node_enter^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
	return false
}

stock lsd_exit(n, const result[])
{
	lsd_emitf("^"e^":^"node_exit^",^"node^":^"%s^",^"result^":^"%s^"", g_lsdIds[n], result)
}

stock lsd_error(n, const msg[])
{
	lsd_emitf("^"e^":^"node_error^",^"node^":^"%s^",^"message^":^"%s^"", g_lsdIds[n], msg)
}

stock lsd_time_left()
{
	new Float:now = g_lsdPaused >= 0 ? g_lsdPausedAt : get_gametime()
	new Float:start = get_gamerules_float("CHalfLifeMultiplay", "m_fRoundCount")
	new secs = get_gamerules_int("CHalfLifeMultiplay", "m_iRoundTimeSecs")
	return max(floatround(float(secs) - (now - start)), 0)
}

stock lsd_hold(id)
{
	if (g_lsdHeld[id] || !is_user_alive(id))
		return
	g_lsdHeld[id] = true
	g_lsdWasFrozen[id] = pev(id, pev_flags) & FL_FROZEN
	g_lsdCanShoot[id] = get_ent_data(id, "CBasePlayer", "m_bCanShoot")
	pev(id, pev_takedamage, g_lsdWasDmg[id])
	set_pev(id, pev_flags, pev(id, pev_flags) | FL_FROZEN)
	set_pev(id, pev_takedamage, 0.0)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_ent_data(id, "CBasePlayer", "m_bCanShoot", false)
}

stock lsd_freeze()
{
	g_lsdPausedAt = get_gametime()
	g_lsdBotStop = get_cvar_num("bot_stop")
	set_cvar_num("bot_stop", 1)
	for (new id = 1; id <= MaxClients; id++)
		lsd_hold(id)
	lsd_emitf("^"e^":^"world_paused^",^"node^":^"%s^"", g_lsdIds[g_lsdPaused])
	lsd_vars()
}

stock lsd_release()
{
	new Float:paused = get_gametime() - g_lsdPausedAt
	set_cvar_num("bot_stop", g_lsdBotStop)
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!g_lsdHeld[id])
			continue
		g_lsdHeld[id] = false
		if (!is_user_connected(id))
			continue
		if (!g_lsdWasFrozen[id])
			set_pev(id, pev_flags, pev(id, pev_flags) & ~FL_FROZEN)
		set_pev(id, pev_takedamage, g_lsdWasDmg[id])
		if (is_user_alive(id))
			set_ent_data(id, "CBasePlayer", "m_bCanShoot", g_lsdCanShoot[id])
	}
	lsd_emitf("^"e^":^"world_resumed^",^"paused^":%.1f", paused)
	// the round clock didn't run while paused (last: it needs the game's rules object)
	set_gamerules_float("CHalfLifeMultiplay", "m_fRoundCount", get_gamerules_float("CHalfLifeMultiplay", "m_fRoundCount") + paused)
	message_begin(MSG_ALL, get_user_msgid("RoundTime"))
	write_short(lsd_time_left())
	message_end()
}

stock lsd_vars()
{
	new t, ct
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id))
			continue
		switch (cs_get_user_team(id))
		{
			case CS_TEAM_T: t++
			case CS_TEAM_CT: ct++
		}
	}
	lsd_emitf("^"e^":^"vars^",^"round^":%d,^"phase^":^"%s^",^"timeLeft^":%d,^"aliveT^":%d,^"aliveCT^":%d,^"paused^":%s",
		g_lsdRound, g_lsdPhase, lsd_time_left(), t, ct, g_lsdBool[g_lsdPaused >= 0 ? 1 : 0])
	new pl[240]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_connected(id))
			continue
		lsd_pl(id, pl, charsmax(pl))
		lsd_emitf("^"e^":^"var_player^",^"p^":%s", pl)
	}
}

stock lsd_set_bps(const list[])
{
	for (new i = 0; i < LSD_N; i++)
		g_lsdBp[i] = false
	new pos, id[32]
	while ((pos = argparse(list, pos, id, charsmax(id))) != -1)
	{
		for (new i = 0; i < LSD_N; i++)
			if (equal(g_lsdIds[i], id))
				g_lsdBp[i] = true
	}
}

stock lsd_resume()
{
	if (g_lsdPaused < 0)
		return
	new n = g_lsdPaused
	g_lsdPaused = -1
	lsd_release()
	g_lsdSkip = n
	new ctx[LSG_CTX]
	for (new i = 0; i < LSG_CTX; i++)
		ctx[i] = g_lsdCtx[i]
	lsd_call(n, ctx)
}

public lsd_poll()
{
	if (g_lsdPaused >= 0)
	{
		for (new id = 1; id <= MaxClients; id++)
			lsd_hold(id)
		if (++g_lsdTick % 4 == 0)
			lsd_vars()
	}
	if (!file_exists(g_lsdCmd))
		return
	new f = fopen(g_lsdCmd, "rt")
	if (!f)
		return
	new line[600], bool:resume
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (equal(line, "resume"))
			resume = true
		else if (equal(line, "vars"))
			lsd_vars()
		else if (equal(line, "bp", 2) && (line[2] == ' ' || line[2] == 0))
			lsd_set_bps(line[2])
	}
	fclose(f)
	delete_file(g_lsdCmd)
	lsd_emitf("^"e^":^"commands^",^"resume^":%s", g_lsdBool[resume ? 1 : 0])
	if (resume)
		lsd_resume()
}"#;

#[cfg(test)]
mod tests {
    use super::super::graph::{parse, validate};
    use super::*;

    const TEMPLATE: &str = include_str!("../../../../ui/src/views/tools/studio/templates/team-dm.graph.json");
    const ALL_NODES: &str = include_str!("testdata/all_nodes.graph.json");

    fn snapshot(name: &str, got: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/studio/testdata").join(name);
        if std::env::var("UPDATE_SNAPSHOTS").is_ok() || !path.exists() {
            std::fs::write(&path, got).unwrap();
        }
        let want = std::fs::read_to_string(&path).unwrap();
        assert!(want == got, "{name} changed; run with UPDATE_SNAPSHOTS=1 if that's intended.\n--- got ---\n{got}");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(pawn_str("a^b"), "\"a^^b\"");
        assert_eq!(pawn_str("q\"x\ny"), "\"qxy\"");
    }

    #[test]
    fn template_release_snapshot() {
        let g = parse(TEMPLATE).unwrap();
        assert!(validate(&g).is_empty());
        let out = generate(&g, false);
        assert!(!out.source.contains("lsd_"), "no debug code in a release build");
        snapshot("rifle_reward.sma", &out.source);
    }

    #[test]
    fn template_debug_snapshot() {
        let g = parse(TEMPLATE).unwrap();
        let out = generate(&g, true);
        assert!(out.source.contains("lsd_enter(") && out.source.contains("node_enter"));
        snapshot("rifle_reward.debug.sma", &out.source);
    }

    #[test]
    fn all_nodes_snapshot() {
        let g = parse(ALL_NODES).unwrap();
        let p = validate(&g);
        assert!(!super::super::graph::has_errors(&p), "{p:?}");
        snapshot("all_nodes.sma", &generate(&g, false).source);
        let dbg = generate(&g, true);
        snapshot("all_nodes.debug.sma", &dbg.source);
        // breakpoints marked in the graph start enabled
        assert!(dbg.source.contains("new bool:g_lsdBp[LSD_N] = { false, false, true"), "{}", dbg.source.lines().find(|l| l.contains("g_lsdBp[LSD_N]")).unwrap_or(""));
    }

    #[test]
    fn spans_point_at_nodes() {
        let g = parse(TEMPLATE).unwrap();
        let out = generate(&g, true);
        let lines: Vec<&str> = out.source.lines().collect();
        for s in &out.spans {
            assert_eq!(lines[s.from - 1], format!("// {} ({})", super::super::graph::registry().get(&g.nodes.iter().find(|n| n.id == s.node).unwrap().kind).unwrap().title, s.node));
            assert_eq!(lines[s.to - 1], "}");
        }
        let give = out.spans.iter().find(|s| s.node == "give_ak").unwrap();
        assert_eq!(out.node_at(give.from + 2), Some("give_ak"));
        assert_eq!(out.node_at(1), None);
    }

    #[test]
    fn unreached_nodes_left_out() {
        let mut g = parse(TEMPLATE).unwrap();
        g.edges.retain(|e| e.id != "w8");
        let out = generate(&g, false);
        assert!(!out.source.contains("lsg_give_ak("));
    }
}
