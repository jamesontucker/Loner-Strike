/*
	gm_simrec - records what every player does, for comparing bot behaviour between builds.
	Not part of Loner-Strike: a test tool for the bot simulation.

	addons/amxmodx/data/simrec/<map>.csv, one line per row:
	  S,t,id,team,x,y,z,vx,vy,vz,yaw,pitch,duck,ground,weapon,clip,keys,hp,venemy,enemydist,matedist
	      a sample every 0.1 s of game time for each living player (keys: the buttons pressed
	      since the last sample, ORed)
	  F,t,id,weapon,speed,duck,ground         a shot
	  H,t,attacker,victim,damage,hitgroup      a hit
	  K,t,killer,victim,headshot,weapon        a kill
	  R,t,event                                round start / end
	  P,id,name,bot                            who someone is
	  C,t,id,team,text                         chat (team: 1 for say_team; commas in the text become ;)
	  E,t,id,event,detail                      something that happened: plant, planted, defuse (detail
	                                           kit/nokit), defused, bomb_drop, bomb_get, hostage_touch,
	                                           hostage_rescue, hostage_kill, spray, pickup (detail: CSW id)

	It also turns on Loner-Strike's bot trace (bot_ls_trace 1: lstrace.csv in the server's
	folder), each map and round, so a recording comes with what the bots were doing and why.
*/

#include <amxmodx>
#include <fakemeta>
#include <hamsandwich>
#include <cstrike>
#include <amxmisc>

#define SAMPLE 0.1

new g_File
new g_Keys[33]
new bool:g_Named[33]

new const GUNS[][] = { "weapon_p228", "weapon_scout", "weapon_xm1014", "weapon_mac10", "weapon_aug",
	"weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas",
	"weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3",
	"weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_deagle", "weapon_sg552", "weapon_ak47",
	"weapon_p90", "weapon_knife" }

public plugin_init()
{
	register_plugin("Sim recorder", "1.0", "loner-strike")
	register_forward(FM_CmdStart, "fw_cmdstart")
	// (RegisterHamPlayer: Condition Zero bots are a different class)
	RegisterHamPlayer(Ham_TakeDamage, "fw_damage_post", 1)
	for (new i = 0; i < sizeof GUNS; i++)
		RegisterHam(Ham_Weapon_PrimaryAttack, GUNS[i], "fw_shot_post", 1)
	register_event("DeathMsg", "ev_death", "a")
	register_logevent("ev_round_start", 2, "1=Round_Start")
	register_logevent("ev_round_end", 2, "1=Round_End")
	// (from the log, not the say command: the bots' lines go straight to the game, past
	// AMXX's command hooks)
	register_logevent("ev_say", 3, "1=say")
	register_logevent("ev_say_team", 3, "1=say_team")
	register_logevent("ev_triggered", 3, "1=triggered")
	register_message(SVC_TEMPENTITY, "msg_tempentity")
	RegisterHamPlayer(Ham_AddPlayerItem, "fw_add_item_post", 1)

	new dir[128], map[32], path[192]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/simrec")
	if (!dir_exists(dir))
		mkdir(dir)
	get_mapname(map, charsmax(map))
	formatex(path, charsmax(path), "%s/%s_%d.csv", dir, map, get_systime())
	g_File = fopen(path, "wt")
	set_task(SAMPLE, "task_sample", 1, _, _, "b")
	set_task(1.0, "task_flush", 2, _, _, "b")
}

public plugin_cfg()
{
	if (cvar_exists("bot_ls_trace"))
		server_cmd("bot_ls_trace 1")
}

public ev_say()
{
	chat(0)
}

public ev_say_team()
{
	chat(1)
}

// "Name<uid><STEAM_ID><TEAM>" say "text"
chat(team)
{
	if (!g_File)
		return
	new who[96], name[32], uid, text[192]
	read_logargv(0, who, charsmax(who))
	read_logargv(2, text, charsmax(text))
	parse_loguser(who, name, charsmax(name), uid)
	new id = find_player("k", uid)
	replace_all(text, charsmax(text), ",", ";")
	fprintf(g_File, "C,%.2f,%d,%d,%s^n", get_gametime(), id, team, text)
}

public plugin_end()
{
	if (g_File)
		fclose(g_File)
}

public task_flush()
{
	if (g_File)
		fflush(g_File)
}

public fw_cmdstart(id, uc)
{
	if (1 <= id <= 32)
		g_Keys[id] |= get_uc(uc, UC_Buttons)
}

public fw_shot_post(weapon)
{
	if (!g_File || !pev_valid(weapon))
		return
	new id = pev(weapon, pev_owner)
	if (!(1 <= id <= 32) || !is_user_alive(id))
		return
	new Float:v[3]
	pev(id, pev_velocity, v)
	new flags = pev(id, pev_flags)
	fprintf(g_File, "F,%.2f,%d,%d,%.0f,%d,%d^n", get_gametime(), id, cs_get_weapon_id(weapon),
		floatsqroot(v[0] * v[0] + v[1] * v[1]), (flags & FL_DUCKING) ? 1 : 0, (flags & FL_ONGROUND) ? 1 : 0)
}

public fw_damage_post(victim, inflictor, attacker, Float:damage, bits)
{
	if (!g_File || !(1 <= attacker <= 32))
		return
	fprintf(g_File, "H,%.2f,%d,%d,%.0f,%d^n", get_gametime(), attacker, victim, damage, get_pdata_int(victim, 75, 5))
}

public ev_death()
{
	if (!g_File)
		return
	new weapon[24]
	read_data(4, weapon, charsmax(weapon))
	fprintf(g_File, "K,%.2f,%d,%d,%d,%s^n", get_gametime(), read_data(1), read_data(2), read_data(3), weapon)
}

