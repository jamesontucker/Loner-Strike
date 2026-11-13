/*
 * gm_battleroyale.sma - Battle Royale (GAMEMODES-PLAN.md 3.1), mode id "br".
 *
 * Everyone for themselves (or in squads of br_squad), one round per game. Players spawn spread
 * out at the map's ls_respawn_points with only a knife (br_start), loot crates and floor
 * weapons, and fight while the safe zone (ls_safe_zone + ls_zone_shrinker) closes in, with the
 * round clock counting to the next change. Supply drops fall during the game. The last player
 * or squad standing wins. The first br_drop seconds nobody can hurt anyone ("loot up").
 *
 * Needs gm_components.amxx (listed first in br.plugins.ini) and a map pack
 * (configs/gamemodes/components/<map>.br.ini: shipped for de_aztec, cs_italy, de_dust2,
 * de_inferno, de_train); other maps get a zone round the middle of their spawns.
 *
 * Bots: free-for-all through mp_freeforall; squad mates are friends to the brain
 * (ls_mode role + friend, both ways). gm_components tells them about loot (attract) and the
 * zone (ls_mode zone). This plugin adds a few personal goals, never one for everyone: looters
 * go for a nearby full crate inside the zone now and then, campers settle somewhere inside the
 * next circle each stage, hunters get nothing (the brain hunts and third-parties by noise).
 * Every goal has an end time, so stale ones go.
 */

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>
#include <hamsandwich>
#include <engine>
#include <cstrike>
#include <fun>
#include <loner>

#pragma semicolon 1

#define VERSION "0.1"
#define TASK_TICK  7738100
#define TASK_SETUP 7738200
#define TASK_GEAR  7738300

enum { PH_IDLE = 0, PH_WAIT, PH_DROP, PH_PLAY, PH_OVER };
enum { TR_HUNTER = 0, TR_LOOTER, TR_CAMPER };

new g_cSquad, g_cStart, g_cDrop, g_cZoneStart, g_cDrops, g_cWaitHuman;

new g_phase;
new Float:g_phaseEnd;
new Float:g_playAt;
new bool:g_zoneStarted;
new g_dropsDone;
new Float:g_dropAt[8];
new g_ndrops;
new g_squad[33];
new g_nsquads;
new g_kills[33];
new g_trait[33];
new Float:g_nextGoal[33];
new g_lastStage;
new g_mapname[64];

public plugin_init()
{
	ls_register_mode("br", "Battle Royale", VERSION, "Loner-Strike");

	g_cSquad = register_cvar("br_squad", "1");
	g_cStart = register_cvar("br_start", "knife");
	g_cDrop = register_cvar("br_drop", "20");
	g_cZoneStart = register_cvar("br_zone_start", "40");
	g_cDrops = register_cvar("br_drops", "3");
	g_cWaitHuman = register_cvar("br_wait_human", "1");

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");
	RegisterHamPlayer(Ham_Spawn, "ham_spawn", 1);
	RegisterHamPlayer(Ham_Killed, "ham_killed", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_takedamage");

	register_clcmd("say /br", "cmd_help");
	get_mapname(g_mapname, charsmax(g_mapname));
	set_task(1.0, "tick", TASK_TICK, _, _, "b");
}

public cmd_help(id)
{
	ls_chat(id, "Battle Royale: loot crates (hold USE) and floor weapons, stay inside the zone, be the last one standing.");
	return PLUGIN_HANDLED;
}

/* =========================================================================
 * A game = a round
 * ========================================================================= */

public ev_new_round()
{
	g_phase = PH_IDLE;
	g_zoneStarted = false;
	g_dropsDone = 0;
	g_lastStage = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		g_kills[p] = 0;
		g_squad[p] = 0;
		g_nextGoal[p] = 0.0;
	}
	if (ls_components_running())
		ls_comp_stop(0);
}

public ev_round_start()
{
	remove_task(TASK_SETUP);
	set_task(0.5, "game_setup", TASK_SETUP);
}

public game_setup()
{
	if (!ls_components_running())
	{
		log_amx("Battle Royale needs gm_components.amxx; add it to br.plugins.ini before gm_battleroyale.amxx.");
		return;
	}
	ensure_zone();
	ls_comp_bots_hint("set br");
	ls_comp_bots_hint("goal_clear all 0");
	make_squads();
	pick_traits();
	if (get_pcvar_num(g_cWaitHuman) && !humans())
	{
		g_phase = PH_WAIT;
		return;
	}
	start_drop();
}

stock start_drop()
{
	g_phase = PH_DROP;
	new drop = get_pcvar_num(g_cDrop);
	g_phaseEnd = get_gametime() + float(drop);
	ls_announce(0, 5.0, "Battle Royale: loot up! Fighting starts in %d seconds", drop);
	if (get_pcvar_num(g_cSquad) > 1)
		ls_chat(0, "Squads of %d: your squad mates are on your side (say /br for help).", get_pcvar_num(g_cSquad));
}

