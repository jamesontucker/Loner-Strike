/*
 * gm_battleroyale.sma - Battle Royale (GAMEMODES-PLAN.md 3.1), mode id "br".
 *
 * Everyone for themselves (or in squads of br_squad), one round per game. Players spawn spread
 * out at the map's ls_respawn_points with only a knife (br_start), loot crates and floor
 * weapons, and fight while the safe zone (ls_safe_zone + ls_zone_shrinker) closes in, with the
 * round clock counting to the next change. Supply drops fall during the game. The last player
 * or squad standing wins. The first br_drop seconds nobody can hurt anyone ("loot up").
 *
 * Needs gm_components.amxx (listed first in br.plugins.ini) and a map pack
 * (configs/gamemodes/components/<map>.br.ini: shipped for de_aztec, cs_italy, de_dust2,
 * de_inferno, de_train); other maps get a zone round the middle of their spawns.
 *
 * Drop-in (br_dropin 1): everyone starts as high as the map's sky allows (up to br_drop_height
 * above a landing spot), falls at 0.2 gravity and steers, lands with normal gravity and no fall
 * damage. Bots start over a spot their habit picks (looters a good crate, campers the edge).
 * Stock maps have low skies (de_aztec: ~500 units of fall), so the drop is short there.
 * Times scale with the number of players (24 = as set; fewer = quicker zone and drops).
 *
 * Bots: free-for-all through mp_freeforall; squad mates are friends to the brain
 * (ls_mode role + friend, both ways). gm_components tells them about loot (attract) and the
 * zone (ls_mode zone). This plugin adds a few personal goals, never one for everyone: looters
 * go for a nearby full crate inside the zone now and then, campers settle somewhere inside the
 * next circle each stage, hunters get nothing (the brain hunts and third-parties by noise).
 * Every goal has an end time, so stale ones go.
 *
 * Built on gm_components' pieces (docs/COMPONENTS.md, "Battle Royale pieces"), the same ones Game
 * Mode Studio offers: squads and the squad lobby (ls_squad_*), the drop-in (ls_drop_in), healing
 * items, armor levels, scarce ammo, rarity glow and death crates (switched on in br.cfg), and the
 * match HUD (ls_match_hud: players left, kills, zone, kill feed, watching your killer). This plugin
 * keeps the game's flow (lobby, drop, loot-up, fight, winner) and where its bots land and loot.
 */

#include <amxmodx>
#include <amxmisc>
#include <fakemeta>
#include <hamsandwich>
#include <engine>
#include <cstrike>
#include <fun>
#define LS_OWN_PLUGIN_NATIVES
#include <loner>

/* gm_people (optional): who a bot is; its playstyle picks its Battle Royale habit */
native people_get(id, const key[]);

#pragma semicolon 1

#define VERSION "0.1"
#define TASK_TICK  7738100
#define TASK_SETUP 7738200
#define TASK_GEAR  7738300

enum { PH_IDLE = 0, PH_WAIT, PH_LOBBY, PH_DROP, PH_PLAY, PH_OVER };
enum { TR_HUNTER = 0, TR_LOOTER, TR_CAMPER };

new g_cSquad, g_cStart, g_cDrop, g_cZoneStart, g_cDrops, g_cWaitHuman, g_cDropIn, g_cDropHeight, g_cLobby, g_cLateFill, g_cBleed;
new g_knocker[33];
new bool:g_contactLogged;

new Float:g_dropSpots[33][2];
new g_ndropSpots;
#define BELOW_Z -250.0
new g_lootTarget[33];
enum { LAYER_BELOW = 0, LAYER_SURFACE, LAYER_HIGH };
new g_layer[33] = { -1, ... };
new Float:g_medianZ;
new g_crateHeaded[2048];
new g_looterN;
new g_lootupGuns, g_gunGoals;
new g_gunGoal[33];
new g_secsAll, g_secsBelow;
new bool:g_wentBelow[33];
new bool:g_gunTaken[2048];
new g_usedDrops[16];
new g_nusedDrops;

new g_phase;
new Float:g_phaseEnd;
new Float:g_playAt;
new bool:g_zoneStarted;
new g_dropsDone;
new Float:g_dropAt[8];
new g_ndrops;
new g_trait[33];
new Float:g_nextGoal[33];
new g_lastStage;
new g_mapname[64];
new Float:g_pace = 1.0;
new Float:g_startPos[33][3];
new g_lateMate[33];
new g_knocks, g_revives;
new g_unstuckTries[33];
new Float:g_releaseAt[33];
new g_stagesOrig[256];

public plugin_natives()
{
	set_module_filter("ls__module_filter");
	set_native_filter("br_native_filter");
}

public br_native_filter(const name[], index, trap)
{
	if (equal(name, "people_", 7))
		return PLUGIN_HANDLED;
	return ls__native_filter(name, index, trap);
}

public plugin_init()
{
	ls_register_mode("br", "Battle Royale", VERSION, "Loner-Strike");

	g_cSquad = register_cvar("br_squad", "1");
	g_cStart = register_cvar("br_start", "knife");
	g_cDrop = register_cvar("br_drop", "15");
	g_cZoneStart = register_cvar("br_zone_start", "30");
	g_cDrops = register_cvar("br_drops", "3");
	g_cWaitHuman = register_cvar("br_wait_human", "1");
	g_cDropIn = register_cvar("br_dropin", "1");
	g_cDropHeight = register_cvar("br_drop_height", "3000");
	/* squads: seconds of lobby before the game (teaming up with /squad); 0 none */
	g_cLobby = register_cvar("br_lobby", "30");
	/* squads: someone who joins during the first N s of fighting fills a squad that's short (else watches) */
	g_cLateFill = register_cvar("br_late_fill", "120");
	/* squads: a deadly hit knocks you down instead; a squad mate holding USE gets you up; you bleed out
	   after this many seconds (45: time for a squad mate to reach you), and you're out when your whole squad is down
	   (0: no knocking down) */
	g_cBleed = register_cvar("br_bleed", "45");

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");
	RegisterHamPlayer(Ham_Spawn, "ham_spawn", 1);
	RegisterHamPlayer(Ham_Killed, "ham_killed", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_takedamage");

	register_clcmd("say /br", "cmd_help");
	register_clcmd("say /squads", "cmd_squads");
	register_srvcmd("br_goals", "cmd_goals");
	/* no shopping in Battle Royale: what you find is what you get */
	new const buycmds[][] = { "buy", "buyequip", "autobuy", "rebuy", "client_buy_open", "cl_autobuy", "cl_rebuy", "cl_setautobuy", "cl_setrebuy", "buyammo1", "buyammo2" };
	for (new i = 0; i < sizeof buycmds; i++)
		register_clcmd(buycmds[i], "cmd_nobuy");
	get_mapname(g_mapname, charsmax(g_mapname));
	set_task(1.0, "tick", TASK_TICK, _, _, "b");
}

/* the next mode may sell things: the brain's "nothing to buy" goes with this one */
public plugin_end()
{
	if (cvar_exists("ls_mode_ver"))
		server_cmd("ls_mode nobuy 0");
	if (ls_components_running())
		ls_down_enable(false);
}

/* big announcements on gm_components' toast line: one at a time, never on top of each other */
stock toast(id, Float:secs, const fmt[], any:...)
{
	new msg[128];
	vformat(msg, charsmax(msg), fmt, 4);
	ls_toast(id, secs, msg);
}

/* server: each bot's loot-up target and how far it is (for tests) */
public cmd_goals()
{
	new Float:o[3], Float:t[3], ln[12];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		pev(p, pev_origin, o);
		layer_name(p, ln, charsmax(ln));
		new e = g_gunGoal[p];
		if (e && pev_valid(e))
		{
			pev(e, pev_origin, t);
			server_print("bot %d (%s) at %.0f %.0f %.0f falling %d armed %d -> gun %d (state %d) %.0f away, dz %.0f", p, ln, o[0], o[1], o[2], ls_dropping(p), has_gun(p), e, ls_comp_state(e), get_distance_f(o, t), o[2] - t[2]);
		}
		else
			server_print("bot %d (%s) at %.0f %.0f %.0f falling %d armed %d, no gun goal", p, ln, o[0], o[1], o[2], ls_dropping(p), has_gun(p));
	}
	return PLUGIN_HANDLED;
}

