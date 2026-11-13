/*
	All Nodes - made with Loner-Strike's Game Mode Studio (graph version 1).
	Generated from all_nodes.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <loner>

#define LSG_MODE "all_nodes"
#define LSG_CTX 4

public plugin_init()
{
	ls_register_mode(LSG_MODE, "All Nodes", "1.0", "Game Mode Studio")
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
	lsg_n2(ctx)
}

// Round Started (r)
lsg_n0(const ctx[])
{
	lsg_n1(ctx)
}

// Show Message (m)
lsg_n1(const ctx[])
{
	ls_announce(0, 2.5, "%s", "Fight! 100% go")
	#pragma unused ctx
}

// Player Killed (k)
lsg_n2(const ctx[])
{
	lsg_n3(ctx)
}

// Is Team (c)
lsg_n3(const ctx[])
{
	new bool:ok = 1 <= ctx[0] <= MaxClients && is_user_connected(ctx[0]) && cs_get_user_team(ctx[0]) == CS_TEAM_CT
	if (ok)
		lsg_n4(ctx)
	else
		lsg_n6(ctx)
}

// Give Weapon (g)
lsg_n4(const ctx[])
{
	ls_give_weapon(ctx[1], "awp", 30)
	lsg_n5(ctx)
}

// End Round (e)
lsg_n5(const ctx[])
{
	server_cmd("endround CT")
	#pragma unused ctx
}

// End Round (e2)
lsg_n6(const ctx[])
{
	server_cmd("endround")
	#pragma unused ctx
}

