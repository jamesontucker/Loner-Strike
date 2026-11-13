/*
 * gm_laststand.sma - Last Stand: hold the base against waves (GAMEMODES-PLAN.md 3.4).
 *
 * Everyone plays Counter-Terrorist. Waves come from the map's ls_wave_spawners (a map pack
 * places them on stock maps; without one, a spawner goes on the Terrorist spawn). Enemies are
 * Loner-Strike's creatures when the server's game DLL has them (ls_creature_ver); otherwise
 * Terrorist bots play knife-only zombies. Between waves: money, a supply drop, buying anywhere,
 * barricade repairs (hold USE), and the dead come back. A downed player can be revived by a
 * teammate holding USE on the spot. Lose when nobody on the team is left standing; win by
 * holding out every wave (lst_waves 0 = endless, with a best-wave record per map).
 *
 * Needs gm_components.amxx (listed before this in laststand.plugins.ini).
 * Settings: lst_waves, lst_difficulty, lst_prepare, lst_break, lst_revive, lst_drops,
 * lst_creatures, lst_defenders, lst_wait_human (1: the stand waits for a person on CT). docs/COMPONENTS.md has the components; docs/CREATURES.md the
 * creatures.
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
#define TASK_TICK 7737100
#define TASK_REV  7737200
#define TASK_SETUP 7737300

enum { PH_IDLE = 0, PH_PREPARE, PH_WAVE, PH_BREAK, PH_OVER };

new const TYPES[][] = { "zombie", "runner", "brute", "spitter", "boss" };
new const TYPE_HP[] = { 100, 60, 450, 120, 3000 };
new const TYPE_MONEY[] = { 100, 100, 250, 150, 1000 };
new const CREATURE_PRECACHE[] = "zombie runner brute spitter boss";

new g_cWaves, g_cDiff, g_cPrepare, g_cBreak, g_cRevive, g_cDrops, g_cCreatures, g_cDefenders, g_cWaveMoney;

new g_phase;
new g_wave;
new Float:g_phaseEnd;
new bool:g_creatures;
new bool:g_hasCreatureDll;
new g_failSpawns;
new Float:g_base[3];
new g_planned;
new g_spawned;
new bool:g_bossDone;
new g_record;
new g_mapname[64];
new g_classZombie;
new g_kills;
new Float:g_waveAt;

new bool:g_down[33];
new Float:g_downAt[33];
new Float:g_downOrg[33][3];
new g_revBy[33];
new Float:g_revProg[33];

new g_msgShake, g_msgRoundTime;
new g_sprBeam;

/* =========================================================================
 * Start-up
 * ========================================================================= */

public plugin_precache()
{
	g_sprBeam = precache_model("sprites/laserbeam.spr");
	/* creature types can only spawn mid-map if they were precached as the map loaded */
	g_hasCreatureDll = get_cvar_pointer("ls_creature_ver") != 0;
	g_cCreatures = register_cvar("lst_creatures", "1");
	if (g_hasCreatureDll && get_pcvar_num(g_cCreatures))
		set_cvar_string("ls_creature_precache", CREATURE_PRECACHE);
}

public plugin_init()
{
	ls_register_mode("laststand", "Last Stand", VERSION, "Loner-Strike");

	g_cWaves = register_cvar("lst_waves", "10");
	g_cDiff = register_cvar("lst_difficulty", "1");
	g_cPrepare = register_cvar("lst_prepare", "45");
	g_cBreak = register_cvar("lst_break", "20");
	g_cRevive = register_cvar("lst_revive", "30");
	g_cDrops = register_cvar("lst_drops", "2");
	g_cDefenders = register_cvar("lst_defenders", "4");
	g_cWaveMoney = register_cvar("lst_wave_money", "1000");

	g_msgShake = get_user_msgid("ScreenShake");
	g_msgRoundTime = get_user_msgid("RoundTime");

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");
	RegisterHam(Ham_Killed, "player", "ham_player_killed", 1);
	RegisterHam(Ham_Spawn, "player", "ham_player_spawn", 1);
	RegisterHam(Ham_Touch, "weaponbox", "ham_touch_weapon");
	RegisterHam(Ham_Touch, "armoury_entity", "ham_touch_weapon");
	if (g_hasCreatureDll)
		RegisterHam(Ham_Killed, "ls_creature", "ham_creature_killed", 1);

	register_clcmd("say /ls", "cmd_help");
	register_clcmd("say /laststand", "cmd_help");

	get_mapname(g_mapname, charsmax(g_mapname));
	g_record = record_load();
	set_task(1.0, "tick", TASK_TICK, _, _, "b");
	set_task(0.25, "tick_revive", TASK_REV, _, _, "b");
}