public ev_round_start()
{
	if (g_File)
		fprintf(g_File, "R,%.2f,start^n", get_gametime())
	if (cvar_exists("bot_ls_trace"))
		server_cmd("bot_ls_trace 1")
}

public ev_round_end()
{
	if (g_File)
		fprintf(g_File, "R,%.2f,end^n", get_gametime())
}

// Can a see b (eye to eye, and roughly in front: within 75 degrees of where a looks)?
bool:sees(a, b, Float:eye[3], Float:fwd[3])
{
	new Float:o[3], Float:ofs[3]
	pev(b, pev_origin, o)
	pev(b, pev_view_ofs, ofs)
	o[2] += ofs[2]
	new Float:d[3]
	d[0] = o[0] - eye[0]; d[1] = o[1] - eye[1]; d[2] = o[2] - eye[2]
	new Float:len = floatsqroot(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
	if (len < 1.0)
		return true
	if ((d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2]) / len < 0.26)
		return false
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, eye, o, IGNORE_MONSTERS, a, tr)
	new Float:frac
	get_tr2(tr, TR_flFraction, frac)
	free_tr2(tr)
	return frac >= 1.0
}

public task_sample()
{
	if (!g_File)
		return
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_connected(id))
			continue
		if (!g_Named[id])
		{
			new name[32]
			get_user_name(id, name, charsmax(name))
			replace_all(name, charsmax(name), ",", " ")
			fprintf(g_File, "P,%d,%s,%d^n", id, name, is_user_bot(id) ? 1 : 0)
			g_Named[id] = true
		}
		if (!is_user_alive(id))
		{
			g_Keys[id] = 0
			continue
		}
		new Float:o[3], Float:v[3], Float:ang[3], Float:ofs[3], Float:fwd[3]
		pev(id, pev_origin, o)
		pev(id, pev_velocity, v)
		pev(id, pev_v_angle, ang)
		pev(id, pev_view_ofs, ofs)
		new Float:eye[3]
		eye[0] = o[0]; eye[1] = o[1]; eye[2] = o[2] + ofs[2]
		engfunc(EngFunc_MakeVectors, ang)
		global_get(glb_v_forward, fwd)

		new team = get_user_team(id)
		new venemy, Float:enemyDist = 99999.0, Float:mateDist = 99999.0
		for (new j = 1; j <= MaxClients; j++)
		{
			if (j == id || !is_user_alive(j))
				continue
			new Float:p[3]
			pev(j, pev_origin, p)
			new Float:dist = get_distance_f(o, p)
			if (get_user_team(j) == team && !get_cvar_num("mp_freeforall"))
			{
				if (dist < mateDist)
					mateDist = dist
			}
			else if (sees(id, j, eye, fwd))
			{
				venemy++
				if (dist < enemyDist)
					enemyDist = dist
			}
		}
		new clip, ammo
		new weapon = get_user_weapon(id, clip, ammo)
		new flags = pev(id, pev_flags)
		fprintf(g_File, "S,%.2f,%d,%d,%.0f,%.0f,%.0f,%.0f,%.0f,%.0f,%.1f,%.1f,%d,%d,%d,%d,%d,%d,%d,%.0f,%.0f^n",
			now, id, team, o[0], o[1], o[2], v[0], v[1], v[2], ang[1], ang[0],
			(flags & FL_DUCKING) ? 1 : 0, (flags & FL_ONGROUND) ? 1 : 0, weapon, clip, g_Keys[id],
			get_user_health(id), venemy, enemyDist, mateDist)
		g_Keys[id] = 0
	}
}

public client_disconnected(id)
{
	g_Named[id] = false
	g_Keys[id] = 0
}

event(id, const what[], const detail[] = "")
{
	if (g_File)
		fprintf(g_File, "E,%.2f,%d,%s,%s^n", get_gametime(), id, what, detail)
}

// "Name<uid><STEAM_ID><TEAM>" triggered "What"
public ev_triggered()
{
	new who[96], what[48], name[32], uid
	read_logargv(0, who, charsmax(who))
	read_logargv(2, what, charsmax(what))
	parse_loguser(who, name, charsmax(name), uid)
	new id = find_player("k", uid)
	if (equal(what, "Planted_The_Bomb"))
		event(id, "planted")
	else if (equal(what, "Begin_Bomb_Defuse_With_Kit"))
		event(id, "defuse", "kit")
	else if (equal(what, "Begin_Bomb_Defuse_Without_Kit"))
		event(id, "defuse", "nokit")
	else if (equal(what, "Defused_The_Bomb"))
		event(id, "defused")
	else if (equal(what, "Dropped_The_Bomb"))
		event(id, "bomb_drop")
	else if (equal(what, "Got_The_Bomb"))
		event(id, "bomb_get")
	else if (equal(what, "Touched_A_Hostage"))
		event(id, "hostage_touch")
	else if (equal(what, "Rescued_A_Hostage"))
		event(id, "hostage_rescue")
	else if (equal(what, "Killed_A_Hostage"))
		event(id, "hostage_kill")
}

// a spray (the engine's impulse 201 and the bots' plugin sprays both send TE_PLAYERDECAL)
public msg_tempentity()
{
	if (get_msg_arg_int(1) == 112)
		event(get_msg_arg_int(2), "spray")
	return PLUGIN_CONTINUE
}

public fw_add_item_post(id, item)
{
	if (!g_File || !pev_valid(item) || !is_user_alive(id))
		return
	// (detail: the CSW id, and ;b in a buy zone: a buy or the spawn's own guns, most likely)
	new detail[12]
	formatex(detail, charsmax(detail), "%d%s", cs_get_weapon_id(item), cs_get_user_buyzone(id) ? ";b" : "")
	event(id, "pickup", detail)
}
