/*
	gm_botfun - the small, silly things real people do on a public server, done by bots
	now and then, each the way its chat personality (gm_botchat) would.

	Weapons:    flicking between the last two weapons (Q-spam), the silencer on and off,
	            the scope in and out, the Glock's or FAMAS's burst on and off, cycling
	            through the slots, reloading a nearly full magazine after a kill
	Spawn:      jumping out of spawn when the round starts; in freeze time: jumping on
	            the spot, the knife out and back, flicking weapons, the flashlight on and off
	Looking:    (needs Loner-Strike's game DLL, which adds ls_bot) saluting a teammate
	            with the gun (looking up and down at them), looking around at the map,
	            shooting into the air after the round is won

	Each personality has its own leanings (LIKES, DISLIKES, activity), each bot its own
	fidgetiness (most hardly, a few all the time) and a couple of habits of its own, all
	from its name, so a regular keeps them. Nothing happens in a fight, in a match, with
	the Classic bot style, or at gm_personality 0; gm_personality scales how often.

	gm_bf_fun        1 all of it on or off
	gm_bf_weapons    1 weapon fidgeting
	gm_bf_spawn      1 spawn and freeze time habits
	gm_bf_flashlight 1 the flashlight (only where mp_flashlight allows it)
	gm_bf_salute     1 saluting teammates
	gm_bf_look       1 looking around at the map
	gm_bf_celebrate  1 shooting into the air after a won round
*/

#include <amxmodx>
#include <fakemeta>
#include <hamsandwich>
#include <cstrike>

native botchat_persona(id)
native hb_bot_idle(id)

#define PLUGIN  "Bot fun"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define STYLE_CLASSIC 0
#define TASK_QSPAM    8600
#define TASK_SLOTS    8700

// what a bot can do, for its own habits (two per bot)
enum
{
	FUN_QSPAM, FUN_SILENCER, FUN_SCOPE, FUN_BURST, FUN_SLOTS, FUN_RELOAD,
	FUN_SPAWNJUMP, FUN_FREEZEJUMP, FUN_KNIFE, FUN_FLASHLIGHT,
	FUN_SALUTE, FUN_LOOK, FUN_CELEBRATE,
	FUN_COUNT
}

new g_pFun, g_pWeapons, g_pSpawn, g_pFlashlight, g_pSalute, g_pLook, g_pCelebrate
new g_pPersonality, g_pStyle, g_pFlashAllowed
new bool:g_Chat, bool:g_Fork, bool:g_HumanBots, bool:g_BombPlanted

new g_Habit[33][2]                       // its two habits (FUN_*)
new Float:g_Fidget[33]                   // how fidgety it is: most hardly, a few all the time
new Float:g_Busy[33]                     // in a fight until then (fired, hurt)
new Float:g_NextFun[33]                  // when this bot may do something again
new Float:g_NextAny                      // when any bot may (so it's never a crowd at once)
new g_Press[33]                          // buttons to press on its next command
new g_Impulse[33]                        // impulse to send on its next command
new g_JumpsLeft[33], Float:g_NextJump[33]
new Float:g_FireUntil[33]
new g_QLeft[33]
new g_PendWeapon[33]                     // the gun a delayed silencer/scope/burst press is for
new bool:g_Freeze

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)

	g_pFun        = register_cvar("gm_bf_fun", "1")
	g_pWeapons    = register_cvar("gm_bf_weapons", "1")
	g_pSpawn      = register_cvar("gm_bf_spawn", "1")
	g_pFlashlight = register_cvar("gm_bf_flashlight", "1")
	g_pSalute     = register_cvar("gm_bf_salute", "1")
	g_pLook       = register_cvar("gm_bf_look", "1")
	g_pCelebrate  = register_cvar("gm_bf_celebrate", "1")

	register_forward(FM_CmdStart, "fw_cmdstart")
	// RegisterHamPlayer also covers Condition Zero bots (a different class)
	RegisterHamPlayer(Ham_TakeDamage, "fw_takedamage_post", 1)
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)
	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0")
	register_logevent("ev_bomb_planted", 3, "2=Planted_The_Bomb")
	register_logevent("ev_round_start", 2, "1=Round_Start")
	register_event("SendAudio", "ev_t_win", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ct_win", "a", "2&%!MRAD_ctwin")
	register_event("DeathMsg", "ev_death", "a")
	set_task(1.0, "task_think", _, _, _, "b")
}

