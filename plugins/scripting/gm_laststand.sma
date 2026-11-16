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
/* (our plugin_natives: loner.inc's optional natives and ReAPI's, below) */
#define LS_OWN_PLUGIN_NATIVES
#include <loner>

/* ReAPI's (reapi_gamedll.inc): the game's own carry limit for an item, when ReAPI is there
   (dedicated servers); optional, so the mode still loads without it (a listen server) */
native rg_set_global_iteminfo(const weapon_id, const type, any:...);
#define LS_ITEMINFO_MAXAMMO1 3          /* ItemInfo_iMaxAmmo1 */

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
/* the round table (what each round brings) is gm_components' (ls_wave_table) */
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
new const PU_KEY[PU_COUNT][] = { "resupply", "double", "onehit", "airstrike" };
new g_puThisWave;
/* the upgrades are gm_components' (perk machines, the bench, the box, paid traps, the downed
   state); the armory menu sells them where the map has no machine */
enum { PK_TOUGH = 0, PK_HANDS, PK_DOUBLE, PK_REVIVE, PK_FEET, PK_COUNT };
new const PK_ID[PK_COUNT][] = { "tough", "hands", "doubletap", "revive", "feet" };
#define PERK_MAX 4
#define MK2_PRICE 3000
#define MK3_PRICE 6000
#define BOX_PRICE 950
#define TURRET_PRICE 2500
#define RIFLE_LAST 2
new bool:g_bledOut[33];
new Float:g_upNext[33];
new bool:g_hasPerkM, bool:g_hasBench;
new Float:g_shop[3];
new bool:g_downOn;
new bool:g_traderWarned;
new bool:g_headLast[2048];
new Float:g_doubleUntil, Float:g_instaUntil;
new g_special;	/* this round: 0 normal, 1 boss, 2 a special pack (ls_wave_kind) */
new g_cAliveMax;
new g_cTiers;
new g_capShown;
#define ROUND_SOUND "buttons/bell1.wav"
new g_record;
new g_mapname[64];
new g_classZombie;
new g_kills;
new g_hePack[33];
new g_cHeCarry, g_cFlashCarry, g_cSmokeCarry;
new bool:g_carryRaised;               /* the game's grenade limits raised by this mode (ReAPI) */
new g_cKeeper;
/* the armory is a gm_components ls_shop (the map's own, or made here at the shop spot) */
new g_shopEnt;
new g_repairing[33];
new Float:g_repairNext[33];
new Float:g_repairUntil[33];
new Float:g_repairResend[33];
new Float:g_repairPickAt[33];
new bool:g_repairCame[33];
new g_repairNo[33];
new Float:g_repairNoUntil[33];
new g_botRepaired;
new Float:g_beatAt;
new Float:g_stuckAt;
new Float:g_lastKillAt;
new Float:g_crPos[2048][3];
new Float:g_idlePos[2048][3];
new Float:g_idleAt[2048];
new Float:g_recallAt[2048];
new Float:g_recallTick;
new g_crStill[2048];
new g_bigTries[2048];
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

/* =========================================================================
 * Start-up
 * ========================================================================= */