stock layer_name(p, out[], len)
{
	static const NAMES[][] = { "below", "surface", "high", "-" };
	new l = g_layer[p];
	copy(out, len, NAMES[(l >= 0 && l <= 2) ? l : 3]);
}

public cmd_nobuy(id)
{
	return PLUGIN_HANDLED;
}

public cmd_help(id)
{
	ls_chat(id, "Battle Royale: loot crates (hold USE) and floor weapons, stay inside the zone, be the last one standing.");
	ls_chat(id, "F or say /heal: healing items. USE on a crate where someone fell: what they had. Squads: say /squad.");
	return PLUGIN_HANDLED;
}

public cmd_squads(id)
{
	ls_squad_menu(id);
	return PLUGIN_HANDLED;
}

/* =========================================================================
 * A game = a round
 * ========================================================================= */

public ev_new_round()
{
	g_phase = PH_IDLE;
	g_zoneStarted = false;
	g_dropsDone = 0;
	g_lastStage = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		g_nextGoal[p] = 0.0;
		g_startPos[p][0] = 0.0;
		g_startPos[p][1] = 0.0;
		g_unstuckTries[p] = 0;
		g_releaseAt[p] = 0.0;
	}
	if (ls_components_running())
	{
		ls_comp_stop(0);
		ls_drop_reset();
	}
}

public ev_round_start()
{
	remove_task(TASK_SETUP);
	set_task(0.5, "game_setup", TASK_SETUP);
}

public game_setup()
{
	if (!ls_components_running())
	{
		log_amx("Battle Royale needs gm_components.amxx; add it to br.plugins.ini before gm_battleroyale.amxx.");
		return;
	}
	ensure_zone();
	ls_comp_bots_hint("set br");
	ls_comp_bots_hint("goal_clear all 0");
	ls_comp_bots_hint("friend all all 0");
	ls_comp_bots_hint("phase off");
	/* nothing to buy here: bots don't wait in the buy state */
	ls_comp_bots_hint("nobuy 1");
	/* the pieces: squads of br_squad, the match HUD (heals, armor levels, scarce ammo, glow and
	   death crates are switched on in br.cfg) */
	ls_squad_max(clamp(get_pcvar_num(g_cSquad), 1, 8));
	for (new p = 1; p <= MaxClients; p++)
		ls_squad_set(p, 0);
	ls_match_hud(true);
	ls_match_hud_text("");
	/* squads: knocked down first (gm_components' downed state), out when the whole squad is down */
	if (get_pcvar_num(g_cSquad) > 1 && get_pcvar_num(g_cBleed) > 0)
		ls_down_enable(true, get_pcvar_float(g_cBleed), 0, false);
	else
		ls_down_enable(false);
	/* a knocked-down player can be finished by the other squads (ls_down_hp health), as in PUBG */
	set_cvar_num("ls_down_finish", get_pcvar_num(g_cSquad) > 1 ? 1 : 0);
	/* (no money for revives here; a Last Stand map before may have left it on) */
	set_cvar_num("ls_down_revive_pay", 0);
	pick_traits();
	if (get_pcvar_num(g_cWaitHuman) && !humans())
	{
		g_phase = PH_WAIT;
		return;
	}
	start_lobby();
}

/* squads: a lobby to team up first (say /squad), then the squads are filled and the drop starts */
stock start_lobby()
{
	new secs = get_pcvar_num(g_cLobby);
	if (get_pcvar_num(g_cSquad) > 1 && secs > 0)
	{
		g_phase = PH_LOBBY;
		ls_squad_lobby(float(secs));
		log_amx("Battle Royale on %s: squad lobby, %d s", g_mapname, secs);
		return;
	}
	ls_squad_fill();
	start_drop();
}

public ls_on_lobby_end(squads)
{
	if (g_phase != PH_LOBBY)
		return;
	log_amx("Battle Royale: lobby over, %d squads", squads);
	start_drop();
}

/* fewer players: a quicker game (the zone closes sooner and faster), so a 12-bot game still
   ends with the zone doing its job; 24 players play the times as set, 32 a little longer */
stock Float:pace()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			n++;
	return floatclamp(float(n) / 24.0, 0.5, 1.25);
}

/* the shrinker's stages with every warn/shrink time times pace() (from the map's own each game) */
stock pace_zone(Float:f)
{
	new sh = ls_comp_find("ls_zone_shrinker");
	if (!sh)
		return;
	if (!g_stagesOrig[0])
		ls_comp_get(sh, "stages", g_stagesOrig, charsmax(g_stagesOrig));
	if (!g_stagesOrig[0])
		return;
	new out[256], tok[48], rest[256], a[4][12];
	copy(rest, charsmax(rest), g_stagesOrig);
	while (rest[0])
	{
		strtok2(rest, tok, charsmax(tok), rest, charsmax(rest), ' ', TRIM_FULL);
		if (!tok[0])
			break;
		replace_all(tok, charsmax(tok), ":", " ");
		parse(tok, a[0], 11, a[1], 11, a[2], 11, a[3], 11);
		format(out, charsmax(out), "%s%s%s:%d:%d:%s", out, out[0] ? " " : "", a[0],
			max(floatround(str_to_float(a[1]) * f), 5), max(floatround(str_to_float(a[2]) * f), 5), a[3][0] ? a[3] : "1");
	}
	ls_comp_set(sh, "stages", out);
}

