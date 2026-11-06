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
	                   nervous reloads, running with the knife out, and a rival (the
	                   player who keeps killing it) it reacts to faster
	gm_hb_afk        1 now and then a bot stands still for a few seconds at round start
	gm_hb_sprays     1 bots spray a logo now and then
	gm_hb_radio      1 radio calls that fit the moment and the bot (enemy spotted, need
	                   backup, enemy down; the team's caller says go, follow me)
	gm_hb_showoff    1 now and then after a kill, when nobody's shooting at it: crouch
	                   spam over the body, a jump, or the knife out for a moment
	gm_hb_reaction   1 human reaction times
	gm_hb_aim        1 flicks, wobble and bursts
	gm_hb_mistakes   1 occasional mistakes
	gm_hb_bhop       1 Locked In and Hackers bunnyhop between fights
	Each style also has a bot personality file (czero/BotProfile_<Style>.db,
	made by the app's setup) that gamemodes.amxx switches to.
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

#define PLUGIN  "Human bots"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define SCAN_INTERVAL 0.1
#define STYLE_MIXED   8
#define OFFSET_ACTIVE_ITEM 373

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
new g_pHabits, g_pAfk, g_pSprays, g_pRadio
new bool:g_Chat                                 // gm_botchat is running
new Float:g_JoinedAt[33], Float:g_ReloadAt[33], Float:g_AfkUntil[33], Float:g_NoTargetSince[33], Float:g_NextKnifeTry[33]
new bool:g_KnifeRun[33], g_Fav[33], g_Rival[33], g_DeathsBy[33][33]
new g_SprayDecals[8], g_SprayCount, g_RadioCall[33], Float:g_Habit[33] = { 1.0, ... }
#define PRIMARY_BITS ((1<<CSW_SCOUT)|(1<<CSW_XM1014)|(1<<CSW_MAC10)|(1<<CSW_AUG)|(1<<CSW_UMP45)|(1<<CSW_SG550)|(1<<CSW_GALIL)|(1<<CSW_FAMAS)|(1<<CSW_AWP)|(1<<CSW_MP5NAVY)|(1<<CSW_M249)|(1<<CSW_M3)|(1<<CSW_M4A1)|(1<<CSW_TMP)|(1<<CSW_G3SG1)|(1<<CSW_SG552)|(1<<CSW_AK47)|(1<<CSW_P90))
#define PISTOL_BITS  ((1<<CSW_P228)|(1<<CSW_ELITE)|(1<<CSW_FIVESEVEN)|(1<<CSW_USP)|(1<<CSW_GLOCK18)|(1<<CSW_DEAGLE))

new g_HopsLeft[33], Float:g_NextHopTry[33]
new bool:g_Career

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
	g_pRadio     = register_cvar("gm_hb_radio", "1")
	register_logevent("ev_round_start", 2, "1=Round_Start")
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

	register_forward(FM_CmdStart, "fw_cmdstart")
	register_forward(FM_PlayerPreThink, "fw_prethink")
	register_forward(FM_PlayerPostThink, "fw_postthink")
	register_forward(FM_PlayerPostThink, "fw_postthink_post", 1)
	register_forward(FM_TraceLine, "fw_traceline")
	set_task(SCAN_INTERVAL, "task_scan", _, _, _, "b")
}

public plugin_natives()
{
	set_native_filter("native_filter")
}

public native_filter(const name[], index, trap)
{
	return (!trap && equal(name, "botchat_", 8)) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
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

// How a personality changes a bot's skill (higher = slower and sloppier)
Float:habit_skill(id)
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
	if (has(p, "o1") && r < 50)
		return CSW_DEAGLE
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
	return g_Skill[id] * g_Habit[id] * (1.0 + (get_pcvar_num(g_pTilt) ? g_Tilt[id] * g_TiltWay[id] : 0.0))
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2)
	if (victim >= 1 && victim <= MaxClients)
		g_AfkUntil[victim] = 0.0
	if (victim >= 1 && victim <= MaxClients && killer != victim)
	{
		g_Tilt[victim] = floatmin(g_Tilt[victim] + 0.08, 0.3)
		// the player who keeps killing this bot becomes its rival
		if (killer >= 1 && killer <= MaxClients && ++g_DeathsBy[victim][killer] >= 3)
			g_Rival[victim] = killer
	}
	if (killer >= 1 && killer <= MaxClients && killer != victim && is_user_bot(killer) && get_pcvar_num(g_pRadio) && !get_cvar_num("gm_scrim_phase"))
	{
		new p = persona(killer)
		if (random(100) < (has(p, "r") ? 35 : has(p, "it") ? 15 : 6))
			engclient_cmd(killer, "enemydown")
	}
	if (killer < 1 || killer > MaxClients || killer == victim || !is_user_bot(killer))
		return
	g_Tilt[killer] = floatmax(g_Tilt[killer] - 0.08, -0.1)
	// a show-off moment, now and then (not in a match)
	if (!get_pcvar_num(g_pShowoff) || get_cvar_num("gm_scrim_phase") || style() == STYLE_CLASSIC)
		return
	// mostly the jokers and show-offs
	new p = persona(killer)
	new chance = has(p, "jkhOmX") ? 4 : p ? 1 : 2
	if (random(100) >= chance)
		return
	new Float:now = get_gametime()
	switch (random(4))
	{
		case 0, 1: { g_Show[killer] = 1; g_ShowUntil[killer] = now + random_float(1.2, 2.2); }
		case 2: { g_Show[killer] = 2; g_ShowUntil[killer] = now + 0.3; }
		default:
		{
			engclient_cmd(killer, "weapon_knife")
			set_task(random_float(1.5, 3.0), "task_lastinv", killer)
		}
	}
}

