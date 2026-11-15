/*
	Every Node W1 - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from every_node_w1.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "every_node_w1"
#define LSG_CTX 8
#define LSG_TXT 2
#define LSG_N 90
#define LSG_TASK 7310000
#pragma dynamic 61696
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
#define LSG_O 3
#define LSG_OT 1

// The mode's variables
stock g_v0 = 0 // Flag (bool)
stock g_pv1[33] // Player flag (bool, per player)
stock g_tv2[4] = { 1, 1, 1, 1 } // Team flag (bool, per team)

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

// room for more messages this server frame (temp entities, HUD, chat): past the budget (the
// Effect messages a frame limit) the effect is skipped, said in the log once a second; a unicast
// (to a player) counts against that player too, at half the budget
stock Float:g_lsgMsgAt, g_lsgMsgN, g_lsgMsgP[33], Float:g_lsgMsgSaid, g_lsgMsgLim = 48
stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0)
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
	{
		if (now - g_lsgMsgSaid >= 1.0)
		{
			g_lsgMsgSaid = now
			log_amx("[%s] %s skipped: too many effects this frame", LSG_MODE, what)
		}
		return false
	}
	g_lsgMsgN += cost
	if (to >= 1 && to <= 32)
		g_lsgMsgP[to] += cost
	return true
}

// room for one more entity: free slots are kept for the game (it crashes with "no free edicts");
// a mode's spawn that would eat into them is skipped, and the server log says what and why
#define LSG_EDICT_RESERVE 96
stock bool:lsg_ent_room(const what[])
{
	if (engfunc(EngFunc_NumberOfEntities) < global_get(glb_maxEntities) - LSG_EDICT_RESERVE)
		return true
	log_amx("[%s] %s skipped: the server is out of entity slots", LSG_MODE, what)
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
	formatex(raw, charsmax(raw), fmt, floatabs(v))
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
stock lsg_mka6(Float:p0, Float:p1)
{
	new a = lsg_atmp()
	lsg_apushc(a, _:(p0))
	lsg_apushc(a, _:(p1))
	return a
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node W1", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	register_forward(FM_StartFrame, "lsg_w1_frame")
}

public lsg_w1_frame()
{
	static Float:at[2]
	new Float:now = get_gametime()
	if (now < at[0] || now - at[0] >= floatmax(0.1, 1.0 / float(100)))
	{
		new ctx[LSG_CTX]
		ctx[0] = _:(at[0] > 0.0 && now > at[0] ? now - at[0] : floatmax(0.1, 1.0 / float(100)))
		at[0] = now
		lsg_n85(ctx)
	}
	if (now < at[1] || now - at[1] >= floatmax(0.02, 1.0 / float(100)))
	{
		new ctx[LSG_CTX]
		ctx[0] = _:(at[1] > 0.0 && now > at[1] ? now - at[1] : floatmax(0.02, 1.0 / float(100)))
		at[1] = now
		lsg_n88(ctx)
	}
	return FMRES_IGNORED
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv1[id] = 0
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n84(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n0(ctx)
	return HAM_IGNORED
}

// Player Spawned (ps)
lsg_n0(ctx[])
{
	lsg_n1(ctx)
}

// Print String (p1)
lsg_n1(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsg_w1_pin(lsg_vec(0.0, 0.0, 64.0), 0, 500.0, 1, 0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n2(ctx)
}

// Print String (p2)
lsg_n2(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_name(lsg_w1_np(-1, ctx[0], 2048.0, 1, 2, 2)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n3(ctx)
}

// Print String (p3)
lsg_n3(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(Float:lsg_w1_np(-1, ctx[0], 2048.0, 1, 2, 2, true)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n4(ctx)
}

// Print String (p4)
lsg_n4(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsg_w1_ein(-1, ctx[0], 800.0, "info_player_start", 64))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n5(ctx)
}

// Print String (p5)
lsg_n5(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsg_w1_ein(lsg_vec(0.0, 0.0, 64.0), 0, 300.0, "", 64))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n6(ctx)
}

// Print String (p6)
lsg_n6(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %d", "[0]", lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n7(ctx)
}

// Print String (p7)
lsg_n7(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(Float:lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0, true)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n8(ctx)
}

// Print String (p8)
lsg_n8(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsg_w1_los(ctx[0], -1, 0, lsg_vec(0.0, 0.0, 64.0), 1)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n9(ctx)
}

// Print String (p9)
lsg_n9(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_los(ctx[0], -1, 0, lsg_vec(0.0, 0.0, 64.0), 1, true)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n10(ctx)
}

// Print String (p10)
lsg_n10(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((random_float(0.0, 100.0) < (30.0) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n11(ctx)
}

// Print String (p11)
lsg_n11(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(random_num(0, 1)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n12(ctx)
}

// Print String (p12)
lsg_n12(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(float(lsg_w1_wpick(lsg_mka6(3.0, 1.0), 27))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n13(ctx)
}

// Print String (p13)
lsg_n13(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_agets(lsg_w1_split("  Hello, World,  again  ", ","), float(lsg_w1_wpick(lsg_mka6(3.0, 1.0), 27))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n14(ctx)
}

// Print String (p14)
lsg_n14(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_map(5.0, 0.0, 10.0, 100.0, 200.0, 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n15(ctx)
}

// Print String (p15)
lsg_n15(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(floatsin(30.0, degrees)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n16(ctx)
}

// Print String (p16)
lsg_n16(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(floattan(0.5, radian)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n17(ctx)
}

// Print String (p17)
lsg_n17(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(floatatan2(1.0, -1.0, degrees)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n18(ctx)
}

// Print String (p18)
lsg_n18(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(((3.142) * 57.2957795)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n19(ctx)
}

// Print String (p19)
lsg_n19(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(float(floatround(-2.7, floatround_tozero))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n20(ctx)
}

// Print String (p20)
lsg_n20(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((floatabs((0.1) - (0.1)) <= floatabs(0.001) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n21(ctx)
}

// Print String (p21)
lsg_n21(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsg_w1_split("  Hello, World,  again  ", ","))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n22(ctx)
}

// Print String (p22)
lsg_n22(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_repl("  Hello, World,  again  ", "World", "There"))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n23(ctx)
}

// Print String (p23)
lsg_n23(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_sub("  Hello, World,  again  ", 2.0, 5.0))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n24(ctx)
}

// Print String (p24)
lsg_n24(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_case("  Hello, World,  again  ", 1))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n25(ctx)
}

// Print String (p25)
lsg_n25(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_case("  Hello, World,  again  ", 0))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n26(ctx)
}

// Print String (p26)
lsg_n26(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_trim("  Hello, World,  again  "))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n27(ctx)
}

// Print String (p27)
lsg_n27(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((lsg_w1_starts("  Hello, World,  again  ", "  hel") ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n28(ctx)
}

// Print String (p28)
lsg_n28(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((lsg_w1_ends("  Hello, World,  again  ", "AIN  ") ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n29(ctx)
}

// Print String (p29)
lsg_n29(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_join(lsg_w1_split("  Hello, World,  again  ", ","), " - "))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n30(ctx)
}

// Print String (p30)
lsg_n30(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_time(3.0, 0))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n31(ctx)
}

// Print String (p31)
lsg_n31(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_num(lsg_w1_map(5.0, 0.0, 10.0, 100.0, 200.0, 1), 2, 1))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n32(ctx)
}

// Print String (p32)
lsg_n32(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 0)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n33(ctx)
}

// Print String (p33)
lsg_n33(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n34(ctx)
}

// Print String (p34)
lsg_n34(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 1, true)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n35(ctx)
}

// Print String (p35)
lsg_n35(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 2)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n36(ctx)
}

// Print String (p36)
lsg_n36(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 2, true)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n37(ctx)
}

// Print String (p37)
lsg_n37(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_astat(lsg_mka6(3.0, 1.0), 3)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n38(ctx)
}

// Print String (p38)
lsg_n38(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", fmt("%d items", lsg_alen(lsg_w1_slice(lsg_w1_split("  Hello, World,  again  ", ","), 1.0, 2.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n39(ctx)
}

// Print String (p39)
lsg_n39(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_dot(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n40(ctx)
}

// Print String (p40)
lsg_n40(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_cross(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n41(ctx)
}

// Print String (p41)
lsg_n41(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_rnd(lsg_vec(0.0, 0.0, 64.0), 200.0, 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n42(ctx)
}

// Print String (p42)
lsg_n42(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_rot(lsg_vec(100.0, 50.0, 0.0), 90.0, 10.0)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n43(ctx)
}

// Print String (p43)
lsg_n43(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_vlerp(lsg_vec(0.0, 0.0, 64.0), lsg_vec(100.0, 50.0, 0.0), 0.25)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n44(ctx)
}

// Print String (p44)
lsg_n44(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_toang(lsg_vec(100.0, 50.0, 0.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n45(ctx)
}

// Print String (p45)
lsg_n45(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_fwd(lsg_w1_toang(lsg_vec(100.0, 50.0, 0.0)))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n46(ctx)
}

// Print String (p46)
lsg_n46(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_local(ctx[0], lsg_vec(100.0, 50.0, 0.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n47(ctx)
}

// Print String (p47)
lsg_n47(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_world(ctx[0], lsg_vec(100.0, 50.0, 0.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n48(ctx)
}

// Print String (p48)
lsg_n48(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 0)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n49(ctx)
}

// Print String (p49)
lsg_n49(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n50(ctx)
}

// Print String (p50)
lsg_n50(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_relc(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 2)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n51(ctx)
}

// Print String (p51)
lsg_n51(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_angto(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0))))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n52(ctx)
}

// Print String (p52)
lsg_n52(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((lsg_w1_cone(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 90.0, 0) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n53(ctx)
}

// Print String (p53)
lsg_n53(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[((lsg_w1_cone(ctx[0], lsg_w1_ne(-1, ctx[0], "info_player_start", 4096.0), 120.0, 1) ? 1 : 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n54(ctx)
}

// Print String (p54)
lsg_n54(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_vplayer(ctx[0], 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n55(ctx)
}

// Print String (p55)
lsg_n55(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_vplayer(ctx[0], 2)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n56(ctx)
}

// Print String (p56)
lsg_n56(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 0)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n57(ctx)
}

// Print String (p57)
lsg_n57(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %d", "[0]", lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 1))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n58(ctx)
}

// Print String (p58)
lsg_n58(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 2)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n59(ctx)
}

// Print String (p59)
lsg_n59(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsg_w1_aim(ctx[0], floatmin(2048.0, float(8192)), 3)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n60(ctx)
}

// Print String (p60)
lsg_n60(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_bits(12.0, 2.0, 4)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n61(ctx)
}

// Print String (p61)
lsg_n61(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_ntext(lsg_w1_bits(5.0, 3.0, 3)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n62(ctx)
}

// Print String (p62)
lsg_n62(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(lsg_w1_free(119, lsg_vec(0.0, 0.0, 64.0), HULL_HUMAN, 256.0, 64, 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n63(ctx)
}

// Print String (p63)
lsg_n63(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsg_w1_free(119, lsg_vec(0.0, 0.0, 64.0), HULL_HUMAN, 256.0, 64, 1)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n64(ctx)
}

// Print String (p64)
lsg_n64(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_lang(ctx[0], 1))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n65(ctx)
}

// Print String (p65)
lsg_n65(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_w1_lang(ctx[0], 0))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n66(ctx)
}

// Draw Debug Line (dl)
lsg_n66(ctx[])
{
	lsg_n67(ctx)
}

// Draw Debug Box (db)
lsg_n67(ctx[])
{
	lsg_n68(ctx)
}

// Draw Debug Point (dp)
lsg_n68(ctx[])
{
	lsg_n69(ctx)
}

// Log Line (log)
lsg_n69(ctx[])
{
	lsg_w1_log(69, lsg_w1_lang(ctx[0], 1), 5)
	lsg_n70(ctx)
}

// Toggle Variable (tg)
lsg_n70(ctx[])
{
	g_v0 = !g_v0
	g_lsgO[0] = g_v0 ? 1 : 0
	lsg_n71(ctx)
}

// Print String (p66)
lsg_n71(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(g_lsgO[0]) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n72(ctx)
}

// Toggle Variable (tgp)
lsg_n72(ctx[])
{
	g_pv1[lsg_slot(ctx[0])] = !g_pv1[lsg_slot(ctx[0])]
	g_lsgO[1] = g_pv1[lsg_slot(ctx[0])] ? 1 : 0
	lsg_n73(ctx)
}

// Toggle Variable (tgt)
lsg_n73(ctx[])
{
	g_tv2[clamp(lsg_team(ctx[0]), 0, 3)] = !g_tv2[clamp(lsg_team(ctx[0]), 0, 3)]
	g_lsgO[2] = g_tv2[clamp(lsg_team(ctx[0]), 0, 3)] ? 1 : 0
	lsg_n74(ctx)
}

// Print String (p67)
lsg_n74(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", g_lsgYesNo[(g_lsgO[2]) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n75(ctx)
}

// Reverse (rev)
lsg_n75(ctx[])
{
	lsg_w1_areverse(lsg_mka6(3.0, 1.0))
	lsg_n76(ctx)
}

// Sequence (seq)
lsg_n76(ctx[])
{
	lsg_n77_reset(ctx)
	lsg_n77_reset_player(ctx)
	lsg_n77(ctx)
}

// Do Once Per Player (dopp)
stock lsg_n77(ctx[])
{
	new p = ctx[0]
	if (p < 1 || p > 32)
	{
	
		return
	}
	if (g_lsgLeft[77] & (1 << (p - 1)))
	{
	
		return
	}
	g_lsgLeft[77] |= 1 << (p - 1)
	lsg_n78(ctx)
}

stock lsg_n77_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[77] = 0
}

stock lsg_n77_reset_player(ctx[])
{
	#pragma unused ctx
	new p = ctx[0]
	if (p >= 1 && p <= 32)
		g_lsgLeft[77] &= ~(1 << (p - 1))
}

// Multi Gate (mg)
stock lsg_n78(ctx[])
{
	if ((g_lsgLeft[78] & 7) == 7)
	{
		g_lsgLeft[78] = 0
	}
	new k = -1
	for (new j = 0; j < 3; j++)
	{
		new q = (1 + j) % 3
		if (!(g_lsgLeft[78] & (1 << q)))
		{
			k = q
			break
		}
	}
	if (k < 0)
		return
	g_lsgLeft[78] |= 1 << k
	switch (k)
	{
		case 0:
		{
			lsg_n79(ctx)
		}
		case 1:
		{
			lsg_n80(ctx)
		}
		case 2:
		{
			lsg_n81(ctx)
		}
	}
}

stock lsg_n78_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[78] = 0
}

// Print String (mgp0)
lsg_n79(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 0")
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n82(ctx)
}

// Print String (mgp1)
lsg_n80(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 1")
	server_print("[%s] %s", LSG_MODE, t)
	#pragma unused ctx
}

// Print String (mgp2)
lsg_n81(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s", "gate 2")
	server_print("[%s] %s", LSG_MODE, t)
	#pragma unused ctx
}

// Multi Gate (mgr)
stock lsg_n82(ctx[])
{
	if ((g_lsgLeft[82] & 3) == 3)
	{
	
		return
	}
	new k = -1
	new free[8], nf
	for (new j = 0; j < 2; j++)
		if (!(g_lsgLeft[82] & (1 << j)))
			free[nf++] = j
	if (nf)
		k = free[random(nf)]
	if (k < 0)
		return
	g_lsgLeft[82] |= 1 << k
	switch (k)
	{
		case 0:
		{
		}
		case 1:
		{
			lsg_n83(ctx)
		}
	}
}

stock lsg_n82_reset(ctx[])
{
	#pragma unused ctx
	g_lsgLeft[82] = 0
}

// Print String (mgrp)
lsg_n83(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s", "random gate")
	server_print("[%s] %s", LSG_MODE, t)
	#pragma unused ctx
}

// Round Started (rs)
lsg_n84(ctx[])
{
	lsg_n78_reset(ctx)
}

// On Tick (tick)
lsg_n85(ctx[])
{
	lsg_n86(ctx)
}

// Print String (tp1)
lsg_n86(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "tick [0]", lsg_vtext(lsg_w1_aim(lsg_w1_np(lsg_vec(0.0, 0.0, 64.0), 0, 8192.0, 1, 0, 0), floatmin(4096.0, float(8192)), 0)))
	server_print("[%s] %s", LSG_MODE, t)
	lsg_n87(ctx)
}

// Print String (tp2)
lsg_n87(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "delta [0]", lsg_w1_time(Float:ctx[0], 2))
	server_print("[%s] %s", LSG_MODE, t)
}

// On Tick (fast)
lsg_n88(ctx[])
{
	lsg_n89(ctx)
}

// Print String (fp)
lsg_n89(ctx[])
{
	new t[192]
	formatex(t, charsmax(t), "%s %s", "fast [0]", g_lsgYesNo[(lsg_w1_los(0, lsg_vec(0.0, 0.0, 64.0), 0, lsg_vec(100.0, 50.0, 0.0), 0)) ? 1 : 0])
	server_print("[%s] %s", LSG_MODE, t)
	#pragma unused ctx
}

