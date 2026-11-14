# Loner-Strike 1.0.0

DRAFT: not reviewed and not published.

Loner-Strike 1.0.0 is the first full release. You still get everything from 0.1, and now you can also build, play, host and share. Bots run on our own bot brain and play, talk and listen like people. There are two new modes made here, Last Stand and Battle Royale, plus a new Tools section (Beta) with a map editor, Game Mode Studio and more. Servers now look after themselves, the Community page shares what people make through GameBanana, and you can let AI agents help.

## Play

- **A clearer app.** The sidebar is grouped into Game, People, Online, My library, Hosting and Tools. A bar on every page says whether you're changing your game or a server, with Back to my game.
- **Settings before a game.** You can set Bots, Rules, Fun and Add-ons before a game starts. Reset bot and fun settings puts the defaults back.
- **My library.** Maps, Mods, Skins & models, Sounds and Get more are in one place. Skins & models lists what you've installed, with Remove. Tags show what you made, what came from Community and what you remixed.
- **27 fun and popular maps.** Setup adds scoutzknivez, aim_map, fy_iceworld, awp_india, he_glass, de_mirage, de_tuscan, the Rats pack and more. If you remove one, it won't come back.
- **Bots play the maps straight away.** Every stock map except de_survivor comes with ready-made bot navigation, and so does every map Loner-Strike installs. Bots name real places in chat, like BombsiteA or Tunnel.
- **Safer map downloads.** A download that's missing a model or sprite it needs would crash the game, so it isn't installed.
- **Maps per game.** Counter-Strike 1.6 no longer lists, votes on or starts Condition Zero only maps. A filter on Maps still shows them.
- **A richer top bar.** It shows rounds won by each team, people and bots counted separately, the next map, and Paused or Recording.
- **Quick bar.** Change bot difficulty and bot style in one click, and start a mode vote from the Round menu.
- **Crosshair & config builder.** Design your crosshair with a live preview and presets. Keys, mouse, network, video and HUD settings are on tabs, and a backup is saved first.
- **Demos.** One table lists your own demos and your servers' HLTV recordings, with map, length, players and date. You can Watch, Rename, Show in folder or Delete a demo, or open it in Replay.
- **Help by symptom.** Settings › Help fixes the usual problems, and the app's messages say what to do next.
- **Updates show what's new.** You see the notes before you install and once after.
- **Radio.** The radio adds PsyStream (psytrance) and nine chill stations, with a Chill tag.
- **Phone panel.** On a phone, tabs swipe, words don't break and tap targets are bigger. Mode votes are full-width rows.

## Bots

