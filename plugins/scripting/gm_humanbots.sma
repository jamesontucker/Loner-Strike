/*
	gm_humanbots - Bot styles: makes CZ bots aim and react more like people.

	CZ decides where a bot looks and when it pulls the trigger. This sits on top:
	  - reaction time: a new enemy can't be shot at until a human-like reaction
	    time has passed (ex-Gaussian: mostly quick, now and then much slower),
	    slower for far-away targets and ones at the edge of view, faster for an
	    enemy seen a moment ago
	  - flicks: the first shots carry the leftover of a human flick, an overshoot
	    that settles along a minimum-jerk curve; bigger turns take longer (Fitts)
	  - tracking wobble: smooth, mean-reverting aim error (Ornstein-Uhlenbeck),
	    larger when the target strafes or the bot is moving
	  - bursts: sprays stop after a few shots and drift upward/sideways like a
	    player losing control of recoil
	  - mistakes: the odd whiffed first shot, late reaction or panicked spray

	Aim error is applied by bending each bullet as it is traced; the bot's view
	and AI are left alone (tested: v_angle offsets make CZ bots stop firing).

	gm_botstyle      0 Classic (off), 1 Pub server, 2 Matchmaking, 3 Veterans, 4 Newbie lobby,
	                 5 Locked In, 6 Hackers (most bullets snap to you, notice you behind them),
	                 7 Gods (everything maxed: every bullet near you is a headshot),
	                 8 Mixed lobby (each bot plays at its own level, newbie to Locked In,
	                   and keeps it while it's on the server)
	gm_hb_tilt       1 bots on a losing run get sloppier (most) or more careful (some);
	                   a few kills settle them again. Low health or several enemies at
	                   once can make one panic: spraying, jumping, missing
	gm_hb_habits     1 each bot plays like its chat personality (gm_botchat): newbies spray
	                   and react late, tryhards pre-aim, old-timers tap; favourite guns
	                   (a Deagle lover, an AWP guy), warming up after joining, getting
	                   tired late at night, over-swinging at someone behind them,
	                   nervous reloads, running with the knife out early in the round
	                   (not in a match), a rival (the player who keeps killing it)
	                   it reacts to faster, and the odd failed bunnyhop from a pub kid
	gm_hb_afk        1 now and then a bot stands still for a few seconds early in the round
	                   (not in modes a plugin steers bots through, deathmatch, or a match
	                   or Matchmaking on Classic), once it's standing somewhere solid;
	                   shots, an enemy, danger nearby (gm_botfun) or getting hurt bring
	                   it back at once
	gm_hb_sprays     1 bots spray a logo now and then: Loner-Strike's (sprites/lonerstrike/
	                   spray.spr, flat on the wall like a player's spray), else one of the
	                   game's own logos. Cleared each round. A few tag a wall after a kill
	gm_hb_disrespect 0 lets bots be disrespectful: crouch spam over someone they killed
	gm_hb_radio      1 the odd radio call that fits the moment and the bot (enemy spotted,
	                   enemy down; in team modes the team's caller says go). Each bot and
	                   each team waits a while between calls: CZ bots walk over to whoever
	                   calls, so a busy radio turns a team into one big group
	gm_hb_showoff    1 now and then after a kill, when no other enemy is in view and it
	                   hasn't been hurt lately: a jump, the knife out for a moment, or
	                   (with gm_hb_disrespect) crouch spam over the body. Any sign of a
	                   fight ends it and the gun comes back
	gm_hb_reaction   1 human reaction times
	gm_hb_aim        1 flicks, wobble and bursts
	gm_hb_mistakes   1 occasional mistakes
	gm_hb_bhop       1 Locked In and Hackers bunnyhop between fights
	gm_hb_hold       1 careful players and lurkers stop now and then and hold an angle for a
	                   few seconds (crouched sometimes); more on aim and duel maps. Not in
	                   a match or deathmatch, and only while nothing is going on around it
	gm_personality 100 how strongly all of the above shows, in percent (0-200): at 0 bots
	                   play their style plainly (no tilt, panic, habits, show-off, afk,
	                   sprays or radio, less spread in skill and mistakes); 200 exaggerates it.
	                   gm_botchat reads it too
	Following you: CZ bots with high teamwork tag along behind a human teammate. With
	a style on, that only happens in team modes (gm_botplay 0, from botplay.ini), and
	only now and then: the rest of the time the human isn't open to being followed.
	Each style also has bot personality files (czero/BotProfile_<Style>.db for team
	modes, _Solo and _Hold for the rest, made by the app's setup) that gamemodes.amxx
	switches to: every bot gets its own playstyle there (rusher, roamer, lurker, careful,
	the odd team player), which is what shapes how they move.
*/

#include <amxmodx>
#include <fakemeta>
#include <cstrike>
#include <hamsandwich>
#include <fun>
#include <amxmisc>

// gm_botchat (optional): the bot's chat personality and its lines
native botchat_persona(id)
native botchat_say(id, const section[], other, Float:chance)
// gm_botfun (optional): danger around the bot (shots, someone hurt, an enemy seen lately)
native bf_danger(id)

#define PLUGIN  "Human bots"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define SCAN_INTERVAL 0.1
#define STYLE_MIXED   8
#define OFFSET_ACTIVE_ITEM 373
#define TASK_KNIFE    1900000             // the show-off knife going back (+ id)
#define TASK_SPRAY    2100000             // a spray (+ userid)
#define TASK_RADIO    2200000             // the caller's call (+ userid)
#define KNIFE_WINDOW  14.0                // knife runs only this early in a round

enum { STYLE_CLASSIC, STYLE_PUB, STYLE_MM, STYLE_VET, STYLE_NEWBIE, STYLE_LOCKED, STYLE_HACKER, STYLE_GOD, STYLE_COUNT }

enum _:StyleData
{
	Float:ST_RtMu,        // reaction time, ex-Gaussian (seconds)
	Float:ST_RtSigma,
	Float:ST_RtTau,
	Float:ST_Wobble,      // tracking error, degrees (standard deviation)
	Float:ST_Overshoot,   // flick overshoot as a share of the turn
	Float:ST_FlickTime,   // seconds to settle a small flick
	ST_BurstMin,
	ST_BurstMax,
	Float:ST_Mistakes,    // chance per engagement
	Float:ST_SkillLo,     // per-bot skill multiplier range (higher = worse)
	Float:ST_SkillHi,
	Float:ST_ViewFov,     // half-angle in which a bot notices enemies
	ST_HeadPct,           // share of bullets that snap to the head (Hackers, Gods)
	ST_BodyPct            // ... and to the chest
}

new const STYLES[STYLE_COUNT][StyleData] =
{
	{ 0.0,  0.0,   0.0,  0.0,  0.0,  0.0,  0,   0,   0.0,  1.0,  1.0,  60.0,  0,   0  },
	{ 0.23, 0.04,  0.12, 1.6,  0.18, 0.22, 6,   14,  0.12, 0.55, 1.8,  60.0,  0,   0  },  // Pub server
	{ 0.20, 0.03,  0.07, 1.0,  0.12, 0.18, 4,   9,   0.06, 0.75, 1.3,  60.0,  0,   0  },  // Matchmaking
	{ 0.17, 0.025, 0.05, 0.65, 0.08, 0.14, 3,   7,   0.03, 0.85, 1.15, 60.0,  0,   0  },  // Veterans
	{ 0.32, 0.06,  0.20, 2.6,  0.28, 0.30, 8,   20,  0.20, 0.8,  1.4,  50.0,  0,   0  },  // Newbie lobby
	{ 0.15, 0.02,  0.03, 0.4,  0.05, 0.11, 4,   10,  0.01, 0.9,  1.1,  70.0,  0,   0  },  // Locked In
	{ 0.06, 0.01,  0.01, 0.15, 0.0,  0.05, 99,  99,  0.0,  0.2,  0.3,  180.0, 65,  27 },  // Hackers
	{ 0.03, 0.005, 0.0,  0.0,  0.0,  0.01, 999, 999, 0.0,  0.0,  0.0,  180.0, 100, 0  }   // Gods
}

new g_pStyle, g_pReaction, g_pAim, g_pMistakes, g_pBhop, g_pFreeForAll, g_pTilt, g_pShowoff
new g_Mix[33]                                   // Mixed lobby: this bot's own style
new Float:g_Tilt[33], Float:g_TiltWay[33]       // >0 sloppier; some get careful instead
new Float:g_ShowUntil[33], g_Show[33]           // a show-off moment: 1 crouch spam, 2 jump
new g_ShowHp[33], Float:g_ShowNext[33], bool:g_ShowDuck[33]   // health then; the next crouch toggle
new bool:g_KnifeOut[33]                         // the show-off knife is out (the gun comes back)
new Float:g_HurtAt[33]                          // when it last took damage
new Float:g_AfkAt[33], g_AfkHp[33]              // when this round's afk starts; health then
new g_pHabits, g_pAfk, g_pSprays, g_pRadio, g_pDisrespect
new const SPRAY_SPRITE[] = "sprites/lonerstrike/spray.spr"
#define MAX_SPRAYS 16
new bool:g_LogoSpray, g_Sprays[MAX_SPRAYS], g_SprayNext   // the logo sprays on walls this round
new bool:g_Chat                                 // gm_botchat is running
new bool:g_BotFun                               // gm_botfun is running (bf_danger)
new bool:g_AfkOn[33]                            // the afk has started (it waits for solid ground)
new g_RoundKills[33]                            // kills this round (a better kill, a bigger show-off)
new bool:g_FailHops[33]                         // these hops are a pub player's failed bunnyhop
new Float:g_JoinedAt[33], Float:g_ReloadAt[33], Float:g_AfkUntil[33], Float:g_NoTargetSince[33], Float:g_NextKnifeTry[33]
new bool:g_KnifeRun[33], g_Fav[33], g_Rival[33], g_DeathsBy[33][33]
new g_SprayDecals[8], g_SprayCount, g_RadioCall[33], Float:g_Habit[33] = { 1.0, ... }
#define PRIMARY_BITS ((1<<CSW_SCOUT)|(1<<CSW_XM1014)|(1<<CSW_MAC10)|(1<<CSW_AUG)|(1<<CSW_UMP45)|(1<<CSW_SG550)|(1<<CSW_GALIL)|(1<<CSW_FAMAS)|(1<<CSW_AWP)|(1<<CSW_MP5NAVY)|(1<<CSW_M249)|(1<<CSW_M3)|(1<<CSW_M4A1)|(1<<CSW_TMP)|(1<<CSW_G3SG1)|(1<<CSW_SG552)|(1<<CSW_AK47)|(1<<CSW_P90))
#define PISTOL_BITS  ((1<<CSW_P228)|(1<<CSW_ELITE)|(1<<CSW_FIVESEVEN)|(1<<CSW_USP)|(1<<CSW_GLOCK18)|(1<<CSW_DEAGLE))

