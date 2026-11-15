/*
	Starter library test B - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from starter_test_b.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "starter_test_b"
#define LSG_CTX 23
#define LSG_TXT 2
#define LSG_N 99
#define LSG_TASK 7310000
#pragma dynamic 67456
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
#define LSG_OT 3

// The mode's variables
stock g_pv0[33] // Protected (bool, per player)
stock Float:g_pv1[33] // Lives (number, per player)
stock Float:g_pv2[33] // Kill streak (number, per player)
stock Float:g_v3 = 0.0 // Countdown left (number)
stock g_v4[64] = "" // Countdown label (text)
stock g_v5[64] = "" // Leaderboard text (text)
stock g_pv6[33][64] // Quest step (text, per player)

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

// ---- loops never freeze the server ----
// every turn of a loop counts against the frame's Loop steps a frame; at the limit an event
// graph's loop is saved where it is and goes on next frame (in order: the innermost first, then
// the loops around it), and a loop in a Function stops at its own ceiling
#define LSG_LF 64
stock Float:g_lsgStepAt, g_lsgSteps, g_lsgStepLim = 20000, Float:g_lsgFnAt, g_lsgFnSteps, g_lsgFnLim = 1000000
stock bool:g_lsgYield, g_lsgLFn, g_lsgLFins = -1, g_lsgLFnode[LSG_LF], g_lsgLFk[LSG_LF], g_lsgLFa[LSG_LF], g_lsgLFb[LSG_LF], g_lsgLFcnt[LSG_LF], g_lsgLFctx[LSG_LF][LSG_CTX], bool:g_lsgLFasked, Float:g_lsgLFsaid, g_lsgLFspread
stock bool:lsg_loop_full()
{
	new Float:now = get_gametime()
	if (now != g_lsgStepAt)
	{
		g_lsgStepAt = now
		g_lsgSteps = 0
	}
	if (g_lsgSteps < g_lsgStepLim)
	{
		g_lsgSteps++
		return false
	}
	return true
}

// a loop stopped where it is: kept, to go on next frame
stock lsg_loop_save(node, const ctx[], k, a, b, n)
{
	g_lsgYield = true
	if (g_lsgLFn >= LSG_LF)
	{
		// (more loops waiting than it keeps: this one ends here, said in the log; never a freeze)
		log_amx("[%s] a loop ended early: %d loops are already waiting for the next frame", LSG_MODE, LSG_LF)
		return
	}
	new at = g_lsgLFins >= 0 ? g_lsgLFins++ : g_lsgLFn
	for (new i = g_lsgLFn; i > at; i--)
	{
		g_lsgLFnode[i] = g_lsgLFnode[i - 1]
		g_lsgLFk[i] = g_lsgLFk[i - 1]
		g_lsgLFa[i] = g_lsgLFa[i - 1]
		g_lsgLFb[i] = g_lsgLFb[i - 1]
		g_lsgLFcnt[i] = g_lsgLFcnt[i - 1]
		for (new j = 0; j < LSG_CTX; j++)
			g_lsgLFctx[i][j] = g_lsgLFctx[i - 1][j]
	}
	g_lsgLFnode[at] = node
	g_lsgLFk[at] = k
	g_lsgLFa[at] = a
	g_lsgLFb[at] = b
	g_lsgLFcnt[at] = n
	for (new j = 0; j < LSG_CTX; j++)
		g_lsgLFctx[at][j] = ctx[j]
	g_lsgLFn++
	g_lsgLFspread++
	if (!g_lsgLFasked)
	{
		g_lsgLFasked = true
		RequestFrame("lsg_loop_resume")
	}
}

// next frame: the waiting loops go on, the first first, as far as the frame's steps go
public lsg_loop_resume()
{
	g_lsgLFasked = false
	new Float:now = get_gametime()
	if (now - g_lsgLFsaid >= 1.0)
	{
		g_lsgLFsaid = now
		log_amx("[%s] long loops were spread over frames (%d times in the last second) so the server keeps running (Loop steps a frame, Limits)", LSG_MODE, g_lsgLFspread)
		g_lsgLFspread = 0
	}
	new ctx[LSG_CTX]
	while (g_lsgLFn > 0)
	{
		new node = g_lsgLFnode[0], k = g_lsgLFk[0], a = g_lsgLFa[0], b = g_lsgLFb[0], n = g_lsgLFcnt[0]
		for (new j = 0; j < LSG_CTX; j++)
			ctx[j] = g_lsgLFctx[0][j]
		for (new i = 1; i < g_lsgLFn; i++)
		{
			g_lsgLFnode[i - 1] = g_lsgLFnode[i]
			g_lsgLFk[i - 1] = g_lsgLFk[i]
			g_lsgLFa[i - 1] = g_lsgLFa[i]
			g_lsgLFb[i - 1] = g_lsgLFb[i]
			g_lsgLFcnt[i - 1] = g_lsgLFcnt[i]
			for (new j = 0; j < LSG_CTX; j++)
				g_lsgLFctx[i - 1][j] = g_lsgLFctx[i][j]
		}
		g_lsgLFn--
		// (what it stops at goes first again, before the loops around it)
		g_lsgLFins = 0
		g_lsgYield = false
		lsg_loop_go(node, ctx, k, a, b, n)
		g_lsgLFins = -1
		if (g_lsgYield)
			break
	}
	if (g_lsgLFn && !g_lsgLFasked)
	{
		g_lsgLFasked = true
		RequestFrame("lsg_loop_resume")
	}
}

// a Function's loop: past its ceiling it stops (said once a second); the Function returns what it has
stock bool:lsg_loop_ceil(const what[])
{
	new Float:now = get_gametime()
	if (now != g_lsgFnAt)
	{
		g_lsgFnAt = now
		g_lsgFnSteps = 0
	}
	if (++g_lsgFnSteps <= g_lsgFnLim)
		return false
	if (now - g_lsgLFsaid >= 1.0)
	{
		g_lsgLFsaid = now
		log_amx("[%s] %s stopped: loops in a Function ran %d times in one go (Loop steps in a Function, Limits); the Function returns what it has", LSG_MODE, what, g_lsgFnLim)
	}
	return true
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

// ---- fun: fire, trails, bursts, swaps, burying, the camera, a drunk view ----
stock g_lsgSprFire, g_lsgSprSmoke, g_lsgSprBlood, g_lsgSprSpray
stock Float:g_lsgBurnTill[33], Float:g_lsgBurnDps[33], Float:g_lsgBurnNext[33], g_lsgBurnBy[33], g_lsgBurnFx[33]
stock bool:g_lsgBuried[33], g_lsgCam[33], g_lsgCamKeep[33], Float:g_lsgDrunkTill[33], Float:g_lsgDrunk[33], g_lsgFunN
#define LSG_TRAILS 256
stock g_lsgTrail[LSG_TRAILS], g_lsgTrailC[LSG_TRAILS][5], bool:g_lsgTrailStill[LSG_TRAILS]

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
	if (g_lsgSprFire && !(g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id])) && lsg_ent_room("Set On Fire's flames"))
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
	if (!lsg_msg_room("Trail", 2))
		return
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

stock lsg_trail(e, on, r, g, b, width, life, lim = 32)
{
	if (e <= 0 || !pev_valid(e))
		return 0
	new k = -1
	for (new i = 0; i < LSG_TRAILS; i++)
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
	for (new i = 0; i < min(lim, LSG_TRAILS) && k < 0; i++)
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
	for (new k = 0; k < LSG_TRAILS; k++)
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
	lsg_view_tick(now)
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
	if (!lsg_set_view(id, third ? CAMERA_3RDPERSON : CAMERA_NONE))
		return 0
	g_lsgCam[id] = third
	g_lsgCamKeep[id] = reset ? 0 : 1
	return 1
}

stock lsg_cam_reapply(id)
{
	if (lsg_ok(id) && g_lsgCam[id])
		lsg_set_view(id, CAMERA_3RDPERSON)
}

stock lsg_fun_died(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_view_back(id)
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
	lsg_view_back(id)
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

// View From: a camera entity each shot; players see through it (set_view) until it ends
stock g_lsgViewCam[33], Float:g_lsgViewTill[33], bool:g_lsgViewFreeze[33], g_lsgCamFollow[2048], Float:g_lsgCamOff[2048], Float:g_lsgCamStop[2048]
// what the next View From looks at (an entity, or a position), whether it frames a conversation
// and ends with it; and per camera, what it keeps pointing at
stock g_lsgVLook, Float:g_lsgVLookPos[3], bool:g_lsgVHasLook, bool:g_lsgVFrame, bool:g_lsgVTalk
stock g_lsgCamLook[2048], Float:g_lsgCamLookPos[2048][3], bool:g_lsgCamLooks[2048], bool:g_lsgViewTalk[33], bool:g_lsgViewTalked[33]

stock lsg_view_look(look, const Float:lp[3], hasLp, frame, talk)
{
	g_lsgVLook = look > 0 && pev_valid(look) ? look : 0
	g_lsgVLookPos = lp
	g_lsgVHasLook = g_lsgVLook > 0 || hasLp != 0
	g_lsgVFrame = frame != 0
	g_lsgVTalk = talk != 0
}

// the point a camera looks at: its entity's (a player's eyes) or its position
stock bool:lsg_cam_target(cam, Float:t[3])
{
	new e = g_lsgCamLook[cam]
	if (e > 0)
	{
		if (!pev_valid(e))
			return false
		pev(e, pev_origin, t)
		new Float:ofs[3]
		pev(e, pev_view_ofs, ofs)
		t[2] += ofs[2] > 0.0 ? ofs[2] : 24.0
		return true
	}
	if (!g_lsgCamLooks[cam])
		return false
	t = g_lsgCamLookPos[cam]
	return true
}

// "until the conversation ends": once one has started for them (Say or Choose may come a little
// after View From), it's over when they aren't talking any more
stock bool:lsg_view_talk_over(p)
{
	if (!g_lsgViewTalk[p] || !ls_components_running())
		return false
	if (ls_npc_talking(p))
	{
		g_lsgViewTalked[p] = true
		return false
	}
	return g_lsgViewTalked[p]
}

// a view entity's angles toward a point (the pitch the other way round, as trigger_camera does)
stock lsg_cam_aim(cam)
{
	new Float:t[3], Float:o[3], Float:d[3], Float:ang[3]
	if (!lsg_cam_target(cam, t))
		return
	pev(cam, pev_origin, o)
	for (new k = 0; k < 3; k++)
		d[k] = t[k] - o[k]
	vector_to_angle(d, ang)
	ang[0] = -ang[0]
	if (ang[0] < -180.0)
		ang[0] += 360.0
	set_pev(cam, pev_angles, ang)
	set_pev(cam, pev_v_angle, ang)
}


stock lsg_view_from(who, one, from, const Float:a[3], const Float:aa[3], hasA, const Float:b[3], const Float:ba[3], hasB, Float:off, Float:move, Float:secs, freeze)
{
	if (from <= 0 && !hasA && !(g_lsgVFrame && g_lsgVLook))
		return 0
	if (from > 0 && !pev_valid(from))
		return 0
	new n = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		if ((who == 0 && p != one) || (who == 1 && get_user_team(p) != 1) || (who == 2 && get_user_team(p) != 2))
			continue
		// (each player a camera of their own)
		new cam = lsg_view_cam(p, from, a, aa, b, ba, hasB, off, move)
		if (!cam)
			continue
		lsg_view_back(p)
		g_lsgViewCam[p] = cam
		g_lsgViewTill[p] = secs > 0.0 ? get_gametime() + secs : 0.0
		g_lsgViewFreeze[p] = freeze != 0
		g_lsgViewTalk[p] = g_lsgVTalk
		g_lsgViewTalked[p] = false
		if (freeze)
			set_pev(p, pev_flags, pev(p, pev_flags) | FL_FROZEN)
		engfunc(EngFunc_SetView, p, cam)
		n++
	}
	return n
}

// one viewer's camera: where it stands, how it moves, what it looks at
stock lsg_view_cam(p, from, const Float:a[3], const Float:aa[3], const Float:b[3], const Float:ba[3], hasB, Float:off, Float:move)
{
	if (!lsg_ent_room("View From"))
		return 0
	new cam = create_entity("info_target")
	if (cam <= 0)
		return 0
	set_pev(cam, pev_classname, "ls_studio_cam")
	engfunc(EngFunc_SetModel, cam, "models/w_c4.mdl")
	set_pev(cam, pev_rendermode, kRenderTransTexture)
	set_pev(cam, pev_renderamt, 0.0)
	set_pev(cam, pev_solid, SOLID_NOT)
	set_pev(cam, pev_movetype, MOVETYPE_NOCLIP)
	new Float:o[3], Float:ang[3]
	g_lsgCamFollow[cam] = 0
	g_lsgCamLook[cam] = g_lsgVLook
	g_lsgCamLookPos[cam] = g_lsgVLookPos
	g_lsgCamLooks[cam] = g_lsgVHasLook
	if (g_lsgVFrame && g_lsgVLook)
	{
		// to the side of the player and the NPC, a little above their eyes, looking at the NPC
		new Float:e[3], Float:t[3], Float:d[3], Float:ofs[3]
		pev(p, pev_origin, e)
		pev(p, pev_view_ofs, ofs)
		e[2] += ofs[2]
		pev(g_lsgVLook, pev_origin, t)
		for (new k = 0; k < 3; k++)
			d[k] = t[k] - e[k]
		new Float:len = floatsqroot(d[0] * d[0] + d[1] * d[1])
		if (len < 1.0)
			len = 1.0
		o[0] = (e[0] + t[0]) * 0.5 - d[1] / len * 110.0
		o[1] = (e[1] + t[1]) * 0.5 + d[0] / len * 110.0
		o[2] = e[2] + 12.0
	}
	else if (from > 0)
	{
		pev(from, pev_origin, o)
		o[2] += off
		pev(from, from <= MaxClients ? pev_v_angle : pev_angles, ang)
		g_lsgCamFollow[cam] = from
		g_lsgCamOff[cam] = off
	}
	else
	{
		o = a
		ang = aa
	}
	engfunc(EngFunc_SetOrigin, cam, o)
	set_pev(cam, pev_angles, ang)
	set_pev(cam, pev_v_angle, ang)
	g_lsgCamStop[cam] = 0.0
	// a glide from A to B: the engine moves it (velocity, angular velocity); stopped when it's there
	if (from <= 0 && hasB && move > 0.0 && !g_lsgVFrame)
	{
		new Float:vel[3], Float:av[3]
		for (new k = 0; k < 3; k++)
		{
			vel[k] = (b[k] - a[k]) / move
			av[k] = (ba[k] - aa[k]) / move
		}
		set_pev(cam, pev_velocity, vel)
		set_pev(cam, pev_avelocity, av)
		g_lsgCamStop[cam] = get_gametime() + move
	}
	lsg_cam_aim(cam)
	return cam
}

// one player's view back: first or third person as Set Camera left it
stock lsg_view_back(p)
{
	new cam = g_lsgViewCam[p]
	if (!cam)
		return
	g_lsgViewCam[p] = 0
	g_lsgViewTill[p] = 0.0
	g_lsgViewTalk[p] = false
	if (is_user_connected(p))
	{
		if (g_lsgViewFreeze[p])
			set_pev(p, pev_flags, pev(p, pev_flags) & ~FL_FROZEN)
		engfunc(EngFunc_SetView, p, p)
		if (g_lsgCam[p])
			lsg_set_view(p, CAMERA_3RDPERSON)
	}
	g_lsgViewFreeze[p] = false
	// (the camera goes when nobody looks through it any more)
	for (new q = 1; q <= MaxClients; q++)
		if (g_lsgViewCam[q] == cam)
			return
	if (pev_valid(cam))
		remove_entity(cam)
}

stock lsg_view_return(p)
{
	if (p)
		lsg_view_back(p)
	else
		for (new q = 1; q <= MaxClients; q++)
			lsg_view_back(q)
}

stock lsg_view_tick(Float:now)
{
	for (new p = 1; p <= MaxClients; p++)
	{
		new cam = g_lsgViewCam[p]
		if (!cam)
			continue
		if (!pev_valid(cam) || (g_lsgViewTill[p] > 0.0 && now >= g_lsgViewTill[p]) || lsg_view_talk_over(p))
		{
			lsg_view_back(p)
			continue
		}
		new f = g_lsgCamFollow[cam]
		if (f > 0)
		{
			if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
			{
				lsg_view_back(p)
				continue
			}
			new Float:o[3], Float:ang[3]
			pev(f, pev_origin, o)
			o[2] += g_lsgCamOff[cam]
			pev(f, f <= MaxClients ? pev_v_angle : pev_angles, ang)
			engfunc(EngFunc_SetOrigin, cam, o)
			set_pev(cam, pev_angles, ang)
			set_pev(cam, pev_v_angle, ang)
		}
		else if (g_lsgCamStop[cam] > 0.0 && now >= g_lsgCamStop[cam])
		{
			set_pev(cam, pev_velocity, Float:{0.0, 0.0, 0.0})
			set_pev(cam, pev_avelocity, Float:{0.0, 0.0, 0.0})
			g_lsgCamStop[cam] = 0.0
		}
		// (a camera with something to look at keeps pointing at it, over its own angles)
		if (g_lsgCamLook[cam] || g_lsgCamLooks[cam])
			lsg_cam_aim(cam)
	}
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


// ---- W1's utility nodes: spatial queries, random picks, text, number arrays, vector math, relative
// space and aim. How many entities a search keeps and how far an aim trace goes are the node's
// own settings; every index and length is clamped and every loop bounded (4096 entities looked at).
stock Float:g_lsgW1Wt[64], g_lsgW1Wk[64]
stock Float:g_lsgW1AimT[33], Float:g_lsgW1AimD[33], Float:g_lsgW1AimPos[33][3], Float:g_lsgW1AimNrm[33][3], g_lsgW1AimEnt[33], g_lsgW1AimHit[33]

// the point a query is around: a point (a vector slot) or an entity (its position; a player's
// feet); false when there's neither
stock bool:lsg_w1_at(pos, ent, Float:o[3])
{
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		return true
	}
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		return true
	}
	return false
}

stock bool:lsg_w1_pfits(p, alive, team, bots)
{
	if (!is_user_connected(p) || (alive && !is_user_alive(p)))
		return false
	if (team && get_user_team(p) != team)
		return false
	if (bots == 1 && is_user_bot(p))
		return false
	if (bots == 2 && !is_user_bot(p))
		return false
	return true
}

stock lsg_w1_pin(pos, ent, Float:r, alive, team)
{
	new a = lsg_atmp(), Float:at[3], Float:o[3]
	if (!lsg_w1_at(pos, ent, at))
		return a
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == ent || !lsg_w1_pfits(p, alive, team, 0))
			continue
		pev(p, pev_origin, o)
		if (get_distance_f(o, at) <= r)
			ArrayPushCell((Array:a), p)
	}
	return a
}

stock lsg_w1_np(pos, ent, Float:maxd, alive, team, bots, bool:dist = false)
{
	new Float:at[3], Float:o[3], best, Float:bd = maxd
	if (lsg_w1_at(pos, ent, at))
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == ent || !lsg_w1_pfits(p, alive, team, bots))
				continue
			pev(p, pev_origin, o)
			new Float:d = get_distance_f(o, at)
			if (d <= bd)
			{
				bd = d
				best = p
			}
		}
	return dist ? (best ? (_:bd) : (_:-1.0)) : best
}

stock lsg_w1_ein(pos, ent, Float:r, const cls[], most)
{
	new a = lsg_atmp(), Float:at[3], Float:o[3], e = MaxClients, n, looked
	if (!lsg_w1_at(pos, ent, at))
		return a
	if (cls[0])
	{
		while (n < most && looked++ < 4096 && (e = engfunc(EngFunc_FindEntityByString, e, "classname", cls)) > 0)
		{
			if (e == ent || e <= MaxClients)
				continue
			pev(e, pev_origin, o)
			if (get_distance_f(o, at) <= r)
			{
				ArrayPushCell((Array:a), e)
				n++
			}
		}
		return a
	}
	while (n < most && looked++ < 4096 && (e = engfunc(EngFunc_FindEntityInSphere, e, at, r)) > 0)
	{
		if (e == ent || e <= MaxClients)
			continue
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_w1_ne(pos, ent, const cls[], Float:maxd, bool:dist = false)
{
	new Float:at[3], Float:o[3], e = MaxClients, best, Float:bd = maxd, looked
	if (cls[0] && lsg_w1_at(pos, ent, at))
		while (looked++ < 4096 && (e = engfunc(EngFunc_FindEntityByString, e, "classname", cls)) > 0)
		{
			if (e == ent || e <= MaxClients)
				continue
			pev(e, pev_origin, o)
			new Float:d = get_distance_f(o, at)
			if (d <= bd)
			{
				bd = d
				best = e
			}
		}
	return dist ? (best ? (_:bd) : (_:-1.0)) : best
}

// an end of a line: an entity (a player's eyes) or a point
stock lsg_w1_end(ent, pos, Float:o[3])
{
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		if (ent <= MaxClients)
		{
			new Float:ofs[3]
			pev(ent, pev_view_ofs, ofs)
			o[2] += ofs[2]
		}
		return
	}
	if (pos >= 0)
		lsg_vto(pos, o)
}

// a clear line from A to B? (the entities at the ends don't block it); hit: where it stopped
stock lsg_w1_los(aent, apos, bent, bpos, monsters, bool:hit = false)
{
	new Float:a[3], Float:b[3], Float:frac, Float:end[3]
	lsg_w1_end(aent, apos, a)
	lsg_w1_end(bent, bpos, b)
	engfunc(EngFunc_TraceLine, a, b, monsters ? DONT_IGNORE_MONSTERS : IGNORE_MONSTERS, aent > 0 ? aent : 0, 0)
	get_tr2(0, TR_flFraction, frac)
	get_tr2(0, TR_vecEndPos, end)
	new t = get_tr2(0, TR_pHit)
	if (hit)
		return lsg_vfrom(frac >= 1.0 ? b : end)
	return (frac >= 1.0 || (bent > 0 && t == bent)) ? 1 : 0
}

// a weighted pick (one roll per node per server frame, so Index and Item agree)
stock lsg_w1_wpick(w, node)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (g_lsgW1Wt[k] == now && now > 0.0)
		return g_lsgW1Wk[k]
	new n = min(lsg_alen(w), LSG_AMAX), Float:total = 0.0
	for (new i = 0; i < n; i++)
		total += floatmax(Float:ArrayGetCell((Array:w), i), 0.0)
	new pick = -1
	if (total > 0.0)
	{
		new Float:r = random_float(0.0, total)
		for (new i = 0; i < n; i++)
		{
			r -= floatmax(Float:ArrayGetCell((Array:w), i), 0.0)
			if (r <= 0.0 && Float:ArrayGetCell((Array:w), i) > 0.0)
			{
				pick = i
				break
			}
		}
		if (pick < 0)
			for (new i = n - 1; i >= 0 && pick < 0; i--)
				if (Float:ArrayGetCell((Array:w), i) > 0.0)
					pick = i
	}
	g_lsgW1Wt[k] = now
	g_lsgW1Wk[k] = pick
	return pick
}

stock Float:lsg_w1_map(Float:x, Float:a, Float:b, Float:c, Float:d, clamp_it)
{
	if (floatabs(b - a) < 0.000001)
		return c
	new Float:t = (x - a) / (b - a)
	if (clamp_it)
		t = floatclamp(t, 0.0, 1.0)
	return c + (d - c) * t
}

// ---- text (every result fits 128 cells; every position clamped)
stock lsg_w1_split(const s[], const sep[])
{
	new a = lsg_atmp(), piece[128], n, len = strlen(s), sl = strlen(sep), start
	if (!sl)
	{
		ArrayPushString((Array:a), s)
		return a
	}
	for (new i = 0; i <= len && n < 64; i++)
	{
		if (i == len || (i + sl <= len && equal(s[i], sep, sl)))
		{
			new c = min(i - start, charsmax(piece))
			copy(piece, c, s[start])
			ArrayPushString((Array:a), piece)
			n++
			start = i + sl
			i += sl - 1
		}
	}
	return a
}

stock lsg_w1_repl(const s[], const find[], const with[])
{
	new t[128]
	copy(t, charsmax(t), s)
	if (find[0])
		replace_all(t, charsmax(t), find, with)
	return t
}

stock lsg_w1_sub(const s[], Float:start, Float:len)
{
	new t[128], l = strlen(s)
	new b = clamp(floatround(start, floatround_floor), 0, l), n = clamp(floatround(len, floatround_floor), 0, charsmax(t))
	copy(t, min(n, l - b), s[b])
	return t
}

stock lsg_w1_case(const s[], up)
{
	new t[128]
	copy(t, charsmax(t), s)
	if (up)
		strtoupper(t)
	else
		strtolower(t)
	return t
}

stock lsg_w1_trim(const s[])
{
	new t[128]
	copy(t, charsmax(t), s)
	trim(t)
	return t
}

stock bool:lsg_w1_starts(const s[], const p[])
	return equali(s, p, strlen(p)) != 0

stock bool:lsg_w1_ends(const s[], const p[])
{
	new l = strlen(s), m = strlen(p)
	return m <= l && equali(s[l - m], p) != 0
}

stock lsg_w1_join(a, const sep[])
{
	new t[128], piece[128], n = min(lsg_alen(a), LSG_AMAX)
	for (new i = 0; i < n; i++)
	{
		ArrayGetString((Array:a), i, piece, charsmax(piece))
		if (i)
			add(t, charsmax(t), sep)
		add(t, charsmax(t), piece)
	}
	return t
}

// 0 auto (hours only when there are any), 1 m:ss, 2 h:mm:ss
stock lsg_w1_time(Float:secs, style)
{
	new t[32], s = max(floatround(secs, floatround_floor), 0)
	new h = s / 3600, m = (s / 60) % 60, x = s % 60
	if (style == 2 || (style == 0 && h > 0))
		formatex(t, charsmax(t), "%d:%02d:%02d", h, m, x)
	else
		formatex(t, charsmax(t), "%d:%02d", s / 60, x)
	return t
}

stock lsg_w1_num(Float:v, decimals, thousands)
{
	new t[48], raw[40], fmt[8]
	formatex(fmt, charsmax(fmt), "%%.%df", clamp(decimals, 0, 4))
	// (AMX Mod X's %f cuts the digits off: half a unit of the last decimal first, so it rounds)
	formatex(raw, charsmax(raw), fmt, floatabs(v) + 0.5 / floatpower(10.0, float(clamp(decimals, 0, 4))))
	if (!thousands)
	{
		formatex(t, charsmax(t), "%s%s", v < 0.0 ? "-" : "", raw)
		return t
	}
	new dot = contain(raw, "."), digits = dot >= 0 ? dot : strlen(raw), o
	if (v < 0.0)
		t[o++] = '-'
	for (new i = 0; i < digits && o < charsmax(t) - 1; i++)
	{
		if (i > 0 && (digits - i) % 3 == 0)
			t[o++] = ','
		t[o++] = raw[i]
	}
	t[o] = 0
	if (dot >= 0)
		add(t, charsmax(t), raw[dot])
	return t
}

// ---- number arrays: 0 sum, 1 min, 2 max, 3 average; the index of the min (1) or max (2)
stock Float:lsg_w1_astat(a, op, bool:idx = false)
{
	new n = min(lsg_alen(a), LSG_AMAX), best = -1
	new Float:acc = 0.0
	for (new i = 0; i < n; i++)
	{
		new Float:x = Float:ArrayGetCell((Array:a), i)
		switch (op)
		{
			case 1: if (best < 0 || x < acc) { acc = x; best = i; }
			case 2: if (best < 0 || x > acc) { acc = x; best = i; }
			default: acc += x
		}
	}
	if (idx)
		return float(best)
	if (op == 3)
		return n ? acc / float(n) : 0.0
	return acc
}

stock lsg_w1_areverse(a)
{
	new n = lsg_alen(a)
	for (new i = 0, j = n - 1; i < j; i++, j--)
		ArraySwap((Array:a), i, j)
}

stock lsg_w1_slice(a, Float:start, Float:count)
{
	new out = lsg_atmp(), n = lsg_alen(a), b[LSG_AC]
	new s = clamp(floatround(start, floatround_floor), 0, n), c = clamp(floatround(count, floatround_floor), 0, LSG_AMAX)
	for (new k = s; k < n && k < s + c; k++)
	{
		ArrayGetArray((Array:a), k, b)
		ArrayPushArray((Array:out), b)
	}
	return out
}

// ---- vectors
stock Float:lsg_w1_dot(a, b)
	return lsg_vc(a, 0) * lsg_vc(b, 0) + lsg_vc(a, 1) * lsg_vc(b, 1) + lsg_vc(a, 2) * lsg_vc(b, 2)

stock lsg_w1_cross(a, b)
	return lsg_vec(lsg_vc(a, 1) * lsg_vc(b, 2) - lsg_vc(a, 2) * lsg_vc(b, 1), lsg_vc(a, 2) * lsg_vc(b, 0) - lsg_vc(a, 0) * lsg_vc(b, 2), lsg_vc(a, 0) * lsg_vc(b, 1) - lsg_vc(a, 1) * lsg_vc(b, 0))

stock lsg_w1_rnd(c, Float:r, ball)
{
	// uniform in the circle / ball (a root of the radius' share, else it bunches in the middle)
	new Float:yaw = random_float(0.0, 360.0)
	if (!ball)
	{
		new Float:d = r * floatsqroot(random_float(0.0, 1.0))
		return lsg_vec(lsg_vc(c, 0) + d * floatcos(yaw, degrees), lsg_vc(c, 1) + d * floatsin(yaw, degrees), lsg_vc(c, 2))
	}
	new Float:z = random_float(-1.0, 1.0), Float:d = r * floatpower(random_float(0.0, 1.0), 1.0 / 3.0)
	new Float:ring = floatsqroot(floatmax(1.0 - z * z, 0.0))
	return lsg_vec(lsg_vc(c, 0) + d * ring * floatcos(yaw, degrees), lsg_vc(c, 1) + d * ring * floatsin(yaw, degrees), lsg_vc(c, 2) + d * z)
}

stock lsg_w1_rot(a, Float:yaw, Float:pitch)
{
	new Float:x = lsg_vc(a, 0), Float:y = lsg_vc(a, 1), Float:z = lsg_vc(a, 2)
	// pitch first (up is negative pitch in GoldSrc), in the vector's own vertical plane
	if (pitch != 0.0)
	{
		new Float:h = floatsqroot(x * x + y * y), Float:cp = floatcos(-pitch, degrees), Float:sp = floatsin(-pitch, degrees)
		new Float:h2 = h * cp - z * sp, Float:z2 = h * sp + z * cp
		if (h > 0.0001)
		{
			x = x / h * h2
			y = y / h * h2
		}
		z = z2
	}
	new Float:cy = floatcos(yaw, degrees), Float:sy = floatsin(yaw, degrees)
	return lsg_vec(x * cy - y * sy, x * sy + y * cy, z)
}

stock lsg_w1_vlerp(a, b, Float:t)
	return lsg_vec(lsg_vc(a, 0) + (lsg_vc(b, 0) - lsg_vc(a, 0)) * t, lsg_vc(a, 1) + (lsg_vc(b, 1) - lsg_vc(a, 1)) * t, lsg_vc(a, 2) + (lsg_vc(b, 2) - lsg_vc(a, 2)) * t)

stock lsg_w1_toang(a)
{
	new Float:d[3], Float:ang[3]
	lsg_vto(a, d)
	vector_to_angle(d, ang)
	// (vector_to_angle gives pitch up as positive; a view's pitch is down-positive)
	return lsg_vec(-ang[0], ang[1], 0.0)
}

stock lsg_w1_fwd(a)
{
	new Float:ang[3], Float:f[3]
	lsg_vto(a, ang)
	angle_vector(ang, ANGLEVECTOR_FORWARD, f)
	return lsg_vfrom(f)
}

// ---- relative space: a thing's facing (a player's view yaw, else its angles' yaw)
stock Float:lsg_w1_yaw(e)
{
	new Float:ang[3]
	if (e > 0 && e <= MaxClients)
		pev(e, pev_v_angle, ang)
	else if (e > 0 && pev_valid(e))
		pev(e, pev_angles, ang)
	return ang[1]
}

stock lsg_w1_local(e, pos)
{
	new Float:o[3], Float:p[3]
	if (e > 0 && pev_valid(e))
		pev(e, pev_origin, o)
	lsg_vto(pos, p)
	new Float:dx = p[0] - o[0], Float:dy = p[1] - o[1], Float:y = lsg_w1_yaw(e)
	new Float:c = floatcos(y, degrees), Float:s = floatsin(y, degrees)
	// forward along the facing, right is clockwise from it (GoldSrc yaw turns left)
	return lsg_vec(dx * c + dy * s, dx * s - dy * c, p[2] - o[2])
}

stock lsg_w1_world(e, off)
{
	new Float:o[3]
	if (e > 0 && pev_valid(e))
		pev(e, pev_origin, o)
	new Float:f = lsg_vc(off, 0), Float:r = lsg_vc(off, 1), Float:y = lsg_w1_yaw(e)
	new Float:c = floatcos(y, degrees), Float:s = floatsin(y, degrees)
	return lsg_vec(o[0] + f * c + r * s, o[1] + f * s - r * c, o[2] + lsg_vc(off, 2))
}

stock Float:lsg_w1_relc(a, b, k)
{
	new Float:o[3]
	if (b > 0 && pev_valid(b))
		pev(b, pev_origin, o)
	return lsg_vc(lsg_w1_local(a, lsg_vfrom(o)), k)
}

stock Float:lsg_w1_angto(a, b)
{
	new Float:o[3], Float:p[3]
	if (a > 0 && pev_valid(a))
		pev(a, pev_origin, o)
	if (b > 0 && pev_valid(b))
		pev(b, pev_origin, p)
	new Float:d = floatatan2(p[1] - o[1], p[0] - o[0], degrees) - lsg_w1_yaw(a)
	while (d > 180.0)
		d -= 360.0
	while (d < -180.0)
		d += 360.0
	return d
}

// B inside a cone round A's facing (back: round A's back)
stock bool:lsg_w1_cone(a, b, Float:cone, back)
{
	new Float:d = floatabs(lsg_w1_angto(a, b))
	if (back)
		d = 180.0 - d
	return d <= cone * 0.5
}

// ---- aim: one trace per player per frame and distance, its four answers kept
stock lsg_w1_aim(id, Float:maxd, what)
{
	new k = clamp(id, 0, 32)
	new Float:now = get_gametime()
	if (g_lsgW1AimT[k] != now || g_lsgW1AimD[k] != maxd || now <= 0.0)
	{
		g_lsgW1AimT[k] = now
		g_lsgW1AimD[k] = maxd
		g_lsgW1AimEnt[k] = 0
		g_lsgW1AimHit[k] = 0
		for (new j = 0; j < 3; j++)
		{
			g_lsgW1AimPos[k][j] = 0.0
			g_lsgW1AimNrm[k][j] = 0.0
		}
		if (lsg_ok(id))
		{
			new Float:eye[3], Float:ofs[3], Float:ang[3], Float:fwd[3], Float:end[3], Float:frac
			pev(id, pev_origin, eye)
			pev(id, pev_view_ofs, ofs)
			eye[2] += ofs[2]
			pev(id, pev_v_angle, ang)
			angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
			new Float:d = floatmax(maxd, 16.0)
			for (new j = 0; j < 3; j++)
				end[j] = eye[j] + fwd[j] * d
			engfunc(EngFunc_TraceLine, eye, end, DONT_IGNORE_MONSTERS, id, 0)
			get_tr2(0, TR_flFraction, frac)
			get_tr2(0, TR_vecEndPos, g_lsgW1AimPos[k])
			get_tr2(0, TR_vecPlaneNormal, g_lsgW1AimNrm[k])
			new t = get_tr2(0, TR_pHit)
			g_lsgW1AimHit[k] = frac < 1.0 ? 1 : 0
			g_lsgW1AimEnt[k] = t > 0 ? t : 0
		}
	}
	switch (what)
	{
		case 1: return g_lsgW1AimEnt[k]
		case 2: return lsg_vfrom(g_lsgW1AimNrm[k])
		case 3: return g_lsgW1AimHit[k]
	}
	return lsg_vfrom(g_lsgW1AimPos[k])
}

// ---- testing: debug drawings (Playtest and Test run builds only: a release build has none; each
// within the shared message budget, lsg_msg_room) and Log Line

stock lsg_w1_te(kind, const Float:p[3], const Float:q[3], r, g, bl, Float:secs)
{
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(kind)
	for (new k = 0; k < 3; k++)
		engfunc(EngFunc_WriteCoord, p[k])
	for (new k = 0; k < 3; k++)
		engfunc(EngFunc_WriteCoord, q[k])
	write_short(clamp(floatround(secs * 10.0), 1, 600))
	write_byte(r)
	write_byte(g)
	write_byte(bl)
	message_end()
}

stock lsg_w1_line(a, b, r, g, bl, Float:secs)
{
	new Float:p[3], Float:q[3]
	lsg_vto(a, p)
	lsg_vto(b, q)
	lsg_w1_te(TE_LINE, p, q, r, g, bl, secs)
}

// a box between two corners (any two opposite ones), or round a point (half: its half size)
stock lsg_w1_box(a, b, Float:half, r, g, bl, Float:secs)
{
	new Float:p[3], Float:q[3], Float:lo[3], Float:hi[3]
	lsg_vto(a, p)
	if (b >= 0)
		lsg_vto(b, q)
	for (new k = 0; k < 3; k++)
	{
		if (b < 0)
		{
			lo[k] = p[k] - half
			hi[k] = p[k] + half
		}
		else
		{
			lo[k] = floatmin(p[k], q[k])
			hi[k] = floatmax(p[k], q[k])
		}
	}
	lsg_w1_te(TE_BOX, lo, hi, r, g, bl, secs)
}

stock Float:g_lsgW1LogT[64], g_lsgW1LogN[64], g_lsgW1LogSkip[64]
// to the AMX Mod X log, at most `most` lines a second from one node (the rest counted, and the
// count written with the next line that goes through)
stock bool:lsg_w1_log(node, const s[], most)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (now - g_lsgW1LogT[k] >= 1.0 || now < g_lsgW1LogT[k])
	{
		g_lsgW1LogT[k] = now
		g_lsgW1LogN[k] = 0
	}
	if (++g_lsgW1LogN[k] > most)
	{
		g_lsgW1LogSkip[k]++
		return false
	}
	if (g_lsgW1LogSkip[k])
		log_amx("%s (%d lines skipped before this one: at most %d a second)", s, g_lsgW1LogSkip[k], most)
	else
		log_amx("%s", s)
	g_lsgW1LogSkip[k] = 0
	return true
}

// ---- whole-number bits: 0 and, 1 or, 2 xor, 3 not, 4 shift left, 5 shift right
stock Float:lsg_w1_bits(Float:a, Float:b, op)
{
	new x = floatround(a, floatround_tozero), y = floatround(b, floatround_tozero), r
	switch (op)
	{
		case 1: r = x | y
		case 2: r = x ^ y
		case 3: r = ~x
		case 4: r = x << clamp(y, 0, 31)
		case 5: r = x >> clamp(y, 0, 31)
		default: r = x & y
	}
	return float(r)
}

// ---- a free spot near a point: the point, then rings round it, a hull trace at each, at most
// `most` traces; one search per node per server frame (Found and Position agree). 0 Found, 1 position
stock Float:g_lsgW1FsT[64], g_lsgW1FsOk[64], Float:g_lsgW1FsPos[64][3]
stock bool:lsg_w1_fits(const Float:p[3], hull)
{
	engfunc(EngFunc_TraceHull, p, p, DONT_IGNORE_MONSTERS, hull, 0, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid) && get_tr2(0, TR_InOpen)
}

stock lsg_w1_free(node, c, hull, Float:maxr, most, what)
{
	new k = node & 63
	new Float:now = get_gametime()
	if (g_lsgW1FsT[k] != now || now <= 0.0)
	{
		g_lsgW1FsT[k] = now
		g_lsgW1FsOk[k] = 0
		new Float:o[3], Float:p[3]
		lsg_vto(c, o)
		g_lsgW1FsPos[k] = o
		new Float:step = hull == HULL_POINT ? 8.0 : (hull == HULL_LARGE ? 68.0 : 36.0)
		new tries = 1
		if (lsg_w1_fits(o, hull))
			g_lsgW1FsOk[k] = 1
		for (new ring = 1; !g_lsgW1FsOk[k] && float(ring) * step <= maxr && tries < most; ring++)
		{
			new n = min(8 * ring, 32)
			for (new j = 0; j < n && tries < most; j++, tries++)
			{
				new Float:ang = 360.0 * float(j) / float(n), Float:d = float(ring) * step
				p[0] = o[0] + d * floatcos(ang, degrees)
				p[1] = o[1] + d * floatsin(ang, degrees)
				p[2] = o[2]
				if (lsg_w1_fits(p, hull))
				{
					g_lsgW1FsOk[k] = 1
					g_lsgW1FsPos[k] = p
					break
				}
			}
		}
	}
	return what ? lsg_vfrom(g_lsgW1FsPos[k]) : g_lsgW1FsOk[k]
}

// ---- a player's language: AMX Mod X's (their "lang" setting, else the server's), or the game's
// (cl_language, asked of their game once: empty until it answers, and for bots)
stock g_lsgW1Lang[33][16], g_lsgW1LangUid[33]
stock lsg_w1_lang(id, game)
{
	new t[16]
	if (!lsg_ok(id))
		return t
	if (!game)
	{
		get_user_info(id, "lang", t, charsmax(t))
		if (!t[0])
			get_cvar_string("amx_language", t, charsmax(t))
		return t
	}
	if (is_user_bot(id))
		return t
	new uid = get_user_userid(id)
	if (g_lsgW1LangUid[id] != uid)
	{
		g_lsgW1LangUid[id] = uid
		g_lsgW1Lang[id][0] = 0
		query_client_cvar(id, "cl_language", "lsg_w1_langq")
	}
	copy(t, charsmax(t), g_lsgW1Lang[id])
	return t
}

public lsg_w1_langq(id, const cvar[], const value[])
{
	if (id < 1 || id > 32 || containi(value, "Bad CVAR") >= 0)
		return
	copy(g_lsgW1Lang[id], charsmax(g_lsgW1Lang[]), value)
}


// enum Entity Kind
stock const g_lsgE0[][] = { "Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None" }
// enum Component Kind
stock const g_lsgE1[][] = { "Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None" }
// enum Spawn Failure
stock const g_lsgE2[][] = { "None", "Blocked", "Out of entity slots", "Limit reached", "Missing file", "Retries used up", "Bad input" }
// Function: End Spawn Protection
stock bool:g_lsgFD0
stock lsg_fr0(a0)
{
	new ctx[LSG_CTX]
	ctx[8] = _:a0
	g_lsgFD0 = false
	lsg_n0(ctx)
}

// Function: Reset Lives
stock bool:g_lsgFD1
stock lsg_fr1(a0, Float:a1)
{
	new ctx[LSG_CTX]
	ctx[9] = _:a0
	ctx[10] = _:a1
	g_lsgFD1 = false
	lsg_n6(ctx)
}

// Function: Lose a Life
stock Float:g_lsgR2_0
stock g_lsgR2_1
stock bool:g_lsgFD2
stock lsg_fr2(a0)
{
	new ctx[LSG_CTX]
	ctx[11] = _:a0
	g_lsgR2_0 = 0.0
	g_lsgR2_1 = 0
	g_lsgFD2 = false
	lsg_n9(ctx)
}

// Function: Count Kill Streak
stock Float:g_lsgR3_0
stock bool:g_lsgFD3
stock lsg_fr3(a0, a1)
{
	new ctx[LSG_CTX]
	ctx[12] = _:a0
	ctx[13] = _:a1
	g_lsgR3_0 = 0.0
	g_lsgFD3 = false
	lsg_n13(ctx)
}

// Function: Show Leaderboard
stock bool:g_lsgFD4
stock lsg_fr4(a0)
{
	new ctx[LSG_CTX]
	ctx[14] = _:a0
	g_lsgFD4 = false
	lsg_n27(ctx)
}

// Function: Make Capture Zone
stock g_lsgR5_0
stock bool:g_lsgFD5
stock lsg_fr5(a0)
{
	new ctx[LSG_CTX]
	ctx[15] = _:a0
	g_lsgR5_0 = 0
	g_lsgFD5 = false
	lsg_n33(ctx)
}

// Function: Set Quest Step
stock bool:g_lsgFD6
stock lsg_fr6(a0, const a1[], const a2[])
{
	new ctx[LSG_CTX]
	ctx[16] = _:a0
	copy(g_lsgOT[1], 127, a1)
	copy(g_lsgOT[2], 127, a2)
	g_lsgFD6 = false
	lsg_n37(ctx)
}

// Function: Quest Step Of
stock g_lsgR7_0[128]
stock g_lsgR7_1
stock bool:g_lsgFD7
stock lsg_fr7(a0)
{
	new ctx[LSG_CTX]
	ctx[17] = _:a0
	g_lsgR7_0[0] = 0
	g_lsgR7_1 = 0
	g_lsgFD7 = false
	lsg_n43(ctx)
}
stock lsg_pf7_0(a0)
{
	lsg_fr7(a0)
	new t[128]
	copy(t, 127, g_lsgR7_0)
	return t
}
stock lsg_pf7_1(a0)
{
	lsg_fr7(a0)
	return g_lsgR7_1
}

// Function: Quest Done
stock bool:g_lsgFD8
stock lsg_fr8(a0, Float:a1)
{
	new ctx[LSG_CTX]
	ctx[18] = _:a0
	ctx[19] = _:a1
	g_lsgFD8 = false
	lsg_n45(ctx)
}


stock Trie:g_lsgSaved = Invalid_Trie, bool:g_lsgSavedDirty

stock lsg_saved_path(out[], len, const ext[])
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(out, len, "%s/studio/%s.saved%s", dir, LSG_MODE, ext)
}

stock lsg_saved_load()
{
	g_lsgSaved = TrieCreate()
	new path[192], line[160], key[128], val[24]
	lsg_saved_path(path, charsmax(path), "")
	// (a crash between writing the new file and renaming it: the new one is whole)
	if (!file_exists(path))
		lsg_saved_path(path, charsmax(path), ".new")
	new f = fopen(path, "rt")
	if (!f)
		return
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		new tab = contain(line, "^t")
		if (tab <= 0)
			continue
		line[tab] = 0
		copy(key, charsmax(key), line)
		copy(val, charsmax(val), line[tab + 1])
		TrieSetCell(g_lsgSaved, key, str_to_float(val))
	}
	fclose(f)
}

public lsg_saved_flush()
{
	if (!g_lsgSavedDirty || g_lsgSaved == Invalid_Trie)
		return
	new path[192], tmp[200], dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_saved_path(path, charsmax(path), "")
	lsg_saved_path(tmp, charsmax(tmp), ".new")
	new f = fopen(tmp, "wt")
	if (!f)
		return
	new Snapshot:snap = TrieSnapshotCreate(g_lsgSaved), key[128], Float:v
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, key, charsmax(key))
		TrieGetCell(g_lsgSaved, key, v)
		fprintf(f, "%s^t%f^n", key, v)
	}
	TrieSnapshotDestroy(snap)
	fclose(f)
	// Windows won't rename over a file: the old one goes first (the .new one is read if a crash lands here)
	if (!rename_file(tmp, path, 1))
	{
		delete_file(path)
		rename_file(tmp, path, 1)
	}
	g_lsgSavedDirty = false
}

stock lsg_saved_key(id, const key[], out[], len)
{
	new who[44]
	get_user_authid(id, who, charsmax(who))
	// bots, LAN and pending ids are the same for everyone: their name instead
	if (is_user_bot(id) || !who[0] || containi(who, "LAN") >= 0 || containi(who, "PENDING") >= 0 || containi(who, "BOT") >= 0 || equali(who, "VALVE_ID_LOOPBACK"))
	{
		get_user_name(id, who, charsmax(who))
		formatex(out, len, "%s@name:%s", key, who)
	}
	else
		formatex(out, len, "%s@%s", key, who)
	// (a tab or a line end in a name would break the file's lines)
	for (new i = 0; out[i]; i++)
		if (out[i] < 32)
			out[i] = '_'
}

stock Float:lsg_saved_get(id, const key[])
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return 0.0
	new k[96], Float:v
	lsg_saved_key(id, key, k, charsmax(k))
	if (!TrieGetCell(g_lsgSaved, k, v))
		return 0.0
	return v
}

stock lsg_saved_set(id, const key[], Float:value)
{
	if (g_lsgSaved == Invalid_Trie || !lsg_ok(id))
		return
	new k[96]
	lsg_saved_key(id, key, k, charsmax(k))
	TrieSetCell(g_lsgSaved, k, value)
	// written within 5 s (a timer), at a new round, when the player leaves and at map end: not
	// the whole file on every call (PerfAudit #6: about 1 ms a write)
	g_lsgSavedDirty = true
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
#define LSG_P5TOP 100
stock Trie:g_lsgP5Txt = Invalid_Trie, bool:g_lsgP5TxtDirty
stock g_lsgP5TopName[LSG_P5TOP][32], Float:g_lsgP5TopVal[LSG_P5TOP]

stock lsg_p5_text_path(out[], len, const ext[])
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(out, len, "%s/studio/%s.savedtext%s", dir, LSG_MODE, ext)
}

stock lsg_p5_text_load()
{
	g_lsgP5Txt = TrieCreate()
	new path[192], line[200], key[128]
	lsg_p5_text_path(path, charsmax(path), "")
	if (!file_exists(path))
		lsg_p5_text_path(path, charsmax(path), ".new")
	new f = fopen(path, "rt")
	if (!f)
		return
	while (fgets(f, line, charsmax(line)))
	{
		new len = strlen(line)
		while (len > 0 && (line[len - 1] == '^n' || line[len - 1] == '^r'))
			line[--len] = 0
		new tab = contain(line, "^t")
		if (tab <= 0)
			continue
		line[tab] = 0
		copy(key, charsmax(key), line)
		TrieSetString(g_lsgP5Txt, key, line[tab + 1])
	}
	fclose(f)
}

public lsg_p5_text_flush()
{
	if (!g_lsgP5TxtDirty || g_lsgP5Txt == Invalid_Trie)
		return
	new path[192], tmp[200], dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_p5_text_path(path, charsmax(path), "")
	lsg_p5_text_path(tmp, charsmax(tmp), ".new")
	new f = fopen(tmp, "wt")
	if (!f)
		return
	new Snapshot:snap = TrieSnapshotCreate(g_lsgP5Txt), key[128], val[64]
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, key, charsmax(key))
		TrieGetString(g_lsgP5Txt, key, val, charsmax(val))
		fprintf(f, "%s^t%s^n", key, val)
	}
	TrieSnapshotDestroy(snap)
	fclose(f)
	if (!rename_file(tmp, path, 1))
	{
		delete_file(path)
		rename_file(tmp, path, 1)
	}
	g_lsgP5TxtDirty = false
}

// text for the file: one line, no tabs or control characters, at most 63 letters
stock lsg_p5_clean(const s[], out[], len)
{
	copy(out, len, s)
	for (new i = 0; out[i]; i++)
		if (out[i] < 32)
			out[i] = ' '
}

stock lsg_p5_text_get(id, const key[])
{
	new s[64], k[96]
	if (g_lsgP5Txt == Invalid_Trie || !lsg_ok(id))
		return s
	lsg_saved_key(id, key, k, charsmax(k))
	TrieGetString(g_lsgP5Txt, k, s, charsmax(s))
	return s
}

stock lsg_p5_text_set(id, const key[], const text[])
{
	if (g_lsgP5Txt == Invalid_Trie || !lsg_ok(id))
		return
	new k[96], v[64]
	lsg_saved_key(id, key, k, charsmax(k))
	lsg_p5_clean(text, v, charsmax(v))
	TrieSetString(g_lsgP5Txt, k, v)
	g_lsgP5TxtDirty = true
}

stock lsg_p5_stext_get(const key[])
{
	new s[64], k[96]
	formatex(k, charsmax(k), "%s@@server", key)
	if (g_lsgP5Txt != Invalid_Trie)
		TrieGetString(g_lsgP5Txt, k, s, charsmax(s))
	return s
}

stock lsg_p5_stext_set(const key[], const text[])
{
	if (g_lsgP5Txt == Invalid_Trie)
		return
	new k[96], v[64]
	formatex(k, charsmax(k), "%s@@server", key)
	lsg_p5_clean(text, v, charsmax(v))
	TrieSetString(g_lsgP5Txt, k, v)
	g_lsgP5TxtDirty = true
}

stock Float:lsg_p5_snum_get(const key[])
{
	new k[96], Float:v
	formatex(k, charsmax(k), "%s@@server", key)
	if (g_lsgSaved == Invalid_Trie || !TrieGetCell(g_lsgSaved, k, v))
		return 0.0
	return v
}

stock lsg_p5_snum_set(const key[], Float:value)
{
	if (g_lsgSaved == Invalid_Trie)
		return
	new k[96]
	formatex(k, charsmax(k), "%s@@server", key)
	TrieSetCell(g_lsgSaved, k, value)
	g_lsgSavedDirty = true
}

// a player's name for Top Players, kept by their SteamID (bots and LAN players are saved by name already)
public lsg_p5_seen(id)
{
	if (!is_user_alive(id) || is_user_bot(id) || g_lsgP5Txt == Invalid_Trie)
		return HAM_IGNORED
	new who[44], k[64], name[32], had[32]
	get_user_authid(id, who, charsmax(who))
	if (!equal(who, "STEAM_", 6))
		return HAM_IGNORED
	formatex(k, charsmax(k), "@nm@%s", who)
	get_user_name(id, name, charsmax(name))
	lsg_p5_clean(name, name, charsmax(name))
	if (!TrieGetString(g_lsgP5Txt, k, had, charsmax(had)) || !equal(had, name))
	{
		TrieSetString(g_lsgP5Txt, k, name)
		g_lsgP5TxtDirty = true
	}
	return HAM_IGNORED
}

// the best `most` players by a saved value ("key@<SteamID>" or "key@name:<name>"), into
// g_lsgP5TopName / g_lsgP5TopVal; how many there are
stock lsg_p5_top(const key[], most, bool:low)
{
	most = min(most, LSG_P5TOP)
	if (g_lsgSaved == Invalid_Trie)
		return 0
	new Snapshot:snap = TrieSnapshotCreate(g_lsgSaved), k[128], Float:v, count, klen = strlen(key)
	for (new i = 0; i < TrieSnapshotLength(snap); i++)
	{
		TrieSnapshotGetKey(snap, i, k, charsmax(k))
		// this key's, and a player's (server values are key@@server)
		if (!equal(k, key, klen) || k[klen] != '@' || k[klen + 1] == '@')
			continue
		TrieGetCell(g_lsgSaved, k, v)
		// where it goes in the list so far (the worst one drops off a full list)
		new at = count
		while (at > 0 && (low ? v < g_lsgP5TopVal[at - 1] : v > g_lsgP5TopVal[at - 1]))
			at--
		if (at >= most)
			continue
		for (new j = min(count, most - 1); j > at; j--)
		{
			g_lsgP5TopVal[j] = g_lsgP5TopVal[j - 1]
			copy(g_lsgP5TopName[j], 31, g_lsgP5TopName[j - 1])
		}
		g_lsgP5TopVal[at] = v
		new who[96]
		copy(who, charsmax(who), k[klen + 1])
		if (equal(who, "name:", 5))
			copy(g_lsgP5TopName[at], 31, who[5])
		else
		{
			new nk[110]
			formatex(nk, charsmax(nk), "@nm@%s", who)
			if (g_lsgP5Txt == Invalid_Trie || !TrieGetString(g_lsgP5Txt, nk, g_lsgP5TopName[at], 31))
				copy(g_lsgP5TopName[at], 31, who)
		}
		if (count < most)
			count++
	}
	TrieSnapshotDestroy(snap)
	return count
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

stock bool:g_lsgClockOn, bool:g_lsgClockPaused, bool:g_lsgClockInf, Float:g_lsgClockEnd, Float:g_lsgClockLeft

stock Float:lsg_clock_left()
{
	if (!g_lsgClockOn)
		return 0.0
	if (g_lsgClockPaused)
		return g_lsgClockLeft
	return floatmax(g_lsgClockEnd - get_gametime(), 0.0)
}

stock lsg_clock_set(Float:secs)
{
	// (rounds mustn't end on the game's own clock while ours runs: its "f" flag holds them)
	if (!g_lsgClockInf && !get_cvar_num("mp_round_infinite"))
	{
		new cur[16]
		get_cvar_string("mp_round_infinite", cur, charsmax(cur))
		if (!cur[0] || equal(cur, "0"))
		{
			set_cvar_string("mp_round_infinite", "f")
			g_lsgClockInf = true
		}
	}
	g_lsgClockOn = true
	g_lsgClockPaused = false
	g_lsgClockEnd = get_gametime() + floatmax(secs, 1.0)
	lsg_round_time(floatround(secs))
}

stock lsg_clock_add(Float:secs)
{
	if (!g_lsgClockOn)
		return
	if (g_lsgClockPaused)
		g_lsgClockLeft = floatmax(g_lsgClockLeft + secs, 0.0)
	else
		g_lsgClockEnd += secs
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_pause(pause)
{
	if (!g_lsgClockOn || bool:pause == g_lsgClockPaused)
		return
	if (pause)
	{
		g_lsgClockLeft = lsg_clock_left()
		g_lsgClockPaused = true
	}
	else
	{
		g_lsgClockEnd = get_gametime() + g_lsgClockLeft
		g_lsgClockPaused = false
	}
	lsg_round_time(floatround(lsg_clock_left()))
}

stock lsg_clock_off()
{
	g_lsgClockOn = false
	g_lsgClockPaused = false
	if (g_lsgClockInf)
	{
		set_cvar_string("mp_round_infinite", "0")
		g_lsgClockInf = false
	}
}

public lsg_clock_tick()
{
	if (!g_lsgClockOn)
		return
	// paused: the round timer on screen keeps counting by itself, so it's set again each second
	if (g_lsgClockPaused)
	{
		lsg_round_time(floatround(g_lsgClockLeft))
		return
	}
	if (lsg_clock_left() <= 0.0)
	{
		g_lsgClockOn = false
		lsg_time_up_hook()
	}
}

// fire, a drunk view and trails, ten times a second (one task for all of them)
public lsg_fun_tick()
	lsg_fun_ticks()

// View From with Freeze: no shooting while viewing (moving is stopped by FL_FROZEN)
public lsg_view_cmd(id, uc, seed)
{
	if (id < 1 || id > 32 || !g_lsgViewCam[id] || !g_lsgViewFreeze[id])
		return FMRES_IGNORED
	set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) & ~(IN_ATTACK | IN_ATTACK2 | IN_USE))
	return FMRES_HANDLED
}

// a death ends fire, burying, a drunk view, and third person when it's set to
public lsg_fun_dead(id)
	lsg_fun_died(id)

// a new round puts out fires, unburies and gives views back
public lsg_fun_round()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		lsg_burn_off(p)
		g_lsgBuried[p] = false
		lsg_view_back(p)
	}
}

public plugin_precache()
{
	g_lsgSprFire = lsg_pre_model("sprites/fire.spr")
	// AMX Mod X's third-person camera is this model
	lsg_pre_model("models/rpgrocket.mdl")
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Starter library test B", "1.0", "Game Mode Studio")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_hurt_before", 0)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	lsg_hook_weapons("lsg_on_fired", Ham_Weapon_PrimaryAttack)
	set_task(0.1, "lsg_fun_tick", LSG_TASK - 410, _, _, "b")
	register_forward(FM_CmdStart, "lsg_view_cmd", 0)
	lsg_p5_text_load()
	set_task(5.0, "lsg_p5_text_flush", LSG_TASK - 610, _, _, "b")
	RegisterHamPlayer(Ham_Spawn, "lsg_p5_seen", 1)
	RegisterHamPlayer(Ham_Killed, "lsg_fun_dead", 1)
	register_event("HLTV", "lsg_fun_round", "a", "1=0", "2=0")
	register_forward(FM_ClientDisconnect, "lsg_ec_left", 0)
	set_task(1.0, "lsg_clock_tick", LSG_TASK - 2, _, _, "b")
	set_task(5.0, "lsg_saved_flush", LSG_TASK - 3, _, _, "b")
	lsg_saved_load()
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
	lsg_clock_off()
	lsg_saved_flush()
}

public plugin_end()
{
	lsg_p5_text_flush()
	lsg_saved_flush()
	lsg_clock_off()
}

lsg_time_up_hook()
{
	new ctx[LSG_CTX]
	lsg_n68(ctx)
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv0[id] = 0
	g_pv1[id] = 3.0
	g_pv2[id] = 0.0
	copy(g_pv6[id], 63, "")
}

public client_disconnected(id)
{
	lsg_fun_left(id)
	lsg_saved_flush()
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n50(ctx)
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
	lsg_n59(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n56(ctx)
	return HAM_IGNORED
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
	lsg_n65(ctx)
	return HAM_IGNORED
}

public lsg_on_fired(wpn)
{
	static Float:last[33]
	new id = pev_valid(wpn) ? pev(wpn, pev_owner) : 0, wn[24]
	if (!lsg_ok(id) || get_gametime() - last[id] < 0.1)
		return HAM_IGNORED
	last[id] = get_gametime()
	pev(wpn, pev_classname, wn, charsmax(wn))
	replace(wn, charsmax(wn), "weapon_", "")
	new ctx[LSG_CTX]
	ctx[0] = id // player
	copy(g_lsgTxt[0], 63, wn) // weapon
	lsg_n57(ctx)
	return HAM_IGNORED
}

lsg_vol_begin_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	#pragma unused ctx
}

lsg_vol_end_hook(vol, who)
{
	new ctx[LSG_CTX]
	ctx[0] = vol // volume
	ctx[1] = who // who
	#pragma unused ctx
}

// Function Entry (sp_end_entry)
stock lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Branch (sp_end_br)
stock lsg_n1(ctx[])
{
	new ok = ((((g_pv0[lsg_slot(ctx[8])]) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n2(ctx)
	else
		lsg_n5(ctx)
}

// God Mode (sp_end_god)
stock lsg_n2(ctx[])
{
	lsg_godmode(ctx[8], 0)
	lsg_n3(ctx)
}

// Set Glow (sp_end_glow)
stock lsg_n3(ctx[])
{
	if (!(!lsg_ok(ctx[8])))
	{
		set_user_rendering(ctx[8])
	}
	lsg_n4(ctx)
}

// Set Variable (sp_end_off)
stock lsg_n4(ctx[])
{
	g_pv0[lsg_slot(ctx[8])] = 0
	lsg_n5(ctx)
}

// Return Node (sp_end_ret)
stock lsg_n5(ctx[])
{
	g_lsgFD0 = true
	#pragma unused ctx
}

// Function Entry (lv_reset_entry)
stock lsg_n6(ctx[])
{
	lsg_n7(ctx)
}

// Set Variable (lv_reset_set)
stock lsg_n7(ctx[])
{
	g_pv1[lsg_slot(ctx[9])] = (((((Float:ctx[10]) > (0.0) ? 1 : 0)) ? 1 : 0) ? (Float:ctx[10]) : (3.0))
	lsg_n8(ctx)
}

// Return Node (lv_reset_ret)
stock lsg_n8(ctx[])
{
	g_lsgFD1 = true
	#pragma unused ctx
}

// Function Entry (lv_lose_entry)
stock lsg_n9(ctx[])
{
	lsg_n10(ctx)
}

// Add To Variable (lv_lose_sub)
stock lsg_n10(ctx[])
{
	g_pv1[lsg_slot(ctx[11])] += -1.0
	lsg_n11(ctx)
}

// Show HUD Text (lv_lose_hud)
stock lsg_n11(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("Lives left: %s", lsg_ntext(g_pv1[lsg_slot(ctx[11])])))
	lsg_hud(ctx[11], 3, 3.0, lsgH)
	lsg_n12(ctx)
}

// Return Node (lv_lose_ret)
stock lsg_n12(ctx[])
{
	g_lsgR2_0 = g_pv1[lsg_slot(ctx[11])]
	g_lsgR2_1 = ((g_pv1[lsg_slot(ctx[11])]) <= (0.0) ? 1 : 0)
	g_lsgFD2 = true
}

// Function Entry (ks_kill_entry)
stock lsg_n13(ctx[])
{
	lsg_n14(ctx)
}

// Set Variable (ks_kill_reset)
stock lsg_n14(ctx[])
{
	g_pv2[lsg_slot(ctx[13])] = 0.0
	lsg_n15(ctx)
}

// Branch (ks_kill_br)
stock lsg_n15(ctx[])
{
	new ok = ((((((ctx[12]) == (ctx[13]) ? 0 : 1)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n16(ctx)
	else
		lsg_n18(ctx)
}

// Add To Variable (ks_kill_add)
stock lsg_n16(ctx[])
{
	g_pv2[lsg_slot(ctx[12])] += 1.0
	lsg_n17(ctx)
}

// Switch on Number (ks_kill_sw)
stock lsg_n17(ctx[])
{
	new Float:x = g_pv2[lsg_slot(ctx[12])]
	if (x == 3.0)
	{
		lsg_n19(ctx)
	}
	else if (x == 5.0)
	{
		lsg_n21(ctx)
	}
	else if (x == 7.0)
	{
		lsg_n23(ctx)
	}
	else if (x == 10.0)
	{
		lsg_n25(ctx)
	}
	else
	{
		lsg_n18(ctx)
	}
}

// Return Node (ks_kill_ret)
stock lsg_n18(ctx[])
{
	g_lsgR3_0 = g_pv2[lsg_slot(ctx[12])]
	g_lsgFD3 = true
}

// Show HUD Text (ks_kill_h3)
stock lsg_n19(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s is on a killing spree!", lsg_name(ctx[12])))
	lsg_hud(0, 0, 4.0, lsgH)
	lsg_n20(ctx)
}

// Play Sound (ks_kill_s3)
stock lsg_n20(ctx[])
{
	client_cmd(0, "spk %s", "buttons/bell1")
	lsg_n18(ctx)
}

// Show HUD Text (ks_kill_h5)
stock lsg_n21(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s is unstoppable!", lsg_name(ctx[12])))
	lsg_hud(0, 0, 4.0, lsgH)
	lsg_n22(ctx)
}

// Play Sound (ks_kill_s5)
stock lsg_n22(ctx[])
{
	client_cmd(0, "spk %s", "buttons/bell1")
	lsg_n18(ctx)
}

// Show HUD Text (ks_kill_h7)
stock lsg_n23(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s is godlike!", lsg_name(ctx[12])))
	lsg_hud(0, 0, 4.0, lsgH)
	lsg_n24(ctx)
}

// Play Sound (ks_kill_s7)
stock lsg_n24(ctx[])
{
	client_cmd(0, "spk %s", "buttons/bell1")
	lsg_n18(ctx)
}

// Show HUD Text (ks_kill_h10)
stock lsg_n25(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s is LEGENDARY!", lsg_name(ctx[12])))
	lsg_hud(0, 0, 4.0, lsgH)
	lsg_n26(ctx)
}

// Play Sound (ks_kill_s10)
stock lsg_n26(ctx[])
{
	client_cmd(0, "spk %s", "buttons/bell1")
	lsg_n18(ctx)
}

// Function Entry (lb_show_entry)
stock lsg_n27(ctx[])
{
	lsg_n28(ctx)
}

// Set Variable (lb_show_clear)
stock lsg_n28(ctx[])
{
	copy(g_v5, 63, "")
	lsg_n29(ctx)
}

// Top Players (lb_show_top)
stock lsg_n29(ctx[])
{
	new count = lsg_p5_top("kills", 10, false)
	for (new r = 0; r < count; r++)
	{
		g_lsgO[0] = _:float(r + 1)
		copy(g_lsgOT[0], 63, g_lsgP5TopName[r])
		g_lsgO[1] = _:g_lsgP5TopVal[r]
		lsg_n30(ctx)
	}
	lsg_n31(ctx)
}

// Set Variable (lb_show_add)
stock lsg_n30(ctx[])
{
	copy(g_v5, 63, fmt("%s%s. %s: %s[br]", g_v5, lsg_ntext(Float:g_lsgO[0]), g_lsgOT[0], lsg_ntext(Float:g_lsgO[1])))
	#pragma unused ctx
}

// Show Info Window (lb_show_win)
stock lsg_n31(ctx[])
{
	lsg_ec_motd(ctx[14], "Top players", "", g_v5)
	lsg_n32(ctx)
}

// Return Node (lb_show_ret)
stock lsg_n32(ctx[])
{
	g_lsgFD4 = true
	#pragma unused ctx
}

// Function Entry (cz_make_entry)
stock lsg_n33(ctx[])
{
	lsg_n34(ctx)
}

// Create Component (cz_make_make)
stock lsg_n34(ctx[])
{
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 34
	if (!lsgRe)
		g_lsgXrN[34] = 0
	g_lsgXrIn = -1
	if (!(ls_components_running()))
		lsgWhy = 6
	else if (!(lsg_ent_room("Create Component")))
		lsgWhy = 2
	else if (!(lsg_ent_origin(ctx[15], o)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, 1, 2, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_vis_note(ls_comp_create("ls_capture_point", o, "time=10|radius=160"))))
		lsgWhy = 1
	new bool:lsgWait = false
	g_lsgOut[34] = e
	g_lsgO[2] = e
	g_lsgO[3] = e ? 1 : 0
	g_lsgO[4] = e ? lsgMv : 0
	g_lsgO[5] = e ? 0 : lsgWhy
	if (!lsgRe)
		lsg_n35(ctx)
	#pragma unused lsgWait
}

// Bots: Goal (cz_make_bots)
stock lsg_n35(ctx[])
{
	if (!(!ls_components_running()))
	{
		if (!lsg_hint(fmt("goal %s capture %s %d %d %d %d %d", lsg_who(0, 0, ""), lsg_ptext(g_lsgO[2]), g_lsgO[2], clamp(floatround(60.0), 0, 100), max(floatround(3600.0), 1), 160, clamp(floatround(50.0), 0, 100))))
			{}
	}
	lsg_n36(ctx)
}

// Return Node (cz_make_ret)
stock lsg_n36(ctx[])
{
	g_lsgR5_0 = g_lsgO[2]
	g_lsgFD5 = true
	#pragma unused ctx
}

// Function Entry (qs_set_entry)
stock lsg_n37(ctx[])
{
	lsg_n38(ctx)
}

// Set Variable (qs_set_set)
stock lsg_n38(ctx[])
{
	copy(g_pv6[lsg_slot(ctx[16])], 63, g_lsgOT[1])
	lsg_n39(ctx)
}

// Branch (qs_set_br)
stock lsg_n39(ctx[])
{
	new ok = (((((equali(g_lsgOT[2], "") ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n42(ctx)
	else
		lsg_n40(ctx)
}

// Show HUD Text (qs_set_hud)
stock lsg_n40(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("Objective: %s", g_lsgOT[2]))
	lsg_hud(ctx[16], 1, 6.0, lsgH)
	lsg_n41(ctx)
}

// Play Sound To Player (qs_set_snd)
stock lsg_n41(ctx[])
{
	lsg_sound_to(ctx[16], "buttons/blip1.wav")
	lsg_n42(ctx)
}

// Return Node (qs_set_ret)
stock lsg_n42(ctx[])
{
	g_lsgFD6 = true
	#pragma unused ctx
}

// Function Entry (qs_of_entry)
stock lsg_n43(ctx[])
{
	lsg_n44(ctx)
}

// Return Node (qs_of_ret)
stock lsg_n44(ctx[])
{
	copy(g_lsgR7_0, 127, g_pv6[lsg_slot(ctx[17])])
	g_lsgR7_1 = (((equali(g_pv6[lsg_slot(ctx[17])], "") ? 1 : 0)) ? 0 : 1)
	g_lsgFD7 = true
}

// Function Entry (qs_done_entry)
stock lsg_n45(ctx[])
{
	lsg_n46(ctx)
}

// Set Variable (qs_done_set)
stock lsg_n46(ctx[])
{
	copy(g_pv6[lsg_slot(ctx[18])], 63, "done")
	lsg_n47(ctx)
}

// Give Money (qs_done_money)
stock lsg_n47(ctx[])
{
	if (!(!lsg_ok(ctx[18])))
	{
		lsg_add_money(ctx[18], floatround(Float:ctx[19]))
	}
	lsg_n48(ctx)
}

// Show HUD Text (qs_done_hud)
stock lsg_n48(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s", "Quest complete!")
	lsg_hud(ctx[18], 3, 4.0, lsgH)
	lsg_n49(ctx)
}

// Return Node (qs_done_ret)
stock lsg_n49(ctx[])
{
	g_lsgFD8 = true
	#pragma unused ctx
}

// Round Started (ev1)
lsg_n50(ctx[])
{
	lsg_n51(ctx)
}

// For Each Player (ev2)
lsg_n51(ctx[])
{
	new lsgK = 1, lsgA = 0, lsgB = 0
	g_lsgStepLim = 20000
	lsg_n51_r(ctx, lsgK, lsgA, lsgB, 0, 0)
}

stock lsg_n51_r(ctx[], k, a, b, n, own)
{
	new c[LSG_CTX]
	while (k <= MaxClients && n < 512)
	{
		if (lsg_loop_full())
		{
			lsg_loop_save(51, ctx, k, a, b, n)
			return
		}
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		new p = k++
		if (!(lsg_match(p, 0)))
			continue
		c[20] = p
		n++
		g_lsgYield = false
		lsg_n52(c)
		if (g_lsgYield)
		{
			lsg_loop_save(51, ctx, k, a, b, n)
			return
		}
	}
	#pragma unused a
	#pragma unused b
	#pragma unused own
	#pragma unused n
	lsg_n54(ctx)
}

// Call (ev4)
lsg_n52(ctx[])
{
	lsg_fr1(ctx[20], 3.0)
	lsg_n53(ctx)
}

// Call (ev7)
lsg_n53(ctx[])
{
	lsg_fr6(ctx[20], "survive", "Survive the round")
}

// Pick Random Entity (ev8)
lsg_n54(ctx[])
{
	new list[256], count
	new p = -1
	while (count < 256 && (p = find_ent_by_class(p, "info_player_deathmatch")) > 0)
		list[count++] = p
	if (count > 0)
	{
		ctx[21] = list[random(count)]
		lsg_n55(ctx)
	}
}

// Call (ev9)
lsg_n55(ctx[])
{
	lsg_fr5(ctx[21])
	g_lsgO[6] = _:g_lsgR5_0
	lsg_n70(ctx)
}

// Player Spawned (ev13)
lsg_n56(ctx[])
{
	lsg_n77(ctx)
}

// Weapon Fired (ev16)
lsg_n57(ctx[])
{
	lsg_n58(ctx)
}

// Call (ev17)
lsg_n58(ctx[])
{
	lsg_fr0(ctx[0])
}

// Player Killed (ev18)
lsg_n59(ctx[])
{
	lsg_n60(ctx)
}

// Call (ev19)
lsg_n60(ctx[])
{
	lsg_fr3(ctx[1], ctx[0])
	g_lsgO[7] = _:g_lsgR3_0
	lsg_n61(ctx)
}

// Call (ev20)
lsg_n61(ctx[])
{
	lsg_fr2(ctx[0])
	g_lsgO[8] = _:g_lsgR2_0
	g_lsgO[9] = _:g_lsgR2_1
	lsg_n83(ctx)
}

// Call (ev24)
lsg_n62(ctx[])
{
	lsg_fr8(ctx[1], 100.0)
	lsg_n63(ctx)
}

// Add To Saved (ev25)
lsg_n63(ctx[])
{
	if (!(!lsg_ok(ctx[1]) || g_lsgSaved == Invalid_Trie))
	{
		lsg_saved_set(ctx[1], "kills", lsg_saved_get(ctx[1], "kills") + (1.0))
	}
	lsg_n64(ctx)
}

// Call (ev26)
lsg_n64(ctx[])
{
	lsg_fr4(ctx[0])
}

// Player Hurt (ev27)
lsg_n65(ctx[])
{
	lsg_n66(ctx)
}

// Branch (ev28)
lsg_n66(ctx[])
{
	new ok = ((((ctx[3]) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n85(ctx)
	else
		lsg_n91(ctx)
}

// Branch (ev35)
lsg_n67(ctx[])
{
	new ok = (((((equali(g_lsgTxt[1], "knife") ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n95(ctx)
}

// Round Time Up (ev39)
lsg_n68(ctx[])
{
	lsg_n69(ctx)
}

// Pick Random Player (ev40)
lsg_n69(ctx[])
{
	new list[256], count
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, 1))
			list[count++] = p
	if (count > 0)
	{
		ctx[22] = list[random(count)]
		lsg_n97(ctx)
	}
}

// Set Variable (ev12)
lsg_n70(ctx[])
{
	copy(g_v4, 63, "Round")
	lsg_n71(ctx)
}

// Set Variable (ev12)
lsg_n71(ctx[])
{
	g_v3 = (((((120.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (120.0) : (60.0))
	lsg_n72(ctx)
}

// Repeat (ev12)
lsg_n72(ctx[])
{
	remove_task(LSG_TASK + 72)
	g_lsgLeft[72] = 0
	set_task(floatmax(1.0, 0.1), "lsg_t72", LSG_TASK + 72, ctx, LSG_CTX)
}

public lsg_t72(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	if (g_lsgLeft[72] > 0 && --g_lsgLeft[72] == 0)
		remove_task(LSG_TASK + 72)
	else
		set_task(floatmax(1.0, 0.1), "lsg_t72", LSG_TASK + 72, data, LSG_CTX)
	lsg_n73(ctx)
}

// Big HUD Text (ev12)
lsg_n73(ctx[])
{
	if (lsg_msg_room("Big HUD Text", 1, 0, 48, 160))
	{
		set_dhudmessage(255, 255, 255, -1.0, 0.12, 0, 0.0, 1.1, 0.2, 0.5)
		show_dhudmessage(0, "%s %s", "", fmt("%s: %s", lsg_tsel((((equali(g_v4, "") ? 1 : 0)) ? 1 : 0), "Time left", g_v4), lsg_w1_time(g_v3, 1)))
	}
	lsg_n74(ctx)
}

// Add To Variable (ev12)
lsg_n74(ctx[])
{
	g_v3 += -1.0
	lsg_n75(ctx)
}

// Branch (ev12)
lsg_n75(ctx[])
{
	new ok = ((((((g_v3) < (0.0) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n76(ctx)
}

// Clear Timer (ev12)
lsg_n76(ctx[])
{
	lsg_tclear((LSG_TASK + 72))
	#pragma unused ctx
}

// God Mode (ev15)
lsg_n77(ctx[])
{
	lsg_godmode(ctx[0], 1)
	lsg_n78(ctx)
}

// Set Glow (ev15)
lsg_n78(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		set_user_rendering(ctx[0], kRenderFxGlowShell, 255, 255, 255, kRenderNormal, 16)
	}
	lsg_n79(ctx)
}

// Set Variable (ev15)
lsg_n79(ctx[])
{
	g_pv0[lsg_slot(ctx[0])] = 1
	lsg_n80(ctx)
}

// Sequence (ev15)
lsg_n80(ctx[])
{
	lsg_n81(ctx)
}

// Delay (ev15)
lsg_n81(ctx[])
{
	set_task(floatmax((((((0.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (0.0) : (3.0)), 0.1), "lsg_t81", LSG_TASK + 81, ctx, LSG_CTX)
}

public lsg_t81(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n82(ctx)
}

// Call (ev15)
lsg_n82(ctx[])
{
	lsg_fr0(ctx[0])
}

// Branch (ev22)
lsg_n83(ctx[])
{
	new ok = ((((((((ctx[0]) == (ctx[1]) ? 0 : 1)) && ((ls_alive(ctx[1]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n84(ctx)
	else
		lsg_n62(ctx)
}

// View From (ev22)
lsg_n84(ctx[])
{
	new Float:lsgA[3], Float:lsgAa[3], Float:lsgB[3], Float:lsgBa[3], Float:lsgL[3]
	lsg_vto(lsg_vplayer(ctx[0], 1), lsgA)
	lsg_vto(lsg_vplayer(ctx[1], 1), lsgB)
	lsg_view_look(0, lsgL, 0, 0, 0)
	lsg_view_from(0, ctx[0], 0, lsgA, lsgAa, 1, lsgB, lsgBa, 1, 96.0, 2.0, 3.0, 0)
	lsg_n62(ctx)
}

// Freeze (ev30)
lsg_n85(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		lsg_freeze(ctx[0], 1)
	}
	lsg_n86(ctx)
}

// Set Glow (ev30)
lsg_n86(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		set_user_rendering(ctx[0], kRenderFxGlowShell, 0, 80, 255, kRenderNormal, 16)
	}
	lsg_n87(ctx)
}

// Sequence (ev30)
lsg_n87(ctx[])
{
	lsg_n88(ctx)
}

// Delay (ev30)
lsg_n88(ctx[])
{
	set_task(floatmax((((((1.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (1.0) : (2.0)), 0.1), "lsg_t88", LSG_TASK + 88, ctx, LSG_CTX)
}

public lsg_t88(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n89(ctx)
}

// Freeze (ev30)
lsg_n89(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		lsg_freeze(ctx[0], 0)
	}
	lsg_n90(ctx)
}

// Set Glow (ev30)
lsg_n90(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		set_user_rendering(ctx[0])
	}
}

// Set Speed (ev33)
lsg_n91(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		set_user_maxspeed(ctx[0], (((((200.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (200.0) : (150.0)))
	}
	lsg_n92(ctx)
}

// Sequence (ev33)
lsg_n92(ctx[])
{
	lsg_n67(ctx)
	lsg_n93(ctx)
}

// Delay (ev33)
lsg_n93(ctx[])
{
	set_task(floatmax((((((1.0) > (0.0) ? 1 : 0)) ? 1 : 0) ? (1.0) : (4.0)), 0.1), "lsg_t93", LSG_TASK + 93, ctx, LSG_CTX)
}

public lsg_t93(const data[], task)
{
	#pragma unused task
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	lsg_n94(ctx)
}

// Set Speed (ev33)
lsg_n94(ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		set_user_maxspeed(ctx[0], 250.0)
	}
}

// Set On Fire (ev38)
lsg_n95(ctx[])
{
	lsg_burn(ctx[0], ctx[1], 4.0, 5.0)
	lsg_n96(ctx)
}

// Bots Avoid Area (ev38)
lsg_n96(ctx[])
{
	if (!(!ls_components_running() || !lsg_avoid_room("burning", 16)))
	{
		ls_bots_avoid_ent("burning", ctx[0], 110.0, 5.0, 4.0, 0)
	}
}

// View From (ev42)
lsg_n97(ctx[])
{
	new Float:lsgA[3], Float:lsgAa[3], Float:lsgB[3], Float:lsgBa[3], Float:lsgL[3]
	lsg_vto(lsg_vadd(lsg_vplayer(ctx[22], 1), lsg_vec(0.0, 0.0, 64.0), 1.0), lsgA)
	lsg_vto(lsg_vplayer(ctx[22], 1), lsgB)
	lsg_view_look(0, lsgL, 0, 0, 0)
	lsg_view_from(3, 0, 0, lsgA, lsgAa, 1, lsgB, lsgBa, 1, 128.0, 3.0, 5.0, 1)
	lsg_n98(ctx)
}

// Show HUD Text (ev42)
lsg_n98(ctx[])
{
	new lsgH[128]
	formatex(lsgH, charsmax(lsgH), "%s %s", "", fmt("%s wins!", lsg_name(ctx[22])))
	lsg_hud(0, 3, 5.0, lsgH)
}

stock lsg_loop_go(node, ctx[], k, a, b, n)
{
	switch (node)
	{
		case 51: lsg_n51_r(ctx, k, a, b, n, 1)
	}
}

