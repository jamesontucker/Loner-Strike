# Loner-Strike

[![Latest release](https://img.shields.io/github/v/release/jamesontucker/Loner-Strike?label=release)](https://github.com/jamesontucker/Loner-Strike/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jamesontucker/Loner-Strike/total)](https://github.com/jamesontucker/Loner-Strike/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Mac%20(CrossOver)-informational)](#install)
[![License: GPL-3.0](https://img.shields.io/github/license/jamesontucker/Loner-Strike)](LICENSE)
[![Discord](https://img.shields.io/discord/1568421500636233748?label=discord&logo=discord&logoColor=white)](https://discord.gg/K9eBHVVwZE)

**Play Counter-Strike like it's 2004, alone or with the friends you don't have.**

A free desktop app that sets up Counter-Strike 1.6 and Condition Zero like an old community server, with bots that play like people.

> A side project I work on now and then. Bugs and ideas are welcome in [Issues](https://github.com/jamesontucker/Loner-Strike/issues).

## What's in it

**Play**
- Bots that play like people: our own bot brain studies every map once (sightlines, cover, wallbangs), sees only what you could, and plays as a team
- Talk to them in team chat or, on a dedicated server, on your mic ("rush b", "money?", "flash for me")
- 24 modes: 22 community classics patched for bots, plus Last Stand and Battle Royale, made here ([all modes](https://lonerstrike.com/docs#modes))
- 27 fun maps installed and thousands more from GameBanana, checked before they're installed

**Make** (Beta)
- Map editor: brushes and entities in 3D, live checks, Hammer, J.A.C.K. and TrenchBroom import, F5 to playtest with bots
- Game Mode Studio: modes from typed node graphs, playtested with bots and debugged live in the game
- Creature Studio for creatures and waves, and Plugin Studio for AMX Mod X plugins

**Host**
- Dedicated servers on your computer, or on a rented Linux machine (a VPS) the app sets up over SSH
- Servers update, check and back themselves up; friends can help run them as Admins or Moderators
- The Loner-Strike server list, a server browser, and a live status in your Discord

**Share**
- Community: browse and install maps, modes and creatures made with Loner-Strike, hosted on GameBanana under the LonerStrike tag
- Publish with credits filled in, the files a mode or map uses, and its editable source for others to remix

## Install

You need Counter-Strike 1.6 or Condition Zero on Steam.

- **Windows 10 or 11:** download it from [Releases](https://github.com/jamesontucker/Loner-Strike/releases) and open it. If SmartScreen warns, click More info, then Run anyway.
- **Mac:** play through [CrossOver](https://www.codeweavers.com/crossover): install Windows Steam and the game in a bottle, then the Mac download of Loner-Strike finds it.

Everything it changes is backed up, and removing it puts your game back. Updates come through the app. Setup, servers and the usual problems are in the [docs](https://lonerstrike.com/docs).

## Links

[lonerstrike.com](https://lonerstrike.com) · [Docs](https://lonerstrike.com/docs) · [Reference](https://lonerstrike.com/reference/) · [Discord](https://discord.gg/K9eBHVVwZE) · [Issues](https://github.com/jamesontucker/Loner-Strike/issues) · [Changelog](CHANGELOG.md)

## Building it

You need [Rust](https://rustup.rs), Node.js 22 and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```
cd app/ui
npm install
npm run tauri dev
```

`cargo test` in `app/src-tauri` runs the tests. Releases are built by GitHub Actions when a `v*` tag is pushed. The app is in `app/` (Rust and React), our plugins in `plugins/`, the mods with our bot fixes in `mods/`, what each mod installs in `catalog/`, and the website in `site/`.

Setup installs [Metamod-P](https://github.com/Bots-United/metamod-p), [AMX Mod X](https://www.amxmodx.org), and our builds of [ReGameDLL](https://github.com/jamesontucker/ReGameDLL_CS) (the bot brain) and [ReHLDS](https://github.com/jamesontucker/ReHLDS) (for servers). Mods come from their original sources, checked against pinned checksums. Voice commands use [whisper.cpp](https://github.com/ggerganov/whisper.cpp) and [llama.cpp](https://github.com/ggerganov/llama.cpp), and run on the machine running the server.

Contributions are welcome; for anything bigger than a fix, open an issue first.

## Credits and license

Made by [nanoshrine](https://nanoshrine.com). It stands on twenty years of community work: [GameBanana](https://gamebanana.com) and its creators, [AlliedModders](https://www.alliedmods.net) (AMX Mod X and most of the classic mods), [the ReHLDS team](https://github.com/rehlds) (ReGameDLL and ReHLDS), and every mod and map author, named in [CREDITS.md](CREDITS.md).

GPL-3.0. Third-party mods keep their own licenses (see CREDITS.md). Not affiliated with Valve. Counter-Strike and Condition Zero are Valve's trademarks. No game files are included; you need your own copy.
