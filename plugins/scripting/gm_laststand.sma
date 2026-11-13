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

/* The built-in rounds (when waves.ini has no [laststand]): an endless ramp in the style of
   zombie-survival games (health grows a little each round through round 9, then 10 % a round;
   more creatures each round with a cap on how many are alive at once; walkers early, runners
   later), roles in the style of wave shooters (fodder, flankers, ranged, tanks, a heavy, bosses),
   a themed special round every 6, a boss every 5 from round 10. Each type unlocks at a round and
   falls back to an older type on a server that doesn't have it yet. docs/notes/W1.md has the
   round table. */
enum { RL_FODDER = 0, RL_FLANK, RL_RANGED, RL_TANK, RL_COUNT };
new const ROSTER[][] = { "zombie", "babycrab", "runner", "spitter", "brute", "vortigaunt", "assassin", "soldier", "controller", "stukabat" };
new const ROSTER_ROLE[] = { RL_FODDER, RL_FODDER, RL_FLANK, RL_RANGED, RL_TANK, RL_RANGED, RL_FLANK, RL_TANK, RL_RANGED, RL_FLANK };
new const ROSTER_FROM[] = { 1, 2, 3, 4, 5, 7, 8, 9, 12, 14 };
new const ROSTER_HP[] = { 100, 30, 60, 120, 450, 150, 90, 300, 200, 60 };
new const ROSTER_PAY[] = { 75, 50, 90, 100, 175, 120, 130, 175, 150, 100 };
new const ROSTER_FALLBACK[][] = { "", "headcrab", "", "", "", "spitter", "runner", "brute", "spitter", "runner" };
/* specials, the heavy and the bosses: name, health, pay, fallback */
new const EXTRA[][] = { "hound", "snark", "babygarg", "boss", "gonarch" };
new const EXTRA_HP[] = { 80, 20, 1500, 3000, 3500 };
new const EXTRA_PAY[] = { 90, 40, 400, 1000, 1000 };
new const EXTRA_FALLBACK[][] = { "runner", "headcrab", "boss", "", "boss" };
enum { EX_HOUND = 0, EX_SNARK, EX_HEAVY, EX_GARG, EX_GONARCH };
#define HEAVY_FROM 11

new g_cWaves, g_cDiff, g_cPrepare, g_cBreak, g_cRevive, g_cDrops, g_cCreatures, g_cDefenders, g_cWaveMoney, g_cWaitHuman, g_cOverrun, g_cLastChance, g_cLives, g_cBaseRadius, g_cBuyAnywhere, g_cShopScale;

new Float:g_gunNext[33];
new g_botGunsBought;
new g_botRifles;
new bool:g_lostLast;
const PRIMARY_BITS = (1<<CSW_SCOUT)|(1<<CSW_XM1014)|(1<<CSW_MAC10)|(1<<CSW_AUG)|(1<<CSW_UMP45)|(1<<CSW_SG550)|(1<<CSW_GALIL)|(1<<CSW_FAMAS)|(1<<CSW_AWP)|(1<<CSW_MP5NAVY)|(1<<CSW_M249)|(1<<CSW_M3)|(1<<CSW_M4A1)|(1<<CSW_TMP)|(1<<CSW_G3SG1)|(1<<CSW_SG552)|(1<<CSW_AK47)|(1<<CSW_P90);
new g_phase;
new g_wave;
new Float:g_phaseEnd;
new bool:g_creatures;
new bool:g_hasCreatureDll;
new g_failSpawns;
new Float:g_base[3];
new g_gatePct[2048];
new Float:g_gateUpAt[2048];
new g_rightShown[33][480];
new Float:g_rightAt[33];
new g_planned;
new g_spawned;
enum { PU_RESUPPLY = 0, PU_DOUBLE, PU_ONEHIT, PU_AIRSTRIKE, PU_COUNT };
new const PU_NAME[PU_COUNT][] = { "Full resupply", "Double pay", "One-hit kills", "Airstrike" };
new const PU_MODEL[PU_COUNT][] = { "models/w_9mmarclip.mdl", "models/w_kevlar.mdl", "models/w_backpack.mdl", "models/w_medkit.mdl" };
new const PU_RGB[PU_COUNT][3] = { {80, 160, 255}, {255, 210, 40}, {255, 60, 60}, {255, 140, 0} };
new g_puEnt[8], g_puKind[8], Float:g_puUntil[8];
new g_puThisWave;
new bool:g_bossDone;
new bool:g_traderWarned;
new bool:g_headLast[2048];
new Float:g_doubleUntil, Float:g_instaUntil;
new g_special = -1;
new g_heavyLeft;
new g_cAliveMax;
#define ROUND_SOUND "buttons/bell1.wav"
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
#define MAX_HOLD 48
new Float:g_hold[MAX_HOLD][3];
new g_holdGate[MAX_HOLD];             /* which gate (index into g_gates), -1 none */
new bool:g_holdHigh[MAX_HOLD];
new g_nhold;
new g_gates[16];
new g_ngates;
new Float:g_gateC[16][3];
new Float:g_groundZ;
new bool:g_holdBuilt;
new Float:g_stepLike[33];
new g_prefGate[33];
new g_holdPick[33] = { -1, ... };
new Float:g_holdScore[33];
new Float:g_holdUntil[33];
new Float:g_reweighAt;
new g_stepSecs, g_floorSecs;
new bool:g_preHeld;
new g_role[33];
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

	g_cWaves = register_cvar("lst_waves", "0");
	g_cDiff = register_cvar("lst_difficulty", "1");
	g_cPrepare = register_cvar("lst_prepare", "45");
	g_cBreak = register_cvar("lst_break", "20");
	g_cRevive = register_cvar("lst_revive", "30");
	g_cDrops = register_cvar("lst_drops", "2");
	g_cDefenders = register_cvar("lst_defenders", "4");
	g_cWaveMoney = register_cvar("lst_wave_money", "600");
	g_cAliveMax = register_cvar("lst_alive_max", "0");
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
	register_srvcmd("lst_gates", "cmd_gates");
	register_srvcmd("lst_holds", "cmd_holds");
	register_srvcmd("lst_perf", "cmd_perf");
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
	{
		RegisterHam(Ham_Killed, "ls_creature", "ham_creature_killed", 1);
		RegisterHam(Ham_TakeDamage, "ls_creature", "ham_creature_hurt");
		RegisterHam(Ham_TakeDamage, "ls_creature", "ham_creature_hurt_post", 1);
		RegisterHam(Ham_TraceAttack, "ls_creature", "ham_creature_trace");
	}
	set_task(0.2, "tick_powerups", TASK_REV + 400, _, _, "b");

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

/* server: entities, creatures alive, defenders up (for big-team tests) */
public cmd_perf()
{
	new ct = 0, up = 0, cr = 0, e = -1;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && cs_get_user_team(p) == CS_TEAM_CT)
		{
			ct++;
			if (is_user_alive(p))
				up++;
		}
	while ((e = find_ent_by_class(e, "ls_creature")))
		if (pev(e, pev_deadflag) == DEAD_NO)
			cr++;
	log_amx("Last Stand perf: %d entities of %d, %d creatures alive (cap %d), %d of %d defenders up, phase %d wave %d", engfunc(EngFunc_NumberOfEntities), global_get(glb_maxEntities), cr, alive_cap(), up, ct, g_phase, g_wave);
	return PLUGIN_HANDLED;
}

