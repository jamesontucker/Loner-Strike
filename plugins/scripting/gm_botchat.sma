/*
	gm_botchat - bots talk like people on a public server.

	Every bot gets a chat personality from its name, so a regular keeps the same one
	every time it shows up:
	  f friendly    t tryhard    s salty    j joker
	  q quiet       n new player c chill    o old-timer
	  k kid         x English isn't their first language     h hype
	  z zoomer      p streamer wannabe   b backseat gamer    i the team's caller
	  d half asleep u blames themselves  w wholesome         r military roleplay
	  m meme lord   and in capitals: A anime fan, B Brit, C conspiracy theorist, D dad
	  gamer, E esports wannabe, F foodie, G grammar nerd, H hacker-accuser, L lurker,
	  M music guy, N stats nerd, O overconfident, P pacifist, S movement nerd,
	  T map tourist, Y yapper
	On top of that each bot has its own habits, also from its name: how chatty it is,
	its laugh (lol, lmao, xD, kkkk, jajaja, mdr, www...), a word it keeps using (bro,
	mate, dude...), text-speak or not, an emoticon, how often it makes typos and shouts.
	So no two bots in a lobby type alike, and a regular always types the same way.
	What they say comes from configs/gamemodes/botchat.ini: sections per event (a
	headshot, a knife kill, a teamkill, a round won, a map ending, someone saying gg...),
	each line marked with the personalities that would say it. A line can ask for a
	reply ("nice shot" gets "ty" or "lucky lol" from the one it was about), so short
	conversations happen. Bots answer people too: say a bot's name and that one answers.

	Moods: dying again and again tilts a bot (salty lines), and a long run of deaths
	makes it crash out: a few angry lines in a row, sometimes in capitals, and a salty
	one may rage quit (the game fills its slot). Friendly and chill bots cheer up a
	teammate who keeps dying, in team chat. A line starting with @ goes to team chat.

	They type like people: a delay that depends on how long the line is, lowercase and
	the odd typo for the casual ones, and not every time. Most of the time nobody says
	anything, like a real pub. How often is gm_botchat:
	  0 off, 1 now and then, 2 normal, 3 chatty
	gm_personality (0-200, percent, from gm_humanbots) scales it: at 0 bots only say gg
	and answer when spoken to, at 200 they talk about twice as much and lines that fit
	their personality come up more. Lines marked * are anyone's; lines marked with
	personality letters are the "characterful" ones.
	No repeats: a line said in the last while isn't picked again (short ones like "gg"
	or "lol" are fine), and a theme (noscopes, lag, hackers, the map...) rests for a few
	minutes once someone brings it up.
	Other cvars: gm_botchat_humans 1 (bots answer what people say: greetings, gg/gl,
	"ns", insults, "bots?", lag, "rush b"/"wait", their name. One bot answers, now and
	then two, and not every time; the one addressed, nearby or involved answers first)
	Answers use the game: "where is he" gets where their killer was or where the team last
	saw someone (the radar's place names, from the map's nav file), "wheres the bomb" who
	has it or where it's planted, "score?" the score, "timeleft" / "next map" / "what map"
	the real ones, "how" after dying what a teammate saw (the gun, a headshot, the place).
	Moods: lines can be marked for a bot that's tilted (-), on a streak (+), new (^) or one
	that has played the person before (=).
	Threads: when a person answers the bot that just spoke to them (within 20 seconds, by
	name, or quoting it), that bot carries on, two more lines at most; it stops when it's
	fighting or the round ends, and no other bot joins in.

	Clans: bots that share a clan tag ("[XyZ] Name", "XyZ|Name") are clanmates. They
	talk to each other in team chat ([clan]: a clanmate's kill, a clanmate joining),
	and back each other up when one keeps dying.

	Rivals: a player who keeps killing a bot this map becomes its nemesis ([nemesis],
	and [nemesis_won] when it finally gets them).
	Memory: bots remember the people they've played with on earlier days (data/vault
	gm_botmemory) and greet them like old faces ([remember], [remember_rival]).
	Leaving: a bot that leaves mid-map says so ([leave]); one that left may come back
	later ([rejoin]). Other plugins use the botchat_say / botchat_persona natives.

	Placeholders in lines: {name} the bot, {other} who it's about or answering,
	{map} the map, {tag} the bot's clan tag. Edit botchat.ini to add your own; gm_botchat_reload re-reads it.
*/

#include <amxmodx>
#include <amxmisc>
#include <nvault>
#include <cstrike>
// sockets (optional, for gm_ai_url): without the module the plugin still loads, the
// feature just stays off (module_filter / native_filter)
#include <sockets>

// gm_community: true for chat that never shows (muted player, filtered out)
native gm_chat_blocked(id, const text[])

#define PLUGIN  "Bot chat"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define MAX_SECTIONS 160
#define TASK_SAY     8100
#define MAX_PENDING  16
#define MAX_CHAIN    3

new const PERSONAS[] = "ftsjqncokxhzpbiduwrmABCDEFGHLMNOPSTYIJKQRUVWXZ0123aeglvy456789"
// how much each personality talks (same order as PERSONAS)
new const Float:CHATTY[] = { 1.0, 0.8, 1.2, 1.4, 0.3, 1.0, 0.7, 0.8, 1.5, 0.8, 1.6, 1.2, 1.3, 1.3, 1.1, 0.6, 0.9, 1.0, 1.0, 1.3,
	1.1, 1.0, 1.1, 0.8, 1.0, 1.0, 1.0, 1.2, 0.15, 1.0, 1.0, 1.3, 0.9, 1.0, 1.0, 1.6,
	1.0, 1.0, 1.2, 1.0, 0.9, 1.0, 1.0, 0.8, 1.2, 0.6, 1.0, 1.0, 1.0, 1.1,
	1.0, 1.1, 1.0, 1.1, 1.3, 1.0, 0.6, 0.9, 0.4, 1.2, 1.2, 0.9 }
// how common each personality is (per 1000; same order): most people on a pub are
// ordinary players who say little; the gimmicks (pirates, anime fans, streamers...) are
// left out, they read like someone wrote them
new const WEIGHT[] = { 90, 70, 70, 35, 120, 60, 100, 70, 12, 40, 5, 0, 0, 20, 30, 30, 35, 10, 0, 0,
	0, 25, 0, 20, 3, 0, 3, 15, 110, 0, 5, 10, 0, 8, 0, 3,
	0, 0, 4, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 2,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }

// habits, picked per bot from its name
// the first ones are for anyone; the last six only for people whose English isn't first
new const LAUGHS[][] = { "lol", "lol", "lol", "lol", "haha", "hehe", "lol", "xD", "hah", "rofl", "lmao", "haha", "kkkk", "jajaja", "mdr", "www", "555", "hehe" }
#define LAUGHS_ANYONE 12
new const WORDS[][] = { "", "", "", "", "", "", "", "", "man", "dude", "mate", "guys", "m8", "lads", "bro", "" }
new const EMOTES[][] = { "", "", "", "", "", ":)", ":D", "^^", ":P", ";)", ":/", ":(", "=)" }
new g_Laugh[33], g_Word[33], g_Emote[33], bool:g_TextSpeak[33], g_Typos[33], g_Shouty[33], Float:g_Chatty[33]

new g_pLevel, g_pHumans, g_pDeadChat
new g_pPersonality                  // gm_personality (gm_humanbots), 0 if it isn't there
new g_CfgDir[128]
new Trie:g_Sections                 // section name -> Array of "letters|text"
new Array:g_Lists[MAX_SECTIONS], g_ListCount
new bool:g_Career

new g_Persona[33]                   // index into PERSONAS
new Float:g_NextBot[33]             // when this bot may talk again
new Float:g_NextAny                 // when any bot may start a new line
new Float:g_LastRound               // when the last round started (none in Gun Game or Deathmatch)
new g_RoundKills[33], bool:g_Streaked[33]
new g_DeathRow[33]                  // deaths since this player's last kill
new bool:g_Raged[33]
new Float:g_NextHeard
new g_KilledBy[33][33]
new g_WinRun[3], g_LoseRun[3]        // rounds won / lost in a row, per side               // this map: how often [killer][victim]
new g_Vault = INVALID_HANDLE
new Trie:g_Left                      // bots that left mid-map: name -> when

// lines said lately (hashes), so the same one doesn't come round again soon
#define RECENT 64
new g_Recent[RECENT], g_RecentPos

// themes rest for a while once someone brings them up: a word in the line -> theme
new const THEME_WORD[][] = { "noscope", "no scope", "scope", "lag", "laggy", "hitreg", "reg", "choke", "ping",
	"hack", "hacks", "hacking", "aimbot", "sus", "demo", "sleep", "tired", "bed", "3am", "zzz",
	"this map", "map again", "{map}", "mouse", "fps", "camp", "camper", "camping", "awp", "1.5", "beta", "old" }
new const THEME_ID[] = { 0, 0, 0, 1, 1, 1, 1, 1, 1,
	2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3,
	4, 4, 4, 5, 5, 6, 6, 6, 7, 8, 8, 8 }
// seconds each theme rests: noscope, lag, hackers, sleep, the map, gear, camping, awp, the old days
new const Float:THEME_REST[] = { 900.0, 150.0, 240.0, 600.0, 300.0, 600.0, 180.0, 90.0, 900.0 }
new Float:g_ThemeAt[sizeof THEME_REST]

// who's involved, for answering people: the last bot to get a kill, who killed each
// person last and whom they killed last
new g_LastBotKiller, Float:g_LastBotKillAt
new g_KillerOf[33], Float:g_KillerOfAt[33], g_VictimOf[33], Float:g_VictimOfAt[33]
// answers lately: the more bots have answered, the less they answer the next thing
new g_HeardRun, Float:g_HeardRunAt

// Game state, for answers that use it (empty when the game doesn't say)
new g_Place[33][32]                  // where each player last was: the map's place names (CZ navs)
new g_KillerPlace[33][32]            // how each person last died: where the killer was,
new g_KillWeapon[33][16], bool:g_KillHs[33], Float:g_DiedAt[33]   // with what, a headshot, when
new g_SeenPlace[3][32], Float:g_SeenAt[3]  // where each team last saw an enemy (a teammate's killer)
new g_BombPlace[32], bool:g_BombPlanted, bool:g_BombDropped
new g_Score[3]                       // team scores: 1 T, 2 CT
new Float:g_FightAt[33]              // last time a player hurt someone or got hurt
new Float:g_JoinedAt[33]
new Float:g_FillerAt                 // last "?", "idk", "ya": they read as robotic when they come often
// What pick_line may put in a line ({place} {weapon} {carrier} {hs}); a line asking for one
// that's empty isn't picked
new g_CtxPlace[32], g_CtxWeapon[16], g_CtxCarrier[32], bool:g_CtxHs, g_CtxOther
// A short thread with a person (per person): the bot talking to them, what about, how many
// lines it has said in it, when it last spoke and what it said
new g_ThreadBot[33], g_ThreadTopic[33][24], g_ThreadTurns[33], Float:g_ThreadAt[33], g_ThreadLine[33][128]

// lines waiting to be "typed"
new g_PendBot[MAX_PENDING], g_PendUser[MAX_PENDING], g_PendOther[MAX_PENDING], g_PendChain[MAX_PENDING], bool:g_PendTeam[MAX_PENDING]
new g_PendText[MAX_PENDING][128], g_PendSec[MAX_PENDING][24], bool:g_PendQuiet[MAX_PENDING]   // quiet: starts no thread
new bool:g_PendRaw[MAX_PENDING]     // the model's text: said as it is

// smarter chat (gm_ai_url): one request per bot
#define TASK_AI    2000000
#define AI_TIMEOUT 4.5
#define HIST       12
new g_pAiUrl, bool:g_Sockets
new g_AiSock[33], g_AiState[33]     // state: 0 none, 1 connecting, 2 waiting for the answer
new Float:g_AiStart[33], g_AiUser[33], g_AiHuman[33], g_AiHumanUser[33], bool:g_AiTeam[33], Float:g_AiExtra[33], g_AiRound[33]
new g_AiFallback[33][128], g_AiSection[33][32], g_AiReq[33][2048], g_AiSent[33], g_AiResp[33][1024], g_AiGot[33]
new g_Round                         // counts rounds, so an answer from last round is dropped
new g_Hist[HIST][100], g_HistTeam[HIST], g_HistPos, g_HistCount   // recent chat
new g_HumanMsg[128]                 // what the person typed, as they typed it

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pLevel  = register_cvar("gm_botchat", "3")
	g_pHumans = register_cvar("gm_botchat_humans", "1")
	g_pDeadChat = register_cvar("gm_botchat_deadchat", "1")
	register_srvcmd("gm_botchat_reload", "cmd_reload")
	// the app sets it to "127.0.0.1:port" while its model is ready, "" when not
	g_pAiUrl = get_cvar_pointer("gm_ai_url")
	if (!g_pAiUrl)
		g_pAiUrl = register_cvar("gm_ai_url", "")

	register_event("DeathMsg", "ev_death", "a")
	register_event("SendAudio", "ev_twin", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ctwin", "a", "2&%!MRAD_ctwin")
	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	set_task(40.0, "task_idle_chatter")
	register_event("30", "ev_intermission", "a")
	register_logevent("ev_planted", 3, "2=Planted_The_Bomb")
	register_logevent("ev_defused", 3, "2=Defused_The_Bomb")
	register_logevent("ev_bomb_dropped", 3, "2=Dropped_The_Bomb")
	register_logevent("ev_bomb_got", 3, "2=Got_The_Bomb")
	register_logevent("ev_bomb_got", 3, "2=Spawned_With_The_Bomb")
	// where people are (the place names on the radar, from the map's nav file), the score,
	// who's fighting
	if (get_user_msgid("Location"))
		register_event("Location", "ev_location", "ab")
	register_event("TeamScore", "ev_teamscore", "a")
	register_event("Damage", "ev_damage", "b", "2!0")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say_team")

	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	g_Sections = TrieCreate()
	load_lines()
}

