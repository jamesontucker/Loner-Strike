/*
	Legacy wires - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from legacy_wires.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "legacy_wires"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 15
#define LSG_TASK 7310000
#pragma dynamic 16384
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
#define LSG_O 5
#define LSG_OT 1

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

public lsg_ed_freed(ent)
{
	lsg_ed_window()
	g_lsgEdFreed++
}

// (frees counted in half-second windows: this one and the one before)
stock lsg_ed_window()
{
	new w = floatround(get_gametime() * 2.0, floatround_floor)
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

stock bool:lsg_ent_room(const what[])
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
	new most = global_get(glb_maxEntities)
	if (g_lsgEdLive + g_lsgEdMade + g_lsgEdFreed + g_lsgEdFreedBefore < most - max(LSG_EDICT_RESERVE, most / 8))
	{
		g_lsgEdMade++
		return true
	}
	lsg_skip_log("slots", what, "%s skipped: the server is out of entity slots", what)
	return false
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


// ---- where a hit struck ----
stock Float:g_lsgHitPos[2048][3], Float:g_lsgHitDir[2048][3], Float:g_lsgHitAt[2048]

public lsg_hit_trace(victim, attacker, Float:damage, Float:dir[3], tr, bits)
{
	if (victim < 1 || victim >= 2048)
		return HAM_IGNORED
	get_tr2(tr, TR_vecEndPos, g_lsgHitPos[victim])
	g_lsgHitDir[victim] = dir
	g_lsgHitAt[victim] = get_gametime()
	return HAM_IGNORED
}

stock lsg_hit_pos(v)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vfrom(g_lsgHitPos[v])
	return lsg_ecenter(v)
}

stock lsg_hit_dir(v, from)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vnorm(lsg_vfrom(g_lsgHitDir[v]))
	if (from > 0 && pev_valid(from) && from != v)
		return lsg_vdir(lsg_vent(from), lsg_vent(v))
	return lsg_vec(0.0, 0.0, 0.0)
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
	if (!lsg_ent_room("Loop Sound"))
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

// ---- creature control ----
stock Float:g_lsgCrPos[2048][3], Float:g_lsgCrStill[2048], g_lsgCrSerial[2048], g_lsgCrSeen[2048]
stock bool:g_lsgCrGoalOn[2048], Float:g_lsgCrGoal[2048][3]

// alive, not friendly (a turret), of the type (empty: any)
stock bool:lsg_cr_match(e, const type[])
{
	if (!lsg_is_creature(e) || pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
		return false
	new Float:hp
	pev(e, pev_health, hp)
	if (hp <= 0.0)
		return false
	if (!type[0])
		return true
	new t[24]
	pev(e, pev_netname, t, charsmax(t))
	return bool:equali(t, type)
}

stock lsg_cr_cmd(e, const fmt[])
{
	if (lsg_cr_match(e, "") && get_cvar_pointer("ls_creature_ver"))
		server_cmd(fmt, e)
}

stock lsg_cr_all(const type[])
{
	new a = lsg_atmp(), e = -1, n
	while (n < 512 && (e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (!lsg_cr_match(e, type))
			continue
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_cr_nearest(const type[], pos, to)
{
	new Float:at[3]
	if (pos >= 0)
		lsg_vto(pos, at)
	else if (to > 0 && pev_valid(to))
		pev(to, pev_origin, at)
	else
		return 0
	new e = -1, best, Float:bd = 999999.0, Float:o[3]
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e == to || !lsg_cr_match(e, type))
			continue
		pev(e, pev_origin, o)
		new Float:d = get_distance_f(o, at)
		if (d < bd)
		{
			bd = d
			best = e
		}
	}
	return best
}

stock lsg_cr_type(e)
{
	new t[24]
	if (lsg_is_creature(e))
		pev(e, pev_netname, t, charsmax(t))
	return t
}

stock Float:lsg_cr_idle(e)
{
	if (e <= 0 || e >= 2048 || !lsg_cr_match(e, "") || g_lsgCrSeen[e] != g_lsgCrSerial[e] || g_lsgCrStill[e] <= 0.0)
		return 0.0
	return get_gametime() - g_lsgCrStill[e]
}

// a clear line from its eyes to the thing's middle (or the thing itself in the way)
stock bool:lsg_cr_sees(e, what)
{
	if (!lsg_cr_match(e, "") || what <= 0 || !pev_valid(what))
		return false
	new Float:a[3], Float:v[3], Float:b[3]
	pev(e, pev_origin, a)
	pev(e, pev_view_ofs, v)
	a[2] += v[2] > 0.0 ? v[2] : 24.0
	pev(what, pev_origin, b)
	if (what <= MaxClients)
	{
		pev(what, pev_view_ofs, v)
		b[2] += v[2]
	}
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, e, tr)
	new Float:f
	get_tr2(tr, TR_flFraction, f)
	new hit = get_tr2(tr, TR_pHit)
	free_tr2(tr)
	return f >= 1.0 || hit == what
}

stock bool:lsg_cr_sees_any(e)
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && lsg_cr_sees(e, p))
			return true
	return false
}

// how: 0 guard there, 1 then resume hunting, 2 flee there
stock lsg_cr_goto(e, to, pos, const name[], how)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:t[3]
	if (pos >= 0)
		lsg_vto(pos, t)
	else
	{
		if (to <= 0 && name[0])
			to = find_ent_by_tname(-1, name)
		if (to <= 0 || !pev_valid(to))
			return
		// (a brush entity, a gate: its middle)
		new Float:mn[3], Float:mx[3]
		pev(to, pev_absmin, mn)
		pev(to, pev_absmax, mx)
		for (new k = 0; k < 3; k++)
			t[k] = (mn[k] + mx[k]) / 2.0
	}
	server_cmd("ls_creature behave %d %s %.0f %.0f %.0f;ls_creature target %d 0", e, how == 2 ? "flee" : "guard", t[0], t[1], t[2], e)
	g_lsgCrGoalOn[e] = how == 1
	g_lsgCrGoal[e] = t
	g_lsgCrSerial[e]++
}

stock lsg_cr_attack(e, t)
{
	if (!lsg_cr_match(e, "") || t <= 0 || !pev_valid(t) || !get_cvar_pointer("ls_creature_ver"))
		return
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature target %d %d;ls_creature behave %d chase", e, t, e)
}

// stays where it is (guarding its spot: it still fights what comes close, then comes back)
stock lsg_cr_hold(e)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:o[3]
	pev(e, pev_origin, o)
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature behave %d guard %.0f %.0f %.0f;ls_creature target %d 0", e, o[0], o[1], o[2], e)
}

// creature control 2 in the bot brain: state, target, default behavior, speed, tier
stock bool:lsg_cr_v2()
{
	return get_cvar_pointer("ls_creature_ver") && get_cvar_num("ls_creature_ver") >= 2
}

// back to its own AI: its type's spawn behavior, its spawn spot as home, no forced target
// (an older brain has no "default": it hunts)
stock lsg_cr_resume(e)
{
	if (e > 0 && e < 2048)
		g_lsgCrGoalOn[e] = false
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	if (lsg_cr_v2())
		server_cmd("ls_creature behave %d default", e)
	else
		server_cmd("ls_creature behave %d chase;ls_creature target %d 0", e, e)
}

// 1 idle, 2 wander, 3 patrol, 4 guard, 5 chase, 6 flee, 7 attack, 8 search, 9 dead; 0 not a creature
stock lsg_cr_state(e)
{
	if (!lsg_cr_v2() || !lsg_is_creature(e))
		return 0
	new s = pev(e, pev_iuser3)
	return s >= 1 && s <= 9 ? s : 0
}

stock lsg_cr_state_name(e)
{
	static const names[][] = { "", "idle", "wander", "patrol", "guard", "chase", "flee", "attack", "search", "dead" }
	new t[8]
	copy(t, charsmax(t), names[lsg_cr_state(e)])
	return t
}

stock lsg_cr_target(e)
{
	if (!lsg_cr_v2() || !lsg_cr_match(e, ""))
		return 0
	new t = pev(e, pev_enemy)
	return t > 0 && pev_valid(t) ? t : 0
}

stock lsg_cr_speed(e, Float:x)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature speed %d %.2f", e, floatclamp(x, 0.2, 3.0))
}

stock lsg_cr_tier(e, t)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature tier %d %d", e, clamp(t, 0, 3))
}

stock lsg_cr_health(e, Float:hp)
{
	if (!lsg_cr_match(e, ""))
		return
	hp = floatclamp(hp, 1.0, 100000.0)
	set_pev(e, pev_health, hp)
	set_pev(e, pev_max_health, hp)
}


// once a second: how long each creature has stood still; Go To's "then resume"; Creature Idle
public lsg_cr_tick()
{
	new Float:now = get_gametime(), e = -1, Float:o[3]
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e >= 2048 || !lsg_cr_match(e, ""))
			continue
		pev(e, pev_origin, o)
		if (g_lsgCrSeen[e] != g_lsgCrSerial[e] || get_distance_f(o, g_lsgCrPos[e]) > 32.0)
		{
			g_lsgCrPos[e] = o
			g_lsgCrStill[e] = now
			g_lsgCrSeen[e] = g_lsgCrSerial[e]
		}
		if (g_lsgCrGoalOn[e] && get_distance_f(o, g_lsgCrGoal[e]) < 96.0)
		{
			g_lsgCrGoalOn[e] = false
			lsg_cr_resume(e)
		}
	}
}

// Objective Markers follow what they're on, ten times a second (one task for all of them)
public lsg_mk_tick()
	lsg_mk_follow()

public plugin_precache()
{
	lsg_pre_model("models/custom/crate1.mdl")
	lsg_pre_model("sprites/custom/arrow.spr")
	lsg_pre_sound("custom/boo.wav")
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Legacy wires", "1.0", "Game Mode Studio")
	RegisterHamPlayer(Ham_TakeDamage, "lsg_hurt_before", 0)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	RegisterHamPlayer(Ham_TraceAttack, "lsg_hit_trace", 0)
	set_task(1.0, "lsg_cr_tick", LSG_TASK + 60000, _, _, "b")
	set_task(0.1, "lsg_mk_tick", LSG_TASK - 400, _, _, "b")
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
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
	ctx[5] = lsg_hit_pos(victim) // hit_pos
	ctx[6] = lsg_hit_dir(victim, attacker >= 1 && attacker <= MaxClients ? attacker : inflictor) // hit_dir
	lsg_n0(ctx)
	return HAM_IGNORED
}

public ls_on_proj_hit(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	#pragma unused pos
	#pragma unused normal
	new ctx[LSG_CTX]
	ctx[0] = proj // proj
	ctx[1] = owner // owner
	ctx[2] = lsg_vfrom(pos) // pos
	ctx[3] = lsg_vfrom(normal) // normal
	ctx[4] = hit // hit
	ctx[5] = (hit == 0 ? 1 : 0) // world
	ctx[6] = _:float(hitgroup) // hitgroup
	lsg_n13(ctx)
}

// Player Hurt (hurt)
lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Print String (pw)
lsg_n1(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "hit in", g_lsgTxt[0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n2(ctx)
}

// Print String (pw2)
lsg_n2(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "with", g_lsgTxt[1])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n3(ctx)
}

// Give Weapon (give)
lsg_n3(ctx[])
{
	lsg_give_named(ctx[1], "m4a1", 30)
	lsg_n4(ctx)
}

// Print String (ph)
lsg_n4(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "holding", lsg_held(ctx[1]))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n5(ctx)
}

// Effect Burst (burst)
lsg_n5(ctx[])
{
	new Float:lsgAt[3], lsgOk
	lsgOk = lsg_ent_origin(ctx[0], lsgAt)
	if (lsgOk && !lsg_msg_room("Effect Burst", 2, 0, 48))
		lsgOk = 0
	if (lsgOk)
		lsg_burst(0, lsgAt, 2, 10, 200, 30)
	lsg_n6(ctx)
}

// Spawn Prop (prop)
lsg_n6(ctx[])
{
	new Float:o[3], e = 0, lsgMv = 0, lsgWhy = 0, lsgRe = g_lsgXrIn == 6
	if (!lsgRe)
		g_lsgXrN[6] = 0
	g_lsgXrIn = -1
	if (!(lsg_have("models/custom/crate1.mdl")))
		lsgWhy = 4
	else if (!(lsg_ent_room("Spawn Prop")))
		lsgWhy = 2
	else if (!(lsg_ent_origin(ctx[0], o)))
		lsgWhy = 6
	else if (!lsg_xf_free(o, 1, 2, 128.0, lsgMv))
		lsgWhy = 1
	else if (!(e = lsg_prop_o(o, "models/custom/crate1.mdl", 1)))
		lsgWhy = 6
	new bool:lsgWait = false
	g_lsgOut[6] = e
	g_lsgO[0] = e
	g_lsgO[1] = e ? 1 : 0
	g_lsgO[2] = e ? lsgMv : 0
	g_lsgO[3] = e ? 0 : lsgWhy
	if (!lsgRe)
		lsg_n7(ctx)
	#pragma unused lsgWait
}

// Play Sound At (snd)
lsg_n7(ctx[])
{
	lsg_sound_at(ctx[5], "custom/boo.wav")
	lsg_n8(ctx)
}

// Objective Marker (mk)
lsg_n8(ctx[])
{
	new Float:lsgAt[3]
	g_lsgOut[8] = lsg_marker("sprites/custom/arrow.spr", g_lsgO[0], lsgAt, 0, 48.0, 0.5, 255, 200, 0, 1, 0, 0, 32)
	lsg_n9(ctx)
}

// Set Glow (glow)
lsg_n9(ctx[])
{
	if (!(!lsg_ok(ctx[0])))
	{
		set_user_rendering(ctx[0], kRenderFxGlowShell, 0, 80, 255, kRenderNormal, 16)
	}
	lsg_n10(ctx)
}

// As Creature (as)
lsg_n10(ctx[])
{
	new lsgE = g_lsgO[0]
	if (lsg_is_creature(lsgE))
	{
		g_lsgO[4] = lsgE
		copy(g_lsgOT[0], 127, lsg_cr_type(lsgE))
		lsg_n11(ctx)
	}
	else
	{
		g_lsgO[4] = 0
	}
}

// Creature Go To (go)
lsg_n11(ctx[])
{
	lsg_cr_goto(g_lsgO[4], g_lsgO[0], -1, "gate1", 0)
	lsg_n12(ctx)
}

// Set Transform (stf)
lsg_n12(ctx[])
{
	lsg_set_transform(ctx[0], lsg_vpev(ctx[0], pev_origin), lsg_vpev(ctx[0], pev_v_angle), -1)
}

// Projectile Hit (hitp)
lsg_n13(ctx[])
{
	lsg_n14(ctx)
}

// Print String (pz)
lsg_n14(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "zone", lsg_ntext(Float:ctx[6]))
	server_print("[%s] %s", LSG_MODE, t)
}