/* server: where each holder was sent and where it is (for tests) */
public cmd_holds()
{
	new Float:o[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		pev(p, pev_origin, o);
		new k = g_holdPick[p];
		if (k >= 0)
			server_print("bot %d role %d at %.0f %.0f %.0f -> spot %d (%s) %.0f %.0f %.0f, %.0f away, repairing %d", p, g_role[p], o[0], o[1], o[2], k, g_holdHigh[k] ? "raised" : "floor", g_hold[k][0], g_hold[k][1], g_hold[k][2], get_distance_f(o, g_hold[k]), g_repairing[p]);
		else
			server_print("bot %d role %d at %.0f %.0f %.0f, no hold", p, g_role[p], o[0], o[1], o[2]);
	}
	return PLUGIN_HANDLED;
}

/* server: the gates list as players see it (for tests) */
public cmd_gates()
{
	new t[400], a[8];
	read_argv(1, a, charsmax(a));
	gates_text(t, charsmax(t), equal(a, "all") != 0);
	replace_all(t, charsmax(t), "^n", " | ");
	log_amx("Last Stand gates:%s", t);
	return PLUGIN_HANDLED;
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
	/* a new stand after a loss starts from the start money, not with the last stand's savings */
	if (g_lostLast)
	{
		g_lostLast = false;
		new start = get_cvar_num("mp_startmoney");
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && cs_get_user_team(p) == CS_TEAM_CT)
				cs_set_user_money(p, start);
	}
	g_botGunsBought = 0;
	g_botRifles = 0;
	clear_powerups();
	/* repairs are paid for (points per board), not charged */
	for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
		ls_comp_set(b, "repair_cost", "0");
	arrayset(g_gatePct, 0, sizeof g_gatePct);
	ls_comp_bots_hint("set laststand");
	if (g_creatures)
		ls_comp_bots_hint("enemyclass ls_creature 1");
	clear_defenders();
	for (new p = 1; p <= MaxClients; p++)
		g_role[p] = 0;
	g_holdBuilt = false;
	g_phase = PH_PREPARE;
	g_overrun = 0;
	g_lastChanceEnd = 0.0;
	g_lives = get_pcvar_num(g_cLives);
	g_phaseEnd = get_gametime() + get_pcvar_float(g_cPrepare);
	set_cvar_num("mp_buy_anywhere", get_pcvar_num(g_cBuyAnywhere) ? 1 : 0);
	find_shop();
	keeper_spawn();
	round_clock(floatround(get_pcvar_float(g_cPrepare)));
	toast(0, 5.0, false, "Last Stand: hold the base! First wave in %d seconds", get_pcvar_num(g_cPrepare));
	bots_to_shop(get_pcvar_num(g_cPrepare));
	if (g_record > 0)
		ls_chat(0, "Best on %s so far: wave %d.", g_mapname, g_record);
	ls_chat(0, "Buy at the base, or press USE at the armory (say /shop) for repair kits, armor, grenades and ammo.");
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

/* Defenders. Most bots (about 85 %) hold somewhere during a wave; the rest play on their own.
   Where a holder goes is weighed, not scripted: each gate's threat (creatures near it, its
   damage, broken or not) against two kinds of spot - up on the firing step / any raised floor
   behind the gate (strongly preferred: they shoot down over the wall) or on the floor just behind
   it (worth more once the gate is broken). Every bot has its own liking for height and a favourite
   gate, and picks among a gate's spots, so no two stands look the same. They still come down to
   repair (a repair goal outranks holding), take supplies, revive, or chase a creature that got
   inside (holders are let go while the base has one), then are sent back up.
   Raised spots: the map's ls_marker kind=lst_hold if it has any, else found by tracing behind
   each gate for floor 64+ units above the gate's own with room to stand. */

stock bool:in_base_pt(const Float:o[3], Float:margin = 0.0)
{
	new b = ls_comp_by_name("lst_base");
	if (b)
	{
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

stock add_hold(const Float:p[3], gate, bool:high)
{
	if (g_nhold >= MAX_HOLD)
		return;
	for (new i = 0; i < g_nhold; i++)
		if (get_distance_f(g_hold[i], p) < 56.0)
			return;
	g_hold[g_nhold] = p;
	g_holdGate[g_nhold] = gate;
	g_holdHigh[g_nhold] = high;
	g_nhold++;
}

/* room for a standing player at p (feet on the floor): CLIP brushes count, so wall tops don't */
stock bool:room_to_stand(const Float:feet[3])
{
	new Float:o[3];
	o = feet;
	o[2] += 37.0;
	engfunc(EngFunc_TraceHull, o, o, IGNORE_MONSTERS, HULL_HUMAN, 0, 0);
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid);
}

stock build_holds()
{
	g_nhold = 0;
	g_ngates = 0;
	g_groundZ = floor_under(g_base);
	for (new b = ls_comp_find("ls_barricade"); b && g_ngates < sizeof g_gates; b = ls_comp_find("ls_barricade", b))
	{
		g_gates[g_ngates] = b;
		gate_centre(b, g_gateC[g_ngates]);
		g_ngates++;
	}
	/* the map's own hold spots */
	new marked = 0;
	for (new m = ls_marker_find("lst_hold"); m; m = ls_marker_find("lst_hold", m))
	{
		new Float:o[3];
		pev(m, pev_origin, o);
		o[2] = floor_under(o);
		add_hold(o, nearest_gate(o), o[2] >= g_groundZ + 48.0);
		marked++;
	}
	for (new g = 0; g < g_ngates; g++)
	{
		new Float:c[3];
		c = g_gateC[g];
		new Float:gz = floor_under(c);
		new Float:dx = g_base[0] - c[0], Float:dy = g_base[1] - c[1];
		new Float:l = floatmax(floatsqroot(dx * dx + dy * dy), 1.0);
		dx /= l;
		dy /= l;
		/* the floor just behind the gate */
		new Float:fl[3];
		fl[0] = c[0] + dx * 160.0;
		fl[1] = c[1] + dy * 160.0;
		fl[2] = gz + 1.0;
		fl[2] = floor_under(fl);
		if (room_to_stand(fl))
			add_hold(fl, g, false);
		if (marked)
			continue;
		/* raised floor behind it: a grid inward and along the wall */
		new n = 0;
		for (new t = 40; t <= 240 && n < 8; t += 40)
			for (new s = -320; s <= 320 && n < 8; s += 64)
			{
				new Float:p[3];
				p[0] = c[0] + dx * float(t) - dy * float(s);
				p[1] = c[1] + dy * float(t) + dx * float(s);
				p[2] = gz + 240.0;
				new Float:fz = floor_under(p);
				if (fz < gz + 64.0 || fz > gz + 220.0)
					continue;
				p[2] = fz + 1.0;
				if (!in_base_pt(p) || !room_to_stand(p))
					continue;
				add_hold(p, g, true);
				n++;
			}
	}
	g_holdBuilt = true;
	new high = 0;
	for (new i = 0; i < g_nhold; i++)
		if (g_holdHigh[i])
			high++;
	log_amx("Last Stand: %d hold spots (%d raised%s) at %d gates", g_nhold, high, marked ? ", from the map" : "", g_ngates);
}

stock nearest_gate(const Float:o[3])
{
	new best = -1, Float:bd = 99999.0;
	for (new g = 0; g < g_ngates; g++)
	{
		new Float:d = get_distance_f(o, g_gateC[g]);
		if (d < bd)
		{
			bd = d;
			best = g;
		}
	}
	return best;
}

/* how badly a gate needs people: creatures near it, its damage, broken */
stock Float:gate_threat(g)
{
	new b = g_gates[g];
	new Float:t = 0.0, Float:o[3];
	new e = -1;
	while ((e = find_ent_by_class(e, "ls_creature")))
	{
		if (pev(e, pev_deadflag) != DEAD_NO)
			continue;
		pev(e, pev_origin, o);
		new Float:d = get_distance_f(o, g_gateC[g]);
		if (d < 1200.0)
			t += d < 500.0 ? 1.0 : 0.5;
	}
	new mx = floatround(ls_comp_getf(b, "hp", 500.0));
	new hp = ls_barricade_hp(b);
	t += 2.0 * (1.0 - float(clamp(hp, 0, mx)) / float(max(mx, 1)));
	if (hp <= 0)
		t += 2.0;
	return t;
}

stock free_bots()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (g_role[p] == 2 && is_user_connected(p))
			n++;
	return n;
}

