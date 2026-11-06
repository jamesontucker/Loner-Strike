/*
	gm_scrim - the "Match (5v5)" game mode: scrims and pick-up games.

	Warmup: free respawns and $16000 until enough players type /ready (or an admin
	  types /start). The HUD shows how many are ready.
	Knife round (optional): knives only; the winners vote to stay or switch sides.
	Live: first to more than half of gm_scrim_maxrounds (MR15 = 30 rounds, MR12 = 24).
	  Teams swap sides (and scores) at half-time. With overtime on, a tie plays MR3
	  blocks with $10000: first to 4 of 6 rounds, swapping after 3.
	Each round starts with the score in chat ("CT 7 - 5 T"); the scoreboard shows the
	  match score, not the game's own (which restarts at half-time).
	Pauses: /pause by a team's captain (a team gets gm_scrim_pauses per match) or an
	  admin. It starts at the next freeze time and holds the whole game (the engine's
	  pause) until /unpause by that team's captain or an admin, or by anyone once
	  gm_scrim_pausetime seconds have passed. Captain: on each team, the player who
	  typed /ready first (else the first human on the team).
	Admin (flag c, or the listen-server host): /start, /restart (the match from 0-0),
	  /warmup (stop the match).
	Results: data/gamemodes/matches.json, one JSON object per line: date, map, both
	  teams' players and scores. data/gamemodes/scrim_status.json (gm_scrimstatus)
	  is the live state for the app.
	Bots: gm_scrim_bots 0 removes them (bot_quota 0); 1 fills the teams up to
	  gm_scrim_players during warmup. Bots count as ready. Hooks use RegisterHamPlayer,
	  so Condition Zero bots (another class) are covered.

	Settings: data/gamemodes/scrim.cfg (written by the app), re-read with gm_scrimreload.
	Server commands: gm_scrimreload, gm_scrimstatus, gm_scrim <start|restart|warmup|pause|unpause>
	Cvars: gm_scrim_maxrounds, gm_scrim_overtime, gm_scrim_knife, gm_scrim_bots,
	       gm_scrim_players, gm_scrim_pauses, gm_scrim_pausetime
*/

#include <amxmodx>
#include <amxmisc>
#include <cstrike>
#include <fun>
#include <fakemeta>
#include <hamsandwich>

#define PLUGIN  "Match mode"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define TAG          "^4[Match]^1"
#define TASK_HUD     7600
#define TASK_RESPAWN 7700
#define TASK_STRIP   7800
#define TASK_START   7900
#define TASK_VOTE    7950
#define TASK_NEXT    7960
#define TASK_BOTS    7970
#define OT_MONEY     10000
#define OT_HALF      3
#define MAX_ROSTER   16

enum { PH_NONE, PH_WARMUP, PH_KNIFE, PH_VOTE, PH_LIVE, PH_OVER }
new const PHASE_NAME[][] = { "none", "warmup", "knife", "vote", "live", "over" }

// Cvars changed for warmup and the knife round, put back when the match goes live
new const OVERRIDE[][] = { "mp_round_infinite", "mp_buy_anywhere", "mp_freezetime", "mp_friendlyfire", "mp_weapons_allow_map_placed", "mp_give_player_c4" }
new g_Live[sizeof OVERRIDE][16], bool:g_HaveLive

new g_pMaxRounds, g_pOvertime, g_pKnife, g_pBots, g_pPlayers, g_pPauses, g_pPauseTime
new g_DataDir[128], g_Hud, g_MsgTeamScore
new bool:g_Career

new g_Phase = PH_NONE
new bool:g_Ready[33], g_ReadyOrder[33], g_ReadyCount

// Live match. Team 0 is the team that was CT when the match went live.
new g_Score[2], CsTeams:g_Side0, g_Rounds, g_Half, g_MaxRounds, bool:g_OtOn
new bool:g_InOt, g_OtRound, g_OtBase[2], g_OtBlock
new bool:g_RoundOver, bool:g_IgnoreEnd, bool:g_InFreeze, bool:g_AnnounceLive
new g_StartMoney[16], bool:g_MoneyChanged
new g_Roster[2][MAX_ROSTER][32], g_RosterN[2]

// Knife round vote
new CsTeams:g_VoteSide, g_Vote[33], g_VoteMenu = -1

// Pauses
new g_PausesLeft[2], bool:g_PausePending, g_PauseTeam = -1, bool:g_Paused, g_PausedAt, g_PauseBy[32]

