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
#define LSG_CTX 28
#define LSG_TXT 2
#define LSG_N 250
#define LSG_TASK 7310000
#pragma dynamic 164096
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
#define LSD_N 250
new const g_lsdIds[LSD_N][] = { "rs", "rs_set", "rs_cd", "rs_msg", "rs_rep", "rs_hud", "rs_delay", "rs_wave", "rs_wstop", "rs_reset", "pk", "pk_br", "pk_add", "pk_add2", "pk_team", "pk_give", "pk_seth", "pk_hud", "ps", "ps_strip", "ps_arm", "ps_spd", "ps_team", "ps_tp", "ps_set", "pj", "pj_hud", "pj_br", "pl", "pl_set", "bp", "bp_hud", "bd", "bd_end", "bx", "bx_end", "re", "re_br", "re_set", "re_stop", "pu", "pu_door", "pu_trap", "pt", "pt_crate", "cc", "cc_lock", "cc_hud", "cl", "cl_ex", "zs", "zs_shrink", "zt", "zt_set", "zt_hud", "ex", "ex_add", "ws", "ws_hud", "wp", "wp_hud", "wc", "wc_start", "lo", "lo_hud", "ck", "ck_set", "bb", "bb_msg", "me", "me_hud", "ck2", "ck2_set", "ck2_make", "ck2_spawn", "ck2_hud", "ck2_hud2", "hurt", "hurt_tadd", "hurt_glow", "tc", "tc_model", "nr", "nr_cvar", "nr_each", "nr_resp", "nr_pick", "nr_ents", "nr_use", "nr_set", "nr_stop", "nr_hud", "cmd", "cmd_menu", "cmd_arm", "cmd_money", "cmd_grav", "cmd_bar", "cmd_chat", "fn", "fn_frag", "fn_snd", "fn_role", "pk2", "pk2_call", "pk2_pick", "pk2_bot", "pk2_hold", "pk2_follow", "pk2_free", "pk2_drop", "pk2_fr", "pk2_kill", "lo2", "lo2_br", "lo2_pick", "lo2_br2", "lo2_resp", "lo2_drop", "lo2_hud", "lo2_br3", "lo2_end", "ms", "ms_pack", "ms_pick", "ms_start", "ms_stop", "ms_make", "ms_drop", "ms_each", "ms_tp", "ah", "ah_br", "ah_block", "ah_scale", "ah_set", "dk", "dk_down", "rv", "rv_hud", "to", "to_add", "to_set", "to_hud", "to_br", "sq", "sq_nf", "sq_set", "sq_ff", "sq_hud", "sq_buy", "sq_allow", "sq_up", "sq_goto", "ck3", "ck3_set", "ck3_add", "ck3_p", "ck3_r", "ck3_hud", "tu", "tu_end", "a0", "a_seq", "a_for", "a_add", "a_sort", "a_shuf", "a_ins", "a_rmi", "a_aset", "a_arem", "a_fe", "a_br", "a_pr", "a_pr2", "a_clr", "b_set", "b_wh", "b_add", "b_sw", "b_pr1", "b_sn", "b_pr2", "b_pr3", "c_each", "c_setp", "c_pos", "c_vel", "c_pr", "c_sl", "c_sf", "c_sf2", "c_push", "c_pr2", "c_sw", "c_pr3", "c_pr4", "d_each", "d_madd", "d_call", "d_pr", "d_pr2", "d_tadd", "d_vadd", "d_rem", "d_pr3", "d_mclr", "d_pr4", "d_pr5", "d_tbf", "d_tset", "d_pr6", "d_tclr", "d_pr7", "e0", "e_once", "e_n", "e_gate", "e_ff", "e_iv", "e_rt", "e_pr", "e_pr2", "e_seq", "f0", "f_brk", "f_br", "f_pr", "f_pr2", "f_pr3", "f_pr4", "f_setc", "f_sw", "f_pr5", "f_sp", "f_pr6", "fa_e", "fa_set", "fa_push", "fa_br", "fa_r1", "fa_r2", "fb_e", "fb_r", "fc_e", "mk_fe", "fc_pr", "mk_pr", "d_mac" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
// The Game Mode Studio's debug runtime (tools/studio/gen.rs puts it into Playtest builds only).
// Every helper is a stock, so the ones a graph doesn't use cost nothing and warn nothing.
new const g_lsdBool[][] = { "false", "true" }
new const g_lsdTeams[][] = { "none", "T", "CT", "spectator" }
#define LSD_TASK 9475100
new g_lsdEvents[160], g_lsdCmd[160], g_lsdSeen[160]
new g_lsdSeq, g_lsdTick, g_lsdRound
new g_lsdPhase[8] = "freeze"
new g_lsdIn[600]
new g_lsdPaused = -1, g_lsdSkip = -1
new g_lsdCtx[LSG_CTX]
// while the world is paused nothing else runs: what would (a loop's other rounds, what comes
// after the paused node, other events) waits here in order and runs after Resume, so a
// breakpoint inside a loop pauses that round and the loop carries on from there
#define LSD_Q 128
new g_lsdQ[LSD_Q][LSG_CTX + 1], g_lsdQn, g_lsdQins
new Float:g_lsdPausedAt, Float:g_lsdResumedAt
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
	formatex(g_lsdSeen, charsmax(g_lsdSeen), "%s/%s.seen", dir, LSG_MODE)
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
	lsd_flush()
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

// events wait in memory and go to the file every 0.25 s (lsd_poll), when the buffer is nearly
// full, and at once for a pause (the page shows it); one open/write/close per flush, not per
// event (PerfAudit #5: 181 us an event). Under 16384: AMX Mod X's fputs takes at most 16383
// characters of a string and drops the rest (node coverage run: 44 events lost, one cut in half)
#define LSD_BUF 16000
new g_lsdBuf[LSD_BUF], g_lsdBufLen
// how big the events file is now (it starts empty at each map)
new g_lsdBytes

stock lsd_flush()
{
	if (!g_lsdBufLen)
		return
	new f = fopen(g_lsdEvents, "at")
	if (f)
	{
		fputs(f, g_lsdBuf)
		fclose(f)
		g_lsdBytes += g_lsdBufLen
	}
	g_lsdBuf[0] = 0
	g_lsdBufLen = 0
}

// ---- who's watching, and how much is written (StudioPerfAudit #5) ----
// The app writes <mode>.seen (how far it has read) each time the page reads. Not changed for
// 15 s: nobody is watching, so nodes' runs aren't written (breakpoints still pause). With no
// .seen at all (a Test run, or before the page's first read) everything is written.
#define LSD_AWAY 15
// the file starts over once the reader has had all of it past 8 MB, or regardless at 32 MB
// (the reader sees it shrink and reads from the top: a "reset")
#define LSD_CAP 8000000
#define LSD_HARD 32000000
// each node writes at most this many runs a half second (4 a second: plenty to watch it); the
// rest are counted, and the next run written carries the count ("skipped"), so totals stay right
#define LSD_PER 2
new bool:g_lsdAway, g_lsdRead = -1
new Float:g_lsdWin[LSD_N], g_lsdWinN[LSD_N], g_lsdSkipped[LSD_N], bool:g_lsdMute[LSD_N]

stock lsd_watchers()
{
	if (!file_exists(g_lsdSeen))
	{
		g_lsdAway = false
		g_lsdRead = -1
		return
	}
	g_lsdAway = get_systime() - GetFileTime(g_lsdSeen, FileTime_LastChange) > LSD_AWAY
	new f = fopen(g_lsdSeen, "rt"), line[24]
	if (f)
	{
		fgets(f, line, charsmax(line))
		fclose(f)
		g_lsdRead = str_to_num(line)
	}
}

stock lsd_rotate()
{
	if (g_lsdBytes > LSD_HARD || (g_lsdBytes > LSD_CAP && g_lsdRead >= g_lsdBytes))
	{
		new f = fopen(g_lsdEvents, "wt")
		if (f)
			fclose(f)
		g_lsdBytes = 0
	}
}

// Is node n's run to be written? (its inputs are only worked out into JSON when it is)
// `run`: false for a note that isn't a run (a Timeline's progress): it shares the budget but
// doesn't count as a skipped run
stock bool:lsd_want(n, bool:run = true)
{
	if (g_lsdBp[n])
	{
		g_lsdMute[n] = false
		return true
	}
	if (g_lsdAway)
	{
		if (run)
			g_lsdSkipped[n]++
		g_lsdMute[n] = true
		return false
	}
	new Float:now = get_gametime()
	if (now - g_lsdWin[n] >= 0.5)
	{
		g_lsdWin[n] = now
		g_lsdWinN[n] = 0
	}
	if (++g_lsdWinN[n] > LSD_PER)
	{
		if (run)
			g_lsdSkipped[n]++
		g_lsdMute[n] = true
		return false
	}
	g_lsdMute[n] = false
	return true
}

stock lsd_emitf(const fmt[], any:...)
{
	static body[1400], line[1500]
	vformat(body, charsmax(body), fmt, 2)
	new n = formatex(line, charsmax(line), "{^"s^":%d,^"t^":%.2f,%s}^n", ++g_lsdSeq, get_gametime(), body)
	if (g_lsdBufLen + n >= LSD_BUF - 1)
		lsd_flush()
	g_lsdBufLen += copy(g_lsdBuf[g_lsdBufLen], LSD_BUF - 1 - g_lsdBufLen, line)
	// a pause, or the world stopping or going on: the page needs it now
	if (containi(body, "breakpoint_hit") >= 0 || containi(body, "world_") >= 0 || containi(body, "session") >= 0)
		lsd_flush()
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

// a loop's result: how many rounds; when some are waiting at a breakpoint, it says so
stock lsd_times(n)
{
	new t[64]
	if (g_lsdPaused >= 0)
		formatex(t, charsmax(t), "%d rounds (some still wait at a breakpoint)", n)
	else
		formatex(t, charsmax(t), "%d times", n)
	return t
}

stock lsd_defer(n, const ctx[])
{
	if (g_lsdQn >= LSD_Q)
	{
		lsd_emitf("^"e^":^"node_error^",^"node^":^"%s^",^"message^":^"Too much waiting while paused: this run was dropped.^"", g_lsdIds[n])
		return
	}
	for (new q = g_lsdQn; q > g_lsdQins; q--)
		for (new k = 0; k <= LSG_CTX; k++)
			g_lsdQ[q][k] = g_lsdQ[q - 1][k]
	g_lsdQ[g_lsdQins][0] = n
	for (new k = 0; k < LSG_CTX; k++)
		g_lsdQ[g_lsdQins][k + 1] = ctx[k]
	g_lsdQins++
	g_lsdQn++
}

stock bool:lsd_enter(n, const ctx[])
{
	if (g_lsdSkip == n)
		g_lsdSkip = -1
	else if (g_lsdPaused >= 0)
	{
		lsd_defer(n, ctx)
		return true
	}
	else if (g_lsdBp[n])
	{
		g_lsdPaused = n
		// (what waits from now on goes before what was already waiting)
		g_lsdQins = 0
		for (new i = 0; i < LSG_CTX; i++)
			g_lsdCtx[i] = ctx[i]
		lsd_emitf("^"e^":^"breakpoint_hit^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
		lsd_freeze()
		return true
	}
	if (g_lsdMute[n])
		return false
	if (g_lsdSkipped[n])
	{
		lsd_emitf("^"e^":^"node_enter^",^"node^":^"%s^",^"skipped^":%d,^"inputs^":{%s}", g_lsdIds[n], g_lsdSkipped[n], g_lsdIn)
		g_lsdSkipped[n] = 0
	}
	else
		lsd_emitf("^"e^":^"node_enter^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
	return false
}

stock lsd_exit(n, const result[])
{
	if (!g_lsdMute[n])
		lsd_emitf("^"e^":^"node_exit^",^"node^":^"%s^",^"result^":^"%s^"", g_lsdIds[n], result)
}

// a note that's always written while someone watches (a Timeline's Finished)
stock lsd_note(n, const result[])
{
	if (!g_lsdAway)
		lsd_emitf("^"e^":^"node_exit^",^"node^":^"%s^",^"result^":^"%s^"", g_lsdIds[n], result)
}

stock lsd_error(n, const msg[])
{
	// (a sampled run's error still shows, unless nobody is watching)
	if (!g_lsdAway)
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
	g_lsdResumedAt = get_gametime()
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

// a live tweak from the app: "set <variable id> <who> <value>" (who: 0 the mode's, a player's
// index, or 1 / 2 a team's; the app checked the value against the variable's type)
stock lsd_set_cmd(const s[])
{
	new vid[32], who[8], pos
	pos = argparse(s, 0, vid, charsmax(vid))
	if (pos == -1)
		return
	pos = argparse(s, pos, who, charsmax(who))
	if (pos == -1)
		return
	while (s[pos] == ' ')
		pos++
	new val[64]
	copy(val, charsmax(val), s[pos])
	if (lsd_graph_set(vid, str_to_num(who), val))
	{
		new esc[140]
		lsd_esc(val, esc, charsmax(esc))
		lsd_emitf("^"e^":^"var_set^",^"v^":^"%s^",^"p^":%d,^"value^":^"%s^"", vid, str_to_num(who), esc)
		lsd_vars()
	}
}

// a timer that fires while the world is paused waits until it isn't
stock bool:lsd_hold_timer()
{
	return g_lsdPaused >= 0
}

// A node's timer (Delay, Repeat, Countdown) that came due while the world was paused: after
// Resume it waits what it still had to wait when the pause began, from the Resume, so a Repeat
// keeps its rhythm instead of firing at once (it's checked four times a second while paused).
// True: not now (it has been put off). `fn`: the timer's own callback; its task id is LSG_TASK + n.
new bool:g_lsdTWait[LSD_N], Float:g_lsdTOwe[LSD_N]
stock bool:lsd_timer_wait(n, const fn[], const data[])
{
	if (g_lsdPaused >= 0)
	{
		if (!g_lsdTWait[n])
		{
			g_lsdTWait[n] = true
			g_lsdTOwe[n] = get_gametime() - g_lsdPausedAt
		}
		set_task(0.25, fn, LSG_TASK + n, data, LSG_CTX)
		return true
	}
	if (!g_lsdTWait[n])
		return false
	g_lsdTWait[n] = false
	new Float:left = g_lsdTOwe[n] - (get_gametime() - g_lsdResumedAt)
	if (left < 0.05)
		return false
	set_task(left, fn, LSG_TASK + n, data, LSG_CTX)
	return true
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
	// then what waited, in order, until it's all run or another breakpoint pauses again
	while (g_lsdPaused < 0 && g_lsdQn > 0)
	{
		new m = g_lsdQ[0][0]
		for (new i = 0; i < LSG_CTX; i++)
			ctx[i] = g_lsdQ[0][i + 1]
		for (new q = 1; q < g_lsdQn; q++)
			for (new k = 0; k <= LSG_CTX; k++)
				g_lsdQ[q - 1][k] = g_lsdQ[q][k]
		g_lsdQn--
		lsd_call(m, ctx)
	}
}

public lsd_poll()
{
	// (who's watching and how far they've read: once a second; the old part of the file goes
	// before this flush, so nothing unread is lost)
	if (++g_lsdTick % 4 == 1)
	{
		lsd_watchers()
		lsd_rotate()
	}
	lsd_flush()
	if (g_lsdPaused >= 0)
	{
		for (new id = 1; id <= MaxClients; id++)
			lsd_hold_player(id)
		if (g_lsdTick % 4 == 0)
			lsd_vars()
	}
	else if (g_lsdHolding && lsd_time_left() <= 0)
		lsd_unhold()
	if (!file_exists(g_lsdCmd))
		return
	// taken aside first: a command the app adds while this reads goes into a new file, not lost
	new take[170]
	formatex(take, charsmax(take), "%s.take", g_lsdCmd)
	if (file_exists(take))
		delete_file(take)
	if (!rename_file(g_lsdCmd, take, 1))
		return
	new f = fopen(take, "rt")
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
		else if (equal(line, "set ", 4))
			lsd_set_cmd(line[4])
	}
	fclose(f)
	delete_file(take)
	lsd_emitf("^"e^":^"commands^",^"resume^":%s", g_lsdBool[resume ? 1 : 0])
	if (resume)
		lsd_resume()
}

// ---- the game and the server (Game, World, Players, Entities, Effects, Advanced nodes) ----
stock g_lsgRounds, g_lsgScore[3], bool:g_lsgFreeze, Float:g_lsgGagTill[33], g_lsgFw = -1, bool:lsg_cmd_ok, g_lsgSprExp, g_lsgSprBeam, Trie:g_lsgHave = Invalid_Trie

// a model or sound the graph names, loaded only if the file is there (on the server or in the
// game's folders): a missing one would stop the map loading. What uses it does without.
stock lsg_pre_model(const p[])
{
	if (!file_exists(p, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, p)
		return 0
	}
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_model(p)
}

stock lsg_pre_sound(const p[])
{
	new full[128]
	formatex(full, charsmax(full), "sound/%s", p)
	if (!file_exists(full, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, full)
		return 0
	}
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_sound(p)
}

// was it loaded (lsg_pre_model / lsg_pre_sound)?
stock bool:lsg_have(const p[])
	return g_lsgHave != Invalid_Trie && TrieKeyExists(g_lsgHave, p)

stock lsg_map_name()
{
	new m[32]
	get_mapname(m, charsmax(m))
	return m
}

// cvars no mode may read (passwords, rcon, where files and logs go)
stock bool:lsg_secret(const c[])
{
	return containi(c, "password") >= 0 || containi(c, "rcon") >= 0 || equali(c, "sv_downloadurl") || equali(c, "log", 3) || containi(c, "_file") >= 0 || equali(c, "amx_sql", 7) || equali(c, "hostport") || equali(c, "ip")
}

stock lsg_cvar_text(const c[])
{
	new t[64]
	if (!lsg_secret(c) && get_cvar_pointer(c))
		get_cvar_string(c, t, charsmax(t))
	return t
}

stock Float:lsg_cvar_num(const c[])
	return !lsg_secret(c) && get_cvar_pointer(c) ? get_cvar_float(c) : 0.0

// 0 all, 1 people, 2 bots, 3 T, 4 CT, 5 spectators, 6 alive
stock lsg_count(which)
{
	new n
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p))
			continue
		new t = get_user_team(p)
		switch (which)
		{
			case 1: if (!is_user_bot(p)) n++
			case 2: if (is_user_bot(p)) n++
			case 3: if (t == 1) n++
			case 4: if (t == 2) n++
			case 5: if (t != 1 && t != 2) n++
			case 6: if (is_user_alive(p)) n++
			default: n++
		}
	}
	return n
}

stock lsg_clock(const fmt[])
{
	new t[40]
	get_time(fmt, t, charsmax(t))
	return t
}

stock bool:lsg_helmet(id)
{
	if (!lsg_ok(id))
		return false
	new CsArmorType:t
	cs_get_user_armor(id, t)
	return t == CS_ARMOR_VESTHELM
}

stock lsg_held(id)
{
	new w[24]
	if (lsg_ok(id) && is_user_alive(id))
	{
		new clip, ammo, wid = get_user_weapon(id, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

stock lsg_ammo(id, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return 0
	new clip, ammo
	get_user_weapon(id, clip, ammo)
	return reserve ? ammo : max(clip, 0)
}

stock lsg_ping(id)
{
	new ping, loss
	if (lsg_ok(id))
		get_user_ping(id, ping, loss)
	return ping
}

stock lsg_steam_id(id)
{
	new s[44]
	if (lsg_ok(id) && !is_user_bot(id))
	{
		get_user_authid(id, s, charsmax(s))
		if (!equal(s, "STEAM_", 6))
			s[0] = 0
	}
	return s
}

// the connected player with this Steam ID: case and the universe digit (STEAM_0 / STEAM_1) aside,
// as the app's roles compare them (only the X:Y after STEAM_n: counts)
stock lsg_by_steam(const id[])
{
	if (!equali(id, "STEAM_", 6) || !id[6] || id[7] != ':')
		return 0
	new s[44]
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		get_user_authid(p, s, charsmax(s))
		if (equali(s, "STEAM_", 6) && s[6] && s[7] == ':' && equali(s[8], id[8]))
			return p
	}
	return 0
}

// a Loner-Strike supporter: access flag "s" (gm_vip's SUPPORTER_FLAG; the app writes it from the
// roles sync). Not bots or HLTV, and not the listen-server host, who has every flag
stock bool:lsg_supporter(id)
{
	if (!lsg_ok(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	if (id == 1 && !is_dedicated_server())
		return false
	return (get_user_flags(id) & ADMIN_LEVEL_G) != 0
}

stock lsg_by_name(const name[])
	return name[0] ? max(find_player("bl", name), 0) : 0

stock lsg_slot_ok(i)
	return lsg_ok(i) ? i : 0

stock Float:lsg_pevf(id, field)
{
	new Float:f
	if (lsg_ok(id))
		pev(id, field, f)
	return f
}

stock lsg_model(id)
{
	new m[32]
	if (lsg_ok(id))
		cs_get_user_model(id, m, charsmax(m))
	return m
}

stock lsg_eclass(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_classname, t, charsmax(t))
	return t
}

stock lsg_ename(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_targetname, t, charsmax(t))
	return t
}

stock bool:lsg_plugin_on(const file[])
	return is_plugin_loaded(file, true) != -1

// (a map name is letters, digits, _ - . only, and the server must have it: nothing else reaches the console)
stock lsg_change_map(const map[])
{
	if (!map[0])
		return
	for (new i = 0; map[i]; i++)
		if (!isalnum(map[i]) && map[i] != '_' && map[i] != '-' && map[i] != '.')
			return
	if (is_map_valid(map))
		server_cmd("changelevel %s", map)
}

// text that goes into a console command's quotes: letters, digits, spaces and . , ! ? - ' ( ) only
// (no quote, semicolon, new line or other control character can end the quotes or add a command)
stock lsg_clean(const src[], dst[], len)
{
	new j
	for (new i = 0; src[i] && j < len; i++)
	{
		new c = src[i]
		if (isalnum(c) || c == ' ' || c == '.' || c == ',' || c == '!' || c == '?' || c == '-' || c == 39 || c == '(' || c == ')')
			dst[j++] = c
	}
	dst[j] = 0
}

stock lsg_slap(id, dmg)
{
	if (lsg_ok(id) && is_user_alive(id))
		user_slap(id, clamp(dmg, 0, 100))
}

stock lsg_fade(id, Float:secs, r, g, b, alpha)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenFade")
	new units = clamp(floatround(secs * 4096.0), 0, 65535)
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(units)
	write_short(units)
	write_short(0)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(alpha)
	message_end()
}

stock lsg_shake(id, Float:amp, Float:secs)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenShake")
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	write_short(clamp(floatround(secs * 4096.0), 0, 65535))
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	message_end()
}

stock lsg_godmode(id, on)
{
	if (lsg_ok(id))
		set_pev(id, pev_takedamage, on ? DAMAGE_NO : DAMAGE_AIM)
}

stock lsg_noclip(id, on)
{
	if (lsg_ok(id) && is_user_alive(id))
		set_pev(id, pev_movetype, on ? MOVETYPE_NOCLIP : MOVETYPE_WALK)
}

// 0 normal, 1 see-through by amount, 2 invisible
stock lsg_render(id, mode, amount)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_rendermode, mode ? kRenderTransAlpha : kRenderNormal)
	set_pev(id, pev_renderamt, mode == 2 ? 0.0 : (mode ? float(amount) : 255.0))
}

