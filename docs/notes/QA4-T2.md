# QA round 4, T2: map tools, personalize tools and the asset browser (fresh QA tester, 2026-11-14)

What was tested: Map doctor (`doctor`), Map simulator (`health`), Nav fixer (`navfix`), Callouts (`callouts`),
Radar maker (`radar`), Spray maker (`spray`), Crosshair & config (`playercfg`), Bot people (`people`) and the asset
browser (`assets`, plus the map editor's Content Browser for sounds, sprites and credits). No product code was changed.

**Counts: 46 checks. PASS 36, FAIL 3, NEEDS USER 7.** Plus 10 low findings inside passing rows (L1-L10) and a
separate UX/UI section (2 high, 6 medium, 7 low).

## Builds and setup
- **Code:** `tools` HEAD **e315c267**, a `git archive` snapshot (UI built from it: `tsc -b && vite build` green; the
  Rust test binary built from it into the shared target, `-j 6`, `nice 15`). HEAD moved to 8441e600 while I worked.
  None of those commits touch the tested tools (only `mapcopy::stock_wads()` was added), so the findings stand on HEAD.
- **Brain:** `git ls-remote` ls-next3 head is 7aacd5e, and its CI run failed (43629175469). So did dc8d586. The newest
  green run is **43627468139 (`57e1fec`)**. Its win32 mp.dll (md5 797a936a…) was copied into the sim. I checked it
  inside the sim: `lonerstrike_mapknow` and `ls_creature_flee_last` are in the DLL, and the console says
  `ReGameDLL version: 5.30.0.1295-dev`.
- **Plugins:** compiled fresh from the snapshot into the sim with `plugins-fresh.sh` (SNAP=snapshot). All 19
  gm_*/Loner-Strike plugins built. The third-party sh_*, zp50_* and war3ft plugins failed on missing includes, as in
  the other QA4 reports.
- **Game copies (all APFS clones, under Sim/):**
  - `QA4T2`: a clone of LSPLAY, CS 1.6 server, no CZ. Used as the sim and as the game for dev server B.
  - `QA4T2HL`: a clone of the Half-Life folder, with CZ. Dev server A, and dev server D (`--mod czero`).
  - `QA4T2NOCZ`: QA4T2HL without czero/czeror. Dev server C, a client install without CZ.
  - `QA4T2SRV`: `LS_SERVERS_DIR` for every dev server.
- **Dev servers:** `serve_tools_dev` on 8471-8474, each with its own `LS_APP_DIR` and `TMPDIR`. Vite on 5471 and 5472,
  each with its own cache dir, HMR off.
- **Browsers:** Playwright in Chromium and WebKit, at 1440x900, 1024x680 and 800x600.
- **Sim:** one sim at a time, started from my shell with `-insecure`, at load 8-17. It ran Sim/QA4T2 on port 27461,
  de_aztec, 12 bots, with a small logging plugin of my own (`qa_loc`, which logs each Location message).
  `crashcheck.sh since <mark>` was empty.
- **Real game and real app data:** never touched. The real Half-Life folder was only read, by cloning it.
- **Evidence:** in my scratchpad,
  `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/qa4t2/`. It holds `shots/`
  (165 screenshots), `sim_run1.log` and `sim_run2.log` (sim consoles; rcon is masked), `sweep.log`, `pw-specs.log`,
  `credits_cmp.txt`, `both_not_stock.txt` and `cargo-build.log`.
- **Left over (NEEDS someone with permission):** the safety hook blocks `rm -rf` outside the worktree, so I could not
  delete my four sim copies: `Sim/QA4T2`, `Sim/QA4T2HL`, `Sim/QA4T2NOCZ` and `Sim/QA4T2SRV`. They are APFS clones and
  use little disk. Nothing of mine is running. My dev servers, Vite servers and the sim were stopped by PID.

---

## FAILs (most serious first)

### F1. With Condition Zero installed, CZ-only files and mod files count as "every player has it" (medium-high). Owner: P5 (f7d1ddd5, `doctor::is_stock`), with T (credits.rs)
The QA4-A fix added Valve's CS 1.6 list (`stock_cstrike.txt`). It kept the old rule that a cstrike file is also stock
when **the same path is in czero/**: `in_cstrike && (stock_cstrike(rel) || czero has it)`. That rule is wrong both
ways:
- **CZ props copied into a CS 1.6 map.** kz_urban uses `models/props/trash.mdl`, `radio.mdl`, `ac_large.mdl` and 6
  more. These are Condition Zero's own props. A CS 1.6-only player doesn't have them. With CZ installed, the doctor
  says "in cstrike (every player has it)" and leaves them out of the .res. Without CZ, it lists them. Evidence on the
  same map:
  - with CZ (8471): `wanted 5`, 23 files marked stock;
  - without CZ (8473): `wanted 14`, 14 files marked stock;
  - the 9 extra "stock" files are all `models/props/*` (shown in `shots/chromium-1440-doctor_kz_urban.png`).
- **Mod content installed into both games.** 452 files in cstrike are also in czero but not on Valve's list: basebuilder,
  warcraft3, zombie_plague, paintball, superhero, DICE, gungame, roundsound and more (`both_not_stock.txt`). On the CZ
  copy, `/api/tools/credits/asset` calls them `stock`: "Every player has it already; it isn't shipped or credited".
  Without CZ, they are `unknown`. Over the whole Content Browser: models 318 vs 170 stock, sounds 601 vs 389, sprites
  164 vs 79 (`credits_cmp.txt`).

Impact: on a computer that has CZ (the setup this project recommends), the map doctor's .res, Publish packaging and
credits all leave out files that players don't have. Joiners can miss models, sounds and skies, and the credits drop
third-party authors.

Repro:
1. Use a Half-Life folder with cstrike and czero, where a file exists at the same path in both but isn't Valve CS 1.6
   content. For example, `models/props/trash.mdl` copied from czero into cstrike.
2. Run the map doctor on a map that uses it (kz_urban): it says "every player has it".
3. Or call `GET /api/tools/credits/asset?folder=cstrike&path=models/props/trash.mdl`: the answer is `source: stock`.

Suggested fix: for cstrike, count only Valve's list (and valve/). The CZ copy of a file is evidence for CZ maps, not
for CS 1.6 ones. For czero maps, the same needs a CZ list.

### F2. Maps in `cstrike_downloads` are invisible to every map tool (medium). Owner: agent A (`mapcopy::find_local`, the `/api/tools/maps` list), with G (doctor)
The game downloads a server's custom maps into `cstrike_downloads/maps/`, and it loads them from there. None of the map
tools list them:
- The copy has 6 such maps (kz_zentemple, aim_ak_colt_2012, cs_assault_1337, ze_lgcs_rats_fixed and others). None is
  in the `/api/tools/maps` list (397 maps).
- `GET /api/tools/doctor/kz_zentemple` answers "That map isn't on this computer."
- The doctor itself says "Looked where the game looks: cstrike, then cstrike_downloads, then valve" for a map's
  *files*. `find_local` only looks in `cstrike/` and `czero/`.

So a player can't check, fix the nav of, name places on, or make a radar for any map they got by joining a server.

Repro: put a .bsp only in `<Half-Life>/cstrike_downloads/maps/`, open Tools › Map doctor and search for it. It isn't
there, and `doctor:<name>` says it isn't on this computer.

### F3. Spray maker's "Put back" leaves the old spray read-only (low-medium). Owner: D / P10 (`spray.rs` restore)
Install makes `tempdecal.wad` read-only (on purpose, so the game doesn't overwrite it). "Put back" restores the old
file's bytes, but keeps it read-only:
- before: `-rw-r--r-- 6296 tempdecal.wad`;
- after install, then `POST /api/tools/spray/restore {"game":"cstrike"}`: `-r--r--r-- 6296` (same md5 as before,
  78583230…).

After "Put back", picking a spray in the game's Options › Multiplayer silently fails, because the game can't write the
file. The toast's **Undo** (safety.rs) restores the mode correctly (`-rw-r--r--`), so the two ways back behave
differently.

Repro: on a writable tempdecal.wad, Install spray (the read-only option is on by default), then More › Put back. Check
the file's mode.

---

## Results

| # | Tool | Check | Result | Evidence |
|---|---|---|---|---|
| 1 | All | Rust unit tests: doctor 8, health 17, navfix 15, callouts 10, overview 6, spray 9, playercfg 5, people 7, assets 8, credits 8, wadwrite 14, nav 7, content browser 3, mapcopy 6 | PASS | 123 passed, 0 failed (snapshot binary) |
| 2 | All | UI build from the snapshot | PASS | `tsc -b && vite build` green (`uibuild.log`) |
| 3 | All | 150 page loads: 25 addresses (each tool, a stock map, a custom map, corrupt and missing maps) x 3 sizes x 2 engines | PASS | No page errors and no horizontal overflow at 800, 1024 or 1440 (`sweep.log`). Only 404/422 from expected misses (no overview, a corrupt map, GameBanana not in the harness) |
| 4 | All | Existing specs: callouts, navfix-ux, p10 (spray, crosshair), sim-report, tools.spec (these tools, History, map workspaces), editor-content, editor-assets | PASS | 48+30+12+8 passed across Chromium and WebKit at both sizes (`pw-specs.log`) |
| 5 | All | Keyboard: Tab order and focus rings (Chromium) | PASS | Every focus stop has a visible ring and the order follows the layout. WebKit only tabs to form fields, which is the platform default |
| 6 | Doctor | 12 maps, including 5 corrupt ones (truncated, zero bytes, random bytes, a lump outside the file, a Quake version) and a 13 MB map | PASS | Plain errors, each 6-8 ms ("This map file is damaged (it's shorter than a map header)…"). See L2 |
| 7 | Doctor | Make/update .res | PASS | temple_of_doom: 7 files listed, `.res.bak` kept on update, the header says it keeps added lines. Bad names (`../`, spaces, 300 chars) refused; a corrupt map gives a readable error. See L6 |
| 8 | Doctor | Copy files from another game folder | PASS | `models/props/trash.mdl` copied from czero. `../../hl.exe`, `/etc/passwd`, `models/../../czero/liblist.gam` and missing files are refused. 500 entries are capped (L7) |
| 9 | Doctor | Fix all safe, then Undo | PASS | WebKit: "6 to look at" → "4 to look at"; the toast's Undo put kz_urban.res back. The copied files stay (they only add) |
| 10 | Doctor | Stock files without CZ (the QA4-A fix) | PASS | No-CZ client copy: p_galil, v_famas, gign, 640hud1.spr, ak47-1.wav, radio/go.wav, greenbk.tga, hostage.mdl, rain.wav are all `stock` |
| 11 | Doctor | Stock files with CZ installed | **FAIL (F1)** | kz_urban wanted 5 vs 14; 452 files |
| 12 | Doctor | Maps in cstrike_downloads | **FAIL (F2)** | 6 maps missing from the list |
| 13 | Doctor | Condition Zero maps (`--mod czero`) | PASS | de_dust_cz, cs_italy_cz, de_dust2: CZ-first lookup, stock map, places read. de_dust_cz's nav correctly warned as "for another version" (nav size 3924584, bsp 4885352) |
| 14 | Doctor | "Teach bots this map", and "the game has this map loaded" | NEEDS USER | Needs the app's hidden server and a running game client |
| 15 | Simulator | Options page and server pick | PASS | Lists my registered sim server, 26 modes, limits of 2-20 bots and 1-15 minutes |
| 16 | Simulator | Report from a real recording: layers, rounds, fights, the picture after leaving and coming back | PASS | NAVH de_dust2 recording: 4 rounds (T 3, CT 1), 8 fight spots. sim-report spec 12/12 in Chromium and WebKit at both sizes |
| 17 | Simulator | A real run, and Cancel during it | NEEDS USER | The dev harness answers "Set up the game on this computer first." A built app would use the real app data, which this pass may not touch |
| 18 | Nav fixer | Odd edits through the API | PASS | Zero size, beyond the map, bigger than any map, non-number, self link, missing area, place "QA Bridge; quit", 500 chars, unknown op: all refused in plain words. 12 undos stop at "Nothing to undo."; redo works |
| 19 | Nav fixer | Reach check on real maps | PASS | real_rats2 tests pass (3 unreachable, 38 trapped, 79 only by falling in). In the page, **Fix 2** on "Only by falling in" took that count from 79 to 0 |
| 20 | Nav fixer | Save, .bak and reload | PASS | de_aztec: area 2186 added and linked; reach 611/611; "Saved de_aztec.nav. The nav you started from is kept as de_aztec.nav.bak." Reload reads 611 areas |
| 21 | Nav fixer | The game uses the saved nav (sim) | PASS | Sim console: `Loner-Strike: map knowledge for de_aztec loaded (611 areas).` The nav was not regenerated (mtime unchanged) |
| 22 | Nav fixer | Unsaved edits survive leaving the page; a triple-clicked Save | PASS | Coming back shows "Unsaved changes, 2 changes"; one save, one toast |
| 23 | Nav fixer | Rebuild bot knowledge after a save | NEEDS USER | Needs the app's hidden server |
| 24 | Callouts | Real save through the page | PASS | de_aztec on the sim copy (Chromium): box → "QaYard", Rename CTSpawn → "ZorkBase", Save: "Saved 82 area names. ZorkBase and QaYard are new to the game, so they're added to its place list (BotChatter.db)". BotChatter.db gained `Place ZorkBase/End` and `Place QaYard/End`; names survive a reload |
| 25 | Callouts | Custom names in game (sim) | PASS | Location messages: `ZorkBase` 66, `QaYard` 3, plus stock `#BombsiteB`, `#Courtyard`, `#Bridge`… (`sim_run2.log`, `[qa_loc.amxx] LOC 1 BigBean ZorkBase`) |
| 26 | Callouts | Names from Condition Zero's nav | PASS | real_callouts: de_inferno (1935 named) and cs_office (862 named) write and check back |
| 27 | Radar | Previews for every look, and odd queries | PASS | Previews in 0.5-0.6 s. cut=±99999 is clamped. Bad style, orient, roofs or cut are refused with 400 (L5) |
| 28 | Radar | Install, restore, install twice, restore | PASS | 1024x768 8-bit BMP (787510 bytes, like Valve's) plus a .txt with ZOOM/ORIGIN/ROTATED/HEIGHT. After two installs, restore gives back the original (md5 b8c229b0 = before). A second restore says "There's no older overview to put back." |
| 29 | Radar | Condition Zero overview read | PASS | `czero/overviews/de_dust_cz.txt` found, readable, rotated |
| 30 | Radar | The overview in the game's spectator view / HLTV | NEEDS USER | Needs the real client |
| 31 | Spray | Composer specs (layers, undo, fonts, Start sheet, opacity) | PASS | p10.spec in Chromium and WebKit at both sizes |
| 32 | Spray | Odd pictures to `/use` | PASS | 1x1 accepted. Not-PNG, bad base64, empty, 3000x10 and 4000x4000 refused (L4) |
| 33 | Spray | Install into both games | PASS | `WAD3`, 1 lump `{LOGO`, type 0x40, 96x96, read-only, in cstrike and czero. The UI Install in WebKit says "Installed", and its toast Undo restored both files |
| 34 | Spray | "Put back" | **FAIL (F3)** | The file stays read-only |
| 35 | Spray | The spray in game (T) | NEEDS USER | Needs the real client (P10 checked opacity by hand on 2026-11-13) |
| 36 | Crosshair | Save into userconfig.cfg | PASS | The block between START and END. The player's own lines above it and a line after it are kept. Aliases for jump+duck. config.cfg ends with `exec userconfig.cfg` |
| 37 | Crosshair | Hostile values | PASS | sensitivity "abc" or 99999, rcon_password, name, "255 0 0; quit", "300 0 0", F3 (Game Mode Menu), "say hi; quit", unknown key, a key used twice: all refused with a reason |
| 38 | Crosshair | Remove and put back | PASS | Remove takes only the block out; put back restores the file as it was. See L10 |
| 39 | Crosshair | Settings take effect in the game | NEEDS USER | Needs the real client |
| 40 | Bot people | Bad input | PASS | A space becomes "_"; ";" blocks Save with "1 thing to fix before saving… a name is up to 31 letters, digits and _ (no spaces)". How common is clamped to 0-1000 |
| 41 | Bot people | Double-clicked Save, backup, put back | PASS | One save (people.ini `just_plays|55`); a backup in people/backups; Put back restores the original (md5 48875ef6) |
| 42 | Assets | Every texture and model in the game copy | PASS | real_assets: 102 WADs, 10907 textures; 1588 models built (2.06 M triangles), 0 problems; usage over 392 maps |
| 43 | Assets | Bad WADs and paths | PASS | Random bytes, empty, and a huge-count WAD: plain errors, listed as "can't be read". `..%2F` path refused |
| 44 | Assets | Content Browser (models, sprites, sounds, credit badge, Choose…, drag onto the map) | PASS | editor-content and editor-assets specs, Chromium and WebKit. Badge values are as in rows 10-11 |
| 45 | Assets | Get more (GameBanana) and adding your own file | NEEDS USER | Needs the network and the app's file dialog (the dev harness has no `/api/gb`) |
| 46 | All | Contrast | PASS for function, see UX-H1 | Measured on every page (`contrast.mjs`) |

### Low findings inside passing rows
- **L1 (Nav fixer, owner Nav):** a one-click fix is several undo steps. "Fix 2" adds 2 links, and one Undo leaves
  "1 change". Expected: one click, one undo step, as in the editor's Checks.
- **L2 (Doctor, owner G):** for a map that can't be read at all (qa_garbage), "Textures and WADs: All passed" and
  "Download list (.res): All passed", and the header says "3 of 5 checks passed". The error has a double period:
  "(map version 1300590692, not 30).." (`shots/webkit-1024-doctor_qa_garbage.png`).
- **L3 (Nav fixer):** a damaged map says "Couldn't open qa_garbage's nav", although the map is what's broken. Draw area
  and Link stay enabled with nothing loaded (`shots/chromium-1024-navfix_qa_garbage.png`).
- **L4 (Spray):** a drawn picture over 2048 px is refused as "The drawn spray was damaged." It should say it's too big.
  The page never sends one that big, so only the API sees it.
- **L5 (Radar):** bad query values get a plain-text 400 from serde ("unknown variant `none`"), not `{error}`.
  The page never sends them.
- **L6 (Doctor):** `POST /doctor/de_dust2/res` rewrites a stock map's own .res. Valve's file is replaced (kept as .bak),
  and stock overviews are added to the list. The page doesn't offer it ("needs no list"), but the route should refuse
  stock maps.
- **L7 (Doctor):** copy with duplicate paths echoes every duplicate in `skipped` (199 lines). Dedupe first.
- **L8 (Callouts):** while the pointer rests on the map, the status bar shows the hover readout instead of "N picked".
  callouts.spec's real-save path fails on that (`expect … "picked"`). It's a spec or status-line issue, not a lost
  pick: the panel said "Name 21 picked areas".
- **L9 (Simulator):** in the dev harness, Options lists the server, and then Run says "Set up the game on this computer
  first." It's a harness gap, but the page could check that before offering Run.
- **L10 (Crosshair):** saving settings equal to the game's own (sensitivity 3 when config.cfg has 3.000000) writes no
  block, yet says "Saved… used from the next time you start the game". Say "Nothing to change" instead.

---

## UX/UI review (ranked)

Screenshots are in `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/qa4t2/shots/`.

### High
- **UX-H1. Low contrast on state that matters (all tools; shared styles, owner B/app UI).** Measured on the dark
  green theme:
  - the selected option of every segmented control (New / Pick / Reach / 5 min / CS 1.6…): **2.8:1**;
  - muted help lines and counts: **3.6:1**;
  - secondary buttons (History, Docs, Other map): 4.2:1;
  - the doctor's red "2 problems" headline: 3.2:1;
  - disabled Undo/Redo: 2.4:1.

  AA needs 4.5:1 for small text. The selected segment is how every tool shows what's on, so this hurts the most.
  Suggestion: make the selected segment's text lighter or put it on a solid fill (≥4.5:1), and raise muted text to
  about #b8c2a8. Screenshots: `chromium-1440-radar_de_aztec.png`, `chromium-1440-people.png`.
- **UX-H2. The doctor tells mappers that CZ props are "in cstrike (every player has it)" (F1, shown in the UI).** A
  mapper trusts this line and ships without the files. Suggestion: fix F1. Also say *which* players have a file when
  it's only in one game ("Condition Zero players have it; CS 1.6 players need it").
  Screenshot: `chromium-1440-doctor_kz_urban.png`.

### Medium
- **UX-M1. Toasts cover the Save area.** After a save, two stacked toasts ("Undo that save?" plus the long message) sit
  over the action bar's main button in Callouts, Nav fixer and Spray. In Callouts they also cover Suggestions.
  Suggestion: put toasts above the action bar, or merge the "Undo that save?" toast into the message toast.
  Screenshots: `callouts-after-save-chromium.png`, `navfix-saved-chromium.png`, `spray-install-webkit.png`.
- **UX-M2. Spray maker in a small window.** At 800x600 the canvas is below the fold: the first screen is only the
  sticker grid. The bar says "No unsaved changes" next to "Not saved yet" for a new design, which contradicts itself.
  Suggestion: below about 1000 px, put the canvas first and the Add panel in a drawer. For a new design, say
  "Not saved yet" only. Screenshot: `chromium-800-spray.png`.
- **UX-M3. Texture search only searches the selected WAD.** Typing "qa_" says "0 textures matching" in cstrike.wad
  only. A first-time user expects a search over everything. Suggestion: search every WAD (or offer "All texture
  files"), and say which file each hit is in. Unreadable WADs are disabled options ("· can't be read") with no reason
  shown; show the error on hover. Screenshot: `assets-qa-search.png`.
- **UX-M4. A damaged map reads as mostly fine.** See L2: "All passed" groups and "3 of 5 checks passed" for a map that
  can't be read. Suggestion: when the .bsp can't be read, show only that problem, and say "Other checks need a
  readable map". Hide Map viewer, Radar maker and Simulate for it. Screenshot: `webkit-1024-doctor_qa_garbage.png`.
- **UX-M5. Radar maker: the zoom buttons vanish on the default green background.** "+", "−" and "Fit" are muted
  buttons on bright green (Fit is barely visible), and the status says "north up" while Turn is "Auto" and the
  installed file is ROTATED 1. Suggestion: give the canvas controls a dark backing, and make the status say the
  orientation that will be written. Screenshot: `chromium-1440-radar_de_aztec.png`.
- **UX-M6. Errors after a dead end say "Try again".** Nav fixer and Radar maker on a damaged map offer only
  "Try again", which can't help. Suggestion: "Check it in the Map doctor" as the main action.
  Screenshot: `chromium-1024-radar_qa_garbage.png`.

### Low
- **UX-L1.** Models: the 3D view shows a model (Admin_CT) that isn't highlighted in the list, and models are lit very
  dark. Highlight the shown model and raise the ambient light. (`chromium-1440-assets_models.png`)
- **UX-L2.** Crosshair & config: "800 / 1024 / 1920" and "1× / 2×" under the preview have no labels. Add "Screen" and
  "Zoom". (`webkit-1024-playercfg.png`)
- **UX-L3.** The simulator report has two Back buttons (the crumb and one in the summary card). Keep the crumb.
  (`webkit-800-health_report_1794685295883.png`)
- **UX-L4.** The doctor's "Other tools" disables Nav fixer and Callouts with no reason. Add a title "Needs a bot nav
  first: Teach bots this map". (`webkit-1024-doctor_qa_garbage.png`)
- **UX-L5.** Error copy uses jargon: "part 0 points outside the file", "map version 1300590692, not 30". Say "The map
  file is cut short or damaged" and "This isn't a Counter-Strike map file".
- **UX-L6.** One-click fixes in the Nav fixer are several undo steps (L1). Same rule as the editor: one click, one step.
- **UX-L7.** Spray maker installs the template's placeholder "NAME" if a first-time user clicks Install right away.
  Start blank, or put the player's name in.

### What works well
- Clear first screens: every tool's empty state says what it does and how to start (Map simulator, the doctor's map
  picker, Spray's "Nothing picked" tips).
- Consistent action bars: Saved / Unsaved changes / N to look at, "Fixing…" and "Saving…" labels, and History and Undo
  after every write.
- Plain, specific refusals in every write route (playercfg, people, navfix, doctor).
