/*
	gm_community - stats, chat moderation, a chat log and player reports, for
	Loner-Strike's Community page.

	Stats: the csx module's ranks (the same ones statsx's /rank, /top15 and /rankstats
	  show) are written to data/gamemodes/stats.json (JSON lines) when a map ends and
	  on gm_statsdump. Bots are marked ("bot":1): their authid is BOT, or, when ranks
	  go by name, their name is one a bot has used (data/gamemodes/bots.txt).
	  Time played isn't kept by csx, so this plugin keeps it in playtime.txt.
	  gm_statsreset sets csstats_reset 1: csx clears the ranks when the map changes.
	  gm_stats_bots 1/0 sets csstats_rankbots (does playing against bots count),
	  -1 leaves it as amxx.cfg has it.
	Mutes (gags): data/gamemodes/gags.txt, one "<SteamID>\t<until>\t<c|v|cv>\t<name>"
	  per line (until is a unix time, 0 for until removed; c chat, v voice). The app
	  writes it and sends gm_commreload; admins (flag i) use amx_gag / amx_ungag, which
	  write it too. A mute follows the SteamID, so it lasts through a reconnect. A
	  player without a real SteamID (LAN) can be muted in game until they leave.
	  Bots and the listen-server host are never muted.
	Chat: the line the game sends (SayText) is rewritten or dropped, the way gm_vip's
	  chat tag works, so no say command is swallowed: commands other plugins handle
	  (/rtv, /rank, /guns...) still work, also for someone muted.
	  gm_filter 0 off, 1 swear words become ***, 2 the message isn't sent.
	  Words: data/gamemodes/filter.txt, one per line. A word matches at the start of
	  a word, so "ass" doesn't catch "class" (it does catch "asshole").
	Chat log: gm_chatlog 1 appends every chat line to data/gamemodes/chat.log
	  ("time\tSteamID\tname\tteam\tall|team\tstatus\ttext"); over 512 KB it moves to
	  chat.old.log and starts again.
	Reports: say /report <name> <reason> (gm_report 1). Appended to reports.log for the
	  app, which shows a desktop notification; admins in game see it too.

	Settings: data/gamemodes/community.cfg (written by the app), re-read with
	gm_commreload. Cvars: gm_filter, gm_chatlog, gm_report, gm_stats_bots
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fakemeta>
#include <csx>

#define PLUGIN  "Community"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TAG          "^4[Server]^1"
#define GAG_CHAT     1
#define GAG_VOICE    2
#define MAX_WORDS    300
#define WORD_LEN     32
#define MAX_GAGS     1000
#define CHATLOG_MAX  524288
#define MAX_BOTNAMES 3000
#define TASK_EXPIRE  7400
#define TASK_BOTS    7401
#define TASK_DUMP    7402
#define DUMP_STEP    300       // ranks written per tick of a dump

enum _:Gag
{
	GAG_ID[36],
	GAG_NAME[32],
	GAG_UNTIL,
	GAG_FLAGS
}

new g_pFilter, g_pChatlog, g_pReport, g_pStatsBots
new g_DataDir[128]
new Array:g_Words
new Array:g_Gags
new Trie:g_Play            // SteamID (or "n:<name>") -> seconds played
new Trie:g_BotNames
new bool:g_ResetPending
// A dump in progress: csx can hold thousands of ranks and get_stats walks its list from
// the top for each one, so they're written DUMP_STEP at a time, 0.1 s apart, to
// stats.json.part, which replaces stats.json once it's complete
new g_DumpFile, g_DumpNext, g_DumpNum

new bool:g_Human[33]       // a real player: not a bot, not HLTV
new g_Auth[33][36]
new g_Muted[33]            // GAG_* bits
new g_MutedUntil[33]       // unix time, 0 = until removed
new bool:g_LocalMute[33]   // muted in game without a SteamID: until they leave
new g_Joined[33]           // unix time, for time played
new Float:g_LastNotice[33]
new Float:g_LastReport[33]

public plugin_natives()
{
	// the plugin still loads (mutes, filter, log, reports) if csx isn't there
	set_native_filter("native_filter")
}

public native_filter(const name[], index, trap)
{
	return trap ? PLUGIN_CONTINUE : PLUGIN_HANDLED
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pFilter    = register_cvar("gm_filter", "1")
	g_pChatlog   = register_cvar("gm_chatlog", "1")
	g_pReport    = register_cvar("gm_report", "1")
	g_pStatsBots = register_cvar("gm_stats_bots", "-1")

	register_srvcmd("gm_commreload", "srv_reload")
	register_srvcmd("gm_statsdump", "srv_dump")
	register_srvcmd("gm_statsreset", "srv_reset")
	register_concmd("amx_gag", "cmd_gag", ADMIN_CHAT, "<name, #userid or SteamID> <minutes, 0 until removed> [chat|voice|both]")
	register_concmd("amx_ungag", "cmd_ungag", ADMIN_CHAT, "<name, #userid or SteamID>")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say_team")
	register_message(get_user_msgid("SayText"), "msg_saytext")
	register_forward(FM_Voice_SetClientListening, "fw_voice")
	register_event("30", "ev_intermission", "a")

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
	g_Words = ArrayCreate(WORD_LEN)
	g_Gags = ArrayCreate(Gag)
	g_Play = TrieCreate()
	g_BotNames = TrieCreate()

	set_task(15.0, "task_expire", TASK_EXPIRE, _, _, "b")
}

public plugin_cfg()
{
	load_settings(false)
	load_gags()
	load_playtime()
	load_botnames()
	// after amxx.cfg, which admin.amxx runs from its own plugin_cfg
	set_task(3.0, "task_rankbots", TASK_BOTS)
}

public plugin_end()
{
	new now = get_systime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_Human[id] && is_user_connected(id))
		{
			add_play(id, now)
			g_Joined[id] = now
		}
	}
	save_playtime()
	// the map is ending: no frames left to spread it over
	dump_stats(true)
}

load_settings(bool:now = true)
{
	new path[160]
	formatex(path, charsmax(path), "%s/community.cfg", g_DataDir)
	if (file_exists(path))
	{
	// at map start the exec just goes in the queue: running the queue then could also
	// run a map change someone queued, in the middle of this map starting
		server_cmd("exec ^"%s^"", path)
		if (now)
			server_exec()
	}
	load_words()
}

public task_rankbots()
{
	new v = get_pcvar_num(g_pStatsBots)
	if (v >= 0 && cvar_exists("csstats_rankbots"))
		set_cvar_num("csstats_rankbots", v ? 1 : 0)
}

public srv_reload()
{
	load_settings()
	load_gags(true)
	task_rankbots()
	server_print("[community] settings reloaded")
	return PLUGIN_HANDLED
}

public srv_dump()
{
	dump_stats()
	// (the file is complete a moment later when there are many ranks)
	server_print("[community] stats written")
	return PLUGIN_HANDLED
}

public srv_reset()
{
	if (cvar_exists("csstats_reset"))
		set_cvar_num("csstats_reset", 1)
	g_ResetPending = true
	TrieClear(g_Play)
	delete_file(fmt("%s/playtime.txt", g_DataDir))
	new now = get_systime()
	for (new id = 1; id <= MaxClients; id++)
		g_Joined[id] = now
	// a dump still going has the old stats in it
	dump_cancel()
	dump_stats(true)
	server_print("[community] stats clear at the next map change")
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Players
// ---------------------------------------------------------------------------

bool:real_steamid(const auth[])
{
	return equal(auth, "STEAM_", 6) && !equal(auth, "STEAM_ID", 8)
}

bool:listen_host(id)
{
	return id == 1 && !is_dedicated_server()
}

public client_authorized(id)
{
	setup_player(id)
}

public client_putinserver(id)
{
	setup_player(id)
	if (is_user_bot(id))
		remember_bot(id)
}

setup_player(id)
{
	g_Human[id] = !is_user_bot(id) && !is_user_hltv(id)
	get_user_authid(id, g_Auth[id], charsmax(g_Auth[]))
	if (!g_Joined[id])
		g_Joined[id] = get_systime()
	if (!g_LocalMute[id])
		apply_gag(id, false)
}

public client_disconnected(id)
{
	if (g_Human[id] && g_Joined[id])
		add_play(id, get_systime())
	g_Human[id] = false
	g_Auth[id][0] = 0
	g_Muted[id] = 0
	g_MutedUntil[id] = 0
	g_LocalMute[id] = false
	g_Joined[id] = 0
	g_LastReport[id] = 0.0
	g_LastNotice[id] = 0.0
}

// gm_pubserver gives bots clan tags after they join
public client_infochanged(id)
{
	if (is_user_connected(id) && is_user_bot(id))
	{
		new name[32]
		get_user_info(id, "name", name, charsmax(name))
		remember_bot_name(name)
	}
}

// ---------------------------------------------------------------------------
// Mutes
// ---------------------------------------------------------------------------

bool:muted(id, what)
{
	if (!(g_Muted[id] & what))
		return false
	if (g_MutedUntil[id] && get_systime() >= g_MutedUntil[id])
		return false
	return true
}

find_gag(const auth[])
{
	new g[Gag]
	for (new i = 0, n = ArraySize(g_Gags); i < n; i++)
	{
		ArrayGetArray(g_Gags, i, g)
		if (equal(g[GAG_ID], auth))
			return i
	}
	return -1
}

// The stored mute for this player, if any. Tells them when it's new.
apply_gag(id, bool:tell)
{
	if (!g_Human[id] || listen_host(id) || g_LocalMute[id])
		return
	new was = g_Muted[id]
	g_Muted[id] = 0
	g_MutedUntil[id] = 0
	if (real_steamid(g_Auth[id]))
	{
		new i = find_gag(g_Auth[id])
		if (i >= 0)
		{
			new g[Gag]
			ArrayGetArray(g_Gags, i, g)
			if (!g[GAG_UNTIL] || g[GAG_UNTIL] > get_systime())
			{
				g_Muted[id] = g[GAG_FLAGS]
				g_MutedUntil[id] = g[GAG_UNTIL]
			}
		}
	}
	if (tell && is_user_connected(id) && g_Muted[id] != was)
	{
		if (g_Muted[id])
			tell_muted(id)
		else if (was)
			client_print_color(id, print_team_default, "%s You can talk again.", TAG)
	}
}

tell_muted(id)
{
	new what[24]
	switch (g_Muted[id] & (GAG_CHAT | GAG_VOICE))
	{
		case GAG_CHAT: copy(what, charsmax(what), "from chat")
		case GAG_VOICE: copy(what, charsmax(what), "from voice chat")
		default: copy(what, charsmax(what), "from chat and voice")
	}
	if (g_MutedUntil[id])
	{
		new left = max(1, (g_MutedUntil[id] - get_systime() + 59) / 60)
		if (left == 1)
			client_print_color(id, print_team_red, "%s You're muted %s for ^3one more minute^1.", TAG, what)
		else
			client_print_color(id, print_team_red, "%s You're muted %s for ^3%d more minutes^1.", TAG, what, left)
	}
	else
		client_print_color(id, print_team_red, "%s You're muted %s.", TAG, what)
}

load_gags(bool:tell = false)
{
	ArrayClear(g_Gags)
	new f = fopen(fmt("%s/gags.txt", g_DataDir), "rt")
	if (f)
	{
		new line[160], sid[40], until[16], flags[8], name[32], rest[120], now = get_systime()
		while (fgets(f, line, charsmax(line)) && ArraySize(g_Gags) < MAX_GAGS)
		{
			trim(line)
			if (!line[0] || line[0] == ';')
				continue
			strtok2(line, sid, charsmax(sid), rest, charsmax(rest), '^t')
			strtok2(rest, until, charsmax(until), line, charsmax(line), '^t')
			strtok2(line, flags, charsmax(flags), name, charsmax(name), '^t')
			new g[Gag]
			copy(g[GAG_ID], charsmax(g[GAG_ID]), sid)
			copy(g[GAG_NAME], charsmax(g[GAG_NAME]), name)
			g[GAG_UNTIL] = str_to_num(until)
			if (contain(flags, "c") >= 0)
				g[GAG_FLAGS] |= GAG_CHAT
			if (contain(flags, "v") >= 0)
				g[GAG_FLAGS] |= GAG_VOICE
			if (!real_steamid(sid) || !g[GAG_FLAGS] || (g[GAG_UNTIL] && g[GAG_UNTIL] <= now))
				continue
			ArrayPushArray(g_Gags, g)
		}
		fclose(f)
	}
	for (new id = 1; id <= MaxClients; id++)
		if (g_Human[id] && is_user_connected(id))
			apply_gag(id, tell)
}

save_gags()
{
	new f = fopen(fmt("%s/gags.txt", g_DataDir), "wt")
	if (!f)
		return
	fputs(f, "; Mutes from Loner-Strike's Community page and amx_gag: SteamID, until (unix time, 0 = until removed), c chat / v voice, name^n")
	new g[Gag], flags[4], now = get_systime()
	for (new i = 0, n = ArraySize(g_Gags); i < n; i++)
	{
		ArrayGetArray(g_Gags, i, g)
		if (g[GAG_UNTIL] && g[GAG_UNTIL] <= now)
			continue
		flags[0] = 0
		if (g[GAG_FLAGS] & GAG_CHAT)
			add(flags, charsmax(flags), "c")
		if (g[GAG_FLAGS] & GAG_VOICE)
			add(flags, charsmax(flags), "v")
		fprintf(f, "%s^t%d^t%s^t%s^n", g[GAG_ID], g[GAG_UNTIL], flags, g[GAG_NAME])
	}
	fclose(f)
}

// Mutes that ran out: tell the player once.
public task_expire()
{
	new now = get_systime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_Muted[id] && g_MutedUntil[id] && now >= g_MutedUntil[id])
		{
			g_Muted[id] = 0
			g_MutedUntil[id] = 0
			g_LocalMute[id] = false
			if (is_user_connected(id))
				client_print_color(id, print_team_default, "%s Your mute ran out. You can talk again.", TAG)
		}
	}
}

// Who the admin means: a player in the game, or a SteamID that isn't here (-2), or nobody (0).
target_of(id, const arg[])
{
	if (real_steamid(arg))
	{
		for (new p = 1; p <= MaxClients; p++)
			if (g_Human[p] && is_user_connected(p) && equal(g_Auth[p], arg))
				return p
		return -2
	}
	return cmd_target(id, arg, CMDTARGET_OBEY_IMMUNITY | CMDTARGET_NO_BOTS | CMDTARGET_ALLOW_SELF)
}

public cmd_gag(id, level, cid)
{
	if (!cmd_access(id, level, cid, 3))
		return PLUGIN_HANDLED
	new arg[40], mins[12], kind[12]
	read_argv(1, arg, charsmax(arg))
	read_argv(2, mins, charsmax(mins))
	read_argv(3, kind, charsmax(kind))
	new minutes = clamp(str_to_num(mins), 0, 525600)
	new flags = GAG_CHAT | GAG_VOICE
	if (equali(kind, "chat"))
		flags = GAG_CHAT
	else if (equali(kind, "voice"))
		flags = GAG_VOICE

	new p = target_of(id, arg)
	if (!p)
		return PLUGIN_HANDLED
	if (p > 0 && listen_host(p))
	{
		console_print(id, "[community] The host can't be muted.")
		return PLUGIN_HANDLED
	}
	new until = minutes ? get_systime() + minutes * 60 : 0
	new auth[36], name[32]
	if (p > 0)
	{
		copy(auth, charsmax(auth), g_Auth[p])
		get_user_name(p, name, charsmax(name))
		replace_all(name, charsmax(name), "^t", " ")
	}
	else
		copy(auth, charsmax(auth), arg)

	if (real_steamid(auth))
	{
		new g[Gag]
		copy(g[GAG_ID], charsmax(g[GAG_ID]), auth)
		copy(g[GAG_NAME], charsmax(g[GAG_NAME]), name)
		g[GAG_UNTIL] = until
		g[GAG_FLAGS] = flags
		new i = find_gag(auth)
		if (i >= 0)
		{
			new old[Gag]
			ArrayGetArray(g_Gags, i, old)
			if (!name[0])
				copy(g[GAG_NAME], charsmax(g[GAG_NAME]), old[GAG_NAME])
			ArraySetArray(g_Gags, i, g)
		}
		else
			ArrayPushArray(g_Gags, g)
		save_gags()
		if (p > 0)
			apply_gag(p, true)
	}
	else
	{
		// no SteamID to remember: muted until they leave
		g_LocalMute[p] = true
		g_Muted[p] = flags
		g_MutedUntil[p] = until
		tell_muted(p)
	}
	if (!name[0])
		copy(name, charsmax(name), auth)
	if (minutes)
		console_print(id, "[community] Muted %s for %d minutes.", name, minutes)
	else
		console_print(id, "[community] Muted %s until removed.", name)
	return PLUGIN_HANDLED
}

public cmd_ungag(id, level, cid)
{
	if (!cmd_access(id, level, cid, 2))
		return PLUGIN_HANDLED
	new arg[40]
	read_argv(1, arg, charsmax(arg))
	new p = target_of(id, arg)
	if (!p)
		return PLUGIN_HANDLED
	new auth[36]
	if (p > 0)
		copy(auth, charsmax(auth), g_Auth[p])
	else
		copy(auth, charsmax(auth), arg)
	new i = real_steamid(auth) ? find_gag(auth) : -1
	if (i >= 0)
	{
		ArrayDeleteItem(g_Gags, i)
		save_gags()
	}
	if (p > 0)
	{
		g_LocalMute[p] = false
		apply_gag(p, true)
	}
	console_print(id, "[community] Unmuted %s.", arg)
	return PLUGIN_HANDLED
}

// Voice: a muted player isn't heard by anyone
public fw_voice(receiver, sender, bool:listen)
{
	if (sender < 1 || sender > MaxClients || receiver == sender || !(g_Muted[sender] & GAG_VOICE) || !muted(sender, GAG_VOICE))
		return FMRES_IGNORED
	engfunc(EngFunc_SetClientListening, receiver, sender, false)
	forward_return(FMV_CELL, false)
	return FMRES_SUPERCEDE
}

// ---------------------------------------------------------------------------
// Chat: the swear filter, the log and /report
// ---------------------------------------------------------------------------

load_words()
{
	ArrayClear(g_Words)
	new f = fopen(fmt("%s/filter.txt", g_DataDir), "rt")
	if (!f)
		return
	new line[64]
	while (fgets(f, line, charsmax(line)) && ArraySize(g_Words) < MAX_WORDS)
	{
		trim(line)
		if (strlen(line) < 2 || line[0] == ';' || strlen(line) >= WORD_LEN)
			continue
		strtolower(line)
		ArrayPushString(g_Words, line)
	}
	fclose(f)
}

bool:word_char(c)
{
	return isalnum(c) || c == '_' || c > 127
}

// Swear words become stars. True when something matched.
bool:censor(text[])
{
	new bool:hit, word[WORD_LEN]
	for (new w = 0, n = ArraySize(g_Words); w < n; w++)
	{
		ArrayGetString(g_Words, w, word, charsmax(word))
		new wl = strlen(word), from = 0
		while (text[from])
		{
			new at = containi(text[from], word)
			if (at < 0)
				break
			at += from
			if (at == 0 || !word_char(text[at - 1]))
			{
				for (new k = 0; k < wl; k++)
					text[at + k] = '*'
				hit = true
			}
			from = at + 1
		}
	}
	return hit
}

bool:has_swear(const text[])
{
	new copy_[192]
	copy(copy_, charsmax(copy_), text)
	return censor(copy_)
}

// The words of a command other plugins answer: a muted player's still work.
bool:command_like(const text[])
{
	if (text[0] == '/' || text[0] == '!' || text[0] == '.')
		return true
	static const words[][] = { "rtv", "rockthevote", "nominate", "nextmap", "timeleft", "thetime", "currentmap", "ff", "motd", "guns", "menu" }
	new first[16]
	copyc(first, charsmax(first), text, ' ')
	for (new i = 0; i < sizeof words; i++)
		if (equali(first, words[i]))
			return true
	return false
}

public cmd_say(id)
{
	return on_say(id, false)
}

public cmd_say_team(id)
{
	return on_say(id, true)
}

on_say(id, bool:team)
{
	if (id < 1 || id > MaxClients || !g_Human[id] || !is_user_connected(id))
		return PLUGIN_CONTINUE
	new text[192]
	read_args(text, charsmax(text))
	remove_quotes(text)
	trim(text)
	if (!text[0])
		return PLUGIN_CONTINUE

	if ((text[0] == '/' || text[0] == '!') && equali(text[1], "report", 6) && (!text[7] || text[7] == ' '))
	{
		log_chat(id, team, text, "report")
		cmd_report(id, text[7])
		return PLUGIN_HANDLED
	}

	new status[12]
	if (muted(id, GAG_CHAT))
	{
		copy(status, charsmax(status), "muted")
		new Float:now = get_gametime()
		if (!command_like(text) && now - g_LastNotice[id] > 3.0)
		{
			g_LastNotice[id] = now
			tell_muted(id)
		}
	}
	else if (get_pcvar_num(g_pFilter) && ArraySize(g_Words) && has_swear(text))
	{
		if (get_pcvar_num(g_pFilter) == 2)
		{
			copy(status, charsmax(status), "blocked")
			client_print_color(id, print_team_red, "%s Your message wasn't sent: it had a word this server doesn't allow.", TAG)
		}
		else
			copy(status, charsmax(status), "filtered")
	}
	log_chat(id, team, text, status)
	return PLUGIN_CONTINUE
}

// The chat line the game sends to each player: dropped for a muted player, or with
// the swear words starred out. Name changes and plugin messages are left alone.
public msg_saytext(msgid, dest, receiver)
{
	if (get_msg_args() < 4)
		return PLUGIN_CONTINUE
	new id = get_msg_arg_int(1)
	if (id < 1 || id > MaxClients || !g_Human[id])
		return PLUGIN_CONTINUE
	new fmt_[64]
	get_msg_arg_string(2, fmt_, charsmax(fmt_))
	// gm_vip may already have rewritten the chat format; it still ends in %s2
	if (!equal(fmt_, "#Cstrike_Chat_", 14) && contain(fmt_, "%s2") < 0)
		return PLUGIN_CONTINUE
	if (muted(id, GAG_CHAT))
		return PLUGIN_HANDLED
	new mode = get_pcvar_num(g_pFilter)
	if (!mode || !ArraySize(g_Words))
		return PLUGIN_CONTINUE
	new text[192]
	get_msg_arg_string(4, text, charsmax(text))
	if (!censor(text))
		return PLUGIN_CONTINUE
	if (mode == 2)
		return PLUGIN_HANDLED
	set_msg_arg_string(4, text)
	return PLUGIN_CONTINUE
}

clean_field(text[], len)
{
	replace_all(text, len, "^t", " ")
	replace_all(text, len, "^n", " ")
	replace_all(text, len, "^r", " ")
}

log_chat(id, bool:team, const said[], const status[])
{
	if (!get_pcvar_num(g_pChatlog))
		return
	new path[160]
	formatex(path, charsmax(path), "%s/chat.log", g_DataDir)
	if (file_exists(path) && file_size(path) > CHATLOG_MAX)
	{
		new old[160]
		formatex(old, charsmax(old), "%s/chat.old.log", g_DataDir)
		delete_file(old)
		rename_file(path, old, 1)
	}
	new f = fopen(path, "at")
	if (!f)
		return
	new name[32], text[192], side[8]
	get_user_name(id, name, charsmax(name))
	clean_field(name, charsmax(name))
	copy(text, charsmax(text), said)
	clean_field(text, charsmax(text))
	switch (cs_get_user_team(id))
	{
		case CS_TEAM_T: copy(side, charsmax(side), "T")
		case CS_TEAM_CT: copy(side, charsmax(side), "CT")
		default: copy(side, charsmax(side), "SPEC")
	}
	fprintf(f, "%d^t%s^t%s^t%s^t%s^t%s^t%s^n", get_systime(), g_Auth[id], name, side, team ? "team" : "all", status, text)
	fclose(f)
}

cmd_report(id, const args[])
{
	if (!get_pcvar_num(g_pReport))
	{
		client_print_color(id, print_team_default, "%s Reports are off on this server.", TAG)
		return
	}
	new who[32], reason[128], rest[160]
	copy(rest, charsmax(rest), args)
	trim(rest)
	strtok2(rest, who, charsmax(who), reason, charsmax(reason), ' ', TRIM_FULL)
	if (!who[0])
	{
		client_print_color(id, print_team_default, "%s Say ^3/report <name> <what happened>^1, like /report Bob wallhacking.", TAG)
		return
	}
	new Float:now = get_gametime()
	if (g_LastReport[id] > 0.0 && now - g_LastReport[id] < 60.0)
	{
		client_print_color(id, print_team_default, "%s You can send one report a minute. Try again shortly.", TAG)
		return
	}

	// an exact name first, else the only name with this in it
	new target, matches
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == id || !is_user_connected(p) || is_user_hltv(p))
			continue
		new name[32]
		get_user_name(p, name, charsmax(name))
		if (equali(name, who))
		{
			target = p
			matches = 1
			break
		}
		if (containi(name, who) >= 0)
		{
			target = p
			matches++
		}
	}
	if (!matches)
	{
		client_print_color(id, print_team_default, "%s Nobody here is called ^3%s^1.", TAG, who)
		return
	}
	if (matches > 1)
	{
		client_print_color(id, print_team_default, "%s More than one player matches ^3%s^1. Type more of the name.", TAG, who)
		return
	}
	if (is_user_bot(target))
	{
		client_print_color(id, print_team_default, "%s That's a bot. Bots can't be reported.", TAG)
		return
	}
	g_LastReport[id] = now
	new rname[32], tname[32]
	get_user_name(id, rname, charsmax(rname))
	get_user_name(target, tname, charsmax(tname))
	clean_field(rname, charsmax(rname))
	clean_field(tname, charsmax(tname))
	clean_field(reason, charsmax(reason))
	if (!reason[0])
		copy(reason, charsmax(reason), "no reason given")

	new f = fopen(fmt("%s/reports.log", g_DataDir), "at")
	if (f)
	{
		new map[32]
		get_mapname(map, charsmax(map))
		fprintf(f, "%d^t%s^t%s^t%s^t%s^t%s^t%s^n", get_systime(), g_Auth[id], rname, g_Auth[target], tname, map, reason)
		fclose(f)
	}
	client_print_color(id, print_team_default, "%s Thanks. Your report about ^3%s^1 was sent to the server's admins.", TAG, tname)
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p != id && g_Human[p] && is_user_connected(p) && (get_user_flags(p) & ADMIN_CHAT) && !listen_host(p))
			client_print_color(p, print_team_red, "^4[Report]^1 %s reported ^3%s^1: %s", rname, tname, reason)
	}
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

play_key(id, out[], len)
{
	if (real_steamid(g_Auth[id]))
		copy(out, len, g_Auth[id])
	else
	{
		new name[32]
		get_user_name(id, name, charsmax(name))
		formatex(out, len, "n:%s", name)
	}
}

add_play(id, now)
{
	new secs = now - g_Joined[id]
	if (secs <= 0 || secs > 86400 * 2)
		return
	new key[48], had
	play_key(id, key, charsmax(key))
	TrieGetCell(g_Play, key, had)
	TrieSetCell(g_Play, key, had + secs)
}

load_playtime()
{
	TrieClear(g_Play)
	new f = fopen(fmt("%s/playtime.txt", g_DataDir), "rt")
	if (!f)
		return
	new line[96], key[48], secs[16]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		strtok2(line, key, charsmax(key), secs, charsmax(secs), '^t')
		if (key[0] && secs[0])
			TrieSetCell(g_Play, key, str_to_num(secs))
	}
	fclose(f)
}

save_playtime()
{
	new f = fopen(fmt("%s/playtime.txt", g_DataDir), "wt")
	if (!f)
		return
	new TrieIter:it = TrieIterCreate(g_Play), key[48], secs
	while (!TrieIterEnded(it))
	{
		TrieIterGetKey(it, key, charsmax(key))
		TrieIterGetCell(it, secs)
		fprintf(f, "%s^t%d^n", key, secs)
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
	fclose(f)
}

load_botnames()
{
	TrieClear(g_BotNames)
	new f = fopen(fmt("%s/bots.txt", g_DataDir), "rt")
	if (!f)
		return
	new line[40]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (line[0])
			TrieSetCell(g_BotNames, line, 1)
	}
	fclose(f)
}

remember_bot(id)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	remember_bot_name(name)
}

remember_bot_name(const name[])
{
	if (!name[0] || TrieKeyExists(g_BotNames, name) || TrieGetSize(g_BotNames) >= MAX_BOTNAMES)
		return
	TrieSetCell(g_BotNames, name, 1)
	new f = fopen(fmt("%s/bots.txt", g_DataDir), "at")
	if (f)
	{
		new clean[32]
		copy(clean, charsmax(clean), name)
		clean_field(clean, charsmax(clean))
		fprintf(f, "%s^n", clean)
		fclose(f)
	}
}

json_escape(const src[], out[], len)
{
	new j
	for (new i = 0; src[i] && j < len - 2; i++)
	{
		new c = src[i]
		if (c == '^"' || c == '\')
		{
			out[j++] = '\'
			out[j++] = c
		}
		else if (c >= 0 && c < 32)
			out[j++] = ' '
		else
			out[j++] = c
	}
	out[j] = 0
}

// Seconds played, counting the current visit of anyone here now
played(const auth[], const name[])
{
	new key[48], secs
	if (real_steamid(auth))
		copy(key, charsmax(key), auth)
	else
		formatex(key, charsmax(key), "n:%s", name)
	TrieGetCell(g_Play, key, secs)
	new now = get_systime(), pname[32], pkey[48]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!g_Human[id] || !g_Joined[id] || !is_user_connected(id))
			continue
		if (real_steamid(g_Auth[id]))
			copy(pkey, charsmax(pkey), g_Auth[id])
		else
		{
			get_user_name(id, pname, charsmax(pname))
			formatex(pkey, charsmax(pkey), "n:%s", pname)
		}
		if (equal(pkey, key))
			secs += max(0, now - g_Joined[id])
	}
	return secs
}

public ev_intermission()
{
	dump_stats()
}

// Writes stats.json: at once when `now` (the map ending, a reset), else over a few frames.
// A dump already going carries on (finished at once when `now`).
dump_stats(bool:now = false)
{
	if (!g_DumpFile)
	{
		g_DumpFile = fopen(fmt("%s/stats.json.part", g_DataDir), "wt")
		if (!g_DumpFile)
			return
		new csx = module_exists("xstats") ? 1 : 0
		g_DumpNum = (csx && !g_ResetPending) ? min(get_statsnum(), 5000) : 0
		g_DumpNext = 0
		new rankbots = cvar_exists("csstats_rankbots") ? get_cvar_num("csstats_rankbots") : -1
		fprintf(g_DumpFile, "{^"t^":^"info^",^"time^":%d,^"csx^":%d,^"rank_by^":%d,^"rankbots^":%d,^"reset_pending^":%d,^"total^":%d}^n",
			get_systime(), csx, get_cvar_num("csstats_rank"), rankbots, g_ResetPending ? 1 : 0, g_DumpNum)
	}
	if (now)
	{
		remove_task(TASK_DUMP)
		dump_ranks(g_DumpNum)
		dump_finish()
	}
	else if (!task_exists(TASK_DUMP))
		task_dump()
}

public task_dump()
{
	if (!g_DumpFile)
		return
	dump_ranks(DUMP_STEP)
	if (g_DumpNext >= g_DumpNum)
		dump_finish()
	else
		set_task(0.1, "task_dump", TASK_DUMP)
}

// Writes up to `count` more ranks of the dump in progress.
dump_ranks(count)
{
	new from = g_DumpNext, to = min(g_DumpNum, g_DumpNext + count)
	g_DumpNext = to
	new stats[STATSX_MAX_STATS], body[MAX_BODYHITS], s2[STATSX_MAX_OBJECTIVE]
	new name[32], auth[36], ename[80], eauth[80]
	for (new i = from; i < to; i++)
	{
		arrayset(stats, 0, sizeof stats)
		arrayset(body, 0, sizeof body)
		arrayset(s2, 0, sizeof s2)
		name[0] = 0
		auth[0] = 0
		get_stats(i, stats, body, name, charsmax(name), auth, charsmax(auth))
		get_stats2(i, s2)
		new bot = (equali(auth, "BOT") || (!real_steamid(auth) && !equal(auth, "VALVE_", 6) && TrieKeyExists(g_BotNames, name))) ? 1 : 0
		json_escape(name, ename, charsmax(ename))
		json_escape(auth, eauth, charsmax(eauth))
		fprintf(g_DumpFile, "{^"t^":^"p^",^"rank^":%d,^"name^":^"%s^",^"auth^":^"%s^",^"bot^":%d,^"k^":%d,^"d^":%d,^"hs^":%d,^"tk^":%d,^"shots^":%d,^"hits^":%d,^"dmg^":%d,^"body^":[%d,%d,%d,%d,%d,%d,%d,%d],^"defusing^":%d,^"defused^":%d,^"planted^":%d,^"exploded^":%d,^"time^":%d}^n",
			i + 1, ename, eauth, bot,
			stats[STATSX_KILLS], stats[STATSX_DEATHS], stats[STATSX_HEADSHOTS], stats[STATSX_TEAMKILLS],
			stats[STATSX_SHOTS], stats[STATSX_HITS], stats[STATSX_DAMAGE],
			body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7],
			s2[STATSX_TOTAL_DEFUSIONS], s2[STATSX_BOMBS_DEFUSED], s2[STATSX_BOMBS_PLANTED], s2[STATSX_BOMB_EXPLOSIONS],
			bot ? 0 : played(auth, name))
	}
}

dump_finish()
{
	fclose(g_DumpFile)
	g_DumpFile = 0
	new path[160], part[160]
	formatex(path, charsmax(path), "%s/stats.json", g_DataDir)
	formatex(part, charsmax(part), "%s/stats.json.part", g_DataDir)
	// (Windows won't rename over an existing file)
	delete_file(path)
	rename_file(part, path, 1)
	save_playtime()
}

dump_cancel()
{
	remove_task(TASK_DUMP)
	if (!g_DumpFile)
		return
	fclose(g_DumpFile)
	g_DumpFile = 0
	delete_file(fmt("%s/stats.json.part", g_DataDir))
}