stock start_play()
{
	g_phase = PH_PLAY;
	g_playAt = get_gametime();
	ls_announce(0, 3.0, "Fight! %d left", alive_sides());
	/* supply drops spread over the game */
	g_ndrops = min(get_pcvar_num(g_cDrops), sizeof g_dropAt);
	for (new i = 0; i < g_ndrops; i++)
		g_dropAt[i] = g_playAt + 60.0 + 90.0 * float(i) + random_float(-15.0, 15.0);
}

/* a map without a pack: a zone round the middle of the spawns, shrinking in five stages */
stock ensure_zone()
{
	if (ls_comp_count("ls_safe_zone"))
		return;
	new Float:sum[3], Float:mn[3] = { 99999.0, 99999.0, 0.0 }, Float:mx[3] = { -99999.0, -99999.0, 0.0 }, n = 0, Float:o[3], e = -1;
	new const cls[][] = { "info_player_start", "info_player_deathmatch" };
	for (new c = 0; c < 2; c++)
	{
		e = -1;
		while ((e = find_ent_by_class(e, cls[c])) > 0)
		{
			pev(e, pev_origin, o);
			for (new i = 0; i < 3; i++)
				sum[i] += o[i];
			for (new i = 0; i < 2; i++)
			{
				mn[i] = floatmin(mn[i], o[i]);
				mx[i] = floatmax(mx[i], o[i]);
			}
			n++;
		}
	}
	if (!n)
		return;
	for (new i = 0; i < 3; i++)
		sum[i] /= float(n);
	new Float:r = floatmax(mx[0] - mn[0], mx[1] - mn[1]) * 0.75 + 600.0;
	new keys[192];
	formatex(keys, charsmax(keys), "radius=%.0f damage=1 targetname=brzone", r);
	ls_comp_create("ls_safe_zone", sum, keys);
	formatex(keys, charsmax(keys), "zone=brzone start_delay=-1 stages=^"%.0f:45:40:1 %.0f:40:35:2 %.0f:35:30:4 %.0f:30:25:8 0:20:20:15^"",
		r * 0.7, r * 0.45, r * 0.25, r * 0.1);
	ls_comp_create("ls_zone_shrinker", sum, keys);
	log_amx("Battle Royale: no map pack for %s, so a zone goes round the spawns (radius %.0f).", g_mapname, r);
}

/* squads: br_squad players each, kept on one CS team, friends to the brain */
stock make_squads()
{
	new size = clamp(get_pcvar_num(g_cSquad), 1, 8);
	new list[32], n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			list[n++] = p;
	/* shuffle, then deal */
	for (new i = n - 1; i > 0; i--)
	{
		new j = random(i + 1), t = list[i];
		list[i] = list[j];
		list[j] = t;
	}
	g_nsquads = 0;
	new line[64];
	for (new i = 0; i < n; i++)
	{
		new p = list[i];
		if (size == 1)
		{
			g_squad[p] = i + 1;
			continue;
		}
		new sq = i / size + 1;
		g_squad[p] = sq;
		g_nsquads = max(g_nsquads, sq);
		cs_set_user_team(p, (sq % 2) ? CS_TEAM_T : CS_TEAM_CT);
		formatex(line, charsmax(line), "role %d sq%d", p, sq);
		ls_comp_bots_hint(line);
	}
	for (new sq = 1; sq <= g_nsquads; sq++)
	{
		formatex(line, charsmax(line), "friend @sq%d @sq%d 1", sq, sq);
		ls_comp_bots_hint(line);
	}
}

stock humans()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			n++;
	return n;
}

/* how many sides still have someone alive (players, or squads) */
stock alive_sides(&last = 0)
{
	new seen[66], n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		/* (someone who joined late has no squad: a side of their own) */
		new key = g_squad[p] ? min(g_squad[p], 32) : 33 + p;
		if (!seen[key])
		{
			seen[key] = 1;
			n++;
			last = p;
		}
	}
	return n;
}

stock alive_count()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p))
			n++;
	return n;
}

/* =========================================================================
 * Each second
 * ========================================================================= */

