/*
 * gm_components.sma - Loner-Strike's game mode components.
 *
 * Every component is an ls_* entity with typed keys (app/src-tauri/src/editor/
 * lonerstrike.fgd): placed in a map, listed in a map pack for maps that weren't
 * built for it, or made at run time by a mode (ls_comp_create). This plugin runs
 * their logic, shows them to players within what the stock 1.6 client can draw,
 * tells Loner-Strike's bots what they mean (ls_mode server commands, when the
 * bots' brain has them), and calls ls_on_* forwards in every plugin.
 *
 * Natives and forwards: include/ls_components.inc. Each component, its keys,
 * events and bot meaning: docs/COMPONENTS.md. The plan: docs/GAMEMODES-PLAN.md.
 *
 * Map packs (no BSP edits), loaded at map start, later lines may add more:
 *   addons/amxmodx/configs/gamemodes/components/<map>.ini      (shipped)
 *   addons/amxmodx/configs/gamemodes/components/<map>.<mode>.ini
 *   addons/amxmodx/configs/components/<map>.ini                (your own)
 *   addons/amxmodx/configs/components/<map>.<mode>.ini
 * One component per line:  ls_capture_point 120 -340 64 radius=160 name="B"
 *
 * Loot tables: configs/gamemodes/loot.ini, then configs/gamemodes/<mode>.loot.ini.
 *
 * Admin console (amx_cvar access): ls_comp_list, ls_comp_spawn <classname> [key=value ...]
 * (where you aim), ls_comp_del <ent>, ls_comp_save (writes configs/components/<map>.ini),
 * ls_comp_start <ent|0>, ls_comp_stop <ent|0>, ls_comp_reset.
 */

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>
#include <hamsandwich>
#include <cstrike>
#include <fun>

#pragma semicolon 1

#define PLUGIN  "Loner-Strike Components"
#define VERSION "0.1"

#define MAX_COMP    384
#define MAX_ENTS    2048
#define MAX_DOORS   4
#define MAX_TRACK   64
#define MAX_TABLES  32
#define TICK        0.25
#define TASK_TICK   7736100
#define TASK_SPAWN  7736200
#define TASK_MENU   7736300

enum
{
	K_NONE = 0,
	K_RESPAWN, K_WEAPON, K_DAMAGE, K_DOOR, K_TRAP, K_CAPTURE, K_ZONE, K_SHRINKER,
	K_LOOT, K_DROP, K_BARRICADE, K_WAVE, K_EXTRACT, K_CHECKPOINT, K_PLATFORM,
	K_CLASS, K_MEETING, K_CRATE,
	K_PERK, K_BENCH, K_BOX, K_PICKUP, K_SHOP, /* W1 */
	K_COUNT
};

new const KIND_NAME[K_COUNT][] =
{
	"",
	"ls_respawn_point", "ls_weapon_spawner", "ls_damage_zone", "ls_timed_door", "ls_trigger_trap",
	"ls_capture_point", "ls_safe_zone", "ls_zone_shrinker", "ls_loot_container", "ls_supply_drop",
	"ls_barricade", "ls_wave_spawner", "ls_extraction_zone", "ls_checkpoint", "ls_moving_platform",
	"ls_player_class", "ls_meeting_point", "ls_supply_crate",
	"ls_perk_machine", "ls_upgrade_bench", "ls_mystery_box", "ls_pickup", "ls_shop" /* W1 */
};

/* where a component came from */
enum { SRC_MAP = 0, SRC_PACK, SRC_RUNTIME };

/* hold-USE actions */
enum { ACT_NONE = 0, ACT_LOOT, ACT_REPAIR, ACT_LOCK, ACT_MEET };

/* per-kind slots in g_pf[][] (floats read from keys or kept as state) */
/* capture point */
#define CP_SECS     0
#define CP_DECAY    1
#define CP_LOCK     2
#define CP_MULT     3
#define CP_FFA      4
#define CP_TEAMLOCK 5
#define CP_CONTEST  6
#define CP_NEXTSEC  7
/* safe zone: g_va = centre now, g_vb = target centre */
#define Z_DMG       0
#define Z_INTERVAL  1
#define Z_FROMR     2
#define Z_TOR       3
#define Z_T0        4
#define Z_T1        5
#define Z_NEXTDMG   6
#define Z_STAGE     7
/* shrinker */
#define S_STAGE     0
#define S_PHASE     1
#define S_NEXT      2
#define S_DELAY     3
/* damage zone */
#define D_DMG       0
#define D_INTERVAL  1
#define D_NEXT      2
#define D_BITS      3
/* door */
#define DR_OPENAT   0
#define DR_CLOSEAT  1
#define DR_OPENSECS 2
#define DR_LOCKUNTIL 3
#define DR_LOCKEDT  4
#define DR_OPENEDAT 5
#define DR_WASOPEN  6
#define DR_DONE     7
/* trap */
#define T_DELAY     0
#define T_REARM     1
#define T_DMG       2
#define T_FIREAT    3
#define T_ARMAT     4
/* weapon spawner / loot / crate */
#define W_RESPAWN   0
#define W_BACKAT    1
#define W_TIER      2
#define W_OPEN      3
#define W_ITEMS     4
#define W_FALL      5
/* barricade */
#define B_MAX       0
#define B_REPAIR    1
#define B_COST      2
#define B_LASTSTEP  3
/* wave spawner */
#define WV_MIN      0
#define WV_MAX      1
#define WV_WAVES    2
#define WV_RATE     3
#define WV_NEXT     4
/* extraction */
#define X_HOLD      0
#define X_OPENAT    1
#define X_CLOSEAT   2
#define X_COST      3
#define X_DONE      4
/* platform */
#define PL_AUTO     0
#define PL_STARTED  1
/* meeting */
#define M_DUR       0
#define M_COOLDOWN  1
#define M_END       2
#define M_READY     3
#define M_MENUAT    4
/* drop marker */
#define DP_AT       0
#define DP_DONE     1

/* trap states */
enum { TRAP_ARMED = 0, TRAP_PENDING, TRAP_COOLDOWN, TRAP_OFF, TRAP_ACTIVE /* W1 */ };
/* crate states */
enum { CRATE_FALLING = 0, CRATE_FULL, CRATE_EMPTY };

/* ---- component slots ---- */
new g_ent[MAX_COMP];
new g_kind[MAX_COMP];
new g_src[MAX_COMP];
new Trie:g_keys[MAX_COMP];
new bool:g_on[MAX_COMP];
new bool:g_temp[MAX_COMP];
new bool:g_linked[MAX_COMP];
new bool:g_told[MAX_COMP];
new g_state[MAX_COMP];
new g_side[MAX_COMP];
new g_aux[MAX_COMP];
new g_team[MAX_COMP];
new Float:g_val[MAX_COMP];
new Float:g_pf[MAX_COMP][8];
new Float:g_org[MAX_COMP][3];
new Float:g_floor[MAX_COMP];
new Float:g_va[MAX_COMP][3];
new Float:g_vb[MAX_COMP][3];
new Float:g_r[MAX_COMP];
new Float:g_h[MAX_COMP];
new bool:g_box[MAX_COMP];
new Float:g_mins[MAX_COMP][3];
new Float:g_maxs[MAX_COMP][3];
new g_tname[MAX_COMP][32];
new g_label[MAX_COMP][32];
new g_item[MAX_COMP][32];
new g_doors[MAX_COMP][MAX_DOORS];
new Float:g_dorg[MAX_COMP][MAX_DOORS][3];
new Float:g_dang[MAX_COMP][MAX_DOORS][3];
new g_ndoors[MAX_COMP];
#define MAX_BOARDS 12
new g_boards[MAX_COMP][MAX_BOARDS];
new g_nboards[MAX_COMP];
new g_boardOf[MAX_ENTS];
new bool:g_seeThrough[MAX_COMP];
new g_nslots;

new g_slotOf[MAX_ENTS];
new g_doorOf[MAX_ENTS];
new g_pend[MAX_ENTS];
new g_parseEnt;
new bool:g_attractOn[MAX_COMP];
new g_nattract;
new Trie:g_pendKeys[MAX_ENTS];

/* ---- players ---- */
new bool:g_alive[33];
new bool:g_bot[33];
new g_pteam[33];
new Float:g_porg[33][3];
new g_inside[33][MAX_COMP / 32 + 1];
new g_act[33];
new g_actType[33];
new Float:g_actEnd[33];
new g_xslot[33];
new Float:g_xprog[33];
new Float:g_xrate[33];
new bool:g_extracted[33];
new Float:g_protect[33];
new g_class[33];
new g_cpLast[33];
new g_laps[33];
new Float:g_lapStart[33];
new g_vote[33];
new g_calls[33];
new bool:g_waveSpawn[33];
new Float:g_waveOrg[33][3];
new g_hud[33][192];
new g_hudShown[33][192];
new Float:g_hudAt[33];
new bool:g_hudWarn[33];
new g_beamLeft[33];
new Float:g_lockMsg[33];

/* ---- waves ---- */
new g_wave;
new bool:g_waveOn;
new bool:g_waveAuto;
new Float:g_waveNextAt;
new g_track[MAX_TRACK];
new g_ntrack;

/* ---- meeting ---- */
new g_meeting = -1;

/* ---- loot tables ---- */
new Trie:g_ltIndex;
new Array:g_ltItems[MAX_TABLES];
new g_ntables;

/* ---- misc ---- */
new Float:g_now;
new Float:g_roundStart;
new Float:g_nextSec;
new bool:g_ready;
new bool:g_quiet;
new bool:g_sawRound;
new bool:g_canPrecache;
new bool:g_brain;
new bool:g_lsBot;
new Trie:g_models;
new g_mapname[64];
new g_modeId[32];
/* Mode map kits: an ls_marker (kind, mode, targetname, size, radius, ...) becomes the component
   its kind names, or stays a named spot the mode's plugin finds (ls_marker_find) */
#define PEND_MARKER 99
new g_cWaveAlive;
new g_gateBlocks;
#define TOAST_Q 4
new g_tqText[33][TOAST_Q][128];
new g_tqRgb[33][TOAST_Q];
new Float:g_tqSecs[33][TOAST_Q];
new g_tqN[33];
new Float:g_tqUntil[33];
new g_tqShown[33][128];
new Trie:g_mkKeys[MAX_ENTS];
new g_nmarkers;
new g_sprBeam, g_sprSmoke, g_mdlGibs;
new g_msgBarTime, g_msgBarTime2, g_msgStatusIcon, g_msgFade, g_msgRoundTime, g_msgRadar;
new g_sync;

new g_cWaveAnnounce;
new g_cHud, g_cBeams, g_cBots, g_cFallback, g_cSpawns, g_cSpawnFar, g_cRadar, g_cRoundReset, g_cDebug;

/* forwards */
new g_fwComponent, g_fwCapture, g_fwContested, g_fwLost, g_fwCapTick, g_fwZoneStage, g_fwZoneTick,
	g_fwZoneCross, g_fwDamage, g_fwDoor, g_fwDoorLocked, g_fwTrap, g_fwWeaponTaken, g_fwWeaponBack,
	g_fwLoot, g_fwLootItem, g_fwDrop, g_fwBarDamaged, g_fwBarBroken, g_fwBarRepaired, g_fwWaveStart,
	g_fwWaveSpawn, g_fwWaveCleared, g_fwExtractOpen, g_fwExtracting, g_fwExtract, g_fwCheckpoint,
	g_fwLap, g_fwPlatform, g_fwClass, g_fwRespawn, g_fwMeetStart, g_fwMeetVote, g_fwMeetEnd;

new const DEFAULT_CRATE[] = "models/mil_crate.mdl";
new const WEAPON_MODELS[][] =
{
	"models/w_ak47.mdl", "models/w_aug.mdl", "models/w_awp.mdl", "models/w_deagle.mdl", "models/w_elite.mdl",
	"models/w_famas.mdl", "models/w_fiveseven.mdl", "models/w_flashbang.mdl", "models/w_g3sg1.mdl",
	"models/w_galil.mdl", "models/w_glock18.mdl", "models/w_hegrenade.mdl", "models/w_knife.mdl",
	"models/w_m249.mdl", "models/w_m3.mdl", "models/w_m4a1.mdl", "models/w_mac10.mdl", "models/w_mp5.mdl",
	"models/w_p228.mdl", "models/w_p90.mdl", "models/w_scout.mdl", "models/w_sg550.mdl", "models/w_sg552.mdl",
	"models/w_smokegrenade.mdl", "models/w_tmp.mdl", "models/w_ump45.mdl", "models/w_usp.mdl", "models/w_xm1014.mdl"
};
new const DEFAULT_BOX[] = "models/w_weaponbox.mdl";

/* =========================================================================
 * Start-up
 * ========================================================================= */

public plugin_precache()
{
	g_canPrecache = true;
	g_models = TrieCreate();
	g_ltIndex = TrieCreate();
	for (new s = 0; s < MAX_COMP; s++)
		g_keys[s] = Invalid_Trie;
	for (new e = 0; e < MAX_ENTS; e++)
	{
		g_pendKeys[e] = Invalid_Trie;
		g_mkKeys[e] = Invalid_Trie;
	}

	g_sprBeam = precache_model("sprites/laserbeam.spr");
	g_sprSmoke = precache_model("sprites/smokepuff.spr");
	g_mdlGibs = precache_model("models/woodgibs.mdl");
	up_precache(); /* W1 */
	precache_model_once(DEFAULT_CRATE);
	precache_model_once(DEFAULT_BOX);
	precache_model_once("models/w_kevlar.mdl");
	precache_model_once("models/w_assault.mdl");
	precache_model_once("models/w_thighpack.mdl");
	precache_model_once("models/w_backpack.mdl");
	precache_model_once("models/w_medkit.mdl");
	precache_model_once("models/w_9mmarclip.mdl");
	/* every weapon a spawner or loot table can show: the game's own precache comes later */
	for (new i = 0; i < sizeof WEAPON_MODELS; i++)
		precache_model_once(WEAPON_MODELS[i]);
	precache_sound("debris/bustcrate1.wav");
	precache_sound("items/gunpickup2.wav");
	precache_sound("items/ammopickup1.wav");
	precache_sound("items/smallmedkit1.wav");
	precache_sound("buttons/bell1.wav");
	precache_sound("buttons/blip1.wav");

	register_forward(FM_KeyValue, "fm_keyvalue");
	register_forward(FM_Spawn, "fm_spawn_post", 1);

	get_mapname(g_mapname, charsmax(g_mapname));
	read_mode_id();
	load_loot();
	load_packs();
	w3_precache(); /* W3 */
}

public plugin_natives()
{
	register_library("ls_components");
	register_native("ls_comp_create", "n_create");
	register_native("ls_comp_remove", "n_remove");
	register_native("ls_comp_is", "n_is");
	register_native("ls_comp_kind", "n_kind");
	register_native("ls_comp_find", "n_find");
	register_native("ls_comp_by_name", "n_by_name");
	register_native("ls_marker_find", "n_marker_find");
	register_native("ls_toast", "n_toast");
	register_native("ls_marker_get", "n_marker_get");
	register_native("ls_comp_count", "n_count");
	register_native("ls_comp_get", "n_get");
	register_native("ls_comp_getf", "n_getf");
	register_native("ls_comp_set", "n_set");
	register_native("ls_comp_enable", "n_enable");
	register_native("ls_comp_enabled", "n_enabled");
	register_native("ls_comp_start", "n_start");
	register_native("ls_comp_stop", "n_stop");
	register_native("ls_comp_reset", "n_reset");
	register_native("ls_comp_load_pack", "n_load_pack");
	register_native("ls_comp_inside", "n_inside");
	register_native("ls_comp_brain", "n_brain");
	register_native("ls_comp_bots_hint", "n_bots_hint");
	register_native("ls_give_item", "n_give_item");
	register_native("ls_cp_owner", "n_cp_owner");
	register_native("ls_cp_progress", "n_cp_progress");
	register_native("ls_cp_capturer", "n_cp_capturer");
	register_native("ls_cp_set_owner", "n_cp_set_owner");
	register_native("ls_cp_lock", "n_cp_lock");
	register_native("ls_zone_set", "n_zone_set");
	register_native("ls_zone_shrink", "n_zone_shrink");
	register_native("ls_zone_radius", "n_zone_radius");
	register_native("ls_zone_center", "n_zone_center");
	register_native("ls_zone_inside", "n_zone_inside");
	register_native("ls_zone_damage", "n_zone_damage");
	register_native("ls_zone_stage", "n_zone_stage");
	register_native("ls_zone_skip", "n_zone_skip");
	register_native("ls_zone_next", "n_zone_next");
	register_native("ls_comp_state", "n_comp_state");
	register_native("ls_door_open", "n_door_open");
	register_native("ls_door_close", "n_door_close");
	register_native("ls_door_lock", "n_door_lock");
	register_native("ls_door_is_open", "n_door_is_open");
	register_native("ls_trap_fire", "n_trap_fire");
	register_native("ls_trap_arm", "n_trap_arm");
	register_native("ls_spawner_set_item", "n_spawner_set_item");
	register_native("ls_spawner_respawn", "n_spawner_respawn");
	register_native("ls_loot_fill", "n_loot_fill");
	register_native("ls_loot_open", "n_loot_open");
	register_native("ls_loot_roll", "n_loot_roll");
	register_native("ls_drop_call", "n_drop_call");
	register_native("ls_barricade_hp", "n_barricade_hp");
	register_native("ls_barricade_set_hp", "n_barricade_set_hp");
	register_native("ls_barricade_repair", "n_barricade_repair");
	register_native("ls_wave_start", "n_wave_start");
	register_native("ls_wave_stop", "n_wave_stop");
	register_native("ls_wave_current", "n_wave_current");
	register_native("ls_wave_alive", "n_wave_alive");
	register_native("ls_wave_track", "n_wave_track");
	register_native("ls_extract_open", "n_extract_open");
	register_native("ls_extract_is_open", "n_extract_is_open");
	register_native("ls_extracted", "n_extracted");
	register_native("ls_checkpoint_of", "n_checkpoint_of");
	register_native("ls_checkpoint_laps", "n_checkpoint_laps");
	register_native("ls_checkpoint_reset", "n_checkpoint_reset");
	register_native("ls_platform_go", "n_platform_go");
	register_native("ls_platform_moving", "n_platform_moving");
	register_native("ls_class_set", "n_class_set");
	register_native("ls_class_get", "n_class_get");
	register_native("ls_class_menu", "n_class_menu");
	register_native("ls_class_find", "n_class_find");
	register_native("ls_meeting_call", "n_meeting_call");
	register_native("ls_meeting_end", "n_meeting_end");
	register_native("ls_meeting_active", "n_meeting_active");
	register_native("ls_meeting_vote", "n_meeting_vote");
	register_native("ls_respawn_pick", "n_respawn_pick");
	register_native("ls_respawn_at", "n_respawn_at");
	w3_natives(); /* W3 */
	up_natives(); /* W1 */
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, "Loner-Strike");
	register_cvar("ls_components_version", VERSION, FCVAR_SERVER | FCVAR_SPONLY);
	g_canPrecache = false;

	g_cHud = register_cvar("ls_comp_hud", "1");
	g_cBeams = register_cvar("ls_comp_beams", "40");
	g_cBots = register_cvar("ls_comp_bots", "1");
	g_cFallback = register_cvar("ls_comp_bot_fallback", "1");
	g_cSpawns = register_cvar("ls_comp_spawns", "1");
	g_cSpawnFar = register_cvar("ls_comp_spawn_far", "0");
	g_cRadar = register_cvar("ls_comp_radar", "0");
	g_cRoundReset = register_cvar("ls_comp_round_reset", "1");
	g_cDebug = register_cvar("ls_comp_debug", "0");
	g_cWaveAnnounce = register_cvar("ls_comp_wave_announce", "1");
	g_cWaveAlive = register_cvar("ls_comp_wave_alive_max", "0");

	g_msgBarTime = get_user_msgid("BarTime");
	g_msgBarTime2 = get_user_msgid("BarTime2");
	g_msgStatusIcon = get_user_msgid("StatusIcon");
	g_msgFade = get_user_msgid("ScreenFade");
	g_msgRoundTime = get_user_msgid("RoundTime");
	g_msgRadar = get_user_msgid("Radar");
	g_sync = CreateHudSyncObj();

	RegisterHamPlayer(Ham_Spawn, "ham_player_spawn", 1);
	RegisterHamPlayer(Ham_Killed, "ham_player_killed", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_player_hurt");
	RegisterHamPlayer(Ham_CS_Player_ResetMaxSpeed, "ham_maxspeed", 1);
	RegisterHam(Ham_Use, "info_target", "ham_use");
	RegisterHam(Ham_TakeDamage, "info_target", "ham_takedamage");
	RegisterHam(Ham_TakeDamage, "func_breakable", "ham_takedamage");
	RegisterHam(Ham_TakeDamage, "func_wall", "ham_takedamage");
	RegisterHam(Ham_Use, "func_door", "ham_door_use");
	RegisterHam(Ham_Use, "func_door_rotating", "ham_door_use");
	RegisterHam(Ham_Touch, "func_door", "ham_door_touch");
	RegisterHam(Ham_Touch, "func_door_rotating", "ham_door_touch");
	register_forward(FM_PlayerPreThink, "fm_prethink");
	register_forward(FM_TraceLine, "fm_traceline_post", 1);
	register_srvcmd("ls_comp_tracestats", "cmd_tracestats");

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");

	register_clcmd("say /class", "cmd_class_menu");
	register_clcmd("say /classes", "cmd_class_menu");
	register_clcmd("say_team /class", "cmd_class_menu");
	register_clcmd("say /vote", "cmd_vote_menu");

	register_concmd("ls_comp_list", "cmd_list", ADMIN_CVAR, "- lists the map's game mode components");
	register_concmd("ls_comp_spawn", "cmd_spawn", ADMIN_CVAR, "<classname> [key=value ...] - makes a component where you aim");
	register_concmd("ls_comp_del", "cmd_del", ADMIN_CVAR, "<ent> - removes a component");
	register_concmd("ls_comp_save", "cmd_save", ADMIN_CVAR, "- saves map-pack and runtime components to configs/components/<map>.ini");
	register_concmd("ls_comp_start", "cmd_start", ADMIN_CVAR, "<ent|0> - starts a component (0 = every manual one)");
	register_concmd("ls_comp_stop", "cmd_stop", ADMIN_CVAR, "<ent|0> - stops a component");
	register_concmd("ls_comp_reset", "cmd_reset", ADMIN_CVAR, "- puts every component back as at round start");

	g_fwComponent = CreateMultiForward("ls_on_component", ET_IGNORE, FP_CELL, FP_STRING);
	g_fwCapture = CreateMultiForward("ls_on_capture", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwContested = CreateMultiForward("ls_on_capture_contested", ET_IGNORE, FP_CELL);
	g_fwLost = CreateMultiForward("ls_on_capture_lost", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwCapTick = CreateMultiForward("ls_on_capture_tick", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwZoneStage = CreateMultiForward("ls_on_zone_stage", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwZoneTick = CreateMultiForward("ls_on_zone_tick", ET_STOP, FP_CELL, FP_CELL, FP_FLOAT);
	g_fwZoneCross = CreateMultiForward("ls_on_zone_cross", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwDamage = CreateMultiForward("ls_on_damage_zone", ET_STOP, FP_CELL, FP_CELL, FP_FLOAT);
	g_fwDoor = CreateMultiForward("ls_on_door", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwDoorLocked = CreateMultiForward("ls_on_door_locked", ET_IGNORE, FP_CELL, FP_CELL, FP_FLOAT);
	g_fwTrap = CreateMultiForward("ls_on_trap", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwWeaponTaken = CreateMultiForward("ls_on_weapon_taken", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	g_fwWeaponBack = CreateMultiForward("ls_on_weapon_respawn", ET_IGNORE, FP_CELL);
	g_fwLoot = CreateMultiForward("ls_on_loot", ET_STOP, FP_CELL, FP_CELL);
	g_fwLootItem = CreateMultiForward("ls_on_loot_item", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	g_fwDrop = CreateMultiForward("ls_on_drop", ET_IGNORE, FP_CELL, FP_CELL, FP_ARRAY);
	g_fwBarDamaged = CreateMultiForward("ls_on_barricade_damaged", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwBarBroken = CreateMultiForward("ls_on_barricade_broken", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwBarRepaired = CreateMultiForward("ls_on_barricade_repaired", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwWaveStart = CreateMultiForward("ls_on_wave_start", ET_IGNORE, FP_CELL);
	g_fwWaveSpawn = CreateMultiForward("ls_on_wave_spawn", ET_STOP, FP_CELL, FP_CELL, FP_ARRAY, FP_STRING);
	g_fwWaveCleared = CreateMultiForward("ls_on_wave_cleared", ET_IGNORE, FP_CELL);
	g_fwExtractOpen = CreateMultiForward("ls_on_extract_open", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwExtracting = CreateMultiForward("ls_on_extracting", ET_IGNORE, FP_CELL, FP_CELL, FP_FLOAT);
	g_fwExtract = CreateMultiForward("ls_on_extract", ET_STOP, FP_CELL, FP_CELL);
	g_fwCheckpoint = CreateMultiForward("ls_on_checkpoint", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwLap = CreateMultiForward("ls_on_lap", ET_IGNORE, FP_CELL, FP_CELL, FP_FLOAT);
	g_fwPlatform = CreateMultiForward("ls_on_platform", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwClass = CreateMultiForward("ls_on_class", ET_STOP, FP_CELL, FP_STRING);
	g_fwRespawn = CreateMultiForward("ls_on_respawn_point", ET_STOP, FP_CELL, FP_CELL);
	g_fwMeetStart = CreateMultiForward("ls_on_meeting_start", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwMeetVote = CreateMultiForward("ls_on_meeting_vote", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	g_fwMeetEnd = CreateMultiForward("ls_on_meeting_end", ET_IGNORE, FP_CELL, FP_CELL);
	w3_init(); /* W3 */
	up_plugin_init(); /* W1 */
}

public plugin_cfg()
{
	g_brain = get_cvar_pointer("ls_mode_ver") != 0;
	g_lsBot = get_cvar_pointer("bot_ls_fight") != 0;
	g_now = get_gametime();
	g_ready = true;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s])
			continue;
		comp_init(s);
		comp_link(s);
		comp_announce(s);
	}
	set_task(TICK, "tick", TASK_TICK, _, _, "b");
	/* a map with no rounds (or a mode that never sends Round_Start) still gets its timers */
	set_task(20.0, "task_round_fallback");
	log_amx("%d components on %s (bots %s)", count_comps(K_NONE), g_mapname,
		g_brain ? "understand ls_mode" : (g_lsBot ? "use ls_bot goto" : "get no hints"));
}

public task_round_fallback()
{
	if (g_roundStart <= 0.0 && !g_sawRound)
		round_begin();
}

public plugin_end()
{
	for (new s = 0; s < MAX_COMP; s++)
		if (g_keys[s] != Invalid_Trie)
			TrieDestroy(g_keys[s]);
	for (new e = 0; e < MAX_ENTS; e++)
		if (g_pendKeys[e] != Invalid_Trie)
			TrieDestroy(g_pendKeys[e]);
	for (new t = 0; t < g_ntables; t++)
		ArrayDestroy(g_ltItems[t]);
	TrieDestroy(g_ltIndex);
	TrieDestroy(g_models);
}

read_mode_id()
{
	new path[192];
	get_datadir(path, charsmax(path));
	format(path, charsmax(path), "%s/gamemodes/current.txt", path);
	g_modeId[0] = EOS;
	new f = fopen(path, "rt");
	if (f)
	{
		fgets(f, g_modeId, charsmax(g_modeId));
		trim(g_modeId);
		fclose(f);
	}
	/* Battle Royale's Duos and Squads use br's map packs, markers and loot W3 */
	if (equali(g_modeId, "br_duo") || equali(g_modeId, "br_squads")) /* W3 */
		copy(g_modeId, charsmax(g_modeId), "br"); /* W3 */
}

stock precache_model_once(const model[])
{
	new idx;
	if (TrieGetCell(g_models, model, idx))
		return idx;
	idx = precache_model(model);
	TrieSetCell(g_models, model, idx);
	return idx;
}

/* SetModel only with a model this plugin precached (else the fallback, which it did) */
stock set_model(ent, const model[], const fallback[])
{
	if (TrieKeyExists(g_models, model))
		engfunc(EngFunc_SetModel, ent, model);
	else
	{
		log_amx("Model %s isn't loaded; showing %s instead.", model, fallback);
		engfunc(EngFunc_SetModel, ent, fallback);
	}
}

/* a model a key asks for: precached while the map loads, otherwise only if it already was */
stock bool:usable_model(const model[])
{
	if (!model[0])
		return false;
	if (TrieKeyExists(g_models, model))
		return true;
	if (!g_canPrecache)
	{
		log_amx("Model %s wasn't loaded with the map, so a component made now can't use it.", model);
		return false;
	}
	if (!file_exists(model))
	{
		/* may still be in valve/ or a download folder; the engine finds those */
		new alt[160];
		formatex(alt, charsmax(alt), "../valve/%s", model);
		if (!file_exists(alt))
		{
			log_amx("Model %s not found; using the default.", model);
			return false;
		}
	}
	precache_model_once(model);
	return true;
}

/* =========================================================================
 * Custom classnames in maps (GAMEMODES-PLAN 2.2): ls_* becomes info_target
 * while the map loads, keys are kept, and the classname comes back at spawn.
 * ========================================================================= */

public fm_keyvalue(ent, kvd)
{
	if (ent <= 0 || ent >= MAX_ENTS)
		return FMRES_IGNORED;
	new key[32], val[512]; /* W1: long values (a shop's items) */
	get_kvd(kvd, KV_KeyName, key, charsmax(key));
	get_kvd(kvd, KV_Value, val, charsmax(val));
	if (equal(key, "classname"))
	{
		/* the engine sends classname twice: first on its own to pick the class (no class
		   yet), then again among the other keys (last of all in a compiled BSP). The first
		   starts this entity afresh; the repeat must keep the keys that came before it */
		/* (nothing may be read from the entity or kvd's class name on the first call: the
		   entity has no game data yet and the class name is NULL. Entities are parsed one at
		   a time, so "the entity being parsed" tells the repeat from a new one) */
		new bool:first = ent != g_parseEnt;
		g_parseEnt = ent;
		new kind = equal(val, "ls_marker") ? PEND_MARKER : kind_of(val);
		if (first)
		{
			g_pend[ent] = K_NONE;
			if (g_pendKeys[ent] != Invalid_Trie)
			{
				TrieDestroy(g_pendKeys[ent]);
				g_pendKeys[ent] = Invalid_Trie;
			}
		}
		if (kind && kind != K_CRATE)
		{
			set_kvd(kvd, KV_Value, "info_target");
			if (g_pend[ent] && g_pendKeys[ent] != Invalid_Trie)
				return FMRES_IGNORED;
			g_pend[ent] = kind;
			g_pendKeys[ent] = TrieCreate();
		}
		return FMRES_IGNORED;
	}
	if (g_pend[ent] && g_pendKeys[ent] != Invalid_Trie)
		TrieSetString(g_pendKeys[ent], key, val);
	return FMRES_IGNORED;
}

public fm_spawn_post(ent)
{
	if (ent == g_parseEnt)
		g_parseEnt = 0;
	if (ent <= 0 || ent >= MAX_ENTS || !g_pend[ent])
		return FMRES_IGNORED;
	new kind = g_pend[ent];
	new Trie:keys = g_pendKeys[ent];
	g_pend[ent] = K_NONE;
	g_pendKeys[ent] = Invalid_Trie;
	if (kind == PEND_MARKER)
	{
		marker_place(ent, keys, SRC_MAP);
		return FMRES_IGNORED;
	}
	if (comp_add(ent, kind, keys, SRC_MAP) < 0)
		TrieDestroy(keys);
	return FMRES_IGNORED;
}

stock kind_of(const classname[])
{
	if (!equal(classname, "ls_", 3))
		return K_NONE;
	for (new k = 1; k < K_COUNT; k++)
		if (equal(classname, KIND_NAME[k]))
			return k;
	return K_NONE;
}

/* =========================================================================
 * Slots and keys
 * ========================================================================= */

stock slot_of(ent)
{
	if (ent <= 0 || ent >= MAX_ENTS)
		return -1;
	return g_slotOf[ent] - 1;
}

stock comp_add(ent, kind, Trie:keys, src)
{
	new s = -1;
	for (new i = 0; i < MAX_COMP; i++)
	{
		if (!g_ent[i])
		{
			s = i;
			break;
		}
	}
	if (s < 0)
	{
		log_amx("Too many components (%d); %s at entity %d left out.", MAX_COMP, KIND_NAME[kind], ent);
		return -1;
	}
	if (s >= g_nslots)
		g_nslots = s + 1;
	g_ent[s] = ent;
	g_kind[s] = kind;
	g_src[s] = src;
	if (g_keys[s] != Invalid_Trie)
		TrieDestroy(g_keys[s]);
	g_keys[s] = keys;
	g_temp[s] = false;
	g_linked[s] = false;
	g_told[s] = false;
	g_slotOf[ent] = s + 1;
	set_pev(ent, pev_classname, KIND_NAME[kind]);
	for (new p = 1; p <= 32; p++)
		inside_set(p, s, false);
	if (g_ready)
	{
		comp_init(s);
		comp_link(s);
		comp_announce(s);
	}
	else
	{
		/* set up in plugin_cfg, once traces hit the world; a model key has to be loaded now */
		new m[64];
		if (ks(s, "model", m, charsmax(m)) && m[0] != '*')
			usable_model(m);
	}
	return s;
}

stock comp_free(s)
{
	if (!g_ent[s])
		return;
	hint_forget(s);
	for (new p = 1; p <= 32; p++)
	{
		if (g_act[p] == s + 1)
			act_cancel(p);
		if (g_xslot[p] == s + 1)
			extract_cancel(p);
		if (g_class[p] == s + 1)
			g_class[p] = 0;
	}
	for (new d = 0; d < g_ndoors[s]; d++)
		if (g_doors[s][d] > 0 && g_doors[s][d] < MAX_ENTS)
			g_doorOf[g_doors[s][d]] = 0;
	g_ndoors[s] = 0;
	if (g_meeting == s)
		g_meeting = -1;
	if (g_ent[s] < MAX_ENTS)
		g_slotOf[g_ent[s]] = 0;
	g_ent[s] = 0;
	g_kind[s] = K_NONE;
	if (g_keys[s] != Invalid_Trie)
	{
		TrieDestroy(g_keys[s]);
		g_keys[s] = Invalid_Trie;
	}
}

stock comp_announce(s)
{
	if (g_told[s] || !g_ent[s])
		return;
	g_told[s] = true;
	new ret;
	ExecuteForward(g_fwComponent, ret, g_ent[s], KIND_NAME[g_kind[s]]);
}

stock bool:ks(s, const key[], out[], len, const def[] = "")
{
	if (g_keys[s] != Invalid_Trie && TrieGetString(g_keys[s], key, out, len) && out[0])
		return true;
	copy(out, len, def);
	return false;
}

stock Float:kf(s, const key[], Float:def)
{
	new v[32];
	if (!ks(s, key, v, charsmax(v)))
		return def;
	return str_to_float(v);
}

stock ki(s, const key[], def)
{
	new v[32];
	if (!ks(s, key, v, charsmax(v)))
		return def;
	return str_to_num(v);
}

/* "t", "ct", "any", or a number: 0 anyone, 1 Terrorists, 2 Counter-Terrorists */
stock kteam(s, const key[], def)
{
	new v[16];
	if (!ks(s, key, v, charsmax(v)))
		return def;
	if (equali(v, "t") || equali(v, "terrorist") || equali(v, "terrorists"))
		return 1;
	if (equali(v, "ct") || equali(v, "counter-terrorists"))
		return 2;
	if (equali(v, "any") || equali(v, "all"))
		return 0;
	return clamp(str_to_num(v), 0, 2);
}

stock kvec(s, const key[], Float:out[3])
{
	new v[64], a[16], b[16], c[16];
	if (!ks(s, key, v, charsmax(v)))
		return 0;
	parse(v, a, charsmax(a), b, charsmax(b), c, charsmax(c));
	out[0] = str_to_float(a);
	out[1] = str_to_float(b);
	out[2] = str_to_float(c);
	return 1;
}

/* "radius=160 name=^"B^" items=3" into a trie */
stock parse_keys(Trie:keys, const text[])
{
	new len = strlen(text), i = 0, n = 0;
	new key[32], val[512]; /* W1: long values (a shop's items) */
	while (i < len)
	{
		while (i < len && text[i] <= ' ')
			i++;
		if (i >= len)
			break;
		new k = 0;
		while (i < len && text[i] != '=' && text[i] > ' ' && k < charsmax(key))
			key[k++] = text[i++];
		key[k] = EOS;
		if (i >= len || text[i] != '=')
		{
			while (i < len && text[i] > ' ')
				i++;
			continue;
		}
		i++;
		new v = 0;
		if (i < len && text[i] == '"')
		{
			i++;
			while (i < len && text[i] != '"' && v < charsmax(val))
				val[v++] = text[i++];
			if (i < len)
				i++;
		}
		else
		{
			while (i < len && text[i] > ' ' && v < charsmax(val))
				val[v++] = text[i++];
		}
		val[v] = EOS;
		if (key[0])
		{
			TrieSetString(keys, key, val);
			n++;
		}
	}
	return n;
}

stock vcopy(const Float:a[3], Float:b[3])
{
	b[0] = a[0];
	b[1] = a[1];
	b[2] = a[2];
}

stock Float:dist2d(const Float:a[3], const Float:b[3])
{
	new Float:dx = a[0] - b[0], Float:dy = a[1] - b[1];
	return floatsqroot(dx * dx + dy * dy);
}

stock bool:inside_get(id, s)
{
	return (g_inside[id][s >> 5] & (1 << (s & 31))) != 0;
}

stock inside_set(id, s, bool:on)
{
	if (on)
		g_inside[id][s >> 5] |= (1 << (s & 31));
	else
		g_inside[id][s >> 5] &= ~(1 << (s & 31));
}

stock Float:round_time()
{
	return g_roundStart > 0.0 ? g_now - g_roundStart : -1.0;
}

stock team_word(team, out[], len)
{
	copy(out, len, team == 1 ? "t" : (team == 2 ? "ct" : "all"));
}

stock count_comps(kind)
{
	new n;
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && (kind == K_NONE || g_kind[s] == kind))
			n++;
	return n;
}

/* =========================================================================
 * Setting a component up
 * ========================================================================= */

stock comp_init(s)
{
	new ent = g_ent[s];
	new Float:o[3];
	pev(ent, pev_origin, o);
	ks(s, "targetname", g_tname[s], charsmax(g_tname[]));
	if (g_tname[s][0])
		set_pev(ent, pev_targetname, g_tname[s]);
	ks(s, "name", g_label[s], charsmax(g_label[]), g_tname[s]);
	g_team[s] = kteam(s, "team", 0);

	/* the area: a brush, a size= box, or a radius */
	new model[64];
	ks(s, "model", model, charsmax(model));
	g_box[s] = false;
	if (model[0] == '*' && g_kind[s] != K_CRATE)
	{
		engfunc(EngFunc_SetModel, ent, model);
		pev(ent, pev_absmin, g_mins[s]);
		pev(ent, pev_absmax, g_maxs[s]);
		g_box[s] = true;
		for (new i = 0; i < 3; i++)
			o[i] = (g_mins[s][i] + g_maxs[s][i]) * 0.5;
		o[2] = g_mins[s][2] + 1.0;
	}
	else
	{
		new Float:size[3];
		if (kvec(s, "size", size) && size[0] > 0.0 && size[1] > 0.0 && size[2] > 0.0
			&& g_kind[s] != K_BARRICADE && g_kind[s] != K_LOOT)
		{
			g_box[s] = true;
			for (new i = 0; i < 2; i++)
			{
				g_mins[s][i] = o[i] - size[i] * 0.5;
				g_maxs[s][i] = o[i] + size[i] * 0.5;
			}
			g_mins[s][2] = o[2] - 4.0;
			g_maxs[s][2] = o[2] + size[2];
		}
	}
	vcopy(o, g_org[s]);
	new k = g_kind[s];
	/* logic-only components have no spot on the floor */
	g_floor[s] = (k == K_CLASS || k == K_SHRINKER || k == K_PLATFORM || k == K_DOOR) ? o[2] : floor_z(o, ent);
	g_r[s] = kf(s, "radius", default_radius(g_kind[s]));
	g_h[s] = kf(s, "height", 160.0);

	switch (g_kind[s])
	{
		case K_WEAPON:
		{
			set_pev(ent, pev_movetype, MOVETYPE_NOCLIP);
			set_pev(ent, pev_solid, SOLID_NOT);
			set_pev(ent, pev_avelocity, Float:{0.0, 60.0, 0.0});
		}
		case K_LOOT, K_CRATE:
		{
			new m[64];
			ks(s, "model", m, charsmax(m), g_kind[s] == K_CRATE ? DEFAULT_CRATE : DEFAULT_BOX);
			if (m[0] == '*' || !usable_model(m))
				copy(m, charsmax(m), g_kind[s] == K_CRATE ? DEFAULT_CRATE : DEFAULT_BOX);
			set_model(ent, m, g_kind[s] == K_CRATE ? DEFAULT_CRATE : DEFAULT_BOX);
			engfunc(EngFunc_SetSize, ent, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 24.0});
			set_pev(ent, pev_movetype, MOVETYPE_NONE);
			/* crates turn solid once nobody stands in them (solid_when_clear) */
			set_pev(ent, pev_solid, SOLID_NOT);
			g_aux[s] = g_kind[s] == K_LOOT ? 1 : 0;
			if (g_kind[s] == K_LOOT)
			{
				o[2] = g_floor[s] + 1.0;
				engfunc(EngFunc_SetOrigin, ent, o);
				vcopy(o, g_org[s]);
			}
		}
		case K_BARRICADE:
		{
			if (model[0] == '*')
			{
				set_pev(ent, pev_solid, SOLID_BSP);
				set_pev(ent, pev_movetype, MOVETYPE_PUSH);
			}
			else
			{
				/* with a model: that model; without one: an invisible box drawn as a beam fence */
				new m[64];
				if (ks(s, "model", m, charsmax(m)))
					set_model(ent, usable_model(m) ? m : DEFAULT_CRATE, DEFAULT_CRATE);
				new Float:size[3];
				if (!kvec(s, "size", size) || size[0] <= 0.0)
				{
					size[0] = 64.0;
					size[1] = 16.0;
					size[2] = 80.0;
				}
				/* fit=x / fit=y: span the corridor along that axis, wall to wall */
				new fit[4];
				ks(s, "fit", fit, charsmax(fit));
				if (fit[0] == 'x' || fit[0] == 'y')
				{
					new ax = fit[0] == 'x' ? 0 : 1;
					new Float:a[3], Float:b[3], Float:hit[3], Float:dd[2];
					vcopy(o, a);
					a[2] = g_floor[s] + 40.0;
					for (new sg = 0; sg < 2; sg++)
					{
						vcopy(a, b);
						b[ax] += sg ? 400.0 : -400.0;
						engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, ent, 0);
						get_tr2(0, TR_vecEndPos, hit);
						dd[sg] = floatabs(hit[ax] - a[ax]);
					}
					size[ax] = floatmax(dd[0] + dd[1] - 2.0, 32.0);
					o[ax] = a[ax] + (dd[1] - dd[0]) * 0.5;
					comp_debug("barricade %d fitted: %.0f wide", ent, size[ax]);
				}
				new Float:mn[3], Float:mx[3];
				mn[0] = -size[0] * 0.5; mn[1] = -size[1] * 0.5; mn[2] = 0.0;
				mx[0] = size[0] * 0.5; mx[1] = size[1] * 0.5; mx[2] = size[2];
				o[2] = g_floor[s] + 1.0;
				engfunc(EngFunc_SetOrigin, ent, o);
				vcopy(o, g_org[s]);
				engfunc(EngFunc_SetSize, ent, mn, mx);
				set_pev(ent, pev_solid, SOLID_BBOX);
				set_pev(ent, pev_movetype, MOVETYPE_NONE);
			}
			pev(ent, pev_absmin, g_mins[s]);
			pev(ent, pev_absmax, g_maxs[s]);
		}
		default:
		{
			/* areas and logic: nothing to see or touch */
			if (model[0] == '*')
			{
				set_pev(ent, pev_solid, SOLID_NOT);
				set_pev(ent, pev_effects, pev(ent, pev_effects) | EF_NODRAW);
			}
		}
	}
	up_comp_init(s); /* W1 */
	comp_reset(s);
}

stock Float:default_radius(kind)
{
	switch (kind)
	{
		case K_CAPTURE: return 128.0;
		case K_ZONE: return 3000.0;
		case K_DAMAGE: return 96.0;
		case K_TRAP: return 96.0;
		case K_EXTRACT: return 128.0;
		case K_CHECKPOINT: return 96.0;
		case K_MEETING: return 160.0;
		case K_WAVE: return 64.0;
	}
	return 64.0;
}

/* links that need the whole map: doors, trains, barricades over map brushes, shrinker -> zone */
/* g_aux[s] = 1 means "solid as soon as nobody overlaps it" (crates, barricades coming back) */
stock solid_when_clear(s, solid)
{
	if (!g_aux[s])
		return;
	new ent = g_ent[s];
	new Float:mn[3], Float:mx[3];
	pev(ent, pev_absmin, mn);
	pev(ent, pev_absmax, mx);
	/* a barricade coming back: a player standing in it is moved out (it doesn't wait for them:
	   a repairing bot leaning into the gate kept it open ~20 s into the next wave) */
	if (g_kind[s] == K_BARRICADE)
		push_out_players(mn, mx);
	if (box_occupied(mn, mx))
		return;
	/* ...and only a creature actually in the gap holds it open */
	if (g_kind[s] == K_BARRICADE && creature_near(mn, mx, 24.0))
		return;
	g_aux[s] = 0;
	set_pev(ent, pev_solid, solid);
	/* relink where it is (a brush's origin isn't its middle) */
	new Float:o[3];
	pev(ent, pev_origin, o);
	engfunc(EngFunc_SetOrigin, ent, o);
}

stock bool:creature_near(const Float:mn[3], const Float:mx[3], Float:r)
{
	new e = 0, Float:o[3];
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		if (pev(e, pev_deadflag) != DEAD_NO)
			continue;
		pev(e, pev_origin, o);
		new Float:d = 0.0;
		for (new i = 0; i < 3; i++)
		{
			new Float:c = floatclamp(o[i], mn[i], mx[i]);
			d += (o[i] - c) * (o[i] - c);
		}
		if (d < r * r)
			return true;
	}
	return false;
}

/* is any living player or creature overlapping this box (with a little room) */
/* players overlapping the box go out of its thin side (x or y), to whichever side their middle is
   on, if there's room there (else the other side) */
stock push_out_players(const Float:mn[3], const Float:mx[3])
{
	new Float:o[3], Float:to[3];
	new axis = (mx[0] - mn[0]) < (mx[1] - mn[1]) ? 0 : 1;
	new Float:mid = (mn[axis] + mx[axis]) * 0.5;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		pev(p, pev_origin, o);
		if (!(o[0] + 16.0 > mn[0] && o[0] - 16.0 < mx[0] && o[1] + 16.0 > mn[1] && o[1] - 16.0 < mx[1] && o[2] + 36.0 > mn[2] && o[2] - 36.0 < mx[2]))
			continue;
		for (new t = 0; t < 2; t++)
		{
			new bool:low = (o[axis] < mid) != (t == 1);
			to = o;
			to[axis] = low ? mn[axis] - 17.0 : mx[axis] + 17.0;
			new hull = (pev(p, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN;
			engfunc(EngFunc_TraceHull, to, to, DONT_IGNORE_MONSTERS, hull, p, 0);
			if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			{
				engfunc(EngFunc_SetOrigin, p, to);
				break;
			}
		}
	}
}

stock bool:box_occupied(const Float:mn[3], const Float:mx[3])
{
	new Float:o[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		pev(p, pev_origin, o);
		if (o[0] + 16.0 > mn[0] && o[0] - 16.0 < mx[0] && o[1] + 16.0 > mn[1] && o[1] - 16.0 < mx[1] && o[2] + 36.0 > mn[2] && o[2] - 36.0 < mx[2])
			return true;
	}
	new e = 0;
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		if (pev(e, pev_deadflag) != DEAD_NO)
			continue;
		new Float:cmn[3], Float:cmx[3];
		pev(e, pev_absmin, cmn);
		pev(e, pev_absmax, cmx);
		if (cmx[0] > mn[0] && cmn[0] < mx[0] && cmx[1] > mn[1] && cmn[1] < mx[1] && cmx[2] > mn[2] && cmn[2] < mx[2])
			return true;
	}
	return false;
}

stock comp_link(s)
{
	if (g_linked[s])
		return;
	g_linked[s] = true;
	new name[32];
	switch (g_kind[s])
	{
		case K_DOOR:
		{
			ks(s, "door", name, charsmax(name), g_tname[s]);
			g_ndoors[s] = 0;
			new e = 0;
			/* by name, or by brush model (*12) for maps whose doors have no name */
			new field[12];
			copy(field, charsmax(field), name[0] == '*' ? "model" : "targetname");
			while (name[0] && g_ndoors[s] < MAX_DOORS && (e = engfunc(EngFunc_FindEntityByString, e, field, name)) > 0)
			{
				new cls[32];
				pev(e, pev_classname, cls, charsmax(cls));
				if (!equal(cls, "func_door") && !equal(cls, "func_door_rotating"))
					continue;
				new d = g_ndoors[s]++;
				g_doors[s][d] = e;
				pev(e, pev_origin, g_dorg[s][d]);
				pev(e, pev_angles, g_dang[s][d]);
				/* our timed doors stay where they're put: the game's "toggle" door flag */
				set_pev(e, pev_spawnflags, pev(e, pev_spawnflags) | 32);
				if (e < MAX_ENTS)
					g_doorOf[e] = s + 1;
			}
			if (!g_ndoors[s])
				log_amx("%s at %.0f %.0f %.0f: no func_door named ^"%s^".", KIND_NAME[K_DOOR], g_org[s][0], g_org[s][1], g_org[s][2], name);
		}
		case K_PLATFORM:
		{
			ks(s, "platform", name, charsmax(name));
			g_aux[s] = name[0] ? engfunc(EngFunc_FindEntityByString, 0, name[0] == '*' ? "model" : "targetname", name) : 0;
			if (g_aux[s] <= 0)
			{
				g_aux[s] = 0;
				log_amx("%s: no train or platform named ^"%s^".", KIND_NAME[K_PLATFORM], name);
			}
		}
		case K_SHRINKER:
		{
			ks(s, "zone", name, charsmax(name));
			g_aux[s] = -1;
			for (new z = 0; z < g_nslots; z++)
			{
				if (g_ent[z] && g_kind[z] == K_ZONE && (!name[0] || equal(g_tname[z], name)))
				{
					g_aux[s] = z;
					break;
				}
			}
		}
		case K_BARRICADE:
		{
			ks(s, "adopt", name, charsmax(name));
			if (name[0])
				barricade_adopt(s, name);
			/* boards=<targetname>: separate board brushes, hidden one by one as it weakens */
			g_nboards[s] = 0;
			ks(s, "boards", name, charsmax(name));
			new e = 0;
			while (name[0] && g_nboards[s] < MAX_BOARDS && (e = engfunc(EngFunc_FindEntityByString, e, "targetname", name)) > 0)
			{
				g_boards[s][g_nboards[s]++] = e;
				if (e < MAX_ENTS)
					g_boardOf[e] = s + 1;
			}
			barricade_tint(s);
		}
		case K_CHECKPOINT:
		{
			/* nothing: orders are read when players reach them */
		}
	}
}

stock barricade_adopt(s, const name[])
{
	new e;
	if (name[0] == '*')
		e = engfunc(EngFunc_FindEntityByString, 0, "model", name);
	else
		e = engfunc(EngFunc_FindEntityByString, 0, "targetname", name);
	if (e <= 0 || e >= MAX_ENTS || g_slotOf[e])
	{
		log_amx("%s: nothing to take over called ^"%s^".", KIND_NAME[K_BARRICADE], name);
		return;
	}
	new old = g_ent[s];
	g_slotOf[old] = 0;
	g_ent[s] = e;
	g_slotOf[e] = s + 1;
	pev(e, pev_absmin, g_mins[s]);
	pev(e, pev_absmax, g_maxs[s]);
	for (new i = 0; i < 3; i++)
		g_org[s][i] = (g_mins[s][i] + g_maxs[s][i]) * 0.5;
	g_floor[s] = g_mins[s][2];
	if (g_src[s] != SRC_MAP || old != e)
		engfunc(EngFunc_RemoveEntity, old);
	g_src[s] = SRC_PACK;
	comp_reset(s);
}

/* =========================================================================
 * Round flow
 * ========================================================================= */

public ev_new_round()
{
	g_now = get_gametime();
	g_roundStart = 0.0;
	meeting_finish(false);
	g_wave = 0;
	g_waveOn = false;
	g_waveAuto = false;
	g_ntrack = 0;
	for (new p = 1; p <= 32; p++)
	{
		act_cancel(p);
		extract_cancel(p);
		g_extracted[p] = false;
		g_cpLast[p] = 0;
		g_laps[p] = 0;
		g_calls[p] = 0;
		g_vote[p] = -1;
		for (new i = 0; i < sizeof g_inside[]; i++)
			g_inside[p][i] = 0;
	}
	g_sawRound = true;
	if (get_pcvar_num(g_cRoundReset))
	{
		/* the bots hear it all at Round_Start */
		g_quiet = true;
		for (new s = 0; s < g_nslots; s++)
		{
			if (!g_ent[s])
				continue;
			if (g_temp[s])
			{
				comp_remove(s);
				continue;
			}
			comp_reset(s);
		}
		g_quiet = false;
	}
}

public ev_round_start()
{
	round_begin();
}

stock round_begin()
{
	g_now = get_gametime();
	g_roundStart = g_now;
	g_brain = get_cvar_pointer("ls_mode_ver") != 0;
	hint("reset");
	for (new k = 0; k < MAX_COMP; k++)
		g_attractOn[k] = false;
	g_nattract = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s])
			continue;
		switch (g_kind[s])
		{
			case K_SHRINKER:
			{
				new Float:delay = kf(s, "start_delay", 30.0);
				if (delay >= 0.0)
				{
					g_state[s] = 1;
					g_pf[s][S_PHASE] = 0.0;
					g_pf[s][S_NEXT] = g_now + delay;
				}
			}
			case K_WAVE:
			{
				if (ki(s, "auto", 0))
				{
					g_waveAuto = true;
					g_waveNextAt = g_now + kf(s, "prepare", 30.0);
				}
			}
			case K_PLATFORM:
			{
				if (g_pf[s][PL_AUTO] >= 0.0)
					g_pf[s][PL_STARTED] = 0.0;
			}
			case K_CHECKPOINT:
			{
				if (ki(s, "order", 1) == 1)
					for (new b = 1; b <= MaxClients; b++)
						if (is_user_connected(b) && is_user_bot(b) && is_user_alive(b))
							hint("goal %d go_hold %.0f %.0f %.0f %d 60", b, g_org[s][0], g_org[s][1], g_org[s][2], g_ent[s]);
			}
		}
		hint_comp(s);
	}
}

/* back to how the round started */
stock comp_reset(s)
{
	new ent = g_ent[s];
	g_on[s] = ki(s, "enabled", 1) != 0;
	g_state[s] = 0;
	g_val[s] = 0.0;
	for (new i = 0; i < 8; i++)
		g_pf[s][i] = 0.0;
	switch (g_kind[s])
	{
		case K_CAPTURE:
		{
			g_side[s] = ki(s, "start_owner", 0);
			g_aux[s] = 0;
			g_pf[s][CP_SECS] = floatmax(kf(s, "capture_secs", 10.0), 0.1);
			g_pf[s][CP_DECAY] = kf(s, "decay", 1.0);
			g_pf[s][CP_MULT] = float(ki(s, "multiplier", 1));
			g_pf[s][CP_FFA] = float(ki(s, "ffa", 0));
			g_pf[s][CP_TEAMLOCK] = float(kteam(s, "team_lock", 0));
		}
		case K_ZONE:
		{
			vcopy(g_org[s], g_va[s]);
			vcopy(g_org[s], g_vb[s]);
			g_val[s] = g_r[s];
			g_pf[s][Z_FROMR] = g_r[s];
			g_pf[s][Z_TOR] = g_r[s];
			g_pf[s][Z_DMG] = kf(s, "damage", 1.0);
			g_pf[s][Z_INTERVAL] = floatmax(kf(s, "interval", 1.0), 0.25);
		}
		case K_SHRINKER:
		{
			g_pf[s][S_STAGE] = 0.0;
		}
		case K_DAMAGE:
		{
			g_pf[s][D_DMG] = kf(s, "dmg", 10.0);
			g_pf[s][D_INTERVAL] = floatmax(kf(s, "interval", 1.0), 0.25);
			new kind[16];
			ks(s, "kind", kind, charsmax(kind), "generic");
			new bits = DMG_GENERIC;
			if (equali(kind, "fire"))
				bits = DMG_BURN;
			else if (equali(kind, "poison"))
				bits = DMG_POISON;
			else if (equali(kind, "shock"))
				bits = DMG_SHOCK;
			else if (equali(kind, "radiation"))
				bits = DMG_RADIATION;
			g_pf[s][D_BITS] = float(bits);
		}
		case K_DOOR:
		{
			g_pf[s][DR_OPENAT] = kf(s, "open_at", -1.0);
			g_pf[s][DR_CLOSEAT] = kf(s, "close_at", -1.0);
			g_pf[s][DR_OPENSECS] = kf(s, "open_secs", 0.0);
			g_pf[s][DR_LOCKUNTIL] = kf(s, "locked_until", 0.0);
			g_pf[s][DR_LOCKEDT] = 0.0;
		}
		case K_TRAP:
		{
			g_state[s] = ki(s, "armed", 1) ? TRAP_ARMED : TRAP_OFF;
			g_pf[s][T_DELAY] = kf(s, "delay", 0.0);
			g_pf[s][T_REARM] = kf(s, "rearm_secs", 5.0);
			g_pf[s][T_DMG] = kf(s, "dmg", 0.0);
		}
		case K_WEAPON:
		{
			g_pf[s][W_RESPAWN] = kf(s, "respawn_secs", 30.0);
			g_pf[s][W_TIER] = float(ki(s, "tier", 1));
			spawner_roll(s);
			spawner_show(s, true);
		}
		case K_LOOT:
		{
			g_pf[s][W_RESPAWN] = kf(s, "respawn_secs", 0.0);
			g_pf[s][W_TIER] = float(ki(s, "tier", 1));
			g_pf[s][W_OPEN] = kf(s, "open_secs", 2.0);
			g_pf[s][W_ITEMS] = float(ki(s, "items", 2));
			ks(s, "table", g_item[s], charsmax(g_item[]), "default");
			loot_look(s, true);
		}
		case K_CRATE:
		{
			g_pf[s][W_OPEN] = kf(s, "open_secs", 3.0);
			g_pf[s][W_ITEMS] = float(ki(s, "items", 4));
			g_pf[s][W_TIER] = float(ki(s, "tier", 5));
			ks(s, "table", g_item[s], charsmax(g_item[]), "drop");
		}
		case K_DROP:
		{
			g_pf[s][DP_AT] = kf(s, "at", -1.0);
		}
		case K_BARRICADE:
		{
			g_pf[s][B_MAX] = floatmax(kf(s, "hp", 500.0), 1.0);
			g_pf[s][B_REPAIR] = floatmax(kf(s, "repair_secs", 1.5), 0.5);
			g_pf[s][B_COST] = kf(s, "repair_cost", 0.0);
			g_val[s] = g_pf[s][B_MAX];
			g_pf[s][B_LASTSTEP] = 4.0;
			barricade_stand(s, true);
		}
		case K_WAVE:
		{
			g_pf[s][WV_MIN] = float(ki(s, "wave_min", 4));
			g_pf[s][WV_MAX] = float(ki(s, "wave_max", 12));
			g_pf[s][WV_WAVES] = float(max(ki(s, "waves", 10), 1));
			g_pf[s][WV_RATE] = floatmax(kf(s, "rate", 1.5), 0.2);
			g_team[s] = kteam(s, "team", 1);
			g_aux[s] = 0;
		}
		case K_EXTRACT:
		{
			g_pf[s][X_HOLD] = floatmax(kf(s, "hold_secs", 8.0), 0.0);
			g_pf[s][X_OPENAT] = kf(s, "open_at", 0.0);
			g_pf[s][X_CLOSEAT] = kf(s, "close_at", -1.0);
			g_pf[s][X_COST] = kf(s, "cost", 0.0);
		}
		case K_PLATFORM:
		{
			g_pf[s][PL_AUTO] = kf(s, "auto", -1.0);
			g_pf[s][PL_STARTED] = 1.0;
		}
		case K_MEETING:
		{
			g_pf[s][M_DUR] = floatmax(kf(s, "duration", 45.0), 5.0);
			g_pf[s][M_COOLDOWN] = kf(s, "cooldown", 60.0);
		}
		case K_RESPAWN:
		{
			g_team[s] = kteam(s, "team", 0);
		}
	}
	up_comp_reset(s); /* W1 */
	if (ent && g_ready)
		hint_comp(s);
}

/* removes a component and its entity */
stock comp_remove(s)
{
	new ent = g_ent[s];
	new src = g_src[s];
	if (g_kind[s] == K_BARRICADE)
		barricade_stand(s, true);
	comp_free(s);
	if (ent > 0 && pev_valid(ent) && (src != SRC_PACK || kind_is_own_entity(ent)))
		engfunc(EngFunc_RemoveEntity, ent);
}

stock bool:kind_is_own_entity(ent)
{
	new cls[32];
	pev(ent, pev_classname, cls, charsmax(cls));
	return equal(cls, "ls_", 3) != 0;
}

/* =========================================================================
 * Map packs
 * ========================================================================= */

stock load_packs()
{
	new dir[160], path[224];
	get_configsdir(dir, charsmax(dir));
	new const subs[][] = { "gamemodes/components", "components" };
	for (new i = 0; i < sizeof subs; i++)
	{
		formatex(path, charsmax(path), "%s/%s/%s.ini", dir, subs[i], g_mapname);
		load_pack(path);
		if (g_modeId[0])
		{
			formatex(path, charsmax(path), "%s/%s/%s.%s.ini", dir, subs[i], g_mapname, g_modeId);
			load_pack(path);
		}
	}
}

stock load_pack(const path[])
{
	new f = fopen(path, "rt");
	if (!f)
		return 0;
	new line[512], cls[32], xs[16], ys[16], zs[16], n = 0;
	while (fgets(f, line, charsmax(line)))
	{
		trim(line);
		if (!line[0] || line[0] == ';' || line[0] == '#' || (line[0] == '/' && line[1] == '/'))
			continue;
		new rest[448];
		/* classname x y z, then keys */
		new pos = 0;
		pos += next_token(line[pos], cls, charsmax(cls));
		pos += next_token(line[pos], xs, charsmax(xs));
		pos += next_token(line[pos], ys, charsmax(ys));
		pos += next_token(line[pos], zs, charsmax(zs));
		copy(rest, charsmax(rest), line[pos]);
		if (equal(cls, "ls_marker") && zs[0])
		{
			new Float:mo[3];
			mo[0] = str_to_float(xs);
			mo[1] = str_to_float(ys);
			mo[2] = str_to_float(zs);
			if (marker_make(mo, rest, SRC_PACK))
				n++;
			continue;
		}
		new kind = kind_of(cls);
		if (!kind || kind == K_CRATE || !zs[0])
		{
			log_amx("%s: can't read ^"%s^"", path, line);
			continue;
		}
		new Float:o[3];
		o[0] = str_to_float(xs);
		o[1] = str_to_float(ys);
		o[2] = str_to_float(zs);
		if (comp_make(kind, o, rest, SRC_PACK))
			n++;
	}
	fclose(f);
	if (n)
		log_amx("Loaded %d components from %s", n, path);
	return n;
}

stock next_token(const text[], out[], len)
{
	new i = 0, o = 0;
	while (text[i] && text[i] <= ' ')
		i++;
	while (text[i] && text[i] > ' ' && o < len)
		out[o++] = text[i++];
	out[o] = EOS;
	return i;
}

/* ---- Mode map kits: ls_marker ------------------------------------------------------------
   kind      what it marks: a component classname (ls_wave_spawner, ls_barricade, ls_safe_zone...)
             makes that component with the marker's other keys; lst_base makes the Last Stand base
             box (an ls_damage_zone that does nothing); anything else (lst_shop, br_drop, ...) is a
             named spot: targetname = its kind unless it has one, found with ls_marker_find
   mode      the mode(s) it's for ("laststand", "br", "laststand,br"; empty or "any": every mode).
             A marker for another mode is left out. */
stock bool:marker_for_mode(Trie:keys)
{
	new m[96];
	if (!TrieGetString(keys, "mode", m, charsmax(m)) || !m[0] || equali(m, "any"))
		return true;
	if (!g_modeId[0])
		return false;
	new tok[32];
	while (m[0])
	{
		strtok2(m, tok, charsmax(tok), m, charsmax(m), ',', TRIM_FULL);
		if (equali(tok, g_modeId))
			return true;
	}
	return false;
}

/* what a marker becomes: a component kind, or K_NONE (a named spot); keys filled in for aliases */
stock marker_kind(Trie:keys)
{
	new k[32];
	TrieGetString(keys, "kind", k, charsmax(k));
	if (equal(k, "lst_base"))
	{
		if (!TrieKeyExists(keys, "targetname"))
			TrieSetString(keys, "targetname", "lst_base");
		TrieSetString(keys, "dmg", "0");
		TrieSetString(keys, "show", "0");
		TrieSetString(keys, "enabled", "0");
		if (!TrieKeyExists(keys, "name"))
			TrieSetString(keys, "name", "Base");
		return K_DAMAGE;
	}
	new up = up_marker_kind(k, keys); /* W1 */
	if (up)
		return up;
	new kind = kind_of(k);
	return kind == K_CRATE ? K_NONE : kind;
}

/* a marker entity (an info_target already spawned) takes its place; keys are kept or freed */
stock marker_place(ent, Trie:keys, src)
{
	if (!marker_for_mode(keys))
	{
		TrieDestroy(keys);
		set_pev(ent, pev_flags, pev(ent, pev_flags) | FL_KILLME);
		return 0;
	}
	new kind = marker_kind(keys);
	if (kind)
	{
		if (comp_add(ent, kind, keys, src) < 0)
		{
			TrieDestroy(keys);
			return 0;
		}
		return ent;
	}
	new k[32], tn[32];
	TrieGetString(keys, "kind", k, charsmax(k));
	if (!TrieGetString(keys, "targetname", tn, charsmax(tn)) || !tn[0])
		copy(tn, charsmax(tn), k);
	set_pev(ent, pev_classname, "ls_marker");
	set_pev(ent, pev_targetname, tn);
	set_pev(ent, pev_netname, k);
	if (g_mkKeys[ent] != Invalid_Trie)
		TrieDestroy(g_mkKeys[ent]);
	g_mkKeys[ent] = keys;
	g_nmarkers++;
	return ent;
}

stock marker_make(const Float:origin[3], const keys[], src)
{
	new ent = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"));
	if (!ent || ent >= MAX_ENTS)
	{
		if (ent)
			engfunc(EngFunc_RemoveEntity, ent);
		return 0;
	}
	new Trie:t = TrieCreate();
	parse_keys(t, keys);
	engfunc(EngFunc_SetOrigin, ent, origin);
	dllfunc(DLLFunc_Spawn, ent);
	if (!marker_place(ent, t, src))
	{
		if (pev_valid(ent))
			engfunc(EngFunc_RemoveEntity, ent);
		return 0;
	}
	return ent;
}

/* native ls_marker_find(const kind[], start = 0): the next named-spot marker of that kind */
public n_marker_find(plugin, params)
{
	new kind[32], k[32];
	get_string(1, kind, charsmax(kind));
	for (new e = max(get_param(2), 0) + 1; e < MAX_ENTS; e++)
	{
		if (g_mkKeys[e] == Invalid_Trie || !pev_valid(e))
			continue;
		pev(e, pev_netname, k, charsmax(k));
		if (!kind[0] || equal(k, kind))
			return e;
	}
	return 0;
}

/* native ls_marker_get(ent, const key[], out[], len): a marker's key; false if it has none */
public n_marker_get(plugin, params)
{
	new ent = get_param(1);
	if (ent <= 0 || ent >= MAX_ENTS || g_mkKeys[ent] == Invalid_Trie)
		return 0;
	new key[32], v[128];
	get_string(2, key, charsmax(key));
	if (!TrieGetString(g_mkKeys[ent], key, v, charsmax(v)))
		return 0;
	set_string(3, v, get_param(4));
	return 1;
}

stock comp_make(kind, const Float:origin[3], const keys[], src)
{
	new ent = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"));
	if (!ent || ent >= MAX_ENTS)
	{
		if (ent)
			engfunc(EngFunc_RemoveEntity, ent);
		return 0;
	}
	new Trie:t = TrieCreate();
	parse_keys(t, keys);
	new Float:ang[3];
	new v[48];
	if (TrieGetString(t, "angle", v, charsmax(v)))
		ang[1] = str_to_float(v);
	else if (TrieGetString(t, "angles", v, charsmax(v)))
	{
		new a[16], b[16], c[16];
		parse(v, a, charsmax(a), b, charsmax(b), c, charsmax(c));
		ang[0] = str_to_float(a);
		ang[1] = str_to_float(b);
		ang[2] = str_to_float(c);
	}
	set_pev(ent, pev_angles, ang);
	engfunc(EngFunc_SetOrigin, ent, origin);
	dllfunc(DLLFunc_Spawn, ent);
	if (comp_add(ent, kind, t, src) < 0)
	{
		TrieDestroy(t);
		engfunc(EngFunc_RemoveEntity, ent);
		return 0;
	}
	return ent;
}

/* =========================================================================
 * The tick: logic 4 times a second, visuals once a second
 * ========================================================================= */

public tick()
{
	g_now = get_gametime();
	new bool:sec = g_now >= g_nextSec;
	if (sec)
		g_nextSec = g_now + 1.0;
	new budget = get_pcvar_num(g_cBeams);
	for (new p = 1; p <= MaxClients; p++)
	{
		g_alive[p] = is_user_connected(p) && is_user_alive(p);
		g_bot[p] = is_user_connected(p) && is_user_bot(p);
		g_hud[p][0] = EOS;
		g_hudWarn[p] = false;
		if (sec)
			g_beamLeft[p] = budget;
		if (!g_alive[p])
			continue;
		pev(p, pev_origin, g_porg[p]);
		g_pteam[p] = get_user_team(p);
		if (g_protect[p] > 0.0 && g_now >= g_protect[p])
		{
			g_protect[p] = 0.0;
			set_pev(p, pev_takedamage, DAMAGE_AIM);
			render(p, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
		}
	}

	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s])
			continue;
		if (!pev_valid(g_ent[s]))
		{
			comp_free(s);
			continue;
		}
		if (!g_on[s])
			continue;
		switch (g_kind[s])
		{
			case K_CAPTURE: tick_capture(s, sec);
			case K_ZONE: tick_zone(s, sec);
			case K_SHRINKER: tick_shrinker(s);
			case K_DAMAGE: tick_damage(s);
			case K_DOOR: tick_door(s);
			case K_TRAP: tick_trap(s);
			case K_WEAPON: tick_spawner(s);
			case K_LOOT: tick_loot(s);
			case K_CRATE: tick_crate(s, sec);
			case K_DROP: tick_drop_marker(s);
			case K_BARRICADE:
			{
				if (g_state[s] == 0)
				{
					new m[4];
					pev(g_ent[s], pev_model, m, charsmax(m));
					solid_when_clear(s, m[0] == '*' ? SOLID_BSP : SOLID_BBOX);
				}
			}
			case K_EXTRACT: tick_extract(s);
			case K_CHECKPOINT: tick_checkpoint(s);
			case K_PLATFORM: tick_platform(s);
			case K_MEETING: tick_meeting(s);
		}
	}
	tick_waves();
	tick_acts();
	use_hints();
	up_tick(sec); /* W1 */
	if (sec)
	{
		visuals();
		lap_hud();
	}
	w3_tick(sec); /* W3 */
	hud_flush();
	for (new p = 1; p <= MaxClients; p++)
		if (g_tqN[p])
			toast_tick(p);
}

/* ---- capture point ---- */

stock side_of(p, s)
{
	return g_pf[s][CP_FFA] > 0.0 ? p : g_pteam[p];
}

stock tick_capture(s, bool:sec)
{
	new count[33], sides = 0, side = 0, first = 0, n = 0;
	new bool:ffa = g_pf[s][CP_FFA] > 0.0;
	for (new p = 1; p <= MaxClients; p++)
	{
		new bool:in = g_alive[p] && (ffa || g_pteam[p] == 1 || g_pteam[p] == 2) && inside(s, g_porg[p]);
		inside_set(p, s, in);
		if (!in)
			continue;
		new sd = side_of(p, s);
		if (!count[sd])
			sides++;
		count[sd]++;
		side = sd;
		if (!first)
			first = p;
		n++;
	}
	new owner = g_side[s];
	new bool:locked = g_pf[s][CP_LOCK] > g_now;
	new tl = floatround(g_pf[s][CP_TEAMLOCK]);
	if (sides > 1)
	{
		if (g_aux[s] != -1)
		{
			g_aux[s] = -1;
			fwd1(g_fwContested, g_ent[s]);
			bar_all_inside(s, 0);
		}
	}
	else if (sides == 1 && side != owner && !locked && (!tl || ffa || side == tl))
	{
		if (g_aux[s] != side)
		{
			/* a new side starts from nothing */
			if (g_aux[s] > 0 || g_aux[s] == -1)
				g_val[s] = 0.0;
			g_aux[s] = side;
		}
		new Float:mult = 1.0;
		if (g_pf[s][CP_MULT] > 0.0)
			mult = floatmin(float(count[side]), 3.0);
		new Float:before = g_val[s];
		g_val[s] += TICK * mult / g_pf[s][CP_SECS];
		if (before == 0.0 || sec)
			bar_all_inside(s, floatround((g_pf[s][CP_SECS] * (1.0 - g_val[s])) / mult + 0.5));
		if (g_val[s] >= 1.0)
		{
			g_val[s] = 0.0;
			g_aux[s] = 0;
			bar_all_inside(s, 0);
			capture_set(s, side, first);
		}
	}
	else
	{
		if (sides == 0 || side == owner)
		{
			if (g_pf[s][CP_DECAY] > 0.0 && g_val[s] > 0.0)
			{
				g_val[s] = floatmax(g_val[s] - TICK * g_pf[s][CP_DECAY] / g_pf[s][CP_SECS], 0.0);
				if (g_val[s] <= 0.0)
					g_aux[s] = 0;
			}
			if (g_aux[s] == -1)
				g_aux[s] = 0;
		}
	}

	/* HUD for those standing on it */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || !inside_get(p, s) || g_bot[p])
			continue;
		new name[40];
		if (g_label[s][0])
			formatex(name, charsmax(name), " %s", g_label[s]);
		if (g_aux[s] == -1)
			hud_add(p, true, "Point%s contested!", name);
		else if (locked && owner != side_of(p, s))
			hud_add(p, false, "Point%s is locked for %d s", name, floatround(g_pf[s][CP_LOCK] - g_now, floatround_ceil));
		else if (g_aux[s] > 0 && g_aux[s] == side_of(p, s))
			hud_add(p, false, "Taking point%s: %d%%", name, floatround(g_val[s] * 100.0));
		else if (owner && owner == side_of(p, s))
			hud_add(p, false, "Holding point%s", name);
		else if (tl && !ffa && g_pteam[p] != tl)
			hud_add(p, false, "Your team can't take point%s", name);
	}

	if (sec && owner)
	{
		fwd3(g_fwCapTick, g_ent[s], owner, 0);
		for (new p = 1; p <= MaxClients; p++)
			if (g_alive[p] && inside_get(p, s) && side_of(p, s) == owner)
				fwd3(g_fwCapTick, g_ent[s], owner, p);
	}
}

stock capture_set(s, side, who)
{
	new old = g_side[s];
	if (old == side)
		return;
	g_side[s] = side;
	if (old)
		fwd2(g_fwLost, g_ent[s], old);
	if (side)
	{
		fwd3(g_fwCapture, g_ent[s], side, who);
		fire_targets(s, who);
		new msg[96];
		if (g_pf[s][CP_FFA] > 0.0)
		{
			new name[32];
			if (1 <= side <= MaxClients && is_user_connected(side))
				get_user_name(side, name, charsmax(name));
			formatex(msg, charsmax(msg), "%s took point %s", name, g_label[s]);
		}
		else
			formatex(msg, charsmax(msg), "%s took point %s", side == 1 ? "Terrorists" : "Counter-Terrorists", g_label[s]);
		announce(0, side == 1 ? 255 : 80, 120, side == 2 ? 255 : 80, 2.5, msg);
		client_cmd(0, "spk buttons/bell1");
	}
	comp_debug("capture %d: side %d took it from %d", g_ent[s], side, old);
	hint_comp(s);
}

stock bar_all_inside(s, secs)
{
	for (new p = 1; p <= MaxClients; p++)
		if (g_alive[p] && !g_bot[p] && inside_get(p, s) && !g_act[p] && !g_xslot[p])
			bar_time(p, secs);
}

/* ---- safe zone ---- */

/* Z_FROMR is the radius until a shrink is under way (Z_T1 > Z_T0); Z_TOR is the next one */
stock Float:zone_radius_now(s)
{
	if (g_pf[s][Z_T1] <= g_pf[s][Z_T0])
		return g_pf[s][Z_FROMR];
	if (g_now >= g_pf[s][Z_T1])
		return g_pf[s][Z_TOR];
	if (g_now <= g_pf[s][Z_T0])
		return g_pf[s][Z_FROMR];
	new Float:f = (g_now - g_pf[s][Z_T0]) / (g_pf[s][Z_T1] - g_pf[s][Z_T0]);
	return g_pf[s][Z_FROMR] + (g_pf[s][Z_TOR] - g_pf[s][Z_FROMR]) * f;
}

stock tick_zone(s, bool:sec)
{
	/* move the centre towards the target while shrinking */
	new Float:r = zone_radius_now(s);
	if (g_pf[s][Z_T1] > g_pf[s][Z_T0] && g_now < g_pf[s][Z_T1] && g_now > g_pf[s][Z_T0])
	{
		new Float:f = (g_now - g_pf[s][Z_T0]) / (g_pf[s][Z_T1] - g_pf[s][Z_T0]);
		for (new i = 0; i < 2; i++)
			g_va[s][i] = g_org[s][i] + (g_vb[s][i] - g_org[s][i]) * f;
	}
	else if (g_now >= g_pf[s][Z_T1] && g_pf[s][Z_T1] > 0.0)
	{
		vcopy(g_vb[s], g_va[s]);
		vcopy(g_vb[s], g_org[s]);
		g_pf[s][Z_FROMR] = g_pf[s][Z_TOR];
		g_pf[s][Z_T0] = 0.0;
		g_pf[s][Z_T1] = 0.0;
	}
	g_val[s] = r;
	new bool:dmgnow = g_now >= g_pf[s][Z_NEXTDMG];
	if (dmgnow)
		g_pf[s][Z_NEXTDMG] = g_now + g_pf[s][Z_INTERVAL];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p])
			continue;
		new Float:d = dist2d(g_porg[p], g_va[s]);
		new bool:out = d > r;
		if (out != inside_get(p, s))
		{
			inside_set(p, s, out);
			fwd3(g_fwZoneCross, g_ent[s], p, out ? 1 : 0);
		}
		if (!out)
		{
			if (!g_bot[p] && g_pf[s][Z_TOR] < r - 1.0)
				hud_add(p, false, "The zone is closing in");
			continue;
		}
		if (!g_bot[p])
		{
			new dir[24];
			dir_word(p, g_va[s], dir, charsmax(dir));
			hud_add(p, true, "OUTSIDE THE ZONE - %d m %s", floatround((d - r) * 0.0254), dir);
		}
		if (dmgnow && g_pf[s][Z_DMG] > 0.0)
		{
			new ret;
			ExecuteForward(g_fwZoneTick, ret, g_ent[s], p, g_pf[s][Z_DMG]);
			if (ret < PLUGIN_HANDLED)
			{
				fade(p, 200, 0, 0, 80, 0.4);
				hurt(p, g_ent[s], g_pf[s][Z_DMG], DMG_GENERIC);
			}
		}
	}
	if (sec)
		zone_beams(s, r);
}

stock zone_set(s, const Float:c[3], Float:r)
{
	vcopy(c, g_org[s]);
	vcopy(c, g_va[s]);
	vcopy(c, g_vb[s]);
	g_pf[s][Z_FROMR] = r;
	g_pf[s][Z_TOR] = r;
	g_pf[s][Z_T0] = 0.0;
	g_pf[s][Z_T1] = 0.0;
	g_val[s] = r;
	hint_comp(s);
}

stock zone_shrink(s, const Float:c[3], Float:r, Float:secs)
{
	/* start from where it is now */
	vcopy(g_va[s], g_org[s]);
	g_pf[s][Z_FROMR] = zone_radius_now(s);
	vcopy(c, g_vb[s]);
	g_pf[s][Z_TOR] = floatmax(r, 0.0);
	g_pf[s][Z_T0] = g_now;
	g_pf[s][Z_T1] = g_now + floatmax(secs, 0.1);
	pulse(g_va[s], g_pf[s][Z_FROMR]);
	hint_comp(s);
}

/* ---- zone shrinker: stages "radius:warn:shrink:damage ..." ---- */

stock bool:shrinker_stage(s, n, &Float:r, &Float:warn, &Float:shrink, &Float:dmg)
{
	new stages[256];
	ks(s, "stages", stages, charsmax(stages), "2400:60:40:1 1600:45:35:2 900:40:30:4 400:30:25:8 0:20:20:15");
	new tok[48], pos = 0, i = 0;
	while (stages[pos])
	{
		pos += next_token(stages[pos], tok, charsmax(tok));
		if (!tok[0])
			break;
		i++;
		if (i < n)
			continue;
		new a[12], b[12], c[12], d[12];
		replace_all(tok, charsmax(tok), ":", " ");
		parse(tok, a, charsmax(a), b, charsmax(b), c, charsmax(c), d, charsmax(d));
		r = str_to_float(a);
		warn = floatmax(str_to_float(b), 0.0);
		shrink = floatmax(str_to_float(c), 0.1);
		dmg = d[0] ? str_to_float(d) : 1.0;
		return true;
	}
	return false;
}

stock tick_shrinker(s)
{
	if (g_state[s] != 1 || g_now < g_pf[s][S_NEXT])
		return;
	new z = g_aux[s];
	if (z < 0 || !g_ent[z] || g_kind[z] != K_ZONE)
	{
		g_state[s] = 0;
		return;
	}
	new Float:r, Float:warn, Float:shrink, Float:dmg;
	if (g_pf[s][S_PHASE] == 0.0)
	{
		/* warn: show the next circle */
		new n = floatround(g_pf[s][S_STAGE]) + 1;
		if (!shrinker_stage(s, n, r, warn, shrink, dmg))
		{
			g_state[s] = 2;
			return;
		}
		r = floatmax(r, kf(s, "final_r", 0.0));
		new Float:c[3];
		vcopy(g_va[z], c);
		if (ki(s, "random_center", 1))
			random_center(g_va[z], g_val[z], r, c);
		vcopy(c, g_vb[z]);
		/* the next circle, shown but not closing yet */
		g_pf[z][Z_TOR] = r;
		g_pf[z][Z_T0] = 0.0;
		g_pf[z][Z_T1] = 0.0;
		g_pf[s][S_STAGE] = float(n);
		g_pf[z][Z_STAGE] = float(n);
		g_pf[s][S_PHASE] = 1.0;
		g_pf[s][S_NEXT] = g_now + warn;
		if (warn > 0.0)
		{
			new msg[64];
			formatex(msg, charsmax(msg), "The zone closes in %d seconds", floatround(warn));
			announce(0, 255, 200, 60, 3.0, msg);
			if (ki(s, "roundtime", 1))
				round_timer(floatround(warn));
		}
		hint("zone %.0f %.0f %.0f %.0f %.0f %.0f %.0f %.0f %.0f", g_va[z][0], g_va[z][1], g_org[z][2], g_val[z], c[0], c[1], g_org[z][2], r, warn);
	}
	else
	{
		/* shrink now */
		new n = floatround(g_pf[s][S_STAGE]);
		shrinker_stage(s, n, r, warn, shrink, dmg);
		r = g_pf[z][Z_TOR];
		new Float:c[3];
		vcopy(g_vb[z], c);
		zone_shrink(z, c, r, shrink);
		g_pf[z][Z_DMG] = dmg;
		g_pf[s][S_PHASE] = 0.0;
		g_pf[s][S_NEXT] = g_now + shrink;
		fwd2(g_fwZoneStage, g_ent[z], n);
		announce(0, 255, 90, 60, 3.0, "The zone is closing!");
		if (ki(s, "roundtime", 1))
			round_timer(floatround(shrink));
	}
}

/* a new centre so the new circle sits inside the old one, on solid ground */
stock random_center(const Float:c[3], Float:r, Float:nr, Float:out[3])
{
	vcopy(c, out);
	new Float:room = r - nr;
	if (room <= 1.0)
		return;
	for (new tries = 0; tries < 8; tries++)
	{
		new Float:a = random_float(0.0, 360.0);
		new Float:d = floatsqroot(random_float(0.0, 1.0)) * room;
		new Float:p[3];
		p[0] = c[0] + d * floatcos(a, degrees);
		p[1] = c[1] + d * floatsin(a, degrees);
		/* ground there, whatever the hills: open air found from high up, then straight down */
		new bool:open = false;
		for (new h = 3000; h >= -3000 && !open; h -= 250)
		{
			p[2] = floatclamp(c[2] + float(h), -4000.0, 4000.0);
			open = engfunc(EngFunc_PointContents, p) == CONTENTS_EMPTY;
		}
		if (!open)
			continue;
		new Float:down[3], Float:frac;
		down = p;
		down[2] = -4090.0;
		engfunc(EngFunc_TraceLine, p, down, IGNORE_MONSTERS, 0, 0);
		get_tr2(0, TR_flFraction, frac);
		if (frac >= 1.0)
			continue;
		new Float:g[3];
		get_tr2(0, TR_vecEndPos, g);
		g[2] += 40.0;
		if (engfunc(EngFunc_PointContents, g) != CONTENTS_EMPTY)
			continue;
		out[0] = p[0];
		out[1] = p[1];
		return;
	}
}

/* ---- damage zone ---- */

stock tick_damage(s)
{
	new bool:now = g_now >= g_pf[s][D_NEXT];
	if (now)
		g_pf[s][D_NEXT] = g_now + g_pf[s][D_INTERVAL];
	new kind[16];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || (g_team[s] && g_pteam[p] != g_team[s]))
			continue;
		if (!inside(s, g_porg[p]))
			continue;
		if (!g_bot[p] && ki(s, "hud_warning", 1))
		{
			ks(s, "kind", kind, charsmax(kind), "danger");
			hud_add(p, true, "Danger: %s", kind);
		}
		if (!now)
			continue;
		new ret;
		ExecuteForward(g_fwDamage, ret, g_ent[s], p, g_pf[s][D_DMG]);
		if (ret >= PLUGIN_HANDLED)
			continue;
		new bits = floatround(g_pf[s][D_BITS]);
		fade(p, bits == DMG_POISON ? 40 : 220, bits == DMG_POISON ? 200 : (bits == DMG_SHOCK ? 200 : 80), bits == DMG_SHOCK ? 255 : 0, 70, 0.3);
		hurt(p, g_ent[s], g_pf[s][D_DMG], bits);
	}
}

/* ---- timed door ---- */

stock bool:door_is_open(s)
{
	for (new d = 0; d < g_ndoors[s]; d++)
	{
		new e = g_doors[s][d];
		if (!pev_valid(e))
			continue;
		new Float:o[3], Float:a[3];
		pev(e, pev_origin, o);
		pev(e, pev_angles, a);
		if (get_distance_f(o, g_dorg[s][d]) > 1.0 || get_distance_f(a, g_dang[s][d]) > 1.0)
			return true;
	}
	return false;
}

stock bool:door_locked(s)
{
	new Float:rt = round_time();
	if (g_pf[s][DR_LOCKEDT] > g_now)
		return true;
	return g_pf[s][DR_LOCKUNTIL] > 0.0 && (rt < 0.0 || rt < g_pf[s][DR_LOCKUNTIL]);
}

new bool:g_doorBypass;

stock door_use(s, bool:open)
{
	for (new d = 0; d < g_ndoors[s]; d++)
	{
		new e = g_doors[s][d];
		if (!pev_valid(e))
			continue;
		new Float:o[3], Float:a[3];
		pev(e, pev_origin, o);
		pev(e, pev_angles, a);
		new bool:closed = get_distance_f(o, g_dorg[s][d]) <= 1.0 && get_distance_f(a, g_dang[s][d]) <= 1.0;
		/* only push a door that's at rest at the other end */
		new Float:vel[3], Float:avel[3];
		pev(e, pev_velocity, vel);
		pev(e, pev_avelocity, avel);
		new bool:moving = vector_length(vel) > 0.1 || vector_length(avel) > 0.1;
		if (moving || closed != open)
			continue;
		g_doorBypass = true;
		ExecuteHamB(Ham_Use, e, g_ent[s], g_ent[s], 3, 0.0);
		g_doorBypass = false;
	}
}

stock tick_door(s)
{
	new Float:rt = round_time();
	if (rt >= 0.0)
	{
		if (g_pf[s][DR_OPENAT] >= 0.0 && rt >= g_pf[s][DR_OPENAT] && !(floatround(g_pf[s][DR_DONE]) & 1))
		{
			g_pf[s][DR_DONE] = float(floatround(g_pf[s][DR_DONE]) | 1);
			door_use(s, true);
		}
		if (g_pf[s][DR_CLOSEAT] >= 0.0 && rt >= g_pf[s][DR_CLOSEAT] && !(floatround(g_pf[s][DR_DONE]) & 2))
		{
			g_pf[s][DR_DONE] = float(floatround(g_pf[s][DR_DONE]) | 2);
			door_use(s, false);
		}
	}
	new bool:open = door_is_open(s);
	new bool:was = g_pf[s][DR_WASOPEN] > 0.0;
	if (open != was)
	{
		g_pf[s][DR_WASOPEN] = open ? 1.0 : 0.0;
		if (open)
		{
			g_pf[s][DR_OPENEDAT] = g_now;
			fire_targets(s, 0);
		}
		fwd2(g_fwDoor, g_ent[s], open ? 1 : 0);
		hint_comp(s);
	}
	if (open && g_pf[s][DR_OPENSECS] > 0.0 && g_now - g_pf[s][DR_OPENEDAT] >= g_pf[s][DR_OPENSECS])
		door_use(s, false);
	/* the lock running out changes what bots may do */
	new bool:lk = door_locked(s);
	if (lk != (g_state[s] != 0))
	{
		g_state[s] = lk ? 1 : 0;
		hint_comp(s);
	}
}

stock door_lock(s, Float:secs, who)
{
	g_pf[s][DR_LOCKEDT] = secs > 0.0 ? g_now + secs : 0.0;
	if (secs > 0.0)
		fwd3f(g_fwDoorLocked, g_ent[s], who, secs);
	g_state[s] = door_locked(s) ? 1 : 0;
	hint_comp(s);
}

public ham_door_use(door, caller, activator, type, Float:value)
{
	if (g_doorBypass || door >= MAX_ENTS || !g_doorOf[door])
		return HAM_IGNORED;
	new s = g_doorOf[door] - 1;
	if (!g_on[s] || !door_locked(s))
		return HAM_IGNORED;
	locked_msg(activator);
	return HAM_SUPERCEDE;
}

public ham_door_touch(door, other)
{
	if (g_doorBypass || door >= MAX_ENTS || !g_doorOf[door])
		return HAM_IGNORED;
	new s = g_doorOf[door] - 1;
	if (!g_on[s] || !door_locked(s))
		return HAM_IGNORED;
	locked_msg(other);
	return HAM_SUPERCEDE;
}

stock locked_msg(id)
{
	if (1 <= id <= MaxClients && is_user_connected(id) && !is_user_bot(id) && get_gametime() >= g_lockMsg[id])
	{
		g_lockMsg[id] = get_gametime() + 2.0;
		client_print(id, print_center, "Locked");
	}
}

/* ---- trap ---- */

stock trap_fire(s, activator)
{
	if (g_state[s] != TRAP_ARMED)
		return 0;
	g_state[s] = TRAP_PENDING;
	g_aux[s] = activator;
	g_pf[s][T_FIREAT] = g_now + g_pf[s][T_DELAY];
	return 1;
}

stock tick_trap(s)
{
	switch (g_state[s])
	{
		case TRAP_ARMED:
		{
			new on[16];
			ks(s, "fire_on", on, charsmax(on), "touch");
			if (!equali(on, "touch"))
				return;
			new tt = kteam(s, "team_only", 0);
			for (new p = 1; p <= MaxClients; p++)
			{
				if (g_alive[p] && (!tt || g_pteam[p] == tt) && inside(s, g_porg[p]))
				{
					trap_fire(s, p);
					break;
				}
			}
		}
		case TRAP_PENDING:
		{
			if (g_now < g_pf[s][T_FIREAT])
				return;
			new act = g_aux[s];
			fire_targets(s, act);
			new victims = 0;
			if (g_pf[s][T_DMG] > 0.0)
			{
				for (new p = 1; p <= MaxClients; p++)
				{
					if (g_alive[p] && inside(s, g_porg[p]))
					{
						hurt(p, (1 <= act <= MaxClients && is_user_connected(act)) ? act : g_ent[s], g_pf[s][T_DMG], DMG_BLAST);
						victims++;
					}
				}
			}
			fwd3(g_fwTrap, g_ent[s], act, victims);
			/* W1: a paid timed trap stays on a while */
			if (up_trap_fired(s))
			{
				hint_comp(s);
				return;
			}
			comp_debug("trap %d fired by %d, %d hurt", g_ent[s], act, victims);
			if (g_pf[s][T_REARM] > 0.0)
			{
				g_state[s] = TRAP_COOLDOWN;
				g_pf[s][T_ARMAT] = g_now + g_pf[s][T_REARM];
			}
			else
				g_state[s] = TRAP_OFF;
			hint_comp(s);
		}
		case TRAP_ACTIVE: up_trap_tick(s); /* W1 */
		case TRAP_COOLDOWN:
		{
			if (g_now >= g_pf[s][T_ARMAT])
			{
				g_state[s] = TRAP_ARMED;
				hint_comp(s);
			}
		}
	}
}

/* ---- weapon spawner ---- */

stock spawner_roll(s)
{
	new item[48];
	ks(s, "item", item, charsmax(item), "table:default");
	if (equal(item, "table:", 6))
	{
		if (!loot_roll(item[6], g_item[s], charsmax(g_item[]), floatround(g_pf[s][W_TIER])))
			copy(g_item[s], charsmax(g_item[]), "glock18");
	}
	else
		copy(g_item[s], charsmax(g_item[]), item);
}

stock spawner_show(s, bool:on)
{
	new ent = g_ent[s];
	g_state[s] = on ? 0 : 1;
	if (on)
	{
		new m[64];
		item_model(g_item[s], m, charsmax(m));
		set_model(ent, m, DEFAULT_BOX);
		engfunc(EngFunc_SetSize, ent, Float:{-8.0, -8.0, 0.0}, Float:{8.0, 8.0, 8.0});
		set_pev(ent, pev_effects, pev(ent, pev_effects) & ~EF_NODRAW);
		new Float:o[3];
		vcopy(g_org[s], o);
		o[2] = g_floor[s] + 8.0;
		engfunc(EngFunc_SetOrigin, ent, o);
		tier_glow(ent, w3_spawner_tier(s, floatround(g_pf[s][W_TIER]))); /* W3 */
	}
	else
		set_pev(ent, pev_effects, pev(ent, pev_effects) | EF_NODRAW);
	if (g_ready)
		hint_comp(s);
}

stock tick_spawner(s)
{
	if (g_state[s] == 1)
	{
		if (g_pf[s][W_RESPAWN] > 0.0 && g_now >= g_pf[s][W_BACKAT] && !ki(s, "once", 0))
		{
			spawner_roll(s);
			spawner_show(s, true);
			fwd1(g_fwWeaponBack, g_ent[s]);
		}
		return;
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p])
			continue;
		new Float:d = dist2d(g_porg[p], g_org[s]);
		new Float:dz = g_porg[p][2] - g_floor[s];
		/* (a bot's path can end a little short of a gun by a wall or on a ledge) */
		if (d > (g_bot[p] ? 96.0 : 40.0) || dz < -16.0 || dz > 96.0)
			continue;
		if (!can_take(p, g_item[s]))
			continue;
		new ret;
		ExecuteForward(g_fwWeaponTaken, ret, g_ent[s], p, g_item[s]);
		if (ret < PLUGIN_HANDLED)
			give_thing(p, g_item[s]);
		spawner_show(s, false);
		g_pf[s][W_BACKAT] = g_now + g_pf[s][W_RESPAWN];
		break;
	}
}

/* ---- loot container ---- */

stock loot_look(s, bool:full)
{
	new ent = g_ent[s];
	g_state[s] = full ? 0 : 1;
	if (full)
	{
		if (ki(s, "glow", 1))
			tier_glow(ent, floatround(g_pf[s][W_TIER]));
		else
			render(ent, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
	}
	else
		render(ent, kRenderFxNone, 0, 0, 0, kRenderTransTexture, 110);
	if (g_ready)
		hint_comp(s);
}

stock tick_loot(s)
{
	solid_when_clear(s, SOLID_BBOX);
	if (g_state[s] == 1)
	{
		if (g_pf[s][W_RESPAWN] > 0.0 && g_now >= g_pf[s][W_BACKAT])
			loot_look(s, true);
		return;
	}
	bots_open(s);
}

/* bots can't hold USE yet: one standing by a full container starts opening it */
stock bots_open(s)
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || !g_bot[p] || g_act[p])
			continue;
		/* (side by side, not 3D: a bot's origin is 36 above the crate's) */
		if (dist2d(g_porg[p], g_org[s]) < 80.0 && floatabs(g_porg[p][2] - g_org[s][2]) < 90.0)
		{
			act_start(p, s, ACT_LOOT, g_pf[s][W_OPEN]);
			break;
		}
	}
}

stock loot_give(s, id)
{
	new ret;
	ExecuteForward(g_fwLoot, ret, g_ent[s], id);
	if (g_kind[s] == K_CRATE)
	{
		fwd_drop(g_ent[s], 2, g_org[s]);
		g_state[s] = CRATE_EMPTY;
		render(g_ent[s], kRenderFxNone, 0, 0, 0, kRenderTransTexture, 110);
		hint_comp(s);
	}
	else
	{
		loot_look(s, false);
		if (!ki(s, "once", 1) || g_pf[s][W_RESPAWN] > 0.0)
			g_pf[s][W_BACKAT] = g_now + floatmax(g_pf[s][W_RESPAWN], 1.0);
		else
			g_pf[s][W_BACKAT] = 999999.0;
	}
	fire_targets(s, id);
	if (ret >= PLUGIN_HANDLED || !is_user_alive(id))
		return;
	new n = max(floatround(g_pf[s][W_ITEMS]), 1);
	new tier = floatround(g_pf[s][W_TIER]);
	new item[32], list[160];
	for (new i = 0; i < n; i++)
	{
		if (!loot_roll(g_item[s], item, charsmax(item), tier))
			break;
		new r2;
		ExecuteForward(g_fwLootItem, r2, g_ent[s], id, item);
		if (r2 >= PLUGIN_HANDLED)
			continue;
		if (give_thing(id, item))
		{
			if (list[0])
				add(list, charsmax(list), ", ");
			add(list, charsmax(list), item);
		}
	}
	if (list[0] && !is_user_bot(id))
		client_print(id, print_center, "Found: %s", list);
	emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
}

/* ---- supply drop: a marker that calls a crate ---- */

stock tick_drop_marker(s)
{
	new Float:rt = round_time();
	if (g_pf[s][DP_AT] < 0.0 || rt < 0.0 || g_pf[s][DP_DONE] > 0.0 || rt < g_pf[s][DP_AT])
		return;
	g_pf[s][DP_DONE] = 1.0;
	if (random_num(1, 100) <= ki(s, "chance", 100))
		drop_from_marker(s);
}

stock drop_from_marker(s)
{
	new table[32], keys[160];
	ks(s, "table", table, charsmax(table), "drop");
	formatex(keys, charsmax(keys), "table=%s fall_secs=%.1f items=%d open_secs=%.1f beacon=%d",
		table, kf(s, "fall_secs", 20.0), ki(s, "items", 4), kf(s, "open_secs", 3.0), ki(s, "beacon", 1));
	return drop_call(g_org[s], keys);
}

stock drop_call(const Float:at[3], const keys[])
{
	new Float:ground[3];
	vcopy(at, ground);
	ground[2] = floor_z(at, 0) + 1.0;
	/* start high: under the sky or 2000 units up, whichever is lower */
	new Float:up[3], Float:hit[3];
	vcopy(ground, up);
	up[2] += 2000.0;
	engfunc(EngFunc_TraceLine, ground, up, IGNORE_MONSTERS, 0, 0);
	get_tr2(0, TR_vecEndPos, hit);
	new Float:top = floatmax(hit[2] - 40.0, ground[2] + 1.0);
	new Float:start[3];
	vcopy(ground, start);
	start[2] = top;
	new ent = comp_make(K_CRATE, start, keys, SRC_RUNTIME);
	if (!ent)
		return 0;
	new s = slot_of(ent);
	g_temp[s] = true;
	vcopy(ground, g_org[s]);
	g_floor[s] = ground[2] - 1.0;
	new Float:fall = floatmax(kf(s, "fall_secs", 20.0), 0.1);
	if (top - ground[2] < 8.0)
		fall = 0.1;
	g_state[s] = CRATE_FALLING;
	g_pf[s][W_FALL] = g_now + fall;
	set_pev(ent, pev_movetype, MOVETYPE_NOCLIP);
	new Float:vel[3];
	vel[2] = -(top - ground[2]) / fall;
	set_pev(ent, pev_velocity, vel);
	fwd_drop(ent, 0, ground);
	announce(0, 120, 220, 255, 3.0, "Supply drop incoming!");
	comp_debug("drop %d: lands at %.0f %.0f %.0f in %.0f s", ent, ground[0], ground[1], ground[2], fall);
	hint("avoid c%d %.0f %.0f %.0f 96 5 %.0f", ent, ground[0], ground[1], ground[2], fall);
	return ent;
}

stock tick_crate(s, bool:sec)
{
	new ent = g_ent[s];
	if (g_state[s] == CRATE_FALLING)
	{
		if (g_now >= g_pf[s][W_FALL])
		{
			set_pev(ent, pev_velocity, Float:{0.0, 0.0, 0.0});
			set_pev(ent, pev_movetype, MOVETYPE_NONE);
			engfunc(EngFunc_SetOrigin, ent, g_org[s]);
			engfunc(EngFunc_SetSize, ent, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 24.0});
			/* never solid around someone standing there: it turns solid once the spot is clear */
			set_pev(ent, pev_solid, SOLID_NOT);
			g_aux[s] = 1;
			g_state[s] = CRATE_FULL;
			tier_glow(ent, 5);
			bots_to_crate(s);
			smoke(g_org[s]);
			fwd_drop(ent, 1, g_org[s]);
			fire_targets(s, 0);
			hint_comp(s);
		}
	}
	else if (g_state[s] == CRATE_FULL)
	{
		/* nobody's opened it yet: the nearest bots are sent again every 15 s for 2 min (a fight
		   or a 45-s goal running out left most of a big map's drops shut) */
		if (sec && g_now - g_pf[s][W_FALL] < 120.0 && floatround(g_now - g_pf[s][W_FALL]) % 15 == 0
			&& !creature_near(g_org[s], g_org[s], 1500.0))
			bots_to_crate(s);
		bots_open(s);
		solid_when_clear(s, SOLID_BBOX);
	}
	if (sec && g_state[s] != CRATE_EMPTY && ki(s, "beacon", 1))
	{
		new Float:a[3], Float:b[3];
		vcopy(g_org[s], a);
		vcopy(g_org[s], b);
		b[2] += 900.0;
		beam(0, a, b, 255, 60, 30, 11, 16, 200);
	}
}

/* a landed supply crate: the two nearest bots go for it (others come anyway: it's attract-ed) */
stock bots_to_crate(s)
{
	new near[2], Float:nd[2] = { 4500.0, 4500.0 }, Float:o[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		pev(p, pev_origin, o);
		new Float:d = get_distance_f(o, g_org[s]);
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
		if (near[i])
			hint("goal %d loot %.0f %.0f %.0f %d 70 30 48", near[i], g_org[s][0], g_org[s][1], g_org[s][2], g_ent[s]);
}

/* ---- barricade ---- */

stock barricade_stand(s, bool:up)
{
	new ent = g_ent[s];
	if (!pev_valid(ent))
		return;
	new model[8];
	pev(ent, pev_model, model, charsmax(model));
	if (up)
	{
		g_state[s] = 0;
		set_pev(ent, pev_effects, pev(ent, pev_effects) & ~EF_NODRAW);
		/* solid now only if nobody is inside it; otherwise once they've stepped out */
		g_aux[s] = 1;
		set_pev(ent, pev_solid, SOLID_NOT);
		solid_when_clear(s, model[0] == '*' ? SOLID_BSP : SOLID_BBOX);
		set_pev(ent, pev_takedamage, DAMAGE_YES);
		set_pev(ent, pev_health, g_val[s] > 0.0 ? g_val[s] : 1.0);
		render(ent, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
		/* brushes stay where the map put them; boxes go back to their spot */
		if (model[0] != '*')
			engfunc(EngFunc_SetOrigin, ent, g_org[s]);
	}
	else
	{
		g_state[s] = 1;
		set_pev(ent, pev_effects, pev(ent, pev_effects) | EF_NODRAW);
		set_pev(ent, pev_solid, SOLID_NOT);
		set_pev(ent, pev_takedamage, DAMAGE_NO);
	}
	barricade_tint(s);
	if (g_ready)
		hint_comp(s);
}

/* how a barricade looks for its health: a brush one is see-through (more so as it weakens,
   or alpha-tested with alpha_test=1 for a { fence texture); boards= brushes go one by one */
stock barricade_tint(s)
{
	new Float:f = floatclamp(g_val[s] / g_pf[s][B_MAX], 0.0, 1.0);
	new ent = g_ent[s];
	new m[4];
	pev(ent, pev_model, m, charsmax(m));
	if (m[0] == '*')
	{
		if (ki(s, "alpha_test", 0))
			render(ent, kRenderFxNone, 0, 0, 0, kRenderTransAlpha, 255);
		else if (ki(s, "see_through", 1))
			render(ent, kRenderFxNone, 0, 0, 0, kRenderTransTexture, 90 + floatround(110.0 * f));
		else
			render(ent, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
	}
	else if (f >= 0.99)
		render(ent, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
	else
		render(ent, kRenderFxNone, 0, 0, 0, kRenderTransTexture, 120 + floatround(135.0 * f));
	/* boards: as many showing as the health is worth */
	/* the bots' brain: pev_iuser4 1 = shoot-through by design (W2), for the barricade and its boards */
	new st = ki(s, "see_through", 1) ? 1 : 0;
	g_seeThrough[s] = st != 0;
	set_pev(ent, pev_iuser4, st);
	new n = g_nboards[s];
	for (new i = 0; i < n; i++)
		if (pev_valid(g_boards[s][i]))
			set_pev(g_boards[s][i], pev_iuser4, st);
	if (n)
	{
		new show = g_state[s] == 1 ? 0 : floatround(float(n) * f, floatround_ceil);
		for (new i = 0; i < n; i++)
		{
			new b = g_boards[s][i];
			if (!pev_valid(b))
				continue;
			if (i < show)
				set_pev(b, pev_effects, pev(b, pev_effects) & ~EF_NODRAW);
			else
				set_pev(b, pev_effects, pev(b, pev_effects) | EF_NODRAW);
		}
	}
}

/* Shooting and seeing through a barricade's gaps (CoD style): nobody walks through an intact
   barricade (movement uses hull traces: the barricade's CLIP brush or box stops players, bots and
   creatures), but a point trace (a bullet, a bot's look) that hits a see-through barricade or one
   of its boards carries on past it to whatever is behind */
new g_traceDepth;

new g_tsSeen, g_tsPassed, g_tsEnded, g_tsSkipped, g_tsBullets;

public cmd_tracestats()
{
	server_print("barricade traces: seen %d (bullet-length %d), passed %d, ended inside %d, skipped (depth/not see-through) %d; creature hits stopped by a gate %d", g_tsSeen, g_tsBullets, g_tsPassed, g_tsEnded, g_tsSkipped, g_gateBlocks);
	return PLUGIN_HANDLED;
}

public fm_traceline_post(const Float:v1[3], const Float:v2[3], noMonsters, skip, tr)
{
	new hit = get_tr2(tr, TR_pHit);
	if (hit > 0 && hit < MAX_ENTS && (g_slotOf[hit] || g_boardOf[hit]))
	{
		new ss = g_slotOf[hit] ? g_slotOf[hit] - 1 : g_boardOf[hit] - 1;
		if (g_kind[ss] == K_BARRICADE)
		{
			g_tsSeen++;
			if (get_distance_f(v1, v2) >= 1024.0)
				g_tsBullets++;
			if (g_traceDepth >= 4 || !g_seeThrough[ss])
				g_tsSkipped++;
		}
	}
	if (hit <= 0 || hit >= MAX_ENTS || g_traceDepth >= 4)
		return FMRES_IGNORED;
	/* only players' traces (their bullets, bots' sight) go through: a creature's claws don't
	   (deeper layers are this hook's own carry-on traces) */
	if (!(1 <= skip <= MaxClients) && !g_traceDepth)
		return FMRES_IGNORED;
	new s = g_slotOf[hit] ? g_slotOf[hit] - 1 : g_boardOf[hit] - 1;
	if (s < 0 || g_kind[s] != K_BARRICADE || !g_seeThrough[s])
		return FMRES_IGNORED;
	new Float:dir[3], Float:start[3];
	new Float:total = get_distance_f(v1, v2);
	if (total < 1.0)
		return FMRES_IGNORED;
	for (new i = 0; i < 3; i++)
		dir[i] = (v2[i] - v1[i]) / total;
	/* carry on from where the line leaves the barricade's box (a shot from right against it or
	   from inside it, and a thick brush, are all past in one step) */
	new Float:mn[3], Float:mx[3];
	pev(hit, pev_absmin, mn);
	pev(hit, pev_absmax, mx);
	new Float:tout = total;
	for (new i = 0; i < 3; i++)
	{
		if (floatabs(dir[i]) < 0.0001)
			continue;
		new Float:t1 = (mn[i] - v1[i]) / dir[i], Float:t2 = (mx[i] - v1[i]) / dir[i];
		new Float:far = floatmax(t1, t2);
		if (far < tout)
			tout = far;
	}
	new Float:done = floatmax(tout + 1.0, 1.0);
	if (done >= total)
	{
		/* the shot ends inside the barricade: as if it went through to nothing */
		set_tr2(tr, TR_flFraction, 1.0);
		set_tr2(tr, TR_vecEndPos, v2);
		set_tr2(tr, TR_pHit, 0);
		set_tr2(tr, TR_StartSolid, 0);
		set_tr2(tr, TR_AllSolid, 0);
		g_tsEnded++;
		return FMRES_IGNORED;
	}
	for (new i = 0; i < 3; i++)
		start[i] = v1[i] + dir[i] * done;
	new tr2 = create_tr2();
	g_traceDepth++;
	engfunc(EngFunc_TraceLine, start, v2, noMonsters, hit, tr2);
	g_traceDepth--;
	new Float:f2, Float:e2[3], Float:n2[3];
	get_tr2(tr2, TR_flFraction, f2);
	get_tr2(tr2, TR_vecEndPos, e2);
	get_tr2(tr2, TR_vecPlaneNormal, n2);
	/* past the barricade it hit the world (or nothing): fakemeta can't set the hit entity to the
	   world, so the result would still name the barricade; make it a clean miss instead */
	if (get_tr2(tr2, TR_pHit) <= 0)
	{
		free_tr2(tr2);
		set_tr2(tr, TR_flFraction, 1.0);
		set_tr2(tr, TR_vecEndPos, v2);
		set_tr2(tr, TR_StartSolid, 0);
		set_tr2(tr, TR_AllSolid, 0);
		g_tsPassed++;
		return FMRES_IGNORED;
	}
	set_tr2(tr, TR_flFraction, floatmin((done + f2 * (total - done)) / total, 1.0));
	set_tr2(tr, TR_vecEndPos, e2);
	set_tr2(tr, TR_vecPlaneNormal, n2);
	set_tr2(tr, TR_pHit, get_tr2(tr2, TR_pHit));
	set_tr2(tr, TR_iHitgroup, get_tr2(tr2, TR_iHitgroup));
	set_tr2(tr, TR_InOpen, get_tr2(tr2, TR_InOpen));
	set_tr2(tr, TR_InWater, get_tr2(tr2, TR_InWater));
	set_tr2(tr, TR_StartSolid, 0);
	set_tr2(tr, TR_AllSolid, get_tr2(tr2, TR_AllSolid));
	free_tr2(tr2);
	g_tsPassed++;
	return FMRES_IGNORED;
}

public ham_takedamage(ent, inflictor, attacker, Float:dmg, bits)
{
	new s = slot_of(ent);
	if (s < 0 || g_kind[s] != K_BARRICADE)
		return HAM_IGNORED;
	if (!g_on[s] || g_state[s] == 1)
		return HAM_SUPERCEDE;
	if (1 <= attacker <= MaxClients && g_team[s] && get_user_team(attacker) == g_team[s] && !ki(s, "friendly_damage", 0))
		return HAM_SUPERCEDE;
	g_val[s] -= dmg;
	set_pev(ent, pev_health, floatmax(g_val[s], 1.0));
	if (g_val[s] <= 0.0)
	{
		g_val[s] = 0.0;
		barricade_break(s, attacker);
		return HAM_SUPERCEDE;
	}
	new Float:step = float(floatround(g_val[s] * 4.0 / g_pf[s][B_MAX], floatround_ceil));
	if (step < g_pf[s][B_LASTSTEP])
	{
		g_pf[s][B_LASTSTEP] = step;
		fwd3(g_fwBarDamaged, g_ent[s], attacker, floatround(g_val[s]));
	}
	barricade_tint(s);
	return HAM_SUPERCEDE;
}

stock barricade_break(s, attacker)
{
	new Float:c[3];
	for (new i = 0; i < 3; i++)
		c[i] = (g_mins[s][i] + g_maxs[s][i]) * 0.5;
	message_begin_f(MSG_PVS, SVC_TEMPENTITY, c);
	write_byte(TE_BREAKMODEL);
	write_coord(floatround(c[0])); write_coord(floatround(c[1])); write_coord(floatround(c[2]));
	write_coord(floatround(g_maxs[s][0] - g_mins[s][0])); write_coord(floatround(g_maxs[s][1] - g_mins[s][1])); write_coord(floatround(g_maxs[s][2] - g_mins[s][2]));
	write_coord(0); write_coord(0); write_coord(30);
	write_byte(20);
	write_short(g_mdlGibs);
	write_byte(16);
	write_byte(40);
	write_byte(0x08);
	message_end();
	emit_sound(g_ent[s], CHAN_BODY, "debris/bustcrate1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	barricade_stand(s, false);
	fwd2(g_fwBarBroken, g_ent[s], attacker);
	fire_targets(s, attacker);
	comp_debug("barricade %d broken by %d", g_ent[s], attacker);
}

stock barricade_repair(s, id)
{
	new Float:mx = g_pf[s][B_MAX];
	/* a player adds one board (1/boards of its health); a kit or the mode rebuilds it all */
	if (id)
		g_val[s] = floatmin(g_val[s] + mx / float(max(ki(s, "boards_count", 6), 1)), mx);
	else
		g_val[s] = mx;
	g_pf[s][B_LASTSTEP] = float(floatround(g_val[s] * 4.0 / mx, floatround_ceil));
	if (g_state[s] == 1)
		barricade_stand(s, true);
	set_pev(g_ent[s], pev_health, g_val[s]);
	barricade_tint(s);
	fwd3(g_fwBarRepaired, g_ent[s], id, floatround(g_val[s]));
}

/* ---- waves (shared by every spawner) ---- */

stock wave_start(n)
{
	g_wave = n > 0 ? n : g_wave + 1;
	g_waveOn = true;
	g_ntrack = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_WAVE || !g_on[s])
			continue;
		new Float:waves = g_pf[s][WV_WAVES];
		new Float:f = waves > 1.0 ? floatmin(float(g_wave - 1) / (waves - 1.0), 1.0) : 1.0;
		g_aux[s] = floatround(g_pf[s][WV_MIN] + (g_pf[s][WV_MAX] - g_pf[s][WV_MIN]) * f);
		g_pf[s][WV_NEXT] = g_now + random_float(0.0, 1.0);
	}
	fwd1(g_fwWaveStart, g_wave);
	new msg[48];
	formatex(msg, charsmax(msg), "Wave %d", g_wave);
	if (get_pcvar_num(g_cWaveAnnounce))
		announce(0, 255, 80, 60, 3.0, msg);
	comp_debug("wave %d started", g_wave);
}

stock wave_stop()
{
	g_waveOn = false;
	g_waveAuto = false;
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_WAVE)
			g_aux[s] = 0;
}

stock wave_alive()
{
	new n = 0;
	for (new i = 0; i < g_ntrack; i++)
	{
		new e = g_track[i];
		if (1 <= e <= MaxClients)
		{
			if (is_user_connected(e) && is_user_alive(e))
				n++;
		}
		else if (e > 0 && pev_valid(e) && pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_health) > 0.0
			&& pev(e, pev_team) != 1) /* W1: (a friendly turret on a dead enemy's entity index) */
			n++;
	}
	return n;
}

stock wave_track(ent)
{
	for (new i = 0; i < g_ntrack; i++)
		if (g_track[i] == ent)
			return;
	if (g_ntrack < MAX_TRACK)
		g_track[g_ntrack++] = ent;
}

stock tick_waves()
{
	if (g_waveAuto && !g_waveOn && g_waveNextAt > 0.0 && g_now >= g_waveNextAt)
	{
		g_waveNextAt = 0.0;
		wave_start(0);
	}
	if (!g_waveOn)
		return;
	new left = 0, maxwaves = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_WAVE || !g_on[s])
			continue;
		maxwaves = max(maxwaves, floatround(g_pf[s][WV_WAVES]));
		if (g_aux[s] <= 0)
			continue;
		left += g_aux[s];
		if (g_now < g_pf[s][WV_NEXT])
			continue;
		/* a cap on how many are alive at once (ls_comp_wave_alive_max): the rest wait */
		new cap = get_pcvar_num(g_cWaveAlive);
		if (cap > 0 && wave_alive() >= cap)
		{
			g_pf[s][WV_NEXT] = g_now + 0.5;
			continue;
		}
		if (wave_spawn_one(s))
		{
			g_aux[s]--;
			g_pf[s][WV_NEXT] = g_now + g_pf[s][WV_RATE];
		}
		else
			g_pf[s][WV_NEXT] = g_now + 1.0;
	}
	if (left == 0 && wave_alive() == 0)
	{
		g_waveOn = false;
		fwd1(g_fwWaveCleared, g_wave);
		for (new s = 0; s < g_nslots; s++)
			if (g_ent[s] && g_kind[s] == K_WAVE && g_on[s])
				fire_targets(s, 0);
		new msg[48];
		formatex(msg, charsmax(msg), "Wave %d cleared", g_wave);
		if (get_pcvar_num(g_cWaveAnnounce))
			announce(0, 120, 255, 120, 3.0, msg);
		comp_debug("wave %d cleared", g_wave);
		if (g_waveAuto && g_wave < maxwaves)
		{
			new Float:between = 15.0;
			for (new s = 0; s < g_nslots; s++)
				if (g_ent[s] && g_kind[s] == K_WAVE)
				{
					between = kf(s, "between", 15.0);
					break;
				}
			g_waveNextAt = g_now + between;
		}
	}
}

stock bool:wave_spawn_one(s)
{
	new Float:o[3];
	if (!free_spot_near(g_org[s], g_r[s], o))
		return false;
	new types[64], ret;
	ks(s, "types", types, charsmax(types));
	new arr = PrepareArray(_:o, 3);
	ExecuteForward(g_fwWaveSpawn, ret, g_ent[s], g_wave, arr, types);
	if (ret >= PLUGIN_HANDLED)
		return true;
	/* a dead player of the spawner's team, bots first */
	new pick = 0;
	for (new pass = 0; pass < 2 && !pick; pass++)
	{
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_connected(p) || is_user_alive(p) || get_user_team(p) != g_team[s] || g_waveSpawn[p])
				continue;
			if (pass == 0 && !is_user_bot(p))
				continue;
			if (pass == 1 && !ki(s, "humans", 0))
				continue;
			pick = p;
			break;
		}
	}
	if (!pick)
		return false;
	g_waveSpawn[pick] = true;
	vcopy(o, g_waveOrg[pick]);
	ExecuteHamB(Ham_CS_RoundRespawn, pick);
	wave_track(pick);
	return true;
}

/* a spot a player fits in, within r of c */
stock bool:free_spot_near(const Float:c[3], Float:r, Float:out[3])
{
	for (new tries = 0; tries < 10; tries++)
	{
		new Float:p[3];
		new Float:a = random_float(0.0, 360.0), Float:d = tries == 0 ? 0.0 : random_float(0.0, r);
		p[0] = c[0] + d * floatcos(a, degrees);
		p[1] = c[1] + d * floatsin(a, degrees);
		p[2] = c[2] + 40.0;
		engfunc(EngFunc_TraceHull, p, p, DONT_IGNORE_MONSTERS, HULL_HUMAN, 0, 0);
		if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid) || !get_tr2(0, TR_InOpen))
			continue;
		vcopy(p, out);
		return true;
	}
	return false;
}

/* ---- extraction ---- */

stock bool:extract_open_now(s)
{
	return g_state[s] == 1;
}

stock extract_set(s, bool:open)
{
	if ((g_state[s] == 1) == open)
		return;
	g_state[s] = open ? 1 : 0;
	if (!open)
		for (new p = 1; p <= MaxClients; p++)
			if (g_xslot[p] == s + 1)
				extract_cancel(p);
	fwd2(g_fwExtractOpen, g_ent[s], open ? 1 : 0);
	if (open)
	{
		new msg[64];
		formatex(msg, charsmax(msg), "Extraction%s%s is open", g_label[s][0] ? " " : "", g_label[s]);
		announce(0, 120, 255, 120, 3.0, msg);
	}
	hint_comp(s);
}

stock tick_extract(s)
{
	new Float:rt = round_time();
	if (rt >= 0.0 && !(floatround(g_pf[s][X_DONE]) & 1) && g_pf[s][X_OPENAT] >= 0.0 && rt >= g_pf[s][X_OPENAT])
	{
		g_pf[s][X_DONE] = float(floatround(g_pf[s][X_DONE]) | 1);
		extract_set(s, true);
	}
	if (rt >= 0.0 && !(floatround(g_pf[s][X_DONE]) & 2) && g_pf[s][X_CLOSEAT] >= 0.0 && rt >= g_pf[s][X_CLOSEAT])
	{
		g_pf[s][X_DONE] = float(floatround(g_pf[s][X_DONE]) | 2);
		extract_set(s, false);
	}
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		new bool:in = g_alive[p] && !g_extracted[p] && (!g_team[s] || g_pteam[p] == g_team[s]) && inside(s, g_porg[p]);
		inside_set(p, s, in);
		if (in)
			n++;
	}
	new bool:shared = ki(s, "shared", 0) != 0;
	new Float:rate = shared ? 1.0 + 0.5 * float(max(n - 1, 0)) : 1.0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!inside_get(p, s))
		{
			if (g_xslot[p] == s + 1)
				extract_cancel(p);
			continue;
		}
		if (!extract_open_now(s))
		{
			if (!g_bot[p])
			{
				new Float:opens = g_pf[s][X_OPENAT] - rt;
				if (g_pf[s][X_OPENAT] >= 0.0 && opens > 0.0 && rt >= 0.0)
					hud_add(p, false, "Extraction closed (opens in %d:%02d)", floatround(opens) / 60, floatround(opens) % 60);
				else
					hud_add(p, false, "Extraction closed");
			}
			continue;
		}
		new Float:hold = g_pf[s][X_HOLD];
		if (g_xslot[p] != s + 1)
		{
			extract_cancel(p);
			g_xslot[p] = s + 1;
			g_xprog[p] = 0.0;
			g_xrate[p] = 0.0;
			status_icon(p, 1, "rescue", 0, 200, 60);
			fwd3f(g_fwExtracting, g_ent[s], p, hold);
		}
		if (g_xrate[p] != rate)
		{
			g_xrate[p] = rate;
			new pct = hold > 0.0 ? floatround(100.0 * g_xprog[p] / hold) : 100;
			bar_time2(p, floatround((hold - g_xprog[p]) / rate + 0.5), pct);
		}
		g_xprog[p] += TICK * rate;
		if (!g_bot[p])
			hud_add(p, false, "Extracting... %d s", floatround(floatmax(hold - g_xprog[p], 0.0) / rate, floatround_ceil));
		if (g_xprog[p] >= hold)
			extract_done(s, p);
	}
}

stock extract_cancel(p)
{
	if (!g_xslot[p])
		return;
	g_xslot[p] = 0;
	g_xprog[p] = 0.0;
	if (is_user_connected(p))
	{
		bar_time(p, 0);
		status_icon(p, 0, "rescue", 0, 0, 0);
	}
}

stock extract_done(s, p)
{
	new cost = floatround(g_pf[s][X_COST]);
	if (cost > 0)
	{
		new money = cs_get_user_money(p);
		if (money < cost)
		{
			if (!g_bot[p])
				client_print(p, print_center, "Extracting here costs $%d", cost);
			extract_cancel(p);
			g_xprog[p] = 0.0;
			return;
		}
		cs_set_user_money(p, money - cost);
	}
	extract_cancel(p);
	g_extracted[p] = true;
	inside_set(p, s, false);
	new ret;
	ExecuteForward(g_fwExtract, ret, g_ent[s], p);
	fire_targets(s, p);
	comp_debug("extract %d: player %d out", g_ent[s], p);
	if (ki(s, "once", 1))
		hint("goal_clear %d %d", p, g_ent[s]);
	if (ret >= PLUGIN_HANDLED)
		return;
	new name[32];
	get_user_name(p, name, charsmax(name));
	client_print(0, print_chat, "%s extracted.", name);
	new action[16];
	ks(s, "action", action, charsmax(action), "spectate");
	if (equali(action, "spectate"))
	{
		new frags = get_user_frags(p), deaths = cs_get_user_deaths(p);
		user_kill(p, 1);
		set_user_frags(p, frags);
		cs_set_user_deaths(p, deaths);
	}
	else if (equali(action, "strip"))
		strip_user_weapons(p);
}

/* ---- checkpoints ---- */

stock checkpoint_max()
{
	new m = 0;
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_CHECKPOINT && g_on[s])
			m = max(m, ki(s, "order", 1));
	return m;
}

stock tick_checkpoint(s)
{
	new order = ki(s, "order", 1);
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || (g_team[s] && g_pteam[p] != g_team[s]))
			continue;
		if (g_cpLast[p] + 1 != order || !inside(s, g_porg[p]))
			continue;
		if (order == 1)
			g_lapStart[p] = g_now;
		g_cpLast[p] = order;
		fwd3(g_fwCheckpoint, g_ent[s], p, order);
		fire_targets(s, p);
		new top = checkpoint_max();
		if (order >= top)
		{
			g_laps[p]++;
			new Float:secs = g_now - g_lapStart[p];
			fwd3f(g_fwLap, p, g_laps[p], secs);
			if (!g_bot[p])
				client_print(p, print_center, "Lap %d: %d:%05.2f", g_laps[p], floatround(secs, floatround_floor) / 60, secs - float((floatround(secs, floatround_floor) / 60) * 60));
			g_cpLast[p] = 0;
		}
		else if (!g_bot[p])
			client_cmd(p, "spk buttons/blip1");
		/* the bot's next checkpoint */
		if (g_bot[p])
		{
			new next = checkpoint_with_order(g_cpLast[p] + 1);
			if (next >= 0)
				hint("goal %d go_hold %.0f %.0f %.0f %d 60", p, g_org[next][0], g_org[next][1], g_org[next][2], g_ent[next]);
		}
	}
}

stock checkpoint_with_order(order)
{
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_CHECKPOINT && g_on[s] && ki(s, "order", 1) == order)
			return s;
	return -1;
}

stock lap_hud()
{
	/* shown once a second; the status line adds it each tick from here */
}

/* ---- moving platform ---- */

stock bool:platform_moving(s)
{
	new e = g_aux[s];
	if (e <= 0 || !pev_valid(e))
		return false;
	new Float:v[3], Float:av[3];
	pev(e, pev_velocity, v);
	pev(e, pev_avelocity, av);
	return vector_length(v) > 1.0 || vector_length(av) > 1.0;
}

stock tick_platform(s)
{
	new Float:rt = round_time();
	if (g_pf[s][PL_STARTED] == 0.0 && g_pf[s][PL_AUTO] >= 0.0 && rt >= g_pf[s][PL_AUTO])
	{
		g_pf[s][PL_STARTED] = 1.0;
		platform_go(s);
	}
	new bool:mv = platform_moving(s);
	if (mv != (g_state[s] != 0))
	{
		g_state[s] = mv ? 1 : 0;
		fwd2(g_fwPlatform, g_ent[s], mv ? 1 : 0);
		hint_comp(s);
	}
}

stock platform_go(s)
{
	new e = g_aux[s];
	if (e > 0 && pev_valid(e))
		ExecuteHamB(Ham_Use, e, g_ent[s], g_ent[s], 3, 0.0);
}

/* ---- meeting point ---- */

stock tick_meeting(s)
{
	if (g_meeting != s)
		return;
	new left = floatround(g_pf[s][M_END] - g_now, floatround_ceil);
	for (new p = 1; p <= MaxClients; p++)
		if (g_alive[p] && !g_bot[p])
			hud_add(p, false, "Meeting: %d s left. Vote in the menu (say /vote)", max(left, 0));
	if (g_pf[s][M_MENUAT] > 0.0 && g_now >= g_pf[s][M_MENUAT])
	{
		g_pf[s][M_MENUAT] = 0.0;
		for (new p = 1; p <= MaxClients; p++)
			if (g_alive[p] && !g_bot[p])
				vote_menu(p);
	}
	if (g_now >= g_pf[s][M_END])
		meeting_finish(true);
}

stock meeting_call(s, caller)
{
	if (g_meeting >= 0 || g_kind[s] != K_MEETING || !g_on[s])
		return 0;
	g_meeting = s;
	g_state[s] = 1;
	g_pf[s][M_END] = g_now + g_pf[s][M_DUR];
	g_pf[s][M_MENUAT] = g_now + 3.0;
	new freeze = ki(s, "freeze", 1), gather = ki(s, "gather", 1);
	new idx = 0, n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		g_vote[p] = -1;
		if (is_user_alive(p))
			n++;
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		act_cancel(p);
		if (gather)
		{
			new Float:a = 360.0 * float(idx++) / float(max(n, 1));
			new Float:o[3];
			o[0] = g_org[s][0] + g_r[s] * 0.6 * floatcos(a, degrees);
			o[1] = g_org[s][1] + g_r[s] * 0.6 * floatsin(a, degrees);
			o[2] = g_floor[s] + 37.0;
			engfunc(EngFunc_TraceHull, o, o, DONT_IGNORE_MONSTERS, HULL_HUMAN, p, 0);
			if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			{
				engfunc(EngFunc_SetOrigin, p, o);
				new Float:look[3];
				look[1] = a + 180.0;
				set_pev(p, pev_angles, look);
				set_pev(p, pev_v_angle, look);
				set_pev(p, pev_fixangle, 1);
			}
		}
		if (freeze)
			set_pev(p, pev_flags, pev(p, pev_flags) | FL_FROZEN);
	}
	new msg[96];
	if (1 <= caller <= MaxClients && is_user_connected(caller))
	{
		new name[32];
		get_user_name(caller, name, charsmax(name));
		formatex(msg, charsmax(msg), "Meeting called by %s", name);
		g_calls[caller]++;
	}
	else
		copy(msg, charsmax(msg), "Meeting!");
	announce(0, 255, 220, 120, 4.0, msg);
	client_cmd(0, "spk buttons/bell1");
	fwd2(g_fwMeetStart, g_ent[s], caller);
	hint_comp(s);
	return 1;
}

stock meeting_finish(bool:count)
{
	new s = g_meeting;
	if (s < 0)
		return;
	g_meeting = -1;
	g_state[s] = 0;
	g_pf[s][M_READY] = g_now + g_pf[s][M_COOLDOWN];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (is_user_connected(p))
		{
			set_pev(p, pev_flags, pev(p, pev_flags) & ~FL_FROZEN);
			if (!is_user_bot(p))
				show_menu(p, 0, " ", 1);
		}
	}
	new ejected = 0;
	if (count)
	{
		new votes[33], skip = 0;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_connected(p) || g_vote[p] < 0)
				continue;
			if (g_vote[p] == 0)
				skip++;
			else if (g_vote[p] <= MaxClients)
				votes[g_vote[p]]++;
		}
		new best = 0, bestn = 0, tie = 0;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (votes[p] > bestn)
			{
				best = p;
				bestn = votes[p];
				tie = 0;
			}
			else if (votes[p] == bestn && bestn > 0)
				tie = 1;
		}
		if (best && !tie && bestn > skip && is_user_alive(best))
		{
			ejected = best;
			new name[32], msg[80];
			get_user_name(best, name, charsmax(name));
			formatex(msg, charsmax(msg), "%s was voted out (%d votes)", name, bestn);
			announce(0, 255, 120, 80, 4.0, msg);
			user_kill(best, 1);
		}
		else
			announce(0, 200, 200, 200, 3.0, "Nobody was voted out");
	}
	for (new p = 1; p <= MaxClients; p++)
		g_vote[p] = -1;
	fwd2(g_fwMeetEnd, g_ent[s], ejected);
	if (count)
		fire_targets(s, ejected);
	hint_comp(s);
}

stock vote_menu(id)
{
	if (g_meeting < 0 || !is_user_alive(id))
		return;
	new m = menu_create("\yVote: who goes?", "menu_vote");
	menu_additem(m, "Skip", "0");
	new name[32], info[4];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		get_user_name(p, name, charsmax(name));
		num_to_str(p, info, charsmax(info));
		menu_additem(m, name, info);
	}
	menu_display(id, m);
}

public menu_vote(id, menu, item)
{
	if (item < 0 || g_meeting < 0)
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[4], name[32], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	menu_destroy(menu);
	meeting_vote(id, str_to_num(info));
	return PLUGIN_HANDLED;
}

stock meeting_vote(voter, target)
{
	if (g_meeting < 0 || !(1 <= voter <= MaxClients) || !is_user_alive(voter))
		return 0;
	if (target < 0 || target > MaxClients || (target && !is_user_alive(target)))
		return 0;
	g_vote[voter] = target;
	fwd3(g_fwMeetVote, g_ent[g_meeting], voter, target);
	new name[32];
	get_user_name(voter, name, charsmax(name));
	client_print(0, print_chat, "%s voted.", name);
	return 1;
}

public cmd_vote_menu(id)
{
	if (g_meeting < 0)
		return PLUGIN_CONTINUE;
	vote_menu(id);
	return PLUGIN_HANDLED;
}

/* =========================================================================
 * Hold-USE actions: open, repair, lock, call a meeting
 * ========================================================================= */

public fm_prethink(id)
{
	if (!is_user_alive(id) || g_act[id])
		return FMRES_IGNORED;
	new b = pev(id, pev_button), ob = pev(id, pev_oldbuttons);
	if ((b & IN_USE) && !(ob & IN_USE))
		on_use(id);
	return FMRES_IGNORED;
}

/* what the player could use from here: returns slot or -1, and the action */
stock usable_near(id, &type, &Float:secs)
{
	new Float:eye[3], Float:fwd[3], Float:ang[3], Float:po[3];
	pev(id, pev_origin, po);
	pev(id, pev_view_ofs, eye);
	eye[0] += po[0]; eye[1] += po[1]; eye[2] += po[2];
	pev(id, pev_v_angle, ang);
	engfunc(EngFunc_MakeVectors, ang);
	global_get(glb_v_forward, fwd);
	new best = -1, Float:bestd = 99999.0;
	new team = get_user_team(id);
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s])
			continue;
		new t = ACT_NONE, Float:range = 80.0, Float:c[3];
		vcopy(g_org[s], c);
		new Float:sc = 0.0;
		switch (g_kind[s])
		{
			case K_LOOT:
				if (g_state[s] == 0) { t = ACT_LOOT; sc = g_pf[s][W_OPEN]; c[2] = g_floor[s] + 12.0; }
			case K_CRATE:
				if (g_state[s] == CRATE_FULL) { t = ACT_LOOT; sc = g_pf[s][W_OPEN]; c[2] = g_floor[s] + 12.0; }
			case K_BARRICADE:
			{
				if ((!g_team[s] || team == g_team[s]) && g_val[s] < g_pf[s][B_MAX] - 0.5)
				{
					t = ACT_REPAIR;
					sc = g_pf[s][B_REPAIR];
					range = 72.0;
					for (new i = 0; i < 3; i++)
						c[i] = floatclamp(eye[i], g_mins[s][i], g_maxs[s][i]);
				}
			}
			case K_DOOR:
			{
				if (ki(s, "lockable", 0) && (!g_team[s] || team == g_team[s]) && !door_locked(s) && g_ndoors[s])
				{
					t = ACT_LOCK;
					sc = kf(s, "lock_time", 3.0);
					range = 100.0;
					new Float:mn[3], Float:mx[3];
					pev(g_doors[s][0], pev_absmin, mn);
					pev(g_doors[s][0], pev_absmax, mx);
					for (new i = 0; i < 3; i++)
						c[i] = floatclamp(eye[i], mn[i], mx[i]);
				}
			}
			case K_MEETING:
			{
				new maxcalls = ki(s, "calls", 1);
				if (g_meeting < 0 && g_now >= g_pf[s][M_READY] && ki(s, "callers", 0) == 0
					&& (maxcalls <= 0 || g_calls[id] < maxcalls) && inside(s, po))
				{
					t = ACT_MEET;
					sc = 1.0;
					range = 99999.0;
					vcopy(po, c);
				}
			}
		}
		if (t == ACT_NONE)
			continue;
		new Float:d = get_distance_f(eye, c);
		if (d > range || d > bestd)
			continue;
		if (t != ACT_MEET && d > 24.0)
		{
			/* roughly facing it */
			new Float:to[3];
			for (new i = 0; i < 3; i++)
				to[i] = (c[i] - eye[i]) / d;
			if (to[0] * fwd[0] + to[1] * fwd[1] + to[2] * fwd[2] < 0.5)
				continue;
		}
		best = s;
		bestd = d;
		type = t;
		secs = sc;
	}
	return best;
}

stock on_use(id)
{
	new type, Float:secs;
	new s = usable_near(id, type, secs);
	if (s < 0)
		return;
	if (type == ACT_REPAIR && g_pf[s][B_COST] > 0.0 && cs_get_user_money(id) < floatround(g_pf[s][B_COST]))
	{
		client_print(id, print_center, "Repairing costs $%d", floatround(g_pf[s][B_COST]));
		return;
	}
	act_start(id, s, type, secs);
}

stock act_start(id, s, type, Float:secs)
{
	act_cancel(id);
	if (g_xslot[id])
		extract_cancel(id);
	g_act[id] = s + 1;
	g_actType[id] = type;
	g_actEnd[id] = g_now + secs;
	if (secs >= 0.5)
		bar_time(id, floatround(secs, floatround_ceil));
}

stock act_cancel(id)
{
	if (!g_act[id])
		return;
	g_act[id] = 0;
	g_actType[id] = ACT_NONE;
	if (is_user_connected(id))
		bar_time(id, 0);
}

stock tick_acts()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_act[p])
			continue;
		new s = g_act[p] - 1;
		if (!g_alive[p] || !g_ent[s] || !g_on[s])
		{
			act_cancel(p);
			continue;
		}
		/* people hold USE; bots just stay close */
		if (!g_bot[p] && !(pev(p, pev_button) & IN_USE))
		{
			act_cancel(p);
			continue;
		}
		new type, Float:secs;
		if (!g_bot[p])
		{
			if (usable_near(p, type, secs) != s)
			{
				act_cancel(p);
				continue;
			}
		}
		else if (get_distance_f(g_porg[p], g_org[s]) > 96.0)
		{
			act_cancel(p);
			continue;
		}
		if (g_now < g_actEnd[p])
			continue;
		new t = g_actType[p];
		act_cancel(p);
		switch (t)
		{
			case ACT_LOOT:
			{
				if ((g_kind[s] == K_LOOT && g_state[s] == 0) || (g_kind[s] == K_CRATE && g_state[s] == CRATE_FULL))
					loot_give(s, p);
			}
			case ACT_REPAIR:
			{
				new cost = floatround(g_pf[s][B_COST]);
				if (cost > 0)
				{
					new money = cs_get_user_money(p);
					if (money < cost)
						continue;
					cs_set_user_money(p, money - cost);
				}
				barricade_repair(s, p);
				/* still holding USE: on to the next board */
				if (!g_bot[p] && (pev(p, pev_button) & IN_USE) && g_val[s] < g_pf[s][B_MAX] - 0.5)
					act_start(p, s, ACT_REPAIR, g_pf[s][B_REPAIR]);
			}
			case ACT_LOCK:
				door_lock(s, kf(s, "lock_secs", 10.0), p);
			case ACT_MEET:
				meeting_call(s, p);
		}
	}
}

/* "Hold USE to ..." for people near something usable */
stock use_hints()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p] || g_act[p] || g_meeting >= 0)
			continue;
		new type, Float:secs;
		new s = usable_near(p, type, secs);
		if (s < 0)
			continue;
		switch (type)
		{
			case ACT_LOOT: hud_add(p, false, "Hold USE to open");
			case ACT_REPAIR:
			{
				if (g_pf[s][B_COST] > 0.0)
					hud_add(p, false, "Hold USE to repair ($%d)", floatround(g_pf[s][B_COST]));
				else
					hud_add(p, false, "Hold USE to repair (%d%%)", floatround(100.0 * g_val[s] / g_pf[s][B_MAX]));
			}
			case ACT_LOCK: hud_add(p, false, "Hold USE to lock the door");
			case ACT_MEET: hud_add(p, false, "Hold USE to call a meeting");
		}
	}
	/* checkpoint progress */
	new top = -1;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p] || g_cpLast[p] <= 0)
			continue;
		if (top < 0)
			top = checkpoint_max();
		new Float:t = g_now - g_lapStart[p];
		new mins = floatround(t, floatround_floor) / 60;
		hud_add(p, false, "Checkpoint %d/%d   %d:%04.1f", g_cpLast[p], top, mins, t - float(mins * 60));
	}
}

/* =========================================================================
 * Players: spawns, classes, protection
 * ========================================================================= */

/* A creature's melee is a reach check, not a trace: through an intact gate it still landed (QA:
   14 deaths 20-50 u behind solid gates). Its hit is refused when a standing barricade lies on the
   line from the creature to the player. (Spit is a projectile and the gate stops it itself.) */
public ham_player_hurt(victim, inflictor, attacker, Float:dmg, bits)
{
	if (!attacker || attacker != inflictor || attacker <= MaxClients || !pev_valid(attacker))
		return HAM_IGNORED;
	new cls[16];
	pev(attacker, pev_classname, cls, charsmax(cls));
	if (!equal(cls, "ls_creature"))
		return HAM_IGNORED;
	new Float:a[3], Float:b[3];
	pev(attacker, pev_origin, a);
	pev(victim, pev_origin, b);
	/* (the creature's middle: its origin can be at its feet) */
	new Float:amn[3], Float:amx[3];
	pev(attacker, pev_absmin, amn);
	pev(attacker, pev_absmax, amx);
	a[2] = (amn[2] + amx[2]) * 0.5;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_BARRICADE || g_state[s] != 0 || pev(g_ent[s], pev_solid) == SOLID_NOT)
			continue;
		new Float:mn[3], Float:mx[3];
		pev(g_ent[s], pev_absmin, mn);
		pev(g_ent[s], pev_absmax, mx);
		if (segment_hits_box(a, b, mn, mx))
		{
			g_gateBlocks++;
			return HAM_SUPERCEDE;
		}
	}
	return HAM_IGNORED;
}

stock bool:segment_hits_box(const Float:a[3], const Float:b[3], const Float:mn[3], const Float:mx[3])
{
	new Float:t0 = 0.0, Float:t1 = 1.0;
	for (new i = 0; i < 3; i++)
	{
		new Float:d = b[i] - a[i];
		if (floatabs(d) < 0.001)
		{
			if (a[i] < mn[i] || a[i] > mx[i])
				return false;
			continue;
		}
		new Float:u = (mn[i] - a[i]) / d, Float:v = (mx[i] - a[i]) / d;
		if (u > v)
		{
			new Float:t = u;
			u = v;
			v = t;
		}
		t0 = floatmax(t0, u);
		t1 = floatmin(t1, v);
		if (t0 > t1)
			return false;
	}
	return true;
}

public ham_player_spawn(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED;
	remove_task(TASK_SPAWN + id);
	set_task(0.1, "task_spawned", TASK_SPAWN + id);
	return HAM_IGNORED;
}

public ham_player_killed(id, killer, gib)
{
	act_cancel(id);
	extract_cancel(id);
	return HAM_IGNORED;
}

public task_spawned(tid)
{
	new id = tid - TASK_SPAWN;
	if (!is_user_alive(id))
		return;
	g_now = get_gametime();
	class_apply(id);
	if (g_waveSpawn[id])
	{
		g_waveSpawn[id] = false;
		engfunc(EngFunc_SetOrigin, id, g_waveOrg[id]);
		set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0});
		return;
	}
	if (!get_pcvar_num(g_cSpawns))
		return;
	new s = respawn_pick(id);
	if (s < 0)
		return;
	respawn_place(id, s);
}

stock respawn_pick(id)
{
	new team = get_user_team(id);
	new cand[MAX_COMP], n = 0;
	new bool:far = get_pcvar_num(g_cSpawnFar) != 0;
	new best = -1, Float:bestd = -1.0;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_RESPAWN || !g_on[s])
			continue;
		if (g_team[s] && g_team[s] != team)
			continue;
		new Float:nearest = 99999.0;
		new bool:taken = false;
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == id || !is_user_alive(p))
				continue;
			new Float:o[3];
			pev(p, pev_origin, o);
			new Float:d = get_distance_f(o, g_org[s]);
			if (d < 64.0)
				taken = true;
			if (d < nearest)
				nearest = d;
		}
		if (taken)
			continue;
		cand[n++] = s;
		if (nearest > bestd)
		{
			bestd = nearest;
			best = s;
		}
	}
	if (!n)
		return -1;
	return far ? best : cand[random(n)];
}

stock respawn_place(id, s)
{
	new ret;
	ExecuteForward(g_fwRespawn, ret, g_ent[s], id);
	if (ret >= PLUGIN_HANDLED)
		return;
	new Float:o[3], Float:ang[3];
	vcopy(g_org[s], o);
	o[2] = g_floor[s] + 37.0;
	pev(g_ent[s], pev_angles, ang);
	engfunc(EngFunc_SetOrigin, id, o);
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0});
	ang[0] = 0.0;
	ang[2] = 0.0;
	set_pev(id, pev_angles, ang);
	set_pev(id, pev_v_angle, ang);
	set_pev(id, pev_fixangle, 1);
	new Float:prot = kf(s, "protect_secs", 3.0);
	if (prot > 0.0)
	{
		g_protect[id] = get_gametime() + prot;
		set_pev(id, pev_takedamage, DAMAGE_NO);
		render(id, kRenderFxGlowShell, get_user_team(id) == 1 ? 200 : 60, 120, get_user_team(id) == 2 ? 200 : 60, kRenderNormal, 12);
	}
}

stock class_apply(id)
{
	new s = g_class[id] - 1;
	if (s < 0 || !g_ent[s])
	{
		/* a class with default=1 for this team */
		s = -1;
		new team = get_user_team(id);
		for (new c = 0; c < g_nslots; c++)
		{
			if (g_ent[c] && g_kind[c] == K_CLASS && g_on[c] && ki(c, "default", 0) && (!g_team[c] || g_team[c] == team))
			{
				s = c;
				break;
			}
		}
		if (s < 0)
			return;
		g_class[id] = s + 1;
	}
	if (ki(s, "strip", 1))
		strip_user_weapons(id);
	new weapons[192], w[32], pos = 0;
	ks(s, "weapons", weapons, charsmax(weapons), "knife");
	while (weapons[pos])
	{
		pos += next_token(weapons[pos], w, charsmax(w));
		if (!w[0])
			break;
		give_thing(id, w);
	}
	new hp = ki(s, "hp", 100);
	set_pev(id, pev_max_health, float(max(hp, 1)));
	set_user_health(id, max(hp, 1));
	new armor = ki(s, "armor", 0);
	if (armor > 0)
		cs_set_user_armor(id, armor, ki(s, "helmet", 1) ? CS_ARMOR_VESTHELM : CS_ARMOR_KEVLAR);
	set_user_gravity(id, kf(s, "gravity", 1.0));
	new model[32];
	if (ks(s, "model", model, charsmax(model)))
		cs_set_user_model(id, model);
	else
		cs_reset_user_model(id);
	if (kf(s, "speed", 0.0) > 0.0)
		ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, id);
	new prof[32];
	if (is_user_bot(id) && ks(s, "bot_profile", prof, charsmax(prof)))
		hint("profile %d %s", id, prof);
}

public ham_maxspeed(id)
{
	if (!is_user_alive(id) || !g_class[id])
		return HAM_IGNORED;
	new s = g_class[id] - 1;
	if (!g_ent[s])
		return HAM_IGNORED;
	new Float:sp = kf(s, "speed", 0.0);
	if (sp > 0.0 && !(pev(id, pev_flags) & FL_FROZEN))
		set_user_maxspeed(id, sp);
	return HAM_IGNORED;
}

stock class_count(s)
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && g_class[p] == s + 1)
			n++;
	return n;
}

stock class_set(id, s)
{
	if (s < 0)
	{
		g_class[id] = 0;
		return 1;
	}
	new name[32];
	copy(name, charsmax(name), g_label[s]);
	new limit = ki(s, "limit", 0);
	if (limit > 0 && g_class[id] != s + 1 && class_count(s) >= limit)
		return 0;
	new ret;
	ExecuteForward(g_fwClass, ret, id, name);
	if (ret >= PLUGIN_HANDLED)
		return 0;
	g_class[id] = s + 1;
	new Float:rt = round_time();
	if (is_user_alive(id) && (rt < 0.0 || rt < 15.0))
		class_apply(id);
	return 1;
}

stock class_by_name(const name[])
{
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_CLASS && equali(g_label[s], name))
			return s;
	return -1;
}

public cmd_class_menu(id)
{
	if (!count_comps(K_CLASS))
		return PLUGIN_CONTINUE;
	class_menu(id);
	return PLUGIN_HANDLED;
}

stock class_menu(id)
{
	new team = get_user_team(id);
	new m = menu_create("\yChoose a class", "menu_class");
	new info[8], text[64], n = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_CLASS || !g_on[s] || !ki(s, "menu", 1))
			continue;
		if (g_team[s] && g_team[s] != team)
			continue;
		new limit = ki(s, "limit", 0);
		if (limit > 0)
			formatex(text, charsmax(text), "%s \d(%d/%d)", g_label[s], class_count(s), limit);
		else
			copy(text, charsmax(text), g_label[s]);
		num_to_str(s, info, charsmax(info));
		menu_additem(m, text, info);
		n++;
	}
	if (!n)
	{
		menu_destroy(m);
		client_print(id, print_chat, "No classes for your team here.");
		return;
	}
	menu_display(id, m);
}

public menu_class(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[8], name[64], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	menu_destroy(menu);
	new s = str_to_num(info);
	if (s >= 0 && s < g_nslots && g_ent[s] && g_kind[s] == K_CLASS)
	{
		if (class_set(id, s))
			client_print(id, print_chat, "You're %s%s.", g_label[s], is_user_alive(id) && round_time() >= 15.0 ? " from your next spawn" : "");
		else
			client_print(id, print_chat, "You can't be %s right now.", g_label[s]);
	}
	return PLUGIN_HANDLED;
}

public client_disconnected(id)
{
	up_disconnected(id); /* W1 */
	g_tqN[id] = 0;
	g_tqShown[id][0] = EOS;
	g_tqUntil[id] = 0.0;
	g_act[id] = 0;
	g_xslot[id] = 0;
	g_class[id] = 0;
	g_cpLast[id] = 0;
	g_laps[id] = 0;
	g_extracted[id] = false;
	g_protect[id] = 0.0;
	g_vote[id] = -1;
	g_calls[id] = 0;
	g_waveSpawn[id] = false;
	g_hudShown[id][0] = EOS;
	remove_task(TASK_SPAWN + id);
}

/* =========================================================================
 * Map logic can set components off: a button whose target is a component
 * ========================================================================= */

public ham_use(ent, caller, activator, type, Float:value)
{
	new s = slot_of(ent);
	if (s < 0)
		return HAM_IGNORED;
	comp_use(s, activator);
	return HAM_SUPERCEDE;
}

stock comp_use(s, activator)
{
	switch (g_kind[s])
	{
		case K_DAMAGE, K_ZONE, K_RESPAWN, K_CAPTURE, K_CHECKPOINT:
		{
			g_on[s] = !g_on[s];
			hint_comp(s);
		}
		case K_EXTRACT: extract_set(s, g_state[s] != 1);
		case K_DOOR: door_use(s, !door_is_open(s));
		case K_TRAP: trap_fire(s, activator);
		default: comp_start(s, activator);
	}
}

stock comp_start(s, activator)
{
	if (!g_on[s])
		return 0;
	switch (g_kind[s])
	{
		case K_SHRINKER:
		{
			g_state[s] = 1;
			g_pf[s][S_PHASE] = 0.0;
			g_pf[s][S_NEXT] = g_now;
		}
		case K_WAVE: wave_start(0);
		case K_DROP: return drop_from_marker(s) != 0;
		case K_PLATFORM: platform_go(s);
		case K_MEETING: return meeting_call(s, activator);
		case K_TRAP: return trap_fire(s, activator);
		case K_EXTRACT: extract_set(s, true);
		case K_DOOR: door_use(s, true);
		case K_WEAPON:
		{
			spawner_roll(s);
			spawner_show(s, true);
		}
		case K_LOOT: loot_look(s, true);
		case K_BARRICADE: barricade_repair(s, 0);
		default: return 0;
	}
	return 1;
}

stock comp_stop(s)
{
	switch (g_kind[s])
	{
		case K_SHRINKER: g_state[s] = 0;
		case K_WAVE: wave_stop();
		case K_MEETING: if (g_meeting == s) meeting_finish(false);
		case K_EXTRACT: extract_set(s, false);
		case K_DOOR: door_use(s, false);
		case K_TRAP: g_state[s] = TRAP_OFF;
		case K_ZONE:
		{
			new Float:c[3];
			vcopy(g_va[s], c);
			zone_set(s, c, zone_radius_now(s));
		}
		default: return 0;
	}
	return 1;
}

/* =========================================================================
 * Bots: ls_mode hints (docs/TOOLS-NOTES.md, W1/W2 interface)
 * ========================================================================= */

/* the brain keeps 32 avoid/attract tags at most: loot hints stop at 26, leaving room for the rest */

stock bool:attract_ok(s)
{
	if (g_attractOn[s])
		return true;
	if (g_nattract >= 26)
		return false;
	g_attractOn[s] = true;
	g_nattract++;
	return true;
}

stock attract_off(s)
{
	if (g_attractOn[s])
	{
		g_attractOn[s] = false;
		g_nattract--;
	}
}

stock hint(const fmt[], any:...)
{
	if (!get_pcvar_num(g_cBots))
		return 0;
	new line[192];
	vformat(line, charsmax(line), fmt, 2);
	if (get_pcvar_num(g_cDebug))
		log_amx("ls_mode %s%s", line, g_brain ? "" : " (not sent: no brain)");
	if (!g_brain)
		return 0;
	server_cmd("ls_mode %s", line);
	return 1;
}

/* without ls_mode: some of the bots walk there with the brain's older ls_bot goto */
stock fallback_goto(team, const Float:p[3])
{
	if (g_brain || !g_lsBot || !get_pcvar_num(g_cBots) || !get_pcvar_num(g_cFallback))
		return;
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b) || !is_user_alive(b))
			continue;
		if (team && get_user_team(b) != team)
			continue;
		if (random_num(0, 2))
			continue;
		server_cmd("ls_bot %d goto %.0f %.0f %.0f", b, p[0], p[1], p[2]);
	}
}

/* what this component means to bots, now */
stock hint_comp(s)
{
	if (!g_ready || g_quiet || !g_ent[s])
		return;
	new ent = g_ent[s];
	new Float:o[3];
	vcopy(g_org[s], o);
	if (!g_on[s])
	{
		hint_forget(s);
		return;
	}
	new who[4], other[4];
	switch (g_kind[s])
	{
		case K_CAPTURE:
		{
			new owner = g_side[s];
			if (g_pf[s][CP_FFA] > 0.0 || owner == 0 || owner > 2)
			{
				hint("goal all capture %.0f %.0f %.0f %d 60 0 %.0f", o[0], o[1], o[2], ent, zone_reach(s));
				hint("attract c%d %.0f %.0f %.0f %.0f 60 0 0", ent, o[0], o[1], o[2], g_r[s]);
				fallback_goto(0, o);
			}
			else
			{
				team_word(owner, who, charsmax(who));
				team_word(3 - owner, other, charsmax(other));
				hint("goal %s defend %.0f %.0f %.0f %d 40 0 %.0f", who, o[0], o[1], o[2], ent, zone_reach(s));
				hint("goal %s capture %.0f %.0f %.0f %d 60 0 %.0f", other, o[0], o[1], o[2], ent, zone_reach(s));
				hint("attract c%d %.0f %.0f %.0f %.0f 60 0 %d", ent, o[0], o[1], o[2], g_r[s], 3 - owner);
				fallback_goto(3 - owner, o);
			}
		}
		case K_ZONE:
		{
			hint("zone %.0f %.0f %.0f %.0f %.0f %.0f %.0f %.0f %.0f", g_va[s][0], g_va[s][1], o[2], zone_radius_now(s),
				g_vb[s][0], g_vb[s][1], o[2], g_pf[s][Z_TOR], floatmax(g_pf[s][Z_T0] - g_now, 0.0));
		}
		case K_DAMAGE:
			hint("avoid c%d %.0f %.0f %.0f %.0f %d 0 %d", ent, o[0], o[1], o[2], zone_reach(s), avoid_cost(s), g_team[s]);
		case K_TRAP:
		{
			if (g_state[s] == TRAP_ARMED && (g_pf[s][T_DMG] > 0.0 || ki(s, "danger", 0)))
				hint("avoid c%d %.0f %.0f %.0f %.0f 6 0 0", ent, o[0], o[1], o[2], zone_reach(s));
			else
				hint("forget c%d", ent);
		}
		case K_DOOR:
		{
			new bool:block = door_locked(s) && !door_is_open(s);
			for (new d = 0; d < g_ndoors[s]; d++)
				hint("block %d %d", g_doors[s][d], block ? -1 : 0);
		}
		case K_WEAPON:
		{
			if (g_state[s] == 0 && attract_ok(s))
				hint("attract c%d %.0f %.0f %.0f 400 %d 0 0", ent, o[0], o[1], o[2], 20 + floatround(g_pf[s][W_TIER]) * 15);
			else if (g_state[s] != 0)
			{
				attract_off(s);
				hint("forget c%d", ent);
			}
		}
		case K_LOOT:
		{
			if (g_state[s] == 0 && attract_ok(s))
				hint("attract c%d %.0f %.0f %.0f 500 %d 0 0", ent, o[0], o[1], o[2], 20 + floatround(g_pf[s][W_TIER]) * 20);
			else if (g_state[s] != 0)
			{
				attract_off(s);
				hint("forget c%d", ent);
			}
		}
		case K_CRATE:
		{
			/* (a supply crate always gets one: it's worth more than any) */
			if (g_state[s] == CRATE_FULL)
			{
				if (!g_attractOn[s])
				{
					g_attractOn[s] = true;
					g_nattract++;
				}
				hint("attract c%d %.0f %.0f %.0f 1500 90 0 0", ent, o[0], o[1], o[2]);
			}
			else if (g_state[s] == CRATE_EMPTY)
			{
				attract_off(s);
				hint("forget c%d", ent);
			}
		}
		case K_BARRICADE:
		{
			team_word(g_team[s], who, charsmax(who));
			if (g_state[s] == 0)
			{
				if (ki(s, "blocks_bots", 1))
					hint("block %d -1", ent);
				/* a goal for every bot of the team piles them all on one spot: only when asked
				   (a mode usually spreads its own defenders) */
				if (g_team[s] && ki(s, "defend_goal", 0))
					hint("goal %s defend %.0f %.0f %.0f %d 30 0 300", who, o[0], o[1], o[2], ent);
			}
			else
			{
				hint("block %d 0", ent);
				if (g_team[s])
					hint("goal_clear %s %d", who, ent);
			}
		}
		case K_WAVE:
			hint("avoid c%d %.0f %.0f %.0f %.0f 3 0 %d", ent, o[0], o[1], o[2], floatmax(g_r[s], 256.0), g_team[s] == 1 ? 2 : 1);
		case K_EXTRACT:
		{
			team_word(g_team[s], who, charsmax(who));
			if (g_state[s] == 1)
			{
				hint("goal %s extract %.0f %.0f %.0f %d 50 0 %.0f", who, o[0], o[1], o[2], ent, zone_reach(s));
				fallback_goto(g_team[s], o);
			}
			else
				hint("goal_clear %s %d", who, ent);
		}
		case K_PLATFORM:
		{
			new e = g_aux[s];
			if (e > 0 && pev_valid(e))
			{
				new Float:mn[3], Float:mx[3];
				pev(e, pev_absmin, mn);
				pev(e, pev_absmax, mx);
				hint("platform c%d %d %d %.0f %.0f %.0f", ent, e, g_state[s], (mn[0] + mx[0]) * 0.5, (mn[1] + mx[1]) * 0.5, mx[2]);
			}
		}
		case K_MEETING:
		{
			if (g_meeting == s)
			{
				hint("goal all meeting %.0f %.0f %.0f %d 90 %d %.0f", o[0], o[1], o[2], ent, floatround(g_pf[s][M_END] - g_now), g_r[s]);
				fallback_goto(0, o);
			}
			else
				hint("goal_clear all %d", ent);
		}
	}
}

/* avoid cost for a damage zone, on the brain's scale: 1 mild .. 10 strong, -1 never walk through */
stock avoid_cost(s)
{
	new Float:dps = g_pf[s][D_DMG] / g_pf[s][D_INTERVAL];
	if (dps >= 50.0)
		return -1;
	return clamp(1 + floatround(dps / 5.0), 1, 10);
}

stock Float:zone_reach(s)
{
	if (!g_box[s])
		return g_r[s];
	return floatmax(g_maxs[s][0] - g_mins[s][0], g_maxs[s][1] - g_mins[s][1]) * 0.5;
}

/* the bots forget this component */
stock hint_forget(s)
{
	if (!g_ready || !g_ent[s])
		return;
	new ent = g_ent[s];
	attract_off(s);
	switch (g_kind[s])
	{
		case K_CAPTURE, K_EXTRACT, K_MEETING:
		{
			hint("goal_clear all %d", ent);
			hint("forget c%d", ent);
		}
		case K_BARRICADE:
		{
			hint("block %d 0", ent);
			hint("goal_clear all %d", ent);
		}
		case K_DOOR:
			for (new d = 0; d < g_ndoors[s]; d++)
				hint("block %d 0", g_doors[s][d]);
		case K_ZONE:
			hint("zone_clear");
		case K_DAMAGE, K_TRAP, K_WEAPON, K_LOOT, K_CRATE, K_WAVE, K_PLATFORM:
			hint("forget c%d", ent);
	}
}

/* =========================================================================
 * Client feedback: status line, director HUD, beams, bars, icons
 * ========================================================================= */

stock hud_add(id, bool:warn, const fmt[], any:...)
{
	new line[96];
	vformat(line, charsmax(line), fmt, 4);
	if (warn)
		g_hudWarn[id] = true;
	if (g_hud[id][0])
	{
		/* two lines at most; a warning goes first */
		if (contain(g_hud[id], "^n") >= 0)
			return;
		if (warn)
		{
			new old[192];
			copy(old, charsmax(old), g_hud[id]);
			formatex(g_hud[id], charsmax(g_hud[]), "%s^n%s", line, old);
			return;
		}
		add(g_hud[id], charsmax(g_hud[]), "^n");
	}
	add(g_hud[id], charsmax(g_hud[]), line);
}

stock hud_flush()
{
	if (!get_pcvar_num(g_cHud))
		return;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || g_bot[p])
			continue;
		new bool:same = equal(g_hud[p], g_hudShown[p]) != 0;
		if (same && (!g_hud[p][0] || g_now - g_hudAt[p] < 0.9))
			continue;
		if (!g_hud[p][0])
		{
			ClearSyncHud(p, g_sync);
			g_hudShown[p][0] = EOS;
			continue;
		}
		copy(g_hudShown[p], charsmax(g_hudShown[]), g_hud[p]);
		g_hudAt[p] = g_now;
		if (g_hudWarn[p])
			set_hudmessage(255, 80, 60, -1.0, 0.70, 0, 0.0, 1.1, 0.0, 0.1, -1);
		else
			set_hudmessage(220, 230, 220, -1.0, 0.70, 0, 0.0, 1.1, 0.0, 0.1, -1);
		ShowSyncHudMsg(p, g_sync, "%s", g_hud[p]);
	}
}

stock announce(id, r, g, b, Float:secs, const text[])
{
	if (!get_pcvar_num(g_cHud))
		return;
	toast_add(id, r, g, b, secs, text, false);
}

/* ---- the toast line: every big announcement (this plugin's and the modes', ls_toast) goes
   through one queue per player and shows one at a time at one spot, so two never overlap ---- */

stock toast_add(id, r, g, b, Float:secs, const text[], bool:replace)
{
	if (!id)
	{
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && !is_user_bot(p))
				toast_add(p, r, g, b, secs, text, replace);
		return;
	}
	if (id < 1 || id > 32)
		return;
	/* the same text again (a countdown's repeat) or a replace: shown now, in place */
	if (replace || equal(g_tqShown[id], text))
	{
		toast_show(id, (r << 16) | (g << 8) | b, secs, text);
		return;
	}
	for (new i = 0; i < g_tqN[id]; i++)
		if (equal(g_tqText[id][i], text))
			return;
	if (g_tqN[id] >= TOAST_Q)
	{
		/* full: the oldest waiting one goes */
		for (new i = 1; i < TOAST_Q; i++)
		{
			copy(g_tqText[id][i - 1], 127, g_tqText[id][i]);
			g_tqRgb[id][i - 1] = g_tqRgb[id][i];
			g_tqSecs[id][i - 1] = g_tqSecs[id][i];
		}
		g_tqN[id]--;
	}
	new n = g_tqN[id]++;
	copy(g_tqText[id][n], 127, text);
	g_tqRgb[id][n] = (r << 16) | (g << 8) | b;
	g_tqSecs[id][n] = secs;
	toast_tick(id);
}

stock toast_show(id, rgb, Float:secs, const text[])
{
	set_dhudmessage((rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255, -1.0, 0.22, 0, 0.0, secs, 0.1, 0.3);
	show_dhudmessage(id, "%s", text);
	copy(g_tqShown[id], 127, text);
	g_tqUntil[id] = get_gametime() + secs + 0.4;
}

stock toast_tick(id)
{
	if (!g_tqN[id] || get_gametime() < g_tqUntil[id])
		return;
	if (!is_user_connected(id))
	{
		g_tqN[id] = 0;
		return;
	}
	toast_show(id, g_tqRgb[id][0], g_tqSecs[id][0], g_tqText[id][0]);
	for (new i = 1; i < g_tqN[id]; i++)
	{
		copy(g_tqText[id][i - 1], 127, g_tqText[id][i]);
		g_tqRgb[id][i - 1] = g_tqRgb[id][i];
		g_tqSecs[id][i - 1] = g_tqSecs[id][i];
	}
	g_tqN[id]--;
}

/* native ls_toast(id, Float:secs, const text[], r = 255, g = 200, b = 60, replace = 0) */
public n_toast(plugin, params)
{
	new text[128];
	get_string(3, text, charsmax(text));
	toast_add(get_param(1), get_param(4), get_param(5), get_param(6), get_param_f(2), text, get_param(7) != 0);
	return 1;
}

stock bar_time(id, secs)
{
	if (!g_msgBarTime || !is_user_connected(id) || is_user_bot(id))
		return;
	message_begin(MSG_ONE, g_msgBarTime, _, id);
	write_short(secs);
	message_end();
}

stock bar_time2(id, secs, pct)
{
	if (!g_msgBarTime2 || !is_user_connected(id) || is_user_bot(id))
		return;
	message_begin(MSG_ONE, g_msgBarTime2, _, id);
	write_short(secs);
	write_short(clamp(pct, 0, 99));
	message_end();
}

stock status_icon(id, status, const sprite[], r, g, b)
{
	if (!g_msgStatusIcon || !is_user_connected(id) || is_user_bot(id))
		return;
	message_begin(MSG_ONE, g_msgStatusIcon, _, id);
	write_byte(status);
	write_string(sprite);
	if (status)
	{
		write_byte(r);
		write_byte(g);
		write_byte(b);
	}
	message_end();
}

stock fade(id, r, g, b, a, Float:secs)
{
	if (!g_msgFade || g_bot[id])
		return;
	message_begin(MSG_ONE_UNRELIABLE, g_msgFade, _, id);
	write_short(floatround(secs * 4096.0));
	write_short(0);
	write_short(0);
	write_byte(r);
	write_byte(g);
	write_byte(b);
	write_byte(a);
	message_end();
}

stock round_timer(secs)
{
	if (!g_msgRoundTime)
		return;
	message_begin(MSG_ALL, g_msgRoundTime);
	write_short(secs);
	message_end();
}

stock beam(id, const Float:a[3], const Float:b[3], r, g, bl, life, width, bright)
{
	if (id)
		message_begin(MSG_ONE_UNRELIABLE, SVC_TEMPENTITY, _, id);
	else
		message_begin(MSG_BROADCAST, SVC_TEMPENTITY);
	write_byte(TE_BEAMPOINTS);
	write_coord(floatround(a[0])); write_coord(floatround(a[1])); write_coord(floatround(a[2]));
	write_coord(floatround(b[0])); write_coord(floatround(b[1])); write_coord(floatround(b[2]));
	write_short(g_sprBeam);
	write_byte(0);
	write_byte(0);
	write_byte(life);
	write_byte(width);
	write_byte(0);
	write_byte(r);
	write_byte(g);
	write_byte(bl);
	write_byte(bright);
	write_byte(0);
	message_end();
}

/* a circle of beam segments for one player, if their budget allows */
stock ring(id, const Float:c[3], Float:z, Float:r, segs, cr, cg, cb, bright)
{
	if (g_beamLeft[id] < segs || r < 8.0)
		return;
	g_beamLeft[id] -= segs;
	new Float:a[3], Float:b[3];
	a[2] = z;
	b[2] = z;
	for (new i = 0; i < segs; i++)
	{
		new Float:t0 = 360.0 * float(i) / float(segs), Float:t1 = 360.0 * float(i + 1) / float(segs);
		a[0] = c[0] + r * floatcos(t0, degrees);
		a[1] = c[1] + r * floatsin(t0, degrees);
		b[0] = c[0] + r * floatcos(t1, degrees);
		b[1] = c[1] + r * floatsin(t1, degrees);
		beam(id, a, b, cr, cg, cb, 12, 20, bright);
	}
}

stock bool:g_ent_has_model(s)
{
	new m[4];
	pev(g_ent[s], pev_model, m, charsmax(m));
	return m[0] != EOS;
}

/* a point barricade with no model: rails at four heights and posts at the ends, green to red by health */
stock fence(id, s)
{
	if (g_beamLeft[id] < 6)
		return;
	g_beamLeft[id] -= 6;
	new Float:f = floatclamp(g_val[s] / g_pf[s][B_MAX], 0.0, 1.0);
	new r = floatround(255.0 * (1.0 - f)), g = floatround(60.0 + 195.0 * f);
	new Float:a[3], Float:b[3];
	new bool:alongx = (g_maxs[s][0] - g_mins[s][0]) >= (g_maxs[s][1] - g_mins[s][1]);
	new Float:mid = alongx ? (g_mins[s][1] + g_maxs[s][1]) * 0.5 : (g_mins[s][0] + g_maxs[s][0]) * 0.5;
	for (new i = 0; i < 4; i++)
	{
		new Float:z = g_mins[s][2] + 8.0 + (g_maxs[s][2] - g_mins[s][2] - 12.0) * float(i) / 3.0;
		if (alongx)
		{
			a[0] = g_mins[s][0]; a[1] = mid; b[0] = g_maxs[s][0]; b[1] = mid;
		}
		else
		{
			a[0] = mid; a[1] = g_mins[s][1]; b[0] = mid; b[1] = g_maxs[s][1];
		}
		a[2] = z;
		b[2] = z;
		beam(id, a, b, r, g, 40, 12, 12, 150);
	}
	for (new e = 0; e < 2; e++)
	{
		if (alongx)
		{
			a[0] = e ? g_maxs[s][0] : g_mins[s][0]; a[1] = mid;
		}
		else
		{
			a[0] = mid; a[1] = e ? g_maxs[s][1] : g_mins[s][1];
		}
		vcopy(a, b);
		a[2] = g_mins[s][2];
		b[2] = g_maxs[s][2];
		beam(id, a, b, r, g, 40, 12, 20, 180);
	}
}

stock box_outline(id, s, cr, cg, cb)
{
	if (g_beamLeft[id] < 4)
		return;
	g_beamLeft[id] -= 4;
	new Float:z = g_mins[s][2] + 2.0;
	new Float:p[4][3];
	p[0][0] = g_mins[s][0]; p[0][1] = g_mins[s][1];
	p[1][0] = g_maxs[s][0]; p[1][1] = g_mins[s][1];
	p[2][0] = g_maxs[s][0]; p[2][1] = g_maxs[s][1];
	p[3][0] = g_mins[s][0]; p[3][1] = g_maxs[s][1];
	for (new i = 0; i < 4; i++)
		p[i][2] = z;
	for (new i = 0; i < 4; i++)
		beam(id, p[i], p[(i + 1) % 4], cr, cg, cb, 12, 20, 160);
}

/* the zone's ring for players near its edge; the pulse when it starts closing */
stock zone_beams(s, Float:r)
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p])
			continue;
		new Float:d = dist2d(g_porg[p], g_va[s]);
		if (floatabs(d - r) > 1500.0)
			continue;
		ring(p, g_va[s], g_porg[p][2] - 30.0, r, 24, 80, 160, 255, 180);
		if (g_pf[s][Z_TOR] < r - 1.0)
			ring(p, g_vb[s], g_porg[p][2] - 30.0, g_pf[s][Z_TOR], 16, 255, 255, 255, 90);
	}
}

stock pulse(const Float:c[3], Float:r)
{
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY);
	write_byte(TE_BEAMCYLINDER);
	write_coord(floatround(c[0])); write_coord(floatround(c[1])); write_coord(floatround(c[2]));
	write_coord(floatround(c[0])); write_coord(floatround(c[1])); write_coord(floatround(c[2] + floatmin(r, 4000.0)));
	write_short(g_sprBeam);
	write_byte(0);
	write_byte(0);
	write_byte(20);
	write_byte(40);
	write_byte(0);
	write_byte(80);
	write_byte(160);
	write_byte(255);
	write_byte(160);
	write_byte(0);
	message_end();
}

stock smoke(const Float:c[3])
{
	message_begin_f(MSG_PVS, SVC_TEMPENTITY, c);
	write_byte(TE_SMOKE);
	write_coord(floatround(c[0])); write_coord(floatround(c[1])); write_coord(floatround(c[2]));
	write_short(g_sprSmoke);
	write_byte(40);
	write_byte(8);
	message_end();
}

/* once a second: rings on the nearest areas each person can see */
stock visuals()
{
	if (!get_pcvar_num(g_cBeams))
		return;
	new bool:radar = get_pcvar_num(g_cRadar) != 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p])
			continue;
		new near[3] = { -1, -1, -1 };
		new Float:nd[3] = { 99999.0, 99999.0, 99999.0 };
		for (new s = 0; s < g_nslots; s++)
		{
			if (!g_ent[s] || !g_on[s])
				continue;
			new k = g_kind[s];
			if (k == K_BARRICADE && g_state[s] == 0 && !g_ent_has_model(s))
			{
				if (get_distance_f(g_porg[p], g_org[s]) < 1200.0)
					fence(p, s);
				continue;
			}
			if (k != K_CAPTURE && k != K_EXTRACT && k != K_CHECKPOINT && k != K_MEETING && k != K_DAMAGE)
				continue;
			if (k == K_CHECKPOINT && ki(s, "order", 1) != g_cpLast[p] + 1)
				continue;
			if (k == K_DAMAGE && !ki(s, "show", 1))
				continue;
			new Float:d = get_distance_f(g_porg[p], g_org[s]);
			if (d > 1500.0)
				continue;
			for (new i = 0; i < 3; i++)
			{
				if (d < nd[i])
				{
					for (new j = 2; j > i; j--)
					{
						nd[j] = nd[j - 1];
						near[j] = near[j - 1];
					}
					nd[i] = d;
					near[i] = s;
					break;
				}
			}
		}
		for (new i = 0; i < 3; i++)
		{
			new s = near[i];
			if (s < 0)
				break;
			new cr = 200, cg = 200, cb = 200;
			switch (g_kind[s])
			{
				case K_CAPTURE:
				{
					new o = g_side[s];
					if (g_aux[s] == -1) { cr = 255; cg = 220; cb = 40; }
					else if (g_pf[s][CP_FFA] > 0.0 && o) { cr = o == p ? 60 : 255; cg = o == p ? 255 : 160; cb = 60; }
					else if (o == 1) { cr = 255; cg = 60; cb = 40; }
					else if (o == 2) { cr = 60; cg = 120; cb = 255; }
				}
				case K_EXTRACT:
				{
					if (g_state[s] == 1) { cr = 60; cg = 255; cb = 90; }
					else { cr = 120; cg = 120; cb = 120; }
				}
				case K_CHECKPOINT: { cr = 255; cg = 220; cb = 60; }
				case K_MEETING: { cr = 255; cg = 200; cb = 140; }
				case K_DAMAGE: { cr = 255; cg = 40; cb = 20; }
			}
			if (g_box[s])
				box_outline(p, s, cr, cg, cb);
			else
				ring(p, g_org[s], g_floor[s] + 4.0, g_r[s], nd[i] > 800.0 ? 12 : 16, cr, cg, cb, 170);
		}
		if (radar)
			radar_dot(p);
	}
}

/* a dot on the radar at the most useful spot (a spare player slot's dot: unverified on the stock client) */
stock radar_dot(p)
{
	if (!g_msgRadar)
		return;
	new spare = 0;
	for (new i = MaxClients; i >= 1; i--)
	{
		if (!is_user_connected(i))
		{
			spare = i;
			break;
		}
	}
	if (!spare)
		return;
	new Float:at[3], bool:have = false;
	for (new s = 0; s < g_nslots && !have; s++)
	{
		if (!g_ent[s] || !g_on[s])
			continue;
		if (g_kind[s] == K_ZONE && inside_get(p, s))
		{
			vcopy(g_va[s], at);
			have = true;
		}
		else if ((g_kind[s] == K_EXTRACT && g_state[s] == 1) || (g_kind[s] == K_CRATE && g_state[s] != CRATE_EMPTY))
		{
			vcopy(g_org[s], at);
			have = true;
		}
	}
	if (!have)
		return;
	message_begin(MSG_ONE_UNRELIABLE, g_msgRadar, _, p);
	write_byte(spare);
	write_coord(floatround(at[0]));
	write_coord(floatround(at[1]));
	write_coord(floatround(at[2]));
	message_end();
}

stock render(ent, fx, r, g, b, mode, amt)
{
	new Float:col[3];
	col[0] = float(r);
	col[1] = float(g);
	col[2] = float(b);
	set_pev(ent, pev_renderfx, fx);
	set_pev(ent, pev_rendercolor, col);
	set_pev(ent, pev_rendermode, mode);
	set_pev(ent, pev_renderamt, float(amt));
}

stock tier_glow(ent, tier)
{
	switch (clamp(tier, 1, 5))
	{
		case 1: render(ent, kRenderFxGlowShell, 230, 230, 230, kRenderNormal, 4); /* W3: white */
		case 2: render(ent, kRenderFxGlowShell, 60, 200, 60, kRenderNormal, 6);
		case 3: render(ent, kRenderFxGlowShell, 60, 120, 255, kRenderNormal, 8);
		case 4: render(ent, kRenderFxGlowShell, 190, 60, 255, kRenderNormal, 10);
		case 5: render(ent, kRenderFxGlowShell, 255, 170, 0, kRenderNormal, 12);
	}
}

stock dir_word(id, const Float:to[3], out[], len)
{
	new Float:ang[3];
	pev(id, pev_v_angle, ang);
	new Float:yaw = floatatan2(to[1] - g_porg[id][1], to[0] - g_porg[id][0], degrees);
	new Float:d = yaw - ang[1];
	while (d > 180.0)
		d -= 360.0;
	while (d < -180.0)
		d += 360.0;
	if (floatabs(d) < 30.0)
		copy(out, len, "ahead");
	else if (floatabs(d) > 150.0)
		copy(out, len, "behind you");
	else if (d > 0.0)
		copy(out, len, "to your left");
	else
		copy(out, len, "to your right");
}

/* ==== W3: Battle Royale pieces ==== */
/* =========================================================================
 * Battle Royale pieces (W3), each usable by any mode and by Game Mode Studio:
 * healing items. Natives and forwards: ls_components.inc ("W3: Battle Royale
 * pieces"); docs/COMPONENTS.md.
 * ========================================================================= */

#define W3_TASK_HEALMENU 7736900

/* ---- healing items: carried, used by key or menu with a progress bar ---- */

enum { HEAL_BANDAGE = 0, HEAL_MEDKIT, HEAL_DRINK, HEAL_KINDS };
new const W3_HEAL_ID[HEAL_KINDS][] = { "bandage", "medkit", "drink" };
new const W3_HEAL_TITLE[HEAL_KINDS][] = { "Bandage", "Medkit", "Energy drink" };
new g_w3Heals[33][HEAL_KINDS];
new g_w3Using[33];
new Float:g_w3UseEnd[33];
new Float:g_w3BoostEnd[33];
new Float:g_w3BoostHp[33];
new Float:g_w3Hurt[33];
new Float:g_w3BotTry[33];
new g_w3cHealCarry, g_w3cHealLimits, g_w3cHealSecs, g_w3cHealKey, g_w3cHealBots;
new g_w3fwHeal;
new bool:g_w3Hooked;

/* armor levels */
new g_w3Vest[33], g_w3Helm[33];
new Float:g_w3HelmHp[33];
new g_w3cArmorLevels;
new g_w3fwArmor;
/* armor points and the share of a hit that still gets through (on top of the game's own vest) */
new const W3_VEST_AP[4] = { 0, 100, 120, 150 };
new const Float:W3_VEST_F[4] = { 1.0, 1.0, 0.85, 0.7 };
/* a helmet takes this much of head hits before it breaks; the share that gets through */
new const Float:W3_HELM_HP[4] = { 0.0, 100.0, 150.0, 200.0 };
new const Float:W3_HELM_F[4] = { 1.0, 0.7, 0.6, 0.45 };
/* death crates */
#define W3_DC_MAX 48
#define W3_DC_ITEMS 16
new g_w3Dc[W3_DC_MAX];
new g_w3DcItem[W3_DC_MAX][W3_DC_ITEMS][24];
new g_w3DcN[W3_DC_MAX];
new g_w3DcName[W3_DC_MAX][32];
new Float:g_w3DcAt[W3_DC_MAX];
new g_w3Snap[33][W3_DC_ITEMS][24];
new g_w3SnapN[33];
new g_w3MenuDc[33];
new g_w3cDeathCrates, g_w3fwCrate, g_w3fwCrateTake;
new g_w3DcTaken, g_w3DcMade;
/* counts for the round's log line */
new g_w3StatHeal[HEAL_KINDS], g_w3StatVest[4], g_w3StatHelm[4], g_w3DcLooted;
/* match HUD */
#define W3_TASK_SPEC 7737000
#define W3_FEED 5
new g_w3cMatchHud, g_w3SyncTop, g_w3SyncSide;
new g_w3Kills[33];
new g_w3Feed[W3_FEED][96];
new Float:g_w3FeedAt[W3_FEED];
new g_w3HudText[128];
new g_w3Killer[33];
/* drop-in */
new g_w3cDropHeight, g_w3fwLanded;
new bool:g_w3Falling[33];
new Float:g_w3FallAt[33];
new Float:g_w3DropSpots[33][2];
new g_w3NDrop;
/* squads */
new g_w3Squad[33];
new g_w3SquadMax = 1;
new g_w3Code[33][6];
new g_w3Invite[33];
new bool:g_w3Lobby;
new Float:g_w3LobbyEnd;
new g_w3cSquadFF, g_w3cSquadMarks, g_w3cSquadTeams, g_w3fwSquad, g_w3fwLobby;
new g_w3SprMark;
new bool:g_w3Down[33];
/* rarity glow */
new g_w3cRarity;
/* scarce ammo */
new g_w3cAmmoScarce, g_w3cAmmoMags;
/* every gun that takes ammo (primaries and pistols) */
#define W3_GUN_IDS ((1 << CSW_SCOUT) | (1 << CSW_XM1014) | (1 << CSW_MAC10) | (1 << CSW_AUG) | (1 << CSW_UMP45) | (1 << CSW_SG550) | (1 << CSW_GALIL) | (1 << CSW_FAMAS) | (1 << CSW_AWP) | (1 << CSW_MP5NAVY) | (1 << CSW_M249) | (1 << CSW_M3) | (1 << CSW_M4A1) | (1 << CSW_TMP) | (1 << CSW_G3SG1) | (1 << CSW_SG552) | (1 << CSW_AK47) | (1 << CSW_P90) | (1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))

stock w3_natives()
{
	register_native("ls_heal_give", "n_w3_heal_give");
	register_native("ls_heal_count", "n_w3_heal_count");
	register_native("ls_heal_use", "n_w3_heal_use");
	register_native("ls_heal_cancel", "n_w3_heal_cancel");
	register_native("ls_heal_using", "n_w3_heal_using");
	register_native("ls_armor_set", "n_w3_armor_set");
	register_native("ls_armor_get", "n_w3_armor_get");
	register_native("ls_item_tier", "n_w3_item_tier");
	register_native("ls_glow", "n_w3_glow");
	register_native("ls_crate_make", "n_w3_crate_make");
	register_native("ls_crate_add", "n_w3_crate_add");
	register_native("ls_crate_count", "n_w3_crate_count");
	register_native("ls_crate_item", "n_w3_crate_item");
	register_native("ls_crate_open", "n_w3_crate_open");
	register_native("ls_match_hud", "n_w3_match_hud");
	register_native("ls_match_hud_text", "n_w3_match_hud_text");
	register_native("ls_feed", "n_w3_feed");
	register_native("ls_kills", "n_w3_kills");
	register_native("ls_spectate", "n_w3_spectate");
	register_native("ls_drop_in", "n_w3_drop_in");
	register_native("ls_dropping", "n_w3_dropping");
	register_native("ls_drop_spot", "n_w3_drop_spot");
	register_native("ls_drop_reset", "n_w3_drop_reset");
	register_native("ls_squad_set", "n_w3_squad_set");
	register_native("ls_squad_get", "n_w3_squad_get");
	register_native("ls_squad_max", "n_w3_squad_max");
	register_native("ls_squad_count", "n_w3_squad_count");
	register_native("ls_squads_alive", "n_w3_squads_alive");
	register_native("ls_squad_fill", "n_w3_squad_fill");
	register_native("ls_squad_lobby", "n_w3_squad_lobby");
	register_native("ls_squad_menu", "n_w3_squad_menu");
	register_native("ls_squad_short", "n_w3_squad_short");
	register_native("ls_squad_code", "n_w3_squad_code");
}

stock w3_init()
{
	/* 1: bandages, medkits and energy drinks are carried and used (key or menu); 0: they heal at once */
	g_w3cHealCarry = register_cvar("ls_heal_carry", "0");
	/* how many of each a player can carry: bandage medkit drink */
	g_w3cHealLimits = register_cvar("ls_heal_limits", "6 2 3");
	/* seconds to use each */
	g_w3cHealSecs = register_cvar("ls_heal_secs", "3 7 4");
	/* 1: the flashlight key (F) uses a healing item (the menu when there's a choice) */
	g_w3cHealKey = register_cvar("ls_heal_key", "1");
	/* 1: bots heal by themselves when hurt and nobody's in sight */
	g_w3cHealBots = register_cvar("ls_heal_bots", "1");
	g_w3fwHeal = CreateMultiForward("ls_on_heal", ET_STOP, FP_CELL, FP_STRING, FP_CELL);
	register_clcmd("ls_heal", "cmd_w3_heal");
	register_srvcmd("ls_comp_players", "cmd_w3_players");
	/* for the bots' brain: ls_comp_heal <bot> [kind] starts a heal (it picks the moment, e.g. in cover) */
	register_srvcmd("ls_comp_heal", "cmd_w3_heal_srv");
	register_clcmd("say /heal", "cmd_w3_heal_menu");
	register_clcmd("say_team /heal", "cmd_w3_heal_menu");
	register_forward(FM_CmdStart, "fm_w3_cmdstart");
	RegisterHamPlayer(Ham_Killed, "ham_w3_killed", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_w3_hurt_post", 1);
	RegisterHamPlayer(Ham_CS_Player_ResetMaxSpeed, "ham_w3_maxspeed", 1);
	register_event("HLTV", "ev_w3_new_round", "a", "1=0", "2=0");
	/* 1: vests and helmets come in levels 1-3 (vest1..3, helmet1..3 as loot) */
	g_w3cArmorLevels = register_cvar("ls_armor_levels", "0");
	g_w3fwArmor = CreateMultiForward("ls_on_armor", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL);
	/* 1: ammo is loot: a gun found comes with its magazine and ls_ammo_mags more, "ammo" gives 2 magazines */
	g_w3cAmmoScarce = register_cvar("ls_ammo_scarce", "0");
	g_w3cAmmoMags = register_cvar("ls_ammo_mags", "1");
	/* 1: floor items glow by their own rarity (white, green, blue, purple, gold), guns players drop too */
	g_w3cRarity = register_cvar("ls_rarity_glow", "0");
	register_forward(FM_SetModel, "fm_w3_setmodel", 1);
	/* 1: a killed player drops one crate with everything they had (open it with USE) */
	g_w3cDeathCrates = register_cvar("ls_death_crates", "0");
	g_w3fwCrate = CreateMultiForward("ls_on_death_crate", ET_IGNORE, FP_CELL, FP_CELL);
	g_w3fwCrateTake = CreateMultiForward("ls_on_crate_take", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	RegisterHamPlayer(Ham_Killed, "ham_w3_killed_pre");
	register_forward(FM_PlayerPreThink, "fm_w3_prethink");
	/* 1: the match HUD (players left, kills, zone timer, armor, heals, kill feed; the dead watch their killer) */
	g_w3cMatchHud = register_cvar("ls_match_hud", "0");
	g_w3SyncTop = CreateHudSyncObj();
	g_w3SyncSide = CreateHudSyncObj();
	/* drop-in: how high above the landing spot at most (the sky is the real limit) */
	g_w3cDropHeight = register_cvar("ls_drop_height", "3000");
	g_w3fwLanded = CreateMultiForward("ls_on_landed", ET_IGNORE, FP_CELL);
	register_forward(FM_PlayerPostThink, "fm_w3_postthink", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_w3_hurt_pre");
	/* squads: squad mates can't hurt each other unless 1; markers over their heads and on the radar */
	g_w3cSquadFF = register_cvar("ls_squad_ff", "0");
	g_w3cSquadMarks = register_cvar("ls_squad_markers", "1");
	/* 1: squads are put on alternating teams (the scoreboard's colours) when they're filled */
	g_w3cSquadTeams = register_cvar("ls_squad_teams", "1");
	g_w3fwSquad = CreateMultiForward("ls_on_squad", ET_IGNORE, FP_CELL, FP_CELL);
	g_w3fwLobby = CreateMultiForward("ls_on_lobby_end", ET_IGNORE, FP_CELL);
	register_clcmd("say", "cmd_w3_say");
	register_clcmd("say_team", "cmd_w3_say");
	register_clcmd("ls_squad", "cmd_w3_squad");
	RegisterHamPlayer(Ham_TraceAttack, "ham_w3_trace");
	g_w3Hooked = true;
}

public ev_w3_new_round()
{
	for (new p = 1; p <= 32; p++)
	{
		w3_reset_player(p);
		g_w3Down[p] = false;
		g_w3Kills[p] = 0;
		g_w3Killer[p] = 0;
		remove_task(W3_TASK_SPEC + p);
	}
	for (new i = 0; i < W3_FEED; i++)
		g_w3Feed[i][0] = EOS;
	for (new c = 0; c < W3_DC_MAX; c++)
		w3_dc_remove(c);
	w3_round_stats();
}

stock w3_reset_player(p)
{
	w3_heal_stop(p, false);
	for (new k = 0; k < HEAL_KINDS; k++)
		g_w3Heals[p][k] = 0;
	g_w3BoostEnd[p] = 0.0;
	g_w3BoostHp[p] = 0.0;
	g_w3Hurt[p] = 0.0;
	g_w3Vest[p] = 0;
	g_w3Helm[p] = 0;
	g_w3HelmHp[p] = 0.0;
	if (g_w3Falling[p])
		w3_land(p, false);
}

public client_putinserver(id)
{
	if (g_w3Hooked)
		w3_reset_player(id);
	g_w3Squad[id] = 0;
	g_w3Invite[id] = 0;
	g_w3Down[id] = false;
}

public ham_w3_killed(id, killer, gib)
{
	w3_heal_stop(id, false);
	g_w3Falling[id] = false;
	if (get_pcvar_num(g_w3cDeathCrates) && g_w3SnapN[id])
		w3_dc_from_death(id);
	g_w3SnapN[id] = 0;
	w3_match_death(id, killer);
	g_w3Down[id] = false;
	for (new k = 0; k < HEAL_KINDS; k++)
		g_w3Heals[id][k] = 0;
	g_w3BoostEnd[id] = 0.0;
	g_w3Vest[id] = 0;
	g_w3Helm[id] = 0;
	g_w3HelmHp[id] = 0.0;
	return HAM_IGNORED;
}

public ham_w3_hurt_post(victim, inflictor, attacker, Float:dmg, bits)
{
	if (!(1 <= victim <= 32) || dmg <= 0.0)
		return HAM_IGNORED;
	g_w3Hurt[victim] = get_gametime();
	/* a bot shot while healing fights back */
	if (g_w3Using[victim] && is_user_bot(victim) && 1 <= attacker <= 32 && attacker != victim)
		w3_heal_stop(victim, true);
	return HAM_IGNORED;
}

/* "6 2 3" -> the kind's number */
stock w3_kind_num(pcvar, kind, def)
{
	new v[64], a[3][12];
	get_pcvar_string(pcvar, v, charsmax(v));
	parse(v, a[0], 11, a[1], 11, a[2], 11);
	return a[kind][0] ? str_to_num(a[kind]) : def;
}

stock Float:w3_kind_secs(kind)
{
	new v[64], a[3][12];
	get_pcvar_string(g_w3cHealSecs, v, charsmax(v));
	parse(v, a[0], 11, a[1], 11, a[2], 11);
	static const Float:DEF[HEAL_KINDS] = { 3.0, 7.0, 4.0 };
	return a[kind][0] ? floatmax(str_to_float(a[kind]), 0.1) : DEF[kind];
}

stock w3_heal_limit(kind)
{
	static const DEF[HEAL_KINDS] = { 6, 2, 3 };
	return max(w3_kind_num(g_w3cHealLimits, kind, DEF[kind]), 0);
}

/* "bandage", "bandage:3", "medkit", "firstaid", "drink", "energy" -> the kind (-1 not a healing item) */
stock w3_heal_kind(const item[])
{
	if (equali(item, "bandage", 7))
		return HEAL_BANDAGE;
	if (equali(item, "medkit", 6) || equali(item, "firstaid", 8))
		return HEAL_MEDKIT;
	if (equali(item, "drink", 5) || equali(item, "energy", 6))
		return HEAL_DRINK;
	return -1;
}

stock w3_item_count(const item[])
{
	new c = contain(item, ":");
	return c > 0 ? max(str_to_num(item[c + 1]), 1) : 1;
}

stock Float:w3_max_hp(id)
{
	new Float:mx;
	pev(id, pev_max_health, mx);
	return floatmax(mx, 100.0);
}

/* gives n of a kind (up to the carry limit); returns how many were taken */
stock w3_heal_give(id, kind, n)
{
	new room = w3_heal_limit(kind) - g_w3Heals[id][kind];
	n = min(n, room);
	if (n <= 0)
		return 0;
	g_w3Heals[id][kind] += n;
	w3_heals_hint(id);
	emit_sound(id, CHAN_ITEM, "items/smallmedkit1.wav", 0.7, ATTN_NORM, 0, PITCH_NORM);
	if (!is_user_bot(id) && get_pcvar_num(g_w3cHealKey))
		client_print(id, print_center, "%s x%d  (F or say /heal to use)", W3_HEAL_TITLE[kind], g_w3Heals[id][kind]);
	return n;
}

/* can a heal of this kind do anything now (bandages only up to 75) */
stock bool:w3_heal_useful(id, kind)
{
	new Float:hp;
	pev(id, pev_health, hp);
	switch (kind)
	{
		case HEAL_BANDAGE: return hp < 75.0;
		case HEAL_MEDKIT: return hp < w3_max_hp(id);
		case HEAL_DRINK: return hp < w3_max_hp(id) || g_w3BoostEnd[id] < get_gametime();
	}
	return false;
}

/* starts using one; kind -1 = the best one for now */
stock w3_heal_start(id, kind)
{
	if (!is_user_alive(id) || g_w3Using[id])
		return 0;
	if (kind < 0)
		kind = w3_heal_best(id);
	if (kind < 0 || g_w3Heals[id][kind] <= 0)
		return 0;
	if (!w3_heal_useful(id, kind))
	{
		if (!is_user_bot(id))
			client_print(id, print_center, kind == HEAL_BANDAGE ? "Bandages heal up to 75" : "You're at full health");
		return 0;
	}
	new ret;
	ExecuteForward(g_w3fwHeal, ret, id, W3_HEAL_ID[kind], 0);
	if (ret >= PLUGIN_HANDLED)
		return 0;
	act_cancel(id);
	new Float:secs = w3_kind_secs(kind);
	g_w3Using[id] = kind + 1;
	g_w3UseEnd[id] = get_gametime() + secs;
	bar_time(id, floatround(secs, floatround_ceil));
	/* bots stand still while healing (people: moving cancels) */
	if (is_user_bot(id))
	{
		set_user_maxspeed(id, 1.0);
		hint("heal %d %s %.0f", id, W3_HEAL_ID[kind], secs);
	}
	return 1;
}

/* the kind to use now: a medkit when low, else bandages up to 75, else a drink */
stock w3_heal_best(id)
{
	new Float:hp;
	pev(id, pev_health, hp);
	if (hp < 45.0 && g_w3Heals[id][HEAL_MEDKIT] > 0)
		return HEAL_MEDKIT;
	if (hp < 75.0 && g_w3Heals[id][HEAL_BANDAGE] > 0)
		return HEAL_BANDAGE;
	if (g_w3Heals[id][HEAL_DRINK] > 0 && w3_heal_useful(id, HEAL_DRINK))
		return HEAL_DRINK;
	if (hp < w3_max_hp(id) && g_w3Heals[id][HEAL_MEDKIT] > 0)
		return HEAL_MEDKIT;
	return -1;
}

stock w3_heal_stop(id, bool:cancelled)
{
	if (!g_w3Using[id])
		return;
	new kind = g_w3Using[id] - 1;
	g_w3Using[id] = 0;
	if (is_user_connected(id))
	{
		bar_time(id, 0);
		if (is_user_alive(id))
			ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, id);
		if (is_user_bot(id))
			hint("heal %d -", id);
	}
	if (cancelled)
	{
		new ret;
		ExecuteForward(g_w3fwHeal, ret, id, W3_HEAL_ID[kind], 2);
	}
}

stock w3_heal_finish(id)
{
	new kind = g_w3Using[id] - 1;
	g_w3Using[id] = 0;
	bar_time(id, 0);
	if (g_w3Heals[id][kind] > 0)
		g_w3Heals[id][kind]--;
	w3_heals_hint(id);
	g_w3StatHeal[kind]++;
	w3_heal_apply(id, kind);
	ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, id);
	if (is_user_bot(id))
		hint("heal %d -", id);
	new ret;
	ExecuteForward(g_w3fwHeal, ret, id, W3_HEAL_ID[kind], 1);
}

stock w3_heal_apply(id, kind)
{
	new Float:hp, Float:mx = w3_max_hp(id);
	pev(id, pev_health, hp);
	switch (kind)
	{
		case HEAL_BANDAGE: hp = floatmax(hp, floatmin(hp + 15.0, 75.0));
		case HEAL_MEDKIT: hp = mx;
		case HEAL_DRINK:
		{
			/* 40 over 10 s, and a little speed meanwhile */
			g_w3BoostEnd[id] = get_gametime() + 10.0;
			g_w3BoostHp[id] = 40.0;
			if (is_user_alive(id))
				ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, id);
		}
	}
	set_pev(id, pev_health, floatmin(hp, mx));
	emit_sound(id, CHAN_ITEM, "items/smallmedkit1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
}

public ham_w3_maxspeed(id)
{
	if (!is_user_alive(id) || (pev(id, pev_flags) & FL_FROZEN))
		return HAM_IGNORED;
	if (g_w3Using[id] && is_user_bot(id))
		set_user_maxspeed(id, 1.0);
	else if (g_w3BoostEnd[id] > get_gametime())
		set_user_maxspeed(id, get_user_maxspeed(id) * 1.08);
	return HAM_IGNORED;
}

/* using: no shooting; a person who moves stops; F opens the healing items */
public fm_w3_cmdstart(id, uc, seed)
{
	if (!is_user_alive(id))
		return FMRES_IGNORED;
	if (g_w3Using[id])
	{
		new b = get_uc(uc, UC_Buttons);
		if (!is_user_bot(id) && (b & (IN_FORWARD | IN_BACK | IN_MOVELEFT | IN_MOVERIGHT | IN_JUMP)))
		{
			w3_heal_stop(id, true);
			client_print(id, print_center, "Healing stopped");
			return FMRES_IGNORED;
		}
		set_uc(uc, UC_Buttons, b & ~(IN_ATTACK | IN_ATTACK2));
		return FMRES_HANDLED;
	}
	if (get_uc(uc, UC_Impulse) == 100 && get_pcvar_num(g_w3cHealKey) && get_pcvar_num(g_w3cHealCarry) && w3_heals_total(id))
	{
		set_uc(uc, UC_Impulse, 0);
		remove_task(W3_TASK_HEALMENU + id);
		set_task(0.01, "task_w3_heal_key", W3_TASK_HEALMENU + id);
		return FMRES_HANDLED;
	}
	return FMRES_IGNORED;
}

public task_w3_heal_key(tid)
{
	new id = tid - W3_TASK_HEALMENU;
	if (!is_user_alive(id))
		return;
	/* one kind that helps now: use it; more: the menu */
	new kinds = 0, last = -1;
	for (new k = 0; k < HEAL_KINDS; k++)
		if (g_w3Heals[id][k] > 0 && w3_heal_useful(id, k))
		{
			kinds++;
			last = k;
		}
	if (kinds == 1)
		w3_heal_start(id, last);
	else
		w3_heal_menu(id);
}

stock w3_heals_total(id)
{
	new n = 0;
	for (new k = 0; k < HEAL_KINDS; k++)
		n += g_w3Heals[id][k];
	return n;
}

public cmd_w3_heal(id)
{
	new a[16];
	read_argv(1, a, charsmax(a));
	if (!a[0])
		w3_heal_menu(id);
	else
		w3_heal_start(id, w3_heal_kind(a));
	return PLUGIN_HANDLED;
}

public cmd_w3_heal_srv()
{
	new a[8], k[16];
	read_argv(1, a, charsmax(a));
	read_argv(2, k, charsmax(k));
	new id = str_to_num(a);
	if (1 <= id <= MaxClients && is_user_alive(id))
		w3_heal_start(id, k[0] ? w3_heal_kind(k) : -1);
	return PLUGIN_HANDLED;
}

/* bots: the brain hears what healing items each one carries when that changes */
stock w3_heals_hint(id)
{
	if (is_user_bot(id))
		hint("heals %d %d %d %d", id, g_w3Heals[id][HEAL_BANDAGE], g_w3Heals[id][HEAL_MEDKIT], g_w3Heals[id][HEAL_DRINK]);
}

public cmd_w3_heal_menu(id)
{
	w3_heal_menu(id);
	return PLUGIN_HANDLED;
}

stock w3_heal_menu(id)
{
	if (!is_user_alive(id))
		return;
	if (!w3_heals_total(id))
	{
		client_print(id, print_center, "No healing items");
		return;
	}
	static const DESC[HEAL_KINDS][] = { "+15 up to 75", "full health", "+40 over 10 s, a little faster" };
	new m = menu_create("Healing items", "menu_w3_heal");
	new line[96], info[4];
	for (new k = 0; k < HEAL_KINDS; k++)
	{
		if (!g_w3Heals[id][k])
			continue;
		formatex(line, charsmax(line), "%s x%d \d(%s, %.0f s)", W3_HEAL_TITLE[k], g_w3Heals[id][k], DESC[k], w3_kind_secs(k));
		num_to_str(k, info, charsmax(info));
		menu_additem(m, line, info);
	}
	menu_display(id, m);
}

public menu_w3_heal(id, menu, item)
{
	if (item >= 0)
	{
		new info[4], acc, cb;
		menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb);
		w3_heal_start(id, str_to_num(info));
	}
	menu_destroy(menu);
	return PLUGIN_HANDLED;
}

/* gm_components' give_thing: -1 = not one of these, else 0/1 */
stock w3_give(id, const item[])
{
	new r = w3_give_armor(id, item);
	if (r >= 0)
		return r;
	r = w3_give_ammo(id, item);
	if (r >= 0)
		return r;
	return w3_give_heal(id, item);
}

stock w3_give_heal(id, const item[])
{
	new kind = w3_heal_kind(item);
	if (kind >= 0)
	{
		new n = w3_item_count(item);
		if (get_pcvar_num(g_w3cHealCarry))
			return w3_heal_give(id, kind, n) > 0 ? 1 : 0;
		/* not carried: used at once (a medkit as before: +50) */
		new Float:hp, Float:mx = w3_max_hp(id);
		pev(id, pev_health, hp);
		if (kind == HEAL_BANDAGE)
			hp = floatmax(hp, floatmin(hp + 15.0 * float(n), 75.0));
		else
			hp = floatmin(hp + (kind == HEAL_MEDKIT ? 50.0 : 25.0) * float(n), mx);
		set_pev(id, pev_health, hp);
		emit_sound(id, CHAN_ITEM, "items/smallmedkit1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		return 1;
	}
	return -1;
}

/* gm_components' can_take: -1 = not one of these */
stock w3_can_take(id, const item[])
{
	new r = w3_can_take_armor(id, item);
	if (r >= 0)
		return r;
	r = w3_can_take_ammo(id, item);
	if (r >= 0)
		return r;
	new kind = w3_heal_kind(item);
	if (kind >= 0)
		return !get_pcvar_num(g_w3cHealCarry) || g_w3Heals[id][kind] < w3_heal_limit(kind);
	return -1;
}

stock bool:w3_item_model(const item[], out[], len)
{
	if (w3_heal_kind(item) >= 0)
	{
		copy(out, len, "models/w_medkit.mdl");
		return true;
	}
	new lv, part = w3_armor_item(item, lv);
	if (part >= 0)
	{
		copy(out, len, part == 0 ? "models/w_kevlar.mdl" : "models/w_assault.mdl");
		return true;
	}
	if (equali(item, "ammo:", 5))
	{
		copy(out, len, "models/w_9mmarclip.mdl");
		return true;
	}
	return false;
}

/* each 0.25 s (from tick) */
stock w3_tick(bool:sec)
{
	new Float:now = get_gametime();
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p])
		{
			if (g_w3Using[p])
				w3_heal_stop(p, false);
			continue;
		}
		if (g_w3Using[p])
		{
			if (now >= g_w3UseEnd[p])
				w3_heal_finish(p);
			else if (!g_bot[p])
				hud_add(p, false, "Using %s... (moving stops it)", W3_HEAL_TITLE[g_w3Using[p] - 1]);
		}
		/* the drink's heal over time */
		if (g_w3BoostEnd[p] > 0.0)
		{
			if (now >= g_w3BoostEnd[p] || g_w3BoostHp[p] <= 0.0)
			{
				g_w3BoostEnd[p] = 0.0;
				ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, p);
			}
			else
			{
				new Float:hp, Float:give = floatmin(1.0, g_w3BoostHp[p]);
				pev(p, pev_health, hp);
				g_w3BoostHp[p] -= give;
				set_pev(p, pev_health, floatmin(hp + give, w3_max_hp(p)));
			}
		}
		if (sec && g_bot[p] && !g_w3Using[p] && get_pcvar_num(g_w3cHealBots) && get_pcvar_num(g_w3cHealCarry))
			w3_bot_heal(p, now);
		if (sec && g_bot[p])
			w3_bot_crate(p);
		else if (!g_bot[p] && !g_act[p])
			w3_crate_hint(p);
		/* a vest shot through */
		if (g_w3Vest[p] && get_user_armor(p) <= 0)
			w3_armor_set(p, 0, -1, true);
	}
	if (sec && get_pcvar_num(g_w3cMatchHud) && !g_w3Lobby)
		w3_match_hud();
	if (sec)
		w3_squad_tick();
}

/* a hurt bot heals when nobody has hurt it for 4 s and no enemy is in sight (W2 may take this over) */
stock w3_bot_heal(p, Float:now)
{
	if (now - g_w3Hurt[p] < 4.0 || now < g_w3BotTry[p] || !w3_heals_total(p))
		return;
	new kind = w3_heal_best(p);
	if (kind < 0)
		return;
	new Float:hp;
	pev(p, pev_health, hp);
	/* (a drink only tops up when a bit hurt) */
	if (kind == HEAL_DRINK && hp > 85.0)
		return;
	g_w3BotTry[p] = now + 2.0;
	if (w3_enemy_in_sight(p, 1500.0))
		return;
	w3_heal_start(p, kind);
}

/* is someone who could shoot this player in sight (not a friend: team when not free for all) */
stock bool:w3_enemy_in_sight(p, Float:range)
{
	new Float:a[3], Float:b[3];
	pev(p, pev_origin, a);
	a[2] += 16.0;
	for (new q = 1; q <= MaxClients; q++)
	{
		if (q == p || !g_alive[q] || !w3_enemies(p, q))
			continue;
		pev(q, pev_origin, b);
		b[2] += 16.0;
		if (get_distance_f(a, b) > range)
			continue;
		engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, p, 0);
		new Float:f;
		get_tr2(0, TR_flFraction, f);
		if (f >= 0.99 || get_tr2(0, TR_pHit) == q)
			return true;
	}
	return false;
}

stock bool:w3_enemies(a, b)
{
	if (get_cvar_num("mp_freeforall"))
		return true;
	return get_user_team(a) != get_user_team(b);
}

/* ---- natives ---- */

/* native ls_heal_give(id, const kind[], n = 1) */
public n_w3_heal_give(plugin, params)
{
	new id = get_param(1), k[16];
	get_string(2, k, charsmax(k));
	new kind = w3_heal_kind(k);
	if (!(1 <= id <= MaxClients) || kind < 0 || !is_user_alive(id))
		return 0;
	return w3_heal_give(id, kind, max(get_param(3), 1));
}

/* native ls_heal_count(id, const kind[]) ("" = all) */
public n_w3_heal_count(plugin, params)
{
	new id = get_param(1), k[16];
	get_string(2, k, charsmax(k));
	if (!(1 <= id <= MaxClients))
		return 0;
	if (!k[0])
		return w3_heals_total(id);
	new kind = w3_heal_kind(k);
	return kind >= 0 ? g_w3Heals[id][kind] : 0;
}

/* native ls_heal_use(id, const kind[] = "") */
public n_w3_heal_use(plugin, params)
{
	new id = get_param(1), k[16];
	get_string(2, k, charsmax(k));
	if (!(1 <= id <= MaxClients))
		return 0;
	return w3_heal_start(id, k[0] ? w3_heal_kind(k) : -1);
}

/* native ls_heal_cancel(id) */
public n_w3_heal_cancel(plugin, params)
{
	new id = get_param(1);
	if (1 <= id <= MaxClients)
		w3_heal_stop(id, true);
	return 1;
}

/* native bool:ls_heal_using(id) */
public n_w3_heal_using(plugin, params)
{
	new id = get_param(1);
	return 1 <= id <= MaxClients && g_w3Using[id] != 0;
}

/* server: each living player's healing items, armor levels, guns and ammo (for tests) */
public cmd_w3_players()
{
	new Float:now = get_gametime();
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		new weapons[32], nw, list[160], len = 0, w[32];
		get_user_weapons(p, weapons, nw);
		for (new i = 0; i < nw; i++)
		{
			if (!((1 << weapons[i]) & W3_GUN_IDS))
				continue;
			get_weaponname(weapons[i], w, charsmax(w));
			new e = w3_find_weapon(p, w);
			len += formatex(list[len], charsmax(list) - len, " %s %d+%d", w[7], e ? cs_get_weapon_ammo(e) : 0, cs_get_user_bpammo(p, weapons[i]));
		}
		server_print("%2d hp %3d armor %3d vest %d helmet %d (%.0f) heals %d/%d/%d%s%s guns:%s", p, get_user_health(p), get_user_armor(p),
			g_w3Vest[p], g_w3Helm[p], g_w3HelmHp[p], g_w3Heals[p][0], g_w3Heals[p][1], g_w3Heals[p][2],
			g_w3Using[p] ? " HEALING" : "", g_w3Falling[p] ? " falling" : "", list);
	}
	new n = 0;
	for (new c = 0; c < W3_DC_MAX; c++)
		if (g_w3Dc[c])
			n++;
	server_print("death crates on the ground: %d (game time %.0f)", n, now);
	return PLUGIN_HANDLED;
}

/* one log line per round: what the BR pieces did (for tests and tuning) */
stock w3_round_stats()
{
	new any = g_w3DcMade + g_w3StatHeal[0] + g_w3StatHeal[1] + g_w3StatHeal[2];
	for (new l = 1; l <= 3; l++)
		any += g_w3StatVest[l] + g_w3StatHelm[l];
	if (any)
		log_amx("BR pieces last round: heals used %d bandage %d medkit %d drink; armor found vest %d/%d/%d helmet %d/%d/%d (levels 1/2/3); death crates %d made, %d searched, %d items taken",
			g_w3StatHeal[0], g_w3StatHeal[1], g_w3StatHeal[2], g_w3StatVest[1], g_w3StatVest[2], g_w3StatVest[3],
			g_w3StatHelm[1], g_w3StatHelm[2], g_w3StatHelm[3], g_w3DcMade, g_w3DcLooted, g_w3DcTaken);
	arrayset(g_w3StatHeal, 0, sizeof g_w3StatHeal);
	arrayset(g_w3StatVest, 0, sizeof g_w3StatVest);
	arrayset(g_w3StatHelm, 0, sizeof g_w3StatHelm);
	g_w3DcMade = 0;
	g_w3DcTaken = 0;
	g_w3DcLooted = 0;
}

/* ---- armor levels: vests and helmets 1-3 ---- */


/* "vest2" -> 0 (vest) and lv 2; "helmet3" -> 1 and 3; with levels on "vest"/"kevlar" is a level-1 vest
   and "vesthelm" a vest and a helmet (2); -1 not armor */
stock w3_armor_item(const item[], &lv)
{
	lv = 0;
	if (equali(item, "vest", 4) && item[4] >= '1' && item[4] <= '3' && !item[5])
	{
		lv = item[4] - '0';
		return 0;
	}
	if (equali(item, "helmet", 6) && item[6] >= '1' && item[6] <= '3' && !item[7])
	{
		lv = item[6] - '0';
		return 1;
	}
	if (!get_pcvar_num(g_w3cArmorLevels))
		return -1;
	lv = 1;
	if (equali(item, "vest") || equali(item, "kevlar"))
		return 0;
	if (equali(item, "vesthelm") || equali(item, "assault"))
		return 2;
	return -1;
}

/* sets levels (-1 keeps one); a new vest comes with its full armor points */
stock w3_armor_set(id, vest, helm, bool:tell = false)
{
	if (vest >= 0)
	{
		vest = clamp(vest, 0, 3);
		g_w3Vest[id] = vest;
		if (is_user_alive(id))
			cs_set_user_armor(id, W3_VEST_AP[vest], vest ? CS_ARMOR_KEVLAR : CS_ARMOR_NONE);
	}
	if (helm >= 0)
	{
		helm = clamp(helm, 0, 3);
		g_w3Helm[id] = helm;
		g_w3HelmHp[id] = W3_HELM_HP[helm];
	}
	if (tell && is_user_connected(id) && !is_user_bot(id))
	{
		if (vest == 0 && helm < 0)
			client_print(id, print_center, "Your vest is broken");
		else if (helm == 0 && vest < 0)
			client_print(id, print_center, "Your helmet is broken");
	}
	new ret;
	ExecuteForward(g_w3fwArmor, ret, id, g_w3Vest[id], g_w3Helm[id]);
}

stock w3_give_armor(id, const item[])
{
	new lv, part = w3_armor_item(item, lv);
	if (part < 0)
		return -1;
	if (!get_pcvar_num(g_w3cArmorLevels))
	{
		/* levels off: a vest is a vest */
		new CsArmorType:t;
		new cur = cs_get_user_armor(id, t);
		cs_set_user_armor(id, 100, part == 0 && !(cur > 0 && t == CS_ARMOR_VESTHELM) ? CS_ARMOR_KEVLAR : CS_ARMOR_VESTHELM);
		return 1;
	}
	if (w3_can_take_armor(id, item) == 0)
		return 0;
	if (part == 0 || part == 2)
	{
		w3_armor_set(id, part == 2 ? max(lv, g_w3Vest[id]) : lv, -1);
		g_w3StatVest[lv]++;
	}
	if (part == 1 || part == 2)
	{
		w3_armor_set(id, -1, part == 2 ? max(lv, g_w3Helm[id]) : lv);
		g_w3StatHelm[lv]++;
	}
	emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	if (!is_user_bot(id))
		client_print(id, print_center, "Vest %d   Helmet %d", g_w3Vest[id], g_w3Helm[id]);
	return 1;
}

/* worth taking: a higher level, or the same level when yours is half gone */
stock w3_can_take_armor(id, const item[])
{
	new lv, part = w3_armor_item(item, lv);
	if (part < 0)
		return -1;
	if (!get_pcvar_num(g_w3cArmorLevels))
		return 1;
	new bool:vest = lv > g_w3Vest[id] || (lv == g_w3Vest[id] && get_user_armor(id) < W3_VEST_AP[lv] / 2);
	new bool:helm = lv > g_w3Helm[id] || (lv == g_w3Helm[id] && g_w3HelmHp[id] < W3_HELM_HP[lv] * 0.5);
	switch (part)
	{
		case 0: return vest;
		case 1: return helm;
	}
	return vest || helm;
}

/* a bullet: the level's share gets through (the game's own vest still counts on the body) */
public ham_w3_trace(victim, attacker, Float:dmg, Float:dir[3], tr, bits)
{
	if (!(1 <= victim <= 32) || dmg <= 0.0 || !get_pcvar_num(g_w3cArmorLevels))
		return HAM_IGNORED;
	new hg = get_tr2(tr, TR_iHitgroup);
	if (hg == HIT_HEAD)
	{
		new lv = g_w3Helm[victim];
		if (!lv)
			return HAM_IGNORED;
		new Float:f = W3_HELM_F[lv];
		/* (head hits count 4x in the game) */
		g_w3HelmHp[victim] -= dmg * 4.0 * (1.0 - f);
		SetHamParamFloat(3, dmg * f);
		if (g_w3HelmHp[victim] <= 0.0)
			w3_armor_set(victim, -1, 0, true);
		return HAM_HANDLED;
	}
	if ((hg == HIT_CHEST || hg == HIT_STOMACH || hg == HIT_LEFTARM || hg == HIT_RIGHTARM || hg == HIT_GENERIC)
		&& g_w3Vest[victim] && get_user_armor(victim) > 0)
	{
		SetHamParamFloat(3, dmg * W3_VEST_F[g_w3Vest[victim]]);
		return HAM_HANDLED;
	}
	return HAM_IGNORED;
}

/* native ls_armor_set(id, vest, helmet) (-1 keeps one) */
public n_w3_armor_set(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= MaxClients) || !is_user_connected(id))
		return 0;
	w3_armor_set(id, get_param(2), get_param(3));
	return 1;
}

/* native ls_armor_get(id, &vest, &helmet): returns the vest's armor points */
public n_w3_armor_get(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= MaxClients))
		return 0;
	set_param_byref(2, g_w3Vest[id]);
	set_param_byref(3, g_w3Helm[id]);
	return is_user_alive(id) ? get_user_armor(id) : 0;
}

/* ---- scarce ammo: reserve ammo is loot ---- */

/* the spare rounds a gun found on the floor or in a crate comes with */
stock w3_found_reserve(csw)
{
	if (!get_pcvar_num(g_w3cAmmoScarce))
		return ammo_full(csw) / 2;
	new clip = cs_get_weapon_info(csw, CS_WEAPONINFO_GUN_CLIP_SIZE);
	return min(floatround(float(clip) * floatmax(get_pcvar_float(g_w3cAmmoMags), 0.0)), ammo_full(csw));
}

/* "762", "556", "9mm", "45", "338", "57", "50", "357", "12g", "box" or a weapon name -> a gun that uses it */
stock w3_caliber_csw(const cal[])
{
	static const NAMES[][] = { "9mm", "45", "45acp", "556", "762", "338", "57", "50", "50ae", "357", "12g", "buckshot", "box", "556box" };
	static const CSWS[] = { CSW_GLOCK18, CSW_USP, CSW_USP, CSW_M4A1, CSW_AK47, CSW_AWP, CSW_P90, CSW_DEAGLE, CSW_DEAGLE, CSW_P228, CSW_M3, CSW_M3, CSW_M249, CSW_M249 };
	for (new i = 0; i < sizeof NAMES; i++)
		if (equali(cal, NAMES[i]))
			return CSWS[i];
	new w[32];
	weapon_name(cal, w, charsmax(w));
	new csw = get_weaponid(w);
	return (1 << csw) & (W3_GUN_IDS) ? csw : 0;
}

/* "ammo:762" (two magazines' worth) or "ammo:762:60" -> the gun and the rounds */
stock w3_ammo_typed(const item[], &rounds)
{
	new cal[16], n[12];
	copyc(cal, charsmax(cal), item[5], ':');
	new c = contain(item[5], ":");
	n[0] = EOS;
	if (c >= 0)
		copy(n, charsmax(n), item[5 + c + 1]);
	new csw = w3_caliber_csw(cal);
	if (!csw)
		return 0;
	rounds = n[0] ? str_to_num(n) : 2 * cs_get_weapon_info(csw, CS_WEAPONINFO_GUN_CLIP_SIZE);
	return csw;
}

stock w3_give_ammo(id, const item[])
{
	if (equali(item, "ammo:", 5))
	{
		new rounds, csw = w3_ammo_typed(item, rounds);
		if (!csw)
			return 0;
		new have = cs_get_user_bpammo(id, csw);
		if (have >= ammo_full(csw))
			return 0;
		cs_set_user_bpammo(id, csw, min(have + rounds, ammo_full(csw)));
		emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		return 1;
	}
	if (!equali(item, "ammo") || !get_pcvar_num(g_w3cAmmoScarce))
		return -1;
	/* scarce: two magazines for each gun carried */
	new weapons[32], n, got = 0;
	get_user_weapons(id, weapons, n);
	for (new i = 0; i < n; i++)
	{
		new csw = weapons[i];
		if (!((1 << csw) & (W3_GUN_IDS)))
			continue;
		new have = cs_get_user_bpammo(id, csw);
		if (have >= ammo_full(csw))
			continue;
		cs_set_user_bpammo(id, csw, min(have + 2 * cs_get_weapon_info(csw, CS_WEAPONINFO_GUN_CLIP_SIZE), ammo_full(csw)));
		got++;
	}
	if (got)
		emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	return got ? 1 : 0;
}

/* ammo is worth taking for a gun you carry that isn't full ("ammo:x" lies there for the gun's owner) */
stock w3_can_take_ammo(id, const item[])
{
	new bool:typed = equali(item, "ammo:", 5) != 0;
	if (!typed && (!equali(item, "ammo") || !get_pcvar_num(g_w3cAmmoScarce)))
		return -1;
	new rounds, want = typed ? w3_ammo_typed(item, rounds) : 0;
	new weapons[32], n;
	get_user_weapons(id, weapons, n);
	for (new i = 0; i < n; i++)
	{
		new csw = weapons[i];
		if (!((1 << csw) & (W3_GUN_IDS)) || cs_get_user_bpammo(id, csw) >= ammo_full(csw))
			continue;
		if (!typed || cs_get_weapon_info(csw, CS_WEAPONINFO_AMMO_TYPE) == cs_get_weapon_info(want, CS_WEAPONINFO_AMMO_TYPE))
			return 1;
	}
	return 0;
}

/* ---- rarity glow: white, green, blue, purple, gold ---- */

/* an item's rarity 1-5: the highest tier the loot tables give it, else by what it is */
stock w3_item_tier(const item[])
{
	new best = 0, row[26];
	for (new t = 0; t < g_ntables; t++)
	{
		new n = ArraySize(g_ltItems[t]);
		for (new i = 0; i < n; i++)
		{
			ArrayGetArray(g_ltItems[t], i, row);
			if (equali(row, item))
				best = max(best, row[25]);
		}
	}
	if (best)
		return clamp(best, 1, 5);
	new kind = w3_heal_kind(item);
	if (kind >= 0)
		return kind == HEAL_MEDKIT ? 3 : (kind == HEAL_DRINK ? 2 : 1);
	new lv, part = w3_armor_item(item, lv);
	if (part >= 0)
		return max(lv, 1);
	if (equali(item, "ammo", 4))
		return 1;
	new w[32];
	weapon_name(item, w, charsmax(w));
	switch (get_weaponid(w))
	{
		case CSW_AWP, CSW_M249, CSW_SG550, CSW_G3SG1: return 5;
		case CSW_AK47, CSW_M4A1, CSW_SG552, CSW_AUG: return 4;
		case CSW_FAMAS, CSW_GALIL, CSW_P90, CSW_XM1014: return 3;
		case CSW_MP5NAVY, CSW_UMP45, CSW_M3, CSW_SCOUT, CSW_DEAGLE, CSW_MAC10, CSW_TMP, CSW_HEGRENADE: return 2;
	}
	return 1;
}

/* a weapon spawner: its item's rarity when ls_rarity_glow is on, else its own tier */
stock w3_spawner_tier(s, tier)
{
	if (!get_pcvar_num(g_w3cRarity) || !g_item[s][0])
		return tier;
	return w3_item_tier(g_item[s]);
}

/* a gun a player dropped (a weaponbox) glows by its rarity */
public fm_w3_setmodel(ent, const model[])
{
	if (!pev_valid(ent) || !get_pcvar_num(g_w3cRarity) || !equal(model, "models/w_", 9))
		return FMRES_IGNORED;
	new cls[12];
	pev(ent, pev_classname, cls, charsmax(cls));
	if (!equal(cls, "weaponbox"))
		return FMRES_IGNORED;
	new w[24];
	copyc(w, charsmax(w), model[9], '.');
	if (equal(w, "mp5"))
		copy(w, charsmax(w), "mp5navy");
	if (equal(w, "weaponbox") || equal(w, "backpack"))
		return FMRES_IGNORED;
	tier_glow(ent, w3_item_tier(w));
	return FMRES_IGNORED;
}

/* native ls_item_tier(const item[]) */
public n_w3_item_tier(plugin, params)
{
	new item[32];
	get_string(1, item, charsmax(item));
	return w3_item_tier(item);
}

/* native ls_glow(ent, tier): 1-5 white..gold, 0 none */
public n_w3_glow(plugin, params)
{
	new ent = get_param(1), tier = get_param(2);
	if (!pev_valid(ent))
		return 0;
	if (tier <= 0)
		render(ent, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
	else
		tier_glow(ent, tier);
	return 1;
}

/* ---- death crates: what a killed player had, in one crate (USE opens a menu) ---- */

stock w3_find_weapon(id, const wname[])
{
	new e = -1;
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", wname)) > 0)
		if (pev(e, pev_owner) == id)
			return e;
	return 0;
}

/* everything a player carries, as crate items: "ak47@30@60" (gun@clip@spare), "he", "bandage:3", "vest2@85", "helmet1@60" */
stock w3_snapshot(id, list[W3_DC_ITEMS][24])
{
	new n = 0, weapons[32], nw, usedAmmo = 0, w[32];
	get_user_weapons(id, weapons, nw);
	/* primary first, then the pistol (a shared ammo pool goes with the first) */
	for (new pass = 0; pass < 2; pass++)
		for (new i = 0; i < nw && n < W3_DC_ITEMS; i++)
		{
			new csw = weapons[i];
			if (!((1 << csw) & W3_GUN_IDS))
				continue;
			new bool:pistol = ((1 << csw) & ((1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))) != 0;
			if (pistol != (pass == 1))
				continue;
			get_weaponname(csw, w, charsmax(w));
			new ent = w3_find_weapon(id, w), clip = ent ? cs_get_weapon_ammo(ent) : 0;
			new type = cs_get_weapon_info(csw, CS_WEAPONINFO_AMMO_TYPE), bp = 0;
			if (type > 0 && type < 32 && !(usedAmmo & (1 << type)))
			{
				bp = cs_get_user_bpammo(id, csw);
				usedAmmo |= (1 << type);
			}
			formatex(list[n++], 23, "%s@%d@%d", w[7], clip, bp);
		}
	static const NADES[] = { CSW_HEGRENADE, CSW_FLASHBANG, CSW_SMOKEGRENADE };
	static const NADE_NAME[][] = { "he", "flash", "smoke" };
	for (new g = 0; g < 3; g++)
	{
		new have = 0;
		for (new i = 0; i < nw; i++)
			if (weapons[i] == NADES[g])
				have = max(cs_get_user_bpammo(id, NADES[g]), 1);
		for (new k = 0; k < have && n < W3_DC_ITEMS; k++)
			copy(list[n++], 23, NADE_NAME[g]);
	}
	for (new k = 0; k < HEAL_KINDS && n < W3_DC_ITEMS; k++)
		if (g_w3Heals[id][k] > 0)
			formatex(list[n++], 23, "%s:%d", W3_HEAL_ID[k], g_w3Heals[id][k]);
	new armor = get_user_armor(id);
	if (n < W3_DC_ITEMS && armor > 0)
	{
		if (g_w3Vest[id])
			formatex(list[n++], 23, "vest%d@%d", g_w3Vest[id], armor);
		else
			formatex(list[n++], 23, "vest@%d", armor);
	}
	if (n < W3_DC_ITEMS && g_w3Helm[id] && g_w3HelmHp[id] > 0.0)
		formatex(list[n++], 23, "helmet%d@%d", g_w3Helm[id], floatround(g_w3HelmHp[id]));
	return n;
}

public ham_w3_killed_pre(id, killer, gib)
{
	g_w3SnapN[id] = 0;
	if (get_pcvar_num(g_w3cDeathCrates) && is_user_connected(id))
		g_w3SnapN[id] = w3_snapshot(id, g_w3Snap[id]);
	return HAM_IGNORED;
}

stock w3_dc_from_death(id)
{
	new Float:o[3];
	pev(id, pev_origin, o);
	/* the game's own dropped gun goes: it's in the crate */
	new e = -1;
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "weaponbox")) > 0)
	{
		if (pev(e, pev_owner) != id)
			continue;
		new Float:wo[3];
		pev(e, pev_origin, wo);
		if (get_distance_f(wo, o) < 96.0)
			set_pev(e, pev_flags, pev(e, pev_flags) | FL_KILLME);
	}
	new name[32];
	get_user_name(id, name, charsmax(name));
	new c = w3_dc_make(o, name);
	if (c < 0)
		return;
	for (new i = 0; i < g_w3SnapN[id]; i++)
		w3_dc_add(c, g_w3Snap[id][i]);
	w3_dc_look(c);
	new ret;
	ExecuteForward(g_w3fwCrate, ret, g_w3Dc[c], id);
}

/* a new crate at a spot (the oldest goes when all are in use) */
stock w3_dc_make(const Float:at[3], const name[])
{
	new c = -1, oldest = 0;
	for (new i = 0; i < W3_DC_MAX; i++)
	{
		if (!g_w3Dc[i] || !pev_valid(g_w3Dc[i]))
		{
			c = i;
			break;
		}
		if (g_w3DcAt[i] < g_w3DcAt[oldest])
			oldest = i;
	}
	if (c < 0)
	{
		w3_dc_remove(oldest);
		c = oldest;
	}
	new ent = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"));
	if (!ent)
		return -1;
	set_pev(ent, pev_classname, "ls_death_crate");
	engfunc(EngFunc_SetModel, ent, DEFAULT_BOX);
	engfunc(EngFunc_SetSize, ent, Float:{-10.0, -10.0, 0.0}, Float:{10.0, 10.0, 12.0});
	set_pev(ent, pev_solid, SOLID_NOT);
	set_pev(ent, pev_movetype, MOVETYPE_TOSS);
	new Float:o[3];
	vcopy(at, o);
	o[2] += 8.0;
	engfunc(EngFunc_SetOrigin, ent, o);
	set_pev(ent, pev_velocity, Float:{0.0, 0.0, -50.0});
	g_w3Dc[c] = ent;
	g_w3DcN[c] = 0;
	g_w3DcAt[c] = get_gametime();
	copy(g_w3DcName[c], charsmax(g_w3DcName[]), name);
	g_w3DcMade++;
	return c;
}

stock w3_dc_add(c, const item[])
{
	if (g_w3DcN[c] >= W3_DC_ITEMS || !item[0])
		return 0;
	copy(g_w3DcItem[c][g_w3DcN[c]++], 23, item);
	return 1;
}

stock w3_dc_remove(c)
{
	new ent = g_w3Dc[c];
	g_w3Dc[c] = 0;
	g_w3DcN[c] = 0;
	if (ent && pev_valid(ent))
	{
		hint("forget c%d", ent);
		set_pev(ent, pev_flags, pev(ent, pev_flags) | FL_KILLME);
	}
}

stock w3_dc_of(ent)
{
	for (new c = 0; c < W3_DC_MAX; c++)
		if (g_w3Dc[c] == ent && ent)
			return c;
	return -1;
}

/* the item name without its @... ("ak47@30@60" -> "ak47") */
stock w3_item_base(const item[], out[], len)
{
	copyc(out, len, item, '@');
}

/* glow by the best thing in it; bots are told it's there */
stock w3_dc_look(c)
{
	new best = 1, base[24];
	for (new i = 0; i < g_w3DcN[c]; i++)
	{
		w3_item_base(g_w3DcItem[c][i], base, charsmax(base));
		best = max(best, w3_item_tier(base));
	}
	tier_glow(g_w3Dc[c], best);
	new Float:o[3];
	pev(g_w3Dc[c], pev_origin, o);
	hint("attract c%d %.0f %.0f %.0f 400 %d 0 0", g_w3Dc[c], o[0], o[1], o[2], 30 + best * 20);
}

/* "AK47 (30+60)", "Bandage x3", "Vest 2 (85)" */
stock w3_item_label(const item[], out[], len)
{
	new base[24], a[12], b[12];
	w3_item_base(item, base, charsmax(base));
	new at = contain(item, "@");
	a[0] = EOS; b[0] = EOS;
	if (at >= 0)
	{
		copyc(a, charsmax(a), item[at + 1], '@');
		new at2 = contain(item[at + 1], "@");
		if (at2 >= 0)
			copy(b, charsmax(b), item[at + 1 + at2 + 1]);
	}
	new kind = w3_heal_kind(base), lv, part = w3_armor_item(base, lv);
	if (kind >= 0)
		formatex(out, len, "%s x%d", W3_HEAL_TITLE[kind], w3_item_count(base));
	else if (equali(base, "vest", 4) && lv)
		formatex(out, len, "Vest %d (%s)", lv, a);
	else if (equali(base, "vest", 4))
		formatex(out, len, "Vest (%s)", a);
	else if (part == 1)
		formatex(out, len, "Helmet %d", lv);
	else if (equali(base, "he"))
		copy(out, len, "HE grenade");
	else if (equali(base, "flash"))
		copy(out, len, "Flashbang");
	else if (equali(base, "smoke"))
		copy(out, len, "Smoke grenade");
	else if (equali(base, "ammo", 4))
		copy(out, len, "Ammo");
	else
	{
		copy(out, len, base);
		if (equali(out, "mp5navy"))
			copy(out, len, "mp5");
		strtoupper(out);
		if (a[0])
			format(out, len, "%s (%s+%s)", out, a, b[0] ? b : "0");
	}
}

/* takes one item for a player: 1 taken, 0 not (no room, not better); swap: a gun in that slot goes into the crate */
stock w3_dc_take(id, c, i, bool:swap)
{
	if (i < 0 || i >= g_w3DcN[c] || !is_user_alive(id))
		return 0;
	new item[24], base[24];
	copy(item, charsmax(item), g_w3DcItem[c][i]);
	w3_item_base(item, base, charsmax(base));
	new ret;
	ExecuteForward(g_w3fwCrateTake, ret, g_w3Dc[c], id, item);
	if (ret >= PLUGIN_HANDLED)
		return 0;
	new at = contain(item, "@"), n1 = 0, n2 = 0;
	if (at >= 0)
	{
		n1 = str_to_num(item[at + 1]);
		new at2 = contain(item[at + 1], "@");
		if (at2 >= 0)
			n2 = str_to_num(item[at + 1 + at2 + 1]);
	}
	new w[32];
	weapon_name(base, w, charsmax(w));
	new csw = get_weaponid(w);
	new bool:done = false, back[24];
	back[0] = EOS;
	if (csw && ((1 << csw) & W3_GUN_IDS))
	{
		/* the slot's gun now, if any */
		new bool:prim = ((1 << csw) & ((1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))) == 0;
		new weapons[32], nw, old = 0;
		get_user_weapons(id, weapons, nw);
		for (new k = 0; k < nw; k++)
		{
			new o = weapons[k];
			if (!((1 << o) & W3_GUN_IDS))
				continue;
			new bool:op = ((1 << o) & ((1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))) == 0;
			if (op == prim)
				old = o;
		}
		if (old && !swap)
			return 0;
		if (old)
		{
			new on[32];
			get_weaponname(old, on, charsmax(on));
			new oe = w3_find_weapon(id, on);
			formatex(back, charsmax(back), "%s@%d@0", on[7], oe ? cs_get_weapon_ammo(oe) : 0);
			w3_strip_weapon(id, on);
		}
		new ent = give_item(id, w);
		if (ent > 0)
		{
			cs_set_weapon_ammo(ent, n1);
			cs_set_user_bpammo(id, csw, min(cs_get_user_bpammo(id, csw) + n2, ammo_full(csw)));
			emit_sound(id, CHAN_ITEM, "items/gunpickup2.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
			done = true;
		}
	}
	else if (w3_heal_kind(base) >= 0)
	{
		/* as many as fit; the rest stays */
		new kind = w3_heal_kind(base), have = w3_item_count(base);
		new got = get_pcvar_num(g_w3cHealCarry) ? w3_heal_give(id, kind, have) : (give_thing(id, base) ? have : 0);
		if (got <= 0)
			return 0;
		if (got < have)
		{
			formatex(g_w3DcItem[c][i], 23, "%s:%d", W3_HEAL_ID[kind], have - got);
			g_w3DcTaken++;
			return 1;
		}
		done = true;
	}
	else if (equali(base, "vest", 4) || equali(base, "helmet", 6))
	{
		new lv, part = w3_armor_item(base, lv);
		new bool:levels = part >= 0 && get_pcvar_num(g_w3cArmorLevels) != 0;
		if (levels)
		{
			/* better than yours, or swap: yours goes into the crate */
			new bool:better = part == 0 ? (lv > g_w3Vest[id] || (lv == g_w3Vest[id] && n1 > get_user_armor(id)))
				: (lv > g_w3Helm[id] || (lv == g_w3Helm[id] && float(n1) > g_w3HelmHp[id]));
			if (!better && !swap)
				return 0;
			if (part == 0)
			{
				if (g_w3Vest[id] && get_user_armor(id) > 0)
					formatex(back, charsmax(back), "vest%d@%d", g_w3Vest[id], get_user_armor(id));
				w3_armor_set(id, lv, -1);
				if (n1 > 0)
					cs_set_user_armor(id, n1, CS_ARMOR_KEVLAR);
			}
			else
			{
				if (g_w3Helm[id] && g_w3HelmHp[id] > 0.0)
					formatex(back, charsmax(back), "helmet%d@%d", g_w3Helm[id], floatround(g_w3HelmHp[id]));
				w3_armor_set(id, -1, lv);
				if (n1 > 0)
					g_w3HelmHp[id] = float(n1);
			}
		}
		else
		{
			if (get_user_armor(id) >= max(n1, 1) && !swap)
				return 0;
			cs_set_user_armor(id, max(n1, 1), CS_ARMOR_KEVLAR);
		}
		emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		done = true;
	}
	else
	{
		if (csw == CSW_HEGRENADE || csw == CSW_FLASHBANG || csw == CSW_SMOKEGRENADE)
		{
			/* (the game's carry limits: one HE, two flashes, one smoke) */
			if (user_has_weapon(id, csw) && cs_get_user_bpammo(id, csw) >= (csw == CSW_FLASHBANG ? 2 : 1))
				return 0;
		}
		else if (!can_take(id, base))
			return 0;
		done = give_thing(id, base) != 0;
	}
	if (!done)
		return 0;
	/* out of the crate (and what was swapped in) */
	if (back[0])
		copy(g_w3DcItem[c][i], 23, back);
	else
	{
		for (new k = i + 1; k < g_w3DcN[c]; k++)
			copy(g_w3DcItem[c][k - 1], 23, g_w3DcItem[c][k]);
		g_w3DcN[c]--;
	}
	g_w3DcTaken++;
	return 1;
}

/* the rounds in a crate's gun (magazine and spare) for a gun of the same ammo the player carries; the gun stays */
stock w3_dc_take_ammo(id, c, i)
{
	new item[24], base[24], w[32];
	copy(item, charsmax(item), g_w3DcItem[c][i]);
	w3_item_base(item, base, charsmax(base));
	weapon_name(base, w, charsmax(w));
	new csw = get_weaponid(w);
	if (!csw || !((1 << csw) & W3_GUN_IDS))
		return 0;
	new at = contain(item, "@"), n1 = 0, n2 = 0;
	if (at < 0)
		return 0;
	n1 = str_to_num(item[at + 1]);
	new at2 = contain(item[at + 1], "@");
	if (at2 >= 0)
		n2 = str_to_num(item[at + 1 + at2 + 1]);
	if (n1 + n2 <= 0)
		return 0;
	new type = cs_get_weapon_info(csw, CS_WEAPONINFO_AMMO_TYPE), weapons[32], nw;
	get_user_weapons(id, weapons, nw);
	for (new k = 0; k < nw; k++)
	{
		new o = weapons[k];
		if (!((1 << o) & W3_GUN_IDS) || cs_get_weapon_info(o, CS_WEAPONINFO_AMMO_TYPE) != type)
			continue;
		new have = cs_get_user_bpammo(id, o), room = ammo_full(o) - have;
		if (room <= 0)
			return 0;
		new take = min(room, n1 + n2);
		cs_set_user_bpammo(id, o, have + take);
		/* (what's left stays in the gun: from the spare first, then the magazine) */
		new left = n1 + n2 - take;
		formatex(g_w3DcItem[c][i], 23, "%s@%d@%d", base, min(left, n1), max(left - n1, 0));
		emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		g_w3DcTaken++;
		return 1;
	}
	return 0;
}

stock w3_strip_weapon(id, const wname[])
{
	new csw = get_weaponid(wname), ent = w3_find_weapon(id, wname);
	if (!ent)
		return 0;
	if (get_user_weapon(id) == csw)
		ExecuteHamB(Ham_Weapon_RetireWeapon, ent);
	if (!ExecuteHamB(Ham_RemovePlayerItem, id, ent))
		return 0;
	ExecuteHamB(Ham_Item_Kill, ent);
	set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << csw));
	return 1;
}

/* after a take: empty crates go, the rest glow by what's left */
stock w3_dc_after(c)
{
	if (c >= 0 && c < W3_DC_MAX)
		g_w3DcLooted++;
	if (g_w3DcN[c] <= 0)
		w3_dc_remove(c);
	else
		w3_dc_look(c);
}

/* the crate a player is looking at, within reach (-1 none) */
stock w3_dc_aimed(id)
{
	new Float:eye[3], Float:fwd[3], Float:ang[3], Float:o[3];
	pev(id, pev_origin, eye);
	new Float:vo[3];
	pev(id, pev_view_ofs, vo);
	eye[2] += vo[2];
	pev(id, pev_v_angle, ang);
	engfunc(EngFunc_MakeVectors, ang);
	global_get(glb_v_forward, fwd);
	new best = -1, Float:bd = 90.0;
	for (new c = 0; c < W3_DC_MAX; c++)
	{
		if (!g_w3Dc[c] || !pev_valid(g_w3Dc[c]))
			continue;
		pev(g_w3Dc[c], pev_origin, o);
		o[2] += 6.0;
		new Float:d = get_distance_f(eye, o);
		if (d > bd)
			continue;
		if (d > 24.0)
		{
			new Float:dot = 0.0;
			for (new k = 0; k < 3; k++)
				dot += (o[k] - eye[k]) / d * fwd[k];
			if (dot < 0.6)
				continue;
		}
		bd = d;
		best = c;
	}
	return best;
}

public fm_w3_prethink(id)
{
	if (!is_user_alive(id) || is_user_bot(id))
		return FMRES_IGNORED;
	new b = pev(id, pev_button), ob = pev(id, pev_oldbuttons);
	if ((b & IN_USE) && !(ob & IN_USE))
	{
		new c = w3_dc_aimed(id);
		if (c >= 0)
			w3_dc_menu(id, c);
	}
	return FMRES_IGNORED;
}

stock w3_crate_hint(p)
{
	if (w3_dc_aimed(p) >= 0)
		hud_add(p, false, "Press USE to search the crate");
}

stock w3_dc_menu(id, c)
{
	if (c < 0 || !g_w3Dc[c])
		return;
	new title[64], line[64], info[8];
	formatex(title, charsmax(title), "%s's crate", g_w3DcName[c]);
	new m = menu_create(title, "menu_w3_crate");
	for (new i = 0; i < g_w3DcN[c]; i++)
	{
		w3_item_label(g_w3DcItem[c][i], line, charsmax(line));
		num_to_str(i, info, charsmax(info));
		menu_additem(m, line, info);
	}
	menu_additem(m, "Take everything that fits", "all");
	g_w3MenuDc[id] = c + 1;
	menu_display(id, m);
}

public menu_w3_crate(id, menu, item)
{
	new c = g_w3MenuDc[id] - 1;
	if (item < 0 || c < 0 || !g_w3Dc[c] || !is_user_alive(id))
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[8], acc, cb;
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb);
	menu_destroy(menu);
	new Float:o[3], Float:po[3];
	pev(g_w3Dc[c], pev_origin, o);
	pev(id, pev_origin, po);
	if (get_distance_f(o, po) > 120.0)
		return PLUGIN_HANDLED;
	if (equal(info, "all"))
	{
		/* (a taken item moves the rest up; a part-taken stack is tried again and then skipped) */
		for (new i = 0; i < g_w3DcN[c]; )
			if (!w3_dc_take(id, c, i, false))
			{
				/* a gun left behind: its rounds, for a gun of yours that takes them */
				w3_dc_take_ammo(id, c, i);
				i++;
			}
	}
	else if (!w3_dc_take(id, c, str_to_num(info), true))
		client_print(id, print_center, "You can't carry that");
	w3_dc_after(c);
	if (g_w3Dc[c])
		w3_dc_menu(id, c);
	return PLUGIN_HANDLED;
}

/* a bot by a crate takes what helps: a better main gun (swapping), a pistol, heals, better armor, grenades */
stock w3_bot_crate(p)
{
	for (new c = 0; c < W3_DC_MAX; c++)
	{
		if (!g_w3Dc[c] || !pev_valid(g_w3Dc[c]))
			continue;
		new Float:o[3];
		pev(g_w3Dc[c], pev_origin, o);
		if (dist2d(g_porg[p], o) > 80.0 || floatabs(g_porg[p][2] - o[2]) > 90.0)
			continue;
		new took = 0;
		for (new i = 0; i < g_w3DcN[c]; )
		{
			new base[24];
			w3_item_base(g_w3DcItem[c][i], base, charsmax(base));
			new bool:swap = w3_bot_wants_swap(p, base);
			new before = g_w3DcN[c];
			if (w3_dc_take(p, c, i, swap))
			{
				took++;
				/* (a swap or a part-taken stack leaves something at i) */
				if (g_w3DcN[c] == before)
					i++;
			}
			else
			{
				took += w3_dc_take_ammo(p, c, i);
				i++;
			}
		}
		if (took)
		{
			w3_dc_after(c);
			comp_debug("bot %d took %d items from a death crate", p, took);
		}
		return;
	}
}

/* a gun in the crate of a higher rarity than the bot's own in that slot */
stock bool:w3_bot_wants_swap(p, const base[])
{
	new w[32];
	weapon_name(base, w, charsmax(w));
	new csw = get_weaponid(w);
	if (!csw || !((1 << csw) & W3_GUN_IDS))
		return false;
	new bool:prim = ((1 << csw) & ((1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))) == 0;
	new weapons[32], nw, on[32];
	get_user_weapons(p, weapons, nw);
	for (new k = 0; k < nw; k++)
	{
		new o = weapons[k];
		if (!((1 << o) & W3_GUN_IDS))
			continue;
		new bool:op = ((1 << o) & ((1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE))) == 0;
		if (op != prim)
			continue;
		get_weaponname(o, on, charsmax(on));
		return w3_item_tier(base) > w3_item_tier(on[7]);
	}
	return false;
}

/* native ls_crate_make(const Float:origin[3], const items[] = "", const name[] = "") -> crate entity */
public n_w3_crate_make(plugin, params)
{
	new Float:o[3], items[256], name[32], tok[32];
	get_array_f(1, o, 3);
	get_string(2, items, charsmax(items));
	get_string(3, name, charsmax(name));
	new c = w3_dc_make(o, name[0] ? name : "A");
	if (c < 0)
		return 0;
	while (items[0])
	{
		strtok2(items, tok, charsmax(tok), items, charsmax(items), ' ', TRIM_FULL);
		if (!tok[0])
			break;
		w3_dc_add(c, tok);
	}
	w3_dc_look(c);
	return g_w3Dc[c];
}

/* native ls_crate_add(ent, const item[]) */
public n_w3_crate_add(plugin, params)
{
	new c = w3_dc_of(get_param(1)), item[24];
	if (c < 0)
		return 0;
	get_string(2, item, charsmax(item));
	new r = w3_dc_add(c, item);
	w3_dc_look(c);
	return r;
}

/* native ls_crate_count(ent) */
public n_w3_crate_count(plugin, params)
{
	new c = w3_dc_of(get_param(1));
	return c < 0 ? 0 : g_w3DcN[c];
}

/* native ls_crate_item(ent, i, out[], len) */
public n_w3_crate_item(plugin, params)
{
	new c = w3_dc_of(get_param(1)), i = get_param(2);
	if (c < 0 || i < 0 || i >= g_w3DcN[c])
		return 0;
	set_string(3, g_w3DcItem[c][i], get_param(4));
	return 1;
}

/* native ls_crate_open(id, ent): the crate's menu */
public n_w3_crate_open(plugin, params)
{
	new id = get_param(1), c = w3_dc_of(get_param(2));
	if (c < 0 || !(1 <= id <= MaxClients) || !is_user_alive(id))
		return 0;
	w3_dc_menu(id, c);
	return 1;
}

/* ---- the match HUD: players left, kills, zone, armor, heals, kill feed; the dead watch their killer ---- */

stock w3_alive_count()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (g_alive[p])
			n++;
	return n;
}

/* "Zone closes in 1:20", "ZONE CLOSING 0:12", "" (no zone or not started); returns the zone's slot (-1 none) */
stock w3_zone_text(out[], len)
{
	out[0] = EOS;
	new z = -1;
	for (new k = 0; k < g_nslots && z < 0; k++)
		if (g_ent[k] && g_kind[k] == K_ZONE && g_on[k])
			z = k;
	if (z < 0)
		return -1;
	if (g_pf[z][Z_T1] > g_pf[z][Z_T0] && g_now < g_pf[z][Z_T1])
	{
		new t = floatround(g_pf[z][Z_T1] - g_now);
		formatex(out, len, "ZONE CLOSING %d:%02d", t / 60, t % 60);
		return z;
	}
	for (new k = 0; k < g_nslots; k++)
		if (g_ent[k] && g_kind[k] == K_SHRINKER && g_aux[k] == z && g_state[k] == 1)
		{
			new t = floatround(floatmax(g_pf[k][S_NEXT] - g_now, 0.0));
			formatex(out, len, "Zone closes in %d:%02d", t / 60, t % 60);
			return z;
		}
	return z;
}

/* the next circle: how far outside it a player is ("Next zone: 120 m to your left"), and its middle on their
   radar (a spare player slot's dot: unverified on the stock client) */
stock w3_next_zone(p, z, out[], len)
{
	out[0] = EOS;
	if (z < 0 || g_pf[z][Z_TOR] >= g_val[z] - 1.0)
		return;
	new Float:d = dist2d(g_porg[p], g_vb[z]);
	if (d > g_pf[z][Z_TOR])
	{
		new dir[24];
		dir_word(p, g_vb[z], dir, charsmax(dir));
		formatex(out, len, "Next zone: %d m %s", floatround((d - g_pf[z][Z_TOR]) * 0.0254), dir);
	}
	else
		copy(out, len, "Inside the next zone");
	if (!g_msgRadar)
		return;
	new spare = 0;
	for (new i = MaxClients; i >= 1 && !spare; i--)
		if (!is_user_connected(i))
			spare = i;
	if (!spare)
		return;
	message_begin(MSG_ONE_UNRELIABLE, g_msgRadar, _, p);
	write_byte(spare);
	write_coord(floatround(g_vb[z][0]));
	write_coord(floatround(g_vb[z][1]));
	write_coord(floatround(g_porg[p][2]));
	message_end();
}

stock w3_feed_add(const text[])
{
	for (new i = W3_FEED - 1; i > 0; i--)
	{
		copy(g_w3Feed[i], charsmax(g_w3Feed[]), g_w3Feed[i - 1]);
		g_w3FeedAt[i] = g_w3FeedAt[i - 1];
	}
	copy(g_w3Feed[0], charsmax(g_w3Feed[]), text);
	g_w3FeedAt[0] = get_gametime();
}

stock w3_match_death(id, killer)
{
	if (!get_pcvar_num(g_w3cMatchHud))
		return;
	new vn[32], kn[32], wn[24], line[96];
	get_user_name(id, vn, charsmax(vn));
	new left = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (p != id && is_user_alive(p))
			left++;
	if (1 <= killer <= MaxClients && killer != id && is_user_connected(killer))
	{
		g_w3Kills[killer]++;
		g_w3Killer[id] = killer;
		get_user_name(killer, kn, charsmax(kn));
		get_weaponname(get_user_weapon(killer), wn, charsmax(wn));
		strtoupper(wn[7]);
		formatex(line, charsmax(line), "%s  [%s]  %s   (%d left)", kn, wn[7], vn, left);
	}
	else
	{
		g_w3Killer[id] = 0;
		formatex(line, charsmax(line), "%s is out   (%d left)", vn, left);
	}
	w3_feed_add(line);
	/* the dead watch who got them (a mode can choose someone else with ls_spectate) */
	if (!is_user_bot(id) && g_w3Killer[id])
	{
		remove_task(W3_TASK_SPEC + id);
		set_task(3.5, "task_w3_spec", W3_TASK_SPEC + id);
	}
}

public task_w3_spec(tid)
{
	new id = tid - W3_TASK_SPEC;
	if (!is_user_connected(id) || is_user_alive(id))
		return;
	new t = g_w3Killer[id];
	if (t && is_user_alive(t))
		w3_spectate(id, t);
}

/* a dead player watches someone, first person */
stock w3_spectate(id, target)
{
	if (!is_user_connected(id) || is_user_alive(id) || !is_user_alive(target))
		return 0;
	new name[32];
	get_user_name(target, name, charsmax(name));
	/* the game's own observer commands (they move its observer target), then the view itself */
	engclient_cmd(id, "specmode", "4");
	engclient_cmd(id, "follow", name);
	set_pev(id, pev_iuser1, 4);
	set_pev(id, pev_iuser2, target);
	return 1;
}

stock w3_match_hud()
{
	new zone[48], top[256], side[512];
	new z = w3_zone_text(zone, charsmax(zone));
	new alive = w3_alive_count(), Float:now = get_gametime();
	/* the kill feed: lines from the last 8 s */
	new feed[400], fl = 0;
	for (new i = 0; i < W3_FEED; i++)
		if (g_w3Feed[i][0] && now - g_w3FeedAt[i] < 8.0)
			fl += formatex(feed[fl], charsmax(feed) - fl, "%s^n", g_w3Feed[i]);
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || g_bot[p])
			continue;
		new len = formatex(top, charsmax(top), "ALIVE %d   KILLS %d", alive, g_w3Kills[p]);
		if (g_w3HudText[0])
			len += formatex(top[len], charsmax(top) - len, "   %s", g_w3HudText);
		else if (zone[0])
			len += formatex(top[len], charsmax(top) - len, "   %s", zone);
		if (g_alive[p])
		{
			new extra[160], el = 0, nz[64];
			w3_next_zone(p, z, nz, charsmax(nz));
			if (nz[0])
				el += formatex(extra[el], charsmax(extra) - el, "%s", nz);
			if (get_pcvar_num(g_w3cArmorLevels) && (g_w3Vest[p] || g_w3Helm[p]))
				el += formatex(extra[el], charsmax(extra) - el, "%sVest %d  Helmet %d", el ? "    " : "", g_w3Vest[p], g_w3Helm[p]);
			if (get_pcvar_num(g_w3cHealCarry) && w3_heals_total(p))
				el += formatex(extra[el], charsmax(extra) - el, "%sBandage %d  Medkit %d  Drink %d", el ? "    " : "",
					g_w3Heals[p][HEAL_BANDAGE], g_w3Heals[p][HEAL_MEDKIT], g_w3Heals[p][HEAL_DRINK]);
			if (el)
				formatex(top[len], charsmax(top) - len, "^n%s", extra);
		}
		set_hudmessage(255, 220, 120, -1.0, 0.02, 0, 0.0, 1.1, 0.0, 0.1, -1);
		ShowSyncHudMsg(p, g_w3SyncTop, "%s", top);
		side[0] = EOS;
		new sl = 0;
		if (feed[0])
			sl += formatex(side[sl], charsmax(side) - sl, "%s", feed);
		sl += w3_side_extra(p, side[sl], charsmax(side) - sl);
		if (side[0])
		{
			set_hudmessage(220, 220, 220, 0.70, 0.16, 0, 0.0, 1.1, 0.0, 0.1, -1);
			ShowSyncHudMsg(p, g_w3SyncSide, "%s", side);
		}
	}
}

/* more for the right side (the squad list, when squads are on) */
stock w3_side_extra(p, out[], len)
{
	out[0] = EOS;
	if (g_w3SquadMax <= 1 || !g_w3Squad[p])
		return 0;
	new n = formatex(out, len, "^nSquad %d^n", g_w3Squad[p]), name[24];
	for (new q = 1; q <= MaxClients; q++)
	{
		if (q == p || g_w3Squad[q] != g_w3Squad[p] || !is_user_connected(q))
			continue;
		get_user_name(q, name, charsmax(name));
		if (is_user_alive(q) && g_w3Down[q])
			n += formatex(out[n], len - n, "%s  DOWN^n", name);
		else if (is_user_alive(q))
			n += formatex(out[n], len - n, "%s  %d^n", name, get_user_health(q));
		else
			n += formatex(out[n], len - n, "%s  out^n", name);
	}
	return n;
}

/* native ls_match_hud(bool:on) */
public n_w3_match_hud(plugin, params)
{
	set_pcvar_num(g_w3cMatchHud, get_param(1) ? 1 : 0);
	return 1;
}

/* native ls_match_hud_text(const text[]): the mode's words in place of the zone timer ("" = the zone) */
public n_w3_match_hud_text(plugin, params)
{
	get_string(1, g_w3HudText, charsmax(g_w3HudText));
	return 1;
}

/* native ls_feed(const text[]) */
public n_w3_feed(plugin, params)
{
	new t[96];
	get_string(1, t, charsmax(t));
	w3_feed_add(t);
	return 1;
}

/* native ls_kills(id): kills this round */
public n_w3_kills(plugin, params)
{
	new id = get_param(1);
	return 1 <= id <= MaxClients ? g_w3Kills[id] : 0;
}

/* native ls_spectate(id, target) */
public n_w3_spectate(plugin, params)
{
	return w3_spectate(get_param(1), get_param(2));
}

/* ---- drop-in: start high over a landing spot, fall slowly and steer, land with no fall damage ---- */

/* room to fall from 250 units above a spot */
stock bool:w3_open_above(const Float:p[3])
{
	new Float:a[3], Float:b[3], Float:hit[3];
	vcopy(p, a);
	a[2] += 40.0;
	vcopy(a, b);
	b[2] += 300.0;
	engfunc(EngFunc_TraceHull, a, b, IGNORE_MONSTERS, HULL_HUMAN, 0, 0);
	get_tr2(0, TR_vecEndPos, hit);
	return !get_tr2(0, TR_StartSolid) && hit[2] - a[2] >= 250.0;
}

stock w3_marker_layer(e)
{
	new v[16];
	if (!w3_marker_str(e, "layer", v, charsmax(v)))
		return -1;
	if (equali(v, "below"))
		return 0;
	if (equali(v, "surface"))
		return 1;
	if (equali(v, "high"))
		return 2;
	return -1;
}

stock bool:w3_marker_str(ent, const key[], out[], len)
{
	out[0] = EOS;
	if (ent <= 0 || ent >= MAX_ENTS || g_mkKeys[ent] == Invalid_Trie)
		return false;
	return bool:TrieGetString(g_mkKeys[ent], key, out, len);
}

/* the open landing spot (a br_drop marker, else a respawn point) nearest a point; spots someone already
   drops on count 1500 u further; layer 0 below / 1 surface / 2 high takes markers tagged so first */
stock bool:w3_drop_spot(const Float:p[3], Float:out[3], layer = -1)
{
	new best = 0, Float:bd = 999999.0, Float:o[3];
	new bool:tagged = false;
	if (layer >= 0)
		for (new e = w3_marker_next("br_drop", 0); e && !tagged; e = w3_marker_next("br_drop", e))
			tagged = w3_marker_layer(e) == layer;
	for (new e = w3_marker_next("br_drop", 0); e; e = w3_marker_next("br_drop", e))
	{
		if (tagged && w3_marker_layer(e) != layer)
			continue;
		pev(e, pev_origin, o);
		new Float:d = get_distance_f(o, p);
		for (new i = 0; i < g_w3NDrop; i++)
			if (floatabs(o[0] - g_w3DropSpots[i][0]) < 300.0 && floatabs(o[1] - g_w3DropSpots[i][1]) < 300.0)
				d += 1500.0;
		if (d < bd && w3_open_above(o))
		{
			bd = d;
			best = e;
		}
	}
	if (!best)
		for (new s = 0; s < g_nslots; s++)
		{
			if (!g_ent[s] || g_kind[s] != K_RESPAWN)
				continue;
			pev(g_ent[s], pev_origin, o);
			new Float:d = get_distance_f(o, p);
			if (d < bd && w3_open_above(o))
			{
				bd = d;
				best = g_ent[s];
			}
		}
	if (!best)
		return false;
	pev(best, pev_origin, out);
	return true;
}

/* the next ls_marker of a kind after ent (0 to begin) */
stock w3_marker_next(const kind[], start)
{
	new k[32];
	for (new e = start + 1; e < MAX_ENTS; e++)
	{
		if (g_mkKeys[e] == Invalid_Trie || !pev_valid(e))
			continue;
		pev(e, pev_netname, k, charsmax(k));
		if (equal(k, kind))
			return e;
	}
	return 0;
}

/* a spread-out landing spot: of a few random markers (or respawn points), the one furthest from others' drops */
stock bool:w3_spread_spot(Float:out[3])
{
	new pts[96], n = 0;
	for (new e = w3_marker_next("br_drop", 0); e && n < sizeof pts; e = w3_marker_next("br_drop", e))
		pts[n++] = e;
	if (!n)
		for (new s = 0; s < g_nslots && n < sizeof pts; s++)
			if (g_ent[s] && g_kind[s] == K_RESPAWN)
				pts[n++] = g_ent[s];
	if (!n)
		return false;
	new best = pts[random(n)], Float:bestd = -1.0, Float:o[3];
	for (new t = 0; t < 6; t++)
	{
		new e = pts[random(n)];
		pev(e, pev_origin, o);
		new Float:d = 99999.0;
		for (new i = 0; i < g_w3NDrop; i++)
			d = floatmin(d, floatsqroot((o[0] - g_w3DropSpots[i][0]) * (o[0] - g_w3DropSpots[i][0]) + (o[1] - g_w3DropSpots[i][1]) * (o[1] - g_w3DropSpots[i][1])));
		if (d > bestd)
		{
			bestd = d;
			best = e;
		}
	}
	pev(best, pev_origin, out);
	return true;
}

/* drops a player in over a spot (0 0 0: a spread-out landing spot); false if there's no sky over it */
stock bool:w3_drop_in(id, const Float:spot[3], Float:height)
{
	if (!is_user_alive(id))
		return false;
	new Float:land[3];
	if (spot[0] == 0.0 && spot[1] == 0.0 && spot[2] == 0.0)
	{
		if (!w3_spread_spot(land))
			pev(id, pev_origin, land);
	}
	else
		vcopy(spot, land);
	/* nobody drops on top of somebody else (they'd hang in the air on each other) */
	for (new tries = 0; tries < 8; tries++)
	{
		new bool:clash = false;
		for (new i = 0; i < g_w3NDrop && !clash; i++)
			clash = floatabs(land[0] - g_w3DropSpots[i][0]) < 96.0 && floatabs(land[1] - g_w3DropSpots[i][1]) < 96.0;
		if (!clash)
			break;
		land[0] += random_float(-160.0, 160.0);
		land[1] += random_float(-160.0, 160.0);
	}
	if (g_w3NDrop < sizeof g_w3DropSpots)
	{
		g_w3DropSpots[g_w3NDrop][0] = land[0];
		g_w3DropSpots[g_w3NDrop][1] = land[1];
		g_w3NDrop++;
	}
	/* as high as the sky allows, with room for a player */
	if (height <= 0.0)
		height = get_pcvar_float(g_w3cDropHeight);
	new Float:a[3], Float:b[3], Float:hit[3];
	vcopy(land, a);
	a[2] += 40.0;
	vcopy(a, b);
	b[2] += height;
	engfunc(EngFunc_TraceHull, a, b, IGNORE_MONSTERS, HULL_HUMAN, id, 0);
	get_tr2(0, TR_vecEndPos, hit);
	if (get_tr2(0, TR_StartSolid) || hit[2] - a[2] < 250.0)
		return false;
	hit[2] -= 16.0;
	engfunc(EngFunc_SetOrigin, id, hit);
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0});
	/* (the "on the ground" flag from where they stood stays until the next physics frame: cleared now,
	   or the very next think would count as the landing) */
	set_pev(id, pev_flags, pev(id, pev_flags) & ~FL_ONGROUND);
	set_pev(id, pev_groundentity, 0);
	set_user_gravity(id, 0.2);
	g_w3Falling[id] = true;
	g_w3FallAt[id] = get_gametime();
	if (!is_user_bot(id))
		toast_add(id, 255, 200, 60, 4.0, "Pick your landing! (steer as you fall)", false);
	return true;
}

stock w3_land(id, bool:tell)
{
	if (!g_w3Falling[id])
		return;
	g_w3Falling[id] = false;
	if (is_user_alive(id))
		set_user_gravity(id, 1.0);
	if (tell)
	{
		new ret;
		ExecuteForward(g_w3fwLanded, ret, id);
	}
}

public fm_w3_postthink(id)
{
	if (!g_w3Falling[id] || !is_user_alive(id))
		return FMRES_IGNORED;
	/* landed: standing on something, a moment after the drop began, no longer falling */
	if ((pev(id, pev_flags) & FL_ONGROUND) && get_gametime() - g_w3FallAt[id] > 0.5)
	{
		new Float:v[3];
		pev(id, pev_velocity, v);
		if (v[2] > -50.0)
		{
			w3_land(id, true);
			return FMRES_IGNORED;
		}
	}
	/* bots come straight down on the spot they picked (people steer) */
	if (is_user_bot(id))
	{
		new Float:v[3];
		pev(id, pev_velocity, v);
		v[0] = 0.0;
		v[1] = 0.0;
		set_pev(id, pev_velocity, v);
	}
	return FMRES_IGNORED;
}

/* falling in: nothing hurts (not a hit, not the landing) */
public ham_w3_hurt_pre(victim, inflictor, attacker, Float:dmg, bits)
{
	if (1 <= victim <= 32 && g_w3Falling[victim])
		return HAM_SUPERCEDE;
	/* the squad lobby: nobody gets hurt */
	if (g_w3Lobby && 1 <= victim <= 32)
		return HAM_SUPERCEDE;
	/* squad mates don't hurt each other */
	if (g_w3SquadMax > 1 && 1 <= attacker <= 32 && attacker != victim && 1 <= victim <= 32 && g_w3Squad[victim]
		&& g_w3Squad[victim] == g_w3Squad[attacker] && !get_pcvar_num(g_w3cSquadFF))
		return HAM_SUPERCEDE;
	return HAM_IGNORED;
}

/* native ls_drop_in(id, const Float:spot[3] = {0.0, 0.0, 0.0}, Float:height = 0.0) */
public n_w3_drop_in(plugin, params)
{
	new id = get_param(1), Float:spot[3];
	get_array_f(2, spot, 3);
	if (!(1 <= id <= MaxClients))
		return 0;
	return w3_drop_in(id, spot, get_param_f(3)) ? 1 : 0;
}

/* native bool:ls_dropping(id) */
public n_w3_dropping(plugin, params)
{
	new id = get_param(1);
	return 1 <= id <= MaxClients && g_w3Falling[id];
}

/* native bool:ls_drop_spot(const Float:near[3], Float:out[3], const layer[] = "") */
public n_w3_drop_spot(plugin, params)
{
	new Float:near[3], Float:out[3], l[12];
	get_array_f(1, near, 3);
	get_string(3, l, charsmax(l));
	new layer = equali(l, "below") ? 0 : (equali(l, "surface") ? 1 : (equali(l, "high") ? 2 : -1));
	if (!w3_drop_spot(near, out, layer))
		return 0;
	set_array_f(2, out, 3);
	return 1;
}

/* native ls_drop_reset(): a new game (landing spots taken are forgotten; anyone falling lands) */
public n_w3_drop_reset(plugin, params)
{
	g_w3NDrop = 0;
	for (new p = 1; p <= 32; p++)
		if (g_w3Falling[p])
			w3_land(p, false);
	return 1;
}

/* ---- squads: who's with whom, a lobby to team up, markers, the list, friendly fire ---- */

stock w3_precache()
{
	g_w3SprMark = precache_model("sprites/radio.spr");
}

/* on the same side: the same squad when squads are on, else the same team (for revives and the like) */
stock bool:w3_friends(a, b)
{
	if (g_w3SquadMax > 1 && 1 <= a <= 32 && 1 <= b <= 32 && g_w3Squad[a] && g_w3Squad[b])
		return g_w3Squad[a] == g_w3Squad[b];
	return get_user_team(a) == get_user_team(b);
}

/* (the downed state's own forwards, for the squad list: DOWN) */
public ls_on_down(id, attacker)
{
	if (1 <= id <= 32)
		g_w3Down[id] = true;
	return PLUGIN_CONTINUE;
}

public ls_on_revive(id, by)
{
	if (1 <= id <= 32)
		g_w3Down[id] = false;
}

public ls_on_bled_out(id)
{
	if (1 <= id <= 32)
		g_w3Down[id] = false;
}

stock w3_squad_set(id, sq)
{
	if (!(1 <= id <= 32))
		return;
	new old = g_w3Squad[id];
	g_w3Squad[id] = clamp(sq, 0, 32);
	if (sq > 0 && !g_w3Code[sq][0])
		w3_new_code(sq);
	if (old == g_w3Squad[id])
		return;
	if (sq > 0)
	{
		hint("role %d sq%d", id, sq);
		hint("friend @sq%d @sq%d 1", sq, sq);
	}
	else
		hint("role %d -", id);
	new ret;
	ExecuteForward(g_w3fwSquad, ret, id, g_w3Squad[id]);
}

/* a 4-letter party code for a squad (friends type say /squad CODE) */
stock w3_new_code(sq)
{
	static const L[] = "ABCDEFGHJKLMNPQRSTUVWXYZ";
	for (new tries = 0; tries < 20; tries++)
	{
		for (new i = 0; i < 4; i++)
			g_w3Code[sq][i] = L[random(sizeof L - 1)];
		g_w3Code[sq][4] = EOS;
		new bool:dup = false;
		for (new k = 1; k <= 32; k++)
			if (k != sq && equal(g_w3Code[k], g_w3Code[sq]))
				dup = true;
		if (!dup)
			return;
	}
}

stock w3_squad_count(sq, bool:alive)
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (g_w3Squad[p] == sq && is_user_connected(p) && (!alive || is_user_alive(p)))
			n++;
	return n;
}

stock w3_free_squad()
{
	for (new sq = 1; sq <= 32; sq++)
		if (!w3_squad_count(sq, false))
		{
			g_w3Code[sq][0] = EOS;
			return sq;
		}
	return 0;
}

/* how many sides still have someone alive: squads, and players in none; last = one of them */
stock w3_sides_alive(&last)
{
	new seen[66], n = 0;
	last = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		new key = g_w3SquadMax > 1 && g_w3Squad[p] ? min(g_w3Squad[p], 32) : 33 + p;
		if (key < sizeof seen && !seen[key])
		{
			seen[key] = 1;
			n++;
			last = p;
		}
	}
	return n;
}

/* is this a private (LAN) address; the network part to compare (a.b.c) */
stock bool:w3_lan_prefix(const ip[], out[], len)
{
	new bool:lan = equal(ip, "10.", 3) || equal(ip, "192.168.", 8) || equal(ip, "127.", 4);
	if (equal(ip, "172.", 4))
	{
		new b = str_to_num(ip[4]);
		lan = b >= 16 && b <= 31;
	}
	copy(out, len, ip);
	new dots = 0;
	for (new i = 0; out[i]; i++)
		if (out[i] == '.' && ++dots == 3)
		{
			out[i] = EOS;
			break;
		}
	return lan;
}

/* players on the server who aren't in a squad yet, people first */
stock w3_playing(p)
{
	return is_user_connected(p) && (get_user_team(p) == 1 || get_user_team(p) == 2);
}

/* fills the squads: squads people made stay; people from one address (or one LAN) together; people
   short of a squad join one that's short; then bots fill every squad up to the size */
stock w3_squad_fill()
{
	new size = max(g_w3SquadMax, 1);
	if (size <= 1)
	{
		for (new p = 1; p <= MaxClients; p++)
			w3_squad_set(p, w3_playing(p) ? p : 0);
		return MaxClients;
	}
	/* (people who left the game don't count) */
	for (new p = 1; p <= MaxClients; p++)
		if (!w3_playing(p))
			g_w3Squad[p] = 0;
	/* same address or LAN, not in a squad yet: grouped */
	new ip[33][24], pre[33][24], bool:lan[33];
	for (new p = 1; p <= MaxClients; p++)
		if (w3_playing(p) && !is_user_bot(p))
		{
			get_user_ip(p, ip[p], charsmax(ip[]), 1);
			lan[p] = w3_lan_prefix(ip[p], pre[p], charsmax(pre[]));
		}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!w3_playing(p) || is_user_bot(p) || g_w3Squad[p])
			continue;
		for (new q = 1; q <= MaxClients; q++)
		{
			if (q == p || !w3_playing(q) || is_user_bot(q) || !g_w3Squad[q] || w3_squad_count(g_w3Squad[q], false) >= size)
				continue;
			if (equal(ip[p], ip[q]) || (lan[p] && lan[q] && equal(pre[p], pre[q])))
			{
				w3_squad_set(p, g_w3Squad[q]);
				break;
			}
		}
		if (!g_w3Squad[p])
		{
			/* another person from there, not in a squad either: a new squad for both */
			for (new q = p + 1; q <= MaxClients; q++)
				if (w3_playing(q) && !is_user_bot(q) && !g_w3Squad[q] && (equal(ip[p], ip[q]) || (lan[p] && lan[q] && equal(pre[p], pre[q]))))
				{
					new sq = w3_free_squad();
					w3_squad_set(p, sq);
					w3_squad_set(q, sq);
					break;
				}
		}
	}
	/* people alone: into a squad with people that's short, else their own */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!w3_playing(p) || is_user_bot(p) || g_w3Squad[p])
			continue;
		new pick = 0;
		for (new sq = 1; sq <= 32 && !pick; sq++)
			if (w3_squad_count(sq, false) && w3_squad_count(sq, false) < size)
				pick = sq;
		w3_squad_set(p, pick ? pick : w3_free_squad());
	}
	/* bots: fill short squads (people's first), then squads of their own */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!w3_playing(p) || !is_user_bot(p) || g_w3Squad[p])
			continue;
		new pick = 0;
		for (new sq = 1; sq <= 32 && !pick; sq++)
			if (w3_squad_count(sq, false) && w3_squad_count(sq, false) < size)
				pick = sq;
		w3_squad_set(p, pick ? pick : w3_free_squad());
	}
	/* the scoreboard: odd squads one side, even the other */
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_w3Squad[p])
			continue;
		n = max(n, g_w3Squad[p]);
		if (get_pcvar_num(g_w3cSquadTeams))
			cs_set_user_team(p, (g_w3Squad[p] % 2) ? CS_TEAM_T : CS_TEAM_CT);
	}
	for (new sq = 1; sq <= n; sq++)
		if (w3_squad_count(sq, false))
			hint("friend @sq%d @sq%d 1", sq, sq);
	return n;
}

/* a squad short of players with someone in it still alive (a late joiner can fill it); 0 none */
stock w3_squad_short()
{
	if (g_w3SquadMax <= 1)
		return 0;
	for (new sq = 1; sq <= 32; sq++)
	{
		new n = w3_squad_count(sq, false);
		if (n && n < g_w3SquadMax && w3_squad_count(sq, true))
			return sq;
	}
	return 0;
}

stock w3_lobby_start(Float:secs)
{
	if (secs <= 0.0)
	{
		w3_lobby_end();
		return;
	}
	g_w3Lobby = true;
	g_w3LobbyEnd = get_gametime() + secs;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && g_w3SquadMax > 1)
			client_print_color(p, print_team_default, "^4[Squads]^1 Squads of %d: say ^3/squad^1 to make one, invite friends or join with a code. The rest fill up when the game starts.", g_w3SquadMax);
}

stock w3_lobby_end()
{
	g_w3Lobby = false;
	new n = w3_squad_fill();
	new ret;
	ExecuteForward(g_w3fwLobby, ret, n);
}

stock w3_squad_tick()
{
	new Float:now = get_gametime();
	if (g_w3Lobby)
	{
		new left = floatround(g_w3LobbyEnd - now, floatround_ceil);
		if (left <= 0)
			w3_lobby_end();
		else
			for (new p = 1; p <= MaxClients; p++)
			{
				if (!is_user_connected(p) || is_user_bot(p))
					continue;
				new msg[256], n, name[24];
				n = formatex(msg, charsmax(msg), "SQUAD LOBBY   the game starts in %d s^n", left);
				if (g_w3SquadMax <= 1)
					n += formatex(msg[n], charsmax(msg) - n, "Solo: everyone for themselves");
				else if (!g_w3Squad[p])
					n += formatex(msg[n], charsmax(msg) - n, "say /squad to team up (squads of %d); or you'll be put in one", g_w3SquadMax);
				else
				{
					n += formatex(msg[n], charsmax(msg) - n, "Your squad (code %s):", g_w3Code[g_w3Squad[p]]);
					for (new q = 1; q <= MaxClients; q++)
						if (g_w3Squad[q] == g_w3Squad[p] && is_user_connected(q))
						{
							get_user_name(q, name, charsmax(name));
							n += formatex(msg[n], charsmax(msg) - n, " %s", name);
						}
				}
				set_hudmessage(120, 220, 255, -1.0, 0.02, 0, 0.0, 1.1, 0.0, 0.1, -1);
				ShowSyncHudMsg(p, g_w3SyncTop, "%s", msg);
			}
	}
	/* squad mates: a marker over their heads and a dot on the radar, for each person in a squad */
	if (g_w3SquadMax <= 1 || !get_pcvar_num(g_w3cSquadMarks))
		return;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || !g_w3Squad[p])
			continue;
		for (new q = 1; q <= MaxClients; q++)
		{
			if (q == p || g_w3Squad[q] != g_w3Squad[p] || !is_user_alive(q))
				continue;
			message_begin(MSG_ONE_UNRELIABLE, SVC_TEMPENTITY, _, p);
			write_byte(TE_PLAYERATTACHMENT);
			write_byte(q);
			write_coord(45);
			write_short(g_w3SprMark);
			write_short(11);
			message_end();
			if (g_msgRadar && is_user_alive(p))
			{
				new Float:o[3];
				pev(q, pev_origin, o);
				message_begin(MSG_ONE_UNRELIABLE, g_msgRadar, _, p);
				write_byte(q);
				write_coord(floatround(o[0]));
				write_coord(floatround(o[1]));
				write_coord(floatround(o[2]));
				message_end();
			}
		}
	}
}

/* say /squad, say /squad CODE */
public cmd_w3_say(id)
{
	new a[64];
	read_args(a, charsmax(a));
	remove_quotes(a);
	trim(a);
	if (!equali(a, "/squad", 6) || (a[6] && a[6] != ' '))
		return PLUGIN_CONTINUE;
	new code[16];
	copy(code, charsmax(code), a[6]);
	trim(code);
	if (code[0])
		w3_join_code(id, code);
	else
		w3_squad_menu(id);
	return PLUGIN_HANDLED;
}

public cmd_w3_squad(id)
{
	new code[16];
	read_argv(1, code, charsmax(code));
	if (code[0])
		w3_join_code(id, code);
	else
		w3_squad_menu(id);
	return PLUGIN_HANDLED;
}

stock bool:w3_can_change(id)
{
	if (g_w3SquadMax <= 1)
	{
		client_print(id, print_chat, "[Squads] This game is solo.");
		return false;
	}
	if (!g_w3Lobby && is_user_alive(id) && g_w3Squad[id])
	{
		client_print(id, print_chat, "[Squads] Squads are set until the next game's lobby.");
		return false;
	}
	return true;
}

stock w3_join_code(id, const code[])
{
	if (!w3_can_change(id))
		return;
	for (new sq = 1; sq <= 32; sq++)
	{
		if (!g_w3Code[sq][0] || !equali(g_w3Code[sq], code) || !w3_squad_count(sq, false))
			continue;
		if (w3_squad_count(sq, false) >= g_w3SquadMax)
		{
			client_print(id, print_chat, "[Squads] That squad is full.");
			return;
		}
		w3_squad_set(id, sq);
		client_print(id, print_chat, "[Squads] You joined squad %s.", g_w3Code[sq]);
		return;
	}
	client_print(id, print_chat, "[Squads] No squad has the code %s.", code);
}

stock w3_squad_menu(id)
{
	if (!w3_can_change(id))
		return;
	new title[96];
	if (g_w3Squad[id])
		formatex(title, charsmax(title), "Squad (code %s: friends say /squad %s)", g_w3Code[g_w3Squad[id]], g_w3Code[g_w3Squad[id]]);
	else
		formatex(title, charsmax(title), "Squads of %d", g_w3SquadMax);
	new m = menu_create(title, "menu_w3_squad");
	menu_additem(m, g_w3Squad[id] ? "Start a new squad" : "Start a squad", "new");
	menu_additem(m, "Invite someone", "inv");
	if (g_w3Squad[id])
		menu_additem(m, "Leave my squad", "leave");
	menu_display(id, m);
}

public menu_w3_squad(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[8], acc, cb;
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb);
	menu_destroy(menu);
	if (equal(info, "new"))
	{
		w3_squad_set(id, w3_free_squad());
		client_print(id, print_chat, "[Squads] Your squad's code is %s: friends say /squad %s to join.", g_w3Code[g_w3Squad[id]], g_w3Code[g_w3Squad[id]]);
	}
	else if (equal(info, "leave"))
		w3_squad_set(id, 0);
	else if (equal(info, "inv"))
		w3_invite_menu(id);
	return PLUGIN_HANDLED;
}

stock w3_invite_menu(id)
{
	if (!g_w3Squad[id])
		w3_squad_set(id, w3_free_squad());
	new m = menu_create("Invite to your squad", "menu_w3_invite"), name[40], info[4];
	/* people first, then bots */
	for (new pass = 0; pass < 2; pass++)
		for (new q = 1; q <= MaxClients; q++)
		{
			if (q == id || !w3_playing(q) || g_w3Squad[q] == g_w3Squad[id] || (is_user_bot(q) ? 1 : 0) != pass)
				continue;
			get_user_name(q, name, charsmax(name));
			if (pass == 1)
				add(name, charsmax(name), " (bot)");
			num_to_str(q, info, charsmax(info));
			menu_additem(m, name, info);
		}
	menu_display(id, m);
}

public menu_w3_invite(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[4], acc, cb;
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb);
	menu_destroy(menu);
	new q = str_to_num(info), sq = g_w3Squad[id];
	if (!is_user_connected(q) || !sq)
		return PLUGIN_HANDLED;
	if (w3_squad_count(sq, false) >= g_w3SquadMax)
	{
		client_print(id, print_chat, "[Squads] Your squad is full.");
		return PLUGIN_HANDLED;
	}
	/* a bot says yes; a person is asked */
	if (is_user_bot(q))
	{
		w3_squad_set(q, sq);
		return PLUGIN_HANDLED;
	}
	g_w3Invite[q] = id;
	new title[80], name[32];
	get_user_name(id, name, charsmax(name));
	formatex(title, charsmax(title), "%s asks you into their squad", name);
	new m = menu_create(title, "menu_w3_accept");
	menu_additem(m, "Join", "yes");
	menu_additem(m, "No thanks", "no");
	menu_display(q, m);
	return PLUGIN_HANDLED;
}

public menu_w3_accept(id, menu, item)
{
	new info[4], acc, cb;
	if (item >= 0)
		menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb);
	menu_destroy(menu);
	new from = g_w3Invite[id];
	g_w3Invite[id] = 0;
	if (!equal(info, "yes") || !from || !is_user_connected(from) || !g_w3Squad[from] || !w3_can_change(id))
		return PLUGIN_HANDLED;
	if (w3_squad_count(g_w3Squad[from], false) >= g_w3SquadMax)
	{
		client_print(id, print_chat, "[Squads] That squad is full now.");
		return PLUGIN_HANDLED;
	}
	w3_squad_set(id, g_w3Squad[from]);
	return PLUGIN_HANDLED;
}

/* native ls_squad_set(id, squad) */
public n_w3_squad_set(plugin, params)
{
	w3_squad_set(get_param(1), get_param(2));
	return 1;
}

/* native ls_squad_get(id) */
public n_w3_squad_get(plugin, params)
{
	new id = get_param(1);
	return 1 <= id <= 32 ? g_w3Squad[id] : 0;
}

/* native ls_squad_max(size = -1): sets the squad size (1 = solo), returns the size before */
public n_w3_squad_max(plugin, params)
{
	new old = g_w3SquadMax, n = get_param(1);
	if (n >= 1)
		g_w3SquadMax = min(n, 16);
	return old;
}

/* native ls_squad_count(squad, bool:alive = false) */
public n_w3_squad_count(plugin, params)
{
	return w3_squad_count(get_param(1), get_param(2) != 0);
}

/* native ls_squads_alive(&last = 0) */
public n_w3_squads_alive(plugin, params)
{
	new last;
	new n = w3_sides_alive(last);
	set_param_byref(1, last);
	return n;
}

/* native ls_squad_fill() */
public n_w3_squad_fill(plugin, params)
{
	return w3_squad_fill();
}

/* native ls_squad_lobby(Float:secs) */
public n_w3_squad_lobby(plugin, params)
{
	w3_lobby_start(get_param_f(1));
	return 1;
}

/* native ls_squad_menu(id) */
public n_w3_squad_menu(plugin, params)
{
	new id = get_param(1);
	if (1 <= id <= MaxClients && is_user_connected(id))
		w3_squad_menu(id);
	return 1;
}

/* native ls_squad_short() */
public n_w3_squad_short(plugin, params)
{
	return w3_squad_short();
}

/* native ls_squad_code(squad, out[], len) */
public n_w3_squad_code(plugin, params)
{
	new sq = get_param(1);
	if (!(1 <= sq <= 32))
		return 0;
	set_string(2, g_w3Code[sq], get_param(3));
	return g_w3Code[sq][0] != EOS;
}

/* ==== end W3 ==== */

/* =========================================================================
 * Areas, damage, targets, forwards
 * ========================================================================= */

stock bool:inside(s, const Float:p[3])
{
	if (g_box[s])
	{
		return p[0] >= g_mins[s][0] - 16.0 && p[0] <= g_maxs[s][0] + 16.0
			&& p[1] >= g_mins[s][1] - 16.0 && p[1] <= g_maxs[s][1] + 16.0
			&& p[2] >= g_mins[s][2] - 36.0 && p[2] <= g_maxs[s][2] + 36.0;
	}
	new Float:dx = p[0] - g_org[s][0], Float:dy = p[1] - g_org[s][1];
	if (dx * dx + dy * dy > g_r[s] * g_r[s])
		return false;
	return p[2] >= g_floor[s] - 32.0 && p[2] <= g_floor[s] + g_h[s];
}

stock Float:floor_z(const Float:p[3], ignore)
{
	new Float:a[3], Float:b[3], Float:hit[3];
	vcopy(p, a);
	vcopy(p, b);
	a[2] += 8.0;
	b[2] -= 4096.0;
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, ignore, 0);
	new Float:frac;
	get_tr2(0, TR_flFraction, frac);
	if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid) || frac >= 1.0)
	{
		/* inside a wall or over the void: keep the height it was given */
		if (ignore)
			log_amx("A component at %.0f %.0f %.0f is inside a wall or has no floor under it.", p[0], p[1], p[2]);
		return p[2] - 36.0;
	}
	get_tr2(0, TR_vecEndPos, hit);
	return hit[2];
}

stock hurt(id, attacker, Float:dmg, bits)
{
	if (!is_user_alive(id) || dmg <= 0.0)
		return;
	ExecuteHamB(Ham_TakeDamage, id, attacker, attacker, dmg, bits);
}

/* sets off the entities named in the component's target key */
stock fire_targets(s, activator)
{
	new t[32];
	if (!ks(s, "target", t, charsmax(t)))
		return;
	new e = 0;
	while ((e = engfunc(EngFunc_FindEntityByString, e, "targetname", t)) > 0)
	{
		if (e == g_ent[s])
			continue;
		ExecuteHamB(Ham_Use, e, activator > 0 ? activator : g_ent[s], g_ent[s], 3, 0.0);
	}
}

stock fwd1(fw, a)
{
	new ret;
	ExecuteForward(fw, ret, a);
}

stock fwd2(fw, a, b)
{
	new ret;
	ExecuteForward(fw, ret, a, b);
}

stock fwd3(fw, a, b, c)
{
	new ret;
	ExecuteForward(fw, ret, a, b, c);
}

stock fwd3f(fw, a, b, Float:c)
{
	new ret;
	ExecuteForward(fw, ret, a, b, c);
}

stock fwd_drop(ent, stage, const Float:o[3])
{
	/* logged, for tests: called / landed / opened */
	static const STAGE[][] = { "called", "landed", "opened" };
	log_amx("Supply drop %d %s at %.0f %.0f %.0f", ent, STAGE[clamp(stage, 0, 2)], o[0], o[1], o[2]);
	new ret, Float:v[3];
	vcopy(o, v);
	ExecuteForward(g_fwDrop, ret, ent, stage, PrepareArray(_:v, 3));
}

stock comp_debug(const fmt[], any:...)
{
	if (!get_pcvar_num(g_cDebug))
		return;
	new msg[192];
	vformat(msg, charsmax(msg), fmt, 2);
	log_amx("[components] %s", msg);
}

/* =========================================================================
 * Items and loot tables
 * ========================================================================= */

stock const PRIMARY_IDS = (1 << CSW_SCOUT) | (1 << CSW_XM1014) | (1 << CSW_MAC10) | (1 << CSW_AUG) | (1 << CSW_UMP45)
	| (1 << CSW_SG550) | (1 << CSW_GALIL) | (1 << CSW_FAMAS) | (1 << CSW_AWP) | (1 << CSW_MP5NAVY) | (1 << CSW_M249)
	| (1 << CSW_M3) | (1 << CSW_M4A1) | (1 << CSW_TMP) | (1 << CSW_G3SG1) | (1 << CSW_SG552) | (1 << CSW_AK47) | (1 << CSW_P90);
stock const SECONDARY_IDS = (1 << CSW_P228) | (1 << CSW_ELITE) | (1 << CSW_FIVESEVEN) | (1 << CSW_USP) | (1 << CSW_GLOCK18) | (1 << CSW_DEAGLE);

stock weapon_name(const item[], out[], len)
{
	if (equal(item, "weapon_", 7))
		copy(out, len, item);
	else if (equali(item, "he") || equali(item, "hegren"))
		copy(out, len, "weapon_hegrenade");
	else if (equali(item, "flash"))
		copy(out, len, "weapon_flashbang");
	else if (equali(item, "smoke") || equali(item, "sgren"))
		copy(out, len, "weapon_smokegrenade");
	else if (equali(item, "mp5"))
		copy(out, len, "weapon_mp5navy");
	else
		formatex(out, len, "weapon_%s", item);
}

stock item_model(const item[], out[], len)
{
	if (w3_item_model(item, out, len)) /* W3 */
		return; /* W3 */
	if (equali(item, "vest") || equali(item, "kevlar") || equali(item, "armor", 5) || equali(item, "nvg"))
		copy(out, len, "models/w_kevlar.mdl");
	else if (equali(item, "vesthelm") || equali(item, "assault"))
		copy(out, len, "models/w_assault.mdl");
	else if (equali(item, "defuse") || equali(item, "defuser"))
		copy(out, len, "models/w_thighpack.mdl");
	else if (equali(item, "hp", 2) || equali(item, "medkit"))
		copy(out, len, "models/w_medkit.mdl");
	else if (equali(item, "ammo"))
		copy(out, len, "models/w_9mmarclip.mdl");
	else if (equali(item, "money", 5))
		copy(out, len, "models/w_backpack.mdl");
	else
	{
		new w[32];
		weapon_name(item, w, charsmax(w));
		if (equal(w, "weapon_mp5navy"))
			copy(out, len, "models/w_mp5.mdl");
		else
			formatex(out, len, "models/w_%s.mdl", w[7]);
		if (!get_weaponid(w))
			copy(out, len, DEFAULT_BOX);
	}
}

/* would giving this do anything (no second primary, no full grenade slot) */
stock bool:can_take(id, const item[])
{
	new w3 = w3_can_take(id, item); /* W3 */
	if (w3 >= 0) /* W3 */
		return w3 != 0; /* W3 */
	new w[32];
	weapon_name(item, w, charsmax(w));
	new csw = get_weaponid(w);
	if (!csw)
		return true;
	new weapons[32], n, have = 0;
	get_user_weapons(id, weapons, n);
	for (new i = 0; i < n; i++)
		have |= (1 << weapons[i]);
	if ((1 << csw) & PRIMARY_IDS)
		return !(have & PRIMARY_IDS);
	if ((1 << csw) & SECONDARY_IDS)
		return !(have & SECONDARY_IDS);
	return !(have & (1 << csw));
}

stock give_thing(id, const item[])
{
	if (!is_user_alive(id) || !item[0])
		return 0;
	new num = 0;
	new colon = contain(item, ":");
	if (colon > 0)
		num = str_to_num(item[colon + 1]);
	new w3 = w3_give(id, item); /* W3 */
	if (w3 >= 0) /* W3 */
		return w3; /* W3 */
	if (equali(item, "hp", 2) || equali(item, "medkit"))
	{
		new mx = floatround(entity_max_health(id));
		set_user_health(id, min(get_user_health(id) + (num > 0 ? num : 50), max(mx, 100)));
		emit_sound(id, CHAN_ITEM, "items/smallmedkit1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		return 1;
	}
	if (equali(item, "armor", 5) || equali(item, "vest") || equali(item, "kevlar"))
	{
		new CsArmorType:t;
		new cur = cs_get_user_armor(id, t);
		cs_set_user_armor(id, max(cur, num > 0 ? num : 100), t == CS_ARMOR_VESTHELM ? CS_ARMOR_VESTHELM : CS_ARMOR_KEVLAR);
		return 1;
	}
	if (equali(item, "vesthelm") || equali(item, "assault"))
	{
		cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM);
		return 1;
	}
	if (equali(item, "money", 5))
	{
		cs_set_user_money(id, min(cs_get_user_money(id) + (num > 0 ? num : 500), 16000));
		return 1;
	}
	if (equali(item, "nvg"))
	{
		cs_set_user_nvg(id, 1);
		return 1;
	}
	if (equali(item, "defuse") || equali(item, "defuser"))
	{
		if (get_user_team(id) != 2)
			return 0;
		cs_set_user_defuse(id, 1);
		return 1;
	}
	if (equali(item, "ammo"))
	{
		new weapons[32], n;
		get_user_weapons(id, weapons, n);
		for (new i = 0; i < n; i++)
		{
			new csw = weapons[i];
			if ((1 << csw) & (PRIMARY_IDS | SECONDARY_IDS))
				cs_set_user_bpammo(id, csw, max(cs_get_user_bpammo(id, csw), ammo_full(csw)));
		}
		emit_sound(id, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		return 1;
	}
	new w[32];
	weapon_name(item, w, charsmax(w));
	new csw = get_weaponid(w);
	if (!csw)
	{
		log_amx("Unknown item ^"%s^"", item);
		return 0;
	}
	if (!can_take(id, item) && !((1 << csw) & ((1 << CSW_HEGRENADE) | (1 << CSW_FLASHBANG) | (1 << CSW_SMOKEGRENADE))))
		return 0;
	if (!give_item(id, w))
		return 0;
	if ((1 << csw) & (PRIMARY_IDS | SECONDARY_IDS))
		cs_set_user_bpammo(id, csw, max(cs_get_user_bpammo(id, csw), w3_found_reserve(csw))); /* W3 */
	emit_sound(id, CHAN_ITEM, "items/gunpickup2.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	return 1;
}

stock Float:entity_max_health(id)
{
	new Float:mx;
	pev(id, pev_max_health, mx);
	return mx;
}

stock ammo_full(csw)
{
	switch (csw)
	{
		case CSW_AWP: return 30;
		case CSW_SCOUT, CSW_G3SG1, CSW_AK47, CSW_SG550, CSW_AUG, CSW_SG552, CSW_M4A1, CSW_FAMAS, CSW_GALIL: return 90;
		case CSW_M249: return 200;
		case CSW_M3, CSW_XM1014: return 32;
		case CSW_P90, CSW_FIVESEVEN: return 100;
		case CSW_MAC10, CSW_UMP45, CSW_USP: return 100;
		case CSW_MP5NAVY, CSW_TMP, CSW_GLOCK18, CSW_ELITE: return 120;
		case CSW_DEAGLE: return 35;
		case CSW_P228: return 52;
	}
	return 0;
}

/* loot.ini: [table] then "item weight [tier]" lines */
stock load_loot()
{
	new dir[160], path[224];
	get_configsdir(dir, charsmax(dir));
	formatex(path, charsmax(path), "%s/gamemodes/loot.ini", dir);
	new bool:any = read_loot(path);
	if (g_modeId[0])
	{
		formatex(path, charsmax(path), "%s/gamemodes/%s.loot.ini", dir, g_modeId);
		any = read_loot(path) || any;
	}
	if (!any)
		builtin_loot();
}

stock table_index(const name[], bool:create, bool:clear)
{
	new idx;
	if (TrieGetCell(g_ltIndex, name, idx))
	{
		if (clear)
			ArrayClear(g_ltItems[idx]);
		return idx;
	}
	if (!create || g_ntables >= MAX_TABLES)
		return -1;
	idx = g_ntables++;
	g_ltItems[idx] = ArrayCreate(26);
	TrieSetCell(g_ltIndex, name, idx);
	return idx;
}

stock loot_add(t, const item[], weight, tier)
{
	new row[26];
	copy(row, 23, item);
	row[24] = weight;
	row[25] = tier;
	ArrayPushArray(g_ltItems[t], row);
}

stock bool:read_loot(const path[])
{
	new f = fopen(path, "rt");
	if (!f)
		return false;
	new line[128], t = -1;
	while (fgets(f, line, charsmax(line)))
	{
		trim(line);
		if (!line[0] || line[0] == ';' || line[0] == '#' || (line[0] == '/' && line[1] == '/'))
			continue;
		if (line[0] == '[')
		{
			new name[32];
			copyc(name, charsmax(name), line[1], ']');
			trim(name);
			/* a mode's file replaces a table of the same name */
			t = table_index(name, true, true);
			continue;
		}
		if (t < 0)
			continue;
		new item[32], w[12], tier[12];
		parse(line, item, charsmax(item), w, charsmax(w), tier, charsmax(tier));
		loot_add(t, item, w[0] ? max(str_to_num(w), 1) : 1, tier[0] ? str_to_num(tier) : 1);
	}
	fclose(f);
	return true;
}

stock builtin_loot()
{
	new t = table_index("default", true, true);
	loot_add(t, "glock18", 20, 1); loot_add(t, "usp", 20, 1); loot_add(t, "p228", 12, 1);
	loot_add(t, "deagle", 8, 2); loot_add(t, "mp5navy", 12, 2); loot_add(t, "ump45", 10, 2);
	loot_add(t, "m3", 8, 2); loot_add(t, "famas", 6, 3); loot_add(t, "galil", 6, 3);
	loot_add(t, "ak47", 4, 4); loot_add(t, "m4a1", 4, 4); loot_add(t, "awp", 1, 5);
	loot_add(t, "vest", 14, 1); loot_add(t, "vesthelm", 6, 3); loot_add(t, "he", 10, 1);
	loot_add(t, "flash", 10, 1); loot_add(t, "smoke", 6, 1); loot_add(t, "hp:50", 14, 1);
	loot_add(t, "ammo", 16, 1);
	t = table_index("drop", true, true);
	loot_add(t, "ak47", 10, 4); loot_add(t, "m4a1", 10, 4); loot_add(t, "awp", 5, 5);
	loot_add(t, "m249", 3, 5); loot_add(t, "vesthelm", 12, 3); loot_add(t, "hp:100", 10, 3);
	loot_add(t, "he", 8, 2); loot_add(t, "ammo", 10, 1);
}

stock loot_roll(const table[], out[], len, tier)
{
	out[0] = EOS;
	new t = table_index(table, false, false);
	if (t < 0)
		t = table_index("default", false, false);
	if (t < 0)
		return 0;
	new n = ArraySize(g_ltItems[t]), total = 0, row[26];
	for (new i = 0; i < n; i++)
	{
		ArrayGetArray(g_ltItems[t], i, row);
		if (!tier || row[25] <= tier)
			total += row[24];
	}
	if (total <= 0)
		return 0;
	new pick = random(total);
	for (new i = 0; i < n; i++)
	{
		ArrayGetArray(g_ltItems[t], i, row);
		if (tier && row[25] > tier)
			continue;
		pick -= row[24];
		if (pick < 0)
		{
			copy(out, len, row);
			return 1;
		}
	}
	return 0;
}

/* =========================================================================
 * Admin console
 * ========================================================================= */

public cmd_list(id, level, cid)
{
	if (!cmd_access(id, level, cid, 1))
		return PLUGIN_HANDLED;
	console_print(id, "%d components on %s (bots: %s)", count_comps(K_NONE), g_mapname, g_brain ? "ls_mode" : (g_lsBot ? "ls_bot fallback" : "no hints"));
	new src[3][] = { "map", "pack", "run" };
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s])
			continue;
		console_print(id, "%4d %-20s %-4s %s %6.0f %6.0f %6.0f state %d side %d %s", g_ent[s], KIND_NAME[g_kind[s]], src[g_src[s]],
			g_on[s] ? "on " : "off", g_org[s][0], g_org[s][1], g_org[s][2], g_state[s], g_side[s], g_label[s]);
	}
	return PLUGIN_HANDLED;
}

public cmd_spawn(id, level, cid)
{
	if (!cmd_access(id, level, cid, 2))
		return PLUGIN_HANDLED;
	new cls[32], args[256];
	read_argv(1, cls, charsmax(cls));
	read_args(args, charsmax(args));
	new pos = contain(args, cls);
	new keys[256];
	if (pos >= 0)
		copy(keys, charsmax(keys), args[pos + strlen(cls)]);
	new kind = kind_of(cls);
	if (!kind || kind == K_CRATE)
	{
		console_print(id, "Not a component: %s", cls);
		return PLUGIN_HANDLED;
	}
	new Float:o[3];
	if (id)
	{
		new io[3];
		get_user_origin(id, io, 3);
		o[0] = float(io[0]);
		o[1] = float(io[1]);
		o[2] = float(io[2]) + 1.0;
	}
	else
	{
		new a[16], b[16], c[16];
		read_argv(2, a, charsmax(a));
		read_argv(3, b, charsmax(b));
		read_argv(4, c, charsmax(c));
		o[0] = str_to_float(a);
		o[1] = str_to_float(b);
		o[2] = str_to_float(c);
	}
	g_now = get_gametime();
	new ent = comp_make(kind, o, keys, SRC_RUNTIME);
	console_print(id, ent ? "Made %s (entity %d) at %.0f %.0f %.0f" : "Couldn't make %s", cls, ent, o[0], o[1], o[2]);
	return PLUGIN_HANDLED;
}

public cmd_del(id, level, cid)
{
	if (!cmd_access(id, level, cid, 2))
		return PLUGIN_HANDLED;
	new a[8];
	read_argv(1, a, charsmax(a));
	new s = slot_of(str_to_num(a));
	if (s < 0)
		console_print(id, "Not a component.");
	else
		comp_remove(s);
	return PLUGIN_HANDLED;
}

public cmd_save(id, level, cid)
{
	if (!cmd_access(id, level, cid, 1))
		return PLUGIN_HANDLED;
	new dir[160], path[224];
	get_configsdir(dir, charsmax(dir));
	formatex(path, charsmax(path), "%s/components", dir);
	if (!dir_exists(path))
		mkdir(path);
	formatex(path, charsmax(path), "%s/components/%s.ini", dir, g_mapname);
	new f = fopen(path, "wt");
	if (!f)
	{
		console_print(id, "Couldn't write %s", path);
		return PLUGIN_HANDLED;
	}
	fprintf(f, "; Game mode components for %s (written by ls_comp_save)^n", g_mapname);
	fprintf(f, "; classname x y z key=value ...   (docs/COMPONENTS.md)^n");
	new n = 0, key[32], val[128], line[512];
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_src[s] == SRC_MAP || g_temp[s])
			continue;
		new Float:o[3];
		pev(g_ent[s], pev_origin, o);
		if (g_kind[s] == K_BARRICADE && g_box[s])
			vcopy(g_org[s], o);
		formatex(line, charsmax(line), "%s %.0f %.0f %.0f", KIND_NAME[g_kind[s]], o[0], o[1], o[2]);
		new TrieIter:it = TrieIterCreate(g_keys[s]);
		while (!TrieIterEnded(it))
		{
			TrieIterGetKey(it, key, charsmax(key));
			TrieIterGetString(it, val, charsmax(val));
			if (contain(val, " ") >= 0 || !val[0])
				format(line, charsmax(line), "%s %s=^"%s^"", line, key, val);
			else
				format(line, charsmax(line), "%s %s=%s", line, key, val);
			TrieIterNext(it);
		}
		TrieIterDestroy(it);
		fprintf(f, "%s^n", line);
		n++;
	}
	fclose(f);
	console_print(id, "Saved %d components to %s", n, path);
	return PLUGIN_HANDLED;
}

public cmd_start(id, level, cid)
{
	if (!cmd_access(id, level, cid, 2))
		return PLUGIN_HANDLED;
	new a[8];
	read_argv(1, a, charsmax(a));
	g_now = get_gametime();
	console_print(id, "Started %d", start_or_stop(str_to_num(a), true, id));
	return PLUGIN_HANDLED;
}

public cmd_stop(id, level, cid)
{
	if (!cmd_access(id, level, cid, 2))
		return PLUGIN_HANDLED;
	new a[8];
	read_argv(1, a, charsmax(a));
	g_now = get_gametime();
	console_print(id, "Stopped %d", start_or_stop(str_to_num(a), false, id));
	return PLUGIN_HANDLED;
}

public cmd_reset(id, level, cid)
{
	if (!cmd_access(id, level, cid, 1))
		return PLUGIN_HANDLED;
	g_now = get_gametime();
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s])
			comp_reset(s);
	return PLUGIN_HANDLED;
}

/* ent 0: every component waiting for a start (start=manual / no timer) */
stock start_or_stop(ent, bool:start, activator)
{
	new n = 0;
	if (ent)
	{
		new s = slot_of(ent);
		if (s >= 0)
			n = start ? comp_start(s, activator) : comp_stop(s);
		return n;
	}
	new bool:waves = false;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s])
			continue;
		new k = g_kind[s];
		if (k != K_SHRINKER && k != K_WAVE && k != K_DROP && k != K_PLATFORM)
			continue;
		if (k == K_WAVE)
		{
			if (waves)
				continue;
			waves = true;
		}
		n += start ? comp_start(s, activator) : comp_stop(s);
	}
	return n;
}

/* =========================================================================
 * Natives
 * ========================================================================= */

/* a zone native's component: a safe zone, or a shrinker (its zone; Studio wires either) */
stock zone_arg(n)
{
	new s = arg_slot(n);
	if (s < 0)
		return -1;
	if (g_kind[s] == K_SHRINKER && g_aux[s] >= 0 && g_aux[s] < MAX_COMP && g_ent[g_aux[s]] && g_kind[g_aux[s]] == K_ZONE)
		return g_aux[s];
	if (g_kind[s] != K_ZONE)
	{
		log_error(AMX_ERR_NATIVE, "Entity %d is %s, not a safe zone or its shrinker", get_param(n), KIND_NAME[g_kind[s]]);
		return -1;
	}
	return s;
}

stock arg_slot(n, kind = K_NONE)
{
	new ent = get_param(n);
	new s = slot_of(ent);
	if (s < 0 || !g_ent[s])
	{
		if (ent)
			log_error(AMX_ERR_NATIVE, "Entity %d isn't a game mode component", ent);
		return -1;
	}
	if (kind != K_NONE && g_kind[s] != kind)
	{
		log_error(AMX_ERR_NATIVE, "Entity %d is %s, not %s", ent, KIND_NAME[g_kind[s]], KIND_NAME[kind]);
		return -1;
	}
	return s;
}

public n_create(plugin, params)
{
	new cls[32], keys[512], Float:o[3];
	get_string(1, cls, charsmax(cls));
	get_array_f(2, o, 3);
	get_string(3, keys, charsmax(keys));
	new kind = kind_of(cls);
	if (!kind || kind == K_CRATE)
		return 0;
	g_now = get_gametime();
	return comp_make(kind, o, keys, SRC_RUNTIME);
}

public n_remove(plugin, params)
{
	new s = arg_slot(1);
	if (s >= 0)
		comp_remove(s);
	return 0;
}

public n_is(plugin, params)
{
	new s = slot_of(get_param(1));
	return s >= 0 && g_ent[s];
}

public n_kind(plugin, params)
{
	new s = slot_of(get_param(1));
	if (s < 0 || !g_ent[s])
	{
		set_string(2, "", get_param(3));
		return 0;
	}
	set_string(2, KIND_NAME[g_kind[s]], get_param(3));
	return 1;
}

public n_find(plugin, params)
{
	new cls[32];
	get_string(1, cls, charsmax(cls));
	new kind = cls[0] ? kind_of(cls) : K_NONE;
	if (cls[0] && !kind)
		return 0;
	new start = get_param(2);
	/* in entity order, so a loop visits each once */
	new best = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		new e = g_ent[s];
		if (!e || e <= start || (kind && g_kind[s] != kind))
			continue;
		if (!best || e < best)
			best = e;
	}
	return best;
}

public n_by_name(plugin, params)
{
	new name[32];
	get_string(1, name, charsmax(name));
	new start = get_param(2), best = 0;
	for (new s = 0; s < g_nslots; s++)
	{
		new e = g_ent[s];
		if (!e || e <= start || !equal(g_tname[s], name))
			continue;
		if (!best || e < best)
			best = e;
	}
	return best;
}

public n_count(plugin, params)
{
	new cls[32];
	get_string(1, cls, charsmax(cls));
	if (!cls[0])
		return count_comps(K_NONE);
	new kind = kind_of(cls);
	return kind ? count_comps(kind) : 0;
}

public n_get(plugin, params)
{
	new s = arg_slot(1);
	new key[32], out[256];
	get_string(2, key, charsmax(key));
	new bool:ok = s >= 0 && ks(s, key, out, charsmax(out));
	set_string(3, ok ? out : "", get_param(4));
	return ok;
}

public Float:n_getf(plugin, params)
{
	new s = arg_slot(1);
	new key[32];
	get_string(2, key, charsmax(key));
	new Float:def = get_param_f(3);
	return s >= 0 ? kf(s, key, def) : def;
}

public n_set(plugin, params)
{
	new s = arg_slot(1);
	if (s < 0)
		return 0;
	new key[32], val[256];
	get_string(2, key, charsmax(key));
	get_string(3, val, charsmax(val));
	TrieSetString(g_keys[s], key, val);
	g_now = get_gametime();
	/* re-read: areas, models and per-kind settings */
	new bool:on = g_on[s];
	new st = g_state[s], side = g_side[s];
	if (equal(key, "radius") || equal(key, "height") || equal(key, "name") || equal(key, "team") || equal(key, "targetname"))
	{
		g_r[s] = kf(s, "radius", default_radius(g_kind[s]));
		g_h[s] = kf(s, "height", 160.0);
		ks(s, "targetname", g_tname[s], charsmax(g_tname[]));
		ks(s, "name", g_label[s], charsmax(g_label[]), g_tname[s]);
		g_team[s] = kteam(s, "team", g_kind[s] == K_WAVE ? 1 : 0);
	}
	else if (equal(key, "door") || equal(key, "platform") || equal(key, "zone") || equal(key, "adopt"))
	{
		g_linked[s] = false;
		comp_link(s);
	}
	else if (!equal(key, "target") && !equal(key, "stages") && !equal(key, "types") && !equal(key, "action"))
	{
		comp_reset(s);
		g_on[s] = on;
		if (g_kind[s] == K_CAPTURE)
			g_side[s] = side;
		if (g_kind[s] == K_EXTRACT || g_kind[s] == K_DOOR || g_kind[s] == K_MEETING)
			g_state[s] = st;
	}
	hint_comp(s);
	return 1;
}

public n_enable(plugin, params)
{
	new s = arg_slot(1);
	if (s < 0)
		return 0;
	new bool:on = get_param(2) != 0;
	if (g_on[s] == on)
		return 1;
	g_on[s] = on;
	g_now = get_gametime();
	if (!on)
	{
		hint_forget(s);
		if (g_kind[s] == K_MEETING && g_meeting == s)
			meeting_finish(false);
		for (new p = 1; p <= MaxClients; p++)
		{
			if (g_act[p] == s + 1)
				act_cancel(p);
			if (g_xslot[p] == s + 1)
				extract_cancel(p);
		}
	}
	else
		hint_comp(s);
	return 1;
}

public n_enabled(plugin, params)
{
	new s = arg_slot(1);
	return s >= 0 && g_on[s];
}

public n_start(plugin, params)
{
	g_now = get_gametime();
	return start_or_stop(get_param(1), true, 0);
}

public n_stop(plugin, params)
{
	g_now = get_gametime();
	return start_or_stop(get_param(1), false, 0);
}

public n_reset(plugin, params)
{
	g_now = get_gametime();
	new ent = get_param(1);
	if (!ent)
	{
		for (new s = 0; s < g_nslots; s++)
			if (g_ent[s])
				comp_reset(s);
		return 1;
	}
	new s = arg_slot(1);
	if (s >= 0)
		comp_reset(s);
	return s >= 0;
}

public n_load_pack(plugin, params)
{
	new rel[160], dir[160], path[352];
	get_string(1, rel, charsmax(rel));
	if (contain(rel, "..") >= 0)
	{
		log_error(AMX_ERR_NATIVE, "Map pack paths stay under configs/");
		return 0;
	}
	get_configsdir(dir, charsmax(dir));
	formatex(path, charsmax(path), "%s/%s", dir, rel);
	g_now = get_gametime();
	return load_pack(path);
}

public n_inside(plugin, params)
{
	new s = arg_slot(1);
	new id = get_param(2);
	if (s < 0 || !(1 <= id <= MaxClients) || !is_user_alive(id))
		return 0;
	new Float:o[3];
	pev(id, pev_origin, o);
	if (g_kind[s] == K_ZONE)
		return dist2d(o, g_va[s]) <= zone_radius_now(s);
	return inside(s, o);
}

public n_brain(plugin, params)
{
	return g_brain;
}

public n_bots_hint(plugin, params)
{
	new line[192];
	get_string(1, line, charsmax(line));
	if (!line[0] || contain(line, ";") >= 0 || contain(line, "^n") >= 0)
		return 0;
	return hint("%s", line);
}

public n_give_item(plugin, params)
{
	new item[32];
	get_string(2, item, charsmax(item));
	return give_thing(get_param(1), item);
}

public n_cp_owner(plugin, params)
{
	new s = arg_slot(1, K_CAPTURE);
	return s >= 0 ? g_side[s] : 0;
}

public Float:n_cp_progress(plugin, params)
{
	new s = arg_slot(1, K_CAPTURE);
	return s >= 0 ? g_val[s] : 0.0;
}

public n_cp_capturer(plugin, params)
{
	new s = arg_slot(1, K_CAPTURE);
	return s >= 0 ? g_aux[s] : 0;
}

public n_cp_set_owner(plugin, params)
{
	new s = arg_slot(1, K_CAPTURE);
	if (s < 0)
		return 0;
	g_now = get_gametime();
	g_val[s] = 0.0;
	g_aux[s] = 0;
	new side = get_param(2);
	if (side == 0)
	{
		new old = g_side[s];
		g_side[s] = 0;
		if (old)
			fwd2(g_fwLost, g_ent[s], old);
		hint_comp(s);
	}
	else
		capture_set(s, side, 0);
	return 1;
}

public n_cp_lock(plugin, params)
{
	new s = arg_slot(1, K_CAPTURE);
	if (s < 0)
		return 0;
	new Float:secs = get_param_f(2);
	g_pf[s][CP_LOCK] = secs > 0.0 ? get_gametime() + secs : 0.0;
	return 1;
}

public n_zone_set(plugin, params)
{
	new s = zone_arg(1);
	if (s < 0)
		return 0;
	new Float:c[3];
	get_array_f(2, c, 3);
	g_now = get_gametime();
	zone_set(s, c, get_param_f(3));
	return 1;
}

public n_zone_shrink(plugin, params)
{
	new s = zone_arg(1);
	if (s < 0)
		return 0;
	new Float:c[3];
	get_array_f(2, c, 3);
	g_now = get_gametime();
	zone_shrink(s, c, get_param_f(3), get_param_f(4));
	return 1;
}

public Float:n_zone_radius(plugin, params)
{
	new s = zone_arg(1);
	if (s < 0)
		return 0.0;
	g_now = get_gametime();
	return zone_radius_now(s);
}

public n_zone_center(plugin, params)
{
	new s = zone_arg(1);
	if (s < 0)
		return 0;
	set_array_f(2, g_va[s], 3);
	return 1;
}

public n_zone_inside(plugin, params)
{
	new ent = get_param(1), id = get_param(2);
	if (!(1 <= id <= MaxClients) || !is_user_alive(id))
		return 0;
	new Float:o[3];
	pev(id, pev_origin, o);
	g_now = get_gametime();
	if (ent)
	{
		/* (any component: a safe zone by its circle, anything else by its area) */
		new s = slot_of(ent);
		if (s < 0 || !g_ent[s])
			return 0;
		if (g_kind[s] != K_ZONE)
			return inside(s, o);
		return dist2d(o, g_va[s]) <= zone_radius_now(s);
	}
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_ZONE && g_on[s] && dist2d(o, g_va[s]) > zone_radius_now(s))
			return 0;
	return 1;
}

public n_zone_damage(plugin, params)
{
	new s = zone_arg(1);
	if (s < 0)
		return 0;
	g_pf[s][Z_DMG] = get_param_f(2);
	return 1;
}

public n_zone_stage(plugin, params)
{
	new s = arg_slot(1);
	if (s < 0)
		return 0;
	if (g_kind[s] == K_SHRINKER)
		return floatround(g_pf[s][S_STAGE]);
	if (g_kind[s] == K_ZONE)
		return floatround(g_pf[s][Z_STAGE]);
	return 0;
}

public n_zone_skip(plugin, params)
{
	new s = arg_slot(1);
	if (s < 0)
		return 0;
	if (g_kind[s] == K_ZONE)
	{
		for (new k = 0; k < g_nslots; k++)
			if (g_ent[k] && g_kind[k] == K_SHRINKER && g_aux[k] == s)
			{
				s = k;
				break;
			}
	}
	if (g_kind[s] != K_SHRINKER)
		return 0;
	g_now = get_gametime();
	if (g_state[s] != 1)
	{
		g_state[s] = 1;
		g_pf[s][S_PHASE] = 0.0;
	}
	g_pf[s][S_NEXT] = g_now;
	return 1;
}

public n_door_open(plugin, params)
{
	new s = arg_slot(1, K_DOOR);
	if (s >= 0)
		door_use(s, true);
	return s >= 0;
}

public n_door_close(plugin, params)
{
	new s = arg_slot(1, K_DOOR);
	if (s >= 0)
		door_use(s, false);
	return s >= 0;
}

public n_door_lock(plugin, params)
{
	new s = arg_slot(1, K_DOOR);
	if (s < 0)
		return 0;
	g_now = get_gametime();
	door_lock(s, get_param_f(2), 0);
	return 1;
}

public n_door_is_open(plugin, params)
{
	new s = arg_slot(1, K_DOOR);
	return s >= 0 && door_is_open(s);
}

public n_trap_fire(plugin, params)
{
	new s = arg_slot(1, K_TRAP);
	if (s < 0)
		return 0;
	g_now = get_gametime();
	return trap_fire(s, get_param(2));
}

public n_trap_arm(plugin, params)
{
	new s = arg_slot(1, K_TRAP);
	if (s < 0)
		return 0;
	g_state[s] = get_param(2) ? TRAP_ARMED : TRAP_OFF;
	hint_comp(s);
	return 1;
}

public n_spawner_set_item(plugin, params)
{
	new s = arg_slot(1, K_WEAPON);
	if (s < 0)
		return 0;
	new item[48];
	get_string(2, item, charsmax(item));
	TrieSetString(g_keys[s], "item", item);
	spawner_roll(s);
	if (g_state[s] == 0)
		spawner_show(s, true);
	return 1;
}

public n_spawner_respawn(plugin, params)
{
	new s = arg_slot(1, K_WEAPON);
	if (s < 0)
		return 0;
	spawner_roll(s);
	spawner_show(s, true);
	fwd1(g_fwWeaponBack, g_ent[s]);
	return 1;
}

public n_loot_fill(plugin, params)
{
	new s = arg_slot(1, K_LOOT);
	if (s < 0)
		return 0;
	new t[32];
	get_string(2, t, charsmax(t));
	if (t[0])
		copy(g_item[s], charsmax(g_item[]), t);
	loot_look(s, true);
	return 1;
}

public n_loot_open(plugin, params)
{
	new ent = get_param(1);
	new s = arg_slot(1);
	if (s < 0 || (g_kind[s] != K_LOOT && g_kind[s] != K_CRATE))
	{
		if (s >= 0)
			log_error(AMX_ERR_NATIVE, "Entity %d isn't a loot container or crate", ent);
		return 0;
	}
	new id = get_param(2);
	if (!(1 <= id <= MaxClients) || !is_user_alive(id))
		return 0;
	g_now = get_gametime();
	loot_give(s, id);
	return 1;
}

public n_loot_roll(plugin, params)
{
	new t[32], out[32];
	get_string(1, t, charsmax(t));
	new ok = loot_roll(t, out, charsmax(out), get_param(4));
	set_string(2, out, get_param(3));
	return ok;
}

public n_drop_call(plugin, params)
{
	new Float:o[3], t[32], keys[96];
	get_array_f(1, o, 3);
	get_string(2, t, charsmax(t));
	formatex(keys, charsmax(keys), "table=%s", t[0] ? t : "drop");
	g_now = get_gametime();
	return drop_call(o, keys);
}

public n_barricade_hp(plugin, params)
{
	new s = arg_slot(1, K_BARRICADE);
	return s >= 0 ? floatround(g_val[s]) : 0;
}

public n_barricade_set_hp(plugin, params)
{
	new s = arg_slot(1, K_BARRICADE);
	if (s < 0)
		return 0;
	new hp = get_param(2);
	g_val[s] = floatmin(float(max(hp, 0)), g_pf[s][B_MAX]);
	if (hp <= 0 && g_state[s] == 0)
		barricade_break(s, 0);
	else if (hp > 0)
	{
		if (g_state[s] == 1)
			barricade_stand(s, true);
		set_pev(g_ent[s], pev_health, g_val[s]);
		barricade_tint(s);
	}
	return 1;
}

public n_barricade_repair(plugin, params)
{
	new s = arg_slot(1, K_BARRICADE);
	if (s < 0)
		return 0;
	new id = get_param(2);
	barricade_repair(s, (1 <= id <= MaxClients) ? id : 0);
	return 1;
}

public n_wave_start(plugin, params)
{
	g_now = get_gametime();
	wave_start(get_param(1));
	return g_wave;
}

public n_wave_stop(plugin, params)
{
	wave_stop();
	return 1;
}

public n_wave_current(plugin, params)
{
	return g_wave;
}

public n_wave_alive(plugin, params)
{
	return wave_alive();
}

public n_wave_track(plugin, params)
{
	new e = get_param(1);
	if (e <= 0 || !pev_valid(e))
		return 0;
	wave_track(e);
	return 1;
}

public n_extract_open(plugin, params)
{
	new s = arg_slot(1, K_EXTRACT);
	if (s < 0)
		return 0;
	g_now = get_gametime();
	extract_set(s, get_param(2) != 0);
	return 1;
}

public n_extract_is_open(plugin, params)
{
	new s = arg_slot(1, K_EXTRACT);
	return s >= 0 && g_state[s] == 1;
}

public n_extracted(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= MaxClients) && g_extracted[id];
}

public n_checkpoint_of(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= MaxClients) ? g_cpLast[id] : 0;
}

public n_checkpoint_laps(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= MaxClients) ? g_laps[id] : 0;
}

public n_checkpoint_reset(plugin, params)
{
	new id = get_param(1);
	for (new p = 1; p <= MaxClients; p++)
	{
		if (id && p != id)
			continue;
		g_cpLast[p] = 0;
		g_laps[p] = 0;
	}
	return 1;
}

public n_platform_go(plugin, params)
{
	new s = arg_slot(1, K_PLATFORM);
	if (s >= 0)
		platform_go(s);
	return s >= 0;
}

public n_platform_moving(plugin, params)
{
	new s = arg_slot(1, K_PLATFORM);
	return s >= 0 && platform_moving(s);
}

public n_class_set(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= MaxClients) || !is_user_connected(id))
		return 0;
	new name[32];
	get_string(2, name, charsmax(name));
	if (!name[0])
		return class_set(id, -1);
	new s = class_by_name(name);
	if (s < 0)
	{
		log_error(AMX_ERR_NATIVE, "No ls_player_class named ^"%s^"", name);
		return 0;
	}
	g_now = get_gametime();
	return class_set(id, s);
}

public n_class_get(plugin, params)
{
	new id = get_param(1);
	new s = (1 <= id <= MaxClients) ? g_class[id] - 1 : -1;
	set_string(2, (s >= 0 && g_ent[s]) ? g_label[s] : "", get_param(3));
	return s >= 0;
}

public n_class_menu(plugin, params)
{
	new id = get_param(1);
	if ((1 <= id <= MaxClients) && is_user_connected(id) && !is_user_bot(id))
		class_menu(id);
	return 1;
}

public n_class_find(plugin, params)
{
	new name[32];
	get_string(1, name, charsmax(name));
	new s = class_by_name(name);
	return s >= 0 ? g_ent[s] : 0;
}

public n_meeting_call(plugin, params)
{
	new s = arg_slot(1, K_MEETING);
	if (s < 0)
		return 0;
	g_now = get_gametime();
	return meeting_call(s, get_param(2));
}

public n_meeting_end(plugin, params)
{
	if (g_meeting < 0)
		return 0;
	g_now = get_gametime();
	meeting_finish(true);
	return 1;
}

public n_meeting_active(plugin, params)
{
	return g_meeting >= 0;
}

public n_meeting_vote(plugin, params)
{
	return meeting_vote(get_param(1), get_param(2));
}

public n_respawn_pick(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= MaxClients) || !is_user_connected(id))
		return 0;
	new s = respawn_pick(id);
	return s >= 0 ? g_ent[s] : 0;
}

public n_respawn_at(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= MaxClients) || !is_user_connected(id) || is_user_alive(id))
		return 0;
	new team = get_user_team(id);
	if (team != 1 && team != 2)
		return 0;
	new ent = get_param(2);
	new s = ent ? arg_slot(2, K_RESPAWN) : respawn_pick(id);
	ExecuteHamB(Ham_CS_RoundRespawn, id);
	if (s >= 0 && is_user_alive(id))
	{
		remove_task(TASK_SPAWN + id);
		class_apply(id);
		respawn_place(id, s);
	}
	return is_user_alive(id);
}

/* seconds to the zone's next change; the next circle; closing = 1 while it shrinks */
public Float:n_zone_next(plugin, params)
{
	new ent = get_param(1);
	new z = -1;
	if (ent)
		z = zone_arg(1);
	else
		for (new k = 0; k < g_nslots && z < 0; k++)
			if (g_ent[k] && g_kind[k] == K_ZONE)
				z = k;
	if (z < 0)
		return -1.0;
	g_now = get_gametime();
	set_array_f(2, g_vb[z], 3);
	set_float_byref(3, g_pf[z][Z_TOR]);
	new bool:closing = g_pf[z][Z_T1] > g_pf[z][Z_T0] && g_now < g_pf[z][Z_T1];
	set_param_byref(4, closing ? 1 : 0);
	if (closing)
		return g_pf[z][Z_T1] - g_now;
	for (new k = 0; k < g_nslots; k++)
		if (g_ent[k] && g_kind[k] == K_SHRINKER && g_aux[k] == z && g_state[k] == 1)
			return floatmax(g_pf[k][S_NEXT] - g_now, 0.0);
	return -1.0;
}

/* a component's state: loot crate 0 full 1 empty; spawner 0 there 1 taken; extraction 1 open ... */
public n_comp_state(plugin, params)
{
	new s = arg_slot(1);
	return s >= 0 ? g_state[s] : -1;
}

/* ==== W1: Last Stand pieces ==== */
/* =========================================================================
 * Player modifiers, perks, the downed state, weapon levels; the perk machine, the upgrade
 * bench, the mystery box, power-up pickups, and the paid timed trap (an ls_trigger_trap with
 * fire_on=use and a cost). Built for Last Stand, usable by any mode (natives in
 * ls_components.inc, "W1: Last Stand pieces"). Bots get these as W2's ls_mode offers.
 * ========================================================================= */

enum { MOD_HEALTH = 0, MOD_SPEED, MOD_RELOAD, MOD_DEPLOY, MOD_DAMAGE, MOD_REVIVE, MOD_FIRE, MOD_COUNT };
new const MOD_ID[MOD_COUNT][] = { "health_max", "speed", "reload", "deploy", "damage", "revive_time", "fire_rate" };
new Float:g_modSet[33][MOD_COUNT];

enum { PK_TOUGH = 0, PK_HANDS, PK_DOUBLE, PK_REVIVE, PK_FEET, PK_COUNT };
new const PK_ID[PK_COUNT][] = { "tough", "hands", "doubletap", "revive", "feet" };
new const PK_NAME[PK_COUNT][] = { "Tough", "Quick hands", "Double tap", "Quick revive", "Light feet" };
new const PK_WHAT[PK_COUNT][] = { "+50% max health", "faster reloads and weapon swaps", "+25% damage", "revive in 2 s; alone, get back up once", "+10% speed" };
new const PK_TAG[PK_COUNT][] = { "TGH", "QH", "DT", "QR", "LF" };
new const PK_RGB[PK_COUNT][3] = { {220, 60, 60}, {60, 200, 80}, {240, 200, 40}, {60, 140, 255}, {200, 120, 255} };
/* the order bots buy them in (W2's offer order, lowest first) */
new const PK_ORDER[PK_COUNT] = { 0, 4, 3, 2, 1 };
new g_perks[33];
new g_nperks[33];
new bool:g_selfRev[33];

/* the downed state */
new bool:g_dnOn;
new Float:g_dnBleed = 30.0;
new g_dnTeam;
new bool:g_dnLosePerks = true;
new bool:g_down[33];
new bool:g_bledOut[33];
new Float:g_downAt[33];
new Float:g_downOrg[33][3];
new g_revBy[33];
new Float:g_revProg[33];
new Float:g_selfRevAt[33];

/* the last hit on a creature was a head */
new bool:g_headLast[MAX_ENTS];
new Float:g_upUseNext[33];
new Float:g_offerAt;
new bool:g_upCreatures;
#define OFS_NEXT_ATTACK 83
#define OFS_WPN_IDLE    48
#define OFS_NEXT_PRIM   46
/* a weapon's level (the bench): kept on the weapon entity, so it goes with the gun */
#define WPN_LEVEL       pev_iuser3

/* perk machine */
#define PM_PRICE    0
#define PM_STEP     1
#define PM_MAX      2
/* bench */
#define BN_PRICE2   0
#define BN_PRICE3   1
#define BN_L3FROM   2
#define BN_DMG2     3
#define BN_DMG3     4
/* box: g_state 1 = here now; g_aux = uses since it came */
#define BX_PRICE    0
#define BX_MOVEAT   1
/* pickup */
#define PU_UNTIL    0
#define PU_RADIUS   1
/* the paid timed trap (slots after the trap's own T_*) */
#define T_UNTIL     5
#define T_NEXTHIT   6

new g_fwPerk, g_fwDown, g_fwRevive, g_fwBledOut, g_fwUpgraded, g_fwBox, g_fwPickup, g_fwCreatureHurt;

/* turrets: friendly creatures (the fork's friendly=1) a player places from a kit; bots by W2's offer */
#define MAX_TUR 16
new g_turPrice, Float:g_turLife = 150.0, g_turPer = 1, g_turType[24] = "turret", g_turHealth, bool:g_turBots = true;
new g_turKits[33];
new g_turEnt[MAX_TUR], g_turOwner[MAX_TUR], Float:g_turUntil[MAX_TUR];
new Float:g_turOfferOrg[33][3];
new bool:g_turOffered[33];
new g_fwTurret;

/* shop */
#define SH_NEXT     0
#define SH_BACK     1
#define SH_SOLID    2
#define SH_KIND     3
#define SH_MAX_ITEMS 12

new g_shopIn[33];
new g_shopVisits[33];
new Float:g_shopMenuUntil[33];
new g_shopMenuSlot[33];
new g_shopWhy[64];
new g_shopCb;
new g_fwShopItem, g_fwShopBuy;

new const KEEPER_FIDGETS[3][3][] = { { "", "", "" }, { "idle2", "idle3", "idle4" }, { "idlebrush", "idlelook", "idle02" } };
new const KEEPER_LINES[3][4][] =
{
	{ "", "", "", "" },
	{ "Need ammo?", "Stay safe out there.", "That'll hold 'em.", "Watch your back, pal." },
	{ "Prepare for... the next wave.", "Wise... choice.", "Time to... choose.", "Rise and... shine." }
};


stock up_precache()
{
	precache_model_once("models/w_battery.mdl");
	precache_sound("buttons/spark5.wav");
	precache_sound("items/suitchargeok1.wav");
	shop_precache();
	for (new p = 0; p <= 32; p++)
		for (new m = 0; m < MOD_COUNT; m++)
			g_modSet[p][m] = 1.0;
}

stock up_natives()
{
	register_native("ls_player_mod", "n_player_mod");
	register_native("ls_player_mod_get", "n_player_mod_get");
	register_native("ls_player_mod_clear", "n_player_mod_clear");
	register_native("ls_perk_give", "n_perk_give");
	register_native("ls_perk_has", "n_perk_has");
	register_native("ls_perk_count", "n_perk_count");
	register_native("ls_perk_clear", "n_perk_clear");
	register_native("ls_perk_buy", "n_perk_buy");
	register_native("ls_perk_price", "n_perk_price");
	register_native("ls_perk_tags", "n_perk_tags");
	register_native("ls_down_enable", "n_down_enable");
	register_native("ls_is_down", "n_is_down");
	register_native("ls_down", "n_down");
	register_native("ls_revive", "n_revive");
	register_native("ls_down_left", "n_down_left");
	register_native("ls_weapon_level", "n_weapon_level");
	register_native("ls_weapon_set_level", "n_weapon_set_level");
	register_native("ls_bench_use", "n_bench_use");
	register_native("ls_box_use", "n_box_use");
	register_native("ls_pickup_spawn", "n_pickup_spawn");
	register_native("ls_turret_setup", "n_turret_setup");
	register_native("ls_turret_give", "n_turret_give");
	register_native("ls_turret_kits", "n_turret_kits");
	register_native("ls_turret_place", "n_turret_place");
	register_native("ls_turret_spawn", "n_turret_spawn");
	register_native("ls_turret_count", "n_turret_count");
	register_native("ls_shop_buy", "n_shop_buy");
	register_native("ls_shop_price", "n_shop_price");
	register_native("ls_shop_menu", "n_shop_menu");
	register_native("ls_shop_open", "n_shop_open");
	register_native("ls_shop_is_open", "n_shop_is_open");
	register_native("ls_shop_why", "n_shop_why");
	register_native("ls_shop_at", "n_shop_at");
}

stock up_plugin_init()
{
	g_fwPerk = CreateMultiForward("ls_on_perk", ET_STOP, FP_CELL, FP_STRING, FP_CELL);
	g_fwDown = CreateMultiForward("ls_on_down", ET_STOP, FP_CELL, FP_CELL);
	g_fwRevive = CreateMultiForward("ls_on_revive", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwBledOut = CreateMultiForward("ls_on_bled_out", ET_IGNORE, FP_CELL);
	g_fwUpgraded = CreateMultiForward("ls_on_weapon_upgraded", ET_IGNORE, FP_CELL, FP_CELL, FP_CELL, FP_CELL);
	g_fwBox = CreateMultiForward("ls_on_box", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	g_fwPickup = CreateMultiForward("ls_on_pickup", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	g_fwCreatureHurt = CreateMultiForward("ls_on_creature_hurt", ET_IGNORE, FP_CELL, FP_CELL, FP_FLOAT, FP_CELL);
	g_fwTurret = CreateMultiForward("ls_on_turret", ET_IGNORE, FP_CELL, FP_CELL);
	g_fwShopItem = CreateMultiForward("ls_on_shop_item", ET_STOP, FP_CELL, FP_CELL, FP_STRING);
	g_fwShopBuy = CreateMultiForward("ls_on_shop_buy", ET_STOP, FP_CELL, FP_CELL, FP_STRING, FP_CELL);

	register_forward(FM_PlayerPreThink, "up_prethink");
	RegisterHamPlayer(Ham_TakeDamage, "up_player_hurt");
	RegisterHamPlayer(Ham_Killed, "up_player_killed", 1);
	RegisterHamPlayer(Ham_CS_Player_ResetMaxSpeed, "up_maxspeed", 1);
	g_upCreatures = get_cvar_pointer("ls_creature_ver") != 0;
	if (g_upCreatures)
	{
		RegisterHam(Ham_TakeDamage, "ls_creature", "up_creature_hurt");
		RegisterHam(Ham_TakeDamage, "ls_creature", "up_creature_hurt_post", 1);
		RegisterHam(Ham_TraceAttack, "ls_creature", "up_creature_trace");
	}
	new const WEAPONS[][] = { "weapon_p228", "weapon_scout", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_elite", "weapon_fiveseven", "weapon_ump45",
		"weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1",
		"weapon_tmp", "weapon_g3sg1", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_p90", "weapon_knife" };
	for (new i = 0; i < sizeof WEAPONS; i++)
	{
		if (!equal(WEAPONS[i], "weapon_knife"))
		{
			RegisterHam(Ham_Weapon_Reload, WEAPONS[i], "up_reload_post", 1);
			RegisterHam(Ham_Weapon_PrimaryAttack, WEAPONS[i], "up_attack_post", 1);
		}
		RegisterHam(Ham_Item_Deploy, WEAPONS[i], "up_deploy_post", 1);
	}
}

/* ---- player modifiers: what a mode sets times what the perks give ---- */

stock mod_index(const what[])
{
	for (new m = 0; m < MOD_COUNT; m++)
		if (equali(what, MOD_ID[m]))
			return m;
	return -1;
}

stock Float:mod_get(id, m)
{
	if (!(1 <= id <= 32))
		return 1.0;
	new Float:v = g_modSet[id][m];
	switch (m)
	{
		case MOD_HEALTH: if (perk_has(id, PK_TOUGH)) v *= 1.5;
		case MOD_SPEED: if (perk_has(id, PK_FEET)) v *= 1.1;
		case MOD_RELOAD, MOD_DEPLOY: if (perk_has(id, PK_HANDS)) v *= 0.6;
		case MOD_DAMAGE: if (perk_has(id, PK_DOUBLE)) v *= 1.25;
		case MOD_REVIVE: if (perk_has(id, PK_REVIVE)) v *= 0.5;
	}
	return v;
}

/* max health and speed follow the modifiers at once */
stock mods_apply(id, Float:oldMax = 0.0)
{
	if (!is_user_alive(id))
		return;
	new Float:mx = 100.0 * mod_get(id, MOD_HEALTH);
	if (oldMax <= 0.0)
	{
		pev(id, pev_max_health, oldMax);
		if (oldMax <= 0.0)
			oldMax = 100.0;
	}
	set_pev(id, pev_max_health, mx);
	new hp = get_user_health(id);
	if (mx > oldMax)
		set_user_health(id, min(hp + floatround(mx - oldMax), floatround(mx)));
	else if (float(hp) > mx)
		set_user_health(id, floatround(mx));
	ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, id);
}

public up_maxspeed(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED;
	new Float:s;
	pev(id, pev_maxspeed, s);
	if (g_down[id])
		set_pev(id, pev_maxspeed, 50.0);
	else
	{
		new Float:m = mod_get(id, MOD_SPEED);
		if (m != 1.0 && s > 1.0)
			set_pev(id, pev_maxspeed, s * m);
	}
	return HAM_IGNORED;
}

public up_reload_post(weapon)
{
	new id = pev(weapon, pev_owner);
	if (!(1 <= id <= 32) || !is_user_alive(id))
		return HAM_IGNORED;
	new Float:m = mod_get(id, MOD_RELOAD);
	if (m == 1.0)
		return HAM_IGNORED;
	new Float:now = get_gametime();
	new Float:na = get_pdata_float(id, OFS_NEXT_ATTACK, 5);
	if (na > 0.0)
		set_pdata_float(id, OFS_NEXT_ATTACK, na * m, 5);
	new Float:idle = get_pdata_float(weapon, OFS_WPN_IDLE, 4);
	if (idle > now)
		set_pdata_float(weapon, OFS_WPN_IDLE, now + (idle - now) * m, 4);
	return HAM_IGNORED;
}

public up_deploy_post(weapon)
{
	new id = pev(weapon, pev_owner);
	if (!(1 <= id <= 32) || !is_user_alive(id))
		return HAM_IGNORED;
	new Float:m = mod_get(id, MOD_DEPLOY);
	if (m == 1.0)
		return HAM_IGNORED;
	new Float:na = get_pdata_float(id, OFS_NEXT_ATTACK, 5);
	if (na > 0.0)
		set_pdata_float(id, OFS_NEXT_ATTACK, na * m, 5);
	return HAM_IGNORED;
}

public up_attack_post(weapon)
{
	new id = pev(weapon, pev_owner);
	if (!(1 <= id <= 32) || !is_user_alive(id))
		return HAM_IGNORED;
	new Float:m = mod_get(id, MOD_FIRE);
	if (m == 1.0 || m <= 0.0)
		return HAM_IGNORED;
	/* fire_rate 1.25 = 25 % faster: the wait to the next shot divided by it */
	new Float:na = get_pdata_float(weapon, OFS_NEXT_PRIM, 4);
	if (na > 0.0)
		set_pdata_float(weapon, OFS_NEXT_PRIM, na / m, 4);
	return HAM_IGNORED;
}

/* ---- perks ---- */

stock perk_index(const name[])
{
	for (new k = 0; k < PK_COUNT; k++)
		if (equali(name, PK_ID[k]))
			return k;
	/* longer names the Studio may use */
	if (equali(name, "quickhands")) return PK_HANDS;
	if (equali(name, "quickrevive")) return PK_REVIVE;
	if (equali(name, "lightfeet")) return PK_FEET;
	if (equali(name, "double_tap")) return PK_DOUBLE;
	return -1;
}

stock bool:perk_has(id, k)
{
	return 1 <= id <= 32 && k >= 0 && (g_perks[id] & (1 << k)) != 0;
}

stock perk_give(id, k)
{
	if (!is_user_alive(id) || k < 0 || perk_has(id, k))
		return 0;
	new Float:oldMax;
	pev(id, pev_max_health, oldMax);
	g_perks[id] |= (1 << k);
	g_nperks[id]++;
	if (k == PK_REVIVE)
		g_selfRev[id] = true;
	mods_apply(id, oldMax);
	return 1;
}

stock perk_clear(id)
{
	g_perks[id] = 0;
	g_nperks[id] = 0;
	g_selfRev[id] = false;
	if (is_user_alive(id))
		mods_apply(id);
}

/* the price at a machine (or with its keys: price, price_step): dearer for each one owned */
stock perk_price_at(s, id)
{
	new base = 1500, step = 1000;
	if (s >= 0)
	{
		base = floatround(g_pf[s][PM_PRICE]);
		step = floatround(g_pf[s][PM_STEP]);
	}
	return base + step * g_nperks[id];
}

stock perk_why_not(s, id, k, out[], len)
{
	out[0] = EOS;
	new mx = s >= 0 ? floatround(g_pf[s][PM_MAX]) : 4;
	if (perk_has(id, k))
		copy(out, len, "you have it");
	else if (mx > 0 && g_nperks[id] >= mx)
		formatex(out, len, "%d perks at most", mx);
	else if (cs_get_user_money(id) < perk_price_at(s, id))
		copy(out, len, "not enough money");
}

/* buying: at machine s (or -1: the default prices) */
stock bool:perk_buy(s, id, k, bool:tell)
{
	if (k < 0 || !is_user_alive(id) || g_down[id])
		return false;
	new why[48];
	perk_why_not(s, id, k, why, charsmax(why));
	if (why[0])
	{
		if (tell)
			client_print(id, print_center, "%s: %s", PK_NAME[k], why);
		return false;
	}
	new price = perk_price_at(s, id);
	new ret;
	ExecuteForward(g_fwPerk, ret, id, PK_ID[k], price);
	if (ret == PLUGIN_HANDLED)
		return false;
	cs_set_user_money(id, cs_get_user_money(id) - price);
	perk_give(id, k);
	if (!g_bot[id])
		client_print(id, print_center, "Perk: %s (%s)", PK_NAME[k], PK_WHAT[k]);
	emit_sound(id, CHAN_ITEM, "items/suitchargeok1.wav", 0.6, ATTN_NORM, 0, PITCH_NORM);
	log_amx("Upgrades: %d bought the %s perk for $%d in wave %d", id, PK_NAME[k], price, g_wave);
	return true;
}

stock perk_tags(id, out[], len)
{
	out[0] = EOS;
	for (new k = 0; k < PK_COUNT; k++)
		if (perk_has(id, k))
			format(out, len, "%s%s%s", out, out[0] ? " " : "Perks: ", PK_TAG[k]);
}

/* ---- weapon levels (the bench) ---- */

stock active_weapon(id)
{
	new w = get_pdata_cbase(id, 373, 5);	/* m_pActiveItem */
	if (w <= 0 || !pev_valid(w))
		return 0;
	new csw = get_user_weapon(id);
	if (csw == CSW_KNIFE || csw == CSW_HEGRENADE || csw == CSW_FLASHBANG || csw == CSW_SMOKEGRENADE || csw == CSW_C4)
		return 0;
	return w;
}

stock wpn_level(w)
{
	return (w > 0 && pev_valid(w)) ? max(pev(w, WPN_LEVEL), 1) : 1;
}

stock Float:level_mult(s, level)
{
	if (level >= 3)
		return s >= 0 ? g_pf[s][BN_DMG3] : 1.9;
	if (level == 2)
		return s >= 0 ? g_pf[s][BN_DMG2] : 1.4;
	return 1.0;
}

/* the damage a level adds: the first bench's keys (or the defaults) */
stock Float:wpn_mult(w)
{
	new lv = wpn_level(w);
	if (lv <= 1)
		return 1.0;
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_BENCH)
			return level_mult(s, lv);
	return level_mult(-1, lv);
}

stock bench_price(s, level)
{
	if (level >= 2)
		return s >= 0 ? floatround(g_pf[s][BN_PRICE3]) : 9000;
	return s >= 0 ? floatround(g_pf[s][BN_PRICE2]) : 5000;
}

stock bench_why_not(s, id, out[], len)
{
	out[0] = EOS;
	new w = active_weapon(id);
	if (!w)
	{
		copy(out, len, "hold a gun");
		return 0;
	}
	new lv = wpn_level(w);
	new from = s >= 0 ? floatround(g_pf[s][BN_L3FROM]) : 12;
	if (lv >= 3)
		copy(out, len, "it's Mk III already");
	else if (lv == 2 && g_wave < from)
		formatex(out, len, "Mk III from wave %d", from);
	else if (cs_get_user_money(id) < bench_price(s, lv))
		copy(out, len, "not enough money");
	return w;
}

stock bool:bench_use(s, id, bool:tell)
{
	if (!is_user_alive(id) || g_down[id])
		return false;
	new why[48];
	new w = bench_why_not(s, id, why, charsmax(why));
	if (why[0])
	{
		if (tell)
			client_print(id, print_center, "Upgrade: %s", why);
		return false;
	}
	new lv = wpn_level(w);
	new price = bench_price(s, lv);
	cs_set_user_money(id, cs_get_user_money(id) - price);
	lv = lv >= 2 ? 3 : 2;
	set_pev(w, WPN_LEVEL, lv);
	/* a magazine and a half, and the reserve filled */
	new clip = cs_get_weapon_ammo(w);
	cs_set_weapon_ammo(w, max(clip, 1) * 3 / 2 + 5);
	give_thing(id, "ammo");
	new wname[32];
	get_weaponname(get_user_weapon(id), wname, charsmax(wname));
	replace(wname, charsmax(wname), "weapon_", "");
	strtoupper(wname);
	if (!g_bot[id])
		client_print(id, print_center, "%s Mk %s!", wname, lv == 3 ? "III" : "II");
	emit_sound(id, CHAN_ITEM, "items/gunpickup2.wav", 0.8, ATTN_NORM, 0, PITCH_NORM);
	log_amx("Upgrades: %d upgraded a %s to Mk %d for $%d in wave %d", id, wname, lv, price, g_wave);
	new ret;
	ExecuteForward(g_fwUpgraded, ret, id, w, lv, price);
	return true;
}

/* ---- the mystery box ---- */

new const BOX_DEFAULT[] = "mp5navy:10 ump45:10 famas:8 galil:8 m3:8 xm1014:6 deagle:8 scout:5 p90:8 m4a1:5 ak47:5 aug:4 sg552:4 awp:3 m249:2";

stock box_pick(s, out[], len)
{
	new list[320], tok[32];
	ks(s, "guns", list, charsmax(list), BOX_DEFAULT);
	/* two passes: the total weight, then the roll */
	new total = 0, at = 0;
	for (;;)
	{
		new n = next_token(list[at], tok, charsmax(tok));
		if (!tok[0])
			break;
		at += n;
		new c = contain(tok, ":");
		total += c >= 0 ? max(str_to_num(tok[c + 1]), 0) : 1;
	}
	new r = random(max(total, 1));
	out[0] = EOS;
	at = 0;
	for (;;)
	{
		new n = next_token(list[at], tok, charsmax(tok));
		if (!tok[0])
			break;
		at += n;
		new c = contain(tok, ":");
		new wgt = c >= 0 ? max(str_to_num(tok[c + 1]), 0) : 1;
		if (c >= 0)
			tok[c] = EOS;
		copy(out, len, tok);
		r -= wgt;
		if (r < 0)
			break;
	}
}

stock box_ammo(csw, out[], len)
{
	switch (csw)
	{
		case CSW_MP5NAVY, CSW_TMP, CSW_GLOCK18, CSW_ELITE: copy(out, len, "9mm");
		case CSW_UMP45, CSW_MAC10, CSW_USP: copy(out, len, "45ACP");
		case CSW_FAMAS, CSW_GALIL, CSW_M4A1, CSW_AUG, CSW_SG552, CSW_SG550: copy(out, len, "556Nato");
		case CSW_M3, CSW_XM1014: copy(out, len, "buckshot");
		case CSW_DEAGLE: copy(out, len, "50AE");
		case CSW_SCOUT, CSW_AK47, CSW_G3SG1: copy(out, len, "762Nato");
		case CSW_P90, CSW_FIVESEVEN: copy(out, len, "57mm");
		case CSW_AWP: copy(out, len, "338Magnum");
		case CSW_M249: copy(out, len, "556NatoBox");
		case CSW_P228: copy(out, len, "357SIG");
		default: out[0] = EOS;
	}
}

#define SECONDARY_BITS ((1<<CSW_USP)|(1<<CSW_GLOCK18)|(1<<CSW_DEAGLE)|(1<<CSW_P228)|(1<<CSW_ELITE)|(1<<CSW_FIVESEVEN))
#define NOT_PRIMARY_BITS (SECONDARY_BITS|(1<<CSW_KNIFE)|(1<<CSW_HEGRENADE)|(1<<CSW_FLASHBANG)|(1<<CSW_SMOKEGRENADE)|(1<<CSW_C4))

stock bool:box_use(s, id, bool:tell)
{
	if (!is_user_alive(id) || g_down[id] || g_state[s] != 1)
		return false;
	new price = floatround(g_pf[s][BX_PRICE]);
	if (cs_get_user_money(id) < price)
	{
		if (tell)
			client_print(id, print_center, "The box: not enough money");
		return false;
	}
	new gun[32], wname[40];
	box_pick(s, gun, charsmax(gun));
	formatex(wname, charsmax(wname), "weapon_%s", gun);
	new csw = get_weaponid(wname);
	if (!csw)
	{
		log_amx("The mystery box %d has no gun called %s.", g_ent[s], gun);
		return false;
	}
	new ret;
	ExecuteForward(g_fwBox, ret, g_ent[s], id, wname);
	if (ret == PLUGIN_HANDLED)
		return false;
	cs_set_user_money(id, cs_get_user_money(id) - price);
	new bool:secondary = ((1 << csw) & SECONDARY_BITS) != 0;
	/* drop what it replaces */
	new weapons[32], n;
	get_user_weapons(id, weapons, n);
	for (new i = 0; i < n; i++)
	{
		new bool:isSec = ((1 << weapons[i]) & SECONDARY_BITS) != 0;
		new bool:isPrim = !((1 << weapons[i]) & NOT_PRIMARY_BITS);
		if ((secondary && isSec) || (!secondary && isPrim))
		{
			new wn[32];
			get_weaponname(weapons[i], wn, charsmax(wn));
			engclient_cmd(id, "drop", wn);
		}
	}
	new w = give_item(id, wname);
	new ammo[16];
	box_ammo(csw, ammo, charsmax(ammo));
	if (ammo[0])
		ExecuteHamB(Ham_GiveAmmo, id, 200, ammo, 200);
	/* the rare one (super=m249): comes as a Mk II with a long belt */
	new sup[32];
	ks(s, "super", sup, charsmax(sup), "m249");
	new bool:super = equali(sup, gun) != 0;
	if (super && w > 0 && pev_valid(w))
	{
		set_pev(w, WPN_LEVEL, 2);
		cs_set_weapon_ammo(w, ki(s, "super_clip", 150));
	}
	new up[32];
	copy(up, charsmax(up), gun);
	strtoupper(up);
	if (!g_bot[id])
	{
		new sname[48];
		ks(s, "super_name", sname, charsmax(sname), "the Grinder");
		if (super)
			client_print(id, print_center, "%s - %s (Mk II, %d rounds)!", up, sname, ki(s, "super_clip", 150));
		else
			client_print(id, print_center, "%s!", up);
	}
	emit_sound(g_ent[s], CHAN_ITEM, "items/gunpickup2.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	log_amx("Upgrades: %d got a %s from the box for $%d in wave %d", id, up, price, g_wave);
	/* after a few uses it moves to another box of its group (if there is one) */
	if (++g_aux[s] >= floatround(g_pf[s][BX_MOVEAT]))
		box_move(s);
	return true;
}

stock box_group_of(s, out[], len)
{
	ks(s, "group", out, len, "box");
}

stock box_show(s, bool:here)
{
	g_state[s] = here ? 1 : 0;
	g_aux[s] = 0;
	g_pf[s][BX_MOVEAT] = float(box_moves(s));
	new e = g_ent[s];
	if (here)
	{
		set_pev(e, pev_effects, pev(e, pev_effects) & ~EF_NODRAW);
		render(e, kRenderFxGlowShell, 80, 200, 255, kRenderNormal, 18);
	}
	else
		set_pev(e, pev_effects, pev(e, pev_effects) | EF_NODRAW);
}

/* moves_after: "4-8" (a random count in that range) or one number; 0 = never moves */
stock box_moves(s)
{
	new v[16];
	ks(s, "moves_after", v, charsmax(v), "4-8");
	new d = contain(v, "-");
	if (d > 0)
	{
		v[d] = EOS;
		return random_num(str_to_num(v), str_to_num(v[d + 1]));
	}
	return str_to_num(v);
}

stock box_move(s)
{
	new grp[32], g2[32];
	box_group_of(s, grp, charsmax(grp));
	new others[32], n = 0;
	for (new o = 0; o < g_nslots && n < sizeof others; o++)
	{
		if (o == s || !g_ent[o] || g_kind[o] != K_BOX || !g_on[o])
			continue;
		box_group_of(o, g2, charsmax(g2));
		if (equal(grp, g2))
			others[n++] = o;
	}
	if (!n || g_pf[s][BX_MOVEAT] <= 0.0)
	{
		g_aux[s] = 0;
		return;
	}
	box_show(s, false);
	box_show(others[random(n)], true);
	toast_add(0, 80, 200, 255, 3.0, "The mystery box moved!", false);
	up_offers_all();
}

/* each group has one box out at a time */
stock box_check_groups()
{
	new grp[32], g2[32];
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || g_kind[s] != K_BOX || !g_on[s])
			continue;
		box_group_of(s, grp, charsmax(grp));
		new any = 0, list[32], n = 0;
		for (new o = 0; o < g_nslots; o++)
		{
			if (!g_ent[o] || g_kind[o] != K_BOX || !g_on[o])
				continue;
			box_group_of(o, g2, charsmax(g2));
			if (!equal(grp, g2))
				continue;
			if (g_state[o] == 1)
				any++;
			if (n < sizeof list)
				list[n++] = o;
		}
		if (!any && n)
			box_show(list[random(n)], true);
	}
}

/* ---- components: perk machine, bench, box, pickup ---- */

stock up_comp_init(s)
{
	new ent = g_ent[s];
	new k = g_kind[s];
	new m[64];
	new bool:trapUse = false;
	if (k == K_TRAP)
	{
		new on[16];
		ks(s, "fire_on", on, charsmax(on));
		trapUse = equali(on, "use") && kf(s, "cost", 0.0) > 0.0;
		if (!trapUse)
			return;
	}
	else if (k != K_PERK && k != K_BENCH && k != K_BOX && k != K_PICKUP)
		return;
	new def[40];
	switch (k)
	{
		case K_PERK: copy(def, charsmax(def), "models/w_medkit.mdl");
		case K_BENCH: copy(def, charsmax(def), "models/w_assault.mdl");
		case K_TRAP: copy(def, charsmax(def), "models/w_battery.mdl");
		case K_BOX: copy(def, charsmax(def), DEFAULT_BOX);
		case K_PICKUP: copy(def, charsmax(def), "models/w_9mmarclip.mdl");
	}
	/* (a trap's model key is its brush area; its switch is the default prop) */
	if (k == K_TRAP || !ks(s, "model", m, charsmax(m)) || m[0] == '*' || !usable_model(m))
		copy(m, charsmax(m), def);
	if (k == K_TRAP && g_box[s])
	{
		/* the area stays the trap's box; the switch shows at its spot */
		new mdl[4];
		pev(ent, pev_model, mdl, charsmax(mdl));
		if (mdl[0] == '*')
			return;
	}
	set_model(ent, m, def);
	engfunc(EngFunc_SetSize, ent, Float:{-8.0, -8.0, 0.0}, Float:{8.0, 8.0, 16.0});
	set_pev(ent, pev_solid, SOLID_NOT);
	new Float:o[3];
	vcopy(g_org[s], o);
	if (!g_box[s])
	{
		/* on the floor under it (from a little above: a spot given at floor height) */
		new Float:a[3], Float:b[3], Float:frac;
		vcopy(o, a);
		vcopy(o, b);
		a[2] += 40.0;
		b[2] -= 256.0;
		engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, ent, 0);
		get_tr2(0, TR_flFraction, frac);
		if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid) && frac < 1.0)
		{
			get_tr2(0, TR_vecEndPos, b);
			o[2] = b[2] + 2.0;
		}
		engfunc(EngFunc_SetOrigin, ent, o);
		if (k != K_TRAP)
			vcopy(o, g_org[s]);
	}
	if (k == K_PICKUP)
	{
		set_pev(ent, pev_movetype, MOVETYPE_NOCLIP);
		set_pev(ent, pev_avelocity, Float:{0.0, 90.0, 0.0});
	}
	else
		set_pev(ent, pev_movetype, MOVETYPE_NONE);
}

stock up_comp_reset(s)
{
	switch (g_kind[s])
	{
		case K_PERK:
		{
			new p[24];
			ks(s, "perk", p, charsmax(p), "tough");
			g_aux[s] = perk_index(p);
			if (g_aux[s] < 0)
			{
				log_amx("A perk machine has an unknown perk (%s); it sells Tough.", p);
				g_aux[s] = PK_TOUGH;
			}
			g_pf[s][PM_PRICE] = kf(s, "price", 1500.0);
			g_pf[s][PM_STEP] = kf(s, "price_step", 1000.0);
			g_pf[s][PM_MAX] = float(ki(s, "max_perks", 4));
			new k = g_aux[s];
			render(g_ent[s], kRenderFxGlowShell, PK_RGB[k][0], PK_RGB[k][1], PK_RGB[k][2], kRenderNormal, 14);
		}
		case K_BENCH:
		{
			g_pf[s][BN_PRICE2] = kf(s, "price2", 5000.0);
			g_pf[s][BN_PRICE3] = kf(s, "price3", 9000.0);
			g_pf[s][BN_L3FROM] = float(ki(s, "level3_from", 12));
			g_pf[s][BN_DMG2] = kf(s, "dmg2", 1.4);
			g_pf[s][BN_DMG3] = kf(s, "dmg3", 1.9);
			render(g_ent[s], kRenderFxGlowShell, 255, 140, 0, kRenderNormal, 14);
		}
		case K_BOX:
		{
			g_pf[s][BX_PRICE] = kf(s, "price", 950.0);
			box_show(s, false);
		}
		case K_PICKUP:
		{
			g_pf[s][PU_UNTIL] = g_now + kf(s, "life", 25.0);
			g_pf[s][PU_RADIUS] = kf(s, "radius", 56.0);
			new Float:c[3];
			if (kvec(s, "glow", c))
				render(g_ent[s], kRenderFxGlowShell, floatround(c[0]), floatround(c[1]), floatround(c[2]), kRenderNormal, 16);
		}
		case K_TRAP:
		{
			if (up_trap_paid(s))
				render(g_ent[s], kRenderFxGlowShell, 80, 160, 255, kRenderNormal, 14);
		}
		case K_SHOP:
		{
			new v[8];
			if (!ks(s, "radius", v, charsmax(v)))
				g_r[s] = 128.0;
			g_state[s] = ki(s, "open", 1) ? 0 : 1;
			keeper_spawn(s);
		}
	}
}

stock bool:up_trap_paid(s)
{
	if (g_kind[s] != K_TRAP)
		return false;
	new on[16];
	ks(s, "fire_on", on, charsmax(on));
	return equali(on, "use") && kf(s, "cost", 0.0) > 0.0;
}

/* the paid trap: switched on, it stays on active_secs, hurting creatures inside hard (and
   players a little), then recharges rearm_secs */
stock bool:up_trap_fired(s)
{
	new Float:act = kf(s, "active_secs", 0.0);
	if (act <= 0.0)
		return false;
	g_state[s] = TRAP_ACTIVE;
	g_pf[s][T_UNTIL] = g_now + act;
	g_pf[s][T_NEXTHIT] = g_now;
	emit_sound(g_ent[s], CHAN_STATIC, "buttons/spark5.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
	return true;
}

stock up_trap_tick(s)
{
	if (g_now >= g_pf[s][T_UNTIL])
	{
		g_state[s] = TRAP_COOLDOWN;
		g_pf[s][T_ARMAT] = g_now + g_pf[s][T_REARM];
		hint_comp(s);
		return;
	}
	if (g_now < g_pf[s][T_NEXTHIT])
		return;
	g_pf[s][T_NEXTHIT] = g_now + 0.5;
	new Float:cd = kf(s, "creature_dps", 80.0) * 0.5, Float:cp = kf(s, "creature_dps_pct", 0.16) * 0.5;
	new Float:pd = kf(s, "player_dps", 4.0) * 0.5;
	new act = g_aux[s];
	new by = (1 <= act <= 32 && is_user_connected(act)) ? act : 0;
	new e = -1, Float:o[3];
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
			continue;
		pev(e, pev_origin, o);
		if (!inside(s, o))
			continue;
		new Float:mh;
		pev(e, pev_max_health, mh);
		ExecuteHamB(Ham_TakeDamage, e, by, by, cd + cp * mh, DMG_SHOCK);
	}
	if (pd > 0.0)
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!g_alive[p] || g_down[p] || !inside(s, g_porg[p]))
				continue;
			new hp = get_user_health(p);
			if (hp > 5)
				set_user_health(p, max(hp - floatround(pd, floatround_ceil), 5));
		}
	/* an arc across the area */
	new Float:mn[3], Float:mx[3];
	if (g_box[s])
	{
		vcopy(g_mins[s], mn);
		vcopy(g_maxs[s], mx);
	}
	else
	{
		for (new i = 0; i < 2; i++)
		{
			mn[i] = g_org[s][i] - g_r[s] * 0.7;
			mx[i] = g_org[s][i] + g_r[s] * 0.7;
		}
		mn[2] = g_floor[s];
	}
	new Float:a[3], Float:b[3];
	a[0] = random_float(mn[0], mx[0]);
	a[1] = random_float(mn[1], mx[1]);
	a[2] = mn[2] + 8.0;
	b[0] = random_float(mn[0], mx[0]);
	b[1] = random_float(mn[1], mx[1]);
	b[2] = mn[2] + 40.0;
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY);
	write_byte(TE_BEAMPOINTS);
	engfunc(EngFunc_WriteCoord, a[0]); engfunc(EngFunc_WriteCoord, a[1]); engfunc(EngFunc_WriteCoord, a[2]);
	engfunc(EngFunc_WriteCoord, b[0]); engfunc(EngFunc_WriteCoord, b[1]); engfunc(EngFunc_WriteCoord, b[2]);
	write_short(g_sprBeam);
	write_byte(0); write_byte(10); write_byte(5); write_byte(30); write_byte(60);
	write_byte(120); write_byte(180); write_byte(255); write_byte(220); write_byte(30);
	message_end();
}

stock bool:trap_buy(s, id, bool:tell)
{
	if (!is_user_alive(id) || g_down[id])
		return false;
	new cost = floatround(kf(s, "cost", 0.0));
	if (g_state[s] != TRAP_ARMED)
	{
		if (tell)
		{
			if (g_state[s] == TRAP_COOLDOWN)
				client_print(id, print_center, "The trap is recharging (%d s)", floatround(g_pf[s][T_ARMAT] - g_now));
			else if (g_state[s] == TRAP_ACTIVE)
				client_print(id, print_center, "The trap is on");
		}
		return false;
	}
	if (cs_get_user_money(id) < cost)
	{
		if (tell)
			client_print(id, print_center, "The trap: not enough money");
		return false;
	}
	cs_set_user_money(id, cs_get_user_money(id) - cost);
	trap_fire(s, id);
	log_amx("Upgrades: %d switched a trap on for $%d in wave %d", id, cost, g_wave);
	return true;
}

/* the usable thing (perk machine, bench, box out, paid trap) within reach, or -1 */
stock up_near(id)
{
	new best = -1, Float:bd = 80.0;
	new team = g_pteam[id];
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s])
			continue;
		new k = g_kind[s];
		if (k == K_BOX)
		{
			if (g_state[s] != 1)
				continue;
		}
		else if (k == K_TRAP)
		{
			if (!up_trap_paid(s))
				continue;
		}
		else if (k != K_PERK && k != K_BENCH)
			continue;
		if (g_team[s] && team != g_team[s])
			continue;
		new Float:o[3];
		pev(g_ent[s], pev_origin, o);
		new Float:d = get_distance_f(g_porg[id], o);
		if (d >= bd)
			continue;
		/* people: roughly looking at it (a box by a shop counter isn't bought by accident) */
		if (!g_bot[id] && d > 32.0)
		{
			new Float:eye[3], Float:ang[3], Float:fwd[3];
			pev(id, pev_view_ofs, eye);
			for (new i = 0; i < 3; i++)
				eye[i] += g_porg[id][i];
			pev(id, pev_v_angle, ang);
			engfunc(EngFunc_MakeVectors, ang);
			global_get(glb_v_forward, fwd);
			o[2] += 8.0;
			new Float:de = get_distance_f(eye, o);
			if (de > 1.0 && ((o[0] - eye[0]) * fwd[0] + (o[1] - eye[1]) * fwd[1] + (o[2] - eye[2]) * fwd[2]) / de < 0.6)
				continue;
		}
		bd = d;
		best = s;
	}
	return best;
}

stock up_use(s, id, bool:tell)
{
	switch (g_kind[s])
	{
		case K_PERK: return perk_buy(s, id, g_aux[s], tell);
		case K_BENCH: return bench_use(s, id, tell);
		case K_BOX: return box_use(s, id, tell);
		case K_TRAP: return trap_buy(s, id, tell);
	}
	return false;
}

stock up_hint(s, id)
{
	new why[48];
	switch (g_kind[s])
	{
		case K_PERK:
		{
			new k = g_aux[s];
			perk_why_not(s, id, k, why, charsmax(why));
			hud_add(id, false, "%s perk: %s - $%d  %s", PK_NAME[k], PK_WHAT[k], perk_price_at(s, id), why[0] ? why : "(USE to buy)");
		}
		case K_BENCH:
		{
			new w = bench_why_not(s, id, why, charsmax(why));
			new lv = w ? wpn_level(w) : 1;
			hud_add(id, false, "Upgrade bench: Mk %s - $%d  %s", lv >= 2 ? "III" : "II", bench_price(s, lv), why[0] ? why : "(USE to upgrade the gun in hand)");
		}
		case K_BOX:
			hud_add(id, false, "Mystery box - $%d for a random gun  (USE)", floatround(g_pf[s][BX_PRICE]));
		case K_TRAP:
		{
			new name[32];
			ks(s, "name", name, charsmax(name), "Electric trap");
			if (g_state[s] == TRAP_ACTIVE)
				hud_add(id, false, "%s - on", name);
			else if (g_state[s] == TRAP_COOLDOWN)
				hud_add(id, false, "%s - recharging (%d s)", name, floatround(floatmax(g_pf[s][T_ARMAT] - g_now, 0.0)));
			else
				hud_add(id, false, "%s - $%d for %d s  (USE to switch on)", name, floatround(kf(s, "cost", 0.0)), floatround(kf(s, "active_secs", 0.0)));
		}
	}
}

/* USE on a machine, the bench, the box or a paid trap: people press it, bots hold it */
public up_prethink(id)
{
	if (!is_user_alive(id) || g_down[id])
		return FMRES_IGNORED;
	new b = pev(id, pev_button);
	if (!(b & IN_USE))
		return FMRES_IGNORED;
	new bool:bot = is_user_bot(id) != 0;
	if (!bot && (pev(id, pev_oldbuttons) & IN_USE))
		return FMRES_IGNORED;
	new Float:now = get_gametime();
	if (now < g_upUseNext[id])
		return FMRES_IGNORED;
	g_now = now;
	g_bot[id] = bot;
	pev(id, pev_origin, g_porg[id]);
	g_pteam[id] = get_user_team(id);
	new s = up_near(id);
	if (s < 0)
	{
		/* a turret kit: placed where they look; a bot with a turret offer: bought and placed */
		if (!bot && g_turKits[id] > 0 && !g_shopIn[id])
		{
			g_upUseNext[id] = now + 1.0;
			turret_place(id, false);
			return FMRES_IGNORED;
		}
		if (bot && g_turOffered[id] && g_turPrice > 0 && cs_get_user_money(id) >= g_turPrice
			&& get_distance_f(g_porg[id], g_turOfferOrg[id]) < 450.0 && turret_count(id) < g_turPer)
		{
			g_upUseNext[id] = now + 1.0;
			if (turret_place(id, true))
				up_offers(id);
			return FMRES_IGNORED;
		}
		/* in a shop: its menu */
		if (!bot && shop_use(id))
			g_upUseNext[id] = now + 1.0;
		return FMRES_IGNORED;
	}
	g_upUseNext[id] = now + 1.0;
	up_use(s, id, !bot);
	if (bot)
		up_offers(id);
	return FMRES_IGNORED;
}

/* ---- pickups: a mode drops one (ls_pickup_spawn); a player walking over takes it ---- */

stock up_pickup_tick(s)
{
	new e = g_ent[s];
	new Float:left = g_pf[s][PU_UNTIL] - g_now;
	if (left <= 0.0)
	{
		comp_remove(s);
		return;
	}
	/* blinks its last 5 s */
	if (left < 5.0)
		set_pev(e, pev_effects, (floatround(g_now * 4.0) % 2) ? (pev(e, pev_effects) | EF_NODRAW) : (pev(e, pev_effects) & ~EF_NODRAW));
	new Float:o[3];
	pev(e, pev_origin, o);
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_down[p] || (g_team[s] && g_pteam[p] != g_team[s]))
			continue;
		if (get_distance_f(o, g_porg[p]) > g_pf[s][PU_RADIUS])
			continue;
		new kind[32], ret;
		ks(s, "kind", kind, charsmax(kind));
		ExecuteForward(g_fwPickup, ret, e, p, kind);
		if (ret == PLUGIN_HANDLED)
			continue;
		emit_sound(p, CHAN_ITEM, "items/ammopickup1.wav", 1.0, ATTN_NORM, 0, PITCH_NORM);
		comp_remove(s);
		return;
	}
}

/* ---- the downed state ---- */

stock bool:down_team_ok(id)
{
	return !g_dnTeam || get_user_team(id) == g_dnTeam;
}

public up_player_hurt(victim, inflictor, attacker, Float:dmg, bits)
{
	if (!is_user_alive(victim) || GetHamReturnStatus() >= HAM_SUPERCEDE)
		return HAM_IGNORED;
	/* (downed: nothing more hurts them; they bleed out on the clock) */
	if (g_down[victim])
		return HAM_SUPERCEDE;
	/* the attacker's damage modifier and gun level, against players too */
	if (1 <= attacker <= 32 && attacker != victim && is_user_alive(attacker))
	{
		new Float:m = mod_get(attacker, MOD_DAMAGE) * wpn_mult(active_weapon(attacker));
		if (m != 1.0 && inflictor == attacker)
		{
			dmg *= m;
			SetHamParamFloat(4, dmg);
		}
	}
	if (!g_dnOn || !down_team_ok(victim) || dmg < float(get_user_health(victim)))
		return HAM_IGNORED;
	/* a teammate's hit with friendly fire off does nothing (the game drops it after us) */
	if (1 <= attacker <= 32 && attacker != victim && is_user_connected(attacker) && get_user_team(attacker) == get_user_team(victim) && !get_cvar_num("mp_friendlyfire"))
		return HAM_IGNORED;
	new ret;
	ExecuteForward(g_fwDown, ret, victim, attacker);
	if (ret == PLUGIN_HANDLED)
		return HAM_IGNORED;
	go_down(victim);
	return HAM_SUPERCEDE;
}

stock go_down(victim)
{
	new bool:selfrev = g_selfRev[victim];
	if (g_dnLosePerks)
		perk_clear(victim);
	set_user_health(victim, 1);
	g_down[victim] = true;
	g_bledOut[victim] = false;
	g_downAt[victim] = get_gametime();
	pev(victim, pev_origin, g_downOrg[victim]);
	g_revBy[victim] = 0;
	g_revProg[victim] = 0.0;
	g_selfRevAt[victim] = 0.0;
	/* alone (no teammate up) with Quick revive: back up in 4 s, once */
	if (selfrev && !other_up(victim))
	{
		g_selfRevAt[victim] = get_gametime() + 4.0;
		g_selfRev[victim] = false;
	}
	set_pev(victim, pev_flags, pev(victim, pev_flags) | FL_NOTARGET);
	ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, victim);
	if (!is_user_bot(victim))
		client_cmd(victim, "+duck");
	render(victim, kRenderFxGlowShell, 255, 40, 40, kRenderNormal, 12);
	act_cancel(victim);
	send_revive(victim);
}

stock bool:other_up(id)
{
	new t = get_user_team(id);
	for (new p = 1; p <= MaxClients; p++)
		if (p != id && is_user_alive(p) && get_user_team(p) == t && w3_friends(p, id) && !g_down[p]) /* W3: squad mates only */
			return true;
	return false;
}

/* the two nearest bots of the team go and hold USE on them (W2: kind use, prio 75) */
stock send_revive(victim)
{
	new t = get_user_team(victim);
	new near[2], Float:nd[2] = { 99999.0, 99999.0 };
	for (new p = 1; p <= MaxClients; p++)
	{
		if (p == victim || !is_user_alive(p) || !is_user_bot(p) || g_down[p] || get_user_team(p) != t || !w3_friends(p, victim)) /* W3: squad mates only */
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
		new left = max(floatround(g_dnBleed - (get_gametime() - g_downAt[victim])), 1);
		hint("goal %d use %.0f %.0f %.0f %d 75 %d 64", near[i], g_downOrg[victim][0], g_downOrg[victim][1], g_downOrg[victim][2], victim, left);
	}
}

stock down_clear(p)
{
	if (g_down[p])
		hint("goal_clear all %d", p);
	if (g_revBy[p] && is_user_connected(g_revBy[p]))
		bar_time(g_revBy[p], 0);
	g_down[p] = false;
	g_revBy[p] = 0;
	g_revProg[p] = 0.0;
	g_selfRevAt[p] = 0.0;
}

stock get_up(d, by)
{
	down_clear(d);
	if (!is_user_alive(d))
		return;
	set_pev(d, pev_flags, pev(d, pev_flags) & ~FL_NOTARGET);
	render(d, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
	set_user_health(d, 50);
	ExecuteHamB(Ham_CS_Player_ResetMaxSpeed, d);
	if (!is_user_bot(d))
		client_cmd(d, "-duck");
	new ret;
	ExecuteForward(g_fwRevive, ret, d, by);
	if (by && by != d)
		log_amx("Upgrades: %d revived %d in wave %d", by, d, g_wave);
}

stock down_tick()
{
	new Float:now = get_gametime();
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!g_down[d])
			continue;
		if (!is_user_connected(d) || !is_user_alive(d))
		{
			down_clear(d);
			continue;
		}
		if (g_selfRevAt[d] > 0.0 && now >= g_selfRevAt[d])
		{
			get_up(d, d);
			continue;
		}
		if (now - g_downAt[d] > g_dnBleed)
		{
			g_bledOut[d] = true;
			down_clear(d);
			new ret;
			ExecuteForward(g_fwBledOut, ret, d);
			set_pev(d, pev_flags, pev(d, pev_flags) & ~FL_NOTARGET);
			render(d, kRenderFxNone, 0, 0, 0, kRenderNormal, 255);
			user_kill(d, 1);
			continue;
		}
		/* a teammate close: people hold USE, bots just stay */
		new who = 0, Float:dO[3], t = get_user_team(d);
		pev(d, pev_origin, dO);
		for (new p = 1; p <= MaxClients; p++)
		{
			if (p == d || !is_user_alive(p) || g_down[p] || get_user_team(p) != t || !w3_friends(p, d)) /* W3: squad mates only */
				continue;
			new Float:o[3];
			pev(p, pev_origin, o);
			if (get_distance_f(o, dO) > 80.0)
				continue;
			if (!(pev(p, pev_button) & IN_USE) && !is_user_bot(p))
				continue;
			who = p;
			break;
		}
		new Float:need = 4.0 * (who ? mod_get(who, MOD_REVIVE) : 1.0);
		if (who != g_revBy[d])
		{
			if (g_revBy[d] && is_user_connected(g_revBy[d]))
				bar_time(g_revBy[d], 0);
			g_revBy[d] = who;
			g_revProg[d] = 0.0;
			if (who)
				bar_time(who, floatround(need, floatround_ceil));
		}
		if (!who)
			continue;
		g_revProg[d] += TICK;
		if (g_revProg[d] >= need)
			get_up(d, who);
	}
}

public up_player_killed(id, killer, gib)
{
	if (g_down[id])
		down_clear(id);
	set_pev(id, pev_flags, pev(id, pev_flags) & ~FL_NOTARGET);
	g_perks[id] = 0;
	g_nperks[id] = 0;
	g_selfRev[id] = false;
	for (new m = 0; m < MOD_COUNT; m++)
		g_modSet[id][m] = 1.0;
	return HAM_IGNORED;
}

stock up_disconnected(id)
{
	g_turKits[id] = 0;
	g_turOffered[id] = false;
	g_shopIn[id] = 0;
	g_shopVisits[id] = 0;
	g_shopMenuUntil[id] = 0.0;
	remove_task(TASK_MENU + 400 + id);
	g_down[id] = false;
	g_bledOut[id] = false;
	g_perks[id] = 0;
	g_nperks[id] = 0;
	g_selfRev[id] = false;
	for (new m = 0; m < MOD_COUNT; m++)
		g_modSet[id][m] = 1.0;
}

/* ---- creatures: the damage modifier and gun level, the last hit's place, the forward ---- */

public up_creature_hurt(ent, inflictor, attacker, Float:dmg, bits)
{
	if (!(1 <= attacker <= 32) || !is_user_alive(attacker) || inflictor != attacker)
		return HAM_IGNORED;
	new Float:m = mod_get(attacker, MOD_DAMAGE) * wpn_mult(active_weapon(attacker));
	if (m != 1.0)
		SetHamParamFloat(4, dmg * m);
	return HAM_IGNORED;
}

public up_creature_hurt_post(ent, inflictor, attacker, Float:dmg, bits)
{
	if (dmg <= 0.0)
		return HAM_IGNORED;
	new ret;
	ExecuteForward(g_fwCreatureHurt, ret, ent, attacker, dmg, (ent < MAX_ENTS && g_headLast[ent]) ? 1 : 0);
	return HAM_IGNORED;
}

public up_creature_trace(ent, attacker, Float:dmg, Float:dir[3], tr, bits)
{
	if (ent < MAX_ENTS)
		g_headLast[ent] = get_tr2(tr, TR_iHitgroup) == HIT_HEAD;
	return HAM_IGNORED;
}

/* ---- bots: W2's offers (ls_mode offer <bot> <kind> x y z ent cost [order]) ---- */

stock up_offers_all()
{
	g_offerAt = 0.0;
}

stock up_offers(p)
{
	if (!g_brain || !is_user_bot(p))
		return;
	hint("offer_clear %d", p);
	if (!is_user_alive(p) || g_down[p])
		return;
	new team = get_user_team(p);
	g_turOffered[p] = false;
	if (g_turPrice > 0 && g_turBots && turret_count(p) < g_turPer)
	{
		/* where: a gate (barricade), a different one per bot */
		new gates[16], ng = 0;
		for (new b = 0; b < g_nslots && ng < sizeof gates; b++)
			if (g_ent[b] && g_kind[b] == K_BARRICADE && (!g_team[b] || g_team[b] == team))
				gates[ng++] = b;
		if (ng)
		{
			new b = gates[p % ng];
			for (new i = 0; i < 3; i++)
				g_turOfferOrg[p][i] = (g_mins[b][i] + g_maxs[b][i]) * 0.5;
			g_turOffered[p] = true;
			hint("offer %d turret %.0f %.0f %.0f 0 %d", p, g_turOfferOrg[p][0], g_turOfferOrg[p][1], g_turOfferOrg[p][2], g_turPrice);
		}
	}
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s] || !ki(s, "bots", 1) || (g_team[s] && team != g_team[s]))
			continue;
		new Float:o[3];
		pev(g_ent[s], pev_origin, o);
		switch (g_kind[s])
		{
			case K_PERK:
			{
				new k = g_aux[s];
				new mx = floatround(g_pf[s][PM_MAX]);
				if (perk_has(p, k) || (mx > 0 && g_nperks[p] >= mx))
					continue;
				hint("offer %d perk %.0f %.0f %.0f %d %d %d", p, o[0], o[1], o[2], g_ent[s], perk_price_at(s, p), PK_ORDER[k]);
			}
			case K_BENCH:
			{
				new why[48];
				new w = bench_why_not(s, p, why, charsmax(why));
				if (!w || (why[0] && !equal(why, "not enough money")))
					continue;
				hint("offer %d bench %.0f %.0f %.0f %d %d", p, o[0], o[1], o[2], g_ent[s], bench_price(s, wpn_level(w)));
			}
			case K_BOX:
			{
				if (g_state[s] != 1)
					continue;
				hint("offer %d box %.0f %.0f %.0f %d %d", p, o[0], o[1], o[2], g_ent[s], floatround(g_pf[s][BX_PRICE]));
			}
			case K_TRAP:
			{
				if (!up_trap_paid(s))
					continue;
				/* x y z = the gate it covers (the nearest barricade) */
				new Float:gc[3], Float:bd = 99999.0;
				vcopy(g_org[s], gc);
				for (new b = 0; b < g_nslots; b++)
				{
					if (!g_ent[b] || g_kind[b] != K_BARRICADE)
						continue;
					new Float:c[3];
					for (new i = 0; i < 3; i++)
						c[i] = (g_mins[b][i] + g_maxs[b][i]) * 0.5;
					new Float:d = get_distance_f(c, g_org[s]);
					if (d < bd)
					{
						bd = d;
						vcopy(c, gc);
					}
				}
				hint("offer %d trap %.0f %.0f %.0f %d %d", p, gc[0], gc[1], gc[2], g_ent[s], floatround(kf(s, "cost", 0.0)));
			}
		}
	}
}

/* bots switch a paid trap on when 4+ creatures are near it (bots_auto=0: only by the brain's offers) */
stock up_bots_traps()
{
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s] || g_kind[s] != K_TRAP || g_state[s] != TRAP_ARMED || !up_trap_paid(s) || !ki(s, "bots_auto", 1))
			continue;
		new n = 0, e = -1, Float:o[3];
		while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
		{
			if (pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
				continue;
			pev(e, pev_origin, o);
			if (get_distance_f(o, g_org[s]) < 300.0)
				n++;
		}
		if (n < 4)
			continue;
		new cost = floatround(kf(s, "cost", 0.0));
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!g_alive[p] || !g_bot[p] || g_down[p] || (g_team[s] && g_pteam[p] != g_team[s]) || cs_get_user_money(p) < cost + 500)
				continue;
			if (get_distance_f(g_porg[p], g_org[s]) > 700.0)
				continue;
			trap_buy(s, p, false);
			break;
		}
	}
}

/* every tick (0.25 s) */
stock up_tick(bool:sec)
{
	down_tick();
	new bool:any = false, bool:shops = false;
	for (new s = 0; s < g_nslots; s++)
	{
		if (!g_ent[s] || !g_on[s])
			continue;
		switch (g_kind[s])
		{
			case K_PICKUP: up_pickup_tick(s);
			case K_PERK, K_BENCH, K_BOX: any = true;
			case K_SHOP:
			{
				keeper_tick(s);
				shops = true;
			}
			case K_TRAP: if (up_trap_paid(s)) any = true;
		}
	}
	if (shops)
		shop_people();
	if (sec)
		turret_tick();
	for (new p = 1; p <= MaxClients; p++)
		if (g_turKits[p] > 0 && g_alive[p] && !g_bot[p])
			hud_add(p, false, "Turret kit: press USE where you want it");
	if (!any && g_turPrice <= 0)
		return;
	if (!any)
	{
		if (g_now >= g_offerAt && g_brain)
		{
			g_offerAt = g_now + 8.0;
			for (new p = 1; p <= MaxClients; p++)
				if (g_alive[p] && g_bot[p])
					up_offers(p);
		}
		return;
	}
	if (sec)
	{
		box_check_groups();
		up_bots_traps();
	}
	/* hints for people close to one */
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p] || g_down[p])
			continue;
		new s = up_near(p);
		if (s >= 0)
			up_hint(s, p);
	}
	if (g_now >= g_offerAt && g_brain)
	{
		g_offerAt = g_now + 8.0;
		for (new p = 1; p <= MaxClients; p++)
			if (g_alive[p] && g_bot[p])
				up_offers(p);
	}
}

/* ---- the shop (ls_shop): a counter with a menu of priced items, maybe a keeper ----
   items = "Name|price|give|what; ..." (give: an item ls_give_item knows - ammo, vesthelm,
   hp:50, hegrenade, weapon names - or @word for the mode to give in ls_on_shop_buy);
   price_rise = dearer by this share each wave; scale; upgrades=1 adds the perks and the bench
   where the map has no machine or bench; buy_menu=1 adds the game's buy menu; keeper = barney,
   gman, none or a model; title, hint (said on walking in), greet (the keeper's first words) */

stock shop_precache()
{
	/* Last Stand's keeper: its model has to be loaded with the map (lst_shopkeeper) */
	read_mode_id();
	if (!equal(g_modeId, "laststand"))
		return;
	new want[64] = "barney";
	if (get_cvar_pointer("lst_shopkeeper"))
		get_cvar_string("lst_shopkeeper", want, charsmax(want));
	if (equali(want, "none"))
		return;
	new m[96];
	keeper_model(want, m, charsmax(m));
	new alt[128];
	formatex(alt, charsmax(alt), "../valve/%s", m);
	if (file_exists(m) || file_exists(alt))
		precache_model_once(m);
}

stock keeper_model(const want[], out[], len)
{
	if (equali(want, "barney") || equali(want, "gman"))
		formatex(out, len, "models/%s.mdl", want);
	else
		copy(out, len, want);
}

/* item i of the shop: false past the last */
stock bool:shop_item(s, i, name[], nlen, &price, give[], glen, what[], wlen)
{
	new list[512];
	ks(s, "items", list, charsmax(list));
	new at = 0, n = 0;
	new len = strlen(list);
	while (at < len)
	{
		new end = at;
		while (end < len && list[end] != ';')
			end++;
		if (n == i)
		{
			new entry[192];
			copy(entry, min(end - at, charsmax(entry)), list[at]);
			new f[4][96], fi = 0, fo = 0;
			for (new c = 0; entry[c] && fi < 4; c++)
			{
				if (entry[c] == '|')
				{
					f[fi][fo] = EOS;
					fi++;
					fo = 0;
					continue;
				}
				if (fo < 95)
					f[fi][fo++] = entry[c];
			}
			if (fi < 4)
				f[fi][fo] = EOS;
			for (new k = 0; k < 4; k++)
				trim(f[k]);
			copy(name, nlen, f[0]);
			price = str_to_num(f[1]);
			copy(give, glen, f[2][0] ? f[2] : f[0]);
			copy(what, wlen, f[3]);
			return name[0] != EOS;
		}
		n++;
		at = end + 1;
	}
	return false;
}

/* the price now: dearer each wave (price_rise), times scale, rounded to $50 */
stock shop_price_now(s, base)
{
	new Float:p = float(base) * (1.0 + kf(s, "price_rise", 0.0) * float(max(g_wave - 1, 0))) * kf(s, "scale", 1.0);
	return max(floatround(p / 50.0) * 50, base > 0 ? 50 : 0);
}

/* the item whose name or give is `item`: its index, or -1 */
stock shop_find(s, const item[])
{
	new name[64], give[64], what[96], price;
	for (new i = 0; i < SH_MAX_ITEMS && shop_item(s, i, name, charsmax(name), price, give, charsmax(give), what, charsmax(what)); i++)
		if (equali(name, item) || equali(give, item))
			return i;
	return -1;
}

/* why it can't be bought now ("" = it can) */
stock shop_why_not(s, id, const give[], price, out[], len)
{
	out[0] = EOS;
	if (equali(give, "vesthelm") || equali(give, "assault") || equali(give, "armor", 5) || equali(give, "vest") || equali(give, "kevlar"))
	{
		new CsArmorType:t;
		if (cs_get_user_armor(id, t) >= 100 && (t == CS_ARMOR_VESTHELM || !equali(give, "vesthelm")))
			copy(out, len, "you have full armor");
	}
	else if (equali(give, "hp", 2) || equali(give, "medkit"))
	{
		if (float(get_user_health(id)) >= floatmax(entity_max_health(id), 100.0))
			copy(out, len, "you're at full health");
	}
	if (!out[0])
	{
		g_shopWhy[0] = EOS;
		new ret;
		ExecuteForward(g_fwShopItem, ret, g_ent[s], id, give);
		if (ret == PLUGIN_HANDLED)
			copy(out, len, g_shopWhy[0] ? g_shopWhy : "not now");
	}
	if (!out[0] && cs_get_user_money(id) < price)
		copy(out, len, "not enough money");
}

stock bool:shop_buy_item(s, id, i, bool:tell)
{
	new name[64], give[64], what[96], base;
	if (!is_user_alive(id) || g_down[id] || !shop_item(s, i, name, charsmax(name), base, give, charsmax(give), what, charsmax(what)))
		return false;
	if (g_state[s] != 0)
	{
		if (tell)
			client_print(id, print_center, "The %s is closed", g_label[s][0] ? g_label[s] : "shop");
		return false;
	}
	new price = shop_price_now(s, base);
	new why[64];
	shop_why_not(s, id, give, price, why, charsmax(why));
	if (why[0])
	{
		if (tell)
			client_print(id, print_center, "%s: %s", name, why);
		return false;
	}
	new ret;
	ExecuteForward(g_fwShopBuy, ret, g_ent[s], id, give, price);
	if (ret == PLUGIN_HANDLED)
		return false;
	if (equali(give, "turret"))
	{
		if (turret_count(id) + g_turKits[id] >= g_turPer)
		{
			if (tell)
				client_print(id, print_center, "%s: you have %d already", name, g_turPer);
			return false;
		}
	}
	else if (give[0] != '@' && !give_thing(id, give))
	{
		if (tell)
			client_print(id, print_center, "%s: you can't carry it", name);
		return false;
	}
	if (equali(give, "turret"))
	{
		g_turKits[id]++;
		if (!g_bot[id])
			client_print(id, print_center, "Turret kit: press USE where you want it");
	}
	cs_set_user_money(id, cs_get_user_money(id) - price);
	log_amx("Shop: %d bought %s for $%d in wave %d", id, name, price, g_wave);
	if (tell)
	{
		client_print(id, print_center, "%s bought", name);
		keeper_thanks(s, id);
	}
	return true;
}

public shop_item_cb(id, menu, item)
{
	new info[8], name[2], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	return info[0] == 'x' ? ITEM_DISABLED : ITEM_IGNORE;
}

stock shop_menu(s, id)
{
	if (!g_shopCb)
		g_shopCb = menu_makecallback("shop_item_cb");
	new title[96], label[32];
	ks(s, "title", label, charsmax(label), g_label[s][0] ? g_label[s] : "Shop");
	formatex(title, charsmax(title), "\y%s  \d(you have $%d)", label, cs_get_user_money(id));
	new m = menu_create(title, "menu_shop_pick");
	new text[160], info[8], why[64], name[64], give[64], what[96], base;
	for (new i = 0; i < SH_MAX_ITEMS && shop_item(s, i, name, charsmax(name), base, give, charsmax(give), what, charsmax(what)); i++)
	{
		new price = shop_price_now(s, base);
		shop_why_not(s, id, give, price, why, charsmax(why));
		if (why[0])
			formatex(text, charsmax(text), "\d%s%s%s ($%d)  \r%s", name, what[0] ? " - " : "", what, price, why);
		else
			formatex(text, charsmax(text), "%s%s%s%s \y($%d)", name, what[0] ? " \d- " : "", what, what[0] ? "" : "", price);
		formatex(info, charsmax(info), "%s%d", why[0] ? "x" : "", i);
		menu_additem(m, text, info, 0, g_shopCb);
	}
	/* no perk machine / bench on the map: they're here */
	if (ki(s, "upgrades", 0))
	{
		new bool:machine = count_comps(K_PERK) > 0, bool:bench = count_comps(K_BENCH) > 0;
		if (!machine)
			for (new k = 0; k < PK_COUNT; k++)
			{
				perk_why_not(-1, id, k, why, charsmax(why));
				formatex(text, charsmax(text), "%sPerk: %s \d- %s \y($%d)%s%s", why[0] ? "\d" : "", PK_NAME[k], PK_WHAT[k], perk_price_at(-1, id), why[0] ? "  \r" : "", why);
				formatex(info, charsmax(info), "%s%d", why[0] ? "x" : "", 100 + k);
				menu_additem(m, text, info, 0, g_shopCb);
			}
		if (!bench)
		{
			new w = bench_why_not(-1, id, why, charsmax(why));
			new lv = w ? wpn_level(w) : 1;
			formatex(text, charsmax(text), "%sUpgrade the gun in hand to Mk %s \y($%d)%s%s", why[0] ? "\d" : "", lv >= 2 ? "III" : "II", bench_price(-1, lv), why[0] ? "  \r" : "", why);
			formatex(info, charsmax(info), "%s200", why[0] ? "x" : "");
			menu_additem(m, text, info, 0, g_shopCb);
		}
	}
	if (ki(s, "buy_menu", 0))
		menu_additem(m, "Open the gun menu \d(B)", "99");
	g_shopMenuUntil[id] = get_gametime() + 30.0;
	g_shopMenuSlot[id] = s + 1;
	menu_display(id, m);
}

public menu_shop_pick(id, menu, item)
{
	if (item < 0)
	{
		g_shopMenuUntil[id] = 0.0;
		menu_destroy(menu);
		return PLUGIN_HANDLED;
	}
	new info[8], name[2], access, cb;
	menu_item_getinfo(menu, item, access, info, charsmax(info), name, charsmax(name), cb);
	menu_destroy(menu);
	g_shopMenuUntil[id] = 0.0;
	new s = g_shopMenuSlot[id] - 1;
	if (s < 0 || !g_ent[s] || g_kind[s] != K_SHOP || info[0] == 'x')
		return PLUGIN_HANDLED;
	new code = str_to_num(info);
	if (code == 99)
	{
		/* the game's own buy menu (works where it's a buy zone) */
		engclient_cmd(id, "buy");
		return PLUGIN_HANDLED;
	}
	g_bot[id] = is_user_bot(id) != 0;
	if (!is_user_alive(id) || !inside(s, g_porg[id]))
		return PLUGIN_HANDLED;
	if (code >= 100 && code < 100 + PK_COUNT)
	{
		if (perk_buy(-1, id, code - 100, true))
			keeper_thanks(s, id);
	}
	else if (code == 200)
	{
		if (bench_use(-1, id, true))
			keeper_thanks(s, id);
	}
	else
		shop_buy_item(s, id, code, true);
	return PLUGIN_HANDLED;
}

/* walking in: the hint, the keeper's greeting; walking out: menus shut */
stock shop_people()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p] || g_bot[p])
		{
			g_shopIn[p] = 0;
			continue;
		}
		new in = 0;
		for (new s = 0; s < g_nslots && !in; s++)
			if (g_ent[s] && g_on[s] && g_kind[s] == K_SHOP && (!g_team[s] || g_pteam[p] == g_team[s]) && inside(s, g_porg[p]))
				in = s + 1;
		if (in && in != g_shopIn[p])
		{
			new s = in - 1, hintText[128];
			if (ks(s, "hint", hintText, charsmax(hintText)))
				client_print(p, print_center, "%s", hintText);
			g_shopVisits[p]++;
			if (g_shopVisits[p] <= 3 && g_aux[s] > 0 && pev_valid(g_aux[s]))
				set_task(1.5, "task_keeper_greet", TASK_MENU + 400 + p);
		}
		else if (!in && g_shopIn[p])
			shop_leave(p);
		g_shopIn[p] = in;
	}
}

stock shop_leave(p)
{
	if (g_shopMenuUntil[p] > 0.0)
	{
		g_shopMenuUntil[p] = 0.0;
		menu_cancel(p);
		new menu, keys;
		get_user_menu(p, menu, keys);
		if (menu != 0)
			show_menu(p, 0, "^n", 1);
	}
	/* the game's buy menu (VGUI on the Steam client): its own "close" */
	client_cmd(p, "slot10");
}

public task_keeper_greet(tid)
{
	new p = tid - TASK_MENU - 400;
	new s = g_shopIn[p] - 1;
	if (!is_user_connected(p) || s < 0 || !g_ent[s] || g_kind[s] != K_SHOP)
		return;
	new greet[160], who[32];
	keeper_name(s, who, charsmax(who));
	if (!ks(s, "greet", greet, charsmax(greet)) || !who[0])
		return;
	client_print_color(p, print_team_default, "^4%s:^1 %s", who, greet);
}

stock keeper_name(s, out[], len)
{
	new k = floatround(g_pf[s][SH_KIND]);
	ks(s, "keeper_name", out, len, k == 1 ? "Barney" : (k == 2 ? "G-Man" : ""));
}

/* the keeper: Barney or the G-Man (Half-Life's models) or any model, by the counter */
stock keeper_spawn(s)
{
	new want[64];
	ks(s, "keeper", want, charsmax(want), "none");
	new e = g_aux[s];
	if (equali(want, "none"))
	{
		if (e > 0 && pev_valid(e))
			engfunc(EngFunc_RemoveEntity, e);
		g_aux[s] = 0;
		return;
	}
	new m[96];
	keeper_model(want, m, charsmax(m));
	new kind = equali(want, "barney") ? 1 : (equali(want, "gman") ? 2 : 0);
	if (!TrieKeyExists(g_models, m) && !usable_model(m))
	{
		/* (not loaded with the map: a CS player model stands in, no gestures) */
		copy(m, charsmax(m), "models/player/gign/gign.mdl");
		kind = 0;
	}
	g_pf[s][SH_KIND] = float(kind);
	if (e <= 0 || !pev_valid(e))
	{
		e = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"));
		if (!e)
			return;
		set_pev(e, pev_classname, "ls_shopkeeper");
		g_aux[s] = e;
	}
	engfunc(EngFunc_SetModel, e, m);
	set_pev(e, pev_solid, SOLID_NOT);
	new Float:o[3];
	keeper_spot(s, e, o);
	engfunc(EngFunc_SetOrigin, e, o);
	engfunc(EngFunc_SetSize, e, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 72.0});
	set_pev(e, pev_movetype, MOVETYPE_NONE);
	set_pev(e, pev_takedamage, DAMAGE_NO);
	set_pev(e, pev_controller_0, 127);
	set_pev(e, pev_controller_1, 127);
	g_pf[s][SH_SOLID] = 0.0;
	keeper_play(e, kind == 2 ? "idle01" : "idle1");
	g_pf[s][SH_NEXT] = g_now + random_float(6.0, 12.0);
	g_pf[s][SH_BACK] = 0.0;
}

/* by the shop, on the floor (a spot on a counter puts the keeper beside it), with room to stand */
stock keeper_spot(s, e, Float:out[3])
{
	new Float:c0[3];
	vcopy(g_org[s], c0);
	if (!g_box[s])
		c0[2] = g_floor[s] + 36.0;
	new Float:base = quiet_floor(c0, e);
	vcopy(c0, out);
	out[2] = base + 1.0;
	new Float:best = 99999.0, Float:lowest = base;
	for (new ring = 0; ring <= 3; ring++)
	{
		for (new k = 0; k < (ring ? 8 : 1); k++)
		{
			new Float:p[3];
			p[0] = c0[0] + 40.0 * float(ring) * floatcos(45.0 * float(k), degrees);
			p[1] = c0[1] + 40.0 * float(ring) * floatsin(45.0 * float(k), degrees);
			p[2] = c0[2];
			new Float:f = quiet_floor(p, e);
			new Float:c[3];
			vcopy(p, c);
			c[2] = f + 37.0;
			engfunc(EngFunc_TraceHull, c, c, IGNORE_MONSTERS, HULL_HUMAN, e, 0);
			if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
				continue;
			/* the lower floor first (the ground, not a counter), then closer */
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

/* the floor under a point (no warning when it starts in a wall: the keeper's search tries those) */
stock Float:quiet_floor(const Float:p[3], ignore)
{
	new Float:a[3], Float:b[3], Float:frac;
	vcopy(p, a);
	vcopy(p, b);
	a[2] += 8.0;
	b[2] -= 4096.0;
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, ignore, 0);
	get_tr2(0, TR_flFraction, frac);
	if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid) || frac >= 1.0)
		return p[2] - 36.0;
	get_tr2(0, TR_vecEndPos, b);
	return b[2];
}

stock keeper_play(e, const seq[])
{
	new Float:fr, bool:loops;
	new n = lookup_sequence(e, seq, fr, loops);
	set_pev(e, pev_sequence, n >= 0 ? n : 0);
	set_pev(e, pev_frame, 0.0);
	set_pev(e, pev_framerate, 1.0);
	set_pev(e, pev_animtime, get_gametime());
}

/* faces whoever's nearest within 300 units, fidgets now and then, solid once nobody's in him */
stock keeper_tick(s)
{
	new e = g_aux[s];
	if (e <= 0 || !pev_valid(e))
		return;
	new kind = floatround(g_pf[s][SH_KIND]);
	new Float:o[3];
	pev(e, pev_origin, o);
	new near = 0, Float:nd = 300.0, bool:overlap = false;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!g_alive[p])
			continue;
		new Float:po[3];
		vcopy(g_porg[p], po);
		if (floatabs(po[0] - o[0]) < 34.0 && floatabs(po[1] - o[1]) < 34.0 && po[2] - o[2] < 110.0 && po[2] - o[2] > -40.0)
			overlap = true;
		new Float:d = get_distance_f(o, po);
		if (d < nd)
		{
			nd = d;
			near = p;
		}
	}
	if (g_pf[s][SH_SOLID] == 0.0 && !overlap)
	{
		g_pf[s][SH_SOLID] = 1.0;
		set_pev(e, pev_solid, SOLID_BBOX);
		engfunc(EngFunc_SetOrigin, e, o);
	}
	if (near)
	{
		new Float:ang[3];
		pev(e, pev_angles, ang);
		new Float:want = floatatan2(g_porg[near][1] - o[1], g_porg[near][0] - o[0], degrees);
		new Float:d = want - ang[1];
		while (d > 180.0) d -= 360.0;
		while (d < -180.0) d += 360.0;
		ang[1] += floatclamp(d, -25.0, 25.0);
		set_pev(e, pev_angles, ang);
	}
	if (!kind)
		return;
	if (g_pf[s][SH_BACK] > 0.0 && g_now >= g_pf[s][SH_BACK])
	{
		g_pf[s][SH_BACK] = 0.0;
		keeper_play(e, kind == 2 ? "idle01" : "idle1");
	}
	if (g_now >= g_pf[s][SH_NEXT] && g_pf[s][SH_BACK] <= 0.0)
	{
		g_pf[s][SH_NEXT] = g_now + random_float(8.0, 15.0);
		keeper_play(e, KEEPER_FIDGETS[kind][random(3)]);
		g_pf[s][SH_BACK] = g_now + 2.5;
	}
}

/* a nod (or Barney's wave) and a line after a purchase */
stock keeper_thanks(s, id)
{
	new e = g_aux[s];
	if (e <= 0 || !pev_valid(e))
		return;
	new kind = floatround(g_pf[s][SH_KIND]);
	if (!kind)
		return;
	keeper_play(e, kind == 2 ? (random_num(0, 1) ? "yes" : "bigyes") : "barn_wave");
	g_pf[s][SH_BACK] = get_gametime() + 2.5;
	new who[32];
	keeper_name(s, who, charsmax(who));
	client_print_color(id, print_team_default, "^4%s:^1 %s", who, KEEPER_LINES[kind][random(4)]);
}

/* USE in the shop opens its menu (people; bots buy with ls_shop_buy) */
stock bool:shop_use(id)
{
	new s = g_shopIn[id] - 1;
	if (s < 0 || !g_ent[s] || g_kind[s] != K_SHOP || !g_on[s])
		return false;
	if (g_state[s] != 0)
	{
		client_print(id, print_center, "The %s is closed now", g_label[s][0] ? g_label[s] : "shop");
		return true;
	}
	shop_menu(s, id);
	return true;
}

/* ---- turrets ---- */

stock turret_count(id)
{
	new n = 0;
	for (new i = 0; i < MAX_TUR; i++)
		if (g_turEnt[i] && g_turOwner[i] == id)
			n++;
	return n;
}

/* a friendly creature (the fork's friendly=1, owner=): shoots creatures, creatures go for it */
stock turret_spawn(owner, const Float:o[3], Float:yaw)
{
	new slot = -1;
	for (new i = 0; i < MAX_TUR && slot < 0; i++)
		if (!g_turEnt[i])
			slot = i;
	if (slot < 0 || !g_upCreatures)
		return 0;
	new extra[48];
	if (g_turHealth > 0)
		formatex(extra, charsmax(extra), " health=%d", g_turHealth);
	set_cvar_num("ls_creature_last", 0);
	server_cmd("ls_creature spawn %s %.0f %.0f %.0f %.0f friendly=1 owner=%d%s", g_turType, o[0], o[1], o[2], yaw, owner, extra);
	server_exec();
	new e = get_cvar_num("ls_creature_last");
	if (e <= 0 || !pev_valid(e))
	{
		log_amx("Turret: the game DLL didn't make a %s (friendly creatures need brain ls-next3 cfd2c3d or later, and the type precached)", g_turType);
		return 0;
	}
	g_turEnt[slot] = e;
	g_turOwner[slot] = owner;
	g_turUntil[slot] = g_turLife > 0.0 ? get_gametime() + g_turLife : 0.0;
	new ret;
	ExecuteForward(g_fwTurret, ret, e, owner);
	log_amx("Turret: %d placed a %s (%d) at %.0f %.0f %.0f in wave %d", owner, g_turType, e, o[0], o[1], o[2], g_wave);
	return e;
}

/* in front of the player, on the floor, with room, not on top of another turret */
stock turret_place(id, bool:pay)
{
	if (!is_user_alive(id) || g_down[id])
		return 0;
	if (turret_count(id) >= g_turPer)
	{
		if (!g_bot[id])
			client_print(id, print_center, "You have %d turret%s out already", g_turPer, g_turPer == 1 ? "" : "s");
		return 0;
	}
	new Float:ang[3], Float:o[3], Float:p[3];
	pev(id, pev_v_angle, ang);
	pev(id, pev_origin, o);
	new Float:yaw = ang[1];
	for (new tries = 0; tries < 3; tries++)
	{
		new Float:d = 64.0 - 16.0 * float(tries);
		p[0] = o[0] + d * floatcos(yaw, degrees);
		p[1] = o[1] + d * floatsin(yaw, degrees);
		p[2] = o[2];
		new Float:f = quiet_floor(p, id);
		if (o[2] - f > 120.0)
			continue;
		new Float:c[3];
		vcopy(p, c);
		c[2] = f + 37.0;
		engfunc(EngFunc_TraceHull, c, c, DONT_IGNORE_MONSTERS, HULL_HUMAN, id, 0);
		if (get_tr2(0, TR_StartSolid) || get_tr2(0, TR_AllSolid))
			continue;
		new bool:crowded = false;
		for (new i = 0; i < MAX_TUR && !crowded; i++)
		{
			if (!g_turEnt[i])
				continue;
			new Float:t[3];
			pev(g_turEnt[i], pev_origin, t);
			crowded = get_distance_f(t, c) < 80.0;
		}
		if (crowded)
			continue;
		if (pay)
		{
			if (cs_get_user_money(id) < g_turPrice)
				return 0;
		}
		else if (g_turKits[id] <= 0)
			return 0;
		p[2] = f + 1.0;
		new e = turret_spawn(id, p, yaw);
		if (!e)
			return 0;
		if (pay)
			cs_set_user_money(id, cs_get_user_money(id) - g_turPrice);
		else
			g_turKits[id]--;
		return e;
	}
	if (!g_bot[id])
		client_print(id, print_center, "No room for the turret there");
	return 0;
}

/* once a second: gone, dead or out of time */
stock turret_tick()
{
	new Float:now = get_gametime();
	for (new i = 0; i < MAX_TUR; i++)
	{
		new e = g_turEnt[i];
		if (!e)
			continue;
		new cls[16];
		if (pev_valid(e))
			pev(e, pev_classname, cls, charsmax(cls));
		if (!pev_valid(e) || !equal(cls, "ls_creature") || pev(e, pev_deadflag) != DEAD_NO)
		{
			g_turEnt[i] = 0;
			continue;
		}
		if (g_turUntil[i] > 0.0 && now >= g_turUntil[i])
		{
			server_cmd("ls_creature kill %d", e);
			g_turEnt[i] = 0;
			new o = g_turOwner[i];
			if (1 <= o <= MaxClients && is_user_connected(o) && !is_user_bot(o))
				client_print(o, print_center, "Your turret ran out");
		}
	}
}

/* lst_perk / lst_bench / lst_box / lst_trap markers (Last Stand's kit) are these components */
stock up_marker_kind(const k[], Trie:keys)
{
	/* Last Stand's spots are the defenders' */
	if ((equal(k, "lst_perk") || equal(k, "lst_bench") || equal(k, "lst_box") || equal(k, "lst_trap")) && !TrieKeyExists(keys, "team"))
		TrieSetString(keys, "team", "ct");
	if (equal(k, "lst_perk"))
		return K_PERK;
	if (equal(k, "lst_bench"))
		return K_BENCH;
	if (equal(k, "lst_box"))
		return K_BOX;
	if (equal(k, "lst_trap"))
	{
		if (!TrieKeyExists(keys, "fire_on"))
			TrieSetString(keys, "fire_on", "use");
		if (!TrieKeyExists(keys, "cost"))
			TrieSetString(keys, "cost", "1000");
		if (!TrieKeyExists(keys, "active_secs"))
			TrieSetString(keys, "active_secs", "30");
		if (!TrieKeyExists(keys, "rearm_secs"))
			TrieSetString(keys, "rearm_secs", "45");
		if (!TrieKeyExists(keys, "size") && !TrieKeyExists(keys, "radius"))
			TrieSetString(keys, "size", "192 192 64");
		return K_TRAP;
	}
	return K_NONE;
}

/* ---- natives ---- */

public n_player_mod(plugin, params)
{
	new id = get_param(1);
	new what[24];
	get_string(2, what, charsmax(what));
	new m = mod_index(what);
	if (!(1 <= id <= 32) || m < 0)
	{
		log_error(AMX_ERR_NATIVE, "ls_player_mod: player %d, modifier %s (one of health_max speed reload deploy damage revive_time fire_rate)", id, what);
		return 0;
	}
	new Float:oldMax;
	if (is_user_alive(id))
		pev(id, pev_max_health, oldMax);
	g_modSet[id][m] = floatmax(get_param_f(3), 0.01);
	if (m == MOD_HEALTH || m == MOD_SPEED)
		mods_apply(id, oldMax);
	return 1;
}

public Float:n_player_mod_get(plugin, params)
{
	new id = get_param(1);
	new what[24];
	get_string(2, what, charsmax(what));
	new m = mod_index(what);
	return m < 0 ? 1.0 : mod_get(id, m);
}

public n_player_mod_clear(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32))
		return 0;
	for (new m = 0; m < MOD_COUNT; m++)
		g_modSet[id][m] = 1.0;
	mods_apply(id);
	return 1;
}

public n_perk_give(plugin, params)
{
	new name[24];
	get_string(2, name, charsmax(name));
	return perk_give(get_param(1), perk_index(name));
}

public bool:n_perk_has(plugin, params)
{
	new name[24];
	get_string(2, name, charsmax(name));
	return perk_has(get_param(1), perk_index(name));
}

public n_perk_count(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= 32) ? g_nperks[id] : 0;
}

public n_perk_clear(plugin, params)
{
	new id = get_param(1);
	if (1 <= id <= 32)
		perk_clear(id);
	return 1;
}

/* ls_perk_buy(id, perk[], bool:tell): at the default prices (a menu with no machine) */
public bool:n_perk_buy(plugin, params)
{
	new name[24];
	get_string(2, name, charsmax(name));
	new id = get_param(1);
	if (!(1 <= id <= 32))
		return false;
	g_bot[id] = is_user_bot(id) != 0;
	return perk_buy(-1, id, perk_index(name), get_param(3) != 0);
}

public n_perk_price(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= 32) ? perk_price_at(-1, id) : 0;
}

public n_perk_tags(plugin, params)
{
	new out[64];
	new id = get_param(1);
	if (1 <= id <= 32)
		perk_tags(id, out, charsmax(out));
	set_string(2, out, get_param(3));
	return strlen(out);
}

/* ls_down_enable(bool:on, Float:bleed_secs = 30.0, team = 0, bool:lose_perks = true) */
public n_down_enable(plugin, params)
{
	g_dnOn = get_param(1) != 0;
	g_dnBleed = floatmax(get_param_f(2), 1.0);
	g_dnTeam = clamp(get_param(3), 0, 2);
	g_dnLosePerks = get_param(4) != 0;
	return 1;
}

public bool:n_is_down(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= 32) && g_down[id];
}

public n_down(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32) || !is_user_alive(id) || g_down[id])
		return 0;
	go_down(id);
	return 1;
}

public n_revive(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32) || !g_down[id])
		return 0;
	get_up(id, get_param(2));
	return 1;
}

public Float:n_down_left(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32) || !g_down[id])
		return 0.0;
	return floatmax(g_dnBleed - (get_gametime() - g_downAt[id]), 0.0);
}

public n_weapon_level(plugin, params)
{
	new id = get_param(1);
	new w = get_param(2);
	if (!w && 1 <= id <= 32 && is_user_alive(id))
		w = active_weapon(id);
	return w ? wpn_level(w) : 0;
}

public n_weapon_set_level(plugin, params)
{
	new w = get_param(1);
	if (w <= 0 || !pev_valid(w))
		return 0;
	set_pev(w, WPN_LEVEL, clamp(get_param(2), 1, 3));
	return 1;
}

/* ls_bench_use(id, bool:tell): an upgrade at the first bench's prices (or the defaults) */
public bool:n_bench_use(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32))
		return false;
	g_bot[id] = is_user_bot(id) != 0;
	new b = -1;
	for (new s = 0; s < g_nslots && b < 0; s++)
		if (g_ent[s] && g_kind[s] == K_BENCH)
			b = s;
	return bench_use(b, id, get_param(2) != 0);
}

/* ls_box_use(ent, id, bool:tell): ent 0 = the box that's out */
public bool:n_box_use(plugin, params)
{
	new ent = get_param(1), id = get_param(2);
	if (!(1 <= id <= 32))
		return false;
	g_bot[id] = is_user_bot(id) != 0;
	new s = -1;
	if (ent)
		s = arg_slot(1, K_BOX);
	else
		for (new o = 0; o < g_nslots && s < 0; o++)
			if (g_ent[o] && g_kind[o] == K_BOX && g_on[o] && g_state[o] == 1)
				s = o;
	return s >= 0 && box_use(s, id, get_param(3) != 0);
}

/* ls_pickup_spawn(const kind[], const Float:origin[3], Float:life = 25.0, const model[] = "", r, g, b, team = 0) */
public n_pickup_spawn(plugin, params)
{
	new kind[32], model[64], keys[192];
	get_string(1, kind, charsmax(kind));
	new Float:o[3];
	get_array_f(2, o, 3);
	get_string(4, model, charsmax(model));
	formatex(keys, charsmax(keys), "kind=%s life=%.1f glow=^"%d %d %d^" team=%d", kind, get_param_f(3), get_param(5), get_param(6), get_param(7), get_param(8));
	if (model[0])
		format(keys, charsmax(keys), "%s model=%s", keys, model);
	g_now = get_gametime();
	return comp_make(K_PICKUP, o, keys, SRC_RUNTIME);
}

/* ls_turret_setup(price, Float:life, per_player, const type[], health, bool:bots) */
public n_turret_setup(plugin, params)
{
	g_turPrice = max(get_param(1), 0);
	g_turLife = get_param_f(2);
	g_turPer = max(get_param(3), 1);
	get_string(4, g_turType, charsmax(g_turType));
	if (!g_turType[0])
		copy(g_turType, charsmax(g_turType), "turret");
	g_turHealth = get_param(5);
	g_turBots = get_param(6) != 0;
	return 1;
}

public n_turret_give(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32))
		return 0;
	g_turKits[id] = max(g_turKits[id] + get_param(2), 0);
	return g_turKits[id];
}

public n_turret_kits(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= 32) ? g_turKits[id] : 0;
}

/* ls_turret_place(id, bool:pay = false): where they look; pay = at the set price, else a kit */
public n_turret_place(plugin, params)
{
	new id = get_param(1);
	if (!(1 <= id <= 32))
		return 0;
	g_bot[id] = is_user_bot(id) != 0;
	return turret_place(id, get_param(2) != 0);
}

public n_turret_spawn(plugin, params)
{
	new Float:o[3];
	get_array_f(2, o, 3);
	return turret_spawn(get_param(1), o, get_param_f(3));
}

public n_turret_count(plugin, params)
{
	new id = get_param(1);
	return (1 <= id <= 32) ? turret_count(id) : 0;
}

/* ls_shop_buy(ent, id, const item[], bool:tell = true): by its name or what it gives */
public bool:n_shop_buy(plugin, params)
{
	new s = arg_slot(1, K_SHOP), id = get_param(2);
	if (s < 0 || !(1 <= id <= 32))
		return false;
	new item[64];
	get_string(3, item, charsmax(item));
	new i = shop_find(s, item);
	if (i < 0)
		return false;
	g_bot[id] = is_user_bot(id) != 0;
	g_now = get_gametime();
	return shop_buy_item(s, id, i, get_param(4) != 0);
}

/* ls_shop_price(ent, const item[]): its price now, -1 if the shop has no such item */
public n_shop_price(plugin, params)
{
	new s = arg_slot(1, K_SHOP);
	if (s < 0)
		return -1;
	new item[64];
	get_string(2, item, charsmax(item));
	new i = shop_find(s, item);
	if (i < 0)
		return -1;
	new name[64], give[64], what[96], base;
	shop_item(s, i, name, charsmax(name), base, give, charsmax(give), what, charsmax(what));
	return shop_price_now(s, base);
}

public n_shop_menu(plugin, params)
{
	new s = arg_slot(1, K_SHOP), id = get_param(2);
	if (s < 0 || !(1 <= id <= 32) || !is_user_alive(id))
		return 0;
	shop_menu(s, id);
	return 1;
}

public n_shop_open(plugin, params)
{
	new s = arg_slot(1, K_SHOP);
	if (s < 0)
		return 0;
	g_state[s] = get_param(2) ? 0 : 1;
	return 1;
}

public bool:n_shop_is_open(plugin, params)
{
	new s = arg_slot(1, K_SHOP);
	return s >= 0 && g_state[s] == 0;
}

/* ls_shop_why(const reason[]): inside ls_on_shop_item, what the menu says */
public n_shop_why(plugin, params)
{
	get_string(1, g_shopWhy, charsmax(g_shopWhy));
	return 1;
}

public bool:n_shop_at(plugin, params)
{
	new s = arg_slot(1, K_SHOP), id = get_param(2);
	if (s < 0 || !(1 <= id <= 32) || !is_user_alive(id))
		return false;
	new Float:o[3];
	pev(id, pev_origin, o);
	return inside(s, o);
}
/* ==== end W1 ==== */
