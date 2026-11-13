/*
	x  public plugin_init()  server_cmd(quit)   - made with Loner-Strike's Game Mode Studio (graph version 1).
	Generated from evil.graph.json: change the graph and compile it again; edits here are lost.
	This is a debug build (Playtest): it reports every node to the Studio.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <loner>

#define LSG_MODE "evil"
#define LSG_CTX 4

#define LSD_N 5
new const g_lsdIds[LSD_N][] = { "r", "m", "g", "e", "k" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false }
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

public plugin_init()
{
	ls_register_mode(LSG_MODE, "x  public plugin_init()  server_cmd(quit", "1.0", "Game Mode Studio")
	lsd_init()
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	lsg_n4(ctx)
}

// Round Started (r)
lsg_n0(const ctx[])
{
	lsd_begin()
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n1(ctx)
}

// Show Message (m)
lsg_n1(const ctx[])
{
	lsd_begin()
	if (lsd_enter(1, ctx))
		return
	ls_announce(0, 20.0, "%s", "hi); server_cmd(quit); #include <evil>")
	lsd_exit(1, "shown")
}

// Give Weapon (g)
lsg_n2(const ctx[])
{
	lsd_begin()
	lsd_in_player("Player", ctx[1])
	if (lsd_enter(2, ctx))
		return
	if (ls_give_weapon(ctx[1], "ak47", 0))
		lsd_exit(2, "given")
	else
		lsd_error(2, "Not given: that player is dead or already has one.")
	lsg_n3(ctx)
}

// End Round (e)
lsg_n3(const ctx[])
{
	lsd_begin()
	if (lsd_enter(3, ctx))
		return
	server_cmd("endround")
	lsd_exit(3, "draw")
}

// Player Killed (k)
lsg_n4(const ctx[])
{
	lsd_begin()
	if (lsd_enter(4, ctx))
		return
	lsd_exit(4, "ran")
	lsg_n2(ctx)
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
	}
}
