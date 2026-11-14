# QA round 4, area F and the app-wide and site rows (QA agent "F", 2026-11-14)

Builds: `tools` 314c7f47 (snapshot via `git archive`, debug `.app` built from it), plugins compiled fresh from the same
snapshot (19 of 19 OK), brain ls-next3 285b895 (run 43604814272; its head matched `git ls-remote`; `mode revive`
found in each sim's mp.dll), engine ReHLDS 3.15.0.896-ls8 (the app's pin, build 4599). The site: `site/` at HEAD
(identical to the working tree), served on 127.0.0.1. Windows-only rows (area H) aren't here.

How: the built app ran with a throwaway HOME, pointed at a sim game copy (`Sim/QA4F`, made from APFS clones of the
Valve server download plus `hl.exe`) and a sim servers folder; the UI was driven with my own Playwright (Chromium and
WebKit) against the app's own listener, and through the API. The real game and the real app data were never touched.
All sims, the sim game copies, the compile folder and the throwaway profiles are deleted; my LaunchServices
registrations are removed; the Discord test webhook is deleted.

**Caveat from the coordinator (received at the end):** the built app ran with a throwaway HOME (its own copy of
CrossOver.conf) against the shared Steam bottle between about 07:58 and 09:40, and the shared wineserver crashed 12
times in that period. Rows that rely on a hang or freeze are marked "maybe the wineserver" below, and need a re-run
once the MCP agent has settled the cause. The CrossOver.conf copies are deleted. Rows that rest on files, hashes, API
answers or logs stand on their own.

Counts: **PASS 34 rows, FAIL 10 findings (in 6 rows), NEEDS USER 9 rows**, plus the observations at the end that need a re-run.

---

## FAILs (most serious first)

### F1. A local server crash after the first one is never counted or posted (crashes.rs; owner MCP, with Y)
The brain appends every crash to the same `cstrike/ls_crash.log`. The app treats one file as one crash with the id
`ls_crash`, so:
- a second crash replaces the first in the list. The count stays at 1 in 24 h, and the record mixes parts of both:
  the new address (0x73ff9880) with the first crash's marks ("21.82 frame 0…");
- the Discord alert never fires for the second crash, because `ls_crash` is already in `seen`
  (crash-alerts.json). The watcher ran at 16:51 and 16:56 after a crash at 16:46 and posted nothing.

Repro: a local server with Discord alerts on. Force a crash (any plugin fault: I used `set_pdata_int` with a bad
offset). The crash shows, "1 crash in 24 h". The app restarts the server. Force a second crash. `ls_crash.log` now
holds two entries (`grep -n exception`), but Health still says 1 and Discord gets nothing.
Proof the posting path itself works: renaming the file to `ls_crash.20261114-0846.log` (as a rotation would) posted
one message 4.5 minutes later: `**QA4C crash test** crashed at 2026-11-14 16:46 UTC: 0xc0000005 in amxmodx_mm.dll.
The details are in Loner-Strike (Servers, Health).` It had no mentions (mentions [], roles [], everyone false).
Fix: split `ls_crash.log` by its timestamp lines into one record each (id = file + time), or have the brain rotate the
file at each crash.

### F2. Local Mac/Windows servers can't turn on Discord crash alerts at all (owner MCP: CrashPanel.tsx, webstatus.rs)
Health › Crashes says "Link the server's Discord status channel first (Website and Discord)". A local server has no
Website and Discord tab, and the route refuses it: `POST /api/servers/:id/webstatus` gives "Website status is for
servers on a Linux machine." So for local servers the switch is a dead end, although crashes.rs says the app posts
for Windows/Mac servers. I tested the posting by writing the hook file by hand (`secrets/discord-live-<id>.url`).
Turning it on then asked nothing in the app (PASS), and an agent's `app_call` to `/crashes/alerts` showed the native
"An AI agent is asking" dialog (PASS).
Small extra: after the switch is clicked, the POST answers with stale `alerts.discord:false` (from the 2-minute cache),
so the switch shows off until the page reloads, although it's on (`?fresh=1` shows true).

### F3. "Update the server's Loner-Strike files (Health, Updates)" points to a button that isn't there (owner MCP)
With the official Ham Sandwich in place, Health shows the warning (PASS) and the server card shows the "Crash-prone
module" badge (PASS). But Updates says "Its Loner-Strike plugins and bot brain are up to date" and offers only
"Update Valve's server files". The only button that puts ours back is Repair › **Reinstall** (it does: hash
d1ebbf71…). Starting the server also puts it back (hamguard). After Reinstall the warning stays for up to 2 minutes,
until Check again.
Repro: copy the official `hamsandwich_amxx.dll` (AMXX 1.10.0-git5487 base, sha256 5f6e5b4e…) over a local server's
module, then open Health.
Fix: offer "Put the fixed one back" next to the warning, or change the text to "Reinstall (Health › Repair), or just
start the server".

