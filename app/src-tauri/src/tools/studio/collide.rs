//! Collision, overlap and hit nodes (W2): players passing through each other, an entity's
//! collision, size, damage and health, trigger volumes with Begin/End Overlap, Entity Touched,
//! Bullet Hit and Player Landed. A child of gen.rs (its helpers are used as they are); nothing here
//! is written into a plugin that has none of these nodes, so other modes' code doesn't change.
//!
//! Costs, all only when a node needs them: one 0.1 s task for every trigger volume (an
//! EngFunc_FindEntityInSphere round each, no touch hooks); FM_Touch (world touches dropped first,
//! then a per-entity "first touch per N s"); FM_TraceLine only while a weapon's primary attack runs
//! (the shooter's own bullet traces), at most once per shooter every N s; FM_PlayerPreThink/PostThink
//! for the semiclip and for landings; FM_AddToFullPack (post) for the semiclip's client side.

use super::*;

/// The kinds of this block's event nodes (their handlers are written here, not from EVENTS).
pub(super) const EVENT_KINDS: [&str; 5] = ["vol.on_begin", "vol.on_end", "col.on_touch", "col.on_bullet", "col.on_landed"];

pub(super) fn is_event(kind: &str) -> bool {
    EVENT_KINDS.contains(&kind)
}

/// Any node of this block in the graph (Bots Avoid Area too: its count check is a stock here).
pub(super) fn used(g: &Graph) -> bool {
    g.nodes.iter().any(|n| n.kind.starts_with("col.") || n.kind.starts_with("vol.") || n.kind.starts_with("bots.avoid"))
}

struct Uses {
    semi: bool,
    vols: bool,
    touch: bool,
    bullet: bool,
    landed: bool,
}

fn uses(gen: &Gen) -> Uses {
    let live = |k: &str| gen.live.iter().any(|id| gen.v.defs[id].kind == k);
    Uses {
        semi: live("col.semiclip"),
        vols: live("vol.spawn") || live("vol.remove") || live("vol.on_begin") || live("vol.on_end") || pure_used(gen, "vol.is_overlapping"),
        touch: live("col.on_touch"),
        bullet: live("col.on_bullet"),
        landed: live("col.on_landed"),
    }
}

/// The shared stocks (only stocks: unused ones aren't compiled).
pub(super) const PAWN: &str = r##"// ---- collision, overlap and hits (W2) ----
stock g_lsgSemi, bool:g_lsgSemiOn[33], g_lsgSemiOff[33][33], g_lsgSemiN[33]
// (sized by the Limits' most: Trigger volumes at once 256, Things inside one 64; the mode's own
// settings - lim_trigger_volumes, lim_volume_members - say how many it really keeps)
#define LSG_VOLS 256
#define LSG_VOLIN 64
stock g_lsgVol[LSG_VOLS], g_lsgVolShape[LSG_VOLS], g_lsgVolFilter[LSG_VOLS], g_lsgVolAt[LSG_VOLS], Float:g_lsgVolPos[LSG_VOLS][3], Float:g_lsgVolSize[LSG_VOLS][3]
stock Float:g_lsgVolAng[LSG_VOLS][3], Float:g_lsgVolOff[LSG_VOLS][3], Float:g_lsgVolTurn[LSG_VOLS][3]
stock g_lsgVolIn[LSG_VOLS][LSG_VOLIN], g_lsgVolInN[LSG_VOLS], g_lsgVolInCap[LSG_VOLS]
stock g_lsgAvoidTag[32][24], g_lsgAvoidN
stock g_lsgFiring, Float:g_lsgAirVel[33], bool:g_lsgAirborne[33]

stock bool:lsg_col_thing(e)
{
	// (a prop, an NPC, a spawned thing: not a player, not the world)
	return e > MaxClients && pev_valid(e)
}

stock lsg_semiclip(mode, id, on)
{
	if (mode == 3)
	{
		if (lsg_ok(id))
			g_lsgSemiOn[id] = on != 0
		// (nobody chosen any more: off)
		new any = 0
		for (new p = 1; p <= MaxClients; p++)
			if (g_lsgSemiOn[p])
				any = 1
		if (!any)
			mode = 0
	}
	new was = g_lsgSemi
	g_lsgSemi = mode
	// (turned off with players inside each other: nudged free in a moment)
	if (was && !mode)
		set_task(0.1, "lsg_semi_unstick", LSG_TASK - 420)
	return 1
}

stock bool:lsg_semi_pair(a, b)
{
	switch (g_lsgSemi)
	{
		case 1: return true
		case 2: return get_user_team(a) == get_user_team(b)
		case 3: return g_lsgSemiOn[a] || g_lsgSemiOn[b]
	}
	return false
}

public lsg_semi_unstick()
{
	static const Float:TRY[][3] = { {0.0, 0.0, 0.0}, {32.0, 0.0, 0.0}, {-32.0, 0.0, 0.0}, {0.0, 32.0, 0.0}, {0.0, -32.0, 0.0}, {24.0, 24.0, 0.0}, {-24.0, -24.0, 0.0}, {24.0, -24.0, 0.0}, {-24.0, 24.0, 0.0}, {0.0, 0.0, 36.0}, {48.0, 0.0, 0.0}, {-48.0, 0.0, 0.0}, {0.0, 48.0, 0.0}, {0.0, -48.0, 0.0} }
	for (new p = 1; p <= MaxClients; p++)
	{
		if (!lsg_ok(p) || !is_user_alive(p))
			continue
		new Float:o[3], Float:t[3], hull = (pev(p, pev_flags) & FL_DUCKING) ? HULL_HEAD : HULL_HUMAN
		pev(p, pev_origin, o)
		engfunc(EngFunc_TraceHull, o, o, 0, hull, p, 0)
		if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			continue
		for (new i = 1; i < sizeof TRY; i++)
		{
			for (new k = 0; k < 3; k++)
				t[k] = o[k] + TRY[i][k]
			engfunc(EngFunc_TraceHull, t, t, 0, hull, p, 0)
			if (!get_tr2(0, TR_StartSolid) && !get_tr2(0, TR_AllSolid))
			{
				engfunc(EngFunc_SetOrigin, p, t)
				break
			}
		}
	}
}

