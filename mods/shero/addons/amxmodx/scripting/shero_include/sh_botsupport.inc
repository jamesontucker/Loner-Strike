/* SuperHero Mod - CZ bot support natives
 * Provided by the patched core: scripting/shero/superheromodnvault.sma
 */
#if defined _sh_botsupport_included
	#endinput
#endif
#define _sh_botsupport_included

// Number of registered heroes (hero IDs are 0 .. num-1)
native sh_get_num_heroes();

// Copies the hero's name. Returns 0 on invalid heroID.
native sh_get_hero_name(heroID, name[], len);

// Hero ID bound to +power<key> for this player, or -1.
native sh_get_user_bind(id, key);

// Presses (down=1) or releases (down=0) +power<key> for a player, exactly like the client command.
native sh_bot_power_key(id, key, down);

// When blocked, bots will never pick this hero automatically.
native sh_set_hero_botblock(heroID, bool:blocked);

// Makes a bot pick heroes for all its free slots now.
native sh_bot_pick_powers(id);
