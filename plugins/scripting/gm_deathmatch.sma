/*
	GM Deathmatch
	Team deathmatch in plain AMX Mod X - no binary module, so it keeps working
	on the Half-Life 25th Anniversary engine (CSDM 2.1.2's module does not).

	- Respawn shortly after dying
	- Free for all when ReGameDLL's mp_freeforall is on
	- Random spawn points from configs/csdm/<map>.spawns.cfg, or picked from the
	  bots' navigation file (maps/<map>.nav) on maps without one
	- Spawn protection
	- Gun menu for players (say /guns to change), random guns for bots
	- No buying, no bomb/hostage objectives, dropped weapons are cleaned up
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fun>
#include <fakemeta>
#include <hamsandwich>

#define PLUGIN  "GM Deathmatch"
#define VERSION "1.0"
#define AUTHOR  "local"

#define MAX_SPAWNS     128
#define NAV_SPAWNS     64
#define NAV_CANDIDATES 2048
#define TASK_RESPAWN   5100
#define TASK_PROTECT   5200
#define TASK_EQUIP     5300
#define TASK_NAV_WAIT  5900

new bool:g_BotHams

new const OBJECTIVES[][] =
{
	"func_bomb_target", "info_bomb_target", "hostage_entity", "monster_scientist",
	"func_hostage_rescue", "info_hostage_rescue", "func_vip_safetyzone", "func_escapezone"
}

new const PRIMARY[][][] =
{
	{ "weapon_ak47",    "AK-47" },
	{ "weapon_m4a1",    "M4A1" },
	{ "weapon_awp",     "AWP" },
	{ "weapon_aug",     "AUG" },
	{ "weapon_sg552",   "SG-552" },
	{ "weapon_famas",   "FAMAS" },
	{ "weapon_galil",   "Galil" },
	{ "weapon_scout",   "Scout" },
	{ "weapon_mp5navy", "MP5" },
	{ "weapon_p90",     "P90" },
	{ "weapon_ump45",   "UMP-45" },
	{ "weapon_m3",      "M3 shotgun" },
	{ "weapon_xm1014",  "Auto shotgun" },
	{ "weapon_m249",    "M249" }
}

new const SECONDARY[][][] =
{
	{ "weapon_deagle",    "Desert Eagle" },
	{ "weapon_usp",       "USP" },
	{ "weapon_glock18",   "Glock" },
	{ "weapon_elite",     "Dual Elites" },
	{ "weapon_fiveseven", "Five-SeveN" },
	{ "weapon_p228",      "P228" }
}

// gmdm_guns: 0 everything, 1 rifles, 2 SMGs & shotguns, 3 snipers, 4 pistols only
enum { GUNS_ALL, GUNS_RIFLES, GUNS_SMGS, GUNS_SNIPERS, GUNS_PISTOLS }
new const PRIMARY_KIND[] = { 1, 1, 3, 1, 1, 1, 1, 3, 2, 2, 2, 2, 2, 1 }

// Bots get one of these (good weapons only)
new const BOT_PRIMARY[] = { 0, 1, 1, 0, 3, 4, 2, 8 }

// Max backpack ammo, indexed by CSW_*
new const MAXBP[31] = { 0, 52, 0, 90, 0, 32, 0, 100, 90, 0, 120, 100, 100, 90, 90, 90, 100, 120, 30, 120, 200, 32, 90, 120, 90, 0, 35, 90, 90, 0, 100 }

new Float:g_Spawns[MAX_SPAWNS][3]
new Float:g_SpawnAngles[MAX_SPAWNS][3]
new g_SpawnCount
new g_NavWaitSize

new g_Primary[33] = { -1, ... }
new g_Secondary[33] = { -1, ... }
new bool:g_Remember[33]

new g_pDelay, g_pProtect, g_pRandom, g_pGrenade, g_pGuns, g_pFreeForAll
new g_FwSpawn

public plugin_precache()
{
	// Remove objective entities before they spawn so bots and rounds ignore them
	g_FwSpawn = register_forward(FM_Spawn, "fw_spawn_entity")
}

public fw_spawn_entity(ent)
{
	if (!pev_valid(ent))
		return FMRES_IGNORED
	new classname[32]
	pev(ent, pev_classname, classname, charsmax(classname))
	for (new i = 0; i < sizeof OBJECTIVES; i++)
	{
		if (equal(classname, OBJECTIVES[i]))
		{
			engfunc(EngFunc_RemoveEntity, ent)
			return FMRES_SUPERCEDE
		}
	}
	return FMRES_IGNORED
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	unregister_forward(FM_Spawn, g_FwSpawn)

	g_pDelay   = register_cvar("gmdm_respawn_delay", "1.5")
	g_pProtect = register_cvar("gmdm_protect", "2")
	g_pRandom  = register_cvar("gmdm_random_spawns", "1")
	g_pGrenade = register_cvar("gmdm_grenade", "1")
	g_pGuns    = register_cvar("gmdm_guns", "0")
	g_pFreeForAll = get_cvar_pointer("mp_freeforall")

	RegisterHam(Ham_Spawn, "player", "fw_player_spawn", 1)
	RegisterHam(Ham_Killed, "player", "fw_player_killed", 1)
	register_event("TeamInfo", "ev_teaminfo", "a")

	register_clcmd("say /guns", "cmd_guns")
	register_clcmd("say guns", "cmd_guns")
	register_clcmd("say_team /guns", "cmd_guns")

	load_spawns()
}

load_spawns()
{
	new cfgdir[96], map[32], path[160], line[160]
	new p[10][12]
	get_configsdir(cfgdir, charsmax(cfgdir))
	get_mapname(map, charsmax(map))
	formatex(path, charsmax(path), "%s/csdm/%s.spawns.cfg", cfgdir, map)

	new f = fopen(path, "rt")
	if (!f)
	{
		load_nav_spawns(map)
		// no nav yet: the bots are probably learning the map, so use theirs once it's saved
		if (!g_SpawnCount)
		{
			g_NavWaitSize = -1
			set_task(10.0, "task_wait_nav", TASK_NAV_WAIT, _, _, "b")
		}
		return
	}
	while (!feof(f) && g_SpawnCount < MAX_SPAWNS)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue
		// origin x y z, angles p y r, team, v_angle p y r
		if (parse(line, p[0], 11, p[1], 11, p[2], 11, p[3], 11, p[4], 11, p[5], 11, p[6], 11, p[7], 11, p[8], 11, p[9], 11) < 3)
			continue
		for (new i = 0; i < 3; i++)
		{
			g_Spawns[g_SpawnCount][i] = str_to_float(p[i])
			g_SpawnAngles[g_SpawnCount][i] = str_to_float(p[i + 3])
		}
		g_SpawnCount++
	}
	fclose(f)
	log_amx("Loaded %d random spawn points for %s", g_SpawnCount, map)
}

// A map that had no nav when it started: once the bots have saved one (the file is there
// and hasn't grown since the last look), random spawns come from it; team spawns until then
public task_wait_nav()
{
	new map[32], path[96]
	get_mapname(map, charsmax(map))
	formatex(path, charsmax(path), "maps/%s.nav", map)
	if (!file_exists(path, true))
		return
	new size = file_size(path, FSOPT_BYTES_COUNT, true)
	if (size <= 0 || size != g_NavWaitSize)
	{
		g_NavWaitSize = size
		return
	}
	remove_task(TASK_NAV_WAIT)
	g_SpawnCount = 0
	load_nav_spawns(map)
}

// Nav area attributes that make a poor spawn spot
#define NAV_CROUCH  0x01
#define NAV_JUMP    0x02
#define NAV_PRECISE 0x04

new Float:g_Cand[NAV_CANDIDATES][3]
new Float:g_Near[NAV_CANDIDATES]

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

// No spawn file: spread spawn points over the walkable areas the bots mapped
load_nav_spawns(const map[])
{
	new path[96]
	formatex(path, charsmax(path), "maps/%s.nav", map)
	new f = fopen(path, "rb", true)
	if (!f)
		return

	new version
	if (read_u32(f) != 0xFEEDFACE || (version = read_u32(f)) > 5)
	{
		fclose(f)
		return
	}
	if (version >= 4)
		read_u32(f)                                  // bsp size
	if (version >= 5)
		for (new n = read_u16(f); n > 0; n--)        // place names
			skip(f, read_u16(f))

	new areas = read_u32(f)
	// an empty or broken file (a huge count reads as negative): nothing to pick from
	if (areas <= 0)
	{
		fclose(f)
		return
	}
	new stride = areas / NAV_CANDIDATES + 1
	new count, Float:lo[3], Float:hi[3]
	for (new a = 0; a < areas && !feof(f); a++)
	{
		read_u32(f)                                  // id
		new flags = read_u8(f)
		for (new i = 0; i < 3; i++) lo[i] = read_float(f)
		for (new i = 0; i < 3; i++) hi[i] = read_float(f)
		skip(f, 8)                                   // corner heights
		for (new d = 0; d < 4; d++)
			skip(f, read_u32(f) * 4)                 // connections
		skip(f, read_u8(f) * (version == 1 ? 12 : 17))   // hiding spots
		skip(f, read_u8(f) * 14)                     // approach areas
		for (new n = read_u32(f); n > 0; n--)        // encounter paths
		{
			skip(f, 10)
			skip(f, read_u8(f) * 5)
		}
		if (version >= 5)
			read_u16(f)                              // place

		if (flags & (NAV_CROUCH | NAV_JUMP | NAV_PRECISE) || a % stride)
			continue
		if (floatabs(hi[0] - lo[0]) < 32.0 || floatabs(hi[1] - lo[1]) < 32.0 || count >= NAV_CANDIDATES)
			continue
		g_Cand[count][0] = (lo[0] + hi[0]) / 2.0
		g_Cand[count][1] = (lo[1] + hi[1]) / 2.0
		g_Cand[count][2] = floatmax(lo[2], hi[2]) + 37.0   // player origin sits above the floor
		count++
	}
	fclose(f)
	if (count < 4)
		return

	// Farthest-point sampling, so spawns cover the whole map rather than clumping
	new pick = random(count)
	for (new i = 0; i < count; i++)
		g_Near[i] = 999999.0
	while (g_SpawnCount < NAV_SPAWNS && g_SpawnCount < count)
	{
		g_Spawns[g_SpawnCount] = g_Cand[pick]
		g_SpawnAngles[g_SpawnCount][1] = random_float(-180.0, 180.0)
		g_SpawnCount++
		new Float:far = -1.0, next = -1
		for (new i = 0; i < count; i++)
		{
			g_Near[i] = floatmin(g_Near[i], get_distance_f(g_Cand[i], g_Cand[pick]))
			if (g_Near[i] > far)
			{
				far = g_Near[i]
				next = i
			}
		}
		if (next == -1 || far < 64.0)
			break
		pick = next
	}
	log_amx("Picked %d random spawn points for %s from its bot nav file", g_SpawnCount, map)
}

// Buying is replaced by the gun menu
public CS_OnBuyAttempt(id, item)
{
	return PLUGIN_HANDLED
}

public client_putinserver(id)
{
	g_Primary[id] = g_Secondary[id] = -1
	g_Remember[id] = false
	if (!g_BotHams && is_user_bot(id) && get_cvar_pointer("bot_quota"))
		set_task(0.1, "task_bot_hams", id)
}

// Condition Zero bots are a different class from human players, so hooks
// registered on "player" never fire for them. Register them again on the first
// bot that joins.
public task_bot_hams(id)
{
	// the slot may have gone to a person in the meantime
	if (g_BotHams || !is_user_connected(id) || !is_user_bot(id))
		return
	g_BotHams = true
	RegisterHamFromEntity(Ham_Spawn, id, "fw_player_spawn", 1)
	RegisterHamFromEntity(Ham_Killed, id, "fw_player_killed", 1)
	// Bots already dead when the hooks arrive still need bringing back
	if (!is_user_alive(id))
		queue_respawn(id)
}

public client_disconnected(id)
{
	remove_task(id + TASK_RESPAWN)
	remove_task(id + TASK_PROTECT)
	remove_task(id + TASK_EQUIP)
}

// A player who picks a team mid-round should not have to wait for the next round
public ev_teaminfo()
{
	new id = read_data(1)
	if (is_user_connected(id) && !is_user_alive(id))
		queue_respawn(id)
}

public fw_player_killed(victim, attacker, gib)
{
	// Clean up the weapons the victim dropped. The box's own think (Kill) also
	// frees the guns inside it; deleting the box directly leaked them, and with
	// constant respawns the server ran out of edicts ("ED_Alloc: no free edicts")
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "weaponbox")) > 0)
		if (pev(ent, pev_owner) == victim)
			set_pev(ent, pev_nextthink, get_gametime() + 0.1)
	queue_respawn(victim)
}

queue_respawn(id)
{
	remove_task(id + TASK_RESPAWN)
	set_task(floatmax(get_pcvar_float(g_pDelay), 0.1), "task_respawn", id + TASK_RESPAWN)
}

public task_respawn(taskid)
{
	new id = taskid - TASK_RESPAWN
	if (!is_user_connected(id) || is_user_alive(id))
		return
	new CsTeams:team = cs_get_user_team(id)
	if (team == CS_TEAM_T || team == CS_TEAM_CT)
		ExecuteHamB(Ham_CS_RoundRespawn, id)
}

public fw_player_spawn(id)
{
	if (!is_user_alive(id))
		return

	if (g_SpawnCount && get_pcvar_num(g_pRandom))
		random_spawn(id)

	new Float:protect = get_pcvar_float(g_pProtect)
	if (protect > 0.0)
	{
		set_user_godmode(id, 1)
		if (free_for_all())
			set_user_rendering(id, kRenderFxGlowShell, 230, 220, 190, kRenderNormal, 12)
		else if (cs_get_user_team(id) == CS_TEAM_T)
			set_user_rendering(id, kRenderFxGlowShell, 200, 60, 40, kRenderNormal, 12)
		else
			set_user_rendering(id, kRenderFxGlowShell, 40, 110, 220, kRenderNormal, 12)
		remove_task(id + TASK_PROTECT)
		set_task(protect, "task_unprotect", id + TASK_PROTECT)
	}
	cs_set_user_money(id, 0, 0)
	remove_task(id + TASK_EQUIP)
	set_task(0.1, "task_equip", id + TASK_EQUIP)
}

public task_unprotect(taskid)
{
	new id = taskid - TASK_PROTECT
	if (!is_user_connected(id))
		return
	set_user_godmode(id, 0)
	set_user_rendering(id)
}

// ReGameDLL's mp_freeforall: everyone is an enemy
bool:free_for_all()
{
	return g_pFreeForAll && get_pcvar_num(g_pFreeForAll) != 0
}

// Pick a free spawn point away from enemies
random_spawn(id)
{
	new Float:mins[3], Float:origin[3]
	pev(id, pev_mins, mins)
	new hull = (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN
	new myteam = free_for_all() ? -1 : get_user_team(id)

	new start = random(g_SpawnCount)
	new best = -1, Float:bestDist = -1.0
	for (new n = 0; n < g_SpawnCount; n++)
	{
		new i = (start + n) % g_SpawnCount
		origin = g_Spawns[i]

		engfunc(EngFunc_TraceHull, origin, origin, 0, hull, id, 0)
		if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid) || !get_tr2(0, TR_InOpen))
			continue

		// distance to the closest living enemy
		new Float:closest = 99999.0
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == id || !is_user_alive(p) || get_user_team(p) == myteam)
				continue
			new Float:other[3]
			pev(p, pev_origin, other)
			closest = floatmin(closest, get_distance_f(origin, other))
		}
		if (closest > 600.0)
		{
			best = i
			break
		}
		if (closest > bestDist)
		{
			bestDist = closest
			best = i
		}
	}
	if (best == -1)
		return

	engfunc(EngFunc_SetOrigin, id, g_Spawns[best])
	set_pev(id, pev_velocity, Float:{ 0.0, 0.0, 0.0 })
	set_pev(id, pev_angles, g_SpawnAngles[best])
	set_pev(id, pev_v_angle, g_SpawnAngles[best])
	set_pev(id, pev_fixangle, 1)
}

public task_equip(taskid)
{
	new id = taskid - TASK_EQUIP
	if (!is_user_alive(id))
		return

	if (is_user_bot(id))
	{
		give_loadout(id, bot_primary(), random(2))
		return
	}
	if (guns_kind() == GUNS_PISTOLS && g_Remember[id] && g_Secondary[id] >= 0)
		give_loadout(id, -1, g_Secondary[id])
	else if (g_Remember[id] && g_Primary[id] >= 0 && primary_allowed(g_Primary[id]))
		give_loadout(id, g_Primary[id], g_Secondary[id])
	else
	{
		// knife and pistol while the menu is open
		strip_user_weapons(id)
		give_item(id, "weapon_knife")
		give_item(id, SECONDARY[0][0])
		cs_set_user_bpammo(id, CSW_DEAGLE, MAXBP[CSW_DEAGLE])
		show_primary_menu(id)
	}
}

guns_kind()
{
	return clamp(get_pcvar_num(g_pGuns), GUNS_ALL, GUNS_PISTOLS)
}

bool:primary_allowed(i)
{
	new kind = guns_kind()
	return kind == GUNS_ALL || PRIMARY_KIND[i] == kind
}

bot_primary()
{
	if (guns_kind() == GUNS_PISTOLS)
		return -1
	new pick = BOT_PRIMARY[random(sizeof BOT_PRIMARY)]
	if (primary_allowed(pick))
		return pick
	new allowed[sizeof PRIMARY], n
	for (new i = 0; i < sizeof PRIMARY; i++)
		if (primary_allowed(i))
			allowed[n++] = i
	return n ? allowed[random(n)] : -1
}

give_loadout(id, primary, secondary)
{
	if (primary >= 0 && !primary_allowed(primary))
		primary = -1
	strip_user_weapons(id)
	give_item(id, "weapon_knife")
	cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM)
	if (secondary >= 0)
		give_full(id, SECONDARY[secondary][0])
	if (primary >= 0)
		give_full(id, PRIMARY[primary][0])
	if (get_pcvar_num(g_pGrenade))
		give_item(id, "weapon_hegrenade")
}

give_full(id, const weapon[])
{
	give_item(id, weapon)
	new csw = get_weaponid(weapon)
	if (0 < csw < sizeof MAXBP && MAXBP[csw])
		cs_set_user_bpammo(id, csw, MAXBP[csw])
}

public cmd_guns(id)
{
	g_Remember[id] = false
	if (is_user_alive(id))
		show_primary_menu(id)
	else
		client_print(id, print_chat, "[Deathmatch] The gun menu opens when you respawn.")
	return PLUGIN_HANDLED
}

show_primary_menu(id)
{
	if (guns_kind() == GUNS_PISTOLS)
	{
		g_Primary[id] = -1
		show_secondary_menu(id)
		return
	}
	new menu = menu_create("\yChoose your gun", "primary_handler")
	new info[4]
	if (g_Primary[id] >= 0 && g_Secondary[id] >= 0 && primary_allowed(g_Primary[id]))
	{
		menu_additem(menu, fmt("Same as last time \d(%s + %s)", PRIMARY[g_Primary[id]][1], SECONDARY[g_Secondary[id]][1]), "L")
		menu_additem(menu, "Same every time \d(say /guns to change)", "A")
	}
	for (new i = 0; i < sizeof PRIMARY; i++)
	{
		if (!primary_allowed(i))
			continue
		num_to_str(i, info, charsmax(info))
		menu_additem(menu, PRIMARY[i][1], info)
	}
	menu_setprop(menu, MPROP_BACKNAME, "Previous page")
	menu_setprop(menu, MPROP_NEXTNAME, "Next page")
	menu_setprop(menu, MPROP_EXITNAME, "Close")
	menu_display(id, menu)
}

public primary_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[4], dummy
	menu_item_getinfo(menu, item, dummy, info, charsmax(info), _, _, dummy)
	menu_destroy(menu)

	if (info[0] == 'L' || info[0] == 'A')
	{
		g_Remember[id] = info[0] == 'A'
		if (is_user_alive(id))
			give_loadout(id, g_Primary[id], g_Secondary[id])
		return PLUGIN_HANDLED
	}
	g_Primary[id] = str_to_num(info)
	show_secondary_menu(id)
	return PLUGIN_HANDLED
}

show_secondary_menu(id)
{
	new menu = menu_create("\yChoose your pistol", "secondary_handler")
	new info[4]
	for (new i = 0; i < sizeof SECONDARY; i++)
	{
		num_to_str(i, info, charsmax(info))
		menu_additem(menu, SECONDARY[i][1], info)
	}
	menu_setprop(menu, MPROP_EXITNAME, "Close")
	menu_display(id, menu)
}

public secondary_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[4], dummy
	menu_item_getinfo(menu, item, dummy, info, charsmax(info), _, _, dummy)
	menu_destroy(menu)

	g_Secondary[id] = str_to_num(info)
	if (guns_kind() == GUNS_PISTOLS)
		g_Remember[id] = true           // no primary to pick; say /guns changes the pistol
	if (is_user_alive(id))
		give_loadout(id, g_Primary[id], g_Secondary[id])
	return PLUGIN_HANDLED
}
