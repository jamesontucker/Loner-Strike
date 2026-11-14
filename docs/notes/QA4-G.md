# QA round 4: area G (community share and import) and the community roles rows

Run on 2026-11-14 (10:45-11:01 Pacific, 18:45-19:01 UTC) by a fresh QA agent. The build was the committed
`tools` HEAD **314c7f47** (`git archive` into a scratch folder, built in the shared target; the Rust unit tests
and a `serve_tools_dev` server ran from that build). Game folder: the sim copy `Sim/QA4G` (cloned from LSPLAY, now
deleted). App data: throwaway `LS_APP_DIR` and `TMPDIR` folders in my scratchpad. I never touched the real game or
the real app data.

The only change to the scratch copy (never in the repo) was a test-only route set in `devserve.rs`. It exposed Get
more's real job list and `start_local_install` / `uninstall_section` by path, because the app's "Add file" opens a
native dialog. Prefab imports used the existing `LS_DEV_PICK`. For the offline rows, a second dev server ran with
`HTTPS_PROXY`/`HTTP_PROXY` set to a dead port, so the box, R2 and GameBanana were all unreachable, plus a crafted
kept index. I didn't use the hosts file: that needs root and would hit the whole Mac.

No sim was started. The mode-install-and-play rows need a real GameBanana page; see NEEDS USER. Live actions: the
harness `discord-check`, read-only roles probes with the admin token and Alone Together's token, forged and
unsigned Polar webhooks (all refused), and safe list-service probes. Nothing was registered on the live list. No
token, secret or rcon password was printed.

**Totals: 54 rows. PASS 26, FAIL 7 (six issues, F1-F6; F6 covers two rows), NEEDS USER 17, and 4 rows the user already passed live (not redone).** (A row that passed only through unit tests is marked PASS
(tests); its live part is NEEDS USER.)

---

## FAILs (repro steps and a likely owner)

### F1. A map package ships the game's own CS 1.6 files when Condition Zero isn't installed (row "A map with custom content")
- **Repro:** use a Half-Life folder with `cstrike/` and no `czero/` (a CS 1.6-only owner; here the HLDS sim). Run
  Publish › Check on `aim_map` (or any map with `skyname des`), then Make the package.
