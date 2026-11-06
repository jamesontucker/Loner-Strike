/*
	gm_pubserver - makes a bot game feel like a public server.

	Regulars:   bots get names (with clan tags) from configs/gamemodes/regular_names.ini.
	            Each CZ bot profile keeps its own name for good (data/gamemodes/regulars.ini),
	            so the same regulars keep turning up; some bots join as one-off newcomers.
	Come & go:  now and then a bot leaves and someone new takes the slot (gm_churn).
	RTV:        "rtv" starts a map vote once enough players want one; "nominate <map>".
	Welcome:    a welcome screen when you join (gm_welcome), replacing the CZ advert.

	Cvars: gm_regulars 0/1, gm_churn 0/1/2, gm_rtv 0/1, gm_welcome 0/1/2
*/

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>

#define PLUGIN  "Pub server"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_CHURN   5100
#define TASK_VOTE    5200
#define TASK_CHANGE  5300
#define TASK_WELCOME 5400

#define VOTE_TIME    15
#define VOTE_MAPS    5
#define MAX_NOMS     4
#define NEWCOMER_PCT 35

new g_pRegulars, g_pChurn, g_pRtv, g_pWelcome
new bool:g_Career
new bool:g_Creating

new Array:g_Pool                 // every name in regular_names.ini
new Trie:g_Regular               // profile name -> regular name
new Trie:g_Taken                 // regular names already given to a profile
new g_DataDir[128], g_CfgDir[128]

new bool:g_Rocked[33]
new g_NomBy[MAX_NOMS], g_Noms[MAX_NOMS][32], g_NomCount
new bool:g_Voting, bool:g_VoteDone
new g_VoteMaps[VOTE_MAPS + 1][32], g_VoteCount[VOTE_MAPS + 1], g_VoteOptions
new Float:g_MapStart
new Array:g_MapPool

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pRegulars = register_cvar("gm_regulars", "1")
	g_pChurn    = register_cvar("gm_churn", "1")
	g_pRtv      = register_cvar("gm_rtv", "1")
	g_pWelcome  = register_cvar("gm_welcome", "1")

	register_forward(FM_CreateFakeClient, "fw_create_fake_client")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say")
	register_clcmd("gm_welcome_show", "cmd_welcome")

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	load_names()
	g_MapStart = get_gametime()
	set_task(15.0, "task_churn", TASK_CHURN, _, _, "b")
}

public plugin_cfg()
{
	// Tour of Duty missions keep their designed bots (same check as gm_humanbots)
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
	// The game would show motd.txt (the CZ advert) to everyone who joins; a file that
	// doesn't exist means no window, and our own welcome screen takes its place.
	if (!g_Career)
		set_cvar_string("motdfile", "addons/amxmodx/data/gamemodes/no_motd.txt")
}

public plugin_end()
{
	if (!g_Career)
		set_cvar_string("motdfile", "motd.txt")
}

// ---------------------------------------------------------------------------
// Regulars: rename bots as the game creates them
// ---------------------------------------------------------------------------
load_names()
{
	g_Pool = ArrayCreate(32)
	g_Regular = TrieCreate()
	g_Taken = TrieCreate()
	new line[64], f = fopen(fmt("%s/gamemodes/regular_names.ini", g_CfgDir), "rt")
	if (f)
	{
		while (fgets(f, line, charsmax(line)))
		{
			trim(line)
			if (line[0] && line[0] != ';' && strlen(line) < 32)
				ArrayPushString(g_Pool, line)
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
			if (profile[0] && name[0])
			{
				TrieSetString(g_Regular, profile, name)
				TrieSetCell(g_Taken, name, 1)
			}
		}
		fclose(f)
	}
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
	for (new tries = 0; tries < 40; tries++)
	{
		ArrayGetString(g_Pool, random(n), name, len)
		if (!TrieKeyExists(g_Taken, name) && !name_in_use(name))
			return true
	}
	return false
}

