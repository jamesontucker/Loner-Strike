/*
	gm_content: a server's own weapon skins and player models, and its precache count.

	A player's game never downloads a file it already has, so a server can't change
	models/v_ak47.mdl for anyone. Loner-Strike puts a server's skins and player models in
	under their own names instead (models/ls/<id>/v_ak47.mdl, models/player/ls<id>/...)
	and lists them in data/gamemodes/content.ini; this loads them, so players download
	them when they join, and swaps them in:
	    weapon <weapon_x> <who> <v model or -> <p model or -> <w model or ->
	    player <who> <model name>
	who: all, ct, t or admin. A weapon's view and player models change when it's drawn;
	its world model (on the ground) only for "all". Player models are set at each spawn.
	Modes with their own models win: nothing is swapped in the zombie modes, Base Builder,
	Paintball or Hide and Seek, and Superhero and Warcraft 3 keep their own player models
	(heroes and races that change how you look) but still get the weapon skins.

	The engine can load 512 models, 512 sounds and 512 other files a map (more stops the
	server). Each map start this counts what was loaded, the map's own pieces included
	(slots are numbered in order, so the highest number given out is the count), and
	writes data/gamemodes/content_status.json for the app.
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fakemeta>
#include <hamsandwich>

#define PLUGIN  "Server skins and models"
#define VERSION "1.0"
#define AUTHOR  "Loner-Strike"

#define MAX_ITEMS 64
// m_pPlayer on a weapon (Linux needs 4 more)
#define OFFSET_WEAPON_OWNER 41
#define LINUX_DIFF_WEAPON 4

enum { WHO_ALL, WHO_CT, WHO_T, WHO_ADMIN }

new g_WeaponClass[MAX_ITEMS][32], g_WeaponWho[MAX_ITEMS], g_V[MAX_ITEMS][96], g_P[MAX_ITEMS][96], g_W[MAX_ITEMS][96], g_WOriginal[MAX_ITEMS][64]
new g_Weapons
new g_PlayerModel[MAX_ITEMS][32], g_PlayerWho[MAX_ITEMS]
new g_Players
new bool:g_Off, bool:g_NoPlayers
new g_MaxModel, g_MaxSound, g_MaxGeneric

public plugin_precache()
{
	// the count: every load from here on gives out the next slot number
	register_forward(FM_PrecacheModel, "fw_PrecacheModel", 1)
	register_forward(FM_PrecacheSound, "fw_PrecacheSound", 1)
	register_forward(FM_PrecacheGeneric, "fw_PrecacheGeneric", 1)

	new mode[32]
	read_data_line("current.txt", mode, charsmax(mode))
	g_Off = equal(mode, "zombie") || equal(mode, "zp5") || equal(mode, "basebuilder") || equal(mode, "paintball") || equal(mode, "hns")
	g_NoPlayers = equal(mode, "shero") || equal(mode, "war3")
	if (g_Off)
		return

	new path[128], dir[64]
	get_datadir(dir, charsmax(dir))
	formatex(path, charsmax(path), "%s/gamemodes/content.ini", dir)
	new f = fopen(path, "rt")
	if (!f)
		return
	new line[512], kind[16], a[96], b[96], c[96], d[96], e[96]
	while (fgets(f, line, charsmax(line)))
	{
		trim(line)
		if (!line[0] || line[0] == ';')
			continue
		kind[0] = a[0] = b[0] = c[0] = d[0] = e[0] = 0
		parse(line, kind, charsmax(kind), a, charsmax(a), b, charsmax(b), c, charsmax(c), d, charsmax(d), e, charsmax(e))
		if (equal(kind, "weapon") && g_Weapons < MAX_ITEMS && equal(a, "weapon_", 7))
		{
			new i = g_Weapons
			copy(g_WeaponClass[i], charsmax(g_WeaponClass[]), a)
			g_WeaponWho[i] = who_of(b)
			g_V[i][0] = g_P[i][0] = g_W[i][0] = g_WOriginal[i][0] = 0
			if (model_ok(c)) { copy(g_V[i], charsmax(g_V[]), c); precache_model(c); }
			if (model_ok(d)) { copy(g_P[i], charsmax(g_P[]), d); precache_model(d); }
			if (model_ok(e))
			{
				copy(g_W[i], charsmax(g_W[]), e)
				precache_model(e)
				// the game's own world model it stands in for: the same file name in models/
				new slash = -1, k
				for (k = 0; e[k]; k++)
					if (e[k] == '/')
						slash = k
				formatex(g_WOriginal[i], charsmax(g_WOriginal[]), "models/%s", e[slash + 1])
			}
			if (g_V[i][0] || g_P[i][0] || g_W[i][0])
				g_Weapons++
		}
		else if (equal(kind, "player") && !g_NoPlayers && g_Players < MAX_ITEMS && a[0] && name_ok(b))
		{
			new file[96]
			formatex(file, charsmax(file), "models/player/%s/%s.mdl", b, b)
			if (!file_exists(file, true))
				continue
			precache_model(file)
			copy(g_PlayerModel[g_Players], charsmax(g_PlayerModel[]), b)
			g_PlayerWho[g_Players] = who_of(a)
			g_Players++
		}
	}
	fclose(f)
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	if (g_Off)
		return
	// one hook per weapon that has a view or player model
	new done[MAX_ITEMS][32], n
	for (new i = 0; i < g_Weapons; i++)
	{
		if (!g_V[i][0] && !g_P[i][0])
			continue
		new seen
		for (new j = 0; j < n; j++)
			if (equal(done[j], g_WeaponClass[i]))
				seen = 1
		if (seen)
			continue
		copy(done[n++], charsmax(done[]), g_WeaponClass[i])
		RegisterHam(Ham_Item_Deploy, g_WeaponClass[i], "fw_Deploy", 1)
	}
	for (new i = 0; i < g_Weapons; i++)
	{
		if (g_W[i][0] && g_WeaponWho[i] == WHO_ALL)
		{
			register_forward(FM_SetModel, "fw_SetModel")
			break
		}
	}
	if (g_Players)
		RegisterHam(Ham_Spawn, "player", "fw_Spawn", 1)
}

public plugin_cfg()
{
	// everything a map loads is loaded by now
	new path[128], dir[64], map[32]
	get_datadir(dir, charsmax(dir))
	get_mapname(map, charsmax(map))
	formatex(path, charsmax(path), "%s/gamemodes/content_status.json", dir)
	new f = fopen(path, "wt")
	if (!f)
		return
	fprintf(f, "{^"map^": ^"%s^", ^"models^": %d, ^"sounds^": %d, ^"generic^": %d, ^"time^": %d}^n", map, g_MaxModel, g_MaxSound, g_MaxGeneric, get_systime())
	fclose(f)
}

public fw_PrecacheModel(const model[])
{
	new n = get_orig_retval()
	if (n > g_MaxModel)
		g_MaxModel = n
}

public fw_PrecacheSound(const sound[])
{
	new n = get_orig_retval()
	if (n > g_MaxSound)
		g_MaxSound = n
}

public fw_PrecacheGeneric(const file[])
{
	new n = get_orig_retval()
	if (n > g_MaxGeneric)
		g_MaxGeneric = n
}

public fw_Deploy(weapon)
{
	if (!pev_valid(weapon))
		return HAM_IGNORED
	new id = get_pdata_cbase(weapon, OFFSET_WEAPON_OWNER, LINUX_DIFF_WEAPON)
	if (!is_user_alive(id))
		return HAM_IGNORED
	new class[32]
	pev(weapon, pev_classname, class, charsmax(class))
	for (new i = 0; i < g_Weapons; i++)
	{
		if (!equal(g_WeaponClass[i], class) || !(g_V[i][0] || g_P[i][0]) || !who_fits(id, g_WeaponWho[i]))
			continue
		if (g_V[i][0])
			set_pev(id, pev_viewmodel2, g_V[i])
		if (g_P[i][0])
			set_pev(id, pev_weaponmodel2, g_P[i])
		break
	}
	return HAM_IGNORED
}

public fw_SetModel(ent, const model[])
{
	if (!pev_valid(ent) || !equal(model, "models/w_", 9))
		return FMRES_IGNORED
	for (new i = 0; i < g_Weapons; i++)
	{
		if (g_W[i][0] && g_WeaponWho[i] == WHO_ALL && equal(model, g_WOriginal[i]))
		{
			engfunc(EngFunc_SetModel, ent, g_W[i])
			return FMRES_SUPERCEDE
		}
	}
	return FMRES_IGNORED
}

public fw_Spawn(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	for (new i = 0; i < g_Players; i++)
	{
		if (who_fits(id, g_PlayerWho[i]))
		{
			cs_set_user_model(id, g_PlayerModel[i])
			break
		}
	}
	return HAM_IGNORED
}

who_of(const s[])
{
	if (equal(s, "ct"))
		return WHO_CT
	if (equal(s, "t"))
		return WHO_T
	if (equal(s, "admin"))
		return WHO_ADMIN
	return WHO_ALL
}

bool:who_fits(id, who)
{
	switch (who)
	{
		case WHO_CT: return cs_get_user_team(id) == CS_TEAM_CT
		case WHO_T: return cs_get_user_team(id) == CS_TEAM_T
		case WHO_ADMIN: return is_user_admin(id) != 0
	}
	return true
}

// a model the list may name: under models/, an .mdl, there on the disk, plain characters
bool:model_ok(const m[])
{
	if (!m[0] || equal(m, "-") || !equal(m, "models/", 7) || contain(m, "..") != -1)
		return false
	new len = strlen(m)
	if (len < 5 || !equali(m[len - 4], ".mdl"))
		return false
	return bool:file_exists(m, true)
}

bool:name_ok(const n[])
{
	if (!n[0])
		return false
	for (new i = 0; n[i]; i++)
		if (!isalnum(n[i]) && n[i] != '_' && n[i] != '-')
			return false
	return true
}

// a one-line file in data/gamemodes
read_data_line(const name[], out[], len)
{
	out[0] = 0
	new path[128], dir[64]
	get_datadir(dir, charsmax(dir))
	formatex(path, charsmax(path), "%s/gamemodes/%s", dir, name)
	new f = fopen(path, "rt")
	if (!f)
		return
	fgets(f, out, len)
	fclose(f)
	trim(out)
}
