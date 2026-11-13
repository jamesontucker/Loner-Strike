// Starting points for a new plugin, and the snippets the editor offers while typing.
// Templates are whole files; snippets are Monaco snippet text (${1:name} placeholders).

export type Template = { id: string; label: string; blurb: string; text: (name: string) => string };

const head = (name: string, what: string) => `/*
	${name} - ${what}
*/

#include <amxmodx>
`;

export const TEMPLATES: Template[] = [
  {
    id: "blank",
    label: "Empty plugin",
    blurb: "Just the basics: a name and plugin_init.",
    text: (n) => `${head(n, "my plugin")}
public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
}
`,
  },
  {
    id: "mode",
    label: "New game mode",
    blurb: "A mode for the Game Mode Menu, with loner.inc: gear on spawn, a countdown, a HUD line.",
    text: (n) => `${head(n, "a game mode for Loner-Strike's Game Mode Menu")}#include <hamsandwich>
#include <loner>

// To play it: add a line to configs/gamemodes/modes.ini, e.g.
//   ${n}  "My Mode"  "What it's about"  ""  ""
// and list ${n}.amxx in configs/gamemodes/${n}.plugins.ini

public plugin_init()
{
	ls_register_mode("${n}", "My Mode", "1.0", "me")
	RegisterHamPlayer(Ham_Spawn, "on_spawn", 1)
	register_logevent("on_round_start", 2, "1=Round_Start")
}

public on_round_start()
{
	ls_countdown(3, "Go in", "on_go")
}

public on_go()
{
	ls_announce(0, 2.0, "GO!")
}

public on_spawn(id)
{
	if (!ls_alive(id) || !ls_mode_active())
		return
	ls_strip(id)
	ls_give_weapon(id, "ak47", 90)
	ls_set_armor(id, 100)
	ls_hud(id, LS_HUD_LEFT, 5.0, "My Mode^nHave fun!")
}
`,
  },
  {
    id: "command",
    label: "New chat command",
    blurb: "Players type /hello in chat and get an answer.",
    text: (n) => `${head(n, "a chat command")}
public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	register_clcmd("say /hello", "cmd_hello")
	register_clcmd("say_team /hello", "cmd_hello")
}

public cmd_hello(id)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	client_print_color(id, print_team_default, "^4[Server]^1 Hello, %s!", name)
	return PLUGIN_HANDLED
}
`,
  },
  {
    id: "killed",
    label: "On player killed",
    blurb: "Do something when someone is killed: here, a headshot bonus.",
    text: (n) => `${head(n, "something on every kill")}#include <cstrike>

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	register_event("DeathMsg", "on_death", "a")
}

public on_death()
{
	new killer = read_data(1), victim = read_data(2), headshot = read_data(3)
	if (!is_user_connected(killer) || killer == victim)
		return
	if (headshot)
	{
		cs_set_user_money(killer, min(cs_get_user_money(killer) + 500, 16000))
		client_print(killer, print_center, "Headshot bonus: $500")
	}
}
`,
  },
  {
    id: "give",
    label: "Give items on spawn",
    blurb: "Everyone gets armor and a grenade when they spawn.",
    text: (n) => `${head(n, "items on spawn")}#include <fun>
#include <cstrike>
#include <hamsandwich>

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	RegisterHamPlayer(Ham_Spawn, "on_spawn", 1)
}

public on_spawn(id)
{
	if (!is_user_alive(id))
		return
	cs_set_user_armor(id, 100, CS_ARMOR_VESTHELM)
	give_item(id, "weapon_hegrenade")
}
`,
  },
  {
    id: "hud",
    label: "HUD message",
    blurb: "A line of text on everyone's screen every few seconds.",
    text: (n) => `${head(n, "a HUD message")}
new g_Sync

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	g_Sync = CreateHudSyncObj()
	set_task(10.0, "show_message", _, _, _, "b")
}

public show_message()
{
	set_hudmessage(0, 200, 120, -1.0, 0.2, 0, 0.0, 4.0, 0.2, 0.5, -1)
	ShowSyncHudMsg(0, g_Sync, "Welcome! Type /help in chat.")
}
`,
  },
  {
    id: "menu",
    label: "Menu",
    blurb: "A menu players open with /guns and pick from.",
    text: (n) => `${head(n, "a menu")}#include <fun>

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	register_clcmd("say /guns", "show_menu")
}

public show_menu(id)
{
	new menu = menu_create("Pick a gun", "menu_picked")
	menu_additem(menu, "AK-47", "weapon_ak47")
	menu_additem(menu, "M4A1", "weapon_m4a1")
	menu_additem(menu, "AWP", "weapon_awp")
	menu_display(id, menu)
	return PLUGIN_HANDLED
}

public menu_picked(id, menu, item)
{
	if (item == MENU_EXIT || !is_user_alive(id))
	{
		menu_destroy(menu)
		return PLUGIN_HANDLED
	}
	new weapon[32], access, callback
	menu_item_getinfo(menu, item, access, weapon, charsmax(weapon), _, _, callback)
	give_item(id, weapon)
	menu_destroy(menu)
	return PLUGIN_HANDLED
}
`,
  },
  {
    id: "timer",
    label: "Timer",
    blurb: "Something every minute, and once 5 seconds after a player joins.",
    text: (n) => `${head(n, "timers")}
#define TASK_WELCOME 1000

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	set_task(60.0, "every_minute", _, _, _, "b")
}

public every_minute()
{
	client_print(0, print_chat, "Another minute has passed.")
}

public client_putinserver(id)
{
	set_task(5.0, "welcome", TASK_WELCOME + id)
}

public client_disconnected(id)
{
	remove_task(TASK_WELCOME + id)
}

public welcome(task)
{
	new id = task - TASK_WELCOME
	if (is_user_connected(id))
		client_print(id, print_chat, "Welcome to the server!")
}
`,
  },
];

