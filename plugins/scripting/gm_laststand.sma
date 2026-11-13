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
 * lst_creatures, lst_defenders, lst_wait_human (1: the stand waits for a person on CT),
 * lst_overrun, lst_last_chance, lst_lives (Nightmare), lst_base_radius (laststand.cfg explains them),
 * lst_buy_anywhere, lst_shop_scale. docs/COMPONENTS.md has the components; docs/CREATURES.md the
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

new g_cWaves, g_cDiff, g_cPrepare, g_cBreak, g_cRevive, g_cDrops, g_cCreatures, g_cDefenders, g_cWaveMoney, g_cWaitHuman, g_cOverrun, g_cLastChance, g_cLives, g_cBaseRadius, g_cBuyAnywhere, g_cShopScale;

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
new g_hePack[33];
new g_keeper;
new g_keeperKind;	/* 0 none, 1 barney, 2 gman, 3 a CS model standing in */
new bool:g_inShop[33];
new g_shopVisits[33];
new g_repairing[33];
new Float:g_repairNext[33];
new Float:g_repairUntil[33];
new Float:g_repairResend[33];
new g_botRepaired;
new Float:g_beatAt;
new Float:g_stuckAt;
new Float:g_crPos[2048][3];
new g_crStill[2048];
new g_overrun;
new Float:g_lastChanceEnd;
new g_lives;
new Float:g_spots[10][3];
new g_nspots;
new g_role[33];
new g_spot[33];
new Float:g_nextShift[33];
new Float:g_waveAt;

new bool:g_down[33];
new Float:g_downAt[33];
new Float:g_downOrg[33][3];
new g_revBy[33];
new Float:g_revProg[33];

new g_msgShake, g_msgRoundTime;
new g_sprBeam;

/* waves.ini [laststand] (see load_waves) */
#define MAX_ROWS 32
#define MAX_MIX 8
new g_wRowFrom[MAX_ROWS], g_wRowTo[MAX_ROWS];
new g_wMixType[MAX_ROWS][MAX_MIX][16];
new g_wMixW[MAX_ROWS][MAX_MIX];
new g_wMixN[MAX_ROWS];
new g_nrows;
new g_wBoss[16] = "boss";
new g_wBossEvery = 5;
new Float:g_wCount = 1.0;
new Float:g_wHp = 1.0;
new bool:g_wTypeHp;

/* =========================================================================
 * Start-up
 * ========================================================================= */

public plugin_precache()
{
	g_sprBeam = precache_model("sprites/laserbeam.spr");
	/* creature types can only spawn mid-map if they were precached as the map loaded */
	g_hasCreatureDll = get_cvar_pointer("ls_creature_ver") != 0;
	g_cCreatures = register_cvar("lst_creatures", "1");
	keeper_precache();
	load_waves();
	if (g_hasCreatureDll && get_pcvar_num(g_cCreatures))
	{
		/* only the types this map's stand will use (each type loads its model and sounds) */
		new list[256];
		precache_types(list, charsmax(list));
		set_cvar_string("ls_creature_precache", list);
		log_amx("Last Stand: creature types for this map: %s", list);
	}
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
	g_cWaitHuman = register_cvar("lst_wait_human", "1");
	g_cOverrun = register_cvar("lst_overrun", "15");
	g_cLastChance = register_cvar("lst_last_chance", "30");
	g_cLives = register_cvar("lst_lives", "5");
	g_cBaseRadius = register_cvar("lst_base_radius", "450");
	g_cBuyAnywhere = register_cvar("lst_buy_anywhere", "0");
	g_cShopScale = register_cvar("lst_shop_scale", "1.0");
	register_clcmd("say /shop", "cmd_shop");
	register_clcmd("say_team /shop", "cmd_shop");
	register_clcmd("lst_shop", "cmd_shop");
	register_forward(FM_PlayerPreThink, "fm_prethink");
	register_forward(FM_SetModel, "fm_setmodel_post", 1);

	g_msgShake = get_user_msgid("ScreenShake");
	g_msgRoundTime = get_user_msgid("RoundTime");

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");
	RegisterHamPlayer(Ham_Killed, "ham_player_killed", 1);
	RegisterHamPlayer(Ham_Spawn, "ham_player_spawn", 1);
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
	set_task(0.2, "keeper_tick", TASK_REV + 1, _, _, "b");
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
	{
		clear_down(p);
		g_repairing[p] = 0;
	}
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
	clear_defenders();
	for (new p = 1; p <= MaxClients; p++)
		g_role[p] = 0;
	g_phase = PH_PREPARE;
	g_overrun = 0;
	g_lastChanceEnd = 0.0;
	g_lives = get_pcvar_num(g_cLives);
	g_phaseEnd = get_gametime() + get_pcvar_float(g_cPrepare);
	set_cvar_num("mp_buy_anywhere", get_pcvar_num(g_cBuyAnywhere) ? 1 : 0);
	find_shop();
	keeper_spawn();
	round_clock(floatround(get_pcvar_float(g_cPrepare)));
	ls_announce(0, 5.0, "Last Stand: hold the base! First wave in %d seconds", get_pcvar_num(g_cPrepare));
	bots_to_shop(get_pcvar_num(g_cPrepare));
	if (g_record > 0)
		ls_chat(0, "Best on %s so far: wave %d.", g_mapname, g_record);
	ls_chat(0, "Buy at the base, or press USE at the armory (say /shop) for repair kits, armour, grenades and ammo.");
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

/* Defenders, spread out: about 6 in 10 bots hold one of a few spots (behind each standing
   barricade, half-way to it, the base) and move to another every minute or so; the rest have
   no goal at all and play on their own (hunting creatures, reacting to noise). Goals are low
   priority with a wide radius, sent only when a wave starts or a bot shifts, and cleared
   between waves so bots buy, reload and wander like they normally do. */
stock build_spots()
{
	g_nspots = 0;
	add_spot(g_base);
	for (new b = ls_comp_find("ls_barricade"); b && g_nspots < sizeof g_spots - 1; b = ls_comp_find("ls_barricade", b))
	{
		if (ls_barricade_hp(b) <= 0)
			continue;
		new Float:mn[3], Float:mx[3], Float:c[3], Float:p[3];
		pev(b, pev_absmin, mn);
		pev(b, pev_absmax, mx);
		for (new i = 0; i < 3; i++)
			c[i] = (mn[i] + mx[i]) * 0.5;
		new Float:d = get_distance_f(c, g_base);
		if (d < 1.0)
			continue;
		/* behind it, on the base's side */
		new Float:f = floatmin(200.0 / d, 0.9);
		for (new i = 0; i < 3; i++)
			p[i] = c[i] + (g_base[i] - c[i]) * f;
		add_spot(p);
		/* half-way back */
		for (new i = 0; i < 3; i++)
			p[i] = (c[i] + g_base[i]) * 0.5;
		add_spot(p);
	}
	/* no barricades: a ring round the base */
	if (g_nspots == 1)
		for (new k = 0; k < 4; k++)
		{
			new Float:p[3];
			p[0] = g_base[0] + 300.0 * floatcos(90.0 * float(k), degrees);
			p[1] = g_base[1] + 300.0 * floatsin(90.0 * float(k), degrees);
			p[2] = g_base[2];
			add_spot(p);
		}
}

stock add_spot(const Float:p[3])
{
	if (g_nspots >= sizeof g_spots)
		return;
	g_spots[g_nspots][0] = p[0];
	g_spots[g_nspots][1] = p[1];
	g_spots[g_nspots][2] = p[2];
	g_nspots++;
}

stock assign_defenders()
{
	build_spots();
	new k = random(max(g_nspots, 1));
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		if (!g_role[p])
			g_role[p] = random_num(1, 10) <= 6 ? 1 : 2;
		if (g_role[p] == 1)
			send_hold(p, k++);
	}
}