new g_HopsLeft[33], Float:g_NextHopTry[33]
new bool:g_Career

new g_pPersonality, g_pHold, g_pBotPlay
new Float:g_NextRadio[33], Float:g_TeamRadioAt[3]       // radio cooldowns: each bot, each team
new Float:g_HoldUntil[33], Float:g_NextHoldTry[33], bool:g_HoldDuck[33], g_HoldHp[33]
new Float:g_FollowOpenUntil[33], Float:g_NextFollowRoll[33]
new Float:g_RoundStartAt, bool:g_BombPlanted, bool:g_IdleMode
new bool:g_MatchMode, bool:g_ClassicMode, g_pBotDm      // match mode, Classic, bot_deathmatch
enum { PLAY_TEAM, PLAY_SOLO, PLAY_HOLD }                // gm_botplay (gamemodes, botplay.ini)
enum { KIND_RUSHER, KIND_ROAMER, KIND_LURKER, KIND_CAREFUL }

new Float:g_Skill[33], g_SkillStyle[33]
new g_Target[33], g_LastTarget[33]
new Float:g_LostAt[33], Float:g_TargetAngVel[33], Float:g_TargetDist[33]
new Float:g_Unlock[33]
new Float:g_FlickStart[33], Float:g_FlickDur[33], Float:g_OverYaw[33], Float:g_OverPitch[33]
new Float:g_WobbleYaw[33], Float:g_WobblePitch[33]
new g_Burst[33], g_BurstLen[33]
new Float:g_LastFire[33], Float:g_PauseUntil[33], Float:g_SprayDrift[33], Float:g_PanicUntil[33]
new bool:g_Fired[33]
new g_InThink
new bool:g_Bending
// forward ids while registered (0: not): TraceLine runs thousands of times a second for
// the bots' own AI, so none of these are hooked while there's nothing for them to do
new g_FwCmdStart, g_FwPreThink, g_FwPostThink, g_FwPostThinkPost, g_FwTraceLine

public plugin_precache()
{
	// Loner-Strike's logo, when setup put it there (one model slot, a 5 KB download)
	if (file_exists(SPRAY_SPRITE))
	{
		precache_model(SPRAY_SPRITE)
		g_LogoSpray = true
	}
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pStyle     = register_cvar("gm_botstyle", "0")
	g_pReaction  = register_cvar("gm_hb_reaction", "1")
	g_pAim       = register_cvar("gm_hb_aim", "1")
	g_pMistakes  = register_cvar("gm_hb_mistakes", "1")
	g_pBhop      = register_cvar("gm_hb_bhop", "1")
	g_pTilt      = register_cvar("gm_hb_tilt", "1")
	g_pShowoff   = register_cvar("gm_hb_showoff", "1")
	g_pHabits    = register_cvar("gm_hb_habits", "1")
	g_pAfk       = register_cvar("gm_hb_afk", "1")
	g_pSprays    = register_cvar("gm_hb_sprays", "1")
	g_pDisrespect = register_cvar("gm_hb_disrespect", "0")
	g_pRadio     = register_cvar("gm_hb_radio", "1")
	g_pHold      = register_cvar("gm_hb_hold", "1")
	g_pPersonality = register_cvar("gm_personality", "100")
	register_logevent("ev_round_start", 2, "1=Round_Start")
	register_event("HLTV", "ev_new_round_sprays", "a", "1=0", "2=0")
	g_Chat = bool:LibraryExists("gm_botchat", LibType_Library)
	// logos to spray: whichever of these the game has
	static const LOGOS[][] = { "{lambda01", "{lambda02", "{lambda03", "{lambda04", "{lambda05", "{lambda06", "{biohaz", "{smiley" }
	for (new i = 0; i < sizeof LOGOS && g_SprayCount < sizeof g_SprayDecals; i++)
	{
		new d = engfunc(EngFunc_DecalIndex, LOGOS[i])
		if (d > 0)
			g_SprayDecals[g_SprayCount++] = d
	}
	register_event("DeathMsg", "ev_death", "a")
	register_event("Damage", "ev_damage", "b", "2!0")
	// RegisterHamPlayer also covers Condition Zero bots (a different class)
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)
	register_logevent("ev_bomb_planted", 3, "2=Planted_The_Bomb")
	set_task(1.0, "task_follow", _, _, _, "b")

	// gamemodes switches styles by setting gm_botstyle; the forwards follow it
	hook_cvar_change(g_pStyle, "cvar_changed")
	hook_cvar_change(g_pAim, "cvar_changed")
	set_task(SCAN_INTERVAL, "task_scan", _, _, _, "b")
}

public plugin_natives()
{
	register_library("gm_humanbots")
	register_native("hb_bot_idle", "n_bot_idle")
	register_native("hb_bot_afk", "n_bot_afk")
	register_native("hb_bot_target", "n_bot_target")
	register_native("hb_bot_level", "n_bot_level")
	set_native_filter("native_filter")
}

// hb_bot_idle(id): true while the bot is afk or holding an angle (gm_botfun leaves it be)
public n_bot_idle(plugin, params)
{
	new id = get_param(1)
	if (id < 1 || id > MaxClients)
		return false
	new Float:now = get_gametime()
	return afk_now(id, now) || g_HoldUntil[id] > now
}

// hb_bot_afk(id): true while the bot is afk (gm_botchat doesn't type then)
public n_bot_afk(plugin, params)
{
	new id = get_param(1)
	return id >= 1 && id <= MaxClients && afk_now(id, get_gametime())
}

// hb_bot_target(id): the enemy the bot has in view (0: none, or the Classic style)
public n_bot_target(plugin, params)
{
	new id = get_param(1)
	return (id >= 1 && id <= MaxClients) ? g_Target[id] : 0
}

// hb_bot_level(id): how good a player the bot is, from its style: 0 new, 1 average,
// 2 good, 3 very good (-1: no style, or not a bot)
public n_bot_level(plugin, params)
{
	new id = get_param(1)
	if (id < 1 || id > MaxClients || !is_user_bot(id) || style() == STYLE_CLASSIC)
		return -1
	new s = bot_style(id)
	switch (s)
	{
		case STYLE_NEWBIE: return 0
		case STYLE_PUB:
		{
			// a pub's own mix: the sloppy ones, the crowd, a few sharp ones
			if (g_SkillStyle[id] != s)
				sample_skill(id, s)
			return g_Skill[id] >= 1.3 ? 0 : g_Skill[id] < 0.8 ? 2 : 1
		}
		case STYLE_MM: return 2
	}
	return 3
}

bool:afk_now(id, Float:now)
{
	return g_AfkOn[id] && g_AfkUntil[id] > now
}

// Danger around it that gm_botfun knows of (gunfire, someone hurt close by, an enemy seen
// in the last few seconds)
bool:danger(id)
{
	return g_BotFun && bf_danger(id)
}

// Standing on something solid, not on a ladder or in water, not using anything: somewhere
// a person would stop for a moment
bool:steady(id, buttons)
{
	return (pev(id, pev_flags) & FL_ONGROUND) && pev(id, pev_movetype) != MOVETYPE_FLY
		&& pev(id, pev_waterlevel) < 2 && !(buttons & IN_USE)
}

// In the air or on a ladder: an afk or a held angle ends
bool:unsteady(id)
{
	return !(pev(id, pev_flags) & FL_ONGROUND) || pev(id, pev_movetype) == MOVETYPE_FLY
}

// A match (gm_scrim): only the guns, nothing silly
bool:is_match()
{
	new phase = get_cvar_num("gm_scrim_phase")
	return phase > 0 && phase < 5
}

// Played like a match: guns only, nothing silly (a match, match mode, and the Matchmaking
// style on Classic)
bool:match_like()
{
	return is_match() || g_MatchMode || (g_ClassicMode && style() == STYLE_MM)
}

// A deathmatch-type mode (its cfg sets bot_deathmatch 1): no afk or holding angles for fun
bool:dm_mode()
{
	return g_pBotDm && get_pcvar_num(g_pBotDm) != 0
}

// The gun back out, only if it's the knife in hand (lastinv on anything else could
// undo what the bot's own weapon logic picked)
ensure_gun(id)
{
	if (get_user_weapon(id) == CSW_KNIFE)
		engclient_cmd(id, "lastinv")
}

