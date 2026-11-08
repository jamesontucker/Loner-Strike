/*
	gm_people - who the bots are (docs/bot-people.md).

	Every bot is someone: an archetype from configs/gamemodes/people.ini ("polite swede",
	"salty vet", "awp whore"...) fixes a few of its traits, the rest are rolled (weighted,
	most people in the middle, combinations that don't go together ruled out). All of it
	comes from the bot's name, so a regular is the same person every time it shows up, and
	a new name is someone new.

	Traits: warmth, ego, temper, talk, humor, age, skill, region, role, two hobbies and a
	playstyle. Each archetype also names the old personality letter it acts as, which
	gm_botchat, gm_humanbots and gm_botfun still read (botchat_persona) while they move over.

	Natives (library "gm_people"):
	  people_match(id, other, key[], op[], value[])  1 / 0, -1 for a key it doesn't know:
	      the conditions bot chat lines use (age=kid, temper>=1, arch=awp_whore, hobby=wow)
	  people_letter(id)                the old personality letter (0 for people, not bots)
	  people_get(id, key[])            a trait's value (-1 unknown)
	  people_arch(id, out[], len)      the archetype's name
	  people_hobby(id, which, out[], len)  one of its two hobbies
	  people_note(bot, other, kind[])  something between them that changes how the bot feels
	                                   about the other one: praise, thanks, greet, joke, saved,
	                                   insult, accuse (gm_botchat calls it from what people say)

	Memory (nVault gm_people): what each bot thinks of each person and each other regular,
	kept between days. Conditions: rel (-5..5, how it feels about them), fam (days played
	together), days (since it last saw them), rival (1: a long, sour history). Feelings fade
	back toward neutral over weeks without seeing each other.
	Console: gm_people <name or #userid> shows who a bot is; gm_people_reload re-reads people.ini.
*/

#include <amxmodx>
#include <amxmisc>
#include <nvault>

#define PLUGIN  "People"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

enum
{
	T_WARMTH, T_EGO, T_TEMPER, T_TALK, T_HUMOR, T_AGE, T_SKILL, T_ROLE, T_REGION, T_STYLE,
	T_COUNT
}
new const TRAIT_NAME[T_COUNT][] = { "warmth", "ego", "temper", "talk", "humor", "age", "skill", "role", "region", "style" }
// the values' names, space separated, in order
new const TRAIT_VALUES[T_COUNT][] = {
	"cold neutral warm",
	"humble normal showoff",
	"chill normal tilty",
	"silent quiet normal chatty spammer",
	"none dry goofy troll",
	"kid teen adult old",
	"newbie average good pro",
	"regular drifter admin clan afk",
	"us uk de se pl ru br fr nl es au fi dk",
	"rusher roamer lurker careful camper awper knifer nader ninja objective weirdgun"
}
new const HOBBIES[] = "wc3 wow halo school work cars music lan cz source awp sleep food girlfriend footy metal"
#define HOBBY_COUNT 16
#define MAX_VALUES  16

#define MAX_ARCH      200
#define MAX_CONFLICTS 64

// archetypes: name, weight, letter, style (-1 rolled), fixed traits (-1 rolled), a fixed hobby (-1)
new g_ArchName[MAX_ARCH][32], g_ArchWeight[MAX_ARCH], g_ArchLetter[MAX_ARCH], g_ArchFixed[MAX_ARCH][T_COUNT], g_ArchHobby[MAX_ARCH]
new g_ArchCount, g_ArchTotal
// conflicts: (trait, value) and (trait, value); hobbies use trait T_COUNT
new g_ConfA[MAX_CONFLICTS], g_ConfAV[MAX_CONFLICTS], g_ConfB[MAX_CONFLICTS], g_ConfBV[MAX_CONFLICTS], g_ConfCount
// weights for rolled traits
new g_Weights[T_COUNT][MAX_VALUES], g_WeightCount[T_COUNT]