stock lsg_set_ammo(id, clip, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	// (the held weapon from the active item: get_user_weapon says 0 while a gun is being drawn,
	// and the reserve can't be set for weapon 0)
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
	{
		cs_set_weapon_ammo(w, clamp(clip, 0, 200))
		cs_set_user_bpammo(id, wid, clamp(reserve, 0, 255))
	}
}

stock lsg_give_ammo(id, n)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
		cs_set_user_bpammo(id, wid, clamp(cs_get_user_bpammo(id, wid) + n, 0, 255))
}

stock lsg_take_weapon(id, const wname[])
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new wid = get_weaponid(wname), e = -1
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", wname)) > 0)
		if (pev(e, pev_owner) == id)
			break
	if (e <= 0)
		return
	if (get_user_weapon(id) == wid)
		ExecuteHamB(Ham_Weapon_RetireWeapon, e)
	if (!ExecuteHamB(Ham_RemovePlayerItem, id, e))
		return
	ExecuteHamB(Ham_Item_Kill, e)
	set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
}

stock lsg_switch_weapon(id, const wname[])
{
	if (lsg_ok(id) && is_user_alive(id) && user_has_weapon(id, get_weaponid(wname)))
		engclient_cmd(id, wname)
}

stock lsg_set_frags(id, n)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_frags, float(n))
	static msg
	if (!msg)
		msg = get_user_msgid("ScoreInfo")
	message_begin(MSG_ALL, msg)
	write_byte(id)
	write_short(n)
	write_short(cs_get_user_deaths(id))
	write_short(0)
	write_short(get_user_team(id))
	message_end()
}

stock lsg_set_deaths(id, n)
{
	if (lsg_ok(id))
		cs_set_user_deaths(id, max(n, 0))
}

stock lsg_to_spec(id)
{
	if (!lsg_ok(id))
		return
	if (is_user_alive(id))
		user_kill(id, 1)
	cs_set_user_team(id, CS_TEAM_SPECTATOR)
}

stock lsg_kick(id, const reason[])
{
	if (!lsg_ok(id))
		return
	new r[61]
	lsg_clean(reason, r, charsmax(r))
	server_cmd("kick #%d ^"%s^"", get_user_userid(id), r)
}

stock lsg_team_chat(team, const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 3)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && get_user_team(p) == team)
			client_print(p, print_chat, "%s", msg)
}

stock lsg_admins_chat(const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 2)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (get_user_flags(p) & ADMIN_KICK))
			client_print(p, print_chat, "[Admins] %s", msg)
}

stock lsg_remove(e)
{
	if (e > MaxClients && pev_valid(e))
		engfunc(EngFunc_RemoveEntity, e)
}

stock lsg_ent_visible(e, on)
{
	if (e > 0 && pev_valid(e))
		set_pev(e, pev_effects, on ? (pev(e, pev_effects) & ~EF_NODRAW) : (pev(e, pev_effects) | EF_NODRAW))
}

stock lsg_sound_to(id, const snd[])
{
	if (lsg_ok(id))
		client_cmd(id, "spk ^"%s^"", snd)
}

stock lsg_fire_event(const name[], a, b)
{
	if (g_lsgFw < 0)
		g_lsgFw = CreateMultiForward("ls_studio_event", ET_IGNORE, FP_STRING, FP_CELL, FP_CELL)
	new ret
	ExecuteForward(g_lsgFw, ret, name, a, b)
}

// a public function in another plugin with up to two numbers; 1 if it was called
stock lsg_call(const plugin[], const func[], nargs, a, b, &r)
{
	r = 0
	if (callfunc_begin(func, plugin) != 1)
		return 0
	if (nargs >= 1)
		callfunc_push_int(a)
	if (nargs >= 2)
		callfunc_push_int(b)
	r = callfunc_end()
	return 1
}

stock lsg_hitgroup(v, bits)
{
	new t[12]
	if (!lsg_ok(v))
		return t
	// (only a bullet or the knife hits a place; the game keeps the last one, so a grenade or a
	// fall after a headshot would say head again)
	switch (bits & DMG_BULLET ? get_ent_data(v, "CBaseMonster", "m_LastHitGroup") : 0)
	{
		case HIT_HEAD: copy(t, charsmax(t), "head")
		case HIT_CHEST: copy(t, charsmax(t), "chest")
		case HIT_STOMACH: copy(t, charsmax(t), "stomach")
		case HIT_LEFTARM, HIT_RIGHTARM: copy(t, charsmax(t), "arm")
		case HIT_LEFTLEG, HIT_RIGHTLEG: copy(t, charsmax(t), "leg")
		default: copy(t, charsmax(t), "body")
	}
	return t
}

// the weapon that did the damage: a grenade by what hit (the attacker may hold a gun by then),
// else what the attacker holds; empty when no player did it. (Its own body: a stock that returns
// another stock's text fails at run time with "memory access".)
stock lsg_wpn_of(a, inflictor)
{
	new w[24]
	if (inflictor > MaxClients && pev_valid(inflictor))
	{
		pev(inflictor, pev_classname, w, charsmax(w))
		if (equal(w, "grenade"))
		{
			copy(w, charsmax(w), "hegrenade")
			return w
		}
		w[0] = 0
	}
	if (lsg_ok(a) && is_user_alive(a))
	{
		new clip, ammo, wid = get_user_weapon(a, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

// Give Weapon with a name wired in: only a weapon the game has (ak47, deagle, knife...)
stock lsg_give_named(id, const name[], ammo)
{
	new w[32]
	formatex(w, charsmax(w), "weapon_%s", name)
	if (equal(name, "weapon_", 7) || !get_weaponid(w))
		return 0
	return ls_give_weapon(id, w, ammo)
}

stock Float:lsg_dist(a, b)
{
	if (!lsg_ok(a) || !lsg_ok(b))
		return 0.0
	new Float:x[3], Float:y[3]
	pev(a, pev_origin, x)
	pev(b, pev_origin, y)
	return get_distance_f(x, y)
}

// what's after the first word ("/give 5" -> "5")
stock lsg_after_word(s[])
{
	new i
	while (s[i] && s[i] != ' ')
		i++
	while (s[i] == ' ')
		i++
	copy(s, 63, s[i])
}

stock lsg_hook_weapons(const fn[], Ham:h)
{
	static const W[][] = { "weapon_p228", "weapon_scout", "weapon_hegrenade", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_smokegrenade", "weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_flashbang", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_knife", "weapon_p90" }
	for (new i = 0; i < sizeof W; i++)
		RegisterHam(h, W[i], fn, 1)
}

stock lsg_prop(at, const model[], solid)
{
	new Float:o[3]
	if (!lsg_ent_origin(at, o))
		return 0
	// (a model that wasn't there when the map loaded: no prop)
	if (!lsg_have(model))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_prop")
	engfunc(EngFunc_SetModel, e, model)
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, solid ? SOLID_BBOX : SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	if (solid)
		engfunc(EngFunc_SetSize, e, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 32.0})
	return e
}

// the round table's kind of round (0 normal, 1 boss, 2 pack) and its type
stock lsg_wave_kind()
{
	new t[32]
	return ls_components_running() ? ls_wave_kind(t, charsmax(t)) : 0
}

stock lsg_wave_type()
{
	new t[32]
	if (ls_components_running())
		ls_wave_kind(t, charsmax(t))
	return t
}

// a drop-in over an entity's place
stock lsg_drop_in(id, ent)
{
	new Float:o[3]
	if (!lsg_ent_origin(ent, o))
		return ls_drop_in(id)
	return ls_drop_in(id, o)
}

stock Float:lsg_ehp(e)
	return e > MaxClients && pev_valid(e) ? entity_get_float(e, EV_FL_health) : 0.0

// ---- bot hints (ls_mode, through gm_components) ----
stock bool:lsg_hint(const line[])
	return ls_components_running() && ls_comp_bots_hint(line) != 0

// who a hint is for: 0 all, 1 ct, 2 t, 3 a player, 4 @tag
stock lsg_who(code, p, const tag[])
{
	new w[40]
	switch (code)
	{
		case 1: copy(w, charsmax(w), "ct")
		case 2: copy(w, charsmax(w), "t")
		case 3: formatex(w, charsmax(w), "%d", lsg_ok(p) ? p : 0)
		case 4: formatex(w, charsmax(w), "@%s", tag)
		default: copy(w, charsmax(w), "all")
	}
	return w
}

// an entity's place as "x y z" (a brush entity: its middle)
stock lsg_ptext(ent)
{
	new t[48], Float:o[3]
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
		{
			new Float:mins[3], Float:maxs[3]
			pev(ent, pev_absmin, mins)
			pev(ent, pev_absmax, maxs)
			for (new k = 0; k < 3; k++)
				o[k] = (mins[k] + maxs[k]) / 2.0
		}
	}
	formatex(t, charsmax(t), "%.0f %.0f %.0f", o[0], o[1], o[2])
	return t
}

// the zone's next move: 0 seconds (-1 none), 1 the next radius, 2 closing now
stock Float:lsg_zone_next(ent, what)
{
	if (!ls_components_running() || ent <= 0)
		return what == 0 ? -1.0 : 0.0
	new Float:c[3], Float:r, closing
	new Float:secs = ls_zone_next(ent, c, r, closing)
	switch (what)
	{
		case 1: return r
		case 2: return closing ? 1.0 : 0.0
	}
	return secs
}

// is ent a component of this kind ("": any component)? gm_components' natives raise an error on
// another kind, which ends the whole flow; the node checks first and says so instead
stock bool:lsg_kind_ok(ent, const kind[])
{
	new k[24]
	if (ent <= 0 || !ls_components_running() || !ls_comp_kind(ent, k, charsmax(k)))
		return false
	return !kind[0] || equal(k, kind)
}

// the safe zone a Zone pin means: a Safe zone itself, or the one a Shrinking zone drives (0: neither)
stock lsg_zone_of(ent)
{
	new k[24], name[32], tn[32], z
	if (ent <= 0 || !ls_comp_kind(ent, k, charsmax(k)))
		return 0
	if (equal(k, "ls_safe_zone"))
		return ent
	if (!equal(k, "ls_zone_shrinker"))
		return 0
	ls_comp_get(ent, "zone", name, charsmax(name))
	while ((z = ls_comp_find("ls_safe_zone", z)) > 0)
	{
		pev(z, pev_targetname, tn, charsmax(tn))
		if (!name[0] || equal(tn, name))
			return z
	}
	return 0
}

// counted now (the brain's ls_creature_alive is only brought up to date a moment later); once a
// server frame (one scan however often it's read), again after the mode makes a creature
stock Float:g_lsgCrAt = -1.0, g_lsgCrN
stock lsg_creatures_alive()
{
	if (get_gametime() == g_lsgCrAt)
		return g_lsgCrN
	new n, e = MaxClients, Float:hp
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		pev(e, pev_health, hp)
		if (pev(e, pev_deadflag) == DEAD_NO && hp > 0.0)
			n++
	}
	g_lsgCrAt = get_gametime()
	g_lsgCrN = n
	return n
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

// a bot's nature as the bot brain plays it (W2: pev_vuser2 = skill, aggression, teamwork 0-1;
// pev_vuser3 = greed 0-1, playstyle, trait bits); people read 0
stock Float:lsg_bot_nat(id, which)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return 0.0
	new Float:v[3]
	pev(id, which == 3 ? pev_vuser3 : pev_vuser2, v)
	return floatclamp((which == 3 ? v[0] : v[which]) * 100.0, 0.0, 100.0)
}

stock bool:lsg_bot_trait(id, bit)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return false
	new Float:v[3]
	pev(id, pev_vuser3, v)
	return (floatround(v[2]) & bit) != 0
}

stock lsg_bot_style(id)
{
	static const S[][] = { "rusher", "roamer", "lurker", "careful", "camper", "awper", "knifer", "nader", "ninja", "objective", "weirdgun" }
	new t[16]
	if (lsg_ok(id) && is_user_bot(id))
	{
		new Float:v[3]
		pev(id, pev_vuser3, v)
		new k = floatround(v[1])
		if (k >= 0 && k < sizeof S)
			copy(t, charsmax(t), S[k])
	}
	return t
}