public plugin_natives()
{
	register_library("gm_botchat")
	register_native("botchat_persona", "n_persona")
	register_native("botchat_say", "n_say")
	// still loads without gm_community, and without the sockets module
	set_native_filter("native_filter")
	set_module_filter("module_filter")
}

public module_filter(const library[], LibType:type)
{
	return equal(library, "sockets") ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

public native_filter(const name[], index, trap)
{
	return (!trap && (equal(name, "gm_chat_blocked") || equal(name, "socket_", 7))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

// botchat_persona(id): the bot's personality letter (0 for people)
public n_persona(plugin, params)
{
	new id = get_param(1)
	return (id >= 1 && id <= MaxClients && is_user_bot(id)) ? PERSONAS[g_Persona[id]] : 0
}

// botchat_say(id, const section[], other, Float:chance): a line from `section`, maybe
public n_say(plugin, params)
{
	new id = get_param(1), section[32]
	if (id < 1 || id > MaxClients || !is_user_connected(id) || !is_user_bot(id))
		return 0
	get_string(2, section, charsmax(section))
	strtolower(section)
	new Float:chance = get_param_f(4)
	if (equal(section, "leave"))
	{
		// remembered, so it can come back later
		new name[32]
		get_user_name(id, name, charsmax(name))
		TrieSetCell(g_Left, name, floatround(get_gametime()))
		// about half say bye (fewer at a low gm_personality); the rest just go
		new text[128]
		if (get_pcvar_num(g_pLevel) && !g_Career && random_float(0.0, 100.0) <= 15.0 + 30.0 * floatmin(personality(), 1.5)
			&& pick_line(id, "leave", text, charsmax(text), true))
			queue(id, 0, text, MAX_CHAIN, false, random_float(0.3, 1.2))
		return 1
	}
	want(id, section, get_param(3), chance)
	return 1
}

public plugin_cfg()
{
	personality_cvar()
	ai_init()
	for (new i = 0; i < sizeof g_ThemeAt; i++)
		g_ThemeAt[i] = -10000.0
	g_Vault = nvault_open("gm_botmemory")
	g_Left = TrieCreate()
	// Tour of Duty missions keep their designed bots
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
}

public cmd_reload()
{
	TrieClear(g_Sections)
	for (new i = 0; i < g_ListCount; i++)
		ArrayDestroy(g_Lists[i])
	g_ListCount = 0
	load_lines()
	server_print("[botchat] %d sections", g_ListCount)
	return PLUGIN_HANDLED
}

load_lines()
{
	new f = fopen(fmt("%s/gamemodes/botchat.ini", g_CfgDir), "rt")
	if (!f)
		return
	new line[192], Array:cur = Invalid_Array
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';')
			continue
		if (line[0] == '[')
		{
			new name[48]
			copyc(name, charsmax(name), line[1], ']')
			strtolower(name)
			if (!TrieGetCell(g_Sections, name, cur))
			{
				if (g_ListCount >= MAX_SECTIONS)
				{
					cur = Invalid_Array
					continue
				}
				cur = g_Lists[g_ListCount++] = ArrayCreate(192)
				TrieSetCell(g_Sections, name, cur)
			}
			continue
		}
		if (cur != Invalid_Array && contain(line, "|") > 0)
			ArrayPushString(cur, line)
	}
	fclose(f)
}

// ---- who the bots are ----------------------------------------------------------------

public client_putinserver(id)
{
	g_NextBot[id] = get_gametime() + random_float(5.0, 20.0)
	g_RoundKills[id] = 0
	g_DeathRow[id] = 0
	g_KillerOf[id] = g_VictimOf[id] = 0
	g_Raged[id] = false
	g_Place[id][0] = g_KillerPlace[id][0] = 0
	g_DiedAt[id] = g_FightAt[id] = -100.0
	g_JoinedAt[id] = get_gametime()
	g_ThreadBot[id] = 0
	for (new k = 1; k <= MaxClients; k++)
		g_KilledBy[id][k] = g_KilledBy[k][id] = 0
	if (is_user_bot(id))
	{
		g_Persona[id] = persona_of(id)
		habits_of(id)
		new name[32], when
		get_user_name(id, name, charsmax(name))
		if (TrieGetCell(g_Left, name, when))
		{
			TrieDeleteKey(g_Left, name)
			want(id, "rejoin", 0, 0.8, random_float(3.0, 8.0), 1)
			return
		}
		if (random_num(1, 100) <= 30)
			want(id, "greet", 0, 0.5, random_float(4.0, 12.0))
		// a clanmate already here says hi to them
		set_task(random_float(6.0, 15.0), "task_clan_hello", id)
	}
	else if (!is_user_hltv(id))
		set_task(random_float(5.0, 12.0), "task_welcome", id)
}

public task_clan_hello(id)
{
	new mate = is_user_connected(id) ? clanmate_bot(id, false) : 0
	if (mate)
		want(mate, "clan_join", id, 0.6)
}

public task_welcome(id)
{
	if (!is_user_connected(id))
		return
	// an old face? the bot that played them most says hi like it
	new best, bestGames, bool:rival
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b))
			continue
		new k, d, day
		if (recall(b, id, k, d, day) && day < today() && k + d > bestGames)
		{
			best = b
			bestGames = k + d
			rival = k + d >= 12
		}
	}
	if (best)
		want(best, rival ? "remember_rival" : "remember", id, 0.7, -1.0, 1)
	else
		want(any_bot(0), "welcome", id, 0.4)
}

// ---- memory between days --------------------------------------------------------------

today()
{
	return get_systime() / 86400
}

memory_key(bot, human, key[], len)
{
	new a[32], b[32]
	get_user_name(bot, a, charsmax(a))
	get_user_name(human, b, charsmax(b))
	formatex(key, len, "%s|%s", a, b)
}

// What `bot` remembers of `human`: kills each way and the day they last played
bool:recall(bot, human, &kills, &deaths, &day)
{
	if (g_Vault == INVALID_HANDLE)
		return false
	new key[72], val[32], ts
	memory_key(bot, human, key, charsmax(key))
	if (!nvault_lookup(g_Vault, key, val, charsmax(val), ts))
		return false
	new a[12], b[12], c[12]
	parse(val, a, charsmax(a), b, charsmax(b), c, charsmax(c))
	kills = str_to_num(a)
	deaths = str_to_num(b)
	day = str_to_num(c)
	return true
}

// Adds this map's kills between `human` and every bot to what they remember
remember_all(human)
{
	if (g_Vault == INVALID_HANDLE || !is_user_connected(human) || is_user_bot(human))
		return
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b))
			continue
		new k, d, day
		recall(b, human, k, d, day)
		k += g_KilledBy[b][human]
		d += g_KilledBy[human][b]
		new key[72]
		memory_key(b, human, key, charsmax(key))
		// only people who actually played each other
		if (g_KilledBy[b][human] || g_KilledBy[human][b])
			nvault_set(g_Vault, key, fmt("%d %d %d", k, d, today()))
		g_KilledBy[b][human] = g_KilledBy[human][b] = 0
	}
}

public client_disconnected(id)
{
	if (!is_user_bot(id))
		remember_all(id)
	if (g_AiState[id])
		ai_close(id)
	// threads with this bot end (the slot may go to someone else)
	for (new h = 1; h <= MaxClients; h++)
		if (g_ThreadBot[h] == id)
			g_ThreadBot[h] = 0
	g_ThreadBot[id] = 0
	// nothing left waiting to be said by whoever had this slot
	for (new i = 0; i < MAX_PENDING; i++)
		if (g_PendBot[i] == id)
		{
			g_PendBot[i] = 0
			remove_task(TASK_SAY + i)
		}
}

// A dead bot's line, shown to the living too (with the game's own *DEAD* tag)
dead_chat(id, const text[], bool:team)
{
	static msg
	if (!msg)
		msg = get_user_msgid("SayText")
	new t = get_user_team(id)
	new fmt_[32]
	copy(fmt_, charsmax(fmt_), team ? (t == 1 ? "#Cstrike_Chat_T_Dead" : "#Cstrike_Chat_CT_Dead") : "#Cstrike_Chat_AllDead")
	for (new r = 1; r <= MaxClients; r++)
	{
		if (!is_user_connected(r) || is_user_bot(r) || !is_user_alive(r) || (team && get_user_team(r) != t))
			continue
		message_begin(MSG_ONE, msg, _, r)
		write_byte(id)
		write_string(fmt_)
		write_string("")
		write_string(text)
		message_end()
	}
}

public plugin_end()
{
	for (new i = 1; i <= MaxClients; i++)
		if (g_AiState[i])
			ai_close(i)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i))
			remember_all(i)
	if (g_Vault != INVALID_HANDLE)
	{
		// faces not seen for two months are forgotten
		nvault_prune(g_Vault, 0, get_systime() - 60 * 86400)
		nvault_close(g_Vault)
	}
}

// The clan tag in a name, as the clan's letters ("WoW" for "~WoW~ Squishy"): a first
// word wrapped in symbols ("[XyZ] Name", "~WoW~ Name", "|LoL| Name", "=DD= yuki",
// "<<OGs>> Munch"), or a short word before a bar ("GG | Phantom", "XyZ|Name")
clan_of(id, tag[], len)
{
	tag[0] = 0
	new name[32]
	get_user_name(id, name, charsmax(name))
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

bool:clanmates(a, b)
{
	if (a == b || !is_user_connected(a) || !is_user_connected(b))
		return false
	new ta[16], tb[16]
	clan_of(a, ta, charsmax(ta))
	clan_of(b, tb, charsmax(tb))
	return ta[0] && equali(ta, tb)
}

// A bot clanmate of `id` (on its team if `team` is set), or 0
clanmate_bot(id, bool:sameteam)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != id && is_user_connected(i) && is_user_bot(i) && clanmates(i, id) && (!sameteam || get_user_team(i) == get_user_team(id)))
			list[n++] = i
	return n ? list[random(n)] : 0
}

// The playstyle personalities (an AWPer, a rusher, a camper...), picked on a roll of their
// own (NEW_SHARE per 1000 bots) so adding them left every other regular's personality as it
// was. Letters and weights; Z is the clutch hero, K wants the admin (the rule enforcer)
new const NEW_KINDS[] = "aeg4567l89vyZK"
new const NEW_WEIGHT[] = { 25, 30, 10, 25, 10, 8, 6, 15, 15, 6, 10, 8, 8, 10 }
#define NEW_SHARE 170

// From the name, so the same regular always has the same personality. New players
// in the Newbie lobby style are more often new to the game too.
persona_of(id)
{
	new name[32], h = 7
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h * 31 + name[i]) & 0xFFFFFF
	// the playstyle roll, from another mix of the name
	new h2 = 2166136261
	for (new i = 0; name[i]; i++)
		h2 = ((h2 ^ name[i]) * 16777619) & 0x7FFFFFFF
	if ((h2 / 13) % 1000 < NEW_SHARE && get_cvar_num("gm_botstyle") != 4)
	{
		new total
		for (new i = 0; i < sizeof NEW_WEIGHT; i++)
			total += NEW_WEIGHT[i]
		new r = (h2 / 13001) % total
		for (new i = 0; i < sizeof NEW_WEIGHT; i++)
		{
			if (r < NEW_WEIGHT[i])
				return contain(PERSONAS, fmt("%c", NEW_KINDS[i]))
			r -= NEW_WEIGHT[i]
		}
	}
	if (get_cvar_num("gm_botstyle") == 4 && h % 3 == 0)
		return 5
	// weighted: plain players are common, gimmicks rare
	new total
	for (new i = 0; i < sizeof WEIGHT; i++)
		total += WEIGHT[i]
	new r = h % total
	for (new i = 0; i < sizeof WEIGHT; i++)
	{
		if (r < WEIGHT[i])
			return i
		r -= WEIGHT[i]
	}
	return 0
}

// A bot's own habits, from its name (a different mix of the name than its personality)
habits_of(id)
{
	new name[32], h = 2166136261
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h ^ name[i]) * 16777619
	h &= 0x7FFFFFFF
	g_Laugh[id] = h % LAUGHS_ANYONE
	g_Word[id] = (h / 7) % sizeof WORDS
	g_Emote[id] = (h / 31) % sizeof EMOTES
	g_TextSpeak[id] = (h / 101) % 3 == 0
	g_Typos[id] = (h / 211) % 15             // percent
	g_Shouty[id] = (h / 401) % 4 == 0 ? 8 : 1
	// a few people do most of the talking: most say little, some a lot
	new tier = (h / 977) % 100
	g_Chatty[id] = tier < 45 ? 0.15 : tier < 75 ? 0.6 : tier < 93 ? 1.5 : 3.0
	new p = PERSONAS[g_Persona[id]]
	if (p == 'x' && g_Laugh[id] < 6)
		g_Laugh[id] = LAUGHS_ANYONE + (h % 6)   // kkkk, jajaja, mdr, www, 555, kek
	if (p == 'k')
		g_TextSpeak[id] = true
}

