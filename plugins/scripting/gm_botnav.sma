/*
	gm_botnav - an autopilot for Condition Zero's bots (ZBots)

	CZ bots only go where their own brain wants. Game modes the brain does not
	understand (Hide and Seek, Deathrun...) use this to send a bot somewhere:

	  - reads the map's bot navigation mesh (maps/<map>.nav, the file CZ makes
	    when bots learn a map): walkable areas, how they connect, hiding spots
	  - finds a route with A* over the areas
	  - drives the bot along it by setting its velocity after each frame of
	    movement (what tested to work: the brain's own move input and view
	    changes lose to it), jumps up ledges and onto jump areas, crouches in
	    crouch areas, turns the view a few degrees per frame toward the way it
	    runs, and gets itself unstuck (jump, sidestep, new route, give up)
	  - hands the bot back to its brain while an enemy is close and in sight,
	    if the caller asked for that (botnav_set_fight)

	Natives (declare them in the plugin that uses them):
	  botnav_has_mesh()                          1 when the map has a mesh
	  botnav_path_to(id, Float:target[3], flags) send a bot there (BN_PERSIST = 1: never give up)
	  botnav_stop(id)                            hand the bot back to its brain
	  botnav_hold(id, duck)                      keep the bot still (crouched)
	  botnav_status(id)                          0 idle 1 routing 2 moving 3 arrived 4 failed 5 holding
	  botnav_set_fight(id, Float:radius)         let it fight enemies seen within radius (0 = never)
	  botnav_hiding_spot_count()
	  botnav_hiding_spot(index, Float:pos[3])    floor position, returns flags (1 cover, 2/4 sniper)
	  botnav_spot_area(index)
	  botnav_area_count()
	  botnav_area(index, Float:center[3])        returns area flags | (number of exits << 8)
	  botnav_area_at(Float:pos[3])               area under a point (-1 none)
	  botnav_flood(slot, Float:origin[3])        walking distance from origin to every area (slots 0, 1)
	  Float:botnav_flood_dist(slot, area)        -1.0 when unreachable
	  botnav_farthest(slot, Float:out[3])        area farthest from the flood origin

	Server command: botnav_info - mesh size and what each bot is doing.
*/

#include <amxmodx>
#include <fakemeta>

#pragma dynamic 32768

#define PLUGIN  "GM Bot Nav"
#define VERSION "1.0"
#define AUTHOR  "Loner-Strike"

#define MAX_AREAS   6144
#define MAX_CONNS   49152
#define MAX_SPOTS   4096
#define MAX_IDMAP   32768
#define MAX_HEAP    32768
#define MAX_PATH    192
#define MAX_EXPAND  9000         // A* node expansions per route
#define FLOOD_SLOTS 2
#define ROUTES_PER_TICK 3        // A* searches per tick, so a whole team asking at once doesn't hitch

#define NAV_CROUCH  0x01
#define NAV_JUMP    0x02

#define BN_IDLE    0
#define BN_PENDING 1
#define BN_MOVING  2
#define BN_ARRIVED 3
#define BN_FAILED  4
#define BN_HOLD    5

#define BN_PERSIST 1

#define TICK       0.1
#define TASK_TICK  48120
#define STEP_HEIGHT 18.0

// ---- mesh ----
new bool:g_Loaded
new g_Areas, g_Conns, g_Spots
new g_AId[MAX_AREAS], g_AFlags[MAX_AREAS], g_AFirst[MAX_AREAS + 1]
new Float:g_AX0[MAX_AREAS], Float:g_AY0[MAX_AREAS], Float:g_AZ0[MAX_AREAS]
new Float:g_AX1[MAX_AREAS], Float:g_AY1[MAX_AREAS], Float:g_AZ1[MAX_AREAS]
new Float:g_ANeZ[MAX_AREAS], Float:g_ASwZ[MAX_AREAS]
new Float:g_ACx[MAX_AREAS], Float:g_ACy[MAX_AREAS], Float:g_ACz[MAX_AREAS]
new g_Conn[MAX_CONNS]
new g_IdMap[MAX_IDMAP]
new Float:g_SX[MAX_SPOTS], Float:g_SY[MAX_SPOTS], Float:g_SZ[MAX_SPOTS], g_SFlags[MAX_SPOTS], g_SArea[MAX_SPOTS]

// ---- search ----
new g_Search
new g_SeenAt[MAX_AREAS], g_DoneAt[MAX_AREAS], g_Parent[MAX_AREAS]
new Float:g_Cost[MAX_AREAS]
new g_Heap[MAX_HEAP], Float:g_HeapKey[MAX_HEAP], g_HeapN
new g_Tmp[MAX_AREAS]
new Float:g_Flood[FLOOD_SLOTS][MAX_AREAS]
new bool:g_FloodDone[FLOOD_SLOTS]

