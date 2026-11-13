# loner.inc: Loner-Strike's plugin helpers

`plugins/scripting/include/loner.inc` makes the common things in an AMX Mod X plugin short:
a game mode that fits the Game Mode Menu, HUD text that doesn't fight other plugins for the
screen, giving weapons with ammo, health and armour, teleporting, timers and countdowns, and
orders for Loner-Strike's bots.

The Plugin Studio (Tools > Make) hands it to the compiler for you: `#include <loner>` just works
there, and the editor shows every helper's docs on hover. To compile elsewhere, put `loner.inc`
in `addons/amxmodx/scripting/include/` (or pass `-i<folder>` to amxxpc).

It needs AMX Mod X 1.9 or 1.10 and the cstrike, fun and fakemeta modules (all on by default in CS 1.6
and Condition Zero).

## A whole game mode

`plugins/scripting/examples/ls_example_mode.sma` ("Deagle Duel", ~50 lines) is the example.
The short version:

```pawn
#include <amxmodx>
#include <hamsandwich>
#include <loner>

public plugin_init()
{
	ls_register_mode("deagleduel", "Deagle Duel", "1.0", "me")
	RegisterHamPlayer(Ham_Spawn, "on_spawn", 1)
	register_logevent("on_round_start", 2, "1=Round_Start")
}

public on_round_start()
{
	ls_countdown(3, "Draw in", "on_draw")   // top of the screen: "Draw in 3", 2, 1
}

public on_draw()
{
	ls_announce(0, 2.0, "DRAW!")
}

public on_spawn(id)
{
	if (!ls_alive(id) || !ls_mode_active())
		return
	ls_strip(id)                     // knife only
	ls_give_weapon(id, "deagle", 35) // with 35 spare bullets
	ls_set_health(id, 150)
	ls_set_armor(id, 100, true)      // kevlar + helmet
	ls_hud(id, LS_HUD_LEFT, 5.0, "Deagle Duel^nLast one standing wins")
}
```

To make it a mode in the menu:

1. `configs/gamemodes/modes.ini`: add a line
   `deagleduel  "Deagle Duel"  "Deagles only, 150 HP"  ""  "w"`
2. `configs/gamemodes/deagleduel.plugins.ini`: `ls_example_mode.amxx`
3. `configs/gamemodes/deagleduel.cfg`: the mode's settings (cvars), run after `base.cfg`.
   If you change a cvar no other mode touches, reset it in `base.cfg` too.
4. Optional: `maps/deagleduel.txt` limits the maps, `mappresets.ini` picks the mode by map prefix.

The plugin only loads in that mode, so `ls_mode_active()` is a belt-and-braces check (true when
the plugin is loaded by hand without the Game Mode Menu).

## What's in it

| Part | Helpers |
|---|---|
| Modes | `ls_register_mode(id, name, version, author)`, `ls_mode_current(out, len)`, `ls_mode_active()` |
| HUD | `ls_hud(id, slot, secs, fmt, ...)`, `ls_hud_rgb(id, slot, r, g, b, secs, fmt, ...)`, `ls_hud_clear(id, slot)`, `ls_announce(id, secs, fmt, ...)`, `ls_chat(id, fmt, ...)` |
| Players | `ls_alive(id)`, `ls_give_weapon(id, weapon, ammo)`, `ls_strip(id, keep_knife)`, `ls_set_health(id, hp)`, `ls_set_armor(id, armor, helmet)`, `ls_teleport(id, origin, angles, turn)`, `ls_set_team(id, team)`, `ls_count_alive(team)` |
| Roles | `ls_set_role(id, role)`, `ls_role(id, out, len)`, `ls_is_role(id, role)`, `ls_count_role(role)` |
| Timers | `ls_after(secs, func, id)`, `ls_every(secs, func, id)`, `ls_stop(id)`, `ls_countdown(secs, text, done)`, `ls_countdown_stop()` |
| Bots | `ls_is_bot(id)`, `ls_bot_goto(bot, origin)`, `ls_bot_hold(bot, secs)`, `ls_bot_follow(bot, who, secs)`, `ls_bot_look(bot, origin, secs)`, `ls_bot_release(bot)` |
| Mode Kit (future) | `ls_kit_running()`, the `mk_*` natives |

### HUD slots

The game has four HUD text channels shared by every plugin. `ls_hud` gives each of its slots
(`LS_HUD_TOP`, `LS_HUD_LEFT`, `LS_HUD_RIGHT`, `LS_HUD_CENTER`) its own sync object, so AMX Mod X
keeps a slot on one channel per player and other plugins' messages don't wipe yours. Each slot
has a sensible place and colour; `ls_hud_rgb` picks your own colour. `ls_announce` uses the
director HUD (not one of the four channels) for short, rare shouts.

```pawn
ls_hud(0, LS_HUD_TOP, 1.1, "Zone closes in %d", secs)
ls_hud(id, LS_HUD_CENTER, 3.0, "You are the hunter!")
ls_chat(0, "%s found the stash", name)
```

### Timers

```pawn
ls_after(5.0, "explode", ent)     // explode(ent) in 5 seconds
ls_every(1.0, "tick")             // tick(0) every second, until ls_stop(0)
ls_countdown(10, "Round starts in", "start_round")
```

`ls_countdown` is one countdown at a time (a new one replaces the last); it beeps for the last
three seconds and calls your public function at zero.

### Bot orders

These send Loner-Strike's own bots (the ReGameDLL fork's `ls_bot` command) orders that work
today. An order lasts until it's done, a fight starts or `ls_bot_release`; the bot picks it back
up after the fight.

```pawn
new Float:stash[3] = {512.0, -128.0, 36.0}
for (new i = 1; i <= MaxClients; i++)
	if (ls_is_bot(i) && ls_is_role(i, "guard"))
		ls_bot_goto(i, stash)
```

## The Mode Kit parts

`docs/GAMEMODES-PLAN.md` (section 2.3) plans `gm_modekit.amxx`: roles, phases, zones, loot,
objectives, a HUD manager, events, creatures and the bots' mode goals, as natives from the
library `modekit`. loner.inc declares them now (`mk_role_set`, `mk_phase_set`, `mk_zone_set`,
`mk_loot_spawn`, `mk_give`, `mk_hud`, `mk_bot_goal`, …) so plugins written against them compile.

Until the kit exists:

- loner.inc's own `plugin_natives()` installs a module and native filter (`ls_optional()`), so a
  plugin using `mk_*` still loads without the kit;
- `ls_kit_running()` is false; check it before calling an `mk_*` native yourself;
- `ls_set_role` keeps roles itself and hands them to the kit when it runs.

If your plugin has its own `plugin_natives()`, define `LS_OWN_PLUGIN_NATIVES` before the include
and call `ls_optional()` from yours:

```pawn
#define LS_OWN_PLUGIN_NATIVES
#include <loner>

public plugin_natives()
{
	ls_optional()
	register_native("my_thing", "n_my_thing")
}
```

The `TODO(GAMEMODES-PLAN.md …)` comments in loner.inc mark what changes when the kit and the
brain's `ls_mode` commands land.
