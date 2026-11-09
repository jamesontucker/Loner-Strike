/*
	gm_botfun - the small, silly things real people do on a public server, done by bots
	now and then, each the way its chat personality (gm_botchat) would.

	Weapons:    flicking between the last two weapons (Q-spam), the silencer off and on
	            (the bot puts it back itself), the scope in and out, the Glock's or FAMAS's
	            burst on and off, cycling through the slots, reloading a nearly full
	            magazine after a kill
	Spawn:      freeze time is when people never stand still: about four in five do a
	            thing or a few (looking about at teammates, the sky, their gun; flicking
	            weapons, a few all freeze time; heads; the knife out and back; jumping or
	            crouch spam on the spot; the flashlight, in the round too on a dark map;
	            a salute). Leaving spawn nearly half do something: jumping out
	            (teammates close by sometimes join in), flicking weapons on the run, the
	            knife out, crouch spam, a look back, or still sorting out their gear and
	            leaving late. In a match, freeze time is only looking about and a weapon
	            check
	Looking:    (needs Loner-Strike's game DLL, which adds ls_bot) saluting a teammate
	            with the gun (looking up and down at them), good aimers flicking from one
	            teammate's head to the next in freeze time, looking around at the map or
	            glancing down while walking, shooting into the air after the round is won
	Breakables: shooting the glass or bottles in view after the round ends; a few slash
	            the same crate with the knife every time they walk past it
	Drops:      a well-off bot with a generous streak drops its rifle for a broke teammate
	            (a person or a bot) who asks in chat ("drop pls", "drop me awp"), and buys
	            another; broke bots sometimes ask
	Bad habits: newer players reloading after every kill whatever's around, flicking
	            weapons for no reason, walking about with the knife out, pulling a grenade
	            out of spawn and putting it away, staring at their teammates

	Each personality has its own leanings (LIKES, DISLIKES, activity), each bot its own
	fidgetiness (most hardly, a few all the time) and a couple of habits of its own, all
	from its name, so a regular keeps them; the newer habits are each only some bots'
	(from the name, leaning on personality and how good a player it is). Nothing happens
	in a match (or with the Matchmaking style on Classic) but quiet freeze time looks, in modes a plugin steers bots
	through (Hide and Seek, Deathrun, zombies...), with the Classic bot style, or at
	gm_personality 0; gm_personality scales how often. Deathmatch (and Gun Game) keeps the
	quick ones (flicks, scopes, a reload after a kill; coming back in, nearly half do
	something like leaving spawn in a round), and none of the standing about.
	A bot fidgets only once it has been quiet 8-10 seconds: no enemy in sight, not hurt,
	no shots of its own, no gunfire or someone hurt near it. Whatever it was doing stops
	the moment that changes (the gun comes back out, a scope, burst or flashlight is put
	back as it was, a look is let go).
	Other plugins can ask bf_danger(id): the same danger, for their own habits.

	gm_bf_fun        1 all of it on or off
	gm_bf_weapons    1 weapon fidgeting
	gm_bf_spawn      1 freeze time and leaving spawn habits
	gm_bf_flashlight 1 the flashlight (only where mp_flashlight allows it)
	gm_bf_salute     1 saluting teammates
	gm_bf_look       1 looking around at the map, at teammates' heads in freeze time
	gm_bf_celebrate  1 shooting into the air after a won round
	gm_bf_breakables 1 shooting glass after the round, slashing a crate on the way past
	gm_bf_drop       1 dropping a gun for a teammate who asks
	gm_bf_bad        1 newer players' bad habits
	gm_bf_cheeky     1 cheeky stuff on a pub: teabagging a kill, slashing a teammate on the way
	                   out of spawn (only with friendly fire off), jumping about after the round
*/

#include <amxmodx>
#include <fakemeta>
#include <hamsandwich>
#include <cstrike>
#include <fun>
#include <amxmisc>

// gm_botchat (optional): the bot's chat personality and its lines
native botchat_persona(id)
native botchat_say(id, const section[], other, Float:chance)
native botchat_say_now(id, const section[], other, bool:team)
// gm_humanbots (optional): afk or holding an angle; how good a player it is (0-3, -1 none)
native hb_bot_idle(id)
native hb_bot_level(id)
native hb_bot_showing_off(id)
native Float:hb_urgent_until(id)
// gm_people (optional): a trait's value (-1 unknown)
native people_get(id, const key[])
native people_mood()

#define PLUGIN  "Bot fun"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define STYLE_CLASSIC 0
#define STYLE_MM      2
// task ids: each kind its own range (id + offset), so one never cancels another
#define TASK_QSPAM    8600
#define TASK_SLOTS    8700
#define TASK_KILL     8800
#define TASK_FREEZE   11000             // (+ id + 32 * slot, FREEZE_SLOTS of them)
#define FREEZE_SLOTS  6
#define TASK_KNIFE    9000
#define TASK_ATTACK2  9100
#define TASK_FLASH    9200
#define TASK_CELEB    9300
#define TASK_SCOPE    9400
#define TASK_THINK    9500
#define TASK_FLICK    9600
#define TASK_BREAK    9700
#define TASK_DROP     9800
#define TASK_BAD      9900
#define TASK_SPAWN    10000
#define TASK_REBUY    10100
#define TASK_GLANCE   10200
#define TASK_LEAVE    10300
#define TASK_SPAWNCHK 10500
#define TASK_TBAG     10600
#define TASK_SLASH    10700
#define TASK_SWEEP    10800
#define TASK_GIFT     10900

// what a bot can do, for its own habits (two per bot)
enum
{
	FUN_QSPAM, FUN_SILENCER, FUN_SCOPE, FUN_BURST, FUN_SLOTS, FUN_RELOAD,
	FUN_SPAWNJUMP, FUN_FREEZEJUMP, FUN_KNIFE, FUN_FLASHLIGHT,
	FUN_SALUTE, FUN_LOOK, FUN_CELEBRATE, FUN_DUCK,
	FUN_COUNT
}

// the newer habits: each only some bots' (quirk(), from the name)
enum
{
	Q_FLICKS, Q_BREAK, Q_SLASH, Q_GENEROUS, Q_ASKS, Q_TEAMJUMP, Q_SWITCHY,
	Q_BADRELOAD, Q_BADSWITCH, Q_BADKNIFE, Q_BADNADE, Q_LOOKMATES, Q_CHEEKY
}

// how the mode is played (PART of every habit's say)
enum { CTX_PUB, CTX_DM, CTX_MATCH, CTX_NOVELTY }

// what the bot is shooting at for us: the sky (a won round) or a point (a breakable)
enum { FIRE_SKY, FIRE_AT }

new g_pFun, g_pWeapons, g_pSpawn, g_pFlashlight, g_pSalute, g_pLook, g_pCelebrate
new g_pBreak, g_pDrop, g_pBad, g_pCheeky
new g_pPersonality, g_pStyle, g_pFlashAllowed, g_pFreeForAll, g_pScrim, g_pBotDm
new bool:g_Chat, bool:g_Fork, bool:g_HumanBots, bool:g_People, bool:g_BombPlanted, bool:g_DarkMap
new bool:g_FunOn                         // fun_allowed(), checked now and then (not every frame)
new bool:g_Novelty, bool:g_MatchMode, bool:g_ClassicMode, bool:g_BuyMode   // from the mode's id

new g_Hash[33]                           // its name, hashed (habits and quirks come from it)
new g_Habit[33][2]                       // its two habits (FUN_*)
new Float:g_Fidget[33]                   // how fidgety it is: most hardly, a few all the time
new Float:g_Busy[33]                     // danger until then (hurt, shots or someone hurt close by)
new Float:g_FireBusy[33]                 // its own fight until then (firing, hitting, using)
new Float:g_SeenAt[33]                   // last time it saw an enemy or got hurt
new Float:g_Quiet[33]                    // how long it must be quiet before fidgeting (8-10 s, each life)
new Float:g_Grace[33]                    // just spawned in deathmatch: quiet enough until then
new bool:g_Sight[33], Float:g_SightAt[33]   // enemy in sight, and when to look again
new Float:g_KillAt[33]                   // its last kill (a fidget after it may follow)
new g_LifeKills[33]                      // kills this life (a better kill, a bigger fidget)
new Float:g_ShotMark[33]                 // its gunfire last warned the bots around then
new Float:g_NextFun[33]                  // when this bot may do something again
new Float:g_NextAny                      // when any bot may (so it's never a crowd at once)
new g_Press[33]                          // buttons to press on its next command
new g_Force[33]                          // ... pressed whatever's going on (a bad habit)
new g_Impulse[33]                        // impulse to send on its next command
new bool:g_FlashOff[33]                  // the flashlight goes off on its next command, whatever
new g_FlashLeft[33]                      // flashlight toggles still to do
new g_JumpsLeft[33], Float:g_NextJump[33]
new g_DucksLeft[33], Float:g_DuckUntil[33], Float:g_NextDuck[33]   // crouch spam
new g_GlanceLeft[33]                     // freeze time glances still to do
// mouse sweeps in freeze time (start_sweep): a spin, a wiggle or a slow pan
enum { SWEEP_SPIN, SWEEP_WIGGLE, SWEEP_PAN }
new g_SweepLeft[33], g_SweepKind[33], Float:g_SweepYaw[33], g_SweepStep[33]
new Float:g_FireUntil[33], bool:g_FireOn[33], Float:g_FireFlip[33], g_FireKind[33], Float:g_FireAt[33][3]
new g_QLeft[33], g_QDone[33], g_QFrom[33], g_QTo[33]
new bool:g_QKill[33]                     // these flicks follow a kill (free_after_kill decides)
new Float:g_QGap[33]                     // the longest gap between flicks
new g_PendWeapon[33]                     // the gun a delayed silencer/scope/burst press is for
new bool:g_Scoping[33]                   // in the middle of scoping in and out
new g_BurstEnt[33], g_BurstWas[33]       // the gun whose burst mode we changed, and how it was
new g_SlotPistol[33]                     // the pistol a slot cycle went through
new g_Out[33]                            // a weapon of ours out (the knife, a grenade): the gun comes back
new Float:g_PuppetUntil[33]              // ls_bot has it looking or standing for us until then
new g_FlickLeft[33], g_FlickLast[33]     // freeze time flicks between teammates' heads
new g_FavBreak[33], Float:g_NextSlash[33]   // the crate it slashes every time it passes
new g_DropFor[33], g_DropUser[33], g_DropGun[33], g_DropBox[33]   // a gun it's dropping for someone
new bool:g_DropDone, bool:g_Asked[3]     // one drop a round; one ask a team
new g_Breaks                             // bots shooting breakables this round end
new Float:g_RoundStartAt
new Float:g_BodyAt[33][3]                // the body it's teabagging
new bool:g_Freeze
// gifts: a person drops a gun where a teammate bot with a worse one (or none) is
new g_GiftBot[33], g_GiftBox[33], g_GiftGun[33], g_GiftTries[33], bool:g_GiftSent[33]

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
	g_pBreak      = register_cvar("gm_bf_breakables", "1")
	g_pDrop       = register_cvar("gm_bf_drop", "1")
	g_pBad        = register_cvar("gm_bf_bad", "1")
	g_pCheeky     = register_cvar("gm_bf_cheeky", "1")

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
	// a draw (an aim map's clock running out, game commencing) ends the round too
	register_event("SendAudio", "ev_draw", "a", "2&%!MRAD_rounddraw")
	register_event("DeathMsg", "ev_death", "a")
	// "drop pls": someone asking a teammate for a gun
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say")
	// a person dropping a gun near a teammate bot with a worse one: a gift
	register_clcmd("drop", "cmd_drop")
	set_task(1.0, "task_think", TASK_THINK)
}

public plugin_cfg()
{
	g_pPersonality = get_cvar_pointer("gm_personality")
	g_pStyle = get_cvar_pointer("gm_botstyle")
	g_pFlashAllowed = get_cvar_pointer("mp_flashlight")
	g_pFreeForAll = get_cvar_pointer("mp_freeforall")
	g_pScrim = get_cvar_pointer("gm_scrim_phase")
	g_pBotDm = get_cvar_pointer("bot_deathmatch")
	g_Chat = bool:LibraryExists("gm_botchat", LibType_Library)
	g_HumanBots = bool:LibraryExists("gm_humanbots", LibType_Library)
	g_People = bool:LibraryExists("gm_people", LibType_Library)
	// Loner-Strike's own game DLL: it adds ls_bot (and these cvars with it)
	g_Fork = get_cvar_pointer("bot_ls_routes") != 0
	// a night or dark map, from its name: the flashlight makes sense in the round too
	new map[32]
	get_mapname(map, charsmax(map))
	strtolower(map)
	g_DarkMap = contain(map, "night") != -1 || contain(map, "nite") != -1 || contain(map, "dark") != -1
		|| contain(map, "black") != -1 || contain(map, "shadow") != -1
	// the mode: ones a plugin steers bots through get nothing (gm_humanbots' NO_IDLE: keep
	// the two lists alike); match mode plays like a match; buying (for drops) only where
	// it's normal
	new mode[32]
	read_mode(mode, charsmax(mode))
	static const NOVELTY[][] = { "hns", "deathrun", "kreedz", "surf", "soccerjam", "zombie", "zp5", "basebuilder" }
	for (new i = 0; i < sizeof NOVELTY; i++)
		if (equal(mode, NOVELTY[i]))
			g_Novelty = true
	g_MatchMode = bool:equal(mode, "match")
	g_ClassicMode = bool:equal(mode, "classic")
	g_BuyMode = g_ClassicMode || equal(mode, "war3") || equal(mode, "shero")
	g_FunOn = fun_allowed()
}

