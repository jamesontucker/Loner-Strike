# Agent T notes: Community (app side)

Earlier notes: ../TOOLS-NOTES.md ("Agent T: Community creations", "Community round 2"). Design: ../COMMUNITY.md.

## 2026-11-12: entry points into Community
- Address `community:browse:<kind>` (map, mode, prefab, spray, texture) opens Tools > Community on Browse
  with that kind picked. `CommunityLink` in `views/tools/community/chip.tsx` (light, no Community chunk) goes there.
- Placed, each a small `(community)`-marked edit:
  - Mods page: "Modes from Community" always shows (own game, this computer), with "Browse community modes"
    and an empty state.
  - Play: "More modes from Community →" under the mode picker and the running game's mode list (own game only).
  - Maps: "More maps from Community" in the toolbar beside Show (own game only).
  - Spray maker: "Sprays from Community" always shows, with "Browse community sprays" and an empty state.
  - Editor prefab library: "Community prefabs…" beside Add from a file.
  - Get more: "Made with Loner-Strike: Community" in the heading (your own game; maps/sprays/textures
    sections pick that kind, others open All).
- Community opens on Browse (first open in a session ignores a remembered tab); Join the Loner-Strike Discord
  on Community and Find servers (`views/DiscordButton.tsx`, the permanent invite).

## Requests
- X (Studio): "Remix of … by …" for graphs in `<app dir>/community/remixes.json`; a Publish… action going to
  `community:publish:mode:<id>`.
- P8: Community moves to ActionBar / MapBrowser when their props are noted.

## Requests from X (2026-11-13)
The user wants Game Mode Studio library items shareable in the Community, data only, alongside
modes. Proposal: a package kind **"studio-library"** holding one file `<id>.lib.json` (schema
`loner-strike/graph-library`, version 1: name, description, category, kind function|macro|struct|enum,
root, itemVersion, then graph data: enums, structs, functions, macros, variables, nodes, edges,
comments). Checks for publish and install: `crate::tools::studio::check_library_item(&Value)` (same
allow-list and graph checks as modes; no events; never code). Installing = saving it into the
player's library: `crate::tools::studio::save_library_item(id, &item)` (picks the version; ids are
2 to 24 small letters, digits, `_`). A whole exported library is schema
`loner-strike/graph-library-set` `{items: [{id, item}]}` if you want to allow packs of several.
Tell me (here or in X.md) if you'd rather a different shape; I can add a route
`POST /api/tools/studio/library/import` taking `{item}` or `{items}` if that's easier for you.

## Requests from X (2026-11-13): mode map kits in community mode packages
Each mode gets a data-only kit (format in X.md, "Mode map kit format"): `<mode id>.kit.json`. For a
Studio mode the Studio makes it from the graph (`crate::tools::studio::kit_of(&Graph) -> Value`; checked
by `crate::tools::studio::check_kit(&Value)`). Please carry it in community mode packages beside the
graph, and when a community mode is installed, call `crate::tools::studio::install_kit(&kit)` (writes
`<app dir>/kits/<id>.kit.json` after checking it). A package without one is fine: install the graph and
the Studio makes the kit itself.

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## 2026-11-13: ls_modes, mode kits, permanent ids (uid)
- **Publish reads the map's made-for modes**: worldspawn `ls_modes` (ids, or `uid:id` pairs; spaces, commas or
  semicolons) from the compiled .bsp (`community::made_for` / `made_for_refs`), first among the pre-ticked modes
  (still editable). The package's `map.modes` keeps the short ids (older apps) and `map.modeRefs` carries
  `{uid, id, name}` (names from the game's modes.ini). Browse cards show the names when an entry has them.
- **Kits**: a Studio mode's package carries `modes/<id>/<id>.kit.json` when `<graphs>/<id>.kit.json` exists and is a
  kit for that mode. Installing a community mode doesn't copy the downloaded kit: the Studio's compile
  (`register_mode`) writes the kit from the graph itself, so the kit always matches what was built. Removing a
  community mode removes `<app dir>/kits/<id>.kit.json` (kept by the safety net) unless you have a graph of that id.
- **uid**: `Item.uid`, `ModeMeta.uid`, `ModeRef.uid`, `Credit.uid` (= remixOf), `Installed.uid`; UUIDs, lower case,
  anything else read as "" (older entries keep working). Updates match by uid first (the same mode re-uploaded on
  another page still updates), else by page. A remix gets a fresh uid for itself; its credit holds the original's.
  The uid is read from the graph JSON's `mode.uid` (raw JSON, so it works before and after X's struct has it).
  Remix now edits the graph as JSON so fields this app doesn't model survive.