stock bots_ct()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && is_user_bot(p) && cs_get_user_team(p) == CS_TEAM_CT)
			n++;
	return n;
}

stock creatures_inside()
{
	new n = 0, e = -1, Float:o[3];
	while ((e = find_ent_by_class(e, "ls_creature")))
	{
		if (pev(e, pev_deadflag) != DEAD_NO)
			continue;
		pev(e, pev_origin, o);
		if (in_base_pt(o))
			n++;
	}
	return n;
}

stock assign_defenders()
{
	if (!g_holdBuilt)
		build_holds();
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		if (!g_role[p])
		{
			/* about 1 in 7 plays free (at least one when there are 4 or more) */
			g_role[p] = random_num(1, 100) <= 85 ? 1 : 2;
			if (g_role[p] == 2 && free_bots() >= max(1, bots_ct() / 7))
				g_role[p] = 1;
			g_stepLike[p] = random_float(0.75, 1.35);
			g_prefGate[p] = g_ngates ? random(g_ngates) : -1;
		}
	}
	g_reweighAt = 0.0;
	weigh_defenders(true);
}

/* each holder's best spot now; a move only for a clearly better one (or its time is up) */
stock weigh_defenders(bool:force = false)
{
	new Float:now = get_gametime();
	if (!force && now < g_reweighAt)
		return;
	g_reweighAt = now + 3.0;
	if (!g_nhold)
		return;
	new Float:threat[16];
	for (new g = 0; g < g_ngates; g++)
		threat[g] = gate_threat(g);
	new inside = creatures_inside();
	new taken[MAX_HOLD];
	for (new p = 1; p <= MaxClients; p++)
		if (g_holdPick[p] >= 0 && g_holdPick[p] < MAX_HOLD)
			taken[g_holdPick[p]]++;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (g_role[p] != 1 || !is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		/* repairing or reviving: those goals outrank holding; it's weighed again after */
		if (g_repairing[p])
			continue;
		/* a creature inside: about half the holders let go and hunt it, the rest stay up */
		if (inside > 0 && (g_stepLike[p] < 1.05 || inside >= 3))
		{
			if (g_holdPick[p] >= 0)
			{
				new line[40];
				formatex(line, charsmax(line), "goal_clear %d capture", p);
				ls_comp_bots_hint(line);
				formatex(line, charsmax(line), "goal_clear %d defend", p);
				ls_comp_bots_hint(line);
				taken[g_holdPick[p]]--;
				g_holdPick[p] = -1;
			}
			continue;
		}
		new best = -1, Float:bs = -99.0, Float:o[3], Float:curSc = -99.0;
		pev(p, pev_origin, o);
		for (new i = 0; i < g_nhold; i++)
		{
			new g = g_holdGate[i];
			new Float:th = g >= 0 ? threat[g] : 0.5;
			new bool:broken = g >= 0 && ls_barricade_hp(g_gates[g]) <= 0;
			new Float:sc;
			if (g_holdHigh[i])
				sc = (6.0 + 4.0 * th) * g_stepLike[p];
			else
				sc = 1.5 + th * (broken ? 2.5 : 0.8);
			if (g == g_prefGate[p])
				sc *= 1.25;
			/* spread out: a spot someone else holds counts less */
			sc /= 1.0 + 0.8 * float(taken[i] - (g_holdPick[p] == i ? 1 : 0));
			/* the way there counts (no running across the yard for a slightly better spot) */
			sc -= get_distance_f(o, g_hold[i]) / 300.0;
			sc += random_float(0.0, 1.0);
			if (i == g_holdPick[p])
				curSc = sc;
			if (sc > bs)
			{
				bs = sc;
				best = i;
			}
		}
		if (best < 0)
			continue;
		new cur = g_holdPick[p];
		if (cur == best && now < g_holdUntil[p] - 2.0)
			continue;
		/* staying put unless clearly better (or its time there is up) */
		if (cur >= 0 && now < g_holdUntil[p] && (bs < curSc + 4.0 || bs < curSc * 1.5))
			continue;
		if (cur >= 0)
			taken[cur]--;
		taken[best]++;
		g_holdPick[p] = best;
		g_holdScore[p] = bs;
		new secs = random_num(25, 55);
		g_holdUntil[p] = now + float(secs);
		new line[112];
		/* up on the step: a tight hold (the brain's "capture" station keeps it within the radius,
		   moving between gaps, rather than wandering 400 units off); on the floor: a looser one */
		formatex(line, charsmax(line), "goal %d %s %.0f %.0f %.0f 0 40 %d %d", p, g_holdHigh[best] ? "capture" : "defend", g_hold[best][0], g_hold[best][1], g_hold[best][2] + 1.0, secs + 5, g_holdHigh[best] ? 110 : 160);
		ls_comp_bots_hint(line);
	}
}

/* step vs floor time (for tuning) */
stock count_height()
{
	if (!g_holdBuilt)
		return;
	new Float:o[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		pev(p, pev_origin, o);
		if (o[2] - 36.0 >= g_groundZ + 64.0)
			g_stepSecs++;
		else
			g_floorSecs++;
	}
}

