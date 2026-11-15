/*
	Every Node 3 - made with Loner-Strike's Game Mode Studio (graph version 3).
	Generated from every_node_3.graph.json: change the graph and compile it again; edits here are lost.
	This is a debug build (Playtest): it reports every node to the Studio.
*/

#include <amxmodx>
#include <cstrike>
#include <fakemeta>
#include <engine>
#include <hamsandwich>
#include <loner>
#include <celltrie>
#include <cellarray>

#define LSG_MODE "every_node_3"
#define LSG_CTX 21
#define LSG_TXT 2
#define LSG_N 124
#define LSG_TASK 7310000
#pragma dynamic 83456
stock g_lsgTxt[LSG_TXT][64]
stock g_lsgOut[LSG_N]
stock g_lsgLeft[LSG_N]
stock const g_lsgYesNo[][] = { "no", "yes" }
stock const g_lsgTeamName[][] = { "no team", "Terrorists", "Counter-Terrorists", "Spectators" }
#define LSG_VR 256
#define LSG_VP 256
#define LSG_RR 64
#define LSG_RP 64
#define LSG_RC 1
#define LSG_RT 1
#define LSG_AC 65
#define LSG_AMAX 10000
#define LSG_TMPA 32
#define LSG_O 8
#define LSG_OT 3

// The mode's variables
stock Float:g_v0 = 30.0 // Hold time (number)
stock g_v1 = 0 // Friendly (bool, each round)
stock Float:g_v2 = 1.0 // Size (number)

// ---- mode settings (hosts change them: the app's mode settings, the admin menu) ----
#define LSG_SN 3
stock const g_lsgSn[LSG_SN][] = { "lsm_every_node_3_hold", "lsm_every_node_3_ff", "lsm_every_node_3_size" }
stock const g_lsgSd[LSG_SN][] = { "30", "0", "1" }
stock const g_lsgSlab[LSG_SN][] = { "Seconds to win", "Friendly fire", "Hill" }
stock const Float:g_lsgSlo[LSG_SN] = { 5.0, 0.0, 0.0 }
stock const Float:g_lsgShi[LSG_SN] = { 120.0, 1.0, 2.0 }
stock const Float:g_lsgSst[LSG_SN] = { 12.0, 1.0, 1.0 }
stock g_lsgSc[LSG_SN], Float:g_lsgSl[LSG_SN], g_lsgSmenu[33]

stock lsg_settings_init()
{
	for (new k = 0; k < LSG_SN; k++)
		g_lsgSc[k] = register_cvar(g_lsgSn[k], g_lsgSd[k])
	register_clcmd("lsmm_every_node_3", "lsg_settings_menu", ADMIN_CFG)
}

// the host's value as it is (hosts are trusted: outside the author's range is theirs to choose)
stock Float:lsg_sv(k)
	return get_pcvar_float(g_lsgSc[k])

stock lsg_settings_path(path[], len)
{
	new dir[160]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	formatex(path, len, "%s/studio_settings/every_node_3.cfg", dir)
}

// the server's own values: only these names, as numbers (never run as commands); a setting the
// file leaves out is at the mode's default
stock lsg_settings_load()
{
	new path[200], line[96], key[64], val[32]
	for (new k = 0; k < LSG_SN; k++)
		set_pcvar_float(g_lsgSc[k], str_to_float(g_lsgSd[k]))
	lsg_settings_path(path, charsmax(path))
	new f = fopen(path, "rt")
	if (!f)
		return
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == '/' || line[0] == ';')
			continue
		parse(line, key, charsmax(key), val, charsmax(val))
		for (new k = 0; k < LSG_SN; k++)
		{
			if (equali(key, g_lsgSn[k]))
			{
				set_pcvar_float(g_lsgSc[k], str_to_float(val))
				break
			}
		}
	}
	fclose(f)
}

stock lsg_settings_save()
{
	new path[200], dir[160]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio_settings")
	if (!dir_exists(dir))
		mkdir(dir)
	lsg_settings_path(path, charsmax(path))
	new f = fopen(path, "wt")
	if (!f)
		return
	fputs(f, "// this server's values for the mode's settings (the app and the admin menu write it)^n")
	for (new k = 0; k < LSG_SN; k++)
		fprintf(f, "%s ^"%.3f^"^n", g_lsgSn[k], lsg_sv(k))
	fclose(f)
}

// the variables take the values (all, or the ones changed since)
stock lsg_settings_apply(bool:all)
{
	for (new k = 0; k < LSG_SN; k++)
	{
		new Float:v = lsg_sv(k)
		if (!all && v == g_lsgSl[k])
			continue
		g_lsgSl[k] = v
		switch (k)
		{
		case 0: g_v0 = v
		case 1: g_v1 = floatround(v) != 0
		case 2: g_v2 = v
		}
	}
}

stock lsg_settings_text(k, Float:v, out[], len)
{
	switch (k)
	{
		case 1:
		{
			copy(out, len, floatround(v) ? "Yes" : "No")
			return
		}
		case 2:
		{
			static const C[][] = { "Small", "Medium", "Large" }
			new c = clamp(floatround(v), 0, sizeof C - 1)
			copy(out, len, C[c])
			return
		}
	}
	if (v == float(floatround(v)))
		formatex(out, len, "%d", floatround(v))
	else
		formatex(out, len, "%.2f", v)
}

public lsg_settings_menu(id, level, cid)
{
	if (!(get_user_flags(id) & level))
		return PLUGIN_HANDLED
	new m = menu_create("\yMode settings^n\dSaved for this server", "lsg_settings_menu_h"), t[96], v[40], info[4]
	menu_additem(m, "Reset all to the mode's defaults", "all")
	for (new k = 0; k < LSG_SN; k++)
	{
		lsg_settings_text(k, lsg_sv(k), v, charsmax(v))
		// (a * marks one that isn't the mode's default)
		formatex(t, charsmax(t), "%s: \y%s%s", g_lsgSlab[k], v, lsg_sv(k) == str_to_float(g_lsgSd[k]) ? "" : " \r*")
		num_to_str(k, info, charsmax(info))
		menu_additem(m, t, info)
	}
	menu_display(id, m)
	return PLUGIN_HANDLED
}

public lsg_settings_menu_h(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new info[4], acc, cb
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb)
	menu_destroy(menu)
	if (equal(info, "all"))
	{
		if (!(get_user_flags(id) & ADMIN_CFG))
			return PLUGIN_HANDLED
		for (new k = 0; k < LSG_SN; k++)
			set_pcvar_float(g_lsgSc[k], str_to_float(g_lsgSd[k]))
		lsg_settings_save()
		lsg_settings_apply(false)
		lsg_settings_menu(id, ADMIN_CFG, 0)
		return PLUGIN_HANDLED
	}
	g_lsgSmenu[id] = str_to_num(info)
	lsg_settings_one(id)
	return PLUGIN_HANDLED
}

stock lsg_settings_one(id)
{
	new k = g_lsgSmenu[id], v[40], t[96]
	lsg_settings_text(k, lsg_sv(k), v, charsmax(v))
	formatex(t, charsmax(t), "\y%s: %s", g_lsgSlab[k], v)
	new m = menu_create(t, "lsg_settings_one_h")
	menu_additem(m, "Less", "-")
	menu_additem(m, "More", "+")
	menu_additem(m, "Smallest", "lo")
	menu_additem(m, "Largest", "hi")
	menu_additem(m, "Reset to the mode's default", "d")
	menu_display(id, m)
}

public lsg_settings_one_h(id, menu, item)
{
	if (item < 0)
	{
		menu_destroy(menu)
		lsg_settings_menu(id, ADMIN_CFG, 0)
		return PLUGIN_HANDLED
	}
	new info[4], acc, cb, k = g_lsgSmenu[id]
	menu_item_getinfo(menu, item, acc, info, charsmax(info), _, _, cb)
	menu_destroy(menu)
	if (!(get_user_flags(id) & ADMIN_CFG) || k < 0 || k >= LSG_SN)
		return PLUGIN_HANDLED
	new Float:v = lsg_sv(k)
	if (equal(info, "-"))
		v -= g_lsgSst[k]
	else if (equal(info, "+"))
		v += g_lsgSst[k]
	else if (equal(info, "lo"))
		v = g_lsgSlo[k]
	else if (equal(info, "hi"))
		v = g_lsgShi[k]
	else
		v = str_to_float(g_lsgSd[k])
	set_pcvar_float(g_lsgSc[k], v)
	lsg_settings_save()
	lsg_settings_apply(false)
	lsg_settings_one(id)
	return PLUGIN_HANDLED
}

stock g_lsgM8[33][LSG_CTX]
stock g_lsgAsk[33], g_lsgAskK[33][6]

stock lsg_npc_line(out[], len, const speaker[], const text[])
{
	if (speaker[0])
		formatex(out, len, "%s: %s", speaker, text)
	else
		copy(out, len, text)
}

stock lsg_npc_rep(out[], len, id, &n, cap, k, const r[])
{
	if (n >= cap || !r[0] || id < 1 || id > MaxClients)
		return
	new t[48]
	copy(t, charsmax(t), r)
	replace_all(t, charsmax(t), "|", "/")
	if (n)
		add(out, len, "|")
	add(out, len, t)
	g_lsgAskK[id][n++] = k
}

public ls_on_npc_choice(id, npc, choice)
{
	#pragma unused npc
	if (id < 1 || id > MaxClients)
		return PLUGIN_CONTINUE
	new k = g_lsgAsk[id]
	g_lsgAsk[id] = 0
	switch (k)
	{
		case 9:
			lsg_ask8(id, choice)
	}
	return PLUGIN_CONTINUE
}

stock bool:lsg_proj_from(e, Float:f, Float:r, Float:u, rot, Float:o[3], Float:d[3])
{
	if (e <= 0 || !pev_valid(e) || (e <= MaxClients && !is_user_alive(e)))
		return false
	new Float:a[3], Float:fw[3], Float:rt[3], Float:up[3]
	pev(e, pev_origin, o)
	if (e <= MaxClients)
	{
		new Float:vo[3]
		pev(e, pev_view_ofs, vo)
		for (new k = 0; k < 3; k++)
			o[k] += vo[k]
		pev(e, pev_v_angle, a)
	}
	else
		pev(e, pev_angles, a)
	if (rot >= 0)
		for (new k = 0; k < 3; k++)
			a[k] += lsg_vc(rot, k)
	angle_vector(a, ANGLEVECTOR_FORWARD, fw)
	angle_vector(a, ANGLEVECTOR_RIGHT, rt)
	angle_vector(a, ANGLEVECTOR_UP, up)
	for (new k = 0; k < 3; k++)
	{
		o[k] += fw[k] * f + rt[k] * r + up[k] * u
		d[k] = fw[k]
	}
	return true
}

#define LSG_PJ 64
stock g_lsgPjEnt[LSG_PJ], g_lsgPjNode[LSG_PJ], g_lsgPjCtx[LSG_PJ][LSG_CTX], g_lsgPjAt

stock lsg_pj_find(e)
{
	if (e <= 0)
		return -1
	for (new s = 0; s < LSG_PJ; s++)
		if (g_lsgPjEnt[s] == e)
			return s
	return -1
}

stock lsg_pj_note(e, node, const ctx[])
{
	new s = lsg_pj_find(e)
	if (s < 0)
	{
		for (new k = 0; k < LSG_PJ && s < 0; k++)
			if (!g_lsgPjEnt[k] || !pev_valid(g_lsgPjEnt[k]))
				s = k
		if (s < 0)
		{
			s = g_lsgPjAt
			g_lsgPjAt = (g_lsgPjAt + 1) % LSG_PJ
		}
	}
	g_lsgPjEnt[s] = e
	g_lsgPjNode[s] = node
	for (new k = 0; k < LSG_CTX; k++)
		g_lsgPjCtx[s][k] = ctx[k]
}

public ls_on_proj_hit_node(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	new s = lsg_pj_find(proj)
	if (s < 0)
		return
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = g_lsgPjCtx[s][k]
	switch (g_lsgPjNode[s])
	{
		case 15:
			lsg_pjhit14(ctx, owner, pos, normal, hit, hitgroup)
	}
}

public ls_on_proj_expire(proj, owner)
{
	#pragma unused owner
	new s = lsg_pj_find(proj)
	if (s < 0)
		return
	new ctx[LSG_CTX], k = g_lsgPjNode[s]
	for (new c = 0; c < LSG_CTX; c++)
		ctx[c] = g_lsgPjCtx[s][c]
	g_lsgPjEnt[s] = 0
	switch (k)
	{
		case 15:
			lsg_pjexp14(ctx)
	}
}

stock lsg_proj_vel(e)
{
	new Float:v[3]
	if (e > 0)
		ls_proj_velocity(e, v)
	return lsg_vfrom(v)
}
#define LSD_N 124
new const g_lsdIds[LSD_N][] = { "ps", "tick", "ph", "phx", "ch", "chp", "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n12", "n25", "n26", "n27", "n28", "n30", "n32", "n34", "n36", "n38", "n40", "n42", "n44", "n46", "n48", "n50", "n52", "n54", "n56", "n58", "n60", "n62", "n82", "n83", "n84", "n85", "n86", "n87", "n88", "n89", "n90", "n91", "n92", "n93", "n94", "n95", "n96", "n97", "n98", "n99", "n100", "n101", "n102", "n106", "n107", "n108", "n109", "n110", "n111", "n112", "n113", "n114", "n115", "n116", "n117", "n118", "n119", "n120", "n121", "n122", "n123", "n124", "n125", "n126", "n127", "n129", "n130", "n131", "n132", "n133", "n139", "n141", "n142", "n143", "n144", "n145", "n146", "n147", "n148", "n149", "n152", "n153", "n154", "n155", "n156", "n162", "n163", "n164", "n165", "n168", "n169", "n170", "n171", "n172", "n173", "n176", "n180", "n186", "n187", "n188", "n189", "n190", "n191", "n192", "n193", "n194", "n195" }
new bool:g_lsdBp[LSD_N] = { false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false, false }
// The Game Mode Studio's debug runtime (tools/studio/gen.rs puts it into Playtest builds only).
// Every helper is a stock, so the ones a graph doesn't use cost nothing and warn nothing.
new const g_lsdBool[][] = { "false", "true" }
new const g_lsdTeams[][] = { "none", "T", "CT", "spectator" }
#define LSD_TASK 9475100
new g_lsdEvents[160], g_lsdCmd[160], g_lsdSeen[160]
new g_lsdSeq, g_lsdTick, g_lsdRound
new g_lsdPhase[8] = "freeze"
new g_lsdIn[600]
new g_lsdPaused = -1, g_lsdSkip = -1
new g_lsdCtx[LSG_CTX]
// while the world is paused nothing else runs: what would (a loop's other rounds, what comes
// after the paused node, other events) waits here in order and runs after Resume, so a
// breakpoint inside a loop pauses that round and the loop carries on from there
#define LSD_Q 128
new g_lsdQ[LSD_Q][LSG_CTX + 1], g_lsdQn, g_lsdQins
new Float:g_lsdPausedAt, Float:g_lsdResumedAt
new g_lsdBotStop
// the round clock: Pawn can't stop the game's own clock safely (ReGameDLL's rules object isn't
// laid out like AMX Mod X's gamedata says), so while paused no round may end (mp_round_infinite)
// and afterwards the round may not end on time until the paused seconds are made up
new Float:g_lsdLiveAt, Float:g_lsdFreezeAt, Float:g_lsdOwed
new g_lsdInfWas[16], bool:g_lsdHolding
new bool:g_lsdHeld[33], g_lsdWasFrozen[33], g_lsdCanShoot[33], Float:g_lsdWasDmg[33]

lsd_init()
{
	new dir[128]
	get_localinfo("amxx_datadir", dir, charsmax(dir))
	add(dir, charsmax(dir), "/studio")
	if (!dir_exists(dir))
		mkdir(dir)
	formatex(g_lsdEvents, charsmax(g_lsdEvents), "%s/%s.events", dir, LSG_MODE)
	formatex(g_lsdCmd, charsmax(g_lsdCmd), "%s/%s.cmd", dir, LSG_MODE)
	formatex(g_lsdSeen, charsmax(g_lsdSeen), "%s/%s.seen", dir, LSG_MODE)
	new f = fopen(g_lsdEvents, "wt")
	if (f)
		fclose(f)
	// the breakpoints the Studio has on now (kept across map changes; "bp" commands change them live)
	new bps[160], line[600]
	formatex(bps, charsmax(bps), "%s/%s.bp", dir, LSG_MODE)
	f = fopen(bps, "rt")
	if (f)
	{
		fgets(f, line, charsmax(line))
		fclose(f)
		trim(line)
		lsd_set_bps(line)
	}
	register_event("HLTV", "lsd_new_round", "a", "1=0", "2=0")
	register_logevent("lsd_round_start", 2, "1=Round_Start")
	register_logevent("lsd_round_end", 2, "1=Round_End")
	set_task(0.25, "lsd_poll", LSD_TASK, _, _, "b")
	new map[32]
	get_mapname(map, charsmax(map))
	lsd_emitf("^"e^":^"session^",^"mode^":^"%s^",^"map^":^"%s^",^"nodes^":%d", LSG_MODE, map, LSD_N)
}

// (gen.rs's plugin_end calls this)
stock lsd_end()
{
	lsd_flush()
	// a map change while paused: the bots mustn't stay stopped, rounds must end again
	if (g_lsdPaused >= 0)
		set_cvar_num("bot_stop", g_lsdBotStop)
	lsd_unhold()
}

public lsd_new_round()
{
	g_lsdRound++
	copy(g_lsdPhase, charsmax(g_lsdPhase), "freeze")
	g_lsdFreezeAt = get_gametime()
	g_lsdOwed = 0.0
	if (g_lsdPaused < 0)
		lsd_unhold()
}

public lsd_round_start()
{
	copy(g_lsdPhase, charsmax(g_lsdPhase), "live")
	g_lsdLiveAt = get_gametime()
	g_lsdOwed = 0.0
}

stock lsd_hold(const how[])
{
	if (!g_lsdHolding)
	{
		get_cvar_string("mp_round_infinite", g_lsdInfWas, charsmax(g_lsdInfWas))
		// someone set it on purpose: leave it alone
		if (g_lsdInfWas[0] && !equal(g_lsdInfWas, "0"))
			return
		g_lsdHolding = true
	}
	set_cvar_string("mp_round_infinite", how)
}

stock lsd_unhold()
{
	if (!g_lsdHolding)
		return
	g_lsdHolding = false
	if (g_lsdInfWas[0])
		set_cvar_string("mp_round_infinite", g_lsdInfWas)
	else
		set_cvar_string("mp_round_infinite", "0")
}

public lsd_round_end()
{
	copy(g_lsdPhase, charsmax(g_lsdPhase), "ended")
}

// events wait in memory and go to the file every 0.25 s (lsd_poll), when the buffer is nearly
// full, and at once for a pause (the page shows it); one open/write/close per flush, not per
// event (PerfAudit #5: 181 us an event). Under 16384: AMX Mod X's fputs takes at most 16383
// characters of a string and drops the rest (node coverage run: 44 events lost, one cut in half)
#define LSD_BUF 16000
new g_lsdBuf[LSD_BUF], g_lsdBufLen
// how big the events file is now (it starts empty at each map)
new g_lsdBytes

stock lsd_flush()
{
	if (!g_lsdBufLen)
		return
	new f = fopen(g_lsdEvents, "at")
	if (f)
	{
		fputs(f, g_lsdBuf)
		fclose(f)
		g_lsdBytes += g_lsdBufLen
	}
	g_lsdBuf[0] = 0
	g_lsdBufLen = 0
}

// ---- who's watching, and how much is written (StudioPerfAudit #5) ----
// The app writes <mode>.seen (how far it has read) each time the page reads. Not changed for
// 15 s: nobody is watching, so nodes' runs aren't written (breakpoints still pause). With no
// .seen at all (a Test run, or before the page's first read) everything is written.
#define LSD_AWAY 15
// the file starts over once the reader has had all of it past 8 MB, or regardless at 32 MB
// (the reader sees it shrink and reads from the top: a "reset")
#define LSD_CAP 8000000
#define LSD_HARD 32000000
// each node writes at most this many runs a half second (4 a second: plenty to watch it); the
// rest are counted, and the next run written carries the count ("skipped"), so totals stay right
#define LSD_PER 2
new bool:g_lsdAway, g_lsdRead = -1
new Float:g_lsdWin[LSD_N], g_lsdWinN[LSD_N], g_lsdSkipped[LSD_N], bool:g_lsdMute[LSD_N]

stock lsd_watchers()
{
	if (!file_exists(g_lsdSeen))
	{
		g_lsdAway = false
		g_lsdRead = -1
		return
	}
	g_lsdAway = get_systime() - GetFileTime(g_lsdSeen, FileTime_LastChange) > LSD_AWAY
	new f = fopen(g_lsdSeen, "rt"), line[24]
	if (f)
	{
		fgets(f, line, charsmax(line))
		fclose(f)
		g_lsdRead = str_to_num(line)
	}
}

stock lsd_rotate()
{
	if (g_lsdBytes > LSD_HARD || (g_lsdBytes > LSD_CAP && g_lsdRead >= g_lsdBytes))
	{
		new f = fopen(g_lsdEvents, "wt")
		if (f)
			fclose(f)
		g_lsdBytes = 0
	}
}

// Is node n's run to be written? (its inputs are only worked out into JSON when it is)
// `run`: false for a note that isn't a run (a Timeline's progress): it shares the budget but
// doesn't count as a skipped run
stock bool:lsd_want(n, bool:run = true)
{
	if (g_lsdBp[n])
	{
		g_lsdMute[n] = false
		return true
	}
	if (g_lsdAway)
	{
		if (run)
			g_lsdSkipped[n]++
		g_lsdMute[n] = true
		return false
	}
	new Float:now = get_gametime()
	if (now - g_lsdWin[n] >= 0.5)
	{
		g_lsdWin[n] = now
		g_lsdWinN[n] = 0
	}
	if (++g_lsdWinN[n] > LSD_PER)
	{
		if (run)
			g_lsdSkipped[n]++
		g_lsdMute[n] = true
		return false
	}
	g_lsdMute[n] = false
	return true
}

stock lsd_emitf(const fmt[], any:...)
{
	static body[1400], line[1500]
	vformat(body, charsmax(body), fmt, 2)
	new n = formatex(line, charsmax(line), "{^"s^":%d,^"t^":%.2f,%s}^n", ++g_lsdSeq, get_gametime(), body)
	if (g_lsdBufLen + n >= LSD_BUF - 1)
		lsd_flush()
	g_lsdBufLen += copy(g_lsdBuf[g_lsdBufLen], LSD_BUF - 1 - g_lsdBufLen, line)
	// a pause, or the world stopping or going on: the page needs it now
	if (containi(body, "breakpoint_hit") >= 0 || containi(body, "world_") >= 0 || containi(body, "session") >= 0)
		lsd_flush()
}

stock lsd_esc(const src[], dst[], len)
{
	new j = 0
	for (new i = 0; src[i] && j < len - 2; i++)
	{
		if (src[i] == '"' || src[i] == '\')
		{
			dst[j++] = '\'
			dst[j++] = src[i]
		}
		else if (src[i] >= 32)
			dst[j++] = src[i]
	}
	dst[j] = 0
}

stock lsd_pl(id, out[], len)
{
	if (id < 1 || id > MaxClients || !is_user_connected(id))
	{
		copy(out, len, "null")
		return
	}
	new name[32], esc[72], wpn[32]
	get_user_name(id, name, charsmax(name))
	lsd_esc(name, esc, charsmax(esc))
	new alive = is_user_alive(id) ? 1 : 0
	if (alive)
	{
		new w = get_user_weapon(id)
		if (w > 0)
		{
			get_weaponname(w, wpn, charsmax(wpn))
			replace(wpn, charsmax(wpn), "weapon_", "")
		}
	}
	// (a player still joining has no team data yet: asking would be an error)
	new team = pev_valid(id) == 2 ? (_:cs_get_user_team(id)) : 0
	if (team < 0 || team > 3)
		team = 0
	formatex(out, len, "{^"id^":%d,^"name^":^"%s^",^"team^":^"%s^",^"hp^":%d,^"alive^":%s,^"weapon^":^"%s^",^"bot^":%s}",
		id, esc, g_lsdTeams[team], alive ? get_user_health(id) : 0, g_lsdBool[alive], wpn, g_lsdBool[is_user_bot(id) ? 1 : 0])
}

stock lsd_begin()
{
	g_lsdIn[0] = 0
}

stock lsd_in_raw(const name[], const json[])
{
	if (g_lsdIn[0])
		add(g_lsdIn, charsmax(g_lsdIn), ",")
	new part[300]
	formatex(part, charsmax(part), "^"%s^":%s", name, json)
	add(g_lsdIn, charsmax(g_lsdIn), part)
}

stock lsd_in_player(const name[], id)
{
	new pl[240]
	lsd_pl(id, pl, charsmax(pl))
	lsd_in_raw(name, pl)
}

stock lsd_in_number(const name[], Float:v)
{
	new q[24]
	formatex(q, charsmax(q), "%.2f", v)
	lsd_in_raw(name, q)
}

stock lsd_in_bool(const name[], v)
{
	lsd_in_raw(name, g_lsdBool[v ? 1 : 0])
}

stock lsd_in_text(const name[], const v[])
{
	new esc[140], q[144]
	lsd_esc(v, esc, charsmax(esc))
	formatex(q, charsmax(q), "^"%s^"", esc)
	lsd_in_raw(name, q)
}

stock lsd_in_entity(const name[], ent)
{
	if (ent >= 1 && ent <= MaxClients)
	{
		lsd_in_player(name, ent)
		return
	}
	new q[200], cls[32], tn[32], ecls[72], etn[72]
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_classname, cls, charsmax(cls))
		pev(ent, pev_targetname, tn, charsmax(tn))
		lsd_esc(cls, ecls, charsmax(ecls))
		lsd_esc(tn, etn, charsmax(etn))
		formatex(q, charsmax(q), "{^"ent^":%d,^"class^":^"%s^",^"name^":^"%s^"}", ent, ecls, etn)
	}
	else
		copy(q, charsmax(q), "null")
	lsd_in_raw(name, q)
}

stock lsd_in_team(const name[], team)
{
	new t = team
	if (t < 0 || t > 3)
		t = 0
	new q[16]
	formatex(q, charsmax(q), "^"%s^"", g_lsdTeams[t])
	lsd_in_raw(name, q)
}

// a loop's result: how many rounds; when some are waiting at a breakpoint, it says so
stock lsd_times(n)
{
	new t[64]
	if (g_lsdPaused >= 0)
		formatex(t, charsmax(t), "%d rounds (some still wait at a breakpoint)", n)
	else
		formatex(t, charsmax(t), "%d times", n)
	return t
}

stock lsd_defer(n, const ctx[])
{
	if (g_lsdQn >= LSD_Q)
	{
		lsd_emitf("^"e^":^"node_error^",^"node^":^"%s^",^"message^":^"Too much waiting while paused: this run was dropped.^"", g_lsdIds[n])
		return
	}
	for (new q = g_lsdQn; q > g_lsdQins; q--)
		for (new k = 0; k <= LSG_CTX; k++)
			g_lsdQ[q][k] = g_lsdQ[q - 1][k]
	g_lsdQ[g_lsdQins][0] = n
	for (new k = 0; k < LSG_CTX; k++)
		g_lsdQ[g_lsdQins][k + 1] = ctx[k]
	g_lsdQins++
	g_lsdQn++
}

stock bool:lsd_enter(n, const ctx[])
{
	if (g_lsdSkip == n)
		g_lsdSkip = -1
	else if (g_lsdPaused >= 0)
	{
		lsd_defer(n, ctx)
		return true
	}
	else if (g_lsdBp[n])
	{
		g_lsdPaused = n
		// (what waits from now on goes before what was already waiting)
		g_lsdQins = 0
		for (new i = 0; i < LSG_CTX; i++)
			g_lsdCtx[i] = ctx[i]
		lsd_emitf("^"e^":^"breakpoint_hit^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
		lsd_freeze()
		return true
	}
	if (g_lsdMute[n])
		return false
	if (g_lsdSkipped[n])
	{
		lsd_emitf("^"e^":^"node_enter^",^"node^":^"%s^",^"skipped^":%d,^"inputs^":{%s}", g_lsdIds[n], g_lsdSkipped[n], g_lsdIn)
		g_lsdSkipped[n] = 0
	}
	else
		lsd_emitf("^"e^":^"node_enter^",^"node^":^"%s^",^"inputs^":{%s}", g_lsdIds[n], g_lsdIn)
	return false
}

stock lsd_exit(n, const result[])
{
	if (!g_lsdMute[n])
		lsd_emitf("^"e^":^"node_exit^",^"node^":^"%s^",^"result^":^"%s^"", g_lsdIds[n], result)
}

// a note that's always written while someone watches (a Timeline's Finished)
stock lsd_note(n, const result[])
{
	if (!g_lsdAway)
		lsd_emitf("^"e^":^"node_exit^",^"node^":^"%s^",^"result^":^"%s^"", g_lsdIds[n], result)
}

stock lsd_error(n, const msg[])
{
	// (a sampled run's error still shows, unless nobody is watching)
	if (!g_lsdAway)
		lsd_emitf("^"e^":^"node_error^",^"node^":^"%s^",^"message^":^"%s^"", g_lsdIds[n], msg)
}

stock lsd_time_left()
{
	new Float:now = g_lsdPaused >= 0 ? g_lsdPausedAt : get_gametime()
	new Float:left
	if (equal(g_lsdPhase, "live"))
		left = get_cvar_float("mp_roundtime") * 60.0 - (now - g_lsdLiveAt - g_lsdOwed)
	else if (equal(g_lsdPhase, "freeze"))
		left = get_cvar_float("mp_freezetime") - (now - g_lsdFreezeAt)
	return max(floatround(left, floatround_ceil), 0)
}

stock lsd_hold_player(id)
{
	if (g_lsdHeld[id] || !is_user_alive(id))
		return
	g_lsdHeld[id] = true
	g_lsdWasFrozen[id] = pev(id, pev_flags) & FL_FROZEN
	g_lsdCanShoot[id] = get_ent_data(id, "CBasePlayer", "m_bCanShoot")
	pev(id, pev_takedamage, g_lsdWasDmg[id])
	set_pev(id, pev_flags, pev(id, pev_flags) | FL_FROZEN)
	set_pev(id, pev_takedamage, 0.0)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_ent_data(id, "CBasePlayer", "m_bCanShoot", false)
}

stock lsd_freeze()
{
	g_lsdPausedAt = get_gametime()
	g_lsdBotStop = get_cvar_num("bot_stop")
	set_cvar_num("bot_stop", 1)
	for (new id = 1; id <= MaxClients; id++)
		lsd_hold_player(id)
	lsd_hold("1")
	lsd_emitf("^"e^":^"world_paused^",^"node^":^"%s^"", g_lsdIds[g_lsdPaused])
	lsd_vars()
}

stock lsd_release()
{
	g_lsdResumedAt = get_gametime()
	new Float:paused = get_gametime() - g_lsdPausedAt
	set_cvar_num("bot_stop", g_lsdBotStop)
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!g_lsdHeld[id])
			continue
		g_lsdHeld[id] = false
		if (!is_user_connected(id))
			continue
		if (!g_lsdWasFrozen[id])
			set_pev(id, pev_flags, pev(id, pev_flags) & ~FL_FROZEN)
		set_pev(id, pev_takedamage, g_lsdWasDmg[id])
		if (is_user_alive(id))
			set_ent_data(id, "CBasePlayer", "m_bCanShoot", g_lsdCanShoot[id])
	}
	// the paused seconds are added to the round: it can't end on time until they're made up
	if (equal(g_lsdPhase, "live"))
	{
		g_lsdOwed += paused
		lsd_hold("f")
	}
	else
		lsd_unhold()
	lsd_emitf("^"e^":^"world_resumed^",^"paused^":%.1f", paused)
	message_begin(MSG_ALL, get_user_msgid("RoundTime"))
	write_short(lsd_time_left())
	message_end()
}