public fw_create_fake_client(const netname[])
{
	if (g_Creating || g_Career || !get_pcvar_num(g_pRegulars) || !ArraySize(g_Pool))
		return FMRES_IGNORED

	new name[32], bool:found
	if (TrieGetString(g_Regular, netname, name, charsmax(name)))
	{
		// the regular shows up most of the time; otherwise a newcomer takes the profile
		found = random(100) >= NEWCOMER_PCT && !name_in_use(name)
		if (!found)
			found = free_name(name, charsmax(name))
	}
	else if (free_name(name, charsmax(name)))
	{
		found = true
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
	forward_return(FMV_CELL, ent)
	return FMRES_SUPERCEDE
}

// ---------------------------------------------------------------------------
// Come and go: a bot leaves now and then and the bot count brings in someone new
// ---------------------------------------------------------------------------
public task_churn()
{
	new level = get_pcvar_num(g_pChurn)
	if (level <= 0 || g_Career || g_Voting || get_cvar_num("bot_quota") < 3)
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
	if (id)
		server_cmd("kick #%d", get_user_userid(id))
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

	if (equali(word, "rtv") || equali(word, "rockthevote"))
	{
		rock(id)
		return PLUGIN_CONTINUE      // others see it in chat, like on a real server
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

rocked()
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (g_Rocked[i] && is_user_connected(i))
			n++
	return n
}

needed()
{
	return max(1, floatround(humans() * 0.6, floatround_ceil))
}

rock(id)
{
	if (!get_pcvar_num(g_pRtv))
		return client_print_color(id, print_team_default, "^4[Server]^1 Rock the vote is turned off.")
	if (g_Voting)
		return client_print_color(id, print_team_default, "^4[Server]^1 A map vote is already running.")
	if (g_VoteDone)
		return client_print_color(id, print_team_default, "^4[Server]^1 The map vote already happened on this map.")
	new wait = 30 - floatround(get_gametime() - g_MapStart)
	if (wait > 0)
		return client_print_color(id, print_team_default, "^4[Server]^1 You can rock the vote in %d seconds.", wait)
	if (g_Rocked[id])
		return client_print_color(id, print_team_default, "^4[Server]^1 You already rocked the vote (%d of %d).", rocked(), needed())

	g_Rocked[id] = true
	new name[32]
	get_user_name(id, name, charsmax(name))
	if (rocked() >= needed())
	{
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 rocked the vote. Starting a map vote...", name)
		set_task(1.5, "start_vote", TASK_VOTE)
		g_Voting = true
	}
	else
		client_print_color(0, print_team_default, "^4[Server]^3 %s^1 wants a new map (%d of %d). Say ^4rtv^1 to agree.", name, rocked(), needed())
	return PLUGIN_HANDLED
}

public client_disconnected(id)
{
	g_Rocked[id] = false
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
		return client_print_color(0, print_team_default, "^4[Server]^1 There are no other maps to vote for.")
	}
	new cur[32]
	get_mapname(cur, charsmax(cur))
	copy(g_VoteMaps[g_VoteOptions], charsmax(g_VoteMaps[]), cur)   // last option: keep playing
	arrayset(g_VoteCount, 0, sizeof g_VoteCount)

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
	new best = -1, most = 0, ties
	for (new i = 0; i <= g_VoteOptions; i++)
	{
		if (g_VoteCount[i] > most)
			best = i, most = g_VoteCount[i], ties = 1
		else if (g_VoteCount[i] == most && most > 0 && random(++ties) == 0)
			best = i
	}
	if (best < 0 || best == g_VoteOptions)
		return client_print_color(0, print_team_default, "^4[Server]^1 Staying on ^4%s^1.", g_VoteMaps[g_VoteOptions])
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
// Welcome screen
// ---------------------------------------------------------------------------
public client_putinserver(id)
{
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

show_welcome(id)
{
	static html[1536], host[64], hostesc[96], pname[32], pesc[64], mode[48], desc[96], map[32], opts[48]
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
	len += formatex(html[len], charsmax(html) - len, admin ?
		"<tr><td><b>/gm</b></td><td>game modes, maps, bots and settings</td></tr><tr><td><b>/q</b></td><td>quick menu</td></tr>" :
		"<tr><td><b>/guns</b></td><td>pick your guns (deathmatch modes)</td></tr>")
	len += formatex(html[len], charsmax(html) - len,
		"<tr><td><b>/welcome</b></td><td>show this again</td></tr></table>\
		<p class=d>Play fair, have fun. gl hf!</p></body></html>")
	show_motd(id, html, hostesc[0] ? host : "Welcome")
}