stock clear_defenders()
{
	ls_comp_bots_hint("goal_clear ct defend");
	ls_comp_bots_hint("goal_clear ct capture");
	for (new p = 1; p <= MaxClients; p++)
		g_holdPick[p] = -1;
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
			toast(0, 1.1, true, "Base being overrun! %d", limit - g_overrun);
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
			toast(0, 3.0, false, "Every player is down! The bots have %d seconds to hold or revive you", chance);
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

/* the team-size factors: counted up to 16 players (more than that: a little more health each,
   not more creatures) */
stock Float:team_count_factor()
{
	return 0.5 + 0.2 * float(min(defenders(), 16));
}

stock Float:team_hp_factor()
{
	new d = defenders();
	return (0.8 + 0.1 * float(min(d, 16))) * (1.0 + 0.02 * float(max(d - 16, 0)));
}

/* creature health: +4 % a round through round 9, then x1.1 a round */
stock Float:round_hp(n)
{
	if (n <= 9)
		return 1.0 + 0.04 * float(n - 1);
	return 1.32 * floatpower(1.1, float(n - 9));
}

stock Float:hp_mult()
{
	return round_hp(g_wave) * team_hp_factor() * diff_mult(true);
}

/* share of walking fodder that runs instead: none in rounds 1-2, +10 % a round, 95 % from 13 */
stock Float:fast_share(n)
{
	if (n < 3)
		return 0.0;
	return floatmin(0.1 * float(n - 2), n >= 13 ? 0.95 : 0.9);
}

/* how many creatures may be alive at once (the rest come in as they die): lst_alive_max, or
   by the team's size (16 + 2 a player, 20-48) */
stock alive_cap()
{
	new c = get_pcvar_num(g_cAliveMax);
	if (c > 0)
		return c;
	return clamp(16 + 2 * defenders(), 20, 48);
}

stock type_hp(const type[])
{
	for (new i = 0; i < sizeof ROSTER; i++)
		if (equal(type, ROSTER[i]))
			return ROSTER_HP[i];
	for (new i = 0; i < sizeof EXTRA; i++)
		if (equal(type, EXTRA[i]))
			return EXTRA_HP[i];
	return 0;
}

stock type_pay(const type[])
{
	for (new i = 0; i < sizeof ROSTER; i++)
		if (equal(type, ROSTER[i]))
			return ROSTER_PAY[i];
	for (new i = 0; i < sizeof EXTRA; i++)
		if (equal(type, EXTRA[i]))
			return EXTRA_PAY[i];
	return 50;
}

stock type_fallback(const type[], out[], len)
{
	out[0] = EOS;
	for (new i = 0; i < sizeof ROSTER; i++)
		if (equal(type, ROSTER[i]))
			copy(out, len, ROSTER_FALLBACK[i]);
	for (new i = 0; i < sizeof EXTRA; i++)
		if (equal(type, EXTRA[i]))
			copy(out, len, EXTRA_FALLBACK[i]);
}

stock start_wave(n)
{
	g_wave = n;
	g_phase = PH_WAVE;
	g_overrun = 0;
	g_lastChanceEnd = 0.0;
	g_special = -1;
	g_heavyLeft = 0;
	g_puThisWave = 0;
	if (g_nrows)
		g_bossDone = g_wBossEvery <= 0 || !g_wBoss[0] || (n % g_wBossEvery) != 0;
	else
	{
		/* a boss every 5 rounds from 10 (garg, then gonarch, in turn); a themed round every 6
		   (a hound pack, then a snark swarm); a heavy or more from round 11 */
		g_bossDone = !(n >= 10 && n % 5 == 0);
		if (!g_bossDone)
			copy(g_wBoss, charsmax(g_wBoss), EXTRA[(n / 5) % 2 == 0 ? EX_GARG : EX_GONARCH]);
		else if (n >= 6 && n % 6 == 0)
			g_special = (n / 6) % 2 == 1 ? EX_HOUND : EX_SNARK;
		if (n >= HEAVY_FROM && g_special < 0)
			g_heavyLeft = 1 + (n - HEAVY_FROM) / 4;
	}
	g_spawned = 0;
	g_failSpawns = 0;
	set_cvar_num("mp_buy_anywhere", 0);
	/* the shop trips end; anyone still waiting for a revive gets their helpers back */
	ls_comp_bots_hint("goal_clear ct use");
	for (new d = 1; d <= MaxClients; d++)
		if (g_down[d])
			send_revive(d);

	/* how many: grows each wave, with the team's size and the difficulty */
	new total = floatround((13.0 + 1.5 * float(n)) * team_count_factor() * diff_mult(false) * g_wCount);
	/* a boss round: fewer grunts with the boss (it's the round's threat); a special round: a
	   fast pack, fewer of them */
	if (!g_bossDone)
		total = floatround(float(total) * 0.6);
	else if (g_special >= 0)
		total = floatround(float(total) * 0.7);
	/* never more alive at once than the cap: the rest come in as they die */
	set_cvar_num("ls_comp_wave_alive_max", alive_cap());
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
	g_stepSecs = 0;
	g_floorSecs = 0;
	g_kills = 0;
	g_waveAt = get_gametime();
	log_amx("Wave %d: %d %s against %d defenders (creature health x%.2f, %d%% of the walkers run, %d alive at most%s%s%s)", n, g_planned, g_creatures ? "creatures" : "zombie bots", defenders(), hp_mult(), floatround(100.0 * fast_share(n)), alive_cap(),
		g_bossDone ? "" : ", boss wave", g_special == EX_HOUND ? ", hound pack" : (g_special == EX_SNARK ? ", snark swarm" : ""), g_heavyLeft ? ", with heavies" : "");
	/* the round's start: a clear cue */
	if (!g_bossDone)
		toast(0, 4.0, true, "Wave %d: a boss is coming!", n);
	else if (g_special == EX_HOUND)
		toast(0, 4.0, true, "Wave %d: a hound pack! Fast and many", n);
	else if (g_special == EX_SNARK)
		toast(0, 4.0, true, "Wave %d: a swarm! Small, fast, everywhere", n);
	else
		toast(0, 3.0, true, "Wave %d", n);
	shake_all();
	client_cmd(0, "spk ^"%s^"", ROUND_SOUND);
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
	/* walking fodder: early rounds walk, later ones run (the zombie's run speed is its own) */
	new speed = 0;
	if (equal(type, "zombie") && random_float(0.0, 1.0) >= fast_share(g_wave))
		speed = 110;
	new cr = spawn_creature(type, o, speed);
	if (!cr)
	{
		/* a type this server doesn't have yet: the one it stands in for */
		new fb[16];
		type_fallback(type, fb, charsmax(fb));
		if (fb[0])
		{
			cr = spawn_creature(fb, o, speed);
			if (!cr)
			{
				new fb2[16];
				type_fallback(fb, fb2, charsmax(fb2));
				if (fb2[0])
					cr = spawn_creature(fb2, o, speed);
			}
		}
	}
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
	if (!g_wTypeHp && type_hp(type))
		base = float(type_hp(type));
	/* the boss and the heavy: the round's growth and difficulty, but only a little of the
	   team-size factor; a special round's pack a little less than usual */
	new Float:mult = (boss || equal(type, EXTRA[EX_HEAVY])) ? round_hp(g_wave) * (0.8 + 0.04 * float(min(defenders(), 16))) * diff_mult(true) : hp_mult();
	if (g_special >= 0)
		mult *= 0.8;
	new Float:hp = floatmax(base * mult * g_wHp, 1.0);
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
	if (g_special >= 0)
	{
		copy(out, len, EXTRA[g_special]);
		return false;
	}
	/* the heavy comes once a quarter of the round is in */
	if (g_heavyLeft > 0 && g_spawned >= g_planned / 4)
	{
		g_heavyLeft--;
		copy(out, len, EXTRA[EX_HEAVY]);
		return false;
	}
	copy(out, len, ROSTER[builtin_type()]);
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
	/* a role by weight (fodder 10, flankers 2 + n/2, ranged 1 + n/4, tanks 1 + n/5, once
	   unlocked), then one of the role's unlocked types */
	new w[RL_COUNT], total = 0;
	for (new r = 0; r < RL_COUNT; r++)
	{
		new any = 0;
		for (new i = 0; i < sizeof ROSTER; i++)
			if (ROSTER_ROLE[i] == r && ROSTER_FROM[i] <= n)
				any = 1;
		if (!any)
			continue;
		switch (r)
		{
			case RL_FODDER: w[r] = 20;
			case RL_FLANK: w[r] = 4 + n;
			case RL_RANGED: w[r] = 2 + n / 2;
			case RL_TANK: w[r] = 2 + (2 * n) / 5;
		}
		total += w[r];
	}
	new role = RL_FODDER, pick = random(max(total, 1));
	for (new r = 0; r < RL_COUNT; r++)
	{
		pick -= w[r];
		if (pick < 0)
		{
			role = r;
			break;
		}
	}
	new cands[sizeof ROSTER], nc = 0;
	for (new i = 0; i < sizeof ROSTER; i++)
		if (ROSTER_ROLE[i] == role && ROSTER_FROM[i] <= n)
			cands[nc++] = i;
	return nc ? cands[random(nc)] : 0;
}

stock spawn_creature(const type[], const Float:o[3], speed = 0)
{
	new ent = create_entity("ls_creature");
	if (!ent)
		return 0;
	DispatchKeyValue(ent, "type", type);
	if (speed > 0)
	{
		new v[8];
		num_to_str(speed, v, charsmax(v));
		DispatchKeyValue(ent, "speed", v);
	}
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
	log_amx("Wave %d held in %d s (%d kills; bots on raised floor %d%% of the time)", g_wave, floatround(get_gametime() - g_waveAt), g_kills, g_stepSecs + g_floorSecs ? 100 * g_stepSecs / (g_stepSecs + g_floorSecs) : 0);
	if (g_wave > g_record)
	{
		g_record = g_wave;
		record_save();
	}
	new waves = get_pcvar_num(g_cWaves);
	if (waves > 0 && g_wave >= waves)
	{
		g_phase = PH_OVER;
		toast(0, 8.0, false, "You held out all %d waves!", waves);
		log_amx("The stand was won: all %d waves held", waves);
		g_lostLast = false;
		server_cmd("endround CT");
		return;
	}
	g_phase = PH_BREAK;
	clear_defenders();
	g_traderWarned = false;
	/* a special round held: a full resupply in the middle of the base */
	if (g_special >= 0)
	{
		new Float:o[3];
		o = g_base;
		o[2] = floor_under(g_base) + 4.0;
		spawn_powerup(PU_RESUPPLY, o);
	}
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
	/* the economy, for tuning: money after the wave's bonus (people and bots apart) */
	new hs = 0, hn = 0, bs = 0, bn = 0, bmin = 99999, bmax = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		new m = cs_get_user_money(p);
		if (is_user_bot(p))
		{
			bs += m;
			bn++;
			bmin = min(bmin, m);
			bmax = max(bmax, m);
		}
		else
		{
			hs += m;
			hn++;
		}
	}
	log_amx("Last Stand money after wave %d: people avg %d (%d), bots avg %d (%d-%d, %d), guns bought by bots so far %d (%d rifles)", g_wave, hn ? hs / hn : 0, hn, bn ? bs / bn : 0, bn ? bmin : 0, bmax, bn, g_botGunsBought, g_botRifles);
	new drops = get_pcvar_num(g_cDrops);
	if (drops > 0 && g_wave % drops == 0)
		supply_drop();
	toast(0, 4.0, false, "Wave %d held! Next wave in %d seconds - restock at the armory", g_wave, brk);
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
	/* the armory closing: a warning 10 s before the next wave */
	if (g_phase == PH_BREAK && !g_traderWarned && g_phaseEnd - now <= 10.0)
	{
		g_traderWarned = true;
		toast(0, 3.0, false, "The armory closes in 10 seconds!");
	}
	/* the last seconds of a break: holders take their places for the wave */
	if ((g_phase == PH_PREPARE || g_phase == PH_BREAK) && g_phaseEnd - now <= 10.0 && !g_preHeld)
	{
		g_preHeld = true;
		assign_defenders();
	}
	if ((g_phase == PH_PREPARE || g_phase == PH_BREAK) && now >= g_phaseEnd)
	{
		g_preHeld = false;
		start_wave(g_wave + 1);
	}

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
		toast(0, 8.0, false, "The base fell on wave %d", g_wave);
		log_amx("The base fell on wave %d (%s) after %d s (bots on raised floor %d%% of the time)", g_wave, why, floatround(get_gametime() - g_waveAt), g_stepSecs + g_floorSecs ? 100 * g_stepSecs / (g_stepSecs + g_floorSecs) : 0);
		g_lostLast = true;
		ls_chat(0, "Best on %s: wave %d.", g_mapname, g_record);
		server_cmd("endround T");
		return;
	}
	bot_repairs();
	if (g_phase == PH_WAVE)
	{
		attack_barricades();
		weigh_defenders();
		count_height();
		if (now >= g_stuckAt)
		{
			g_stuckAt = now + 15.0;
			unstick_creatures();
		}
	}
	if (now >= g_beatAt)
	{
		g_beatAt = now + 120.0;
		log_amx("Last Stand: phase %d, wave %d, %d enemies left, %d of %d defenders standing, bots bought %d guns", g_phase, g_wave, g_phase == PH_WAVE ? enemies_left() : 0, alive, team, g_botGunsBought);
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
	new gates[200];
	gates_text(gates, charsmax(gates));
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue;
		/* in the armory, how to buy, under the wave line (the centre is the game's own
		   centre text and the toasts': nothing of ours sits there for long) */
		if (g_inShop[p])
			ls_hud(p, LS_HUD_TOP, 1.1, "%s^nArmory  -  B: weapons   USE at the counter: supplies", top);
		else
			ls_hud(p, LS_HUD_TOP, 1.1, "%s", top);
		/* the team and the nearest barricade on the right: menus open on the left, the
		   radar is top left and chat bottom left, so nothing here ever sits under them */
		new right[480];
		formatex(right, charsmax(right), "%s%s", team, gates);
		/* sent only when it changes (and now and then, so it never fades) */
		if (!equal(right, g_rightShown[p]) || now - g_rightAt[p] > 8.0)
		{
			copy(g_rightShown[p], charsmax(g_rightShown[]), right);
			g_rightAt[p] = now;
			ls_hud(p, LS_HUD_RIGHT, 10.0, "%s", right);
		}
	}
}

