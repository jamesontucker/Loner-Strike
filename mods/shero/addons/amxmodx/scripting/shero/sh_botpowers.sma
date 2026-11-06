/* SuperHero Mod - Bot Powers (for CZ bots / any fake clients)
 *
 * - Bots pick heroes for every free slot (done by the patched core on spawn; this
 *   plugin also triggers a pick as soon as a bot levels up mid-round).
 * - Heroes listed as "skip" in configs/shero/shbots.ini are never picked by bots.
 * - Bots press their +power keys for bind heroes according to shbots.ini:
 *     aim  - an enemy is in (or very near) the bot's crosshair within <range>
 *     near - a visible enemy is within <range> in front of the bot
 *     any  - at random moments while alive (no enemy needed, not in combat)
 *   <chance> is the probability per second of pressing while the condition holds,
 *   the key is held for a random time between <holdmin> and <holdmax> seconds.
 *
 * Cvars:
 *   sh_bot_powers 1      - 0 = bots never press power keys (they still pick heroes)
 *   sh_bot_powerdelay 3  - min seconds between two presses of the same key
 */

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>
#include <superheromod>
#include <sh_botsupport>

#define MODE_DEFAULT	0
#define MODE_SKIP	1
#define MODE_AIM	2
#define MODE_NEAR	3
#define MODE_ANY	4
#define MODE_NEVER	5

#define THINK_INTERVAL	0.25
#define MAXKEYS		3

new gMode[SH_MAXHEROS]
new Float:gRange[SH_MAXHEROS]
new Float:gChance[SH_MAXHEROS]
new Float:gHoldMin[SH_MAXHEROS]
new Float:gHoldMax[SH_MAXHEROS]

new bool:gHeld[SH_MAXSLOTS+1][MAXKEYS+1]
new Float:gReleaseAt[SH_MAXSLOTS+1][MAXKEYS+1]
new Float:gNextTry[SH_MAXSLOTS+1][MAXKEYS+1]
new gLastLevel[SH_MAXSLOTS+1]

new pCvarEnable, pCvarDelay
new gMaxPlayers

public plugin_init()
{
	register_plugin("SuperHero Bot Powers", "1.0", "CZ panel")
	pCvarEnable = register_cvar("sh_bot_powers", "1")
	pCvarDelay = register_cvar("sh_bot_powerdelay", "3")
	gMaxPlayers = get_maxplayers()
	register_concmd("sh_botconfig_reload", "cmdReload", ADMIN_RCON, "- reloads configs/shero/shbots.ini")
}

public plugin_cfg()
{
	// All heroes are registered in plugin_init, so they exist now
	loadBotConfig()
	set_task(THINK_INTERVAL, "botThink", _, _, _, "b")
}

public cmdReload(id, level, cid)
{
	if ( !cmd_access(id, level, cid, 1) ) return PLUGIN_HANDLED
	loadBotConfig()
	console_print(id, "[SH] shbots.ini reloaded")
	return PLUGIN_HANDLED
}

loadBotConfig()
{
	new numHeroes = min(sh_get_num_heroes(), SH_MAXHEROS)
	for ( new h = 0; h < numHeroes; h++ ) {
		gMode[h] = MODE_DEFAULT
		gRange[h] = 800.0
		gChance[h] = 0.5
		gHoldMin[h] = 0.2
		gHoldMax[h] = 0.6
		sh_set_hero_botblock(h, false)
	}

	new path[128]
	get_configsdir(path, charsmax(path))
	add(path, charsmax(path), "/shero/shbots.ini")

	new fp = fopen(path, "rt")
	if ( !fp ) {
		log_amx("[SH] %s not found, using defaults for bot powers", path)
		return
	}

	new line[192], name[32], mode[16], range[12], chance[12], hmin[12], hmax[12]
	while ( !feof(fp) ) {
		fgets(fp, line, charsmax(line))
		trim(line)
		if ( !line[0] || line[0] == ';' || (line[0] == '/' && line[1] == '/') ) continue

		name[0] = mode[0] = range[0] = chance[0] = hmin[0] = hmax[0] = 0
		parse(line, name, charsmax(name), mode, charsmax(mode), range, charsmax(range), chance, charsmax(chance), hmin, charsmax(hmin), hmax, charsmax(hmax))

		new h = sh_get_hero_id(name)
		if ( h < 0 || h >= numHeroes ) continue	// hero not loaded in this mode

		if ( equali(mode, "skip") ) gMode[h] = MODE_SKIP
		else if ( equali(mode, "aim") ) gMode[h] = MODE_AIM
		else if ( equali(mode, "near") ) gMode[h] = MODE_NEAR
		else if ( equali(mode, "any") ) gMode[h] = MODE_ANY
		else if ( equali(mode, "never") ) gMode[h] = MODE_NEVER
		else {
			log_amx("[SH] shbots.ini: unknown mode ^"%s^" for %s", mode, name)
			continue
		}

		if ( range[0] ) gRange[h] = str_to_float(range)
		if ( chance[0] ) gChance[h] = str_to_float(chance)
		if ( hmin[0] ) gHoldMin[h] = str_to_float(hmin)
		if ( hmax[0] ) gHoldMax[h] = str_to_float(hmax)
		if ( gHoldMax[h] < gHoldMin[h] ) gHoldMax[h] = gHoldMin[h]

		if ( gMode[h] == MODE_SKIP ) sh_set_hero_botblock(h, true)
	}
	fclose(fp)
}

public client_putinserver(id)
{
	resetPlayer(id)
	gLastLevel[id] = -1
}

public client_disconnected(id)
{
	resetPlayer(id)
}

