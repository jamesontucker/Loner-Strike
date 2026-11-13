// Loner-Strike render comparison (development tool, not shipped): puts you at an exact spot
// and view, so an in-game screenshot can be compared with the map viewer's
// (docs/TOOLS-RENDER.md). Only works with sv_cheats 1, on your own listen server.
//
//   ls_cam <x> <y> <z> <pitch> <yaw>     origin as the viewer's ?cam= takes it (eye 17 above)

#include <amxmodx>
#include <fakemeta>

public plugin_init()
{
	register_plugin("LS camera", "1.0", "Loner-Strike")
	register_clcmd("ls_cam", "cmd_cam")
}

public cmd_cam(id)
{
	if (!get_cvar_num("sv_cheats") || read_argc() < 6)
	{
		console_print(id, "ls_cam <x> <y> <z> <pitch> <yaw> (needs sv_cheats 1)")
		return PLUGIN_HANDLED
	}
	new arg[24], Float:o[3], Float:a[3]
	for (new i = 0; i < 3; i++)
	{
		read_argv(i + 1, arg, charsmax(arg))
		o[i] = str_to_float(arg)
	}
	read_argv(4, arg, charsmax(arg))
	a[0] = str_to_float(arg)
	read_argv(5, arg, charsmax(arg))
	a[1] = str_to_float(arg)
	a[2] = 0.0
	set_pev(id, pev_movetype, MOVETYPE_NOCLIP)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_angles, a)
	set_pev(id, pev_v_angle, a)
	set_pev(id, pev_fixangle, 1)
	return PLUGIN_HANDLED
}