stock hud_wait()
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "Last Stand starts when a player joins the Counter-Terrorists");
}

/* Every gate on the map in one block under the team list: "Gates" then one per line, "West gate
   100%", "East gate 40% !", "South gate broken", "North gate rebuilding 60%"; more than 6: the 3
   weakest and "and N more". Names: the barricade's label or name key, else its targetname made
   readable, else the compass point from the base ("N gate", numbered when two share one) */
stock gate_name(b, out[], len)
{
	if (ls_comp_get(b, "label", out, len) && out[0])
		return;
	/* (a map pack's "Barricade1" is no name) */
	if (ls_comp_get(b, "name", out, len) && out[0] && !(equali(out, "barricade", 9) && (!out[9] || isdigit(out[9]))))
		return;
	new tn[32];
	pev(b, pev_targetname, tn, charsmax(tn));
	if (tn[0])
	{
		replace_all(tn, charsmax(tn), "_", " ");
		replace_all(tn, charsmax(tn), "-", " ");
		trim(tn);
		if ('a' <= tn[0] <= 'z')
			tn[0] -= 32;
		copy(out, len, tn);
		return;
	}
	out[0] = EOS;
}

stock gate_centre(b, Float:c[3])
{
	new Float:mn[3], Float:mx[3];
	pev(b, pev_absmin, mn);
	pev(b, pev_absmax, mx);
	for (new i = 0; i < 3; i++)
		c[i] = (mn[i] + mx[i]) * 0.5;
}

stock gate_compass(b, out[], len)
{
	static const DIRS[][] = { "E", "NE", "N", "NW", "W", "SW", "S", "SE" };
	new Float:c[3];
	gate_centre(b, c);
	new Float:a = floatatan2(c[1] - g_base[1], c[0] - g_base[0], degrees);
	if (a < 0.0)
		a += 360.0;
	copy(out, len, DIRS[floatround(a / 45.0) % 8]);
}