public plugin_cfg()
{
	g_creatures = g_hasCreatureDll && get_pcvar_num(g_cCreatures) != 0;
	if (!ls_components_running())
		log_amx("Last Stand needs gm_components.amxx; add it to laststand.plugins.ini before gm_laststand.amxx.");
	log_amx("Last Stand on %s: enemies are %s (best wave here: %d)", g_mapname, g_creatures ? "creatures" : "zombie bots", g_record);
}

public cmd_help(id)
{
	ls_chat(id, "Last Stand: hold the base. Buy and repair barricades (hold USE) between waves; hold USE on a downed teammate to revive them.");
	return PLUGIN_HANDLED;
}

/* =========================================================================
 * Rounds: one round is one stand, from the first wave to a win or a loss
 * ========================================================================= */

public ev_new_round()
{
	g_phase = PH_IDLE;
	g_wave = 0;
	g_planned = 0;
	g_spawned = 0;
	for (new p = 1; p <= MaxClients; p++)
		clear_down(p);
	if (ls_components_running())
		ls_wave_stop();
	if (g_creatures)
		server_cmd("ls_creature remove all");
}

public ev_round_start()
{
	remove_task(TASK_SETUP);
	set_task(0.5, "round_setup", TASK_SETUP);
}

public round_setup()
{
	if (!ls_components_running())
		return;
	/* zombie bots can be picked any time; creatures only for a map that loaded them */
	if (g_creatures && !get_pcvar_num(g_cCreatures))
		g_creatures = false;
	find_base();
	ensure_components();
	assign_teams(true);
	ls_comp_bots_hint("set laststand");
	if (g_creatures)
		ls_comp_bots_hint("enemyclass ls_creature 1");
	defend_hint();
	g_phase = PH_PREPARE;
	g_phaseEnd = get_gametime() + get_pcvar_float(g_cPrepare);
	set_cvar_num("mp_buy_anywhere", 1);
	round_clock(floatround(get_pcvar_float(g_cPrepare)));
	ls_announce(0, 5.0, "Last Stand: hold the base! First wave in %d seconds", get_pcvar_num(g_cPrepare));
	if (g_record > 0)
		ls_chat(0, "Best on %s so far: wave %d. Buy now - you can buy anywhere until the wave comes.", g_mapname, g_record);
}

/* the base: the middle of the CT respawn points, or of the CT spawns */
stock find_base()
{
	new Float:sum[3], n = 0, Float:o[3];
	for (new e = ls_comp_find("ls_respawn_point"); e; e = ls_comp_find("ls_respawn_point", e))
	{
		new team[8];
		ls_comp_get(e, "team", team, charsmax(team));
		if (team[0] == '1' || equali(team, "t"))
			continue;
		pev(e, pev_origin, o);
		sum[0] += o[0]; sum[1] += o[1]; sum[2] += o[2];
		n++;
	}
	if (!n)
	{
		new e = -1;
		while ((e = find_ent_by_class(e, "info_player_start")) > 0)
		{
			pev(e, pev_origin, o);
			sum[0] += o[0]; sum[1] += o[1]; sum[2] += o[2];
			n++;
		}
	}
	if (n)
	{
		g_base[0] = sum[0] / n;
		g_base[1] = sum[1] / n;
		g_base[2] = sum[2] / n;
	}
}

