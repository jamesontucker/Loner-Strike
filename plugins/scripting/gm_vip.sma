/*
	gm_vip - VIP perks for people the host gives VIP in Loner-Strike's Admins page.

	Who is a VIP: anyone with AMX Mod X access flag "t" (users.ini). The app writes
	those entries; admins and the owner get "t" too. Bots never have flags.

	Perks (each one can be switched off):
	  free kevlar + helmet on spawn, a defuse kit for CTs, free grenades,
	  extra money each round, "VIP" on the scoreboard, a green [VIP] chat tag.
	Spawn perks and money only apply in modes where they don't wreck the balance
	(see mode_perks); the scoreboard mark and chat tag work everywhere.

	Say /vip   what VIPs get
	Say /donate  the server's donate link (donations are handled outside the game)

	Settings: data/gamemodes/vip.cfg (written by the app), re-read with gm_vipreload.
	Cvars: gm_vip_armor, gm_vip_defuse, gm_vip_nades 0/1, gm_vip_money <amount>,
	       gm_vip_tag, gm_vip_chat 0/1, gm_vip_host 0/1, gm_vip_donate "<url>"

	The listen-server host has every flag (users.ini "loopback"), so the host only
	gets perks with gm_vip_host 1.

	Also runs banned.cfg and listip.cfg on every map, so bans made from the app (or
	with amx_ban) stick on a listen server too, like they do on a dedicated one.
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fun>
#include <hamsandwich>

#define PLUGIN  "VIP perks"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define VIP_FLAG     ADMIN_LEVEL_H
#define TASK_SPAWN   6100
#define SCORE_VIP    (1<<2)

enum { PERKS_NONE, PERKS_LIGHT, PERKS_FULL }

new g_pArmor, g_pDefuse, g_pNades, g_pMoney, g_pTag, g_pChat, g_pHost, g_pDonate
new g_DataDir[128]
new g_Perks = PERKS_NONE
new bool:g_Career

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pArmor  = register_cvar("gm_vip_armor", "1")
	g_pDefuse = register_cvar("gm_vip_defuse", "1")
	g_pNades  = register_cvar("gm_vip_nades", "1")
	g_pMoney  = register_cvar("gm_vip_money", "500")
	g_pTag    = register_cvar("gm_vip_tag", "1")
	g_pChat   = register_cvar("gm_vip_chat", "1")
	g_pHost   = register_cvar("gm_vip_host", "0")
	g_pDonate = register_cvar("gm_vip_donate", "")

	// RegisterHamPlayer also covers Condition Zero bots (a different class)
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)
	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	register_message(get_user_msgid("ScoreAttrib"), "msg_scoreattrib")
	register_message(get_user_msgid("SayText"), "msg_saytext")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say")
	register_srvcmd("gm_vipreload", "srv_reload")

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
}

public plugin_cfg()
{
	// Tour of Duty missions play as designed (same check as the other plugins)
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))

	new mode[32]
	read_line(fmt("%s/current.txt", g_DataDir), mode, charsmax(mode))
	g_Perks = g_Career ? PERKS_NONE : mode_perks(mode)

	load_settings()
	if (file_exists("banned.cfg"))
		server_cmd("exec banned.cfg")
	if (file_exists("listip.cfg"))
		server_cmd("exec listip.cfg")
}

// Which spawn perks a Game Mode Menu mode can take without breaking it.
// Unknown modes (new add-ons) get none, to be safe.
mode_perks(const mode[])
{
	if (!mode[0] || equal(mode, "classic") || equal(mode, "war3") || equal(mode, "shero") || equal(mode, "surf"))
		return PERKS_FULL
	// armor and a kit only: free grenades or money would change these modes
	if (equal(mode, "csdm") || equal(mode, "csdm_ffa") || equal(mode, "pistols") || equal(mode, "deagle")
		|| equal(mode, "awp") || equal(mode, "scoutz") || equal(mode, "gungame") || equal(mode, "gungame_team"))
		return PERKS_LIGHT
	// knife, zombie, zp5, hns, deathrun, kreedz, basebuilder, soccerjam, paintball...
	return PERKS_NONE
}

read_line(const path[], out[], len)
{
	out[0] = 0
	new f = fopen(path, "rt")
	if (f)
	{
		fgets(f, out, len)
		trim(out)
		fclose(f)
	}
}

load_settings()
{
	new path[160]
	formatex(path, charsmax(path), "%s/vip.cfg", g_DataDir)
	if (file_exists(path))
	{
		server_cmd("exec ^"%s^"", path)
		server_exec()
	}
}

public srv_reload()
{
	load_settings()
	server_print("[VIP] settings reloaded")
	return PLUGIN_HANDLED
}

bool:is_vip(id)
{
	if (!is_user_connected(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	if (!(get_user_flags(id) & VIP_FLAG))
		return false
	// the listen-server host has every flag; perks only when the host asks for them
	if (id == 1 && !is_dedicated_server() && !get_pcvar_num(g_pHost))
		return false
	return true
}

// ---- spawn perks -------------------------------------------------------------

public fw_spawn_post(id)
{
	if (g_Perks == PERKS_NONE || !is_vip(id))
		return HAM_IGNORED
	// after the mode's own loadout (which may strip weapons on spawn)
	remove_task(id + TASK_SPAWN)
	set_task(0.3, "task_spawn", id + TASK_SPAWN)
	return HAM_IGNORED
}

public task_spawn(taskid)
{
	new id = taskid - TASK_SPAWN
	if (!is_user_alive(id) || !is_vip(id))
		return
	new CsTeams:team = cs_get_user_team(id)
	if (team != CS_TEAM_T && team != CS_TEAM_CT)
		return

	if (get_pcvar_num(g_pArmor))
	{
		new CsArmorType:kind
		if (cs_get_user_armor(id, kind) < 100 || kind != CS_ARMOR_VESTHELM)
			cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM)
	}
	if (get_pcvar_num(g_pDefuse) && team == CS_TEAM_CT && !cs_get_user_defuse(id))
		cs_set_user_defuse(id, 1)

	if (g_Perks == PERKS_FULL && get_pcvar_num(g_pNades))
	{
		if (!user_has_weapon(id, CSW_HEGRENADE))
			give_item(id, "weapon_hegrenade")
		if (!user_has_weapon(id, CSW_FLASHBANG))
			give_item(id, "weapon_flashbang")
		if (!user_has_weapon(id, CSW_SMOKEGRENADE))
			give_item(id, "weapon_smokegrenade")
	}
}

public client_disconnected(id)
{
	remove_task(id + TASK_SPAWN)
}

// ---- money each round ----------------------------------------------------------

public ev_newround()
{
	new amount = get_pcvar_num(g_pMoney)
	if (g_Perks != PERKS_FULL || amount <= 0)
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (is_vip(id))
			cs_set_user_money(id, min(cs_get_user_money(id) + amount, 16000), 1)
	}
}

// ---- scoreboard: the "VIP" mark ---------------------------------------------------

public msg_scoreattrib(msgid, dest, receiver)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= MaxClients && get_pcvar_num(g_pTag) && is_vip(id))
		set_msg_arg_int(2, ARG_BYTE, get_msg_arg_int(2) | SCORE_VIP)
	return PLUGIN_CONTINUE
}

// ---- chat: green [VIP] tag ---------------------------------------------------------
// Rewrites the chat line the game sends, so no say command is ever swallowed:
// commands other plugins handle (/gm, /guns, rtv...) never reach this message.

public msg_saytext(msgid, dest, receiver)
{
	if (get_msg_args() < 4 || !get_pcvar_num(g_pChat))
		return PLUGIN_CONTINUE
	new id = get_msg_arg_int(1)
	if (id < 1 || id > MaxClients || !is_vip(id))
		return PLUGIN_CONTINUE

	new token[32]
	get_msg_arg_string(2, token, charsmax(token))
	new prefix[40]
	if (equal(token, "#Cstrike_Chat_All"))
		prefix[0] = 0
	else if (equal(token, "#Cstrike_Chat_AllDead"))
		copy(prefix, charsmax(prefix), "^1*DEAD* ")
	else if (equal(token, "#Cstrike_Chat_AllSpec"))
		copy(prefix, charsmax(prefix), "^1*SPEC* ")
	else if (equal(token, "#Cstrike_Chat_CT"))
		copy(prefix, charsmax(prefix), "^1(Counter-Terrorist) ")
	else if (equal(token, "#Cstrike_Chat_T"))
		copy(prefix, charsmax(prefix), "^1(Terrorist) ")
	else if (equal(token, "#Cstrike_Chat_CT_Dead"))
		copy(prefix, charsmax(prefix), "^1*DEAD*(Counter-Terrorist) ")
	else if (equal(token, "#Cstrike_Chat_T_Dead"))
		copy(prefix, charsmax(prefix), "^1*DEAD*(Terrorist) ")
	else if (equal(token, "#Cstrike_Chat_Spec"))
		copy(prefix, charsmax(prefix), "^1(Spectator) ")
	else
		return PLUGIN_CONTINUE      // name changes, plugin messages: leave alone

	set_msg_arg_string(2, fmt("%s^4[VIP] ^3%%s1^1 :  %%s2", prefix))
	return PLUGIN_CONTINUE
}

// ---- say /vip, /donate ----------------------------------------------------------------

public cmd_say(id)
{
	new msg[32], word[16], rest[16]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	trim(msg)
	if (msg[0] != '/' && msg[0] != '!')
		return PLUGIN_CONTINUE
	strtok2(msg[1], word, charsmax(word), rest, charsmax(rest), ' ', TRIM_FULL)

	if (equali(word, "donate"))
	{
		show_donate(id)
		return PLUGIN_HANDLED
	}
	if (equali(word, "vip") || equali(word, "vips"))
	{
		show_vip(id)
		return PLUGIN_HANDLED
	}
	return PLUGIN_CONTINUE
}

show_donate(id)
{
	new url[128]
	get_pcvar_string(g_pDonate, url, charsmax(url))
	if (!url[0])
	{
		client_print_color(id, print_team_default, "^4[VIP]^1 This server doesn't take donations.")
		return
	}
	client_print_color(id, print_team_default, "^4[VIP]^1 Like the server? You can support it here:")
	client_print_color(id, print_team_default, "^3%s", url)
	client_print_color(id, print_team_default, "^4[VIP]^1 The host gives supporters VIP by hand. Say ^3/vip^1 to see the perks.")
	client_print(id, print_console, "Donate: %s", url)
}

show_vip(id)
{
	new list[160]
	if (get_pcvar_num(g_pArmor))
		add(list, charsmax(list), "armor + helmet, ")
	if (get_pcvar_num(g_pDefuse))
		add(list, charsmax(list), "defuse kit, ")
	if (get_pcvar_num(g_pNades))
		add(list, charsmax(list), "grenades, ")
	if (get_pcvar_num(g_pMoney) > 0)
		add(list, charsmax(list), fmt("+$%d a round, ", get_pcvar_num(g_pMoney)))
	if (get_pcvar_num(g_pTag))
		add(list, charsmax(list), "VIP on the scoreboard, ")
	if (get_pcvar_num(g_pChat))
		add(list, charsmax(list), "[VIP] chat tag, ")
	new len = strlen(list)
	if (len >= 2)
		list[len - 2] = 0
	else
		copy(list, charsmax(list), "nothing right now")

	client_print_color(id, print_team_default, "^4[VIP]^1 VIPs get: %s.", list)
	if (g_Perks == PERKS_LIGHT)
		client_print_color(id, print_team_default, "^4[VIP]^1 This mode only allows armor and the defuse kit.")
	else if (g_Perks == PERKS_NONE)
		client_print_color(id, print_team_default, "^4[VIP]^1 This mode has no spawn perks, to keep it fair.")
	if (is_vip(id))
		client_print_color(id, print_team_default, "^4[VIP]^1 You're a VIP. Thanks for the support!")
	else
	{
		new url[8]
		get_pcvar_string(g_pDonate, url, charsmax(url))
		if (url[0])
			client_print_color(id, print_team_default, "^4[VIP]^1 Say ^3/donate^1 to support the server.")
	}
}
