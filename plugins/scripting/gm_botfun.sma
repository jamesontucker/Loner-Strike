/*
	gm_botfun - the small, silly things real people do on a public server, done by bots
	now and then, each the way its chat personality (gm_botchat) would.

	Weapons:    flicking between the last two weapons (Q-spam), the silencer on and off,
	            the scope in and out, the Glock's or FAMAS's burst on and off, cycling
	            through the slots, reloading a nearly full magazine after a kill
	Spawn:      jumping out of spawn when the round starts; in freeze time: jumping on
	            the spot, the knife out and back, flicking weapons, the flashlight on and off
	            (in the round too on a dark map)
	Looking:    (needs Loner-Strike's game DLL, which adds ls_bot) saluting a teammate
	            with the gun (looking up and down at them), looking around at the map,
	            shooting into the air after the round is won

	Each personality has its own leanings (LIKES, DISLIKES, activity), each bot its own
	fidgetiness (most hardly, a few all the time) and a couple of habits of its own, all
	from its name, so a regular keeps them. Nothing happens in a fight, in a match, with
	the Classic bot style, or at gm_personality 0; gm_personality scales how often.
	A bot fidgets only once it has been quiet 8-10 seconds: no enemy in sight, not hurt,
	no shots of its own, no gunfire or someone hurt near it. Whatever it was doing stops
	the moment that changes (a scope or flashlight is put back as it was).

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
// task ids: each kind its own range (id + offset), so one never cancels another
#define TASK_QSPAM    8600
#define TASK_SLOTS    8700
#define TASK_KILL     8800
#define TASK_FREEZE   8900
#define TASK_KNIFE    9000
#define TASK_ATTACK2  9100
#define TASK_FLASH    9200
#define TASK_CELEB    9300
#define TASK_SCOPE    9400
#define TASK_THINK    9500

// what a bot can do, for its own habits (two per bot)
enum
{
	FUN_QSPAM, FUN_SILENCER, FUN_SCOPE, FUN_BURST, FUN_SLOTS, FUN_RELOAD,
	FUN_SPAWNJUMP, FUN_FREEZEJUMP, FUN_KNIFE, FUN_FLASHLIGHT,
	FUN_SALUTE, FUN_LOOK, FUN_CELEBRATE,
	FUN_COUNT
}

new g_pFun, g_pWeapons, g_pSpawn, g_pFlashlight, g_pSalute, g_pLook, g_pCelebrate
new g_pPersonality, g_pStyle, g_pFlashAllowed, g_pFreeForAll, g_pScrim
new bool:g_Chat, bool:g_Fork, bool:g_HumanBots, bool:g_BombPlanted, bool:g_DarkMap
new bool:g_FunOn                         // fun_allowed(), checked now and then (not every frame)

new g_Habit[33][2]                       // its two habits (FUN_*)
new Float:g_Fidget[33]                   // how fidgety it is: most hardly, a few all the time
new Float:g_Busy[33]                     // danger until then (hurt, shots or someone hurt close by)
new Float:g_FireBusy[33]                 // its own fight until then (firing, hitting, using)
new Float:g_SeenAt[33]                   // last time it saw an enemy or got hurt
new Float:g_Quiet[33]                    // how long it must be quiet before fidgeting (8-10 s)
new bool:g_Sight[33], Float:g_SightAt[33]   // enemy in sight, and when to look again
new Float:g_KillAt[33]                   // its last kill (a fidget after it may follow)
new Float:g_ShotMark[33]                 // its gunfire last warned the bots around then
new Float:g_NextFun[33]                  // when this bot may do something again
new Float:g_NextAny                      // when any bot may (so it's never a crowd at once)
new g_Press[33]                          // buttons to press on its next command
new g_Impulse[33]                        // impulse to send on its next command
new bool:g_FlashOff[33]                  // the flashlight goes off on its next command, whatever
new g_FlashLeft[33]                      // flashlight toggles still to do
new g_JumpsLeft[33], Float:g_NextJump[33]
new Float:g_FireUntil[33], bool:g_FireOn[33], Float:g_FireFlip[33]
new g_QLeft[33], g_QDone[33], g_QFrom[33], g_QTo[33]
new bool:g_QKill[33]                     // these flicks follow a kill (free_after_kill decides)
new g_PendWeapon[33]                     // the gun a delayed silencer/scope/burst press is for
new bool:g_Scoping[33]                   // in the middle of scoping in and out
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
	// gunfire: the bots around hear it (guns only, not the knife, grenades or the bomb)
	static const GUNS[][] = { "weapon_p228", "weapon_scout", "weapon_xm1014", "weapon_mac10", "weapon_aug",
		"weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas",
		"weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3",
		"weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_p90" }
	for (new i = 0; i < sizeof GUNS; i++)
		RegisterHam(Ham_Weapon_PrimaryAttack, GUNS[i], "fw_primary_post", 1)
	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0")
	register_logevent("ev_bomb_planted", 3, "2=Planted_The_Bomb")
	register_logevent("ev_round_start", 2, "1=Round_Start")
	register_event("SendAudio", "ev_t_win", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ct_win", "a", "2&%!MRAD_ctwin")
	register_event("DeathMsg", "ev_death", "a")
	set_task(1.0, "task_think", TASK_THINK)
}

public plugin_cfg()
{
	g_pPersonality = get_cvar_pointer("gm_personality")
	g_pStyle = get_cvar_pointer("gm_botstyle")
	g_pFlashAllowed = get_cvar_pointer("mp_flashlight")
	g_pFreeForAll = get_cvar_pointer("mp_freeforall")
	g_pScrim = get_cvar_pointer("gm_scrim_phase")
	g_Chat = bool:LibraryExists("gm_botchat", LibType_Library)
	g_HumanBots = bool:LibraryExists("gm_humanbots", LibType_Library)
	// Loner-Strike's own game DLL: it adds ls_bot (and these cvars with it)
	g_Fork = get_cvar_pointer("bot_ls_routes") != 0
	// a night or dark map, from its name: the flashlight makes sense in the round too
	new map[32]
	get_mapname(map, charsmax(map))
	strtolower(map)
	g_DarkMap = contain(map, "night") != -1 || contain(map, "nite") != -1 || contain(map, "dark") != -1
		|| contain(map, "black") != -1 || contain(map, "shadow") != -1
	g_FunOn = fun_allowed()
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
	new phase = g_pScrim ? get_pcvar_num(g_pScrim) : 0
	return !(phase > 0 && phase < 5)
}

// Everything a bot has going on here stops (its tasks, what it was about to press)
clear_bot(id)
{
	remove_task(id + TASK_QSPAM)
	remove_task(id + TASK_SLOTS)
	remove_task(id + TASK_KILL)
	remove_task(id + TASK_FREEZE)
	remove_task(id + TASK_KNIFE)
	remove_task(id + TASK_ATTACK2)
	remove_task(id + TASK_FLASH)
	remove_task(id + TASK_CELEB)
	remove_task(id + TASK_SCOPE)
	g_QLeft[id] = 0
	g_JumpsLeft[id] = 0
	g_Press[id] = 0
	g_Impulse[id] = 0
	g_FlashOff[id] = false
	g_FlashLeft[id] = 0
	g_FireUntil[id] = 0.0
	g_PendWeapon[id] = 0
	g_Scoping[id] = false
}

// A bot leaving: nothing of its carries over to whoever gets the slot next
public client_disconnected(id)
{
	clear_bot(id)
}

public client_putinserver(id)
{
	clear_bot(id)
	g_Busy[id] = g_FireBusy[id] = g_SeenAt[id] = g_SightAt[id] = g_KillAt[id] = 0.0
	g_Sight[id] = false
	g_Quiet[id] = random_float(8.0, 10.0)
	g_NextFun[id] = get_gametime() + random_float(20.0, 60.0)
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

// Free to do something: alive, a bot, no danger, no enemy in sight. calm: and it has been
// quiet a while (nothing seen, no shots of its own) - everything but celebrating needs that
bool:free_bot(id, bool:calm = true)
{
	if (!is_user_alive(id) || !is_user_bot(id))
		return false
	new Float:now = get_gametime()
	if (now < g_Busy[id] || g_BombPlanted)
		return false
	if (calm && (now < g_FireBusy[id] || now - g_SeenAt[id] < g_Quiet[id]))
		return false
	// the bomb carrier has somewhere to be once the round is on
	if (!g_Freeze && user_has_weapon(id, CSW_C4))
		return false
	// gm_humanbots has it afk or holding an angle: leave it be
	if (g_HumanBots && hb_bot_idle(id))
		return false
	return !sees_enemy(id)
}

// Right after its own kill: free if nothing else is going on. The fight it just won doesn't
// count (it saw that one, and fired), anything since the kill does
bool:free_after_kill(id)
{
	if (!is_user_alive(id) || !is_user_bot(id) || g_BombPlanted)
		return false
	new Float:now = get_gametime()
	if (now < g_Busy[id] || now < g_FireBusy[id] || g_SeenAt[id] > g_KillAt[id] + 0.4)
		return false
	if (!g_Freeze && user_has_weapon(id, CSW_C4))
		return false
	if (g_HumanBots && hb_bot_idle(id))
		return false
	return !sees_enemy(id)
}

// Enemy in sight, looked at again every quarter second or so (each bot on its own beat);
// seeing one is remembered (g_SeenAt)
bool:sees_enemy(id)
{
	new Float:now = get_gametime()
	if (now >= g_SightAt[id])
	{
		g_SightAt[id] = now + random_float(0.2, 0.3)
		g_Sight[id] = enemy_in_sight(id)
	}
	if (g_Sight[id])
		g_SeenAt[id] = now
	return g_Sight[id]
}

// Some danger: for `secs` it's busy (never shorter than it already was)
busy(id, Float:secs)
{
	g_Busy[id] = floatmax(g_Busy[id], get_gametime() + secs)
}

bool:enemy_in_sight(id)
{
	new Float:eye[3], Float:other[3], Float:ofs[3]
	pev(id, pev_origin, eye)
	pev(id, pev_view_ofs, ofs)
	eye[2] += ofs[2]
	new team = get_user_team(id)
	// free for all: everyone else is an enemy
	new bool:ffa = g_pFreeForAll && get_pcvar_num(g_pFreeForAll) != 0
	new tr = create_tr2()
	new bool:seen
	for (new i = 1; i <= MaxClients && !seen; i++)
	{
		if (i == id || !is_user_alive(i) || (!ffa && get_user_team(i) == team))
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
	if (!(1 <= victim <= MaxClients) || bits & DMG_FALL)
		return
	new Float:now = get_gametime()
	// hurt: it stays wary a good while
	busy(victim, random_float(10.0, 12.0))
	g_SeenAt[victim] = now
	// a hit that didn't kill: still its fight (the killing blow comes after the DeathMsg,
	// and a fight it won shouldn't block what it does after)
	if (1 <= attacker <= MaxClients && attacker != victim && is_user_alive(victim))
		g_FireBusy[attacker] = floatmax(g_FireBusy[attacker], now + 4.0)
	// someone shot close by: the bots around are wary too
	new Float:at[3], Float:o[3]
	pev(victim, pev_origin, at)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == victim || i == attacker || !is_user_alive(i) || !is_user_bot(i))
			continue
		pev(i, pev_origin, o)
		if (get_distance_f(at, o) < 1200.0)
			busy(i, random_float(6.0, 8.0))
	}
}

// A shot: the bots around hear it (each shooter at most a few times a second)
public fw_primary_post(weapon)
{
	if (!g_FunOn)
		return
	new shooter = pev(weapon, pev_owner)
	if (!(1 <= shooter <= MaxClients))
		return
	new Float:now = get_gametime()
	// (its own shots into the air after a won round don't count)
	if (now < g_ShotMark[shooter] || now < g_FireUntil[shooter])
		return
	g_ShotMark[shooter] = now + 0.3
	new Float:at[3], Float:o[3]
	pev(shooter, pev_origin, at)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == shooter || !is_user_alive(i) || !is_user_bot(i))
			continue
		pev(i, pev_origin, o)
		if (get_distance_f(at, o) < 1500.0)
			busy(i, random_float(6.0, 8.0))
	}
}

public fw_spawn_post(id)
{
	if (!is_user_alive(id))
		return
	clear_bot(id)
	// a new life: last round's dangers are over
	g_Busy[id] = g_FireBusy[id] = g_SeenAt[id] = g_SightAt[id] = 0.0
	g_Sight[id] = false
}

public ev_bomb_planted()
{
	g_BombPlanted = true
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2)
	if (1 <= victim <= MaxClients)
		clear_bot(victim)
	if (!(1 <= killer <= MaxClients) || killer == victim || !is_user_bot(killer))
		return
	// the fight it just won is over: its own shots at that one no longer count
	g_KillAt[killer] = get_gametime()
	g_FireBusy[killer] = 0.0
	// after a kill, once it's quiet again: a reload it doesn't need, or a weapon flick
	if (!fun_allowed() || !get_pcvar_num(g_pWeapons) || task_exists(killer + TASK_KILL))
		return
	if (roll(killer, FUN_RELOAD, 0.15))
		set_task(random_float(0.6, 1.4), "task_after_kill", killer + TASK_KILL, "r", 1)
	else if (roll(killer, FUN_QSPAM, 0.06))
		set_task(random_float(0.8, 2.0), "task_after_kill", killer + TASK_KILL, "q", 1)
}

// Waits a few seconds for it to be quiet (it may still be firing, or someone else shows up);
// nothing if it doesn't get quiet
public task_after_kill(const what[], tid)
{
	new id = tid - TASK_KILL
	if (!is_user_alive(id) || !is_user_bot(id))
		return
	if (!free_after_kill(id))
	{
		if (get_gametime() - g_KillAt[id] < 7.0)
			set_task(random_float(0.3, 0.6), "task_after_kill", tid, what, 1)
		return
	}
	if (what[0] == 'r')
		do_reload(id)
	else
		start_qspam(id, true)
}

// Freeze time starts (before the dead respawn: the rolls wait a moment)
public ev_new_round()
{
	g_Freeze = true
	g_BombPlanted = false
	g_FunOn = fun_allowed()
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
			set_task(delay, "task_freeze_do", id + TASK_FREEZE, "j", 1)
		else if (roll(id, FUN_KNIFE, 0.07))
			set_task(delay, "task_freeze_do", id + TASK_FREEZE, "k", 1)
		else if (roll(id, FUN_QSPAM, 0.06))
			set_task(delay, "task_freeze_do", id + TASK_FREEZE, "q", 1)
		else if (get_pcvar_num(g_pFlashlight) && flashlight_allowed() && roll(id, FUN_FLASHLIGHT, 0.05))
			set_task(delay, "task_freeze_do", id + TASK_FREEZE, "f", 1)
		else if (g_Fork && get_pcvar_num(g_pSalute) && roll(id, FUN_SALUTE, 0.06))
			set_task(delay, "task_freeze_do", id + TASK_FREEZE, "s", 1)
	}
}

public task_freeze_do(const what[], tid)
{
	new id = tid - TASK_FREEZE
	switch (what[0])
	{
		case 'j': freeze_jump(id)
		case 'k': knife_flick(id)
		case 'q': start_qspam(id)
		case 'f': flashlight(id)
		case 's': salute(id)
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
		if (free_bot(id) && roll(id, FUN_SPAWNJUMP, 0.08))
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
		if (get_user_team(id) != team || !free_bot(id, false) || !roll(id, FUN_CELEBRATE, 0.12))
			continue
		n++
		set_task(random_float(0.3, 1.5), "task_celebrate", id + TASK_CELEB)
	}
}

// ---- now and then, during the round -----------------------------------------------------

public task_think()
{
	// no set beat: each look a little sooner or later than the last
	set_task(random_float(0.7, 1.4), "task_think", TASK_THINK)
	g_FunOn = fun_allowed()
	if (!g_FunOn || g_Freeze)
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
	// in the round only on a dark map (freeze time: anywhere)
	if (g_DarkMap && get_pcvar_num(g_pFlashlight) && flashlight_allowed() && roll(id, FUN_FLASHLIGHT, 0.02))
		flashlight(id)
}

// ---- the things themselves ----------------------------------------------------------------

public fw_cmdstart(id, uc, seed)
{
	if (!is_user_bot(id) || !is_user_alive(id))
		return FMRES_IGNORED

	new Float:now = get_gametime()
	new buttons = get_uc(uc, UC_Buttons)
	if (buttons & IN_ATTACK && now >= g_FireUntil[id])
		g_FireBusy[id] = floatmax(g_FireBusy[id], now + 3.0)
	// using something (defusing, a hostage): busy
	if (buttons & IN_USE)
		g_FireBusy[id] = floatmax(g_FireBusy[id], now + 3.0)
	// keeps its eyes open (and its memory of danger up to date) while fun is on
	new bool:danger = g_FunOn && (sees_enemy(id) || now < g_Busy[id])

	// anything of ours under way stops the moment there's danger, or gm_humanbots has it
	// afk or holding an angle
	new bool:stop = danger
	if (!stop && g_HumanBots && (g_Press[id] || g_Impulse[id] || g_JumpsLeft[id] > 0 || g_Scoping[id]) && hb_bot_idle(id))
		stop = true
	if (stop && g_Scoping[id])
		scope_abort(id)

	new changed
	if (g_Press[id])
	{
		if (!stop)
		{
			buttons |= g_Press[id]
			changed = 1
		}
		g_Press[id] = 0
	}
	if (g_JumpsLeft[id] > 0 && now >= g_NextJump[id])
	{
		if (stop)
			g_JumpsLeft[id] = 0
		else if (pev(id, pev_flags) & FL_ONGROUND)
		{
			buttons |= IN_JUMP
			g_JumpsLeft[id]--
			g_NextJump[id] = now + random_float(0.55, 0.95)
			changed = 1
		}
	}
	if (now < g_FireUntil[id])
	{
		// only while it's really looking up into the air with a gun: anything else (the look
		// was refused, a fight, a grenade in hand) and it doesn't fire
		new Float:view[3]
		pev(id, pev_v_angle, view)
		if (stop || now < g_FireBusy[id] || !holding_gun(id))
			g_FireUntil[id] = 0.0
		else if (view[0] < -35.0)
		{
			// short bursts with gaps, each its own length
			if (now >= g_FireFlip[id])
			{
				g_FireOn[id] = !g_FireOn[id]
				g_FireFlip[id] = now + (g_FireOn[id] ? random_float(0.15, 0.35) : random_float(0.09, 0.2))
			}
			if (g_FireOn[id])
				buttons |= IN_ATTACK
			else
				buttons &= ~IN_ATTACK
			changed = 1
		}
	}
	if (changed)
		set_uc(uc, UC_Buttons, buttons)
	// the flashlight going off happens whatever's going on
	if (g_FlashOff[id])
	{
		g_FlashOff[id] = false
		g_Impulse[id] = 0
		if (pev(id, pev_effects) & EF_DIMLIGHT)
			set_uc(uc, UC_Impulse, 100)
	}
	else if (g_Impulse[id])
	{
		if (!stop)
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
		set_task(random_float(2.4, 3.5), "task_attack2", id + TASK_ATTACK2)
}

// In, further in, out again (all four have two zoom levels). However it goes, it ends
// unzoomed: scope_end, or scope_abort when something comes up
scope(id)
{
	g_PendWeapon[id] = get_user_weapon(id)
	g_Scoping[id] = true
	g_Press[id] |= IN_ATTACK2
	new Float:at
	for (new i = 1; i < 3; i++)
	{
		at = 0.35 * i + random_float(0.0, 0.25)
		set_task(at, "task_attack2", id + TASK_ATTACK2)
	}
	set_task(at + 0.5, "task_scope_end", id + TASK_SCOPE)
}

public task_scope_end(tid)
{
	new id = tid - TASK_SCOPE
	if (g_Scoping[id])
		scope_abort(id)
}

// Not left zoomed in: the presses still to come are dropped, the zoom goes
scope_abort(id)
{
	g_Scoping[id] = false
	remove_task(id + TASK_ATTACK2)
	remove_task(id + TASK_SCOPE)
	g_Press[id] &= ~IN_ATTACK2
	if (is_user_alive(id) && get_user_weapon(id) == g_PendWeapon[id] && cs_get_user_zoom(id) > CS_SET_NO_ZOOM)
		cs_set_user_zoom(id, CS_SET_NO_ZOOM, 1)
}

burst(id)
{
	g_PendWeapon[id] = get_user_weapon(id)
	g_Press[id] |= IN_ATTACK2
	if (random_num(0, 2))
		set_task(random_float(0.6, 1.5), "task_attack2", id + TASK_ATTACK2)
}

public task_attack2(tid)
{
	new id = tid - TASK_ATTACK2
	if (free_bot(id) && get_user_weapon(id) == g_PendWeapon[id])
		g_Press[id] |= IN_ATTACK2
	else if (g_Scoping[id])
		scope_abort(id)
}

do_reload(id)
{
	new clip, ammo
	get_user_ammo(id, get_user_weapon(id), clip, ammo)
	if (holding_gun(id) && clip > 0 && ammo > 0)
		g_Press[id] |= IN_RELOAD
}

// Flicks between its gun and the last weapon it held (lastinv): only between guns and the
// knife, never onto a grenade (the bot's brain pulls the pin on one in hand) or the bomb.
// after_kill: the quiet check after a kill (free_after_kill) was done already
start_qspam(id, bool:after_kill = false)
{
	if (!after_kill && !free_bot(id))
		return
	// a knife out is gm_humanbots' (a knife run, a show-off): flicks would undo it
	if (!holding_gun(id))
		return
	new last = get_ent_data_entity(id, "CBasePlayer", "m_pLastItem")
	if (last <= 0 || !pev_valid(last))
		return
	new to = cs_get_weapon_id(last)
	if (!to || to == CSW_C4 || is_nade(to) || !user_has_weapon(id, to))
		return
	g_QFrom[id] = get_user_weapon(id)
	g_QTo[id] = to
	g_QDone[id] = 0
	g_QKill[id] = after_kill
	// mostly an even number of flicks (it ends on the gun it started with)
	g_QLeft[id] = random_num(2, 6)
	if (random_num(0, 3))
		g_QLeft[id] &= ~1
	qspam_step(id)
}

public task_qspam(tid)
{
	qspam_step(tid - TASK_QSPAM)
}

qspam_step(id)
{
	if (g_QLeft[id] <= 0 || !is_user_alive(id))
		return
	// the weapon it should have in hand now: if it's another, something else (gm_humanbots'
	// knife, the bot's brain) took over, and that's left alone
	if (get_user_weapon(id) != ((g_QDone[id] & 1) ? g_QTo[id] : g_QFrom[id]))
	{
		g_QLeft[id] = 0
		return
	}
	if (!(g_QKill[id] ? free_after_kill(id) : free_bot(id)))
	{
		// stopped halfway (a fight): back to the gun it started with
		if (g_QDone[id] & 1)
			engclient_cmd(id, "lastinv")
		g_QLeft[id] = 0
		return
	}
	engclient_cmd(id, "lastinv")
	g_QDone[id]++
	if (--g_QLeft[id] > 0)
		set_task(random_float(0.12, 0.28), "task_qspam", id + TASK_QSPAM)
}

start_slots(id)
{
	if (!free_bot(id) || !holding_gun(id))
		return
	// knife, pistol, back to the gun it had (by name: slot1..slot4 are keys in the player's
	// own game, the server never sees them). Not a grenade: the bot's brain pulls the pin
	// on one in hand
	new held = get_user_weapon(id)
	new steps[3][24], n
	copy(steps[n++], charsmax(steps[]), "weapon_knife")
	new weapons[32], count, pistol
	get_user_weapons(id, weapons, count)
	for (new i = 0; i < count && !pistol; i++)
		if (is_pistol(weapons[i]))
			pistol = weapons[i]
	if (pistol && pistol != held)
		get_weaponname(pistol, steps[n++], charsmax(steps[]))
	get_weaponname(held, steps[n++], charsmax(steps[]))
	for (new i = 0; i < n; i++)
		set_task(0.35 * i + random_float(0.0, 0.2), i == n - 1 ? "task_slot_back" : "task_slot", id + TASK_SLOTS, steps[i], sizeof steps[])
}

bool:is_pistol(w)
{
	return w == CSW_GLOCK18 || w == CSW_USP || w == CSW_P228 || w == CSW_DEAGLE || w == CSW_FIVESEVEN || w == CSW_ELITE
}

bool:is_nade(w)
{
	return w == CSW_HEGRENADE || w == CSW_FLASHBANG || w == CSW_SMOKEGRENADE
}

public task_slot(const data[], tid)
{
	new id = tid - TASK_SLOTS
	if (free_bot(id))
		engclient_cmd(id, data)
}

// The last step (its own gun again) happens even if something came up meanwhile
public task_slot_back(const data[], tid)
{
	new id = tid - TASK_SLOTS
	if (is_user_alive(id) && is_user_bot(id) && user_has_weapon(id, get_weaponid(data)))
		engclient_cmd(id, data)
}

knife_flick(id)
{
	if (!free_bot(id) || !holding_gun(id))
		return
	engclient_cmd(id, "weapon_knife")
	set_task(random_float(0.8, 2.5), "task_lastinv", id + TASK_KNIFE)
}

public task_lastinv(tid)
{
	new id = tid - TASK_KNIFE
	// even if something came up meanwhile: it shouldn't be left with the knife out
	if (is_user_alive(id) && is_user_bot(id) && get_user_weapon(id) == CSW_KNIFE)
		engclient_cmd(id, "lastinv")
}

freeze_jump(id)
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

// On and off a few times, each toggle its own gap; it always ends off
flashlight(id)
{
	if (!free_bot(id))
		return
	g_FlashLeft[id] = 2 * random_num(1, 3) - 1
	g_Impulse[id] = 100
	set_task(random_float(0.18, 0.45), "task_flash_step", id + TASK_FLASH)
}

public task_flash_step(tid)
{
	new id = tid - TASK_FLASH
	if (!is_user_alive(id) || !is_user_bot(id))
		return
	if (g_FlashLeft[id] > 0 && free_bot(id))
	{
		g_Impulse[id] = 100
		g_FlashLeft[id]--
		set_task(random_float(0.18, 0.45), "task_flash_step", tid)
		return
	}
	// done or cut short: off, if it isn't
	g_FlashLeft[id] = 0
	g_FlashOff[id] = true
}

// Looks at a teammate close by that it can see and moves the gun up and down at them
salute(id)
{
	if (!g_Fork || !free_bot(id))
		return
	new Float:me[3], Float:other[3], best, Float:bestDist = 600.0
	eye_pos(id, me)
	new team = get_user_team(id)
	new tr = create_tr2()
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == id || !is_user_alive(i) || get_user_team(i) != team)
			continue
		eye_pos(i, other)
		new Float:d = get_distance_f(me, other)
		// people more than bots (it's mostly for them)
		if (!is_user_bot(i))
			d *= 0.5
		if (d >= bestDist)
			continue
		// only someone it can see
		engfunc(EngFunc_TraceLine, me, other, DONT_IGNORE_MONSTERS, id, tr)
		new Float:frac
		get_tr2(tr, TR_flFraction, frac)
		if (frac < 1.0 && get_tr2(tr, TR_pHit) != i)
			continue
		bestDist = d
		best = i
	}
	free_tr2(tr)
	if (!best)
		return
	// at the teammate as they stand now, a little under the eyes
	eye_pos(best, other)
	other[2] -= 6.0
	server_cmd("ls_bot %d nod %.0f %.0f %.0f %.1f %.2f", id, other[0], other[1], other[2], random_float(1.2, 2.4), random_float(0.22, 0.38))
	server_cmd("ls_bot %d hold %.1f", id, random_float(1.0, 2.0))
}

eye_pos(id, Float:out[3])
{
	new Float:ofs[3]
	pev(id, pev_origin, out)
	pev(id, pev_view_ofs, ofs)
	out[2] += ofs[2]
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

public task_celebrate(tid)
{
	new id = tid - TASK_CELEB
	if (!g_Fork || !free_bot(id, false))
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
	set_task(0.5, "task_fire", id + TASK_CELEB)
}

public task_fire(tid)
{
	new id = tid - TASK_CELEB
	if (free_bot(id, false) && holding_gun(id))
	{
		new Float:now = get_gametime()
		g_FireUntil[id] = now + random_float(0.6, 1.4)
		g_FireOn[id] = true
		g_FireFlip[id] = now + random_float(0.15, 0.35)
	}
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