bool:has_persona(id, const letters[])
{
	if (letters[0] == '*')
		return true
	return contain(letters, fmt("%c", PERSONAS[g_Persona[id]])) != -1
}

// How a bot is doing, as the mood marks lines can carry: - tilted or losing (short, salty),
// + on a streak or winning (cocky, relaxed), ^ just joined (polite), = knows the person it's
// talking to (casual; `other`, a person)
mood_of(id, other, out[], len)
{
	out[0] = 0
	new team = get_user_team(id)
	new diff = get_user_frags(id) - get_user_deaths(id)
	new bool:losing = team >= 1 && team <= 2 && g_LoseRun[team] >= 3
	new bool:winning = team >= 1 && team <= 2 && g_WinRun[team] >= 3
	if (g_DeathRow[id] >= 3 || losing || diff <= -6)
		add(out, len, "-")
	else if (g_RoundKills[id] >= 2 || winning || diff >= 8)
		add(out, len, "+")
	if (get_gametime() - g_JoinedAt[id] < 150.0)
		add(out, len, "^^")
	if (other && is_user_connected(other) && !is_user_bot(other))
	{
		new k, d, day
		if ((recall(id, other, k, d, day) && day < today()) || g_KilledBy[id][other] + g_KilledBy[other][id] >= 6)
			add(out, len, "=")
	}
}

// Fights in the last few seconds: no time to type
bool:in_combat(id)
{
	return get_gametime() - g_FightAt[id] < 5.0
}

// ---- deciding to say something ---------------------------------------------------------

personality_cvar()
{
	g_pPersonality = get_cvar_pointer("gm_personality")
}

// gm_personality as a factor: 0.0 (bots keep to themselves) to 2.0 (chatty), 1.0 without it
Float:personality()
{
	if (!g_pPersonality)
		personality_cvar()
	if (!g_pPersonality)
		return 1.0
	return float(clamp(get_pcvar_num(g_pPersonality), 0, 200)) / 100.0
}

Float:level_mult()
{
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: return 0.2
		case 3: return 1.8
	}
	return 0.6
}

// gaps get longer as personality goes down (x1.5 at 0) and shorter as it goes up (x0.5 at 200)
Float:gap_mult()
{
	return 1.5 - 0.5 * personality()
}

Float:global_gap()
{
	new Float:g
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: g = 40.0
		case 3: g = 4.0
		default: g = 12.0
	}
	return g * gap_mult()
}

Float:bot_gap()
{
	new Float:g
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: g = 150.0
		case 3: g = 20.0
		default: g = 60.0
	}
	return g * gap_mult()
}

// A bot might say a line from `section` about `other`, with the chance `base`
// (before the chattiness setting, gm_personality and its own personality). chain > 0: a
// reply. Answers to people go through answer() instead.
want(id, const section[], other, Float:base, Float:delay = -1.0, chain = 0, bool:team = false)
{
	if (!id || !is_user_connected(id) || !is_user_bot(id) || g_Career || !get_pcvar_num(g_pLevel))
		return
	new Float:now = get_gametime()
	if (!chain && (now < g_NextAny || now < g_NextBot[id]))
		return
	// at gm_personality 0 this is 0: bots only answer people and say gg
	if (random_float(0.0, 1.0) > base * level_mult() * personality() * CHATTY[g_Persona[id]] * g_Chatty[id])
		return
	new text[128]
	g_CtxOther = other
	new bool:ok = pick_line(id, section, text, charsmax(text))
	g_CtxOther = 0
	if (ok)
		queue(id, other, text, chain, team, delay, section)
}

// A line's theme (THEME_REST), or -1
theme_of(const text[])
{
	new low[128]
	copy(low, charsmax(low), text)
	strtolower(low)
	for (new w = 0; w < sizeof THEME_WORD; w++)
		if (word_in(low, THEME_WORD[w]))
			return THEME_ID[w]
	return -1
}

line_hash(const text[])
{
	new h = 5381
	for (new i = 0; text[i]; i++)
	{
		new c = text[i]
		if (c >= 'A' && c <= 'Z')
			c += 32
		h = ((h << 5) + h + c) & 0x7FFFFFFF
	}
	return h ? h : 1
}

bool:said_lately(h)
{
	for (new i = 0; i < RECENT; i++)
		if (g_Recent[i] == h)
			return true
	return false
}

// "?", "idk", "ya" and the like: fine now and then, robotic when they keep coming
bool:is_filler(const text[])
{
	static const FILLER[][] = { "?", "??", "idk", "ya", "k", "hm", "hm?", "huh", "dunno", "wat", "what", "yep", "nope", "idk lol" }
	new t[16]
	copy(t, charsmax(t), text)
	trim(t)
	for (new i = 0; i < sizeof FILLER; i++)
		if (equali(t, FILLER[i]))
			return true
	return false
}

// A line asking for game state ({place}, {weapon}...) only when that's known now
bool:ctx_ok(const text[])
{
	if (contain(text, "{") == -1)
		return true
	if (contain(text, "{place}") != -1 && !g_CtxPlace[0])
		return false
	if (contain(text, "{weapon}") != -1 && !g_CtxWeapon[0])
		return false
	if (contain(text, "{carrier}") != -1 && !g_CtxCarrier[0])
		return false
	if (contain(text, "{hs}") != -1 && !g_CtxHs)
		return false
	if (contain(text, "{timeleft}") != -1 && get_timeleft() <= 0)
		return false
	if (contain(text, "{nextmap}") != -1)
	{
		new next[32]
		next_map(next, charsmax(next))
		if (!next[0])
			return false
	}
	return true
}

bool:uses_state(const text[])
{
	return contain(text, "{place}") != -1 || contain(text, "{weapon}") != -1 || contain(text, "{carrier}") != -1
		|| contain(text, "{us}") != -1 || contain(text, "{diff}") != -1 || contain(text, "{timeleft}") != -1
		|| contain(text, "{nextmap}") != -1 || contain(text, "{hs}") != -1
}

// The game state in a line, as people would put it ("b", "deag", "7 5", "3 min")
fill_state(id, text[], len)
{
	if (contain(text, "{") == -1)
		return
	replace_string(text, len, "{place}", g_CtxPlace)
	replace_string(text, len, "{weapon}", g_CtxWeapon)
	replace_string(text, len, "{carrier}", g_CtxCarrier)
	replace_string(text, len, "{hs}", "hs")
	new team = get_user_team(id)
	if (team == 1 || team == 2)
	{
		new us = g_Score[team], them = g_Score[3 - team]
		replace_string(text, len, "{us}", fmt("%d", us))
		replace_string(text, len, "{them}", fmt("%d", them))
		replace_string(text, len, "{diff}", fmt("%d", abs(us - them)))
	}
	if (contain(text, "{timeleft}") != -1)
	{
		new left = get_timeleft(), when[16]
		if (left >= 90)
			formatex(when, charsmax(when), "%d min", (left + 30) / 60)
		else if (left >= 45)
			copy(when, charsmax(when), "1 min")
		else
			formatex(when, charsmax(when), "%d sec", max(left, 5))
		replace_string(text, len, "{timeleft}", when)
	}
	if (contain(text, "{nextmap}") != -1)
	{
		new next[32]
		next_map(next, charsmax(next))
		replace_string(text, len, "{nextmap}", next)
	}
}

// A map's name as people say it: "dust2" for de_dust2
casual_map(const map[], out[], len)
{
	new at = 0
	if (strlen(map) > 3 && map[2] == '_')
		at = 3
	copy(out, len, map[at])
}

// The next map (nextmap.amxx / gm_extras put it in amx_nextmap), as people say it; "" if
// unknown or this one again
next_map(out[], len)
{
	out[0] = 0
	if (!get_cvar_pointer("amx_nextmap"))
		return
	new next[32], map[32]
	get_cvar_string("amx_nextmap", next, charsmax(next))
	get_mapname(map, charsmax(map))
	if (!next[0] || equali(next, map) || containi(next, "[") != -1)
		return
	casual_map(next, out, len)
}

// A line from `section` for this bot: anyone's lines (*), lines for its personality
// (weighted by gm_personality) and lines for its mood (MOOD_OF: tilted, on a streak, new,
// knows the person). Not one said lately, not a theme that's resting, and only lines whose
// game state is known; lines with game state in them come up more.
// answer: answering a person, so a fitting line beats a fresh one
bool:pick_line(id, const section[], text[], len, bool:answer = false)
{
	new Array:list
	if (!TrieGetCell(g_Sections, section, list))
		return false
	new n = ArraySize(list)
	if (!n)
		return false
	new Float:p = personality(), Float:now = get_gametime()
	new mood[8]
	mood_of(id, g_CtxOther, mood, charsmax(mood))
	new raw[192], letters[64], pick = -1, Float:total, fallback = -1, fseen
	for (new i = 0; i < n; i++)
	{
		ArrayGetString(list, i, raw, charsmax(raw))
		new bar = contain(raw, "|")
		copy(letters, min(bar, charsmax(letters)), raw)
		new body = bar + 1
		new Float:w
		new bool:moody = false
		for (new m = 0; mood[m]; m++)
			if (contain(letters, fmt("%c", mood[m])) != -1)
				moody = true
		if (moody)
			w = 1.5 + 0.5 * p                // how it's doing shows whatever the setting
		else if (letters[0] == '*')
			w = 1.0
		else if (has_persona(id, letters))
			w = p
		if (w <= 0.0 || !ctx_ok(raw[body]))
			continue
		if (uses_state(raw[body]))
			w *= 2.0
		new bool:fresh = true
		// short lines ("gg", "lol", "ns") come up all the time on a real server
		if (strlen(raw[body]) > 6 && said_lately(line_hash(raw[body])))
			fresh = false
		new t = theme_of(raw[body])
		if (t >= 0 && now - g_ThemeAt[t] < THEME_REST[t] && !answer)
			fresh = false
		if (is_filler(raw[body]) && now - g_FillerAt < 120.0)
			fresh = false
		if (!fresh)
		{
			if (random(++fseen) == 0)
				fallback = i
			continue
		}
		total += w
		if (random_float(0.0, total) < w)
			pick = i
	}
	if (pick < 0)
	{
		// everything fitting was said lately: keep quiet, unless answering someone
		if (!answer || fallback < 0)
			return false
		pick = fallback
	}
	ArrayGetString(list, pick, raw, charsmax(raw))
	copy(text, len, raw[contain(raw, "|") + 1])
	trim(text)
	g_Recent[g_RecentPos] = line_hash(text)
	g_RecentPos = (g_RecentPos + 1) % RECENT
	new t = theme_of(text)
	if (t >= 0)
		g_ThemeAt[t] = now
	if (is_filler(text))
		g_FillerAt = now
	fill_state(id, text, len)
	return true
}

// Reading what happened, then typing it: people type about 6 to 10 characters a second
// in game, and take a moment before they start
Float:typing_time(const text[], Float:read_min = 0.8, Float:read_max = 2.2)
{
	return random_float(read_min, read_max) + float(strlen(text)) * random_float(0.1, 0.17)
}

queue(id, other, const text[], chain, bool:team, Float:delay, const section[] = "", bool:quiet = false, bool:raw = false)
{
	new slot = -1
	for (new i = 0; i < MAX_PENDING; i++)
		if (!g_PendBot[i])
		{
			slot = i
			break
		}
	if (slot < 0)
		return
	g_PendBot[slot] = id
	g_PendUser[slot] = get_user_userid(id)
	g_PendOther[slot] = other
	g_PendChain[slot] = chain
	g_PendTeam[slot] = team
	copy(g_PendText[slot], charsmax(g_PendText[]), text)
	copy(g_PendSec[slot], charsmax(g_PendSec[]), section)
	g_PendQuiet[slot] = quiet
	g_PendRaw[slot] = raw
	if (delay < 0.0)
		delay = typing_time(text)
	new Float:now = get_gametime()
	g_NextBot[id] = now + delay + bot_gap() * random_float(0.6, 1.4) / floatsqroot(floatmax(g_Chatty[id], 0.15))
	if (!chain)
		g_NextAny = now + delay + global_gap() * random_float(0.6, 1.4)
	set_task(delay, "task_say", TASK_SAY + slot)
}

