/*
 * Deathrun: bot trap activator and bot runners
 *
 * CZ bots don't know how to use deathrun trap buttons. When the terrorist (the activator) is a bot,
 * this plugin "presses" a trap button (func_button / func_rot_button) for it when a living CT is
 * close to something that button moves, breaks or triggers. Targets are followed through
 * multi_manager / trigger_relay chains (multi_manager keys are captured while the map loads).
 *
 * Cvars:
 *   dr_bot_traps        1     on/off
 *   dr_bot_trap_chance  35    % chance per check (4 checks/second) to press when a CT is in range
 *   dr_bot_trap_radius  260   how close (units) a CT must be to a trap part
 *   dr_bot_trap_delay   1.5   minimum seconds between two presses
 *   dr_bot_trap_range   0     max distance bot <-> button (0 = no limit)
 *
 * Bot runners (needs the gm_botnav plugin and a bot navigation mesh, maps/<map>.nav):
 * CZ bots don't know a deathrun course is something to run. Each CT bot is driven along the
 * course to its end - the place on the navigation mesh farthest (by walking distance) from the CT
 * spawn - jumping where the mesh says so. They leave a second or two apart, so the traps above get
 * a chance at each of them. A bot that sees the terrorist close by fights him, then runs on; at the
 * end of the course the bot's own brain takes over (and hunts the terrorist).
 *   dr_bot_run          1     on/off
 *   dr_bot_run_fight    450   fight the terrorist when he is seen within this many units (0 = never)
 */

#include <amxmodx>
#include <fakemeta>

#pragma dynamic 16384

#define MAX_BUTTONS 160
#define MAX_PTS     12
#define MAX_MM      256
#define TICK        0.25
#define SF_BUTTON_TOUCH_ONLY 256

// gm_botnav natives (optional: without the plugin the bot runners just stay off)
native botnav_has_mesh( );
native botnav_path_to( id, const Float:target[ 3 ], flags = 0 );
native botnav_stop( id );
native botnav_status( id );
native botnav_set_fight( id, Float:radius );
native botnav_flood( slot, const Float:origin[ 3 ] );
native Float:botnav_flood_dist( slot, area );
native botnav_farthest( slot, Float:out[ 3 ] );

#define BN_ARRIVED  3
#define BN_PERSIST  1

enum { RUN_NONE = 0, RUN_WAIT, RUN_GOING, RUN_DONE };

new bool:g_bBotNav, bool:g_bGoalTried, bool:g_bHaveGoal;
new Float:g_fGoal[ 3 ];
new g_iRun[ 33 ];
new Float:g_fRunAt[ 33 ];
new g_pRun, g_pRunFight;

new g_iButton[ MAX_BUTTONS ], g_iButtons;
new Float:g_fPts[ MAX_BUTTONS ][ MAX_PTS ][ 3 ], g_iPts[ MAX_BUTTONS ];
new Float:g_fNextUse[ MAX_BUTTONS ];
new Float:g_fNextAny;
new bool:g_bScanned;

// multi_manager entity -> its target names (collected while the map spawns)
new g_iMMEnt[ MAX_MM ], g_szMMTargets[ MAX_MM ][ 256 ], g_iMM;
new g_fwKeyValue;

new g_pOn, g_pChance, g_pRadius, g_pDelay, g_pRange;
new g_iMaxPlayers;

public plugin_precache( ) {
	g_fwKeyValue = register_forward( FM_KeyValue, "FwdKeyValue", 0 );
}

public FwdKeyValue( iEnt, kvd ) {
	if( !pev_valid( iEnt ) )
		return FMRES_IGNORED;

	static szClass[ 32 ], szKey[ 64 ], szValue[ 64 ];
	get_kvd( kvd, KV_ClassName, szClass, charsmax( szClass ) );

	if( !equal( szClass, "multi_manager" ) )
		return FMRES_IGNORED;

	get_kvd( kvd, KV_KeyName, szKey, charsmax( szKey ) );

	if( equal( szKey, "classname" ) || equal( szKey, "targetname" ) || equal( szKey, "origin" )
	|| equal( szKey, "wait" ) || equal( szKey, "spawnflags" ) || equal( szKey, "angles" ) )
		return FMRES_IGNORED;

	// Hammer writes duplicate keys as "name#1", "name#2"...
	new iHash = contain( szKey, "#" );
	if( iHash > 0 )
		szKey[ iHash ] = 0;

	new i;
	for( i = 0; i < g_iMM; i++ )
		if( g_iMMEnt[ i ] == iEnt )
			break;

	if( i == g_iMM ) {
		if( g_iMM >= MAX_MM )
			return FMRES_IGNORED;

		g_iMMEnt[ g_iMM ] = iEnt;
		g_szMMTargets[ g_iMM ][ 0 ] = 0;
		g_iMM++;
	}

	get_kvd( kvd, KV_Value, szValue, charsmax( szValue ) );

	if( strlen( g_szMMTargets[ i ] ) + strlen( szKey ) + 2 < charsmax( g_szMMTargets[ ] ) ) {
		add( g_szMMTargets[ i ], charsmax( g_szMMTargets[ ] ), szKey );
		add( g_szMMTargets[ i ], charsmax( g_szMMTargets[ ] ), " " );
	}

	return FMRES_IGNORED;
}