stock start_drop()
{
	g_phase = PH_DROP;
	g_pace = pace();
	/* a big map (zone radius 4000+): it takes a while to get anywhere, so the zone takes longer
	   (20 bots on br_valley: about 5 min of game time at pace 0.75-1.0, the zone doing the
	   last 5 kills; 1.5 aims at 7-8 min) */
	new zone = ls_comp_find("ls_safe_zone");
	if (zone && ls_comp_getf(zone, "radius") >= 4000.0)
		g_pace = floatmax(g_pace, 2.0);
	pace_zone(g_pace);
	/* small games: a little more in each crate */
	new extra[4];
	num_to_str(g_pace < 0.75 ? 3 : 2, extra, charsmax(extra));
	for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
		if (ls_comp_getf(e, "tier", 1.0) < 4.0)
			ls_comp_set(e, "items", extra);
	/* everyone's kit now (spawns at round start came before the game was set up) */
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p))
			give_start(p);
	g_ndropSpots = 0;
	g_nusedDrops = 0;
	assign_layers();
	g_looterN = 0;
	g_lootupGuns = 0;
	arrayset(g_gunGoal, 0, sizeof g_gunGoal);
	g_gunGoals = 0;
	g_secsAll = 0;
	g_secsBelow = 0;
	arrayset(g_wentBelow, false, sizeof g_wentBelow);
	arrayset(g_gunTaken, false, sizeof g_gunTaken);
	if (get_pcvar_num(g_cDropIn))
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_alive(p))
				br_drop_in(p);
	/* while nobody can be hurt, bots loot and keep apart instead of chasing each other (else
	   they start the fight in knife range: half a 20-bot lobby went in the first 15 s on br_valley) */
	ls_comp_bots_hint("phase loot");
	new drop = floatround(get_pcvar_float(g_cDrop) * floatclamp(0.6 + g_pace * 0.4, 0.7, 1.1));
	/* dropping in takes a while (a slow fall from up to br_drop_height): the loot-up starts
	   counting for real once people are down, so add the fall (they landed with 3 s left before) */
	if (get_pcvar_num(g_cDropIn))
		drop += floatround(floatclamp(get_pcvar_float(g_cDropHeight) / 250.0, 4.0, 14.0));
	g_phaseEnd = get_gametime() + float(drop);
	/* anyone who couldn't drop in (no sky over their spot) goes for a gun now */
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && !ls_dropping(p))
			loot_goal_on_landing(p);
	toast(0, 5.0, "Battle Royale: loot up! Fighting starts in %d seconds", drop);
	log_amx("Battle Royale on %s: game starts, %d players, pace %.2f, loot-up %d s", g_mapname, alive_count(), g_pace, drop);
	if (get_pcvar_num(g_cSquad) > 1)
		ls_chat(0, "Squads of %d: your squad mates are on your side (say /br for help).", get_pcvar_num(g_cSquad));
}

stock start_play()
{
	g_phase = PH_PLAY;
	g_playAt = get_gametime();
	g_contactLogged = false;
	g_knocks = 0;
	g_revives = 0;
	ls_comp_bots_hint("phase fight");
	/* how close people are when the fighting starts (the peace phase should keep them apart) */
	new close = 0, near = 0;
	for (new a = 1; a <= MaxClients; a++)
		for (new b = a + 1; b <= MaxClients; b++)
			if (is_user_alive(a) && is_user_alive(b))
			{
				new Float:d = entity_range(a, b);
				if (d < 100.0)
					close++;
				else if (d < 350.0)
					near++;
			}
	new armed = 0, below = 0, high = 0, alive = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		alive++;
		if (has_gun(p))
			armed++;
		new Float:o[3];
		pev(p, pev_origin, o);
		if (o[2] < BELOW_Z)
			below++;
		else if (o[2] > g_medianZ + 200.0)
			high++;
	}
	log_amx("Battle Royale: fight starts, %d pairs within 100 u, %d more within 350 u; %d of %d armed (%d guns picked up in the loot-up, %d sent for one), %d below ground, %d high, %d on the surface", close, near, armed, alive, g_lootupGuns, g_gunGoals, below, high, alive - below - high);
	toast(0, 3.0, "Fight! %d left", ls_squads_alive());
	/* supply drops spread over the game */
	g_ndrops = min(get_pcvar_num(g_cDrops), sizeof g_dropAt);
	for (new i = 0; i < g_ndrops; i++)
		g_dropAt[i] = g_playAt + (60.0 + 90.0 * float(i)) * g_pace + random_float(-10.0, 10.0);
}

/* a map without a pack: a zone round the middle of the spawns, shrinking in five stages */
stock ensure_zone()
{
	if (ls_comp_count("ls_safe_zone"))
		return;
	new Float:sum[3], Float:mn[3] = { 99999.0, 99999.0, 0.0 }, Float:mx[3] = { -99999.0, -99999.0, 0.0 }, n = 0, Float:o[3], e = -1;
	new const cls[][] = { "info_player_start", "info_player_deathmatch" };
	for (new c = 0; c < 2; c++)
	{
		e = -1;
		while ((e = find_ent_by_class(e, cls[c])) > 0)
		{
			pev(e, pev_origin, o);
			for (new i = 0; i < 3; i++)
				sum[i] += o[i];
			for (new i = 0; i < 2; i++)
			{
				mn[i] = floatmin(mn[i], o[i]);
				mx[i] = floatmax(mx[i], o[i]);
			}
			n++;
		}
	}
	if (!n)
		return;
	for (new i = 0; i < 3; i++)
		sum[i] /= float(n);
	new Float:r = floatmax(mx[0] - mn[0], mx[1] - mn[1]) * 0.75 + 600.0;
	new keys[192];
	formatex(keys, charsmax(keys), "radius=%.0f damage=1 targetname=brzone", r);
	ls_comp_create("ls_safe_zone", sum, keys);
	formatex(keys, charsmax(keys), "zone=brzone start_delay=-1 stages=^"%.0f:45:40:1 %.0f:40:35:2 %.0f:35:30:4 %.0f:30:25:8 0:20:20:15^"",
		r * 0.7, r * 0.45, r * 0.25, r * 0.1);
	ls_comp_create("ls_zone_shrinker", sum, keys);
	log_amx("Battle Royale: no map pack for %s, so a zone goes round the spawns (radius %.0f).", g_mapname, r);
}

stock humans()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			n++;
	return n;
}

stock alive_count()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p))
			n++;
	return n;
}

/* =========================================================================
 * Each second
 * ========================================================================= */

public tick()
{
	if (g_phase == PH_IDLE || g_phase == PH_OVER || !ls_components_running())
		return;
	new Float:now = get_gametime();
	if (g_phase == PH_WAIT)
	{
		if (humans())
		{
			/* someone joined: a fresh game where everyone starts spread out */
			g_phase = PH_OVER;
			server_cmd("endround");
			return;
		}
		hud_line("Battle Royale starts when a player joins");
		return;
	}
	if (g_phase == PH_LOBBY)
		return;
	if (g_phase == PH_DROP)
	{
		/* people who join while looting up still play (in a squad that's short, or their own) */
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && !is_user_bot(p) && !is_user_alive(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			{
				if (!ls_squad_get(p))
					ls_squad_fill();
				ExecuteHamB(Ham_CS_RoundRespawn, p);
			}
		if (now >= g_phaseEnd)
			start_play();
	}
	if (g_phase == PH_PLAY)
	{
		if (!g_zoneStarted && now - g_playAt >= get_pcvar_float(g_cZoneStart) * g_pace)
		{
			g_zoneStarted = true;
			new sh = ls_comp_find("ls_zone_shrinker");
			if (sh)
				ls_comp_start(sh);
		}
		if (g_dropsDone < g_ndrops && now >= g_dropAt[g_dropsDone])
		{
			g_dropsDone++;
			supply_drop();
		}
		squads_none_up();
		if (check_winner())
			return;
		late_joiners();
	}
	bot_goals();
	unstick_bots();
	/* the gun a bot was walking to went to someone else: the next nearest (loot-up and the
	   first half minute of the fight) */
	if (g_phase == PH_DROP || (g_phase == PH_PLAY && get_gametime() - g_playAt < 30.0))
		for (new p = 1; p <= MaxClients; p++)
		{
			new e = g_gunGoal[p];
			if (e && is_user_alive(p) && is_user_bot(p) && !ls_dropping(p) && pev_valid(e) && ls_comp_state(e) != 0 && !has_gun(p))
			{
				g_gunGoal[p] = 0;
				loot_goal_on_landing(p);
			}
		}
	/* (for tests: how much of the time people spend below ground, and how many went at all) */
	if (g_phase == PH_PLAY)
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_alive(p))
			{
				new Float:o[3];
				pev(p, pev_origin, o);
				g_secsAll++;
				if (o[2] < BELOW_Z)
				{
					g_secsBelow++;
					g_wentBelow[p] = true;
				}
			}
	hud();
}