set_phase(ph)
{
	g_Phase = ph
	set_cvar_num("gm_scrim_phase", ph == PH_OVER ? PH_NONE : ph)
}

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	// other plugins read this (gm_extras leaves warmup alone); a new map starts with no match
	set_pcvar_num(register_cvar("gm_scrim_phase", "0"), PH_NONE)
	g_pMaxRounds = register_cvar("gm_scrim_maxrounds", "30")
	g_pOvertime  = register_cvar("gm_scrim_overtime", "1")
	g_pKnife     = register_cvar("gm_scrim_knife", "1")
	g_pBots      = register_cvar("gm_scrim_bots", "0")
	g_pPlayers   = register_cvar("gm_scrim_players", "10")
	g_pPauses    = register_cvar("gm_scrim_pauses", "2")
	g_pPauseTime = register_cvar("gm_scrim_pausetime", "60")

	register_srvcmd("gm_scrimreload", "srv_reload")
	register_srvcmd("gm_scrimstatus", "srv_status")
	register_srvcmd("gm_scrim", "srv_scrim")

	say_cmd("ready", "cmd_ready")
	say_cmd("r", "cmd_ready")
	say_cmd("notready", "cmd_notready")
	say_cmd("unready", "cmd_notready")
	say_cmd("start", "cmd_start")
	say_cmd("forcestart", "cmd_start")
	say_cmd("pause", "cmd_pause")
	say_cmd("unpause", "cmd_unpause")
	say_cmd("resume", "cmd_unpause")
	say_cmd("restart", "cmd_restart")
	say_cmd("warmup", "cmd_warmup")
	say_cmd("score", "cmd_score")
	say_cmd("stay", "cmd_stay")
	say_cmd("switch", "cmd_switch")
	say_cmd("match", "cmd_help")

	// RegisterHamPlayer also covers Condition Zero bots (a different class)
	RegisterHamPlayer(Ham_Spawn, "fw_spawn_post", 1)
	RegisterHamPlayer(Ham_Killed, "fw_killed_post", 1)
	RegisterHam(Ham_Touch, "weaponbox", "fw_touch_weapon")
	RegisterHam(Ham_Touch, "armoury_entity", "fw_touch_weapon")

	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	register_logevent("ev_round_start", 2, "1=Round_Start")
	register_event("SendAudio", "ev_twin", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ctwin", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "ev_draw", "a", "2&%!MRAD_rounddraw")
	register_event("TeamInfo", "ev_teaminfo", "a")
	g_MsgTeamScore = get_user_msgid("TeamScore")
	register_message(g_MsgTeamScore, "msg_teamscore")

	get_datadir(g_DataDir, charsmax(g_DataDir))
	add(g_DataDir, charsmax(g_DataDir), "/gamemodes")
	g_Hud = CreateHudSyncObj()
}

say_cmd(const name[], const handler[])
{
	register_clcmd(fmt("say /%s", name), handler)
	register_clcmd(fmt("say_team /%s", name), handler)
	register_clcmd(fmt("say .%s", name), handler)
	register_clcmd(fmt("say_team .%s", name), handler)
}

public plugin_cfg()
{
	// Tour of Duty missions play as designed (same check as the other plugins)
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
	if (g_Career)
		return
	// The Game Mode Menu puts the mode's and the saved settings back at 4 seconds
	set_task(6.0, "task_begin")
}

public plugin_end()
{
	restore_live()
	restore_money()
}

public task_begin()
{
	load_settings()
	apply_bots()
	enter_warmup(false)
	set_task(1.0, "task_hud", TASK_HUD, _, _, "b")
	set_task(5.0, "task_bots", TASK_BOTS, _, _, "b")
}

load_settings()
{
	new path[160]
	formatex(path, charsmax(path), "%s/scrim.cfg", g_DataDir)
	if (file_exists(path))
	{
		server_cmd("exec ^"%s^"", path)
		server_exec()
	}
}

public srv_reload()
{
	if (g_Career)
		return PLUGIN_HANDLED
	load_settings()
	apply_bots()
	server_print("[Match] settings reloaded")
	return PLUGIN_HANDLED
}

// gm_scrim <start|restart|warmup|pause|unpause>: the app's buttons (as an admin)
public srv_scrim()
{
	if (g_Career || g_Phase == PH_NONE)
	{
		server_print("[Match] Not running.")
		return PLUGIN_HANDLED
	}
	new a[16]
	read_argv(1, a, charsmax(a))
	if (equal(a, "start"))        admin_start("The host")
	else if (equal(a, "restart")) admin_restart("The host")
	else if (equal(a, "warmup"))  admin_warmup("The host")
	else if (equal(a, "pause"))   request_pause(0, -1, "The host")
	else if (equal(a, "unpause")) do_unpause("The host")
	else                          server_print("[Match] Unknown action.")
	write_status()
	return PLUGIN_HANDLED
}

msg(id, const text[], any:...)
{
	new buf[192]
	vformat(buf, charsmax(buf), text, 3)
	client_print_color(id, print_team_default, "%s %s", TAG, buf)
	if (id == 0)
		server_print("[Match] %s", buf)
}

bool:is_admin(id)
{
	// the listen-server host is player 1
	return (get_user_flags(id) & ADMIN_KICK) != 0 || (!is_dedicated_server() && id == 1)
}

bool:on_team(id)
{
	new CsTeams:t = cs_get_user_team(id)
	return t == CS_TEAM_T || t == CS_TEAM_CT
}

bool:is_human(id)
{
	return is_user_connected(id) && !is_user_bot(id) && !is_user_hltv(id)
}

// 0 or 1 (match team) for a side; sides are CS_TEAM_T / CS_TEAM_CT
team_of(CsTeams:side)
{
	return side == g_Side0 ? 0 : 1
}

side_score(CsTeams:side)
{
	return g_Score[team_of(side)]
}

// The captain: the player on that side who typed /ready first, else its first human
captain_of(CsTeams:side)
{
	new best = 0, order = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_human(p) || cs_get_user_team(p) != side)
			continue
		if (g_ReadyOrder[p] && (!order || g_ReadyOrder[p] < order))
		{
			best = p
			order = g_ReadyOrder[p]
		}
		else if (!best)
			best = p
	}
	return best
}

score_line(out[], len)
{
	formatex(out, len, "CT %d - %d T", side_score(CS_TEAM_CT), side_score(CS_TEAM_T))
}

// ---------------------------------------------------------------------------
// Cvars for warmup and the knife round
// ---------------------------------------------------------------------------

save_live()
{
	if (g_HaveLive)
		return
	for (new i = 0; i < sizeof OVERRIDE; i++)
		if (get_cvar_pointer(OVERRIDE[i]))
			get_cvar_string(OVERRIDE[i], g_Live[i], charsmax(g_Live[]))
	g_HaveLive = true
}

restore_live()
{
	if (!g_HaveLive)
		return
	for (new i = 0; i < sizeof OVERRIDE; i++)
		if (get_cvar_pointer(OVERRIDE[i]))
			set_cvar_string(OVERRIDE[i], g_Live[i])
	g_HaveLive = false
}

