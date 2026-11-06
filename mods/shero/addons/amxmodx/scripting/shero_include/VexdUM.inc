/* VexdUM backwards compatibility
 *
 * by the AMX Mod X Development Team
 *
 * This file is provided as is (no warranties).
 */


#if !defined _fakemeta_included
	#include <fakemeta>
#endif

#if !defined _engine_included
	#include <engine>
#endif

#if defined _vexd_bcompat_included
  #endinput
#endif
#define _vexd_bcompat_included

#include <VexdUM_const>

/* SuperHero/CZ: VexdUM natives+forwards removed - not provided by AMX Mod X 1.10 (no amxmod_compat).
   Plugins needing them fail to compile instead of failing to load. */

#include <VexdUM_stock>