public plugin_natives()
{
	register_library("gm_botfun")
	register_native("bf_danger", "n_danger")
	register_native("bf_ask_drop", "n_ask_drop")
	set_native_filter("native_filter")
}

// (gm_humanbots without hb_urgent_until, an older one: not asked)
new bool:g_NoUrgent

public native_filter(const name[], index, trap)
{
	if (!trap && equal(name, "hb_urgent_until"))
		g_NoUrgent = true
	return (!trap && (equal(name, "botchat_", 8) || equal(name, "hb_", 3) || equal(name, "people_", 7))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

// bf_danger(id): danger around the bot: hurt, gunfire or someone hurt close by lately, or an
// enemy seen in the last 8-10 seconds (the one it has just killed doesn't count, like
// free_after_kill). False for the dead and people. Kept up to date while bot fun is on
public n_danger(plugin, params)
{
	new id = get_param(1)
	if (id < 1 || id > MaxClients || !is_user_alive(id) || !is_user_bot(id))
		return false
	new Float:now = get_gametime()
	if (now < g_Busy[id])
		return true
	return now - g_SeenAt[id] < g_Quiet[id] && g_SeenAt[id] > g_KillAt[id] + 0.4
}

// bf_ask_drop(asker, want, prefer): someone asked for a gun (gm_botchat, from what they said):
// the same as "drop pls" in chat. want: a CSW_ id or 0 for any gun; prefer: a bot they asked
// by name, or 0. That bot gives if it can (rich, close, free) unless it's a cold one, without
// needing the generous streak. 1 when a drop is on its way
public n_ask_drop(plugin, params)
{
	new asker = get_param(1), want = get_param(2), prefer = get_param(3)
	if (asker < 1 || asker > MaxClients || !is_user_alive(asker) || !drop_mode() || !buy_time())
		return 0
	if (want < 0 || want > CSW_P90)
		want = 0
	if (prefer < 1 || prefer > MaxClients)
		prefer = 0
	return ask_drop(asker, want, 0.6, prefer) ? 1 : 0
}

// The current mode's id (gamemodes keeps it in data/gamemodes/current.txt)
read_mode(mode[], len)
{
	new path[128]
	mode[0] = 0
	get_datadir(path, charsmax(path))
	new f = fopen(fmt("%s/gamemodes/current.txt", path), "rt")
	if (f)
	{
		fgets(f, mode, len)
		trim(mode)
		fclose(f)
	}
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

// How the mode is played: a pub (everything), deathmatch (only the quick things), a match
// (guns only: a match, match mode, the Matchmaking style on Classic) or a mode a plugin
// steers bots through (nothing)
mode_ctx()
{
	if (g_Novelty)
		return CTX_NOVELTY
	new phase = g_pScrim ? get_pcvar_num(g_pScrim) : 0
	if ((phase > 0 && phase < 5) || g_MatchMode || (g_ClassicMode && g_pStyle && get_pcvar_num(g_pStyle) == STYLE_MM))
		return CTX_MATCH
	if (g_pBotDm && get_pcvar_num(g_pBotDm))
		return CTX_DM
	return CTX_PUB
}

// Not in a match, nor in a mode a plugin steers bots through, nor with the Classic bot
// style (plain CZ bots), nor at personality 0
bool:fun_allowed()
{
	if (!get_pcvar_num(g_pFun) || pers() <= 0.0)
		return false
	if (g_pStyle && get_pcvar_num(g_pStyle) == STYLE_CLASSIC)
		return false
	new c = mode_ctx()
	return c == CTX_PUB || c == CTX_DM
}

// Personality at Plain (gm_personality 0) on a pub: no quirks, but not stock-stiff either;
// about a third of the bots still look about, hop or crouch in freeze time and leaving spawn.
// (Original style, gm_botstyle 0, is the game's own bots: nothing)
bool:plain_floor()
{
	if (!get_pcvar_num(g_pFun) || pers() > 0.0)
		return false
	if (g_pStyle && get_pcvar_num(g_pStyle) == STYLE_CLASSIC)
		return false
	// (deathmatch too: only the respawn's quiet hop, crouch or look)
	new ctx = mode_ctx()
	return ctx == CTX_PUB || ctx == CTX_DM
}

// How good a player it is: 0 new, 1 average, 2 good, 3 very good (its style in
// gm_humanbots, else gm_people's skill, else average)
level(id)
{
	new l = g_HumanBots ? hb_bot_level(id) : -1
	if (l < 0 && g_People)
		l = people_get(id, "skill")
	return l < 0 ? 1 : clamp(l, 0, 3)
}

// A number 0-99 of its own for each quirk, from its name: the share of bots with one is
// whatever it's compared against
quirk_roll(id, which)
{
	new h = g_Hash[id] ^ ((which + 1) * 0x45D9F3B)
	h = ((h >>> 16) ^ h) * 0x45D9F3B
	h = (h >>> 16) ^ h
	return (h & 0x7FFFFFFF) % 100
}

bool:quirk(id, which, pct)
{
	return quirk_roll(id, which) < pct
}

// How many bots (percent) have each newer habit, by personality and how good they are
bool:has_quirk(id, which)
{
	new p = persona(id), l = level(id)
	switch (which)
	{
		// good aimers, flicking between heads
		case Q_FLICKS: return l >= 2 && quirk(id, which, has(p, "tESN") ? 75 : 50)
		case Q_BREAK: return quirk(id, which, has(p, "kjzhm") ? 45 : has(p, "qLd") ? 5 : 18)
		case Q_SLASH: return quirk(id, which, has(p, "kjzS") ? 20 : 6)
		case Q_GENEROUS: return quirk(id, which, warmth(id) == 2 ? 55 : warmth(id) == 0 ? 3 : 20)
		case Q_ASKS: return quirk(id, which, has(p, "nkuz") ? 50 : has(p, "otE") ? 5 : 18)
		case Q_TEAMJUMP: return quirk(id, which, has(p, "jkhzfS") ? 60 : has(p, "qLd") ? 10 : 35)
		case Q_SWITCHY: return quirk(id, which, has(p, "kjzhS") ? 40 : 12)
		// newer and sloppier players' bad habits
		case Q_BADRELOAD: return quirk(id, which, l == 0 ? 60 : l == 1 ? 12 : 0)
		case Q_BADSWITCH: return quirk(id, which, l == 0 ? 40 : l == 1 ? 10 : 0)
		case Q_BADKNIFE: return quirk(id, which, l == 0 ? 35 : l == 1 ? 5 : 0)
		case Q_BADNADE: return quirk(id, which, l == 0 ? 30 : 0)
		case Q_LOOKMATES: return quirk(id, which, l == 0 ? 40 : l == 1 ? 8 : 0)
		// kids and jokers mostly; never the quiet or the serious ones
		// (more of them in a toxic lobby, few in a friendly one)
		case Q_CHEEKY: return quirk(id, which, floatround((has(p, "kjzh") ? 30.0 : has(p, "qLdtoE") ? 0.0 : 5.0) * mood_mult(), floatround_floor))
	}
	return false
}

// How warm a person it is: 0 cold, 1 neutral, 2 warm (gm_people, else its personality)
warmth(id)
{
	new w = g_People ? people_get(id, "warmth") : -1
	if (w >= 0)
		return w
	new p = persona(id)
	return has(p, "fwcDo") ? 2 : has(p, "stOHE") ? 0 : 1
}

// A chance for one of the newer habits: its personality's activity and gm_personality
bool:chance(id, Float:base)
{
	return random_float(0.0, 1.0) < floatmin(base * activity(persona(id)) * pers(), 0.9)
}

// Everything a bot has going on here stops (its tasks, what it was about to press)
clear_bot(id)
{
	remove_task(id + TASK_QSPAM)
	remove_task(id + TASK_SLOTS)
	remove_task(id + TASK_KILL)
	for (new i = 0; i < FREEZE_SLOTS; i++)
		remove_task(id + TASK_FREEZE + 32 * i)
	remove_task(id + TASK_KNIFE)
	remove_task(id + TASK_ATTACK2)
	remove_task(id + TASK_FLASH)
	remove_task(id + TASK_CELEB)
	remove_task(id + TASK_SCOPE)
	remove_task(id + TASK_FLICK)
	remove_task(id + TASK_BREAK)
	remove_task(id + TASK_DROP)
	remove_task(id + TASK_BAD)
	remove_task(id + TASK_SPAWN)
	remove_task(id + TASK_REBUY)
	remove_task(id + TASK_GLANCE)
	remove_task(id + TASK_SPAWNCHK)
	remove_task(id + TASK_SWEEP)
	g_SweepLeft[id] = 0
	remove_task(id + TASK_TBAG)
	remove_task(id + TASK_SLASH)
	remove_task(id + TASK_SLASH + 32)
	for (new i = 0; i < 5; i++)
		remove_task(id + TASK_LEAVE + 32 * i)
	burst_restore(id)
	g_QLeft[id] = 0
	g_JumpsLeft[id] = 0
	g_DucksLeft[id] = 0
	g_GlanceLeft[id] = 0
	g_Press[id] = 0
	g_Force[id] = 0
	g_Impulse[id] = 0
	g_FlashOff[id] = false
	g_FlashLeft[id] = 0
	g_FireUntil[id] = 0.0
	g_PendWeapon[id] = 0
	g_Scoping[id] = false
	g_Out[id] = 0
	g_PuppetUntil[id] = 0.0
	g_FlickLeft[id] = 0
	g_DropFor[id] = 0
}

// A bot leaving: nothing of its carries over to whoever gets the slot next
public client_disconnected(id)
{
	clear_bot(id)
}

public client_putinserver(id)
{
	clear_bot(id)
	g_Busy[id] = g_FireBusy[id] = g_SeenAt[id] = g_SightAt[id] = g_KillAt[id] = g_Grace[id] = 0.0
	g_Sight[id] = false
	g_LifeKills[id] = 0
	g_FavBreak[id] = 0
	g_NextSlash[id] = 0.0
	g_Quiet[id] = random_float(8.0, 10.0)
	g_NextFun[id] = get_gametime() + random_float(20.0, 60.0)
	if (is_user_bot(id))
	{
		// its two habits, from its name (so a regular keeps them)
		new name[32], h = 5381
		get_user_name(id, name, charsmax(name))
		for (new i = 0; name[i]; i++)
			h = ((h * 33) ^ name[i]) & 0x7FFFFFFF
		g_Hash[id] = h
		g_Habit[id][0] = h % FUN_COUNT
		g_Habit[id][1] = (h / FUN_COUNT) % FUN_COUNT
		// like chattiness in gm_botchat: 45% hardly, 30% some, 22% a lot, 3% constantly
		new tier = (h / 997) % 100
		g_Fidget[id] = tier < 45 ? 0.3 : tier < 75 ? 0.8 : tier < 97 ? 1.6 : 2.0
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
	"fjkhwmOeZ7",    // celebrate
	"kjzhSn5"        // crouch spam
}
new const DISLIKES[FUN_COUNT][] = {
	"qoDdPr6", "", "cn", "", "tEL", "c", "qLotdDrP46", "qLotdDrP46", "", "qLt6", "LstH", "tE", "qLosrdP6", "qLotdDrP46E"
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
	if (is_habit(id, what))
		w *= 4.0
	return w * pers()
}

bool:is_habit(id, what)
{
	return g_Habit[id][0] == what || g_Habit[id][1] == what
}

bool:roll(id, what, Float:base)
{
	return random_float(0.0, 1.0) < floatmin(base * weight(id, what), 0.9)
}

// Free to do something: alive, a bot, no danger, no enemy in sight. calm: and it has been
// quiet a while (nothing seen, no shots of its own) - everything but celebrating needs that.
// Freeze time, and the first seconds after spawning in deathmatch, count as quiet
bool:free_bot(id, bool:calm = true)
{
	if (!is_user_alive(id) || !is_user_bot(id))
		return false
	new Float:now = get_gametime()
	if (now < g_Busy[id] || g_BombPlanted)
		return false
	// flashed: nobody fidgets blind
	if (get_ent_data_float(id, "CBasePlayer", "m_blindUntilTime") > now)
		return false
	if (calm && !g_Freeze && now >= g_Grace[id] && (now < g_FireBusy[id] || now - g_SeenAt[id] < g_Quiet[id]))
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
	// someone shot close by (a teammate too): the bots around are wary
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
	// (its own shots into the air after a won round, or at a window, don't count)
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
	// a new life: last round's dangers are over, and it's a little different each life
	new Float:now = get_gametime()
	g_Busy[id] = g_FireBusy[id] = g_SeenAt[id] = g_SightAt[id] = g_Grace[id] = 0.0
	g_Sight[id] = false
	g_LifeKills[id] = 0
	g_Quiet[id] = random_float(8.0, 10.0)
	g_NextFun[id] = now + random_float(20.0, 45.0)
	// (personality Plain: a quiet few still hop, crouch or look about as they come in)
	new bool:plain = !g_FunOn && plain_floor()
	if (!is_user_bot(id) || (!g_FunOn && !plain) || mode_ctx() != CTX_DM || !get_pcvar_num(g_pSpawn))
		return
	// deathmatch has no freeze time: a couple of seconds after spawning count as quiet, and
	// coming back in, nearly half do something like leaving spawn in a round (a jump or a
	// few, flicking weapons, the knife out a moment, crouch spam, a look about)
	g_Grace[id] = now + random_float(2.0, 4.0)
	// a quick look over the shoulder first (spawn kills are common), about half of them
	if (g_Fork && get_pcvar_num(g_pLook) && random_float(0.0, 1.0) < 0.5)
		set_task(random_float(0.15, 0.5), "task_spawn_check", id + TASK_SPAWNCHK)
	new Float:act = activity(persona(id))
	// (at most about half, whatever the personality: with a respawn every few seconds, more is
	// constant fidgeting)
	if (random_float(0.0, 1.0) >= (plain ? 0.3 : floatmin(floatclamp(0.46 + 0.2 * act, 0.3, 0.8) * pers(), 0.55)))
		return
	new Float:w[5]
	w[0] = 3.0 * taste(id, FUN_SPAWNJUMP)
	w[1] = !plain && get_pcvar_num(g_pWeapons) ? 2.5 * taste(id, FUN_QSPAM) : 0.0
	w[2] = plain ? 0.0 : 1.0 * taste(id, FUN_KNIFE)
	w[3] = 0.6 * taste(id, FUN_DUCK)
	w[4] = g_Fork && get_pcvar_num(g_pLook) ? 1.2 * taste(id, FUN_LOOK) : 0.0
	new Float:total = w[0] + w[1] + w[2] + w[3] + w[4]
	new Float:r = random_float(0.0, total), a
	for (a = 0; a < 4; a++)
	{
		r -= w[a]
		if (r <= 0.0 && w[a] > 0.0)
			break
	}
	if (a == 4 && w[4] <= 0.0)
		return
	if (a == 0)
	{
		g_JumpsLeft[id] = random_num(1, 3)
		g_NextJump[id] = now + random_float(0.3, 1.0)
	}
	else
	{
		new what[2]
		what[0] = a == 1 ? 'q' : a == 2 ? 'k' : a == 3 ? 'c' : 'g'
		set_task(random_float(0.3, 1.2), "task_spawn_do", id + TASK_SPAWN, what, 2)
	}
}

public task_spawn_check(tid)
{
	new id = tid - TASK_SPAWNCHK
	if (!free_bot(id))
		return
	new Float:me[3], Float:at[3], Float:view[3]
	eye_pos(id, me)
	pev(id, pev_v_angle, view)
	new Float:yaw = view[1] + 180.0 + random_float(-70.0, 70.0)
	at[0] = me[0] + floatcos(yaw, degrees) * 400.0
	at[1] = me[1] + floatsin(yaw, degrees) * 400.0
	at[2] = me[2]
	ls_look(id, at, random_float(0.3, 0.6), 0.0)
}

public task_spawn_do(const what[], tid)
{
	new id = tid - TASK_SPAWN
	switch (what[0])
	{
		case 'q': start_qspam(id)
		case 'k': if (free_bot(id) && holding_gun(id)) knife_out(id, random_float(0.6, 1.5))
		case 'c': start_ducks(id, random_num(2, 4))
		// a glance or two on the way (it's moving already)
		case 'g': if (g_Fork && free_bot(id)) { g_GlanceLeft[id] = random_num(1, 2); glance_step(id); }
	}
}

public ev_bomb_planted()
{
	g_BombPlanted = true
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2), hs = read_data(3)
	if (1 <= victim <= MaxClients)
		clear_bot(victim)
	if (!(1 <= killer <= MaxClients) || killer == victim || !is_user_bot(killer))
		return
	// the fight it just won is over: its own shots at that one no longer count
	g_KillAt[killer] = get_gametime()
	g_FireBusy[killer] = 0.0
	g_LifeKills[killer]++
	// a cheeky one teabags the body, once it's quiet (a pub, close to it)
	// (an enemy it killed, not a teammate)
	if (cheeky_ok() && rude_ok() && mode_ctx() == CTX_PUB && 1 <= victim <= MaxClients && (free_for_all() || get_user_team(victim) != get_user_team(killer))
		&& has_quirk(killer, Q_CHEEKY) && chance(killer, 0.25))
	{
		new Float:at[3]
		pev(killer, pev_origin, at)
		pev(victim, pev_origin, g_BodyAt[killer])
		if (get_distance_f(at, g_BodyAt[killer]) < 220.0)
		{
			set_task(random_float(0.6, 1.2), "task_tbag", killer + TASK_TBAG)
			return
		}
	}
	if (!fun_allowed() || !get_pcvar_num(g_pWeapons) || task_exists(killer + TASK_KILL))
		return
	// a newer player's habit: a reload after every kill, whoever's still around
	if (get_pcvar_num(g_pBad) && has_quirk(killer, Q_BADRELOAD) && random_float(0.0, 1.0) < 0.6 * floatmin(pers(), 1.5))
	{
		set_task(random_float(0.25, 0.6), "task_bad_reload", killer + TASK_KILL)
		return
	}
	// after a kill, once it's quiet again: a reload it doesn't need, or a weapon flick (more
	// after a headshot or the third kill of a life, and in deathmatch)
	new Float:more = floatmin(1.0 + 0.5 * float(g_LifeKills[killer] - 1), 2.5) * (hs ? 1.5 : 1.0)
	if (mode_ctx() == CTX_DM)
		more *= 1.5
	if (roll(killer, FUN_RELOAD, 0.15))
		set_task(random_float(0.6, 1.4), "task_after_kill", killer + TASK_KILL, "r", 1)
	else if (roll(killer, FUN_QSPAM, 0.06 * more))
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

// The reload straight after the kill: no waiting for it to be quiet
public task_bad_reload(tid)
{
	new id = tid - TASK_KILL
	if (is_user_alive(id) && is_user_bot(id) && needs_reload(id))
		g_Force[id] |= IN_RELOAD
}

// Freeze time starts (before the dead respawn: the rolls wait a moment)
public ev_new_round()
{
	g_Freeze = true
	g_BombPlanted = false
	g_DropDone = false
	g_Asked[1] = g_Asked[2] = false
	g_Breaks = 0
	g_FunOn = fun_allowed()
	set_task(0.3, "task_freeze_rolls")
}

// What a bot does in freeze time, in the order of the plan's letters (task_freeze_do)
enum { ACT_GLANCE, ACT_QSPAM, ACT_FLICKS, ACT_KNIFE, ACT_JUMP, ACT_DUCK, ACT_FLASH, ACT_SALUTE, ACT_MATES, ACT_NOD, ACT_SPIN, ACT_WIGGLE, ACT_PAN, ACT_COUNT }
new const ACT_LETTER[ACT_COUNT] = { 'g', 'q', 'h', 'k', 'j', 'c', 'f', 's', 'm', 'n', 'p', 'w', 'a' }

public task_freeze_rolls()
{
	new ctx = mode_ctx()
	// a match: the quiet things only (looking about, a weapon check); a pub: everything
	new bool:match = ctx == CTX_MATCH && match_freeze_allowed()
	new bool:plain = !match && plain_floor()
	if (!match && !plain && (!fun_allowed() || ctx != CTX_PUB))
		return
	new bool:spawn = get_pcvar_num(g_pSpawn) != 0
	// the freeze time left (this task runs 0.3 s in)
	new Float:left = get_cvar_float("mp_freezetime") - 0.3
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		// broke and nothing but a pistol: asking the team for a drop (after the others buy)
		if (!match && !plain && get_pcvar_num(g_pDrop) && g_Chat && g_BuyMode && !primary_of(id) && cs_get_user_money(id) < 2000
			&& has_quirk(id, Q_ASKS) && chance(id, 0.3))
			set_task(random_float(2.0, 4.5), "task_ask_drop", id + TASK_DROP)
		if (spawn && left > 1.0)
			plan_freeze(id, left, match, plain)
	}
}

// A few things through freeze time, one after another: most people never stand still
// (about four in five do something; the quiet ones less, kids and jokers more), each in
// its own way (its personality's likes, its habits and quirks)
plan_freeze(id, Float:left, bool:match, bool:plain = false)
{
	new Float:act = activity(persona(id))
	new Float:any = plain ? 0.3 : floatclamp((match ? 0.45 : 0.55) + 0.25 * act, 0.4, 0.97) * floatmin(pers(), 1.3)
	if (random_float(0.0, 1.0) >= floatmin(any, 0.97))
		return
	new Float:w[ACT_COUNT]
	// (Plain: the quiet things, and a hop or a crouch; nothing that's a quirk)
	freeze_weights(id, w, match || plain)
	if (plain)
	{
		w[ACT_FLICKS] = 0.0
		w[ACT_JUMP] = 0.8
		w[ACT_DUCK] = 0.8
		act = 0.0
	}
	// one, two or three things (fidgety ones more), from 0.4 s in to the end of freeze time
	new count = 1
	if (random_float(0.0, 1.0) < 0.35 * act * g_Fidget[id] + (match ? 0.0 : 0.15))
		count++
	if (!match && random_float(0.0, 1.0) < 0.2 * act * g_Fidget[id])
		count++
	// a long freeze time (15, 30 seconds): more to fill it, about one more thing a 7 seconds
	if (left > 9.0)
		count += random_num(0, floatround((left - 6.0) / 7.0, floatround_floor))
	count = min(count, FREEZE_SLOTS)
	new plan[4], n
	new Float:at = random_float(0.4, 1.6)
	new Float:slot = (left - at) / float(count)
	for (new i = 0; i < count && at < left - 0.6; i++)
	{
		new a = pick_act(w)
		if (a < 0)
			break
		// the same thing twice in a freeze time is fine, not straight after itself
		w[a] *= 0.3
		plan[0] = ACT_LETTER[a]
		plan[1] = 0
		set_task(at, "task_freeze_do", id + TASK_FREEZE + 32 * i, plan, 2)
		at += slot * random_float(0.7, 1.1)
		n++
	}
}

// How much each freeze time thing appeals to this bot (0: it can't or won't)
freeze_weights(id, Float:w[ACT_COUNT], bool:match)
{
	new bool:look = g_Fork && get_pcvar_num(g_pLook) != 0
	new bool:guns = get_pcvar_num(g_pWeapons) != 0
	w[ACT_GLANCE] = look ? 3.0 * taste(id, FUN_LOOK) : 0.0
	w[ACT_QSPAM] = guns ? (has_quirk(id, Q_SWITCHY) ? 6.0 : 2.0) * taste(id, FUN_QSPAM) : 0.0
	w[ACT_FLICKS] = look && has_quirk(id, Q_FLICKS) ? 4.0 : 0.0
	// the ways people move the mouse while they wait, each its own sort: a quick left-right
	// wiggle (warming up the aim: the tryhards and good players), a slow pan across the map
	// (the quiet, patient ones), fast nodding up and down and spinning round (kids, jokers)
	new p = persona(id), l = level(id)
	w[ACT_WIGGLE] = look ? (l >= 2 ? 1.6 : 0.3) * (has(p, "tE7N") ? 2.0 : 1.0) : 0.0
	w[ACT_PAN] = look ? (has(p, "qLd6c4") ? 2.5 : 0.6) : 0.0
	if (match)
	{
		// waiting for a match to start: looking about, flicking heads, a weapon check
		w[ACT_QSPAM] *= 0.5
		return
	}
	w[ACT_NOD] = look ? (has(p, "kjzhS5") ? 2.0 : 0.3) : 0.0
	w[ACT_SPIN] = look ? (has(p, "jzhk") ? 1.2 : 0.12) : 0.0
	w[ACT_KNIFE] = 1.2 * taste(id, FUN_KNIFE)
	w[ACT_JUMP] = 1.0 * taste(id, FUN_FREEZEJUMP)
	w[ACT_DUCK] = 0.8 * taste(id, FUN_DUCK)
	w[ACT_FLASH] = get_pcvar_num(g_pFlashlight) && flashlight_allowed() ? 0.35 * taste(id, FUN_FLASHLIGHT) : 0.0
	w[ACT_SALUTE] = g_Fork && get_pcvar_num(g_pSalute) ? 0.4 * taste(id, FUN_SALUTE) : 0.0
	w[ACT_MATES] = look && get_pcvar_num(g_pBad) && has_quirk(id, Q_LOOKMATES) ? 2.0 : 0.0
}

pick_act(const Float:w[ACT_COUNT])
{
	new Float:total
	for (new a = 0; a < ACT_COUNT; a++)
		total += w[a]
	if (total <= 0.0)
		return -1
	new Float:r = random_float(0.0, total)
	for (new a = 0; a < ACT_COUNT; a++)
	{
		r -= w[a]
		if (r <= 0.0 && w[a] > 0.0)
			return a
	}
	return -1
}

// A thing's appeal to this bot alone (its personality's likes and its own habits), apart
// from how fidgety it is
Float:taste(id, what)
{
	new p = persona(id)
	new Float:t = 1.0
	if (has(p, LIKES[what]))
		t *= 3.0
	else if (has(p, DISLIKES[what]))
		t *= 0.2
	if (is_habit(id, what))
		t *= 3.0
	return t
}

// Freeze time in a match (match mode, a scrim, Matchmaking on Classic): only with bot fun
// on, at a personality above 0, and not with the plain CZ bots
bool:match_freeze_allowed()
{
	return get_pcvar_num(g_pFun) && pers() > 0.0 && !(g_pStyle && get_pcvar_num(g_pStyle) == STYLE_CLASSIC)
}

// The freeze time look chains (glances, sweeps, head flicks) stop where they are
stop_looks(id)
{
	g_GlanceLeft[id] = 0
	g_SweepLeft[id] = 0
	g_FlickLeft[id] = 0
	remove_task(id + TASK_GLANCE)
	remove_task(id + TASK_SWEEP)
	remove_task(id + TASK_FLICK)
}

public task_freeze_do(const what[], tid)
{
	new id = (tid - TASK_FREEZE) % 32
	if (!id)
		id = 32
	if (!g_Freeze)
		return
	// one thing at a time: whatever look it was in the middle of stops for the next
	stop_looks(id)
	switch (what[0])
	{
		case 'g': start_glances(id)
		case 'j': freeze_jump(id)
		case 'k': knife_flick(id)
		// the switchy ones flick all freeze time
		case 'q': start_qspam(id, false, has_quirk(id, Q_SWITCHY))
		case 'c': start_ducks(id, random_num(2, 6))
		case 'f': flashlight(id)
		case 's': salute(id)
		case 'h': start_flicks(id)
		case 'm': stare_mate(id)
		case 'n': nod_fast(id)
		case 'p': start_sweep(id, SWEEP_SPIN)
		case 'w': start_sweep(id, SWEEP_WIGGLE)
		case 'a': start_sweep(id, SWEEP_PAN)
	}
}

start_ducks(id, count)
{
	if (!free_bot(id))
		return
	g_DucksLeft[id] = count
	g_NextDuck[id] = get_gametime()
}

// Freeze time is over: about half do something on the way out of spawn (jumping out, a
// teammate next to a jumper sometimes joining in, flicking weapons on the run, the knife
// out for the run, crouch spam, a look back, or still finishing up and leaving late); a
// newer player pulls out a grenade and puts it away again
public ev_round_start()
{
	g_Freeze = false
	g_RoundStartAt = get_gametime()
	new Float:now = get_gametime()
	// freeze time's looks, nods and stares end with it (nobody runs out of spawn nodding)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (is_user_alive(i) && is_user_bot(i))
		{
			stop_looks(i)
			stop_all(i, now)
		}
	}
	new bool:plain = plain_floor()
	if (!plain && (!fun_allowed() || mode_ctx() != CTX_PUB))
		return
	if (!plain && get_pcvar_num(g_pBad))
	{
		for (new id = 1; id <= MaxClients; id++)
			if (is_user_alive(id) && is_user_bot(id) && user_has_weapon(id, CSW_HEGRENADE) && has_quirk(id, Q_BADNADE) && chance(id, 0.3))
				set_task(random_float(2.0, 7.0), "task_bad_nade", id + TASK_BAD)
	}
	if (!get_pcvar_num(g_pSpawn))
		return
	new bool:jumper[33]
	// (a slash at a teammate still next to it: nobody can attack in freeze time, so now)
	// (never with friendly fire on, nor in free for all, where a "teammate" is really an opponent)
	new bool:slashes = !plain && g_Fork && cheeky_ok() && !get_cvar_num("mp_friendlyfire") && !free_for_all()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!free_bot(id, false))
			continue
		if (slashes && has_quirk(id, Q_CHEEKY) && chance(id, 0.3))
		{
			slash_mate(id)
			if (task_exists(id + TASK_SLASH))
				continue
		}
		new Float:act = activity(persona(id))
		if (random_float(0.0, 1.0) >= (plain ? 0.3 : floatmin(floatclamp(0.46 + 0.2 * act, 0.3, 0.8) * pers(), 0.9)))
			continue
		new Float:w[6]
		w[0] = 3.0 * taste(id, FUN_SPAWNJUMP)
		w[1] = get_pcvar_num(g_pWeapons) ? 2.0 * taste(id, FUN_QSPAM) : 0.0
		w[2] = 1.0 * taste(id, FUN_KNIFE)
		w[3] = 0.6 * taste(id, FUN_DUCK)
		w[4] = g_Fork && get_pcvar_num(g_pLook) ? 1.5 * taste(id, FUN_LOOK) : 0.0
		// (leaving late: now and then, and only a moment; standing in spawn looks like a bot)
		w[5] = g_Fork ? 0.4 : 0.0
		// (Plain: a hop, a crouch or a look back, nothing that's a quirk)
		if (plain)
		{
			w[0] = 1.0
			w[1] = w[2] = w[5] = 0.0
			w[3] = 1.0
			w[4] = g_Fork && get_pcvar_num(g_pLook) ? 1.0 : 0.0
		}
		new Float:total = w[0] + w[1] + w[2] + w[3] + w[4] + w[5]
		if (total <= 0.0)
			continue
		new Float:r = random_float(0.0, total), a
		for (a = 0; a < 5; a++)
		{
			r -= w[a]
			if (r <= 0.0 && w[a] > 0.0)
				break
		}
		if (a == 5 && w[5] <= 0.0)
			continue
		if (a == 0)
		{
			g_JumpsLeft[id] = random_num(2, 6)
			g_NextJump[id] = now + random_float(0.2, 0.8)
			jumper[id] = true
		}
		else
			set_task(a == 5 ? 0.05 : random_float(0.3, 1.8), "task_leave", id + TASK_LEAVE + 32 * (a - 1))
	}
	new Float:a[3], Float:b[3]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!jumper[id])
			continue
		pev(id, pev_origin, a)
		for (new m = 1; m <= MaxClients; m++)
		{
			if (m == id || g_JumpsLeft[m] > 0 || get_user_team(m) != get_user_team(id) || !has_quirk(m, Q_TEAMJUMP) || !free_bot(m, false))
				continue
			pev(m, pev_origin, b)
			if (get_distance_f(a, b) < 350.0 && chance(m, 0.4))
			{
				g_JumpsLeft[m] = random_num(2, 4)
				g_NextJump[m] = g_NextJump[id] + random_float(0.15, 0.6)
			}
		}
	}
}