stock lsd_vars()
{
	new t, ct
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id))
			continue
		switch (cs_get_user_team(id))
		{
			case CS_TEAM_T: t++
			case CS_TEAM_CT: ct++
		}
	}
	lsd_emitf("^"e^":^"vars^",^"round^":%d,^"phase^":^"%s^",^"timeLeft^":%d,^"aliveT^":%d,^"aliveCT^":%d,^"paused^":%s",
		g_lsdRound, g_lsdPhase, lsd_time_left(), t, ct, g_lsdBool[g_lsdPaused >= 0 ? 1 : 0])
	new pl[240]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_connected(id))
			continue
		lsd_pl(id, pl, charsmax(pl))
		lsd_emitf("^"e^":^"var_player^",^"p^":%s", pl)
	}
	lsd_graph_vars()
}

stock lsd_var_number(const v[], p, Float:value)
{
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":%.2f", v, p, value)
}

stock lsd_var_bool(const v[], p, value)
{
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":%s", v, p, g_lsdBool[value ? 1 : 0])
}

stock lsd_var_text(const v[], p, const value[])
{
	new esc[140]
	lsd_esc(value, esc, charsmax(esc))
	lsd_emitf("^"e^":^"var_value^",^"v^":^"%s^",^"p^":%d,^"value^":^"%s^"", v, p, esc)
}

// a live tweak from the app: "set <variable id> <who> <value>" (who: 0 the mode's, a player's
// index, or 1 / 2 a team's; the app checked the value against the variable's type)
stock lsd_set_cmd(const s[])
{
	new vid[32], who[8], pos
	pos = argparse(s, 0, vid, charsmax(vid))
	if (pos == -1)
		return
	pos = argparse(s, pos, who, charsmax(who))
	if (pos == -1)
		return
	while (s[pos] == ' ')
		pos++
	new val[64]
	copy(val, charsmax(val), s[pos])
	if (lsd_graph_set(vid, str_to_num(who), val))
	{
		new esc[140]
		lsd_esc(val, esc, charsmax(esc))
		lsd_emitf("^"e^":^"var_set^",^"v^":^"%s^",^"p^":%d,^"value^":^"%s^"", vid, str_to_num(who), esc)
		lsd_vars()
	}
}

// a timer that fires while the world is paused waits until it isn't
stock bool:lsd_hold_timer()
{
	return g_lsdPaused >= 0
}

// A node's timer (Delay, Repeat, Countdown) that came due while the world was paused: after
// Resume it waits what it still had to wait when the pause began, from the Resume, so a Repeat
// keeps its rhythm instead of firing at once (it's checked four times a second while paused).
// True: not now (it has been put off). `fn`: the timer's own callback; its task id is LSG_TASK + n.
new bool:g_lsdTWait[LSD_N], Float:g_lsdTOwe[LSD_N]
stock bool:lsd_timer_wait(n, const fn[], const data[])
{
	if (g_lsdPaused >= 0)
	{
		if (!g_lsdTWait[n])
		{
			g_lsdTWait[n] = true
			g_lsdTOwe[n] = get_gametime() - g_lsdPausedAt
		}
		set_task(0.25, fn, LSG_TASK + n, data, LSG_CTX)
		return true
	}
	if (!g_lsdTWait[n])
		return false
	g_lsdTWait[n] = false
	new Float:left = g_lsdTOwe[n] - (get_gametime() - g_lsdResumedAt)
	if (left < 0.05)
		return false
	set_task(left, fn, LSG_TASK + n, data, LSG_CTX)
	return true
}

stock lsd_set_bps(const list[])
{
	for (new i = 0; i < LSD_N; i++)
		g_lsdBp[i] = false
	new pos, id[32]
	while ((pos = argparse(list, pos, id, charsmax(id))) != -1)
	{
		for (new i = 0; i < LSD_N; i++)
			if (equal(g_lsdIds[i], id))
				g_lsdBp[i] = true
	}
}

stock lsd_resume()
{
	if (g_lsdPaused < 0)
		return
	new n = g_lsdPaused
	g_lsdPaused = -1
	lsd_release()
	g_lsdSkip = n
	new ctx[LSG_CTX]
	for (new i = 0; i < LSG_CTX; i++)
		ctx[i] = g_lsdCtx[i]
	lsd_call(n, ctx)
	// then what waited, in order, until it's all run or another breakpoint pauses again
	while (g_lsdPaused < 0 && g_lsdQn > 0)
	{
		new m = g_lsdQ[0][0]
		for (new i = 0; i < LSG_CTX; i++)
			ctx[i] = g_lsdQ[0][i + 1]
		for (new q = 1; q < g_lsdQn; q++)
			for (new k = 0; k <= LSG_CTX; k++)
				g_lsdQ[q - 1][k] = g_lsdQ[q][k]
		g_lsdQn--
		lsd_call(m, ctx)
	}
}

public lsd_poll()
{
	// (who's watching and how far they've read: once a second; the old part of the file goes
	// before this flush, so nothing unread is lost)
	if (++g_lsdTick % 4 == 1)
	{
		lsd_watchers()
		lsd_rotate()
	}
	lsd_flush()
	if (g_lsdPaused >= 0)
	{
		for (new id = 1; id <= MaxClients; id++)
			lsd_hold_player(id)
		if (g_lsdTick % 4 == 0)
			lsd_vars()
	}
	else if (g_lsdHolding && lsd_time_left() <= 0)
		lsd_unhold()
	if (!file_exists(g_lsdCmd))
		return
	// taken aside first: a command the app adds while this reads goes into a new file, not lost
	new take[170]
	formatex(take, charsmax(take), "%s.take", g_lsdCmd)
	if (file_exists(take))
		delete_file(take)
	if (!rename_file(g_lsdCmd, take, 1))
		return
	new f = fopen(take, "rt")
	if (!f)
		return
	new line[600], bool:resume
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (equal(line, "resume"))
			resume = true
		else if (equal(line, "vars"))
			lsd_vars()
		else if (equal(line, "bp", 2) && (line[2] == ' ' || line[2] == 0))
			lsd_set_bps(line[2])
		else if (equal(line, "set ", 4))
			lsd_set_cmd(line[4])
	}
	fclose(f)
	delete_file(take)
	lsd_emitf("^"e^":^"commands^",^"resume^":%s", g_lsdBool[resume ? 1 : 0])
	if (resume)
		lsd_resume()
}

// ---- the game and the server (Game, World, Players, Entities, Effects, Advanced nodes) ----
stock g_lsgRounds, g_lsgScore[3], bool:g_lsgFreeze, Float:g_lsgGagTill[33], g_lsgFw = -1, bool:lsg_cmd_ok, g_lsgSprExp, g_lsgSprBeam, Trie:g_lsgHave = Invalid_Trie

// a model or sound the graph names, loaded only if the file is there (on the server or in the
// game's folders): a missing one would stop the map loading. What uses it does without.
// (a mode loads at most 160 files of its own: the game's 512 models and 512 sounds are for
// everything on the server; limits.rs PRECACHE_MAX)
stock g_lsgPreN
stock bool:lsg_pre_room(const p[])
{
	if (g_lsgPreN < 160)
	{
		g_lsgPreN++
		return true
	}
	log_amx("[%s] %s not loaded: the mode has loaded 160 files of its own already", LSG_MODE, p)
	return false
}

stock lsg_pre_model(const p[])
{
	if (!file_exists(p, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, p)
		return 0
	}
	if (!lsg_pre_room(p))
		return 0
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_model(p)
}

stock lsg_pre_sound(const p[])
{
	new full[128]
	formatex(full, charsmax(full), "sound/%s", p)
	if (!file_exists(full, true))
	{
		log_amx("[%s] missing %s: what uses it does without", LSG_MODE, full)
		return 0
	}
	if (!lsg_pre_room(full))
		return 0
	if (g_lsgHave == Invalid_Trie)
		g_lsgHave = TrieCreate()
	TrieSetCell(g_lsgHave, p, 1)
	return precache_sound(p)
}

// room for more messages this server frame (temp entities, HUD, chat): past the budget (the
// Effect messages a frame limit) the effect is skipped, said in the log once a second; a unicast
// (to a player) counts against that player too, at half the budget
stock Float:g_lsgMsgAt, g_lsgMsgN, g_lsgMsgP[33], Float:g_lsgMsgSaid, g_lsgMsgLim = 48
stock bool:lsg_msg_room(const what[], cost = 1, to = 0, lim = 0)
{
	if (lim <= 0)
		lim = g_lsgMsgLim
	new Float:now = get_gametime()
	if (now != g_lsgMsgAt)
	{
		g_lsgMsgAt = now
		g_lsgMsgN = 0
		arrayset(g_lsgMsgP, 0, sizeof g_lsgMsgP)
	}
	if (g_lsgMsgN + cost > lim || (to >= 1 && to <= 32 && g_lsgMsgP[to] + cost > lim / 2))
	{
		if (now - g_lsgMsgSaid >= 1.0)
		{
			g_lsgMsgSaid = now
			log_amx("[%s] %s skipped: too many effects this frame", LSG_MODE, what)
		}
		return false
	}
	g_lsgMsgN += cost
	if (to >= 1 && to <= 32)
		g_lsgMsgP[to] += cost
	return true
}

// room for one more entity: free slots are kept for the game (it crashes with "no free edicts");
// a mode's spawn that would eat into them is skipped, and the server log says what and why
#define LSG_EDICT_RESERVE 96
stock bool:lsg_ent_room(const what[])
{
	if (engfunc(EngFunc_NumberOfEntities) < global_get(glb_maxEntities) - LSG_EDICT_RESERVE)
		return true
	log_amx("[%s] %s skipped: the server is out of entity slots", LSG_MODE, what)
	return false
}

// was it loaded (lsg_pre_model / lsg_pre_sound)?
stock bool:lsg_have(const p[])
	return g_lsgHave != Invalid_Trie && TrieKeyExists(g_lsgHave, p)

stock lsg_map_name()
{
	new m[32]
	get_mapname(m, charsmax(m))
	return m
}

// cvars no mode may read (passwords, rcon, where files and logs go)
stock bool:lsg_secret(const c[])
{
	return containi(c, "password") >= 0 || containi(c, "rcon") >= 0 || equali(c, "sv_downloadurl") || equali(c, "log", 3) || containi(c, "_file") >= 0 || equali(c, "amx_sql", 7) || equali(c, "hostport") || equali(c, "ip")
}

stock lsg_cvar_text(const c[])
{
	new t[64]
	if (!lsg_secret(c) && get_cvar_pointer(c))
		get_cvar_string(c, t, charsmax(t))
	return t
}

stock Float:lsg_cvar_num(const c[])
	return !lsg_secret(c) && get_cvar_pointer(c) ? get_cvar_float(c) : 0.0

// 0 all, 1 people, 2 bots, 3 T, 4 CT, 5 spectators, 6 alive
stock lsg_count(which)
{
	new n
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p))
			continue
		new t = get_user_team(p)
		switch (which)
		{
			case 1: if (!is_user_bot(p)) n++
			case 2: if (is_user_bot(p)) n++
			case 3: if (t == 1) n++
			case 4: if (t == 2) n++
			case 5: if (t != 1 && t != 2) n++
			case 6: if (is_user_alive(p)) n++
			default: n++
		}
	}
	return n
}

stock lsg_clock(const fmt[])
{
	new t[40]
	get_time(fmt, t, charsmax(t))
	return t
}

stock bool:lsg_helmet(id)
{
	if (!lsg_ok(id))
		return false
	new CsArmorType:t
	cs_get_user_armor(id, t)
	return t == CS_ARMOR_VESTHELM
}

stock lsg_held(id)
{
	new w[24]
	if (lsg_ok(id) && is_user_alive(id))
	{
		new clip, ammo, wid = get_user_weapon(id, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

stock lsg_ammo(id, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return 0
	new clip, ammo
	get_user_weapon(id, clip, ammo)
	return reserve ? ammo : max(clip, 0)
}

stock lsg_ping(id)
{
	new ping, loss
	if (lsg_ok(id))
		get_user_ping(id, ping, loss)
	return ping
}

stock lsg_steam_id(id)
{
	new s[44]
	if (lsg_ok(id) && !is_user_bot(id))
	{
		get_user_authid(id, s, charsmax(s))
		if (!equal(s, "STEAM_", 6))
			s[0] = 0
	}
	return s
}

// the connected player with this Steam ID: case and the universe digit (STEAM_0 / STEAM_1) aside,
// as the app's roles compare them (only the X:Y after STEAM_n: counts)
stock lsg_by_steam(const id[])
{
	if (!equali(id, "STEAM_", 6) || !id[6] || id[7] != ':')
		return 0
	new s[44]
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		get_user_authid(p, s, charsmax(s))
		if (equali(s, "STEAM_", 6) && s[6] && s[7] == ':' && equali(s[8], id[8]))
			return p
	}
	return 0
}

// a Loner-Strike supporter: access flag "s" (gm_vip's SUPPORTER_FLAG; the app writes it from the
// roles sync). Not bots or HLTV, and not the listen-server host, who has every flag
stock bool:lsg_supporter(id)
{
	if (!lsg_ok(id) || is_user_bot(id) || is_user_hltv(id))
		return false
	if (id == 1 && !is_dedicated_server())
		return false
	return (get_user_flags(id) & ADMIN_LEVEL_G) != 0
}

stock lsg_by_name(const name[])
	return name[0] ? max(find_player("bl", name), 0) : 0

stock lsg_slot_ok(i)
	return lsg_ok(i) ? i : 0

stock Float:lsg_pevf(id, field)
{
	new Float:f
	if (lsg_ok(id))
		pev(id, field, f)
	return f
}

stock lsg_model(id)
{
	new m[32]
	if (lsg_ok(id))
		cs_get_user_model(id, m, charsmax(m))
	return m
}

stock lsg_eclass(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_classname, t, charsmax(t))
	return t
}

stock lsg_ename(e)
{
	new t[32]
	if (e > 0 && pev_valid(e))
		pev(e, pev_targetname, t, charsmax(t))
	return t
}

stock bool:lsg_plugin_on(const file[])
	return is_plugin_loaded(file, true) != -1

// (a map name is letters, digits, _ - . only, and the server must have it: nothing else reaches the console)
stock lsg_change_map(const map[])
{
	if (!map[0])
		return
	for (new i = 0; map[i]; i++)
		if (!isalnum(map[i]) && map[i] != '_' && map[i] != '-' && map[i] != '.')
			return
	if (is_map_valid(map))
		server_cmd("changelevel %s", map)
}

// text that goes into a console command's quotes: letters, digits, spaces and . , ! ? - ' ( ) only
// (no quote, semicolon, new line or other control character can end the quotes or add a command)
stock lsg_clean(const src[], dst[], len)
{
	new j
	for (new i = 0; src[i] && j < len; i++)
	{
		new c = src[i]
		if (isalnum(c) || c == ' ' || c == '.' || c == ',' || c == '!' || c == '?' || c == '-' || c == 39 || c == '(' || c == ')')
			dst[j++] = c
	}
	dst[j] = 0
}

stock lsg_slap(id, dmg)
{
	if (lsg_ok(id) && is_user_alive(id))
		user_slap(id, clamp(dmg, 0, 100))
}

stock lsg_fade(id, Float:secs, r, g, b, alpha)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenFade")
	new units = clamp(floatround(secs * 4096.0), 0, 65535)
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(units)
	write_short(units)
	write_short(0)
	write_byte(r)
	write_byte(g)
	write_byte(b)
	write_byte(alpha)
	message_end()
}

stock lsg_shake(id, Float:amp, Float:secs)
{
	if (!lsg_ok(id))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("ScreenShake")
	message_begin(MSG_ONE_UNRELIABLE, msg, _, id)
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	write_short(clamp(floatround(secs * 4096.0), 0, 65535))
	write_short(clamp(floatround(amp * 4096.0), 0, 65535))
	message_end()
}

stock lsg_godmode(id, on)
{
	if (lsg_ok(id))
		set_pev(id, pev_takedamage, on ? DAMAGE_NO : DAMAGE_AIM)
}

stock lsg_noclip(id, on)
{
	if (lsg_ok(id) && is_user_alive(id))
		set_pev(id, pev_movetype, on ? MOVETYPE_NOCLIP : MOVETYPE_WALK)
}

// 0 normal, 1 see-through by amount, 2 invisible
stock lsg_render(id, mode, amount)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_rendermode, mode ? kRenderTransAlpha : kRenderNormal)
	set_pev(id, pev_renderamt, mode == 2 ? 0.0 : (mode ? float(amount) : 255.0))
}

stock lsg_set_ammo(id, clip, reserve)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	// (the held weapon from the active item: get_user_weapon says 0 while a gun is being drawn,
	// and the reserve can't be set for weapon 0)
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
	{
		cs_set_weapon_ammo(w, clamp(clip, 0, 200))
		cs_set_user_bpammo(id, wid, clamp(reserve, 0, 255))
	}
}

stock lsg_give_ammo(id, n)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
	new wid = w > 0 && pev_valid(w) ? cs_get_weapon_id(w) : 0
	if (wid > 0 && wid != CSW_KNIFE)
		cs_set_user_bpammo(id, wid, clamp(cs_get_user_bpammo(id, wid) + n, 0, 255))
}

stock lsg_take_weapon(id, const wname[])
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new wid = get_weaponid(wname), e = -1
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", wname)) > 0)
		if (pev(e, pev_owner) == id)
			break
	if (e <= 0)
		return
	if (get_user_weapon(id) == wid)
		ExecuteHamB(Ham_Weapon_RetireWeapon, e)
	if (!ExecuteHamB(Ham_RemovePlayerItem, id, e))
		return
	ExecuteHamB(Ham_Item_Kill, e)
	set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
}

stock lsg_switch_weapon(id, const wname[])
{
	if (lsg_ok(id) && is_user_alive(id) && user_has_weapon(id, get_weaponid(wname)))
		engclient_cmd(id, wname)
}

stock lsg_set_frags(id, n)
{
	if (!lsg_ok(id))
		return
	set_pev(id, pev_frags, float(n))
	static msg
	if (!msg)
		msg = get_user_msgid("ScoreInfo")
	message_begin(MSG_ALL, msg)
	write_byte(id)
	write_short(n)
	write_short(cs_get_user_deaths(id))
	write_short(0)
	write_short(get_user_team(id))
	message_end()
}

stock lsg_set_deaths(id, n)
{
	if (lsg_ok(id))
		cs_set_user_deaths(id, max(n, 0))
}

stock lsg_to_spec(id)
{
	if (!lsg_ok(id))
		return
	if (is_user_alive(id))
		user_kill(id, 1)
	cs_set_user_team(id, CS_TEAM_SPECTATOR)
}

stock lsg_kick(id, const reason[])
{
	if (!lsg_ok(id))
		return
	new r[61]
	lsg_clean(reason, r, charsmax(r))
	server_cmd("kick #%d ^"%s^"", get_user_userid(id), r)
}

stock lsg_team_chat(team, const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 3)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && get_user_team(p) == team)
			client_print(p, print_chat, "%s", msg)
}

stock lsg_admins_chat(const fmt[], any:...)
{
	new msg[192]
	vformat(msg, charsmax(msg), fmt, 2)
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && (get_user_flags(p) & ADMIN_KICK))
			client_print(p, print_chat, "[Admins] %s", msg)
}

stock lsg_remove(e)
{
	if (e > MaxClients && pev_valid(e))
		engfunc(EngFunc_RemoveEntity, e)
}

stock lsg_ent_visible(e, on)
{
	if (e > 0 && pev_valid(e))
		set_pev(e, pev_effects, on ? (pev(e, pev_effects) & ~EF_NODRAW) : (pev(e, pev_effects) | EF_NODRAW))
}

stock lsg_sound_to(id, const snd[])
{
	if (lsg_ok(id))
		client_cmd(id, "spk ^"%s^"", snd)
}

stock lsg_fire_event(const name[], a, b)
{
	if (g_lsgFw < 0)
		g_lsgFw = CreateMultiForward("ls_studio_event", ET_IGNORE, FP_STRING, FP_CELL, FP_CELL)
	new ret
	ExecuteForward(g_lsgFw, ret, name, a, b)
}

// a public function in another plugin with up to two numbers; 1 if it was called
stock lsg_call(const plugin[], const func[], nargs, a, b, &r)
{
	r = 0
	if (callfunc_begin(func, plugin) != 1)
		return 0
	if (nargs >= 1)
		callfunc_push_int(a)
	if (nargs >= 2)
		callfunc_push_int(b)
	r = callfunc_end()
	return 1
}

stock lsg_hitgroup(v, bits)
{
	new t[12]
	if (!lsg_ok(v))
		return t
	// (only a bullet or the knife hits a place; the game keeps the last one, so a grenade or a
	// fall after a headshot would say head again)
	switch (bits & DMG_BULLET ? get_ent_data(v, "CBaseMonster", "m_LastHitGroup") : 0)
	{
		case HIT_HEAD: copy(t, charsmax(t), "head")
		case HIT_CHEST: copy(t, charsmax(t), "chest")
		case HIT_STOMACH: copy(t, charsmax(t), "stomach")
		case HIT_LEFTARM, HIT_RIGHTARM: copy(t, charsmax(t), "arm")
		case HIT_LEFTLEG, HIT_RIGHTLEG: copy(t, charsmax(t), "leg")
		default: copy(t, charsmax(t), "body")
	}
	return t
}

// the weapon that did the damage: a grenade by what hit (the attacker may hold a gun by then),
// else what the attacker holds; empty when no player did it. (Its own body: a stock that returns
// another stock's text fails at run time with "memory access".)
stock lsg_wpn_of(a, inflictor)
{
	new w[24]
	if (inflictor > MaxClients && pev_valid(inflictor))
	{
		pev(inflictor, pev_classname, w, charsmax(w))
		if (equal(w, "grenade"))
		{
			copy(w, charsmax(w), "hegrenade")
			return w
		}
		w[0] = 0
	}
	if (lsg_ok(a) && is_user_alive(a))
	{
		new clip, ammo, wid = get_user_weapon(a, clip, ammo)
		if (wid > 0)
			get_weaponname(wid, w, charsmax(w))
		replace(w, charsmax(w), "weapon_", "")
	}
	return w
}

// Give Weapon with a name wired in: only a weapon the game has (ak47, deagle, knife...)
stock lsg_give_named(id, const name[], ammo)
{
	new w[32]
	formatex(w, charsmax(w), "weapon_%s", name)
	if (equal(name, "weapon_", 7) || !get_weaponid(w))
		return 0
	return ls_give_weapon(id, w, ammo)
}

stock Float:lsg_dist(a, b)
{
	if (!lsg_ok(a) || !lsg_ok(b))
		return 0.0
	new Float:x[3], Float:y[3]
	pev(a, pev_origin, x)
	pev(b, pev_origin, y)
	return get_distance_f(x, y)
}

// what's after the first word ("/give 5" -> "5")
stock lsg_after_word(s[])
{
	new i
	while (s[i] && s[i] != ' ')
		i++
	while (s[i] == ' ')
		i++
	copy(s, 63, s[i])
}

stock lsg_hook_weapons(const fn[], Ham:h)
{
	static const W[][] = { "weapon_p228", "weapon_scout", "weapon_hegrenade", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_smokegrenade", "weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_flashbang", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_knife", "weapon_p90" }
	for (new i = 0; i < sizeof W; i++)
		RegisterHam(h, W[i], fn, 1)
}

stock lsg_prop(at, const model[], solid)
{
	new Float:o[3]
	if (!lsg_ent_origin(at, o))
		return 0
	// (a model that wasn't there when the map loaded: no prop)
	if (!lsg_have(model))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_prop")
	engfunc(EngFunc_SetModel, e, model)
	engfunc(EngFunc_SetOrigin, e, o)
	set_pev(e, pev_solid, solid ? SOLID_BBOX : SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	if (solid)
		engfunc(EngFunc_SetSize, e, Float:{-16.0, -16.0, 0.0}, Float:{16.0, 16.0, 32.0})
	return e
}

// the round table's kind of round (0 normal, 1 boss, 2 pack) and its type
stock lsg_wave_kind()
{
	new t[32]
	return ls_components_running() ? ls_wave_kind(t, charsmax(t)) : 0
}

stock lsg_wave_type()
{
	new t[32]
	if (ls_components_running())
		ls_wave_kind(t, charsmax(t))
	return t
}

// a drop-in over an entity's place
stock lsg_drop_in(id, ent)
{
	new Float:o[3]
	if (!lsg_ent_origin(ent, o))
		return ls_drop_in(id)
	return ls_drop_in(id, o)
}

stock Float:lsg_ehp(e)
	return e > MaxClients && pev_valid(e) ? entity_get_float(e, EV_FL_health) : 0.0

// ---- bot hints (ls_mode, through gm_components) ----
stock bool:lsg_hint(const line[])
	return ls_components_running() && ls_comp_bots_hint(line) != 0

// who a hint is for: 0 all, 1 ct, 2 t, 3 a player, 4 @tag
stock lsg_who(code, p, const tag[])
{
	new w[40]
	switch (code)
	{
		case 1: copy(w, charsmax(w), "ct")
		case 2: copy(w, charsmax(w), "t")
		case 3: formatex(w, charsmax(w), "%d", lsg_ok(p) ? p : 0)
		case 4: formatex(w, charsmax(w), "@%s", tag)
		default: copy(w, charsmax(w), "all")
	}
	return w
}

// an entity's place as "x y z" (a brush entity: its middle)
stock lsg_ptext(ent)
{
	new t[48], Float:o[3]
	if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
		{
			new Float:mins[3], Float:maxs[3]
			pev(ent, pev_absmin, mins)
			pev(ent, pev_absmax, maxs)
			for (new k = 0; k < 3; k++)
				o[k] = (mins[k] + maxs[k]) / 2.0
		}
	}
	formatex(t, charsmax(t), "%.0f %.0f %.0f", o[0], o[1], o[2])
	return t
}

// the zone's next move: 0 seconds (-1 none), 1 the next radius, 2 closing now
stock Float:lsg_zone_next(ent, what)
{
	if (!ls_components_running() || ent <= 0)
		return what == 0 ? -1.0 : 0.0
	new Float:c[3], Float:r, closing
	new Float:secs = ls_zone_next(ent, c, r, closing)
	switch (what)
	{
		case 1: return r
		case 2: return closing ? 1.0 : 0.0
	}
	return secs
}

// is ent a component of this kind ("": any component)? gm_components' natives raise an error on
// another kind, which ends the whole flow; the node checks first and says so instead
stock bool:lsg_kind_ok(ent, const kind[])
{
	new k[24]
	if (ent <= 0 || !ls_components_running() || !ls_comp_kind(ent, k, charsmax(k)))
		return false
	return !kind[0] || equal(k, kind)
}

// the safe zone a Zone pin means: a Safe zone itself, or the one a Shrinking zone drives (0: neither)
stock lsg_zone_of(ent)
{
	new k[24], name[32], tn[32], z
	if (ent <= 0 || !ls_comp_kind(ent, k, charsmax(k)))
		return 0
	if (equal(k, "ls_safe_zone"))
		return ent
	if (!equal(k, "ls_zone_shrinker"))
		return 0
	ls_comp_get(ent, "zone", name, charsmax(name))
	while ((z = ls_comp_find("ls_safe_zone", z)) > 0)
	{
		pev(z, pev_targetname, tn, charsmax(tn))
		if (!name[0] || equal(tn, name))
			return z
	}
	return 0
}