/** Snippets offered by the editor's completion: label, what it does, the snippet text. */
export const SNIPPETS: [string, string, string][] = [
  // the plugin's start
  ["plugin_init", "The plugin's start: register_plugin and your hooks", 'public plugin_init()\n{\n\tregister_plugin("${1:My plugin}", "${2:1.0}", "${3:me}")\n\t$0\n}'],
  ["plugin_precache", "Load models, sounds and sprites before the map starts", 'public plugin_precache()\n{\n\tprecache_sound("${1:buttons/blip1.wav}")\n\t$0\n}'],
  ["plugin_cfg", "After the configs ran: read your cvars here", "public plugin_cfg()\n{\n\t$0\n}"],
  ["plugin_end", "The map is ending: save what you need", "public plugin_end()\n{\n\t$0\n}"],
  // commands
  ["cmd", "A chat command (/word)", 'register_clcmd("say /${1:hello}", "${2:cmd_hello}")\nregister_clcmd("say_team /${1:hello}", "${2:cmd_hello}")\n\npublic ${2:cmd_hello}(id)\n{\n\t$0\n\treturn PLUGIN_HANDLED\n}'],
  ["clcmd", "A console command for players (register_clcmd)", 'register_clcmd("${1:my_command}", "${2:cmd_mine}", ${3:-1}, "${4:what it does}")\n\npublic ${2:cmd_mine}(id)\n{\n\t$0\n\treturn PLUGIN_HANDLED\n}'],
  ["concmd", "An admin command with an access flag (register_concmd + cmd_access, needs amxmisc)", 'register_concmd("${1:amx_mycmd}", "${2:cmd_admin}", ${3:ADMIN_KICK}, "<${4:target}> - ${5:what it does}")\n\npublic ${2:cmd_admin}(id, level, cid)\n{\n\tif (!cmd_access(id, level, cid, ${6:2}))\n\t\treturn PLUGIN_HANDLED\n\tnew arg[32]\n\tread_argv(1, arg, charsmax(arg))\n\t$0\n\treturn PLUGIN_HANDLED\n}'],
  ["srvcmd", "A server console command (register_srvcmd)", 'register_srvcmd("${1:my_srvcmd}", "${2:srv_mine}")\n\npublic ${2:srv_mine}()\n{\n\t$0\n\treturn PLUGIN_HANDLED\n}'],
  // players coming and going
  ["putinserver", "A player joined the game (client_putinserver)", "public client_putinserver(id)\n{\n\t$0\n}"],
  ["disconnected", "A player left (client_disconnected)", "public client_disconnected(id)\n{\n\t$0\n}"],
  ["authorized", "A player's Steam ID is known (client_authorized)", "public client_authorized(id, const authid[])\n{\n\t$0\n}"],
  // events (RegisterHamPlayer, not RegisterHam(..., \"player\"): ReGameDLL's bots have their own class)
  ["spawn", "When a player (or bot) spawns: RegisterHamPlayer, needs hamsandwich", 'RegisterHamPlayer(Ham_Spawn, "${1:on_spawn}", 1)\n\npublic ${1:on_spawn}(id)\n{\n\tif (!is_user_alive(id))\n\t\treturn\n\t$0\n}'],
  ["killed", "When a player (or bot) is killed: RegisterHamPlayer(Ham_Killed)", 'RegisterHamPlayer(Ham_Killed, "${1:on_killed}", 1)\n\npublic ${1:on_killed}(victim, killer, shouldgib)\n{\n\t$0\n}'],
  ["damage", "Before a player takes damage (change or block it)", 'RegisterHamPlayer(Ham_TakeDamage, "${1:on_damage}")\n\npublic ${1:on_damage}(victim, inflictor, attacker, Float:damage, damagebits)\n{\n\t$0\n\treturn HAM_IGNORED\n}'],
  ["death", "When a player is killed (DeathMsg: killer, victim, headshot, weapon)", 'register_event("DeathMsg", "${1:on_death}", "a")\n\npublic ${1:on_death}()\n{\n\tnew killer = read_data(1), victim = read_data(2), headshot = read_data(3)\n\t$0\n}'],
  ["roundstart", "When a round starts (after freeze time)", 'register_logevent("${1:on_round_start}", 2, "1=Round_Start")\n\npublic ${1:on_round_start}()\n{\n\t$0\n}'],
  ["newround", "A new round (freeze time begins)", 'register_event("HLTV", "${1:on_new_round}", "a", "1=0", "2=0")\n\npublic ${1:on_new_round}()\n{\n\t$0\n}'],
  ["roundend", "When a round ends", 'register_logevent("${1:on_round_end}", 2, "1=Round_End")\n\npublic ${1:on_round_end}()\n{\n\t$0\n}'],
  // doing things
  ["give", "Give a weapon (needs fun)", 'give_item(${1:id}, "weapon_${2:ak47}")'],
  ["givefull", "Give a weapon with spare ammo (loner.inc)", 'ls_give_weapon(${1:id}, "${2:ak47}", ${3:90})'],
  ["hud", "A HUD message (its own sync object, so other plugins don't wipe it)", 'new g_Sync = CreateHudSyncObj()\n\nset_hudmessage(${1:255}, ${2:200}, ${3:60}, -1.0, ${4:0.25}, 0, 0.0, ${5:4.0}, 0.1, 0.2, -1)\nShowSyncHudMsg(${6:0}, g_Sync, "${7:Hello}")'],
  ["lshud", "A HUD line in a shared slot (loner.inc)", 'ls_hud(${1:0}, ${2|LS_HUD_TOP,LS_HUD_LEFT,LS_HUD_RIGHT,LS_HUD_CENTER|}, ${3:4.0}, "${4:Hello}")'],
  ["chat", "A colored chat line", 'client_print_color(${1:0}, print_team_default, "^4[${2:Server}]^1 ${3:text}")'],
  ["center", "Text in the middle of the screen", 'client_print(${1:0}, print_center, "${2:text}")'],
  // timers
  ["task", "Run a function later (set_task)", 'set_task(${1:5.0}, "${2:later}")\n\npublic ${2:later}()\n{\n\t$0\n}'],
  ["repeat", "Run a function every few seconds", 'set_task(${1:10.0}, "${2:every}", _, _, _, "b")\n\npublic ${2:every}()\n{\n\t$0\n}'],
  ["playertask", "A timer per player (task id = TASK + player), removed when they leave", '#define TASK_${1:LATER} ${2:1000}\n\nset_task(${3:5.0}, "${4:player_later}", TASK_${1:LATER} + id)\n\npublic ${4:player_later}(task)\n{\n\tnew id = task - TASK_${1:LATER}\n\tif (!is_user_connected(id))\n\t\treturn\n\t$0\n}'],
  ["countdown", "A countdown on everyone's screen, then your function (loner.inc)", 'ls_countdown(${1:5}, "${2:Starts in}", "${3:on_go}")'],
  // menus
  ["menu", "A menu with items and its handler (newmenus)", 'public ${1:show_menu}(id)\n{\n\tnew menu = menu_create("${2:Title}", "${1:show_menu}_picked")\n\tmenu_additem(menu, "${3:First}", "1")\n\tmenu_additem(menu, "${4:Second}", "2")\n\tmenu_display(id, menu)\n\treturn PLUGIN_HANDLED\n}\n\npublic ${1:show_menu}_picked(id, menu, item)\n{\n\tif (item == MENU_EXIT)\n\t{\n\t\tmenu_destroy(menu)\n\t\treturn PLUGIN_HANDLED\n\t}\n\tnew info[8], access, callback\n\tmenu_item_getinfo(menu, item, access, info, charsmax(info), _, _, callback)\n\t$0\n\tmenu_destroy(menu)\n\treturn PLUGIN_HANDLED\n}'],
  // settings
  ["cvar", "A setting the server can change (create_cvar + bind_pcvar_num)", 'new g_${1:Amount}\n\nbind_pcvar_num(create_cvar("${2:my_amount}", "${3:10}", _, "${4:What it does}", true, ${5:0.0}), g_${1:Amount})'],
  ["cvarfloat", "A decimal setting (bind_pcvar_float)", 'new Float:g_${1:Speed}\n\nbind_pcvar_float(create_cvar("${2:my_speed}", "${3:1.5}", _, "${4:What it does}"), g_${1:Speed})'],
  ["cvartext", "A text setting (bind_pcvar_string)", 'new g_${1:Tag}[32]\n\nbind_pcvar_string(create_cvar("${2:my_tag}", "${3:[Server]}", _, "${4:What it does}"), g_${1:Tag}, charsmax(g_${1:Tag}))'],
  // loops and flow
  ["players", "Loop over every player", "for (new ${1:id} = 1; ${1:id} <= MaxClients; ${1:id}++)\n{\n\tif (!is_user_connected(${1:id}))\n\t\tcontinue\n\t$0\n}"],
  ["alive", "Every living player (get_players)", 'new players[MAX_PLAYERS], count\nget_players(players, count, "a")\nfor (new i = 0; i < count; i++)\n{\n\tnew id = players[i]\n\t$0\n}'],
  ["for", "A for loop", "for (new ${1:i} = 0; ${1:i} < ${2:count}; ${1:i}++)\n{\n\t$0\n}"],
  ["if", "An if", "if (${1:condition})\n{\n\t$0\n}"],
  ["switch", "A switch", "switch (${1:value})\n{\n\tcase ${2:0}:\n\t{\n\t\t$0\n\t}\n\tdefault:\n\t{\n\t}\n}"],
  // Loner-Strike
  ["mode", "Register a Game Mode Menu mode (loner.inc)", 'ls_register_mode("${1:mymode}", "${2:My Mode}", "${3:1.0}", "${4:me}")'],
];