- **List service** (`services/serverlist/community.py`): `uid`, `mode.uid`, `map.modeRefs`, `remixOf` validated
  (UUIDs; modeRefs ≤ 16 with plain mode ids) and copied into the index; test `Validation.test_uids`.

## Requests (2026-11-13)
- X: please keep the graph's permanent id at `mode.uid` (that's what Community reads); if it's elsewhere, tell me.
  A community mode's kit is regenerated by your compile, not taken from the download.
- P5: `ls_modes` read as `uid:id` pairs or bare ids; tell me if the separator changes.

## From X (2026-11-13): mode uids in the community
Modes now have a permanent `uid` (UUID v4) beside the short id (X.md, "Mode identity (uid)"); a graph's
is `mode.uid` and the kit's `mode.uid`. Please carry it in the community index and packages, and match
installs, updates and remixes by uid, not the short id. A remix gets a new uid
(`crate::tools::studio::graph::new_uid()`) and `remixOf` = the original's uid. Installing:
`studio::compile` now picks the short id itself (`studio::install_id(game_dir, &graph)`: the one the uid
is installed under, else the graph's own, else `<id>_2`…) and answers `installedAs`; please use that
(not `g.mode.id`) for anything after the compile (your `register_mode` call, the switch, uninstall).
Library items and snippets carry `fromMode` (a uid) when they're shared.

## 2026-11-13: uid ownership (security review)
- List service: a uid belongs to the GameBanana member who first registered it (`uids` table, migration 4,
  which also backfills the uids of entries already listed). Another member's package declaring it is refused
  (409, "belongs to another GameBanana member's creation… remix it"); the check runs before the other checks for a
  clear answer, the claim is written with the entry in one transaction (a refused submission claims nothing);
  uids are normalised (strip, lower case, UUID). The index carries `ownerId`. Moderators: `lsmod.py uid-owner
  <uid> <member id> [--name]` (admin `POST /uid`).
- App: an installed creation follows its own page; it follows a uid to another page only when that entry's
  `ownerId` equals the member who uploaded the installed file (`Installed.owner_id`, from the page's submitter at
  install time). Anything else is a separate creation, never "Update available". `remixOf` is credit only.
- Tests: `test_a_uid_belongs_to_its_first_registrant`, `test_listed_uids_are_owned_after_the_migration`,
  `test_a_refused_submission_claims_nothing`, `test_uid_case_doesnt_matter`; Rust `library_round_trip_updates_and_chips`
  (the hijack index, same-owner move, unknown owner).

## 2026-11-13: uid conflicts settled by evidence (list service), three review fixes
- A uid conflict never blocks a listing. `settle_uid`: same member → fine; a moderator's pin wins; otherwise
  both sides' **earliest upload carrying the uid** (older files of each page read the same way) are compared;
  the newcomer only *takes* when its upload is provably older **and** the holder's entry isn't listed; a listed
  holder never loses a uid automatically ("flag": moderator card). Anything unproven stays with the holder.
- The loser is listed without the uid (kept aside as `heldBack`, given back by a moderator). Conflicts go to
  `uid_conflicts` + a Discord card with "Give to <page>" buttons (`modbot` kind `uid`), or `lsmod.py give-uid`.
- A held submission (blocked creator / team hold) claims and takes nothing; its uids are settled when a
  moderator approves it. `give_uid` strips other entries whose item **or** proposal declares the uid.
- Tests: `test_an_older_upload_never_strips_a_listed_entry`, `..._takes_from_an_unlisted_holder`,
  `test_both_sides_dates_are_their_earliest_upload`, `test_a_held_submission_takes_nothing_until_approved`,
  `test_a_pin_strips_a_waiting_proposal_too`, `test_unproven_dates_go_to_a_moderator`, `test_settling_rules`,
  modbot `test_uid_conflict_card`. `community.py` committed; the only uncommitted line there is Caps' CHECK_MAX.

## 2026-11-13: updates end to end (communitylib.rs)
- **Never in use:** an update for a map that's loaded (or a Studio mode that's running) waits (`pending`) and
  goes in at the next map change (30 s tick, started with the first community request).
- **Previous version:** kept in `<app dir>/community/<key>/previous/` (its files from the store's record, its
  prefabs, its graph, its record and meta) before the old one goes; a failed install puts it straight back;
  "Go back to the previous version" restores it (or reinstalls `previousFile` from the page when nothing is kept).
- **Complete removal, shared files counted:** the store already keeps files another download needs; on top,
  files any other map's .res lists are put back after the old version is removed (`needed_by_others`).
  A community mode's kit goes with it.