// counted now (the brain's ls_creature_alive is only brought up to date a moment later); once a
// server frame (one scan however often it's read), again after the mode makes a creature
stock Float:g_lsgCrAt = -1.0, g_lsgCrN
stock lsg_creatures_alive()
{
	if (get_gametime() == g_lsgCrAt)
		return g_lsgCrN
	new n, e = MaxClients, Float:hp
	while ((e = engfunc(EngFunc_FindEntityByString, e, "classname", "ls_creature")) > 0)
	{
		pev(e, pev_health, hp)
		if (pev(e, pev_deadflag) == DEAD_NO && hp > 0.0)
			n++
	}
	g_lsgCrAt = get_gametime()
	g_lsgCrN = n
	return n
}

stock lsg_slot(id)
	return id >= 1 && id <= MaxClients ? id : 0

stock bool:lsg_ok(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id)

stock bool:lsg_match(p, which)
{
	if (!is_user_connected(p))
		return false
	switch (which)
	{
		case 1: return bool:is_user_alive(p)
		case 2: return !is_user_alive(p) && (lsg_team(p) == 1 || lsg_team(p) == 2)
		case 3: return lsg_team(p) == 1
		case 4: return lsg_team(p) == 2
		case 5: return is_user_alive(p) && lsg_team(p) == 1
		case 6: return is_user_alive(p) && lsg_team(p) == 2
		case 7: return bool:is_user_bot(p)
		case 8: return !is_user_bot(p)
	}
	return true
}

stock lsg_end_round(team)
{
	if (team == 1)
		server_cmd("endround T")
	else if (team == 2)
		server_cmd("endround CT")
	else
		server_cmd("endround")
}

stock bool:lsg_use(const name[])
{
	new ent = -1, used
	while (name[0] && used < 64 && (ent = find_ent_by_tname(ent, name)) > 0)
	{
		ExecuteHamB(Ham_Use, ent, 0, 0, 2, 1.0)
		used++
	}
	return used > 0
}

stock lsg_bar(id, secs)
{
	if (id && !lsg_ok(id))
		return
	message_begin(id ? MSG_ONE : MSG_ALL, get_user_msgid("BarTime"), _, id)
	write_short(secs)
	message_end()
}

stock lsg_add_money(id, amount)
{
	if (pev_valid(id) == 2)
		cs_set_user_money(id, clamp(cs_get_user_money(id) + amount, 0, 16000), 1)
}

stock Float:lsg_get_money(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_money(id)) : 0.0

stock lsg_add_frags(id, amount)
{
	set_user_frags(id, get_user_frags(id) + amount)
	// (setting the deaths again sends the scoreboard its new numbers)
	if (pev_valid(id) == 2)
		cs_set_user_deaths(id, cs_get_user_deaths(id))
}

stock Float:lsg_get_frags(id)
	return lsg_ok(id) ? float(get_user_frags(id)) : 0.0

stock Float:lsg_get_deaths(id)
	return lsg_ok(id) && pev_valid(id) == 2 ? float(cs_get_user_deaths(id)) : 0.0

stock lsg_freeze(id, on)
{
	new flags = pev(id, pev_flags)
	set_pev(id, pev_flags, on ? (flags | FL_FROZEN) : (flags & ~FL_FROZEN))
}

// a bot's nature as the bot brain plays it (W2: pev_vuser2 = skill, aggression, teamwork 0-1;
// pev_vuser3 = greed 0-1, playstyle, trait bits); people read 0
stock Float:lsg_bot_nat(id, which)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return 0.0
	new Float:v[3]
	pev(id, which == 3 ? pev_vuser3 : pev_vuser2, v)
	return floatclamp((which == 3 ? v[0] : v[which]) * 100.0, 0.0, 100.0)
}

stock bool:lsg_bot_trait(id, bit)
{
	if (!lsg_ok(id) || !is_user_bot(id))
		return false
	new Float:v[3]
	pev(id, pev_vuser3, v)
	return (floatround(v[2]) & bit) != 0
}

stock lsg_bot_style(id)
{
	static const S[][] = { "rusher", "roamer", "lurker", "careful", "camper", "awper", "knifer", "nader", "ninja", "objective", "weirdgun" }
	new t[16]
	if (lsg_ok(id) && is_user_bot(id))
	{
		new Float:v[3]
		pev(id, pev_vuser3, v)
		new k = floatround(v[1])
		if (k >= 0 && k < sizeof S)
			copy(t, charsmax(t), S[k])
	}
	return t
}

// a follow with a personality influence (0-100: 50 as usual; W2's ls_mode_ver 2)
stock lsg_bot_follow(bot, who, secs, influence)
{
	if (ls_is_bot(bot) && is_user_connected(who))
		server_cmd("ls_bot %d follow %d %d %d", bot, who, secs, influence)
}

stock lsg_bot_goto(bot, const name[], secs)
{
	new Float:o[3]
	if (!lsg_origin_of(name, o))
		return
	ls_bot_goto(bot, o)
	// a timed goal: the bot is free again after it
	remove_task(LSG_TASK - 100 - bot)
	if (secs > 0)
		set_task(float(secs), "lsg_bot_free", LSG_TASK - 100 - bot)
}

public lsg_bot_free(task)
	ls_bot_release(LSG_TASK - 100 - task)

// Drop Weapon: a person drops it; a bot loses it (a bot's "drop" can crash this game's DLL)
stock lsg_drop(id)
{
	if (!is_user_bot(id))
	{
		engclient_cmd(id, "drop")
		return
	}
	new wid = get_user_weapon(id)
	if (!wid || wid == CSW_KNIFE)
		return
	new name[32]
	get_weaponname(wid, name, charsmax(name))
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", name)) > 0)
	{
		if (pev(ent, pev_owner) != id)
			continue
		ExecuteHamB(Ham_Weapon_RetireWeapon, ent)
		if (ExecuteHamB(Ham_RemovePlayerItem, id, ent))
		{
			set_pev(id, pev_weapons, pev(id, pev_weapons) & ~(1 << wid))
			ExecuteHamB(Ham_Item_Kill, ent)
		}
		break
	}
}

stock Float:lsg_mod(Float:a, Float:b)
{
	if (b == 0.0)
		return 0.0
	return a - b * float(floatround(a / b, floatround_floor))
}

stock Float:lsg_pow(Float:a, Float:b)
{
	new Float:r = floatpower(a, b)
	if (r != r)
		return 0.0
	return floatclamp(r, -100000.0, 100000.0)
}

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
	return id >= 1 && id <= MaxClients && is_user_connected(id) && pev_valid(id) == 2 ? (_:cs_get_user_team(id)) : 0

stock lsg_is_bot(id)
	return id >= 1 && id <= MaxClients && is_user_connected(id) && is_user_bot(id) ? 1 : 0

// a number as text: whole numbers as they are, others with two decimals (0.5, not 1)
stock lsg_ntext(Float:n)
{
	new t[24]
	if (floatfract(n) == 0.0 && floatabs(n) < 1000000.0)
		formatex(t, charsmax(t), "%d", floatround(n))
	else
		formatex(t, charsmax(t), "%.2f", n)
	return t
}

stock lsg_name(id)
{
	new name[32]
	if (id >= 1 && id <= MaxClients && is_user_connected(id))
		get_user_name(id, name, charsmax(name))
	else
		copy(name, charsmax(name), "nobody")
	return name
}

// (a scan of every entity: the answer is kept for the rest of the server frame, so a Find in a
// loop or read many times costs one scan; PerfAudit #9)
stock lsg_find(const name[])
{
	static Trie:seen = Invalid_Trie, Float:at = -1.0
	if (!name[0])
		return 0
	new Float:now = get_gametime(), e
	if (seen == Invalid_Trie)
		seen = TrieCreate()
	if (now != at)
	{
		TrieClear(seen)
		at = now
	}
	if (TrieGetCell(seen, name, e) && (!e || pev_valid(e)))
		return e
	e = max(find_ent_by_tname(-1, name), 0)
	TrieSetCell(seen, name, e)
	return e
}

stock bool:lsg_named(ent, const name[])
{
	new tn[32]
	pev(ent, pev_targetname, tn, charsmax(tn))
	return bool:equal(tn, name)
}

stock bool:lsg_origin_of(const name[], Float:o[3])
{
	// "#12": entity 12 (an entity wired in, not a name)
	if (name[0] == '#')
		return lsg_ent_origin(str_to_num(name[1]), o)
	return lsg_ent_origin(lsg_find(name), o)
}

stock bool:lsg_ent_origin(ent, Float:o[3])
{
	if (ent <= 0 || !pev_valid(ent))
		return false
	pev(ent, pev_origin, o)
	// (a brush entity's origin is often 0 0 0: its middle instead)
	if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new k = 0; k < 3; k++)
			o[k] = (mins[k] + maxs[k]) / 2.0
	}
	return true
}

stock bool:lsg_teleport_to(id, ent)
{
	new Float:o[3]
	if (!ls_alive(id) || !lsg_ent_origin(ent, o))
		return false
	ls_teleport(id, o)
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
	return lsg_creature_o(kind, o)
}

/// (an entity's name for lsg_creature: "" picks no entity, so its origin is looked up as #index)
stock lsg_ent_find_origin(ent)
{
	new s[16]
	formatex(s, charsmax(s), "#%d", ent)
	return s
}

stock lsg_creature_o(const kind[], const Float:o[3])
{
	if (!get_cvar_pointer("ls_creature_ver"))
		return 0
	new ent = create_entity("ls_creature")
	if (ent <= 0)
		return 0
	DispatchKeyValue(ent, "type", kind)
	entity_set_origin(ent, o)
	DispatchSpawn(ent)
	return is_valid_ent(ent) && !(entity_get_int(ent, EV_INT_flags) & FL_KILLME) ? ent : 0
}

stock lsg_dmg_kind(bits, out[], len)
{
	if (bits & DMG_FALL)
		copy(out, len, "fall")
	else if (bits & DMG_BLAST)
		copy(out, len, "blast")
	else if (bits & DMG_BURN)
		copy(out, len, "burn")
	else if (bits & DMG_BULLET)
		copy(out, len, "shot")
	else if (bits & DMG_SLASH)
		copy(out, len, "slash")
	else
		copy(out, len, "other")
}

stock lsg_launch(id, Float:speed)
{
	new Float:v[3]
	pev(id, pev_velocity, v)
	v[2] = speed
	set_pev(id, pev_velocity, v)
}

stock lsg_round_time(secs)
{
	message_begin(MSG_ALL, get_user_msgid("RoundTime"))
	write_short(max(secs, 0))
	message_end()
}
// ---- Status Icon, Objective Marker, Kill Feed Line ----
stock g_lsgIcon[33][16]
stock lsg_icon_send(to, status, const name[], r, g, b)
{
	if ((to && !is_user_connected(to)) || !lsg_msg_room("Status Icon", 1, to))
		return
	static msg
	if (!msg)
		msg = get_user_msgid("StatusIcon")
	message_begin(to ? MSG_ONE : MSG_ALL, msg, _, to)
	write_byte(status)
	write_string(name)
	if (status)
	{
		write_byte(r)
		write_byte(g)
		write_byte(b)
	}
	message_end()
}

// to 0: everyone; death: noted, so it's taken away when they die
stock lsg_icon(to, status, const name[], r, g, b, death)
{
	if (to && !lsg_ok(to))
		return
	lsg_icon_send(to, status, name, r, g, b)
	if (!death)
		return
	for (new p = to ? to : 1; p <= (to ? to : MaxClients); p++)
		if (is_user_connected(p))
			copy(g_lsgIcon[p], 15, status ? name : "")
}

stock lsg_icon_clear(id)
{
	if (id < 1 || id > MaxClients || !g_lsgIcon[id][0])
		return
	lsg_icon_send(id, 0, g_lsgIcon[id], 0, 0, 0)
	g_lsgIcon[id][0] = 0
}

// markers: a sprite on an info_target, moved by one task to what it follows (no think each frame)
#define LSG_MKS 256
stock g_lsgMk[LSG_MKS], g_lsgMkF[LSG_MKS], Float:g_lsgMkH[LSG_MKS], g_lsgMkWho[2048], g_lsgMkFor[2048]

stock lsg_marker(const spr[], follow, const Float:at[3], hasAt, Float:h, Float:scale, r, g, b, pulse, who, forp, lim = 32)
{
	// (a sprite that wasn't there when the map loaded: no marker)
	if (!lsg_have(spr) || (follow <= 0 && !hasAt) || (follow > 0 && !pev_valid(follow)))
		return 0
	new slot = -1
	for (new k = 0; k < min(lim, LSG_MKS); k++)
	{
		if (g_lsgMk[k] && !pev_valid(g_lsgMk[k]))
			g_lsgMk[k] = 0
		if (!g_lsgMk[k])
		{
			slot = k
			break
		}
	}
	if (slot < 0 || !lsg_ent_room("Objective Marker"))
		return 0
	new e = create_entity("info_target")
	if (e <= 0)
		return 0
	set_pev(e, pev_classname, "ls_studio_marker")
	engfunc(EngFunc_SetModel, e, spr)
	set_pev(e, pev_movetype, MOVETYPE_NOCLIP)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_rendermode, kRenderTransAdd)
	set_pev(e, pev_renderamt, 255.0)
	new Float:c[3]
	c[0] = float(r)
	c[1] = float(g)
	c[2] = float(b)
	set_pev(e, pev_rendercolor, c)
	set_pev(e, pev_scale, scale)
	if (pulse)
		set_pev(e, pev_renderfx, kRenderFxPulseSlow)
	new Float:o[3]
	if (follow > 0)
		pev(follow, pev_origin, o)
	else
		o = at
	o[2] += h
	engfunc(EngFunc_SetOrigin, e, o)
	g_lsgMk[slot] = e
	g_lsgMkF[slot] = follow
	g_lsgMkH[slot] = h
	g_lsgMkWho[e] = who
	g_lsgMkFor[e] = forp
	return e
}

stock lsg_mk_free(k)
{
	new e = g_lsgMk[k]
	g_lsgMk[k] = 0
	if (e > 0 && e < 2048)
	{
		g_lsgMkWho[e] = 0
		g_lsgMkFor[e] = 0
	}
	if (e > 0 && pev_valid(e))
		remove_entity(e)
}

stock lsg_mk_follow()
{
	for (new k = 0; k < LSG_MKS; k++)
	{
		new e = g_lsgMk[k], f = g_lsgMkF[k]
		if (!e)
			continue
		if (!pev_valid(e))
		{
			g_lsgMk[k] = 0
			continue
		}
		if (f <= 0)
			continue
		// what it was on is gone (or the player left): so is the marker
		if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
		{
			lsg_mk_free(k)
			continue
		}
		// (a dead player's marker waits, out of sight)
		new fx = pev(e, pev_effects)
		if (f <= MaxClients && !is_user_alive(f))
		{
			set_pev(e, pev_effects, fx | EF_NODRAW)
			continue
		}
		set_pev(e, pev_effects, fx & ~EF_NODRAW)
		new Float:o[3]
		pev(f, pev_origin, o)
		o[2] += g_lsgMkH[k]
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

// 0 or a marker: that one, or all of them
stock lsg_marker_remove(m)
{
	for (new k = 0; k < LSG_MKS; k++)
		if (g_lsgMk[k] && (!m || g_lsgMk[k] == m))
			lsg_mk_free(k)
}

stock bool:lsg_mk_sees(ent, host)
{
	new w = g_lsgMkWho[ent]
	if (w == 3)
		return host == g_lsgMkFor[ent]
	return host >= 1 && host <= MaxClients && get_user_team(host) == w
}

// a line in the kill feed, to each person's game (one at a time: plugins that count deaths from
// the game's own DeathMsg, the mode's Player Killed among them, don't see it as a death)
stock lsg_killfeed(killer, victim, hs, const w[])
{
	if (!lsg_ok(victim))
		return 0
	if (!lsg_ok(killer))
		killer = 0
	static msg
	if (!msg)
		msg = get_user_msgid("DeathMsg")
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p) || !lsg_msg_room("Kill Feed Line", 1, p))
			continue
		message_begin(MSG_ONE, msg, _, p)
		write_byte(killer)
		write_byte(victim)
		write_byte(hs ? 1 : 0)
		write_string(w)
		message_end()
	}
	return 1
}
// ---- fun: fire, trails, bursts, swaps, burying, the camera, a drunk view ----
stock g_lsgSprFire, g_lsgSprSmoke, g_lsgSprBlood, g_lsgSprSpray
stock Float:g_lsgBurnTill[33], Float:g_lsgBurnDps[33], Float:g_lsgBurnNext[33], g_lsgBurnBy[33], g_lsgBurnFx[33]
stock bool:g_lsgBuried[33], g_lsgCam[33], g_lsgCamKeep[33], Float:g_lsgDrunkTill[33], Float:g_lsgDrunk[33], g_lsgFunN
#define LSG_TRAILS 256
stock g_lsgTrail[LSG_TRAILS], g_lsgTrailC[LSG_TRAILS][5], bool:g_lsgTrailStill[LSG_TRAILS]

stock lsg_burn(id, by, Float:secs, Float:dps)
{
	if (!ls_alive(id))
		return 0
	new Float:now = get_gametime()
	g_lsgBurnTill[id] = now + secs
	g_lsgBurnDps[id] = dps
	g_lsgBurnBy[id] = lsg_ok(by) ? by : 0
	if (g_lsgBurnNext[id] < now)
		g_lsgBurnNext[id] = now + 1.0
	// flames on them: a sprite that rides along (the engine moves it: MOVETYPE_FOLLOW)
	if (g_lsgSprFire && !(g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id])) && lsg_ent_room("Set On Fire's flames"))
	{
		new e = create_entity("info_target")
		if (e > 0)
		{
			set_pev(e, pev_classname, "ls_studio_fire")
			engfunc(EngFunc_SetModel, e, "sprites/fire.spr")
			set_pev(e, pev_movetype, MOVETYPE_FOLLOW)
			set_pev(e, pev_aiment, id)
			set_pev(e, pev_solid, SOLID_NOT)
			set_pev(e, pev_rendermode, kRenderTransAdd)
			set_pev(e, pev_renderamt, 255.0)
			g_lsgBurnFx[id] = e
		}
	}
	return 1
}

stock lsg_burn_off(id)
{
	if (id < 1 || id > 32)
		return
	g_lsgBurnTill[id] = 0.0
	if (g_lsgBurnFx[id] && pev_valid(g_lsgBurnFx[id]))
		remove_entity(g_lsgBurnFx[id])
	g_lsgBurnFx[id] = 0
}

stock lsg_drunk(id, Float:secs, Float:strength)
{
	if (!ls_alive(id))
		return 0
	g_lsgDrunkTill[id] = get_gametime() + secs
	g_lsgDrunk[id] = strength
	return 1
}

stock lsg_drunk_off(id)
{
	if (g_lsgDrunkTill[id] > 0.0 && is_user_connected(id))
		set_pev(id, pev_punchangle, Float:{0.0, 0.0, 0.0})
	g_lsgDrunkTill[id] = 0.0
}

stock lsg_trail_send(k)
{
	new e = g_lsgTrail[k]
	if (!lsg_msg_room("Trail", 2))
		return
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_KILLBEAM)
	write_short(e)
	message_end()
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMFOLLOW)
	write_short(e)
	write_short(g_lsgSprBeam)
	write_byte(g_lsgTrailC[k][4])
	write_byte(g_lsgTrailC[k][3])
	write_byte(g_lsgTrailC[k][0])
	write_byte(g_lsgTrailC[k][1])
	write_byte(g_lsgTrailC[k][2])
	write_byte(200)
	message_end()
}

stock lsg_trail(e, on, r, g, b, width, life, lim = 32)
{
	if (e <= 0 || !pev_valid(e))
		return 0
	new k = -1
	for (new i = 0; i < LSG_TRAILS; i++)
		if (g_lsgTrail[i] == e)
			k = i
	if (!on)
	{
		if (k >= 0)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_KILLBEAM)
			write_short(e)
			message_end()
			g_lsgTrail[k] = 0
		}
		return 1
	}
	for (new i = 0; i < min(lim, LSG_TRAILS) && k < 0; i++)
		if (!g_lsgTrail[i] || !pev_valid(g_lsgTrail[i]))
			k = i
	if (k < 0)
		return 0
	g_lsgTrail[k] = e
	g_lsgTrailC[k][0] = r
	g_lsgTrailC[k][1] = g
	g_lsgTrailC[k][2] = b
	g_lsgTrailC[k][3] = width
	g_lsgTrailC[k][4] = life
	g_lsgTrailStill[k] = false
	lsg_trail_send(k)
	return 1
}

// a trail dies with a teleport, a respawn or a long stop: sent again when they move once more
stock lsg_trail_check()
{
	for (new k = 0; k < LSG_TRAILS; k++)
	{
		new e = g_lsgTrail[k]
		if (!e)
			continue
		if (!pev_valid(e) || (e <= MaxClients && !is_user_connected(e)))
		{
			g_lsgTrail[k] = 0
			continue
		}
		new Float:vel[3]
		pev(e, pev_velocity, vel)
		if (vector_length(vel) < 1.0)
			g_lsgTrailStill[k] = true
		else if (g_lsgTrailStill[k])
		{
			g_lsgTrailStill[k] = false
			lsg_trail_send(k)
		}
	}
}

stock lsg_fun_ticks()
{
	new Float:now = get_gametime()
	lsg_view_tick(now)
	for (new id = 1; id <= MaxClients; id++)
	{
		if (g_lsgBurnTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgBurnTill[id] || pev(id, pev_waterlevel) >= 2)
				lsg_burn_off(id)
			else if (now >= g_lsgBurnNext[id])
			{
				g_lsgBurnNext[id] = now + 1.0
				new by = g_lsgBurnBy[id]
				if (by && !is_user_connected(by))
					by = 0
				if (g_lsgBurnDps[id] > 0.0)
					ExecuteHamB(Ham_TakeDamage, id, by ? by : 0, by ? by : 0, g_lsgBurnDps[id], DMG_BURN)
			}
		}
		if (g_lsgDrunkTill[id] > 0.0)
		{
			if (!is_user_alive(id) || now >= g_lsgDrunkTill[id])
				lsg_drunk_off(id)
			else
			{
				new Float:s = g_lsgDrunk[id], Float:pa[3]
				pa[0] = floatsin(now * 2.3, radian) * s * 2.0
				pa[1] = floatcos(now * 1.7, radian) * s * 2.0
				pa[2] = floatsin(now * 1.1, radian) * s
				set_pev(id, pev_punchangle, pa)
			}
		}
	}
	if (++g_lsgFunN >= 10)
	{
		g_lsgFunN = 0
		lsg_trail_check()
	}
}

// the hull a player has now fits at o (nothing solid there)
stock bool:lsg_fits(id, const Float:o[3])
{
	engfunc(EngFunc_TraceHull, o, o, IGNORE_MONSTERS, (pev(id, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN, id, 0)
	return !get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid)
}

stock lsg_swap(a, b)
{
	if (!ls_alive(a) || !ls_alive(b) || a == b || g_lsgBuried[a] || g_lsgBuried[b])
		return 0
	new Float:oa[3], Float:ob[3], Float:aa[3], Float:ab[3], Float:va[3], Float:vb[3]
	pev(a, pev_origin, oa)
	pev(b, pev_origin, ob)
	if (!lsg_fits(a, ob) || !lsg_fits(b, oa))
		return 0
	pev(a, pev_v_angle, aa)
	pev(b, pev_v_angle, ab)
	pev(a, pev_velocity, va)
	pev(b, pev_velocity, vb)
	engfunc(EngFunc_SetOrigin, a, ob)
	engfunc(EngFunc_SetOrigin, b, oa)
	set_pev(a, pev_angles, ab)
	set_pev(a, pev_v_angle, ab)
	set_pev(a, pev_fixangle, 1)
	set_pev(b, pev_angles, aa)
	set_pev(b, pev_v_angle, aa)
	set_pev(b, pev_fixangle, 1)
	set_pev(a, pev_velocity, vb)
	set_pev(b, pev_velocity, va)
	return 1
}

// chest deep: 30 units down, and no walking (they can still turn and shoot)
stock lsg_bury(id)
{
	if (!ls_alive(id) || g_lsgBuried[id])
		return 0
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] -= 30.0
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_velocity, Float:{0.0, 0.0, 0.0})
	set_pev(id, pev_movetype, MOVETYPE_NONE)
	g_lsgBuried[id] = true
	return 1
}

stock lsg_unbury(id)
{
	if (!lsg_ok(id) || !g_lsgBuried[id])
		return 0
	g_lsgBuried[id] = false
	if (!is_user_alive(id))
		return 1
	set_pev(id, pev_movetype, MOVETYPE_WALK)
	new Float:o[3]
	pev(id, pev_origin, o)
	o[2] += 30.0
	// (onto the floor, never into a wall: a little higher until they fit)
	for (new i = 0; i < 8 && !lsg_fits(id, o); i++)
		o[2] += 8.0
	engfunc(EngFunc_SetOrigin, id, o)
	return 1
}

// view: 0 first person, 1 third person, 2 the other one; reset: back to first person at death/respawn
stock lsg_camera(id, view, reset)
{
	if (!lsg_ok(id))
		return 0
	new third = view == 2 ? (g_lsgCam[id] ? 0 : 1) : view
	set_view(id, third ? CAMERA_3RDPERSON : CAMERA_NONE)
	g_lsgCam[id] = third
	g_lsgCamKeep[id] = reset ? 0 : 1
	return 1
}

stock lsg_cam_reapply(id)
{
	if (lsg_ok(id) && g_lsgCam[id])
		set_view(id, CAMERA_3RDPERSON)
}

stock lsg_fun_died(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_view_back(id)
	lsg_burn_off(id)
	lsg_drunk_off(id)
	g_lsgBuried[id] = false
	if (g_lsgCam[id] && !g_lsgCamKeep[id])
	{
		set_view(id, CAMERA_NONE)
		g_lsgCam[id] = 0
	}
}

stock lsg_fun_spawned(id)
{
	if (id < 1 || id > MaxClients || !is_user_alive(id))
		return
	g_lsgBuried[id] = false
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			lsg_trail_send(k)
	if (g_lsgCam[id])
	{
		// (kept: the game may put the view back as they spawn, so it's set again a moment later)
		if (g_lsgCamKeep[id])
			set_task(0.2, "lsg_cam_again", LSG_TASK - 480 + id)
		else
		{
			set_view(id, CAMERA_NONE)
			g_lsgCam[id] = 0
		}
	}
}

stock lsg_fun_left(id)
{
	if (id < 1 || id > MaxClients)
		return
	lsg_view_back(id)
	lsg_burn_off(id)
	g_lsgDrunkTill[id] = 0.0
	g_lsgBuried[id] = false
	g_lsgCam[id] = 0
	g_lsgCamKeep[id] = 0
	remove_task(LSG_TASK - 480 + id)
	for (new k = 0; k < 32; k++)
		if (g_lsgTrail[k] == id)
			g_lsgTrail[k] = 0
}

// effect: 0 sparks, 1 smoke, 2 blood, 3 firework, 4 implosion, 5 tar, 6 teleport, 7 lava, 8 a colored flash
stock lsg_burst(kind, const Float:o[3], count, r, g, b)
{
	// (smoke and blood need their sprites: not there when the map loaded, no burst)
	if ((kind == 1 && !g_lsgSprSmoke) || (kind == 2 && (!g_lsgSprBlood || !g_lsgSprSpray)))
		return
	for (new n = 0; n < count; n++)
	{
		message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
		switch (kind)
		{
			case 1:
			{
				write_byte(TE_SMOKE)
				lsg_coords(o)
				write_short(g_lsgSprSmoke)
				write_byte(20)
				write_byte(12)
			}
			case 2:
			{
				write_byte(TE_BLOODSPRITE)
				lsg_coords(o)
				write_short(g_lsgSprSpray)
				write_short(g_lsgSprBlood)
				write_byte(247)
				write_byte(12)
			}
			case 3:
			{
				write_byte(TE_EXPLOSION2)
				lsg_coords(o)
				write_byte(random_num(0, 200))
				write_byte(16)
			}
			case 4:
			{
				write_byte(TE_IMPLOSION)
				lsg_coords(o)
				write_byte(128)
				write_byte(20)
				write_byte(5)
			}
			case 5:
			{
				write_byte(TE_TAREXPLOSION)
				lsg_coords(o)
			}
			case 6:
			{
				write_byte(TE_TELEPORT)
				lsg_coords(o)
			}
			case 7:
			{
				write_byte(TE_LAVASPLASH)
				lsg_coords(o)
			}
			case 8:
			{
				write_byte(TE_DLIGHT)
				lsg_coords(o)
				write_byte(30)
				write_byte(r)
				write_byte(g)
				write_byte(b)
				write_byte(8)
				write_byte(30)
			}
			default:
			{
				write_byte(TE_SPARKS)
				lsg_coords(o)
			}
		}
		message_end()
		// (a firework's light, in its color)
		if (kind == 3)
		{
			message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
			write_byte(TE_DLIGHT)
			lsg_coords(o)
			write_byte(25)
			write_byte(r)
			write_byte(g)
			write_byte(b)
			write_byte(6)
			write_byte(20)
			message_end()
		}
	}
}

stock lsg_coords(const Float:o[3])
{
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
}

// View From: a camera entity each shot; players see through it (set_view) until it ends
stock g_lsgViewCam[33], Float:g_lsgViewTill[33], bool:g_lsgViewFreeze[33], g_lsgCamFollow[2048], Float:g_lsgCamOff[2048], Float:g_lsgCamStop[2048]