set_if(const cvar[], const value[])
{
	if (get_cvar_pointer(cvar))
		set_cvar_string(cvar, value)
}

// Overtime money, put back afterwards
set_ot_money()
{
	if (!g_MoneyChanged)
	{
		get_cvar_string("mp_startmoney", g_StartMoney, charsmax(g_StartMoney))
		g_MoneyChanged = true
	}
	set_cvar_num("mp_startmoney", OT_MONEY)
}

restore_money()
{
	if (!g_MoneyChanged)
		return
	set_cvar_string("mp_startmoney", g_StartMoney)
	g_MoneyChanged = false
}

// ---------------------------------------------------------------------------
// Bots
// ---------------------------------------------------------------------------

apply_bots()
{
	if (!get_cvar_pointer("bot_quota"))
		return
	if (!get_pcvar_num(g_pBots))
	{
		if (get_cvar_num("bot_quota") != 0)
			server_cmd("bot_quota 0")
		return
	}
	fill_bots()
}

// Fills the teams up to gm_scrim_players while there's no match on
fill_bots()
{
	new humans = 0
	for (new p = 1; p <= MaxClients; p++)
		if (is_human(p) && cs_get_user_team(p) != CS_TEAM_SPECTATOR)
			humans++
	new want = clamp(get_pcvar_num(g_pPlayers), 2, MaxClients)
	new mode[16]
	get_cvar_string("bot_quota_mode", mode, charsmax(mode))
	new quota = equali(mode, "fill") ? want : max(0, want - humans)
	if (get_cvar_num("bot_quota") != quota)
		server_cmd("bot_quota %d", quota)
}

public task_bots()
{
	if (get_pcvar_num(g_pBots) && g_Phase == PH_WARMUP && get_cvar_pointer("bot_quota"))
		fill_bots()
}

// ---------------------------------------------------------------------------
// Warmup and ready
// ---------------------------------------------------------------------------

enter_warmup(bool:restart)
{
	remove_task(TASK_START)
	remove_task(TASK_VOTE)
	remove_task(TASK_NEXT)
	close_vote()
	set_phase(PH_WARMUP)
	g_Paused = false
	g_PausePending = false
	restore_money()
	arrayset(g_Ready, false, sizeof g_Ready)
	arrayset(g_ReadyOrder, 0, sizeof g_ReadyOrder)
	g_ReadyCount = 0
	save_live()
	set_if("mp_round_infinite", "1")
	set_if("mp_buy_anywhere", "1")
	set_if("mp_freezetime", "0")
	set_if("mp_friendlyfire", "0")
	set_if("mp_give_player_c4", "1")
	restore_one("mp_weapons_allow_map_placed")
	if (restart)
	{
		g_IgnoreEnd = true
		server_cmd("sv_restart 1")
	}
	else
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && on_team(p) && !is_user_alive(p))
				queue_respawn(p, 1.0)
	msg(0, "Warmup. Type ^4/ready^1 when you are; the match starts when %d players are ready.", needed())
}

restore_one(const cvar[])
{
	for (new i = 0; i < sizeof OVERRIDE; i++)
		if (equal(OVERRIDE[i], cvar) && g_HaveLive)
			set_if(cvar, g_Live[i])
}

needed()
{
	return clamp(get_pcvar_num(g_pPlayers), 1, MaxClients)
}

// Ready players on a team; bots are always ready
ready_count()
{
	new n = 0
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_hltv(p) && on_team(p) && (is_user_bot(p) || g_Ready[p]))
			n++
	return n
}

public cmd_ready(id)
{
	if (g_Phase != PH_WARMUP)
		return PLUGIN_HANDLED
	if (!on_team(id))
	{
		msg(id, "Join a team first.")
		return PLUGIN_HANDLED
	}
	if (g_Ready[id])
	{
		msg(id, "You're already ready. Type ^4/notready^1 to take it back.")
		return PLUGIN_HANDLED
	}
	g_Ready[id] = true
	g_ReadyOrder[id] = ++g_ReadyCount
	new name[32]
	get_user_name(id, name, charsmax(name))
	msg(0, "%s is ready (%d/%d).", name, ready_count(), needed())
	check_ready()
	return PLUGIN_HANDLED
}

public cmd_notready(id)
{
	if (g_Phase != PH_WARMUP || !g_Ready[id])
		return PLUGIN_HANDLED
	g_Ready[id] = false
	g_ReadyOrder[id] = 0
	new name[32]
	get_user_name(id, name, charsmax(name))
	msg(0, "%s isn't ready any more (%d/%d).", name, ready_count(), needed())
	if (task_exists(TASK_START))
	{
		remove_task(TASK_START)
		msg(0, "Start cancelled.")
	}
	return PLUGIN_HANDLED
}

check_ready()
{
	if (g_Phase != PH_WARMUP || task_exists(TASK_START))
		return
	if (ready_count() >= needed())
	{
		msg(0, "Everyone's ready. Starting in 5 seconds.")
		set_task(5.0, "task_start", TASK_START)
	}
}

public task_start()
{
	if (g_Phase != PH_WARMUP)
		return
	start_match()
}

start_match()
{
	if (get_pcvar_num(g_pKnife))
		start_knife()
	else
		go_live(false)
}

public cmd_start(id)
{
	if (!is_admin(id))
	{
		msg(id, "Only an admin can force the start. Type ^4/ready^1 instead.")
		return PLUGIN_HANDLED
	}
	new name[32]
	get_user_name(id, name, charsmax(name))
	admin_start(name)
	return PLUGIN_HANDLED
}

