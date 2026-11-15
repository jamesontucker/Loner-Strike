# Working on the Tools pages

## One command: every tool in a browser, real game files

```sh
cd app/ui
npm run tools:dev
```

It starts two things and stops both on Ctrl+C:

1. **The Rust Tools routes** over your real Half-Life folder
   (`cargo test serve_tools_dev -- --ignored`, in `app/src-tauri/src/tools/devserve.rs`) on port 8037,
   so it doesn't clash with a running Loner-Strike on 8027. The first run builds the Rust side,
   which takes a few minutes.
2. **Vite**, with `/api` sent to that server.

Then open **http://localhost:5173/tools-dev.html?open=overview**. This is the Tools section on its
own, in the app's layout, with a sidebar of every tool. `?open=` takes any Tools address, and the
address bar follows what's open, so a reload lands on the same page:

| address | opens |
|---|---|
| `overview` | the cards |
| `map:de_dust2` | the map viewer on de_dust2 |
| `map:de_dust2@-400,-800,100` | ...looking at that spot (`,192` on the end sets how close) |
| `navfix:cs_rats2`, `callouts:cs_rats2`, `doctor:de_dust2`, `health:de_dust2` | a tool on a map (any of them also takes `@x,y,z`) |
| `replay:<id>` | a recording |
| `editor`, `spray`, `assets`, ... | any tool by its registry id |

Settings, all optional:

- `LS_TEST_GAME`: the Half-Life folder (the one with `cstrike/` in it). Found by itself for Steam
  under CrossOver or macOS, Windows' default Steam folder and Linux's `~/.steam`.
- `--mod czero`: Condition Zero instead of CS 1.6.
- `LS_DEV_PORT` (8037) and `LS_VITE_PORT` (5173).
- `--no-rust` with `LS_API=http://127.0.0.1:8027`: only Vite, against a server you started yourself
  (for example the app itself, built from this branch).

**Saving writes to that game folder.** The nav fixer, callouts and map doctor save into
`cstrike/maps` like the app does. To keep the real files as they are, point `LS_TEST_GAME` at a copy.
The app's own data (health check reports, editing sessions) goes to a temp folder.

Things that need the app's window answer in words here: file dialogs (map editor open/save, spray
pictures), learning a map, the health check's hidden server. The 3D view's picture button downloads
the picture instead of asking where to save it.

The older harnesses still work for what they were made for: `render-dev.html` (the viewer alone,
for comparing with in-game screenshots, docs/TOOLS-RENDER.md) and `editor-dev.html` (the map editor
alone). `serve_tools_dev` replaces `serve_tools_for_ui` and `serve_editor_for_ui` for everything else.

## Screenshot tests

```sh
cd app/ui
npm run test:tools
```

These are Playwright tests (`tests/tools.spec.ts`, `playwright.config.ts`). They start
`npm run tools:dev` if it isn't already running, then open every tool in the registry (plus the map
viewer, doctor, nav fixer and callouts on a real map) at two window sizes, 1440×900 and 1024×680.
A page fails when:

- the console shows an error, or the page crashes (404s for missing map files are left out);
- the page's error screen is up;
- the page scrolls sideways, or something in the content sticks out past the window (the test
  names the first three that do).

A screenshot of every page goes to `test-results/<test>/`, pass or fail, for looking over by eye.
There's also a check that `?` opens the keys and Go to moves the viewer.

- `LS_TOOLS_MAP` (de_dust2) and `LS_TOOLS_NAV_MAP` (cs_rats2, any map with a nav) pick the maps.
- One test: `npx playwright test -g "navfix:cs_rats2"`; one size: `--project small`.
- The browser is Playwright's Chromium (`npx playwright install chromium` once, if it isn't there).
- A new tool is tested as soon as it has its line in `registry.tsx`.

## Talking to the app from a tool page

Use `views/tools/common/api.ts`, not your own fetch wrappers:

- `get<T>(path, signal)`: JSON that throws `ToolsError` with words for the page, a 200 `{error}`
  included. Use it in TanStack Query `queryFn`s.
- `getBytes(path, signal, notFound?)`: a bundle or picture, with the same errors.
- `tryGet<T>` / `post<T>(path, body)`: never throw. The answer has `error` instead, which is what a
  button handler wants. `post` sends `X-Panel`, which every tools POST route checks.
- `errorText(e)`: words for anything thrown. Never shows "Failed to fetch".
- `busy(key, label, fn)`: the app's spinner and label in the top bar while slow work runs.
- `useBusy()`: `{working, run}` for a button. `run("Saving…", fn)` ignores a second press, and
  `working` is the label to show.

## Addresses, keys and remembering (common/address.ts, common/Shortcuts.tsx)

- A tool with a map calls ``useReportAddress("<id>", map ? `<id>:${map}` : null)`` once. Tools then
  comes back to that map next time, "Copy link to here" in the keys sheet works, and the dev page's
  address bar follows it.
- Tools opens `<id>:<map>@x,y,z` by setting `useToolArg()` to the map and asking the 3D view to look
  at the spot (`useTools.look`). Any tool built on `viewer/Cameras` follows it.
- Keys: add your tool's keys to `SHORTCUTS` in `common/Shortcuts.tsx` (the `?` sheet). `?` and `/`
  work on every page; `/` focuses the page's first visible `input[type=search]`.
- Search: filter lists with `matches(searchWords(q), ...fields)` from `common/search.ts`, so every
  search box behaves the same way.

## Crumbs, Back and focus mode (common/shell.tsx)

- The Tools header shows `Back · Tools › <tool> › <map> › <page>` from the address. A page deeper than its map reports
  that as its address (`health:report:<id>`) and reads it on mount; `useToolCrumbs(tool, [...])` names the crumbs.
- Every address is a history step: crumbs, Back, Alt+←/→ and the mouse's buttons reopen it "exactly" (the tool's page
  starts over on it). Report unsaved work with `useDirty` (store.ts) so they ask first.
- Focus mode (Shift+Mod+Enter, per tool) hides everything but the tool via `html[data-tools-focus]`; size pages by
  their box, not `100vh`, so they fill it.