stock lsg_view_from(who, one, from, const Float:a[3], const Float:aa[3], hasA, const Float:b[3], const Float:ba[3], hasB, Float:off, Float:move, Float:secs, freeze)
{
	if (from <= 0 && !hasA)
		return 0
	if (from > 0 && !pev_valid(from))
		return 0
	if (!lsg_ent_room("View From"))
		return 0
	new cam = create_entity("info_target")
	if (cam <= 0)
		return 0
	set_pev(cam, pev_classname, "ls_studio_cam")
	engfunc(EngFunc_SetModel, cam, "models/w_c4.mdl")
	set_pev(cam, pev_rendermode, kRenderTransTexture)
	set_pev(cam, pev_renderamt, 0.0)
	set_pev(cam, pev_solid, SOLID_NOT)
	set_pev(cam, pev_movetype, MOVETYPE_NOCLIP)
	new Float:o[3], Float:ang[3]
	if (from > 0)
	{
		pev(from, pev_origin, o)
		o[2] += off
		pev(from, from <= MaxClients ? pev_v_angle : pev_angles, ang)
		g_lsgCamFollow[cam] = from
		g_lsgCamOff[cam] = off
	}
	else
	{
		o = a
		ang = aa
		g_lsgCamFollow[cam] = 0
	}
	engfunc(EngFunc_SetOrigin, cam, o)
	set_pev(cam, pev_angles, ang)
	set_pev(cam, pev_v_angle, ang)
	g_lsgCamStop[cam] = 0.0
	// a glide from A to B: the engine moves it (velocity, angular velocity); stopped when it's there
	if (from <= 0 && hasB && move > 0.0)
	{
		new Float:vel[3], Float:av[3]
		for (new k = 0; k < 3; k++)
		{
			vel[k] = (b[k] - a[k]) / move
			av[k] = (ba[k] - aa[k]) / move
		}
		set_pev(cam, pev_velocity, vel)
		set_pev(cam, pev_avelocity, av)
		g_lsgCamStop[cam] = get_gametime() + move
	}
	new n = 0
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p) || is_user_bot(p))
			continue
		if ((who == 0 && p != one) || (who == 1 && get_user_team(p) != 1) || (who == 2 && get_user_team(p) != 2))
			continue
		lsg_view_back(p)
		g_lsgViewCam[p] = cam
		g_lsgViewTill[p] = secs > 0.0 ? get_gametime() + secs : 0.0
		g_lsgViewFreeze[p] = freeze != 0
		if (freeze)
			set_pev(p, pev_flags, pev(p, pev_flags) | FL_FROZEN)
		engfunc(EngFunc_SetView, p, cam)
		n++
	}
	if (!n)
		remove_entity(cam)
	return n
}

// one player's view back: first or third person as Set Camera left it
stock lsg_view_back(p)
{
	new cam = g_lsgViewCam[p]
	if (!cam)
		return
	g_lsgViewCam[p] = 0
	g_lsgViewTill[p] = 0.0
	if (is_user_connected(p))
	{
		if (g_lsgViewFreeze[p])
			set_pev(p, pev_flags, pev(p, pev_flags) & ~FL_FROZEN)
		engfunc(EngFunc_SetView, p, p)
		if (g_lsgCam[p])
			set_view(p, CAMERA_3RDPERSON)
	}
	g_lsgViewFreeze[p] = false
	// (the camera goes when nobody looks through it any more)
	for (new q = 1; q <= MaxClients; q++)
		if (g_lsgViewCam[q] == cam)
			return
	if (pev_valid(cam))
		remove_entity(cam)
}

stock lsg_view_return(p)
{
	if (p)
		lsg_view_back(p)
	else
		for (new q = 1; q <= MaxClients; q++)
			lsg_view_back(q)
}

stock lsg_view_tick(Float:now)
{
	for (new p = 1; p <= MaxClients; p++)
	{
		new cam = g_lsgViewCam[p]
		if (!cam)
			continue
		if (!pev_valid(cam) || (g_lsgViewTill[p] > 0.0 && now >= g_lsgViewTill[p]))
		{
			lsg_view_back(p)
			continue
		}
		new f = g_lsgCamFollow[cam]
		if (f > 0)
		{
			if (!pev_valid(f) || (f <= MaxClients && !is_user_connected(f)))
			{
				lsg_view_back(p)
				continue
			}
			new Float:o[3], Float:ang[3]
			pev(f, pev_origin, o)
			o[2] += g_lsgCamOff[cam]
			pev(f, f <= MaxClients ? pev_v_angle : pev_angles, ang)
			engfunc(EngFunc_SetOrigin, cam, o)
			set_pev(cam, pev_angles, ang)
			set_pev(cam, pev_v_angle, ang)
		}
		else if (g_lsgCamStop[cam] > 0.0 && now >= g_lsgCamStop[cam])
		{
			set_pev(cam, pev_velocity, Float:{0.0, 0.0, 0.0})
			set_pev(cam, pev_avelocity, Float:{0.0, 0.0, 0.0})
			g_lsgCamStop[cam] = 0.0
		}
	}
}
// one server command, only an allowed one (graph::cmd_rules: the app checks the same). The
// engine splits words on spaces and on { } ( ) ' : , and drops quotes: none of those may be in
// it, so the first word checked is the one that runs. amx: an AMX Mod X command instead.
stock bool:lsg_cmd_denied(const w[])
{
	static const DENY_WORDS[][] = { "quit", "exit", "restart", "_restart", "changelevel", "changelevel2", "map", "meta", "amxx", "condump", "logaddress" }
	static const DENY_PARTS[][] = { "file", "cfg", "log", "password", "rcon", "downloadurl", "motd", "exec", "dump", "alias", "plugin", "write", "save", "load", "admin", "ban", "sql", "nav", "del", "remove", "reset", "trace", "record", "demo", "map", "list" }
	for (new i = 0; i < sizeof DENY_WORDS; i++)
		if (equal(w, DENY_WORDS[i]))
			return true
	for (new i = 0; i < sizeof DENY_PARTS; i++)
		if (contain(w, DENY_PARTS[i]) >= 0)
			return true
	return equal(w, "logaddress", 10) != 0
}

stock bool:lsg_server_word(const w[])
{
	static const WORDS[][] = { "say", "humans_join_team", "sv_gravity", "sv_airaccelerate", "sv_accelerate", "sv_maxspeed", "sv_friction", "sv_stopspeed", "sv_alltalk", "sv_maxvelocity", "sv_stepsize", "sv_wateraccelerate", "sv_waterfriction", "sv_bounce", "sv_restart", "sv_restartround", "sv_voiceenable", "bot_quota", "bot_quota_mode", "bot_difficulty", "bot_add", "bot_add_t", "bot_add_ct", "bot_kick", "bot_kill", "bot_knives_only", "bot_pistols_only", "bot_snipers_only", "bot_all_weapons", "bot_allow_rogues", "bot_allow_pistols", "bot_allow_shotguns", "bot_allow_sub_machine_guns", "bot_allow_rifles", "bot_allow_machine_guns", "bot_allow_grenades", "bot_allow_snipers", "bot_allow_shield", "bot_join_team", "bot_join_after_player", "bot_chatter", "bot_stop", "bot_zombie", "bot_defer_to_human", "bot_auto_vacate", "bot_walk", "ls_comp_start", "ls_comp_stop", "ls_comp_spawn", "ls_comp_heal", "ls_comp_players" }
	static const PREFIXES[][] = { "mp_" }
	if (lsg_cmd_denied(w))
		return false
	for (new i = 0; i < sizeof WORDS; i++)
		if (equal(w, WORDS[i]))
			return true
	for (new i = 0; i < sizeof PREFIXES; i++)
		if (equal(w, PREFIXES[i], strlen(PREFIXES[i])) && strlen(w) > strlen(PREFIXES[i]))
			return true
	return false
}

stock bool:lsg_safe_cmd(const cmd[], amx)
{
	lsg_cmd_ok = false
	new c[128], w[48], w2[48], n, k
	copy(c, charsmax(c), cmd)
	trim(c)
	if (!c[0] || strlen(c) > 120)
		return false
	for (new i = 0; c[i]; i++)
		if (!isalnum(c[i]) && c[i] != ' ' && c[i] != '_' && c[i] != '.' && c[i] != '-' && c[i] != '+' && c[i] != '!' && c[i] != '?' && c[i] != '@' && c[i] != '#')
			return false
	// the first two words, lower case (a word that doesn't fit is refused: the same whole words
	// as the app's check, WORD_MAX = 47)
	for (k = 0; c[k] == ' '; k++) {}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w); k++)
		w[n++] = tolower(c[k])
	w[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	for (; c[k] == ' '; k++) {}
	for (n = 0; c[k] && c[k] != ' ' && n < charsmax(w2); k++)
		w2[n++] = tolower(c[k])
	w2[n] = 0
	if (c[k] && c[k] != ' ')
		return false
	if (amx)
	{
		static const AMX_WORDS[][] = { "amx_slap", "amx_slay", "amx_kick", "amx_say", "amx_psay", "amx_tsay", "amx_csay", "amx_chat", "amx_cvar", "amx_map", "amx_nextmap" }
		new found
		for (new i = 0; i < sizeof AMX_WORDS; i++)
			if (equal(w, AMX_WORDS[i]))
				found = 1
		if (!found)
			return false
		if (equal(w, "amx_cvar") && !lsg_server_word(w2))
			return false
		if (equal(w, "amx_map") && (!w2[0] || strlen(w2) > 31 || !is_map_valid(w2)))
			return false
	}
	else if (equal(w, "amx_", 4) || !lsg_server_word(w))
		return false
	server_cmd("%s", c)
	lsg_cmd_ok = true
	return true
}


// ---- where a hit struck ----
stock Float:g_lsgHitPos[2048][3], Float:g_lsgHitDir[2048][3], Float:g_lsgHitAt[2048]

public lsg_hit_trace(victim, attacker, Float:damage, Float:dir[3], tr, bits)
{
	if (victim < 1 || victim >= 2048)
		return HAM_IGNORED
	get_tr2(tr, TR_vecEndPos, g_lsgHitPos[victim])
	g_lsgHitDir[victim] = dir
	g_lsgHitAt[victim] = get_gametime()
	return HAM_IGNORED
}

stock lsg_hit_pos(v)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vfrom(g_lsgHitPos[v])
	return lsg_ecenter(v)
}

stock lsg_hit_dir(v, from)
{
	if (v >= 1 && v < 2048 && get_gametime() - g_lsgHitAt[v] < 0.05)
		return lsg_vnorm(lsg_vfrom(g_lsgHitDir[v]))
	if (from > 0 && pev_valid(from) && from != v)
		return lsg_vdir(lsg_vent(from), lsg_vent(v))
	return lsg_vec(0.0, 0.0, 0.0)
}

// ---- graph version 3: vectors, structs, arrays, maps, text, timers
// Vectors and structs are slots (a ring that's reused, then each variable's own slots); a
// value is the slot's number. Arrays and maps are handles. Every index is clamped and every
// handle and size checked, so a graph never reads or writes outside them.
stock Float:g_lsgVec[LSG_VP][3], g_lsgVecAt
stock g_lsgRec[LSG_RP][LSG_RC], g_lsgRecT[LSG_RP][LSG_RT][64], g_lsgRecAt
stock g_lsgO[LSG_O], g_lsgOT[LSG_OT][128]
stock g_lsgTmpA[LSG_TMPA], g_lsgTmpAt
stock const g_lsgRun[][] = { "stopped", "running" }

stock lsg_vi(v)
	return clamp(v, 0, LSG_VP - 1)

stock lsg_vnew()
{
	if (++g_lsgVecAt >= LSG_VR)
		g_lsgVecAt = 0
	return g_lsgVecAt
}

stock lsg_vec(Float:x, Float:y, Float:z)
{
	new v = lsg_vnew()
	g_lsgVec[v][0] = x
	g_lsgVec[v][1] = y
	g_lsgVec[v][2] = z
	return v
}

stock Float:lsg_vc(v, k)
	return g_lsgVec[lsg_vi(v)][clamp(k, 0, 2)]

stock lsg_vset(dst, src)
{
	new d = lsg_vi(dst), s = lsg_vi(src)
	for (new k = 0; k < 3; k++)
		g_lsgVec[d][k] = g_lsgVec[s][k]
}

stock lsg_vzero(dst)
{
	new d = lsg_vi(dst)
	for (new k = 0; k < 3; k++)
		g_lsgVec[d][k] = 0.0
}

stock lsg_vfrom(const Float:o[3])
	return lsg_vec(o[0], o[1], o[2])

stock lsg_vto(v, Float:o[3])
{
	for (new k = 0; k < 3; k++)
		o[k] = lsg_vc(v, k)
}

stock lsg_vadd(a, b, Float:sign)
	return lsg_vec(lsg_vc(a, 0) + sign * lsg_vc(b, 0), lsg_vc(a, 1) + sign * lsg_vc(b, 1), lsg_vc(a, 2) + sign * lsg_vc(b, 2))

stock lsg_vscale(a, Float:n)
	return lsg_vec(lsg_vc(a, 0) * n, lsg_vc(a, 1) * n, lsg_vc(a, 2) * n)

stock Float:lsg_vlen(a)
	return floatsqroot(lsg_vc(a, 0) * lsg_vc(a, 0) + lsg_vc(a, 1) * lsg_vc(a, 1) + lsg_vc(a, 2) * lsg_vc(a, 2))

stock Float:lsg_vdist(a, b)
{
	new Float:x = lsg_vc(a, 0) - lsg_vc(b, 0), Float:y = lsg_vc(a, 1) - lsg_vc(b, 1), Float:z = lsg_vc(a, 2) - lsg_vc(b, 2)
	return floatsqroot(x * x + y * y + z * z)
}

stock lsg_vnorm(a)
{
	new Float:l = lsg_vlen(a)
	if (l < 0.0001)
		return lsg_vec(0.0, 0.0, 0.0)
	return lsg_vscale(a, 1.0 / l)
}

stock lsg_vdir(from, to)
	return lsg_vnorm(lsg_vadd(to, from, -1.0))

stock bool:lsg_veq(a, b)
	return lsg_vc(a, 0) == lsg_vc(b, 0) && lsg_vc(a, 1) == lsg_vc(b, 1) && lsg_vc(a, 2) == lsg_vc(b, 2)

stock lsg_vtext(v)
{
	new t[64], Float:x = lsg_vc(v, 0), Float:y = lsg_vc(v, 1), Float:z = lsg_vc(v, 2)
	// (a short vector, a direction above all, gets two decimals: in whole units one is "0 0 0")
	if (floatabs(x) < 10.0 && floatabs(y) < 10.0 && floatabs(z) < 10.0)
		formatex(t, charsmax(t), "%.2f %.2f %.2f", x, y, z)
	else
		formatex(t, charsmax(t), "%.0f %.0f %.0f", x, y, z)
	return t
}

// where a player is: 0 feet, 1 eyes, 2 aim direction, 3 what the aim hits, 4 velocity
stock lsg_vplayer(id, what)
{
	new Float:o[3]
	if (!lsg_ok(id))
		return lsg_vfrom(o)
	new Float:ofs[3], Float:ang[3], Float:fwd[3], Float:end[3]
	switch (what)
	{
		case 1, 3:
		{
			pev(id, pev_origin, o)
			pev(id, pev_view_ofs, ofs)
			for (new k = 0; k < 3; k++)
				o[k] += ofs[k]
			if (what == 3)
			{
				pev(id, pev_v_angle, ang)
				angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
				for (new k = 0; k < 3; k++)
					end[k] = o[k] + fwd[k] * 8192.0
				engfunc(EngFunc_TraceLine, o, end, DONT_IGNORE_MONSTERS, id, 0)
				get_tr2(0, TR_vecEndPos, o)
			}
		}
		case 2:
		{
			pev(id, pev_v_angle, ang)
			angle_vector(ang, ANGLEVECTOR_FORWARD, o)
		}
		case 4:
			pev(id, pev_velocity, o)
		default:
			pev(id, pev_origin, o)
	}
	return lsg_vfrom(o)
}

// where an entity is (a brush entity: the middle of its box)
stock lsg_vent(ent)
{
	new Float:o[3]
	if (ent < 1 || !pev_valid(ent))
		return lsg_vfrom(o)
	pev(ent, pev_origin, o)
	if (o[0] == 0.0 && o[1] == 0.0 && o[2] == 0.0)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new k = 0; k < 3; k++)
			o[k] = (mins[k] + maxs[k]) * 0.5
	}
	return lsg_vfrom(o)
}

stock lsg_vsetpos(id, v)
{
	new Float:o[3], Float:still[3]
	lsg_vto(v, o)
	engfunc(EngFunc_SetOrigin, id, o)
	set_pev(id, pev_velocity, still)
}

stock lsg_vsetvel(id, v)
{
	new Float:o[3]
	lsg_vto(v, o)
	for (new k = 0; k < 3; k++)
		o[k] = floatclamp(o[k], -4000.0, 4000.0)
	set_pev(id, pev_velocity, o)
}

// ---- structs
stock lsg_ri(r)
	return clamp(r, 0, LSG_RP - 1)

stock lsg_rzero(r)
{
	new i = lsg_ri(r)
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[i][k] = 0
	for (new k = 0; k < LSG_RT; k++)
		g_lsgRecT[i][k][0] = 0
}

stock lsg_rnew()
{
	if (++g_lsgRecAt >= LSG_RR)
		g_lsgRecAt = 0
	lsg_rzero(g_lsgRecAt)
	return g_lsgRecAt
}

stock lsg_rset(dst, src)
{
	new d = lsg_ri(dst), s = lsg_ri(src)
	if (d == s)
		return
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[d][k] = g_lsgRec[s][k]
	for (new k = 0; k < LSG_RT; k++)
		copy(g_lsgRecT[d][k], 63, g_lsgRecT[s][k])
}

stock bool:lsg_req(a, b)
{
	new x = lsg_ri(a), y = lsg_ri(b)
	for (new k = 0; k < LSG_RC; k++)
		if (g_lsgRec[x][k] != g_lsgRec[y][k])
			return false
	for (new k = 0; k < LSG_RT; k++)
		if (!equal(g_lsgRecT[x][k], g_lsgRecT[y][k]))
			return false
	return true
}

stock lsg_rvec(r, off)
{
	new i = lsg_ri(r)
	return lsg_vec(Float:g_lsgRec[i][off], Float:g_lsgRec[i][off + 1], Float:g_lsgRec[i][off + 2])
}

stock lsg_rsetvec(r, off, v)
{
	new i = lsg_ri(r)
	for (new k = 0; k < 3; k++)
		g_lsgRec[i][off + k] = _:lsg_vc(v, k)
}

stock lsg_rpack(r, b[LSG_AC])
{
	new i = lsg_ri(r)
	for (new k = 0; k < LSG_RC; k++)
		b[k] = g_lsgRec[i][k]
	for (new t = 0; t < LSG_RT; t++)
		for (new c = 0; c < 64; c++)
			b[LSG_RC + t * 64 + c] = g_lsgRecT[i][t][c]
}

stock lsg_runpack(const b[LSG_AC])
{
	new r = lsg_rnew()
	for (new k = 0; k < LSG_RC; k++)
		g_lsgRec[r][k] = b[k]
	for (new t = 0; t < LSG_RT; t++)
	{
		for (new c = 0; c < 63; c++)
			g_lsgRecT[r][t][c] = b[LSG_RC + t * 64 + c]
		g_lsgRecT[r][t][63] = 0
	}
	return r
}

// ---- arrays (every array holds items of LSG_AC cells; at most LSG_AMAX items)
stock lsg_atmp()
{
	if (++g_lsgTmpAt >= LSG_TMPA)
		g_lsgTmpAt = 0
	if (!g_lsgTmpA[g_lsgTmpAt])
		g_lsgTmpA[g_lsgTmpAt] = _:ArrayCreate(LSG_AC)
	else
		ArrayClear((Array:g_lsgTmpA[g_lsgTmpAt]))
	return g_lsgTmpA[g_lsgTmpAt]
}

stock lsg_alen(a)
	return a ? ArraySize((Array:a)) : 0

// an index, or -1 when it's outside the array
stock lsg_aidx(a, Float:i)
{
	new k = floatround(i, floatround_floor)
	return k >= 0 && k < lsg_alen(a) ? k : -1
}

stock bool:lsg_aroom(a)
	return a && ArraySize((Array:a)) < LSG_AMAX

stock lsg_aremi(a, Float:i)
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayDeleteItem((Array:a), k)
}

stock lsg_aclear(a)
{
	if (a)
		ArrayClear((Array:a))
}

stock lsg_ashuffle(a)
{
	for (new k = lsg_alen(a) - 1; k > 0; k--)
		ArraySwap((Array:a), k, random(k + 1))
}

// kind: 0 numbers, 1 by cell, 2 text
stock lsg_asort(a, kind, desc)
{
	if (lsg_alen(a) < 2)
		return
	new d[2]
	d[0] = kind
	d[1] = desc
	ArraySort((Array:a), "lsg_acmp", d, 2)
}

public lsg_acmp(Array:a, i1, i2, const d[], size)
{
	#pragma unused size
	new r
	switch (d[0])
	{
		case 0:
		{
			new Float:x = Float:ArrayGetCell(a, i1), Float:y = Float:ArrayGetCell(a, i2)
			r = x < y ? -1 : (x > y ? 1 : 0)
		}
		case 2:
		{
			new s1[64], s2[64]
			ArrayGetString(a, i1, s1, 63)
			ArrayGetString(a, i2, s2, 63)
			r = strcmp(s1, s2, true)
			r = r < 0 ? -1 : (r > 0 ? 1 : 0)
		}
		default:
		{
			new x = ArrayGetCell(a, i1), y = ArrayGetCell(a, i2)
			r = x < y ? -1 : (x > y ? 1 : 0)
		}
	}
	return d[1] ? -r : r
}

stock lsg_acopy(dst, src)
{
	if (!dst || dst == src)
		return
	ArrayClear((Array:dst))
	new n = min(lsg_alen(src), LSG_AMAX), b[LSG_AC]
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:src), k, b)
		ArrayPushArray((Array:dst), b)
	}
}

stock lsg_aplayers(which)
{
	new a = lsg_atmp()
	for (new p = 1; p <= MaxClients; p++)
		if (lsg_match(p, which))
			ArrayPushCell((Array:a), p)
	return a
}

// items in a cell
stock lsg_apushc(a, v)
	return lsg_aroom(a) ? ArrayPushCell((Array:a), v) : -1

stock lsg_agetc(a, Float:i)
{
	new k = lsg_aidx(a, i)
	return k < 0 ? 0 : ArrayGetCell((Array:a), k)
}

stock lsg_asetc(a, Float:i, v)
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArraySetCell((Array:a), k, v)
}

stock lsg_ainsc(a, Float:i, v)
{
	if (!lsg_aroom(a))
		return
	new k = floatround(i, floatround_floor)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushCell((Array:a), v)
	else
		ArrayInsertCellBefore((Array:a), k, v)
}

stock lsg_afindc(a, v)
{
	new n = lsg_alen(a)
	for (new k = 0; k < n; k++)
		if (ArrayGetCell((Array:a), k) == v)
			return k
	return -1
}

stock bool:lsg_aremc(a, v)
{
	new bool:any
	for (new k = lsg_alen(a) - 1; k >= 0; k--)
		if (ArrayGetCell((Array:a), k) == v)
		{
			ArrayDeleteItem((Array:a), k)
			any = true
		}
	return any
}

stock lsg_arandc(a)
{
	new n = lsg_alen(a)
	return n ? ArrayGetCell((Array:a), random(n)) : 0
}

// text items
stock lsg_apushs(a, const s[])
	return lsg_aroom(a) ? ArrayPushString((Array:a), s) : -1

stock lsg_agets(a, Float:i)
{
	new t[64], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetString((Array:a), k, t, 63)
	return t
}

stock lsg_asets(a, Float:i, const s[])
{
	new k = lsg_aidx(a, i)
	if (k >= 0)
		ArraySetString((Array:a), k, s)
}

stock lsg_ainss(a, Float:i, const s[])
{
	if (!lsg_aroom(a))
		return
	new k = floatround(i, floatround_floor)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushString((Array:a), s)
	else
		ArrayInsertStringBefore((Array:a), k, s)
}

stock lsg_afinds(a, const s[])
{
	new n = lsg_alen(a), t[64]
	for (new k = 0; k < n; k++)
	{
		ArrayGetString((Array:a), k, t, 63)
		if (equal(t, s))
			return k
	}
	return -1
}

stock bool:lsg_arems(a, const s[])
{
	new bool:any, t[64]
	for (new k = lsg_alen(a) - 1; k >= 0; k--)
	{
		ArrayGetString((Array:a), k, t, 63)
		if (equal(t, s))
		{
			ArrayDeleteItem((Array:a), k)
			any = true
		}
	}
	return any
}

stock lsg_arands(a)
{
	new t[64], n = lsg_alen(a)
	if (n)
		ArrayGetString((Array:a), random(n), t, 63)
	return t
}

// vector items (3 cells)
stock lsg_vbuf(v, b[LSG_AC])
{
	for (new k = 0; k < 3; k++)
		b[k] = _:lsg_vc(v, k)
}

stock lsg_apushv(a, v)
{
	if (!lsg_aroom(a))
		return -1
	new b[LSG_AC]
	lsg_vbuf(v, b)
	return ArrayPushArray((Array:a), b)
}

stock lsg_agetv(a, Float:i)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetArray((Array:a), k, b)
	return lsg_vec(Float:b[0], Float:b[1], Float:b[2])
}

stock lsg_asetv(a, Float:i, v)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	lsg_vbuf(v, b)
	if (k >= 0)
		ArraySetArray((Array:a), k, b)
}

stock lsg_ainsv(a, Float:i, v)
{
	if (!lsg_aroom(a))
		return
	new b[LSG_AC], k = floatround(i, floatround_floor)
	lsg_vbuf(v, b)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushArray((Array:a), b)
	else
		ArrayInsertArrayBefore((Array:a), k, b)
}

stock lsg_afindv(a, v)
{
	new n = lsg_alen(a), b[LSG_AC]
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:a), k, b)
		if (Float:b[0] == lsg_vc(v, 0) && Float:b[1] == lsg_vc(v, 1) && Float:b[2] == lsg_vc(v, 2))
			return k
	}
	return -1
}

stock bool:lsg_aremv(a, v)
{
	new bool:any, k
	while ((k = lsg_afindv(a, v)) >= 0)
	{
		ArrayDeleteItem((Array:a), k)
		any = true
	}
	return any
}

stock lsg_arandv(a)
{
	new n = lsg_alen(a)
	return lsg_agetv(a, n ? float(random(n)) : -1.0)
}

// struct items (packed)
stock lsg_apushr(a, r)
{
	if (!lsg_aroom(a))
		return -1
	new b[LSG_AC]
	lsg_rpack(r, b)
	return ArrayPushArray((Array:a), b)
}

stock lsg_agetr(a, Float:i)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	if (k >= 0)
		ArrayGetArray((Array:a), k, b)
	return lsg_runpack(b)
}

stock lsg_asetr(a, Float:i, r)
{
	new b[LSG_AC], k = lsg_aidx(a, i)
	lsg_rpack(r, b)
	if (k >= 0)
		ArraySetArray((Array:a), k, b)
}

stock lsg_ainsr(a, Float:i, r)
{
	if (!lsg_aroom(a))
		return
	new b[LSG_AC], k = floatround(i, floatround_floor)
	lsg_rpack(r, b)
	if (k < 0 || k >= ArraySize((Array:a)))
		ArrayPushArray((Array:a), b)
	else
		ArrayInsertArrayBefore((Array:a), k, b)
}

stock lsg_afindr(a, r)
{
	new n = lsg_alen(a), want[LSG_AC], b[LSG_AC]
	lsg_rpack(r, want)
	for (new k = 0; k < n; k++)
	{
		ArrayGetArray((Array:a), k, b)
		new same = 1
		for (new c = 0; c < LSG_AC && same; c++)
			if (b[c] != want[c])
				same = 0
		if (same)
			return k
	}
	return -1
}

stock bool:lsg_aremr(a, r)
{
	new bool:any, k
	while ((k = lsg_afindr(a, r)) >= 0)
	{
		ArrayDeleteItem((Array:a), k)
		any = true
	}
	return any
}

stock lsg_arandr(a)
{
	new n = lsg_alen(a)
	return lsg_agetr(a, n ? float(random(n)) : -1.0)
}

// ---- maps: keys are text (a number written out, a player as #userid)
stock lsg_kn(Float:n)
{
	new k[24]
	formatex(k, charsmax(k), "%.3f", n)
	return k
}

stock lsg_kp(id)
{
	new k[16]
	if (lsg_ok(id))
		formatex(k, charsmax(k), "#%d", get_user_userid(id))
	return k
}

stock lsg_by_userid(u)
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p) && get_user_userid(p) == u)
			return p
	return 0
}

stock lsg_mlen(m)
	return m ? TrieGetSize((Trie:m)) : 0

stock bool:lsg_mhas(m, const k[])
	return m && TrieKeyExists((Trie:m), k)

stock lsg_mdel(m, const k[])
{
	if (m)
		TrieDeleteKey((Trie:m), k)
}

stock lsg_mclear(m)
{
	if (m)
		TrieClear((Trie:m))
}

stock lsg_msetc(m, const k[], v)
{
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetCell((Trie:m), k, v)
}

stock lsg_msets(m, const k[], const s[])
{
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetString((Trie:m), k, s)
}

stock lsg_msetv(m, const k[], v)
{
	new Float:b[3]
	lsg_vto(v, b)
	if (m && (TrieGetSize((Trie:m)) < LSG_AMAX || TrieKeyExists((Trie:m), k)))
		TrieSetArray((Trie:m), k, b, 3)
}

stock lsg_mgetc(m, const k[])
{
	new v
	if (m)
		TrieGetCell((Trie:m), k, v)
	return v
}

stock lsg_mgets(m, const k[])
{
	new t[64]
	if (m)
		TrieGetString((Trie:m), k, t, 63)
	return t
}