public plugin_cfg()
{
	g_pPersonality = get_cvar_pointer("gm_personality")
	g_pStyle = get_cvar_pointer("gm_botstyle")
	g_pFlashAllowed = get_cvar_pointer("mp_flashlight")
	g_Chat = bool:LibraryExists("gm_botchat", LibType_Library)
	g_HumanBots = bool:LibraryExists("gm_humanbots", LibType_Library)
	// Loner-Strike's own game DLL: it adds ls_bot (and these cvars with it)
	g_Fork = get_cvar_pointer("bot_ls_routes") != 0
}

public plugin_natives()
{
	set_native_filter("native_filter")
}

public native_filter(const name[], index, trap)
{
	return (!trap && (equal(name, "botchat_", 8) || equal(name, "hb_", 3))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

// ---- who does what, and when ------------------------------------------------------------

persona(id)
{
	return g_Chat ? botchat_persona(id) : 0
}

bool:has(p, const letters[])
{
	return p && contain(letters, fmt("%c", p)) != -1
}

// gm_personality as a factor: 0 nothing at all, 1 normal, 2 a lot
Float:pers()
{
	return g_pPersonality ? float(clamp(get_pcvar_num(g_pPersonality), 0, 200)) / 100.0 : 1.0
}

// Not in a match, nor with the Classic bot style (plain CZ bots), nor at personality 0
bool:fun_allowed()
{
	if (!get_pcvar_num(g_pFun) || pers() <= 0.0)
		return false
	if (g_pStyle && get_pcvar_num(g_pStyle) == STYLE_CLASSIC)
		return false
	new phase = get_cvar_num("gm_scrim_phase")
	return !(phase > 0 && phase < 5)
}

public client_putinserver(id)
{
	g_Busy[id] = 0.0
	g_NextFun[id] = get_gametime() + random_float(20.0, 60.0)
	g_Press[id] = 0
	g_Impulse[id] = 0
	g_JumpsLeft[id] = 0
	g_FireUntil[id] = 0.0
	if (is_user_bot(id))
	{
		// its two habits, from its name (so a regular keeps them)
		new name[32], h = 5381
		get_user_name(id, name, charsmax(name))
		for (new i = 0; name[i]; i++)
			h = ((h * 33) ^ name[i]) & 0x7FFFFFFF
		g_Habit[id][0] = h % FUN_COUNT
		g_Habit[id][1] = (h / FUN_COUNT) % FUN_COUNT
		// like chattiness in gm_botchat: 45% hardly, 30% some, 18% a lot, 7% constantly
		new tier = (h / 997) % 100
		g_Fidget[id] = tier < 45 ? 0.3 : tier < 75 ? 0.8 : tier < 93 ? 1.6 : 3.0
	}
}

// Per personality letter (gm_botchat's): what each thing appeals to (x3) and what it doesn't
// (x0.2), in FUN_* order. Tuned with a simulation of a 20-bot lobby so the common
// personalities (quiet, lurker, chill, friendly...) don't all end up doing the same things
new const LIKES[FUN_COUNT][] = {
	"stkhzmEYe3",    // Q-spam
	"LorBNMG",       // silencer
	"LtEOa",         // scope
	"no",            // burst
	"nkd",           // slots
	"qstnEOuNZ4",    // reload
	"jkhzSe",        // spawn jump
	"xjkzSe",        // freeze time jump
	"cxkS5",         // knife flick
	"njkzmDl",       // flashlight
	"cfjirwBbPY7",   // salute
	"qLcndDiTbHPM64v", // look around
	"fjkhwmOeZ7"     // celebrate
}
new const DISLIKES[FUN_COUNT][] = {
	"qoDdPr6", "", "cn", "", "tEL", "c", "qLotdDrP46", "qLotdDrP46", "", "qLt6", "LstH", "tE", "qLosrdP6"
}

// How much a personality fidgets at all: the quiet and the lurkers hardly, kids and
// jokers all the time
Float:activity(p)
{
	switch (p)
	{
		case 'q': return 0.4
		case '6': return 0.5
		case 'L', 'd', 'D', 'P', '4': return 0.6
		case 'o', 't', 'u', 'H', 'N', 'G': return 0.8
		case 'c', 's', 'i', 'r', 'B', 'b': return 0.9
		case 'O': return 1.1
		case 'n', 'Y', '3': return 1.2
		case 'S', 'e': return 1.3
		case 'm': return 1.4
		case 'h', 'z': return 1.5
		case 'j': return 1.6
		case 'k': return 1.8
	}
	return 1.0
}

// How likely a bot is to do this: its personality, how fidgety it is, its own habits and
// gm_personality
Float:weight(id, what)
{
	new p = persona(id)
	new Float:w = activity(p) * g_Fidget[id]
	if (has(p, LIKES[what]))
		w *= 3.0
	else if (has(p, DISLIKES[what]))
		w *= 0.2
	if (g_Habit[id][0] == what || g_Habit[id][1] == what)
		w *= 4.0
	return w * pers()
}

bool:roll(id, what, Float:base)
{
	return random_float(0.0, 1.0) < floatmin(base * weight(id, what), 0.9)
}

// Free to do something: alive, a bot, not in a fight, no enemy in sight
bool:free_bot(id)
{
	if (!is_user_alive(id) || !is_user_bot(id))
		return false
	new Float:now = get_gametime()
	if (now < g_Busy[id] || g_BombPlanted)
		return false
	// the bomb carrier has somewhere to be once the round is on
	if (!g_Freeze && user_has_weapon(id, CSW_C4))
		return false
	// gm_humanbots has it afk or holding an angle: leave it be
	if (g_HumanBots && hb_bot_idle(id))
		return false
	return !enemy_in_sight(id)
}

bool:enemy_in_sight(id)
{
	new Float:eye[3], Float:other[3], Float:ofs[3]
	pev(id, pev_origin, eye)
	pev(id, pev_view_ofs, ofs)
	eye[2] += ofs[2]
	new team = get_user_team(id)
	new tr = create_tr2()
	new bool:seen
	for (new i = 1; i <= MaxClients && !seen; i++)
	{
		if (i == id || !is_user_alive(i) || get_user_team(i) == team)
			continue
		pev(i, pev_origin, other)
		if (get_distance_f(eye, other) > 3000.0)
			continue
		engfunc(EngFunc_TraceLine, eye, other, IGNORE_MONSTERS, id, tr)
		new Float:frac
		get_tr2(tr, TR_flFraction, frac)
		seen = frac >= 0.97
	}
	free_tr2(tr)
	return seen
}

// ---- the game's events --------------------------------------------------------------------

public fw_takedamage_post(victim, inflictor, attacker, Float:damage, bits)
{
	if (1 <= victim <= MaxClients)
		g_Busy[victim] = get_gametime() + 5.0
	if (1 <= attacker <= MaxClients && attacker != victim)
		g_Busy[attacker] = get_gametime() + 4.0
}

public fw_spawn_post(id)
{
	if (!is_user_alive(id))
		return
	g_Press[id] = 0
	g_Impulse[id] = 0
	g_JumpsLeft[id] = 0
	g_QLeft[id] = 0
	g_FireUntil[id] = 0.0
	g_PendWeapon[id] = 0
}

public ev_bomb_planted()
{
	g_BombPlanted = true
}

public ev_death()
{
	new killer = read_data(1)
	if (!(1 <= killer <= MaxClients) || killer == read_data(2) || !is_user_bot(killer))
		return
	// after a kill, once it's quiet again: a reload it doesn't need, or a weapon flick
	if (!fun_allowed() || !get_pcvar_num(g_pWeapons))
		return
	if (roll(killer, FUN_RELOAD, 0.15))
		set_task(random_float(0.6, 1.4), "task_reload", killer)
	else if (roll(killer, FUN_QSPAM, 0.06))
		set_task(random_float(0.8, 2.0), "task_start_qspam", killer)
}

// Freeze time starts (before the dead respawn: the rolls wait a moment)
public ev_new_round()
{
	g_Freeze = true
	g_BombPlanted = false
	set_task(0.3, "task_freeze_rolls")
}

public task_freeze_rolls()
{
	if (!fun_allowed() || !get_pcvar_num(g_pSpawn))
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		new Float:delay = random_float(0.5, 3.5)
		if (roll(id, FUN_FREEZEJUMP, 0.07))
			set_task(delay, "task_freeze_jump", id)
		else if (roll(id, FUN_KNIFE, 0.07))
			set_task(delay, "task_knife_flick", id)
		else if (roll(id, FUN_QSPAM, 0.06))
			set_task(delay, "task_start_qspam", id)
		else if (get_pcvar_num(g_pFlashlight) && flashlight_allowed() && roll(id, FUN_FLASHLIGHT, 0.05))
			set_task(delay, "task_flashlight", id)
		else if (g_Fork && get_pcvar_num(g_pSalute) && roll(id, FUN_SALUTE, 0.06))
			set_task(delay, "task_salute", id)
	}
}

// Freeze time is over: some jump their way out of spawn
public ev_round_start()
{
	g_Freeze = false
	if (!fun_allowed() || !get_pcvar_num(g_pSpawn))
		return
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (is_user_alive(id) && is_user_bot(id) && roll(id, FUN_SPAWNJUMP, 0.08))
		{
			g_JumpsLeft[id] = random_num(2, 6)
			g_NextJump[id] = now + random_float(0.2, 0.8)
		}
	}
}

