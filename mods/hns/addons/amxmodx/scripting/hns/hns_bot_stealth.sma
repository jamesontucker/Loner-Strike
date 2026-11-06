/*
 * Hide and Seek: darkness hides you from bot seekers
 *
 * CZ bots see in the dark as well as in daylight. ReGameDLL's bots skip anyone with
 * FL_NOTARGET when they look for enemies (CCSBot::IsVisible, cs_bot_vision.cpp), and
 * nothing else in Counter-Strike uses that flag for players. So a hider who keeps still
 * in a dark spot gets it, and loses it again on moving, being in light, attacking or a
 * seeker walking right into them. Once seen, a hider stays seen for a moment, so bots
 * don't lose someone the instant they stop.
 *
 * Human seekers are unaffected: this only changes what bots notice.
 *
 * Cvars:
 *   hns_bot_stealth        1     on/off
 *   hns_bot_stealth_dark   40    how dark (the engine's light level, 0-255) a spot must be
 *                                to hide someone standing; crouching hides in a bit more light
 *   hns_bot_stealth_near   150   a seeker this close (units) finds you anyway
 */

#include <amxmodx>
#include <fakemeta>

#define TICK          0.1
#define REVEAL_HOLD   1.5     // seconds a hider stays visible after being given away
#define CROUCH_BONUS  20
#define STILL_SPEED   60.0    // creeping is fine, walking isn't

new g_pOn, g_pDark, g_pNear;
new bool:g_bSet[ 33 ];            // we set FL_NOTARGET on this player (never touch it otherwise)
new Float:g_fShownUntil[ 33 ];
new g_iMaxPlayers;

public plugin_init( ) {
	register_plugin( "HNS Bot Stealth", "1.0", "Loner-Strike" );

	g_pOn   = register_cvar( "hns_bot_stealth", "1" );
	g_pDark = register_cvar( "hns_bot_stealth_dark", "40" );
	g_pNear = register_cvar( "hns_bot_stealth_near", "150" );
	g_iMaxPlayers = get_maxplayers( );

	register_forward( FM_CmdStart, "FwdCmdStart", 0 );
	register_event( "HLTV", "EventNewRound", "a", "1=0", "2=0" );
	register_event( "DeathMsg", "EventDeath", "a" );
	set_task( TICK, "TaskThink", _, _, _, "b" );
}

public plugin_end( ) {
	for( new i = 1; i <= g_iMaxPlayers; i++ )
		Show( i );
}

public client_disconnected( id ) {
	g_bSet[ id ] = false;
	g_fShownUntil[ id ] = 0.0;
}

public EventNewRound( ) {
	for( new i = 1; i <= g_iMaxPlayers; i++ ) {
		Show( i );
		g_fShownUntil[ i ] = 0.0;
	}
}

public EventDeath( ) {
	Show( read_data( 2 ) );
}

// attacking or throwing gives you away
public FwdCmdStart( id, uc ) {
	if( id < 1 || id > g_iMaxPlayers || !g_bSet[ id ] && g_fShownUntil[ id ] < get_gametime( ) - 5.0 )
		return FMRES_IGNORED;

	if( get_uc( uc, UC_Buttons ) & ( IN_ATTACK | IN_ATTACK2 ) )
		Reveal( id );

	return FMRES_IGNORED;
}

Hide( id ) {
	if( g_bSet[ id ] || !is_user_alive( id ) )
		return;
	set_pev( id, pev_flags, pev( id, pev_flags ) | FL_NOTARGET );
	g_bSet[ id ] = true;
}

Show( id ) {
	if( id < 1 || id > g_iMaxPlayers || !g_bSet[ id ] )
		return;
	g_bSet[ id ] = false;
	if( is_user_connected( id ) )
		set_pev( id, pev_flags, pev( id, pev_flags ) & ~FL_NOTARGET );
}

Reveal( id ) {
	Show( id );
	g_fShownUntil[ id ] = get_gametime( ) + REVEAL_HOLD;
}

public TaskThink( ) {
	if( !get_pcvar_num( g_pOn ) ) {
		for( new i = 1; i <= g_iMaxPlayers; i++ )
			Show( i );
		return;
	}

	// the seekers that matter: living bot CTs
	new iSeekers[ 32 ], iNum;
	for( new i = 1; i <= g_iMaxPlayers; i++ )
		if( is_user_alive( i ) && is_user_bot( i ) && get_user_team( i ) == 2 )
			iSeekers[ iNum++ ] = i;

	new Float:fNow = get_gametime( );
	new Float:fNear = get_pcvar_float( g_pNear );
	new iDark = get_pcvar_num( g_pDark );

	for( new id = 1; id <= g_iMaxPlayers; id++ ) {
		if( !is_user_alive( id ) || get_user_team( id ) != 1 || !iNum ) {
			Show( id );
			continue;
		}

		if( fNow < g_fShownUntil[ id ] )
			continue;

		new Float:fVel[ 3 ];
		pev( id, pev_velocity, fVel );
		new bool:bCrouched = ( pev( id, pev_flags ) & FL_DUCKING ) != 0;
		new bool:bStill = floatsqroot( fVel[ 0 ] * fVel[ 0 ] + fVel[ 1 ] * fVel[ 1 ] ) < STILL_SPEED && fVel[ 2 ] == 0.0;

		new iLight = engfunc( EngFunc_GetEntityIllum, id );
		new bool:bDark = iLight < iDark + ( bCrouched ? CROUCH_BONUS : 0 );

		// a seeker right on top of you finds you, dark or not
		new bool:bBumped;
		new Float:fMe[ 3 ], Float:fThem[ 3 ];
		pev( id, pev_origin, fMe );
		for( new s = 0; s < iNum && !bBumped; s++ ) {
			pev( iSeekers[ s ], pev_origin, fThem );
			if( get_distance_f( fMe, fThem ) < fNear )
				bBumped = true;
		}

		if( bDark && bStill && !bBumped )
			Hide( id );
		else if( g_bSet[ id ] )
			Reveal( id );
	}
}