// One of the ways out of spawn (task_leave's slot: 0 flicks, 1 knife, 2 crouch spam, 3 a
// look back, 4 leaving late)
public task_leave(tid)
{
	new slot = (tid - TASK_LEAVE - 1) / 32
	new id = tid - TASK_LEAVE - 32 * slot
	if (!free_bot(id, false) || get_gametime() - g_RoundStartAt > 6.0)
		return
	switch (slot)
	{
		case 0: start_qspam(id, false, false)
		case 1: if (holding_gun(id)) knife_out(id, random_float(2.0, 5.0))
		case 2: start_ducks(id, random_num(2, 4))
		case 3:
		{
			// a look back at the team, or behind it
			new Float:me[3], Float:at[3], Float:view[3]
			eye_pos(id, me)
			pev(id, pev_v_angle, view)
			new mate = visible_mate(id, 1200.0, 0)
			if (mate)
				pev(mate, pev_origin, at)
			else
			{
				new Float:yaw = view[1] + 180.0 + random_float(-50.0, 50.0)
				at[0] = me[0] + floatcos(yaw, degrees) * 400.0
				at[1] = me[1] + floatsin(yaw, degrees) * 400.0
				at[2] = me[2]
			}
			ls_look(id, at, random_float(0.5, 1.0), 0.0)
		}
		case 4:
		{
			// still sorting out its gear: looking down at it a moment before it goes
			new Float:me[3], Float:at[3], Float:view[3]
			eye_pos(id, me)
			pev(id, pev_v_angle, view)
			at[0] = me[0] + floatcos(view[1], degrees) * 90.0
			at[1] = me[1] + floatsin(view[1], degrees) * 90.0
			at[2] = me[2] - 60.0
			new Float:secs = random_float(0.5, 1.3)
			ls_look(id, at, secs, secs)
		}
	}
}