// What bot a remembers about b (a is always a bot; b a bot or a person), loaded the first
// time it's needed this map, saved when either leaves or the map ends. Feelings in tenths:
// -50..50 is rel -5..5
new g_Vault = INVALID_HANDLE
new bool:g_MemLoaded[33][33], bool:g_MemDirty[33][33]
// what was said this map has moved a bot's feelings about someone this much (capped: twenty
// "ns" in a row count no more than a few)
new g_NoteSum[33][33]
#define NOTE_CAP 8
new g_Aff[33][33], g_Fam[33][33], g_Last[33][33], g_PrevLast[33][33], g_KillsOn[33][33], g_DeathsBy[33][33]
new Float:g_JoinedAt[33]
#define FAM_AFTER 600.0                  // together this long on a day counts as a day together

// each player's person, worked out for the name it has (again when that changes)
new g_For[33][32]
new g_Alias[33][32]                  // who they really are while called something else ("Player")
new g_Arch[33], g_Trait[33][T_COUNT], g_Hobby[33][2]

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	register_srvcmd("gm_people", "cmd_people")
	register_srvcmd("gm_people_reload", "cmd_reload")
	register_event("DeathMsg", "ev_death", "a")
	g_Vault = nvault_open("gm_people")
	load_people()
}

public plugin_end()
{
	for (new a = 1; a <= MaxClients; a++)
		save_all(a)
	if (g_Vault != INVALID_HANDLE)
		nvault_close(g_Vault)
}

public client_putinserver(id)
{
	g_JoinedAt[id] = get_gametime()
	for (new j = 1; j <= MaxClients; j++)
	{
		g_MemLoaded[id][j] = g_MemLoaded[j][id] = g_MemDirty[id][j] = g_MemDirty[j][id] = false
		g_NoteSum[id][j] = g_NoteSum[j][id] = 0
	}
}

public client_disconnected(id)
{
	g_Alias[id][0] = 0
	save_all(id)
	for (new j = 1; j <= MaxClients; j++)
		g_MemLoaded[id][j] = g_MemLoaded[j][id] = false
	g_For[id][0] = 0
}

public plugin_natives()
{
	register_library("gm_people")
	register_native("people_match", "n_match")
	register_native("people_letter", "n_letter")
	register_native("people_get", "n_get")
	register_native("people_arch", "n_arch")
	register_native("people_hobby", "n_hobby")
	register_native("people_note", "n_note")
	register_native("people_alias", "n_alias")
}

public cmd_reload()
{
	load_people()
	for (new i = 1; i <= MaxClients; i++)
		g_For[i][0] = 0
	server_print("[gm_people] %d archetypes", g_ArchCount)
	return PLUGIN_HANDLED
}

// ---- people.ini -------------------------------------------------------------------------

load_people()
{
	g_ArchCount = g_ArchTotal = g_ConfCount = 0
	for (new t = 0; t < T_COUNT; t++)
		g_WeightCount[t] = 0
	new path[160]
	get_configsdir(path, charsmax(path))
	add(path, charsmax(path), "/gamemodes/people.ini")
	new f = fopen(path, "rt")
	if (!f)
	{
		log_amx("[gm_people] %s is missing: bots are plain players", path)
		return
	}
	new line[256], section[16]
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';')
			continue
		if (line[0] == '[')
		{
			copyc(section, charsmax(section), line[1], ']')
			continue
		}
		if (equal(section, "archetypes"))
			read_archetype(line)
		else if (equal(section, "conflicts"))
			read_conflict(line)
		else if (equal(section, "weights"))
			read_weights(line)
	}
	fclose(f)
}

