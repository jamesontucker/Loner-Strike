/*
 * loner.inc - Loner-Strike's helper include for AMX Mod X plugins.
 *
 * Makes the common things short: a game mode plugin that fits Loner-Strike's
 * Game Mode Menu (gamemodes.amxx), HUD messages that don't fight other plugins
 * for the screen, giving weapons with ammo, health and armour, teleporting,
 * timers and countdowns, and orders for Loner-Strike's bots.
 *
 *   #include <amxmodx>
 *   #include <loner>
 *
 *   public plugin_init()
 *   {
 *       ls_register_mode("knifeduel", "Knife Duel", "1.0", "me")
 *   }
 *
 * docs/LONER-INC.md has more examples. The Mode Kit parts near the top (roles,
 * phases, zones, loot) are declared now so plugins compile; they come with the
 * future gm_modekit.amxx (docs/GAMEMODES-PLAN.md, section 2.3). Until it runs,
 * ls_kit_running() is false and those calls do nothing.
 *
 * If your plugin has its own plugin_natives(), put
 *   #define LS_OWN_PLUGIN_NATIVES
 * before #include <loner> and call ls_optional() from your plugin_natives().
 */

#if defined _loner_included
	#endinput
#endif
#define _loner_included

#include <amxmodx>
#include <cstrike>
#include <fun>
#include <fakemeta>
#include <ls_components>

#define LS_INC_VERSION "0.1"

/* ---------------------------------------------------------------------------
 * The Mode Kit (future: gm_modekit.amxx, library "modekit")
 *
 * TODO(GAMEMODES-PLAN.md 2.3): these are the kit's natives as planned. They
 * compile now; the plugin still loads without the kit (ls_optional), and
 * every ls_ helper checks ls_kit_running() first. Call them yourself only
 * after checking ls_kit_running().
 * ------------------------------------------------------------------------- */

#pragma reqlib modekit
#if !defined AMXMODX_NOAUTOLOAD
	#pragma loadlib modekit
#endif

/* Definition: the mode's .mode.ini (2.2) */
native mk_mode_id(out[], len);
native mk_get(const section[], const key[], out[], len);
/* Roles */
native mk_role_set(id, const role[]);
native mk_role(id, out[], len);
native mk_role_count(const role[]);
/* Phases */
native mk_phase_set(const name[], secs);
native mk_phase(out[], len);
/* Zones */
native mk_zone_set(const Float:center[3], Float:radius, const Float:next_center[3], Float:next_radius, Float:shrink_secs);
native bool:mk_zone_in(id);
/* Loot */
native mk_loot_spawn(const table[], const Float:origin[3]);
/* Objectives */
native mk_obj_add(const type[], const params[]);
native mk_obj_progress(obj, id);
/* Equipment: "ak47 vesthelm he" */
native mk_give(id, const items[]);
/* HUD manager (owns the channels when it runs) */
native mk_hud(id, const slot[], const text[], Float:secs);
native mk_bar(id, secs);
/* Events */
native mk_event_fire(const name[]);
/* Creatures (6.1) */
native mk_creature_spawn(const type[], const Float:origin[3], flags = 0);
native mk_creature_count();

/* TODO(GAMEMODES-PLAN.md 4.1): the brain's mode goals, through the kit's bot bridge */
native mk_bot_goal(id, const kind[], const Float:pos[3], ent = 0, prio = 0);
native mk_bot_profile(id, const profile[]);
native mk_bot_enemy_class(const classname[]);

/* TODO(GAMEMODES-PLAN.md 2.3): the kit's forwards, for when it runs:
 * forward mk_role_assigned(id, const role[]);
 * forward mk_phase_changed(const from[], const to[]);
 * forward mk_obj_done(obj, id);
 * forward mk_event(const name[]);
 * forward mk_creature_killed(ent, killer);
 */

/**
 * Is the Mode Kit running on this server?
 */
