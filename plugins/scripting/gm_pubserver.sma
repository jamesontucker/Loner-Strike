/*
	gm_pubserver - makes a bot game feel like a public server.

	Regulars:   bots get names (with clan tags) from configs/gamemodes/regular_names.ini.
	            Each CZ bot profile keeps its own name for good (data/gamemodes/regulars.ini),
	            so the same regulars keep turning up; some bots join as one-off newcomers.
	            Tags stay rare: a clan is at most three on the server, and tagged names
	            are a small part of it (tag_fits). Names with a year in them are skipped.
	Come & go:  now and then a bot leaves and someone new takes the slot (gm_churn).
	RTV:        "rtv" starts a map vote once enough players want one; "nominate <map>".
	            With gm_rtv_cycle 1 (gm_extras) both offer only the map rotation's maps.
	Welcome:    a welcome screen when you join (gm_welcome), replacing the CZ advert.
	Clans:      bots with the same clan tag play on the same side (gm_clans): just before
	            a round starts, a clan member on the other side swaps with a bot with no
	            clan, so team sizes stay the same. Only in plain team modes, never in a match.

	Modes:      with gm_modevote 1, rock the vote picks the mode too: "rtv" (or "rtm",
	            "rockthemode", "votemode") starts it once gm_modevote_ratio percent of the
	            people want it (not in a map's first 3 minutes, nor for 10 minutes after a
	            vote that changed nothing). First the mode ("Keep <mode>", then up to 4 others:
	            nominated ones first, "nominatemode <name>"), then a map for it: the maps that
	            suit the winning mode (gamemodes.amxx's gm_modes_maps). Keep wins: the usual
	            map vote. The switch goes through the Game Mode Menu, so the mode's own saved
	            settings, plugins and bot personalities apply and the bot count stays.
	            Votable modes: data/gamemodes/votemodes.ini (the app's Rules page), else
	            configs/gamemodes/votemodes.ini, else the built-in list; a mode with no map
	            that suits it is left out. gm_modevote_every N: every Nth map ends with the
	            same mode-then-map vote instead of mapchooser's (paused for that map); the
	            pick waits for the map to end. "gm_votemode" (admin, or the app) starts one.
	            With gm_modevote 0 rock the vote and the end of the map work as before.
	Room:       with the bot count on "fill", a person joining a team makes a bot leave. The
	            game would kick the first bot in the list (the longest-standing regulars);
	            here a bot that's dead, a newcomer, and new on the server goes instead, and
	            it says it's off a moment after the person connects, before they pick a team.
	Life:       bots join a few at a time when a later map starts (gm_botjoin; on the first, all at once), say they're
	            going before they leave (gm_churn), and some come back later; now and
	            then one has tweaked its name. Bots join in on rock the vote and vote
	            for maps (gm_botvotes); people's votes decide, bots only break ties.
	Cvars: gm_regulars 0/1, gm_churn 0/1/2, gm_rtv 0/1, gm_welcome 0/1/2, gm_clans 0/1,
	       gm_botjoin 0/1, gm_botvotes 0/1, gm_modevote 0/1, gm_modevote_ratio 1-100,
       gm_modevote_every 0/2/3/5
*/

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>
#include <cstrike>

// gm_botchat (optional): bots' lines
native botchat_say(id, const section[], other, Float:chance)
native botchat_left(id)
native people_get(id, const key[])
native people_alias(id, const name[])

// gamemodes.amxx (library gm_modes): the modes, the maps that suit one, switching
native gm_modes_count()
native gm_modes_info(mode, id[], idlen, name[], namelen)
native gm_modes_current()
native gm_modes_maps(mode, Array:out)
native gm_modes_go(mode, const map[])
native gm_modes_next(mode, const map[])

#define PLUGIN  "Pub server"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_CHURN   5100
#define TASK_VOTE    5200
#define TASK_CHANGE  5300
#define TASK_WELCOME 5400
#define TASK_CLANS   5500
#define TASK_ENDVOTE 5600
#define TASK_BYE     5700   // a bot saying it's off to make room (+ userid)

#define VOTE_TIME    15
#define VOTE_MAPS    5
#define MAX_NOMS     4
#define NEWCOMER_PCT 35
#define VOTE_MODES   4      // modes in the mode vote besides "Keep"
#define MODE_WAIT    180    // seconds into a map before a vote that can change the mode
#define MODE_RETRY   600    // seconds after a vote that changed nothing
#define END_LEAD     140    // the map-end vote starts with this much time left (mapchooser: 129)

new g_pRegulars, g_pChurn, g_pRtv, g_pWelcome, g_pClans, g_pBotJoin, g_pBotVotes, g_pRampTarget
new bool:g_Chat
new Array:g_LeftNames, Array:g_LeftAt   // bots that left this map, and when
new g_RealName[33][32]           // a bot's own name when it shows up as a variant ("Dave_" is Dave)
new g_RampTarget, g_RampSet             // joining a few at a time
new g_BotVote[VOTE_MAPS + 1]            // bots' votes, kept apart from people's
new bool:g_Career
new bool:g_Creating

new Array:g_Pool                 // every name in regular_names.ini
new Trie:g_InPool                // the same names, to look up
new Trie:g_Regular               // profile name -> regular name
new Trie:g_Taken                 // regular names already given to a profile
new g_DataDir[128], g_CfgDir[128]

new bool:g_Rocked[33]
new g_NomBy[MAX_NOMS], g_Noms[MAX_NOMS][32], g_NomCount
new g_WelcomeFor
new bool:g_Voting, bool:g_VoteDone
new g_VoteMaps[VOTE_MAPS + 1][32], g_VoteCount[VOTE_MAPS + 1], g_VoteOptions
new Float:g_MapStart
new Array:g_MapPool

// the mode vote
new g_pModeVote, g_pModeRatio, g_pModeEvery, g_pModeRotate
new g_MNomBy[MAX_NOMS], g_MNoms[MAX_NOMS], g_MNomCount        // nominated modes (mode indexes)
new g_MVMode[VOTE_MODES + 1], g_MVCount[VOTE_MODES + 1], g_MVBot[VOTE_MODES + 1], g_MVOptions
new bool:g_ModeFlow          // this vote can change the mode (rtv with mode voting on)
new bool:g_EndVote           // this vote runs because the map is ending
new bool:g_EndDue            // this map ends with a mode-then-map vote
new bool:g_EndStarted        // ... and it has started
new bool:g_McPaused          // mapchooser paused for this map
new bool:g_Changing          // a vote's map change is on its way
new bool:g_BothZombies       // "zombie" and "zp5" are both in the vote (zp5 keeps its full name)
new g_StageMode = -1         // the map stage: maps for this mode
new Float:g_BotSince[33]     // when each bot joined
new g_LeaverFor[33]          // a person who just connected: the bot that leaves for them (userid)
new bool:g_Leaving[33]       // this bot is the one going for someone
new bool:g_IsRegular[33]     // this bot is a returning regular (a quota kick takes a newcomer first)
new bool:g_NoLeft            // gm_botchat has no botchat_left (an older one)
new bool:g_NoAlias           // gm_people has no people_alias (an older one)
new g_Score[2]

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pRegulars = register_cvar("gm_regulars", "1")
	g_pChurn    = register_cvar("gm_churn", "1")
	g_pRtv      = register_cvar("gm_rtv", "1")
	g_pWelcome  = register_cvar("gm_welcome", "1")
	g_pClans    = register_cvar("gm_clans", "1")
	g_pBotJoin  = register_cvar("gm_botjoin", "1")
	g_pBotVotes = register_cvar("gm_botvotes", "1")
	g_pModeVote  = register_cvar("gm_modevote", "1")
	g_pModeRatio = register_cvar("gm_modevote_ratio", "50")
	g_pModeEvery = register_cvar("gm_modevote_every", "0")
	// what happens to the mode every Nth map: 0 a vote, 1 the next mode on the list, 2 a random one
	g_pModeRotate = register_cvar("gm_moderotate", "0")
	// the full bot count while they trickle in (0 otherwise), so the Game Mode Menu
	// saves that and not the low live bot_quota; it sets this to 0 to end the ramp
	g_pRampTarget = register_cvar("gm_botjoin_target", "0")
	set_pcvar_num(g_pRampTarget, 0)
	g_Chat = bool:LibraryExists("gm_botchat", LibType_Library)
	g_LeftNames = ArrayCreate(32)
	g_LeftAt = ArrayCreate(1)
	// conditions in one register_event must all match, so one per sound
	register_event("SendAudio", "ev_round_end", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_round_end", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "ev_round_end", "a", "2&%!MRAD_rounddraw")

	register_forward(FM_CreateFakeClient, "fw_create_fake_client")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say")
	register_clcmd("gm_welcome_show", "cmd_welcome")
	register_concmd("gm_votemode", "cmd_votemode", ADMIN_VOTE, "- starts a game mode vote, then a map vote")
	register_event("TeamScore", "ev_teamscore", "a")
	// a person picking a team (VGUI and the old menu): a bot makes room first, our choice
	register_clcmd("jointeam", "cmd_jointeam")
	register_menucmd(register_menuid("Team_Select", 1), 1023, "menu_team")

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	load_names()
	g_MapStart = get_gametime()
	set_task(15.0, "task_churn", TASK_CHURN, _, _, "b")
	set_task(5.0, "task_end_check", TASK_ENDVOTE, _, _, "b")
}

public plugin_natives()
{
	set_native_filter("native_filter")
}

