# Changelog

What changed in each version. The downloads are on the [Releases](https://github.com/jamesontucker/Loner-Strike/releases) page.

## 0.1.0 (not released yet)

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