// ---- bots ----
new g_State[33], g_Flags[33]
new Float:g_Goal[33][3]
new g_Path[33][MAX_PATH], g_PathLen[33], g_W[33]
new bool:g_Partial[33], bool:g_Truncated[33]
new Float:g_Wp[33][3], bool:g_WpJump[33], Float:g_WpJumpV[33], bool:g_WpDuck[33]
new Float:g_Best[33], Float:g_ProgressAt[33], g_Stuck[33]
new bool:g_ForceJump[33], Float:g_StrafeUntil[33], Float:g_StrafeSign[33]
new Float:g_JumpUntil[33], Float:g_NextJump[33], Float:g_WaitUntil[33]
new Float:g_FightRadius[33], bool:g_Released[33], Float:g_ReleaseUntil[33]
new bool:g_HoldDuck[33]
new g_OffTicks[33]
new Float:g_Aim[33][3], g_AimW[33]             // look-ahead: a later waypoint in straight reach

new g_MaxPlayers, g_pGravity

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	register_forward(FM_PlayerPostThink, "fw_postthink", 1)
	register_forward(FM_CmdStart, "fw_cmdstart")
	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	register_srvcmd("botnav_info", "cmd_info")
	g_MaxPlayers = get_maxplayers()
	g_pGravity = get_cvar_pointer("sv_gravity")
}

public plugin_cfg()
{
	load_mesh()
	set_task(TICK, "task_tick", TASK_TICK, _, _, "b")
}

public plugin_natives()
{
	register_library("gm_botnav")
	register_native("botnav_has_mesh", "n_has_mesh")
	register_native("botnav_path_to", "n_path_to")
	register_native("botnav_stop", "n_stop")
	register_native("botnav_hold", "n_hold")
	register_native("botnav_status", "n_status")
	register_native("botnav_set_fight", "n_set_fight")
	register_native("botnav_hiding_spot_count", "n_spot_count")
	register_native("botnav_hiding_spot", "n_spot")
	register_native("botnav_spot_area", "n_spot_area")
	register_native("botnav_area_count", "n_area_count")
	register_native("botnav_area", "n_area")
	register_native("botnav_area_at", "n_area_at")
	register_native("botnav_flood", "n_flood")
	register_native("botnav_flood_dist", "n_flood_dist")
	register_native("botnav_farthest", "n_farthest")
}

// A mesh made by the bots learning the map only exists after they finished
// (the map then reloads), so a map without one is tried again each round.
public ev_newround()
{
	if (!g_Loaded)
		load_mesh()
}

public client_disconnected(id)
{
	reset_bot(id)
}

reset_bot(id)
{
	g_State[id] = BN_IDLE
	g_Released[id] = false
	g_FightRadius[id] = 0.0
	g_HoldDuck[id] = false
}

// ===========================================================================
// Loading the .nav file (format: ReGameDLL game_shared/bot/nav_file.cpp,
// CNavArea::Load and HidingSpot::Load, versions 1 to 5)
// ===========================================================================

read_u8(f)
{
	new v
	fread(f, v, BLOCK_BYTE)
	return v & 0xFF
}

read_u16(f)
{
	new v
	fread(f, v, BLOCK_SHORT)
	return v & 0xFFFF
}

read_u32(f)
{
	new v
	fread(f, v, BLOCK_INT)
	return v
}

Float:read_float(f)
{
	new v
	fread(f, v, BLOCK_INT)
	return Float:v
}

skip(f, bytes)
{
	if (bytes > 0)
		fseek(f, bytes, SEEK_CUR)
}

