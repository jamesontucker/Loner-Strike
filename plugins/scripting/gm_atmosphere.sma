/*
	gm_atmosphere - the small things around the game that made a 2004 pub feel like one.

	Adverts:    every few minutes one of configs/gamemodes/adverts.ini in chat, the way
	            servers ran their rules and "type /rank" lines (gm_atmo_adverts 0/1)
	Spectator:  now and then (not every map) a bot sits a few rounds out in spectator, the
	            way someone did who'd gone to get food, then joins again (gm_atmo_spec 0/1).
	            Only in modes where one player fewer doesn't matter (not Hide and Seek,
	            Deathrun, zombies, Base Builder and the like), and never from a team of
	            fewer than four

	Nothing happens in a match or a career game.
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fun>

#define PLUGIN  "Atmosphere"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TASK_ADVERT  8800
#define TASK_SPEC    8810
#define MAX_ADVERTS  32

new g_pAdverts, g_pSpec
new Array:g_Adverts, g_AdvertNext
new bool:g_Career
new g_Spec, CsTeams:g_SpecTeam, g_SpecUser
new bool:g_SpecMode                  // a mode where a player sitting out fits

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pAdverts = register_cvar("gm_atmo_adverts", "1")
	g_pSpec    = register_cvar("gm_atmo_spec", "1")
	g_Adverts = ArrayCreate(160)
	load_adverts()
	set_task(random_float(90.0, 180.0), "task_advert", TASK_ADVERT)
	// a spectator on about one map in four, some minutes in
	if (random_num(1, 4) == 1)
		set_task(random_float(180.0, 600.0), "task_spec_out", TASK_SPEC)
}

public plugin_cfg()
{
	// career (Tour of Duty) keeps its designed bots and texts
	g_Career = get_cvar_num("mp_career") != 0
	// modes built around the teams as they are (hiders and seekers, a deathrun's terrorist,
	// zombies, builders) or where nobody's really playing a round: no spectator
	new mode[32], path[128]
	get_datadir(path, charsmax(path))
	new f = fopen(fmt("%s/gamemodes/current.txt", path), "rt")
	if (f)
	{
		fgets(f, mode, charsmax(mode))
		trim(mode)
		fclose(f)
	}
	static const NO_SPEC[][] = { "hns", "deathrun", "zombie", "zp5", "basebuilder", "kreedz", "surf", "soccerjam", "match" }
	g_SpecMode = true
	for (new i = 0; i < sizeof NO_SPEC; i++)
		if (equal(mode, NO_SPEC[i]))
			g_SpecMode = false
}

// Players on a team (bots and people)
team_count(CsTeams:team)
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && cs_get_user_team(i) == team)
			n++
	return n
}

bool:quiet()
{
	new phase = get_cvar_num("gm_scrim_phase")
	return g_Career || (phase > 0 && phase < 5)
}

// Someone (not a bot) to see it
bool:players()
{
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i))
			return true
	return false
}

// ---- adverts ------------------------------------------------------------------------------

load_adverts()
{
	new path[160]
	get_configsdir(path, charsmax(path))
	add(path, charsmax(path), "/gamemodes/adverts.ini")
	new f = fopen(path, "rt")
	if (!f)
		return
	new line[160]
	while (!feof(f) && ArraySize(g_Adverts) < MAX_ADVERTS)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (line[0] && line[0] != ';')
			ArrayPushString(g_Adverts, line)
	}
	fclose(f)
	// a different first one each map
	if (ArraySize(g_Adverts))
		g_AdvertNext = random(ArraySize(g_Adverts))
}

public task_advert()
{
	set_task(random_float(150.0, 300.0), "task_advert", TASK_ADVERT)
	new n = ArraySize(g_Adverts)
	if (!get_pcvar_num(g_pAdverts) || !n || quiet() || !players())
		return
	new text[160]
	ArrayGetString(g_Adverts, g_AdvertNext, text, charsmax(text))
	g_AdvertNext = (g_AdvertNext + 1) % n
	new host[64]
	get_cvar_string("hostname", host, charsmax(host))
	replace_all(text, charsmax(text), "%hostname%", host)
	client_print_color(0, print_team_default, "^4[AMXX]^1 %s", text)
}

// ---- a spectator ----------------------------------------------------------------------------

// A bot that's dead (so nothing is cut short) goes to spectator for a few minutes
public task_spec_out()
{
	if (!get_pcvar_num(g_pSpec) || !g_SpecMode || quiet() || g_Spec)
		return
	// (a team left with fewer than three would feel it)
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i) && !is_user_alive(i))
		{
			new CsTeams:t = cs_get_user_team(i)
			if ((t == CS_TEAM_T || t == CS_TEAM_CT) && team_count(t) > 3)
				list[n++] = i
		}
	if (!n)
	{
		// nobody dead right now: try again in a bit
		set_task(20.0, "task_spec_out", TASK_SPEC)
		return
	}
	g_Spec = list[random(n)]
	g_SpecUser = get_user_userid(g_Spec)
	g_SpecTeam = cs_get_user_team(g_Spec)
	cs_set_user_team(g_Spec, CS_TEAM_SPECTATOR)
	set_task(random_float(120.0, 360.0), "task_spec_back", TASK_SPEC)
}

public task_spec_back()
{
	new id = g_Spec
	g_Spec = 0
	if (!id || !is_user_connected(id) || get_user_userid(id) != g_SpecUser || cs_get_user_team(id) != CS_TEAM_SPECTATOR)
		return
	// back on the side it left (it spawns with the next round)
	cs_set_user_team(id, g_SpecTeam)
}

public client_disconnected(id)
{
	if (id == g_Spec)
		g_Spec = 0
}
