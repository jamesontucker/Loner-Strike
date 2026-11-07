# Loner-Strike

[![Latest release](https://img.shields.io/github/v/release/jamesontucker/Loner-Strike?label=release)](https://github.com/jamesontucker/Loner-Strike/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jamesontucker/Loner-Strike/total)](https://github.com/jamesontucker/Loner-Strike/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Mac%20(CrossOver)-informational)](#requirements)
[![License: GPL-3.0](https://img.shields.io/github/license/jamesontucker/Loner-Strike)](LICENSE)
[![Discord](https://img.shields.io/discord/1568421500636233748?label=discord&logo=discord&logoColor=white)](https://discord.gg/K9eBHVVwZE)

**Play Counter-Strike like it's 2004, alone or with the friends you don't have.**

Counter-Strike 1.6 and Condition Zero set up like an old community server, except it's only you, your bots and maybe a friend.

Loner-Strike is a free desktop app for Windows, and for Mac through CrossOver, that installs the mods, runs the game for you and gives you a control panel for all of it.

[lonerstrike.com](https://lonerstrike.com)

## What's in it

**Playing**
- 22 game modes: Classic, Match (5v5), Deathmatch, Free for all, Gun Game (solo and teams), Zombie Plague (two versions), Warcraft 3, SuperHero, Hide and Seek, Paintball, Deathrun, Base Builder, SoccerJam, Surf, Kreedz, plus knife, pistol, deagle, AWP and scout modes
- Bot styles with human reaction times and aim, from a newbie lobby up to bunnyhopping pros
- 1,600 bot names with clan tags, so the same "regulars" keep showing up
- Thousands of maps from GameBanana with pictures, favourites and search, plus skins, models, sounds and sprays
- Round-end music, Quake sounds and classic sound packs
- Add your own maps, sounds and AMX Mod X plugins from a file
- The Game Mode Menu in the game (F3) and quick actions in the app while you play

**Friends**
- Seven ways to connect: the same network, Direct (your router), [playit.gg](https://playit.gg), [Tailscale](https://tailscale.com), ZeroTier, Radmin VPN and Hamachi
- Server name, join password, an invite to send and a custom welcome page

**Servers**
- Dedicated servers on your computer, or on a rented Linux machine (a VPS) that the app sets up over SSH and that keeps running without it
- Bring a server you already run, or copy one server's mods to another
- Admins, moderators and VIPs with abilities per role, bans, mutes, a swear filter, a chat log and player reports
- Share the admin list, bans, swear filter, welcome page and chat messages across servers; mods and settings stay per server
- Map rotation, timed chat messages, AFK and high-ping kickers, stats and ranks, votekick and voteban
- Match mode for 5v5s, HLTV spectators and demos, fast downloads and scheduled restarts
- Join password, console password and extra start options per server
- A step-by-step guide in the app to renting a server: where, what size and how to set it up

**The app**
- Guided setup that finds your game
- Control it from your phone by scanning a QR code (anything that deletes or reinstalls stays on the computer)
- Export your setup to another computer or server
- Signed automatic updates

## What we made

The mods are community classics. We added:

- **Bot styles.** Stock CZ bots react instantly and aim like turrets. The styles give them reaction delays, overshooting flicks, shaky tracking, burst fire and the occasional whiff. The top styles bunnyhop.
- **Bot fixes in every mod.** A lot of old plugins don't see CZ bots at all, so hits, kills and spawns get missed. Every mod here is patched for that. Some also got bot logic they never had: SuperHero bots pick heroes, Deathrun bots trigger traps, SoccerJam bots chase the ball and Roll the Dice bots roll.
- **The Game Mode Menu, the server plugins and the app**, so you never have to edit a config file.
- **Fixes** for Gun Game and Deathmatch spawns on maps without spawn files, the map timer while bots learn a map, and a Deathmatch crash on long sessions.

## Requirements

- Counter-Strike 1.6, Condition Zero or both, bought on Steam (other copies aren't supported)
- Windows 10 or 11, or a Mac with [CrossOver](https://www.codeweavers.com/crossover) (Apple Silicon or Intel)
- About 1 GB free for the mods, plus room for any maps you add
- For a server on a VPS: an Ubuntu or Debian machine on x86 (AMD or Intel) with SSH access

## Install

- **Windows:** download Loner-Strike from [Releases](https://github.com/jamesontucker/Loner-Strike/releases) and open it. Setup finds the game and walks you through the rest. Windows may show "Windows protected your PC" (SmartScreen) because the app is new and not yet widely downloaded: click More info, then Run anyway.
- **Mac:** the Mac version of Counter-Strike can't run mods, so play it through [CrossOver](https://www.codeweavers.com/crossover) (paid, with a free trial): install Windows Steam in a CrossOver bottle and Counter-Strike in that. Then download Loner-Strike from Releases; it finds the game inside CrossOver. With Parallels, run the Windows version of Loner-Strike inside the Windows virtual machine instead.

Everything it changes is backed up. Remove one mode, or all of Loner-Strike, from the app and your game goes back the way it was.

Updates come through the app (Settings, Updates). What changed in each version is on the [Releases](https://github.com/jamesontucker/Loner-Strike/releases) page and in [CHANGELOG.md](CHANGELOG.md).

## Troubleshooting

- **The game won't start or the panel says no game is loaded.** Start the game from Play in the app, not from Steam, so it loads with the mods.
- **Friends can't see your game on the LAN tab.** Allow the game through the firewall when asked. On Windows, also make sure your Wi-Fi is set to Private network (Settings, Network & internet).
- **A mod misbehaves after a game update.** Open Settings and click Run full setup; it reinstalls what's missing and keeps your settings.
- **You want everything gone.** Settings has Remove Loner-Strike, which puts your game back the way it was.

Still stuck? Ask in the [Discord](https://discord.gg/K9eBHVVwZE), or open Settings, click Copy next to Copy diagnostics (version, game folders, what's installed and recent errors; no passwords) and paste it into a [new issue](https://github.com/jamesontucker/Loner-Strike/issues/new).

## Run a server on a VPS

The app can set up an always-on server on a Linux machine you rent (Ubuntu or Debian, x86). In Servers, choose New server, pick "A Linux server (VPS)" and give it the machine's address, user and SSH key. It checks the machine's identity, installs Valve's server and Loner-Strike over SSH and runs it as a service on UDP 27015, so it keeps going with the app closed. Oracle's free tier works, as does any VPS at around $4 to $7 a month (Hetzner, DigitalOcean, Vultr).

After that it's managed like any other server:
- Changes you make in the app (settings, admins, mods, maps, sounds) go up by themselves, and only the files that changed. New plugins and maps load on the next map, or with Restart now.
- Update gets Valve's latest server files on the machine. Bans and stats made on the server stay there.
- HLTV runs next to it on UDP 27020, and its demos can be copied to your game.
- If the machine stops answering, the app says so instead of showing it as running.

[docs/linux-server.md](docs/linux-server.md) walks through it: picking a host and size, making an SSH key, opening UDP 27015-27020 in the provider's firewall, managing the server and fixing common problems.

## How it works

Setup installs [Metamod-P](https://github.com/Bots-United/metamod-p), [AMX Mod X](https://www.amxmodx.org) and [ReGameDLL](https://github.com/rehlds/ReGameDLL_CS), then our plugins and the mods you pick from [catalog/](catalog). Mods are downloaded from their original sources and checked against pinned checksums. The app talks to the game over RCON on your computer; nothing goes through a server of ours.

```
app/src-tauri/   the app: setup, the game and server control API, sharing (Rust, Tauri)
app/ui/          the control panel (React), shown in the app window and on your phone
plugins/         our AMX Mod X plugins and the mode settings
mods/            the community mods' code with our bot fixes
catalog/         what each mod installs and where it comes from
site/            lonerstrike.com
```

## Building it

You need [Rust](https://rustup.rs), Node.js 22 and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```
cd app/ui
npm install
npm run tauri dev       # runs the app with the UI built in
```

`cargo test` in `app/src-tauri` runs the tests. Releases are built by GitHub Actions when a `v*` tag is pushed (see `.github/workflows/release.yml`).

## Contributing

Contributions are welcome. It's a side project, so I work on it on and off and replies can take a while. For anything bigger than a fix, open an issue first so we can talk it through.

## Credits

Made by [nanoshrine](https://x.com/nanoshrine). See [CREDITS.md](CREDITS.md) for every mod and its authors.

Not affiliated with Valve. Counter-Strike and Condition Zero are Valve's trademarks. No game files are included; you need your own copy.

## License

GPL-3.0. Third-party mods keep their own licenses (see CREDITS.md).
