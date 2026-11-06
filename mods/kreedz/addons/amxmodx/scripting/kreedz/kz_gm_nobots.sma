/*
	Kreedz helper for the Game Mode Menu.

	The menu re-applies the saved global bot count (bot_quota) and Auto bunnyhop
	(gm_autobhop) after the mode's .cfg, so "bot_quota 0" in kreedz.cfg alone
	would be overridden. For the first seconds of each kreedz map this plugin
	puts both back to 0 whenever they are set. It never saves anything, so the
	usual bot count and bunnyhop setting come back when another mode is chosen.
	Adding bots from the menu later in the map still works.
*/

#include <amxmodx>

#define ENFORCE_SECONDS 15.0
#define TASK_FORCE 7310

new bool:g_Enforce = true
new g_pQuota, g_pAutoBhop, g_pKeepBhopOff

public plugin_init()
{
	register_plugin("Kreedz: no bots", "1.0", "gamemodes")
	g_pKeepBhopOff = register_cvar("kz_gm_autobhop_off", "1")
}

public plugin_cfg()
{
	g_pQuota = get_cvar_pointer("bot_quota")
	g_pAutoBhop = get_cvar_pointer("gm_autobhop")
	if (g_pQuota)
		hook_cvar_change(g_pQuota, "on_cvar_change")
	if (g_pAutoBhop)
		hook_cvar_change(g_pAutoBhop, "on_cvar_change")
	set_task(0.1, "task_force", TASK_FORCE)
	set_task(ENFORCE_SECONDS, "task_stop")
}

public on_cvar_change(pcvar, const old_value[], const new_value[])
{
	if (!g_Enforce || str_to_float(new_value) == 0.0)
		return
	// Queue the reset right behind the command that set it (same frame), so the
	// bot manager never sees the saved count and no bot is created; the task is a fallback.
	if (pcvar == g_pQuota)
		server_cmd("bot_quota 0")
	else if (pcvar == g_pAutoBhop && get_pcvar_num(g_pKeepBhopOff))
		server_cmd("gm_autobhop 0")
	if (!task_exists(TASK_FORCE))
		set_task(0.1, "task_force", TASK_FORCE)
}

public task_force()
{
	if (g_pQuota && get_pcvar_num(g_pQuota) != 0)
		set_pcvar_num(g_pQuota, 0)
	if (g_pAutoBhop && get_pcvar_num(g_pKeepBhopOff) && get_pcvar_num(g_pAutoBhop) != 0)
		set_pcvar_num(g_pAutoBhop, 0)
}

public task_stop()
{
	g_Enforce = false
	task_force()
}
