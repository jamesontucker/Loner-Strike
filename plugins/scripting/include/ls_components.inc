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

/** Mode map kits: the next ls_marker named spot of this kind (lst_shop, br_drop, ...) after start
    (0 to begin; "" any kind). Markers whose kind is a component classname are that component. */
native ls_marker_find(const kind[], start = 0);

/** A marker's key (size, radius, angle, any other); false if it has none. */
native bool:ls_marker_get(ent, const key[], out[], len);

/** A big announcement on the shared toast line (one at a time per player, queued; the same text
    again, or replace 1, shows at once in place: a countdown). id 0: everyone. */
native ls_toast(id, Float:secs, const text[], r = 255, g = 200, b = 60, replace = 0);

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

/** A component's state: a loot crate 0 full / 1 empty, a weapon spawner 0 there / 1 taken,
 *  an extraction 1 open, a barricade 0 standing / 1 broken, a supply crate 0 falling / 1 landed / 2 opened. */
native ls_comp_state(ent);

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
/**
 * When the safe zone changes next (ent 0 = the first zone).
 * @param center    the next circle's centre
 * @param radius    the next circle's radius
 * @param closing   1 while it's shrinking (the return is then the time until it stops)
 * @return          seconds, or -1.0 when nothing is scheduled
 */
native Float:ls_zone_next(ent, Float:center[3], &Float:radius, &closing);

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

/* ==== W3: Battle Royale pieces ==== */
/* ---------------------------------------------------------------------------
 * Battle Royale pieces (any mode can use them)
 * ------------------------------------------------------------------------- */

/* Healing items: "bandage" (+15, up to 75, 3 s), "medkit" (full health, 7 s), "drink" (+40 over 10 s and
 * a little speed, 4 s). As loot items too ("bandage", "bandage:3", "medkit", "drink"). With the cvar
 * ls_heal_carry 1 they're carried (limits: ls_heal_limits "6 2 3") and used with F (ls_heal_key), say /heal
 * or the console command "ls_heal [kind]", with a progress bar: no shooting meanwhile, moving stops it.
 * With ls_heal_carry 0 (the default) they heal at once when picked up. */

/** Gives a player n of a kind, up to the carry limit. @return how many they took */
native ls_heal_give(id, const kind[], n = 1);
/** How many of a kind a player carries ("" = all kinds). */
native ls_heal_count(id, const kind[] = "");
/** Starts using one ("" = the best for their health now). @return 1 if started */
native ls_heal_use(id, const kind[] = "");
/** Stops using one (nothing used up). */
native ls_heal_cancel(id);
/** Is the player using a healing item now? */
native bool:ls_heal_using(id);
/** A healing item: stage 0 starting (handled: refused), 1 used, 2 stopped before the end. */
forward ls_on_heal(id, const kind[], stage);

/* Armor levels (cvar ls_armor_levels 1): vests and helmets 1-3, loot items "vest1".."vest3" and
 * "helmet1".."helmet3" ("vest" is a level-1 vest, "vesthelm" both at level 1). A vest has 100/120/150 armor
 * points and lets 100/85/70 % of a body hit through on top of the game's own vest; a helmet lets 70/60/45 %
 * of a head hit through and breaks after 100/150/200 of head damage. A higher level is picked up over a
 * lower one, the same level only when yours is half gone. Levels off: vestN/helmetN are plain armor. */

/** Sets a player's armor levels 0-3 (-1 keeps one); a new vest comes with its full armor points. */
native ls_armor_set(id, vest, helmet);
/** A player's vest and helmet levels. @return the vest's armor points left */
native ls_armor_get(id, &vest, &helmet);
/** A player's armor levels changed (picked up, set, or one broke: level 0). */
forward ls_on_armor(id, vest, helmet);

/* Scarce ammo (cvar ls_ammo_scarce 1): a gun found (spawner, crate) comes with its magazine and ls_ammo_mags
 * more (1); the loot item "ammo" gives two magazines for each gun carried. Typed ammo works either way:
 * "ammo:762", "ammo:556", "ammo:9mm", "ammo:45", "ammo:338", "ammo:57", "ammo:50", "ammo:357", "ammo:12g",
 * "ammo:box" (M249) or "ammo:<weapon>", two magazines' worth, or "ammo:762:60" for 60 rounds (ls_give_item). */