public native_filter(const name[], index, trap)
{
	return (!trap && (equal(name, "botchat_", 8) || equal(name, "bf_", 3))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

// The bot's chat personality letter, or 0 (gm_botchat isn't running)
persona(id)
{
	return g_Chat ? botchat_persona(id) : 0
}

say_line(id, const section[], other, Float:chance)
{
	if (g_Chat)
		botchat_say(id, section, other, chance)
}

bool:has(p, const letters[])
{
	return p && contain(letters, fmt("%c", p)) != -1
}

// gm_personality as a factor: 0 plain, 1 normal, 2 exaggerated
Float:pers()
{
	return float(clamp(get_pcvar_num(g_pPersonality), 0, 200)) / 100.0
}

// A factor's pull away from 1.0, made stronger or weaker by gm_personality
Float:pers_factor(Float:k)
{
	return floatmax(1.0 + (k - 1.0) * pers(), 0.3)
}

// How bots play together on this map: team, solo or hold (gamemodes sets gm_botplay)
botplay()
{
	return g_pBotPlay ? get_pcvar_num(g_pBotPlay) : PLAY_SOLO
}

// How a personality changes a bot's skill (higher = slower and sloppier)
Float:habit_skill(id)
{
	return pers_factor(habit_skill_raw(id))
}

Float:habit_skill_raw(id)
{
	if (!get_pcvar_num(g_pHabits))
		return 1.0
	new p = persona(id), Float:k = 1.0
	if (has(p, "nkuL"))
		k = 1.12                                   // new, young or down on themselves
	else if (has(p, "tEiSN"))
		k = 0.93                                   // tryhards, would-be pros, movement and stats nerds
	else if (has(p, "O"))
		k = 1.05                                   // overconfident: peeks too much
	// warming up: the first couple of minutes after joining are a bit off
	new Float:on = get_gametime() - g_JoinedAt[id]
	if (on < 120.0)
		k *= 1.0 + 0.15 * (1.0 - on / 120.0)
	// tired: the half-asleep get slower the longer they play, everyone a bit after 1am
	if (has(p, "dY"))
		k *= 1.0 + floatmin(on / 60.0 * 0.005, 0.25)
	new h
	time(h)
	if (h >= 1 && h < 5)
		k *= 1.08
	return k
}

// How long a personality holds the trigger
Float:habit_burst(id)
{
	return pers_factor(habit_burst_raw(id))
}

Float:habit_burst_raw(id)
{
	if (!get_pcvar_num(g_pHabits))
		return 1.0
	new p = persona(id)
	if (has(p, "o1"))
		return 0.6                                 // old-timers and cowboys tap
	if (has(p, "nkhX"))
		return 1.6                                 // spray and pray
	if (has(p, "tE"))
		return 0.8
	return 1.0
}

// A favourite gun, from the bot's name (most have none)
fav_of(id)
{
	new name[32], h = 5381
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = ((h << 5) + h + name[i]) & 0xFFFFFF
	new p = persona(id), r = h % 100
	// fewer favourites with a weaker personality, none when plain
	new Float:k = pers()
	if (k <= 0.0)
		return 0
	r = floatround(float(r) / floatmin(k, 1.5))
	if (has(p, "o1") && r < 50)
		return CSW_DEAGLE
	// the AWPer, and the one who plays the odd guns (shotguns, the TMP, Elites...)
	if (has(p, "a") && r < 85)
		return CSW_AWP
	if (has(p, "y") && r < 85)
	{
		static const ODD[] = { CSW_M3, CSW_XM1014, CSW_TMP, CSW_MAC10, CSW_P90, CSW_ELITE }
		return ODD[(h / 101) % sizeof ODD]
	}
	if (has(p, "S") && r < 60)
		return CSW_SCOUT
	if (r < 8)
		return CSW_AWP
	if (r < 14)
		return CSW_DEAGLE
	if (r < 17)
		return CSW_SCOUT
	return 0
}

public plugin_cfg()
{
	g_pFreeForAll = get_cvar_pointer("mp_freeforall")
	// Tour of Duty missions keep their designed bots
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
	g_pBotPlay = get_cvar_pointer("gm_botplay")
	// holding an angle and afk fit modes where bots fight; not ones a plugin steers them
	// through (gm_botnav in hns and deathrun sets their velocity every frame)
	new mode[32]
	read_mode(mode, charsmax(mode))
	// (gm_botfun skips the same modes: keep the two lists alike)
	static const NO_IDLE[][] = { "hns", "deathrun", "kreedz", "surf", "soccerjam", "zombie", "zp5", "basebuilder" }
	g_IdleMode = true
	for (new i = 0; i < sizeof NO_IDLE; i++)
		if (equal(mode, NO_IDLE[i]))
			g_IdleMode = false
	g_MatchMode = bool:equal(mode, "match")
	g_ClassicMode = bool:equal(mode, "classic")
	g_pBotDm = get_cvar_pointer("bot_deathmatch")
	g_BotFun = bool:LibraryExists("gm_botfun", LibType_Library)
	update_forwards(style(), get_pcvar_num(g_pAim))
}

// (the hook can run before the cvar holds its new value: read it from new_value)
public cvar_changed(pcvar, const old_value[], const new_value[])
{
	new v = str_to_num(new_value)
	if (pcvar == g_pStyle)
		update_forwards((g_Career || !(STYLE_CLASSIC < v < STYLE_COUNT || v == STYLE_MIXED)) ? STYLE_CLASSIC : v, get_pcvar_num(g_pAim))
	else
		update_forwards(style(), v)
}

// Hooks what a style needs: thinking and commands for any style but Classic, and the
// bullet traces (with PostThink, which marks them) only while gm_hb_aim bends them.
update_forwards(s, aim)
{
	new bool:on = s != STYLE_CLASSIC
	g_FwCmdStart = set_forward(on, FM_CmdStart, "fw_cmdstart", g_FwCmdStart, 0)
	g_FwPreThink = set_forward(on, FM_PlayerPreThink, "fw_prethink", g_FwPreThink, 0)
	on = on && aim != 0
	g_FwPostThink = set_forward(on, FM_PlayerPostThink, "fw_postthink", g_FwPostThink, 0)
	g_FwPostThinkPost = set_forward(on, FM_PlayerPostThink, "fw_postthink_post", g_FwPostThinkPost, 1)
	g_FwTraceLine = set_forward(on, FM_TraceLine, "fw_traceline", g_FwTraceLine, 0)
	if (!on)
		g_InThink = 0
}

// Registers or unregisters one forward; returns its id (0 when off).
set_forward(bool:on, type, const func[], id, post)
{
	if (on && !id)
		return register_forward(type, func, post)
	if (!on && id)
	{
		unregister_forward(type, id, post)
		return 0
	}
	return id
}

style()
{
	if (g_Career)
		return STYLE_CLASSIC
	new s = get_pcvar_num(g_pStyle)
	return (STYLE_CLASSIC < s < STYLE_COUNT || s == STYLE_MIXED) ? s : STYLE_CLASSIC
}

// The style a bot plays with: the server's, or in a Mixed lobby its own
bot_style(id)
{
	new s = style()
	if (s != STYLE_MIXED)
		return s
	if (!g_Mix[id])
	{
		// like a real lobby: lots of average players, some new, a few very good. From the
		// name, so a regular plays at the same level every time; its personality leans it
		new r = name_hash(id) % 100, p = persona(id)
		if (has(p, "nkuL"))
			r = r / 2                       // toward newbie
		else if (has(p, "tESNi"))
			r = 50 + r / 2                  // toward veteran
		g_Mix[id] = r < 22 ? STYLE_NEWBIE : r < 57 ? STYLE_PUB : r < 80 ? STYLE_MM : r < 95 ? STYLE_VET : STYLE_LOCKED
	}
	return g_Mix[id]
}

name_hash(id)
{
	new name[32], h = 33
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h * 131 + name[i]) & 0xFFFFFF
	return h
}

// How easily a style panics
Float:panic_factor(s)
{
	switch (s)
	{
		case STYLE_NEWBIE: return 1.5
		case STYLE_PUB: return 1.0
		case STYLE_MM: return 0.5
		case STYLE_VET: return 0.3
		case STYLE_LOCKED: return 0.15
	}
	return 0.0
}

// The bot's skill right now (higher = slower and sloppier), with its mood
Float:skill_of(id)
{
	return g_Skill[id] * g_Habit[id] * (1.0 + (get_pcvar_num(g_pTilt) ? g_Tilt[id] * g_TiltWay[id] * pers() : 0.0))
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2)
	// nothing of this life carries over to the next
	if (victim >= 1 && victim <= MaxClients)
		reset_bot(victim)
	if (victim >= 1 && victim <= MaxClients && killer != victim)
	{
		g_Tilt[victim] = floatmin(g_Tilt[victim] + 0.08, 0.3)
		// the player who keeps killing this bot becomes its rival
		if (killer >= 1 && killer <= MaxClients && ++g_DeathsBy[victim][killer] >= 3)
			g_Rival[victim] = killer
	}
	if (killer >= 1 && killer <= MaxClients && killer != victim && is_user_bot(killer))
	{
		new p = persona(killer)
		radio(killer, "enemydown", has(p, "r") ? 0.12 : has(p, "it") ? 0.06 : 0.02, 30.0, 15.0)
	}
	if (killer < 1 || killer > MaxClients || killer == victim || !is_user_bot(killer))
		return
	g_Tilt[killer] = floatmax(g_Tilt[killer] - 0.08, -0.1)
	g_RoundKills[killer]++
	// a show-off moment, now and then (not in a match), and only when it's over: no other
	// enemy in view and not hurt lately
	if (style() == STYLE_CLASSIC || match_like())
		return
	new Float:now = get_gametime()
	if ((g_Target[killer] && g_Target[killer] != victim) || now - g_HurtAt[killer] < 4.0 || g_Show[killer] || g_KnifeOut[killer])
		return
	new p = persona(killer)
	// a tag on the wall after a kill, for the ones who spray a lot (not in deathmatch: no
	// time to stand at a wall)
	if (get_pcvar_num(g_pSprays) && g_IdleMode && !dm_mode() && (g_SprayCount || g_LogoSpray)
		&& random_float(0.0, 1.0) < (has(p, "9") ? 0.25 : has(p, "jkhXS3") ? 0.06 : 0.015) * pers())
		set_task(random_float(1.2, 2.5), "task_spray", TASK_SPRAY + get_user_userid(killer))
	if (!get_pcvar_num(g_pShowoff))
		return
	// mostly the jokers and show-offs; a better kill (a headshot, the third this round)
	// makes it likelier
	new Float:chance = (has(p, "jkhOmX") ? 0.04 : p ? 0.01 : 0.02) * pers()
	chance *= floatmin(1.0 + 0.5 * float(g_RoundKills[killer] - 1), 2.5) * (read_data(3) ? 1.6 : 1.0)
	if (random_float(0.0, 1.0) >= chance)
		return
	g_ShowHp[killer] = get_user_health(killer)
	// crouch spam over the body is disrespect: only when the server allows it
	new pick = get_pcvar_num(g_pDisrespect) ? random(4) : 2 + random(2)
	switch (pick)
	{
		case 0, 1:
		{
			g_Show[killer] = 1
			g_ShowUntil[killer] = now + random_float(1.2, 2.2)
			// no set rhythm: each press its own length, starting anywhere
			g_ShowDuck[killer] = bool:random(2)
			g_ShowNext[killer] = now + random_float(0.0, 0.15)
		}
		case 2: { g_Show[killer] = 2; g_ShowUntil[killer] = now + 0.3; }
		default:
		{
			if (!holding_gun(killer))
				return
			g_KnifeOut[killer] = true
			engclient_cmd(killer, "weapon_knife")
			set_task(random_float(1.5, 3.0) + 0.3 * float(min(g_RoundKills[killer], 4)), "task_lastinv", TASK_KNIFE + killer)
		}
	}
}

