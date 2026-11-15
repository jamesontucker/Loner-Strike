/*
	Every node 5 - collision and hits - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from every_node_5.graph.json: change the graph and compile it again; edits here are lost.
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

#define LSG_MODE "every_node_5"
#define LSG_CTX 9
#define LSG_TXT 2
#define LSG_N 31
#define LSG_TASK 7310000
#pragma dynamic 23936
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
#define LSG_O 10
#define LSG_OT 1

stock Float:g_lsgDmg, bool:g_lsgDmgBlock
#define LSD_N 31
new const g_lsdIds[LSD_N][] = { "rs", "pk", "semi", "semi2", "vs", "vs2", "prop", "solid", "size", "dmg", "hp", "phpr", "povl", "rm", "av", "av2", "avs", "ob", "pob", "oe", "poe", "ot", "pot", "bh", "pbh", "ld", "pld", "ah", "pah", "pt", "ppt" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
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
// (what the mode has out that players see: lsg_vis_note, below)
#define LSG_VISIBLE_MOST 128
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
// the mode keeps at most LSG_VISIBLE_MOST visible things of its own out at once (trigger volumes and
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

// (a spawn that players see, when the mode already has LSG_VISIBLE_MOST of them out: lsg_ent_room)
stock bool:lsg_vis_full(const what[], bool:visible)
{
	if (!visible || g_lsgVisN < LSG_VISIBLE_MOST)
		return false
	lsg_skip_log("visible", what, "%s skipped: the mode has %d things players see out (their games are sent at most 256 at once)", what, g_lsgVisN)
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


// ---- collision, overlap and hits (W2) ----
stock g_lsgSemi, bool:g_lsgSemiOn[33], g_lsgSemiOff[33][33], g_lsgSemiN[33]
// (sized by the Limits' most: Trigger volumes at once 256, Things inside one 64; the mode's own
// settings - lim_trigger_volumes, lim_volume_members - say how many it really keeps)
#define LSG_VOLS 256
#define LSG_VOLIN 64
stock g_lsgVol[LSG_VOLS], g_lsgVolShape[LSG_VOLS], g_lsgVolFilter[LSG_VOLS], g_lsgVolAt[LSG_VOLS], Float:g_lsgVolPos[LSG_VOLS][3], Float:g_lsgVolSize[LSG_VOLS][3]
stock Float:g_lsgVolAng[LSG_VOLS][3], Float:g_lsgVolOff[LSG_VOLS][3], Float:g_lsgVolTurn[LSG_VOLS][3]
stock g_lsgVolIn[LSG_VOLS][LSG_VOLIN], g_lsgVolInN[LSG_VOLS], g_lsgVolInCap[LSG_VOLS]
stock g_lsgAvoidTag[32][24], g_lsgAvoidN
stock g_lsgFiring, Float:g_lsgAirVel[33], bool:g_lsgAirborne[33]

stock bool:lsg_col_thing(e)
{
	// (a prop, an NPC, a spawned thing: not a player, not the world)
	return e > MaxClients && pev_valid(e)
}

stock lsg_semiclip(mode, id, on)
{
	if (mode == 3)
	{
		if (lsg_ok(id))
			g_lsgSemiOn[id] = on != 0
		// (nobody chosen any more: off)
		new any = 0
		for (new p = 1; p <= MaxClients; p++)
			if (g_lsgSemiOn[p])
				any = 1
		if (!any)
			mode = 0
	}
	new was = g_lsgSemi
	g_lsgSemi = mode
	// (turned off with players inside each other: nudged free in a moment)
	if (was && !mode)
		set_task(0.1, "lsg_semi_unstick", LSG_TASK - 420)
	return 1
}

stock bool:lsg_semi_pair(a, b)
{
	switch (g_lsgSemi)
	{
		case 1: return true
		case 2: return get_user_team(a) == get_user_team(b)
		case 3: return g_lsgSemiOn[a] || g_lsgSemiOn[b]
	}
	return false
}

public lsg_semi_unstick()
{
	static const Float:TRY[][3] = { {0.0, 0.0, 0.0}, {32.0, 0.0, 0.0}, {-32.0, 0.0, 0.0}, {0.0, 32.0, 0.0}, {0.0, -32.0, 0.0}, {24.0, 24.0, 0.0}, {-24.0, -24.0, 0.0}, {24.0, -24.0, 0.0}, {-24.0, 24.0, 0.0}, {0.0, 0.0, 36.0}, {48.0, 0.0, 0.0}, {-48.0, 0.0, 0.0}, {0.0, 48.0, 0.0}, {0.0, -48.0, 0.0} }
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_ok(p) || !is_user_alive(p))
			continue
		new Float:o[3], Float:t[3], hull = (pev(p, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN
		pev(p, pev_origin, o)
		engfunc(EngFunc_TraceHull, o, o, 0, hull, p, 0)
		if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			continue
		for (new i = 1; i < sizeof TRY; i++)
		{
			for (new k = 0; k < 3; k++)
				t[k] = o[k] + TRY[i][k]
			engfunc(EngFunc_TraceHull, t, t, 0, hull, p, 0)
			if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			{
				engfunc(EngFunc_SetOrigin, p, t)
				break
			}
		}
	}
}

stock lsg_set_solid(e, kind)
{
	if (!lsg_col_thing(e))
		return 0
	new model[4]
	pev(e, pev_model, model, charsmax(model))
	// (a brush keeps its own solid kind when it's made solid again)
	new solid = kind == 1 ? SOLID_TRIGGER : (kind == 2 ? SOLID_NOT : (model[0] == '*' ? SOLID_BSP : SOLID_BBOX))
	set_pev(e, pev_solid, solid)
	new Float:o[3]
	pev(e, pev_origin, o)
	engfunc(EngFunc_SetOrigin, e, o)
	return 1
}

stock lsg_set_size(e, Float:mn0, Float:mn1, Float:mn2, Float:mx0, Float:mx1, Float:mx2)
{
	if (!lsg_col_thing(e) || mn0 >= mx0 || mn1 >= mx1 || mn2 >= mx2)
		return 0
	new Float:mn[3], Float:mx[3]
	mn[0] = mn0
	mn[1] = mn1
	mn[2] = mn2
	mx[0] = mx0
	mx[1] = mx1
	mx[2] = mx2
	engfunc(EngFunc_SetSize, e, mn, mx)
	return 1
}

stock lsg_takes_damage(e, on)
{
	if (!lsg_col_thing(e))
		return 0
	set_pev(e, pev_takedamage, on ? DAMAGE_YES : DAMAGE_NO)
	return 1
}

stock Float:lsg_ent_health(e)
{
	new Float:hp = 0.0
	if (e > 0 && pev_valid(e))
		pev(e, pev_health, hp)
	return hp
}

stock lsg_set_ent_health(e, Float:hp)
{
	if (!lsg_col_thing(e))
		return 0
	set_pev(e, pev_health, floatmax(hp, 1.0))
	return 1
}

// ---- trigger volumes: an info_target that holds the name; checked ten times a second
// where a volume is and how it's turned: at its entity (plus the local offset by the entity's facing,
// and the rotation on top of the entity's) or at its position
stock lsg_vol_place(v)
{
	new at = g_lsgVolAt[v]
	if (!at)
	{
		g_lsgVolTurn[v] = g_lsgVolAng[v]
		return
	}
	new Float:o[3], Float:a[3], Float:f[3], Float:r[3], Float:u[3]
	lsg_ent_origin(at, o)
	pev(at, pev_angles, a)
	if (at <= MaxClients)
	{
		// (a player: the way they face, level)
		pev(at, pev_v_angle, a)
		a[0] = 0.0
	}
	engfunc(EngFunc_MakeVectors, a)
	global_get(glb_v_forward, f)
	global_get(glb_v_right, r)
	global_get(glb_v_up, u)
	for (new k = 0; k < 3; k++)
	{
		g_lsgVolPos[v][k] = o[k] + f[k] * g_lsgVolOff[v][0] + r[k] * g_lsgVolOff[v][1] + u[k] * g_lsgVolOff[v][2]
		g_lsgVolTurn[v][k] = a[k] + g_lsgVolAng[v][k]
	}
}

// why the last Spawn Trigger Volume failed: the Spawn Failure enum (0 None, 2 Out of entity slots,
// 3 Limit reached, 6 Bad input; gen/fx.rs WHY_*), and its words for the debugger
stock g_lsgVolWhy
stock const g_lsgVolWhyName[][] = { "", "Blocked", "Out of entity slots", "Limit reached: Trigger volumes at once (Limits)", "Missing file", "Retries used up", "Bad input: At entity is gone and there's no Position" }

stock lsg_vol_spawn(at, const Float:pos[3], const Float:ang[3], const Float:off[3], shape, Float:sx, Float:sy, Float:sz, filter, const name[], cap, incap, need_at)
{
	g_lsgVolWhy = 0
	if (need_at && !(at > 0 && pev_valid(at)))
	{
		g_lsgVolWhy = 6
		return 0
	}
	new v = -1
	for (new i = 0; i < min(cap, LSG_VOLS) && v < 0; i++)
		if (!g_lsgVol[i] || !pev_valid(g_lsgVol[i]))
			v = i
	if (v < 0)
	{
		lsg_skip_log("trigger_volumes", "Spawn Trigger Volume", "Spawn Trigger Volume skipped: as many are out as Trigger volumes at once allows (%d, Limits)", cap)
		g_lsgVolWhy = 3
		return 0
	}
	if (!lsg_ent_room("Spawn Trigger Volume", false))
	{
		g_lsgVolWhy = 2
		return 0
	}
	if (!(at > 0 && pev_valid(at)))
		at = 0
	g_lsgVolAt[v] = at
	g_lsgVolAng[v] = ang
	g_lsgVolOff[v] = off
	g_lsgVolPos[v] = pos
	lsg_vol_place(v)
	new Float:o[3]
	o = g_lsgVolPos[v]
	new e = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"))
	if (!e)
	{
		g_lsgVolWhy = 2
		return 0
	}
	set_pev(e, pev_classname, "lsg_volume")
	if (name[0])
		set_pev(e, pev_targetname, name)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	set_pev(e, pev_effects, EF_NODRAW)
	engfunc(EngFunc_SetOrigin, e, o)
	g_lsgVol[v] = e
	g_lsgVolShape[v] = shape
	g_lsgVolFilter[v] = filter
	g_lsgVolInCap[v] = clamp(incap, 1, LSG_VOLIN)
	g_lsgVolSize[v][0] = sx
	g_lsgVolSize[v][1] = sy
	g_lsgVolSize[v][2] = sz
	g_lsgVolInN[v] = 0
	return e
}

// the volume is still out (a handler before may have removed it: Remove Trigger Volume frees the
// entity at once, and its index can come back as something else)
stock bool:lsg_vol_live(vol)
	return lsg_vol_slot(vol) >= 0 && pev_valid(vol) != 0

stock lsg_vol_slot(e)
{
	if (e <= 0)
		return -1
	for (new i = 0; i < LSG_VOLS; i++)
		if (g_lsgVol[i] == e)
			return i
	return -1
}

stock bool:lsg_vol_counts(v, e)
{
	if (e <= 0 || e == g_lsgVol[v] || e == g_lsgVolAt[v] || !pev_valid(e))
		return false
	if (e <= MaxClients)
		return g_lsgVolFilter[v] != 1 && is_user_alive(e)
	new cls[16]
	pev(e, pev_classname, cls, charsmax(cls))
	if (g_lsgVolFilter[v] == 1)
		return equal(cls, "ls_creature") && pev(e, pev_deadflag) == DEAD_NO
	if (g_lsgVolFilter[v] == 2)
		return !equal(cls, "lsg_volume") && pev(e, pev_solid) != SOLID_NOT
	return false
}

stock bool:lsg_vol_inside(v, e)
{
	new Float:p[3]
	lsg_ent_origin(e, p)
	if (g_lsgVolShape[v] == 1)
		return get_distance_f(p, g_lsgVolPos[v]) <= g_lsgVolSize[v][0]
	// (a box, turned: the point in the box's own axes)
	new Float:d[3], Float:ax[3][3]
	for (new k = 0; k < 3; k++)
		d[k] = p[k] - g_lsgVolPos[v][k]
	engfunc(EngFunc_MakeVectors, g_lsgVolTurn[v])
	global_get(glb_v_forward, ax[0])
	global_get(glb_v_right, ax[1])
	global_get(glb_v_up, ax[2])
	for (new k = 0; k < 3; k++)
		if (floatabs(d[0] * ax[k][0] + d[1] * ax[k][1] + d[2] * ax[k][2]) > g_lsgVolSize[v][k] * 0.5)
			return false
	return true
}

stock lsg_vol_has(vol, e)
{
	new v = lsg_vol_slot(vol)
	return v >= 0 && lsg_vol_counts(v, e) && lsg_vol_inside(v, e) ? 1 : 0
}

stock lsg_vol_remove(vol)
{
	new v = lsg_vol_slot(vol)
	if (v < 0)
		return 0
	// (whoever was inside leaves it)
	new n = g_lsgVolInN[v]
	g_lsgVolInN[v] = 0
	for (new i = 0; i < n; i++)
		lsg_vol_end_hook(vol, g_lsgVolIn[v][i])
	g_lsgVol[v] = 0
	if (pev_valid(vol))
		engfunc(EngFunc_RemoveEntity, vol)
	return 1
}

public lsg_vol_tick()
{
	for (new v = 0; v < LSG_VOLS; v++)
	{
		new vol = g_lsgVol[v]
		if (!vol)
			continue
		if (!pev_valid(vol))
		{
			g_lsgVol[v] = 0
			continue
		}
		// (riding along: where the entity is now; it's gone: the volume stays where it was)
		if (g_lsgVolAt[v])
		{
			if (pev_valid(g_lsgVolAt[v]))
			{
				lsg_vol_place(v)
				engfunc(EngFunc_SetOrigin, vol, g_lsgVolPos[v])
			}
			else
				g_lsgVolAt[v] = 0
		}
		new Float:reach = g_lsgVolShape[v] == 1 ? g_lsgVolSize[v][0] : floatsqroot(g_lsgVolSize[v][0] * g_lsgVolSize[v][0] + g_lsgVolSize[v][1] * g_lsgVolSize[v][1] + g_lsgVolSize[v][2] * g_lsgVolSize[v][2]) * 0.5
		new now[LSG_VOLIN], nn = 0, e = -1
		if (g_lsgVolFilter[v] == 0)
		{
			// players only (the usual): the players themselves, not a search through every entity
			// near it (P5's spawn flood: 256 volumes among piled-up props and projectiles)
			for (e = 1; e <= MaxClients && nn < g_lsgVolInCap[v]; e++)
				if (lsg_vol_counts(v, e) && lsg_vol_inside(v, e))
					now[nn++] = e
		}
		else
			while ((e = engfunc(EngFunc_FindEntityInSphere, e, g_lsgVolPos[v], reach)) > 0 && nn < g_lsgVolInCap[v])
				if (lsg_vol_counts(v, e) && lsg_vol_inside(v, e))
					now[nn++] = e
		// who left, then who came in
		new was[LSG_VOLIN], wn = g_lsgVolInN[v]
		for (new i = 0; i < wn; i++)
			was[i] = g_lsgVolIn[v][i]
		for (new i = 0; i < nn; i++)
			g_lsgVolIn[v][i] = now[i]
		g_lsgVolInN[v] = nn
		for (new i = 0; i < wn; i++)
		{
			new still = 0
			for (new j = 0; j < nn && !still; j++)
				if (now[j] == was[i])
					still = 1
			if (!still)
				lsg_vol_end_hook(vol, was[i])
		}
		for (new j = 0; j < nn; j++)
		{
			new had = 0
			for (new i = 0; i < wn && !had; i++)
				if (was[i] == now[j])
					had = 1
			if (!had && g_lsgVol[v] == vol)
				lsg_vol_begin_hook(vol, now[j])
		}
	}
}

// ---- Bots Avoid Area: the tags out now, at most Areas bots avoid at once (Limits; the brain keeps 32)
stock bool:lsg_avoid_room(const tag[], cap)
{
	for (new i = 0; i < g_lsgAvoidN; i++)
		if (equal(g_lsgAvoidTag[i], tag))
			return true
	if (g_lsgAvoidN >= min(cap, sizeof g_lsgAvoidTag))
	{
		lsg_skip_log("avoid_areas", tag, "Bots Avoid Area %s skipped: as many are out as Areas bots avoid at once allows (%d, Limits)", tag, cap)
		return false
	}
	copy(g_lsgAvoidTag[g_lsgAvoidN++], 23, tag)
	return true
}

stock lsg_avoid_gone(const tag[])
{
	for (new i = 0; i < g_lsgAvoidN; i++)
		if (equal(g_lsgAvoidTag[i], tag))
		{
			copy(g_lsgAvoidTag[i], 23, g_lsgAvoidTag[--g_lsgAvoidN])
			return
		}
}

// ---- Bullet Hit: the shooter's own traces while their weapon's primary attack runs
public lsg_col_fire_pre(wpn)
{
	g_lsgFiring = pev_valid(wpn) ? pev(wpn, pev_owner) : 0
	if (!lsg_ok(g_lsgFiring))
		g_lsgFiring = 0
	return HAM_IGNORED
}

public lsg_col_fire_post(wpn)
{
	#pragma unused wpn
	g_lsgFiring = 0
	return HAM_IGNORED
}

stock lsg_col_hook_fire()
{
	static const W[][] = { "weapon_p228", "weapon_scout", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_p90", "weapon_knife" }
	for (new i = 0; i < sizeof W; i++)
	{
		RegisterHam(Ham_Weapon_PrimaryAttack, W[i], "lsg_col_fire_pre", 0)
		RegisterHam(Ham_Weapon_PrimaryAttack, W[i], "lsg_col_fire_post", 1)
	}
}

stock lsg_col_hitgroup(e, tr, out[], len)
{
	out[0] = EOS
	if (e < 1 || e > MaxClients)
		return
	switch (get_tr2(tr, TR_iHitgroup))
	{
		case HIT_HEAD: copy(out, len, "head")
		case HIT_CHEST: copy(out, len, "chest")
		case HIT_STOMACH: copy(out, len, "stomach")
		case HIT_LEFTARM: copy(out, len, "left arm")
		case HIT_RIGHTARM: copy(out, len, "right arm")
		case HIT_LEFTLEG: copy(out, len, "left leg")
		case HIT_RIGHTLEG: copy(out, len, "right leg")
		default: copy(out, len, "body")
	}
}

// ---- Player Landed: the game's fall damage for a fall speed (CS: from 580, 100 at 1024)
stock Float:lsg_fall_damage(Float:speed)
{
	return speed > 580.0 ? (speed - 580.0) * (100.0 / (1024.0 - 580.0)) : 0.0
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

public plugin_precache()
{
	lsg_pre_model("models/w_c4.mdl")
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every node 5 - collision and hits", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_touch("trigger_multiple", "player", "lsg_on_player_touched")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_pre_damage", 0)
	register_forward(FM_PlayerPreThink, "lsg_col_pre", 0)
	register_forward(FM_PlayerPostThink, "lsg_col_post", 0)
	register_forward(FM_AddToFullPack, "lsg_col_pack", 1)
	set_task(0.1, "lsg_vol_tick", LSG_TASK - 430, _, _, "b")
	register_forward(FM_Touch, "lsg_col_touch", 1)
	lsg_col_hook_fire()
	register_forward(FM_TraceLine, "lsg_col_trace", 1)
}

public plugin_end()
{
	lsd_end()
}

public lsg_pre_damage(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	// (saved and put back: damage done inside the flow comes back here)
	new Float:was = g_lsgDmg, bool:wasBlock = g_lsgDmgBlock
	g_lsgDmg = damage
	g_lsgDmgBlock = false
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	lsg_dmg_kind(bits, g_lsgTxt[0], 63) // kind
	ctx[3] = bits & DMG_BULLET && get_ent_data(victim, "CBaseMonster", "m_LastHitGroup") == HIT_HEAD ? 1 : 0 // headshot (W2: as Player Hurt)
	copy(g_lsgTxt[1], 63, lsg_hitgroup(victim, bits)) // hitgroup
	lsg_n27(ctx)
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

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
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
	if (lsg_named(ent, "zone"))
		lsg_n29(ctx)
	return PLUGIN_CONTINUE
}

stock Float:g_lsgTouch21[2048]
stock Float:g_lsgBullet23[33]
public lsg_col_pre(id)
{
	if (id < 1 || id > MaxClients || !is_user_alive(id))
		return FMRES_IGNORED
	// (falling: how fast, for the landing)
	g_lsgAirborne[id] = !(pev(id, pev_flags) & FL_ONGROUND)
	if (g_lsgAirborne[id])
		pev(id, pev_flFallVelocity, g_lsgAirVel[id])
	// (the players this one passes through: not solid while it moves)
	g_lsgSemiN[id] = 0
	if (g_lsgSemi)
		for (new p = 1; p <= MaxClients; p++)
			if (p != id && is_user_alive(p) && pev(p, pev_solid) == SOLID_SLIDEBOX && lsg_semi_pair(id, p))
			{
				set_pev(p, pev_solid, SOLID_NOT)
				g_lsgSemiOff[id][g_lsgSemiN[id]++] = p
			}
	return FMRES_IGNORED
}

public lsg_col_post(id)
{
	if (id < 1 || id > MaxClients)
		return FMRES_IGNORED
	for (new i = 0; i < g_lsgSemiN[id]; i++)
		if (is_user_alive(g_lsgSemiOff[id][i]))
			set_pev(g_lsgSemiOff[id][i], pev_solid, SOLID_SLIDEBOX)
	g_lsgSemiN[id] = 0
	// a landing: in the air when the frame began, on the ground now
	if (g_lsgAirborne[id] && is_user_alive(id) && (pev(id, pev_flags) & FL_ONGROUND))
	{
		g_lsgAirborne[id] = false
		new Float:speed = g_lsgAirVel[id]
		new ctx[LSG_CTX]
		ctx[0] = id // player
		ctx[1] = _:speed // speed
		ctx[2] = _:lsg_fall_damage(speed) // damage
		if (speed > 250.0)
			lsg_n25(ctx)
	}
	return FMRES_IGNORED
}

// (the semiclip in each player's own game: the others it passes through aren't solid to it)
public lsg_col_pack(es, e, ent, host, flags, player, pSet)
{
	#pragma unused e
	#pragma unused flags
	#pragma unused pSet
	if (player && g_lsgSemi && ent != host && host >= 1 && host <= MaxClients && ent >= 1 && ent <= MaxClients && is_user_alive(host) && is_user_alive(ent) && lsg_semi_pair(host, ent))
		set_es(es, ES_Solid, SOLID_NOT)
	return FMRES_IGNORED
}

lsg_vol_begin_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	if (lsg_vol_live(vol) && lsg_named(vol, "zone"))
		lsg_n17(ctx)
	// (Player Touched hears a volume like a map's trigger_multiple)
	if (who >= 1 && who <= MaxClients && lsg_vol_live(vol))
		lsg_on_player_touched(vol, who)
}

lsg_vol_end_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	if (lsg_vol_live(vol))
		lsg_n19(ctx)
}

public lsg_col_touch(ent, other)
{
	// (the world's walls and floors aren't "another entity")
	if (ent <= 0 || other <= 0 || ent >= 2048 || !pev_valid(ent) || !pev_valid(other))
		return FMRES_IGNORED
	new Float:now = get_gametime()
	new ctx[LSG_CTX]
	ctx[0] = ent // entity
	ctx[1] = other // other
	if (now - g_lsgTouch21[ent] >= 1.0)
	{
		g_lsgTouch21[ent] = now
		lsg_n21(ctx)
	}
	return FMRES_IGNORED
}

public lsg_col_trace(Float:v1[3], Float:v2[3], nomonsters, skip, tr)
{
	#pragma unused v1
	#pragma unused v2
	#pragma unused nomonsters
	new id = g_lsgFiring
	if (!id || skip != id)
		return FMRES_IGNORED
	new Float:now = get_gametime(), go = 0
	if (now - g_lsgBullet23[id] >= 0.2)
		go = 1
	if (!go)
		return FMRES_IGNORED
	new Float:end[3], Float:nrm[3], hit = get_tr2(tr, TR_pHit), wn[24], w = get_user_weapon(id)
	get_tr2(tr, TR_vecEndPos, end)
	get_tr2(tr, TR_vecPlaneNormal, nrm)
	if (hit < 0 || !pev_valid(hit))
		hit = 0
	if (w)
		get_weaponname(w, wn, charsmax(wn))
	replace(wn, charsmax(wn), "weapon_", "")
	new ctx[LSG_CTX]
	ctx[0] = id // shooter
	ctx[1] = lsg_vfrom(end) // pos
	ctx[2] = lsg_vfrom(nrm) // normal
	ctx[3] = hit // hit
	ctx[4] = hit ? 0 : 1 // world
	copy(g_lsgTxt[0], 63, wn) // weapon
	lsg_col_hitgroup(hit, tr, g_lsgTxt[1], 63) // hitgroup
	if (now - g_lsgBullet23[id] >= 0.2)
	{
		g_lsgBullet23[id] = now
		lsg_n23(ctx)
	}
	return FMRES_IGNORED
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

// Pick Random Player (pk)
lsg_n1(ctx[])
{
	if (lsd_want(1))
	{
	lsd_begin()
	}
	if (lsd_enter(1, ctx))
		return
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[8] = list[random(count)]
		lsd_exit(1, fmt("picked %d of %d", ctx[8], count))
		lsg_n2(ctx)
	}
	else
	{
		lsd_exit(1, "none")
	}
}

// Players Pass Through Each Other (semi)
lsg_n2(ctx[])
{
	if (lsd_want(2))
	{
	lsd_begin()
	}
	if (lsd_enter(2, ctx))
		return
	lsg_semiclip(2, 0, 1)
	lsd_exit(2, "set")
	lsg_n3(ctx)
}

// Players Pass Through Each Other (semi2)
lsg_n3(ctx[])
{
	new lsi0 = ctx[8]
	if (lsd_want(3))
	{
	lsd_begin()
	lsd_in_player("Player (chosen players)", lsi0)
	}
	if (lsd_enter(3, ctx))
		return
	lsg_semiclip(3, lsi0, 1)
	lsd_exit(3, "set")
	lsg_n4(ctx)
}

// Spawn Trigger Volume (vs)
lsg_n4(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 36.0)
	new lsi1 = lsg_vec(0.0, 45.0, 0.0)
	if (lsd_want(4))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	lsd_in_text("Rotation", lsg_vtext(lsi1))
	}
	if (lsd_enter(4, ctx))
		return
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsi0, lsgVp)
	lsg_vto(lsi1, lsgVa)
	g_lsgOut[4] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 0, 256.0, 256.0, 128.0, 0, "zone", 32, 32, 0)
	g_lsgO[0] = g_lsgOut[4]
	g_lsgO[1] = g_lsgOut[4] ? 1 : 0
	g_lsgO[2] = g_lsgVolWhy
	if (!g_lsgOut[4])
		lsd_error(4, g_lsgVolWhyName[clamp(g_lsgVolWhy, 0, 6)])
	else
		lsd_exit(4, fmt("entity %d", g_lsgOut[4]))
	lsg_n5(ctx)
}

// Spawn Trigger Volume (vs2)
lsg_n5(ctx[])
{
	new lsi2 = ctx[8]
	new lsi3 = lsg_vec(64.0, 0.0, 0.0)
	if (lsd_want(5))
	{
	lsd_begin()
	lsd_in_entity("At entity", lsi2)
	lsd_in_text("Local offset", lsg_vtext(lsi3))
	}
	if (lsd_enter(5, ctx))
		return
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsi3, lsgVo)
	g_lsgOut[5] = lsg_vol_spawn(lsi2, lsgVp, lsgVa, lsgVo, 1, 128.0, 0.0, 0.0, 2, "near", 32, 32, 1)
	g_lsgO[3] = g_lsgOut[5]
	g_lsgO[4] = g_lsgOut[5] ? 1 : 0
	g_lsgO[5] = g_lsgVolWhy
	if (!g_lsgOut[5])
		lsd_error(5, g_lsgVolWhyName[clamp(g_lsgVolWhy, 0, 6)])
	else
		lsd_exit(5, fmt("entity %d", g_lsgOut[5]))
	lsg_n6(ctx)
}

// Spawn Prop (prop)
lsg_n6(ctx[])
{
	new lsi0 = ctx[8]
	new Float:lsi6 = 1.0
	if (lsd_want(6))
	{
	lsd_begin()
	lsd_in_entity("At entity", lsi0)
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(6, ctx))
		return
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 6
	if (!lsgRe)
		g_lsgXrN[6] = 0
	g_lsgXrIn = -1
	if (!(lsg_have("models/w_c4.mdl")))
		lsgWhy = 4
	else if (!(lsg_ent_room("Spawn Prop")))
		lsgWhy = 2
	else if (!(lsg_ent_origin(lsi0, o)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, 1, 2, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_vis_note(lsg_prop_o(o, "models/w_c4.mdl", 1))))
		lsgWhy = 6
	new bool:lsgWait = false
	g_lsgOut[6] = e
	g_lsgO[6] = e
	g_lsgO[7] = e ? 1 : 0
	g_lsgO[8] = e ? lsgMv : 0
	g_lsgO[9] = e ? 0 : lsgWhy
	if (e)
	{
		if (lsgMv)
		lsd_exit(6, "moved to a nearby free spot")
		else
		lsd_exit(6, "spawned")
	}
	else if (lsgWait)
		lsd_exit(6, "blocked: trying again")
	else
	{
		switch (lsgWhy)
		{
			case 4: lsd_error(6, "No prop: its model isn't loaded with the map.")
			case 2: lsd_error(6, "No prop: the server is out of entity slots.")
			case 6: lsd_error(6, "No place: no such entity, or nothing wired into At.")
			case 1: lsd_error(6, "The spot was blocked (a player, a wall or something solid there) and no free spot was found within 128 units.")
			case 5: lsd_error(6, "The spot stayed blocked: every retry was used up.")
		}
	}
	if (!lsgRe)
		lsg_n7(ctx)
}

// Set Entity Collision (solid)
lsg_n7(ctx[])
{
	new lsi0 = g_lsgO[6]
	if (lsd_want(7))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(7, ctx))
		return
	g_lsgOut[7] = lsg_set_solid(lsi0, 1)
	if (!g_lsgOut[7])
		lsd_error(7, "Nothing done: no entity there, or it's a player (or the world).")
	else
		lsd_exit(7, "done")
	lsg_n8(ctx)
}

// Set Entity Size (size)
lsg_n8(ctx[])
{
	new lsi0 = g_lsgO[6]
	if (lsd_want(8))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(8, ctx))
		return
	g_lsgOut[8] = lsg_set_size(lsi0, -8.0, -8.0, 0.0, 8.0, 8.0, 16.0)
	if (!g_lsgOut[8])
		lsd_error(8, "Nothing done: no entity there, it's a player, or a min isn't below its max.")
	else
		lsd_exit(8, "done")
	lsg_n9(ctx)
}

// Entity Takes Damage (dmg)
lsg_n9(ctx[])
{
	new lsi0 = g_lsgO[6]
	if (lsd_want(9))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(9, ctx))
		return
	g_lsgOut[9] = lsg_takes_damage(lsi0, 1)
	if (!g_lsgOut[9])
		lsd_error(9, "Nothing done: no entity there, or it's a player (or the world).")
	else
		lsd_exit(9, "done")
	lsg_n10(ctx)
}

// Set Entity Health (hp)
lsg_n10(ctx[])
{
	new lsi0 = g_lsgO[6]
	new Float:lsi1 = 250.0
	if (lsd_want(10))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_number("Health", lsi1)
	}
	if (lsd_enter(10, ctx))
		return
	g_lsgOut[10] = lsg_set_ent_health(lsi0, lsi1)
	if (!g_lsgOut[10])
		lsd_error(10, "Nothing done: no entity there, or it's a player (or the world).")
	else
		lsd_exit(10, "done")
	lsg_n11(ctx)
}

// Print String (phpr)
lsg_n11(ctx[])
{
	new Float:lsi0 = lsg_ent_health(g_lsgO[6])
	if (lsd_want(11))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(11, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "prop health", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(11, esc)
	lsg_n12(ctx)
}

// Print String (povl)
lsg_n12(ctx[])
{
	new lsi0 = lsg_vol_has(g_lsgO[0], ctx[8])
	if (lsd_want(12))
	{
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	}
	if (lsd_enter(12, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "picked player in zone", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(12, esc)
	lsg_n13(ctx)
}

// Remove Trigger Volume (rm)
lsg_n13(ctx[])
{
	new lsi0 = g_lsgO[3]
	if (lsd_want(13))
	{
	lsd_begin()
	lsd_in_entity("Volume", lsi0)
	}
	if (lsd_enter(13, ctx))
		return
	if (!lsg_vol_remove(lsi0))
		lsd_error(13, "Nothing removed: that isn't a trigger volume.")
	else
		lsd_exit(13, "removed")
	lsg_n14(ctx)
}

// Bots Avoid Area (av)
lsg_n14(ctx[])
{
	new lsi1 = ctx[8]
	if (lsd_want(14))
	{
	lsd_begin()
	lsd_in_entity("At entity", lsi1)
	}
	if (lsd_enter(14, ctx))
		return
	if (!ls_components_running() || !lsg_avoid_room("carrier", 16))
		lsd_error(14, "Not done: gm_components isn't running on this server, or as many areas are avoided as Areas bots avoid at once allows (Limits).")
	else
	{
		ls_bots_avoid_ent("carrier", lsi1, 200.0, 7.0, 0.0, 1)
		lsd_exit(14, "done")
	}
	lsg_n15(ctx)
}

// Bots Avoid Area (av2)
lsg_n15(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 36.0)
	if (lsd_want(15))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	}
	if (lsd_enter(15, ctx))
		return
	new Float:lsgAv[3]
	lsg_vto(lsi0, lsgAv)
	if (!ls_components_running() || !lsg_avoid_room("spot", 16))
		lsd_error(15, "Not done: gm_components isn't running on this server, or as many areas are avoided as Areas bots avoid at once allows (Limits).")
	else
	{
		ls_bots_avoid("spot", lsgAv, 128.0, 3.0, 30.0, 0)
		lsd_exit(15, "done")
	}
	lsg_n16(ctx)
}

// Bots Stop Avoiding (avs)
lsg_n16(ctx[])
{
	if (lsd_want(16))
	{
	lsd_begin()
	}
	if (lsd_enter(16, ctx))
		return
	if (!ls_components_running())
		lsd_error(16, "Not done: gm_components isn't running on this server, or as many areas are avoided as Areas bots avoid at once allows (Limits).")
	else
	{
		lsg_avoid_gone("spot")
		ls_bots_avoid_stop("spot")
		lsd_exit(16, "done")
	}
}

// Begin Overlap (ob)
lsg_n17(ctx[])
{
	if (lsd_want(17))
	{
	lsd_begin()
	}
	if (lsd_enter(17, ctx))
		return
	lsd_exit(17, "ran")
	lsg_n18(ctx)
}

// Print String (pob)
lsg_n18(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(18))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(18, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "begin overlap", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(18, esc)
}

// End Overlap (oe)
lsg_n19(ctx[])
{
	if (lsd_want(19))
	{
	lsd_begin()
	}
	if (lsd_enter(19, ctx))
		return
	lsd_exit(19, "ran")
	lsg_n20(ctx)
}

// Print String (poe)
lsg_n20(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(20))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(20, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "end overlap", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(20, esc)
}

// Entity Touched (ot)
lsg_n21(ctx[])
{
	if (lsd_want(21))
	{
	lsd_begin()
	}
	if (lsd_enter(21, ctx))
		return
	lsd_exit(21, "ran")
	lsg_n22(ctx)
}

// Print String (pot)
lsg_n22(ctx[])
{
	new lsi0 = ctx[1]
	if (lsd_want(22))
	{
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	}
	if (lsd_enter(22, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "touched", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(22, esc)
}

// Bullet Hit (bh)
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

// Print String (pbh)
lsg_n24(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[1])
	if (lsd_want(24))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(24, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "bullet hit", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(24, esc)
}

// Player Landed (ld)
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

// Print String (pld)
lsg_n26(ctx[])
{
	new Float:lsi0 = Float:ctx[1]
	if (lsd_want(26))
	{
	lsd_begin()
	lsd_in_number("Value", lsi0)
	}
	if (lsd_enter(26, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "landed", lsg_ntext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(26, esc)
}

// Player About To Be Hurt (ah)
lsg_n27(ctx[])
{
	if (lsd_want(27))
	{
	lsd_begin()
	}
	if (lsd_enter(27, ctx))
		return
	lsd_exit(27, "called")
	lsg_n28(ctx)
}

// Print String (pah)
lsg_n28(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgTxt[1])
	if (lsd_want(28))
	{
	lsd_begin()
	lsd_in_text("Value", lsi0)
	}
	if (lsd_enter(28, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "about to be hurt", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(28, esc)
}

// Player Touched (pt)
lsg_n29(ctx[])
{
	if (lsd_want(29))
	{
	lsd_begin()
	}
	if (lsd_enter(29, ctx))
		return
	lsd_exit(29, "ran")
	lsg_n30(ctx)
}

// Print String (ppt)
lsg_n30(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(30))
	{
	lsd_begin()
	lsd_in_player("Value", lsi0)
	}
	if (lsd_enter(30, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "player touched", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(30, esc)
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
	}
}