admin_start(const who[])
{
	if (g_Phase != PH_WARMUP)
	{
		server_print("[Match] The match has already started.")
		return
	}
	remove_task(TASK_START)
	msg(0, "%s started the match.", who)
	start_match()
}

// ---------------------------------------------------------------------------
// Knife round and the side vote
// ---------------------------------------------------------------------------

start_knife()
{
	set_phase(PH_KNIFE)
	save_live()
	set_if("mp_round_infinite", "0")
	set_if("mp_buy_anywhere", "0")
	set_if("mp_freezetime", "3")
	set_if("mp_friendlyfire", "0")
	set_if("mp_weapons_allow_map_placed", "0")
	set_if("mp_give_player_c4", "0")
	g_IgnoreEnd = true
	server_cmd("sv_restart 1")
	msg(0, "Knife round. The winners pick their side.")
}

knife_won(CsTeams:side)
{
	set_phase(PH_VOTE)
	g_VoteSide = side
	arrayset(g_Vote, 0, sizeof g_Vote)
	new voters = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_human(p) || cs_get_user_team(p) != side)
			continue
		show_vote(p)
		voters++
	}
	msg(0, "%s won the knife round and are choosing a side.", side == CS_TEAM_CT ? "The CTs" : "The Terrorists")
	set_task(voters ? 15.0 : 3.0, "task_vote_end", TASK_VOTE)
}

show_vote(id)
{
	if (g_VoteMenu == -1)
	{
		g_VoteMenu = menu_create("You won the knife round.^nPick your side:", "menu_vote")
		menu_additem(g_VoteMenu, g_VoteSide == CS_TEAM_CT ? "Stay CT" : "Stay T", "1")
		menu_additem(g_VoteMenu, g_VoteSide == CS_TEAM_CT ? "Switch to T" : "Switch to CT", "2")
		menu_setprop(g_VoteMenu, MPROP_EXIT, MEXIT_NEVER)
	}
	menu_display(id, g_VoteMenu)
}

close_vote()
{
	if (g_VoteMenu != -1)
	{
		for (new p = 1; p <= MaxClients; p++)
		{
			new menu, newmenu
			if (is_human(p) && player_menu_info(p, menu, newmenu) && newmenu == g_VoteMenu)
				menu_cancel(p)
		}
		menu_destroy(g_VoteMenu)
		g_VoteMenu = -1
	}
}

public menu_vote(id, menu, item)
{
	if (item < 0 || g_Phase != PH_VOTE)
		return PLUGIN_HANDLED
	cast_vote(id, item == 0 ? 1 : 2)
	return PLUGIN_HANDLED
}

public cmd_stay(id)
{
	cast_vote(id, 1)
	return PLUGIN_HANDLED
}

public cmd_switch(id)
{
	cast_vote(id, 2)
	return PLUGIN_HANDLED
}

cast_vote(id, choice)
{
	if (g_Phase != PH_VOTE || cs_get_user_team(id) != g_VoteSide)
		return
	g_Vote[id] = choice
	new name[32]
	get_user_name(id, name, charsmax(name))
	for (new p = 1; p <= MaxClients; p++)
		if (is_human(p) && cs_get_user_team(p) == g_VoteSide)
			msg(p, "%s votes to %s.", name, choice == 1 ? "stay" : "switch")
	// everyone on the team has voted: no need to wait
	new left = 0
	for (new p = 1; p <= MaxClients; p++)
		if (is_human(p) && cs_get_user_team(p) == g_VoteSide && !g_Vote[p])
			left++
	if (!left)
	{
		remove_task(TASK_VOTE)
		set_task(1.0, "task_vote_end", TASK_VOTE)
	}
}

public task_vote_end()
{
	if (g_Phase != PH_VOTE)
		return
	new stay = 0, swap = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_human(p) || cs_get_user_team(p) != g_VoteSide)
			continue
		if (g_Vote[p] == 1) stay++
		else if (g_Vote[p] == 2) swap++
	}
	close_vote()
	new bool:sw = swap > stay
	msg(0, sw ? "They switch sides." : "They stay on their side.")
	go_live(sw)
}

// ---------------------------------------------------------------------------
// Live
// ---------------------------------------------------------------------------

go_live(bool:swap)
{
	if (swap)
		swap_teams()
	restore_live()
	restore_money()
	// the match decides when it's over, not the map
	set_cvar_num("mp_maxrounds", 0)
	set_cvar_num("mp_winlimit", 0)
	set_cvar_num("mp_timelimit", 0)
	if (get_pcvar_num(g_pBots) && get_cvar_pointer("bot_quota"))
		fill_bots()

	set_phase(PH_LIVE)
	g_Side0 = CS_TEAM_CT
	g_Score[0] = g_Score[1] = 0
	g_Rounds = 0
	g_Half = 1
	g_MaxRounds = get_pcvar_num(g_pMaxRounds) == 24 ? 24 : 30
	g_OtOn = get_pcvar_num(g_pOvertime) != 0
	g_InOt = false
	g_OtRound = g_OtBlock = 0
	g_PausesLeft[0] = g_PausesLeft[1] = clamp(get_pcvar_num(g_pPauses), 0, 10)
	g_PausePending = false
	g_RosterN[0] = g_RosterN[1] = 0
	g_AnnounceLive = true
	g_IgnoreEnd = true
	server_cmd("sv_restart 1")
	msg(0, "Going live: first to %d (MR%d)%s.", g_MaxRounds / 2 + 1, g_MaxRounds / 2, g_OtOn ? ", overtime on a tie" : "")
}

// Everyone on T goes CT and back, keeping a matching player model
swap_teams()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_hltv(p))
			continue
		new CsInternalModel:model
		new CsTeams:t = cs_get_user_team(p, model)
		if (t == CS_TEAM_T)
			cs_set_user_team(p, CS_TEAM_CT, swap_model(model, true))
		else if (t == CS_TEAM_CT)
			cs_set_user_team(p, CS_TEAM_T, swap_model(model, false))
	}
}