load_mesh()
{
	g_Loaded = false
	g_Areas = g_Conns = g_Spots = 0
	g_FloodDone[0] = g_FloodDone[1] = false

	new map[64], path[96]
	get_mapname(map, charsmax(map))
	formatex(path, charsmax(path), "maps/%s.nav", map)
	new f = fopen(path, "rb", true)
	if (!f)
		return

	new version
	if (read_u32(f) != 0xFEEDFACE || (version = read_u32(f)) < 1 || version > 5)
	{
		fclose(f)
		log_amx("%s: not a navigation file I can read", path)
		return
	}
	if (version >= 4)
		read_u32(f)                                     // size of the bsp it was made for
	if (version >= 5)
		for (new n = read_u16(f); n > 0; n--)           // place names
			skip(f, read_u16(f))

	new count = read_u32(f)
	if (count <= 0 || count > 200000)
	{
		fclose(f)
		return
	}

	arrayset(g_IdMap, -1, MAX_IDMAP)
	new Float:ext[6], bool:bad, dropped
	for (new a = 0; a < count && !bad; a++)
	{
		if (feof(f))
		{
			bad = true
			break
		}
		new id = read_u32(f)
		new flags = read_u8(f)
		fread_blocks(f, ext, 6, BLOCK_INT)
		new Float:neZ = read_float(f)
		new Float:swZ = read_float(f)

		new bool:keep = g_Areas < MAX_AREAS
		new idx = g_Areas
		if (keep)
		{
			g_AId[idx] = id
			g_AFlags[idx] = flags
			g_AX0[idx] = ext[0]; g_AY0[idx] = ext[1]; g_AZ0[idx] = ext[2]
			g_AX1[idx] = ext[3]; g_AY1[idx] = ext[4]; g_AZ1[idx] = ext[5]
			g_ANeZ[idx] = neZ; g_ASwZ[idx] = swZ
			g_ACx[idx] = (ext[0] + ext[3]) * 0.5
			g_ACy[idx] = (ext[1] + ext[4]) * 0.5
			g_ACz[idx] = (ext[2] + ext[5]) * 0.5
			g_AFirst[idx] = g_Conns
		}
		else
			dropped++

		// connections, per direction (north, east, south, west)
		for (new d = 0; d < 4 && !bad; d++)
		{
			new n = read_u32(f)
			if (n < 0 || n > 1024)
			{
				bad = true
				break
			}
			for (new i = 0; i < n; i++)
			{
				new cid = read_u32(f)
				if (keep && g_Conns < MAX_CONNS)
					g_Conn[g_Conns++] = cid
			}
		}
		if (bad)
			break

		// hiding spots
		for (new h = read_u8(f); h > 0; h--)
		{
			new Float:pos[3], sflags = 1
			if (version >= 2)
				read_u32(f)                             // spot id
			fread_blocks(f, pos, 3, BLOCK_INT)
			if (version >= 2)
				sflags = read_u8(f)
			if (keep && g_Spots < MAX_SPOTS)
			{
				g_SX[g_Spots] = pos[0]
				g_SY[g_Spots] = pos[1]
				g_SZ[g_Spots] = pos[2]
				g_SFlags[g_Spots] = sflags
				g_SArea[g_Spots] = idx
				g_Spots++
			}
		}

		skip(f, read_u8(f) * 14)                        // approach areas

		new enc = read_u32(f)                           // encounter paths
		if (enc < 0 || enc > 20000)
		{
			bad = true
			break
		}
		if (version < 3)
		{
			for (new e = 0; e < enc; e++)
			{
				skip(f, 32)
				skip(f, read_u8(f) * 16)
			}
		}
		else
		{
			for (new e = 0; e < enc; e++)
			{
				skip(f, 10)
				skip(f, read_u8(f) * 5)
			}
			if (version >= 5)
				read_u16(f)                             // place
		}

		if (keep)
		{
			if (id >= 0 && id < MAX_IDMAP)
				g_IdMap[id] = idx
			g_Areas++
		}
	}
	fclose(f)

	if (bad && g_Areas < 2)
	{
		g_Areas = g_Conns = g_Spots = 0
		log_amx("%s is damaged, bots won't use it", path)
		return
	}

	g_AFirst[g_Areas] = g_Conns
	for (new c = 0; c < g_Conns; c++)
	{
		new cid = g_Conn[c]
		g_Conn[c] = (cid >= 0 && cid < MAX_IDMAP) ? g_IdMap[cid] : -1
	}
	g_Loaded = g_Areas > 0
	log_amx("Bot navigation: %d areas, %d links, %d hiding spots from %s%s", g_Areas, g_Conns, g_Spots, path,
		dropped ? " (map too big, some areas left out)" : "")
}

// ===========================================================================
// Geometry
// ===========================================================================

// Height of an area's floor at (x, y) (CNavArea::GetZ)
Float:area_z(a, Float:x, Float:y)
{
	new Float:dx = g_AX1[a] - g_AX0[a]
	new Float:dy = g_AY1[a] - g_AY0[a]
	if (dx <= 0.0 || dy <= 0.0)
		return g_ANeZ[a]
	new Float:u = floatclamp((x - g_AX0[a]) / dx, 0.0, 1.0)
	new Float:v = floatclamp((y - g_AY0[a]) / dy, 0.0, 1.0)
	new Float:north = g_AZ0[a] + u * (g_ANeZ[a] - g_AZ0[a])
	new Float:south = g_ASwZ[a] + u * (g_AZ1[a] - g_ASwZ[a])
	return north + v * (south - north)
}

// A player (origin about 36 units above the floor) standing in area a
bool:in_area(a, const Float:org[3])
{
	if (org[0] < g_AX0[a] || org[0] > g_AX1[a] || org[1] < g_AY0[a] || org[1] > g_AY1[a])
		return false
	new Float:dz = org[2] - area_z(a, org[0], org[1])
	return dz >= -20.0 && dz <= 90.0
}

// The area under a point (player origin or floor point); nearest area if none
area_at(const Float:pos[3])
{
	new best = -1, Float:bestScore = 999999.0
	for (new a = 0; a < g_Areas; a++)
	{
		if (pos[0] < g_AX0[a] - 4.0 || pos[0] > g_AX1[a] + 4.0 || pos[1] < g_AY0[a] - 4.0 || pos[1] > g_AY1[a] + 4.0)
			continue
		new Float:dz = pos[2] - area_z(a, pos[0], pos[1])
		if (dz < -30.0 || dz > 120.0)
			continue
		// floors below the point first, the closest one wins
		new Float:score = dz >= 0.0 ? dz : 500.0 - dz
		if (score < bestScore)
		{
			bestScore = score
			best = a
		}
	}
	if (best != -1)
		return best

	bestScore = 999999999.0
	for (new a = 0; a < g_Areas; a++)
	{
		new Float:dx = pos[0] - floatclamp(pos[0], g_AX0[a], g_AX1[a])
		new Float:dy = pos[1] - floatclamp(pos[1], g_AY0[a], g_AY1[a])
		new Float:dz = (pos[2] - 36.0 - g_ACz[a]) * 2.0
		new Float:d = dx * dx + dy * dy + dz * dz
		if (d < bestScore)
		{
			bestScore = d
			best = a
		}
	}
	return best
}