### F4. ReHLDS ls8 breaks relocalizebugfix on every server (owner Y)
On the ls8 engine (3.15.0.922-dev, build 4599), `meta list` shows `[ 2] ReLocalizeBug F  fail … Start Never`. With
`log on`: `RehldsApi_Init : REHLDS Api major version mismatch; expected 2, real 3` and `Failed to attach plugin
'ReLocalizeBug Fix'`. Setup installs relocalizebugfix v2.8 everywhere, and prod runs ls8 since today. AMXX also logs
`GameConfig CRC mismatch … library "engine"` for this engine build. ReAPI isn't installed in these servers, so I
couldn't check it, but it uses the same API check.
Repro: make a local server (the app installs ls8), start it, send `meta list`.

### F5. After an update from 0.1.2, a server made in 0.1.2 can't be controlled, then fails to restart (owner: servers.rs and game.rs; Mac)
A 0.1.2-made server has no `addons/amxmodx/data/gamemodes/panel_rcon.txt`. In 1.0, after the update:
1. Start it. The console says "The server isn't set up yet." (`Entry::rcon()` needs the file).
2. Open its page (`/api/state` with X-Target). `ensure_password_file` writes a new password file, but the running
   server never read it, so RCON fails.
3. 90 s later: "server friday-test hasn't answered for 90 s: ending it so it starts again". **The old hlds.exe keeps
   running** and holds UDP 27016, so the restart fails with "Port 27016 is in use by another program on this
   computer" and the state is `crashed`.

After I ended the leftover by hand, a fresh start worked (the plugin read the file). Fix: write `panel_rcon.txt` for
the app's own servers before the first start in 1.0 (a migration), and see F6.
(The whole update path is under "Update from 0.1.2, Mac" below; everything else there passed.)

### F6. Ending a hung server (or the hidden learner) on a Mac leaves hlds.exe running (owner: servers.rs, navs.rs)
Seen five times: the "hasn't answered for 90 s" restart, **Stop** on a hung server, and **Stop** on the nav learner
all ended the `winewrapper.exe` but not the wine-side `hlds.exe` (still listed by `pgrep`, still on its port).
`kill -TERM <hlds pid>` ended it every time. A healthy server's Stop ends cleanly (rcon quit). After a leftover, the
next start fails with "Port … is in use" (state crashed) or runs as a duplicate.
Repro: a local server that doesn't answer RCON (F5 makes one). Wait for the 90 s restart and run
`pgrep -fl 'Servers\\<id>\\hlds'`: the old pid is still there.
(Maybe the wineserver: some hangs that started this may come from the wineserver crashes. But the leftover after
F5's restart had nothing to do with a hang: the server just had a different password.)

### F7. The servers' shared Valve download (.base) on this Mac breaks when the app updates it, and the app still says it worked (owner: servers.rs `download_hlds`)
Every server is made from `Servers/.base`, and `download_hlds` always runs `app_update 90 validate`. On a copy of
this Mac's real `Loner-Strike/Servers/.base`, the 1.0 app's step **and** 0.1.2's step each removed apps 70 and 10:
`steamapps/appmanifest_70.acf` and `appmanifest_10.acf` were gone, `valve/` went from 4248 files to 7, `cstrike/`
from 1157 to 9, and the log said "Success! App '90' fully installed." The app checks only `hlds.exe` and
`cstrike/liblist.gam`, so it went on. The servers made from it stop at "Console initialized." forever (no
`valve/gfx.wad`, `delta.lst`, `hl.dll`). The app then "ends" them every 90 s and hits F6.
- A **fresh** download into an empty folder was complete (4248/1157, apps 70 and 10 present). Re-running the step on
  that copy, with the same steamcmd, a different steamcmd, or a clone at a new path, kept everything. So it is tied
  to the state of this Mac's `.base` (made Nov 6, updated Nov 12). The manifests are identical, so I couldn't find
  the trigger.
- **Risk for the user:** the next server the user's real app makes (or the hidden learner's first download for a new
  game) runs this step on the real `.base`, and every new server would then hang.