public native_filter(const name[], index, trap)
{
	if (!trap && equal(name, "botchat_left"))
		g_NoLeft = true
	if (!trap && equal(name, "people_alias"))
		g_NoAlias = true
	return (!trap && (equal(name, "botchat_", 8) || equal(name, "gm_modes_", 9) || equal(name, "people_", 7))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

say_line(id, const section[], Float:chance)
{
	if (g_Chat)
		botchat_say(id, section, 0, chance)
}

public plugin_cfg()
{
	// after the Game Mode Menu has put the saved bot count back (1 s and 4 s in)
	set_task(6.0, "task_ramp_start")
	// Tour of Duty missions keep their designed bots (same check as gm_humanbots)
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
	// The game would show motd.txt (the CZ advert) to everyone who joins; a file that
	// doesn't exist means no window, and our own welcome screen takes its place.
	if (!g_Career)
		set_cvar_string("motdfile", "addons/amxmodx/data/gamemodes/no_motd.txt")
	remember_mode()
	// every Nth map ends with a mode-then-map vote (maps played since the last one)
	new every = get_pcvar_num(g_pModeEvery), since[8]
	get_localinfo("gm_mvmaps", since, charsmax(since))
	g_EndDue = every > 0 && str_to_num(since) >= every - 1
}

public plugin_end()
{
	if (!g_Career)
		set_cvar_string("motdfile", "motd.txt")
	// a map change in the middle of bots trickling in: the full count carries over
	if (g_RampTarget && get_cvar_num("bot_quota") == g_RampSet)
		set_cvar_num("bot_quota", g_RampTarget)
	set_pcvar_num(g_pRampTarget, 0)
	// count the maps played to the end (not a reload a moment after loading)
	if (get_gametime() - g_MapStart > 60.0)
	{
		new since[8]
		get_localinfo("gm_mvmaps", since, charsmax(since))
		set_localinfo("gm_mvmaps", g_EndStarted ? "0" : fmt("%d", min(str_to_num(since) + 1, 99)))
	}
}

// ---------------------------------------------------------------------------
// Regulars: rename bots as the game creates them
// ---------------------------------------------------------------------------
load_names()
{
	g_Pool = ArrayCreate(32)
	g_InPool = TrieCreate()
	g_Regular = TrieCreate()
	g_Taken = TrieCreate()
	new line[64], f = fopen(fmt("%s/gamemodes/regular_names.ini", g_CfgDir), "rt")
	if (f)
	{
		while (fgets(f, line, charsmax(line)))
		{
			trim(line)
			// a name with a quote, or starting with #, can't be kicked by name ("bot_kick"),
			// so such a bot would never leave
			// and no birth years or "2004" in names: nobody's name dates the server
			if (line[0] && line[0] != ';' && line[0] != '#' && contain(line, "^"") == -1 && strlen(line) < 32 && !has_year(line))
			{
				ArrayPushString(g_Pool, line)
				TrieSetCell(g_InPool, line, 1)
			}
		}
		fclose(f)
	}
	f = fopen(fmt("%s/regulars.ini", g_DataDir), "rt")
	if (f)
	{
		new profile[32], name[32]
		while (fgets(f, line, charsmax(line)))
		{
			trim(line)
			if (!line[0] || line[0] == ';')
				continue
			name[0] = 0
			strtok2(line, profile, charsmax(profile), name, charsmax(name), '=', TRIM_FULL)
			// a name taken out of regular_names.ini since is handed out fresh
			if (profile[0] && name[0] && (!ArraySize(g_Pool) || TrieKeyExists(g_InPool, name)))
			{
				TrieSetString(g_Regular, profile, name)
				TrieSetCell(g_Taken, name, 1)
			}
		}
		fclose(f)
	}
}

// A year in the name: four digits in a row from 1950 to 2039 ("nacho2004", "LAN party 2003")
bool:has_year(const name[])
{
	for (new i = 0; name[i]; i++)
	{
		if (!isdigit(name[i]) || (i > 0 && isdigit(name[i - 1])))
			continue
		new n, v
		while (isdigit(name[i + n]))
		{
			v = v * 10 + name[i + n] - '0'
			n++
		}
		if (n == 4 && v >= 1950 && v <= 2039)
			return true
	}
	return false
}

// How many on the server wear `tag`, and how many wear any tag at all
clan_count(const tag[], &tagged)
{
	new n, other[16]
	tagged = 0
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i))
			continue
		clan_of(i, other, charsmax(other))
		if (!other[0])
			continue
		tagged++
		if (tag[0] && equali(other, tag))
			n++
	}
	return n
}

// Most people on a pub have no tag, and a clan is two or three of them: a tagged name
// is fine while its clan has fewer than three here and tags stay a small part of the server
bool:tag_fits(const name[])
{
	new tag[16], tagged, players
	clan_of_name(name, tag, charsmax(tag))
	if (!tag[0])
		return true
	if (clan_count(tag, tagged) >= 3)
		return false
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i))
			players++
	return tagged < max(2, players / 5)
}

bool:name_in_use(const name[])
{
	new other[32]
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i))
			continue
		get_user_name(i, other, charsmax(other))
		if (equali(other, name))
			return true
	}
	return false
}

// A pool name that isn't anyone's regular name and isn't on the server
bool:free_name(name[], len)
{
	new n = ArraySize(g_Pool)
	if (!n)
		return false
	for (new tries = 0; tries < 60; tries++)
	{
		ArrayGetString(g_Pool, random(n), name, len)
		// the last tries take any free name rather than none
		// (regulars off: their names are free to anyone)
		if ((!get_pcvar_num(g_pRegulars) || !TrieKeyExists(g_Taken, name)) && !name_in_use(name) && (tries >= 50 || tag_fits(name)))
			return true
	}
	return false
}

public fw_create_fake_client(const netname[])
{
	if (g_Creating || g_Career || !ArraySize(g_Pool))
		return FMRES_IGNORED

	new name[32], untweaked[32], bool:found, bool:regular
	// regulars off: the game's own names, except a second bot with a name already here (the
	// game would call it "(1)Dave", and which one is which would swap between maps): a free
	// name from the pool instead
	if (!get_pcvar_num(g_pRegulars))
	{
		if (!name_in_use(netname) || !free_name(name, charsmax(name)))
			return FMRES_IGNORED
		found = true
	}
	else if (TrieGetString(g_Regular, netname, name, charsmax(name)))
	{
		// the regular shows up most of the time; otherwise a newcomer takes the profile
		found = random(100) >= NEWCOMER_PCT && !name_in_use(name)
		regular = found
		if (!found)
		{
			// (someone who left and came back counts as a regular too: it's been here before,
			// and isn't the first to go again)
			found = came_back(name, charsmax(name))
			regular = found
			if (!found)
				found = clan_friend(name, charsmax(name)) || free_name(name, charsmax(name))
		}
		else
		{
			// (a small change to the name is the same person: gm_people is told who it is)
			copy(untweaked, charsmax(untweaked), name)
			tweak_name(name, charsmax(name))
			if (equal(untweaked, name))
				untweaked[0] = 0
		}
	}
	else
	{
		regular = came_back(name, charsmax(name))
		found = regular || clan_friend(name, charsmax(name)) || free_name(name, charsmax(name))
	}
	if (found && get_pcvar_num(g_pRegulars) && !TrieKeyExists(g_Regular, netname))
	{
		TrieSetString(g_Regular, netname, name)
		TrieSetCell(g_Taken, name, 1)
		new f = fopen(fmt("%s/regulars.ini", g_DataDir), "at")
		if (f)
		{
			fprintf(f, "%s = %s^n", netname, name)
			fclose(f)
		}
	}
	if (!found)
		return FMRES_IGNORED

	g_Creating = true
	new ent = engfunc(EngFunc_CreateFakeClient, name)
	g_Creating = false
	if (1 <= ent <= MaxClients)
	{
		g_IsRegular[ent] = regular
		copy(g_RealName[ent], charsmax(g_RealName[]), untweaked[0] ? untweaked : name)
		if (untweaked[0] && !g_NoAlias && LibraryExists("gm_people", LibType_Library))
			people_alias(ent, untweaked)
	}
	forward_return(FMV_CELL, ent)
	return FMRES_SUPERCEDE
}

// Now and then someone brings a clanmate: a free name from a clan already on the server,
// while that clan has fewer than three here
bool:clan_friend(out[], len)
{
	if (!get_pcvar_num(g_pClans) || random(100) >= 12)
		return false
	new here[33][16], n, tagged
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id))
		{
			clan_of(id, here[n], charsmax(here[]))
			if (here[n][0] && clan_count(here[n], tagged) < 3)
				n++
		}
	if (!n)
		return false
	new pick[16], name[32], tag[16], found, seen
	copy(pick, charsmax(pick), here[random(n)])
	for (new i = 0, size = ArraySize(g_Pool); i < size; i++)
	{
		ArrayGetString(g_Pool, i, name, charsmax(name))
		clan_of_name(name, tag, charsmax(tag))
		if (!equali(tag, pick) || name_in_use(name))
			continue
		// a random one of the free members
		if (random(++seen) == 0)
		{
			copy(out, len, name)
			found = true
		}
	}
	return bool:found
}

// ---------------------------------------------------------------------------
// Come and go: a bot leaves now and then and the bot count brings in someone new
// ---------------------------------------------------------------------------
public task_churn()
{
	new level = get_pcvar_num(g_pChurn)
	if (level <= 0 || g_Career || g_Voting || get_cvar_num("bot_quota") < 3 || get_cvar_num("gm_scrim_phase"))
		return
	// one leaves every ~4 minutes, or every ~90 seconds on "busy"
	if (random_float(0.0, 1.0) > 15.0 / (level >= 2 ? 90.0 : 240.0))
		return
	new map[32]
	get_mapname(map, charsmax(map))
	if (!file_exists(fmt("maps/%s.nav", map)))   // bots are still learning this map
		return

	// people tend to leave while they're dead
	new dead[32], alive[32], nd, na
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i) || is_user_hltv(i))
			continue
		if (is_user_alive(i))
			alive[na++] = i
		else
			dead[nd++] = i
	}
	// only dead bots leave: kicking a living one wrecks round-based modes
	// (eliminations, the deathrun activator, team swaps); try again later otherwise
	#pragma unused alive, na
	new id = nd ? dead[random(nd)] : 0
	if (!id)
		return
	// say bye first, like people do; remembered so they may come back later
	say_line(id, "leave", 1.0)
	new name[32]
	get_user_name(id, name, charsmax(name))
	// (by its own name: a "Dave_" who comes back is Dave, untweaked)
	if (g_RealName[id][0])
		copy(name, charsmax(name), g_RealName[id])
	ArrayPushString(g_LeftNames, name)
	ArrayPushCell(g_LeftAt, floatround(get_gametime()))
	set_task(random_float(2.5, 5.0), "task_kick", get_user_userid(id))
}

public task_kick(userid)
{
	new id = find_player("k", userid)
	if (id && is_user_bot(id))
		server_cmd("kick #%d", userid)
}

// ---- joining a few at a time ---------------------------------------------------------

public task_ramp_start()
{
	// the first map after the game (or server) starts has everyone in at once: nobody is
	// waiting on a filling server then. localinfo outlasts map changes, not a restart
	new first[4]
	get_localinfo("gm_botjoin_seen", first, charsmax(first))
	if (!first[0])
	{
		set_localinfo("gm_botjoin_seen", "1")
		return
	}
	new q = get_cvar_num("bot_quota")
	if (!get_pcvar_num(g_pBotJoin) || g_Career || q < 4 || get_cvar_num("gm_scrim_phase"))
		return
	// only the ones not in yet trickle in: lowering the count would kick bots already playing
	new now
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i))
			now++
	if (now >= q - 1)
		return
	g_RampTarget = q
	g_RampSet = max(max(2, q / 3), now)
	set_pcvar_num(g_pRampTarget, q)
	set_cvar_num("bot_quota", g_RampSet)
	set_task(random_float(3.0, 9.0), "task_ramp")
}

public task_ramp()
{
	// someone changed the bot count meanwhile, or the menu ended the ramp: theirs wins
	if (get_cvar_num("bot_quota") != g_RampSet || !get_pcvar_num(g_pRampTarget))
	{
		end_ramp()
		return
	}
	g_RampSet = min(g_RampTarget, g_RampSet + random_num(1, 2))
	set_cvar_num("bot_quota", g_RampSet)
	if (g_RampSet < g_RampTarget)
		set_task(random_float(3.0, 12.0), "task_ramp")
	else
		end_ramp()
}

end_ramp()
{
	g_RampTarget = 0
	set_pcvar_num(g_pRampTarget, 0)
}



// Someone who left earlier comes back now and then (under the same name)
bool:came_back(out[], len)
{
	new n = ArraySize(g_LeftNames)
	if (!n || random(100) >= 30)
		return false
	new now = floatround(get_gametime())
	for (new tries = 0; tries < 4; tries++)
	{
		new i = random(n)
		if (now - ArrayGetCell(g_LeftAt, i) < 90)
			continue
		ArrayGetString(g_LeftNames, i, out, len)
		if (name_in_use(out))
			continue
		ArrayDeleteItem(g_LeftNames, i)
		ArrayDeleteItem(g_LeftAt, i)
		return true
	}
	return false
}