stock bool:ls_kit_running()
{
	return bool:LibraryExists("modekit", LibType_Library);
}

/**
 * Lets the plugin load without the Mode Kit. loner.inc calls it for you from
 * plugin_natives(), unless you #define LS_OWN_PLUGIN_NATIVES (then call it
 * from yours).
 */
stock ls_optional()
{
	set_module_filter("ls__module_filter");
	set_native_filter("ls__native_filter");
}

public ls__module_filter(const library[], LibType:type)
{
	return equal(library, "modekit") || equal(library, "gm_modes") || equal(library, "ls_components") ? PLUGIN_HANDLED : PLUGIN_CONTINUE;
}

public ls__native_filter(const name[], index, trap)
{
	/* missing at load: fine; a call made anyway does nothing */
	if (equal(name, "mk_", 3) || equal(name, "gm_modes_", 9) || equal(name, "ls_", 3))
		return PLUGIN_HANDLED;
	return PLUGIN_CONTINUE;
}

#if !defined LS_OWN_PLUGIN_NATIVES
public plugin_natives()
{
	ls_optional();
}
#endif

/* ---------------------------------------------------------------------------
 * Game modes
 *
 * A mode is a line in configs/gamemodes/modes.ini plus <id>.plugins.ini (the
 * plugins that load in it) and <id>.cfg (its settings). Your mode plugin goes
 * in <id>.plugins.ini, so it only runs in that mode. See docs/LONER-INC.md.
 * ------------------------------------------------------------------------- */

/* The Game Mode Menu's own natives (gamemodes.amxx, library "gm_modes"). */
native gm_modes_count();
native gm_modes_info(mode, id[], idlen, name[], namelen);
native gm_modes_current();

stock g_lsModeId[32];

/**
 * Registers a game mode plugin: register_plugin() with the mode's name, and
 * remembers which mode id it belongs to (for ls_mode_active()).
 *
 * @param id        The mode's id, as in modes.ini ("knifeduel")
 * @param name      The plugin's name, shown in amxx plugins
 * @param version   The plugin's version
 * @param author    Who made it
 * @return          The plugin's id from register_plugin
 */
stock ls_register_mode(const id[], const name[], const version[], const author[])
{
	copy(g_lsModeId, charsmax(g_lsModeId), id);
	return register_plugin(name, version, author);
}

/**
 * The id of the mode playing now ("classic", "csdm", "knifeduel"...).
 *
 * @param out       Where the id goes
 * @param len       Room in out
 * @return          true if the Game Mode Menu answered, false without it
 */
stock bool:ls_mode_current(out[], len)
{
	out[0] = EOS;
	if (!LibraryExists("gm_modes", LibType_Library))
		return false;
	new mode = gm_modes_current();
	if (mode < 0)
		return false;
	new name[2];
	return gm_modes_info(mode, out, len, name, 0) != 0;
}

/**
 * Is the mode given to ls_register_mode() the one playing now? True without
 * the Game Mode Menu (the plugin was loaded by hand, so it's wanted).
 */
stock bool:ls_mode_active()
{
	if (!g_lsModeId[0] || !LibraryExists("gm_modes", LibType_Library))
		return true;
	new cur[32];
	ls_mode_current(cur, charsmax(cur));
	return bool:equali(cur, g_lsModeId);
}

/* ---------------------------------------------------------------------------
 * HUD messages that share the screen
 *
 * The game has 4 HUD text channels for everyone. Each slot here uses its own
 * sync object, so AMX Mod X keeps the slot on one channel per player and other
 * plugins' messages don't wipe it.
 * ------------------------------------------------------------------------- */

enum LsHud
{
	LS_HUD_TOP = 0,     /* top middle: timers, phase, scores */
	LS_HUD_LEFT,        /* left side: objective, team info */
	LS_HUD_RIGHT,       /* right side: counts, notes */
	LS_HUD_CENTER       /* middle of the screen: events, warnings */
};