public ev_t_win()
{
	round_end(1)
}

public ev_ct_win()
{
	round_end(2)
}

public ev_draw()
{
	round_end(0)
}

round_end(team)
{
	// the round is over: the bomb doesn't matter any more, nor a gift on its way
	g_BombPlanted = false
	for (new id = 1; id <= MaxClients; id++)
		if (g_GiftBox[id] && g_GiftBot[id])
			gift_done(id)
	// (a deathmatch-type mode has no rounds to win: a round end there is a time limit or a
	// wipe, nothing to celebrate or show off over)
	if (mode_ctx() == CTX_DM)
		return
	celebrate(team)
	breakables()
	show_off()
}

// The round's decided: a couple of the cheeky ones jump about (and some knife out) until the
// next one. (Loner-Strike's modes have no warmup: this is the round-end version of it)
show_off()
{
	if (!cheeky_ok() || mode_ctx() != CTX_PUB)
		return
	new n
	for (new id = 1; id <= MaxClients && n < 2; id++)
	{
		if (!free_bot(id, false) || !has_quirk(id, Q_CHEEKY) || !chance(id, 0.4))
			continue
		n++
		g_JumpsLeft[id] = random_num(3, 7)
		g_NextJump[id] = get_gametime() + random_float(0.5, 1.5)
		if (holding_gun(id) && random_num(0, 1))
			knife_out(id, random_float(2.0, 4.0))
	}
}

bool:cheeky_ok()
{
	return g_FunOn && get_pcvar_num(g_pCheeky) != 0
}

// The lobby mood (gm_mood, the app's Lobby mood): 0 friendly, 1 normal, 2 toxic; gm_people's
// (one rule for every plugin: a LAN party counts as friendly), else gm_mood's own
lobby_mood()
{
	if (g_People)
		return people_mood()
	new p = get_cvar_pointer("gm_mood")
	new m = p ? clamp(get_pcvar_num(p), 0, 2) : 1
	// (a LAN party is friends: friendly unless the lobby's set to something else)
	return m == 1 && get_cvar_num("gm_botstyle") == 9 ? 0 : m
}

Float:mood_mult()
{
	switch (lobby_mood())
	{
		case 0: return 0.3
		case 2: return 2.0
	}
	return 1.0
}

// Free for all (mp_freeforall): everyone's an opponent, whatever team they're listed in
bool:free_for_all()
{
	return g_pFreeForAll && get_pcvar_num(g_pFreeForAll) != 0
}

// Disrespect (teabagging): only when "Bots can be rude" is on (gm_hb_disrespect, off unless
// turned on: a hard cap, whatever the mood), and never in a friendly lobby. A toxic lobby
// with it on does it more (mood_mult)
bool:rude_ok()
{
	new p = get_cvar_pointer("gm_hb_disrespect")
	if (!p || !get_pcvar_num(p))
		return false
	return lobby_mood() != 0
}

// Teabagging: once it's quiet after the kill, crouch spam standing over the body, looking down
// at it (danger stops it at once, like everything else here)
public task_tbag(tid)
{
	new id = tid - TASK_TBAG
	if (!is_user_alive(id) || !is_user_bot(id))
		return
	// gm_humanbots is showing off for this kill already: one teabag, not two
	if (g_HumanBots && hb_bot_showing_off(id))
		return
	if (!free_after_kill(id))
	{
		if (get_gametime() - g_KillAt[id] < 6.0)
			set_task(random_float(0.3, 0.6), "task_tbag", tid)
		return
	}
	new Float:at[3]
	pev(id, pev_origin, at)
	if (get_distance_f(at, g_BodyAt[id]) > 220.0)
		return
	new Float:secs = random_float(1.2, 2.2)
	if (g_Fork)
		ls_look(id, g_BodyAt[id], secs, secs)
	g_DucksLeft[id] = random_num(4, 8)
	g_NextDuck[id] = get_gametime()
}