stock send_hold(p, k)
{
	if (!g_nspots)
		return;
	new spot = k % g_nspots;
	g_spot[p] = spot;
	new secs = random_num(45, 80);
	g_nextShift[p] = get_gametime() + float(secs);
	new line[96];
	formatex(line, charsmax(line), "goal %d defend %.0f %.0f %.0f 0 35 %d 350", p, g_spots[spot][0], g_spots[spot][1], g_spots[spot][2], secs);
	ls_comp_bots_hint(line);
}

/* a holder whose time is up moves to another spot */
stock shift_defenders()
{
	new Float:now = get_gametime();
	for (new p = 1; p <= MaxClients; p++)
	{
		if (g_role[p] != 1 || now < g_nextShift[p] || !is_user_alive(p) || !is_user_bot(p))
			continue;
		build_spots();
		send_hold(p, g_spot[p] + random_num(1, max(g_nspots - 1, 1)));
	}
}

stock clear_defenders()
{
	ls_comp_bots_hint("goal_clear ct defend");
}

/* =========================================================================
 * Waves
 * ========================================================================= */

/* Easy gives longer, Nightmare shorter */
stock Float:time_scale()
{
	switch (clamp(get_pcvar_num(g_cDiff), 0, 3))
	{
		case 0: return 1.5;
		case 2: return 0.7;
		case 3: return 0.5;
	}
	return 1.0;
}

/* the base: a component named lst_base (a map's inner room), else lst_base_radius round the base */
stock bool:in_base(e, Float:margin = 0.0)
{
	new Float:o[3];
	pev(e, pev_origin, o);
	new b = ls_comp_by_name("lst_base");
	if (b)
	{
		if ((1 <= e <= MaxClients) && margin <= 0.0)
			return ls_comp_inside(b, e);
		new Float:bo[3], sz[32];
		pev(b, pev_origin, bo);
		if (ls_comp_get(b, "size", sz, charsmax(sz)))
		{
			new a[3][12];
			parse(sz, a[0], 11, a[1], 11, a[2], 11);
			return floatabs(o[0] - bo[0]) <= str_to_float(a[0]) * 0.5 + margin && floatabs(o[1] - bo[1]) <= str_to_float(a[1]) * 0.5 + margin
				&& o[2] > bo[2] - 64.0 - margin && o[2] < bo[2] + str_to_float(a[2]) + 64.0 + margin;
		}
		new Float:r = ls_comp_getf(b, "radius", 300.0) + margin;
		return floatsqroot((o[0] - bo[0]) * (o[0] - bo[0]) + (o[1] - bo[1]) * (o[1] - bo[1])) <= r && floatabs(o[2] - bo[2]) < 160.0 + margin;
	}
	new Float:r = get_pcvar_float(g_cBaseRadius) + margin;
	return floatsqroot((o[0] - g_base[0]) * (o[0] - g_base[0]) + (o[1] - g_base[1]) * (o[1] - g_base[1])) <= r && floatabs(o[2] - g_base[2]) < 200.0 + margin;
}

/* sets why when the stand is lost this second */
stock lose_checks(why[], len)
{
	new Float:now = get_gametime();
	/* overrun: enemies in the base and no defender alive in it, for lst_overrun seconds */
	new limit = floatround(get_pcvar_float(g_cOverrun) * time_scale());
	if (get_pcvar_num(g_cOverrun) > 0)
	{
		/* genuinely abandoned: two or more enemies (or the boss) inside the base, and no
		   defender in it or fighting close outside it (350 u round it) */
		new enemies = 0, inside = 0;
		if (g_creatures)
		{
			new e = -1, type[16];
			while ((e = find_ent_by_class(e, "ls_creature")) > 0 && enemies < 2)
				if (pev(e, pev_deadflag) == DEAD_NO && in_base(e))
				{
					pev(e, pev_netname, type, charsmax(type));
					enemies += equal(type, g_wBoss) ? 2 : 1;
				}
		}
		else
			for (new p = 1; p <= MaxClients && enemies < 2; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T && in_base(p))
					enemies++;
		for (new p = 1; p <= MaxClients && !inside; p++)
			if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_CT && in_base(p, 350.0))
				inside++;
		if (enemies >= 2 && !inside)
		{
			g_overrun++;
			if (g_overrun >= limit)
			{
				copy(why, len, "The base was overrun");
				return;
			}
			ls_announce(0, 1.1, "Base being overrun! %d", limit - g_overrun);
		}
		else
			g_overrun = 0;
	}
	/* people: when every human defender is down or out, the bots get a last chance */
	new humans = 0, up = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		humans++;
		if (is_user_alive(p))
			up++;
	}
	new chance = floatround(get_pcvar_float(g_cLastChance) * time_scale());
	if (humans && !up && get_pcvar_num(g_cLastChance) > 0)
	{
		if (g_lastChanceEnd <= 0.0)
		{
			g_lastChanceEnd = now + float(chance);
			ls_announce(0, 3.0, "Every player is down! The bots have %d seconds to hold or revive you", chance);
		}
		else if (now >= g_lastChanceEnd)
		{
			copy(why, len, "The bots couldn't hold without you");
			return;
		}
	}
	else
		g_lastChanceEnd = 0.0;
}

