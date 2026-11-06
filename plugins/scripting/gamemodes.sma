/*
	Game Mode Menu
	One in-game menu for game modes, maps, bots, match/fun settings, player tools,
	add-ons and every AMX Mod X admin menu.

	Open with: gamemenu (bound to F3), or say /gm, /menu, /modes

	Files:
	  configs/gamemodes/modes.ini            list of modes
	  configs/gamemodes/<id>.plugins.ini     plugins loaded for that mode
	  configs/gamemodes/<id>.cfg             cvars for that mode
	  configs/gamemodes/base.cfg             defaults executed before every mode
	  configs/gamemodes/mappresets.ini       maps that automatically use a mode
	  configs/gamemodes/maps/<id>.txt        optional map list for that mode
	  configs/gamemodes/addons.ini           optional add-ons (+ addons/<id>.ini settings)
	  configs/gamemodes/help.txt             text shown by Help
	  configs/plugins-gamemode.ini           generated, do not edit
	  configs/plugins-gmaddons.ini           generated, do not edit
	  data/gamemodes/                        saved menu settings and state

	During Tour of Duty (career) matches the menu stays out of the way: mode plugins
	are paused and no settings are applied, so missions play as designed.
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fun>
#include <fakemeta>
#include <hamsandwich>

#define PLUGIN  "Game Mode Menu"
#define VERSION "2.0"
#define AUTHOR  "local"

#define ACCESS        ADMIN_CFG
#define MAX_MODES     32
#define MAX_ADDONS    32
#define TASK_LOADOUT  4100
#define TASK_HINT     4200
#define TASK_NAVPOLL  4300
#define TASK_AUTODIFF 4600
#define TASK_LEARNGUARD 4700
#define TASK_NOBOMB   4800
#define TASK_SELFRESPAWN 4400
#define TASK_MONEY       4500
#define TASK_EDICTS   4900

enum { CAT_HIDDEN, CAT_BOT, CAT_MATCH, CAT_FUN }
enum { SC_NONE, SC_GLOBAL, SC_MODE }

enum _:SettingData
{
	SD_Cvar[32],
	SD_Name[24],
	SD_Values[64],
	SD_Labels[96],
	SD_Cat,
	SD_Scope,
	SD_NeedFlag[4]
}

new const SETTINGS[][SettingData] =
{
	{ "bot_quota",          "Bot count",        "",                         "",                                    CAT_HIDDEN, SC_GLOBAL, "" },
	{ "gm_mappresets",      "Map presets",      "0 1",                      "Off On",                              CAT_HIDDEN, SC_GLOBAL, "" },
	{ "bot_difficulty",     "Difficulty",       "0 1 2 3",                  "Easy Normal Hard Expert",             CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_bot_weapons",     "Bot weapons",      "0 1 2 3",                  "Any Pistols Snipers Knives",          CAT_BOT,    SC_MODE,   "" },
	{ "bot_chatter",        "Bot chatter",      "normal minimal radio off", "Normal Minimal Radio Off",            CAT_BOT,    SC_GLOBAL, "" },
	{ "bot_join_team",      "Bots join",        "any T CT",                 "Any Terrorists CTs",                  CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_botstyle",        "Bot style",        "0 4 1 2 3 5 6 7",          "Classic|Newbie lobby|Pub server|Matchmaking|Veterans|Locked In|Hackers|Gods", CAT_BOT, SC_GLOBAL, "" },
	{ "gm_hb_reaction",     "Human reaction times", "1 0",                  "On Off",                              CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_hb_aim",          "Human aim & bursts", "1 0",                    "On Off",                              CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_hb_mistakes",     "Bot mistakes",     "1 0",                      "On Off",                              CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_hb_bhop",         "Pro bots bunnyhop", "1 0",                     "On Off",                              CAT_BOT,    SC_GLOBAL, "" },
	{ "bot_profile_db",     "Bot roster",       "BotProfile.db BotProfileEnhanced.db", "Classic|Enhanced (next map)", CAT_BOT, SC_GLOBAL, "" },
	{ "bot_stop",           "Freeze bots",      "0 1",                      "No Yes",                              CAT_BOT,    SC_NONE,   "" },
	{ "gm_autodiff",        "Auto difficulty",  "0 1",                      "Off|On (bots adapt to how you're doing)", CAT_BOT, SC_GLOBAL, "" },
	{ "gm_regulars",        "Bot names",        "1 0",                      "Regulars & clan tags|Classic CZ names", CAT_BOT, SC_GLOBAL, "" },
	{ "gm_churn",           "Players come & go", "0 1 2",                   "Off|Now and then|Busy server",        CAT_BOT,    SC_GLOBAL, "" },
	{ "gm_rtv",             "Rock the vote",    "1 0",                      "On Off",                              CAT_MATCH,  SC_GLOBAL, "" },
	{ "gm_welcome",         "Welcome screen",   "1 2 0",                    "Once per session|Every map|Off",      CAT_MATCH,  SC_GLOBAL, "" },
	{ "wc3_xp_multiplier",     "Leveling speed",      "0.1 0.25 0.5 1.0",          "Very fast|Fast|Normal|Slow (original)", CAT_MATCH, SC_MODE, "" },
	{ "wc3_start_level",       "Starting level",      "0 2 4 6",                   "0|2|4|6",                               CAT_MATCH, SC_MODE, "" },
	{ "wc3_races",             "Races",               "8 9",                       "Standard 8|Plus Chameleon",             CAT_MATCH, SC_MODE, "" },
	{ "wc3_ult_cooldown",      "Ultimate cooldown",   "20 35 50",                  "Short|Normal|Long",                     CAT_MATCH, SC_MODE, "" },
	{ "wc3_bot_buy_item",      "Bots buy items",      "0 0.33 0.66 1.0",           "Never|Sometimes|Often|Always",          CAT_BOT, SC_MODE, "" },
	{ "zp_sniper_chance",      "Sniper rounds",       "10 20 50",                  "Often|Sometimes|Rare",                  CAT_MATCH, SC_MODE, "" },
	{ "zp_assassin_enable",    "Assassin rounds",     "1 0",                       "On Off",                                CAT_MATCH, SC_MODE, "" },
	{ "zp_starting_ammo_packs", "Starting ammo packs", "5 20 100",                  "5|20|100",                              CAT_MATCH, SC_MODE, "" },
	{ "hns_hidetime",          "Hide time",           "5 10 15 20",                "5s|10s|15s|20s",                        CAT_MATCH, SC_MODE, "" },
	{ "hns_hiders_flashbangs", "Hider flashbangs",    "0 1 2",                     "None|One|Two",                          CAT_MATCH, SC_MODE, "" },
	{ "hns_semiclip",          "Walk through players", "0 1 2",                     "Off|Teammates|Everyone",                CAT_MATCH, SC_MODE, "" },
	{ "PBR_Deathmatch",        "Paintball respawn",   "0 1",                       "Elimination|Respawn after 3s",          CAT_MATCH, SC_MODE, "" },
	{ "pbmoves_enabled",       "Sprint (right-click)", "1 0",                       "On Off",                                CAT_MATCH, SC_MODE, "" },
	{ "kz_hook_prize",         "Hook after finishing", "1 0",                       "On Off",                                CAT_MATCH, SC_MODE, "" },
	{ "deathrun_lifesystem",   "Extra lives",         "0 1 2",                     "None|One|Two",                          CAT_MATCH, SC_MODE, "" },
	{ "deathrun_giveusp",      "Runners get a USP",   "0 1",                       "No Yes",                                CAT_MATCH, SC_MODE, "" },
	{ "dr_bot_traps",          "Bot springs traps",   "1 0",                       "On Off",                                CAT_BOT, SC_MODE, "" },
	{ "dr_bot_trap_chance",    "Bot trap eagerness",  "15 35 70",                  "Low|Normal|High",                       CAT_BOT, SC_MODE, "" },
	{ "bb_build_time",         "Build time",          "45 90 145",                 "45s|90s|145s",                          CAT_MATCH, SC_MODE, "" },
	{ "bb_bot_zombie_classes", "Bot zombie classes",  "1 3 6 12",                  "Basic|3|6|All",                         CAT_BOT, SC_MODE, "" },
	{ "sj_score",              "Goals to win",        "5 10 15 25",                "5|10|15|25",                            CAT_MATCH, SC_MODE, "" },
	{ "sj_random",             "Shuffle teams",       "0 1",                       "No Yes",                                CAT_MATCH, SC_MODE, "" },
	{ "sj_bots",               "Bots chase the ball", "1 0",                       "On Off",                                CAT_BOT, SC_MODE, "" },
	{ "sh_maxpowers",          "Max heroes",          "3 5 10 15",                 "3|5|10|15",                             CAT_MATCH, SC_MODE, "" },
	{ "sh_minlevel",           "Starting level",      "0 1 3 5",                   "0|1|3|5",                               CAT_MATCH, SC_MODE, "" },
	{ "sh_maxbinds",           "Power-key heroes",    "1 2 3",                     "1|2|3",                                 CAT_MATCH, SC_MODE, "" },
	{ "sh_bot_powers",         "Bots use powers",     "1 0",                       "On Off",                                CAT_BOT, SC_MODE, "" },
	{ "bot_allow_pistols",  "Bots use pistols", "1 0",                      "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_shotguns", "Bots use shotguns", "1 0",                     "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_sub_machine_guns", "Bots use SMGs", "1 0",                 "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_rifles",   "Bots use rifles",  "1 0",                      "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_machine_guns", "Bots use machine guns", "1 0",             "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_snipers",  "Bots use snipers", "1 0",                      "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_allow_grenades", "Bots use grenades", "1 0",                     "Yes No",                              CAT_BOT,    SC_MODE,   "" },
	{ "bot_defer_to_human", "Leave objective to me", "0 1",                       "Off On",                              CAT_BOT,  SC_MODE,   "" },
	{ "bot_walk",           "Bots walk only",   "0 1",                       "Off On",                              CAT_BOT,  SC_MODE,   "" },
	{ "bot_allow_rogues",   "Rogue bots",       "1 0",                       "Some|None",                           CAT_BOT,  SC_MODE,   "" },
	{ "bot_quota_mode",     "Bot count mode",   "normal fill match",         "Fixed|Fill server|Per human",         CAT_BOT,  SC_GLOBAL,   "" },
	{ "bot_zombie",         "Peaceful bots",    "0 1",                      "Off|On (they don't move or shoot)",   CAT_BOT,    SC_NONE,   "" },

	{ "mp_roundtime",       "Round time",       "1.75 3 5 9",               "1:45 3:00 5:00 9:00",                 CAT_MATCH,  SC_MODE,   "" },
	{ "mp_freezetime",      "Freeze time",      "0 3 6 10",                 "0s 3s 6s 10s",                        CAT_MATCH,  SC_MODE,   "" },
	{ "mp_buytime",         "Buy time",         "0.25 0.5 1.5 3 9",         "0:15 0:30 1:30 3:00 9:00",            CAT_MATCH,  SC_MODE,   "" },
	{ "mp_startmoney",      "Start money",      "800 3000 8000 16000",      "$800 $3000 $8000 $16000",             CAT_MATCH,  SC_MODE,   "" },
	{ "mp_friendlyfire",    "Friendly fire",    "0 1",                      "Off On",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_autoteambalance", "Team balance",     "1 0",                      "Auto|Off",                            CAT_MATCH,  SC_MODE,   "" },
	{ "mp_limitteams",      "Max team difference", "2 1 3 0",               "2 players|1 player|3 players|No limit", CAT_MATCH, SC_MODE,  "" },
	{ "mp_respawn_immunitytime", "Spawn protection", "0 2 3 5",             "Off|2s|3s|5s",                        CAT_MATCH,  SC_MODE,   "" },
	{ "mp_timelimit",       "Map time limit",   "0 10 20 30 45 60",         "None|10 min|20 min|30 min|45 min|60 min", CAT_MATCH, SC_MODE, "" },
	{ "mp_maxrounds",       "Round limit",      "0 10 16 30",               "None 10 16 30",                       CAT_MATCH,  SC_MODE,   "" },
	{ "mp_buy_anywhere",    "Buy anywhere",     "0 1",                       "Off On",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_round_infinite",  "Endless rounds",   "0 1",                       "Off On",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_falldamage",      "Fall damage",      "1 0",                       "On Off",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_footsteps",       "Footsteps",        "1 0",                       "On Off",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_free_armor",      "Free armor",       "0 1 2",                     "Off|Kevlar|Kevlar + helmet",          CAT_MATCH,  SC_MODE,   "" },
	{ "mp_auto_reload_weapons", "Auto-reload",      "0 1",                       "Off On",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_weapons_allow_map_placed", "Map weapons",      "1 0",                       "Keep|Remove",                         CAT_MATCH,  SC_MODE,   "" },
	{ "mp_scoreboard_showhealth", "Scoreboard health", "3 0",                       "Shown|Hidden",                        CAT_MATCH,  SC_MODE,   "" },
	{ "mp_scoreboard_showmoney", "Scoreboard money", "3 0",                       "Shown|Hidden",                        CAT_MATCH,  SC_MODE,   "" },
	{ "gm_nobomb",          "Bomb planting",    "0 1",                      "Allowed|Off",                         CAT_MATCH,  SC_MODE,   "" },
	{ "gg_block_objectives", "Bomb & hostages", "0 1",                      "On|Off",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_c4timer",         "C4 timer",         "25 35 45",                 "25s 35s 45s",                         CAT_MATCH,  SC_MODE,   "" },
	{ "mp_autoteambalance", "Team balance",     "0 1",                      "Off On",                              CAT_MATCH,  SC_MODE,   "" },
	{ "mp_limitteams",      "Team size limit",  "0 2",                      "Off 2",                               CAT_MATCH,  SC_MODE,   "" },
	{ "gmdm_random_spawns", "Spawn points",     "1 0",                      "Random|Team spawns",                  CAT_MATCH,  SC_MODE,   "" },
	{ "gmdm_respawn_delay", "Respawn delay",    "0.5 1.5 3 5",               "0.5s 1.5s 3s 5s",                     CAT_MATCH,  SC_MODE,   "" },
	{ "gmdm_protect",       "Spawn protection", "0 2 4",                     "Off 2s 4s",                           CAT_MATCH,  SC_MODE,   "" },
	{ "gmdm_guns",          "Gun menu",         "0 1 2 3 4",                "Everything|Rifles|SMGs & shotguns|Snipers|Pistols only", CAT_MATCH, SC_MODE, "" },
	{ "gmdm_grenade",       "Free grenade",     "1 0",                       "Yes No",                              CAT_MATCH,  SC_MODE,   "" },
	{ "gg_dm",              "Respawn",          "1 0",                       "Instant|Next round",                  CAT_MATCH,  SC_MODE,   "" },
	{ "gg_dm_spawn_random", "Spawn points",     "2 0",                       "Random|Team spawns",                  CAT_MATCH,  SC_MODE,   "" },
	{ "gg_dm_spawn_delay",  "Respawn delay",    "0.5 1.5 3 5",               "0.5s 1.5s 3s 5s",                     CAT_MATCH,  SC_MODE,   "" },
	{ "gg_dm_sp_time",      "Spawn protection", "0 1 3",                     "Off 1s 3s",                           CAT_MATCH,  SC_MODE,   "" },
	{ "gm_gg_order",        "Weapon order",     "0 1 2 3 4",                "Classic|No shotguns|Short game|Big guns first|Pistols & snipers", CAT_MATCH, SC_MODE, "g" },
	{ "gg_warmup_timer_setting", "Warmup",       "0 30 60",                  "Off 30s 60s",                         CAT_MATCH,  SC_MODE,   "" },
	{ "gg_ignore_bots",     "Bots can win",     "0 2",                      "Yes No",                              CAT_MATCH,  SC_MODE,   "" },
	{ "gg_kills_per_lvl",   "Kills per gun",    "1 2 3 5",                   "1 2 3 5",                             CAT_MATCH,  SC_MODE,   "" },
	{ "gg_kills_botmod",    "Bot kills count",  "1 0.5",                     "Full|Half",                           CAT_MATCH,  SC_MODE,   "" },
	{ "gg_turbo",           "New gun",          "1 0",                       "Right away|Next spawn",               CAT_MATCH,  SC_MODE,   "" },
	{ "gg_knife_pro",       "Knife steals a level", "1 0",                       "Yes No",                              CAT_MATCH,  SC_MODE,   "" },
	{ "gg_nade_glock",      "Pistol with grenade", "1 0",                       "Yes No",                              CAT_MATCH,  SC_MODE,   "" },
	{ "zp_deathmatch",      "Respawn",          "0 1 2 3 4",                 "Off|As human|As zombie|Random|Balanced", CAT_MATCH,  SC_MODE,   "" },
	{ "zp_spawn_delay",     "Respawn delay",    "2 5 10",                    "2s 5s 10s",                           CAT_MATCH,  SC_MODE,   "" },
	{ "zp_zombie_health_scale", "Zombie health", "0.5 1 1.5 2",             "Half|Normal|1.5x|Double",             CAT_MATCH,  SC_MODE,   "" },
	{ "zp_zombie_speed_scale", "Zombie speed",  "0.8 1 1.2 1.4",            "Slower|Normal|Faster|Much faster",    CAT_MATCH,  SC_MODE,   "" },
	{ "zp_zombie_first_hp", "First zombie health", "1 2 3",                 "Normal|Double|Triple",                CAT_MATCH,  SC_MODE,   "" },
	{ "zp_human_speed",     "Human speed",      "0 240 280",                "Weapon speed|Normal|Fast",            CAT_MATCH,  SC_MODE,   "" },
	{ "zp_nvg_give",        "Night vision",     "1 2 0",                    "Auto on|In inventory|None",           CAT_MATCH,  SC_MODE,   "" },
	{ "zp_human_health",    "Human health",     "100 200 500",               "100 200 500",                         CAT_MATCH,  SC_MODE,   "" },
	{ "gm_zp_specials",     "Special rounds",   "10 20 50",                 "Often|Sometimes|Rarely",              CAT_MATCH,  SC_MODE,   "z" },
	{ "zp_nem_enabled",     "Nemesis rounds",   "1 0",                       "On Off",                              CAT_MATCH,  SC_MODE,   "" },
	{ "zp_surv_enabled",    "Survivor rounds",  "1 0",                       "On Off",                              CAT_MATCH,  SC_MODE,   "" },

	{ "gm_weapons",         "Weapons",          "0 1 2 3 4 5",              "All Knives Pistols Deagle AWP Scout", CAT_FUN,    SC_MODE,   "w" },
	{ "sv_gravity",         "Gravity",          "800 600 400 210",          "Normal|Low|Lower|Scoutzknivez",       CAT_FUN,    SC_MODE,   "" },
	{ "sv_airaccelerate",   "Air control",      "10 100",                   "Normal|Surf / bhop",                  CAT_FUN,    SC_MODE,   "" },
	{ "gm_infammo",         "Unlimited ammo",   "0 1",                      "Off On",                              CAT_FUN,    SC_MODE,   "" },
	{ "gm_hsonly",          "Headshots only",   "0 1",                      "Off On",                              CAT_FUN,    SC_MODE,   "" },
	{ "zp_lighting",        "Zombie darkness",  "a d g 0",                  "Pitch black|Very dark|Dark|Normal lighting", CAT_FUN, SC_MODE, "" },
	{ "gm_autobhop",        "Auto bunnyhop",    "0 1",                      "Off On",                              CAT_FUN,    SC_GLOBAL, "" },
	{ "gm_tracers",         "Bullet tracers",   "0 1 2",                    "Off Tracers Lasers",                  CAT_FUN,    SC_GLOBAL, "" },
	{ "gm_dmgnumbers",      "Damage numbers",   "0 1",                      "Off On",                              CAT_FUN,    SC_GLOBAL, "" },
	{ "mp_infinite_grenades", "Infinite grenades", "0 1",                       "Off On",                              CAT_FUN,  SC_MODE,   "" },
	{ "gm_impacts",         "Show bullet impacts", "0 1",                       "Off On",                              CAT_FUN,  SC_GLOBAL,   "" },
	{ "gm_nadetrail",       "Grenade trails",   "0 1",                       "Off On",                              CAT_FUN,  SC_GLOBAL,   "" },
	// "w": only modes with normal rounds (Zombie Plague, Deathrun, Base Builder and SoccerJam run their own respawns)
	{ "gm_autorespawn",     "Respawn me instantly", "0 1",                  "Off On",                              CAT_FUN,    SC_MODE,   "w" },
	{ "gm_infmoney",        "Unlimited money",  "0 1",                      "Off On",                              CAT_FUN,    SC_MODE,   "" }
}

// Max backpack ammo, indexed by CSW_*
new const MAXBP[31] = { 0, 52, 0, 90, 0, 32, 0, 100, 90, 0, 120, 100, 100, 90, 90, 90, 100, 120, 30, 120, 200, 32, 90, 120, 90, 0, 35, 90, 90, 0, 100 }

new const BOT_COUNTS[] = { 0, 1, 2, 3, 4, 5, 7, 9, 11, 13, 15, 19, 23, 31 }

new const GIVE_WEAPONS[][][] =
{
	{ "weapon_ak47",      "AK-47" },
	{ "weapon_m4a1",      "M4A1" },
	{ "weapon_awp",       "AWP" },
	{ "weapon_deagle",    "Desert Eagle" },
	{ "weapon_scout",     "Scout" },
	{ "weapon_m249",      "M249" },
	{ "weapon_mp5navy",   "MP5" },
	{ "weapon_p90",       "P90" },
	{ "weapon_xm1014",    "Auto shotgun" },
	{ "weapon_sg552",     "SG-552" },
	{ "weapon_aug",       "AUG" },
	{ "weapon_g3sg1",     "G3SG1 auto-sniper" },
	{ "weapon_elite",     "Dual Elites" },
	{ "nades",            "Grenade pack" },
	{ "armor",            "Kevlar + helmet" }
}

new const ADMIN_MENUS[][][] =
{
	{ "amx_kickmenu",        "Kick players" },
	{ "amx_banmenu",         "Ban players" },
	{ "amx_slapmenu",        "Slap / slay players" },
	{ "amx_teammenu",        "Move players to a team" },
	{ "amx_clcmdmenu",       "Client commands" },
	{ "amx_votemapmenu",     "Start a map vote" },
	{ "amx_restmenu",        "Weapon restrictions" },
	{ "amx_statscfgmenu",    "Stats & announcements" },
	{ "amx_cvarmenu",        "Server cvars" },
	{ "amx_plugincvarmenu",  "Plugin cvars" },
	{ "amx_plugincmdmenu",   "Plugin commands" },
	{ "amx_pausecfgmenu",    "Pause / unpause plugins" },
	{ "amx_cmdmenu",         "Command menu" },
	{ "amx_cfgmenu",         "Config menu" },
	{ "amx_speechmenu",      "Speech menu" },
	{ "amx_langmenu",        "Language" },
	{ "amxmodmenu",          "Full AMX Mod X menu" }
}

enum { MC_DE, MC_CS, MC_OBJ, MC_SURF, MC_FUN, MC_OTHER, MC_COUNT }
new const MAPCAT_NAMES[MC_COUNT][] = { "Bomb defusal (de_)", "Hostage rescue (cs_)", "Other objectives (as_, es_)", "Surf", "Arena & fun", "Everything else" }

new bool:g_BotHams
new bool:g_Paused
new g_ModeId[MAX_MODES][32]
new g_ModeName[MAX_MODES][48]
new g_ModeDesc[MAX_MODES][64]
new g_ModeCmd[MAX_MODES][32]
new g_ModeFlags[MAX_MODES][8]
new g_ModeCount
new g_Cur

new g_AddonId[MAX_ADDONS][32]
new g_AddonName[MAX_ADDONS][48]
new g_AddonPlugins[MAX_ADDONS][160]
new g_AddonDesc[MAX_ADDONS][64]
new g_AddonCmd[MAX_ADDONS][32]
new bool:g_AddonOn[MAX_ADDONS]
new bool:g_AddonLoaded[MAX_ADDONS]
new g_AddonCount

#define MAX_PRESETS 48

new g_PresetId[MAX_PRESETS][24]
new g_PresetName[MAX_PRESETS][40]
new g_PresetDesc[MAX_PRESETS][64]
new g_PresetMode[MAX_PRESETS][32]
new g_PresetMap[MAX_PRESETS][32]
new g_PresetBots[MAX_PRESETS]
new g_PresetSettings[MAX_PRESETS][1024]
new bool:g_PresetUser[MAX_PRESETS]
new g_PresetCount
new g_PickPreset[33]
new bool:g_ApplyingPreset

new bool:g_SavedHas[sizeof SETTINGS]
new g_SavedVal[sizeof SETTINGS][40]

new Array:g_Maps
new Array:g_MapNav

new g_PickMode[33]      // mode chosen in the mode menu, -1 = keep current
new g_PickCat[33]
new g_Target[33]        // player picked in player tools
new bool:g_God[33]
new bool:g_Noclip[33]

new bool:g_Career
new bool:g_LearnGuard, Float:g_LearnTimelimit
new bool:g_Reloading
new g_ReloadNote[96]
new g_NavPolls

new g_CfgDir[128], g_DataDir[128]
new g_pWeapons, g_pBotWeapons, g_pInfAmmo, g_pHsOnly, g_pLoadout
new g_pGGOrder, g_pZPSpecials, g_pNoBomb, g_pImpacts, g_pNadeTrail
new bool:g_Recording, g_DemoName[64]
new g_pBhop, g_pTracers, g_pDmgNumbers
new g_BeamSpr, g_HudSync[2], g_HudNext, g_HudNav
new Float:g_PreHealth[33]
new g_pAutoRespawn, g_pInfMoney, g_pAutoDiff
new g_DiffKills, g_DiffDeaths

// practice: one saved spot per player
new Float:g_SavedPos[33][3], Float:g_SavedAng[33][3], bool:g_HasPos[33]

// crash memory and round-start summary
new g_CrashMap[32]
new bool:g_SummaryShown
new g_LastPreset[80]

// kill feed / chat log for the control panel (ring buffer of JSON fragments)
#define MAX_EVENTS 40
new g_EvBody[MAX_EVENTS][256]
new g_EvTime[MAX_EVENTS]
new g_EvSeq

public plugin_precache()
{
	g_BeamSpr = precache_model("sprites/laserbeam.spr")
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)

	g_pWeapons    = register_cvar("gm_weapons", "0")
	g_pGGOrder    = register_cvar("gm_gg_order", "0")
	g_pNoBomb     = register_cvar("gm_nobomb", "0")
	g_pImpacts    = register_cvar("gm_impacts", "0")
	g_pNadeTrail  = register_cvar("gm_nadetrail", "0")
	g_pZPSpecials = register_cvar("gm_zp_specials", "20")
	g_pBotWeapons = register_cvar("gm_bot_weapons", "0")
	g_pInfAmmo    = register_cvar("gm_infammo", "0")
	g_pHsOnly     = register_cvar("gm_hsonly", "0")
	g_pLoadout    = register_cvar("gm_loadout", "")
	g_pBhop       = register_cvar("gm_autobhop", "0")
	g_pTracers    = register_cvar("gm_tracers", "0")
	g_pDmgNumbers = register_cvar("gm_dmgnumbers", "0")
	g_pAutoRespawn = register_cvar("gm_autorespawn", "0")
	g_pInfMoney   = register_cvar("gm_infmoney", "0")
	g_pAutoDiff   = register_cvar("gm_autodiff", "0")
	set_task(60.0, "task_autodiff", TASK_AUTODIFF, _, _, "b")

	register_clcmd("gamequick", "cmd_quick")
	register_clcmd("say /q", "cmd_quick")
	register_clcmd("gm_savepos", "cmd_savepos")
	register_clcmd("gm_loadpos", "cmd_loadpos")
	register_srvcmd("gm_pos", "srv_pos")
	register_srvcmd("gm_crash", "srv_crash")

	RegisterHam(Ham_Killed, "player", "fw_killed_post", 1)
	register_event("Money", "ev_money", "b")
	register_event("DeathMsg", "ev_death", "a")
	register_logevent("ev_roundstart", 2, "1=Round_Start")
	register_event("SendAudio", "ev_win_t", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_win_ct", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "ev_win_draw", "a", "2&%!MRAD_rounddraw")
	register_clcmd("say", "hook_say")
	register_clcmd("say_team", "hook_say")
	register_cvar("gm_mappresets", "1")

	register_clcmd("gamemenu", "cmd_menu")
	register_clcmd("say /gm", "cmd_menu")
	register_clcmd("say /menu", "cmd_menu")
	register_clcmd("say /modes", "cmd_menu")
	register_clcmd("say_team /gm", "cmd_menu")

	// Control panel API (sent over RCON by the companion web app)
	register_srvcmd("gm_status", "srv_status")
	register_srvcmd("gm_mode", "srv_mode")
	register_srvcmd("gm_map", "srv_map")
	register_srvcmd("gm_set", "srv_set")
	register_srvcmd("gm_addon", "srv_addon")
	register_srvcmd("gm_addonset", "srv_addonset")
	register_srvcmd("gm_restartmap", "srv_restartmap")
	register_srvcmd("gm_pause", "srv_pause")
	register_srvcmd("gm_do", "srv_do")
	register_srvcmd("gm_announce", "srv_announce")
	register_srvcmd("gm_team", "srv_team")
	register_srvcmd("gm_bots", "srv_bots")
	register_srvcmd("gm_match", "srv_match")
	register_srvcmd("gm_player", "srv_player")
	register_srvcmd("gm_nav", "srv_nav")
	register_srvcmd("gm_preset", "srv_preset")
	register_srvcmd("gm_presetsave", "srv_presetsave")
	register_srvcmd("gm_presetdel", "srv_presetdel")
	register_clcmd("gm_presetname", "cmd_presetname")

	RegisterHam(Ham_Spawn, "player", "fw_spawn_post", 1)
	RegisterHam(Ham_TraceAttack, "player", "fw_traceattack")
	RegisterHam(Ham_TraceAttack, "player", "fw_traceattack_post", 1)
	RegisterHam(Ham_TraceAttack, "worldspawn", "fw_traceattack_post", 1)
	RegisterHam(Ham_TakeDamage, "player", "fw_takedamage_pre")
	RegisterHam(Ham_TakeDamage, "player", "fw_takedamage_post", 1)
	register_forward(FM_PlayerPreThink, "fw_prethink")
	register_forward(FM_SetModel, "fw_setmodel_post", 1)
	register_event("CurWeapon", "ev_curweapon", "be", "1=1")
	set_task(1.5, "task_bot_ammo", _, _, _, "b")   // bots get no CurWeapon messages
	g_HudSync[0] = CreateHudSyncObj()
	g_HudSync[1] = CreateHudSyncObj()
	g_HudNav = CreateHudSyncObj()

	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	get_datadir(g_DataDir, charsmax(g_DataDir))
	format(g_DataDir, charsmax(g_DataDir), "%s/gamemodes", g_DataDir)
	if (!dir_exists(g_DataDir))
		mkdir(g_DataDir)

	// The control panel stores its RCON password here (written by the panel helper).
	new pw[64]
	read_data_line("panel_rcon.txt", pw, charsmax(pw))
	if (pw[0])
		set_cvar_string("rcon_password", pw)

	g_Career = detect_career()
	check_last_session()
	read_data_line("lastpreset.txt", g_LastPreset, charsmax(g_LastPreset))

	load_modes()
	load_current()
	load_saved()
	load_addons()
	load_presets()
	check_map_presets()
}

public plugin_cfg()
{
	if (g_Career)
	{
		career_cleanup()
		return
	}
	if (g_Reloading)
	{
		new map[32]
		get_mapname(map, charsmax(map))
		set_task(0.5, "task_changelevel", 0, map, sizeof map)
		return
	}
	set_task(1.0, "task_apply")
	set_task(4.0, "task_reapply_saved")
	set_task(5.0, "task_learn_guard_start")
	set_task(30.0, "task_edicts", TASK_EDICTS, _, _, "b")
	remember_recent()
	if (navbatch_active())
		set_task(3.0, "task_navbatch")
}

public task_apply()
{
	apply_all(true)
}

// GunGame and Zombie Plague run their own config files around the same time as
// task_apply, so put the mode's settings and the saved ones back once they're done
public task_reapply_saved()
{
	if (g_Career)
		return
	new path[192]
	formatex(path, charsmax(path), "%s/gamemodes/%s.cfg", g_CfgDir, g_ModeId[g_Cur])
	exec_if_exists(path)
	formatex(path, charsmax(path), "%s/global.cfg", g_DataDir)
	exec_if_exists(path)
	formatex(path, charsmax(path), "%s/mode_%s.cfg", g_DataDir, g_ModeId[g_Cur])
	exec_if_exists(path)
	// server name and password from the panel's Friends tab
	formatex(path, charsmax(path), "%s/friends.cfg", g_DataDir)
	exec_if_exists(path)
	server_exec()
	apply_gg_order(false)
	apply_zp_specials()
	apply_nobomb()
	apply_botstyle()
}

// Practice: a small box where each of your bullets landed (red on players), for 4s
show_impact(id, const Float:at[3], bool:player)
{
	message_begin(MSG_ONE_UNRELIABLE, SVC_TEMPENTITY, _, id)
	write_byte(TE_BOX)
	write_coord(floatround(at[0] - 1.5))
	write_coord(floatround(at[1] - 1.5))
	write_coord(floatround(at[2] - 1.5))
	write_coord(floatround(at[0] + 1.5))
	write_coord(floatround(at[1] + 1.5))
	write_coord(floatround(at[2] + 1.5))
	write_short(40)
	if (player) { write_byte(255); write_byte(40);  write_byte(40);  }
	else        { write_byte(60);  write_byte(140); write_byte(255); }
	message_end()
}

// Practice: a trail behind thrown grenades
public fw_setmodel_post(ent, const model[])
{
	if (!get_pcvar_num(g_pNadeTrail) || !pev_valid(ent) || !equal(model, "models/w_", 9))
		return
	new r, g, b
	if (contain(model, "hegrenade") != -1)          { r = 255; g = 60;  b = 40;  }
	else if (contain(model, "flashbang") != -1)     { r = 255; g = 255; b = 255; }
	else if (contain(model, "smokegrenade") != -1)  { r = 80;  g = 255; b = 80;  }
	else
		return
	new classname[16]
	pev(ent, pev_classname, classname, charsmax(classname))
	if (!equal(classname, "grenade"))
		return
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMFOLLOW)
	write_short(ent)
	write_short(g_BeamSpr)
	write_byte(30)   // life (3s)
	write_byte(2)    // width
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(180)  // brightness
	message_end()
}

public plugin_end()
{
	write_data_line("session.txt", "clean")
	if (g_Maps != Invalid_Array)
	{
		ArrayDestroy(g_Maps)
		ArrayDestroy(g_MapNav)
	}
}

// ---------------------------------------------------------------------------
// Tour of Duty detection
// ---------------------------------------------------------------------------

// The Tour of Duty screen sets a random sv_password and forces humans_join_team
// before running "career <map>". Normal New Game / Create Server do neither.
bool:detect_career()
{
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	return pass[0] && (equali(team, "CT") || equali(team, "T"))
}

// Pause the current mode's plugins and undo any per-mode tweaks left in cvars.
career_cleanup()
{
	new path[192], line[128], name[64]
	formatex(path, charsmax(path), "%s/plugins-gamemode.ini", g_CfgDir)
	new f = fopen(path, "rt")
	if (f)
	{
		while (!feof(f))
		{
			fgets(f, line, charsmax(line))
			trim(line)
			if (!line[0] || line[0] == ';')
				continue
			parse(line, name, charsmax(name))
			pause("ac", name)
		}
		fclose(f)
	}
	set_pcvar_num(g_pWeapons, 0)
	set_pcvar_num(g_pBotWeapons, 0)
	set_pcvar_num(g_pInfAmmo, 0)
	set_pcvar_num(g_pHsOnly, 0)
	set_pcvar_string(g_pLoadout, "")
	apply_bot_weapons()
	set_cvar_num("sv_gravity", 800)
	set_cvar_num("sv_airaccelerate", 10)
	log_amx("Tour of Duty match detected - game mode plugins paused, settings left alone.")
}

// ---------------------------------------------------------------------------
// Modes & config files
// ---------------------------------------------------------------------------

load_modes()
{
	new path[192], line[256], id[32], name[48], desc[64], cmd[32], flags[8]
	formatex(path, charsmax(path), "%s/gamemodes/modes.ini", g_CfgDir)

	new f = fopen(path, "rt")
	if (!f)
	{
		log_amx("Missing %s", path)
		copy(g_ModeId[0], charsmax(g_ModeId[]), "classic")
		copy(g_ModeName[0], charsmax(g_ModeName[]), "Classic")
		g_ModeCount = 1
		return
	}

	while (!feof(f) && g_ModeCount < MAX_MODES)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue

		id[0] = name[0] = desc[0] = cmd[0] = flags[0] = 0
		parse(line, id, charsmax(id), name, charsmax(name), desc, charsmax(desc), cmd, charsmax(cmd), flags, charsmax(flags))
		if (!id[0])
			continue

		copy(g_ModeId[g_ModeCount], charsmax(g_ModeId[]), id)
		copy(g_ModeName[g_ModeCount], charsmax(g_ModeName[]), name[0] ? name : id)
		copy(g_ModeDesc[g_ModeCount], charsmax(g_ModeDesc[]), desc)
		copy(g_ModeCmd[g_ModeCount], charsmax(g_ModeCmd[]), cmd)
		copy(g_ModeFlags[g_ModeCount], charsmax(g_ModeFlags[]), flags)
		g_ModeCount++
	}
	fclose(f)
}

find_mode(const id[])
{
	for (new i = 0; i < g_ModeCount; i++)
		if (equali(g_ModeId[i], id))
			return i
	return -1
}

read_data_line(const file[], out[], len)
{
	new path[192]
	formatex(path, charsmax(path), "%s/%s", g_DataDir, file)
	out[0] = 0
	new f = fopen(path, "rt")
	if (f)
	{
		fgets(f, out, len)
		trim(out)
		fclose(f)
	}
}

write_data_line(const file[], const value[])
{
	new path[192]
	formatex(path, charsmax(path), "%s/%s", g_DataDir, file)
	if (!value[0])
	{
		delete_file(path)
		return
	}
	new f = fopen(path, "wt")
	if (f)
	{
		fprintf(f, "%s^n", value)
		fclose(f)
	}
}

load_current()
{
	new id[32]
	read_data_line("current.txt", id, charsmax(id))
	g_Cur = max(find_mode(id), 0)
}

mode_has_flag(mode, flag)
{
	return contain(g_ModeFlags[mode], fmt("%c", flag)) != -1
}

// Writes configs/plugins-gamemode.ini from the mode's plugin list. AMXX loads every
// configs/plugins-*.ini on map load, so this takes effect on the next map change.
write_mode_plugins(mode)
{
	new src[192], dst[192], line[192]
	formatex(src, charsmax(src), "%s/gamemodes/%s.plugins.ini", g_CfgDir, g_ModeId[mode])
	formatex(dst, charsmax(dst), "%s/plugins-gamemode.ini", g_CfgDir)

	new out = fopen(dst, "wt")
	if (!out)
		return
	fprintf(out, "; Generated by gamemodes.amxx - edit configs/gamemodes/%s.plugins.ini instead^n", g_ModeId[mode])

	new in = fopen(src, "rt")
	if (in)
	{
		while (!feof(in))
		{
			fgets(in, line, charsmax(line))
			fputs(out, line)
		}
		fclose(in)
	}
	fclose(out)
	write_data_line("current.txt", g_ModeId[mode])
}

exec_if_exists(const path[])
{
	if (file_exists(path))
		server_cmd("exec ^"%s^"", path)
}

// base.cfg -> <mode>.cfg -> saved global settings -> saved mode settings -> add-on settings
apply_all(bool:restart)
{
	new path[192]
	formatex(path, charsmax(path), "%s/gamemodes/base.cfg", g_CfgDir)
	exec_if_exists(path)
	formatex(path, charsmax(path), "%s/gamemodes/%s.cfg", g_CfgDir, g_ModeId[g_Cur])
	exec_if_exists(path)
	formatex(path, charsmax(path), "%s/global.cfg", g_DataDir)
	exec_if_exists(path)
	formatex(path, charsmax(path), "%s/mode_%s.cfg", g_DataDir, g_ModeId[g_Cur])
	exec_if_exists(path)
	for (new a = 0; a < g_AddonCount; a++)
	{
		if (!g_AddonLoaded[a])
			continue
		formatex(path, charsmax(path), "%s/gamemodes/addons/%s.cfg", g_CfgDir, g_AddonId[a])
		exec_if_exists(path)
		formatex(path, charsmax(path), "%s/addon_%s.cfg", g_DataDir, g_AddonId[a])
		exec_if_exists(path)
	}
	server_exec()

	apply_weapons()
	apply_gg_order(false)
	apply_zp_specials()
	apply_nobomb()
	apply_botstyle()
	if (restart)
		server_cmd("sv_restart 1")
}

// gm_gg_order picks one of these for GunGame's gg_weapon_order
new const GG_ORDERS[][] =
{
	"glock18,usp,p228,deagle,fiveseven,elite,m3,xm1014,tmp,mac10,mp5navy,ump45,p90,galil,famas,ak47,scout,m4a1,sg552,aug,m249,hegrenade,knife",
	"glock18,usp,p228,deagle,fiveseven,elite,tmp,mac10,mp5navy,ump45,p90,galil,famas,ak47,scout,m4a1,sg552,aug,m249,hegrenade,knife",
	"glock18,deagle,mp5navy,p90,m3,famas,ak47,m4a1,awp,hegrenade,knife",
	"m249,aug,sg552,m4a1,ak47,famas,galil,scout,p90,ump45,mp5navy,mac10,tmp,xm1014,m3,elite,fiveseven,deagle,p228,usp,glock18,hegrenade,knife",
	"glock18,usp,p228,fiveseven,elite,deagle,scout,awp,sg550,g3sg1,hegrenade,knife"
}

// GunGame reads the order 6 seconds into the map; mid-map it needs telling
apply_gg_order(bool:now)
{
	if (!get_cvar_pointer("gg_weapon_order"))
		return
	set_cvar_string("gg_weapon_order", GG_ORDERS[clamp(get_pcvar_num(g_pGGOrder), 0, sizeof GG_ORDERS - 1)])
	if (now && callfunc_begin("setup_weapon_order", "gungame.amxx") == 1)
		callfunc_end()
}

// gm_zp_specials sets the 1-in-N chance of every Zombie Plague special round
apply_zp_specials()
{
	if (!get_cvar_pointer("zp_nem_chance"))
		return
	new n = clamp(get_pcvar_num(g_pZPSpecials), 2, 100)
	new const cvars[][] = { "zp_nem_chance", "zp_surv_chance", "zp_swarm_chance", "zp_multi_chance", "zp_plague_chance" }
	for (new i = 0; i < sizeof cvars; i++)
		set_cvar_num(cvars[i], n)
}

saved_file(scope, path[], len)
{
	if (scope == SC_GLOBAL)
		formatex(path, len, "%s/global.cfg", g_DataDir)
	else
		formatex(path, len, "%s/mode_%s.cfg", g_DataDir, g_ModeId[g_Cur])
}

load_saved()
{
	new path[192], line[128], cvar[32], val[40]
	for (new scope = SC_GLOBAL; scope <= SC_MODE; scope++)
	{
		saved_file(scope, path, charsmax(path))
		new f = fopen(path, "rt")
		if (!f)
			continue
		while (!feof(f))
		{
			fgets(f, line, charsmax(line))
			trim(line)
			if (!line[0] || line[0] == '/')
				continue
			parse(line, cvar, charsmax(cvar), val, charsmax(val))
			new s = find_setting(cvar)
			if (s != -1 && SETTINGS[s][SD_Scope] == scope)
			{
				g_SavedHas[s] = true
				copy(g_SavedVal[s], charsmax(g_SavedVal[]), val)
			}
		}
		fclose(f)
	}
}

write_saved(scope)
{
	new path[192]
	saved_file(scope, path, charsmax(path))
	new f = fopen(path, "wt")
	if (!f)
		return
	fprintf(f, "// Saved by the Game Mode Menu^n")
	for (new s = 0; s < sizeof SETTINGS; s++)
		if (g_SavedHas[s] && SETTINGS[s][SD_Scope] == scope)
			fprintf(f, "%s ^"%s^"^n", SETTINGS[s][SD_Cvar], g_SavedVal[s])
	fclose(f)
}

find_setting(const cvar[])
{
	for (new s = 0; s < sizeof SETTINGS; s++)
		if (equali(SETTINGS[s][SD_Cvar], cvar))
			return s
	return -1
}

set_setting(s, const val[])
{
	set_cvar_string(SETTINGS[s][SD_Cvar], val)

	new scope = SETTINGS[s][SD_Scope]
	if (scope != SC_NONE)
	{
		g_SavedHas[s] = true
		copy(g_SavedVal[s], charsmax(g_SavedVal[]), val)
		write_saved(scope)
	}
}

bool:presets_enabled()
{
	new s = find_setting("gm_mappresets")
	return !g_SavedHas[s] || str_to_num(g_SavedVal[s]) != 0
}

public task_save_quota()
{
	new q[8]
	get_cvar_string("bot_quota", q, charsmax(q))
	set_setting(find_setting("bot_quota"), q)
}

// ---------------------------------------------------------------------------
// Map presets: maps listed in mappresets.ini switch to a mode automatically.
// The mode that was active before is restored on the next map without a preset.
// ---------------------------------------------------------------------------

preset_mode_for(const map[])
{
	new path[192], line[128], pattern[64], id[32]
	formatex(path, charsmax(path), "%s/gamemodes/mappresets.ini", g_CfgDir)
	new f = fopen(path, "rt")
	if (!f)
		return -1
	new result = -1
	while (!feof(f) && result == -1)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue
		parse(line, pattern, charsmax(pattern), id, charsmax(id))
		new len = strlen(pattern)
		if (len && pattern[len - 1] == '*')
		{
			if (equali(map, pattern, len - 1))
				result = find_mode(id)
		}
		else if (equali(map, pattern))
			result = find_mode(id)
	}
	fclose(f)
	return result
}

check_map_presets()
{
	new map[32], last[32]
	get_mapname(map, charsmax(map))
	read_data_line("lastmap.txt", last, charsmax(last))
	write_data_line("lastmap.txt", map)

	if (g_Career || navbatch_active() || equali(map, last) || !presets_enabled())
		return

	new premode[32]
	read_data_line("premode.txt", premode, charsmax(premode))

	new target = preset_mode_for(map)
	if (target >= 0)
	{
		if (target == g_Cur)
			return
		if (!premode[0])
			write_data_line("premode.txt", g_ModeId[g_Cur])
		formatex(g_ReloadNote, charsmax(g_ReloadNote), "Map preset: %s", g_ModeName[target])
	}
	else
	{
		if (!premode[0])
			return
		write_data_line("premode.txt", "")
		target = find_mode(premode)
		if (target < 0 || target == g_Cur)
			return
		formatex(g_ReloadNote, charsmax(g_ReloadNote), "Back to %s", g_ModeName[target])
	}

	write_mode_plugins(target)
	g_Cur = target
	g_Reloading = true
	write_data_line("reloadnote.txt", g_ReloadNote)
	log_amx("%s - reloading %s", g_ReloadNote, map)
}

// ---------------------------------------------------------------------------
// Bot navigation (.nav) tools
// ---------------------------------------------------------------------------

bool:has_nav(const map[])
{
	return bool:file_exists(fmt("maps/%s.nav", map), true)
}

bool:navbatch_active()
{
	return bool:file_exists(fmt("%s/navqueue.txt", g_DataDir))
}

// Safety net: when the map is close to the engine's entity limit (a crash with
// "ED_Alloc: no free edicts"), clear dropped weapons the proper way
public task_edicts()
{
	new used = engfunc(EngFunc_NumberOfEntities), most = global_get(glb_maxEntities)
	if (used * 100 < most * 85)
		return
	new ent = MaxClients, cleared
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "weaponbox")) > 0)
	{
		new model[24]
		pev(ent, pev_model, model, charsmax(model))
		if (equal(model, "models/w_backpack.mdl"))   // the bomb
			continue
		set_pev(ent, pev_nextthink, get_gametime() + 0.1)   // its Kill think frees the guns inside too
		cleared++
	}
	log_amx("Entities at %d of %d - cleared %d dropped weapons", used, most, cleared)
}

// Learning a big map can outlast the map timer, and a map change throws the work
// away, so the timer is held at 0 until the .nav file is saved.
public task_learn_guard_start()
{
	new map[32]
	get_mapname(map, charsmax(map))
	if (!g_Career && get_cvar_num("bot_quota") > 0 && !has_nav(map))
		learn_guard_start()
}

learn_guard_start()
{
	if (g_LearnGuard)
		return
	g_LearnGuard = true
	g_LearnTimelimit = get_cvar_float("mp_timelimit")
	set_cvar_float("mp_timelimit", 0.0)
	set_task(5.0, "task_learn_guard", TASK_LEARNGUARD, _, _, "b")
	if (g_LearnTimelimit > 0.0)
		gm_print(0, "Map timer paused while the bots learn this map.")
}

public task_learn_guard()
{
	new map[32]
	get_mapname(map, charsmax(map))
	if (!has_nav(map))
		return
	remove_task(TASK_LEARNGUARD)
	g_LearnGuard = false
	// mp_timelimit counts from map start, so give the full time again from now
	if (g_LearnTimelimit > 0.0)
	{
		set_cvar_float("mp_timelimit", g_LearnTimelimit + get_gametime() / 60.0)
		gm_print(0, "Bots finished learning the map. Map timer restarted: %d minutes.", floatround(g_LearnTimelimit))
	}
}

// CZ has no "generate" command: learning starts when a bot spawns on a map
// without a .nav file. Bots that are already in (and stuck) are re-added.
start_learning()
{
	learn_guard_start()
	if (get_playersnum_ex(GetPlayers_ExcludeHuman))
	{
		server_cmd("bot_kick")
		set_task(1.0, "task_addbot")
	}
	else
		server_cmd("bot_add")
	g_NavPolls = 0
	remove_task(TASK_NAVPOLL)
	set_task(5.0, "task_navpoll", TASK_NAVPOLL, _, _, "b")
}

public task_addbot()
{
	server_cmd("bot_add")
}

// bot_nav_analyze is carried out by a bot, so make sure one is in the game.
start_analyze()
{
	if (get_playersnum_ex(GetPlayers_ExcludeHuman))
		server_cmd("bot_nav_analyze")
	else
	{
		server_cmd("bot_add")
		set_task(3.0, "task_analyze")
	}
}

public task_analyze()
{
	server_cmd("bot_nav_analyze")
}

public task_navpoll()
{
	new map[32]
	get_mapname(map, charsmax(map))
	g_NavPolls++

	if (has_nav(map))
	{
		remove_task(TASK_NAVPOLL)
		gm_print(0, "Bot navigation saved for %s.", map)
		if (navbatch_active())
			set_task(2.0, "task_navbatch")
		return
	}
	if (navbatch_active())
	{
		set_hudmessage(100, 200, 255, -1.0, 0.15, 0, 0.0, 5.5, 0.0, 0.0, -1)
		ShowSyncHudMsg(0, g_HudNav, "Preparing bot navigation: %s^n%d maps left - say /gm to stop", map, navqueue_count())
		if (g_NavPolls > 120) // ~10 minutes without a result: skip this map
		{
			remove_task(TASK_NAVPOLL)
			navqueue_remove(map)
			set_task(1.0, "task_navbatch")
		}
	}
}

navqueue_count()
{
	new path[192], line[64], n
	formatex(path, charsmax(path), "%s/navqueue.txt", g_DataDir)
	new f = fopen(path, "rt")
	if (!f)
		return 0
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (line[0])
			n++
	}
	fclose(f)
	return n
}

// Removes a map from the queue and returns the first remaining map without a .nav file.
navqueue_remove(const map[], next[] = "", nlen = 0)
{
	new path[192], line[64]
	formatex(path, charsmax(path), "%s/navqueue.txt", g_DataDir)
	new Array:keep = ArrayCreate(32)
	new f = fopen(path, "rt")
	if (f)
	{
		while (!feof(f))
		{
			fgets(f, line, charsmax(line))
			trim(line)
			if (line[0] && !equali(line, map) && !has_nav(line))
				ArrayPushString(keep, line)
		}
		fclose(f)
	}
	if (nlen)
		next[0] = 0
	if (!ArraySize(keep))
		delete_file(path)
	else
	{
		f = fopen(path, "wt")
		for (new i = 0; i < ArraySize(keep); i++)
		{
			ArrayGetString(keep, i, line, charsmax(line))
			fprintf(f, "%s^n", line)
		}
		fclose(f)
		if (nlen)
			ArrayGetString(keep, 0, next, nlen)
	}
	ArrayDestroy(keep)
}

public task_navbatch()
{
	if (!navbatch_active())
		return
	new map[32], next[32]
	get_mapname(map, charsmax(map))
	if (!has_nav(map))
	{
		start_learning()
		return
	}
	navqueue_remove(map, next, charsmax(next))
	if (!next[0])
	{
		gm_print(0, "All maps now have bot navigation. Batch finished.")
		return
	}
	gm_print(0, "Next map to prepare: %s (%d left)", next, navqueue_count())
	set_task(3.0, "task_changelevel", 0, next, sizeof next)
}

// A shipped .nav can be broken (bots stuck, or CZ stops with "No areas for this
// zone"). Move it to data/gamemodes/navbackup, reload the map, and learn it again.
rebuild_nav()
{
	new map[32]
	get_mapname(map, charsmax(map))
	rebuild_nav_map(map)
}

rebuild_nav_map(const map[])
{
	new backup[192], path[96]
	formatex(backup, charsmax(backup), "%s/navbackup", g_DataDir)
	if (!dir_exists(backup))
		mkdir(backup)
	format(backup, charsmax(backup), "%s/%s.nav", backup, map)

	new const dirs[][] = { "maps", "../cstrike/maps" }
	for (new i = 0; i < sizeof dirs; i++)
	{
		formatex(path, charsmax(path), "%s/%s.nav", dirs[i], map)
		if (!file_exists(path))
			continue
		if (file_exists(backup))
			delete_file(backup)
		if (!rename_file(path, backup, 1))
			delete_file(path)
	}
	if (has_nav(map))
	{
		gm_print(0, "Couldn't remove the old navigation file for %s.", map)
		return
	}

	new f = fopen(fmt("%s/navqueue.txt", g_DataDir), "wt")
	if (f)
	{
		fprintf(f, "%s^n", map)
		fclose(f)
	}
	gm_print(0, "Rebuilding bot navigation for %s: the map restarts, then bots learn it.", map)
	new data[32]
	copy(data, charsmax(data), map)
	set_task(1.5, "task_changelevel", 0, data, sizeof data)
}

start_navbatch(id)
{
	build_maps()
	new path[192], map[32], count
	formatex(path, charsmax(path), "%s/navqueue.txt", g_DataDir)
	new f = fopen(path, "wt")
	if (!f)
		return
	for (new i = 0; i < ArraySize(g_Maps); i++)
	{
		if (ArrayGetCell(g_MapNav, i))
			continue
		ArrayGetString(g_Maps, i, map, charsmax(map))
		fprintf(f, "%s^n", map)
		count++
	}
	fclose(f)
	if (!count)
	{
		delete_file(path)
		gm_print(id, "Every map already has bot navigation.")
		return
	}
	gm_print(0, "Preparing bot navigation for %d maps. This takes a few minutes per map.", count)
	set_task(1.0, "task_navbatch")
}

// ---------------------------------------------------------------------------
// Add-ons
// ---------------------------------------------------------------------------

// configs/gamemodes/addons.ini:  id "Name" "plugin1.amxx plugin2.amxx" "Description" "options command"
load_addons()
{
	new path[192], line[384], id[32], name[48], plugins[160], desc[64], cmd[32]
	formatex(path, charsmax(path), "%s/gamemodes/addons.ini", g_CfgDir)

	new f = fopen(path, "rt")
	if (!f)
		return
	while (!feof(f) && g_AddonCount < MAX_ADDONS)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue
		id[0] = name[0] = plugins[0] = desc[0] = cmd[0] = 0
		parse(line, id, charsmax(id), name, charsmax(name), plugins, charsmax(plugins), desc, charsmax(desc), cmd, charsmax(cmd))
		if (!id[0] || !plugins[0])
			continue
		copy(g_AddonId[g_AddonCount], charsmax(g_AddonId[]), id)
		copy(g_AddonName[g_AddonCount], charsmax(g_AddonName[]), name[0] ? name : id)
		copy(g_AddonPlugins[g_AddonCount], charsmax(g_AddonPlugins[]), plugins)
		copy(g_AddonDesc[g_AddonCount], charsmax(g_AddonDesc[]), desc)
		copy(g_AddonCmd[g_AddonCount], charsmax(g_AddonCmd[]), cmd)
		g_AddonCount++
	}
	fclose(f)

	formatex(path, charsmax(path), "%s/addons.txt", g_DataDir)
	f = fopen(path, "rt")
	if (!f)
		return
	while (!feof(f))
	{
		fgets(f, id, charsmax(id))
		trim(id)
		for (new i = 0; i < g_AddonCount; i++)
			if (id[0] && equali(id, g_AddonId[i]))
				g_AddonOn[i] = g_AddonLoaded[i] = true
	}
	fclose(f)
}

// Saves enabled add-ons and writes configs/plugins-gmaddons.ini (loaded on next map).
write_addons()
{
	new path[192], name[64], pos
	formatex(path, charsmax(path), "%s/addons.txt", g_DataDir)
	new f = fopen(path, "wt")
	if (f)
	{
		for (new i = 0; i < g_AddonCount; i++)
			if (g_AddonOn[i])
				fprintf(f, "%s^n", g_AddonId[i])
		fclose(f)
	}

	formatex(path, charsmax(path), "%s/plugins-gmaddons.ini", g_CfgDir)
	f = fopen(path, "wt")
	if (!f)
		return
	fprintf(f, "; Generated by gamemodes.amxx - toggle add-ons from the in-game menu^n")
	for (new i = 0; i < g_AddonCount; i++)
	{
		if (!g_AddonOn[i])
			continue
		pos = 0
		while ((pos = argparse(g_AddonPlugins[i], pos, name, charsmax(name))) != -1)
			fprintf(f, "%s^n", name)
	}
	fclose(f)
}

bool:addons_pending()
{
	for (new i = 0; i < g_AddonCount; i++)
		if (g_AddonOn[i] != g_AddonLoaded[i])
			return true
	return false
}

// configs/gamemodes/addons/<id>.ini:  cvar "Label" "value1 value2 ..." "Label1|Label2|..."
addon_ini(a, path[], len)
{
	formatex(path, len, "%s/gamemodes/addons/%s.ini", g_CfgDir, g_AddonId[a])
}

bool:addon_has_options(a)
{
	new path[192]
	addon_ini(a, path, charsmax(path))
	return g_AddonCmd[a][0] || file_exists(path)
}

bool:read_addon_setting(a, n, cvar[], clen, name[], nlen, values[], vlen, labels[], llen)
{
	new path[192], line[256], i
	addon_ini(a, path, charsmax(path))
	new f = fopen(path, "rt")
	if (!f)
		return false
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue
		if (i++ != n)
			continue
		parse(line, cvar, clen, name, nlen, values, vlen, labels, llen)
		fclose(f)
		return true
	}
	fclose(f)
	return false
}

save_addon_settings(a)
{
	new path[192], cvar[32], name[32], values[96], labels[128], cur[64]
	formatex(path, charsmax(path), "%s/addon_%s.cfg", g_DataDir, g_AddonId[a])
	new f = fopen(path, "wt")
	if (!f)
		return
	fprintf(f, "// Saved by the Game Mode Menu^n")
	for (new n = 0; read_addon_setting(a, n, cvar, charsmax(cvar), name, charsmax(name), values, charsmax(values), labels, charsmax(labels)); n++)
	{
		if (!get_cvar_pointer(cvar))
			continue
		get_cvar_string(cvar, cur, charsmax(cur))
		fprintf(f, "%s ^"%s^"^n", cvar, cur)
	}
	fclose(f)
}

// ---------------------------------------------------------------------------
// Weapon presets, bot weapons, loadouts, effects
// ---------------------------------------------------------------------------

apply_weapons()
{
	switch (get_pcvar_num(g_pWeapons))
	{
		case 1: { server_cmd("amx_restrict on");                                     set_pcvar_string(g_pLoadout, "knife");  set_pcvar_num(g_pBotWeapons, 3); }
		case 2: { server_cmd("amx_restrict on;amx_restrict off pistol ammo equip"); set_pcvar_string(g_pLoadout, "");       set_pcvar_num(g_pBotWeapons, 1); }
		case 3: { server_cmd("amx_restrict on;amx_restrict off deagle ammo equip"); set_pcvar_string(g_pLoadout, "deagle"); set_pcvar_num(g_pBotWeapons, 1); }
		case 4: { server_cmd("amx_restrict on;amx_restrict off awp ammo equip");    set_pcvar_string(g_pLoadout, "awp");    set_pcvar_num(g_pBotWeapons, 2); }
		case 5: { server_cmd("amx_restrict on;amx_restrict off scout ammo equip");  set_pcvar_string(g_pLoadout, "scout");  set_pcvar_num(g_pBotWeapons, 2); }
		default:{ server_cmd("amx_restrict off");                                    set_pcvar_string(g_pLoadout, ""); }
	}
	apply_bot_weapons()
}

apply_bot_weapons()
{
	new w = get_pcvar_num(g_pBotWeapons)
	set_cvar_num("bot_pistols_only", w == 1)
	set_cvar_num("bot_snipers_only", w == 2)
	set_cvar_num("bot_knives_only", w == 3)
}

give_full(id, const weapon[])
{
	if (equal(weapon, "nades"))
	{
		give_item(id, "weapon_hegrenade")
		give_item(id, "weapon_flashbang")
		give_item(id, "weapon_flashbang")
		give_item(id, "weapon_smokegrenade")
		return
	}
	if (equal(weapon, "armor"))
	{
		cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM)
		return
	}
	give_item(id, weapon)
	new csw = get_weaponid(weapon)
	if (0 < csw < sizeof MAXBP && MAXBP[csw])
		cs_set_user_bpammo(id, csw, MAXBP[csw])
}

public fw_spawn_post(id)
{
	if (!is_user_alive(id))
		return
	if (g_God[id])
		set_user_godmode(id, 1)
	if (g_Noclip[id])
		set_user_noclip(id, 1)
	new loadout[64]
	get_pcvar_string(g_pLoadout, loadout, charsmax(loadout))
	if (loadout[0])
		set_task(0.2, "task_loadout", id + TASK_LOADOUT)
	if (get_pcvar_num(g_pNoBomb))
		set_task(0.3, "task_nobomb", id + TASK_NOBOMB)
}

public task_nobomb(taskid)
{
	strip_c4(taskid - TASK_NOBOMB)
}

// gm_nobomb: nobody carries the bomb, so it can't be planted
strip_c4(id)
{
	if (!is_user_alive(id) || !user_has_weapon(id, CSW_C4))
		return
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "weapon_c4")) > 0)
	{
		if (pev(ent, pev_owner) != id)
			continue
		if (held_weapon(id) == CSW_C4)
			engclient_cmd(id, "weapon_knife")
		if (ExecuteHamB(Ham_RemovePlayerItem, id, ent))
		{
			ExecuteHamB(Ham_Item_Kill, ent)
			set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << CSW_C4))
		}
		break
	}
	cs_set_user_plant(id, 0, 0)
}

// Each bot style (gm_humanbots.amxx) has its own personality file. CZ reads
// bot_profile_db when a map loads, so the personalities follow on the next map.
new const BOTSTYLE_FILES[][] = { "", "BotProfile_Pub.db", "BotProfile_Matchmaking.db", "BotProfile_Veterans.db", "BotProfile_Newbie.db",
	"BotProfile_LockedIn.db", "BotProfile_Hackers.db", "BotProfile_Gods.db" }

apply_botstyle()
{
	if (!get_cvar_pointer("gm_botstyle") || g_Career)
		return
	new st = get_cvar_num("gm_botstyle")
	if (0 < st < sizeof BOTSTYLE_FILES && file_exists(BOTSTYLE_FILES[st], true))
		set_cvar_string("bot_profile_db", BOTSTYLE_FILES[st])
}

// ReGameDLL can stop the bomb being handed out at all; strip_c4 covers the rest
apply_nobomb()
{
	if (get_cvar_pointer("mp_give_player_c4"))
		set_cvar_num("mp_give_player_c4", get_pcvar_num(g_pNoBomb) ? 0 : 1)
}

strip_c4_all()
{
	for (new id = 1; id <= MaxClients; id++)
		strip_c4(id)
}

public task_loadout(taskid)
{
	new id = taskid - TASK_LOADOUT
	if (!is_user_alive(id))
		return

	new loadout[64], name[24], weapon[32]
	get_pcvar_string(g_pLoadout, loadout, charsmax(loadout))
	if (!loadout[0])
		return

	new bool:hadC4 = bool:user_has_weapon(id, CSW_C4)
	strip_user_weapons(id)
	give_item(id, "weapon_knife")

	new pos
	while ((pos = argparse(loadout, pos, name, charsmax(name))) != -1)
	{
		if (equali(name, "knife"))
			continue
		formatex(weapon, charsmax(weapon), "weapon_%s", name)
		give_full(id, weapon)
	}

	if (hadC4)
	{
		give_item(id, "weapon_c4")
		cs_set_user_plant(id, 1, 1)
	}
}

// The weapon a player holds. get_user_weapon is stale for bots (they get no
// CurWeapon messages), so read the active item instead.
held_weapon(id)
{
	new item = get_pdata_cbase(id, 373)
	return pev_valid(item) ? cs_get_weapon_id(item) : 0
}

// Unlimited ammo for bots, which ev_curweapon never sees
public task_bot_ammo()
{
	if (!get_pcvar_num(g_pInfAmmo))
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id) || !is_user_bot(id))
			continue
		new csw = held_weapon(id)
		if (0 < csw < sizeof MAXBP && MAXBP[csw] && cs_get_user_bpammo(id, csw) < MAXBP[csw])
			cs_set_user_bpammo(id, csw, MAXBP[csw])
	}
}

public ev_curweapon(id)
{
	if (!get_pcvar_num(g_pInfAmmo))
		return
	new csw = read_data(2)
	if (0 < csw < sizeof MAXBP && MAXBP[csw] && cs_get_user_bpammo(id, csw) < MAXBP[csw])
		cs_set_user_bpammo(id, csw, MAXBP[csw])
}

public fw_traceattack(victim, attacker, Float:damage, Float:dir[3], tr, bits)
{
	if (!get_pcvar_num(g_pHsOnly) || !is_user_connected(attacker) || attacker == victim)
		return HAM_IGNORED
	if (held_weapon(attacker) == CSW_KNIFE)
		return HAM_IGNORED
	if (get_tr2(tr, TR_iHitgroup) != HIT_HEAD)
		return HAM_SUPERCEDE
	return HAM_IGNORED
}

public fw_traceattack_post(victim, attacker, Float:damage, Float:dir[3], tr, bits)
{
	new mode = get_pcvar_num(g_pTracers), impacts = get_pcvar_num(g_pImpacts)
	if ((!mode && !impacts) || !(bits & DMG_BULLET) || !(1 <= attacker <= MaxClients) || !is_user_alive(attacker))
		return
	if (held_weapon(attacker) == CSW_KNIFE)
		return

	new Float:end[3], start[3]
	get_tr2(tr, TR_vecEndPos, end)
	if (impacts && !is_user_bot(attacker))
		show_impact(attacker, end, victim > MaxClients || victim < 1 ? false : true)
	if (!mode)
		return
	get_user_origin(attacker, start, 1)
	start[2] -= 6

	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	if (mode == 1)
	{
		write_byte(TE_TRACER)
		write_coord(start[0])
		write_coord(start[1])
		write_coord(start[2])
		write_coord(floatround(end[0]))
		write_coord(floatround(end[1]))
		write_coord(floatround(end[2]))
	}
	else
	{
		write_byte(TE_BEAMPOINTS)
		write_coord(start[0])
		write_coord(start[1])
		write_coord(start[2])
		write_coord(floatround(end[0]))
		write_coord(floatround(end[1]))
		write_coord(floatround(end[2]))
		write_short(g_BeamSpr)
		write_byte(0)    // start frame
		write_byte(0)    // framerate
		write_byte(1)    // life (0.1s)
		write_byte(4)    // width
		write_byte(0)    // noise
		if (get_user_team(attacker) == 1) { write_byte(255); write_byte(120); write_byte(40); }
		else                              { write_byte(60);  write_byte(160); write_byte(255); }
		write_byte(200)  // brightness
		write_byte(0)    // scroll
	}
	message_end()
}

public fw_takedamage_pre(victim, inflictor, attacker, Float:damage, bits)
{
	pev(victim, pev_health, g_PreHealth[victim])
}

public fw_takedamage_post(victim, inflictor, attacker, Float:damage, bits)
{
	if (!get_pcvar_num(g_pDmgNumbers) || !(1 <= attacker <= MaxClients) || attacker == victim)
		return
	if (!is_user_connected(attacker) || is_user_bot(attacker))
		return

	new Float:hp
	pev(victim, pev_health, hp)
	new dealt = floatround(g_PreHealth[victim] - floatmax(hp, 0.0))
	if (dealt <= 0)
		return

	g_HudNext = !g_HudNext
	if (hp <= 0.0)
		set_hudmessage(255, 40, 40, 0.53, g_HudNext ? 0.44 : 0.48, 0, 0.0, 1.0, 0.0, 0.3, -1)
	else
		set_hudmessage(255, 200, 40, 0.53, g_HudNext ? 0.44 : 0.48, 0, 0.0, 1.0, 0.0, 0.3, -1)
	ShowSyncHudMsg(attacker, g_HudSync[g_HudNext], "%d", dealt)
}

public fw_prethink(id)
{
	if (!get_pcvar_num(g_pBhop) || !is_user_alive(id))
		return FMRES_IGNORED

	set_pev(id, pev_fuser2, 0.0) // no slowdown after landing
	if (!(pev(id, pev_button) & IN_JUMP))
		return FMRES_IGNORED
	if (!(pev(id, pev_flags) & FL_ONGROUND) || pev(id, pev_waterlevel) >= 2 || pev(id, pev_movetype) == MOVETYPE_FLY)
		return FMRES_IGNORED

	new Float:vel[3]
	pev(id, pev_velocity, vel)
	vel[2] = 250.0
	set_pev(id, pev_velocity, vel)
	set_pev(id, pev_gaitsequence, 6)
	return FMRES_IGNORED
}

public client_putinserver(id)
{
	g_God[id] = g_Noclip[id] = false
	if (!is_user_bot(id))
		set_task(8.0, "task_hint", id + TASK_HINT)
	else if (!g_BotHams && get_cvar_pointer("bot_quota"))
		set_task(0.1, "task_bot_hams", id)
}

// Who joined, for the app's Players list (the app folds seen.log into its own list).
public client_authorized(id)
{
	if (is_user_bot(id) || is_user_hltv(id))
		return
	new auth[40], name[32]
	get_user_authid(id, auth, charsmax(auth))
	// real SteamIDs only: not STEAM_ID_LAN / STEAM_ID_PENDING / BOT
	if (!equal(auth, "STEAM_", 6) || equal(auth, "STEAM_ID", 8))
		return
	get_user_name(id, name, charsmax(name))
	new f = fopen(fmt("%s/seen.log", g_DataDir), "at")
	if (f)
	{
		fprintf(f, "%d^t%s^t%s^n", get_systime(), auth, name)
		fclose(f)
	}
}

// Condition Zero bots are a different class from human players, so hooks
// registered on "player" never fire for them. Register them again on the first
// bot that joins.
public task_bot_hams(id)
{
	if (g_BotHams || !is_user_connected(id))
		return
	g_BotHams = true
	RegisterHamFromEntity(Ham_Killed, id, "fw_killed_post", 1)
	RegisterHamFromEntity(Ham_Spawn, id, "fw_spawn_post", 1)
	RegisterHamFromEntity(Ham_TraceAttack, id, "fw_traceattack")
	RegisterHamFromEntity(Ham_TraceAttack, id, "fw_traceattack_post", 1)
	RegisterHamFromEntity(Ham_TakeDamage, id, "fw_takedamage_pre")
	RegisterHamFromEntity(Ham_TakeDamage, id, "fw_takedamage_post", 1)
}

public client_disconnected(id)
{
	// per-player tasks must not carry over to whoever takes the slot next
	remove_task(id + TASK_HINT)
	remove_task(id + TASK_LOADOUT)
	remove_task(id + TASK_NOBOMB)
	remove_task(id + TASK_SELFRESPAWN)
	remove_task(id + TASK_MONEY)
}

public task_hint(taskid)
{
	new id = taskid - TASK_HINT
	if (!is_user_connected(id))
		return
	if (g_Career)
	{
		gm_print(id, "Tour of Duty match - game modes are paused. F3 still opens effects and add-ons.")
		return
	}
	new note[96]
	read_data_line("reloadnote.txt", note, charsmax(note))
	if (note[0])
	{
		write_data_line("reloadnote.txt", "")
		gm_print(id, "%s.", note)
	}
	if (g_CrashMap[0])
		gm_print(id, "Last session ended unexpectedly on %s. F3 shows what you can do about it.", g_CrashMap)
	gm_print(id, "Now playing %s. F3 opens the menu, F4 quick actions.", g_ModeName[g_Cur])
}

// Green "[Game Mode]" prefix in chat. Also echoed to the server console so the
// control panel sees the result of the commands it sends.
gm_print(id, const msg[], any:...)
{
	new text[192]
	vformat(text, charsmax(text), msg, 3)
	if (id == 0 || is_user_connected(id))
		client_print_color(id, print_team_default, "^4[Game Mode]^1 %s", text)
	if (id == 0)
	{
		server_print("[Game Mode] %s", text)
		new esc[200]
		json_escape(text, esc, charsmax(esc))
		add_event(fmt("^"kind^":^"info^",^"text^":^"%s^"", esc))
	}
}

// ---------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------

nth_token(const str[], n, out[], len)
{
	new pos, i
	out[0] = 0
	while ((pos = argparse(str, pos, out, len)) != -1)
	{
		if (i++ == n)
			return true
	}
	out[0] = 0
	return false
}

// Index of cur in a space-separated value list, or -1.
value_index(const values[], const cur[])
{
	new val[40], pos, i
	while ((pos = argparse(values, pos, val, charsmax(val))) != -1)
	{
		if (equali(cur, val) || (isdigit(val[0]) && isdigit(cur[0]) && floatabs(str_to_float(cur) - str_to_float(val)) < 0.01))
			return i
		i++
	}
	return -1
}

count_tokens(const values[])
{
	new val[40], pos, n
	while ((pos = argparse(values, pos, val, charsmax(val))) != -1)
		n++
	return n
}

next_value(const values[], const cur[], out[], len)
{
	new n = count_tokens(values)
	if (n)
		nth_token(values, (value_index(values, cur) + 1) % n, out, len)
}

// Labels are separated by '|' if present, otherwise by spaces. Falls back to the raw value.
value_label(const values[], const labels[], const cur[], out[], len)
{
	new n = value_index(values, cur)
	copy(out, len, cur)
	if (n == -1 || !labels[0])
		return
	if (contain(labels, "|") == -1)
	{
		nth_token(labels, n, out, len)
		return
	}
	new start, i
	for (new pos = 0; ; pos++)
	{
		if (labels[pos] == '|' || !labels[pos])
		{
			if (i++ == n)
			{
				copy(out, min(len, pos - start), labels[start])
				return
			}
			if (!labels[pos])
				return
			start = pos + 1
		}
	}
}

// Submenus: 8/9 page through the list, 0 goes back to the parent menu.
menu_nav(menu)
{
	menu_setprop(menu, MPROP_BACKNAME, "Previous page")
	menu_setprop(menu, MPROP_NEXTNAME, "Next page")
	menu_setprop(menu, MPROP_EXITNAME, "Back")
}

get_info(menu, item, info[], len)
{
	new dummy
	menu_item_getinfo(menu, item, dummy, info, len, _, _, dummy)
}

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

bool:has_access(id)
{
	if (get_user_flags(id) & ACCESS)
		return true
	gm_print(id, "You need admin access to use this menu.")
	return false
}

public cmd_menu(id)
{
	if (has_access(id))
		show_main(id)
	return PLUGIN_HANDLED
}

show_main(id)
{
	new title[160], map[32]
	get_mapname(map, charsmax(map))
	if (g_Career)
		formatex(title, charsmax(title), "\yGame Mode Menu^n\wTour of Duty: \r%s^n\dGame modes are paused during missions", map)
	else
		formatex(title, charsmax(title), "\yGame Mode Menu^n\wMode: \r%s\w   Map: \r%s%s", g_ModeName[g_Cur], map, has_nav(map) ? "" : " \d(no bot nav)")

	new const diff[][] = { "Easy", "Normal", "Hard", "Expert" }
	new menu = menu_create(title, "main_handler")
	if (g_CrashMap[0] && !g_Career)
		menu_additem(menu, fmt("\rLast session crashed on %s", g_CrashMap), "crash")
	if (!g_Career)
	{
		if (g_PresetCount)
			menu_additem(menu, "Presets", "presets")
		menu_additem(menu, fmt("Game mode: \y%s", g_ModeName[g_Cur]), "mode")
		menu_additem(menu, navbatch_active() ? fmt("Maps: \rnav batch: %d left", navqueue_count()) : "Maps", "maps")
		menu_additem(menu, fmt("Bots: \y%d \d| \y%s", get_playersnum_ex(GetPlayers_ExcludeHuman), diff[clamp(get_cvar_num("bot_difficulty"), 0, 3)]), "bots")
		menu_additem(menu, "Match settings", "match")
	}
	menu_additem(menu, "Fun & effects", "fun")
	menu_additem(menu, "Player tools", "players")
	if (g_AddonCount)
	{
		new on
		for (new a = 0; a < g_AddonCount; a++)
			if (g_AddonOn[a])
				on++
		menu_additem(menu, addons_pending() ? "Add-ons: \rrestart map to apply" : fmt("Add-ons: \y%d on", on), "addons")
	}
	menu_additem(menu, "Admin tools", "admin")
	if (g_Career)
		menu_additem(menu, "Help", "help")
	menu_setprop(menu, MPROP_PERPAGE, 0)
	menu_setprop(menu, MPROP_EXITNAME, "Close")
	menu_display(id, menu)
}

public main_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "crash"))        show_crash(id)
	else if (equal(info, "presets")) show_presets(id)
	else if (equal(info, "mode"))    show_modes(id)
	else if (equal(info, "maps"))
	{
		g_PickMode[id] = -1
		show_map_root(id)
	}
	else if (equal(info, "bots"))    show_settings(id, CAT_BOT, 0)
	else if (equal(info, "match"))   show_settings(id, CAT_MATCH, 0)
	else if (equal(info, "fun"))     show_settings(id, CAT_FUN, 0)
	else if (equal(info, "players")) show_players(id)
	else if (equal(info, "addons"))  show_addons(id, 0)
	else if (equal(info, "admin"))   show_admin(id, 0)
	else if (equal(info, "help"))
	{
		new path[192]
		formatex(path, charsmax(path), "%s/gamemodes/help.txt", g_CfgDir)
		show_motd(id, path, "Game Mode Menu - Help")
	}
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Game mode menu
// ---------------------------------------------------------------------------

show_modes(id)
{
	new menu = menu_create("\yGame Mode Menu \d> \yGame mode^n\dThe map reloads to switch", "modes_handler")
	new text[128], info[8]
	if (g_ModeCmd[g_Cur][0])
		menu_additem(menu, fmt("\y%s options", g_ModeName[g_Cur]), "opt")
	for (new i = 0; i < g_ModeCount; i++)
	{
		if (i == g_Cur)
			formatex(text, charsmax(text), "%s \y(current)", g_ModeName[i])
		else if (g_ModeDesc[i][0])
			formatex(text, charsmax(text), "%s \d- %s", g_ModeName[i], g_ModeDesc[i])
		else
			copy(text, charsmax(text), g_ModeName[i])
		num_to_str(i, info, charsmax(info))
		menu_additem(menu, text, info)
	}
	menu_nav(menu)
	menu_display(id, menu)
}

public modes_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[8]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "opt"))
	{
		client_cmd(id, g_ModeCmd[g_Cur])
		return PLUGIN_HANDLED
	}
	g_PickMode[id] = str_to_num(info)
	show_map_root(id)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Maps
// ---------------------------------------------------------------------------

map_category(const map[])
{
	if (equali(map, "de_", 3)) return MC_DE
	if (equali(map, "cs_", 3)) return MC_CS
	if (equali(map, "as_", 3) || equali(map, "es_", 3)) return MC_OBJ
	if (equali(map, "surf", 4)) return MC_SURF
	new const fun[][] = { "fy_", "aim_", "awp_", "ka_", "35hp", "scout", "he_", "gg_", "zm_", "dm_", "mg_", "fun_", "knife", "arena", "pool_", "deathrun", "jail", "bhop", "kz_" }
	for (new i = 0; i < sizeof fun; i++)
		if (equali(map, fun[i], strlen(fun[i])))
			return MC_FUN
	return MC_OTHER
}

add_map(Trie:seen, const map[])
{
	if (TrieKeyExists(seen, map) || !is_map_valid(map))
		return
	TrieSetCell(seen, map, 1)
	if (list_has("hidden_maps.txt", map))
		return
	ArrayPushString(g_Maps, map)
}

add_maps_from_dir(Trie:seen, const dir[])
{
	new file[64], FileType:type
	new h = open_dir(dir, file, charsmax(file), type)
	if (!h)
		return
	do
	{
		new len = strlen(file)
		if (len > 4 && equali(file[len - 4], ".bsp"))
		{
			file[len - 4] = 0
			add_map(seen, file)
		}
	}
	while (next_file(h, file, charsmax(file), type))
	close_dir(h)
}

public sort_maps(Array:array, const a[], const b[])
{
	return strcmp(a, b, true)
}

// Builds the list of all playable maps (czero + cstrike) with their nav status.
build_maps()
{
	if (g_Maps != Invalid_Array)
	{
		ArrayDestroy(g_Maps)
		ArrayDestroy(g_MapNav)
	}
	g_Maps = ArrayCreate(32)
	g_MapNav = ArrayCreate(1)
	new Trie:seen = TrieCreate()
	add_maps_from_dir(seen, "maps")
	add_maps_from_dir(seen, "../cstrike/maps")
	TrieDestroy(seen)
	ArraySortEx(g_Maps, "sort_maps")

	new map[32]
	for (new i = 0; i < ArraySize(g_Maps); i++)
	{
		ArrayGetString(g_Maps, i, map, charsmax(map))
		ArrayPushCell(g_MapNav, has_nav(map))
	}
}

show_map_root(id)
{
	build_maps()
	new mode = g_PickMode[id] >= 0 ? g_PickMode[id] : g_Cur
	new title[128], cur[32], path[192]
	get_mapname(cur, charsmax(cur))
	if (g_PickMode[id] >= 0)
		formatex(title, charsmax(title), "\yStart \r%s\y on which map?", g_ModeName[mode])
	else
		formatex(title, charsmax(title), "\yGame Mode Menu \d> \yMaps^n\d%d maps installed", ArraySize(g_Maps))

	new menu = menu_create(title, "maproot_handler")
	menu_additem(menu, fmt("%s \d(%s)", g_PickMode[id] >= 0 ? "Current map" : "Restart map", cur), "cur")
	menu_additem(menu, "Random map", "rnd")
	new favs = list_count("favorites.txt"), hidden = list_count("hidden_maps.txt")
	if (favs)
		menu_additem(menu, fmt("Favourites \d(%d)", favs), "c-3")
	if (list_count("recent.txt") > 1)
		menu_additem(menu, "Recently played", "c-4")

	formatex(path, charsmax(path), "%s/gamemodes/maps/%s.txt", g_CfgDir, g_ModeId[mode])
	if (file_exists(path))
		menu_additem(menu, fmt("%s maps", g_ModeName[mode]), "list")

	new counts[MC_COUNT], map[32]
	for (new i = 0; i < ArraySize(g_Maps); i++)
	{
		ArrayGetString(g_Maps, i, map, charsmax(map))
		counts[map_category(map)]++
	}
	for (new c = 0; c < MC_COUNT; c++)
		if (counts[c])
			menu_additem(menu, fmt("%s \d(%d)", MAPCAT_NAMES[c], counts[c]), fmt("c%d", c))
	menu_additem(menu, fmt("All maps \d(%d)", ArraySize(g_Maps)), fmt("c%d", MC_COUNT))

	if (g_PickMode[id] < 0)
	{
		menu_additem(menu, list_has("favorites.txt", cur) ? fmt("Remove %s from favourites", cur) : fmt("Add %s to favourites", cur), "fav")
		if (hidden)
			menu_additem(menu, fmt("Hidden maps \d(%d)", hidden), "c-5")
		menu_additem(menu, "Bot navigation tools", "nav")
		menu_additem(menu, fmt("Map presets: %s \d(surf, scoutz... pick their mode)", presets_enabled() ? "\yOn" : "\rOff"), "preset")
	}
	menu_nav(menu)
	menu_display(id, menu)
}

public maproot_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		if (g_PickMode[id] >= 0) show_modes(id)
		else show_main(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "cur"))
	{
		new map[32]
		get_mapname(map, charsmax(map))
		go_map(id, map)
	}
	else if (equal(info, "rnd"))
	{
		new map[32], cur[32]
		get_mapname(cur, charsmax(cur))
		new n = ArraySize(g_Maps)
		for (new tries = 0; tries < 20 && n > 1; tries++)
		{
			ArrayGetString(g_Maps, random(n), map, charsmax(map))
			if (!equali(map, cur))
				break
		}
		go_map(id, map)
	}
	else if (equal(info, "list"))
	{
		g_PickCat[id] = -1
		show_map_list(id, 0)
	}
	else if (info[0] == 'c')
	{
		g_PickCat[id] = str_to_num(info[1])
		if (g_PickCat[id] < 0)
		{
			show_map_list(id, 0)
			return PLUGIN_HANDLED
		}
		show_map_list(id, 0)
	}
	else if (equal(info, "nav"))
		show_nav(id)
	else if (equal(info, "fav"))
	{
		new cur[32]
		get_mapname(cur, charsmax(cur))
		list_set("favorites.txt", cur, !list_has("favorites.txt", cur))
		show_map_root(id)
	}
	else if (equal(info, "preset"))
	{
		set_setting(find_setting("gm_mappresets"), presets_enabled() ? "0" : "1")
		show_map_root(id)
	}
	return PLUGIN_HANDLED
}

show_map_list(id, page)
{
	new mode = g_PickMode[id] >= 0 ? g_PickMode[id] : g_Cur
	new cat = g_PickCat[id]
	new title[96], map[32], cur[32], text[96]
	get_mapname(cur, charsmax(cur))

	if (cat <= -3)
	{
		new const titles[][] = { "\yFavourites", "\yRecently played", "\yHidden maps^n\dPick one to show it again" }
		new const files[][] = { "favorites.txt", "recent.txt", "hidden_maps.txt" }
		new menu = menu_create(titles[-cat - 3], "maplist_handler")
		new Array:list = ArrayCreate(32)
		list_read(files[-cat - 3], list)
		for (new i = 0; i < ArraySize(list); i++)
		{
			ArrayGetString(list, i, map, charsmax(map))
			if (cat == -4 && equali(map, cur))
				continue
			if (cat == -5 || is_map_valid(map))
				menu_additem(menu, fmt("%s%s", map, (cat == -5 || has_nav(map)) ? "" : " \r*"), map)
		}
		ArrayDestroy(list)
		menu_nav(menu)
		menu_display(id, menu, page)
		return
	}
	if (cat == -1)
		formatex(title, charsmax(title), "\y%s maps", g_ModeName[mode])
	else
		formatex(title, charsmax(title), "\y%s^n\d\r*\d = no bot navigation yet", cat == MC_COUNT ? "All maps" : MAPCAT_NAMES[cat])
	new menu = menu_create(title, "maplist_handler")

	if (cat == -1)
	{
		new path[192], line[64]
		formatex(path, charsmax(path), "%s/gamemodes/maps/%s.txt", g_CfgDir, g_ModeId[mode])
		new f = fopen(path, "rt")
		if (f)
		{
			while (!feof(f))
			{
				fgets(f, line, charsmax(line))
				trim(line)
				if (!line[0] || line[0] == ';' || line[0] == '/')
					continue
				parse(line, map, charsmax(map))
				if (is_map_valid(map))
					menu_additem(menu, fmt("%s%s", map, has_nav(map) ? "" : " \r*"), map)
			}
			fclose(f)
		}
	}
	else
	{
		for (new i = 0; i < ArraySize(g_Maps); i++)
		{
			ArrayGetString(g_Maps, i, map, charsmax(map))
			if (cat != MC_COUNT && map_category(map) != cat)
				continue
			formatex(text, charsmax(text), "%s%s%s", map, ArrayGetCell(g_MapNav, i) ? "" : " \r*", equali(map, cur) ? " \y(current)" : "")
			menu_additem(menu, text, map)
		}
	}
	menu_nav(menu)
	menu_display(id, menu, page)
}

public maplist_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_map_root(id)
		return PLUGIN_HANDLED
	}
	new map[32]
	get_info(menu, item, map, charsmax(map))
	menu_destroy(menu)
	if (g_PickCat[id] == -5)
	{
		list_set("hidden_maps.txt", map, false)
		gm_print(id, "%s is back in the map list.", map)
		show_map_root(id)
		return PLUGIN_HANDLED
	}
	go_map(id, map)
	return PLUGIN_HANDLED
}

go_map(id, const map[])
{
	do_go(g_PickMode[id], map)
}

// mode -1 keeps the current mode (map presets may still apply on load)
do_go(mode, const map[])
{
	if (mode >= 0)
	{
		// an explicit choice wins over map presets
		write_mode_plugins(mode)
		write_data_line("lastmap.txt", map)
		write_data_line("premode.txt", "")
		gm_print(0, "Switching to %s on %s...", g_ModeName[mode], map)
	}
	else
		gm_print(0, "Changing map to %s...", map)
	new data[32]
	copy(data, charsmax(data), map)
	set_task(1.5, "task_changelevel", 0, data, sizeof data)
}

// One map change at a time: the engine's own changelevel ignores a second one for the
// same map load (a raw "changelevel" command queued behind another could run while the
// next map is still starting)
public task_changelevel(const map[])
{
	engine_changelevel(map)
}

show_nav(id)
{
	new map[32], title[192]
	get_mapname(map, charsmax(map))
	build_maps()
	new missing
	for (new i = 0; i < ArraySize(g_Maps); i++)
		if (!ArrayGetCell(g_MapNav, i))
			missing++

	formatex(title, charsmax(title), "\yMaps \d> \yBot navigation^n\wThis map: %s^n\d%d of %d maps still need navigation", has_nav(map) ? "\yready" : "\rmissing - bots will stand still", missing, ArraySize(g_Maps))
	new menu = menu_create(title, "nav_handler")
	if (!has_nav(map))
		menu_additem(menu, "Learn this map now \d(builds bot navigation, a few minutes)", "learn")
	menu_additem(menu, "Analyze this map \d(bot_nav_analyze: hiding & approach spots)", "analyze")
	if (has_nav(map))
		menu_additem(menu, "Rebuild navigation \d(bots act strangely, or the game crashed here)", "rebuild")
	if (navbatch_active())
		menu_additem(menu, fmt("\rStop preparing all maps \d(%d left)", navqueue_count()), "stop")
	else if (missing)
		menu_additem(menu, fmt("Prepare all %d maps \d(leave it running)", missing), "batch")
	menu_additem(menu, "Show maps without navigation", "list")
	menu_nav(menu)
	menu_display(id, menu)
}

public nav_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_map_root(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "list"))
		show_missing_nav(id, 0)
	else
	{
		do_nav(id, info)
		if (equal(info, "stop"))
			show_nav(id)
	}
	return PLUGIN_HANDLED
}

// learn | analyze | batch | stop
do_nav(id, const action[])
{
	if (equal(action, "learn"))
	{
		gm_print(0, "Learning this map. The game may pause while bots map it out, then the map restarts.")
		start_learning()
	}
	else if (equal(action, "analyze"))
	{
		new map[32]
		get_mapname(map, charsmax(map))
		if (has_nav(map))
		{
			start_analyze()
			gm_print(0, "Analyzing bot navigation for this map...")
		}
		else
		{
			gm_print(0, "This map has no navigation yet - learning it first (analysis runs automatically after).")
			start_learning()
		}
	}
	else if (equal(action, "rebuild"))
		rebuild_nav()
	else if (equal(action, "batch"))
		start_navbatch(id)
	else if (equal(action, "stop"))
	{
		delete_file(fmt("%s/navqueue.txt", g_DataDir))
		remove_task(TASK_NAVPOLL)
		gm_print(0, "Stopped preparing maps.")
	}
}

show_missing_nav(id, page)
{
	new menu = menu_create("\yMaps without bot navigation^n\dPick one to load it and learn it", "missingnav_handler")
	new map[32]
	for (new i = 0; i < ArraySize(g_Maps); i++)
	{
		if (ArrayGetCell(g_MapNav, i))
			continue
		ArrayGetString(g_Maps, i, map, charsmax(map))
		menu_additem(menu, map, map)
	}
	menu_nav(menu)
	menu_display(id, menu, page)
}

public missingnav_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_nav(id)
		return PLUGIN_HANDLED
	}
	new map[32], path[192]
	get_info(menu, item, map, charsmax(map))
	menu_destroy(menu)

	// a one-map batch: load it, learn it, then stop
	formatex(path, charsmax(path), "%s/navqueue.txt", g_DataDir)
	new f = fopen(path, "wt")
	if (f)
	{
		fprintf(f, "%s^n", map)
		fclose(f)
	}
	g_PickMode[id] = -1
	go_map(id, map)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Settings menus (bots / match / fun)
// ---------------------------------------------------------------------------

setting_visible(s, cat)
{
	if (SETTINGS[s][SD_Cat] != cat)
		return false
	// settings that belong to a mode's plugin only show while it is loaded
	if (!get_cvar_pointer(SETTINGS[s][SD_Cvar]))
		return false
	if (g_Career && SETTINGS[s][SD_Scope] != SC_GLOBAL)
		return false
	// the style's own personalities replace the roster; its switches only matter with a style on
	if (get_cvar_pointer("gm_botstyle"))
	{
		new bool:styled = get_cvar_num("gm_botstyle") != 0
		if (equal(SETTINGS[s][SD_Cvar], "bot_profile_db") && styled)
			return false
		if (equal(SETTINGS[s][SD_Cvar], "gm_hb_", 6) && !styled)
			return false
		// only Locked In (5), Hackers (6) and Gods (7) hop
		if (equal(SETTINGS[s][SD_Cvar], "gm_hb_bhop") && get_cvar_num("gm_botstyle") < 5)
			return false
	}
	// GunGame has its own objectives switch, and deathmatch removes them
	if (equal(SETTINGS[s][SD_Cvar], "gm_nobomb") && (get_cvar_pointer("gg_block_objectives") || get_cvar_pointer("gmdm_guns")))
		return false
	if (SETTINGS[s][SD_NeedFlag][0] && !mode_has_flag(g_Cur, SETTINGS[s][SD_NeedFlag][0]))
		return false
	return true
}

show_settings(id, cat, page)
{
	new const titles[][] = { "", "\yGame Mode Menu \d> \yBots", "\yGame Mode Menu \d> \yMatch settings", "\yGame Mode Menu \d> \yFun & effects" }
	new title[96]
	formatex(title, charsmax(title), "%s^n\dSaved for %s", titles[cat], g_Career ? "all modes" : g_ModeName[g_Cur])
	new menu = menu_create(title, "settings_handler")
	new text[96], info[16], label[40], cur[40]

	if (cat == CAT_BOT)
	{
		menu_additem(menu, fmt("Bot count: \y%d", get_playersnum_ex(GetPlayers_ExcludeHuman)), "a:count")
		menu_additem(menu, "Add a Terrorist bot", "a:addt")
		menu_additem(menu, "Add a CT bot", "a:addct")
		menu_additem(menu, "Kick all bots", "a:kick")
		menu_additem(menu, "Kill all bots", "a:kill")
	}
	else if (cat == CAT_MATCH)
	{
		menu_additem(menu, "Restart round now", "a:restart")
		menu_additem(menu, "\rReset this mode's settings", "a:reset")
	}

	for (new s = 0; s < sizeof SETTINGS; s++)
	{
		if (!setting_visible(s, cat))
			continue
		get_cvar_string(SETTINGS[s][SD_Cvar], cur, charsmax(cur))
		value_label(SETTINGS[s][SD_Values], SETTINGS[s][SD_Labels], cur, label, charsmax(label))
		formatex(text, charsmax(text), "%s: \y%s", SETTINGS[s][SD_Name], label)
		formatex(info, charsmax(info), "%d %d", cat, s)
		menu_additem(menu, text, info)
	}

	menu_nav(menu)
	menu_display(id, menu, page)
}

public settings_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	new page = item / 7

	if (info[0] == 'a')
	{
		if (equal(info, "a:count"))
		{
			show_botcount(id)
			return PLUGIN_HANDLED
		}
		else if (equal(info, "a:restart") || equal(info, "a:reset"))
		{
			do_match(info[2])
			show_settings(id, CAT_MATCH, page)
			return PLUGIN_HANDLED
		}
		do_bots(info[2], 0)
		set_task(0.3, "task_reshow_bots", id)
		return PLUGIN_HANDLED
	}

	new scat[4], sidx[4]
	parse(info, scat, charsmax(scat), sidx, charsmax(sidx))
	new cat = str_to_num(scat), s = str_to_num(sidx)

	new cur[40], val[40]
	get_cvar_string(SETTINGS[s][SD_Cvar], cur, charsmax(cur))
	next_value(SETTINGS[s][SD_Values], cur, val, charsmax(val))
	do_set(id, s, val)

	show_settings(id, cat, page)
	return PLUGIN_HANDLED
}

// restart | reset
do_match(const action[])
{
	if (equal(action, "reset"))
	{
		new path[192]
		saved_file(SC_MODE, path, charsmax(path))
		delete_file(path)
		for (new s = 0; s < sizeof SETTINGS; s++)
			if (SETTINGS[s][SD_Scope] == SC_MODE)
				g_SavedHas[s] = false
		apply_all(true)
		gm_print(0, "%s settings reset to defaults.", g_ModeName[g_Cur])
	}
	else
		server_cmd("sv_restart 1")
}

// addt | addct | kick | kill | count (with n)
do_bots(const action[], n)
{
	if (equal(action, "addt"))       server_cmd("bot_add_t")
	else if (equal(action, "addct")) server_cmd("bot_add_ct")
	else if (equal(action, "kick"))  server_cmd("bot_kick")
	else if (equal(action, "kill"))  server_cmd("bot_kill")
	else if (equal(action, "count")) server_cmd("bot_quota %d", clamp(n, 0, MaxClients - 1))
	set_task(1.0, "task_save_quota")
}

// Sets a setting, saves it, and applies whatever goes with it.
do_set(id, s, const val[])
{
	if (!g_ApplyingPreset && g_LastPreset[0] && SETTINGS[s][SD_Scope] != SC_NONE)
	{
		g_LastPreset[0] = 0
		write_data_line("lastpreset.txt", "")
	}
	set_setting(s, val)

	new cvar[32]
	copy(cvar, charsmax(cvar), SETTINGS[s][SD_Cvar])
	if (equal(cvar, "gm_weapons"))
	{
		apply_weapons()
		// keep the bot weapon choice that the preset picked
		new bw[4]
		get_pcvar_string(g_pBotWeapons, bw, charsmax(bw))
		set_setting(find_setting("gm_bot_weapons"), bw)
		gm_print(0, "Weapon preset applies from the next spawn.")
	}
	else if (equal(cvar, "gm_bot_weapons"))
		apply_bot_weapons()
	else if (equal(cvar, "gm_gg_order"))
	{
		apply_gg_order(true)
		gm_print(0, "New weapon order. Restart the map for everyone to start from the first gun.")
	}
	else if (equal(cvar, "gm_zp_specials"))
		apply_zp_specials()
	else if (equal(cvar, "gm_nobomb"))
	{
		apply_nobomb()
		if (get_pcvar_num(g_pNoBomb))
			strip_c4_all()
		gm_print(0, get_pcvar_num(g_pNoBomb) ? "Bomb planting is off." : "Bomb planting is back on from the next round.")
	}
	else if (equal(cvar, "mp_weapons_allow_map_placed") || equal(cvar, "mp_round_infinite") || equal(cvar, "mp_free_armor"))
		gm_print(id, "%s changes from the next round.", SETTINGS[s][SD_Name])
	else if (equal(cvar, "gg_block_objectives"))
		gm_print(0, "%s changes from the next round.", SETTINGS[s][SD_Name])
	else if (equal(cvar, "zp_", 3) || equal(cvar, "gg_dm_spawn_random") || equal(cvar, "gmdm_guns"))
		gm_print(id, "%s applies from the next spawn or round.", SETTINGS[s][SD_Name])
	else if (equal(cvar, "bot_difficulty"))
		respawn_bots()
	else if (equal(cvar, "bot_profile_db"))
		gm_print(id, "The bot roster changes on the next map.")
	else if (equal(cvar, "gm_botstyle"))
	{
		if (get_cvar_num("gm_botstyle"))
			apply_botstyle()
		else
		{
			// back to the roster picked in the menu
			new r = find_setting("bot_profile_db")
			set_cvar_string("bot_profile_db", g_SavedHas[r] ? g_SavedVal[r] : "BotProfile.db")
		}
		gm_print(0, "Bot aim changes now. Their personalities (how they roam and camp) change on the next map.")
	}
	else if (equal(cvar, "mp_", 3))
		gm_print(id, "%s changes take effect next round (Match settings > Restart round).", SETTINGS[s][SD_Name])
}

public task_reshow_bots(id)
{
	if (is_user_connected(id))
		show_settings(id, CAT_BOT, 0)
}

// Bot difficulty only applies to newly added bots, so re-add the current ones.
respawn_bots()
{
	new q = get_cvar_num("bot_quota")
	if (q <= 0)
		return
	server_cmd("bot_kick")
	set_task(0.5, "task_requota", q)
	gm_print(0, "Re-adding bots with the new difficulty.")
}

public task_requota(q)
{
	server_cmd("bot_quota %d", q)
	set_task(1.0, "task_save_quota")
}

show_botcount(id)
{
	new menu = menu_create("\yHow many bots?", "botcount_handler")
	new info[4], limit = MaxClients - 1
	for (new i = 0; i < sizeof BOT_COUNTS; i++)
	{
		if (BOT_COUNTS[i] > limit)
			break
		num_to_str(BOT_COUNTS[i], info, charsmax(info))
		menu_additem(menu, BOT_COUNTS[i] ? info : "None", info)
	}
	if (BOT_COUNTS[sizeof BOT_COUNTS - 1] > limit)
	{
		num_to_str(limit, info, charsmax(info))
		menu_additem(menu, fmt("%d (server full)", limit), info)
	}
	menu_nav(menu)
	menu_display(id, menu)
}

public botcount_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_settings(id, CAT_BOT, 0)
		return PLUGIN_HANDLED
	}
	new info[4]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	server_cmd("bot_quota %s", info)
	set_task(1.0, "task_save_quota")
	set_task(1.2, "task_reshow_bots", id)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Player tools
// ---------------------------------------------------------------------------

show_players(id)
{
	new menu = menu_create("\yGame Mode Menu \d> \yPlayer tools^n\dPick yourself or another player", "players_handler")
	new name[32], text[96], info[8]
	new players[32], num
	get_players(players, num)

	get_user_name(id, name, charsmax(name))
	menu_additem(menu, fmt("\yMe \d(%s)", name), fmt("%d", id))
	menu_additem(menu, "Everyone: respawn the dead", "all:respawn")
	menu_additem(menu, "Everyone: heal + armor", "all:heal")
	for (new i = 0; i < num; i++)
	{
		new p = players[i]
		if (p == id)
			continue
		get_user_name(p, name, charsmax(name))
		new CsTeams:team = cs_get_user_team(p)
		formatex(text, charsmax(text), "%s \d(%s%s%s)", name, team == CS_TEAM_T ? "T" : (team == CS_TEAM_CT ? "CT" : "spec"),
			is_user_bot(p) ? ", bot" : "", is_user_alive(p) ? "" : ", dead")
		num_to_str(p, info, charsmax(info))
		menu_additem(menu, text, info)
	}
	menu_nav(menu)
	menu_display(id, menu)
}

public players_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "all:", 4))
	{
		new players[32], num
		get_players(players, num)
		for (new i = 0; i < num; i++)
		{
			new p = players[i]
			if (equal(info, "all:respawn"))
			{
				if (!is_user_alive(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
					ExecuteHamB(Ham_CS_RoundRespawn, p)
			}
			else if (is_user_alive(p))
			{
				set_user_health(p, 100)
				cs_set_user_armor(p, 100, CS_ARMOR_VESTHELM)
			}
		}
		show_players(id)
		return PLUGIN_HANDLED
	}

	new p = str_to_num(info)
	if (!is_user_connected(p))
	{
		show_players(id)
		return PLUGIN_HANDLED
	}
	g_Target[id] = p
	show_player_actions(id)
	return PLUGIN_HANDLED
}

show_player_actions(id)
{
	new p = g_Target[id]
	if (!is_user_connected(p))
	{
		show_players(id)
		return
	}
	new name[32], title[96]
	get_user_name(p, name, charsmax(name))
	formatex(title, charsmax(title), "\y%s^n\dHealth %d  Money $%d", name, is_user_alive(p) ? get_user_health(p) : 0, cs_get_user_money(p))
	new menu = menu_create(title, "pactions_handler")

	menu_additem(menu, fmt("God mode: %s", g_God[p] ? "\yOn" : "\dOff"), "god")
	menu_additem(menu, fmt("Noclip (fly): %s", g_Noclip[p] ? "\yOn" : "\dOff"), "noclip")
	menu_additem(menu, "Give a weapon", "give")
	menu_additem(menu, "Heal + armor", "heal")
	menu_additem(menu, "Give $16000", "money")
	menu_additem(menu, is_user_alive(p) ? "Respawn" : "Respawn \y(dead)", "respawn")
	menu_additem(menu, "Switch team", "team")
	if (p != id)
	{
		menu_additem(menu, "Bring to where I'm looking", "bring")
		menu_additem(menu, "Teleport me to them", "goto")
		menu_additem(menu, (pev(p, pev_flags) & FL_FROZEN) ? "Unfreeze" : "Freeze", "freeze")
		menu_additem(menu, "Slap", "slap")
		menu_additem(menu, "Slay", "slay")
		menu_additem(menu, "\rKick", "kick")
	}
	menu_nav(menu)
	menu_display(id, menu)
}

public pactions_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_players(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	new p = g_Target[id]
	if (!is_user_connected(p))
	{
		show_players(id)
		return PLUGIN_HANDLED
	}
	if (equal(info, "give"))
	{
		show_give(id)
		return PLUGIN_HANDLED
	}
	do_player(id, p, info)
	if (equal(info, "kick"))
		set_task(0.3, "task_reshow_players", id)
	else
		show_player_actions(id)
	return PLUGIN_HANDLED
}

// actor is the player "bring"/"goto" are relative to.
do_player(actor, p, const action[], const arg[] = "")
{
	new bool:alive = bool:is_user_alive(p)

	if (equal(action, "god"))
	{
		g_God[p] = !g_God[p]
		if (alive) set_user_godmode(p, g_God[p])
	}
	else if (equal(action, "noclip"))
	{
		g_Noclip[p] = !g_Noclip[p]
		if (alive) set_user_noclip(p, g_Noclip[p])
	}
	else if (equal(action, "give") && alive && arg[0])
		give_full(p, arg)
	else if (equal(action, "heal") && alive)
	{
		set_user_health(p, 100)
		cs_set_user_armor(p, 100, CS_ARMOR_VESTHELM)
	}
	else if (equal(action, "money"))
		cs_set_user_money(p, 16000)
	else if (equal(action, "respawn"))
	{
		if (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT)
			ExecuteHamB(Ham_CS_RoundRespawn, p)
	}
	else if (equal(action, "team"))
	{
		new CsTeams:team = cs_get_user_team(p)
		cs_set_user_team(p, team == CS_TEAM_T ? CS_TEAM_CT : CS_TEAM_T)
		if (alive)
			ExecuteHamB(Ham_CS_RoundRespawn, p)
	}
	else if (equal(action, "bring") && alive && is_user_connected(actor) && actor != p)
	{
		new origin[3]
		get_user_origin(actor, origin, 3)
		origin[2] += 40
		set_user_origin(p, origin)
	}
	else if (equal(action, "goto") && alive && is_user_alive(actor) && actor != p)
	{
		new origin[3]
		get_user_origin(p, origin)
		origin[2] += 80
		set_user_origin(actor, origin)
	}
	else if (equal(action, "freeze") && alive)
	{
		new flags = pev(p, pev_flags)
		set_pev(p, pev_flags, (flags & FL_FROZEN) ? (flags & ~FL_FROZEN) : (flags | FL_FROZEN))
	}
	else if (equal(action, "slap") && alive)
		user_slap(p, 0)
	else if (equal(action, "slay") && alive)
		user_kill(p)
	else if (equal(action, "kick"))
		server_cmd("kick #%d", get_user_userid(p))
}

public task_reshow_players(id)
{
	if (is_user_connected(id))
		show_players(id)
}

show_give(id)
{
	new menu = menu_create("\yGive a weapon", "give_handler")
	for (new i = 0; i < sizeof GIVE_WEAPONS; i++)
		menu_additem(menu, GIVE_WEAPONS[i][1], GIVE_WEAPONS[i][0])
	menu_nav(menu)
	menu_display(id, menu)
}

public give_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_player_actions(id)
		return PLUGIN_HANDLED
	}
	new weapon[24]
	get_info(menu, item, weapon, charsmax(weapon))
	menu_destroy(menu)

	new p = g_Target[id]
	if (is_user_alive(p))
		give_full(p, weapon)
	show_give(id)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Add-ons menu
// ---------------------------------------------------------------------------

show_addons(id, page)
{
	new menu = menu_create("\yGame Mode Menu \d> \yAdd-ons^n\dWork in every mode. Changes apply on map restart.", "addons_handler")
	new text[128], info[4]

	if (addons_pending())
		menu_additem(menu, "\rRestart map now to apply", "r")

	for (new i = 0; i < g_AddonCount; i++)
	{
		formatex(text, charsmax(text), "%s: %s%s%s", g_AddonName[i],
			g_AddonOn[i] ? ((g_AddonOn[i] != g_AddonLoaded[i]) ? "\rOn*" : "\yOn") : ((g_AddonOn[i] != g_AddonLoaded[i]) ? "\rOff*" : "\dOff"),
			g_AddonDesc[i][0] ? " \d- " : "", g_AddonDesc[i])
		num_to_str(i, info, charsmax(info))
		menu_additem(menu, text, info)
	}
	menu_nav(menu)
	menu_display(id, menu, page)
}

public addons_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[4]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (info[0] == 'r')
	{
		new map[32]
		get_mapname(map, charsmax(map))
		gm_print(0, "Restarting map to apply add-ons...")
		set_task(1.5, "task_changelevel", 0, map, sizeof map)
		return PLUGIN_HANDLED
	}

	new i = str_to_num(info)
	if (g_AddonLoaded[i] && g_AddonOn[i] && addon_has_options(i))
	{
		show_addon_options(id, i)
		return PLUGIN_HANDLED
	}
	g_AddonOn[i] = !g_AddonOn[i]
	write_addons()
	show_addons(id, item / 7)
	return PLUGIN_HANDLED
}

show_addon_options(id, a, page = 0)
{
	new title[96], info[16], text[128], label[48]
	new cvar[32], name[32], values[96], labels[128], cur[64]
	formatex(title, charsmax(title), "\y%s", g_AddonName[a])
	new menu = menu_create(title, "addon_opt_handler")

	for (new n = 0; read_addon_setting(a, n, cvar, charsmax(cvar), name, charsmax(name), values, charsmax(values), labels, charsmax(labels)); n++)
	{
		if (!get_cvar_pointer(cvar))
			continue
		get_cvar_string(cvar, cur, charsmax(cur))
		value_label(values, labels, cur, label, charsmax(label))
		formatex(text, charsmax(text), "%s: \y%s", name, label)
		formatex(info, charsmax(info), "s %d %d", a, n)
		menu_additem(menu, text, info)
	}
	if (g_AddonCmd[a][0])
	{
		formatex(info, charsmax(info), "o %d", a)
		menu_additem(menu, "More options...", info)
	}
	formatex(info, charsmax(info), "t %d", a)
	menu_additem(menu, "\rTurn off", info)
	menu_nav(menu)
	menu_display(id, menu, page)
}

public addon_opt_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_addons(id, 0)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	new kind[2], sa[4], sn[4]
	parse(info, kind, charsmax(kind), sa, charsmax(sa), sn, charsmax(sn))
	new a = str_to_num(sa)

	if (kind[0] == 'o')
		client_cmd(id, g_AddonCmd[a])
	else if (kind[0] == 't')
	{
		g_AddonOn[a] = false
		write_addons()
		show_addons(id, 0)
	}
	else
	{
		new cvar[32], name[32], values[96], labels[128], cur[64], val[64]
		if (read_addon_setting(a, str_to_num(sn), cvar, charsmax(cvar), name, charsmax(name), values, charsmax(values), labels, charsmax(labels)))
		{
			get_cvar_string(cvar, cur, charsmax(cur))
			next_value(values, cur, val, charsmax(val))
			set_cvar_string(cvar, val)
			save_addon_settings(a)
		}
		show_addon_options(id, a, item / 7)
	}
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Admin tools: shortcuts to every AMX Mod X menu
// ---------------------------------------------------------------------------

show_admin(id, page)
{
	new menu = menu_create("\yGame Mode Menu \d> \yAdmin tools^n\dAMX Mod X menus", "admin_handler")
	menu_additem(menu, "\yHelp & commands", "help")
	for (new i = 0; i < sizeof ADMIN_MENUS; i++)
		menu_additem(menu, ADMIN_MENUS[i][1], ADMIN_MENUS[i][0])
	menu_nav(menu)
	menu_display(id, menu, page)
}

public admin_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new cmd[32]
	get_info(menu, item, cmd, charsmax(cmd))
	menu_destroy(menu)
	if (equal(cmd, "help"))
		show_motd(id, fmt("%s/gamemodes/help.txt", g_CfgDir), "Game Mode Menu - Help")
	else
		client_cmd(id, cmd)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Control panel API. The companion web app sends these over RCON.
// gm_status writes data/gamemodes/status.json (one JSON object per line);
// RCON output is too lossy for large replies, so the panel reads the file.
// ---------------------------------------------------------------------------

json_escape(const src[], out[], len)
{
	new j
	for (new i = 0; src[i] && j < len - 2; i++)
	{
		new c = src[i]
		if (c == '"' || c == '\')
		{
			out[j++] = '\'
			out[j++] = c
		}
		else if (c >= 32)
			out[j++] = c
	}
	out[j] = 0
}

// The listen-server host (first human), used as "me" for bring/goto.
host_player()
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && !is_user_hltv(p))
			return p
	return 0
}

public srv_status()
{
	new map[32], esc[128], esc2[128], esc3[160], cur[40]
	get_mapname(map, charsmax(map))

	new plabel[48], pesc[96]
	get_preset_label(plabel, charsmax(plabel))
	json_escape(plabel, pesc, charsmax(pesc))

	new f = fopen(fmt("%s/status.json", g_DataDir), "wt")
	if (!f)
		return PLUGIN_HANDLED

	fprintf(f, "{^"t^":^"state^",^"mode^":^"%s^",^"map^":^"%s^",^"nav^":%d,^"career^":%d,^"navbatch^":%d,^"navleft^":%d,^"bots^":%d,^"humans^":%d,^"maxplayers^":%d,^"timeleft^":%d,^"presets^":%d,^"addons_pending^":%d,^"version^":^"%s^",^"crashmap^":^"%s^",^"preset^":^"%s^",^"peaceful^":%d,^"paused^":%d,^"recording^":%d,^"timelimit^":%d}^n",
		g_ModeId[g_Cur], map, has_nav(map), g_Career, navbatch_active(), navbatch_active() ? navqueue_count() : 0,
		get_playersnum_ex(GetPlayers_ExcludeHuman), get_playersnum_ex(GetPlayers_ExcludeBots), MaxClients,
		get_timeleft(), presets_enabled(), addons_pending(), VERSION, g_CrashMap, pesc, get_cvar_num("bot_zombie"), g_Paused, g_Recording, get_cvar_num("mp_timelimit"))

	for (new i = 0; i < g_ModeCount; i++)
	{
		json_escape(g_ModeName[i], esc, charsmax(esc))
		json_escape(g_ModeDesc[i], esc2, charsmax(esc2))
		fprintf(f, "{^"t^":^"mode^",^"id^":^"%s^",^"name^":^"%s^",^"desc^":^"%s^",^"flags^":^"%s^",^"options^":%d}^n",
			g_ModeId[i], esc, esc2, g_ModeFlags[i], g_ModeCmd[i][0] != 0)
	}

	for (new s = 0; s < sizeof SETTINGS; s++)
	{
		if (SETTINGS[s][SD_Cat] == CAT_HIDDEN)
			continue
		get_cvar_string(SETTINGS[s][SD_Cvar], cur, charsmax(cur))
		json_escape(cur, esc, charsmax(esc))
		json_escape(SETTINGS[s][SD_Labels], esc3, charsmax(esc3))
		fprintf(f, "{^"t^":^"setting^",^"cvar^":^"%s^",^"name^":^"%s^",^"cat^":%d,^"scope^":%d,^"value^":^"%s^",^"values^":^"%s^",^"labels^":^"%s^",^"visible^":%d}^n",
			SETTINGS[s][SD_Cvar], SETTINGS[s][SD_Name], SETTINGS[s][SD_Cat], SETTINGS[s][SD_Scope], esc,
			SETTINGS[s][SD_Values], esc3, setting_visible(s, SETTINGS[s][SD_Cat]))
	}

	new cvar[32], name[32], values[96], labels[128]
	for (new a = 0; a < g_AddonCount; a++)
	{
		json_escape(g_AddonName[a], esc, charsmax(esc))
		json_escape(g_AddonDesc[a], esc2, charsmax(esc2))
		fprintf(f, "{^"t^":^"addon^",^"id^":^"%s^",^"name^":^"%s^",^"desc^":^"%s^",^"on^":%d,^"loaded^":%d}^n",
			g_AddonId[a], esc, esc2, g_AddonOn[a], g_AddonLoaded[a])
		if (!g_AddonLoaded[a])
			continue
		for (new n = 0; read_addon_setting(a, n, cvar, charsmax(cvar), name, charsmax(name), values, charsmax(values), labels, charsmax(labels)); n++)
		{
			if (!get_cvar_pointer(cvar))
				continue
			get_cvar_string(cvar, cur, charsmax(cur))
			json_escape(cur, esc, charsmax(esc))
			json_escape(labels, esc3, charsmax(esc3))
			fprintf(f, "{^"t^":^"addonset^",^"addon^":^"%s^",^"cvar^":^"%s^",^"name^":^"%s^",^"value^":^"%s^",^"values^":^"%s^",^"labels^":^"%s^"}^n",
				g_AddonId[a], cvar, name, esc, values, esc3)
		}
	}

	new pname[32], team[8], auth[40], authesc[80]
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_hltv(p))
			continue
		get_user_name(p, pname, charsmax(pname))
		json_escape(pname, esc, charsmax(esc))
		get_user_authid(p, auth, charsmax(auth))
		json_escape(auth, authesc, charsmax(authesc))
		switch (cs_get_user_team(p))
		{
			case CS_TEAM_T:  copy(team, charsmax(team), "T")
			case CS_TEAM_CT: copy(team, charsmax(team), "CT")
			default:         copy(team, charsmax(team), "SPEC")
		}
		new bool:alive = bool:is_user_alive(p)
		fprintf(f, "{^"t^":^"player^",^"uid^":%d,^"name^":^"%s^",^"team^":^"%s^",^"bot^":%d,^"alive^":%d,^"hp^":%d,^"armor^":%d,^"money^":%d,^"frags^":%d,^"deaths^":%d,^"god^":%d,^"noclip^":%d,^"frozen^":%d,^"host^":%d,^"steamid^":^"%s^"}^n",
			get_user_userid(p), esc, team, is_user_bot(p), alive, alive ? get_user_health(p) : 0, alive ? get_user_armor(p) : 0,
			cs_get_user_money(p), get_user_frags(p), cs_get_user_deaths(p), g_God[p], g_Noclip[p],
			(pev(p, pev_flags) & FL_FROZEN) != 0, p == host_player(), authesc)
	}
	write_preset_status(f)
	write_event_status(f)
	new host = host_player()
	fprintf(f, "{^"t^":^"me^",^"uid^":%d,^"haspos^":%d}^n", host ? get_user_userid(host) : 0, (host && g_HasPos[host]) ? 1 : 0)
	fprintf(f, "{^"t^":^"end^"}^n")
	fclose(f)
	server_print("GM status written")
	return PLUGIN_HANDLED
}

bool:career_blocked()
{
	if (!g_Career)
		return false
	server_print("[Game Mode] Not available during a Tour of Duty match.")
	return true
}

// gm_mode <id> [map]
public srv_mode()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new id[32], map[32]
	read_argv(1, id, charsmax(id))
	read_argv(2, map, charsmax(map))
	new mode = find_mode(id)
	if (mode < 0)
	{
		server_print("[Game Mode] Unknown mode: %s", id)
		return PLUGIN_HANDLED
	}
	if (!map[0])
		get_mapname(map, charsmax(map))
	if (!is_map_valid(map))
	{
		server_print("[Game Mode] Unknown map: %s", map)
		return PLUGIN_HANDLED
	}
	do_go(mode, map)
	return PLUGIN_HANDLED
}

// gm_map <map>  (keeps the mode; map presets may apply)
public srv_map()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new map[32]
	read_argv(1, map, charsmax(map))
	if (!is_map_valid(map))
	{
		server_print("[Game Mode] Unknown map: %s", map)
		return PLUGIN_HANDLED
	}
	do_go(-1, map)
	return PLUGIN_HANDLED
}

// gm_set <cvar> <value>  (only settings the menu knows about)
public srv_set()
{
	new cvar[32], val[40]
	read_argv(1, cvar, charsmax(cvar))
	read_argv(2, val, charsmax(val))
	new s = find_setting(cvar)
	if (s == -1 || SETTINGS[s][SD_Cat] == CAT_HIDDEN && !equal(cvar, "gm_mappresets"))
	{
		server_print("[Game Mode] Unknown setting: %s", cvar)
		return PLUGIN_HANDLED
	}
	if (SETTINGS[s][SD_Scope] != SC_GLOBAL && career_blocked())
		return PLUGIN_HANDLED
	do_set(0, s, val)
	server_print("[Game Mode] %s = %s", cvar, val)
	return PLUGIN_HANDLED
}

find_addon(const id[])
{
	for (new a = 0; a < g_AddonCount; a++)
		if (equali(g_AddonId[a], id))
			return a
	return -1
}

// gm_addon <id> <0|1>
public srv_addon()
{
	new id[32], val[4]
	read_argv(1, id, charsmax(id))
	read_argv(2, val, charsmax(val))
	new a = find_addon(id)
	if (a == -1)
	{
		server_print("[Game Mode] Unknown add-on: %s", id)
		return PLUGIN_HANDLED
	}
	g_AddonOn[a] = str_to_num(val) != 0
	write_addons()
	server_print("[Game Mode] Add-on %s %s (restart the map to apply)", id, g_AddonOn[a] ? "on" : "off")
	return PLUGIN_HANDLED
}

// gm_addonset <addon> <cvar> <value>
public srv_addonset()
{
	new id[32], cvar[32], val[64], name[32], values[96], labels[128], c[32]
	read_argv(1, id, charsmax(id))
	read_argv(2, cvar, charsmax(cvar))
	read_argv(3, val, charsmax(val))
	new a = find_addon(id)
	if (a == -1)
		return PLUGIN_HANDLED
	for (new n = 0; read_addon_setting(a, n, c, charsmax(c), name, charsmax(name), values, charsmax(values), labels, charsmax(labels)); n++)
	{
		if (equali(c, cvar) && get_cvar_pointer(cvar))
		{
			set_cvar_string(cvar, val)
			save_addon_settings(a)
			server_print("[Game Mode] %s = %s", cvar, val)
			break
		}
	}
	return PLUGIN_HANDLED
}

public srv_restartmap()
{
	new map[32]
	get_mapname(map, charsmax(map))
	gm_print(0, "Restarting map...")
	set_task(1.0, "task_changelevel", 0, map, sizeof map)
	return PLUGIN_HANDLED
}

// gm_pause: toggles the engine's pause. On a listen server the console's
// "pause" is sent on by the host player, so everything freezes, bots included.
// Plugin tasks don't run while paused, which is why pausable stays on.
public srv_pause()
{
	set_cvar_num("pausable", 1)
	server_cmd("pause")
	g_Paused = !g_Paused
	server_print("GM %s", g_Paused ? "paused" : "resumed")
	return PLUGIN_HANDLED
}

// gm_do <addtime|endround|money|heal|demo>: one-click actions for the host
public srv_do()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new arg[16]
	read_argv(1, arg, charsmax(arg))
	new host = host_player()
	if (equal(arg, "addtime"))
	{
		new Float:t = get_cvar_float("mp_timelimit")
		if (t <= 0.0)
			gm_print(0, "This map has no time limit.")
		else
		{
			set_cvar_float("mp_timelimit", t + 10.0)
			gm_print(0, "Added 10 minutes to the map.")
		}
	}
	else if (equal(arg, "endround"))
	{
		if (get_cvar_pointer("mp_give_player_c4"))   // ReGameDLL's endround command
			server_cmd("endround")
		else
			server_cmd("sv_restartround 1")
	}
	else if (equal(arg, "money") && host)
	{
		cs_set_user_money(host, max(get_cvar_num("mp_maxmoney"), 16000))
		gm_print(host, "Money topped up.")
	}
	else if (equal(arg, "heal") && host && is_user_alive(host))
	{
		set_user_health(host, max(get_user_health(host), 100))
		cs_set_user_armor(host, 100, CS_ARMOR_VESTHELM)
		gm_print(host, "Health and armor restored.")
	}
	else if (equal(arg, "demo"))
	{
		// on a listen server the console's record/stop act on the host's game
		if (g_Recording)
		{
			server_cmd("stop")
			gm_print(0, "Demo saved: czero/%s.dem", g_DemoName)
		}
		else
		{
			new map[32]
			get_mapname(map, charsmax(map))
			get_time(fmt("%s_%%Y%%m%%d_%%H%%M", map), g_DemoName, charsmax(g_DemoName))
			server_cmd("record ^"%s^"", g_DemoName)
			gm_print(0, "Recording demo %s - press it again to stop.", g_DemoName)
		}
		g_Recording = !g_Recording
	}
	return PLUGIN_HANDLED
}

// gm_announce <text>: message in the middle of everyone's screen
public srv_announce()
{
	new text[128]
	read_args(text, charsmax(text))
	remove_quotes(text)
	trim(text)
	if (!text[0])
		return PLUGIN_HANDLED
	set_hudmessage(255, 220, 80, -1.0, 0.3, 0, 0.0, 6.0, 0.3, 0.5, -1)
	show_hudmessage(0, "%s", text)
	client_print(0, print_chat, "[Host] %s", text)
	return PLUGIN_HANDLED
}

// gm_team <swap|t|ct|spec>: swap everyone's team, or move the host
public srv_team()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new arg[8]
	read_argv(1, arg, charsmax(arg))
	if (equal(arg, "swap"))
	{
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_connected(p))
				continue
			new CsTeams:t = cs_get_user_team(p)
			if (t == CS_TEAM_T)
				cs_set_user_team(p, CS_TEAM_CT)
			else if (t == CS_TEAM_CT)
				cs_set_user_team(p, CS_TEAM_T)
		}
		// bots would otherwise rejoin their old side
		set_cvar_string("bot_join_team", "any")
		server_cmd("sv_restart 1")
		gm_print(0, "Teams swapped. The round restarts.")
		return PLUGIN_HANDLED
	}
	new host = host_player()
	if (!host)
		return PLUGIN_HANDLED
	if (equal(arg, "spec"))
	{
		set_cvar_num("allow_spectators", 1)
		engclient_cmd(host, "jointeam", "6")
		gm_print(host, "You're spectating. Use the panel or the team menu (M) to join again.")
	}
	else if (equal(arg, "t") || equal(arg, "ct"))
	{
		new CsTeams:team = equal(arg, "t") ? CS_TEAM_T : CS_TEAM_CT
		if (cs_get_user_team(host) != team)
		{
			if (is_user_alive(host))
				user_silentkill(host)
			cs_set_user_team(host, team)
		}
		ExecuteHamB(Ham_CS_RoundRespawn, host)
		gm_print(host, "You're now on the %s side.", team == CS_TEAM_T ? "Terrorist" : "Counter-Terrorist")
	}
	return PLUGIN_HANDLED
}

// gm_bots <addt|addct|kick|kill|count> [n]
public srv_bots()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new action[16]
	read_argv(1, action, charsmax(action))
	do_bots(action, read_argv_int(2))
	return PLUGIN_HANDLED
}

// gm_match <restart|reset>
public srv_match()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new action[16]
	read_argv(1, action, charsmax(action))
	do_match(action)
	return PLUGIN_HANDLED
}

// gm_player <userid|all> <action> [weapon]
public srv_player()
{
	new target[16], action[16], arg[32]
	read_argv(1, target, charsmax(target))
	read_argv(2, action, charsmax(action))
	read_argv(3, arg, charsmax(arg))
	new host = host_player()

	if (equal(target, "all"))
	{
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_user_connected(p))
				continue
			if (equal(action, "respawn") && is_user_alive(p))
				continue
			do_player(host, p, action, arg)
		}
		return PLUGIN_HANDLED
	}
	new p = find_player_ex(FindPlayer_MatchUserId, str_to_num(target))
	if (p)
		do_player(host, p, action, arg)
	return PLUGIN_HANDLED
}

// gm_nav <learn|analyze|batch|stop>
public srv_nav()
{
	if (career_blocked())
		return PLUGIN_HANDLED
	new action[16]
	read_argv(1, action, charsmax(action))
	do_nav(0, action)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Presets: one-click setups (mode + settings + bots + optional map).
// Built-in ones live in configs/gamemodes/presets.ini, saved ones in
// data/gamemodes/presets_user.ini. Same line format in both:
//   id "Name" "Description" "mode" "map" "bots" "cvar=value cvar=value ..."
// ---------------------------------------------------------------------------


load_presets()
{
	g_PresetCount = 0
	load_preset_file(fmt("%s/gamemodes/presets.ini", g_CfgDir), false)
	load_preset_file(fmt("%s/presets_user.ini", g_DataDir), true)
}

load_preset_file(const path[], bool:user)
{
	new f = fopen(path, "rt")
	if (!f)
		return
	static line[1400]
	new bots[8]
	while (!feof(f) && g_PresetCount < MAX_PRESETS)
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';' || line[0] == '/')
			continue
		new i = g_PresetCount
		bots[0] = 0
		parse(line, g_PresetId[i], charsmax(g_PresetId[]), g_PresetName[i], charsmax(g_PresetName[]),
			g_PresetDesc[i], charsmax(g_PresetDesc[]), g_PresetMode[i], charsmax(g_PresetMode[]),
			g_PresetMap[i], charsmax(g_PresetMap[]), bots, charsmax(bots), g_PresetSettings[i], charsmax(g_PresetSettings[]))
		if (!g_PresetId[i][0])
			continue
		g_PresetBots[i] = bots[0] ? str_to_num(bots) : -1
		g_PresetUser[i] = user
		g_PresetCount++
	}
	fclose(f)
}

find_preset(const id[])
{
	for (new i = 0; i < g_PresetCount; i++)
		if (equali(g_PresetId[i], id))
			return i
	return -1
}

apply_preset(p)
{
	if (g_Career)
	{
		gm_print(0, "Presets aren't available during a Tour of Duty match.")
		return
	}
	new mode = g_PresetMode[p][0] ? find_mode(g_PresetMode[p]) : g_Cur
	if (mode < 0)
	{
		gm_print(0, "Preset %s uses a mode that isn't installed (%s).", g_PresetName[p], g_PresetMode[p])
		return
	}
	new map[32], cur[32]
	get_mapname(cur, charsmax(cur))
	copy(map, charsmax(map), g_PresetMap[p][0] ? g_PresetMap[p] : cur)
	if (!is_map_valid(map))
		copy(map, charsmax(map), cur)

	// Start the target mode from its defaults
	delete_file(fmt("%s/mode_%s.cfg", g_DataDir, g_ModeId[mode]))
	if (mode == g_Cur)
	{
		for (new s = 0; s < sizeof SETTINGS; s++)
			if (SETTINGS[s][SD_Scope] == SC_MODE)
				g_SavedHas[s] = false
		apply_all(false)
	}

	g_ApplyingPreset = true
	// Global settings apply now; mode settings apply now (same mode) or are
	// written to the target mode's file so they load with it.
	new token[64], cvar[32], val[40], pos, eq
	new f = (mode != g_Cur) ? fopen(fmt("%s/mode_%s.cfg", g_DataDir, g_ModeId[mode]), "wt") : 0
	if (f)
		fprintf(f, "// Saved by the Game Mode Menu (preset %s)^n", g_PresetId[p])
	while ((pos = argparse(g_PresetSettings[p], pos, token, charsmax(token))) != -1)
	{
		eq = contain(token, "=")
		if (eq <= 0)
			continue
		copy(cvar, min(eq, charsmax(cvar)), token)
		copy(val, charsmax(val), token[eq + 1])
		new s = find_setting(cvar)
		if (s == -1)
			continue
		if (SETTINGS[s][SD_Scope] == SC_MODE && mode != g_Cur)
		{
			if (f)
				fprintf(f, "%s ^"%s^"^n", cvar, val)
		}
		else if (!equal(cvar, "bot_difficulty") || g_PresetBots[p] < 0)
			do_set(0, s, val)
		else
		{
			// bots are re-added below with the new count, so just store it
			set_setting(s, val)
			server_cmd("bot_kick")
		}
	}
	if (f)
		fclose(f)
	g_ApplyingPreset = false

	if (g_PresetBots[p] >= 0)
		set_task(1.0, "task_preset_bots", g_PresetBots[p])

	gm_print(0, "Preset: %s", g_PresetName[p])
	formatex(g_LastPreset, charsmax(g_LastPreset), "%s|%s", g_ModeId[mode], g_PresetName[p])
	write_data_line("lastpreset.txt", g_LastPreset)
	g_SummaryShown = false
	if (mode != g_Cur || !equali(map, cur))
		do_go(mode, map)
	else
		server_cmd("sv_restart 1")
}

public task_preset_bots(n)
{
	do_bots("count", n)
}

// Saves the current mode, bot count and settings as a new preset.
save_preset(const rawname[])
{
	new name[40]
	copy(name, charsmax(name), rawname)
	replace_all(name, charsmax(name), "^"", "")
	replace_all(name, charsmax(name), "_", " ")
	trim(name)
	if (!name[0])
		return

	new id[24], n = 1
	do formatex(id, charsmax(id), "my%d", n++)
	while (find_preset(id) != -1)

	static settings[1024]
	new cur[40], len
	settings[0] = 0
	for (new s = 0; s < sizeof SETTINGS; s++)
	{
		if (SETTINGS[s][SD_Cat] == CAT_HIDDEN || SETTINGS[s][SD_Scope] == SC_NONE || !get_cvar_pointer(SETTINGS[s][SD_Cvar]))
			continue
		if (equal(SETTINGS[s][SD_Cvar], "bot_profile_db"))
			continue
		get_cvar_string(SETTINGS[s][SD_Cvar], cur, charsmax(cur))
		if (!cur[0] || contain(cur, " ") != -1)
			continue
		len += formatex(settings[len], charsmax(settings) - len, "%s%s=%s", len ? " " : "", SETTINGS[s][SD_Cvar], cur)
	}

	new map[32]
	get_mapname(map, charsmax(map))
	new f = fopen(fmt("%s/presets_user.ini", g_DataDir), "at")
	if (!f)
		return
	fprintf(f, "%s ^"%s^" ^"%s on %s^" ^"%s^" ^"^" ^"%d^" ^"%s^"^n", id, name, g_ModeName[g_Cur], map, g_ModeId[g_Cur],
		get_playersnum_ex(GetPlayers_ExcludeHuman), settings)
	fclose(f)
	load_presets()
	gm_print(0, "Saved preset: %s", name)
}

delete_preset(p)
{
	if (!g_PresetUser[p])
		return
	new path[192], line[512], id[24], name[40]
	copy(name, charsmax(name), g_PresetName[p])
	formatex(path, charsmax(path), "%s/presets_user.ini", g_DataDir)
	new Array:keep = ArrayCreate(512)
	new f = fopen(path, "rt")
	if (f)
	{
		while (!feof(f))
		{
			fgets(f, line, charsmax(line))
			trim(line)
			if (!line[0])
				continue
			parse(line, id, charsmax(id))
			if (!equali(id, g_PresetId[p]))
				ArrayPushString(keep, line)
		}
		fclose(f)
	}
	f = fopen(path, "wt")
	if (f)
	{
		for (new i = 0; i < ArraySize(keep); i++)
		{
			ArrayGetString(keep, i, line, charsmax(line))
			fprintf(f, "%s^n", line)
		}
		fclose(f)
	}
	ArrayDestroy(keep)
	load_presets()
	gm_print(0, "Deleted preset: %s", name)
}

show_presets(id)
{
	new menu = menu_create("\yGame Mode Menu \d> \yPresets^n\dOne click sets the mode, settings and bots", "presets_handler")
	new text[128], info[8]
	for (new i = 0; i < g_PresetCount; i++)
	{
		if (g_PresetUser[i] && (i == 0 || !g_PresetUser[i - 1]))
			menu_addtext(menu, "\yYour presets", 0)
		formatex(text, charsmax(text), "%s \d- %s", g_PresetName[i], g_PresetDesc[i])
		num_to_str(i, info, charsmax(info))
		menu_additem(menu, text, info)
	}
	menu_additem(menu, "\ySave current setup as a preset...", "save")
	menu_nav(menu)
	menu_display(id, menu)
}

public presets_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[8]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	if (equal(info, "save"))
	{
		gm_print(id, "Type a name for this preset and press Enter.")
		client_cmd(id, "messagemode gm_presetname")
		return PLUGIN_HANDLED
	}
	new p = str_to_num(info)
	if (g_PresetUser[p])
	{
		g_PickPreset[id] = p
		new m = menu_create(fmt("\y%s^n\d%s", g_PresetName[p], g_PresetDesc[p]), "userpreset_handler")
		menu_additem(m, "Apply", "a")
		menu_additem(m, "\rDelete", "d")
		menu_nav(m)
		menu_display(id, m)
		return PLUGIN_HANDLED
	}
	apply_preset(p)
	return PLUGIN_HANDLED
}

public userpreset_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_presets(id)
		return PLUGIN_HANDLED
	}
	new info[4]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)
	new p = g_PickPreset[id]
	if (p < 0 || p >= g_PresetCount)
		return PLUGIN_HANDLED
	if (info[0] == 'a')
		apply_preset(p)
	else
	{
		delete_preset(p)
		show_presets(id)
	}
	return PLUGIN_HANDLED
}

public cmd_presetname(id)
{
	if (!(get_user_flags(id) & ACCESS))
		return PLUGIN_HANDLED
	new name[64]
	read_args(name, charsmax(name))
	remove_quotes(name)
	save_preset(name)
	return PLUGIN_HANDLED
}

// gm_preset <id>
public srv_preset()
{
	new id[24]
	read_argv(1, id, charsmax(id))
	new p = find_preset(id)
	if (p == -1)
		server_print("[Game Mode] Unknown preset: %s", id)
	else
		apply_preset(p)
	return PLUGIN_HANDLED
}

// gm_presetsave <name_with_underscores>
public srv_presetsave()
{
	new name[64]
	read_argv(1, name, charsmax(name))
	save_preset(name)
	return PLUGIN_HANDLED
}

// gm_presetdel <id>
public srv_presetdel()
{
	new id[24]
	read_argv(1, id, charsmax(id))
	new p = find_preset(id)
	if (p != -1)
		delete_preset(p)
	return PLUGIN_HANDLED
}

write_preset_status(f)
{
	new esc[96], esc2[128]
	for (new i = 0; i < g_PresetCount; i++)
	{
		json_escape(g_PresetName[i], esc, charsmax(esc))
		json_escape(g_PresetDesc[i], esc2, charsmax(esc2))
		fprintf(f, "{^"t^":^"preset^",^"id^":^"%s^",^"name^":^"%s^",^"desc^":^"%s^",^"mode^":^"%s^",^"map^":^"%s^",^"bots^":%d,^"user^":%d}^n",
			g_PresetId[i], esc, esc2, g_PresetMode[i], g_PresetMap[i], g_PresetBots[i], g_PresetUser[i])
	}
}

// ---------------------------------------------------------------------------
// Map lists in data/gamemodes: favorites.txt, recent.txt, hidden_maps.txt
// (one map per line; the control panel edits the same files)
// ---------------------------------------------------------------------------

list_read(const file[], Array:out)
{
	new f = fopen(fmt("%s/%s", g_DataDir, file), "rt")
	if (!f)
		return
	new line[64]
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (line[0])
			ArrayPushString(out, line)
	}
	fclose(f)
}

list_write(const file[], Array:list)
{
	new f = fopen(fmt("%s/%s", g_DataDir, file), "wt")
	if (!f)
		return
	new line[64]
	for (new i = 0; i < ArraySize(list); i++)
	{
		ArrayGetString(list, i, line, charsmax(line))
		fprintf(f, "%s^n", line)
	}
	fclose(f)
}

bool:list_has(const file[], const map[])
{
	new Array:list = ArrayCreate(64)
	list_read(file, list)
	new line[64], bool:found
	for (new i = 0; i < ArraySize(list) && !found; i++)
	{
		ArrayGetString(list, i, line, charsmax(line))
		found = bool:equali(line, map)
	}
	ArrayDestroy(list)
	return found
}

list_count(const file[])
{
	new Array:list = ArrayCreate(64)
	list_read(file, list)
	new n = ArraySize(list)
	ArrayDestroy(list)
	return n
}

// Adds or removes a map; added maps go to the front.
list_set(const file[], const map[], bool:on, cap = 0)
{
	new Array:list = ArrayCreate(64)
	list_read(file, list)
	new line[64]
	for (new i = ArraySize(list) - 1; i >= 0; i--)
	{
		ArrayGetString(list, i, line, charsmax(line))
		if (equali(line, map))
			ArrayDeleteItem(list, i)
	}
	if (on)
	{
		if (ArraySize(list))
			ArrayInsertStringBefore(list, 0, map)
		else
			ArrayPushString(list, map)
	}
	while (cap && ArraySize(list) > cap)
		ArrayDeleteItem(list, ArraySize(list) - 1)
	list_write(file, list)
	ArrayDestroy(list)
}

remember_recent()
{
	new map[32]
	get_mapname(map, charsmax(map))
	list_set("recent.txt", map, true, 12)
}

// ---------------------------------------------------------------------------
// Crash memory: session.txt holds the map while it is running and "clean"
// after a normal map change or quit. Anything else means the game died.
// ---------------------------------------------------------------------------

check_last_session()
{
	new last[32], map[32]
	read_data_line("session.txt", last, charsmax(last))
	if (last[0] && !equal(last, "clean"))
	{
		copy(g_CrashMap, charsmax(g_CrashMap), last)
		write_data_line("crashmap.txt", last)
		log_amx("Last session ended unexpectedly on %s", last)
	}
	else
		read_data_line("crashmap.txt", g_CrashMap, charsmax(g_CrashMap))
	get_mapname(map, charsmax(map))
	write_data_line("session.txt", map)
}

clear_crash()
{
	g_CrashMap[0] = 0
	write_data_line("crashmap.txt", "")
}

// rebuild | hide | dismiss
do_crash(const action[])
{
	if (!g_CrashMap[0])
		return
	new map[32]
	copy(map, charsmax(map), g_CrashMap)
	clear_crash()
	if (equal(action, "rebuild"))
		rebuild_nav_map(map)
	else if (equal(action, "hide"))
	{
		list_set("hidden_maps.txt", map, true)
		gm_print(0, "%s is hidden from the map list. Maps > Hidden maps brings it back.", map)
	}
}

show_crash(id)
{
	new menu = menu_create(fmt("\rLast session ended unexpectedly^n\wMap: \y%s^n\dUsually a broken bot navigation file for that map.", g_CrashMap), "crash_handler")
	menu_additem(menu, "Load it and rebuild its bot navigation", "rebuild")
	menu_additem(menu, "Hide it from the map list", "hide")
	menu_additem(menu, "Dismiss", "dismiss")
	menu_nav(menu)
	menu_display(id, menu)
}

public crash_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		show_main(id)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)
	do_crash(info)
	if (!equal(info, "rebuild"))
		show_main(id)
	return PLUGIN_HANDLED
}

// gm_crash <rebuild|hide|dismiss>
public srv_crash()
{
	new action[16]
	read_argv(1, action, charsmax(action))
	do_crash(action)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Practice tools: instant respawn, unlimited money, saved spot
// ---------------------------------------------------------------------------


public fw_killed_post(victim, attacker, gib)
{
	new host = host_player()
	if (host && victim == host)
		g_DiffDeaths++
	else if (host && attacker == host)
		g_DiffKills++
	if (get_pcvar_num(g_pAutoRespawn) && is_user_connected(victim) && !is_user_bot(victim))
		set_task(1.0, "task_selfrespawn", victim + TASK_SELFRESPAWN)
}

// Auto difficulty: once a minute, look at the host's kills and deaths since
// the last change and nudge bot_difficulty one step.
public task_autodiff()
{
	if (!get_pcvar_num(g_pAutoDiff) || g_Career || !host_player())
		return
	new diff = get_cvar_num("bot_difficulty"), step = 0
	if (g_DiffKills >= 6 && g_DiffKills >= g_DiffDeaths * 2 && diff < 3)
		step = 1
	else if (g_DiffDeaths >= 5 && g_DiffDeaths * 2 >= g_DiffKills * 3 && diff > 0)
		step = -1
	if (!step)
		return
	new s = find_setting("bot_difficulty"), val[4]
	num_to_str(diff + step, val, charsmax(val))
	do_set(0, s, val)
	new const NAMES[][] = { "Easy", "Normal", "Hard", "Expert" }
	gm_print(0, step > 0 ? "You're on a roll (%d kills, %d deaths). Bots are now %s." : "Tough going (%d kills, %d deaths). Bots are now %s.",
		g_DiffKills, g_DiffDeaths, NAMES[diff + step])
	g_DiffKills = g_DiffDeaths = 0
}

public task_selfrespawn(taskid)
{
	new id = taskid - TASK_SELFRESPAWN
	// a mode with its own respawn rules
	if (!mode_has_flag(g_Cur, 'w') || get_cvar_pointer("zp_deathmatch"))
		return
	if (is_user_connected(id) && !is_user_alive(id) && (cs_get_user_team(id) == CS_TEAM_T || cs_get_user_team(id) == CS_TEAM_CT))
		ExecuteHamB(Ham_CS_RoundRespawn, id)
}

public ev_money(id)
{
	if (get_pcvar_num(g_pInfMoney) && read_data(1) < 16000)
	{
		remove_task(id + TASK_MONEY)
		set_task(0.1, "task_money", id + TASK_MONEY)
	}
}

public task_money(taskid)
{
	new id = taskid - TASK_MONEY
	if (is_user_connected(id))
		cs_set_user_money(id, 16000, 0)
}

save_pos(id)
{
	if (!is_user_alive(id))
		return
	pev(id, pev_origin, g_SavedPos[id])
	pev(id, pev_v_angle, g_SavedAng[id])
	g_HasPos[id] = true
	gm_print(id, "Spot saved.")
}

load_pos(id)
{
	if (!g_HasPos[id])
	{
		gm_print(id, "Save a spot first.")
		return
	}
	if (!is_user_alive(id))
		return
	engfunc(EngFunc_SetOrigin, id, g_SavedPos[id])
	set_pev(id, pev_velocity, Float:{ 0.0, 0.0, 0.0 })
	set_pev(id, pev_angles, g_SavedAng[id])
	set_pev(id, pev_v_angle, g_SavedAng[id])
	set_pev(id, pev_fixangle, 1)
}

public cmd_savepos(id)
{
	if (get_user_flags(id) & ACCESS)
		save_pos(id)
	return PLUGIN_HANDLED
}

public cmd_loadpos(id)
{
	if (get_user_flags(id) & ACCESS)
		load_pos(id)
	return PLUGIN_HANDLED
}

// gm_pos <save|load>  (for the host)
public srv_pos()
{
	new action[8]
	read_argv(1, action, charsmax(action))
	new host = host_player()
	if (!host)
		return PLUGIN_HANDLED
	if (equal(action, "save"))
		save_pos(host)
	else
		load_pos(host)
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Quick menu (F4): the actions used most often, one key each
// ---------------------------------------------------------------------------

public cmd_quick(id)
{
	if (has_access(id))
		show_quick(id)
	return PLUGIN_HANDLED
}

show_quick(id)
{
	new menu = menu_create("\yQuick actions^n\dF3 opens the full menu", "quick_handler")
	if (!g_Career)
	{
		menu_additem(menu, "Restart round", "restart")
		menu_additem(menu, fmt("Bots +1 \d(%d now)", get_playersnum_ex(GetPlayers_ExcludeHuman)), "botup")
		menu_additem(menu, "Bots -1", "botdown")
	}
	menu_additem(menu, fmt("God mode: %s", g_God[id] ? "\yOn" : "\dOff"), "god")
	menu_additem(menu, fmt("Noclip: %s", g_Noclip[id] ? "\yOn" : "\dOff"), "noclip")
	menu_additem(menu, "Respawn me", "respawn")
	menu_additem(menu, g_HasPos[id] ? "Save my spot \d(replaces the saved one)" : "Save my spot", "save")
	menu_additem(menu, g_HasPos[id] ? "Back to my saved spot" : "\dBack to my saved spot", "load")
	if (!g_Career)
		menu_additem(menu, fmt("Peaceful bots: %s", get_cvar_num("bot_zombie") ? "\yOn" : "\dOff"), "peace")
	new map[32]
	get_mapname(map, charsmax(map))
	menu_additem(menu, list_has("favorites.txt", map) ? fmt("Favourite map: \y%s", map) : fmt("Add %s to favourites", map), "fav")
	menu_additem(menu, "Heal me", "heal")
	if (!g_Career)
		menu_additem(menu, "Give me $16000", "money")
	menu_setprop(menu, MPROP_BACKNAME, "Previous page")
	menu_setprop(menu, MPROP_NEXTNAME, "More")
	menu_setprop(menu, MPROP_EXITNAME, "Close")
	menu_display(id, menu)
}

public quick_handler(id, menu, item)
{
	if (item == MENU_EXIT)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[16]
	get_info(menu, item, info, charsmax(info))
	menu_destroy(menu)

	new bots = get_playersnum_ex(GetPlayers_ExcludeHuman)
	if (equal(info, "restart"))      do_match("restart")
	else if (equal(info, "botup"))   do_bots("count", bots + 1)
	else if (equal(info, "botdown")) do_bots("count", max(bots - 1, 0))
	else if (equal(info, "save"))    save_pos(id)
	else if (equal(info, "load"))    load_pos(id)
	else if (equal(info, "peace"))   do_set(id, find_setting("bot_zombie"), get_cvar_num("bot_zombie") ? "0" : "1")
	else if (equal(info, "fav"))
	{
		new map[32]
		get_mapname(map, charsmax(map))
		new bool:on = !list_has("favorites.txt", map)
		list_set("favorites.txt", map, on)
		gm_print(id, on ? "%s added to your favourite maps." : "%s removed from your favourite maps.", map)
	}
	else if (equal(info, "heal"))    server_cmd("gm_do heal")
	else if (equal(info, "money"))   server_cmd("gm_do money")
	else                             do_player(id, id, info)

	set_task(0.3, "task_reshow_quick", id)
	return PLUGIN_HANDLED
}

public task_reshow_quick(id)
{
	if (is_user_connected(id))
		show_quick(id)
}

// ---------------------------------------------------------------------------
// Round-start summary and the control panel's feed
// ---------------------------------------------------------------------------

get_preset_label(out[], len)
{
	out[0] = 0
	new sep = contain(g_LastPreset, "|")
	if (sep > 0 && equal(g_LastPreset, g_ModeId[g_Cur], sep))
		copy(out, len, g_LastPreset[sep + 1])
}

public ev_roundstart()
{
	if (get_pcvar_num(g_pNoBomb))
		strip_c4_all()
	add_event("^"kind^":^"round^",^"text^":^"Round started^"")
	if (!g_SummaryShown)
	{
		g_SummaryShown = true
		set_task(0.5, "task_summary")
	}
}

public task_summary()
{
	if (g_Career)
		return
	new const diff[][] = { "Easy", "Normal", "Hard", "Expert" }
	new preset[48]
	get_preset_label(preset, charsmax(preset))
	set_hudmessage(233, 227, 207, -1.0, 0.18, 0, 0.0, 5.0, 0.4, 1.0, -1)
	show_hudmessage(0, "%s%s%s^n%d bots, %s   |   F3 menu   F4 quick actions", g_ModeName[g_Cur], preset[0] ? "  -  " : "", preset,
		get_playersnum_ex(GetPlayers_ExcludeHuman), diff[clamp(get_cvar_num("bot_difficulty"), 0, 3)])
}

new Float:g_LastWin
round_result(const text[])
{
	if (get_gametime() - g_LastWin < 1.0)
		return
	g_LastWin = get_gametime()
	add_event(fmt("^"kind^":^"round^",^"text^":^"%s^"", text))
}

public ev_win_t()    { round_result("Terrorists win the round"); }
public ev_win_ct()   { round_result("Counter-Terrorists win the round"); }
public ev_win_draw() { round_result("Round draw"); }

team_code(id, out[], len)
{
	switch (cs_get_user_team(id))
	{
		case CS_TEAM_T:  copy(out, len, "T")
		case CS_TEAM_CT: copy(out, len, "CT")
		default:         copy(out, len, "SPEC")
	}
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2), hs = read_data(3)
	new weapon[24], kname[32], vname[32], kesc[64], vesc[64], kteam[6], vteam[6]
	read_data(4, weapon, charsmax(weapon))
	if (!is_user_connected(victim))
		return
	get_user_name(victim, vname, charsmax(vname))
	json_escape(vname, vesc, charsmax(vesc))
	team_code(victim, vteam, charsmax(vteam))
	kesc[0] = kteam[0] = 0
	if (killer != victim && is_user_connected(killer))
	{
		get_user_name(killer, kname, charsmax(kname))
		json_escape(kname, kesc, charsmax(kesc))
		team_code(killer, kteam, charsmax(kteam))
	}
	add_event(fmt("^"kind^":^"kill^",^"killer^":^"%s^",^"kteam^":^"%s^",^"victim^":^"%s^",^"vteam^":^"%s^",^"weapon^":^"%s^",^"hs^":%d",
		kesc, kteam, vesc, vteam, weapon, hs))
}

public hook_say(id)
{
	new msg[128], cmd[12], name[32], esc[160], nesc[64], team[6]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	if (!msg[0] || !is_user_connected(id))
		return PLUGIN_CONTINUE
	read_argv(0, cmd, charsmax(cmd))
	get_user_name(id, name, charsmax(name))
	json_escape(msg, esc, charsmax(esc))
	json_escape(name, nesc, charsmax(nesc))
	team_code(id, team, charsmax(team))
	add_event(fmt("^"kind^":^"chat^",^"name^":^"%s^",^"team^":^"%s^",^"teamonly^":%d,^"text^":^"%s^"", nesc, team, equal(cmd, "say_team"), esc))
	return PLUGIN_CONTINUE
}

add_event(const body[])
{
	new i = g_EvSeq % MAX_EVENTS
	copy(g_EvBody[i], charsmax(g_EvBody[]), body)
	g_EvTime[i] = get_systime()
	g_EvSeq++
}

write_event_status(f)
{
	for (new n = max(0, g_EvSeq - MAX_EVENTS); n < g_EvSeq; n++)
	{
		new i = n % MAX_EVENTS
		fprintf(f, "{^"t^":^"event^",^"seq^":%d,^"time^":%d,%s}^n", n, g_EvTime[i], g_EvBody[i])
	}
}