public plugin_init( ) {
	register_plugin( "DR Bot Traps", "1.0", "CZ panel" );

	if( g_fwKeyValue )
		unregister_forward( FM_KeyValue, g_fwKeyValue, 0 );

	g_pOn     = register_cvar( "dr_bot_traps",       "1" );
	g_pChance = register_cvar( "dr_bot_trap_chance", "35" );
	g_pRadius = register_cvar( "dr_bot_trap_radius", "260" );
	g_pDelay  = register_cvar( "dr_bot_trap_delay",  "1.5" );
	g_pRange  = register_cvar( "dr_bot_trap_range",  "0" );
	g_pRun      = register_cvar( "dr_bot_run",       "1" );
	g_pRunFight = register_cvar( "dr_bot_run_fight", "450" );
	g_bBotNav = bool:LibraryExists( "gm_botnav", LibType_Library );

	g_iMaxPlayers = get_maxplayers( );

	new szMap[ 32 ];
	get_mapname( szMap, charsmax( szMap ) );
	if( containi( szMap, "deathrun" ) == -1 && containi( szMap, "dr_" ) != 0 )
		return;

	register_event( "HLTV", "EventNewRound", "a", "1=0", "2=0" );
	register_logevent( "EventRoundStart", 2, "1=Round_Start" );
	set_task( TICK, "TaskThink", _, _, _, "b" );
	set_task( TICK, "TaskRunners", _, _, _, "b" );
}

public plugin_natives( ) {
	set_native_filter( "NativeFilter" );
}

public NativeFilter( const szNative[ ], iIndex, iTrap ) {
	return ( !iTrap && equal( szNative, "botnav_", 7 ) ) ? PLUGIN_HANDLED : PLUGIN_CONTINUE;
}

public EventNewRound( ) {
	arrayset( _:g_fNextUse, 0, MAX_BUTTONS );
	g_fNextAny = get_gametime( ) + 3.0;

	// runners start over once the round's respawn has put them back at the start
	for( new i = 1; i <= g_iMaxPlayers; i++ )
		StopRunner( i );
}

public EventRoundStart( ) {
	// freeze time is over: leave one after another
	new Float:fNow = get_gametime( );
	for( new i = 1; i <= g_iMaxPlayers; i++ ) {
		StopRunner( i );
		g_iRun[ i ] = RUN_WAIT;
		g_fRunAt[ i ] = fNow + random_float( 0.0, 2.5 );
	}
}

GetCenter( iEnt, Float:fOut[ 3 ] ) {
	new Float:fMin[ 3 ], Float:fMax[ 3 ];
	pev( iEnt, pev_absmin, fMin );
	pev( iEnt, pev_absmax, fMax );

	if( fMin[ 0 ] == fMax[ 0 ] && fMin[ 1 ] == fMax[ 1 ] && fMin[ 2 ] == fMax[ 2 ] ) {
		pev( iEnt, pev_origin, fOut );
		return;
	}

	fOut[ 0 ] = ( fMin[ 0 ] + fMax[ 0 ] ) * 0.5;
	fOut[ 1 ] = ( fMin[ 1 ] + fMax[ 1 ] ) * 0.5;
	fOut[ 2 ] = ( fMin[ 2 ] + fMax[ 2 ] ) * 0.5;
}

AddPoint( b, iEnt ) {
	if( g_iPts[ b ] >= MAX_PTS )
		return;

	// only things that physically exist: brush entities (doors, trains, breakables, hurt...)
	static szModel[ 4 ];
	pev( iEnt, pev_model, szModel, charsmax( szModel ) );
	if( szModel[ 0 ] != '*' )
		return;

	GetCenter( iEnt, g_fPts[ b ][ g_iPts[ b ] ] );
	g_iPts[ b ]++;
}