Fix: after the download, check `valve/gfx.wad`, `valve/delta.lst` and `<game>/maps/*.bsp` (fail with a plain
message, or re-download into a fresh folder), and don't treat a stripped base as success.

### F8. The nav learner doesn't notice its hidden server died (owner: navs.rs)
CZ, `cs_havana_cz`, relearn. The hidden server logged `FATAL ERROR (shutting down): Unable to initialize Steam.` at
09:13 and then sat at 0 % CPU. The job kept saying "A bot is walking the map (13 min so far)", and would have waited
until the 45-minute limit. Stop marked the job done (no error, nothing learned) and left the hlds.exe running (F6).
Watch the learn log for `FATAL ERROR` and treat it as a failure. (The Steam init failure itself was most likely my
throwaway-HOME setup: the same command from a normal shell started fine. See CZ below.)

### F9. Crash records name the wrong module (owner: MCP crashes.rs)
The brain's log says `where: 0x771a9880 fakemeta_amxx.dll+0x9880`, but the record (and Copy, and the Discord
message) says "in amxmodx_mm.dll", which is the first return address. Use the `where:` line for `module`.
Also: local records have no **map** (the row asks for time, map, fault and stack) and `lastLines` is empty. That is
expected until the brain writes it on Windows/Mac (Linux's lscrashinfo has it), but the row can't pass as written.

### F10. The monthly Support Pack's Polar description is wrong for a subscription (owner: the user, in Polar; Site.md)
Both checkout links open the right products: "Loner-Strike Support Pack", $5 suggested, $3 minimum; and "Loner-Strike
Support Pack (monthly)", $5/mo. But the monthly one also says **"Your Supporter perks are yours for good."** (that
holds only for the one-time pack; Terms says a lapsed or refunded purchase ends them). Both descriptions say
"Supporter Pack" where the site and Terms say "Support Pack".

---

## Every row

### Crash tracking
| Row | Result | Evidence |
|---|---|---|
| Local: a crash shows in Health › Crashes within 2 min with time, fault, stack | **FAIL** (F1, F9) | Forced fault 16:40:55; `/crashes` had it at 16:41:11; "2026-11-14 16:40 UTC · 0xc0000005 in amxmodx_mm.dll"; stack and marks present; no map; a second crash isn't counted |
| Local: Copy | PASS | the clipboard holds "QA4C crash test: crash at … stack: … last marks: …" |
| Local: card "1 crash in 24 h" | PASS | All servers card: "Stopped · 1 crash in 24 h" |
| Local: debugger/stop fault listed, not counted | PASS | a real 0xfff4fadc log from LSPLAY placed beside it: listed with "This fault happens when a debugger attaches or the server is being stopped: not counted as a crash."; count stays 1 |
| Module check flags an official Ham Sandwich | PASS | the official dll (sha 5f6e5b4e…): `hamsandwich: "not the fixed one"`, warning on Health, "Crash-prone module" badge on the card, `/api/crashes/summary` warnings 1 |
| "Update files" puts the fixed one back | **FAIL** (F3) | there's no such button when up to date; Reinstall and server start do restore it |
| Discord via the test channel | **FAIL** (F1, F2) | in-app switch asks nothing (PASS); agent's call asks (PASS); one plain message, no mentions (PASS, after a rename); old crashes not posted (PASS: `seen` seeded); a second real crash never posted; local servers can't link a channel |
| Remote (Linux) crash records | NEEDS USER | no Linux test machine (prod is off limits) |
| Crash format with Y on Windows/Mac | NEEDS USER | Y's capture is Linux-only so far; local servers still read `ls_crash*.log` |

### Ham Sandwich guard
| Row | Result | Evidence |
|---|---|---|
| On Play | PASS | official dll in the sim client, Play → `{"note":"Restored Loner-Strike's fixed Ham Sandwich.","ok":true}`, toast shown, `hamsandwich_amxx.dll.replaced-20261114` kept (sha 5f6e…), ours back (d1eb…), errors.log line |
| On local server start | PASS | same on QA4C's start: replaced, kept once, quiet (no toast), errors.log "starting QA4C crash test: Restored…" |
| Setup on CrossOver installs the fixed one | PASS | both games and the server: d1ebbf71… (resources/windows/hamsandwich/README.md still says "CrossOver installs on the Mac keep the official module": out of date) |

