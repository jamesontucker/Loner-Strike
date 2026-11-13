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
new const CREATURE_PRECACHE[] = "zombie runner brute spitter boss";

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
	g_cWaitHuman = register_cvar("lst_wait_human", "1");
	g_cOverrun = register_cvar("lst_overrun", "10");
	g_cLastChance = register_cvar("lst_last_chance", "30");
	g_cLives = register_cvar("lst_lives", "5");
	g_cBaseRadius = register_cvar("lst_base_radius", "450");
	g_cBuyAnywhere = register_cvar("lst_buy_anywhere", "0");
	g_cShopScale = register_cvar("lst_shop_scale", "1.0");
	register_clcmd("say /shop", "cmd_shop");
	register_clcmd("say_team /shop", "cmd_shop");
	register_clcmd("lst_shop", "cmd_shop");
	register_forward(FM_PlayerPreThink, "fm_prethink");

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
stock bool:in_base(e)
{
	new Float:o[3];
	pev(e, pev_origin, o);
	new b = ls_comp_by_name("lst_base");
	if (b)
	{
		if (1 <= e <= MaxClients)
			return ls_comp_inside(b, e);
		new Float:bo[3], sz[32];
		pev(b, pev_origin, bo);
		if (ls_comp_get(b, "size", sz, charsmax(sz)))
		{
			new a[3][12];
			parse(sz, a[0], 11, a[1], 11, a[2], 11);
			return floatabs(o[0] - bo[0]) <= str_to_float(a[0]) * 0.5 && floatabs(o[1] - bo[1]) <= str_to_float(a[1]) * 0.5
				&& o[2] > bo[2] - 64.0 && o[2] < bo[2] + str_to_float(a[2]) + 64.0;
		}
		new Float:r = ls_comp_getf(b, "radius", 300.0);
		return floatsqroot((o[0] - bo[0]) * (o[0] - bo[0]) + (o[1] - bo[1]) * (o[1] - bo[1])) <= r && floatabs(o[2] - bo[2]) < 160.0;
	}
	new Float:r = get_pcvar_float(g_cBaseRadius);
	return floatsqroot((o[0] - g_base[0]) * (o[0] - g_base[0]) + (o[1] - g_base[1]) * (o[1] - g_base[1])) <= r && floatabs(o[2] - g_base[2]) < 200.0;
}

/* sets why when the stand is lost this second */
stock lose_checks(why[], len)
{
	new Float:now = get_gametime();
	/* overrun: enemies in the base and no defender alive in it, for lst_overrun seconds */
	new limit = floatround(get_pcvar_float(g_cOverrun) * time_scale());
	if (get_pcvar_num(g_cOverrun) > 0)
	{
		new enemies = 0, inside = 0;
		if (g_creatures)
		{
			new e = -1;
			while ((e = find_ent_by_class(e, "ls_creature")) > 0 && !enemies)
				if (pev(e, pev_deadflag) == DEAD_NO && in_base(e))
					enemies++;
		}
		else
			for (new p = 1; p <= MaxClients && !enemies; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T && in_base(p))
					enemies++;
		for (new p = 1; p <= MaxClients && !inside; p++)
			if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_CT && in_base(p))
				inside++;
		if (enemies && !inside)
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
	g_bossDone = (n % 5) != 0;
	g_spawned = 0;
	g_failSpawns = 0;
	set_cvar_num("mp_buy_anywhere", 0);
	ls_comp_bots_hint("goal_clear ct use");

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
	if (g_phase == PH_WAVE)
	{
		attack_barricades();
		shift_defenders();
	}
	hud();
	beacons();
	bots_shop_tick();
	if (g_phase != PH_WAVE || (floatround(now) % 3) == 0)
		shop_hint();
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
	/* the two nearest bots go to help */
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
		formatex(line, charsmax(line), "goal %d go_hold %.0f %.0f %.0f %d 60 %d 40", near[i], g_downOrg[victim][0], g_downOrg[victim][1], g_downOrg[victim][2], victim, get_pcvar_num(g_cRevive));
		ls_comp_bots_hint(line);
	}
	return HAM_IGNORED;
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

new const SHOP_NAME[][] = { "Barricade repair kit", "Armour refill", "Grenade pack", "Ammo refill", "Medkit" };
new const SHOP_PRICE[] = { 300, 200, 300, 150, 250 };

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

stock shop_price(i)
{
	return floatround(float(SHOP_PRICE[i]) * (1.0 + 0.1 * float(max(g_wave - 1, 0))) * get_pcvar_float(g_cShopScale));
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
	new m = menu_create("\yArmory", "menu_shop");
	new text[64], info[4];
	for (new i = 0; i < sizeof SHOP_NAME; i++)
	{
		formatex(text, charsmax(text), "%s \R\y$%d", SHOP_NAME[i], shop_price(i));
		num_to_str(i, info, charsmax(info));
		menu_additem(m, text, info);
	}
	menu_display(id, m);
}

public menu_shop(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[4], name[64], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	menu_destroy(menu);
	if (is_user_alive(id) && at_shop(id))
		shop_buy(id, str_to_num(info), true);
	return PLUGIN_HANDLED;
}

stock bool:shop_buy(id, i, bool:tell)
{
	new price = shop_price(i), money = cs_get_user_money(id);
	if (money < price)
	{
		if (tell)
			client_print(id, print_center, "Not enough money ($%d)", price);
		return false;
	}
	switch (i)
	{
		case 0:
		{
			/* the most damaged barricade, fully */
			new best = 0, worst = 101;
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
			if (!best || worst >= 100)
			{
				if (tell)
					client_print(id, print_center, "The barricades are fine");
				return false;
			}
			ls_barricade_repair(best, 0);
		}
		case 1: cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM);
		case 2:
		{
			ls_give_item(id, "he");
			ls_give_item(id, "flash");
			ls_give_item(id, "smoke");
			cs_set_user_bpammo(id, CSW_FLASHBANG, 2);
		}
		case 3: ls_give_item(id, "ammo");
		case 4: set_user_health(id, max(get_user_health(id), 100));
	}
	cs_set_user_money(id, money - price);
	if (tell)
	{
		client_print(id, print_center, "%s bought", SHOP_NAME[i]);
		keeper_thanks(id);
	}
	return true;
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

stock shop_hint()
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && !is_user_bot(p) && at_shop(p))
			client_print(p, print_center, "Armory: press USE (or say /shop)");
}

/* =========================================================================
 * The shopkeeper: Barney (or the G-Man) at the armory, from Half-Life's own models
 * ========================================================================= */

new g_cKeeper;
new g_keeperModel[64];
new g_keeper;
new g_keeperKind;	/* 0 none, 1 barney, 2 gman, 3 a CS model standing in */
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
	new Float:o[3];
	o = g_shop;
	o[2] = floor_under(g_shop) + 1.0;
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