// Where to cross from area a into area b: on their shared edge, as close to
// the bot's line as the doorway allows (keeps the route straight)
portal(a, b, const Float:from[3], Float:out[3])
{
	new Float:lo = floatmax(g_AX0[a], g_AX0[b]), Float:hi = floatmin(g_AX1[a], g_AX1[b])
	out[0] = (hi - lo > 64.0) ? floatclamp(from[0], lo + 24.0, hi - 24.0) : (lo + hi) * 0.5
	lo = floatmax(g_AY0[a], g_AY0[b]); hi = floatmin(g_AY1[a], g_AY1[b])
	out[1] = (hi - lo > 64.0) ? floatclamp(from[1], lo + 24.0, hi - 24.0) : (lo + hi) * 0.5
	out[2] = area_z(b, out[0], out[1])
}

Float:dist2d(const Float:a[3], const Float:b[3])
{
	new Float:dx = a[0] - b[0], Float:dy = a[1] - b[1]
	return floatsqroot(dx * dx + dy * dy)
}

// ===========================================================================
// A* and flood fill over the areas
// ===========================================================================

heap_push(a, Float:key)
{
	if (g_HeapN >= MAX_HEAP)
		return
	new i = g_HeapN++
	while (i > 0)
	{
		new p = (i - 1) >> 1
		if (g_HeapKey[p] <= key)
			break
		g_Heap[i] = g_Heap[p]
		g_HeapKey[i] = g_HeapKey[p]
		i = p
	}
	g_Heap[i] = a
	g_HeapKey[i] = key
}

heap_pop()
{
	new top = g_Heap[0]
	g_HeapN--
	if (g_HeapN > 0)
	{
		new a = g_Heap[g_HeapN], Float:key = g_HeapKey[g_HeapN]
		new i = 0
		for (;;)
		{
			new c = 2 * i + 1
			if (c >= g_HeapN)
				break
			if (c + 1 < g_HeapN && g_HeapKey[c + 1] < g_HeapKey[c])
				c++
			if (g_HeapKey[c] >= key)
				break
			g_Heap[i] = g_Heap[c]
			g_HeapKey[i] = g_HeapKey[c]
			i = c
		}
		g_Heap[i] = a
		g_HeapKey[i] = key
	}
	return top
}

new_search()
{
	if (++g_Search > 0x3FFFFFFF)
	{
		arrayset(g_SeenAt, 0, MAX_AREAS)
		arrayset(g_DoneAt, 0, MAX_AREAS)
		g_Search = 1
	}
	g_HeapN = 0
}

Float:center_dist(a, b)
{
	new Float:dx = g_ACx[b] - g_ACx[a], Float:dy = g_ACy[b] - g_ACy[a], Float:dz = g_ACz[b] - g_ACz[a]
	return floatsqroot(dx * dx + dy * dy + dz * dz)
}

Float:edge_cost(a, b)
{
	new Float:c = center_dist(a, b)
	if (g_AFlags[b] & NAV_CROUCH)
		c *= 1.5
	if (g_AFlags[b] & NAV_JUMP)
		c += 50.0
	if (g_ACz[b] - g_ACz[a] > STEP_HEIGHT)
		c += 30.0
	return c
}

// Returns the goal, or (reached = false) the searched area closest to it
astar(s, g, &bool:reached)
{
	new_search()
	reached = false
	g_SeenAt[s] = g_Search
	g_Cost[s] = 0.0
	g_Parent[s] = -1
	new Float:bestH = center_dist(s, g), best = s
	heap_push(s, bestH)

	new expanded
	while (g_HeapN > 0 && expanded < MAX_EXPAND)
	{
		new a = heap_pop()
		if (g_DoneAt[a] == g_Search)
			continue
		g_DoneAt[a] = g_Search
		expanded++
		if (a == g)
		{
			reached = true
			return g
		}
		new Float:h = center_dist(a, g)
		if (h < bestH)
		{
			bestH = h
			best = a
		}
		for (new c = g_AFirst[a], end = g_AFirst[a + 1]; c < end; c++)
		{
			new b = g_Conn[c]
			if (b < 0 || g_DoneAt[b] == g_Search)
				continue
			new Float:cost = g_Cost[a] + edge_cost(a, b)
			if (g_SeenAt[b] == g_Search && cost >= g_Cost[b])
				continue
			g_SeenAt[b] = g_Search
			g_Cost[b] = cost
			g_Parent[b] = a
			heap_push(b, cost + center_dist(b, g))
		}
	}
	return best
}