public ev_t_win()
{
	celebrate(1)
}

public ev_ct_win()
{
	celebrate(2)
}

// The winners: a few shoot into the air
celebrate(team)
{
	// the round is over: the bomb doesn't matter any more
	g_BombPlanted = false
	if (!g_Fork || !fun_allowed() || !get_pcvar_num(g_pCelebrate))
		return
	new n
	for (new id = 1; id <= MaxClients && n < 3; id++)
	{
		if (get_user_team(id) != team || !free_bot(id) || !roll(id, FUN_CELEBRATE, 0.12))
			continue
		n++
		set_task(random_float(0.3, 1.5), "task_celebrate", id)
	}
}

// ---- now and then, during the round -----------------------------------------------------

public task_think()
{
	if (!fun_allowed() || g_Freeze)
		return
	new Float:now = get_gametime()
	if (now < g_NextAny)
		return
	// one bot at a time, a few seconds apart (more often at higher gm_personality)
	new list[32], n
	for (new id = 1; id <= MaxClients; id++)
		if (now >= g_NextFun[id] && free_bot(id))
			list[n++] = id
	if (!n)
		return
	new id = list[random(n)]
	g_NextAny = now + random_float(3.0, 8.0) / floatmax(pers(), 0.25)
	g_NextFun[id] = now + random_float(25.0, 60.0)

	if (get_pcvar_num(g_pWeapons))
	{
		new weapon = get_user_weapon(id)
		switch (weapon)
		{
			case CSW_M4A1, CSW_USP: if (roll(id, FUN_SILENCER, 0.12)) { silencer(id); return; }
			case CSW_AWP, CSW_SCOUT, CSW_SG550, CSW_G3SG1: if (roll(id, FUN_SCOPE, 0.15)) { scope(id); return; }
			case CSW_GLOCK18, CSW_FAMAS: if (roll(id, FUN_BURST, 0.12)) { burst(id); return; }
		}
		if (roll(id, FUN_QSPAM, 0.06)) { start_qspam(id); return; }
		if (roll(id, FUN_SLOTS, 0.04)) { start_slots(id); return; }
	}
	if (g_Fork && get_pcvar_num(g_pLook) && roll(id, FUN_LOOK, 0.05))
	{
		look_around(id)
		return
	}
	if (get_pcvar_num(g_pFlashlight) && flashlight_allowed() && roll(id, FUN_FLASHLIGHT, 0.02))
		task_flashlight(id)
}