stock bool:nightmare()
{
	return get_pcvar_num(g_cDiff) >= 3;
}

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
	g_overrun = 0;
	g_lastChanceEnd = 0.0;
	g_bossDone = g_wBossEvery <= 0 || !g_wBoss[0] || (n % g_wBossEvery) != 0;
	g_spawned = 0;
	g_failSpawns = 0;
	set_cvar_num("mp_buy_anywhere", 0);
	/* the shop trips end; anyone still waiting for a revive gets their helpers back */
	ls_comp_bots_hint("goal_clear ct use");
	for (new d = 1; d <= MaxClients; d++)
		if (g_down[d])
			send_revive(d);

	/* how many: grows each wave, with the team's size and the difficulty */
	new total = floatround((5.0 + 2.5 * float(n - 1)) * (0.5 + 0.2 * float(defenders())) * diff_mult(false) * g_wCount);
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
	if (g_botRepaired > 0)
		log_amx("Last Stand: bots repaired %d barricade health before wave %d", g_botRepaired, n);
	g_botRepaired = 0;
	assign_defenders();
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
	new type[16];
	new bool:boss = pick_type(type, charsmax(type));
	new Float:o[3];
	o[0] = origin[0];
	o[1] = origin[1];
	o[2] = floor_under(origin) + 1.0;
	new cr = spawn_creature(type, o);
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
	if (boss)
		g_bossDone = true;
	/* health: Last Stand's own per type (or the type's own from creatures.ini with type_hp 1),
	   scaled by wave, team size, difficulty and waves.ini's hp_scale */
	new Float:own = entity_get_float(cr, EV_FL_health), Float:base = own;
	if (!g_wTypeHp)
		for (new i = 0; i < sizeof TYPES; i++)
			if (equal(type, TYPES[i]))
				base = float(TYPE_HP[i]);
	new Float:hp = floatmax(base * hp_mult() * g_wHp, 1.0);
	entity_set_float(cr, EV_FL_health, hp);
	entity_set_float(cr, EV_FL_max_health, hp);
	ls_wave_track(cr);
	g_spawned++;
	return PLUGIN_HANDLED;
}

/* the type for the next enemy (true = the wave's boss) */
stock bool:pick_type(out[], len)
{
	if (!g_bossDone)
	{
		copy(out, len, g_wBoss);
		return true;
	}
	if (g_nrows)
	{
		new r = wave_row(g_wave);
		new total = 0;
		for (new m = 0; m < g_wMixN[r]; m++)
			total += g_wMixW[r][m];
		if (total > 0)
		{
			new pick = random(total);
			for (new m = 0; m < g_wMixN[r]; m++)
			{
				pick -= g_wMixW[r][m];
				if (pick < 0)
				{
					copy(out, len, g_wMixType[r][m]);
					return false;
				}
			}
		}
	}
	copy(out, len, TYPES[builtin_type()]);
	return false;
}

/* the row for wave n: the one covering it, else the closest one before it, else the first */
stock wave_row(n)
{
	new best = -1, bestFrom = -1;
	for (new r = 0; r < g_nrows; r++)
	{
		if (g_wRowFrom[r] <= n && (g_wRowTo[r] == 0 || n <= g_wRowTo[r]))
			return r;
		if (g_wRowFrom[r] <= n && g_wRowFrom[r] > bestFrom)
		{
			bestFrom = g_wRowFrom[r];
			best = r;
		}
	}
	return best >= 0 ? best : 0;
}

/* Last Stand's own mix when waves.ini has no [laststand] */
stock builtin_type()
{
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

stock spawn_creature(const type[], const Float:o[3])
{
	new ent = create_entity("ls_creature");
	if (!ent)
		return 0;
	DispatchKeyValue(ent, "type", type);
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
	clear_defenders();
	new brk = get_pcvar_num(g_cBreak);
	g_phaseEnd = get_gametime() + float(brk);
	set_cvar_num("mp_buy_anywhere", get_pcvar_num(g_cBuyAnywhere) ? 1 : 0);
	round_clock(brk);
	bots_to_shop(brk);
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
			/* Nightmare: coming back costs one of the team's shared lives */
			if (nightmare())
			{
				if (g_lives <= 0)
					continue;
				g_lives--;
			}
			ExecuteHamB(Ham_CS_RoundRespawn, p);
		}
	}
	new drops = get_pcvar_num(g_cDrops);
	if (drops > 0 && g_wave % drops == 0)
		supply_drop();
	ls_announce(0, 4.0, "Wave %d held! Next wave in %d seconds - restock at the armory", g_wave, brk);
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
	/* lst_wait_human: bots alone don't play the stand; a person joining gets a fresh one */
	new humans = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && cs_get_user_team(p) == CS_TEAM_CT)
			humans++;
	if (get_pcvar_num(g_cWaitHuman) && humans == 0)
	{
		if (g_phase == PH_PREPARE)
		{
			g_phaseEnd = now + get_pcvar_float(g_cPrepare);
			hud_wait();
			return;
		}
		if (g_phase == PH_WAVE || g_phase == PH_BREAK)
		{
			/* everyone left mid-stand: start over, and wait */
			g_phase = PH_OVER;
			server_cmd("endround");
			return;
		}
	}
	/* a person who joined during the prepare time or a break plays straight away */
	if (g_phase == PH_PREPARE || g_phase == PH_BREAK)
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && !is_user_bot(p) && cs_get_user_team(p) == CS_TEAM_CT && !is_user_alive(p) && !g_down[p])
				ExecuteHamB(Ham_CS_RoundRespawn, p);
	if ((g_phase == PH_PREPARE || g_phase == PH_BREAK) && now >= g_phaseEnd)
		start_wave(g_wave + 1);

	/* lost: nobody standing, the base overrun, or no person left and the bots' last chance gone */
	new alive = 0, team = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		team++;
		if (is_user_alive(p))
			alive++;
	}
	new why[64];
	if (team > 0 && alive == 0 && g_phase == PH_WAVE)
		copy(why, charsmax(why), "Nobody left standing");
	else if (g_phase == PH_WAVE)
		lose_checks(why, charsmax(why));
	if (why[0])
	{
		g_phase = PH_OVER;
		ls_chat(0, "%s.", why);
		if (g_wave - 1 > g_record)
		{
			g_record = g_wave - 1;
			record_save();
		}
		ls_announce(0, 8.0, "The base fell on wave %d", g_wave);
		log_amx("The base fell on wave %d (%s)", g_wave, why);
		ls_chat(0, "Best on %s: wave %d.", g_mapname, g_record);
		server_cmd("endround T");
		return;
	}
	bot_repairs();
	if (g_phase == PH_WAVE)
	{
		attack_barricades();
		shift_defenders();
		if (now >= g_stuckAt)
		{
			g_stuckAt = now + 15.0;
			unstick_creatures();
		}
	}
	if (now >= g_beatAt)
	{
		g_beatAt = now + 120.0;
		log_amx("Last Stand: phase %d, wave %d, %d enemies left, %d of %d defenders standing", g_phase, g_wave, g_phase == PH_WAVE ? enemies_left() : 0, alive, team);
	}
	hud();
	beacons();
	bots_shop_tick();
	shop_hint();
}

/* A creature that hasn't moved for a minute with no defender within 800 units is stuck
   somewhere nobody can reach: it goes, so a wave can't hang on it */
stock unstick_creatures()
{
	if (!g_creatures)
		return;
	new e = -1, Float:o[3], Float:po[3];
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e >= sizeof g_crPos || pev(e, pev_deadflag) != DEAD_NO)
			continue;
		pev(e, pev_origin, o);
		if (get_distance_f(o, g_crPos[e]) > 32.0)
		{
			g_crPos[e] = o;
			g_crStill[e] = 0;
			continue;
		}
		if (++g_crStill[e] < 4)
			continue;
		new bool:near = false;
		for (new p = 1; p <= MaxClients && !near; p++)
		{
			if (!is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_CT)
				continue;
			pev(p, pev_origin, po);
			near = get_distance_f(o, po) < 800.0;
		}
		if (near)
			continue;
		log_amx("Last Stand: creature %d stuck at %.0f %.0f %.0f for a minute; removed", e, o[0], o[1], o[2]);
		g_crStill[e] = 0;
		server_cmd("ls_creature kill %d", e);
	}
}