public task_say(task)
{
	new slot = task - TASK_SAY
	new id = g_PendBot[slot], other = g_PendOther[slot], chain = g_PendChain[slot]
	g_PendBot[slot] = 0
	// still the same bot (the slot may have gone to someone else meanwhile)
	if (!is_user_connected(id) || !is_user_bot(id) || get_user_userid(id) != g_PendUser[slot])
		return
	if (other && !is_user_connected(other))
		other = 0

	new text[128], tag[24]
	copy(text, charsmax(text), g_PendText[slot])
	new bool:team = g_PendTeam[slot]
	// the model's text is said as it came (no tags, placeholders or added typos)
	if (!g_PendRaw[slot])
	{
		// a reply asked for: "...  >tag"
		new at = contain(text, ">")
		if (at > 0)
		{
			copy(tag, charsmax(tag), text[at + 1])
			trim(tag)
			text[at] = 0
			trim(text)
		}
		// "@..." is said to the team
		if (text[0] == '@')
		{
			team = true
			copy(text, charsmax(text), text[1])
			trim(text)
		}
		fill(id, other, text, charsmax(text))
		style_text(id, text)
		decorate(id, text, charsmax(text))
	}
	if (!text[0])
		return
	engclient_cmd(id, team ? "say_team" : "say", text)
	hist_add(id, text, team ? get_user_team(id) : 0)
	// the living don't see the dead talk; like pub servers' dead-chat plugins, show it to them
	if (!is_user_alive(id) && get_pcvar_num(g_pDeadChat))
		dead_chat(id, text, team)
	// said to a person who can see it: they may answer, and this bot carries on (a thread)
	if (other && !is_user_bot(other) && !g_PendQuiet[slot] && (!team || get_user_team(other) == get_user_team(id)))
		thread_said(other, id, g_PendSec[slot], text)

	// someone answers: the one it was about if that's a bot (and can see it), else
	// another bot (a teammate, for team chat)
	if (tag[0] && chain < MAX_CHAIN)
	{
		new who
		if (other && is_user_bot(other) && other != id && (!team || get_user_team(other) == get_user_team(id)))
			who = other
		else
			who = team ? any_bot_on(get_user_team(id), id) : any_bot(id)
		want(who, fmt("r_%s", tag), id, 0.5, -1.0, chain + 1, team)
	}
}

fill(id, other, text[], len)
{
	new name[32]
	if (contain(text, "{name}") != -1)
	{
		short_name(id, name, charsmax(name))
		replace_string(text, len, "{name}", name)
	}
	if (contain(text, "{other}") != -1)
	{
		if (other && is_user_connected(other))
			short_name(other, name, charsmax(name))
		else
			copy(name, charsmax(name), "man")
		replace_string(text, len, "{other}", name)
	}
	if (contain(text, "{tag}") != -1)
	{
		new tag[16]
		clan_of(id, tag, charsmax(tag))
		replace_string(text, len, "{tag}", tag[0] ? tag : "the clan")
	}
	if (contain(text, "{map}") != -1)
	{
		new map[32]
		get_mapname(map, charsmax(map))
		casual_map(map, name, charsmax(name))
		replace_string(text, len, "{map}", name)
	}
}

// What people call each other: the name without its clan tag, often just the start
short_name(id, out[], len)
{
	base_name(id, out, len)
	if (strlen(out) > 8 && random_num(1, 100) <= 40)
		out[random_num(4, 6)] = 0
}

// The name without its clan tag
base_name(id, out[], len)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	trim(name)
	copy(out, len, name)
	// "-=[CoC]=- Squishy" / "[xDS]Bob" -> the name after the tag
	new tag[16]
	clan_of(id, tag, charsmax(tag))
	if (tag[0])
	{
		new at = contain(name, tag) + strlen(tag)
		while (name[at] && !isalnum(name[at]))
			at++
		if (name[at])
			copy(out, len, name[at])
	}
	// "GG | Phantom" / "XyZ|Name" -> Phantom / Name
	new bar = contain(out, "|")
	if (bar > 0 && bar <= 7 && out[bar + 1])
	{
		copy(out, len, out[bar + 1])
		trim(out)
	}
	if (!out[0])
		copy(out, len, name)
}

// Casual typers: lowercase, no full stop, the odd swapped letters
style_text(id, text[])
{
	new p = PERSONAS[g_Persona[id]]
	// a line already in capitals is shouted on purpose
	new bool:shout = true
	for (new i = 0; text[i]; i++)
		if (text[i] >= 'a' && text[i] <= 'z')
			shout = false
	if (!shout && contain("njcstkxhzpbdumAHLOS", fmt("%c", p)) != -1)
	{
		if (random_num(1, 100) <= 80)
			strtolower(text)
		new l = strlen(text)
		if (l > 1 && text[l - 1] == '.')
			text[l - 1] = 0
	}
	if (random_num(1, 100) <= g_Typos[id] + ((p == 'n' || p == 'k') ? 8 : 0))
	{
		new l = strlen(text)
		if (l > 4)
		{
			new i = random_num(1, l - 3)
			if (text[i] != ' ' && text[i + 1] != ' ')
			{
				new c = text[i]
				text[i] = text[i + 1]
				text[i + 1] = c
			}
		}
	}
}

// The same line never comes out quite the same: a stretched word, an ending each
// personality likes, very rarely shouting. All of it scales with gm_personality.
decorate(id, text[], len)
{
	new p = PERSONAS[g_Persona[id]]
	new Float:pm = personality()
	new l = strlen(text)
	if (!l || text[0] == '/')
		return
	// stretch the last vowel of a short word: nice -> niiice, no -> nooo
	if (l <= 8 && (p == 'j' || p == 'f' || p == 'n' || p == 's') && random_float(0.0, 100.0) <= 5.0 * pm)
	{
		for (new i = l - 1; i >= 0; i--)
		{
			if (contain("aeiouAEIOU", fmt("%c", text[i])) != -1)
			{
				new out[128], k
				for (new j = 0; j < l && k < charsmax(out) - 3; j++)
				{
					out[k++] = text[j]
					if (j == i)
					{
						out[k++] = text[j]
						out[k++] = text[j]
					}
				}
				out[k] = 0
				copy(text, len, out)
				break
			}
		}
	}
	// its own laugh and text-speak
	if (g_Laugh[id] > 2)
	{
		replace_word(text, len, "lol", LAUGHS[g_Laugh[id]])
		replace_word(text, len, "haha", LAUGHS[g_Laugh[id]])
	}
	if (g_TextSpeak[id])
	{
		static const FROM[][] = { "you're", "your", "you", "thanks", "please", "though", "probably", "because", "okay", "really", "people", "tonight" }
		static const TO[][] = { "ur", "ur", "u", "thx", "pls", "tho", "prob", "cuz", "k", "rly", "ppl", "tn" }
		for (new w = 0; w < sizeof FROM; w++)
			replace_word(text, len, FROM[w], TO[w])
	}
	// already ends in a laugh, an emoticon or a question: nothing more on the end
	l = strlen(text)
	new last = l ? text[l - 1] : 0
	new bool:ended = last == '?' || last == ')' || last == '(' || last == 'D' || last == 'P' || last == '3' || last == '>'
		|| (l >= 3 && (containi(text[l - 3], "lol") != -1 || containi(text[l - 3], "kek") != -1 || containi(text[l - 2], "xd") != -1 || containi(text[l - 3], "mao") != -1 || containi(text[l - 3], "aha") != -1 || containi(text[l - 3], "ehe") != -1))
	if (!ended)
	{
		// the word it keeps using
		if (WORDS[g_Word[id]][0] && strlen(text) < 30 && random_float(0.0, 100.0) <= 7.0 * pm)
			add(text, len, fmt(" %s", WORDS[g_Word[id]]))
		// its emoticon, or an ending its personality likes (with its own laugh)
		else if (EMOTES[g_Emote[id]][0] && random_float(0.0, 100.0) <= 6.0 * pm)
			add(text, len, fmt(" %s", EMOTES[g_Emote[id]]))
		else
		{
			new laugh[12]
			copy(laugh, charsmax(laugh), LAUGHS[g_Laugh[id]])
			new Float:r = random_float(0.0, 100.0) / floatmax(pm, 0.01)
			switch (p)
			{
				case 'f': if (r <= 8.0) add(text, len, random_num(0, 1) ? " :)" : " :D")
				case 'j': if (r <= 10.0) add(text, len, random_num(0, 1) ? fmt(" %s", laugh) : " :P")
				case 'n': if (r <= 8.0) add(text, len, "?")
				case 's': if (r <= 8.0) add(text, len, random_num(0, 1) ? "..." : " -_-")
				case 'c': if (r <= 6.0) add(text, len, fmt(" %s", laugh))
				case 'o': if (r <= 8.0) add(text, len, ".")
				case 'k': if (r <= 10.0) add(text, len, random_num(0, 1) ? "!" : fmt(" %s", laugh))
			}
		}
	}
	// shouting is rare: only the loud ones, now and then, and short lines
	if (g_Shouty[id] > 1 && (p == 'j' || p == 's' || p == 'k' || p == 'h') && random_float(0.0, 100.0) <= 1.5 * pm && strlen(text) <= 20)
		strtoupper(text)
}

// `from` as a whole word (any case) replaced by `to`
replace_word(text[], len, const from[], const to[])
{
	new fl = strlen(from), at, start = 0
	while ((at = containi(text[start], from)) != -1)
	{
		new s = start + at
		new before = s > 0 ? text[s - 1] : ' ', after = text[s + fl]
		if (!isalnum(before) && !isalnum(after) && before != 39 && after != 39)
		{
			new rest[128]
			copy(rest, charsmax(rest), text[s + fl])
			text[s] = 0
			add(text, len, to)
			add(text, len, rest)
			start = s + strlen(to)
		}
		else
			start = s + 1
		if (start >= strlen(text))
			break
	}
}

// A random bot other than `not`, or 0
any_bot(not)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != not && is_user_connected(i) && is_user_bot(i))
			list[n++] = i
	return n ? list[random(n)] : 0
}

any_bot_on(team, not)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != not && is_user_connected(i) && is_user_bot(i) && get_user_team(i) == team)
			list[n++] = i
	return n ? list[random(n)] : 0
}

// ---- what happens ---------------------------------------------------------------------

any_dead_bot_on(team)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i) && !is_user_alive(i) && get_user_team(i) == team)
			list[n++] = i
	return n ? list[random(n)] : 0
}

// One bot left on its side against three or more
last_alive_check(team)
{
	new alive, last, enemies
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_alive(i))
			continue
		new t = get_user_team(i)
		if (t == team)
		{
			alive++
			last = i
		}
		else if (t == 1 || t == 2)
			enemies++
	}
	if (alive == 1 && enemies >= 3 && is_user_bot(last))
		want(last, "last_alive", 0, 0.45, -1.0, 0, random_num(0, 1) == 1)
}

// Modes without rounds (Gun Game, Deathmatch) have no freeze time, when most chat happens:
// every so often someone says something anyway
public task_idle_chatter()
{
	set_task(random_float(25.0, 50.0), "task_idle_chatter")
	if (get_gametime() - g_LastRound < 150.0)
		return
	new h
	time(h)
	want(any_bot(0), (h >= 0 && h < 5 && random_num(0, 1)) ? "late_night" : "idle", 0, 0.35, random_float(1.0, 4.0))
}