// ---- the things themselves ----------------------------------------------------------------

public fw_cmdstart(id, uc, seed)
{
	if (!is_user_bot(id) || !is_user_alive(id))
		return FMRES_IGNORED

	new buttons = get_uc(uc, UC_Buttons)
	if (buttons & IN_ATTACK && get_gametime() >= g_FireUntil[id])
		g_Busy[id] = floatmax(g_Busy[id], get_gametime() + 3.0)
	// using something (defusing, a hostage): busy
	if (buttons & IN_USE)
		g_Busy[id] = floatmax(g_Busy[id], get_gametime() + 3.0)

	new changed
	if (g_Press[id])
	{
		buttons |= g_Press[id]
		g_Press[id] = 0
		changed = 1
	}
	if (g_JumpsLeft[id] > 0 && get_gametime() >= g_NextJump[id])
	{
		// stop at once if a fight starts
		if (get_gametime() < g_Busy[id])
			g_JumpsLeft[id] = 0
		else if (pev(id, pev_flags) & FL_ONGROUND)
		{
			buttons |= IN_JUMP
			g_JumpsLeft[id]--
			g_NextJump[id] = get_gametime() + random_float(0.55, 0.95)
			changed = 1
		}
	}
	if (get_gametime() < g_FireUntil[id])
	{
		// only while it's really looking up into the air with a gun: anything else (the look
		// was refused, a fight, a grenade in hand) and it doesn't fire
		new Float:view[3]
		pev(id, pev_v_angle, view)
		if (get_gametime() < g_Busy[id] || !holding_gun(id))
			g_FireUntil[id] = 0.0
		else if (view[0] < -35.0)
		{
			// a short burst every so often, not the whole magazine
			if (floatround(get_gametime() * 8.0) % 3 != 0)
				buttons |= IN_ATTACK
			else
				buttons &= ~IN_ATTACK
			changed = 1
		}
	}
	if (changed)
		set_uc(uc, UC_Buttons, buttons)
	if (g_Impulse[id])
	{
		set_uc(uc, UC_Impulse, g_Impulse[id])
		g_Impulse[id] = 0
	}
	return changed ? FMRES_HANDLED : FMRES_IGNORED
}

