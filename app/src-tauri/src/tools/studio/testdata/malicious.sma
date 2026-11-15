/*
	x  public plugin_init()  server_cmd(quit)   - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from evil.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>

#define LSG_MODE "evil"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 8
#define LSG_TASK 7310000
#pragma dynamic 16384
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock g_v0[64] = "x); server_cmd(quit); " // evil_var  server_cmd(quit)  (text)

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

// ---- Status Icon, Objective Marker, Kill Feed Line ----
stock g_lsgIcon[33][16]
stock lsg_icon_send(to, status, const name[], r, g, b)
{
	if (to && !is_user_connected(to))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("StatusIcon")
	message_begin(to ? MSG_ONE : MSG_ALL, msg, _, to)
	write_byte(status)
	write_string(name)
	if (status)
	{
		write_byte(r)
		write_byte(g)
		write_byte(b)
	}
	message_end()
}

// to 0: everyone; death: noted, so it's taken away when they die
stock lsg_icon(to, status, const name[], r, g, b, death)
{
	if (to && !lsg_ok(to))
		return
	lsg_icon_send(to, status, name, r, g, b)
	if (!death)
		return
	for (new p = to ? to : 1; p <= (to ? to : MaxClients); p++)
		if (is_user_connected(p))
			copy(g_lsgIcon[p], 15, status ? name : "")
}

stock lsg_icon_clear(id)
{
	if (id < 1 || id > MaxClients || !g_lsgIcon[id][0])
		return
	lsg_icon_send(id, 0, g_lsgIcon[id], 0, 0, 0)
	g_lsgIcon[id][0] = 0
}

// markers: a sprite on an info_target, moved by one task to what it follows (no think each frame)
#define LSG_MKS 32
stock g_lsgMk[LSG_MKS], g_lsgMkF[LSG_MKS], Float:g_lsgMkH[LSG_MKS], g_lsgMkWho[2048], g_lsgMkFor[2048]

stock lsg_marker(const spr[], follow, const Float:at[3], hasAt, Float:h, Float:scale, r, g, b, pulse, who, forp)
{
	// (a sprite that wasn't there when the map loaded: no marker)
	if (!lsg_have(spr) || (follow <= 0 && !hasAt) || (follow > 0 && !pev_valid(follow)))
		return 0
	new slot = -1
	for (new k = 0; k < LSG_MKS; k++)
	{
		if (g_lsgMk[k] && !pev_valid(g_lsgMk[k]))
			g_lsgMk[k] = 0
		if (!g_lsgMk[k])
		{
			slot = k
			break
		}
	}
	if (slot < 0)
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_marker")
	engfunc(EngFunc_SetModel, e, spr)
	set_pev(e, pev_movetype, MOVETYPE_NOCLIP)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_rendermode, kRenderTransAdd)
	set_pev(e, pev_renderamt, 255.0)
	new Float:c[3]
	c[0] = float(r)
	c[1] = float(g)
	c[2] = float(b)
	set_pev(e, pev_rendercolor, c)
	set_pev(e, pev_scale, scale)
	if (pulse)
		set_pev(e, pev_renderfx, kRenderFxPulseSlow)
	new Float:o[3]
	if (follow > 0)
		pev(follow, pev_origin, o)
	else
		o = at
	o[2] += h
	engfunc(EngFunc_SetOrigin, e, o)
	g_lsgMk[slot] = e
	g_lsgMkF[slot] = follow
	g_lsgMkH[slot] = h
	g_lsgMkWho[e] = who
	g_lsgMkFor[e] = forp
	return e
}

stock lsg_mk_free(k)
{
	new e = g_lsgMk[k]
	g_lsgMk[k] = 0
	if (e > 0 && e < 2048)
	{
		g_lsgMkWho[e] = 0
		g_lsgMkFor[e] = 0
	}
	if (e > 0 && pev_valid(e))
		remove_entity(e)
}

stock lsg_mk_follow()
{
	for (new k = 0; k < LSG_MKS; k++)
	{
		new e = g_lsgMk[k], f = g_lsgMkF[k]
		if (!e)
			continue
		if (!pev_valid(e))
		{
			g_lsgMk[k] = 0
			continue
		}
		if (f <= 0)
			continue
		// what it was on is gone (or the player left): so is the marker
		if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
		{
			lsg_mk_free(k)
			continue
		}
		// (a dead player's marker waits, out of sight)
		new fx = pev(e, pev_effects)
		if (f <= MaxClients && !is_user_alive(f))
		{
			set_pev(e, pev_effects, fx | EF_NODRAW)
			continue
		}
		set_pev(e, pev_effects, fx & ~EF_NODRAW)
		new Float:o[3]
		pev(f, pev_origin, o)
		o[2] += g_lsgMkH[k]
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

// 0 or a marker: that one, or all of them
stock lsg_marker_remove(m)
{
	for (new k = 0; k < LSG_MKS; k++)
		if (g_lsgMk[k] && (!m || g_lsgMk[k] == m))
			lsg_mk_free(k)
}

stock bool:lsg_mk_sees(ent, host)
{
	new w = g_lsgMkWho[ent]
	if (w == 3)
		return host == g_lsgMkFor[ent]
	return host >= 1 && host <= MaxClients && get_user_team(host) == w
}

// a line in the kill feed, to each person's game (one at a time: plugins that count deaths from
// the game's own DeathMsg, the mode's Player Killed among them, don't see it as a death)
stock lsg_killfeed(killer, victim, hs, const w[])
{
	if (!lsg_ok(victim))
		return 0
	if (!lsg_ok(killer))
		killer = 0
	static msg
	if (!msg)
		msg = get_user_msgid("DeathMsg")
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		message_begin(MSG_ONE, msg, _, p)
		write_byte(killer)
		write_byte(victim)
		write_byte(hs ? 1 : 0)
		write_string(w)
		message_end()
	}
	return 1
}

// ---- fun: fire, trails, bursts, swaps, burying, the camera, a drunk view ----
stock g_lsgSprFire, g_lsgSprSmoke, g_lsgSprBlood, g_lsgSprSpray
stock Float:g_lsgBurnTill[33], Float:g_lsgBurnDps[33], Float:g_lsgBurnNext[33], g_lsgBurnBy[33], g_lsgBurnFx[33]
stock bool:g_lsgBuried[33], g_lsgCam[33], g_lsgCamKeep[33], Float:g_lsgDrunkTill[33], Float:g_lsgDrunk[33], g_lsgFunN
stock g_lsgTrail[32], g_lsgTrailC[32][5], bool:g_lsgTrailStill[32]

stock lsg_burn(id, by, Float:secs, Float:dps)
{
	if (!ls_alive(id))
		return 0
	new Float:now = get_gametime()
	g_lsgBurnTill[id] = now + secs
	g_lsgBurnDps[id] = dps
	g_lsgBurnBy[id] = lsg_ok(by) ? by : 0
	if (g_lsgBurnNext[id] < now)
		g_lsgBurnNext[id] = now + 1.0
	// flames on them: a sprite that rides along (the engine moves it: MOVETYPE_FOLLOW)
	if (g_lsgSprFire && !(g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id])))
	{
		new e = create_entity("info_target")
		if (e > 0)
		{
			set_pev(e, pev_classname, "ls_studio_fire")
			engfunc(EngFunc_SetModel, e, "sprites/fire.spr")
			set_pev(e, pev_movetype, MOVETYPE_FOLLOW)
			set_pev(e, pev_aiment, id)
			set_pev(e, pev_solid, SOLID_NOT)
			set_pev(e, pev_rendermode, kRenderTransAdd)
			set_pev(e, pev_renderamt, 255.0)
			g_lsgBurnFx[id] = e
		}
	}
	return 1
}

stock lsg_burn_off(id)
{
	if (id < 1 || id > 32)
		return
	g_lsgBurnTill[id] = 0.0
	if (g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id]))
		remove_entity(g_lsgBurnFx[id])
	g_lsgBurnFx[id] = 0
}

stock lsg_drunk(id, Float:secs, Float:strength)
{
	if (!ls_alive(id))
		return 0
	g_lsgDrunkTill[id] = get_gametime() + secs
	g_lsgDrunk[id] = strength
	return 1
}

stock lsg_drunk_off(id)
{
	if (g_lsgDrunkTill[id] > 0.0 && is_user_connected(id))
		set_pev(id, pev_punchangle, Float:{0.0, 0.0, 0.0})
	g_lsgDrunkTill[id] = 0.0
}

stock lsg_trail_send(k)
{
	new e = g_lsgTrail[k]
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_KILLBEAM)
	write_short(e)
	message_end()
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMFOLLOW)
	write_short(e)
	write_short(g_lsgSprBeam)
	write_byte(g_lsgTrailC[k][4])
	write_byte(g_lsgTrailC[k][3])
	write_byte(g_lsgTrailC[k][0])
	write_byte(g_lsgTrailC[k][1])
	write_byte(g_lsgTrailC[k][2])
	write_byte(200)
	message_end()
}

stock lsg_trail(e, on, r, g, b, width, life)
{
	if (e <= 0 || !pev_valid(e))
		return 0
	new k = -1
	for (new i = 0; i < 32; i++)
		if (g_lsgTrail[i] == e)
			k = i
	if (!on)
	{
		if (k >= 0)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_KILLBEAM)
			write_short(e)
			message_end()
			g_lsgTrail[k] = 0
		}
		return 1
	}
	for (new i = 0; i < 32 && k < 0; i++)
		if (!g_lsgTrail[i] || !pev_valid(g_lsgTrail[i]))
			k = i
	if (k < 0)
		return 0
	g_lsgTrail[k] = e
	g_lsgTrailC[k][0] = r
	g_lsgTrailC[k][1] = g
	g_lsgTrailC[k][2] = b
	g_lsgTrailC[k][3] = width
	g_lsgTrailC[k][4] = life
	g_lsgTrailStill[k] = false
	lsg_trail_send(k)
	return 1
}

// a trail dies with a teleport, a respawn or a long stop: sent again when they move once more
stock lsg_trail_check()
{
	for (new k = 0; k < 32; k++)
	{
		new e = g_lsgTrail[k]
		if (!e)
			continue
		if (!pev_valid(e) || (e <= MaxClients && !is_user_connected(e)))
		{
			g_lsgTrail[k] = 0
			continue
		}
		new Float:vel[3]
		pev(e, pev_velocity, vel)
		if (vector_length(vel) < 1.0)
			g_lsgTrailStill[k] = true
		else if (g_lsgTrailStill[k])
		{
			g_lsgTrailStill[k] = false
			lsg_trail_send(k)
		}
	}
}

stock lsg_fun_ticks()
{
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_lsgBurnTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgBurnTill[id] || pev(id, pev_waterlevel) >= 2)
				lsg_burn_off(id)
			else if (now >= g_lsgBurnNext[id])
			{
				g_lsgBurnNext[id] = now + 1.0
				new by = g_lsgBurnBy[id]
				if (by && !is_user_connected(by))
					by = 0
				if (g_lsgBurnDps[id] > 0.0)
					ExecuteHamB(Ham_TakeDamage, id, by ? by : 0, by ? by : 0, g_lsgBurnDps[id], DMG_BURN)
			}
		}
		if (g_lsgDrunkTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgDrunkTill[id])
				lsg_drunk_off(id)
			else
			{
				new Float:s = g_lsgDrunk[id], Float:pa[3]
				pa[0] = floatsin(now * 2.3, radian) * s * 2.0
				pa[1] = floatcos(now * 1.7, radian) * s * 2.0
				pa[2] = floatsin(now * 1.1, radian) * s
				set_pev(id, pev_punchangle, pa)
			}
		}
	}
	if (++g_lsgFunN >= 10)
	{
		g_lsgFunN = 0
		lsg_trail_check()
	}
}

// the hull a player has now fits at o (nothing solid there)
stock bool:lsg_fits(id, const Float:o[3])
{
	engfunc(EngFunc_TraceHull, o, o, IGNORE_MONSTERS, (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN, id, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid)
}

stock lsg_swap(a, b)
{
	if (!ls_alive(a) || !ls_alive(b) || a == b || g_lsgBuried[a] || g_lsgBuried[b])
		return 0
	new Float:oa[3], Float:ob[3], Float:aa[3], Float:ab[3], Float:va[3], Float:vb[3]
	pev(a, pev_origin, oa)
	pev(b, pev_origin, ob)
	if (!lsg_fits(a, ob) || !lsg_fits(b, oa))
		return 0
	pev(a, pev_v_angle, aa)
	pev(b, pev_v_angle, ab)
	pev(a, pev_velocity, va)
	pev(b, pev_velocity, vb)
	engfunc(EngFunc_SetOrigin, a, ob)
	engfunc(EngFunc_SetOrigin, b, oa)
	set_pev(a, pev_angles, ab)
	set_pev(a, pev_v_angle, ab)
	set_pev(a, pev_fixangle, 1)
	set_pev(b, pev_angles, aa)
	set_pev(b, pev_v_angle, aa)
	set_pev(b, pev_fixangle, 1)
	set_pev(a, pev_velocity, vb)
	set_pev(b, pev_velocity, va)
	return 1
}

// chest deep: 30 units down, and no walking (they can still turn and shoot)
stock lsg_bury(id)
{
	if (!ls_alive(id) || g_lsgBuried[id])
		return 0
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] -= 30.0
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_pev(id, pev_movetype, MOVETYPE_NONE)
	g_lsgBuried[id] = true
	return 1
}

stock lsg_unbury(id)
{
	if (!lsg_ok(id) || !g_lsgBuried[id])
		return 0
	g_lsgBuried[id] = false
	if (!is_user_alive(id))
		return 1
	set_pev(id, pev_movetype, MOVETYPE_WALK)
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] += 30.0
	// (onto the floor, never into a wall: a little higher until they fit)
	for (new i = 0; i < 8 && !lsg_fits(id, o); i++)
		o[2] += 8.0
	engfunc(EngFunc_SetOrigin, id, o)
	return 1
}

// view: 0 first person, 1 third person, 2 the other one; reset: back to first person at death/respawn
stock lsg_camera(id, view, reset)
{
	if (!lsg_ok(id))
		return 0
	new third = view == 2 ? (g_lsgCam[id] ? 0 : 1) : view
	set_view(id, third ? CAMERA_3RDPERSON : CAMERA_NONE)
	g_lsgCam[id] = third
	g_lsgCamKeep[id] = reset ? 0 : 1
	return 1
}

stock lsg_cam_reapply(id)
{
	if (lsg_ok(id) && g_lsgCam[id])
		set_view(id, CAMERA_3RDPERSON)
}

stock lsg_fun_died(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_burn_off(id)
	lsg_drunk_off(id)
	g_lsgBuried[id] = false
	if (g_lsgCam[id] && !g_lsgCamKeep[id])
	{
		set_view(id, CAMERA_NONE)
		g_lsgCam[id] = 0
	}
}

stock lsg_fun_spawned(id)
{
	if (id < 1 || id > MaxClients || !is_user_alive(id))
		return
	g_lsgBuried[id] = false
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			lsg_trail_send(k)
	if (g_lsgCam[id])
	{
		// (kept: the game may put the view back as they spawn, so it's set again a moment later)
		if (g_lsgCamKeep[id])
			set_task(0.2, "lsg_cam_again", LSG_TASK - 480 + id)
		else
		{
			set_view(id, CAMERA_NONE)
			g_lsgCam[id] = 0
		}
	}
}

stock lsg_fun_left(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_burn_off(id)
	g_lsgDrunkTill[id] = 0.0
	g_lsgBuried[id] = false
	g_lsgCam[id] = 0
	g_lsgCamKeep[id] = 0
	remove_task(LSG_TASK - 480 + id)
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			g_lsgTrail[k] = 0
}

// effect: 0 sparks, 1 smoke, 2 blood, 3 firework, 4 implosion, 5 tar, 6 teleport, 7 lava, 8 a colored flash
stock lsg_burst(kind, const Float:o[3], count, r, g, b)
{
	// (smoke and blood need their sprites: not there when the map loaded, no burst)
	if ((kind == 1 && !g_lsgSprSmoke) || (kind == 2 && (!g_lsgSprBlood || !g_lsgSprSpray)))
		return
	for (new n = 0; n < count; n++)
	{
		message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
		switch (kind)
		{
			case 1:
			{
				write_byte(TE_SMOKE)
				lsg_coords(o)
				write_short(g_lsgSprSmoke)
				write_byte(20)
				write_byte(12)
			}
			case 2:
			{
				write_byte(TE_BLOODSPRITE)
				lsg_coords(o)
				write_short(g_lsgSprSpray)
				write_short(g_lsgSprBlood)
				write_byte(247)
				write_byte(12)
			}
			case 3:
			{
				write_byte(TE_EXPLOSION2)
				lsg_coords(o)
				write_byte(random_num(0, 200))
				write_byte(16)
			}
			case 4:
			{
				write_byte(TE_IMPLOSION)
				lsg_coords(o)
				write_byte(128)
				write_byte(20)
				write_byte(5)
			}
			case 5:
			{
				write_byte(TE_TAREXPLOSION)
				lsg_coords(o)
			}
			case 6:
			{
				write_byte(TE_TELEPORT)
				lsg_coords(o)
			}
			case 7:
			{
				write_byte(TE_LAVASPLASH)
				lsg_coords(o)
			}
			case 8:
			{
				write_byte(TE_DLIGHT)
				lsg_coords(o)
				write_byte(30)
				write_byte(r)
				write_byte(g)
				write_byte(b)
				write_byte(8)
				write_byte(30)
			}
			default:
			{
				write_byte(TE_SPARKS)
				lsg_coords(o)
			}
		}
		message_end()
		// (a firework's light, in its color)
		if (kind == 3)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_DLIGHT)
			lsg_coords(o)
			write_byte(25)
			write_byte(r)
			write_byte(g)
			write_byte(b)
			write_byte(6)
			write_byte(20)
			message_end()
		}
	}
}

stock lsg_coords(const Float:o[3])
{
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
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


public plugin_init()
{
	ls_register_mode(LSG_MODE, "x  public plugin_init()  server_cmd(quit", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
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
	lsg_n4(ctx)
}

// Round Started (r)
lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Show Message (m)
lsg_n1(ctx[])
{
	ls_announce(0, 20.0, "%s", "hi); server_cmd(quit); #include <evil>")
	lsg_n5(ctx)
}

// Give Weapon (g)
lsg_n2(ctx[])
{
	ls_give_weapon(ctx[1], "ak47", 0)
	lsg_n3(ctx)
}

// End Round (e)
lsg_n3(ctx[])
{
	server_cmd("endround")
	#pragma unused ctx
}

// Player Killed (k)
lsg_n4(ctx[])
{
	lsg_n2(ctx)
}

// Teleport (t)
lsg_n5(ctx[])
{
	lsg_teleport(ctx[0], "aserver_cmdquit")
	lsg_n6(ctx)
}

// Set Variable (s)
lsg_n6(ctx[])
{
	copy(g_v0, 63, "y); server_cmd(quit); ")
	lsg_n7(ctx)
}

// Create Component (c)
lsg_n7(ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_origin_of("x", o))
		g_lsgOut[7] = ls_comp_create("ls_capture_point", o, "radius=1; server_cmdquit")
	#pragma unused ctx
}