CsInternalModel:swap_model(CsInternalModel:model, bool:to_ct)
{
	switch (model)
	{
		case CS_T_TERROR:   return CS_CT_URBAN
		case CS_T_LEET:     return CS_CT_GSG9
		case CS_T_ARCTIC:   return CS_CT_SAS
		case CS_T_GUERILLA: return CS_CT_GIGN
		case CS_CT_URBAN:   return CS_T_TERROR
		case CS_CT_GSG9:    return CS_T_LEET
		case CS_CT_SAS:     return CS_T_ARCTIC
		case CS_CT_GIGN:    return CS_T_GUERILLA
		case CZ_T_MILITIA:  return CZ_CT_SPETSNAZ
		case CZ_CT_SPETSNAZ: return CZ_T_MILITIA
	}
	return to_ct ? CS_CT_URBAN : CS_T_TERROR
}

send_scores()
{
	message_begin(MSG_ALL, g_MsgTeamScore)
	write_string("CT")
	write_short(side_score(CS_TEAM_CT))
	message_end()
	message_begin(MSG_ALL, g_MsgTeamScore)
	write_string("TERRORIST")
	write_short(side_score(CS_TEAM_T))
	message_end()
}

// The scoreboard shows the match score
public msg_teamscore(msgid, dest, id)
{
	if (g_Phase != PH_LIVE && g_Phase != PH_OVER)
		return PLUGIN_CONTINUE
	new team[16]
	get_msg_arg_string(1, team, charsmax(team))
	if (team[0] == 'C')
		set_msg_arg_int(2, get_msg_argtype(2), side_score(CS_TEAM_CT))
	else if (team[0] == 'T')
		set_msg_arg_int(2, get_msg_argtype(2), side_score(CS_TEAM_T))
	return PLUGIN_CONTINUE
}

public ev_newround()
{
	g_InFreeze = true
	g_RoundOver = false
	g_IgnoreEnd = false
	if (g_Phase == PH_LIVE)
	{
		add_roster()
		if (g_AnnounceLive)
		{
			g_AnnounceLive = false
			set_hudmessage(80, 220, 120, -1.0, 0.3, 0, 0.0, 5.0, 0.2, 0.5, -1)
			ShowSyncHudMsg(0, g_Hud, "LIVE^nGood luck, have fun")
			msg(0, "^4LIVE.^1 Good luck, have fun.")
		}
		new line[32]
		score_line(line, charsmax(line))
		new round = g_Rounds + 1
		if (g_InOt)
			msg(0, "Overtime %d, round %d of 6. %s", g_OtBlock + 1, g_OtRound + 1, line)
		else
			msg(0, "Round %d of %d. %s", round, g_MaxRounds, line)
		send_scores()
		if (g_PausePending)
			set_task(0.3, "task_pause_now")
	}
}

public ev_round_start()
{
	g_InFreeze = false
}

public ev_twin()
{
	round_won(CS_TEAM_T)
}

public ev_ctwin()
{
	round_won(CS_TEAM_CT)
}

public ev_draw()
{
	round_won(CS_TEAM_UNASSIGNED)
}

round_won(CsTeams:side)
{
	if (g_IgnoreEnd || g_RoundOver)
		return
	g_RoundOver = true
	if (g_Phase == PH_KNIFE)
	{
		if (side == CS_TEAM_T || side == CS_TEAM_CT)
			knife_won(side)
		else
			msg(0, "A draw. The knife round goes again.")
		return
	}
	if (g_Phase != PH_LIVE)
		return
	g_Rounds++
	if (side == CS_TEAM_T || side == CS_TEAM_CT)
		g_Score[team_of(side)]++
	send_scores()
	new line[32]
	score_line(line, charsmax(line))
	if (side == CS_TEAM_UNASSIGNED)
		msg(0, "Round drawn. %s", line)
	else
		msg(0, "%s win the round. %s", side == CS_TEAM_CT ? "CTs" : "Terrorists", line)
	check_progress()
}

// After each live round: half-time, overtime or the end
check_progress()
{
	new a = g_Score[0], b = g_Score[1], half = g_MaxRounds / 2
	if (!g_InOt)
	{
		if (a > half || b > half)
			return finish()
		if (g_Rounds == half)
			return halftime()
		if (g_Rounds >= g_MaxRounds)
		{
			if (a != b || !g_OtOn)
				return finish()
			start_overtime_block()
		}
		return 0
	}
	g_OtRound++
	new da = a - g_OtBase[0], db = b - g_OtBase[1]
	if (da > OT_HALF || db > OT_HALF)
		return finish()
	if (g_OtRound == OT_HALF)
		return halftime()
	if (g_OtRound >= OT_HALF * 2)
	{
		if (da != db)
			return finish()
		g_OtBlock++
		start_overtime_block()
	}
	return 0
}

start_overtime_block()
{
	g_InOt = true
	g_OtRound = 0
	g_OtBase[0] = g_Score[0]
	g_OtBase[1] = g_Score[1]
	msg(0, "Tied. Overtime %d: first to 4 of the next 6 rounds, $%d each.", g_OtBlock + 1, OT_MONEY)
	set_task(3.0, "task_ot_start", TASK_NEXT)
}

public task_ot_start()
{
	if (g_Phase != PH_LIVE)
		return
	set_ot_money()
	g_IgnoreEnd = true
	server_cmd("sv_restart 1")
}