// The show-off knife goes back, whatever happened meanwhile (engage may have done it already)
public task_lastinv(tid)
{
	new id = tid - TASK_KNIFE
	if (!g_KnifeOut[id])
		return
	g_KnifeOut[id] = false
	if (is_user_alive(id) && is_user_bot(id))
		ensure_gun(id)
}

public ev_damage(victim)
{
	if (victim >= 1 && victim <= MaxClients)
		g_HurtAt[victim] = get_gametime()
}

// (the living respawn at a new round with what they held: a knife of ours goes back)
public fw_spawn_post(id)
{
	if (!is_user_alive(id))
		return
	if (is_user_bot(id) && (g_KnifeOut[id] || g_KnifeRun[id]))
		ensure_gun(id)
	reset_bot(id)
}

public client_putinserver(id)
{
	reset_bot(id)
	g_SkillStyle[id] = STYLE_CLASSIC
	g_Mix[id] = 0
	g_Tilt[id] = 0.0
	// most people get sloppy when it goes badly; some get careful
	g_TiltWay[id] = (name_hash(id) / 7) % 100 < 70 ? 1.0 : -0.5
	g_JoinedAt[id] = get_gametime()
	g_Habit[id] = 1.0
	g_Rival[id] = 0
	// nobody holds a grudge against whoever had this slot before
	for (new k = 1; k <= MaxClients; k++)
		if (g_Rival[k] == id)
			g_Rival[k] = 0
	g_Fav[id] = -1
	for (new k = 1; k <= MaxClients; k++)
		g_DeathsBy[id][k] = g_DeathsBy[k][id] = 0
	g_NextRadio[id] = g_NextHoldTry[id] = g_FollowOpenUntil[id] = g_NextFollowRoll[id] = 0.0
}

// each bot gets its own skill within the style (higher = slower and sloppier)
sample_skill(id, s)
{
	g_SkillStyle[id] = s
	// Pub lobbies are lopsided: a few sharp players, a crowd of average ones, some bad
	new Float:mid = (STYLES[s][ST_SkillLo] + STYLES[s][ST_SkillHi]) / 2.0
	if (s == STYLE_PUB)
	{
		new r = random(10)
		g_Skill[id] = r < 2 ? random_float(0.55, 0.75) : r < 7 ? random_float(0.85, 1.15) : random_float(1.3, 1.8)
		mid = 1.0
	}
	else
		g_Skill[id] = random_float(STYLES[s][ST_SkillLo], STYLES[s][ST_SkillHi])
	// gm_personality: bots closer together in skill when plain, further apart when exaggerated
	// (not Hackers and Gods: their snapping is the point)
	if (!STYLES[s][ST_HeadPct])
		g_Skill[id] = floatclamp(mid + (g_Skill[id] - mid) * (0.5 + 0.5 * pers()), 0.3, 2.6)
}

reset_bot(id)
{
	g_Target[id] = g_LastTarget[id] = 0
	g_Unlock[id] = g_FlickDur[id] = 0.0
	g_WobbleYaw[id] = g_WobblePitch[id] = 0.0
	g_Burst[id] = 0
	g_PauseUntil[id] = g_PanicUntil[id] = 0.0
	g_Show[id] = 0
	g_ReloadAt[id] = g_AfkUntil[id] = 0.0
	g_KnifeRun[id] = g_KnifeOut[id] = false
	g_HoldUntil[id] = g_AfkAt[id] = 0.0
	g_AfkOn[id] = false
	g_HopsLeft[id] = 0
	g_FailHops[id] = false
	g_RoundKills[id] = 0
	g_HurtAt[id] = 0.0
	remove_task(TASK_KNIFE + id)
}

// ---------------------------------------------------------------------------
// Who is each bot looking at? (10 times a second)
// ---------------------------------------------------------------------------

bool:is_enemy(a, b)
{
	if (a == b || !is_user_alive(b))
		return false
	if (g_pFreeForAll && get_pcvar_num(g_pFreeForAll))
		return true
	return cs_get_user_team(a) != cs_get_user_team(b)
}

eye_pos(id, Float:out[3])
{
	new Float:ofs[3]
	pev(id, pev_origin, out)
	pev(id, pev_view_ofs, ofs)
	out[0] += ofs[0]
	out[1] += ofs[1]
	out[2] += ofs[2]
}

bool:can_see(id, const Float:from[3], target)
{
	new Float:to[3], tr = create_tr2()
	eye_pos(target, to)
	engfunc(EngFunc_TraceLine, from, to, DONT_IGNORE_MONSTERS, id, tr)
	new Float:frac
	get_tr2(tr, TR_flFraction, frac)
	new hit = get_tr2(tr, TR_pHit)
	if (frac < 1.0 && hit != target)
	{
		// head hidden? try the chest
		pev(target, pev_origin, to)
		engfunc(EngFunc_TraceLine, from, to, DONT_IGNORE_MONSTERS, id, tr)
		get_tr2(tr, TR_flFraction, frac)
		hit = get_tr2(tr, TR_pHit)
	}
	free_tr2(tr)
	return frac >= 1.0 || hit == target
}

public task_scan()
{
	if (style() == STYLE_CLASSIC)
		return
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		new s = bot_style(id), seen
		// the personality factor, worked out here rather than every frame
		g_Habit[id] = habit_skill(id)

		new Float:eye[3], Float:view[3], Float:fwd[3]
		eye_pos(id, eye)
		pev(id, pev_v_angle, view)
		angle_vector(view, ANGLEVECTOR_FORWARD, fwd)

		new best, Float:bestAngle = STYLES[s][ST_ViewFov], Float:bestDist
		for (new e = 1; e <= MaxClients; e++)
		{
			if (!is_enemy(id, e))
				continue
			new Float:to[3], Float:dir[3]
			eye_pos(e, to)
			for (new i = 0; i < 3; i++)
				dir[i] = to[i] - eye[i]
			new Float:dist = vector_length(dir)
			if (dist < 1.0)
				continue
			new Float:dot = (dir[0] * fwd[0] + dir[1] * fwd[1] + dir[2] * fwd[2]) / dist
			new Float:angle = floatacos(floatclamp(dot, -1.0, 1.0), degrees)
			// one line-of-sight check per enemy in view (traces are the expensive part)
			if (angle >= STYLES[s][ST_ViewFov] || !can_see(id, eye, e))
				continue
			seen++
			if (angle < bestAngle)
			{
				best = e
				bestAngle = angle
				bestDist = dist
			}
		}

		// holding an angle: over when it's hit or the bomb goes down (CTs go for it)
		if (g_HoldUntil[id] > 0.0 && (best || get_user_health(id) < g_HoldHp[id] || g_BombPlanted && get_user_team(id) == 2 || now >= g_HoldUntil[id]))
		{
			g_HoldUntil[id] = 0.0
			g_NextHoldTry[id] = now + random_float(15.0, 40.0)
		}
		if (!best)
		{
			if (g_Target[id])
			{
				g_LastTarget[id] = g_Target[id]
				g_LostAt[id] = now
				g_Target[id] = 0
				g_NoTargetSince[id] = now
				// a nervous reload after a fight (sometimes right as someone peeks)
				if (get_pcvar_num(g_pHabits) && random_float(0.0, 1.0) < (has(persona(id), "nkuH") ? 0.35 : 0.12) * pers())
					g_ReloadAt[id] = now + random_float(0.2, 0.9)
			}
			else if (get_pcvar_num(g_pHabits) && now >= g_NextKnifeTry[id])
				knife_run(id, now)
			// the early part of the round is over: the gun comes back out
			if (g_KnifeRun[id] && now - g_RoundStartAt > KNIFE_WINDOW + 2.0)
			{
				g_KnifeRun[id] = false
				ensure_gun(id)
			}
			if (!g_Target[id] && now >= g_NextHoldTry[id] && g_HoldUntil[id] == 0.0)
				try_hold(id, now)
			continue
		}
		// several enemies at once, or nearly dead: some panic for a moment
		if (get_pcvar_num(g_pTilt) && get_pcvar_num(g_pMistakes) && g_PanicUntil[id] < now
			&& (seen >= 2 || get_user_health(id) < 30) && random_float(0.0, 1.0) < 0.006 * panic_factor(s) * pers())
			g_PanicUntil[id] = now + random_float(1.0, 2.0)
		g_TargetDist[id] = bestDist
		g_TargetAngVel[id] = angular_speed(id, best, bestDist)
		if (best != g_Target[id])
			engage(id, best, bestAngle, bestDist, s, now)
	}
}

