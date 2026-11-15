/*
	Every Node EC - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from every_node_ec.graph.json: change the graph and compile it again; edits here are lost.
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

#define LSG_MODE "every_node_ec"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 86
#define LSG_TASK 7310000
#pragma dynamic 59136
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
#define LSG_O 8
#define LSG_OT 1


stock bool:lsg_npc_at(v, Float:o[3])
{
	for (new k = 0; k < 3; k++)
		o[k] = lsg_vc(v, k)
	return true
}
#define LSD_N 86
new const g_lsdIds[LSD_N][] = { "ps", "dmg", "pset", "pcol", "big", "cc", "cc2", "push", "jr", "jra", "radio", "radio2", "motd", "vo", "pr1", "pr2", "pr3", "pr4", "pr5", "prm0", "prm1", "prm2", "prm3", "mst", "noobj", "voice", "prt", "prt0", "prt1", "prt2", "prt3", "npc", "mv", "arr", "fail", "fp", "atp", "fin", "fp2", "stop", "prop", "anim", "att", "att2", "det", "pset2", "gt", "fuse", "gpush", "gprt", "ge", "gprt2", "gpush2", "gprt3", "gprt4", "mf", "mfp", "mfp2", "mfp3", "mfp4", "mf2", "mf2p", "ws", "wsp", "wsp2", "wsp3", "wsp4", "ze", "zep", "zl", "zlp", "rs", "rer", "vote", "v_w1", "v_w3", "v_tie", "v_none", "bota", "botr", "ps2", "hfp", "hfol", "hifp", "hifp2", "hstop" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
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

// The message budget. Two limits, both must have room:
//  - messages a server frame (the Effect messages a frame limit; a unicast counts against its
//    player too, at half the budget)
//  - bytes each client can take: every client has a bucket of unreliable bytes that fills at 40%
//    of its rate (bytes a second, capped by sv_maxrate; the rest is the game's own entities and
//    sounds) and holds at most LSG_MSG_BURST, under one datagram after the engine's own traffic.
//    A message to everyone needs room in every client's bucket (bots don't count). So no setting
//    can overflow a client's datagram: an effects flood stutters nobody (P5's fc_st_fx)
// Two kinds of message use it:
//  - cosmetic (effects, a sound, a chat line): lsg_msg_room; past the budget it's skipped, said in
//    the log once a second
//  - state (a scoreboard's team or tag, a score, an icon, weather: what lasts): lsg_msg_state;
//    never dropped. Past the budget it waits in a queue that goes out on the next frames, in
//    order; the same resend and key queued again keeps its place, and the resend reads the state
//    when it goes, so the latest value wins
stock Float:g_lsgMsgAt, g_lsgMsgN, g_lsgMsgP[33], Float:g_lsgMsgSaid, g_lsgMsgLim = 48
#define LSG_MSG_BURST 1000.0
#define LSG_MSG_BYTES 32
stock Float:g_lsgMsgTok[33], Float:g_lsgMsgTokAt[33], Float:g_lsgMsgFill[33], Float:g_lsgMsgFillAt[33]
// a client's bytes of room now (its bucket filled since it was last used)
stock Float:lsg_msg_tokens(p, Float:now)
{
	if (g_lsgMsgFill[p] <= 0.0 || now - g_lsgMsgFillAt[p] >= 2.0 || now < g_lsgMsgFillAt[p])
	{
		g_lsgMsgFillAt[p] = now
		new r[12], rate
		get_user_info(p, "rate", r, charsmax(r))
		rate = str_to_num(r)
		if (rate <= 0)
			rate = 25000
		static maxr
		maxr = get_cvar_num("sv_maxrate")
		if (maxr > 0 && rate > maxr)
			rate = maxr
		g_lsgMsgFill[p] = float(clamp(rate, 2500, 100000)) * 0.4
	}
	new Float:t = now < g_lsgMsgTokAt[p] ? LSG_MSG_BURST : g_lsgMsgTok[p] + (now - g_lsgMsgTokAt[p]) * g_lsgMsgFill[p]
	g_lsgMsgTokAt[p] = now
	g_lsgMsgTok[p] = t > LSG_MSG_BURST ? LSG_MSG_BURST : t
	return g_lsgMsgTok[p]
}

stock bool:lsg_msg_fits(cost, to, lim, bytes = 0)
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
	new Float:b = floatmin(float(bytes > 0 ? bytes : cost * LSG_MSG_BYTES), LSG_MSG_BURST)
	new one = to >= 1 && to <= 32
	for (new p = one ? to : 1; p <= (one ? to : MaxClients); p++)
		if (is_user_connected(p) && !is_user_bot(p) && lsg_msg_tokens(p, now) < b)
			return false
	for (new p = one ? to : 1; p <= (one ? to : MaxClients); p++)
		if (is_user_connected(p) && !is_user_bot(p))
			g_lsgMsgTok[p] -= b
	g_lsgMsgN += cost
	if (one)
		g_lsgMsgP[to] += cost
	return true
}

stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0, bytes = 0)
{
	if (lsg_msg_fits(cost, to, lim, bytes))
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
stock g_lsgMsgQFn[LSG_MSGQ][32], g_lsgMsgQKey[LSG_MSGQ], g_lsgMsgQCost[LSG_MSGQ], g_lsgMsgQTo[LSG_MSGQ], g_lsgMsgQBytes[LSG_MSGQ], g_lsgMsgQN, bool:g_lsgMsgQAsked, bool:g_lsgMsgQSending, g_lsgMsgQWaited, g_lsgMsgQFrames
stock bool:lsg_msg_state(const what[], const resend[], key, cost = 1, to = 0, lim = 0, bytes = 0)
{
	// (the queue sending it: its room is taken already)
	if (g_lsgMsgQSending)
		return true
	// (others waiting: it waits behind them, so the order holds)
	if (!g_lsgMsgQN && lsg_msg_fits(cost, to, lim, bytes))
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
	g_lsgMsgQBytes[g_lsgMsgQN] = bytes
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
	// (each waiting one that has room now goes, in order; one whose client has no room yet waits
	// without holding up the others: the buckets fill again, so everything goes)
	new keep = 0, fn[32]
	for (new i = 0; i < g_lsgMsgQN; i++)
	{
		if (!lsg_msg_fits(g_lsgMsgQCost[i], g_lsgMsgQTo[i], 0, g_lsgMsgQBytes[i]))
		{
			if (keep != i)
			{
				copy(g_lsgMsgQFn[keep], 31, g_lsgMsgQFn[i])
				g_lsgMsgQKey[keep] = g_lsgMsgQKey[i]
				g_lsgMsgQCost[keep] = g_lsgMsgQCost[i]
				g_lsgMsgQTo[keep] = g_lsgMsgQTo[i]
				g_lsgMsgQBytes[keep] = g_lsgMsgQBytes[i]
			}
			keep++
			continue
		}
		copy(fn, charsmax(fn), g_lsgMsgQFn[i])
		g_lsgMsgQSending = true
		if (callfunc_begin(fn) == 1)
		{
			callfunc_push_int(g_lsgMsgQKey[i])
			callfunc_end()
		}
		g_lsgMsgQSending = false
	}
	g_lsgMsgQN = keep
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

// a cap's "skipped" line in the server log: at most once a second for each cap and what it skipped,
// with how many times since (P5's final check: a mode spawning past a cap every frame wrote 75 lines
// a second for each, enough to take the server down to 4 fps). Every cap uses it (W2).
#define LSG_SKIPN 24
stock g_lsgSkipKey[LSG_SKIPN][48], Float:g_lsgSkipAt[LSG_SKIPN], g_lsgSkipN[LSG_SKIPN], g_lsgSkipUsed
stock lsg_skip_log(const cap[], const what[], const fmt[], any:...)
{
	new key[48], k = -1
	formatex(key, charsmax(key), "%s|%s", cap, what)
	for (new i = 0; i < g_lsgSkipUsed && k < 0; i++)
		if (equal(g_lsgSkipKey[i], key))
			k = i
	if (k < 0)
	{
		k = g_lsgSkipUsed < LSG_SKIPN ? g_lsgSkipUsed++ : LSG_SKIPN - 1
		copy(g_lsgSkipKey[k], charsmax(g_lsgSkipKey[]), key)
		g_lsgSkipAt[k] = -10.0
		g_lsgSkipN[k] = 0
	}
	g_lsgSkipN[k]++
	// (the game's clock starts again on a new map)
	new Float:now = get_gametime()
	if (now >= g_lsgSkipAt[k] && now - g_lsgSkipAt[k] < 1.0)
		return
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 4)
	new Float:since = now - g_lsgSkipAt[k]
	if (g_lsgSkipN[k] > 1 && since <= 2.0)
		log_amx("[%s] %s (skipped %d times in the last second)", LSG_MODE, msg, g_lsgSkipN[k])
	else if (g_lsgSkipN[k] > 1)
		log_amx("[%s] %s (skipped %d times in the last %d seconds)", LSG_MODE, msg, g_lsgSkipN[k], floatround(since))
	else
		log_amx("[%s] %s", LSG_MODE, msg)
	g_lsgSkipAt[k] = now
	g_lsgSkipN[k] = 0
}

// room for one more entity: free slots are kept for the game (it crashes with "no free edicts");
// a mode's spawn that would eat into them is skipped, and the server log says what and why
// The engine doesn't hand a slot out again for 0.5 s after it's freed, so in a flood (things removed
// and made every frame) a slot freed just now is as good as taken: the live entities (the engine's
// count, read once a frame), the ones made since this frame began and the ones freed in the last
// 0.5-1 s must leave the reserve free: an eighth of the server's slots, at least 128 (bots buying,
// dropping and throwing). O(1) a call: a flood asks thousands of times a second (W2, P5's final check)
#define LSG_EDICT_RESERVE 128
stock Float:g_lsgEdAt = -1.0, g_lsgEdLive, g_lsgEdMade, g_lsgEdWin, g_lsgEdFreed, g_lsgEdFreedBefore, bool:g_lsgEdHooked
// (what the mode has out that players see: lsg_vis_note, below; at most its Visible things at once,
// lsg_vis_most(), 128 unless the mode's Limits say)
stock bool:g_lsgVis[2048], g_lsgVisN

public lsg_ed_freed(ent)
{
	lsg_ed_window()
	g_lsgEdFreed++
	if (ent > 0 && ent < sizeof g_lsgVis && g_lsgVis[ent])
	{
		g_lsgVis[ent] = false
		g_lsgVisN--
	}
}

// What players' games are sent: an entity with a model, not EF_NODRAW (ReGameDLL's AddToFullPack).
// Old clients take 256 entities in a packet, players and the map's own included, and a final-check
// player had 247 datagram overflows with hundreds of the mode's props and projectiles in sight. So
// the mode keeps at most its Visible things at once (Limits, 128) of its own out (trigger volumes and
// loop sounds are never sent, so they don't count). Counted as made (lsg_vis_note on what a spawn
// returns) and freed (the free hook): O(1). W2
stock lsg_vis_note(e)
{
	if (e > 0 && e < sizeof g_lsgVis && !g_lsgVis[e] && pev(e, pev_modelindex) && !(pev(e, pev_effects) & EF_NODRAW))
	{
		g_lsgVis[e] = true
		g_lsgVisN++
	}
	return e
}

// (frees counted in half-second windows: this one and the one before)
stock lsg_ed_window()
{
	new w = floatround(get_gametime() * 2.0, floatround_floor)
	// (a new map: its clock starts again, and the last map's things are gone)
	if (w < g_lsgEdWin)
	{
		arrayset(g_lsgVis, false, sizeof g_lsgVis)
		g_lsgVisN = 0
	}
	if (w != g_lsgEdWin)
	{
		g_lsgEdFreedBefore = w == g_lsgEdWin + 1 ? g_lsgEdFreed : 0
		g_lsgEdFreed = 0
		g_lsgEdWin = w
	}
}

// AMX Mod X's set_view makes its camera with models/rpgrocket.mdl: unless that was loaded with
// the map, the server stops (PF_setmodel_I: no precache). Third person is skipped instead, said once.
stock bool:lsg_set_view(id, mode)
{
	if (mode != CAMERA_NONE && !lsg_have("models/rpgrocket.mdl"))
	{
		lsg_skip_log("camera", "Third person", "Third person skipped: models/rpgrocket.mdl wasn't loaded with the map")
		return false
	}
	set_view(id, mode)
	return true
}

stock bool:lsg_ent_room(const what[], bool:visible = true)
{
	if (!g_lsgEdHooked)
	{
		g_lsgEdHooked = true
		register_forward(FM_OnFreeEntPrivateData, "lsg_ed_freed")
	}
	new Float:now = get_gametime()
	if (now != g_lsgEdAt)
	{
		g_lsgEdAt = now
		g_lsgEdLive = engfunc(EngFunc_NumberOfEntities)
		g_lsgEdMade = 0
	}
	lsg_ed_window()
	if (lsg_vis_full(what, visible))
		return false
	new most = global_get(glb_maxEntities)
	if (g_lsgEdLive + g_lsgEdMade + g_lsgEdFreed + g_lsgEdFreedBefore < most - max(LSG_EDICT_RESERVE, most / 8))
	{
		g_lsgEdMade++
		return true
	}
	lsg_skip_log("slots", what, "%s skipped: the server is out of entity slots", what)
	return false
}

// (a spawn that players see, when the mode already has its Visible things at once out: lsg_ent_room)
stock bool:lsg_vis_full(const what[], bool:visible)
{
	if (!visible || g_lsgVisN < lsg_vis_most())
		return false
	lsg_skip_log("visible", what, "%s skipped: the mode has %d things players see out, as many as Visible things at once allows (Limits)", what, g_lsgVisN)
	return true
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
stock lsg_vis_most()
	return 128

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


// enum Entity Kind
stock const g_lsgE0[][] = { "Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None" }
// enum Component Kind
stock const g_lsgE1[][] = { "Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None" }
// enum Spawn Failure
stock const g_lsgE2[][] = { "None", "Blocked", "Out of entity slots", "Limit reached", "Missing file", "Retries used up", "Bad input" }
stock lsg_mka44(p0, p1)
{
	new a = lsg_atmp()
	lsg_apushv(a, p0)
	lsg_apushv(a, p1)
	return a
}

lsd_graph_vars()
{
}

lsd_graph_set(const vid[], who, const val[])
{
	#pragma unused vid, who, val
	return false
}

// ---- effects, sounds and spawning (fx.rs) ----
// the engine's crash points are gen.rs's shared guards: lsg_msg_room (effect messages a frame,
// the mode's "messages" limit) and lsg_ent_room (entity slots with a reserve)
stock g_lsgXfSrc

stock bool:lsg_xf_from(src, bool:found)
{
	g_lsgXfSrc = src
	return found
}

// a prop at a place (Spawn Prop with the shared transform)
stock lsg_prop_o(const Float:o[3], const model[], solid)
{
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

stock bool:lsg_have_snd(const p[])
{
	new full[128]
	formatex(full, charsmax(full), "sound/%s", p)
	return lsg_have(full) || lsg_have(p)
}

stock lsg_wcoord(const Float:o[3])
{
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
}

// where a spawn goes: an entity's spot (plus an offset along its own forward, right and up),
// or a position; remembers the entity for its rotation
stock bool:lsg_xf_at(src, pos, ofs, Float:o[3])
{
	g_lsgXfSrc = 0
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		return true
	}
	if (!lsg_ent_origin(src, o))
		return false
	g_lsgXfSrc = src
	if (ofs >= 0)
	{
		new Float:a[3], Float:f[3], Float:r[3], Float:u[3]
		pev(src, is_user_alive(src) ? pev_v_angle : pev_angles, a)
		engfunc(EngFunc_MakeVectors, a)
		global_get(glb_v_forward, f)
		global_get(glb_v_right, r)
		global_get(glb_v_up, u)
		for (new k = 0; k < 3; k++)
			o[k] += f[k] * lsg_vc(ofs, 0) + r[k] * lsg_vc(ofs, 1) + u[k] * lsg_vc(ofs, 2)
	}
	return true
}

// turn, push and size a new entity: Rotation, else the At entity's facing (if copied) plus the
// rotation offset; Velocity (a still thing falls and slides from there); Scale (models, sprites)
stock lsg_xf_set(e, copy, rot, rot_ofs, vel, Float:scale)
{
	if (e <= 0 || !pev_valid(e))
		return
	new Float:a[3], bool:turn = false
	if (rot >= 0)
	{
		lsg_vto(rot, a)
		turn = true
	}
	else if (copy && g_lsgXfSrc > 0 && pev_valid(g_lsgXfSrc))
	{
		pev(g_lsgXfSrc, pev_angles, a)
		// (a player's model pitch isn't where they look: keep their heading)
		if (is_user_alive(g_lsgXfSrc))
			a[0] = 0.0
		turn = true
	}
	else
		pev(e, pev_angles, a)
	if (rot_ofs >= 0)
	{
		for (new k = 0; k < 3; k++)
			a[k] += lsg_vc(rot_ofs, k)
		turn = true
	}
	if (turn)
	{
		set_pev(e, pev_angles, a)
		if (pev(e, pev_flags) & FL_MONSTER)
			set_pev(e, pev_ideal_yaw, a[1])
	}
	if (vel >= 0)
	{
		new Float:v[3]
		lsg_vto(vel, v)
		if (pev(e, pev_movetype) == MOVETYPE_NONE)
			set_pev(e, pev_movetype, MOVETYPE_TOSS)
		set_pev(e, pev_velocity, v)
	}
	if (scale > 0.0 && scale != 1.0)
		set_pev(e, pev_scale, floatclamp(scale, 0.05, 10.0))
}

// ---- effects
stock g_lsgDlFollow[33], Float:g_lsgDlEnd[33], g_lsgDl[33][5], g_lsgDlN, g_lsgDlLim[33]

public lsg_fx_dl_tick(t)
{
	// (the task's id: its slot plus the base)
	new k = clamp(t - (LSG_TASK - 200000), 0, 31)
	new e = g_lsgDlFollow[k]
	if (e <= 0 || !pev_valid(e) || get_gametime() >= g_lsgDlEnd[k])
	{
		remove_task(k + LSG_TASK - 200000)
		g_lsgDlFollow[k] = 0
		return
	}
	new Float:o[3]
	if (!lsg_ent_origin(e, o) || !lsg_msg_room("Dynamic Light", 1, 0, g_lsgDlLim[k]))
		return
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_DLIGHT)
	lsg_wcoord(o)
	write_byte(g_lsgDl[k][0])
	write_byte(g_lsgDl[k][1])
	write_byte(g_lsgDl[k][2])
	write_byte(g_lsgDl[k][3])
	write_byte(2)
	write_byte(g_lsgDl[k][4])
	message_end()
}

stock lsg_fx_dlight(follow, pos, r, g, b, Float:radius, Float:life, Float:decay, lim)
{
	new rad = clamp(floatround(radius / 10.0), 1, 255), dec = clamp(floatround(decay / 10.0), 0, 255)
	if (follow > 0 && pev_valid(follow))
	{
		// following: a short light each tenth of a second where it is (32 at once at most)
		new k = g_lsgDlN++ % 32
		remove_task(k + LSG_TASK - 200000)
		g_lsgDlFollow[k] = follow
		g_lsgDlEnd[k] = get_gametime() + floatclamp(life, 0.1, 60.0)
		// (as the message wants them: radius, r, g, b, then the decay after the life)
		g_lsgDl[k][0] = rad
		g_lsgDl[k][1] = r
		g_lsgDl[k][2] = g
		g_lsgDl[k][3] = b
		g_lsgDl[k][4] = dec
		g_lsgDlLim[k] = lim
		lsg_fx_dl_tick(k + LSG_TASK - 200000)
		set_task(0.1, "lsg_fx_dl_tick", k + LSG_TASK - 200000, _, _, "b")
		return 1
	}
	if (pos < 0 || !lsg_msg_room("Dynamic Light", 1, 0, lim))
		return 0
	new Float:o[3]
	lsg_vto(pos, o)
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_DLIGHT)
	lsg_wcoord(o)
	write_byte(rad)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(dec)
	message_end()
	return 1
}

// a decal on the nearest surface within reach (traced six ways from the point)
stock lsg_fx_decal(pos, const name[], Float:reach, lim)
{
	if (pos < 0)
		return 0
	new d = engfunc(EngFunc_DecalIndex, name)
	if (d < 0)
		return 0
	new Float:o[3], Float:end[3], Float:hit[3], Float:best = 2.0, Float:at[3], who = 0
	static const Float:DIRS[6][3] = { {0.0, 0.0, -1.0}, {0.0, 0.0, 1.0}, {1.0, 0.0, 0.0}, {-1.0, 0.0, 0.0}, {0.0, 1.0, 0.0}, {0.0, -1.0, 0.0} }
	lsg_vto(pos, o)
	reach = floatclamp(reach, 8.0, 512.0)
	for (new k = 0; k < 6; k++)
	{
		for (new j = 0; j < 3; j++)
			end[j] = o[j] + DIRS[k][j] * reach
		engfunc(EngFunc_TraceLine, o, end, IGNORE_MONSTERS, 0, 0)
		new Float:f
		get_tr2(0, TR_flFraction, f)
		if (f < best)
		{
			best = f
			get_tr2(0, TR_vecEndPos, hit)
			at = hit
			who = max(get_tr2(0, TR_pHit), 0)
		}
	}
	if (best >= 1.0 || !lsg_msg_room("Decal", 1, 0, lim))
		return 0
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	if (who > 0)
	{
		write_byte(d > 255 ? TE_DECALHIGH : TE_DECAL)
		lsg_wcoord(at)
		write_byte(d & 255)
		write_short(who)
	}
	else
	{
		write_byte(d > 255 ? TE_WORLDDECALHIGH : TE_WORLDDECAL)
		lsg_wcoord(at)
		write_byte(d & 255)
	}
	message_end()
	return 1
}

// pieces of a model flying out from a point (or an entity's middle), with the material's sound
stock lsg_fx_gibs(at, pos, const model[], flags, const snd[], count, Float:speed, Float:life, Float:size, lim)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	if (!lsg_have(model) || !lsg_msg_room("Break Into Gibs", 2, 0, lim))
		return 0
	new Float:sz = floatclamp(size, 1.0, 512.0)
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_BREAKMODEL)
	lsg_wcoord(o)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, sz)
	engfunc(EngFunc_WriteCoord, 0.0)
	engfunc(EngFunc_WriteCoord, 0.0)
	engfunc(EngFunc_WriteCoord, floatclamp(speed, 0.0, 2000.0) * 0.5)
	write_byte(clamp(floatround(speed / 10.0), 0, 255))
	write_short(engfunc(EngFunc_ModelIndex, model))
	write_byte(count)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(flags)
	message_end()
	if (snd[0] && lsg_have_snd(snd))
		engfunc(EngFunc_EmitAmbientSound, 0, o, snd, 1.0, ATTN_NORM, 0, PITCH_NORM)
	return 1
}

// a shockwave ring growing from a point (or an entity's spot)
stock lsg_fx_ring(at, pos, Float:radius, r, g, b, Float:life, width, lim)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	new spr = lsg_have("sprites/shockwave.spr") ? engfunc(EngFunc_ModelIndex, "sprites/shockwave.spr") : (lsg_have("sprites/laserbeam.spr") ? engfunc(EngFunc_ModelIndex, "sprites/laserbeam.spr") : 0)
	if (!spr || !lsg_msg_room("Beam Ring", 1, 0, lim))
		return 0
	engfunc(EngFunc_MessageBegin, MSG_PVS, SVC_TEMPENTITY, o, 0)
	write_byte(TE_BEAMCYLINDER)
	lsg_wcoord(o)
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2] + floatclamp(radius, 8.0, 4096.0))
	write_short(spr)
	write_byte(0)
	write_byte(0)
	write_byte(clamp(floatround(life * 10.0), 1, 255))
	write_byte(width)
	write_byte(0)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(200)
	write_byte(0)
	message_end()
	return 1
}

// ---- sounds (map sounds are best mono: a stereo .wav is heard in the middle, not where it is)
// heard as is, not from a place: a player, a team (1 T, 2 CT) or everyone; volume and pitch go
// through the game's speaker command (v = volume %, p = pitch %)
stock lsg_snd_2d(who, arg, const snd[], Float:vol, pitch, lim)
{
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	new name[72], n = 0
	copy(name, charsmax(name), snd)
	new len = strlen(name)
	if (len > 4 && equali(name[len - 4], ".wav"))
		name[len - 4] = 0
	new v = clamp(floatround(vol * 100.0), 0, 100)
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		if ((who == 1 && p != arg) || (who == 2 && get_user_team(p) != arg))
			continue
		if (!lsg_msg_room("Play Sound 2D", 1, p, lim, 96))
			continue
		client_cmd(p, "spk ^"(v%d p%d) %s^"", v, pitch, name)
		n++
	}
	return n > 0 || who == 0
}

// at a place (a position, else an entity's spot, staying there)
stock lsg_snd_at(at, pos, const snd[], Float:vol, Float:attn, pitch)
{
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(at, o))
		return 0
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	engfunc(EngFunc_EmitAmbientSound, 0, o, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	return 1
}

// on an entity's channel: it follows the entity
stock lsg_snd_on(e, chan, const snd[], Float:vol, Float:attn, pitch)
{
	if (e <= 0 || !pev_valid(e) || !snd[0] || !lsg_have_snd(snd))
		return 0
	emit_sound(e, chan, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	return 1
}

// a looping sound: an emitter at a spot, or riding on an entity; the file loops by itself if it
// has loop points, else it's played again every Every seconds
stock g_lsgLoops
public lsg_snd_again(t)
{
	new e = t - (LSG_TASK - 300000)
	if (e <= MaxClients || !pev_valid(e))
	{
		remove_task(t)
		return
	}
	new snd[72]
	pev(e, pev_noise, snd, charsmax(snd))
	emit_sound(e, CHAN_STATIC, snd, entity_get_float(e, EV_FL_fuser1), entity_get_float(e, EV_FL_fuser2), 0, pev(e, pev_iuser1))
}

stock lsg_snd_loop(on, pos, const snd[], Float:vol, Float:attn, pitch, Float:every, most)
{
	if (!snd[0] || !lsg_have_snd(snd))
		return 0
	if (g_lsgLoops >= most)
	{
		lsg_skip_log("loop_sounds", "Loop Sound", "Loop Sound skipped: %d are playing (the mode's Loop sounds at once limit)", most)
		return 0
	}
	new Float:o[3]
	if (pos >= 0)
		lsg_vto(pos, o)
	else if (!lsg_ent_origin(on, o))
		return 0
	if (!lsg_ent_room("Loop Sound", false))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_loop")
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_effects, EF_NODRAW)
	if (pos < 0 && on > 0)
	{
		set_pev(e, pev_movetype, MOVETYPE_FOLLOW)
		set_pev(e, pev_aiment, on)
	}
	else
		set_pev(e, pev_movetype, MOVETYPE_NONE)
	set_pev(e, pev_noise, snd)
	set_pev(e, pev_fuser1, floatclamp(vol, 0.0, 1.0))
	set_pev(e, pev_fuser2, attn)
	set_pev(e, pev_iuser1, pitch)
	emit_sound(e, CHAN_STATIC, snd, floatclamp(vol, 0.0, 1.0), attn, 0, pitch)
	if (every > 0.0)
		set_task(floatclamp(every, 0.5, 600.0), "lsg_snd_again", e + LSG_TASK - 300000, _, _, "b")
	g_lsgLoops++
	return e
}

stock lsg_snd_stop_loop(e)
{
	if (e <= MaxClients || !pev_valid(e))
		return 0
	new cls[24]
	pev(e, pev_classname, cls, charsmax(cls))
	if (!equal(cls, "ls_studio_loop"))
		return 0
	new snd[72]
	pev(e, pev_noise, snd, charsmax(snd))
	emit_sound(e, CHAN_STATIC, snd, 0.0, ATTN_NORM, SND_STOP, PITCH_NORM)
	remove_task(e + LSG_TASK - 300000)
	engfunc(EngFunc_RemoveEntity, e)
	if (g_lsgLoops > 0)
		g_lsgLoops--
	return 1
}

stock lsg_snd_stop_on(e, chan)
{
	if (e <= 0 || !pev_valid(e) || !lsg_have_snd("common/null.wav"))
		return 0
	emit_sound(e, chan, "common/null.wav", 0.0, ATTN_NORM, SND_STOP, PITCH_NORM)
	return 1
}

stock lsg_snd_stop_player(p)
{
	if (!lsg_ok(p))
		return 0
	client_cmd(p, "stopsound")
	return 1
}

// ---- spawning: a class from Spawn Entity's list, its keys, then spawned
stock lsg_spawn_kv(const cls[], const Float:o[3], const k1[] = "", const v1[] = "", ...)
{
	new e = create_entity(cls)
	if (e <= 0)
		return 0
	engfunc(EngFunc_SetOrigin, e, o)
	if (k1[0])
		DispatchKeyValue(e, k1, v1)
	// (the rest, in pairs)
	new n = numargs(), key[64], val[128]
	for (new a = 4; a + 1 < n; a += 2)
	{
		lsg_argstr(a, key, charsmax(key))
		lsg_argstr(a + 1, val, charsmax(val))
		if (key[0])
			DispatchKeyValue(e, key, val)
	}
	DispatchSpawn(e)
	return pev_valid(e) && !(pev(e, pev_flags) & FL_KILLME) ? e : 0
}

stock lsg_argstr(a, s[], len)
{
	new i = 0
	for (; i < len; i++)
	{
		s[i] = getarg(a, i)
		if (!s[i])
			break
	}
	s[min(i, len)] = 0
}

// ---- solid spawns: is the spot free (a hull the size of the thing, players and walls count)?
// If not: the nearest free spot within reach (rings round it, in sight of it), or nothing.
stock g_lsgXrN[LSG_N], g_lsgXrIn = -1, Float:g_lsgXrO[LSG_N][3]

// (Retry: a timer with the flow's ctx, then the spot and the tries so far)
stock lsg_xr_wait(i, const fn[], Float:every, const ctx[], const Float:o[3])
{
	new d[LSG_CTX + 4]
	for (new k = 0; k < LSG_CTX; k++)
		d[k] = ctx[k]
	d[LSG_CTX] = _:o[0]
	d[LSG_CTX + 1] = _:o[1]
	d[LSG_CTX + 2] = _:o[2]
	d[LSG_CTX + 3] = g_lsgXrN[i]
	set_task(every, fn, LSG_TASK + 410000 + i, d, LSG_CTX + 4)
}

stock lsg_xr_back(i, const d[])
{
	g_lsgXrIn = i
	g_lsgXrO[i][0] = Float:d[LSG_CTX]
	g_lsgXrO[i][1] = Float:d[LSG_CTX + 1]
	g_lsgXrO[i][2] = Float:d[LSG_CTX + 2]
	g_lsgXrN[i] = d[LSG_CTX + 3]
}

// (Retry: the spot of the first try, kept)
stock lsg_xr_put(i, ok, const Float:o[3])
{
	if (ok)
	{
		g_lsgXrO[i][0] = o[0]
		g_lsgXrO[i][1] = o[1]
		g_lsgXrO[i][2] = o[2]
	}
	return ok
}

stock lsg_xr_get(i, Float:o[3])
{
	o[0] = g_lsgXrO[i][0]
	o[1] = g_lsgXrO[i][1]
	o[2] = g_lsgXrO[i][2]
	return 1
}

stock bool:lsg_xf_clear(const Float:o[3], size)
{
	new Float:c[3]
	c[0] = o[0]
	c[1] = o[1]
	c[2] = o[2] + (size >= 2 ? 37.0 : 19.0)
	engfunc(EngFunc_TraceHull, c, c, DONT_IGNORE_MONSTERS, size >= 2 ? HULL_HUMAN : HULL_HEAD, 0, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid)
}

// mode: 0 always spawn (no check), 1 a nearby spot or in place anyway, 2 a nearby spot or not at
// all, 3 exactly there or not at all, 4 exactly there (the node retries)
stock bool:lsg_xf_free(Float:o[3], size, mode, Float:maxd, &moved)
{
	moved = 0
	if (size < 0 || mode == 0 || lsg_xf_clear(o, size))
		return true
	if (mode >= 3)
		return false
	new Float:c[3], Float:from[3], Float:to[3], Float:f
	from[0] = o[0]
	from[1] = o[1]
	from[2] = o[2] + 18.0
	for (new Float:r = 16.0; r <= maxd; r += 16.0)
	{
		new steps = clamp(floatround(r / 6.0), 8, 32)
		for (new k = 0; k < steps; k++)
		{
			new Float:ang = 6.2831853 * float(k) / float(steps)
			for (new z = 0; z < 3; z++)
			{
				c[0] = o[0] + floatcos(ang) * r
				c[1] = o[1] + floatsin(ang) * r
				c[2] = o[2] + float(z) * 18.0
				if (!lsg_xf_clear(c, size))
					continue
				to[0] = c[0]
				to[1] = c[1]
				to[2] = c[2] + 18.0
				engfunc(EngFunc_TraceLine, from, to, IGNORE_MONSTERS, 0, 0)
				get_tr2(0, TR_flFraction, f)
				if (f < 1.0)
					continue
				o[0] = c[0]
				o[1] = c[1]
				o[2] = c[2]
				moved = 1
				return true
			}
		}
	}
	// (mode 1: no free spot near, so in place anyway)
	return mode == 1
}

// ---- engine coverage nodes (EC): only stocks here; the hooks are written for the nodes used ----
// comfort limits are mode settings (limits.rs: ec_attach, ec_anim_loops, ec_map_hooks), passed in as
// lim; these are what the arrays hold (a call past the limit is skipped; the debugger says so)
#define LSG_EC_ATTACH 256
#define LSG_EC_LOOPS 256
#define LSG_EC_CLASSES 256
// (big HUD texts, colored chat and radio lines go through the shared per-frame budget, lsg_msg_room)
stock const g_lsgEcGren[][] = { "", "HE grenade", "Flashbang", "Smoke grenade" }
stock g_lsgEcGType[2048], bool:g_lsgEcGBoom[2048], g_lsgEcGOwner[2048]
stock g_lsgEcLastW[33], g_lsgEcLastE[33], g_lsgEcZone[33], bool:g_lsgEcZoneTurn
stock g_lsgEcJr[33], g_lsgEcAir[33], Float:g_lsgEcPow[33], g_lsgEcUsed[33]
stock g_lsgEcJrAll, g_lsgEcAirAll, Float:g_lsgEcPowAll = 268.0
stock g_lsgEcVoice = -1, Float:g_lsgEcVoiceDist, g_lsgEcVoiceOv[33]
stock g_lsgEcAtt[LSG_EC_ATTACH], g_lsgEcAttTo[LSG_EC_ATTACH], g_lsgEcAttPt[LSG_EC_ATTACH], bool:g_lsgEcAttDie[LSG_EC_ATTACH], g_lsgEcAttSolid[LSG_EC_ATTACH], Float:g_lsgEcAttOff[LSG_EC_ATTACH][3]
stock g_lsgEcLoop[LSG_EC_LOOPS], g_lsgEcLoopSeq[LSG_EC_LOOPS], Float:g_lsgEcLoopLen[LSG_EC_LOOPS], Float:g_lsgEcLoopAt[LSG_EC_LOOPS]
stock g_lsgEcUseLast, Float:g_lsgEcUseAt, g_lsgEcUseCaller
stock bool:g_lsgEcNoObj

// Deal Damage: through the game's own TakeDamage (kills, the kill feed and scores count)
stock lsg_ec_damage(t, a, Float:dmg, bits, zone, bool:noarmor)
{
	if (t <= 0 || !pev_valid(t) || dmg <= 0.0 || pev(t, pev_takedamage) == DAMAGE_NO)
		return 0
	new bool:pl = t <= MaxClients
	if (pl && !is_user_alive(t))
		return 0
	if (!pl && pev_valid(t) != 2)
		return 0
	if (!lsg_ok(a) || !is_user_alive(a))
		a = 0
	if (pl)
		set_ent_data(t, "CBaseMonster", "m_LastHitGroup", zone)
	new Float:arm
	if (pl && noarmor)
	{
		pev(t, pev_armorvalue, arm)
		set_pev(t, pev_armorvalue, 0.0)
	}
	ExecuteHamB(Ham_TakeDamage, t, a, a, dmg, bits)
	if (pl && noarmor && is_user_connected(t))
		set_pev(t, pev_armorvalue, arm)
	return 1
}

// Get/Set Entity Property: 0 rendermode 1 renderfx 2 color 3 amount 4 skin 5 body 6 scale
// 7 gravity 8 movetype 9 solid 10 health 11 framerate
stock Float:lsg_ec_get(e, f)
{
	if (e <= 0 || !pev_valid(e))
		return 0.0
	new Float:x
	switch (f)
	{
		case 0: return float(pev(e, pev_rendermode))
		case 1: return float(pev(e, pev_renderfx))
		case 3: pev(e, pev_renderamt, x)
		case 4: return float(pev(e, pev_skin))
		case 5: return float(pev(e, pev_body))
		case 6: pev(e, pev_scale, x)
		case 7: pev(e, pev_gravity, x)
		case 8: return float(pev(e, pev_movetype))
		case 9: return float(pev(e, pev_solid))
		case 10: pev(e, pev_health, x)
		case 11: pev(e, pev_framerate, x)
	}
	return x
}

stock lsg_ec_getcolor(e)
{
	new Float:c[3]
	if (e > 0 && pev_valid(e))
		pev(e, pev_rendercolor, c)
	return lsg_vfrom(c)
}

stock bool:lsg_ec_brush(e)
{
	new m[2]
	pev(e, pev_model, m, charsmax(m))
	return m[0] == '*'
}

stock lsg_ec_set(e, f, Float:v, pick, cv)
{
	if (e <= 0 || !pev_valid(e))
		return 0
	new bool:pl = e <= MaxClients
	if (pl && !is_user_connected(e))
		return 0
	switch (f)
	{
		case 0, 1, 8, 9:
		{
			if (pick < 0)
				return 0
			if (f == 0)
				set_pev(e, pev_rendermode, pick)
			else if (f == 1)
				set_pev(e, pev_renderfx, pick)
			else if (f == 8)
			{
				// players, brushes (doors, trains) and attached things keep their own movement
				if (pl || lsg_ec_brush(e) || pev(e, pev_movetype) == MOVETYPE_FOLLOW)
					return 0
				set_pev(e, pev_movetype, pick)
			}
			else
			{
				if (pl)
					return 0
				new s = pick == SOLID_BBOX && lsg_ec_brush(e) ? SOLID_BSP : pick
				set_pev(e, pev_solid, s)
				new Float:o[3]
				pev(e, pev_origin, o)
				engfunc(EngFunc_SetOrigin, e, o)
			}
		}
		case 2:
		{
			new Float:c[3]
			lsg_vto(cv, c)
			for (new k = 0; k < 3; k++)
				c[k] = floatclamp(c[k], 0.0, 255.0)
			set_pev(e, pev_rendercolor, c)
		}
		case 3: set_pev(e, pev_renderamt, floatclamp(v, 0.0, 255.0))
		case 4: set_pev(e, pev_skin, clamp(floatround(v), 0, 255))
		case 5: set_pev(e, pev_body, clamp(floatround(v), 0, 255))
		case 6:
		{
			if (pl)
				return 0
			set_pev(e, pev_scale, floatclamp(v, 0.05, 10.0))
		}
		case 7: set_pev(e, pev_gravity, floatclamp(v, 0.01, 10.0))
		case 10:
		{
			if (v < 1.0 || (pl && !is_user_alive(e)))
				return 0
			set_pev(e, pev_health, floatmin(v, 100000.0))
		}
		default: set_pev(e, pev_framerate, floatclamp(v, 0.0, 10.0))
	}
	return 1
}

// Colored Chat: a player whose team color is wanted (0: the reader's own)
stock lsg_ec_sender(p)
	return lsg_ok(p) ? p : 0

// color codes (bytes 1-4) out of text that came from players or tables
stock lsg_ec_clean(s[])
{
	for (new k = 0; s[k]; k++)
		if (s[k] < ' ')
			s[k] = ' '
}

// Player Movement State: 0 ground 1 ducking 2 in water 4 ladder; 3 water level, 5 speed, 6 vertical
stock lsg_ec_mv(p, w)
{
	if (!lsg_ok(p) || !is_user_alive(p))
		return 0
	new fl = pev(p, pev_flags)
	switch (w)
	{
		case 0: return fl & FL_ONGROUND ? 1 : 0
		case 1: return fl & FL_DUCKING ? 1 : 0
		case 2: return pev(p, pev_waterlevel) > 0 ? 1 : 0
		case 4: return pev(p, pev_movetype) == MOVETYPE_FLY ? 1 : 0
	}
	return 0
}

stock Float:lsg_ec_mvf(p, w)
{
	if (!lsg_ok(p) || !is_user_alive(p))
		return 0.0
	if (w == 3)
		return float(pev(p, pev_waterlevel))
	new Float:v[3]
	pev(p, pev_velocity, v)
	if (w == 6)
		return v[2]
	return floatsqroot(v[0] * v[0] + v[1] * v[1])
}

// Push: 0 aim 1 facing 2 the direction 3 away from the point 4 up
stock lsg_ec_push(e, along, dv, pv, Float:str, Float:lift, bool:replace)
{
	if (e <= 0 || !pev_valid(e) || (e <= MaxClients && !is_user_alive(e)))
		return 0
	new Float:d[3], Float:a[3]
	switch (along)
	{
		case 0, 1:
		{
			if (e <= MaxClients)
				pev(e, pev_v_angle, a)
			else
				pev(e, pev_angles, a)
			if (along == 1)
				a[0] = 0.0
			angle_vector(a, ANGLEVECTOR_FORWARD, d)
		}
		case 2:
		{
			if (dv < 0)
				return 0
			lsg_vto(dv, d)
		}
		case 3:
		{
			if (pv < 0)
				return 0
			new Float:o[3]
			lsg_vto(pv, a)
			pev(e, pev_origin, o)
			for (new k = 0; k < 3; k++)
				d[k] = o[k] - a[k]
		}
		default: d[2] = 1.0
	}
	new Float:len = floatsqroot(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
	if (len < 0.001)
		return 0
	new Float:v[3]
	if (!replace)
		pev(e, pev_velocity, v)
	for (new k = 0; k < 3; k++)
		v[k] += d[k] / len * str
	v[2] += lift
	set_pev(e, pev_velocity, v)
	return 1
}

// Jump Rules: flags 1 no jump, 2 no duck, 4 auto bunny hop, 8 back to normal on death, 16 at a new round
stock lsg_ec_jump(id, flags, air, Float:pow)
{
	if (id)
	{
		if (lsg_ok(id))
		{
			g_lsgEcJr[id] = flags
			g_lsgEcAir[id] = air
			g_lsgEcPow[id] = pow
		}
		return
	}
	g_lsgEcJrAll = flags
	g_lsgEcAirAll = air
	g_lsgEcPowAll = pow
	for (new p = 1; p <= MaxClients; p++)
	{
		g_lsgEcJr[p] = flags
		g_lsgEcAir[p] = air
		g_lsgEcPow[p] = pow
	}
}

stock lsg_ec_jump_reset(id, bit)
{
	if ((g_lsgEcJr[id] & bit) && id >= 1 && id <= 32)
	{
		g_lsgEcJr[id] = 0
		g_lsgEcAir[id] = 0
	}
}

public lsg_ec_cmd(id, uc, seed)
{
	#pragma unused seed
	if (id < 1 || id > 32 || !(g_lsgEcJr[id] & 3))
		return FMRES_IGNORED
	new b = get_uc(uc, UC_Buttons)
	if (g_lsgEcJr[id] & 1)
		b &= ~IN_JUMP
	if (g_lsgEcJr[id] & 2)
		b &= ~IN_DUCK
	set_uc(uc, UC_Buttons, b)
	return FMRES_IGNORED
}

public lsg_ec_think(id)
{
	if (id < 1 || id > 32 || (!g_lsgEcAir[id] && !(g_lsgEcJr[id] & 4)) || !is_user_alive(id))
		return FMRES_IGNORED
	new b = pev(id, pev_button), ob = pev(id, pev_oldbuttons), fl = pev(id, pev_flags)
	if (fl & FL_ONGROUND)
	{
		g_lsgEcUsed[id] = 0
		if ((g_lsgEcJr[id] & 4) && (b & IN_JUMP))
		{
			set_pev(id, pev_oldbuttons, ob & ~IN_JUMP)
			set_pev(id, pev_fuser2, 0.0)
		}
	}
	else if (g_lsgEcAir[id] && (b & IN_JUMP) && !(ob & IN_JUMP) && g_lsgEcUsed[id] < g_lsgEcAir[id] && pev(id, pev_waterlevel) < 2 && pev(id, pev_movetype) == MOVETYPE_WALK)
	{
		g_lsgEcUsed[id]++
		new Float:v[3]
		pev(id, pev_velocity, v)
		v[2] = g_lsgEcPow[id]
		set_pev(id, pev_velocity, v)
	}
	return FMRES_IGNORED
}

public lsg_ec_dead(id)
	lsg_ec_jump_reset(id, 8)

public lsg_ec_round()
{
	for (new p = 1; p <= MaxClients; p++)
		lsg_ec_jump_reset(p, 16)
}

// grenades: the kind of a thrown one (1 HE, 2 flash, 3 smoke; 0 anything else), its thrower
stock lsg_ec_gtype(e, const model[])
{
	if (e <= MaxClients || e >= 2048 || !pev_valid(e))
		return 0
	new c[10]
	pev(e, pev_classname, c, charsmax(c))
	if (!equal(c, "grenade"))
		return 0
	new t = 0
	if (equal(model, "models/w_hegrenade.mdl"))
		t = 1
	else if (equal(model, "models/w_flashbang.mdl"))
		t = 2
	else if (equal(model, "models/w_smokegrenade.mdl"))
		t = 3
	g_lsgEcGType[e] = t
	g_lsgEcGBoom[e] = false
	new o = pev(e, pev_owner)
	g_lsgEcGOwner[e] = lsg_ok(o) ? o : 0
	return t
}

// the moment it goes off (once): its fuse is over (a smoke grenade also has to lie still)
stock lsg_ec_gdue(e)
{
	if (e <= MaxClients || e >= 2048 || !g_lsgEcGType[e] || g_lsgEcGBoom[e] || !pev_valid(e))
		return 0
	new Float:t
	pev(e, pev_dmgtime, t)
	if (t > get_gametime())
		return 0
	if (g_lsgEcGType[e] == 3 && !(pev(e, pev_flags) & FL_ONGROUND))
		return 0
	g_lsgEcGBoom[e] = true
	return g_lsgEcGType[e]
}

stock lsg_ec_fuse(e, Float:secs)
{
	if (e <= MaxClients || e >= 2048 || !pev_valid(e) || !g_lsgEcGType[e] || g_lsgEcGBoom[e])
		return 0
	set_pev(e, pev_dmgtime, get_gametime() + secs)
	return 1
}

// Play Animation (looping one-time sequences are played again from the tick)
stock lsg_ec_anim(e, const name[], num, Float:rate, bool:restart, bool:loop, Float:len, lim)
{
	if (e <= MaxClients || !pev_valid(e))
		return 0
	new s = num
	if (s < 0)
	{
		new Float:fr, bool:loops
		s = lookup_sequence(e, name, fr, loops)
		if (s < 0)
			return 0
		if (loops)
			loop = false
	}
	set_pev(e, pev_sequence, s)
	if (restart)
	{
		set_pev(e, pev_frame, 0.0)
		set_pev(e, pev_animtime, get_gametime())
	}
	set_pev(e, pev_framerate, rate)
	new k = -1
	for (new i = 0; i < LSG_EC_LOOPS; i++)
		if (g_lsgEcLoop[i] == e)
			k = i
	if (!loop)
	{
		if (k >= 0)
			g_lsgEcLoop[k] = 0
		return 1
	}
	if (k < 0)
	{
		new used = 0
		for (new i = 0; i < LSG_EC_LOOPS; i++)
		{
			if (g_lsgEcLoop[i] && !pev_valid(g_lsgEcLoop[i]))
				g_lsgEcLoop[i] = 0
			if (g_lsgEcLoop[i])
				used++
			else if (k < 0)
				k = i
		}
		if (used >= lim)
			return 0
	}
	if (k < 0)
		return 0
	g_lsgEcLoop[k] = e
	g_lsgEcLoopSeq[k] = s
	g_lsgEcLoopLen[k] = len
	g_lsgEcLoopAt[k] = get_gametime()
	return 1
}

stock lsg_ec_anim_tick()
{
	new Float:now = get_gametime()
	for (new i = 0; i < LSG_EC_LOOPS; i++)
	{
		new e = g_lsgEcLoop[i]
		if (!e)
			continue
		if (!pev_valid(e) || pev(e, pev_sequence) != g_lsgEcLoopSeq[i])
		{
			g_lsgEcLoop[i] = 0
			continue
		}
		if (now - g_lsgEcLoopAt[i] >= g_lsgEcLoopLen[i])
		{
			g_lsgEcLoopAt[i] = now
			set_pev(e, pev_frame, 0.0)
			set_pev(e, pev_animtime, now)
		}
	}
}

// Attach To: moved with its parent every server frame (its own velocity too, so it's smooth)
stock lsg_ec_attach(e, to, ov, pt, bool:ondeath, lim)
{
	if (e <= MaxClients || !pev_valid(e) || to <= 0 || !pev_valid(to) || e == to)
		return 0
	new k = -1
	for (new i = 0; i < LSG_EC_ATTACH; i++)
		if (g_lsgEcAtt[i] == e)
			k = i
	// never round in a circle
	for (new i = 0; i < LSG_EC_ATTACH; i++)
		if (g_lsgEcAtt[i] == to && g_lsgEcAttTo[i] == e)
			return 0
	if (k < 0)
	{
		new used = 0
		for (new i = 0; i < LSG_EC_ATTACH; i++)
		{
			if (g_lsgEcAtt[i])
				used++
			else if (k < 0)
				k = i
		}
		if (used >= lim)
			return 0
	}
	if (k < 0)
		return 0
	if (g_lsgEcAtt[k] != e)
		g_lsgEcAttSolid[k] = pev(e, pev_solid)
	g_lsgEcAtt[k] = e
	g_lsgEcAttTo[k] = to
	g_lsgEcAttPt[k] = pt
	g_lsgEcAttDie[k] = ondeath
	new Float:off[3]
	if (ov >= 0)
		lsg_vto(ov, off)
	g_lsgEcAttOff[k] = off
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NOCLIP)
	lsg_ec_attach_put(k)
	return 1
}

stock lsg_ec_attach_put(k)
{
	new e = g_lsgEcAtt[k], to = g_lsgEcAttTo[k]
	new Float:o[3], Float:a[3], Float:v[3]
	if (g_lsgEcAttPt[k] >= 0)
		engfunc(EngFunc_GetAttachment, to, g_lsgEcAttPt[k], o, a)
	else
		pev(to, pev_origin, o)
	pev(to, to <= MaxClients ? pev_v_angle : pev_angles, a)
	a[0] = 0.0
	for (new i = 0; i < 3; i++)
		o[i] += g_lsgEcAttOff[k][i]
	pev(to, pev_velocity, v)
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_angles, a)
	set_pev(e, pev_velocity, v)
}

stock lsg_ec_detach(e, bool:fall)
{
	for (new k = 0; k < LSG_EC_ATTACH; k++)
	{
		if (g_lsgEcAtt[k] != e || !e)
			continue
		g_lsgEcAtt[k] = 0
		if (!pev_valid(e))
			continue
		set_pev(e, pev_velocity, Float:{0.0, 0.0, 0.0})
		set_pev(e, pev_movetype, fall ? MOVETYPE_TOSS : MOVETYPE_NONE)
		set_pev(e, pev_solid, g_lsgEcAttSolid[k])
		new Float:o[3]
		pev(e, pev_origin, o)
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

public lsg_ec_frame()
{
	for (new k = 0; k < LSG_EC_ATTACH; k++)
	{
		new e = g_lsgEcAtt[k]
		if (!e)
			continue
		new to = g_lsgEcAttTo[k]
		if (!pev_valid(e))
		{
			g_lsgEcAtt[k] = 0
			continue
		}
		if (!pev_valid(to) || (to <= MaxClients && (!is_user_connected(to) || (g_lsgEcAttDie[k] && !is_user_alive(to)))) || (to > MaxClients && g_lsgEcAttDie[k] && pev(to, pev_deadflag) != DEAD_NO))
		{
			lsg_ec_detach(e, false)
			continue
		}
		lsg_ec_attach_put(k)
	}
	return FMRES_IGNORED
}

// On Map Entity Fired: Use hooked on one entity of each kind (by its code, so twice never)
// that has a name when the map starts
stock lsg_ec_hook_use(lim)
{
	new seen[LSG_EC_CLASSES], n = 0, tn[2]
	new top = global_get(glb_maxEntities)
	lim = clamp(lim, 1, LSG_EC_CLASSES)
	for (new e = MaxClients + 1; e < top && n < lim; e++)
	{
		if (pev_valid(e) != 2)
			continue
		pev(e, pev_targetname, tn, charsmax(tn))
		if (!tn[0])
			continue
		new vt = get_pdata_int(e, 0, 0, 0), bool:dup = false
		for (new i = 0; i < n && !dup; i++)
			dup = seen[i] == vt
		if (dup)
			continue
		seen[n++] = vt
		RegisterHamFromEntity(Ham_Use, e, "lsg_ec_used", 1)
	}
	if (n >= lim)
		log_amx("Studio: On Map Entity Fired hooks %d kinds of entity (Limits: Map entity kinds heard); others on this map aren't heard", lim)
}

// the same entity fired twice in one go (a class hooked through a parent) counts once
stock bool:lsg_ec_twice(ent, caller)
{
	new Float:now = get_gametime()
	if (ent == g_lsgEcUseLast && caller == g_lsgEcUseCaller && now == g_lsgEcUseAt)
		return true
	g_lsgEcUseLast = ent
	g_lsgEcUseCaller = caller
	g_lsgEcUseAt = now
	return false
}

// Remove Map Objectives: 1 buy, 2 bomb, 4 hostages, 8 rescue, 16 VIP, 32 escape
stock lsg_ec_noobj(mask)
{
	new const cls[][] = { "func_buyzone", "func_bomb_target", "info_bomb_target", "hostage_entity", "monster_scientist", "func_hostage_rescue", "info_hostage_rescue", "func_vip_safetyzone", "func_escapezone" }
	new const bit[] = { 1, 2, 2, 4, 4, 8, 8, 16, 32 }
	for (new i = 0; i < sizeof cls; i++)
	{
		if (!(mask & bit[i]))
			continue
		new e = -1
		while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", cls[i])) > 0)
		{
			new tn[16]
			pev(e, pev_targetname, tn, charsmax(tn))
			if (!equal(tn, "lsg_ec_standin"))
				engfunc(EngFunc_RemoveEntity, e)
		}
	}
	// a stand-in nobody reaches: the game still sees a buy zone (else it lets them buy at spawn)
	if ((mask & 1) && !g_lsgEcNoObj)
	{
		new d = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "func_buyzone"))
		if (d > 0)
		{
			set_pev(d, pev_targetname, "lsg_ec_standin")
			g_lsgEcNoObj = true
		}
	}
}

// Map Type: 1 bomb 2 hostage 3 VIP 4 escape
stock bool:lsg_ec_mhas(w)
{
	switch (w)
	{
		case 1: return lsg_ec_any("func_bomb_target") || lsg_ec_any("info_bomb_target")
		case 2: return lsg_ec_any("hostage_entity") || lsg_ec_any("monster_scientist")
		case 3: return lsg_ec_any("func_vip_safetyzone")
	}
	return lsg_ec_any("func_escapezone")
}

stock bool:lsg_ec_any(const c[])
	return engfunc(EngFunc_FindEntityByString, -1, "classname", c) > 0

stock lsg_ec_mtype()
{
	new s[12]
	if (lsg_ec_mhas(1))
		copy(s, charsmax(s), "bomb")
	else if (lsg_ec_mhas(2))
		copy(s, charsmax(s), "hostage")
	else if (lsg_ec_mhas(3))
		copy(s, charsmax(s), "vip")
	else if (lsg_ec_mhas(4))
		copy(s, charsmax(s), "escape")
	else
		copy(s, charsmax(s), "other")
	return s
}

// Weapon Switched: a weapon's short name (ak47); "" for none
stock lsg_ec_wname(csw)
{
	new s[24]
	if (csw > 0 && csw <= 32)
	{
		get_weaponname(csw, s, charsmax(s))
		if (equal(s, "weapon_", 7))
			copy(s, charsmax(s), s[7])
	}
	return s
}

// Voice Rules: 0 all 1 team 2 dead with dead 3 team and dead with dead 4 distance only; -1 unset
stock bool:lsg_ec_hears(r, s)
{
	if (g_lsgEcVoiceOv[s] == 1 || g_lsgEcVoiceOv[r] == 3)
		return true
	if (g_lsgEcVoiceOv[s] == 2)
		return false
	if (g_lsgEcVoice < 0)
		return true
	new tr = get_user_team(r), ts = get_user_team(s)
	new bool:ar = is_user_alive(r) != 0, bool:as = is_user_alive(s) != 0
	if (tr != 1 && tr != 2)
		tr = 3
	if (ts != 1 && ts != 2)
		ts = 3
	switch (g_lsgEcVoice)
	{
		case 1: if (tr != ts) return false
		case 2: if (ar != as) return false
		case 3:
		{
			if (ar != as)
				return false
			if (ar && tr != ts)
				return false
		}
	}
	if (g_lsgEcVoiceDist > 0.0 && ar && as)
	{
		new Float:a[3], Float:b[3]
		pev(r, pev_origin, a)
		pev(s, pev_origin, b)
		if (get_distance_f(a, b) > g_lsgEcVoiceDist)
			return false
	}
	return true
}

public lsg_ec_voice(r, s, bool:listen)
{
	#pragma unused listen
	if (r == s || !lsg_ok(r) || !lsg_ok(s) || (g_lsgEcVoice < 0 && !g_lsgEcVoiceOv[r] && !g_lsgEcVoiceOv[s]))
		return FMRES_IGNORED
	// Mute Voice still mutes
	new bool:ok = get_speak(s) != SPEAK_MUTED && (get_speak(r) == SPEAK_LISTENALL || lsg_ec_hears(r, s))
	engfunc(EngFunc_SetClientListening, r, s, ok)
	forward_return(FMV_CELL, ok)
	return FMRES_SUPERCEDE
}

// Play Radio: the game's own lines (SendAudio) and the radio text (TextMsg)
stock lsg_ec_radio(who, team, from, const snd[], const txt[], bool:ann)
{
	if (!lsg_ok(from))
		from = 0
	new fname[32], num[4]
	if (from)
		get_user_name(from, fname, charsmax(fname))
	else
		copy(fname, charsmax(fname), "Radio")
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || (who && p != who) || (team && get_user_team(p) != team))
			continue
		message_begin(MSG_ONE, get_user_msgid("SendAudio"), _, p)
		write_byte(from ? from : p)
		write_string(snd)
		write_short(100)
		message_end()
		if (!txt[0])
			continue
		message_begin(MSG_ONE, get_user_msgid("TextMsg"), _, p)
		if (ann)
		{
			write_byte(4)
			write_string(txt)
		}
		else
		{
			num_to_str(from ? from : p, num, charsmax(num))
			write_byte(5)
			write_string(num)
			write_string("#Game_radio")
			write_string(fname)
			write_string(txt)
		}
		message_end()
	}
}

// In Map Zone: the game's own zone signals
stock lsg_ec_inzone(p, bit)
	return lsg_ok(p) && is_user_alive(p) && (cs_get_user_mapzones(p) & bit) ? 1 : 0

// Show Info Window: always a page made here, its text escaped ([br] a new line, [b] bold)
stock lsg_ec_html(out[], len, const s[], &bool:bold)
{
	new n = 0
	for (new k = 0; s[k] && n < len - 8; k++)
	{
		new c = s[k]
		if (c == '[' && equal(s[k], "[br]", 4))
		{
			n += copy(out[n], len - n, "<br>")
			k += 3
		}
		else if (c == '[' && equal(s[k], "[b]", 3))
		{
			n += copy(out[n], len - n, bold ? "</b>" : "<b>")
			bold = !bold
			k += 2
		}
		else if (c == '<')
			n += copy(out[n], len - n, "&lt;")
		else if (c == '>')
			n += copy(out[n], len - n, "&gt;")
		else if (c == '&')
			n += copy(out[n], len - n, "&amp;")
		else if (c == '"')
			n += copy(out[n], len - n, "&quot;")
		else if (c == 39)
			n += copy(out[n], len - n, "&#39;")
		else if (c == '^n')
			n += copy(out[n], len - n, "<br>")
		else if (c >= ' ')
			out[n++] = c
	}
	out[n] = 0
	return n
}

stock lsg_ec_motd(id, const title[], const body[], const more[])
{
	if (!lsg_ok(id) || is_user_bot(id))
		return 0
	static buf[1536]
	new bool:bold = false
	new len = charsmax(buf) - 24
	new n = copy(buf, len, "<html><head><style>body{background:#111;color:#e8e2c8;font:13px Tahoma,Verdana,sans-serif;padding:8px}b{color:#ffb000}</style></head><body>")
	n += lsg_ec_html(buf[n], len - n, body, bold)
	if (more[0] && n < len - 8)
	{
		n += copy(buf[n], len - n, "<br>")
		n += lsg_ec_html(buf[n], len - n, more, bold)
	}
	if (bold)
		n += copy(buf[n], charsmax(buf) - n, "</b>")
	copy(buf[n], charsmax(buf) - n, "</body></html>")
	show_motd(id, buf, title)
	return 1
}

// Hostage Follow / Stop / Is Following (the game's own follow, classic and Condition Zero hostages)
stock bool:lsg_ec_hostage(h)
{
	if (h <= MaxClients || h >= 2048 || !pev_valid(h))
		return false
	new c[16]
	pev(h, pev_classname, c, charsmax(c))
	return equal(c, "hostage_entity") && pev(h, pev_deadflag) == DEAD_NO && !(pev(h, pev_effects) & EF_NODRAW)
}

stock lsg_ec_hleader(h)
{
	if (!lsg_ec_hostage(h))
		return 0
	new f = cs_get_hostage_foll(h)
	return lsg_ok(f) ? f : 0
}

stock lsg_ec_hfollow(h, p)
{
	if (!lsg_ec_hostage(h) || (p && (!lsg_ok(p) || !is_user_alive(p))))
		return 0
	cs_set_hostage_foll(h, p)
	return 1
}

// Add / Remove Bots: the bot brain's own commands (fixed words; never text from the graph);
// one slot kept for a person; the host's count comes back with the next map's settings
stock g_lsgEcBotSkill = -1

stock lsg_ec_bots_add(n, team, skill)
{
	new free = MaxClients - get_playersnum(1) - 1
	n = min(n, free)
	if (n <= 0)
		return 0
	if (skill >= 0)
	{
		if (g_lsgEcBotSkill < 0)
			g_lsgEcBotSkill = get_cvar_num("bot_difficulty")
		set_cvar_num("bot_difficulty", skill)
		remove_task(LSG_TASK - 612)
		set_task(2.0, "lsg_ec_bots_skill_back", LSG_TASK - 612)
	}
	for (new i = 0; i < n; i++)
		server_cmd(team == 1 ? "bot_add_t" : (team == 2 ? "bot_add_ct" : "bot_add"))
	return n
}

public lsg_ec_bots_skill_back()
{
	if (g_lsgEcBotSkill >= 0)
		set_cvar_num("bot_difficulty", g_lsgEcBotSkill)
	g_lsgEcBotSkill = -1
}

stock lsg_ec_bots_remove(n, team)
{
	new gone = 0
	for (new p = MaxClients; p >= 1 && gone < n; p--)
	{
		if (!is_user_connected(p) || !is_user_bot(p) || (team && get_user_team(p) != team))
			continue
		server_cmd("kick #%d", get_user_userid(p))
		gone++
	}
	if (gone)
		set_cvar_num("bot_quota", max(0, get_cvar_num("bot_quota") - gone))
	return gone
}

// leaving: nothing carries over to whoever gets the slot next
public lsg_ec_left(id)
{
	if (id < 1 || id > 32)
		return FMRES_IGNORED
	g_lsgEcLastW[id] = 0
	g_lsgEcLastE[id] = 0
	g_lsgEcZone[id] = 0
	g_lsgEcJr[id] = g_lsgEcJrAll
	g_lsgEcAir[id] = g_lsgEcAirAll
	g_lsgEcPow[id] = g_lsgEcPowAll
	g_lsgEcUsed[id] = 0
	g_lsgEcVoiceOv[id] = 0
	return FMRES_IGNORED
}

// ---- NPC moves (EC): gm_components walks them (ls_npc_move); Arrived / Failed come back here
// with the flow's own values. A comfort limit (limits.rs ec_npc_moves, passed in as lim): moves waiting
// for an answer at once; the array holds this many
#define LSG_EC_NPCS 64
stock g_lsgEcNpcEnt[LSG_EC_NPCS], g_lsgEcNpcNode[LSG_EC_NPCS], g_lsgEcNpcCtx[LSG_EC_NPCS][LSG_CTX]

stock lsg_ec_npc_find(npc)
{
	if (npc <= 0)
		return -1
	for (new s = 0; s < LSG_EC_NPCS; s++)
		if (g_lsgEcNpcEnt[s] == npc)
			return s
	return -1
}

stock lsg_ec_npc_keep(npc, node, const ctx[], lim)
{
	new s = lsg_ec_npc_find(npc), used = 0
	for (new k = 0; k < LSG_EC_NPCS; k++)
	{
		if (g_lsgEcNpcEnt[k] && !pev_valid(g_lsgEcNpcEnt[k]))
			g_lsgEcNpcEnt[k] = 0
		if (g_lsgEcNpcEnt[k])
			used++
		else if (s < 0)
			s = k
	}
	if (s < 0 || (used >= lim && lsg_ec_npc_find(npc) < 0))
	{
		log_amx("Studio: more than %d NPC moves at once (Limits: NPC moves at once); this one's Arrived won't run", lim)
		return
	}
	g_lsgEcNpcEnt[s] = npc
	g_lsgEcNpcNode[s] = node
	for (new k = 0; k < LSG_CTX; k++)
		g_lsgEcNpcCtx[s][k] = ctx[k]
}

stock bool:lsg_ec_npc_to(npc, to, pos, Float:speed, flags)
{
	if (!ls_components_running() || npc <= MaxClients || !pev_valid(npc))
		return false
	new Float:p[3]
	if (to > 0 && pev_valid(to))
		pev(to, pev_origin, p)
	else if (pos >= 0)
		lsg_vto(pos, p)
	else
		return false
	// a player's or a prop's place: its feet
	if (to > 0 && to <= MaxClients)
		p[2] -= 36.0
	new s = lsg_ec_npc_find(npc)
	if (s >= 0)
		g_lsgEcNpcEnt[s] = 0
	return ls_npc_move(npc, p, 1, speed, flags)
}

stock bool:lsg_ec_npc_path(npc, arr, const track[], Float:speed, flags)
{
	if (!ls_components_running() || npc <= MaxClients || !pev_valid(npc))
		return false
	new s = lsg_ec_npc_find(npc)
	if (s >= 0)
		g_lsgEcNpcEnt[s] = 0
	if (arr < 0)
		return ls_npc_move_path(npc, track, speed, flags)
	new n = min(lsg_alen(arr), 24)
	if (n <= 0)
		return false
	new Float:pts[72], Float:o[3]
	for (new i = 0; i < n; i++)
	{
		lsg_vto(lsg_agetv(arr, float(i)), o)
		pts[i * 3] = o[0]
		pts[i * 3 + 1] = o[1]
		pts[i * 3 + 2] = o[2]
	}
	return ls_npc_move(npc, pts, n, speed, flags)
}

stock lsg_ec_npc_stop(npc, bool:stay)
{
	new s = lsg_ec_npc_find(npc)
	if (s >= 0)
		g_lsgEcNpcEnt[s] = 0
	if (ls_components_running() && npc > MaxClients)
		ls_npc_move_stop(npc, stay)
}

// ---- Start Vote (EC): one vote at a time, never over the server's map or mode vote ----
stock g_lsgEcVoteN[9], g_lsgEcVoteNode = -1, g_lsgEcVoteCtx[LSG_CTX], g_lsgEcVoteMenu = -1, g_lsgEcVoteOpts, bool:g_lsgEcVoted[33]

// gm_pubserver's map/mode vote running now (0 when it isn't loaded)
stock bool:lsg_ec_mapvote()
{
	if (callfunc_begin("gm_pubserver_voting", "gm_pubserver.amxx") != 1)
		return false
	return callfunc_end() != 0
}

stock bool:lsg_ec_vote_begin(k, const ctx[])
{
	if (g_lsgEcVoteMenu >= 0 || lsg_ec_mapvote())
		return false
	g_lsgEcVoteNode = k
	for (new i = 0; i < LSG_CTX; i++)
		g_lsgEcVoteCtx[i] = ctx[i]
	arrayset(g_lsgEcVoteN, 0, sizeof g_lsgEcVoteN)
	arrayset(g_lsgEcVoted, false, sizeof g_lsgEcVoted)
	return true
}

// who: 0 everyone, 1 Terrorists, 2 Counter-Terrorists, 3 the living
stock lsg_ec_vote_show(m, n, who, bool:bots, Float:secs)
{
	g_lsgEcVoteMenu = m
	g_lsgEcVoteOpts = n
	menu_setprop(m, MPROP_EXIT, MEXIT_NEVER)
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || (who == 1 && get_user_team(p) != 1) || (who == 2 && get_user_team(p) != 2) || (who == 3 && !is_user_alive(p)))
			continue
		if (is_user_bot(p))
		{
			if (bots)
			{
				g_lsgEcVoted[p] = true
				g_lsgEcVoteN[random_num(1, n)]++
				g_lsgEcVoteN[0]++
			}
			continue
		}
		// a menu already open (the server's own, a shop) isn't taken over
		new old, nm
		player_menu_info(p, old, nm)
		if (old > 0 || nm >= 0)
			continue
		menu_display(p, m, 0, floatround(secs))
	}
	remove_task(LSG_TASK - 611)
	set_task(secs, "lsg_ec_vote_end", LSG_TASK - 611)
}

public lsg_ec_vote_pick(id, menu, item)
{
	if (menu != g_lsgEcVoteMenu || item < 0 || item >= g_lsgEcVoteOpts || id < 1 || id > 32 || g_lsgEcVoted[id])
		return PLUGIN_HANDLED
	g_lsgEcVoted[id] = true
	g_lsgEcVoteN[item + 1]++
	g_lsgEcVoteN[0]++
	return PLUGIN_HANDLED
}

// the result: the winner (1-8), -1 a tie, 0 no votes
stock lsg_ec_vote_close()
{
	new m = g_lsgEcVoteMenu
	g_lsgEcVoteMenu = -1
	g_lsgEcVoteNode = -1
	if (m >= 0)
		menu_destroy(m)
	new best = 0, w = 0, ties = 0
	for (new i = 1; i <= g_lsgEcVoteOpts; i++)
	{
		if (g_lsgEcVoteN[i] > best)
		{
			best = g_lsgEcVoteN[i]
			w = i
			ties = 0
		}
		else if (best > 0 && g_lsgEcVoteN[i] == best)
			ties++
	}
	if (!best)
		return 0
	return ties ? -1 : w
}

public plugin_precache()
{
	// an NPC the mode makes: its model is loaded with the map
	ls_npc_precache("models/barney.mdl")
	lsg_pre_model("models/w_c4.mdl")
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node EC", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	set_task(1.0, "lsg_on_map_started")
	register_forward(FM_CmdStart, "lsg_ec_cmd", 0)
	register_forward(FM_PlayerPreThink, "lsg_ec_think", 0)
	RegisterHamPlayer(Ham_Killed, "lsg_ec_dead", 1)
	register_event("HLTV", "lsg_ec_round", "a", "1=0", "2=0")
	register_forward(FM_SetModel, "lsg_ec_setmodel", 1)
	RegisterHam(Ham_Think, "grenade", "lsg_ec_gthink", 0)
	lsg_ec_hook_use(96)
	RegisterHam(Ham_Item_Deploy, "weapon_knife", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_glock18", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_usp", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_p228", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_deagle", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_elite", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_fiveseven", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m3", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_xm1014", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_mac10", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_tmp", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_mp5navy", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_ump45", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_p90", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_galil", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_famas", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_ak47", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m4a1", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_sg552", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_aug", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_scout", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_awp", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_g3sg1", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_sg550", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m249", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_hegrenade", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_flashbang", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_smokegrenade", "lsg_ec_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_c4", "lsg_ec_deploy", 1)
	register_forward(FM_Voice_SetClientListening, "lsg_ec_voice", 0)
	register_forward(FM_StartFrame, "lsg_ec_frame", 0)
	set_task(0.1, "lsg_ec_tick", LSG_TASK - 610, _, _, "b")
	register_forward(FM_ClientDisconnect, "lsg_ec_left", 0)
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

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n71(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n0(ctx)
	lsg_n80(ctx)
	return HAM_IGNORED
}

public lsg_on_map_started()
{
	new ctx[LSG_CTX]
	lsg_n23(ctx)
}

public lsg_ec_setmodel(e, const model[])
{
	new t = lsg_ec_gtype(e, model)
	if (t <= 0)
		return FMRES_IGNORED
	new ctx[LSG_CTX], Float:o[3]
	pev(e, pev_origin, o)
	ctx[0] = g_lsgEcGOwner[e] // player
	ctx[1] = e // grenade
	ctx[2] = lsg_vfrom(o) // pos
	copy(g_lsgTxt[0], 63, g_lsgEcGren[t]) // kind
	if (t == 1)
		lsg_n46(ctx)
	return FMRES_IGNORED
}

public lsg_ec_gthink(e)
{
	new t = lsg_ec_gdue(e)
	if (!t)
		return HAM_IGNORED
	new ctx[LSG_CTX], Float:o[3]
	pev(e, pev_origin, o)
	ctx[0] = g_lsgEcGOwner[e] // player
	ctx[1] = e // grenade
	ctx[2] = lsg_vfrom(o) // pos
	copy(g_lsgTxt[0], 63, g_lsgEcGren[t]) // kind
	lsg_n50(ctx)
	return HAM_IGNORED
}

public lsg_ec_used(ent, caller, activator, kind, Float:value)
{
	#pragma unused kind
	#pragma unused value
	if (ent <= MaxClients || !pev_valid(ent) || lsg_ec_twice(ent, caller))
		return HAM_IGNORED
	new tn[33]
	pev(ent, pev_targetname, tn, charsmax(tn))
	if (!tn[0])
		return HAM_IGNORED
	new ctx[LSG_CTX]
	copy(g_lsgTxt[0], 63, tn) // name
	ctx[0] = ent // entity
	ctx[1] = (activator >= 1 && activator <= MaxClients ? activator : 0) // activator
	ctx[2] = (caller > 0 ? caller : 0) // caller
	lsg_n55(ctx)
	if (equal(tn, "door1"))
		lsg_n60(ctx)
	return HAM_IGNORED
}

public lsg_ec_deploy(w)
{
	if (!pev_valid(w))
		return HAM_IGNORED
	new id = get_ent_data_entity(w, "CBasePlayerItem", "m_pPlayer")
	if (!lsg_ok(id))
		return HAM_IGNORED
	new csw = cs_get_weapon_id(w), was = g_lsgEcLastW[id]
	if (csw == was && w == g_lsgEcLastE[id])
		return HAM_IGNORED
	g_lsgEcLastW[id] = csw
	g_lsgEcLastE[id] = w
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = w // weapon
	copy(g_lsgTxt[0], 63, lsg_ec_wname(csw)) // new
	copy(g_lsgTxt[1], 63, lsg_ec_wname(was)) // old
	lsg_n62(ctx)
	return HAM_IGNORED
}

public lsg_ec_tick()
{
	lsg_ec_anim_tick()
	g_lsgEcZoneTurn = !g_lsgEcZoneTurn
	if (g_lsgEcZoneTurn)
		return
	for (new p = 1; p <= MaxClients; p++)
	{
		new z = is_user_alive(p) ? cs_get_user_mapzones(p) : 0, was = g_lsgEcZone[p]
		g_lsgEcZone[p] = z
		if (z == was)
			continue
		new ctx[LSG_CTX]
		if ((z & ~was) & 1)
		{
			ctx[0] = p // player
			lsg_n67(ctx)
		}
		if ((was & ~z) & 2)
		{
			ctx[0] = p // player
			lsg_n69(ctx)
		}
	}
}

public lsg_ec_vote_end()
{
	new k = g_lsgEcVoteNode, w = lsg_ec_vote_close()
	new ctx[LSG_CTX]
	for (new i = 0; i < LSG_CTX; i++)
		ctx[i] = g_lsgEcVoteCtx[i]
	switch (k)
	{
		case 0:
		{
			switch (w)
			{
				case 1: lsg_n74(ctx)
				case 3: lsg_n75(ctx)
				case -1: lsg_n76(ctx)
				case 0: lsg_n77(ctx)
			}
		}
	}
	#pragma unused ctx
	#pragma unused w
}

public ls_on_npc_arrived(npc, point, last)
{
	#pragma unused point
	new s = lsg_ec_npc_find(npc)
	if (s < 0)
		return
	new ctx[LSG_CTX], k = g_lsgEcNpcNode[s]
	for (new i = 0; i < LSG_CTX; i++)
		ctx[i] = g_lsgEcNpcCtx[s][i]
	if (last)
		g_lsgEcNpcEnt[s] = 0
	switch (k)
	{
		case 0:
		{
			if (last)
				lsg_n33(ctx)
		}
		case 1:
		{
			lsg_n36(ctx)
			if (last)
				lsg_n37(ctx)
		}
	}
	#pragma unused ctx
}

public ls_on_npc_move_failed(npc, point)
{
	#pragma unused point
	new s = lsg_ec_npc_find(npc)
	if (s < 0)
		return
	new ctx[LSG_CTX], k = g_lsgEcNpcNode[s]
	for (new i = 0; i < LSG_CTX; i++)
		ctx[i] = g_lsgEcNpcCtx[s][i]
	g_lsgEcNpcEnt[s] = 0
	switch (k)
	{
		case 0: lsg_n34(ctx)
		case 2: lsg_n39(ctx)
	}
	#pragma unused ctx
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

// Deal Damage (dmg)
lsg_n1(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = ctx[0]
	new Float:lsi2 = 5.0
	if (lsd_want(1))
	{
	lsd_begin()
	lsd_in_entity("Target", lsi0)
	lsd_in_player("Attacker", lsi1)
	lsd_in_number("Amount", lsi2)
	}
	if (lsd_enter(1, ctx))
		return
	if (!lsg_ec_damage(lsi0, lsi1, floatclamp(lsi2, 0.0, 10000.0), 8, 1, true))
		lsd_error(1, "Not hurt: the target isn't there, is dead, or can't be hurt.")
	else
		lsd_exit(1, "hurt")
	lsg_n2(ctx)
}

// Set Entity Property (pset)
lsg_n2(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 1.0
	if (lsd_want(2))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(2, ctx))
		return
	if (!lsg_ec_set(lsi0, 1, lsi1, 19, lsg_vec(255.0, 255.0, 255.0)))
		lsd_error(2, "Not set: the entity isn't there, or that property can't change on it (a player's movement or collision, a brush's movement, a scale on a player).")
	else
		lsd_exit(2, "set")
	lsg_n3(ctx)
}

// Set Entity Property (pcol)
lsg_n3(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 1.0
	if (lsd_want(3))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(3, ctx))
		return
	if (!lsg_ec_set(lsi0, 2, lsi1, -1, lsg_vec(0.0, 200.0, 255.0)))
		lsd_error(3, "Not set: the entity isn't there, or that property can't change on it (a player's movement or collision, a brush's movement, a scale on a player).")
	else
		lsd_exit(3, "set")
	lsg_n4(ctx)
}

// Big HUD Text (big)
lsg_n4(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = lsg_ec_mvf(ctx[0], 6)
	if (lsd_want(4))
	{
	lsd_begin()
	lsd_in_player("Player (empty: everyone)", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(4, ctx))
		return
	if (lsg_msg_room("Big HUD Text", 1, lsi0, 48, 160))
	{
		set_dhudmessage(255, 80, 40, -1.0, 0.3, 2, 0.5, 3.0, 0.05, 0.5)
		show_dhudmessage(lsi0, "%s %s", "Go!", lsg_ntext(lsi1))
	}
	lsd_exit(4, "shown")
	lsg_n5(ctx)
}

// Colored Chat (cc)
lsg_n5(ctx[])
{
	new lsi0 = ctx[0]
	new lsi2 = ctx[0]
	if (lsd_want(5))
	{
	lsd_begin()
	lsd_in_player("Player (empty: everyone)", lsi0)
	lsd_in_player("Color of", lsi2)
	}
	if (lsd_enter(5, ctx))
		return
	if (lsg_msg_room("Colored Chat", 1, lsi0, 48, 200))
	{
		new lsgM[190]
		formatex(lsgM, charsmax(lsgM), "%s", "Hi")
		lsg_ec_clean(lsgM)
		client_print_color(lsi0, lsg_ec_sender(lsi2), "^4[EC]^1 ^3%s", lsgM)
	}
	lsd_exit(5, "said")
	lsg_n6(ctx)
}

// Colored Chat (cc2)
lsg_n6(ctx[])
{
	new lsi1[128]
	copy(lsi1, 127, lsg_name(ctx[0]))
	if (lsd_want(6))
	{
	lsd_begin()
	lsd_in_text("Value", lsi1)
	}
	if (lsd_enter(6, ctx))
		return
	if (lsg_msg_room("Colored Chat", 1, 0, 48, 200))
	{
		new lsgM[190]
		formatex(lsgM, charsmax(lsgM), "%s %s", "Green 100%", lsi1)
		lsg_ec_clean(lsgM)
		client_print_color(0, print_team_default, "^4%s", lsgM)
	}
	lsd_exit(6, "said")
	lsg_n7(ctx)
}

// Push (push)
lsg_n7(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi3 = 300.0
	if (lsd_want(7))
	{
	lsd_begin()
	lsd_in_entity("Player or entity", lsi0)
	lsd_in_number("Strength", lsi3)
	}
	if (lsd_enter(7, ctx))
		return
	if (!lsg_ec_push(lsi0, 4, -1, -1, floatclamp(lsi3, 0.0, 3000.0), 0.0, false))
		lsd_error(7, "Not pushed: nothing there, a dead player, or no Direction / Point wired in for that choice.")
	else
		lsd_exit(7, "pushed")
	lsg_n8(ctx)
}

// Jump Rules (jr)
lsg_n8(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(8))
	{
	lsd_begin()
	lsd_in_player("Player (empty: everyone)", lsi0)
	}
	if (lsd_enter(8, ctx))
		return
	lsg_ec_jump(lsi0, 12, 2, 268.0)
	lsd_exit(8, "set")
	lsg_n9(ctx)
}

// Jump Rules (jra)
lsg_n9(ctx[])
{
	if (lsd_want(9))
	{
	lsd_begin()
	}
	if (lsd_enter(9, ctx))
		return
	lsg_ec_jump(0, 18, 0, 268.0)
	lsd_exit(9, "set")
	lsg_n10(ctx)
}

// Play Radio (radio)
lsg_n10(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = ctx[0]
	if (lsd_want(10))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_player("From", lsi1)
	}
	if (lsd_enter(10, ctx))
		return
	if (lsg_msg_room("Play Radio", 2, lsi0, 48, 160))
		lsg_ec_radio(lsi0, 0, lsi1, "%!MRAD_GO", "#Go_go_go", false)
	lsd_exit(10, "played")
	lsg_n11(ctx)
}

// Play Radio (radio2)
lsg_n11(ctx[])
{
	if (lsd_want(11))
	{
	lsd_begin()
	}
	if (lsd_enter(11, ctx))
		return
	if (lsg_msg_room("Play Radio", 2, 0, 48, 160))
		lsg_ec_radio(0, 1, 0, "%!MRAD_terwin", "#Terrorists_Win", true)
	lsd_exit(11, "played")
	lsg_n12(ctx)
}

// Show Info Window (motd)
lsg_n12(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(12))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(12, ctx))
		return
	if (!lsg_ec_motd(lsi0, "Rules", "Hello [b]you[b][br]Have fun <3 & more", ""))
		lsd_error(12, "Not shown: nobody there, or a bot.")
	else
		lsd_exit(12, "shown")
	lsg_n13(ctx)
}

// Voice Override (vo)
lsg_n13(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(13))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(13, ctx))
		return
	new p = lsi0
	if (lsg_ok(p))
		g_lsgEcVoiceOv[p] = 3
	lsd_exit(13, "set")
	lsg_n14(ctx)
}

// Print String (pr1)
lsg_n14(ctx[])
{
	new lsi0 = lsg_ec_mv(ctx[0], 0)
	if (lsd_want(14))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(14, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "state", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(14, esc)
	lsg_n15(ctx)
}

// Print String (pr2)
lsg_n15(ctx[])
{
	new Float:lsi0 = lsg_ec_mvf(ctx[0], 5)
	if (lsd_want(15))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(15, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "speed", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(15, esc)
	lsg_n16(ctx)
}

// Print String (pr3)
lsg_n16(ctx[])
{
	new lsi0 = lsg_ec_inzone(ctx[0], 2)
	if (lsd_want(16))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(16, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "zone", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(16, esc)
	lsg_n17(ctx)
}

// Print String (pr4)
lsg_n17(ctx[])
{
	new Float:lsi0 = lsg_ec_get(ctx[0], 10)
	if (lsd_want(17))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(17, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "prop", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(17, esc)
	lsg_n18(ctx)
}

// Print String (pr5)
lsg_n18(ctx[])
{
	new lsi0 = lsg_ec_getcolor(ctx[0])
	if (lsd_want(18))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(18, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "color", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(18, esc)
	lsg_n19(ctx)
}

// Print String (prm0)
lsg_n19(ctx[])
{
	new lsi0 = lsg_ec_mv(ctx[0], 1)
	if (lsd_want(19))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(19, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "duck", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(19, esc)
	lsg_n20(ctx)
}

// Print String (prm1)
lsg_n20(ctx[])
{
	new lsi0 = lsg_ec_mv(ctx[0], 2)
	if (lsd_want(20))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(20, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "water", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(20, esc)
	lsg_n21(ctx)
}

// Print String (prm2)
lsg_n21(ctx[])
{
	new Float:lsi0 = lsg_ec_mvf(ctx[0], 3)
	if (lsd_want(21))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(21, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "level", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(21, esc)
	lsg_n22(ctx)
}

// Print String (prm3)
lsg_n22(ctx[])
{
	new lsi0 = lsg_ec_mv(ctx[0], 4)
	if (lsd_want(22))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(22, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "ladder", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(22, esc)
}

// Map Started (mst)
lsg_n23(ctx[])
{
	if (lsd_want(23))
	{
	lsd_begin()
	}
	if (lsd_enter(23, ctx))
		return
	lsd_exit(23, "ran")
	lsg_n24(ctx)
}

// Remove Map Objectives (noobj)
lsg_n24(ctx[])
{
	if (lsd_want(24))
	{
	lsd_begin()
	}
	if (lsd_enter(24, ctx))
		return
	lsg_ec_noobj(62)
	lsd_exit(24, "removed")
	lsg_n25(ctx)
}

// Voice Rules (voice)
lsg_n25(ctx[])
{
	if (lsd_want(25))
	{
	lsd_begin()
	}
	if (lsd_enter(25, ctx))
		return
	g_lsgEcVoice = 3
	g_lsgEcVoiceDist = 1500.0
	lsd_exit(25, "set")
	lsg_n26(ctx)
}

// Print String (prt)
lsg_n26(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, lsg_ec_mtype())
	if (lsd_want(26))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(26, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "map", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(26, esc)
	lsg_n27(ctx)
}

// Print String (prt0)
lsg_n27(ctx[])
{
	new lsi0 = (lsg_ec_mhas(1) ? 1 : 0)
	if (lsd_want(27))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(27, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "bomb", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(27, esc)
	lsg_n28(ctx)
}

// Print String (prt1)
lsg_n28(ctx[])
{
	new lsi0 = (lsg_ec_mhas(2) ? 1 : 0)
	if (lsd_want(28))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(28, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "hostage", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(28, esc)
	lsg_n29(ctx)
}

// Print String (prt2)
lsg_n29(ctx[])
{
	new lsi0 = (lsg_ec_mhas(3) ? 1 : 0)
	if (lsd_want(29))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(29, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "vip", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(29, esc)
	lsg_n30(ctx)
}

// Print String (prt3)
lsg_n30(ctx[])
{
	new lsi0 = (lsg_ec_mhas(4) ? 1 : 0)
	if (lsd_want(30))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(30, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "escape", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(30, esc)
	lsg_n31(ctx)
}

// Spawn NPC (npc)
lsg_n31(ctx[])
{
	new lsi1 = lsg_vec(0.0, 0.0, 0.0)
	new Float:lsi6 = 1.0
	if (lsd_want(31))
	{
	lsd_begin()
	lsd_in_text("At position", lsg_vtext(lsi1))
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(31, ctx))
		return
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 31
	if (!lsgRe)
		g_lsgXrN[31] = 0
	g_lsgXrIn = -1
	if (!(ls_components_running()))
		lsgWhy = 6
	else if (!(lsg_ent_room("Spawn NPC")))
		lsgWhy = 2
	else if (!(lsg_xf_at(0, lsi1, -1, o)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, 2, 2, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_vis_note(ls_npc_spawn("models/barney.mdl", o, 0.0, "Walker", "behavior=stand unkillable=1 blocked=here"))))
		lsgWhy = 6
	new bool:lsgWait = false
	g_lsgOut[31] = e
	g_lsgO[0] = e
	g_lsgO[1] = e ? 1 : 0
	g_lsgO[2] = e ? lsgMv : 0
	g_lsgO[3] = e ? 0 : lsgWhy
	if (e)
	{
		if (lsgMv)
		lsd_exit(31, "moved to a nearby free spot")
		else
		lsd_exit(31, "spawned")
	}
	else if (lsgWait)
		lsd_exit(31, "blocked: trying again")
	else
	{
		switch (lsgWhy)
		{
			case 6: lsd_error(31, "gm_components isn't running on this server.")
			case 2: lsd_error(31, "Not made: the server is out of entity slots.")
			case 1: lsd_error(31, "The spot was blocked (a player, a wall or something solid there) and no free spot was found within 128 units.")
			case 5: lsd_error(31, "The spot stayed blocked: every retry was used up.")
		}
	}
	if (!lsgRe)
		lsg_n32(ctx)
}

// NPC Move To (mv)
lsg_n32(ctx[])
{
	new lsi0 = g_lsgO[0]
	new lsi2 = lsg_vec(128.0, 0.0, 0.0)
	if (lsd_want(32))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	lsd_in_text("Or point", lsg_vtext(lsi2))
	}
	if (lsd_enter(32, ctx))
		return
	new lsgNpc = lsi0
	if (lsg_ec_npc_to(lsi0, 0, lsi2, 220.0, 6))
		lsg_ec_npc_keep(lsgNpc, 0, ctx, 32)
	else
	{
		lsd_error(32, "Not moving: not an NPC (or dead), gm_components isn't running, or no points.")
		lsg_n34(ctx)
	}
	lsd_exit(32, "moving")
	lsg_n40(ctx)
}

// Print String (arr)
lsg_n33(ctx[])
{
	if (lsd_want(33))
	{
	lsd_begin()
	}
	if (lsd_enter(33, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "arrived")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(33, esc)
	lsg_n35(ctx)
}

// Print String (fail)
lsg_n34(ctx[])
{
	if (lsd_want(34))
	{
	lsd_begin()
	}
	if (lsd_enter(34, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "failed")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(34, esc)
	lsg_n38(ctx)
}

// NPC Follow Path (fp)
lsg_n35(ctx[])
{
	new lsi0 = g_lsgO[0]
	new lsi1 = lsg_mka44(lsg_vec(128.0, 0.0, 0.0), lsg_vec(128.0, 128.0, 0.0))
	if (lsd_want(35))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	lsd_in_text("Points", fmt("%d items", lsg_alen(lsi1)))
	}
	if (lsd_enter(35, ctx))
		return
	new lsgNpc = lsi0
	if (lsg_ec_npc_path(lsi0, lsi1, "path1", 90.0, 0))
		lsg_ec_npc_keep(lsgNpc, 1, ctx, 32)
	else
	{
		lsd_error(35, "Not moving: not an NPC (or dead), gm_components isn't running, or no points.")
	}
	lsd_exit(35, "moving")
}

// Print String (atp)
lsg_n36(ctx[])
{
	if (lsd_want(36))
	{
	lsd_begin()
	}
	if (lsd_enter(36, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "at point")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(36, esc)
}

// Print String (fin)
lsg_n37(ctx[])
{
	if (lsd_want(37))
	{
	lsd_begin()
	}
	if (lsd_enter(37, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "finished")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(37, esc)
}

// NPC Follow Path (fp2)
lsg_n38(ctx[])
{
	new lsi0 = g_lsgO[0]
	if (lsd_want(38))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	}
	if (lsd_enter(38, ctx))
		return
	new lsgNpc = lsi0
	if (lsg_ec_npc_path(lsi0, -1, "path1", 90.0, 1))
		lsg_ec_npc_keep(lsgNpc, 2, ctx, 32)
	else
	{
		lsd_error(38, "Not moving: not an NPC (or dead), gm_components isn't running, or no points.")
		lsg_n39(ctx)
	}
	lsd_exit(38, "moving")
}

// NPC Stop (stop)
lsg_n39(ctx[])
{
	new lsi0 = g_lsgO[0]
	if (lsd_want(39))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	}
	if (lsd_enter(39, ctx))
		return
	lsg_ec_npc_stop(lsi0, true)
	lsd_exit(39, "stopped")
}

// Spawn Prop (prop)
lsg_n40(ctx[])
{
	new lsi0 = g_lsgO[0]
	new Float:lsi6 = 1.0
	if (lsd_want(40))
	{
	lsd_begin()
	lsd_in_entity("At entity", lsi0)
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(40, ctx))
		return
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 40
	if (!lsgRe)
		g_lsgXrN[40] = 0
	g_lsgXrIn = -1
	if (!(lsg_have("models/w_c4.mdl")))
		lsgWhy = 4
	else if (!(lsg_ent_room("Spawn Prop")))
		lsgWhy = 2
	else if (!(lsg_ent_origin(lsi0, o)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, -1, 2, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_vis_note(lsg_prop_o(o, "models/w_c4.mdl", 0))))
		lsgWhy = 6
	new bool:lsgWait = false
	g_lsgOut[40] = e
	g_lsgO[4] = e
	g_lsgO[5] = e ? 1 : 0
	g_lsgO[6] = e ? lsgMv : 0
	g_lsgO[7] = e ? 0 : lsgWhy
	if (e)
	{
		if (lsgMv)
		lsd_exit(40, "moved to a nearby free spot")
		else
		lsd_exit(40, "spawned")
	}
	else if (lsgWait)
		lsd_exit(40, "blocked: trying again")
	else
	{
		switch (lsgWhy)
		{
			case 4: lsd_error(40, "No prop: its model isn't loaded with the map.")
			case 2: lsd_error(40, "No prop: the server is out of entity slots.")
			case 6: lsd_error(40, "No place: no such entity, or nothing wired into At.")
			case 1: lsd_error(40, "The spot was blocked (a player, a wall or something solid there) and no free spot was found within 128 units.")
			case 5: lsd_error(40, "The spot stayed blocked: every retry was used up.")
		}
	}
	if (!lsgRe)
		lsg_n41(ctx)
}

// Play Animation (anim)
lsg_n41(ctx[])
{
	new lsi0 = g_lsgO[4]
	if (lsd_want(41))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(41, ctx))
		return
	if (!lsg_ec_anim(lsi0, "idle", -1, 1.0, true, true, 1.5, 64))
		lsd_error(41, "Not played: not a model entity (players are animated by the game), no sequence by that name, or too many looping already (Limits).")
	else
		lsd_exit(41, "playing")
	lsg_n42(ctx)
}

// Attach To (att)
lsg_n42(ctx[])
{
	new lsi0 = g_lsgO[4]
	new lsi1 = g_lsgO[0]
	new lsi2 = lsg_vec(128.0, 0.0, 0.0)
	if (lsd_want(42))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_entity("Parent", lsi1)
	lsd_in_text("Offset", lsg_vtext(lsi2))
	}
	if (lsd_enter(42, ctx))
		return
	if (!lsg_ec_attach(lsi0, lsi1, lsi2, -1, true, 64))
		lsd_error(42, "Not attached: a player can't be attached, the parent isn't there, it's itself, or too many are attached already (Limits).")
	else
		lsd_exit(42, "attached")
	lsg_n43(ctx)
}

// Attach To (att2)
lsg_n43(ctx[])
{
	new lsi0 = g_lsgO[4]
	new lsi1 = g_lsgO[0]
	if (lsd_want(43))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_entity("Parent", lsi1)
	}
	if (lsd_enter(43, ctx))
		return
	if (!lsg_ec_attach(lsi0, lsi1, -1, 0, false, 64))
		lsd_error(43, "Not attached: a player can't be attached, the parent isn't there, it's itself, or too many are attached already (Limits).")
	else
		lsd_exit(43, "attached")
	lsg_n44(ctx)
}

// Detach (det)
lsg_n44(ctx[])
{
	new lsi0 = g_lsgO[4]
	if (lsd_want(44))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(44, ctx))
		return
	lsg_ec_detach(lsi0, true)
	lsd_exit(44, "detached")
	lsg_n45(ctx)
}

// Set Entity Property (pset2)
lsg_n45(ctx[])
{
	new lsi0 = g_lsgO[4]
	new Float:lsi1 = 2.0
	if (lsd_want(45))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_number("Value", lsi1)
	}
	if (lsd_enter(45, ctx))
		return
	if (!lsg_ec_set(lsi0, 6, lsi1, -1, lsg_vec(255.0, 255.0, 255.0)))
		lsd_error(45, "Not set: the entity isn't there, or that property can't change on it (a player's movement or collision, a brush's movement, a scale on a player).")
	else
		lsd_exit(45, "set")
}

// Grenade Thrown (gt)
lsg_n46(ctx[])
{
	if (lsd_want(46))
	{
	lsd_begin()
	}
	if (lsd_enter(46, ctx))
		return
	lsd_exit(46, "ran")
	lsg_n47(ctx)
}

// Set Grenade Fuse (fuse)
lsg_n47(ctx[])
{
	new lsi0 = ctx[1]
	new Float:lsi1 = 0.5
	if (lsd_want(47))
	{
	lsd_begin()
	lsd_in_entity("Grenade", lsi0)
	lsd_in_number("Seconds", lsi1)
	}
	if (lsd_enter(47, ctx))
		return
	if (!lsg_ec_fuse(lsi0, floatclamp(lsi1, 0.0, 60.0)))
		lsd_error(47, "Not set: that isn't a thrown grenade (or it went off already).")
	else
		lsd_exit(47, "set")
	lsg_n48(ctx)
}

// Push (gpush)
lsg_n48(ctx[])
{
	new lsi0 = ctx[1]
	new lsi1 = ctx[2]
	new Float:lsi3 = 200.0
	if (lsd_want(48))
	{
	lsd_begin()
	lsd_in_entity("Player or entity", lsi0)
	lsd_in_text("Direction", lsg_vtext(lsi1))
	lsd_in_number("Strength", lsi3)
	}
	if (lsd_enter(48, ctx))
		return
	if (!lsg_ec_push(lsi0, 2, lsi1, -1, floatclamp(lsi3, 0.0, 3000.0), 0.0, false))
		lsd_error(48, "Not pushed: nothing there, a dead player, or no Direction / Point wired in for that choice.")
	else
		lsd_exit(48, "pushed")
	lsg_n49(ctx)
}

// Print String (gprt)
lsg_n49(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[0])
	if (lsd_want(49))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(49, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "thrown", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(49, esc)
	lsg_n54(ctx)
}

// Grenade Exploded (ge)
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

// Print String (gprt2)
lsg_n51(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[0])
	if (lsd_want(51))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(51, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "boom", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(51, esc)
	lsg_n52(ctx)
}

// Push (gpush2)
lsg_n52(ctx[])
{
	new lsi0 = ctx[0]
	new lsi2 = ctx[2]
	new Float:lsi3 = 400.0
	if (lsd_want(52))
	{
	lsd_begin()
	lsd_in_entity("Player or entity", lsi0)
	lsd_in_text("Away from point", lsg_vtext(lsi2))
	lsd_in_number("Strength", lsi3)
	}
	if (lsd_enter(52, ctx))
		return
	if (!lsg_ec_push(lsi0, 3, -1, lsi2, floatclamp(lsi3, 0.0, 3000.0), 200.0, true))
		lsd_error(52, "Not pushed: nothing there, a dead player, or no Direction / Point wired in for that choice.")
	else
		lsd_exit(52, "pushed")
	lsg_n53(ctx)
}

// Print String (gprt3)
lsg_n53(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(53))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(53, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "grenade", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(53, esc)
}

// Print String (gprt4)
lsg_n54(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(54))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(54, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "thrower", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(54, esc)
}

// On Map Entity Fired (mf)
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

// Print String (mfp)
lsg_n56(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[0])
	if (lsd_want(56))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(56, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "fired", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(56, esc)
	lsg_n57(ctx)
}

// Print String (mfp2)
lsg_n57(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(57))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(57, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "by", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(57, esc)
	lsg_n58(ctx)
}

// Print String (mfp3)
lsg_n58(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(58))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(58, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "ent", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(58, esc)
	lsg_n59(ctx)
}

// Print String (mfp4)
lsg_n59(ctx[])
{
	new lsi0 = ctx[2]
	if (lsd_want(59))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(59, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "caller", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(59, esc)
}

// On Map Entity Fired (mf2)
lsg_n60(ctx[])
{
	if (lsd_want(60))
	{
	lsd_begin()
	}
	if (lsd_enter(60, ctx))
		return
	lsd_exit(60, "ran")
	lsg_n61(ctx)
}

// Print String (mf2p)
lsg_n61(ctx[])
{
	if (lsd_want(61))
	{
	lsd_begin()
	}
	if (lsd_enter(61, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "door1")
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(61, esc)
}

// Weapon Switched (ws)
lsg_n62(ctx[])
{
	if (lsd_want(62))
	{
	lsd_begin()
	}
	if (lsd_enter(62, ctx))
		return
	lsd_exit(62, "ran")
	lsg_n63(ctx)
}

// Print String (wsp)
lsg_n63(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[0])
	if (lsd_want(63))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(63, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "new", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(63, esc)
	lsg_n64(ctx)
}

// Print String (wsp2)
lsg_n64(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[1])
	if (lsd_want(64))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(64, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "old", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(64, esc)
	lsg_n65(ctx)
}

// Print String (wsp3)
lsg_n65(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(65))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(65, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "weapon", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(65, esc)
	lsg_n66(ctx)
}

// Print String (wsp4)
lsg_n66(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(66))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(66, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "who", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(66, esc)
}

// Entered Map Zone (ze)
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

// Colored Chat (zep)
lsg_n68(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(68))
	{
	lsd_begin()
	lsd_in_player("Player (empty: everyone)", lsi0)
	}
	if (lsd_enter(68, ctx))
		return
	if (lsg_msg_room("Colored Chat", 1, lsi0, 48, 200))
	{
		new lsgM[190]
		formatex(lsgM, charsmax(lsgM), "%s", "Buy zone")
		lsg_ec_clean(lsgM)
		client_print_color(lsi0, print_team_blue, "^4[Zone]^1 ^3%s", lsgM)
	}
	lsd_exit(68, "said")
}

// Left Map Zone (zl)
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

// Big HUD Text (zlp)
lsg_n70(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(70))
	{
	lsd_begin()
	lsd_in_player("Player (empty: everyone)", lsi0)
	}
	if (lsd_enter(70, ctx))
		return
	if (lsg_msg_room("Big HUD Text", 1, lsi0, 48, 160))
	{
		set_dhudmessage(255, 255, 255, 0.1, 0.8, 1, 1.0, 3.0, 0.1, 0.5)
		show_dhudmessage(lsi0, "%s", "Left the site")
	}
	lsd_exit(70, "shown")
}

// Round Started (rs)
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

// Round End Rules (rer)
lsg_n72(ctx[])
{
	if (lsd_want(72))
	{
	lsd_begin()
	}
	if (lsd_enter(72, ctx))
		return
	set_cvar_string("mp_round_infinite", "afhijk")
	lsd_exit(72, "set")
	lsg_n73(ctx)
}

// Start Vote (vote)
lsg_n73(ctx[])
{
	if (lsd_want(73))
	{
	lsd_begin()
	}
	if (lsd_enter(73, ctx))
		return
	if (lsg_ec_vote_begin(0, ctx))
	{
		new m = menu_create("Weapons?", "lsg_ec_vote_pick")
		menu_additem(m, "Knives")
		menu_additem(m, "Pistols")
		menu_additem(m, "Rifles")
		lsg_ec_vote_show(m, 3, 0, true, 10.0)
	}
	else
	{
		lsd_error(73, "Not started: another vote is on (this mode's or the server's map vote).")
		lsg_n77(ctx)
	}
	lsd_exit(73, "voting")
	lsg_n78(ctx)
}

// Print String (v_w1)
lsg_n74(ctx[])
{
	new Float:lsi0 = float(g_lsgEcVoteN[clamp(floatround(0.0), 0, 8)])
	if (lsd_want(74))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(74, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "knives won", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(74, esc)
}

// Print String (v_w3)
lsg_n75(ctx[])
{
	if (lsd_want(75))
	{
	lsd_begin()
	}
	if (lsd_enter(75, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "rifles won")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(75, esc)
}

// Print String (v_tie)
lsg_n76(ctx[])
{
	if (lsd_want(76))
	{
	lsd_begin()
	}
	if (lsd_enter(76, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "tie")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(76, esc)
}

// Print String (v_none)
lsg_n77(ctx[])
{
	if (lsd_want(77))
	{
	lsd_begin()
	}
	if (lsd_enter(77, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s", "no votes")
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(77, esc)
}

// Add Bots (bota)
lsg_n78(ctx[])
{
	new Float:lsi0 = 2.0
	if (lsd_want(78))
	{
	lsd_begin()
	lsd_in_number("How many", lsi0)
	}
	if (lsd_enter(78, ctx))
		return
	if (!lsg_ec_bots_add(clamp(floatround(lsi0), 1, 31), 2, 2))
		lsd_error(78, "No bots added: no free slot (one is kept for a person).")
	else
		lsd_exit(78, "added")
	lsg_n79(ctx)
}

// Remove Bots (botr)
lsg_n79(ctx[])
{
	new Float:lsi0 = 1.0
	if (lsd_want(79))
	{
	lsd_begin()
	lsd_in_number("How many", lsi0)
	}
	if (lsd_enter(79, ctx))
		return
	if (!lsg_ec_bots_remove(clamp(floatround(lsi0), 1, 31), 1))
		lsd_error(79, "No bots removed: none on that team.")
	else
		lsd_exit(79, "removed")
}

// Player Spawned (ps2)
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

// Print String (hfp)
lsg_n81(ctx[])
{
	new lsi0 = (lsg_ok(ctx[0]) && (get_user_flags(ctx[0]) & 32) ? 1 : 0)
	if (lsd_want(81))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(81, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "can change map", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(81, esc)
	lsg_n82(ctx)
}

// Hostage Follow (hfol)
lsg_n82(ctx[])
{
	new lsi0 = lsg_agetc(lsg_find_all(0, "hostage_entity"), 0.0)
	new lsi1 = ctx[0]
	if (lsd_want(82))
	{
	lsd_begin()
	lsd_in_entity("Hostage", lsi0)
	lsd_in_player("Leader", lsi1)
	}
	if (lsd_enter(82, ctx))
		return
	if (!lsg_ec_hfollow(lsi0, lsi1))
		lsd_error(82, "Not following: not a hostage (or a rescued or dead one), or the leader isn't alive.")
	else
		lsd_exit(82, "following")
	lsg_n83(ctx)
}

// Print String (hifp)
lsg_n83(ctx[])
{
	new lsi0 = (lsg_ec_hleader(lsg_agetc(lsg_find_all(0, "hostage_entity"), 0.0)) > 0 ? 1 : 0)
	if (lsd_want(83))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(83, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "following", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(83, esc)
	lsg_n84(ctx)
}

// Print String (hifp2)
lsg_n84(ctx[])
{
	new lsi0 = lsg_ec_hleader(lsg_agetc(lsg_find_all(0, "hostage_entity"), 0.0))
	if (lsd_want(84))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(84, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "leader", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(84, esc)
	lsg_n85(ctx)
}

// Hostage Stop (hstop)
lsg_n85(ctx[])
{
	new lsi0 = lsg_agetc(lsg_find_all(0, "hostage_entity"), 0.0)
	if (lsd_want(85))
	{
	lsd_begin()
	lsd_in_entity("Hostage", lsi0)
	}
	if (lsd_enter(85, ctx))
		return
	if (!lsg_ec_hfollow(lsi0, 0))
		lsd_error(85, "Not a hostage (or a rescued or dead one).")
	else
		lsd_exit(85, "stopped")
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
	}
}
