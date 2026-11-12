/*
	gm_botping - bots show a believable ping on the scoreboard instead of "BOT".

	The scoreboard writes "BOT" for any player whose userinfo has the "*bot" key,
	which the game sets on every bot it adds. That gives the bots away at a glance.
	This plugin takes the key off each bot again (the game only writes it, it never
	reads it back: bots are found by the FL_FAKECLIENT flag) and then sends every
	player who is looking at the scoreboard a pings message with made-up values
	for the bots. Real players' pings are whatever the engine says they are; the
	engine's own message is sent first and ours only overrides the bot entries.

	Each bot gets its own base ping from its name, so a regular keeps roughly the
	same ping every time it shows up: most sit between 25 and 60 ms, some between
	60 and 95, a few up to about 115 (the one with a bad connection up to 120). The
	number wanders a few ms over time (a slow walk that keeps coming back, wider for the
	higher pings, with a ms or two of flicker), and now and then a bot gets a short spike
	(+20-50 ms for two or three seconds), like someone's connection hiccuping.
	Over 100 is rare; nobody shows more than 135.

	Cvars: gm_botping 0/1, gm_botping_min 20, gm_botping_max 95 (the range most
	base pings come from), gm_botping_spikes 0/1, gm_botping_home us (where the server is:
	us, eu or a country code from gm_people's regions, ru, br, au; bots from there are the
	low pings, the further away the higher, all inside min..max). A LAN party (bot style 9)
	is everyone on one switch: 3-15 ms, no spikes.

	The pings message (SVC_PINGS, sent right after the engine's own one while a
	player holds the scoreboard key) is done the way Ping Faker by MeRcyLeZZ does
	it (AlliedModders, GPL): the same bit layout the engine writes, flag 1 bit,
	player slot 5 bits, ping 12 bits, loss 7 bits, then a 0 bit to end the list.
*/

#include <amxmodx>
#include <fakemeta>

// gm_botchat (optional): the lagger personality ('l') gets the bad connection
native botchat_persona(id)
// gm_people (optional): where the bot plays from (its region)
native people_get(id, const key[])

#define PLUGIN  "Bot ping"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_TICK 9100
#define TASK_KEY  9200

#define DRIFT_MAX   8        // how far the ping wanders from its base, in ms
#define SPIKE_ODDS  400      // a spike starts with 1 in this chance per bot per second
#define PING_CAP    135      // the most any bot shows (a spike too)

new g_pOn, g_pMin, g_pMax, g_pSpikes, g_pHome

new g_Shown[33]              // the ping each bot shows right now
new g_Drift[33]              // slow wander around the base
new g_Spike[33], g_SpikeLeft[33]
new bool:g_Cleared[33]       // we took the *bot key off this bot

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pOn     = register_cvar("gm_botping", "1")
	g_pMin    = register_cvar("gm_botping_min", "20")
	g_pMax    = register_cvar("gm_botping_max", "95")
	g_pSpikes = register_cvar("gm_botping_spikes", "1")
	g_pHome   = register_cvar("gm_botping_home", "us")

	register_forward(FM_UpdateClientData, "fw_UpdateClientData")
	// the scoreboard forgets our pings after these in CS, so send them again
	register_event("DeathMsg", "ev_resend", "a")
	register_event("TeamInfo", "ev_resend", "a")

	set_task(1.0, "task_tick", TASK_TICK, _, _, "b")
}

public client_putinserver(id)
{
	g_Drift[id] = 0
	g_Spike[id] = 0
	g_SpikeLeft[id] = 0
	g_Cleared[id] = false
	if (is_user_bot(id))
	{
		g_Shown[id] = base_of(id)
		// the game writes the *bot key just after putinserver, so wait a moment
		set_task(0.2, "task_key", TASK_KEY + id)
	}
}

public client_disconnected(id)
{
	g_Cleared[id] = false
	remove_task(TASK_KEY + id)
}

public task_key(taskid)
{
	new id = taskid - TASK_KEY
	if (is_user_connected(id) && is_user_bot(id))
		fix_key(id, get_pcvar_num(g_pOn) != 0)
}

// Puts the *bot key back when the plugin is turned off, takes it off when it's on
fix_key(id, bool:on)
{
	new v[4]
	get_user_info(id, "*bot", v, charsmax(v))
	if (on && v[0])
	{
		set_user_info(id, "*bot", "")
		g_Cleared[id] = true
	}
	else if (!on && !v[0] && g_Cleared[id])
	{
		set_user_info(id, "*bot", "1")
		g_Cleared[id] = false
	}
}

