/*
 * CSB Grappling Hook
 * Copyright (C) 2026 counter-strike-boost.com
 *
 * A grappling hook bound to +hook. It traces from the player's eyes along their
 * aim to the first solid surface within range, anchors there, draws a laser beam
 * to the anchor and pulls the player toward it by setting pev_velocity every
 * frame in PlayerPreThink until they release the key or arrive. Includes a max
 * range, a per-use cooldown and optional VIP/flag gating.
 *
 * Inspired by the classic Hook plugin. Independent GPL re-implementation; no
 * original code is reused.
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or (at your option) any later
 * version. It is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE. See <https://www.gnu.org/licenses/> for details.
 */

#include <amxmodx>
#include <fakemeta>

new const PLUGIN[] = "CSB Grappling Hook"
new const VERSION[] = "1.0.0"
new const AUTHOR[]  = "counter-strike-boost.com"

#define ACCESS_FLAG ADMIN_LEVEL_H

#if !defined SVC_TEMPENTITY
    #define SVC_TEMPENTITY 23
#endif
#define TE_BEAMPOINTS 0

new g_pEnabled, g_pRange, g_pPull, g_pCooldown, g_pVipOnly

new bool:g_bHooking[33]
new Float:g_fHook[33][3]
new Float:g_fNextHook[33]
new g_iBeamSpr

public plugin_init()
{
    register_plugin(PLUGIN, VERSION, AUTHOR)

    g_pEnabled  = register_cvar("csb_hook_enabled", "1")
    g_pRange    = register_cvar("csb_hook_range", "1500")
    g_pPull     = register_cvar("csb_hook_pull", "700")
    g_pCooldown = register_cvar("csb_hook_cooldown", "3.0")
    g_pVipOnly  = register_cvar("csb_hook_viponly", "1")

    register_clcmd("+hook", "cmdHookStart")
    register_clcmd("-hook", "cmdHookStop")

    register_forward(FM_PlayerPreThink, "fwPreThink")
}

public plugin_precache()
{
    g_iBeamSpr = precache_model("sprites/laserbeam.spr")
}

public client_putinserver(id)
{
    g_bHooking[id]  = false
    g_fNextHook[id] = 0.0
}

public client_disconnected(id)
{
    g_bHooking[id] = false
}

bool:hasAccess(id)
{
    if (!get_pcvar_num(g_pVipOnly))
        return true

    return (get_user_flags(id) & ACCESS_FLAG) != 0
}

public cmdHookStart(id)
{
    if (!get_pcvar_num(g_pEnabled) || !is_user_alive(id))
        return PLUGIN_HANDLED

    if (!hasAccess(id))
    {
        client_print(id, print_chat, "[CSB] The grappling hook is a VIP feature.")
        return PLUGIN_HANDLED
    }

    if (get_gametime() < g_fNextHook[id])
    {
        new left = floatround(g_fNextHook[id] - get_gametime(), floatround_ceil)
        client_print(id, print_chat, "[CSB] Hook on cooldown (%d s).", left)
        return PLUGIN_HANDLED
    }

    static Float:fStart[3], Float:fOfs[3], Float:fAng[3], Float:fFwd[3], Float:fEnd[3]
    pev(id, pev_origin, fStart)
    pev(id, pev_view_ofs, fOfs)
    fStart[0] += fOfs[0]
    fStart[1] += fOfs[1]
    fStart[2] += fOfs[2]

    pev(id, pev_v_angle, fAng)
    angle_vector(fAng, ANGLEVECTOR_FORWARD, fFwd)

    new Float:range = get_pcvar_float(g_pRange)
    fEnd[0] = fStart[0] + fFwd[0] * range
    fEnd[1] = fStart[1] + fFwd[1] * range
    fEnd[2] = fStart[2] + fFwd[2] * range

    engfunc(EngFunc_TraceLine, fStart, fEnd, IGNORE_MONSTERS, id, 0)

    static Float:fFrac, Float:fHit[3]
    get_tr2(0, TR_flFraction, fFrac)
    get_tr2(0, TR_vecEndPos, fHit)

    if (fFrac >= 1.0)
    {
        client_print(id, print_chat, "[CSB] Nothing in hook range.")
        return PLUGIN_HANDLED
    }

    g_fHook[id][0] = fHit[0]
    g_fHook[id][1] = fHit[1]
    g_fHook[id][2] = fHit[2]
    g_bHooking[id] = true
    return PLUGIN_HANDLED
}

public cmdHookStop(id)
{
    if (g_bHooking[id])
    {
        g_bHooking[id]  = false
        g_fNextHook[id] = get_gametime() + get_pcvar_float(g_pCooldown)
    }
    return PLUGIN_HANDLED
}

public fwPreThink(id)
{
    if (!g_bHooking[id])
        return FMRES_IGNORED

    if (!is_user_alive(id))
    {
        g_bHooking[id] = false
        return FMRES_IGNORED
    }

    static Float:fOrigin[3], Float:fDir[3]
    pev(id, pev_origin, fOrigin)

    fDir[0] = g_fHook[id][0] - fOrigin[0]
    fDir[1] = g_fHook[id][1] - fOrigin[1]
    fDir[2] = g_fHook[id][2] - fOrigin[2]

    new Float:dist = vector_distance(fOrigin, g_fHook[id])

    if (dist < 64.0)
    {
        g_bHooking[id]  = false
        g_fNextHook[id] = get_gametime() + get_pcvar_float(g_pCooldown)
        return FMRES_IGNORED
    }

    if (dist > 0.0)
    {
        fDir[0] /= dist
        fDir[1] /= dist
        fDir[2] /= dist
    }

    new Float:pull = get_pcvar_float(g_pPull)

    static Float:fVel[3]
    fVel[0] = fDir[0] * pull
    fVel[1] = fDir[1] * pull
    fVel[2] = fDir[2] * pull
    set_pev(id, pev_velocity, fVel)

    drawBeam(fOrigin, g_fHook[id])
    return FMRES_IGNORED
}

drawBeam(const Float:from[3], const Float:to[3])
{
    message_begin(MSG_BROADCAST, SVC_TEMPENTITY)
    write_byte(TE_BEAMPOINTS)
    write_coord(floatround(from[0]))
    write_coord(floatround(from[1]))
    write_coord(floatround(from[2]))
    write_coord(floatround(to[0]))
    write_coord(floatround(to[1]))
    write_coord(floatround(to[2]))
    write_short(g_iBeamSpr)
    write_byte(0)
    write_byte(0)
    write_byte(1)
    write_byte(12)
    write_byte(0)
    write_byte(200)
    write_byte(200)
    write_byte(255)
    write_byte(200)
    write_byte(0)
    message_end()
    return;
}