/* Rarity glow: 1 white, 2 green, 3 blue, 4 purple, 5 gold. Crates and spawners glow by tier; with the cvar
 * ls_rarity_glow 1 a spawner glows by its item's own rarity and guns players drop glow too. */

/** An item's rarity 1-5 (the highest tier the loot tables give it, else by what it is). */
native ls_item_tier(const item[]);
/** Makes any entity glow in a rarity's colour (0 = no glow). */
native ls_glow(ent, tier);

/* Death crates (cvar ls_death_crates 1): a killed player drops one crate with everything they had: guns with
 * their magazine and spare ammo ("ak47@30@60"), grenades, healing items ("bandage:3"), armor ("vest2@85",
 * "helmet1@60"). It glows by the best thing in it; USE opens a menu (pick one: a gun in that slot goes into
 * the crate in its place; or take everything that fits). Bots by a crate take what helps them. Empty crates go. */

/** Makes a crate anywhere with items ("ak47@30@60 bandage:2 vest2"; any loot item works). @return its entity */
native ls_crate_make(const Float:origin[3], const items[] = "", const name[] = "");
/** Puts one more item in a crate. */
native ls_crate_add(ent, const item[]);
/** How many items are in a crate. */
native ls_crate_count(ent);
/** A crate's item i (0-based), as text. */
native ls_crate_item(ent, i, out[], len);
/** Opens a crate's menu for a player. */
native ls_crate_open(id, ent);
/** A death crate was made for a player who died (victim) or by ls_crate_make (victim 0... the maker's choice). */
forward ls_on_death_crate(ent, victim);
/** A player is taking an item from a crate. Handled: refused. */
forward ls_on_crate_take(ent, id, const item[]);

/* Match HUD (cvar ls_match_hud 1): for each person, at the top: players left, their kills, the zone's
 * timer (or the mode's own words), their armor levels and healing items; on the right: the kill feed and
 * their squad with health. A player who's killed watches their killer. */

/** Switches the match HUD on or off (same as the cvar ls_match_hud). */
native ls_match_hud(bool:on);
/** The mode's words in place of the zone timer ("Loot up: fighting in 12 s"); "" shows the zone again. */
native ls_match_hud_text(const text[]);
/** A line in the kill feed. */
native ls_feed(const text[]);
/** A player's kills this round (counted while the match HUD is on). */
native ls_kills(id);
/** A dead player watches someone (first person). @return 1 if done */
native ls_spectate(id, target);

/* Drop-in: a player starts high over a spot (as high as the sky allows, up to ls_drop_height, 3000), falls
 * at 0.2 gravity steering as they like (bots come straight down), takes no damage until they land, then
 * ls_on_landed. Landing spots: the map's br_drop markers (Mode map kits; `layer` below/surface/high tags
 * them), else its ls_respawn_points; nobody lands on someone else. */

/** Drops a player in over a spot (0 0 0: a spread-out landing spot); height 0 = ls_drop_height.
    @return 0 if there's no sky over it (they stay where they are) */
native ls_drop_in(id, const Float:spot[3] = {0.0, 0.0, 0.0}, Float:height = 0.0);
/** Is the player still dropping in? */
native bool:ls_dropping(id);
/** The open landing spot nearest a point ("below"/"surface"/"high" markers first when given), spots
    others already drop on counting 1500 u further. @return false if the map has none */
native bool:ls_drop_spot(const Float:near[3], Float:out[3], const layer[] = "");
/** A new game: the landing spots taken are forgotten (and anyone still falling lands). */
native ls_drop_reset();
/** A player who dropped in is down on the ground. */
forward ls_on_landed(id);

/* Squads: squad mates can't hurt each other (ls_squad_ff 0), see markers over each other's heads and dots on
 * the radar (ls_squad_markers 1), and the match HUD lists them with health. Bots are told who's a friend.
 * A lobby (ls_squad_lobby) lets people team up with say /squad: start a squad, invite someone, leave, or
 * join with a party code (say /squad ABCD). When it ends the squads are filled: people from one address or
 * LAN together, people alone into squads that are short, then bots. The server can't see Steam friends. */