// People change their names now and then: a small tweak on a regular's name
tweak_name(name[], len)
{
	if (random(100) >= 5)
		return
	static const ENDS[][] = { "^^", "_", ".", "x", "2", "!", "~", "-" }
	new try[32]
	copy(try, charsmax(try), name)
	if (random(3) == 0)
		strtolower(try)
	else
		add(try, charsmax(try), ENDS[random(sizeof ENDS)])
	if (strlen(try) < 31 && !name_in_use(try))
		copy(name, len, try)
}

// ---------------------------------------------------------------------------
// Rock the vote
// ---------------------------------------------------------------------------
public cmd_say(id)
{
	new msg[64], word[16], rest[40]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	strtok2(msg, word, charsmax(word), rest, charsmax(rest), ' ', TRIM_FULL)
	if (word[0] == '/' || word[0] == '!')
		copy(word, charsmax(word), word[1])

	new bool:modeWord = equali(word, "rtm") || equali(word, "rockthemode") || equali(word, "votemode")
	if (modeWord || equali(word, "rtv") || equali(word, "rockthevote"))
	{
		// bots join in once a person has asked; a bot's rtv alone starts nothing
		if (is_user_bot(id))
		{
			if (get_pcvar_num(g_pBotVotes) && human_rocked())
				rock(id, modeWord)
			return PLUGIN_CONTINUE
		}
		rock(id, modeWord)
		if (get_pcvar_num(g_pBotVotes))
			for (new b = 1; b <= MaxClients; b++)
				if (is_user_connected(b) && is_user_bot(b) && random(100) < (modeWord ? 20 : 30))
					set_task(random_float(2.0, 9.0), modeWord ? "task_bot_rtm" : "task_bot_rtv", get_user_userid(b))
		return PLUGIN_CONTINUE      // others see it in chat, like on a real server
	}
	if (equali(word, "nominatemode") || equali(word, "nommode"))
	{
		nominate_mode(id, rest)
		return PLUGIN_HANDLED
	}
	if (equali(word, "nominate") || equali(word, "nom"))
	{
		nominate(id, rest)
		return PLUGIN_HANDLED
	}
	if (equali(word, "welcome") || equali(word, "motd") || equali(word, "rules"))
	{
		show_welcome(id)
		return PLUGIN_HANDLED
	}
	return PLUGIN_CONTINUE
}

humans()
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			n++
	return n
}

bool:human_rocked()
{
	for (new i = 1; i <= MaxClients; i++)
		if (g_Rocked[i] && is_user_connected(i) && !is_user_bot(i))
			return true
	return false
}

public task_bot_rtv(userid)
{
	bot_rock(userid, false)
}

public task_bot_rtm(userid)
{
	bot_rock(userid, true)
}

bot_rock(userid, bool:modeWord)
{
	new id = find_player("k", userid)
	if (!id || !is_user_bot(id) || g_Voting || g_Changing || (g_VoteDone && !mode_voting()))
		return
	// shown in chat (a bot's own command doesn't reach the say hook); like every bot's
	// rtv it doesn't count towards the vote
	engclient_cmd(id, "say", modeWord ? "rtm" : "rtv")
	rock(id, modeWord)
}

// people only, like needed(): a bot's rtv is chat flavour and doesn't count
rocked()
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (g_Rocked[i] && is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			n++
	return n
}

needed()
{
	// with mode voting: gm_modevote_ratio percent of the people (half by default), rounded up
	if (mode_voting())
		return max(1, (humans() * clamp(get_pcvar_num(g_pModeRatio), 1, 100) + 99) / 100)
	return max(1, floatround(humans() * 0.6, floatround_ceil))
}

rock(id, bool:modeWord = false)
{
	new bool:modes = mode_voting()
	if (modeWord && !modes)
		return client_print_color(id, print_team_default, "^4[Server]^1 Mode voting is turned off.")
	if (!modeWord && !get_pcvar_num(g_pRtv))
		return client_print_color(id, print_team_default, "^4[Server]^1 Rock the vote is turned off.")
	if (g_Voting)
		return client_print_color(id, print_team_default, "^4[Server]^1 A vote is already running.")
	if (g_Changing)
		return client_print_color(id, print_team_default, "^4[Server]^1 The map is about to change.")
	new wait
	if (modes)
	{
		if (g_EndStarted)
			return client_print_color(id, print_team_default, "^4[Server]^1 The next map is already picked.")
		wait = MODE_WAIT - floatround(get_gametime() - g_MapStart)
		if (wait > 0)
			return client_print_color(id, print_team_default, "^4[Server]^1 You can rock the vote in %d:%02d.", wait / 60, wait % 60)
		wait = mode_cooldown()
		if (wait > 0)
			return client_print_color(id, print_team_default, "^4[Server]^1 The last vote kept everything as it was. The next one can start in %d:%02d.", wait / 60, wait % 60)
	}
	else
	{
		if (g_VoteDone)
			return client_print_color(id, print_team_default, "^4[Server]^1 The map vote already happened on this map.")
		wait = 30 - floatround(get_gametime() - g_MapStart)
		if (wait > 0)
			return client_print_color(id, print_team_default, "^4[Server]^1 You can rock the vote in %d seconds.", wait)
	}
	if (g_Rocked[id])
		return client_print_color(id, print_team_default, "^4[Server]^1 You already rocked the vote (%d of %d).", rocked(), needed())

	g_Rocked[id] = true
	new name[32]
	get_user_name(id, name, charsmax(name))
	if (rocked() >= needed())
	{
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 rocked the vote. Starting a %s vote...", name, modes ? "mode" : "map")
		set_task(1.5, modes ? "start_mode_vote" : "start_vote", TASK_VOTE)
		g_Voting = true
	}
	else
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 wants a %s (%d of %d). Say ^4%s^1 to agree.", name, modes ? "different mode or map" : "new map", rocked(), needed(), modeWord ? "rtm" : "rtv")
	return PLUGIN_HANDLED
}

public client_disconnected(id)
{
	g_Rocked[id] = false
	// a person who never picked a team: the bot that was going for them stays after all
	release_leaver(id)
	g_Leaving[id] = false
	g_IsRegular[id] = false
	g_RealName[id][0] = 0
	for (new i = 0; i < g_MNomCount; i++)
	{
		if (g_MNomBy[i] != id)
			continue
		g_MNomCount--
		for (new j = i; j < g_MNomCount; j++)
		{
			g_MNomBy[j] = g_MNomBy[j + 1]
			g_MNoms[j] = g_MNoms[j + 1]
		}
		break
	}
	for (new i = 0; i < g_NomCount; i++)
	{
		if (g_NomBy[i] != id)
			continue
		g_NomCount--
		for (new j = i; j < g_NomCount; j++)
		{
			g_NomBy[j] = g_NomBy[j + 1]
			copy(g_Noms[j], charsmax(g_Noms[]), g_Noms[j + 1])
		}
		break
	}
}

bool:read_list(const file[], Trie:into)
{
	new f = fopen(fmt("%s/%s", g_DataDir, file), "rt")
	if (!f)
		return false
	new line[40]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		strtolower(line)
		if (line[0])
			TrieSetCell(into, line, 1)
	}
	fclose(f)
	return true
}

// Maps worth offering: favourites plus every map bots can play (has navigation),
// minus hidden maps and the current one.
build_pool()
{
	if (g_MapPool)
		return
	g_MapPool = ArrayCreate(32)
	// the host's map rotation only (gm_extras' setting), when it has other maps
	if (get_cvar_num("gm_rtv_cycle") && rotation_pool())
		return
	new Trie:hidden = TrieCreate(), Trie:seen = TrieCreate(), Trie:fav = TrieCreate()
	read_list("hidden_maps.txt", hidden)
	read_list("favorites.txt", fav)
	new cur[32]
	get_mapname(cur, charsmax(cur))
	strtolower(cur)
	TrieSetCell(seen, cur, 1)

	new file[64], len, dir = open_dir("maps", file, charsmax(file))
	if (dir)
	{
		do
		{
			len = strlen(file)
			if (len < 5 || len > 35 || !equali(file[len - 4], ".bsp"))
				continue
			file[len - 4] = 0
			strtolower(file)
			if (TrieKeyExists(seen, file) || TrieKeyExists(hidden, file))
				continue
			if (!plain_name(file))              // the name goes into "gm_map <name>"
				continue
			if (!TrieKeyExists(fav, file) && !file_exists(fmt("maps/%s.nav", file)))
				continue
			TrieSetCell(seen, file, 1)
			ArrayPushString(g_MapPool, file)
		}
		while (next_file(dir, file, charsmax(file)))
		close_dir(dir)
	}
	TrieDestroy(hidden)
	TrieDestroy(seen)
	TrieDestroy(fav)
}

// ---- clans: clanmates on the same side ------------------------------------------------

// Plain team modes, where who's on which side doesn't matter to the mode
new const CLAN_MODES[][] = { "classic", "csdm", "gungame_team", "knife", "pistols", "deagle", "awp", "scoutz", "paintball", "war3", "shero" }

public ev_round_end()
{
	// after the round's over, before the next one spawns everyone
	if (get_pcvar_num(g_pClans) && !g_Career)
		set_task(3.5, "task_group_clans", TASK_CLANS)
}

public task_group_clans()
{
	if (get_cvar_num("gm_scrim_phase"))
		return
	new mode[32]
	new f = fopen(fmt("%s/current.txt", g_DataDir), "rt")
	if (f)
	{
		fgets(f, mode, charsmax(mode))
		trim(mode)
		fclose(f)
	}
	new bool:ok
	for (new i = 0; i < sizeof CLAN_MODES; i++)
		if (equal(mode, CLAN_MODES[i]))
			ok = true
	if (!ok)
		return

	new tags[33][16]
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id) && is_user_bot(id))
			clan_of(id, tags[id], charsmax(tags[]))

	// one swap per round is enough to look natural
	for (new a = 1; a <= MaxClients; a++)
	{
		// only the dead change sides (a living one would stand on the other team, maybe with the bomb)
		if (!tags[a][0] || !is_user_connected(a) || is_user_alive(a))
			continue
		new CsTeams:ta = cs_get_user_team(a)
		if (ta != CS_TEAM_T && ta != CS_TEAM_CT)
			continue
		// how many of the clan are on each side
		new onT, onCT
		for (new b = 1; b <= MaxClients; b++)
		{
			if (!tags[b][0] || !equali(tags[a], tags[b]) || !is_user_connected(b))
				continue
			switch (cs_get_user_team(b))
			{
				case CS_TEAM_T: onT++
				case CS_TEAM_CT: onCT++
			}
		}
		if (!onT || !onCT)
			continue
		new CsTeams:home = onT >= onCT ? CS_TEAM_T : CS_TEAM_CT
		if (ta == home)
			continue
		// a bot with no clan on the clan's side takes this one's place
		for (new c = 1; c <= MaxClients; c++)
		{
			if (c == a || tags[c][0] || !is_user_connected(c) || !is_user_bot(c) || is_user_alive(c) || cs_get_user_team(c) != home)
				continue
			cs_set_user_team(a, home)
			cs_set_user_team(c, ta)
			return
		}
	}
}

