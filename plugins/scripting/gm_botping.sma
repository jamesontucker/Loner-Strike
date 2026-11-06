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
	same ping every time it shows up: most sit between 25 and 90 ms, some between
	90 and 160. The number drifts a few ms over time, and now and then a bot gets
	a short spike (+80-200 ms for two or three seconds), like someone's connection
	hiccuping.

	Cvars: gm_botping 0/1, gm_botping_min 20, gm_botping_max 160 (the range the
	base pings come from), gm_botping_spikes 0/1

	The pings message (SVC_PINGS, sent right after the engine's own one while a
	player holds the scoreboard key) is done the way Ping Faker by MeRcyLeZZ does
	it (AlliedModders, GPL): the same bit layout the engine writes, flag 1 bit,
	player slot 5 bits, ping 12 bits, loss 7 bits, then a 0 bit to end the list.
*/

#include <amxmodx>
#include <fakemeta>

#define PLUGIN  "Bot ping"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_TICK 9100
#define TASK_KEY  9200

#define DRIFT_MAX   8        // how far the ping wanders from its base, in ms
#define SPIKE_ODDS  400      // a spike starts with 1 in this chance per bot per second

new g_pOn, g_pMin, g_pMax, g_pSpikes

new g_Shown[33]              // the ping each bot shows right now
new g_Drift[33]              // slow wander around the base
new g_Spike[33], g_SpikeLeft[33]
new bool:g_Cleared[33]       // we took the *bot key off this bot

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pOn     = register_cvar("gm_botping", "1")
	g_pMin    = register_cvar("gm_botping_min", "20")
	g_pMax    = register_cvar("gm_botping_max", "160")
	g_pSpikes = register_cvar("gm_botping_spikes", "1")

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

// A bot's base ping, from its name, within gm_botping_min..max: 80% in the lower half
base_of(id)
{
	new name[32], h = 5381
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h * 33 + name[i]) & 0x7FFFFFFF
	new lo = clamp(get_pcvar_num(g_pMin), 1, 4000)
	new hi = clamp(get_pcvar_num(g_pMax), lo + 1, 4095)
	new mid = lo + (hi - lo) / 2
	if (h % 100 < 80)
		return lo + 5 + (h / 100) % max(1, mid - lo - 5)
	return mid + (h / 100) % max(1, hi - mid + 1)
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
		g_Drift[id] = clamp(g_Drift[id] + random_num(-2, 2), -DRIFT_MAX, DRIFT_MAX)
		if (g_SpikeLeft[id] > 0)
		{
			// a spike starts high and settles back down
			g_SpikeLeft[id]--
			g_Spike[id] = g_SpikeLeft[id] ? g_Spike[id] * 2 / 3 : 0
		}
		else if (get_pcvar_num(g_pSpikes) && random_num(1, SPIKE_ODDS) == 1)
		{
			g_Spike[id] = random_num(80, 200)
			g_SpikeLeft[id] = random_num(2, 4)
		}
		g_Shown[id] = clamp(base_of(id) + g_Drift[id] + g_Spike[id] + random_num(-3, 3), 5, 4095)
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