/* any map plays: without a map pack, waves come from the Terrorist spawn */
stock ensure_components()
{
	if (!ls_comp_count("ls_wave_spawner"))
	{
		new Float:sum[3], n = 0, Float:o[3], e = -1;
		while ((e = find_ent_by_class(e, "info_player_deathmatch")) > 0)
		{
			pev(e, pev_origin, o);
			sum[0] += o[0]; sum[1] += o[1]; sum[2] += o[2];
			n++;
		}
		if (n)
		{
			sum[0] /= n; sum[1] /= n; sum[2] /= n;
			ls_comp_create("ls_wave_spawner", sum, "team=t radius=160 rate=1.2");
			log_amx("Last Stand: no wave spawners on %s, so one goes on the Terrorist spawn.", g_mapname);
		}
	}
	if (!g_creatures && !g_classZombie)
		g_classZombie = ls_comp_create("ls_player_class", g_base, "name=Zombie team=t default=1 menu=0 weapons=knife hp=100 speed=270");
}

/* creatures: every bot defends. Zombie bots: lst_defenders bots defend, the rest are zombies */
stock assign_teams(bool:fresh)
{
	set_cvar_string("bot_join_team", g_creatures ? "CT" : "any");
	new want = g_creatures ? 32 : max(get_pcvar_num(g_cDefenders), 0);
	new ct = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) == CS_TEAM_SPECTATOR || cs_get_user_team(p) == CS_TEAM_UNASSIGNED)
			continue;
		if (!is_user_bot(p))
		{
			if (cs_get_user_team(p) != CS_TEAM_CT)
				move_to(p, CS_TEAM_CT, fresh);
			continue;
		}
		if (ct < want)
		{
			ct++;
			if (cs_get_user_team(p) != CS_TEAM_CT)
				move_to(p, CS_TEAM_CT, fresh);
		}
		else if (cs_get_user_team(p) != CS_TEAM_T)
			move_to(p, CS_TEAM_T, fresh);
	}
	/* zombie bots wait dead until a wave brings them in */
	if (!g_creatures)
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T)
				user_silentkill(p);
}

stock move_to(p, CsTeams:team, bool:respawn)
{
	cs_set_user_team(p, team);
	if (respawn && team == CS_TEAM_CT)
		ExecuteHamB(Ham_CS_RoundRespawn, p);
}

stock defend_hint()
{
	new line[96];
	formatex(line, charsmax(line), "goal ct defend %.0f %.0f %.0f 0 70 0 450", g_base[0], g_base[1], g_base[2]);
	ls_comp_bots_hint(line);
}

/* =========================================================================
 * Waves
 * ========================================================================= */

stock defenders()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && cs_get_user_team(p) == CS_TEAM_CT)
			n++;
	return max(n, 1);
}

stock Float:diff_mult(bool:hp)
{
	switch (clamp(get_pcvar_num(g_cDiff), 0, 3))
	{
		case 0: return hp ? 0.8 : 0.7;
		case 2: return hp ? 1.25 : 1.35;
		case 3: return hp ? 1.6 : 1.8;
	}
	return 1.0;
}

stock Float:hp_mult()
{
	return (1.0 + 0.07 * float(g_wave - 1)) * (0.8 + 0.1 * float(defenders())) * diff_mult(true);
}