stock lsg_mgetv(m, const k[])
{
	new Float:b[3]
	if (m)
		TrieGetArray((Trie:m), k, b, 3)
	return lsg_vfrom(b)
}

// keys: 0 text, 1 numbers, 2 players
stock lsg_mkeys(m, kind)
{
	new a = lsg_atmp()
	if (!m)
		return a
	new TrieIter:it = TrieIterCreate(Trie:m), k[64], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		TrieIterGetKey(it, k, charsmax(k))
		switch (kind)
		{
			case 1: ArrayPushCell((Array:a), _:str_to_float(k))
			case 2:
			{
				new p = lsg_by_userid(str_to_num(k[1]))
				if (p)
					ArrayPushCell((Array:a), p)
			}
			default: ArrayPushString((Array:a), k)
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
	return a
}

// values: 0 cells, 1 text, 2 vectors
stock lsg_mvalues(m, kind)
{
	new a = lsg_atmp()
	if (!m)
		return a
	new TrieIter:it = TrieIterCreate(Trie:m), t[64], v, Float:b[3], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		switch (kind)
		{
			case 1:
			{
				TrieIterGetString(it, t, charsmax(t))
				ArrayPushString((Array:a), t)
			}
			case 2:
			{
				TrieIterGetArray(it, b, 3)
				lsg_apushv(a, lsg_vfrom(b))
			}
			default:
			{
				TrieIterGetCell(it, v)
				ArrayPushCell((Array:a), v)
			}
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
	return a
}

stock lsg_mcopy(dst, src, kind)
{
	if (!dst || dst == src)
		return
	TrieClear((Trie:dst))
	if (!src)
		return
	new TrieIter:it = TrieIterCreate((Trie:src)), k[64], t[64], v, Float:b[3], n
	while (!TrieIterEnded(it) && n++ < LSG_AMAX)
	{
		TrieIterGetKey(it, k, charsmax(k))
		switch (kind)
		{
			case 1:
			{
				TrieIterGetString(it, t, charsmax(t))
				TrieSetString((Trie:dst), k, t)
			}
			case 2:
			{
				TrieIterGetArray(it, b, 3)
				TrieSetArray((Trie:dst), k, b, 3)
			}
			default:
			{
				TrieIterGetCell(it, v)
				TrieSetCell((Trie:dst), k, v)
			}
		}
		TrieIterNext(it)
	}
	TrieIterDestroy(it)
}

// ---- text, numbers, things
stock lsg_tcat(const a[], const b[])
{
	new t[128]
	formatex(t, charsmax(t), "%s%s", a, b)
	return t
}

stock lsg_tsel(c, const a[], const b[])
{
	new t[128]
	if (c)
		copy(t, charsmax(t), a)
	else
		copy(t, charsmax(t), b)
	return t
}

stock lsg_etext(e)
{
	new t[32]
	formatex(t, charsmax(t), "entity %d", e)
	return t
}

stock Float:lsg_sign(Float:a)
	return a > 0.0 ? 1.0 : (a < 0.0 ? -1.0 : 0.0)

stock Float:lsg_clampf(Float:a, Float:lo, Float:hi)
{
	if (lo > hi)
		return floatclamp(a, hi, lo)
	return floatclamp(a, lo, hi)
}

stock bool:lsg_valid(e)
{
	if (e >= 1 && e <= MaxClients)
		return bool:is_user_connected(e)
	return e > MaxClients && pev_valid(e) > 0
}

stock bool:lsg_is_creature(e)
{
	if (e <= MaxClients || !pev_valid(e))
		return false
	new c[16]
	pev(e, pev_classname, c, charsmax(c))
	return bool:equal(c, "ls_creature")
}

stock bool:lsg_tactive(h)
	return h >= LSG_TASK && h < LSG_TASK + LSG_N && task_exists(h)

stock lsg_tclear(h)
{
	if (h >= LSG_TASK && h < LSG_TASK + LSG_N)
		remove_task(h)
}

// ---- (vectors) transforms, entities, effects ----
stock lsg_vpev(id, field)
{
	new Float:o[3]
	if (id > 0 && pev_valid(id))
		pev(id, field, o)
	return lsg_vfrom(o)
}

stock lsg_set_transform(id, pos, ang, vel)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new Float:o[3]
	if (pos >= 0)
	{
		lsg_vto(pos, o)
		engfunc(EngFunc_SetOrigin, id, o)
	}
	if (ang >= 0)
	{
		lsg_vto(ang, o)
		set_pev(id, pev_angles, o)
		set_pev(id, pev_v_angle, o)
		set_pev(id, pev_fixangle, 1)
	}
	if (vel >= 0)
	{
		lsg_vto(vel, o)
		set_pev(id, pev_velocity, o)
	}
}

stock lsg_look_at(id, ent, pos)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return
	new Float:t[3], Float:me[3], Float:ofs[3], Float:d[3], Float:ang[3]
	if (ent > 0)
	{
		if (!lsg_ent_origin(ent, t))
			return
	}
	else if (pos >= 0)
		lsg_vto(pos, t)
	else
		return
	pev(id, pev_origin, me)
	pev(id, pev_view_ofs, ofs)
	for (new k = 0; k < 3; k++)
		d[k] = t[k] - (me[k] + ofs[k])
	vector_to_angle(d, ang)
	ang[0] = -ang[0]
	set_pev(id, pev_angles, ang)
	set_pev(id, pev_v_angle, ang)
	set_pev(id, pev_fixangle, 1)
}

stock lsg_find_all(byname, const name[])
{
	new a = lsg_atmp(), e = -1, n
	while (n < 512 && (e = engfunc(EngFunc_FindEntityByString, e, byname ? "targetname" : "classname", name)) > 0)
	{
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_ent_pos(e, v)
{
	if (e > MaxClients && pev_valid(e))
	{
		new Float:o[3]
		lsg_vto(v, o)
		engfunc(EngFunc_SetOrigin, e, o)
	}
}

stock lsg_ent_angles(e, v)
{
	if (e > MaxClients && pev_valid(e))
	{
		new Float:o[3]
		lsg_vto(v, o)
		set_pev(e, pev_angles, o)
	}
}

stock lsg_explode(v, Float:dmg, Float:radius)
{
	new Float:o[3]
	lsg_vto(v, o)
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_EXPLOSION)
	engfunc(EngFunc_WriteCoord, o[0])
	engfunc(EngFunc_WriteCoord, o[1])
	engfunc(EngFunc_WriteCoord, o[2])
	write_short(g_lsgSprExp)
	write_byte(30)
	write_byte(15)
	write_byte(0)
	message_end()
	if (dmg <= 0.0)
		return
	new Float:p[3]
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id))
			continue
		pev(id, pev_origin, p)
		new Float:d = get_distance_f(o, p)
		if (d < radius)
			ExecuteHamB(Ham_TakeDamage, id, 0, 0, dmg * (1.0 - d / radius), DMG_BLAST)
	}
}

stock lsg_beam(a, b, r, g, bl, Float:secs)
{
	new Float:x[3], Float:y[3]
	lsg_vto(a, x)
	lsg_vto(b, y)
	message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
	write_byte(TE_BEAMPOINTS)
	engfunc(EngFunc_WriteCoord, x[0])
	engfunc(EngFunc_WriteCoord, x[1])
	engfunc(EngFunc_WriteCoord, x[2])
	engfunc(EngFunc_WriteCoord, y[0])
	engfunc(EngFunc_WriteCoord, y[1])
	engfunc(EngFunc_WriteCoord, y[2])
	write_short(g_lsgSprBeam)
	write_byte(0)
	write_byte(0)
	write_byte(clamp(floatround(secs * 10.0), 1, 255))
	write_byte(20)
	write_byte(0)
	write_byte(r)
	write_byte(g)
	write_byte(bl)
	write_byte(200)
	write_byte(0)
	message_end()
}

stock lsg_sound_at(v, const snd[])
{
	// (a sound that wasn't there when the map loaded: nothing)
	if (!lsg_have(snd))
		return
	new Float:o[3]
	lsg_vto(v, o)
	engfunc(EngFunc_EmitAmbientSound, 0, o, snd, 1.0, ATTN_NORM, 0, PITCH_NORM)
}

// Timelines: smooth in and out between two keys (0..1 -> 0..1)
stock Float:lsg_tl_ease(Float:x)
{
	x = floatclamp(x, 0.0, 1.0)
	return x * x * (3.0 - 2.0 * x)
}

// an entity's rotation (a player's view angles)
stock lsg_erot(e)
	return lsg_vpev(e, e >= 1 && e <= MaxClients ? pev_v_angle : pev_angles)

// the middle of an entity's box
stock lsg_ecenter(e)
{
	new Float:mn[3], Float:mx[3], Float:c[3]
	if (e > 0 && pev_valid(e))
	{
		pev(e, pev_absmin, mn)
		pev(e, pev_absmax, mx)
		for (new k = 0; k < 3; k++)
			c[k] = (mn[k] + mx[k]) / 2.0
	}
	return lsg_vfrom(c)
}


// enum Entity Kind
stock const g_lsgE0[][] = { "Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None" }
// enum Component Kind
stock const g_lsgE1[][] = { "Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None" }
// Data table: Guns
#define LSG_DT0_N 2
stock const Float:g_lsgDt0_0[2] = { 0.0, 1.0 }
stock Float:lsg_dt0_0(Float:row)
{
	new r = floatround(row, floatround_floor)
	return r >= 0 && r < LSG_DT0_N ? g_lsgDt0_0[r] : 0.0
}
stock const g_lsgDt0_1[2][49] = { "glock18", "ak47" }
stock lsg_dt0_1(Float:row)
{
	new s[49], r = floatround(row, floatround_floor)
	if (r >= 0 && r < LSG_DT0_N)
		copy(s, charsmax(s), g_lsgDt0_1[r])
	return s
}
stock const Float:g_lsgDt0_2[2] = { 120.0, 90.0 }
stock Float:lsg_dt0_2(Float:row)
{
	new r = floatround(row, floatround_floor)
	return r >= 0 && r < LSG_DT0_N ? g_lsgDt0_2[r] : 0.0
}
stock const g_lsgDt0_3[2] = { 1, 0 }
stock lsg_dt0_3(Float:row)
{
	new r = floatround(row, floatround_floor)
	return r >= 0 && r < LSG_DT0_N ? g_lsgDt0_3[r] : 0
}
stock Float:lsg_dt0_find(Float:key)
{
	// (the same key as last time: the same row)
	static bool:had, Float:last, Float:lastRow
	if (had && key == last)
		return lastRow
	new Float:row = -1.0
	for (new r = 0; r < LSG_DT0_N; r++)
		if (g_lsgDt0_0[r] == key)
		{
			row = float(r)
			break
		}
	had = true
	last = key
	lastRow = row
	return row
}
stock lsg_dt0_keys()
{
	new a = lsg_atmp()
	for (new r = 0; r < LSG_DT0_N; r++)
		ArrayPushCell((Array:a), g_lsgDt0_0[r])
	return a
}
stock lsg_dt0_where0(Float:v)
{
	new a = lsg_atmp()
	for (new r = 0; r < LSG_DT0_N; r++)
		if (g_lsgDt0_0[r] == v)
			ArrayPushCell((Array:a), _:float(r))
	return a
}
stock lsg_dt0_where1(const v[])
{
	new a = lsg_atmp()
	for (new r = 0; r < LSG_DT0_N; r++)
		if (equal(g_lsgDt0_1[r], v))
			ArrayPushCell((Array:a), _:float(r))
	return a
}
stock lsg_dt0_where2(Float:v)
{
	new a = lsg_atmp()
	for (new r = 0; r < LSG_DT0_N; r++)
		if (g_lsgDt0_2[r] == v)
			ArrayPushCell((Array:a), _:float(r))
	return a
}
stock lsg_dt0_where3(v)
{
	new a = lsg_atmp()
	for (new r = 0; r < LSG_DT0_N; r++)
		if (!g_lsgDt0_3[r] == !v)
			ArrayPushCell((Array:a), _:float(r))
	return a
}


lsd_graph_vars()
{
	lsd_var_number("hold", 0, g_v0)
	lsd_var_bool("ff", 0, g_v1)
	lsd_var_number("size", 0, g_v2)
}

lsd_graph_set(const vid[], who, const val[])
{
	if (equal(vid, "hold"))
	{
		g_v0 = str_to_float(val)
		return true
	}
	if (equal(vid, "ff"))
	{
		g_v1 = str_to_num(val) ? 1 : 0
		return true
	}
	if (equal(vid, "size"))
	{
		g_v2 = str_to_float(val)
		return true
	}
	#pragma unused who
	return false
}

// ---- Get Bone / Attachment Position: a model's bones by name and attachments by number, from
// the entity's own model file (the header's tables), cached per model; -1 when it has none
stock Trie:g_lsgMp = Invalid_Trie

stock lsg_mp_model(ent, m[], len)
{
	m[0] = 0
	if (ent < 1 || !pev_valid(ent))
		return 0
	pev(ent, pev_model, m, len)
	new n = strlen(m)
	return n > 4 && equali(m[n - 4], ".mdl") && pev(ent, pev_modelindex) > 0
}

stock lsg_mp_bone(ent, const name[])
{
	new m[96], key[140], k
	if (!name[0] || !lsg_mp_model(ent, m, charsmax(m)))
		return -1
	formatex(key, charsmax(key), "b|%s|%s", m, name)
	if (g_lsgMp == Invalid_Trie)
		g_lsgMp = TrieCreate()
	if (TrieGetCell(g_lsgMp, key, k))
		return k
	k = lsg_mp_read(m, name, -1)
	TrieSetCell(g_lsgMp, key, k)
	return k
}

stock lsg_mp_att(ent, n)
{
	new m[96], key[140], k
	if (n < 0 || n > 3 || !lsg_mp_model(ent, m, charsmax(m)))
		return -1
	formatex(key, charsmax(key), "a|%s|%d", m, n)
	if (g_lsgMp == Invalid_Trie)
		g_lsgMp = TrieCreate()
	if (TrieGetCell(g_lsgMp, key, k))
		return k
	k = lsg_mp_read(m, "", n)
	TrieSetCell(g_lsgMp, key, k)
	return k
}

// a bone's index by name (att -1), or the attachment number if the model has it
stock lsg_mp_read(const m[], const name[], att)
{
	new f = fopen(m, "rb", true)
	if (!f)
		return -1
	new id, ver, n, off, k = -1
	fread(f, id, BLOCK_INT)
	fread(f, ver, BLOCK_INT)
	if (id == 0x54534449 && ver == 10)
	{
		fseek(f, att >= 0 ? 212 : 140, SEEK_SET)
		fread(f, n, BLOCK_INT)
		fread(f, off, BLOCK_INT)
		if (att >= 0)
			k = att < n ? att : -1
		else
		{
			new nm[33]
			for (new i = 0; i < n && i < 128; i++)
			{
				fseek(f, off + i * 112, SEEK_SET)
				fread_blocks(f, nm, 32, BLOCK_CHAR)
				nm[32] = 0
				if (equali(nm, name))
				{
					k = i
					break
				}
			}
		}
	}
	fclose(f)
	return k
}

// where (which 0) or which way (1) a bone (kind 0) or attachment (1) is now; the entity's own
// place when it has none
stock lsg_mp_vec(ent, k, kind, which)
{
	new Float:o[3], Float:a[3]
	if (k >= 0 && pev_valid(ent))
	{
		if (kind)
			engfunc(EngFunc_GetAttachment, ent, k, o, a)
		else
			engfunc(EngFunc_GetBonePosition, ent, k, o, a)
	}
	else if (ent > 0 && pev_valid(ent))
	{
		pev(ent, pev_origin, o)
		pev(ent, pev_angles, a)
	}
	return lsg_vfrom(which ? a : o)
}

#define LSG_VMN 1
stock const g_lsgVmList[LSG_VMN + 1][] = { "", "" }
// ---- HUD & View (engine nodes): the game's own switches, kept through what resets them ----
stock g_lsgHud[33], g_lsgHudSet[33], bool:g_lsgHudKeep[33], g_lsgFov[33], bool:g_lsgFovKeep[33]
stock g_lsgVm[33][33], g_lsgPm[33][33], bool:g_lsgNoLight[33], bool:g_lsgQuiet[33]
stock g_lsgTag[33], bool:g_lsgTagKeep[33], g_lsgShowTeam[33], g_lsgSpec[33]
// when a node last set something for the player: a Player Spawned that sets it isn't undone by
// the same spawn's "back to normal"
stock Float:g_lsgEngAt[33]
stock const g_lsgTeamWord[][] = { "UNASSIGNED", "TERRORIST", "CT", "SPECTATOR" }

// Hide HUD Parts: our bits in the game's m_iHideHUD (it sends HideWeapon and the crosshair itself)
stock lsg_eng_hud(id, mask, bool:keep)
{
	g_lsgHud[id] = mask
	g_lsgHudKeep[id] = keep
	g_lsgEngAt[id] = get_gametime()
	lsg_eng_hud_put(id)
}

stock lsg_eng_hud_put(id)
{
	if (!is_user_connected(id))
		return
	new cur = get_ent_data(id, "CBasePlayer", "m_iHideHUD")
	cur = (cur & ~g_lsgHudSet[id]) | g_lsgHud[id]
	g_lsgHudSet[id] = g_lsgHud[id]
	set_ent_data(id, "CBasePlayer", "m_iHideHUD", cur)
}

// Set Field of View: the game's m_iFOV (it sends SetFOV); 0 is the game's own
stock lsg_eng_fov(id, fov, bool:keep)
{
	g_lsgFov[id] = fov == 90 ? 0 : fov
	g_lsgFovKeep[id] = keep
	g_lsgEngAt[id] = get_gametime()
	lsg_eng_fov_put(id, true)
}

stock lsg_eng_fov_put(id, bool:now)
{
	if (!is_user_connected(id))
		return
	new want = g_lsgFov[id] ? g_lsgFov[id] : 90
	new cur = get_ent_data(id, "CBasePlayer", "m_iFOV")
	// (a scope zoomed in: left alone, and this comes back once it's off)
	if (!now && cur != 90 && cur != want)
		return
	if (cur != want)
	{
		set_ent_data(id, "CBasePlayer", "m_iFOV", want)
		set_pev(id, pev_fov, float(want))
	}
}

// Set Weapon View Model: per player and weapon, the model to show (index into g_lsgVmList)
stock lsg_eng_vm(id, const wname[], vm, pm)
{
	if (!lsg_ok(id))
		return 0
	new csw
	if (wname[0] == '-')
		csw = is_user_alive(id) ? get_user_weapon(id) : 0
	else
		csw = get_weaponid(wname)
	if (csw <= 0 || csw > 32)
		return 0
	if ((vm && !lsg_eng_vm_ok(vm)) || (pm && !lsg_eng_vm_ok(pm)))
		return 0
	g_lsgVm[id][csw] = vm
	g_lsgPm[id][csw] = pm
	// the one in hand: now (back to its own: deployed again)
	if (is_user_alive(id) && get_user_weapon(id) == csw)
	{
		new w = get_ent_data_entity(id, "CBasePlayer", "m_pActiveItem")
		if (!vm && !pm && w > 0)
			ExecuteHamB(Ham_Item_Deploy, w)
		else
			lsg_eng_vm_put(id, csw)
	}
	return 1
}

stock bool:lsg_eng_vm_ok(k)
{
	return k >= 1 && k <= LSG_VMN && g_lsgHave != Invalid_Trie && TrieKeyExists(g_lsgHave, g_lsgVmList[k])
}

stock lsg_eng_vm_put(id, csw)
{
	if (g_lsgVm[id][csw])
		set_pev(id, pev_viewmodel2, g_lsgVmList[g_lsgVm[id][csw]])
	if (g_lsgPm[id][csw])
		set_pev(id, pev_weaponmodel2, g_lsgVmList[g_lsgPm[id][csw]])
}

public lsg_eng_deploy(w)
{
	if (!pev_valid(w))
		return HAM_IGNORED
	new id = get_ent_data_entity(w, "CBasePlayerItem", "m_pPlayer")
	if (!lsg_ok(id))
		return HAM_IGNORED
	new csw = cs_get_weapon_id(w)
	if (csw > 0 && csw <= 32)
		lsg_eng_vm_put(id, csw)
	return HAM_IGNORED
}

// Flashlight: 0 off, 1 on, 2 allow, 3 deny; battery -1 leaves it
stock lsg_eng_light(id, what, bat)
{
	if (!lsg_ok(id) || !is_user_alive(id))
		return 0
	if (what == 2)
		g_lsgNoLight[id] = false
	else if (what == 3)
		g_lsgNoLight[id] = true
	if (bat >= 0)
		set_ent_data(id, "CBasePlayer", "m_iFlashBattery", clamp(bat, 0, 100))
	new bool:lit = (pev(id, pev_effects) & EF_DIMLIGHT) != 0
	if (what == 1)
		lit = true
	else if (what == 0 || what == 3)
		lit = false
	set_pev(id, pev_effects, lit ? (pev(id, pev_effects) | EF_DIMLIGHT) : (pev(id, pev_effects) & ~EF_DIMLIGHT))
	set_ent_data_float(id, "CBasePlayer", "m_flFlashLightTime", get_gametime() + (lit ? 1.2 : 0.2))
	message_begin(MSG_ONE, get_user_msgid("Flashlight"), _, id)
	write_byte(lit ? 1 : 0)
	write_byte(get_ent_data(id, "CBasePlayer", "m_iFlashBattery"))
	message_end()
	return 1
}

public lsg_eng_impulse(id)
	return lsg_ok(id) && g_lsgNoLight[id] ? PLUGIN_HANDLED_MAIN : PLUGIN_CONTINUE

// Night Vision: 0 take, 1 give, 2 on, 3 off
stock lsg_eng_nvg(id, what)
{
	if (!lsg_ok(id))
		return 0
	if (what == 1 || what == 2)
		cs_set_user_nvg(id, 1)
	new bool:on = what == 2
	if (what == 0 || what == 3 || what == 2)
	{
		set_ent_data(id, "CBasePlayer", "m_bNightVisionOn", on)
		if (!is_user_bot(id))
		{
			message_begin(MSG_ONE, get_user_msgid("NVGToggle"), _, id)
			write_byte(on ? 1 : 0)
			message_end()
		}
	}
	if (what == 0)
		cs_set_user_nvg(id, 0)
	return 1
}

// Silent Footsteps: the game waits for the next step sound
public lsg_eng_think(id)
{
	if (id >= 1 && id <= 32 && g_lsgQuiet[id])
		set_pev(id, pev_flTimeStepSound, 999)
	return FMRES_IGNORED
}

// Scoreboard Tag: 1 dead, 2 bomb, 4 VIP (0: the game's own), put back whenever the game sends one
stock lsg_eng_tag(id, tag, bool:keep)
{
	g_lsgTag[id] = tag
	g_lsgTagKeep[id] = keep
	g_lsgEngAt[id] = get_gametime()
	message_begin(MSG_ALL, get_user_msgid("ScoreAttrib"))
	write_byte(id)
	write_byte(lsg_eng_real_attr(id) | tag)
	message_end()
}

stock lsg_eng_real_attr(id)
{
	new f = is_user_alive(id) ? 0 : 1
	if (user_has_weapon(id, CSW_C4))
		f |= 2
	if (cs_get_user_vip(id))
		f |= 4
	return f
}

public lsg_eng_attr(msg, dest, to)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= 32 && g_lsgTag[id])
		set_msg_arg_int(2, ARG_BYTE, get_msg_arg_int(2) | g_lsgTag[id])
	return PLUGIN_CONTINUE
}

// Scoreboard Team Color: what everyone's game is told the team is (0: the real one)
stock lsg_eng_color(id, t)
{
	g_lsgShowTeam[id] = t
	new real = _:cs_get_user_team(id)
	message_begin(MSG_ALL, get_user_msgid("TeamInfo"))
	write_byte(id)
	write_string(g_lsgTeamWord[t ? t : clamp(real, 0, 3)])
	message_end()
}

public lsg_eng_teaminfo(msg, dest, to)
{
	new id = get_msg_arg_int(1)
	if (id >= 1 && id <= 32 && g_lsgShowTeam[id])
		set_msg_arg_string(2, g_lsgTeamWord[g_lsgShowTeam[id]])
	return PLUGIN_CONTINUE
}

// Force Spectate: through Target's eyes, kept there
stock lsg_eng_spec(id, target)
{
	if (!lsg_ok(id) || is_user_alive(id) || !lsg_ok(target) || target == id)
		return 0
	g_lsgSpec[id] = target
	lsg_eng_spec_put(id)
	return 1
}

stock lsg_eng_spec_put(id)
{
	new t = g_lsgSpec[id]
	if (!lsg_ok(t) || is_user_alive(id))
	{
		g_lsgSpec[id] = 0
		return
	}
	if (pev(id, pev_iuser2) != t || pev(id, pev_iuser1) != 4)
	{
		set_ent_data_entity(id, "CBasePlayer", "m_hObserverTarget", t)
		set_pev(id, pev_iuser1, 4)
		set_pev(id, pev_iuser2, t)
	}
}

// Play Soundtrack: as a map's trigger_cdaudio does it (the player's own music files)
stock lsg_eng_cd(id, track, bool:loop)
{
	if (!is_user_connected(id) || is_user_bot(id))
		return
	if (!track)
		client_cmd(id, "cd stop")
	else
		client_cmd(id, "cd %s %d", loop ? "loop" : "play", track)
}

// a quarter second: the HUD parts the game shows again (it clears some itself: the timer, the
// weapon panel on a pickup), the field of view after a scope; the spectated stay on their target
public lsg_eng_tick()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!is_user_connected(p))
			continue
		if (g_lsgHud[p])
			lsg_eng_hud_put(p)
		if (g_lsgFov[p] && is_user_alive(p))
			lsg_eng_fov_put(p, false)
		if (g_lsgSpec[p])
			lsg_eng_spec_put(p)
	}
}

// the HUD redrawn (a respawn, a reconnect's first frame): ours again
public lsg_eng_reset(id)
{
	if (g_lsgHud[id])
		lsg_eng_hud_put(id)
	if (g_lsgFov[id])
		lsg_eng_fov_put(id, true)
}

// death/respawn: what's set to end there ends, the rest is put back
public lsg_eng_dead(id)
	lsg_eng_ends(id)

public lsg_eng_spawn(id)
{
	if (!is_user_alive(id))
		return
	// (set by this same spawn's Player Spawned: kept)
	if (id >= 1 && id <= 32 && get_gametime() - g_lsgEngAt[id] > 0.1)
		lsg_eng_ends(id)
	if (g_lsgHud[id])
		lsg_eng_hud_put(id)
	if (g_lsgFov[id])
		lsg_eng_fov_put(id, true)
}

stock lsg_eng_ends(id)
{
	if (id < 1 || id > 32)
		return
	if (g_lsgHud[id] && !g_lsgHudKeep[id])
	{
		g_lsgHud[id] = 0
		lsg_eng_hud_put(id)
	}
	if (g_lsgFov[id] && !g_lsgFovKeep[id])
	{
		g_lsgFov[id] = 0
		lsg_eng_fov_put(id, true)
	}
	if (g_lsgTag[id] && !g_lsgTagKeep[id])
		g_lsgTag[id] = 0
}

// leaving: nothing carries over to whoever gets the slot next
public lsg_eng_left(id)
{
	if (id < 1 || id > 32)
		return FMRES_IGNORED
	g_lsgHud[id] = 0
	g_lsgHudSet[id] = 0
	g_lsgFov[id] = 0
	g_lsgNoLight[id] = false
	g_lsgQuiet[id] = false
	g_lsgTag[id] = 0
	g_lsgShowTeam[id] = 0
	g_lsgSpec[id] = 0
	for (new k = 0; k < 33; k++)
	{
		g_lsgVm[id][k] = 0
		g_lsgPm[id][k] = 0
	}
	for (new p = 1; p <= 32; p++)
		if (g_lsgSpec[p] == id)
			g_lsgSpec[p] = 0
	return FMRES_IGNORED
}

// Custom Event: Gate Opened
stock lsg_ce0(a0, Float:a1, const a2[])
{
	{
		new ctx[LSG_CTX]
		ctx[8] = _:a0
		ctx[9] = _:a1
		copy(g_lsgOT[1], 127, a2)
		lsg_n99(ctx)
	}
}

// Custom Event: Tick
stock lsg_ce1()
{
	{
		new ctx[LSG_CTX]
		lsg_n1(ctx)
	}
}

// ---- custom events the map fires: ls_event entities and info_targets named lsev_<name> ----
new const g_lsgEvNames[][] = { "gate_opened" }
#define LSG_EV_MAX 64
new g_lsgEvN, g_lsgEvEnt[LSG_EV_MAX], g_lsgEvK[LSG_EV_MAX], g_lsgEvA[LSG_EV_MAX][32], g_lsgEvB[LSG_EV_MAX][32]
new g_lsgEvParse, bool:g_lsgEvPend, g_lsgEvName[32], g_lsgEvTn[40], g_lsgEvVa[32], g_lsgEvVb[32]

public lsg_ev_kv(ent, kvd)
{
	if (ent <= 0)
		return FMRES_IGNORED
	new key[32], val[64]
	get_kvd(kvd, KV_KeyName, key, charsmax(key))
	get_kvd(kvd, KV_Value, val, charsmax(val))
	if (equal(key, "classname"))
	{
		if (ent != g_lsgEvParse)
		{
			g_lsgEvParse = ent
			g_lsgEvPend = equal(val, "ls_event") || equal(val, "info_target")
			g_lsgEvName[0] = g_lsgEvTn[0] = g_lsgEvVa[0] = g_lsgEvVb[0] = 0
		}
		if (g_lsgEvPend && equal(val, "ls_event"))
			set_kvd(kvd, KV_Value, "info_target")
		return FMRES_IGNORED
	}
	if (!g_lsgEvPend || ent != g_lsgEvParse)
		return FMRES_IGNORED
	if (equal(key, "event"))
		copy(g_lsgEvName, charsmax(g_lsgEvName), val)
	else if (equal(key, "targetname"))
		copy(g_lsgEvTn, charsmax(g_lsgEvTn), val)
	else if (equal(key, "a"))
		copy(g_lsgEvVa, charsmax(g_lsgEvVa), val)
	else if (equal(key, "b"))
		copy(g_lsgEvVb, charsmax(g_lsgEvVb), val)
	return FMRES_IGNORED
}

