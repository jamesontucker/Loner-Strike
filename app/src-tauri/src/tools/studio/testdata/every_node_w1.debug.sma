/*
	Every Node W1 - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from every_node_w1.graph.json: change the graph and compile it again; edits here are lost.
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

#define LSG_MODE "every_node_w1"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 90
#define LSG_TASK 7310000
#pragma dynamic 61696
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }
#define LSG_VR 256
#define LSG_VP 256
#define LSG_RR 64
#define LSG_RP 64
#define LSG_RC 1
#define LSG_RT 1
#define LSG_AC 65
#define LSG_AMAX 10000
#define LSG_TMPA 32
#define LSG_O 3
#define LSG_OT 1

// The mode's variables
stock g_v0 = 0 // Flag (bool)
stock g_pv1[33] // Player flag (bool, per player)
stock g_tv2[4] = { 1, 1, 1, 1 } // Team flag (bool, per team)

#define LSD_N 90
new const g_lsdIds[LSD_N][] = { "ps", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14", "p15", "p16", "p17", "p18", "p19", "p20", "p21", "p22", "p23", "p24", "p25", "p26", "p27", "p28", "p29", "p30", "p31", "p32", "p33", "p34", "p35", "p36", "p37", "p38", "p39", "p40", "p41", "p42", "p43", "p44", "p45", "p46", "p47", "p48", "p49", "p50", "p51", "p52", "p53", "p54", "p55", "p56", "p57", "p58", "p59", "p60", "p61", "p62", "p63", "p64", "p65", "dl", "db", "dp", "log", "tg", "p66", "tgp", "tgt", "p67", "rev", "seq", "dopp", "mg", "mgp0", "mgp1", "mgp2", "mgr", "mgrp", "rs", "tick", "tp1", "tp2", "fast", "fp" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
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
// (a mode loads at most 160 files of its own: the game's 512 models and 512 sounds are for
// everything on the server; limits.rs PRECACHE_MAX)
stock g_lsgPreN
stock bool:lsg_pre_room(const p[])
{
	if (g_lsgPreN < 160)
	{
		g_lsgPreN++
		return true
	}
	log_amx("[%s] %s not loaded: the mode has loaded 160 files of its own already", LSG_MODE, p)
	return false
}

stock lsg_pre_model(const p[])
{
	if (!file_exists(p, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, p)
		return 0
	}
	if (!lsg_pre_room(p))
		return 0
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
	if (!lsg_pre_room(full))
		return 0
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_sound(p)
}

// The per-frame message budget (the Effect messages a frame limit; a unicast counts against its
// player too, at half the budget). Two kinds of message use it:
//  - cosmetic (effects, a sound, a chat line): lsg_msg_room; past the budget it's skipped, said in
//    the log once a second
//  - state (a scoreboard's team or tag, a score, an icon, weather: what lasts): lsg_msg_state;
//    never dropped. Past the budget it waits in a queue that goes out on the next frames, in
//    order; the same resend and key queued again keeps its place, and the resend reads the state
//    when it goes, so the latest value wins
stock Float:g_lsgMsgAt, g_lsgMsgN, g_lsgMsgP[33], Float:g_lsgMsgSaid, g_lsgMsgLim = 48
stock bool:lsg_msg_fits(cost, to, lim)
{
	if (lim <= 0)
		lim = g_lsgMsgLim
	new Float:now = get_gametime()
	if (now != g_lsgMsgAt)
	{
		g_lsgMsgAt = now
		g_lsgMsgN = 0
		arrayset(g_lsgMsgP, 0, sizeof g_lsgMsgP)
	}
	if (g_lsgMsgN + cost > lim || (to >= 1 && to <= 32 && g_lsgMsgP[to] + cost > lim / 2))
		return false
	g_lsgMsgN += cost
	if (to >= 1 && to <= 32)
		g_lsgMsgP[to] += cost
	return true
}

stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0)
{
	if (lsg_msg_fits(cost, to, lim))
		return true
	new Float:now = get_gametime()
	if (now - g_lsgMsgSaid >= 1.0)
	{
		g_lsgMsgSaid = now
		log_amx("[%s] %s skipped: too many effects this frame", LSG_MODE, what)
	}
	return false
}

// a state message: true, send it now; false, it's queued (resend: a public taking the key, which
// sends the state as it is then; key: whose, usually the player)
#define LSG_MSGQ 512
stock g_lsgMsgQFn[LSG_MSGQ][32], g_lsgMsgQKey[LSG_MSGQ], g_lsgMsgQCost[LSG_MSGQ], g_lsgMsgQTo[LSG_MSGQ], g_lsgMsgQN, bool:g_lsgMsgQAsked, bool:g_lsgMsgQSending, g_lsgMsgQWaited, g_lsgMsgQFrames
stock bool:lsg_msg_state(const what[], const resend[], key, cost = 1, to = 0, lim = 0)
{
	// (the queue sending it: its room is taken already)
	if (g_lsgMsgQSending)
		return true
	// (others waiting: it waits behind them, so the order holds)
	if (!g_lsgMsgQN && lsg_msg_fits(cost, to, lim))
		return true
	for (new i = 0; i < g_lsgMsgQN; i++)
		if (g_lsgMsgQKey[i] == key && equal(g_lsgMsgQFn[i], resend))
			return false
	if (g_lsgMsgQN >= LSG_MSGQ)
	{
		// (never dropped: over the queue too, it goes now)
		log_amx("[%s] %s sent over the budget: the queue of state messages is full", LSG_MODE, what)
		return true
	}
	copy(g_lsgMsgQFn[g_lsgMsgQN], 31, resend)
	g_lsgMsgQKey[g_lsgMsgQN] = key
	g_lsgMsgQCost[g_lsgMsgQN] = cost
	g_lsgMsgQTo[g_lsgMsgQN] = to
	g_lsgMsgQN++
	g_lsgMsgQWaited++
	if (!g_lsgMsgQAsked)
	{
		g_lsgMsgQAsked = true
		RequestFrame("lsg_msg_flush")
	}
	return false
}

// the queue: as many as the frame's budget holds, in order; the rest on the next frame
public lsg_msg_flush()
{
	g_lsgMsgQAsked = false
	g_lsgMsgQFrames++
	new done = 0, fn[32]
	while (done < g_lsgMsgQN && lsg_msg_fits(g_lsgMsgQCost[done], g_lsgMsgQTo[done], 0))
	{
		copy(fn, charsmax(fn), g_lsgMsgQFn[done])
		new key = g_lsgMsgQKey[done]
		done++
		g_lsgMsgQSending = true
		if (callfunc_begin(fn) == 1)
		{
			callfunc_push_int(key)
			callfunc_end()
		}
		g_lsgMsgQSending = false
	}
	// (a frame with no room at all still sends one: the queue always moves)
	if (!done && g_lsgMsgQN)
	{
		copy(fn, charsmax(fn), g_lsgMsgQFn[0])
		g_lsgMsgQSending = true
		if (callfunc_begin(fn) == 1)
		{
			callfunc_push_int(g_lsgMsgQKey[0])
			callfunc_end()
		}
		g_lsgMsgQSending = false
		done = 1
	}
	for (new i = done; i < g_lsgMsgQN; i++)
	{
		copy(g_lsgMsgQFn[i - done], 31, g_lsgMsgQFn[i])
		g_lsgMsgQKey[i - done] = g_lsgMsgQKey[i]
		g_lsgMsgQCost[i - done] = g_lsgMsgQCost[i]
		g_lsgMsgQTo[i - done] = g_lsgMsgQTo[i]
	}
	g_lsgMsgQN -= done
	if (g_lsgMsgQN && !g_lsgMsgQAsked)
	{
		g_lsgMsgQAsked = true
		RequestFrame("lsg_msg_flush")
	}
	else if (!g_lsgMsgQN)
	{
		log_amx("[%s] %d state messages waited for room in the frame's budget: all sent, over %d more frames", LSG_MODE, g_lsgMsgQWaited, g_lsgMsgQFrames)
		g_lsgMsgQWaited = 0
		g_lsgMsgQFrames = 0
	}
}

// room for one more entity: free slots are kept for the game (it crashes with "no free edicts");
// a mode's spawn that would eat into them is skipped, and the server log says what and why
#define LSG_EDICT_RESERVE 96
stock bool:lsg_ent_room(const what[])
{
	if (engfunc(EngFunc_NumberOfEntities) < global_get(glb_maxEntities) - LSG_EDICT_RESERVE)
		return true
	log_amx("[%s] %s skipped: the server is out of entity slots", LSG_MODE, what)
	return false
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
	// (any entity handle: a removed one, or one going this frame, isn't valid)
	return e > MaxClients && pev_valid(e) > 0 && !(pev(e, pev_flags) & FL_KILLME)
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


// ---- W1's utility nodes: spatial queries, random picks, text, number arrays, vector math, relative
// space and aim. How many entities a search keeps and how far an aim trace goes are the node's
// own settings; every index and length is clamped and every loop bounded (4096 entities looked at).
stock Float:g_lsgW1Wt[64], g_lsgW1Wk[64]
stock Float:g_lsgW1AimT[33], Float:g_lsgW1AimD[33], Float:g_lsgW1AimPos[33][3], Float:g_lsgW1AimNrm[33][3], g_lsgW1AimEnt[33], g_lsgW1AimHit[33]

// the point a query is around: a point (a vector slot) or an entity (its position; a player's
// feet); false when there's neither
stock bool:lsg_w1_at(pos, ent, Float:o[3])
{
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		return true
	}
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		return true
	}
	return false
}

stock bool:lsg_w1_pfits(p, alive, team, bots)
{
	if (!is_user_connected(p) || (alive && !is_user_alive(p)))
		return false
	if (team && get_user_team(p) != team)
		return false
	if (bots == 1 && is_user_bot(p))
		return false
	if (bots == 2 && !is_user_bot(p))
		return false
	return true
}

stock lsg_w1_pin(pos, ent, Float:r, alive, team)
{
	new a = lsg_atmp(), Float:at[3], Float:o[3]
	if (!lsg_w1_at(pos, ent, at))
		return a
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == ent || !lsg_w1_pfits(p, alive, team, 0))
			continue
		pev(p, pev_origin, o)
		if (get_distance_f(o, at) <= r)
			ArrayPushCell((Array:a), p)
	}
	return a
}

stock lsg_w1_np(pos, ent, Float:maxd, alive, team, bots, bool:dist = false)
{
	new Float:at[3], Float:o[3], best, Float:bd = maxd
	if (lsg_w1_at(pos, ent, at))
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == ent || !lsg_w1_pfits(p, alive, team, bots))
				continue
			pev(p, pev_origin, o)
			new Float:d = get_distance_f(o, at)
			if (d <= bd)
			{
				bd = d
				best = p
			}
		}
	return dist ? (best ? (_:bd) : (_:-1.0)) : best
}

stock lsg_w1_ein(pos, ent, Float:r, const cls[], most)
{
	new a = lsg_atmp(), Float:at[3], Float:o[3], e = MaxClients, n, looked
	if (!lsg_w1_at(pos, ent, at))
		return a
	if (cls[0])
	{
		while (n < most && looked++ < 4096 && (e = engfunc(EngFunc_FindEntityByString, e, "classname", cls)) > 0)
		{
			if (e == ent || e <= MaxClients)
				continue
			pev(e, pev_origin, o)
			if (get_distance_f(o, at) <= r)
			{
				ArrayPushCell((Array:a), e)
				n++
			}
		}
		return a
	}
	while (n < most && looked++ < 4096 && (e = engfunc(EngFunc_FindEntityInSphere, e, at, r)) > 0)
	{
		if (e == ent || e <= MaxClients)
			continue
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_w1_ne(pos, ent, const cls[], Float:maxd, bool:dist = false)
{
	new Float:at[3], Float:o[3], e = MaxClients, best, Float:bd = maxd, looked
	if (cls[0] && lsg_w1_at(pos, ent, at))
		while (looked++ < 4096 && (e = engfunc(EngFunc_FindEntityByString, e, "classname", cls)) > 0)
		{
			if (e == ent || e <= MaxClients)
				continue
			pev(e, pev_origin, o)
			new Float:d = get_distance_f(o, at)
			if (d <= bd)
			{
				bd = d
				best = e
			}
		}
	return dist ? (best ? (_:bd) : (_:-1.0)) : best
}

// an end of a line: an entity (a player's eyes) or a point
stock lsg_w1_end(ent, pos, Float:o[3])
{
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		if (ent <= MaxClients)
		{
			new Float:ofs[3]
			pev(ent, pev_view_ofs, ofs)
			o[2] += ofs[2]
		}
		return
	}
	if (pos >= 0)
		lsg_vto(pos, o)
}

// a clear line from A to B? (the entities at the ends don't block it); hit: where it stopped
stock lsg_w1_los(aent, apos, bent, bpos, monsters, bool:hit = false)
{
	new Float:a[3], Float:b[3], Float:frac, Float:end[3]
	lsg_w1_end(aent, apos, a)
	lsg_w1_end(bent, bpos, b)
	engfunc(EngFunc_TraceLine, a, b, monsters ? DONT_IGNORE_MONSTERS : IGNORE_MONSTERS, aent > 0 ? aent : 0, 0)
	get_tr2(0, TR_flFraction, frac)
	get_tr2(0, TR_vecEndPos, end)
	new t = get_tr2(0, TR_pHit)
	if (hit)
		return lsg_vfrom(frac >= 1.0 ? b : end)
	return (frac >= 1.0 || (bent > 0 && t == bent)) ? 1 : 0
}

// a weighted pick (one roll per node per server frame, so Index and Item agree)
stock lsg_w1_wpick(w, node)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (g_lsgW1Wt[k] == now && now > 0.0)
		return g_lsgW1Wk[k]
	new n = min(lsg_alen(w), LSG_AMAX), Float:total = 0.0
	for (new i = 0; i < n; i++)
		total += floatmax(Float:ArrayGetCell((Array:w), i), 0.0)
	new pick = -1
	if (total > 0.0)
	{
		new Float:r = random_float(0.0, total)
		for (new i = 0; i < n; i++)
		{
			r -= floatmax(Float:ArrayGetCell((Array:w), i), 0.0)
			if (r <= 0.0 && Float:ArrayGetCell((Array:w), i) > 0.0)
			{
				pick = i
				break
			}
		}
		if (pick < 0)
			for (new i = n - 1; i >= 0 && pick < 0; i--)
				if (Float:ArrayGetCell((Array:w), i) > 0.0)
					pick = i
	}
	g_lsgW1Wt[k] = now
	g_lsgW1Wk[k] = pick
	return pick
}

stock Float:lsg_w1_map(Float:x, Float:a, Float:b, Float:c, Float:d, clamp_it)
{
	if (floatabs(b - a) < 0.000001)
		return c
	new Float:t = (x - a) / (b - a)
	if (clamp_it)
		t = floatclamp(t, 0.0, 1.0)
	return c + (d - c) * t
}

// ---- text (every result fits 128 cells; every position clamped)
stock lsg_w1_split(const s[], const sep[])
{
	new a = lsg_atmp(), piece[128], n, len = strlen(s), sl = strlen(sep), start
	if (!sl)
	{
		ArrayPushString((Array:a), s)
		return a
	}
	for (new i = 0; i <= len && n < 64; i++)
	{
		if (i == len || (i + sl <= len && equal(s[i], sep, sl)))
		{
			new c = min(i - start, charsmax(piece))
			copy(piece, c, s[start])
			ArrayPushString((Array:a), piece)
			n++
			start = i + sl
			i += sl - 1
		}
	}
	return a
}

stock lsg_w1_repl(const s[], const find[], const with[])
{
	new t[128]
	copy(t, charsmax(t), s)
	if (find[0])
		replace_all(t, charsmax(t), find, with)
	return t
}

stock lsg_w1_sub(const s[], Float:start, Float:len)
{
	new t[128], l = strlen(s)
	new b = clamp(floatround(start, floatround_floor), 0, l), n = clamp(floatround(len, floatround_floor), 0, charsmax(t))
	copy(t, min(n, l - b), s[b])
	return t
}

stock lsg_w1_case(const s[], up)
{
	new t[128]
	copy(t, charsmax(t), s)
	if (up)
		strtoupper(t)
	else
		strtolower(t)
	return t
}

stock lsg_w1_trim(const s[])
{
	new t[128]
	copy(t, charsmax(t), s)
	trim(t)
	return t
}

stock bool:lsg_w1_starts(const s[], const p[])
	return equali(s, p, strlen(p)) != 0

stock bool:lsg_w1_ends(const s[], const p[])
{
	new l = strlen(s), m = strlen(p)
	return m <= l && equali(s[l - m], p) != 0
}

stock lsg_w1_join(a, const sep[])
{
	new t[128], piece[128], n = min(lsg_alen(a), LSG_AMAX)
	for (new i = 0; i < n; i++)
	{
		ArrayGetString((Array:a), i, piece, charsmax(piece))
		if (i)
			add(t, charsmax(t), sep)
		add(t, charsmax(t), piece)
	}
	return t
}

// 0 auto (hours only when there are any), 1 m:ss, 2 h:mm:ss
stock lsg_w1_time(Float:secs, style)
{
	new t[32], s = max(floatround(secs, floatround_floor), 0)
	new h = s / 3600, m = (s / 60) % 60, x = s % 60
	if (style == 2 || (style == 0 && h > 0))
		formatex(t, charsmax(t), "%d:%02d:%02d", h, m, x)
	else
		formatex(t, charsmax(t), "%d:%02d", s / 60, x)
	return t
}

stock lsg_w1_num(Float:v, decimals, thousands)
{
	new t[48], raw[40], fmt[8]
	formatex(fmt, charsmax(fmt), "%%.%df", clamp(decimals, 0, 4))
	// (AMX Mod X's %f cuts the digits off: half a unit of the last decimal first, so it rounds)
	formatex(raw, charsmax(raw), fmt, floatabs(v) + 0.5 / floatpower(10.0, float(clamp(decimals, 0, 4))))
	if (!thousands)
	{
		formatex(t, charsmax(t), "%s%s", v < 0.0 ? "-" : "", raw)
		return t
	}
	new dot = contain(raw, "."), digits = dot >= 0 ? dot : strlen(raw), o
	if (v < 0.0)
		t[o++] = '-'
	for (new i = 0; i < digits && o < charsmax(t) - 1; i++)
	{
		if (i > 0 && (digits - i) % 3 == 0)
			t[o++] = ','
		t[o++] = raw[i]
	}
	t[o] = 0
	if (dot >= 0)
		add(t, charsmax(t), raw[dot])
	return t
}

// ---- number arrays: 0 sum, 1 min, 2 max, 3 average; the index of the min (1) or max (2)
stock Float:lsg_w1_astat(a, op, bool:idx = false)
{
	new n = min(lsg_alen(a), LSG_AMAX), best = -1
	new Float:acc = 0.0
	for (new i = 0; i < n; i++)
	{
		new Float:x = Float:ArrayGetCell((Array:a), i)
		switch (op)
		{
			case 1: if (best < 0 || x < acc) { acc = x; best = i; }
			case 2: if (best < 0 || x > acc) { acc = x; best = i; }
			default: acc += x
		}
	}
	if (idx)
		return float(best)
	if (op == 3)
		return n ? acc / float(n) : 0.0
	return acc
}

stock lsg_w1_areverse(a)
{
	new n = lsg_alen(a)
	for (new i = 0, j = n - 1; i < j; i++, j--)
		ArraySwap((Array:a), i, j)
}

stock lsg_w1_slice(a, Float:start, Float:count)
{
	new out = lsg_atmp(), n = lsg_alen(a), b[LSG_AC]
	new s = clamp(floatround(start, floatround_floor), 0, n), c = clamp(floatround(count, floatround_floor), 0, LSG_AMAX)
	for (new k = s; k < n && k < s + c; k++)
	{
		ArrayGetArray((Array:a), k, b)
		ArrayPushArray((Array:out), b)
	}
	return out
}

// ---- vectors
stock Float:lsg_w1_dot(a, b)
	return lsg_vc(a, 0) * lsg_vc(b, 0) + lsg_vc(a, 1) * lsg_vc(b, 1) + lsg_vc(a, 2) * lsg_vc(b, 2)

stock lsg_w1_cross(a, b)
	return lsg_vec(lsg_vc(a, 1) * lsg_vc(b, 2) - lsg_vc(a, 2) * lsg_vc(b, 1), lsg_vc(a, 2) * lsg_vc(b, 0) - lsg_vc(a, 0) * lsg_vc(b, 2), lsg_vc(a, 0) * lsg_vc(b, 1) - lsg_vc(a, 1) * lsg_vc(b, 0))

stock lsg_w1_rnd(c, Float:r, ball)
{
	// uniform in the circle / ball (a root of the radius' share, else it bunches in the middle)
	new Float:yaw = random_float(0.0, 360.0)
	if (!ball)
	{
		new Float:d = r * floatsqroot(random_float(0.0, 1.0))
		return lsg_vec(lsg_vc(c, 0) + d * floatcos(yaw, degrees), lsg_vc(c, 1) + d * floatsin(yaw, degrees), lsg_vc(c, 2))
	}
	new Float:z = random_float(-1.0, 1.0), Float:d = r * floatpower(random_float(0.0, 1.0), 1.0 / 3.0)
	new Float:ring = floatsqroot(floatmax(1.0 - z * z, 0.0))
	return lsg_vec(lsg_vc(c, 0) + d * ring * floatcos(yaw, degrees), lsg_vc(c, 1) + d * ring * floatsin(yaw, degrees), lsg_vc(c, 2) + d * z)
}

stock lsg_w1_rot(a, Float:yaw, Float:pitch)
{
	new Float:x = lsg_vc(a, 0), Float:y = lsg_vc(a, 1), Float:z = lsg_vc(a, 2)
	// pitch first (up is negative pitch in GoldSrc), in the vector's own vertical plane
	if (pitch != 0.0)
	{
		new Float:h = floatsqroot(x * x + y * y), Float:cp = floatcos(-pitch, degrees), Float:sp = floatsin(-pitch, degrees)
		new Float:h2 = h * cp - z * sp, Float:z2 = h * sp + z * cp
		if (h > 0.0001)
		{
			x = x / h * h2
			y = y / h * h2
		}
		z = z2
	}
	new Float:cy = floatcos(yaw, degrees), Float:sy = floatsin(yaw, degrees)
	return lsg_vec(x * cy - y * sy, x * sy + y * cy, z)
}

stock lsg_w1_vlerp(a, b, Float:t)
	return lsg_vec(lsg_vc(a, 0) + (lsg_vc(b, 0) - lsg_vc(a, 0)) * t, lsg_vc(a, 1) + (lsg_vc(b, 1) - lsg_vc(a, 1)) * t, lsg_vc(a, 2) + (lsg_vc(b, 2) - lsg_vc(a, 2)) * t)

stock lsg_w1_toang(a)
{
	new Float:d[3], Float:ang[3]
	lsg_vto(a, d)
	vector_to_angle(d, ang)
	// (vector_to_angle gives pitch up as positive; a view's pitch is down-positive)
	return lsg_vec(-ang[0], ang[1], 0.0)
}

stock lsg_w1_fwd(a)
{
	new Float:ang[3], Float:f[3]
	lsg_vto(a, ang)
	angle_vector(ang, ANGLEVECTOR_FORWARD, f)
	return lsg_vfrom(f)
}

// ---- relative space: a thing's facing (a player's view yaw, else its angles' yaw)
stock Float:lsg_w1_yaw(e)
{
	new Float:ang[3]
	if (e > 0 && e <= MaxClients)
		pev(e, pev_v_angle, ang)
	else if (e > 0 && pev_valid(e))
		pev(e, pev_angles, ang)
	return ang[1]
}

stock lsg_w1_local(e, pos)
{
	new Float:o[3], Float:p[3]
	if (e > 0 && pev_valid(e))
		pev(e, pev_origin, o)
	lsg_vto(pos, p)
	new Float:dx = p[0] - o[0], Float:dy = p[1] - o[1], Float:y = lsg_w1_yaw(e)
	new Float:c = floatcos(y, degrees), Float:s = floatsin(y, degrees)
	// forward along the facing, right is clockwise from it (GoldSrc yaw turns left)
	return lsg_vec(dx * c + dy * s, dx * s - dy * c, p[2] - o[2])
}

stock lsg_w1_world(e, off)
{
	new Float:o[3]
	if (e > 0 && pev_valid(e))
		pev(e, pev_origin, o)
	new Float:f = lsg_vc(off, 0), Float:r = lsg_vc(off, 1), Float:y = lsg_w1_yaw(e)
	new Float:c = floatcos(y, degrees), Float:s = floatsin(y, degrees)
	return lsg_vec(o[0] + f * c + r * s, o[1] + f * s - r * c, o[2] + lsg_vc(off, 2))
}

stock Float:lsg_w1_relc(a, b, k)
{
	new Float:o[3]
	if (b > 0 && pev_valid(b))
		pev(b, pev_origin, o)
	return lsg_vc(lsg_w1_local(a, lsg_vfrom(o)), k)
}

stock Float:lsg_w1_angto(a, b)
{
	new Float:o[3], Float:p[3]
	if (a > 0 && pev_valid(a))
		pev(a, pev_origin, o)
	if (b > 0 && pev_valid(b))
		pev(b, pev_origin, p)
	new Float:d = floatatan2(p[1] - o[1], p[0] - o[0], degrees) - lsg_w1_yaw(a)
	while (d > 180.0)
		d -= 360.0
	while (d < -180.0)
		d += 360.0
	return d
}

// B inside a cone round A's facing (back: round A's back)
stock bool:lsg_w1_cone(a, b, Float:cone, back)
{
	new Float:d = floatabs(lsg_w1_angto(a, b))
	if (back)
		d = 180.0 - d
	return d <= cone * 0.5
}

// ---- aim: one trace per player per frame and distance, its four answers kept
stock lsg_w1_aim(id, Float:maxd, what)
{
	new k = clamp(id, 0, 32)
	new Float:now = get_gametime()
	if (g_lsgW1AimT[k] != now || g_lsgW1AimD[k] != maxd || now <= 0.0)
	{
		g_lsgW1AimT[k] = now
		g_lsgW1AimD[k] = maxd
		g_lsgW1AimEnt[k] = 0
		g_lsgW1AimHit[k] = 0
		for (new j = 0; j < 3; j++)
		{
			g_lsgW1AimPos[k][j] = 0.0
			g_lsgW1AimNrm[k][j] = 0.0
		}
		if (lsg_ok(id))
		{
			new Float:eye[3], Float:ofs[3], Float:ang[3], Float:fwd[3], Float:end[3], Float:frac
			pev(id, pev_origin, eye)
			pev(id, pev_view_ofs, ofs)
			eye[2] += ofs[2]
			pev(id, pev_v_angle, ang)
			angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
			new Float:d = floatmax(maxd, 16.0)
			for (new j = 0; j < 3; j++)
				end[j] = eye[j] + fwd[j] * d
			engfunc(EngFunc_TraceLine, eye, end, DONT_IGNORE_MONSTERS, id, 0)
			get_tr2(0, TR_flFraction, frac)
			get_tr2(0, TR_vecEndPos, g_lsgW1AimPos[k])
			get_tr2(0, TR_vecPlaneNormal, g_lsgW1AimNrm[k])
			new t = get_tr2(0, TR_pHit)
			g_lsgW1AimHit[k] = frac < 1.0 ? 1 : 0
			g_lsgW1AimEnt[k] = t > 0 ? t : 0
		}
	}
	switch (what)
	{
		case 1: return g_lsgW1AimEnt[k]
		case 2: return lsg_vfrom(g_lsgW1AimNrm[k])
		case 3: return g_lsgW1AimHit[k]
	}
	return lsg_vfrom(g_lsgW1AimPos[k])
}

// ---- testing: debug drawings (Playtest and Test run builds only: a release build has none; each
// within the shared message budget, lsg_msg_room) and Log Line

stock lsg_w1_te(kind, const Float:p[3], const Float:q[3], r, g, bl, Float:secs)
{
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(kind)
	for (new k = 0; k < 3; k++)
		engfunc(EngFunc_WriteCoord, p[k])
	for (new k = 0; k < 3; k++)
		engfunc(EngFunc_WriteCoord, q[k])
	write_short(clamp(floatround(secs * 10.0), 1, 600))
	write_byte(r)
	write_byte(g)
	write_byte(bl)
	message_end()
}

stock lsg_w1_line(a, b, r, g, bl, Float:secs)
{
	new Float:p[3], Float:q[3]
	lsg_vto(a, p)
	lsg_vto(b, q)
	lsg_w1_te(TE_LINE, p, q, r, g, bl, secs)
}

// a box between two corners (any two opposite ones), or round a point (half: its half size)
stock lsg_w1_box(a, b, Float:half, r, g, bl, Float:secs)
{
	new Float:p[3], Float:q[3], Float:lo[3], Float:hi[3]
	lsg_vto(a, p)
	if (b >= 0)
		lsg_vto(b, q)
	for (new k = 0; k < 3; k++)
	{
		if (b < 0)
		{
			lo[k] = p[k] - half
			hi[k] = p[k] + half
		}
		else
		{
			lo[k] = floatmin(p[k], q[k])
			hi[k] = floatmax(p[k], q[k])
		}
	}
	lsg_w1_te(TE_BOX, lo, hi, r, g, bl, secs)
}

stock Float:g_lsgW1LogT[64], g_lsgW1LogN[64], g_lsgW1LogSkip[64]
// to the AMX Mod X log, at most `most` lines a second from one node (the rest counted, and the
// count written with the next line that goes through)
stock bool:lsg_w1_log(node, const s[], most)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (now - g_lsgW1LogT[k] >= 1.0 || now < g_lsgW1LogT[k])
	{
		g_lsgW1LogT[k] = now
		g_lsgW1LogN[k] = 0
	}
	if (++g_lsgW1LogN[k] > most)
	{
		g_lsgW1LogSkip[k]++
		return false
	}
	if (g_lsgW1LogSkip[k])
		log_amx("%s (%d lines skipped before this one: at most %d a second)", s, g_lsgW1LogSkip[k], most)
	else
		log_amx("%s", s)
	g_lsgW1LogSkip[k] = 0
	return true
}

// ---- whole-number bits: 0 and, 1 or, 2 xor, 3 not, 4 shift left, 5 shift right
stock Float:lsg_w1_bits(Float:a, Float:b, op)
{
	new x = floatround(a, floatround_tozero), y = floatround(b, floatround_tozero), r
	switch (op)
	{
		case 1: r = x | y
		case 2: r = x ^ y
		case 3: r = ~x
		case 4: r = x << clamp(y, 0, 31)
		case 5: r = x >> clamp(y, 0, 31)
		default: r = x & y
	}
	return float(r)
}

// ---- a free spot near a point: the point, then rings round it, a hull trace at each, at most
// `most` traces; one search per node per server frame (Found and Position agree). 0 Found, 1 position
stock Float:g_lsgW1FsT[64], g_lsgW1FsOk[64], Float:g_lsgW1FsPos[64][3]
stock bool:lsg_w1_fits(const Float:p[3], hull)
{
	engfunc(EngFunc_TraceHull, p, p, DONT_IGNORE_MONSTERS, hull, 0, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid) && get_tr2(0, TR_InOpen)
}

stock lsg_w1_free(node, c, hull, Float:maxr, most, what)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (g_lsgW1FsT[k] != now || now <= 0.0)
	{
		g_lsgW1FsT[k] = now
		g_lsgW1FsOk[k] = 0
		new Float:o[3], Float:p[3]
		lsg_vto(c, o)
		g_lsgW1FsPos[k] = o
		new Float:step = hull == HULL_POINT ? 8.0 : (hull == HULL_LARGE ? 68.0 : 36.0)
		new tries = 1
		if (lsg_w1_fits(o, hull))
			g_lsgW1FsOk[k] = 1
		for (new ring = 1; !g_lsgW1FsOk[k] && float(ring) * step <= maxr && tries < most; ring++)
		{
			new n = min(8 * ring, 32)
			for (new j = 0; j < n && tries < most; j++, tries++)
			{
				new Float:ang = 360.0 * float(j) / float(n), Float:d = float(ring) * step
				p[0] = o[0] + d * floatcos(ang, degrees)
				p[1] = o[1] + d * floatsin(ang, degrees)
				p[2] = o[2]
				if (lsg_w1_fits(p, hull))
				{
					g_lsgW1FsOk[k] = 1
					g_lsgW1FsPos[k] = p
					break
				}
			}
		}
	}
	return what ? lsg_vfrom(g_lsgW1FsPos[k]) : g_lsgW1FsOk[k]
}

// ---- a player's language: AMX Mod X's (their "lang" setting, else the server's), or the game's
// (cl_language, asked of their game once: empty until it answers, and for bots)
stock g_lsgW1Lang[33][16], g_lsgW1LangUid[33]
stock lsg_w1_lang(id, game)
{
	new t[16]
	if (!lsg_ok(id))
		return t
	if (!game)
	{
		get_user_info(id, "lang", t, charsmax(t))
		if (!t[0])
			get_cvar_string("amx_language", t, charsmax(t))
		return t
	}
	if (is_user_bot(id))
		return t
	new uid = get_user_userid(id)
	if (g_lsgW1LangUid[id] != uid)
	{
		g_lsgW1LangUid[id] = uid
		g_lsgW1Lang[id][0] = 0
		query_client_cvar(id, "cl_language", "lsg_w1_langq")
	}
	copy(t, charsmax(t), g_lsgW1Lang[id])
	return t
}

public lsg_w1_langq(id, const cvar[], const value[])
{
	if (id < 1 || id > 32 || containi(value, "Bad CVAR") >= 0)
		return
	copy(g_lsgW1Lang[id], charsmax(g_lsgW1Lang[]), value)
}


// enum Entity Kind
stock const g_lsgE0[][] = { "Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None" }
// enum Component Kind
stock const g_lsgE1[][] = { "Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None" }
// enum Spawn Failure
stock const g_lsgE2[][] = { "None", "Blocked", "Out of entity slots", "Limit reached", "Missing file", "Retries used up", "Bad input" }
stock lsg_mka6(Float:p0, Float:p1)
{
	new a = lsg_atmp()
	lsg_apushc(a, _:(p0))
	lsg_apushc(a, _:(p1))
	return a
}

lsd_graph_vars()
{
	lsd_var_bool("flag", 0, g_v0)
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
			lsd_var_bool("pflag", id, g_pv1[id])
	lsd_var_bool("tflag:T", 0, g_tv2[1])
	lsd_var_bool("tflag:CT", 0, g_tv2[2])
}

lsd_graph_set(const vid[], who, const val[])
{
	if (equal(vid, "flag"))
	{
		g_v0 = str_to_num(val) ? 1 : 0
		return true
	}
	if (equal(vid, "pflag"))
	{
		g_pv1[clamp(who, 1, 32)] = str_to_num(val) ? 1 : 0
		return true
	}
	if (equal(vid, "tflag"))
	{
		g_tv2[clamp(who, 1, 2)] = str_to_num(val) ? 1 : 0
		return true
	}
	return false
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node W1", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	register_forward(FM_StartFrame, "lsg_w1_frame")
}

public lsg_w1_frame()
{
	static Float:at[2]
	new Float:now = get_gametime()
	if (now < at[0] || now - at[0] >= floatmax(0.1, 1.0 / float(100)))
	{
		new ctx[LSG_CTX]
		ctx[0] = _:(at[0] > 0.0 && now > at[0] ? now - at[0] : floatmax(0.1, 1.0 / float(100)))
		at[0] = now
		lsg_n85(ctx)
	}
	if (now < at[1] || now - at[1] >= floatmax(0.02, 1.0 / float(100)))
	{
		new ctx[LSG_CTX]
		ctx[0] = _:(at[1] > 0.0 && now > at[1] ? now - at[1] : floatmax(0.02, 1.0 / float(100)))
		at[1] = now
		lsg_n88(ctx)
	}
	return FMRES_IGNORED
}

public plugin_end()
{
	lsd_end()
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv1[id] = 0
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n84(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n0(ctx)
	return HAM_IGNORED
}

// Player Spawned (ps)
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

// Print String (p1)
lsg_n1(ctx[])
{
	new lsi0 = lsg_w1_pin(lsg_vec(0.0, 0.0, 64.0), 0, 500.0, 1, 0)
	if (lsd_want(1))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(1, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsi0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(1, esc)
	lsg_n2(ctx)
}

// Print String (p2)
lsg_n2(ctx[])
{
	new lsi0 = lsg_w1_np(-1, ctx[0], 2048.0, 1, 2, 2)
	if (lsd_want(2))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(2, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(2, esc)
	lsg_n3(ctx)
}

// Print String (p3)
lsg_n3(ctx[])
{
	new Float:lsi0 = Float:lsg_w1_np(-1, ctx[0], 2048.0, 1, 2, 2, true)
	if (lsd_want(3))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(3, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(3, esc)
	lsg_n4(ctx)
}

// Print String (p4)
lsg_n4(ctx[])
{
	new lsi0 = lsg_w1_ein(-1, ctx[0], 800.0, "info_player_start", 64)
	if (lsd_want(4))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(4, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsi0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(4, esc)
	lsg_n5(ctx)
}

// Print String (p5)
lsg_n5(ctx[])
{
	new lsi0 = lsg_w1_ein(lsg_vec(0.0, 0.0, 64.0), 0, 300.0, "", 64)
	if (lsd_want(5))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(5, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsi0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(5, esc)
	lsg_n6(ctx)
}

// Print String (p6)
lsg_n6(ctx[])
{
	new lsi0 = lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0)
	if (lsd_want(6))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(6, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(6, esc)
	lsg_n7(ctx)
}

// Print String (p7)
lsg_n7(ctx[])
{
	new Float:lsi0 = Float:lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0, true)
	if (lsd_want(7))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(7, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(7, esc)
	lsg_n8(ctx)
}

// Print String (p8)
lsg_n8(ctx[])
{
	new lsi0 = lsg_w1_los(ctx[0], -1, 0, lsg_vec(0.0, 0.0, 64.0), 1)
	if (lsd_want(8))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(8, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(8, esc)
	lsg_n9(ctx)
}

// Print String (p9)
lsg_n9(ctx[])
{
	new lsi0 = lsg_w1_los(ctx[0], -1, 0, lsg_vec(0.0, 0.0, 64.0), 1, true)
	if (lsd_want(9))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(9, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(9, esc)
	lsg_n10(ctx)
}

// Print String (p10)
lsg_n10(ctx[])
{
	new lsi0 = (random_float(0.0, 100.0) < (30.0) ? 1 : 0)
	if (lsd_want(10))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(10, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(10, esc)
	lsg_n11(ctx)
}

// Print String (p11)
lsg_n11(ctx[])
{
	new lsi0 = random_num(0, 1)
	if (lsd_want(11))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(11, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(11, esc)
	lsg_n12(ctx)
}

// Print String (p12)
lsg_n12(ctx[])
{
	new Float:lsi0 = float(lsg_w1_wpick(lsg_mka6(3.0, 1.0), 27))
	if (lsd_want(12))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(12, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(12, esc)
	lsg_n13(ctx)
}

// Print String (p13)
lsg_n13(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_agets(lsg_w1_split("  Hello, World,  again  ", ","), float(lsg_w1_wpick(lsg_mka6(3.0, 1.0), 27))))
	if (lsd_want(13))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(13, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(13, esc)
	lsg_n14(ctx)
}

// Print String (p14)
lsg_n14(ctx[])
{
	new Float:lsi0 = lsg_w1_map(5.0, 0.0, 10.0, 100.0, 200.0, 1)
	if (lsd_want(14))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(14, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(14, esc)
	lsg_n15(ctx)
}

// Print String (p15)
lsg_n15(ctx[])
{
	new Float:lsi0 = floatsin(30.0, degrees)
	if (lsd_want(15))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(15, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(15, esc)
	lsg_n16(ctx)
}

// Print String (p16)
lsg_n16(ctx[])
{
	new Float:lsi0 = floattan(0.5, radian)
	if (lsd_want(16))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(16, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(16, esc)
	lsg_n17(ctx)
}

// Print String (p17)
lsg_n17(ctx[])
{
	new Float:lsi0 = floatatan2(1.0, -1.0, degrees)
	if (lsd_want(17))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(17, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(17, esc)
	lsg_n18(ctx)
}

// Print String (p18)
lsg_n18(ctx[])
{
	new Float:lsi0 = ((3.142) * 57.2957795)
	if (lsd_want(18))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(18, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(18, esc)
	lsg_n19(ctx)
}

// Print String (p19)
lsg_n19(ctx[])
{
	new Float:lsi0 = float(floatround(-2.7, floatround_tozero))
	if (lsd_want(19))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(19, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(19, esc)
	lsg_n20(ctx)
}

// Print String (p20)
lsg_n20(ctx[])
{
	new lsi0 = (floatabs((0.1) - (0.1)) <= floatabs(0.001) ? 1 : 0)
	if (lsd_want(20))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(20, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(20, esc)
	lsg_n21(ctx)
}

// Print String (p21)
lsg_n21(ctx[])
{
	new lsi0 = lsg_w1_split("  Hello, World,  again  ", ",")
	if (lsd_want(21))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(21, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsi0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(21, esc)
	lsg_n22(ctx)
}

// Print String (p22)
lsg_n22(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_repl("  Hello, World,  again  ", "World", "There"))
	if (lsd_want(22))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(22, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(22, esc)
	lsg_n23(ctx)
}

// Print String (p23)
lsg_n23(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_sub("  Hello, World,  again  ", 2.0, 5.0))
	if (lsd_want(23))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(23, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(23, esc)
	lsg_n24(ctx)
}

// Print String (p24)
lsg_n24(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_case("  Hello, World,  again  ", 1))
	if (lsd_want(24))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(24, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(24, esc)
	lsg_n25(ctx)
}

// Print String (p25)
lsg_n25(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_case("  Hello, World,  again  ", 0))
	if (lsd_want(25))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(25, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(25, esc)
	lsg_n26(ctx)
}

// Print String (p26)
lsg_n26(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_trim("  Hello, World,  again  "))
	if (lsd_want(26))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(26, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(26, esc)
	lsg_n27(ctx)
}

// Print String (p27)
lsg_n27(ctx[])
{
	new lsi0 = (lsg_w1_starts("  Hello, World,  again  ", "  hel") ? 1 : 0)
	if (lsd_want(27))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(27, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(27, esc)
	lsg_n28(ctx)
}

// Print String (p28)
lsg_n28(ctx[])
{
	new lsi0 = (lsg_w1_ends("  Hello, World,  again  ", "AIN  ") ? 1 : 0)
	if (lsd_want(28))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(28, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(28, esc)
	lsg_n29(ctx)
}

// Print String (p29)
lsg_n29(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_join(lsg_w1_split("  Hello, World,  again  ", ","), " - "))
	if (lsd_want(29))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(29, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(29, esc)
	lsg_n30(ctx)
}

// Print String (p30)
lsg_n30(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_time(3.0, 0))
	if (lsd_want(30))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(30, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(30, esc)
	lsg_n31(ctx)
}

// Print String (p31)
lsg_n31(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_num(lsg_w1_map(5.0, 0.0, 10.0, 100.0, 200.0, 1), 2, 1))
	if (lsd_want(31))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(31, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(31, esc)
	lsg_n32(ctx)
}

// Print String (p32)
lsg_n32(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 0)
	if (lsd_want(32))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(32, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(32, esc)
	lsg_n33(ctx)
}

// Print String (p33)
lsg_n33(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 1)
	if (lsd_want(33))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(33, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(33, esc)
	lsg_n34(ctx)
}

// Print String (p34)
lsg_n34(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 1, true)
	if (lsd_want(34))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(34, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(34, esc)
	lsg_n35(ctx)
}

// Print String (p35)
lsg_n35(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 2)
	if (lsd_want(35))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(35, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(35, esc)
	lsg_n36(ctx)
}

// Print String (p36)
lsg_n36(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 2, true)
	if (lsd_want(36))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(36, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(36, esc)
	lsg_n37(ctx)
}

// Print String (p37)
lsg_n37(ctx[])
{
	new Float:lsi0 = lsg_w1_astat(lsg_mka6(3.0, 1.0), 3)
	if (lsd_want(37))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(37, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(37, esc)
	lsg_n38(ctx)
}

// Print String (p38)
lsg_n38(ctx[])
{
	new lsi0 = lsg_w1_slice(lsg_w1_split("  Hello, World,  again  ", ","), 1.0, 2.0)
	if (lsd_want(38))
	{
	lsd_begin()
	lsd_in_text("Value", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(38, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsi0)))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(38, esc)
	lsg_n39(ctx)
}

// Print String (p39)
lsg_n39(ctx[])
{
	new Float:lsi0 = lsg_w1_dot(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0))
	if (lsd_want(39))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(39, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(39, esc)
	lsg_n40(ctx)
}

// Print String (p40)
lsg_n40(ctx[])
{
	new lsi0 = lsg_w1_cross(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0))
	if (lsd_want(40))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(40, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(40, esc)
	lsg_n41(ctx)
}

// Print String (p41)
lsg_n41(ctx[])
{
	new lsi0 = lsg_w1_rnd(lsg_vec(0.0, 0.0, 64.0), 200.0, 1)
	if (lsd_want(41))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(41, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(41, esc)
	lsg_n42(ctx)
}

// Print String (p42)
lsg_n42(ctx[])
{
	new lsi0 = lsg_w1_rot(lsg_vec(100.0, 50.0, 0.0), 90.0, 10.0)
	if (lsd_want(42))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(42, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(42, esc)
	lsg_n43(ctx)
}

// Print String (p43)
lsg_n43(ctx[])
{
	new lsi0 = lsg_w1_vlerp(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0), 0.25)
	if (lsd_want(43))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(43, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(43, esc)
	lsg_n44(ctx)
}

// Print String (p44)
lsg_n44(ctx[])
{
	new lsi0 = lsg_w1_toang(lsg_vec(100.0, 50.0, 0.0))
	if (lsd_want(44))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(44, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(44, esc)
	lsg_n45(ctx)
}

// Print String (p45)
lsg_n45(ctx[])
{
	new lsi0 = lsg_w1_fwd(lsg_w1_toang(lsg_vec(100.0, 50.0, 0.0)))
	if (lsd_want(45))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(45, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(45, esc)
	lsg_n46(ctx)
}

// Print String (p46)
lsg_n46(ctx[])
{
	new lsi0 = lsg_w1_local(ctx[0], lsg_vec(100.0, 50.0, 0.0))
	if (lsd_want(46))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(46, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(46, esc)
	lsg_n47(ctx)
}

// Print String (p47)
lsg_n47(ctx[])
{
	new lsi0 = lsg_w1_world(ctx[0], lsg_vec(100.0, 50.0, 0.0))
	if (lsd_want(47))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(47, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(47, esc)
	lsg_n48(ctx)
}

// Print String (p48)
lsg_n48(ctx[])
{
	new Float:lsi0 = lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 0)
	if (lsd_want(48))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(48, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(48, esc)
	lsg_n49(ctx)
}

// Print String (p49)
lsg_n49(ctx[])
{
	new Float:lsi0 = lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 1)
	if (lsd_want(49))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(49, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(49, esc)
	lsg_n50(ctx)
}

// Print String (p50)
lsg_n50(ctx[])
{
	new Float:lsi0 = lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 2)
	if (lsd_want(50))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(50, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(50, esc)
	lsg_n51(ctx)
}

// Print String (p51)
lsg_n51(ctx[])
{
	new Float:lsi0 = lsg_w1_angto(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0))
	if (lsd_want(51))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(51, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(51, esc)
	lsg_n52(ctx)
}

// Print String (p52)
lsg_n52(ctx[])
{
	new lsi0 = (lsg_w1_cone(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 90.0, 0) ? 1 : 0)
	if (lsd_want(52))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(52, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(52, esc)
	lsg_n53(ctx)
}

// Print String (p53)
lsg_n53(ctx[])
{
	new lsi0 = (lsg_w1_cone(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 120.0, 1) ? 1 : 0)
	if (lsd_want(53))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(53, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(53, esc)
	lsg_n54(ctx)
}

// Print String (p54)
lsg_n54(ctx[])
{
	new lsi0 = lsg_vplayer(ctx[0], 1)
	if (lsd_want(54))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(54, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(54, esc)
	lsg_n55(ctx)
}

// Print String (p55)
lsg_n55(ctx[])
{
	new lsi0 = lsg_vplayer(ctx[0], 2)
	if (lsd_want(55))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(55, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(55, esc)
	lsg_n56(ctx)
}

// Print String (p56)
lsg_n56(ctx[])
{
	new lsi0 = lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 0)
	if (lsd_want(56))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(56, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(56, esc)
	lsg_n57(ctx)
}

// Print String (p57)
lsg_n57(ctx[])
{
	new lsi0 = lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 1)
	if (lsd_want(57))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(57, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(57, esc)
	lsg_n58(ctx)
}

// Print String (p58)
lsg_n58(ctx[])
{
	new lsi0 = lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 2)
	if (lsd_want(58))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(58, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(58, esc)
	lsg_n59(ctx)
}

// Print String (p59)
lsg_n59(ctx[])
{
	new lsi0 = lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 3)
	if (lsd_want(59))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(59, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(59, esc)
	lsg_n60(ctx)
}

// Print String (p60)
lsg_n60(ctx[])
{
	new Float:lsi0 = lsg_w1_bits(12.0, 2.0, 4)
	if (lsd_want(60))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(60, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(60, esc)
	lsg_n61(ctx)
}

// Print String (p61)
lsg_n61(ctx[])
{
	new Float:lsi0 = lsg_w1_bits(5.0, 3.0, 3)
	if (lsd_want(61))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(61, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(61, esc)
	lsg_n62(ctx)
}

// Print String (p62)
lsg_n62(ctx[])
{
	new lsi0 = lsg_w1_free(119, lsg_vec(0.0, 0.0, 64.0), HULL_HUMAN, 256.0, 64, 0)
	if (lsd_want(62))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(62, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(62, esc)
	lsg_n63(ctx)
}

// Print String (p63)
lsg_n63(ctx[])
{
	new lsi0 = lsg_w1_free(119, lsg_vec(0.0, 0.0, 64.0), HULL_HUMAN, 256.0, 64, 1)
	if (lsd_want(63))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(63, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(63, esc)
	lsg_n64(ctx)
}

// Print String (p64)
lsg_n64(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_lang(ctx[0], 1))
	if (lsd_want(64))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(64, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(64, esc)
	lsg_n65(ctx)
}

// Print String (p65)
lsg_n65(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_lang(ctx[0], 0))
	if (lsd_want(65))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(65, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(65, esc)
	lsg_n66(ctx)
}

// Draw Debug Line (dl)
lsg_n66(ctx[])
{
	new lsi0 = lsg_vplayer(ctx[0], 1)
	new lsi1 = lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 0)
	if (lsd_want(66))
	{
	lsd_begin()
	lsd_in_text("From", lsg_vtext(lsi0))
	lsd_in_text("To", lsg_vtext(lsi1))
	}
	if (lsd_enter(66, ctx))
		return
	if (!lsg_msg_room("Draw Debug Line", 1, 0, 48))
		lsd_error(66, "Skipped: too many effects this server frame (Effect messages a frame, Limits).")
	else
	{
		lsg_w1_line(lsi0, lsi1, 0, 255, 0, 3.0)
		lsd_exit(66, "drawn")
	}
	lsg_n67(ctx)
}

// Draw Debug Box (db)
lsg_n67(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 64.0)
	new lsi1 = lsg_vec(100.0, 50.0, 0.0)
	if (lsd_want(67))
	{
	lsd_begin()
	lsd_in_text("Corner", lsg_vtext(lsi0))
	lsd_in_text("Opposite corner", lsg_vtext(lsi1))
	}
	if (lsd_enter(67, ctx))
		return
	if (!lsg_msg_room("Draw Debug Box", 1, 0, 48))
		lsd_error(67, "Skipped: too many effects this server frame (Effect messages a frame, Limits).")
	else
	{
		lsg_w1_box(lsi0, lsi1, 0.0, 255, 230, 0, 2.0)
		lsd_exit(67, "drawn")
	}
	lsg_n68(ctx)
}

// Draw Debug Point (dp)
lsg_n68(ctx[])
{
	new lsi0 = lsg_w1_free(119, lsg_vec(0.0, 0.0, 64.0), HULL_HUMAN, 256.0, 64, 1)
	if (lsd_want(68))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	}
	if (lsd_enter(68, ctx))
		return
	if (!lsg_msg_room("Draw Debug Point", 1, 0, 48))
		lsd_error(68, "Skipped: too many effects this server frame (Effect messages a frame, Limits).")
	else
	{
		lsg_w1_box(lsi0, -1, 8.0, 255, 40, 40, 2.0)
		lsd_exit(68, "drawn")
	}
	lsg_n69(ctx)
}

// Log Line (log)
lsg_n69(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_lang(ctx[0], 1))
	if (lsd_want(69))
	{
	lsd_begin()
	lsd_in_text("Text", lsi0)
	}
	if (lsd_enter(69, ctx))
		return
	if (lsg_w1_log(69, lsi0, 5))
		lsd_exit(69, "logged")
	else
		lsd_exit(69, "skipped: too many a second")
	lsg_n70(ctx)
}

// Toggle Variable (tg)
lsg_n70(ctx[])
{
	if (lsd_want(70))
	{
	lsd_begin()
	}
	if (lsd_enter(70, ctx))
		return
	g_v0 = !g_v0
	g_lsgO[0] = g_v0 ? 1 : 0
	lsd_exit(70, g_lsdBool[g_v0 ? 1 : 0])
	lsg_n71(ctx)
}

// Print String (p66)
lsg_n71(ctx[])
{
	new lsi0 = g_lsgO[0]
	if (lsd_want(71))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(71, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(71, esc)
	lsg_n72(ctx)
}

// Toggle Variable (tgp)
lsg_n72(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(72))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(72, ctx))
		return
	g_pv1[lsg_slot(lsi0)] = !g_pv1[lsg_slot(lsi0)]
	g_lsgO[1] = g_pv1[lsg_slot(lsi0)] ? 1 : 0
	lsd_exit(72, g_lsdBool[g_pv1[lsg_slot(lsi0)] ? 1 : 0])
	lsg_n73(ctx)
}

// Toggle Variable (tgt)
lsg_n73(ctx[])
{
	new lsi1 = lsg_team(ctx[0])
	if (lsd_want(73))
	{
	lsd_begin()
	lsd_in_team("Team", lsi1)
	}
	if (lsd_enter(73, ctx))
		return
	g_tv2[clamp(lsi1, 0, 3)] = !g_tv2[clamp(lsi1, 0, 3)]
	g_lsgO[2] = g_tv2[clamp(lsi1, 0, 3)] ? 1 : 0
	lsd_exit(73, g_lsdBool[g_tv2[clamp(lsi1, 0, 3)] ? 1 : 0])
	lsg_n74(ctx)
}

// Print String (p67)
lsg_n74(ctx[])
{
	new lsi0 = g_lsgO[2]
	if (lsd_want(74))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(74, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(74, esc)
	lsg_n75(ctx)
}

// Reverse (rev)
lsg_n75(ctx[])
{
	new lsi0 = lsg_mka6(3.0, 1.0)
	if (lsd_want(75))
	{
	lsd_begin()
	lsd_in_text("Array", fmt("%d items", lsg_alen(lsi0)))
	}
	if (lsd_enter(75, ctx))
		return
	lsg_w1_areverse(lsi0)
	lsd_exit(75, "reversed")
	lsg_n76(ctx)
}

// Sequence (seq)
lsg_n76(ctx[])
{
	if (lsd_want(76))
	{
	lsd_begin()
	}
	if (lsd_enter(76, ctx))
		return
	lsd_exit(76, "ran")
	lsg_n77_reset(ctx)
	lsg_n77_reset_player(ctx)
	lsg_n77(ctx)
}

// Do Once Per Player (dopp)
stock lsg_n77(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(77))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(77, ctx))
		return
	new p = lsi0
	if (p < 1 || p > 32)
	{
		lsd_exit(77, "no player")
		return
	}
	if (g_lsgLeft[77] & (1 << (p - 1)))
	{
		lsd_exit(77, "done for this player")
		return
	}
	g_lsgLeft[77] |= 1 << (p - 1)
	lsd_exit(77, "through")
	lsg_n78(ctx)
}

stock lsg_n77_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[77] = 0
}

stock lsg_n77_reset_player(ctx[])
{
	#pragma unused ctx
	new p = ctx[0]
	if (p >= 1 && p <= 32)
		g_lsgLeft[77] &= ~(1 << (p - 1))
}

// Multi Gate (mg)
stock lsg_n78(ctx[])
{
	if (lsd_want(78))
	{
	lsd_begin()
	}
	if (lsd_enter(78, ctx))
		return
	if ((g_lsgLeft[78] & 7) == 7)
	{
		g_lsgLeft[78] = 0
	}
	new k = -1
	for (new j = 0; j < 3; j++)
	{
		new q = (1 + j) % 3
		if (!(g_lsgLeft[78] & (1 << q)))
		{
			k = q
			break
		}
	}
	if (k < 0)
		return
	g_lsgLeft[78] |= 1 << k
	switch (k)
	{
		case 0:
		{
			lsd_exit(78, "Out 0")
			lsg_n79(ctx)
		}
		case 1:
		{
			lsd_exit(78, "Out 1")
			lsg_n80(ctx)
		}
		case 2:
		{
			lsd_exit(78, "Out 2")
			lsg_n81(ctx)
		}
	}
}

stock lsg_n78_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[78] = 0
}

// Print String (mgp0)
lsg_n79(ctx[])
{
	if (lsd_want(79))
	{
	lsd_begin()
	}
	if (lsd_enter(79, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 0")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(79, esc)
	lsg_n82(ctx)
}

// Print String (mgp1)
lsg_n80(ctx[])
{
	if (lsd_want(80))
	{
	lsd_begin()
	}
	if (lsd_enter(80, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 1")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(80, esc)
}

// Print String (mgp2)
lsg_n81(ctx[])
{
	if (lsd_want(81))
	{
	lsd_begin()
	}
	if (lsd_enter(81, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 2")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(81, esc)
}

// Multi Gate (mgr)
stock lsg_n82(ctx[])
{
	if (lsd_want(82))
	{
	lsd_begin()
	}
	if (lsd_enter(82, ctx))
		return
	if ((g_lsgLeft[82] & 3) == 3)
	{
		lsd_exit(82, "all used")
		return
	}
	new k = -1
	new free[8], nf
	for (new j = 0; j < 2; j++)
		if (!(g_lsgLeft[82] & (1 << j)))
			free[nf++] = j
	if (nf)
		k = free[random(nf)]
	if (k < 0)
		return
	g_lsgLeft[82] |= 1 << k
	switch (k)
	{
		case 0:
		{
			lsd_exit(82, "Out 0")
		}
		case 1:
		{
			lsd_exit(82, "Out 1")
			lsg_n83(ctx)
		}
	}
}

stock lsg_n82_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[82] = 0
}

// Print String (mgrp)
lsg_n83(ctx[])
{
	if (lsd_want(83))
	{
	lsd_begin()
	}
	if (lsd_enter(83, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "random gate")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(83, esc)
}

// Round Started (rs)
lsg_n84(ctx[])
{
	if (lsd_want(84))
	{
	lsd_begin()
	}
	if (lsd_enter(84, ctx))
		return
	lsd_exit(84, "ran")
	lsg_n78_reset(ctx)
}

// On Tick (tick)
lsg_n85(ctx[])
{
	if (lsd_want(85))
	{
	lsd_begin()
	}
	if (lsd_enter(85, ctx))
		return
	lsd_exit(85, "tick")
	lsg_n86(ctx)
}

// Print String (tp1)
lsg_n86(ctx[])
{
	new lsi0 = lsg_w1_aim(lsg_w1_np(lsg_vec(0.0, 0.0, 64.0), 0, 8192.0, 1, 0, 0), floatmin(4096.0, float(8192)), 0)
	if (lsd_want(86))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(86, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "tick [0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(86, esc)
	lsg_n87(ctx)
}

// Print String (tp2)
lsg_n87(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_w1_time(Float:ctx[0], 2))
	if (lsd_want(87))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(87, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "delta [0]", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(87, esc)
}

// On Tick (fast)
lsg_n88(ctx[])
{
	if (lsd_want(88))
	{
	lsd_begin()
	}
	if (lsd_enter(88, ctx))
		return
	lsd_exit(88, "tick")
	lsg_n89(ctx)
}

// Print String (fp)
lsg_n89(ctx[])
{
	new lsi0 = lsg_w1_los(0, lsg_vec(0.0, 0.0, 64.0), 0, lsg_vec(100.0, 50.0, 0.0), 0)
	if (lsd_want(89))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(89, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "fast [0]", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(89, esc)
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
	}
}