stock g_lsHudSync[LsHud];
stock const Float:LS_HUD_POS[LsHud][2] = { {-1.0, 0.08}, {0.02, 0.25}, {0.80, 0.25}, {-1.0, 0.35} };
stock const LS_HUD_RGB[LsHud][3] = { {255, 220, 90}, {200, 230, 200}, {200, 230, 200}, {255, 120, 80} };

/**
 * Shows text in a HUD slot, in the slot's own colour.
 *
 * @param id        A player, or 0 for everyone
 * @param slot      LS_HUD_TOP, LS_HUD_LEFT, LS_HUD_RIGHT or LS_HUD_CENTER
 * @param secs      How long it stays
 * @param fmt       The text, with format() codes
 */
stock ls_hud(id, LsHud:slot, Float:secs, const fmt[], any:...)
{
	new msg[192];
	vformat(msg, charsmax(msg), fmt, 5);
	ls_hud_rgb(id, slot, LS_HUD_RGB[slot][0], LS_HUD_RGB[slot][1], LS_HUD_RGB[slot][2], secs, "%s", msg);
}

/**
 * ls_hud() in a colour of your own.
 */
stock ls_hud_rgb(id, LsHud:slot, r, g, b, Float:secs, const fmt[], any:...)
{
	new msg[192];
	vformat(msg, charsmax(msg), fmt, 8);
	if (!g_lsHudSync[slot])
		g_lsHudSync[slot] = CreateHudSyncObj();
	set_hudmessage(r, g, b, LS_HUD_POS[slot][0], LS_HUD_POS[slot][1], 0, 0.0, secs, 0.1, 0.2, -1);
	ShowSyncHudMsg(id, g_lsHudSync[slot], "%s", msg);
}

/**
 * Clears a HUD slot.
 */
stock ls_hud_clear(id, LsHud:slot)
{
	if (g_lsHudSync[slot])
		ClearSyncHud(id, g_lsHudSync[slot]);
}

/**
 * A big announcement in the middle of the screen (director HUD: it doesn't use
 * one of the 4 channels, but keep it short and rare).
 */
stock ls_announce(id, Float:secs, const fmt[], any:...)
{
	new msg[128];
	vformat(msg, charsmax(msg), fmt, 4);
	set_dhudmessage(255, 200, 60, -1.0, 0.25, 0, 0.0, secs, 0.2, 0.4);
	show_dhudmessage(id, "%s", msg);
}

/**
 * A chat line with a green tag, like Loner-Strike's own plugins.
 *
 * @param id        A player, or 0 for everyone
 */
stock ls_chat(id, const fmt[], any:...)
{
	new msg[180];
	vformat(msg, charsmax(msg), fmt, 3);
	client_print_color(id, print_team_default, "^4[Server]^1 %s", msg);
}

/* ---------------------------------------------------------------------------
 * Players
 * ------------------------------------------------------------------------- */

/**
 * A connected, living player (not a dead one, not an empty slot).
 */
stock bool:ls_alive(id)
{
	return 1 <= id <= MaxClients && is_user_connected(id) && is_user_alive(id);
}

/**
 * Gives a weapon with spare ammo.
 *
 * @param id        The player
 * @param weapon    "weapon_ak47", or just "ak47"
 * @param ammo      Spare bullets (not counting the loaded magazine); -1 leaves the default
 * @return          The weapon's entity, or 0 if it couldn't be given
 */
stock ls_give_weapon(id, const weapon[], ammo = -1)
{
	if (!ls_alive(id))
		return 0;
	new name[32];
	if (equal(weapon, "weapon_", 7))
		copy(name, charsmax(name), weapon);
	else
		formatex(name, charsmax(name), "weapon_%s", weapon);
	new ent = give_item(id, name);
	new csw = get_weaponid(name);
	if (ent > 0 && csw > 0 && ammo >= 0 && csw != CSW_KNIFE)
		cs_set_user_bpammo(id, csw, ammo);
	return ent > 0 ? ent : 0;
}

