# QA round 4: core app pages and the classic modes (overview pass)

Fresh QA agent ("QA4-CORE"), 2026-11-14, 11:00-12:35. Breadth first: every core page and every classic mode was
smoke-tested, and I dug in only where something looked off. Nothing in the product was changed.

**Builds.**
- App: `tools` HEAD **e315c267**, from a clean `git archive` snapshot (UI built from it; the Rust test binary built from
  it into the shared target, `-j 6`, `nice 15`). For the first-time setup section the UI is from **377a0a42** (the
  Setup copy change; no server-side setup code changed between the two).
- Brain: newest green **ls-next3**: `git ls-remote` head **bdf4e8d** = CI run **43629835474** (success). Its win32
  mp.dll was put in the sim and checked inside (`revive-held`, `ls_creature_flee_last`, `ls_perf` present; the log
  says ReGameDLL 5.30.0.1301-dev).
- Plugins: **all 151** plugins the sim's configs list compiled fresh with amxxpc from the same snapshot, third-party
  modes included (zp, zp5, war3, shero, basebuilder, gungame...: every include folder under `mods/` on the include
  path), plus the 7 zp5 API plugins under their installed `zp5_*` names. Only AMX Mod X's own stock plugins (no
  source in the repo) were kept as shipped. Configs from the same snapshot.

**How.**
- The whole app's local listener (`server::router(..., Listener::Local)`, the same routes the window uses) ran from a
  scratch test harness in my snapshot only (never committed), with the app data in a scratch folder (`LS_APP_DIR`),
  `LS_SERVERS_DIR` in `Sim/QA4CORE-srv`, and the player's game pointed at a sim copy (`Sim/QA4CORE`, an APFS clone of
  LSPLAY). Pages driven with Playwright (Chromium) at 1320x900 and at the window's minimum, 820x600. The harness
  needed 4 test-only patches in the snapshot to keep real data out (see F6).
- One sim at a time: `Sim/QA4CORE` on port 27430, started from my shell with `-insecure`, 5 bots; mode switches went
  through the app (`POST /api/cmd gm_mode`, what the Play page sends). Each mode ran 130-150 s; evidence is the
  server log (kills, round ends, plugin errors) and `/api/state`.
- First-time setup: a second harness with a fresh empty app folder over a second sim copy (`Sim/QA4FTUE`, an APFS
  clone of a game folder with `addons/` removed), `LS_SERVERS_DIR` in `Sim/QA4FTUE-srv`. The folder picker (a native
  dialog the harness has no window for) was answered by Playwright, and the scan was filtered to the sim copy so the
  real game could never be picked.
- The real game and the real app data were never touched: no file under the real Half-Life folder and nothing in
  `~/Library/Application Support/Loner-Strike` is newer than 11:00 (checked with `find -newer`). No built app ran with
  another HOME. Every sim, game copy, servers folder, scratch app folder and snapshot is deleted; my harnesses, sim and
  scripts are stopped (by PID). `Sim/crashes.log` has nothing new since 08:23.
- Screenshots are in `~/.playwright-mcp/qa4core/`; logs (rcon masked) and scripts in my scratchpad `qa4core/`.

**Disk:** free space fell to 6.3 GB at about 11:55 (other agents' builds). I stopped my second Rust build then (the
setup section uses the first binary with the new UI instead). It was back to 12 GB at the end.

**Counts:** pages and app-wide rows (30): **PASS 25** (Play with friends with a NEEDS USER part), **FAIL 3**, **NEEDS USER 2**.
Modes (21): **PASS 19** (Kreedz with a NEEDS USER part), **FAIL 1**, **NEEDS USER 1**. Mode vote: **PASS**. FAIL findings: **6** (F1-F6, below), plus 13 UX/UI items.

---

## FAILs (most serious first)