// Leaving spawn: the knife out and a slash or two at the teammate right next to it (only with
// friendly fire off: it does nothing but look silly)
slash_mate(id)
{
	if (!g_Fork || !free_bot(id, false) || !holding_gun(id) || get_cvar_num("mp_friendlyfire") || free_for_all())
		return
	new mate = visible_mate(id, 90.0, 0)
	if (!mate)
		return
	new Float:at[3]
	pev(mate, pev_origin, at)
	ls_look(id, at, 1.4, 0.0)
	knife_out(id, random_float(1.6, 2.4))
	set_task(0.45, "task_mate_slash", id + TASK_SLASH)
	if (random_num(0, 1))
		set_task(0.95, "task_mate_slash", id + TASK_SLASH + 32)
}

public task_mate_slash(tid)
{
	new id = (tid - TASK_SLASH - 1) % 32 + 1
	if (is_user_alive(id) && held_weapon(id) == CSW_KNIFE && !get_cvar_num("mp_friendlyfire") && !free_for_all() && free_bot(id, false))
		g_Press[id] |= IN_ATTACK
}

// The winners: a few shoot into the air
celebrate(team)
{
	if (!g_Fork || !fun_allowed() || mode_ctx() != CTX_PUB || !get_pcvar_num(g_pCelebrate))
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

// After the round: a couple of the ones who like breaking things shoot the glass in view
breakables()
{
	if (!g_Fork || !fun_allowed() || mode_ctx() != CTX_PUB || !get_pcvar_num(g_pBreak))
		return
	for (new id = 1; id <= MaxClients && g_Breaks < 2; id++)
	{
		if (task_exists(id + TASK_CELEB) || !free_bot(id, false) || !has_quirk(id, Q_BREAK) || !chance(id, 0.35))
			continue
		g_Breaks++
		set_task(random_float(0.5, 2.0), "task_break_shoot", id + TASK_BREAK)
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
	new c = mode_ctx()
	// slashing a crate on the way past happens when it's passed, not one bot at a time
	if (c == CTX_PUB && get_pcvar_num(g_pBreak))
		slash_check(now)
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
	// (a pub a little livelier than deathmatch: more of them, more often)
	g_NextAny = now + (c == CTX_PUB ? random_float(2.0, 5.0) : random_float(3.0, 8.0)) / floatmax(pers(), 0.25)
	g_NextFun[id] = now + (c == CTX_PUB ? random_float(18.0, 45.0) : random_float(25.0, 60.0))

	// deathmatch: only the quick things with the gun in hand (more weapon flicks)
	if (c == CTX_DM)
	{
		if (!get_pcvar_num(g_pWeapons))
			return
		switch (held_weapon(id))
		{
			case CSW_AWP, CSW_SCOUT, CSW_SG550, CSW_G3SG1: if (roll(id, FUN_SCOPE, 0.15)) { scope(id); return; }
			case CSW_GLOCK18, CSW_FAMAS: if (roll(id, FUN_BURST, 0.12)) { burst(id); return; }
		}
		if (roll(id, FUN_QSPAM, 0.09))
			start_qspam(id)
		return
	}
	if (get_pcvar_num(g_pBad) && bad_habit(id, now))
		return
	if (get_pcvar_num(g_pWeapons))
	{
		new weapon = held_weapon(id)
		switch (weapon)
		{
			// a single press, now and then, from the ones it's a habit of: the bot's brain
			// puts the silencer back the way it likes it a few seconds later
			case CSW_M4A1, CSW_USP: if (is_habit(id, FUN_SILENCER) && roll(id, FUN_SILENCER, 0.04)) { silencer(id); return; }
			case CSW_AWP, CSW_SCOUT, CSW_SG550, CSW_G3SG1: if (roll(id, FUN_SCOPE, 0.15)) { scope(id); return; }
			case CSW_GLOCK18, CSW_FAMAS: if (roll(id, FUN_BURST, 0.12)) { burst(id); return; }
		}
		if (roll(id, FUN_QSPAM, 0.10)) { start_qspam(id); return; }
		if (roll(id, FUN_SLOTS, 0.06)) { start_slots(id); return; }
	}
	if (g_Fork && get_pcvar_num(g_pLook) && roll(id, FUN_LOOK, 0.08))
	{
		look_around(id)
		return
	}
	// in the round only on a dark map (freeze time: anywhere)
	if (g_DarkMap && get_pcvar_num(g_pFlashlight) && flashlight_allowed() && roll(id, FUN_FLASHLIGHT, 0.02))
		flashlight(id)
}

// A newer player's bad habit, for the bots that have one: flicking weapons for no reason,
// the knife out in the middle of the round, staring at a teammate. True if it did one
bool:bad_habit(id, Float:now)
{
	if (has_quirk(id, Q_BADSWITCH) && get_pcvar_num(g_pWeapons) && chance(id, 0.25))
	{
		start_qspam(id)
		return true
	}
	if (has_quirk(id, Q_BADKNIFE) && now - g_RoundStartAt > 15.0 && holding_gun(id) && chance(id, 0.2))
	{
		knife_out(id, random_float(2.0, 4.0))
		return true
	}
	if (g_Fork && has_quirk(id, Q_LOOKMATES) && get_pcvar_num(g_pLook) && chance(id, 0.2))
	{
		new mate = visible_mate(id, 1200.0, 0)
		if (mate)
		{
			new Float:at[3]
			eye_pos(mate, at)
			ls_look(id, at, random_float(0.6, 1.2), 0.0)
			return true
		}
	}
	return false
}

// ---- the things themselves ----------------------------------------------------------------

public fw_cmdstart(id, uc, seed)
{
	if (!is_user_bot(id) || !is_user_alive(id))
		return FMRES_IGNORED

	new Float:now = get_gametime()
	new buttons = get_uc(uc, UC_Buttons)
	// its own shooting: busy (not the last shots of a fight it just won, nor ours)
	if (buttons & IN_ATTACK && now >= g_FireUntil[id] && now - g_KillAt[id] >= 0.5)
		g_FireBusy[id] = floatmax(g_FireBusy[id], now + 3.0)
	// using something (defusing, a hostage): busy
	if (buttons & IN_USE)
		g_FireBusy[id] = floatmax(g_FireBusy[id], now + 3.0)
	// keeps its eyes open (and its memory of danger up to date) while fun is on
	new bool:danger = g_FunOn && (sees_enemy(id) || now < g_Busy[id])

	// anything of ours under way stops the moment there's danger, or gm_humanbots has it
	// afk or holding an angle
	new bool:stop = danger
	if (!stop && g_HumanBots && (g_Press[id] || g_Impulse[id] || g_JumpsLeft[id] > 0 || g_DucksLeft[id] > 0 || g_Scoping[id] || g_Out[id] || g_FlickLeft[id] || g_GlanceLeft[id] || g_SweepLeft[id]) && hb_bot_idle(id))
		stop = true
	if (stop)
		stop_all(id, now)

	new changed
	// a bad habit's press goes through whatever's going on
	if (g_Force[id])
	{
		buttons |= g_Force[id]
		g_Force[id] = 0
		changed = 1
	}
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
	// crouch spam: short taps, each its own length
	if (now < g_DuckUntil[id] && !stop)
	{
		buttons |= IN_DUCK
		changed = 1
	}
	else if (g_DucksLeft[id] > 0 && now >= g_NextDuck[id])
	{
		if (stop)
			g_DucksLeft[id] = 0
		else
		{
			g_DucksLeft[id]--
			g_DuckUntil[id] = now + random_float(0.12, 0.3)
			g_NextDuck[id] = g_DuckUntil[id] + random_float(0.12, 0.4)
			buttons |= IN_DUCK
			changed = 1
		}
	}
	if (now < g_FireUntil[id])
	{
		// only while it's really looking where it should with a gun: up into the air, or at
		// the glass. Anything else (the look was refused, a fight, a grenade in hand) and it
		// doesn't fire
		if (stop || now < g_FireBusy[id] || !holding_gun(id))
			g_FireUntil[id] = 0.0
		else if (aimed(id))
		{
			// short bursts with gaps, each its own length (at glass: taps)
			if (now >= g_FireFlip[id])
			{
				g_FireOn[id] = !g_FireOn[id]
				if (g_FireKind[id] == FIRE_AT)
					g_FireFlip[id] = now + (g_FireOn[id] ? random_float(0.05, 0.1) : random_float(0.2, 0.45))
				else
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

// Danger: whatever of ours it had going ends, and things are put back the way they were
// (the zoom, the burst mode, the gun in hand, its view)
stop_all(id, Float:now)
{
	if (g_Scoping[id])
		scope_abort(id)
	if (g_BurstEnt[id])
		burst_restore(id)
	if (g_PuppetUntil[id] > now)
	{
		server_cmd("ls_bot %d release bf", id)
		g_PuppetUntil[id] = 0.0
	}
	if (g_Out[id])
		out_back(id)
	if (g_FlickLeft[id])
	{
		g_FlickLeft[id] = 0
		remove_task(id + TASK_FLICK)
	}
	if (g_GlanceLeft[id])
	{
		g_GlanceLeft[id] = 0
		remove_task(id + TASK_GLANCE)
	}
	if (g_SweepLeft[id])
	{
		g_SweepLeft[id] = 0
		remove_task(id + TASK_SWEEP)
	}
	g_DucksLeft[id] = 0
}

// Looking where its shots should go: up into the sky, or at the point it's shooting
bool:aimed(id)
{
	new Float:view[3]
	pev(id, pev_v_angle, view)
	if (g_FireKind[id] == FIRE_SKY)
		return view[0] < -35.0
	new Float:eye[3], Float:dir[3], Float:fwd[3]
	eye_pos(id, eye)
	for (new i = 0; i < 3; i++)
		dir[i] = g_FireAt[id][i] - eye[i]
	new Float:len = vector_length(dir)
	if (len < 1.0)
		return false
	angle_vector(view, ANGLEVECTOR_FORWARD, fwd)
	new Float:dot = (dir[0] * fwd[0] + dir[1] * fwd[1] + dir[2] * fwd[2]) / len
	return dot > 0.9945             // within about 6 degrees
}

// The gun back out after a knife or grenade of ours (only if that's what's in hand: the
// bot's brain may have picked something else meanwhile)
out_back(id)
{
	if (is_user_alive(id) && g_Out[id] && held_weapon(id) == g_Out[id])
		engclient_cmd(id, "lastinv")
	g_Out[id] = 0
	remove_task(id + TASK_KNIFE)
	remove_task(id + TASK_BAD)
	remove_task(id + TASK_BREAK)
}

// A single press of the silencer, from the ones it's a habit of. The bot's brain puts the
// silencer back the way it likes it once it's been calm a few seconds (SilencerCheck), so
// it reads as someone checking it, never as a fight over it
silencer(id)
{
	g_PendWeapon[id] = held_weapon(id)
	g_Press[id] |= IN_ATTACK2
}

// In, further in, out again (all four have two zoom levels). However it goes, it ends
// unzoomed: scope_end, or scope_abort when something comes up
scope(id)
{
	g_PendWeapon[id] = held_weapon(id)
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
	if (is_user_alive(id) && held_weapon(id) == g_PendWeapon[id] && cs_get_user_zoom(id) > CS_SET_NO_ZOOM)
		cs_set_user_zoom(id, CS_SET_NO_ZOOM, 1)
}

public task_attack2(tid)
{
	new id = tid - TASK_ATTACK2
	if (free_bot(id) && held_weapon(id) == g_PendWeapon[id])
		g_Press[id] |= IN_ATTACK2
	else if (g_Scoping[id])
		scope_abort(id)
}

// The burst on (or off) and back a moment later. The way it was is remembered, so it's put
// back whatever happens (a fight, the round ending)
burst(id)
{
	new ent = active_item(id)
	if (!ent)
		return
	g_PendWeapon[id] = held_weapon(id)
	g_BurstEnt[id] = ent
	g_BurstWas[id] = cs_get_weapon_burst(ent)
	g_Press[id] |= IN_ATTACK2
	set_task(random_float(0.8, 2.0), "task_burst_back", id + TASK_ATTACK2)
}

public task_burst_back(tid)
{
	burst_restore(tid - TASK_ATTACK2)
}

burst_restore(id)
{
	new ent = g_BurstEnt[id]
	if (!ent)
		return
	g_BurstEnt[id] = 0
	remove_task(id + TASK_ATTACK2)
	g_Press[id] &= ~IN_ATTACK2
	// (still that gun: an edict number gets used again)
	if (pev_valid(ent) && cs_get_weapon_id(ent) == g_PendWeapon[id] && cs_get_weapon_burst(ent) != g_BurstWas[id])
		cs_set_weapon_burst(ent, g_BurstWas[id])
}

// The weapon in its hands (CBasePlayer::m_pActiveItem, in AMXX 1.10's gamedata), or 0
active_item(id)
{
	new ent = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	return (ent > 0 && pev_valid(ent)) ? ent : 0
}

// A full magazine for each gun (CSW_* order)
new const MAX_CLIP[] = { 0, 13, 0, 10, 0, 7, 0, 30, 30, 0, 30, 20, 25, 30, 35, 25, 12, 20, 10, 30, 100, 8, 30, 30, 20, 0, 7, 30, 30, 0, 50 }

// What it has in hand: its active item. (get_user_weapon goes stale for bots: it comes from
// the CurWeapon message, and the engine only sends players' client data to real clients)
held_item(id)
{
	new item = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	return (item > 0 && pev_valid(item)) ? item : 0
}

held_weapon(id)
{
	new item = held_item(id)
	return item ? cs_get_weapon_id(item) : 0
}

// A gun in hand whose magazine isn't full, with ammo to fill it
bool:needs_reload(id)
{
	if (!holding_gun(id))
		return false
	new item = held_item(id)
	if (item <= 0)
		return false
	new w = cs_get_weapon_id(item)
	new clip = cs_get_weapon_ammo(item), ammo = cs_get_user_bpammo(id, w)
	return w > 0 && w < sizeof MAX_CLIP && clip > 0 && clip < MAX_CLIP[w] && ammo > 0
}

do_reload(id)
{
	if (needs_reload(id))
		g_Press[id] |= IN_RELOAD
}

// Flicks between its gun and the last weapon it held (lastinv): only between guns and the
// knife, never onto a grenade (the bot's brain pulls the pin on one in hand) or the bomb.
// It always ends on the gun it started with (an even number of flicks).
// after_kill: the quiet check after a kill (free_after_kill) was done already. many: a
// long, quick run of them (freeze time, for the ones who can't stop). Freeze time: the
// switchy ones start another run when one ends, until the round starts
start_qspam(id, bool:after_kill = false, bool:many = false)
{
	if (!after_kill && !free_bot(id))
		return
	// a knife out is gm_humanbots' (a knife run, a show-off): flicks would undo it
	if (!holding_gun(id))
		return
	// (CBasePlayer::m_pLastItem is in AMXX 1.10's cstrike gamedata)
	new last = get_ent_data_entity(id, "CBasePlayer", "m_pLastItem")
	if (last <= 0 || !pev_valid(last))
		return
	new to = cs_get_weapon_id(last)
	if (!to || to == CSW_C4 || is_nade(to) || !user_has_weapon(id, to))
		return
	g_QFrom[id] = held_weapon(id)
	g_QTo[id] = to
	g_QDone[id] = 0
	g_QKill[id] = after_kill
	// (people spam it: several in a row, quickly; the ones who can't stop, a long run)
	g_QLeft[id] = 2 * (many ? random_num(6, 12) : random_num(2, 5))
	g_QGap[id] = many ? 0.16 : 0.24
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
	if (held_weapon(id) != ((g_QDone[id] & 1) ? g_QTo[id] : g_QFrom[id]))
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
		set_task(random_float(g_QGap[id] * 0.45, g_QGap[id]), "task_qspam", id + TASK_QSPAM)
	// the switchy ones keep at it through freeze time, a short pause between runs
	else if (g_Freeze && has_quirk(id, Q_SWITCHY) && random_num(0, 3))
		set_task(random_float(0.4, 1.2), "task_qspam_again", id + TASK_QSPAM)
}

public task_qspam_again(tid)
{
	new id = tid - TASK_QSPAM
	if (g_Freeze && is_user_alive(id))
		start_qspam(id, false, true)
}

start_slots(id)
{
	if (!free_bot(id) || !holding_gun(id))
		return
	// knife, pistol, back to the gun it had (by name: slot1..slot4 are keys in the player's
	// own game, the server never sees them). Not a grenade: the bot's brain pulls the pin
	// on one in hand
	new held = held_weapon(id)
	new steps[3][24], n
	copy(steps[n++], charsmax(steps[]), "weapon_knife")
	new weapons[32], count, pistol
	get_user_weapons(id, weapons, count)
	for (new i = 0; i < count && !pistol; i++)
		if (is_pistol(weapons[i]))
			pistol = weapons[i]
	g_SlotPistol[id] = 0
	if (pistol && pistol != held)
	{
		get_weaponname(pistol, steps[n++], charsmax(steps[]))
		g_SlotPistol[id] = pistol
	}
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

// The last step (its own gun again) happens even if something came up meanwhile, as long
// as what's in hand is one of the cycle's (the knife or that pistol): anything else is the
// bot's brain's choice
public task_slot_back(const data[], tid)
{
	new id = tid - TASK_SLOTS
	if (!is_user_alive(id) || !is_user_bot(id) || !user_has_weapon(id, get_weaponid(data)))
		return
	new w = held_weapon(id)
	if (w == CSW_KNIFE || (g_SlotPistol[id] && w == g_SlotPistol[id]))
		engclient_cmd(id, data)
}

knife_flick(id)
{
	if (!free_bot(id) || !holding_gun(id))
		return
	knife_out(id, random_float(0.8, 2.5))
}

// The knife out for a while, then the gun back (sooner if something comes up)
knife_out(id, Float:secs)
{
	engclient_cmd(id, "weapon_knife")
	g_Out[id] = CSW_KNIFE
	set_task(secs, "task_lastinv", id + TASK_KNIFE)
}

public task_lastinv(tid)
{
	new id = tid - TASK_KNIFE
	// even if something came up meanwhile: it shouldn't be left with the knife out
	if (is_user_alive(id) && is_user_bot(id) && held_weapon(id) == CSW_KNIFE)
		engclient_cmd(id, "lastinv")
	g_Out[id] = 0
}

// A newer player out of spawn: the grenade out, and away again a moment later. Only an HE
// grenade, and early (the bot's brain only throws one in hand well into the round)
public task_bad_nade(tid)
{
	new id = tid - TASK_BAD
	if (!free_bot(id, false) || !holding_gun(id) || !user_has_weapon(id, CSW_HEGRENADE) || get_gametime() - g_RoundStartAt > 9.0)
		return
	engclient_cmd(id, "weapon_hegrenade")
	g_Out[id] = CSW_HEGRENADE
	set_task(random_float(0.6, 1.0), "task_nade_back", tid)
}

public task_nade_back(tid)
{
	new id = tid - TASK_BAD
	// ours to put away only while it's still ours (g_Out) and still the grenade in hand: if
	// the bot brain took it over (switched, threw it), it's left alone. (Not the attack
	// button: a bot holds attack whenever it has a grenade out, ours too)
	if (g_Out[id] == CSW_HEGRENADE && held_weapon(id) != CSW_HEGRENADE)
	{
		g_Out[id] = 0
		return
	}
	if (g_Out[id] == CSW_HEGRENADE)
		out_back(id)
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

// ---- looking (ls_bot) ---------------------------------------------------------------------

// Looks at a point for `secs` (and stands still for `hold`, if any); remembered, so danger
// lets it go at once
ls_look(id, const Float:at[3], Float:secs, Float:hold, bool:fast = false)
{
	// (a calm look doesn't take the view from gm_humanbots' urgent turn to someone behind it)
	if (!fast && !g_NoUrgent && LibraryExists("gm_humanbots", LibType_Library) && hb_urgent_until(id) > get_gametime())
		return
	// (fast: a flick of the mouse, not the brain's calm turn; holds are tagged "bf" so
	// another plugin's release doesn't end them)
	server_cmd("ls_bot %d look %.0f %.0f %.0f %.2f %d", id, at[0], at[1], at[2], secs, fast ? 1 : 0)
	if (hold > 0.0)
		server_cmd("ls_bot %d hold %.2f bf", id, hold)
	g_PuppetUntil[id] = floatmax(g_PuppetUntil[id], get_gametime() + floatmax(secs, hold))
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
	new Float:secs = random_float(1.2, 2.4)
	server_cmd("ls_bot %d nod %.0f %.0f %.0f %.1f %.2f", id, other[0], other[1], other[2], secs, random_float(0.22, 0.38))
	server_cmd("ls_bot %d hold %.1f bf", id, random_float(1.0, 2.0))
	g_PuppetUntil[id] = get_gametime() + secs
}

// A teammate it can see within `range`, picked at random (not `skip`), or 0
visible_mate(id, Float:range, skip)
{
	new Float:me[3], Float:other[3], list[32], n
	eye_pos(id, me)
	new team = get_user_team(id)
	new tr = create_tr2()
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == id || i == skip || !is_user_alive(i) || get_user_team(i) != team)
			continue
		eye_pos(i, other)
		if (get_distance_f(me, other) > range)
			continue
		engfunc(EngFunc_TraceLine, me, other, DONT_IGNORE_MONSTERS, id, tr)
		new Float:frac
		get_tr2(tr, TR_flFraction, frac)
		if (frac < 1.0 && get_tr2(tr, TR_pHit) != i)
			continue
		list[n++] = i
	}
	free_tr2(tr)
	return n ? list[random(n)] : 0
}

// Freeze time: the view snaps from one teammate's head to the next, a few times, a short
// stop on each (what good aimers do while they wait)
start_flicks(id)
{
	if (!g_Fork || !free_bot(id))
		return
	g_FlickLeft[id] = random_num(3, 5)
	g_FlickLast[id] = 0
	flick_step(id)
}

public task_flick(tid)
{
	flick_step(tid - TASK_FLICK)
}

flick_step(id)
{
	if (g_FlickLeft[id] <= 0 || !free_bot(id))
	{
		g_FlickLeft[id] = 0
		return
	}
	new mate = visible_mate(id, 1500.0, g_FlickLast[id])
	if (!mate)
	{
		g_FlickLeft[id] = 0
		return
	}
	g_FlickLast[id] = mate
	new Float:at[3]
	eye_pos(mate, at)
	new Float:secs = random_float(0.3, 0.6)
	ls_look(id, at, secs + 0.15, secs, true)
	if (--g_FlickLeft[id] > 0)
		set_task(secs + random_float(0.0, 0.15), "task_flick", id + TASK_FLICK)
}

// Freeze time: a few looks about, each its own: at a teammate, up at the sky or a roof, down
// at the gun (the buy menu), off to the side; how people pass the wait
start_glances(id)
{
	if (!g_Fork || !free_bot(id))
		return
	g_GlanceLeft[id] = random_num(2, 4)
	glance_step(id)
}

public task_glance(tid)
{
	glance_step(tid - TASK_GLANCE)
}

glance_step(id)
{
	// (freeze time, or just after a deathmatch spawn)
	if (g_GlanceLeft[id] <= 0 || (!g_Freeze && get_gametime() >= g_Grace[id]) || !free_bot(id))
	{
		g_GlanceLeft[id] = 0
		return
	}
	g_GlanceLeft[id]--
	new Float:me[3], Float:at[3], Float:view[3]
	eye_pos(id, me)
	pev(id, pev_v_angle, view)
	new r = random_num(0, 99), mate
	if (r < 35 && (mate = visible_mate(id, 1500.0, 0)))
	{
		pev(mate, pev_origin, at)
		at[2] += random_float(0.0, 20.0)
	}
	else
	{
		new Float:yaw = view[1] + random_float(-150.0, 150.0)
		new Float:dist, Float:dz
		if (r < 60)
		{
			dist = random_float(300.0, 700.0)
			dz = random_float(200.0, 500.0)
		}
		else if (r < 80)
		{
			yaw = view[1] + random_float(-25.0, 25.0)
			dist = random_float(60.0, 120.0)
			dz = -random_float(50.0, 64.0)
		}
		else
		{
			dist = random_float(300.0, 800.0)
			dz = random_float(-20.0, 30.0)
		}
		at[0] = me[0] + floatcos(yaw, degrees) * dist
		at[1] = me[1] + floatsin(yaw, degrees) * dist
		at[2] = me[2] + dz
	}
	new Float:secs = random_float(0.5, 1.3)
	ls_look(id, at, secs, 0.0)
	if (g_GlanceLeft[id] > 0)
		set_task(secs + random_float(0.0, 0.3), "task_glance", id + TASK_GLANCE)
}

// Fast nodding: the view bobbing up and down at nothing in particular, quickly (ls_bot nod)
nod_fast(id)
{
	if (!g_Fork || !free_bot(id))
		return
	new Float:me[3], Float:view[3], Float:at[3]
	eye_pos(id, me)
	pev(id, pev_v_angle, view)
	at[0] = me[0] + floatcos(view[1], degrees) * 300.0
	at[1] = me[1] + floatsin(view[1], degrees) * 300.0
	at[2] = me[2]
	new Float:secs = random_float(0.8, 1.6)
	server_cmd("ls_bot %d nod %.0f %.0f %.0f %.1f %.2f 1", id, at[0], at[1], at[2], secs, random_float(0.09, 0.15))
	g_PuppetUntil[id] = floatmax(g_PuppetUntil[id], get_gametime() + secs)
}

// Mouse sweeps, a step at a time: a spin round (a quarter turn each step, once or twice
// round), a quick wiggle left and right (a dozen degrees or so each way), or a slow pan
// across the map (a few long looks well apart)

start_sweep(id, kind)
{
	if (!g_Fork || !free_bot(id))
		return
	new Float:view[3]
	pev(id, pev_v_angle, view)
	g_SweepKind[id] = kind
	g_SweepYaw[id] = view[1]
	g_SweepStep[id] = 0
	g_SweepLeft[id] = kind == SWEEP_SPIN ? 7 * random_num(1, 2) : kind == SWEEP_WIGGLE ? random_num(4, 7) : random_num(2, 3)
	sweep_step(id)
}

public task_sweep(tid)
{
	sweep_step(tid - TASK_SWEEP)
}

sweep_step(id)
{
	if (g_SweepLeft[id] <= 0 || !g_Freeze || !free_bot(id))
	{
		g_SweepLeft[id] = 0
		return
	}
	g_SweepLeft[id]--
	g_SweepStep[id]++
	new Float:me[3], Float:at[3], Float:yaw, Float:secs, Float:dz = 0.0
	eye_pos(id, me)
	switch (g_SweepKind[id])
	{
		case SWEEP_SPIN:
		{
			// (45-60 degrees a step: a quarter turn at a time would flip back the short way)
			g_SweepYaw[id] += random_float(45.0, 60.0)
			yaw = g_SweepYaw[id]
			secs = random_float(0.1, 0.16)
		}
		case SWEEP_WIGGLE:
		{
			yaw = g_SweepYaw[id] + ((g_SweepStep[id] % 2) ? 1.0 : -1.0) * random_float(8.0, 18.0)
			dz = random_float(-8.0, 8.0)
			secs = random_float(0.08, 0.15)
		}
		default:
		{
			yaw = g_SweepYaw[id] + float(g_SweepStep[id]) * random_float(45.0, 80.0) * ((g_SweepYaw[id] > 0.0) ? 1.0 : -1.0)
			dz = random_float(-15.0, 40.0)
			secs = random_float(1.2, 2.2)
		}
	}
	at[0] = me[0] + floatcos(yaw, degrees) * 500.0
	at[1] = me[1] + floatsin(yaw, degrees) * 500.0
	at[2] = me[2] + dz
	ls_look(id, at, secs + 0.1, 0.0, g_SweepKind[id] != SWEEP_PAN)
	if (g_SweepLeft[id] > 0)
		set_task(secs, "task_sweep", id + TASK_SWEEP)
}

// A newer player staring at one teammate for a while (freeze time)
stare_mate(id)
{
	if (!g_Fork || !free_bot(id))
		return
	new mate = visible_mate(id, 1200.0, 0)
	if (!mate)
		return
	new Float:at[3]
	eye_pos(mate, at)
	ls_look(id, at, random_float(1.5, 3.0), 0.0)
}

eye_pos(id, Float:out[3])
{
	new Float:ofs[3]
	pev(id, pev_origin, out)
	pev(id, pev_view_ofs, ofs)
	out[2] += ofs[2]
}

// Stops for a moment and looks at something: up at the sky or a roof, or off to the side.
// Now and then (four in ten) only a glance down at the ground ahead, still walking
look_around(id)
{
	new Float:me[3], Float:at[3], Float:view[3]
	pev(id, pev_origin, me)
	pev(id, pev_v_angle, view)
	if (random_num(0, 9) < 4)
	{
		new Float:yaw = view[1] + random_float(-30.0, 30.0)
		new Float:dist = random_float(90.0, 160.0)
		at[0] = me[0] + floatcos(yaw, degrees) * dist
		at[1] = me[1] + floatsin(yaw, degrees) * dist
		at[2] = me[2] - 36.0
		ls_look(id, at, random_float(0.6, 1.2), 0.0)
		return
	}
	new Float:yaw = view[1] + random_float(-120.0, 120.0)
	new Float:dist = random_float(200.0, 600.0)
	at[0] = me[0] + floatcos(yaw, degrees) * dist
	at[1] = me[1] + floatsin(yaw, degrees) * dist
	at[2] = me[2] + (random_num(0, 2) ? random_float(-20.0, 80.0) : random_float(200.0, 500.0))
	new Float:secs = random_float(1.0, 2.5)
	ls_look(id, at, secs, secs * random_float(0.6, 1.0))
}

public task_celebrate(tid)
{
	new id = tid - TASK_CELEB
	if (!g_Fork || !free_bot(id, false))
		return
	// the knife can't shoot: the gun out first
	if (held_weapon(id) == CSW_KNIFE)
		engclient_cmd(id, "lastinv")
	new Float:me[3]
	pev(id, pev_origin, me)
	me[2] += 600.0
	me[0] += random_float(-120.0, 120.0)
	me[1] += random_float(-120.0, 120.0)
	ls_look(id, me, 2.5, 0.0)
	g_FireKind[id] = FIRE_SKY
	set_task(random_float(0.4, 0.8), "task_fire", id + TASK_CELEB)
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
	switch (held_weapon(id))
	{
		case CSW_KNIFE, CSW_HEGRENADE, CSW_FLASHBANG, CSW_SMOKEGRENADE, CSW_C4, 0: return false
	}
	return true
}

// ---- breakables ---------------------------------------------------------------------------

// A func_breakable that's still there and can be broken by hitting it (not one only a
// trigger breaks, nor one that explodes); its middle in `mid`
bool:breakable_ok(ent, Float:mid[3])
{
	if (!pev_valid(ent) || pev(ent, pev_solid) == SOLID_NOT || pev(ent, pev_effects) & EF_NODRAW)
		return false
	new Float:takedamage, Float:health
	pev(ent, pev_takedamage, takedamage)
	pev(ent, pev_health, health)
	// (spawnflag 1: only a trigger breaks it; impulse: how big its explosion is)
	if (takedamage == DAMAGE_NO || health <= 0.0 || pev(ent, pev_spawnflags) & 1 || pev(ent, pev_impulse) > 0)
		return false
	new Float:mins[3], Float:maxs[3]
	pev(ent, pev_absmin, mins)
	pev(ent, pev_absmax, maxs)
	for (new i = 0; i < 3; i++)
		mid[i] = (mins[i] + maxs[i]) * 0.5
	return true
}

// The breakable it'd go for: in front of it (within `cone` degrees), between `near` and
// `far` away, nothing in the way. glass: only glass, screens and small things (bottles)
find_breakable(id, Float:near, Float:far, Float:cone, bool:glass, Float:out[3])
{
	new Float:eye[3], Float:view[3], Float:fwd[3], Float:mid[3], Float:dir[3]
	eye_pos(id, eye)
	pev(id, pev_v_angle, view)
	angle_vector(view, ANGLEVECTOR_FORWARD, fwd)
	new Float:mindot = floatcos(cone, degrees)
	new tr = create_tr2(), best, Float:bestDist = far
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "func_breakable")) > 0)
	{
		if (!breakable_ok(ent, mid))
			continue
		if (glass)
		{
			// glass 0, computer screens 6, unbreakable glass 7; anything small (bottles) too
			new Float:health
			pev(ent, pev_health, health)
			new mat = get_ent_data(ent, "CBreakable", "m_Material")
			if (mat == 7 || (mat != 0 && mat != 6 && health > 30.0))
				continue
		}
		for (new i = 0; i < 3; i++)
			dir[i] = mid[i] - eye[i]
		new Float:d = vector_length(dir)
		if (d < near || d >= bestDist)
			continue
		if ((dir[0] * fwd[0] + dir[1] * fwd[1] + dir[2] * fwd[2]) / d < mindot)
			continue
		engfunc(EngFunc_TraceLine, eye, mid, DONT_IGNORE_MONSTERS, id, tr)
		new Float:frac
		get_tr2(tr, TR_flFraction, frac)
		if (frac < 1.0 && get_tr2(tr, TR_pHit) != ent)
			continue
		best = ent
		bestDist = d
		out = mid
	}
	free_tr2(tr)
	return best
}

// After the round: the glass in view, a few shots at it
public task_break_shoot(tid)
{
	new id = tid - TASK_BREAK
	if (!g_Fork || !free_bot(id, false) || !holding_gun(id))
		return
	new Float:at[3]
	if (!find_breakable(id, 80.0, 800.0, 70.0, true, at))
		return
	ls_look(id, at, 2.0, 1.5)
	g_FireKind[id] = FIRE_AT
	g_FireAt[id] = at
	set_task(random_float(0.4, 0.8), "task_fire", id + TASK_CELEB)
}

// The ones with the habit slash a crate with the knife when they walk past it: the first
// they come across, and after that the same one every time
slash_check(Float:now)
{
	for (new id = 1; id <= MaxClients; id++)
	{
		if (now < g_NextSlash[id] || !is_user_alive(id) || !is_user_bot(id))
			continue
		g_NextSlash[id] = now + 1.0
		if (!has_quirk(id, Q_SLASH) || !holding_gun(id) || !free_bot(id))
			continue
		new Float:at[3]
		new ent = find_breakable(id, 0.0, 90.0, g_Fork ? 80.0 : 40.0, false, at)
		if (!ent || (g_FavBreak[id] && ent != g_FavBreak[id] && breakable_ok(g_FavBreak[id], at)))
			continue
		if (random_float(0.0, 1.0) >= (g_FavBreak[id] ? 0.8 : 0.4))
		{
			g_NextSlash[id] = now + 20.0
			continue
		}
		g_FavBreak[id] = ent
		g_NextSlash[id] = now + random_float(15.0, 25.0)
		// find_breakable left its middle in `at` (the fav check above may have changed it)
		new Float:mid[3]
		breakable_ok(ent, mid)
		if (g_Fork)
			ls_look(id, mid, 1.4, 0.0)
		knife_out(id, random_float(1.3, 1.8))
		set_task(0.45, "task_slash", id + TASK_BREAK)
		if (random_num(0, 1))
			set_task(0.95, "task_slash", id + TASK_BREAK)
	}
}

public task_slash(tid)
{
	new id = tid - TASK_BREAK
	if (g_Out[id] == CSW_KNIFE && held_weapon(id) == CSW_KNIFE && free_bot(id, false))
		g_Press[id] |= IN_ATTACK
}

// ---- dropping a gun for a teammate ----------------------------------------------------------

#define PRIMARY_BITS ((1<<CSW_SCOUT)|(1<<CSW_XM1014)|(1<<CSW_MAC10)|(1<<CSW_AUG)|(1<<CSW_UMP45)|(1<<CSW_SG550)|(1<<CSW_GALIL)|(1<<CSW_FAMAS)|(1<<CSW_AWP)|(1<<CSW_MP5NAVY)|(1<<CSW_M249)|(1<<CSW_M3)|(1<<CSW_M4A1)|(1<<CSW_TMP)|(1<<CSW_G3SG1)|(1<<CSW_SG552)|(1<<CSW_AK47)|(1<<CSW_P90))

// Its primary weapon, or 0
primary_of(id)
{
	new weapons[32], n
	get_user_weapons(id, weapons, n)
	for (new i = 0; i < n; i++)
		if ((1 << weapons[i]) & PRIMARY_BITS)
			return weapons[i]
	return 0
}

gun_price(w)
{
	switch (w)
	{
		case CSW_AWP: return 4750
		case CSW_AK47: return 2500
		case CSW_M4A1: return 3100
		case CSW_SG552, CSW_AUG: return 3500
		case CSW_FAMAS: return 2250
		case CSW_GALIL: return 2000
		case CSW_SCOUT: return 2750
		case CSW_M249: return 5750
		case CSW_G3SG1: return 5000
		case CSW_SG550: return 4200
		case CSW_XM1014: return 3000
		case CSW_M3: return 1700
		case CSW_P90: return 2350
		case CSW_MP5NAVY: return 1500
		case CSW_UMP45: return 1700
		case CSW_MAC10: return 1400
		case CSW_TMP: return 1250
	}
	return 0
}

// Spare ammo a bought gun comes with here (what the bot would buy with it)
gun_ammo(w)
{
	switch (w)
	{
		case CSW_AWP: return 30
		case CSW_M3, CSW_XM1014: return 32
		case CSW_M249: return 200
		case CSW_P90, CSW_UMP45, CSW_MAC10: return 100
		case CSW_MP5NAVY, CSW_TMP: return 120
	}
	return 90
}

// Still time to buy: freeze time, or within mp_buytime of the round starting
bool:buy_time()
{
	if (g_Freeze)
		return true
	new Float:secs = buy_secs()
	return secs < 0.0 || get_gametime() - g_RoundStartAt < secs
}

// The buy time in seconds the way the game counts it: never under 15 (it puts a lower
// mp_buytime, 0 too, up to that), and -1 for no limit
Float:buy_secs()
{
	new Float:v = get_cvar_float("mp_buytime")
	return v == -1.0 ? -1.0 : floatmax(v * 60.0, 15.0)
}

// Drops happen where buying is normal (Classic, Warcraft 3, SuperHero), in a pub round
bool:drop_mode()
{
	return get_pcvar_num(g_pDrop) && g_FunOn && g_BuyMode && mode_ctx() == CTX_PUB
		&& !get_cvar_num("gm_weapons")
}

// `word` in msg on its own ("drop" in "drop pls", not in "dropped")
bool:word_in(const msg[], const word[])
{
	new len = strlen(word), from, at
	while ((at = containi(msg[from], word)) != -1)
	{
		new s = from + at
		if ((s == 0 || !isalnum(msg[s - 1])) && !isalnum(msg[s + len]))
			return true
		from = s + 1
	}
	return false
}

// Someone (a person) asking for a gun: "drop pls", "drop me awp", "can someone drop"
public cmd_say(id)
{
	if (!is_user_alive(id) || is_user_bot(id) || !drop_mode() || !buy_time())
		return PLUGIN_CONTINUE
	new msg[96]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	if (strlen(msg) > 40 || !word_in(msg, "drop") || word_in(msg, "bomb") || word_in(msg, "c4") || word_in(msg, "dont"))
		return PLUGIN_CONTINUE
	new want = word_in(msg, "awp") ? CSW_AWP : (word_in(msg, "ak") || word_in(msg, "ak47")) ? CSW_AK47
		: (word_in(msg, "m4") || word_in(msg, "m4a1")) ? CSW_M4A1 : 0
	ask_drop(id, want, 0.6)
	return PLUGIN_CONTINUE
}

// ---- gifts: a person drops a gun where a teammate bot with a worse one (or none) is ----
// How much a bot would rather have it (the price, mostly; the machine gun and the
// auto-snipers aren't what anyone wants instead of a rifle)
gift_rank(w)
{
	switch (w)
	{
		case 0: return 0
		case CSW_M249, CSW_G3SG1, CSW_SG550: return 2000
	}
	return gun_price(w)
}

public cmd_drop(id)
{
	if (!is_user_alive(id) || is_user_bot(id) || !drop_mode())
		return PLUGIN_CONTINUE
	new gun = held_weapon(id)
	if (!((1 << gun) & PRIMARY_BITS))
		return PLUGIN_CONTINUE
	// the weapon box only exists once the drop is done: the gun's entity, looked at after
	g_GiftBox[id] = active_item(id)
	g_GiftGun[id] = gun
	remove_task(id + TASK_GIFT)
	set_task(0.1, "task_gift_box", id + TASK_GIFT)
	return PLUGIN_CONTINUE
}

public task_gift_box(tid)
{
	new id = tid - TASK_GIFT
	new ent = g_GiftBox[id], gun = g_GiftGun[id], team = get_user_team(id)
	g_GiftBox[id] = 0
	if (!ent || !pev_valid(ent) || !is_user_connected(id) || (team != 1 && team != 2))
		return
	new box = pev(ent, pev_owner), cls[16]
	if (box <= MaxClients || !pev_valid(box))
		return
	pev(box, pev_classname, cls, charsmax(cls))
	if (!equal(cls, "weaponbox"))
		return
	// the nearest teammate bot close by, quiet, with something worse
	new Float:at[3], Float:o[3], best, Float:bestd = 600.0
	pev(box, pev_origin, at)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_alive(i) || !is_user_bot(i) || get_user_team(i) != team || g_GiftBot[i] || !free_bot(i))
			continue
		if (gift_rank(primary_of(i)) >= gift_rank(gun))
			continue
		pev(i, pev_origin, o)
		new Float:d = get_distance_f(at, o)
		if (d < bestd)
		{
			bestd = d
			best = i
		}
	}
	if (!best)
		return
	g_GiftBot[best] = id
	g_GiftBox[best] = box
	g_GiftGun[best] = gun
	g_GiftTries[best] = 0
	set_task(random_float(0.5, 1.0), "task_gift_take", best + TASK_GIFT)
}

// Over to the gift, its own gun dropped once there, and the gift picked up. In freeze time it
// can't walk: it looks, and goes once the round starts (task_gift_check sends it then)
public task_gift_take(tid)
{
	new id = tid - TASK_GIFT
	new box = g_GiftBox[id], giver = g_GiftBot[id]
	if (!gift_still(id))
		return gift_done(id)
	new Float:at[3]
	pev(box, pev_origin, at)
	if (g_Fork)
	{
		new Float:eye[3]
		eye_pos(giver, eye)
		ls_look(id, eye, 0.8, 0.6)
		if (!g_Freeze)
		{
			server_cmd("ls_bot %d goto %.0f %.0f %.0f", id, at[0], at[1], at[2])
			g_GiftSent[id] = true
		}
	}
	set_task(0.5, "task_gift_check", tid, _, _, "b")
	return 0
}

public task_gift_check(tid)
{
	new id = tid - TASK_GIFT
	if (is_user_alive(id) && primary_of(id) == g_GiftGun[id])
	{
		if (g_Chat && is_user_connected(g_GiftBot[id]))
			botchat_say_now(id, "gift_ty", g_GiftBot[id], true)
		return gift_done(id)
	}
	// (the wait for freeze time to end isn't counted: 8 seconds to get there once it can walk)
	if (!gift_still(id) || (!g_Freeze && ++g_GiftTries[id] > 16))
		return gift_done(id)
	new box = g_GiftBox[id], Float:at[3], Float:me[3]
	pev(box, pev_origin, at)
	pev(id, pev_origin, me)
	// freeze time is over: now it walks over (once)
	if (g_Fork && !g_Freeze && !g_GiftSent[id])
	{
		server_cmd("ls_bot %d goto %.0f %.0f %.0f", id, at[0], at[1], at[2])
		g_GiftSent[id] = true
	}
	// only once it's there, at arm's length (never pulled in from across the room)
	new Float:d = get_distance_f(at, me)
	if (d < 90.0 && (pev(box, pev_flags) & FL_ONGROUND))
	{
		new own = primary_of(id)
		if (own)
		{
			new name[24]
			get_weaponname(own, name, charsmax(name))
			engclient_cmd(id, "drop", name)
		}
		ExecuteHamB(Ham_Touch, box, id)
	}
	return 0
}

bool:gift_still(id)
{
	new box = g_GiftBox[id]
	if (!is_user_alive(id) || !box || !pev_valid(box) || g_BombPlanted || sees_enemy(id))
		return false
	new cls[16]
	pev(box, pev_classname, cls, charsmax(cls))
	return bool:equal(cls, "weaponbox")
}

gift_done(id)
{
	remove_task(id + TASK_GIFT)
	g_GiftBot[id] = g_GiftBox[id] = g_GiftGun[id] = 0
	g_GiftSent[id] = false
	return 0
}

// A broke bot asks its team (in freeze time, once the others have bought)
public task_ask_drop(tid)
{
	new id = tid - TASK_DROP
	new team = get_user_team(id)
	if (!is_user_alive(id) || (team != 1 && team != 2) || g_Asked[team] || !drop_mode() || primary_of(id) || cs_get_user_money(id) >= 2000)
		return
	g_Asked[team] = true
	if (g_Chat && botchat_say_now(id, "drop_ask", 0, true))
		ask_drop(id, 0, 0.5)
}

// Maybe a well-off teammate with a generous streak drops a gun for `asker` (rare: most of
// the time nobody does). want: a gun they asked for by name; prefer: a bot asked by name,
// which gives if it can and isn't cold. True when a drop is on its way
bool:ask_drop(asker, want, Float:odds, prefer = 0)
{
	new team = get_user_team(asker)
	if (g_DropDone || (team != 1 && team != 2))
		return false
	// someone who has a gun only asks for something in particular
	if ((want && user_has_weapon(asker, want)) || (!want && primary_of(asker)))
		return false
	new Float:me[3], Float:o[3], list[32], guns[32], n, cold, picked, pickedGun
	pev(asker, pev_origin, me)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == asker || !is_user_alive(i) || !is_user_bot(i) || get_user_team(i) != team || !cs_get_user_buyzone(i))
			continue
		new gun = want ? (user_has_weapon(i, want) ? want : 0) : primary_of(i)
		// rich: it can buy the same again and still have some left
		if (!gun || cs_get_user_money(i) < gun_price(gun) + 1500)
			continue
		pev(i, pev_origin, o)
		if (get_distance_f(me, o) > 1200.0 || !free_bot(i, false))
			continue
		// asked by name: it gives (a cold one still says no)
		if (i == prefer && warmth(i) != 0)
		{
			picked = i
			pickedGun = gun
			break
		}
		if (!has_quirk(i, Q_GENEROUS))
		{
			if (warmth(i) == 0)
				cold = i
			continue
		}
		guns[n] = gun
		list[n++] = i
	}
	if (!picked && (!n || random_float(0.0, 1.0) >= odds * floatmin(pers(), 1.5)))
	{
		// now and then someone says no
		if (g_Chat && cold && random_num(1, 100) <= 12)
			botchat_say(cold, "drop_no", asker, 1.0)
		return false
	}
	new giver, gun
	if (picked)
	{
		giver = picked
		gun = pickedGun
	}
	else
	{
		new k = random(n)
		giver = list[k]
		gun = guns[k]
	}
	g_DropDone = true
	g_DropFor[giver] = asker
	g_DropUser[giver] = get_user_userid(asker)
	g_DropGun[giver] = gun
	set_task(random_float(1.5, 4.0), "task_drop", giver + TASK_DROP)
	return true
}