public task_lastinv(id)
{
	if (is_user_alive(id) && is_user_bot(id) && !g_Target[id])
		engclient_cmd(id, "lastinv")
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
}

// each bot gets its own skill within the style (higher = slower and sloppier)
sample_skill(id, s)
{
	g_SkillStyle[id] = s
	// Pub lobbies are lopsided: a few sharp players, a crowd of average ones, some bad
	if (s == STYLE_PUB)
	{
		new r = random(10)
		g_Skill[id] = r < 2 ? random_float(0.55, 0.75) : r < 7 ? random_float(0.85, 1.15) : random_float(1.3, 1.8)
	}
	else
		g_Skill[id] = random_float(STYLES[s][ST_SkillLo], STYLES[s][ST_SkillHi])
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
	g_KnifeRun[id] = false
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

		if (!best)
		{
			if (g_Target[id])
			{
				g_LastTarget[id] = g_Target[id]
				g_LostAt[id] = now
				g_Target[id] = 0
				g_NoTargetSince[id] = now
				// a nervous reload after a fight (sometimes right as someone peeks)
				if (get_pcvar_num(g_pHabits) && random(100) < (has(persona(id), "nkuH") ? 35 : 12))
					g_ReloadAt[id] = now + random_float(0.2, 0.9)
			}
			else if (get_pcvar_num(g_pHabits) && now >= g_NextKnifeTry[id])
				knife_run(id, now)
			continue
		}
		// several enemies at once, or nearly dead: some panic for a moment
		if (get_pcvar_num(g_pTilt) && get_pcvar_num(g_pMistakes) && g_PanicUntil[id] < now
			&& (seen >= 2 || get_user_health(id) < 30) && random_float(0.0, 1.0) < 0.006 * panic_factor(s))
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
			rt *= 0.85                      // been waiting for this one
		if (g_KnifeRun[id])
		{
			// running with the knife out: the gun comes back first
			g_KnifeRun[id] = false
			engclient_cmd(id, "lastinv")
			rt += 0.35
		}
	}
	if (get_pcvar_num(g_pRadio) && !recent && !get_cvar_num("gm_scrim_phase"))
	{
		new p = persona(id)
		if (random(100) < (has(p, "itr") ? 20 : 6))
			engclient_cmd(id, "enemyspot")
	}

	new Float:over = (angle * STYLES[s][ST_Overshoot] * random_float(0.5, 1.5) + 0.6) * skill
	if (recent)
		over *= 0.4
	// surprised from behind: the first spray goes wide
	if (get_pcvar_num(g_pHabits) && angle > 100.0 && random(100) < 50)
	{
		over *= 1.8
		rt *= 1.2
	}

	if (get_pcvar_num(g_pMistakes) && random_float(0.0, 1.0) < STYLES[s][ST_Mistakes] * skill)
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

	// standing still for a moment (afk): no shooting either
	if (g_AfkUntil[id] > 0.0)
	{
		if (now < g_AfkUntil[id] && !g_Target[id])
		{
			set_uc(uc, UC_Buttons, 0)
			set_uc(uc, UC_ForwardMove, 0.0)
			set_uc(uc, UC_SideMove, 0.0)
			return FMRES_IGNORED
		}
		g_AfkUntil[id] = 0.0
		say_line(id, "afk_back", 0, 0.5)
	}
	if (g_ReloadAt[id] > 0.0 && now >= g_ReloadAt[id])
	{
		g_ReloadAt[id] = 0.0
		set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) | IN_RELOAD)
	}
	// a show-off moment, only while nobody's in its sights
	if (g_Show[id])
	{
		if (now >= g_ShowUntil[id] || g_Target[id])
			g_Show[id] = 0
		else
		{
			new b = get_uc(uc, UC_Buttons) & ~IN_ATTACK
			if (g_Show[id] == 1)
				b = (floatround(now * 7.0) % 2) ? (b | IN_DUCK) : (b & ~IN_DUCK)
			else
				b |= IN_JUMP
			set_uc(uc, UC_Buttons, b)
			return FMRES_IGNORED
		}
	}
	// panicking players jump about
	if (g_PanicUntil[id] > now && (s == STYLE_PUB || s == STYLE_NEWBIE) && random(100) < 2)
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
	if (g_AfkUntil[id] > get_gametime() && !g_Target[id])
	{
		new Float:v[3]
		pev(id, pev_velocity, v)
		v[0] = v[1] = 0.0
		set_pev(id, pev_velocity, v)
		return
	}
	new s = bot_style(id)
	// chance per check (every 0.5s of straight running), and hops per chain
	new Float:chance, Float:gain, Float:cap, hopsLo, hopsHi
	switch (s)
	{
		case STYLE_LOCKED: { chance = 0.12; gain = 1.06; cap = 320.0; hopsLo = 2; hopsHi = 5; }
		case STYLE_HACKER: { chance = 0.7;  gain = 1.10; cap = 400.0; hopsLo = 4; hopsHi = 10; }
		case STYLE_GOD:    { chance = 0.95; gain = 1.15; cap = 500.0; hopsLo = 6; hopsHi = 15; }
		default:
		{
			g_HopsLeft[id] = 0
			return
		}
	}
	if (!get_pcvar_num(g_pBhop))
		return
	if (g_HopsLeft[id])
		set_pev(id, pev_fuser2, 0.0)     // no slowdown after landing
	if (!(pev(id, pev_flags) & FL_ONGROUND))
		return

	new Float:now = get_gametime()
	if (!g_HopsLeft[id])
	{
		if (now < g_NextHopTry[id])
			return
		g_NextHopTry[id] = now + 0.5
		if (random_float(0.0, 1.0) >= chance || !hop_clear(id))
			return
		g_HopsLeft[id] = random_num(hopsLo, hopsHi)
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

public ev_round_start()
{
	if (style() == STYLE_CLASSIC || get_cvar_num("gm_scrim_phase") > 0 && get_cvar_num("gm_scrim_phase") < 5)
	{
		// a match: only the guns, the rest would look silly
		if (style() != STYLE_CLASSIC)
			set_task(0.4, "task_favourites")
		return
	}
	set_task(0.4, "task_favourites")
	new Float:now = get_gametime()
	new bool:called[3]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		new p = persona(id)
		// a mood settles between rounds; last round's afk is over
		g_Tilt[id] *= 0.85
		g_AfkUntil[id] = 0.0
		if (get_pcvar_num(g_pAfk) && random(1000) < (has(p, "dYLF") ? 15 : 7))
			g_AfkUntil[id] = now + random_float(20.0, 60.0)
		if (get_pcvar_num(g_pSprays) && g_SprayCount && random(100) < (has(p, "jkhXS3") ? 25 : 10))
			set_task(random_float(1.0, 8.0), "task_spray", get_user_userid(id))
		new team = get_user_team(id)
		if (get_pcvar_num(g_pRadio) && (team == 1 || team == 2) && !called[team] && has(p, "itr") && random(100) < 35)
		{
			called[team] = true
			set_task(random_float(0.3, 2.0), "task_radio", get_user_userid(id))
			g_RadioCall[id] = random(6)
		}
	}
}