stock gates_text(out[], len, bool:all = false)
{
	new ents[16], names[16][28], pct[16], n = 0;
	for (new b = ls_comp_find("ls_barricade"); b && n < sizeof ents; b = ls_comp_find("ls_barricade", b))
	{
		ents[n] = b;
		gate_name(b, names[n], charsmax(names[]));
		new mx = floatround(ls_comp_getf(b, "hp", 500.0));
		pct[n] = clamp(100 * ls_barricade_hp(b) / max(mx, 1), 0, 100);
		n++;
	}
	out[0] = EOS;
	if (!n)
		return;
	for (new i = 0; i < n; i++)
	{
		if (names[i][0])
			continue;
		new dir[4];
		gate_compass(ents[i], dir, charsmax(dir));
		formatex(names[i], charsmax(names[]), "%s gate", dir);
	}
	for (new i = 0; i < n; i++)
	{
		new k = 1;
		for (new j = i + 1; j < n; j++)
			if (equal(names[i], names[j]))
			{
				k++;
				format(names[j], charsmax(names[]), "%s %d", names[j], k);
			}
	}
	/* rebuilding: health went up in the last 3 s */
	new Float:now = get_gametime();
	for (new i = 0; i < n; i++)
	{
		new e = ents[i];
		if (e >= sizeof g_gatePct)
			continue;
		/* (stored + 1: 0 is "not seen yet") */
		if (g_gatePct[e] && pct[i] + 1 > g_gatePct[e])
			g_gateUpAt[e] = now;
		g_gatePct[e] = pct[i] + 1;
	}
	new order[16], shown = n;
	for (new i = 0; i < n; i++)
		order[i] = i;
	if (n > 6 && !all)
	{
		for (new i = 0; i < n; i++)
			for (new j = i + 1; j < n; j++)
				if (pct[order[j]] < pct[order[i]])
				{
					new t = order[i];
					order[i] = order[j];
					order[j] = t;
				}
		shown = 3;
	}
	copy(out, len, "^n^nGates");
	for (new k = 0; k < shown; k++)
	{
		new i = order[k], e = ents[i], st[24];
		if (pct[i] <= 0)
			copy(st, charsmax(st), "broken");
		else if (e < sizeof g_gatePct && pct[i] < 100 && now - g_gateUpAt[e] < 3.0)
			formatex(st, charsmax(st), "rebuilding %d%%", pct[i]);
		else
			formatex(st, charsmax(st), "%d%%%s", pct[i], pct[i] < 50 ? " !" : "");
		format(out, len, "%s^n%s  %s", out, names[i], st);
	}
	if (shown < n)
		format(out, len, "%s^nand %d more", out, n - shown);
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

public ls_on_barricade_broken(ent, attacker)
{
	new name[28];
	gate_name(ent, name, charsmax(name));
	new what[32] = "?";
	if (attacker > 0 && pev_valid(attacker))
	{
		pev(attacker, pev_classname, what, charsmax(what));
		if (equal(what, "ls_creature"))
			pev(attacker, pev_netname, what, charsmax(what));
	}
	log_amx("Last Stand: %s broken in wave %d (by %s)", name[0] ? name : "a gate", g_wave, what);
}

public ham_player_killed(victim, killer, gib)
{
	g_hePack[victim] = 0;
	if (g_phase == PH_WAVE && !g_creatures && is_user_connected(victim) && cs_get_user_team(victim) == CS_TEAM_T)
		g_kills++;
	if (g_phase != PH_WAVE && g_phase != PH_BREAK && g_phase != PH_PREPARE)
		return HAM_IGNORED;
	if (!is_user_connected(victim) || cs_get_user_team(victim) != CS_TEAM_CT || get_pcvar_num(g_cRevive) <= 0)
		return HAM_IGNORED;
	/* for tuning: what got them and where they stood */
	if (g_phase == PH_WAVE)
	{
		new what[32] = "?", Float:o[3];
		if (killer > 0 && pev_valid(killer))
		{
			pev(killer, pev_classname, what, charsmax(what));
			if (equal(what, "ls_creature"))
				pev(killer, pev_netname, what, charsmax(what));
		}
		pev(victim, pev_origin, o);
		log_amx("Last Stand: defender %d down in wave %d by %s at %.0f %.0f %.0f (%s the base)", victim, g_wave, what, o[0], o[1], o[2], in_base(victim) ? "in" : "outside");
	}
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
	if (!is_user_alive(id))
		return HAM_IGNORED;
	/* a big team on a map with fewer spawn points: nobody stays stuck in someone else */
	if (cs_get_user_team(id) == CS_TEAM_CT)
		unstack_spawn(id);
	if (g_phase == PH_IDLE)
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
	/* points: the type's pay, more for a head or a knife */
	new money = type_pay(type);
	if (ent < sizeof g_headLast && g_headLast[ent])
		money += 50;
	if (get_user_weapon(killer) == CSW_KNIFE)
		money += 50;
	pay(killer, money);
	maybe_powerup(ent);
	if (equal(type, "boss") || equal(type, "gonarch"))
	{
		new name[32];
		get_user_name(killer, name, charsmax(name));
		toast(0, 4.0, false, "%s brought the boss down!", name);
	}
	return HAM_IGNORED;
}

/* ---- points: per hit, per kill, per board repaired; double pay doubles them ---- */
stock pay(id, amount)
{
	if (!(1 <= id <= MaxClients) || !is_user_connected(id) || cs_get_user_team(id) != CS_TEAM_CT)
		return;
	if (get_gametime() < g_doubleUntil)
		amount *= 2;
	cs_set_user_money(id, min(cs_get_user_money(id) + amount, 16000));
}

public ham_creature_hurt(ent, inflictor, attacker, Float:dmg, bits)
{
	if (!(1 <= attacker <= MaxClients) || !is_user_connected(attacker) || cs_get_user_team(attacker) != CS_TEAM_CT)
		return HAM_IGNORED;
	/* one-hit kills: anything but a boss or a heavy goes down to the next hit */
	if (get_gametime() < g_instaUntil)
	{
		new type[16];
		pev(ent, pev_netname, type, charsmax(type));
		if (!equal(type, "boss") && !equal(type, "gonarch") && !equal(type, EXTRA[EX_HEAVY]))
			SetHamParamFloat(4, floatmax(dmg, entity_get_float(ent, EV_FL_health) + 1.0));
	}
	return HAM_IGNORED;
}

public ham_creature_hurt_post(ent, inflictor, attacker, Float:dmg, bits)
{
	if (1 <= attacker <= MaxClients && dmg > 0.0)
		pay(attacker, 10);
	return HAM_IGNORED;
}

/* the last hit's place: a head counts at the kill */
public ham_creature_trace(ent, attacker, Float:dmg, Float:dir[3], tr, bits)
{
	if (ent < sizeof g_headLast)
		g_headLast[ent] = get_tr2(tr, TR_iHitgroup) == HIT_HEAD;
	return HAM_IGNORED;
}

stock unstack_spawn(id)
{
	new Float:o[3], Float:po[3];
	pev(id, pev_origin, o);
	new bool:stacked = false;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == id || !is_user_alive(p))
			continue;
		pev(p, pev_origin, po);
		if (floatabs(o[0] - po[0]) < 33.0 && floatabs(o[1] - po[1]) < 33.0 && floatabs(o[2] - po[2]) < 73.0)
		{
			stacked = true;
			break;
		}
	}
	if (!stacked)
		return;
	/* a ring of spots round where it stands, then round the base */
	for (new k = 0; k < 48; k++)
	{
		new Float:c[3];
		c = (k < 24) ? o : g_base;
		new Float:a = float(k) * 47.0, Float:r = 48.0 + 24.0 * float(k % 8);
		new Float:t[3];
		t[0] = c[0] + r * floatcos(a, degrees);
		t[1] = c[1] + r * floatsin(a, degrees);
		t[2] = c[2] + 8.0;
		engfunc(EngFunc_TraceHull, t, t, DONT_IGNORE_MONSTERS, HULL_HUMAN, id, 0);
		if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
			continue;
		/* (a floor under it, not a drop) */
		if (t[2] - floor_under(t) > 80.0)
			continue;
		engfunc(EngFunc_SetOrigin, id, t);
		return;
	}
}