stock lsg_set_solid(e, kind)
{
	if (!lsg_col_thing(e))
		return 0
	new model[4]
	pev(e, pev_model, model, charsmax(model))
	// (a brush keeps its own solid kind when it's made solid again)
	new solid = kind == 1 ? SOLID_TRIGGER : (kind == 2 ? SOLID_NOT : (model[0] == '*' ? SOLID_BSP : SOLID_BBOX))
	set_pev(e, pev_solid, solid)
	new Float:o[3]
	pev(e, pev_origin, o)
	engfunc(EngFunc_SetOrigin, e, o)
	return 1
}

stock lsg_set_size(e, Float:mn0, Float:mn1, Float:mn2, Float:mx0, Float:mx1, Float:mx2)
{
	if (!lsg_col_thing(e) || mn0 >= mx0 || mn1 >= mx1 || mn2 >= mx2)
		return 0
	new Float:mn[3], Float:mx[3]
	mn[0] = mn0
	mn[1] = mn1
	mn[2] = mn2
	mx[0] = mx0
	mx[1] = mx1
	mx[2] = mx2
	engfunc(EngFunc_SetSize, e, mn, mx)
	return 1
}

stock lsg_takes_damage(e, on)
{
	if (!lsg_col_thing(e))
		return 0
	set_pev(e, pev_takedamage, on ? DAMAGE_YES : DAMAGE_NO)
	return 1
}

stock Float:lsg_ent_health(e)
{
	new Float:hp = 0.0
	if (e > 0 && pev_valid(e))
		pev(e, pev_health, hp)
	return hp
}

stock lsg_set_ent_health(e, Float:hp)
{
	if (!lsg_col_thing(e))
		return 0
	set_pev(e, pev_health, floatmax(hp, 1.0))
	return 1
}

// ---- trigger volumes: an info_target that holds the name; checked ten times a second
// where a volume is and how it's turned: at its entity (plus the local offset by the entity's facing,
// and the rotation on top of the entity's) or at its position
stock lsg_vol_place(v)
{
	new at = g_lsgVolAt[v]
	if (!at)
	{
		g_lsgVolTurn[v] = g_lsgVolAng[v]
		return
	}
	new Float:o[3], Float:a[3], Float:f[3], Float:r[3], Float:u[3]
	lsg_ent_origin(at, o)
	pev(at, pev_angles, a)
	if (at <= MaxClients)
	{
		// (a player: the way they face, level)
		pev(at, pev_v_angle, a)
		a[0] = 0.0
	}
	engfunc(EngFunc_MakeVectors, a)
	global_get(glb_v_forward, f)
	global_get(glb_v_right, r)
	global_get(glb_v_up, u)
	for (new k = 0; k < 3; k++)
	{
		g_lsgVolPos[v][k] = o[k] + f[k] * g_lsgVolOff[v][0] + r[k] * g_lsgVolOff[v][1] + u[k] * g_lsgVolOff[v][2]
		g_lsgVolTurn[v][k] = a[k] + g_lsgVolAng[v][k]
	}
}

// why the last Spawn Trigger Volume failed: the Spawn Failure enum (0 None, 2 Out of entity slots,
// 3 Limit reached, 6 Bad input; gen/fx.rs WHY_*), and its words for the debugger
stock g_lsgVolWhy
stock const g_lsgVolWhyName[][] = { "", "Blocked", "Out of entity slots", "Limit reached: Trigger volumes at once (Limits)", "Missing file", "Retries used up", "Bad input: At entity is gone and there's no Position" }

stock lsg_vol_spawn(at, const Float:pos[3], const Float:ang[3], const Float:off[3], shape, Float:sx, Float:sy, Float:sz, filter, const name[], cap, incap, need_at)
{
	g_lsgVolWhy = 0
	if (need_at && !(at > 0 && pev_valid(at)))
	{
		g_lsgVolWhy = 6
		return 0
	}
	new v = -1
	for (new i = 0; i < min(cap, LSG_VOLS) && v < 0; i++)
		if (!g_lsgVol[i] || !pev_valid(g_lsgVol[i]))
			v = i
	if (v < 0)
	{
		lsg_skip_log("trigger_volumes", "Spawn Trigger Volume", "Spawn Trigger Volume skipped: as many are out as Trigger volumes at once allows (%d, Limits)", cap)
		g_lsgVolWhy = 3
		return 0
	}
	if (!lsg_ent_room("Spawn Trigger Volume"))
	{
		g_lsgVolWhy = 2
		return 0
	}
	if (!(at > 0 && pev_valid(at)))
		at = 0
	g_lsgVolAt[v] = at
	g_lsgVolAng[v] = ang
	g_lsgVolOff[v] = off
	g_lsgVolPos[v] = pos
	lsg_vol_place(v)
	new Float:o[3]
	o = g_lsgVolPos[v]
	new e = engfunc(EngFunc_CreateNamedEntity, engfunc(EngFunc_AllocString, "info_target"))
	if (!e)
	{
		g_lsgVolWhy = 2
		return 0
	}
	set_pev(e, pev_classname, "lsg_volume")
	if (name[0])
		set_pev(e, pev_targetname, name)
	set_pev(e, pev_solid, SOLID_NOT)
	set_pev(e, pev_movetype, MOVETYPE_NONE)
	set_pev(e, pev_effects, EF_NODRAW)
	engfunc(EngFunc_SetOrigin, e, o)
	g_lsgVol[v] = e
	g_lsgVolShape[v] = shape
	g_lsgVolFilter[v] = filter
	g_lsgVolInCap[v] = clamp(incap, 1, LSG_VOLIN)
	g_lsgVolSize[v][0] = sx
	g_lsgVolSize[v][1] = sy
	g_lsgVolSize[v][2] = sz
	g_lsgVolInN[v] = 0
	return e
}