// how fast the target sweeps across the bot's view, degrees per second
Float:angular_speed(id, target, Float:dist)
{
	new Float:a[3], Float:b[3], Float:va[3], Float:vb[3], Float:u[3], Float:rel[3]
	eye_pos(id, a)
	eye_pos(target, b)
	pev(id, pev_velocity, va)
	pev(target, pev_velocity, vb)
	for (new i = 0; i < 3; i++)
	{
		u[i] = (b[i] - a[i]) / dist
		rel[i] = vb[i] - va[i]
	}
	new Float:along = rel[0] * u[0] + rel[1] * u[1] + rel[2] * u[2]
	for (new i = 0; i < 3; i++)
		rel[i] -= along * u[i]
	return floatatan(vector_length(rel) / dist, degrees)
}

// A new enemy came into view: when can the bot first shoot, and how far off
// will its first shots be?
engage(id, target, Float:angle, Float:dist, s, Float:now)
{
	new bool:recent = target == g_LastTarget[id] && now - g_LostAt[id] < 1.5
	g_Target[id] = target
	if (g_SkillStyle[id] != s)
		sample_skill(id, s)

	new Float:skill = skill_of(id)
	new Float:rt = ex_gaussian(STYLES[s][ST_RtMu], STYLES[s][ST_RtSigma], STYLES[s][ST_RtTau]) * skill
	rt *= (1.0 + dist / 3000.0) * (1.0 + angle / 90.0)
	new Float:vel[3]
	pev(target, pev_velocity, vel)
	if (vector_length(vel) > 120.0)
		rt *= 0.85                          // movement catches the eye
	if (recent)
		rt *= 0.45                          // knew they were there
	if (get_pcvar_num(g_pHabits))
	{
		if (target == g_Rival[id])
			rt *= pers_factor(0.85)         // been waiting for this one
	}
	if (g_KnifeRun[id] || g_KnifeOut[id])
	{
		// a knife of ours out (running, showing off): the gun comes back first
		g_KnifeRun[id] = g_KnifeOut[id] = false
		remove_task(TASK_KNIFE + id)
		if (get_user_weapon(id) == CSW_KNIFE)
			rt += 0.35
		ensure_gun(id)
	}
	if (!recent)
		radio(id, "enemyspot", has(persona(id), "itr") ? 0.06 : 0.02, 45.0, 20.0)

	new Float:over = (angle * STYLES[s][ST_Overshoot] * random_float(0.5, 1.5) + 0.6) * skill
	if (recent)
		over *= 0.4
	// surprised from behind: the first spray goes wide
	if (get_pcvar_num(g_pHabits) && angle > 100.0 && random_float(0.0, 1.0) < 0.5 * pers())
	{
		over *= 1.8
		rt *= 1.2
	}

	if (get_pcvar_num(g_pMistakes) && random_float(0.0, 1.0) < STYLES[s][ST_Mistakes] * skill * (0.5 + 0.5 * pers()))
	{
		switch (random(3))
		{
			case 0: over *= 3.0                       // whiffed the flick
			case 1: rt *= 2.0                         // didn't notice in time
			case 2: g_PanicUntil[id] = now + 1.5      // panic spray
		}
	}
	over = floatclamp(over, 0.3, 12.0)

	g_Unlock[id] = get_pcvar_num(g_pReaction) ? now + rt : now
	g_FlickStart[id] = g_Unlock[id]
	g_FlickDur[id] = STYLES[s][ST_FlickTime] * (0.6 + 0.4 * log2(1.0 + angle / 4.0)) * skill

	// the overshoot carries on in the direction of the turn
	new Float:eye[3], Float:to[3], Float:dir[3], Float:want[3], Float:view[3]
	eye_pos(id, eye)
	eye_pos(target, to)
	for (new i = 0; i < 3; i++)
		dir[i] = to[i] - eye[i]
	vector_to_angle(dir, want)
	pev(id, pev_v_angle, view)
	new Float:dYaw = wrap(want[1] - view[1])
	new Float:dPitch = want[0] + view[0]    // v_angle pitch is down-positive, vector_to_angle up-positive
	new Float:total = floatabs(dYaw) + floatabs(dPitch) + 0.01
	g_OverYaw[id] = over * (floatabs(dYaw) / total + 0.2) * (dYaw >= 0.0 ? 1.0 : -1.0)
	g_OverPitch[id] = over * (floatabs(dPitch) / total) * (dPitch >= 0.0 ? 1.0 : -1.0) * 0.6
	g_Burst[id] = 0
	new_burst(id, s)
}

new_burst(id, s)
{
	new len = random_num(STYLES[s][ST_BurstMin], STYLES[s][ST_BurstMax])
	if (g_TargetDist[id] < 500.0)
		len = len * 9 / 5                  // up close everyone holds the trigger longer
	if (g_PanicUntil[id] > get_gametime())
		len *= 2
	len = floatround(float(len) * habit_burst(id))
	g_BurstLen[id] = max(len, 1)
	g_SprayDrift[id] = random_float(-0.25, 0.25) * skill_of(id)
}

// ---------------------------------------------------------------------------
// Every frame: hold the trigger, move the wobble along
// ---------------------------------------------------------------------------

bool:holding_gun(id)
{
	new item = get_pdata_cbase(id, OFFSET_ACTIVE_ITEM)
	if (!pev_valid(item))
		return false
	switch (cs_get_weapon_id(item))
	{
		case CSW_KNIFE, CSW_HEGRENADE, CSW_FLASHBANG, CSW_SMOKEGRENADE, CSW_C4: return false
	}
	return true
}

public fw_cmdstart(id, uc, seed)
{
	if (!is_user_alive(id) || !is_user_bot(id))
		return FMRES_IGNORED
	new s = bot_style(id)
	if (s == STYLE_CLASSIC)
		return FMRES_IGNORED

	new Float:now = get_gametime()
	new Float:dt = floatclamp(float(get_uc(uc, UC_Msec)) / 1000.0, 0.001, 0.1)
	update_wobble(id, s, dt, now)

	// a fight: an enemy in view, or the bot's own brain shooting at one (maybe outside the
	// plugin's view cone), or danger close by (gunfire, someone hurt, an enemy seen just
	// now). Read before anything here changes the buttons
	new buttons0 = get_uc(uc, UC_Buttons)
	new bool:fight = g_Target[id] || (buttons0 & IN_ATTACK) || danger(id)
	// a knife of ours out (running, showing off): the gun comes back
	if (fight && (g_KnifeRun[id] || g_KnifeOut[id]))
	{
		g_KnifeRun[id] = g_KnifeOut[id] = false
		remove_task(TASK_KNIFE + id)
		ensure_gun(id)
	}

	// standing still for a moment (afk): no shooting either. It starts once it's standing
	// somewhere solid; over at once in a fight, when it's hurt, or off the ground
	if (g_AfkUntil[id] > 0.0 && now >= g_AfkAt[id])
	{
		new bool:on = g_AfkOn[id]
		new bool:ok = now < g_AfkUntil[id] && !fight && get_user_health(id) >= g_AfkHp[id] && !user_has_weapon(id, CSW_C4)
		if (ok && (on ? !unsteady(id) : steady(id, buttons0)))
		{
			g_AfkOn[id] = true
			set_uc(uc, UC_Buttons, 0)
			set_uc(uc, UC_ForwardMove, 0.0)
			set_uc(uc, UC_SideMove, 0.0)
			return FMRES_IGNORED
		}
		// (not started yet and not standing anywhere solid: it waits)
		if (!ok || on)
		{
			g_AfkUntil[id] = g_AfkAt[id] = 0.0
			g_AfkOn[id] = false
			if (on)
				say_line(id, "afk_back", 0, 0.5)
		}
	}
	// holding an angle: no walking, sometimes crouched; it still looks around. A fight, or
	// leaving the ground, ends it
	if (g_HoldUntil[id] > now)
	{
		if (fight || unsteady(id) || (buttons0 & IN_USE))
		{
			g_HoldUntil[id] = 0.0
			g_NextHoldTry[id] = now + random_float(15.0, 40.0)
		}
		else
		{
			new b = get_uc(uc, UC_Buttons) & ~(IN_FORWARD | IN_BACK | IN_MOVELEFT | IN_MOVERIGHT | IN_JUMP | IN_RUN)
			set_uc(uc, UC_Buttons, g_HoldDuck[id] ? (b | IN_DUCK) : b)
			set_uc(uc, UC_ForwardMove, 0.0)
			set_uc(uc, UC_SideMove, 0.0)
		}
	}
	// a nervous reload, unless someone showed up meanwhile (the danger it just fought
	// through is why it reloads)
	if (g_ReloadAt[id] > 0.0 && now >= g_ReloadAt[id])
	{
		g_ReloadAt[id] = 0.0
		if (!g_Target[id] && !(buttons0 & IN_ATTACK))
			set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) | IN_RELOAD)
	}
	// a show-off moment, only while nothing's going on: a fight or getting hurt ends it
	if (g_Show[id])
	{
		if (now >= g_ShowUntil[id] || fight || get_user_health(id) < g_ShowHp[id])
			g_Show[id] = 0
		else
		{
			new b = get_uc(uc, UC_Buttons)
			if (g_Show[id] == 1)
			{
				// crouch spam: each press and release its own length, no set rhythm
				if (now >= g_ShowNext[id])
				{
					g_ShowDuck[id] = !g_ShowDuck[id]
					g_ShowNext[id] = now + random_float(0.09, 0.16)
				}
				b = g_ShowDuck[id] ? (b | IN_DUCK) : (b & ~IN_DUCK)
			}
			else
				b |= IN_JUMP
			set_uc(uc, UC_Buttons, b)
			return FMRES_IGNORED
		}
	}
	// panicking players jump about
	if (g_PanicUntil[id] > now && (s == STYLE_PUB || s == STYLE_NEWBIE) && random_float(0.0, 1.0) < 0.02 * pers())
		set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) | IN_JUMP)

	new buttons = get_uc(uc, UC_Buttons)
	if (!(buttons & IN_ATTACK) || !holding_gun(id))
		return FMRES_IGNORED

	new bool:hold = false
	if (g_Target[id] && now < g_Unlock[id])
		hold = true                             // still reacting
	else if (get_pcvar_num(g_pAim) && now < g_PauseUntil[id])
		hold = true                             // between bursts
	if (hold)
		set_uc(uc, UC_Buttons, buttons & ~IN_ATTACK)
	return FMRES_IGNORED
}

