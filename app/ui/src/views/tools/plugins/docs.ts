// Where to read more: AMX Mod X's API reference for a word from its own includes, and the
// guides the Help panel lists. (No Monaco here: the page and the editor both use it.)

import type { Word } from "./pluginsApi";

/** The includes AMX Mod X's API reference (amxmodx.org/api) has pages for. */
const API_INCLUDES = new Set(
  "amxconst amxmisc amxmodx_version amxmodx cellarray cellstack celltrie core csstats_const csstats cstrike_const cstrike csx cvars datapack dbi dodconst dodfun dodstats dodx engine_const engine_stocks engine esf_const esf fakemeta_const fakemeta_stocks fakemeta_util fakemeta file float fun gameconfig geoip ham_const hamsandwich hlsdk_const json lang message_const message_stocks messages newmenus ns_const ns ns2amx nvault regex sockets sorting sqlx string_const string_stocks string textparse_ini textparse_smc tfcconst tfcstats tfcx time tsconst tsfun tsstats tsx vault vector xs".split(" "),
);

/** The word's page in AMX Mod X's API reference: functions have their own page, constants
 *  are on their include's page. Only for AMX Mod X's own includes (not a mod's copy in a folder). */
export function docsUrl(s: Pick<Word, "name" | "kind" | "file">): string | null {
  if (s.file.includes("/")) return null;
  const inc = s.file.replace(/\.inc$/, "");
  if (!API_INCLUDES.has(inc)) return null;
  const fn = s.kind === "native" || s.kind === "forward" || s.kind === "stock";
  return fn ? `https://www.amxmodx.org/api/${inc}/${encodeURIComponent(s.name)}` : `https://www.amxmodx.org/api/${inc}`;
}

export type Guide = { title: string; url: string; what: string };

/** The guides worth reading, best first. */
export const GUIDES: Guide[] = [
  { title: "AMX Mod X API reference", url: "https://www.amxmodx.org/api/", what: "Every native and forward of AMX Mod X and its modules, with examples. The words in the editor link here." },
  { title: "Intro to AMX Mod X scripting", url: "https://wiki.alliedmods.net/Intro_to_AMX_Mod_X_Scripting", what: "Your first plugin: registering it, commands, cvars, events." },
  { title: "Pawn tutorial", url: "https://wiki.alliedmods.net/Pawn_Tutorial", what: "The language itself: variables, arrays, strings, tags (Float:), functions." },
  { title: "Advanced scripting", url: "https://wiki.alliedmods.net/Advanced_Scripting_(AMX_Mod_X)", what: "Tasks, menus, forwards between plugins, natives, dynamic arrays." },
  { title: "AMX Mod X scripting articles", url: "https://wiki.alliedmods.net/Category:Scripting_(AMX_Mod_X)", what: "The AlliedModders wiki's scripting pages: optimizing, translations, SQL, more." },
  { title: "Half-Life game events", url: "https://wiki.alliedmods.net/Half-Life_1_Game_Events", what: "The messages and events you can hook (DeathMsg, ResetHUD, …) and what they carry." },
  { title: "AMX Mod X documentation", url: "https://wiki.alliedmods.net/Category:Documentation_(AMX_Mod_X)", what: "Installing, configuring, plugins.ini, the compiler and its options." },
  { title: "AlliedModders forums", url: "https://forums.alliedmods.net/", what: "Ask in AMX Mod X ▸ Scripting Help; Code Snippets/Tutorials has worked examples." },
  { title: "The Pawn language (CompuPhase)", url: "https://www.compuphase.com/pawn/pawn.htm", what: "Pawn's own manual. AMX Mod X uses an older Pawn (3.x) with ^ as the escape character." },
  { title: "ReGameDLL_CS", url: "https://github.com/rehlds/ReGameDLL_CS", what: "The game library Loner-Strike runs: its cvars and commands. Use RegisterHamPlayer, not RegisterHam(…, \"player\"), so bots are hooked too." },
  { title: "ReAPI", url: "https://github.com/rehlds/ReAPI", what: "Hooks and members of ReGameDLL from plugins (not installed by Loner-Strike yet)." },
];
