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
	the odd typo for the casual ones, and not every time. How often is gm_botchat:
	  0 off, 1 now and then, 2 normal, 3 chatty
	Other cvars: gm_botchat_humans 1 (bots answer what people say)

	Clans: bots that share a clan tag ("[XyZ] Name", "XyZ|Name") are clanmates. They
	talk to each other in team chat ([clan]: a clanmate's kill, a clanmate joining),
	and back each other up when one keeps dying.

	Placeholders in lines: {name} the bot, {other} who it's about or answering,
	{map} the map, {tag} the bot's clan tag. Edit botchat.ini to add your own; gm_botchat_reload re-reads it.
*/

#include <amxmodx>
#include <amxmisc>

#define PLUGIN  "Bot chat"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define MAX_SECTIONS 96
#define TASK_SAY     8100
#define MAX_PENDING  16
#define MAX_CHAIN    3

new const PERSONAS[] = "ftsjqncokxhzpbiduwrmABCDEFGHLMNOPSTY"
// how much each personality talks (same order as PERSONAS)
new const Float:CHATTY[] = { 1.0, 0.8, 1.2, 1.4, 0.3, 1.0, 0.7, 0.8, 1.5, 0.8, 1.6, 1.2, 1.3, 1.3, 1.1, 0.6, 0.9, 1.0, 1.0, 1.3,
	1.1, 1.0, 1.1, 0.8, 1.0, 1.0, 1.0, 1.2, 0.15, 1.0, 1.0, 1.3, 0.9, 1.0, 1.0, 1.6 }

// habits, picked per bot from its name
new const LAUGHS[][] = { "lol", "lol", "lol", "lmao", "haha", "hahaha", "xD", "xd", "kkkk", "jajaja", "mdr", "www", "555", "kek", "hehe", "rofl", "lmfao", "ahah" }
new const WORDS[][] = { "", "", "", "", "bro", "mate", "dude", "man", "lads", "fam", "my guy", "buddy", "bruh", "boys", "guys", "homie" }
new const EMOTES[][] = { "", "", "", ":)", ":D", "^^", ":3", "<3", "o7", ":P", ";)", "xD", ":(" }
new g_Laugh[33], g_Word[33], g_Emote[33], bool:g_TextSpeak[33], g_Typos[33], g_Shouty[33], Float:g_Chatty[33]

new g_pLevel, g_pHumans
new g_CfgDir[128]
new Trie:g_Sections                 // section name -> Array of "letters|text"
new Array:g_Lists[MAX_SECTIONS], g_ListCount
new bool:g_Career

new g_Persona[33]                   // index into PERSONAS
new Float:g_NextBot[33]             // when this bot may talk again
new Float:g_NextAny                 // when any bot may start a new line
new g_RoundKills[33], bool:g_Streaked[33]
new g_DeathRow[33]                  // deaths since this player's last kill
new bool:g_Raged[33]
new Float:g_NextHeard                // bots don't answer every line someone types

// lines waiting to be "typed"
new g_PendBot[MAX_PENDING], g_PendOther[MAX_PENDING], g_PendChain[MAX_PENDING], bool:g_PendTeam[MAX_PENDING]
new g_PendText[MAX_PENDING][128]

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pLevel  = register_cvar("gm_botchat", "2")
	g_pHumans = register_cvar("gm_botchat_humans", "1")
	register_srvcmd("gm_botchat_reload", "cmd_reload")

	register_event("DeathMsg", "ev_death", "a")
	register_event("SendAudio", "ev_twin", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ctwin", "a", "2&%!MRAD_ctwin")
	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	register_event("30", "ev_intermission", "a")
	register_logevent("ev_planted", 3, "2=Planted_The_Bomb")
	register_logevent("ev_defused", 3, "2=Defused_The_Bomb")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say_team")

	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	g_Sections = TrieCreate()
	load_lines()
}

public plugin_cfg()
{
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
				cur = g_Lists[g_ListCount++] = ArrayCreate(128)
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
	g_Raged[id] = false
	if (is_user_bot(id))
	{
		g_Persona[id] = persona_of(id)
		habits_of(id)
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
	if (is_user_connected(id))
		want(any_bot(0), "welcome", id, 0.4)
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

// From the name, so the same regular always has the same personality. New players
// in the Newbie lobby style are more often new to the game too.
persona_of(id)
{
	new name[32], h = 7
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h * 31 + name[i]) & 0xFFFFFF
	new p = h % (sizeof PERSONAS - 1)
	if (get_cvar_num("gm_botstyle") == 4 && h % 3 == 0)
		p = 5
	return p
}

// A bot's own habits, from its name (a different mix of the name than its personality)
habits_of(id)
{
	new name[32], h = 2166136261
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h ^ name[i]) * 16777619
	h &= 0x7FFFFFFF
	g_Laugh[id] = h % sizeof LAUGHS
	g_Word[id] = (h / 7) % sizeof WORDS
	g_Emote[id] = (h / 31) % sizeof EMOTES
	g_TextSpeak[id] = (h / 101) % 3 == 0
	g_Typos[id] = (h / 211) % 15             // percent
	g_Shouty[id] = (h / 401) % 4 == 0 ? 8 : 1
	g_Chatty[id] = 0.4 + float((h / 977) % 13) * 0.1
	new p = PERSONAS[g_Persona[id]]
	if (p == 'x' && g_Laugh[id] < 6)
		g_Laugh[id] = 8 + (h % 6)            // kkkk, jajaja, mdr, www, 555, kek
	if (p == 'k')
		g_TextSpeak[id] = true
}

bool:has_persona(id, const letters[])
{
	if (letters[0] == '*')
		return true
	return contain(letters, fmt("%c", PERSONAS[g_Persona[id]])) != -1
}

// ---- deciding to say something ---------------------------------------------------------

Float:level_mult()
{
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: return 0.35
		case 3: return 2.0
	}
	return 1.0
}