read_archetype(const line[])
{
	if (g_ArchCount >= MAX_ARCH)
		return
	new name[32], weight[8], letter[4], style[16], fixed[160]
	new pos = split_field(line, 0, name, charsmax(name))
	pos = split_field(line, pos, weight, charsmax(weight))
	pos = split_field(line, pos, letter, charsmax(letter))
	pos = split_field(line, pos, style, charsmax(style))
	split_field(line, pos, fixed, charsmax(fixed))
	new a = g_ArchCount
	copy(g_ArchName[a], charsmax(g_ArchName[]), name)
	g_ArchWeight[a] = max(str_to_num(weight), 0)
	g_ArchLetter[a] = letter[0]
	for (new t = 0; t < T_COUNT; t++)
		g_ArchFixed[a][t] = -1
	g_ArchHobby[a] = -1
	if (style[0] && style[0] != '-')
		g_ArchFixed[a][T_STYLE] = value_index(TRAIT_VALUES[T_STYLE], style)
	new tok[32], p
	while (next_word(fixed, p, tok, charsmax(tok)))
	{
		new t, v
		if (parse_pair(tok, t, v))
		{
			if (t == T_COUNT)
				g_ArchHobby[a] = v
			else
				g_ArchFixed[a][t] = v
		}
	}
	g_ArchTotal += g_ArchWeight[a]
	g_ArchCount++
}

read_conflict(const line[])
{
	if (g_ConfCount >= MAX_CONFLICTS)
		return
	new a[32], b[32], p
	if (!next_word(line, p, a, charsmax(a)) || !next_word(line, p, b, charsmax(b)))
		return
	new ta, va, tb, vb
	if (!parse_pair(a, ta, va) || !parse_pair(b, tb, vb))
		return
	g_ConfA[g_ConfCount] = ta
	g_ConfAV[g_ConfCount] = va
	g_ConfB[g_ConfCount] = tb
	g_ConfBV[g_ConfCount] = vb
	g_ConfCount++
}

read_weights(const line[])
{
	new name[16], rest[96]
	new pos = split_field(line, 0, name, charsmax(name))
	split_field(line, pos, rest, charsmax(rest))
	new t = trait_index(name)
	if (t < 0)
		return
	new tok[8], p, n
	while (n < MAX_VALUES && next_word(rest, p, tok, charsmax(tok)))
		g_Weights[t][n++] = max(str_to_num(tok), 0)
	g_WeightCount[t] = n
}

// "trait=value" (a value's name or number); trait T_COUNT is a hobby
bool:parse_pair(const tok[], &t, &v)
{
	new eq = contain(tok, "=")
	if (eq <= 0)
		return false
	new key[16], val[24]
	copy(key, min(eq, charsmax(key)), tok)
	copy(val, charsmax(val), tok[eq + 1])
	if (equal(key, "hobby"))
	{
		t = T_COUNT
		v = value_index(HOBBIES, val)
		return v >= 0
	}
	t = trait_index(key)
	if (t < 0)
		return false
	v = value_index(TRAIT_VALUES[t], val)
	return v >= 0
}

trait_index(const name[])
{
	for (new t = 0; t < T_COUNT; t++)
		if (equali(TRAIT_NAME[t], name))
			return t
	return -1
}

// A value's place in a space-separated list, by name or as a number; -1 if neither
value_index(const list[], const val[])
{
	if (val[0] >= '0' && val[0] <= '9')
		return str_to_num(val)
	new tok[16], p, i
	while (next_word(list, p, tok, charsmax(tok)))
	{
		if (equali(tok, val))
			return i
		i++
	}
	return -1
}

// The field from `pos` up to the next | (returns where the next one starts)
split_field(const src[], pos, out[], len)
{
	new n
	while (src[pos] && src[pos] != '|')
	{
		if (n < len)
			out[n++] = src[pos]
		pos++
	}
	out[n] = 0
	trim(out)
	return src[pos] ? pos + 1 : pos
}

bool:next_word(const src[], &pos, out[], len)
{
	while (src[pos] == ' ' || src[pos] == '^t')
		pos++
	if (!src[pos])
		return false
	new n
	while (src[pos] && src[pos] != ' ' && src[pos] != '^t')
	{
		if (n < len)
			out[n++] = src[pos]
		pos++
	}
	out[n] = 0
	return true
}

// ---- who someone is ---------------------------------------------------------------------

