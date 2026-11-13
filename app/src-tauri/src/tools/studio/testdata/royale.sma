/*
	Royale Lite - made with Loner-Strike's Game Mode Studio (graph version 2).
	Generated from royale_lite.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>

#define LSG_MODE "royale_lite"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 26
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock Float:g_v0 = 0.0 // Players left (number)

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

stock lsg_bot_goto(bot, const name[])
{
	new Float:o[3]
	if (lsg_origin_of(name, o))
		ls_bot_goto(bot, o)
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
	return lsg_ent_origin(lsg_find(name), o)

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
	ls_register_mode(LSG_MODE, "Royale Lite", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n2(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	lsg_n13(ctx)
}

public lsg_on_map_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public ls_on_zone_tick(ent, id, Float:dmg)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // zone
	ctx[1] = id // player
	ctx[2] = _:dmg // damage
	lsg_n22(ctx)
}

public ls_on_loot(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // container
	ctx[1] = id // player
	lsg_n24(ctx)
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
		if (!ls_comp_load_pack(fmt("gamemodes/components/%s.%s.ini", map, "br")))
			{}
	}
	#pragma unused ctx
}

// Round Started (rs)
lsg_n2(ctx[])
{
	lsg_n3(ctx)
}

// For Each Player (rs_each)
lsg_n3(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 1))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[4] = p
		lsg_n4(c)
	}
	#pragma unused count
	lsg_n6(ctx)
}

// Strip Weapons (rs_strip)
lsg_n4(ctx[])
{
	if (!(!ls_alive(ctx[4])))
	{
		ls_strip(ctx[4], true)
	}
	lsg_n5(ctx)
}

// Respawn At Point (rs_spot)
lsg_n5(ctx[])
{
	if (!(!ls_components_running() || !lsg_ok(ctx[4])))
	{
		ls_respawn_at(ctx[4], 0)
	}
}

// For Each Entity (rs_zone)
lsg_n6(ctx[])
{
	new c[LSG_CTX], count
	new p = -1
	while (count < 512 && (p = find_ent_by_class(p, "ls_zone_shrinker")) > 0)
	{
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[5] = p
		lsg_n7(c)
	}
	#pragma unused count
	lsg_n8(ctx)
}

// Start Component (rs_start)
lsg_n7(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_comp_start(ctx[5])
	}
}

// Show Message (rs_msg)
lsg_n8(ctx[])
{
	ls_announce(0, 5.0, "%s", "Loot up and stay in the zone. Last one standing wins!")
	lsg_n9(ctx)
}

// Set Variable (rs_alive)
lsg_n9(ctx[])
{
	g_v0 = 0.0
	lsg_n10(ctx)
}

// Repeat (rs_drops)
lsg_n10(ctx[])
{
	remove_task(LSG_TASK + 10)
	g_lsgLeft[10] = 4
	set_task(floatmax(60.0, 0.1), "lsg_t10", LSG_TASK + 10, ctx, LSG_CTX)
}

public lsg_t10(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (g_lsgLeft[10] > 0 && --g_lsgLeft[10] == 0)
		remove_task(LSG_TASK + 10)
	else
		set_task(floatmax(60.0, 0.1), "lsg_t10", LSG_TASK + 10, data, LSG_CTX)
	lsg_n11(ctx)
}

// Pick Random Entity (rs_pick)
lsg_n11(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "ls_respawn_point")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[6] = list[random(count)]
		lsg_n12(ctx)
	}
}

// Call Supply Drop (rs_drop)
lsg_n12(ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_ent_origin(ctx[6], o))
		ls_drop_call(o, "")
}

// Player Killed (pk)
lsg_n13(ctx[])
{
	lsg_n14(ctx)
}

// Set Variable (pk_set)
lsg_n14(ctx[])
{
	g_v0 = float(ls_count_alive(CsTeams:(0)))
	lsg_n15(ctx)
}

// Show HUD Text (pk_hud)
lsg_n15(ctx[])
{
	ls_hud(0, LsHud:2, 5.0, "%s %d", "Players left:", floatround(float(ls_count_alive(CsTeams:(0)))))
	lsg_n16(ctx)
}

// Branch (pk_br)
lsg_n16(ctx[])
{
	new ok = ((((((float(ls_count_alive(CsTeams:(0)))) <= (1.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n17(ctx)
}

// Pick Random Player (pk_win)
lsg_n17(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[7] = list[random(count)]
		lsg_n18(ctx)
	}
	else
	{
		lsg_n21(ctx)
	}
}

// Show HUD Text (pk_name)
lsg_n18(ctx[])
{
	ls_hud(0, LsHud:3, 6.0, "%s %s", "Winner:", lsg_name(ctx[7]))
	lsg_n19(ctx)
}

// Add Score (pk_pts)
lsg_n19(ctx[])
{
	if (!(!lsg_ok(ctx[7])))
	{
		lsg_add_frags(ctx[7], floatround(5.0))
	}
	lsg_n20(ctx)
}

// End Round (pk_end)
lsg_n20(ctx[])
{
	lsg_end_round(lsg_team(ctx[7]))
}

// End Round (pk_draw)
lsg_n21(ctx[])
{
	server_cmd("endround")
	#pragma unused ctx
}

// Zone Hurt Player (zt)
lsg_n22(ctx[])
{
	lsg_n23(ctx)
}

// Show HUD Text (zt_hud)
lsg_n23(ctx[])
{
	ls_hud(ctx[1], LsHud:3, 1.0, "%s", "Get back in the zone!")
}

// Crate Opened (lo)
lsg_n24(ctx[])
{
	lsg_n25(ctx)
}

// Chat Message (lo_chat)
lsg_n25(ctx[])
{
	ls_chat(ctx[1], "%s", "Crate opened.")
}