### Setup and repair (sim game copy, throwaway HOME)
| Row | Result | Evidence |
|---|---|---|
| Setup end to end | PASS | Set it up › both games › Only the basics › Install: every step done in about 3 min; brain marker in both mp.dll |
| Repair / Update all | PASS | the parts marked outdated in both records → Update all did CS 1.6 then CZ ("1 of 2", "2 of 2"), clean in 56 s, bar gone, no question asked twice |
| Reinstall a server | PASS | Health › Repair › Reinstall: confirm sheet, job done, mods back |
| Nav pack | PASS | the Maps page after setup: 76 maps, all with navs (pack + game), no dojo or jigsaw2 |
| CZ: the hidden server uses our brain | PASS | the learner's console: `ReGameDLL version: 5.30.0.1047-dev`; `lonerstrike_mapknow` in its czero mp.dll; CZ's own BotProfile.db there |
| CZ: learning completes, CZ bots use our behavior | NEEDS USER (re-run) | under my throwaway HOME the CZ hidden server failed Steam init (appid 80); the same command from a normal shell started fine. Re-run on a normal profile once the wineserver issue is settled. See F8 |

### Updates
| Row | Result | Evidence |
|---|---|---|
| Update banner: both games, Update all, Details | PASS | "Updates waiting: Counter-Strike 1.6 (2 parts) and Condition Zero (2 parts)…" with Details (per game: own plugins, bot brain) and Update all |
| App update and brain update in one banner | NEEDS USER | an app update can't be offered without a signed feed (P5's limit too). From the code: the app update is a startup dialog plus Settings, the files bar comes after the restart |
| Update from 0.1.2, Mac (P5's method, throwaway profile) | **FAIL** (F5) | 0.1.2 (the R2 DMG) set up a sim copy and made "Friday Test" (join password, 20 players, admin STEAM_0:1:12345678), with prefs updates off, keep servers on, joins off. The 1.0 app started on the same profile: settings.json and servers.json **byte for byte the same**, admin kept, game shown as set up with 2 parts per game waiting, the server listed. Play put ours back (the official dll placed after setup) and the server start did too. The server then hit F5. Records, backups and recent maps weren't tried (no match could be played) |
| Update from 0.1.2, Windows | NEEDS USER | area H |

### Engine (ls8) and voice
| Row | Result | Evidence |
|---|---|---|
| RCON password masked, app RCON features still work | PASS | local server on ls8: status/stats, `amxx plugins`, gm_bots, gm_announce, gm_set mp_friendlyfire, gm_map de_inferno, the app's gm_status polls, bad password. Since start, 26 `rcon … "***"` lines and 0 with the password; the password string appears 0 times in loner-strike-server.log. qconsole.log still has unmasked lines from an earlier run on the older engine (it's appended, never rotated) |
| ls8 with relocalizebugfix | **FAIL** (F4) | `meta list`: ReLocalizeBug fail |
| Voice on its file channel | PASS | ls-voice/ls-intent built from the snapshot; Voice on → "the server answers; voice feed 28010" through `data/gamemodes/lsvoice` (status.txt `feed 28010 / channel 1`, `.cmd` files); loner-strike-voice.log has 0 "rcon" lines and 0 password hits; Voice off stopped both processes |
| Voice: bots answer aloud, feed, Who's here | NEEDS USER | needs a mic |

### MCP
| Row | Result | Evidence |
|---|---|---|
| Bridge connects, tool count | PASS | `--mcp-stdio`: 175 tools (79 read-only) |
| Read-only with Allow changes off | PASS | `server_stop` and `server_set_bots`: "Changes are off. The person can turn on…" |
| Allow changes asks first in the app | PASS | Settings: "Let AI agents change things?…" sheet, then on |
| Risky calls ask or refuse | PASS | the `/crashes/alerts` POST showed "An AI agent is asking … Don't allow / Allow" (screenshot, 08:53); `console_send quit` and `exec server.cfg` waited for the person; refused at once: `rcon_password abc` ("The console password is the person's to set."), `/api/app/remove-everything` ("…off limits to agents"), `/rcon-password/show` ("Console passwords … are off limits") |
| "Don't allow" leaves things as they were | NEEDS USER | the Mac's screen was locked (loginwindow in front), so the native alert couldn't be clicked |
| Activity log | PASS | `/api/mcp` log lists each call with done/refused |
| Copy for Claude Code | PASS | the copied command is clean ASCII. Small: the on-screen command shows the app path with a U+2060 word joiner ("Loner-⁠Strike.app"), so selecting the text by hand would break the path |