public lsg_ev_spawn(ent)
{
	if (ent != g_lsgEvParse || !g_lsgEvPend)
		return FMRES_IGNORED
	g_lsgEvPend = false
	g_lsgEvParse = 0
	// the event's name: its event key, else the name after lsev_
	new name[32]
	if (g_lsgEvName[0])
		copy(name, charsmax(name), g_lsgEvName)
	else if (equal(g_lsgEvTn, "lsev_", 5))
		copy(name, charsmax(name), g_lsgEvTn[5])
	strtolower(name)
	for (new k = 0; k < sizeof g_lsgEvNames; k++)
	{
		if (name[0] && equal(name, g_lsgEvNames[k]) && g_lsgEvN < LSG_EV_MAX && pev_valid(ent))
		{
			g_lsgEvEnt[g_lsgEvN] = ent
			g_lsgEvK[g_lsgEvN] = k
			copy(g_lsgEvA[g_lsgEvN], 31, g_lsgEvVa)
			copy(g_lsgEvB[g_lsgEvN], 31, g_lsgEvVb)
			g_lsgEvN++
			break
		}
	}
	return FMRES_IGNORED
}

public lsg_ev_use(ent, caller, activator)
{
	new k = -1, va[32], vb[32]
	for (new i = 0; i < g_lsgEvN; i++)
	{
		if (g_lsgEvEnt[i] == ent)
		{
			k = g_lsgEvK[i]
			copy(va, charsmax(va), g_lsgEvA[i])
			copy(vb, charsmax(vb), g_lsgEvB[i])
			break
		}
	}
	// (one made while playing, by another plugin: by its name, lsev_<event>, without values)
	if (k < 0 && pev_valid(ent))
	{
		new tn[40]
		pev(ent, pev_targetname, tn, charsmax(tn))
		if (equal(tn, "lsev_", 5))
		{
			strtolower(tn)
			for (new j = 0; j < sizeof g_lsgEvNames; j++)
				if (equal(tn[5], g_lsgEvNames[j]))
					k = j
		}
	}
	if (k < 0)
		return HAM_IGNORED
	// who set it off: Ham_Use's first value is really the activator (the engine's Use(pActivator,
	// pCaller): ham_const.inc's names are the wrong way round), the second the trigger; take
	// whichever is a player (QA4-B: it was always nobody)
	new who = caller >= 1 && caller <= MaxClients && is_user_connected(caller) ? caller : activator >= 1 && activator <= MaxClients && is_user_connected(activator) ? activator : 0
	switch (k)
	{
		case 0: lsg_ce0(who, str_to_float(va), vb)
	}
	#pragma unused va, vb, who
	return HAM_IGNORED
}


// ---- creature control ----
stock Float:g_lsgCrPos[2048][3], Float:g_lsgCrStill[2048], g_lsgCrSerial[2048], g_lsgCrSeen[2048]
stock bool:g_lsgCrGoalOn[2048], Float:g_lsgCrGoal[2048][3]

// alive, not friendly (a turret), of the type (empty: any)
stock bool:lsg_cr_match(e, const type[])
{
	if (!lsg_is_creature(e) || pev(e, pev_deadflag) != DEAD_NO || pev(e, pev_team) == 1)
		return false
	new Float:hp
	pev(e, pev_health, hp)
	if (hp <= 0.0)
		return false
	if (!type[0])
		return true
	new t[24]
	pev(e, pev_netname, t, charsmax(t))
	return bool:equali(t, type)
}

stock lsg_cr_cmd(e, const fmt[])
{
	if (lsg_cr_match(e, "") && get_cvar_pointer("ls_creature_ver"))
		server_cmd(fmt, e)
}

stock lsg_cr_all(const type[])
{
	new a = lsg_atmp(), e = -1, n
	while (n < 512 && (e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (!lsg_cr_match(e, type))
			continue
		ArrayPushCell((Array:a), e)
		n++
	}
	return a
}

stock lsg_cr_nearest(const type[], pos, to)
{
	new Float:at[3]
	if (pos >= 0)
		lsg_vto(pos, at)
	else if (to > 0 && pev_valid(to))
		pev(to, pev_origin, at)
	else
		return 0
	new e = -1, best, Float:bd = 999999.0, Float:o[3]
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e == to || !lsg_cr_match(e, type))
			continue
		pev(e, pev_origin, o)
		new Float:d = get_distance_f(o, at)
		if (d < bd)
		{
			bd = d
			best = e
		}
	}
	return best
}

stock lsg_cr_type(e)
{
	new t[24]
	if (lsg_is_creature(e))
		pev(e, pev_netname, t, charsmax(t))
	return t
}

stock Float:lsg_cr_idle(e)
{
	if (e <= 0 || e >= 2048 || !lsg_cr_match(e, "") || g_lsgCrSeen[e] != g_lsgCrSerial[e] || g_lsgCrStill[e] <= 0.0)
		return 0.0
	return get_gametime() - g_lsgCrStill[e]
}

// a clear line from its eyes to the thing's middle (or the thing itself in the way)
stock bool:lsg_cr_sees(e, what)
{
	if (!lsg_cr_match(e, "") || what <= 0 || !pev_valid(what))
		return false
	new Float:a[3], Float:v[3], Float:b[3]
	pev(e, pev_origin, a)
	pev(e, pev_view_ofs, v)
	a[2] += v[2] > 0.0 ? v[2] : 24.0
	pev(what, pev_origin, b)
	if (what <= MaxClients)
	{
		pev(what, pev_view_ofs, v)
		b[2] += v[2]
	}
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, a, b, IGNORE_MONSTERS, e, tr)
	new Float:f
	get_tr2(tr, TR_flFraction, f)
	new hit = get_tr2(tr, TR_pHit)
	free_tr2(tr)
	return f >= 1.0 || hit == what
}

stock bool:lsg_cr_sees_any(e)
{
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_alive(p) && lsg_cr_sees(e, p))
			return true
	return false
}

// how: 0 guard there, 1 then resume hunting, 2 flee there
stock lsg_cr_goto(e, to, pos, const name[], how)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:t[3]
	if (pos >= 0)
		lsg_vto(pos, t)
	else
	{
		if (to <= 0 && name[0])
			to = find_ent_by_tname(-1, name)
		if (to <= 0 || !pev_valid(to))
			return
		// (a brush entity, a gate: its middle)
		new Float:mn[3], Float:mx[3]
		pev(to, pev_absmin, mn)
		pev(to, pev_absmax, mx)
		for (new k = 0; k < 3; k++)
			t[k] = (mn[k] + mx[k]) / 2.0
	}
	server_cmd("ls_creature behave %d %s %.0f %.0f %.0f;ls_creature target %d 0", e, how == 2 ? "flee" : "guard", t[0], t[1], t[2], e)
	g_lsgCrGoalOn[e] = how == 1
	g_lsgCrGoal[e] = t
	g_lsgCrSerial[e]++
}

stock lsg_cr_attack(e, t)
{
	if (!lsg_cr_match(e, "") || t <= 0 || !pev_valid(t) || !get_cvar_pointer("ls_creature_ver"))
		return
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature target %d %d;ls_creature behave %d chase", e, t, e)
}

// stays where it is (guarding its spot: it still fights what comes close, then comes back)
stock lsg_cr_hold(e)
{
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	new Float:o[3]
	pev(e, pev_origin, o)
	g_lsgCrGoalOn[e] = false
	server_cmd("ls_creature behave %d guard %.0f %.0f %.0f;ls_creature target %d 0", e, o[0], o[1], o[2], e)
}

// creature control 2 in the bot brain: state, target, default behavior, speed, tier
stock bool:lsg_cr_v2()
{
	return get_cvar_pointer("ls_creature_ver") && get_cvar_num("ls_creature_ver") >= 2
}

// back to its own AI: its type's spawn behavior, its spawn spot as home, no forced target
// (an older brain has no "default": it hunts)
stock lsg_cr_resume(e)
{
	if (e > 0 && e < 2048)
		g_lsgCrGoalOn[e] = false
	if (!lsg_cr_match(e, "") || !get_cvar_pointer("ls_creature_ver"))
		return
	if (lsg_cr_v2())
		server_cmd("ls_creature behave %d default", e)
	else
		server_cmd("ls_creature behave %d chase;ls_creature target %d 0", e, e)
}

// 1 idle, 2 wander, 3 patrol, 4 guard, 5 chase, 6 flee, 7 attack, 8 search, 9 dead; 0 not a creature
stock lsg_cr_state(e)
{
	if (!lsg_cr_v2() || !lsg_is_creature(e))
		return 0
	new s = pev(e, pev_iuser3)
	return s >= 1 && s <= 9 ? s : 0
}

stock lsg_cr_state_name(e)
{
	static const names[][] = { "", "idle", "wander", "patrol", "guard", "chase", "flee", "attack", "search", "dead" }
	new t[8]
	copy(t, charsmax(t), names[lsg_cr_state(e)])
	return t
}

stock lsg_cr_target(e)
{
	if (!lsg_cr_v2() || !lsg_cr_match(e, ""))
		return 0
	new t = pev(e, pev_enemy)
	return t > 0 && pev_valid(t) ? t : 0
}

stock lsg_cr_speed(e, Float:x)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature speed %d %.2f", e, floatclamp(x, 0.2, 3.0))
}

stock lsg_cr_tier(e, t)
{
	if (lsg_cr_v2() && lsg_cr_match(e, ""))
		server_cmd("ls_creature tier %d %d", e, clamp(t, 0, 3))
}

stock lsg_cr_health(e, Float:hp)
{
	if (!lsg_cr_match(e, ""))
		return
	hp = floatclamp(hp, 1.0, 100000.0)
	set_pev(e, pev_health, hp)
	set_pev(e, pev_max_health, hp)
}


stock Float:g_lsgCrNext111[2048]
// once a second: how long each creature has stood still; Go To's "then resume"; Creature Idle
public lsg_cr_tick()
{
	new Float:now = get_gametime(), e = -1, Float:o[3]
	while ((e = find_ent_by_class(e, "ls_creature")) > 0)
	{
		if (e >= 2048 || !lsg_cr_match(e, ""))
			continue
		pev(e, pev_origin, o)
		if (g_lsgCrSeen[e] != g_lsgCrSerial[e] || get_distance_f(o, g_lsgCrPos[e]) > 32.0)
		{
			g_lsgCrPos[e] = o
			g_lsgCrStill[e] = now
			g_lsgCrSeen[e] = g_lsgCrSerial[e]
		}
		if (g_lsgCrGoalOn[e] && get_distance_f(o, g_lsgCrGoal[e]) < 96.0)
		{
			g_lsgCrGoalOn[e] = false
			lsg_cr_resume(e)
		}
		if (now - g_lsgCrStill[e] >= 12.0 && now >= g_lsgCrNext111[e] && !lsg_cr_sees_any(e))
		{
			g_lsgCrNext111[e] = now + 12.0
			new ctx[LSG_CTX]
			ctx[0] = e
			ctx[1] = _:(now - g_lsgCrStill[e])
			lsg_n111(ctx)
		}
	}
}

public lsg_count_round()
	g_lsgRounds++

public lsg_team_score()
{
	new t[2]
	read_data(1, t, charsmax(t))
	g_lsgScore[t[0] == 'T' ? 1 : 2] = read_data(2)
}

public lsg_freeze_on()
	g_lsgFreeze = true

public lsg_freeze_off()
	g_lsgFreeze = false

stock lsg_mute(id, Float:secs)
{
	if (!lsg_ok(id))
		return
	set_speak(id, SPEAK_MUTED)
	remove_task(LSG_TASK - 300 - id)
	if (secs > 0.0)
		set_task(secs, "lsg_unmute", LSG_TASK - 300 - id)
}

public lsg_unmute(task)
{
	new id = LSG_TASK - 300 - task
	if (lsg_ok(id))
		set_speak(id, SPEAK_NORMAL)
}

// Kind Of: the built-in Entity Kind enum's value (Player, Bot, Creature, NPC, Interactable,
// Component, Prop, Map entity, None)
stock lsg_kind_of(e)
{
	if (e <= 0 || !pev_valid(e))
		return 8
	if (e <= MaxClients)
		return is_user_connected(e) ? (is_user_bot(e) ? 1 : 0) : 8
	if (lsg_is_creature(e))
		return 2
	if (lsg_kind_ok(e, "ls_npc"))
		return 3
	if (lsg_kind_ok(e, "ls_interactable"))
		return 4
	if (lsg_kind_ok(e, ""))
		return 5
	new c[24]
	pev(e, pev_classname, c, charsmax(c))
	return equal(c, "ls_studio_prop") ? 6 : 7
}

stock const g_lsgCompKinds[][] = { "ls_capture_point", "ls_extraction_zone", "ls_checkpoint", "ls_shop", "ls_barricade", "ls_trigger_trap", "ls_loot_container", "ls_supply_drop", "ls_supply_crate", "ls_perk_machine", "ls_upgrade_bench", "ls_mystery_box", "ls_meeting_point", "ls_moving_platform", "ls_timed_door", "ls_wave_spawner", "ls_safe_zone", "ls_zone_shrinker", "ls_damage_zone", "ls_weapon_spawner", "ls_respawn_point", "ls_player_class", "ls_pickup", "ls_npc", "ls_interactable" }
stock lsg_comp_kind_of(e)
{
	new k[24]
	if (e <= 0 || !ls_components_running() || !ls_comp_kind(e, k, charsmax(k)))
		return 25
	for (new i = 0; i < sizeof g_lsgCompKinds; i++)
		if (equal(k, g_lsgCompKinds[i]))
			return i
	return 25
}

// fire, a drunk view and trails, ten times a second (one task for all of them)
public lsg_fun_tick()
	lsg_fun_ticks()

// View From with Freeze: no shooting while viewing (moving is stopped by FL_FROZEN)
public lsg_view_cmd(id, uc, seed)
{
	if (id < 1 || id > 32 || !g_lsgViewCam[id] || !g_lsgViewFreeze[id])
		return FMRES_IGNORED
	set_uc(uc, UC_Buttons, get_uc(uc, UC_Buttons) & ~(IN_ATTACK | IN_ATTACK2 | IN_USE))
	return FMRES_HANDLED
}

// a death ends fire, burying, a drunk view, and third person when it's set to
public lsg_fun_dead(id)
	lsg_fun_died(id)

// a respawn: the trail again, third person again (or not), out of the ground
public lsg_fun_spawn(id)
	lsg_fun_spawned(id)

public lsg_cam_again(task)
	lsg_cam_reapply(task - (LSG_TASK - 480))

// a new round puts out fires, unburies and gives views back
public lsg_fun_round()
{
	for (new p = 1; p <= MaxClients; p++)
	{
		lsg_burn_off(p)
		g_lsgBuried[p] = false
		lsg_view_back(p)
	}
}

// Objective Markers follow what they're on, ten times a second (one task for all of them)
public lsg_mk_tick()
	lsg_mk_follow()

// Status Icon (until death): it goes when they die
public lsg_icon_dead(id)
	lsg_icon_clear(id)

stock lsg_gag(id, Float:secs)
{
	if (lsg_ok(id))
		g_lsgGagTill[id] = secs > 0.0 ? get_gametime() + secs : 99999999.0
}

public lsg_gag_say(id)
	return lsg_ok(id) && get_gametime() < g_lsgGagTill[id] ? PLUGIN_HANDLED : PLUGIN_CONTINUE

public plugin_precache()
{
	// a projectile the mode fires: loaded with the map
	if (lsg_pre_model("models/w_hegrenade.mdl"))
		ls_proj_precache("models/w_hegrenade.mdl")
	// an NPC the mode makes: its model is loaded with the map
	ls_npc_precache("models/barney.mdl")
	g_lsgSprExp = precache_model("sprites/zerogxplode.spr")
	g_lsgSprBeam = precache_model("sprites/laserbeam.spr")
	g_lsgSprFire = lsg_pre_model("sprites/fire.spr")
	lsg_pre_model("models/w_c4.mdl")
	lsg_pre_model("sprites/arrow1.spr")
	lsg_pre_sound("buttons/bell1.wav")
	// the map's custom event entities, as they load
	register_forward(FM_KeyValue, "lsg_ev_kv")
	register_forward(FM_Spawn, "lsg_ev_spawn", 1)
}

public plugin_init()
{
	ls_register_mode(LSG_MODE, "Every Node 3", "1.0", "Game Mode Studio")
	lsd_init()
	register_event("HLTV", "lsg_new_round", "a", "1=0", "2=0")
	register_event("HLTV", "lsg_count_round", "a", "1=0", "2=0")
	register_event("TeamScore", "lsg_team_score", "a")
	register_event("HLTV", "lsg_freeze_on", "a", "1=0", "2=0")
	register_logevent("lsg_freeze_off", 2, "1=Round_Start")
	RegisterHamPlayer(Ham_Spawn, "lsg_on_player_spawned", 1)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_hurt_before", 0)
	RegisterHamPlayer(Ham_TakeDamage, "lsg_on_player_hurt", 1)
	register_clcmd("say", "lsg_on_said")
	register_clcmd("say_team", "lsg_on_said")
	register_logevent("lsg_on_freeze_ended", 2, "1=Round_Start")
	register_event("30", "lsg_on_map_ending", "a")
	lsg_hook_weapons("lsg_on_fired", Ham_Weapon_PrimaryAttack)
	lsg_hook_weapons("lsg_on_reloaded", Ham_Weapon_Reload)
	register_logevent("lsg_on_hostage_rescued", 3, "2=Rescued_A_Hostage")
	register_logevent("lsg_on_vip_escaped", 3, "2=Escaped_As_VIP")
	RegisterHam(Ham_Use, "info_target", "lsg_ev_use", 1)
	lsg_settings_init()
	RegisterHamPlayer(Ham_TraceAttack, "lsg_hit_trace", 0)
	if (get_cvar_pointer("ls_creature_ver"))
		RegisterHam(Ham_TraceAttack, "ls_creature", "lsg_hit_trace", 0)
	set_task(1.0, "lsg_cr_tick", LSG_TASK + 60000, _, _, "b")
	if (get_cvar_pointer("ls_creature_ver") && get_cvar_num("ls_creature_ver") < 2)
		log_amx("This mode's Creature State, Target, Speed and Tier nodes need a newer Loner-Strike bot brain (creature control 2): update the server. Until then they do nothing.")
	register_logevent("lsg_tl_auto", 2, "1=Round_Start")
	register_clcmd("say", "lsg_gag_say")
	register_clcmd("say_team", "lsg_gag_say")
	set_task(0.1, "lsg_mk_tick", LSG_TASK - 400, _, _, "b")
	set_task(0.1, "lsg_fun_tick", LSG_TASK - 410, _, _, "b")
	register_forward(FM_CmdStart, "lsg_view_cmd", 0)
	RegisterHamPlayer(Ham_Killed, "lsg_fun_dead", 1)
	RegisterHamPlayer(Ham_Spawn, "lsg_fun_spawn", 1)
	register_event("HLTV", "lsg_fun_round", "a", "1=0", "2=0")
	RegisterHamPlayer(Ham_Killed, "lsg_icon_dead", 1)
	RegisterHamPlayer(Ham_Spawn, "lsg_eng_spawn", 1)
	RegisterHamPlayer(Ham_Killed, "lsg_eng_dead", 1)
	register_event("ResetHUD", "lsg_eng_reset", "be")
	set_task(0.25, "lsg_eng_tick", LSG_TASK - 520, _, _, "b")
	RegisterHam(Ham_Item_Deploy, "weapon_knife", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_glock18", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_usp", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_p228", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_deagle", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_elite", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_fiveseven", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m3", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_xm1014", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_mac10", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_tmp", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_mp5navy", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_ump45", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_p90", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_galil", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_famas", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_ak47", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m4a1", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_sg552", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_aug", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_scout", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_awp", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_g3sg1", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_sg550", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_m249", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_hegrenade", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_flashbang", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_smokegrenade", "lsg_eng_deploy", 1)
	RegisterHam(Ham_Item_Deploy, "weapon_c4", "lsg_eng_deploy", 1)
	register_impulse(100, "lsg_eng_impulse")
	register_forward(FM_PlayerPreThink, "lsg_eng_think", 0)
	register_message(get_user_msgid("ScoreAttrib"), "lsg_eng_attr")
	register_message(get_user_msgid("TeamInfo"), "lsg_eng_teaminfo")
	register_forward(FM_ClientDisconnect, "lsg_eng_left", 0)
}

// Timelines with AutoPlay
public lsg_tl_auto()
{
	new c[LSG_CTX]
	lsg_tl113_save(0, c)
	lsg_tl113_go(0, 0.0, 1)
}

// the host's values for the mode settings, as the map starts
public plugin_cfg()
{
	lsg_settings_load()
	lsg_settings_apply(true)
}

// a new round: timers stop, round variables start again
public lsg_new_round()
{
	lsg_stop_timers()
	lsg_settings_load()
	lsg_settings_apply(false)
	g_v1 = floatround(lsg_sv(1)) != 0
}

public plugin_end()
{
	lsd_end()
}

stock lsg_stop_timers()
{
	for (new i = 0; i < LSG_N; i++)
		remove_task(LSG_TASK + i)
}

public client_disconnected(id)
{
	lsg_fun_left(id)
}

public lsg_on_player_spawned(id)
{
	if (!is_user_alive(id))
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = id // player
	lsg_n0(ctx)
	return HAM_IGNORED
}

new Float:g_lsgHurtHp[33]

public lsg_hurt_before(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused attacker
	#pragma unused damage
	#pragma unused bits
	if (victim >= 1 && victim <= MaxClients)
		pev(victim, pev_health, g_lsgHurtHp[victim])
	return HAM_IGNORED
}

public lsg_on_player_hurt(victim, inflictor, attacker, Float:damage, bits)
{
	#pragma unused inflictor
	#pragma unused bits
	if (victim < 1 || victim > MaxClients)
		return HAM_IGNORED
	new Float:hp
	pev(victim, pev_health, hp)
	damage = g_lsgHurtHp[victim] - hp
	if (damage <= 0.0)
		return HAM_IGNORED
	new ctx[LSG_CTX]
	ctx[0] = victim // victim
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:damage // damage
	ctx[3] = bits & DMG_BULLET && get_ent_data(victim, "CBaseMonster", "m_LastHitGroup") == HIT_HEAD ? 1 : 0 // headshot
	ctx[4] = _:hp // left
	copy(g_lsgTxt[0], 63, lsg_hitgroup(victim, bits)) // hitgroup
	copy(g_lsgTxt[1], 63, lsg_wpn_of(attacker, inflictor)) // weapon
	ctx[5] = lsg_hit_pos(victim) // hit_pos
	ctx[6] = lsg_hit_dir(victim, attacker >= 1 && attacker <= MaxClients ? attacker : inflictor) // hit_dir
	lsg_n2(ctx)
	return HAM_IGNORED
}

public ls_on_creature_hurt(ent, attacker, Float:dmg, headshot)
{
	new ctx[LSG_CTX]
	ctx[0] = ent // creature
	ctx[1] = attacker >= 1 && attacker <= MaxClients ? attacker : 0 // attacker
	ctx[2] = _:dmg // damage
	ctx[3] = headshot ? 1 : 0 // headshot
	ctx[4] = lsg_hit_pos(ent) // hit_pos
	ctx[5] = lsg_hit_dir(ent, attacker) // hit_dir
	lsg_n4(ctx)
}

public ls_on_npc_talk(id, npc)
{
	if (id < 1 || id > MaxClients)
		return PLUGIN_CONTINUE
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = npc // npc
	lsg_n6(ctx)
	return PLUGIN_CONTINUE
}

public ls_on_interact(id, ent)
{
	if (id < 1 || id > MaxClients)
		return PLUGIN_CONTINUE
	new ctx[LSG_CTX]
	ctx[0] = id // player
	ctx[1] = ent // entity
	lsg_n7(ctx)
	return PLUGIN_CONTINUE
}

public ls_on_proj_hit(proj, owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	#pragma unused pos
	#pragma unused normal
	new ctx[LSG_CTX]
	ctx[0] = proj // proj
	ctx[1] = owner // owner
	ctx[2] = lsg_vfrom(pos) // pos
	ctx[3] = lsg_vfrom(normal) // normal
	ctx[4] = hit // hit
	ctx[5] = (hit == 0 ? 1 : 0) // world
	ctx[6] = _:float(hitgroup) // hitgroup
	lsg_n15(ctx)
}

public lsg_on_said(id)
{
	new txt[128], cmd[12]
	read_argv(0, cmd, charsmax(cmd))
	read_args(txt, charsmax(txt))
	remove_quotes(txt)
	if (!lsg_ok(id))
		return PLUGIN_CONTINUE
	new ctx[LSG_CTX]
	ctx[0] = id // player
	copy(g_lsgTxt[0], 63, txt) // text
	ctx[1] = equal(cmd, "say_team") ? 1 : 0 // team
	lsg_n90(ctx)
	return PLUGIN_CONTINUE
}

public lsg_on_freeze_ended()
{
	new ctx[LSG_CTX]
	lsg_n91(ctx)
}

public lsg_on_map_ending()
{
	new ctx[LSG_CTX]
	lsg_n92(ctx)
}

public lsg_on_fired(wpn)
{
	static Float:last[33]
	new id = pev_valid(wpn) ? pev(wpn, pev_owner) : 0, wn[24]
	if (!lsg_ok(id) || get_gametime() - last[id] < 0.1)
		return HAM_IGNORED
	last[id] = get_gametime()
	pev(wpn, pev_classname, wn, charsmax(wn))
	replace(wn, charsmax(wn), "weapon_", "")
	new ctx[LSG_CTX]
	ctx[0] = id // player
	copy(g_lsgTxt[0], 63, wn) // weapon
	lsg_n93(ctx)
	return HAM_IGNORED
}

public lsg_on_reloaded(wpn)
{
	new id = pev_valid(wpn) ? pev(wpn, pev_owner) : 0, wn[24]
	if (!lsg_ok(id))
		return HAM_IGNORED
	pev(wpn, pev_classname, wn, charsmax(wn))
	replace(wn, charsmax(wn), "weapon_", "")
	new ctx[LSG_CTX]
	ctx[0] = id // player
	copy(g_lsgTxt[0], 63, wn) // weapon
	lsg_n94(ctx)
	return HAM_IGNORED
}

public lsg_on_hostage_rescued()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n95(ctx)
}

public lsg_on_vip_escaped()
{
	new ctx[LSG_CTX]
	ctx[0] = lsg_log_player() // player
	lsg_n96(ctx)
}

public ls_studio_event(const name[], a, b)
{
	new ctx[LSG_CTX]
	ctx[0] = _:float(a) // a
	ctx[1] = _:float(b) // b
	if (equal(name, "my_event"))
		lsg_n89(ctx)
}

// Player Spawned (ps)
lsg_n0(ctx[])
{
	if (lsd_want(0))
	{
	lsd_begin()
	}
	if (lsd_enter(0, ctx))
		return
	lsd_exit(0, "ran")
	lsg_n8(ctx)
}

// Custom Event (tick)
lsg_n1(ctx[])
{
	if (lsd_want(1))
	{
	lsd_begin()
	}
	if (lsd_enter(1, ctx))
		return
	lsd_exit(1, "called")
}

// Player Hurt (ph)
lsg_n2(ctx[])
{
	if (lsd_want(2))
	{
	lsd_begin()
	}
	if (lsd_enter(2, ctx))
		return
	lsd_exit(2, "ran")
	lsg_n3(ctx)
}

// Explosion (phx)
lsg_n3(ctx[])
{
	new lsi0 = ctx[5]
	new Float:lsi1 = 0.0
	if (lsd_want(3))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	lsd_in_number("Damage", lsi1)
	}
	if (lsd_enter(3, ctx))
		return
	lsg_explode(lsi0, lsi1, 250.0)
	lsd_exit(3, "done")
}

// Creature Hurt (ch)
lsg_n4(ctx[])
{
	if (lsd_want(4))
	{
	lsd_begin()
	}
	if (lsd_enter(4, ctx))
		return
	lsd_exit(4, "ran")
	lsg_n5(ctx)
}

// Print String (chp)
lsg_n5(ctx[])
{
	new lsi0 = ctx[5]
	if (lsd_want(5))
	{
	lsd_begin()
	lsd_in_text("Value", lsg_vtext(lsi0))
	}
	if (lsd_enter(5, ctx))
		return
	new t[192]
	formatex(t, charsmax(t), "%s %s", "[0]", lsg_vtext(lsi0))
	server_print("[%s] %s", LSG_MODE, t)
	client_print(0, print_chat, "%s", t)
	new esc[200]
	lsd_esc(t, esc, charsmax(esc))
	lsd_exit(5, esc)
}

// Talked To NPC (n0)
lsg_n6(ctx[])
{
	if (lsd_want(6))
	{
	lsd_begin()
	}
	if (lsd_enter(6, ctx))
		return
	lsd_exit(6, "ran")
}

// Used Interactable (n1)
lsg_n7(ctx[])
{
	if (lsd_want(7))
	{
	lsd_begin()
	}
	if (lsd_enter(7, ctx))
		return
	lsd_exit(7, "ran")
}