/* Some bots stay in the brain's buy state at their spawn when there's nothing to buy (a request
   for W2 in docs/notes/W2.md). A person-style order takes them out of it: a bot that hasn't
   moved 8 s into the game walks to a nearby crate (or respawn point), then is let go */
stock unstick_bots()
{
	/* (only once the fighting is on: during the loot-up a bot opening a crate stands still on
	   purpose, and the brain's nobuy has taken care of the buy state since) */
	if (g_phase != PH_PLAY)
		return;
	new Float:now = get_gametime(), Float:o[3];
	new Float:since = now - g_playAt;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		if (g_releaseAt[p] > 0.0 && now >= g_releaseAt[p])
		{
			g_releaseAt[p] = 0.0;
			server_cmd("ls_bot %d release", p);
		}
		/* (campers stand still on purpose) */
		if (ls_dropping(p) || g_unstuckTries[p] >= 3 || g_trait[p] == TR_CAMPER)
			continue;
		pev(p, pev_origin, o);
		if (g_startPos[p][0] == 0.0 && g_startPos[p][1] == 0.0)
		{
			g_startPos[p] = o;
			continue;
		}
		new Float:moved = floatsqroot((o[0] - g_startPos[p][0]) * (o[0] - g_startPos[p][0]) + (o[1] - g_startPos[p][1]) * (o[1] - g_startPos[p][1]));
		if (moved > 64.0)
		{
			g_unstuckTries[p] = 3;
			continue;
		}
		if (since < 12.0 + 8.0 * float(g_unstuckTries[p]))
			continue;
		new Float:to[3];
		if (!nearby_target(o, to))
			continue;
		g_unstuckTries[p]++;
		g_releaseAt[p] = now + 12.0;
		server_cmd("ls_bot %d goto %.0f %.0f %.0f", p, to[0], to[1], to[2]);
		log_amx("Battle Royale: bot %d hadn't moved (try %d); sent to %.0f %.0f %.0f", p, g_unstuckTries[p], to[0], to[1], to[2]);
	}
}

/* a full crate within 1500 units, else a respawn point that isn't this spot */
stock bool:nearby_target(const Float:o[3], Float:out[3])
{
	new best = 0, Float:bd = 1500.0, Float:e[3];
	for (new c = ls_comp_find("ls_loot_container"); c; c = ls_comp_find("ls_loot_container", c))
	{
		if (ls_comp_state(c) != 0)
			continue;
		pev(c, pev_origin, e);
		new Float:d = get_distance_f(o, e);
		if (d > 100.0 && d < bd)
		{
			bd = d;
			best = c;
		}
	}
	if (!best)
		for (new r = ls_comp_find("ls_respawn_point"); r; r = ls_comp_find("ls_respawn_point", r))
		{
			pev(r, pev_origin, e);
			new Float:d = get_distance_f(o, e);
			if (d > 200.0 && d < 3000.0)
			{
				best = r;
				break;
			}
		}
	if (!best)
		return false;
	pev(best, pev_origin, out);
	return true;
}

stock bool:check_winner()
{
	new last;
	new sides = ls_squads_alive(last);
	if (sides > 1)
		return false;
	g_phase = PH_OVER;
	new went = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (g_wentBelow[p])
			went++;
	if (sides == 0 || !last)
	{
		toast(0, 6.0, "Nobody survived");
		/* (the game's figures even with no winner - QA4-DE F5) */
		log_amx("Battle Royale on %s: nobody survived, after %d s of fighting; %d players went below ground, %d%% of the time there; %d knocked down, %d revived", g_mapname, floatround(get_gametime() - g_playAt), went, g_secsAll ? 100 * g_secsBelow / g_secsAll : 0, g_knocks, g_revives);
		server_cmd("endround");
		return true;
	}
	new name[32];
	get_user_name(last, name, charsmax(name));
	if (get_pcvar_num(g_cSquad) > 1)
		toast(0, 8.0, "%s's squad wins!", name);
	else
		toast(0, 8.0, "%s wins! (%d kills)", name, ls_kills(last));
	log_amx("Battle Royale on %s won by %s (%d kills) after %d s of fighting; %d players went below ground, %d%% of the time there; %d knocked down, %d revived", g_mapname, name, ls_kills(last), floatround(get_gametime() - g_playAt), went, g_secsAll ? 100 * g_secsBelow / g_secsAll : 0, g_knocks, g_revives);
	server_cmd(cs_get_user_team(last) == CS_TEAM_T ? "endround T" : "endround CT");
	return true;
}

/* squads: someone who joins in the first br_late_fill s of fighting fills a squad that's short, next
   to a squad mate who's alive; anyone else watches until the next game */
stock late_joiners()
{
	if (get_pcvar_num(g_cSquad) <= 1 || get_gametime() - g_playAt > get_pcvar_float(g_cLateFill))
		return;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || is_user_alive(p) || ls_squad_get(p)
			|| (cs_get_user_team(p) != CS_TEAM_T && cs_get_user_team(p) != CS_TEAM_CT))
			continue;
		new sq = ls_squad_short();
		if (!sq)
			continue;
		new mate = 0;
		for (new q = 1; q <= MaxClients && !mate; q++)
			if (q != p && is_user_alive(q) && ls_squad_get(q) == sq)
				mate = q;
		if (!mate)
			continue;
		ls_squad_set(p, sq);
		cs_set_user_team(p, (sq % 2) ? CS_TEAM_T : CS_TEAM_CT);
		ExecuteHamB(Ham_CS_RoundRespawn, p);
		if (!is_user_alive(p))
			continue;
		/* next to the mate once gm_components has placed the spawn (0.1 s) */
		g_lateMate[p] = mate;
		set_task(0.5, "task_late_place", TASK_GEAR + 96 + p);
		new name[32];
		get_user_name(mate, name, charsmax(name));
		toast(p, 5.0, "You're in %s's squad: they need you!", name);
		log_amx("Battle Royale: player %d joined late into squad %d", p, sq);
	}
}

public task_late_place(tid)
{
	new p = tid - TASK_GEAR - 96, mate = g_lateMate[p];
	g_lateMate[p] = 0;
	if (!is_user_alive(p) || !mate || !is_user_alive(mate))
		return;
	new Float:o[3];
	pev(mate, pev_origin, o);
	o[2] += 80.0;
	engfunc(EngFunc_SetOrigin, p, o);
	set_pev(p, pev_velocity, Float:{0.0, 0.0, 0.0});
}

stock supply_drop()
{
	new markers[16], n = 0;
	for (new e = ls_comp_find("ls_supply_drop"); e && n < sizeof markers; e = ls_comp_find("ls_supply_drop", e))
	{
		/* not one in water (a crate in the river can't be reached or opened) */
		new Float:w[3];
		pev(e, pev_origin, w);
		w[2] += 16.0;
		if (engfunc(EngFunc_PointContents, w) == CONTENTS_WATER)
			continue;
		markers[n++] = e;
	}
	/* prefer spots inside the next circle */
	new Float:c[3], Float:r, closing;
	ls_zone_next(0, c, r, closing);
	new pick = 0;
	for (new tries = 0; tries < 12 && n; tries++)
	{
		new e = markers[random(n)];
		/* not a spot that already had a drop this game (a second crate on an opened one) */
		new bool:used = false;
		for (new u = 0; u < g_nusedDrops; u++)
			if (g_usedDrops[u] == e)
				used = true;
		if (used && tries < 11)
			continue;
		new Float:o[3];
		pev(e, pev_origin, o);
		pick = e;
		if (r <= 0.0 || floatsqroot((o[0] - c[0]) * (o[0] - c[0]) + (o[1] - c[1]) * (o[1] - c[1])) < r)
			break;
	}
	if (pick)
	{
		if (g_nusedDrops < sizeof g_usedDrops)
			g_usedDrops[g_nusedDrops++] = pick;
		ls_comp_start(pick);
	}
	else if (r > 0.0)
		ls_drop_call(c, "drop");
}