update_wobble(id, s, Float:dt, Float:now)
{
	new Float:vel[3]
	pev(id, pev_velocity, vel)
	new Float:own = floatsqroot(vel[0] * vel[0] + vel[1] * vel[1])
	if (g_SkillStyle[id] != s)
		sample_skill(id, s)
	new Float:sigma = STYLES[s][ST_Wobble] * skill_of(id) * (1.0 + g_TargetAngVel[id] / 40.0 + own / 260.0)
	if (g_PanicUntil[id] > now)
		sigma *= 1.6
	// Ornstein-Uhlenbeck: pulled back toward zero, pushed by noise; std settles at sigma
	new Float:theta = 5.0
	new Float:kick = sigma * floatsqroot(2.0 * theta * dt)
	g_WobbleYaw[id] += -theta * g_WobbleYaw[id] * dt + kick * gaussian()
	g_WobblePitch[id] += -theta * g_WobblePitch[id] * dt + kick * gaussian() * 0.6
}

// ---------------------------------------------------------------------------
// Bunnyhopping (Locked In, Hackers): when running in a straight line with no
// fight going on, chain a few hops, each one carrying a bit more speed.
// Same trick as gamemodes' Auto bunnyhop: jump by setting the velocity.
// ---------------------------------------------------------------------------

public fw_prethink(id)
{
	if (!is_user_alive(id) || !is_user_bot(id))
		return
	if ((afk_now(id, get_gametime()) || g_HoldUntil[id] > get_gametime()) && !g_Target[id])
	{
		new Float:v[3]
		pev(id, pev_velocity, v)
		v[0] = v[1] = 0.0
		set_pev(id, pev_velocity, v)
		return
	}
	new s = bot_style(id)
	// chance per check (every 0.5s of straight running), and hops per chain
	new Float:chance, Float:gain, Float:cap, hopsLo, hopsHi, bool:fail
	switch (s)
	{
		case STYLE_LOCKED: { chance = 0.12; gain = 1.06; cap = 320.0; hopsLo = 2; hopsHi = 5; }
		case STYLE_HACKER: { chance = 0.7;  gain = 1.10; cap = 400.0; hopsLo = 4; hopsHi = 10; }
		case STYLE_GOD:    { chance = 0.95; gain = 1.15; cap = 500.0; hopsLo = 6; hopsHi = 15; }
		case STYLE_PUB, STYLE_NEWBIE:
		{
			// a pub player who's seen bunnyhopping and can't do it: a hop or three that lose
			// speed (gm_hb_habits; not in a match, nor where a plugin steers the bots)
			if (!get_pcvar_num(g_pHabits) || !fail_hopper(id) || match_like() || !g_IdleMode)
			{
				g_HopsLeft[id] = 0
				return
			}
			chance = 0.004 * pers(); gain = 0.9; cap = 300.0; hopsLo = 1; hopsHi = 3; fail = true
		}
		default:
		{
			g_HopsLeft[id] = 0
			return
		}
	}
	if (!fail && !get_pcvar_num(g_pBhop))
		return
	if (g_HopsLeft[id] && !g_FailHops[id])
		set_pev(id, pev_fuser2, 0.0)     // no slowdown after landing (a failed hop keeps it)
	if (!(pev(id, pev_flags) & FL_ONGROUND))
		return

	new Float:now = get_gametime()
	if (!g_HopsLeft[id])
	{
		if (now < g_NextHopTry[id])
			return
		g_NextHopTry[id] = now + 0.5
		if (random_float(0.0, 1.0) >= chance || !hop_clear(id) || (fail && danger(id)))
			return
		g_HopsLeft[id] = random_num(hopsLo, hopsHi)
		g_FailHops[id] = fail
		if (!fail)
			say_line(id, "bunnyhop", 0, 0.04)
	}
	else if (!hop_clear(id))
	{
		g_HopsLeft[id] = 0
		return
	}

	new Float:vel[3]
	pev(id, pev_velocity, vel)
	new Float:speed = floatsqroot(vel[0] * vel[0] + vel[1] * vel[1])
	new Float:scale = floatmin(speed * gain, cap) / speed
	vel[0] *= scale
	vel[1] *= scale
	vel[2] = 250.0
	set_pev(id, pev_velocity, vel)
	set_pev(id, pev_gaitsequence, 6)
	g_HopsLeft[id]--
}

// Some pub players try to bunnyhop now and then (and fail): kids, zoomers, jokers, the
// movement nerds most, from the name so a regular keeps it
bool:fail_hopper(id)
{
	new r = (name_hash(id) / 31) % 100, p = persona(id)
	return r < (has(p, "kzjS") ? 45 : has(p, "hnm") ? 20 : 4)
}

// running straight, not fighting, nothing in the way and somewhere to land
bool:hop_clear(id)
{
	if (g_Target[id] && g_TargetDist[id] < 1500.0)
		return false
	if (pev(id, pev_flags) & FL_DUCKING || pev(id, pev_movetype) == MOVETYPE_FLY || pev(id, pev_waterlevel) >= 2)
		return false

	new Float:vel[3], Float:view[3]
	pev(id, pev_velocity, vel)
	new Float:speed = floatsqroot(vel[0] * vel[0] + vel[1] * vel[1])
	if (speed < 180.0)
		return false
	pev(id, pev_v_angle, view)
	new Float:heading = floatatan2(vel[1], vel[0], degrees)
	if (floatabs(wrap(heading - view[1])) > 25.0)
		return false                      // turning or strafing: a hop would overshoot the path

	// where a hop lands (about 0.6s of flight)
	new Float:from[3], Float:to[3], Float:down[3], Float:frac
	pev(id, pev_origin, from)
	new Float:reach = speed * 0.62
	to[0] = from[0] + vel[0] / speed * reach
	to[1] = from[1] + vel[1] / speed * reach
	to[2] = from[2]
	new tr = create_tr2()
	engfunc(EngFunc_TraceHull, from, to, IGNORE_MONSTERS, HULL_HUMAN, id, tr)
	get_tr2(tr, TR_flFraction, frac)
	if (frac < 1.0)
	{
		free_tr2(tr)
		return false                      // wall or doorframe ahead
	}
	down = to
	down[2] -= 76.0                       // feet are 36 below the origin; allow a 40 unit drop
	engfunc(EngFunc_TraceLine, to, down, IGNORE_MONSTERS, id, tr)
	get_tr2(tr, TR_flFraction, frac)
	free_tr2(tr)
	return frac < 1.0                     // no floor there: ledge
}

// ---------------------------------------------------------------------------
// Bending bullets: weapons fire during PostThink, so long traces from the bot
// while it thinks are its bullets
// ---------------------------------------------------------------------------

public fw_postthink(id)
{
	if (is_user_alive(id) && is_user_bot(id))
	{
		g_InThink = id
		g_Fired[id] = false
	}
}

public fw_postthink_post(id)
{
	g_InThink = 0
	if (!g_Fired[id])
	{
		if (g_Burst[id] && get_gametime() - g_LastFire[id] > 0.3)
			g_Burst[id] = 0                      // let go of the trigger: recoil resets
		return
	}
	new s = bot_style(id)
	new Float:now = get_gametime()
	g_Fired[id] = false
	g_LastFire[id] = now
	if (++g_Burst[id] >= g_BurstLen[id] && s != STYLE_CLASSIC)
	{
		g_PauseUntil[id] = now + random_float(0.18, 0.4) * (g_PanicUntil[id] > now ? 0.5 : 1.0)
		g_Burst[id] = 0
		new_burst(id, s)
	}
}

public fw_traceline(const Float:start[3], const Float:end[3], cond, skip, tr)
{
	if (g_Bending || !g_InThink || skip != g_InThink || !get_pcvar_num(g_pAim) || style() == STYLE_CLASSIC)
		return FMRES_IGNORED
	new Float:dir[3]
	for (new i = 0; i < 3; i++)
		dir[i] = end[i] - start[i]
	new Float:len = vector_length(dir)
	if (len < 1024.0)                      // bullets trace 4096+ units; this is something else
		return FMRES_IGNORED

	new id = skip
	g_Fired[id] = true
	new Float:yaw = g_WobbleYaw[id], Float:pitch = g_WobblePitch[id]

	new Float:now = get_gametime()
	if (g_Target[id] && g_FlickDur[id] > 0.0 && now >= g_FlickStart[id])
	{
		new Float:t = (now - g_FlickStart[id]) / g_FlickDur[id]
		if (t < 1.0)
		{
			new Float:left = 1.0 - min_jerk(t)
			yaw += g_OverYaw[id] * left
			pitch += g_OverPitch[id] * left
		}
	}
	new s = bot_style(id)
	new n = STYLES[s][ST_HeadPct] ? 0 : g_Burst[id] - 2
	if (n > 0)
	{
		yaw += g_SprayDrift[id] * float(n)
		pitch += 0.18 * float(n) * skill_of(id)   // recoil not fully pulled down
	}

	new Float:ang[3], Float:fwd[3], Float:newend[3]
	if (STYLES[s][ST_HeadPct] && snap_to_target(id, start, dir, s))
		pitch = yaw = 0.0
	vector_to_angle(dir, ang)
	ang[0] = -(ang[0] + pitch)             // to view-angle convention (down-positive)
	ang[1] = wrap(ang[1] + yaw)
	angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
	for (new i = 0; i < 3; i++)
		newend[i] = start[i] + fwd[i] * len
	g_Bending = true
	engfunc(EngFunc_TraceLine, start, newend, cond, skip, tr)
	g_Bending = false
	return FMRES_SUPERCEDE
}