/* Bots repair barricades: in breaks and the prepare time, and mid-wave only while that gate is
   quiet (no enemy within 600 units), the nearest one or two bots per damaged barricade walk to
   its inside and add a board every 1.5 s (paying like a person) until it's full */
stock bot_repairs()
{
	if (g_phase != PH_BREAK && g_phase != PH_PREPARE && g_phase != PH_WAVE)
		return;
	new Float:now = get_gametime();
	for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
	{
		new mxhp = floatround(ls_comp_getf(b, "hp", 500.0));
		new hp = ls_barricade_hp(b);
		new Float:mn[3], Float:mx[3], Float:c[3];
		pev(b, pev_absmin, mn);
		pev(b, pev_absmax, mx);
		for (new i = 0; i < 3; i++)
			c[i] = (mn[i] + mx[i]) * 0.5;
		new bool:calm = !enemy_near(c, 600.0);
		new want = hp >= mxhp - 1 || !calm ? 0 : (g_phase == PH_WAVE ? 1 : 2);
		/* who's on it now; release them when it's full or the gate turns hot */
		new have = 0;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (g_repairing[p] != b)
				continue;
			if (!want || !is_user_alive(p) || now >= g_repairUntil[p])
			{
				release_repair(p);
				continue;
			}
			have++;
			/* the brain's use goal ends after ~6 s there: send it again while there's work */
			if (now >= g_repairResend[p])
				send_repair_goal(p, b, c, mn);
			/* close to it: a board every 1.5 s */
			if (near_box(p, mn, mx, 110.0) && now >= g_repairNext[p])
			{
				g_repairNext[p] = now + 1.5;
				new cost = floatround(ls_comp_getf(b, "repair_cost", 0.0));
				new money = cs_get_user_money(p);
				if (money >= cost)
				{
					cs_set_user_money(p, money - cost);
					new before = ls_barricade_hp(b);
					ls_barricade_repair(b, p);
					g_botRepaired += ls_barricade_hp(b) - before;
				}
			}
		}
		/* more hands: the nearest free bots */
		while (have < want)
		{
			new best = 0, Float:bd = 2500.0, Float:o[3];
			for (new p = 1; p <= MaxClients; p++)
			{
				if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT || g_repairing[p])
					continue;
				pev(p, pev_origin, o);
				new Float:d = get_distance_f(o, c);
				if (d < bd)
				{
					bd = d;
					best = p;
				}
			}
			if (!best)
				break;
			g_repairing[best] = b;
			g_repairNext[best] = now + 1.5;
			g_repairUntil[best] = now + 40.0;
			send_repair_goal(best, b, c, mn);
			have++;
		}
	}
}

/* walk to the base's side of it and work on it */
stock send_repair_goal(p, b, const Float:c[3], const Float:mn[3])
{
	g_repairResend[p] = get_gametime() + 6.0;
	new Float:to[3];
	new Float:dx = g_base[0] - c[0], Float:dy = g_base[1] - c[1];
	new Float:l = floatmax(floatsqroot(dx * dx + dy * dy), 1.0);
	to[0] = c[0] + dx / l * 56.0;
	to[1] = c[1] + dy / l * 56.0;
	to[2] = mn[2] + 1.0;
	new line[96];
	formatex(line, charsmax(line), "goal %d use %.0f %.0f %.0f %d 50 10 64", p, to[0], to[1], to[2], b);
	ls_comp_bots_hint(line);
}

stock release_repair(p)
{
	if (!g_repairing[p])
		return;
	new line[48];
	formatex(line, charsmax(line), "goal_clear %d %d", p, g_repairing[p]);
	ls_comp_bots_hint(line);
	g_repairing[p] = 0;
}

stock bool:enemy_near(const Float:c[3], Float:r)
{
	new Float:o[3];
	if (g_creatures)
	{
		new e = -1;
		while ((e = find_ent_by_class(e, "ls_creature")) > 0)
		{
			if (pev(e, pev_deadflag) != DEAD_NO)
				continue;
			pev(e, pev_origin, o);
			if (get_distance_f(o, c) < r)
				return true;
		}
		return false;
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_T)
			continue;
		pev(p, pev_origin, o);
		if (get_distance_f(o, c) < r)
			return true;
	}
	return false;
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
				if (pev(e, pev_deadflag) == DEAD_NO && near_box(e, mn, mx, 56.0) && outside(e, mn, mx))
				{
					ExecuteHamB(Ham_TakeDamage, b, e, e, dmg, DMG_SLASH);
					hits++;
				}
		}
		else
		{
			for (new p = 1; p <= MaxClients && hits < 6; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T && near_box(p, mn, mx, 48.0) && outside(p, mn, mx))
				{
					ExecuteHamB(Ham_TakeDamage, b, p, p, dmg, DMG_SLASH);
					hits++;
				}
		}
	}
}

/* on the far side of a barricade from the base: only enemies still outside hit it */
stock bool:outside(e, const Float:mn[3], const Float:mx[3])
{
	new Float:o[3], Float:c[3], Float:v[3];
	pev(e, pev_origin, o);
	pev(e, pev_velocity, v);
	for (new i = 0; i < 3; i++)
		c[i] = (mn[i] + mx[i]) * 0.5;
	new Float:tbx = g_base[0] - c[0], Float:tby = g_base[1] - c[1];
	if ((o[0] - c[0]) * tbx + (o[1] - c[1]) * tby >= 0.0)
		return false;
	/* and trying to get through: pressed against it (barely moving) or heading in */
	new Float:speed = floatsqroot(v[0] * v[0] + v[1] * v[1]);
	return speed < 60.0 || v[0] * (c[0] - o[0]) + v[1] * (c[1] - o[1]) > 0.0;
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
			formatex(top, charsmax(top), "Get ready: first wave in %d s", max(floatround(g_phaseEnd - now), 0));
		case PH_BREAK:
			formatex(top, charsmax(top), "Wave %d held - next in %d s", g_wave, max(floatround(g_phaseEnd - now), 0));
		case PH_WAVE:
		{
			if (waves > 0)
				formatex(top, charsmax(top), "WAVE %d/%d  -  %d left", g_wave, waves, enemies_left());
			else
				formatex(top, charsmax(top), "WAVE %d  -  %d left  (best %d)", g_wave, enemies_left(), g_record);
			if (g_lastChanceEnd > 0.0)
				format(top, charsmax(top), "%s^nLAST CHANCE: %d s", top, max(floatround(g_lastChanceEnd - now), 0));
		}
	}
	if (nightmare() && g_phase != PH_IDLE)
		format(top, charsmax(top), "%s   Lives %d", top, g_lives);
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
		/* the team and the nearest barricade on the right: menus open on the left, the
		   radar is top left and chat bottom left, so nothing here ever sits under them */
		new right[480], bartxt[48];
		new bar = nearest_barricade(p);
		if (bar)
		{
			new mx = floatround(ls_comp_getf(bar, "hp", 500.0));
			new hp = ls_barricade_hp(bar);
			if (hp > 0)
				formatex(bartxt, charsmax(bartxt), "^nBarricade %d%%", 100 * hp / max(mx, 1));
			else
				copy(bartxt, charsmax(bartxt), "^nBarricade DOWN");
		}
		formatex(right, charsmax(right), "%s%s", team, bartxt);
		ls_hud(p, LS_HUD_RIGHT, 1.1, "%s", right);
	}
}