// a follow with a personality influence (0-100: 50 as usual; W2's ls_mode_ver 2)
stock lsg_bot_follow(bot, who, secs, influence)
{
	if (ls_is_bot(bot) && is_user_connected(who))
		server_cmd("ls_bot %d follow %d %d %d", bot, who, secs, influence)
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

// Drop Weapon: a person drops it; a bot loses it (a bot's "drop" can crash this game's DLL)
stock lsg_drop(id)
{
	if (!is_user_bot(id))
	{
		engclient_cmd(id, "drop")
		return
	}
	new wid = get_user_weapon(id)
	if (!wid || wid == CSW_KNIFE)
		return
	new name[32]
	get_weaponname(wid, name, charsmax(name))
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", name)) > 0)
	{
		if (pev(ent, pev_owner) != id)
			continue
		ExecuteHamB(Ham_Weapon_RetireWeapon, ent)
		if (ExecuteHamB(Ham_RemovePlayerItem, id, ent))
		{
			set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
			ExecuteHamB(Ham_Item_Kill, ent)
		}
		break
	}
}

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

// a number as text: whole numbers as they are, others with two decimals (0.5, not 1)
stock lsg_ntext(Float:n)
{
	new t[24]
	if (floatfract(n) == 0.0 && floatabs(n) < 1000000.0)
		formatex(t, charsmax(t), "%d", floatround(n))
	else
		formatex(t, charsmax(t), "%.2f", n)
	return t
}

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

// (a scan of every entity: the answer is kept for the rest of the server frame, so a Find in a
// loop or read many times costs one scan; PerfAudit #9)
stock lsg_find(const name[])
{
	static Trie:seen = Invalid_Trie, Float:at = -1.0
	if (!name[0])
		return 0
	new Float:now = get_gametime(), e
	if (seen == Invalid_Trie)
		seen = TrieCreate()
	if (now != at)
	{
		TrieClear(seen)
		at = now
	}
	if (TrieGetCell(seen, name, e) && (!e || pev_valid(e)))
		return e
	e = max(find_ent_by_tname(-1, name), 0)
	TrieSetCell(seen, name, e)
	return e
}

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
// one server command, only an allowed one (graph::cmd_rules: the app checks the same). The
// engine splits words on spaces and on { } ( ) ' : , and drops quotes: none of those may be in
// it, so the first word checked is the one that runs. amx: an AMX Mod X command instead.
stock bool:lsg_cmd_denied(const w[])
{
	static const DENY_WORDS[][] = { "quit", "exit", "restart", "_restart", "changelevel", "changelevel2", "map", "meta", "amxx", "condump", "logaddress" }
	static const DENY_PARTS[][] = { "file", "cfg", "log", "password", "rcon", "downloadurl", "motd", "exec", "dump", "alias", "plugin", "write", "save", "load", "admin", "ban", "sql", "nav", "del", "remove", "reset", "trace", "record", "demo", "map", "list" }
	for (new i = 0; i < sizeof DENY_WORDS; i++)
		if (equal(w, DENY_WORDS[i]))
			return true
	for (new i = 0; i < sizeof DENY_PARTS; i++)
		if (contain(w, DENY_PARTS[i]) >= 0)
			return true
	return equal(w, "logaddress", 10) != 0
}

stock bool:lsg_server_word(const w[])
{
	static const WORDS[][] = { "say", "humans_join_team", "sv_gravity", "sv_airaccelerate", "sv_accelerate", "sv_maxspeed", "sv_friction", "sv_stopspeed", "sv_alltalk", "sv_maxvelocity", "sv_stepsize", "sv_wateraccelerate", "sv_waterfriction", "sv_bounce", "sv_restart", "sv_restartround", "sv_voiceenable", "bot_quota", "bot_quota_mode", "bot_difficulty", "bot_add", "bot_add_t", "bot_add_ct", "bot_kick", "bot_kill", "bot_knives_only", "bot_pistols_only", "bot_snipers_only", "bot_all_weapons", "bot_allow_rogues", "bot_allow_pistols", "bot_allow_shotguns", "bot_allow_sub_machine_guns", "bot_allow_rifles", "bot_allow_machine_guns", "bot_allow_grenades", "bot_allow_snipers", "bot_allow_shield", "bot_join_team", "bot_join_after_player", "bot_chatter", "bot_stop", "bot_zombie", "bot_defer_to_human", "bot_auto_vacate", "bot_walk", "ls_comp_start", "ls_comp_stop", "ls_comp_spawn", "ls_comp_heal", "ls_comp_players" }
	static const PREFIXES[][] = { "mp_" }
	if (lsg_cmd_denied(w))
		return false
	for (new i = 0; i < sizeof WORDS; i++)
		if (equal(w, WORDS[i]))
			return true
	for (new i = 0; i < sizeof PREFIXES; i++)
		if (equal(w, PREFIXES[i], strlen(PREFIXES[i])) && strlen(w) > strlen(PREFIXES[i]))
			return true
	return false
}

stock bool:lsg_safe_cmd(const cmd[], amx)
{
	lsg_cmd_ok = false
	new c[128], w[48], w2[48], n, k
	copy(c, charsmax(c), cmd)
	trim(c)
	if (!c[0] || strlen(c) > 120)
		return false
	for (new i = 0; c[i]; i++)
		if (!isalnum(c[i]) && c[i] != ' ' && c[i] != '_' && c[i] != '.' && c[i] != '-' && c[i] != '+' && c[i] != '!' && c[i] != '?' && c[i] != '@' && c[i] != '#')
			return false
	// the first two words, lower case (a word that doesn't fit is refused: the same whole words
	// as the app's check, WORD_MAX = 47)
	for (k = 0; c[k] == ' '; k++) {}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w); k++)
		w[n++] = tolower(c[k])
	w[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	for (; c[k] == ' '; k++) {}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w2); k++)
		w2[n++] = tolower(c[k])
	w2[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	if (amx)
	{
		static const AMX_WORDS[][] = { "amx_slap", "amx_slay", "amx_kick", "amx_say", "amx_psay", "amx_tsay", "amx_csay", "amx_chat", "amx_cvar", "amx_map", "amx_nextmap" }
		new found
		for (new i = 0; i < sizeof AMX_WORDS; i++)
			if (equal(w, AMX_WORDS[i]))
				found = 1
		if (!found)
			return false
		if (equal(w, "amx_cvar") && !lsg_server_word(w2))
			return false
		if (equal(w, "amx_map") && (!w2[0] || strlen(w2) > 31 || !is_map_valid(w2)))
			return false
	}
	else if (equal(w, "amx_", 4) || !lsg_server_word(w))
		return false
	server_cmd("%s", c)
	lsg_cmd_ok = true
	return true
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
	new t[64], Float:x = lsg_vc(v, 0), Float:y = lsg_vc(v, 1), Float:z = lsg_vc(v, 2)
	// (a short vector, a direction above all, gets two decimals: in whole units one is "0 0 0")
	if (floatabs(x) < 10.0 && floatabs(y) < 10.0 && floatabs(z) < 10.0)
		formatex(t, charsmax(t), "%.2f %.2f %.2f", x, y, z)
	else
		formatex(t, charsmax(t), "%.0f %.0f %.0f", x, y, z)
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

// ---- (vectors) transforms, entities, effects ----
stock lsg_vpev(id, field)
{
	new Float:o[3]
	if (id > 0 && pev_valid(id))
		pev(id, field, o)
	return lsg_vfrom(o)
}

stock lsg_set_transform(id, pos, ang, vel)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new Float:o[3]
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		engfunc(EngFunc_SetOrigin, id, o)
	}
	if (ang >= 0)
	{
		lsg_vto(ang, o)
		set_pev(id, pev_angles, o)
		set_pev(id, pev_v_angle, o)
		set_pev(id, pev_fixangle, 1)
	}
	if (vel >= 0)
	{
		lsg_vto(vel, o)
		set_pev(id, pev_velocity, o)
	}
}

stock lsg_look_at(id, ent, pos)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new Float:t[3], Float:me[3], Float:ofs[3], Float:d[3], Float:ang[3]
	if (ent > 0)
	{
		if (!lsg_ent_origin(ent, t))
			return
	}
	else if (pos >= 0)
		lsg_vto(pos, t)
	else
		return
	pev(id, pev_origin, me)
	pev(id, pev_view_ofs, ofs)
	for (new k = 0; k < 3; k++)
		d[k] = t[k] - (me[k] + ofs[k])
	vector_to_angle(d, ang)
	ang[0] = -ang[0]
	set_pev(id, pev_angles, ang)
	set_pev(id, pev_v_angle, ang)
	set_pev(id, pev_fixangle, 1)
}

stock lsg_find_all(byname, const name[])
{
	new a = lsg_atmp(), e = -1, n
	while (n < 512 && (e = engfunc(EngFunc_FindEntityByString, e, byname ? "targetname" : "classname", name)) > 0)
	{
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_ent_pos(e, v)
{
	if (e > MaxClients && pev_valid(e))
	{
		new Float:o[3]
		lsg_vto(v, o)
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

stock lsg_ent_angles(e, v)
{
	if (e > MaxClients && pev_valid(e))
	{
		new Float:o[3]
		lsg_vto(v, o)
		set_pev(e, pev_angles, o)
	}
}

stock lsg_explode(v, Float:dmg, Float:radius)
{
	new Float:o[3]
	lsg_vto(v, o)
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_EXPLOSION)
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
	write_short(g_lsgSprExp)
	write_byte(30)
	write_byte(15)
	write_byte(0)
	message_end()
	if (dmg <= 0.0)
		return
	new Float:p[3]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id))
			continue
		pev(id, pev_origin, p)
		new Float:d = get_distance_f(o, p)
		if (d < radius)
			ExecuteHamB(Ham_TakeDamage, id, 0, 0, dmg * (1.0 - d / radius), DMG_BLAST)
	}
}

stock lsg_beam(a, b, r, g, bl, Float:secs)
{
	new Float:x[3], Float:y[3]
	lsg_vto(a, x)
	lsg_vto(b, y)
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMPOINTS)
	engfunc(EngFunc_WriteCoord, x[0])
	engfunc(EngFunc_WriteCoord, x[1])
	engfunc(EngFunc_WriteCoord, x[2])
	engfunc(EngFunc_WriteCoord, y[0])
	engfunc(EngFunc_WriteCoord, y[1])
	engfunc(EngFunc_WriteCoord, y[2])
	write_short(g_lsgSprBeam)
	write_byte(0)
	write_byte(0)
	write_byte(clamp(floatround(secs * 10.0), 1, 255))
	write_byte(20)
	write_byte(0)
	write_byte(r)
	write_byte(g)
	write_byte(bl)
	write_byte(200)
	write_byte(0)
	message_end()
}

stock lsg_sound_at(v, const snd[])
{
	// (a sound that wasn't there when the map loaded: nothing)
	if (!lsg_have(snd))
		return
	new Float:o[3]
	lsg_vto(v, o)
	engfunc(EngFunc_EmitAmbientSound, 0, o, snd, 1.0, ATTN_NORM, 0, PITCH_NORM)
}

// Timelines: smooth in and out between two keys (0..1 -> 0..1)
stock Float:lsg_tl_ease(Float:x)
{
	x = floatclamp(x, 0.0, 1.0)
	return x * x * (3.0 - 2.0 * x)
}

// an entity's rotation (a player's view angles)
stock lsg_erot(e)
	return lsg_vpev(e, e >= 1 && e <= MaxClients ? pev_v_angle : pev_angles)

// the middle of an entity's box
stock lsg_ecenter(e)
{
	new Float:mn[3], Float:mx[3], Float:c[3]
	if (e > 0 && pev_valid(e))
	{
		pev(e, pev_absmin, mn)
		pev(e, pev_absmax, mx)
		for (new k = 0; k < 3; k++)
			c[k] = (mn[k] + mx[k]) / 2.0
	}
	return lsg_vfrom(c)
}


// enum Class
stock const g_lsgE0[][] = { "Scout", "Tank", "Medic" }
// enum Entity Kind
stock const g_lsgE1[][] = { "Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None" }
// enum Component Kind
stock const g_lsgE2[][] = { "Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None" }
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
	ctx[23] = _:a0
	ctx[24] = _:a1
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
	ctx[25] = _:a0
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

lsd_graph_set(const vid[], who, const val[])
{
	if (equal(vid, "kills"))
	{
		g_pv0[clamp(who, 1, 32)] = str_to_float(val)
		return true
	}
	if (equal(vid, "round"))
	{
		g_v1 = str_to_float(val)
		return true
	}
	if (equal(vid, "flag"))
	{
		g_v2 = str_to_num(val) ? 1 : 0
		return true
	}
	if (equal(vid, "last"))
	{
		copy(g_v3, 63, val)
		return true
	}
	if (equal(vid, "tscore"))
	{
		g_tv4[clamp(who, 1, 2)] = str_to_float(val)
		return true
	}
	if (equal(vid, "phase"))
	{
		copy(g_v5, 63, val)
		return true
	}
	if (equal(vid, "vcls"))
	{
		g_pv16[clamp(who, 1, 32)] = str_to_num(val)
		return true
	}
	if (equal(vid, "lsum"))
	{
		g_v18 = str_to_float(val)
		return true
	}
	return false
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
// the two bots sent to each downed player
stock g_lsgRevBots[33][2]

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
	// (a use goal on the spot, not on the player: the brain keeps a use goal on a player only while
	// they're alive and down gm_components' way, and this one is dead; on the spot it stays by them)
	for (new i = 0; i < 2; i++)
	{
		g_lsgRevBots[victim][i] = near[i]
		if (near[i])
			server_cmd("ls_mode goal %d use %.0f %.0f %.0f 0 75 %d 64", near[i], g_lsgDownOrg[victim][0], g_lsgDownOrg[victim][1], g_lsgDownOrg[victim][2], left)
	}
}

stock lsg_clear_down(p)
{
	if (g_lsgDown[p] && get_cvar_pointer("ls_mode_ver"))
		for (new i = 0; i < 2; i++)
			if (g_lsgRevBots[p][i] && is_user_connected(g_lsgRevBots[p][i]))
				server_cmd("ls_mode goal_clear %d use", g_lsgRevBots[p][i])
	g_lsgRevBots[p][0] = 0
	g_lsgRevBots[p][1] = 0
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
	// written within 5 s (a timer), at a new round, when the player leaves and at map end: not
	// the whole file on every call (PerfAudit #6: about 1 ms a write)
	g_lsgSavedDirty = true
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

// ---- map markers ----
#define LSG_MK_MAX 128
new const g_lsgMkIds[][] = { "spot", "route" }
new g_lsgMkN, g_lsgMkEnt[LSG_MK_MAX], g_lsgMkKind[LSG_MK_MAX], g_lsgMkPath[LSG_MK_MAX], g_lsgMkOrder[LSG_MK_MAX], Trie:g_lsgMkKeys[LSG_MK_MAX]
new g_lsgMkParse, bool:g_lsgMkPend, Trie:g_lsgMkPendKeys = Invalid_Trie

public lsg_mk_kv(ent, kvd)
{
	if (ent <= 0)
		return FMRES_IGNORED
	new key[32], val[64]
	get_kvd(kvd, KV_KeyName, key, charsmax(key))
	get_kvd(kvd, KV_Value, val, charsmax(val))
	if (equal(key, "classname"))
	{
		// the engine sends the class first on its own, then again among the keys
		if (ent != g_lsgMkParse)
		{
			g_lsgMkParse = ent
			g_lsgMkPend = false
			if (g_lsgMkPendKeys != Invalid_Trie)
				TrieDestroy(g_lsgMkPendKeys)
			g_lsgMkPendKeys = Invalid_Trie
			// (another plugin may have made an ls_marker an info_target already: its kind decides)
			if (equal(val, "ls_marker") || equal(val, "info_target"))
			{
				g_lsgMkPend = true
				g_lsgMkPendKeys = TrieCreate()
			}
		}
		if (g_lsgMkPend && equal(val, "ls_marker"))
			set_kvd(kvd, KV_Value, "info_target")
		return FMRES_IGNORED
	}
	if (g_lsgMkPend && ent == g_lsgMkParse)
		TrieSetString(g_lsgMkPendKeys, key, val)
	return FMRES_IGNORED
}

public lsg_mk_spawn(ent)
{
	if (ent != g_lsgMkParse || !g_lsgMkPend)
		return FMRES_IGNORED
	g_lsgMkPend = false
	g_lsgMkParse = 0
	new Trie:keys = g_lsgMkPendKeys
	g_lsgMkPendKeys = Invalid_Trie
	new kind[32], mode[32], num[12], k = -1
	TrieGetString(keys, "kind", kind, charsmax(kind))
	TrieGetString(keys, "mode", mode, charsmax(mode))
	for (new i = 0; i < sizeof g_lsgMkIds; i++)
	{
		if (kind[0] && equal(kind, g_lsgMkIds[i]))
		{
			k = i
			break
		}
	}
	if (k < 0 || !lsg_mk_mode_ok(mode) || g_lsgMkN >= LSG_MK_MAX || !pev_valid(ent))
	{
		TrieDestroy(keys)
		return FMRES_IGNORED
	}
	set_pev(ent, pev_effects, pev(ent, pev_effects) | EF_NODRAW)
	set_pev(ent, pev_solid, SOLID_NOT)
	new path, order
	if (TrieGetString(keys, "path", num, charsmax(num)))
		path = str_to_num(num)
	if (TrieGetString(keys, "order", num, charsmax(num)))
		order = str_to_num(num)
	// in kind, path, order order (then the map's)
	new at = g_lsgMkN
	while (at > 0 && (g_lsgMkKind[at - 1] > k || (g_lsgMkKind[at - 1] == k && (g_lsgMkPath[at - 1] > path || (g_lsgMkPath[at - 1] == path && g_lsgMkOrder[at - 1] > order)))))
	{
		g_lsgMkEnt[at] = g_lsgMkEnt[at - 1]
		g_lsgMkKind[at] = g_lsgMkKind[at - 1]
		g_lsgMkPath[at] = g_lsgMkPath[at - 1]
		g_lsgMkOrder[at] = g_lsgMkOrder[at - 1]
		g_lsgMkKeys[at] = g_lsgMkKeys[at - 1]
		at--
	}
	g_lsgMkEnt[at] = ent
	g_lsgMkKind[at] = k
	g_lsgMkPath[at] = path
	g_lsgMkOrder[at] = order
	g_lsgMkKeys[at] = keys
	g_lsgMkN++
	return FMRES_IGNORED
}

// a marker's mode: empty, "any", this mode's id, or a comma list with it
stock bool:lsg_mk_mode_ok(const mode[])
{
	if (!mode[0] || equal(mode, "any") || equal(mode, LSG_MODE))
		return true
	new list[40], me[32]
	formatex(list, charsmax(list), ",%s,", mode)
	replace_all(list, charsmax(list), " ", "")
	formatex(me, charsmax(me), ",%s,", LSG_MODE)
	return contain(list, me) >= 0 || contain(list, ",any,") >= 0
}

stock lsg_mk_count(k)
{
	new n
	for (new m = 0; m < g_lsgMkN; m++)
		if (g_lsgMkKind[m] == k)
			n++
	return n
}

// the n-th (from 1) marker of a kind, or 0
stock lsg_mk_get(k, n)
{
	for (new m = 0; m < g_lsgMkN; m++)
		if (g_lsgMkKind[m] == k && --n == 0)
			return pev_valid(g_lsgMkEnt[m]) ? g_lsgMkEnt[m] : 0
	return 0
}

stock lsg_mk_key(ent, const key[])
{
	new t[64]
	for (new m = 0; m < g_lsgMkN; m++)
	{
		if (g_lsgMkEnt[m] != ent || !ent)
			continue
		if (!TrieGetString(g_lsgMkKeys[m], key, t, charsmax(t)))
			lsg_mk_default(g_lsgMkKind[m], key, t)
		break
	}
	return t
}

stock lsg_mk_default(k, const key[], t[64])
{
	if (k == 0 && equal(key, "wave"))
		copy(t, charsmax(t), "2")
	#pragma unused k
	#pragma unused key
	#pragma unused t
}

// a creature with the mode's own health, tier and pay for its killer
new g_lsgPay[2048]
stock lsg_creature_x(const kind[], const at[], tier, Float:hp, pay)
{
	new Float:o[3]
	if (!get_cvar_pointer("ls_creature_ver") || !lsg_origin_of(at, o))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	if (tier > 0)
	{
		new v[4]
		num_to_str(tier, v, charsmax(v))
		DispatchKeyValue(ent, "tier", v)
	}
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	if (!is_valid_ent(ent) || (entity_get_int(ent, EV_INT_flags) & FL_KILLME))
		return 0
	if (hp > 0.0)
	{
		entity_set_float(ent, EV_FL_health, hp)
		entity_set_float(ent, EV_FL_max_health, hp)
	}
	if (ent < sizeof g_lsgPay)
		g_lsgPay[ent] = clamp(pay, 0, 16000)
	// (Creatures Alive counts again)
	g_lsgCrAt = -1.0
	return ent
}

public lsg_pay_killer(ent, killer)
{
	if (ent > 0 && ent < sizeof g_lsgPay && g_lsgPay[ent])
	{
		if (lsg_ok(killer))
			cs_set_user_money(killer, min(cs_get_user_money(killer) + g_lsgPay[ent], 16000))
		g_lsgPay[ent] = 0
	}
	return HAM_IGNORED
}

public lsg_count_round()
	g_lsgRounds++

public plugin_precache()
{
	// the map's ls_marker entities, as they load
	register_forward(FM_KeyValue, "lsg_mk_kv")
	register_forward(FM_Spawn, "lsg_mk_spawn", 1)
	// creature types this mode spawns: loaded as the map starts (with what other plugins asked for)
	if (get_cvar_pointer("ls_creature_precache"))
		lsg_precache_types("all")
}

stock lsg_precache_types(const want[])
{
	new cur[256], w[32], rest[256], padded[280], pw[36]
	get_cvar_string("ls_creature_precache", cur, charsmax(cur))
	if (equali(cur, "all"))
		return
	if (equali(want, "all"))
	{
		set_cvar_string("ls_creature_precache", "all")
		return
	}
	copy(rest, charsmax(rest), want)
	while (rest[0])
	{
		strtok(rest, w, charsmax(w), rest, charsmax(rest), ' ')
		formatex(padded, charsmax(padded), " %s ", cur)
		formatex(pw, charsmax(pw), " %s ", w)
		if (w[0] && containi(padded, pw) < 0)
		{
			if (cur[0])
				add(cur, charsmax(cur), " ")
			add(cur, charsmax(cur), w)
		}
	}
	set_cvar_string("ls_creature_precache", cur)
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node", "1.0", "Game Mode Studio")
	lsd_init()
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	register_event("HLTV", "lsg_count_round", "a", "1=0", "2=0")
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
	RegisterHamPlayer(Ham_TakeDamage, "lsg_hurt_before", 0)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	register_event("TeamInfo", "lsg_on_team_changed", "a")
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_on_new_round", "a", "1=0", "2=0")
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_on_creature_killed", 1)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_pre_damage", 0)
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_pay_killer", 1)
	set_task(0.25, "lsg_revive_tick", LSG_TASK - 1, _, _, "b")
	set_task(1.0, "lsg_clock_tick", LSG_TASK - 2, _, _, "b")
	set_task(5.0, "lsg_saved_flush", LSG_TASK - 3, _, _, "b")
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
	register_clcmd("say", "lsg_chat92")
	register_clcmd("say_team", "lsg_chat92")
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
	new ctx[LSG_CTX], s[64], w[40]
	read_args(s, charsmax(s))
	remove_quotes(s)
	trim(s)
	copyc(w, charsmax(w), s, ' ')
	if ((w[0] != '/' && w[0] != '!') || !equali(w[1], "shop"))
		return PLUGIN_CONTINUE
	ctx[0] = id // player
	copy(g_lsgTxt[0], 63, s) // args
	lsg_after_word(g_lsgTxt[0])
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
	ctx[1] = _:float(g_lsgRounds) // round
	lsg_n36(ctx)
}