/** Puts a player in a squad (1-32; 0 none). */
native ls_squad_set(id, squad);
/** A player's squad (0 none). */
native ls_squad_get(id);
/** Sets the squad size (1 = solo, 2 duos, 4 squads...). @return the size before */
native ls_squad_max(size = -1);
/** How many are in a squad (alive only, if asked). */
native ls_squad_count(squad, bool:alive = false);
/** How many sides have someone alive: squads, and players in none. last = one of them. */
native ls_squads_alive(&last = 0);
/** Fills the squads now (as at the end of the lobby). @return the highest squad number */
native ls_squad_fill();
/** A lobby of some seconds for teaming up (0 = fill now); ls_on_lobby_end when it's over. */
native ls_squad_lobby(Float:secs);
/** Opens the squad menu for a player. */
native ls_squad_menu(id);
/** A squad that's short of players with someone alive in it (0 none): for someone joining late. */
native ls_squad_short();
/** A squad's party code. */
native ls_squad_code(squad, out[], len);
/** A player joined a squad (0: left one). */
forward ls_on_squad(id, squad);
/** The squad lobby ended and the squads are filled. */
forward ls_on_lobby_end(squads);
/* ==== end W3 ==== */

/* ==== W1: Last Stand pieces ==== */
/* ---------------------------------------------------------------------------
 * Player modifiers, perks, the downed state, weapon levels; the perk machine
 * (ls_perk_machine), the upgrade bench (ls_upgrade_bench), the mystery box
 * (ls_mystery_box), pickups (ls_pickup), and the paid timed trap (an
 * ls_trigger_trap with fire_on=use cost=... active_secs=...). Bots are offered
 * each buyable one (W2's ls_mode offer) unless it has bots=0.
 * ------------------------------------------------------------------------- */

/**
 * A player modifier, times what their perks give. what: health_max, speed,
 * reload, deploy (time: 0.6 = 40 % faster), damage, revive_time (time),
 * fire_rate (1.25 = 25 % faster). Cleared when they die.
 */
native ls_player_mod(id, const what[], Float:mult);
/** The modifier now (the mode's times the perks'). */
native Float:ls_player_mod_get(id, const what[]);
native ls_player_mod_clear(id);

/** Perks: tough (+50 % max health), hands (faster reloads and swaps), doubletap
 *  (+25 % damage), revive (revive in half the time; alone, get back up once),
 *  feet (+10 % speed). Lost when downed (see ls_down_enable) or dead. */
native ls_perk_give(id, const perk[]);
native bool:ls_perk_has(id, const perk[]);
native ls_perk_count(id);
native ls_perk_clear(id);
/** Buys one with no machine (a menu): 1500 + 1000 per perk owned, 4 at most. */
native bool:ls_perk_buy(id, const perk[], bool:tell = true);
/** That default price for this player now. */
native ls_perk_price(id);
/** "Perks: TGH QH ..." for a HUD; "" with none. */
native ls_perk_tags(id, out[], len);

/**
 * The downed state: on, a hit that would kill a player of `team` (0 any) puts
 * them down instead: 1 HP, nothing hurts them, creatures ignore them, they crawl
 * and bleed out over bleed_secs; a teammate within 80 u holding USE for 4 s
 * (bots: standing there) gets them back up with 50 HP. Off: nobody new goes down.
 */
/* (cvar ls_down_finish 1: a downed player has ls_down_hp health and enemies' hits can finish them) */
native ls_down_enable(bool:on, Float:bleed_secs = 30.0, team = 0, bool:lose_perks = true);
native bool:ls_is_down(id);
/** Puts a player down now. */
native ls_down(id);
/** Gets a downed player up (by = who did it, 0 nobody). */
native ls_revive(id, by = 0);
/** What getting a teammate up pays the reviver (cvar ls_down_revive_pay; 0 off, once a downing, not for
 *  getting yourself up). Last Stand pays $150. */
native ls_down_revive_pay(amount);
/** Seconds of bleeding left (0 when not down). */
native Float:ls_down_left(id);

/** The level of a weapon (1, 2 Mk II, 3 Mk III); weapon 0 = the one in hand. */
native ls_weapon_level(id, weapon = 0);
native ls_weapon_set_level(weapon, level);
/** An upgrade of the gun in hand at the first bench's prices (or 3000 / 6000). */
native bool:ls_bench_use(id, bool:tell = true);
/** A roll of the box that's out (ent 0) or of that box. */
native bool:ls_box_use(ent, id, bool:tell = true);

