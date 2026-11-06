# Loner-Strike

**Re-live the golden days of Counter-Strike locally or with the friends you don't have.**

Counter-Strike 1.6 / Condition Zero set up like an old community server, except it's just you, your bots, and maybe a friend.

## What's in it

- 20 game modes: Classic, Deathmatch, Free for all, Gun Game, Zombie Plague (two versions), Warcraft 3, SuperHero, Hide and Seek, Paintball, Deathrun, Base Builder, SoccerJam, Surf, Kreedz, plus knife/pistol/deagle/AWP/scout modes
- Add-ons: grappling hook, Quake sounds, gore, Roll the Dice, round-end music
- Bot styles, from "newbie lobby" up to "Gods"
- Bots with names and clan tags that come and go like real players
- Rock the vote, a welcome screen, and an in-game menu on F3
- A control panel in your browser (or phone) for modes, maps, bots and settings, with a map browser for GameBanana
- Play with friends over [playit.gg](https://playit.gg) or [Tailscale](https://tailscale.com), no port forwarding

## What we made

The mods are community classics. We added:

- **Bot styles.** Stock CZ bots react instantly and aim like turrets. These give them reaction delays, overshooting flicks, shaky tracking, burst fire and the occasional whiff. The top styles bunnyhop.
- **Bot fixes in every mod.** A lot of old plugins don't see CZ bots at all, so hits, kills and spawns get missed. Every mod here is patched for that. Some also got bot logic they never had: SuperHero bots pick heroes, Deathrun bots trigger traps, SoccerJam bots chase the ball, and Roll the Dice bots roll.
- **Regulars.** 1,600 bot names with clan tags. The same bots keep showing up, so it feels like a server with regulars.
- **The menu and control panel**, so you never have to edit a config file.
- **Fixes** for Gun Game/Deathmatch spawns on maps without spawn files, the map timer during bot map-learning, and a Deathmatch crash on long sessions.

## Install

You need Counter-Strike 1.6 and/or Condition Zero on Steam.

- **Windows:** download Loner-Strike from [Releases](https://github.com/jamesontucker/Loner-Strike/releases) and open it. Setup finds the game and walks you through the rest.
- **Mac:** the Mac version of Counter-Strike can't run mods, so play it through [CrossOver](https://www.codeweavers.com/crossover) (paid, with a free trial): install Windows Steam in a CrossOver bottle and Counter-Strike in that. Then download Loner-Strike from Releases; it finds the game inside CrossOver.

Everything it changes is backed up. Remove one mode, or all of Loner-Strike, from the app's Mods page and your game goes back the way it was.

Prefer a script? `python3 setup.py` installs the same things from the terminal (Python 3 needed), and `python3 setup.py --uninstall` takes them out.

## Servers

The app can also run dedicated servers: it downloads Valve's free server, sets it up like your game (or another server) and runs it, each with its own modes, maps, admins, VIPs, bans and welcome page. You can bring in a server you already run too. The Servers tab has a guide to free and cheap places to host one and how big it needs to be.

## How it works

It installs [Metamod-P](https://github.com/Bots-United/metamod-p), [AMX Mod X](https://www.amxmodx.org) and [ReGameDLL](https://github.com/rehlds/ReGameDLL_CS), then our plugins and the mods in [catalog/](catalog). Mods are downloaded from their original sources during install. The panel talks to the game over RCON on your computer.

```
setup.py    the installer
panel/      the control panel
plugins/    our plugins and mode settings
mods/       the community mods' code with our bot fixes
catalog/    what each mod installs and where it comes from
```

## Credits

See [CREDITS.md](CREDITS.md) for every mod and its authors.

Not affiliated with Valve. Counter-Strike and Condition Zero are Valve's trademarks. No game files are included; you need your own copy.

## License

GPL-3.0. Third-party mods keep their own licenses (see CREDITS.md).