stock start_wave(n)
{
	g_wave = n;
	g_phase = PH_WAVE;
	g_bossDone = (n % 5) != 0;
	g_spawned = 0;
	g_failSpawns = 0;
	set_cvar_num("mp_buy_anywhere", 0);

	/* how many: grows each wave, with the team's size and the difficulty */
	new total = floatround((5.0 + 2.5 * float(n - 1)) * (0.5 + 0.2 * float(defenders())) * diff_mult(false));
	total = max(total, 1);
	new nsp = max(ls_comp_count("ls_wave_spawner"), 1);
	g_planned = 0;
	new i = 0;
	for (new e = ls_comp_find("ls_wave_spawner"); e; e = ls_comp_find("ls_wave_spawner", e))
	{
		new each = total / nsp + (i < total % nsp ? 1 : 0);
		if (!g_bossDone && i == 0)
			each++;
		new v[8];
		num_to_str(each, v, charsmax(v));
		ls_comp_set(e, "wave_min", v);
		ls_comp_set(e, "wave_max", v);
		ls_comp_set(e, "waves", "1");
		g_planned += each;
		i++;
	}
	if (!g_creatures && g_classZombie)
	{
		new v[8];
		num_to_str(floatround(100.0 * hp_mult()), v, charsmax(v));
		ls_comp_set(g_classZombie, "hp", v);
	}
	ls_wave_start(n);
	defend_hint();
	g_kills = 0;
	g_waveAt = get_gametime();
	log_amx("Wave %d: %d %s against %d defenders", n, g_planned, g_creatures ? "creatures" : "zombie bots", defenders());
	if (!g_bossDone)
	{
		ls_announce(0, 4.0, "Wave %d: a boss is coming!", n);
		shake_all();
	}
}

stock enemies_left()
{
	return max(g_planned - g_spawned, 0) + ls_wave_alive();
}

public ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[])
{
	if (g_phase != PH_WAVE)
		return PLUGIN_HANDLED;
	if (!g_creatures)
		return PLUGIN_CONTINUE;
	new t = pick_type();
	new hp = floatround(float(TYPE_HP[t]) * hp_mult());
	new Float:o[3];
	o[0] = origin[0];
	o[1] = origin[1];
	o[2] = floor_under(origin) + 1.0;
	new cr = spawn_creature(TYPES[t], o, hp);
	if (!cr)
	{
		/* full (ls_creature_max) or the type wasn't loaded: try again in a moment, and after
		   a long run of failures with nothing alive, fall back to zombie bots */
		if (++g_failSpawns >= 20 && !ls_wave_alive())
		{
			log_amx("Last Stand: creatures won't spawn on %s; using zombie bots instead.", g_mapname);
			g_creatures = false;
			ensure_components();
			assign_teams(false);
		}
		/* not handled: the spawner finds no dead Terrorist either and tries again in a second */
		return PLUGIN_CONTINUE;
	}
	g_failSpawns = 0;
	if (t == 4)
		g_bossDone = true;
	ls_wave_track(cr);
	g_spawned++;
	return PLUGIN_HANDLED;
}

stock pick_type()
{
	if (!g_bossDone)
		return 4;
	new n = g_wave;
	new w[4];
	w[0] = 10;
	w[1] = n >= 2 ? 4 + n : 0;
	w[2] = n >= 3 ? 1 + n / 3 : 0;
	w[3] = n >= 4 ? 2 : 0;
	new total = w[0] + w[1] + w[2] + w[3];
	new r = random(total);
	for (new i = 0; i < 4; i++)
	{
		r -= w[i];
		if (r < 0)
			return i;
	}
	return 0;
}

stock spawn_creature(const type[], const Float:o[3], hp)
{
	new ent = create_entity("ls_creature");
	if (!ent)
		return 0;
	new v[16];
	DispatchKeyValue(ent, "type", type);
	num_to_str(hp, v, charsmax(v));
	DispatchKeyValue(ent, "health", v);
	DispatchKeyValue(ent, "behaviour", "chase");
	DispatchKeyValue(ent, "hates", "ct");
	new Float:ang[3];
	ang[1] = random_float(0.0, 360.0);
	entity_set_vector(ent, EV_VEC_angles, ang);
	entity_set_origin(ent, o);
	DispatchSpawn(ent);
	if (!is_valid_ent(ent) || (entity_get_int(ent, EV_INT_flags) & FL_KILLME))
		return 0;
	return ent;
}

