/*
	Compat - made with Loner-Strike's Game Mode Studio (graph version 4).
	Generated from c_st_leaderboard.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "c_st_leaderboard"
#define LSG_CTX 9
#define LSG_TXT 2
#define LSG_N 6
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
#define LSG_O 2
#define LSG_OT 1

// The mode's variables
stock g_v0[64] = "" // Leaderboard text (text)

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
// Function: Show Leaderboard
stock bool:g_lsgFD0
stock lsg_fr0(a0)
{
	new ctx[LSG_CTX]
	ctx[8] = _:a0
	g_lsgFD0 = false
	lsg_n0(ctx)
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

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Compat", "1.0", "Game Mode Studio")
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	lsg_p5_text_load()
	set_task(5.0, "lsg_p5_text_flush", LSG_TASK - 610, _, _, "b")
	RegisterHamPlayer(Ham_Spawn, "lsg_p5_seen", 1)
	register_forward(FM_ClientDisconnect, "lsg_ec_left", 0)
	set_task(5.0, "lsg_saved_flush", LSG_TASK - 3, _, _, "b")
	lsg_saved_load()
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
	lsg_saved_flush()
}

public plugin_end()
{
	lsg_p5_text_flush()
	lsg_saved_flush()
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_disconnected(id)
{
	lsg_saved_flush()
}

// Function Entry (lb_show_entry)
stock lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Set Variable (lb_show_clear)
stock lsg_n1(ctx[])
{
	copy(g_v0, 63, "")
	lsg_n2(ctx)
}

// Top Players (lb_show_top)
stock lsg_n2(ctx[])
{
	new count = lsg_p5_top("kills", 10, false)
	for (new r = 0; r < count; r++)
	{
		g_lsgO[0] = _:float(r + 1)
		copy(g_lsgOT[0], 63, g_lsgP5TopName[r])
		g_lsgO[1] = _:g_lsgP5TopVal[r]
		lsg_n3(ctx)
	}
	lsg_n4(ctx)
}

// Set Variable (lb_show_add)
stock lsg_n3(ctx[])
{
	copy(g_v0, 63, fmt("%s%s. %s: %s[br]", g_v0, lsg_ntext(Float:g_lsgO[0]), g_lsgOT[0], lsg_ntext(Float:g_lsgO[1])))
	#pragma unused ctx
}

// Show Info Window (lb_show_win)
stock lsg_n4(ctx[])
{
	lsg_ec_motd(ctx[8], "Top players", "", g_v0)
	lsg_n5(ctx)
}

// Return Node (lb_show_ret)
stock lsg_n5(ctx[])
{
	g_lsgFD0 = true
	#pragma unused ctx
}

