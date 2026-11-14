# QA round 4 — checklist (DRAFT, not run; the user starts it)

Drafted by MCP, 2026-11-14, from docs/notes/QA.md (rounds 1-3) and every agent's notes. It covers what landed since
round 3 (HEAD 1ceff77 then) plus round 3's open items. Same method as before: real routes, real SDHLT through
CrossOver, HLDS sims cloned from `Sim/LSPLAY`, the real client where it says so, no product code changed by QA, bugs
filed under "Requests from QA, round 4" in the owner's notes (editor → P5.md, compile → P4.md, Studio → X.md and
X2.md, Last Stand / components → W1.md, Battle Royale → W3.md, brain/bots → W2.md, DLL/creatures → Y.md, site → Site.md,
legal → Legal.md, crash tracking and MCP → MCP.md).

Before starting: check what's still in flight in the owners' notes. Items marked **(when it lands)** are skipped if
they haven't landed, and noted as "not in this build".

## Rules for every area
- Sims: at most 4 per QA agent, at most 8 on the Mac in all (`pgrep -fl 'Sim\\.*hlds.exe' | wc -l`), only 2 while
  that agent compiles; the load average under 20 before a heavy step; compiles `-threads 6` under `nice -n 15`.
- Stop only your own servers (by PID or your own folder in the pattern). Never attach a debugger to a CrossOver hlds
  (Y.md: the 0xfff4fadc crash). After each run: `~/loner-strike-sim/creatures/crashcheck.sh since <mark>`.
- Delete each sim copy when its area is done. Tests never write into the real game (LS_TEST_GAME → a sim copy).
- A row passes only on evidence: a log line, a route's answer, a screenshot, or a Studio debug event. Write which.

## Areas, sims and order
| # | Area | Sims (folder, port) | Wave |
|---|---|---|---|
| A | Map editor (perf, selection, Checks, Budget, leaks, I/O, new tools) | QA4E 27400 (playtest) | 1 |
| B | Game Mode Studio (Compile, Test run, palette, nodes, security, examples) | QA4S 27402 (playtest/debugger); Test run's own hidden server | 1 |
| C | Last Stand and creatures | QA4L 27404 (16 bots, ls_outpost) | 1 |
| F | Platform: crash tracking, MCP live, site and legal, setup | QA4C 27410 (crash tests); the Linux test machine | 1 |
| D | Battle Royale (squads, downed, revives) | QA4B 27406 (squads), QA4B2 27407 (duos/solo) | 2 |
| E | Deathrun maps and the mode vote | QA4D 27408 | 2 |
| G | Community: share and import | QA4G 27412 (mode installs and play); the user's test GameBanana page | 2 |
| H | Windows (real Windows 10 and 11 PCs; see docs/notes/Windows.md) | local servers on the PC | any (a separate machine) |

Wave 1 runs A, B, C and F side by side (5 sims at most, 6 while a Test run is going). Wave 2 starts D, E and G when
A and F have stopped their sims (4 more). G's offline and malicious-archive rows need no sim and can start in wave 1. The real client is needed in C, D and E (one at a time: the user's game).

---