public ev_newround()
{
	g_LastRound = get_gametime()
	g_Round++
	arrayset(g_RoundKills, 0, sizeof g_RoundKills)
	arrayset(g_Streaked, false, sizeof g_Streaked)
	g_BombPlanted = g_BombDropped = false
	g_BombPlace[0] = g_SeenPlace[1][0] = g_SeenPlace[2][0] = 0
	// freeze time is when people chat most
	if (random_num(1, 100) <= 22)
	{
		new h
		time(h)
		want(any_bot(0), (h >= 0 && h < 5 && random_num(0, 1)) ? "late_night" : "idle", 0, 0.6, random_float(2.0, 9.0))
	}
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2), hs = read_data(3)
	new weapon[24]
	read_data(4, weapon, charsmax(weapon))
	if (victim < 1 || victim > MaxClients)
		return
	new bool:tk = killer >= 1 && killer <= MaxClients && killer != victim && get_user_team(killer) == get_user_team(victim)

	if (killer < 1 || killer > MaxClients || killer == victim)
	{
		g_DeathRow[victim]++
		want(victim, "death_self", 0, 0.25)
		return
	}
	if (tk)
	{
		want(victim, "teamkilled", killer, 0.8)
		want(killer, "teamkill", victim, 0.7)
		return
	}

	// who was involved, for when a person says something about it
	new Float:now = get_gametime()
	if (is_user_bot(killer))
	{
		g_LastBotKiller = killer
		g_LastBotKillAt = now
	}
	else
	{
		g_VictimOf[killer] = victim
		g_VictimOfAt[killer] = now
	}
	if (!is_user_bot(victim))
	{
		g_KillerOf[victim] = killer
		g_KillerOfAt[victim] = now
		// how they died, for "how?" and "where is he"
		g_DiedAt[victim] = now
		g_KillHs[victim] = hs != 0
		copy(g_KillerPlace[victim], charsmax(g_KillerPlace[]), g_Place[killer])
		casual_weapon(weapon, g_KillWeapon[victim], charsmax(g_KillWeapon[]))
	}
	// the victim's team now knows where one of them is
	new vt = get_user_team(victim)
	if ((vt == 1 || vt == 2) && g_Place[killer][0])
	{
		copy(g_SeenPlace[vt], charsmax(g_SeenPlace[]), g_Place[killer])
		g_SeenAt[vt] = now
	}

	g_RoundKills[killer]++
	// rivals: someone who keeps killing a bot this map, and the bot finally getting them
	new was = g_KilledBy[victim][killer]
	g_KilledBy[killer][victim]++
	if (is_user_bot(victim) && g_KilledBy[killer][victim] == 4)
		want(victim, "nemesis", killer, 0.6, -1.0, 1)
	else if (is_user_bot(killer) && was >= 4 && random_num(1, 100) <= 70)
		want(killer, "nemesis_won", victim, 0.8, -1.0, 1)
	g_DeathRow[killer] = 0
	g_Raged[killer] = false
	if (++g_DeathRow[victim] >= 3 && mood(victim))
		return
	if (g_RoundKills[killer] >= 3 && !g_Streaked[killer])
	{
		g_Streaked[killer] = true
		want(killer, "streak", victim, 0.35)
		want(any_bot_on(get_user_team(victim), victim), "enemy_streak", killer, 0.35)
	}

	if (g_RoundKills[killer] == 5)
		want(killer, "ace", 0, 0.8, -1.0, 1)
	// scoped rifles without the scope (the line's theme keeps it to one now and then)
	if (is_user_bot(killer) && (equal(weapon, "awp") || equal(weapon, "scout")) && cs_get_user_zoom(killer) == CS_SET_NO_ZOOM)
		want(killer, "noscope", victim, 0.3)
	// someone dead and watching says something about a teammate's kill
	new watcher = any_dead_bot_on(get_user_team(killer))
	if (watcher && random_num(1, 100) <= 4)
		want(watcher, "spectating", killer, 0.6, -1.0, 0, true)
	last_alive_check(get_user_team(victim))

	new bool:knife = bool:equal(weapon, "knife"), bool:nade = bool:equal(weapon, "grenade")
	if (knife)
	{
		want(killer, "kill_knife", victim, 0.35)
		want(victim, "death_knife", killer, 0.45)
	}
	else if (nade)
	{
		want(killer, "kill_nade", victim, 0.2)
		want(victim, "death_nade", killer, 0.3)
	}
	else if (hs)
	{
		want(killer, "kill_headshot", victim, 0.15)
		want(victim, "death_headshot", killer, 0.18)
	}
	else
	{
		want(killer, equal(weapon, "awp") ? "kill_awp" : "kill", victim, 0.06)
		want(victim, "death", killer, 0.1)
	}
	// a clanmate on the killer's team cheers them on; one on the victim's side is out for revenge
	new mate = clanmate_bot(killer, true)
	if (mate)
		want(mate, "clan", killer, 0.18, -1.0, 0, true)
	new avenger = clanmate_bot(victim, true)
	if (avenger && random_num(1, 100) <= 50)
		want(avenger, "clan_avenge", killer, 0.25, -1.0, 0, true)
	// a teammate of the killer says something about it; about a person's kill, now and
	// then with the gun or the kind of kill in it ("deag ns", "clean hs")
	if (!is_user_bot(killer))
	{
		new mate = any_bot_on(get_user_team(killer), killer)
		casual_weapon(weapon, g_CtxWeapon, charsmax(g_CtxWeapon))
		g_CtxHs = hs != 0
		want(mate, knife ? "human_kill_knife" : nade ? "human_kill_nade" : hs ? "human_kill_hs" : "human_kill", killer,
			(hs || knife || nade) ? 0.2 : 0.07, -1.0, 0, random_num(1, 100) <= 40)
		g_CtxWeapon[0] = 0
		g_CtxHs = false
	}
	else if (hs || knife)
		want(any_bot_on(get_user_team(killer), killer), "nice_shot", killer, 0.1, -1.0, 0, true)
}

// A gun's name as people say it ("deag" for deagle); "" for ones nobody would name
casual_weapon(const weapon[], out[], len)
{
	static const FROM[][] = { "deagle", "ak47", "m4a1", "awp", "scout", "usp", "glock18", "mp5navy", "famas", "galil",
		"p90", "m3", "xm1014", "aug", "sg552", "m249", "elite", "fiveseven", "ump45", "mac10", "tmp", "p228", "knife", "grenade", "sg550", "g3sg1" }
	static const TO[][] = { "deag", "ak", "m4", "awp", "scout", "usp", "glock", "mp5", "famas", "galil",
		"p90", "pump", "auto shotty", "aug", "krieg", "para", "dualies", "five seven", "ump", "mac10", "tmp", "p228", "knife", "nade", "auto", "auto" }
	out[0] = 0
	for (new i = 0; i < sizeof FROM; i++)
		if (equal(weapon, FROM[i]))
		{
			copy(out, len, TO[i])
			return
		}
}

// A place name from the game ("#BombsiteB", "LongHall", "CTSpawn") as people say it
// ("b", "long", "ct spawn")
casual_place(const raw[], out[], len)
{
	new name[32]
	copy(name, charsmax(name), raw[raw[0] == '#' ? 1 : 0])
	out[0] = 0
	if (!name[0])
		return
	static const FROM[][] = { "BombsiteA", "BombsiteB", "BombsiteC", "Middle", "MiddleHall", "CTSpawn", "TSpawn", "LongHall", "ShortHall", "Hostages", "HostageRescueZone" }
	static const TO[][] = { "a", "b", "c", "mid", "mid", "ct spawn", "t spawn", "long", "short", "hostages", "rescue" }
	for (new i = 0; i < sizeof FROM; i++)
		if (equali(name, FROM[i]))
		{
			copy(out, len, TO[i])
			return
		}
	// "UpperTunnel" -> "upper tunnel"
	new k
	for (new i = 0; name[i] && k < len - 1; i++)
	{
		new c = name[i]
		if (i > 0 && c >= 'A' && c <= 'Z' && !(name[i - 1] >= 'A' && name[i - 1] <= 'Z') && k < len - 2)
			out[k++] = ' '
		else if (c == '_')
			c = ' '
		out[k++] = tolower(c)
	}
	out[k] = 0
}

// ---- game state -------------------------------------------------------------------------

// Where someone is: the place names on the radar (only maps whose nav file names its places)
public ev_location()
{
	new id = read_data(1), raw[32]
	read_data(2, raw, charsmax(raw))
	if (id >= 1 && id <= MaxClients && raw[0] && raw[1])
		casual_place(raw, g_Place[id], charsmax(g_Place[]))
}

public ev_teamscore()
{
	new team[16]
	read_data(1, team, charsmax(team))
	new score = read_data(2)
	if (equal(team, "TERRORIST"))
		g_Score[1] = score
	else if (equal(team, "CT"))
		g_Score[2] = score
}

public ev_damage(victim)
{
	new Float:now = get_gametime()
	if (victim >= 1 && victim <= MaxClients)
	{
		g_FightAt[victim] = now
		new attacker = get_user_attacker(victim)
		if (attacker >= 1 && attacker <= MaxClients)
			g_FightAt[attacker] = now
	}
}

public ev_bomb_dropped()
{
	new id = get_loguser_index()
	g_BombDropped = true
	g_BombPlace[0] = 0
	if (id)
		copy(g_BombPlace, charsmax(g_BombPlace), g_Place[id])
}

public ev_bomb_got()
{
	g_BombDropped = false
}

// Dying again and again: a bot tilts, then crashes out; teammates may cheer them up.
// True when something was said about it (the usual death line is skipped then).
bool:mood(victim)
{
	new row = g_DeathRow[victim], team = get_user_team(victim)
	// someone on the team cheers them up (people too)
	if (row == 5 || row == 8)
	{
		new mate = clanmate_bot(victim, true)
		want(mate ? mate : any_bot_on(team, victim), "support", victim, mate ? 0.45 : 0.2, -1.0, 0, true)
	}
	if (!is_user_bot(victim))
		return false
	new p = PERSONAS[g_Persona[victim]]
	if (row >= 6 && !g_Raged[victim] && (p == 's' || p == 'j' || p == 't' || p == 'O' || p == 'H' || random_num(1, 100) <= 25))
	{
		g_Raged[victim] = true
		crash_out(victim)
		return true
	}
	if (row == 4)
	{
		want(victim, "tilt", 0, 0.35)
		return true
	}
	return false
}

// A few angry lines in quick succession; a salty one may leave for good
crash_out(id)
{
	if (g_Career || !get_pcvar_num(g_pLevel) || random_float(0.0, 1.0) > 0.8 * level_mult() * personality())
		return
	new text[128], Float:at = random_float(1.0, 2.0)
	new n = random_num(1, 3)
	for (new i = 0; i < n; i++)
	{
		if (!pick_line(id, "rage", text, charsmax(text)))
			break
		queue(id, 0, text, MAX_CHAIN, false, at)
		at += typing_time(text, 0.3, 1.0)
	}
	// the hot-headed quit a third of the time, anyone now and then
	new p = PERSONAS[g_Persona[id]]
	if (random_num(1, 100) <= (contain("stOH", fmt("%c", p)) != -1 ? 25 : 5) && pick_line(id, "ragequit", text, charsmax(text)))
	{
		queue(id, 0, text, MAX_CHAIN, false, at + 0.8)
		set_task(at + 2.5, "task_ragequit", get_user_userid(id))
	}
	g_NextAny = get_gametime() + at + global_gap()
}

public task_ragequit(userid)
{
	new id = find_player("k", userid)
	if (id && is_user_bot(id))
		server_cmd("kick #%d", userid)
}

public ev_twin()
{
	round_end(1)
}

public ev_ctwin()
{
	round_end(2)
}

round_end(winner)
{
	// the round's over: threads with people end here, and answers still being written
	arrayset(g_ThreadBot, 0, sizeof g_ThreadBot)
	g_Round++
	new loser = 3 - winner
	new bool:comeback = g_LoseRun[winner] >= 4
	g_WinRun[winner]++
	g_LoseRun[winner] = 0
	g_LoseRun[loser]++
	g_WinRun[loser] = 0
	if (comeback)
		want(any_bot_on(winner, 0), "comeback", 0, 0.6, -1.0, 0, random_num(0, 1) == 1)
	else if (g_WinRun[winner] == 4 || g_WinRun[winner] == 7 || g_WinRun[winner] == 10)
		want(any_bot_on(winner, 0), "winning_streak_team", 0, 0.6, -1.0, 0, true)
	if (g_LoseRun[loser] == 4 || g_LoseRun[loser] == 7 || g_LoseRun[loser] == 10)
		want(any_bot_on(loser, 0), "losing_streak_team", 0, 0.6, -1.0, 0, true)
	// survived with almost nothing left
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && is_user_bot(i) && get_user_health(i) <= 15 && random_num(1, 100) <= 40)
		{
			want(i, "low_hp", 0, 0.6)
			break
		}
	// the last one standing on the winning side, if it did a lot of the work
	new alive, last
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == winner)
		{
			alive++
			last = i
		}
	if (alive == 1 && g_RoundKills[last] >= 2)
	{
		want(last, "clutch", 0, 0.5)
		want(any_bot_on(winner, last), "teammate_clutch", last, 0.5)
		return
	}
	want(any_bot_on(winner, 0), "round_win", 0, 0.12, -1.0, 0, random_num(0, 1) == 1)
	want(any_bot_on(3 - winner, 0), "round_lose", 0, 0.12, -1.0, 0, random_num(0, 1) == 1)
}

public ev_planted()
{
	new id = get_loguser_index()
	g_BombPlanted = true
	g_BombDropped = false
	if (id)
		copy(g_BombPlace, charsmax(g_BombPlace), g_Place[id])
	// the terrorists say so in team chat
	want(any_bot_on(1, 0), "bomb_planted", 0, 0.2, -1.0, 0, true)
}

public ev_defused()
{
	new id = get_loguser_index()
	// defused with terrorists still alive: a ninja defuse
	new ts
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == 1)
			ts++
	want(id, ts ? "defuse_ninja" : "defused", 0, ts ? 0.7 : 0.4)
	want(any_bot_on(1, 0), "they_defused", id, 0.3)
}

public ev_intermission()
{
	if (g_Career || !get_pcvar_num(g_pLevel))
		return
	// the end of the map: a few gg's, not every bot (some gg's even at personality 0)
	new said, most = 2 + floatround(personality()), Float:pct = 25.0 + 10.0 * personality()
	new order[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i))
			order[n++] = i
	for (new k = 0; k < n && said < most; k++)
	{
		// in random order, so it isn't always the lowest slots
		new j = random_num(k, n - 1), i = order[j]
		order[j] = order[k]
		if (random_float(0.0, 100.0) > pct)
			continue
		new text[128]
		if (pick_line(i, "map_end", text, charsmax(text), true))
		{
			queue(i, 0, text, MAX_CHAIN, false, random_float(0.8, 3.0) + said * random_float(0.8, 2.0) + typing_time(text, 0.0, 0.0))
			said++
		}
	}
}

get_loguser_index()
{
	new user[80], name[32]
	read_logargv(0, user, charsmax(user))
	parse_loguser(user, name, charsmax(name))
	return get_user_index(name)
}

// ---- what people say ------------------------------------------------------------------

public cmd_say(id)
{
	heard(id, false)
	return PLUGIN_CONTINUE
}

public cmd_say_team(id)
{
	heard(id, true)
	return PLUGIN_CONTINUE
}

