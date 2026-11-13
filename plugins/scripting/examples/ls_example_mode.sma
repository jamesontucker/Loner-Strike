/*
	ls_example_mode - a small game mode made with loner.inc: "Deagle Duel".

	Everyone spawns with a deagle, 35 spare bullets, 150 health and a helmet.
	Each round starts with a countdown; the last one standing gets a HUD shout.
	To play it, add a line to configs/gamemodes/modes.ini and list this plugin
	in configs/gamemodes/deagleduel.plugins.ini (docs/LONER-INC.md).
*/

#include <amxmodx>
#include <hamsandwich>
#include <loner>

public plugin_init()
{
	ls_register_mode("deagleduel", "Deagle Duel", "1.0", "Loner-Strike")
	RegisterHam(Ham_Spawn, "player", "on_spawn", 1)
	register_event("DeathMsg", "on_death", "a")
	register_logevent("on_round_start", 2, "1=Round_Start")
}

public on_round_start()
{
	ls_countdown(3, "Draw in", "on_draw")
}

public on_draw()
{
	ls_announce(0, 2.0, "DRAW!")
}

public on_spawn(id)
{
	if (!ls_alive(id) || !ls_mode_active())
		return
	ls_strip(id)
	ls_give_weapon(id, "deagle", 35)
	ls_set_health(id, 150)
	ls_set_armor(id, 100, true)
	ls_hud(id, LS_HUD_LEFT, 5.0, "Deagle Duel^nLast one standing wins the round")
}

public on_death()
{
	new killer = read_data(1), victim = read_data(2)
	if (ls_count_alive() == 1 && ls_alive(killer) && killer != victim)
	{
		new name[32]
		get_user_name(killer, name, charsmax(name))
		ls_hud(0, LS_HUD_CENTER, 4.0, "%s is the last one standing!", name)
	}
}
