/*
	gm_vip - VIP perks for people the host gives VIP in Loner-Strike's Admins page.

	Who is a VIP: anyone with AMX Mod X access flag "t" (users.ini). The app writes
	those entries; admins and the owner get "t" too. Bots never have flags.

	Perks (each one can be switched off):
	  free kevlar + helmet on spawn, a defuse kit for CTs, free grenades,
	  extra money each round, "VIP" on the scoreboard, a green [VIP] chat tag (staff get their role's: [OWNER], [ADMIN], [MOD]).
	Spawn perks and money only apply in modes where they don't wreck the balance
	(see mode_perks); the scoreboard mark and chat tag work everywhere (flag "t").

	Abilities: the host picks which roles get each one (the Admins page grid).
	A player's role is the highest of owner (flag l), admin (d), moderator (c),
	vip (t). Each ability's cvar is a bitmask of roles: 1 vip, 2 moderator,
	4 admin, 8 owner.
	  gm_vip_gear     the spawn perks and money above
	  gm_vip_hook     the grappling hook add-on (csb_grappling_hook asks gm_can_hook)
	  gm_vip_jump     double jump
	  gm_vip_respawn  say /respawn while dead, once a round
	  gm_vip_noclip   say /noclip
	  gm_vip_god      say /god
	  gm_vip_tp       say /save, then /tp to go back there
	Double jump, respawn, noclip and god are off in modes they break (the same
	modes that get no spawn perks, and Tour of Duty) unless gm_vip_anymode is 1.

	Say /vip   what VIPs get, and what you can use
	Say /donate  the server's donate link (donations are handled outside the game)

	Loner-Strike supporters (community roles, docs/ROLES.md): access flag "s" (the app writes
	them in their own block of users.ini, from the Loner-Strike Discord). They're role bit 16
	in the ability masks, which no default mask has: by default a supporter gets the [VIP] tag,
	the scoreboard mark, a reserved slot and a join message, never the host's gear, money or
	abilities. A VIP who also supports counts as a VIP.
	  gm_vip_reserve  roles mask with a reserved slot (default 17: VIPs and supporters)
	  gm_vip_slots    reserved slots (default 0, off): sv_visiblemaxplayers is set to
	                  maxplayers - slots; someone without a reservation who takes a hidden slot
	                  is kicked ("That slot is kept for supporters"), unless a bot can make room
	                  (a bot is kicked first; bot_quota fills its place again later)
	  gm_vip_joinmsg  roles mask announced when they join (default 16: supporters)
	  gm_vip_sup_chat, gm_vip_sup_score  1/0: the [VIP] chat tag and the scoreboard mark for
	                  supporters (default 1)

	Settings: data/gamemodes/vip.cfg (written by the app), re-read with gm_vipreload.
	Cvars: gm_vip_armor, gm_vip_defuse, gm_vip_nades 0/1, gm_vip_money <amount>,
	       gm_vip_tag, gm_vip_chat 0/1, gm_vip_host 0/1, gm_vip_donate "<url>",
	       the ability masks above, gm_vip_tagroles (the roles whose chat tag shows) and gm_vip_anymode 0/1

	The listen-server host has every flag (users.ini "loopback"), so the host only
	gets perks and abilities (as an owner) with gm_vip_host 1.

	Also runs banned.cfg and listip.cfg on every map, so bans made from the app (or
	with amx_ban) stick on a listen server too, like they do on a dedicated one.
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fakemeta>
#include <fun>
#include <hamsandwich>

#define PLUGIN  "VIP perks"
#define VERSION "1.2"
#define AUTHOR  "gamemodes"

#define VIP_FLAG     ADMIN_LEVEL_H
// a Loner-Strike supporter (community roles): "s"
#define SUPPORTER_FLAG ADMIN_LEVEL_G
#define TASK_JOIN    6200
#define TASK_SPAWN   6100
#define SCORE_VIP    (1<<2)

#define TAG          "^4[Server]^1"

enum { PERKS_NONE, PERKS_LIGHT, PERKS_FULL }

// role bits, as the app writes them
#define ROLE_VIP     1
#define ROLE_MOD     2
#define ROLE_ADMIN   4
#define ROLE_OWNER   8
#define ROLE_SUPPORTER 16

enum { AB_GEAR, AB_HOOK, AB_JUMP, AB_RESPAWN, AB_NOCLIP, AB_GOD, AB_TP, AB_COUNT }
new const AB_CVAR[AB_COUNT][] = { "gm_vip_gear", "gm_vip_hook", "gm_vip_jump", "gm_vip_respawn", "gm_vip_noclip", "gm_vip_god", "gm_vip_tp" }
// defaults: gear and hook VIP and up, noclip and save/teleport admins and up, god owner only
new const AB_DEFAULT[AB_COUNT][] = { "15", "15", "0", "0", "12", "8", "12" }
new const AB_NAME[AB_COUNT][] = { "Spawn gear", "The grappling hook", "Double jump", "Respawning", "Noclip", "God mode", "Save and teleport" }
// the ones that break modes like zombie, hide and seek or deathrun
new const bool:AB_UNFAIR[AB_COUNT] = { false, false, true, true, true, true, false }

new g_pArmor, g_pDefuse, g_pNades, g_pMoney, g_pTag, g_pChat, g_pHost, g_pDonate
new g_pAb[AB_COUNT], g_pAnyMode, g_pTagRoles
new g_pReserve, g_pSlots, g_pJoinMsg, g_pSupChat, g_pSupScore, bool:g_SetVisible
new g_DataDir[128]
new g_Perks = PERKS_NONE
new bool:g_Career

new bool:g_Noclip[33], bool:g_God[33], bool:g_Respawned[33]
new bool:g_Saved[33], Float:g_SavePos[33][3], Float:g_SaveAng[33][3], bool:g_SaveDuck[33]
new g_AirJumps[33], bool:g_DoJump[33]

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
	for (new i = 0; i < AB_COUNT; i++)
		g_pAb[i] = register_cvar(AB_CVAR[i], AB_DEFAULT[i])
	g_pAnyMode = register_cvar("gm_vip_anymode", "0")
	// which roles show their chat tag (the same role bits as the abilities; all by default)
	g_pTagRoles = register_cvar("gm_vip_tagroles", "15")
	g_pReserve = register_cvar("gm_vip_reserve", "17")
	g_pSlots   = register_cvar("gm_vip_slots", "0")
	g_pJoinMsg = register_cvar("gm_vip_joinmsg", "16")
	g_pSupChat = register_cvar("gm_vip_sup_chat", "1")
	g_pSupScore = register_cvar("gm_vip_sup_score", "1")

	// RegisterHamPlayer also covers Condition Zero bots (a different class)
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)
	RegisterHamPlayer(Ham_Killed, "fw_killed_post", 1)
	RegisterHamPlayer(Ham_Player_PreThink, "fw_prethink")
	RegisterHamPlayer(Ham_Player_PostThink, "fw_postthink")
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

	load_settings(false)
	// after vip.cfg ran (it's in the queue at map start), and again later: something at map start
	// puts sv_visiblemaxplayers back to -1 after the first seconds (seen on prod)
	set_task(1.0, "apply_slots")
	set_task(5.0, "apply_slots")
	set_task(15.0, "apply_slots")
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

load_settings(bool:now = true)
{
	new path[160]
	formatex(path, charsmax(path), "%s/vip.cfg", g_DataDir)
	if (file_exists(path))
	{
	// at map start the exec just goes in the queue: running the queue then could also
	// run a map change someone queued, in the middle of this map starting
		server_cmd("exec ^"%s^"", path)
		if (now)
			server_exec()
	}
}

public srv_reload()
{
	load_settings()
	// someone who lost noclip or god mode in the change loses it now
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_Noclip[id] && !can_use(id, AB_NOCLIP))
		{
			set_noclip(id, false)
			client_print_color(id, print_team_default, "%s Noclip was switched off by the host.", TAG)
		}
		if (g_God[id] && !can_use(id, AB_GOD))
		{
			set_god(id, false)
			client_print_color(id, print_team_default, "%s God mode was switched off by the host.", TAG)
		}
	}
	apply_slots()
	server_print("[VIP] settings reloaded")
	return PLUGIN_HANDLED
}

// The [VIP] tag (chat) or the scoreboard mark: VIPs, and Loner-Strike supporters when the host
// lets them have it.
bool:is_marked(id, bool:chat)
{
	if (is_vip(id))
		return true
	if (!get_pcvar_num(chat ? g_pSupChat : g_pSupScore))
		return false
	if (!is_user_connected(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	if (id == 1 && !is_dedicated_server())
		return false
	return (get_user_flags(id) & SUPPORTER_FLAG) != 0
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

// The player's role as one bit (the highest they have), 0 for nobody, bots and
// the listen-server host unless gm_vip_host is on (then the host is an owner).
role_bit(id)
{
	if (!is_user_connected(id) || is_user_bot(id) || is_user_hltv(id))
		return 0
	if (id == 1 && !is_dedicated_server())
		return get_pcvar_num(g_pHost) ? ROLE_OWNER : 0
	new flags = get_user_flags(id)
	if (flags & ADMIN_RCON)
		return ROLE_OWNER
	if (flags & ADMIN_BAN)
		return ROLE_ADMIN
	if (flags & ADMIN_KICK)
		return ROLE_MOD
	if (flags & VIP_FLAG)
		return ROLE_VIP
	if (flags & SUPPORTER_FLAG)
		return ROLE_SUPPORTER
	return 0
}

bool:has_ability(id, ab)
{
	return (get_pcvar_num(g_pAb[ab]) & role_bit(id)) != 0
}

// The mode allows it (double jump, respawn, noclip and god break some modes)
bool:mode_allows(ab)
{
	return !AB_UNFAIR[ab] || g_Perks != PERKS_NONE || get_pcvar_num(g_pAnyMode) != 0
}

bool:can_use(id, ab)
{
	return has_ability(id, ab) && mode_allows(ab)
}

// For csb_grappling_hook (callfunc): 1 when this player may use the hook
public gm_can_hook(id)
{
	return has_ability(id, AB_HOOK) ? 1 : 0
}

// ---- spawn perks -------------------------------------------------------------

public fw_spawn_post(id)
{
	// a new life (and every new round) starts without noclip or god mode
	if (is_user_alive(id))
	{
		if (g_Noclip[id])
			set_noclip(id, false)
		if (g_God[id])
			set_god(id, false)
		g_AirJumps[id] = 0
		g_DoJump[id] = false
	}
	if (g_Perks == PERKS_NONE || !has_ability(id, AB_GEAR))
		return HAM_IGNORED
	// after the mode's own loadout (which may strip weapons on spawn)
	remove_task(id + TASK_SPAWN)
	set_task(0.3, "task_spawn", id + TASK_SPAWN)
	return HAM_IGNORED
}

public task_spawn(taskid)
{
	new id = taskid - TASK_SPAWN
	if (!is_user_alive(id) || !has_ability(id, AB_GEAR))
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

public client_putinserver(id)
{
	reset_player(id)
	// admin.amxx gives flags as they come in: look a moment later
	if (!is_user_bot(id) && !is_user_hltv(id))
		set_task(1.0, "task_joined", TASK_JOIN + id)
}

// ---- reserved slots and join messages ------------------------------------------------------

// AMX Mod X's own slot reservation (adminslots.amxx, amx_reservation 0) puts sv_visiblemaxplayers
// back to -1 whenever someone connects or leaves; this plugin loads after it, so it sets the kept
// slots again right after
public client_authorized(id)
{
	apply_slots()
}

public client_remove(id)
{
	apply_slots()
}

public apply_slots()
{
	new slots = clamp(get_pcvar_num(g_pSlots), 0, MaxClients - 1)
	if (slots > 0)
	{
		if (get_cvar_num("sv_visiblemaxplayers") != MaxClients - slots)
			set_cvar_num("sv_visiblemaxplayers", MaxClients - slots)
		g_SetVisible = true
	}
	else if (g_SetVisible)
	{
		set_cvar_num("sv_visiblemaxplayers", -1)
		g_SetVisible = false
	}
}

public task_joined(taskid)
{
	new id = taskid - TASK_JOIN
	if (!is_user_connected(id))
		return
	apply_slots()
	new role = role_bit(id)
	new slots = clamp(get_pcvar_num(g_pSlots), 0, MaxClients - 1)
	if (slots > 0 && get_playersnum(1) > MaxClients - slots)
	{
		// into a kept slot: a bot makes room if there is one; else only someone with a reservation stays
		new bot = 0
		for (new i = 1; i <= MaxClients; i++)
			if (i != id && is_user_connected(i) && is_user_bot(i))
				bot = i
		if (bot)
			server_cmd("kick #%d ^"Making room^"", get_user_userid(bot))
		else if (!(get_pcvar_num(g_pReserve) & role))
		{
			server_cmd("kick #%d ^"That slot is kept for supporters.^"", get_user_userid(id))
			return
		}
	}
	if (role && (get_pcvar_num(g_pJoinMsg) & role))
	{
		new name[32]
		get_user_name(id, name, charsmax(name))
		if (role == ROLE_SUPPORTER)
			client_print_color(0, id, "^4[VIP]^3 %s^1, a Loner-Strike supporter, joined. Thank you!", name)
		else
			client_print_color(0, id, "^4[VIP]^3 %s^1 joined.", name)
	}
}

public client_disconnected(id)
{
	remove_task(id + TASK_SPAWN)
	remove_task(TASK_JOIN + id)
	reset_player(id)
}

reset_player(id)
{
	g_Noclip[id] = false
	g_God[id] = false
	g_Respawned[id] = false
	g_Saved[id] = false
	g_AirJumps[id] = 0
	g_DoJump[id] = false
}

public fw_killed_post(victim, attacker, gib)
{
	if (g_Noclip[victim])
		set_noclip(victim, false)
	if (g_God[victim])
		set_god(victim, false)
	return HAM_IGNORED
}

// ---- money each round ----------------------------------------------------------

public ev_newround()
{
	// the kept slots again (a map change or another config can put sv_visiblemaxplayers back)
	apply_slots()
	for (new id = 1; id <= MaxClients; id++)
		g_Respawned[id] = false

	new amount = get_pcvar_num(g_pMoney)
	if (g_Perks != PERKS_FULL || amount <= 0)
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (has_ability(id, AB_GEAR))
			cs_set_user_money(id, min(cs_get_user_money(id) + amount, 16000), 1)
	}
}

// ---- double jump: one more jump in the air -------------------------------------------

public fw_prethink(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new flags = pev(id, pev_flags)
	if (flags & FL_ONGROUND)
	{
		g_AirJumps[id] = 0
		return HAM_IGNORED
	}
	// a fresh press of jump in the air (not held), walking (not on a ladder or swimming)
	if ((pev(id, pev_button) & IN_JUMP) && !(pev(id, pev_oldbuttons) & IN_JUMP) && !g_AirJumps[id]
		&& pev(id, pev_movetype) == MOVETYPE_WALK && pev(id, pev_waterlevel) < 2 && can_use(id, AB_JUMP))
	{
		g_AirJumps[id] = 1
		g_DoJump[id] = true
	}
	return HAM_IGNORED
}

public fw_postthink(id)
{
	if (!g_DoJump[id])
		return HAM_IGNORED
	g_DoJump[id] = false
	if (!is_user_alive(id))
		return HAM_IGNORED
	new Float:vel[3]
	pev(id, pev_velocity, vel)
	vel[2] = 270.0
	set_pev(id, pev_velocity, vel)
	return HAM_IGNORED
}

// ---- noclip, god mode ------------------------------------------------------------------

set_noclip(id, bool:on)
{
	g_Noclip[id] = on
	if (is_user_alive(id))
		set_user_noclip(id, on ? 1 : 0)
}

set_god(id, bool:on)
{
	g_God[id] = on
	if (is_user_alive(id))
		set_user_godmode(id, on ? 1 : 0)
}

// Says why not and returns false when the player can't use ab right now.
bool:check(id, ab)
{
	if (!has_ability(id, ab))
	{
		client_print_color(id, print_team_default, "%s %s isn't available to you on this server.", TAG, AB_NAME[ab])
		return false
	}
	if (!mode_allows(ab))
	{
		client_print_color(id, print_team_default, "%s %s is off in this mode, to keep it fair.", TAG, AB_NAME[ab])
		return false
	}
	return true
}

cmd_noclip(id)
{
	if (!check(id, AB_NOCLIP))
		return
	if (!is_user_alive(id))
	{
		client_print_color(id, print_team_default, "%s You need to be alive to use noclip.", TAG)
		return
	}
	set_noclip(id, !g_Noclip[id])
	if (g_Noclip[id])
		client_print_color(id, print_team_default, "%s Noclip ^3on^1: fly through walls. Say ^3/noclip^1 again to stop.", TAG)
	else
		client_print_color(id, print_team_default, "%s Noclip ^3off^1.", TAG)
}

cmd_god(id)
{
	if (!check(id, AB_GOD))
		return
	if (!is_user_alive(id))
	{
		client_print_color(id, print_team_default, "%s You need to be alive to use god mode.", TAG)
		return
	}
	set_god(id, !g_God[id])
	if (g_God[id])
		client_print_color(id, print_team_default, "%s God mode ^3on^1: nothing can hurt you. Say ^3/god^1 again to stop.", TAG)
	else
		client_print_color(id, print_team_default, "%s God mode ^3off^1.", TAG)
}

// ---- respawn once a round ---------------------------------------------------------------

cmd_respawn(id)
{
	if (!check(id, AB_RESPAWN))
		return
	new CsTeams:team = cs_get_user_team(id)
	if (team != CS_TEAM_T && team != CS_TEAM_CT)
	{
		client_print_color(id, print_team_default, "%s Join a team first.", TAG)
		return
	}
	if (is_user_alive(id))
	{
		client_print_color(id, print_team_default, "%s You're still alive. Say ^3/respawn^1 after you die.", TAG)
		return
	}
	if (g_Respawned[id])
	{
		client_print_color(id, print_team_default, "%s You've already respawned this round.", TAG)
		return
	}
	g_Respawned[id] = true
	ExecuteHamB(Ham_CS_RoundRespawn, id)
	client_print_color(id, print_team_default, "%s Back in! You can respawn once a round.", TAG)
}

// ---- save a spot, teleport back ---------------------------------------------------------

cmd_save(id)
{
	if (!check(id, AB_TP))
		return
	if (!is_user_alive(id))
	{
		client_print_color(id, print_team_default, "%s You need to be alive to save a spot.", TAG)
		return
	}
	pev(id, pev_origin, g_SavePos[id])
	pev(id, pev_v_angle, g_SaveAng[id])
	g_SaveDuck[id] = (pev(id, pev_flags) & FL_DUCKING) != 0
	g_Saved[id] = true
	client_print_color(id, print_team_default, "%s Spot saved. Say ^3/tp^1 to come back here.", TAG)
}

cmd_tp(id)
{
	if (!check(id, AB_TP))
		return
	if (!is_user_alive(id))
	{
		client_print_color(id, print_team_default, "%s You need to be alive to teleport.", TAG)
		return
	}
	if (!g_Saved[id])
	{
		client_print_color(id, print_team_default, "%s Say ^3/save^1 first to save a spot.", TAG)
		return
	}
	if (g_SaveDuck[id])
	{
		// crouched there: arrive crouched so a low spot doesn't trap them
		set_pev(id, pev_flags, pev(id, pev_flags) | FL_DUCKING)
		engfunc(EngFunc_SetSize, id, Float:{-16.0, -16.0, -18.0}, Float:{16.0, 16.0, 18.0})
		set_pev(id, pev_view_ofs, Float:{0.0, 0.0, 12.0})
	}
	engfunc(EngFunc_SetOrigin, id, g_SavePos[id])
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_pev(id, pev_angles, g_SaveAng[id])
	set_pev(id, pev_v_angle, g_SaveAng[id])
	set_pev(id, pev_fixangle, 1)
	client_print_color(id, print_team_default, "%s Teleported to your saved spot.", TAG)
}

// ---- scoreboard: the "VIP" mark ---------------------------------------------------

public msg_scoreattrib(msgid, dest, receiver)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= MaxClients && get_pcvar_num(g_pTag) && is_marked(id, false))
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
	// staff by their role, so players can see who runs the server and who's keeping order;
	// then VIPs (and supporters the host lets have it). The listen-server host has no tag.
	if (id < 1 || id > MaxClients)
		return PLUGIN_CONTINUE
	new rb = role_bit(id), tag[12]
	if (rb == ROLE_OWNER)
		copy(tag, charsmax(tag), "[OWNER]")
	else if (rb == ROLE_ADMIN)
		copy(tag, charsmax(tag), "[ADMIN]")
	else if (rb == ROLE_MOD)
		copy(tag, charsmax(tag), "[MOD]")
	else if (is_marked(id, true))
		copy(tag, charsmax(tag), "[VIP]")
	else
		return PLUGIN_CONTINUE
	// a role the Admins page's grid leaves without a tag (a supporter's [VIP] has its own switch)
	if (rb >= ROLE_VIP && rb <= ROLE_OWNER && !(get_pcvar_num(g_pTagRoles) & rb))
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

	set_msg_arg_string(2, fmt("%s^4%s ^3%%s1^1 :  %%s2", prefix, tag))
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

	// abilities: when no role has one, the word is left for other plugins
	new ab = -1
	if (equali(word, "noclip"))
		ab = AB_NOCLIP
	else if (equali(word, "god") || equali(word, "godmode"))
		ab = AB_GOD
	else if (equali(word, "respawn"))
		ab = AB_RESPAWN
	else if (equali(word, "save") || equali(word, "tp"))
		ab = AB_TP
	if (ab < 0 || !get_pcvar_num(g_pAb[ab]) || is_user_bot(id))
		return PLUGIN_CONTINUE

	switch (ab)
	{
		case AB_NOCLIP: cmd_noclip(id)
		case AB_GOD: cmd_god(id)
		case AB_RESPAWN: cmd_respawn(id)
		default:
		{
			if (equali(word, "save"))
				cmd_save(id)
			else
				cmd_tp(id)
		}
	}
	return PLUGIN_HANDLED
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
	new mine[160]
	if (can_use(id, AB_JUMP))
		add(mine, charsmax(mine), "double jump, ")
	if (can_use(id, AB_RESPAWN))
		add(mine, charsmax(mine), "/respawn, ")
	if (can_use(id, AB_NOCLIP))
		add(mine, charsmax(mine), "/noclip, ")
	if (can_use(id, AB_GOD))
		add(mine, charsmax(mine), "/god, ")
	if (can_use(id, AB_TP))
		add(mine, charsmax(mine), "/save and /tp, ")
	if (has_ability(id, AB_HOOK) && cvar_exists("csb_hook_enabled"))
		add(mine, charsmax(mine), "the hook (bind a key to +hook), ")
	len = strlen(mine)
	if (len >= 2)
	{
		mine[len - 2] = 0
		client_print_color(id, print_team_default, "^4[VIP]^1 You can use: %s.", mine)
	}
	if (is_vip(id))
		client_print_color(id, print_team_default, "^4[VIP]^1 You're a VIP. Thanks for the support!")
	else if (role_bit(id) == ROLE_SUPPORTER)
		client_print_color(id, print_team_default, "^4[VIP]^1 You support Loner-Strike: thank you!")
	else
	{
		new url[8]
		get_pcvar_string(g_pDonate, url, charsmax(url))
		if (url[0])
			client_print_color(id, print_team_default, "^4[VIP]^1 Say ^3/donate^1 to support the server.")
	}
}
