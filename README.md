# Loner-Strike

[![Latest release](https://img.shields.io/github/v/release/jamesontucker/Loner-Strike?label=release)](https://github.com/jamesontucker/Loner-Strike/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jamesontucker/Loner-Strike/total)](https://github.com/jamesontucker/Loner-Strike/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Mac%20(CrossOver)-informational)](#requirements)
[![License: GPL-3.0](https://img.shields.io/github/license/jamesontucker/Loner-Strike)](LICENSE)
[![Discord](https://img.shields.io/discord/1568421500636233748?label=discord&logo=discord&logoColor=white)](https://discord.gg/K9eBHVVwZE)

**Play Counter-Strike like it's 2004, alone or with the friends you don't have.**

> It's a side project I work on now and then. The [docs](https://lonerstrike.com/docs) cover setup and the usual problems; bugs and ideas are welcome in [Issues](https://github.com/jamesontucker/Loner-Strike/issues).

Counter-Strike 1.6 and Condition Zero set up like an old community server, except it's only you, your bots and maybe a friend.

Loner-Strike is a free desktop app for Windows, and for Mac through CrossOver, that installs the mods, runs the game for you and gives you a control panel for all of it.

[lonerstrike.com](https://lonerstrike.com)

## What's in it

**Playing**
- 22 game modes: Classic, Match (5v5), Deathmatch, Free for all, Gun Game (solo and teams), Zombie Plague (two versions), Warcraft 3, SuperHero, Hide and Seek, Paintball, Deathrun, Base Builder, SoccerJam, Surf, Kreedz, plus knife, pistol, deagle, AWP and scout modes
- Bots that play like people, from a newbie lobby up to bunnyhopping pros: they study every map, see only what you could, make their own calls, aim like a hand on a mouse and play as a team; each one has its own playstyle, skill and about 70 habits
- Talk to them in team chat: ask "money?" or "where'd they go" and they answer from what's really happening; call "rush b", "fake a", "flash for me", "two long" or "someone buy me an awp" and they do it, or say no in character
- Or say it on your mic: on a dedicated server, the optional voice commands add-on turns what you say into the same orders, on the machine running the server (no audio is kept or sent anywhere)
- 1,600 regulars, a few of them in small clans with a tag, who keep showing up and remember you: friends, rivals and grudges, kept by Steam ID on a server
- 27 fun and popular maps installed for you (scoutzknivez, aim_map, fy_iceworld, awp_india, he_glass, de_mirage, the Rats pack and more), and the stock and included maps come ready for bots
- Thousands more maps from GameBanana with pictures, favourites and search, each checked for missing files before it's installed, plus weapon skins, player models, sprays and sounds
- Round-end music, Quake sounds and classic sound packs
- Add your own maps, sounds and AMX Mod X plugins from a file
- Set up bots, rules and fun before you start a game; changes apply when it starts
- The Game Mode Menu in the game (F3) and quick actions in the app while you play

**Friends**
- Seven ways to connect: the same network, Direct (your router), [playit.gg](https://playit.gg), [Tailscale](https://tailscale.com), ZeroTier, Radmin VPN and Hamachi
- Server name, join password, an invite to send and a custom welcome page
- Find servers: a server browser like Steam's old one, with games on your own network

**Servers**
- Dedicated servers on your computer, or on a rented Linux machine (a VPS) that the app sets up over SSH and that keeps running without it
- Friends help run it from their own Loner-Strike as an Admin or Moderator, even with your computer off, and you see what each of them did
- Servers update themselves once nobody's playing; on a Linux machine the game runs in a sandbox, RCON is closed to the internet, and the server's Health tab checks the machine and fixes what it can
- Linux servers backed up to your computer every day, and a message in your Discord if one goes down
- Bring a server you already run, or copy one server's mods to another
- Admins, moderators and VIPs with abilities per role; Moderation with player reports, a chat log, mutes, a swear filter, bans and player history
- Share the admin list, bans, swear filter, welcome page and chat messages across servers; mods and settings stay per server
- Map rotation per mode (modes like Deathrun or Zombie Plague go through their own maps by themselves), mode votes or a mode rotation, timed chat messages, AFK and high-ping kickers, votekick and voteban, and Stats with ranks and 5v5 results
- Match mode for 5v5s, HLTV spectators and demos, fast downloads and scheduled restarts
- Join password, console password, tick rate and extra start options per server
- A live status in your Discord, and the Loner-Strike server list for anyone to find your server
- Weapon skins and player models everyone on the server sees
- A step-by-step guide in the app to renting a server: where, what size and how to set it up

**The app**
- Guided setup that finds your game
- One sidebar for your own game, grouped like a server's tabs (Game, People, Online, My library, Hosting), and a bar on every page that says whether you're changing your game or a server
- Control it from your phone by scanning a QR code (anything that deletes or reinstalls stays on the computer)
- Back up everything to one file and restore it, on this computer or another
- Help in Settings: the docs, diagnostics to copy, and the usual problems with a button to the page that fixes each
- Signed automatic updates

## What we made

The mods are community classics. We added:

- **A bot brain.** Our own build of [ReGameDLL](https://github.com/jamesontucker/ReGameDLL_CS) (branch `loner-strike`) with a bot brain built from the ground up:
  - **Map study:** each map is studied once in the background (millions of sightlines, cover, corners, the angles enemies come from, and wallbangs by the game's real penetration rules), and bots use as much of it as their skill would know.
  - **Fair vision:** no wallhack knowledge. They notice a barrel or a foot past cover, read flashes and sounds, and remember where they last saw you, then decide whether to hold, re-peek, flank or fall back.
  - **Human aim and movement:** still most of the time, flicks that land a touch off, a little tremor; their own routes, Ts taking sites together, CTs holding and retaking.
  - **People:** 11 playstyles, about 70 personal habits each, moods, and regulars who remember the players they meet.
  - **Chat:** they answer questions and take orders from what's really going on.

  Stock CZ bots react instantly and aim like turrets; these were tuned against recordings of a person playing. The ideas come from games known for their AI: Killzone, Quake III, Unreal Tournament 2004's BotPrize and F.E.A.R.
- **Voice commands.** A small program beside the server turns what players say on voice chat into text with [Whisper](https://github.com/ggerganov/whisper.cpp), and a small model of our own (a fine-tuned Gemma 3 270M, run with [llama.cpp](https://github.com/ggerganov/llama.cpp)) turns loose wording into the order it means. It runs on the machine running the server, and needs our build of [ReHLDS](https://github.com/jamesontucker/ReHLDS) for the voice feed.
- **Bot styles.** On top of the brain, lobbies from newbies to bunnyhopping pros, or a mixed lobby where each bot keeps its own level.
- **Bot fixes in every mod.** A lot of old plugins don't see CZ bots at all, so hits, kills and spawns get missed. Every mod here is patched for that. Some also got bot logic they never had: SuperHero bots pick heroes, SoccerJam bots chase the ball and Roll the Dice bots roll.
- **The Game Mode Menu, the server plugins and the app**, so you never have to edit a config file.
- **Fixes** for Gun Game and Deathmatch spawns on maps without spawn files, the map timer while bots learn a map, and a Deathmatch crash on long sessions.

## Requirements

- Counter-Strike 1.6, Condition Zero or both, bought on Steam (other copies aren't supported)
- Windows 10 or 11, or a Mac with [CrossOver](https://www.codeweavers.com/crossover) (Apple Silicon or Intel)
- About 1 GB free for the mods, plus room for any maps you add
- For a server on a VPS: an Ubuntu or Debian machine on x86 (AMD or Intel) with SSH access
- For voice commands (optional): a dedicated server, about 500 MB for the speech models, and a processor from about 2013 on (AVX2)

## Install

- **Windows:** download Loner-Strike from [Releases](https://github.com/jamesontucker/Loner-Strike/releases) and open it. Setup finds the game and walks you through the rest. Windows may show "Windows protected your PC" (SmartScreen) because the app is new and not yet widely downloaded: click More info, then Run anyway.
- **Mac:** the Mac version of Counter-Strike can't run mods, so play it through [CrossOver](https://www.codeweavers.com/crossover) (paid, with a free trial): install Windows Steam in a CrossOver bottle and Counter-Strike in that. Then download Loner-Strike from Releases; it finds the game inside CrossOver. With Parallels, run the Windows version of Loner-Strike inside the Windows virtual machine instead.

Everything it changes is backed up. Remove one mode, or all of Loner-Strike, from the app and your game goes back the way it was.

Updates come through the app (Settings, Updates). What changed in each version is on the [Releases](https://github.com/jamesontucker/Loner-Strike/releases) page and in [CHANGELOG.md](CHANGELOG.md).

## Troubleshooting

- **The game won't start or the panel says no game is loaded.** Start the game from Play in the app, not from Steam, so it loads with the mods.
- **Friends can't see your game on the LAN tab.** Allow the game through the firewall when asked. On Windows, also make sure your Wi-Fi is set to Private network (Settings, Network & internet).
- **A mod misbehaves after a game update.** Open Mods and click Repair; it puts Loner-Strike's basics back and keeps your settings and mods. Reinstall next to a mod puts its files in fresh.
- **You want everything gone.** In Settings, Remove from (your game) puts that game back the way it was, and Remove everything also deletes Loner-Strike's own files.

Still stuck? See the [docs](https://lonerstrike.com/docs#no-bots-help), or open Settings, and under Help click Copy next to Copy diagnostics (version, game folders, what's installed and recent errors; no passwords) and paste it into a [new issue](https://github.com/jamesontucker/Loner-Strike/issues/new).

## Run a server on a VPS

The app can set up an always-on server on a Linux machine you rent (Ubuntu or Debian, x86). In Servers, choose New server, pick "A Linux server (VPS)" and give it the machine's address, user and SSH key. It checks the machine's identity, installs Valve's server and Loner-Strike over SSH and runs it as a service on UDP 27015, so it keeps going with the app closed. Oracle's free tier works, as does a small VPS from a host like Hetzner, DigitalOcean or Vultr.

After that it's managed like any other server:
- Changes you make in the app (settings, admins, mods, maps, sounds) go up by themselves, and only the files that changed. New plugins and maps load on the next map, or with Restart now.
- Update on its Health tab gets Valve's latest server files on the machine. Bans and stats made on the server stay there.
- HLTV runs next to it on UDP 27020, and its demos can be copied to your game.
- If the machine stops answering, the app says so instead of showing it as running, and with Discord set up you hear about a server that stays down.
- After an app update, each server updates itself once nobody's playing.
- The game runs in a sandbox, RCON only answers from the machine itself, security updates and fail2ban are turned on, and old logs are trimmed. The Security and health panel on the server's Health tab checks all of it and fixes what it can.
- Admins and Moderators you invite help from their own Loner-Strike through the server itself (a small service, `ls-control`), so your computer can be off.
- Its bans, admins and settings are backed up to your computer every day.

[docs/linux-server.md](docs/linux-server.md) walks through it: picking a host and size, making an SSH key, opening UDP 27015-27020 in the provider's firewall, managing the server and fixing common problems.

## How it works

Setup installs [Metamod-P](https://github.com/Bots-United/metamod-p), [AMX Mod X](https://www.amxmodx.org) and our builds of [ReGameDLL](https://github.com/jamesontucker/ReGameDLL_CS) (the bot brain) and, for servers, [ReHLDS](https://github.com/jamesontucker/ReHLDS), then our plugins and the mods you pick from [catalog/](catalog). Mods are downloaded from their original sources and checked against pinned checksums. The app talks to the game over RCON on your computer; nothing goes through a server of ours.

```
app/src-tauri/   the app: setup, the game and server control API, sharing (Rust, Tauri)
app/ui/          the control panel (React), shown in the app window and on your phone
app/server-agent/ ls-control: shared control on a Linux server (Rust)
app/voice-agent/ ls-voice: players' voice to text for the bots (Rust, Whisper)
app/intent-agent/ ls-intent: loosely worded lines to the bots' commands (Rust, llama.cpp)
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

`cargo test` in `app/src-tauri` runs the tests. The voice programs build with `cargo build --release` in `app/voice-agent` and `app/intent-agent`, and for Linux servers with `scripts/build-voice-agent.sh` (Docker). Releases are built by GitHub Actions when a `v*` tag is pushed (see `.github/workflows/release.yml`).

## Contributing

Contributions are welcome. It's a side project, so I work on it on and off and replies can take a while. For anything bigger than a fix, open an issue first so we can talk it through.

## Support the people who made this possible

Loner-Strike stands on twenty years of community work. If you enjoy it, give some love to them too:

- **[GameBanana](https://gamebanana.com):** maps, skins and sounds come from its creators. Like and comment on what you play, and follow the people who make it.
- **[AlliedModders](https://www.alliedmods.net):** home of AMX Mod X and most of the classic mods. Its forums and plugin library still keep these games alive.
- **[The ReHLDS team](https://github.com/rehlds):** ReGameDLL and ReHLDS are the base of our bot brain and server engine. Star their projects, and report what you find.
- **The mod and map makers:** Zombie Plague, Warcraft 3, SuperHero, Gun Game and the rest. Every author is named in [CREDITS.md](CREDITS.md).

## Credits

Made by [nanoshrine](https://nanoshrine.com). See [CREDITS.md](CREDITS.md) for every mod and its authors.

Not affiliated with Valve. Counter-Strike and Condition Zero are Valve's trademarks. No game files are included; you need your own copy.

## License

GPL-3.0. Third-party mods keep their own licenses (see CREDITS.md).