// Hackers: a bullet heading anywhere near the target snaps to it - usually the
// head, sometimes the chest, and now and then it's left alone so it can miss
bool:snap_to_target(id, const Float:start[3], Float:dir[3], s)
{
	new t = g_Target[id]
	if (!t || !is_user_alive(t))
		return false
	new r = random(100)
	if (r >= STYLES[s][ST_HeadPct] + STYLES[s][ST_BodyPct])
		return false
	new Float:head[3], Float:want[3]
	if (r < STYLES[s][ST_HeadPct])
		eye_pos(t, head)
	else
		pev(t, pev_origin, head)            // chest height
	for (new i = 0; i < 3; i++)
		want[i] = head[i] - start[i]
	new Float:a = vector_length(dir), Float:b = vector_length(want)
	if (a < 1.0 || b < 1.0)
		return false
	new Float:dot = (dir[0] * want[0] + dir[1] * want[1] + dir[2] * want[2]) / (a * b)
	if (floatacos(floatclamp(dot, -1.0, 1.0), degrees) > (s == STYLE_GOD ? 35.0 : 15.0))
		return false
	for (new i = 0; i < 3; i++)
		dir[i] = want[i] / b * a
	return true
}

// ---------------------------------------------------------------------------
// Maths
// ---------------------------------------------------------------------------

Float:gaussian()
{
	// Box-Muller
	new Float:u = floatmax(random_float(0.0, 1.0), 0.000001)
	return floatsqroot(-2.0 * floatlog(u, 2.718281828)) * floatcos(random_float(0.0, 6.2831853), radian)
}

// mostly near mu, with a long tail of slow reactions
Float:ex_gaussian(Float:mu, Float:sigma, Float:tau)
{
	new Float:u = floatmax(random_float(0.0, 1.0), 0.000001)
	return floatmax(mu + sigma * gaussian() - tau * floatlog(u, 2.718281828), 0.02)
}

// share of a minimum-jerk movement completed at t (0..1)
Float:min_jerk(Float:t)
{
	return t * t * t * (10.0 - 15.0 * t + 6.0 * t * t)
}

Float:log2(Float:x)
{
	return floatlog(x, 2.0)
}

Float:wrap(Float:yaw)
{
	while (yaw > 180.0) yaw -= 360.0
	while (yaw < -180.0) yaw += 360.0
	return yaw
}

// ---------------------------------------------------------------------------
// Round start: favourite guns, afk moments, sprays, the caller's call
// ---------------------------------------------------------------------------

public ev_new_round_sprays()
{
	clear_sprays()
}

public ev_round_start()
{
	clear_sprays()
	g_RoundStartAt = get_gametime()
	g_BombPlanted = false
	// last round's afk and held angles are over (in a match too)
	for (new id = 1; id <= MaxClients; id++)
	{
		g_AfkUntil[id] = g_AfkAt[id] = g_HoldUntil[id] = 0.0
		g_AfkOn[id] = false
	}
	if (style() == STYLE_CLASSIC || is_match())
	{
		// a match: favourite guns only, nothing silly (no afk, sprays or radio calls here;
		// knife runs and held angles check for a match themselves)
		if (style() != STYLE_CLASSIC)
			set_task(0.4, "task_favourites")
		return
	}
	set_task(0.4, "task_favourites")
	new Float:now = get_gametime()
	new bool:called[3]
	new Float:k = pers()
	// played like a match (Matchmaking on Classic): no afk or sprays either; deathmatch: no afk
	new bool:silly = !match_like()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		new p = persona(id)
		// a mood settles between rounds
		g_Tilt[id] *= 0.85
		// a few seconds, starting a moment into the round (not on the dot), never with the
		// bomb (the round would wait on it), and not where a plugin steers the bots
		if (get_pcvar_num(g_pAfk) && silly && g_IdleMode && !dm_mode() && !user_has_weapon(id, CSW_C4) && random_float(0.0, 1.0) < (has(p, "dYLF") ? 0.015 : 0.007) * k)
		{
			g_AfkAt[id] = now + random_float(0.0, 6.0)
			g_AfkUntil[id] = g_AfkAt[id] + random_float(3.0, 10.0)
			g_AfkHp[id] = get_user_health(id)
		}
		if (get_pcvar_num(g_pSprays) && silly && (g_SprayCount || g_LogoSpray) && random_float(0.0, 1.0) < (has(p, "9") ? 0.70 : has(p, "jkhXS3") ? 0.25 : 0.10) * k)
			set_task(random_float(1.0, 8.0), "task_spray", TASK_SPRAY + get_user_userid(id))
		// the team's caller, now and then, in team modes only. Never "follow me", "stick
		// together" or "regroup": CZ bots answer those by walking over to the caller
		new team = get_user_team(id)
		if (get_pcvar_num(g_pRadio) && botplay() == PLAY_TEAM && (team == 1 || team == 2) && !called[team] && has(p, "itr") && random_float(0.0, 1.0) < 0.15 * k)
		{
			called[team] = true
			set_task(random_float(0.3, 2.0), "task_radio", TASK_RADIO + get_user_userid(id))
			g_RadioCall[id] = random(3)
		}
	}
}


public task_radio(tid)
{
	static const CALLS[][] = { "go", "stormfront", "getinpos" }
	new id = find_player("k", tid - TASK_RADIO)
	if (id && is_user_alive(id) && is_user_bot(id))
		engclient_cmd(id, CALLS[g_RadioCall[id] % sizeof CALLS])
}

// Buying like the bot always does: its favourite, when it can afford it and doesn't have
// something already (modes with normal buying only)
public task_favourites()
{
	if (!get_pcvar_num(g_pHabits) || get_cvar_float("mp_buytime") <= 0.0 || get_cvar_num("gm_weapons"))
		return
	// modes with normal buying only (not Hide and Seek, Zombie Plague, Gun Game...)
	new mode[32]
	read_mode(mode, charsmax(mode))
	if (!equal(mode, "classic") && !equal(mode, "match") && !equal(mode, "war3") && !equal(mode, "shero"))
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id) || !cs_get_user_buyzone(id))
			continue
		if (g_Fav[id] < 0)
			g_Fav[id] = fav_of(id)
		new fav = g_Fav[id], money = cs_get_user_money(id)
		switch (fav)
		{
			case CSW_DEAGLE:
			{
				if (money >= 650 && !user_has_weapon(id, CSW_DEAGLE))
				{
					drop_slot(id, 2)
					give_item(id, "weapon_deagle")
					cs_set_user_bpammo(id, CSW_DEAGLE, 35)
					cs_set_user_money(id, money - 650)
				}
			}
			case CSW_ELITE:
			{
				if (money >= 800 && !user_has_weapon(id, CSW_ELITE))
				{
					drop_slot(id, 2)
					give_item(id, "weapon_elite")
					cs_set_user_bpammo(id, CSW_ELITE, 120)
					cs_set_user_money(id, money - 800)
				}
			}
			case CSW_M3, CSW_XM1014, CSW_TMP, CSW_MAC10, CSW_P90:
			{
				new price, item[24], ammo
				switch (fav)
				{
					case CSW_M3:     { price = 1700; item = "weapon_m3"; ammo = 32; }
					case CSW_XM1014: { price = 3000; item = "weapon_xm1014"; ammo = 32; }
					case CSW_TMP:    { price = 1250; item = "weapon_tmp"; ammo = 120; }
					case CSW_MAC10:  { price = 1400; item = "weapon_mac10"; ammo = 100; }
					default:         { price = 2350; item = "weapon_p90"; ammo = 100; }
				}
				if (money >= price && !has_primary(id))
				{
					give_item(id, item)
					cs_set_user_bpammo(id, fav, ammo)
					cs_set_user_money(id, money - price)
				}
			}
			case CSW_AWP, CSW_SCOUT:
			{
				new price = fav == CSW_AWP ? 4750 : 2750
				if (money >= price && !has_primary(id))
				{
					give_item(id, fav == CSW_AWP ? "weapon_awp" : "weapon_scout")
					cs_set_user_bpammo(id, fav, fav == CSW_AWP ? 30 : 90)
					cs_set_user_money(id, money - price)
				}
			}
		}
	}
}

bool:has_primary(id)
{
	new weapons[32], n
	get_user_weapons(id, weapons, n)
	for (new i = 0; i < n; i++)
		if ((1 << weapons[i]) & PRIMARY_BITS)
			return true
	return false
}


// Takes away the pistol it holds, so a new one fits (slot 2)
drop_slot(id, slot)
{
	if (slot != 2)
		return
	new weapons[32], n, name[32]
	get_user_weapons(id, weapons, n)
	for (new i = 0; i < n; i++)
	{
		if (!((1 << weapons[i]) & PISTOL_BITS))
			continue
		get_weaponname(weapons[i], name, charsmax(name))
		new ent = -1
		while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", name)) > 0)
		{
			if (pev(ent, pev_owner) != id)
				continue
			ExecuteHamB(Ham_Weapon_RetireWeapon, ent)
			if (ExecuteHamB(Ham_RemovePlayerItem, id, ent))
			{
				ExecuteHamB(Ham_Item_Kill, ent)
				set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << weapons[i]))
			}
			break
		}
	}
}