// the volume is still out (a handler before may have removed it: Remove Trigger Volume frees the
// entity at once, and its index can come back as something else)
stock bool:lsg_vol_live(vol)
	return lsg_vol_slot(vol) >= 0 && pev_valid(vol) != 0

stock lsg_vol_slot(e)
{
	if (e <= 0)
		return -1
	for (new i = 0; i < LSG_VOLS; i++)
		if (g_lsgVol[i] == e)
			return i
	return -1
}

stock bool:lsg_vol_counts(v, e)
{
	if (e <= 0 || e == g_lsgVol[v] || e == g_lsgVolAt[v] || !pev_valid(e))
		return false
	if (e <= MaxClients)
		return g_lsgVolFilter[v] != 1 && is_user_alive(e)
	new cls[16]
	pev(e, pev_classname, cls, charsmax(cls))
	if (g_lsgVolFilter[v] == 1)
		return equal(cls, "ls_creature") && pev(e, pev_deadflag) == DEAD_NO
	if (g_lsgVolFilter[v] == 2)
		return !equal(cls, "lsg_volume") && pev(e, pev_solid) != SOLID_NOT
	return false
}

stock bool:lsg_vol_inside(v, e)
{
	new Float:p[3]
	lsg_ent_origin(e, p)
	if (g_lsgVolShape[v] == 1)
		return get_distance_f(p, g_lsgVolPos[v]) <= g_lsgVolSize[v][0]
	// (a box, turned: the point in the box's own axes)
	new Float:d[3], Float:ax[3][3]
	for (new k = 0; k < 3; k++)
		d[k] = p[k] - g_lsgVolPos[v][k]
	engfunc(EngFunc_MakeVectors, g_lsgVolTurn[v])
	global_get(glb_v_forward, ax[0])
	global_get(glb_v_right, ax[1])
	global_get(glb_v_up, ax[2])
	for (new k = 0; k < 3; k++)
		if (floatabs(d[0] * ax[k][0] + d[1] * ax[k][1] + d[2] * ax[k][2]) > g_lsgVolSize[v][k] * 0.5)
			return false
	return true
}

stock lsg_vol_has(vol, e)
{
	new v = lsg_vol_slot(vol)
	return v >= 0 && lsg_vol_counts(v, e) && lsg_vol_inside(v, e) ? 1 : 0
}

stock lsg_vol_remove(vol)
{
	new v = lsg_vol_slot(vol)
	if (v < 0)
		return 0
	// (whoever was inside leaves it)
	new n = g_lsgVolInN[v]
	g_lsgVolInN[v] = 0
	for (new i = 0; i < n; i++)
		lsg_vol_end_hook(vol, g_lsgVolIn[v][i])
	g_lsgVol[v] = 0
	if (pev_valid(vol))
		engfunc(EngFunc_RemoveEntity, vol)
	return 1
}

public lsg_vol_tick()
{
	for (new v = 0; v < LSG_VOLS; v++)
	{
		new vol = g_lsgVol[v]
		if (!vol)
			continue
		if (!pev_valid(vol))
		{
			g_lsgVol[v] = 0
			continue
		}
		// (riding along: where the entity is now; it's gone: the volume stays where it was)
		if (g_lsgVolAt[v])
		{
			if (pev_valid(g_lsgVolAt[v]))
			{
				lsg_vol_place(v)
				engfunc(EngFunc_SetOrigin, vol, g_lsgVolPos[v])
			}
			else
				g_lsgVolAt[v] = 0
		}
		new Float:reach = g_lsgVolShape[v] == 1 ? g_lsgVolSize[v][0] : floatsqroot(g_lsgVolSize[v][0] * g_lsgVolSize[v][0] + g_lsgVolSize[v][1] * g_lsgVolSize[v][1] + g_lsgVolSize[v][2] * g_lsgVolSize[v][2]) * 0.5
		new now[LSG_VOLIN], nn = 0, e = -1
		while ((e = engfunc(EngFunc_FindEntityInSphere, e, g_lsgVolPos[v], reach)) > 0 && nn < g_lsgVolInCap[v])
			if (lsg_vol_counts(v, e) && lsg_vol_inside(v, e))
				now[nn++] = e
		// who left, then who came in
		new was[LSG_VOLIN], wn = g_lsgVolInN[v]
		for (new i = 0; i < wn; i++)
			was[i] = g_lsgVolIn[v][i]
		for (new i = 0; i < nn; i++)
			g_lsgVolIn[v][i] = now[i]
		g_lsgVolInN[v] = nn
		for (new i = 0; i < wn; i++)
		{
			new still = 0
			for (new j = 0; j < nn && !still; j++)
				if (now[j] == was[i])
					still = 1
			if (!still)
				lsg_vol_end_hook(vol, was[i])
		}
		for (new j = 0; j < nn; j++)
		{
			new had = 0
			for (new i = 0; i < wn && !had; i++)
				if (was[i] == now[j])
					had = 1
			if (!had && g_lsgVol[v] == vol)
				lsg_vol_begin_hook(vol, now[j])
		}
	}
}

// ---- Bots Avoid Area: the tags out now, at most Areas bots avoid at once (Limits; the brain keeps 32)
stock bool:lsg_avoid_room(const tag[], cap)
{
	for (new i = 0; i < g_lsgAvoidN; i++)
		if (equal(g_lsgAvoidTag[i], tag))
			return true
	if (g_lsgAvoidN >= min(cap, sizeof g_lsgAvoidTag))
	{
		lsg_skip_log("avoid_areas", tag, "Bots Avoid Area %s skipped: as many are out as Areas bots avoid at once allows (%d, Limits)", tag, cap)
		return false
	}
	copy(g_lsgAvoidTag[g_lsgAvoidN++], 23, tag)
	return true
}

stock lsg_avoid_gone(const tag[])
{
	for (new i = 0; i < g_lsgAvoidN; i++)
		if (equal(g_lsgAvoidTag[i], tag))
		{
			copy(g_lsgAvoidTag[i], 23, g_lsgAvoidTag[--g_lsgAvoidN])
			return
		}
}