// The clan in a name, as its letters: "~WoW~ Squishy", "[XyZ] Name", "GG | Phantom" (as gm_botchat reads it)
clan_of(id, tag[], len)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	clan_of_name(name, tag, len)
}

clan_of_name(const source[], tag[], len)
{
	tag[0] = 0
	new name[32]
	copy(name, charsmax(name), source)
	trim(name)
	// "[xDS]Bob", "-=[CoC]=- Bob", "~WoW~ Squishy", "{SoB} Rex": symbols, the clan's letters,
	// symbols, then the name. A name that's only decorated ("~*Leek*~") has no clan.
	new i, k
	while (name[i] && !isalnum(name[i]) && name[i] != ' ')
		i++
	if (i > 0)
	{
		while (name[i] && isalnum(name[i]) && k < len)
			tag[k++] = name[i++]
		tag[k] = 0
		new sym
		while (name[i] && !isalnum(name[i]) && name[i] != ' ')
		{
			i++
			sym++
		}
		while (name[i] == ' ')
			i++
		if (!sym || !name[i])
			tag[0] = 0
	}
	else
	{
		// "GG | Phantom", "XyZ|Name": a short word before a bar
		new bar = contain(name, "|")
		if (bar > 0 && bar <= 7)
		{
			for (new j = 0; j < bar && k < len; j++)
				if (isalnum(name[j]))
					tag[k++] = name[j]
			tag[k] = 0
		}
	}
	if (strlen(tag) < 2)
		tag[0] = 0
}

// The rotation gm_extras uses: the current mode's own list, else the game's map cycle
bool:rotation_pool()
{
	new mode[32], cycle[64]
	new f = fopen(fmt("%s/current.txt", g_DataDir), "rt")
	if (f)
	{
		fgets(f, mode, charsmax(mode))
		trim(mode)
		fclose(f)
	}
	if (mode[0] && plain_name(mode))
		pool_from(fmt("%s/rotation_%s.txt", g_DataDir, mode))
	if (!ArraySize(g_MapPool))
	{
		get_cvar_string("mapcyclefile", cycle, charsmax(cycle))
		pool_from(cycle[0] ? cycle : "mapcycle.txt")
	}
	return ArraySize(g_MapPool) > 0
}

pool_from(const path[])
{
	new f = fopen(path, "rt")
	if (!f)
		return
	new line[64], map[32], cur[32]
	get_mapname(cur, charsmax(cur))
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (!line[0] || line[0] == ';' || (line[0] == '/' && line[1] == '/'))
			continue
		parse(line, map, charsmax(map))
		new len = strlen(map)
		if (len > 4 && equali(map[len - 4], ".bsp"))
			map[len - 4] = 0
		if (!plain_name(map) || equali(map, cur) || !is_map_valid(map))
			continue
		new bool:dup
		for (new i = 0; i < ArraySize(g_MapPool) && !dup; i++)
		{
			new have[32]
			ArrayGetString(g_MapPool, i, have, charsmax(have))
			dup = bool:equali(have, map)
		}
		if (!dup)
			ArrayPushString(g_MapPool, map)
	}
	fclose(f)
}

bool:plain_name(const name[])
{
	for (new i = 0; name[i]; i++)
		if (!isalnum(name[i]) && name[i] != '_' && name[i] != '-' && name[i] != '.')
			return false
	return true
}

nominate(id, const query[])
{
	if (!get_pcvar_num(g_pRtv))
		return PLUGIN_HANDLED
	if (g_Voting || g_VoteDone)
		return client_print_color(id, print_team_default, "^4[Server]^1 Too late to nominate on this map.")
	build_pool()
	new menu = menu_create(query[0] ? fmt("\yNominate a map^n\dMatching ^"%s^"", query) : "\yNominate a map", "nominate_handler")
	new map[32], found, only[32]
	for (new i = 0; i < ArraySize(g_MapPool); i++)
	{
		ArrayGetString(g_MapPool, i, map, charsmax(map))
		if (query[0] && containi(map, query) < 0)
			continue
		menu_additem(menu, map, map)
		copy(only, charsmax(only), map)
		found++
	}
	if (!found)
	{
		menu_destroy(menu)
		return client_print_color(id, print_team_default, "^4[Server]^1 No map matches ^"%s^".", query)
	}
	if (found == 1)
	{
		menu_destroy(menu)
		return add_nomination(id, only)
	}
	menu_display(id, menu)
	return PLUGIN_HANDLED
}

public nominate_handler(id, menu, item)
{
	if (item >= 0)
	{
		new map[32], access, cb
		menu_item_getinfo(menu, item, access, map, charsmax(map), _, _, cb)
		add_nomination(id, map)
	}
	menu_destroy(menu)
	return PLUGIN_HANDLED
}

add_nomination(id, const map[])
{
	for (new i = 0; i < g_NomCount; i++)
		if (equali(g_Noms[i], map))
			return client_print_color(id, print_team_default, "^4[Server]^1 %s is already nominated.", map)
	new slot = -1
	for (new i = 0; i < g_NomCount; i++)
		if (g_NomBy[i] == id)
			slot = i                       // replace your earlier nomination
	if (slot < 0)
	{
		if (g_NomCount >= MAX_NOMS)
			return client_print_color(id, print_team_default, "^4[Server]^1 The nomination list is full.")
		slot = g_NomCount++
	}
	g_NomBy[slot] = id
	copy(g_Noms[slot], charsmax(g_Noms[]), map)
	new name[32]
	get_user_name(id, name, charsmax(name))
	client_print_color(0, print_team_default, "^4[Server]^3 %s^1 nominated ^4%s^1.", name, map)
	return PLUGIN_HANDLED
}

public start_vote()
{
	build_pool()
	g_VoteOptions = 0
	for (new i = 0; i < g_NomCount && g_VoteOptions < VOTE_MAPS; i++)
		copy(g_VoteMaps[g_VoteOptions++], charsmax(g_VoteMaps[]), g_Noms[i])
	new n = ArraySize(g_MapPool), map[32]
	for (new tries = 0; g_VoteOptions < VOTE_MAPS && tries < 200 && n; tries++)
	{
		ArrayGetString(g_MapPool, random(n), map, charsmax(map))
		new bool:dup
		for (new j = 0; j < g_VoteOptions; j++)
			if (equali(g_VoteMaps[j], map))
				dup = true
		if (!dup)
			copy(g_VoteMaps[g_VoteOptions++], charsmax(g_VoteMaps[]), map)
	}
	if (!g_VoteOptions)
	{
		g_Voting = false
		vote_settled(false)
		return client_print_color(0, print_team_default, "^4[Server]^1 There are no other maps to vote for.")
	}
	new cur[32]
	get_mapname(cur, charsmax(cur))
	copy(g_VoteMaps[g_VoteOptions], charsmax(g_VoteMaps[]), cur)   // last option: keep playing
	arrayset(g_VoteCount, 0, sizeof g_VoteCount)
	arrayset(g_BotVote, 0, sizeof g_BotVote)
	if (get_pcvar_num(g_pBotVotes))
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 60)
				set_task(random_float(2.0, float(VOTE_TIME) - 2.0), "task_bot_vote", get_user_userid(b))

	new menu = menu_create(fmt("\yVote for the next map \d(%d seconds)", VOTE_TIME), "vote_handler")
	for (new i = 0; i < g_VoteOptions; i++)
		menu_additem(menu, g_VoteMaps[i])
	menu_additem(menu, fmt("Keep playing %s", cur))
	menu_setprop(menu, MPROP_EXIT, MEXIT_NEVER)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			menu_display(i, menu, 0, VOTE_TIME)
	new data[1]
	data[0] = menu
	set_task(float(VOTE_TIME) + 0.5, "end_vote", TASK_VOTE, data, sizeof data)
	client_print_color(0, print_team_default, "^4[Server]^1 Vote for the next map. Results in %d seconds.", VOTE_TIME)
	return PLUGIN_HANDLED
}

// A bot picks a map like a person would: the nominations and maps it likes a bit more
public task_bot_vote(userid)
{
	new id = find_player("k", userid)
	if (!g_Voting || !id || !is_user_bot(id))
		return
	new item = random(g_VoteOptions + 1)
	if (g_NomCount && random(100) < 40)
		item = random(min(g_NomCount, g_VoteOptions))
	g_BotVote[item]++
	new name[32]
	get_user_name(id, name, charsmax(name))
	if (item == g_VoteOptions)
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 wants to keep playing.", name)
	else
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 voted for ^4%s^1.", name, g_VoteMaps[item])
}

public vote_handler(id, menu, item)
{
	if (!g_Voting || item < 0 || item > g_VoteOptions)
		return PLUGIN_HANDLED
	g_VoteCount[item]++
	new name[32]
	get_user_name(id, name, charsmax(name))
	if (item == g_VoteOptions)
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 wants to keep playing.", name)
	else
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 voted for ^4%s^1.", name, g_VoteMaps[item])
	return PLUGIN_HANDLED
}

public end_vote(const data[])
{
	menu_destroy(data[0])    // also closes it for anyone who didn't vote
	g_Voting = false
	g_VoteDone = true
	// people's votes decide; bots' votes only break a tie between people's favourites
	// (or decide when no person voted)
	new best = -1, most = 0, ties
	new bool:people
	for (new i = 0; i <= g_VoteOptions; i++)
		if (g_VoteCount[i])
			people = true
	for (new i = 0; i <= g_VoteOptions; i++)
	{
		new v = people ? g_VoteCount[i] * 100 + g_BotVote[i] : g_BotVote[i]
		if (people && !g_VoteCount[i])
			continue
		if (v > most)
			best = i, most = v, ties = 1
		else if (v == most && most > 0 && random(++ties) == 0)
			best = i
	}
	most = best >= 0 ? g_VoteCount[best] + g_BotVote[best] : 0
	if (best >= 0)
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 12)
				say_line(b, "map_vote", 0.6)
	if (best < 0 || best == g_VoteOptions)
	{
		vote_settled(false)
		return client_print_color(0, print_team_default, "^4[Server]^1 Staying on ^4%s^1.", g_VoteMaps[g_VoteOptions])
	}
	vote_settled(true)
	g_Changing = true
	client_print_color(0, print_team_default, "^4[Server]^1 Next map: ^4%s^1 (%d vote%s). Changing in 5 seconds.", g_VoteMaps[best], most, most == 1 ? "" : "s")
	set_task(5.0, "task_change", TASK_CHANGE, g_VoteMaps[best], sizeof g_VoteMaps[])
	return PLUGIN_HANDLED
}

public task_change(const map[])
{
	// through the Game Mode Menu so the current mode carries over
	server_cmd("gm_map %s", map)
}

// ---------------------------------------------------------------------------
// Mode vote: rock the vote with the mode first, then a map for it
// ---------------------------------------------------------------------------

// Votable modes when neither votemodes.ini exists (zp5 shows as "Zombie Plague")
new const DEFAULT_VOTE_MODES[][] = { "classic", "csdm", "csdm_ffa", "gungame", "gungame_team", "deathrun", "basebuilder",
	"soccerjam", "zp5", "scoutz", "war3", "hns", "paintball", "shero" }