public tick()
{
	if (g_phase == PH_IDLE || g_phase == PH_OVER || !ls_components_running())
		return;
	new Float:now = get_gametime();
	if (g_phase == PH_WAIT)
	{
		if (humans())
		{
			/* someone joined: a fresh game where everyone starts spread out */
			g_phase = PH_OVER;
			server_cmd("endround");
			return;
		}
		hud_line("Battle Royale starts when a player joins");
		return;
	}
	if (g_phase == PH_DROP)
	{
		/* people who join while looting up still play */
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && !is_user_bot(p) && !is_user_alive(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
				ExecuteHamB(Ham_CS_RoundRespawn, p);
		if (now >= g_phaseEnd)
			start_play();
	}
	if (g_phase == PH_PLAY)
	{
		if (!g_zoneStarted && now - g_playAt >= get_pcvar_float(g_cZoneStart))
		{
			g_zoneStarted = true;
			new sh = ls_comp_find("ls_zone_shrinker");
			if (sh)
				ls_comp_start(sh);
		}
		if (g_dropsDone < g_ndrops && now >= g_dropAt[g_dropsDone])
		{
			g_dropsDone++;
			supply_drop();
		}
		if (check_winner())
			return;
	}
	bot_goals();
	hud();
}

stock bool:check_winner()
{
	new last;
	new sides = alive_sides(last);
	if (sides > 1)
		return false;
	g_phase = PH_OVER;
	if (sides == 0 || !last)
	{
		ls_announce(0, 6.0, "Nobody survived");
		server_cmd("endround");
		return true;
	}
	new name[32];
	get_user_name(last, name, charsmax(name));
	if (get_pcvar_num(g_cSquad) > 1)
		ls_announce(0, 8.0, "%s's squad wins!", name);
	else
		ls_announce(0, 8.0, "%s wins! (%d kills)", name, g_kills[last]);
	log_amx("Battle Royale on %s won by %s (%d kills)", g_mapname, name, g_kills[last]);
	server_cmd(cs_get_user_team(last) == CS_TEAM_T ? "endround T" : "endround CT");
	return true;
}

stock supply_drop()
{
	new markers[16], n = 0;
	for (new e = ls_comp_find("ls_supply_drop"); e && n < sizeof markers; e = ls_comp_find("ls_supply_drop", e))
		markers[n++] = e;
	/* prefer spots inside the next circle */
	new Float:c[3], Float:r, closing;
	ls_zone_next(0, c, r, closing);
	new pick = 0;
	for (new tries = 0; tries < 6 && n; tries++)
	{
		new e = markers[random(n)];
		new Float:o[3];
		pev(e, pev_origin, o);
		pick = e;
		if (r <= 0.0 || floatsqroot((o[0] - c[0]) * (o[0] - c[0]) + (o[1] - c[1]) * (o[1] - c[1])) < r)
			break;
	}
	if (pick)
		ls_comp_start(pick);
	else if (r > 0.0)
		ls_drop_call(c, "drop");
}

/* =========================================================================
 * Players
 * ========================================================================= */

public ham_spawn(id)
{
	if (!is_user_alive(id) || g_phase == PH_IDLE || g_phase == PH_OVER)
		return HAM_IGNORED;
	remove_task(TASK_GEAR + id);
	set_task(0.3, "task_gear", TASK_GEAR + id);
	return HAM_IGNORED;
}

/* everyone starts with only br_start (after gm_components moved them to a respawn point) */
public task_gear(tid)
{
	new id = tid - TASK_GEAR;
	if (!is_user_alive(id))
		return;
	strip_user_weapons(id);
	new items[128], w[32], pos = 0;
	get_pcvar_string(g_cStart, items, charsmax(items));
	while (items[pos])
	{
		new i = 0;
		/* "knife glock18", "knife+glock18" or "knife,glock18" */
		while (items[pos] == ' ' || items[pos] == '+' || items[pos] == ',')
			pos++;
		while (items[pos] && items[pos] != ' ' && items[pos] != '+' && items[pos] != ',' && i < charsmax(w))
			w[i++] = items[pos++];
		w[i] = EOS;
		if (w[0])
			ls_give_item(id, w);
	}
	cs_set_user_money(id, 0);
	cs_set_user_armor(id, 0, CS_ARMOR_NONE);
}

public ham_takedamage(victim, inflictor, attacker, Float:dmg, bits)
{
	if (!(1 <= attacker <= MaxClients) || attacker == victim || !is_user_connected(attacker))
		return HAM_IGNORED;
	/* nobody hurts anybody while looting up; squad mates never hurt each other */
	if (g_phase == PH_WAIT || g_phase == PH_DROP)
		return HAM_SUPERCEDE;
	if (get_pcvar_num(g_cSquad) > 1 && g_squad[victim] && g_squad[victim] == g_squad[attacker])
		return HAM_SUPERCEDE;
	return HAM_IGNORED;
}

public ham_killed(victim, killer, gib)
{
	if (g_phase != PH_PLAY)
		return HAM_IGNORED;
	if (1 <= killer <= MaxClients && killer != victim)
		g_kills[killer]++;
	new place = alive_count() + 1;
	new name[32];
	get_user_name(victim, name, charsmax(name));
	ls_chat(0, "%s is out (#%d).", name, place);
	return HAM_IGNORED;
}

/* =========================================================================
 * Bots: a few personal goals, varied, each with an end time
 * ========================================================================= */

stock pick_traits()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || !is_user_bot(p))
			continue;
		new r = random_num(1, 100);
		g_trait[p] = r <= 35 ? TR_LOOTER : (r <= 60 ? TR_CAMPER : TR_HUNTER);
		g_nextGoal[p] = get_gametime() + random_float(3.0, 12.0);
	}
}