public lsg_on_player_killed()
{
	new v = read_data(2), a = read_data(1), wpn[24]
	read_data(4, wpn, charsmax(wpn))
	new ctx[LSG_CTX]
	ctx[0] = v // victim
	ctx[1] = a // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	copy(g_lsgTxt[0], 63, wpn) // weapon
	ctx[3] = _:lsg_dist(a, v) // distance
	ctx[4] = a != v && lsg_ok(a) && lsg_ok(v) && get_user_team(a) == get_user_team(v) ? 1 : 0 // team_kill
	ctx[5] = a == v ? 1 : 0 // suicide
	ctx[6] = lsg_ok(a) ? 0 : 1 // world
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

new Float:g_lsgHurtHp[33]

public lsg_hurt_before(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused attacker
	#pragma unused damage
	#pragma unused bits
	if (victim >= 1 && victim <= MaxClients)
		pev(victim, pev_health, g_lsgHurtHp[victim])
	return HAM_IGNORED
}

public lsg_on_player_hurt(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused bits
	if (victim < 1 || victim > MaxClients)
		return HAM_IGNORED
	new Float:hp
	pev(victim, pev_health, hp)
	damage = g_lsgHurtHp[victim] - hp
	if (damage <= 0.0)
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	ctx[3] = bits & DMG_BULLET && get_ent_data(victim, "CBaseMonster", "m_LastHitGroup") == HIT_HEAD ? 1 : 0 // headshot
	ctx[4] = _:hp // left
	copy(g_lsgTxt[0], 63, lsg_hitgroup(victim, bits)) // hitgroup
	copy(g_lsgTxt[1], 63, lsg_wpn_of(attacker, inflictor)) // weapon
	lsg_n77(ctx)
	return HAM_IGNORED
}

public lsg_on_team_changed()
{
	static last[33]
	new id = read_data(1), tn[12]
	read_data(2, tn, charsmax(tn))
	if (id < 1 || id > MaxClients)
		return
	if (tn[0] == 'U' || !tn[0])
	{
		last[id] = 0
		return
	}
	new team = tn[0] == 'T' ? 1 : (tn[0] == 'C' ? 2 : 3)
	if (team == last[id])
		return
	last[id] = team
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = team == 3 ? 0 : team // team
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
	if (lsd_want(0))
	{
	lsd_begin()
	}
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n1(ctx)
}

// Set Variable (rs_set)
lsg_n1(ctx[])
{
	new Float:lsi2 = ((g_v1) + (1.0))
	if (lsd_want(1))
	{
	lsd_begin()
	lsd_in_number("Value", lsi2)
	}
	if (lsd_enter(1, ctx))
		return
	g_v1 = lsi2
	lsd_exit(1, fmt("%.2f", g_v1))
	lsg_n2(ctx)
}

// Countdown (rs_cd)
lsg_n2(ctx[])
{
	if (lsd_want(2))
	{
	lsd_begin()
	}
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
	if (lsd_timer_wait(2, "lsg_t2", data))
		return
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
	if (lsd_want(3))
	{
	lsd_begin()
	}
	if (lsd_enter(3, ctx))
		return
	ls_announce(0, 2.0, "%s", "Go!")
	lsd_exit(3, "shown")
}

// Repeat (rs_rep)
lsg_n4(ctx[])
{
	new Float:lsi0 = 10.0
	if (lsd_want(4))
	{
	lsd_begin()
	lsd_in_number("Every (s)", lsi0)
	}
	if (lsd_enter(4, ctx))
		return
	remove_task(LSG_TASK + 4)
	g_lsgLeft[4] = 3
	set_task(floatmax(lsi0, 0.1), "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
	lsd_exit(4, "started")
	lsg_n6(ctx)
}

public lsg_t4(const data[], task)
{
	if (lsd_timer_wait(4, "lsg_t4", data))
		return
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
	new Float:lsi1 = float(ls_count_alive(CsTeams:(0)))
	if (lsd_want(5))
	{
	lsd_begin()
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(5, ctx))
		return
	ls_hud(0, LsHud:2, 5.0, "%s %s", "Alive:", lsg_ntext(lsi1))
	lsd_exit(5, "shown")
}

// Delay (rs_delay)
lsg_n6(ctx[])
{
	new Float:lsi0 = 5.0
	if (lsd_want(6))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(6, ctx))
		return
	set_task(floatmax(lsi0, 0.1), "lsg_t6", LSG_TASK + 6, ctx, LSG_CTX)
	lsd_exit(6, "waiting")
}

public lsg_t6(const data[], task)
{
	if (lsd_timer_wait(6, "lsg_t6", data))
		return
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
	new Float:lsi0 = 0.0
	if (lsd_want(7))
	{
	lsd_begin()
	lsd_in_number("Wave", lsi0)
	}
	if (lsd_enter(7, ctx))
		return
	if (!ls_components_running())
		lsd_error(7, "gm_components isn't running on this server.")
	else
	{
		ls_wave_start(max(floatround(lsi0), 0))
		lsd_exit(7, "done")
	}
	lsg_n8(ctx)
}

// Stop Waves (rs_wstop)
lsg_n8(ctx[])
{
	if (lsd_want(8))
	{
	lsd_begin()
	}
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
	if (lsd_want(9))
	{
	lsd_begin()
	}
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
	if (lsd_want(10))
	{
	lsd_begin()
	}
	if (lsd_enter(10, ctx))
		return
	lsd_exit(10, "ran")
	lsg_n11(ctx)
}

// Branch (pk_br)
lsg_n11(ctx[])
{
	new lsi0 = ctx[2]
	if (lsd_want(11))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(11, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(11, g_lsdBool[ok])
	if (ok)
		lsg_n12(ctx)
	else
		lsg_n13(ctx)
}

// Add To Variable (pk_add)
lsg_n12(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi2 = 2.0
	if (lsd_want(12))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi2)
	}
	if (lsd_enter(12, ctx))
		return
	g_pv0[lsg_slot(lsi0)] += lsi2
	lsd_exit(12, fmt("%.2f", g_pv0[lsg_slot(lsi0)]))
	lsg_n14(ctx)
}

// Add To Variable (pk_add2)
lsg_n13(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi2 = lsg_random(1.0, 3.0, 0)
	if (lsd_want(13))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi2)
	}
	if (lsd_enter(13, ctx))
		return
	g_pv0[lsg_slot(lsi0)] += lsi2
	lsd_exit(13, fmt("%.2f", g_pv0[lsg_slot(lsi0)]))
}

// Is Team (pk_team)
lsg_n14(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1 = 2
	if (lsd_want(14))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_team("Team", lsi1)
	}
	if (lsd_enter(14, ctx))
		return
	new ok = ((lsg_team(lsi0) == lsi1) ? 1 : 0)
	lsd_exit(14, g_lsdBool[ok])
	if (ok)
		lsg_n15(ctx)
	else
		lsg_n17(ctx)
}

// Give Weapon (pk_give)
lsg_n15(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi2 = 60.0
	if (lsd_want(15))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Spare bullets", lsi2)
	}
	if (lsd_enter(15, ctx))
		return
	if (ls_give_weapon(lsi0, "m4a1", 60))
		lsd_exit(15, "given")
	else
		lsd_error(15, "Not given: that player is dead or already has one.")
	lsg_n16(ctx)
}

// Set Health (pk_seth)
lsg_n16(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi1 = ((lsg_health(ctx[1])) + (10.0))
	if (lsd_want(16))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Health", lsi1)
	}
	if (lsd_enter(16, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(16, "Nothing done: that player isn't alive.")
	else
	{
		ls_set_health(lsi0, max(floatround(lsi1), 1))
		lsd_exit(16, "set")
	}
}

// Show HUD Text (pk_hud)
lsg_n17(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1[128]
	copy(lsi1, 127, lsg_name(ctx[0]))
	if (lsd_want(17))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Value", lsi1)
	}
	if (lsd_enter(17, ctx))
		return
	ls_hud(lsi0, LsHud:3, 4.0, "%s %s", "You got", lsi1)
	lsd_exit(17, "shown")
}

// Player Spawned (ps)
lsg_n18(ctx[])
{
	if (lsd_want(18))
	{
	lsd_begin()
	}
	if (lsd_enter(18, ctx))
		return
	lsd_exit(18, "ran")
	lsg_n19(ctx)
}

// Strip Weapons (ps_strip)
lsg_n19(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(19))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(19, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(19, "Nothing done: that player isn't alive.")
	else
	{
		ls_strip(lsi0, true)
		lsd_exit(19, "stripped")
	}
	lsg_n20(ctx)
}

// Set Armor (ps_arm)
lsg_n20(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 50.0
	if (lsd_want(20))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Armor", lsi1)
	}
	if (lsd_enter(20, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(20, "Nothing done: that player isn't alive.")
	else
	{
		ls_set_armor(lsi0, max(floatround(lsi1), 0), true)
		lsd_exit(20, "set")
	}
	lsg_n21(ctx)
}

// Set Speed (ps_spd)
lsg_n21(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 300.0
	if (lsd_want(21))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Speed", lsi1)
	}
	if (lsd_enter(21, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(21, "Nothing done: that player isn't alive.")
	else
	{
		set_user_maxspeed(lsi0, lsi1)
		lsd_exit(21, "set")
	}
	lsg_n22(ctx)
}

// Set Team (ps_team)
lsg_n22(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = 1
	if (lsd_want(22))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_team("Team", lsi1)
	}
	if (lsd_enter(22, ctx))
		return
	if (lsi0 < 1 || lsi0 > MaxClients || !is_user_connected(lsi0))
		lsd_error(22, "Nothing done: nobody there.")
	else
	{
		ls_set_team(lsi0, CsTeams:(lsi1))
		lsd_exit(22, "moved")
	}
	lsg_n23(ctx)
}

// Teleport (ps_tp)
lsg_n23(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(23))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(23, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(23, "Not moved: that player isn't alive.")
	else if (!lsg_teleport(lsi0, "spawn_a"))
		lsd_error(23, "Not moved: no such entity on this map.")
	else
		lsd_exit(23, "teleported")
	lsg_n24(ctx)
}

// Set Variable (ps_set)
lsg_n24(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi2 = 0.0
	if (lsd_want(24))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Value", lsi2)
	}
	if (lsd_enter(24, ctx))
		return
	g_pv0[lsg_slot(lsi0)] = lsi2
	lsd_exit(24, fmt("%.2f", g_pv0[lsg_slot(lsi0)]))
}

// Player Joined (pj)
lsg_n25(ctx[])
{
	if (lsd_want(25))
	{
	lsd_begin()
	}
	if (lsd_enter(25, ctx))
		return
	lsd_exit(25, "ran")
	lsg_n26(ctx)
}

// Show HUD Text (pj_hud)
lsg_n26(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = lsg_is_bot(ctx[0])
	if (lsd_want(26))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_bool("Value", lsi1)
	}
	if (lsd_enter(26, ctx))
		return
	ls_hud(lsi0, LsHud:1, 4.0, "%s %s", "Bot:", g_lsgYesNo[(lsi1) ? 1 : 0])
	lsd_exit(26, "shown")
	lsg_n27(ctx)
}