/** Drops a pickup; whoever walks over it takes it (ls_on_pickup). Returns its entity. */
native ls_pickup_spawn(const kind[], const Float:origin[3], Float:life = 25.0, const model[] = "", r = 255, g = 255, b = 255, team = 0);

/** A perk is being bought (machine or ls_perk_buy). Handled: refused. */
forward ls_on_perk(id, const perk[], price);
/** A player is going down. Handled: they die as usual. */
forward ls_on_down(id, attacker);
/** A downed player is back up (by = who, the player themselves for Quick revive, 0 a mode). */
forward ls_on_revive(id, by);
/** A downed player bled out (they're killed right after). */
forward ls_on_bled_out(id);
forward ls_on_weapon_upgraded(id, weapon, level, price);
/** The box is giving a gun (weapon_...). Handled: nothing given, nothing paid. */
forward ls_on_box(ent, id, const weapon[]);
/** A player is taking a pickup. Handled: it stays. */
forward ls_on_pickup(ent, id, const kind[]);
/** A creature took damage (after it was dealt); headshot 1 when the hit was a head. */
forward ls_on_creature_hurt(ent, attacker, Float:dmg, headshot);

/**
 * ls_shop: a counter (a point with a radius, or a brush) where USE opens a menu.
 * Keys: items = "Name|price|give|what; ..." (give: what ls_give_item gives - ammo,
 * vesthelm, hp:50, hegrenade, weapon names - or @word, given by the mode in
 * ls_on_shop_buy), price_rise (dearer by this share each wave), scale, upgrades=1
 * (perks and the bench when the map has no machine or bench), buy_menu=1 (the game's
 * buy menu as the last line), keeper (barney, gman, none or a model), keeper_name,
 * greet, hint, title, open, team.
 */
native bool:ls_shop_buy(ent, id, const item[], bool:tell = true);
/** An item's price now (by its name or what it gives); -1 if the shop has none. */
native ls_shop_price(ent, const item[]);
native ls_shop_menu(ent, id);
native ls_shop_open(ent, bool:open);
native bool:ls_shop_is_open(ent);
/** Is the player in the shop? */
native bool:ls_shop_at(ent, id);
/** Inside ls_on_shop_item: the reason the menu shows. */
native ls_shop_why(const reason[]);
/** Can this player buy this item now? Handled: no (the line is greyed; ls_shop_why says why). */
forward ls_on_shop_item(ent, id, const item[]);
/** An item is being bought. Handled: refused. An @item is the mode's to give here. */
forward ls_on_shop_buy(ent, id, const item[], price);

/**
 * Turrets: friendly creatures (the game DLL's friendly=1, owner=; brain ls-next3 cfd2c3d or
 * later) that shoot creatures and that creatures attack. A player places one from a kit (a
 * shop item with give=turret, or ls_turret_give) by pressing USE where they look; a bot is
 * offered one (W2's "offer <bot> turret x y z 0 cost") and pays and places it where it holds
 * USE within 450 u of that point. price 0 = off (no bot offers). life 0 = until destroyed.
 * The type has to be precached (ls_creature_precache).
 */
native ls_turret_setup(price = 2500, Float:life = 150.0, per_player = 1, const type[] = "turret", health = 0, bool:bots = true);
native ls_turret_give(id, n = 1);
native ls_turret_kits(id);
/** Places one where the player looks: pay = at the set price, else from a kit. Returns the entity. */
native ls_turret_place(id, bool:pay = false);
native ls_turret_spawn(owner, const Float:origin[3], Float:yaw = 0.0);
/** Turrets this player has out. */
native ls_turret_count(id);
forward ls_on_turret(ent, owner);

/**
 * The round table (zombie-survival rounds): on, each ls_wave_start plans the round on the wave
 * spawners - how many (11 + 1.4 a round, more the first rounds, by the team's size up to 16),
 * which creature types (unlocked by round: zombie, then runners, spitters, babycrabs, brutes,
 * vortigaunts, assassins, soldiers, controllers, stukabats; weighted by role, with stand-ins the
 * server has), their health (+4 % a round to 9, then x1.1 a round, by team size), a boss (a
 * baby garg at 7, a garg or gonarch every 5 from 10; fewer grunts), a hound pack or snark swarm
 * every 6, heavies from 11, tiers 2/3 from rounds 15/20, and how many alive at once - and the
 * spawners make the creatures (a plugin handling ls_on_wave_spawn still comes first).
 * section: a [section] of configs/gamemodes/waves.ini (boss, boss_every, count_scale, hp_scale,
 * type_hp, "wave N | N-M | N+ = type weight ..." rows) replacing the built-in mix.
 * team: the defenders (creatures hunt them). creatures false: the counts only (dead players of
 * the spawners' team come back as the enemies).
 */