halftime()
{
	new line[32]
	score_line(line, charsmax(line))
	msg(0, "Half-time. %s. Teams switch sides.", line)
	set_hudmessage(255, 255, 255, -1.0, 0.3, 0, 0.0, 4.0, 0.2, 0.5, -1)
	ShowSyncHudMsg(0, g_Hud, "Half-time^n%s", line)
	set_task(3.0, "task_halftime", TASK_NEXT)
	return 0
}

public task_halftime()
{
	if (g_Phase != PH_LIVE)
		return
	swap_teams()
	g_Side0 = g_Side0 == CS_TEAM_CT ? CS_TEAM_T : CS_TEAM_CT
	g_Half++
	g_IgnoreEnd = true
	server_cmd("sv_restart 1")
	send_scores()
}

// Who played for each team (anyone on it at the start of a live round)
add_roster()
{
	new name[32]
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_hltv(p) || !on_team(p))
			continue
		new t = team_of(cs_get_user_team(p))
		get_user_name(p, name, charsmax(name))
		new bool:known = false
		for (new i = 0; i < g_RosterN[t]; i++)
			if (equal(g_Roster[t][i], name))
				known = true
		if (!known && g_RosterN[t] < MAX_ROSTER)
			copy(g_Roster[t][g_RosterN[t]++], charsmax(g_Roster[][]), name)
	}
}

team_label(t, out[], len)
{
	new CsTeams:side = t == 0 ? g_Side0 : (g_Side0 == CS_TEAM_CT ? CS_TEAM_T : CS_TEAM_CT)
	new c = captain_of(side)
	if (c)
	{
		new name[32]
		get_user_name(c, name, charsmax(name))
		formatex(out, len, "Team %s", name)
	}
	else
		formatex(out, len, "Team %d", t + 1)
}

finish()
{
	set_phase(PH_OVER)
	remove_task(TASK_NEXT)
	new l0[40], l1[40]
	team_label(0, l0, charsmax(l0))
	team_label(1, l1, charsmax(l1))
	new a = g_Score[0], b = g_Score[1]
	set_hudmessage(255, 220, 80, -1.0, 0.28, 0, 0.0, 10.0, 0.2, 0.5, -1)
	if (a == b)
	{
		msg(0, "Match over: a draw, %d - %d.", a, b)
		ShowSyncHudMsg(0, g_Hud, "Match over^nDraw, %d - %d", a, b)
	}
	else
	{
		msg(0, "Match over: %s wins %d - %d.", a > b ? l0 : l1, max(a, b), min(a, b))
		ShowSyncHudMsg(0, g_Hud, "Match over^n%s wins %d - %d", a > b ? l0 : l1, max(a, b), min(a, b))
	}
	write_result(l0, l1)
	write_status()
	msg(0, "Back to warmup in 20 seconds.")
	set_task(20.0, "task_after_match", TASK_NEXT)
	return 0
}

public task_after_match()
{
	if (g_Phase == PH_OVER)
		enter_warmup(true)
}

// ---------------------------------------------------------------------------
// Admin: restart and stop
// ---------------------------------------------------------------------------

public cmd_restart(id)
{
	if (!is_admin(id))
	{
		msg(id, "Only an admin can restart the match.")
		return PLUGIN_HANDLED
	}
	new name[32]
	get_user_name(id, name, charsmax(name))
	admin_restart(name)
	return PLUGIN_HANDLED
}

admin_restart(const who[])
{
	if (g_Paused)
		do_unpause(who)
	remove_task(TASK_NEXT)
	if (g_Phase == PH_KNIFE || g_Phase == PH_VOTE)
	{
		remove_task(TASK_VOTE)
		close_vote()
		msg(0, "%s restarted the knife round.", who)
		start_knife()
	}
	else if (g_Phase == PH_LIVE || g_Phase == PH_OVER)
	{
		msg(0, "%s restarted the match from 0 - 0.", who)
		go_live(false)
	}
	else
		msg(0, "No match yet: type ^4/start^1 to begin.")
}

public cmd_warmup(id)
{
	if (!is_admin(id))
	{
		msg(id, "Only an admin can stop the match.")
		return PLUGIN_HANDLED
	}
	new name[32]
	get_user_name(id, name, charsmax(name))
	admin_warmup(name)
	return PLUGIN_HANDLED
}

admin_warmup(const who[])
{
	if (g_Phase == PH_WARMUP)
		return
	if (g_Paused)
		do_unpause(who)
	msg(0, "%s stopped the match.", who)
	enter_warmup(true)
}

// ---------------------------------------------------------------------------
// Pauses
// ---------------------------------------------------------------------------

public cmd_pause(id)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	if (is_admin(id))
	{
		request_pause(id, -1, name)
		return PLUGIN_HANDLED
	}
	new CsTeams:side = cs_get_user_team(id)
	if (side != CS_TEAM_T && side != CS_TEAM_CT)
		return PLUGIN_HANDLED
	if (captain_of(side) != id)
	{
		new cap = captain_of(side), cname[32]
		get_user_name(cap, cname, charsmax(cname))
		msg(id, "Only your captain (%s) or an admin can pause.", cname)
		return PLUGIN_HANDLED
	}
	request_pause(id, team_of(side), name)
	return PLUGIN_HANDLED
}

// team: 0/1 for a captain's tactical pause, -1 for an admin
request_pause(id, team, const who[])
{
	if (g_Phase != PH_LIVE)
	{
		if (id) msg(id, "Pauses are for live matches.")
		else server_print("[Match] Pauses are for live matches.")
		return
	}
	if (g_Paused || g_PausePending)
	{
		if (id) msg(id, "A pause is already on or coming.")
		return
	}
	if (team >= 0)
	{
		if (g_PausesLeft[team] <= 0)
		{
			msg(id, "Your team has no pauses left.")
			return
		}
		g_PausesLeft[team]--
	}
	g_PauseTeam = team
	copy(g_PauseBy, charsmax(g_PauseBy), who)
	if (g_InFreeze)
		pause_now()
	else
	{
		g_PausePending = true
		msg(0, "%s called a pause. It starts at the next freeze time.", who)
	}
}