- **Loner-Strike's own bot brain.** We rebuilt how bots see, aim, move, plan and talk in our own fork of ReGameDLL. Dedicated servers also run our fork of the ReHLDS engine. Both are open source.
- **Aim like a hand on a mouse.** A bot holds still, flicks, lands a touch off and corrects. Better players are quicker and steadier. Bots don't aim at you without shooting, and they fight the enemy that's the biggest threat.
- **Teamwork.** Bots trade and flank. Ts gather out of sight and take a site together, and CTs hold and retake. One T picks up a dropped bomb, and others guard a planted one.
- **They play fair and see what you'd see.** Bots notice a barrel, shoulder or foot past cover, and the better ones shoot through it. They turn away from flashes, glance back at sounds, and panic, tilt and back off like people.
- **Each bot is someone.** There are 13 playstyle personalities, each with its own mood, skill and habits. Bots fidget in freeze time, celebrate a won round, spray the Loner-Strike logo, and remember the regulars they play with.
- **Talk to them in team chat.** Try "money?", "where'd they go", "rush b", "fake a", "flash for me" or "someone buy me an awp". Bots answer from what's really happening and do it or say no in character. Start with a name or nickname to ask just one bot.
- **Call plays for the whole team.** "rush b" sends everyone, and "two go a, rest b" splits the team. The Bots follow orders setting decides how often they listen: Rarely, Their own way, Mostly or Always.
- **Voice commands (Advanced Bot Interaction).** On a dedicated server, bots understand your mic the same as team chat, loose wording included. It's an optional add-on per server, about 500 MB once per computer, and it works on Linux servers too. Speech becomes text on the machine running the server, and no audio is kept or sent anywhere.
- **Bigger chat.** Bots know thousands of lines about buys, grenades, fakes, flanks and callouts. They react to players on voice and answer questions you didn't ask. UK bots use UK spelling.
- **"He's lit" calls.** A bot that hurts an enemy who gets away tells its team where, and how much health the enemy has left.
- **Ghosting (optional, off by default).** A hot-headed dead bot can now and then tell its team where an enemy is.
- **Every mode played right.** This covers deathmatch, Gun Game's knife level, fy_, awp_ and aim_ maps, Deathrun (where bots now play with the brain), and Last Stand and Battle Royale.
- **Bots surf.** On surf maps, bots strafe into each ramp they reach and keep their speed instead of standing still.
- **Bot style picker.** Bot style is a dropdown in Settings, from Original (the stock bots) through the easiest style to Gods.
- **Learning a map.** When bots learn a new map, a progress line shows on screen and on the Bots page. Maps split by a wall, like Base Builder and Deathrun, are learned properly, and every spawn gets navigation.
- **Your own callout names.** Names you give places in the Callouts tool are used by bots in chat, on the radio and in orders ("go catwalk"). The names go to your servers with the map.
- **Believable pings.** Bot pings on the scoreboard rarely go over 100.

## Modes

- **Last Stand (made here).** Hold the base against endless rounds of Half-Life creatures, or knife-only zombie bots. It has barricades you repair, a shop and armory, perks, a gun upgrade bench, a mystery box, paid traps, turrets, power-ups and bosses. If you go down you crawl and bleed out until a teammate revives you. Bots defend, repair, buy and revive with you.
- **ls_outpost.** A walled outpost at night, built for Last Stand. It has three boarded gates, battlements to shoot down from, and spawns for 4 to 16 players (playable up to 32).
- **Battle Royale (made here).** You drop in from the sky and steer down. Loot up before anyone can be hurt, then survive a shrinking zone. Loot includes healing items, armor levels 1 to 3, scarce ammo, items that glow by rarity, supply drops, and death crates with everything a player carried.
- **Duos and Squads.** These are their own modes. You get a squad lobby, no squad damage, markers and radar dots for squad mates, and a knocked-down state where only your squad can revive you.
- **Battle Royale HUD.** It shows players left, kills, the zone timer, and how far away and which way the next circle is. You get a kill feed, and you watch your killer after you die.
- **br_valley.** A big Battle Royale map in a river valley, with towns, a river and waterfall, ridges, and a whole level of caves, mines and bunkers below ground. Bots spread over the surface, high ground and the level below.
- **Creatures.** New Half-Life creature types join Last Stand: vortigaunt, baby gargantua, assassin, soldier, gonarch and more. Tougher tiers glow, heavies enrage when hurt, and barnacles hang from ceilings.
- **Mode rotation.** Every few maps the server changes to the next mode on your list, or a random one, even with nobody on. Rock the vote still works on top.
- **Modes stay on their own maps.** Deathrun, Base Builder, Kreedz, Surf, SoccerJam, the zombie modes and Hide and Seek cycle through their own maps by themselves, so a switch never lands on de_dust2.
- **Mode settings.** Some modes let hosts change their options. You'll find them on the Play page, on a server's Modes tab, and in the in-game admin menu.
- **Scoutz on scoutzdeaglez** gives you the Deagle the map is made for.
- **New defaults.** New installs start with Quake sounds off and bot chat at Normal. Pub server is the default bot style. Auto bunnyhop no longer carries over into other modes. Your settings, plugins, admins and edited configs are kept through updates.

## Make

All of these are in the new Tools section, marked Beta.