stock hud_wait()
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "Last Stand starts when a player joins the Counter-Terrorists");
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
	g_hePack[victim] = 0;
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
	new name[32];
	get_user_name(victim, name, charsmax(name));
	ls_chat(0, "%s is down! Hold USE on the blue beam to revive them.", name);
	send_revive(victim);
	return HAM_IGNORED;
}

/* the two nearest bots go and hold USE on a downed player (W2: kind use, prio 75) */
stock send_revive(victim)
{
	new line[96];
	new near[2], Float:nd[2] = { 99999.0, 99999.0 };
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == victim || !is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		new Float:o[3];
		pev(p, pev_origin, o);
		new Float:d = get_distance_f(o, g_downOrg[victim]);
		if (d < nd[0])
		{
			nd[1] = nd[0]; near[1] = near[0];
			nd[0] = d; near[0] = p;
		}
		else if (d < nd[1])
		{
			nd[1] = d; near[1] = p;
		}
	}
	for (new i = 0; i < 2; i++)
	{
		if (!near[i])
			continue;
		new left = max(floatround(get_pcvar_float(g_cRevive) - (get_gametime() - g_downAt[victim])), 1);
		formatex(line, charsmax(line), "goal %d use %.0f %.0f %.0f %d 75 %d 64", near[i], g_downOrg[victim][0], g_downOrg[victim][1], g_downOrg[victim][2], victim, left);
		ls_comp_bots_hint(line);
	}
}

stock clear_down(p)
{
	if (g_down[p])
	{
		new line[48];
		formatex(line, charsmax(line), "goal_clear ct %d", p);
		/* (ent = the downed player: only the revive goals go) */
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
 * The armory: Last Stand's own shop (CS buying stays the map's buy zone)
 * ========================================================================= */

new Float:g_shop[3];
new Float:g_useNext[33];
new Float:g_menuUntil[33];

new const SHOP_NAME[][] = { "Barricade repair kit", "Armour", "3 HE grenades", "Ammo", "Medkit" };
new const SHOP_WHAT[][] = { "rebuild the most broken barricade fully", "kevlar + helmet", "three HE grenades", "refill all your guns", "+50 HP" };
new const SHOP_PRICE[] = { 600, 650, 900, 300, 500 };
new g_shopCb;

/* an entity or component named lst_shop (the map's armory), else the base */
stock find_shop()
{
	new e = ls_comp_by_name("lst_shop");
	if (!e)
		e = find_ent_by_tname(-1, "lst_shop");
	if (e > 0)
	{
		new Float:mn[3], Float:mx[3];
		pev(e, pev_absmin, mn);
		pev(e, pev_absmax, mx);
		if (mx[0] > mn[0])
			for (new i = 0; i < 3; i++)
				g_shop[i] = (mn[i] + mx[i]) * 0.5;
		else
			pev(e, pev_origin, g_shop);
	}
	else if (!buyzone_spot(g_shop))
		g_shop = g_base;
}

/* stock maps: a clear spot in the CT buy zone nearest the base, off the spawn points */
stock bool:buyzone_spot(Float:out[3])
{
	new e = -1, best = 0, Float:bd = 99999.0, Float:c[3];
	while ((e = find_ent_by_class(e, "func_buyzone")) > 0)
	{
		new Float:mn[3], Float:mx[3], Float:m[3];
		pev(e, pev_absmin, mn);
		pev(e, pev_absmax, mx);
		for (new i = 0; i < 3; i++)
			m[i] = (mn[i] + mx[i]) * 0.5;
		new Float:d = get_distance_f(m, g_base);
		if (d < bd)
		{
			bd = d;
			best = e;
			c = m;
		}
	}
	if (!best || bd > 1500.0)
		return false;
	for (new ring = 0; ring <= 6; ring++)
	{
		for (new k = 0; k < 8; k++)
		{
			new Float:p[3];
			p[0] = c[0] + 60.0 * float(ring) * floatcos(45.0 * float(k), degrees);
			p[1] = c[1] + 60.0 * float(ring) * floatsin(45.0 * float(k), degrees);
			p[2] = c[2];
			p[2] = floor_under(p) + 37.0;
			engfunc(EngFunc_TraceHull, p, p, IGNORE_MONSTERS, HULL_HUMAN, 0, 0);
			if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
				continue;
			new bool:onspawn = false, s = -1, Float:so[3];
			while ((s = find_ent_by_class(s, "info_player_start")) > 0 && !onspawn)
			{
				pev(s, pev_origin, so);
				onspawn = get_distance_f(so, p) < 40.0;
			}
			if (onspawn)
				continue;
			out = p;
			return true;
		}
		if (ring == 0)
			continue;
	}
	return false;
}

stock bool:at_shop(id)
{
	new Float:o[3];
	pev(id, pev_origin, o);
	return get_distance_f(o, g_shop) < 160.0 && floatabs(o[2] - g_shop[2]) < 120.0;
}

/* 5 % dearer each wave, rounded to $50 so prices stay readable */
stock shop_price(i)
{
	new Float:p = float(SHOP_PRICE[i]) * (1.0 + 0.05 * float(max(g_wave - 1, 0))) * get_pcvar_float(g_cShopScale);
	return max(floatround(p / 50.0) * 50, 50);
}

/* why an item can't be bought now ("" = it can) */
stock shop_why_not(id, i, out[], len)
{
	out[0] = EOS;
	switch (i)
	{
		case 0: if (!most_broken_barricade()) copy(out, len, "the barricades are fine");
		case 1:
		{
			new CsArmorType:t;
			if (cs_get_user_armor(id, t) >= 100 && t == CS_ARMOR_VESTHELM)
				copy(out, len, "you have full armour");
		}
		case 2: if (cs_get_user_bpammo(id, CSW_HEGRENADE) >= 3) copy(out, len, "you have 3 already");
		case 4: if (get_user_health(id) >= 100) copy(out, len, "you're at full health");
	}
	if (!out[0] && cs_get_user_money(id) < shop_price(i))
		copy(out, len, "not enough money");
}

stock most_broken_barricade()
{
	new best = 0, worst = 100;
	for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
	{
		new mx = floatround(ls_comp_getf(b, "hp", 500.0));
		new pct = 100 * ls_barricade_hp(b) / max(mx, 1);
		if (pct < worst)
		{
			worst = pct;
			best = b;
		}
	}
	return best;
}

public shop_item_cb(id, menu, item)
{
	new info[4], name[2], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, 0, cb);
	new i = str_to_num(info);
	if (i < 0 || i >= sizeof SHOP_NAME)
		return ITEM_ENABLED;
	new why[48];
	shop_why_not(id, i, why, charsmax(why));
	return why[0] ? ITEM_DISABLED : ITEM_ENABLED;
}