public ls_on_wave_cleared(wave)
{
	if (g_phase != PH_WAVE || wave != g_wave)
		return;
	log_amx("Wave %d held in %d s (%d kills)", g_wave, floatround(get_gametime() - g_waveAt), g_kills);
	if (g_wave > g_record)
	{
		g_record = g_wave;
		record_save();
	}
	new waves = get_pcvar_num(g_cWaves);
	if (waves > 0 && g_wave >= waves)
	{
		g_phase = PH_OVER;
		ls_announce(0, 8.0, "You held out all %d waves!", waves);
		server_cmd("endround CT");
		return;
	}
	g_phase = PH_BREAK;
	new brk = get_pcvar_num(g_cBreak);
	g_phaseEnd = get_gametime() + float(brk);
	set_cvar_num("mp_buy_anywhere", 1);
	round_clock(brk);
	/* money, the dead back, maybe a supply drop */
	new bonus = get_pcvar_num(g_cWaveMoney);
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		cs_set_user_money(p, min(cs_get_user_money(p) + bonus, 16000));
		if (!is_user_alive(p))
		{
			clear_down(p);
			ExecuteHamB(Ham_CS_RoundRespawn, p);
		}
	}
	new drops = get_pcvar_num(g_cDrops);
	if (drops > 0 && g_wave % drops == 0)
		supply_drop();
	ls_announce(0, 4.0, "Wave %d held! Next wave in %d seconds - buy anywhere now", g_wave, brk);
}

stock supply_drop()
{
	new markers[32], n = 0;
	for (new e = ls_comp_find("ls_supply_drop"); e && n < sizeof markers; e = ls_comp_find("ls_supply_drop", e))
		markers[n++] = e;
	if (n)
	{
		ls_comp_start(markers[random(n)]);
		return;
	}
	new Float:o[3];
	o[0] = g_base[0] + random_float(-200.0, 200.0);
	o[1] = g_base[1] + random_float(-200.0, 200.0);
	o[2] = g_base[2];
	ls_drop_call(o, "drop");
}

/* =========================================================================
 * The stand: once a second
 * ========================================================================= */

public tick()
{
	if (g_phase == PH_IDLE || g_phase == PH_OVER || !ls_components_running())
		return;
	new Float:now = get_gametime();
	if ((g_phase == PH_PREPARE || g_phase == PH_BREAK) && now >= g_phaseEnd)
		start_wave(g_wave + 1);

	/* lost: nobody on the team standing */
	new alive = 0, team = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		team++;
		if (is_user_alive(p))
			alive++;
	}
	if (team > 0 && alive == 0 && g_phase == PH_WAVE)
	{
		g_phase = PH_OVER;
		if (g_wave - 1 > g_record)
		{
			g_record = g_wave - 1;
			record_save();
		}
		ls_announce(0, 8.0, "The base fell on wave %d", g_wave);
		log_amx("The base fell on wave %d", g_wave);
		ls_chat(0, "Best on %s: wave %d.", g_mapname, g_record);
		server_cmd("endround T");
		return;
	}
	if (g_phase == PH_WAVE)
		attack_barricades();
	hud();
	beacons();
}

/* creatures and zombies that reach a barricade hit it */
stock attack_barricades()
{
	new Float:dmg = 12.0 * (1.0 + 0.1 * float(g_wave - 1)) * diff_mult(false);
	for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
	{
		if (ls_barricade_hp(b) <= 0)
			continue;
		new Float:mn[3], Float:mx[3];
		pev(b, pev_absmin, mn);
		pev(b, pev_absmax, mx);
		new hits = 0;
		new e = -1;
		if (g_creatures)
		{
			while ((e = find_ent_by_class(e, "ls_creature")) > 0 && hits < 6)
				if (pev(e, pev_deadflag) == DEAD_NO && near_box(e, mn, mx, 56.0))
				{
					ExecuteHamB(Ham_TakeDamage, b, e, e, dmg, DMG_SLASH);
					hits++;
				}
		}
		else
		{
			for (new p = 1; p <= MaxClients && hits < 6; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T && near_box(p, mn, mx, 48.0))
				{
					ExecuteHamB(Ham_TakeDamage, b, p, p, dmg, DMG_SLASH);
					hits++;
				}
		}
	}
}

