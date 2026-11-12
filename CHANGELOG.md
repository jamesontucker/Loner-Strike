# Changelog

What changed in each version. The downloads are on the [Releases](https://github.com/jamesontucker/Loner-Strike/releases) page.

## 1.0.0

The first full release:
- Bots that play like people, on Loner-Strike's own bot brain
- Talk to them in team chat, or on your mic with voice commands
- An easier app: a clearer sidebar, settings before a game, and My library
- Servers that look after themselves: updates, health checks and daily backups
- Docs for everything at lonerstrike.com/docs

### Bots
- Loner-Strike's own bot brain (a fork of ReGameDLL) rebuilds how bots see, aim, move, plan and talk. Dedicated servers also run our fork of the ReHLDS engine. Both are open source.
- Talk to them in team chat: "money?", "where'd they go", "rush b", "fake a", "flash for me", "two long", "someone buy me an awp". They answer from what's really going on, do it or say no in character, and only one ever buys you that awp. Start with a name or nickname to ask just one.
- They aim like a hand on a mouse: still, then a flick that lands a touch off and gets corrected. Better players are quicker and steadier.
- They play as a team: trades and flanks, Ts gathering out of sight and taking a site together, CTs holding and retaking, one T picking up a dropped bomb, guards around a planted one.
- They look where it matters, hold angles, aim around the chest, glance back at a sound, and panic, tilt and back off like people.
- Each bot is someone: a personality, mood and skill, and regulars who remember you.
- Every mode is played right, from deathmatch to Gun Game's knife level, and fy_, awp_ and aim_ maps no longer stall.
- Call plays to your whole team: "rush b" sends the team, "two go a, rest b" splits it.
- They read flashes like people: turn away from one they see coming, then look back where it came from.
- They notice a gun barrel, shoulder or foot past cover, and the better ones shoot through it.
- They remember who they backed off from and decide what to do about it: hold, take another angle, go back in or go round, each in their own way.
- Fights look more human: they spread out instead of stacking, stop to shoot by habit, reload out of sight, and fire a shotgun mid-reload.
- They buy ammo with their Deagle and don't charge in with an empty gun.

### Voice commands
- On a dedicated server, the bots understand voice chat the same as typed team chat, loose wording included. An optional add-on per server (about 500 MB, once per computer), installed and removed from its Bot voice commands tab. Speech is turned into text on the machine running the server; no audio is kept or sent anywhere.

### The app
- The sidebar is grouped Game, People, Online, My library and Hosting, and a bar on every page says whether you're changing your game or a server.
- Bots, Rules, Fun and Add-ons work before a game starts, and Reset bot and fun settings puts the defaults back.
- My library: Maps, Mods, Skins & models (what you've installed, with Remove), Sounds and Get more.
- Counter-Strike 1.6 no longer lists Condition Zero only maps it can't load.
- The quick bar changes bot difficulty and style in one click.
- Settings, Help fixes the usual problems by symptom, and the app's messages say what to do next.
- The maps Loner-Strike installs and the stock maps (all but the two biggest, de_survivor and de_torn) come with ready-made bot navigation and map knowledge, so bots play them straight away with no learning. On the stock maps bots name places in chat, like BombsiteA, Middle or Tunnel.
- Bots learn maps split by a wall or barrier properly (Base Builder, Deathrun): every spawn gets navigation.
- Zombie Plague Advanced no longer crashes when a map loads on Windows and Mac servers: its settings were read back empty.
- Base Builder: aiming at a block on maps with long entity names (bb_street) no longer breaks that player's building controls.
- Setup now adds 27 fun and popular maps (about 70 MB): scoutzknivez, aim_map, fy_iceworld, awp_india, he_glass, de_mirage, de_tuscan, the Rats pack and more. Remove any it added that you don't want in Maps, and they won't come back. Dedicated servers keep to the maps you give them.
- Map downloads are checked first: one missing a model or sprite it needs, which would crash the game when it loads, isn't installed.
- Zombie Plague now comes with zm_zod_labtest instead of zm_halloween, which crashed on loading, and zm_tehina is removed (unless you added it yourself). Kreedz now comes with kz_beginner and kz_airmountain; kz_urban (which crashed too), kz_summercliff2 and bkz_goldbhop are removed when you update, unless you added them yourself. Base Builder no longer comes with bb_dust_iplay, which has more models than the game can load with the mode running.
- Scoutz on scoutzdeaglez gives you the deagle the map is made for.
- New installs start with Quake sounds off and bot chat at Normal. Your settings, plugins, admins and edited config files are kept through updates.

### Servers
- A server's tabs are grouped Server, Game, People and Library, with new Health, Who can join, Moderation and Stats tabs. Helpers see what's owner only.
- Servers update themselves once nobody's playing, or with Update now, and show their progress.
- Health checks a Linux machine in plain words (logins, RCON, updates, fail2ban, disk, memory) and fixes what it can. New servers start locked down.
- Linux servers are backed up to your computer every day, and Discord hears when one goes down.
- Shared control is safer: Moderators can't act against the owner or Admins, and can see bans.
- Choose a server's tick rate in its advanced settings.
- A mode rotation: every few maps the mode changes to the next one on your list, or a random one, even with nobody on. Rock the vote still works on top.
- Modes played on their own maps (Deathrun, Base Builder, Kreedz, Surf, SoccerJam, the zombie modes, Hide and Seek) go through those maps by themselves, so a switch never lands on de_dust2.
- From your phone you can now change a server's settings, use its console, run its security checks, set up HLTV, website status and voice chat, share lists between servers, and install maps, sounds and add-ons. Deleting, reinstalling, making servers and shared control stay on the computer.

### Fixes
- No bots or "Bad Rcon": the app puts back what a Steam update undid and says what's wrong. Mods has a Repair button.
- Dedicated servers load de_survivor, cs_siege, cs_747 and other maps the 25th anniversary update changed, which used to stop the server with an error.
- Windows: voice runs without extra installs on any processor from about 2013 on, updates don't trip over it, and game folders in any language work.
- Big downloads resume where they stopped, and a full disk is named as such.
- Paintball runs lighter in busy fights: fewer balls and marks on screen at once, marks that fade sooner, and splash sounds heard nearby only.
- Old retail or repacked copies of the game are caught at setup.
- "Remove everything" no longer deletes the app itself on Windows.
- Updates show what's new before you install, and once after.
- Bot pings on the scoreboard stay believable, rarely over 100.

## 0.1.2

- Gun Game (every player for themselves) has no teams underneath any more, so bots stop moving in groups with teammates they couldn't shoot. Team Gun Game is unchanged.

## 0.1.1

### Fixes
- The game no longer crashes on a map change on Windows. AMX Mod X's Ham Sandwich module used memory it had already freed, which Windows caught. Loner-Strike brings a fixed build of it, on Windows, on the Mac through CrossOver (where Wine only hid the bug) and on dedicated servers on Linux.
- Windows 10: map downloads packed as RAR files unpack again (Windows 10's own unpacker can't open many of them, so the Windows app now brings 7-Zip).
- Windows: Base Builder and other modes with their own folder compile during setup.
- Mac: Base Builder's compile no longer crashes in Condition Zero under CrossOver.
- Changing the resolution in the game while hosting no longer crashes it: the game goes back to its main menu, and you start the game again from there or from the app.
- The window fits the screen it opens on, when it was last used on a bigger monitor.

### Changes
- Bots start at Hard, and bot chat at Chatty. Changing bot chat takes effect straight away.
- Map pictures are looked up for every map in the background, and fill in on their own. Radar pictures lose their green background.
- Website status can keep a Discord message up to date with who's playing, and post when people come on.
- Website status can serve fast downloads (maps, sounds and models) from the server itself, and sets the download address.
- Find servers: a server browser like Steam's old one, with Internet (servers Loner-Strike hosts share), Favorites, History and LAN, server info with who's on, filters and passwords. A server can be put on the list from its settings.
- The radio has eleven stations, with old-school hardcore and jungle, hands up, hardstyle, demoscene, video game music, 2000s hits and nu metal added, plus shuffle and a station list.
- Copy diagnostics includes the end of AMX Mod X's error log.
- Servers can use weapon skins and player models: a new Skins & models tab lists what a server uses, who sees each (everyone, one team or admins), and how close each map gets to the engine's limit of 512 models and 512 sounds. They go in under their own names, so players download them when they join. Modes with their own models, like Paintball, keep them.
- Paintball is lighter to draw: at most 85 paintballs and paint marks at once (PBR_MaxBalls), with the oldest marks making room instead of shots going missing, and dropped guns gone after 10 seconds. Busy areas had dropped players' games to single-digit frame rates.
- Gun Game spawns players at random again on maps without a CSDM spawn file: the spots taken from the bots' map count as a proper random set.
- A server's content is together under Content: Mods, Maps (with Get maps), Skins & models and Sounds.
- Two announcers for Quake sounds, picked per game or server on the Sounds page.
- Quake sounds are heard: setup brings the announcer (first blood, multi kill, godlike and the rest, from Advanced Quake Sounds) and switches the announcements on. Servers send the sounds to players like any mode's.

## 0.1.0

The first public version.

### Setup
- Sets up Counter-Strike 1.6 and Condition Zero on Windows, and on Mac through CrossOver: Metamod-P, AMX Mod X, ReGameDLL, the Game Mode Menu and the mods you pick.
- Everything it changes is backed up. Remove one mod, or all of Loner-Strike, and your game goes back the way it was.
- Updates from the app itself, and an update bar when a part of your game or a server needs refreshing.

### Playing
- 22 classic community modes, from Zombie Plague, Warcraft 3 and SuperHero to Gun Game, Deathrun and Surf, patched so bots can play them.
- Start a game from the app: pick the mode, map and bots, and it starts the game and loads straight in. If the game is already open, it restarts it into your game.
- Presets for a quick setup (Competitive 5v5, Arcade chaos, Zombie night and more), and a quick bar for the round, the map, bots and yourself.
- Maps with screenshots, search and filters, favourites and hidden maps, map downloads from GameBanana, and map presets that switch the mode for a map.
- Round-end music, Quake sounds and sound packs.
- A radio in the app, with Bassdrive and SomaFM.

### Bots
- Bot styles with human-like reactions and aim, from a newbie lobby to bots that bunnyhop, and a Personality setting for how much of their character shows.
- Bots play as individuals: rushers, roamers, lurkers and careful players, holding angles on aim maps, and no shadowing you around the map. Only Classic and Match play as a team.
- Short, quiet 2004 pub chat: they react to kills and rounds, answer you when you talk to them (where he was, the score, the bomb, how you died), keep short conversations going and don't repeat themselves.
- 1,600 regular names that come back, a few of them in small clans.
- Bot map files shared from your game to your servers, and Teach bots maps to learn custom maps ahead of time.

### Servers
- Dedicated servers on your computer or on a rented Linux machine (VPS) over SSH, set up and run as a service that keeps going with the app closed.
- Each server has its own modes, maps, admins, bans, welcome page, sounds and settings, and changes go up as you make them. Files the server keeps itself, like mutes, favourites and stats, are read from the server.
- A public address (like play.example.com), bots on an empty server so it looks alive in the server list, and network settings tuned for steady play.
- Copy your own maps to a server, with everything each map needs.
- HLTV to watch matches and record demos.
- Website status: a small service on the server that tells your site the live player count and counts visits, over its own HTTPS or a Cloudflare Tunnel.
- Mode voting: rtv picks the mode first, then a map that suits it, with an optional mode vote at the end of a map.

### People
- A players list with a menu on each row: kick, timed bans, mutes, admin and VIP roles, respawn, heal and more, with bots tagged and a filter for people or bots.
- Admins and VIPs, shared ban and admin lists across servers, stats and moderation, player reports and a word filter.
- Seven ways for friends to join: the same network, a direct connection through your router, playit.gg, Tailscale, ZeroTier, Radmin VPN or Hamachi.
- A control panel on your phone, signed in with a QR code.