// ---- Bullet Hit: the shooter's own traces while their weapon's primary attack runs
public lsg_col_fire_pre(wpn)
{
	g_lsgFiring = pev_valid(wpn) ? pev(wpn, pev_owner) : 0
	if (!lsg_ok(g_lsgFiring))
		g_lsgFiring = 0
	return HAM_IGNORED
}

public lsg_col_fire_post(wpn)
{
	#pragma unused wpn
	g_lsgFiring = 0
	return HAM_IGNORED
}

stock lsg_col_hook_fire()
{
	static const W[][] = { "weapon_p228", "weapon_scout", "weapon_xm1014", "weapon_mac10", "weapon_aug", "weapon_elite", "weapon_fiveseven", "weapon_ump45", "weapon_sg550", "weapon_galil", "weapon_famas", "weapon_usp", "weapon_glock18", "weapon_awp", "weapon_mp5navy", "weapon_m249", "weapon_m3", "weapon_m4a1", "weapon_tmp", "weapon_g3sg1", "weapon_deagle", "weapon_sg552", "weapon_ak47", "weapon_p90", "weapon_knife" }
	for (new i = 0; i < sizeof W; i++)
	{
		RegisterHam(Ham_Weapon_PrimaryAttack, W[i], "lsg_col_fire_pre", 0)
		RegisterHam(Ham_Weapon_PrimaryAttack, W[i], "lsg_col_fire_post", 1)
	}
}

stock lsg_col_hitgroup(e, tr, out[], len)
{
	out[0] = EOS
	if (e < 1 || e > MaxClients)
		return
	switch (get_tr2(tr, TR_iHitgroup))
	{
		case HIT_HEAD: copy(out, len, "head")
		case HIT_CHEST: copy(out, len, "chest")
		case HIT_STOMACH: copy(out, len, "stomach")
		case HIT_LEFTARM: copy(out, len, "left arm")
		case HIT_RIGHTARM: copy(out, len, "right arm")
		case HIT_LEFTLEG: copy(out, len, "left leg")
		case HIT_RIGHTLEG: copy(out, len, "right leg")
		default: copy(out, len, "body")
	}
}

// ---- Player Landed: the game's fall damage for a fall speed (CS: from 580, 100 at 1024)
stock Float:lsg_fall_damage(Float:speed)
{
	return speed > 580.0 ? (speed - 580.0) * (100.0 / (1024.0 - 580.0)) : 0.0
}
"##;

/// plugin_init's lines for what's used.
pub(super) fn init(gen: &Gen, o: &mut Out) {
    let u = uses(gen);
    if u.semi || u.landed {
        o.line("\tregister_forward(FM_PlayerPreThink, \"lsg_col_pre\", 0)\n\tregister_forward(FM_PlayerPostThink, \"lsg_col_post\", 0)");
    }
    if u.semi {
        o.line("\tregister_forward(FM_AddToFullPack, \"lsg_col_pack\", 1)");
    }
    if u.vols {
        o.line("\tset_task(0.1, \"lsg_vol_tick\", LSG_TASK - 430, _, _, \"b\")");
    }
    if u.touch {
        o.line("\tregister_forward(FM_Touch, \"lsg_col_touch\", 1)");
    }
    if u.bullet {
        o.line("\tlsg_col_hook_fire()\n\tregister_forward(FM_TraceLine, \"lsg_col_trace\", 1)");
    }
}

/// The event nodes of one kind, each with its index (for its own arrays) and node id.
fn nodes_of<'g>(gen: &'g Gen, kind: &str) -> Vec<(usize, &'g str)> {
    gen.live.iter().copied().filter(|id| gen.v.defs[*id].kind == kind).map(|id| (gen.index(id), id)).collect()
}

/// ctx[...] for a cell pin of an event node kind (None: not a pin of it).
fn cell(kind: &str, pin: &str) -> Option<usize> {
    match event_slot(registry_def(kind), pin) {
        Some((false, i)) => Some(i),
        _ => None,
    }
}

fn txt(kind: &str, pin: &str) -> Option<usize> {
    match event_slot(registry_def(kind), pin) {
        Some((true, i)) => Some(i),
        _ => None,
    }
}

