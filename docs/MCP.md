# AI agents (MCP connector)

Loner-Strike can let coding agents (Claude Code and others) use its Tools over the
[Model Context Protocol](https://modelcontextprotocol.io). They can read maps, check them for bots, build
in the map editor, compile, fix navs, read replays and run the player's own servers.
Code: `app/src-tauri/src/mcp/`. Settings UI: `app/ui/src/views/AgentSettings.tsx`, shown in Settings.

## Safety
- **Off by default.** Settings > AI agents > "Let AI agents connect". While it's off nothing listens.
- **Local only.** It listens on `127.0.0.1:8029/mcp`, or on the port picked in Settings. If that port is
  taken it uses a free one, and Settings says so. This listener is separate from the app window's.
  - A request needs a local `Host` (`127.0.0.1`, `localhost` or `[::1]` with that port).
  - An `Origin` from anywhere else gets 403. That stops web pages, including DNS-rebinding tricks.
- **Token.** Every request needs `Authorization: Bearer <token>`.
  - The token is new each time the app starts, and "New token" makes another one.
  - **"Keep the same token across restarts"** (off by default) is for HTTP setups. It keeps the token in
    `<app dir>/mcp-token`, readable by this user only (`sys::write_private`).
  - **The stdio bridge needs no token in its config.** While the connector listens, the app writes
    `<app dir>/mcp-connection.json` (`{port, token, pid}`).
    - The file is written atomically, readable by this user only, and rewritten when the token or
      port changes. It's deleted when the connector is turned off, or at start if it's off.
    - `<app dir>` is `~/Library/Application Support/Loner-Strike` on the Mac and the per-user
      `%APPDATA%\Loner-Strike` on Windows.
  - The token is compared without stopping at the first difference.
  - It's shown only on the computer's Settings page, never on the phone listener.
- **Read-only unless "Allow changes" is on.**
  - Change tools are refused with a message that points to the setting, and they never reach the app.
  - Turning AI agents off also turns "Allow changes" off.
- **The person is asked** in a native dialog in the app ("An AI agent is asking", Allow / Don't allow) before:
  - saving a nav over the `.nav`;
  - writing a map's `.res`, or copying a map's missing files into the game;
  - saving the editor project over its file;
  - closing an editor map with unsaved changes;
  - stopping a server, or changing its map or mode, while people are on it;
  - downloading the compiler;
  - compiling into the game's maps folder;
  - compiling a Game Mode Studio mode, which installs it into the game;
  - copying the editor map's models, sprites and sounds into the game;
  - saving callouts (place names) into a map's nav;
  - saving a Studio graph or a prefab over one that's there;
  - starting the game (playtest).

  If it can't tell, it asks anyway: when it doesn't know who's on, or can't read the editor's state. With
  no window, the answer is no.
- **Files are never pathed by agents.**
  - Maps are names, checked with `maps::safe_arg`.
  - Ids are checked before they go into a route.
  - Open, Save as and Export use the system dialog, so the person picks the file.
- **Server control** only covers the player's own dedicated servers on this computer, plus the game they
  host from Play (`"game"`). Servers on Linux machines and servers shared by friends are out. Passwords and
  folders are left out of answers.
- **Activity log.** Settings lists every tool call this run: time, tool, arguments, and whether it was
  done, refused or failed.

## How it works
- Every tool calls the app's own routes in-process. It uses the same axum router the app window uses,
  through its local guard, with `X-Panel: 1`. So the rules, checks and wording are the tools' own, and no
  logic is copied.
- Big answers are trimmed to about 48 KB: long lists are cut, and a `<key>Omitted` count says how many
  went. The nav fixer's whole-nav state comes back as counts.
- **Protocol:** JSON-RPC 2.0 over Streamable HTTP, written in `rpc.rs` (about 300 lines). It's dual-era:
  - **2026-07-28 (modern):** stateless. `params._meta["io.modelcontextprotocol/protocolVersion"]` is
    checked against the `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers (base64 sentinel
    decoded). A mismatch gets 400 / -32020, and an unknown version gets -32022 with the versions we speak.
    `server/discover` is answered, results carry `resultType: "complete"`, and an unknown method gets
    404 / -32601.
  - **2025-11-25, 2025-06-18, 2025-03-26 (legacy):** `initialize` negotiates the version and returns an
    `Mcp-Session-Id`, which the client sends on later requests. An unknown session gets 404, so the
    client starts again.
  - A request with neither a session nor a modern version gets a plain 400 / -32600. That's what a
    dual-era client falls back on.
  - Answers are always one JSON object. There's no SSE, GET and DELETE get 405, and batches are refused.
- **Why no SDK:** the official Rust SDK, `rmcp` (3.10.0 at the time), would add chrono, tracing,
  indexmap, schemars and second copies of sha2, rand and base64. That's a lot for a size-checked app,
  when the app already has axum and serde. The protocol part we need is small and fully tested.
  `tower` moved from dev-dependencies to dependencies; it was already in the tree through axum.
- **stdio bridge:** `loner-strike --mcp-stdio` (`stdio.rs`, standard library only).
  - It reads JSON-RPC lines on stdin and POSTs each one to the running app. Each answer goes out as one
    line on stdout.
  - The port and token are looked up for every message. `LONER_MCP_TOKEN` and `LONER_MCP_PORT` win if
    they're set; otherwise it reads `mcp-connection.json`. A config of just `<app> --mcp-stdio` keeps
    working across restarts and new tokens.
  - When the app isn't running, or the connector is off, it answers with a JSON-RPC error that says so
    and how to turn it on.
  - It mirrors the modern headers and keeps the legacy session. If the app restarted (its sessions are
    gone, so 404), it re-sends the client's `initialize` and retries, and the client never notices.
  - It's for agents that only start local programs. It checks for its flag before Tauri starts, so it
    never opens a window or trips the single-instance lock.

## What agents can't do
Agents can do anything the person can do in the app. They go through the same routes and the same
handlers, under these rules (`mcp/routes.rs`, tested in `what_agents_may_do_with_each_route` and
`app_call_follows_the_routes_rules`).

**The person's alone.** These are refused even with "Allow changes" on, and the agent is told it's
the person's call:
1. The AI agents settings themselves: turning agents on, Allow changes, the token and the port
   (`/api/mcp*`).
2. App updates, restarting or removing Loner-Strike, and setting up, repairing or removing the game's
   whole setup (`/api/app/update*`, `/api/app/remove-everything*`, `/api/app/restart`;
   `/api/setup/install` with `repair` or no components; `/api/setup/uninstall` with no components).
   Installing or removing *named add-ons* (MonsterMod and the like) is allowed, and asks first.
3. Phone access, shared control (inviting, joining, opening ports) and sharing links
   (`/api/app/phone*`, POST `/api/control/*`, POST `/api/share`).
4. Showing or setting server console passwords and website-status secrets.
5. A server's extra start options (`extra_args`, they can run anything), putting back all of
   Loner-Strike's default files and clearing old backups (`/api/safety/defaults`, `/api/safety/clear`).
6. Any route not listed in `routes.rs` (default deny), and any path that isn't plain: `.` or `..` parts,
   empty parts, `%` escapes, backslashes, `?`/`#`, non-ASCII (`routes::canonical`; app_call sends
   exactly the checked path, and a query only with GETs). Test `path_tricks_are_refused`.

Routes that hand the game a raw console line or a setting it runs (`/api/servers/:id/console/send`,
`/api/app/mode-settings`) go through the same classifier as `console_send`. No route is weaker through
`app_call` than through its typed tool: `typed_tools_are_never_stronger_than_app_call` runs every typed
tool and classifies each request it makes.
   `cargo test mcp_route_coverage -- --ignored` lists any route that isn't classified yet.

**Every route has a policy someone chose** (test `every_route_has_an_explicit_policy`): a POST that no rule
catches is a plain change only when it's on the reviewed list `routes::CHANGES` (edits to the open map or a nav
session, new things, settings easy to change back); a route added later is asked about until it's reviewed.
Installing into the game (plugins, sounds, radar, spray, a Studio mode, a map's files and `.res`), renaming or
deleting files, throwing away unsaved work, switching the game or its folder, copying maps and teaching bots a map
all ask. Applying or dropping a waiting prefab update is the person's alone.

**Asked in the app every time** ("An AI agent is asking", Allow / Don't allow; with no window the answer
is no):
- deleting or removing anything: servers, maps, demos, backups, projects, plugins, prefabs, graphs,
  add-ons, sound packs, presets;
- clearing, resetting and restoring;
- kicking, banning and slaying;
- stopping, restarting, updating or rebuilding a server;
- changing the map while people play;
- starting the game, or closing it to start it again;
- anything public or online: GameBanana installs and community "try it", community publish and
  register, website status and Discord, direct connect and port forwarding, opening URLs or folders,
  SSH to other machines, sending navs to servers, restoring backups.

**Never shown:** passwords, RCON passwords, tokens, keys, webhooks, Steam ids, invite codes and QR codes.
They're removed from every tool's answer (`routes::redact`, test `secrets_never_reach_agents`), and
from console lines (`hide_passwords`).

**Read-only until "Allow changes":** GETs, the read tools, and a console allowlist. With changes off, the
console takes only:
- `status`, `version`, `stats`;
- `meta list`, `amxx list` / `plugins` / `modules` / `cvars`;
- `ls_mode status`, `ls_creature list` / `status` / `types`, `listmaps`;
- `cvarlist <prefix>`;
- a setting's name on its own to read it (`bot_ls_*`, `ls_creature_*`, `sv_*` except `sv_password`,
  `mp_*`, and `bot_*` settings, not bot commands).

With changes on, any single console line goes and is logged, but `quit`, `exit`, `killserver`, `kick`,
`banid`, `addip`, `amx_kick`, `amx_ban`, `amx_slay`, `writeid`, `writeip`, `sv_password`, `map`,
`changelevel`, `restart`, `exec` and `sv_downloadurl` ask first. Never allowed: `;` chains,
`rcon_password` and `rcon`.

## Tools (160)
R = read (works with changes off). C = needs Allow changes (edits to the open map or draft are undoable).
A = needs Allow changes and may ask the person.

| Area | Tools |
|---|---|
| **The whole app** | `app_routes` (R: every route, what it's for, what an agent may do with it), `app_call` (GET = R; POST under the rules above; `target` aims "the game" routes at one of your servers) |
| Maps | `maps_list`, `map_info`, `map_entities`, `map_nav_summary`, `map_nav_areas`, `map_knowledge`, `map_probe` (R) |
| Map doctor | `map_doctor` (R), `map_doctor_write_res`, `map_doctor_copy_files` (A) |
| Map simulator (was "health check") | `health_options`, `health_progress`, `health_reports`, `health_report` (R); `health_start`, `health_cancel` (C) |
| Map editor: files | `editor_state` (brushes, entities, groups, world keys, visgroups and cordon under `editor`), `editor_list_entities`, `editor_history` (R); `editor_export_map`, `editor_history_jump`, `editor_undo`, `editor_redo` (C); `editor_new`, `editor_open`, `editor_import_map`, `editor_save` (A) |
| Map editor: building | `editor_add_box`, `editor_add_cylinder`, `editor_add_shape` (box, cylinder, wedge, spike, cone, sphere, arch, torus with every setting), `editor_add_entity`, `editor_set_key`, `editor_set_keys`, `editor_transform` (move, turn, stretch, pivot, texture lock, scale lock), `editor_move_face`, `editor_extrude_face`, `editor_vertices` (move, split, merge corners), `editor_clip`, `editor_hollow`, `editor_carve`, `editor_merge`, `editor_snap`, `editor_delete`, `editor_duplicate`, `editor_array`, `editor_paste`, `editor_group` (group, ungroup, hide, lock, rename, move into), `editor_tie` (to an entity / back to the world), `editor_world` (WADs and worldspawn keys), `editor_replace`, `editor_arrange` (align, distribute), `editor_settings` (visgroups, cordon), `editor_command` (any editor op) (C); `editor_find` (by text, texture, class, value, touching, inside) (R) |
| Map editor: textures | `editor_textures`, `editor_faces` (each face's index, facing, texture and alignment) (R); `editor_set_texture`, `editor_align_texture` (set, shift, scale, rotate, Fit, Justify, Center, to the world/face/view), `editor_wrap_texture` (C) |
| Map editor: Checks and Budget | `editor_findings` (R: structured findings: id, kind, severity, message, location, brush/entity/face ids, hint, fixes by `fixId`; the stretched-texture limits as arguments), `editor_checks` (R: the raw panel answer), `editor_budget` (R: every bar with status, the fix and the biggest contributors with ids; precache with the game's share, the modes' creatures and the guard's reserve; red-bar warnings); `editor_apply_fix` (C: one or several fixes by id as one undo step); `editor_leak` (R: the last leak line or a loaded pointfile, its hole `{at, index}`, the world brushes round it `nearest [{brush, distance}]`, brush entities near it, the entity it starts at, advice) |
| Content Browser and prefabs | `content_list` (models, sprites, sounds with sources), `content_credits` (who made it, may it be shared), `editor_assets` (files the map names: found or missing), `prefabs_list` (R); `editor_place_asset`, `editor_place_prefab`, `prefab_save` (C; A when replacing); `editor_copy_assets` (A into the game). Recording permission or typing a credit is the person's alone |
| Gameplay panel | `components_library`, `components_pack`, `editor_kits` (R); `components_load_pack`, `editor_place_marker`, `editor_made_for` (C); `components_save_pack` (A) |
| Compile and playtest | `compile_status`, `compile_progress` (R: steps, problems in plain words with brush/entity ids, the leak line) ; `compile_start` (fast, normal, final), `compile_cancel` (C; A with install); `compile_setup`, `playtest` (A) |
| Nav fixer | `nav_open`, `nav_problems`, `nav_reach` (R); `nav_add_area`, `nav_link`, `nav_unlink`, `nav_edit`, `nav_undo`, `nav_redo` (C); `nav_save` (A) |
| Callouts | `callouts_read`, `callouts_suggest` (R); `callouts_save` (A) |
| Replay | `replay_list`, `replay_summary`, `replay_trace` (a bot's trace in a time window; `every` gives its path) (R) |
| Servers and play | `servers_list`, `server_status` (R); `server_start`, `server_set_bots` (C); `server_stop`, `server_change_map` (A). Everything else (making and configuring servers, rotation, HLTV, admins, the web panel's controls, launching the game) goes through `app_call` |
| **Console and logs** | `console_send` (allowlist above), `console_log` (console / amxx / errors; `contains`, `prefixes`, `errors`; `cursor` to follow) |
| **Bot brain** | `brain_settings`, `brain_trace_read`, `mode_status`, `creatures_list` (R); `brain_set` (typed switches: `bot_ls_trace`, `bot_ls_surf` 0/1/2, `bot_ls_fight`...), `brain_trace` (on/off plus `ls_mark` notes) (C) |
| Recorder | `recording_status` (R), `recording_set` (C: pause or resume gm_simrec where it's loaded) |
| Game Mode Studio: graphs | `studio_status`, `studio_nodes`, `studio_graphs`, `studio_graph_read`, `studio_validate`, `studio_code` (paged; also the "does it compile" check, nothing installed) (R); `studio_graph_save`, `studio_graph_delete`, `studio_uninstall` (A) |
| Game Mode Studio: editing | `studio_draft` (R: every node with its pins resolved, wires, variables, Functions, Macros, structs, enums, comments, markers, problems); `studio_draft_open` (saved, new or a template), `studio_edit` (add/remove nodes and wires, pin values, breakpoints, variables, Functions, Macros, structs, enums, comments, map markers, mode settings, snippets; all or nothing, one undo step), `studio_draft_undo`, `studio_draft_redo` (C); `studio_draft_save` (A) |
| Game Mode Studio: build and debug | `studio_debug_events` (cursor, kinds) (R); `studio_debug` (breakpoints, vars, resume) (C); `studio_compile` (installs: A), `studio_playtest` (A). `draft: true` on validate, code, compile and playtest uses the open draft |
| Game Mode Studio: reuse | `studio_snippets`, `studio_library`, `studio_kits` (R); saving a snippet or a library piece through `app_call` (asks) |
| Creature Studio | `creature_types`, `creature_type_read`, `creature_waves` (R); `creature_type_save` (types, tiers and enrage), `creature_waves_save`, `creature_apply` (C). Map-placed hazards (barnacle, sentry, mini turret) are entities: `components_library`, `editor_add_entity` |
| Plugin Studio | `plugins_list`, `plugin_read` (paged), `plugin_log` (cursor) (R); `plugin_compile` (C); `plugin_save`, `plugin_delete`, `plugin_try`, `plugin_untry` (A) |
| Publish | `publish_check` (R: a map, mode or prefab); building and registering packages through `app_call` (asks) |
| Safety net | `backups_list` (R), `backup_restore` (A); compare, undo and defaults through `app_call /api/safety/*` |
| Radar and demos | `radar_preview` (R: returns the picture), `demos_list` (R); install, restore, details, watch and to-replay through `app_call` |
| Docs | `docs_read` (R, paged): components, loner-inc, creatures, bot-people, mcp, gamemodes-plan, studio-nodes |

**Fixing what Checks finds, as an agent.** `editor_findings` gives each finding a short `id` and each one-click
fix a `fixId`, both worked out from what the finding is about (kind, brush, entity, face, texture, place), so they
stay the same while the map changes elsewhere. `editor_apply_fix {fixId}` (or `fixIds` for several in one undo step)
re-reads the checks, finds the fix and sends it the way the Checks panel's button does. Fixes that place something
(a spawn, a light, a mode marker) use `origin`, or the finding's location; "Replace with the current texture" needs
`texture`. The answer says how many findings are left.

**The Budget, as an agent.** `editor_budget` answers what the panel draws, including what the page adds on its
own: the game's share of the precache (197 models, 315 sounds, measured), the creatures of the modes the map is made
for, and the creature guard's reserve (40 models, 60 sounds). Each bar has `status` (green under 75%, yellow, red
over 90%), the usual `fix` and `top` contributors with their brush and entity ids.

**Editing a Studio graph, as an agent.** `studio_draft_open` loads a graph (or a new one, or a template) into a draft
in the app's memory; `studio_edit` takes a list of edits and applies all of them or none (refusing, say, a wire
between pins whose types don't fit), as one undo step; `studio_draft` reads it back with every pin's type and
wires. Nothing is written until `studio_draft_save`, which asks before it replaces a graph file. The Studio page
doesn't show an agent's draft until it's saved.

Bot people, the crosshair and config builder, the community browser, the project panel, GetMore / GameBanana, add-ons, sounds, voice, smarter chat, admins, lists, extras, scrims, the
server browser, backups, map copy and nav learning are all reachable through `app_call`. Use
`app_routes` with a search word to find the route.

**Resources:**
- fixed: `loner://maps`, `loner://editor/doc`, `loner://health/reports`, and `loner://docs/<name>` for
  each doc;
- templates: `loner://map/{map}/info`, `loner://map/{map}/doctor`, `loner://map/{map}/nav`, `loner://health/{id}`.

**Prompts:** `make_small_arena`, `check_map_for_bots`, `debug_bots_on_map`, `make_game_mode`, `fix_map_nav`.

Long answers are cut to about 48 KB. Logs, traces, debugger events and plugin logs take a `cursor`;
docs, code and files take an `offset`.

## Debugging with an agent
This is what we do ourselves, step by step, with an agent. Example: bots keep falling on surf_ski_2 and
get stuck on cs_rats2.
1. **What's known.** Call `map_doctor`, `map_nav_summary`, `nav_reach` and `map_knowledge`, then
   `health_reports` / `health_report` for earlier checks.
2. **A map simulation.** `health_start {map, minutes: 5, bots: 12}` runs the bots on a hidden server. Poll
   `health_progress`, then read `health_report`. It lists falls, stuck spots and deaths to the map by
   place, plus console errors.
3. **The recording.** `replay_list`, then `replay_summary {id}` for kills, falls with the drop, and stuck
   spans. Then `replay_trace {id, bot, from, to}` around a fall shows the brain's task, goal, why and
   speed row by row; `every: 5` gives the bot's path.
4. **Live, on the person's game or server** (with their OK for changes):
   - `brain_settings`, then `brain_trace {on: true, note: "before"}`.
   - Let it play, then `brain_trace_read {bot, cursor}` to follow one bot.
   - For surf, `brain_set bot_ls_surf 2`, then
     `console_log {source: "console", prefixes: ["LSSURF"], cursor}` every ~10 s.
   - Plugin errors: `console_log {source: "errors"}`. Game mode lines: `prefixes: ["[gm_"]`.
   - `mode_status` and `creatures_list` show what the mode told the brain.
5. **Your own mode or plugin.**
   - Game Mode Studio: `studio_playtest {id, map, breakpoints}`, then follow
     `studio_debug_events {mode, from}` (node inputs and results, errors, breakpoint hits). Use
     `studio_debug {vars: true}` for live variables and `studio_debug {resume: true}`.
   - Plugin Studio: `plugin_try`, then follow `plugin_log {name, cursor}`.
6. **Fix and check again.** Use the nav fixer tools, components or brain settings, then run another
   map simulation.

## Connect an agent
1. In Loner-Strike, open Settings > AI agents and turn on "Let AI agents connect". Turn on "Allow changes"
   if the agent should build or fix things.
2. Under **Connect an agent**, pick your client and click **Copy**, then paste it where the line under the
   picker says. These setups use the stdio bridge, need no token, and stay valid. The app binary on a
   Mac is `/Applications/Loner-Strike.app/Contents/MacOS/loner-strike`; the copied setups have the real
   path.

All of these formats were checked against each client's docs on 2026-11-12. Codex, OpenCode, Cursor and
VS Code came from their current pages. The Claude Desktop format came from modelcontextprotocol.io.
Claude Code's `claude mcp add` comes from its CLI.

| Client | Where | stdio setup (preferred) |
|---|---|---|
| Claude Code | terminal | `claude mcp add --scope user --transport stdio loner-strike -- "<app>" --mcp-stdio` |
| Codex | `~/.codex/config.toml` | `[mcp_servers.loner-strike]` / `command = "<app>"` / `args = ["--mcp-stdio"]` |
| OpenCode | `opencode.json` | `"mcp": {"servers": {"loner-strike": {"type": "local", "command": ["<app>", "--mcp-stdio"]}}}` |
| Cursor | `~/.cursor/mcp.json` | `"mcpServers": {"loner-strike": {"type": "stdio", "command": "<app>", "args": ["--mcp-stdio"]}}` |
| Claude Desktop | `claude_desktop_config.json` (Settings > Developer > Edit Config) | `"mcpServers": {"loner-strike": {"command": "<app>", "args": ["--mcp-stdio"]}}` |
| VS Code | `.vscode/mcp.json` or MCP: Open User Configuration | `"servers": {"loner-strike": {"type": "stdio", "command": "<app>", "args": ["--mcp-stdio"]}}` |
| Others (Windsurf…) | the client's MCP config | the `mcpServers` / `command` / `args` shape |

**HTTP instead:** tick "Connect over HTTP with the token instead". For HTTP, turn on "Keep the same token
across restarts" and pick a port that's free, or copy the setup again after each restart.
- Claude Code: `--transport http ... --header "Authorization: Bearer <token>"`.
- Codex: `url` plus `http_headers = { "Authorization" = "Bearer <token>" }`.
- OpenCode: `"type": "remote"`, `"oauth": false` and `headers`.
- Cursor: `url` and `headers`.
- VS Code: `"type": "http"`, `url` and `headers`.
- Claude Desktop's config file only starts local programs, so it uses stdio.

Any other client: `http://127.0.0.1:<port>/mcp` (Streamable HTTP), with the header
`Authorization: Bearer <token>`.

## Manual test recipe
1. **Connect.** Do the steps above, then run `claude mcp list`. `loner-strike` should say "Connected".
   - Turn the setting off: it should fail to connect.
   - With the HTTP setup and a wrong token you should get 401, "Wrong or missing token…".
   - Restart Loner-Strike: the stdio setup should still connect, with no change to the config.
2. **List tools.** In Claude Code, run `/mcp` and pick loner-strike. You should see 160 tools, 10 resources
   and 5 prompts.
3. **Read a map.** Ask: "Use loner-strike to tell me about de_dust2: its spawns, nav and whether bots know
   it". The agent should call `map_info`, `map_nav_summary` and `map_knowledge`. Settings > AI agents >
   "What agents did" lists the calls.
4. **Read-only.** With "Allow changes" off, ask it to add a box in the map editor. The tool should answer
   that changes are off, and the log should say "Not allowed".
5. **Make a box.** Turn on "Allow changes" and open Tools > Map editor. Ask: "Start a new map and add a
   128-unit crate in the middle". The agent should call `editor_new` and then `editor_add_box`.
   - The editor page shows the new brush when it refreshes. Reopen the tab if it doesn't redraw, or see
     the request in TOOLS-NOTES.
   - Then ask it to run `editor_checks` and `editor_export_map`. The save dialog appears for you.
6. **Ask first.**
   - Make an unsaved change in the editor, then ask the agent for a new map. A dialog asks you, and
     "Don't allow" leaves the map open.
   - Ask it to save a nav after a `nav_link`: it asks every time.
   - Start one of your servers, join it, then ask the agent to stop it: it asks first.
7. **Prompts.** `/loner-strike:check_map_for_bots cs_rats2` (the name may differ by client) runs the
   read-only check.

**Without the app** (for working on the connector):
```
LONER_MCP_TOKEN=trytoken0123456789abcd LONER_MCP_CHANGES=1 cargo test serve_mcp_for_try -- --ignored --nocapture
```
- This serves the real routes with your game on 127.0.0.1:8029, or the printed port. Questions for the
  person are answered "Don't allow".
- Try it with `curl` (examples in the test), or with the bridge:
  `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}' | target/debug/loner-strike --mcp-stdio`. It finds the harness through `mcp-connection.json`. That file
  stays behind when the harness is killed, so delete it afterwards; the app removes a stale one at start.

Checked this way on 2026-11-12:
- initialize (legacy), modern tools/list, `map_info` and `map_nav_summary` on the real
  de_dust2 (737 areas);
- `editor_new`, then `editor_add_box`, then `editor_checks` (0 issues), through the stdio bridge, across
  both eras;
- 401 with no token, 403 with a foreign Origin.

Claude Code itself was not run against it here, because adding the server changes the user's Claude
configuration.
