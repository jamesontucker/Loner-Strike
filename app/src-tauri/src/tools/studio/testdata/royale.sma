/*
	Royale Lite - made with Loner-Strike's Game Mode Studio (graph version 3).
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
#define LSG_N 43
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock Float:g_v0 = 0.0 // Squads left (number)
stock g_v1 = 0 // Looting (bool)

stock Float:g_lsgDmg, bool:g_lsgDmgBlock
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
	ls_register_mode(LSG_MODE, "Royale Lite", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_pre_damage", 0)
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
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
	lsg_n40(ctx)
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
	lsg_n2(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	lsg_n10(ctx)
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
	lsg_n19(ctx)
}

public ls_on_loot(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // container
	ctx[1] = id // player
	lsg_n21(ctx)
}

public ls_on_lobby_end(squads)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(squads) // squads
	lsg_n31(ctx)
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
	lsg_n23(ctx)
}

// Round Started (rs)
lsg_n2(ctx[])
{
	lsg_n26(ctx)
}

// For Each Entity (rs_zone)
lsg_n3(ctx[])
{
	new c[LSG_CTX], count
	new p = -1
	while (count < 512 && (p = find_ent_by_class(p, "ls_zone_shrinker")) > 0)
	{
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[4] = p
		lsg_n4(c)
	}
	#pragma unused count
	lsg_n5(ctx)
}

// Start Component (rs_start)
lsg_n4(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_comp_start(ctx[4])
	}
}

// Show Message (rs_msg)
lsg_n5(ctx[])
{
	ls_announce(0, 5.0, "%s", "Loot up and stay in the zone. Last one standing wins!")
	lsg_n6(ctx)
}

// Set Variable (rs_alive)
lsg_n6(ctx[])
{
	g_v0 = 0.0
	lsg_n7(ctx)
}

// Repeat (rs_drops)
lsg_n7(ctx[])
{
	remove_task(LSG_TASK + 7)
	g_lsgLeft[7] = 4
	set_task(floatmax(60.0, 0.1), "lsg_t7", LSG_TASK + 7, ctx, LSG_CTX)
}

public lsg_t7(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (g_lsgLeft[7] > 0 && --g_lsgLeft[7] == 0)
		remove_task(LSG_TASK + 7)
	else
		set_task(floatmax(60.0, 0.1), "lsg_t7", LSG_TASK + 7, data, LSG_CTX)
	lsg_n8(ctx)
}

// Pick Random Entity (rs_pick)
lsg_n8(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "ls_respawn_point")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[5] = list[random(count)]
		lsg_n9(ctx)
	}
}

// Call Supply Drop (rs_drop)
lsg_n9(ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_ent_origin(ctx[5], o))
		ls_drop_call(o, "")
}

// Player Killed (pk)
lsg_n10(ctx[])
{
	lsg_n11(ctx)
}

// Set Variable (pk_set)
lsg_n11(ctx[])
{
	g_v0 = float(ls_components_running() ? ls_squads_alive() : 0)
	lsg_n12(ctx)
}

// Show HUD Text (pk_hud)
lsg_n12(ctx[])
{
	ls_hud(0, LsHud:2, 5.0, "%s %d", "Squads left:", floatround(float(ls_components_running() ? ls_squads_alive() : 0)))
	lsg_n13(ctx)
}

// Branch (pk_br)
lsg_n13(ctx[])
{
	new ok = ((((((float(ls_components_running() ? ls_squads_alive() : 0)) <= (1.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n14(ctx)
}

// Pick Random Player (pk_win)
lsg_n14(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[6] = list[random(count)]
		lsg_n15(ctx)
	}
	else
	{
		lsg_n18(ctx)
	}
}

// Show HUD Text (pk_name)
lsg_n15(ctx[])
{
	ls_hud(0, LsHud:3, 6.0, "%s %s", "Winner:", lsg_name(ctx[6]))
	lsg_n16(ctx)
}

// Add Score (pk_pts)
lsg_n16(ctx[])
{
	if (!(!lsg_ok(ctx[6])))
	{
		lsg_add_frags(ctx[6], floatround(5.0))
	}
	lsg_n17(ctx)
}

// End Round (pk_end)
lsg_n17(ctx[])
{
	lsg_end_round(lsg_team(ctx[6]))
}

// End Round (pk_draw)
lsg_n18(ctx[])
{
	server_cmd("endround")
	#pragma unused ctx
}

// Zone Hurt Player (zt)
lsg_n19(ctx[])
{
	lsg_n20(ctx)
}

// Show HUD Text (zt_hud)
lsg_n20(ctx[])
{
	ls_hud(ctx[1], LsHud:3, 1.0, "%s", "Get back in the zone!")
}

// Crate Opened (lo)
lsg_n21(ctx[])
{
	lsg_n22(ctx)
}

// Chat Message (lo_chat)
lsg_n22(ctx[])
{
	ls_chat(ctx[1], "%s", "Crate opened.")
}

// Match HUD (ms_hud)
lsg_n23(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_match_hud(true)
	}
	lsg_n24(ctx)
}

// Knocked Down On (ms_kd)
lsg_n24(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_down_enable(true, 30.0, 0, false)
	}
	lsg_n25(ctx)
}

// Squad Size (ms_size)
lsg_n25(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_squad_max(clamp(floatround(4.0), 1, 8))
	}
	#pragma unused ctx
}

// Bots: Don't Buy (rs_nobuy)
lsg_n26(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("nobuy %d", 1)))
			{}
	}
	lsg_n27(ctx)
}

// Bots: Phase (rs_loot)
lsg_n27(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("phase %s", "loot")))
			{}
	}
	lsg_n28(ctx)
}

// Set Variable (rs_looting)
lsg_n28(ctx[])
{
	g_v1 = 1
	lsg_n29(ctx)
}

// Match HUD Words (rs_text)
lsg_n29(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_match_hud_text("Squads forming: say slash squad")
	}
	lsg_n30(ctx)
}

// Squad Lobby (rs_lobby)
lsg_n30(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_squad_lobby(floatmax(15.0, 0.0))
	}
	lsg_n3(ctx)
}

// Lobby Ended (le)
lsg_n31(ctx[])
{
	lsg_n32(ctx)
}

// For Each Player (le_each)
lsg_n32(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 1))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[7] = p
		lsg_n33(c)
	}
	#pragma unused count
	lsg_n35(ctx)
}

// Strip Weapons (le_strip)
lsg_n33(ctx[])
{
	if (!(!ls_alive(ctx[7])))
	{
		ls_strip(ctx[7], true)
	}
	lsg_n34(ctx)
}

// Drop In (le_drop)
lsg_n34(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_drop_in(ctx[7])
	}
}

// Match HUD Words (le_text)
lsg_n35(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_match_hud_text("Loot up: the fight starts in 45 s")
	}
	lsg_n36(ctx)
}

// Delay (le_wait)
lsg_n36(ctx[])
{
	set_task(floatmax(45.0, 0.1), "lsg_t36", LSG_TASK + 36, ctx, LSG_CTX)
}

public lsg_t36(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n37(ctx)
}

// Bots: Phase (le_fight)
lsg_n37(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("phase %s", "fight")))
			{}
	}
	lsg_n38(ctx)
}

// Set Variable (le_fighting)
lsg_n38(ctx[])
{
	g_v1 = 0
	lsg_n39(ctx)
}

// Match HUD Words (le_text2)
lsg_n39(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_match_hud_text("")
	}
	#pragma unused ctx
}

// Player About To Be Hurt (ah)
lsg_n40(ctx[])
{
	lsg_n41(ctx)
}

// Branch (ah_br)
lsg_n41(ctx[])
{
	new ok = ((((g_v1) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n42(ctx)
}

// Block Damage (ah_block)
lsg_n42(ctx[])
{
	g_lsgDmgBlock = true
	#pragma unused ctx
}