new g_Phase                   // 1: voting on modes, 2: voting on a map for a mode

bool:mode_voting()
{
	return get_pcvar_num(g_pModeVote) && !g_Career && !get_cvar_num("gm_scrim_phase")
		&& LibraryExists("gm_modes", LibType_Library) && gm_modes_current() >= 0
}

// Seconds until a vote may start after one that kept everything as it was
mode_cooldown()
{
	new buf[16]
	get_localinfo("gm_mvfail", buf, charsmax(buf))
	new at = str_to_num(buf), now = get_systime()
	if (at <= 0 || at > now)
		return 0
	return max(0, MODE_RETRY - (now - at))
}

// A vote that could change the mode is over. One that kept everything means a pause
// before the next; either way people can rock the vote again later on this map.
vote_settled(bool:changed)
{
	if (!g_ModeFlow)
		return
	g_ModeFlow = false
	if (!changed)
		set_localinfo("gm_mvfail", fmt("%d", get_systime()))
	g_VoteDone = false
	arrayset(g_Rocked, false, sizeof g_Rocked)
	g_NomCount = 0
	g_MNomCount = 0
}

current_mode_id(out[], len)
{
	out[0] = 0
	new f = fopen(fmt("%s/current.txt", g_DataDir), "rt")
	if (f)
	{
		fgets(f, out, len)
		trim(out)
		fclose(f)
	}
}

// The last three modes played (localinfo outlasts the map change), newest first
remember_mode()
{
	new id[32], list[100], out[100], tok[32], pos, n = 1
	current_mode_id(id, charsmax(id))
	if (!id[0] || !plain_name(id))
		return
	get_localinfo("gm_mvrecent", list, charsmax(list))
	copy(out, charsmax(out), id)
	while (n < 3 && (pos = argparse(list, pos, tok, charsmax(tok))) != -1)
	{
		if (equali(tok, id))
			continue
		add(out, charsmax(out), fmt(" %s", tok))
		n++
	}
	set_localinfo("gm_mvrecent", out)
}

bool:recent_mode(const id[])
{
	new list[100], tok[32], pos
	get_localinfo("gm_mvrecent", list, charsmax(list))
	while ((pos = argparse(list, pos, tok, charsmax(tok))) != -1)
		if (equali(tok, id))
			return true
	return false
}

// One mode id per line; ; # and // comments. Whether it named any.
bool:read_vote_list(const path[], Array:ids)
{
	new f = fopen(path, "rt")
	if (!f)
		return false
	new line[64], id[32]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '#' || line[0] == '/')
			continue
		parse(line, id, charsmax(id))
		// "kind:id" lines are kept for later (presets in the vote); not modes
		if (id[0] && contain(id, ":") == -1)
			ArrayPushString(ids, id)
	}
	fclose(f)
	return ArraySize(ids) > 0
}

find_mode_id(const id[])
{
	new mid[32], name[2]
	for (new m = 0, n = gm_modes_count(); m < n; m++)
		if (gm_modes_info(m, mid, charsmax(mid), name, charsmax(name)) && equali(mid, id))
			return m
	return -1
}

// The modes people can vote for now (mode indexes): the server's list, installed ones
// only, not the one playing, and only ones with a map that suits them
allowed_modes(Array:out)
{
	new Array:ids = ArrayCreate(32)
	if (!read_vote_list(fmt("%s/votemodes.ini", g_DataDir), ids) && !read_vote_list(fmt("%s/gamemodes/votemodes.ini", g_CfgDir), ids))
		for (new i = 0; i < sizeof DEFAULT_VOTE_MODES; i++)
			ArrayPushString(ids, DEFAULT_VOTE_MODES[i])
	new cur = gm_modes_current(), id[32], cid[32], name[2]
	new bool:zombie, bool:zp5
	gm_modes_info(cur, cid, charsmax(cid), name, charsmax(name))
	zombie = bool:equali(cid, "zombie")
	zp5 = bool:equali(cid, "zp5")
	new Array:maps = ArrayCreate(32)
	for (new i = 0; i < ArraySize(ids); i++)
	{
		ArrayGetString(ids, i, id, charsmax(id))
		new m = find_mode_id(id)
		if (m < 0 || m == cur || ArrayFindValue(out, m) != -1)
			continue
		ArrayClear(maps)
		if (!gm_modes_maps(m, maps))
			continue
		ArrayPushCell(out, m)
		if (equali(id, "zombie"))
			zombie = true
		else if (equali(id, "zp5"))
			zp5 = true
	}
	g_BothZombies = zombie && zp5
	ArrayDestroy(maps)
	ArrayDestroy(ids)
}

// The name in the vote: Zombie Plague Advanced is just "Zombie Plague" unless both are in it
vote_name(mode, out[], len)
{
	new id[32]
	out[0] = 0
	gm_modes_info(mode, id, charsmax(id), out, len)
	if (equali(id, "zp5") && !g_BothZombies)
		copy(out, len, "Zombie Plague")
}

// How much bots like a mode, roughly how busy those servers were
mode_pop(mode)
{
	new id[32], name[2]
	gm_modes_info(mode, id, charsmax(id), name, charsmax(name))
	new const POP[][] = { "classic", "csdm", "gungame", "zp5", "zombie", "war3", "deathrun", "hns", "csdm_ffa", "shero" }
	new const W[] = { 5, 5, 4, 4, 4, 3, 3, 3, 3, 2 }
	for (new i = 0; i < sizeof POP; i++)
		if (equali(id, POP[i]))
			return W[i]
	return 1
}

// ---- nominating a mode -------------------------------------------------------------

nominate_mode(id, const query[])
{
	if (!mode_voting())
		return client_print_color(id, print_team_default, "^4[Server]^1 Mode voting is turned off.")
	if (g_Voting || g_Changing || g_EndStarted)
		return client_print_color(id, print_team_default, "^4[Server]^1 Too late to nominate a mode now.")
	new Array:allowed = ArrayCreate(1)
	allowed_modes(allowed)
	new menu = menu_create(query[0] ? fmt("\yNominate a mode^n\dMatching ^"%s^"", query) : "\yNominate a mode", "nominate_mode_handler")
	new name[48], mid[32], skip[2], found, only = -1
	for (new i = 0; i < ArraySize(allowed); i++)
	{
		new m = ArrayGetCell(allowed, i)
		vote_name(m, name, charsmax(name))
		gm_modes_info(m, mid, charsmax(mid), skip, charsmax(skip))
		if (query[0] && containi(name, query) < 0 && containi(mid, query) < 0)
			continue
		menu_additem(menu, name, fmt("%d", m))
		only = m
		found++
	}
	ArrayDestroy(allowed)
	if (!found)
	{
		menu_destroy(menu)
		if (query[0])
			return client_print_color(id, print_team_default, "^4[Server]^1 No mode in the vote matches ^"%s^".", query)
		return client_print_color(id, print_team_default, "^4[Server]^1 There are no other modes to vote for here.")
	}
	if (found == 1)
	{
		menu_destroy(menu)
		return add_mode_nomination(id, only)
	}
	menu_display(id, menu)
	return PLUGIN_HANDLED
}

public nominate_mode_handler(id, menu, item)
{
	if (item >= 0)
	{
		new info[8], access, cb
		menu_item_getinfo(menu, item, access, info, charsmax(info), _, _, cb)
		add_mode_nomination(id, str_to_num(info))
	}
	menu_destroy(menu)
	return PLUGIN_HANDLED
}

add_mode_nomination(id, mode)
{
	new name[48]
	vote_name(mode, name, charsmax(name))
	for (new i = 0; i < g_MNomCount; i++)
		if (g_MNoms[i] == mode)
			return client_print_color(id, print_team_default, "^4[Server]^1 %s is already nominated.", name)
	new slot = -1
	for (new i = 0; i < g_MNomCount; i++)
		if (g_MNomBy[i] == id)
			slot = i                       // replace your earlier nomination
	if (slot < 0)
	{
		if (g_MNomCount >= MAX_NOMS)
			return client_print_color(id, print_team_default, "^4[Server]^1 The mode nomination list is full.")
		slot = g_MNomCount++
	}
	g_MNomBy[slot] = id
	g_MNoms[slot] = mode
	new who[32]
	get_user_name(id, who, charsmax(who))
	client_print_color(0, print_team_default, "^4[Server]^3 %s^1 nominated ^4%s^1.", who, name)
	return PLUGIN_HANDLED
}

// ---- stage 1: the mode ---------------------------------------------------------------

public start_mode_vote()
{
	begin_mode_vote(false)
}

begin_mode_vote(bool:end)
{
	g_EndVote = end
	g_ModeFlow = !end
	g_Voting = true
	new cur = gm_modes_current()
	if (cur < 0)
	{
		g_Voting = false
		g_ModeFlow = false
		return
	}
	new Array:allowed = ArrayCreate(1)
	allowed_modes(allowed)
	g_MVOptions = 0
	g_MVMode[g_MVOptions++] = cur
	// nominated modes first
	for (new i = 0; i < g_MNomCount && g_MVOptions <= VOTE_MODES; i++)
		if (ArrayFindValue(allowed, g_MNoms[i]) != -1)
			add_mode_option(g_MNoms[i])
	// then a random few, the ones not played lately first
	new n = ArraySize(allowed), id[32], name[2]
	for (new i = n - 1; i > 0; i--)
		ArraySwap(allowed, i, random(i + 1))
	for (new pass = 0; pass < 2; pass++)
		for (new i = 0; i < n && g_MVOptions <= VOTE_MODES; i++)
		{
			new m = ArrayGetCell(allowed, i)
			gm_modes_info(m, id, charsmax(id), name, charsmax(name))
			if (recent_mode(id) == (pass == 1))
				add_mode_option(m)
		}
	ArrayDestroy(allowed)

	if (g_MVOptions == 1)
	{
		client_print_color(0, print_team_default, "^4[Server]^1 There are no other modes to vote for, so it's a map vote.")
		set_task(1.0, "task_keep_stage", TASK_VOTE)
		return
	}
	arrayset(g_MVCount, 0, sizeof g_MVCount)
	arrayset(g_MVBot, 0, sizeof g_MVBot)
	g_Phase = 1
	new menu = menu_create(fmt("\yVote for the next mode \d(%d seconds)", VOTE_TIME), "mode_vote_handler")
	new label[64]
	for (new i = 0; i < g_MVOptions; i++)
	{
		vote_name(g_MVMode[i], label, charsmax(label))
		menu_additem(menu, i ? label : fmt("Keep %s", label))
	}
	menu_setprop(menu, MPROP_EXIT, MEXIT_NEVER)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			menu_display(i, menu, 0, VOTE_TIME)
	if (get_pcvar_num(g_pBotVotes))
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 60)
				set_task(random_float(2.0, float(VOTE_TIME) - 2.0), "task_bot_mode_vote", get_user_userid(b))
	new data[1]
	data[0] = menu
	set_task(float(VOTE_TIME) + 0.5, "end_mode_vote", TASK_VOTE, data, sizeof data)
	if (end)
		client_print_color(0, print_team_default, "^4[Server]^1 The map ends soon. Vote for the next mode, then a map. Results in %d seconds.", VOTE_TIME)
	else
		client_print_color(0, print_team_default, "^4[Server]^1 Vote for the next mode, then a map. Results in %d seconds.", VOTE_TIME)
}