// Words people type, and the section the answer comes from (first match wins, so
// "gg ez" is an ez, "nice try" isn't a "nice", "my bad" isn't a "bad" and "how are you"
// isn't a "how")
new const HEARD[][][] =
{
	{ "how are you", "r_howru" }, { "how r u", "r_howru" }, { "how are u", "r_howru" }, { "how r you", "r_howru" },
	{ "hows it going", "r_howru" }, { "how is it going", "r_howru" }, { "how u doing", "r_howru" },
	{ "how you doing", "r_howru" }, { "hru", "r_howru" }, { "whats up with u", "r_howru" },
	{ "where", "r_where" }, { "wheres", "r_where" }, { "where r they", "r_where" }, { "where is he", "r_where" },
	{ "score", "r_score" }, { "whats the score", "r_score" },
	{ "timeleft", "r_timeleft" }, { "time left", "r_timeleft" }, { "how long left", "r_timeleft" },
	{ "how much time", "r_timeleft" }, { "how long is this map", "r_timeleft" },
	{ "nextmap", "r_nextmap" }, { "next map", "r_nextmap" }, { "whats next", "r_nextmap" }, { "what map is next", "r_nextmap" },
	{ "what map", "r_whatmap" }, { "which map", "r_whatmap" }, { "map name", "r_whatmap" }, { "what is this map", "r_whatmap" },
	{ "hack", "r_hack" }, { "hacks", "r_hack" }, { "hacker", "r_hack" }, { "hacking", "r_hack" }, { "haxor", "r_hack" },
	{ "aimbot", "r_hack" }, { "wallhack", "r_hack" }, { "wh", "r_hack" }, { "cheat", "r_hack" }, { "cheater", "r_hack" },
	{ "cheating", "r_hack" }, { "sus", "r_hack" },
	{ "bot", "r_bot" }, { "bots", "r_bot" }, { "botz", "r_bot" }, { "a bot", "r_bot" },
	{ "nt", "r_nt" }, { "nice try", "r_nt" },
	{ "ns", "r_nice_from_human" }, { "nice shot", "r_nice_from_human" }, { "nice", "r_nice_from_human" },
	{ "good shot", "r_nice_from_human" }, { "wp", "r_nice_from_human" }, { "gj", "r_nice_from_human" },
	{ "well played", "r_nice_from_human" }, { "nice one", "r_nice_from_human" },
	{ "ez", "r_ez" }, { "ezpz", "r_ez" }, { "easy", "r_ez" }, { "too easy", "r_ez" },
	{ "sorry", "r_sorry" }, { "sry", "r_sorry" }, { "my bad", "r_sorry" }, { "mb", "r_sorry" }, { "oops", "r_sorry" },
	{ "noob", "r_noob" }, { "nub", "r_noob" }, { "n00b", "r_noob" }, { "newb", "r_noob" }, { "noobs", "r_noob" },
	{ "trash", "r_noob" }, { "suck", "r_noob" }, { "sucks", "r_noob" }, { "bad", "r_noob" }, { "idiot", "r_noob" },
	{ "loser", "r_noob" }, { "l2p", "r_noob" }, { "owned", "r_noob" }, { "pwned", "r_noob" }, { "stfu", "r_noob" },
	{ "shut up", "r_noob" }, { "dumb", "r_noob" }, { "garbage", "r_noob" }, { "lame", "r_noob" },
	{ "camp", "r_camping" }, { "camper", "r_camping" }, { "camping", "r_camping" }, { "campers", "r_camping" },
	{ "gg", "r_gg" }, { "ggs", "r_gg" }, { "ggwp", "r_gg" }, { "good game", "r_gg" },
	{ "gl", "r_gl" }, { "hf", "r_gl" }, { "glhf", "r_gl" }, { "good luck", "r_gl" },
	{ "hi", "r_hi" }, { "hey", "r_hi" }, { "yo", "r_hi" }, { "sup", "r_hi" }, { "hello", "r_hi" }, { "hiya", "r_hi" },
	{ "heya", "r_hi" }, { "hola", "r_hi" }, { "howdy", "r_hi" }, { "evening", "r_hi" }, { "whats up", "r_hi" },
	{ "wassup", "r_hi" }, { "hai", "r_hi" }, { "ello", "r_hi" },
	{ "lag", "r_lag" }, { "laggy", "r_lag" }, { "lagging", "r_lag" }, { "ping", "r_lag" }, { "hitreg", "r_lag" },
	{ "reg", "r_lag" }, { "choke", "r_lag" }, { "loss", "r_lag" },
	{ "wait", "r_wait" }, { "hold", "r_wait" }, { "stay", "r_wait" }, { "dont push", "r_wait" }, { "slow", "r_wait" },
	{ "rush", "r_plan" }, { "rush b", "r_plan" }, { "go a", "r_plan" }, { "go b", "r_plan" }, { "come a", "r_plan" },
	{ "come b", "r_plan" }, { "follow me", "r_plan" }, { "follow", "r_plan" }, { "cover me", "r_plan" }, { "cover", "r_plan" },
	{ "push", "r_plan" }, { "stack", "r_plan" }, { "help", "r_plan" }, { "eco", "r_plan" }, { "save", "r_plan" },
	{ "lets go", "r_plan" }, { "go go", "r_plan" }, { "with me", "r_plan" },
	{ "ty", "r_np" }, { "thx", "r_np" }, { "thanks", "r_np" }, { "thank", "r_np" }, { "tyvm", "r_np" },
	{ "im back", "r_wb" },
	{ "how", "r_how" },
	{ "wtf", "r_wtf" }, { "omg", "r_wtf" },
	{ "lol", "r_lol" }, { "haha", "r_lol" }, { "lmao", "r_lol" }, { "rofl", "r_lol" }, { "xd", "r_lol" }, { "hehe", "r_lol" },
	{ "?", "r_question" }
}

// How bots take what people say, per answer section: the chance one answers, who it
// should be and whether a second one may chime in
enum
{
	WHO_ANY,        // anyone, nearby and alive first
	WHO_INVOLVED,   // the bot that killed them or that they killed, just now
	WHO_KILLER,     // the bot that got the kill they're talking about
	WHO_TEAM,       // a teammate, alive if possible
	WHO_WATCHER     // a teammate, dead (watching) if possible
}

reply_style(const section[], &Float:chance, &who, &Float:second)
{
	who = WHO_ANY
	second = 0.0
	if (equal(section, "r_where"))          { chance = 0.5;  who = WHO_TEAM; }
	else if (equal(section, "r_bomb"))      { chance = 0.5;  who = WHO_TEAM; }
	else if (equal(section, "r_score"))     { chance = 0.45; who = WHO_TEAM; }
	else if (equal(section, "r_timeleft"))  { chance = 0.45; }
	else if (equal(section, "r_nextmap"))   { chance = 0.45; }
	else if (equal(section, "r_whatmap"))   { chance = 0.35; }
	else if (equal(section, "r_how"))       { chance = 0.4;  who = WHO_WATCHER; }
	else if (equal(section, "r_howru"))     { chance = 0.5; }
	else if (equal(section, "r_hack"))      { chance = 0.4;  who = WHO_INVOLVED; }
	else if (equal(section, "r_bot"))       { chance = 0.45; second = 0.15; }
	else if (equal(section, "r_nt"))        { chance = 0.2;  who = WHO_TEAM; }
	else if (equal(section, "r_nice_from_human")) { chance = 0.55; who = WHO_KILLER; }
	else if (equal(section, "r_ez"))        { chance = 0.3;  who = WHO_INVOLVED; }
	else if (equal(section, "r_noob"))      { chance = 0.35; who = WHO_INVOLVED; }
	else if (equal(section, "r_camping"))   { chance = 0.3;  who = WHO_INVOLVED; }
	else if (equal(section, "r_gg"))        { chance = 0.5;  second = 0.3; }
	else if (equal(section, "r_gl"))        { chance = 0.35; second = 0.15; }
	else if (equal(section, "r_hi"))        { chance = 0.45; second = 0.2; }
	else if (equal(section, "r_lag"))       { chance = 0.3; }
	else if (equal(section, "r_wait"))      { chance = 0.35; who = WHO_TEAM; }
	else if (equal(section, "r_plan"))      { chance = 0.45; who = WHO_TEAM; }
	else if (equal(section, "r_sorry"))     { chance = 0.35; who = WHO_INVOLVED; }
	else if (equal(section, "r_np"))        { chance = 0.25; }
	else if (equal(section, "r_wb"))        { chance = 0.35; }
	else if (equal(section, "r_wtf"))       { chance = 0.12; }
	else if (equal(section, "r_lol"))       { chance = 0.06; }
	else if (equal(section, "r_question"))  { chance = 0.08; }
	else if (equal(section, "r_named"))     { chance = 0.85; }
	else                                    { chance = 0.015; }
}

// The section for what was said, or "" (msg in lowercase)
classify(const msg[], section[], len)
{
	section[0] = 0
	if (equal(msg, "o/") || equal(msg, "\o"))
	{
		copy(section, len, "r_hi")
		return
	}
	if (equal(msg, "re") || equal(msg, "back") || equal(msg, "back."))
	{
		copy(section, len, "r_wb")
		return
	}
	// "wheres the bomb", "bomb?", "who has c4"
	if ((word_in(msg, "bomb") || word_in(msg, "c4")) && (contain(msg, "?") != -1 || word_in(msg, "where") || word_in(msg, "wheres") || word_in(msg, "who")))
	{
		copy(section, len, "r_bomb")
		return
	}
	for (new w = 0; w < sizeof HEARD; w++)
		if (word_in(msg, HEARD[w][0]))
		{
			copy(section, len, HEARD[w][1])
			return
		}
}

// Lines that go nowhere on their own: in a thread they carry on what was being said
bool:weak_section(const section[])
{
	return !section[0] || equal(section, "r_lol") || equal(section, "r_ack") || equal(section, "r_question")
		|| equal(section, "r_named") || equal(section, "r_wtf")
}

// What people type, made easier to match: lowercase, "heyyy" -> "hey", "ggggg" -> "gg"
normalize(msg[])
{
	strtolower(msg)
	new out[128], k
	for (new i = 0; msg[i] && k < charsmax(out); i++)
	{
		new c = msg[i]
		new run = 1
		while (msg[i + run] == c)
			run++
		if (run >= 3)
		{
			out[k++] = c
			if (c == 'g' && k < charsmax(out))
				out[k++] = c
			i += run - 1
			continue
		}
		out[k++] = c
	}
	out[k] = 0
	copy(msg, 127, out)
}

// The first word of a bot's name without its clan tag, lowercase ("[XyZ] Tariq K." -> "tariq")
first_word(id, out[], len)
{
	new name[32]
	base_name(id, name, charsmax(name))
	strtolower(name)
	new i, k
	while (name[i] && !isalnum(name[i]))
		i++
	while (name[i] && isalnum(name[i]) && k < len)
		out[k++] = name[i++]
	out[k] = 0
}

// `part` at the start of a word in msg ("tari" in "nice tariq")
bool:word_start_in(const msg[], const part[])
{
	new from = 0, at
	while ((at = contain(msg[from], part)) != -1)
	{
		new s = from + at
		if (s == 0 || !isalnum(msg[s - 1]))
			return true
		from = s + 1
	}
	return false
}

// The bot someone is talking to by name, or 0
addressed(const msg[])
{
	new word[32], sec[24], hit[5]
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i))
			continue
		first_word(i, word, charsmax(word))
		if (strlen(word) < 3)
			continue
		// a bot called "Lag" or "Ghost": "lag" in chat is about lag, not about them
		classify(word, sec, charsmax(sec))
		if (sec[0])
			continue
		if (word_in(msg, word))
			return i
		// people shorten names: "tari" for "tariq", "dyla" for "dylan52"
		if (strlen(word) >= 5)
		{
			copy(hit, 4, word)
			if (word_start_in(msg, hit))
				return i
		}
	}
	return 0
}

// Something the bot said, quoted back ("lol no" -> "lol no?", or a longer word from it)
bool:quoted(const msg[], const line[])
{
	new low[128]
	copy(low, charsmax(low), line)
	strtolower(low)
	if (strlen(low) >= 4 && contain(msg, low) != -1)
		return true
	new word[32], rest[128]
	copy(rest, charsmax(rest), low)
	while (rest[0])
	{
		strtok2(rest, word, charsmax(word), rest, charsmax(rest), ' ', TRIM_FULL)
		if (strlen(word) >= 5 && word_in(msg, word))
			return true
	}
	return false
}

// The bot that should answer: `prefer` if it can, else one of the rest, likelier when
// it's alive, nearby, on their team, and not in the middle of a fight
responder(human, prefer, who, bool:team)
{
	new t = get_user_team(human)
	new bool:teamonly = team || who == WHO_TEAM || who == WHO_WATCHER
	if (prefer && is_user_connected(prefer) && is_user_bot(prefer) && (!teamonly || get_user_team(prefer) == t))
		return prefer
	new list[32], Float:weight[32], n, Float:total
	new org[3], o2[3]
	new bool:alive = bool:is_user_alive(human)
	if (alive)
		get_user_origin(human, org)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i))
			continue
		new bool:mate = get_user_team(i) == t
		if (teamonly && !mate)
			continue
		new Float:w = 1.0
		if (who == WHO_WATCHER)
			w += is_user_alive(i) ? 0.0 : 3.0
		else if (is_user_alive(i))
		{
			w += who == WHO_TEAM ? 3.0 : 1.5
			if (alive)
			{
				get_user_origin(i, o2)
				if (get_distance(org, o2) < 1000)
					w += 2.0
			}
		}
		if (mate)
			w += 1.0
		if (in_combat(i))
			w *= 0.15
		list[n] = i
		weight[n++] = w
		total += w
	}
	if (!n)
		return 0
	new Float:r = random_float(0.0, total)
	for (new k = 0; k < n; k++)
	{
		r -= weight[k]
		if (r <= 0.0)
			return list[k]
	}
	return list[n - 1]
}

