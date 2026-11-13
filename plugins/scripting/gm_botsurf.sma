/*
	gm_botsurf - the basics of surfing for bots (Surf mode only)

	A surf ramp is a slope steeper than the game lets anyone stand on (its surface normal's
	up part under 0.7): the game treats a player on it as in the air, so there's no friction,
	and holding a strafe into the ramp keeps them on it while gravity turns into speed along it.

	Each frame, for a bot touching a ramp, this sets its movement command the way a player
	surfs: facing along the ramp the way it's going, no forward key, the strafe key that pushes
	into the ramp (D on the left side of a ramp, A on the right). In the air at speed it faces
	along its velocity and lets go of the keys, which keeps the speed. On flat ground the bot's
	own brain has it.

	Not a route: bots ride the ramps they reach; finishing a whole course would need each
	map's ramps worked out ahead of time.

	Cvars: gm_botsurf 1 (on), gm_botsurf_log 0 (1: a line every 30 s, ramp time and top speed)
*/

#include <amxmodx>
#include <fakemeta>

#define PLUGIN  "GM Bot Surf"
#define VERSION "1.0"
#define AUTHOR  "Loner-Strike"

// a surface a player can't stand on but can ride: steeper than walkable, not a wall
#define RAMP_MIN_UP 0.05
#define RAMP_MAX_UP 0.7
#define PROBE       20.0

new g_pOn, g_pLog, g_pDebug, g_pBrainSurf
new Float:g_DebugAt
new Float:g_RampTime[33], Float:g_TopSpeed[33], Float:g_Last[33]
new Float:g_RampSum, Float:g_TopAll
new Float:g_StillSince[33], Float:g_ExitYaw[33], Float:g_ExitUntil[33]

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pOn = register_cvar("gm_botsurf", "1")
	g_pLog = register_cvar("gm_botsurf_log", "0")
	g_pDebug = register_cvar("gm_botsurf_debug", "0")
	register_forward(FM_CmdStart, "fw_CmdStart")
	// (the bot brain steers surf itself when it has bot_ls_surf on: this stands aside)
	g_pBrainSurf = get_cvar_pointer("bot_ls_surf")
	set_task(30.0, "report", 0, _, _, "b")
}

public client_putinserver(id)
{
	g_RampTime[id] = g_TopSpeed[id] = g_Last[id] = 0.0
	g_StillSince[id] = g_ExitUntil[id] = 0.0
}

// How far the ramp under/beside "at" goes on along (rx, ry): steps along it looking down for
// the same kind of surface (a ramp leaning the same way), up to 1024 units
Float:ramp_run(id, const Float:at[3], Float:rx, Float:ry, const Float:n[3])
{
	new tr = create_tr2()
	new Float:p[3], Float:q[3], Float:f, Float:pn[3], Float:run = 0.0
	for (new k = 1; k <= 16; k++)
	{
		p[0] = at[0] + rx * k * 64.0; p[1] = at[1] + ry * k * 64.0; p[2] = at[2] + 160.0
		q[0] = p[0]; q[1] = p[1]; q[2] = at[2] - 320.0
		engfunc(EngFunc_TraceLine, p, q, IGNORE_MONSTERS, id, tr)
		get_tr2(tr, TR_flFraction, f)
		if (f >= 1.0)
			break
		get_tr2(tr, TR_vecPlaneNormal, pn)
		if (!(pn[2] > RAMP_MIN_UP && pn[2] < RAMP_MAX_UP) || pn[0] * n[0] + pn[1] * n[1] < 0.0)
			break
		run = k * 64.0
	}
	free_tr2(tr)
	return run
}

// Height of the ramp 256 units along (rx, ry) from "at" (the same ramp's surface), or 99999 if
// it doesn't go on that way
Float:ramp_height(id, const Float:at[3], Float:rx, Float:ry)
{
	new tr = create_tr2()
	new Float:p[3], Float:q[3], Float:f, Float:pn[3], Float:h = 99999.0
	p[0] = at[0] + rx * 256.0; p[1] = at[1] + ry * 256.0; p[2] = at[2] + 300.0
	q[0] = p[0]; q[1] = p[1]; q[2] = at[2] - 600.0
	engfunc(EngFunc_TraceLine, p, q, IGNORE_MONSTERS, id, tr)
	get_tr2(tr, TR_flFraction, f)
	if (f < 1.0)
	{
		get_tr2(tr, TR_vecPlaneNormal, pn)
		if (pn[2] > RAMP_MIN_UP && pn[2] < RAMP_MAX_UP)
		{
			get_tr2(tr, TR_vecEndPos, q)
			h = q[2]
		}
	}
	free_tr2(tr)
	return h
}

