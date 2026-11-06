/*
	gm_extras - the "server extras" from Loner-Strike's Server extras page.

	Map rotation: picks the next map from the rotation and puts it in amx_nextmap,
	  which nextmap.amxx changes to when the map ends ("say nextmap" shows it). The
	  rotation is data/gamemodes/rotation_<mode>.txt for the current Game Mode Menu
	  mode when the host made one, else the game's map cycle (the mapcyclefile cvar,
	  mapcycle.txt). Without nextmap.amxx this plugin changes the map itself.
	  A map vote (mapchooser, rtv) still decides over the rotation.
	  gm_rotation_random 1  a random map from the list instead of the next one
	  gm_rtv_cycle 1        gm_pubserver's rtv and nominations offer only these maps
	Chat messages: data/gamemodes/ads.txt (written by the app), one message every
	  gm_ads_interval minutes (0 = off), in order or at random (gm_ads_random 1).
	  Lines: "prefix <text>" once, then "<color> <message>" with color d (default),
	  g (green), r (red), b (blue) or s (grey). {server} {map} {nextmap} {players}
	  {timeleft} are filled in.
	Away (AFK): no keys pressed for gm_afk_spec seconds while alive on a team moves the
	  player to spectator; gm_afk_kick seconds later (0 = never) they're kicked unless
	  they picked a team again. Someone stuck on the team menu is kicked after both.
	High ping: every 10 seconds the average of the last 3 pings; gm_ping_checks checks
	  in a row above gm_ping_max ms gets a kick, with a warning each time. Not in the
	  first minute of a map, nor a player's first 45 seconds.
	Fast downloads: gm_fastdl "<url>" sets sv_downloadurl, and clears it again when
	  emptied (a value the host set some other way is left alone).

	Never acts on bots, HLTV or the listen-server host; flag a (immunity) is never
	moved or kicked, and admins (flag c) aren't kicked for ping. Nothing runs in Tour
	of Duty missions.

	Settings: data/gamemodes/extras.cfg (written by the app), re-read with gm_extrasreload.
	Cvars: gm_rotation_random, gm_rtv_cycle, gm_ads_interval, gm_ads_random, gm_afk,
	       gm_afk_spec, gm_afk_kick, gm_ping, gm_ping_max, gm_ping_checks, gm_fastdl
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fakemeta>
#include <hamsandwich>

#define PLUGIN  "Server extras"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_ADS     7100
#define TASK_AFK     7200
#define TASK_PING    7300

#define AFK_STEP     5
#define PING_STEP    10.0
#define PING_SAMPLES 3
#define MAX_ADS      40
#define AD_LEN       192

#define TAG          "^4[Server]^1"

new g_pRotRandom, g_pAdsInterval, g_pAdsRandom
new g_pAfk, g_pAfkSpec, g_pAfkKick, g_pPing, g_pPingMax, g_pPingChecks, g_pFastdl
new g_pNextmap, g_pChattime
new g_DataDir[128]
new bool:g_Career
new g_OurNext[32]                // what we put in amx_nextmap (a vote may change it)
new bool:g_ChattimeRaised

new Array:g_Ads, Array:g_AdColors, g_AdPrefix[40], g_AdNext, g_AdLast = -1, g_AdEvery

new bool:g_Watch[33]             // a real player (not a bot, HLTV or the listen host)
new Float:g_Active[33], Float:g_Spawned[33], Float:g_Look[33][3]
new g_Idle[33], g_Away[33], bool:g_Moved[33], bool:g_Warned[33]
new g_Pings[33][PING_SAMPLES], g_PingN[33], g_Strikes[33], Float:g_Joined[33]

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pRotRandom   = register_cvar("gm_rotation_random", "0")
	register_cvar("gm_rtv_cycle", "0")                      // read by gm_pubserver
	g_pAdsInterval = register_cvar("gm_ads_interval", "0")
	g_pAdsRandom   = register_cvar("gm_ads_random", "0")
	g_pAfk         = register_cvar("gm_afk", "0")
	g_pAfkSpec     = register_cvar("gm_afk_spec", "120")
	g_pAfkKick     = register_cvar("gm_afk_kick", "0")
	g_pPing        = register_cvar("gm_ping", "0")
	g_pPingMax     = register_cvar("gm_ping_max", "250")
	g_pPingChecks  = register_cvar("gm_ping_checks", "5")
	g_pFastdl      = register_cvar("gm_fastdl", "")
	g_pChattime    = get_cvar_pointer("mp_chattime")

	register_srvcmd("gm_extrasreload", "srv_reload")
	register_event("30", "ev_intermission", "a")
	register_forward(FM_CmdStart, "fw_cmdstart")
	// RegisterHamPlayer also covers Condition Zero bots; the callback ignores them
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
	g_Ads = ArrayCreate(AD_LEN)
	g_AdColors = ArrayCreate(1)

	set_task(float(AFK_STEP), "task_afk", TASK_AFK, _, _, "b")
	set_task(PING_STEP, "task_ping", TASK_PING, _, _, "b")
}

public plugin_cfg()
{
	// Tour of Duty missions play as designed (same check as the other plugins)
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))

	// nextmap.amxx (loaded earlier) made amx_nextmap; without it, we keep our own
	g_pNextmap = get_cvar_pointer("amx_nextmap")
	if (!g_pNextmap)
		g_pNextmap = register_cvar("amx_nextmap", "", FCVAR_SERVER|FCVAR_SPONLY)

	new ad[8]
	get_localinfo("gm_adnext", ad, charsmax(ad))
	g_AdNext = str_to_num(ad)
	load(true)
}

public plugin_end()
{
	if (g_ChattimeRaised && g_pChattime)
		set_pcvar_float(g_pChattime, get_pcvar_float(g_pChattime) - 2.0)
	new ad[8]
	num_to_str(g_AdNext, ad, charsmax(ad))
	set_localinfo("gm_adnext", ad)
}

public srv_reload()
{
	load(false)
	server_print("[extras] settings reloaded")
	return PLUGIN_HANDLED
}

load(bool:first)
{
	new path[160]
	formatex(path, charsmax(path), "%s/extras.cfg", g_DataDir)
	if (file_exists(path))
	{
		server_cmd("exec ^"%s^"", path)
		server_exec()
	}
	apply_fastdl()
	load_ads()
	if (!g_Career)
		pick_next(first)
}

read_line(const path[], out[], len)
{
	out[0] = 0
	new f = fopen(path, "rt")
	if (f)
	{
		fgets(f, out, len)
		trim(out)
		fclose(f)
	}
}

bool:watched(id)
{
	return id >= 1 && id <= MaxClients && g_Watch[id] && is_user_connected(id)
}

// ---------------------------------------------------------------------------
// Map rotation
// ---------------------------------------------------------------------------

bool:plain_name(const name[])
{
	for (new i = 0; name[i]; i++)
		if (!isalnum(name[i]) && name[i] != '_' && name[i] != '-' && name[i] != '.')
			return false
	return name[0] != 0
}

// Maps in a rotation file (one per line; ; and // comments; a trailing .bsp is fine)
read_maps(const path[], Array:into)
{
	new f = fopen(path, "rt")
	if (!f)
		return
	new line[64], map[32]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (!line[0] || line[0] == ';' || (line[0] == '/' && line[1] == '/'))
			continue
		parse(line, map, charsmax(map))
		new len = strlen(map)
		if (len > 4 && equali(map[len - 4], ".bsp"))
			map[len - 4] = 0
		if (plain_name(map) && is_map_valid(map))
			ArrayPushString(into, map)
	}
	fclose(f)
}

// The rotation in use: the current mode's own, else the game's map cycle
rotation(Array:into)
{
	new mode[32]
	read_line(fmt("%s/current.txt", g_DataDir), mode, charsmax(mode))
	if (mode[0] && plain_name(mode))
		read_maps(fmt("%s/rotation_%s.txt", g_DataDir, mode), into)
	if (!ArraySize(into))
	{
		new cycle[64]
		get_cvar_string("mapcyclefile", cycle, charsmax(cycle))
		read_maps(cycle[0] ? cycle : "mapcycle.txt", into)
	}
}

bool:recently_played(const map[], count)
{
	if (count <= 0)
		return false
	new f = fopen(fmt("%s/recent.txt", g_DataDir), "rt")
	if (!f)
		return false
	new line[40], bool:found
	while (count-- > 0 && fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (equali(line, map))
		{
			found = true
			break
		}
	}
	fclose(f)
	return found
}

pick_next(bool:first)
{
	// on a reload, a vote's result stays
	if (!first)
	{
		new now[32]
		get_pcvar_string(g_pNextmap, now, charsmax(now))
		if (g_OurNext[0] && !equali(now, g_OurNext))
			return
	}
	new Array:list = ArrayCreate(32)
	rotation(list)
	new n = ArraySize(list)
	if (!n)
	{
		ArrayDestroy(list)
		return          // nothing to go on: nextmap.amxx's own choice stays
	}
	new cur[32], map[32], next[32], pos = -1
	get_mapname(cur, charsmax(cur))

	if (get_pcvar_num(g_pRotRandom) && n > 1)
	{
		// skip the current map, and the last few played while there's choice
		new avoid = min(5, (n - 1) / 2)
		for (new tries = 0; tries < 60; tries++)
		{
			new i = random(n)
			ArrayGetString(list, i, map, charsmax(map))
			if (equali(map, cur) || (tries < 40 && recently_played(map, avoid + 1)))
				continue
			copy(next, charsmax(next), map)
			break
		}
	}
	if (!next[0])
	{
		for (new i = 0; i < n; i++)
		{
			ArrayGetString(list, i, map, charsmax(map))
			if (equali(map, cur))
			{
				pos = (i + 1) % n
				break
			}
		}
		// not in the rotation (picked by hand or by a vote): carry on where it left off
		if (pos < 0)
		{
			new saved[8]
			get_localinfo("gm_rotpos", saved, charsmax(saved))
			pos = str_to_num(saved) % n
		}
		ArrayGetString(list, pos, next, charsmax(next))
		new saved[8]
		num_to_str((pos + 1) % n, saved, charsmax(saved))
		set_localinfo("gm_rotpos", saved)
	}
	ArrayDestroy(list)
	copy(g_OurNext, charsmax(g_OurNext), next)
	set_pcvar_string(g_pNextmap, next)
}

// The map ended. nextmap.amxx changes to amx_nextmap; without it, we do.
public ev_intermission()
{
	if (g_Career || is_plugin_loaded("nextmap.amxx", true) != -1)
		return
	new map[32]
	get_pcvar_string(g_pNextmap, map, charsmax(map))
	if (!plain_name(map) || !is_map_valid(map))
		return
	new Float:wait = g_pChattime ? get_pcvar_float(g_pChattime) : 0.0
	if (g_pChattime && !g_ChattimeRaised)
	{
		set_pcvar_float(g_pChattime, wait + 2.0)     // so the game doesn't change first
		g_ChattimeRaised = true
	}
	set_task(floatmax(wait, 0.5), "task_change", 0, map, sizeof map)
}

public task_change(const map[])
{
	engine_changelevel(map)
}

// ---------------------------------------------------------------------------
// Chat messages
// ---------------------------------------------------------------------------

load_ads()
{
	ArrayClear(g_Ads)
	ArrayClear(g_AdColors)
	g_AdPrefix[0] = 0
	new f = fopen(fmt("%s/ads.txt", g_DataDir), "rt")
	if (f)
	{
		new line[AD_LEN + 16], word[8], rest[AD_LEN]
		while (fgets(f, line, charsmax(line)) && ArraySize(g_Ads) < MAX_ADS)
		{
			trim(line)
			if (!line[0] || line[0] == ';')
				continue
			strtok2(line, word, charsmax(word), rest, charsmax(rest), ' ', TRIM_FULL)
			if (equal(word, "prefix"))
				copy(g_AdPrefix, charsmax(g_AdPrefix), rest)
			else if (rest[0] && strlen(word) == 1 && containi("dgrbs", word) >= 0)
			{
				ArrayPushString(g_Ads, rest)
				ArrayPushCell(g_AdColors, word[0])
			}
		}
		fclose(f)
	}
	new every = g_Career ? 0 : clamp(get_pcvar_num(g_pAdsInterval), 0, 60)
	if (!ArraySize(g_Ads))
		every = 0
	if (every != g_AdEvery || !task_exists(TASK_ADS))
	{
		remove_task(TASK_ADS)
		if (every > 0)
			set_task(every * 60.0, "task_ad", TASK_ADS, _, _, "b")
	}
	g_AdEvery = every
}

humans()
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			n++
	return n
}

fill(text[], len)
{
	new value[64]
	if (contain(text, "{server}") >= 0)
	{
		get_cvar_string("hostname", value, charsmax(value))
		replace_string(text, len, "{server}", value)
	}
	if (contain(text, "{map}") >= 0)
	{
		get_mapname(value, charsmax(value))
		replace_string(text, len, "{map}", value)
	}
	if (contain(text, "{nextmap}") >= 0)
	{
		get_pcvar_string(g_pNextmap, value, charsmax(value))
		if (!value[0])
			copy(value, charsmax(value), "not decided yet")
		replace_string(text, len, "{nextmap}", value)
	}
	if (contain(text, "{players}") >= 0)
	{
		formatex(value, charsmax(value), "%d/%d", get_playersnum(), MaxClients)
		replace_string(text, len, "{players}", value)
	}
	if (contain(text, "{timeleft}") >= 0)
	{
		new left = get_timeleft()
		if (get_cvar_float("mp_timelimit") <= 0.0 || left < 0)
			copy(value, charsmax(value), "no time limit")
		else
			formatex(value, charsmax(value), "%d:%02d", left / 60, left % 60)
		replace_string(text, len, "{timeleft}", value)
	}
}

public task_ad()
{
	new n = ArraySize(g_Ads)
	if (!n || g_Career || !humans())
		return
	new i
	if (get_pcvar_num(g_pAdsRandom) && n > 1)
	{
		do
		{
			i = random(n)
		}
		while (i == g_AdLast)
	}
	else
		i = g_AdNext++ % n
	g_AdLast = i

	new text[AD_LEN + 64]
	ArrayGetString(g_Ads, i, text, charsmax(text))
	fill(text, charsmax(text))
	new sender = print_team_default, code = 1
	switch (ArrayGetCell(g_AdColors, i))
	{
		case 'g': code = 4
		case 'r': { code = 3; sender = print_team_red; }
		case 'b': { code = 3; sender = print_team_blue; }
		case 's': { code = 3; sender = print_team_grey; }
	}
	// the text goes in as an argument, never as the format: a % in it is just a %
	if (g_AdPrefix[0])
		client_print_color(0, sender, "^4%s %c%s", g_AdPrefix, code, text)
	else
		client_print_color(0, sender, "%c%s", code, text)
}

// ---------------------------------------------------------------------------
// Away (AFK) and high ping
// ---------------------------------------------------------------------------

public client_putinserver(id)
{
	// the listen-server host is player 1 and is never moved or kicked
	g_Watch[id] = !is_user_bot(id) && !is_user_hltv(id) && !(id == 1 && !is_dedicated_server())
	g_Active[id] = get_gametime()
	g_Joined[id] = get_gametime()
	g_Idle[id] = 0
	g_Away[id] = 0
	g_Moved[id] = false
	g_Warned[id] = false
	g_PingN[id] = 0
	g_Strikes[id] = 0
}

public client_disconnected(id)
{
	g_Watch[id] = false
}

public fw_spawn_post(id)
{
	if (watched(id))
		g_Spawned[id] = get_gametime()
}

// Any key, movement or mouse look counts as being there
public fw_cmdstart(id, uc)
{
	if (id < 1 || id > MaxClients || !g_Watch[id])
		return FMRES_IGNORED
	new Float:look[3], Float:now = get_gametime()
	get_uc(uc, UC_ViewAngles, look)
	new bool:looked = floatabs(look[0] - g_Look[id][0]) > 0.5 || floatabs(look[1] - g_Look[id][1]) > 0.5
	g_Look[id] = look
	// a spawn turns the view by itself
	if (looked && now - g_Spawned[id] < 1.5)
		looked = false
	new Float:fwd, Float:side
	get_uc(uc, UC_ForwardMove, fwd)
	get_uc(uc, UC_SideMove, side)
	if (looked || get_uc(uc, UC_Buttons) || fwd != 0.0 || side != 0.0)
		g_Active[id] = now
	return FMRES_IGNORED
}

kick(id, const reason[])
{
	server_cmd("kick #%d ^"%s^"", get_user_userid(id), reason)
}

to_spec(id, kick_after)
{
	if (is_user_alive(id))
		user_kill(id, 1)              // 1: no frag lost
	engclient_cmd(id, "jointeam", "6")
	if (cs_get_user_team(id) != CS_TEAM_SPECTATOR)   // spectators not allowed: do it anyway
		cs_set_user_team(id, CS_TEAM_SPECTATOR)
	g_Moved[id] = true
	g_Idle[id] = 0
	g_Away[id] = 0
	g_Warned[id] = false
	new name[32]
	get_user_name(id, name, charsmax(name))
	client_print_color(0, print_team_grey, "%s ^3%s^1 was moved to spectator for being away.", TAG, name)
	if (kick_after > 0)
		client_print_color(id, print_team_default, "%s Pick a team within %d seconds to stay on the server.", TAG, kick_after)
}

public task_afk()
{
	if (g_Career || !get_pcvar_num(g_pAfk))
		return
	new spec = clamp(get_pcvar_num(g_pAfkSpec), 30, 3600)
	new kick_after = get_pcvar_num(g_pAfkKick)
	if (kick_after > 0)
		kick_after = clamp(kick_after, 30, 3600)
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!watched(id))
			continue
		if (get_user_flags(id) & ADMIN_IMMUNITY)
		{
			g_Idle[id] = 0
			g_Away[id] = 0
			continue
		}
		new CsTeams:team = cs_get_user_team(id)
		new bool:active = now - g_Active[id] < float(AFK_STEP) + 0.5
		if (team == CS_TEAM_T || team == CS_TEAM_CT)
		{
			g_Moved[id] = false
			g_Away[id] = 0
			if (active)
			{
				g_Idle[id] = 0
				g_Warned[id] = false
				continue
			}
			if (!is_user_alive(id))       // waiting to respawn isn't being away
				continue
			g_Idle[id] += AFK_STEP
			if (g_Idle[id] >= spec)
				to_spec(id, kick_after)
			else if (!g_Warned[id] && g_Idle[id] + 15 >= spec)
			{
				g_Warned[id] = true
				client_print_color(id, print_team_red, "%s ^3Are you there?^1 Move, or you'll be moved to spectator in %d seconds.", TAG, spec - g_Idle[id])
			}
		}
		else if (kick_after > 0 && (g_Moved[id] || team == CS_TEAM_UNASSIGNED))
		{
			g_Away[id] += AFK_STEP
			new limit = g_Moved[id] ? kick_after : spec + kick_after
			if (g_Away[id] >= limit)
				kick(id, "You were away too long (AFK). Come back any time!")
			else if (!g_Warned[id] && g_Away[id] + 20 >= limit)
			{
				g_Warned[id] = true
				client_print_color(id, print_team_red, "%s ^3Pick a team^1 or you'll be kicked for being away in %d seconds.", TAG, limit - g_Away[id])
			}
		}
		else
			g_Away[id] = 0
	}
}

public task_ping()
{
	if (g_Career || !get_pcvar_num(g_pPing))
		return
	// everyone's ping jumps while a map loads
	if (get_gametime() < 60.0)
		return
	new limit = clamp(get_pcvar_num(g_pPingMax), 50, 1000)
	new checks = clamp(get_pcvar_num(g_pPingChecks), 2, 20)
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!watched(id))
			continue
		if ((get_user_flags(id) & (ADMIN_IMMUNITY | ADMIN_KICK)) || now - g_Joined[id] < 45.0)
		{
			g_PingN[id] = 0
			g_Strikes[id] = 0
			continue
		}
		new ping, loss
		get_user_ping(id, ping, loss)
		g_Pings[id][g_PingN[id] % PING_SAMPLES] = ping
		g_PingN[id]++
		if (g_PingN[id] < PING_SAMPLES)
			continue
		new sum
		for (new i = 0; i < PING_SAMPLES; i++)
			sum += g_Pings[id][i]
		new avg = sum / PING_SAMPLES
		if (avg <= limit)
		{
			g_Strikes[id] = 0
			continue
		}
		if (++g_Strikes[id] >= checks)
		{
			new reason[96]
			formatex(reason, charsmax(reason), "Your ping was too high (%d ms, the limit here is %d ms)", avg, limit)
			kick(id, reason)
		}
		else
			client_print_color(id, print_team_red, "%s Your ping is high: ^3%d ms^1 (the limit is %d). Warning %d of %d before a kick.", TAG, avg, limit, g_Strikes[id], checks - 1)
	}
}

// ---------------------------------------------------------------------------
// Fast downloads
// ---------------------------------------------------------------------------

apply_fastdl()
{
	new url[128], was[4]
	get_pcvar_string(g_pFastdl, url, charsmax(url))
	get_localinfo("gm_fastdl", was, charsmax(was))
	if (url[0])
	{
		set_cvar_string("sv_downloadurl", url)
		set_localinfo("gm_fastdl", "1")
	}
	else if (was[0] == '1')
	{
		// only clear what we set: a URL from server.cfg stays
		set_cvar_string("sv_downloadurl", "")
		set_localinfo("gm_fastdl", "")
	}
}
