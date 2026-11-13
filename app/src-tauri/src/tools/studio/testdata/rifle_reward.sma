/*
	Rifle Reward - made with Loner-Strike's Game Mode Studio (graph version 1).
	Generated from rifle_reward.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <loner>

#define LSG_MODE "rifle_reward"
#define LSG_CTX 4

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Rifle Reward", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
}

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_start(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	lsg_killed(ctx)
}

// Round Started (start)
lsg_start(const ctx[])
{
	lsg_hello(ctx)
}

// Show Message (hello)
lsg_hello(const ctx[])
{
	ls_announce(0, 4.0, "%s", "Every kill earns you a rifle!")
	#pragma unused ctx
}

// Player Killed (killed)
lsg_killed(const ctx[])
{
	lsg_is_ct(ctx)
}

// Is Team (is_ct)
lsg_is_ct(const ctx[])
{
	new bool:ok = 1 <= ctx[1] <= MaxClients && is_user_connected(ctx[1]) && cs_get_user_team(ctx[1]) == CS_TEAM_CT
	if (ok)
		lsg_give_m4(ctx)
	else
		lsg_is_t(ctx)
}

// Give Weapon (give_m4)
lsg_give_m4(const ctx[])
{
	ls_give_weapon(ctx[1], "m4a1", 90)
}

// Is Team (is_t)
lsg_is_t(const ctx[])
{
	new bool:ok = 1 <= ctx[1] <= MaxClients && is_user_connected(ctx[1]) && cs_get_user_team(ctx[1]) == CS_TEAM_T
	if (ok)
		lsg_give_ak(ctx)
}

// Give Weapon (give_ak)
lsg_give_ak(const ctx[])
{
	ls_give_weapon(ctx[1], "ak47", 90)
}