// A small random sequence of its own, from the name
next_rand(&rnd)
{
	rnd = (rnd * 1103515245 + 12345) & 0x7FFFFFFF
	return rnd >> 8
}

name_seed(const name[])
{
	new h = 2166136261
	for (new i = 0; name[i]; i++)
		h = (h ^ name[i]) * 16777619
	return h & 0x7FFFFFFF
}

// Works out (or reuses) who this player is, for the name it has now
bool:person(id)
{
	if (id < 1 || id > MaxClients || !is_user_connected(id) || !is_user_bot(id) || !g_ArchCount)
		return false
	new name[32]
	if (g_Alias[id][0])
		copy(name, charsmax(name), g_Alias[id])
	else
		get_user_name(id, name, charsmax(name))
	if (equal(g_For[id], name))
		return true
	// a new name is a new person: what the old one remembered is saved under the old name
	if (g_For[id][0])
	{
		save_all(id)
		for (new j = 1; j <= MaxClients; j++)
			g_MemLoaded[id][j] = g_MemLoaded[j][id] = false
	}
	copy(g_For[id], charsmax(g_For[]), name)
	// the name without a clan tag: the same person, whatever clan they're in now
	new bare[32]
	bare_name(name, bare, charsmax(bare))
	new seed = name_seed(bare)
	new rnd = seed

	// the archetype
	new r = next_rand(rnd) % max(g_ArchTotal, 1), a
	for (a = 0; a < g_ArchCount - 1; a++)
	{
		if (r < g_ArchWeight[a])
			break
		r -= g_ArchWeight[a]
	}
	g_Arch[id] = a

	// the traits: fixed by the archetype, else rolled; conflicts rolled again
	for (new t = 0; t < T_COUNT; t++)
		g_Trait[id][t] = g_ArchFixed[a][t] >= 0 ? g_ArchFixed[a][t] : roll(t, rnd)
	g_Hobby[id][0] = g_ArchHobby[a] >= 0 ? g_ArchHobby[a] : next_rand(rnd) % HOBBY_COUNT
	g_Hobby[id][1] = next_rand(rnd) % HOBBY_COUNT
	if (g_Hobby[id][1] == g_Hobby[id][0])
		g_Hobby[id][1] = (g_Hobby[id][1] + 1 + next_rand(rnd) % (HOBBY_COUNT - 1)) % HOBBY_COUNT
	for (new tries = 0; tries < 12; tries++)
	{
		new c = conflict(id)
		if (c < 0)
			break
		// roll again whichever side the archetype didn't fix
		new t = g_ConfB[c], other = g_ConfA[c]
		if (t == T_COUNT || g_ArchFixed[a][t] >= 0)
		{
			t = other
			if (t == T_COUNT || g_ArchFixed[a][t] >= 0)
			{
				// both fixed, or a hobby: change a hobby instead (the first too, every other
				// try, unless the archetype fixed it: it may be the one in the way)
				new h = (tries & 1) && g_ArchHobby[a] < 0 ? 0 : 1
				g_Hobby[id][h] = (g_Hobby[id][h] + 1) % HOBBY_COUNT
				if (g_Hobby[id][h] == g_Hobby[id][1 - h])
					g_Hobby[id][h] = (g_Hobby[id][h] + 1) % HOBBY_COUNT
				continue
			}
		}
		g_Trait[id][t] = roll(t, rnd)
	}
	return true
}

// Clan tags off: "[TAG]Name" and "-=TAG=-Name" are both Name
bare_name(const name[], out[], len)
{
	copy(out, len, name)
	new close = -1
	for (new i = 0; out[i]; i++)
		if (out[i] == ']' || out[i] == '}' || out[i] == ')' || (out[i] == '-' && i > 0 && out[i - 1] == '='))
		{
			close = i
			break
		}
	if (close >= 0 && close < 12 && out[close + 1])
		copy(out, len, out[close + 1])
	// (a bar is left alone: "GG | Glacier" and "Paprika | aXe" put the tag on either side,
	// and cutting at it would make a whole clan one person)
	trim(out)
}

