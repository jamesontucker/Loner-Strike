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
#include <celltrie>
#include <cellarray>

#define LSG_MODE "last_stand_lite"
#define LSG_CTX 9
#define LSG_TXT 2
#define LSG_N 61
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

// The mode's variables
stock Float:g_v0 = 0.0 // Wave (number)
stock g_v1 = 0 // Shop (Entity)
stock Float:g_v2 = 0.0 // Standing (number)

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


// ---- map markers ----
#define LSG_MK_MAX 128
new const g_lsgMkIds[][] = { "lst_shop" }
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
	#pragma unused k
	#pragma unused key
	#pragma unused t
}

public plugin_precache()
{
	// the shopkeeper of a shop the mode makes
	precache_model("models/barney.mdl")
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
	ls_register_mode(LSG_MODE, "Last Stand Lite", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	set_task(1.0, "lsg_on_map_started")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	register_clcmd("say /shop", "lsg_chat48")
	register_clcmd("say !shop", "lsg_chat48")
	register_clcmd("say_team /shop", "lsg_chat48")
	register_clcmd("say_team !shop", "lsg_chat48")
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
}

public lsg_chat48(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n48(ctx)
	return PLUGIN_CONTINUE
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
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

public ls_on_down(id, attacker)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = attacker // attacker
	lsg_n52(ctx)
	return PLUGIN_CONTINUE
}

public ls_on_revive(id, by)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = by >= 1 && by <= MaxClients ? by : 0 // by
	lsg_n50(ctx)
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
	lsg_n41(ctx)
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
		lsg_n38(ctx)
}

// Branch (pk_br)
lsg_n35(ctx[])
{
	new ok = ((((((float(ls_count_alive(CsTeams:(2)))) == (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n36(ctx)
}

// Show Message (pk_lost)
lsg_n36(ctx[])
{
	ls_announce(0, 5.0, "%s", "The base is overrun!")
	lsg_n37(ctx)
}

// End Round (pk_end)
lsg_n37(ctx[])
{
	server_cmd("endround T")
	#pragma unused ctx
}

// Is Team (pk_by)
lsg_n38(ctx[])
{
	new ok = ((lsg_team(ctx[1]) == 2) ? 1 : 0)
	if (ok)
		lsg_n39(ctx)
}

// Give Money (pk_pay)
lsg_n39(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_money(ctx[1], floatround(300.0))
	}
	lsg_n40(ctx)
}

// Add Score (pk_score)
lsg_n40(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_frags(ctx[1], floatround(1.0))
	}
}

// Knocked Down On (ms_kd)
lsg_n41(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_down_enable(true, 25.0, 2, true)
	}
	lsg_n42(ctx)
}

// Branch (ms_has)
lsg_n42(ctx[])
{
	new ok = (((((lsg_mk_get(0, floatround(1.0)) > 0 ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n43(ctx)
	else
		lsg_n45(ctx)
}

// Create Component (ms_shop)
lsg_n43(ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_ent_origin(lsg_mk_get(0, floatround(1.0)), o))
		g_lsgOut[43] = ls_comp_create("ls_shop", o, "items=AK-47|2500|ak47|rifle;Kevlar and helmet|1000|vesthelm|armor;HE grenade|300|hegrenade|grenade;Ammo|200|ammo|ammo upgrades=1 keeper=barney price_rise=0.05")
	lsg_n44(ctx)
}

// Set Variable (ms_keep)
lsg_n44(ctx[])
{
	g_v1 = g_lsgOut[43]
	#pragma unused ctx
}

// Pick Random Entity (ms_spawn)
lsg_n45(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_start")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[7] = list[random(count)]
		lsg_n46(ctx)
	}
}

// Create Component (ms_shop2)
lsg_n46(ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_ent_origin(ctx[7], o))
		g_lsgOut[46] = ls_comp_create("ls_shop", o, "items=AK-47|2500|ak47|rifle;Kevlar and helmet|1000|vesthelm|armor;HE grenade|300|hegrenade|grenade;Ammo|200|ammo|ammo upgrades=1 keeper=barney price_rise=0.05")
	lsg_n47(ctx)
}

// Set Variable (ms_keep2)
lsg_n47(ctx[])
{
	g_v1 = g_lsgOut[46]
	#pragma unused ctx
}

// Chat Command (sh)
lsg_n48(ctx[])
{
	lsg_n49(ctx)
}

// Open Shop Menu (sh_menu)
lsg_n49(ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_shop_menu(g_v1, ctx[0])
	}
}

// Got Up (up)
lsg_n50(ctx[])
{
	lsg_n51(ctx)
}

// Give Money (up_pay)
lsg_n51(ctx[])
{
	if (!(!lsg_ok(ctx[1])))
	{
		lsg_add_money(ctx[1], floatround(200.0))
	}
}

// Knocked Down (dn)
lsg_n52(ctx[])
{
	lsg_n53(ctx)
}

// Delay (dn_wait)
lsg_n53(ctx[])
{
	set_task(floatmax(0.1, 0.1), "lsg_t53", LSG_TASK + 53, ctx, LSG_CTX)
}

public lsg_t53(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n54(ctx)
}

// Set Variable (dn_zero)
lsg_n54(ctx[])
{
	g_v2 = 0.0
	lsg_n55(ctx)
}

// For Each Player (dn_each)
lsg_n55(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 6))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[8] = p
		lsg_n56(c)
	}
	#pragma unused count
	lsg_n58(ctx)
}

// Branch (dn_br)
lsg_n56(ctx[])
{
	new ok = (((((ls_components_running() && ls_is_down(ctx[8]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (!ok)
		lsg_n57(ctx)
}

// Add To Variable (dn_add)
lsg_n57(ctx[])
{
	g_v2 += 1.0
	#pragma unused ctx
}

// Branch (dn_br2)
lsg_n58(ctx[])
{
	new ok = ((((((g_v2) == (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n59(ctx)
}

// Show Message (dn_lost)
lsg_n59(ctx[])
{
	ls_announce(0, 5.0, "%s", "The base is overrun!")
	lsg_n60(ctx)
}

// End Round (dn_end)
lsg_n60(ctx[])
{
	server_cmd("endround T")
	#pragma unused ctx
}