- **Map editor.** A modern GoldSrc editor in 3D and 2D views (Top, Front, Side), with the camera controls of Unreal, Blender or Hammer.
  - Shapes: box, cylinder, wedge, spike, sphere, arch, cone and torus.
  - Brush editing: clip, push/pull, extrude, vertex editing and shear.
  - Texturing: a face sheet that works like Hammer's, with an eyedropper and texture wrapping.
  - Entity forms, prefabs, undo, autosave with crash recovery, and Hammer or TrenchBroom keys.
- **Hammer and J.A.C.K. import.** Open .rmf, .jmf and .map files with their texture alignment, groups and visgroups. Slanted faces come in exactly where they were.
- **Compile and Playtest.** Press F5 to compile and play your map with bots. The compiler (SDHLT) downloads on first use. Problems point into the map, and the leak line and the hole are shown. On a Mac, the compiler runs through CrossOver.
- **Live checks and Budget.** The editor finds leaks, stuck spawns, flickering faces, lights inside walls, missing textures, models and sounds, and how close the map is to the engine's limits. Most problems have a one-click fix.
- **Content Browser.** Browse models with 3D thumbnails, animated sprites, sounds you can play, textures and prefabs, and drag them onto the map. Open it with Ctrl+Space on Windows or Cmd+Shift+Space on a Mac. Each item shows whether you may share it.
- **Gameplay panel.** Drag in game mode pieces, like spawners, zones, barricades, loot, supply drops, waves and creatures. Mode kits show what a mode still needs on your map.
- **Entity I/O view.** See what triggers what in one graph, with broken links marked.
- **Game Mode Studio.** Build a whole game mode visually, Blueprint style, with nodes for players, bots, creatures, zones, shops, perks, revives, squads and more. No coding needed.
  - Includes: functions, macros, custom events, Timelines, variables, a snippet library and My library to reuse your own pieces.
  - Start from five example modes: Infection, Gun Game Lite, King of the Hill, One in the Chamber and Juggernaut. Or start from the Last Stand Lite and Royale Lite templates.
  - Press F1 on any node for help, or take the short tour.