public plugin_precache()
{
	g_sprBeam = precache_model("sprites/laserbeam.spr");
	precache_model("models/w_medkit.mdl");
	precache_model("models/w_assault.mdl");
	precache_model("models/w_battery.mdl");
	precache_model("models/w_weaponbox.mdl");
	precache_sound("buttons/spark5.wav");
	/* creature types can only spawn mid-map if they were precached as the map loaded */
	g_hasCreatureDll = get_cvar_pointer("ls_creature_ver") != 0;
	g_cCreatures = register_cvar("lst_creatures", "1");
	keeper_precache();
	/* the round table: waves.ini [laststand] if the Creature Studio wrote one, else the built-in */
	ls_wave_table("laststand", 2, true);
	if (g_hasCreatureDll && get_pcvar_num(g_cCreatures))
	{
		/* only the types this map's stand will use (each type loads its model and sounds) */
		new list[512];
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
	g_cWaveMoney = register_cvar("lst_wave_money", "800");
	g_cAliveMax = register_cvar("lst_alive_max", "0");
	g_cTiers = register_cvar("lst_tiers", "1");
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
	register_srvcmd("lst_jump", "cmd_jump");
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
	/* grenades a player can carry (with ReAPI: the game's limit; without, the shop's 3 HE still
	   come one after another) */
	g_cHeCarry = register_cvar("lst_he_carry", "3");
	g_cFlashCarry = register_cvar("lst_flash_carry", "2");
	g_cSmokeCarry = register_cvar("lst_smoke_carry", "1");
}

public plugin_natives()
{
	set_module_filter("ls__module_filter");
	set_native_filter("ls_native_filter");
}

/* ReAPI may not be loaded (a listen server): its native is optional, like loner.inc's */
public ls_native_filter(const name[], index, trap)
{
	if (!trap && equal(name, "rg_set_global_iteminfo"))
		return PLUGIN_HANDLED;
	return ls__native_filter(name, index, trap);
}

/* how many of each grenade a player can carry in this mode: the game's own limit (ReAPI), so
   buying and picking up more both work; put back to the game's 1 / 2 / 1 when the map ends */
stock carry_limits(bool:on)
{
	if (!LibraryExists("reapi", LibType_Library))
		return;
	if (on)
	{
		rg_set_global_iteminfo(CSW_HEGRENADE, LS_ITEMINFO_MAXAMMO1, clamp(get_pcvar_num(g_cHeCarry), 1, 10));
		rg_set_global_iteminfo(CSW_FLASHBANG, LS_ITEMINFO_MAXAMMO1, clamp(get_pcvar_num(g_cFlashCarry), 1, 10));
		rg_set_global_iteminfo(CSW_SMOKEGRENADE, LS_ITEMINFO_MAXAMMO1, clamp(get_pcvar_num(g_cSmokeCarry), 1, 10));
		g_carryRaised = true;
	}
	else if (g_carryRaised)
	{
		rg_set_global_iteminfo(CSW_HEGRENADE, LS_ITEMINFO_MAXAMMO1, 1);
		rg_set_global_iteminfo(CSW_FLASHBANG, LS_ITEMINFO_MAXAMMO1, 2);
		rg_set_global_iteminfo(CSW_SMOKEGRENADE, LS_ITEMINFO_MAXAMMO1, 1);
		g_carryRaised = false;
	}
}

public plugin_end()
	carry_limits(false);

public plugin_cfg()
{
	carry_limits(true);
	g_creatures = g_hasCreatureDll && get_pcvar_num(g_cCreatures) != 0;
	if (!ls_components_running())
		log_amx("Last Stand needs gm_components.amxx; add it to laststand.plugins.ini before gm_laststand.amxx.");
	log_amx("Last Stand on %s: enemies are %s (best wave here: %d)", g_mapname, g_creatures ? "creatures" : "zombie bots", g_record);
}

/* server, for tests: the next wave is this one (lst_jump <n> during a break or the prepare time) */
public cmd_jump()
{
	new a[8];
	read_argv(1, a, charsmax(a));
	new n = str_to_num(a);
	if (n >= 1 && (g_phase == PH_BREAK || g_phase == PH_PREPARE))
	{
		g_wave = n - 1;
		log_amx("Last Stand: next wave set to %d (test)", n);
	}
	return PLUGIN_HANDLED;
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
	new shell = 0, red = 0;
	while ((e = find_ent_by_class(e, "ls_creature")))
		if (pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_team) != 1)
		{
			cr++;
			/* tier / enrage looks: a glow shell, red-ish for tier 3 and enraged ones */
			if (pev(e, pev_renderfx) == kRenderFxGlowShell)
			{
				shell++;
				new Float:c[3];
				pev(e, pev_rendercolor, c);
				if (c[0] > 150.0 && c[1] < 120.0)
					red++;
			}
		}
	log_amx("Last Stand perf: %d creatures with a glow shell (%d red)", shell, red);
	new capnow = get_cvar_pointer("ls_creature_cap_now") ? get_cvar_num("ls_creature_cap_now") : -1;
	log_amx("Last Stand perf: %d entities of %d, %d creatures alive (cap %d, ls_creature_cap_now %d), %d of %d defenders up, phase %d wave %d", engfunc(EngFunc_NumberOfEntities), global_get(glb_maxEntities), cr, ls_wave_cap_now(), capnow, up, ct, g_phase, g_wave);
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
	for (new p = 1; p <= MaxClients; p++)
		ls_perk_clear(p);
	ls_comp_bots_hint("offer_clear all");
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
	find_upgrades();
	/* turrets: from the armory (a kit), bots by the brain's offer */
	ls_turret_setup(turrets_ok() ? TURRET_PRICE : 0, 150.0, 1, "turret", 0, true);
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
		if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
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
		if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
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
			new e = -1, type[32];
			while ((e = find_ent_by_class(e, "ls_creature")) > 0 && enemies < 2)
				if (pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_team) != 1 && in_base(e))
				{
					pev(e, pev_netname, type, charsmax(type));
					enemies += ls_wave_is_big(type) ? 2 : 1;
				}
		}
		else
			for (new p = 1; p <= MaxClients && enemies < 2; p++)
				if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_T && in_base(p))
					enemies++;
		for (new p = 1; p <= MaxClients && !inside; p++)
			if (is_user_alive(p) && !is_down(p) && cs_get_user_team(p) == CS_TEAM_CT && in_base(p, 350.0))
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
		if (is_user_alive(p) && !is_down(p))
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