native ls_wave_table(const section[] = "", team = 2, bool:creatures = true);
native ls_wave_table_off();
/** The mode's difficulty: times the count and the health. */
native ls_wave_scale(Float:count, Float:hp);
/** Alive at once (0: 16 + 2 a defender, 20-96); never over the game DLL's adaptive cap. */
native ls_wave_cap(n);
native ls_wave_cap_now();
/** Tiers (yellow from round 15, red-orange from 20) on or off. */
native ls_wave_tiers(bool:on);
/** This round's planned count and how many the table has made so far. */
native ls_wave_planned();
native ls_wave_spawned();
/** 0 a normal round, 1 a boss round (until the boss is out), 2 a special pack round; out = the type. */
native ls_wave_kind(out[], len);
/** The health multiplier this round (for enemies a mode makes itself). */
native Float:ls_wave_hp_mult();
/** The table's pay for a kill of this type. */
native ls_wave_type_pay(const type[]);
/** A boss or a heavy (boss, gonarch, babygarg, the waves.ini boss). */
native bool:ls_wave_is_big(const type[]);
/** Every type rounds 1..upto use, with stand-ins, space-separated (for ls_creature_precache). */
native ls_wave_types(out[], len, upto = 0);
/** False once creatures wouldn't spawn (the table fell back to dead players). */
native bool:ls_wave_creatures_ok();
/* ==== end W1 ==== */

/* ==== NPCs and interactables (T1fix) ==== */
/**
 * ls_npc: a character players talk to (USE). Keys: model (models/barney.mdl, gman, scientist,
 * or any .mdl loaded with the map; a bare word means models/<word>.mdl), name, behavior
 * (stand | wander | patrol | follow), targets (patrol: entity names, space-separated, visited in
 * order), radius (wander range, 160), follow_secs (follow: how long after a talk, 10),
 * unkillable (1), health (killable ones, 100), team (0 anyone, 1 T, 2 CT: who can talk to it),
 * greet (said when someone starts talking), target (set off on a talk), angle.
 * It faces whoever talks to it; bots never target it (no damage taken while unkillable).
 *
 * ls_interactable: any model or brush made usable. Keys: prompt ("Press E to use"), range
 * (96), cooldown (1 s), once (0: again after the cooldown; 1: once a round), hide (1: gone
 * once used, back next round), team (0 anyone, 1 T, 2 CT), target (set off on use), model.
 *
 * Text from maps and modes goes to players only (chat, center text, menus), never to the
 * server console.
 */

/** Someone pressed USE on an NPC (after its greeting). */
forward ls_on_npc_talk(id, npc);
/** Someone used an interactable (its targets fire too). */
forward ls_on_interact(id, ent);
/**
 * A reply to ls_npc_choose: 0-5 the reply picked, -1 timed out, -2 walked away (over 200
 * units from the NPC), died, left, closed the menu, or a new conversation replaced it.
 */
forward ls_on_npc_choice(id, npc, choice);

/** The NPC (0: a narrator) says a line to a player (0: everyone). */
native ls_npc_say(npc, id, const text[]);
/**
 * The NPC (0: a narrator) says a line and the player picks one of up to 6 replies
 * ("Yes|No|Tell me more", separated by |). One conversation per player: a new one closes the
 * old (its choice comes as -2). timeout: seconds (0 = no limit). Returns false when nothing
 * was shown (no such player, no replies).
 */
native bool:ls_npc_choose(npc, id, const text[], const replies[], Float:timeout = 0.0);
/** stand, wander, patrol (its targets) or follow (follows `target` for its follow_secs). */
native bool:ls_npc_behave(npc, const behavior[], target = 0);
/** Turns the NPC to face an entity (a player, another NPC...). */
native bool:ls_npc_look_at(npc, target);
/**
 * An NPC made now. model: models/x.mdl (or barney, gman, scientist...) loaded with the map
 * (see ls_npc_precache); yaw: facing; keys: more keys ("behavior=wander unkillable=1").
 * Returns its entity or 0.
 */