/* =========================================================================
 * Players
 * ========================================================================= */

public ham_spawn(id)
{
	if (!is_user_alive(id) || g_phase == PH_IDLE || g_phase == PH_OVER)
		return HAM_IGNORED;
	remove_task(TASK_GEAR + id);
	set_task(0.3, "task_gear", TASK_GEAR + id);
	return HAM_IGNORED;
}

/* everyone starts with only br_start (after gm_components moved them to a respawn point) */
public task_gear(tid)
{
	new id = tid - TASK_GEAR;
	if (!is_user_alive(id))
		return;
	give_start(id);
	if (g_phase == PH_DROP && get_pcvar_num(g_cDropIn))
		br_drop_in(id);
}

stock give_start(id)
{
	strip_user_weapons(id);
	new items[128], w[32], pos = 0;
	get_pcvar_string(g_cStart, items, charsmax(items));
	while (items[pos])
	{
		new i = 0;
		/* "knife glock18", "knife+glock18" or "knife,glock18" */
		while (items[pos] == ' ' || items[pos] == '+' || items[pos] == ',')
			pos++;
		while (items[pos] && items[pos] != ' ' && items[pos] != '+' && items[pos] != ',' && i < charsmax(w))
			w[i++] = items[pos++];
		w[i] = EOS;
		if (w[0])
			ls_give_item(id, w);
	}
	cs_set_user_money(id, 0);
	cs_set_user_armor(id, 0, CS_ARMOR_NONE);
}

/* =========================================================================
 * The drop-in: start high over a landing spot, fall slowly and steer, land
 * ========================================================================= */

/* where this player comes down: bots by habit (looters over a good crate, campers out at the
   edge, hunters anywhere), people over a spread-out respawn point they steer away from */
/* ---- layers: below ground, the surface, high places (roofs, walkways). Each bot gets one for
   the game (a share of each: about 3 in 10 below, 1 in 4 high, the rest the surface; looters
   lean below, campers high), told to the brain (ls_mode layer <i> <below|surface|high>) so its
   own loot choice can weigh it, and used for where it lands ---- */
stock layer_split(secs)
{
	new below = 0, high = 0, alive = 0, hb = 0, hu = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		alive++;
		new Float:o[3];
		pev(p, pev_origin, o);
		new bool:up = o[2] > g_medianZ + 200.0;
		if (o[2] < BELOW_Z)
			below++;
		else if (up)
			high++;
		if (g_layer[p] == LAYER_HIGH)
		{
			hb++;
			if (up)
				hu++;
		}
	}
	log_amx("Battle Royale: first contact at %d s: %d below ground, %d high, %d on the surface (%d of %d meant for high are up)", secs, below, high, alive - below - high, hu, hb);
}

stock crate_layer(e)
{
	new Float:o[3];
	pev(e, pev_origin, o);
	if (o[2] < BELOW_Z)
		return LAYER_BELOW;
	if (o[2] > g_medianZ + 200.0)
		return LAYER_HIGH;
	return LAYER_SURFACE;
}

stock compute_median_z()
{
	new Float:zs[160], n = 0;
	for (new e = ls_comp_find("ls_loot_container"); e && n < sizeof zs; e = ls_comp_find("ls_loot_container", e))
	{
		new Float:o[3];
		pev(e, pev_origin, o);
		if (o[2] >= BELOW_Z)
			zs[n++] = o[2];
	}
	/* (a simple selection: the middle value) */
	for (new i = 0; i < n; i++)
		for (new j = i + 1; j < n; j++)
			if (zs[j] < zs[i])
			{
				new Float:t = zs[i];
				zs[i] = zs[j];
				zs[j] = t;
			}
	g_medianZ = n ? zs[n / 2] : 0.0;
}

stock assign_layers()
{
	compute_median_z();
	new bots[32], n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		g_layer[p] = -1;
		if (is_user_alive(p) && is_user_bot(p))
			bots[n++] = p;
	}
	/* shuffle, then looters first for the places below, campers first for high */
	for (new i = n - 1; i > 0; i--)
	{
		new j = random(i + 1), t = bots[i];
		bots[i] = bots[j];
		bots[j] = t;
	}
	new below = (n * 3 + 5) / 10, high = (n + 2) / 4;
	for (new pass = 0; pass < 2; pass++)
		for (new i = 0; i < n; i++)
		{
			new p = bots[i];
			if (g_layer[p] >= 0)
				continue;
			if (below > 0 && (pass == 1 || g_trait[p] == TR_LOOTER))
			{
				g_layer[p] = LAYER_BELOW;
				below--;
			}
			else if (high > 0 && (pass == 1 || g_trait[p] == TR_CAMPER))
			{
				g_layer[p] = LAYER_HIGH;
				high--;
			}
		}
	new line[48];
	static const NAMES[][] = { "below", "surface", "high" };
	for (new i = 0; i < n; i++)
	{
		new p = bots[i];
		if (g_layer[p] < 0)
			g_layer[p] = LAYER_SURFACE;
		formatex(line, charsmax(line), "layer %d %s", p, NAMES[g_layer[p]]);
		ls_comp_bots_hint(line);
	}
	arrayset(g_crateHeaded, 0, sizeof g_crateHeaded);
}

stock marker_layer(e)
{
	new v[16];
	if (!ls_marker_get(e, "layer", v, charsmax(v)))
		return -1;
	if (equali(v, "below"))
		return LAYER_BELOW;
	if (equali(v, "high"))
		return LAYER_HIGH;
	if (equali(v, "surface"))
		return LAYER_SURFACE;
	return -1;
}

/* a full crate on the bot's layer, few others headed near it */
stock crowd_pick_crate(id)
{
	new best = 0, Float:bs = -999999.0, Float:o[3], Float:q[3];
	for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
	{
		if (ls_comp_state(e) != 0 || crate_layer(e) != g_layer[id])
			continue;
		pev(e, pev_origin, o);
		new Float:sc = random_float(0.0, 400.0) + 100.0 * ls_comp_getf(e, "tier", 1.0);
		for (new k = ls_comp_find("ls_loot_container"); k; k = ls_comp_find("ls_loot_container", k))
		{
			if (k >= sizeof g_crateHeaded || !g_crateHeaded[k])
				continue;
			pev(k, pev_origin, q);
			new Float:d = get_distance_f(o, q);
			if (d < 1200.0)
				sc -= 800.0 * float(g_crateHeaded[k]) * (1.0 - d / 1200.0);
		}
		if (sc > bs)
		{
			bs = sc;
			best = e;
		}
	}
	if (best && best < sizeof g_crateHeaded)
		g_crateHeaded[best]++;
	return best;
}

/* room to fall from 250 units above a spot */
stock bool:open_above(const Float:p[3])
{
	new Float:a[3], Float:b[3], Float:hit[3];
	a = p;
	a[2] += 40.0;
	b = a;
	b[2] += 300.0;
	engfunc(EngFunc_TraceHull, a, b, IGNORE_MONSTERS, HULL_HUMAN, 0, 0);
	get_tr2(0, TR_vecEndPos, hit);
	return !get_tr2(0, TR_StartSolid) && hit[2] - a[2] >= 250.0;
}