stock start_wave(n)
{
	g_wave = n;
	g_phase = PH_WAVE;
	g_overrun = 0;
	g_lastChanceEnd = 0.0;
	g_puThisWave = 0;
	g_spawned = 0;
	set_cvar_num("mp_buy_anywhere", 0);
	/* the shop trips end; anyone still waiting for a revive gets their helpers back */
	ls_comp_bots_hint("goal_clear ct use");
	for (new d = 1; d <= MaxClients; d++)
		if (g_down[d])
			send_revive(d);
	/* the round table plans it (gm_components): how many, which types, health, a boss or a
	   special pack, how many alive at once; difficulty scales count and health */
	ls_wave_table("laststand", 2, g_creatures);
	ls_wave_scale(diff_mult(false), diff_mult(true));
	ls_wave_cap(get_pcvar_num(g_cAliveMax));
	ls_wave_tiers(get_pcvar_num(g_cTiers) != 0);
	ls_wave_start(n);
	g_planned = ls_wave_planned();
	new kind[16];
	g_special = ls_wave_kind(kind, charsmax(kind));
	if (!g_creatures && g_classZombie)
	{
		new v[8];
		num_to_str(floatround(100.0 * ls_wave_hp_mult()), v, charsmax(v));
		ls_comp_set(g_classZombie, "hp", v);
	}
	if (g_botRepaired > 0)
		log_amx("Last Stand: bots repaired %d barricade health before wave %d", g_botRepaired, n);
	g_botRepaired = 0;
	assign_defenders();
	g_stepSecs = 0;
	g_floorSecs = 0;
	g_kills = 0;
	g_waveAt = get_gametime();
	g_lastKillAt = g_waveAt;
	/* the round's start: a clear cue */
	if (g_special == 1)
		toast(0, 4.0, true, "Wave %d: a boss is coming!", n);
	else if (g_special == 2 && equal(kind, "hound"))
		toast(0, 4.0, true, "Wave %d: a hound pack! Fast and many", n);
	else if (g_special == 2)
		toast(0, 4.0, true, "Wave %d: a swarm! Small, fast, everywhere", n);
	else
		toast(0, 3.0, true, "Wave %d", n);
	shake_all();
	client_cmd(0, "spk ^"%s^"", ROUND_SOUND);
}

stock enemies_left()
{
	if (g_creatures)
		return max(ls_wave_planned() - ls_wave_spawned(), 0) + ls_wave_alive();
	return max(g_planned - g_spawned, 0) + ls_wave_alive();
}

/* the round table's spawners make the creatures; none outside a wave */
public ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[])
{
	return g_phase == PH_WAVE ? PLUGIN_CONTINUE : PLUGIN_HANDLED;
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
	if (g_special == 2)
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
		if (is_user_alive(p) && !is_down(p))
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
		/* creatures that won't spawn here: zombie bots instead */
		if (g_creatures && !ls_wave_creatures_ok())
		{
			log_amx("Last Stand: creatures won't spawn on %s; using zombie bots instead.", g_mapname);
			g_creatures = false;
			ensure_components();
			assign_teams(false);
		}
		/* the alive cap follows the game DLL's adaptive one through the round */
		new cap = ls_wave_cap_now();
		if (cap != g_capShown)
		{
			if (g_capShown)
				log_amx("Last Stand: creature cap now %d (wave %d)", cap, g_wave);
			g_capShown = cap;
			set_cvar_num("ls_comp_wave_alive_max", cap);
		}
		if (now >= g_recallTick)
		{
			g_recallTick = now + 3.0;
			recall_idle();
		}
		if (now >= g_stuckAt)
		{
			g_stuckAt = now + 15.0;
			unstick_creatures();
			/* the last one or two that nobody has killed for 3 minutes (one that keeps moving
			   somewhere nobody goes): they go, so a round can't hang */
			if (g_creatures && enemies_left() <= 2 && ls_wave_planned() <= ls_wave_spawned() && g_lastKillAt > 0.0 && now - g_lastKillAt > 180.0)
			{
				new e = -1;
				while ((e = find_ent_by_class(e, "ls_creature")) > 0)
					if (pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_team) != 1)
					{
						new Float:o[3];
						pev(e, pev_origin, o);
						new type[32];
						pev(e, pev_netname, type, charsmax(type));
						log_amx("Last Stand: creature %d (%s) at %.0f %.0f %.0f not killed for 3 minutes at the end of wave %d; removed", e, type, o[0], o[1], o[2], g_wave);
						server_cmd("ls_creature kill %d", e);
					}
				g_lastKillAt = now;
			}
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
	/* with the map's own machines the brain decides (gm_components' offers); else the armory */
	down_sync();
	if (g_shopEnt)
		ls_shop_open(g_shopEnt, g_phase != PH_IDLE && g_phase != PH_OVER);
	bots_upgrades();
}

/* A creature that hasn't moved for a minute with no defender within 800 units is stuck
   somewhere nobody can reach: it goes, so a wave can't hang on it */