// Say and Choose (n2)
lsg_n8(ctx[])
{
	new lsi0 = ctx[0]
	new lsi2[128]
	copy(lsi2, 127, "")
	new lsi3[128]
	copy(lsi3, 127, "Can you help me?")
	new lsi4[128]
	copy(lsi4, 127, "Yes")
	new lsi5[128]
	copy(lsi5, 127, "No")
	new lsi6[128]
	copy(lsi6, 127, "")
	new lsi7[128]
	copy(lsi7, 127, "")
	new lsi8[128]
	copy(lsi8, 127, "")
	new lsi9[128]
	copy(lsi9, 127, "")
	new Float:lsi10 = 6.0
	new Float:lsi11 = 30.0
	if (lsd_want(8))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Speaker", lsi2)
	lsd_in_text("Line", lsi3)
	lsd_in_text("Reply 1", lsi4)
	lsd_in_text("Reply 2", lsi5)
	lsd_in_text("Reply 3", lsi6)
	lsd_in_text("Reply 4", lsi7)
	lsd_in_text("Reply 5", lsi8)
	lsd_in_text("Reply 6", lsi9)
	lsd_in_number("Replies", lsi10)
	lsd_in_number("Time to answer", lsi11)
	}
	if (lsd_enter(8, ctx))
		return
	new lsgL[192], lsgR[256], lsgN = 0
	if (!ls_components_running() || !lsg_ok(lsi0))
		lsd_error(8, "Not asked: nobody there, or gm_components isn't running.")
	else
	{
		new lsgC = clamp(floatround(lsi10), 1, 6)
		lsg_npc_line(lsgL, charsmax(lsgL), lsi2, lsi3)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 1, lsi4)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 2, lsi5)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 3, lsi6)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 4, lsi7)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 5, lsi8)
		lsg_npc_rep(lsgR, charsmax(lsgR), lsi0, lsgN, lsgC, 6, lsi9)
		for (new k = 0; k < LSG_CTX; k++)
			g_lsgM8[lsi0][k] = ctx[k]
		if (lsgN && ls_npc_choose(0, lsi0, lsgL, lsgR, floatclamp(lsi11, 0.0, 300.0)))
			g_lsgAsk[lsi0] = 9
		lsd_exit(8, "asked")
	}
}

// Say and Choose (n2): the reply
stock lsg_ask8(id, choice)
{
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = g_lsgM8[id][k]
	new r = choice >= 0 && choice < 6 ? g_lsgAskK[id][choice] : 0
	g_lsgO[0] = _:float(r)
	switch (choice < 0 ? choice : r)
	{
		case 1:
		{
			lsd_exit(8, "reply 1")
			lsg_n9(ctx)
		}
		case 2:
		{
			lsd_exit(8, "reply 2")
			#pragma unused ctx
		}
		case 3:
		{
			lsd_exit(8, "reply 3")
			#pragma unused ctx
		}
		case 4:
		{
			lsd_exit(8, "reply 4")
			#pragma unused ctx
		}
		case 5:
		{
			lsd_exit(8, "reply 5")
			#pragma unused ctx
		}
		case 6:
		{
			lsd_exit(8, "reply 6")
			#pragma unused ctx
		}
		case -1:
		{
			lsd_exit(8, "timed out")
			#pragma unused ctx
		}
		case -2, 0:
		{
			lsd_exit(8, "walked away")
			#pragma unused ctx
		}
	}
}

// Say (n3)
lsg_n9(ctx[])
{
	new lsi2[128]
	copy(lsi2, 127, "")
	new lsi3[128]
	copy(lsi3, 127, "Hello there.")
	if (lsd_want(9))
	{
	lsd_begin()
	lsd_in_text("Speaker", lsi2)
	lsd_in_text("Line", lsi3)
	}
	if (lsd_enter(9, ctx))
		return
	new lsgL[192]
	if (!ls_components_running())
		lsd_error(9, "gm_components isn't running on this server.")
	else
	{
		lsg_npc_line(lsgL, charsmax(lsgL), lsi2, lsi3)
		ls_npc_say(0, 0, lsgL)
		lsd_exit(9, "said")
	}
	lsg_n10(ctx)
}

// Set NPC Behavior (n4)
lsg_n10(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(10))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	}
	if (lsd_enter(10, ctx))
		return
	if (!ls_components_running() || lsi0 <= 0)
		lsd_error(10, "Not done: no NPC wired in (or gm_components isn't running).")
	else
	{
		ls_npc_behave(lsi0, "stand", 0)
		lsd_exit(10, "done")
	}
	lsg_n11(ctx)
}

// NPC Look At (n5)
lsg_n11(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new lsi1 = lsg_find("spot1")
	if (lsd_want(11))
	{
	lsd_begin()
	lsd_in_entity("NPC", lsi0)
	lsd_in_entity("Target", lsi1)
	}
	if (lsd_enter(11, ctx))
		return
	if (!ls_components_running() || lsi0 <= 0)
		lsd_error(11, "Not done: no NPC wired in (or gm_components isn't running).")
	else
	{
		ls_npc_look_at(lsi0, lsi1)
		lsd_exit(11, "done")
	}
	lsg_n12(ctx)
}

// Spawn NPC (n6)
lsg_n12(ctx[])
{
	new Float:lsi6 = 1.0
	if (lsd_want(12))
	{
	lsd_begin()
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(12, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(12, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("", o))
		lsd_error(12, "No entity has that name on this map.")
	else
	{
		g_lsgOut[12] = ls_npc_spawn("models/barney.mdl", o, 0.0, "Guide", "behavior=stand unkillable=1")
		if (g_lsgOut[12])
			lsd_exit(12, fmt("entity %d", g_lsgOut[12]))
		else
			lsd_error(12, "Not made: its model isn't loaded with this map, or there's no room.")
	}
	lsg_n13(ctx)
}

// Spawn Interactable (n7)
lsg_n13(ctx[])
{
	new Float:lsi6 = 1.0
	if (lsd_want(13))
	{
	lsd_begin()
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(13, ctx))
		return
	new Float:o[3]
	if (!ls_components_running())
		lsd_error(13, "gm_components isn't running on this server.")
	else if (!lsg_origin_of("", o))
		lsd_error(13, "No entity has that name on this map.")
	else
	{
		g_lsgOut[13] = ls_interactable_spawn("models/w_weaponbox.mdl", o, "Press E to use", "once=1 hide=0 bots=0")
		if (g_lsgOut[13])
			lsd_exit(13, fmt("entity %d", g_lsgOut[13]))
		else
			lsd_error(13, "Not made: its model isn't loaded with this map, or there's no room.")
	}
	lsg_n14(ctx)
}

// Spawn Projectile (n8)
lsg_n14(ctx[])
{
	new Float:lsi7 = 1000.0
	new Float:lsi8 = 0.0
	new Float:lsi9 = 1.0
	if (lsd_want(14))
	{
	lsd_begin()
	lsd_in_number("Speed", lsi7)
	lsd_in_number("Damage", lsi8)
	lsd_in_number("Scale", lsi9)
	}
	if (lsd_enter(14, ctx))
		return
	new Float:o[3], Float:d[3], lsgK[160], bool:lsgA = true
	lsgA = lsg_proj_from(0, 16.0, 0.0, 0.0, -1, o, d)
	formatex(lsgK, charsmax(lsgK), "trail=0 spin=0 bounce=0 radius=0 falloff=1 explode=0 dangerous=0 damage=%.0f scale=%.2f", floatclamp(lsi8, 0.0, 5000.0), floatclamp(lsi9, 0.05, 4.0))
	if (get_cvar_num("ls_proj_max") != 64)
		set_cvar_num("ls_proj_max", 64)
	if (!ls_components_running())
		lsd_error(14, "gm_components isn't running on this server.")
	else if (!lsgA)
		lsd_error(14, "Not fired: no Position, and no At entity or living owner to fire from.")
	else
	{
		g_lsgOut[14] = (lsg_ent_room("Spawn Projectile") ? ls_proj_spawn(0, "models/w_hegrenade.mdl", o, d, floatclamp(lsi7, 1.0, 5000.0), 0.0, 5.0, lsgK) : 0)
		if (g_lsgOut[14])
			lsg_pj_note(g_lsgOut[14], 15, ctx)
		if (g_lsgOut[14])
			lsd_exit(14, fmt("entity %d", g_lsgOut[14]))
		else
			lsd_error(14, "Spawn Projectile skipped: the server is out of entity slots.")
	}
	lsg_n16(ctx)
}

// Spawn Projectile (n8): its hits and its end
stock lsg_pjhit14(ctx[], owner, const Float:pos[3], const Float:normal[3], hit, hitgroup)
{
	#pragma unused owner
	#pragma unused pos
	#pragma unused normal
	#pragma unused hit
	#pragma unused hitgroup
	lsd_exit(14, "hit")
	#pragma unused ctx
}

stock lsg_pjexp14(ctx[])
{
	lsd_exit(14, "expired")
	#pragma unused ctx
}

// Projectile Hit (n9)
lsg_n15(ctx[])
{
	if (lsd_want(15))
	{
	lsd_begin()
	}
	if (lsd_enter(15, ctx))
		return
	lsd_exit(15, "ran")
}

// Remove Projectile (n10)
lsg_n16(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(16))
	{
	lsd_begin()
	lsd_in_entity("Projectile", lsi0)
	}
	if (lsd_enter(16, ctx))
		return
	if (!ls_components_running() || lsi0 <= 0)
		lsd_error(16, "Not done: no projectile wired in (or gm_components isn't running).")
	else
	{
		ls_proj_remove(lsi0)
		lsd_exit(16, "done")
	}
	lsg_n17(ctx)
}

// Set Projectile Velocity (n12)
lsg_n17(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new lsi1 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(17))
	{
	lsd_begin()
	lsd_in_entity("Projectile", lsi0)
	lsd_in_text("Velocity", lsg_vtext(lsi1))
	}
	if (lsd_enter(17, ctx))
		return
	if (!ls_components_running() || lsi0 <= 0)
		lsd_error(17, "Not done: no projectile wired in (or gm_components isn't running).")
	else
	{
		new Float:pv[3]
		lsg_vto(lsi1, pv)
		ls_proj_set_velocity(lsi0, pv)
		lsd_exit(17, "done")
	}
	lsg_n18(ctx)
}

// Change Map (n25)
lsg_n18(ctx[])
{
	if (lsd_want(18))
	{
	lsd_begin()
	}
	if (lsd_enter(18, ctx))
		return
	lsg_change_map("de_dust2")
	lsd_exit(18, "done")
	lsg_n19(ctx)
}

// Restart Round (n26)
lsg_n19(ctx[])
{
	if (lsd_want(19))
	{
	lsd_begin()
	}
	if (lsd_enter(19, ctx))
		return
	server_cmd("sv_restartround 1")
	lsd_exit(19, "done")
	lsg_n20(ctx)
}

// Run Server Command (n27)
lsg_n20(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, "sv_gravity 800")
	if (lsd_want(20))
	{
	lsd_begin()
	lsd_in_text("Command", lsi0)
	}
	if (lsd_enter(20, ctx))
		return
	g_lsgO[1] = lsg_safe_cmd(lsi0, 0) ? 1 : 0
	if (!lsg_cmd_ok)
		lsd_error(20, "Refused: a mode may only send game rules (mp_), gameplay sv_ settings, bot settings and commands, our plugins' gameplay commands, say and humans_join_team; AMX commands: slap, slay, kick, the say commands, amx_cvar on those settings, amx_map and amx_nextmap.")
	else
		lsd_exit(20, "sent")
	lsg_n21(ctx)
}

// Run AMX Command (n28)
lsg_n21(ctx[])
{
	new lsi0[128]
	copy(lsi0, 127, "amx_slap @T 0")
	if (lsd_want(21))
	{
	lsd_begin()
	lsd_in_text("Command", lsi0)
	}
	if (lsd_enter(21, ctx))
		return
	g_lsgO[2] = lsg_safe_cmd(lsi0, 1) ? 1 : 0
	if (!lsg_cmd_ok)
		lsd_error(21, "Refused: a mode may only send game rules (mp_), gameplay sv_ settings, bot settings and commands, our plugins' gameplay commands, say and humans_join_team; AMX commands: slap, slay, kick, the say commands, amx_cvar on those settings, amx_map and amx_nextmap.")
	else
		lsd_exit(21, "sent")
	lsg_n22(ctx)
}

// Set Gravity (n30)
lsg_n22(ctx[])
{
	new Float:lsi0 = 800.0
	if (lsd_want(22))
	{
	lsd_begin()
	lsd_in_number("Gravity", lsi0)
	}
	if (lsd_enter(22, ctx))
		return
	set_cvar_float("sv_gravity", lsi0)
	lsd_exit(22, "done")
	lsg_n23(ctx)
}

// Set Friction (n32)
lsg_n23(ctx[])
{
	new Float:lsi0 = 4.0
	if (lsd_want(23))
	{
	lsd_begin()
	lsd_in_number("Friction", lsi0)
	}
	if (lsd_enter(23, ctx))
		return
	set_cvar_float("sv_friction", lsi0)
	lsd_exit(23, "done")
	lsg_n24(ctx)
}

// Set Max Speed (n34)
lsg_n24(ctx[])
{
	new Float:lsi0 = 320.0
	if (lsd_want(24))
	{
	lsd_begin()
	lsd_in_number("Max Speed", lsi0)
	}
	if (lsd_enter(24, ctx))
		return
	set_cvar_float("sv_maxspeed", lsi0)
	lsd_exit(24, "done")
	lsg_n25(ctx)
}

// Set Round Time (n36)
lsg_n25(ctx[])
{
	new Float:lsi0 = 2.5
	if (lsd_want(25))
	{
	lsd_begin()
	lsd_in_number("Round Time", lsi0)
	}
	if (lsd_enter(25, ctx))
		return
	set_cvar_float("mp_roundtime", lsi0)
	lsd_exit(25, "done")
	lsg_n26(ctx)
}

// Set Freeze Time (n38)
lsg_n26(ctx[])
{
	new Float:lsi0 = 6.0
	if (lsd_want(26))
	{
	lsd_begin()
	lsd_in_number("Freeze Time", lsi0)
	}
	if (lsd_enter(26, ctx))
		return
	set_cvar_float("mp_freezetime", lsi0)
	lsd_exit(26, "done")
	lsg_n27(ctx)
}

// Set Buy Time (n40)
lsg_n27(ctx[])
{
	new Float:lsi0 = 0.25
	if (lsd_want(27))
	{
	lsd_begin()
	lsd_in_number("Buy Time", lsi0)
	}
	if (lsd_enter(27, ctx))
		return
	set_cvar_float("mp_buytime", lsi0)
	lsd_exit(27, "done")
	lsg_n28(ctx)
}

// Set Friendly Fire (n42)
lsg_n28(ctx[])
{
	new lsi0 = 0
	if (lsd_want(28))
	{
	lsd_begin()
	lsd_in_bool("Friendly Fire", lsi0)
	}
	if (lsd_enter(28, ctx))
		return
	set_cvar_num("mp_friendlyfire", (lsi0) ? 1 : 0)
	lsd_exit(28, "done")
	lsg_n29(ctx)
}

// Set Team Balance (n44)
lsg_n29(ctx[])
{
	new lsi0 = 1
	if (lsd_want(29))
	{
	lsd_begin()
	lsd_in_bool("Team Balance", lsi0)
	}
	if (lsd_enter(29, ctx))
		return
	set_cvar_num("mp_autoteambalance", (lsi0) ? 1 : 0)
	lsd_exit(29, "done")
	lsg_n30(ctx)
}

// Set Team Limit (n46)
lsg_n30(ctx[])
{
	new Float:lsi0 = 2.0
	if (lsd_want(30))
	{
	lsd_begin()
	lsd_in_number("Team Limit", lsi0)
	}
	if (lsd_enter(30, ctx))
		return
	set_cvar_float("mp_limitteams", lsi0)
	lsd_exit(30, "done")
	lsg_n31(ctx)
}

// Set Start Money (n48)
lsg_n31(ctx[])
{
	new Float:lsi0 = 800.0
	if (lsd_want(31))
	{
	lsd_begin()
	lsd_in_number("Start Money", lsi0)
	}
	if (lsd_enter(31, ctx))
		return
	set_cvar_float("mp_startmoney", lsi0)
	lsd_exit(31, "done")
	lsg_n32(ctx)
}

// Set C4 Timer (n50)
lsg_n32(ctx[])
{
	new Float:lsi0 = 35.0
	if (lsd_want(32))
	{
	lsd_begin()
	lsd_in_number("C4 Timer", lsi0)
	}
	if (lsd_enter(32, ctx))
		return
	set_cvar_float("mp_c4timer", lsi0)
	lsd_exit(32, "done")
	lsg_n33(ctx)
}

// Set Fall Damage (n52)
lsg_n33(ctx[])
{
	new lsi0 = 1
	if (lsd_want(33))
	{
	lsd_begin()
	lsd_in_bool("Fall Damage", lsi0)
	}
	if (lsd_enter(33, ctx))
		return
	set_cvar_num("mp_falldamage", (lsi0) ? 1 : 0)
	lsd_exit(33, "done")
	lsg_n34(ctx)
}

// Set Footsteps (n54)
lsg_n34(ctx[])
{
	new lsi0 = 1
	if (lsd_want(34))
	{
	lsd_begin()
	lsd_in_bool("Footsteps", lsi0)
	}
	if (lsd_enter(34, ctx))
		return
	set_cvar_num("mp_footsteps", (lsi0) ? 1 : 0)
	lsd_exit(34, "done")
	lsg_n35(ctx)
}

// Set All Talk (n56)
lsg_n35(ctx[])
{
	new lsi0 = 0
	if (lsd_want(35))
	{
	lsd_begin()
	lsd_in_bool("All Talk", lsi0)
	}
	if (lsd_enter(35, ctx))
		return
	set_cvar_num("sv_alltalk", (lsi0) ? 1 : 0)
	lsd_exit(35, "done")
	lsg_n36(ctx)
}

// Set Time Limit (n58)
lsg_n36(ctx[])
{
	new Float:lsi0 = 20.0
	if (lsd_want(36))
	{
	lsd_begin()
	lsd_in_number("Time Limit", lsi0)
	}
	if (lsd_enter(36, ctx))
		return
	set_cvar_float("mp_timelimit", lsi0)
	lsd_exit(36, "done")
	lsg_n37(ctx)
}

// Set Win Limit (n60)
lsg_n37(ctx[])
{
	new Float:lsi0 = 0.0
	if (lsd_want(37))
	{
	lsd_begin()
	lsd_in_number("Win Limit", lsi0)
	}
	if (lsd_enter(37, ctx))
		return
	set_cvar_float("mp_winlimit", lsi0)
	lsd_exit(37, "done")
	lsg_n38(ctx)
}

// Set Frag Limit (n62)
lsg_n38(ctx[])
{
	new Float:lsi0 = 0.0
	if (lsd_want(38))
	{
	lsd_begin()
	lsd_in_number("Frag Limit", lsi0)
	}
	if (lsd_enter(38, ctx))
		return
	set_cvar_float("mp_fraglimit", lsi0)
	lsd_exit(38, "done")
	lsg_n39(ctx)
}

// Set Transform (n82)
lsg_n39(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(39))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(39, ctx))
		return
	lsg_set_transform(lsi0, -1, -1, -1)
	lsd_exit(39, "done")
	lsg_n40(ctx)
}

// Set Rotation (n83)
lsg_n40(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(40))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_text("Rotation", lsg_vtext(lsi1))
	}
	if (lsd_enter(40, ctx))
		return
	lsg_set_transform(lsi0, -1, lsi1, -1)
	lsd_exit(40, "done")
	lsg_n41(ctx)
}

// Look At (n84)
lsg_n41(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(41))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(41, ctx))
		return
	lsg_look_at(lsi0, 0, -1)
	lsd_exit(41, "done")
	lsg_n42(ctx)
}

// Slap (n85)
lsg_n42(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 5.0
	if (lsd_want(42))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Damage", lsi1)
	}
	if (lsd_enter(42, ctx))
		return
	lsg_slap(lsi0, floatround(lsi1))
	lsd_exit(42, "done")
	lsg_n43(ctx)
}

// Screen Fade (n86)
lsg_n43(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(43))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(43, ctx))
		return
	lsg_fade(lsi0, 2.0, 255, 255, 255, 255)
	lsd_exit(43, "done")
	lsg_n44(ctx)
}

// Screen Shake (n87)
lsg_n44(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(44))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(44, ctx))
		return
	lsg_shake(lsi0, 8.0, 2.0)
	lsd_exit(44, "done")
	lsg_n45(ctx)
}

// God Mode (n88)
lsg_n45(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(45))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(45, ctx))
		return
	lsg_godmode(lsi0, 1)
	lsd_exit(45, "done")
	lsg_n46(ctx)
}

// No Clip (n89)
lsg_n46(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(46))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(46, ctx))
		return
	lsg_noclip(lsi0, 1)
	lsd_exit(46, "done")
	lsg_n47(ctx)
}

// Set Visibility (n90)
lsg_n47(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(47))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(47, ctx))
		return
	lsg_render(lsi0, 2, 80)
	lsd_exit(47, "done")
	lsg_n48(ctx)
}

// Set Ammo (n91)
lsg_n48(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 30.0
	new Float:lsi2 = 90.0
	if (lsd_want(48))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Magazine", lsi1)
	lsd_in_number("Spare", lsi2)
	}
	if (lsd_enter(48, ctx))
		return
	lsg_set_ammo(lsi0, floatround(lsi1), floatround(lsi2))
	lsd_exit(48, "done")
	lsg_n49(ctx)
}

// Give Ammo (n92)
lsg_n49(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 30.0
	if (lsd_want(49))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Bullets", lsi1)
	}
	if (lsd_enter(49, ctx))
		return
	lsg_give_ammo(lsi0, floatround(lsi1))
	lsd_exit(49, "done")
	lsg_n50(ctx)
}

// Remove Weapon (n93)
lsg_n50(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(50))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(50, ctx))
		return
	lsg_take_weapon(lsi0, "weapon_ak47")
	lsd_exit(50, "done")
	lsg_n51(ctx)
}

// Switch To Weapon (n94)
lsg_n51(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(51))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(51, ctx))
		return
	lsg_switch_weapon(lsi0, "weapon_knife")
	lsd_exit(51, "done")
	lsg_n52(ctx)
}

// Set Kills (n95)
lsg_n52(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 0.0
	if (lsd_want(52))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Kills", lsi1)
	}
	if (lsd_enter(52, ctx))
		return
	lsg_set_frags(lsi0, floatround(lsi1))
	lsd_exit(52, "done")
	lsg_n53(ctx)
}

// Set Deaths (n96)
lsg_n53(ctx[])
{
	new lsi0 = ctx[0]
	new Float:lsi1 = 0.0
	if (lsd_want(53))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	lsd_in_number("Deaths", lsi1)
	}
	if (lsd_enter(53, ctx))
		return
	lsg_set_deaths(lsi0, floatround(lsi1))
	lsd_exit(53, "done")
	lsg_n54(ctx)
}

// Move To Spectators (n97)
lsg_n54(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(54))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(54, ctx))
		return
	lsg_to_spec(lsi0)
	lsd_exit(54, "done")
	lsg_n55(ctx)
}

// Kick (n98)
lsg_n55(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(55))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(55, ctx))
		return
	lsg_kick(lsi0, "Kicked by the mode")
	lsd_exit(55, "done")
	lsg_n56(ctx)
}

// Mute Voice (n99)
lsg_n56(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(56))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(56, ctx))
		return
	lsg_mute(lsi0, 60.0)
	lsd_exit(56, "done")
	lsg_n57(ctx)
}

// Gag Chat (n100)
lsg_n57(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(57))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(57, ctx))
		return
	lsg_gag(lsi0, 60.0)
	lsd_exit(57, "done")
	lsg_n58(ctx)
}

// Chat To Team (n101)
lsg_n58(ctx[])
{
	if (lsd_want(58))
	{
	lsd_begin()
	}
	if (lsd_enter(58, ctx))
		return
	lsg_team_chat(2, "%s", "Hold the line!")
	lsd_exit(58, "done")
	lsg_n59(ctx)
}

// Notify Admins (n102)
lsg_n59(ctx[])
{
	if (lsd_want(59))
	{
	lsd_begin()
	}
	if (lsd_enter(59, ctx))
		return
	lsg_admins_chat("%s", "Please check player:")
	lsd_exit(59, "done")
	lsg_n60(ctx)
}

// Spawn Prop (n106)
lsg_n60(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new Float:lsi6 = 1.0
	if (lsd_want(60))
	{
	lsd_begin()
	lsd_in_entity("At entity", lsi0)
	lsd_in_number("Scale", lsi6)
	}
	if (lsd_enter(60, ctx))
		return
	g_lsgOut[60] = lsg_prop(lsi0, "models/w_c4.mdl", 1)
	if (!g_lsgOut[60])
		lsd_error(60, "No prop: the model isn't loaded, or there's no such entity.")
	else
		lsd_exit(60, "spawned")
	lsg_n61(ctx)
}

// Remove Entity (n107)
lsg_n61(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(61))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(61, ctx))
		return
	lsg_remove(lsi0)
	lsd_exit(61, "done")
	lsg_n62(ctx)
}

// Set Entity Position (n108)
lsg_n62(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new lsi1 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(62))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_text("Position", lsg_vtext(lsi1))
	}
	if (lsd_enter(62, ctx))
		return
	lsg_ent_pos(lsi0, lsi1)
	lsd_exit(62, "done")
	lsg_n63(ctx)
}

// Set Entity Rotation (n109)
lsg_n63(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new lsi1 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(63))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	lsd_in_text("Rotation", lsg_vtext(lsi1))
	}
	if (lsd_enter(63, ctx))
		return
	lsg_ent_angles(lsi0, lsi1)
	lsd_exit(63, "done")
	lsg_n64(ctx)
}

// Set Entity Visibility (n110)
lsg_n64(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(64))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(64, ctx))
		return
	lsg_ent_visible(lsi0, 1)
	lsd_exit(64, "done")
	lsg_n65(ctx)
}

// Explosion (n111)
lsg_n65(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 64.0)
	new Float:lsi1 = 0.0
	if (lsd_want(65))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	lsd_in_number("Damage", lsi1)
	}
	if (lsd_enter(65, ctx))
		return
	lsg_explode(lsi0, lsi1, 250.0)
	lsd_exit(65, "done")
	lsg_n66(ctx)
}

// Beam (n112)
lsg_n66(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 64.0)
	new lsi1 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(66))
	{
	lsd_begin()
	lsd_in_text("From", lsg_vtext(lsi0))
	lsd_in_text("To", lsg_vtext(lsi1))
	}
	if (lsd_enter(66, ctx))
		return
	lsg_beam(lsi0, lsi1, 230, 30, 30, 1.0)
	lsd_exit(66, "done")
	lsg_n67(ctx)
}

// Play Sound At (n113)
lsg_n67(ctx[])
{
	new lsi0 = lsg_vec(0.0, 0.0, 64.0)
	if (lsd_want(67))
	{
	lsd_begin()
	lsd_in_text("Position", lsg_vtext(lsi0))
	}
	if (lsd_enter(67, ctx))
		return
	lsg_sound_at(lsi0, "ambience/thunder_clap.wav")
	lsd_exit(67, "done")
	lsg_n68(ctx)
}

// Play Sound To Player (n114)
lsg_n68(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(68))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(68, ctx))
		return
	lsg_sound_to(lsi0, "buttons/bell1.wav")
	lsd_exit(68, "done")
	lsg_n69(ctx)
}

// Set Light Style (n115)
lsg_n69(ctx[])
{
	if (lsd_want(69))
	{
	lsd_begin()
	}
	if (lsd_enter(69, ctx))
		return
	set_lights("#OFF")
	lsd_exit(69, "done")
	lsg_n70(ctx)
}

// Status Icon (n116)
lsg_n70(ctx[])
{
	if (lsd_want(70))
	{
	lsd_begin()
	}
	if (lsd_enter(70, ctx))
		return
	g_lsgMsgLim = 48
	lsg_icon(0, 1, "buyzone", 0, 160, 0, 1)
	lsd_exit(70, "shown")
	lsg_n71(ctx)
}

// Objective Marker (n117)
lsg_n71(ctx[])
{
	if (lsd_want(71))
	{
	lsd_begin()
	}
	if (lsd_enter(71, ctx))
		return
	new Float:lsgAt[3]
	g_lsgOut[71] = lsg_marker("sprites/arrow1.spr", 0, lsgAt, 0, 48.0, 0.5, 255, 200, 0, 1, 0, 0, 32)
	if (!g_lsgOut[71])
		lsd_error(71, "No marker: its sprite isn't loaded, nothing to follow or no position, as many are up as Objective markers at once allows (Limits), or the server is out of entity slots.")
	else
		lsd_exit(71, "made")
	lsg_n72(ctx)
}

// Remove Marker (n118)
lsg_n72(ctx[])
{
	if (lsd_want(72))
	{
	lsd_begin()
	}
	if (lsd_enter(72, ctx))
		return
	lsg_marker_remove(0)
	lsd_exit(72, "removed")
	lsg_n73(ctx)
}

// Kill Feed Line (n119)
lsg_n73(ctx[])
{
	new lsi1 = ctx[0]
	new lsi2 = 0
	if (lsd_want(73))
	{
	lsd_begin()
	lsd_in_player("Victim", lsi1)
	lsd_in_bool("Headshot", lsi2)
	}
	if (lsd_enter(73, ctx))
		return
	g_lsgMsgLim = 48
	if (!lsg_killfeed(0, lsi1, ((lsi2) ? 1 : 0), "knife"))
		lsd_error(73, "Not sent: the Victim isn't a player in the game.")
	else
		lsd_exit(73, "sent")
	lsg_n74(ctx)
}

// Set On Fire (n120)
lsg_n74(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(74))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(74, ctx))
		return
	if (!lsg_burn(lsi0, 0, 5.0, 5.0))
		lsd_error(74, "Not lit: that player isn't alive.")
	else
		lsd_exit(74, "burning")
	lsg_n75(ctx)
}