roll(t, &rnd)
{
	new n = g_WeightCount[t], total
	if (!n)
		return 0
	for (new i = 0; i < n; i++)
		total += g_Weights[t][i]
	if (total <= 0)
		return 0
	new r = next_rand(rnd) % total
	for (new i = 0; i < n; i++)
	{
		if (r < g_Weights[t][i])
			return i
		r -= g_Weights[t][i]
	}
	return n - 1
}

// The first conflict this person has, or -1
conflict(id)
{
	for (new c = 0; c < g_ConfCount; c++)
		if (has_value(id, g_ConfA[c], g_ConfAV[c]) && has_value(id, g_ConfB[c], g_ConfBV[c]))
			return c
	return -1
}

bool:has_value(id, t, v)
{
	if (t == T_COUNT)
		return g_Hobby[id][0] == v || g_Hobby[id][1] == v
	return g_Trait[id][t] == v
}

// ---- natives -----------------------------------------------------------------------------

// people_match(id, other, key[], op[], value[]): the conditions lines use
public n_match(plugin, params)
{
	new id = get_param(1)
	new key[24], op[4], value[32]
	get_string(3, key, charsmax(key))
	get_string(4, op, charsmax(op))
	get_string(5, value, charsmax(value))
	if (!person(id))
		return -1
	new negate = op[0] == '!' ? 1 : 0
	if (equal(key, "arch"))
		return (equali(g_ArchName[g_Arch[id]], value) ? 1 : 0) != negate ? 1 : 0
	if (equal(key, "hobby"))
	{
		new v = value_index(HOBBIES, value)
		new has = (v >= 0 && (g_Hobby[id][0] == v || g_Hobby[id][1] == v)) ? 1 : 0
		return has != negate ? 1 : 0
	}
	if (equal(key, "rel") || equal(key, "fam") || equal(key, "days") || equal(key, "rival"))
	{
		new other = get_param(2)
		if (!memory(id, other))
			return -1
		new have
		if (equal(key, "rel"))
			have = toward_zero(g_Aff[id][other], 10)
		else if (equal(key, "fam"))
			have = g_Fam[id][other]
		else if (equal(key, "days"))
			have = g_PrevLast[id][other] ? today() - g_PrevLast[id][other] : 9999
		else
			have = (g_Aff[id][other] <= -20 && g_KillsOn[id][other] + g_DeathsBy[id][other] >= 10) ? 1 : 0
		return compare(have, op, str_to_num(value))
	}
	new t = trait_index(key)
	if (t < 0)
		return -1
	new have = g_Trait[id][t], want = value_index(TRAIT_VALUES[t], value)
	if (want < 0)
		return 0
	return compare(have, op, want)
}

compare(have, const op[], want)
{
	new bool:ok
	switch (op[0])
	{
		case '>': ok = op[1] == '=' ? have >= want : have > want
		case '<': ok = op[1] == '=' ? have <= want : have < want
		case '!': ok = have != want
		default: ok = have == want
	}
	return ok ? 1 : 0
}

public n_letter(plugin, params)
{
	new id = get_param(1)
	return person(id) ? g_ArchLetter[g_Arch[id]] : 0
}

public n_get(plugin, params)
{
	new id = get_param(1)
	new key[24]
	get_string(2, key, charsmax(key))
	if (!person(id))
		return -1
	if (equal(key, "hobby1"))
		return g_Hobby[id][0]
	if (equal(key, "hobby2"))
		return g_Hobby[id][1]
	new t = trait_index(key)
	return t < 0 ? -1 : g_Trait[id][t]
}

public n_arch(plugin, params)
{
	new id = get_param(1)
	if (!person(id))
		return set_string(2, "", get_param(3))
	return set_string(2, g_ArchName[g_Arch[id]], get_param(3))
}

// people_hobby(id, which, out[], len): one of its two hobbies, by name
public n_hobby(plugin, params)
{
	new id = get_param(1), which = clamp(get_param(2), 0, 1)
	new out[16]
	if (person(id))
		value_name(HOBBIES, g_Hobby[id][which], out, charsmax(out))
	return set_string(3, out, get_param(4))
}