flood(slot, s)
{
	for (new a = 0; a < g_Areas; a++)
		g_Flood[slot][a] = -1.0
	new_search()
	g_SeenAt[s] = g_Search
	g_Cost[s] = 0.0
	heap_push(s, 0.0)
	new reached
	while (g_HeapN > 0)
	{
		new a = heap_pop()
		if (g_DoneAt[a] == g_Search)
			continue
		g_DoneAt[a] = g_Search
		g_Flood[slot][a] = g_Cost[a]
		reached++
		for (new c = g_AFirst[a], end = g_AFirst[a + 1]; c < end; c++)
		{
			new b = g_Conn[c]
			if (b < 0 || g_DoneAt[b] == g_Search)
				continue
			new Float:cost = g_Cost[a] + edge_cost(a, b)
			if (g_SeenAt[b] == g_Search && cost >= g_Cost[b])
				continue
			g_SeenAt[b] = g_Search
			g_Cost[b] = cost
			heap_push(b, cost)
		}
	}
	g_FloodDone[slot] = true
	return reached
}

// ===========================================================================
// Routing a bot
// ===========================================================================

compute_path(id)
{
	new Float:org[3]
	pev(id, pev_origin, org)
	new s = area_at(org)
	new g = area_at(g_Goal[id])
	if (s < 0 || g < 0)
	{
		fail(id)
		return
	}

	new bool:reached
	new last = astar(s, g, reached)
	if (!reached && last == s && s != g)
	{
		fail(id)
		return
	}

	new n
	for (new a = last; a != -1 && n < MAX_AREAS; a = g_Parent[a])
		g_Tmp[n++] = a
	new len = n < MAX_PATH ? n : MAX_PATH
	for (new i = 0; i < len; i++)
		g_Path[id][i] = g_Tmp[n - 1 - i]
	g_PathLen[id] = len
	g_Truncated[id] = n > MAX_PATH
	g_Partial[id] = !reached
	g_State[id] = BN_MOVING
	g_AimW[id] = 0
	g_OffTicks[id] = 0
	set_waypoint(id, 1, org)
}

// Unreachable or hopeless: callers asking to persist get another try shortly
fail(id)
{
	if (g_Flags[id] & BN_PERSIST)
	{
		g_State[id] = BN_PENDING
		g_WaitUntil[id] = get_gametime() + 2.0
	}
	else
		g_State[id] = BN_FAILED
}

// Waypoint w (1..len): the crossing from path area w-1 into w; w == len is the goal itself
set_waypoint(id, w, const Float:org[3])
{
	new n = g_PathLen[id]
	g_W[id] = w
	g_WpJump[id] = false
	g_WpDuck[id] = false
	g_WpJumpV[id] = 270.0
	if (w >= n)
	{
		new last = g_Path[id][n - 1]
		if (g_Partial[id] || g_Truncated[id])
		{
			g_Wp[id][0] = g_ACx[last]
			g_Wp[id][1] = g_ACy[last]
			g_Wp[id][2] = area_z(last, g_ACx[last], g_ACy[last])
		}
		else
			g_Wp[id] = g_Goal[id]
		g_WpDuck[id] = (g_AFlags[last] & NAV_CROUCH) != 0
	}
	else
	{
		new a = g_Path[id][w - 1], b = g_Path[id][w]
		portal(a, b, org, g_Wp[id])
		new Float:dz = g_Wp[id][2] - area_z(a, g_Wp[id][0], g_Wp[id][1])
		if (dz > STEP_HEIGHT || ((g_AFlags[a] | g_AFlags[b]) & NAV_JUMP))
		{
			g_WpJump[id] = true
			new Float:grav = g_pGravity ? get_pcvar_float(g_pGravity) : 800.0
			new Float:h = floatmax(dz, 20.0) + 14.0
			g_WpJumpV[id] = floatclamp(floatsqroot(2.0 * grav * h), 250.0, 345.0)
		}
		g_WpDuck[id] = ((g_AFlags[a] | g_AFlags[b]) & NAV_CROUCH) != 0
	}
	if (g_AimW[id] <= w)
	{
		g_AimW[id] = w
		g_Aim[id] = g_Wp[id]
	}
	g_Best[id] = 999999.0
	g_ProgressAt[id] = get_gametime()
}

// CZ meshes are made of many small areas; going from crossing to crossing
// zigzags. Aim further down the route (up to 5 crossings) while the straight
// line there stays on the route's own areas - so no walls, no pits - and
// stop before any crossing that needs a jump or a crouch.
look_ahead(id, const Float:org[3])
{
	new n = g_PathLen[id], w = g_W[id]
	if (w >= n || g_WpJump[id] || g_WpDuck[id])
		return
	new Float:pt[3], Float:bestPt[3], best = w
	new last = w + 5 < n ? w + 5 : n
	for (new k = w + 1; k <= last; k++)
	{
		if (k < n)
		{
			new a = g_Path[id][k - 1], b = g_Path[id][k]
			if ((g_AFlags[a] | g_AFlags[b]) & (NAV_JUMP | NAV_CROUCH))
				break
			portal(a, b, org, pt)
			if (pt[2] - area_z(a, pt[0], pt[1]) > STEP_HEIGHT)
				break
		}
		else
		{
			if (g_Partial[id] || g_Truncated[id] || (g_AFlags[g_Path[id][n - 1]] & NAV_CROUCH))
				break
			pt = g_Goal[id]
		}
		if (!on_route(id, org, pt, w - 1, k < n ? k : n - 1))
			break
		best = k
		bestPt = pt
	}
	if (best > w)
	{
		g_AimW[id] = best
		g_Aim[id] = bestPt
	}
}