// The way along a ramp to ride: downhill if it slopes along its length (that's where the speed
// comes from), else the way it goes on longest
Float:ride_way(id, const Float:at[3], Float:rx, Float:ry, const Float:n[3])
{
	new Float:h1 = ramp_height(id, at, rx, ry), Float:h2 = ramp_height(id, at, -rx, -ry)
	if (h1 < 99999.0 && h2 < 99999.0 && floatabs(h1 - h2) > 16.0)
		return h1 < h2 ? 1.0 : -1.0
	return ramp_run(id, at, rx, ry, n) >= ramp_run(id, at, -rx, -ry, n) ? 1.0 : -1.0
}

// Standing on a platform: the nearest way off it onto a ramp. Walks out in 16 directions on
// the flat (as far as 600 units), and at each step looks straight down for a ramp below the
// edge. The yaw of the closest one in yaw; false if there's none
bool:find_exit(id, const Float:o[3], &Float:yaw)
{
	new hull = (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN
	new tr = create_tr2()
	new Float:bestDist = 99999.0, bool:found = false
	new Float:a, Float:dx, Float:dy, Float:f, Float:pn[3], Float:from[3], Float:to[3], Float:at[3]
	for (new i = 0; i < 16; i++)
	{
		a = i * 22.5
		dx = floatcos(a, degrees); dy = floatsin(a, degrees)
		for (new step = 1; step <= 12; step++)
		{
			new Float:d = step * 50.0
			from[0] = o[0]; from[1] = o[1]; from[2] = o[2]
			to[0] = o[0] + dx * d; to[1] = o[1] + dy * d; to[2] = o[2]
			// blocked by a wall this way: no further
			engfunc(EngFunc_TraceHull, from, to, IGNORE_MONSTERS, hull, id, tr)
			get_tr2(tr, TR_flFraction, f)
			if (f < 1.0)
				break
			// the floor under that point: still the platform, or the drop (a ramp below?)
			at[0] = to[0]; at[1] = to[1]; at[2] = to[2] - 2000.0
			engfunc(EngFunc_TraceHull, to, at, IGNORE_MONSTERS, hull, id, tr)
			get_tr2(tr, TR_flFraction, f)
			if (f >= 1.0)
				break
			get_tr2(tr, TR_vecPlaneNormal, pn)
			if (pn[2] >= RAMP_MAX_UP && f * 2000.0 < 40.0)
				continue		// flat floor right under: still on the platform
			if (pn[2] > RAMP_MIN_UP && pn[2] < RAMP_MAX_UP && d < bestDist)
			{
				bestDist = d
				found = true
				// land moving along it, not straight down it: walk off at an angle, toward the
				// way the ramp runs on longest
				new Float:hit[3]
				get_tr2(tr, TR_vecEndPos, hit)
				new Float:hl = floatsqroot(pn[0] * pn[0] + pn[1] * pn[1])
				if (hl > 0.01)
				{
					new Float:rx = -pn[1] / hl, Float:ry = pn[0] / hl
					if (ride_way(id, hit, rx, ry, pn) < 0.0)
					{
						rx = -rx; ry = -ry
					}
					yaw = floatatan2(dy * 0.45 + ry * 0.9, dx * 0.45 + rx * 0.9, degrees)
				}
				else
					yaw = a
			}
			break
		}
	}
	free_tr2(tr)
	return found
}

public report()
{
	if (!get_pcvar_num(g_pLog))
		return
	new Float:now = 0.0
	for (new i = 1; i <= MaxClients; i++)
		now += g_RampTime[i]
	g_RampSum = now
	for (new i = 1; i <= MaxClients; i++)
		if (g_TopSpeed[i] > g_TopAll)
			g_TopAll = g_TopSpeed[i]
	log_amx("BOTSURF ramp-seconds %.0f, top speed %.0f", g_RampSum, g_TopAll)
}

// The nearest ramp touching the bot: below it or to a side. Its normal in n
bool:find_ramp(id, const Float:o[3], Float:n[3])
{
	static const Float:DIRS[][3] = {
		{ 0.0, 0.0, -1.0 },
		{ 1.0, 0.0, -0.5 }, { -1.0, 0.0, -0.5 }, { 0.0, 1.0, -0.5 }, { 0.0, -1.0, -0.5 },
		{ 0.7071, 0.7071, -0.5 }, { 0.7071, -0.7071, -0.5 }, { -0.7071, 0.7071, -0.5 }, { -0.7071, -0.7071, -0.5 }
	}
	new hull = (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN
	new tr = create_tr2()
	new Float:best = 2.0, bool:found = false
	new Float:end[3], Float:f, Float:pn[3]
	for (new i = 0; i < sizeof DIRS; i++)
	{
		end[0] = o[0] + DIRS[i][0] * PROBE
		end[1] = o[1] + DIRS[i][1] * PROBE
		end[2] = o[2] + DIRS[i][2] * PROBE
		engfunc(EngFunc_TraceHull, o, end, IGNORE_MONSTERS, hull, id, tr)
		if (get_tr2(tr, TR_StartSolid) || get_tr2(tr, TR_AllSolid))
			continue
		get_tr2(tr, TR_flFraction, f)
		if (f >= 1.0 || f >= best)
			continue
		get_tr2(tr, TR_vecPlaneNormal, pn)
		if (pn[2] > RAMP_MIN_UP && pn[2] < RAMP_MAX_UP)
		{
			best = f
			n[0] = pn[0]; n[1] = pn[1]; n[2] = pn[2]
			found = true
		}
	}
	free_tr2(tr)
	return found
}

public fw_CmdStart(id, uc, seed)
{
	if (!get_pcvar_num(g_pOn) || !is_user_bot(id) || (g_pBrainSurf && get_pcvar_num(g_pBrainSurf) > 0))
		return FMRES_IGNORED
	// (dead: the last life's timers and walk-off are forgotten, not carried into the next)
	if (!is_user_alive(id))
	{
		g_StillSince[id] = g_ExitUntil[id] = g_Last[id] = 0.0
		return FMRES_IGNORED
	}

	new Float:o[3], Float:v[3]
	pev(id, pev_origin, o)
	pev(id, pev_velocity, v)
	new Float:speed = floatsqroot(v[0] * v[0] + v[1] * v[1])
	if (speed > g_TopSpeed[id])
		g_TopSpeed[id] = speed
	new Float:now = get_gametime()
	new Float:dt = now - g_Last[id]
	g_Last[id] = now
	if (dt < 0.0 || dt > 0.25)
		dt = 0.0

	// (a player on a ramp is never on the ground: no ramp traces for bots standing about)
	new flags = pev(id, pev_flags)
	new Float:n[3]
	if (!(flags & FL_ONGROUND) && find_ramp(id, o, n))
	{
		// the ramp's outward side, flat, and the two ways along it
		new Float:hl = floatsqroot(n[0] * n[0] + n[1] * n[1])
		if (hl < 0.01)
			return FMRES_IGNORED
		new Float:nx = n[0] / hl, Float:ny = n[1] / hl
		new Float:rx = -ny, Float:ry = nx
		// along the ramp the way it's already going (standing still: the way it faces)
		new Float:along = v[0] * rx + v[1] * ry
		// slow along it: downhill if the ramp slopes along its length (the speed's there), else
		// the way it goes on longest
		if (floatabs(along) < 150.0)
			along = ride_way(id, o, rx, ry, n)
		if (along < 0.0)
		{
			rx = -rx; ry = -ry
		}
		// facing along the ramp, tilted 20 degrees down it (toward its outside): it slides down
		// as it goes and turns the drop into speed, instead of hanging level on the ramp (tuned
		// in a simulation of the game's movement: about twice the speed and ground of facing
		// straight along it)
		new Float:ly = floatatan2(ry, rx, degrees), Float:oy = floatatan2(ny, nx, degrees)
		new Float:dd = oy - ly
		while (dd > 180.0) dd -= 360.0
		while (dd < -180.0) dd += 360.0
		new Float:yaw = ly + (dd > 0.0 ? floatmin(20.0, dd) : -floatmin(20.0, -dd))
		if (yaw > 180.0) yaw -= 360.0
		else if (yaw < -180.0) yaw += 360.0
		// strafe toward the ramp: right is (sin yaw, -cos yaw); into the ramp is -n
		new Float:rightInto = floatsin(yaw, degrees) * -nx + -floatcos(yaw, degrees) * -ny
		new Float:side = rightInto > 0.0 ? 400.0 : -400.0
		new Float:va[3]
		va[0] = 0.0; va[1] = yaw; va[2] = 0.0
		set_uc(uc, UC_ViewAngles, va)
		set_uc(uc, UC_ForwardMove, 0.0)
		set_uc(uc, UC_SideMove, side)
		set_uc(uc, UC_UpMove, 0.0)
		set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) & ~(IN_FORWARD | IN_BACK | IN_JUMP | IN_DUCK | IN_MOVELEFT | IN_MOVERIGHT))
		set_pev(id, pev_v_angle, va)
		set_pev(id, pev_angles, va)
		g_RampTime[id] += dt
		if (get_pcvar_num(g_pDebug) && now >= g_DebugAt)
		{
			g_DebugAt = now + 0.25
			new Float:alongV = v[0] * rx + v[1] * ry, Float:outV = v[0] * nx + v[1] * ny
			log_amx("SURFDBG id %d n %.2f %.2f %.2f along %.0f out %.0f vz %.0f side %.0f yaw %.0f airaccel %.0f buttons %d", id, n[0], n[1], n[2], alongV, outV, v[2], side, yaw, get_cvar_float("sv_airaccelerate"), get_uc(uc, UC_Buttons))
		}
		return FMRES_HANDLED
	}

	// standing about on a platform: walk off it toward the nearest ramp below
	if (flags & FL_ONGROUND)
	{
		if (now < g_ExitUntil[id])
		{
			new Float:va[3]
			va[0] = 0.0; va[1] = g_ExitYaw[id]; va[2] = 0.0
			set_uc(uc, UC_ViewAngles, va)
			set_uc(uc, UC_ForwardMove, 250.0)
			set_uc(uc, UC_SideMove, 0.0)
			set_uc(uc, UC_Buttons, (get_uc(uc, UC_Buttons) | IN_FORWARD) & ~(IN_BACK | IN_JUMP | IN_DUCK))
			set_pev(id, pev_v_angle, va)
			set_pev(id, pev_angles, va)
			return FMRES_HANDLED
		}
		if (speed > 60.0)
			g_StillSince[id] = now
		else if (g_StillSince[id] == 0.0)
			g_StillSince[id] = now + random_float(0.0, 1.0)	// (bots spread out, not all in one frame)
		else if (now - g_StillSince[id] > 2.0)
		{
			// (the search is a few hundred traces: found nothing, it waits a good while to look again)
			g_StillSince[id] = now
			new Float:yaw
			if (find_exit(id, o, yaw))
			{
				g_ExitYaw[id] = yaw
				g_ExitUntil[id] = now + 3.0
			}
			else
				g_StillSince[id] = now + 5.0 + random_float(0.0, 3.0)
		}
		return FMRES_IGNORED
	}
	g_ExitUntil[id] = 0.0
	g_StillSince[id] = 0.0

	// in the air at speed: face along the velocity and let go (no friction up here)
	if (!(flags & FL_ONGROUND) && speed > 300.0)
	{
		new Float:va[3]
		va[0] = 0.0; va[1] = floatatan2(v[1], v[0], degrees); va[2] = 0.0
		set_uc(uc, UC_ViewAngles, va)
		set_uc(uc, UC_ForwardMove, 0.0)
		set_uc(uc, UC_SideMove, 0.0)
		set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) & ~(IN_FORWARD | IN_BACK | IN_JUMP | IN_DUCK | IN_MOVELEFT | IN_MOVERIGHT))
		set_pev(id, pev_v_angle, va)
		set_pev(id, pev_angles, va)
		return FMRES_HANDLED
	}
	return FMRES_IGNORED
}
