# MCP notes (agent "MCP": the AI agents connector)

## Done (committed)
- 2b9ddff1: agents use the whole map editor and the Game Mode Studio (160 tools then): typed edit tools, Checks as
  findings with fixes by id, the Budget, faces, Content Browser, prefabs, kits, Studio drafts edited node by node,
  creature waves, callouts, publish checks; end-to-end test (`mcp/e2e_tests.rs`).
- 09063d44: security review fixes; every route has an explicit policy (`routes::CHANGES` is the reviewed list of
  plain changes, anything else asks; test `every_route_has_an_explicit_policy`).
- 75a3b177: starting a map simulation asks first.

## In progress (2026-11-13, paused for the usage limit): the editor's selection for agents
Not committed yet: written, UI type-checks, Rust not run (the build was broken by X's uncommitted
`tools/studio/gen.rs`, `SCHEMA`/`VERSION`/`json!` not in scope at ~line 4745).
- `app/src-tauri/src/tools/editor/selection.rs` (new): GET/POST `/api/tools/editor/selection`, one selection per
  editor, `seq` and `by` (page/agent), each item in a few words, bounds, center.
- `app/src-tauri/src/tools/editor.rs`: two hookup lines (`pub mod selection;` and `.merge(selection::routes())`,
  marked `// MCP`). The file has lots of other agents' uncommitted lines: commit only these two hunks, from an index
  built from the current HEAD.
- `app/src-tauri/src/mcp/selection_tools.rs` (new): `editor_selection`, `editor_select`, `selection: true` on the
  typed edit tools (listed schemas get the switch), and its test `an_agent_paints_what_the_person_selected`.
- `mcp/catalog.rs`, `mcp/mod.rs`, `mcp/routes.rs` (routes listed, POST on CHANGES), `docs/MCP.md` (section and count).
- `app/ui/src/views/tools/editor/selectionSync.ts` (new) + two lines in `MapEditor.tsx` (`useSelectionSync()`).

Next: when the build is green, `nice -n 15 cargo test -j 6 mcp::` (load under 20), fix, then
`git commit -- <those paths>` (editor.rs by hunk), `git show --stat`. Check the tool count in docs/MCP.md against
`grep -h 'Spec { name:' src/mcp/*.rs | wc -l` (164 when written, with P5's editor_leak).