// The game state an answer needs, put where pick_line finds it; the section may change
// to the one for what's known ("r_where" -> "r_where_unknown" when nobody saw anything)
resolve_state(section[], len, human, bot)
{
	new Float:now = get_gametime()
	new t = get_user_team(human)
	if (equal(section, "r_where"))
	{
		if (now - g_DiedAt[human] < 30.0 && g_KillerPlace[human][0])
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_KillerPlace[human])
		else if ((t == 1 || t == 2) && now - g_SeenAt[t] < 25.0 && g_SeenPlace[t][0])
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_SeenPlace[t])
		if (!g_CtxPlace[0])
			copy(section, len, "r_where_unknown")
	}
	else if (equal(section, "r_bomb"))
	{
		new carrier
		if (!g_BombPlanted && t == 1)
			for (new i = 1; i <= MaxClients && !carrier; i++)
				if (is_user_alive(i) && get_user_team(i) == 1 && user_has_weapon(i, CSW_C4))
					carrier = i
		if (g_BombPlanted && g_BombPlace[0])
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_BombPlace)
			copy(section, len, "r_bomb_planted")
		}
		else if (carrier == human)
			copy(section, len, "r_bomb_you")
		else if (carrier)
		{
			if (carrier == bot)
				copy(section, len, "r_bomb_me")
			else
			{
				short_name(carrier, g_CtxCarrier, charsmax(g_CtxCarrier))
				copy(section, len, "r_bomb_carrier")
			}
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_Place[carrier])
		}
		else if (t == 1 && g_BombDropped)
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_BombPlace)
			copy(section, len, "r_bomb_dropped")
		}
		else
			copy(section, len, "r_bomb_unknown")
	}
	else if (equal(section, "r_score"))
	{
		new bt = get_user_team(bot)
		new us = (bt == 1 || bt == 2) ? g_Score[bt] : 0, them = (bt == 1 || bt == 2) ? g_Score[3 - bt] : 0
		copy(section, len, us > them ? "r_score_ahead" : us < them ? "r_score_behind" : "r_score_tied")
	}
	else if (equal(section, "r_how"))
	{
		// "how" about their own death, just now; otherwise it's just a question
		if (now - g_DiedAt[human] < 15.0)
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_KillerPlace[human])
			copy(g_CtxWeapon, charsmax(g_CtxWeapon), g_KillWeapon[human])
			g_CtxHs = g_KillHs[human]
		}
		else
			copy(section, len, "r_question")
	}
	else if (equal(section, "r_timeleft") && get_timeleft() <= 0)
		copy(section, len, "r_timeleft_none")
	else if (equal(section, "r_nextmap"))
	{
		new next[32]
		next_map(next, charsmax(next))
		if (!next[0])
			copy(section, len, "r_nextmap_none")
	}
}

clear_state()
{
	g_CtxPlace[0] = g_CtxWeapon[0] = g_CtxCarrier[0] = 0
	g_CtxHs = false
}

// One bot's answer to a person, typed after reading it, with the game state it needs
bool:answer(bot, const what[], human, bool:team, Float:extra, bool:quiet = false, bool:thread = false)
{
	// still typing an answer from the model
	if (!bot || g_AiState[bot])
		return false
	new section[32], text[128]
	copy(section, charsmax(section), what)
	resolve_state(section, charsmax(section), human, bot)
	g_CtxOther = human
	new bool:ok = pick_line(bot, section, text, charsmax(text), true)
	g_CtxOther = 0
	// with gm_ai_url set the model writes it (the written line is the fallback); a second
	// bot chiming in ("gg") keeps to the written lines
	if (ok && !quiet && ai_ask(bot, human, text, section, team, extra, thread))
	{
		clear_state()
		return true
	}
	clear_state()
	if (!ok)
		return false
	queue(bot, human, text, 1, team, extra + typing_time(text, 1.2, 3.0), section, quiet)
	return true
}

// A bot said something to a person: that's a thread they may carry on
thread_said(human, bot, const section[], const text[])
{
	if (g_ThreadBot[human] != bot)
	{
		g_ThreadBot[human] = bot
		g_ThreadTurns[human] = 1
		copy(g_ThreadTopic[human], charsmax(g_ThreadTopic[]), section)
	}
	else
	{
		g_ThreadTurns[human]++
		if (section[0] && !equal(section, "t_", 2))
			copy(g_ThreadTopic[human], charsmax(g_ThreadTopic[]), section)
	}
	g_ThreadAt[human] = get_gametime()
	copy(g_ThreadLine[human], charsmax(g_ThreadLine[]), text)
}

// The carry-on section for a thread about `topic`: t_bot for r_bot, t_score for
// r_score_ahead, t_human for human_kill_hs..., t_any when there's none
thread_section(const topic[], out[], len)
{
	new name[32]
	copy(name, charsmax(name), topic[equal(topic, "r_", 2) ? 2 : 0])
	new Array:list
	while (name[0])
	{
		formatex(out, len, "t_%s", name)
		if (TrieGetCell(g_Sections, out, list))
			return
		new cut = -1
		for (new i = 0; name[i]; i++)
			if (name[i] == '_')
				cut = i
		if (cut < 0)
			break
		name[cut] = 0
	}
	copy(out, len, "t_any")
}

// The person answers the bot that was just talking to them: it carries on, a couple of
// lines at most, and only it (true: this was a thread, nobody else answers)
bool:continue_thread(id, const msg[], const said[], to, bool:team)
{
	new bot = g_ThreadBot[id]
	if (!bot)
		return false
	new Float:now = get_gametime()
	if (!is_user_connected(bot) || !is_user_bot(bot) || g_ThreadTurns[id] >= 3 || in_combat(bot)
		|| (to && to != bot) || (team && get_user_team(bot) != get_user_team(id))
		|| (now - g_ThreadAt[id] >= 20.0 && to != bot && !quoted(msg, g_ThreadLine[id])))
	{
		g_ThreadBot[id] = 0
		return false
	}
	// something new ("where is he", "gg") gets its own answer, from this bot; anything
	// else ("sure", "lol", the same thing again) carries on what it was saying
	new section[32], topic[24]
	copy(topic, charsmax(topic), g_ThreadTopic[id])
	new bool:same = said[0] && equal(topic, said, strlen(said))
	if (weak_section(said) || same)
		thread_section(topic, section, charsmax(section))
	else
	{
		// "where is he" is for teammates, not the enemy who was talking
		new Float:c, who, Float:sec
		reply_style(said, c, who, sec)
		if ((who == WHO_TEAM || who == WHO_WATCHER) && get_user_team(bot) != get_user_team(id))
		{
			g_ThreadBot[id] = 0
			return false
		}
		copy(section, charsmax(section), said)
	}
	new Float:chance = g_ThreadTurns[id] <= 1 ? 0.8 : 0.55
	if (equal(section, "t_any"))
		chance *= 0.6
	chance *= 0.6 + 0.4 * personality()
	if (random_float(0.0, 1.0) > chance || !answer(bot, section, id, team, 0.0, false, true))
		g_ThreadBot[id] = 0
	return true
}

heard(id, bool:team)
{
	if (!is_user_connected(id) || is_user_bot(id) || !get_pcvar_num(g_pHumans) || g_Career || !get_pcvar_num(g_pLevel))
		return
	new msg[128]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	if (!msg[0] || msg[0] == '/' || msg[0] == '!')
		return
	// nobody else saw it (muted or filtered), so no bot answers it
	if (LibraryExists("gm_community", LibType_Library) && gm_chat_blocked(id, msg))
		return
	copy(g_HumanMsg, charsmax(g_HumanMsg), msg)
	hist_add(id, msg, team ? get_user_team(id) : 0)
	normalize(msg)
	// "timeleft", "nextmap": the server answers those itself
	new first[16]
	copyc(first, charsmax(first), msg, ' ')
	if (equal(first, "timeleft") || equal(first, "nextmap") || equal(first, "currentmap") || equal(first, "thetime"))
		return

	new to = addressed(msg)
	// a bot on the other side doesn't see team chat
	if (to && team && get_user_team(to) != get_user_team(id))
		to = 0
	new Float:now = get_gametime()
	new section[24]
	classify(msg, section, charsmax(section))
	// answering the bot that was just talking to them
	if (continue_thread(id, msg, section, to, team))
		return
	// someone typing a lot doesn't get an answer to every line
	if (!to && now < g_NextHeard)
		return
	if (!section[0])
		copy(section, charsmax(section), to ? "r_named" : "r_ack")

	new Float:chance, who, Float:second
	reply_style(section, chance, who, second)
	// who it's about: the bot that killed them or that they just killed, the one that got the kill
	new prefer = to
	if (!prefer && who == WHO_INVOLVED)
	{
		if (g_KillerOf[id] && now - g_KillerOfAt[id] < 15.0 && is_user_bot(g_KillerOf[id]))
			prefer = g_KillerOf[id]
		else if (g_VictimOf[id] && now - g_VictimOfAt[id] < 15.0 && is_user_bot(g_VictimOf[id]))
			prefer = g_VictimOf[id]
	}
	else if (!prefer && who == WHO_KILLER)
	{
		if (g_KillerOf[id] && now - g_KillerOfAt[id] < 10.0 && is_user_bot(g_KillerOf[id]))
			prefer = g_KillerOf[id]
		else if (g_LastBotKiller && now - g_LastBotKillAt < 6.0)
			prefer = g_LastBotKiller
		else
			chance *= 0.3          // "nice" about nothing in particular
	}
	// busy fighting: someone else answers
	if (prefer && prefer != to && in_combat(prefer))
		prefer = 0
	if (to)
		chance = floatmax(chance, 0.85)

	// the more they've answered lately, the less they answer now
	if (now - g_HeardRunAt > 45.0)
		g_HeardRun = 0
	chance /= 1.0 + (to ? 0.2 : 0.6) * float(g_HeardRun)
	// at personality 0 they still answer when spoken to, at 200 a bit more
	chance *= 0.6 + 0.4 * personality()
	if (random_float(0.0, 1.0) > floatmin(chance, 0.95))
		return

	new bot = responder(id, prefer, who, team)
	if (!answer(bot, section, id, team, 0.0))
		return
	g_HeardRun++
	g_HeardRunAt = now
	g_NextHeard = now + random_float(2.0, 4.0)
	// now and then a second one ("gg", "hi"), a little later; it doesn't start a thread
	if (second > 0.0 && random_float(0.0, 1.0) <= second * (0.5 + 0.5 * personality()))
	{
		new other = responder(id, 0, who, team)
		if (other && other != bot)
			answer(other, section, id, team, random_float(1.5, 4.0), true)
	}
}

// ---- smarter chat: a small local model writes answers to people -------------------------
// The app runs the model and sets gm_ai_url to "127.0.0.1:port" while it's ready ("" when
// not). Answers to people and thread lines go to it (POST /api/botchat/reply); a 200 with
// {"text": "..."} is what the bot types. Anything else, no answer within AI_TIMEOUT, or no
// sockets module: the written line picked as usual. Whether a bot answers at all is still
// decided here, so the model doesn't make bots chattier. Never blocks: a nonblocking socket
// polled every 0.1 s, one request per bot.

// What each personality is, for the model (same order as PERSONAS)
new const PERSONA_DESC[][] = { "friendly", "tryhard", "salty", "joker", "quiet", "new player", "chill", "old-timer",
	"kid", "english is not their first language", "hype", "zoomer", "streamer wannabe", "backseat gamer", "the team's caller",
	"half asleep", "blames themselves", "wholesome", "military roleplay", "meme lord", "anime fan", "brit",
	"conspiracy theorist", "dad gamer", "esports wannabe", "foodie", "grammar nerd", "sees hackers everywhere", "lurker",
	"music guy", "stats nerd", "overconfident", "pacifist", "movement nerd", "map tourist", "yapper", "it guy", "gym bro",
	"wants the admin", "trivia guy", "patient coach", "pirate", "villain roleplay", "philosopher", "extreme 2000s gamer",
	"clutch hero, plays bold when last alive", "jokes about being a bot", "cowboy", "detective", "leetspeak",
	"awper, always wants the awp", "rusher, first out of spawn", "nade spammer", "lagger, blames the ping",
	"map voter, always wants rtv", "plays weird guns like shotguns and the tmp", "camper, holds one spot",
	"knifer, loves knife kills", "ninja, sneaky and quiet", "clan recruiter", "objective guy, wants the bomb planted",
	"spray artist" }

ai_init()
{
	g_Sockets = bool:LibraryExists("sockets", LibType_Library)
}

// Something said in chat, for the history the model sees (team: 0 everyone, 1 or 2 that team)
hist_add(id, const text[], team)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	formatex(g_Hist[g_HistPos], charsmax(g_Hist[]), "%s: %s", name, text)
	g_HistTeam[g_HistPos] = team
	g_HistPos = (g_HistPos + 1) % HIST
	if (g_HistCount < HIST)
		g_HistCount++
}