public task_radio(userid)
{
	static const CALLS[][] = { "go", "followme", "stormfront", "getinpos", "sticktog", "regroup" }
	new id = find_player("k", userid)
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
	new mode[32], path[128]
	get_datadir(path, charsmax(path))
	new f = fopen(fmt("%s/gamemodes/current.txt", path), "rt")
	if (f)
	{
		fgets(f, mode, charsmax(mode))
		trim(mode)
		fclose(f)
	}
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

// Every few seconds without a fight: some put the knife out to run faster
knife_run(id, Float:now)
{
	g_NextKnifeTry[id] = now + 3.0
	if (g_KnifeRun[id] || now - g_NoTargetSince[id] < 5.0 || !holding_gun(id))
		return
	new p = persona(id)
	if (random(100) < (has(p, "P") ? 70 : has(p, "SJn") ? 25 : 7))
	{
		g_KnifeRun[id] = true
		engclient_cmd(id, "weapon_knife")
	}
}

// A logo on the wall in front of it
public task_spray(userid)
{
	new id = find_player("k", userid)
	if (!id || !is_user_alive(id) || !is_user_bot(id) || g_Target[id] || !g_SprayCount)
		return
	new Float:eye[3], Float:view[3], Float:fwd[3], Float:end[3]
	eye_pos(id, eye)
	pev(id, pev_v_angle, view)
	angle_vector(view, ANGLEVECTOR_FORWARD, fwd)
	for (new i = 0; i < 3; i++)
		end[i] = eye[i] + fwd[i] * 128.0
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, eye, end, IGNORE_MONSTERS, id, tr)
	new Float:frac
	get_tr2(tr, TR_flFraction, frac)
	new hit = get_tr2(tr, TR_pHit)
	get_tr2(tr, TR_vecEndPos, end)
	free_tr2(tr)
	// only on a wall close by (not on another entity)
	if (frac >= 1.0 || (hit > 0 && pev_valid(hit)))
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
	emit_sound(id, CHAN_VOICE, "player/sprayer.wav", 1.0, ATTN_NORM, 0, PITCH_NORM)
	say_line(id, "spray", 0, 0.08)
}