stock bool:near_box(e, const Float:mn[3], const Float:mx[3], Float:reach)
{
	new Float:o[3];
	pev(e, pev_origin, o);
	new Float:d = 0.0;
	for (new i = 0; i < 2; i++)
	{
		new Float:c = floatclamp(o[i], mn[i], mx[i]);
		d += (o[i] - c) * (o[i] - c);
	}
	return d <= reach * reach && o[2] > mn[2] - 80.0 && o[2] < mx[2] + 80.0;
}

stock hud()
{
	new Float:now = get_gametime();
	new top[96];
	new waves = get_pcvar_num(g_cWaves);
	switch (g_phase)
	{
		case PH_PREPARE:
			formatex(top, charsmax(top), "Get ready: first wave in %d s  (buy anywhere)", max(floatround(g_phaseEnd - now), 0));
		case PH_BREAK:
			formatex(top, charsmax(top), "Wave %d held - next in %d s  (buy anywhere)", g_wave, max(floatround(g_phaseEnd - now), 0));
		case PH_WAVE:
		{
			if (waves > 0)
				formatex(top, charsmax(top), "WAVE %d/%d  -  %d left", g_wave, waves, enemies_left());
			else
				formatex(top, charsmax(top), "WAVE %d  -  %d left  (best %d)", g_wave, enemies_left(), g_record);
		}
	}
	/* the team: who's up, who's down */
	new team[400], name[24], len = 0, lines = 0;
	for (new p = 1; p <= MaxClients && len < charsmax(team) - 40 && lines < 10; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		lines++;
		get_user_name(p, name, charsmax(name));
		if (is_user_alive(p))
			len += formatex(team[len], charsmax(team) - len, "%s  %d^n", name, get_user_health(p));
		else if (g_down[p])
			len += formatex(team[len], charsmax(team) - len, "%s  DOWN %ds^n", name, max(floatround(get_pcvar_float(g_cRevive) - (now - g_downAt[p])), 0));
		else
			len += formatex(team[len], charsmax(team) - len, "%s  out^n", name);
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue;
		ls_hud(p, LS_HUD_TOP, 1.1, "%s", top);
		ls_hud(p, LS_HUD_LEFT, 1.1, "%s", team);
		new bar = nearest_barricade(p);
		if (bar)
		{
			new mx = floatround(ls_comp_getf(bar, "hp", 500.0));
			new hp = ls_barricade_hp(bar);
			if (hp > 0)
				ls_hud(p, LS_HUD_RIGHT, 1.1, "Barricade %d%%", 100 * hp / max(mx, 1));
			else
				ls_hud(p, LS_HUD_RIGHT, 1.1, "Barricade DOWN");
		}
	}
}

stock nearest_barricade(p)
{
	new Float:o[3], Float:bo[3], best = 0, Float:bd = 1200.0;
	pev(p, pev_origin, o);
	for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
	{
		new Float:mn[3], Float:mx[3];
		pev(b, pev_absmin, mn);
		pev(b, pev_absmax, mx);
		for (new i = 0; i < 3; i++)
			bo[i] = (mn[i] + mx[i]) * 0.5;
		new Float:d = get_distance_f(o, bo);
		if (d < bd)
		{
			bd = d;
			best = b;
		}
	}
	return best;
}

stock beacons()
{
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!g_down[d])
			continue;
		new Float:a[3], Float:b[3];
		a = g_downOrg[d];
		b = g_downOrg[d];
		a[2] -= 36.0;
		b[2] += 60.0;
		message_begin(MSG_BROADCAST, SVC_TEMPENTITY);
		write_byte(TE_BEAMPOINTS);
		write_coord(floatround(a[0])); write_coord(floatround(a[1])); write_coord(floatround(a[2]));
		write_coord(floatround(b[0])); write_coord(floatround(b[1])); write_coord(floatround(b[2]));
		write_short(g_sprBeam);
		write_byte(0); write_byte(0); write_byte(11); write_byte(30); write_byte(0);
		write_byte(60); write_byte(160); write_byte(255); write_byte(200); write_byte(0);
		message_end();
	}
}

