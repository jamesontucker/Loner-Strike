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

// ---------------------------------------------------------------------------
// Bot helpers. CZ bots never get CurWeapon/AmmoX/ResetHUD/Damage messages, so
// get_user_weapon() (and the weapon from get_user_attacker()) is stale for them.
// ---------------------------------------------------------------------------
#include <fakemeta>
#include <hamsandwich>
#include <cstrike>

// CSW_ id of the weapon the player is really holding (0 if none). Reads m_pActiveItem.
stock sh_get_held_weapon(id, &item = 0)
{
	item = get_pdata_cbase(id, 373)
	return pev_valid(item) ? cs_get_weapon_id(item) : 0
}

// Unlimited ammo for bots (they never send the CurWeapon message heroes refill on).
// Call every 1-2 s. mode: like sh_reload_ammo (0 = server's sh_reloadmode, 1 = full clip,
// 2 = full backpack, 3 = treated as 1). onlyWeapon: CSW_ id to limit to, 0 = any gun.
stock sh_bot_refill_ammo(id, mode = 0, onlyWeapon = 0)
{
	if ( !is_user_alive(id) ) return
	new item, wid = sh_get_held_weapon(id, item)
	if ( !wid || (onlyWeapon && wid != onlyWeapon) ) return
	new slot = sh_get_weapon_slot(wid)
	if ( slot != 1 && slot != 2 ) return

	if ( !mode ) mode = get_cvar_num("sh_reloadmode")
	switch ( mode ) {
		case 0: return
		case 2: {
			if ( cs_get_user_bpammo(id, wid) < sh_get_max_bpammo(wid) ) cs_set_user_bpammo(id, wid, sh_get_max_bpammo(wid))
		}
		default: {
			if ( cs_get_weapon_ammo(item) < sh_get_max_clipammo(wid) ) cs_set_weapon_ammo(item, sh_get_max_clipammo(wid))
		}
	}
}

// ---------------------------------------------------------------------------
// "Damage" event handlers never run when the victim is a bot (the message is only
// sent to real clients). Heroes keep their Damage event for human victims and add
//   RegisterHam(Ham_TakeDamage, "player", "<x>_ham_damage", 1, true)
//   public <x>_ham_damage(victim, inflictor, attacker, Float:damage, damagebits)
//       if ( sh_bot_damage_begin(victim, inflictor, attacker, damage, damagebits) ) { <handler>(victim); sh_bot_damage_end(); }
// and in the handler use sh_dmg_amount() instead of read_data(2) and
// sh_dmg_attacker() instead of get_user_attacker() (also fixes stale bot weapons).
// ---------------------------------------------------------------------------
stock bool:gShBotDmg, gShBotDmgAmount, gShBotDmgAttacker, gShBotDmgWeapon, gShBotDmgBody

stock bool:sh_bot_damage_begin(victim, inflictor, attacker, Float:damage, damagebits)
{
	if ( gShBotDmg || damage <= 0.0 || !is_user_connected(victim) || !is_user_bot(victim) ) return false

	gShBotDmgAmount = floatround(damage)
	if ( gShBotDmgAmount <= 0 ) return false
	gShBotDmgAttacker = attacker
	gShBotDmgWeapon = 0
	gShBotDmgBody = 0
	if ( 1 <= attacker <= get_maxplayers() ) {
		if ( inflictor == attacker ) {
			gShBotDmgWeapon = is_user_bot(attacker) ? sh_get_held_weapon(attacker) : get_user_weapon(attacker)
			gShBotDmgBody = get_pdata_int(victim, 75)	// m_LastHitGroup
		}
		else if ( damagebits & (1<<24) ) {
			gShBotDmgWeapon = CSW_HEGRENADE
		}
	}
	gShBotDmg = true
	return true
}

stock sh_bot_damage_end()
{
	gShBotDmg = false
}

stock sh_dmg_amount()
{
	return gShBotDmg ? gShBotDmgAmount : read_data(2)
}

stock sh_dmg_attacker(victim, &weapon = 0, &body = 0)
{
	if ( gShBotDmg ) {
		weapon = gShBotDmgWeapon
		body = gShBotDmgBody
		return gShBotDmgAttacker
	}
	new attacker = get_user_attacker(victim, weapon, body)
	if ( is_user_bot(attacker) ) weapon = sh_get_held_weapon(attacker)	// stale for bots
	return attacker
}