Float:global_gap()
{
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: return 30.0
		case 3: return 4.0
	}
	return 10.0
}

Float:bot_gap()
{
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: return 90.0
		case 3: return 15.0
	}
	return 40.0
}

// A bot might say a line from `section` about `other`, with the chance `base`
// (before the chattiness setting and its personality). chain > 0: a reply.
want(id, const section[], other, Float:base, Float:delay = -1.0, chain = 0, bool:team = false)
{
	if (!id || !is_user_connected(id) || !is_user_bot(id) || g_Career || !get_pcvar_num(g_pLevel))
		return
	new Float:now = get_gametime()
	if (!chain && (now < g_NextAny || now < g_NextBot[id]))
		return
	if (random_float(0.0, 1.0) > base * level_mult() * CHATTY[g_Persona[id]] * g_Chatty[id])
		return
	new text[128]
	if (!pick_line(id, section, text, charsmax(text)))
		return
	queue(id, other, text, chain, team, delay)
}

bool:pick_line(id, const section[], text[], len)
{
	new Array:list
	if (!TrieGetCell(g_Sections, section, list))
		return false
	new n = ArraySize(list)
	if (!n)
		return false
	new raw[192], letters[16]
	for (new tries = 0; tries < 16; tries++)
	{
		ArrayGetString(list, random(n), raw, charsmax(raw))
		new bar = contain(raw, "|")
		copy(letters, min(bar, charsmax(letters)), raw)
		if (has_persona(id, letters))
		{
			copy(text, len, raw[bar + 1])
			trim(text)
			return true
		}
	}
	return false
}

queue(id, other, const text[], chain, bool:team, Float:delay)
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
	g_PendOther[slot] = other
	g_PendChain[slot] = chain
	g_PendTeam[slot] = team
	copy(g_PendText[slot], charsmax(g_PendText[]), text)
	// reading what happened, then typing it
	if (delay < 0.0)
		delay = random_float(0.6, 2.0) + strlen(text) * random_float(0.05, 0.11)
	new Float:now = get_gametime()
	g_NextBot[id] = now + delay + bot_gap() * random_float(0.6, 1.4)
	if (!chain)
		g_NextAny = now + delay + global_gap() * random_float(0.6, 1.4)
	set_task(delay, "task_say", TASK_SAY + slot)
}

public task_say(task)
{
	new slot = task - TASK_SAY
	new id = g_PendBot[slot], other = g_PendOther[slot], chain = g_PendChain[slot]
	g_PendBot[slot] = 0
	if (!is_user_connected(id))
		return

	new text[128], tag[24]
	copy(text, charsmax(text), g_PendText[slot])
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
	new bool:team = g_PendTeam[slot]
	if (text[0] == '@')
	{
		team = true
		copy(text, charsmax(text), text[1])
		trim(text)
	}
	fill(id, other, text, charsmax(text))
	style_text(id, text)
	decorate(id, text, charsmax(text))
	if (!text[0])
		return
	engclient_cmd(id, team ? "say_team" : "say", text)

	// someone answers: the one it was about if that's a bot, else another bot
	if (tag[0] && chain < MAX_CHAIN)
	{
		new who = (other && is_user_bot(other) && other != id) ? other : any_bot(id)
		want(who, fmt("r_%s", tag), id, 0.75, -1.0, chain + 1)
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
		get_mapname(name, charsmax(name))
		replace_string(text, len, "{map}", name)
	}
}