/// The forwards and hooks (after the event nodes' own functions are known).
pub(super) fn handlers(gen: &Gen, o: &mut Out) {
    let u = uses(gen);
    let v = &gen.v;
    let player_touched = gen.live.iter().any(|id| v.defs[id].kind == "event.player_touched");
    // per-node state
    for (k, _) in nodes_of(gen, "col.on_touch") {
        o.line(format!("stock Float:g_lsgTouch{k}[2048]"));
    }
    for (k, _) in nodes_of(gen, "col.on_bullet") {
        o.line(format!("stock Float:g_lsgBullet{k}[33]"));
    }

    // the semiclip and landings: one PreThink/PostThink pair
    if u.semi || u.landed {
        let mut pre = String::from("public lsg_col_pre(id)\n{\n\tif (id < 1 || id > MaxClients || !is_user_alive(id))\n\t\treturn FMRES_IGNORED");
        if u.landed {
            pre.push_str("\n\t// (falling: how fast, for the landing)\n\tg_lsgAirborne[id] = !(pev(id, pev_flags) & FL_ONGROUND)\n\tif (g_lsgAirborne[id])\n\t\tpev(id, pev_flFallVelocity, g_lsgAirVel[id])");
        }
        if u.semi {
            pre.push_str("\n\t// (the players this one passes through: not solid while it moves)\n\tg_lsgSemiN[id] = 0\n\tif (g_lsgSemi)\n\t\tfor (new p = 1; p <= MaxClients; p++)\n\t\t\tif (p != id && is_user_alive(p) && pev(p, pev_solid) == SOLID_SLIDEBOX && lsg_semi_pair(id, p))\n\t\t\t{\n\t\t\t\tset_pev(p, pev_solid, SOLID_NOT)\n\t\t\t\tg_lsgSemiOff[id][g_lsgSemiN[id]++] = p\n\t\t\t}");
        }
        pre.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(pre);
        let mut post = String::from("public lsg_col_post(id)\n{\n\tif (id < 1 || id > MaxClients)\n\t\treturn FMRES_IGNORED");
        if u.semi {
            post.push_str("\n\tfor (new i = 0; i < g_lsgSemiN[id]; i++)\n\t\tif (is_user_alive(g_lsgSemiOff[id][i]))\n\t\t\tset_pev(g_lsgSemiOff[id][i], pev_solid, SOLID_SLIDEBOX)\n\tg_lsgSemiN[id] = 0");
        }
        if u.landed {
            post.push_str("\n\t// a landing: in the air when the frame began, on the ground now\n\tif (g_lsgAirborne[id] && is_user_alive(id) && (pev(id, pev_flags) & FL_ONGROUND))\n\t{\n\t\tg_lsgAirborne[id] = false\n\t\tnew Float:speed = g_lsgAirVel[id]\n\t\tnew ctx[LSG_CTX]");
            let kind = "col.on_landed";
            for (pin, read) in [("player", "id"), ("speed", "_:speed"), ("damage", "_:lsg_fall_damage(speed)")] {
                if let Some(i) = cell(kind, pin) {
                    post.push_str(&format!("\n\t\tctx[{i}] = {read} // {pin}"));
                }
            }
            for (_, id) in nodes_of(gen, kind) {
                let min = gen.num(v.nodes[id], v.defs[id], "min").clamp(0.0, 2000.0);
                post.push_str(&format!("\n\t\tif (speed > {})\n\t\t\t{}(ctx)", float_lit(min), gen.fname(id)));
            }
            post.push_str("\n\t}");
        }
        post.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(post);
    }
    if u.semi {
        o.line("// (the semiclip in each player's own game: the others it passes through aren't solid to it)\npublic lsg_col_pack(es, e, ent, host, flags, player, pSet)\n{\n\t#pragma unused e\n\t#pragma unused flags\n\t#pragma unused pSet\n\tif (player && g_lsgSemi && ent != host && host >= 1 && host <= MaxClients && ent >= 1 && ent <= MaxClients && is_user_alive(host) && is_user_alive(ent) && lsg_semi_pair(host, ent))\n\t\tset_es(es, ES_Solid, SOLID_NOT)\n\treturn FMRES_IGNORED\n}\n");
    }

    // trigger volumes: who went in, who came out (and Player Touched for a volume's name); the
    // hooks are always there with the stocks (the volume tick is a public that calls them)
    {
        for (hook, kind) in [("lsg_vol_begin_hook", "vol.on_begin"), ("lsg_vol_end_hook", "vol.on_end")] {
            let mut t = format!("{hook}(vol, who)\n{{\n\tnew ctx[LSG_CTX]");
            if let Some(i) = cell(kind, "volume") {
                t.push_str(&format!("\n\tctx[{i}] = vol // volume"));
            }
            if let Some(i) = cell(kind, "who") {
                t.push_str(&format!("\n\tctx[{i}] = who // who"));
            }
            let nodes = nodes_of(gen, kind);
            for (_, id) in &nodes {
                let name = name_text(gen.text(v.nodes[id], v.defs[id], "name"));
                // (each only while the volume is still out: an earlier handler may have removed it)
                if name.is_empty() {
                    t.push_str(&format!("\n\tif (lsg_vol_live(vol))\n\t\t{}(ctx)", gen.fname(id)));
                } else {
                    t.push_str(&format!("\n\tif (lsg_vol_live(vol) && lsg_named(vol, {}))\n\t\t{}(ctx)", pawn_str(&name), gen.fname(id)));
                }
            }
            if kind == "vol.on_begin" && player_touched {
                t.push_str("\n\t// (Player Touched hears a volume like a map's trigger_multiple)\n\tif (who >= 1 && who <= MaxClients && lsg_vol_live(vol))\n\t\tlsg_on_player_touched(vol, who)");
            }
            if nodes.is_empty() && !(kind == "vol.on_begin" && player_touched) {
                t.push_str("\n\t#pragma unused ctx");
            }
            t.push_str("\n}\n");
            o.line(t);
        }
    }

    // Entity Touched
    if u.touch {
        let kind = "col.on_touch";
        let mut t = String::from("public lsg_col_touch(ent, other)\n{\n\t// (the world's walls and floors aren't \"another entity\")\n\tif (ent <= 0 || other <= 0 || ent >= 2048 || !pev_valid(ent) || !pev_valid(other))\n\t\treturn FMRES_IGNORED\n\tnew Float:now = get_gametime()\n\tnew ctx[LSG_CTX]");
        for (pin, read) in [("entity", "ent"), ("other", "other")] {
            if let Some(i) = cell(kind, pin) {
                t.push_str(&format!("\n\tctx[{i}] = {read} // {pin}"));
            }
        }
        for (k, id) in nodes_of(gen, kind) {
            let name = name_text(gen.text(v.nodes[id], v.defs[id], "name"));
            let secs = float_lit(gen.num(v.nodes[id], v.defs[id], "secs").clamp(0.1, 60.0));
            let named = if name.is_empty() { String::new() } else { format!(" && lsg_named(ent, {})", pawn_str(&name)) };
            t.push_str(&format!("\n\tif (now - g_lsgTouch{k}[ent] >= {secs}{named})\n\t{{\n\t\tg_lsgTouch{k}[ent] = now\n\t\t{}(ctx)\n\t}}", gen.fname(id)));
        }
        t.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(t);
    }

    // Bullet Hit
    if u.bullet {
        let kind = "col.on_bullet";
        let mut t = String::from("public lsg_col_trace(Float:v1[3], Float:v2[3], nomonsters, skip, tr)\n{\n\t#pragma unused v1\n\t#pragma unused v2\n\t#pragma unused nomonsters\n\tnew id = g_lsgFiring\n\tif (!id || skip != id)\n\t\treturn FMRES_IGNORED\n\tnew Float:now = get_gametime(), go = 0");
        let nodes = nodes_of(gen, kind);
        for (k, id) in &nodes {
            let secs = float_lit(gen.num(v.nodes[id], v.defs[id], "secs").clamp(0.05, 10.0));
            t.push_str(&format!("\n\tif (now - g_lsgBullet{k}[id] >= {secs})\n\t\tgo = 1"));
        }
        t.push_str("\n\tif (!go)\n\t\treturn FMRES_IGNORED\n\tnew Float:end[3], Float:nrm[3], hit = get_tr2(tr, TR_pHit), wn[24], w = get_user_weapon(id)\n\tget_tr2(tr, TR_vecEndPos, end)\n\tget_tr2(tr, TR_vecPlaneNormal, nrm)\n\tif (hit < 0 || !pev_valid(hit))\n\t\thit = 0\n\tif (w)\n\t\tget_weaponname(w, wn, charsmax(wn))\n\treplace(wn, charsmax(wn), \"weapon_\", \"\")\n\tnew ctx[LSG_CTX]");
        for (pin, read) in [("shooter", "id"), ("pos", "lsg_vfrom(end)"), ("normal", "lsg_vfrom(nrm)"), ("hit", "hit"), ("world", "hit ? 0 : 1")] {
            if let Some(i) = cell(kind, pin) {
                t.push_str(&format!("\n\tctx[{i}] = {read} // {pin}"));
            }
        }
        if let Some(i) = txt(kind, "weapon") {
            t.push_str(&format!("\n\tcopy(g_lsgTxt[{i}], 63, wn) // weapon"));
        }
        if let Some(i) = txt(kind, "hitgroup") {
            t.push_str(&format!("\n\tlsg_col_hitgroup(hit, tr, g_lsgTxt[{i}], 63) // hitgroup"));
        }
        for (k, id) in &nodes {
            let secs = float_lit(gen.num(v.nodes[id], v.defs[id], "secs").clamp(0.05, 10.0));
            t.push_str(&format!("\n\tif (now - g_lsgBullet{k}[id] >= {secs})\n\t{{\n\t\tg_lsgBullet{k}[id] = now\n\t\t{}(ctx)\n\t}}", gen.fname(*id)));
        }
        t.push_str("\n\treturn FMRES_IGNORED\n}\n");
        o.line(t);
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::studio::{gen, graph};
    use serde_json::json;

    const GRAPH: &str = include_str!("testdata/every_node_5.graph.json");

    /// Every node of the block in one mode: no problems, and each piece's code and hooks are there.
    #[test]
    fn collide_nodes_generate() {
        let g = graph::parse(GRAPH).unwrap();
        let p = graph::validate(&g);
        assert!(p.is_empty(), "{p:#?}");
        for debug in [false, true] {
            let src = gen::generate(&g, debug).source;
            for want in [
                "register_forward(FM_PlayerPreThink, \"lsg_col_pre\", 0)", "register_forward(FM_AddToFullPack, \"lsg_col_pack\", 1)", "set_task(0.1, \"lsg_vol_tick\"",
                "register_forward(FM_Touch, \"lsg_col_touch\", 1)", "lsg_col_hook_fire()", "register_forward(FM_TraceLine, \"lsg_col_trace\", 1)",
                "lsg_semiclip(2, 0, 1)", "lsg_semiclip(3, ", "lsg_set_solid(", "lsg_set_size(", "lsg_takes_damage(", "lsg_set_ent_health(", "lsg_ent_health(",
                "lsg_vol_spawn(", "lsg_vol_has(", "lsg_vol_remove(", "lsg_vol_begin_hook(vol, who)", "lsg_vol_end_hook(vol, who)",
                "lsg_on_player_touched(vol, who)", "lsg_fall_damage(speed)", "lsg_col_hitgroup(hit, tr", "lsg_hitgroup(victim, bits)) // hitgroup",
                "if (lsg_vol_live(vol) && lsg_named(vol, \"zone\"))", "g_lsgTouch", "g_lsgBullet",
                "ls_bots_avoid_ent(\"carrier\", ", ", 200.0, 7.0, 0.0, 1)", "ls_bots_avoid(\"spot\", lsgAv, 128.0, 3.0, 30.0, 0)", "ls_bots_avoid_stop(\"spot\")",
                "lsg_avoid_room(\"carrier\", 16)", "lsg_avoid_gone(\"spot\")", "lsg_ent_room(\"Spawn Trigger Volume\")", ", 32, 32, ", "g_lsgVolWhy = 3",
            ] {
                assert!(src.contains(want), "{want} (debug {debug})");
            }
        }
    }

    /// The events' handlers themselves (written by handlers(), which gen.rs must call): a Begin
    /// Overlap, a Bullet Hit and a Player Landed graph gets their public forwards and hooks, not only
    /// the registrations. Fails if gen.rs stops calling collide::handlers.
    #[test]
    fn collide_handlers_are_written() {
        let n = |id: &str, t: &str, p: serde_json::Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "ev", "name": "Events"},
            "nodes": [n("rs", "event.round_started", json!({})), n("vm", "vec.make", json!({"x": 0, "y": 0, "z": 36})), n("vs", "vol.spawn", json!({"name": "zone"})),
                      n("ob", "vol.on_begin", json!({"name": "zone"})), n("p1", "debug.print", json!({"text": "in", "chat": false})),
                      n("bh", "col.on_bullet", json!({"secs": 0.2})), n("p2", "debug.print", json!({"text": "hit", "chat": false})),
                      n("ld", "col.on_landed", json!({"min": 250})), n("p3", "debug.print", json!({"text": "landed", "chat": false}))],
            "edges": [e(1, "rs", "then", "vs", "exec"), e(2, "vm", "out", "vs", "pos"), e(3, "ob", "then", "p1", "exec"), e(4, "bh", "then", "p2", "exec"), e(5, "ld", "then", "p3", "exec")]})).unwrap();
        assert!(graph::validate(&g).is_empty(), "{:#?}", graph::validate(&g));
        let src = gen::generate(&g, false).source;
        for want in ["lsg_vol_begin_hook(vol, who)\n{", "lsg_vol_end_hook(vol, who)\n{", "public lsg_col_trace(Float:v1[3], Float:v2[3], nomonsters, skip, tr)", "public lsg_col_post(id)", "public lsg_col_pre(id)", "lsg_fall_damage(speed)"] {
            assert!(src.contains(want), "missing {want}: is collide::handlers called from gen.rs?");
        }
    }

    /// The spawn outcome standard (FX.md): Spawned or Failed runs, then Then; Entity / Success /
    /// Failure reason are kept, and At entity alone (no Position) must be there (Bad input). A
    /// saved graph's older outputs (Volume, success) load as the standard ones (out, ok).
    #[test]
    fn vol_spawn_outcome() {
        let n = |id: &str, t: &str, p: serde_json::Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "vo", "name": "Outcome"},
            "nodes": [n("ps", "event.player_spawned", json!({})), n("vs", "vol.spawn", json!({"name": "zone"})),
                      n("ok", "debug.print", json!({"text": "made", "chat": false})), n("no", "debug.print", json!({"text": "failed", "chat": false})), n("br", "flow.branch", json!({}))],
            "edges": [e(1, "ps", "then", "vs", "exec"), e(2, "ps", "player", "vs", "at_ent"), e(3, "vs", "spawned", "ok", "exec"), e(4, "vs", "volume", "ok", "value"),
                      e(5, "vs", "failed", "no", "exec"), e(6, "vs", "reason", "no", "value"), e(7, "vs", "then", "br", "exec"), e(8, "vs", "success", "br", "cond")]})).unwrap();
        // (the older ids, as a graph saved before the standard has them)
        assert!(g.edges.iter().any(|e| e.from.node == "vs" && e.from.pin == "out") && g.edges.iter().any(|e| e.from.node == "vs" && e.from.pin == "ok"));
        assert!(!g.edges.iter().any(|e| e.from.node == "vs" && (e.from.pin == "volume" || e.from.pin == "success")));
        assert!(graph::validate(&g).is_empty(), "{:#?}", graph::validate(&g));
        for debug in [false, true] {
            let src = gen::generate(&g, debug).source;
            let call = src.lines().find(|l| l.contains("= lsg_vol_spawn(")).expect("the spawn");
            assert!(call.trim_end().ends_with(", 1)"), "At entity alone must be there: {call}");
            assert!(src.contains("g_lsgVolWhy = 6") && src.contains("g_lsgVolWhy = 2"), "Bad input, Out of entity slots");
            assert!(src.contains("] = g_lsgVolWhy\n"), "Failure reason kept");
            assert!(src.contains("] ? 1 : 0"), "Success kept");
            let i = src.find("= lsg_vol_spawn(").unwrap();
            let body = &src[i..i + src[i..].find("\n}").unwrap()];
            assert!(body.contains("\telse\n\t\t"), "Spawned and Failed both go somewhere: {body}");
            let (sw, th) = (body.find("\telse\n\t\t").unwrap(), body.rfind("\n\tlsg_n").unwrap());
            assert!(th > sw, "Then runs after Spawned / Failed: {body}");
        }
    }

    /// X2's sim (Starter library A): a Begin Overlap that removes its volume (a pickup taken) made the
    /// next named handler read the freed entity (run time error 10 in lsg_named). Every handler in
    /// both hooks now runs only while the volume is still out.
    #[test]
    fn vol_hooks_stop_once_the_volume_is_removed() {
        let n = |id: &str, t: &str, p: serde_json::Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let e = |i: usize, a: &str, ap: &str, b: &str, bp: &str| json!({"id": format!("w{i}"), "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}});
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "vr", "name": "Removed"},
            "nodes": [n("rs", "event.round_started", json!({})), n("vm", "vec.make", json!({"x": 0, "y": 0, "z": 0})), n("vs", "vol.spawn", json!({"name": "heal"})),
                      n("b1", "vol.on_begin", json!({"name": "heal"})), n("rm", "vol.remove", json!({})),
                      n("b2", "vol.on_begin", json!({"name": "weap"})), n("p2", "debug.print", json!({"text": "weap", "chat": false})),
                      n("b3", "vol.on_begin", json!({"name": ""})), n("p3", "debug.print", json!({"text": "any", "chat": false})),
                      n("e1", "vol.on_end", json!({"name": "heal"})), n("rm2", "vol.remove", json!({})),
                      n("e2", "vol.on_end", json!({"name": "weap"})), n("p4", "debug.print", json!({"text": "left", "chat": false}))],
            "edges": [e(1, "rs", "then", "vs", "exec"), e(2, "vm", "out", "vs", "pos"), e(3, "b1", "then", "rm", "exec"), e(4, "b1", "volume", "rm", "volume"),
                      e(5, "b2", "then", "p2", "exec"), e(6, "b3", "then", "p3", "exec"),
                      e(7, "e1", "then", "rm2", "exec"), e(8, "e1", "volume", "rm2", "volume"), e(9, "e2", "then", "p4", "exec")]})).unwrap();
        assert!(graph::validate(&g).is_empty(), "{:#?}", graph::validate(&g));
        let src = gen::generate(&g, false).source;
        for hook in ["lsg_vol_begin_hook(vol, who)\n{", "lsg_vol_end_hook(vol, who)\n{"] {
            let at = src.find(hook).unwrap_or_else(|| panic!("{hook}"));
            let body = &src[at..at + src[at..].find("\n}").unwrap()];
            let calls: Vec<&str> = body.lines().filter(|l| l.contains("(ctx)")).collect();
            assert!(calls.len() >= 2, "{body}");
            // every handler call sits under a still-out check (the line before it)
            let lines: Vec<&str> = body.lines().collect();
            for (i, l) in lines.iter().enumerate().filter(|(_, l)| l.contains("(ctx)")) {
                assert!(lines[i - 1].contains("lsg_vol_live(vol)"), "unguarded after a possible removal:\n{body}\n(at {l})");
            }
            assert!(!body.contains("\tif (lsg_named(vol"), "a bare name check reads a freed volume: {body}");
        }
        assert!(src.contains("stock bool:lsg_vol_live(vol)"));
    }

    /// P5's final check: every cap's "skipped" line goes through the once-a-second lsg_skip_log
    /// (Spawn Trigger Volume, Bots Avoid Area, Loop Sound, out of entity slots), never a bare
    /// log_amx that would write every frame.
    #[test]
    fn cap_skip_lines_are_rate_limited() {
        for (what, src) in [("collide.rs", super::PAWN), ("gen.rs", super::super::HELPERS), ("fx.rs", include_str!("gen/fx.rs"))] {
            for l in src.lines().filter(|l| l.contains("skipped") && l.contains("log_amx")) {
                // (the helper's own lines, and the effects cap that already says it once a second)
                assert!(l.contains("too many effects this frame") || l.contains("LSG_MODE, msg"), "{what}: a skip line straight to the log: {l}");
            }
        }
        for pawn in [super::PAWN, include_str!("gen/fx.rs")] {
            assert!(pawn.contains("lsg_skip_log("));
        }
        let h = super::super::HELPERS;
        let at = h.find("stock lsg_skip_log(").expect("the helper");
        let body = &h[at..at + h[at..].find("\n}").unwrap()];
        assert!(body.contains("< 1.0") && body.contains("times in the last second") && body.contains("now >= g_lsgSkipAt[k]"), "{body}");
    }

    /// Every limit's setting label fits a setting label (30): a longer one failed every mode using it.
    #[test]
    fn limit_labels_fit_their_settings() {
        for l in crate::tools::studio::limits::LIMITS {
            assert!(l.label.chars().count() <= 30, "{} is {} letters: {:?}", l.id, l.label.chars().count(), l.label);
            let v = crate::tools::studio::limits::var_json(l);
            assert_eq!(v["setting"]["label"], l.label);
        }
        // a mode with Spawn Trigger Volume and its limit settings passes its check
        let mut g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "lm", "name": "Limits"},
            "nodes": [{"id": "rs", "type": "event.round_started", "pos": {"x": 0, "y": 0}, "params": {}}, {"id": "vm", "type": "vec.make", "pos": {"x": 0, "y": 0}, "params": {}},
                      {"id": "vs", "type": "vol.spawn", "pos": {"x": 0, "y": 0}, "params": {}}],
            "edges": [{"id": "w1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "vs", "pin": "exec"}}, {"id": "w2", "from": {"node": "vm", "pin": "out"}, "to": {"node": "vs", "pin": "pos"}}]})).unwrap();
        let mut v: serde_json::Value = serde_json::to_value(&g).unwrap();
        v["variables"] = json!(crate::tools::studio::limits::LIMITS.iter().filter(|l| l.kinds.contains(&"vol.spawn")).map(crate::tools::studio::limits::var_json).collect::<Vec<_>>());
        g = graph::from_value(v).unwrap();
        let errs: Vec<_> = graph::validate(&g).into_iter().filter(|p| p.level == "error").collect();
        assert!(errs.is_empty(), "{errs:#?}");
    }

    /// None of it in a mode without these nodes (other modes' plugins stay as they were).
    #[test]
    fn collide_absent_without_nodes() {
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "plain", "name": "Plain"}, "nodes": [{"id": "rs", "type": "event.round_started"}, {"id": "p", "type": "debug.print", "params": {"text": "x"}}], "edges": [{"id": "w", "from": {"node": "rs", "pin": "then"}, "to": {"node": "p", "pin": "exec"}}]})).unwrap();
        let src = gen::generate(&g, false).source;
        assert!(!src.contains("lsg_vol_tick") && !src.contains("lsg_col_") && !src.contains("lsg_semiclip"));
    }

    /// The checks: a volume with nowhere to be, a size with a min over its max, chosen players with
    /// nobody wired, overlap events with no volume in the mode.
    #[test]
    fn collide_problems() {
        let n = |id: &str, t: &str, p: serde_json::Value| json!({"id": id, "type": t, "pos": {"x": 0, "y": 0}, "params": p});
        let g = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "bad", "name": "Bad"},
            "nodes": [n("rs", "event.round_started", json!({})), n("vs", "vol.spawn", json!({})), n("sz", "col.set_size", json!({"minx": 10, "maxx": 5})), n("sc", "col.semiclip", json!({"mode": "chosen"})), n("pk", "flow.pick_entity", json!({"by": "class", "name": "func_wall"}))],
            "edges": [{"id": "a", "from": {"node": "rs", "pin": "then"}, "to": {"node": "vs", "pin": "exec"}}, {"id": "b", "from": {"node": "vs", "pin": "then"}, "to": {"node": "pk", "pin": "exec"}}, {"id": "c", "from": {"node": "pk", "pin": "then"}, "to": {"node": "sz", "pin": "exec"}}, {"id": "d", "from": {"node": "pk", "pin": "entity"}, "to": {"node": "sz", "pin": "ent"}}, {"id": "e", "from": {"node": "sz", "pin": "then"}, "to": {"node": "sc", "pin": "exec"}}]})).unwrap();
        let p: Vec<String> = graph::validate(&g).into_iter().map(|p| p.message).collect();
        for want in ["Spawn Trigger Volume needs", "Set Entity Size needs", "chosen players needs"] {
            assert!(p.iter().any(|m| m.contains(want)), "{want}: {p:?}");
        }
        let lone = graph::from_value(json!({"schema": graph::SCHEMA, "version": graph::VERSION, "mode": {"id": "lone", "name": "Lone"},
            "nodes": [n("ob", "vol.on_begin", json!({})), n("p", "debug.print", json!({"text": "x"}))],
            "edges": [{"id": "a", "from": {"node": "ob", "pin": "then"}, "to": {"node": "p", "pin": "exec"}}]})).unwrap();
        let p: Vec<String> = graph::validate(&lone).into_iter().map(|p| p.message).collect();
        assert!(p.iter().any(|m| m.contains("no Spawn Trigger Volume")), "{p:?}");
    }
}