public ls_on_barricade_repaired(ent, id, hp)
{
	if (g_phase != PH_IDLE && 1 <= id <= MaxClients)
		pay(id, 10);
}

/* ---- power-ups: now and then a killed creature leaves one; whoever on the team touches it
   gets it for everyone ---- */

stock maybe_powerup(ent)
{
	if (g_phase != PH_WAVE || g_puThisWave >= 4 || random_float(0.0, 1.0) >= 0.025)
		return;
	new Float:o[3];
	pev(ent, pev_origin, o);
	o[2] = floor_under(o) + 4.0;
	spawn_powerup(random(PU_COUNT), o);
}

stock spawn_powerup(kind, const Float:o[3])
{
	new slot = -1;
	for (new i = 0; i < sizeof g_puEnt; i++)
		if (!g_puEnt[i] || !pev_valid(g_puEnt[i]))
		{
			slot = i;
			break;
		}
	if (slot < 0)
		return;
	new e = create_entity("info_target");
	if (!e)
		return;
	entity_set_string(e, EV_SZ_classname, "lst_powerup");
	entity_set_model(e, PU_MODEL[kind]);
	entity_set_origin(e, o);
	set_rendering(e, kRenderFxGlowShell, PU_RGB[kind][0], PU_RGB[kind][1], PU_RGB[kind][2], kRenderNormal, 16);
	set_pev(e, pev_movetype, MOVETYPE_NOCLIP);
	set_pev(e, pev_avelocity, Float:{0.0, 90.0, 0.0});
	g_puEnt[slot] = e;
	g_puKind[slot] = kind;
	g_puUntil[slot] = get_gametime() + 25.0;
	g_puThisWave++;
	toast(0, 2.0, false, "%s dropped!", PU_NAME[kind]);
}

public tick_powerups()
{
	new Float:now = get_gametime(), Float:o[3], Float:po[3];
	for (new i = 0; i < sizeof g_puEnt; i++)
	{
		new e = g_puEnt[i];
		if (!e)
			continue;
		if (!pev_valid(e) || now >= g_puUntil[i])
		{
			if (pev_valid(e))
				remove_entity(e);
			g_puEnt[i] = 0;
			continue;
		}
		/* blinks its last 5 s */
		if (g_puUntil[i] - now < 5.0)
			set_pev(e, pev_effects, (floatround(now * 4.0) % 2) ? (pev(e, pev_effects) | EF_NODRAW) : (pev(e, pev_effects) & ~EF_NODRAW));
		pev(e, pev_origin, o);
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_CT)
				continue;
			pev(p, pev_origin, po);
			if (get_distance_f(o, po) > 56.0)
				continue;
			take_powerup(g_puKind[i], p);
			remove_entity(e);
			g_puEnt[i] = 0;
			break;
		}
	}
	/* the timed ones running out */
	if (g_doubleUntil > 0.0 && now >= g_doubleUntil)
	{
		g_doubleUntil = 0.0;
		toast(0, 2.0, false, "Double pay is over");
	}
	if (g_instaUntil > 0.0 && now >= g_instaUntil)
	{
		g_instaUntil = 0.0;
		toast(0, 2.0, false, "One-hit kills are over");
	}
}

stock take_powerup(kind, by)
{
	new name[32];
	get_user_name(by, name, charsmax(name));
	switch (kind)
	{
		case PU_RESUPPLY:
		{
			for (new p = 1; p <= MaxClients; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_CT)
					ls_give_item(p, "ammo");
			toast(0, 3.0, false, "Full resupply! (%s)", name);
		}
		case PU_DOUBLE:
		{
			g_doubleUntil = get_gametime() + 30.0;
			toast(0, 3.0, false, "Double pay for 30 seconds! (%s)", name);
		}
		case PU_ONEHIT:
		{
			g_instaUntil = get_gametime() + 30.0;
			toast(0, 3.0, false, "One-hit kills for 30 seconds! (%s)", name);
		}
		case PU_AIRSTRIKE:
		{
			new n = 0, e = -1;
			while ((e = find_ent_by_class(e, "ls_creature")))
			{
				if (pev(e, pev_deadflag) != DEAD_NO)
					continue;
				new type[16];
				pev(e, pev_netname, type, charsmax(type));
				if (equal(type, "boss") || equal(type, "gonarch") || equal(type, EXTRA[EX_HEAVY]))
					continue;
				ExecuteHamB(Ham_TakeDamage, e, 0, 0, entity_get_float(e, EV_FL_health) + 1000.0, DMG_BLAST);
				n++;
			}
			for (new p = 1; p <= MaxClients; p++)
				if (is_user_connected(p) && cs_get_user_team(p) == CS_TEAM_CT)
					pay(p, 400);
			shake_all();
			toast(0, 3.0, false, "Airstrike! %d cleared (%s)", n, name);
		}
	}
	log_amx("Last Stand: %s picked up by %d in wave %d", PU_NAME[kind], by, g_wave);
}

stock clear_powerups()
{
	for (new i = 0; i < sizeof g_puEnt; i++)
	{
		if (g_puEnt[i] && pev_valid(g_puEnt[i]))
			remove_entity(g_puEnt[i]);
		g_puEnt[i] = 0;
	}
	g_doubleUntil = 0.0;
	g_instaUntil = 0.0;
}

/* =========================================================================
 * The armory: Last Stand's own shop (CS buying stays the map's buy zone)
 * ========================================================================= */

new Float:g_shop[3];
new Float:g_useNext[33];
new Float:g_menuUntil[33];

new const SHOP_NAME[][] = { "Barricade repair kit", "Armor", "3 HE grenades", "Ammo", "Medkit" };
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
				copy(out, len, "you have full armor");
		}
		case 2: if (g_hePack[id] >= 3) copy(out, len, "you have 3 already");
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
			/* the game carries one HE at a time (the client shows and throws only one), so the
			   pack is one in hand and two more: each throw brings out the next, and it says how
			   many are left */
			if (!user_has_weapon(id, CSW_HEGRENADE))
				ls_give_item(id, "he");
			cs_set_user_bpammo(id, CSW_HEGRENADE, 1);
			g_hePack[id] = 3;
			if (!is_user_bot(id))
				client_print(id, print_center, "3 HE grenades: throw one and the next comes out");
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
	if (!is_user_bot(id))
	{
		if (g_hePack[id] > 0)
			client_print(id, print_center, "HE grenades: %d more", g_hePack[id]);
		else
			client_print(id, print_center, "That was your last HE grenade");
	}
	return FMRES_IGNORED;
}

