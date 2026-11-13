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
	RegisterHam(Ham_Spawn, "player", "on_spawn", 1)
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
    blurb: "Everyone gets armour and a grenade when they spawn.",
    text: (n) => `${head(n, "items on spawn")}#include <fun>
#include <cstrike>
#include <hamsandwich>

public plugin_init()
{
	register_plugin("${n}", "1.0", "me")
	RegisterHam(Ham_Spawn, "player", "on_spawn", 1)
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
  ["plugin_init", "The plugin's start: register_plugin and your hooks", 'public plugin_init()\n{\n\tregister_plugin("${1:My plugin}", "${2:1.0}", "${3:me}")\n\t$0\n}'],
  ["cmd", "A chat command (/word)", 'register_clcmd("say /${1:hello}", "${2:cmd_hello}")\n\npublic ${2:cmd_hello}(id)\n{\n\t$0\n\treturn PLUGIN_HANDLED\n}'],
  ["death", "When a player is killed (DeathMsg)", 'register_event("DeathMsg", "${1:on_death}", "a")\n\npublic ${1:on_death}()\n{\n\tnew killer = read_data(1), victim = read_data(2)\n\t$0\n}'],
  ["spawn", "When a player spawns (Ham_Spawn, needs hamsandwich)", 'RegisterHam(Ham_Spawn, "player", "${1:on_spawn}", 1)\n\npublic ${1:on_spawn}(id)\n{\n\tif (!is_user_alive(id))\n\t\treturn\n\t$0\n}'],
  ["roundstart", "When a round starts", 'register_logevent("${1:on_round_start}", 2, "1=Round_Start")\n\npublic ${1:on_round_start}()\n{\n\t$0\n}'],
  ["give", "Give a weapon (needs fun)", 'give_item(${1:id}, "weapon_${2:ak47}")'],
  ["hud", "A HUD message", 'set_hudmessage(${1:255}, ${2:200}, ${3:60}, -1.0, ${4:0.25}, 0, 0.0, ${5:4.0}, 0.1, 0.2, -1)\nshow_hudmessage(${6:0}, "${7:Hello}")'],
  ["task", "Run a function later (set_task)", 'set_task(${1:5.0}, "${2:later}")\n\npublic ${2:later}()\n{\n\t$0\n}'],
  ["repeat", "Run a function every few seconds", 'set_task(${1:10.0}, "${2:every}", _, _, _, "b")\n\npublic ${2:every}()\n{\n\t$0\n}'],
  ["menu", "A menu with items", 'new menu = menu_create("${1:Title}", "${2:menu_picked}")\nmenu_additem(menu, "${3:First}")\nmenu_display(${4:id}, menu)'],
  ["chat", "A coloured chat line", 'client_print_color(${1:0}, print_team_default, "^4[${2:Server}]^1 ${3:text}")'],
  ["players", "Loop over every player", "for (new ${1:id} = 1; ${1:id} <= MaxClients; ${1:id}++)\n{\n\tif (!is_user_connected(${1:id}))\n\t\tcontinue\n\t$0\n}"],
  ["for", "A for loop", "for (new ${1:i} = 0; ${1:i} < ${2:count}; ${1:i}++)\n{\n\t$0\n}"],
  ["if", "An if", "if (${1:condition})\n{\n\t$0\n}"],
  ["switch", "A switch", "switch (${1:value})\n{\n\tcase ${2:0}:\n\t{\n\t\t$0\n\t}\n\tdefault:\n\t{\n\t}\n}"],
  ["cvar", "A setting players can change (cvar)", 'new g_p${1:Name} = create_cvar("${2:my_setting}", "${3:1}", _, "${4:What it does}")'],
];