/**
 * Takes every weapon away, then gives the knife back.
 */
stock ls_strip(id, bool:keep_knife = true)
{
	if (!ls_alive(id))
		return;
	strip_user_weapons(id);
	if (keep_knife)
		give_item(id, "weapon_knife");
}

/**
 * Sets health (and the maximum, so medkits and regen go up to it).
 */
stock ls_set_health(id, hp)
{
	if (!ls_alive(id))
		return;
	set_pev(id, pev_max_health, float(max(hp, 1)));
	set_user_health(id, max(hp, 1));
}

/**
 * Sets armour, with or without a helmet.
 */
stock ls_set_armor(id, armor, bool:helmet = true)
{
	if (!is_user_connected(id))
		return;
	cs_set_user_armor(id, clamp(armor, 0, 999), armor <= 0 ? CS_ARMOR_NONE : (helmet ? CS_ARMOR_VESTHELM : CS_ARMOR_KEVLAR));
}

/**
 * Moves a player to a spot and stops them there.
 *
 * @param origin    Where to (GoldSrc units, the player's middle)
 * @param angles    Which way to face, or leave out to keep looking where they were
 */
stock ls_teleport(id, const Float:origin[3], const Float:angles[3] = {0.0, 0.0, 0.0}, bool:turn = false)
{
	if (!ls_alive(id))
		return;
	engfunc(EngFunc_SetOrigin, id, origin);
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0});
	if (turn)
	{
		set_pev(id, pev_angles, angles);
		set_pev(id, pev_v_angle, angles);
		set_pev(id, pev_fixangle, 1);
	}
}

/**
 * Moves a player to a team (CS_TEAM_T, CS_TEAM_CT, CS_TEAM_SPECTATOR).
 * The scoreboard updates now; the model changes on the next spawn.
 */
stock ls_set_team(id, CsTeams:team)
{
	if (is_user_connected(id))
		cs_set_user_team(id, team);
}

/* A role is a word for what a player is in your mode ("hunter", "runner").
 * Kept here until the Mode Kit runs, then the kit's roles are used. */
stock g_lsRole[33][24];

/**
 * Gives a player a role in your mode.
 */
stock ls_set_role(id, const role[])
{
	if (!(1 <= id <= MaxClients))
		return;
	copy(g_lsRole[id], charsmax(g_lsRole[]), role);
	if (ls_kit_running())
		mk_role_set(id, role);
}

/**
 * The player's role, or "" if they have none.
 */
stock ls_role(id, out[], len)
{
	out[0] = EOS;
	if (1 <= id <= MaxClients)
		copy(out, len, g_lsRole[id]);
}

/**
 * Is the player in this role?
 */
stock bool:ls_is_role(id, const role[])
{
	return 1 <= id <= MaxClients && bool:equali(g_lsRole[id], role);
}

/**
 * How many living players have this role.
 */
stock ls_count_role(const role[])
{
	new n;
	for (new i = 1; i <= MaxClients; i++)
		if (ls_alive(i) && equali(g_lsRole[i], role))
			n++;
	return n;
}

/**
 * How many living players are on a team (CS_TEAM_UNASSIGNED counts everyone).
 */
stock ls_count_alive(CsTeams:team = CS_TEAM_UNASSIGNED)
{
	new n;
	for (new i = 1; i <= MaxClients; i++)
		if (ls_alive(i) && (team == CS_TEAM_UNASSIGNED || cs_get_user_team(i) == team))
			n++;
	return n;
}

/* ---------------------------------------------------------------------------
 * Timers and countdowns
 * ------------------------------------------------------------------------- */

#define LS_TASK_COUNTDOWN 9474100

stock g_lsCdLeft;
stock g_lsCdText[64];
stock g_lsCdDone[32];

/**
 * Calls a public function of yours after some seconds.
 *
 * @param secs      How long to wait
 * @param func      The public function's name
 * @param id        Passed to func as its only argument (a player, usually)
 */