public task_pause_now()
{
	if (g_PausePending && g_Phase == PH_LIVE)
		pause_now()
}

pause_now()
{
	g_PausePending = false
	g_Paused = true
	g_PausedAt = get_systime()
	new secs = max(10, get_pcvar_num(g_pPauseTime))
	if (g_PauseTeam >= 0)
		msg(0, "Paused by %s (%d left for their team). They can type ^4/unpause^1; anyone can after %d seconds.", g_PauseBy, g_PausesLeft[g_PauseTeam], secs)
	else
		msg(0, "Paused by %s. An admin types ^4/unpause^1 to go on.", g_PauseBy)
	write_status()
	set_cvar_num("pausable", 1)
	// setpause/unpause don't toggle, so this stays in step with the game's own pause.
	// A dedicated server takes it from a player, a listen server from the host.
	server_cmd("setpause")
	new p = any_human()
	if (p)
		client_cmd(p, "setpause")
}

any_human()
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_human(p))
			return p
	return 0
}

public cmd_unpause(id)
{
	if (!g_Paused)
	{
		if (g_PausePending && (is_admin(id) || (on_team(id) && team_of(cs_get_user_team(id)) == g_PauseTeam && captain_of(cs_get_user_team(id)) == id)))
		{
			g_PausePending = false
			if (g_PauseTeam >= 0)
				g_PausesLeft[g_PauseTeam]++
			msg(0, "Pause called off.")
		}
		return PLUGIN_HANDLED
	}
	new name[32]
	get_user_name(id, name, charsmax(name))
	new bool:mine = on_team(id) && team_of(cs_get_user_team(id)) == g_PauseTeam && captain_of(cs_get_user_team(id)) == id
	new waited = get_systime() - g_PausedAt
	new limit = max(10, get_pcvar_num(g_pPauseTime))
	if (is_admin(id) || mine || (g_PauseTeam >= 0 && waited >= limit))
		do_unpause(name)
	else if (g_PauseTeam >= 0)
		msg(id, "The pause has %d seconds left. Their captain or an admin can end it sooner.", limit - waited)
	else
		msg(id, "An admin paused the game; an admin ends it.")
	return PLUGIN_HANDLED
}

do_unpause(const who[])
{
	if (!g_Paused)
	{
		g_PausePending = false
		return
	}
	g_Paused = false
	msg(0, "%s ended the pause. Back to the game.", who)
	server_cmd("unpause")
	new p = any_human()
	if (p)
		client_cmd(p, "unpause")
}

// ---------------------------------------------------------------------------
// Players: respawn in warmup, knives in the knife round
// ---------------------------------------------------------------------------

public client_disconnected(id)
{
	g_Ready[id] = false
	g_ReadyOrder[id] = 0
	g_Vote[id] = 0
	remove_task(id + TASK_RESPAWN)
	remove_task(id + TASK_STRIP)
}

public fw_spawn_post(id)
{
	if (!is_user_alive(id))
		return
	if (g_Phase == PH_WARMUP)
		cs_set_user_money(id, max(get_cvar_num("mp_maxmoney"), 16000), 0)
	else if (g_Phase == PH_KNIFE || g_Phase == PH_VOTE)
		set_task(0.2, "task_strip", id + TASK_STRIP)
}

public task_strip(taskid)
{
	new id = taskid - TASK_STRIP
	if (!is_user_alive(id) || (g_Phase != PH_KNIFE && g_Phase != PH_VOTE))
		return
	strip_user_weapons(id)
	give_item(id, "weapon_knife")
	cs_set_user_money(id, 0, 0)
}

public fw_touch_weapon(ent, toucher)
{
	if ((g_Phase == PH_KNIFE || g_Phase == PH_VOTE) && toucher >= 1 && toucher <= MaxClients)
		return HAM_SUPERCEDE
	return HAM_IGNORED
}

public fw_killed_post(victim, attacker, gib)
{
	if (g_Phase != PH_WARMUP)
		return
	// the dropped guns' own think frees them (deleting the box leaks edicts)
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "weaponbox")) > 0)
		if (pev(ent, pev_owner) == victim)
			set_pev(ent, pev_nextthink, get_gametime() + 0.1)
	queue_respawn(victim, 2.0)
}

// Someone who picks a team in warmup shouldn't wait for a round that never ends
public ev_teaminfo()
{
	new id = read_data(1)
	if (g_Phase == PH_WARMUP && is_user_connected(id) && !is_user_alive(id))
		queue_respawn(id, 1.0)
}

queue_respawn(id, Float:delay)
{
	remove_task(id + TASK_RESPAWN)
	set_task(delay, "task_respawn", id + TASK_RESPAWN)
}

public task_respawn(taskid)
{
	new id = taskid - TASK_RESPAWN
	if (g_Phase != PH_WARMUP || !is_user_connected(id) || is_user_alive(id) || !on_team(id))
		return
	ExecuteHamB(Ham_CS_RoundRespawn, id)
}

// ---------------------------------------------------------------------------
// HUD and chat
// ---------------------------------------------------------------------------