// ---- memory ------------------------------------------------------------------------------

today()
{
	return get_systime() / 86400
}

// How a player is known: a person by their Steam ID (their name changes), a bot by its name
// without the clan tag
memory_name(id, out[], len)
{
	if (!is_user_bot(id))
	{
		get_user_authid(id, out, len)
		if (equal(out, "STEAM_", 6) && !equal(out, "STEAM_ID_", 9))
			return
	}
	new name[32]
	if (is_user_bot(id) && g_For[id][0])
		copy(name, charsmax(name), g_For[id])
	else
		get_user_name(id, name, charsmax(name))
	bare_name(name, out, len)
}

// Loads what bot a remembers about b (true when there's something to go on)
bool:memory(a, b)
{
	if (a < 1 || a > MaxClients || b < 1 || b > MaxClients || a == b || g_Vault == INVALID_HANDLE)
		return false
	if (!is_user_connected(a) || !is_user_bot(a) || !is_user_connected(b))
		return false
	if (g_MemLoaded[a][b])
		return true
	g_MemLoaded[a][b] = true
	g_MemDirty[a][b] = false
	g_Aff[a][b] = g_Fam[a][b] = g_Last[a][b] = g_PrevLast[a][b] = g_KillsOn[a][b] = g_DeathsBy[a][b] = 0
	new key[80], val[64], ts
	memory_key(a, b, key, charsmax(key))
	if (!nvault_lookup(g_Vault, key, val, charsmax(val), ts))
		return true
	new f[5][12]
	parse(val, f[0], 11, f[1], 11, f[2], 11, f[3], 11, f[4], 11)
	g_Aff[a][b] = str_to_num(f[0])
	g_Fam[a][b] = str_to_num(f[1])
	g_Last[a][b] = g_PrevLast[a][b] = str_to_num(f[2])
	g_KillsOn[a][b] = str_to_num(f[3])
	g_DeathsBy[a][b] = str_to_num(f[4])
	// feelings fade toward neutral: a tenth a week since this pair was last saved (the
	// vault's own stamp, so a faded value saved again doesn't fade twice)
	new weeks = (get_systime() - ts) / (7 * 86400)
	for (new w = 0; w < weeks && w < 60 && g_Aff[a][b]; w++)
		g_Aff[a][b] = toward_zero(g_Aff[a][b] * 9, 10)
	return true
}

// Divides rounding toward zero (Pawn rounds down: -2 / 10 would be -1, and negative
// feelings would never fade)
toward_zero(n, d)
{
	return n < 0 ? -((-n) / d) : n / d
}

memory_key(a, b, key[], len)
{
	new na[32], nb[40]
	memory_name(a, na, charsmax(na))
	memory_name(b, nb, charsmax(nb))
	formatex(key, len, "%s|%s", na, nb)
}

save_pair(a, b)
{
	if (!g_MemLoaded[a][b] || g_Vault == INVALID_HANDLE)
		return
	// a day together counts once they've played a while
	new Float:now = get_gametime()
	if (g_Last[a][b] != today() && now - floatmax(g_JoinedAt[a], g_JoinedAt[b]) >= FAM_AFTER)
	{
		g_Fam[a][b]++
		g_Last[a][b] = today()
		g_MemDirty[a][b] = true
	}
	if (!g_MemDirty[a][b])
		return
	new key[80], val[64]
	memory_key(a, b, key, charsmax(key))
	formatex(val, charsmax(val), "%d %d %d %d %d", g_Aff[a][b], g_Fam[a][b], g_Last[a][b], g_KillsOn[a][b], g_DeathsBy[a][b])
	nvault_set(g_Vault, key, val)
	g_MemDirty[a][b] = false
}

save_all(id)
{
	for (new j = 1; j <= MaxClients; j++)
	{
		if (g_MemLoaded[id][j])
			save_pair(id, j)
		if (g_MemLoaded[j][id])
			save_pair(j, id)
	}
}