public cmd_shop(id)
{
	if (g_phase == PH_IDLE || !is_user_alive(id))
		return PLUGIN_HANDLED;
	if (!at_shop(id))
	{
		ls_chat(id, "The armory is at the base: go there and press USE (or say /shop).");
		return PLUGIN_HANDLED;
	}
	shop_menu(id);
	return PLUGIN_HANDLED;
}

stock shop_menu(id)
{
	if (!g_shopCb)
		g_shopCb = menu_makecallback("shop_item_cb");
	new title[64];
	formatex(title, charsmax(title), "\yArmory  \d(you have $%d)", cs_get_user_money(id));
	new m = menu_create(title, "menu_shop");
	new text[128], info[4], why[48];
	for (new i = 0; i < sizeof SHOP_NAME; i++)
	{
		shop_why_not(id, i, why, charsmax(why));
		if (why[0])
			formatex(text, charsmax(text), "\d%s - %s ($%d)  \r%s", SHOP_NAME[i], SHOP_WHAT[i], shop_price(i), why);
		else
			formatex(text, charsmax(text), "%s \d- %s \y($%d)", SHOP_NAME[i], SHOP_WHAT[i], shop_price(i));
		num_to_str(i, info, charsmax(info));
		menu_additem(m, text, info, 0, g_shopCb);
	}
	menu_additem(m, "Open the gun menu \d(B)", "99");
	g_menuUntil[id] = get_gametime() + 30.0;
	menu_display(id, m);
}

public menu_shop(id, menu, item)
{
	if (item < 0)
	{
		g_menuUntil[id] = 0.0;
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[4], name[64], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	menu_destroy(menu);
	g_menuUntil[id] = 0.0;
	if (str_to_num(info) == 99)
	{
		/* the game's own buy menu (works here: the armory is a buy zone) */
		engclient_cmd(id, "buy");
		return PLUGIN_HANDLED;
	}
	if (is_user_alive(id) && at_shop(id))
		shop_buy(id, str_to_num(info), true);
	return PLUGIN_HANDLED;
}

stock bool:shop_buy(id, i, bool:tell)
{
	new price = shop_price(i), money = cs_get_user_money(id);
	new why[48];
	shop_why_not(id, i, why, charsmax(why));
	if (why[0])
	{
		if (tell)
			client_print(id, print_center, "%s: %s", SHOP_NAME[i], why);
		return false;
	}
	switch (i)
	{
		case 0: ls_barricade_repair(most_broken_barricade(), 0);
		case 1: cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM);
		case 2:
		{
			if (!user_has_weapon(id, CSW_HEGRENADE))
				ls_give_item(id, "he");
			cs_set_user_bpammo(id, CSW_HEGRENADE, 3);
			/* and a promise of 3: if anything caps the count at 1, each throw hands the next one */
			g_hePack[id] = 3;
		}
		case 3: ls_give_item(id, "ammo");
		case 4: set_user_health(id, min(get_user_health(id) + 50, 100));
	}
	cs_set_user_money(id, money - price);
	if (tell)
	{
		client_print(id, print_center, "%s bought", SHOP_NAME[i]);
		keeper_thanks(id);
	}
	return true;
}

/* an HE grenade leaving someone's hand: one less owed from their pack */
public fm_setmodel_post(ent, const model[])
{
	if (!equal(model, "models/w_hegrenade.mdl"))
		return FMRES_IGNORED;
	new id = pev(ent, pev_owner);
	if (!(1 <= id <= MaxClients) || g_hePack[id] <= 0)
		return FMRES_IGNORED;
	g_hePack[id]--;
	if (g_hePack[id] > 0)
		set_task(0.3, "task_he_next", TASK_REV + 100 + id);
	return FMRES_IGNORED;
}

public task_he_next(tid)
{
	new id = tid - TASK_REV - 100;
	if (!is_user_alive(id) || g_hePack[id] <= 0)
		return;
	if (!user_has_weapon(id, CSW_HEGRENADE))
		ls_give_item(id, "he");
	if (cs_get_user_bpammo(id, CSW_HEGRENADE) < g_hePack[id])
		cs_set_user_bpammo(id, CSW_HEGRENADE, g_hePack[id]);
}

/* USE at the armory opens the shop; a hint when close */
public fm_prethink(id)
{
	if (g_phase == PH_IDLE || g_phase == PH_OVER || !is_user_alive(id) || is_user_bot(id))
		return FMRES_IGNORED;
	new b = pev(id, pev_button), ob = pev(id, pev_oldbuttons);
	if ((b & IN_USE) && !(ob & IN_USE) && get_gametime() >= g_useNext[id] && at_shop(id))
	{
		g_useNext[id] = get_gametime() + 1.0;
		shop_menu(id);
	}
	return FMRES_IGNORED;
}

/* bots: some go to the armory at a break, stock up, and go back to playing */
stock bots_to_shop(secs)
{
	new line[96];
	formatex(line, charsmax(line), "goal ct use %.0f %.0f %.0f 0 25 %d 120", g_shop[0], g_shop[1], g_shop[2], secs);
	ls_comp_bots_hint(line);
}

stock bots_shop_tick()
{
	if (g_phase != PH_BREAK && g_phase != PH_PREPARE)
		return;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT || !at_shop(p))
			continue;
		if (get_gametime() < g_useNext[p])
			continue;
		g_useNext[p] = get_gametime() + 20.0;
		new CsArmorType:t;
		if (cs_get_user_armor(p, t) < 60)
			shop_buy(p, 1, false);
		shop_buy(p, 3, false);
		if (random_num(0, 2) == 0)
			shop_buy(p, 2, false);
		if (random_num(0, 3) == 0)
			shop_buy(p, 0, false);
	}
}

/* Both ways to buy, made obvious: on walking into the armory a centre message (in full the first
   3 visits on a map, short after), a line on the HUD while inside, and the shopkeeper says it */
/* walking out: shut the supplies menu and the game's buy menu, clear the armory line */
stock leave_shop(p)
{
	if (g_menuUntil[p] > 0.0)
	{
		g_menuUntil[p] = 0.0;
		menu_cancel(p);
	}
	new menu, keys;
	get_user_menu(p, menu, keys);
	if (menu != 0)
		show_menu(p, 0, "^n", 1);
	/* the buy menu (VGUI on the Steam client): the game's own "close" */
	client_cmd(p, "slot10");
	ls_hud_clear(p, LS_HUD_CENTER);
}