// Every 8 units of the line from org to pt (a floor point) is over one of path areas from..to
bool:on_route(id, const Float:org[3], const Float:pt[3], from, to)
{
	new Float:dx = pt[0] - org[0], Float:dy = pt[1] - org[1], Float:dz = pt[2] + 36.0 - org[2]
	new steps = floatround(floatsqroot(dx * dx + dy * dy) / 8.0, floatround_ceil)
	if (steps < 1)
		steps = 1
	if (from < 0)
		from = 0
	new Float:p[3]
	for (new s = 1; s <= steps; s++)
	{
		new Float:t = float(s) / float(steps)
		p[0] = org[0] + dx * t
		p[1] = org[1] + dy * t
		p[2] = org[2] + dz * t
		new bool:found
		for (new j = from; j <= to && !found; j++)
			found = in_area(g_Path[id][j], p)
		if (!found)
			return false
	}
	return true
}

// ===========================================================================
// Driving: every frame, after the bot's own movement
// ===========================================================================

public fw_postthink(id)
{
	new st = g_State[id]
	if (st != BN_MOVING && st != BN_HOLD)
		return FMRES_IGNORED
	if (!is_user_alive(id))
		return FMRES_IGNORED

	new Float:vel[3]
	pev(id, pev_velocity, vel)

	if (st == BN_HOLD)
	{
		vel[0] = vel[1] = 0.0
		if (vel[2] > 0.0 && (pev(id, pev_flags) & FL_ONGROUND))
			vel[2] = 0.0
		set_pev(id, pev_velocity, vel)
		return FMRES_IGNORED
	}

	if (g_Released[id] || pev(id, pev_movetype) == MOVETYPE_FLY)
		return FMRES_IGNORED                         // fighting, or on a ladder: the brain drives
	new Float:speed
	pev(id, pev_maxspeed, speed)
	if (speed < 50.0)
		return FMRES_IGNORED                         // frozen (freeze time, Hide and Seek seekers)
	if (speed > 900.0)
		speed = 250.0

	new Float:org[3], Float:now = get_gametime()
	pev(id, pev_origin, org)

	// move on once the bot is in a later area of the route (the farthest one
	// it could have reached: one past the waypoint it aims at)
	new n = g_PathLen[id], w = g_W[id]
	if (w < n)
	{
		new top = (g_AimW[id] > w ? g_AimW[id] : w) + 1
		if (top > n - 1)
			top = n - 1
		for (new j = top; j >= w; j--)
		{
			if (in_area(g_Path[id][j], org))
			{
				set_waypoint(id, j + 1, org)
				g_Stuck[id] = 0
				break
			}
		}
		if (g_W[id] == w && dist2d(org, g_Wp[id]) < 16.0)
		{
			set_waypoint(id, w + 1, org)
			g_Stuck[id] = 0
		}
		w = g_W[id]
	}

	// steer at the look-ahead point when there is one, else the next crossing
	new Float:dx, Float:dy
	if (g_AimW[id] > w)
	{
		dx = g_Aim[id][0] - org[0]
		dy = g_Aim[id][1] - org[1]
	}
	else
	{
		dx = g_Wp[id][0] - org[0]
		dy = g_Wp[id][1] - org[1]
	}
	new Float:len = floatsqroot(dx * dx + dy * dy)

	if (w >= n && len < 24.0)
	{
		new Float:dz = org[2] - g_Wp[id][2]
		if (dz > -60.0 && dz < 110.0)
		{
			if (g_Truncated[id])
				g_State[id] = BN_PENDING             // long trip: route the rest from here
			else if (g_Partial[id])
				fail(id)
			else
				g_State[id] = BN_ARRIVED
			return FMRES_IGNORED
		}
	}
	if (len < 1.0)
		return FMRES_IGNORED

	new Float:want = speed
	if (w >= n && len < 64.0)
		want = floatmax(90.0, len * 4.0)
	new Float:dirx = dx / len, Float:diry = dy / len
	new Float:wx = dirx * want, Float:wy = diry * want
	if (now < g_StrafeUntil[id])
	{
		// sidestep around whatever blocks the way
		wx = (dirx * 0.5 - diry * g_StrafeSign[id] * 0.85) * want
		wy = (diry * 0.5 + dirx * g_StrafeSign[id] * 0.85) * want
	}

	if (pev(id, pev_flags) & FL_ONGROUND)
	{
		vel[0] = wx
		vel[1] = wy
		if (now >= g_NextJump[id] && (g_ForceJump[id] || (g_WpJump[id] && len < 64.0)))
		{
			vel[2] = g_ForceJump[id] ? 270.0 : g_WpJumpV[id]
			g_ForceJump[id] = false
			g_JumpUntil[id] = now + 1.0
			g_NextJump[id] = now + 0.7
		}
	}
	else if (now < g_JumpUntil[id])
	{
		vel[0] = wx                                  // carry our own jumps onto the ledge
		vel[1] = wy
	}
	else
	{
		vel[0] = vel[0] * 0.9 + wx * 0.1
		vel[1] = vel[1] * 0.9 + wy * 0.1
	}
	set_pev(id, pev_velocity, vel)

	// turn the view toward where we run, a few degrees per frame
	new Float:va[3]
	pev(id, pev_v_angle, va)
	new Float:diff = floatatan2(dy, dx, degrees) - va[1]
	while (diff > 180.0) diff -= 360.0
	while (diff < -180.0) diff += 360.0
	va[1] += floatclamp(diff, -6.0, 6.0)
	set_pev(id, pev_v_angle, va)
	return FMRES_IGNORED
}