stock unstick_creatures()
{
	if (!g_creatures)
		return;
	new e = -1, Float:o[3], Float:po[3];
	/* the last few: 30 s without moving is enough, near a defender or not (balance stands: they
	   wedged south of the south gate, inside 800 u of the base, and held rounds for minutes) */
	new bool:few = enemies_left() <= 3;
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e >= sizeof g_crPos || pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
			continue;
		pev(e, pev_origin, o);
		if (get_distance_f(o, g_crPos[e]) > 32.0)
		{
			g_crPos[e] = o;
			g_crStill[e] = 0;
			g_bigTries[e] = 0;
			continue;
		}
		if (++g_crStill[e] < (few ? 2 : 4))
			continue;
		new Float:dmin = 99999.0;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_CT)
				continue;
			pev(p, pev_origin, po);
			dmin = floatmin(dmin, get_distance_f(o, po));
		}
		/* (one of the last few that stands still while fighting - at a gate, or within 500 u of a
		   defender - is busy, not stuck; a ranged one counts as fighting from where it shoots) */
		if (few)
		{
			new bool:atGate = false;
			for (new b = ls_comp_find("ls_barricade"); b && !atGate; b = ls_comp_find("ls_barricade", b))
			{
				if (ls_barricade_hp(b) <= 0)
					continue;
				new Float:mn[3], Float:mx[3];
				pev(b, pev_absmin, mn);
				pev(b, pev_absmax, mx);
				atGate = near_box(e, mn, mx, 160.0);
			}
			if (atGate || dmin < 500.0)
				continue;
		}
		else if (dmin < 800.0)
			continue;
		new type[32];
		pev(e, pev_netname, type, charsmax(type));
		g_crStill[e] = 0;
		/* a boss or a heavy is never just removed (QA4-C: a garg wedged in a broken gateway went, and
		   nobody got the boss pay): moved to open floor near it, and only after three tries killed,
		   its pay shared by the defenders */
		if (ls_wave_is_big(type))
		{
			if (++g_bigTries[e] <= 3 && unwedge(e, o))
			{
				log_amx("Last Stand: %s %d wedged at %.0f %.0f %.0f; moved to open floor (try %d)", type, e, o[0], o[1], o[2], g_bigTries[e]);
				continue;
			}
			log_amx("Last Stand: %s %d wedged at %.0f %.0f %.0f and couldn't be moved; killed, its pay shared", type, e, o[0], o[1], o[2]);
			share_pay(ls_wave_type_pay(type));
			server_cmd("ls_creature kill %d", e);
			continue;
		}
		log_amx("Last Stand: creature %d (%s) stuck at %.0f %.0f %.0f for %s; removed", e, type, o[0], o[1], o[2], few ? "30 s (one of the last)" : "a minute");
		server_cmd("ls_creature kill %d", e);
	}
}

/* A ranged creature (spitter, vortigaunt, soldier) that knows where a defender is but can't see
   one stops where it is and waits (balance stands: four held wave 8 for 9 minutes from the south
   road, out of sight of everyone). After 12 s still and out of sight it's sent to guard the
   outside of the nearest gate, where it sees through the boards and fights. */
/* a free spot near a wedged creature, toward the base first: rings 64-256 u out, room for its hull,
   floor under it */
stock bool:unwedge(e, const Float:o[3])
{
	new Float:mn[3], Float:mx[3];
	pev(e, pev_mins, mn);
	pev(e, pev_maxs, mx);
	new Float:toBase = floatatan2(g_base[1] - o[1], g_base[0] - o[0], degrees);
	for (new r = 64; r <= 256; r += 48)
		for (new k = 0; k < 12; k++)
		{
			/* 0, +30, -30, +60, -60 ... degrees from the way to the base */
			new Float:a = toBase + 30.0 * float((k + 1) / 2) * (k % 2 ? 1.0 : -1.0);
			new Float:t[3];
			t[0] = o[0] + float(r) * floatcos(a, degrees);
			t[1] = o[1] + float(r) * floatsin(a, degrees);
			t[2] = o[2] + 16.0;
			new Float:f = floor_under(t, e);
			if (floatabs(f - o[2]) > 64.0 && floatabs(f - (o[2] + mn[2])) > 64.0)
				continue;
			t[2] = f - mn[2] + 2.0;
			engfunc(EngFunc_TraceHull, t, t, DONT_IGNORE_MONSTERS, (mx[0] - mn[0]) > 40.0 ? HULL_LARGE : HULL_HUMAN, e, 0);
			if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
				continue;
			engfunc(EngFunc_SetOrigin, e, t);
			/* (from here: moving on by itself clears its tries; staying wedged counts on) */
			g_crPos[e] = t;
			return true;
		}
	return false;
}

stock share_pay(amount)
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_CT)
			n++;
	if (!n)
		return;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && cs_get_user_team(p) == CS_TEAM_CT)
			pay(p, max(amount / n, 10));
}

stock recall_idle()
{
	if (!g_creatures || g_base[0] == 0.0 && g_base[1] == 0.0)
		return;
	new Float:now = get_gametime();
	new e = -1, Float:o[3];
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e >= sizeof g_idlePos || pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
			continue;
		pev(e, pev_origin, o);
		if (get_distance_f(o, g_idlePos[e]) > 32.0 || g_idleAt[e] <= 0.0)
		{
			g_idlePos[e] = o;
			g_idleAt[e] = now;
			continue;
		}
		if (now - g_idleAt[e] < 12.0 || now < g_recallAt[e])
			continue;
		/* busy at a gate, or someone in sight: it's fighting */
		new best = 0, Float:bd = 99999.0, Float:gc[3], bool:atGate = false;
		for (new b = ls_comp_find("ls_barricade"); b; b = ls_comp_find("ls_barricade", b))
		{
			new Float:c[3];
			gate_centre(b, c);
			new Float:d = get_distance_f(o, c);
			if (d < 220.0)
				atGate = true;
			if (d < bd)
			{
				bd = d;
				best = b;
				gc = c;
			}
		}
		if (atGate || !best || creature_sees_defender(e, o))
			continue;
		/* right outside the gate (64 u out from its middle, away from the base): it hits the boards there */
		new Float:dx = gc[0] - g_base[0], Float:dy = gc[1] - g_base[1];
		new Float:ln = floatsqroot(dx * dx + dy * dy);
		if (ln < 1.0)
			continue;
		new Float:t[3];
		t[0] = gc[0] + 64.0 * dx / ln;
		t[1] = gc[1] + 64.0 * dy / ln;
		t[2] = gc[2];
		t[2] = floor_under(t) + 8.0;
		new type[32];
		pev(e, pev_netname, type, charsmax(type));
		log_amx("Last Stand: creature %d (%s) idle out of sight at %.0f %.0f %.0f; sent to the gate at %.0f %.0f", e, type, o[0], o[1], o[2], t[0], t[1]);
		server_cmd("ls_creature behave %d guard %.0f %.0f %.0f;ls_creature target %d 0", e, t[0], t[1], t[2], e);
		g_recallAt[e] = now + 20.0;
		g_idleAt[e] = now;
	}
}