// What people call each other: the name without its clan tag, often just the start
short_name(id, out[], len)
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
	if (strlen(out) > 8 && random_num(1, 100) <= 40)
		out[random_num(4, 6)] = 0
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
// personality likes, now and then shouting
decorate(id, text[], len)
{
	new p = PERSONAS[g_Persona[id]]
	new l = strlen(text)
	if (!l || text[0] == '/')
		return
	// stretch the last vowel of a short word: nice -> niiice, no -> nooo
	if (l <= 8 && (p == 'j' || p == 'f' || p == 'n' || p == 's') && random_num(1, 100) <= 15)
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
		replace_string(text, len, "lol", LAUGHS[g_Laugh[id]])
		replace_string(text, len, "haha", LAUGHS[g_Laugh[id]])
	}
	if (g_TextSpeak[id])
	{
		static const FROM[][] = { "you're", "your", "you", "thanks", "please", "though", "probably", "because", "okay", "really", "people", "tonight" }
		static const TO[][] = { "ur", "ur", "u", "thx", "pls", "tho", "prob", "cuz", "k", "rly", "ppl", "tn" }
		for (new w = 0; w < sizeof FROM; w++)
			replace_word(text, len, FROM[w], TO[w])
	}
	// the word it keeps using
	if (WORDS[g_Word[id]][0] && strlen(text) < 40 && random_num(1, 100) <= 18)
		add(text, len, fmt(" %s", WORDS[g_Word[id]]))
	// its emoticon
	if (EMOTES[g_Emote[id]][0] && random_num(1, 100) <= 14)
	{
		add(text, len, fmt(" %s", EMOTES[g_Emote[id]]))
		return
	}
	// endings
	new r = random_num(1, 100)
	switch (p)
	{
		case 'f': if (r <= 18) add(text, len, random_num(0, 1) ? " :)" : " :D")
		case 'j': if (r <= 25) add(text, len, random_num(0, 2) == 0 ? " xd" : random_num(0, 1) ? " lol" : " :P")
		case 'n': if (r <= 15) add(text, len, random_num(0, 1) ? "?" : " lol")
		case 's': if (r <= 15) add(text, len, random_num(0, 1) ? "..." : " -_-")
		case 'c': if (r <= 12) add(text, len, " haha")
		case 't': if (r <= 10) add(text, len, "!")
		case 'o': if (r <= 10) add(text, len, ".")
		case 'k': if (r <= 25) add(text, len, random_num(0, 1) ? "!!!" : " lol")
		case 'h': if (r <= 30) add(text, len, random_num(0, 1) ? "!!" : " LETS GO")
	}
	// excitement
	if ((p == 'h' && random_num(1, 100) <= 40) || ((p == 'j' || p == 's' || p == 'k') && random_num(1, 100) <= 4 * g_Shouty[id]))
		strtoupper(text)
	else if ((p == 'j' || p == 'n') && random_num(1, 100) <= 10)
		add(text, len, "!!")
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

public ev_newround()
{
	arrayset(g_RoundKills, 0, sizeof g_RoundKills)
	arrayset(g_Streaked, false, sizeof g_Streaked)
	if (random_num(1, 100) <= 12)
		want(any_bot(0), "idle", 0, 0.6, random_float(2.0, 9.0))
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

	g_RoundKills[killer]++
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

	new bool:knife = bool:equal(weapon, "knife"), bool:nade = bool:equal(weapon, "grenade")
	if (knife)
	{
		want(killer, "kill_knife", victim, 0.5)
		want(victim, "death_knife", killer, 0.6)
	}
	else if (nade)
	{
		want(killer, "kill_nade", victim, 0.3)
		want(victim, "death_nade", killer, 0.4)
	}
	else if (hs)
	{
		want(killer, "kill_headshot", victim, 0.15)
		want(victim, "death_headshot", killer, 0.18)
	}
	else
	{
		want(killer, equal(weapon, "awp") ? "kill_awp" : "kill", victim, 0.08)
		want(victim, "death", killer, 0.12)
	}
	// a clanmate on the killer's team cheers them on; one on the victim's side is out for revenge
	new mate = clanmate_bot(killer, true)
	if (mate)
		want(mate, "clan", killer, 0.18, -1.0, 0, true)
	new avenger = clanmate_bot(victim, true)
	if (avenger && random_num(1, 100) <= 50)
		want(avenger, "clan_avenge", killer, 0.25, -1.0, 0, true)
	// a teammate of the killer says something about it
	if (hs || knife)
		want(any_bot_on(get_user_team(killer), killer), "nice_shot", killer, 0.15, -1.0, 0, true)
}