Resolve( b, const szName[ ], iDepth ) {
	if( !szName[ 0 ] || iDepth > 4 || g_iPts[ b ] >= MAX_PTS )
		return;

	new iEnt, iGuard;
	static szClass[ 32 ];
	new szTarget[ 64 ];

	while( ( iEnt = engfunc( EngFunc_FindEntityByString, iEnt, "targetname", szName ) ) > 0 && iGuard++ < 32 ) {
		pev( iEnt, pev_classname, szClass, charsmax( szClass ) );

		if( equal( szClass, "func_button" ) || equal( szClass, "func_rot_button" ) || equal( szClass, "player" ) )
			continue;

		if( equal( szClass, "multi_manager" ) ) {
			for( new i = 0; i < g_iMM; i++ ) {
				if( g_iMMEnt[ i ] != iEnt )
					continue;

				new szList[ 256 ], szOne[ 64 ];
				copy( szList, charsmax( szList ), g_szMMTargets[ i ] );

				while( szList[ 0 ] ) {
					strtok( szList, szOne, charsmax( szOne ), szList, charsmax( szList ), ' ' );
					trim( szList );
					if( szOne[ 0 ] && !equal( szOne, szName ) )
						Resolve( b, szOne, iDepth + 1 );
				}
				break;
			}
			continue;
		}

		AddPoint( b, iEnt );

		pev( iEnt, pev_target, szTarget, charsmax( szTarget ) );
		if( szTarget[ 0 ] && !equal( szTarget, szName ) && ( equal( szClass, "trigger_relay" ) || equal( szClass, "multisource" ) || equal( szClass, "env_render" ) ) )
			Resolve( b, szTarget, iDepth + 1 );
	}
}

ScanButtons( ) {
	g_bScanned = true;
	g_iButtons = 0;

	new const szClasses[ ][ ] = { "func_button", "func_rot_button" };
	new szTarget[ 64 ];

	for( new c = 0; c < sizeof szClasses; c++ ) {
		new iEnt;

		while( ( iEnt = engfunc( EngFunc_FindEntityByString, iEnt, "classname", szClasses[ c ] ) ) > 0 ) {
			if( g_iButtons >= MAX_BUTTONS )
				return;

			if( pev( iEnt, pev_spawnflags ) & SF_BUTTON_TOUCH_ONLY )
				continue;

			pev( iEnt, pev_target, szTarget, charsmax( szTarget ) );
			if( !szTarget[ 0 ] )
				continue;

			new b = g_iButtons;
			g_iPts[ b ] = 0;
			Resolve( b, szTarget, 0 );

			if( g_iPts[ b ] > 0 ) {
				g_iButton[ b ] = iEnt;
				g_iButtons++;
			}
		}
	}
}