stock ls_after(Float:secs, const func[], id = 0)
{
	set_task(secs, func, id);
}

/**
 * Calls a public function of yours every few seconds until ls_stop(id).
 */
stock ls_every(Float:secs, const func[], id = 0)
{
	set_task(secs, func, id, _, _, "b");
}

/**
 * Stops ls_after() and ls_every() timers that were given this id.
 */
stock ls_stop(id = 0)
{
	remove_task(id);
}

/**
 * A countdown everyone sees in the top HUD slot ("Round starts in 5"), then
 * calls your public function. A new countdown replaces the last one.
 *
 * @param secs      From how many seconds
 * @param text      The words before the number
 * @param done      Your public function to call at zero ("" for none)
 */
stock ls_countdown(secs, const text[], const done[] = "")
{
	ls_countdown_stop();
	g_lsCdLeft = max(secs, 1);
	copy(g_lsCdText, charsmax(g_lsCdText), text);
	copy(g_lsCdDone, charsmax(g_lsCdDone), done);
	ls__countdown_tick();
	set_task(1.0, "ls__countdown_tick", LS_TASK_COUNTDOWN, _, _, "b");
}

/**
 * Stops the countdown without calling its function.
 */
stock ls_countdown_stop()
{
	remove_task(LS_TASK_COUNTDOWN);
	g_lsCdLeft = 0;
}

/* (inside ls_countdown: called every second) */
public ls__countdown_tick()
{
	if (g_lsCdLeft <= 0)
	{
		remove_task(LS_TASK_COUNTDOWN);
		if (g_lsCdDone[0] && callfunc_begin(g_lsCdDone) == 1)
			callfunc_end();
		ls_hud_clear(0, LS_HUD_TOP);
		return;
	}
	ls_hud(0, LS_HUD_TOP, 1.1, "%s %d", g_lsCdText, g_lsCdLeft);
	if (g_lsCdLeft <= 3)
		client_cmd(0, "spk buttons/blip1");
	g_lsCdLeft--;
}

/* ---------------------------------------------------------------------------
 * Bot orders (Loner-Strike's bots: the ReGameDLL fork's ls_bot command)
 *
 * These work today. An order lasts until it's done, a fight starts or
 * ls_bot_release(); the bot picks it back up after a fight.
 * ------------------------------------------------------------------------- */

/**
 * Is this player one of the game's bots?
 */
stock bool:ls_is_bot(id)
{
	return is_user_connected(id) && bool:is_user_bot(id);
}

/**
 * Sends a bot to a spot. It walks there along the nav and holds.
 */
stock ls_bot_goto(bot, const Float:origin[3])
{
	if (ls_is_bot(bot))
		server_cmd("ls_bot %d goto %.0f %.0f %.0f", bot, origin[0], origin[1], origin[2]);
}

/**
 * Makes a bot stay where it is for some seconds.
 */
stock ls_bot_hold(bot, secs = 20)
{
	if (ls_is_bot(bot))
		server_cmd("ls_bot %d hold %d", bot, secs);
}

/**
 * Makes a bot follow a player for some seconds.
 */
stock ls_bot_follow(bot, who, secs = 30)
{
	if (ls_is_bot(bot) && is_user_connected(who))
		server_cmd("ls_bot %d follow %d %d", bot, who, secs);
}

/**
 * Makes a bot look at a spot for some seconds.
 */
stock ls_bot_look(bot, const Float:origin[3], Float:secs = 2.0)
{
	if (ls_is_bot(bot))
		server_cmd("ls_bot %d look %.0f %.0f %.0f %.1f", bot, origin[0], origin[1], origin[2], secs);
}

/**
 * Lets a bot go back to playing on its own.
 */
stock ls_bot_release(bot)
{
	if (ls_is_bot(bot))
		server_cmd("ls_bot %d release", bot);
}