public fw_cmdstart(id, uc, seed)
{
	if (id < 1 || id > 32)
		return FMRES_IGNORED
	if (g_State[id] == BN_HOLD)
	{
		new buttons = get_uc(uc, UC_Buttons) & ~IN_JUMP     // keep still means no hopping either
		set_uc(uc, UC_Buttons, g_HoldDuck[id] ? (buttons | IN_DUCK) : buttons)
	}
	else if (g_State[id] == BN_MOVING && !g_Released[id] && g_WpDuck[id])
		set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) | IN_DUCK)
	return FMRES_IGNORED
}

// ===========================================================================
// Every 0.1 s: routing, fights, getting unstuck
// ===========================================================================

public task_tick()
{
	if (!g_Loaded)
		return
	new budget = ROUTES_PER_TICK
	new Float:now = get_gametime()
	for (new id = 1; id <= g_MaxPlayers; id++)
	{
		new st = g_State[id]
		if (st != BN_PENDING && st != BN_MOVING && st != BN_HOLD)
			continue
		if (!is_user_alive(id))
		{
			g_State[id] = BN_IDLE
			continue
		}
		if (st == BN_PENDING)
		{
			if (now >= g_WaitUntil[id] && budget > 0)
			{
				budget--
				compute_path(id)
			}
			continue
		}
		if (st != BN_MOVING)
			continue

		if (g_FightRadius[id] > 0.0)
		{
			if (enemy_in_sight(id, g_FightRadius[id]))
			{
				g_Released[id] = true
				g_ReleaseUntil[id] = now + 2.0
			}
			else if (g_Released[id] && now >= g_ReleaseUntil[id])
			{
				g_Released[id] = false
				g_State[id] = BN_PENDING             // the fight moved us: new route from here
				g_WaitUntil[id] = 0.0
				continue
			}
		}

		new Float:speed
		pev(id, pev_maxspeed, speed)
		if (g_Released[id] || speed < 50.0)
		{
			g_ProgressAt[id] = now
			continue
		}

		new Float:org[3]
		pev(id, pev_origin, org)
		// knocked or fell off the route: standing outside both areas of the
		// current crossing for 0.6 s
		new w = g_W[id], n = g_PathLen[id]
		new here = g_Path[id][w - 1], next = g_Path[id][w < n ? w : n - 1]
		if ((pev(id, pev_flags) & FL_ONGROUND) && !in_area(here, org) && !in_area(next, org))
		{
			if (++g_OffTicks[id] >= 6)
			{
				g_OffTicks[id] = 0
				g_State[id] = BN_PENDING
				g_WaitUntil[id] = 0.0
				continue
			}
		}
		else
			g_OffTicks[id] = 0

		look_ahead(id, org)
		new Float:d = dist2d(org, g_Wp[id])
		if (d < g_Best[id] - 8.0)
		{
			g_Best[id] = d
			g_ProgressAt[id] = now
			continue
		}
		if (now - g_ProgressAt[id] < 1.5)
			continue

		g_ProgressAt[id] = now
		g_Best[id] = d
		switch (++g_Stuck[id])
		{
			case 1, 3:
			{
				g_ForceJump[id] = true
				g_StrafeUntil[id] = now + 0.5
				g_StrafeSign[id] = random_num(0, 1) ? 1.0 : -1.0
			}
			case 2, 4:
			{
				if (budget > 0)
					budget--
				compute_path(id)
			}
			default:
			{
				g_Stuck[id] = 0
				if (g_Flags[id] & BN_PERSIST)
				{
					g_State[id] = BN_PENDING
					g_WaitUntil[id] = now + 1.0
				}
				else
					g_State[id] = BN_FAILED
			}
		}
	}
}

bool:enemy_in_sight(id, Float:radius)
{
	new Float:org[3], Float:eye[3], Float:other[3]
	pev(id, pev_origin, org)
	pev(id, pev_view_ofs, eye)
	eye[0] += org[0]; eye[1] += org[1]; eye[2] += org[2]
	new team = get_user_team(id)
	for (new i = 1; i <= g_MaxPlayers; i++)
	{
		if (i == id || !is_user_alive(i) || get_user_team(i) == team)
			continue
		pev(i, pev_origin, other)
		if (get_distance_f(org, other) > radius)
			continue
		other[2] += 17.0
		engfunc(EngFunc_TraceLine, eye, other, IGNORE_MONSTERS, id, 0)
		new Float:frac
		get_tr2(0, TR_flFraction, frac)
		if (frac >= 1.0)
			return true
	}
	return false
}

