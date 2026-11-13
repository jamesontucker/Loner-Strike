/*
 * ls_components.inc - natives and forwards of gm_components.amxx, Loner-Strike's
 * game mode components (docs/COMPONENTS.md, docs/GAMEMODES-PLAN.md section 2).
 *
 * A component is an ls_* entity: placed in the map, listed in a map pack
 * (configs/components/<map>.ini), or made by a mode with ls_comp_create().
 * gm_components runs its logic, tells Loner-Strike's bots what it means, and
 * calls the ls_on_* forwards below in every plugin that has them.
 *
 * loner.inc includes this file. Without gm_components running, these natives
 * are missing: check ls_components_running() before calling them (loner.inc's
 * native filter lets a plugin load anyway).
 *
 * Words used below:
 *   side   a team (1 Terrorists, 2 Counter-Terrorists), or a player's index
 *          on a capture point with ffa=1 (every player for themselves)
 *   ent    a component's entity index
 */

#if defined _ls_components_included
	#endinput
#endif
#define _ls_components_included

#define LS_COMPONENTS_VERSION "1"

#pragma reqlib ls_components
#if !defined AMXMODX_NOAUTOLOAD
	#pragma loadlib ls_components
#endif

/**
 * Is gm_components running on this server?
 */
stock bool:ls_components_running()
{
	return bool:LibraryExists("ls_components", LibType_Library);
}

/* ---------------------------------------------------------------------------
 * Any component
 * ------------------------------------------------------------------------- */

/**
 * Makes a component at a spot, as if the map had it.
 *
 * @param classname     "ls_capture_point", "ls_loot_container", ...
 * @param origin        Where (GoldSrc units)
 * @param keys          Its keys as text: "radius=160 capture_secs=8 name=^"B^""
 * @return              The entity, or 0 if the classname isn't a component
 */
native ls_comp_create(const classname[], const Float:origin[3], const keys[] = "");

/** Removes a component (and anything it spawned). */
native ls_comp_remove(ent);

/** Is this entity a component? */
native bool:ls_comp_is(ent);

/**
 * A component's classname.
 * @return              1 if ent is a component, 0 if not (out is "")
 */
native ls_comp_kind(ent, out[], len);

/**
 * The next component of a kind after start (0 to begin). "" finds any kind.
 *   for (new e = ls_comp_find("ls_capture_point"); e; e = ls_comp_find("ls_capture_point", e)) ...
 * @return              The entity, or 0 when there are no more
 */
native ls_comp_find(const classname[], start = 0);

/** The next component whose targetname is this, after start (0 to begin). */
native ls_comp_by_name(const targetname[], start = 0);

/** How many components of a kind there are ("" counts all). */
native ls_comp_count(const classname[] = "");

/**
 * Reads one of a component's keys as text.
 * @return              1 if the key is set, 0 if not (out is "")
 */
native ls_comp_get(ent, const key[], out[], len);

/** Reads one of a component's keys as a number. */
native Float:ls_comp_getf(ent, const key[], Float:def = 0.0);

/** Changes a key; the component picks the new value up straight away. */
native ls_comp_set(ent, const key[], const value[]);

/** Switches a component on or off (off: no logic, no visuals, no bot hints). */
native ls_comp_enable(ent, bool:on);

/** Is a component switched on? */
native bool:ls_comp_enabled(ent);

/**
 * Starts what a component does over time: a zone shrinker's stages, a wave
 * spawner's first wave, a supply drop's fall, a meeting, a platform's ride,
 * a trap. ent 0 starts every component that waits for a start (start=manual).
 */
native ls_comp_start(ent);

/** Stops what ls_comp_start() started. ent 0 stops all. */
native ls_comp_stop(ent);

/** Puts a component back as it was at the round's start. ent 0 resets all. */
native ls_comp_reset(ent = 0);

/**
 * Loads a map pack file (path under addons/amxmodx/configs/).
 * @return              How many components it made
 */
native ls_comp_load_pack(const path[]);

/** Is the player inside a component's area (capture point, zone, extraction...)? */
native bool:ls_comp_inside(ent, id);

/** Do Loner-Strike's bots understand ls_mode hints on this server? */
native bool:ls_comp_brain();

/**
 * Sends one ls_mode line to the bots' brain ("goal all defend 10 20 30").
 * @return              1 if sent, 0 if the brain doesn't take ls_mode
 */
native ls_comp_bots_hint(const line[]);

/**
 * Gives an item by name: "ak47", "weapon_m4a1", "vest", "vesthelm", "he",
 * "flash", "smoke", "nvg", "defuse", "ammo" (fills every gun), "hp:50",
 * "armor:100", "money:800".
 * @return              1 if given
 */
native ls_give_item(id, const item[]);

/* ---------------------------------------------------------------------------
 * ls_capture_point
 * ------------------------------------------------------------------------- */

