/*
	Every Node 2 - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from every_node_2.graph.json: change the graph and compile it again; edits here are lost.
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

#define LSG_MODE "every_node_2"
#define LSG_CTX 4
#define LSG_TXT 2
#define LSG_N 37
#define LSG_TASK 7310000
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
#define LSG_O 1
#define LSG_OT 1

#define LSD_N 37
new const g_lsdIds[LSD_N][] = { "ps", "tag", "fr", "ly", "rs", "ph", "nb", "ce", "goal", "off", "gc", "oc", "bset", "p_bhp", "brep", "p_bhp2", "loot", "p_loot", "p_rolled", "skip", "p_zst", "p_zn", "cs", "p_cs", "p_chp", "dz", "p_zst2", "drop", "ph2", "bd", "p_bd", "br", "p_br", "dl", "p_dl", "ck", "p_ck" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
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
// while the world is paused nothing else runs: what would (a loop's other rounds, what comes
// after the paused node, other events) waits here in order and runs after Resume, so a
// breakpoint inside a loop pauses that round and the loop carries on from there
#define LSD_Q 128
new g_lsdQ[LSD_Q][LSG_CTX + 1], g_lsdQn, g_lsdQins
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
	}
	fclose(f)
	delete_file(take)
	lsd_emitf("^"e^":^"commands^",^"resume^":%s", g_lsdBool[resume ? 1 : 0])
	if (resume)
		lsd_resume()
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

// counted now (the brain's ls_creature_alive is only brought up to date a moment later)
stock lsg_creatures_alive()
{
	new n, e = MaxClients, Float:hp
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		pev(e, pev_health, hp)
		if (pev(e, pev_deadflag) == DEAD_NO && hp > 0.0)
			n++
	}
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


lsd_graph_vars()
{
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

public plugin_precache()
{
	// creature types this mode spawns: loaded as the map starts (with what other plugins asked for)
	if (get_cvar_pointer("ls_creature_precache"))
		lsg_precache_types("zombie")
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
	ls_register_mode(LSG_MODE, "Every Node 2", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_on_creature_killed", 1)
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_pay_killer", 1)
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
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
	lsg_n4(ctx)
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

public ls_on_barricade_damaged(ent, attacker, hp)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // barricade
	ctx[1] = attacker // attacker
	ctx[2] = _:float(hp) // hp
	lsg_n29(ctx)
}

public ls_on_barricade_repaired(ent, id, hp)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // barricade
	ctx[1] = id >= 1 && id <= MaxClients ? id : 0 // player
	ctx[2] = _:float(hp) // hp
	lsg_n31(ctx)
}

public ls_on_drop(ent, stage, const Float:origin[3])
{
	#pragma unused origin
	if (stage != 1)
		return
	new ctx[LSG_CTX]
	ctx[0] = ent // drop
	lsg_n33(ctx)
}

public lsg_on_creature_killed(ent, killer)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // creature
	ctx[1] = killer >= 1 && killer <= MaxClients ? killer : 0 // killer
	pev(ent, pev_netname, g_lsgTxt[0], 63) // kind
	lsg_n35(ctx)
	return HAM_IGNORED
}

// Player Spawned (ps)
lsg_n0(ctx[])
{
	lsd_begin()
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n1(ctx)
}

// Bots: Tag (tag)
lsg_n1(ctx[])
{
	new lsi0 = ctx[0]
	lsd_begin()
	lsd_in_player("Player", lsi0)
	if (lsd_enter(1, ctx))
		return
	if (!ls_components_running())
		lsd_error(1, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("role %d %s", lsi0, "sq1")))
			lsd_error(1, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(1, "told")
	}
	lsg_n2(ctx)
}

// Bots: Friends (fr)
lsg_n2(ctx[])
{
	new lsi2 = 1
	lsd_begin()
	lsd_in_bool("Friends", lsi2)
	if (lsd_enter(2, ctx))
		return
	if (!ls_components_running())
		lsd_error(2, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("friend %s %s %d", lsg_who(4, 0, "sq1"), lsg_who(4, 0, "sq1"), ((lsi2) ? 1 : 0))))
			lsd_error(2, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(2, "told")
	}
	lsg_n3(ctx)
}

// Bots: Prefer Layer (ly)
lsg_n3(ctx[])
{
	new lsi0 = ctx[0]
	lsd_begin()
	lsd_in_player("Bot", lsi0)
	if (lsd_enter(3, ctx))
		return
	if (!ls_components_running())
		lsd_error(3, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("layer %s %s", lsg_who(3, lsi0, ""), "high")))
			lsd_error(3, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(3, "told")
	}
}

// Round Started (rs)
lsg_n4(ctx[])
{
	lsd_begin()
	if (lsd_enter(4, ctx))
		return
	lsd_exit(4, "ran")
	lsg_n5(ctx)
}

// Bots: Phase (ph)
lsg_n5(ctx[])
{
	lsd_begin()
	if (lsd_enter(5, ctx))
		return
	if (!ls_components_running())
		lsd_error(5, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("phase %s", "loot")))
			lsd_error(5, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(5, "told")
	}
	lsg_n6(ctx)
}

// Bots: Don't Buy (nb)
lsg_n6(ctx[])
{
	new lsi0 = 1
	lsd_begin()
	lsd_in_bool("Don't buy", lsi0)
	if (lsd_enter(6, ctx))
		return
	if (!ls_components_running())
		lsd_error(6, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("nobuy %d", ((lsi0) ? 1 : 0))))
			lsd_error(6, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(6, "told")
	}
	lsg_n7(ctx)
}

// Bots: Fight Creatures (ce)
lsg_n7(ctx[])
{
	new lsi0 = 1
	lsd_begin()
	lsd_in_bool("Fight them", lsi0)
	if (lsd_enter(7, ctx))
		return
	if (!ls_components_running())
		lsd_error(7, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("enemyclass ls_creature %d", ((lsi0) ? 1 : 0))))
			lsd_error(7, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(7, "told")
	}
	lsg_n8(ctx)
}

// Bots: Goal (goal)
lsg_n8(ctx[])
{
	new lsi1 = lsg_find("gate1")
	new Float:lsi2 = 40.0
	new Float:lsi3 = 30.0
	lsd_begin()
	lsd_in_entity("At", lsi1)
	lsd_in_number("Priority", lsi2)
	lsd_in_number("For (seconds)", lsi3)
	if (lsd_enter(8, ctx))
		return
	if (!ls_components_running())
		lsd_error(8, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("goal %s defend %s %d %d %d %d", lsg_who(1, 0, ""), lsg_ptext(lsi1), lsi1, clamp(floatround(lsi2), 0, 100), max(floatround(lsi3), 1), 128)))
			lsd_error(8, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(8, "told")
	}
	lsg_n9(ctx)
}

// Bots: Offer (off)
lsg_n9(ctx[])
{
	new lsi1 = lsg_find("gate1")
	new Float:lsi2 = 500.0
	lsd_begin()
	lsd_in_entity("At", lsi1)
	lsd_in_number("Price", lsi2)
	if (lsd_enter(9, ctx))
		return
	if (!ls_components_running())
		lsd_error(9, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("offer %s trap %s %d %d %d", lsg_who(0, 0, ""), lsg_ptext(lsi1), lsi1, max(floatround(lsi2), 0), 0)))
			lsd_error(9, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(9, "told")
	}
	lsg_n10(ctx)
}

// Bots: Clear Goals (gc)
lsg_n10(ctx[])
{
	lsd_begin()
	if (lsd_enter(10, ctx))
		return
	if (!ls_components_running())
		lsd_error(10, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("goal_clear %s %s", lsg_who(1, 0, ""), "defend")))
			lsd_error(10, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(10, "told")
	}
	lsg_n11(ctx)
}

// Bots: Clear Offer (oc)
lsg_n11(ctx[])
{
	new lsi1 = lsg_find("gate1")
	lsd_begin()
	lsd_in_entity("At", lsi1)
	if (lsd_enter(11, ctx))
		return
	if (!ls_components_running())
		lsd_error(11, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("offer_clear %s %d", lsg_who(0, 0, ""), lsi1)))
			lsd_error(11, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(11, "told")
	}
	lsg_n12(ctx)
}

// Set Barricade Health (bset)
lsg_n12(ctx[])
{
	new lsi0 = lsg_find("gate1")
	new Float:lsi1 = 50.0
	lsd_begin()
	lsd_in_entity("Barricade", lsi0)
	lsd_in_number("Health", lsi1)
	if (lsd_enter(12, ctx))
		return
	if (!ls_components_running())
		lsd_error(12, "gm_components isn't running on this server.")
	else
	{
		ls_barricade_set_hp(lsi0, max(floatround(lsi1), 0))
		lsd_exit(12, "done")
	}
	lsg_n13(ctx)
}

// Print String (p_bhp)
lsg_n13(ctx[])
{
	new Float:lsi0 = float(ls_components_running() ? ls_barricade_hp(lsg_find("gate1")) : 0)
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(13, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 barricade at:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(13, esc)
	lsg_n14(ctx)
}

// Repair Barricade (brep)
lsg_n14(ctx[])
{
	new lsi0 = lsg_find("gate1")
	lsd_begin()
	lsd_in_entity("Barricade", lsi0)
	if (lsd_enter(14, ctx))
		return
	if (!ls_components_running())
		lsd_error(14, "gm_components isn't running on this server.")
	else
	{
		ls_barricade_repair(lsi0, 0)
		lsd_exit(14, "done")
	}
	lsg_n15(ctx)
}

// Print String (p_bhp2)
lsg_n15(ctx[])
{
	new Float:lsi0 = float(ls_components_running() ? ls_barricade_hp(lsg_find("gate1")) : 0)
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(15, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 barricade mended:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(15, esc)
	lsg_n16(ctx)
}

// Roll Loot (loot)
lsg_n16(ctx[])
{
	new Float:lsi0 = 1.0
	lsd_begin()
	lsd_in_number("Tier", lsi0)
	if (lsd_enter(16, ctx))
		return
	new x[64], ok
	if (!!ls_components_running())
		ok = ls_loot_roll("default", x, charsmax(x), max(floatround(lsi0), 0))
	copy(g_lsgOT[0], 127, ok ? x : "")
	g_lsgO[0] = ok ? 1 : 0
	if (!ls_components_running())
		lsd_error(16, "gm_components isn't running on this server.")
	else
		lsd_exit(16, ok ? g_lsgOT[0] : "nothing")
	lsg_n17(ctx)
}

// Print String (p_loot)
lsg_n17(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, g_lsgOT[0])
	lsd_begin()
	lsd_in_text("Value", lsi0)
	if (lsd_enter(17, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "X2 loot:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(17, esc)
	lsg_n18(ctx)
}

// Print String (p_rolled)
lsg_n18(ctx[])
{
	new lsi0 = g_lsgO[0]
	lsd_begin()
	lsd_in_bool("Value", lsi0)
	if (lsd_enter(18, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "X2 rolled:", g_lsgYesNo[(lsi0) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(18, esc)
	lsg_n19(ctx)
}

// Next Zone Stage Now (skip)
lsg_n19(ctx[])
{
	new lsi0 = lsg_find("shr")
	lsd_begin()
	lsd_in_entity("Zone", lsi0)
	if (lsd_enter(19, ctx))
		return
	if (!ls_components_running())
		lsd_error(19, "gm_components isn't running on this server.")
	else
	{
		ls_zone_skip(lsi0)
		lsd_exit(19, "done")
	}
	lsg_n20(ctx)
}

// Print String (p_zst)
lsg_n20(ctx[])
{
	new Float:lsi0 = float(ls_components_running() ? ls_zone_stage(lsg_find("shr")) : 0)
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(20, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 zone stage:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(20, esc)
	lsg_n21(ctx)
}

// Print String (p_zn)
lsg_n21(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, fmt("X2 zone next: %s s, radius %s, closing %s", lsg_ntext(lsg_zone_next(lsg_find("shr"), 0)), lsg_ntext(lsg_zone_next(lsg_find("shr"), 1)), g_lsgYesNo[(_:lsg_zone_next(lsg_find("shr"), 2)) ? 1 : 0]))
	lsd_begin()
	lsd_in_text("Value", lsi0)
	if (lsd_enter(21, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(21, esc)
	lsg_n22(ctx)
}

// Spawn Creature (cs)
lsg_n22(ctx[])
{
	new Float:lsi1 = 300.0
	new Float:lsi2 = 150.0
	lsd_begin()
	lsd_in_number("Health", lsi1)
	lsd_in_number("Pay", lsi2)
	if (lsd_enter(22, ctx))
		return
	if (!get_cvar_pointer("ls_creature_ver"))
		lsd_error(22, "This server's bot brain has no creatures (ls_creature).")
	else if (!(g_lsgOut[22] = lsg_creature_x("zombie", "spot1", 2, lsi1, floatround(lsi2))))
		lsd_error(22, "Not spawned: no entity has that name, or that type isn't loaded on this map.")
	else
		lsd_exit(22, fmt("entity %d", g_lsgOut[22]))
	lsg_n23(ctx)
}

// Print String (p_cs)
lsg_n23(ctx[])
{
	new lsi0 = g_lsgOut[22]
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	if (lsd_enter(23, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 creature:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(23, esc)
	lsg_n24(ctx)
}

// Print String (p_chp)
lsg_n24(ctx[])
{
	new Float:lsi0 = lsg_ehp(g_lsgOut[22])
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(24, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 creature health:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(24, esc)
	lsg_n27(ctx)
}

// Delay (dz)
lsg_n25(ctx[])
{
	new Float:lsi0 = 2.0
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	if (lsd_enter(25, ctx))
		return
	set_task(floatmax(lsi0, 0.1), "lsg_t25", LSG_TASK + 25, ctx, LSG_CTX)
	lsd_exit(25, "waiting")
}

public lsg_t25(const data[], task)
{
	if (lsd_hold_timer())
	{
		set_task(0.25, "lsg_t25", LSG_TASK + 25, data, LSG_CTX)
		return
	}
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsd_exit(25, "fired")
	lsg_n26(ctx)
}

// Print String (p_zst2)
lsg_n26(ctx[])
{
	new Float:lsi0 = float(ls_components_running() ? ls_zone_stage(lsg_find("shr")) : 0)
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(26, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 zone stage 2 s later:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(26, esc)
}

// Call Supply Drop (drop)
lsg_n27(ctx[])
{
	lsd_begin()
	if (lsd_enter(27, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(27, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("spot1", o))
		lsd_error(27, "No entity has that name on this map.")
	else
	{
		ls_drop_call(o, "")
		lsd_exit(27, "called")
	}
	lsg_n28(ctx)
}

// Bots: Phase (ph2)
lsg_n28(ctx[])
{
	lsd_begin()
	if (lsd_enter(28, ctx))
		return
	if (!ls_components_running())
		lsd_error(28, "gm_components isn't running on this server.")
	else
	{
		if (!lsg_hint(fmt("phase %s", "fight")))
			lsd_error(28, "This server's bot brain doesn't take hints (ls_mode).")
		else
			lsd_exit(28, "told")
	}
	lsg_n25(ctx)
}

// Barricade Damaged (bd)
lsg_n29(ctx[])
{
	lsd_begin()
	if (lsd_enter(29, ctx))
		return
	lsd_exit(29, "ran")
	lsg_n30(ctx)
}

// Print String (p_bd)
lsg_n30(ctx[])
{
	new Float:lsi0 = Float:ctx[2]
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(30, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 barricade damaged, left:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(30, esc)
}

// Barricade Repaired (br)
lsg_n31(ctx[])
{
	lsd_begin()
	if (lsd_enter(31, ctx))
		return
	lsd_exit(31, "ran")
	lsg_n32(ctx)
}

// Print String (p_br)
lsg_n32(ctx[])
{
	new Float:lsi0 = Float:ctx[2]
	lsd_begin()
	lsd_in_number("Value", lsi0)
	if (lsd_enter(32, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 barricade repaired to:", floatround(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(32, esc)
}

// Supply Drop Landed (dl)
lsg_n33(ctx[])
{
	lsd_begin()
	if (lsd_enter(33, ctx))
		return
	lsd_exit(33, "ran")
	lsg_n34(ctx)
}

// Print String (p_dl)
lsg_n34(ctx[])
{
	new lsi0 = ctx[0]
	lsd_begin()
	lsd_in_entity("Value", lsi0)
	if (lsd_enter(34, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %d", "X2 drop landed:", lsi0)
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(34, esc)
}

// Creature Killed (ck)
lsg_n35(ctx[])
{
	lsd_begin()
	if (lsd_enter(35, ctx))
		return
	lsd_exit(35, "ran")
	lsg_n36(ctx)
}

// Print String (p_ck)
lsg_n36(ctx[])
{
	new lsi0 = ctx[1]
	lsd_begin()
	lsd_in_player("Value", lsi0)
	if (lsd_enter(36, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "X2 creature killed by:", lsg_name(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(36, esc)
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
	}
}
