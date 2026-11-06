# Loner-Strike

**Re-live the golden days of Counter-Strike locally or with the friends you don't have.**

Loner-Strike turns Counter-Strike: Condition Zero or Counter-Strike 1.6 into a public server from 2004 that only you (and maybe a friend or two) play on. You get the classic community game modes, bots that play like people, a crowd of regulars with clan tags, and a web control panel to run it all from your browser or phone.

> Early days: this repo is being assembled from a working setup. The one-step installer isn't here yet (see [the roadmap](docs/ROADMAP.md)).

## What you get

- **20 game modes**, switchable from an in-game menu or the panel:
  - classics: Classic, Deathmatch, Free for all
  - weapon modes: Gun Game (solo and teams), Knife, Pistols, Deagle, AWP, Scoutz & Knivez
  - zombies: Zombie Plague and Zombie Plague Advanced
  - community mods: Warcraft 3, SuperHero, Hide and Seek, Paintball, Deathrun, Base Builder, SoccerJam, Surf, Kreedz
- **Bots that play like people.** Choose a bot style:
  - Newbie lobby, Pub server, Matchmaking or Veterans, with human reaction times, flicks that overshoot, wobbly tracking and burst fire
  - Locked In, Hackers and Gods, when you want to suffer
  - Classic, which leaves CZ's bots unchanged
- **A server that feels alive:**
  - over 1,600 names for bot regulars, with clan tags; the same regulars keep coming back
  - players come and go during a map
  - rock the vote and nominate
  - a welcome screen when you join
- **Add-ons in any mode:** grappling hook, Quake sounds, gore, Roll the Dice, round-end music.
- **A control panel in your browser:** modes, maps (with pictures), bots, match settings, players, add-ons, and a map browser that installs maps from GameBanana.
- **Play with friends without touching your router**, over [playit.gg](https://playit.gg) or [Tailscale](https://tailscale.com). The panel writes the invite message for you.

## What's custom here

The community mods are classics. These parts are new, written for Loner-Strike:

- **Bot styles.** Condition Zero's bots react instantly and aim like turrets, so Loner-Strike gives them human habits:
  - reaction times drawn from a human-like distribution
  - flicks that overshoot and settle
  - aim that drifts while tracking
  - fire in bursts with spray that climbs
  - the odd mistake
  - on the top styles, bunnyhopping between fights

  It works by bending the bots' bullets and timing their trigger, the only parts of CZ's bots a plugin can reliably steer.
- **Bot patches for every mod.** Many classic plugins never notice CZ's bots: their hits, spawns and kills slip past (bots use a separate code path the plugins don't hook). Each mod here is patched for that, and where a mode had no bot logic it got some:
  - SuperHero bots pick heroes and use their powers
  - Deathrun bots spring traps
  - SoccerJam bots chase the ball
  - Base Builder zombie bots pick classes
  - Roll the Dice bots roll each round
  - Warcraft 3 bots keep their levels between sessions
  - Paintball hits on bots register
  - Hide and Seek bots stop walking to a hidden hostage
- **Regulars.** Bots get names from a list of 1,600, with clan tags. Each keeps its name for good, so the same people keep turning up, along with the odd newcomer. Now and then someone leaves and someone new joins.
- **A Game Mode Menu** (F3 in game) and a **web control panel** that switch between 20 modes and change any setting without editing a config file.
- **Play with friends** over playit.gg or Tailscale, with the invite message written for you.
- **Fixes along the way:**
  - Gun Game and Deathmatch spread spawns over the bot navigation map when a map has no spawn file
  - the map timer waits while bots are learning a new map
  - Deathmatch no longer runs the server out of entity slots on long sessions

## Requirements

- Counter-Strike: Condition Zero and/or Counter-Strike 1.6 on Steam.
- Windows, or a Mac running the game through CrossOver.
- Python 3 for the control panel. It uses only the standard library.

## How it works

The game runs as a "listen server": you host and play in the same window. Loner-Strike adds [Metamod-P](https://github.com/theAsmodai/metamod-p), [AMX Mod X](https://www.amxmodx.org) and [ReGameDLL_CS](https://github.com/s1lentq/ReGameDLL_CS), then the plugins in this repo and the community mods listed in [catalog/components.json](catalog/components.json). The panel talks to the game over RCON on your own computer.

## Layout

```
panel/      the control panel (Python helper + one-page web app)
plugins/    our AMX Mod X plugins (Pawn sources) and the mode and add-on configs
catalog/    every installable piece: where it comes from and how it's installed
docs/       roadmap and notes
```

## Credits

Loner-Strike stands on two decades of community work. See [CREDITS.md](CREDITS.md).

## Not affiliated with Valve

Counter-Strike and Condition Zero are trademarks of Valve Corporation. Loner-Strike is an unofficial fan project and contains no game files. You need your own copy of the game on Steam.

## License

GPL-3.0. See [LICENSE](LICENSE). The third-party mods keep their own licenses and authors (see CREDITS.md).