// Branch (pj_br)
lsg_n27(ctx[])
{
	new lsi0 = (ls_alive(ctx[0]) ? 1 : 0)
	if (lsd_want(27))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(27, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(27, g_lsdBool[ok])
}

// Player Left (pl)
lsg_n28(ctx[])
{
	if (lsd_want(28))
	{
	lsd_begin()
	}
	if (lsd_enter(28, ctx))
		return
	lsd_exit(28, "ran")
	lsg_n29(ctx)
}

// Set Variable (pl_set)
lsg_n29(ctx[])
{
	new lsi2[128]
	copy(lsi2, 127, "someone left")
	if (lsd_want(29))
	{
	lsd_begin()
	lsd_in_text("Value", lsi2)
	}
	if (lsd_enter(29, ctx))
		return
	copy(g_v3, 63, lsi2)
	lsd_exit(29, "set")
}

// Bomb Planted (bp)
lsg_n30(ctx[])
{
	if (lsd_want(30))
	{
	lsd_begin()
	}
	if (lsd_enter(30, ctx))
		return
	lsd_exit(30, "ran")
	lsg_n31(ctx)
}

// Show HUD Text (bp_hud)
lsg_n31(ctx[])
{
	new lsi1 = lsg_team(ctx[0])
	if (lsd_want(31))
	{
	lsd_begin()
	lsd_in_team("Value", lsi1)
	}
	if (lsd_enter(31, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Planted by", g_lsgTeamName[clamp(lsi1, 0, 3)])
	lsd_exit(31, "shown")
}

// Bomb Defused (bd)
lsg_n32(ctx[])
{
	if (lsd_want(32))
	{
	lsd_begin()
	}
	if (lsd_enter(32, ctx))
		return
	lsd_exit(32, "ran")
	lsg_n33(ctx)
}

// End Round (bd_end)
lsg_n33(ctx[])
{
	new lsi0 = 2
	if (lsd_want(33))
	{
	lsd_begin()
	lsd_in_team("Winner", lsi0)
	}
	if (lsd_enter(33, ctx))
		return
	lsg_end_round(lsi0)
	lsd_exit(33, "ended")
}

// Bomb Exploded (bx)
lsg_n34(ctx[])
{
	if (lsd_want(34))
	{
	lsd_begin()
	}
	if (lsd_enter(34, ctx))
		return
	lsd_exit(34, "ran")
	lsg_n35(ctx)
}

// End Round (bx_end)
lsg_n35(ctx[])
{
	new lsi0 = 1
	if (lsd_want(35))
	{
	lsd_begin()
	lsd_in_team("Winner", lsi0)
	}
	if (lsd_enter(35, ctx))
		return
	lsg_end_round(lsi0)
	lsd_exit(35, "ended")
}

// Round Ended (re)
lsg_n36(ctx[])
{
	if (lsd_want(36))
	{
	lsd_begin()
	}
	if (lsd_enter(36, ctx))
		return
	lsd_exit(36, "ran")
	lsg_n37(ctx)
}

// Branch (re_br)
lsg_n37(ctx[])
{
	new lsi0 = ((((float(ls_count_alive(CsTeams:(ctx[0])))) > (0.0) ? 1 : 0)) && (((g_v2) ? 0 : 1)) ? 1 : 0)
	if (lsd_want(37))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(37, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(37, g_lsdBool[ok])
	if (ok)
		lsg_n38(ctx)
	else
		lsg_n39(ctx)
}

// Set Variable (re_set)
lsg_n38(ctx[])
{
	new lsi2 = 1
	if (lsd_want(38))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi2)
	}
	if (lsd_enter(38, ctx))
		return
	g_v2 = ((lsi2) ? 1 : 0)
	lsd_exit(38, g_lsdBool[g_v2 ? 1 : 0])
}

// Stop Timers (re_stop)
lsg_n39(ctx[])
{
	if (lsd_want(39))
	{
	lsd_begin()
	}
	if (lsd_enter(39, ctx))
		return
	lsg_stop_timers()
	lsd_exit(39, "stopped")
}

// Player Used (pu)
lsg_n40(ctx[])
{
	if (lsd_want(40))
	{
	lsd_begin()
	}
	if (lsd_enter(40, ctx))
		return
	lsd_exit(40, "ran")
	lsg_n41(ctx)
}

// Door (pu_door)
lsg_n41(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(41))
	{
	lsd_begin()
	lsd_in_entity("Door", lsi0)
	}
	if (lsd_enter(41, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, "ls_timed_door"))
		lsd_error(41, "Not done: that isn't a Timed door (or gm_components isn't running).")
	else
	{
		ls_door_open(lsi0)
		lsd_exit(41, "done")
	}
	lsg_n42(ctx)
}

// Fire Trap (pu_trap)
lsg_n42(ctx[])
{
	new lsi0 = lsg_find("trap1")
	if (lsd_want(42))
	{
	lsd_begin()
	lsd_in_entity("Trap", lsi0)
	}
	if (lsd_enter(42, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, "ls_trigger_trap"))
		lsd_error(42, "Not done: that isn't a Trap (or gm_components isn't running).")
	else
	{
		ls_trap_fire(lsi0)
		lsd_exit(42, "done")
	}
}

// Player Touched (pt)
lsg_n43(ctx[])
{
	if (lsd_want(43))
	{
	lsd_begin()
	}
	if (lsd_enter(43, ctx))
		return
	lsd_exit(43, "ran")
	lsg_n44(ctx)
}

// Open Crate (pt_crate)
lsg_n44(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1 = ctx[0]
	if (lsd_want(44))
	{
	lsd_begin()
	lsd_in_entity("Container", lsi0)
	lsd_in_player("Player", lsi1)
	}
	if (lsd_enter(44, ctx))
		return
	if (!ls_components_running())
		lsd_error(44, "gm_components isn't running on this server.")
	else
	{
		ls_loot_open(lsi0, lsi1)
		lsd_exit(44, "done")
	}
}

// Point Captured (cc)
lsg_n45(ctx[])
{
	if (lsd_want(45))
	{
	lsd_begin()
	}
	if (lsd_enter(45, ctx))
		return
	lsd_exit(45, "ran")
	lsg_n46(ctx)
}

// Lock Point (cc_lock)
lsg_n46(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 20.0
	if (lsd_want(46))
	{
	lsd_begin()
	lsd_in_entity("Point", lsi0)
	lsd_in_number("Seconds", lsi1)
	}
	if (lsd_enter(46, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, "ls_capture_point"))
		lsd_error(46, "Not done: that isn't a Capture point (or gm_components isn't running).")
	else
	{
		ls_cp_lock(lsi0, lsi1)
		lsd_exit(46, "done")
	}
	lsg_n47(ctx)
}

// Show HUD Text (cc_hud)
lsg_n47(ctx[])
{
	new lsi1 = ctx[1]
	if (lsd_want(47))
	{
	lsd_begin()
	lsd_in_team("Value", lsi1)
	}
	if (lsd_enter(47, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %s", "Taken by", g_lsgTeamName[clamp(lsi1, 0, 3)])
	lsd_exit(47, "shown")
}

// Point Lost (cl)
lsg_n48(ctx[])
{
	if (lsd_want(48))
	{
	lsd_begin()
	}
	if (lsd_enter(48, ctx))
		return
	lsd_exit(48, "ran")
	lsg_n49(ctx)
}

// Open Extraction (cl_ex)
lsg_n49(ctx[])
{
	new lsi0 = lsg_find("ex1")
	new lsi1 = 1
	if (lsd_want(49))
	{
	lsd_begin()
	lsd_in_entity("Point", lsi0)
	lsd_in_bool("Open", lsi1)
	}
	if (lsd_enter(49, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, "ls_extraction_zone"))
		lsd_error(49, "Not done: that isn't an Extraction zone (or gm_components isn't running).")
	else
	{
		ls_extract_open(lsi0, bool:((lsi1) ? 1 : 0))
		lsd_exit(49, "done")
	}
}

// Zone Stage (zs)
lsg_n50(ctx[])
{
	if (lsd_want(50))
	{
	lsd_begin()
	}
	if (lsd_enter(50, ctx))
		return
	lsd_exit(50, "ran")
	lsg_n51(ctx)
}

// Shrink Zone (zs_shrink)
lsg_n51(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi2 = ((Float:ctx[1]) * (100.0))
	if (lsd_want(51))
	{
	lsd_begin()
	lsd_in_entity("Zone", lsi0)
	lsd_in_number("Radius", lsi2)
	}
	if (lsd_enter(51, ctx))
		return
	new Float:o[3], zz = !ls_components_running() ? 0 : lsg_zone_of(lsi0)
	if (!ls_components_running())
		lsd_error(51, "gm_components isn't running on this server.")
	else if (!zz)
		lsd_error(51, "That isn't a zone: wire a Safe zone or a Shrinking zone into Zone.")
	else if (!lsg_origin_of("mid", o))
		lsd_error(51, "No entity has that name on this map.")
	else
	{
		ls_zone_shrink(zz, o, lsi2, 20.0)
		lsd_exit(51, "done")
	}
}

// Zone Hurt Player (zt)
lsg_n52(ctx[])
{
	if (lsd_want(52))
	{
	lsd_begin()
	}
	if (lsd_enter(52, ctx))
		return
	lsd_exit(52, "ran")
	lsg_n53(ctx)
}

// Set Zone (zt_set)
lsg_n53(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi2 = 900.0
	if (lsd_want(53))
	{
	lsd_begin()
	lsd_in_entity("Zone", lsi0)
	lsd_in_number("Radius", lsi2)
	}
	if (lsd_enter(53, ctx))
		return
	new Float:o[3], zz = !ls_components_running() ? 0 : lsg_zone_of(lsi0)
	if (!ls_components_running())
		lsd_error(53, "gm_components isn't running on this server.")
	else if (!zz)
		lsd_error(53, "That isn't a zone: wire a Safe zone or a Shrinking zone into Zone.")
	else if (!lsg_origin_of("mid", o))
		lsd_error(53, "No entity has that name on this map.")
	else
	{
		ls_zone_set(zz, o, lsi2)
		lsd_exit(53, "done")
	}
	lsg_n54(ctx)
}

// Show HUD Text (zt_hud)
lsg_n54(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi1 = Float:ctx[2]
	if (lsd_want(54))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(54, ctx))
		return
	ls_hud(lsi0, LsHud:3, 4.0, "%s %s", "Ouch", lsg_ntext(lsi1))
	lsd_exit(54, "shown")
}

// Player Extracted (ex)
lsg_n55(ctx[])
{
	if (lsd_want(55))
	{
	lsd_begin()
	}
	if (lsd_enter(55, ctx))
		return
	lsd_exit(55, "ran")
	lsg_n56(ctx)
}

// Add To Variable (ex_add)
lsg_n56(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi2 = 5.0
	if (lsd_want(56))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi2)
	}
	if (lsd_enter(56, ctx))
		return
	g_pv0[lsg_slot(lsi0)] += lsi2
	lsd_exit(56, fmt("%.2f", g_pv0[lsg_slot(lsi0)]))
}

// Wave Started (ws)
lsg_n57(ctx[])
{
	if (lsd_want(57))
	{
	lsd_begin()
	}
	if (lsd_enter(57, ctx))
		return
	lsd_exit(57, "ran")
	lsg_n58(ctx)
}

// Show HUD Text (ws_hud)
lsg_n58(ctx[])
{
	new Float:lsi1 = Float:ctx[0]
	if (lsd_want(58))
	{
	lsd_begin()
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(58, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %s", "Wave", lsg_ntext(lsi1))
	lsd_exit(58, "shown")
}

// Wave Spawned One (wp)
lsg_n59(ctx[])
{
	if (lsd_want(59))
	{
	lsd_begin()
	}
	if (lsd_enter(59, ctx))
		return
	lsd_exit(59, "ran")
	lsg_n60(ctx)
}

// Show HUD Text (wp_hud)
lsg_n60(ctx[])
{
	new lsi1 = ctx[0]
	if (lsd_want(60))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi1)
	}
	if (lsd_enter(60, ctx))
		return
	ls_hud(0, LsHud:2, 4.0, "%s %d", "In:", lsi1)
	lsd_exit(60, "shown")
}

// Wave Cleared (wc)
lsg_n61(ctx[])
{
	if (lsd_want(61))
	{
	lsd_begin()
	}
	if (lsd_enter(61, ctx))
		return
	lsd_exit(61, "ran")
	lsg_n62(ctx)
}

// Start Wave (wc_start)
lsg_n62(ctx[])
{
	new Float:lsi0 = ((Float:ctx[0]) + (1.0))
	if (lsd_want(62))
	{
	lsd_begin()
	lsd_in_number("Wave", lsi0)
	}
	if (lsd_enter(62, ctx))
		return
	if (!ls_components_running())
		lsd_error(62, "gm_components isn't running on this server.")
	else
	{
		ls_wave_start(max(floatround(lsi0), 0))
		lsd_exit(62, "done")
	}
}

// Crate Opened (lo)
lsg_n63(ctx[])
{
	if (lsd_want(63))
	{
	lsd_begin()
	}
	if (lsd_enter(63, ctx))
		return
	lsd_exit(63, "ran")
	lsg_n64(ctx)
}

// Show HUD Text (lo_hud)
lsg_n64(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(64))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(64, ctx))
		return
	ls_hud(lsi0, LsHud:1, 4.0, "%s", "Looted")
	lsd_exit(64, "shown")
}

// Checkpoint Reached (ck)
lsg_n65(ctx[])
{
	if (lsd_want(65))
	{
	lsd_begin()
	}
	if (lsd_enter(65, ctx))
		return
	lsd_exit(65, "ran")
	lsg_n66(ctx)
}

// Set Variable (ck_set)
lsg_n66(ctx[])
{
	new Float:lsi2 = Float:ctx[2]
	if (lsd_want(66))
	{
	lsd_begin()
	lsd_in_number("Value", lsi2)
	}
	if (lsd_enter(66, ctx))
		return
	g_v1 = lsi2
	lsd_exit(66, fmt("%.2f", g_v1))
}

// Barricade Broken (bb)
lsg_n67(ctx[])
{
	if (lsd_want(67))
	{
	lsd_begin()
	}
	if (lsd_enter(67, ctx))
		return
	lsd_exit(67, "ran")
	lsg_n68(ctx)
}

// Show Message (bb_msg)
lsg_n68(ctx[])
{
	if (lsd_want(68))
	{
	lsd_begin()
	}
	if (lsd_enter(68, ctx))
		return
	ls_announce(0, 3.0, "%s", "The barricade is down!")
	lsd_exit(68, "shown")
}

// Meeting Ended (me)
lsg_n69(ctx[])
{
	if (lsd_want(69))
	{
	lsd_begin()
	}
	if (lsd_enter(69, ctx))
		return
	lsd_exit(69, "ran")
	lsg_n70(ctx)
}

// Show HUD Text (me_hud)
lsg_n70(ctx[])
{
	new lsi1 = ctx[1]
	if (lsd_want(70))
	{
	lsd_begin()
	lsd_in_player("Value", lsi1)
	}
	if (lsd_enter(70, ctx))
		return
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Out:", lsg_name(lsi1))
	lsd_exit(70, "shown")
}

// Creature Killed (ck2)
lsg_n71(ctx[])
{
	if (lsd_want(71))
	{
	lsd_begin()
	}
	if (lsd_enter(71, ctx))
		return
	lsd_exit(71, "ran")
	lsg_n72(ctx)
}

// Set Variable (ck2_set)
lsg_n72(ctx[])
{
	new lsi2[128]
	copy(lsi2, 127, g_lsgTxt[0])
	if (lsd_want(72))
	{
	lsd_begin()
	lsd_in_text("Value", lsi2)
	}
	if (lsd_enter(72, ctx))
		return
	copy(g_v3, 63, lsi2)
	lsd_exit(72, "set")
	lsg_n73(ctx)
}

// Create Component (ck2_make)
lsg_n73(ctx[])
{
	if (lsd_want(73))
	{
	lsd_begin()
	}
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
		if (g_lsgOut[73])
			lsd_exit(73, fmt("entity %d", g_lsgOut[73]))
		else
			lsd_error(73, "gm_components couldn't make that component (it has no room, or the keys are wrong).")
	}
	lsg_n74(ctx)
}

// Spawn Creature (ck2_spawn)
lsg_n74(ctx[])
{
	new Float:lsi1 = 0.0
	new Float:lsi2 = 0.0
	if (lsd_want(74))
	{
	lsd_begin()
	lsd_in_number("Health", lsi1)
	lsd_in_number("Pay", lsi2)
	}
	if (lsd_enter(74, ctx))
		return
	if (!get_cvar_pointer("ls_creature_ver"))
		lsd_error(74, "This server's bot brain has no creatures (ls_creature).")
	else if (!(g_lsgOut[74] = lsg_creature_x("zombie", "spot1", 0, lsi1, floatround(lsi2))))
		lsd_error(74, "Not spawned: no entity has that name, or that type isn't loaded on this map.")
	else
		lsd_exit(74, fmt("entity %d", g_lsgOut[74]))
	lsg_n75(ctx)
}

// Show HUD Text (ck2_hud)
lsg_n75(ctx[])
{
	new lsi1 = g_lsgOut[74]
	if (lsd_want(75))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi1)
	}
	if (lsd_enter(75, ctx))
		return
	ls_hud(0, LsHud:2, 4.0, "%s %d", "New one:", lsi1)
	lsd_exit(75, "shown")
	lsg_n76(ctx)
}

// Show HUD Text (ck2_hud2)
lsg_n76(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1[128]
	copy(lsi1, 127, g_v3)
	if (lsd_want(76))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Value", lsi1)
	}
	if (lsd_enter(76, ctx))
		return
	ls_hud(lsi0, LsHud:1, 4.0, "%s %s", "Killed:", lsi1)
	lsd_exit(76, "shown")
}

// Player Hurt (hurt)
lsg_n77(ctx[])
{
	if (lsd_want(77))
	{
	lsd_begin()
	}
	if (lsd_enter(77, ctx))
		return
	lsd_exit(77, "ran")
	lsg_n78(ctx)
}

// Add To Variable (hurt_tadd)
lsg_n78(ctx[])
{
	new lsi1 = lsg_team(ctx[1])
	new Float:lsi2 = 1.0
	if (lsd_want(78))
	{
	lsd_begin()
	lsd_in_team("Team", lsi1)
	lsd_in_number("Amount", lsi2)
	}
	if (lsd_enter(78, ctx))
		return
	g_tv4[clamp(lsi1, 0, 3)] += lsi2
	lsd_exit(78, fmt("%.2f", g_tv4[clamp(lsi1, 0, 3)]))
	lsg_n79(ctx)
}

// Set Glow (hurt_glow)
lsg_n79(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(79))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(79, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(79, "Nothing done: nobody there.")
	else
	{
		set_user_rendering(lsi0, kRenderFxGlowShell, 255, 0, 0, kRenderNormal, 16)
		lsd_exit(79, "done")
	}
}

// Team Changed (tc)
lsg_n80(ctx[])
{
	if (lsd_want(80))
	{
	lsd_begin()
	}
	if (lsd_enter(80, ctx))
		return
	lsd_exit(80, "ran")
	lsg_n81(ctx)
}

// Set Model (tc_model)
lsg_n81(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(81))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(81, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(81, "Nothing done: nobody there.")
	else
	{
		cs_set_user_model(lsi0, "gign")
		lsd_exit(81, "done")
	}
}

// New Round (nr)
lsg_n82(ctx[])
{
	if (lsd_want(82))
	{
	lsd_begin()
	}
	if (lsd_enter(82, ctx))
		return
	lsd_exit(82, "ran")
	lsg_n83(ctx)
}

// Set Setting (nr_cvar)
lsg_n83(ctx[])
{
	new Float:lsi0 = 0.0
	if (lsd_want(83))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(83, ctx))
		return
	set_cvar_float("mp_freeforall", lsi0)
	lsd_exit(83, "set")
	lsg_n84(ctx)
}

// For Each Player (nr_each)
lsg_n84(ctx[])
{
	if (lsd_want(84))
	{
	lsd_begin()
	}
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
		c[8] = p
		lsg_n85(c)
	}
	lsd_exit(84, lsd_times(count))
	lsg_n86(ctx)
}

// Respawn (nr_resp)
lsg_n85(ctx[])
{
	new lsi0 = ctx[8]
	if (lsd_want(85))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(85, ctx))
		return
	if (!lsg_ok(lsi0) || is_user_alive(lsi0) || lsg_team(lsi0) < 1 || lsg_team(lsi0) > 2)
		lsd_error(85, "Nothing done: only a dead player on a team comes back.")
	else
	{
		ExecuteHamB(Ham_CS_RoundRespawn, lsi0)
		lsd_exit(85, "respawned")
	}
}

// Pick Random Entity (nr_pick)
lsg_n86(ctx[])
{
	if (lsd_want(86))
	{
	lsd_begin()
	}
	if (lsd_enter(86, ctx))
		return
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[9] = list[random(count)]
		lsd_exit(86, fmt("picked %d of %d", ctx[9], count))
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
	if (lsd_want(87))
	{
	lsd_begin()
	}
	if (lsd_enter(87, ctx))
		return
	new c[LSG_CTX], count
	new p = -1
	while (count < 512 && (p = find_ent_by_tname(p, "door1")) > 0)
	{
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[10] = p
		lsg_n88(c)
	}
	lsd_exit(87, lsd_times(count))
	lsg_n90(ctx)
}

// Use Entity (nr_use)
lsg_n88(ctx[])
{
	if (lsd_want(88))
	{
	lsd_begin()
	}
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
	new lsi2[128]
	copy(lsi2, 127, "play")
	if (lsd_want(89))
	{
	lsd_begin()
	lsd_in_text("Value", lsi2)
	}
	if (lsd_enter(89, ctx))
		return
	copy(g_v5, 63, lsi2)
	lsd_exit(89, "set")
}

// Stop Timer (nr_stop)
lsg_n90(ctx[])
{
	if (lsd_want(90))
	{
	lsd_begin()
	}
	if (lsd_enter(90, ctx))
		return
	remove_task(LSG_TASK + 4)
	lsd_exit(90, "stopped")
}

// Show HUD Text (nr_hud)
lsg_n91(ctx[])
{
	new lsi1 = ctx[9]
	if (lsd_want(91))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi1)
	}
	if (lsd_enter(91, ctx))
		return
	ls_hud(0, LsHud:0, 4.0, "%s %d", "Spot:", lsi1)
	lsd_exit(91, "shown")
}

// Chat Command (cmd)
lsg_n92(ctx[])
{
	if (lsd_want(92))
	{
	lsd_begin()
	}
	if (lsd_enter(92, ctx))
		return
	lsd_exit(92, "called")
	lsg_n93(ctx)
}

