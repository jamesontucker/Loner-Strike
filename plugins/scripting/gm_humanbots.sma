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
	                 7 Gods (everything maxed: every bullet near you is a headshot)
	gm_hb_reaction   1 human reaction times
	gm_hb_aim        1 flicks, wobble and bursts
	gm_hb_mistakes   1 occasional mistakes
	gm_hb_bhop       1 Locked In and Hackers bunnyhop between fights
	Each style also has a bot personality file (czero/BotProfile_<Style>.db,
	made by the control panel's botstyles.py) that gamemodes.amxx switches to.
*/

#include <amxmodx>
#include <fakemeta>
#include <cstrike>
#include <hamsandwich>

#define PLUGIN  "Human bots"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define SCAN_INTERVAL 0.1
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

new g_pStyle, g_pReaction, g_pAim, g_pMistakes, g_pBhop, g_pFreeForAll
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

	register_forward(FM_CmdStart, "fw_cmdstart")
	register_forward(FM_PlayerPreThink, "fw_prethink")
	register_forward(FM_PlayerPostThink, "fw_postthink")
	register_forward(FM_PlayerPostThink, "fw_postthink_post", 1)
	register_forward(FM_TraceLine, "fw_traceline")
	set_task(SCAN_INTERVAL, "task_scan", _, _, _, "b")
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
	return (STYLE_CLASSIC < s < STYLE_COUNT) ? s : STYLE_CLASSIC
}

public client_putinserver(id)
{
	reset_bot(id)
	g_SkillStyle[id] = STYLE_CLASSIC
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
	new s = style()
	if (s == STYLE_CLASSIC)
		return
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue

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
			if (angle < bestAngle && can_see(id, eye, e))
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
			}
			continue
		}
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

	new Float:skill = g_Skill[id]
	new Float:rt = ex_gaussian(STYLES[s][ST_RtMu], STYLES[s][ST_RtSigma], STYLES[s][ST_RtTau]) * skill
	rt *= (1.0 + dist / 3000.0) * (1.0 + angle / 90.0)
	new Float:vel[3]
	pev(target, pev_velocity, vel)
	if (vector_length(vel) > 120.0)
		rt *= 0.85                          // movement catches the eye
	if (recent)
		rt *= 0.45                          // knew they were there

	new Float:over = (angle * STYLES[s][ST_Overshoot] * random_float(0.5, 1.5) + 0.6) * skill
	if (recent)
		over *= 0.4

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
	g_BurstLen[id] = max(len, 1)
	g_SprayDrift[id] = random_float(-0.25, 0.25) * g_Skill[id]
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
	new s = style()
	if (s == STYLE_CLASSIC)
		return FMRES_IGNORED

	new Float:now = get_gametime()
	new Float:dt = floatclamp(float(get_uc(uc, UC_Msec)) / 1000.0, 0.001, 0.1)
	update_wobble(id, s, dt, now)

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
	new Float:sigma = STYLES[s][ST_Wobble] * g_Skill[id] * (1.0 + g_TargetAngVel[id] / 40.0 + own / 260.0)
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
	new s = style()
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
	new s = style()
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
	new s = style()
	new n = STYLES[s][ST_HeadPct] ? 0 : g_Burst[id] - 2
	if (n > 0)
	{
		yaw += g_SprayDrift[id] * float(n)
		pitch += 0.18 * float(n) * g_Skill[id]   // recoil not fully pulled down
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