### F1. The top strip's map picture is an empty dark box on a first start, and stays empty (App.tsx strip + maps.rs)
**Owner:** the app UI (App.tsx's strip; whoever owns the shared shell), with maps.rs (`thumbnail`).
- The strip draws `<div class="thumb" style="background-image: url(/thumb/<map>.png)">`. That inline style replaces
  the CSS placeholder (the gradient grid in `.thumb`), so when the picture isn't there the box is plain `#1f2318`.
- `/thumb/<map>.png` answers **404 "no picture"** while a GameBanana screenshot lookup for the map is still running,
  even when the map has a radar (`maps.rs` `thumbnail`: `if kind == "radar" && looking { return (None, true) }`).
- The strip never asks again: its URL has no version key (the Maps page uses `?v=<thumb>` and an `onError`
  fallback; the strip has neither), so the box stays empty until something re-renders it with a new map.
- Measured on a cold cache (fresh app folder, fresh browser profile): the strip asked at **1.3 s** and got 404; the
  server had the radar from about **4-5 s** (`x-thumb-kind: radar`, 0.32 s to convert, 2 ms after); in **21 s** the
  strip asked **no second time** and was still empty (`strip-cold-later.png`; the first-pass screenshots of Bots,
  Rules and Maps show the same empty box). A map with no radar and no GameBanana match (a custom map) gets 404
  for good: the box is always empty.
- Same root cause, smaller: every map tile still being looked up logs `Failed to load resource: 404` in the console
  (17 on the first Maps visit, 5 in the Play mode picker). The tiles fall back to the name tile, so only the console
  suffers.
- Repro: empty app cache, open the app with a game running on de_dust2: the strip's picture box is blank and stays
  blank (until the map changes or the page reloads).
- Fix direction: serve the radar while the lookup runs (`no-cache`, as the comment above it already intends for the
  card), give the strip the same `?v=` key the Maps page uses so it re-asks when the state says a better picture
  exists, and draw the picture in an `<img>` over the existing placeholder (or the Maps page's name tile: map prefix
  and name on a neutral tile) instead of replacing the background, so a missing picture never shows an empty box.
  Converting the radar ahead of time (the map list already knows which maps have one) would make it instant.

### F2. Admins › VIP & donations: the Loner-Strike supporters' settings never load ("Couldn't reach the app.")
**Owner:** MCP (admins.rs `api_supporters`, b9fa4f70) with the UI side (Roles.tsx `SupporterVip`, 79962a3d).
- `GET /api/admins/supporters` refuses any request without `X-Panel: 1` (403 `{"error":"forbidden"}`), but the page
  reads it with `getJson`, which only sends `X-Panel` on POSTs. So the whole supporter VIP block (claimed/official,
  honor, tag, scoreboard, reserved slots, join message) shows "Couldn't reach the app." for everyone, every time.
- The route itself works: with the header it answers `{"claimed":false,"honor":false,"joinmsg":true,...}`.
- It's the only GET with this problem: I probed the other 10 GET routes whose handlers mention `from_panel`
  (state, maps, health, modevote, welcome, scrim, share, mods/plugins, app/direct, servers-folder,
  community/stats): all 200 without the header.
- Repro: Admins › VIP & donations: the last block reads "Couldn't reach the app."; the console shows the 403.
- Fix direction: drop the `from_panel` check on the GET (it's a read; the POST keeps it), or send `X-Panel` from
  `getJson`.

### F3. Base Builder: its SQL fails on every map, and a GameBanana bb_ map kills the server (mods/basebuilder)
**Owner:** whoever owns the Base Builder package (`mods/basebuilder`, catalog `mode-basebuilder`).
- Every Base Builder map logs, 5 times: `[basebuilder.amxx] [SQL-LOG] Zapytanie watkowe nie powiodlo sie. Blad:
  near "`name`": syntax error (1)` ("threaded query failed"): its queries use MySQL backtick syntax on the SQLite
  driver, so accounts, missions and shop data are very likely never saved. Seen on bb_dust_iplay and bb_aztec.
- On **bb_dust_iplay** (a GameBanana map, not in Base Builder's own list; the bb_* map preset picks Base Builder for
  it) the map change ends in `Host_Error: ED_ParseEdict: Model '*252' failed to precache because the item count is
  over the 512 limit.` The server then hung (no RCON answer), and the app's strip said "No game is loaded. Start one
  from Play." On a player's own game this drops them to the console. On **bb_aztec** (shipped) the map loads and 5
  bots join, but no round ended or kill happened in 130 s (the build phase; the card says "Best with a friend").
- Repro: Play › Base Builder on bb_dust_iplay (Get more can install it), or `gm_mode basebuilder bb_dust_iplay`.
- Fix direction: SQLite-safe queries (double quotes or none around column names). For the precache limit: check
  which models Base Builder's plugins precache per map. Maybe take bb_ maps without a known-good mark out of the
  automatic preset, or warn when a map's brush models plus the mode's precaches pass the limit.

### F4. Setup keeps any ReGameDLL it finds, so a player can end up without Loner-Strike's bot brain (setup/foundation.rs)
**Owner:** setup (foundation.rs `regamedll`, MCP/P5).
- `regamedll()` keeps the game's `dlls/mp.dll` whenever it isn't in Loner-Strike's own records and mentions
  "ReGameDLL" ("a server that already runs ReGameDLL (maybe a newer one) keeps its own"). A fresh app folder has no
  records, so on a game that already has *any* ReGameDLL (another mod pack's stock build, or an older Loner-Strike
  brain left after reinstalling the app), setup keeps that one. The setup done screen then says only
  "Counter-Strike 1.6: Kept the ReGameDLL that was already there." (`ftue-6-done.png`).
- After that the part is "theirs", so updates never replace it. This is the same trap QA4.md (area A) found for CZ
  ("setup recorded it as its own then"), and it holds for CS 1.6 as well.
- Seen here: the first-time setup on the sim copy (whose mp.dll was an older Loner-Strike brain, 5.30.0.1047-dev)
  kept it instead of installing the pinned 5.30.0.814-ls20 build.
- Fix direction: keep a foreign mp.dll only if it is Loner-Strike's own brain at the pinned version or newer (a
  marker such as `bot_ls_` plus the version). Otherwise back it up and install ours, and say so in plain words
  ("Replaced the server code (ReGameDLL 5.21) with Loner-Strike's, which has the new bots. The old one is backed
  up."). Please confirm the intended behavior.

### F5. Gun Game logs a bad language line on every map (low)
**Owner:** the Gun Game package (`mods/gungame`).
- `Invalid multi-lingual line (file "cstrike\addons\amxmodx\data\lang\gungame.txt" key "// For GunGame 2.13 - last
  updated 10:58 PM 1/2/2013" lang "")` on every Gun Game and Gun Game (Teams) map. AMX Mod X lang files don't take
  `//` comments. Harmless, but it's a plugin error line in every log. Fix: drop or `;`-comment that first line.

### F6. Test isolation: `LS_APP_DIR` doesn't cover settings.json or the cache (app_api.rs, sys.rs; medium for QA)
**Owner:** MCP / P5 (the dev server and test harness conventions).
- `app_api::settings_path()` always uses the real config folder (`dirs::config_dir()/Loner-Strike/settings.json`),
  and `sys::cache_root()`, `setup::fetch::legacy_cache()` and `sys::old_panel_dir()` ignore `LS_APP_DIR` too. The
  Tools dev server (`devserve.rs`) and any test that saves a setting (game folders, the servers folder, phone, MCP,
  text size, notifications), downloads a setup part or adopts the old panel's files reads or **writes the real app
  data**, though the docs say the dev server's data goes to a temp folder.
- I patched these four in my snapshot only, so nothing real was written in this round.
- Fix direction: the same `#[cfg(test)] LS_APP_DIR` check that `sys::app_dir()` already has, in those four places.

---

## Core pages
| Page | Result | Evidence |
|---|---|---|
| Top strip (map, mode, people, score, Live) | **FAIL** (F1) | Mode, people, bots, time left and next map right in every mode; score switches rounds/kills by mode (Gun Game "kills 2:0"); picture box empty on a cold start |
| Update banner | PASS | "Updates waiting: Counter-Strike 1.6 (9 parts). Close Counter-Strike 1.6 to update." with Details (the 9 parts listed, ReGameDLL among them because the sim has a newer brain than the pin), Update all disabled while the game runs, × hides it. UX items 3 and 4 |
| Play | PASS | 7 presets, 26 mode cards each with a bots rating; a card opens the map picker ("Play Classic on de_dust2") → `POST /api/cmd gm_mode classic` → `[Game Mode] Switching to Classic on de_dust2...`; strip showed Classic 15 s later. Offline Play (first-run copy) has a clear "Start a game" form |
| Bots | PASS | ± (`gm_bots count 6` then `5`), per-mode settings, difficulty, style; no console errors |
| Rules | PASS | Restart round (`gm_match restart`), per-mode settings with the mode tag, Voting tab with the mode-vote list |
| Fun | PASS | Gravity Low → `sv_gravity 600`, Normal → `800`, both echoed from the server |
| Add-ons | PASS | Grappling hook on/off → `gm_addon hook 1/0`, "(restart the map to apply)" |
| Rotation & extras | PASS | Rotation lists maps with "not installed" in red. Save stays disabled with "Remove the maps that aren't installed before saving." (by design; UX item 10). The 6 tabs load |
| Players | PASS | 5 bots with health/money/K-D; Heal everyone → `gm_player all heal` |
| Moderation | PASS | Reports, Chat log, Mutes & filter, Bans, Player history (real SteamIDs with roles and Ban in one click) |
| Stats | PASS | Ranks table (bots left out, says so), 5v5 matches tab with the in-game commands |
| Admins (Admins & VIPs, roles) | PASS | Add someone: STEAM_0:1:7654321 as Admin → `users.ini` line 57 `"STEAM_0:1:7654321" "" "bcdefijtu" "ce"`; a bad ID → "That doesn't look like a SteamID." and Save disabled; Remove works. UX item 6 |
| Admins › VIP & donations (incl. supporter settings) | **FAIL** (F2) | VIP perks controls load; the supporters block says "Couldn't reach the app." (403) |
| Welcome page | PASS | Message saved: `POST /api/welcome {"ok":true}` |
| Play with friends | PASS (view) / NEEDS USER (connect) | Options and steps clear (LAN, UPnP, playit.gg, Tailscale, ZeroTier, Radmin, Hamachi); connecting playit or opening a router port needs the person's account and network |
| Find servers | PASS | Internet and LAN lists answer 200, empty with clear text ("No games found on your network..."); my sim on port 27430 isn't on the LAN scan's ports (expected). UX item 5 |
| Maps | PASS | 19 maps, name tiles while pictures load, filters, picker. Console 404s (F1) |
| Mods | PASS | 26 modes and add-ons with install state, "The game is running. Changes load on the next map" |
| Skins & models | PASS | Empty-state text; Get skins & models opens the GameBanana browser on Weapon skins |
| Sounds | PASS | Quake sounds off/on → `POST /api/sounds/addon {"how":"live","ok":true}` twice, switch follows |
| Get more (GameBanana) | PASS | Browse by section and sort; **installed de_dust2_mini into the sim copy only** (`maps-83757-320064` job: Downloading → "Installed de_dust2_mini"; the .bsp is in `Sim/QA4CORE/cstrike/maps`, nothing in the real game) |
| Servers (All servers) | PASS (view) | Empty state explains dedicated servers, links to guides; no server made (it downloads 1 GB and starts a second sim) |
| Settings | PASS | Updates, phone, AI agents, text size, notifications, game, setup, backups, storage, help all load. "Servers folder: Not known until a game is set up" and "Back up › Just your game: the game isn't set up on this computer yet" are this harness (the sim isn't a Steam install, `/api/app/servers-folder` → `mac:false`), not the app |
| Backups / History & restore | PASS | A mode-vote change kept a version (`X-LS-Undo` header, "Taken out of the mode vote" toast). Settings › Backups & defaults listed votemodes.ini; History showed "Today 11:30 AM, Settings: Changed the mode vote list"; Restore default asked first ("...it's removed. The file as it is now is kept in its history"), then `POST /api/safety/defaults` "...is back to Loner-Strike's default." UX item 7 |
| Backup to a file / restore a file | NEEDS USER | Uses the native save/open dialog, and its game list comes from the detected Steam installs (the real game here) |
| Voice commands | NEEDS USER | They live on a dedicated server's Voice tab (`/api/servers/:id/voice`). That needs a new server (a 1 GB download and a second sim), a model download and a microphone. Not in reach under one-sim |
| Radio | PASS | Opens, lists stations (Bassdrive...), no errors |
| Small window (820x600) | PASS (no horizontal page scroll on 19 pages) | Find servers clips its last columns (UX item 5); the chrome takes about 40% of the height (UX item 3) |
| US English | PASS | 255,000 characters of page text, tab contents, titles and aria-labels: no UK spellings. One aria-label in the source says "favourite" (Play.tsx:476, the map picker's star; UX item 11) |
| Console errors | **FAIL** (part of F1) | No page errors or React warnings on any page. The only console errors are the 404s from map pictures still being looked up (and the 403 of F2) |

## Classic modes (one sim, 5 bots, 130-150 s each, switched through the app)
| Mode | Map | Result | Evidence |
|---|---|---|---|
| Classic | de_dust2 | PASS | 5 kills, 2 rounds; earlier run: bomb planted and defused, CT win; no plugin errors |
| Deathmatch | de_dust2 | PASS | 20 kills in 150 s, respawns |
| Free for all | de_inferno | PASS | 21 kills |
| Gun Game | de_dust2 | PASS (F5 warning) | 15 kills, 3 rounds; strip score by kills |
| Gun Game (Teams) | de_aztec | PASS (F5 warning) | 10 kills, 2 rounds |
| Zombie Plague | zm_deko2 | PASS | Round 1 ended Terrorists_Win (zombies) after 79 s with a knife kill. Infections aren't logged, so evidence is light |
| Match (5v5) | de_nuke | PASS | 9 kills, 1 round; bots play while waiting for /ready |
| Knife Arena | de_dust2 | PASS | 5 kills, 2 rounds. **Info:** on ka_monkeybars all bots fell to trigger_hurt within 5 s every round (9 rounds, 0 kills). The nav there was the unshipped raw one (`navpack/raw`, "32 of 32 spawns on a cut-off part" in knowscreen.md), so that's a test setup issue; the shipped pack has no ka_monkeybars nav, so a player's own learned nav decides it |
| Pistols Only | de_dust2 | PASS | 6 kills |
| Deagle Only | de_inferno | PASS | 3 kills |
| AWP Only | awp_india | PASS | 11 kills, 4 rounds |
| Scoutz & Knivez | de_dust2 | PASS | 3 kills with scouts |
| Surf | surf_ski_2 | PASS | 13 kills, 5 rounds, free pistols (nav from `navpack/raw`; the pack has no surf navs) |
| Warcraft 3 | de_dust2 | PASS | 4 kills, 3 rounds; War3FT 3.0.1 cvars, no errors |
| Hide and Seek | hns_floppytown | PASS | A CT knifed a T; 1 long round (hiding) |
| Paintball | de_dust2 | PASS | 3 kills |
| Kreedz (climb) | kz_beginner | PASS / NEEDS USER | Bots leave on purpose ("Solo, no bots"); climbing, the timer and checkpoints need a person |
| Base Builder | bb_aztec, bb_dust_iplay | **FAIL** (F3) | SQL errors every map; Host_Error on bb_dust_iplay |
| SoccerJam | sj_stadium | PASS | 16 kills, 2 rounds |
| Zombie Plague Advanced | zm_deko2 | NEEDS USER | Map and plugins load clean, 5 bots, 1 round start, then nothing in the log for 2 minutes (no kills, no round end). ZP doesn't log infections, so I couldn't tell play from a stall. Worth one look in the client |
| SuperHero | de_dust2 | PASS | 3 kills, 2 rounds |
| **Mode vote** | de_dust2 | PASS | Rules › Start a mode vote → `gm_votemode` → "Mode vote started." toast; 19 s later `[gm_pubserver.amxx] Map vote for Deathmatch: de_dust2 cs_office fy_iceworld de_inferno br_valley`, then "Switching to Deathmatch on cs_office". The vote list edit (Rules › Voting) saved and restored from History (above). Small note: br_valley (a Battle Royale map) was offered for Deathmatch |

Every mode: no `Run time error`, no AMXX plugin failures. Two lines in every mode are from the sim: `[miscstats.amxx]
Could not locate <sound/fvox/flatline.wav>`, because Valve's server download has no Half-Life sounds (a real
dedicated server made by the app would log it too, so it's harmless, but it's noise), and the de_dust2 `.res`
asking for `de_dust2_detail.txt`.

---

## First-time setup (fresh app folder, a sim game copy; UI at 377a0a42)
Times are from page load. Clicks only: about **30 s** from first screen to Install; the install itself took
**about 7 min (418 s)** at a load average of 13-20; then Play. Starting the game from Play launches the player's
client through their Steam in CrossOver, so the first match itself is **NEEDS USER**.

1. **No game found** (`ftue-0-first.png`): clear, with Look again, Choose the game folder and Continue anyway, and the
   Mac/CrossOver hint with a docs link. A fresh start with no game goes straight here (no intro first), which is fine.
2. **Choose the game folder** → `POST /api/app/game-folder` → the intro (`ftue-1-after-choose.png`). The promised
   toast "Found it. Looking again…" wasn't visible in the screenshot taken 3 s later (it may have gone already).
3. **Intro**: "Setup adds bots that play like people, 22 classic community modes, Last Stand and Battle Royale..."
   Clear and short. "Close Counter-Strike before you start." is good.
4. **Which game?** (`ftue-2-game.png`): CS 1.6 "Ready to set up", CZ "Not installed in this Steam", plus a CrossOver
   note. It doesn't show *which* folder each game is in, which matters once a chosen folder sits beside the Steam
   one (UX item 9).
5. **What to install** (`ftue-3-what.png`): Everything (15) is the default; every mode has a one-line description and
   a bots rating. "The basics are always included: Classic, Deathmatch, ... and Surf" doesn't mention Last Stand and
   Battle Royale, which the intro just promised (UX item 9).
6. **Review** (`ftue-4-review.png`): plain and complete. It says "About 100 MB. Usually 2 to 5 minutes": it took 7
   here, about 2.5 of them on "Getting the fun and popular maps" (24 maps, one by one).
7. **Install** (`ftue-5-installing.png`): steps tick off with a 99% bar and "Getting the map fy_snow (11 of 24)". The
   radio offer is a nice touch. It stood at 99% for the whole map download (minutes), which reads as stuck
   (UX item 8).
8. **You're set** (`ftue-6-done.png`): F3/F4, maps, Make, Phone tips, and "Counter-Strike 1.6: Kept the ReGameDLL
   that was already there." (F4: a player can't act on this line, and it hides that they won't get the new bots).
9. **Start playing** → Play (`ftue-7-play.png`): "Start a game" with mode, map, time limit, friendly fire, bots,
   difficulty and style, and one big "Play Classic on de_dust2". Clear. The sidebar opens with the Tools (Beta)
   group expanded (17 items) on a first run (UX item 12).
No dead ends were found, apart from the Mac-native case in UX item 13.

---

## UX/UI (notable items only, most important first)
1. **Empty map picture in the top strip** (F1; `strip-cold-later.png`, `wide-Bots.png`). Suggestion: keep the
   placeholder (the Maps page's prefix-and-name tile) under the picture, and fade the picture in when it loads.
2. **Supporter VIP block says "Couldn't reach the app."** (F2; `admins-vip.png`). Beyond the fix, a block that can't
   load should say what to try, not blame the connection.
3. **Little room left in a small window** (`small-Play.png`, `small-Players.png`). At 820x600 the strip, the "Your
   game" line, the quick-action bar and the update banner take about 250 of 600 px, and the strip's text cuts to
   "Classic · ...". Suggestion: at narrow heights fold the update banner into one line (or a badge on Mods,
   which already shows "9"), and let the quick bar wrap into a "More" menu.
4. **"Update all" looks enabled when it isn't** (`wide-Rules.png`): a gold primary button that does nothing while
   the game runs. The reason is only in the sentence. Suggestion: muted style while disabled, with a tooltip "Close
   Counter-Strike 1.6 first".
5. **Find servers at 820 px clips Map and Latency** (`small-Find_servers.png`) with no way to scroll to them.
   Suggestion: drop Bots/Game below about 900 px, or let the table scroll sideways.
6. **Admins › Add someone: the Name field doesn't line up** (`admins-add.png`). It's taller and starts lower than
   SteamID. Suggestion: the same input height and label row as SteamID.
7. **Settings › Backups numbers and words** (`history-sheet.png`): "The oldest go by themselves past 1907.3 MB"
   (the 2,000,000,000-byte cap): say "past 2 GB". The single-file confirm button says "Restore defaults" (plural):
   "Restore default".
8. **Setup sits at 99% for minutes** (`ftue-5-installing.png`) while 24 maps download, and the estimate says 2-5
   minutes. Suggestion: give the maps their own share of the bar (or a separate "Maps 11 of 24" bar), and say "about
   5-10 minutes" or base it on the measured speed.
9. **Setup copy gaps.** "The basics are always included" should name Last Stand and Battle Royale (the intro promises
   them). Which game? should show each game's folder when there's more than one.
10. **Map rotation can't be saved while the stock list has maps that aren't installed** (`rotation-maps.png`): the
    person has to remove them one by one. Suggestion: a "Remove the ones that aren't installed" button next to the
    message.
11. **US English in an aria-label:** Play.tsx:476 `aria-label="favourite"`, which should be "favorite".
12. **First run opens with Tools (Beta) expanded** (`ftue-7-play.png`): 17 more items under the core sections.
    Suggestion: collapsed until opened once.
13. **No game found, Mac-native case:** with the Mac Half-Life installed the page hides "Choose the game folder", but
    the note under the buttons still says "Choose the Half-Life folder", a dead end for someone whose CrossOver game
    is in an unusual bottle. Suggestion: keep the Choose button in that case too, or change the note.
Smaller: the setup card shows an empty scrollbar track on its right (`ftue-1-after-choose.png`). The Bots page shows
"Personality 100 Normal" with no unit, which is hard to read.

## Not covered / for the user
- Starting the game from Play and playing a first match (the player's own client).
- Voice commands (a dedicated server, a model download and a microphone).
- Play with friends' connect steps (playit.gg account, router UPnP), and Find servers with live internet servers.
- Backup to a file / restore from a file (native dialogs).
- Kreedz climbing and Zombie Plague Advanced's rounds, by hand in the client.
- "Teach bots maps" and making a dedicated server: each starts another server, and this pass kept to one sim.

---

## Fixed (QA4-FIX, 2026-11-14)
Verified in one sim (`Sim/QA4FIX`, port 27461, `-insecure`, ls-next3 **6c2f748** = run 43638373487, mp.dll
5.30.0.1308-dev; plugins compiled fresh from the working tree on tools 564fc781 + these changes).
- **F3 Base Builder SQL.** The "backtick" error was not the backticks (SQLite takes them): Pawn joins a `\`-continued
  string without the next line's leading spaces, so the play-time save became `... CURRENT_TIMESTAMPWHERE \`name\` ...`
  and failed for every player on every map change. Fixed in `sqlFile.inl`; also `NOW()` (no such function in SQLite) in
  the clan-create query in `clans.inl` (and its buffer, 128 → 256). Sim: no `[SQL-LOG]` line on bb_aztec, bb_street,
  bb_inferno_iplay, bb_castle_final; `basebuilder.sq3` gets a row per player with `timePlay` and `lastlogin` written at
  each map change (e.g. `~Dice~ 26 s`). New check: `python3 scripts/mod-files-check.py` prepares every Base Builder
  query on SQLite and lints the AMXX language files (it flags all three old bugs on the previous HEAD).
- **F3 bb_dust_iplay hang.** Base Builder with Loner-Strike's plugins uses **260** model slots before a map's brush
  pieces (measured; Loner-Strike's own plugins add ~2, the rest is the game and Base Builder's ~95 models, all used by
  its classes, so nothing worth cutting). bb_dust_iplay has 263 pieces: 523 > 511. The shipped maps fit: bb_aztec 188,
  bb_street 226, bb_castle_final 234, bb_inferno_iplay 238 (13 spare); all four loaded in the sim. gamemodes.amxx now
  reads each map's piece count from its .bsp and learns each mode's own count every time it plays
  (`data/gamemodes/modelslots.ini`, Base Builder defaults to 264 before that), and:
  the bb_* preset skips a map that doesn't fit ("Map preset: bb_dust_iplay has too many models for Base Builder (526
  of 511 slots) - staying in Classic"); `gm_mode`/menu/`gm_map` refuse it with a plain message; mode-vote/rotation
  map lists leave it out; and as a last net (the app starting a game on it, a raw `changelevel`) the mode's plugins are
  paused while the map loads and the server falls back to the pre-preset mode or Classic. All three seen in the sim
  (the raw `changelevel bb_dust_iplay` from Base Builder ended on bb_dust_iplay in Classic with a bot, no Host_Error).
  `gm_mapfit <mode> <map>` prints the numbers. Side note for gm_content's owner: content_status.json's "models"
  doesn't include the map's brush pieces (it said 260 on bb_aztec), though its comment says it does.
- **F5 Gun Game lang.** The `//` first line of `gungame.txt` removed. Sim: no "Invalid multi-lingual line" on Gun Game.
- **Not verified:** a returning player's saved stats seen in game (bots get new names each map here; the load query
  runs without errors and every save needs a loaded row). One unexplained stall in the first run (bb_aztec → bb_street
  map change during nav learning, 0% CPU, no error line); not reproduced in the next three map changes on those maps.

---

## Fixed (P5, 2026-11-14)
F3 (Base Builder) and F5 (Gun Game) went to another agent. The contrast items went to Parity A's app-wide pass.
- **F1, the strip's map picture.** `/thumb/<map>.png?now=1` returns the radar right away, even while a GameBanana
  screenshot is still being looked up (`no-cache`). Every answer, including a 404, now carries `x-thumb-looking`. The
  strip (`StripThumb`, App.tsx) draws the Maps page's name tile (prefix and name), and the picture fades in over it once
  it has loaded (an `<img>`, no inline background). While the app says it's still looking, the strip asks again every
  4 s for up to a minute, with a new `?v=`. A custom map with no picture keeps its name tile and isn't asked about again.
- **F2, the supporters' VIP settings.** Every read from the pages now sends `X-Panel`: getJson, getJsonAt, the tools'
  get/tryGet/fetchOk, and the 14 direct fetches in the editor, navfix and viewer. `scripts/test-panel-gets.mjs`
  (`npm run test:panel`) reads the Rust GET routes whose handlers check from_panel (4 today), and fails on any UI fetch
  to the app without X-Panel, or on any panel-only GET used as an `src`, `href`, `url()` or EventSource. When the block
  doesn't load, it now says what to try and offers a Retry.
- **F4, an existing ReGameDLL.** `foundation::server_code`:
  - Loner-Strike's brain of any version (its `bot_ls_` cvars) is replaced with the pinned one, as an update. The old
    one is backed up and a note says so.
  - Another ReGameDLL is kept. The done screen says "<game>: the bots won't have Loner-Strike's brain." and offers
    **Use Loner-Strike's** (`POST /api/setup/use-our-brain`, which backs up the old one; MCP asks first).
  - Valve's mp.dll, or one setup wrote itself, is replaced. That includes the stock 5.30.0.814 on CZ from QA4-A's trap.
  - The same rule applies to both games.
- **F6, test isolation.** In test builds every app path is under `LS_APP_DIR`, or a scratch folder in the temp folder
  when it's unset. That covers app_dir, settings.json (now `app_dir()/settings.json`), the cache, setup's downloads,
  the big folders, the window's data, the old panel's folder, the old installer's cache (none), and servers by default.
  The first time a test build asks for an app path it checks `sys::app_paths()` and panics if any of them would be
  outside. The Tools dev server checks at start.
- **First-run time (UX 8).** The fun maps download 4 at a time ("Downloading maps (n of 24)"). The step's share of the
  bar is sized by the maps still to download (1 per map), so the bar no longer sits at 99% while they come in. The
  review's estimate now counts the maps: basics "2 to 4 minutes", everything "4 to 8 minutes".
- **UX items:**
  - 3: In a narrow window, Announce and Record demo fold into a More menu. Under 700 px of height the update bar is
    one line.
  - 4: A disabled main button looks switched off, and Update all's tooltip says "Close Counter-Strike 1.6 first".
  - 5: Find servers drops Game and Bots in a narrow window (a container query), so Map and Latency stay in view.
  - 6: The Admins Name field lines up with SteamID.
  - 7: The backup cap reads "past 2 GB", and the confirm button for a single file says "Restore default".
  - 9: The basics name Match, Last Stand and Battle Royale. When there's more than one install, Which game? shows
    each game's folder.
  - 10: A "Remove the N that aren't installed" button.
  - 11: "favorite".
  - 12: Tools (Beta) starts folded until it has been opened once.
  - 13: With the Mac Half-Life, "Choose the game folder" stays, with a note that fits.
  - 1 and 2: covered by F1 and F2 above.
- **Not done:** the smaller items (the setup card's empty scrollbar track, the Bots page's "Personality 100 Normal").
- **Tests:**
  - Rust: `which_server_code_is_kept` and `a_kept_regamedll_reaches_the_done_screen` (foundation),
    `test_builds_keep_every_app_path_in_the_test_folder` (sys), the radar-now and looking headers in
    panel_api `endpoints`, and the funmaps share in `plan_checks_requests`.
  - Node: `test-panel-gets.mjs`.
  - Playwright: `tests/qa4-core.spec.ts`, 6 tests passing in Chromium and WebKit (strip picture, custom map tile,
    X-Panel, done screen and Use Loner-Strike's, Tools fold, small window).