/** Who holds it: a team, a player (ffa=1), or 0 for nobody. */
native ls_cp_owner(ent);
/** How far the side taking it has got, 0.0 to 1.0. */
native Float:ls_cp_progress(ent);
/** The side taking it now: 0 nobody, -1 contested. */
native ls_cp_capturer(ent);
/** Gives it to a side (0 = nobody) without the capture time. */
native ls_cp_set_owner(ent, side);
/** Nobody can take it for some seconds (0 unlocks). */
native ls_cp_lock(ent, Float:secs);

/* ---------------------------------------------------------------------------
 * ls_safe_zone and ls_zone_shrinker
 * ------------------------------------------------------------------------- */

/** Moves the safe zone at once. */
native ls_zone_set(ent, const Float:center[3], Float:radius);
/** Shrinks (or grows) the safe zone to a circle over some seconds. */
native ls_zone_shrink(ent, const Float:center[3], Float:radius, Float:secs);
/** The safe zone's radius now. */
native Float:ls_zone_radius(ent);
/** The safe zone's centre now. */
native ls_zone_center(ent, Float:out[3]);
/** Is the player inside the safe zone (ent 0: inside every safe zone)? */
native bool:ls_zone_inside(ent, id);
/** Damage per tick for players outside. */
native ls_zone_damage(ent, Float:dmg);
/** The shrinker's stage (ent = shrinker or its zone); 0 before the first. */
native ls_zone_stage(ent);
/** A shrinker goes to its next stage now. */
native ls_zone_skip(ent);

/* ---------------------------------------------------------------------------
 * ls_timed_door
 * ------------------------------------------------------------------------- */

native ls_door_open(ent);
native ls_door_close(ent);
/** Players can't open or close it for some seconds (0 unlocks). */
native ls_door_lock(ent, Float:secs);
native bool:ls_door_is_open(ent);

/* ---------------------------------------------------------------------------
 * ls_trigger_trap
 * ------------------------------------------------------------------------- */

/** Sets the trap off now, as if activator did (0 = the world). */
native ls_trap_fire(ent, activator = 0);
/** Arms or disarms it. */
native ls_trap_arm(ent, bool:armed);

/* ---------------------------------------------------------------------------
 * ls_weapon_spawner, ls_loot_container, ls_supply_drop
 * ------------------------------------------------------------------------- */

/** What the spawner gives ("ak47", "table:rifles"); it respawns with it. */
native ls_spawner_set_item(ent, const item[]);
/** Puts the item back now. */
native ls_spawner_respawn(ent);
/** Fills a container from a loot table (it can be opened again). */
native ls_loot_fill(ent, const table[]);
/** Opens a container for a player now (no wait). */
native ls_loot_open(ent, id);
/**
 * Rolls one item from a loot table (configs/gamemodes/loot.ini).
 * @param tier          Only items of this tier or lower (0 = any)
 * @return              1 if something was rolled
 */
native ls_loot_roll(const table[], out[], len, tier = 0);
/**
 * Calls a supply drop to a spot: announced, falls, lands, can be opened.
 * @return              The crate's entity
 */
native ls_drop_call(const Float:origin[3], const table[] = "");

/* ---------------------------------------------------------------------------
 * ls_barricade
 * ------------------------------------------------------------------------- */

native ls_barricade_hp(ent);
/** Sets its health (0 breaks it; above 0 stands it back up). */
native ls_barricade_set_hp(ent, hp);
/** Repairs it fully (id 0), or one step as if player id repaired it. */
native ls_barricade_repair(ent, id = 0);

/* ---------------------------------------------------------------------------
 * ls_wave_spawner (waves are shared by every spawner on the map)
 * ------------------------------------------------------------------------- */

/** Starts a wave (0 = the next one). */
native ls_wave_start(wave = 0);
/** Stops spawning (what's alive stays). */
native ls_wave_stop();
/** The wave now (0 before the first). */
native ls_wave_current();
/** How many of this wave's enemies are still alive. */
native ls_wave_alive();
/** Counts an entity you spawned in ls_on_wave_spawn() as one of the wave's enemies. */
native ls_wave_track(ent);

/* ---------------------------------------------------------------------------
 * ls_extraction_zone
 * ------------------------------------------------------------------------- */

native ls_extract_open(ent, bool:open);
native bool:ls_extract_is_open(ent);
/** Has this player extracted this round? */
native bool:ls_extracted(id);

/* ---------------------------------------------------------------------------
 * ls_checkpoint
 * ------------------------------------------------------------------------- */

/** The order number of the last checkpoint the player reached (0 = none). */
native ls_checkpoint_of(id);
/** Laps the player has finished. */
native ls_checkpoint_laps(id);
/** Back to the start (id 0 = everyone). */
native ls_checkpoint_reset(id = 0);

/* ---------------------------------------------------------------------------
 * ls_moving_platform
 * ------------------------------------------------------------------------- */