// Put Out Fire (n121)
lsg_n75(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(75))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(75, ctx))
		return
	lsg_burn_off(lsi0)
	lsd_exit(75, "put out")
	lsg_n76(ctx)
}

// Trail (n122)
lsg_n76(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(76))
	{
	lsd_begin()
	lsd_in_entity("On", lsi0)
	}
	if (lsd_enter(76, ctx))
		return
	g_lsgMsgLim = 48
	if (!lsg_trail(lsi0, 1, 0, 160, 255, 6, 20, 32))
		lsd_error(76, "No trail: nothing there, or as many trails are on as Trails at once allows (Limits).")
	else
		lsd_exit(76, "done")
	lsg_n77(ctx)
}

// Effect Burst (n123)
lsg_n77(ctx[])
{
	if (lsd_want(77))
	{
	lsd_begin()
	}
	if (lsd_enter(77, ctx))
		return
	new Float:lsgAt[3], lsgOk
	if (lsgOk && !lsg_msg_room("Effect Burst", 1, 0, 48))
		lsgOk = 0
	if (lsgOk)
		lsg_burst(0, lsgAt, 1, 255, 120, 0)
	if (!lsgOk)
		lsd_error(77, "No burst: no entity there and no Position, or too many effects this frame (Effect messages a frame, Limits).")
	else
		lsd_exit(77, "done")
	lsg_n78(ctx)
}

// Swap Places (n124)
lsg_n78(ctx[])
{
	new lsi0 = ctx[0]
	new lsi1 = ctx[0]
	if (lsd_want(78))
	{
	lsd_begin()
	lsd_in_player("Player A", lsi0)
	lsd_in_player("Player B", lsi1)
	}
	if (lsd_enter(78, ctx))
		return
	g_lsgO[3] = lsg_swap(lsi0, lsi1)
	if (!g_lsgO[3])
		lsd_error(78, "Not swapped: one isn't alive, it's the same player, or one would be stuck in a wall.")
	else
		lsd_exit(78, "swapped")
	lsg_n79(ctx)
}

// Bury (n125)
lsg_n79(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(79))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(79, ctx))
		return
	if (!lsg_bury(lsi0))
		lsd_error(79, "Not buried: that player isn't alive, or is buried already.")
	else
		lsd_exit(79, "buried")
	lsg_n80(ctx)
}

// Unbury (n126)
lsg_n80(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(80))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(80, ctx))
		return
	if (!lsg_unbury(lsi0))
		lsd_error(80, "Nothing done: that player isn't buried.")
	else
		lsd_exit(80, "out")
	lsg_n81(ctx)
}

// Set Camera (n127)
lsg_n81(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(81))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(81, ctx))
		return
	if (!lsg_camera(lsi0, 1, 1))
		lsd_error(81, "Nothing done: nobody there.")
	else
		lsd_exit(81, "set")
	lsg_n82(ctx)
}

// View From (n129)
lsg_n82(ctx[])
{
	if (lsd_want(82))
	{
	lsd_begin()
	}
	if (lsd_enter(82, ctx))
		return
	new Float:lsgA[3], Float:lsgAa[3], Float:lsgB[3], Float:lsgBa[3]
	lsg_view_from(0, 0, 0, lsgA, lsgAa, 0, lsgB, lsgBa, 0, 64.0, 3.0, 5.0, 1)
	lsd_exit(82, "viewing")
	lsg_n83(ctx)
}

// Return View (n130)
lsg_n83(ctx[])
{
	if (lsd_want(83))
	{
	lsd_begin()
	}
	if (lsd_enter(83, ctx))
		return
	lsg_view_return(0)
	lsd_exit(83, "returned")
	lsg_n84(ctx)
}

// Drunk View (n131)
lsg_n84(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(84))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(84, ctx))
		return
	if (!lsg_drunk(lsi0, 8.0, 4.0))
		lsd_error(84, "Nothing done: that player isn't alive.")
	else
		lsd_exit(84, "drunk")
	lsg_n85(ctx)
}

// As Player (n132)
lsg_n85(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(85))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(85, ctx))
		return
	new lsgE = lsi0
	if (lsgE >= 1 && lsgE <= MaxClients && is_user_connected(lsgE))
	{
		g_lsgO[4] = lsgE
		lsd_exit(85, "success")
		lsg_n86(ctx)
	}
	else
	{
		g_lsgO[4] = 0
		lsd_exit(85, "failed")
	}
}

// As Creature (n133)
lsg_n86(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(86))
	{
	lsd_begin()
	lsd_in_entity("Entity", lsi0)
	}
	if (lsd_enter(86, ctx))
		return
	new lsgE = lsi0
	if (lsg_is_creature(lsgE))
	{
		g_lsgO[5] = lsgE
		copy(g_lsgOT[0], 127, lsg_cr_type(lsgE))
		lsd_exit(86, "success")
		lsg_n87(ctx)
	}
	else
	{
		g_lsgO[5] = 0
		lsd_exit(86, "failed")
	}
}

// Call Plugin Function (n139)
lsg_n87(ctx[])
{
	new Float:lsi0 = 0.0
	new Float:lsi1 = 0.0
	if (lsd_want(87))
	{
	lsd_begin()
	lsd_in_number("Argument 1", lsi0)
	lsd_in_number("Argument 2", lsi1)
	}
	if (lsd_enter(87, ctx))
		return
	new r, called = lsg_call("gm_components.amxx", "plugin_cfg", 0, floatround(lsi0), floatround(lsi1), r)
	g_lsgO[6] = _:float(r)
	g_lsgO[7] = called
	if (!called)
		lsd_error(87, "Not called: no such plugin running, or no such public function in it.")
	else
		lsd_exit(87, "called")
	lsg_n88(ctx)
}

// Fire Custom Event (n141)
lsg_n88(ctx[])
{
	new Float:lsi0 = 0.0
	new Float:lsi1 = 0.0
	if (lsd_want(88))
	{
	lsd_begin()
	lsd_in_number("A", lsi0)
	lsd_in_number("B", lsi1)
	}
	if (lsd_enter(88, ctx))
		return
	lsg_fire_event("my_event", floatround(lsi0), floatround(lsi1))
	lsd_exit(88, "done")
	lsg_n97(ctx)
}

// On Custom Event (n142)
lsg_n89(ctx[])
{
	if (lsd_want(89))
	{
	lsd_begin()
	}
	if (lsd_enter(89, ctx))
		return
	lsd_exit(89, "ran")
}

// Player Said (n143)
lsg_n90(ctx[])
{
	if (lsd_want(90))
	{
	lsd_begin()
	}
	if (lsd_enter(90, ctx))
		return
	lsd_exit(90, "ran")
}

// Freeze Time Ended (n144)
lsg_n91(ctx[])
{
	if (lsd_want(91))
	{
	lsd_begin()
	}
	if (lsd_enter(91, ctx))
		return
	lsd_exit(91, "ran")
}

// Map Ending (n145)
lsg_n92(ctx[])
{
	if (lsd_want(92))
	{
	lsd_begin()
	}
	if (lsd_enter(92, ctx))
		return
	lsd_exit(92, "ran")
}

// Weapon Fired (n146)
lsg_n93(ctx[])
{
	if (lsd_want(93))
	{
	lsd_begin()
	}
	if (lsd_enter(93, ctx))
		return
	lsd_exit(93, "ran")
}

// Reloaded (n147)
lsg_n94(ctx[])
{
	if (lsd_want(94))
	{
	lsd_begin()
	}
	if (lsd_enter(94, ctx))
		return
	lsd_exit(94, "ran")
}

// Hostage Rescued (n148)
lsg_n95(ctx[])
{
	if (lsd_want(95))
	{
	lsd_begin()
	}
	if (lsd_enter(95, ctx))
		return
	lsd_exit(95, "ran")
}

// VIP Escaped (n149)
lsg_n96(ctx[])
{
	if (lsd_want(96))
	{
	lsd_begin()
	}
	if (lsd_enter(96, ctx))
		return
	lsd_exit(96, "ran")
}

// Revive Pay (n152)
lsg_n97(ctx[])
{
	new Float:lsi0 = 150.0
	if (lsd_want(97))
	{
	lsd_begin()
	lsd_in_number("Amount ($)", lsi0)
	}
	if (lsd_enter(97, ctx))
		return
	if (!ls_components_running())
		lsd_error(97, "gm_components isn't running on this server.")
	else
	{
		ls_down_revive_pay(clamp(floatround(lsi0), 0, 16000))
		lsd_exit(97, "done")
	}
	lsg_n98(ctx)
}

// Set Timer by Event (n153)
lsg_n98(ctx[])
{
	new Float:lsi0 = 5.0
	if (lsd_want(98))
	{
	lsd_begin()
	lsd_in_number("Seconds", lsi0)
	}
	if (lsd_enter(98, ctx))
		return
	remove_task(LSG_TASK + 98)
	set_task(floatmax(lsi0, 0.1), "lsg_te98", LSG_TASK + 98, _, _, "b")
	lsd_exit(98, "started")
	lsg_n100(ctx)
}

public lsg_te98()
{
	if (lsd_hold_timer())
		return
	lsg_ce1()
}

// Custom Event (n154)
lsg_n99(ctx[])
{
	if (lsd_want(99))
	{
	lsd_begin()
	}
	if (lsd_enter(99, ctx))
		return
	lsd_exit(99, "called")
}

// Call Custom Event (n155)
lsg_n100(ctx[])
{
	if (lsd_want(100))
	{
	lsd_begin()
	}
	if (lsd_enter(100, ctx))
		return
	lsg_ce0(0, 0.0, "")
	lsd_exit(100, "called")
	lsg_n101(ctx)
}

// For Each Creature (n156)
lsg_n101(ctx[])
{
	if (lsd_want(101))
	{
	lsd_begin()
	}
	if (lsd_enter(101, ctx))
		return
	new c[LSG_CTX], count, p = -1
	while (count < 512 && (p = find_ent_by_class(p, "ls_creature")) > 0)
	{
		if (!lsg_cr_match(p, ""))
			continue
		count++
		for (new k = 0; k < LSG_CTX; k++)
			c[k] = ctx[k]
		c[10] = p
		#pragma unused c
	}
	lsd_exit(101, lsd_times(count))
	lsg_n102(ctx)
}

// Creature Go To (n162)
lsg_n102(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(102))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	}
	if (lsd_enter(102, ctx))
		return
	lsg_cr_goto(lsi0, 0, -1, "gate1", 0)
	lsd_exit(102, "done")
	lsg_n103(ctx)
}

// Creature Attack (n163)
lsg_n103(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new lsi1 = lsg_find("spot1")
	if (lsd_want(103))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	lsd_in_entity("Target", lsi1)
	}
	if (lsd_enter(103, ctx))
		return
	lsg_cr_attack(lsi0, lsi1)
	lsd_exit(103, "done")
	lsg_n104(ctx)
}

// Creature Hold (n164)
lsg_n104(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(104))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	}
	if (lsd_enter(104, ctx))
		return
	lsg_cr_hold(lsi0)
	lsd_exit(104, "done")
	lsg_n105(ctx)
}

// Creature Resume AI (n165)
lsg_n105(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(105))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	}
	if (lsd_enter(105, ctx))
		return
	lsg_cr_resume(lsi0)
	lsd_exit(105, "done")
	lsg_n106(ctx)
}

// Set Creature Speed (n168)
lsg_n106(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new Float:lsi1 = 1.5
	if (lsd_want(106))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	lsd_in_number("Times", lsi1)
	}
	if (lsd_enter(106, ctx))
		return
	lsg_cr_speed(lsi0, lsi1)
	lsd_exit(106, "done")
	lsg_n107(ctx)
}

// Set Creature Tier (n169)
lsg_n107(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new Float:lsi1 = 2.0
	if (lsd_want(107))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	lsd_in_number("Tier", lsi1)
	}
	if (lsd_enter(107, ctx))
		return
	lsg_cr_tier(lsi0, floatround(lsi1))
	lsd_exit(107, "done")
	lsg_n108(ctx)
}

// Creature Flee To (n170)
lsg_n108(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(108))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	}
	if (lsd_enter(108, ctx))
		return
	lsg_cr_goto(lsi0, 0, -1, "", 2)
	lsd_exit(108, "done")
	lsg_n109(ctx)
}

// Set Creature Health (n171)
lsg_n109(ctx[])
{
	new lsi0 = lsg_find("spot1")
	new Float:lsi1 = 100.0
	if (lsd_want(109))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	lsd_in_number("Health", lsi1)
	}
	if (lsd_enter(109, ctx))
		return
	lsg_cr_health(lsi0, lsi1)
	lsd_exit(109, "done")
	lsg_n110(ctx)
}

// Remove Creature (n172)
lsg_n110(ctx[])
{
	new lsi0 = lsg_find("spot1")
	if (lsd_want(110))
	{
	lsd_begin()
	lsd_in_entity("Creature", lsi0)
	}
	if (lsd_enter(110, ctx))
		return
	lsg_cr_cmd(lsi0, "kill %d")
	lsd_exit(110, "done")
	lsg_n112(ctx)
}

// Creature Idle (n173)
lsg_n111(ctx[])
{
	if (lsd_want(111))
	{
	lsd_begin()
	}
	if (lsd_enter(111, ctx))
		return
	lsd_exit(111, "ran")
}

// For Each Row (n176)
lsg_n112(ctx[])
{
	if (lsd_want(112))
	{
	lsd_begin()
	}
	if (lsd_enter(112, ctx))
		return
	new c[LSG_CTX], count
	for (new r = 0; r < LSG_DT0_N; r++)
	{
		count++
		for (new j = 0; j < LSG_CTX; j++)
			c[j] = ctx[j]
		c[12] = _:lsg_dt0_0(float(r))
		c[13] = _:lsg_dt0_0(float(r))
		copy(g_lsgOT[2], 127, lsg_dt0_1(float(r)))
		c[14] = _:lsg_dt0_2(float(r))
		c[15] = _:lsg_dt0_3(float(r))
		c[11] = _:float(r)
		#pragma unused c
	}
	lsd_exit(112, lsd_times(count))
	lsg_n113(ctx)
}

// Timeline (n180)
stock lsg_n113(ctx[])
{
	if (lsd_want(113))
	{
	lsd_begin()
	}
	if (lsd_enter(113, ctx))
		return
	new inst = 0
	lsg_tl113_save(inst, ctx)
	lsg_tl113_go(inst, -1.0, 1)
	lsd_exit(113, "playing")
}

// Timeline: Platform
stock Float:g_lsgTlT113[33], g_lsgTlDir113[33], bool:g_lsgTlOn113[33], g_lsgTlC113[33][LSG_CTX], Float:g_lsgTlAt113[33], bool:g_lsgTlNew113[33]
stock lsg_tl113_save(inst, ctx[])
{
	for (new k = 0; k < LSG_CTX; k++)
		g_lsgTlC113[inst][k] = ctx[k]
}
stock lsg_tl113_go(inst, Float:from, dir)
{
	if (from >= 0.0)
		g_lsgTlT113[inst] = from
	else if (!g_lsgTlOn113[inst] && ((dir > 0 && g_lsgTlT113[inst] >= 4.0) || (dir < 0 && g_lsgTlT113[inst] <= 0.0)))
		g_lsgTlT113[inst] = dir > 0 ? 0.0 : 4.0
	g_lsgTlDir113[inst] = dir
	g_lsgTlNew113[inst] = true
	if (!g_lsgTlOn113[inst])
	{
		g_lsgTlOn113[inst] = true
		g_lsgTlAt113[inst] = get_gametime()
		set_task(0.1, "lsg_tl113_tick", LSG_TASK + 50000 + 113 * 33 + inst, _, _, "b")
	}
}
stock lsg_tl113_stop(inst)
{
	g_lsgTlOn113[inst] = false
	remove_task(LSG_TASK + 50000 + 113 * 33 + inst)
}
stock Float:lsg_tl113_v0(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 2.0)
		return 0.0 + 128.0 * lsg_tl_ease((t - 0.0) / 2.0)
	if (t <= 4.0)
		return 128.0 + -128.0 * ((t - 2.0) / 2.0)
	return 0.0
}
stock Float:lsg_tl113_v1_0(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 4.0)
		return 0.0 + 256.0 * ((t - 0.0) / 4.0)
	return 256.0
}
stock Float:lsg_tl113_v1_1(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 4.0)
		return 0.0 + 0.0 * ((t - 0.0) / 4.0)
	return 0.0
}
stock Float:lsg_tl113_v1_2(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 4.0)
		return 0.0 + 64.0 * ((t - 0.0) / 4.0)
	return 64.0
}
stock Float:lsg_tl113_v2_0(Float:t)
{
	if (t <= 0.0)
		return 255.0
	if (t <= 2.0)
		return 255.0 + -255.0 * ((t - 0.0) / 2.0)
	if (t <= 4.0)
		return 0.0 + 255.0 * lsg_tl_ease((t - 2.0) / 2.0)
	return 255.0
}
stock Float:lsg_tl113_v2_1(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 2.0)
		return 0.0 + 0.0 * ((t - 0.0) / 2.0)
	if (t <= 4.0)
		return 0.0 + 0.0 * lsg_tl_ease((t - 2.0) / 2.0)
	return 0.0
}
stock Float:lsg_tl113_v2_2(Float:t)
{
	if (t <= 0.0)
		return 0.0
	if (t <= 2.0)
		return 0.0 + 255.0 * ((t - 0.0) / 2.0)
	if (t <= 4.0)
		return 255.0 + -255.0 * lsg_tl_ease((t - 2.0) / 2.0)
	return 0.0
}
public lsg_tl113_tick(task)
{
	new inst = task - (LSG_TASK + 50000 + 113 * 33)
	if (inst < 0 || inst > 32)
		return
	if (lsd_hold_timer())
	{
		g_lsgTlAt113[inst] = get_gametime()
		return
	}
	new Float:now = get_gametime(), Float:step = floatclamp(now - g_lsgTlAt113[inst], 0.0, 0.5)
	g_lsgTlAt113[inst] = now
	new bool:fresh = g_lsgTlNew113[inst]
	g_lsgTlNew113[inst] = false
	new Float:was = g_lsgTlT113[inst], dir = g_lsgTlDir113[inst]
	new Float:t = was + step * float(dir)
	new bool:done, bool:wrapped
	if (dir > 0 && t > 4.0)
	{
		t -= 4.0
		wrapped = true
	}
	else if (dir < 0 && t < 0.0)
	{
		t += 4.0
		wrapped = true
	}
	g_lsgTlT113[inst] = t
	new c[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		c[k] = g_lsgTlC113[inst][k]
	c[16] = dir > 0 ? 1 : 0
	c[17] = _:t
	c[18] = _:lsg_tl113_v0(t)
	c[19] = lsg_vec(lsg_tl113_v1_0(t), lsg_tl113_v1_1(t), lsg_tl113_v1_2(t))
	c[20] = lsg_vec(lsg_tl113_v2_0(t), lsg_tl113_v2_1(t), lsg_tl113_v2_2(t))
	lsg_n114(c)
	static Float:dbg[33]
	if (done)
		lsd_note(113, fmt("%.2f of 4.00 s, %s; Lift: %.2f; Position: %.0f %.0f %.0f; Glow: %.0f %.0f %.0f", t, dir > 0 ? "forward" : "backward", lsg_tl113_v0(t), lsg_tl113_v1_0(t), lsg_tl113_v1_1(t), lsg_tl113_v1_2(t), lsg_tl113_v2_0(t), lsg_tl113_v2_1(t), lsg_tl113_v2_2(t)))
	else if (get_gametime() - dbg[inst] >= 0.25 && lsd_want(113, false))
	{
		dbg[inst] = get_gametime()
		lsd_exit(113, fmt("%.2f of 4.00 s, %s; Lift: %.2f; Position: %.0f %.0f %.0f; Glow: %.0f %.0f %.0f", t, dir > 0 ? "forward" : "backward", lsg_tl113_v0(t), lsg_tl113_v1_0(t), lsg_tl113_v1_1(t), lsg_tl113_v1_2(t), lsg_tl113_v2_0(t), lsg_tl113_v2_1(t), lsg_tl113_v2_2(t)))
	}
	if (done)
	{
		lsg_tl113_stop(inst)
	}
	#pragma unused wrapped, done, was, fresh
}


stock lsg_n113_play_start(ctx[])
{
	new inst = 0
	lsg_tl113_save(inst, ctx)
	lsg_tl113_go(inst, 0.0, 1)
}

stock lsg_n113_stop(ctx[])
{
	new inst = 0
	#pragma unused ctx
	lsg_tl113_stop(inst)
}

stock lsg_n113_reverse(ctx[])
{
	new inst = 0
	lsg_tl113_save(inst, ctx)
	lsg_tl113_go(inst, -1.0, -1)
}

stock lsg_n113_reverse_end(ctx[])
{
	new inst = 0
	lsg_tl113_save(inst, ctx)
	lsg_tl113_go(inst, 4.0, -1)
}

stock lsg_n113_set_time(ctx[])
{
	new inst = 0
	lsg_tl113_save(inst, ctx)
	g_lsgTlT113[inst] = floatclamp(0.0, 0.0, 4.0)
}

// Hide HUD Parts (n186)
lsg_n114(ctx[])
{
	if (lsd_want(114))
	{
	lsd_begin()
	}
	if (lsd_enter(114, ctx))
		return
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p))
			lsg_eng_hud(p, 64, false)
	lsd_exit(114, "hidden")
	lsg_n115(ctx)
}

// Set Field of View (n187)
lsg_n115(ctx[])
{
	new Float:lsi1 = 90.0
	if (lsd_want(115))
	{
	lsd_begin()
	lsd_in_number("Degrees", lsi1)
	}
	if (lsd_enter(115, ctx))
		return
	new f = 90
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p))
			lsg_eng_fov(p, f, false)
	lsd_exit(115, "set")
	lsg_n116(ctx)
}

// Set Weapon View Model (n188)
lsg_n116(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(116))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(116, ctx))
		return
	if (!lsg_eng_vm(lsi0, "-", 0, 0))
		lsd_error(116, "Not done: nobody there, nothing in hand, or that model isn't on the server.")
	else
		lsd_exit(116, "set")
	lsg_n117(ctx)
}

// Flashlight (n189)
lsg_n117(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(117))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(117, ctx))
		return
	if (!lsg_eng_light(lsi0, 1, -1))
		lsd_error(117, "Not done: that player isn't alive.")
	else
		lsd_exit(117, "done")
	lsg_n118(ctx)
}

// Night Vision (n190)
lsg_n118(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(118))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(118, ctx))
		return
	if (!lsg_eng_nvg(lsi0, 1))
		lsd_error(118, "Not done: nobody there.")
	else
		lsd_exit(118, "done")
	lsg_n119(ctx)
}

// Silent Footsteps (n191)
lsg_n119(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(119))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(119, ctx))
		return
	new p = lsi0
	if (lsg_ok(p))
		g_lsgQuiet[p] = true
	lsd_exit(119, "silent")
	lsg_n120(ctx)
}

// Scoreboard Tag (n192)
lsg_n120(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(120))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(120, ctx))
		return
	new p = lsi0
	if (lsg_ok(p))
		lsg_eng_tag(p, 4, true)
	lsd_exit(120, "done")
	lsg_n121(ctx)
}

// Scoreboard Team Color (n193)
lsg_n121(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(121))
	{
	lsd_begin()
	lsd_in_player("Player", lsi0)
	}
	if (lsd_enter(121, ctx))
		return
	new p = lsi0
	if (lsg_ok(p))
		lsg_eng_color(p, 1)
	lsd_exit(121, "done")
	lsg_n122(ctx)
}

// Force Spectate (n194)
lsg_n122(ctx[])
{
	new lsi0 = ctx[0]
	if (lsd_want(122))
	{
	lsd_begin()
	lsd_in_player("Watcher", lsi0)
	}
	if (lsd_enter(122, ctx))
		return
	if (!lsg_eng_spec(lsi0, 0))
		lsd_error(122, "Not done: the watcher is alive (only the dead and spectators can be made to watch), or Target isn't there.")
	else
		lsd_exit(122, "watching")
	lsg_n123(ctx)
}

// Play Soundtrack (n195)
lsg_n123(ctx[])
{
	if (lsd_want(123))
	{
	lsd_begin()
	}
	if (lsd_enter(123, ctx))
		return
	for (new p = 1; p <= MaxClients; p++)
		if (is_user_connected(p))
			lsg_eng_cd(p, 2, false)
	lsd_exit(123, "playing")
}

lsd_call(n, const data[])
{
	new ctx[LSG_CTX]
	for (new k = 0; k < LSG_CTX; k++)
		ctx[k] = data[k]
	switch (n)
	{
		case 0: lsg_n0(ctx)
		case 1: lsg_n1(ctx)
		case 2: lsg_n2(ctx)
		case 3: lsg_n3(ctx)
		case 4: lsg_n4(ctx)
		case 5: lsg_n5(ctx)
		case 6: lsg_n6(ctx)
		case 7: lsg_n7(ctx)
		case 8: lsg_n8(ctx)
		case 9: lsg_n9(ctx)
		case 10: lsg_n10(ctx)
		case 11: lsg_n11(ctx)
		case 12: lsg_n12(ctx)
		case 13: lsg_n13(ctx)
		case 14: lsg_n14(ctx)
		case 15: lsg_n15(ctx)
		case 16: lsg_n16(ctx)
		case 17: lsg_n17(ctx)
		case 18: lsg_n18(ctx)
		case 19: lsg_n19(ctx)
		case 20: lsg_n20(ctx)
		case 21: lsg_n21(ctx)
		case 22: lsg_n22(ctx)
		case 23: lsg_n23(ctx)
		case 24: lsg_n24(ctx)
		case 25: lsg_n25(ctx)
		case 26: lsg_n26(ctx)
		case 27: lsg_n27(ctx)
		case 28: lsg_n28(ctx)
		case 29: lsg_n29(ctx)
		case 30: lsg_n30(ctx)
		case 31: lsg_n31(ctx)
		case 32: lsg_n32(ctx)
		case 33: lsg_n33(ctx)
		case 34: lsg_n34(ctx)
		case 35: lsg_n35(ctx)
		case 36: lsg_n36(ctx)
		case 37: lsg_n37(ctx)
		case 38: lsg_n38(ctx)
		case 39: lsg_n39(ctx)
		case 40: lsg_n40(ctx)
		case 41: lsg_n41(ctx)
		case 42: lsg_n42(ctx)
		case 43: lsg_n43(ctx)
		case 44: lsg_n44(ctx)
		case 45: lsg_n45(ctx)
		case 46: lsg_n46(ctx)
		case 47: lsg_n47(ctx)
		case 48: lsg_n48(ctx)
		case 49: lsg_n49(ctx)
		case 50: lsg_n50(ctx)
		case 51: lsg_n51(ctx)
		case 52: lsg_n52(ctx)
		case 53: lsg_n53(ctx)
		case 54: lsg_n54(ctx)
		case 55: lsg_n55(ctx)
		case 56: lsg_n56(ctx)
		case 57: lsg_n57(ctx)
		case 58: lsg_n58(ctx)
		case 59: lsg_n59(ctx)
		case 60: lsg_n60(ctx)
		case 61: lsg_n61(ctx)
		case 62: lsg_n62(ctx)
		case 63: lsg_n63(ctx)
		case 64: lsg_n64(ctx)
		case 65: lsg_n65(ctx)
		case 66: lsg_n66(ctx)
		case 67: lsg_n67(ctx)
		case 68: lsg_n68(ctx)
		case 69: lsg_n69(ctx)
		case 70: lsg_n70(ctx)
		case 71: lsg_n71(ctx)
		case 72: lsg_n72(ctx)
		case 73: lsg_n73(ctx)
		case 74: lsg_n74(ctx)
		case 75: lsg_n75(ctx)
		case 76: lsg_n76(ctx)
		case 77: lsg_n77(ctx)
		case 78: lsg_n78(ctx)
		case 79: lsg_n79(ctx)
		case 80: lsg_n80(ctx)
		case 81: lsg_n81(ctx)
		case 82: lsg_n82(ctx)
		case 83: lsg_n83(ctx)
		case 84: lsg_n84(ctx)
		case 85: lsg_n85(ctx)
		case 86: lsg_n86(ctx)
		case 87: lsg_n87(ctx)
		case 88: lsg_n88(ctx)
		case 89: lsg_n89(ctx)
		case 90: lsg_n90(ctx)
		case 91: lsg_n91(ctx)
		case 92: lsg_n92(ctx)
		case 93: lsg_n93(ctx)
		case 94: lsg_n94(ctx)
		case 95: lsg_n95(ctx)
		case 96: lsg_n96(ctx)
		case 97: lsg_n97(ctx)
		case 98: lsg_n98(ctx)
		case 99: lsg_n99(ctx)
		case 100: lsg_n100(ctx)
		case 101: lsg_n101(ctx)
		case 102: lsg_n102(ctx)
		case 103: lsg_n103(ctx)
		case 104: lsg_n104(ctx)
		case 105: lsg_n105(ctx)
		case 106: lsg_n106(ctx)
		case 107: lsg_n107(ctx)
		case 108: lsg_n108(ctx)
		case 109: lsg_n109(ctx)
		case 110: lsg_n110(ctx)
		case 111: lsg_n111(ctx)
		case 112: lsg_n112(ctx)
		case 113: lsg_n113(ctx)
		case 114: lsg_n114(ctx)
		case 115: lsg_n115(ctx)
		case 116: lsg_n116(ctx)
		case 117: lsg_n117(ctx)
		case 118: lsg_n118(ctx)
		case 119: lsg_n119(ctx)
		case 120: lsg_n120(ctx)
		case 121: lsg_n121(ctx)
		case 122: lsg_n122(ctx)
		case 123: lsg_n123(ctx)
	}
}
