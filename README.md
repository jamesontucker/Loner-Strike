# Loner-Strike

**Play Counter-Strike like it's 2004, alone or with the friends you don't have.**

Counter-Strike 1.6 and Condition Zero set up like an old community server, except it's just you, your bots, and maybe a friend.

Loner-Strike is a desktop app for Windows and Mac that installs the mods, runs the game for you and gives you a control panel for all of it.

[lonerstrike.com](https://lonerstrike.com)

## What's in it

**Playing**
- 22 game modes: Classic, Match (5v5), Deathmatch, Free for all, Gun Game (solo and teams), Zombie Plague (two versions), Warcraft 3, SuperHero, Hide and Seek, Paintball, Deathrun, Base Builder, SoccerJam, Surf, Kreedz, plus knife, pistol, deagle, AWP and scout modes
- Bot styles with human reaction times and aim, from a newbie lobby up to bunnyhopping pros
- 1,600 bot names with clan tags, so the same "regulars" keep showing up
- Thousands of maps from GameBanana with pictures, favourites and search, plus skins, models, sounds and sprays
- Round-end music, Quake sounds and classic sound packs
- Add your own maps, sounds and AMX Mod X plugins from a file
- An in-game menu on F3 and quick actions in the app while you play

**Friends**
- Seven ways to connect: LAN, your router, [playit.gg](https://playit.gg), [Tailscale](https://tailscale.com), ZeroTier, Radmin VPN and Hamachi
- Server name, password, an invite to send and a custom welcome page

**Servers**
- Dedicated servers downloaded and set up for you, or bring one you already run
- Admins, moderators and VIPs with abilities per role, bans, gags, a swear filter, a chat log and player reports
- Map rotation, timed chat messages, AFK and high-ping kickers, stats and ranks, votekick and voteban
- Match mode for 5v5s, HLTV, fast downloads and scheduled restarts
- A guide in the app to free and cheap places to host and how big a server needs to be

**The app**
- Guided setup that finds your game
- Control it from your phone by scanning a QR code
- Export your setup to another computer or server
- Signed automatic updates

## What we made

The mods are community classics. We added:

- **Bot styles.** Stock CZ bots react instantly and aim like turrets. The styles give them reaction delays, overshooting flicks, shaky tracking, burst fire and the occasional whiff. The top styles bunnyhop.
- **Bot fixes in every mod.** A lot of old plugins don't see CZ bots at all, so hits, kills and spawns get missed. Every mod here is patched for that. Some also got bot logic they never had: SuperHero bots pick heroes, Deathrun bots trigger traps, SoccerJam bots chase the ball and Roll the Dice bots roll.
- **The Game Mode Menu, the server plugins and the app**, so you never have to edit a config file.
- **Fixes** for Gun Game and Deathmatch spawns on maps without spawn files, the map timer while bots learn a map, and a Deathmatch crash on long sessions.

## Install

You need Counter-Strike 1.6 and/or Condition Zero on Steam.

- **Windows:** download Loner-Strike from [Releases](https://github.com/jamesontucker/Loner-Strike/releases) and open it. Setup finds the game and walks you through the rest.
- **Mac:** the Mac version of Counter-Strike can't run mods, so play it through [CrossOver](https://www.codeweavers.com/crossover) (paid, with a free trial): install Windows Steam in a CrossOver bottle and Counter-Strike in that. Then download Loner-Strike from Releases; it finds the game inside CrossOver. With Parallels, run the Windows version of Loner-Strike inside the Windows virtual machine instead.

Everything it changes is backed up. Remove one mode, or all of Loner-Strike, from the app and your game goes back the way it was.

Only the Steam versions of the games are supported.

## Run a server on a VPS

The app can set up an always-on server on a Linux machine you rent (Ubuntu or Debian, x86). In Servers > New server, pick "A Linux server (VPS)" and give it the machine's address, user and SSH key. It installs Valve's server and Loner-Strike over SSH and runs it as a service on UDP 27015, so it keeps going with the app closed. Oracle's free tier works, as does any VPS at around $4 to $7 a month.

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

You need [Rust](https://rustup.rs), Node.js 20 and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```
cd app/ui
npm install
npm run tauri dev       # runs the app with the UI built in
```

`cargo test` in `app/src-tauri` runs the tests. Releases are built by GitHub Actions when a `v*` tag is pushed (see `.github/workflows/release.yml`).

## Credits

Made by [nanoshrine](https://x.com/nanoshrine). See [CREDITS.md](CREDITS.md) for every mod and its authors.

Not affiliated with Valve. Counter-Strike and Condition Zero are Valve's trademarks. No game files are included; you need your own copy.

## License

GPL-3.0. Third-party mods keep their own licenses (see CREDITS.md).