### Servers page
| Row | Result | Evidence |
|---|---|---|
| Two servers on one remote | NEEDS USER | no Linux test machine |
| Mode reload without a map change | PASS (server side) | a new modes.ini line was "Unknown mode", `gm_modes_reload` → "Modes reloaded: 15", then switched |
| 128-mode limit | PASS | 140 lines: "modes.ini lists 140 modes; the Game Mode Menu holds 128, so the last 12 are left out. Remove some modes." The 113th extra worked, the 114th was "Unknown mode". Small: the app's `/api/servers/:id/modes` still lists all 140 (the menu holds 128) |

### Fonts, site, deep link
| Row | Result | Evidence |
|---|---|---|
| Offline fonts | PASS | every non-local request blocked, in Chromium and WebKit: Barlow Semi Condensed 400–700 and Big Shoulders Stencil load from the app, 0 failed, 0 external requests, no clipped labels; no Google Fonts URL in dist |
| Homepage, titles, meta | PASS | 7 h2 sections; title and description as in Site.md; the release banner names 1.0.0 (deliberate) |
| Links | PASS | 7 pages, 0 broken local links or anchors; 36 external: all 200 except `github.com/…/blob/main/docs/LONER-INC.md` (404 until tools merges to main) |
| #get, direct R2 downloads | PASS | from an origin R2 allows, the buttons switch to `dl.lonerstrike.com/download/…`; downloaded both: `Loner-Strike_0.1.2_x64-setup.exe` 11,769,864 B (NSIS PE) and `Loner-Strike_0.1.2_universal.dmg` 24,793,005 B, names from Content-Disposition, sha256 equal to the GitHub v0.1.2 assets; the DMG verifies, mounts, holds Loner-Strike.app 0.1.2, Team PC4KPNUNP8. (They're 0.1.2 until the release; R2's CORS also allows the dev origin http://127.0.0.1:8088) |
| Support Pack section and checkout links | **FAIL** (F10) | both open the right Polar products (not bought) |
| thanks.html | PASS | Discord on the receipt, the /link steps, R2 download buttons, docs and GitHub links, noindex. Landing there after a real purchase: done live by the user earlier (QA4 G) |
| Terms, Privacy | PASS | "The Support Pack" paragraph present, "Last updated November 14, 2026" |
| Docs | PASS | Windows first ("Install" before "Mac (CrossOver)") |
| Made by players, empty state | PASS | live index has 0 items: "Nothing shared yet. Make a map or a game mode in Loner-Strike, share it from the Community tab, and it shows up here. How sharing works"; the filter bar is hidden |
| Made by players, with entries | PASS | `?community=` sample (local preview only): unlisted and revoked entries left out; cards show the GameBanana, Open in Loner-Strike (`loner-strike://community/mod/<gb>`) and Get buttons, risk badges, the bots line; grid centered (150/150 px at 1440); support cards centered (150/150, 16/16 at 1024 and 390) |
| Phone widths | PASS | 375 and 414 px: no sideways scroll on any page (Chromium). At 320 in desktop Chromium the homepage scrolls 8 px (the community cards' min width 297 + 16 gutter against a 305 px viewport with a scrollbar); real phones have overlay scrollbars, so 320 fits |
| Open in Loner-Strike (deep link) | PASS for a running app; cold start NEEDS USER | `open -a <debug bundle> loner-strike://community/mod/656789`: taken by the page within 0.3 s (`/api/app/deep-link` already empty), one window; `?install=1` and `settings/mod/5` → "deep link ignored". A cold start by link has to go through LaunchServices (no throwaway HOME possible), and the browser prompt needs a person |

## Observations that need a re-run (maybe the wineserver)
- An app-run ls8 server hung once right after the bot-style map reload (an `Assertion Failed: Failed to load
  "SDL3.dll"` line, no Rcon answer). Another one froze about 6 minutes after Voice was turned on. Neither happened
  again in 3 later starts (one with voice on for 5 min). Both fall inside the wineserver-crash window.
- The CZ hidden server: `Unable to initialize Steam` (appid 80) under the throwaway HOME; fine from a normal shell.
- Environment, not product: steamcmd (Valve's) writes `~/Library/LaunchAgents/com.valvesoftware.steamclean.plist`
  into the HOME it runs under. The app's server download will do the same for users (a background item the app
  doesn't mention).
- LaunchServices had 3+ registered bundles for `loner-strike:` (the user's /Applications app, old DMG volumes, test
  bundles). Another agent's `open loner-strike://…` landed in my test app. Agents testing deep links should
  `lsregister -u` their bundles afterwards (I did mine).