// Menu (cmd_menu)
lsg_n93(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(93))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(93, ctx))
		return
	if (!lsg_ok(lsi0) || is_user_bot(lsi0))
		lsd_error(93, "Not shown: nobody there, or a bot.")
	else
	{
		for (new k = 0; k < LSG_CTX; k++)
			g_lsgM93[lsi0][k] = ctx[k]
		new m = menu_create("Shop", "lsg_mh93")
		menu_additem(m, "Armor")
		menu_additem(m, "Money")
		menu_additem(m, "Gravity")
		menu_display(lsi0, m)
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
	new lsi0 = ctx[0]
	if (lsd_want(94))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(94, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(94, "Not given: that player isn't alive.")
	else
	{
		cs_set_user_armor(lsi0, 100, CS_ARMOR_VESTHELM)
		lsd_exit(94, "done")
	}
}

// Give Money (cmd_money)
lsg_n95(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = -500.0
	if (lsd_want(95))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi1)
	}
	if (lsd_enter(95, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(95, "Nothing done: nobody there.")
	else
	{
		lsg_add_money(lsi0, floatround(lsi1))
		lsd_exit(95, "done")
	}
}

// Set Gravity (cmd_grav)
lsg_n96(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 0.5
	if (lsd_want(96))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Gravity", lsi1)
	}
	if (lsd_enter(96, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(96, "Nothing done: that player isn't alive.")
	else
	{
		set_user_gravity(lsi0, lsi1)
		lsd_exit(96, "done")
	}
}

// Progress Bar (cmd_bar)
lsg_n97(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(97))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(97, ctx))
		return
	lsg_bar(lsi0, 3)
	lsd_exit(97, "shown")
	lsg_n98(ctx)
}

// Chat Message (cmd_chat)
lsg_n98(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = lsg_get_money(ctx[0])
	if (lsd_want(98))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(98, ctx))
		return
	ls_chat(lsi0, "%s %s", "Money:", lsg_ntext(lsi1))
	lsd_exit(98, "said")
}

// Function (fn)
lsg_n99(ctx[])
{
	if (lsd_want(99))
	{
	lsd_begin()
	}
	if (lsd_enter(99, ctx))
		return
	lsd_exit(99, "called")
	lsg_n100(ctx)
}

// Add Score (fn_frag)
lsg_n100(ctx[])
{
	new lsi0 = ctx[11]
	new Float:lsi1 = Float:ctx[12]
	if (lsd_want(100))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi1)
	}
	if (lsd_enter(100, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(100, "Nothing done: nobody there.")
	else
	{
		lsg_add_frags(lsi0, floatround(lsi1))
		lsd_exit(100, "done")
	}
	lsg_n101(ctx)
}

// Play Sound (fn_snd)
lsg_n101(ctx[])
{
	new lsi0 = ctx[11]
	if (lsd_want(101))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(101, ctx))
		return
	client_cmd(lsi0, "spk %s", "buttons/bell1")
	lsd_exit(101, "played")
	lsg_n102(ctx)
}

// Set Role (fn_role)
lsg_n102(ctx[])
{
	new lsi0 = ctx[11]
	if (lsd_want(102))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(102, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(102, "Nothing done: nobody there.")
	else
	{
		ls_set_role(lsi0, "hero")
		lsd_exit(102, "done")
	}
}

// Player Extracted (pk2)
lsg_n103(ctx[])
{
	if (lsd_want(103))
	{
	lsd_begin()
	}
	if (lsd_enter(103, ctx))
		return
	lsd_exit(103, "ran")
	lsg_n104(ctx)
}

// Call Function (pk2_call)
lsg_n104(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi1 = 3.0
	if (lsd_want(104))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Number", lsi1)
	}
	if (lsd_enter(104, ctx))
		return
	new c[LSG_CTX]
	c[11] = lsi0
	c[12] = _:(lsi1)
	lsg_n99(c)
	lsd_exit(104, "called")
	lsg_n105(ctx)
}

// Pick Random Player (pk2_pick)
lsg_n105(ctx[])
{
	if (lsd_want(105))
	{
	lsd_begin()
	}
	if (lsd_enter(105, ctx))
		return
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 5))
			list[count++] = p
	if (count > 0)
	{
		ctx[13] = list[random(count)]
		lsd_exit(105, fmt("picked %d of %d", ctx[13], count))
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
	new lsi0 = ctx[13]
	if (lsd_want(106))
	{
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	}
	if (lsd_enter(106, ctx))
		return
	if (!ls_alive(lsi0) || !is_user_bot(lsi0))
		lsd_error(106, "Nothing done: that player isn't a living bot.")
	else
	{
		lsg_bot_goto(lsi0, "spot1", 0)
		lsd_exit(106, "ordered")
	}
	lsg_n107(ctx)
}

// Bot Hold (pk2_hold)
lsg_n107(ctx[])
{
	new lsi0 = ctx[13]
	new Float:lsi1 = 10.0
	if (lsd_want(107))
	{
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	lsd_in_number("Seconds", lsi1)
	}
	if (lsd_enter(107, ctx))
		return
	if (!ls_alive(lsi0) || !is_user_bot(lsi0))
		lsd_error(107, "Nothing done: that player isn't a living bot.")
	else
	{
		ls_bot_hold(lsi0, max(floatround(lsi1), 1))
		lsd_exit(107, "ordered")
	}
	lsg_n108(ctx)
}

// Bot Follow (pk2_follow)
lsg_n108(ctx[])
{
	new lsi0 = ctx[13]
	new lsi1 = ctx[1]
	new Float:lsi2 = 50.0
	if (lsd_want(108))
	{
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	lsd_in_player("Follow", lsi1)
	lsd_in_number("Personality influence", lsi2)
	}
	if (lsd_enter(108, ctx))
		return
	if (!ls_alive(lsi0) || !is_user_bot(lsi0))
		lsd_error(108, "Nothing done: that player isn't a living bot.")
	else
	{
		lsg_bot_follow(lsi0, lsi1, 10, clamp(floatround(lsi2), 0, 100))
		lsd_exit(108, "ordered")
	}
	lsg_n109(ctx)
}

// Bot Free (pk2_free)
lsg_n109(ctx[])
{
	new lsi0 = ctx[13]
	if (lsd_want(109))
	{
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	}
	if (lsd_enter(109, ctx))
		return
	if (!ls_alive(lsi0) || !is_user_bot(lsi0))
		lsd_error(109, "Nothing done: that player isn't a living bot.")
	else
	{
		ls_bot_release(lsi0)
		lsd_exit(109, "ordered")
	}
	lsg_n110(ctx)
}

// Drop Weapon (pk2_drop)
lsg_n110(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(110))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(110, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(110, "Nothing done: that player isn't alive.")
	else
	{
		lsg_drop(lsi0)
		lsd_exit(110, "done")
	}
	lsg_n111(ctx)
}

// Freeze (pk2_fr)
lsg_n111(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1 = 1
	if (lsd_want(111))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_bool("Frozen", lsi1)
	}
	if (lsd_enter(111, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(111, "Nothing done: that player isn't alive.")
	else
	{
		lsg_freeze(lsi0, ((lsi1) ? 1 : 0))
		lsd_exit(111, "done")
	}
	lsg_n112(ctx)
}

// Kill (pk2_kill)
lsg_n112(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(112))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(112, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(112, "Nothing done: that player isn't alive.")
	else
	{
		user_kill(lsi0, 1)
		lsd_exit(112, "done")
	}
}

// Point Lost (lo2)
lsg_n113(ctx[])
{
	if (lsd_want(113))
	{
	lsd_begin()
	}
	if (lsd_enter(113, ctx))
		return
	lsd_exit(113, "ran")
	lsg_n115(ctx)
}

// Branch (lo2_br)
lsg_n114(ctx[])
{
	new lsi0 = (equali(g_v5, "play") ? 1 : 0)
	if (lsd_want(114))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(114, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(114, g_lsdBool[ok])
	if (ok)
		lsg_n116(ctx)
}

// Pick Random Player (lo2_pick)
lsg_n115(ctx[])
{
	if (lsd_want(115))
	{
	lsd_begin()
	}
	if (lsd_enter(115, ctx))
		return
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[14] = list[random(count)]
		lsd_exit(115, fmt("picked %d of %d", ctx[14], count))
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
	new lsi0 = (((ls_components_running() && ls_zone_inside(lsg_find("zone1"), ctx[14]) ? 1 : 0)) || ((ls_is_role(ctx[14], "hero") ? 1 : 0)) ? 1 : 0)
	if (lsd_want(116))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(116, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(116, g_lsdBool[ok])
	if (ok)
		lsg_n117(ctx)
	else
		lsg_n118(ctx)
}

// Respawn At Point (lo2_resp)
lsg_n117(ctx[])
{
	new lsi0 = ctx[14]
	if (lsd_want(117))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(117, ctx))
		return
	if (!ls_components_running() || !lsg_ok(lsi0))
		lsd_error(117, "gm_components isn't running, or nobody there.")
	else
	{
		ls_respawn_at(lsi0, 0)
		lsd_exit(117, "done")
	}
}

// Call Supply Drop (lo2_drop)
lsg_n118(ctx[])
{
	if (lsd_want(118))
	{
	lsd_begin()
	}
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
	new Float:lsi1 = ((float(ls_count_role("hero"))) + (float(lsg_creatures_alive())))
	if (lsd_want(119))
	{
	lsd_begin()
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(119, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %s", "Heroes and creatures:", lsg_ntext(lsi1))
	lsd_exit(119, "shown")
	lsg_n120(ctx)
}

// Branch (lo2_br3)
lsg_n120(ctx[])
{
	new lsi0 = ((lsg_get_frags(ctx[14])) > (lsg_get_deaths(ctx[14])) ? 1 : 0)
	if (lsd_want(120))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(120, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(120, g_lsdBool[ok])
	if (ok)
		lsg_n121(ctx)
}

// End Round (lo2_end)
lsg_n121(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(121))
	{
	lsd_begin()
	lsd_in_team("Winner", lsi0)
	}
	if (lsd_enter(121, ctx))
		return
	lsg_end_round(lsi0)
	lsd_exit(121, "ended")
}

// Map Started (ms)
lsg_n122(ctx[])
{
	if (lsd_want(122))
	{
	lsd_begin()
	}
	if (lsd_enter(122, ctx))
		return
	lsd_exit(122, "ran")
	lsg_n123(ctx)
}

// Load Map Pack (ms_pack)
lsg_n123(ctx[])
{
	if (lsd_want(123))
	{
	lsd_begin()
	}
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
	if (lsd_want(124))
	{
	lsd_begin()
	}
	if (lsd_enter(124, ctx))
		return
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "ls_zone_shrinker")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[15] = list[random(count)]
		lsd_exit(124, fmt("picked %d of %d", ctx[15], count))
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
	new lsi0 = ctx[15]
	if (lsd_want(125))
	{
	lsd_begin()
	lsd_in_entity("Component", lsi0)
	}
	if (lsd_enter(125, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, ""))
		lsd_error(125, "Not done: that isn't a component (or gm_components isn't running).")
	else
	{
		ls_comp_start(lsi0)
		lsd_exit(125, "done")
	}
	lsg_n126(ctx)
}

// Stop Component (ms_stop)
lsg_n126(ctx[])
{
	new lsi0 = ctx[15]
	if (lsd_want(126))
	{
	lsd_begin()
	lsd_in_entity("Component", lsi0)
	}
	if (lsd_enter(126, ctx))
		return
	if (!ls_components_running() || !lsg_kind_ok(lsi0, ""))
		lsd_error(126, "Not done: that isn't a component (or gm_components isn't running).")
	else
	{
		ls_comp_stop(lsi0)
		lsd_exit(126, "done")
	}
	lsg_n127(ctx)
}

// Create Component (ms_make)
lsg_n127(ctx[])
{
	new lsi0 = ctx[15]
	if (lsd_want(127))
	{
	lsd_begin()
	lsd_in_entity("At", lsi0)
	}
	if (lsd_enter(127, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(127, "gm_components isn't running on this server.")
	else if (!lsg_ent_origin(lsi0, o))
		lsd_error(127, "No entity has that name on this map.")
	else
	{
		g_lsgOut[127] = ls_comp_create("ls_loot_container", o, "table=default")
		if (g_lsgOut[127])
			lsd_exit(127, fmt("entity %d", g_lsgOut[127]))
		else
			lsd_error(127, "gm_components couldn't make that component (it has no room, or the keys are wrong).")
	}
	lsg_n128(ctx)
}

// Call Supply Drop (ms_drop)
lsg_n128(ctx[])
{
	new lsi0 = ctx[15]
	if (lsd_want(128))
	{
	lsd_begin()
	lsd_in_entity("At", lsi0)
	}
	if (lsd_enter(128, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(128, "gm_components isn't running on this server.")
	else if (!lsg_ent_origin(lsi0, o))
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
	if (lsd_want(129))
	{
	lsd_begin()
	}
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
		c[16] = p
		lsg_n130(c)
	}
	lsd_exit(129, lsd_times(count))
}

// Teleport (ms_tp)
lsg_n130(ctx[])
{
	new lsi0 = ctx[16]
	new lsi1 = ctx[15]
	if (lsd_want(130))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_entity("To", lsi1)
	}
	if (lsd_enter(130, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(130, "Not moved: that player isn't alive.")
	else if (!lsg_teleport_to(lsi0, lsi1))
		lsd_error(130, "Not moved: no such entity on this map.")
	else
		lsd_exit(130, "teleported")
}

// Player About To Be Hurt (ah)
lsg_n131(ctx[])
{
	if (lsd_want(131))
	{
	lsd_begin()
	}
	if (lsd_enter(131, ctx))
		return
	lsd_exit(131, "called")
	lsg_n132(ctx)
}

// Branch (ah_br)
lsg_n132(ctx[])
{
	new lsi0 = (equali(g_lsgTxt[0], "fall") ? 1 : 0)
	if (lsd_want(132))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(132, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(132, g_lsdBool[ok])
	if (ok)
		lsg_n133(ctx)
	else
		lsg_n134(ctx)
}

// Block Damage (ah_block)
lsg_n133(ctx[])
{
	if (lsd_want(133))
	{
	lsd_begin()
	}
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
	new Float:lsi0 = 0.5
	if (lsd_want(134))
	{
	lsd_begin()
	lsd_in_number("Times", lsi0)
	}
	if (lsd_enter(134, ctx))
		return
	g_lsgDmg = g_lsgDmg * floatmax(lsi0, 0.0)
	if (g_lsgDmgBlock)
		lsd_exit(134, "blocked")
	else
		lsd_exit(134, fmt("%.1f", g_lsgDmg))
	lsg_n135(ctx)
}

// Set Damage (ah_set)
lsg_n135(ctx[])
{
	new Float:lsi0 = 5.0
	if (lsd_want(135))
	{
	lsd_begin()
	lsd_in_number("Damage", lsi0)
	}
	if (lsd_enter(135, ctx))
		return
	g_lsgDmg = floatmax(lsi0, 0.0)
	if (g_lsgDmgBlock)
		lsd_exit(135, "blocked")
	else
		lsd_exit(135, fmt("%.1f", g_lsgDmg))
}

// Player Killed (dk)
lsg_n136(ctx[])
{
	if (lsd_want(136))
	{
	lsd_begin()
	}
	if (lsd_enter(136, ctx))
		return
	lsd_exit(136, "ran")
	lsg_n137(ctx)
}

// Make Downed (dk_down)
lsg_n137(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(137))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(137, ctx))
		return
	if (!lsg_make_down(lsi0, 20.0, 2.0, 40))
		lsd_error(137, "Not downed: that player isn't dead (wire it from Player Killed).")
	else
		lsd_exit(137, "downed")
}

// Player Revived (rv)
lsg_n138(ctx[])
{
	if (lsd_want(138))
	{
	lsd_begin()
	}
	if (lsd_enter(138, ctx))
		return
	lsd_exit(138, "called")
	lsg_n139(ctx)
}

// Show HUD Text (rv_hud)
lsg_n139(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = ctx[1]
	if (lsd_want(139))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_player("Value", lsi1)
	}
	if (lsd_enter(139, ctx))
		return
	ls_hud(lsi0, LsHud:3, 4.0, "%s %s", "Saved by", lsg_name(lsi1))
	lsd_exit(139, "shown")
}

// Downed Player Out (to)
lsg_n140(ctx[])
{
	if (lsd_want(140))
	{
	lsd_begin()
	}
	if (lsd_enter(140, ctx))
		return
	lsd_exit(140, "called")
	lsg_n141(ctx)
}

// Add To Saved (to_add)
lsg_n141(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 1.0
	if (lsd_want(141))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Amount", lsi1)
	}
	if (lsd_enter(141, ctx))
		return
	if (!lsg_ok(lsi0) || g_lsgSaved == Invalid_Trie)
		lsd_error(141, "Not saved: nobody there, or the store didn't open.")
	else
	{
		lsg_saved_set(lsi0, "outs", lsg_saved_get(lsi0, "outs") + (lsi1))
		lsd_exit(141, fmt("%.1f", lsg_saved_get(lsi0, "outs")))
	}
	lsg_n142(ctx)
}

// Save Value (to_set)
lsg_n142(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 1.0
	if (lsd_want(142))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(142, ctx))
		return
	if (!lsg_ok(lsi0) || g_lsgSaved == Invalid_Trie)
		lsd_error(142, "Not saved: nobody there, or the store didn't open.")
	else
	{
		lsg_saved_set(lsi0, "last_out", lsi1)
		lsd_exit(142, fmt("%.1f", lsg_saved_get(lsi0, "last_out")))
	}
	lsg_n143(ctx)
}

// Show HUD Text (to_hud)
lsg_n143(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = lsg_saved_get(ctx[0], "outs")
	if (lsd_want(143))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(143, ctx))
		return
	ls_hud(lsi0, LsHud:1, 4.0, "%s %s", "Times out:", lsg_ntext(lsi1))
	lsd_exit(143, "shown")
	lsg_n144(ctx)
}

// Branch (to_br)
lsg_n144(ctx[])
{
	new lsi0 = (g_lsgDown[lsg_slot(ctx[0])] ? 1 : 0)
	if (lsd_want(144))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(144, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(144, g_lsdBool[ok])
}

// Player Spawned (sq)
lsg_n145(ctx[])
{
	if (lsd_want(145))
	{
	lsd_begin()
	}
	if (lsd_enter(145, ctx))
		return
	lsd_exit(145, "ran")
	lsg_n146(ctx)
}

// No Fall Damage (sq_nf)
lsg_n146(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(146))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(146, ctx))
		return
	g_lsgNoFall[lsg_slot(lsi0)] = 2
	lsd_exit(146, "set")
	lsg_n147(ctx)
}

// Set Squad (sq_set)
lsg_n147(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 2.0
	if (lsd_want(147))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Squad", lsi1)
	}
	if (lsd_enter(147, ctx))
		return
	if (!lsg_ok(lsi0))
		lsd_error(147, "Nothing done: nobody there.")
	else
	{
		lsg_set_squad(lsi0, clamp(floatround(lsi1), 0, 32))
		lsd_exit(147, "set")
	}
	lsg_n148(ctx)
}