// ===========================================================================
// Natives
// ===========================================================================

bool:valid_bot(id)
{
	return 1 <= id <= g_MaxPlayers && is_user_connected(id) && is_user_bot(id)
}

public n_has_mesh(plugin, params)
{
	return g_Loaded
}

public n_path_to(plugin, params)
{
	new id = get_param(1)
	if (!g_Loaded || !valid_bot(id))
		return 0
	get_array_f(2, g_Goal[id], 3)
	g_Flags[id] = params >= 3 ? get_param(3) : 0
	g_State[id] = BN_PENDING
	g_WaitUntil[id] = 0.0
	g_Released[id] = false
	g_Stuck[id] = 0
	return 1
}

public n_stop(plugin, params)
{
	new id = get_param(1)
	if (1 <= id <= 32)
	{
		g_State[id] = BN_IDLE
		g_Released[id] = false
	}
	return 1
}

public n_hold(plugin, params)
{
	new id = get_param(1)
	if (!valid_bot(id))
		return 0
	g_State[id] = BN_HOLD
	g_HoldDuck[id] = params >= 2 ? get_param(2) != 0 : true
	return 1
}

public n_status(plugin, params)
{
	new id = get_param(1)
	return (1 <= id <= 32) ? g_State[id] : BN_IDLE
}

public n_set_fight(plugin, params)
{
	new id = get_param(1)
	if (1 <= id <= 32)
	{
		g_FightRadius[id] = get_param_f(2)
		if (g_FightRadius[id] <= 0.0)
			g_Released[id] = false
	}
	return 1
}

public n_spot_count(plugin, params)
{
	return g_Loaded ? g_Spots : 0
}

public n_spot(plugin, params)
{
	new i = get_param(1)
	if (i < 0 || i >= g_Spots)
		return 0
	new Float:pos[3]
	pos[0] = g_SX[i]; pos[1] = g_SY[i]; pos[2] = g_SZ[i]
	set_array_f(2, pos, 3)
	return g_SFlags[i]
}

public n_spot_area(plugin, params)
{
	new i = get_param(1)
	return (i < 0 || i >= g_Spots) ? -1 : g_SArea[i]
}

public n_area_count(plugin, params)
{
	return g_Loaded ? g_Areas : 0
}

public n_area(plugin, params)
{
	new a = get_param(1)
	if (a < 0 || a >= g_Areas)
		return 0
	new Float:c[3]
	c[0] = g_ACx[a]; c[1] = g_ACy[a]; c[2] = area_z(a, c[0], c[1])
	set_array_f(2, c, 3)
	return g_AFlags[a] | ((g_AFirst[a + 1] - g_AFirst[a]) << 8)
}

public n_area_at(plugin, params)
{
	if (!g_Loaded)
		return -1
	new Float:pos[3]
	get_array_f(1, pos, 3)
	return area_at(pos)
}

public n_flood(plugin, params)
{
	new slot = get_param(1)
	if (!g_Loaded || slot < 0 || slot >= FLOOD_SLOTS)
		return 0
	new Float:pos[3]
	get_array_f(2, pos, 3)
	new s = area_at(pos)
	return s < 0 ? 0 : flood(slot, s)
}

public Float:n_flood_dist(plugin, params)
{
	new slot = get_param(1), a = get_param(2)
	if (slot < 0 || slot >= FLOOD_SLOTS || !g_FloodDone[slot] || a < 0 || a >= g_Areas)
		return -1.0
	return g_Flood[slot][a]
}

public n_farthest(plugin, params)
{
	new slot = get_param(1)
	if (slot < 0 || slot >= FLOOD_SLOTS || !g_FloodDone[slot])
		return -1
	new best = -1, Float:far = -1.0
	for (new a = 0; a < g_Areas; a++)
	{
		if (g_Flood[slot][a] > far)
		{
			far = g_Flood[slot][a]
			best = a
		}
	}
	if (best >= 0)
	{
		new Float:c[3]
		c[0] = g_ACx[best]; c[1] = g_ACy[best]; c[2] = area_z(best, c[0], c[1])
		set_array_f(2, c, 3)
	}
	return best
}

// ===========================================================================

public cmd_info()
{
	static const NAMES[][] = { "idle", "routing", "moving", "arrived", "failed", "holding" }
	if (!g_Loaded)
	{
		server_print("[botnav] no navigation mesh for this map (bots make maps/<map>.nav when they learn it)")
		return PLUGIN_HANDLED
	}
	server_print("[botnav] %d areas, %d links, %d hiding spots", g_Areas, g_Conns, g_Spots)
	new name[32]
	for (new id = 1; id <= g_MaxPlayers; id++)
	{
		if (!is_user_connected(id) || !is_user_bot(id))
			continue
		get_user_name(id, name, charsmax(name))
		server_print("  %-20s %-8s waypoint %d/%d stuck %d%s%s", name, NAMES[g_State[id]], g_W[id], g_PathLen[id],
			g_Stuck[id], g_Released[id] ? " (fighting)" : "", g_Partial[id] ? " (partial route)" : "")
	}
	return PLUGIN_HANDLED
}
