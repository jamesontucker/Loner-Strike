/*
	Every Node - made with Loner-Strike's Game Mode Studio (graph version 2).
	Generated from every_node.graph.json: change the graph and compile it again; edits here are lost.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>

#define LSG_MODE "every_node"
#define LSG_CTX 4
#define LSG_TXT 2
#define LSG_N 77
#define LSG_TASK 7310000
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }

// The mode's variables
stock Float:g_pv0[33] // Kills (number, per player)
stock Float:g_v1 = 1.0 // Round (number)
stock g_v2 = 0 // Flag (bool)
stock g_v3[64] = "none" // Last thing (text)

stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

stock Float:lsg_div(Float:a, Float:b)
	return b == 0.0 ? 0.0 : a / b

stock Float:lsg_min(Float:a, Float:b)
	return a < b ? a : b

stock Float:lsg_max(Float:a, Float:b)
	return a > b ? a : b

stock Float:lsg_random(Float:a, Float:b, frac)
{
	if (a > b)
	{
		new Float:t = a
		a = b
		b = t
	}
	return frac ? random_float(a, b) : float(random_num(floatround(a), floatround(b)))
}

stock Float:lsg_health(id)
	return ls_alive(id) ? float(get_user_health(id)) : 0.0

stock lsg_team(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) ? (_:cs_get_user_team(id)) : 0

stock lsg_is_bot(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) && is_user_bot(id) ? 1 : 0

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

stock lsg_find(const name[])
	return name[0] ? max(find_ent_by_tname(-1, name), 0) : 0

stock bool:lsg_named(ent, const name[])
{
	new tn[32]
	pev(ent, pev_targetname, tn, charsmax(tn))
	return bool:equal(tn, name)
}

stock bool:lsg_origin_of(const name[], Float:o[3])
{
	new ent = lsg_find(name)
	if (ent <= 0)
		return false
	pev(ent, pev_origin, o)
	return true
}

stock bool:lsg_teleport(id, const name[])
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_origin_of(name, o))
		return false
	ls_teleport(id, o)
	return true
}

stock lsg_log_player()
{
	new who[96], name[32], uid
	read_logargv(0, who, charsmax(who))
	parse_loguser(who, name, charsmax(name), uid)
	return find_player("k", uid)
}

stock lsg_creature(const kind[], const at[])
{
	new Float:o[3]
	if (!get_cvar_pointer("ls_creature_ver") || !lsg_origin_of(at, o))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node", "1.0", "Game Mode Studio")
	register_logevent("lsg_on_round_started", 2, "1=Round_Start")
	register_event("SendAudio", "lsg_on_round_end_t", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "lsg_on_round_end_ct", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "lsg_on_round_end_draw", "a", "2&%!MRAD_rounddraw")
	register_event("DeathMsg", "lsg_on_player_killed", "a")
	RegisterHam(Ham_Spawn, "player", "lsg_on_player_spawned", 1)
	register_logevent("lsg_on_bomb_planted", 3, "2=Planted_The_Bomb")
	register_logevent("lsg_on_bomb_defused", 3, "2=Defused_The_Bomb")
	register_logevent("lsg_on_bomb_exploded", 6, "3=Target_Bombed")
	RegisterHam(Ham_Use, "func_button", "lsg_on_player_used", 0)
	register_touch("trigger_multiple", "player", "lsg_on_player_touched")
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_Killed, "ls_creature", "lsg_on_creature_killed", 1)
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
}

// timers stop when a new round starts
public lsg_new_round()
	lsg_stop_timers()

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_putinserver(id)
{
	g_pv0[id] = 0.0
	lsg_on_player_joined(id)
}

public client_disconnected(id)
	lsg_on_player_left(id)

public lsg_on_round_started()
{
	new ctx[LSG_CTX]
	lsg_n0(ctx)
}

public lsg_on_round_end_t()
	lsg_on_round_ended(1)

public lsg_on_round_end_ct()
	lsg_on_round_ended(2)

public lsg_on_round_end_draw()
	lsg_on_round_ended(0)

lsg_on_round_ended(winner)
{
	new ctx[LSG_CTX]
	ctx[0] = winner // winner
	lsg_n36(ctx)
}

public lsg_on_player_killed()
{
	new ctx[LSG_CTX]
	ctx[0] = read_data(2) // victim
	ctx[1] = read_data(1) // attacker
	ctx[2] = read_data(3) ? 1 : 0 // headshot
	lsg_n10(ctx)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n18(ctx)
	return HAM_IGNORED
}

lsg_on_player_joined(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n25(ctx)
}

lsg_on_player_left(id)
{
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n28(ctx)
}

public lsg_on_bomb_planted()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n30(ctx)
}

public lsg_on_bomb_defused()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n32(ctx)
}

public lsg_on_bomb_exploded()
{
	new ctx[LSG_CTX]
	lsg_n34(ctx)
}

public lsg_on_player_used(ent, caller, activator)
{
	if (activator < 1 || activator > MaxClients)
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = activator // player
	ctx[1] = ent // entity
	if (lsg_named(ent, "door_button"))
		lsg_n40(ctx)
	return HAM_IGNORED
}

public lsg_on_player_touched(ent, id)
{
	static Float:last[33]
	if (id < 1 || id > MaxClients || get_gametime() - last[id] < 1.0)
		return PLUGIN_CONTINUE
	last[id] = get_gametime()
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = ent // entity
	lsg_n43(ctx)
	return PLUGIN_CONTINUE
}

public ls_on_capture(ent, side, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = side == 1 || side == 2 ? side : 0 // team
	ctx[2] = id // player
	lsg_n45(ctx)
}

public ls_on_capture_lost(ent, side)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = side == 1 || side == 2 ? side : 0 // team
	lsg_n48(ctx)
}

public ls_on_zone_stage(ent, stage)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // zone
	ctx[1] = _:float(stage) // stage
	lsg_n50(ctx)
}

public ls_on_zone_tick(ent, id, Float:dmg)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // zone
	ctx[1] = id // player
	ctx[2] = _:dmg // damage
	lsg_n52(ctx)
}

public ls_on_extract(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = id // player
	lsg_n55(ctx)
}

public ls_on_wave_start(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n57(ctx)
}

public ls_on_wave_spawn(ent, wave, const Float:origin[3], const types[])
{
	#pragma unused origin
	#pragma unused types
	new ctx[LSG_CTX]
	ctx[0] = ent // spawned
	ctx[1] = _:float(wave) // wave
	lsg_n59(ctx)
}

public ls_on_wave_cleared(wave)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(wave) // wave
	lsg_n61(ctx)
}

public ls_on_loot(ent, id)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // container
	ctx[1] = id // player
	lsg_n63(ctx)
}

public ls_on_checkpoint(ent, id, order)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // point
	ctx[1] = id // player
	ctx[2] = _:float(order) // order
	lsg_n65(ctx)
}

public ls_on_barricade_broken(ent, attacker)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // barricade
	ctx[1] = attacker // attacker
	lsg_n67(ctx)
}

public ls_on_meeting_end(ent, ejected)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // meeting
	ctx[1] = ejected // ejected
	lsg_n69(ctx)
}

public lsg_on_creature_killed(ent, killer)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // creature
	ctx[1] = killer >= 1 && killer <= MaxClients ? killer : 0 // killer
	pev(ent, pev_netname, g_lsgTxt[0], 63) // kind
	lsg_n71(ctx)
	return HAM_IGNORED
}

// Round Started (rs)
lsg_n0(const ctx[])
{
	lsg_n1(ctx)
}

// Set Variable (rs_set)
lsg_n1(const ctx[])
{
	g_v1 = ((g_v1) + (1.0))
	lsg_n2(ctx)
}

// Countdown (rs_cd)
lsg_n2(const ctx[])
{
	remove_task(LSG_TASK + 2)
	g_lsgLeft[2] = 5
	lsg_c2_show()
	set_task(1.0, "lsg_t2", LSG_TASK + 2, ctx, LSG_CTX)
	lsg_n4(ctx)
}

lsg_c2_show()
	ls_hud(0, LsHud:0, 1.2, "%s %d", "Go in", g_lsgLeft[2])

public lsg_t2(const ctx[], task)
{
	#pragma unused task
	if (--g_lsgLeft[2] > 0)
	{
		lsg_c2_show()
		set_task(1.0, "lsg_t2", LSG_TASK + 2, ctx, LSG_CTX)
		return
	}
	lsg_n3(ctx)
}

// Show Message (rs_msg)
lsg_n3(const ctx[])
{
	ls_announce(0, 2.0, "%s", "Go!")
	#pragma unused ctx
}

// Repeat (rs_rep)
lsg_n4(const ctx[])
{
	remove_task(LSG_TASK + 4)
	g_lsgLeft[4] = 3
	set_task(floatmax(10.0, 0.1), "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
	lsg_n6(ctx)
}

public lsg_t4(const ctx[], task)
{
	#pragma unused task
	if (g_lsgLeft[4] > 0 && --g_lsgLeft[4] == 0)
		remove_task(LSG_TASK + 4)
	else
		set_task(floatmax(10.0, 0.1), "lsg_t4", LSG_TASK + 4, ctx, LSG_CTX)
	lsg_n5(ctx)
}

// Show HUD Text (rs_hud)
lsg_n5(const ctx[])
{
	ls_hud(0, LsHud:2, 5.0, "%s %d", "Alive:", floatround(float(ls_count_alive(CsTeams:(0)))))
	#pragma unused ctx
}

// Delay (rs_delay)
lsg_n6(const ctx[])
{
	set_task(floatmax(5.0, 0.1), "lsg_t6", LSG_TASK + 6, ctx, LSG_CTX)
}

public lsg_t6(const ctx[], task)
{
	#pragma unused task
	lsg_n7(ctx)
}

// Start Wave (rs_wave)
lsg_n7(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_wave_start(max(floatround(0.0), 0))
	}
	lsg_n8(ctx)
}

// Stop Waves (rs_wstop)
lsg_n8(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_wave_stop()
	}
	lsg_n9(ctx)
}

// Reset Components (rs_reset)
lsg_n9(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_comp_reset()
	}
	#pragma unused ctx
}

// Player Killed (pk)
lsg_n10(const ctx[])
{
	lsg_n11(ctx)
}

// Branch (pk_br)
lsg_n11(const ctx[])
{
	new ok = ((((ctx[2]) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n12(ctx)
	else
		lsg_n13(ctx)
}

// Add To Variable (pk_add)
lsg_n12(const ctx[])
{
	g_pv0[lsg_slot(ctx[1])] += 2.0
	lsg_n14(ctx)
}

// Add To Variable (pk_add2)
lsg_n13(const ctx[])
{
	g_pv0[lsg_slot(ctx[1])] += lsg_random(1.0, 3.0, 0)
}

// Is Team (pk_team)
lsg_n14(const ctx[])
{
	new ok = ((ctx[1] >= 1 && ctx[1] <= MaxClients && is_user_connected(ctx[1]) && _:cs_get_user_team(ctx[1]) == 2) ? 1 : 0)
	if (ok)
		lsg_n15(ctx)
	else
		lsg_n17(ctx)
}

// Give Weapon (pk_give)
lsg_n15(const ctx[])
{
	ls_give_weapon(ctx[1], "m4a1", 60)
	lsg_n16(ctx)
}

// Set Health (pk_seth)
lsg_n16(const ctx[])
{
	if (!(!ls_alive(ctx[1])))
	{
		ls_set_health(ctx[1], max(floatround(((lsg_health(ctx[1])) + (10.0))), 1))
	}
}

// Show HUD Text (pk_hud)
lsg_n17(const ctx[])
{
	ls_hud(ctx[1], LsHud:3, 4.0, "%s %s", "You got", lsg_name(ctx[0]))
}

// Player Spawned (ps)
lsg_n18(const ctx[])
{
	lsg_n19(ctx)
}

// Strip Weapons (ps_strip)
lsg_n19(const ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		ls_strip(ctx[0], true)
	}
	lsg_n20(ctx)
}

// Set Armor (ps_arm)
lsg_n20(const ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		ls_set_armor(ctx[0], max(floatround(50.0), 0), true)
	}
	lsg_n21(ctx)
}

// Set Speed (ps_spd)
lsg_n21(const ctx[])
{
	if (!(!ls_alive(ctx[0])))
	{
		set_user_maxspeed(ctx[0], 300.0)
	}
	lsg_n22(ctx)
}

// Set Team (ps_team)
lsg_n22(const ctx[])
{
	if (!(ctx[0] < 1 || ctx[0] > MaxClients || !is_user_connected(ctx[0])))
	{
		ls_set_team(ctx[0], CsTeams:(1))
	}
	lsg_n23(ctx)
}

// Teleport (ps_tp)
lsg_n23(const ctx[])
{
	lsg_teleport(ctx[0], "spawn_a")
	lsg_n24(ctx)
}

// Set Variable (ps_set)
lsg_n24(const ctx[])
{
	g_pv0[lsg_slot(ctx[0])] = 0.0
}

// Player Joined (pj)
lsg_n25(const ctx[])
{
	lsg_n26(ctx)
}

// Show HUD Text (pj_hud)
lsg_n26(const ctx[])
{
	ls_hud(ctx[0], LsHud:1, 4.0, "%s %s", "Bot:", g_lsgYesNo[(lsg_is_bot(ctx[0])) ? 1 : 0])
	lsg_n27(ctx)
}

// Branch (pj_br)
lsg_n27(const ctx[])
{
	new ok = (((((ls_alive(ctx[0]) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	#pragma unused ok
}

// Player Left (pl)
lsg_n28(const ctx[])
{
	lsg_n29(ctx)
}

// Set Variable (pl_set)
lsg_n29(const ctx[])
{
	copy(g_v3, 63, "someone left")
	#pragma unused ctx
}

// Bomb Planted (bp)
lsg_n30(const ctx[])
{
	lsg_n31(ctx)
}

// Show HUD Text (bp_hud)
lsg_n31(const ctx[])
{
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Planted by", g_lsgTeamName[clamp(lsg_team(ctx[0]), 0, 3)])
}

// Bomb Defused (bd)
lsg_n32(const ctx[])
{
	lsg_n33(ctx)
}

// End Round (bd_end)
lsg_n33(const ctx[])
{
	server_cmd("endround CT")
	#pragma unused ctx
}

// Bomb Exploded (bx)
lsg_n34(const ctx[])
{
	lsg_n35(ctx)
}

// End Round (bx_end)
lsg_n35(const ctx[])
{
	server_cmd("endround T")
	#pragma unused ctx
}

// Round Ended (re)
lsg_n36(const ctx[])
{
	lsg_n37(ctx)
}

// Branch (re_br)
lsg_n37(const ctx[])
{
	new ok = ((((((((float(ls_count_alive(CsTeams:(ctx[0])))) > (0.0) ? 1 : 0)) && (((g_v2) ? 0 : 1)) ? 1 : 0)) ? 1 : 0)) ? 1 : 0)
	if (ok)
		lsg_n38(ctx)
	else
		lsg_n39(ctx)
}

// Set Variable (re_set)
lsg_n38(const ctx[])
{
	g_v2 = 1
	#pragma unused ctx
}

// Stop Timers (re_stop)
lsg_n39(const ctx[])
{
	lsg_stop_timers()
	#pragma unused ctx
}

// Player Used (pu)
lsg_n40(const ctx[])
{
	lsg_n41(ctx)
}

// Door (pu_door)
lsg_n41(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_door_open(ctx[1])
	}
	lsg_n42(ctx)
}

// Fire Trap (pu_trap)
lsg_n42(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_trap_fire(lsg_find("trap1"))
	}
	#pragma unused ctx
}

// Player Touched (pt)
lsg_n43(const ctx[])
{
	lsg_n44(ctx)
}

// Open Crate (pt_crate)
lsg_n44(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_loot_open(ctx[1], ctx[0])
	}
}

// Point Captured (cc)
lsg_n45(const ctx[])
{
	lsg_n46(ctx)
}

// Lock Point (cc_lock)
lsg_n46(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_cp_lock(ctx[0], 20.0)
	}
	lsg_n47(ctx)
}

// Show HUD Text (cc_hud)
lsg_n47(const ctx[])
{
	ls_hud(0, LsHud:0, 4.0, "%s %s", "Taken by", g_lsgTeamName[clamp(ctx[1], 0, 3)])
}

// Point Lost (cl)
lsg_n48(const ctx[])
{
	lsg_n49(ctx)
}

// Open Extraction (cl_ex)
lsg_n49(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_extract_open(lsg_find("ex1"), bool:1)
	}
	#pragma unused ctx
}

// Zone Stage (zs)
lsg_n50(const ctx[])
{
	lsg_n51(ctx)
}

// Shrink Zone (zs_shrink)
lsg_n51(const ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_origin_of("mid", o))
		ls_zone_shrink(ctx[0], o, ((Float:ctx[1]) * (100.0)), 20.0)
}

// Zone Hurt Player (zt)
lsg_n52(const ctx[])
{
	lsg_n53(ctx)
}

// Set Zone (zt_set)
lsg_n53(const ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_origin_of("mid", o))
		ls_zone_set(ctx[0], o, 900.0)
	lsg_n54(ctx)
}

// Show HUD Text (zt_hud)
lsg_n54(const ctx[])
{
	ls_hud(ctx[1], LsHud:3, 4.0, "%s %d", "Ouch", floatround(Float:ctx[2]))
}

// Player Extracted (ex)
lsg_n55(const ctx[])
{
	lsg_n56(ctx)
}

// Add To Variable (ex_add)
lsg_n56(const ctx[])
{
	g_pv0[lsg_slot(ctx[1])] += 5.0
}

// Wave Started (ws)
lsg_n57(const ctx[])
{
	lsg_n58(ctx)
}

// Show HUD Text (ws_hud)
lsg_n58(const ctx[])
{
	ls_hud(0, LsHud:0, 4.0, "%s %d", "Wave", floatround(Float:ctx[0]))
}

// Wave Spawned One (wp)
lsg_n59(const ctx[])
{
	lsg_n60(ctx)
}

// Show HUD Text (wp_hud)
lsg_n60(const ctx[])
{
	ls_hud(0, LsHud:2, 4.0, "%s %d", "In:", ctx[0])
}

// Wave Cleared (wc)
lsg_n61(const ctx[])
{
	lsg_n62(ctx)
}

// Start Wave (wc_start)
lsg_n62(const ctx[])
{
	if (!(!ls_components_running()))
	{
		ls_wave_start(max(floatround(((Float:ctx[0]) + (1.0))), 0))
	}
}

// Crate Opened (lo)
lsg_n63(const ctx[])
{
	lsg_n64(ctx)
}

// Show HUD Text (lo_hud)
lsg_n64(const ctx[])
{
	ls_hud(ctx[1], LsHud:1, 4.0, "%s", "Looted")
}

// Checkpoint Reached (ck)
lsg_n65(const ctx[])
{
	lsg_n66(ctx)
}

// Set Variable (ck_set)
lsg_n66(const ctx[])
{
	g_v1 = Float:ctx[2]
}

// Barricade Broken (bb)
lsg_n67(const ctx[])
{
	lsg_n68(ctx)
}

// Show Message (bb_msg)
lsg_n68(const ctx[])
{
	ls_announce(0, 3.0, "%s", "The barricade is down!")
	#pragma unused ctx
}

// Meeting Ended (me)
lsg_n69(const ctx[])
{
	lsg_n70(ctx)
}

// Show HUD Text (me_hud)
lsg_n70(const ctx[])
{
	ls_hud(0, LsHud:3, 4.0, "%s %s", "Out:", lsg_name(ctx[1]))
}

// Creature Killed (ck2)
lsg_n71(const ctx[])
{
	lsg_n72(ctx)
}

// Set Variable (ck2_set)
lsg_n72(const ctx[])
{
	copy(g_v3, 63, g_lsgTxt[0])
	lsg_n73(ctx)
}

// Create Component (ck2_make)
lsg_n73(const ctx[])
{
	new Float:o[3]
	if (!!ls_components_running() && lsg_origin_of("crate_spot", o))
		g_lsgOut[73] = ls_comp_create("ls_loot_container", o, "table=rifles radius=40")
	lsg_n74(ctx)
}

// Spawn Creature (ck2_spawn)
lsg_n74(const ctx[])
{
	g_lsgOut[74] = lsg_creature("zombie", "spot1")
	lsg_n75(ctx)
}

// Show HUD Text (ck2_hud)
lsg_n75(const ctx[])
{
	ls_hud(0, LsHud:2, 4.0, "%s %d", "New one:", g_lsgOut[74])
	lsg_n76(ctx)
}

// Show HUD Text (ck2_hud2)
lsg_n76(const ctx[])
{
	ls_hud(ctx[1], LsHud:1, 4.0, "%s %s", "Killed:", g_v3)
}