- **Test your mode.** Playtest puts it straight into your game. Compile checks it, and a debugger with breakpoints shows live values. Test run plays it with bots on a hidden server and reports pass, warn or fail. You can mark a variable as a setting hosts can change.
- **Plugin Studio.** Write AMX Mod X plugins with suggestions, hover docs and plain-language compile errors, then try them in your game. Loner-Strike's own plugins are read-only.
- **Creature Studio.** Make creature types with their health, speed, attacks, tiers and fleeing, and see a live 3D preview. You can also set up waves for Last Stand.
- **Map simulator.** Bots play your map on a hidden server. You get a report drawn on the map showing where bots went and died, falls, stuck spots, rounds won per side, and places nobody visits.
- **Map doctor.** Check a map's files without playing it, including missing textures, models, sounds, sky, .res, nav and overview. Most fixes are one click.
- **Nav fixer.** Edit the bots' navigation on the map in 3D or top-down. Draw areas, link them, fix problems with Show me, and save with a backup.
- **Callouts.** Paint place names onto the map from above. Bots and the game use your names.
- **Radar maker.** Draw a map's spectator overview from the map itself and install it.
- **Map viewer, Replay, Textures & models.** Fly through a map lit the way the game lights it, and watch a recorded match from above. Browse every texture and model, and make your own textures.
- **Bot people editor.** Edit who the bots are: their names, regulars and clans.
- **History & restore.** Every tool that writes a file keeps earlier versions. Compare, Restore or Reset to Loner-Strike's default from the History button.
- **Easier to find your way.** Each tool has a breadcrumb trail and Back (Alt+arrows or the mouse's back button). Focus mode fills the window, ? shows each tool's keys, and / searches.

## Host

- **Shared control.** Friends can help run your servers as Admin or Moderator. On Linux servers it works with your computer off and with no port forwarding.
- **Servers update themselves.** Loner-Strike's parts go in once nobody's playing, or when you press Update now. Progress shows on the server's card and page. A failed update backs off instead of restarting the server every hour.
- **Health and security.** A Linux server's Health tab checks logins, RCON, updates, fail2ban, disk, memory and CPU in plain words, and fixes what it can. New servers start locked down, and logs are trimmed every day.
- **Crash tracking.** Health › Crashes lists each crash with its map and details and a Copy button. The card shows a "crashes in 24 h" badge. Discord alerts are optional.
- **Down alerts.** Discord hears when a server stops answering, and again when it's back. A stop you made on purpose sends no alert.
- **Backups.** Linux servers are backed up to your computer every day: bans, admins and settings, with the last 7 kept. Settings › Backup & restore saves and restores your servers and app preferences.
- **New server tabs.** Tabs are grouped Server, Game, People and Library, with new Health, Who can join, Moderation, Stats and Add-ons tabs. Helpers see which things are owner only.
- **Better console.** Chat shows team tags and colored names. Kills show the weapon, and you see joins, leaves and map changes. Errors are in red. There are suggestions for every setting, plus Copy and Clear.
- **Start in a mode.** Pick the mode a server starts in. Pub games are every bot for itself, and Matchmaking is team play with real teamwork.
- **Tick rate.** Choose a server's tick rate in its advanced settings.
- **Run servers from your phone.** From your phone you can change settings, use the console, run security checks, set up HLTV, website status and voice commands, share lists between servers, and install maps, sounds and add-ons. Deleting servers, making new ones and shared control stay on the computer.
- **Maps load on dedicated servers.** de_survivor, cs_siege, cs_747 and other maps changed by the 25th anniversary update load properly.
- **Downloads and uploads.** Downloads come from Loner-Strike's own mirror first, resume where they stopped and retry a busy server. Uploads go up in pieces that retry.

## Share

- **Community.** Browse and try maps, prefabs and Game Mode Studio modes made with Loner-Strike and hosted on GameBanana. Try it installs the item, and a download is checked against GameBanana's own file.
- **Publish.** Numbered steps check your map or mode with Map doctor, Map simulator, Callouts and Radar maker. Then the app builds a package you upload to GameBanana, and it shows up in Community.
- **Credits are tracked for you.** Get more shows a GameBanana item's terms before you download. Publish lists where every file came from and fills in the credits. It warns about "ask first" content, and you can record that an author gave you permission.
- **Updates with Go back.** An update never installs while its map or mode is in use. It goes in at the next map change, and your previous version is kept so you can go back. The sidebar shows how many updates are waiting, and Update all installs them.
- **Your servers get updates too.** A server that carries a community item is updated once nobody is playing that map or mode.
- **Safe modes only.** Community shares content and Studio modes, never plugins that run their own code. A mode that runs server commands or can kick players says so in plain words, and asks you before installing.
- **How well bots play it.** Published modes carry a bot rating, and you can filter by it.
- **Get everything.** A mode's page shows the maps and content it needs, marks what you have, and installs the rest with one click.
- **Remixes and credit.** Creations keep a permanent identity, so updates and remix credit follow them. You can unlist your own creation and list it again.
- **Playtest screenshots.** When you publish a mode, you can pick from the screenshots you took while playtesting.
- **Spray maker.** Make a spray from layers of pictures, words and stickers. You can use the fonts on your computer (Windows and macOS), start from templates, and see it on a real wall the way the game draws it. Install it with one click, and your old one is kept.
- **Prefabs carry their credits** when you share them, and installed creations remember who made them.
- **Join the Discord** from Community and Find servers.
- **Terms.** You'll find Terms and Privacy in About. Publishing asks you to confirm you have the right to share.

## AI agents

- **Connect an AI agent.** Let Claude Code, Codex, OpenCode, Cursor, Claude Desktop, VS Code or any other MCP agent use Loner-Strike. Go to Settings › AI agents, pick your agent, press Copy and paste the setup where it says.
- **The map editor and Game Mode Studio.** Agents can do every edit you can: build, texture, check and fix maps, compile and playtest. In the Studio they can build, check and test-run modes.
- **They see what you've selected.** Agents know what you've picked in the map editor and can pick things for you.
- **The rest of the tools.** Agents can fix navs, name places, make overviews, read replays and demos, and run Map simulator (it asks first).
- **Servers.** Agents can run your servers and read their consoles, logs, crashes and what the bots are thinking.
- **You stay in charge.** The connector is off until you turn it on, and only your own computer can reach it. It's look-only until you turn on Allow changes.
- **Big actions ask first.** The app asks before deleting, kicking or banning, stopping a server people are on, starting the game, compiling into the game, installing community content, or anything public.
- **Some things stay yours.** Agents can't touch these settings, updates, phone access, shared control or credit permissions. Passwords, tokens and keys are never shown to them.
- **What agents did.** A log lists every call: done, not allowed, or didn't work.
- Try: "make a small aim arena, check it for bots and playtest it".

## Fixes

- **No bots or "Bad Rcon".** When a Steam update undoes the setup, the app puts it back and says what's wrong. Mods has a Repair button. The app also catches it when another copy of the game is the one running.
- **Old copies of the game.** Old retail or repacked copies are caught at setup.
- **Fewer crashes.** Loner-Strike's fixed Ham Sandwich module is put back before every game and server start, so a crash it caused on map changes doesn't come back.
- **Zombie Plague Advanced** no longer crashes when a map loads on Windows and Mac servers.
- **Crashing maps replaced.** Zombie Plague, Kreedz and Base Builder no longer ship maps that crashed or wouldn't load.
- **Base Builder.** Aiming at a block on maps with long names (like bb_street) no longer breaks your building controls.
- **Paintball** runs lighter in busy fights.
- **Windows: game folders.** Game folders in any language work, older Steam library lists are found, and saves no longer fail when antivirus or OneDrive briefly locks a file.
- **Windows: setup.** If the game is open, setup waits with "Close Counter-Strike to continue" instead of failing.
- **Windows: crashed servers.** A crashed server exits at once instead of hanging behind a "has stopped working" box. A port another program uses is named plainly instead of causing a restart loop.
- **Windows: browser keys.** F5, Ctrl+R, Ctrl+P and files dropped from Explorer no longer reload or replace the app.
- **Windows: Remove everything** no longer deletes the app itself or fails partway.
- **Windows: voice commands** run on any processor from about 2013 on, with no extra installs.
- **Windows: archives.** RAR and other archives unpack without creating links or hidden streams.
- **Downloads** resume where they stopped, and a full disk is named as such.

## Known issues

- Tools are a Beta, so expect rough edges.
- Bots surf ramp to ramp but don't follow whole routes through a surf course yet.
- In Duos and Squads, bots seldom revive a knocked-down squad mate.
- The Last Stand Lite template in Game Mode Studio is simpler than the built-in Last Stand. It has no power-ups, for example.
- Map simulator takes about 4 minutes before bots start playing.
- Nav fixer's top-down view shows the first surface from above. Under roofs and in tunnels, use 3D or Split.
- You can't change a shared server's mode settings from your app over shared control. Change them in the server owner's app.
- Mac (CrossOver): editing very large maps is slower than on Windows. The first Playtest can take up to about 90 seconds. Timelines run slower in game.
- Windows: on German and French keyboards, AltGr combinations can trigger app shortcuts. Uninstalling leaves firewall rules and server folders behind.

## Not sure it landed (check before publishing)

- Game Mode Studio data tables and live tweaks while playtesting. The site docs mention them, but the notes say they're waiting to be committed.
- Bots learning a Counter-Strike 1.6 map before an editor Playtest. The fix is committed, but the notes say it hasn't been run against a live server.
- Windows installer as a single setup.exe (no MSI), with 0.1.2 updating in place. It's committed in the build config, but no release build has run it yet.