native ls_npc_spawn(const model[], const Float:origin[3], Float:yaw = 0.0, const name[] = "", const keys[] = "");
/** Loads an NPC model with the map: call it from plugin_precache. True when it's loaded. */
native bool:ls_npc_precache(const model[]);
/** An interactable made now (model: a .mdl loaded with the map, or "" for an invisible one). */
native ls_interactable_spawn(const model[], const Float:origin[3], const prompt[] = "", const keys[] = "");
/** The NPC a player is talking to now (0 none or a narrator). */
native ls_npc_talking(id);
/**
 * Bots may use this interactable (on) or not: while it's usable (cooldown over, not used up,
 * on, their team) bots get it as an errand (walk there, press USE). The map key bots=1 does the
 * same from the start.
 */
native bool:ls_interactable_offer(ent, bool:on);
/**
 * Bots may shoot this killable NPC (on) or not (the default; unkillable ones never). The map
 * key bots_attack=1 does the same. Players on its team side hurt it only with mp_friendlyfire.
 */
native bool:ls_npc_targetable(npc, bool:on);
/* Scripted moves (EC): the NPC walks (or runs) to each point in turn along the bot navigation
 * mesh's route when gm_botnav is loaded with a mesh (straight otherwise), facing where it goes,
 * then goes back to its behavior (or stands, NM_STAY). Up to 24 points; speed 20-320. */
#define NM_LOOP 1
#define NM_STAY 2
#define NM_RUN  4
native bool:ls_npc_move(npc, const Float:pts[], n, Float:speed = 90.0, flags = 0);
/** The same along a map's path_track chain (first by targetname, then each one's target). */
native bool:ls_npc_move_path(npc, const first[], Float:speed = 90.0, flags = 0);
/** Stops a scripted move (no forward); stay: it stands instead of its old behavior. */
native bool:ls_npc_move_stop(npc, bool:stay = false);
native bool:ls_npc_moving(npc);
/** A point of a scripted move reached (point: its index); last: the move is over. */
forward ls_on_npc_arrived(npc, point, last);
/** A scripted move gave up: no way there, blocked three times, or out of time. */
forward ls_on_npc_move_failed(npc, point);
/* ==== end NPCs and interactables ==== */

/* ==== Projectiles (T1fix) ==== */
/**
 * A projectile hit something: where, the surface's normal there, what it hit (0 for the world)
 * and, for a player, the hitgroup (1 head ... 7 right leg; 0 when unknown). Its damage and
 * explosion follow; it's removed after (unless it bounced off the world).
 */
forward ls_on_proj_hit(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup);
/** The same hit, for Game Mode Studio's per-projectile On Hit (a plugin uses ls_on_proj_hit). */
forward ls_on_proj_hit_node(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup);
/** A projectile's lifetime ran out with no hit (it's removed after). */
forward ls_on_proj_expire(proj, owner);
/** Loads a projectile's model or sprite with the map: call it from plugin_precache (models/...mdl or sprites/...spr). */
native bool:ls_proj_precache(const model[]);
/**
 * A projectile (0 if none could be made). model: loaded with the map (else a grenade flies);
 * dir needn't be normalized; gravity 0 flies straight, 1 falls like a grenade; life in seconds.
 * keys: trail=1 spin=1 bounce=N (bounces off the world) damage=N radius=N (0: only what it hits)
 * falloff=0|1 explode=1 dangerous=1 (noted for bots; they don't dodge yet) scale=N (a sprite's or a model's size).
 * Kills go to owner. At most 64 fly at once (the oldest goes); all go at a new round.
 */
native ls_proj_spawn(owner, const model[], const Float:origin[3], const Float:dir[3], Float:speed = 1000.0, Float:gravity = 0.0, Float:life = 5.0, const keys[] = "");
/** A player's eyes (a little in front) and the way they aim; false when they aren't alive. */
native bool:ls_proj_aim(id, Float:origin[3], Float:dir[3]);
native bool:ls_proj_remove(proj);
native bool:ls_proj_velocity(proj, Float:velocity[3]);
native bool:ls_proj_set_velocity(proj, const Float:velocity[3]);
/** Its owner (0 none, or not a projectile). */
native ls_proj_owner(proj);
/** How many fly now. */
native ls_proj_count();
/* ==== end Projectiles ==== */
