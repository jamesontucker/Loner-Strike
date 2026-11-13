/*
	Last Stand Lite - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from last_stand_lite.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>

#define LSG_MODE "last_stand_lite"
#define LSG_CTX 7
#define LSG_TXT 2
#define LSG_N 58
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock Float:g_v0 = 0.0 // Wave (number)

stock g_lsgM45[33][LSG_CTX]
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

public plugin_precache()
{
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
	ls_register_mode(LSG_MODE, "Last Stand Lite", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	set_task(0.25, "lsg_revive_tick", LSG_TASK - 1, _, _, "b")
	register_clcmd("say /shop", "lsg_chat44")
	register_clcmd("say !shop", "lsg_chat44")
	register_clcmd("say_team /shop", "lsg_chat44")
	register_clcmd("say_team !shop", "lsg_chat44")
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
	for (new p = 1; p <= MaxClients; p++)
		lsg_clear_down(p)
}

public lsg_chat44(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n44(ctx)
	return PLUGIN_CONTINUE
}

lsg_revived_hook(id, by)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = by // by
	lsg_n42(ctx)
}

lsg_down_out_hook(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	#pragma unused ctx
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
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n10(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	lsg_n33(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n2(ctx)
	return HAM_IGNORED
}

public lsg_on_map_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public ls_on_wave_start(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n17(ctx)
}

public ls_on_wave_cleared(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n21(ctx)
}

// Map Started (ms)
lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Load Map Pack (ms_pack)
lsg_n1(ctx[])
{
	if (!(!ls_components_running()))
	{
		new map[32]
		get_mapname(map, charsmax(map))
		if (!ls_comp_load_pack(fmt("gamemodes/components/%s.%s.ini", map, "laststand")))
			{}
	}
	#pragma unused ctx
}

// Player Spawned (sp)
lsg_n2(ctx[])
{
	lsg_n3(ctx)
}

// Is Team (sp_t)
lsg_n3(ctx[])
{
	new ok = ((lsg_team(ctx[0]) == 1) ? 1 : 0)
	if (ok)
		lsg_n4(ctx)
	else
		lsg_n7(ctx)
}

// Strip Weapons (sp_strip)
lsg_n4(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		ls_strip(ctx[0], true)
	}
	lsg_n5(ctx)
}

// Set Health (sp_hp)
lsg_n5(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		ls_set_health(ctx[0], max(floatround(150.0), 1))
	}
	lsg_n6(ctx)
}

// Set Glow (sp_glow)
lsg_n6(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		set_user_rendering(ctx[0], kRenderFxGlowShell, 0, 255, 0, kRenderNormal, 16)
	}
}

// Strip Weapons (sp_ctstrip)
lsg_n7(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		ls_strip(ctx[0], true)
	}
	lsg_n8(ctx)
}

// Give Weapon (sp_gun)
lsg_n8(ctx[])
{
	ls_give_weapon(ctx[0], "m4a1", 120)
	lsg_n9(ctx)
}

// Give Item (sp_arm)
lsg_n9(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		cs_set_user_armor(ctx[0], 100, CS_ARMOR_VESTHELM)
	}
}

// Round Started (rs)
lsg_n10(ctx[])
{
	lsg_n11(ctx)
}

// Set Variable (rs_w0)
lsg_n11(ctx[])
{
	g_v0 = 0.0
	lsg_n12(ctx)
}

// For Each Player (rs_kill)
lsg_n12(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 5))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[4] = p
		lsg_n13(c)
	}
	#pragma unused count
	lsg_n14(ctx)
}

// Kill (rs_k)
lsg_n13(ctx[])
{
	if (!(!ls_alive(ctx[4])))
	{
		user_kill(ctx[4], 1)
	}
}

// Show Message (rs_msg)
lsg_n14(ctx[])
{
	ls_announce(0, 4.0, "%s", "Hold the base! The first wave is coming.")
	lsg_n15(ctx)
}

// Countdown (rs_cd)
lsg_n15(ctx[])
{
	remove_task(LSG_TASK + 15)
	g_lsgLeft[15] = 15
	lsg_c15_show()
	set_task(1.0, "lsg_t15", LSG_TASK + 15, ctx, LSG_CTX)
}

lsg_c15_show()
	ls_hud(0, LsHud:0, 1.2, "%s %d", "First wave in", g_lsgLeft[15])

public lsg_t15(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (--g_lsgLeft[15] > 0)
	{
		lsg_c15_show()
		set_task(1.0, "lsg_t15", LSG_TASK + 15, data, LSG_CTX)
		return
	}
	lsg_n16(ctx)
}

// Start Wave (rs_go)
lsg_n16(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_wave_start(max(floatround(0.0), 0))
	}
	#pragma unused ctx
}

// Wave Started (ws)
lsg_n17(ctx[])
{
	lsg_n18(ctx)
}

// Set Variable (ws_set)
lsg_n18(ctx[])
{
	g_v0 = Float:ctx[0]
	lsg_n19(ctx)
}

// Show HUD Text (ws_hud)
lsg_n19(ctx[])
{
	ls_hud(0, LsHud:0, 6.0, "%s %d", "Wave", floatround(Float:ctx[0]))
	lsg_n20(ctx)
}

// Play Sound (ws_snd)
lsg_n20(ctx[])
{
	client_cmd(0, "spk %s", "radio/locknload")
	#pragma unused ctx
}

// Wave Cleared (wc)
lsg_n21(ctx[])
{
	lsg_n22(ctx)
}

// Branch (wc_br)
lsg_n22(ctx[])
{
	new ok = ((((((Float:ctx[0]) >= (5.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n23(ctx)
	else
		lsg_n25(ctx)
}

// Show Message (wc_won)
lsg_n23(ctx[])
{
	ls_announce(0, 5.0, "%s", "You held out! The base is safe.")
	lsg_n24(ctx)
}

// End Round (wc_end)
lsg_n24(ctx[])
{
	server_cmd("endround CT")
	#pragma unused ctx
}

// For Each Player (wc_dead)
lsg_n25(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 2))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[5] = p
		lsg_n26(c)
	}
	#pragma unused count
	lsg_n28(ctx)
}

// Is Team (wc_isct)
lsg_n26(ctx[])
{
	new ok = ((lsg_team(ctx[5]) == 2) ? 1 : 0)
	if (ok)
		lsg_n27(ctx)
}

// Respawn (wc_resp)
lsg_n27(ctx[])
{
	if (!(!lsg_ok(ctx[5]) || is_user_alive(ctx[5]) || lsg_team(ctx[5]) < 1 || lsg_team(ctx[5]) > 2))
	{
		ExecuteHamB(Ham_CS_RoundRespawn, ctx[5])
	}
}

// For Each Player (wc_pay)
lsg_n28(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 6))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[6] = p
		lsg_n29(c)
	}
	#pragma unused count
	lsg_n30(ctx)
}

// Give Money (wc_money)
lsg_n29(ctx[])
{
	if (!(!lsg_ok(ctx[6])))
	{
		lsg_add_money(ctx[6], floatround(1500.0))
	}
}

// Countdown (wc_cd)
lsg_n30(ctx[])
{
	remove_task(LSG_TASK + 30)
	g_lsgLeft[30] = 15
	lsg_c30_show()
	set_task(1.0, "lsg_t30", LSG_TASK + 30, ctx, LSG_CTX)
	lsg_n32(ctx)
}

lsg_c30_show()
	ls_hud(0, LsHud:0, 1.2, "%s %d", "Next wave in", g_lsgLeft[30])

public lsg_t30(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (--g_lsgLeft[30] > 0)
	{
		lsg_c30_show()
		set_task(1.0, "lsg_t30", LSG_TASK + 30, data, LSG_CTX)
		return
	}
	lsg_n31(ctx)
}

// Start Wave (wc_next)
lsg_n31(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_wave_start(max(floatround(0.0), 0))
	}
	#pragma unused ctx
}

// Chat Message (wc_tip)
lsg_n32(ctx[])
{
	ls_chat(0, "%s", "Wave cleared: +1500. Say !shop to buy.")
	#pragma unused ctx
}

// Player Killed (pk)
lsg_n33(ctx[])
{
	lsg_n34(ctx)
}

// Is Team (pk_ct)
lsg_n34(ctx[])
{
	new ok = ((lsg_team(ctx[0]) == 2) ? 1 : 0)
	if (ok)
		lsg_n35(ctx)
	else
		lsg_n39(ctx)
}

// Make Downed (pk_down)
lsg_n35(ctx[])
{
	lsg_make_down(ctx[0], 25.0, 3.0, 50)
	lsg_n36(ctx)
}

// Branch (pk_br)
lsg_n36(ctx[])
{
	new ok = ((((((float(ls_count_alive(CsTeams:(2)))) == (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n37(ctx)
}

// Show Message (pk_lost)
lsg_n37(ctx[])
{
	ls_announce(0, 5.0, "%s", "The base is overrun!")
	lsg_n38(ctx)
}

// End Round (pk_end)
lsg_n38(ctx[])
{
	server_cmd("endround T")
	#pragma unused ctx
}

// Is Team (pk_by)
lsg_n39(ctx[])
{
	new ok = ((lsg_team(ctx[1]) == 2) ? 1 : 0)
	if (ok)
		lsg_n40(ctx)
}

// Give Money (pk_pay)
lsg_n40(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_money(ctx[1], floatround(300.0))
	}
	lsg_n41(ctx)
}

// Add Score (pk_score)
lsg_n41(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_frags(ctx[1], floatround(1.0))
	}
}

// Player Revived (rv)
lsg_n42(ctx[])
{
	lsg_n43(ctx)
}

// Give Money (rv_pay)
lsg_n43(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_money(ctx[1], floatround(200.0))
	}
}

// Chat Command (sh)
lsg_n44(ctx[])
{
	lsg_n45(ctx)
}

// Menu (sh_menu)
lsg_n45(ctx[])
{
	if (!(!lsg_ok(ctx[0]) || is_user_bot(ctx[0])))
	{
		for (new k = 0; k < LSG_CTX; k++)
			g_lsgM45[ctx[0]][k] = ctx[k]
		new m = menu_create("Shop", "lsg_mh45")
		menu_additem(m, "AK-47 (2500)")
		menu_additem(m, "Kevlar and helmet (1000)")
		menu_additem(m, "HE grenade (300)")
		menu_display(ctx[0], m)
	}
}

// Menu (sh_menu): the choice
public lsg_mh45(id, menu, item)
{
	menu_destroy(menu)
	if (item < 0 || id < 1 || id > MaxClients)
		return PLUGIN_HANDLED
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = g_lsgM45[id][k]
	switch (item)
	{
		case 0:
		{
			lsg_n46(ctx)
		}
		case 1:
		{
			lsg_n50(ctx)
		}
		case 2:
		{
			lsg_n54(ctx)
		}
	}
	return PLUGIN_HANDLED
}

// Branch (sh_b1)
lsg_n46(ctx[])
{
	new ok = ((((((lsg_get_money(ctx[0])) >= (2500.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n47(ctx)
	else
		lsg_n49(ctx)
}

// Give Weapon (sh_g1)
lsg_n47(ctx[])
{
	ls_give_weapon(ctx[0], "ak47", 90)
	lsg_n48(ctx)
}

// Give Money (sh_p1)
lsg_n48(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		lsg_add_money(ctx[0], floatround(-2500.0))
	}
}

// Chat Message (sh_n1)
lsg_n49(ctx[])
{
	ls_chat(ctx[0], "%s", "Not enough money for that.")
}

// Branch (sh_b2)
lsg_n50(ctx[])
{
	new ok = ((((((lsg_get_money(ctx[0])) >= (1000.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n51(ctx)
	else
		lsg_n53(ctx)
}

// Give Item (sh_g2)
lsg_n51(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		cs_set_user_armor(ctx[0], 100, CS_ARMOR_VESTHELM)
	}
	lsg_n52(ctx)
}

// Give Money (sh_p2)
lsg_n52(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		lsg_add_money(ctx[0], floatround(-1000.0))
	}
}

// Chat Message (sh_n2)
lsg_n53(ctx[])
{
	ls_chat(ctx[0], "%s", "Not enough money for that.")
}

// Branch (sh_b3)
lsg_n54(ctx[])
{
	new ok = ((((((lsg_get_money(ctx[0])) >= (300.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n55(ctx)
	else
		lsg_n57(ctx)
}

// Give Weapon (sh_g3)
lsg_n55(ctx[])
{
	ls_give_weapon(ctx[0], "hegrenade", 0)
	lsg_n56(ctx)
}

// Give Money (sh_p3)
lsg_n56(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		lsg_add_money(ctx[0], floatround(-300.0))
	}
}

// Chat Message (sh_n3)
lsg_n57(ctx[])
{
	ls_chat(ctx[0], "%s", "Not enough money for that.")
}

