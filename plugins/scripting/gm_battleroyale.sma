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

enum { PH_IDLE = 0, PH_WAIT, PH_DROP, PH_PLAY, PH_OVER };
enum { TR_HUNTER = 0, TR_LOOTER, TR_CAMPER };

new g_cSquad, g_cStart, g_cDrop, g_cZoneStart, g_cDrops, g_cWaitHuman, g_cDropIn, g_cDropHeight;

new Float:g_dropSpots[33][2];
new g_ndropSpots;
#define BELOW_Z -250.0
new g_lootTarget[33];
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
new g_squad[33];
new g_nsquads;
new g_kills[33];
new g_trait[33];
new Float:g_nextGoal[33];
new g_lastStage;
new g_mapname[64];
new Float:g_pace = 1.0;
new bool:g_falling[33];
new Float:g_fallAt[33];
new Float:g_startPos[33][3];
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
	register_forward(FM_PlayerPostThink, "fm_postthink", 1);

	register_event("HLTV", "ev_new_round", "a", "1=0", "2=0");
	register_logevent("ev_round_start", 2, "1=Round_Start");
	RegisterHamPlayer(Ham_Spawn, "ham_spawn", 1);
	RegisterHamPlayer(Ham_Killed, "ham_killed", 1);
	RegisterHamPlayer(Ham_TakeDamage, "ham_takedamage");

	register_clcmd("say /br", "cmd_help");
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
	new Float:o[3], Float:t[3];
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p) || !is_user_bot(p))
			continue;
		pev(p, pev_origin, o);
		new e = g_gunGoal[p];
		if (e && pev_valid(e))
		{
			pev(e, pev_origin, t);
			server_print("bot %d at %.0f %.0f %.0f falling %d armed %d -> gun %d (state %d) %.0f away, dz %.0f", p, o[0], o[1], o[2], g_falling[p], has_gun(p), e, ls_comp_state(e), get_distance_f(o, t), o[2] - t[2]);
		}
		else
			server_print("bot %d at %.0f %.0f %.0f falling %d armed %d, no gun goal", p, o[0], o[1], o[2], g_falling[p], has_gun(p));
	}
	return PLUGIN_HANDLED;
}

public cmd_nobuy(id)
{
	return PLUGIN_HANDLED;
}

public cmd_help(id)
{
	ls_chat(id, "Battle Royale: loot crates (hold USE) and floor weapons, stay inside the zone, be the last one standing.");
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
		g_kills[p] = 0;
		g_squad[p] = 0;
		g_nextGoal[p] = 0.0;
		g_startPos[p][0] = 0.0;
		g_startPos[p][1] = 0.0;
		g_unstuckTries[p] = 0;
		g_releaseAt[p] = 0.0;
		if (g_falling[p])
			land(p);
	}
	if (ls_components_running())
		ls_comp_stop(0);
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
	make_squads();
	pick_traits();
	if (get_pcvar_num(g_cWaitHuman) && !humans())
	{
		g_phase = PH_WAIT;
		return;
	}
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
		g_pace = floatmax(g_pace, 1.5);
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
				drop_in(p);
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
		if (is_user_alive(p) && !g_falling[p])
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
	new armed = 0, below = 0, alive = 0;
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
	}
	log_amx("Battle Royale: fight starts, %d pairs within 100 u, %d more within 350 u; %d of %d armed (%d guns picked up in the loot-up, %d sent for one), %d below ground", close, near, armed, alive, g_lootupGuns, g_gunGoals, below);
	toast(0, 3.0, "Fight! %d left", alive_sides());
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

/* squads: br_squad players each, kept on one CS team, friends to the brain */
stock make_squads()
{
	new size = clamp(get_pcvar_num(g_cSquad), 1, 8);
	new list[32], n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			list[n++] = p;
	/* shuffle, then deal */
	for (new i = n - 1; i > 0; i--)
	{
		new j = random(i + 1), t = list[i];
		list[i] = list[j];
		list[j] = t;
	}
	g_nsquads = 0;
	new line[64];
	for (new i = 0; i < n; i++)
	{
		new p = list[i];
		if (size == 1)
		{
			g_squad[p] = i + 1;
			continue;
		}
		new sq = i / size + 1;
		g_squad[p] = sq;
		g_nsquads = max(g_nsquads, sq);
		cs_set_user_team(p, (sq % 2) ? CS_TEAM_T : CS_TEAM_CT);
		formatex(line, charsmax(line), "role %d sq%d", p, sq);
		ls_comp_bots_hint(line);
	}
	squad_friends();
}