// Squad Friendly Fire (sq_ff)
lsg_n148(ctx[])
{
	new lsi0 = 0
	if (lsd_want(148))
	{
	lsd_begin()
	lsd_in_bool("On", lsi0)
	}
	if (lsd_enter(148, ctx))
		return
	g_lsgSquadFF = bool:((lsi0) ? 1 : 0)
	lsd_exit(148, "set")
	lsg_n149(ctx)
}

// Show HUD Text (sq_hud)
lsg_n149(ctx[])
{
	new Float:lsi1 = ((float(g_lsgSquad[lsg_slot(ctx[0])])) + (float(lsg_squads_alive())))
	if (lsd_want(149))
	{
	lsd_begin()
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(149, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %s", "Squad:", lsg_ntext(lsi1))
	lsd_exit(149, "shown")
	lsg_n150(ctx)
}

// Buying (sq_buy)
lsg_n150(ctx[])
{
	if (lsd_want(150))
	{
	lsd_begin()
	}
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
	if (lsd_want(151))
	{
	lsd_begin()
	}
	if (lsd_enter(151, ctx))
		return
	g_lsgBuyAllow = 1
	lsd_exit(151, "set")
	lsg_n153(ctx)
}

// Launch Up (sq_up)
lsg_n152(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 600.0
	if (lsd_want(152))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Speed", lsi1)
	}
	if (lsd_enter(152, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(152, "Nothing done: that player isn't alive.")
	else
	{
		lsg_launch(lsi0, lsi1)
		lsd_exit(152, "done")
	}
}

// Bot Go To (sq_goto)
lsg_n153(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(153))
	{
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	}
	if (lsd_enter(153, ctx))
		return
	if (!ls_alive(lsi0) || !is_user_bot(lsi0))
		lsd_error(153, "Nothing done: that player isn't a living bot.")
	else
	{
		lsg_bot_goto(lsi0, "spot1", 15)
		lsd_exit(153, "ordered")
	}
	lsg_n152(ctx)
}

// Round Started (ck3)
lsg_n154(ctx[])
{
	if (lsd_want(154))
	{
	lsd_begin()
	}
	if (lsd_enter(154, ctx))
		return
	lsd_exit(154, "ran")
	lsg_n155(ctx)
}

// Set Round Clock (ck3_set)
lsg_n155(ctx[])
{
	new Float:lsi0 = 180.0
	if (lsd_want(155))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(155, ctx))
		return
	lsg_clock_set(lsi0)
	lsd_exit(155, fmt("%.0f s left", lsg_clock_left()))
	lsg_n156(ctx)
}

// Add Round Time (ck3_add)
lsg_n156(ctx[])
{
	new Float:lsi0 = 30.0
	if (lsd_want(156))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(156, ctx))
		return
	lsg_clock_add(lsi0)
	lsd_exit(156, fmt("%.0f s left", lsg_clock_left()))
	lsg_n157(ctx)
}

// Pause Round Clock (ck3_p)
lsg_n157(ctx[])
{
	if (lsd_want(157))
	{
	lsd_begin()
	}
	if (lsd_enter(157, ctx))
		return
	lsg_clock_pause(1)
	lsd_exit(157, fmt("%.0f s left", lsg_clock_left()))
	lsg_n158(ctx)
}

// Pause Round Clock (ck3_r)
lsg_n158(ctx[])
{
	if (lsd_want(158))
	{
	lsd_begin()
	}
	if (lsd_enter(158, ctx))
		return
	lsg_clock_pause(0)
	lsd_exit(158, fmt("%.0f s left", lsg_clock_left()))
	lsg_n159(ctx)
}

// Show HUD Text (ck3_hud)
lsg_n159(ctx[])
{
	new Float:lsi1 = lsg_clock_left()
	if (lsd_want(159))
	{
	lsd_begin()
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(159, ctx))
		return
	ls_hud(0, LsHud:1, 4.0, "%s %s", "Time:", lsg_ntext(lsi1))
	lsd_exit(159, "shown")
}

// Round Time Up (tu)
lsg_n160(ctx[])
{
	if (lsd_want(160))
	{
	lsd_begin()
	}
	if (lsd_enter(160, ctx))
		return
	lsd_exit(160, "called")
	lsg_n161(ctx)
}

// End Round (tu_end)
lsg_n161(ctx[])
{
	new lsi0 = 2
	if (lsd_want(161))
	{
	lsd_begin()
	lsd_in_team("Winner", lsi0)
	}
	if (lsd_enter(161, ctx))
		return
	lsg_end_round(lsi0)
	lsd_exit(161, "ended")
}

// Round Started (a0)
lsg_n162(ctx[])
{
	if (lsd_want(162))
	{
	lsd_begin()
	}
	if (lsd_enter(162, ctx))
		return
	lsd_exit(162, "ran")
	lsg_n163(ctx)
}

// Sequence (a_seq)
lsg_n163(ctx[])
{
	if (lsd_want(163))
	{
	lsd_begin()
	}
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
	new Float:lsi0 = 1.0
	new Float:lsi1 = 5.0
	if (lsd_want(164))
	{
	lsd_begin()
	lsd_in_number("First", lsi0)
	lsd_in_number("Last", lsi1)
	}
	if (lsd_enter(164, ctx))
		return
	new c[LSG_CTX], n
	new last = floatround(lsi1)
	for (new k = floatround(lsi0); k <= last && n < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[17] = _:float(k)
		lsg_n165(c)
	}
	lsd_exit(164, lsd_times(n))
	lsg_n166(ctx)
}

// Add (a_add)
lsg_n165(ctx[])
{
	new lsi0 = g_v9
	new Float:lsi1 = Float:ctx[17]
	if (lsd_want(165))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_number("Item", lsi1)
	}
	if (lsd_enter(165, ctx))
		return
	g_lsgO[0] = _:float(lsg_apushc(lsi0, _:(lsi1)))
	lsd_exit(165, fmt("%d items", lsg_alen(lsi0)))
}

// Sort (a_sort)
lsg_n166(ctx[])
{
	new lsi0 = g_v9
	if (lsd_want(166))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(166, ctx))
		return
	lsg_asort(lsi0, 0, 1)
	lsd_exit(166, fmt("%d items", lsg_alen(lsi0)))
	lsg_n167(ctx)
}

// Shuffle (a_shuf)
lsg_n167(ctx[])
{
	new lsi0 = g_v9
	if (lsd_want(167))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(167, ctx))
		return
	lsg_ashuffle(lsi0)
	lsd_exit(167, fmt("%d items", lsg_alen(lsi0)))
	lsg_n168(ctx)
}

// Insert (a_ins)
lsg_n168(ctx[])
{
	new lsi0 = g_v9
	new Float:lsi1 = 99.0
	new Float:lsi2 = 0.0
	if (lsd_want(168))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_number("Item", lsi1)
	lsd_in_number("Index", lsi2)
	}
	if (lsd_enter(168, ctx))
		return
	lsg_ainsc(lsi0, lsi2, _:(lsi1))
	lsd_exit(168, fmt("%d items", lsg_alen(lsi0)))
	lsg_n169(ctx)
}

// Remove Index (a_rmi)
lsg_n169(ctx[])
{
	new lsi0 = g_v9
	new Float:lsi1 = 1.0
	if (lsd_want(169))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_number("Index", lsi1)
	}
	if (lsd_enter(169, ctx))
		return
	lsg_aremi(lsi0, lsi1)
	lsd_exit(169, fmt("%d items", lsg_alen(lsi0)))
	lsg_n170(ctx)
}

// Set Array Item (a_aset)
lsg_n170(ctx[])
{
	new lsi0 = g_v9
	new Float:lsi1 = 0.0
	new Float:lsi2 = 7.0
	if (lsd_want(170))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_number("Index", lsi1)
	lsd_in_number("Item", lsi2)
	}
	if (lsd_enter(170, ctx))
		return
	lsg_asetc(lsi0, lsi1, _:(lsi2))
	lsd_exit(170, fmt("%d items", lsg_alen(lsi0)))
	lsg_n171(ctx)
}

// Remove Item (a_arem)
lsg_n171(ctx[])
{
	new lsi0 = g_v9
	new Float:lsi1 = 99.0
	if (lsd_want(171))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_number("Item", lsi1)
	}
	if (lsd_enter(171, ctx))
		return
	g_lsgO[1] = lsg_aremc(lsi0, _:(lsi1)) ? 1 : 0
	lsd_exit(171, fmt("%d items", lsg_alen(lsi0)))
	lsg_n172(ctx)
}

// For Each (a_fe)
stock lsg_n172(ctx[])
{
	new lsi0 = g_v9
	if (lsd_want(172))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(172, ctx))
		return
	new c[LSG_CTX], n
	g_lsgLeft[172] = 0
	static snap
	if (!snap)
		snap = _:ArrayCreate(LSG_AC)
	lsg_acopy(snap, lsi0)
	new size = lsg_alen(snap)
	for (new k = 0; k < size && k < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[19] = _:float(k)
		c[18] = ArrayGetCell(Array:snap, k)
		lsg_n173(c)
		if (g_lsgLeft[172])
			break
	}
	lsd_exit(172, lsd_times(n))
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
	new lsi0 = ((Float:ctx[18]) > (3.0) ? 1 : 0)
	if (lsd_want(173))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(173, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(173, g_lsdBool[ok])
	if (ok)
		lsg_n172_break(ctx)
	else
		lsg_n174(ctx)
}

// Print String (a_pr)
lsg_n174(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_ntext(Float:ctx[18]))
	if (lsd_want(174))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(174, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Item", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(174, esc)
}

// Print String (a_pr2)
lsg_n175(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("%s items, has 7 %s at %s, one is %s (100%%)", lsg_ntext(float(lsg_alen(g_v9))), g_lsgYesNo[((lsg_afindc(g_v9, _:(7.0)) >= 0 ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_afindc(g_v9, _:(7.0)))), lsg_ntext(Float:lsg_arandc(g_v9))))
	if (lsd_want(175))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(175, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Numbers:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(175, esc)
	lsg_n176(ctx)
}

// Clear (a_clr)
lsg_n176(ctx[])
{
	new lsi0 = g_v9
	if (lsd_want(176))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(176, ctx))
		return
	lsg_aclear(lsi0)
	lsd_exit(176, fmt("%d items", lsg_alen(lsi0)))
}

// Set Variable (b_set)
lsg_n177(ctx[])
{
	new lsi2 = lsg_mka231("a", lsg_tcat("b", "c"))
	if (lsd_want(177))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi2)))
	}
	if (lsd_enter(177, ctx))
		return
	lsg_acopy(g_v8, lsi2)
	lsd_exit(177, "set")
	lsg_n178(ctx)
}

// While Loop (b_wh)
lsg_n178(ctx[])
{
	if (lsd_want(178))
	{
	lsd_begin()
	lsd_in_bool("Condition", ((float(lsg_alen(g_v8))) < (5.0) ? 1 : 0))
	}
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
	lsd_exit(178, lsd_times(n))
	lsg_n180(ctx)
}

// Add (b_add)
lsg_n179(ctx[])
{
	new lsi0 = g_v8
	new lsi1[128]
	copy(lsi1, 127, fmt("%s items", lsg_ntext(float(lsg_alen(g_v8)))))
	if (lsd_want(179))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_text("Item", lsi1)
	}
	if (lsd_enter(179, ctx))
		return
	g_lsgO[2] = _:float(lsg_apushs(lsi0, lsi1))
	lsd_exit(179, fmt("%d items", lsg_alen(lsi0)))
}

// Switch on Text (b_sw)
lsg_n180(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_agets(g_v8, 0.0))
	if (lsd_want(180))
	{
	lsd_begin()
	lsd_in_text("Selection", lsi0)
	}
	if (lsd_enter(180, ctx))
		return
	new x[128]
	copy(x, 127, lsi0)
	if (equali(x, "a"))
	{
		lsd_exit(180, "a")
		lsg_n181(ctx)
	}
	else if (equali(x, "bc"))
	{
		lsd_exit(180, "bc")
		lsg_n182(ctx)
	}
	else
	{
		lsd_exit(180, "default")
		lsg_n184(ctx)
	}
}

// Print String (b_pr1)
lsg_n181(ctx[])
{
	if (lsd_want(181))
	{
	lsd_begin()
	}
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
	new Float:lsi0 = str_to_float("2")
	if (lsd_want(182))
	{
	lsd_begin()
	lsd_in_number("Selection", lsi0)
	}
	if (lsd_enter(182, ctx))
		return
	new Float:x = lsi0
	if (x == 1.0)
	{
		lsd_exit(182, "1")
	}
	else if (x == 2.0)
	{
		lsd_exit(182, "2")
		lsg_n183(ctx)
	}
	else if (x == 3.0)
	{
		lsd_exit(182, "3")
	}
	else
	{
		lsd_exit(182, "default")
	}
}

// Print String (b_pr2)
lsg_n183(ctx[])
{
	if (lsd_want(183))
	{
	lsd_begin()
	}
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
	new lsi0[128]
	copy(lsi0, 127, fmt("%s long, %s", lsg_ntext(float(strlen(lsg_arands(g_v8)))), g_lsgYesNo[((containi(lsg_arands(g_v8), "item") >= 0 ? 1 : 0)) ? 1 : 0]))
	if (lsd_want(184))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(184, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Other:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(184, esc)
}

// For Each Player (c_each)
lsg_n185(ctx[])
{
	if (lsd_want(185))
	{
	lsd_begin()
	}
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
		c[20] = p
		lsg_n186(c)
	}
	lsd_exit(185, lsd_times(count))
	lsg_n190(ctx)
}

// Set Variable (c_setp)
lsg_n186(ctx[])
{
	new lsi0 = ctx[20]
	new lsi2 = lsg_vplayer(ctx[20], 3)
	if (lsd_want(186))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Value", lsg_vtext(lsi2))
	}
	if (lsd_enter(186, ctx))
		return
	lsg_vset((LSG_VR + 1 + lsg_slot(lsi0)), lsi2)
	lsd_exit(186, "set")
	lsg_n187(ctx)
}

// Set Position (c_pos)
lsg_n187(ctx[])
{
	new lsi0 = ctx[20]
	new lsi1 = lsg_vadd(lsg_vplayer(ctx[20], 0), lsg_vec(0.0, 0.0, 36.0), 1.0)
	if (lsd_want(187))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Position", lsg_vtext(lsi1))
	}
	if (lsd_enter(187, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(187, "Nothing done: that player isn't alive.")
	else
	{
		lsg_vsetpos(lsi0, lsi1)
		lsd_exit(187, "done")
	}
	lsg_n188(ctx)
}

// Set Velocity (c_vel)
lsg_n188(ctx[])
{
	new lsi0 = ctx[20]
	new lsi1 = lsg_vscale(lsg_vnorm(lsg_vdir(lsg_vplayer(ctx[20], 0), lsg_vent(ctx[20]))), 300.0)
	if (lsd_want(188))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Velocity", lsg_vtext(lsi1))
	}
	if (lsd_enter(188, ctx))
		return
	if (!ls_alive(lsi0))
		lsd_error(188, "Nothing done: that player isn't alive.")
	else
	{
		lsg_vsetvel(lsi0, lsi1)
		lsd_exit(188, "done")
	}
	lsg_n189(ctx)
}

// Print String (c_pr)
lsg_n189(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("%s away, %s, at %s", lsg_ntext(lsg_vdist((LSG_VR + 1 + lsg_slot(ctx[20])), (LSG_VR + 0))), lsg_ntext(((lsg_vc(lsg_vplayer(ctx[20], 3), 2)) + (lsg_vlen(lsg_vadd(lsg_vplayer(ctx[20], 0), lsg_vec(0.0, 0.0, 36.0), 1.0))))), lsg_vtext(lsg_vplayer(ctx[20], 3))))
	if (lsd_want(189))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(189, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Vectors:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(189, esc)
}

// Set Variable (c_sl)
lsg_n190(ctx[])
{
	new lsi2 = lsg_mks0(30.0, "kit", lsg_vec(0.0, 0.0, 36.0), 0, 1)
	if (lsd_want(190))
	{
	lsd_begin()
	}
	if (lsd_enter(190, ctx))
		return
	lsg_rset((LSG_RR + 0), lsi2)
	lsd_exit(190, "set")
	lsg_n191(ctx)
}

// Set Field (c_sf)
lsg_n191(ctx[])
{
	new Float:lsi0 = lsg_clampf(50.0, 0.0, 40.0)
	if (lsd_want(191))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(191, ctx))
		return
	g_lsgRec[lsg_ri((LSG_RR + 0))][0] = _:(lsi0)
	lsd_exit(191, "set")
	lsg_n192(ctx)
}

// Set Field (c_sf2)
lsg_n192(ctx[])
{
	new lsi0 = (LSG_VR + 0)
	if (lsd_want(192))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(192, ctx))
		return
	lsg_rsetvec((LSG_RR + 0), 1, lsi0)
	lsd_exit(192, "set")
	lsg_n193(ctx)
}

// Add (c_push)
lsg_n193(ctx[])
{
	new lsi0 = g_v11
	new lsi1 = (LSG_RR + 0)
	if (lsd_want(193))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(193, ctx))
		return
	g_lsgO[3] = _:float(lsg_apushr(lsi0, lsi1))
	lsd_exit(193, fmt("%d items", lsg_alen(lsi0)))
	lsg_n194(ctx)
}

// Print String (c_pr2)
lsg_n194(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("%s %s %s %s", lsg_ntext(Float:g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][0]), g_lsgRecT[lsg_ri(lsg_agetr(g_v11, 0.0))][0], g_lsgE0[clamp(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5], 0, 2)], lsg_vtext(lsg_rvec(lsg_agetr(g_v11, 0.0), 1))))
	if (lsd_want(194))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(194, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Loadout:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(194, esc)
	lsg_n195(ctx)
}

// Switch on Enum (c_sw)
lsg_n195(ctx[])
{
	new lsi0 = g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5]
	if (lsd_want(195))
	{
	lsd_begin()
	lsd_in_text("Selection", g_lsgE0[clamp(lsi0, 0, 2)])
	}
	if (lsd_enter(195, ctx))
		return
	switch (lsi0)
	{
		case 0:
		{
			lsd_exit(195, "Scout")
			lsg_n196(ctx)
		}
		case 1:
		{
			lsd_exit(195, "Tank")
			lsg_n197(ctx)
		}
		case 2:
		{
			lsd_exit(195, "Medic")
		}
		default:
		{
			lsd_exit(195, "default")
		}
	}
}