public task_he_next(tid)
{
	new id = tid - TASK_REV - 100;
	if (!is_user_alive(id) || g_hePack[id] <= 0)
		return;
	if (!user_has_weapon(id, CSW_HEGRENADE))
		ls_give_item(id, "he");
	else if (cs_get_user_bpammo(id, CSW_HEGRENADE) < 1)
		cs_set_user_bpammo(id, CSW_HEGRENADE, 1);
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

/* best first; prices as the game's. 0-2 are rifles */
new const g_botGuns[][] = { "weapon_m4a1", "weapon_aug", "weapon_famas", "weapon_p90", "weapon_ump45", "weapon_mp5navy" };
new const g_botGunCost[] = { 3100, 3500, 2250, 2350, 1700, 1500 };
new const g_botGunAmmo[][] = { "556Nato", "556Nato", "556Nato", "57mm", "45ACP", "9mm" };
#define RIFLE_LAST 2

stock bots_shop_tick()
{
	if (g_phase != PH_BREAK && g_phase != PH_PREPARE)
		return;
	/* guns first: the whole stand is one round and CS bots only buy at round start, so between
	   waves a bot in the base, at the armory or in a buy zone buys here. A rifle comes before
	   everything else (QA: buying SMGs, armour and grenades kept them at $200-2500, never a rifle) */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		if (get_gametime() < g_gunNext[p])
			continue;
		if (!at_shop(p) && !cs_get_user_buyzone(p) && !in_base(p))
			continue;
		g_gunNext[p] = get_gametime() + 5.0;
		bot_buy_guns(p);
	}
	/* the armory: armour, ammo, sometimes grenades or a repair kit - once it has a rifle, or with
	   the rifle's money kept */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT || !at_shop(p))
			continue;
		if (get_gametime() < g_useNext[p])
			continue;
		g_useNext[p] = get_gametime() + 20.0;
		new bool:rifle = bot_gun_rank(p) <= RIFLE_LAST;
		new keep = rifle ? 0 : g_botGunCost[0];
		new CsArmorType:t;
		if (cs_get_user_armor(p, t) < 60 && cs_get_user_money(p) - shop_price(1) >= keep)
			shop_buy(p, 1, false);
		if (cs_get_user_money(p) - shop_price(3) >= keep)
			shop_buy(p, 3, false);
		if (random_num(0, 2) == 0 && cs_get_user_money(p) - shop_price(2) >= keep)
			shop_buy(p, 2, false);
		if (random_num(0, 3) == 0 && cs_get_user_money(p) - shop_price(0) >= keep)
			shop_buy(p, 0, false);
	}
}

/* the bot's best primary as an index in g_botGuns (0 best; a floor AK, sniper or machine gun
   counts as 0; sizeof g_botGuns = none of them) */
stock bot_gun_rank(p, primary[] = "", len = 0)
{
	new weapons[32], n, have = sizeof g_botGuns;
	get_user_weapons(p, weapons, n);
	if (len)
		primary[0] = EOS;
	for (new i = 0; i < n; i++)
	{
		new name[32];
		get_weaponname(weapons[i], name, charsmax(name));
		for (new g = 0; g < sizeof g_botGuns; g++)
			if (g < have && equal(name, g_botGuns[g]))
				have = g;
		if (equal(name, "weapon_ak47") || equal(name, "weapon_sg552") || equal(name, "weapon_m249") || equal(name, "weapon_awp")
			|| equal(name, "weapon_g3sg1") || equal(name, "weapon_sg550") || equal(name, "weapon_galil"))
			have = 0;
		if (len && ((1 << weapons[i]) & PRIMARY_BITS))
			copy(primary, len, name);
	}
	return have;
}

stock bot_buy_guns(p)
{
	new primary[32];
	new have = bot_gun_rank(p, primary, charsmax(primary));
	new money = cs_get_user_money(p);
	/* with no primary: the best it can pay for. With an SMG: only a rifle (it saves for one).
	   $100 kept for a board repair */
	new last = have >= sizeof g_botGuns ? sizeof g_botGuns - 1 : min(have - 1, RIFLE_LAST);
	for (new g = 0; g <= last; g++)
	{
		if (money - g_botGunCost[g] < 100)
			continue;
		if (primary[0])
			engclient_cmd(p, "drop", primary);
		give_item(p, g_botGuns[g]);
		ExecuteHamB(Ham_GiveAmmo, p, 90, g_botGunAmmo[g], 200);
		money -= g_botGunCost[g];
		cs_set_user_money(p, money);
		g_botGunsBought++;
		if (g <= RIFLE_LAST)
			g_botRifles++;
		have = g;
		break;
	}
	/* a Deagle once the rifle's there and money's left over */
	if (have <= RIFLE_LAST && !user_has_weapon(p, CSW_DEAGLE) && money - 650 >= 400)
	{
		new pistols[][] = { "weapon_usp", "weapon_glock18", "weapon_p228", "weapon_fiveseven", "weapon_elite" };
		for (new i = 0; i < sizeof pistols; i++)
			if (user_has_weapon(p, get_weaponid(pistols[i])))
				engclient_cmd(p, "drop", pistols[i]);
		give_item(p, "weapon_deagle");
		ExecuteHamB(Ham_GiveAmmo, p, 35, "50AE", 35);
		cs_set_user_money(p, money - 650);
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
}

public task_keeper_greet(tid)
{
	new p = tid - TASK_REV - 200;
	if (!is_user_connected(p) || !g_inShop[p] || !g_keeper || !is_valid_ent(g_keeper))
		return;
	if (g_keeperKind == 2)
		client_print_color(p, print_team_default, "^4G-Man:^1 Weapons... on the B menu. Supplies... from me.");
	else if (g_keeperKind == 1)
		client_print_color(p, print_team_default, "^4Barney:^1 Guns are on the B menu, supplies from me.");
}

/* big announcements go on gm_components' toast line: one at a time, never two on top of each other */
stock toast(id, Float:secs, bool:replace, const fmt[], any:...)
{
	new msg[128];
	vformat(msg, charsmax(msg), fmt, 5);
	ls_toast(id, secs, msg, 255, 200, 60, replace ? 1 : 0);
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
			/* once, on walking in: the game's centre text; the shopkeeper says it in chat a
			   moment later (the first 3 visits), not on the same frame */
			client_print(p, print_center, "Armory: B for weapons - USE at the counter for supplies");
			if (g_shopVisits[p] <= 3 && g_keeper && is_valid_ent(g_keeper))
				set_task(1.5, "task_keeper_greet", TASK_REV + 200 + p);
		}
		else if (!in && g_inShop[p])
			leave_shop(p);
		g_inShop[p] = in;
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
		/* every type up to round 20 (and what each falls back to); a type the server doesn't
		   have is skipped by the game DLL */
		for (new i = 0; i < sizeof ROSTER; i++)
			if (ROSTER_FROM[i] <= min(n, 20))
			{
				add_word(list, len, ROSTER[i]);
				if (ROSTER_FALLBACK[i][0])
					add_word(list, len, ROSTER_FALLBACK[i]);
			}
		if (n >= 6)
			for (new i = 0; i < sizeof EXTRA; i++)
			{
				add_word(list, len, EXTRA[i]);
				if (EXTRA_FALLBACK[i][0])
					add_word(list, len, EXTRA_FALLBACK[i]);
			}
	}
	if (g_nrows && g_wBoss[0] && g_wBossEvery > 0 && g_wBossEvery <= n)
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
