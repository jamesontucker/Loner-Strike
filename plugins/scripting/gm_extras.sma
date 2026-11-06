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
	Joins: "Name (Germany) joined." / "Name left." in chat (gm_joins). The country comes
	  from AMX Mod X's geoip module and its GeoLite2 database when both are there
	  (gm_joins_country); addresses on the local network show none. gm_joins_skipbots 1
	  leaves bots out. A map change isn't a leave and a join: who's here is kept in
	  localinfo by userid.
	Votes: say /votekick or /voteban opens a menu of players; everyone else then votes
	  for 20 seconds. gm_vote_percent of the players who can vote (not the target) must
	  say yes, with at least gm_vote_min players on the server; gm_vote_cooldown minutes
	  between votes. Bans last gm_vote_bantime minutes (by SteamID, else by address).
	  Immune: flag a, the listen-server host and HLTV; bots only to votekick, with
	  gm_vote_bots 1 (and the bot count goes down by one).
	Empty server: gm_empty minutes after the last real player leaves, go back to
	  gm_empty_map (and Game Mode Menu mode gm_empty_mode, through its gm_mode command).
	gm_extraswarn <minutes>: the app's daily restart warning (0: restarting now).

	Never acts on bots, HLTV or the listen-server host; flag a (immunity) is never
	moved or kicked, and admins (flag c) aren't kicked for ping. Nothing runs in Tour
	of Duty missions.

	Settings: data/gamemodes/extras.cfg (written by the app), re-read with gm_extrasreload.
	Cvars: gm_rotation_random, gm_rtv_cycle, gm_ads_interval, gm_ads_random, gm_afk,
	       gm_afk_spec, gm_afk_kick, gm_ping, gm_ping_max, gm_ping_checks, gm_fastdl,
	       gm_joins, gm_joins_country, gm_joins_skipbots, gm_votekick, gm_voteban,
	       gm_vote_percent, gm_vote_min, gm_vote_bantime, gm_vote_cooldown, gm_vote_bots,
	       gm_empty, gm_empty_map, gm_empty_mode
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fakemeta>
#include <hamsandwich>
#include <geoip>

#define PLUGIN  "Server extras"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_ADS     7100
#define TASK_AFK     7200
#define TASK_PING    7300
#define TASK_EMPTY   7400
#define TASK_VOTE    7500

#define VOTE_TIME    20

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

new g_pJoins, g_pJoinsCountry, g_pJoinsSkipBots
new g_pVoteKick, g_pVoteBan, g_pVotePct, g_pVoteMin, g_pVoteBanTime, g_pVoteCool, g_pVoteBots
new g_pEmpty, g_pEmptyMap, g_pEmptyMode
new bool:g_Geo

// the vote going on (g_VoteMenu -1: none)
new g_VoteMenu = -1, bool:g_VoteBan, g_VoteUid, g_VoteName[32], g_VoteAuth[40], g_VoteIp[24]
new g_VoteYes, g_VoteNo, g_VoteVoters, bool:g_CanVote[33], g_Voted[33], Float:g_VoteEnded

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
	g_pJoins         = register_cvar("gm_joins", "0")
	g_pJoinsCountry  = register_cvar("gm_joins_country", "1")
	g_pJoinsSkipBots = register_cvar("gm_joins_skipbots", "1")
	g_pVoteKick      = register_cvar("gm_votekick", "0")
	g_pVoteBan       = register_cvar("gm_voteban", "0")
	g_pVotePct       = register_cvar("gm_vote_percent", "60")
	g_pVoteMin       = register_cvar("gm_vote_min", "3")
	g_pVoteBanTime   = register_cvar("gm_vote_bantime", "30")
	g_pVoteCool      = register_cvar("gm_vote_cooldown", "3")
	g_pVoteBots      = register_cvar("gm_vote_bots", "0")
	g_pEmpty         = register_cvar("gm_empty", "0")
	g_pEmptyMap      = register_cvar("gm_empty_map", "")
	g_pEmptyMode     = register_cvar("gm_empty_mode", "")
	g_pChattime    = get_cvar_pointer("mp_chattime")

	register_srvcmd("gm_extrasreload", "srv_reload")
	register_srvcmd("gm_extraswarn", "srv_warn")
	register_event("30", "ev_intermission", "a")
	register_forward(FM_CmdStart, "fw_cmdstart")
	// the game's own disconnect: not called on a map change (AMX Mod X's client_disconnected is)
	register_forward(FM_ClientDisconnect, "fw_disconnect")
	for (new i = 0; i < 2; i++)
	{
		new say[12]
		copy(say, charsmax(say), i ? "say_team" : "say")
		register_clcmd(fmt("%s /votekick", say), "cmd_votekick")
		register_clcmd(fmt("%s votekick", say), "cmd_votekick")
		register_clcmd(fmt("%s /voteban", say), "cmd_voteban")
		register_clcmd(fmt("%s voteban", say), "cmd_voteban")
	}
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
	g_Geo = bool:LibraryExists("geoip", LibType_Library)
	load(true)
}