// Still worth doing: both there, the asker still without it, nothing going on
bool:drop_still(id)
{
	new asker = g_DropFor[id], gun = g_DropGun[id]
	if (!asker || !is_user_alive(asker) || get_user_userid(asker) != g_DropUser[id] || !buy_time())
		return false
	if (!free_bot(id, false) || !user_has_weapon(id, gun) || user_has_weapon(asker, gun))
		return false
	return gun == CSW_AWP || !primary_of(asker)
}

// Says so, looks at them, gets the gun out
public task_drop(tid)
{
	new id = tid - TASK_DROP
	if (!drop_still(id))
	{
		g_DropFor[id] = 0
		return
	}
	new asker = g_DropFor[id], gun = g_DropGun[id]
	if (g_Chat)
		botchat_say_now(id, "drop_give", asker, true)
	if (g_Fork)
	{
		new Float:at[3]
		eye_pos(asker, at)
		ls_look(id, at, 1.4, 1.0)
	}
	if (held_weapon(id) != gun)
	{
		new name[24]
		get_weaponname(gun, name, charsmax(name))
		engclient_cmd(id, name)
	}
	set_task(random_float(0.6, 0.9), "task_drop_throw", tid)
}

// Drops it their way (the weapon box is sent toward them), and buys another
public task_drop_throw(tid)
{
	new id = tid - TASK_DROP
	new asker = g_DropFor[id], gun = g_DropGun[id]
	if (!drop_still(id) || held_weapon(id) != gun)
	{
		g_DropFor[id] = 0
		return
	}
	new ent = active_item(id)
	engclient_cmd(id, "drop")
	// the dropped gun now sits in a weaponbox (its owner)
	new box = ent ? pev(ent, pev_owner) : 0
	g_DropBox[id] = 0
	if (box > MaxClients && pev_valid(box))
	{
		new cls[16]
		pev(box, pev_classname, cls, charsmax(cls))
		if (equal(cls, "weaponbox"))
		{
			new Float:from[3], Float:to[3], Float:vel[3]
			pev(box, pev_origin, from)
			pev(asker, pev_origin, to)
			new Float:dx = to[0] - from[0], Float:dy = to[1] - from[1]
			new Float:d = floatsqroot(dx * dx + dy * dy)
			if (d > 1.0)
			{
				new Float:speed = floatclamp(d * 1.1, 120.0, 380.0)
				vel[0] = dx / d * speed
				vel[1] = dy / d * speed
				vel[2] = 160.0
				set_pev(box, pev_velocity, vel)
			}
			g_DropBox[id] = box
			// a bot doesn't go looking for it: it gets it once it lands
			if (is_user_bot(asker))
				set_task(1.4, "task_drop_pickup", tid)
		}
	}
	set_task(random_float(0.8, 2.0), "task_rebuy", id + TASK_REBUY)
}