// How bot a's feelings about b change (tenths, kept in -50..50)
feel(a, b, change)
{
	if (!memory(a, b))
		return
	g_Aff[a][b] = clamp(g_Aff[a][b] + change, -50, 50)
	g_MemDirty[a][b] = true
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2), hs = read_data(3)
	new weapon[16]
	read_data(4, weapon, charsmax(weapon))
	if (killer < 1 || killer > MaxClients || killer == victim || !is_user_connected(killer) || !is_user_connected(victim))
		return
	new bool:team = get_user_team(killer) == get_user_team(victim)
	// the bot that died: about whoever killed it (a teamkill most of all)
	if (is_user_bot(victim) && memory(victim, killer))
	{
		g_DeathsBy[victim][killer]++
		new change = team ? -10 : -2
		if (hs)
			change -= 1
		if (equal(weapon, "knife"))
			change -= 3
		feel(victim, killer, change)
	}
	// the bot that killed: it remembers having the better of them
	if (is_user_bot(killer) && memory(killer, victim))
	{
		g_KillsOn[killer][victim]++
		g_MemDirty[killer][victim] = true
	}
}

// people_alias(id, name[]): who this bot really is while it goes by another name (a newcomer
// still called "Player"); "" to stop
public n_alias(plugin, params)
{
	new id = get_param(1)
	if (id < 1 || id > MaxClients)
		return 0
	get_string(2, g_Alias[id], charsmax(g_Alias[]))
	return 1
}

// people_note(bot, other, kind[]): what was said or done changes how the bot feels
public n_note(plugin, params)
{
	new bot = get_param(1), other = get_param(2)
	if (bot < 1 || bot > MaxClients || other < 1 || other > MaxClients)
		return 0
	new kind[16]
	get_string(3, kind, charsmax(kind))
	static const KINDS[][] = { "praise", "thanks", "greet", "joke", "saved", "insult", "accuse" }
	static const CHANGE[] = { 3, 2, 1, 1, 4, -3, -2 }
	for (new i = 0; i < sizeof KINDS; i++)
		if (equal(kind, KINDS[i]))
		{
			new change = clamp(g_NoteSum[bot][other] + CHANGE[i], -NOTE_CAP, NOTE_CAP) - g_NoteSum[bot][other]
			if (change)
			{
				g_NoteSum[bot][other] += change
				feel(bot, other, change)
			}
			return 1
		}
	return 0
}

// ---- console -----------------------------------------------------------------------------

// gm_people [name or #userid]: who the bots are (all of them without an argument)
public cmd_people()
{
	new arg[32]
	read_argv(1, arg, charsmax(arg))
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!person(id))
			continue
		new name[32]
		get_user_name(id, name, charsmax(name))
		if (arg[0] && containi(name, arg) == -1 && !(arg[0] == '#' && str_to_num(arg[1]) == get_user_userid(id)))
			continue
		new out[256], val[16]
		formatex(out, charsmax(out), "%s: %s (%c)", name, g_ArchName[g_Arch[id]], g_ArchLetter[g_Arch[id]])
		for (new t = 0; t < T_COUNT; t++)
		{
			value_name(TRAIT_VALUES[t], g_Trait[id][t], val, charsmax(val))
			add(out, charsmax(out), fmt(" %s=%s", TRAIT_NAME[t], val))
		}
		new h1[16], h2[16]
		value_name(HOBBIES, g_Hobby[id][0], h1, charsmax(h1))
		value_name(HOBBIES, g_Hobby[id][1], h2, charsmax(h2))
		add(out, charsmax(out), fmt(" hobbies=%s,%s", h1, h2))
		server_print("%s", out)
	}
	return PLUGIN_HANDLED
}

value_name(const list[], index, out[], len)
{
	new p, i
	while (next_word(list, p, out, len))
	{
		if (i++ == index)
			return
	}
	formatex(out, len, "%d", index)
}