resetPlayer(id)
{
	for ( new k = 0; k <= MAXKEYS; k++ ) {
		gHeld[id][k] = false
		gReleaseAt[id][k] = 0.0
		gNextTry[id][k] = 0.0
	}
}

public sh_client_spawn(id, bool:newRound)
{
	if ( id < 1 || id > gMaxPlayers ) return
	new Float:now = get_gametime()
	for ( new k = 0; k <= MAXKEYS; k++ ) {
		gHeld[id][k] = false
		// don't fire powers in the first seconds after spawning
		gNextTry[id][k] = now + random_float(2.0, 5.0)
	}
}

public botThink()
{
	if ( !sh_is_active() ) return

	new Float:now = get_gametime()
	new bool:enabled = get_pcvar_num(pCvarEnable) != 0
	new bool:canUse = sh_is_inround() && !sh_is_freezetime()

	for ( new id = 1; id <= gMaxPlayers; id++ ) {
		if ( !is_user_connected(id) || !is_user_bot(id) ) continue

		new bool:alive = is_user_alive(id) ? true : false

		// release keys that are held too long or when the bot died / round ended
		for ( new k = 1; k <= MAXKEYS; k++ ) {
			if ( gHeld[id][k] && (!alive || !canUse || !enabled || now >= gReleaseAt[id][k]) ) {
				gHeld[id][k] = false
				sh_bot_power_key(id, k, 0)
				gNextTry[id][k] = now + get_pcvar_float(pCvarDelay) + random_float(0.0, 2.0)
			}
		}

		if ( !alive ) continue

		// pick heroes right away after a level up
		new lvl = sh_get_user_lvl(id)
		if ( lvl != gLastLevel[id] ) {
			if ( gLastLevel[id] != -1 && lvl > gLastLevel[id] ) sh_bot_pick_powers(id)
			gLastLevel[id] = lvl
		}

		if ( !enabled || !canUse ) continue

		for ( new k = 1; k <= MAXKEYS; k++ ) {
			if ( gHeld[id][k] || now < gNextTry[id][k] ) continue

			new h = sh_get_user_bind(id, k)
			if ( h < 0 || h >= SH_MAXHEROS ) continue

			new mode = gMode[h]
			if ( mode == MODE_DEFAULT ) mode = MODE_NEAR
			if ( mode == MODE_SKIP || mode == MODE_NEVER ) continue

			new bool:want = false
			switch ( mode ) {
				case MODE_AIM: want = enemyInCrosshair(id, gRange[h])
				case MODE_NEAR: want = enemyNear(id, gRange[h])
				case MODE_ANY: want = !enemyNear(id, 1500.0)
			}
			if ( !want ) continue

			// chance is per second, we think every THINK_INTERVAL seconds
			if ( random_float(0.0, 1.0) > gChance[h] * THINK_INTERVAL ) continue

			gHeld[id][k] = true
			gReleaseAt[id][k] = now + random_float(gHoldMin[h], gHoldMax[h])
			sh_bot_power_key(id, k, 1)
		}
	}
}

bool:isEnemy(id, other)
{
	if ( other == id || other < 1 || other > gMaxPlayers ) return false
	if ( !is_user_alive(other) ) return false
	if ( get_user_team(other) == get_user_team(id) ) return false
	return true
}

bool:canSee(id, other)
{
	new Float:start[3], Float:end[3], Float:ofs[3]
	pev(id, pev_origin, start)
	pev(id, pev_view_ofs, ofs)
	start[0] += ofs[0]; start[1] += ofs[1]; start[2] += ofs[2]
	pev(other, pev_origin, end)

	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, start, end, IGNORE_MONSTERS, id, tr)
	new Float:frac
	get_tr2(tr, TR_flFraction, frac)
	free_tr2(tr)
	return frac >= 0.99
}

// angle (degrees) between where id is looking and the direction to other, plus distance
Float:aimAngle(id, other, &Float:dist)
{
	new Float:start[3], Float:end[3], Float:ofs[3], Float:ang[3], Float:fwd[3], Float:dir[3]
	pev(id, pev_origin, start)
	pev(id, pev_view_ofs, ofs)
	start[0] += ofs[0]; start[1] += ofs[1]; start[2] += ofs[2]
	pev(other, pev_origin, end)

	dir[0] = end[0] - start[0]; dir[1] = end[1] - start[1]; dir[2] = end[2] - start[2]
	dist = floatsqroot(dir[0]*dir[0] + dir[1]*dir[1] + dir[2]*dir[2])
	if ( dist < 1.0 ) return 0.0

	pev(id, pev_v_angle, ang)
	engfunc(EngFunc_MakeVectors, ang)
	global_get(glb_v_forward, fwd)

	new Float:dot = (fwd[0]*dir[0] + fwd[1]*dir[1] + fwd[2]*dir[2]) / dist
	if ( dot > 1.0 ) dot = 1.0
	else if ( dot < -1.0 ) dot = -1.0
	return floatacos(dot, degrees)
}

bool:enemyInCrosshair(id, Float:range)
{
	new target, body
	get_user_aiming(id, target, body, floatround(range))
	if ( isEnemy(id, target) ) return true

	// bots don't always have the crosshair exactly on the body: allow a small cone
	new Float:dist
	for ( new e = 1; e <= gMaxPlayers; e++ ) {
		if ( !isEnemy(id, e) ) continue
		if ( aimAngle(id, e, dist) > 8.0 || dist > range ) continue
		if ( canSee(id, e) ) return true
	}
	return false
}

bool:enemyNear(id, Float:range)
{
	new Float:dist
	for ( new e = 1; e <= gMaxPlayers; e++ ) {
		if ( !isEnemy(id, e) ) continue
		if ( aimAngle(id, e, dist) > 70.0 || dist > range ) continue
		if ( canSee(id, e) ) return true
	}
	return false
}
