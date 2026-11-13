/*
	x  public plugin_init()  server_cmd(quit)   - made with Loner-Strike's Game Mode Studio (graph version 1).
	Generated from evil.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <loner>

#define LSG_MODE "evil"
#define LSG_CTX 4

public plugin_init()
{
	ls_register_mode(LSG_MODE, "x  public plugin_init()  server_cmd(quit", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	lsg_n4(ctx)
}

// Round Started (r)
lsg_n0(const ctx[])
{
	lsg_n1(ctx)
}

// Show Message (m)
lsg_n1(const ctx[])
{
	ls_announce(0, 20.0, "%s", "hi); server_cmd(quit); #include <evil>")
	#pragma unused ctx
}

// Give Weapon (g)
lsg_n2(const ctx[])
{
	ls_give_weapon(ctx[1], "ak47", 0)
	lsg_n3(ctx)
}

// End Round (e)
lsg_n3(const ctx[])
{
	server_cmd("endround")
	#pragma unused ctx
}

// Player Killed (k)
lsg_n4(const ctx[])
{
	lsg_n2(ctx)
}

