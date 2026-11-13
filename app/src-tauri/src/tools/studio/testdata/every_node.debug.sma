/*
	Every Node - made with Loner-Strike's Game Mode Studio (graph version 2).
	Generated from every_node.graph.json: change the graph and compile it again; edits here are lost.
	This is a debug build (Playtest): it reports every node to the Studio.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>

#define LSG_MODE "every_node"
#define LSG_CTX 4
#define LSG_TXT 2
#define LSG_N 77
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock Float:g_pv0[33] // Kills (number, per player)
stock Float:g_v1 = 1.0 // Round (number)
stock g_v2 = 0 // Flag (bool)
stock g_v3[64] = "none" // Last thing (text)

#define LSD_N 77
new const g_lsdIds[LSD_N][] = { "rs", "rs_set", "rs_cd", "rs_msg", "rs_rep", "rs_hud", "rs_delay", "rs_wave", "rs_wstop", "rs_reset", "pk", "pk_br", "pk_add", "pk_add2", "pk_team", "pk_give", "pk_seth", "pk_hud", "ps", "ps_strip", "ps_arm", "ps_spd", "ps_team", "ps_tp", "ps_set", "pj", "pj_hud", "pj_br", "pl", "pl_set", "bp", "bp_hud", "bd", "bd_end", "bx", "bx_end", "re", "re_br", "re_set", "re_stop", "pu", "pu_door", "pu_trap", "pt", "pt_crate", "cc", "cc_lock", "cc_hud", "cl", "cl_ex", "zs", "zs_shrink", "zt", "zt_set", "zt_hud", "ex", "ex_add", "ws", "ws_hud", "wp", "wp_hud", "wc", "wc_start", "lo", "lo_hud", "ck", "ck_set", "bb", "bb_msg", "me", "me_hud", "ck2", "ck2_set", "ck2_make", "ck2_spawn", "ck2_hud", "ck2_hud2" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
// The Game Mode Studio's debug runtime (tools/studio/gen.rs puts it into Playtest builds only).
// Every helper is a stock, so the ones a graph doesn't use cost nothing and warn nothing.
new const g_lsdBool[][] = { "false", "true" }
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
// the round clock: Pawn can't stop the game's own clock safely (ReGameDLL's rules object isn't
// laid out like AMX Mod X's gamedata says), so while paused no round may end (mp_round_infinite)
// and afterwards the round may not end on time until the paused seconds are made up
new Float:g_lsdLiveAt, Float:g_lsdFreezeAt, Float:g_lsdOwed
new g_lsdInfWas[16], bool:g_lsdHolding
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
	// the breakpoints the Studio has on now (kept across map changes; "bp" commands change them live)
	new bps[160], line[600]
	formatex(bps, charsmax(bps), "%s/%s.bp", dir, LSG_MODE)
	f = fopen(bps, "rt")
	if (f)
	{
		fgets(f, line, charsmax(line))
		fclose(f)
		trim(line)
		lsd_set_bps(line)
	}
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
	// a map change while paused: the bots mustn't stay stopped, rounds must end again
	if (g_lsdPaused >= 0)
		set_cvar_num("bot_stop", g_lsdBotStop)
	lsd_unhold()
}

public lsd_new_round()
{
	g_lsdRound++
	copy(g_lsdPhase, charsmax(g_lsdPhase), "freeze")
	g_lsdFreezeAt = get_gametime()
	g_lsdOwed = 0.0
	if (g_lsdPaused < 0)
		lsd_unhold()
}

public lsd_round_start()
{
	copy(g_lsdPhase, charsmax(g_lsdPhase), "live")
	g_lsdLiveAt = get_gametime()
	g_lsdOwed = 0.0
}

stock lsd_hold(const how[])
{
	if (!g_lsdHolding)
	{
		get_cvar_string("mp_round_infinite", g_lsdInfWas, charsmax(g_lsdInfWas))
		// someone set it on purpose: leave it alone
		if (g_lsdInfWas[0] && !equal(g_lsdInfWas, "0"))
			return
		g_lsdHolding = true
	}
	set_cvar_string("mp_round_infinite", how)
}

stock lsd_unhold()
{
	if (!g_lsdHolding)
		return
	g_lsdHolding = false
	if (g_lsdInfWas[0])
		set_cvar_string("mp_round_infinite", g_lsdInfWas)
	else
		set_cvar_string("mp_round_infinite", "0")
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

stock lsd_in_number(const name[], Float:v)
{
	new q[24]
	formatex(q, charsmax(q), "%.2f", v)
	lsd_in_raw(name, q)
}

stock lsd_in_bool(const name[], v)
{
	lsd_in_raw(name, g_lsdBool[v ? 1 : 0])
}

stock lsd_in_text(const name[], const v[])
{
	new esc[140], q[144]
	lsd_esc(v, esc, charsmax(esc))
	formatex(q, charsmax(q), "^"%s^"", esc)
	lsd_in_raw(name, q)
}

stock lsd_in_entity(const name[], ent)
{
	if (ent >= 1 && ent <= MaxClients)
	{
		lsd_in_player(name, ent)
		return
	}
	new q[200], cls[32], tn[32], ecls[72], etn[72]
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_classname, cls, charsmax(cls))
		pev(ent, pev_targetname, tn, charsmax(tn))
		lsd_esc(cls, ecls, charsmax(ecls))
		lsd_esc(tn, etn, charsmax(etn))
		formatex(q, charsmax(q), "{^"ent^":%d,^"class^":^"%s^",^"name^":^"%s^"}", ent, ecls, etn)
	}
	else
		copy(q, charsmax(q), "null")
	lsd_in_raw(name, q)
}

stock lsd_in_team(const name[], team)
{
	new t = team
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
	new Float:left
	if (equal(g_lsdPhase, "live"))
		left = get_cvar_float("mp_roundtime") * 60.0 - (now - g_lsdLiveAt - g_lsdOwed)
	else if (equal(g_lsdPhase, "freeze"))
		left = get_cvar_float("mp_freezetime") - (now - g_lsdFreezeAt)
	return max(floatround(left, floatround_ceil), 0)
}

stock lsd_hold_player(id)
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
		lsd_hold_player(id)
	lsd_hold("1")
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
	// the paused seconds are added to the round: it can't end on time until they're made up
	if (equal(g_lsdPhase, "live"))
	{
		g_lsdOwed += paused
		lsd_hold("f")
	}
	else
		lsd_unhold()
	lsd_emitf("^"e^":^"world_resumed^",^"paused^":%.1f", paused)
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
	lsd_graph_vars()
}

stock lsd_var_number(const v[], p, Float:value)
{
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":%.2f", v, p, value)
}

stock lsd_var_bool(const v[], p, value)
{
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":%s", v, p, g_lsdBool[value ? 1 : 0])
}

stock lsd_var_text(const v[], p, const value[])
{
	new esc[140]
	lsd_esc(value, esc, charsmax(esc))
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":^"%s^"", v, p, esc)
}

// a timer that fires while the world is paused waits until it isn't
stock bool:lsd_hold_timer()
{
	return g_lsdPaused >= 0
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
			lsd_hold_player(id)
		if (++g_lsdTick % 4 == 0)
			lsd_vars()
	}
	else if (g_lsdHolding && lsd_time_left() <= 0)
		lsd_unhold()
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
}

lsd_graph_vars()
{
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
			lsd_var_number("kills", id, g_pv0[id])
	lsd_var_number("round", 0, g_v1)
	lsd_var_bool("flag", 0, g_v2)
	lsd_var_text("last", 0, g_v3)
}

stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

stock Float:lsg_div(Float:a, Float:b)
	return b == 0.0 ? 0.0 : a / b

stock Float:lsg_min(Float:a, Float:b)
	return a < b ? a : b

stock Float:lsg_max(Float:a, Float:b)
	return a > b ? a : b

stock Float:lsg_random(Float:a, Float:b, frac)
{
	if (a > b)
	{
		new Float:t = a
		a = b
		b = t
	}
	return frac ? random_float(a, b) : float(random_num(floatround(a), floatround(b)))
}

stock Float:lsg_health(id)
	return ls_alive(id) ? float(get_user_health(id)) : 0.0

stock lsg_team(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) ? (_:cs_get_user_team(id)) : 0

stock lsg_is_bot(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) && is_user_bot(id) ? 1 : 0

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

stock lsg_find(const name[])
	return name[0] ? max(find_ent_by_tname(-1, name), 0) : 0

stock bool:lsg_named(ent, const name[])
{
	new tn[32]
	pev(ent, pev_targetname, tn, charsmax(tn))
	return bool:equal(tn, name)
}

stock bool:lsg_origin_of(const name[], Float:o[3])
{
	new ent = lsg_find(name)
	if (ent <= 0)
		return false
	pev(ent, pev_origin, o)
	return true
}

stock bool:lsg_teleport(id, const name[])
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_origin_of(name, o))
		return false
	ls_teleport(id, o)
	return true
}

stock lsg_log_player()
{
	new who[96], name[32], uid
	read_logargv(0, who, charsmax(who))
	parse_loguser(who, name, charsmax(name), uid)
	return find_player("k", uid)
}

stock lsg_creature(const kind[], const at[])
{
	new Float:o[3]
	if (!get_cvar_pointer("ls_creature_ver") || !lsg_origin_of(at, o))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("SendAudio", "lsg_on_round_end_t", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "lsg_on_round_end_ct", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "lsg_on_round_end_draw", "a", "2&%!MRAD_rounddraw")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	RegisterHam(Ham_Spawn, "player", "lsg_on_player_spawned", 1)
	register_logevent("lsg_on_bomb_planted", 3, "2=Planted_The_Bomb")
	register_logevent("lsg_on_bomb_defused", 3, "2=Defused_The_Bomb")
	register_logevent("lsg_on_bomb_exploded", 6, "3=Target_Bombed")
	RegisterHam(Ham_Use, "func_button", "lsg_on_player_used", 0)
	register_touch("trigger_multiple", "player", "lsg_on_player_touched")
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_on_creature_killed", 1)
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
}

// timers stop when a new round starts
public lsg_new_round()
	lsg_stop_timers()

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv0[id] = 0.0
	lsg_on_player_joined(id)
}

public client_disconnected(id)
	lsg_on_player_left(id)

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public lsg_on_round_end_t()
	lsg_on_round_ended(1)

public lsg_on_round_end_ct()
	lsg_on_round_ended(2)

public lsg_on_round_end_draw()
	lsg_on_round_ended(0)

lsg_on_round_ended(winner)
{
	new ctx[LSG_CTX]
	ctx[0] = winner // winner
	lsg_n36(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	lsg_n10(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n18(ctx)
	return HAM_IGNORED
}

lsg_on_player_joined(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n25(ctx)
}

lsg_on_player_left(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n28(ctx)
}

public lsg_on_bomb_planted()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n30(ctx)
}

public lsg_on_bomb_defused()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n32(ctx)
}

public lsg_on_bomb_exploded()
{
	new ctx[LSG_CTX]
	lsg_n34(ctx)
}

public lsg_on_player_used(ent, caller, activator)
{
	if (activator < 1 || activator > MaxClients)
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = activator // player
	ctx[1] = ent // entity
	if (lsg_named(ent, "door_button"))
		lsg_n40(ctx)
	return HAM_IGNORED
}

public lsg_on_player_touched(ent, id)
{
	static Float:last[33]
	if (id < 1 || id > MaxClients || get_gametime() - last[id] < 1.0)
		return PLUGIN_CONTINUE
	last[id] = get_gametime()
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = ent // entity
	lsg_n43(ctx)
	return PLUGIN_CONTINUE
}

public ls_on_capture(ent, side, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = side == 1 || side == 2 ? side : 0 // team
	ctx[2] = id // player
	lsg_n45(ctx)
}

public ls_on_capture_lost(ent, side)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = side == 1 || side == 2 ? side : 0 // team
	lsg_n48(ctx)
}

public ls_on_zone_stage(ent, stage)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // zone
	ctx[1] = _:float(stage) // stage
	lsg_n50(ctx)
}

public ls_on_zone_tick(ent, id, Float:dmg)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // zone
	ctx[1] = id // player
	ctx[2] = _:dmg // damage
	lsg_n52(ctx)
}

public ls_on_extract(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = id // player
	lsg_n55(ctx)
}

public ls_on_wave_start(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n57(ctx)
}

public ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[])
{
	#pragma unused origin
	#pragma unused types
	new ctx[LSG_CTX]
	ctx[0] = ent // spawned
	ctx[1] = _:float(wave) // wave
	lsg_n59(ctx)
}

public ls_on_wave_cleared(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n61(ctx)
}

public ls_on_loot(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // container
	ctx[1] = id // player
	lsg_n63(ctx)
}

public ls_on_checkpoint(ent, id, order)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = id // player
	ctx[2] = _:float(order) // order
	lsg_n65(ctx)
}

public ls_on_barricade_broken(ent, attacker)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // barricade
	ctx[1] = attacker // attacker
	lsg_n67(ctx)
}

public ls_on_meeting_end(ent, ejected)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // meeting
	ctx[1] = ejected // ejected
	lsg_n69(ctx)
}

public lsg_on_creature_killed(ent, killer)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // creature
	ctx[1] = killer >= 1 && killer <= MaxClients ? killer : 0 // killer
	pev(ent, pev_netname, g_lsgTxt[0], 63) // kind
	lsg_n71(ctx)
	return HAM_IGNORED
}

// Round Started (rs)
lsg_n0(const ctx[])
{
	lsd_begin()
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n1(ctx)
}

// Set Variable (rs_set)
lsg_n1(const ctx[])
{
	lsd_begin()
	lsd_in_number("Value", ((g_v1) + (1.0)))
	if (lsd_enter(1, ctx))
		return
	g_v1 = ((g_v1) + (1.0))
	lsd_exit(1, fmt("%.2f", g_v1))
	lsg_n2(ctx)
}

// Countdown (rs_cd)
lsg_n2(const ctx[])
{
	lsd_begin()
	if (lsd_enter(2, ctx))
		return
	remove_task(LSG_TASK + 2)
	g_lsgLeft[2] = 5
	lsg_c2_show()
	set_task(1.0, "lsg_t2", LSG_TASK + 2, ctx, LSG_CTX)
	lsd_exit(2, "started")
	lsg_n4(ctx)
}

lsg_c2_show()
	ls_hud(0, LsHud:0, 1.2, "%s %d", "Go in", g_lsgLeft[2])

public lsg_t2(const ctx[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t2", LSG_TASK + 2, ctx, LSG_CTX)
		return
	}
	#pragma unused task
	if (--g_lsgLeft[2] > 0)
	{
		lsg_c2_show()
		set_task(1.0, "lsg_t2", LSG_TASK + 2, ctx, LSG_CTX)
		return
	}
	lsd_exit(2, "done")
	lsg_n3(ctx)
}

// Show Message (rs_msg)
lsg_n3(const ctx[])
{
	lsd_begin()
	if (lsd_enter(3, ctx))
		return
	ls_announce(0, 2.0, "%s", "Go!")
	lsd_exit(3, "shown")
}

// Repeat (rs_rep)
lsg_n4(const ctx[])
{
	lsd_begin()
	lsd_in_number("Every (s)", 10.0)
	if (lsd_enter(4, ctx))
		return
	remove_task(LSG_TASK + 4)
	g_lsgLeft[4] = 3
	set_task(floatmax(10.0, 0.1), "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
	lsd_exit(4, "started")
	lsg_n6(ctx)
}

public lsg_t4(const ctx[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
		return
	}
	#pragma unused task
	if (g_lsgLeft[4] > 0 && --g_lsgLeft[4] == 0)
		remove_task(LSG_TASK + 4)
	else
		set_task(floatmax(10.0, 0.1), "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
	lsd_exit(4, "each")
	lsg_n5(ctx)
}

// Show HUD Text (rs_hud)
lsg_n5(const ctx[])
{
	lsd_begin()
	lsd_in_number("Value", float(ls_count_alive(CsTeams:(0))))
	if (lsd_enter(5, ctx))
		return
	ls_hud(0, LsHud:2, 5.0, "%s %d", "Alive:", floatround(float(ls_count_alive(CsTeams:(0)))))
	lsd_exit(5, "shown")
}

// Delay (rs_delay)
lsg_n6(const ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 5.0)
	if (lsd_enter(6, ctx))
		return
	set_task(floatmax(5.0, 0.1), "lsg_t6", LSG_TASK + 6, ctx, LSG_CTX)
	lsd_exit(6, "waiting")
}

public lsg_t6(const ctx[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t6", LSG_TASK + 6, ctx, LSG_CTX)
		return
	}
	#pragma unused task
	lsd_exit(6, "fired")
	lsg_n7(ctx)
}

// Start Wave (rs_wave)
lsg_n7(const ctx[])
{
	lsd_begin()
	lsd_in_number("Wave", 0.0)
	if (lsd_enter(7, ctx))
		return
	if (!ls_components_running())
		lsd_error(7, "gm_components isn't running on this server.")
	else
	{
		ls_wave_start(max(floatround(0.0), 0))
		lsd_exit(7, "done")
	}
	lsg_n8(ctx)
}

// Stop Waves (rs_wstop)
lsg_n8(const ctx[])
{
	lsd_begin()
	if (lsd_enter(8, ctx))
		return
	if (!ls_components_running())
		lsd_error(8, "gm_components isn't running on this server.")
	else
	{
		ls_wave_stop()
		lsd_exit(8, "done")
	}
	lsg_n9(ctx)
}

// Reset Components (rs_reset)
lsg_n9(const ctx[])
{
	lsd_begin()
	if (lsd_enter(9, ctx))
		return
	if (!ls_components_running())
		lsd_error(9, "gm_components isn't running on this server.")
	else
	{
		ls_comp_reset()
		lsd_exit(9, "done")
	}
}

// Player Killed (pk)
lsg_n10(const ctx[])
{
	lsd_begin()
	if (lsd_enter(10, ctx))
		return
	lsd_exit(10, "ran")
	lsg_n11(ctx)
}

// Branch (pk_br)
lsg_n11(const ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ctx[2])
	if (lsd_enter(11, ctx))
		return
	new ok = ((((ctx[2]) ? 1 : 0)) ? 1 : 0)
	lsd_exit(11, g_lsdBool[ok])
	if (ok)
		lsg_n12(ctx)
	else
		lsg_n13(ctx)
}

// Add To Variable (pk_add)
lsg_n12(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Amount", 2.0)
	if (lsd_enter(12, ctx))
		return
	g_pv0[lsg_slot(ctx[1])] += 2.0
	lsd_exit(12, fmt("%.2f", g_pv0[lsg_slot(ctx[1])]))
	lsg_n14(ctx)
}

// Add To Variable (pk_add2)
lsg_n13(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Amount", lsg_random(1.0, 3.0, 0))
	if (lsd_enter(13, ctx))
		return
	g_pv0[lsg_slot(ctx[1])] += lsg_random(1.0, 3.0, 0)
	lsd_exit(13, fmt("%.2f", g_pv0[lsg_slot(ctx[1])]))
}

// Is Team (pk_team)
lsg_n14(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_team("Team", 2)
	if (lsd_enter(14, ctx))
		return
	new ok = ((ctx[1] >= 1 && ctx[1] <= MaxClients && is_user_connected(ctx[1]) && _:cs_get_user_team(ctx[1]) == 2) ? 1 : 0)
	lsd_exit(14, g_lsdBool[ok])
	if (ok)
		lsg_n15(ctx)
	else
		lsg_n17(ctx)
}

// Give Weapon (pk_give)
lsg_n15(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(15, ctx))
		return
	if (ls_give_weapon(ctx[1], "m4a1", 60))
		lsd_exit(15, "given")
	else
		lsd_error(15, "Not given: that player is dead or already has one.")
	lsg_n16(ctx)
}

// Set Health (pk_seth)
lsg_n16(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Health", ((lsg_health(ctx[1])) + (10.0)))
	if (lsd_enter(16, ctx))
		return
	if (!ls_alive(ctx[1]))
		lsd_error(16, "Nothing done: that player isn't alive.")
	else
	{
		ls_set_health(ctx[1], max(floatround(((lsg_health(ctx[1])) + (10.0))), 1))
		lsd_exit(16, "set")
	}
}

// Show HUD Text (pk_hud)
lsg_n17(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_text("Value", lsg_name(ctx[0]))
	if (lsd_enter(17, ctx))
		return
	ls_hud(ctx[1], LsHud:3, 4.0, "%s %s", "You got", lsg_name(ctx[0]))
	lsd_exit(17, "shown")
}

// Player Spawned (ps)
lsg_n18(const ctx[])
{
	lsd_begin()
	if (lsd_enter(18, ctx))
		return
	lsd_exit(18, "ran")
	lsg_n19(ctx)
}

// Strip Weapons (ps_strip)
lsg_n19(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(19, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(19, "Nothing done: that player isn't alive.")
	else
	{
		ls_strip(ctx[0], true)
		lsd_exit(19, "stripped")
	}
	lsg_n20(ctx)
}

// Set Armor (ps_arm)
lsg_n20(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Armor", 50.0)
	if (lsd_enter(20, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(20, "Nothing done: that player isn't alive.")
	else
	{
		ls_set_armor(ctx[0], max(floatround(50.0), 0), true)
		lsd_exit(20, "set")
	}
	lsg_n21(ctx)
}

// Set Speed (ps_spd)
lsg_n21(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Speed", 300.0)
	if (lsd_enter(21, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(21, "Nothing done: that player isn't alive.")
	else
	{
		set_user_maxspeed(ctx[0], 300.0)
		lsd_exit(21, "set")
	}
	lsg_n22(ctx)
}

// Set Team (ps_team)
lsg_n22(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_team("Team", 1)
	if (lsd_enter(22, ctx))
		return
	if (ctx[0] < 1 || ctx[0] > MaxClients || !is_user_connected(ctx[0]))
		lsd_error(22, "Nothing done: nobody there.")
	else
	{
		ls_set_team(ctx[0], CsTeams:(1))
		lsd_exit(22, "moved")
	}
	lsg_n23(ctx)
}

// Teleport (ps_tp)
lsg_n23(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(23, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(23, "Not moved: that player isn't alive.")
	else if (!lsg_teleport(ctx[0], "spawn_a"))
		lsd_error(23, "Not moved: no entity has that name on this map.")
	else
		lsd_exit(23, "teleported")
	lsg_n24(ctx)
}

// Set Variable (ps_set)
lsg_n24(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Value", 0.0)
	if (lsd_enter(24, ctx))
		return
	g_pv0[lsg_slot(ctx[0])] = 0.0
	lsd_exit(24, fmt("%.2f", g_pv0[lsg_slot(ctx[0])]))
}

// Player Joined (pj)
lsg_n25(const ctx[])
{
	lsd_begin()
	if (lsd_enter(25, ctx))
		return
	lsd_exit(25, "ran")
	lsg_n26(ctx)
}

// Show HUD Text (pj_hud)
lsg_n26(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_bool("Value", lsg_is_bot(ctx[0]))
	if (lsd_enter(26, ctx))
		return
	ls_hud(ctx[0], LsHud:1, 4.0, "%s %s", "Bot:", g_lsgYesNo[(lsg_is_bot(ctx[0])) ? 1 : 0])
	lsd_exit(26, "shown")
	lsg_n27(ctx)
}

// Branch (pj_br)
lsg_n27(const ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (ls_alive(ctx[0]) ? 1 : 0))
	if (lsd_enter(27, ctx))
		return
	new ok = (((((ls_alive(ctx[0]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(27, g_lsdBool[ok])
}

// Player Left (pl)
lsg_n28(const ctx[])
{
	lsd_begin()
	if (lsd_enter(28, ctx))
		return
	lsd_exit(28, "ran")
	lsg_n29(ctx)
}

// Set Variable (pl_set)
lsg_n29(const ctx[])
{
	lsd_begin()
	lsd_in_text("Value", "someone left")
	if (lsd_enter(29, ctx))
		return
	copy(g_v3, 63, "someone left")
	lsd_exit(29, "set")
}

// Bomb Planted (bp)
lsg_n30(const ctx[])
{
	lsd_begin()
	if (lsd_enter(30, ctx))
		return
	lsd_exit(30, "ran")
	lsg_n31(ctx)
}

// Show HUD Text (bp_hud)
lsg_n31(const ctx[])
{
	lsd_begin()
	lsd_in_team("Value", lsg_team(ctx[0]))
	if (lsd_enter(31, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Planted by", g_lsgTeamName[clamp(lsg_team(ctx[0]), 0, 3)])
	lsd_exit(31, "shown")
}

// Bomb Defused (bd)
lsg_n32(const ctx[])
{
	lsd_begin()
	if (lsd_enter(32, ctx))
		return
	lsd_exit(32, "ran")
	lsg_n33(ctx)
}

// End Round (bd_end)
lsg_n33(const ctx[])
{
	lsd_begin()
	if (lsd_enter(33, ctx))
		return
	server_cmd("endround CT")
	lsd_exit(33, "Counter-Terrorists win")
}

// Bomb Exploded (bx)
lsg_n34(const ctx[])
{
	lsd_begin()
	if (lsd_enter(34, ctx))
		return
	lsd_exit(34, "ran")
	lsg_n35(ctx)
}

// End Round (bx_end)
lsg_n35(const ctx[])
{
	lsd_begin()
	if (lsd_enter(35, ctx))
		return
	server_cmd("endround T")
	lsd_exit(35, "Terrorists win")
}

// Round Ended (re)
lsg_n36(const ctx[])
{
	lsd_begin()
	if (lsd_enter(36, ctx))
		return
	lsd_exit(36, "ran")
	lsg_n37(ctx)
}

// Branch (re_br)
lsg_n37(const ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ((((float(ls_count_alive(CsTeams:(ctx[0])))) > (0.0) ? 1 : 0)) && (((g_v2) ? 0 : 1)) ? 1 : 0))
	if (lsd_enter(37, ctx))
		return
	new ok = ((((((((float(ls_count_alive(CsTeams:(ctx[0])))) > (0.0) ? 1 : 0)) && (((g_v2) ? 0 : 1)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(37, g_lsdBool[ok])
	if (ok)
		lsg_n38(ctx)
	else
		lsg_n39(ctx)
}

// Set Variable (re_set)
lsg_n38(const ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", 1)
	if (lsd_enter(38, ctx))
		return
	g_v2 = 1
	lsd_exit(38, g_lsdBool[g_v2 ? 1 : 0])
}

// Stop Timers (re_stop)
lsg_n39(const ctx[])
{
	lsd_begin()
	if (lsd_enter(39, ctx))
		return
	lsg_stop_timers()
	lsd_exit(39, "stopped")
}

// Player Used (pu)
lsg_n40(const ctx[])
{
	lsd_begin()
	if (lsd_enter(40, ctx))
		return
	lsd_exit(40, "ran")
	lsg_n41(ctx)
}

// Door (pu_door)
lsg_n41(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Door", ctx[1])
	if (lsd_enter(41, ctx))
		return
	if (!ls_components_running())
		lsd_error(41, "gm_components isn't running on this server.")
	else
	{
		ls_door_open(ctx[1])
		lsd_exit(41, "done")
	}
	lsg_n42(ctx)
}

// Fire Trap (pu_trap)
lsg_n42(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Trap", lsg_find("trap1"))
	if (lsd_enter(42, ctx))
		return
	if (!ls_components_running())
		lsd_error(42, "gm_components isn't running on this server.")
	else
	{
		ls_trap_fire(lsg_find("trap1"))
		lsd_exit(42, "done")
	}
}

// Player Touched (pt)
lsg_n43(const ctx[])
{
	lsd_begin()
	if (lsd_enter(43, ctx))
		return
	lsd_exit(43, "ran")
	lsg_n44(ctx)
}

// Open Crate (pt_crate)
lsg_n44(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Container", ctx[1])
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(44, ctx))
		return
	if (!ls_components_running())
		lsd_error(44, "gm_components isn't running on this server.")
	else
	{
		ls_loot_open(ctx[1], ctx[0])
		lsd_exit(44, "done")
	}
}

// Point Captured (cc)
lsg_n45(const ctx[])
{
	lsd_begin()
	if (lsd_enter(45, ctx))
		return
	lsd_exit(45, "ran")
	lsg_n46(ctx)
}

// Lock Point (cc_lock)
lsg_n46(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Point", ctx[0])
	lsd_in_number("Seconds", 20.0)
	if (lsd_enter(46, ctx))
		return
	if (!ls_components_running())
		lsd_error(46, "gm_components isn't running on this server.")
	else
	{
		ls_cp_lock(ctx[0], 20.0)
		lsd_exit(46, "done")
	}
	lsg_n47(ctx)
}

// Show HUD Text (cc_hud)
lsg_n47(const ctx[])
{
	lsd_begin()
	lsd_in_team("Value", ctx[1])
	if (lsd_enter(47, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %s", "Taken by", g_lsgTeamName[clamp(ctx[1], 0, 3)])
	lsd_exit(47, "shown")
}

// Point Lost (cl)
lsg_n48(const ctx[])
{
	lsd_begin()
	if (lsd_enter(48, ctx))
		return
	lsd_exit(48, "ran")
	lsg_n49(ctx)
}

// Open Extraction (cl_ex)
lsg_n49(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Point", lsg_find("ex1"))
	lsd_in_bool("Open", 1)
	if (lsd_enter(49, ctx))
		return
	if (!ls_components_running())
		lsd_error(49, "gm_components isn't running on this server.")
	else
	{
		ls_extract_open(lsg_find("ex1"), bool:1)
		lsd_exit(49, "done")
	}
}

// Zone Stage (zs)
lsg_n50(const ctx[])
{
	lsd_begin()
	if (lsd_enter(50, ctx))
		return
	lsd_exit(50, "ran")
	lsg_n51(ctx)
}

// Shrink Zone (zs_shrink)
lsg_n51(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Zone", ctx[0])
	lsd_in_number("Radius", ((Float:ctx[1]) * (100.0)))
	if (lsd_enter(51, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(51, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("mid", o))
		lsd_error(51, "No entity has that name on this map.")
	else
	{
		ls_zone_shrink(ctx[0], o, ((Float:ctx[1]) * (100.0)), 20.0)
		lsd_exit(51, "done")
	}
}

// Zone Hurt Player (zt)
lsg_n52(const ctx[])
{
	lsd_begin()
	if (lsd_enter(52, ctx))
		return
	lsd_exit(52, "ran")
	lsg_n53(ctx)
}

// Set Zone (zt_set)
lsg_n53(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Zone", ctx[0])
	lsd_in_number("Radius", 900.0)
	if (lsd_enter(53, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(53, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("mid", o))
		lsd_error(53, "No entity has that name on this map.")
	else
	{
		ls_zone_set(ctx[0], o, 900.0)
		lsd_exit(53, "done")
	}
	lsg_n54(ctx)
}

// Show HUD Text (zt_hud)
lsg_n54(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Value", Float:ctx[2])
	if (lsd_enter(54, ctx))
		return
	ls_hud(ctx[1], LsHud:3, 4.0, "%s %d", "Ouch", floatround(Float:ctx[2]))
	lsd_exit(54, "shown")
}

// Player Extracted (ex)
lsg_n55(const ctx[])
{
	lsd_begin()
	if (lsd_enter(55, ctx))
		return
	lsd_exit(55, "ran")
	lsg_n56(ctx)
}

// Add To Variable (ex_add)
lsg_n56(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Amount", 5.0)
	if (lsd_enter(56, ctx))
		return
	g_pv0[lsg_slot(ctx[1])] += 5.0
	lsd_exit(56, fmt("%.2f", g_pv0[lsg_slot(ctx[1])]))
}

// Wave Started (ws)
lsg_n57(const ctx[])
{
	lsd_begin()
	if (lsd_enter(57, ctx))
		return
	lsd_exit(57, "ran")
	lsg_n58(ctx)
}

// Show HUD Text (ws_hud)
lsg_n58(const ctx[])
{
	lsd_begin()
	lsd_in_number("Value", Float:ctx[0])
	if (lsd_enter(58, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %d", "Wave", floatround(Float:ctx[0]))
	lsd_exit(58, "shown")
}

// Wave Spawned One (wp)
lsg_n59(const ctx[])
{
	lsd_begin()
	if (lsd_enter(59, ctx))
		return
	lsd_exit(59, "ran")
	lsg_n60(ctx)
}

// Show HUD Text (wp_hud)
lsg_n60(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Value", ctx[0])
	if (lsd_enter(60, ctx))
		return
	ls_hud(0, LsHud:2, 4.0, "%s %d", "In:", ctx[0])
	lsd_exit(60, "shown")
}

// Wave Cleared (wc)
lsg_n61(const ctx[])
{
	lsd_begin()
	if (lsd_enter(61, ctx))
		return
	lsd_exit(61, "ran")
	lsg_n62(ctx)
}

// Start Wave (wc_start)
lsg_n62(const ctx[])
{
	lsd_begin()
	lsd_in_number("Wave", ((Float:ctx[0]) + (1.0)))
	if (lsd_enter(62, ctx))
		return
	if (!ls_components_running())
		lsd_error(62, "gm_components isn't running on this server.")
	else
	{
		ls_wave_start(max(floatround(((Float:ctx[0]) + (1.0))), 0))
		lsd_exit(62, "done")
	}
}

// Crate Opened (lo)
lsg_n63(const ctx[])
{
	lsd_begin()
	if (lsd_enter(63, ctx))
		return
	lsd_exit(63, "ran")
	lsg_n64(ctx)
}

// Show HUD Text (lo_hud)
lsg_n64(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(64, ctx))
		return
	ls_hud(ctx[1], LsHud:1, 4.0, "%s", "Looted")
	lsd_exit(64, "shown")
}

// Checkpoint Reached (ck)
lsg_n65(const ctx[])
{
	lsd_begin()
	if (lsd_enter(65, ctx))
		return
	lsd_exit(65, "ran")
	lsg_n66(ctx)
}

// Set Variable (ck_set)
lsg_n66(const ctx[])
{
	lsd_begin()
	lsd_in_number("Value", Float:ctx[2])
	if (lsd_enter(66, ctx))
		return
	g_v1 = Float:ctx[2]
	lsd_exit(66, fmt("%.2f", g_v1))
}

// Barricade Broken (bb)
lsg_n67(const ctx[])
{
	lsd_begin()
	if (lsd_enter(67, ctx))
		return
	lsd_exit(67, "ran")
	lsg_n68(ctx)
}

// Show Message (bb_msg)
lsg_n68(const ctx[])
{
	lsd_begin()
	if (lsd_enter(68, ctx))
		return
	ls_announce(0, 3.0, "%s", "The barricade is down!")
	lsd_exit(68, "shown")
}

// Meeting Ended (me)
lsg_n69(const ctx[])
{
	lsd_begin()
	if (lsd_enter(69, ctx))
		return
	lsd_exit(69, "ran")
	lsg_n70(ctx)
}

// Show HUD Text (me_hud)
lsg_n70(const ctx[])
{
	lsd_begin()
	lsd_in_player("Value", ctx[1])
	if (lsd_enter(70, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Out:", lsg_name(ctx[1]))
	lsd_exit(70, "shown")
}

// Creature Killed (ck2)
lsg_n71(const ctx[])
{
	lsd_begin()
	if (lsd_enter(71, ctx))
		return
	lsd_exit(71, "ran")
	lsg_n72(ctx)
}

// Set Variable (ck2_set)
lsg_n72(const ctx[])
{
	lsd_begin()
	lsd_in_text("Value", g_lsgTxt[0])
	if (lsd_enter(72, ctx))
		return
	copy(g_v3, 63, g_lsgTxt[0])
	lsd_exit(72, "set")
	lsg_n73(ctx)
}

// Create Component (ck2_make)
lsg_n73(const ctx[])
{
	lsd_begin()
	if (lsd_enter(73, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(73, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("crate_spot", o))
		lsd_error(73, "No entity has that name on this map.")
	else
	{
		g_lsgOut[73] = ls_comp_create("ls_loot_container", o, "table=rifles radius=40")
		lsd_exit(73, fmt("entity %d", g_lsgOut[73]))
	}
	lsg_n74(ctx)
}

// Spawn Creature (ck2_spawn)
lsg_n74(const ctx[])
{
	lsd_begin()
	if (lsd_enter(74, ctx))
		return
	if (!get_cvar_pointer("ls_creature_ver"))
		lsd_error(74, "This server's bot brain has no creatures (ls_creature).")
	else if (!(g_lsgOut[74] = lsg_creature("zombie", "spot1")))
		lsd_error(74, "Not spawned: no entity has that name, or that type isn't loaded on this map.")
	else
		lsd_exit(74, fmt("entity %d", g_lsgOut[74]))
	lsg_n75(ctx)
}

// Show HUD Text (ck2_hud)
lsg_n75(const ctx[])
{
	lsd_begin()
	lsd_in_entity("Value", g_lsgOut[74])
	if (lsd_enter(75, ctx))
		return
	ls_hud(0, LsHud:2, 4.0, "%s %d", "New one:", g_lsgOut[74])
	lsd_exit(75, "shown")
	lsg_n76(ctx)
}

// Show HUD Text (ck2_hud2)
lsg_n76(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_text("Value", g_v3)
	if (lsd_enter(76, ctx))
		return
	ls_hud(ctx[1], LsHud:1, 4.0, "%s %s", "Killed:", g_v3)
	lsd_exit(76, "shown")
}

lsd_call(n, const ctx[])
{
	switch (n)
	{
		case 0: lsg_n0(ctx)
		case 1: lsg_n1(ctx)
		case 2: lsg_n2(ctx)
		case 3: lsg_n3(ctx)
		case 4: lsg_n4(ctx)
		case 5: lsg_n5(ctx)
		case 6: lsg_n6(ctx)
		case 7: lsg_n7(ctx)
		case 8: lsg_n8(ctx)
		case 9: lsg_n9(ctx)
		case 10: lsg_n10(ctx)
		case 11: lsg_n11(ctx)
		case 12: lsg_n12(ctx)
		case 13: lsg_n13(ctx)
		case 14: lsg_n14(ctx)
		case 15: lsg_n15(ctx)
		case 16: lsg_n16(ctx)
		case 17: lsg_n17(ctx)
		case 18: lsg_n18(ctx)
		case 19: lsg_n19(ctx)
		case 20: lsg_n20(ctx)
		case 21: lsg_n21(ctx)
		case 22: lsg_n22(ctx)
		case 23: lsg_n23(ctx)
		case 24: lsg_n24(ctx)
		case 25: lsg_n25(ctx)
		case 26: lsg_n26(ctx)
		case 27: lsg_n27(ctx)
		case 28: lsg_n28(ctx)
		case 29: lsg_n29(ctx)
		case 30: lsg_n30(ctx)
		case 31: lsg_n31(ctx)
		case 32: lsg_n32(ctx)
		case 33: lsg_n33(ctx)
		case 34: lsg_n34(ctx)
		case 35: lsg_n35(ctx)
		case 36: lsg_n36(ctx)
		case 37: lsg_n37(ctx)
		case 38: lsg_n38(ctx)
		case 39: lsg_n39(ctx)
		case 40: lsg_n40(ctx)
		case 41: lsg_n41(ctx)
		case 42: lsg_n42(ctx)
		case 43: lsg_n43(ctx)
		case 44: lsg_n44(ctx)
		case 45: lsg_n45(ctx)
		case 46: lsg_n46(ctx)
		case 47: lsg_n47(ctx)
		case 48: lsg_n48(ctx)
		case 49: lsg_n49(ctx)
		case 50: lsg_n50(ctx)
		case 51: lsg_n51(ctx)
		case 52: lsg_n52(ctx)
		case 53: lsg_n53(ctx)
		case 54: lsg_n54(ctx)
		case 55: lsg_n55(ctx)
		case 56: lsg_n56(ctx)
		case 57: lsg_n57(ctx)
		case 58: lsg_n58(ctx)
		case 59: lsg_n59(ctx)
		case 60: lsg_n60(ctx)
		case 61: lsg_n61(ctx)
		case 62: lsg_n62(ctx)
		case 63: lsg_n63(ctx)
		case 64: lsg_n64(ctx)
		case 65: lsg_n65(ctx)
		case 66: lsg_n66(ctx)
		case 67: lsg_n67(ctx)
		case 68: lsg_n68(ctx)
		case 69: lsg_n69(ctx)
		case 70: lsg_n70(ctx)
		case 71: lsg_n71(ctx)
		case 72: lsg_n72(ctx)
		case 73: lsg_n73(ctx)
		case 74: lsg_n74(ctx)
		case 75: lsg_n75(ctx)
		case 76: lsg_n76(ctx)
	}
}