stock squad_friends()
{
	new line[64];
	for (new sq = 1; sq <= g_nsquads; sq++)
	{
		formatex(line, charsmax(line), "friend @sq%d @sq%d 1", sq, sq);
		ls_comp_bots_hint(line);
	}
}

stock humans()
{
	new n = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && !is_user_bot(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
			n++;
	return n;
}

/* how many sides still have someone alive (players, or squads) */
stock alive_sides(&last = 0)
{
	new seen[66], n = 0;
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_alive(p))
			continue;
		/* (someone who joined late has no squad: a side of their own) */
		new key = g_squad[p] ? min(g_squad[p], 32) : 33 + p;
		if (!seen[key])
		{
			seen[key] = 1;
			n++;
			last = p;
		}
	}
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
	if (g_phase == PH_DROP)
	{
		/* people who join while looting up still play */
		for (new p = 1; p <= MaxClients; p++)
			if (is_user_connected(p) && !is_user_bot(p) && !is_user_alive(p) && (cs_get_user_team(p) == CS_TEAM_T || cs_get_user_team(p) == CS_TEAM_CT))
				ExecuteHamB(Ham_CS_RoundRespawn, p);
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
		if (check_winner())
			return;
	}
	bot_goals();
	unstick_bots();
	/* the gun a bot was walking to went to someone else: the next nearest (loot-up and the
	   first half minute of the fight) */
	if (g_phase == PH_DROP || (g_phase == PH_PLAY && get_gametime() - g_playAt < 30.0))
		for (new p = 1; p <= MaxClients; p++)
		{
			new e = g_gunGoal[p];
			if (e && is_user_alive(p) && is_user_bot(p) && !g_falling[p] && pev_valid(e) && ls_comp_state(e) != 0 && !has_gun(p))
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
		if (g_falling[p] || g_unstuckTries[p] >= 3 || g_trait[p] == TR_CAMPER)
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
	new sides = alive_sides(last);
	if (sides > 1)
		return false;
	g_phase = PH_OVER;
	if (sides == 0 || !last)
	{
		toast(0, 6.0, "Nobody survived");
		server_cmd("endround");
		return true;
	}
	new name[32];
	get_user_name(last, name, charsmax(name));
	if (get_pcvar_num(g_cSquad) > 1)
		toast(0, 8.0, "%s's squad wins!", name);
	else
		toast(0, 8.0, "%s wins! (%d kills)", name, g_kills[last]);
	new went = 0;
	for (new p = 1; p <= MaxClients; p++)
		if (g_wentBelow[p])
			went++;
	log_amx("Battle Royale on %s won by %s (%d kills) after %d s of fighting; %d players went below ground, %d%% of the time there", g_mapname, name, g_kills[last], floatround(get_gametime() - g_playAt), went, g_secsAll ? 100 * g_secsBelow / g_secsAll : 0);
	server_cmd(cs_get_user_team(last) == CS_TEAM_T ? "endround T" : "endround CT");
	return true;
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
		drop_in(id);
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
stock bool:nearest_landing(const Float:p[3], Float:out[3])
{
	new best = 0, Float:bd = 999999.0, Float:o[3];
	for (new e = ls_marker_find("br_drop"); e; e = ls_marker_find("br_drop", e))
	{
		pev(e, pev_origin, o);
		new Float:d = get_distance_f(o, p);
		/* (one someone already drops on is as good as further away: no crowd at one spot) */
		for (new i = 0; i < g_ndropSpots; i++)
			if (floatabs(o[0] - g_dropSpots[i][0]) < 300.0 && floatabs(o[1] - g_dropSpots[i][1]) < 300.0)
				d += 1500.0;
		if (d < bd && open_above(o))
		{
			bd = d;
			best = e;
		}
	}
	if (!best)
		for (new e = ls_comp_find("ls_respawn_point"); e; e = ls_comp_find("ls_respawn_point", e))
		{
			pev(e, pev_origin, o);
			new Float:d = get_distance_f(o, p);
			if (d < bd && open_above(o))
			{
				bd = d;
				best = e;
			}
		}
	if (!best)
		return false;
	pev(best, pev_origin, out);
	return true;
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
	if (!has_gun(p))
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
			if (d < bd)
			{
				bd = d;
				best = e;
			}
		}
		if (!best)
			for (new e = ls_comp_find("ls_loot_container"); e; e = ls_comp_find("ls_loot_container", e))
			{
				if (ls_comp_state(e) != 0)
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
	if (t && pev_valid(t) && ls_comp_state(t) == 0)
	{
		new Float:o[3];
		pev(t, pev_origin, o);
		formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 55 %d 64", p, o[0], o[1], o[2], t, left + 30);
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

stock drop_in(id)
{
	new Float:land[3];
	landing_spot(id, land);
	/* nobody drops on top of somebody else (they'd hang in the air on each other) */
	for (new tries = 0; tries < 8; tries++)
	{
		new bool:clash = false;
		for (new i = 0; i < g_ndropSpots && !clash; i++)
			clash = floatabs(land[0] - g_dropSpots[i][0]) < 96.0 && floatabs(land[1] - g_dropSpots[i][1]) < 96.0;
		if (!clash)
			break;
		land[0] += random_float(-160.0, 160.0);
		land[1] += random_float(-160.0, 160.0);
	}
	if (g_ndropSpots < sizeof g_dropSpots)
	{
		g_dropSpots[g_ndropSpots][0] = land[0];
		g_dropSpots[g_ndropSpots][1] = land[1];
		g_ndropSpots++;
	}
	/* as high as the sky allows (or br_drop_height), with room for a player */
	new Float:a[3], Float:b[3], Float:hit[3];
	a = land;
	a[2] += 40.0;
	b = a;
	b[2] += get_pcvar_float(g_cDropHeight);
	engfunc(EngFunc_TraceHull, a, b, IGNORE_MONSTERS, HULL_HUMAN, id, 0);
	get_tr2(0, TR_vecEndPos, hit);
	if (get_tr2(0, TR_StartSolid) || hit[2] - a[2] < 250.0)
		return;
	hit[2] -= 16.0;
	engfunc(EngFunc_SetOrigin, id, hit);
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0});
	/* (the "on the ground" flag from where they stood stays until the next physics frame:
	   cleared now, or the very next think would count as the landing) */
	set_pev(id, pev_flags, pev(id, pev_flags) & ~FL_ONGROUND);
	set_pev(id, pev_groundentity, 0);
	set_user_gravity(id, 0.2);
	g_falling[id] = true;
	g_fallAt[id] = get_gametime();
	if (!is_user_bot(id))
		toast(id, 4.0, "Pick your landing! (steer as you fall)");
}

/* landed (or hurt by someone close to the ground): normal gravity again */
stock land(id)
{
	if (!g_falling[id])
		return;
	g_falling[id] = false;
	if (is_user_alive(id))
	{
		set_user_gravity(id, 1.0);
		if (g_phase == PH_DROP)
			loot_goal_on_landing(id);
	}
}

public fm_postthink(id)
{
	if (!g_falling[id] || !is_user_alive(id))
		return FMRES_IGNORED;
	/* landed: standing on something, a moment after the drop began, no longer falling */
	if ((pev(id, pev_flags) & FL_ONGROUND) && get_gametime() - g_fallAt[id] > 0.5)
	{
		new Float:v[3];
		pev(id, pev_velocity, v);
		if (v[2] > -50.0)
		{
			/* (the landing's fall damage came before this and was refused) */
			land(id);
			return FMRES_IGNORED;
		}
	}
	/* bots come straight down on the spot they picked (people steer) */
	if (is_user_bot(id))
	{
		new Float:v[3];
		pev(id, pev_velocity, v);
		v[0] = 0.0;
		v[1] = 0.0;
		set_pev(id, pev_velocity, v);
	}
	return FMRES_IGNORED;
}

public ham_takedamage(victim, inflictor, attacker, Float:dmg, bits)
{
	/* falling in: nothing hurts (not a hit, not the landing) and gravity stays low until
	   touchdown, which ends the fall in fm_postthink */
	if (g_falling[victim])
		return HAM_SUPERCEDE;
	/* the loot-up: nothing hurts at all (a bot that landed on a high roof and walked off died) */
	if (g_phase == PH_DROP)
		return HAM_SUPERCEDE;
	if (!(1 <= attacker <= MaxClients) || attacker == victim || !is_user_connected(attacker))
		return HAM_IGNORED;
	/* nobody hurts anybody while looting up; squad mates never hurt each other */
	if (g_phase == PH_WAIT || g_phase == PH_DROP)
		return HAM_SUPERCEDE;
	if (get_pcvar_num(g_cSquad) > 1 && g_squad[victim] && g_squad[victim] == g_squad[attacker])
		return HAM_SUPERCEDE;
	return HAM_IGNORED;
}

public ham_killed(victim, killer, gib)
{
	if (g_phase == PH_DROP)
		log_amx("Battle Royale: player %d died during the drop-in/loot-up (falling %d)", victim, g_falling[victim]);
	g_falling[victim] = false;
	if (g_phase != PH_PLAY)
		return HAM_IGNORED;
	if (1 <= killer <= MaxClients && killer != victim)
		g_kills[killer]++;
	if (get_pcvar_num(g_cSquad) > 1)
		set_task(4.0, "task_watch_squad", TASK_GEAR + 64 + victim);
	new place = alive_count() + 1;
	new name[32];
	get_user_name(victim, name, charsmax(name));
	ls_chat(0, "%s is out (#%d).", name, place);
	new wpn[24] = "world", Float:dist = 0.0;
	if (1 <= killer <= MaxClients && killer != victim && is_user_connected(killer))
	{
		get_weaponname(get_user_weapon(killer), wpn, charsmax(wpn));
		dist = entity_range(killer, victim);
	}
	log_amx("Battle Royale: player %d out at %d s (#%d), by %d (%s, %.0f u)", victim, floatround(get_gametime() - g_playAt), place, killer, wpn, dist);
	return HAM_IGNORED;
}

/* out: watch a squad mate who's still in it */
public task_watch_squad(tid)
{
	new id = tid - TASK_GEAR - 64;
	if (!is_user_connected(id) || is_user_alive(id) || is_user_bot(id))
		return;
	for (new q = 1; q <= MaxClients; q++)
	{
		if (q != id && is_user_alive(q) && g_squad[q] && g_squad[q] == g_squad[id])
		{
			set_pev(id, pev_iuser1, 4);	/* first person */
			set_pev(id, pev_iuser2, q);
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
		if (g_trait[p] != TR_LOOTER || now < g_nextGoal[p] || g_phase == PH_WAIT)
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
		formatex(line, charsmax(line), "goal %d loot %.0f %.0f %.0f %d 40 25 80", p, o[0], o[1], o[2], best);
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
	new Float:now = get_gametime();
	new top[128];
	new alive = alive_count();
	if (g_phase == PH_DROP)
		formatex(top, charsmax(top), "ALIVE %d   Loot up: fighting in %d s", alive, max(floatround(g_phaseEnd - now), 0));
	else
	{
		new Float:c[3], Float:r, closing;
		new Float:secs = ls_zone_next(0, c, r, closing);
		if (!g_zoneStarted)
			formatex(top, charsmax(top), "ALIVE %d   The zone starts closing in %d s", alive, max(floatround(get_pcvar_float(g_cZoneStart) * g_pace - (now - g_playAt)), 0));
		else if (closing)
			formatex(top, charsmax(top), "ALIVE %d   ZONE CLOSING (%d s)", alive, floatround(secs));
		else if (secs >= 0.0)
			formatex(top, charsmax(top), "ALIVE %d   Zone closes in %d:%02d", alive, floatround(secs) / 60, floatround(secs) % 60);
		else
			formatex(top, charsmax(top), "ALIVE %d   Final zone", alive);
	}
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue;
		ls_hud(p, LS_HUD_TOP, 1.1, "%s", top);
		/* kills and the squad on the right (menus open on the left) */
		if (!(get_pcvar_num(g_cSquad) > 1 && g_squad[p]))
			ls_hud(p, LS_HUD_RIGHT, 1.1, "Kills %d", g_kills[p]);
		if (get_pcvar_num(g_cSquad) > 1 && g_squad[p])
		{
			new team[256], name[24], len = 0;
			for (new q = 1; q <= MaxClients; q++)
			{
				if (q == p || g_squad[q] != g_squad[p] || !is_user_connected(q))
					continue;
				get_user_name(q, name, charsmax(name));
				if (is_user_alive(q))
					len += formatex(team[len], charsmax(team) - len, "%s  %d^n", name, get_user_health(q));
				else
					len += formatex(team[len], charsmax(team) - len, "%s  out^n", name);
			}
			ls_hud(p, LS_HUD_RIGHT, 1.1, "Kills %d^n^nSquad^n%s", g_kills[p], team);
		}
	}
}