stock bool:creature_sees_defender(e, const Float:o[3])
{
	new Float:a[3], Float:b[3], Float:frac;
	a = o;
	a[2] += 32.0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || cs_get_user_team(p) != CS_TEAM_CT)
			continue;
		pev(p, pev_origin, b);
		b[2] += 17.0;
		engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, e, 0);
		get_tr2(0, TR_flFraction, frac);
		if (frac >= 1.0)
			return true;
	}
	return false;
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
			/* a bot that hasn't come within 300 u 9 s after it was picked turned the job down (the brain
			   decides once per gate, W2): let it go and ask the next nearest */
			new Float:po[3];
			pev(p, pev_origin, po);
			if (get_distance_f(po, c) < 300.0)
				g_repairCame[p] = true;
			else if (!g_repairCame[p] && now - g_repairPickAt[p] > 9.0)
			{
				g_repairNo[p] = b;
				g_repairNoUntil[p] = now + 60.0;
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
				if (g_repairNo[p] == b && now < g_repairNoUntil[p])
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
			g_repairPickAt[best] = now;
			g_repairCame[best] = false;
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
			if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
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
				if (pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_team) != 1 && near_box(e, mn, mx, 56.0) && outside(e, mn, mx))
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
		if (is_user_alive(p) && !is_down(p))
			len += formatex(team[len], charsmax(team) - len, "%s  %d^n", name, get_user_health(p));
		else if (ls_is_down(p))
			len += formatex(team[len], charsmax(team) - len, "%s  DOWN %ds^n", name, floatround(ls_down_left(p)));
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
		new tags[48];
		ls_perk_tags(p, tags, charsmax(tags));
		if (ls_is_down(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "%s^nDOWN - %d s to be revived", top, floatround(ls_down_left(p)));
		else if (g_down[p])
			ls_hud(p, LS_HUD_TOP, 1.1, "%s^nDOWN - %d s to be revived", top, max(floatround(get_pcvar_float(g_cRevive) - (now - g_downAt[p])), 0));
		else if (at_shop(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "%s^nArmory  -  B: weapons   USE at the counter: supplies%s%s", top, tags[0] ? "^n" : "", tags);
		else
			ls_hud(p, LS_HUD_TOP, 1.1, "%s%s%s", top, tags[0] ? "^n" : "", tags);
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
	/* bled out (the downed state ran out): out until the next wave */
	if (g_bledOut[victim])
	{
		g_bledOut[victim] = false;
		clear_down(victim);
		new name[32];
		get_user_name(victim, name, charsmax(name));
		ls_chat(0, "%s bled out and is out until the next wave.", name);
		return HAM_IGNORED;
	}
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
	/* (a defenders' turret going down) */
	if (pev(ent, pev_team) == 1)
		return HAM_IGNORED;
	g_kills++;
	g_lastKillAt = get_gametime();
	if (!(1 <= killer <= MaxClients) || !is_user_connected(killer))
		return HAM_IGNORED;
	new type[32];
	pev(ent, pev_netname, type, charsmax(type));
	/* points: the type's pay, more for a head or a knife */
	new money = ls_wave_type_pay(type);
	if (ent < sizeof g_headLast && g_headLast[ent])
		money += 50;
	if (get_user_weapon(killer) == CSW_KNIFE)
		money += 50;
	pay(killer, money);
	maybe_powerup(ent);
	if (equal(type, "boss") || equal(type, "gonarch") || (ls_wave_is_big(type) && g_wave == 7))
	{
		new name[32];
		get_user_name(killer, name, charsmax(name));
		toast(0, 4.0, false, "%s brought the boss down!", name);
	}
	return HAM_IGNORED;
}

/* =========================================================================
 * Upgrades (zombie-survival style, our own names): gm_components has them (perks, the downed
 * state, the upgrade bench, paid electric traps, the mystery box). Map spots: lst_perk (perk=
 * tough|hands|doubletap|revive|feet), lst_bench, lst_trap (size or radius), lst_box markers
 * become those components. Without them: perks and weapon upgrades from the armory menu, a box
 * at the armory.
 * ========================================================================= */

/* down in a wave (alive, crawling) or dead and waiting for a revive */
stock bool:is_down(p)
{
	return g_down[p] || ls_is_down(p);
}

/* the downed state is on in waves only */
stock down_sync()
{
	new bool:on = g_phase == PH_WAVE && get_pcvar_num(g_cRevive) > 0;
	if (on == g_downOn)
		return;
	g_downOn = on;
	ls_down_enable(on, floatmax(get_pcvar_float(g_cRevive), 1.0), 2, true);
	/* getting a teammate up pays */
	ls_down_revive_pay(150);
	/* (Battle Royale's knock-down finishing off: creatures leave downed defenders alone) */
	set_cvar_num("ls_down_finish", 0);
}

public ls_on_down(id, attacker)
{
	if (cs_get_user_team(id) != CS_TEAM_CT)
		return PLUGIN_HANDLED;
	new what[32] = "?", Float:o[3];
	if (attacker > 0 && pev_valid(attacker))
	{
		pev(attacker, pev_classname, what, charsmax(what));
		if (equal(what, "ls_creature"))
			pev(attacker, pev_netname, what, charsmax(what));
	}
	pev(id, pev_origin, o);
	log_amx("Last Stand: defender %d down in wave %d by %s at %.0f %.0f %.0f (%s the base)", id, g_wave, what, o[0], o[1], o[2], in_base(id) ? "in" : "outside");
	new name[32];
	get_user_name(id, name, charsmax(name));
	ls_chat(0, "%s is down! Hold USE on them to revive (%d s left).", name, get_pcvar_num(g_cRevive));
	return PLUGIN_CONTINUE;
}

public ls_on_bled_out(id)
{
	g_bledOut[id] = true;
}

public ls_on_revive(id, by)
{
	new a[32], b[32];
	get_user_name(id, a, charsmax(a));
	if (by == id)
		ls_chat(0, "%s got back up (Quick revive).", a);
	else if (1 <= by <= MaxClients && is_user_connected(by))
	{
		get_user_name(by, b, charsmax(b));
		ls_chat(0, "%s revived %s.", b, a);
		log_amx("Last Stand: %d revived %d in wave %d", by, id, g_wave);
	}
}

/* what the map has: machines, a bench, a box (else the armory sells them; a box goes there) */
stock find_upgrades()
{
	g_hasPerkM = ls_comp_count("ls_perk_machine") > 0;
	g_hasBench = ls_comp_count("ls_upgrade_bench") > 0;
	if (!ls_comp_count("ls_mystery_box"))
	{
		new Float:o[3];
		o = g_shop;
		if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
			o = g_base;
		else
			box_beside(o);
		o[2] += 16.0;
		ls_comp_create("ls_mystery_box", o, "team=ct moves_after=0 name=^"Mystery box^"");
	}
}

/* the box beside the armory, not on it: the shopkeeper stands at the armory's spot, and USE
   there opened the box instead of his menu. 112 u to one side, with a clear line from the spot
   and a floor under it (else where it was) */
stock box_beside(Float:o[3])
{
	static const Float:dirs[4][2] = { { 1.0, 0.0 }, { -1.0, 0.0 }, { 0.0, 1.0 }, { 0.0, -1.0 } };
	new Float:a[3], Float:b[3], Float:down[3], Float:frac;
	a = o;
	a[2] += 24.0;
	for (new k = 0; k < 4; k++)
	{
		b[0] = a[0] + dirs[k][0] * 112.0;
		b[1] = a[1] + dirs[k][1] * 112.0;
		b[2] = a[2];
		engfunc(EngFunc_TraceHull, a, b, IGNORE_MONSTERS, HULL_HEAD, 0, 0);
		get_tr2(0, TR_flFraction, frac);
		if (frac < 1.0 || get_tr2(0, TR_StartSolid))
			continue;
		down = b;
		down[2] -= 120.0;
		engfunc(EngFunc_TraceLine, b, down, IGNORE_MONSTERS, 0, 0);
		get_tr2(0, TR_flFraction, frac);
		if (frac >= 1.0)
			continue;
		get_tr2(0, TR_vecEndPos, down);
		o[0] = down[0];
		o[1] = down[1];
		o[2] = down[2];
		return;
	}
}

/* bots: perks (Tough first), the bench, the box now and then - in breaks, once their rifle is
   bought and money is left over (at the armory or in the base) */
stock bots_upgrades()
{
	if (g_phase != PH_BREAK && g_phase != PH_PREPARE)
		return;
	static const ORDER[] = { PK_TOUGH, PK_FEET, PK_REVIVE, PK_DOUBLE, PK_HANDS };
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT || is_down(p))
			continue;
		if (get_gametime() < g_upNext[p] || bot_gun_rank(p) > RIFLE_LAST)
			continue;
		if (!at_shop(p) && !in_base(p))
			continue;
		g_upNext[p] = get_gametime() + 4.0;
		new money = cs_get_user_money(p);
		new n = ls_perk_count(p);
		/* from wave 6, with 2 perks: the gun's Mk II before more perks (stands: no bench ever) */
		if (!g_hasBench && n >= 2 && g_wave >= 6 && ls_weapon_level(p) == 1)
		{
			if (money >= MK2_PRICE + 300)
				ls_bench_use(p, false);
			continue;
		}
		if (!g_hasPerkM && n < PERK_MAX && money >= ls_perk_price(p) + 400)
		{
			for (new k = 0; k < sizeof ORDER; k++)
				if (!ls_perk_has(p, PK_ID[ORDER[k]]))
				{
					ls_perk_buy(p, PK_ID[ORDER[k]], false);
					break;
				}
			continue;
		}
		if (!g_hasBench)
		{
			new lv = ls_weapon_level(p);
			if ((lv == 1 && n >= 2 && money >= MK2_PRICE + 600) || (lv == 2 && n >= 3 && money >= MK3_PRICE + 600))
			{
				ls_bench_use(p, false);
				continue;
			}
		}
		if (money >= BOX_PRICE + 2500 && random_num(1, 8) == 1)
			ls_box_use(0, p, false);
	}
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
	/* (double tap and the gun's Mk: gm_components) one-hit kills: anything but a boss or a heavy goes down to the next hit */
	if (get_gametime() < g_instaUntil)
	{
		new type[32];
		pev(ent, pev_netname, type, charsmax(type));
		if (!ls_wave_is_big(type))
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

/* a gm_components pickup: whoever on the team walks over it takes it (ls_on_pickup) */
stock spawn_powerup(kind, const Float:o[3])
{
	if (ls_comp_count("ls_pickup") >= 8)
		return;
	if (!ls_pickup_spawn(PU_KEY[kind], o, 25.0, PU_MODEL[kind], PU_RGB[kind][0], PU_RGB[kind][1], PU_RGB[kind][2], 2))
		return;
	g_puThisWave++;
	toast(0, 2.0, false, "%s dropped!", PU_NAME[kind]);
}

public ls_on_pickup(ent, id, const kind[])
{
	for (new k = 0; k < PU_COUNT; k++)
		if (equal(kind, PU_KEY[k]))
		{
			take_powerup(k, id);
			return PLUGIN_CONTINUE;
		}
	return PLUGIN_CONTINUE;
}

public tick_powerups()
{
	new Float:now = get_gametime();
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
				if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
					continue;
				new type[32];
				pev(e, pev_netname, type, charsmax(type));
				if (ls_wave_is_big(type))
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
	new e = ls_comp_find("ls_pickup");
	while (e)
	{
		new next = ls_comp_find("ls_pickup", e);
		ls_comp_remove(e);
		e = next;
	}
	g_doubleUntil = 0.0;
	g_instaUntil = 0.0;
}

/* =========================================================================
 * The armory: Last Stand's own shop (CS buying stays the map's buy zone)
 * ========================================================================= */

new Float:g_useNext[33];
new const SHOP_ITEMS[] = "Barricade repair kit|600|@repair|rebuild the most broken barricade fully; Armor|650|vesthelm|kevlar + helmet; 3 HE grenades|900|@he3|three HE grenades; Ammo|300|ammo|refill all your guns; Medkit|500|hp:50|+50 HP";
/* with friendly creatures in the game DLL (ls-next3 cfd2c3d+): a sentry turret, placed from a kit */
new const SHOP_TURRET[] = "; Sentry turret|2500|turret|guards where you put it for 150 s";


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
	shop_component();
}