public task_hud()
{
	if (g_Phase == PH_WARMUP)
	{
		new notready[160], name[32], n = 0
		for (new p = 1; p <= MaxClients; p++)
		{
			if (!is_human(p) || !on_team(p) || g_Ready[p])
				continue
			get_user_name(p, name, charsmax(name))
			if (n < 5)
				format(notready, charsmax(notready), "%s%s%s", notready, n ? ", " : "", name)
			n++
		}
		if (n > 5)
			format(notready, charsmax(notready), "%s and %d more", notready, n - 5)
		set_hudmessage(120, 200, 255, 0.02, 0.2, 0, 0.0, 1.1, 0.0, 0.0, -1)
		ShowSyncHudMsg(0, g_Hud, "Warmup: %d/%d ready^nType /ready%s%s", ready_count(), needed(), n ? "^nWaiting for: " : "", notready)
	}
	else if (g_Phase == PH_VOTE)
	{
		set_hudmessage(255, 220, 80, -1.0, 0.2, 0, 0.0, 1.1, 0.0, 0.0, -1)
		ShowSyncHudMsg(0, g_Hud, "%s won the knife round^nThey're picking a side", g_VoteSide == CS_TEAM_CT ? "The CTs" : "The Terrorists")
	}
}

public cmd_score(id)
{
	if (g_Phase == PH_LIVE || g_Phase == PH_OVER)
	{
		new line[32]
		score_line(line, charsmax(line))
		msg(id, "%s (%d of %d rounds played%s).", line, g_Rounds, g_MaxRounds, g_InOt ? ", overtime" : "")
	}
	else if (g_Phase == PH_WARMUP)
		msg(id, "Warmup: %d/%d ready.", ready_count(), needed())
	else
		msg(id, "Knife round.")
	return PLUGIN_HANDLED
}

public cmd_help(id)
{
	msg(id, "/ready, /notready, /score. Captains: /pause, /unpause. Admins: /start, /restart, /warmup.")
	return PLUGIN_HANDLED
}

// ---------------------------------------------------------------------------
// Files for the app
// ---------------------------------------------------------------------------

json_escape(const src[], out[], len)
{
	new j
	for (new i = 0; src[i] && j < len - 2; i++)
	{
		new c = src[i]
		if (c == '"' || c == '\')
		{
			out[j++] = '\'
			out[j++] = c
		}
		else if (c >= 32)
			out[j++] = c
	}
	out[j] = 0
}

write_names(f, t)
{
	new esc[72]
	for (new i = 0; i < g_RosterN[t]; i++)
	{
		json_escape(g_Roster[t][i], esc, charsmax(esc))
		fprintf(f, "%s^"%s^"", i ? "," : "", esc)
	}
}

// matches.json: one finished match per line
write_result(const l0[], const l1[])
{
	add_roster()
	new f = fopen(fmt("%s/matches.json", g_DataDir), "at")
	if (!f)
		return
	new map[32], date[24], e0[80], e1[80]
	get_mapname(map, charsmax(map))
	get_time("%Y-%m-%d %H:%M", date, charsmax(date))
	json_escape(l0, e0, charsmax(e0))
	json_escape(l1, e1, charsmax(e1))
	new a = g_Score[0], b = g_Score[1]
	fprintf(f, "{^"date^":^"%s^",^"ts^":%d,^"map^":^"%s^",^"maxrounds^":%d,^"rounds^":%d,^"overtime^":%d,^"winner^":%d,^"teams^":[",
		date, get_systime(), map, g_MaxRounds, g_Rounds, g_InOt, a > b ? 1 : (b > a ? 2 : 0))
	fprintf(f, "{^"name^":^"%s^",^"score^":%d,^"players^":[", e0, a)
	write_names(f, 0)
	fprintf(f, "]},{^"name^":^"%s^",^"score^":%d,^"players^":[", e1, b)
	write_names(f, 1)
	fprintf(f, "]}]}^n")
	fclose(f)
}

public srv_status()
{
	write_status()
	server_print("Scrim status written")
	return PLUGIN_HANDLED
}

write_status()
{
	new f = fopen(fmt("%s/scrim_status.json", g_DataDir), "wt")
	if (!f)
		return
	new map[32], esc[72], name[32]
	get_mapname(map, charsmax(map))
	new live = g_Phase == PH_LIVE || g_Phase == PH_OVER
	new left = g_Paused ? max(0, max(10, get_pcvar_num(g_pPauseTime)) - (get_systime() - g_PausedAt)) : 0
	fprintf(f, "{^"phase^":^"%s^",^"map^":^"%s^",^"ready^":%d,^"needed^":%d,^"ct^":%d,^"t^":%d,^"round^":%d,^"maxrounds^":%d,^"half^":%d,^"overtime^":%d,^"paused^":%d,^"pause_pending^":%d,^"pause_left^":%d,^"vote_side^":^"%s^",^"players^":[",
		PHASE_NAME[g_Phase], map, ready_count(), needed(), live ? side_score(CS_TEAM_CT) : 0, live ? side_score(CS_TEAM_T) : 0,
		live ? g_Rounds + (g_Phase == PH_LIVE && !g_RoundOver ? 1 : 0) : 0, live ? g_MaxRounds : get_pcvar_num(g_pMaxRounds),
		live ? g_Half : 0, g_InOt, g_Paused, g_PausePending, left,
		g_Phase == PH_VOTE ? (g_VoteSide == CS_TEAM_CT ? "CT" : "T") : "")
	new first = true, capct = captain_of(CS_TEAM_CT), capt = captain_of(CS_TEAM_T)
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_hltv(p))
			continue
		new CsTeams:t = cs_get_user_team(p)
		get_user_name(p, name, charsmax(name))
		json_escape(name, esc, charsmax(esc))
		fprintf(f, "%s{^"name^":^"%s^",^"team^":^"%s^",^"bot^":%d,^"ready^":%d,^"captain^":%d}", first ? "" : ",", esc,
			t == CS_TEAM_CT ? "CT" : (t == CS_TEAM_T ? "T" : "SPEC"), is_user_bot(p), is_user_bot(p) || g_Ready[p], p == capct || p == capt)
		first = false
	}
	fprintf(f, "]}^n")
	fclose(f)
}