// Dying again and again: a bot tilts, then crashes out; teammates may cheer them up.
// True when something was said about it (the usual death line is skipped then).
bool:mood(victim)
{
	new row = g_DeathRow[victim], team = get_user_team(victim)
	// someone on the team cheers them up (people too)
	if (row == 3 || row == 6)
	{
		new mate = clanmate_bot(victim, true)
		want(mate ? mate : any_bot_on(team, victim), "support", victim, mate ? 0.7 : 0.45, -1.0, 0, true)
	}
	if (!is_user_bot(victim))
		return false
	new p = PERSONAS[g_Persona[victim]]
	if (row >= 5 && !g_Raged[victim] && (p == 's' || p == 'j' || p == 't' || random_num(1, 100) <= 25))
	{
		g_Raged[victim] = true
		crash_out(victim)
		return true
	}
	if (row == 3 || row == 4)
	{
		want(victim, "tilt", 0, 0.5)
		return true
	}
	return false
}

// A few angry lines in quick succession; a salty one may leave for good
crash_out(id)
{
	if (g_Career || !get_pcvar_num(g_pLevel) || random_float(0.0, 1.0) > 0.6 * level_mult())
		return
	new text[128], Float:at = random_float(0.8, 1.6)
	new n = random_num(2, 4)
	for (new i = 0; i < n; i++)
	{
		if (!pick_line(id, "rage", text, charsmax(text)))
			break
		if (random_num(1, 100) <= 35)
			strtoupper(text)
		queue(id, 0, text, MAX_CHAIN, false, at)
		at += random_float(0.7, 2.2) + strlen(text) * 0.04
	}
	if (PERSONAS[g_Persona[id]] == 's' && random_num(1, 100) <= 30 && pick_line(id, "ragequit", text, charsmax(text)))
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
	want(any_bot_on(2, 0), "bomb_planted", 0, 0.2, -1.0, 0, true)
}

public ev_defused()
{
	new id = get_loguser_index()
	want(id, "defused", 0, 0.4)
	want(any_bot_on(1, 0), "they_defused", id, 0.3)
}

public ev_intermission()
{
	// the end of the map: a few gg's, not every bot
	new said
	for (new i = 1; i <= MaxClients && said < 4; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i) || random_num(1, 100) > 45)
			continue
		new text[128]
		if (pick_line(i, "map_end", text, charsmax(text)))
		{
			queue(i, 0, text, MAX_CHAIN, false, random_float(0.5, 4.0))
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

// Words people type, and the section the answer comes from (first match wins)
new const HEARD[][][] =
{
	{ "gg", "r_gg" }, { "good game", "r_gg" },
	{ "hack", "r_hack" }, { "aimbot", "r_hack" }, { "cheat", "r_hack" }, { "wallhack", "r_hack" },
	{ "bot", "r_bot" },
	{ "noob", "r_noob" }, { "nub", "r_noob" }, { "trash", "r_noob" }, { "ez", "r_ez" },
	{ "lag", "r_lag" }, { "ping", "r_lag" },
	{ "nice", "r_nice_from_human" }, { "ns", "r_nice_from_human" }, { "wp", "r_nice_from_human" },
	{ "hello", "r_hi" }, { "hi", "r_hi" }, { "hey", "r_hi" }, { "yo", "r_hi" }, { "sup", "r_hi" },
	{ "lol", "r_lol" }, { "haha", "r_lol" }, { "lmao", "r_lol" },
	{ "wtf", "r_wtf" },
	{ "sorry", "r_sorry" }, { "my bad", "r_sorry" }, { "mb", "r_sorry" },
	{ "rush", "r_plan" }, { "go b", "r_plan" }, { "go a", "r_plan" }, { "eco", "r_plan" },
	{ "?", "r_question" }
}

heard(id, bool:team)
{
	if (!is_user_connected(id) || is_user_bot(id) || !get_pcvar_num(g_pHumans))
		return
	new msg[128]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	if (!msg[0] || msg[0] == '/' || msg[0] == '!' || get_gametime() < g_NextHeard)
		return
	strtolower(msg)

	// talking to a bot by name: that one answers
	new to = 0, name[32]
	for (new i = 1; i <= MaxClients && !to; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i))
			continue
		short_name(i, name, charsmax(name))
		name[min(strlen(name), 5)] = 0
		strtolower(name)
		if (strlen(name) >= 3 && contain(msg, name) != -1)
			to = i
	}

	new section[24]
	for (new w = 0; w < sizeof HEARD; w++)
		if (word_in(msg, HEARD[w][0]))
		{
			copy(section, charsmax(section), HEARD[w][1])
			break
		}
	if (!section[0])
	{
		if (!to)
			return
		copy(section, charsmax(section), "r_named")
	}
	new who = to ? to : (team ? any_bot_on(get_user_team(id), 0) : any_bot(0))
	g_NextHeard = get_gametime() + random_float(3.0, 6.0)
	// a reply jumps the queue like a conversation would, but not every time
	want(who, section, id, to ? 0.85 : 0.4, -1.0, 1, team)
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