// Print String (c_pr3)
lsg_n196(ctx[])
{
	if (lsd_want(196))
	{
	lsd_begin()
	}
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
	new Float:lsi0 = float(g_lsgRec[lsg_ri(lsg_agetr(g_v11, 0.0))][5])
	if (lsd_want(197))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(197, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Tank number", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(197, esc)
}

// For Each Player (d_each)
lsg_n198(ctx[])
{
	if (lsd_want(198))
	{
	lsd_begin()
	}
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
		c[21] = p
		lsg_n199(c)
	}
	lsd_exit(198, lsd_times(count))
	lsg_n203(ctx)
}

// Map Add (d_madd)
lsg_n199(ctx[])
{
	new lsi0 = g_v12
	new lsi1 = ctx[21]
	new Float:lsi2 = ((Float:lsg_mgetc(g_v12, lsg_kp(ctx[21]))) + (1.0))
	if (lsd_want(199))
	{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(lsi0)))
	lsd_in_player("Key", lsi1)
	lsd_in_number("Value", lsi2)
	}
	if (lsd_enter(199, ctx))
		return
	lsg_msetc(lsi0, lsg_kp(lsi1), _:(lsi2))
	lsd_exit(199, fmt("%d keys", lsg_mlen(lsi0)))
	lsg_n200(ctx)
}

// Call (d_call)
lsg_n200(ctx[])
{
	new lsi0 = ctx[21]
	new Float:lsi1 = 5.0
	new lsi2[128]
	copy(lsi2, 127, "kill")
	if (lsd_want(200))
	{
	lsd_begin()
	lsd_in_player("Who", lsi0)
	lsd_in_number("Amount", lsi1)
	lsd_in_text("Why", lsi2)
	}
	if (lsd_enter(200, ctx))
		return
	lsg_fr0(lsi0, lsi1, lsi2)
	g_lsgO[4] = _:g_lsgR0_0
	copy(g_lsgOT[0], 127, g_lsgR0_1)
	lsd_exit(200, "called")
	lsg_n201(ctx)
}

// Print String (d_pr)
lsg_n201(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgOT[0])
	if (lsd_want(201))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(201, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Said:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(201, esc)
	lsg_n202(ctx)
}

// Print String (d_pr2)
lsg_n202(ctx[])
{
	new Float:lsi0 = Float:g_lsgO[4]
	if (lsd_want(202))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(202, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Total:", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(202, esc)
}

// Map Add (d_tadd)
lsg_n203(ctx[])
{
	new lsi0 = g_v13
	new lsi1[128]
	copy(lsi1, 127, "k")
	new lsi2[128]
	copy(lsi2, 127, "k")
	if (lsd_want(203))
	{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(lsi0)))
	lsd_in_text("Key", lsi1)
	lsd_in_text("Value", lsi2)
	}
	if (lsd_enter(203, ctx))
		return
	lsg_msets(lsi0, lsi1, lsi2)
	lsd_exit(203, fmt("%d keys", lsg_mlen(lsi0)))
	lsg_n204(ctx)
}

// Map Add (d_vadd)
lsg_n204(ctx[])
{
	new lsi0 = g_v14
	new Float:lsi1 = 1.0
	new lsi2 = lsg_vec(0.0, 0.0, 36.0)
	if (lsd_want(204))
	{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(lsi0)))
	lsd_in_number("Key", lsi1)
	lsd_in_text("Value", lsg_vtext(lsi2))
	}
	if (lsd_enter(204, ctx))
		return
	lsg_msetv(lsi0, lsg_kn(lsi1), lsi2)
	lsd_exit(204, fmt("%d keys", lsg_mlen(lsi0)))
	lsg_n205(ctx)
}

// Map Remove (d_rem)
lsg_n205(ctx[])
{
	new lsi0 = g_v13
	new lsi1[128]
	copy(lsi1, 127, "kill")
	if (lsd_want(205))
	{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(lsi0)))
	lsd_in_text("Key", lsi1)
	}
	if (lsd_enter(205, ctx))
		return
	lsg_mdel(lsi0, lsi1)
	lsd_exit(205, fmt("%d keys", lsg_mlen(lsi0)))
	lsg_n206(ctx)
}

// Print String (d_pr3)
lsg_n206(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("%s %s %s %s", lsg_ntext(float(lsg_mlen(g_v12))), g_lsgYesNo[((lsg_mhas(g_v13, "k") ? 1 : 0)) ? 1 : 0], lsg_ntext(float(lsg_alen(lsg_mkeys(g_v12, 2)))), lsg_ntext(float(lsg_alen(lsg_mvalues(g_v14, 2))))))
	if (lsd_want(206))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(206, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Maps:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(206, esc)
	lsg_n207(ctx)
}

// Map Clear (d_mclr)
lsg_n207(ctx[])
{
	new lsi0 = g_v14
	if (lsd_want(207))
	{
	lsd_begin()
	lsd_in_text("Map", fmt("%d keys", lsg_mlen(lsi0)))
	}
	if (lsd_enter(207, ctx))
		return
	lsg_mclear(lsi0)
	lsd_exit(207, fmt("%d keys", lsg_mlen(lsi0)))
	lsg_n208(ctx)
}

// Print String (d_pr4)
lsg_n208(ctx[])
{
	new Float:lsi0 = lsg_pf1_0(4.0)
	if (lsd_want(208))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(208, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Double:", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(208, esc)
	lsg_n209(ctx)
}

// Print String (d_pr5)
lsg_n209(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_pf1_1(4.0))
	if (lsd_want(209))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(209, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "As text:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(209, esc)
	lsg_n210(ctx)
}

// Set Timer by Function (d_tbf)
lsg_n210(ctx[])
{
	new Float:lsi0 = 2.0
	if (lsd_want(210))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(210, ctx))
		return
	remove_task(LSG_TASK + 210)
	set_task(floatmax(lsi0, 0.1), "lsg_tf210", LSG_TASK + 210, _, _, "b")
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
	new lsi2 = (LSG_TASK + 210)
	if (lsd_want(211))
	{
	lsd_begin()
	lsd_in_text("Value", g_lsgRun[lsg_tactive(lsi2) ? 1 : 0])
	}
	if (lsd_enter(211, ctx))
		return
	g_v17 = lsi2
	lsd_exit(211, "set")
	lsg_n212(ctx)
}

// Print String (d_pr6)
lsg_n212(ctx[])
{
	new lsi0 = (lsg_tactive(g_v17) ? 1 : 0)
	if (lsd_want(212))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(212, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Timer on:", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(212, esc)
	lsg_n213(ctx)
}

// Clear Timer (d_tclr)
lsg_n213(ctx[])
{
	new lsi0 = g_v17
	if (lsd_want(213))
	{
	lsd_begin()
	lsd_in_text("Timer", g_lsgRun[lsg_tactive(lsi0) ? 1 : 0])
	}
	if (lsd_enter(213, ctx))
		return
	lsg_tclear(lsi0)
	lsd_exit(213, "cleared")
	lsg_n249(ctx)
}

// Print String (d_pr7)
lsg_n214(ctx[])
{
	new Float:lsi0 = ((3.0) * (2.0))
	if (lsd_want(214))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(214, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Macro gave:", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(214, esc)
}

// Player Killed (e0)
lsg_n215(ctx[])
{
	if (lsd_want(215))
	{
	lsd_begin()
	}
	if (lsd_enter(215, ctx))
		return
	lsd_exit(215, "ran")
	lsg_n216(ctx)
}

// Do Once (e_once)
stock lsg_n216(ctx[])
{
	if (lsd_want(216))
	{
	lsd_begin()
	}
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
stock lsg_n217(ctx[])
{
	new Float:lsi0 = 3.0
	if (lsd_want(217))
	{
	lsd_begin()
	lsd_in_number("N", lsi0)
	}
	if (lsd_enter(217, ctx))
		return
	if (g_lsgLeft[217] >= floatround(lsi0))
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
stock lsg_n218(ctx[])
{
	if (lsd_want(218))
	{
	lsd_begin()
	}
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
	if (lsd_want(219))
	{
	lsd_begin()
	}
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
	new lsi0 = ctx[0]
	if (lsd_want(220))
	{
	lsd_begin()
	lsd_in_entity("Object", lsi0)
	}
	if (lsd_enter(220, ctx))
		return
	if (lsg_valid(lsi0))
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
	new Float:lsi0 = 2.0
	if (lsd_want(221))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(221, ctx))
		return
	remove_task(LSG_TASK + 221)
	set_task(floatmax(lsi0, 0.1), "lsg_t221", LSG_TASK + 221, ctx, LSG_CTX)
	lsd_exit(221, "waiting")
}

public lsg_t221(const data[], task)
{
	if (lsd_timer_wait(221, "lsg_t221", data))
		return
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
	new lsi0[128]
	copy(lsi0, 127, lsg_ntext(Float:g_lsgO[5]))
	if (lsd_want(222))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(222, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Count:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(222, esc)
	lsg_n223(ctx)
}

// Print String (e_pr2)
lsg_n223(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgYesNo[(g_lsgO[6]) ? 1 : 0])
	if (lsd_want(223))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(223, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Was A:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(223, esc)
	lsg_n217_reset(ctx)
}

// Sequence (e_seq)
lsg_n224(ctx[])
{
	if (lsd_want(224))
	{
	lsd_begin()
	}
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
	if (lsd_want(225))
	{
	lsd_begin()
	}
	if (lsd_enter(225, ctx))
		return
	lsd_exit(225, "ran")
	lsg_n226(ctx)
}

// For Loop with Break (f_brk)
stock lsg_n226(ctx[])
{
	new Float:lsi0 = 0.0
	new Float:lsi1 = 10.0
	if (lsd_want(226))
	{
	lsd_begin()
	lsd_in_number("First", lsi0)
	lsd_in_number("Last", lsi1)
	}
	if (lsd_enter(226, ctx))
		return
	new c[LSG_CTX], n
	g_lsgLeft[226] = 0
	new last = floatround(lsi1)
	for (new k = floatround(lsi0); k <= last && n < LSG_AMAX; k++)
	{
		n++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[22] = _:float(k)
		lsg_n227(c)
		if (g_lsgLeft[226])
			break
	}
	lsd_exit(226, lsd_times(n))
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
	new lsi0 = ((Float:ctx[22]) >= (3.0) && (Float:ctx[22]) <= (5.0) ? 1 : 0)
	if (lsd_want(227))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(227, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(227, g_lsdBool[ok])
	if (ok)
		lsg_n226_break(ctx)
	else
		lsg_n228(ctx)
}

// Print String (f_pr)
lsg_n228(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("%s %s %s %s", lsg_ntext(floatabs(Float:ctx[22])), lsg_ntext(((Float:ctx[22]) + ((10.0) - (Float:ctx[22])) * (0.25))), lsg_ntext((((((Float:ctx[22]) == (2.0) ? 1 : 0)) ? 1 : 0) ? (floatsqroot(floatmax(Float:ctx[22], 0.0))) : (lsg_mod(Float:ctx[22], 3.0)))), lsg_ntext(lsg_pow(Float:ctx[22], 2.0))))
	if (lsd_want(228))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(228, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Maths:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(228, esc)
}

// Print String (f_pr2)
lsg_n229(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_tsel((((lsg_valid(ctx[0]) ? 1 : 0)) ? 1 : 0), "valid", "gone"))
	if (lsd_want(229))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(229, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Player is", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(229, esc)
	lsg_n230(ctx)
}

// Print String (f_pr3)
lsg_n230(ctx[])
{
	new lsi0 = (lsg_ok(ctx[0]) ? 1 : 0)
	if (lsd_want(230))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(230, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Is a player:", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(230, esc)
	lsg_n231(ctx)
}

// Print String (f_pr4)
lsg_n231(ctx[])
{
	new lsi0 = (lsg_is_creature(ctx[0]) ? 1 : 0)
	if (lsd_want(231))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(231, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Is a creature:", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(231, esc)
	lsg_n232(ctx)
}

// Set Variable (f_setc)
lsg_n232(ctx[])
{
	new lsi0 = ctx[0]
	new lsi2 = 2
	if (lsd_want(232))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Value", g_lsgE0[clamp(lsi2, 0, 2)])
	}
	if (lsd_enter(232, ctx))
		return
	g_pv16[lsg_slot(lsi0)] = lsi2
	lsd_exit(232, "set")
	lsg_n233(ctx)
}

// Switch on Enum (f_sw)
lsg_n233(ctx[])
{
	new lsi0 = g_pv16[lsg_slot(ctx[0])]
	if (lsd_want(233))
	{
	lsd_begin()
	lsd_in_text("Selection", g_lsgE0[clamp(lsi0, 0, 2)])
	}
	if (lsd_enter(233, ctx))
		return
	switch (lsi0)
	{
		case 0:
		{
			lsd_exit(233, "Scout")
		}
		case 1:
		{
			lsd_exit(233, "Tank")
		}
		case 2:
		{
			lsd_exit(233, "Medic")
			lsg_n234(ctx)
		}
		default:
		{
			lsd_exit(233, "default")
		}
	}
}

// Print String (f_pr5)
lsg_n234(ctx[])
{
	new lsi0 = ((g_pv16[lsg_slot(ctx[0])]) == (2) ? 1 : 0)
	if (lsd_want(234))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(234, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Medic:", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(234, esc)
	lsg_n235(ctx)
}

// Set Variable (f_sp)
lsg_n235(ctx[])
{
	new lsi2 = lsg_aplayers(1)
	if (lsd_want(235))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi2)))
	}
	if (lsd_enter(235, ctx))
		return
	lsg_acopy(g_v10, lsi2)
	lsd_exit(235, "set")
	lsg_n236(ctx)
}

// Print String (f_pr6)
lsg_n236(ctx[])
{
	new Float:lsi0 = float(lsg_alen(lsg_aplayers(1)))
	if (lsd_want(236))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(236, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Alive:", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(236, esc)
}

// Function Entry (fa_e)
stock lsg_n237(ctx[])
{
	if (lsd_want(237))
	{
	lsd_begin()
	}
	if (lsd_enter(237, ctx))
		return
	lsd_exit(237, "called")
	lsg_n238(ctx)
}

// Set Variable (fa_set)
stock lsg_n238(ctx[])
{
	new Float:lsi2 = ((Float:ctx[24]) + (g_v18))
	if (lsd_want(238))
	{
	lsd_begin()
	lsd_in_number("Value", lsi2)
	}
	if (lsd_enter(238, ctx))
		return
	g_v18 = lsi2
	lsd_exit(238, fmt("%.2f", g_v18))
	lsg_n239(ctx)
}

// Add (fa_push)
stock lsg_n239(ctx[])
{
	new lsi0 = g_v19
	new lsi1 = ctx[23]
	if (lsd_want(239))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	lsd_in_player("Item", lsi1)
	}
	if (lsd_enter(239, ctx))
		return
	g_lsgO[7] = _:float(lsg_apushc(lsi0, lsi1))
	lsd_exit(239, fmt("%d items", lsg_alen(lsi0)))
	lsg_n240(ctx)
}

// Branch (fa_br)
stock lsg_n240(ctx[])
{
	new lsi0 = (ls_alive(ctx[23]) ? 1 : 0)
	if (lsd_want(240))
	{
	lsd_begin()
	lsd_in_bool("Condition", lsi0)
	}
	if (lsd_enter(240, ctx))
		return
	new ok = ((((lsi0) ? 1 : 0)) ? 1 : 0)
	lsd_exit(240, g_lsdBool[ok])
	if (ok)
		lsg_n241(ctx)
	else
		lsg_n242(ctx)
}

// Return Node (fa_r1)
stock lsg_n241(ctx[])
{
	new Float:lsi0 = g_v18
	new lsi1[128]
	copy(lsi1, 127, lsg_tcat(g_lsgOT[1], "!"))
	if (lsd_want(241))
	{
	lsd_begin()
	lsd_in_number("Total", lsi0)
	lsd_in_text("Message", lsi1)
	}
	if (lsd_enter(241, ctx))
		return
	g_lsgR0_0 = lsi0
	copy(g_lsgR0_1, 127, lsi1)
	g_lsgFD0 = true
	lsd_exit(241, "returned")
}

// Return Node (fa_r2)
stock lsg_n242(ctx[])
{
	if (lsd_want(242))
	{
	lsd_begin()
	}
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
	if (lsd_want(243))
	{
	lsd_begin()
	}
	if (lsd_enter(243, ctx))
		return
	lsd_exit(243, "called")
	lsg_n244(ctx)
}

// Return Node (fb_r)
stock lsg_n244(ctx[])
{
	new Float:lsi0 = ((Float:ctx[25]) * (2.0))
	new lsi1[128]
	copy(lsi1, 127, lsg_ntext(((Float:ctx[25]) * (2.0))))
	if (lsd_want(244))
	{
	lsd_begin()
	lsd_in_number("Y", lsi0)
	lsd_in_text("As text", lsi1)
	}
	if (lsd_enter(244, ctx))
		return
	g_lsgR1_0 = lsi0
	copy(g_lsgR1_1, 127, lsi1)
	g_lsgFD1 = true
	lsd_exit(244, "returned")
}

// Function Entry (fc_e)
stock lsg_n245(ctx[])
{
	if (lsd_want(245))
	{
	lsd_begin()
	}
	if (lsd_enter(245, ctx))
		return
	lsd_exit(245, "called")
	lsg_n246(ctx)
}

// For Each Marker (mk_fe)
stock lsg_n246(ctx[])
{
	if (lsd_want(246))
	{
	lsd_begin()
	}
	if (lsd_enter(246, ctx))
		return
	new c[LSG_CTX], count
	for (new m = 0; m < g_lsgMkN; m++)
	{
		if (g_lsgMkKind[m] != 0)
			continue
		count++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[26] = g_lsgMkEnt[m]
		c[27] = _:float(count)
		lsg_n248(c)
		if (g_lsgFD2)
			return
	}
	lsd_exit(246, lsd_times(count))
	lsg_n247(ctx)
}

// Print String (fc_pr)
stock lsg_n247(ctx[])
{
	new Float:lsi0 = float(lsg_mk_count(0))
	if (lsd_want(247))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(247, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "tick", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(247, esc)
}

// Print String (mk_pr)
stock lsg_n248(ctx[])
{
	new lsi0 = lsg_vent(lsg_mk_get(1, floatround(str_to_float(lsg_mk_key(ctx[26], "wave")))))
	if (lsd_want(248))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(248, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "Route spot:", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(248, esc)
}

// Print String (d_mac)
lsg_n249(ctx[])
{
	new Float:lsi0 = 3.0
	if (lsd_want(249))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(249, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "In the macro:", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(249, esc)
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
		case 248: lsg_n248(ctx)
		case 249: lsg_n249(ctx)
	}
}