stock bot_goals()
{
	new Float:now = get_gametime();
	new Float:c[3], Float:r, closing;
	new Float:secs = ls_zone_next(0, c, r, closing);
	/* a new stage: campers pick a spot inside the next circle */
	new stage = ls_zone_stage(ls_comp_find("ls_zone_shrinker"));
	new bool:newStage = stage != g_lastStage;
	g_lastStage = stage;
	new line[128];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		if (g_trait[p] == TR_CAMPER && newStage && r > 0.0 && stage > 0)
		{
			new Float:a = random_float(0.0, 360.0), Float:d = r * random_float(0.25, 0.65);
			new Float:o[3];
			o[0] = c[0] + d * floatcos(a, degrees);
			o[1] = c[1] + d * floatsin(a, degrees);
			o[2] = c[2];
			formatex(line, charsmax(line), "goal %d go_hold %.0f %.0f %.0f 0 45 %d 300", p, o[0], o[1], o[2], floatround(floatmax(secs, 30.0) + 20.0));
			ls_comp_bots_hint(line);
			continue;
		}
		if (g_trait[p] != TR_LOOTER || now < g_nextGoal[p] || g_phase == PH_WAIT)
			continue;
		g_nextGoal[p] = now + random_float(15.0, 30.0);
		/* the nearest full crate that's inside the zone (and not too far) */
		new Float:po[3], best = 0, Float:bd = 1500.0;
		pev(p, pev_origin, po);
		for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
		{
			if (ls_comp_state(e) != 0)
				continue;
			new Float:o[3];
			pev(e, pev_origin, o);
			if (r > 0.0 && floatsqroot((o[0] - c[0]) * (o[0] - c[0]) + (o[1] - c[1]) * (o[1] - c[1])) > r)
				continue;
			new Float:d = get_distance_f(po, o);
			if (d < bd)
			{
				bd = d;
				best = e;
			}
		}
		if (!best)
			continue;
		new Float:o[3];
		pev(best, pev_origin, o);
		formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 40 25 80", p, o[0], o[1], o[2], best);
		ls_comp_bots_hint(line);
	}
}

/* =========================================================================
 * HUD
 * ========================================================================= */

stock hud_line(const text[])
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "%s", text);
}

stock hud()
{
	new Float:now = get_gametime();
	new top[128];
	new alive = alive_count();
	if (g_phase == PH_DROP)
		formatex(top, charsmax(top), "ALIVE %d   Loot up: fighting in %d s", alive, max(floatround(g_phaseEnd - now), 0));
	else
	{
		new Float:c[3], Float:r, closing;
		new Float:secs = ls_zone_next(0, c, r, closing);
		if (!g_zoneStarted)
			formatex(top, charsmax(top), "ALIVE %d   The zone starts closing in %d s", alive, max(floatround(get_pcvar_float(g_cZoneStart) - (now - g_playAt)), 0));
		else if (closing)
			formatex(top, charsmax(top), "ALIVE %d   ZONE CLOSING (%d s)", alive, floatround(secs));
		else if (secs >= 0.0)
			formatex(top, charsmax(top), "ALIVE %d   Zone closes in %d:%02d", alive, floatround(secs) / 60, floatround(secs) % 60);
		else
			formatex(top, charsmax(top), "ALIVE %d   Final zone", alive);
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue;
		ls_hud(p, LS_HUD_TOP, 1.1, "%s", top);
		ls_hud(p, LS_HUD_RIGHT, 1.1, "Kills %d", g_kills[p]);
		if (get_pcvar_num(g_cSquad) > 1 && g_squad[p])
		{
			new team[256], name[24], len = 0;
			for (new q = 1; q <= MaxClients; q++)
			{
				if (q == p || g_squad[q] != g_squad[p] || !is_user_connected(q))
					continue;
				get_user_name(q, name, charsmax(name));
				if (is_user_alive(q))
					len += formatex(team[len], charsmax(team) - len, "%s  %d^n", name, get_user_health(q));
				else
					len += formatex(team[len], charsmax(team) - len, "%s  out^n", name);
			}
			if (len)
				ls_hud(p, LS_HUD_LEFT, 1.1, "Squad^n%s", team);
		}
	}
}