/* the open landing spot (br_drop marker, else respawn point) nearest to a point */
stock bool:nearest_landing(const Float:p[3], Float:out[3], layer = -1)
{
	/* (gm_components' landing search: br_drop markers, layer-tagged ones first, crowded spots further) */
	static const NAMES[][] = { "below", "surface", "high" };
	return ls_drop_spot(p, out, layer >= 0 ? NAMES[layer] : "");
}

/* landed: an unarmed bot goes for the nearest gun on the floor (else a crate); a looter with a
   crate in mind goes for it (below ground too: the nav takes it down the stairs and hatches) */
stock loot_goal_on_landing(p)
{
	if (!is_user_bot(p) || !is_user_alive(p))
		return;
	new Float:po[3], line[128];
	pev(p, pev_origin, po);
	new left = max(floatround(g_phaseEnd - get_gametime()), 0) + 20;
	/* (a bot meant for below ground heads straight down: there are guns down there too) */
	if (!has_gun(p) && !((g_layer[p] == LAYER_BELOW || g_layer[p] == LAYER_HIGH) && g_lootTarget[p]))
	{
		new best = 0, Float:bd = 1600.0, Float:o[3];
		for (new e = ls_comp_find("ls_weapon_spawner"); e; e = ls_comp_find("ls_weapon_spawner", e))
		{
			if (ls_comp_state(e) != 0)
				continue;
			pev(e, pev_origin, o);
			/* (same layer first: a gun right below the floor is a long way round) */
			new Float:d = get_distance_f(o, po) + 3.0 * floatabs(o[2] - po[2]);
			/* (one another bot is already after counts a bit further) */
			if (g_gunTaken[e < sizeof g_gunTaken ? e : 0])
				d += 1500.0;
			/* (a bot meant for the high ground takes only guns up there: W2 saw "high" bots spend
			   the loot-up fetching ground guns, 1 of 16 up at first contact) */
			if (g_layer[p] == LAYER_HIGH && o[2] <= g_medianZ + 200.0)
				continue;
			if (d < bd)
			{
				bd = d;
				best = e;
			}
		}
		if (!best)
			for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
			{
				if (ls_comp_state(e) != 0 || (g_layer[p] == LAYER_HIGH && crate_layer(e) != LAYER_HIGH))
					continue;
				pev(e, pev_origin, o);
				new Float:d = get_distance_f(o, po);
				if (d < bd)
				{
					bd = d;
					best = e;
				}
			}
		if (best)
		{
			if (best < sizeof g_gunTaken)
				g_gunTaken[best] = true;
			g_gunGoal[p] = best;
			pev(best, pev_origin, o);
			formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 65 %d 16", p, o[0], o[1], o[2], best, left);
			g_gunGoals++;
			/* and a person-style order to walk there: the loot-up's "give the others room" moves
			   otherwise win over a goal (let go when the gun is taken or the loot-up ends) */
			server_cmd("ls_bot %d goto %.0f %.0f %.0f", p, o[0], o[1], o[2]);
			g_releaseAt[p] = g_phaseEnd;
			ls_comp_bots_hint(line);
			g_nextGoal[p] = get_gametime() + float(left);
			return;
		}
	}
	new t = g_lootTarget[p];
	/* (the brain picks loot by itself, layer and crowding counted; only a bot meant for below
	   ground is walked to its crate, the trenches and hatches being far) */
	if (t && pev_valid(t) && ls_comp_state(t) == 0 && (g_layer[p] == LAYER_BELOW || g_layer[p] == LAYER_HIGH))
	{
		new Float:o[3];
		pev(t, pev_origin, o);
		formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 55 %d 64", p, o[0], o[1], o[2], t, left + 60);
		ls_comp_bots_hint(line);
		/* below ground or far: walk there as ordered through the loot-up, the goal after */
		server_cmd("ls_bot %d goto %.0f %.0f %.0f", p, o[0], o[1], o[2]);
		g_releaseAt[p] = g_phaseEnd;
		g_nextGoal[p] = get_gametime() + float(left + 30);
	}
}

public ls_on_weapon_taken(ent, id, const item[])
{
	if (g_phase == PH_DROP)
		g_lootupGuns++;
	/* got one: the walk-there order is done */
	if (1 <= id <= MaxClients && is_user_bot(id) && g_releaseAt[id] > 0.0)
	{
		g_releaseAt[id] = 0.0;
		server_cmd("ls_bot %d release", id);
	}
	return PLUGIN_CONTINUE;
}

stock bool:has_gun(p)
{
	new weapons[32], n;
	get_user_weapons(p, weapons, n);
	for (new i = 0; i < n; i++)
		if (weapons[i] != CSW_KNIFE && weapons[i] != CSW_HEGRENADE && weapons[i] != CSW_FLASHBANG && weapons[i] != CSW_SMOKEGRENADE && weapons[i] != CSW_C4)
			return true;
	return false;
}

stock landing_spot(id, Float:out[3])
{
	new Float:c[3], Float:r, closing;
	ls_zone_next(0, c, r, closing);
	g_lootTarget[id] = 0;
	/* bots: a crate on their own layer that few others are headed to (crowding), landing on it
	   if it's open to the sky, else at the nearest open landing spot to walk from */
	if (is_user_bot(id) && g_trait[id] != TR_CAMPER && g_layer[id] >= 0)
	{
		new crate = crowd_pick_crate(id);
		if (crate)
		{
			g_lootTarget[id] = crate;
			pev(crate, pev_origin, out);
			/* a high crate open to the sky: land right on it (br_valley's drop spots tagged high are
			   hilltops under 100 u, below what counts as high, 200 over the crates' median) */
			if (g_layer[id] == LAYER_HIGH && open_above(out))
			{
				out[0] += random_float(-40.0, 40.0);
				out[1] += random_float(-40.0, 40.0);
				return 1;
			}
			if (g_layer[id] != LAYER_SURFACE || !open_above(out))
			{
				new Float:o[3];
				o = out;
				if (nearest_landing(o, out, g_layer[id]))
					return 1;
			}
			out[0] += random_float(-120.0, 120.0);
			out[1] += random_float(-120.0, 120.0);
			return 1;
		}
	}
	/* meant for the high ground with no crate up there: land on a high spot all the same */
	if (is_user_bot(id) && g_layer[id] == LAYER_HIGH)
	{
		new Float:o[3];
		o = c;
		o[0] += random_float(-1500.0, 1500.0);
		o[1] += random_float(-1500.0, 1500.0);
		if (nearest_landing(o, out, LAYER_HIGH))
			return 1;
	}
	if (is_user_bot(id) && g_trait[id] == TR_LOOTER)
	{
		/* looters spread over the layers: about every third goes for a crate below ground
		   (the map's lower level), the rest for one up top */
		new bool:below = (g_looterN++ % 3) == 0;
		new cands[128], n = 0;
		for (new pass = 0; pass < 2 && !n; pass++)
			for (new e = ls_comp_find("ls_loot_container"); e && n < sizeof cands; e = ls_comp_find("ls_loot_container", e))
			{
				if (ls_comp_state(e) != 0 || ls_comp_getf(e, "tier", 1.0) < 2.0)
					continue;
				new Float:o[3];
				pev(e, pev_origin, o);
				/* (second pass: any layer) */
				if (pass == 0 && (o[2] < BELOW_Z) != below)
					continue;
				cands[n++] = e;
			}
		if (n)
		{
			new crate = spread_pick(cands, n);
			g_lootTarget[id] = crate;
			pev(crate, pev_origin, out);
			/* a crate under a roof or below ground: nobody can fall onto it, so the drop is to the
			   nearest landing spot in the open and the bot walks the rest (stairs, hatches) */
			if (!open_above(out))
			{
				new Float:o[3];
				o = out;
				if (nearest_landing(o, out))
					return 1;
			}
			out[0] += random_float(-120.0, 120.0);
			out[1] += random_float(-120.0, 120.0);
			return 1;
		}
	}
	new pts[64], n = 0;
	/* a map kit's landing spots (ls_marker kind=br_drop), else the respawn points */
	for (new e = ls_marker_find("br_drop"); e && n < sizeof pts; e = ls_marker_find("br_drop", e))
		pts[n++] = e;
	if (!n)
		for (new e = ls_comp_find("ls_respawn_point"); e && n < sizeof pts; e = ls_comp_find("ls_respawn_point", e))
			pts[n++] = e;
	if (!n)
	{
		pev(id, pev_origin, out);
		return 0;
	}
	new pick = spread_pick(pts, n);
	if (is_user_bot(id) && g_trait[id] == TR_CAMPER && r > 0.0)
	{
		/* the respawn point furthest from the middle */
		new Float:bd = -1.0;
		for (new i = 0; i < n; i++)
		{
			new Float:o[3];
			pev(pts[i], pev_origin, o);
			new Float:d = floatsqroot((o[0] - c[0]) * (o[0] - c[0]) + (o[1] - c[1]) * (o[1] - c[1])) + random_float(0.0, 400.0);
			/* (not where another camper already drops: the far corner drew them all) */
			for (new u = 0; u < g_ndropSpots; u++)
				if (floatabs(o[0] - g_dropSpots[u][0]) < 600.0 && floatabs(o[1] - g_dropSpots[u][1]) < 600.0)
					d -= 2500.0;
			if (d > bd)
			{
				bd = d;
				pick = pts[i];
			}
		}
	}
	else if (!is_user_bot(id))
		pev(id, pev_origin, out);
	if (is_user_bot(id) || !n)
		pev(pick, pev_origin, out);
	return 1;
}