/* the map's ls_shop if it has one, else one made at the shop spot with Last Stand's items */
stock shop_component()
{
	if (!g_shopEnt || !ls_comp_is(g_shopEnt))
		g_shopEnt = ls_comp_find("ls_shop");
	if (g_shopEnt)
	{
		shop_scale();
		return;
	}
	new keeper[32];
	get_pcvar_string(g_cKeeper, keeper, charsmax(keeper));
	new keys[512], Float:o[3];
	o = g_shop;
	formatex(keys, charsmax(keys), "name=Armory radius=160 height=200 team=ct price_rise=0.05 upgrades=1 buy_menu=1 keeper=%s hint=^"Armory: B for weapons - USE at the counter for supplies^" greet=^"%s^" items=^"%s^"",
		keeper, equali(keeper, "gman") ? "Weapons... on the B menu. Supplies... from me." : "Guns are on the B menu, supplies from me.", SHOP_ITEMS);
	if (turrets_ok())
	{
		new l = strlen(keys);
		if (l > 0 && keys[l - 1] == '"')
		{
			keys[l - 1] = EOS;
			format(keys, charsmax(keys), "%s%s^"", keys, SHOP_TURRET);
		}
	}
	g_shopEnt = ls_comp_create("ls_shop", o, keys);
	if (!g_shopEnt)
		log_amx("Last Stand: couldn't make the armory");
	else
		shop_scale();
}

stock bool:turrets_ok()
{
	return g_creatures && get_cvar_pointer("ls_creature_friendly_max") != 0;
}

stock shop_scale()
{
	new v[16];
	formatex(v, charsmax(v), "%.2f", get_pcvar_float(g_cShopScale));
	ls_comp_set(g_shopEnt, "scale", v);
}

/* the armory's own items */
public ls_on_shop_item(ent, id, const item[])
{
	if (equal(item, "@repair") && !most_broken_barricade())
	{
		ls_shop_why("the barricades are fine");
		return PLUGIN_HANDLED;
	}
	if (equal(item, "@he3") && (g_carryRaised ? cs_get_user_bpammo(id, CSW_HEGRENADE) >= clamp(get_pcvar_num(g_cHeCarry), 1, 10) : g_hePack[id] >= 3))
	{
		ls_shop_why("you have 3 already");
		return PLUGIN_HANDLED;
	}
	return PLUGIN_CONTINUE;
}

public ls_on_shop_buy(ent, id, const item[], price)
{
	if (equal(item, "@repair"))
		ls_barricade_repair(most_broken_barricade(), 0);
	else if (equal(item, "@he3"))
	{
		/* the game carries one HE at a time (the client shows and throws only one), so the
		   pack is one in hand and two more: each throw brings out the next, and it says how
		   many are left */
		if (!user_has_weapon(id, CSW_HEGRENADE))
			ls_give_item(id, "he");
		/* with the game's limit raised: three more in the pack, up to it */
		if (g_carryRaised)
		{
			new cap = clamp(get_pcvar_num(g_cHeCarry), 1, 10);
			cs_set_user_bpammo(id, CSW_HEGRENADE, min(max(cs_get_user_bpammo(id, CSW_HEGRENADE), 0) + 3, cap));
			if (!is_user_bot(id))
				client_print(id, print_center, "HE grenades: %d", cs_get_user_bpammo(id, CSW_HEGRENADE));
			return PLUGIN_CONTINUE;
		}
		cs_set_user_bpammo(id, CSW_HEGRENADE, 1);
		g_hePack[id] = 3;
		if (!is_user_bot(id))
			client_print(id, print_center, "3 HE grenades: throw one and the next comes out");
	}
	return PLUGIN_CONTINUE;
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

public cmd_shop(id)
{
	if (g_phase == PH_IDLE || !is_user_alive(id))
		return PLUGIN_HANDLED;
	if (!at_shop(id))
	{
		ls_chat(id, "The armory is at the base: go there and press USE (or say /shop).");
		return PLUGIN_HANDLED;
	}
	ls_shop_menu(g_shopEnt, id);
	return PLUGIN_HANDLED;
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
		if (!g_shopEnt || !is_user_alive(p) || !is_user_bot(p) || cs_get_user_team(p) != CS_TEAM_CT || !at_shop(p))
			continue;
		if (get_gametime() < g_useNext[p])
			continue;
		g_useNext[p] = get_gametime() + 20.0;
		new bool:rifle = bot_gun_rank(p) <= RIFLE_LAST;
		new keep = rifle ? 0 : g_botGunCost[0];
		new CsArmorType:t;
		if (cs_get_user_armor(p, t) < 60 && cs_get_user_money(p) - ls_shop_price(g_shopEnt, "vesthelm") >= keep)
			ls_shop_buy(g_shopEnt, p, "vesthelm", false);
		if (cs_get_user_money(p) - ls_shop_price(g_shopEnt, "ammo") >= keep)
			ls_shop_buy(g_shopEnt, p, "ammo", false);
		if (random_num(0, 2) == 0 && cs_get_user_money(p) - ls_shop_price(g_shopEnt, "@he3") >= keep)
			ls_shop_buy(g_shopEnt, p, "@he3", false);
		if (random_num(0, 3) == 0 && cs_get_user_money(p) - ls_shop_price(g_shopEnt, "@repair") >= keep)
			ls_shop_buy(g_shopEnt, p, "@repair", false);
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

/* big announcements go on gm_components' toast line: one at a time, never two on top of each other */
stock toast(id, Float:secs, bool:replace, const fmt[], any:...)
{
	new msg[128];
	vformat(msg, charsmax(msg), fmt, 5);
	ls_toast(id, secs, msg, 255, 200, 60, replace ? 1 : 0);
}

/* =========================================================================
 * The shopkeeper: Barney (or the G-Man) at the armory, from Half-Life's own models; the
 * ls_shop component places and animates him (keeper=), the model is loaded here
 * ========================================================================= */

stock keeper_precache()
{
	g_cKeeper = register_cvar("lst_shopkeeper", "barney");
	new want[16];
	get_pcvar_string(g_cKeeper, want, charsmax(want));
	if (equali(want, "none"))
		return;
	new m[64], alt[96];
	formatex(m, charsmax(m), "models/%s.mdl", equali(want, "gman") ? "gman" : "barney");
	/* Half-Life's models live in valve/, which the engine reads from cstrike too */
	formatex(alt, charsmax(alt), "../valve/%s", m);
	if (!file_exists(m) && !file_exists(alt))
	{
		log_amx("Last Stand: %s not found on this server; the shopkeeper is a CS model instead.", m);
		return;
	}
	precache_model(m);
}

/* the types waves 1..lst_waves use (every wave when endless), from the round table */
stock precache_types(list[], len)
{
	new p = get_cvar_pointer("lst_waves");
	/* (the first map after a start: lst_waves isn't registered yet; its default is 0, endless) */
	new n = p ? get_pcvar_num(p) : 0;
	ls_wave_types(list, len, n > 0 ? n : 1000);
	/* the defenders' sentry turrets */
	add_word(list, len, "turret");
}

stock add_word(list[], len, const word[])
{
	if (!word[0])
		return;
	new padded[600], w[40];
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