add_mode_option(mode)
{
	for (new j = 0; j < g_MVOptions; j++)
		if (g_MVMode[j] == mode)
			return
	g_MVMode[g_MVOptions++] = mode
}

bool:mode_nominated(mode)
{
	for (new i = 0; i < g_MNomCount; i++)
		if (g_MNoms[i] == mode)
			return true
	return false
}

say_mode_vote(id, item)
{
	new who[32], name[48]
	get_user_name(id, who, charsmax(who))
	vote_name(g_MVMode[item], name, charsmax(name))
	if (item == 0)
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 wants to keep ^4%s^1.", who, name)
	else
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 voted for ^4%s^1.", who, name)
}

// A bot leans toward the modes people are voting for and the busy ones; its vote only
// breaks a tie between people's favourites
public task_bot_mode_vote(userid)
{
	new id = find_player("k", userid)
	if (g_Phase != 1 || !id || !is_user_bot(id))
		return
	new w[VOTE_MODES + 1], total
	for (new i = 0; i < g_MVOptions; i++)
	{
		w[i] = (i == 0 ? 3 : mode_pop(g_MVMode[i])) + g_MVCount[i] * 3 + (mode_nominated(g_MVMode[i]) ? 2 : 0)
		total += w[i]
	}
	new r = random(max(total, 1)), item
	for (item = 0; item < g_MVOptions - 1 && r >= w[item]; item++)
		r -= w[item]
	g_MVBot[item]++
	say_mode_vote(id, item)
}

public mode_vote_handler(id, menu, item)
{
	if (g_Phase != 1 || item < 0 || item >= g_MVOptions)
		return PLUGIN_HANDLED
	g_MVCount[item]++
	say_mode_vote(id, item)
	return PLUGIN_HANDLED
}

// People's votes decide; bots' votes only break a tie between people's favourites (or
// decide when no person voted). -1: nobody voted.
tally(const people[], const bots[], n)
{
	new best = -1, most = 0, ties
	new bool:any
	for (new i = 0; i < n; i++)
		if (people[i])
			any = true
	for (new i = 0; i < n; i++)
	{
		if (any && !people[i])
			continue
		new v = any ? people[i] * 100 + bots[i] : bots[i]
		if (v > most)
		{
			best = i
			most = v
			ties = 1
		}
		else if (v == most && most > 0 && random(++ties) == 0)
			best = i
	}
	return best
}

public end_mode_vote(const data[])
{
	menu_destroy(data[0])    // also closes it for anyone who didn't vote
	g_Phase = 0
	new best = tally(g_MVCount, g_MVBot, g_MVOptions)
	if (best >= 0)
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 12)
				say_line(b, "mode_vote", 0.6)
	new name[48]
	if (best <= 0)
	{
		vote_name(g_MVMode[0], name, charsmax(name))
		client_print_color(0, print_team_default, "^4[Server]^1 Keeping ^4%s^1. Now vote for the map.", name)
		set_task(2.0, "task_keep_stage", TASK_VOTE)
		return
	}
	new votes = g_MVCount[best] + g_MVBot[best]
	vote_name(g_MVMode[best], name, charsmax(name))
	client_print_color(0, print_team_default, "^4[Server]^1 Next mode: ^4%s^1 (%d vote%s). Now pick a map for it.", name, votes, votes == 1 ? "" : "s")
	g_StageMode = g_MVMode[best]
	set_task(2.0, "task_mode_stage", TASK_VOTE)
}

// ---- stage 2: the map ----------------------------------------------------------------

// The mode stays: the usual map vote (at the end of a map: without "keep playing")
public task_keep_stage()
{
	if (g_EndVote)
		start_map_stage(gm_modes_current(), true)
	else
		start_vote()
}

public task_mode_stage()
{
	start_map_stage(g_StageMode, false)
}

bool:option_listed(const map[])
{
	for (new j = 0; j < g_VoteOptions; j++)
		if (equali(g_VoteMaps[j], map))
			return true
	return false
}

// `usual`: rock the vote's own map list; else the maps that suit the mode
start_map_stage(mode, bool:usual)
{
	g_StageMode = mode
	new Array:pool = ArrayCreate(32), map[32], cur[32]
	if (usual)
	{
		build_pool()
		for (new i = 0; i < ArraySize(g_MapPool); i++)
		{
			ArrayGetString(g_MapPool, i, map, charsmax(map))
			ArrayPushString(pool, map)
		}
	}
	else
		gm_modes_maps(mode, pool)
	// the map that's ending isn't one to pick again; for a switch now it counts if it suits
	get_mapname(cur, charsmax(cur))
	new bool:curFits
	for (new i = ArraySize(pool) - 1; i >= 0; i--)
	{
		ArrayGetString(pool, i, map, charsmax(map))
		if (!equali(map, cur))
			continue
		ArrayDeleteItem(pool, i)
		curFits = !g_EndVote
	}
	g_VoteOptions = 0
	if (curFits)
		copy(g_VoteMaps[g_VoteOptions++], charsmax(g_VoteMaps[]), cur)
	// nominated maps that suit come next
	for (new i = 0; i < g_NomCount && g_VoteOptions < VOTE_MAPS; i++)
		for (new k = 0; k < ArraySize(pool); k++)
		{
			ArrayGetString(pool, k, map, charsmax(map))
			if (equali(map, g_Noms[i]) && !option_listed(map))
				copy(g_VoteMaps[g_VoteOptions++], charsmax(g_VoteMaps[]), map)
		}
	new n = ArraySize(pool)
	for (new tries = 0; g_VoteOptions < VOTE_MAPS && tries < 200 && n; tries++)
	{
		ArrayGetString(pool, random(n), map, charsmax(map))
		if (!option_listed(map))
			copy(g_VoteMaps[g_VoteOptions++], charsmax(g_VoteMaps[]), map)
	}
	ArrayDestroy(pool)
	if (g_VoteOptions <= 1)
	{
		finish_stage(g_VoteOptions ? 0 : -1)
		return
	}
	arrayset(g_VoteCount, 0, sizeof g_VoteCount)
	arrayset(g_BotVote, 0, sizeof g_BotVote)
	g_Phase = 2
	new name[48]
	vote_name(mode, name, charsmax(name))
	new menu = menu_create(usual ? fmt("\yVote for the next map \d(%d seconds)", VOTE_TIME) : fmt("\yPick a map for %s \d(%d seconds)", name, VOTE_TIME), "stage_handler")
	for (new i = 0; i < g_VoteOptions; i++)
		menu_additem(menu, (curFits && i == 0) ? fmt("%s \d(this map)", cur) : g_VoteMaps[i])
	menu_setprop(menu, MPROP_EXIT, MEXIT_NEVER)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i) && !is_user_hltv(i))
			menu_display(i, menu, 0, VOTE_TIME)
	if (get_pcvar_num(g_pBotVotes))
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 60)
				set_task(random_float(2.0, float(VOTE_TIME) - 2.0), "task_bot_stage", get_user_userid(b))
	new data[1]
	data[0] = menu
	set_task(float(VOTE_TIME) + 0.5, "end_stage", TASK_VOTE, data, sizeof data)
	client_print_color(0, print_team_default, "^4[Server]^1 Vote for the map. Results in %d seconds.", VOTE_TIME)
}

bool:map_nominated(const map[])
{
	for (new i = 0; i < g_NomCount; i++)
		if (equali(g_Noms[i], map))
			return true
	return false
}

say_map_vote(id, item)
{
	new who[32]
	get_user_name(id, who, charsmax(who))
	client_print_color(0, print_team_default, "^4[Server]^3 %s^1 voted for ^4%s^1.", who, g_VoteMaps[item])
}

public task_bot_stage(userid)
{
	new id = find_player("k", userid)
	if (g_Phase != 2 || !id || !is_user_bot(id))
		return
	new w[VOTE_MAPS + 1], total
	for (new i = 0; i < g_VoteOptions; i++)
	{
		w[i] = 2 + g_VoteCount[i] * 3 + (map_nominated(g_VoteMaps[i]) ? 2 : 0)
		total += w[i]
	}
	new r = random(max(total, 1)), item
	for (item = 0; item < g_VoteOptions - 1 && r >= w[item]; item++)
		r -= w[item]
	g_BotVote[item]++
	say_map_vote(id, item)
}

public stage_handler(id, menu, item)
{
	if (g_Phase != 2 || item < 0 || item >= g_VoteOptions)
		return PLUGIN_HANDLED
	g_VoteCount[item]++
	say_map_vote(id, item)
	return PLUGIN_HANDLED
}

public end_stage(const data[])
{
	menu_destroy(data[0])
	g_Phase = 0
	new best = tally(g_VoteCount, g_BotVote, g_VoteOptions)
	if (best >= 0)
		for (new b = 1; b <= MaxClients; b++)
			if (is_user_connected(b) && is_user_bot(b) && random(100) < 10)
				say_line(b, "map_vote", 0.6)
	finish_stage(best)
}

// The map is picked (-1: nobody voted, or there was nothing to pick)
finish_stage(best)
{
	new mode = g_StageMode, cur = gm_modes_current(), name[48]
	vote_name(mode, name, charsmax(name))
	g_Voting = false
	if (best < 0 && g_VoteOptions && (!g_EndVote || mode != cur))
		best = random(g_VoteOptions)     // the mode won: it needs a map
	if (g_EndVote)
	{
		g_EndVote = false
		g_VoteDone = true
		if (best < 0)
			return client_print_color(0, print_team_default, "^4[Server]^1 Nobody voted for a map. The next one comes from the rotation.")
		gm_modes_next(mode, g_VoteMaps[best])
		if (mode == cur)
			return client_print_color(0, print_team_default, "^4[Server]^1 Next map: ^4%s^1.", g_VoteMaps[best])
		return client_print_color(0, print_team_default, "^4[Server]^1 Next: ^4%s^1 on ^4%s^1, when this map ends.", name, g_VoteMaps[best])
	}
	if (best < 0)
	{
		vote_settled(false)
		return client_print_color(0, print_team_default, "^4[Server]^1 There's no map to play %s on, so nothing changes.", name)
	}
	vote_settled(true)
	g_Changing = true
	client_print_color(0, print_team_default, "^4[Server]^1 Switching to ^4%s^1 on ^4%s^1 in 5 seconds.", name, g_VoteMaps[best])
	new data[33]
	data[0] = mode
	copy(data[1], charsmax(data) - 1, g_VoteMaps[best])
	set_task(5.0, "task_mode_go", TASK_CHANGE, data, sizeof data)
	return PLUGIN_HANDLED
}

public task_mode_go(const data[])
{
	// through the Game Mode Menu: the mode's saved settings, plugins and bot personalities
	if (!gm_modes_go(data[0], data[1]))
		g_Changing = false
}

// ---- the end of the map ----------------------------------------------------------------

public ev_teamscore()
{
	new team[2]
	read_data(1, team, charsmax(team))
	g_Score[team[0] == 'C' ? 0 : 1] = read_data(2)
}