/* of a few random picks, the one furthest from where others already drop: a game doesn't start
   with half the lobby landing on one crate and knifing it out the moment the loot-up ends */
stock spread_pick(const ents[], n)
{
	new best = ents[random(n)], Float:bestd = -1.0, Float:o[3];
	for (new t = 0; t < 6; t++)
	{
		new e = ents[random(n)];
		pev(e, pev_origin, o);
		new Float:d = 99999.0;
		for (new i = 0; i < g_ndropSpots; i++)
			d = floatmin(d, floatsqroot((o[0] - g_dropSpots[i][0]) * (o[0] - g_dropSpots[i][0]) + (o[1] - g_dropSpots[i][1]) * (o[1] - g_dropSpots[i][1])));
		if (d > bestd)
		{
			bestd = d;
			best = e;
		}
	}
	return best;
}

/* drops a player in over the landing spot their habit picks (gm_components' ls_drop_in does the fall:
   slow, steered, no damage, ls_on_landed when down) */
stock br_drop_in(id)
{
	new Float:land[3];
	landing_spot(id, land);
	if (g_ndropSpots < sizeof g_dropSpots)
	{
		g_dropSpots[g_ndropSpots][0] = land[0];
		g_dropSpots[g_ndropSpots][1] = land[1];
		g_ndropSpots++;
	}
	ls_drop_in(id, land, get_pcvar_float(g_cDropHeight));
}

public ls_on_landed(id)
{
	if (g_phase == PH_DROP && is_user_alive(id))
		loot_goal_on_landing(id);
}

public ham_takedamage(victim, inflictor, attacker, Float:dmg, bits)
{
	/* (falling in, the squad lobby and squad mates: gm_components refuses those) */
	/* the loot-up: nothing hurts at all (a bot that landed on a high roof and walked off died) */
	if (g_phase == PH_DROP)
		return HAM_SUPERCEDE;
	if (!(1 <= attacker <= MaxClients) || attacker == victim || !is_user_connected(attacker))
		return HAM_IGNORED;
	/* the first hit between players: where everyone is then */
	if (g_phase == PH_PLAY && !g_contactLogged && dmg > 0.0)
	{
		g_contactLogged = true;
		layer_split(floatround(get_gametime() - g_playAt));
	}
	/* nobody hurts anybody while waiting for people */
	if (g_phase == PH_WAIT || g_phase == PH_LOBBY)
		return HAM_SUPERCEDE;
	return HAM_IGNORED;
}

public ham_killed(victim, killer, gib)
{
	if (g_phase == PH_DROP)
		log_amx("Battle Royale: player %d died during the drop-in/loot-up (falling %d)", victim, ls_dropping(victim));
	if (g_phase != PH_PLAY)
		return HAM_IGNORED;
	/* (kills, the kill feed and watching the killer: gm_components' match HUD) */
	if (get_pcvar_num(g_cSquad) > 1)
		set_task(4.5, "task_watch_squad", TASK_GEAR + 64 + victim);
	new place = alive_count() + 1;
	new wpn[24] = "world", Float:dist = 0.0;
	if (1 <= killer <= MaxClients && killer != victim && is_user_connected(killer))
	{
		get_weaponname(get_user_weapon(killer), wpn, charsmax(wpn));
		dist = entity_range(killer, victim);
	}
	log_amx("Battle Royale: player %d out at %d s (#%d), by %d (%s, %.0f u)", victim, floatround(get_gametime() - g_playAt), place, killer, wpn, dist);
	return HAM_IGNORED;
}

/* squads: a deadly hit knocks a player down while a squad mate is still up; the last one standing going
   down takes the whole squad out */
public ls_on_down(id, attacker)
{
	g_knocker[id] = attacker;
	if (g_phase != PH_PLAY || get_pcvar_num(g_cSquad) <= 1)
		return PLUGIN_HANDLED;
	new sq = ls_squad_get(id);
	new bool:up = false;
	for (new q = 1; q <= MaxClients && sq; q++)
		if (q != id && is_user_alive(q) && ls_squad_get(q) == sq && !ls_is_down(q))
			up = true;
	if (!sq || !up)
	{
		/* the squad's out: the others who are down go too */
		for (new q = 1; q <= MaxClients && sq; q++)
			if (q != id && is_user_alive(q) && ls_squad_get(q) == sq && ls_is_down(q))
				set_task(0.1, "task_squad_out", TASK_GEAR + 128 + q);
		return PLUGIN_HANDLED;
	}
	new vn[32], kn[32], line[96];
	get_user_name(id, vn, charsmax(vn));
	if (1 <= attacker <= MaxClients && attacker != id && is_user_connected(attacker))
	{
		get_user_name(attacker, kn, charsmax(kn));
		formatex(line, charsmax(line), "%s knocked down %s", kn, vn);
	}
	else
		formatex(line, charsmax(line), "%s is down", vn);
	ls_feed(line);
	g_knocks++;
	log_amx("Battle Royale: player %d knocked down at %d s by %d", id, floatround(get_gametime() - g_playAt), attacker);
	return PLUGIN_CONTINUE;
}

public ls_on_revive(id, by)
{
	if (g_phase == PH_PLAY)
	{
		g_revives++;
		log_amx("Battle Royale: player %d revived by %d", id, by);
	}
}