stock shop_hint()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || is_user_bot(p))
			continue;
		new bool:in = at_shop(p);
		if (in && !g_inShop[p])
		{
			/* walking in: a short hint (and the shopkeeper's line the first 3 visits) */
			g_shopVisits[p]++;
			client_print(p, print_center, "Armory: B for weapons - E at the counter for supplies");
			if (g_shopVisits[p] <= 3 && g_keeper && is_valid_ent(g_keeper))
			{
				if (g_keeperKind == 2)
					client_print_color(p, print_team_default, "^4G-Man:^1 Weapons... on the B menu. Supplies... from me.");
				else if (g_keeperKind == 1)
					client_print_color(p, print_team_default, "^4Barney:^1 Guns are on the B menu, supplies from me.");
			}
		}
		else if (!in && g_inShop[p])
			leave_shop(p);
		g_inShop[p] = in;
		if (in)
			ls_hud(p, LS_HUD_CENTER, 1.1, "Armory  -  B: weapons   USE: supplies");
	}
}

/* =========================================================================
 * The shopkeeper: Barney (or the G-Man) at the armory, from Half-Life's own models
 * ========================================================================= */

new g_cKeeper;
new g_keeperModel[64];
new Float:g_keeperNext;
new Float:g_keeperBackAt;
new bool:g_keeperSolid;

new const BARNEY_FIDGETS[][] = { "idle2", "idle3", "idle4" };
new const GMAN_FIDGETS[][] = { "idlebrush", "idlelook", "idle02" };
new const BARNEY_LINES[][] = { "Need ammo?", "Stay safe out there.", "That'll hold 'em.", "Watch your back, pal." };
new const GMAN_LINES[][] = { "Prepare for... the next wave.", "Wise... choice.", "Time to... choose.", "Rise and... shine." };

stock keeper_precache()
{
	g_cKeeper = register_cvar("lst_shopkeeper", "barney");
	new want[16];
	get_pcvar_string(g_cKeeper, want, charsmax(want));
	if (equali(want, "none"))
		return;
	g_keeperKind = equali(want, "gman") ? 2 : 1;
	formatex(g_keeperModel, charsmax(g_keeperModel), "models/%s.mdl", g_keeperKind == 2 ? "gman" : "barney");
	/* Half-Life's models live in valve/, which the engine reads from cstrike too */
	new alt[96];
	formatex(alt, charsmax(alt), "../valve/%s", g_keeperModel);
	if (!file_exists(g_keeperModel) && !file_exists(alt))
	{
		log_amx("Last Stand: %s not found on this server; the shopkeeper is a CS model instead.", g_keeperModel);
		copy(g_keeperModel, charsmax(g_keeperModel), "models/player/gign/gign.mdl");
		g_keeperKind = 3;
	}
	precache_model(g_keeperModel);
}

stock keeper_spawn()
{
	if (!g_keeperKind)
		return;
	if (!g_keeper || !is_valid_ent(g_keeper))
	{
		g_keeper = create_entity("info_target");
		if (!g_keeper)
			return;
		entity_set_string(g_keeper, EV_SZ_classname, "lst_shopkeeper");
		entity_set_model(g_keeper, g_keeperModel);
	}
	/* (not solid while placed, and the floor trace looks past him) */
	set_pev(g_keeper, pev_solid, SOLID_NOT);
	new Float:o[3];
	keeper_spot(o);
	entity_set_origin(g_keeper, o);
	entity_set_size(g_keeper, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 72.0});
	set_pev(g_keeper, pev_movetype, MOVETYPE_NONE);
	set_pev(g_keeper, pev_solid, SOLID_NOT);
	set_pev(g_keeper, pev_takedamage, DAMAGE_NO);
	set_pev(g_keeper, pev_controller_0, 127);
	set_pev(g_keeper, pev_controller_1, 127);
	g_keeperSolid = false;
	keeper_play(g_keeperKind == 2 ? "idle01" : "idle1");
	g_keeperNext = get_gametime() + random_float(6.0, 12.0);
}

/* by the shop marker, on the floor (a marker on a counter puts him beside it, not on it), with
   room to stand, the closest such spot */
stock keeper_spot(Float:out[3])
{
	new Float:base = floor_under(g_shop, g_keeper);
	out = g_shop;
	out[2] = base + 1.0;
	new Float:best = 99999.0, Float:lowest = base;
	for (new ring = 0; ring <= 3; ring++)
	{
		for (new k = 0; k < (ring ? 8 : 1); k++)
		{
			new Float:p[3];
			p[0] = g_shop[0] + 40.0 * float(ring) * floatcos(45.0 * float(k), degrees);
			p[1] = g_shop[1] + 40.0 * float(ring) * floatsin(45.0 * float(k), degrees);
			p[2] = g_shop[2];
			new Float:f = floor_under(p, g_keeper);
			new Float:c[3];
			c = p;
			c[2] = f + 37.0;
			engfunc(EngFunc_TraceHull, c, c, IGNORE_MONSTERS, HULL_HUMAN, g_keeper, 0);
			if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
				continue;
			/* lower floor first (the ground, not a counter), then closer */
			new Float:score = (f - lowest) * 10.0 + 40.0 * float(ring);
			if (f < lowest - 4.0)
			{
				lowest = f;
				score = 40.0 * float(ring);
				best = 99999.0;
			}
			if (score < best)
			{
				best = score;
				out[0] = p[0];
				out[1] = p[1];
				out[2] = f + 1.0;
			}
		}
	}
}

stock keeper_play(const seq[])
{
	new Float:fr, bool:loops;
	new n = lookup_sequence(g_keeper, seq, fr, loops);
	set_pev(g_keeper, pev_sequence, n >= 0 ? n : 0);
	set_pev(g_keeper, pev_frame, 0.0);
	set_pev(g_keeper, pev_framerate, 1.0);
	set_pev(g_keeper, pev_animtime, get_gametime());
}

/* faces whoever's nearest within 300 units, fidgets now and then, solid once nobody's in him */
public keeper_tick()
{
	if (!g_keeper || !is_valid_ent(g_keeper))
		return;
	new Float:now = get_gametime(), Float:o[3], Float:po[3];
	pev(g_keeper, pev_origin, o);
	new near = 0, Float:nd = 300.0;
	new bool:overlap = false;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		pev(p, pev_origin, po);
		new Float:d = get_distance_f(o, po);
		if (floatabs(po[0] - o[0]) < 34.0 && floatabs(po[1] - o[1]) < 34.0 && po[2] - o[2] < 110.0 && po[2] - o[2] > -40.0)
			overlap = true;
		if (d < nd)
		{
			nd = d;
			near = p;
		}
	}
	if (!g_keeperSolid && !overlap)
	{
		g_keeperSolid = true;
		set_pev(g_keeper, pev_solid, SOLID_BBOX);
		entity_set_origin(g_keeper, o);
	}
	if (near)
	{
		pev(near, pev_origin, po);
		new Float:ang[3];
		pev(g_keeper, pev_angles, ang);
		new Float:want = floatatan2(po[1] - o[1], po[0] - o[0], degrees);
		new Float:d = want - ang[1];
		while (d > 180.0) d -= 360.0;
		while (d < -180.0) d += 360.0;
		ang[1] += floatclamp(d, -25.0, 25.0);
		set_pev(g_keeper, pev_angles, ang);
	}
	if (g_keeperBackAt > 0.0 && now >= g_keeperBackAt)
	{
		g_keeperBackAt = 0.0;
		keeper_play(g_keeperKind == 2 ? "idle01" : "idle1");
	}
	if (now >= g_keeperNext && g_keeperBackAt <= 0.0 && g_keeperKind != 3)
	{
		g_keeperNext = now + random_float(8.0, 15.0);
		keeper_play(g_keeperKind == 2 ? GMAN_FIDGETS[random(sizeof GMAN_FIDGETS)] : BARNEY_FIDGETS[random(sizeof BARNEY_FIDGETS)]);
		g_keeperBackAt = now + 2.5;
	}
}