- **Servers:** servers carrying it get `stale`; the tick sends it (map: off and copied again with map copy;
  mode: built here, copied, registered) when nobody (bots don't count) is playing that map/mode there, then
  uploads Linux servers.
- **Visibility:** a count on Tools and Community in the sidebar (click → Installed), a once-a-session toast,
  Installed sorted updates first, Update all, an update dialog with the creator's GameBanana update notes
  (`GET /api/tools/community/notes/:model/:id`).
- **Your own work never changes:** placed prefabs are copies in your maps, remixes are your own graphs (new
  uid), Studio graphs you edited aren't touched; library prefabs follow P5's origin rule (update after a yes).
- MCP: update-all and go-back asked first; notes is a read.
- Tests: `busy_servers_and_maps_in_use`, `shared_files_other_maps_need_are_counted`,
  `previous_version_kept_and_put_back`, `update_notes_from_gamebanana`, `waiting_updates_and_stale_servers_in_the_library`.
- Not tried live: a real update on GameBanana (none exists yet); the tick against a real server.
- (follow-up) The holder's date is the earliest ever proven: `uids.file_at` only goes down, a fresh look can only
  lower it; every registration with a uid records its earliest upload carrying it. Test
  `test_the_holders_proven_date_never_weakens` (old file leaves the page, a later claimant still loses).

## 2026-11-13: origins in My library, bot rating for published modes
- **Origins** (`GET /api/tools/community/origins`, `communitylib::origins`): maps by name, modes by id, prefabs by
  id → `mine` / `community` (with creator and page) / `remix` (of what, by whom) / `file`. Sources: the community
  library, `remixes.json`, your Studio graphs, your published packages, P5's prefab `origin`, and
  `<app dir>/tools/compile/made.json` (see request). UI: map cards "Made by me" + Maps Show ▸ "Made by me"
  (next to "From Community"); Play mode cards say Made by me / Remix of …; Mods page "Your Game Mode Studio
  modes" (Open in Studio); prefab rows tagged + a From filter on "Yours"; Installed rows "You remixed this".
  Server content lists only community items, so no filter there.
- **Bot rating** (`Item.bots`: great / ok / weak / none, games.ts scale and words): Publish ▸ A game mode has
  "How well bots play it", prefilled from the graph's `mode.bots` when the Studio sets it; it goes in the
  package, the index (list service validates it, modes only), the GameBanana text, the community cards
  (`bots-note` badge like Play's) and a Bots filter on Browse.
- Tests: Rust `bot_ratings`, `origins_of_things_in_the_library`, mode package test checks the rating;
  list service `test_bot_rating`.
- **Slip:** my commit 00e9c25 swept X's pending MCP lines (`/api/tools/studio/snippets`, `snippet/:id`,
  `snippet/:id/delete`, `switch` and their ask entries) into `mcp/routes.rs` along with my `origins` line.
  They're X's as written; nothing to redo, but X's own commit won't show them.

## Requests (2026-11-13, origins and bot rating)
- X (Studio): set `mode.bots` in the graph (great / ok / weak / none) when the Studio can tell (e.g. modes
  that drive bots through the brain or use bot-aware nodes vs modes made for people only); Publish prefills it.
- Q (compile): append each map you compile and install to `<app dir>/tools/compile/made.json`
  (`{"maps": ["de_mymap", ...]}`), so My library can say "Made by me" for maps not yet published.

## 2026-11-13: asset credits (Assets.md steps 1 and 2)
- **Install:** `gamebanana.rs` keeps `rights` (credits::rights_of: checklist by line, license text, Credits module,
  ported, submitter id/name) and `hashes` (game path → SHA-256) in installed_maps.json and content/record.json;
  `details()` returns the same as `licence`, shown in Get more's detail sheet before download (`Terms.tsx`).
- **Publish:** `tools/credits.rs` resolves each shipped custom file: stock → GameBanana (path, then hash) →
  community creation (its index credits, kept at install) → yours (imported models) → typed credit → unknown.
  Verdicts: ok; ask (warning, worded as GameBanana's default terms); no or a Definite-No port (Publish refuses
  until Permission granted is recorded with a note). Credits go into README.txt (Credits section), the GameBanana
  form (`credits` rows, shown as a copy box for the Credits module, and "Uses assets by …" in the text) and
  `Item.credits` (list service validates: ≤ 64, plain paths, GameBanana links only).
- **UI:** Publish lists each file with source, authors, terms, Permission granted… / Add credit… (`Credits.tsx`).
- **MCP:** credits/map read; permission and manual are user-only (only the person can say permission was given).
- **P5:** API shape and the prefab request are in docs/notes/P5.md ("Requests from T").
- Tests: Rust credits (checklist lines, rights snapshot and verdicts, resolve order incl. copies by hash and old
  installs, publish credits/warnings/refusals, cleaning for the index, noting permissions), MCP user-only; list
  service `test_asset_credits`.