// The geoip module is optional: without it, joins show no country
public plugin_natives()
{
	set_module_filter("module_filter")
	set_native_filter("native_filter")
}

public module_filter(const module[], LibType:type)
{
	return equali(module, "geoip") ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

public native_filter(const name[], index, trap)
{
	return (!trap && equali(name, "geoip_", 6)) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
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
	check_empty(0)
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

// Real players on the server (not bots or HLTV), leaving out `skip`
humans(skip = 0)
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (i != skip && is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
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
	g_CanVote[id] = false
	joined(id)
	if (!is_user_bot(id) && !is_user_hltv(id))
		remove_task(TASK_EMPTY)
}

public client_disconnected(id)
{
	g_Watch[id] = false
}

// Someone really left (or was kicked): not called on a map change
public fw_disconnect(id)
{
	if (id < 1 || id > MaxClients || !is_user_connected(id))
		return FMRES_IGNORED
	left(id)
	vote_left(id)
	check_empty(id)
	return FMRES_IGNORED
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

// ---------------------------------------------------------------------------
// Join and leave messages
// ---------------------------------------------------------------------------

// Who's on the server, by userid, kept over map changes (",12,15,")
bool:was_here(uid)
{
	new list[320]
	get_localinfo("gm_here", list, charsmax(list))
	return contain(list, fmt(",%d,", uid)) >= 0
}

set_here(uid, bool:on)
{
	new list[320], key[16]
	get_localinfo("gm_here", list, charsmax(list))
	formatex(key, charsmax(key), ",%d,", uid)
	replace_string(list, charsmax(list), key, ",")
	if (!list[0] || strlen(list) > 280)       // a fresh server, or something went missing
		copy(list, charsmax(list), ",")
	if (on)
		add(list, charsmax(list), key[1])
	set_localinfo("gm_here", list)
}

// No country for this computer and the local network (or playit.gg, which arrives as local)
bool:lan_ip(const ip[])
{
	if (!ip[0] || equal(ip, "loopback") || equal(ip, "127.", 4) || equal(ip, "10.", 3) || equal(ip, "192.168.", 8) || equal(ip, "169.254.", 8))
		return true
	if (equal(ip, "172.", 4))
	{
		new second = str_to_num(ip[4])
		return second >= 16 && second <= 31
	}
	return false
}

bool:show_joins(id)
{
	if (g_Career || !get_pcvar_num(g_pJoins) || is_user_hltv(id))
		return false
	return !(is_user_bot(id) && get_pcvar_num(g_pJoinsSkipBots))
}

joined(id)
{
	if (is_user_hltv(id))
		return
	new uid = get_user_userid(id)
	if (was_here(uid))          // back after a map change
		return
	set_here(uid, true)
	if (!show_joins(id))
		return
	new name[32], ip[24], country[48]
	get_user_name(id, name, charsmax(name))
	if (g_Geo && !is_user_bot(id) && get_pcvar_num(g_pJoinsCountry))
	{
		get_user_ip(id, ip, charsmax(ip), 1)
		if (!lan_ip(ip) && geoip_country_ex(ip, country, charsmax(country)) <= 0)
			country[0] = 0
	}
	if (country[0])
		client_print_color(0, print_team_grey, "%s ^3%s^1 (%s) joined.", TAG, name, country)
	else
		client_print_color(0, print_team_grey, "%s ^3%s^1 joined.", TAG, name)
}

left(id)
{
	set_here(get_user_userid(id), false)
	if (!show_joins(id))
		return
	new name[32]
	get_user_name(id, name, charsmax(name))
	client_print_color(0, print_team_grey, "%s ^3%s^1 left.", TAG, name)
}

// ---------------------------------------------------------------------------
// Vote kick and vote ban
// ---------------------------------------------------------------------------

public cmd_votekick(id)
{
	vote_menu(id, false)
	return PLUGIN_CONTINUE
}

public cmd_voteban(id)
{
	vote_menu(id, true)
	return PLUGIN_CONTINUE
}

bool:is_host(id)
{
	return id == 1 && !is_dedicated_server()
}

bool:can_target(id, bool:ban, starter)
{
	if (id == starter || !is_user_connected(id) || is_user_hltv(id) || is_host(id))
		return false
	if (get_user_flags(id) & ADMIN_IMMUNITY)
		return false
	if (is_user_bot(id))
		return !ban && get_pcvar_num(g_pVoteBots) != 0
	return true
}

// Whether `id` may start a vote now; tells them why not
bool:may_start(id, bool:ban)
{
	if (g_Career || !is_user_connected(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	new what[8]
	copy(what, charsmax(what), ban ? "ban" : "kick")
	if (!get_pcvar_num(ban ? g_pVoteBan : g_pVoteKick))
	{
		client_print_color(id, print_team_default, "%s Vote %s is off on this server.", TAG, what)
		return false
	}
	if (g_VoteMenu != -1)
	{
		client_print_color(id, print_team_default, "%s A vote is going on already.", TAG)
		return false
	}
	new need = clamp(get_pcvar_num(g_pVoteMin), 2, 32)
	if (humans() < need)
	{
		client_print_color(id, print_team_default, "%s Vote %s needs at least %d players on the server.", TAG, what, need)
		return false
	}
	new wait = floatround(g_VoteEnded + clamp(get_pcvar_num(g_pVoteCool), 0, 60) * 60.0 - get_gametime(), floatround_ceil)
	if (g_VoteEnded > 0.0 && wait > 0)
	{
		client_print_color(id, print_team_default, "%s The next vote can start in %d:%02d.", TAG, wait / 60, wait % 60)
		return false
	}
	return true
}

vote_menu(id, bool:ban)
{
	if (!may_start(id, ban))
		return
	new menu = menu_create(ban ? "\yVote to ban who?" : "\yVote to kick who?", "pick_handler")
	new name[32], info[16], n
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!can_target(i, ban, id))
			continue
		get_user_name(i, name, charsmax(name))
		formatex(info, charsmax(info), "%d %d", ban, get_user_userid(i))
		menu_additem(menu, name, info)
		n++
	}
	if (!n)
	{
		menu_destroy(menu)
		client_print_color(id, print_team_default, "%s There's nobody here you can vote to %s.", TAG, ban ? "ban" : "kick")
		return
	}
	menu_display(id, menu)
}

by_userid(uid)
{
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && get_user_userid(i) == uid)
			return i
	return 0
}

public pick_handler(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[16], access, cb, a[4], b[12]
	menu_item_getinfo(menu, item, access, info, charsmax(info), _, _, cb)
	menu_destroy(menu)
	parse(info, a, charsmax(a), b, charsmax(b))
	new bool:ban = str_to_num(a) != 0
	new target = by_userid(str_to_num(b))
	// everything again: time passed while the menu was open
	if (!may_start(id, ban))
		return PLUGIN_HANDLED
	if (!target || !can_target(target, ban, id))
	{
		client_print_color(id, print_team_default, "%s That player can't be voted on any more.", TAG)
		return PLUGIN_HANDLED
	}
	start_vote(id, target, ban)
	return PLUGIN_HANDLED
}

start_vote(starter, target, bool:ban)
{
	g_VoteBan = ban
	g_VoteUid = get_user_userid(target)
	get_user_name(target, g_VoteName, charsmax(g_VoteName))
	get_user_authid(target, g_VoteAuth, charsmax(g_VoteAuth))
	get_user_ip(target, g_VoteIp, charsmax(g_VoteIp), 1)
	g_VoteYes = 0
	g_VoteNo = 0
	g_VoteVoters = 0

	new title[96]
	if (ban)
		formatex(title, charsmax(title), "\yBan %s for %d minutes?", g_VoteName, clamp(get_pcvar_num(g_pVoteBanTime), 1, 10080))
	else
		formatex(title, charsmax(title), "\yKick %s?", g_VoteName)
	g_VoteMenu = menu_create(title, "vote_handler")
	menu_additem(g_VoteMenu, "Yes")
	menu_additem(g_VoteMenu, "No")
	menu_setprop(g_VoteMenu, MPROP_EXIT, MEXIT_NEVER)

	new name[32]
	get_user_name(starter, name, charsmax(name))
	client_print_color(0, print_team_grey, "%s ^3%s^1 started a vote to %s ^3%s^1. Vote in the menu.", TAG, name, ban ? "ban" : "kick", g_VoteName)
	for (new i = 1; i <= MaxClients; i++)
	{
		g_Voted[i] = 0
		g_CanVote[i] = i != target && is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i)
		if (!g_CanVote[i])
			continue
		g_VoteVoters++
		if (i == starter)
		{
			g_Voted[i] = 1
			g_VoteYes++
		}
		else
			menu_display(i, g_VoteMenu, 0, VOTE_TIME)
	}
	set_task(VOTE_TIME + 1.0, "task_vote_end", TASK_VOTE)
	if (g_VoteYes + g_VoteNo >= g_VoteVoters)
		end_vote()
}

public vote_handler(id, menu, item)
{
	if (menu != g_VoteMenu || item < 0 || id < 1 || id > MaxClients || !g_CanVote[id] || g_Voted[id])
		return PLUGIN_HANDLED
	g_Voted[id] = item == 0 ? 1 : 2
	if (item == 0)
		g_VoteYes++
	else
		g_VoteNo++
	if (g_VoteYes + g_VoteNo >= g_VoteVoters)
		end_vote()
	return PLUGIN_HANDLED
}

// A voter left: they no longer count. The target left: a kick vote is over.
vote_left(id)
{
	if (g_VoteMenu == -1)
		return
	if (get_user_userid(id) == g_VoteUid && !g_VoteBan)
	{
		close_vote()
		client_print_color(0, print_team_grey, "%s ^3%s^1 left, so the vote is over.", TAG, g_VoteName)
		return
	}
	if (g_CanVote[id])
	{
		g_CanVote[id] = false
		g_VoteVoters--
		// their vote goes with them
		if (g_Voted[id] == 1)
			g_VoteYes--
		else if (g_Voted[id] == 2)
			g_VoteNo--
		g_Voted[id] = 0
		if (g_VoteVoters <= 0 || g_VoteYes + g_VoteNo >= g_VoteVoters)
			end_vote()
	}
}

close_vote()
{
	if (g_VoteMenu == -1)
		return
	new menu = g_VoteMenu
	g_VoteMenu = -1
	remove_task(TASK_VOTE)
	g_VoteEnded = get_gametime()
	for (new i = 1; i <= MaxClients; i++)
	{
		new old, open
		if (is_user_connected(i) && player_menu_info(i, old, open) && open == menu)
			menu_cancel(i)
	}
	menu_destroy(menu)
}

public task_vote_end()
{
	end_vote()
}

bool:steam_id(const auth[])
{
	if (!equal(auth, "STEAM_", 6) || equal(auth, "STEAM_ID", 8))
		return false
	for (new i = 0; auth[i]; i++)
		if (!isdigit(auth[i]) && !(auth[i] >= 'A' && auth[i] <= 'Z') && auth[i] != '_' && auth[i] != ':')
			return false
	return true
}

bool:plain_ip(const ip[])
{
	for (new i = 0; ip[i]; i++)
		if (!isdigit(ip[i]) && ip[i] != '.')
			return false
	return ip[0] != 0
}

end_vote()
{
	if (g_VoteMenu == -1)
		return
	close_vote()
	new pct = clamp(get_pcvar_num(g_pVotePct), 30, 100)
	new voters = max(g_VoteVoters, 1)
	new need = (pct * voters + 99) / 100
	if (g_VoteYes < need)
	{
		client_print_color(0, print_team_grey, "%s The vote failed: %d of %d voted yes, %d needed. ^3%s^1 stays.", TAG, g_VoteYes, voters, need, g_VoteName)
		return
	}
	new target = by_userid(g_VoteUid)
	if (g_VoteBan)
	{
		new minutes = clamp(get_pcvar_num(g_pVoteBanTime), 1, 10080)
		// a player through playit.gg or on this computer shares the local address: kick only
		if (steam_id(g_VoteAuth))
			server_cmd("banid %d %s", minutes, g_VoteAuth)
		else if (plain_ip(g_VoteIp) && !equal(g_VoteIp, "127.", 4))
			server_cmd("addip %d %s", minutes, g_VoteIp)
		if (target)
			kick(target, fmt("Banned for %d minutes by a vote of the players", minutes))
		client_print_color(0, print_team_grey, "%s The vote passed (%d of %d). ^3%s^1 is banned for %d minutes.", TAG, g_VoteYes, voters, g_VoteName, minutes)
		return
	}
	if (!target)
	{
		client_print_color(0, print_team_grey, "%s The vote passed, but ^3%s^1 left already.", TAG, g_VoteName)
		return
	}
	if (is_user_bot(target))
	{
		// one bot fewer, instead of the game adding a new one
		new quota = get_cvar_pointer("bot_quota")
		if (quota && get_pcvar_num(quota) > 0)
			set_pcvar_num(quota, get_pcvar_num(quota) - 1)
	}
	kick(target, "Kicked by a vote of the players")
	client_print_color(0, print_team_grey, "%s The vote passed (%d of %d). ^3%s^1 was kicked.", TAG, g_VoteYes, voters, g_VoteName)
}

// ---------------------------------------------------------------------------
// Empty server: back to a default map (and mode)
// ---------------------------------------------------------------------------

// true when there's nothing to go back to, or the server is there already
bool:at_default()
{
	new map[32], cur[32], mode[32], now[32]
	get_pcvar_string(g_pEmptyMap, map, charsmax(map))
	if (!plain_name(map) || !is_map_valid(map))
		return true
	get_mapname(cur, charsmax(cur))
	if (!equali(map, cur))
		return false
	get_pcvar_string(g_pEmptyMode, mode, charsmax(mode))
	if (!mode[0] || is_plugin_loaded("gamemodes.amxx", true) == -1)
		return true
	read_line(fmt("%s/current.txt", g_DataDir), now, charsmax(now))
	return equali(mode, now) != 0
}

// `leaving`: a player on their way out (still counted as connected)
check_empty(leaving)
{
	remove_task(TASK_EMPTY)
	new minutes = get_pcvar_num(g_pEmpty)
	if (g_Career || minutes <= 0 || humans(leaving) > 0 || at_default())
		return
	set_task(clamp(minutes, 1, 120) * 60.0, "task_empty", TASK_EMPTY)
}

public task_empty()
{
	if (g_Career || get_pcvar_num(g_pEmpty) <= 0 || humans() > 0 || at_default())
		return
	// someone still loading in counts too: try again later
	for (new i = 1; i <= MaxClients; i++)
	{
		if (is_user_connecting(i) && !is_user_bot(i) && !is_user_hltv(i))
		{
			set_task(60.0, "task_empty", TASK_EMPTY)
			return
		}
	}
	new map[32], mode[32]
	get_pcvar_string(g_pEmptyMap, map, charsmax(map))
	get_pcvar_string(g_pEmptyMode, mode, charsmax(mode))
	new bool:menu = is_plugin_loaded("gamemodes.amxx", true) != -1
	server_print("[extras] nobody is playing: back to %s", map)
	if (menu && mode[0] && plain_name(mode))
		server_cmd("gm_mode %s %s", mode, map)
	else if (menu)
		server_cmd("gm_map %s", map)
	else
		server_cmd("changelevel %s", map)
}

// ---------------------------------------------------------------------------
// Daily restart warning (sent by the app)
// ---------------------------------------------------------------------------

public srv_warn()
{
	new arg[8], text[96]
	read_argv(1, arg, charsmax(arg))
	new minutes = str_to_num(arg)
	if (minutes <= 0)
		copy(text, charsmax(text), "The server is restarting now. Rejoin in a minute.")
	else if (minutes == 1)
		copy(text, charsmax(text), "The server restarts in 1 minute.")
	else
		formatex(text, charsmax(text), "The server restarts in %d minutes.", minutes)
	set_hudmessage(255, 220, 80, -1.0, 0.3, 0, 0.0, 8.0, 0.3, 0.5, -1)
	show_hudmessage(0, "%s", text)
	client_print_color(0, print_team_red, "%s ^3%s", TAG, text)
	return PLUGIN_HANDLED
}
