# Changelog

What changed in each version. The downloads are on the [Releases](https://github.com/jamesontucker/Loner-Strike/releases) page.

## 0.1.3

### Bots that play like people
- Talk to them in chat. Ask "money?", "hp?", "where'd they go", "who killed me" or "are they eco" and they answer from what's really going on. Call "rush b", "fake a", "fall back", "watch flank", "eco", "force", "flash for me", "smoke mid" or "someone buy me an awp" and they do it, or say no in character: the whole team hears a call and each decides for itself, and only one bot ever buys you that awp. Callouts like "two long" or "awp mid" turn their heads and pull CTs over to a site being hit. Start with a bot's name, a short form or a nickname to ask just that one. The app lists what they understand, beside Bots answer you. A bot only says yes when it actually does it.
- They look around and aim like a hand on a mouse: still most of the time, quick flicks that land a touch off and get corrected, a small tremor, and a lag behind fast strafes. Better players are quicker and steadier.
- Freeze time and leaving spawn feel alive: looking about, flicking guns, hops and crouches, and a few always late out of the shop.
- Team play: trades and flanks, CTs holding the sites, Ts picking a site together and switching after early losses, the bomb carrier going in with a teammate, guards spreading out round a planted bomb, defuses heard and kit holders let through.
- They react like people: flashed bots panic their own way, AWPers step aside after a shot, a flanker who shoots them gets noticed, and being outnumbered makes the careful ones back off.
- Ts take a site together: they gather just out of sight on their ways in, then go in at once, instead of walking into the CTs one at a time.
- A dropped bomb gets picked up: one T goes for it (not none, and not all of them), the rest of the push leaves them to it, and they find their way to it from wherever it landed.
- Retakes go for the bomb: CTs who can still make it run there, and learn the site from a teammate on it or one who just went down there. Ones who can't make it save.
- They look where it matters: running somewhere they face the way they're going and glance over a shoulder now and then, a sound from behind gets a quick look (or none, from a rusher) instead of running backwards, and holding a spot they check the openings in turn, through glass and doors, never staring at a wall. How often they check, and how long they hold an angle, depends on who they are.
- They hold an angle for a few seconds instead of glancing round every second, and aim around the chest like most players, so headshots come at a pub's rate rather than every kill or none.
- Each bot is its own person: a personality, mood and skill that show in how it plays and talks, regulars who remember you between sessions, tilt from a bad run, and callouts like "rush b" before they go down.
- Chat volume fits the mode, deathmatch chat is short reactions, and with Bots can be rude off nobody gets insulted.
- Every mode is played right: deathmatch spreads out and goes for the fighting, Gun Game's grenade and knife levels work, fy_ maps get a run for the floor guns, and rounds on awp_ and aim_ maps don't stall.
- Deathmatch and Gun Game no longer end the round and respawn everyone at 9 minutes.

### Advanced Bot Interaction
- The bots understand what people say on voice chat, the same as typed team chat: say "rush b" or "Derek, flash for me" on your mic and they react. Loose wording works too ("can somebody get over to B real quick"): a small model made for this turns it into the order it is, and anything that isn't one (chat, trash talk) reaches the bots as said. An optional add-on per server, installed from its Bot voice commands tab (about 500 MB of models, once per computer) and removed just as easily. Speech is turned into text on the machine running the server, and no audio is kept or sent anywhere. Works for servers on this computer and on Linux machines (there it runs beside the server, only able to talk to the server itself). Not for games hosted from Play: it needs Loner-Strike's server engine. It needs a processor from about 2013 on (AVX2), and says so plainly on an older one.

### Servers
- Servers update themselves: after an app update, each server gets its new plugins and bot brain once nobody's playing on it (bots don't count). The servers list shows what's waiting, with Update now, which asks first if people are on. Turn it off per server on its Overview tab. While one updates, its card and its page show the step it's on and how far along it is, and Start, Stop and the other buttons wait until it's done.
- A Security and health panel on each Linux server's Overview checks the machine (password logins, RCON, automatic updates, fail2ban, a restart waiting, the game's sandbox, disk space, memory and CPU) in plain words, with a button for what the app can fix. The servers' logs are trimmed once a day so they can't fill the disk. New servers get RCON closed to the internet, automatic security updates and fail2ban from the start, and the docs have a Keeping it safe section.
- Linux servers are locked down further: the game can't use the login's admin rights or touch SSH keys and login scripts, and shared control runs with only what it needs. Both go in at the server's next start.
- Shared control: Moderators can no longer kick, slap, slay, ban or mute the owner, Admins or other helpers, and invites ask for a SteamID. The Cloudflare steps are spelled out with copy buttons and fold away once the route works, with a clear message if it's in the wrong order.
- Website status checks shared control, fast downloads and the Discord messages from outside, so you can see they work. With Discord set up, it also says when a server stops answering while it's meant to be running, and when it's back (a server stopped on purpose says nothing).
- Servers on Linux machines are backed up every day while Loner-Strike is open (bans, admins and settings, the last 7 kept on this computer), so losing the machine doesn't lose them. Turn it off in Settings, Backup & restore.
- Uploads to a Linux server go up in pieces and retry on their own when the connection stalls, instead of hanging.

### Other
- When the game won't let the app in (no bots, "Bad Rcon" in the console), the app checks the game's setup and puts back what a Steam update or file check undid, then says what was wrong: Loner-Strike not loading (and that an antivirus may have taken its files), a different copy of the game running, or a config setting its own console password.
- Mods has a Repair button that puts Loner-Strike's basics back in place, keeping your settings and mods. It also mends a read-only liblist.gam, and keeps any other Metamod plugins you had.
- Windows: the voice helpers run without Microsoft's Visual C++ runtime and on any processor from about 2013 on, an app update no longer trips over voice running in the background, and a game folder named in Russian, Chinese or any other language is read right.
- Big downloads (the voice models, maps) carry on where they stopped when the connection drops, a slow connection gets there however long it takes, and a full disk is named as such instead of blamed on the internet.
- Docs at lonerstrike.com/docs: getting started, every page of the app, the bots, voice commands, LAN parties, servers (the VPS guide moved here) and fixes for the usual problems, with search. The app's messages link to the right part.
- Clearer all round: questions are asked before anything says it's busy, buttons only spin for what you pressed, one name for each thing (Repair, Restore a backup, All servers), links that land on the right part of Settings, and no dead ends when the game isn't found or a server is full.
- A copy of the game from before Valve's 2013 update (old retail or repacked ones) is caught at setup: Loner-Strike's mods can't load on it.
- Windows: "Remove everything" no longer deletes the installed app along with everything else, so Loner-Strike opens again afterwards (at its first-time setup). Its download cache moved out of the app's own folder, which also keeps it through updates.
- Updates show what's new before you install, and once after.

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