// gm_ai_url split into host and port; false when the model isn't there
bool:ai_target(host[], len, &port)
{
	if (!g_Sockets || !g_pAiUrl)
		return false
	new url[64]
	get_pcvar_string(g_pAiUrl, url, charsmax(url))
	trim(url)
	new colon = contain(url, ":")
	if (colon <= 0)
		return false
	port = str_to_num(url[colon + 1])
	url[colon] = 0
	copy(host, len, url)
	return port > 0 && port < 65536
}

// `src` as the inside of a JSON string: quotes, backslashes and control characters escaped;
// bytes past ascii as \u00XX, so the request is always valid JSON
json_add(out[], len, const src[])
{
	new k = strlen(out)
	for (new i = 0; src[i] && k < len - 7; i++)
	{
		new c = src[i] & 0xFF
		if (c == '"' || c == '\')
		{
			out[k++] = '\'
			out[k++] = c
		}
		else if (c == 10)
		{
			out[k++] = '\'
			out[k++] = 'n'
		}
		else if (c < 32 || c >= 127)
		{
			static const HEX[] = "0123456789abcdef"
			out[k++] = '\'
			out[k++] = 'u'
			out[k++] = '0'
			out[k++] = '0'
			out[k++] = HEX[c >> 4]
			out[k++] = HEX[c & 15]
		}
		else
			out[k++] = c
	}
	out[k] = 0
}

// "key": "value",
json_field(out[], len, const key[], const value[], bool:comma = true)
{
	// (no fmt here: callers may pass a fmt() result as the value)
	add(out, len, "^"")
	add(out, len, key)
	add(out, len, "^": ^"")
	json_add(out, len, value)
	add(out, len, comma ? "^", " : "^"")
}

// Ask the model for `bot`'s answer to `human`. `written` is the line it says otherwise.
// True when the request is on its way.
bool:ai_ask(bot, human, const written[], const section[], bool:team, Float:extra, bool:thread)
{
	new host[48], port
	if (!ai_target(host, charsmax(host), port) || g_AiState[bot] || !is_user_connected(human))
		return false
	new err
	new sock = socket_open(host, port, SOCKET_TCP, err, SOCK_NON_BLOCKING)
	if (sock <= 0)
		return false

	static body[1700]
	new name[32]
	body = "{"
	get_user_name(bot, name, charsmax(name))
	json_field(body, charsmax(body), "bot", name)
	// the letter for the ones the app knows, else a few words
	new p = g_Persona[bot]
	if (contain("ftsjqnockxbiduBDHLOS", fmt("%c", PERSONAS[p])) != -1)
		json_field(body, charsmax(body), "persona", fmt("%c", PERSONAS[p]))
	else
		json_field(body, charsmax(body), "persona", PERSONA_DESC[p])
	new mood[8]
	mood_of(bot, human, mood, charsmax(mood))
	json_field(body, charsmax(body), "mood", contain(mood, "-") != -1 ? "tilted" : contain(mood, "+") != -1 ? "cocky"
		: contain(mood, "^^") != -1 ? "polite" : "normal")
	json_field(body, charsmax(body), "kind", thread ? "thread" : "reply")
	json_field(body, charsmax(body), "message", g_HumanMsg)
	get_user_name(human, name, charsmax(name))
	json_field(body, charsmax(body), "human", name)
	// the last chat lines the person could see, oldest first
	add(body, charsmax(body), "^"history^": [")
	new t = get_user_team(human), first = 1, shown
	// count back up to 8 visible lines
	new idx[8]
	for (new back = 0; back < g_HistCount && shown < 8; back++)
	{
		new i = (g_HistPos - 1 - back + HIST * 2) % HIST
		if (!g_HistTeam[i] || g_HistTeam[i] == t)
			idx[shown++] = i
	}
	for (new k = shown - 1; k >= 0; k--)
	{
		add(body, charsmax(body), first ? "^"" : ", ^"")
		json_add(body, charsmax(body), g_Hist[idx[k]])
		add(body, charsmax(body), "^"")
		first = 0
	}
	add(body, charsmax(body), "], ^"context^": {")
	// map, score and alive, then only what this answer is about (the app shows a few
	// extra keys, so nothing that doesn't matter here)
	new map[32], cmap[32]
	get_mapname(map, charsmax(map))
	casual_map(map, cmap, charsmax(cmap))
	json_field(body, charsmax(body), "map", cmap)
	new bt = get_user_team(bot)
	if (bt == 1 || bt == 2)
	{
		new us = g_Score[bt], them = g_Score[3 - bt]
		json_field(body, charsmax(body), "score", us > them ? fmt("%d %d us", us, them) : us < them ? fmt("%d %d them", them, us) : fmt("%d %d tied", us, them))
	}
	if (g_CtxPlace[0])
		json_field(body, charsmax(body), equal(section, "r_how") ? "killer_place" : equal(section, "r_bomb", 6) ? "bomb_place" : "enemy_place", g_CtxPlace)
	if (g_CtxWeapon[0])
		json_field(body, charsmax(body), "killer_weapon", g_CtxWeapon)
	if (g_CtxHs)
		add(body, charsmax(body), "^"headshot^": true, ")
	if (g_CtxCarrier[0])
		json_field(body, charsmax(body), "bomb_carrier", g_CtxCarrier)
	if (equal(section, "r_bomb_you"))
		add(body, charsmax(body), "^"human_has_bomb^": true, ")
	else if (equal(section, "r_bomb_me"))
		add(body, charsmax(body), "^"bot_has_bomb^": true, ")
	if (equal(section, "r_timeleft", 10) && get_timeleft() > 0)
		add(body, charsmax(body), fmt("^"timeleft_min^": %d, ", (get_timeleft() + 30) / 60))
	if (equal(section, "r_nextmap", 9))
	{
		new next[32]
		next_map(next, charsmax(next))
		if (next[0])
			json_field(body, charsmax(body), "nextmap", next)
	}
	if (contain(mood, "=") != -1)
		add(body, charsmax(body), "^"knows_human^": true, ")
	if (team)
		add(body, charsmax(body), "^"team_chat^": true, ")
	if (thread)
		json_field(body, charsmax(body), "topic", g_ThreadTopic[human])
	add(body, charsmax(body), fmt("^"alive^": %s", is_user_alive(bot) ? "true" : "false"))
	add(body, charsmax(body), "}}")

	formatex(g_AiReq[bot], charsmax(g_AiReq[]), "POST /api/botchat/reply HTTP/1.1^r^nHost: %s:%d^r^nContent-Type: application/json^r^nContent-Length: %d^r^nConnection: close^r^n^r^n%s",
		host, port, strlen(body), body)
	g_AiSock[bot] = sock
	g_AiState[bot] = 1
	g_AiSent[bot] = 0
	g_AiGot[bot] = 0
	g_AiResp[bot][0] = 0
	g_AiStart[bot] = get_gametime()
	g_AiUser[bot] = get_user_userid(bot)
	g_AiHuman[bot] = human
	g_AiHumanUser[bot] = get_user_userid(human)
	g_AiTeam[bot] = team
	g_AiExtra[bot] = extra
	g_AiRound[bot] = g_Round
	copy(g_AiFallback[bot], charsmax(g_AiFallback[]), written)
	copy(g_AiSection[bot], charsmax(g_AiSection[]), section)
	set_task(0.1, "task_ai_poll", TASK_AI + bot, _, _, "b")
	return true
}

// The answer no longer fits: the bot or the person left, or the round moved on
bool:ai_stale(bot)
{
	new human = g_AiHuman[bot]
	return !is_user_connected(bot) || get_user_userid(bot) != g_AiUser[bot] || !is_user_connected(human)
		|| get_user_userid(human) != g_AiHumanUser[bot] || g_Round != g_AiRound[bot]
}

ai_close(bot)
{
	remove_task(TASK_AI + bot)
	if (g_AiState[bot] && g_AiSock[bot] > 0)
		socket_close(g_AiSock[bot])
	g_AiSock[bot] = 0
	g_AiState[bot] = 0
}

// Done with a request: the model's text, or the written line, typed in the time that's left
ai_finish(bot, const said[])
{
	new bool:stale = ai_stale(bot)
	ai_close(bot)
	if (stale)
		return
	new text[128], bool:raw = said[0] != 0
	copy(text, charsmax(text), raw ? said : g_AiFallback[bot])
	if (!text[0])
		return
	// the time already spent waiting counts as reading it
	new Float:delay = g_AiExtra[bot] + typing_time(text, 1.2, 3.0) - (get_gametime() - g_AiStart[bot])
	queue(bot, g_AiHuman[bot], text, 1, g_AiTeam[bot], floatmax(delay, 0.3), g_AiSection[bot], false, raw)
}

public task_ai_poll(task)
{
	new bot = task - TASK_AI
	if (!g_AiState[bot])
	{
		remove_task(task)
		return
	}
	if (ai_stale(bot))
	{
		ai_finish(bot, "")
		return
	}
	new sock = g_AiSock[bot]
	if (get_gametime() - g_AiStart[bot] > AI_TIMEOUT)
	{
		ai_finish(bot, "")
		return
	}
	// connecting: send the request once the socket is ready
	if (g_AiState[bot] == 1)
	{
		if (!socket_is_writable(sock, 0))
			return
		new total = strlen(g_AiReq[bot])
		new n = socket_send(sock, g_AiReq[bot][g_AiSent[bot]], total - g_AiSent[bot])
		if (n < 0)
		{
			ai_finish(bot, "")
			return
		}
		g_AiSent[bot] += n
		if (g_AiSent[bot] >= total)
			g_AiState[bot] = 2
		return
	}
	// waiting: read what's there; the server closes the connection when it's done
	static chunk[512]
	while (socket_is_readable(sock, 0))
	{
		new n = socket_recv(sock, chunk, charsmax(chunk))
		if (n <= 0)
		{
			ai_parse(bot)
			return
		}
		chunk[min(n, charsmax(chunk))] = 0
		new room = charsmax(g_AiResp[]) - g_AiGot[bot]
		if (room <= 0)
		{
			ai_parse(bot)
			return
		}
		copy(g_AiResp[bot][g_AiGot[bot]], room, chunk)
		g_AiGot[bot] = strlen(g_AiResp[bot])
	}
	// everything the headers said is here (some servers keep the connection a moment)
	new body = contain(g_AiResp[bot], "^r^n^r^n")
	new cl = containi(g_AiResp[bot], "content-length:")
	if (body > 0 && cl > 0 && cl < body)
	{
		new want_len = str_to_num(g_AiResp[bot][cl + 15])
		if (g_AiGot[bot] - (body + 4) >= want_len)
			ai_parse(bot)
	}
}

// 200 with {"text": "..."}: the bot says it; anything else: the written line
ai_parse(bot)
{
	new resp[1024]
	copy(resp, charsmax(resp), g_AiResp[bot])
	new said[128]
	if (equal(resp, "HTTP/1.", 7) && str_to_num(resp[9]) == 200)
	{
		new body = contain(resp, "^r^n^r^n")
		new at = body > 0 ? contain(resp[body], "^"text^"") : -1
		if (at >= 0)
		{
			new i = body + at + 6
			while (resp[i] && resp[i] != '"')
				i++
			if (resp[i] == '"')
				json_string(resp[i + 1], said, charsmax(said))
		}
	}
	clean_ai(said)
	ai_finish(bot, said)
}

// A JSON string's text (after the opening quote), escapes undone; non-ascii dropped
json_string(const src[], out[], len)
{
	new k
	for (new i = 0; src[i] && src[i] != '"' && k < len; i++)
	{
		new c = src[i]
		if (c == '\' && src[i + 1])
		{
			new e = src[++i]
			switch (e)
			{
				case 'n', 'r', 't': c = ' '
				case 'u':
				{
					new v
					for (new h = 1; h <= 4 && src[i + h]; h++)
					{
						new d = src[i + h]
						v = v * 16 + (d >= '0' && d <= '9' ? d - '0' : d >= 'a' && d <= 'f' ? d - 'a' + 10 : d >= 'A' && d <= 'F' ? d - 'A' + 10 : 0)
					}
					i += 4
					c = v < 128 ? v : 0
				}
				default: c = e
			}
		}
		if (c >= 32 && c < 127)
			out[k++] = c
	}
	out[k] = 0
}

// What the model wrote, made safe to say: one line, no quotes or commands, not too long
clean_ai(text[])
{
	for (new i = 0; text[i]; i++)
		if (text[i] == '"')
			text[i] = 39
	trim(text)
	while (text[0] == '@' || text[0] == '/' || text[0] == '!' || text[0] == '.')
	{
		copy(text, 127, text[1])
		trim(text)
	}
	if (strlen(text) > 100)
	{
		text[100] = 0
		new sp = 100
		while (sp > 60 && text[sp] != ' ')
			sp--
		if (sp > 60)
			text[sp] = 0
	}
	trim(text)
}

// `word` as a whole word ("hi" in "hi all", not in "this"), or anywhere for "?"
bool:word_in(const msg[], const word[])
{
	if (word[0] == '?')
		return contain(msg, "?") != -1
	new len = strlen(word), from = 0, at
	while ((at = contain(msg[from], word)) != -1)
	{
		new s = from + at
		new before = s > 0 ? msg[s - 1] : ' ', after = msg[s + len]
		if (!isalnum(before) && !isalnum(after))
			return true
		from = s + 1
	}
	return false
}