silencer(id)
{
	g_PendWeapon[id] = get_user_weapon(id)
	g_Press[id] |= IN_ATTACK2
	// and back again now and then, like someone who can't decide
	if (random_num(0, 1))
		set_task(random_float(2.4, 3.5), "task_attack2", id)
}

scope(id)
{
	g_PendWeapon[id] = get_user_weapon(id)
	// in, (further in,) and out again
	new presses = get_user_weapon(id) == CSW_AWP || get_user_weapon(id) == CSW_SCOUT ? 3 : 2
	g_Press[id] |= IN_ATTACK2
	for (new i = 1; i < presses; i++)
		set_task(0.35 * i + random_float(0.0, 0.25), "task_attack2", id)
}

burst(id)
{
	g_PendWeapon[id] = get_user_weapon(id)
	g_Press[id] |= IN_ATTACK2
	if (random_num(0, 2))
		set_task(random_float(0.6, 1.5), "task_attack2", id)
}

public task_attack2(id)
{
	if (free_bot(id) && get_user_weapon(id) == g_PendWeapon[id])
		g_Press[id] |= IN_ATTACK2
}

public task_reload(id)
{
	if (!free_bot(id))
		return
	new clip, ammo
	get_user_ammo(id, get_user_weapon(id), clip, ammo)
	if (clip > 0 && ammo > 0)
		g_Press[id] |= IN_RELOAD
}

public task_start_qspam(id)
{
	start_qspam(id)
}

start_qspam(id)
{
	if (!free_bot(id))
		return
	// mostly an even number of flicks (it ends on the gun it started with)
	g_QLeft[id] = random_num(2, 6)
	if (random_num(0, 3))
		g_QLeft[id] &= ~1
	task_qspam(id + TASK_QSPAM)
}

public task_qspam(tid)
{
	new id = tid - TASK_QSPAM
	if (g_QLeft[id] <= 0 || !free_bot(id))
		return
	engclient_cmd(id, "lastinv")
	if (--g_QLeft[id] > 0)
		set_task(random_float(0.12, 0.28), "task_qspam", tid)
}

start_slots(id)
{
	if (!free_bot(id))
		return
	// knife, pistol, grenade, back to the gun
	static const SLOTS[][] = { "weapon_knife", "slot2", "slot4", "slot1" }
	for (new i = 0; i < sizeof SLOTS; i++)
	{
		new data[24]
		copy(data, charsmax(data), SLOTS[i])
		set_task(0.35 * i + random_float(0.0, 0.2), "task_slot", id + TASK_SLOTS, data, sizeof data)
	}
}