// Close to the end, the way mapchooser judges it (rounds or wins to go, or time left)
bool:near_end()
{
	new winlimit = get_cvar_num("mp_winlimit"), maxrounds = get_cvar_num("mp_maxrounds")
	if (winlimit)
		return max(g_Score[0], g_Score[1]) >= winlimit - 2
	if (maxrounds)
		return g_Score[0] + g_Score[1] >= maxrounds - 2
	new left = get_timeleft()
	return get_cvar_float("mp_timelimit") > 0.0 && left > 0 && left <= END_LEAD
}

// One vote at the end, not two: mapchooser sits this map out
set_mapchooser(bool:paused)
{
	g_McPaused = paused
	if (is_plugin_loaded("mapchooser.amxx", true) == -1)
		return
	if (paused)
		pause("ac", "mapchooser.amxx")
	else
		unpause("ac", "mapchooser.amxx")
}

public task_end_check()
{
	new rotate = get_pcvar_num(g_pModeRotate)
	new bool:due = g_EndDue && get_pcvar_num(g_pModeEvery) > 0 && (rotate ? mode_rotating() : mode_voting())
	if (!g_EndStarted && due != g_McPaused)
		set_mapchooser(due)
	// (a rotation needs nobody there; a vote needs people to vote)
	if (!due || g_EndStarted || g_Voting || g_Changing || (!rotate && !humans()) || !near_end())
		return
	g_EndStarted = true
	if (rotate)
		rotate_mode(rotate == 2)
	else
		begin_mode_vote(true)
}

// A rotation can run: the Game Mode Menu is there, not a Tour of Duty match or a scrim
bool:mode_rotating()
{
	return !g_Career && !get_cvar_num("gm_scrim_phase") && LibraryExists("gm_modes", LibType_Library) && gm_modes_current() >= 0
}

// Every Nth map with the rotation on: the next mode on the server's list (the modes it lets
// people vote for, in that order), or a random one of them, with a map that suits it (the
// mode's own rotation first), from the next map on. Modes with no map that suits them are
// skipped; with none left, the mode stays.
rotate_mode(bool:random_pick)
{
	new Array:ok = ArrayCreate(1)
	allowed_modes(ok)
	new n = ArraySize(ok)
	if (!n)
	{
		ArrayDestroy(ok)
		return
	}
	new pick = -1
	if (random_pick)
		pick = ArrayGetCell(ok, random(n))
	else
	{
		// the list's order, starting after the mode playing now (from the top if it isn't listed)
		new Array:ids = ArrayCreate(32)
		if (!read_vote_list(fmt("%s/votemodes.ini", g_DataDir), ids) && !read_vote_list(fmt("%s/gamemodes/votemodes.ini", g_CfgDir), ids))
			for (new i = 0; i < sizeof DEFAULT_VOTE_MODES; i++)
				ArrayPushString(ids, DEFAULT_VOTE_MODES[i])
		new cur = gm_modes_current(), id[32], at = -1, total = ArraySize(ids)
		for (new i = 0; i < total && at < 0; i++)
		{
			ArrayGetString(ids, i, id, charsmax(id))
			if (find_mode_id(id) == cur)
				at = i
		}
		for (new k = 1; k <= total && pick < 0; k++)
		{
			ArrayGetString(ids, (at + k + total) % total, id, charsmax(id))
			new m = find_mode_id(id)
			if (m >= 0 && ArrayFindValue(ok, m) != -1)
				pick = m
		}
		ArrayDestroy(ids)
	}
	ArrayDestroy(ok)
	if (pick < 0)
		return
	new Array:maps = ArrayCreate(32), map[32], cur[32], chosen[32]
	gm_modes_maps(pick, maps)
	get_mapname(cur, charsmax(cur))
	new count = ArraySize(maps)
	for (new tries = 0; tries < 20 && count && !chosen[0]; tries++)
	{
		ArrayGetString(maps, random(count), map, charsmax(map))
		if (!equali(map, cur) || count == 1)
			copy(chosen, charsmax(chosen), map)
	}
	ArrayDestroy(maps)
	if (!chosen[0] || !gm_modes_next(pick, chosen))
		return
	new name[48]
	vote_name(pick, name, charsmax(name))
	client_print_color(0, print_team_default, "^4[Server]^1 Next: ^4%s^1 on ^4%s^1, when this map ends.", name, chosen)
}

// ---- an admin (or the app) starts one ------------------------------------------------------

vote_reply(id, const msg[])
{
	if (id)
	{
		console_print(id, "%s", msg)
		client_print_color(id, print_team_default, "^4[Server]^1 %s", msg)
	}
	else
		server_print("[Game Mode] %s", msg)
}

public cmd_votemode(id, level, cid)
{
	if (!cmd_access(id, level, cid, 1))
		return PLUGIN_HANDLED
	if (!LibraryExists("gm_modes", LibType_Library) || g_Career || gm_modes_current() < 0)
		vote_reply(id, "Mode votes need the Game Mode Menu, and don't run in a Tour of Duty match.")
	else if (get_cvar_num("gm_scrim_phase"))
		vote_reply(id, "Not during a match.")
	else if (g_Voting)
		vote_reply(id, "A vote is already running.")
	else if (g_Changing)
		vote_reply(id, "The map is about to change.")
	else
	{
		g_Voting = true
		set_task(1.0, "start_mode_vote", TASK_VOTE)
		client_print_color(0, print_team_default, "^4[Server]^1 An admin started a mode vote.")
		if (!id)
			server_print("[Game Mode] Mode vote started.")
		else
			console_print(id, "Mode vote started.")
	}
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Making room: which bot leaves when a person takes a slot (bot_quota_mode fill)
// ---------------------------------------------------------------------------

// A bot will have to go when this person joins a team: the bot count fills the server up
// to bot_quota with people on teams counted, and one more is about to be
bool:room_needed(id)
{
	if (g_Career || get_cvar_num("gm_scrim_phase"))
		return false
	new mode[8]
	get_cvar_string("bot_quota_mode", mode, charsmax(mode))
	if (!equali(mode, "fill"))
		return false
	new bots, people
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || is_user_hltv(i))
			continue
		if (is_user_bot(i))
			bots++
		else if (i != id && (get_user_team(i) == 1 || get_user_team(i) == 2))
			people++
	}
	return bots > 0 && bots > get_cvar_num("bot_quota") - (people + 1)
}

// The bot to go: one on the team the person is joining (so the teams stay even and the game
// takes the join), a dead one before a living one (a kick mid-round wrecks it less), a
// newcomer before a regular, and the newest on the server first; never one already going
// One the round hangs on right now: the bomb carrier, a team's last one alive, or one still
// doing what a person asked (pick_leaver's +2000, on its own)
bool:key_bot(i)
{
	if (!is_user_alive(i))
		return false
	if (user_has_weapon(i, CSW_C4))
		return true
	new t = get_user_team(i), n
	for (new j = 1; j <= MaxClients; j++)
		if (j != i && is_user_alive(j) && get_user_team(j) == t)
			n++
	if ((t == 1 || t == 2) && n == 0)
		return true
	new Float:now = get_gametime(), Float:asked, Float:order[3]
	pev(i, pev_fuser4, asked)
	pev(i, pev_vuser1, order)
	return (asked > 0.0 && asked <= now && now - asked < 30.0) || order[0] > now
}

pick_leaver(team = 0)
{
	new best, best_score = cellmax, Float:now = get_gametime()
	new alive[3]
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && (get_user_team(i) == 1 || get_user_team(i) == 2))
			alive[get_user_team(i)]++
	new bool:planted = engfunc(EngFunc_FindEntityByString, -1, "model", "models/w_c4.mdl") > 0
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i) || is_user_hltv(i) || g_Leaving[i])
			continue
		new t = get_user_team(i)
		// (never, if it can be helped, one the round hangs on: the bomb carrier, a planter or
		// defuser, a team's last one alive, or one doing what a person just asked of it; the
		// brain notes an order it took in fuser4)
		// (and while that order runs: its end time is in vuser1's x)
		new Float:asked, Float:order[3]
		pev(i, pev_fuser4, asked)
		pev(i, pev_vuser1, order)
		new bool:key = is_user_alive(i) && (user_has_weapon(i, CSW_C4) || ((t == 1 || t == 2) && alive[t] == 1)
			|| (t == 2 && planted && (pev(i, pev_button) & IN_USE)) || (asked > 0.0 && asked <= now && now - asked < 30.0)
			|| order[0] > now)
		new score = ((team == 1 || team == 2) && t != team ? 5000 : 0) + (key ? 2000 : 0)
			+ (is_user_alive(i) ? 1000 : 0) + (g_IsRegular[i] ? 300 : 0) + min(floatround((now - g_BotSince[i]) / 60.0), 200)
		if (score < best_score)
		{
			best_score = score
			best = i
		}
	}
	return best
}

// A person connected and a bot will have to make room: one is set aside for them now (so
// several people coming in at once each get a different one); it says bye and goes only
// once the person picks a team. Nothing is said before then: they may never pick one
make_room_early(id)
{
	g_LeaverFor[id] = 0
	if (!room_needed(id))
		return
	new bot = pick_leaver()
	if (!bot)
		return
	g_Leaving[bot] = true
	g_LeaverFor[id] = get_user_userid(bot)
}

// The bot set aside for a person who went to spectate (or left): it stays after all
release_leaver(id)
{
	if (!g_LeaverFor[id])
		return
	new bot = find_player("k", g_LeaverFor[id])
	if (bot)
		g_Leaving[bot] = false
	g_LeaverFor[id] = 0
}

// Just before the person's team choice is handled: the bot picked for them says bye and goes
// now, so the bot count is right already and the game doesn't kick one of its own choosing.
// One from the team they're joining, so the teams stay even and the game takes the join
make_room(id, team)
{
	if (is_user_bot(id) || (get_user_team(id) == 1 || get_user_team(id) == 2) || !room_needed(id))
		return
	new bot = g_LeaverFor[id] ? find_player("k", g_LeaverFor[id]) : 0
	if (bot && (team == 1 || team == 2) && get_user_team(bot) != team)
	{
		// (one on their team instead, if there is one)
		g_Leaving[bot] = false
		new other = pick_leaver(team)
		if (other && get_user_team(other) == team)
			bot = other
		else
			g_Leaving[bot] = true
	}
	if (!bot || !is_user_bot(bot))
		bot = pick_leaver(team)
	// (the one set aside when they connected may since have become one the round hangs on:
	// handed the bomb at round start, Game Commencing, the last of its team; another, if any)
	else if (key_bot(bot))
	{
		g_Leaving[bot] = false
		new other = pick_leaver(team)
		if (other && !key_bot(other) && ((team != 1 && team != 2) || get_user_team(other) == team))
			bot = other
		else
			g_Leaving[bot] = true
	}
	g_LeaverFor[id] = 0
	if (!bot)
		return
	// (said now, straight to the game: the kick comes right after it; not by one that never
	// talks, nor with personality off; gm_botchat notes it left, for a "wb" if it comes back)
	new bool:quiet = get_cvar_num("gm_personality") == 0
		|| (LibraryExists("gm_people", LibType_Library) && people_get(bot, "talk") == 0)
	if (g_Chat && !g_NoLeft)
		botchat_left(bot)
	if (g_Chat && !quiet)
	{
		static const BYE[][] = { "gtg", "gtg cya", "ok im off", "cya all", "gn all", "bbl", "brb food", "gotta go", "cya", "im out gg" }
		new bn[32]
		get_user_name(bot, bn, charsmax(bn))
		new h
		for (new c = 0; bn[c]; c++)
			h = h * 31 + bn[c]
		engclient_cmd(bot, "say", BYE[(h & 0x7FFFFFFF) % sizeof BYE])
	}
	new name[32]
	get_user_name(bot, name, charsmax(name))
	// (by its own name: a "Dave_" who comes back is Dave, untweaked)
	if (g_RealName[bot][0])
		copy(name, charsmax(name), g_RealName[bot])
	ArrayPushString(g_LeftNames, name)
	ArrayPushCell(g_LeftAt, floatround(get_gametime()))
	server_cmd("kick #%d", get_user_userid(bot))
	server_exec()
}