public task_drop_pickup(tid)
{
	new id = tid - TASK_DROP
	new box = g_DropBox[id], asker = g_DropFor[id]
	g_DropBox[id] = 0
	g_DropFor[id] = 0
	if (!box || !pev_valid(box) || !asker || !is_user_alive(asker) || get_user_userid(asker) != g_DropUser[id] || primary_of(asker))
		return
	new cls[16]
	pev(box, pev_classname, cls, charsmax(cls))
	if (!equal(cls, "weaponbox") || !(pev(box, pev_flags) & FL_ONGROUND))
		return
	new Float:a[3], Float:b[3]
	pev(box, pev_origin, a)
	pev(asker, pev_origin, b)
	if (get_distance_f(a, b) > 400.0)
		return
	ExecuteHamB(Ham_Touch, box, asker)
}

// The same gun again, if it can still buy
public task_rebuy(tid)
{
	new id = tid - TASK_REBUY
	new gun = g_DropGun[id]
	if (!is_user_alive(id) || !gun || primary_of(id) || !cs_get_user_buyzone(id) || !buy_time())
		return
	new price = gun_price(gun), money = cs_get_user_money(id)
	if (money < price)
		return
	new name[24]
	get_weaponname(gun, name, charsmax(name))
	give_item(id, name)
	cs_set_user_bpammo(id, gun, gun_ammo(gun))
	cs_set_user_money(id, money - price)
}
