/*
	Every Node - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from every_node.graph.json: change the graph and compile it again; edits here are lost.
	This is a debug build (Playtest): it reports every node to the Studio.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "every_node"
#define LSG_CTX 22
#define LSG_TXT 2
#define LSG_N 248
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }
#define LSG_VR 256
#define LSG_VP 290
#define LSG_RR 64
#define LSG_RP 65
#define LSG_RC 6
#define LSG_RT 1
#define LSG_AC 70
#define LSG_AMAX 10000
#define LSG_TMPA 32
#define LSG_O 8
#define LSG_OT 2

// The mode's variables
stock Float:g_pv0[33] // Kills (number, per player)
stock Float:g_v1 = 1.0 // Round (number)
stock g_v2 = 0 // Flag (bool)
stock g_v3[64] = "none" // Last thing (text)
stock Float:g_tv4[4] = { 0.0, 0.0, 0.0, 0.0 } // Team score (number, per team, each round)
stock g_v5[64] = "wait" // Phase (text)
// Spot is in slot (LSG_VR + 0)
// Player spot is in slot (LSG_VR + 1 + lsg_slot(0))
stock g_v8 = 0 // Names (Array of Text)
stock g_v9 = 0 // Numbers (Array of Number, each round)
stock g_v10 = 0 // Players (Array of Player)
stock g_v11 = 0 // Loadouts (Array of Loadout)
stock g_v12 = 0 // Scores (Map of Player to Number)
stock g_v13 = 0 // Words (Map of Text to Text)
stock g_v14 = 0 // Marks (Map of Number to Vector)
// Loadout is in slot (LSG_RR + 0)
stock g_pv16[33] // Class (Class, per player)
stock g_v17 = 0 // Timer (Timer)
stock Float:g_v18 = 0.0 // Sum (number)
stock g_v19 = 0 // Seen (Array of Player)

stock Float:g_lsgDmg, bool:g_lsgDmgBlock
stock g_lsgNoFall[33]
stock g_lsgM93[33][LSG_CTX]
#define LSD_N 248
new const g_lsdIds[LSD_N][] = { "rs", "rs_set", "rs_cd", "rs_msg", "rs_rep", "rs_hud", "rs_delay", "rs_wave", "rs_wstop", "rs_reset", "pk", "pk_br", "pk_add", "pk_add2", "pk_team", "pk_give", "pk_seth", "pk_hud", "ps", "ps_strip", "ps_arm", "ps_spd", "ps_team", "ps_tp", "ps_set", "pj", "pj_hud", "pj_br", "pl", "pl_set", "bp", "bp_hud", "bd", "bd_end", "bx", "bx_end", "re", "re_br", "re_set", "re_stop", "pu", "pu_door", "pu_trap", "pt", "pt_crate", "cc", "cc_lock", "cc_hud", "cl", "cl_ex", "zs", "zs_shrink", "zt", "zt_set", "zt_hud", "ex", "ex_add", "ws", "ws_hud", "wp", "wp_hud", "wc", "wc_start", "lo", "lo_hud", "ck", "ck_set", "bb", "bb_msg", "me", "me_hud", "ck2", "ck2_set", "ck2_make", "ck2_spawn", "ck2_hud", "ck2_hud2", "hurt", "hurt_tadd", "hurt_glow", "tc", "tc_model", "nr", "nr_cvar", "nr_each", "nr_resp", "nr_pick", "nr_ents", "nr_use", "nr_set", "nr_stop", "nr_hud", "cmd", "cmd_menu", "cmd_arm", "cmd_money", "cmd_grav", "cmd_bar", "cmd_chat", "fn", "fn_frag", "fn_snd", "fn_role", "pk2", "pk2_call", "pk2_pick", "pk2_bot", "pk2_hold", "pk2_follow", "pk2_free", "pk2_drop", "pk2_fr", "pk2_kill", "lo2", "lo2_br", "lo2_pick", "lo2_br2", "lo2_resp", "lo2_drop", "lo2_hud", "lo2_br3", "lo2_end", "ms", "ms_pack", "ms_pick", "ms_start", "ms_stop", "ms_make", "ms_drop", "ms_each", "ms_tp", "ah", "ah_br", "ah_block", "ah_scale", "ah_set", "dk", "dk_down", "rv", "rv_hud", "to", "to_add", "to_set", "to_hud", "to_br", "sq", "sq_nf", "sq_set", "sq_ff", "sq_hud", "sq_buy", "sq_allow", "sq_up", "sq_goto", "ck3", "ck3_set", "ck3_add", "ck3_p", "ck3_r", "ck3_hud", "tu", "tu_end", "a0", "a_seq", "a_for", "a_add", "a_sort", "a_shuf", "a_ins", "a_rmi", "a_aset", "a_arem", "a_fe", "a_br", "a_pr", "a_pr2", "a_clr", "b_set", "b_wh", "b_add", "b_sw", "b_pr1", "b_sn", "b_pr2", "b_pr3", "c_each", "c_setp", "c_pos", "c_vel", "c_pr", "c_sl", "c_sf", "c_sf2", "c_push", "c_pr2", "c_sw", "c_pr3", "c_pr4", "d_each", "d_madd", "d_call", "d_pr", "d_pr2", "d_tadd", "d_vadd", "d_rem", "d_pr3", "d_mclr", "d_pr4", "d_pr5", "d_tbf", "d_tset", "d_pr6", "d_tclr", "d_pr7", "e0", "e_once", "e_n", "e_gate", "e_ff", "e_iv", "e_rt", "e_pr", "e_pr2", "e_seq", "f0", "f_brk", "f_br", "f_pr", "f_pr2", "f_pr3", "f_pr4", "f_setc", "f_sw", "f_pr5", "f_sp", "f_pr6", "fa_e", "fa_set", "fa_push", "fa_br", "fa_r1", "fa_r2", "fb_e", "fb_r", "fc_e", "fc_pr", "d_mac" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
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

// (gen.rs's plugin_end calls this)
stock lsd_end()
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
	// (a player still joining has no team data yet: asking would be an error)
	new team = pev_valid(id) == 2 ? (_:cs_get_user_team(id)) : 0
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

stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

stock bool:lsg_ok(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id)

stock bool:lsg_match(p, which)
{
	if (!is_user_connected(p))
		return false
	switch (which)
	{
		case 1: return bool:is_user_alive(p)
		case 2: return !is_user_alive(p) && (lsg_team(p) == 1 || lsg_team(p) == 2)
		case 3: return lsg_team(p) == 1
		case 4: return lsg_team(p) == 2
		case 5: return is_user_alive(p) && lsg_team(p) == 1
		case 6: return is_user_alive(p) && lsg_team(p) == 2
		case 7: return bool:is_user_bot(p)
		case 8: return !is_user_bot(p)
	}
	return true
}

stock lsg_end_round(team)
{
	if (team == 1)
		server_cmd("endround T")
	else if (team == 2)
		server_cmd("endround CT")
	else
		server_cmd("endround")
}

stock bool:lsg_use(const name[])
{
	new ent = -1, used
	while (name[0] && used < 64 && (ent = find_ent_by_tname(ent, name)) > 0)
	{
		ExecuteHamB(Ham_Use, ent, 0, 0, 2, 1.0)
		used++
	}
	return used > 0
}

stock lsg_bar(id, secs)
{
	if (id && !lsg_ok(id))
		return
	message_begin(id ? MSG_ONE : MSG_ALL, get_user_msgid("BarTime"), _, id)
	write_short(secs)
	message_end()
}

stock lsg_add_money(id, amount)
{
	if (pev_valid(id) == 2)
		cs_set_user_money(id, clamp(cs_get_user_money(id) + amount, 0, 16000), 1)
}

stock Float:lsg_get_money(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_money(id)) : 0.0

stock lsg_add_frags(id, amount)
{
	set_user_frags(id, get_user_frags(id) + amount)
	// (setting the deaths again sends the scoreboard its new numbers)
	if (pev_valid(id) == 2)
		cs_set_user_deaths(id, cs_get_user_deaths(id))
}

stock Float:lsg_get_frags(id)
	return lsg_ok(id) ? float(get_user_frags(id)) : 0.0

stock Float:lsg_get_deaths(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_deaths(id)) : 0.0

stock lsg_freeze(id, on)
{
	new flags = pev(id, pev_flags)
	set_pev(id, pev_flags, on ? (flags | FL_FROZEN) : (flags & ~FL_FROZEN))
}

stock lsg_bot_goto(bot, const name[], secs)
{
	new Float:o[3]
	if (!lsg_origin_of(name, o))
		return
	ls_bot_goto(bot, o)
	// a timed goal: the bot is free again after it
	remove_task(LSG_TASK - 100 - bot)
	if (secs > 0)
		set_task(float(secs), "lsg_bot_free", LSG_TASK - 100 - bot)
}

public lsg_bot_free(task)
	ls_bot_release(LSG_TASK - 100 - task)

stock Float:lsg_mod(Float:a, Float:b)
{
	if (b == 0.0)
		return 0.0
	return a - b * float(floatround(a / b, floatround_floor))
}

stock Float:lsg_pow(Float:a, Float:b)
{
	new Float:r = floatpower(a, b)
	if (r != r)
		return 0.0
	return floatclamp(r, -100000.0, 100000.0)
}

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
	return id >= 1 && id <= MaxClients && is_user_connected(id) && pev_valid(id) == 2 ? (_:cs_get_user_team(id)) : 0

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
	// "#12": entity 12 (an entity wired in, not a name)
	if (name[0] == '#')
		return lsg_ent_origin(str_to_num(name[1]), o)
	return lsg_ent_origin(lsg_find(name), o)
}

stock bool:lsg_ent_origin(ent, Float:o[3])
{
	if (ent <= 0 || !pev_valid(ent))
		return false
	pev(ent, pev_origin, o)
	// (a brush entity's origin is often 0 0 0: its middle instead)
	if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new k = 0; k < 3; k++)
			o[k] = (mins[k] + maxs[k]) / 2.0
	}
	return true
}

stock bool:lsg_teleport_to(id, ent)
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_ent_origin(ent, o))
		return false
	ls_teleport(id, o)
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
	return lsg_creature_o(kind, o)
}

/// (an entity's name for lsg_creature: "" picks no entity, so its origin is looked up as #index)
stock lsg_ent_find_origin(ent)
{
	new s[16]
	formatex(s, charsmax(s), "#%d", ent)
	return s
}

stock lsg_creature_o(const kind[], const Float:o[3])
{
	if (!get_cvar_pointer("ls_creature_ver"))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
}

stock lsg_dmg_kind(bits, out[], len)
{
	if (bits & DMG_FALL)
		copy(out, len, "fall")
	else if (bits & DMG_BLAST)
		copy(out, len, "blast")
	else if (bits & DMG_BURN)
		copy(out, len, "burn")
	else if (bits & DMG_BULLET)
		copy(out, len, "shot")
	else if (bits & DMG_SLASH)
		copy(out, len, "slash")
	else
		copy(out, len, "other")
}

stock lsg_launch(id, Float:speed)
{
	new Float:v[3]
	pev(id, pev_velocity, v)
	v[2] = speed
	set_pev(id, pev_velocity, v)
}

stock lsg_round_time(secs)
{
	message_begin(MSG_ALL, get_user_msgid("RoundTime"))
	write_short(max(secs, 0))
	message_end()
}

// ---- graph version 3: vectors, structs, arrays, maps, text, timers
// Vectors and structs are slots (a ring that's reused, then each variable's own slots); a
// value is the slot's number. Arrays and maps are handles. Every index is clamped and every
// handle and size checked, so a graph never reads or writes outside them.
stock Float:g_lsgVec[LSG_VP][3], g_lsgVecAt
stock g_lsgRec[LSG_RP][LSG_RC], g_lsgRecT[LSG_RP][LSG_RT][64], g_lsgRecAt
stock g_lsgO[LSG_O], g_lsgOT[LSG_OT][128]
stock g_lsgTmpA[LSG_TMPA], g_lsgTmpAt
stock const g_lsgRun[][] = { "stopped", "running" }

stock lsg_vi(v)
	return clamp(v, 0, LSG_VP - 1)

stock lsg_vnew()
{
	if (++g_lsgVecAt >= LSG_VR)
		g_lsgVecAt = 0
	return g_lsgVecAt
}

stock lsg_vec(Float:x, Float:y, Float:z)
{
	new v = lsg_vnew()
	g_lsgVec[v][0] = x
	g_lsgVec[v][1] = y
	g_lsgVec[v][2] = z
	return v
}

stock Float:lsg_vc(v, k)
	return g_lsgVec[lsg_vi(v)][clamp(k, 0, 2)]

stock lsg_vset(dst, src)
{
	new d = lsg_vi(dst), s = lsg_vi(src)
	for (new k = 0; k < 3; k++)
		g_lsgVec[d][k] = g_lsgVec[s][k]
}

stock lsg_vzero(dst)
{
	new d = lsg_vi(dst)
	for (new k = 0; k < 3; k++)
		g_lsgVec[d][k] = 0.0
}

stock lsg_vfrom(const Float:o[3])
	return lsg_vec(o[0], o[1], o[2])

stock lsg_vto(v, Float:o[3])
{
	for (new k = 0; k < 3; k++)
		o[k] = lsg_vc(v, k)
}

stock lsg_vadd(a, b, Float:sign)
	return lsg_vec(lsg_vc(a, 0) + sign * lsg_vc(b, 0), lsg_vc(a, 1) + sign * lsg_vc(b, 1), lsg_vc(a, 2) + sign * lsg_vc(b, 2))

stock lsg_vscale(a, Float:n)
	return lsg_vec(lsg_vc(a, 0) * n, lsg_vc(a, 1) * n, lsg_vc(a, 2) * n)

stock Float:lsg_vlen(a)
	return floatsqroot(lsg_vc(a, 0) * lsg_vc(a, 0) + lsg_vc(a, 1) * lsg_vc(a, 1) + lsg_vc(a, 2) * lsg_vc(a, 2))

stock Float:lsg_vdist(a, b)
{
	new Float:x = lsg_vc(a, 0) - lsg_vc(b, 0), Float:y = lsg_vc(a, 1) - lsg_vc(b, 1), Float:z = lsg_vc(a, 2) - lsg_vc(b, 2)
	return floatsqroot(x * x + y * y + z * z)
}

stock lsg_vnorm(a)
{
	new Float:l = lsg_vlen(a)
	if (l < 0.0001)
		return lsg_vec(0.0, 0.0, 0.0)
	return lsg_vscale(a, 1.0 / l)
}

stock lsg_vdir(from, to)
	return lsg_vnorm(lsg_vadd(to, from, -1.0))

stock bool:lsg_veq(a, b)
	return lsg_vc(a, 0) == lsg_vc(b, 0) && lsg_vc(a, 1) == lsg_vc(b, 1) && lsg_vc(a, 2) == lsg_vc(b, 2)

stock lsg_vtext(v)
{
	new t[64]
	formatex(t, charsmax(t), "%.0f %.0f %.0f", lsg_vc(v, 0), lsg_vc(v, 1), lsg_vc(v, 2))
	return t
}

// where a player is: 0 feet, 1 eyes, 2 aim direction, 3 what the aim hits, 4 velocity
stock lsg_vplayer(id, what)
{
	new Float:o[3]
	if (!lsg_ok(id))
		return lsg_vfrom(o)
	new Float:ofs[3], Float:ang[3], Float:fwd[3], Float:end[3]
	switch (what)
	{
		case 1, 3:
		{
			pev(id, pev_origin, o)
			pev(id, pev_view_ofs, ofs)
			for (new k = 0; k < 3; k++)
				o[k] += ofs[k]
			if (what == 3)
			{
				pev(id, pev_v_angle, ang)
				angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
				for (new k = 0; k < 3; k++)
					end[k] = o[k] + fwd[k] * 8192.0
				engfunc(EngFunc_TraceLine, o, end, DONT_IGNORE_MONSTERS, id, 0)
				get_tr2(0, TR_vecEndPos, o)
			}
		}
		case 2:
		{
			pev(id, pev_v_angle, ang)
			angle_vector(ang, ANGLEVECTOR_FORWARD, o)
		}
		case 4:
			pev(id, pev_velocity, o)
		default:
			pev(id, pev_origin, o)
	}
	return lsg_vfrom(o)
}

// where an entity is (a brush entity: the middle of its box)
stock lsg_vent(ent)
{
	new Float:o[3]
	if (ent < 1 || !pev_valid(ent))
		return lsg_vfrom(o)
	pev(ent, pev_origin, o)
	if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new k = 0; k < 3; k++)
			o[k] = (mins[k] + maxs[k]) * 0.5
	}
	return lsg_vfrom(o)
}

stock lsg_vsetpos(id, v)
{
	new Float:o[3], Float:still[3]
	lsg_vto(v, o)
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_velocity, still)
}

stock lsg_vsetvel(id, v)
{
	new Float:o[3]
	lsg_vto(v, o)
	for (new k = 0; k < 3; k++)
		o[k] = floatclamp(o[k], -4000.0, 4000.0)
	set_pev(id, pev_velocity, o)
}

// ---- structs
stock lsg_ri(r)
	return clamp(r, 0, LSG_RP - 1)

stock lsg_rzero(r)
{
	new i = lsg_ri(r)
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[i][k] = 0
	for (new k = 0; k < LSG_RT; k++)
		g_lsgRecT[i][k][0] = 0
}

stock lsg_rnew()
{
	if (++g_lsgRecAt >= LSG_RR)
		g_lsgRecAt = 0
	lsg_rzero(g_lsgRecAt)
	return g_lsgRecAt
}

stock lsg_rset(dst, src)
{
	new d = lsg_ri(dst), s = lsg_ri(src)
	if (d == s)
		return
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[d][k] = g_lsgRec[s][k]
	for (new k = 0; k < LSG_RT; k++)
		copy(g_lsgRecT[d][k], 63, g_lsgRecT[s][k])
}

stock bool:lsg_req(a, b)
{
	new x = lsg_ri(a), y = lsg_ri(b)
	for (new k = 0; k < LSG_RC; k++)
		if (g_lsgRec[x][k] != g_lsgRec[y][k])
			return false
	for (new k = 0; k < LSG_RT; k++)
		if (!equal(g_lsgRecT[x][k], g_lsgRecT[y][k]))
			return false
	return true
}

stock lsg_rvec(r, off)
{
	new i = lsg_ri(r)
	return lsg_vec(Float:g_lsgRec[i][off], Float:g_lsgRec[i][off + 1], Float:g_lsgRec[i][off + 2])
}

stock lsg_rsetvec(r, off, v)
{
	new i = lsg_ri(r)
	for (new k = 0; k < 3; k++)
		g_lsgRec[i][off + k] = _:lsg_vc(v, k)
}

stock lsg_rpack(r, b[LSG_AC])
{
	new i = lsg_ri(r)
	for (new k = 0; k < LSG_RC; k++)
		b[k] = g_lsgRec[i][k]
	for (new t = 0; t < LSG_RT; t++)
		for (new c = 0; c < 64; c++)
			b[LSG_RC + t * 64 + c] = g_lsgRecT[i][t][c]
}

stock lsg_runpack(const b[LSG_AC])
{
	new r = lsg_rnew()
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[r][k] = b[k]
	for (new t = 0; t < LSG_RT; t++)
	{
		for (new c = 0; c < 63; c++)
			g_lsgRecT[r][t][c] = b[LSG_RC + t * 64 + c]
		g_lsgRecT[r][t][63] = 0
	}
	return r
}

// ---- arrays (every array holds items of LSG_AC cells; at most LSG_AMAX items)
stock lsg_atmp()
{
	if (++g_lsgTmpAt >= LSG_TMPA)
		g_lsgTmpAt = 0
	if (!g_lsgTmpA[g_lsgTmpAt])
		g_lsgTmpA[g_lsgTmpAt] = _:ArrayCreate(LSG_AC)
	else
		ArrayClear((Array:g_lsgTmpA[g_lsgTmpAt]))
	return g_lsgTmpA[g_lsgTmpAt]
}

stock lsg_alen(a)
	return a ? ArraySize((Array:a)) : 0

// an index, or -1 when it's outside the array
stock lsg_aidx(a, Float:i)
{
	new k = floatround(i, floatround_floor)
	return k >= 0 && k < lsg_alen(a) ? k : -1
}

stock bool:lsg_aroom(a)
	return a && ArraySize((Array:a)) < LSG_AMAX

stock lsg_aremi(a, Float:i)
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayDeleteItem((Array:a), k)
}

stock lsg_aclear(a)
{
	if (a)
		ArrayClear((Array:a))
}

stock lsg_ashuffle(a)
{
	for (new k = lsg_alen(a) - 1; k > 0; k--)
		ArraySwap((Array:a), k, random(k + 1))
}

// kind: 0 numbers, 1 by cell, 2 text
stock lsg_asort(a, kind, desc)
{
	if (lsg_alen(a) < 2)
		return
	new d[2]
	d[0] = kind
	d[1] = desc
	ArraySort((Array:a), "lsg_acmp", d, 2)
}

public lsg_acmp(Array:a, i1, i2, const d[], size)
{
	#pragma unused size
	new r
	switch (d[0])
	{
		case 0:
		{
			new Float:x = Float:ArrayGetCell(a, i1), Float:y = Float:ArrayGetCell(a, i2)
			r = x < y ? -1 : (x > y ? 1 : 0)
		}
		case 2:
		{
			new s1[64], s2[64]
			ArrayGetString(a, i1, s1, 63)
			ArrayGetString(a, i2, s2, 63)
			r = strcmp(s1, s2, true)
			r = r < 0 ? -1 : (r > 0 ? 1 : 0)
		}
		default:
		{
			new x = ArrayGetCell(a, i1), y = ArrayGetCell(a, i2)
			r = x < y ? -1 : (x > y ? 1 : 0)
		}
	}
	return d[1] ? -r : r
}

stock lsg_acopy(dst, src)
{
	if (!dst || dst == src)
		return
	ArrayClear((Array:dst))
	new n = min(lsg_alen(src), LSG_AMAX), b[LSG_AC]
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:src), k, b)
		ArrayPushArray((Array:dst), b)
	}
}

stock lsg_aplayers(which)
{
	new a = lsg_atmp()
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, which))
			ArrayPushCell((Array:a), p)
	return a
}

// items in a cell
stock lsg_apushc(a, v)
	return lsg_aroom(a) ? ArrayPushCell((Array:a), v) : -1

stock lsg_agetc(a, Float:i)
{
	new k = lsg_aidx(a, i)
	return k < 0 ? 0 : ArrayGetCell((Array:a), k)
}

stock lsg_asetc(a, Float:i, v)
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArraySetCell((Array:a), k, v)
}

stock lsg_ainsc(a, Float:i, v)
{
	if (!lsg_aroom(a))
		return
	new k = floatround(i, floatround_floor)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushCell((Array:a), v)
	else
		ArrayInsertCellBefore((Array:a), k, v)
}

stock lsg_afindc(a, v)
{
	new n = lsg_alen(a)
	for (new k = 0; k < n; k++)
		if (ArrayGetCell((Array:a), k) == v)
			return k
	return -1
}

stock bool:lsg_aremc(a, v)
{
	new bool:any
	for (new k = lsg_alen(a) - 1; k >= 0; k--)
		if (ArrayGetCell((Array:a), k) == v)
		{
			ArrayDeleteItem((Array:a), k)
			any = true
		}
	return any
}

stock lsg_arandc(a)
{
	new n = lsg_alen(a)
	return n ? ArrayGetCell((Array:a), random(n)) : 0
}

// text items
stock lsg_apushs(a, const s[])
	return lsg_aroom(a) ? ArrayPushString((Array:a), s) : -1

stock lsg_agets(a, Float:i)
{
	new t[64], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetString((Array:a), k, t, 63)
	return t
}

stock lsg_asets(a, Float:i, const s[])
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArraySetString((Array:a), k, s)
}

stock lsg_ainss(a, Float:i, const s[])
{
	if (!lsg_aroom(a))
		return
	new k = floatround(i, floatround_floor)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushString((Array:a), s)
	else
		ArrayInsertStringBefore((Array:a), k, s)
}

stock lsg_afinds(a, const s[])
{
	new n = lsg_alen(a), t[64]
	for (new k = 0; k < n; k++)
	{
		ArrayGetString((Array:a), k, t, 63)
		if (equal(t, s))
			return k
	}
	return -1
}

stock bool:lsg_arems(a, const s[])
{
	new bool:any, t[64]
	for (new k = lsg_alen(a) - 1; k >= 0; k--)
	{
		ArrayGetString((Array:a), k, t, 63)
		if (equal(t, s))
		{
			ArrayDeleteItem((Array:a), k)
			any = true
		}
	}
	return any
}

stock lsg_arands(a)
{
	new t[64], n = lsg_alen(a)
	if (n)
		ArrayGetString((Array:a), random(n), t, 63)
	return t
}

// vector items (3 cells)
stock lsg_vbuf(v, b[LSG_AC])
{
	for (new k = 0; k < 3; k++)
		b[k] = _:lsg_vc(v, k)
}

stock lsg_apushv(a, v)
{
	if (!lsg_aroom(a))
		return -1
	new b[LSG_AC]
	lsg_vbuf(v, b)
	return ArrayPushArray((Array:a), b)
}

stock lsg_agetv(a, Float:i)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetArray((Array:a), k, b)
	return lsg_vec(Float:b[0], Float:b[1], Float:b[2])
}

stock lsg_asetv(a, Float:i, v)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	lsg_vbuf(v, b)
	if (k >= 0)
		ArraySetArray((Array:a), k, b)
}

stock lsg_ainsv(a, Float:i, v)
{
	if (!lsg_aroom(a))
		return
	new b[LSG_AC], k = floatround(i, floatround_floor)
	lsg_vbuf(v, b)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushArray((Array:a), b)
	else
		ArrayInsertArrayBefore((Array:a), k, b)
}

stock lsg_afindv(a, v)
{
	new n = lsg_alen(a), b[LSG_AC]
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:a), k, b)
		if (Float:b[0] == lsg_vc(v, 0) && Float:b[1] == lsg_vc(v, 1) && Float:b[2] == lsg_vc(v, 2))
			return k
	}
	return -1
}

stock bool:lsg_aremv(a, v)
{
	new bool:any, k
	while ((k = lsg_afindv(a, v)) >= 0)
	{
		ArrayDeleteItem((Array:a), k)
		any = true
	}
	return any
}

stock lsg_arandv(a)
{
	new n = lsg_alen(a)
	return lsg_agetv(a, n ? float(random(n)) : -1.0)
}

// struct items (packed)
stock lsg_apushr(a, r)
{
	if (!lsg_aroom(a))
		return -1
	new b[LSG_AC]
	lsg_rpack(r, b)
	return ArrayPushArray((Array:a), b)
}

stock lsg_agetr(a, Float:i)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetArray((Array:a), k, b)
	return lsg_runpack(b)
}

stock lsg_asetr(a, Float:i, r)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	lsg_rpack(r, b)
	if (k >= 0)
		ArraySetArray((Array:a), k, b)
}

stock lsg_ainsr(a, Float:i, r)
{
	if (!lsg_aroom(a))
		return
	new b[LSG_AC], k = floatround(i, floatround_floor)
	lsg_rpack(r, b)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushArray((Array:a), b)
	else
		ArrayInsertArrayBefore((Array:a), k, b)
}

stock lsg_afindr(a, r)
{
	new n = lsg_alen(a), want[LSG_AC], b[LSG_AC]
	lsg_rpack(r, want)
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:a), k, b)
		new same = 1
		for (new c = 0; c < LSG_AC && same; c++)
			if (b[c] != want[c])
				same = 0
		if (same)
			return k
	}
	return -1
}

stock bool:lsg_aremr(a, r)
{
	new bool:any, k
	while ((k = lsg_afindr(a, r)) >= 0)
	{
		ArrayDeleteItem((Array:a), k)
		any = true
	}
	return any
}

stock lsg_arandr(a)
{
	new n = lsg_alen(a)
	return lsg_agetr(a, n ? float(random(n)) : -1.0)
}

// ---- maps: keys are text (a number written out, a player as #userid)
stock lsg_kn(Float:n)
{
	new k[24]
	formatex(k, charsmax(k), "%.3f", n)
	return k
}

stock lsg_kp(id)
{
	new k[16]
	if (lsg_ok(id))
		formatex(k, charsmax(k), "#%d", get_user_userid(id))
	return k
}

stock lsg_by_userid(u)
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && get_user_userid(p) == u)
			return p
	return 0
}

stock lsg_mlen(m)
	return m ? TrieGetSize((Trie:m)) : 0

stock bool:lsg_mhas(m, const k[])
	return m && TrieKeyExists((Trie:m), k)

stock lsg_mdel(m, const k[])
{
	if (m)
		TrieDeleteKey((Trie:m), k)
}

stock lsg_mclear(m)
{
	if (m)
		TrieClear((Trie:m))
}

stock lsg_msetc(m, const k[], v)
{
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetCell((Trie:m), k, v)
}

stock lsg_msets(m, const k[], const s[])
{
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetString((Trie:m), k, s)
}

stock lsg_msetv(m, const k[], v)
{
	new Float:b[3]
	lsg_vto(v, b)
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetArray((Trie:m), k, b, 3)
}

stock lsg_mgetc(m, const k[])
{
	new v
	if (m)
		TrieGetCell((Trie:m), k, v)
	return v
}

stock lsg_mgets(m, const k[])
{
	new t[64]
	if (m)
		TrieGetString((Trie:m), k, t, 63)
	return t
}

stock lsg_mgetv(m, const k[])
{
	new Float:b[3]
	if (m)
		TrieGetArray((Trie:m), k, b, 3)
	return lsg_vfrom(b)
}

// keys: 0 text, 1 numbers, 2 players
stock lsg_mkeys(m, kind)
{
	new a = lsg_atmp()
	if (!m)
		return a
	new TrieIter:it = TrieIterCreate(Trie:m), k[64], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		TrieIterGetKey(it, k, charsmax(k))
		switch (kind)
		{
			case 1: ArrayPushCell((Array:a), _:str_to_float(k))
			case 2:
			{
				new p = lsg_by_userid(str_to_num(k[1]))
				if (p)
					ArrayPushCell((Array:a), p)
			}
			default: ArrayPushString((Array:a), k)
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
	return a
}

// values: 0 cells, 1 text, 2 vectors
stock lsg_mvalues(m, kind)
{
	new a = lsg_atmp()
	if (!m)
		return a
	new TrieIter:it = TrieIterCreate(Trie:m), t[64], v, Float:b[3], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		switch (kind)
		{
			case 1:
			{
				TrieIterGetString(it, t, charsmax(t))
				ArrayPushString((Array:a), t)
			}
			case 2:
			{
				TrieIterGetArray(it, b, 3)
				lsg_apushv(a, lsg_vfrom(b))
			}
			default:
			{
				TrieIterGetCell(it, v)
				ArrayPushCell((Array:a), v)
			}
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
	return a
}

stock lsg_mcopy(dst, src, kind)
{
	if (!dst || dst == src)
		return
	TrieClear((Trie:dst))
	if (!src)
		return
	new TrieIter:it = TrieIterCreate((Trie:src)), k[64], t[64], v, Float:b[3], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		TrieIterGetKey(it, k, charsmax(k))
		switch (kind)
		{
			case 1:
			{
				TrieIterGetString(it, t, charsmax(t))
				TrieSetString((Trie:dst), k, t)
			}
			case 2:
			{
				TrieIterGetArray(it, b, 3)
				TrieSetArray((Trie:dst), k, b, 3)
			}
			default:
			{
				TrieIterGetCell(it, v)
				TrieSetCell((Trie:dst), k, v)
			}
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
}

// ---- text, numbers, things
stock lsg_tcat(const a[], const b[])
{
	new t[128]
	formatex(t, charsmax(t), "%s%s", a, b)
	return t
}

stock lsg_tsel(c, const a[], const b[])
{
	new t[128]
	if (c)
		copy(t, charsmax(t), a)
	else
		copy(t, charsmax(t), b)
	return t
}

stock lsg_ntext(Float:n)
{
	new t[24]
	if (floatfract(n) == 0.0 && floatabs(n) < 1000000.0)
		formatex(t, charsmax(t), "%d", floatround(n))
	else
		formatex(t, charsmax(t), "%.2f", n)
	return t
}

stock lsg_etext(e)
{
	new t[32]
	formatex(t, charsmax(t), "entity %d", e)
	return t
}

stock Float:lsg_sign(Float:a)
	return a > 0.0 ? 1.0 : (a < 0.0 ? -1.0 : 0.0)

stock Float:lsg_clampf(Float:a, Float:lo, Float:hi)
{
	if (lo > hi)
		return floatclamp(a, hi, lo)
	return floatclamp(a, lo, hi)
}

stock bool:lsg_valid(e)
{
	if (e >= 1 && e <= MaxClients)
		return bool:is_user_connected(e)
	return e > MaxClients && pev_valid(e) > 0
}

stock bool:lsg_is_creature(e)
{
	if (e <= MaxClients || !pev_valid(e))
		return false
	new c[16]
	pev(e, pev_classname, c, charsmax(c))
	return bool:equal(c, "ls_creature")
}

stock bool:lsg_tactive(h)
	return h >= LSG_TASK && h < LSG_TASK + LSG_N && task_exists(h)

stock lsg_tclear(h)
{
	if (h >= LSG_TASK && h < LSG_TASK + LSG_N)
		remove_task(h)
}


// enum Class
stock const g_lsgE0[][] = { "Scout", "Tank", "Medic" }
// struct Loadout
stock lsg_mks0(Float:f0, const f1[], f2, f3, f4)
{
	new r = lsg_rnew()
	g_lsgRec[r][0] = _:f0
	copy(g_lsgRecT[r][0], 63, f1)
	lsg_rsetvec(r, 1, f2)
	g_lsgRec[r][4] = _:f3
	g_lsgRec[r][5] = _:f4
	return r
}
stock lsg_mka231(const p0[], const p1[])
{
	new a = lsg_atmp()
	lsg_apushs(a, p0)
	lsg_apushs(a, p1)
	return a
}
// Function: Reward
stock Float:g_lsgR0_0
stock g_lsgR0_1[128]
stock bool:g_lsgFD0
stock lsg_fr0(a0, Float:a1, const a2[])
{
	new ctx[LSG_CTX]
	ctx[19] = _:a0
	ctx[20] = _:a1
	copy(g_lsgOT[1], 127, a2)
	g_lsgR0_0 = 0.0
	g_lsgR0_1[0] = 0
	g_lsgFD0 = false
	g_v18 = 0.0
	lsg_aclear(g_v19)
	lsg_n237(ctx)
}

// Function: Double
stock Float:g_lsgR1_0
stock g_lsgR1_1[128]
stock bool:g_lsgFD1
stock lsg_fr1(Float:a0)
{
	new ctx[LSG_CTX]
	ctx[21] = _:a0
	g_lsgR1_0 = 0.0
	g_lsgR1_1[0] = 0
	g_lsgFD1 = false
	lsg_n243(ctx)
}
stock Float:lsg_pf1_0(Float:a0)
{
	lsg_fr1(a0)
	return g_lsgR1_0
}
stock lsg_pf1_1(Float:a0)
{
	lsg_fr1(a0)
	new t[128]
	copy(t, 127, g_lsgR1_1)
	return t
}

// Function: Tick
stock bool:g_lsgFD2
stock lsg_fr2()
{
	new ctx[LSG_CTX]
	g_lsgFD2 = false
	lsg_n245(ctx)
}

lsd_graph_vars()
{
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
			lsd_var_number("kills", id, g_pv0[id])
	lsd_var_number("round", 0, g_v1)
	lsd_var_bool("flag", 0, g_v2)
	lsd_var_text("last", 0, g_v3)
	lsd_var_number("tscore:T", 0, g_tv4[1])
	lsd_var_number("tscore:CT", 0, g_tv4[2])
	lsd_var_text("phase", 0, g_v5)
	lsd_var_text("vpos", 0, lsg_vtext((LSG_VR + 0)))
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
			lsd_var_text("vppos", id, lsg_vtext((LSG_VR + 1 + lsg_slot(id))))
	lsd_var_text("vlist", 0, fmt("%d items", lsg_alen(g_v8)))
	lsd_var_text("vnums", 0, fmt("%d items", lsg_alen(g_v9)))
	lsd_var_text("vplayers", 0, fmt("%d items", lsg_alen(g_v10)))
	lsd_var_text("vloads", 0, fmt("%d items", lsg_alen(g_v11)))
	lsd_var_text("vmap", 0, fmt("%d keys", lsg_mlen(g_v12)))
	lsd_var_text("vtmap", 0, fmt("%d keys", lsg_mlen(g_v13)))
	lsd_var_text("vvmap", 0, fmt("%d keys", lsg_mlen(g_v14)))
	lsd_var_text("vload", 0, "Loadout")
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
			lsd_var_text("vcls", id, g_lsgE0[clamp(g_pv16[lsg_slot(id)], 0, 2)])
	lsd_var_text("vtimer", 0, g_lsgRun[lsg_tactive(g_v17) ? 1 : 0])
	lsd_var_number("lsum", 0, g_v18)
	lsd_var_text("lseen", 0, fmt("%d items", lsg_alen(g_v19)))
}

stock g_lsgSquad[33], bool:g_lsgSquadFF

stock lsg_set_squad(id, squad)
{
	g_lsgSquad[id] = squad
	// the bots' brain: a squad is one side, both ways
	if (!get_cvar_pointer("ls_mode_ver"))
		return
	if (squad > 0)
	{
		server_cmd("ls_mode role %d sq%d", id, squad)
		server_cmd("ls_mode friend @sq%d @sq%d 1", squad, squad)
	}
	else
		server_cmd("ls_mode role %d -", id)
}

stock bool:lsg_same_side(a, b)
{
	if (g_lsgSquad[a] || g_lsgSquad[b])
		return g_lsgSquad[a] == g_lsgSquad[b]
	return !get_cvar_num("mp_freeforall") && lsg_team(a) > 0 && lsg_team(a) == lsg_team(b)
}

stock lsg_squads_alive()
{
	new seen[34], n
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue
		new key = g_lsgSquad[p] ? min(g_lsgSquad[p], 32) : 0
		if (key == 0)
			n++
		else if (!seen[key])
		{
			seen[key] = 1
			n++
		}
	}
	return n
}

stock bool:g_lsgDown[33], Float:g_lsgDownAt[33], Float:g_lsgDownOrg[33][3], g_lsgRevBy[33], Float:g_lsgRevProg[33]
stock Float:g_lsgDownSecs[33], Float:g_lsgDownHold[33], g_lsgDownHp[33]

stock lsg_make_down(p, Float:secs, Float:hold, hp)
{
	if (!lsg_ok(p) || is_user_alive(p))
		return 0
	g_lsgDown[p] = true
	g_lsgDownAt[p] = get_gametime()
	pev(p, pev_origin, g_lsgDownOrg[p])
	g_lsgRevBy[p] = 0
	g_lsgRevProg[p] = 0.0
	g_lsgDownSecs[p] = secs
	g_lsgDownHold[p] = hold
	g_lsgDownHp[p] = hp
	ls_chat(0, "%s is down! Hold USE on them to bring them back.", lsg_name(p))
	lsg_send_revive(p)
	return 1
}

// the two nearest bots on the same side go and stand on the downed player (the brain's "use" goal)
stock lsg_send_revive(victim)
{
	if (!get_cvar_pointer("ls_mode_ver"))
		return
	new near[2], Float:nd[2] = { 99999.0, 99999.0 }
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == victim || !is_user_alive(p) || !is_user_bot(p) || !lsg_same_side(p, victim))
			continue
		new Float:o[3]
		pev(p, pev_origin, o)
		new Float:d = get_distance_f(o, g_lsgDownOrg[victim])
		if (d < nd[0])
		{
			nd[1] = nd[0]
			near[1] = near[0]
			nd[0] = d
			near[0] = p
		}
		else if (d < nd[1])
		{
			nd[1] = d
			near[1] = p
		}
	}
	new left = max(floatround(g_lsgDownSecs[victim] - (get_gametime() - g_lsgDownAt[victim])), 1)
	for (new i = 0; i < 2; i++)
		if (near[i])
			server_cmd("ls_mode goal %d use %.0f %.0f %.0f %d 75 %d 64", near[i], g_lsgDownOrg[victim][0], g_lsgDownOrg[victim][1], g_lsgDownOrg[victim][2], victim, left)
}

stock lsg_clear_down(p)
{
	if (g_lsgDown[p] && get_cvar_pointer("ls_mode_ver"))
		server_cmd("ls_mode goal_clear all %d", p)
	if (g_lsgRevBy[p] && lsg_ok(g_lsgRevBy[p]) && !is_user_bot(g_lsgRevBy[p]))
		lsg_bar(g_lsgRevBy[p], 0)
	g_lsgDown[p] = false
	g_lsgRevBy[p] = 0
	g_lsgRevProg[p] = 0.0
}

public lsg_revive_tick()
{
	new Float:now = get_gametime()
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!g_lsgDown[d])
			continue
		if (!is_user_connected(d) || is_user_alive(d))
		{
			lsg_clear_down(d)
			continue
		}
		if (now - g_lsgDownAt[d] > g_lsgDownSecs[d])
		{
			ls_chat(0, "%s is out.", lsg_name(d))
			lsg_clear_down(d)
			lsg_down_out_hook(d)
			continue
		}
		// someone on the same side close by: people hold USE, bots just stay there
		new who = 0
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == d || !is_user_alive(p) || !lsg_same_side(p, d))
				continue
			new Float:o[3]
			pev(p, pev_origin, o)
			if (get_distance_f(o, g_lsgDownOrg[d]) > 72.0)
				continue
			if (!is_user_bot(p) && !(pev(p, pev_button) & IN_USE))
				continue
			who = p
			break
		}
		if (who != g_lsgRevBy[d])
		{
			if (g_lsgRevBy[d] && lsg_ok(g_lsgRevBy[d]) && !is_user_bot(g_lsgRevBy[d]))
				lsg_bar(g_lsgRevBy[d], 0)
			g_lsgRevBy[d] = who
			g_lsgRevProg[d] = 0.0
			if (who && !is_user_bot(who))
				lsg_bar(who, floatround(g_lsgDownHold[d], floatround_ceil))
		}
		if (!who)
			continue
		g_lsgRevProg[d] += 0.25
		if (g_lsgRevProg[d] >= g_lsgDownHold[d])
			lsg_revive(d, who)
	}
}

stock lsg_revive(d, by)
{
	new Float:o[3]
	o = g_lsgDownOrg[d]
	new hp = g_lsgDownHp[d]
	lsg_clear_down(d)
	ExecuteHamB(Ham_CS_RoundRespawn, d)
	if (!is_user_alive(d))
		return
	// back where they fell, when a player fits there
	engfunc(EngFunc_TraceHull, o, o, DONT_IGNORE_MONSTERS, HULL_HUMAN, d, 0)
	if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
		engfunc(EngFunc_SetOrigin, d, o)
	set_user_health(d, hp)
	ls_chat(0, "%s brought %s back.", lsg_name(by), lsg_name(d))
	lsg_revived_hook(d, by)
}

stock bool:g_lsgBuyOff, g_lsgBuyAllow

stock bool:lsg_buy_ok(item)
{
	new bool:armor = item == CSI_VEST || item == CSI_VESTHELM
	new bit = item >= 0 && item < 31 ? (1 << item) : 0
	switch (g_lsgBuyAllow)
	{
		case 1: return armor || item >= CSI_DEFUSER || bool:(bit & (CSI_ALL_PISTOLS | CSI_ALL_GRENADES))
		case 2: return !(bit & CSI_ALL_SNIPERRIFLES)
		case 3: return item != CSI_AWP
		case 4: return !(bit & CSI_ALL_GRENADES)
		case 5: return item != CSI_M249
		case 6: return armor
		case 7: return false
	}
	return true
}

public CS_OnBuyAttempt(id, item)
{
	if (g_lsgBuyOff || !lsg_buy_ok(item))
	{
		client_print(id, print_center, "You can't buy that in this mode.")
		return PLUGIN_HANDLED
	}
	return PLUGIN_CONTINUE
}

stock Trie:g_lsgSaved = Invalid_Trie, bool:g_lsgSavedDirty

stock lsg_saved_path(out[], len, const ext[])
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(out, len, "%s/studio/%s.saved%s", dir, LSG_MODE, ext)
}

stock lsg_saved_load()
{
	g_lsgSaved = TrieCreate()
	new path[192], line[160], key[128], val[24]
	lsg_saved_path(path, charsmax(path), "")
	// (a crash between writing the new file and renaming it: the new one is whole)
	if (!file_exists(path))
		lsg_saved_path(path, charsmax(path), ".new")
	new f = fopen(path, "rt")
	if (!f)
		return
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		new tab = contain(line, "^t")
		if (tab <= 0)
			continue
		line[tab] = 0
		copy(key, charsmax(key), line)
		copy(val, charsmax(val), line[tab + 1])
		TrieSetCell(g_lsgSaved, key, str_to_float(val))
	}
	fclose(f)
}

public lsg_saved_flush()
{
	if (!g_lsgSavedDirty || g_lsgSaved == Invalid_Trie)
		return
	new path[192], tmp[200], dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_saved_path(path, charsmax(path), "")
	lsg_saved_path(tmp, charsmax(tmp), ".new")
	new f = fopen(tmp, "wt")
	if (!f)
		return
	new Snapshot:snap = TrieSnapshotCreate(g_lsgSaved), key[128], Float:v
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, key, charsmax(key))
		TrieGetCell(g_lsgSaved, key, v)
		fprintf(f, "%s^t%f^n", key, v)
	}
	TrieSnapshotDestroy(snap)
	fclose(f)
	// Windows won't rename over a file: the old one goes first (the .new one is read if a crash lands here)
	if (!rename_file(tmp, path, 1))
	{
		delete_file(path)
		rename_file(tmp, path, 1)
	}
	g_lsgSavedDirty = false
}

stock lsg_saved_key(id, const key[], out[], len)
{
	new who[44]
	get_user_authid(id, who, charsmax(who))
	// bots, LAN and pending ids are the same for everyone: their name instead
	if (is_user_bot(id) || !who[0] || containi(who, "LAN") >= 0 || containi(who, "PENDING") >= 0 || containi(who, "BOT") >= 0 || equali(who, "VALVE_ID_LOOPBACK"))
	{
		get_user_name(id, who, charsmax(who))
		formatex(out, len, "%s@name:%s", key, who)
	}
	else
		formatex(out, len, "%s@%s", key, who)
	// (a tab or a line end in a name would break the file's lines)
	for (new i = 0; out[i]; i++)
		if (out[i] < 32)
			out[i] = '_'
}

stock Float:lsg_saved_get(id, const key[])
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return 0.0
	new k[96], Float:v
	lsg_saved_key(id, key, k, charsmax(k))
	if (!TrieGetCell(g_lsgSaved, k, v))
		return 0.0
	return v
}

stock lsg_saved_set(id, const key[], Float:value)
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return
	new k[96]
	lsg_saved_key(id, key, k, charsmax(k))
	TrieSetCell(g_lsgSaved, k, value)
	g_lsgSavedDirty = true
	// written straight away: a crash or a killed server keeps it
	lsg_saved_flush()
}

stock bool:g_lsgClockOn, bool:g_lsgClockPaused, bool:g_lsgClockInf, Float:g_lsgClockEnd, Float:g_lsgClockLeft

stock Float:lsg_clock_left()
{
	if (!g_lsgClockOn)
		return 0.0
	if (g_lsgClockPaused)
		return g_lsgClockLeft
	return floatmax(g_lsgClockEnd - get_gametime(), 0.0)
}

stock lsg_clock_set(Float:secs)
{
	// (rounds mustn't end on the game's own clock while ours runs: its "f" flag holds them)
	if (!g_lsgClockInf && !get_cvar_num("mp_round_infinite"))
	{
		new cur[16]
		get_cvar_string("mp_round_infinite", cur, charsmax(cur))
		if (!cur[0] || equal(cur, "0"))
		{
			set_cvar_string("mp_round_infinite", "f")
			g_lsgClockInf = true
		}
	}
	g_lsgClockOn = true
	g_lsgClockPaused = false
	g_lsgClockEnd = get_gametime() + floatmax(secs, 1.0)
	lsg_round_time(floatround(secs))
}

stock lsg_clock_add(Float:secs)
{
	if (!g_lsgClockOn)
		return
	if (g_lsgClockPaused)
		g_lsgClockLeft = floatmax(g_lsgClockLeft + secs, 0.0)
	else
		g_lsgClockEnd += secs
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_pause(pause)
{
	if (!g_lsgClockOn || bool:pause == g_lsgClockPaused)
		return
	if (pause)
	{
		g_lsgClockLeft = lsg_clock_left()
		g_lsgClockPaused = true
	}
	else
	{
		g_lsgClockEnd = get_gametime() + g_lsgClockLeft
		g_lsgClockPaused = false
	}
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_off()
{
	g_lsgClockOn = false
	g_lsgClockPaused = false
	if (g_lsgClockInf)
	{
		set_cvar_string("mp_round_infinite", "0")
		g_lsgClockInf = false
	}
}

public lsg_clock_tick()
{
	if (!g_lsgClockOn)
		return
	// paused: the round timer on screen keeps counting by itself, so it's set again each second
	if (g_lsgClockPaused)
	{
		lsg_round_time(floatround(g_lsgClockLeft))
		return
	}
	if (lsg_clock_left() <= 0.0)
	{
		g_lsgClockOn = false
		lsg_time_up_hook()
	}
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
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	register_logevent("lsg_on_bomb_planted", 3, "2=Planted_The_Bomb")
	register_logevent("lsg_on_bomb_defused", 3, "2=Defused_The_Bomb")
	register_logevent("lsg_on_bomb_exploded", 6, "3=Target_Bombed")
	RegisterHam(Ham_Use, "func_button", "lsg_on_player_used", 0)
	register_touch("trigger_multiple", "player", "lsg_on_player_touched")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	register_event("TeamInfo", "lsg_on_team_changed", "a")
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_on_new_round", "a", "1=0", "2=0")
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_on_creature_killed", 1)
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_pre_damage", 0)
	set_task(0.25, "lsg_revive_tick", LSG_TASK - 1, _, _, "b")
	set_task(1.0, "lsg_clock_tick", LSG_TASK - 2, _, _, "b")
	lsg_saved_load()
	g_v8 = _:ArrayCreate(LSG_AC)
	g_v9 = _:ArrayCreate(LSG_AC)
	g_v10 = _:ArrayCreate(LSG_AC)
	g_v11 = _:ArrayCreate(LSG_AC)
	g_v12 = _:TrieCreate()
	g_v13 = _:TrieCreate()
	g_v14 = _:TrieCreate()
	g_v19 = _:ArrayCreate(LSG_AC)
	g_lsgLeft[218] = 1
	register_clcmd("say /shop", "lsg_chat92")
	register_clcmd("say !shop", "lsg_chat92")
	register_clcmd("say_team /shop", "lsg_chat92")
	register_clcmd("say_team !shop", "lsg_chat92")
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
	lsg_clock_off()
	for (new p = 1; p <= MaxClients; p++)
		lsg_clear_down(p)
	lsg_saved_flush()
	for (new t = 0; t < 4; t++)
		g_tv4[t] = 0.0
	lsg_aclear(g_v9)
}

public lsg_chat92(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n92(ctx)
	return PLUGIN_CONTINUE
}

public plugin_end()
{
	lsd_end()
	lsg_saved_flush()
	lsg_clock_off()
}

public lsg_pre_damage(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	if ((bits & DMG_FALL) && victim >= 1 && victim <= MaxClients && g_lsgNoFall[victim])
	{
		if (g_lsgNoFall[victim] == 2)
			g_lsgNoFall[victim] = 0
		return HAM_SUPERCEDE
	}
	if (!g_lsgSquadFF && attacker != victim && attacker >= 1 && attacker <= MaxClients && g_lsgSquad[victim] && g_lsgSquad[victim] == g_lsgSquad[attacker])
		return HAM_SUPERCEDE
	// (saved and put back: damage done inside the flow comes back here)
	new Float:was = g_lsgDmg, bool:wasBlock = g_lsgDmgBlock
	g_lsgDmg = damage
	g_lsgDmgBlock = false
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	lsg_dmg_kind(bits, g_lsgTxt[0], 63) // kind
	lsg_n131(ctx)
	new Float:now = g_lsgDmg, bool:block = g_lsgDmgBlock
	g_lsgDmg = was
	g_lsgDmgBlock = wasBlock
	if (block)
		return HAM_SUPERCEDE
	if (now != damage)
	{
		SetHamParamFloat(4, now)
		return HAM_HANDLED
	}
	return HAM_IGNORED
}

lsg_revived_hook(id, by)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = by // by
	lsg_n138(ctx)
}

lsg_down_out_hook(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n140(ctx)
}

lsg_time_up_hook()
{
	new ctx[LSG_CTX]
	lsg_n160(ctx)
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_lsgSquad[id] = 0
	lsg_clear_down(id)
	g_lsgNoFall[id] = 0
	g_pv0[id] = 0.0
	lsg_vzero((LSG_VR + 1 + lsg_slot(id)))
	g_pv16[id] = 1
	lsg_on_player_joined(id)
}

public client_disconnected(id)
{
	lsg_on_player_left(id)
	lsg_saved_flush()
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
	lsg_n154(ctx)
	lsg_n162(ctx)
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
	lsg_n136(ctx)
	lsg_n215(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n18(ctx)
	lsg_n145(ctx)
	lsg_n225(ctx)
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

public lsg_on_player_hurt(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused bits
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	lsg_n77(ctx)
	return HAM_IGNORED
}

public lsg_on_team_changed()
{
	new id = read_data(1), tn[12]
	read_data(2, tn, charsmax(tn))
	if (id < 1 || id > MaxClients)
		return
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = tn[0] == 'T' ? 1 : (tn[0] == 'C' ? 2 : 0) // team
	lsg_n80(ctx)
}

public lsg_on_map_started()
{
	new ctx[LSG_CTX]
	lsg_n122(ctx)
}

public lsg_on_new_round()
{
	new ctx[LSG_CTX]
	lsg_n82(ctx)
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
	lsg_n113(ctx)
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
	lsg_n103(ctx)
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
lsg_n0(ctx[])
{
	lsd_begin()
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n1(ctx)
}

// Set Variable (rs_set)
lsg_n1(ctx[])
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
lsg_n2(ctx[])
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

public lsg_t2(const data[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t2", LSG_TASK + 2, data, LSG_CTX)
		return
	}
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (--g_lsgLeft[2] > 0)
	{
		lsg_c2_show()
		set_task(1.0, "lsg_t2", LSG_TASK + 2, data, LSG_CTX)
		return
	}
	lsd_exit(2, "done")
	lsg_n3(ctx)
}

// Show Message (rs_msg)
lsg_n3(ctx[])
{
	lsd_begin()
	if (lsd_enter(3, ctx))
		return
	ls_announce(0, 2.0, "%s", "Go!")
	lsd_exit(3, "shown")
}

// Repeat (rs_rep)
lsg_n4(ctx[])
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

public lsg_t4(const data[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t4", LSG_TASK + 4, data, LSG_CTX)
		return
	}
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (g_lsgLeft[4] > 0 && --g_lsgLeft[4] == 0)
		remove_task(LSG_TASK + 4)
	else
		set_task(floatmax(10.0, 0.1), "lsg_t4", LSG_TASK + 4, data, LSG_CTX)
	lsd_exit(4, "each")
	lsg_n5(ctx)
}

// Show HUD Text (rs_hud)
lsg_n5(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", float(ls_count_alive(CsTeams:(0))))
	if (lsd_enter(5, ctx))
		return
	ls_hud(0, LsHud:2, 5.0, "%s %d", "Alive:", floatround(float(ls_count_alive(CsTeams:(0)))))
	lsd_exit(5, "shown")
}

// Delay (rs_delay)
lsg_n6(ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 5.0)
	if (lsd_enter(6, ctx))
		return
	set_task(floatmax(5.0, 0.1), "lsg_t6", LSG_TASK + 6, ctx, LSG_CTX)
	lsd_exit(6, "waiting")
}

public lsg_t6(const data[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t6", LSG_TASK + 6, data, LSG_CTX)
		return
	}
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsd_exit(6, "fired")
	lsg_n7(ctx)
}

// Start Wave (rs_wave)
lsg_n7(ctx[])
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
lsg_n8(ctx[])
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
lsg_n9(ctx[])
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
lsg_n10(ctx[])
{
	lsd_begin()
	if (lsd_enter(10, ctx))
		return
	lsd_exit(10, "ran")
	lsg_n11(ctx)
}

// Branch (pk_br)
lsg_n11(ctx[])
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
lsg_n12(ctx[])
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
lsg_n13(ctx[])
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
lsg_n14(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_team("Team", 2)
	if (lsd_enter(14, ctx))
		return
	new ok = ((lsg_team(ctx[1]) == 2) ? 1 : 0)
	lsd_exit(14, g_lsdBool[ok])
	if (ok)
		lsg_n15(ctx)
	else
		lsg_n17(ctx)
}

// Give Weapon (pk_give)
lsg_n15(ctx[])
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
lsg_n16(ctx[])
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
lsg_n17(ctx[])
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
lsg_n18(ctx[])
{
	lsd_begin()
	if (lsd_enter(18, ctx))
		return
	lsd_exit(18, "ran")
	lsg_n19(ctx)
}

// Strip Weapons (ps_strip)
lsg_n19(ctx[])
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
lsg_n20(ctx[])
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
lsg_n21(ctx[])
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
lsg_n22(ctx[])
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
lsg_n23(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(23, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(23, "Not moved: that player isn't alive.")
	else if (!lsg_teleport(ctx[0], "spawn_a"))
		lsd_error(23, "Not moved: no such entity on this map.")
	else
		lsd_exit(23, "teleported")
	lsg_n24(ctx)
}

// Set Variable (ps_set)
lsg_n24(ctx[])
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
lsg_n25(ctx[])
{
	lsd_begin()
	if (lsd_enter(25, ctx))
		return
	lsd_exit(25, "ran")
	lsg_n26(ctx)
}

// Show HUD Text (pj_hud)
lsg_n26(ctx[])
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
lsg_n27(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (ls_alive(ctx[0]) ? 1 : 0))
	if (lsd_enter(27, ctx))
		return
	new ok = (((((ls_alive(ctx[0]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(27, g_lsdBool[ok])
}

// Player Left (pl)
lsg_n28(ctx[])
{
	lsd_begin()
	if (lsd_enter(28, ctx))
		return
	lsd_exit(28, "ran")
	lsg_n29(ctx)
}

// Set Variable (pl_set)
lsg_n29(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", "someone left")
	if (lsd_enter(29, ctx))
		return
	copy(g_v3, 63, "someone left")
	lsd_exit(29, "set")
}

// Bomb Planted (bp)
lsg_n30(ctx[])
{
	lsd_begin()
	if (lsd_enter(30, ctx))
		return
	lsd_exit(30, "ran")
	lsg_n31(ctx)
}

// Show HUD Text (bp_hud)
lsg_n31(ctx[])
{
	lsd_begin()
	lsd_in_team("Value", lsg_team(ctx[0]))
	if (lsd_enter(31, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Planted by", g_lsgTeamName[clamp(lsg_team(ctx[0]), 0, 3)])
	lsd_exit(31, "shown")
}

// Bomb Defused (bd)
lsg_n32(ctx[])
{
	lsd_begin()
	if (lsd_enter(32, ctx))
		return
	lsd_exit(32, "ran")
	lsg_n33(ctx)
}

// End Round (bd_end)
lsg_n33(ctx[])
{
	lsd_begin()
	lsd_in_team("Winner", 2)
	if (lsd_enter(33, ctx))
		return
	server_cmd("endround CT")
	lsd_exit(33, "Counter-Terrorists win")
}

// Bomb Exploded (bx)
lsg_n34(ctx[])
{
	lsd_begin()
	if (lsd_enter(34, ctx))
		return
	lsd_exit(34, "ran")
	lsg_n35(ctx)
}

// End Round (bx_end)
lsg_n35(ctx[])
{
	lsd_begin()
	lsd_in_team("Winner", 1)
	if (lsd_enter(35, ctx))
		return
	server_cmd("endround T")
	lsd_exit(35, "Terrorists win")
}

// Round Ended (re)
lsg_n36(ctx[])
{
	lsd_begin()
	if (lsd_enter(36, ctx))
		return
	lsd_exit(36, "ran")
	lsg_n37(ctx)
}

// Branch (re_br)
lsg_n37(ctx[])
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
lsg_n38(ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", 1)
	if (lsd_enter(38, ctx))
		return
	g_v2 = 1
	lsd_exit(38, g_lsdBool[g_v2 ? 1 : 0])
}

// Stop Timers (re_stop)
lsg_n39(ctx[])
{
	lsd_begin()
	if (lsd_enter(39, ctx))
		return
	lsg_stop_timers()
	lsd_exit(39, "stopped")
}

// Player Used (pu)
lsg_n40(ctx[])
{
	lsd_begin()
	if (lsd_enter(40, ctx))
		return
	lsd_exit(40, "ran")
	lsg_n41(ctx)
}

// Door (pu_door)
lsg_n41(ctx[])
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
lsg_n42(ctx[])
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
lsg_n43(ctx[])
{
	lsd_begin()
	if (lsd_enter(43, ctx))
		return
	lsd_exit(43, "ran")
	lsg_n44(ctx)
}

// Open Crate (pt_crate)
lsg_n44(ctx[])
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
lsg_n45(ctx[])
{
	lsd_begin()
	if (lsd_enter(45, ctx))
		return
	lsd_exit(45, "ran")
	lsg_n46(ctx)
}

// Lock Point (cc_lock)
lsg_n46(ctx[])
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
lsg_n47(ctx[])
{
	lsd_begin()
	lsd_in_team("Value", ctx[1])
	if (lsd_enter(47, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %s", "Taken by", g_lsgTeamName[clamp(ctx[1], 0, 3)])
	lsd_exit(47, "shown")
}

// Point Lost (cl)
lsg_n48(ctx[])
{
	lsd_begin()
	if (lsd_enter(48, ctx))
		return
	lsd_exit(48, "ran")
	lsg_n49(ctx)
}

// Open Extraction (cl_ex)
lsg_n49(ctx[])
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
lsg_n50(ctx[])
{
	lsd_begin()
	if (lsd_enter(50, ctx))
		return
	lsd_exit(50, "ran")
	lsg_n51(ctx)
}

// Shrink Zone (zs_shrink)
lsg_n51(ctx[])
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
lsg_n52(ctx[])
{
	lsd_begin()
	if (lsd_enter(52, ctx))
		return
	lsd_exit(52, "ran")
	lsg_n53(ctx)
}

// Set Zone (zt_set)
lsg_n53(ctx[])
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
lsg_n54(ctx[])
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
lsg_n55(ctx[])
{
	lsd_begin()
	if (lsd_enter(55, ctx))
		return
	lsd_exit(55, "ran")
	lsg_n56(ctx)
}

// Add To Variable (ex_add)
lsg_n56(ctx[])
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
lsg_n57(ctx[])
{
	lsd_begin()
	if (lsd_enter(57, ctx))
		return
	lsd_exit(57, "ran")
	lsg_n58(ctx)
}

// Show HUD Text (ws_hud)
lsg_n58(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", Float:ctx[0])
	if (lsd_enter(58, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %d", "Wave", floatround(Float:ctx[0]))
	lsd_exit(58, "shown")
}

// Wave Spawned One (wp)
lsg_n59(ctx[])
{
	lsd_begin()
	if (lsd_enter(59, ctx))
		return
	lsd_exit(59, "ran")
	lsg_n60(ctx)
}

// Show HUD Text (wp_hud)
lsg_n60(ctx[])
{
	lsd_begin()
	lsd_in_entity("Value", ctx[0])
	if (lsd_enter(60, ctx))
		return
	ls_hud(0, LsHud:2, 4.0, "%s %d", "In:", ctx[0])
	lsd_exit(60, "shown")
}

// Wave Cleared (wc)
lsg_n61(ctx[])
{
	lsd_begin()
	if (lsd_enter(61, ctx))
		return
	lsd_exit(61, "ran")
	lsg_n62(ctx)
}

// Start Wave (wc_start)
lsg_n62(ctx[])
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
lsg_n63(ctx[])
{
	lsd_begin()
	if (lsd_enter(63, ctx))
		return
	lsd_exit(63, "ran")
	lsg_n64(ctx)
}

// Show HUD Text (lo_hud)
lsg_n64(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(64, ctx))
		return
	ls_hud(ctx[1], LsHud:1, 4.0, "%s", "Looted")
	lsd_exit(64, "shown")
}

// Checkpoint Reached (ck)
lsg_n65(ctx[])
{
	lsd_begin()
	if (lsd_enter(65, ctx))
		return
	lsd_exit(65, "ran")
	lsg_n66(ctx)
}

// Set Variable (ck_set)
lsg_n66(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", Float:ctx[2])
	if (lsd_enter(66, ctx))
		return
	g_v1 = Float:ctx[2]
	lsd_exit(66, fmt("%.2f", g_v1))
}

// Barricade Broken (bb)
lsg_n67(ctx[])
{
	lsd_begin()
	if (lsd_enter(67, ctx))
		return
	lsd_exit(67, "ran")
	lsg_n68(ctx)
}

// Show Message (bb_msg)
lsg_n68(ctx[])
{
	lsd_begin()
	if (lsd_enter(68, ctx))
		return
	ls_announce(0, 3.0, "%s", "The barricade is down!")
	lsd_exit(68, "shown")
}

// Meeting Ended (me)
lsg_n69(ctx[])
{
	lsd_begin()
	if (lsd_enter(69, ctx))
		return
	lsd_exit(69, "ran")
	lsg_n70(ctx)
}

// Show HUD Text (me_hud)
lsg_n70(ctx[])
{
	lsd_begin()
	lsd_in_player("Value", ctx[1])
	if (lsd_enter(70, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Out:", lsg_name(ctx[1]))
	lsd_exit(70, "shown")
}

// Creature Killed (ck2)
lsg_n71(ctx[])
{
	lsd_begin()
	if (lsd_enter(71, ctx))
		return
	lsd_exit(71, "ran")
	lsg_n72(ctx)
}

// Set Variable (ck2_set)
lsg_n72(ctx[])
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
lsg_n73(ctx[])
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
lsg_n74(ctx[])
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
lsg_n75(ctx[])
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
lsg_n76(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_text("Value", g_v3)
	if (lsd_enter(76, ctx))
		return
	ls_hud(ctx[1], LsHud:1, 4.0, "%s %s", "Killed:", g_v3)
	lsd_exit(76, "shown")
}

// Player Hurt (hurt)
lsg_n77(ctx[])
{
	lsd_begin()
	if (lsd_enter(77, ctx))
		return
	lsd_exit(77, "ran")
	lsg_n78(ctx)
}

// Add To Variable (hurt_tadd)
lsg_n78(ctx[])
{
	lsd_begin()
	lsd_in_team("Team", lsg_team(ctx[1]))
	lsd_in_number("Amount", 1.0)
	if (lsd_enter(78, ctx))
		return
	g_tv4[clamp(lsg_team(ctx[1]), 0, 3)] += 1.0
	lsd_exit(78, fmt("%.2f", g_tv4[clamp(lsg_team(ctx[1]), 0, 3)]))
	lsg_n79(ctx)
}

// Set Glow (hurt_glow)
lsg_n79(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(79, ctx))
		return
	if (!lsg_ok(ctx[0]))
		lsd_error(79, "Nothing done: nobody there.")
	else
	{
		set_user_rendering(ctx[0], kRenderFxGlowShell, 255, 0, 0, kRenderNormal, 16)
		lsd_exit(79, "done")
	}
}

// Team Changed (tc)
lsg_n80(ctx[])
{
	lsd_begin()
	if (lsd_enter(80, ctx))
		return
	lsd_exit(80, "ran")
	lsg_n81(ctx)
}

// Set Model (tc_model)
lsg_n81(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(81, ctx))
		return
	if (!lsg_ok(ctx[0]))
		lsd_error(81, "Nothing done: nobody there.")
	else
	{
		cs_set_user_model(ctx[0], "gign")
		lsd_exit(81, "done")
	}
}

// New Round (nr)
lsg_n82(ctx[])
{
	lsd_begin()
	if (lsd_enter(82, ctx))
		return
	lsd_exit(82, "ran")
	lsg_n83(ctx)
}

// Set Setting (nr_cvar)
lsg_n83(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", 0.0)
	if (lsd_enter(83, ctx))
		return
	set_cvar_float("mp_freeforall", 0.0)
	lsd_exit(83, "set")
	lsg_n84(ctx)
}

// For Each Player (nr_each)
lsg_n84(ctx[])
{
	lsd_begin()
	if (lsd_enter(84, ctx))
		return
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 2))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[4] = p
		lsg_n85(c)
	}
	lsd_exit(84, fmt("%d times", count))
	lsg_n86(ctx)
}

// Respawn (nr_resp)
lsg_n85(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[4])
	if (lsd_enter(85, ctx))
		return
	if (!lsg_ok(ctx[4]) || is_user_alive(ctx[4]) || lsg_team(ctx[4]) < 1 || lsg_team(ctx[4]) > 2)
		lsd_error(85, "Nothing done: only a dead player on a team comes back.")
	else
	{
		ExecuteHamB(Ham_CS_RoundRespawn, ctx[4])
		lsd_exit(85, "respawned")
	}
}

// Pick Random Entity (nr_pick)
lsg_n86(ctx[])
{
	lsd_begin()
	if (lsd_enter(86, ctx))
		return
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[5] = list[random(count)]
		lsd_exit(86, fmt("picked %d of %d", ctx[5], count))
		lsg_n87(ctx)
	}
	else
	{
		lsd_exit(86, "none")
		lsg_n89(ctx)
	}
}

// For Each Entity (nr_ents)
lsg_n87(ctx[])
{
	lsd_begin()
	if (lsd_enter(87, ctx))
		return
	new c[LSG_CTX], count
	new p = -1
	while (count < 512 && (p = find_ent_by_tname(p, "door1")) > 0)
	{
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[6] = p
		lsg_n88(c)
	}
	lsd_exit(87, fmt("%d times", count))
	lsg_n90(ctx)
}

// Use Entity (nr_use)
lsg_n88(ctx[])
{
	lsd_begin()
	if (lsd_enter(88, ctx))
		return
	if (!lsg_use("door1"))
		lsd_error(88, "No entity has that name on this map.")
	else
		lsd_exit(88, "used")
	lsg_n91(ctx)
}

// Set Variable (nr_set)
lsg_n89(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", "play")
	if (lsd_enter(89, ctx))
		return
	copy(g_v5, 63, "play")
	lsd_exit(89, "set")
}

// Stop Timer (nr_stop)
lsg_n90(ctx[])
{
	lsd_begin()
	if (lsd_enter(90, ctx))
		return
	remove_task(LSG_TASK + 4)
	lsd_exit(90, "stopped")
}

// Show HUD Text (nr_hud)
lsg_n91(ctx[])
{
	lsd_begin()
	lsd_in_entity("Value", ctx[5])
	if (lsd_enter(91, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %d", "Spot:", ctx[5])
	lsd_exit(91, "shown")
}

// Chat Command (cmd)
lsg_n92(ctx[])
{
	lsd_begin()
	if (lsd_enter(92, ctx))
		return
	lsd_exit(92, "called")
	lsg_n93(ctx)
}

// Menu (cmd_menu)
lsg_n93(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(93, ctx))
		return
	if (!lsg_ok(ctx[0]) || is_user_bot(ctx[0]))
		lsd_error(93, "Not shown: nobody there, or a bot.")
	else
	{
		for (new k = 0; k < LSG_CTX; k++)
			g_lsgM93[ctx[0]][k] = ctx[k]
		new m = menu_create("Shop", "lsg_mh93")
		menu_additem(m, "Armor")
		menu_additem(m, "Money")
		menu_additem(m, "Gravity")
		menu_display(ctx[0], m)
		lsd_exit(93, "shown")
	}
	lsg_n97(ctx)
}

// Menu (cmd_menu): the choice
public lsg_mh93(id, menu, item)
{
	menu_destroy(menu)
	if (item < 0 || id < 1 || id > MaxClients)
		return PLUGIN_HANDLED
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = g_lsgM93[id][k]
	switch (item)
	{
		case 0:
		{
			lsd_exit(93, "choice 1")
			lsg_n94(ctx)
		}
		case 1:
		{
			lsd_exit(93, "choice 2")
			lsg_n95(ctx)
		}
		case 2:
		{
			lsd_exit(93, "choice 4")
			lsg_n96(ctx)
		}
	}
	return PLUGIN_HANDLED
}

// Give Item (cmd_arm)
lsg_n94(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(94, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(94, "Not given: that player isn't alive.")
	else
	{
		cs_set_user_armor(ctx[0], 100, CS_ARMOR_VESTHELM)
		lsd_exit(94, "done")
	}
}

// Give Money (cmd_money)
lsg_n95(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Amount", -500.0)
	if (lsd_enter(95, ctx))
		return
	if (!lsg_ok(ctx[0]))
		lsd_error(95, "Nothing done: nobody there.")
	else
	{
		lsg_add_money(ctx[0], floatround(-500.0))
		lsd_exit(95, "done")
	}
}

// Set Gravity (cmd_grav)
lsg_n96(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Gravity", 0.5)
	if (lsd_enter(96, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(96, "Nothing done: that player isn't alive.")
	else
	{
		set_user_gravity(ctx[0], 0.5)
		lsd_exit(96, "done")
	}
}

// Progress Bar (cmd_bar)
lsg_n97(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(97, ctx))
		return
	lsg_bar(ctx[0], 3)
	lsd_exit(97, "shown")
	lsg_n98(ctx)
}

// Chat Message (cmd_chat)
lsg_n98(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Value", lsg_get_money(ctx[0]))
	if (lsd_enter(98, ctx))
		return
	ls_chat(ctx[0], "%s %d", "Money:", floatround(lsg_get_money(ctx[0])))
	lsd_exit(98, "said")
}

// Function (fn)
lsg_n99(ctx[])
{
	lsd_begin()
	if (lsd_enter(99, ctx))
		return
	lsd_exit(99, "called")
	lsg_n100(ctx)
}

// Add Score (fn_frag)
lsg_n100(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[7])
	lsd_in_number("Amount", Float:ctx[8])
	if (lsd_enter(100, ctx))
		return
	if (!lsg_ok(ctx[7]))
		lsd_error(100, "Nothing done: nobody there.")
	else
	{
		lsg_add_frags(ctx[7], floatround(Float:ctx[8]))
		lsd_exit(100, "done")
	}
	lsg_n101(ctx)
}

// Play Sound (fn_snd)
lsg_n101(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[7])
	if (lsd_enter(101, ctx))
		return
	client_cmd(ctx[7], "spk %s", "buttons/bell1")
	lsd_exit(101, "played")
	lsg_n102(ctx)
}

// Set Role (fn_role)
lsg_n102(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[7])
	if (lsd_enter(102, ctx))
		return
	if (!lsg_ok(ctx[7]))
		lsd_error(102, "Nothing done: nobody there.")
	else
	{
		ls_set_role(ctx[7], "hero")
		lsd_exit(102, "done")
	}
}

// Player Extracted (pk2)
lsg_n103(ctx[])
{
	lsd_begin()
	if (lsd_enter(103, ctx))
		return
	lsd_exit(103, "ran")
	lsg_n104(ctx)
}

// Call Function (pk2_call)
lsg_n104(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_number("Number", 3.0)
	if (lsd_enter(104, ctx))
		return
	new c[LSG_CTX]
	c[7] = ctx[1]
	c[8] = _:(3.0)
	lsg_n99(c)
	lsd_exit(104, "called")
	lsg_n105(ctx)
}

// Pick Random Player (pk2_pick)
lsg_n105(ctx[])
{
	lsd_begin()
	if (lsd_enter(105, ctx))
		return
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 5))
			list[count++] = p
	if (count > 0)
	{
		ctx[9] = list[random(count)]
		lsd_exit(105, fmt("picked %d of %d", ctx[9], count))
		lsg_n106(ctx)
	}
	else
	{
		lsd_exit(105, "none")
	}
}

// Bot Go To (pk2_bot)
lsg_n106(ctx[])
{
	lsd_begin()
	lsd_in_player("Bot", ctx[9])
	if (lsd_enter(106, ctx))
		return
	if (!ls_alive(ctx[9]) || !is_user_bot(ctx[9]))
		lsd_error(106, "Nothing done: that player isn't a living bot.")
	else
	{
		lsg_bot_goto(ctx[9], "spot1", 0)
		lsd_exit(106, "ordered")
	}
	lsg_n107(ctx)
}

// Bot Hold (pk2_hold)
lsg_n107(ctx[])
{
	lsd_begin()
	lsd_in_player("Bot", ctx[9])
	lsd_in_number("Seconds", 10.0)
	if (lsd_enter(107, ctx))
		return
	if (!ls_alive(ctx[9]) || !is_user_bot(ctx[9]))
		lsd_error(107, "Nothing done: that player isn't a living bot.")
	else
	{
		ls_bot_hold(ctx[9], max(floatround(10.0), 1))
		lsd_exit(107, "ordered")
	}
	lsg_n108(ctx)
}

// Bot Follow (pk2_follow)
lsg_n108(ctx[])
{
	lsd_begin()
	lsd_in_player("Bot", ctx[9])
	lsd_in_player("Follow", ctx[1])
	if (lsd_enter(108, ctx))
		return
	if (!ls_alive(ctx[9]) || !is_user_bot(ctx[9]))
		lsd_error(108, "Nothing done: that player isn't a living bot.")
	else
	{
		ls_bot_follow(ctx[9], ctx[1], 10)
		lsd_exit(108, "ordered")
	}
	lsg_n109(ctx)
}

// Bot Free (pk2_free)
lsg_n109(ctx[])
{
	lsd_begin()
	lsd_in_player("Bot", ctx[9])
	if (lsd_enter(109, ctx))
		return
	if (!ls_alive(ctx[9]) || !is_user_bot(ctx[9]))
		lsd_error(109, "Nothing done: that player isn't a living bot.")
	else
	{
		ls_bot_release(ctx[9])
		lsd_exit(109, "ordered")
	}
	lsg_n110(ctx)
}

// Drop Weapon (pk2_drop)
lsg_n110(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(110, ctx))
		return
	if (!ls_alive(ctx[1]))
		lsd_error(110, "Nothing done: that player isn't alive.")
	else
	{
		engclient_cmd(ctx[1], "drop")
		lsd_exit(110, "done")
	}
	lsg_n111(ctx)
}

// Freeze (pk2_fr)
lsg_n111(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	lsd_in_bool("Frozen", 1)
	if (lsd_enter(111, ctx))
		return
	if (!ls_alive(ctx[1]))
		lsd_error(111, "Nothing done: that player isn't alive.")
	else
	{
		lsg_freeze(ctx[1], 1)
		lsd_exit(111, "done")
	}
	lsg_n112(ctx)
}

// Kill (pk2_kill)
lsg_n112(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(112, ctx))
		return
	if (!ls_alive(ctx[1]))
		lsd_error(112, "Nothing done: that player isn't alive.")
	else
	{
		user_kill(ctx[1], 1)
		lsd_exit(112, "done")
	}
}

// Point Lost (lo2)
lsg_n113(ctx[])
{
	lsd_begin()
	if (lsd_enter(113, ctx))
		return
	lsd_exit(113, "ran")
	lsg_n115(ctx)
}

// Branch (lo2_br)
lsg_n114(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (equali(g_v5, "play") ? 1 : 0))
	if (lsd_enter(114, ctx))
		return
	new ok = (((((equali(g_v5, "play") ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(114, g_lsdBool[ok])
	if (ok)
		lsg_n116(ctx)
}

// Pick Random Player (lo2_pick)
lsg_n115(ctx[])
{
	lsd_begin()
	if (lsd_enter(115, ctx))
		return
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[10] = list[random(count)]
		lsd_exit(115, fmt("picked %d of %d", ctx[10], count))
		lsg_n114(ctx)
	}
	else
	{
		lsd_exit(115, "none")
	}
}

// Branch (lo2_br2)
lsg_n116(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (((ls_components_running() && ls_zone_inside(lsg_find("zone1"), ctx[10]) ? 1 : 0)) || ((ls_is_role(ctx[10], "hero") ? 1 : 0)) ? 1 : 0))
	if (lsd_enter(116, ctx))
		return
	new ok = (((((((ls_components_running() && ls_zone_inside(lsg_find("zone1"), ctx[10]) ? 1 : 0)) || ((ls_is_role(ctx[10], "hero") ? 1 : 0)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(116, g_lsdBool[ok])
	if (ok)
		lsg_n117(ctx)
	else
		lsg_n118(ctx)
}

// Respawn At Point (lo2_resp)
lsg_n117(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[10])
	if (lsd_enter(117, ctx))
		return
	if (!ls_components_running() || !lsg_ok(ctx[10]))
		lsd_error(117, "gm_components isn't running, or nobody there.")
	else
	{
		ls_respawn_at(ctx[10], 0)
		lsd_exit(117, "done")
	}
}

// Call Supply Drop (lo2_drop)
lsg_n118(ctx[])
{
	lsd_begin()
	if (lsd_enter(118, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(118, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("drop1", o))
		lsd_error(118, "No entity has that name on this map.")
	else
	{
		ls_drop_call(o, "")
		lsd_exit(118, "called")
	}
	lsg_n119(ctx)
}

// Show HUD Text (lo2_hud)
lsg_n119(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", ((float(ls_count_role("hero"))) + (float(get_cvar_num("ls_creature_alive")))))
	if (lsd_enter(119, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %d", "Heroes and creatures:", floatround(((float(ls_count_role("hero"))) + (float(get_cvar_num("ls_creature_alive"))))))
	lsd_exit(119, "shown")
	lsg_n120(ctx)
}

// Branch (lo2_br3)
lsg_n120(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ((lsg_get_frags(ctx[10])) > (lsg_get_deaths(ctx[10])) ? 1 : 0))
	if (lsd_enter(120, ctx))
		return
	new ok = ((((((lsg_get_frags(ctx[10])) > (lsg_get_deaths(ctx[10])) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(120, g_lsdBool[ok])
	if (ok)
		lsg_n121(ctx)
}

// End Round (lo2_end)
lsg_n121(ctx[])
{
	lsd_begin()
	lsd_in_team("Winner", ctx[1])
	if (lsd_enter(121, ctx))
		return
	lsg_end_round(ctx[1])
	lsd_exit(121, "ended")
}

// Map Started (ms)
lsg_n122(ctx[])
{
	lsd_begin()
	if (lsd_enter(122, ctx))
		return
	lsd_exit(122, "ran")
	lsg_n123(ctx)
}

// Load Map Pack (ms_pack)
lsg_n123(ctx[])
{
	lsd_begin()
	if (lsd_enter(123, ctx))
		return
	if (!ls_components_running())
		lsd_error(123, "gm_components isn't running on this server.")
	else
	{
		new map[32]
		get_mapname(map, charsmax(map))
		if (!ls_comp_load_pack(fmt("gamemodes/components/%s.%s.ini", map, "br")))
			lsd_error(123, "This map has no such pack.")
		else
			lsd_exit(123, "loaded")
	}
	lsg_n124(ctx)
}

// Pick Random Entity (ms_pick)
lsg_n124(ctx[])
{
	lsd_begin()
	if (lsd_enter(124, ctx))
		return
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "ls_zone_shrinker")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[11] = list[random(count)]
		lsd_exit(124, fmt("picked %d of %d", ctx[11], count))
		lsg_n125(ctx)
	}
	else
	{
		lsd_exit(124, "none")
	}
}

// Start Component (ms_start)
lsg_n125(ctx[])
{
	lsd_begin()
	lsd_in_entity("Component", ctx[11])
	if (lsd_enter(125, ctx))
		return
	if (!ls_components_running())
		lsd_error(125, "gm_components isn't running on this server.")
	else
	{
		ls_comp_start(ctx[11])
		lsd_exit(125, "done")
	}
	lsg_n126(ctx)
}

// Stop Component (ms_stop)
lsg_n126(ctx[])
{
	lsd_begin()
	lsd_in_entity("Component", ctx[11])
	if (lsd_enter(126, ctx))
		return
	if (!ls_components_running())
		lsd_error(126, "gm_components isn't running on this server.")
	else
	{
		ls_comp_stop(ctx[11])
		lsd_exit(126, "done")
	}
	lsg_n127(ctx)
}

// Create Component (ms_make)
lsg_n127(ctx[])
{
	lsd_begin()
	lsd_in_entity("At", ctx[11])
	if (lsd_enter(127, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(127, "gm_components isn't running on this server.")
	else if (!lsg_ent_origin(ctx[11], o))
		lsd_error(127, "No entity has that name on this map.")
	else
	{
		g_lsgOut[127] = ls_comp_create("ls_loot_container", o, "table=default")
		lsd_exit(127, fmt("entity %d", g_lsgOut[127]))
	}
	lsg_n128(ctx)
}

// Call Supply Drop (ms_drop)
lsg_n128(ctx[])
{
	lsd_begin()
	lsd_in_entity("At", ctx[11])
	if (lsd_enter(128, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(128, "gm_components isn't running on this server.")
	else if (!lsg_ent_origin(ctx[11], o))
		lsd_error(128, "No entity has that name on this map.")
	else
	{
		ls_drop_call(o, "")
		lsd_exit(128, "called")
	}
	lsg_n129(ctx)
}

// For Each Player (ms_each)
lsg_n129(ctx[])
{
	lsd_begin()
	if (lsd_enter(129, ctx))
		return
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 1))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[12] = p
		lsg_n130(c)
	}
	lsd_exit(129, fmt("%d times", count))
}

// Teleport (ms_tp)
lsg_n130(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[12])
	lsd_in_entity("To", ctx[11])
	if (lsd_enter(130, ctx))
		return
	if (!ls_alive(ctx[12]))
		lsd_error(130, "Not moved: that player isn't alive.")
	else if (!lsg_teleport_to(ctx[12], ctx[11]))
		lsd_error(130, "Not moved: no such entity on this map.")
	else
		lsd_exit(130, "teleported")
}

// Player About To Be Hurt (ah)
lsg_n131(ctx[])
{
	lsd_begin()
	if (lsd_enter(131, ctx))
		return
	lsd_exit(131, "called")
	lsg_n132(ctx)
}

// Branch (ah_br)
lsg_n132(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (equali(g_lsgTxt[0], "fall") ? 1 : 0))
	if (lsd_enter(132, ctx))
		return
	new ok = (((((equali(g_lsgTxt[0], "fall") ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(132, g_lsdBool[ok])
	if (ok)
		lsg_n133(ctx)
	else
		lsg_n134(ctx)
}

// Block Damage (ah_block)
lsg_n133(ctx[])
{
	lsd_begin()
	if (lsd_enter(133, ctx))
		return
	g_lsgDmgBlock = true
	if (g_lsgDmgBlock)
		lsd_exit(133, "blocked")
	else
		lsd_exit(133, fmt("%.1f", g_lsgDmg))
}

// Scale Damage (ah_scale)
lsg_n134(ctx[])
{
	lsd_begin()
	lsd_in_number("Times", 0.5)
	if (lsd_enter(134, ctx))
		return
	g_lsgDmg = g_lsgDmg * floatmax(0.5, 0.0)
	if (g_lsgDmgBlock)
		lsd_exit(134, "blocked")
	else
		lsd_exit(134, fmt("%.1f", g_lsgDmg))
	lsg_n135(ctx)
}

// Set Damage (ah_set)
lsg_n135(ctx[])
{
	lsd_begin()
	lsd_in_number("Damage", 5.0)
	if (lsd_enter(135, ctx))
		return
	g_lsgDmg = floatmax(5.0, 0.0)
	if (g_lsgDmgBlock)
		lsd_exit(135, "blocked")
	else
		lsd_exit(135, fmt("%.1f", g_lsgDmg))
}

// Player Killed (dk)
lsg_n136(ctx[])
{
	lsd_begin()
	if (lsd_enter(136, ctx))
		return
	lsd_exit(136, "ran")
	lsg_n137(ctx)
}

// Make Downed (dk_down)
lsg_n137(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(137, ctx))
		return
	if (!lsg_make_down(ctx[0], 20.0, 2.0, 40))
		lsd_error(137, "Not downed: that player isn't dead (wire it from Player Killed).")
	else
		lsd_exit(137, "downed")
}

// Player Revived (rv)
lsg_n138(ctx[])
{
	lsd_begin()
	if (lsd_enter(138, ctx))
		return
	lsd_exit(138, "called")
	lsg_n139(ctx)
}

// Show HUD Text (rv_hud)
lsg_n139(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_player("Value", ctx[1])
	if (lsd_enter(139, ctx))
		return
	ls_hud(ctx[0], LsHud:3, 4.0, "%s %s", "Saved by", lsg_name(ctx[1]))
	lsd_exit(139, "shown")
}

// Downed Player Out (to)
lsg_n140(ctx[])
{
	lsd_begin()
	if (lsd_enter(140, ctx))
		return
	lsd_exit(140, "called")
	lsg_n141(ctx)
}

// Add To Saved (to_add)
lsg_n141(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Amount", 1.0)
	if (lsd_enter(141, ctx))
		return
	if (!lsg_ok(ctx[0]) || g_lsgSaved == Invalid_Trie)
		lsd_error(141, "Not saved: nobody there, or the store didn't open.")
	else
	{
		lsg_saved_set(ctx[0], "outs", lsg_saved_get(ctx[0], "outs") + (1.0))
		lsd_exit(141, fmt("%.1f", lsg_saved_get(ctx[0], "outs")))
	}
	lsg_n142(ctx)
}

// Save Value (to_set)
lsg_n142(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Value", 1.0)
	if (lsd_enter(142, ctx))
		return
	if (!lsg_ok(ctx[0]) || g_lsgSaved == Invalid_Trie)
		lsd_error(142, "Not saved: nobody there, or the store didn't open.")
	else
	{
		lsg_saved_set(ctx[0], "last_out", 1.0)
		lsd_exit(142, fmt("%.1f", lsg_saved_get(ctx[0], "last_out")))
	}
	lsg_n143(ctx)
}

// Show HUD Text (to_hud)
lsg_n143(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Value", lsg_saved_get(ctx[0], "outs"))
	if (lsd_enter(143, ctx))
		return
	ls_hud(ctx[0], LsHud:1, 4.0, "%s %d", "Times out:", floatround(lsg_saved_get(ctx[0], "outs")))
	lsd_exit(143, "shown")
	lsg_n144(ctx)
}

// Branch (to_br)
lsg_n144(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (g_lsgDown[lsg_slot(ctx[0])] ? 1 : 0))
	if (lsd_enter(144, ctx))
		return
	new ok = (((((g_lsgDown[lsg_slot(ctx[0])] ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(144, g_lsdBool[ok])
}

// Player Spawned (sq)
lsg_n145(ctx[])
{
	lsd_begin()
	if (lsd_enter(145, ctx))
		return
	lsd_exit(145, "ran")
	lsg_n146(ctx)
}

// No Fall Damage (sq_nf)
lsg_n146(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	if (lsd_enter(146, ctx))
		return
	g_lsgNoFall[lsg_slot(ctx[0])] = 2
	lsd_exit(146, "set")
	lsg_n147(ctx)
}

// Set Squad (sq_set)
lsg_n147(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Squad", 2.0)
	if (lsd_enter(147, ctx))
		return
	if (!lsg_ok(ctx[0]))
		lsd_error(147, "Nothing done: nobody there.")
	else
	{
		lsg_set_squad(ctx[0], clamp(floatround(2.0), 0, 32))
		lsd_exit(147, "set")
	}
	lsg_n148(ctx)
}

// Squad Friendly Fire (sq_ff)
lsg_n148(ctx[])
{
	lsd_begin()
	lsd_in_bool("On", 0)
	if (lsd_enter(148, ctx))
		return
	g_lsgSquadFF = bool:0
	lsd_exit(148, "set")
	lsg_n149(ctx)
}

// Show HUD Text (sq_hud)
lsg_n149(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", ((float(g_lsgSquad[lsg_slot(ctx[0])])) + (float(lsg_squads_alive()))))
	if (lsd_enter(149, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %d", "Squad:", floatround(((float(g_lsgSquad[lsg_slot(ctx[0])])) + (float(lsg_squads_alive())))))
	lsd_exit(149, "shown")
	lsg_n150(ctx)
}

// Buying (sq_buy)
lsg_n150(ctx[])
{
	lsd_begin()
	if (lsd_enter(150, ctx))
		return
	g_lsgBuyOff = false
	set_cvar_num("mp_buy_anywhere", 1)
	lsd_exit(150, "set")
	lsg_n151(ctx)
}

// Allow Buying (sq_allow)
lsg_n151(ctx[])
{
	lsd_begin()
	if (lsd_enter(151, ctx))
		return
	g_lsgBuyAllow = 1
	lsd_exit(151, "set")
	lsg_n153(ctx)
}

// Launch Up (sq_up)
lsg_n152(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_number("Speed", 600.0)
	if (lsd_enter(152, ctx))
		return
	if (!ls_alive(ctx[0]))
		lsd_error(152, "Nothing done: that player isn't alive.")
	else
	{
		lsg_launch(ctx[0], 600.0)
		lsd_exit(152, "done")
	}
}

// Bot Go To (sq_goto)
lsg_n153(ctx[])
{
	lsd_begin()
	lsd_in_player("Bot", ctx[0])
	if (lsd_enter(153, ctx))
		return
	if (!ls_alive(ctx[0]) || !is_user_bot(ctx[0]))
		lsd_error(153, "Nothing done: that player isn't a living bot.")
	else
	{
		lsg_bot_goto(ctx[0], "spot1", 15)
		lsd_exit(153, "ordered")
	}
	lsg_n152(ctx)
}

// Round Started (ck3)
lsg_n154(ctx[])
{
	lsd_begin()
	if (lsd_enter(154, ctx))
		return
	lsd_exit(154, "ran")
	lsg_n155(ctx)
}

// Set Round Clock (ck3_set)
lsg_n155(ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 180.0)
	if (lsd_enter(155, ctx))
		return
	lsg_clock_set(180.0)
	lsd_exit(155, fmt("%.0f s left", lsg_clock_left()))
	lsg_n156(ctx)
}

// Add Round Time (ck3_add)
lsg_n156(ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 30.0)
	if (lsd_enter(156, ctx))
		return
	lsg_clock_add(30.0)
	lsd_exit(156, fmt("%.0f s left", lsg_clock_left()))
	lsg_n157(ctx)
}

// Pause Round Clock (ck3_p)
lsg_n157(ctx[])
{
	lsd_begin()
	if (lsd_enter(157, ctx))
		return
	lsg_clock_pause(1)
	lsd_exit(157, fmt("%.0f s left", lsg_clock_left()))
	lsg_n158(ctx)
}

// Pause Round Clock (ck3_r)
lsg_n158(ctx[])
{
	lsd_begin()
	if (lsd_enter(158, ctx))
		return
	lsg_clock_pause(0)
	lsd_exit(158, fmt("%.0f s left", lsg_clock_left()))
	lsg_n159(ctx)
}

// Show HUD Text (ck3_hud)
lsg_n159(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", lsg_clock_left())
	if (lsd_enter(159, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %d", "Time:", floatround(lsg_clock_left()))
	lsd_exit(159, "shown")
}

// Round Time Up (tu)
lsg_n160(ctx[])
{
	lsd_begin()
	if (lsd_enter(160, ctx))
		return
	lsd_exit(160, "called")
	lsg_n161(ctx)
}

// End Round (tu_end)
lsg_n161(ctx[])
{
	lsd_begin()
	lsd_in_team("Winner", 2)
	if (lsd_enter(161, ctx))
		return
	server_cmd("endround CT")
	lsd_exit(161, "Counter-Terrorists win")
}

// Round Started (a0)
lsg_n162(ctx[])
{
	lsd_begin()
	if (lsd_enter(162, ctx))
		return
	lsd_exit(162, "ran")
	lsg_n163(ctx)
}

// Sequence (a_seq)
lsg_n163(ctx[])
{
	lsd_begin()
	if (lsd_enter(163, ctx))
		return
	lsd_exit(163, "ran")
	lsg_n164(ctx)
	lsg_n177(ctx)
	lsg_n185(ctx)
	lsg_n198(ctx)
}

// For Loop (a_for)
lsg_n164(ctx[])
{
	lsd_begin()
	lsd_in_number("First", 1.0)
	lsd_in_number("Last", 5.0)
	if (lsd_enter(164, ctx))
		return
	new c[LSG_CTX], n
	new last = floatround(5.0)
	for (new k = floatround(1.0); k <= last && n < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[13] = _:float(k)
		lsg_n165(c)
	}
	lsd_exit(164, fmt("%d times", n))
	lsg_n166(ctx)
}

// Add (a_add)
lsg_n165(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	lsd_in_number("Item", Float:ctx[13])
	if (lsd_enter(165, ctx))
		return
	g_lsgO[0] = _:float(lsg_apushc(g_v9, _:(Float:ctx[13])))
	lsd_exit(165, fmt("%d items", lsg_alen(g_v9)))
}

// Sort (a_sort)
lsg_n166(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	if (lsd_enter(166, ctx))
		return
	lsg_asort(g_v9, 0, 1)
	lsd_exit(166, fmt("%d items", lsg_alen(g_v9)))
	lsg_n167(ctx)
}

// Shuffle (a_shuf)
lsg_n167(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	if (lsd_enter(167, ctx))
		return
	lsg_ashuffle(g_v9)
	lsd_exit(167, fmt("%d items", lsg_alen(g_v9)))
	lsg_n168(ctx)
}

// Insert (a_ins)
lsg_n168(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	lsd_in_number("Item", 99.0)
	lsd_in_number("Index", 0.0)
	if (lsd_enter(168, ctx))
		return
	lsg_ainsc(g_v9, 0.0, _:(99.0))
	lsd_exit(168, fmt("%d items", lsg_alen(g_v9)))
	lsg_n169(ctx)
}

// Remove Index (a_rmi)
lsg_n169(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	lsd_in_number("Index", 1.0)
	if (lsd_enter(169, ctx))
		return
	lsg_aremi(g_v9, 1.0)
	lsd_exit(169, fmt("%d items", lsg_alen(g_v9)))
	lsg_n170(ctx)
}

// Set Array Item (a_aset)
lsg_n170(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	lsd_in_number("Index", 0.0)
	lsd_in_number("Item", 7.0)
	if (lsd_enter(170, ctx))
		return
	lsg_asetc(g_v9, 0.0, _:(7.0))
	lsd_exit(170, fmt("%d items", lsg_alen(g_v9)))
	lsg_n171(ctx)
}

// Remove Item (a_arem)
lsg_n171(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	lsd_in_number("Item", 99.0)
	if (lsd_enter(171, ctx))
		return
	g_lsgO[1] = lsg_aremc(g_v9, _:(99.0)) ? 1 : 0
	lsd_exit(171, fmt("%d items", lsg_alen(g_v9)))
	lsg_n172(ctx)
}

// For Each (a_fe)
lsg_n172(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	if (lsd_enter(172, ctx))
		return
	new c[LSG_CTX], n
	g_lsgLeft[172] = 0
	static snap
	if (!snap)
		snap = _:ArrayCreate(LSG_AC)
	lsg_acopy(snap, g_v9)
	new size = lsg_alen(snap)
	for (new k = 0; k < size && k < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[15] = _:float(k)
		c[14] = ArrayGetCell(Array:snap, k)
		lsg_n173(c)
		if (g_lsgLeft[172])
			break
	}
	lsd_exit(172, fmt("%d times", n))
	lsg_n175(ctx)
}

stock lsg_n172_break(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[172] = 1
}

// Branch (a_br)
lsg_n173(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ((Float:ctx[14]) > (3.0) ? 1 : 0))
	if (lsd_enter(173, ctx))
		return
	new ok = ((((((Float:ctx[14]) > (3.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(173, g_lsdBool[ok])
	if (ok)
		lsg_n172_break(ctx)
	else
		lsg_n174(ctx)
}

// Print String (a_pr)
lsg_n174(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", lsg_ntext(Float:ctx[14]))
	if (lsd_enter(174, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Item", lsg_ntext(Float:ctx[14]))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(174, esc)
}

// Print String (a_pr2)
lsg_n175(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s items, has 7 %s at %s, one is %s (100%%)", lsg_ntext(float(lsg_alen(g_v9))), g_lsgYesNo[((lsg_afindc(g_v9, _:(7.0)) >= 0 ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_afindc(g_v9, _:(7.0)))), lsg_ntext(Float:lsg_arandc(g_v9))))
	if (lsd_enter(175, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Numbers:", fmt("%s items, has 7 %s at %s, one is %s (100%%)", lsg_ntext(float(lsg_alen(g_v9))), g_lsgYesNo[((lsg_afindc(g_v9, _:(7.0)) >= 0 ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_afindc(g_v9, _:(7.0)))), lsg_ntext(Float:lsg_arandc(g_v9))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(175, esc)
	lsg_n176(ctx)
}

// Clear (a_clr)
lsg_n176(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v9)))
	if (lsd_enter(176, ctx))
		return
	lsg_aclear(g_v9)
	lsd_exit(176, fmt("%d items", lsg_alen(g_v9)))
}

// Set Variable (b_set)
lsg_n177(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsg_mka231("a", lsg_tcat("b", "c")))))
	if (lsd_enter(177, ctx))
		return
	lsg_acopy(g_v8, lsg_mka231("a", lsg_tcat("b", "c")))
	lsd_exit(177, "set")
	lsg_n178(ctx)
}

// While Loop (b_wh)
lsg_n178(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ((float(lsg_alen(g_v8))) < (5.0) ? 1 : 0))
	if (lsd_enter(178, ctx))
		return
	new c[LSG_CTX], n
	while (n < 100 && ((((float(lsg_alen(g_v8))) < (5.0) ? 1 : 0)) ? 1 : 0))
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		lsg_n179(c)
	}
	lsd_exit(178, fmt("%d times", n))
	lsg_n180(ctx)
}

// Add (b_add)
lsg_n179(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v8)))
	lsd_in_text("Item", fmt("%s items", lsg_ntext(float(lsg_alen(g_v8)))))
	if (lsd_enter(179, ctx))
		return
	g_lsgO[2] = _:float(lsg_apushs(g_v8, fmt("%s items", lsg_ntext(float(lsg_alen(g_v8))))))
	lsd_exit(179, fmt("%d items", lsg_alen(g_v8)))
}

// Switch on Text (b_sw)
lsg_n180(ctx[])
{
	lsd_begin()
	lsd_in_text("Selection", lsg_agets(g_v8, 0.0))
	if (lsd_enter(180, ctx))
		return
	new x[128]
	copy(x, 127, lsg_agets(g_v8, 0.0))
	if (equali(x, "a"))
	{
		lsg_n181(ctx)
	}
	else if (equali(x, "bc"))
	{
		lsg_n182(ctx)
	}
	else
	{
		lsg_n184(ctx)
	}
}

// Print String (b_pr1)
lsg_n181(ctx[])
{
	lsd_begin()
	if (lsd_enter(181, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "First is a")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(181, esc)
}

// Switch on Number (b_sn)
lsg_n182(ctx[])
{
	lsd_begin()
	lsd_in_number("Selection", str_to_float("2"))
	if (lsd_enter(182, ctx))
		return
	new Float:x = str_to_float("2")
	if (x == 1.0)
	{
	}
	else if (x == 2.0)
	{
		lsg_n183(ctx)
	}
	else if (x == 3.0)
	{
	}
	else
	{
	}
}

// Print String (b_pr2)
lsg_n183(ctx[])
{
	lsd_begin()
	if (lsd_enter(183, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "Two")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(183, esc)
}

// Print String (b_pr3)
lsg_n184(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s long, %s", lsg_ntext(float(strlen(lsg_arands(g_v8)))), g_lsgYesNo[((containi(lsg_arands(g_v8), "item") >= 0 ? 1 : 0)) ? 1 : 0]))
	if (lsd_enter(184, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Other:", fmt("%s long, %s", lsg_ntext(float(strlen(lsg_arands(g_v8)))), g_lsgYesNo[((containi(lsg_arands(g_v8), "item") >= 0 ? 1 : 0)) ? 1 : 0]))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(184, esc)
}

// For Each Player (c_each)
lsg_n185(ctx[])
{
	lsd_begin()
	if (lsd_enter(185, ctx))
		return
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 1))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[16] = p
		lsg_n186(c)
	}
	lsd_exit(185, fmt("%d times", count))
	lsg_n190(ctx)
}

// Set Variable (c_setp)
lsg_n186(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[16])
	lsd_in_text("Value", lsg_vtext(lsg_vplayer(ctx[16], 3)))
	if (lsd_enter(186, ctx))
		return
	lsg_vset((LSG_VR + 1 + lsg_slot(ctx[16])), lsg_vplayer(ctx[16], 3))
	lsd_exit(186, "set")
	lsg_n187(ctx)
}

// Set Position (c_pos)
lsg_n187(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[16])
	lsd_in_text("Position", lsg_vtext(lsg_vadd(lsg_vplayer(ctx[16], 0), lsg_vec(0.0, 0.0, 36.0), 1.0)))
	if (lsd_enter(187, ctx))
		return
	if (!ls_alive(ctx[16]))
		lsd_error(187, "Nothing done: that player isn't alive.")
	else
	{
		lsg_vsetpos(ctx[16], lsg_vadd(lsg_vplayer(ctx[16], 0), lsg_vec(0.0, 0.0, 36.0), 1.0))
		lsd_exit(187, "done")
	}
	lsg_n188(ctx)
}

// Set Velocity (c_vel)
lsg_n188(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[16])
	lsd_in_text("Velocity", lsg_vtext(lsg_vscale(lsg_vnorm(lsg_vdir(lsg_vplayer(ctx[16], 0), lsg_vent(ctx[16]))), 300.0)))
	if (lsd_enter(188, ctx))
		return
	if (!ls_alive(ctx[16]))
		lsd_error(188, "Nothing done: that player isn't alive.")
	else
	{
		lsg_vsetvel(ctx[16], lsg_vscale(lsg_vnorm(lsg_vdir(lsg_vplayer(ctx[16], 0), lsg_vent(ctx[16]))), 300.0))
		lsd_exit(188, "done")
	}
	lsg_n189(ctx)
}

// Print String (c_pr)
lsg_n189(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s away, %s, at %s", lsg_ntext(lsg_vdist((LSG_VR + 1 + lsg_slot(ctx[16])), (LSG_VR + 0))), lsg_ntext(((lsg_vc(lsg_vplayer(ctx[16], 3), 2)) + (lsg_vlen(lsg_vadd(lsg_vplayer(ctx[16], 0), lsg_vec(0.0, 0.0, 36.0), 1.0))))), lsg_vtext(lsg_vplayer(ctx[16], 3))))
	if (lsd_enter(189, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Vectors:", fmt("%s away, %s, at %s", lsg_ntext(lsg_vdist((LSG_VR + 1 + lsg_slot(ctx[16])), (LSG_VR + 0))), lsg_ntext(((lsg_vc(lsg_vplayer(ctx[16], 3), 2)) + (lsg_vlen(lsg_vadd(lsg_vplayer(ctx[16], 0), lsg_vec(0.0, 0.0, 36.0), 1.0))))), lsg_vtext(lsg_vplayer(ctx[16], 3))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(189, esc)
}

// Set Variable (c_sl)
lsg_n190(ctx[])
{
	lsd_begin()
	if (lsd_enter(190, ctx))
		return
	lsg_rset((LSG_RR + 0), lsg_mks0(30.0, "kit", lsg_vec(0.0, 0.0, 36.0), 0, 1))
	lsd_exit(190, "set")
	lsg_n191(ctx)
}

// Set Field (c_sf)
lsg_n191(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", lsg_clampf(50.0, 0.0, 40.0))
	if (lsd_enter(191, ctx))
		return
	g_lsgRec[lsg_ri((LSG_RR + 0))][0] = _:(lsg_clampf(50.0, 0.0, 40.0))
	lsd_exit(191, "set")
	lsg_n192(ctx)
}

// Set Field (c_sf2)
lsg_n192(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext((LSG_VR + 0)))
	if (lsd_enter(192, ctx))
		return
	lsg_rsetvec((LSG_RR + 0), 1, (LSG_VR + 0))
	lsd_exit(192, "set")
	lsg_n193(ctx)
}

// Add (c_push)
lsg_n193(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v11)))
	if (lsd_enter(193, ctx))
		return
	g_lsgO[3] = _:float(lsg_apushr(g_v11, (LSG_RR + 0)))
	lsd_exit(193, fmt("%d items", lsg_alen(g_v11)))
	lsg_n194(ctx)
}

// Print String (c_pr2)
lsg_n194(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s %s %s %s", lsg_ntext(Float:g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][0]), g_lsgRecT[lsg_ri(lsg_agetr(g_v11, 0.0))][0], g_lsgE0[clamp(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5], 0, 2)], lsg_vtext(lsg_rvec(lsg_agetr(g_v11, 0.0), 1))))
	if (lsd_enter(194, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Loadout:", fmt("%s %s %s %s", lsg_ntext(Float:g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][0]), g_lsgRecT[lsg_ri(lsg_agetr(g_v11, 0.0))][0], g_lsgE0[clamp(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5], 0, 2)], lsg_vtext(lsg_rvec(lsg_agetr(g_v11, 0.0), 1))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(194, esc)
	lsg_n195(ctx)
}

// Switch on Enum (c_sw)
lsg_n195(ctx[])
{
	lsd_begin()
	lsd_in_text("Selection", g_lsgE0[clamp(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5], 0, 2)])
	if (lsd_enter(195, ctx))
		return
	switch (g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5])
	{
		case 0:
		{
			lsg_n196(ctx)
		}
		case 1:
		{
			lsg_n197(ctx)
		}
		case 2:
		{
			}
	}
}

// Print String (c_pr3)
lsg_n196(ctx[])
{
	lsd_begin()
	if (lsd_enter(196, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "Scout")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(196, esc)
}

// Print String (c_pr4)
lsg_n197(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", float(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5]))
	if (lsd_enter(197, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "Tank number", floatround(float(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5])))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(197, esc)
}

// For Each Player (d_each)
lsg_n198(ctx[])
{
	lsd_begin()
	if (lsd_enter(198, ctx))
		return
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 0))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[17] = p
		lsg_n199(c)
	}
	lsd_exit(198, fmt("%d times", count))
	lsg_n203(ctx)
}

// Map Add (d_madd)
lsg_n199(ctx[])
{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(g_v12)))
	lsd_in_player("Key", ctx[17])
	lsd_in_number("Value", ((Float:lsg_mgetc(g_v12, lsg_kp(ctx[17]))) + (1.0)))
	if (lsd_enter(199, ctx))
		return
	lsg_msetc(g_v12, lsg_kp(ctx[17]), _:(((Float:lsg_mgetc(g_v12, lsg_kp(ctx[17]))) + (1.0))))
	lsd_exit(199, fmt("%d keys", lsg_mlen(g_v12)))
	lsg_n200(ctx)
}

// Call (d_call)
lsg_n200(ctx[])
{
	lsd_begin()
	lsd_in_player("Who", ctx[17])
	lsd_in_number("Amount", 5.0)
	lsd_in_text("Why", "kill")
	if (lsd_enter(200, ctx))
		return
	lsg_fr0(ctx[17], 5.0, "kill")
	g_lsgO[4] = _:g_lsgR0_0
	copy(g_lsgOT[0], 127, g_lsgR0_1)
	lsd_exit(200, "called")
	lsg_n201(ctx)
}

// Print String (d_pr)
lsg_n201(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", g_lsgOT[0])
	if (lsd_enter(201, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Said:", g_lsgOT[0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(201, esc)
	lsg_n202(ctx)
}

// Print String (d_pr2)
lsg_n202(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", Float:g_lsgO[4])
	if (lsd_enter(202, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "Total:", floatround(Float:g_lsgO[4]))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(202, esc)
}

// Map Add (d_tadd)
lsg_n203(ctx[])
{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(g_v13)))
	lsd_in_text("Key", "k")
	lsd_in_text("Value", "k")
	if (lsd_enter(203, ctx))
		return
	lsg_msets(g_v13, "k", "k")
	lsd_exit(203, fmt("%d keys", lsg_mlen(g_v13)))
	lsg_n204(ctx)
}

// Map Add (d_vadd)
lsg_n204(ctx[])
{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(g_v14)))
	lsd_in_number("Key", 1.0)
	lsd_in_text("Value", lsg_vtext(lsg_vec(0.0, 0.0, 36.0)))
	if (lsd_enter(204, ctx))
		return
	lsg_msetv(g_v14, lsg_kn(1.0), lsg_vec(0.0, 0.0, 36.0))
	lsd_exit(204, fmt("%d keys", lsg_mlen(g_v14)))
	lsg_n205(ctx)
}

// Map Remove (d_rem)
lsg_n205(ctx[])
{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(g_v13)))
	lsd_in_text("Key", "kill")
	if (lsd_enter(205, ctx))
		return
	lsg_mdel(g_v13, "kill")
	lsd_exit(205, fmt("%d keys", lsg_mlen(g_v13)))
	lsg_n206(ctx)
}

// Print String (d_pr3)
lsg_n206(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s %s %s %s", lsg_ntext(float(lsg_mlen(g_v12))), g_lsgYesNo[((lsg_mhas(g_v13, "k") ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_alen(lsg_mkeys(g_v12, 2)))), lsg_ntext(float(lsg_alen(lsg_mvalues(g_v14, 2))))))
	if (lsd_enter(206, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Maps:", fmt("%s %s %s %s", lsg_ntext(float(lsg_mlen(g_v12))), g_lsgYesNo[((lsg_mhas(g_v13, "k") ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_alen(lsg_mkeys(g_v12, 2)))), lsg_ntext(float(lsg_alen(lsg_mvalues(g_v14, 2))))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(206, esc)
	lsg_n207(ctx)
}

// Map Clear (d_mclr)
lsg_n207(ctx[])
{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(g_v14)))
	if (lsd_enter(207, ctx))
		return
	lsg_mclear(g_v14)
	lsd_exit(207, fmt("%d keys", lsg_mlen(g_v14)))
	lsg_n208(ctx)
}

// Print String (d_pr4)
lsg_n208(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", lsg_pf1_0(4.0))
	if (lsd_enter(208, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "Double:", floatround(lsg_pf1_0(4.0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(208, esc)
	lsg_n209(ctx)
}

// Print String (d_pr5)
lsg_n209(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", lsg_pf1_1(4.0))
	if (lsd_enter(209, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "As text:", lsg_pf1_1(4.0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(209, esc)
	lsg_n210(ctx)
}

// Set Timer by Function (d_tbf)
lsg_n210(ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 2.0)
	if (lsd_enter(210, ctx))
		return
	remove_task(LSG_TASK + 210)
	set_task(floatmax(2.0, 0.1), "lsg_tf210", LSG_TASK + 210, _, _, "b")
	lsd_exit(210, "started")
	lsg_n211(ctx)
}

public lsg_tf210()
{
	if (lsd_hold_timer())
		return
	lsg_fr2()
}

// Set Variable (d_tset)
lsg_n211(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", g_lsgRun[lsg_tactive((LSG_TASK + 210)) ? 1 : 0])
	if (lsd_enter(211, ctx))
		return
	g_v17 = (LSG_TASK + 210)
	lsd_exit(211, "set")
	lsg_n212(ctx)
}

// Print String (d_pr6)
lsg_n212(ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", (lsg_tactive(g_v17) ? 1 : 0))
	if (lsd_enter(212, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Timer on:", g_lsgYesNo[((lsg_tactive(g_v17) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(212, esc)
	lsg_n213(ctx)
}

// Clear Timer (d_tclr)
lsg_n213(ctx[])
{
	lsd_begin()
	lsd_in_text("Timer", g_lsgRun[lsg_tactive(g_v17) ? 1 : 0])
	if (lsd_enter(213, ctx))
		return
	lsg_tclear(g_v17)
	lsd_exit(213, "cleared")
	lsg_n247(ctx)
}

// Print String (d_pr7)
lsg_n214(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", ((3.0) * (2.0)))
	if (lsd_enter(214, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "Macro gave:", floatround(((3.0) * (2.0))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(214, esc)
}

// Player Killed (e0)
lsg_n215(ctx[])
{
	lsd_begin()
	if (lsd_enter(215, ctx))
		return
	lsd_exit(215, "ran")
	lsg_n216(ctx)
}

// Do Once (e_once)
lsg_n216(ctx[])
{
	lsd_begin()
	if (lsd_enter(216, ctx))
		return
	if (g_lsgLeft[216])
	{
		lsd_exit(216, "closed")
		return
	}
	g_lsgLeft[216] = 1
	lsd_exit(216, "through")
	lsg_n217(ctx)
}

stock lsg_n216_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[216] = 0
}

// Do N (e_n)
lsg_n217(ctx[])
{
	lsd_begin()
	lsd_in_number("N", 3.0)
	if (lsd_enter(217, ctx))
		return
	if (g_lsgLeft[217] >= floatround(3.0))
	{
		lsd_exit(217, "done N times")
		return
	}
	g_lsgLeft[217]++
	g_lsgO[5] = _:float(g_lsgLeft[217])
	lsd_exit(217, "through")
	lsg_n218(ctx)
}

stock lsg_n217_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[217] = 0
}

// Gate (e_gate)
lsg_n218(ctx[])
{
	lsd_begin()
	if (lsd_enter(218, ctx))
		return
	if (!g_lsgLeft[218])
	{
		lsd_exit(218, "closed")
		return
	}
	lsd_exit(218, "through")
	lsg_n219(ctx)
}

stock lsg_n218_open(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[218] = 1
}

stock lsg_n218_close(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[218] = 0
}

stock lsg_n218_toggle(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[218] ^= 1
}

// Flip Flop (e_ff)
lsg_n219(ctx[])
{
	lsd_begin()
	if (lsd_enter(219, ctx))
		return
	g_lsgLeft[219] ^= 1
	g_lsgO[6] = g_lsgLeft[219]
	if (g_lsgLeft[219])
	{
		lsd_exit(219, "A")
		lsg_n220(ctx)
	}
	else
	{
		lsd_exit(219, "B")
		lsg_n224(ctx)
	}
}

// Is Valid? (e_iv)
lsg_n220(ctx[])
{
	lsd_begin()
	lsd_in_player("Object", ctx[0])
	if (lsd_enter(220, ctx))
		return
	if (lsg_valid(ctx[0]))
	{
		lsd_exit(220, "valid")
		lsg_n221(ctx)
	}
	else
	{
		lsd_exit(220, "not valid")
		lsg_n216_reset(ctx)
	}
}

// Retriggerable Delay (e_rt)
lsg_n221(ctx[])
{
	lsd_begin()
	lsd_in_number("Seconds", 2.0)
	if (lsd_enter(221, ctx))
		return
	remove_task(LSG_TASK + 221)
	set_task(floatmax(2.0, 0.1), "lsg_t221", LSG_TASK + 221, ctx, LSG_CTX)
	lsd_exit(221, "waiting")
}

public lsg_t221(const data[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t221", LSG_TASK + 221, data, LSG_CTX)
		return
	}
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsd_exit(221, "fired")
	lsg_n222(ctx)
}

// Print String (e_pr)
lsg_n222(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", lsg_ntext(Float:g_lsgO[5]))
	if (lsd_enter(222, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Count:", lsg_ntext(Float:g_lsgO[5]))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(222, esc)
	lsg_n223(ctx)
}

// Print String (e_pr2)
lsg_n223(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", g_lsgYesNo[(g_lsgO[6]) ? 1 : 0])
	if (lsd_enter(223, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Was A:", g_lsgYesNo[(g_lsgO[6]) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(223, esc)
	lsg_n217_reset(ctx)
}

// Sequence (e_seq)
lsg_n224(ctx[])
{
	lsd_begin()
	if (lsd_enter(224, ctx))
		return
	lsd_exit(224, "ran")
	lsg_n218_toggle(ctx)
	lsg_n218_close(ctx)
	lsg_n218_open(ctx)
}

// Player Spawned (f0)
lsg_n225(ctx[])
{
	lsd_begin()
	if (lsd_enter(225, ctx))
		return
	lsd_exit(225, "ran")
	lsg_n226(ctx)
}

// For Loop with Break (f_brk)
lsg_n226(ctx[])
{
	lsd_begin()
	lsd_in_number("First", 0.0)
	lsd_in_number("Last", 10.0)
	if (lsd_enter(226, ctx))
		return
	new c[LSG_CTX], n
	g_lsgLeft[226] = 0
	new last = floatround(10.0)
	for (new k = floatround(0.0); k <= last && n < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[18] = _:float(k)
		lsg_n227(c)
		if (g_lsgLeft[226])
			break
	}
	lsd_exit(226, fmt("%d times", n))
	lsg_n229(ctx)
}

stock lsg_n226_break(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[226] = 1
}

// Branch (f_br)
lsg_n227(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", ((Float:ctx[18]) >= (3.0) && (Float:ctx[18]) <= (5.0) ? 1 : 0))
	if (lsd_enter(227, ctx))
		return
	new ok = ((((((Float:ctx[18]) >= (3.0) && (Float:ctx[18]) <= (5.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(227, g_lsdBool[ok])
	if (ok)
		lsg_n226_break(ctx)
	else
		lsg_n228(ctx)
}

// Print String (f_pr)
lsg_n228(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%s %s %s %s", lsg_ntext(floatabs(Float:ctx[18])), lsg_ntext(((Float:ctx[18]) + ((10.0) - (Float:ctx[18])) * (0.25))), lsg_ntext((((((Float:ctx[18]) == (2.0) ? 1 : 0)) ? 1 : 0) ? (floatsqroot(floatmax(Float:ctx[18], 0.0))) : (lsg_mod(Float:ctx[18], 3.0)))), lsg_ntext(lsg_pow(Float:ctx[18], 2.0))))
	if (lsd_enter(228, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Maths:", fmt("%s %s %s %s", lsg_ntext(floatabs(Float:ctx[18])), lsg_ntext(((Float:ctx[18]) + ((10.0) - (Float:ctx[18])) * (0.25))), lsg_ntext((((((Float:ctx[18]) == (2.0) ? 1 : 0)) ? 1 : 0) ? (floatsqroot(floatmax(Float:ctx[18], 0.0))) : (lsg_mod(Float:ctx[18], 3.0)))), lsg_ntext(lsg_pow(Float:ctx[18], 2.0))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(228, esc)
}

// Print String (f_pr2)
lsg_n229(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", lsg_tsel((((lsg_valid(ctx[0]) ? 1 : 0)) ? 1 : 0), "valid", "gone"))
	if (lsd_enter(229, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Player is", lsg_tsel((((lsg_valid(ctx[0]) ? 1 : 0)) ? 1 : 0), "valid", "gone"))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(229, esc)
	lsg_n230(ctx)
}

// Print String (f_pr3)
lsg_n230(ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", (lsg_ok(ctx[0]) ? 1 : 0))
	if (lsd_enter(230, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Is a player:", g_lsgYesNo[((lsg_ok(ctx[0]) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(230, esc)
	lsg_n231(ctx)
}

// Print String (f_pr4)
lsg_n231(ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", (lsg_is_creature(ctx[0]) ? 1 : 0))
	if (lsd_enter(231, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Is a creature:", g_lsgYesNo[((lsg_is_creature(ctx[0]) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(231, esc)
	lsg_n232(ctx)
}

// Set Variable (f_setc)
lsg_n232(ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[0])
	lsd_in_text("Value", g_lsgE0[clamp(2, 0, 2)])
	if (lsd_enter(232, ctx))
		return
	g_pv16[lsg_slot(ctx[0])] = 2
	lsd_exit(232, "set")
	lsg_n233(ctx)
}

// Switch on Enum (f_sw)
lsg_n233(ctx[])
{
	lsd_begin()
	lsd_in_text("Selection", g_lsgE0[clamp(g_pv16[lsg_slot(ctx[0])], 0, 2)])
	if (lsd_enter(233, ctx))
		return
	switch (g_pv16[lsg_slot(ctx[0])])
	{
		case 0:
		{
			}
		case 1:
		{
			}
		case 2:
		{
			lsg_n234(ctx)
		}
	}
}

// Print String (f_pr5)
lsg_n234(ctx[])
{
	lsd_begin()
	lsd_in_bool("Value", ((g_pv16[lsg_slot(ctx[0])]) == (2) ? 1 : 0))
	if (lsd_enter(234, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Medic:", g_lsgYesNo[(((g_pv16[lsg_slot(ctx[0])]) == (2) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(234, esc)
	lsg_n235(ctx)
}

// Set Variable (f_sp)
lsg_n235(ctx[])
{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsg_aplayers(1))))
	if (lsd_enter(235, ctx))
		return
	lsg_acopy(g_v10, lsg_aplayers(1))
	lsd_exit(235, "set")
	lsg_n236(ctx)
}

// Print String (f_pr6)
lsg_n236(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", float(lsg_alen(lsg_aplayers(1))))
	if (lsd_enter(236, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "Alive:", floatround(float(lsg_alen(lsg_aplayers(1)))))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(236, esc)
}

// Function Entry (fa_e)
stock lsg_n237(ctx[])
{
	lsd_begin()
	if (lsd_enter(237, ctx))
		return
	lsd_exit(237, "called")
	lsg_n238(ctx)
}

// Set Variable (fa_set)
stock lsg_n238(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", ((Float:ctx[20]) + (g_v18)))
	if (lsd_enter(238, ctx))
		return
	g_v18 = ((Float:ctx[20]) + (g_v18))
	lsd_exit(238, fmt("%.2f", g_v18))
	lsg_n239(ctx)
}

// Add (fa_push)
stock lsg_n239(ctx[])
{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(g_v19)))
	lsd_in_player("Item", ctx[19])
	if (lsd_enter(239, ctx))
		return
	g_lsgO[7] = _:float(lsg_apushc(g_v19, ctx[19]))
	lsd_exit(239, fmt("%d items", lsg_alen(g_v19)))
	lsg_n240(ctx)
}

// Branch (fa_br)
stock lsg_n240(ctx[])
{
	lsd_begin()
	lsd_in_bool("Condition", (ls_alive(ctx[19]) ? 1 : 0))
	if (lsd_enter(240, ctx))
		return
	new ok = (((((ls_alive(ctx[19]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	lsd_exit(240, g_lsdBool[ok])
	if (ok)
		lsg_n241(ctx)
	else
		lsg_n242(ctx)
}

// Return Node (fa_r1)
stock lsg_n241(ctx[])
{
	lsd_begin()
	lsd_in_number("Total", g_v18)
	lsd_in_text("Message", lsg_tcat(g_lsgOT[1], "!"))
	if (lsd_enter(241, ctx))
		return
	g_lsgR0_0 = g_v18
	copy(g_lsgR0_1, 127, lsg_tcat(g_lsgOT[1], "!"))
	g_lsgFD0 = true
	lsd_exit(241, "returned")
}

// Return Node (fa_r2)
stock lsg_n242(ctx[])
{
	lsd_begin()
	if (lsd_enter(242, ctx))
		return
	g_lsgR0_0 = 0.0
	copy(g_lsgR0_1, 127, "")
	g_lsgFD0 = true
	lsd_exit(242, "returned")
}

// Function Entry (fb_e)
stock lsg_n243(ctx[])
{
	lsd_begin()
	if (lsd_enter(243, ctx))
		return
	lsd_exit(243, "called")
	lsg_n244(ctx)
}

// Return Node (fb_r)
stock lsg_n244(ctx[])
{
	lsd_begin()
	lsd_in_number("Y", ((Float:ctx[21]) * (2.0)))
	lsd_in_text("As text", lsg_ntext(((Float:ctx[21]) * (2.0))))
	if (lsd_enter(244, ctx))
		return
	g_lsgR1_0 = ((Float:ctx[21]) * (2.0))
	copy(g_lsgR1_1, 127, lsg_ntext(((Float:ctx[21]) * (2.0))))
	g_lsgFD1 = true
	lsd_exit(244, "returned")
}

// Function Entry (fc_e)
stock lsg_n245(ctx[])
{
	lsd_begin()
	if (lsd_enter(245, ctx))
		return
	lsd_exit(245, "called")
	lsg_n246(ctx)
}

// Print String (fc_pr)
stock lsg_n246(ctx[])
{
	lsd_begin()
	if (lsd_enter(246, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "tick")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(246, esc)
}

// Print String (d_mac)
lsg_n247(ctx[])
{
	lsd_begin()
	lsd_in_number("Value", 3.0)
	if (lsd_enter(247, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "In the macro:", floatround(3.0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(247, esc)
	lsg_n214(ctx)
}

lsd_call(n, const data[])
{
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
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
		case 77: lsg_n77(ctx)
		case 78: lsg_n78(ctx)
		case 79: lsg_n79(ctx)
		case 80: lsg_n80(ctx)
		case 81: lsg_n81(ctx)
		case 82: lsg_n82(ctx)
		case 83: lsg_n83(ctx)
		case 84: lsg_n84(ctx)
		case 85: lsg_n85(ctx)
		case 86: lsg_n86(ctx)
		case 87: lsg_n87(ctx)
		case 88: lsg_n88(ctx)
		case 89: lsg_n89(ctx)
		case 90: lsg_n90(ctx)
		case 91: lsg_n91(ctx)
		case 92: lsg_n92(ctx)
		case 93: lsg_n93(ctx)
		case 94: lsg_n94(ctx)
		case 95: lsg_n95(ctx)
		case 96: lsg_n96(ctx)
		case 97: lsg_n97(ctx)
		case 98: lsg_n98(ctx)
		case 99: lsg_n99(ctx)
		case 100: lsg_n100(ctx)
		case 101: lsg_n101(ctx)
		case 102: lsg_n102(ctx)
		case 103: lsg_n103(ctx)
		case 104: lsg_n104(ctx)
		case 105: lsg_n105(ctx)
		case 106: lsg_n106(ctx)
		case 107: lsg_n107(ctx)
		case 108: lsg_n108(ctx)
		case 109: lsg_n109(ctx)
		case 110: lsg_n110(ctx)
		case 111: lsg_n111(ctx)
		case 112: lsg_n112(ctx)
		case 113: lsg_n113(ctx)
		case 114: lsg_n114(ctx)
		case 115: lsg_n115(ctx)
		case 116: lsg_n116(ctx)
		case 117: lsg_n117(ctx)
		case 118: lsg_n118(ctx)
		case 119: lsg_n119(ctx)
		case 120: lsg_n120(ctx)
		case 121: lsg_n121(ctx)
		case 122: lsg_n122(ctx)
		case 123: lsg_n123(ctx)
		case 124: lsg_n124(ctx)
		case 125: lsg_n125(ctx)
		case 126: lsg_n126(ctx)
		case 127: lsg_n127(ctx)
		case 128: lsg_n128(ctx)
		case 129: lsg_n129(ctx)
		case 130: lsg_n130(ctx)
		case 131: lsg_n131(ctx)
		case 132: lsg_n132(ctx)
		case 133: lsg_n133(ctx)
		case 134: lsg_n134(ctx)
		case 135: lsg_n135(ctx)
		case 136: lsg_n136(ctx)
		case 137: lsg_n137(ctx)
		case 138: lsg_n138(ctx)
		case 139: lsg_n139(ctx)
		case 140: lsg_n140(ctx)
		case 141: lsg_n141(ctx)
		case 142: lsg_n142(ctx)
		case 143: lsg_n143(ctx)
		case 144: lsg_n144(ctx)
		case 145: lsg_n145(ctx)
		case 146: lsg_n146(ctx)
		case 147: lsg_n147(ctx)
		case 148: lsg_n148(ctx)
		case 149: lsg_n149(ctx)
		case 150: lsg_n150(ctx)
		case 151: lsg_n151(ctx)
		case 152: lsg_n152(ctx)
		case 153: lsg_n153(ctx)
		case 154: lsg_n154(ctx)
		case 155: lsg_n155(ctx)
		case 156: lsg_n156(ctx)
		case 157: lsg_n157(ctx)
		case 158: lsg_n158(ctx)
		case 159: lsg_n159(ctx)
		case 160: lsg_n160(ctx)
		case 161: lsg_n161(ctx)
		case 162: lsg_n162(ctx)
		case 163: lsg_n163(ctx)
		case 164: lsg_n164(ctx)
		case 165: lsg_n165(ctx)
		case 166: lsg_n166(ctx)
		case 167: lsg_n167(ctx)
		case 168: lsg_n168(ctx)
		case 169: lsg_n169(ctx)
		case 170: lsg_n170(ctx)
		case 171: lsg_n171(ctx)
		case 172: lsg_n172(ctx)
		case 173: lsg_n173(ctx)
		case 174: lsg_n174(ctx)
		case 175: lsg_n175(ctx)
		case 176: lsg_n176(ctx)
		case 177: lsg_n177(ctx)
		case 178: lsg_n178(ctx)
		case 179: lsg_n179(ctx)
		case 180: lsg_n180(ctx)
		case 181: lsg_n181(ctx)
		case 182: lsg_n182(ctx)
		case 183: lsg_n183(ctx)
		case 184: lsg_n184(ctx)
		case 185: lsg_n185(ctx)
		case 186: lsg_n186(ctx)
		case 187: lsg_n187(ctx)
		case 188: lsg_n188(ctx)
		case 189: lsg_n189(ctx)
		case 190: lsg_n190(ctx)
		case 191: lsg_n191(ctx)
		case 192: lsg_n192(ctx)
		case 193: lsg_n193(ctx)
		case 194: lsg_n194(ctx)
		case 195: lsg_n195(ctx)
		case 196: lsg_n196(ctx)
		case 197: lsg_n197(ctx)
		case 198: lsg_n198(ctx)
		case 199: lsg_n199(ctx)
		case 200: lsg_n200(ctx)
		case 201: lsg_n201(ctx)
		case 202: lsg_n202(ctx)
		case 203: lsg_n203(ctx)
		case 204: lsg_n204(ctx)
		case 205: lsg_n205(ctx)
		case 206: lsg_n206(ctx)
		case 207: lsg_n207(ctx)
		case 208: lsg_n208(ctx)
		case 209: lsg_n209(ctx)
		case 210: lsg_n210(ctx)
		case 211: lsg_n211(ctx)
		case 212: lsg_n212(ctx)
		case 213: lsg_n213(ctx)
		case 214: lsg_n214(ctx)
		case 215: lsg_n215(ctx)
		case 216: lsg_n216(ctx)
		case 217: lsg_n217(ctx)
		case 218: lsg_n218(ctx)
		case 219: lsg_n219(ctx)
		case 220: lsg_n220(ctx)
		case 221: lsg_n221(ctx)
		case 222: lsg_n222(ctx)
		case 223: lsg_n223(ctx)
		case 224: lsg_n224(ctx)
		case 225: lsg_n225(ctx)
		case 226: lsg_n226(ctx)
		case 227: lsg_n227(ctx)
		case 228: lsg_n228(ctx)
		case 229: lsg_n229(ctx)
		case 230: lsg_n230(ctx)
		case 231: lsg_n231(ctx)
		case 232: lsg_n232(ctx)
		case 233: lsg_n233(ctx)
		case 234: lsg_n234(ctx)
		case 235: lsg_n235(ctx)
		case 236: lsg_n236(ctx)
		case 237: lsg_n237(ctx)
		case 238: lsg_n238(ctx)
		case 239: lsg_n239(ctx)
		case 240: lsg_n240(ctx)
		case 241: lsg_n241(ctx)
		case 242: lsg_n242(ctx)
		case 243: lsg_n243(ctx)
		case 244: lsg_n244(ctx)
		case 245: lsg_n245(ctx)
		case 246: lsg_n246(ctx)
		case 247: lsg_n247(ctx)
	}
}