- **Got:** the zip holds `gfx/env/desup.tga … desbk.tga` (CS 1.6's own desert sky), and they're listed in the
  `.res`. Publish › Check for `cs_assault` also lists `models/hostage.mdl` as a custom file. Each one gets an "add a
  credit" warning.
- **Why:** `doctor::is_stock` counts a file as stock only when it's in `valve/`, or in **both** `cstrike/` and
  `czero/`. With no CZ, every CS 1.6 file looks custom. On the user's Mac CZ is installed, so this doesn't show
  there. `hostage.mdl` isn't in czero at all, so it may count as custom even with both games installed.
- **Expected:** no stock or Valve file in the package. Suggest a list of CS 1.6's own files (like `STOCK_WADS`), or
  "in cstrike and it came with the game".
- **Owner:** T (community.rs readiness) / map doctor (`tools/doctor.rs is_stock`, P5).

### F2. Publish › Check doesn't name a missing file (row "…depending on content not in the pack")
- **Repro:** a map whose `cycler_sprite` uses `models/qa_missing_model.mdl` (not on the computer). Run Publish ›
  Check.
- **Got:** "1 files the map uses aren't where the game looks; they'd be missing for players. The map doctor can copy
  or find them." It doesn't name the file, and it says "1 files". The package was still made without it, and nothing
  from outside the game folder was pulled in (both of those parts pass).
- **Expected:** a warning that names the file ("…isn't on this computer").
- **Owner:** T (`community.rs` `readiness`, the `missing` counter).

### F3. An oversized screenshot isn't refused by name (row "Huge files and caps")
- **Repro:** POST `/api/tools/community/publish/qa_custom` with two screenshots, `small.png` and `huge_shot.png`
  (9.2 MB).
- **Got:** "A screenshot is over 8 MB." It doesn't say which one. The 1.1 GB case passes ("The package would be over
  1 GB.", in 42 s), and nothing half-written was left in `packages/`.
- **Expected:** the big screenshot named.
- **Owner:** T (`community.rs` around line 1593).

### F4. A newer-format index never says "Update Loner-Strike" (row "Older/newer format")
- **Repro:** serve an index with `"schema": 2` (here, as the kept copy with the box unreachable). `parse_index`
  makes "The community list is newer than this app. Update Loner-Strike to see it.", but `load_index` drops that
  error.
- **Got:** a live schema-2 download counts as "didn't answer", so the page says "Showing the Loner-Strike list from
  earlier (the download server didn't answer)". With no kept copy it says "The Loner-Strike list couldn't be
  reached…". The kept copy is used (that part passes). The graph half of the row passes: "That graph was made by a
  newer Loner-Strike (graph version 99). Update the app to open it."
- **Expected:** "Update Loner-Strike" shown, and the kept copy used.
- **Owner:** T (`community.rs` `load_index`/`Source`; `Community.tsx` lines 196-200).

### F5. A CS 1.6-only item installs into Condition Zero (and back) with no warning (row "CS 1.6 item into CZ and back")
- **Repro:** look at `start_try` and the UI. The index's `games` (`["cstrike"]` / `["czero"]`) is parsed and
  cleaned (community.rs line 610), but nothing compares it with the player's game: not `start_try` and not the
  Community page. In the browser (offline index), a `games: ["czero"]` map shows on a CS 1.6 game with no mark at
  all.
- **Expected:** refused or warned in plain words ("made for Counter-Strike 1.6"); a both-games item goes into
  czero/.
- **Owner:** T (community.rs `start_try`, `Community.tsx` cards/details).

### F6. The mode package doesn't say what it needs, and the brain it needs isn't recorded (rows "A Studio mode using components/creatures + a kit" and "A mode needing a newer brain")
- **Repro:** save the Last Stand template as `qa_stand` (ls_wave_spawner, creature nodes, markers), then Publish ›
  A game mode › Make the package.
- **Got:** the zip is right: `modes/qa_stand/qa_stand.graph.json` and `.kit.json`, README, `loner-strike.json`, and
  no plugin, `.amxx` or `.sma`. The kit lists the components and creatures. But the GameBanana description text, the
  README and the list entry (`item.mode`) never say it needs components, creatures or the bot brain. Only the Studio's
  Requirements panel shows "Also needs" (`known.brain: true`). No brain version is recorded anywhere, and Publish
  doesn't say which brain it needs.
- **Expected:** the description says what it needs, and Publish names the brain it needs, which the package records.
- **Owner:** X2 / T (`community.rs` mode publish, `requirements.rs`).

### Minor findings (not row FAILs)
- **Silent drops.** Get more's local install drops `../../evil.bsp` and the symlink entry without listing them under
  "skipped" (the `.dll`/`.so`/`.exe`/`.amxx`/`.bat` and `/etc/x` entries are listed with a reason). It's safe, but
  there's no reason shown. Owner: archive.rs/gamebanana.rs.
- **React key warning.** Publish › A game mode logs "Encountered two children with the same key" (`Checks`,
  Publish.tsx line 144) when two checks share a title ("Requirements", "The graph has problems"). It's a console
  error in dev, and tools.spec fails on console errors. Owner: T.
- **Odd wording.** `GET https://list.lonerstrike.com/roles/polar/webhook` (wrong method) answers 401 "This server
  isn't claimed…". The wording is odd but harmless. Owner: roles.py.
- **Prefab WADs left behind.** A community prefab's new WAD (`qa_newwad.wad`) goes into `cstrike/`. Removing the
  prefab from the editor library doesn't remove that WAD. The community Uninstall test covers removal
  (`uninstalling_a_prefab_removes_only_its_files`), but I couldn't run a community prefab install live (it needs
  GameBanana).

---

## Sharing
| Row | Result | Evidence |
|---|---|---|
| A map with custom content (local) | **FAIL** (F1) | `qa_custom` (aim_map with an entity lump adding `aim_xmas.wad`, `models/bag.mdl` cycler_sprite, `sound/benfolds_fired.wav` ambient_generic, `sprites/3dmflaora.spr` env_sprite). The zip holds the .bsp, a fresh `.res`, every custom file where the game looks, README, `loner-strike.json` (schema 1, kind map, bspSha256). No nav (none fits). Credits: each unknown file gets an "Add a credit…" warning, and `credits: []`. The stock desert sky was shipped (F1) |
| …content not in the pack (local) | **FAIL** (F2) | Missing model: counted, not named. The package was made without it, and nothing outside the game folder was added |
| Studio mode with components/creatures + kit | **FAIL** (F6) | Package contents are right and there's no plugin. The description doesn't say what it needs |
| A mode needing a newer brain | **FAIL** (F6) | No brain version in Publish or the package |
| Prefabs (local) | PASS (partly) | Share on builtin `Doorway` (export via `LS_DEV_PICK`): zip has `prefabs/doorway.map`, `.json`, `.png`, README, `loner-strike.json` (kind prefab, uid, wads). Credits naming a downloaded model's page: **NEEDS USER** (needs a model installed from GameBanana); unit `prefab_credits`/`community_files_by_kind` pass |
| Names with spaces or Unicode | PASS | `de_my map`: Publish `"That isn't a map name."` (the editor's compile gives "Map names can only use letters, numbers, _ and - (up to 31)."); Requirements refuses `"de_my map isn't a map name."`. Mode "Zoë's Rush": README and json keep "Zoë's Rush", the id stays `qa_risky`. Prefab "Café sign" imported as id `mine:caf_sign`, name kept. `..%2F..%2Fetc` refused |
| Huge files and caps | **FAIL** (F3) | 1.1 GB: "The package would be over 1 GB.", nothing left in `packages/`. 9.2 MB screenshot: "A screenshot is over 8 MB." (not named). A non-image refused: "Screenshots have to be PNG or JPEG pictures.". 7 screenshots: "At most 6 screenshots." |
| Requirements in the Studio (local) | PASS (partly) | `/api/tools/requirements/qa_calm`: default map de_dust2 `source: base`; recommended `qa_custom` → `local` with "Share this map first so players can get it.", `de_nuke` → base. The Spawn Prop model and the sound are listed with `have`/`link`. `known` shows components/creatures/brain (qa_stand: brain true, ls_wave_spawner, creatures all). The Play Sound node only takes its choice list ("Pick one of the choices for Sound."), so a custom sound comes only through the model/content path. A community-sourced item with a suggested link: **NEEDS USER** (needs an installed community item). MCP `studio_requirements` not run here (area F) |
| Requirements at Publish | PASS | Unlinked: "Not linked to a community or GameBanana item, so players may not have them: models/bag.mdl, sound/benfolds_fired.wav." (a warning). Found nowhere: blocker "models/qa_nowhere.mdl isn't on this computer and isn't linked to an item that has it. Link it in My mode › Details › Requirements, or remove what uses it." |
| Playtest screenshots (local) | PASS (route + UI) | Three `qa_customNNNN.bmp` planted in the game folder, as F5 writes them (a real F5 in a playtest needs the client: **NEEDS USER**). `/playtest-shots` returns them as PNG with map `qa_custom`; Publish shows them under "From your playtests". Picked 2 → `qa_risky-mode-loner-strike-screenshots/` holds the cover (1280×720) and the 2 picks (320×240) |
| Cover card (local) | PASS | Chromium against my server: "The cover card" 1280×720 is drawn first; it's redrawn after the Name change and again after the line change (data changed both times). Unticked → the package folder has only the 2 picks. With every non-localhost request aborted, the card still drew (no external font requests). Screenshot: initials emblem, name, line, "Made with Loner-Strike" |
| Registering (live) | NEEDS USER | Needs the test GameBanana page. The local refusals work: an untagged real page (mods/739741) → "Add the tag \"Software Used: LonerStrike\" to the page (Edit > Attributes), then try again."; non-GameBanana or `javascript:` links → "Paste the link of your GameBanana page…". Nothing sent, and no `mine.json` written |
| Updating something shared (live) | NEEDS USER (tests pass) | list service `test_updates`, `test_a_newer_file_updates_the_listing`, `test_uids`. **An update can't add risks without asking:** `risk_gate` + `approved_risks` + `not_asking` (communitylib.rs 1563-1580): an update whose graph does more than the installed one stops with "This version does more than the one you have, so it isn't installed by itself…"; unit `the_install_is_bound_to_what_the_player_saw` passes |
| Unlisting/deleting (live) | NEEDS USER | live probe: `POST /community/unlist {key: mod:1}` → 404 "That creation isn't on the list."; junk body → 400 "Say which creation and what to do."; `GET /community/submission/mod/1` → `state: unknown` |

## Importing
| Row | Result | Evidence |
|---|---|---|
| Happy path (map) | NEEDS USER | Needs a listed GameBanana page. Get more's local install of `good.zip` → "Installed qa_good" (the same installer job) |
| Happy path (mode with kit) | NEEDS USER | `install_mode` is reached only from a GameBanana download; unit `mode_packages_carry_one_good_graph`, `mode_clashes_and_remix_ids` pass |
| A mode with risky nodes | PASS (badges, words) / NEEDS USER (the ask after a real download) | Published `qa_risky` (Run Server Command + Kick) → `loner-strike.json` `capabilities: ["server_commands","kicks_players"]`. In the browser (offline index), the QA Loud card shows "Settings only", "Runs server commands" and "Can kick players"; an unknown `wipe_disk` code is dropped. The trust confirm text: "This mode does more than its own game: runs server commands; can kick players…". A plugin mode shows "Plugins" and `allowed: false`. The details page and Try it need GameBanana (offline: "Couldn't reach GameBanana. Check your internet connection." with Try again, no raw error) |
| Plays on / Needs / Get everything (live) | NEEDS USER | unit `a_modes_page_says_what_it_plays_on_and_needs` passes |
| A missing file at run time | NEEDS USER | needs the client or a sim with an installed community mode |
| Missing content | PASS (map part, local) | `qa_custom` with a missing model installs and its check counts it (F2 is about naming). The mode part: NEEDS USER |
| CS 1.6 item into CZ and back | **FAIL** (F5) | no `games` check in `start_try` or the UI |
| Name clash | PASS (map) | `clash.zip` with `maps/ls_outpost.bsp` and `de_dust2.bsp`: "Installed qa_clash_new. You already had de_dust2, ls_outpost."; both MD5s unchanged. Mode id clash: unit `mode_clashes_and_remix_ids` |
| Older/newer format | **FAIL** (F4) | the graph part passes |
| Corrupt/partial download, then retry | PASS (tests) / NEEDS USER (Wi-Fi off mid-download) | unit `md5_matches_known_values`, `picks_a_clean_file`, `nested_failure_leaves_nothing`. Offline Try it → "Couldn't reach GameBanana. Check your internet connection." |
| Malicious archives | PASS | Get more local install into the sim: `../../evil.bsp` dropped (nothing outside; silently, see minor findings); `C:\x.bsp` → `maps/x.bsp`; `/etc/qa_x` skipped as `etc/qa_x`, `/tmp/qa4g_abs.bsp` → `maps/qa4g_abs.bsp`; symlink `maps/link.bsp` not created; `dlls/mp.dll`, `addons/x.so`, `setup.exe`, `addons/amxmodx/plugins/evil.amxx`, `maps/evil.bat` skipped with a reason; nested zip: the inner traversal dropped, `qa_nested` installed; bomb (300 MB from 298 KB) → "That download unpacks to 300 MB from a 298 KB file — far more than real game content packs to, so it wasn't unpacked."; 65,600 files → "That download has too many files in it: 65600 — the most is 65536.". After everything, `find` for escaped files in /, /etc, /tmp and the folders above the game: none. Prefab import of the same tricks: nothing outside the library, `cached.wad` not replaced (MD5 same), the new WAD went into `cstrike/` only. Unit `archive::tests` 22 pass |
| Install then uninstall | PASS (Get more, local) | 9 installs (maps and a sounds pack replacing `sound/weapons/ak47_clipin.wav`) then uninstall of each: the game folder's file list is byte-for-byte the same as before (`diff` of 9064 paths: none); the replaced wav's MD5 is back (e01827a3…). The community-library path: NEEDS USER (a real Try it); unit `uninstalling_a_prefab_removes_only_its_files` passes |
| Re-import the same item | NEEDS USER | needs a GameBanana page; the library `update` logic was checked offline (below) |
| While the game is open | NEEDS USER (tests pass) | unit `busy_servers_and_maps_in_use`, `waiting_updates_and_stale_servers_in_the_library` |

## Moderation and ownership
| Row | Result | Evidence |
|---|---|---|
| Unlist and list again (live) | NEEDS USER | list service `test_unlist_then_list_again_with_the_code` passes |
| Unlisted while installed | PASS (offline library) / NEEDS USER (a real second computer) | an installed entry whose index status is `unlisted` → `gone: "The author unlisted this. You can keep yours, or remove it."`; with the index file id raised to 9: no update offered (the listed control item got `update: 9`); Remove → `{"ok":true}` |
| Withdrawn page (live) | NEEDS USER (library note PASS) | an installed entry with `withdrawn` → "This creation's page was withdrawn from GameBanana. You can keep yours, or remove it."; Try it → "This creation was withdrawn from GameBanana (its page or file is gone), so it can't be downloaded any more." |
| Unlisted and then pulled (live) | NEEDS USER | an installed revoked entry (in the item and in top-level `revoked`) → `pulled: true`; list service `test_unlisted_entries_are_still_rechecked` passes |
| A file that vanished (live) | NEEDS USER | - |
| Report (live) | NEEDS USER | no test entry exists to report; unit `reports_carry_only_the_page_and_the_reason` |
| Taken down after install | PASS (offline library) | `pulled: true`, no update offered |
| Ownership after reinstall | NEEDS USER | - |

## Community roles
| Row | Result | Evidence |
|---|---|---|
| Live harness: #qa-live and QA- roles | PASS | `discord-check`: post 200, edit 200, delete 204, QA-Tester given 204, member has it 200, taken back 204. #qa-live overwrites: @everyone deny VIEW, only the bot and the owner allowed. Bot role (pos 25) above Server Moderator (21) and Supporter (18) |
| Roles endpoints (admin token, never printed) | PASS | `lsmod.py roles servers`: server 1 official, `play.lonerstrike.com:27015`, owner linked, applications, discord_mods_decide. `roles log` has the claim, owner, official, settings, link, supporter-remove, discord-role and unlink rows from the user's run. `roles supporters`: 0. Alone Together's token: `GET /roles/server` 200 (official, settings), `/roles/vip` 200 `{"steamids": [], "ttl": 300}`, `/roles/applications` 200 `[]`. No token or a wrong token: 401 |
| Claim | PASSED by the user (not redone) | - |
| Claimed again (re-claim resets official) | PASS (tests) / NEEDS USER live | `test_a_reclaimed_official_server_starts_from_nothing` and `test_claim_needs_the_cvar_and_a_new_claim_replaces_the_token` pass (test_roles 41 OK). Not run live: it would take Alone Together off official |
| /link | PASSED by the user / NEEDS USER (wrong code, 11 minutes, app closed) | `test_codes_run_out_and_wrong_codes_lock_out`, `test_link_relink_unlink` pass |
| Link squatting | NEEDS USER (tests pass) | `test_an_official_link_wins_over_an_unverified_one`, `test_the_player_has_to_be_on_the_server`, `test_moderators_evict_a_link` |
| Supporters, live | PASSED by the user (not redone) | the state is still consistent: owner has no Supporter role on Discord; `/roles/vip` is empty; the users.ini Loner-Strike block has no supporter lines |
| Supporters (Polar test events) | PASS (tests + safe probes) | Unsigned → 401 "bad signature"; a forged `v1` signature → 401; a stale timestamp (−1 h) → 401 "stale"; a junk webhook-id → 401; a `v0` or empty signature → 401. `roles log` and supporters unchanged afterwards. Never forged a valid one. Box settings (values not printed): `POLAR_WEBHOOK_SECRET` set, `POLAR_SUPPORTER_PRODUCTS` = 2 ids (the allowlist), `POLAR_SUPPORTER_DAYS=0` (lifetime). Unit: `test_signature_replay_and_staleness`, `test_only_the_support_products`, `test_no_products_set_means_nothing_counts`, `test_a_full_refund_takes_its_days_off`, `test_a_refund_before_its_payment_and_before_the_grant`, `test_for_good_and_another_minimum`, `test_a_lifetime_supporter_gets_the_role_back_after_a_cancel`, `test_out_of_order_grants`, `test_monthly_and_one_time_stacking` pass |
| VIP pull to Alone Together | PASSED by the user / consistent now | the roles unit on the box is active; users.ini (written 11:13 UTC) has the Loner-Strike block with no supporter lines, matching 0 supporters |
| Role sync | PASS (tests + live state) | `test_every_account_in_turn_and_failures_tried_again`, `test_someone_who_left_gets_it_at_the_next_check`, `test_the_check_stops_when_discord_says_slow_down`, `test_removed_by_hand_and_back` pass. Live: the hand-removed supporter has no Supporter role |
| VIP in game | NEEDS USER | a linked supporter has to join |
| Application, owner | NEEDS USER (Discord buttons) | `test_needs_a_link_and_official_needs_an_official_link`, `test_end_to_end` pass |
| Application, Discord moderators | NEEDS USER | `test_discord_moderators_when_allowed_and_deny_with_reason` passes |
| Admin invites / Delegation / DMs closed, revoke, unclaim | NEEDS USER | `test_invites_run_out`, `test_unclaim_revokes_like_revoke`; Rust `control::rules::tests::community_roles_for_admins_of_their_server_only`, `roles::` 4 pass |
| Log | PASS | `lsmod.py roles log` lists every event from the user's run (above) |

## Offline
| Row | Result | Evidence |
|---|---|---|
| Box/R2 down | PASS | box, R2 and GameBanana unreachable (dead proxy): `browse` answered in 0.016 s with `index: cached` and `gamebanana: offline`; the page shows "GameBanana didn't answer, so this is the Loner-Strike list only. Trying one still needs GameBanana."; unlisted, withdrawn and revoked entries stayed out; a bad kind, a negative id and `../evil` were dropped. No kept copy → `index: none` and the "couldn't be reached" note. "Registering waits and retries": unit `what_the_list_says_to_a_registration` |
| GameBanana down | PASS | the same run: a plain "Couldn't reach GameBanana. Check your internet connection." with Try again; Installed (library) works; nothing hung |
| GameBanana-dependent list rows (a real tagged page) | NEEDS USER | `community_live` (read-only) passes but finds "0 tagged creations": the live index is empty (`items: []`, its `.sha256` matches). A real tagged GameBanana page is needed for Registering, Updating, Try it, Unlist, Withdrawn, Report and Get everything |

## Test suites on 314c7f47
Rust: `tools::community` 41 (includes communitylib 13), `archive::` 22, `tools::requirements` 3, `tools::playshots` 2,
`prefabs` 5, `gamebanana` 8, `roles::` 4, `content::` 7, `risk` 5: all pass. List service (unittest): test_community
133, test_roles 41, test_applications 22, test_caps 3, test_modbot 15, test_serverlist 6, test_backup 4: all OK;
test_publish_check OK.

## Cleanup
- `qa/live.py cleanup`: channels 1571131443671728168 and 1571131442061254688 and role 1571131449208217670 removed;
  `verify-clean` → "clean" (exit 0). No test entries or links were recorded.
- Alone Together, 19:01 UTC: all six `lonerstrike-loner-strike-alone-together*` units active, `lonerstrike-list`
  active, A2S answers "Loner-Strike | Alone Together" on de_dust2, still official. Its main unit's
  ActiveEnterTimestamp is 18:56:43 UTC. That restart wasn't mine (I only read from the box).
- Stopped my own PIDs: dev servers 9679 and 54069, Vite 55935 and 63703, ssh tunnel 59980. Deleted `Sim/QA4G`. No
  sim ran, so there's no crashcheck window.
