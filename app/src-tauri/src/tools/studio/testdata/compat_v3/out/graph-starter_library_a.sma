/*
	Starter library test A - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from starter_test_a.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "starter_test_a"
#define LSG_CTX 37
#define LSG_TXT 2
#define LSG_N 181
#define LSG_TASK 7310000
#pragma dynamic 119936
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }
#define LSG_VR 256
#define LSG_VP 258
#define LSG_RR 64
#define LSG_RP 64
#define LSG_RC 1
#define LSG_RT 1
#define LSG_AC 65
#define LSG_AMAX 10000
#define LSG_TMPA 32
#define LSG_O 58
#define LSG_OT 1

// The mode's variables
stock g_pv0[33] // Double jumped (bool, per player)
stock Float:g_pv1[33] // Dash ready at (number, per player)
stock Float:g_pv2[33] // Last teleport (number, per player)
stock Float:g_pv3[33] // Ability ready at (number, per player)
stock g_v4 = 0 // Enemy near (bool)
stock g_v5 = 0 // Mines (Array of Entity)
stock g_v6 = 0 // Mine markers (Array of Entity)
stock g_v7 = 0 // Mine owners (Array of Player)
stock g_v8 = 0 // Mine damage (Array of Number)
stock g_v9 = 0 // Health Pack volumes (Array of Entity)
stock g_v10 = 0 // Health Pack spots (Array of Vector)
stock g_v11 = 0 // Health Pack markers (Array of Entity)
// Health Pack respawn spot is in slot (LSG_VR + 0)
stock g_v13 = 0 // Weapon Pickup volumes (Array of Entity)
stock g_v14 = 0 // Weapon Pickup spots (Array of Vector)
stock g_v15 = 0 // Weapon Pickup markers (Array of Entity)
// Weapon Pickup respawn spot is in slot (LSG_VR + 1)
stock g_v17 = 0 // Potato holder (Player)
stock Float:g_v18 = 0.0 // Potato left (number)


stock bool:lsg_proj_from(e, Float:f, Float:r, Float:u, rot, copy, Float:o[3], Float:d[3])
{
	if (e <= 0 || !pev_valid(e) || (e <= MaxClients && !is_user_alive(e)))
		return false
	new Float:a[3], Float:fw[3], Float:rt[3], Float:up[3]
	pev(e, pev_origin, o)
	if (e <= MaxClients)
	{
		new Float:vo[3]
		pev(e, pev_view_ofs, vo)
		for (new k = 0; k < 3; k++)
			o[k] += vo[k]
		pev(e, pev_v_angle, a)
	}
	else
		pev(e, pev_angles, a)
	// Local offset: along the entity's own forward, right and up
	angle_vector(a, ANGLEVECTOR_FORWARD, fw)
	angle_vector(a, ANGLEVECTOR_RIGHT, rt)
	angle_vector(a, ANGLEVECTOR_UP, up)
	for (new k = 0; k < 3; k++)
		o[k] += fw[k] * f + rt[k] * r + up[k] * u
	// which way: its facing (Face the way At entity faces) or none, turned by Rotation offset
	if (!copy)
		a[0] = a[1] = a[2] = 0.0
	if (rot >= 0)
		for (new k = 0; k < 3; k++)
			a[k] += lsg_vc(rot, k)
	angle_vector(a, ANGLEVECTOR_FORWARD, d)
	return true
}

// Spawn Projectile's spot and way: At position and Direction / Rotation win over the entity's
stock bool:lsg_proj_at(e, Float:f, Float:r, Float:u, rot, copy, pos, dir, ang, Float:o[3], Float:d[3])
{
	new bool:ok = lsg_proj_from(e, f, r, u, rot, copy, o, d)
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		ok = true
	}
	if (dir >= 0)
		lsg_vto(dir, d)
	else if (ang >= 0)
	{
		new Float:pa[3]
		lsg_vto(ang, pa)
		angle_vector(pa, ANGLEVECTOR_FORWARD, d)
	}
	return ok
}

#define LSG_PJ 64
stock g_lsgPjEnt[LSG_PJ], g_lsgPjNode[LSG_PJ], g_lsgPjCtx[LSG_PJ][LSG_CTX], g_lsgPjAt

stock lsg_pj_find(e)
{
	if (e <= 0)
		return -1
	for (new s = 0; s < LSG_PJ; s++)
		if (g_lsgPjEnt[s] == e)
			return s
	return -1
}

stock lsg_pj_note(e, node, const ctx[])
{
	new s = lsg_pj_find(e)
	if (s < 0)
	{
		for (new k = 0; k < LSG_PJ && s < 0; k++)
			if (!g_lsgPjEnt[k] || !pev_valid(g_lsgPjEnt[k]))
				s = k
		if (s < 0)
		{
			s = g_lsgPjAt
			g_lsgPjAt = (g_lsgPjAt + 1) % LSG_PJ
		}
	}
	g_lsgPjEnt[s] = e
	g_lsgPjNode[s] = node
	for (new k = 0; k < LSG_CTX; k++)
		g_lsgPjCtx[s][k] = ctx[k]
}

public ls_on_proj_hit_node(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	new s = lsg_pj_find(proj)
	if (s < 0)
		return
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = g_lsgPjCtx[s][k]
	switch (g_lsgPjNode[s])
	{
		case 110:
			lsg_pjhit109(ctx, owner, pos, normal, hit, hitgroup)
	}
}

public ls_on_proj_expire(proj, owner)
{
	#pragma unused owner
	new s = lsg_pj_find(proj)
	if (s < 0)
		return
	new ctx[LSG_CTX], k = g_lsgPjNode[s]
	for (new c = 0; c < LSG_CTX; c++)
		ctx[c] = g_lsgPjCtx[s][c]
	g_lsgPjEnt[s] = 0
	switch (k)
	{
		case 110:
			lsg_pjexp109(ctx)
	}
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
// ---- Show HUD Text: the latest text per player and slot ----
stock g_lsgHudTxt[33][4][128], Float:g_lsgHudSecs[33][4]
stock lsg_hud(who, slot, Float:secs, const text[])
{
	slot = clamp(slot, 0, 3)
	for (new p = who ? who : 1; p <= (who ? who : MaxClients); p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		copy(g_lsgHudTxt[p][slot], 127, text)
		g_lsgHudSecs[p][slot] = secs
		if (lsg_msg_state("Show HUD Text", "lsg_hud_again", p * 4 + slot, 1, p, 0, 32 + strlen(text)))
			lsg_hud_again(p * 4 + slot)
	}
}

public lsg_hud_again(key)
{
	new p = key / 4, slot = key % 4
	if (p >= 1 && p <= MaxClients && is_user_connected(p))
		ls_hud(p, LsHud:slot, g_lsgHudSecs[p][slot], "%s", g_lsgHudTxt[p][slot])
}

stock const g_lsgIconNames[][] = { "buyzone", "c4", "defuser", "rescue", "escape", "vipsafety", "stopwatch", "smallskull", "smallc4", "smallvip", "hostage", "dollar", "cross", "item_healthkit", "item_battery", "item_longjump", "dmg_poison", "dmg_bio", "dmg_chem", "dmg_drown", "dmg_gas", "dmg_cold", "dmg_heat", "dmg_rad", "dmg_shock" }
// ---- Status Icon, Objective Marker, Kill Feed Line ----
stock g_lsgIcon[33][16]
// (the last of each player's icons, for a send the budget put off: key = player * 64 + icon)
stock g_lsgIconSt[33][64], g_lsgIconRgb[33][64]
stock lsg_icon_send(to, status, const name[], r, g, b)
{
	if (to && !is_user_connected(to))
		return
	new k = lsg_icon_k(name)
	if (k >= 0)
	{
		g_lsgIconSt[to][k] = status
		g_lsgIconRgb[to][k] = (r << 16) | (g << 8) | b
		if (!lsg_msg_state("Status Icon", "lsg_icon_again", to * 64 + k, 1, to))
			return
	}
	lsg_icon_put(to, status, name, r, g, b)
}

public lsg_icon_again(key)
{
	new to = key / 64, k = key % 64, c = g_lsgIconRgb[to][k]
	if (!to || is_user_connected(to))
		lsg_icon_put(to, g_lsgIconSt[to][k], g_lsgIconNames[k], (c >> 16) & 255, (c >> 8) & 255, c & 255)
}

stock lsg_icon_k(const name[])
{
	for (new k = 0; k < sizeof g_lsgIconNames; k++)
		if (equal(g_lsgIconNames[k], name))
			return k
	return -1
}

stock lsg_icon_put(to, status, const name[], r, g, b)
{
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
#define LSG_MKS 256
stock g_lsgMk[LSG_MKS], g_lsgMkF[LSG_MKS], Float:g_lsgMkH[LSG_MKS], g_lsgMkWho[2048], g_lsgMkFor[2048]

stock lsg_marker(const spr[], follow, const Float:at[3], hasAt, Float:h, Float:scale, r, g, b, pulse, who, forp, lim = 32)
{
	// (a sprite that wasn't there when the map loaded: no marker)
	if (!lsg_have(spr) || (follow <= 0 && !hasAt) || (follow > 0 && !pev_valid(follow)))
		return 0
	new slot = -1
	for (new k = 0; k < min(lim, LSG_MKS); k++)
	{
		if (g_lsgMk[k] && !pev_valid(g_lsgMk[k]))
			g_lsgMk[k] = 0
		if (!g_lsgMk[k])
		{
			slot = k
			break
		}
	}
	if (slot < 0 || !lsg_ent_room("Objective Marker"))
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
	g_lsgMk[slot] = lsg_vis_note(e)
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
		if (!is_user_connected(p) || is_user_bot(p) || !lsg_msg_room("Kill Feed Line", 1, p))
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
// Function: Make Jump Pad
stock bool:g_lsgFD0
stock lsg_fr0(a0)
{
	new ctx[LSG_CTX]
	ctx[8] = _:a0
	g_lsgFD0 = false
	lsg_n0(ctx)
}

// Function: Launch From Jump Pad
stock bool:g_lsgFD1
stock lsg_fr1(a0, Float:a1)
{
	new ctx[LSG_CTX]
	ctx[9] = _:a0
	ctx[10] = _:a1
	g_lsgFD1 = false
	lsg_n4(ctx)
}

// Function: Teleport Through
stock g_lsgR2_0
stock bool:g_lsgFD2
stock lsg_fr2(a0, a1, Float:a2)
{
	new ctx[LSG_CTX]
	ctx[11] = _:a0
	ctx[12] = _:a1
	ctx[13] = _:a2
	g_lsgR2_0 = 0
	g_lsgFD2 = false
	lsg_n9(ctx)
}

// Function: Bot Wants Ability
stock g_lsgR3_0
stock bool:g_lsgFD3
stock lsg_fr3(a0, Float:a1, Float:a2)
{
	new ctx[LSG_CTX]
	ctx[14] = _:a0
	ctx[15] = _:a1
	ctx[16] = _:a2
	g_lsgR3_0 = 0
	g_lsgFD3 = false
	lsg_n17(ctx)
}

// Function: Place Mine
stock g_lsgR4_0
stock bool:g_lsgFD4
stock lsg_fr4(a0, Float:a1, Float:a2)
{
	new ctx[LSG_CTX]
	ctx[18] = _:a0
	ctx[19] = _:a1
	ctx[20] = _:a2
	g_lsgR4_0 = 0
	g_lsgFD4 = false
	lsg_n23(ctx)
}

// Function: Mine Stepped On
stock bool:g_lsgFD5
stock lsg_fr5(a0, a1)
{
	new ctx[LSG_CTX]
	ctx[21] = _:a0
	ctx[22] = _:a1
	g_lsgFD5 = false
	lsg_n32(ctx)
}

// Function: Give Potato
stock bool:g_lsgFD6
stock lsg_fr6(a0, Float:a1)
{
	new ctx[LSG_CTX]
	ctx[23] = _:a0
	ctx[24] = _:a1
	g_lsgFD6 = false
	lsg_n44(ctx)
}

// Function: Mark Potato Holder
stock bool:g_lsgFD7
stock lsg_fr7(a0)
{
	new ctx[LSG_CTX]
	ctx[25] = _:a0
	g_lsgFD7 = false
	lsg_n49(ctx)
}

// Function: Pass Potato
stock g_lsgR8_0
stock bool:g_lsgFD8
stock lsg_fr8(a0, a1)
{
	new ctx[LSG_CTX]
	ctx[27] = _:a0
	ctx[28] = _:a1
	g_lsgR8_0 = 0
	g_lsgFD8 = false
	lsg_n58(ctx)
}

// Function: Potato Tick
stock g_lsgR9_0
stock bool:g_lsgFD9
stock lsg_fr9()
{
	new ctx[LSG_CTX]
	g_lsgR9_0 = 0
	g_lsgFD9 = false
	lsg_n65(ctx)
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

// ---- player input, cooldowns, pickups, weapon tuning, saving (P5) ----
stock bool:g_lsgP5Picking, Float:g_lsgP5TouchAt[33], g_lsgP5Armor[33]

stock bool:lsg_p5_holding(id, bits)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return false
	return (pev(id, pev_button) & bits) != 0
}

// Only this player: none wired in (0) means nobody, so nothing runs
stock bool:lsg_p5_only(id, who)
	return who > 0 && id == who

// the first button of these bits, by name
stock lsg_p5_btn_name(bits, out[], len)
{
	static const b[] = { 1, 2, 4, 8, 16, 32, 512, 1024, 2048, 4096, 8192 }
	static const names[][] = { "attack", "jump", "duck", "forward", "back", "use", "left", "right", "attack2", "walk", "reload" }
	for (new k = 0; k < sizeof b; k++)
	{
		if (bits & b[k])
		{
			copy(out, len, names[k])
			return
		}
	}
	out[0] = 0
}

// a command's arguments as plain text: control characters out (it's only ever text)
stock lsg_p5_plain(s[])
{
	for (new i = 0; s[i]; i++)
		if (s[i] < 32)
			s[i] = ' '
	trim(s)
}
// Objective Markers follow what they're on, ten times a second (one task for all of them)
public lsg_mk_tick()
	lsg_mk_follow()

public plugin_precache()
{
	// a projectile the mode fires: loaded with the map
	if (lsg_pre_model("sprites/fexplo.spr"))
		ls_proj_precache("sprites/fexplo.spr")
	g_lsgSprExp = precache_model("sprites/zerogxplode.spr")
	lsg_pre_model("sprites/arrow1.spr")
	lsg_pre_model("sprites/glow01.spr")
	lsg_pre_model("sprites/ledglow.spr")
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Starter library test A", "1.0", "Game Mode Studio")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_hurt_before", 0)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	set_task(0.1, "lsg_mk_tick", LSG_TASK - 400, _, _, "b")
	set_task(0.1, "lsg_vol_tick", LSG_TASK - 430, _, _, "b")
	register_forward(FM_PlayerPreThink, "lsg_p5_think", 0)
	g_v5 = _:ArrayCreate(LSG_AC)
	g_v6 = _:ArrayCreate(LSG_AC)
	g_v7 = _:ArrayCreate(LSG_AC)
	g_v8 = _:ArrayCreate(LSG_AC)
	g_v9 = _:ArrayCreate(LSG_AC)
	g_v10 = _:ArrayCreate(LSG_AC)
	g_v11 = _:ArrayCreate(LSG_AC)
	g_v13 = _:ArrayCreate(LSG_AC)
	g_v14 = _:ArrayCreate(LSG_AC)
	g_v15 = _:ArrayCreate(LSG_AC)
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
}

// Player Pressed Button: what changed since the last frame (it arrives with the player's ping)
public lsg_p5_think(id)
{
	if (!is_user_alive(id))
		return FMRES_IGNORED
	new now = pev(id, pev_button), was = pev(id, pev_oldbuttons)
	if (now == was)
		return FMRES_IGNORED
	new down = now & ~was, up = was & ~now, bot = is_user_bot(id)
	if ((down & 2))
	{
		new ctx[LSG_CTX]
		ctx[0] = id // player
		lsg_p5_btn_name(down & 2, g_lsgTxt[0], 63) // button
		lsg_n102(ctx)
	}
	if ((down & 8192))
	{
		new ctx[LSG_CTX]
		ctx[0] = id // player
		lsg_p5_btn_name(down & 8192, g_lsgTxt[0], 63) // button
		lsg_n103(ctx)
	}
	#pragma unused bot
	#pragma unused down
	#pragma unused up
	return FMRES_IGNORED
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv0[id] = 0
	g_pv1[id] = 0.0
	g_pv2[id] = 0.0
	g_pv3[id] = 0.0
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n76(ctx)
}

new Float:g_lsgHurtHp[33]

public lsg_hurt_before(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused attacker
	#pragma unused damage
	#pragma unused bits
	if (victim >= 1 && victim <= MaxClients)
		pev(victim, pev_health, g_lsgHurtHp[victim])
	return HAM_IGNORED
}

public lsg_on_player_hurt(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused bits
	if (victim < 1 || victim > MaxClients)
		return HAM_IGNORED
	new Float:hp
	pev(victim, pev_health, hp)
	damage = g_lsgHurtHp[victim] - hp
	if (damage <= 0.0)
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	ctx[3] = bits & DMG_BULLET && get_ent_data(victim, "CBaseMonster", "m_LastHitGroup") == HIT_HEAD ? 1 : 0 // headshot
	ctx[4] = _:hp // left
	copy(g_lsgTxt[0], 63, lsg_hitgroup(victim, bits)) // hitgroup
	copy(g_lsgTxt[1], 63, lsg_wpn_of(attacker, inflictor)) // weapon
	lsg_n100(ctx)
	return HAM_IGNORED
}

lsg_vol_begin_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	if (lsg_vol_live(vol) && lsg_named(vol, "heal"))
		lsg_n94(ctx)
	if (lsg_vol_live(vol) && lsg_named(vol, "weap"))
		lsg_n95(ctx)
	if (lsg_vol_live(vol) && lsg_named(vol, "jumppad"))
		lsg_n96(ctx)
	if (lsg_vol_live(vol) && lsg_named(vol, "mine"))
		lsg_n98(ctx)
	if (lsg_vol_live(vol) && lsg_named(vol, "tp"))
		lsg_n104(ctx)
}

lsg_vol_end_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	#pragma unused ctx
}

// Function Entry (jp_make_entry)
stock lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Spawn Trigger Volume (jp_make_vol)
stock lsg_n1(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsg_vent(ctx[8]), lsgVp)
	g_lsgOut[1] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 96.0, 0.0, 0.0, 0, "jumppad", 32, 32, 0)
	g_lsgO[0] = g_lsgOut[1]
	g_lsgO[1] = g_lsgOut[1] ? 1 : 0
	g_lsgO[2] = g_lsgVolWhy
	lsg_n2(ctx)
}

// Objective Marker (jp_make_mark)
stock lsg_n2(ctx[])
{
	new Float:lsgAt[3]
	g_lsgOut[2] = lsg_marker("sprites/arrow1.spr", g_lsgO[0], lsgAt, 0, 8.0, 0.4, 80, 200, 255, 1, 0, 0, 32)
	lsg_n3(ctx)
}

// Return Node (jp_make_ret)
stock lsg_n3(ctx[])
{
	g_lsgFD0 = true
	#pragma unused ctx
}

// Function Entry (jp_launch_entry)
stock lsg_n4(ctx[])
{
	lsg_n5(ctx)
}

// As Player (jp_launch_asp)
stock lsg_n5(ctx[])
{
	new lsgE = ctx[9]
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[3] = lsgE
		lsg_n6(ctx)
	}
	else
	{
		g_lsgO[3] = 0
		lsg_n8(ctx)
	}
}

// Launch Up (jp_launch_up)
stock lsg_n6(ctx[])
{
	if (!(!ls_alive(g_lsgO[3])))
	{
		lsg_launch(g_lsgO[3], (((((Float:ctx[10]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[10]) : (800.0)))
	}
	lsg_n7(ctx)
}

// Play Sound To Player (jp_launch_snd)
stock lsg_n7(ctx[])
{
	lsg_sound_to(g_lsgO[3], "items/gunpickup2.wav")
	lsg_n8(ctx)
}

// Return Node (jp_launch_ret)
stock lsg_n8(ctx[])
{
	g_lsgFD1 = true
	#pragma unused ctx
}

// Function Entry (tp_go_entry)
stock lsg_n9(ctx[])
{
	lsg_n10(ctx)
}

// As Player (tp_go_asp)
stock lsg_n10(ctx[])
{
	new lsgE = ctx[11]
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[4] = lsgE
		lsg_n11(ctx)
	}
	else
	{
		g_lsgO[4] = 0
		lsg_n16(ctx)
	}
}

// Branch (tp_go_br)
stock lsg_n11(ctx[])
{
	new ok = ((((((((get_gametime()) - (g_pv2[lsg_slot(g_lsgO[4])]))) >= ((((((Float:ctx[13]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[13]) : (2.0))) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n12(ctx)
	else
		lsg_n16(ctx)
}

// Teleport (tp_go_tp)
stock lsg_n12(ctx[])
{
	lsg_teleport_to(g_lsgO[4], ctx[12])
	lsg_n13(ctx)
}

// Set Variable (tp_go_st)
stock lsg_n13(ctx[])
{
	g_pv2[lsg_slot(g_lsgO[4])] = get_gametime()
	lsg_n14(ctx)
}

// Play Sound To Player (tp_go_snd)
stock lsg_n14(ctx[])
{
	lsg_sound_to(g_lsgO[4], "debris/beamstart2.wav")
	lsg_n15(ctx)
}

// Return Node (tp_go_ret)
stock lsg_n15(ctx[])
{
	g_lsgR2_0 = ((1.0) == (1.0) ? 1 : 0)
	g_lsgFD2 = true
	#pragma unused ctx
}

// Return Node (tp_go_ret2)
stock lsg_n16(ctx[])
{
	g_lsgR2_0 = ((0.0) == (1.0) ? 1 : 0)
	g_lsgFD2 = true
	#pragma unused ctx
}

// Function Entry (ab_bot_entry)
stock lsg_n17(ctx[])
{
	lsg_n18(ctx)
}

// Set Variable (ab_bot_s0)
stock lsg_n18(ctx[])
{
	g_v4 = 0
	lsg_n19(ctx)
}

// For Each Player (ab_bot_each)
stock lsg_n19(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 1))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[17] = p
		lsg_n20(c)
	}
	#pragma unused count
	lsg_n22(ctx)
}

// Branch (ab_bot_br2)
stock lsg_n20(ctx[])
{
	new ok = ((((((((lsg_team(ctx[14])) == (lsg_team(ctx[17])) ? 0 : 1)) && (((lsg_vdist(lsg_vplayer(ctx[14], 0), lsg_vplayer(ctx[17], 0))) < ((((((Float:ctx[16]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[16]) : (1500.0))) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n21(ctx)
}

// Set Variable (ab_bot_s1)
stock lsg_n21(ctx[])
{
	g_v4 = 1
	#pragma unused ctx
}

// Return Node (ab_bot_ret)
stock lsg_n22(ctx[])
{
	g_lsgR3_0 = ((((lsg_is_bot(ctx[14])) && (g_v4) ? 1 : 0)) && (((lsg_random(0.0, 100.0, 0)) < ((((((Float:ctx[15]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[15]) : (25.0))) ? 1 : 0)) ? 1 : 0)
	g_lsgFD3 = true
}

// Function Entry (mine_place_entry)
stock lsg_n23(ctx[])
{
	lsg_n24(ctx)
}

// Spawn Trigger Volume (mine_place_vol)
stock lsg_n24(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsg_vplayer(ctx[18], 0), lsgVp)
	g_lsgOut[24] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 96.0, 0.0, 0.0, 0, "mine", 32, 32, 0)
	g_lsgO[5] = g_lsgOut[24]
	g_lsgO[6] = g_lsgOut[24] ? 1 : 0
	g_lsgO[7] = g_lsgVolWhy
	lsg_n25(ctx)
}

// Objective Marker (mine_place_mark)
stock lsg_n25(ctx[])
{
	new Float:lsgAt[3]
	lsg_vto(lsg_vplayer(ctx[18], 0), lsgAt)
	g_lsgOut[25] = lsg_marker("sprites/ledglow.spr", 0, lsgAt, 1, 4.0, 0.3, 255, 40, 0, 1, 0, 0, 32)
	lsg_n26(ctx)
}

// Bots Avoid Area (mine_place_avoid)
stock lsg_n26(ctx[])
{
	if (!(!ls_components_running() || !lsg_avoid_room("mine", 16)))
	{
		ls_bots_avoid_ent("mine", g_lsgO[5], 120.0, 8.0, 0.0, 0)
	}
	lsg_n27(ctx)
}

// Add (mine_place_add0)
stock lsg_n27(ctx[])
{
	g_lsgO[8] = _:float(lsg_apushc(g_v5, g_lsgO[5]))
	lsg_n28(ctx)
}

// Add (mine_place_add1)
stock lsg_n28(ctx[])
{
	g_lsgO[9] = _:float(lsg_apushc(g_v6, g_lsgOut[25]))
	lsg_n29(ctx)
}

// Add (mine_place_add2)
stock lsg_n29(ctx[])
{
	g_lsgO[10] = _:float(lsg_apushc(g_v7, ctx[18]))
	lsg_n30(ctx)
}

// Add (mine_place_add3)
stock lsg_n30(ctx[])
{
	g_lsgO[11] = _:float(lsg_apushc(g_v8, _:((((((Float:ctx[20]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[20]) : (120.0)))))
	lsg_n31(ctx)
}

// Return Node (mine_place_ret)
stock lsg_n31(ctx[])
{
	g_lsgR4_0 = g_lsgO[5]
	g_lsgFD4 = true
	#pragma unused ctx
}

// Function Entry (mine_step_entry)
stock lsg_n32(ctx[])
{
	lsg_n33(ctx)
}

// Branch (mine_step_br0)
stock lsg_n33(ctx[])
{
	new ok = ((((((float(lsg_afindc(g_v5, ctx[21]))) >= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n34(ctx)
	else
		lsg_n43(ctx)
}

// As Player (mine_step_asp)
stock lsg_n34(ctx[])
{
	new lsgE = ctx[22]
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[12] = lsgE
		lsg_n35(ctx)
	}
	else
	{
		g_lsgO[12] = 0
		lsg_n36(ctx)
	}
}

// Branch (mine_step_br)
stock lsg_n35(ctx[])
{
	new ok = ((((((g_lsgO[12]) == (lsg_agetc(g_v7, float(lsg_afindc(g_v5, ctx[21])))) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n43(ctx)
	else
		lsg_n36(ctx)
}

// Explosion (mine_step_boom)
stock lsg_n36(ctx[])
{
	lsg_explode(lsg_vent(ctx[21]), Float:lsg_agetc(g_v8, float(lsg_afindc(g_v5, ctx[21]))), 200.0)
	lsg_n37(ctx)
}

// Remove Marker (mine_step_rmmark)
stock lsg_n37(ctx[])
{
	lsg_marker_remove(lsg_agetc(g_v6, float(lsg_afindc(g_v5, ctx[21]))))
	lsg_n38(ctx)
}

// Remove Trigger Volume (mine_step_rmvol)
stock lsg_n38(ctx[])
{
	lsg_vol_remove(ctx[21])
	lsg_n39(ctx)
}

// Remove Index (mine_step_rm0)
stock lsg_n39(ctx[])
{
	lsg_aremi(g_v5, float(lsg_afindc(g_v5, ctx[21])))
	lsg_n40(ctx)
}

// Remove Index (mine_step_rm1)
stock lsg_n40(ctx[])
{
	lsg_aremi(g_v6, float(lsg_afindc(g_v5, ctx[21])))
	lsg_n41(ctx)
}

// Remove Index (mine_step_rm2)
stock lsg_n41(ctx[])
{
	lsg_aremi(g_v7, float(lsg_afindc(g_v5, ctx[21])))
	lsg_n42(ctx)
}

// Remove Index (mine_step_rm3)
stock lsg_n42(ctx[])
{
	lsg_aremi(g_v8, float(lsg_afindc(g_v5, ctx[21])))
	lsg_n43(ctx)
}

// Return Node (mine_step_ret)
stock lsg_n43(ctx[])
{
	g_lsgFD5 = true
	#pragma unused ctx
}

// Function Entry (hp_give_entry)
stock lsg_n44(ctx[])
{
	lsg_n45(ctx)
}

// Set Variable (hp_give_hold)
stock lsg_n45(ctx[])
{
	g_v17 = ctx[23]
	lsg_n46(ctx)
}

// Set Variable (hp_give_left)
stock lsg_n46(ctx[])
{
	g_v18 = (((((Float:ctx[24]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[24]) : (20.0))
	lsg_n47(ctx)
}

// Call (hp_give_mark)
stock lsg_n47(ctx[])
{
	lsg_fr7(ctx[23])
	lsg_n48(ctx)
}

// Return Node (hp_give_ret)
stock lsg_n48(ctx[])
{
	g_lsgFD6 = true
	#pragma unused ctx
}

// Function Entry (hp_mark_entry)
stock lsg_n49(ctx[])
{
	lsg_n50(ctx)
}

// Set Glow (hp_mark_glow)
stock lsg_n50(ctx[])
{
	if (!(!lsg_ok(ctx[25])))
	{
		set_user_rendering(ctx[25], kRenderFxGlowShell, 255, 0, 0, kRenderNormal, 16)
	}
	lsg_n51(ctx)
}

// Show HUD Text (hp_mark_hud)
stock lsg_n51(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s has the hot potato!", lsg_name(ctx[25])))
	lsg_hud(0, 0, 3.0, lsgH)
	lsg_n52(ctx)
}

// Bots: Clear Goals (hp_mark_clear)
stock lsg_n52(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("goal_clear %s %s", lsg_who(0, 0, ""), "0")))
			{}
	}
	lsg_n53(ctx)
}

// Bots: Goal (hp_mark_hunt)
stock lsg_n53(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("goal %s defend %s %d %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(ctx[25]), ctx[25], clamp(floatround(70.0), 0, 100), max(floatround(3600.0), 1), 64, clamp(floatround(50.0), 0, 100))))
			{}
	}
	lsg_n54(ctx)
}

// Branch (hp_mark_brb)
stock lsg_n54(ctx[])
{
	new ok = ((((lsg_is_bot(ctx[25])) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n55(ctx)
	else
		lsg_n57(ctx)
}

// Pick Random Player (hp_mark_far)
stock lsg_n55(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[26] = list[random(count)]
		lsg_n56(ctx)
	}
	else
	{
		lsg_n57(ctx)
	}
}

// Bot Follow (hp_mark_follow)
stock lsg_n56(ctx[])
{
	if (!(!ls_alive(ctx[25]) || !is_user_bot(ctx[25])))
	{
		lsg_bot_follow(ctx[25], ctx[26], 6, clamp(floatround(20.0), 0, 100))
	}
	lsg_n57(ctx)
}

// Return Node (hp_mark_ret)
stock lsg_n57(ctx[])
{
	g_lsgFD7 = true
	#pragma unused ctx
}

// Function Entry (hp_pass_entry)
stock lsg_n58(ctx[])
{
	lsg_n59(ctx)
}

// Branch (hp_pass_br)
stock lsg_n59(ctx[])
{
	new ok = ((((((((ctx[27]) == (g_v17) ? 1 : 0)) && ((((ls_alive(ctx[27]) ? 1 : 0)) && ((ls_alive(ctx[28]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n60(ctx)
	else
		lsg_n64(ctx)
}

// Set Glow (hp_pass_off)
stock lsg_n60(ctx[])
{
	if (!(!lsg_ok(ctx[27])))
	{
		set_user_rendering(ctx[27])
	}
	lsg_n61(ctx)
}

// Set Variable (hp_pass_hold)
stock lsg_n61(ctx[])
{
	g_v17 = ctx[28]
	lsg_n62(ctx)
}

// Call (hp_pass_mark)
stock lsg_n62(ctx[])
{
	lsg_fr7(ctx[28])
	lsg_n63(ctx)
}

// Return Node (hp_pass_ret)
stock lsg_n63(ctx[])
{
	g_lsgR8_0 = ((((ctx[27]) == (g_v17) ? 1 : 0)) && ((((ls_alive(ctx[27]) ? 1 : 0)) && ((ls_alive(ctx[28]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	g_lsgFD8 = true
}

// Return Node (hp_pass_ret2)
stock lsg_n64(ctx[])
{
	g_lsgR8_0 = ((((ctx[27]) == (g_v17) ? 1 : 0)) && ((((ls_alive(ctx[27]) ? 1 : 0)) && ((ls_alive(ctx[28]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	g_lsgFD8 = true
}

// Function Entry (hp_tick_entry)
stock lsg_n65(ctx[])
{
	lsg_n66(ctx)
}

// Add To Variable (hp_tick_dec)
stock lsg_n66(ctx[])
{
	g_v18 += -1.0
	lsg_n67(ctx)
}

// Branch (hp_tick_br)
stock lsg_n67(ctx[])
{
	new ok = ((((((g_v18) <= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n68(ctx)
	else
		lsg_n74(ctx)
}

// Branch (hp_tick_br2)
stock lsg_n68(ctx[])
{
	new ok = (((((ls_alive(g_v17) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n69(ctx)
	else
		lsg_n73(ctx)
}

// Explosion (hp_tick_boom)
stock lsg_n69(ctx[])
{
	lsg_explode(lsg_vplayer(g_v17, 0), 500.0, 180.0)
	lsg_n70(ctx)
}

// Set Glow (hp_tick_off)
stock lsg_n70(ctx[])
{
	if (!(!lsg_ok(g_v17)))
	{
		set_user_rendering(g_v17)
	}
	lsg_n71(ctx)
}

// Bots: Clear Goals (hp_tick_clear)
stock lsg_n71(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("goal_clear %s %s", lsg_who(0, 0, ""), "0")))
			{}
	}
	lsg_n72(ctx)
}

// Return Node (hp_tick_ret)
stock lsg_n72(ctx[])
{
	g_lsgR9_0 = ((g_v18) <= (0.0) ? 1 : 0)
	g_lsgFD9 = true
	#pragma unused ctx
}

// Return Node (hp_tick_ret2)
stock lsg_n73(ctx[])
{
	g_lsgR9_0 = ((g_v18) <= (0.0) ? 1 : 0)
	g_lsgFD9 = true
	#pragma unused ctx
}

// Branch (hp_tick_br5)
stock lsg_n74(ctx[])
{
	new ok = ((((((g_v18) == (5.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n75(ctx)
	else
		lsg_n73(ctx)
}

// Bots Avoid Area (hp_tick_avoid)
stock lsg_n75(ctx[])
{
	if (!(!ls_components_running() || !lsg_avoid_room("potato", 16)))
	{
		ls_bots_avoid_ent("potato", g_v17, 260.0, 8.0, 5.0, 0)
	}
	lsg_n73(ctx)
}

// Round Started (ev1)
lsg_n76(ctx[])
{
	lsg_n77(ctx)
}

// Sequence (ev2)
lsg_n77(ctx[])
{
	lsg_n78(ctx)
	lsg_n88(ctx)
}

// Pick Random Entity (ev3)
lsg_n78(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[29] = list[random(count)]
		lsg_n157(ctx)
	}
}

// Pick Random Entity (ev5)
lsg_n79(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[30] = list[random(count)]
		lsg_n163(ctx)
	}
}

// Pick Random Entity (ev7)
lsg_n80(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[31] = list[random(count)]
		lsg_n81(ctx)
	}
}

// Call (ev8)
lsg_n81(ctx[])
{
	lsg_fr0(ctx[31])
	lsg_n82(ctx)
}

// Pick Random Entity (ev9)
lsg_n82(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_start")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[32] = list[random(count)]
		lsg_n83(ctx)
	}
}

// Spawn Trigger Volume (ev10)
lsg_n83(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsg_vent(ctx[32]), lsgVp)
	g_lsgOut[83] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 64.0, 0.0, 0.0, 0, "tp", 32, 32, 0)
	g_lsgO[13] = g_lsgOut[83]
	g_lsgO[14] = g_lsgOut[83] ? 1 : 0
	g_lsgO[15] = g_lsgVolWhy
	lsg_n84(ctx)
}

// Pick Random Player (ev12)
lsg_n84(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[33] = list[random(count)]
		lsg_n85(ctx)
	}
}

// Call (ev14)
lsg_n85(ctx[])
{
	lsg_fr6(ctx[33], 25.0)
	lsg_n86(ctx)
}

// Pick Random Player (ev15)
lsg_n86(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 7))
			list[count++] = p
	if (count > 0)
	{
		ctx[34] = list[random(count)]
		lsg_n87(ctx)
	}
}

// Call (ev18)
lsg_n87(ctx[])
{
	lsg_fr4(ctx[34], 0.0, 0.0)
	g_lsgO[16] = _:g_lsgR4_0
}

// Repeat (ev19)
lsg_n88(ctx[])
{
	remove_task(LSG_TASK + 88)
	g_lsgLeft[88] = 0
	set_task(floatmax(1.0, 0.1), "lsg_t88", LSG_TASK + 88, ctx, LSG_CTX)
}

public lsg_t88(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (g_lsgLeft[88] > 0 && --g_lsgLeft[88] == 0)
		remove_task(LSG_TASK + 88)
	else
		set_task(floatmax(1.0, 0.1), "lsg_t88", LSG_TASK + 88, data, LSG_CTX)
	lsg_n89(ctx)
}

// Sequence (ev20)
lsg_n89(ctx[])
{
	lsg_n90(ctx)
	lsg_n91(ctx)
}

// Call (ev21)
lsg_n90(ctx[])
{
	lsg_fr9()
	g_lsgO[17] = _:g_lsgR9_0
	#pragma unused ctx
}

// For Each Player (ev22)
lsg_n91(ctx[])
{
	new c[LSG_CTX], count
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_match(p, 7))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[35] = p
		lsg_n92(c)
	}
	#pragma unused count
}

// Call (ev25)
lsg_n92(ctx[])
{
	lsg_fr3(ctx[35], 0.0, 0.0)
	g_lsgO[18] = _:g_lsgR3_0
	lsg_n93(ctx)
}

// Branch (ev26)
lsg_n93(ctx[])
{
	new ok = ((((g_lsgO[18]) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n107(ctx)
	else
		lsg_n114(ctx)
}

// Begin Overlap (ev42)
lsg_n94(ctx[])
{
	lsg_n120(ctx)
}

// Begin Overlap (ev44)
lsg_n95(ctx[])
{
	lsg_n134(ctx)
}

// Begin Overlap (ev46)
lsg_n96(ctx[])
{
	lsg_n97(ctx)
}

// Call (ev47)
lsg_n97(ctx[])
{
	lsg_fr1(ctx[1], 0.0)
}

// Begin Overlap (ev48)
lsg_n98(ctx[])
{
	lsg_n99(ctx)
}

// Call (ev49)
lsg_n99(ctx[])
{
	lsg_fr5(ctx[0], ctx[1])
}

// Player Hurt (ev50)
lsg_n100(ctx[])
{
	lsg_n101(ctx)
}

// Call (ev51)
lsg_n101(ctx[])
{
	lsg_fr8(ctx[1], ctx[0])
	g_lsgO[19] = _:g_lsgR8_0
}

// Player Pressed Button (ev52)
lsg_n102(ctx[])
{
	lsg_n147(ctx)
}

// Player Pressed Button (ev55)
lsg_n103(ctx[])
{
	lsg_n153(ctx)
}

// Begin Overlap (ev59)
lsg_n104(ctx[])
{
	lsg_n105(ctx)
}

// Pick Random Entity (ev60)
lsg_n105(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[36] = list[random(count)]
		lsg_n106(ctx)
	}
}

// Call (ev62)
lsg_n106(ctx[])
{
	lsg_fr2(ctx[1], ctx[36], 0.0)
	g_lsgO[20] = _:g_lsgR2_0
}

// Branch (ev28)
lsg_n107(ctx[])
{
	new ok = ((((((((g_pv3[lsg_slot(ctx[35])]) - (get_gametime()))) <= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n108(ctx)
	else
		lsg_n110(ctx)
}

// Set Variable (ev28)
lsg_n108(ctx[])
{
	g_pv3[lsg_slot(ctx[35])] = ((get_gametime()) + ((((((6.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (6.0) : (10.0))))
	lsg_n109(ctx)
}

// Spawn Projectile (ev31)
lsg_n109(ctx[])
{
	new Float:d[3], lsgK[160]
	formatex(lsgK, charsmax(lsgK), "trail=%d spin=0 bounce=0 radius=150 falloff=1 explode=1 dangerous=1 damage=%.0f scale=%.2f", lsg_msg_room("Spawn Projectile", 1, 0, 48) ? 1 : 0, floatclamp((((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (60.0)), 0.0, 5000.0), floatclamp(0.6, 0.05, 4.0))
	if (get_cvar_num("ls_proj_max") != 64)
		set_cvar_num("ls_proj_max", 64)
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 109
	if (!lsgRe)
		g_lsgXrN[109] = 0
	g_lsgXrIn = -1
	if (!(ls_components_running()))
		lsgWhy = 6
	else if (!(lsg_ent_room("Spawn Projectile")))
		lsgWhy = 2
	else if (!(ls_proj_count() < 64))
		lsgWhy = 3
	else if (!(lsg_have("sprites/fexplo.spr")))
		lsgWhy = 4
	else if (!(lsg_proj_at(ctx[35], 16.0, 0.0, 0.0, -1, 1, lsg_vplayer(ctx[35], 1), lsg_vplayer(ctx[35], 2), -1, o, d)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, -1, 0, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_vis_note(ls_proj_spawn(ctx[35], "sprites/fexplo.spr", o, d, floatclamp((((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (1100.0)), 1.0, 5000.0), 0.0, 4.0, lsgK))))
		lsgWhy = 6
	if (e)
	{
		lsg_pj_note(e, 110, ctx)
	}
	new bool:lsgWait = false
	g_lsgOut[109] = e
	g_lsgO[21] = e
	g_lsgO[29] = e ? 1 : 0
	g_lsgO[30] = e ? lsgMv : 0
	g_lsgO[31] = e ? 0 : lsgWhy
	#pragma unused lsgWait
}

// Spawn Projectile (x4_fb_throw_proj): its hits and its end
stock lsg_pjhit109(ctx[], owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	#pragma unused owner
	#pragma unused pos
	#pragma unused normal
	#pragma unused hit
	#pragma unused hitgroup
	g_lsgO[22] = lsg_vfrom(pos)
	g_lsgO[23] = lsg_vfrom(normal)
	g_lsgO[24] = (hit >= 1 && hit <= MaxClients) ? hit : 0
	g_lsgO[25] = (hit > MaxClients) ? hit : 0
	g_lsgO[26] = hit == 0 ? 1 : 0
	g_lsgO[27] = _:float(hitgroup)
	g_lsgO[28] = owner
	#pragma unused ctx
}

stock lsg_pjexp109(ctx[])
{
	#pragma unused ctx
}

// Branch (ev34)
lsg_n110(ctx[])
{
	new ok = ((((((((g_pv1[lsg_slot(ctx[35])]) - (get_gametime()))) <= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n111(ctx)
}

// Set Velocity (ev34)
lsg_n111(ctx[])
{
	if (!(!ls_alive(ctx[35])))
	{
		lsg_vsetvel(ctx[35], lsg_vadd(lsg_vscale(lsg_vnorm(lsg_vec(lsg_vc(lsg_vplayer(ctx[35], 2), 0), lsg_vc(lsg_vplayer(ctx[35], 2), 1), 0.0)), (((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (700.0))), lsg_vec(0.0, 0.0, 120.0), 1.0))
	}
	lsg_n112(ctx)
}

// Set Variable (ev34)
lsg_n112(ctx[])
{
	g_pv1[lsg_slot(ctx[35])] = ((get_gametime()) + ((((((4.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (4.0) : (3.0))))
	lsg_n113(ctx)
}

// Play Sound To Player (ev34)
lsg_n113(ctx[])
{
	lsg_sound_to(ctx[35], "weapons/knife_slash1.wav")
}

// Branch (ev36)
lsg_n114(ctx[])
{
	new ok = (((((lsg_ok(ctx[35]) && (pev(ctx[35], pev_flags) & FL_ONGROUND) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n115(ctx)
	else
		lsg_n116(ctx)
}

// Set Variable (ev36)
lsg_n115(ctx[])
{
	g_pv0[lsg_slot(ctx[35])] = 0
}

// Branch (ev36)
lsg_n116(ctx[])
{
	new ok = ((((g_pv0[lsg_slot(ctx[35])]) ? 1 : 0)) ? 1 : 0)
	if (!ok)
		lsg_n117(ctx)
}

// Set Velocity (ev36)
lsg_n117(ctx[])
{
	if (!(!ls_alive(ctx[35])))
	{
		lsg_vsetvel(ctx[35], lsg_vec(lsg_vc(lsg_vpev(ctx[35], pev_velocity), 0), lsg_vc(lsg_vpev(ctx[35], pev_velocity), 1), (((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (320.0))))
	}
	lsg_n118(ctx)
}

// Set Variable (ev36)
lsg_n118(ctx[])
{
	g_pv0[lsg_slot(ctx[35])] = 1
	lsg_n119(ctx)
}

// Play Sound To Player (ev36)
lsg_n119(ctx[])
{
	lsg_sound_to(ctx[35], "player/pl_jump2.wav")
}

// Branch (ev43)
lsg_n120(ctx[])
{
	new ok = ((((((float(lsg_afindc(g_v9, ctx[0]))) >= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n121(ctx)
}

// As Player (ev43)
lsg_n121(ctx[])
{
	new lsgE = ctx[1]
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[32] = lsgE
		lsg_n122(ctx)
	}
	else
	{
		g_lsgO[32] = 0
	}
}

// Branch (ev43)
lsg_n122(ctx[])
{
	new ok = ((((((lsg_health(g_lsgO[32])) < (100.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n123(ctx)
}

// Set Health (ev43)
lsg_n123(ctx[])
{
	if (!(!ls_alive(g_lsgO[32])))
	{
		ls_set_health(g_lsgO[32], max(floatround(lsg_min(((lsg_health(g_lsgO[32])) + ((((((50.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (50.0) : (50.0)))), 100.0)), 1))
	}
	lsg_n124(ctx)
}

// Play Sound To Player (ev43)
lsg_n124(ctx[])
{
	lsg_sound_to(g_lsgO[32], "items/smallmedkit1.wav")
	lsg_n125(ctx)
}

// Bots: Clear Offer (ev43)
lsg_n125(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer_clear %s %d", lsg_who(0, 0, ""), ctx[0])))
			{}
	}
	lsg_n126(ctx)
}

// Remove Marker (ev43)
lsg_n126(ctx[])
{
	lsg_marker_remove(lsg_agetc(g_v11, float(lsg_afindc(g_v9, ctx[0]))))
	lsg_n127(ctx)
}

// Remove Trigger Volume (ev43)
lsg_n127(ctx[])
{
	lsg_vol_remove(ctx[0])
	lsg_n128(ctx)
}

// Set Variable (ev43)
lsg_n128(ctx[])
{
	lsg_vset((LSG_VR + 0), lsg_agetv(g_v10, float(lsg_afindc(g_v9, ctx[0]))))
	lsg_n129(ctx)
}

// Remove Index (ev43)
lsg_n129(ctx[])
{
	lsg_aremi(g_v9, float(lsg_afindc(g_v9, ctx[0])))
	lsg_n130(ctx)
}

// Remove Index (ev43)
lsg_n130(ctx[])
{
	lsg_aremi(g_v10, float(lsg_afindc(g_v9, ctx[0])))
	lsg_n131(ctx)
}

// Remove Index (ev43)
lsg_n131(ctx[])
{
	lsg_aremi(g_v11, float(lsg_afindc(g_v9, ctx[0])))
	lsg_n132(ctx)
}

// Sequence (ev43)
lsg_n132(ctx[])
{
	lsg_n133(ctx)
}

// Delay (ev43)
lsg_n133(ctx[])
{
	set_task(floatmax((((((10.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (10.0) : (20.0)), 0.1), "lsg_t133", LSG_TASK + 133, ctx, LSG_CTX)
}

public lsg_t133(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n169(ctx)
}

// Branch (ev45)
lsg_n134(ctx[])
{
	new ok = ((((((float(lsg_afindc(g_v13, ctx[0]))) >= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n135(ctx)
}

// As Player (ev45)
lsg_n135(ctx[])
{
	new lsgE = ctx[1]
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[33] = lsgE
		lsg_n136(ctx)
	}
	else
	{
		g_lsgO[33] = 0
	}
}

// Give Weapon (ev45)
lsg_n136(ctx[])
{
	lsg_give_named(g_lsgO[33], "awp", 90)
	lsg_n137(ctx)
}

// Play Sound To Player (ev45)
lsg_n137(ctx[])
{
	lsg_sound_to(g_lsgO[33], "items/gunpickup2.wav")
	lsg_n138(ctx)
}

// Bots: Clear Offer (ev45)
lsg_n138(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer_clear %s %d", lsg_who(0, 0, ""), ctx[0])))
			{}
	}
	lsg_n139(ctx)
}

// Remove Marker (ev45)
lsg_n139(ctx[])
{
	lsg_marker_remove(lsg_agetc(g_v15, float(lsg_afindc(g_v13, ctx[0]))))
	lsg_n140(ctx)
}

// Remove Trigger Volume (ev45)
lsg_n140(ctx[])
{
	lsg_vol_remove(ctx[0])
	lsg_n141(ctx)
}

// Set Variable (ev45)
lsg_n141(ctx[])
{
	lsg_vset((LSG_VR + 1), lsg_agetv(g_v14, float(lsg_afindc(g_v13, ctx[0]))))
	lsg_n142(ctx)
}

// Remove Index (ev45)
lsg_n142(ctx[])
{
	lsg_aremi(g_v13, float(lsg_afindc(g_v13, ctx[0])))
	lsg_n143(ctx)
}

// Remove Index (ev45)
lsg_n143(ctx[])
{
	lsg_aremi(g_v14, float(lsg_afindc(g_v13, ctx[0])))
	lsg_n144(ctx)
}

// Remove Index (ev45)
lsg_n144(ctx[])
{
	lsg_aremi(g_v15, float(lsg_afindc(g_v13, ctx[0])))
	lsg_n145(ctx)
}

// Sequence (ev45)
lsg_n145(ctx[])
{
	lsg_n146(ctx)
}

// Delay (ev45)
lsg_n146(ctx[])
{
	set_task(floatmax((((((15.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (15.0) : (20.0)), 0.1), "lsg_t146", LSG_TASK + 146, ctx, LSG_CTX)
}

public lsg_t146(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n175(ctx)
}

// Branch (ev54)
lsg_n147(ctx[])
{
	new ok = (((((lsg_ok(ctx[0]) && (pev(ctx[0], pev_flags) & FL_ONGROUND) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n148(ctx)
	else
		lsg_n149(ctx)
}

// Set Variable (ev54)
lsg_n148(ctx[])
{
	g_pv0[lsg_slot(ctx[0])] = 0
}

// Branch (ev54)
lsg_n149(ctx[])
{
	new ok = ((((g_pv0[lsg_slot(ctx[0])]) ? 1 : 0)) ? 1 : 0)
	if (!ok)
		lsg_n150(ctx)
}

// Set Velocity (ev54)
lsg_n150(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		lsg_vsetvel(ctx[0], lsg_vec(lsg_vc(lsg_vpev(ctx[0], pev_velocity), 0), lsg_vc(lsg_vpev(ctx[0], pev_velocity), 1), (((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (320.0))))
	}
	lsg_n151(ctx)
}

// Set Variable (ev54)
lsg_n151(ctx[])
{
	g_pv0[lsg_slot(ctx[0])] = 1
	lsg_n152(ctx)
}

// Play Sound To Player (ev54)
lsg_n152(ctx[])
{
	lsg_sound_to(ctx[0], "player/pl_jump2.wav")
}

// Branch (ev58)
lsg_n153(ctx[])
{
	new ok = ((((((((g_pv1[lsg_slot(ctx[0])]) - (get_gametime()))) <= (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n154(ctx)
}

// Set Velocity (ev58)
lsg_n154(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		lsg_vsetvel(ctx[0], lsg_vadd(lsg_vscale(lsg_vnorm(lsg_vec(lsg_vc(lsg_vplayer(ctx[0], 2), 0), lsg_vc(lsg_vplayer(ctx[0], 2), 1), 0.0)), (((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (700.0))), lsg_vec(0.0, 0.0, 120.0), 1.0))
	}
	lsg_n155(ctx)
}

// Set Variable (ev58)
lsg_n155(ctx[])
{
	g_pv1[lsg_slot(ctx[0])] = ((get_gametime()) + ((((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (3.0))))
	lsg_n156(ctx)
}

// Play Sound To Player (ev58)
lsg_n156(ctx[])
{
	lsg_sound_to(ctx[0], "weapons/knife_slash1.wav")
}

// Spawn Trigger Volume (ev4)
lsg_n157(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsg_vent(ctx[29]), lsgVp)
	g_lsgOut[157] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 48.0, 0.0, 0.0, 0, "heal", 32, 32, 0)
	g_lsgO[34] = g_lsgOut[157]
	g_lsgO[35] = g_lsgOut[157] ? 1 : 0
	g_lsgO[36] = g_lsgVolWhy
	lsg_n158(ctx)
}

// Objective Marker (ev4)
lsg_n158(ctx[])
{
	new Float:lsgAt[3]
	lsg_vto(lsg_vent(ctx[29]), lsgAt)
	g_lsgOut[158] = lsg_marker("sprites/glow01.spr", 0, lsgAt, 1, 16.0, 0.5, 40, 255, 60, 1, 0, 0, 32)
	lsg_n159(ctx)
}

// Add (ev4)
lsg_n159(ctx[])
{
	g_lsgO[37] = _:float(lsg_apushc(g_v9, g_lsgO[34]))
	lsg_n160(ctx)
}

// Add (ev4)
lsg_n160(ctx[])
{
	g_lsgO[38] = _:float(lsg_apushv(g_v10, lsg_vent(ctx[29])))
	lsg_n161(ctx)
}

// Add (ev4)
lsg_n161(ctx[])
{
	g_lsgO[39] = _:float(lsg_apushc(g_v11, g_lsgOut[158]))
	lsg_n162(ctx)
}

// Bots: Offer (ev4)
lsg_n162(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer %s perk %s %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(g_lsgO[34]), g_lsgO[34], max(floatround(60.0), 0), 2, clamp(floatround(50.0), 0, 100))))
			{}
	}
	lsg_n79(ctx)
}

// Spawn Trigger Volume (ev6)
lsg_n163(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto(lsg_vent(ctx[30]), lsgVp)
	g_lsgOut[163] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 48.0, 0.0, 0.0, 0, "weap", 32, 32, 0)
	g_lsgO[40] = g_lsgOut[163]
	g_lsgO[41] = g_lsgOut[163] ? 1 : 0
	g_lsgO[42] = g_lsgVolWhy
	lsg_n164(ctx)
}

// Objective Marker (ev6)
lsg_n164(ctx[])
{
	new Float:lsgAt[3]
	lsg_vto(lsg_vent(ctx[30]), lsgAt)
	g_lsgOut[164] = lsg_marker("sprites/arrow1.spr", 0, lsgAt, 1, 24.0, 0.4, 255, 200, 0, 1, 0, 0, 32)
	lsg_n165(ctx)
}

// Add (ev6)
lsg_n165(ctx[])
{
	g_lsgO[43] = _:float(lsg_apushc(g_v13, g_lsgO[40]))
	lsg_n166(ctx)
}

// Add (ev6)
lsg_n166(ctx[])
{
	g_lsgO[44] = _:float(lsg_apushv(g_v14, lsg_vent(ctx[30])))
	lsg_n167(ctx)
}

// Add (ev6)
lsg_n167(ctx[])
{
	g_lsgO[45] = _:float(lsg_apushc(g_v15, g_lsgOut[164]))
	lsg_n168(ctx)
}

// Bots: Offer (ev6)
lsg_n168(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer %s perk %s %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(g_lsgO[40]), g_lsgO[40], max(floatround(60.0), 0), 1, clamp(floatround(50.0), 0, 100))))
			{}
	}
	lsg_n80(ctx)
}

// Spawn Trigger Volume (ev43)
lsg_n169(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto((LSG_VR + 0), lsgVp)
	g_lsgOut[169] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 48.0, 0.0, 0.0, 0, "heal", 32, 32, 0)
	g_lsgO[46] = g_lsgOut[169]
	g_lsgO[47] = g_lsgOut[169] ? 1 : 0
	g_lsgO[48] = g_lsgVolWhy
	lsg_n170(ctx)
}

// Objective Marker (ev43)
lsg_n170(ctx[])
{
	new Float:lsgAt[3]
	lsg_vto((LSG_VR + 0), lsgAt)
	g_lsgOut[170] = lsg_marker("sprites/glow01.spr", 0, lsgAt, 1, 16.0, 0.5, 40, 255, 60, 1, 0, 0, 32)
	lsg_n171(ctx)
}

// Add (ev43)
lsg_n171(ctx[])
{
	g_lsgO[49] = _:float(lsg_apushc(g_v9, g_lsgO[46]))
	lsg_n172(ctx)
}

// Add (ev43)
lsg_n172(ctx[])
{
	g_lsgO[50] = _:float(lsg_apushv(g_v10, (LSG_VR + 0)))
	lsg_n173(ctx)
}

// Add (ev43)
lsg_n173(ctx[])
{
	g_lsgO[51] = _:float(lsg_apushc(g_v11, g_lsgOut[170]))
	lsg_n174(ctx)
}

// Bots: Offer (ev43)
lsg_n174(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer %s perk %s %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(g_lsgO[46]), g_lsgO[46], max(floatround(60.0), 0), 2, clamp(floatround(50.0), 0, 100))))
			{}
	}
	#pragma unused ctx
}

// Spawn Trigger Volume (ev45)
lsg_n175(ctx[])
{
	new Float:lsgVp[3], Float:lsgVa[3], Float:lsgVo[3]
	lsg_vto((LSG_VR + 1), lsgVp)
	g_lsgOut[175] = lsg_vol_spawn(0, lsgVp, lsgVa, lsgVo, 1, 48.0, 0.0, 0.0, 0, "weap", 32, 32, 0)
	g_lsgO[52] = g_lsgOut[175]
	g_lsgO[53] = g_lsgOut[175] ? 1 : 0
	g_lsgO[54] = g_lsgVolWhy
	lsg_n176(ctx)
}

// Objective Marker (ev45)
lsg_n176(ctx[])
{
	new Float:lsgAt[3]
	lsg_vto((LSG_VR + 1), lsgAt)
	g_lsgOut[176] = lsg_marker("sprites/arrow1.spr", 0, lsgAt, 1, 24.0, 0.4, 255, 200, 0, 1, 0, 0, 32)
	lsg_n177(ctx)
}

// Add (ev45)
lsg_n177(ctx[])
{
	g_lsgO[55] = _:float(lsg_apushc(g_v13, g_lsgO[52]))
	lsg_n178(ctx)
}

// Add (ev45)
lsg_n178(ctx[])
{
	g_lsgO[56] = _:float(lsg_apushv(g_v14, (LSG_VR + 1)))
	lsg_n179(ctx)
}

// Add (ev45)
lsg_n179(ctx[])
{
	g_lsgO[57] = _:float(lsg_apushc(g_v15, g_lsgOut[176]))
	lsg_n180(ctx)
}

// Bots: Offer (ev45)
lsg_n180(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("offer %s perk %s %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(g_lsgO[52]), g_lsgO[52], max(floatround(60.0), 0), 1, clamp(floatround(50.0), 0, 100))))
			{}
	}
	#pragma unused ctx
}