// Every few seconds without a fight, early in the round: some put the knife out to run faster
knife_run(id, Float:now)
{
	g_NextKnifeTry[id] = now + 3.0
	// only early in the round, when nobody can be close yet (and never in a match)
	if (g_KnifeRun[id] || now - g_NoTargetSince[id] < 5.0 || now - g_RoundStartAt > KNIFE_WINDOW || is_match() || !holding_gun(id) || danger(id))
		return
	new p = persona(id)
	if (random_float(0.0, 1.0) < (has(p, "P5") ? 0.30 : has(p, "SJne") ? 0.25 : has(p, "46") ? 0.02 : 0.07) * pers())
	{
		g_KnifeRun[id] = true
		engclient_cmd(id, "weapon_knife")
	}
}

// A logo on the wall in front of it
public task_spray(tid)
{
	new id = find_player("k", tid - TASK_SPRAY)
	// (not with danger around: gunfire, someone hurt close by, an enemy seen just now)
	if (!id || !is_user_alive(id) || !is_user_bot(id) || g_Target[id] || danger(id) || (!g_SprayCount && !g_LogoSpray))
		return
	new Float:eye[3], Float:view[3], Float:fwd[3], Float:end[3]
	eye_pos(id, eye)
	pev(id, pev_v_angle, view)
	angle_vector(view, ANGLEVECTOR_FORWARD, fwd)
	for (new i = 0; i < 3; i++)
		end[i] = eye[i] + fwd[i] * 128.0
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, eye, end, IGNORE_MONSTERS, id, tr)
	new Float:frac, Float:normal[3]
	get_tr2(tr, TR_flFraction, frac)
	new hit = get_tr2(tr, TR_pHit)
	get_tr2(tr, TR_vecEndPos, end)
	get_tr2(tr, TR_vecPlaneNormal, normal)
	free_tr2(tr)
	// only on a wall close by (not on another entity)
	if (frac >= 1.0 || (hit > 0 && pev_valid(hit)))
		return
	if (g_LogoSpray)
		spray_logo(end, normal)
	else
		spray_decal(end)
	emit_sound(id, CHAN_VOICE, "player/sprayer.wav", 1.0, ATTN_NORM, 0, PITCH_NORM)
	say_line(id, "spray", 0, 0.08)
}

// The logo, flat on the wall a hair in front of it, like a player's spray. Sixteen at most
// on the map: a new one takes the place of the oldest
spray_logo(const Float:at[3], const Float:normal[3])
{
	new ent = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"))
	// (0 is the world: what it returns when the map is out of entities)
	if (ent <= MaxClients || !pev_valid(ent))
		return
	set_pev(ent, pev_classname, "ls_spray")
	engfunc(EngFunc_SetModel, ent, SPRAY_SPRITE)
	new Float:origin[3], Float:angles[3]
	for (new i = 0; i < 3; i++)
		origin[i] = at[i] + normal[i] * 0.5
	// facing into the wall, the way someone looking at it faces: the logo reads the right way
	// round (an entity's pitch is the other way up from a direction's)
	new Float:into[3]
	for (new i = 0; i < 3; i++)
		into[i] = -normal[i]
	vector_to_angle(into, angles)
	angles[0] = -angles[0]
	set_pev(ent, pev_angles, angles)
	engfunc(EngFunc_SetOrigin, ent, origin)
	set_pev(ent, pev_movetype, MOVETYPE_NONE)
	set_pev(ent, pev_solid, SOLID_NOT)
	set_pev(ent, pev_rendermode, kRenderTransAlpha)
	set_pev(ent, pev_renderamt, 255.0)
	set_pev(ent, pev_scale, random_float(0.8, 1.0))
	remove_spray(g_SprayNext)
	g_Sprays[g_SprayNext] = ent
	g_SprayNext = (g_SprayNext + 1) % MAX_SPRAYS
}

clear_sprays()
{
	for (new i = 0; i < MAX_SPRAYS; i++)
		remove_spray(i)
	g_SprayNext = 0
}

// (only if that entity is still one of ours: an edict number gets used again)
remove_spray(slot)
{
	new ent = g_Sprays[slot]
	g_Sprays[slot] = 0
	if (!ent || !pev_valid(ent))
		return
	new cls[16]
	pev(ent, pev_classname, cls, charsmax(cls))
	if (equal(cls, "ls_spray"))
		engfunc(EngFunc_RemoveEntity, ent)
}

// One of the game's own logos
spray_decal(const Float:end[3])
{
	if (!g_SprayCount)
		return
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	// the index is one byte: decals past 255 go in the "high" message
	new d = g_SprayDecals[random(g_SprayCount)]
	write_byte(d > 255 ? TE_WORLDDECALHIGH : TE_WORLDDECAL)
	engfunc(EngFunc_WriteCoord, end[0])
	engfunc(EngFunc_WriteCoord, end[1])
	engfunc(EngFunc_WriteCoord, end[2])
	write_byte(d > 255 ? d - 256 : d)
	message_end()
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

// ---------------------------------------------------------------------------
// Radio: the odd call, with a wait for each bot and each team. CZ bots answer
// "enemy spotted" and the like by walking over to whoever called, so a chatty
// radio pulls a team into one group.
// ---------------------------------------------------------------------------

radio(id, const cmd[], Float:chance, Float:botWait, Float:teamWait)
{
	if (!get_pcvar_num(g_pRadio) || is_match() || style() == STYLE_CLASSIC)
		return
	new team = get_user_team(id)
	if (team != 1 && team != 2)
		return
	new Float:now = get_gametime()
	if (now < g_NextRadio[id] || now < g_TeamRadioAt[team])
		return
	// individuals barely talk to their team
	if (botplay() != PLAY_TEAM)
		chance *= 0.5
	if (random_float(0.0, 1.0) >= chance * pers())
		return
	engclient_cmd(id, cmd)
	g_NextRadio[id] = now + botWait * random_float(1.0, 1.5)
	g_TeamRadioAt[team] = now + teamWait
}

public ev_bomb_planted()
{
	g_BombPlanted = true
}

// ---------------------------------------------------------------------------
// Holding an angle: careful players and lurkers stop now and then and wait,
// looking around (CZ keeps their view and trigger), until something shows up.
// Stopping a bot is what tested to work (like the afk moments); steering one isn't.
// ---------------------------------------------------------------------------

// What kind of player this bot is in the plugin's eyes (from its name, so a regular
// plays the same way every time; its chat personality leans it)
bot_kind(id)
{
	new r = (name_hash(id) / 13) % 100, p = persona(id)
	if (has(p, "O1hX"))
		r = r / 2                     // show-offs and cowboys push
	else if (has(p, "dYo2"))
		r = 50 + r / 2                // the tired, old-timers and patient ones sit back
	return r < 20 ? KIND_RUSHER : r < 45 ? KIND_ROAMER : r < 70 ? KIND_LURKER : KIND_CAREFUL
}

try_hold(id, Float:now)
{
	g_NextHoldTry[id] = now + 2.5
	// (not in deathmatch: nobody holds an angle for fun there)
	if (!get_pcvar_num(g_pHold) || !g_IdleMode || is_match() || dm_mode() || now - g_RoundStartAt < 4.0 || g_AfkUntil[id] > now)
		return
	if (user_has_weapon(id, CSW_C4) || !holding_gun(id) || g_BombPlanted && get_user_team(id) == 2)
		return
	// only standing somewhere solid, with nothing going on around it
	if (!steady(id, pev(id, pev_button)) || danger(id))
		return
	new kind = bot_kind(id), play = botplay()
	new Float:chance
	switch (kind)
	{
		case KIND_ROAMER: chance = 0.05
		case KIND_LURKER: chance = 0.06
		case KIND_CAREFUL: chance = 0.12
		default: return
	}
	chance *= play == PLAY_HOLD ? 1.6 : play == PLAY_SOLO ? 1.2 : 1.0
	if (random_float(0.0, 1.0) >= chance)
		return
	// not in a doorway with a teammate right behind it
	new Float:at[3], Float:o[3]
	pev(id, pev_origin, at)
	for (new m = 1; m <= MaxClients; m++)
	{
		if (m == id || !is_user_alive(m) || get_user_team(m) != get_user_team(id))
			continue
		pev(m, pev_origin, o)
		if (get_distance_f(at, o) < 96.0)
			return
	}
	new Float:len = kind == KIND_CAREFUL ? random_float(6.0, 14.0) : kind == KIND_LURKER ? random_float(4.0, 9.0) : random_float(3.0, 6.0)
	if (play == PLAY_HOLD)
		len *= 1.3
	g_HoldUntil[id] = now + len
	g_HoldDuck[id] = random_float(0.0, 1.0) < (kind == KIND_CAREFUL ? 0.35 : 0.15)
	g_HoldHp[id] = get_user_health(id)
}

// ---------------------------------------------------------------------------
// Following you: a CZ bot with teamwork over 40 that sees a human teammate close by
// may follow them, if that human is open to it (CBasePlayer::m_allowAutoFollowTime).
// Bots shadowing the host is one of the clearest bot tells, so with a style on the
// humans are kept closed, and in team modes opened now and then for a few seconds.
// Radio calls ("follow me") still work: CZ answers those another way.
// ---------------------------------------------------------------------------

public task_follow()
{
	if (style() == STYLE_CLASSIC)
		return
	new Float:now = get_gametime(), bool:team = botplay() == PLAY_TEAM
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || is_user_bot(id))
			continue
		if (team)
		{
			if (now < g_FollowOpenUntil[id])
				continue
			if (now >= g_NextFollowRoll[id])
			{
				g_NextFollowRoll[id] = now + random_float(40.0, 80.0)
				if (random_float(0.0, 1.0) < 0.35)
				{
					g_FollowOpenUntil[id] = now + random_float(8.0, 15.0)
					set_ent_data_float(id, "CBasePlayer", "m_allowAutoFollowTime", 0.0)
					continue
				}
			}
		}
		if (get_ent_data_float(id, "CBasePlayer", "m_allowAutoFollowTime") < now + 3.0)
			set_ent_data_float(id, "CBasePlayer", "m_allowAutoFollowTime", now + 5.0)
	}
}