/* =========================================================================
 * Down and revive
 * ========================================================================= */

public ham_player_killed(victim, killer, gib)
{
	if (g_phase == PH_WAVE && !g_creatures && is_user_connected(victim) && cs_get_user_team(victim) == CS_TEAM_T)
		g_kills++;
	if (g_phase != PH_WAVE && g_phase != PH_BREAK && g_phase != PH_PREPARE)
		return HAM_IGNORED;
	if (!is_user_connected(victim) || cs_get_user_team(victim) != CS_TEAM_CT || get_pcvar_num(g_cRevive) <= 0)
		return HAM_IGNORED;
	g_down[victim] = true;
	g_downAt[victim] = get_gametime();
	pev(victim, pev_origin, g_downOrg[victim]);
	g_revBy[victim] = 0;
	g_revProg[victim] = 0.0;
	new name[32], line[96];
	get_user_name(victim, name, charsmax(name));
	ls_chat(0, "%s is down! Hold USE on the blue beam to revive them.", name);
	formatex(line, charsmax(line), "goal ct go_hold %.0f %.0f %.0f %d 55 %d 40", g_downOrg[victim][0], g_downOrg[victim][1], g_downOrg[victim][2], victim, get_pcvar_num(g_cRevive));
	ls_comp_bots_hint(line);
	return HAM_IGNORED;
}

stock clear_down(p)
{
	if (g_down[p])
	{
		new line[48];
		formatex(line, charsmax(line), "goal_clear ct %d", p);
		ls_comp_bots_hint(line);
	}
	if (g_revBy[p] && is_user_connected(g_revBy[p]))
		bar(g_revBy[p], 0);
	g_down[p] = false;
	g_revBy[p] = 0;
	g_revProg[p] = 0.0;
}

public tick_revive()
{
	new Float:now = get_gametime();
	new Float:limit = get_pcvar_float(g_cRevive);
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!g_down[d])
			continue;
		if (!is_user_connected(d) || is_user_alive(d) || now - g_downAt[d] > limit)
		{
			if (is_user_connected(d) && !is_user_alive(d))
			{
				new name[32];
				get_user_name(d, name, charsmax(name));
				ls_chat(0, "%s is out until the next wave.", name);
			}
			clear_down(d);
			continue;
		}
		/* someone close: people hold USE, bots just stay there */
		new who = 0;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == d || !is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_CT)
				continue;
			new Float:o[3];
			pev(p, pev_origin, o);
			if (get_distance_f(o, g_downOrg[d]) > 72.0)
				continue;
			if (!is_user_bot(p) && !(pev(p, pev_button) & IN_USE))
				continue;
			who = p;
			break;
		}
		if (who != g_revBy[d])
		{
			if (g_revBy[d] && is_user_connected(g_revBy[d]))
				bar(g_revBy[d], 0);
			g_revBy[d] = who;
			g_revProg[d] = 0.0;
			if (who)
				bar(who, 3);
		}
		if (!who)
			continue;
		g_revProg[d] += 0.25;
		if (g_revProg[d] >= 3.0)
			revive(d, who);
	}
}

stock revive(d, by)
{
	new Float:o[3];
	o = g_downOrg[d];
	clear_down(d);
	ExecuteHamB(Ham_CS_RoundRespawn, d);
	if (!is_user_alive(d))
		return;
	engfunc(EngFunc_TraceHull, o, o, DONT_IGNORE_MONSTERS, HULL_HUMAN, d, 0);
	if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
		engfunc(EngFunc_SetOrigin, d, o);
	set_user_health(d, 50);
	new a[32], b[32];
	get_user_name(d, a, charsmax(a));
	get_user_name(by, b, charsmax(b));
	ls_chat(0, "%s revived %s.", b, a);
}