// jointeam <1 T, 2 CT, 5 auto, 6 spectator>
public cmd_jointeam(id)
{
	new arg[4]
	read_argv(1, arg, charsmax(arg))
	new t = str_to_num(arg)
	if (t == 1 || t == 2 || t == 5)
		make_room(id, t)
	else if (t == 6)
		release_leaver(id)
	return PLUGIN_CONTINUE
}

// the old team menu: 1 T, 2 CT, 5 auto, 6 spectator (keys from 0)
public menu_team(id, key)
{
	if (key == 0 || key == 1 || key == 4)
		make_room(id, key == 4 ? 5 : key + 1)
	else if (key == 5)
		release_leaver(id)
	return PLUGIN_CONTINUE
}

// ---------------------------------------------------------------------------
// Welcome screen
// ---------------------------------------------------------------------------
public client_putinserver(id)
{
	g_Leaving[id] = false
	g_LeaverFor[id] = 0
	if (is_user_bot(id))
		g_BotSince[id] = get_gametime()
	if (!is_user_bot(id) && !is_user_hltv(id))
		make_room_early(id)
	if (is_user_bot(id) || is_user_hltv(id) || g_Career)
		return
	new mode = get_pcvar_num(g_pWelcome)
	if (mode <= 0)
		return
	if (mode == 1)
	{
		// once per game session per player; localinfo survives map changes
		new auth[40], key[40], seen[4], k
		get_user_authid(id, auth, charsmax(auth))
		if (!auth[0] || equal(auth, "BOT"))
			get_user_name(id, auth, charsmax(auth))
		k = copy(key, charsmax(key), "gmw_")
		for (new i = 0; auth[i] && k < 28; i++)       // infobuffer keys: plain characters only
			if (isalnum(auth[i]))
				key[k++] = auth[i]
		key[k] = 0
		get_localinfo(key, seen, charsmax(seen))
		if (seen[0])
			return
		set_localinfo(key, "1")
	}
	set_task(2.0, "task_welcome", TASK_WELCOME + id)
}

public task_welcome(taskid)
{
	new id = taskid - TASK_WELCOME
	if (is_user_connected(id))
		show_welcome(id)
}

public cmd_welcome(id)
{
	show_welcome(id)
	return PLUGIN_HANDLED
}

html_escape(const src[], dst[], len)
{
	new j
	for (new i = 0; src[i] && j < len - 6; i++)
	{
		switch (src[i])
		{
			case '<': j += copy(dst[j], len - j, "&lt;")
			case '>': j += copy(dst[j], len - j, "&gt;")
			case '&': j += copy(dst[j], len - j, "&amp;")
			case '"': j += copy(dst[j], len - j, "&quot;")
			default: dst[j++] = src[i]
		}
	}
	dst[j] = 0
}

// The mode's name and description from modes.ini, for the current mode in current.txt
current_mode(name[], nlen, desc[], dlen, opts[] = "", olen = 0)
{
	new id[32], line[256], f = fopen(fmt("%s/current.txt", g_DataDir), "rt")
	if (f)
	{
		fgets(f, id, charsmax(id))
		trim(id)
		fclose(f)
	}
	copy(name, nlen, "Classic")
	desc[0] = 0
	f = fopen(fmt("%s/gamemodes/modes.ini", g_CfgDir), "rt")
	if (!f)
		return
	new key[32], q1[48], q2[96], q3[48]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (!line[0] || line[0] == ';')
			continue
		if (parse(line, key, charsmax(key), q1, charsmax(q1), q2, charsmax(q2), q3, charsmax(q3)) >= 2 && equali(key, id))
		{
			copy(name, nlen, q1)
			copy(desc, dlen, q2)
			if (olen)
				copy(opts, olen, q3)
			break
		}
	}
	fclose(f)
}

// The host's own welcome page from the app (data/gamemodes): welcome_url.txt (a web
// page the game's browser opens) or welcome.html with {server} {player} {map}
// {mode} {players} filled in. Returns 0 when there's neither.
custom_welcome(const hostesc[], const pesc[], const map[], const mode[], players, out[], len)
{
	new f = fopen(fmt("%s/welcome_url.txt", g_DataDir), "rt")
	if (f)
	{
		fgets(f, out, len)
		fclose(f)
		trim(out)
		if (equali(out, "http://", 7) || equali(out, "https://", 8))
		{
			welcome_query(out, len, players)
			return 1
		}
	}
	f = fopen(fmt("%s/welcome.html", g_DataDir), "rt")
	if (!f)
		return 0
	new n
	out[0] = 0
	while (n < len - 1 && fgets(f, out[n], len - n))
		n = strlen(out)
	fclose(f)
	if (!out[0])
		return 0
	new num[8], modesc[96]
	num_to_str(players, num, charsmax(num))
	html_escape(mode, modesc, charsmax(modesc))
	replace_string(out, len, "{server}", hostesc)
	replace_string(out, len, "{player}", pesc)
	replace_string(out, len, "{map}", map)
	replace_string(out, len, "{mode}", modesc)
	replace_string(out, len, "{players}", num)
	return 1
}

// The welcome page at a web address gets ?server=&player=&mode=&map=&players= added, so
// it can greet the player and show what's on (the raw values, url-encoded; the player is
// whoever the page is being shown to, read from g_WelcomeFor)
welcome_query(url[], len, players)
{
	new host[64], pname[32], mode[32], map[32], v[160]
	get_cvar_string("hostname", host, charsmax(host))
	get_user_name(g_WelcomeFor, pname, charsmax(pname))
	current_mode_id(mode, charsmax(mode))
	get_mapname(map, charsmax(map))
	add(url, len, contain(url, "?") == -1 ? "?" : "&")
	url_encode(host, v, charsmax(v)); add(url, len, fmt("server=%s", v))
	url_encode(pname, v, charsmax(v)); add(url, len, fmt("&player=%s", v))
	url_encode(mode, v, charsmax(v)); add(url, len, fmt("&mode=%s", v))
	url_encode(map, v, charsmax(v)); add(url, len, fmt("&map=%s", v))
	add(url, len, fmt("&players=%d%%2F%d", players, MaxClients))
}

url_encode(const text[], out[], len)
{
	new n, c
	static const HEX[] = "0123456789ABCDEF"
	for (new i = 0; text[i] && n < len - 3; i++)
	{
		c = text[i] & 0xFF
		if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.' || c == '~')
			out[n++] = c
		else
		{
			out[n++] = '%'
			out[n++] = HEX[c >> 4]
			out[n++] = HEX[c & 15]
		}
	}
	out[n] = 0
}

// welcome_note.txt: the host's own line(s) under the built-in page
welcome_note(out[], len)
{
	new raw[320], line[160], f = fopen(fmt("%s/welcome_note.txt", g_DataDir), "rt")
	if (!f)
		return 0
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (raw[0])
			add(raw, charsmax(raw), "^n")
		add(raw, charsmax(raw), line)
	}
	fclose(f)
	trim(raw)
	if (!raw[0])
		return 0
	html_escape(raw, out, len)
	replace_string(out, len, "^n", "<br>")
	return 1
}

show_welcome(id)
{
	static html[1536], host[64], hostesc[96], pname[32], pesc[64], mode[48], desc[96], map[32], opts[48], note[400]
	get_cvar_string("hostname", host, charsmax(host))
	html_escape(host, hostesc, charsmax(hostesc))
	get_user_name(id, pname, charsmax(pname))
	html_escape(pname, pesc, charsmax(pesc))
	current_mode(mode, charsmax(mode), desc, charsmax(desc), opts, charsmax(opts))
	get_mapname(map, charsmax(map))
	new players
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_hltv(i))
			players++
	new bool:admin = bool:(get_user_flags(id) & ADMIN_CFG)
	g_WelcomeFor = id
	if (custom_welcome(hostesc, pesc, map, mode, players, html, charsmax(html)))
	{
		show_motd(id, html, hostesc[0] ? host : "Welcome")
		return
	}

	new len = formatex(html, charsmax(html),
		"<html><head><meta charset=^"utf-8^"><style>\
		body{background:#2C3124;color:#E9E3CF;font:16px/1.45 Verdana,Tahoma,sans-serif;margin:0;padding:22px 26px}\
		h1{font:bold 34px Impact,'Arial Narrow',sans-serif;letter-spacing:1px;margin:0 0 4px}\
		.o{color:#E8792B}.d{color:#B5B09C}\
		.now{background:#3A402F;border-left:4px solid #E8792B;padding:10px 14px;margin:16px 0}\
		td{padding:3px 14px 3px 0;vertical-align:top}b{color:#E8792B}\
		</style></head><body><h1>%s</h1><div class=d>Welcome, %s</div>\
		<div class=now><b>%s</b> on %s &middot; %d players<br><span class=d>%s</span></div><table>",
		hostesc[0] ? hostesc : "Condition Zero", pesc, mode, map, players, desc)
	// the mode's own menu, e.g. "say /war3menu" in modes.ini
	if (equali(opts, "say ", 4))
		len += formatex(html[len], charsmax(html) - len, "<tr><td><b>%s</b></td><td>%s menu</td></tr>", opts[4], mode)
	if (get_pcvar_num(g_pRtv))
		len += formatex(html[len], charsmax(html) - len,
			"<tr><td><b>rtv</b></td><td>vote for a new map</td></tr><tr><td><b>nominate</b> map</td><td>suggest a map for the vote</td></tr>")
	if (get_pcvar_num(g_pModeVote))
		len += formatex(html[len], charsmax(html) - len, "<tr><td><b>rtm</b></td><td>vote for a different mode</td></tr>")
	len += formatex(html[len], charsmax(html) - len, admin ?
		"<tr><td><b>/gm</b></td><td>game modes, maps, bots and settings</td></tr><tr><td><b>/q</b></td><td>quick menu</td></tr>" :
		"<tr><td><b>/guns</b></td><td>pick your guns (deathmatch modes)</td></tr>")
	len += formatex(html[len], charsmax(html) - len,
		"<tr><td><b>/welcome</b></td><td>show this again</td></tr></table><p class=d>%s</p></body></html>",
		welcome_note(note, charsmax(note)) ? note : "Play fair, have fun. gl hf!")
	show_motd(id, html, hostesc[0] ? host : "Welcome")
}
