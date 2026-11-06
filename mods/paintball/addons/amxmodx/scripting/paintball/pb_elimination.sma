#include <amxmodx>
#include <fakemeta> 
#include <hamsandwich>

#define PLUGIN "CS Elimination Respawn"
#define VERSION "1.0"
#define AUTHOR "Sneaky.amxx"

#define RESPAWN_TIME 5
#define TASK_REVIVE 22092015

new g_MyKiller[33], g_RespawnTimeCount[33]
new g_MsgBarTime, g_MaxPlayers

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	
	RegisterHam(Ham_Spawn, "player", "fw_PlayerSpawn_Post", 1, true) // CZ bot patch: specialBot
	RegisterHam(Ham_Killed, "player", "fw_PlayerKilled_Post", 1, true)
	
	register_message(get_user_msgid("ClCorpse"), "Message_ClCorpse")
	
	g_MsgBarTime = get_user_msgid("BarTime")
	g_MaxPlayers = get_maxplayers()
}


// players eliminated by someone who leaves come back like when their killer dies
public client_disconnected(id)
{
	remove_task(id+TASK_REVIVE)
	g_MyKiller[id] = 0
	for(new i = 1; i <= g_MaxPlayers; i++)
	{
		if(i == id || g_MyKiller[i] != id)
			continue
		g_MyKiller[i] = 0
		if(is_user_connected(i) && !is_user_alive(i))
			Check_PlayerDeath(i+TASK_REVIVE)
	}
}

public fw_PlayerSpawn_Post(id)
{
	g_MyKiller[id] = 0
}

public fw_PlayerKilled_Post(Victim, Attacker)
{
	if(!is_user_connected(Victim)) return
	
	static Killer; Killer = 0
	if(is_user_connected(Attacker)) Killer = Attacker
	
	for(new i = 1; i <= g_MaxPlayers; i++)
	{
		if(!is_user_connected(i))
			continue
		if(is_user_alive(i))
			continue
			
		if(g_MyKiller[i] == Victim)
		{
			Check_PlayerDeath(i+TASK_REVIVE)
			g_MyKiller[i] = 0
		}
	}
	
	if(Killer)
	{
		if(Killer == Victim) 
		{
			Check_PlayerDeath(Victim+TASK_REVIVE)
			return
		}
		
		g_MyKiller[Victim] = Killer
		client_print(Victim, print_center, "You will respawn when your killer is eliminated")
	} else {
		Check_PlayerDeath(Victim+TASK_REVIVE)
	}
}

public Check_PlayerDeath(id)
{
	id -= TASK_REVIVE
	
	if(!is_user_connected(id) || is_user_alive(id))
		return
	if(pev(id, pev_deadflag) != 2)
	{
		set_task(0.25, "Check_PlayerDeath", id+TASK_REVIVE)
		return
	}

	g_RespawnTimeCount[id] = RESPAWN_TIME

	// Bar
	message_begin(MSG_ONE_UNRELIABLE, g_MsgBarTime, {0, 0, 0}, id)
	write_byte(g_RespawnTimeCount[id])
	write_byte(0)
	message_end()
	
	// Check Respawn
	Start_Revive(id+TASK_REVIVE)
}

public Start_Revive(id)
{
	id -= TASK_REVIVE
	
	if(!is_user_connected(id) || is_user_alive(id))  
		return
	if(g_RespawnTimeCount[id] <= 0.0)
	{
		Revive_Now(id+TASK_REVIVE)
		return
	}
		
	client_print(id, print_center, "Respawning in %i", g_RespawnTimeCount[id])
	
	g_RespawnTimeCount[id]--
	set_task(1.0, "Start_Revive", id+TASK_REVIVE)
}

public Revive_Now(id)
{
	id -= TASK_REVIVE
	
	if(!is_user_connected(id) || is_user_alive(id))
		return
	
	
	// Remove Task
	remove_task(id+TASK_REVIVE)
	
	set_pev(id, pev_deadflag, DEAD_RESPAWNABLE)
	ExecuteHamB(Ham_CS_RoundRespawn, id)
}

public Message_ClCorpse()
{
	//static id; id = get_msg_arg_int(12)
	return PLUGIN_HANDLED
}