public TaskThink( ) {
	if( !get_pcvar_num( g_pOn ) )
		return;

	if( !g_bScanned )
		ScanButtons( );

	if( !g_iButtons )
		return;

	new Float:fNow = get_gametime( );
	if( fNow < g_fNextAny )
		return;

	// find a living bot terrorist and the living CTs
	new iBot, iCTs[ 32 ], iNumCT;
	for( new i = 1; i <= g_iMaxPlayers; i++ ) {
		if( !is_user_alive( i ) )
			continue;

		new iTeam = get_user_team( i );
		if( iTeam == 1 && !iBot && is_user_bot( i ) )
			iBot = i;
		else if( iTeam == 2 )
			iCTs[ iNumCT++ ] = i;
	}

	if( !iBot || !iNumCT )
		return;

	if( random_num( 1, 100 ) > get_pcvar_num( g_pChance ) )
		return;

	new Float:fRadius = get_pcvar_float( g_pRadius );
	new Float:fRange = get_pcvar_float( g_pRange );
	new Float:fBot[ 3 ], Float:fCT[ 3 ], Float:fBtn[ 3 ];
	pev( iBot, pev_origin, fBot );

	// start at a random button so the same one isn't always preferred
	new iStart = random_num( 0, g_iButtons - 1 );

	for( new n = 0; n < g_iButtons; n++ ) {
		new b = ( iStart + n ) % g_iButtons;
		new iBtn = g_iButton[ b ];

		if( fNow < g_fNextUse[ b ] || !pev_valid( iBtn ) )
			continue;

		GetCenter( iBtn, fBtn );

		if( fRange > 0.0 && get_distance_f( fBot, fBtn ) > fRange )
			continue;

		// don't spring a trap on ourselves
		new bool:bSelf;
		for( new p = 0; p < g_iPts[ b ] && !bSelf; p++ )
			if( get_distance_f( fBot, g_fPts[ b ][ p ] ) < fRadius )
				bSelf = true;
		if( bSelf )
			continue;

		new bool:bHit, bool:bCTAtButton;

		for( new c = 0; c < iNumCT && !bCTAtButton; c++ ) {
			pev( iCTs[ c ], pev_origin, fCT );

			// lead the trap: where the CT will be in ~0.4 s
			new Float:fVel[ 3 ], Float:fLead[ 3 ];
			pev( iCTs[ c ], pev_velocity, fVel );
			fLead[ 0 ] = fCT[ 0 ] + fVel[ 0 ] * 0.4;
			fLead[ 1 ] = fCT[ 1 ] + fVel[ 1 ] * 0.4;
			fLead[ 2 ] = fCT[ 2 ] + fVel[ 2 ] * 0.4;

			// a CT standing at the button is probably using it himself - leave it alone
			if( get_distance_f( fCT, fBtn ) < 120.0 ) {
				bCTAtButton = true;
				break;
			}

			for( new p = 0; p < g_iPts[ b ] && !bHit; p++ )
				if( get_distance_f( fLead, g_fPts[ b ][ p ] ) < fRadius )
					bHit = true;
		}

		if( !bHit || bCTAtButton )
			continue;

		// the same thing a player's +use does: CBaseButton::Use -> ButtonUse (ignored while moving/locked)
		dllfunc( DLLFunc_Use, iBtn, iBot );

		g_fNextUse[ b ] = fNow + 12.0;
		g_fNextAny = fNow + get_pcvar_float( g_pDelay );
		return;
	}
}

/*
 * Bot runners
 */

StopRunner( id ) {
	if( g_iRun[ id ] == RUN_GOING && g_bBotNav )
		botnav_stop( id );
	g_iRun[ id ] = RUN_NONE;
}

// The end of the course: the place on the mesh farthest (walking) from the CT spawn
FindGoal( ) {
	g_bGoalTried = true;
	new iEnt = engfunc( EngFunc_FindEntityByString, 0, "classname", "info_player_start" );
	if( iEnt <= 0 )
		return;

	new Float:fStart[ 3 ];
	pev( iEnt, pev_origin, fStart );
	if( botnav_flood( 0, fStart ) < 2 )
		return;

	new iArea = botnav_farthest( 0, g_fGoal );
	if( iArea < 0 || botnav_flood_dist( 0, iArea ) < 400.0 )
		return;

	g_bHaveGoal = true;
	log_amx( "Deathrun bots: course end at %.0f %.0f %.0f, %.0f units from the start",
		g_fGoal[ 0 ], g_fGoal[ 1 ], g_fGoal[ 2 ], botnav_flood_dist( 0, iArea ) );
}

public TaskRunners( ) {
	if( !g_bBotNav )
		return;

	new bool:bOn = get_pcvar_num( g_pRun ) && botnav_has_mesh( );
	if( bOn && !g_bGoalTried )
		FindGoal( );
	bOn = bOn && g_bHaveGoal;

	new Float:fNow = get_gametime( );
	for( new i = 1; i <= g_iMaxPlayers; i++ ) {
		if( !bOn || !is_user_alive( i ) || !is_user_bot( i ) || get_user_team( i ) != 2 ) {
			StopRunner( i );
			continue;
		}

		switch( g_iRun[ i ] ) {
			case RUN_NONE: {
				// joined or came back to life (deathrun_lifesystem) mid-round
				g_iRun[ i ] = RUN_WAIT;
				g_fRunAt[ i ] = fNow + random_float( 0.5, 1.5 );
			}
			case RUN_WAIT: {
				if( fNow >= g_fRunAt[ i ] ) {
					botnav_set_fight( i, get_pcvar_float( g_pRunFight ) );
					botnav_path_to( i, g_fGoal, BN_PERSIST );
					g_iRun[ i ] = RUN_GOING;
				}
			}
			case RUN_GOING: {
				if( botnav_status( i ) == BN_ARRIVED ) {
					// made it: the bot's own brain takes it from here
					botnav_stop( i );
					g_iRun[ i ] = RUN_DONE;
				}
			}
		}
	}
}
