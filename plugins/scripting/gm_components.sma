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
	K_COUNT
};

new const KIND_NAME[K_COUNT][] =
{
	"",
	"ls_respawn_point", "ls_weapon_spawner", "ls_damage_zone", "ls_timed_door", "ls_trigger_trap",
	"ls_capture_point", "ls_safe_zone", "ls_zone_shrinker", "ls_loot_container", "ls_supply_drop",
	"ls_barricade", "ls_wave_spawner", "ls_extraction_zone", "ls_checkpoint", "ls_moving_platform",
	"ls_player_class", "ls_meeting_point", "ls_supply_crate"
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
enum { TRAP_ARMED = 0, TRAP_PENDING, TRAP_COOLDOWN, TRAP_OFF };
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
new g_nslots;

new g_slotOf[MAX_ENTS];
new g_doorOf[MAX_ENTS];
new g_pend[MAX_ENTS];
new g_parseEnt;
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
		g_pendKeys[e] = Invalid_Trie;

	g_sprBeam = precache_model("sprites/laserbeam.spr");
	g_sprSmoke = precache_model("sprites/smokepuff.spr");
	g_mdlGibs = precache_model("models/woodgibs.mdl");
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

	g_msgBarTime = get_user_msgid("BarTime");
	g_msgBarTime2 = get_user_msgid("BarTime2");
	g_msgStatusIcon = get_user_msgid("StatusIcon");
	g_msgFade = get_user_msgid("ScreenFade");
	g_msgRoundTime = get_user_msgid("RoundTime");
	g_msgRadar = get_user_msgid("Radar");
	g_sync = CreateHudSyncObj();

	RegisterHamPlayer(Ham_Spawn, "ham_player_spawn", 1);
	RegisterHamPlayer(Ham_Killed, "ham_player_killed", 1);
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
	register_forward(FM_ShouldCollide, "fm_shouldcollide");

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
	new key[32], val[128];
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
		new kind = kind_of(val);
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
	new key[32], val[128];
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
	if (box_occupied(mn, mx))
		return;
	/* a barricade only re-forms with no creature close by (on either side) */
	if (g_kind[s] == K_BARRICADE && creature_near(mn, mx, 250.0))
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
stock bool:box_occupied(const Float:mn[3], const Float:mx[3])
{
	new Float:o[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		pev(p, pev_origin, o);
		if (o[0] + 18.0 > mn[0] && o[0] - 18.0 < mx[0] && o[1] + 18.0 > mn[1] && o[1] - 18.0 < mx[1] && o[2] + 38.0 > mn[2] && o[2] - 38.0 < mx[2])
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
				g_boards[s][g_nboards[s]++] = e;
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
	if (sec)
	{
		visuals();
		lap_hud();
	}
	hud_flush();
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
		p[2] = c[2] + 64.0;
		if (engfunc(EngFunc_PointContents, p) != CONTENTS_EMPTY)
			continue;
		new Float:f = floor_z(p, 0);
		if (p[2] - f > 2000.0)
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
		tier_glow(ent, floatround(g_pf[s][W_TIER]));
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
		if (d > 40.0 || dz < -16.0 || dz > 96.0)
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
		if (get_distance_f(g_porg[p], g_org[s]) < 64.0)
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
			smoke(g_org[s]);
			fwd_drop(ent, 1, g_org[s]);
			fire_targets(s, 0);
			hint_comp(s);
		}
	}
	else if (g_state[s] == CRATE_FULL)
	{
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
	new n = g_nboards[s];
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

/* Players (their bullets, their bots' sight, their steps) pass through a see-through barricade;
   creatures don't, so they have to break it */
public fm_shouldcollide(touched, other)
{
	if (touched <= 0 || touched >= MAX_ENTS || !g_slotOf[touched] || !(1 <= other <= MaxClients))
		return FMRES_IGNORED;
	new s = g_slotOf[touched] - 1;
	if (g_kind[s] != K_BARRICADE || !ki(s, "see_through", 1))
		return FMRES_IGNORED;
	forward_return(FMV_CELL, 0);
	return FMRES_SUPERCEDE;
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
		else if (e > 0 && pev_valid(e) && pev(e, pev_deadflag) == DEAD_NO && pev(e, pev_health) > 0.0)
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
			if (g_state[s] == 0)
				hint("attract c%d %.0f %.0f %.0f 400 %d 0 0", ent, o[0], o[1], o[2], 20 + floatround(g_pf[s][W_TIER]) * 15);
			else
				hint("forget c%d", ent);
		}
		case K_LOOT:
		{
			if (g_state[s] == 0)
				hint("attract c%d %.0f %.0f %.0f 500 %d 0 0", ent, o[0], o[1], o[2], 20 + floatround(g_pf[s][W_TIER]) * 20);
			else
				hint("forget c%d", ent);
		}
		case K_CRATE:
		{
			if (g_state[s] == CRATE_FULL)
				hint("attract c%d %.0f %.0f %.0f 1500 90 0 0", ent, o[0], o[1], o[2]);
			else if (g_state[s] == CRATE_EMPTY)
				hint("forget c%d", ent);
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
	set_dhudmessage(r, g, b, -1.0, 0.22, 0, 0.0, secs, 0.2, 0.5);
	show_dhudmessage(id, "%s", text);
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
		case 1: render(ent, kRenderFxGlowShell, 180, 180, 180, kRenderNormal, 4);
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
		cs_set_user_bpammo(id, csw, max(cs_get_user_bpammo(id, csw), ammo_full(csw) / 2));
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
	new s = arg_slot(1, K_ZONE);
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
	new s = arg_slot(1, K_ZONE);
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
	new s = arg_slot(1, K_ZONE);
	if (s < 0)
		return 0.0;
	g_now = get_gametime();
	return zone_radius_now(s);
}

public n_zone_center(plugin, params)
{
	new s = arg_slot(1, K_ZONE);
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
		new s = arg_slot(1, K_ZONE);
		return s >= 0 && dist2d(o, g_va[s]) <= zone_radius_now(s);
	}
	for (new s = 0; s < g_nslots; s++)
		if (g_ent[s] && g_kind[s] == K_ZONE && g_on[s] && dist2d(o, g_va[s]) > zone_radius_now(s))
			return 0;
	return 1;
}

public n_zone_damage(plugin, params)
{
	new s = arg_slot(1, K_ZONE);
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
		z = arg_slot(1, K_ZONE);
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