stock bar(id, secs)
{
	if (is_user_bot(id))
		return;
	static msg;
	if (!msg)
		msg = get_user_msgid("BarTime");
	message_begin(MSG_ONE, msg, _, id);
	write_short(secs);
	message_end();
}

/* =========================================================================
 * Players, zombies, creatures
 * ========================================================================= */

public ham_player_spawn(id)
{
	if (!is_user_alive(id) || g_phase == PH_IDLE)
		return HAM_IGNORED;
	if (cs_get_user_team(id) == CS_TEAM_T && !g_creatures)
	{
		/* a zombie: green, no money, knife only (its class does the rest) */
		if (g_phase == PH_WAVE)
			g_spawned++;
		set_rendering(id, kRenderFxGlowShell, 40, 200, 40, kRenderNormal, 10);
		cs_set_user_money(id, 0);
	}
	return HAM_IGNORED;
}

public ham_touch_weapon(weapon, toucher)
{
	if (g_creatures || g_phase == PH_IDLE || !(1 <= toucher <= MaxClients) || !is_user_alive(toucher))
		return HAM_IGNORED;
	return cs_get_user_team(toucher) == CS_TEAM_T ? HAM_SUPERCEDE : HAM_IGNORED;
}

public ham_creature_killed(ent, killer, gib)
{
	g_kills++;
	if (!(1 <= killer <= MaxClients) || !is_user_connected(killer))
		return HAM_IGNORED;
	new type[16];
	pev(ent, pev_netname, type, charsmax(type));
	new money = 100;
	for (new i = 0; i < sizeof TYPES; i++)
		if (equal(type, TYPES[i]))
			money = TYPE_MONEY[i];
	cs_set_user_money(killer, min(cs_get_user_money(killer) + money, 16000));
	if (equal(type, "boss"))
	{
		new name[32];
		get_user_name(killer, name, charsmax(name));
		ls_announce(0, 4.0, "%s brought the boss down!", name);
	}
	return HAM_IGNORED;
}

/* =========================================================================
 * Bits
 * ========================================================================= */

stock Float:floor_under(const Float:p[3])
{
	new Float:a[3], Float:b[3], Float:hit[3];
	a = p;
	b = p;
	b[2] -= 512.0;
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, 0, 0);
	get_tr2(0, TR_vecEndPos, hit);
	return hit[2];
}

stock round_clock(secs)
{
	if (!g_msgRoundTime)
		return;
	message_begin(MSG_ALL, g_msgRoundTime);
	write_short(secs);
	message_end();
}

stock shake_all()
{
	if (!g_msgShake)
		return;
	message_begin(MSG_ALL, g_msgShake);
	write_short(10 << 12);
	write_short(2 << 12);
	write_short(10 << 12);
	message_end();
}

stock record_path(out[], len)
{
	get_datadir(out, len);
	format(out, len, "%s/gamemodes/laststand_records.ini", out);
}

stock record_load()
{
	new path[192], line[96], map[64], wave[12];
	record_path(path, charsmax(path));
	new f = fopen(path, "rt");
	if (!f)
		return 0;
	new best = 0;
	while (fgets(f, line, charsmax(line)))
	{
		parse(line, map, charsmax(map), wave, charsmax(wave));
		if (equali(map, g_mapname))
			best = str_to_num(wave);
	}
	fclose(f);
	return best;
}

stock record_save()
{
	new path[192], tmp[200], line[96], map[64], wave[12];
	record_path(path, charsmax(path));
	formatex(tmp, charsmax(tmp), "%s.tmp", path);
	new out = fopen(tmp, "wt");
	if (!out)
		return;
	new in = fopen(path, "rt");
	if (in)
	{
		while (fgets(in, line, charsmax(line)))
		{
			parse(line, map, charsmax(map), wave, charsmax(wave));
			if (map[0] && !equali(map, g_mapname))
				fprintf(out, "%s %s^n", map, wave);
		}
		fclose(in);
	}
	fprintf(out, "%s %d^n", g_mapname, g_record);
	fclose(out);
	delete_file(path);
	rename_file(tmp, path, 1);
}