// ---- what each bot's ping is --------------------------------------------------------

public plugin_natives()
{
	set_native_filter("native_filter")
}

public native_filter(const name[], index, trap)
{
	return (!trap && (equal(name, "botchat_", 8) || equal(name, "people_", 7))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

bool:lagger(id)
{
	return LibraryExists("gm_botchat", LibType_Library) && botchat_persona(id) == 'l'
}

// A bot's base ping, from its name without its clan tag (a regular keeps it when it changes
// clan): where it plays from first (gm_people's region, against gm_botping_home: the same
// part of the world in the lowest part of min..max, next door a little higher, across an
// ocean higher again, the other side of the world at the top), else 75% low, 20% up to
// gm_botping_max and a few a little beyond; the lagger always beyond. A LAN party is all on
// the same switch (3-15). Never past PING_CAP, and never past a little over gm_botping_max
base_of(id)
{
	new name[32], h = 5381
	get_user_name(id, name, charsmax(name))
	strip_tag(name)
	for (new i = 0; name[i]; i++)
		h = (h * 33 + name[i]) & 0x7FFFFFFF
	if (get_cvar_num("gm_botstyle") == 9)
		return 3 + h % 13
	new lo = clamp(get_pcvar_num(g_pMin), 1, 4000)
	new hi = clamp(get_pcvar_num(g_pMax), lo + 1, PING_CAP - 40)
	new top = min(hi + 20, PING_CAP - 20)
	if (lagger(id))
		return min(hi + 10 + (h / 100) % 16, PING_CAP - 15)   // the one with a bad connection
	new region = LibraryExists("gm_people", LibType_Library) ? people_get(id, "region") : -1
	if (region >= 0)
	{
		// how far: 0 the same part of the world, 1 next door (Europe and Russia), 2 across an
		// ocean (North America and Europe), 3 the other side of the world; as a share of the
		// range lo..top (in percent)
		static const FROM[] = { 0, 20, 45, 70 }
		static const TO[] = { 30, 50, 80, 100 }
		new d = world_apart(home_part(), part_of(region))
		new span = top - lo
		new from = lo + span * FROM[d] / 100, to = lo + span * TO[d] / 100
		return clamp(from + (h / 100) % max(1, to - from + 1), lo, top)
	}
	new mid = lo + (hi - lo) / 2
	// most close by, a tail of people far away: like a real server's scoreboard
	new r = h % 100
	if (r < 75)
		return lo + 5 + (h / 100) % max(1, mid - lo - 5)
	if (r < 95)
		return mid + (h / 100) % max(1, hi - mid + 1)
	return min(hi + (h / 100) % 21, top)                // 95-115 at defaults
}

// The part of the world a gm_people region is in (us uk de se pl ru br fr nl es au fi dk):
// 0 North America, 1 Europe, 2 Russia, 3 South America, 4 Australia
part_of(region)
{
	switch (region)
	{
		case 0: return 0
		case 5: return 2
		case 6: return 3
		case 10: return 4
	}
	return 1
}

// The server's part of the world, from gm_botping_home (us, eu, ru, br, au or a country)
home_part()
{
	new home[8]
	get_pcvar_string(g_pHome, home, charsmax(home))
	strtolower(home)
	if (equal(home, "us") || equal(home, "ca") || equal(home, "na"))
		return 0
	if (equal(home, "ru"))
		return 2
	if (equal(home, "br") || equal(home, "sa"))
		return 3
	if (equal(home, "au") || equal(home, "oc"))
		return 4
	return 1
}

// How far apart two parts of the world are, for a ping: 0 the same, 1 next door (Europe and
// Russia), 2 across an ocean (North America and Europe), 3 the other side of the world
world_apart(a, b)
{
	if (a == b)
		return 0
	if ((a == 1 && b == 2) || (a == 2 && b == 1))
		return 1
	if ((a == 0 && b == 1) || (a == 1 && b == 0))
		return 2
	return 3
}

// The name without a clan tag: "[TAG]Name", "Name[TAG]", "-=TAG=-Name", "{TAG}Name", "TAG|Name"
strip_tag(name[])
{
	new out[32], n, depth
	for (new i = 0; name[i] && n < charsmax(out); i++)
	{
		new c = name[i]
		if (c == '[' || c == '(' || c == '{')
			depth++
		else if ((c == ']' || c == ')' || c == '}') && depth > 0)
			depth--
		else if (!depth)
			out[n++] = c
	}
	out[n] = 0
	// -=TAG=-Name and TAG|Name: what comes after the tag
	new at = contain(out, "=-")
	if (at >= 0)
		copy(out, charsmax(out), out[at + 2])
	at = contain(out, "|")
	if (at >= 0 && out[at + 1])
		copy(out, charsmax(out), out[at + 1])
	trim(out)
	if (out[0])
		copy(name, 31, out)
}

public task_tick()
{
	new bool:on = get_pcvar_num(g_pOn) != 0
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_connected(id) || !is_user_bot(id))
			continue
		fix_key(id, on)
		if (!on)
			continue
		// a slow walk around the base that keeps drifting back to it (wider for a higher ping)
		// (a LAN: a ms or two either way and no spikes; everyone's on the same switch)
		new bool:lan = get_cvar_num("gm_botstyle") == 9
		new base = base_of(id), far = lan ? 1 : DRIFT_MAX + base / 20
		new back = (g_Drift[id] > 0 && random_num(0, 2) == 0) ? -1 : (g_Drift[id] < 0 && random_num(0, 2) == 0) ? 1 : 0
		g_Drift[id] = clamp(g_Drift[id] + random_num(-2, 2) + back, -far, far)
		if (g_SpikeLeft[id] > 0)
		{
			// a spike starts high and settles back down
			g_SpikeLeft[id]--
			g_Spike[id] = g_SpikeLeft[id] ? g_Spike[id] * 2 / 3 : 0
		}
		else if (!lan && get_pcvar_num(g_pSpikes) && random_num(1, lagger(id) ? SPIKE_ODDS / 8 : SPIKE_ODDS) == 1)
		{
			g_Spike[id] = random_num(20, 50)
			g_SpikeLeft[id] = random_num(2, 4)
		}
		g_Shown[id] = clamp(base + g_Drift[id] + g_Spike[id] + random_num(-1, 1), lan ? 1 : 5, PING_CAP)
	}
}