/* a nod (or Barney's wave) and a line after a purchase */
stock keeper_thanks(id)
{
	if (!g_keeper || !is_valid_ent(g_keeper) || g_keeperKind == 3)
		return;
	keeper_play(g_keeperKind == 2 ? (random_num(0, 1) ? "yes" : "bigyes") : "barn_wave");
	g_keeperBackAt = get_gametime() + 2.5;
	if (g_keeperKind == 2)
		client_print_color(id, print_team_default, "^4G-Man:^1 %s", GMAN_LINES[random(sizeof GMAN_LINES)]);
	else
		client_print_color(id, print_team_default, "^4Barney:^1 %s", BARNEY_LINES[random(sizeof BARNEY_LINES)]);
}

/* =========================================================================
 * waves.ini [laststand] (written by the Creature Studio): boss, boss_every, count_scale,
 * hp_scale, type_hp, and "wave N | N-M | N+ = type weight type weight ..." rows
 * ========================================================================= */


stock load_waves()
{
	new path[192];
	get_configsdir(path, charsmax(path));
	format(path, charsmax(path), "%s/gamemodes/waves.ini", path);
	new f = fopen(path, "rt");
	if (!f)
		return;
	new line[512], key[32], val[480];
	new bool:mine = false, bool:found = false;
	while (fgets(f, line, charsmax(line)))
	{
		/* comments start with ; # or // */
		new c = contain(line, ";");
		if (c >= 0) line[c] = EOS;
		c = contain(line, "#");
		if (c >= 0) line[c] = EOS;
		c = contain(line, "//");
		if (c >= 0) line[c] = EOS;
		trim(line);
		if (!line[0])
			continue;
		if (line[0] == '[')
		{
			new sec[32];
			copyc(sec, charsmax(sec), line[1], ']');
			trim(sec);
			mine = bool:equali(sec, "laststand");
			if (mine)
				found = true;
			continue;
		}
		if (!mine)
			continue;
		strtok2(line, key, charsmax(key), val, charsmax(val), '=', TRIM_FULL);
		strtolower(key);
		if (equal(key, "boss"))
			copy(g_wBoss, charsmax(g_wBoss), val);
		else if (equal(key, "boss_every"))
			g_wBossEvery = clamp(str_to_num(val), 0, 1000);
		else if (equal(key, "count_scale"))
			g_wCount = floatclamp(str_to_float(val), 0.1, 10.0);
		else if (equal(key, "hp_scale"))
			g_wHp = floatclamp(str_to_float(val), 0.1, 10.0);
		else if (equal(key, "type_hp"))
			g_wTypeHp = str_to_num(val) != 0;
		else if (equal(key, "wave", 4) && g_nrows < MAX_ROWS)
		{
			new r = g_nrows;
			new range[16];
			copy(range, charsmax(range), key[4]);
			trim(range);
			new dash = contain(range, "-");
			new plus = contain(range, "+");
			if (plus >= 0)
			{
				range[plus] = EOS;
				g_wRowFrom[r] = str_to_num(range);
				g_wRowTo[r] = 0;
			}
			else if (dash >= 0)
			{
				g_wRowTo[r] = str_to_num(range[dash + 1]);
				range[dash] = EOS;
				g_wRowFrom[r] = str_to_num(range);
			}
			else
			{
				g_wRowFrom[r] = str_to_num(range);
				g_wRowTo[r] = g_wRowFrom[r];
			}
			if (g_wRowFrom[r] < 1)
				continue;
			g_wMixN[r] = 0;
			new rest[480], t[16], w[12];
			copy(rest, charsmax(rest), val);
			while (rest[0] && g_wMixN[r] < MAX_MIX)
			{
				strtok2(rest, t, charsmax(t), rest, charsmax(rest), ' ', TRIM_FULL);
				strtok2(rest, w, charsmax(w), rest, charsmax(rest), ' ', TRIM_FULL);
				if (!t[0] || !w[0])
					break;
				strtolower(t);
				new m = g_wMixN[r]++;
				copy(g_wMixType[r][m], charsmax(g_wMixType[][]), t);
				g_wMixW[r][m] = clamp(str_to_num(w), 0, 1000);
			}
			if (g_wMixN[r])
				g_nrows++;
		}
	}
	fclose(f);
	if (found)
		log_amx("Last Stand: waves from waves.ini (%d rows, boss %s every %d).", g_nrows, g_wBoss, g_wBossEvery);
}

/* the types waves 1..lst_waves use (every wave when endless), from waves.ini or the built-in mix */
stock precache_types(list[], len)
{
	list[0] = EOS;
	new p = get_cvar_pointer("lst_waves");
	new n = p ? get_pcvar_num(p) : 10;
	if (n <= 0)
		n = 1000;
	if (g_nrows)
	{
		for (new r = 0; r < g_nrows; r++)
			if (g_wRowFrom[r] <= n)
				for (new m = 0; m < g_wMixN[r]; m++)
					if (g_wMixW[r][m] > 0)
						add_word(list, len, g_wMixType[r][m]);
	}
	else
	{
		add_word(list, len, "zombie");
		if (n >= 2) add_word(list, len, "runner");
		if (n >= 3) add_word(list, len, "brute");
		if (n >= 4) add_word(list, len, "spitter");
	}
	if (g_wBoss[0] && g_wBossEvery > 0 && g_wBossEvery <= n)
		add_word(list, len, g_wBoss);
	if (!list[0])
		copy(list, len, "zombie");
}

stock add_word(list[], len, const word[])
{
	if (!word[0])
		return;
	new padded[280], w[20];
	formatex(padded, charsmax(padded), " %s ", list);
	formatex(w, charsmax(w), " %s ", word);
	if (containi(padded, w) >= 0)
		return;
	add(list, len, " ");
	add(list, len, word);
}

/* =========================================================================
 * Bits
 * ========================================================================= */

/* the floor under a point; a point sitting on (or just in) the floor finds that floor, and
   nothing sensible under it keeps the point's own height (never drops something under the map) */
stock Float:floor_under(const Float:p[3], ignore = 0)
{
	new Float:a[3], Float:b[3], Float:hit[3], Float:frac;
	a = p;
	a[2] += 32.0;
	b = p;
	b[2] -= 512.0;
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, ignore, 0);
	get_tr2(0, TR_flFraction, frac);
	if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid) || frac >= 1.0)
		return p[2] - 1.0;
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