/** Sets the platform off (uses its train/plat). */
native ls_platform_go(ent);
native bool:ls_platform_moving(ent);

/* ---------------------------------------------------------------------------
 * ls_player_class
 * ------------------------------------------------------------------------- */

/** Gives a player a class by name ("" takes it away); applies now if alive. */
native ls_class_set(id, const name[]);
/** The player's class name ("" none). */
native ls_class_get(id, out[], len);
/** Shows the class menu. */
native ls_class_menu(id);
/** The ls_player_class entity with this name, or 0. */
native ls_class_find(const name[]);

/* ---------------------------------------------------------------------------
 * ls_meeting_point
 * ------------------------------------------------------------------------- */

/** Calls a meeting at a meeting point (caller 0 = the server). */
native ls_meeting_call(ent, caller = 0);
/** Ends the meeting now and counts the votes (ent 0 = whichever is on). */
native ls_meeting_end(ent = 0);
native bool:ls_meeting_active();
/** Casts a vote (for bots and modes); target 0 = skip. */
native ls_meeting_vote(voter, target);

/* ---------------------------------------------------------------------------
 * ls_respawn_point
 * ------------------------------------------------------------------------- */

/** Picks a free respawn point for a player's team (0 if none). */
native ls_respawn_pick(id);
/** Respawns a dead player at a respawn point (ent 0 = pick one). */
native ls_respawn_at(id, ent = 0);

/* ---------------------------------------------------------------------------
 * Forwards. Return PLUGIN_HANDLED where it says "handled" to stop the default.
 * ------------------------------------------------------------------------- */

/** A component is ready (from the map, a map pack, or ls_comp_create). */
forward ls_on_component(ent, const kind[]);

/** A side took a capture point; id is a player who was on it (0 if none). */
forward ls_on_capture(ent, side, id);
/** Both sides are on it (once per contest). */
forward ls_on_capture_contested(ent);
/** The side that held it lost it. */
forward ls_on_capture_lost(ent, side);
/** Every second while held: once with id 0, then once per holder standing on it. */
forward ls_on_capture_tick(ent, side, id);

/** A shrinker moved to a stage (ent = the safe zone); 0 = back to the start. */
forward ls_on_zone_stage(ent, stage);
/** A player outside the safe zone is about to take dmg. Handled: no damage. */
forward ls_on_zone_tick(ent, id, Float:dmg);
/** A player crossed the safe zone's edge (outside 1 = left it). */
forward ls_on_zone_cross(ent, id, outside);

/** A damage zone is about to hurt a player. Handled: no damage. */
forward ls_on_damage_zone(ent, id, Float:dmg);

/** A timed door opened (1) or closed (0). */
forward ls_on_door(ent, open);
/** A player (or 0) locked a timed door for some seconds. */
forward ls_on_door_locked(ent, id, Float:secs);

/** A trap went off; victims = players hurt by its own damage. */
forward ls_on_trap(ent, activator, victims);

/** A player is taking a spawner's item. Handled: not given (the item still goes). */
forward ls_on_weapon_taken(ent, id, const item[]);
/** A spawner's item came back. */
forward ls_on_weapon_respawn(ent);

/** A player opened a container or crate. Handled: nothing given. */
forward ls_on_loot(ent, id);
/** One item from a container is about to be given. Handled: skip it. */
forward ls_on_loot_item(ent, id, const item[]);

/** A supply drop: stage 0 announced, 1 landed, 2 opened. */
forward ls_on_drop(ent, stage, const Float:origin[3]);

forward ls_on_barricade_damaged(ent, attacker, hp);
forward ls_on_barricade_broken(ent, attacker);
forward ls_on_barricade_repaired(ent, id, hp);

forward ls_on_wave_start(wave);
/**
 * A spawner wants one enemy at origin. Handled: you spawned it (call
 * ls_wave_track on it). Otherwise a dead player of the spawner's team is
 * respawned there (bots first).
 */
forward ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[]);
forward ls_on_wave_cleared(wave);

forward ls_on_extract_open(ent, open);
/** A player started extracting (secs = how long it takes). */
forward ls_on_extracting(ent, id, Float:secs);
/** A player extracted. Handled: skip the zone's own action (spectate/strip). */
forward ls_on_extract(ent, id);

forward ls_on_checkpoint(ent, id, order);
forward ls_on_lap(id, lap, Float:secs);

/** A platform started moving (1) or stopped (0). */
forward ls_on_platform(ent, moving);

/** A player is getting a class. Handled: refused. */
forward ls_on_class(id, const name[]);

/** A player spawned and is being moved to a respawn point. Handled: stays put. */
forward ls_on_respawn_point(ent, id);

forward ls_on_meeting_start(ent, caller);
forward ls_on_meeting_vote(ent, voter, target);
/** A meeting ended; ejected = the player voted out (0 nobody). */
forward ls_on_meeting_end(ent, ejected);