public task_slot(const data[], tid)
{
	new id = tid - TASK_SLOTS
	if (free_bot(id))
		engclient_cmd(id, data)
}

public task_knife_flick(id)
{
	if (!free_bot(id))
		return
	engclient_cmd(id, "weapon_knife")
	set_task(random_float(0.8, 2.5), "task_lastinv", id)
}

public task_lastinv(id)
{
	if (free_bot(id))
		engclient_cmd(id, "lastinv")
}

public task_freeze_jump(id)
{
	if (!free_bot(id))
		return
	g_JumpsLeft[id] = random_num(1, 4)
	g_NextJump[id] = get_gametime()
}

bool:flashlight_allowed()
{
	return !g_pFlashAllowed || get_pcvar_num(g_pFlashAllowed) != 0
}

public task_flashlight(id)
{
	if (!free_bot(id))
		return
	// on and off a few times (an even number: it ends off)
	new times = 2 * random_num(1, 3)
	g_Impulse[id] = 100
	for (new i = 1; i < times; i++)
		set_task(0.25 * i + random_float(0.0, 0.15), "task_flash_once", id)
}

public task_flash_once(id)
{
	if (is_user_alive(id) && is_user_bot(id))
		g_Impulse[id] = 100
}

// Looks at a teammate close by and moves the gun up and down at them
public task_salute(id)
{
	if (!g_Fork || !free_bot(id))
		return
	new Float:me[3], Float:other[3], best, Float:bestDist = 600.0
	pev(id, pev_origin, me)
	new team = get_user_team(id)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == id || !is_user_alive(i) || get_user_team(i) != team)
			continue
		pev(i, pev_origin, other)
		new Float:d = get_distance_f(me, other)
		// people more than bots (it's mostly for them)
		if (!is_user_bot(i))
			d *= 0.5
		if (d < bestDist)
		{
			bestDist = d
			best = i
		}
	}
	if (!best)
		return
	pev(best, pev_origin, other)
	other[2] += 16.0
	server_cmd("ls_bot %d nod %.0f %.0f %.0f %.1f %.2f", id, other[0], other[1], other[2], random_float(1.2, 2.4), random_float(0.22, 0.38))
	server_cmd("ls_bot %d hold %.1f", id, random_float(1.0, 2.0))
}

// Stops for a moment and looks at something: up at the sky or a roof, or off to the side
look_around(id)
{
	new Float:me[3], Float:at[3], Float:view[3]
	pev(id, pev_origin, me)
	pev(id, pev_v_angle, view)
	new Float:yaw = view[1] + random_float(-120.0, 120.0)
	new Float:dist = random_float(200.0, 600.0)
	at[0] = me[0] + floatcos(yaw, degrees) * dist
	at[1] = me[1] + floatsin(yaw, degrees) * dist
	at[2] = me[2] + (random_num(0, 2) ? random_float(-20.0, 80.0) : random_float(200.0, 500.0))
	new Float:secs = random_float(1.0, 2.5)
	server_cmd("ls_bot %d look %.0f %.0f %.0f %.1f", id, at[0], at[1], at[2], secs)
	server_cmd("ls_bot %d hold %.1f", id, secs * random_float(0.6, 1.0))
}

public task_celebrate(id)
{
	if (!g_Fork || !free_bot(id))
		return
	// the knife can't shoot: the gun out first
	if (get_user_weapon(id) == CSW_KNIFE)
		engclient_cmd(id, "lastinv")
	new Float:me[3]
	pev(id, pev_origin, me)
	me[2] += 600.0
	me[0] += random_float(-120.0, 120.0)
	me[1] += random_float(-120.0, 120.0)
	server_cmd("ls_bot %d look %.0f %.0f %.0f 2.5", id, me[0], me[1], me[2])
	set_task(0.5, "task_fire", id)
}

public task_fire(id)
{
	if (free_bot(id) && holding_gun(id))
		g_FireUntil[id] = get_gametime() + random_float(0.6, 1.4)
}

// A gun in hand: a primary or a pistol (not the knife, a grenade or the bomb)
bool:holding_gun(id)
{
	switch (get_user_weapon(id))
	{
		case CSW_KNIFE, CSW_HEGRENADE, CSW_FLASHBANG, CSW_SMOKEGRENADE, CSW_C4, 0: return false
	}
	return true
}