/* a squad with nobody standing (the last one up left the server, or died some other way): its
   downed players are out at once, the kill going to whoever knocked them (QA4-DE F4) */
stock squads_none_up()
{
	if (get_pcvar_num(g_cSquad) <= 1)
		return;
	for (new d = 1; d <= MaxClients; d++)
	{
		if (!is_user_alive(d) || !ls_is_down(d))
			continue;
		new sq = ls_squad_get(d);
		new bool:up = false;
		for (new q = 1; q <= MaxClients && sq && !up; q++)
			if (q != d && is_user_alive(q) && ls_squad_get(q) == sq && !ls_is_down(q))
				up = true;
		if (up)
			continue;
		new k = g_knocker[d];
		log_amx("Battle Royale: player %d out: nobody of squad %d is up", d, sq);
		if (1 <= k <= MaxClients && k != d && is_user_connected(k))
			ExecuteHamB(Ham_Killed, d, k, 0);
		else
			user_kill(d, 1);
	}
}

public task_squad_out(tid)
{
	new q = tid - TASK_GEAR - 128;
	if (is_user_alive(q) && ls_is_down(q))
		user_kill(q, 1);
}

/* out: watch a squad mate who's still in it */
public task_watch_squad(tid)
{
	new id = tid - TASK_GEAR - 64;
	if (!is_user_connected(id) || is_user_alive(id) || is_user_bot(id))
		return;
	new sq = ls_squad_get(id);
	for (new q = 1; q <= MaxClients && sq; q++)
	{
		if (q != id && is_user_alive(q) && ls_squad_get(q) == sq)
		{
			ls_spectate(id, q);
			new name[32];
			get_user_name(q, name, charsmax(name));
			ls_chat(id, "Watching %s from your squad.", name);
			return;
		}
	}
}

/* a supply drop: each person hears how far and which way */
public ls_on_drop(ent, stage, const Float:origin[3])
{
	if (stage != 0 || g_phase != PH_PLAY)
		return;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || is_user_bot(p))
			continue;
		new Float:o[3], Float:ang[3];
		pev(p, pev_origin, o);
		pev(p, pev_v_angle, ang);
		new Float:d = floatsqroot((origin[0] - o[0]) * (origin[0] - o[0]) + (origin[1] - o[1]) * (origin[1] - o[1]));
		new Float:yaw = floatatan2(origin[1] - o[1], origin[0] - o[0], degrees) - ang[1];
		while (yaw > 180.0) yaw -= 360.0;
		while (yaw < -180.0) yaw += 360.0;
		new dir[24];
		if (floatabs(yaw) < 30.0)
			copy(dir, charsmax(dir), "ahead of you");
		else if (floatabs(yaw) > 150.0)
			copy(dir, charsmax(dir), "behind you");
		else if (yaw > 0.0)
			copy(dir, charsmax(dir), "to your left");
		else
			copy(dir, charsmax(dir), "to your right");
		ls_chat(p, "Supply drop coming down %d m %s.", floatround(d * 0.0254), dir);
	}
}

/* =========================================================================
 * Bots: a few personal goals, varied, each with an end time
 * ========================================================================= */

/* the bot's playstyle from people.ini: greedy roamers loot, careful ones camp, rushers hunt */
stock persona_trait(p)
{
	new style = LibraryExists("gm_people", LibType_Library) ? people_get(p, "style") : -1;
	switch (style)
	{
		/* rusher, knifer, ninja */
		case 0, 6, 8: return TR_HUNTER;
		/* roamer, nader, objective, weirdgun */
		case 1, 7, 9, 10: return TR_LOOTER;
		/* lurker, careful, camper, awper */
		case 2, 3, 4, 5: return TR_CAMPER;
	}
	new r = random_num(1, 100);
	return r <= 35 ? TR_LOOTER : (r <= 60 ? TR_CAMPER : TR_HUNTER);
}

stock pick_traits()
{
	new n[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || !is_user_bot(p))
			continue;
		g_trait[p] = persona_trait(p);
		n[g_trait[p]]++;
		g_nextGoal[p] = get_gametime() + random_float(3.0, 12.0);
	}
	log_amx("Battle Royale bots: %d hunters, %d looters, %d campers (pace %.2f)", n[TR_HUNTER], n[TR_LOOTER], n[TR_CAMPER], pace());
}

stock bot_goals()
{
	new Float:now = get_gametime();
	new Float:c[3], Float:r, closing;
	new Float:secs = ls_zone_next(0, c, r, closing);
	/* a new stage: campers pick a spot inside the next circle */
	new stage = ls_zone_stage(ls_comp_find("ls_zone_shrinker"));
	new bool:newStage = stage != g_lastStage;
	g_lastStage = stage;
	new line[128];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		if (g_trait[p] == TR_CAMPER && newStage && r > 0.0 && stage > 0)
		{
			new Float:a = random_float(0.0, 360.0), Float:d = r * random_float(0.25, 0.65);
			new Float:o[3];
			o[0] = c[0] + d * floatcos(a, degrees);
			o[1] = c[1] + d * floatsin(a, degrees);
			o[2] = c[2];
			formatex(line, charsmax(line), "goal %d go_hold %.0f %.0f %.0f 0 45 %d 300", p, o[0], o[1], o[2], floatround(floatmax(secs, 30.0) + 20.0));
			ls_comp_bots_hint(line);
			continue;
		}
		/* (the loot-up and the first 90 s of the fight are the brain's own looting) */
		if (g_trait[p] != TR_LOOTER || now < g_nextGoal[p] || g_phase == PH_WAIT || g_phase == PH_DROP || now - g_playAt < 90.0)
			continue;
		/* (not a bot on the high ground: it holds sightline spots there, the brain's call) */
		if (g_layer[p] == LAYER_HIGH)
			continue;
		g_nextGoal[p] = now + random_float(15.0, 30.0);
		/* the nearest full crate that's inside the zone (and not too far) */
		new Float:po[3], best = 0, Float:bd = 1500.0;
		pev(p, pev_origin, po);
		for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
		{
			if (ls_comp_state(e) != 0)
				continue;
			new Float:o[3];
			pev(e, pev_origin, o);
			if (r > 0.0 && floatsqroot((o[0] - c[0]) * (o[0] - c[0]) + (o[1] - c[1]) * (o[1] - c[1])) > r)
				continue;
			new Float:d = get_distance_f(po, o);
			if (d < bd)
			{
				bd = d;
				best = e;
			}
		}
		if (!best)
			continue;
		new Float:o[3];
		pev(best, pev_origin, o);
		formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 40 60 80", p, o[0], o[1], o[2], best);
		ls_comp_bots_hint(line);
	}
}

/* =========================================================================
 * HUD
 * ========================================================================= */

stock hud_line(const text[])
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p))
			ls_hud(p, LS_HUD_TOP, 1.1, "%s", text);
}

stock hud()
{
	/* (players left, kills, zone, armor, heals, the kill feed and the squad list: gm_components'
	   match HUD; this adds the game's phase in place of the zone timer until the zone starts) */
	new Float:now = get_gametime();
	new text[96];
	if (g_phase == PH_DROP)
		formatex(text, charsmax(text), "Loot up: fighting in %d s", max(floatround(g_phaseEnd - now), 0));
	else if (g_phase == PH_PLAY && !g_zoneStarted)
		formatex(text, charsmax(text), "The zone starts closing in %d s", max(floatround(get_pcvar_float(g_cZoneStart) * g_pace - (now - g_playAt)), 0));
	ls_match_hud_text(text);
}
