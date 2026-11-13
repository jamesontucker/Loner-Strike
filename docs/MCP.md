# AI agents (MCP connector)

Loner-Strike can let coding agents (Claude Code and others) use its Tools over the
[Model Context Protocol](https://modelcontextprotocol.io). They can read maps, check them for bots, build
in the map editor, compile, fix navs, read replays and run the player's own servers.
Code: `app/src-tauri/src/mcp/`. Settings UI: `app/ui/src/views/AgentSettings.tsx`, shown in Settings.

## Safety
- **Off by default.** Settings > AI agents > "Let AI agents connect". While it's off nothing listens.
- **Local only.** It listens on `127.0.0.1:8029/mcp`, or on any free port if 8029 is taken. The setup
  command always shows the real address. This listener is separate from the app window's.
  - A request needs a local `Host` (`127.0.0.1`, `localhost` or `[::1]` with that port).
  - An `Origin` from anywhere else gets 403. That stops web pages, including DNS-rebinding tricks.
- **Token.** Every request needs `Authorization: Bearer <token>`.
  - The token is new each time the app starts, and "New token" makes another one.
  - The token is compared without stopping at the first difference.
  - It's shown only on the computer's Settings page, never on the phone listener.
- **Read-only unless "Allow changes" is on.**
  - Change tools are refused with a message that points to the setting, and they never reach the app.
  - Turning AI agents off also turns "Allow changes" off.
- **The person is asked** in a native dialog in the app ("An AI agent is asking", Allow / Don't allow) before:
  - saving a nav over the `.nav`;
  - updating an existing `.res`;
  - saving the editor project over its file;
  - closing an editor map with unsaved changes;
  - stopping a server, or changing its map or mode, while people are on it;
  - downloading the compiler;
  - compiling into the game's maps folder;
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
  - It reads JSON-RPC lines on stdin and POSTs each one to the running app with `LONER_MCP_TOKEN` and
    `LONER_MCP_PORT`. Each answer goes out as one line on stdout.
  - It mirrors the modern headers, and it keeps the legacy session.
  - It's for agents that only start local programs. It checks for its flag before Tauri starts, so it
    never opens a window or trips the single-instance lock.

## Tools (55)
**Maps**
- `maps_list`, `map_info`, `map_entities`
- `map_nav_summary`, `map_nav_areas` (by ids, place or near a point)
- `map_knowledge`, `map_probe` (floor under a point)
- `map_doctor`, `map_doctor_write_res`*, `map_doctor_copy_files`
- `health_options`, `health_start`, `health_progress`, `health_cancel`, `health_reports`, `health_report`

**Editor**
- `editor_state`, `editor_list_entities`
- `editor_new`*, `editor_open`*, `editor_import_map`*
- `editor_add_box`, `editor_add_cylinder`, `editor_add_entity`, `editor_set_key`
- `editor_command`: any editor op, including add_shape, delete, transform, hollow, clip, set_texture and set_world
- `editor_undo`, `editor_redo`, `editor_checks`, `editor_textures`
- `editor_save`*, `editor_export_map`

**Compile**
- `compile_status`, `compile_setup`*, `compile_start` (* with `install`), `compile_progress`, `compile_cancel`
- `playtest`*

**Nav fixer**
- `nav_open`, `nav_problems`
- `nav_add_area`, `nav_link`, `nav_unlink`, `nav_edit` (any edit, including a problem's fix)
- `nav_undo`, `nav_redo`, `nav_save`*

**Replay:** `replay_list`, `replay_summary` (kills, headshots, deaths to the world, falls with the drop, stuck spots).

**Servers:** `servers_list`, `server_status`, `server_start`, `server_stop`*, `server_change_map`*, `server_set_bots`.

Every tool except the read tools needs "Allow changes". * = may ask the person.

**Resources:**
- fixed: `loner://maps`, `loner://editor/doc`, `loner://health/reports`
- templates: `loner://map/{map}/info`, `loner://map/{map}/doctor`, `loner://map/{map}/nav`, `loner://health/{id}`

**Prompts:** `make_small_arena` (size, theme) and `check_map_for_bots` (map).

## Connect Claude Code
1. In Loner-Strike, open Settings > AI agents and turn on "Let AI agents connect". Turn on "Allow changes"
   if the agent should build or fix things.
2. Click **Copy setup command** and run it in a terminal:
   ```
   claude mcp add --transport http loner-strike http://127.0.0.1:8029/mcp --header "Authorization: Bearer <token>"
   ```
   Or tick "Use the stdio bridge instead":
   ```
   claude mcp add --transport stdio loner-strike --env LONER_MCP_TOKEN=<token> --env LONER_MCP_PORT=8029 -- "<app binary>" --mcp-stdio
   ```
3. After Loner-Strike restarts, the token is new. Run `claude mcp remove loner-strike`, then run the new
   command.

Other clients: the address is `http://127.0.0.1:<port>/mcp` (Streamable HTTP), with the header
`Authorization: Bearer <token>`.

## Manual test recipe
1. **Connect.** Do the steps above, then run `claude mcp list`. `loner-strike` should say "Connected".
   - Turn the setting off: it should fail to connect.
   - Run the setup command with a wrong token: you should get 401, "Wrong or missing token…".
2. **List tools.** In Claude Code, run `/mcp` and pick loner-strike. You should see 55 tools, 3 resources
   and 2 prompts.
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
  `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}' | LONER_MCP_TOKEN=... LONER_MCP_PORT=... target/debug/loner-strike --mcp-stdio`.

Checked this way on 2026-11-12:
- initialize (legacy), modern tools/list, `map_info` and `map_nav_summary` on the real
  de_dust2 (737 areas);
- `editor_new`, then `editor_add_box`, then `editor_checks` (0 issues), through the stdio bridge, across
  both eras;
- 401 with no token, 403 with a foreign Origin.

Claude Code itself was not run against it here, because adding the server changes the user's Claude
configuration.