## A. Map editor (P5, Parity A, Checks, MCP)
Use br_valley v6 (3 MB .map) and ls_outpost; WebKit (the app's engine) first, Chromium second; 1440×900.
| Item | Pass when |
|---|---|
| Big-map perf (PerfAudit #1-#10, 4f578685/7cffb07d, 5c7cb5be, 5bd9f4b9) | br_valley in Quad: panning a 2D view stays ≥ 30 fps in WebKit; a nudge and a drag-move have no frame over 100 ms in the frame-split spec; idle traffic ≤ 2-3 requests a minute (the long polls counted) |
| Selection for agents (10190014, c0db9c22) | Select 3 brushes in the page: `editor_selection` (MCP) lists them within a second with sizes and bounds; "paint these" with `editor_set_texture {selection: true}` changes only those, one undo step; `editor_select` from the agent selects in the page within a second; with the app's network blocked, the page asks at most once a second |
| Checks (f866295a, 12bf42b7, 2158ee2a) | Every kind on its purpose-built map (Checks.md table) is found, Pick/Look go there, each one-click fix works as one undo step; an edit isn't blocked while checks run on br_valley |
| Budget (c6c0c15f, c00088f6) | Bars match the last compile's chart; a red bar makes Playtest and Publish ask first; a made-for Last Stand adds its creature roster |
| Leak tracing (5c1e278f) | A room with a gap: after compile the bar shows, the line goes through the gap, the hole's brushes are listed and picked; closing the gap clears it |
| Entity I/O view (07ab5f76) | ls_outpost's gates and multi_managers show every target/killtarget/master link with delays; clicking a link picks both ends |
| Extrude, vertex split/merge, shear (a0c5f385, 7887a143) | Each refuses a non-convex result with a reason; each is one undo step; shear in Front view leans the box on the grid |
| Primitives cone/torus/arch spirals (d80291a0) | Every setting in Preview matches the placed brushes; all valid, convex, base corners on the grid |
| Face sheet (7e1946f1) | To the view, Pick up, right-click paint, Alt+right-click wrap round an edge (texture runs on) |
| 2D views (e8f90dda) | 8 handles stretch with the edge on the grid; zoom to the cursor |
| Content Browser + credits (9a28bb66, d4b35435) | Drag a model/sprite/sound onto the map; Choose… on an entity key; the credit badge matches `content_credits` |
| Playtest on QA4E | Compile Normal → install into the sim's game (not the real one) → bots load the map |
| Round 3 regressions | The Hammer/J.A.C.K. imports from round 3 still re-import → save → export → compile clean |

## B. Game Mode Studio (X, X2)
| Item | Pass when |
|---|---|
| Compile button (c0c650c5, b2865b91) | Real amxxpc without installing; a compiler error lands on its node; status icon; Mod+B / F7 |
| Test run (159181be) | On a good example mode: pass with cost against the baseline and every node ran; on a mode with a deliberate run-time error: fail with the error on its node; Cancel stops the hidden server (its process gone); the person's game untouched |
| All Actions palette (5c7cb5be) | Folders and sub-folders, search opens matches, Favorites, Recently used, Context Sensitive from a dropped wire |
| Several event graphs (44dd882a) | Events on a second tab run; copy/paste across tabs keeps inner wires; two of the same event run in tab order |
| Custom events (69a594c2) | Call Custom Event with typed inputs; Set Timer by Event; an `ls_event` entity on the map fires it |
| Timelines **(when it lands)** | A looping number track drives a node every tick; Finished fires; reverse works |
| Node help (f90f5238) | F1 on any node shows its help; search finds by words; the example drops in |
| New nodes (cd541ea8, 71ca7012, 4d7cf6b1) | Each W1/W3 component node and the round-table/turret nodes does what its help says in a playtest (debug events as evidence) |
| Security (2536e0b9, b2865b91, c0c650c5) | Run Server Command refuses `exec`, `rcon_password`, `quit`, quoted first words, `amx_` lines; Run AMX Command allows only its list; Kick reasons and map names can't inject |
| Example modes (60d61837 Infection; X2's five) **(when they land)** | Each compiles, test-runs to pass and plays 3 rounds with bots on QA4S without run-time errors |
| Debugger | Breakpoints, Resume, live variables (round 3's For Each freeze stays fixed) |
| MCP Studio (d3b922bb) | An agent builds a 3-node mode with `studio_edit`, `studio_check` passes, `studio_test_run` asks first and returns pass/warn/fail |

## C. Last Stand and creatures (W1, Y)
ls_outpost (32 spawns), 6 Normal bots + the user on the client for the visual rows.
| Item | Pass when |
|---|---|
| Economy and shop (06732e70, be8b2f36) | Bots and players earn kill pay and wave money; bots rifle up by wave 2-3; ls_shop items dearer each wave; perks and the gun upgrade buyable |
| Knocked down (5e5947a8, 16fa01a4) | A downed player crawls, the bleed-out bar shows, a revive by holding USE works, perks are lost; the visuals read on the real client |
| Round table (82c4e4b7) | Counts follow round and team size; types unlock by round; the alive cap follows `ls_creature_cap_now` |
| Turrets (7213bb76) | Bought and placed where the player looks; friendly; shoots creatures, not players |
| Tiers and enrage (a7a7e7dd, 49534c0a) | Tier 2 from round 15, tier 3 from 20 (`lst_jump`), enraged speed/damage under the threshold |
| Fleeing (aeee90ca, Y's flee_hp/flee_home/flee_last) | A hurt spitter runs home and comes back; with 3 or fewer left none flees; no wave hangs over 3 minutes on one creature |
| Wave pace | 6 Normal bots reach wave 7-9; no crash in 30 waves (`crashcheck.sh`) |

## D. Battle Royale (W3, W1, W2)
br_valley v6; squads on QA4B (16 bots), duos/solo on QA4B2.
| Item | Pass when |
|---|---|
| Squads and duos (5bc0e2df, 14fb4c12, fe158c49) | br_duo and br_squads in the vote and modes.ini; the 30 s lobby (say /squad); squad mates can't hurt each other; markers and radar dots |
| Knocked down in squads (c5ef3a88) | Downed until bled out or revived; out when the whole squad is down |
| Bots reviving **(as tuned by W2/W3)** | In 4 squad games, bots revive a knocked squad mate at least half the time it's safe (W3.md had 1 in 4 games) |
| Finishing a downed player **(when it lands: W1's `ls_down_finish`)** | An enemy's shots finish a downed player; the kill counts |
| Loot (5ca3ad76, 060d9a90, 6b4aecdf, 25140068, bed0141b) | Death crates hold the dead player's guns and ammo; armor levels reduce damage; healing items work with the bar; rarity glow colors |
| HUD and drop-in (1d543873, 2c89e395, 83f5f83d) | Players left, kills, zone timer, next-zone distance and radar dot; the drop-in steers and lands without damage |
| Bot layers (decb83fb) | About 3 in 10 bots go below ground, 1 in 4 high; no drop-in deaths in 10 games |

## E. Deathrun maps and the mode vote
| Item | Pass when |
|---|---|
| The Deathrun maps that ship (Deathrun.md; sims DR1-DR3 were the agent's) | Each loads, bots run the course and reach the end, traps fire when the terrorist presses them; no crash in 10 rounds |
| Mode vote (83fad7c8) | Battle Royale, Duos and Squads appear in the app's and gm_pubserver's lists, matching votemodes.ini; a vote switches the mode |
| Queued after round 4 (fun_allinone, vehicles) | Not tested: queued for after this round (Deathrun.md) |

## F. Platform: crash tracking, MCP live, site and legal, setup
| Item | Pass when |
|---|---|
| Crash tracking, local (63f17010) | On QA4C: a crash (Y's qa_drop or a forced fault) appears in Servers › Health › Crashes within 2 minutes with time, map, fault and stack; Copy gives the text; the card shows "1 crash in 24 h"; a debugger/stop fault is listed but not counted |
| Crash tracking, Ham Sandwich | Put the official hamsandwich_amxx.dll on QA4C: the warning shows on Health and the badge; Update files puts the fixed one back and the warning goes |
| Crash tracking, Linux **(on the user's Linux test machine, not the production server)** | A crash file appears on the page over SSH; the official .so is replaced on the next start (9d43280b) and the warning clears |
| Crash tracking, Discord | With a test channel's webhook: turning alerts on asks nothing in the app but asks an agent; a new crash posts one plain message with no mentions within 5 minutes; nothing posts for old crashes |
| Crash format with Y **(when Y's capture lands)** | Y's `crashes/*.json` and `modules.json` read as agreed in Y.md; a bad module from the module check shows its own warning |
| MCP live (Claude Code against the built app) | `claude mcp add` with the stdio bridge connects; 170 tools listed; read-only with Allow changes off; every ask-first action shows the native dialog and "Don't allow" leaves things as they were; the activity log lists each call; an agent builds a room, fixes a Checks finding by id, reads the Budget and makes a Studio mode end to end |
| Site (8553153e, 183fc15e, ac912386, e6cd1d71) | Homepage sections, titles and meta as in Site.md; no version numbers; every link works; Windows-first docs; phone width without sideways scroll |
| Legal (91a31d52, 9faeebc8, 302761ea, ee7222f1) | Terms and Privacy linked under Set it up and in About; Publish requires "I have the right to share this"; DMCA agent and legal email right; the warranty panel is bold and bordered |
| Fresh setup and update | A clean bottle: setup end to end, then an update; the fixed Ham Sandwich is installed on Windows/CrossOver |

## G. Community: share and import (T's browser and library, the list service, the publish workflow)
How it works today (docs/COMMUNITY.md, COMMUNITY-SERVER.md, notes/T.md, notes/community.md): files live on
GameBanana (tagged pages); the Loner-Strike index (`dl.lonerstrike.com/community/index.json` + `.sha256`, published from
the list service on the box by the GitHub workflow to R2) adds what GameBanana can't say; Publish builds a package zip
(`<app dir>/tools/community/packages/`), the person uploads it on GameBanana and pastes the link; Try it installs
through the existing installers and notes it in `<app dir>/community/<key>/meta.json`; uids belong to the first
GameBanana member who registered them. Kinds: map, mode (Game Mode Studio graph + its map kit), prefab, spray,
texture. Creature presets aren't shared: the user decided the community shares content and Game Mode
Studio modes only (2026-11-14), so there are no creature-preset rows here.

**Needs the user's OK first:** every row that registers, updates or reports on the live list, or uploads to GameBanana.
Use a test GameBanana page (a private/WIP submission) and test entries only, and clean them up afterwards with the
user (`lsmod.py` pull/delete, and delete the GameBanana page). Rows marked (local) touch nothing outside this computer.
Expected messages are the app's own words; a row fails if a failure shows a raw error, a stack, or nothing.

### Sharing
| Item | Steps | Pass when |
|---|---|---|
| A map with custom content (local) | Make a map in the editor using a custom WAD texture, a model (cycler_sprite), a sound (ambient_generic) and a sprite (env_sprite), compile it into the sim's game, Publish › Check, Make the package | The zip holds the .bsp, nav/knowledge when they fit, a fresh .res and every custom file where the game looks; no stock/Valve file; README and loner-strike.json; credits listed for each file |
| …depending on content not in the pack (local) | Reference a model that isn't on this computer | Check shows a warning naming the missing file ("…isn't on this computer"), the package is still made without it, nothing outside the game folder is pulled in |
| A Studio mode using components/creatures + a kit (local) | A mode with ls_* component nodes, a creature node and two map markers; Publish › A game mode | The package has `modes/<id>/<id>.graph.json` and its kit; no plugin, no .amxx/.sma; the description says what it needs (components, creatures, the bot brain) |
| A mode needing a newer brain (local) | A graph using a node that needs the brain (creatures) | Publish says which brain it needs; the package records it |
| Prefabs (local) | Content › Prefabs › Share on a prefab using a downloaded model | Zip has `.map`, `.json` (with credits), `.png`; the credits name the model's page and author |
| Names with spaces or Unicode (local) | A map named `de_my map`, a mode named "Zoë's Rush", a prefab "Café sign" | A map name with a space is refused with "Map names can only use letters, numbers, _ and -"; the mode and prefab names keep their text in README/json, ids stay plain |
| Huge files and caps (local) | A map with 1.1 GB of custom files; a screenshot over 8 MB; a package zip over 256 MB | "The package would be over 1 GB."; the big screenshot refused by name; nothing half-written left in packages/ |
| Registering (live: user's OK) | Upload the package to the test GameBanana page, paste the link | Listed (or "pending" for a blocked creator); Yours shows it; the moderators' channel gets the card |
| Updating something shared (live) | Upload a new file to the same page, register again; then try registering another member's page declaring the same mode uid | The same entry updates; the other member's is refused: "…belongs to another GameBanana member's creation… remix it" |
| Unlisting/deleting (live) | Moderator pull (`lsmod.py`) or delete the GameBanana page | Gone from the list after the next publish (≤ 15 min + the workflow); Yours says it was pulled |

### Importing
| Item | Steps | Pass when |
|---|---|---|
| Happy path (map) | Community › a listed map › Try it, then Play with bots | Installed through the Get more job; the Maps page shows it with the Community chip; Play starts with bots |
| Happy path (mode with kit) | Try it on a Studio mode | Built with the Studio's compiler on this computer, its kit in the kits folder, Play buttons per mode |
| A mode with risky nodes | Publish a test mode with Run Server Command and Kick nodes; open it in the browser; Try it. Also a mode whose list entry doesn't say (an older entry) | The card and the details show "Runs server commands" and "Can kick players" badges; Try it asks first, naming them, and Cancel installs nothing; the older entry is asked about after the download, before anything installs |
| Missing content | Install a map whose WAD/model/sound isn't in the pack; a mode needing a component/brain this game lacks | The map installs and the Map doctor names what's missing; the mode says what it needs and links to Mods; nothing crashes the game |
| CS 1.6 item into CZ and back | Switch the game to Condition Zero, Try it on a cstrike-only item | Refused or warned in plain words ("made for Counter-Strike 1.6"); a both-games item installs into czero/ |
| Name clash | Install a map with the same name as a map you made; a mode whose id clashes with yours | The installer never writes over existing files (it says which); the mode is refused: the id clashes with a mode you have |
| Older/newer format | Point the app at a test index with `"schema": 2`; a graph file with a newer `version` | "Update Loner-Strike" for the index (the kept copy used); the graph refused with "made by a newer Loner-Strike… Update the app" |
| Corrupt/partial download, then retry | Cut the network mid-download (Wi-Fi off); then a file whose MD5 doesn't match | A plain error, nothing half-installed, retry works once back; the MD5 mismatch is refused, never installed |
| Malicious archives (local, test zips via a local test server or Get more's Add file) | Zips with `../../evil.bsp`, `C:\x.bsp`, `/etc/x`, a symlink, `.dll/.so/.exe/.amxx` inside, a nested zip, a zip bomb (200:1, > 65,536 files) | Each refused or the bad files skipped with a reason; nothing written outside the game folder; the bomb stopped at the caps ("…over 256 MB" / file count) |
| Install then uninstall | Install a map, a mode and a prefab; uninstall each in Installed | Every file the install put in is gone, files it replaced are back, the user's own files untouched (compare a file list before/after); the mode is out of modes.ini |
| Re-import the same item | Try it again on an installed item | Says it's installed (or offers Update); no duplicates |
| While the game is open | Try it/Update for the map being played | The update waits ("pending") and goes in at the next map change; nothing replaced under the running game |

### Moderation and ownership
| Item | Steps | Pass when |
|---|---|---|
| Report (live: test entry) | Report a test entry with each reason and a 500-character note | Sent once; the moderators' channel gets the card; a second quick report is rate-limited politely |
| Taken down after install | Pull a test entry you installed | Installed copy stays and shows the pulled note; Try it is gone; Update never offered |
| Ownership after reinstall | Remove and reinstall the app (keep the GameBanana account), update your test entry | Updating still works: ownership is the GameBanana member, not the app copy |

### Offline
| Item | Steps | Pass when |
|---|---|---|
| Box/R2 down | Block list.lonerstrike.com and dl.lonerstrike.com (hosts file) | The list shows "from earlier" with the kept index; with no kept copy, GameBanana's tag search alone (no badges, modes can't be installed, content can); registering waits and retries |
| GameBanana down | Block gamebanana.com | A plain message, the Installed tab still works, nothing hangs the page |

## H. Windows (real Windows 10 and 11; release build; docs/notes/Windows.md)
Two PCs if possible: one with a Unicode user name (e.g. `Zoë`) and Documents redirected to OneDrive, one plain. A
Steam library on a second drive. Defender on. Note every prompt, window and message word for word.
| Item | Steps | Pass when |
|---|---|---|
| Install and first run | Run the NSIS setup.exe; open the app; Set it up | No console windows flash; the game is found (also in the second-drive library); setup ends without "os error" |
| Old libraryfolders.vdf | On a PC whose Steam library list is the old format (or edit a copy), Settings › rescan | The game in that library is found |
| Game started during setup | Start setup, start Counter-Strike while it downloads | A plain "close Counter-Strike" message, not "os error 32"; Repair afterwards finishes |
| Read-only config | Mark cstrike\config.cfg and liblist.gam read-only; Repair | Repair finishes; the files are written |
| Ham Sandwich | Replace the module with the official hamsandwich_amxx.dll; press Play with the game closed | Ours is back (hash), `.replaced-<date>` beside it, the quiet toast; with the game open: the "next time" note, nothing broken |
| Browser keys | In the editor (a view, a property field, fly mode), the Studio (Monaco), Plugin Studio, Spray: press F5, Ctrl+R, Ctrl+P, Ctrl+F, F3, F7 | No reload, print dialog, find bar or caret prompt; the editor's own F5 (Playtest) and Ctrl+F (Find) still work |
| Explorer drops | Drag a .map, .bsp and .png from Explorer onto the editor, the sidebar and the Studio | Nothing happens; the app stays |
| Right-click | On a sidebar item, an editor panel, empty space; in a text field; on selected text | No Back/Refresh/Print menu outside text; cut/copy/paste in fields and selections |
| Clipboard | Copy brush text from Notepad, Ctrl+V in the editor; same in the Studio | Pastes (note any permission prompt) |
| Alt in the editor | Tap Alt alone then Q; Alt+1; Alt+right-click a face (Texture tool); Alt-drag a texture | No ding, no stuck menu mode, the action happens |
| Ctrl+1-9 | In the editor and the Studio | Note what it does (bookmarks vs sections) |
| Offline fonts | Unplug the network, start the app | Labels aren't cut off |
| Local server | Make a server, start it | Note the firewall prompt (Private and Public ticked?); a friend on the LAN can join; no console window; the log fills |
| Second server, same port taken | Start another program on 27015 first | A plain "port in use" message, no restart loop |
| Crash dialog | Crash hlds (bad metamod DLL) | Time until the app restarts it; does a dialog cover a full-screen game? |
| Keep servers running | Keep running on, close the app, open it again | The server kept running and is picked up |
| Voice | Voice on for a local server, talk in game, stop the server | ls-voice.exe and ls-intent.exe are gone in Task Manager |
| MCP stdio | `'{"jsonrpc":"2.0","id":1,"method":"ping"}' \| & "$env:LOCALAPPDATA\Loner-Strike\loner-strike.exe" --mcp-stdio` with agents on; then Settings › AI agents › Copy for Claude Code and run it in PowerShell | One JSON line back; `claude mcp list` shows loner-strike Connected |
| Update | One server with Keep running, one without, voice and an MCP bridge on; update in passive mode | Which processes survive; voice comes back; the agent reconnects; latest.json's windows entry is the setup.exe |
| Remove everything | With the window open | Only EBWebView is left for the uninstaller; no failure list otherwise |
| Uninstall | With "Delete app data" ticked | List what's left in %APPDATA%, %LOCALAPPDATA%, firewall rules, the game folder, %SystemDrive%\Loner-Strike, %USERPROFILE%\.loner-strike |
| Spray fonts | Open the spray font picker the first time; a per-user installed font | Time it; the per-user font loads |
| Pointer lock | Unreal preset right-drag; Hammer preset Z; Esc | Esc releases and the camera stops looking around |
| OneDrive Documents | Save a project into Documents (OneDrive, files on demand) | Saves and reopens; no lock errors in the diagnostics |

## Round 3 items to re-check
From QA.md "Round 3": the brain drop crash (30 rounds of drop/drop_all/drop_dead), Last Stand bots buying guns, the
BR start loadout and drop-in, and the Studio's gm_components loading for templates.