// ---- telling the clients ------------------------------------------------------------

public fw_UpdateClientData(id, sendweapons, cd_handle)
{
	if (!get_pcvar_num(g_pOn) || is_user_bot(id))
		return FMRES_IGNORED
	// the engine only sends pings while the scoreboard key is held; so do we
	if (!(pev(id, pev_button) & IN_SCORE) && !(pev(id, pev_oldbuttons) & IN_SCORE))
		return FMRES_IGNORED
	send_pings(id)
	return FMRES_IGNORED
}

public ev_resend()
{
	if (!get_pcvar_num(g_pOn))
		return
	for (new id = 1; id <= MaxClients; id++)
		if (is_user_connected(id) && !is_user_bot(id) && !is_user_hltv(id)
			&& ((pev(id, pev_button) & IN_SCORE) || (pev(id, pev_oldbuttons) & IN_SCORE)))
			send_pings(id)
}

// One SVC_PINGS message to this player with the bots' pings, laid out as the engine does
send_pings(id)
{
	new bits, count, bool:sending
	for (new bot = 1; bot <= MaxClients; bot++)
	{
		if (!is_user_connected(bot) || !is_user_bot(bot))
			continue
		if (!sending)
		{
			message_begin(MSG_ONE_UNRELIABLE, SVC_PINGS, _, id)
			sending = true
		}
		add_bits(bits, count, 1, 1)             // another entry follows
		add_bits(bits, count, bot - 1, 5)       // the engine counts players from 0
		add_bits(bits, count, g_Shown[bot], 12) // ping
		add_bits(bits, count, 0, 7)             // loss
		write_bytes(bits, count, false)
	}
	if (sending)
	{
		add_bits(bits, count, 0, 1)             // end of the list
		write_bytes(bits, count, true)
		message_end()
	}
}

// Bits go in low end first, the way the engine's bit writer packs them
add_bits(&bits, &count, value, width)
{
	if (width < 1 || width > 32 - count)
		return
	if (value < 0)
		value = 0
	else if (value >= (1 << width))
		value = (1 << width) - 1
	bits |= value << count
	count += width
}

write_bytes(&bits, &count, bool:rest)
{
	while (count >= 8)
	{
		write_byte(bits & 0xFF)
		bits >>>= 8
		count -= 8
	}
	if (rest && count > 0)
	{
		write_byte(bits & 0xFF)
		bits = 0
		count = 0
	}
}
