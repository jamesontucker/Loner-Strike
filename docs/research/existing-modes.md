# Existing CS 1.6 game-mode plugins (research, 2026-11-12)

Scope: AMXX / Metamod / ReGameDLL mods for Battle Royale, TTT, Extraction, wave survival, Predator/one-vs-all, Jailbreak, and mode frameworks.
Method: quick web search (AlliedMods, GitHub, ModDB). **Last-update dates and licences were not checked one by one. Fields marked "?" need checking before anyone relies on them.** AMXX plugins posted on AlliedMods are GPL by convention, since they link against AMXX.

## Summary
- **Battle Royale / Extraction: no established CS 1.6 implementation found.** This is open ground. The nearest building blocks are zone plugins.
- **TTT:** one serious port (GuskiS), and it is worth studying.
- **Zombie / survival:** a very mature family (Zombie Plague and its forks). It is the reference for HUD, class, and shop UX on the stock client.
- **Jailbreak:** many server-specific forks and few clean repos.
- **Frameworks:** Multi-Mod Manager-style switchers (map/vote level) and ReDeathmatch "round modes" exist. No real composable round-modifier framework was found.

## Catalogue

| Mode | Name / link | Updated / licence | What it does | Bots |
|---|---|---|---|---|
| TTT | [GuskiS/Trouble-in-Terrorist-Town](https://github.com/GuskiS/Trouble-in-Terrorist-Town) | ? / GPL? | Port of the GMod TTT mod: Traitor/Detective/Innocent roles, karma, shops, DNA, body identification. README: "adjusted to possibilities and limitations of HL engine" | No (bots can't reason about roles) |
| Zombie | Zombie Plague 4.3 / 5.0 ([AM thread](https://forums.alliedmods.net/showthread.php?p=1087655) ZPA, [ModDB](https://www.moddb.com/mods/zombie-plague)) | 2008–2010s / GPL | Infection, plus nemesis/survivor/swarm/plague "round modes". Extra-items API, classes | Partial (CZ/PODBot bots run, ignore mode logic) |
| Zombie | [z0h1r-LK/Zombie_Escape](https://github.com/z0h1r-LK/Zombie_Escape) | recent? / ? | ZE built on the ZP5 core, needs ReAPI and AMXX 1.10 | ? |
| Zombie | [csbtedhan/CS1.6-Zombie-Z-AMXX-Re-Api](https://github.com/csbtedhan/CS1.6-Zombie-Z-AMXX-Re-Api) | ? / ? | CSO-style Zombie Z on ReAPI | ? |
| Predator / one-vs-all | [AVP mod](https://forums.alliedmods.net/showthread.php?p=962874) | ~2009 / GPL | Aliens vs Predator survival with special classes | ? |
| Predator | "Predator Mod_b2" (listed in [Multi-Mod Manager](https://forums.alliedmods.net/showthread.php?p=2351893)) | old / ? | Cloaked hunter versus team | ? |
| Prop hunt | [CS Prop Hunt](https://forums.alliedmods.net/archive/index.php/t-292415.html) | ~2017 / GPL | Hide-and-seek as props | No |
| Jailbreak | [xUnkNx/jailbreak](https://github.com/xUnkNx/jailbreak), AM jail threads (Simon, days, shop) | ? / ? | CT guards and T prisoners, Simon commands, special days, LR duels | No |
| Framework | [Multi-Mod Manager](https://forums.alliedmods.net/showthread.php?p=2351893), [zz's Multi-Mod](https://forums.alliedmods.net/showthread.php?p=2352014) | ~2015 / GPL | Vote between mods; swaps plugins.ini and changes map | n/a |
| Framework | [ReDeathmatch](https://redeathmatch.github.io) | active / ? | ReGameDLL DM with "round modes", cvar presets, hot reload | yes (DM) |
| Framework | [d3m37r4/regg](https://github.com/d3m37r4/regg) | ? / ? | ReAPI GunGame with a mode controller (single/team/FFA) and a vote | ? |
| Zone building block | [Barrier Zone](https://www.moddb.com/addons/barrier-zone) | ? | Dynamic barrier zones (no shrinking) | n/a |

## How they work around the stock client
- **Roles / private info:** colour-coded `client_print` / say text, a team-coloured HUD (TTT uses a role HUD and a scoreboard attribute hack). Roles are never shown in the scoreboard; TTT hides team by putting everyone on one team.
- **Menus / voting:** `show_menu` / newmenus (keys 1-9). TTT body "identify" and shops use these. No real meeting UI exists anywhere; the menu plus chat is the standard.
- **Inventories / loot:** ZP-style "extra items" bought from a menu. Weapons on the ground (`weaponbox`) serve as loot. There are no grid inventories; the HUD text (`set_dhudmessage`) lists items.
- **Zones (what BR would need):** nothing ships. Possible options: a beam ring of `TE_BEAMPOINTS` / `TE_BEAMCYLINDER` segments, sprite models, fog / `ScreenFade`, a `set_hudmessage` timer, and a damage tick. Beam entity count and the 512-entity / TE message rate limits are the real constraint.
- **Radar / map:** the stock radar can't draw zones. Mods use HUD distance and arrow text instead.

## Bots
Almost none of these mods are bot-aware. Stock CZ/ZBot or PODBot just play deathmatch logic and ignore roles, zones, and objectives. Our bot-brain fork is a real differentiator; every mode we ship should declare its bot hooks (zone centre, loot targets, role knowledge).

## Worth learning
- ZP's **round-mode + extra-item natives/forwards API**. It is the de facto pattern for composable modes, with forks reusing it for a decade.
- TTT's role-secrecy tricks and its karma system, for social deduction on the HL client.
- ReDeathmatch's **cvar-preset / hot-reload** config model and its ReAPI-first design.
- Multi-Mod's lesson: switching modes by map change and plugins.ini is clunky. Switching per round inside one plugin set is better.

## Worth avoiding
- Giant monolithic .sma files (ZP 4.3, most jailbreak forks), hardcoded strings, and server-specific forks without licences.
- Heavy custom models/sounds that force big downloads (do FastDL or keep stock assets).
- HUD channel collisions: the client has only 4 hudmessage channels, so plan to use one HUD manager.
- Assuming bots will "just work" in a new mode.

## Gaps / follow-ups
- Verify dates and licences (GitHub API, AM thread dates).
- Search dev-cs.ru directly (Russian-language: "королевская битва", "режимы раунда") because many ReAPI mode frameworks live there.
- Look for "Last Man Standing" and "Hide'n'Seek" frameworks as BR stand-ins.
