#!/usr/bin/env node
// Screenshots for the website (site/img/1.0/), taken the same way every time (docs/notes/Site.md,
// "Screenshots"):
//   node scripts/site-shots.mjs                 every shot below
//   node scripts/site-shots.mjs editor studio   only these (names, or a group: @editor, @studio)
//   --list        the shots and what each shows
//   --keep        keep the throwaway folder (game clone, app data) for the next run
//   --headed      watch it work
//   --verbose     say each step
// LS_TEST_BIN: use this test build of the app instead of building the working tree's.
//
// Nothing of yours is used or changed: the game is an APFS clone of your Half-Life folder
// (copy-on-write: no space, and edits land in the clone), the app's data is a new empty folder,
// Vite has its own cache, and the servers listen on their own ports (8139, 5279). Content comes
// from the repo (maps-src, the Studio's examples) and from recordings of real runs kept in
// app/ui/scripts/site-shots/.
//
// Each shot: a 1440×900 window (the site's 16:10 frame), the app's own dark theme; saved as
// <name>.webp (1440 wide, cwebp -q 80), <name>.png (1200 wide; 256 colours when pngquant is
// installed, else full colour) and thumb/<name>.webp (320 wide). Needs cwebp (brew install webp).

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { connect } from "node:net";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const ui = resolve(here, "..");
const repo = resolve(ui, "../..");
const tauri = resolve(ui, "../src-tauri");
const OUT = process.env.SITE_SHOTS_OUT || join(repo, "site/img/1.0");
const DATA = join(here, "site-shots");
const args = process.argv.slice(2);
const flag = (f) => args.includes(f);
const W = 1440, H = 900;
const RUST_PORT = 8139, VITE_PORT = 5279;
const tmp = join(tmpdir(), "ls-site-shots");

const GUESSES = [
  join(homedir(), "Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life"),
  join(homedir(), "Library/Application Support/Steam/steamapps/common/Half-Life"),
];

// ---- the shots ------------------------------------------------------------------------------

const H_PANEL = { "X-Panel": "1", "Content-Type": "application/json" };
const api = (page, path, body) =>
  page.evaluate(
    async ([p, b, h]) => (await fetch(p, b === undefined ? {} : { method: "POST", headers: h, body: JSON.stringify(b) })).json(),
    [path, body, H_PANEL],
  );
const settle = (page, ms = 1200) => page.waitForTimeout(ms);
const step = (what) => flag("--verbose") && console.log(`  · ${what}`);

/** The editor with the shot's map imported (`pick`: the dev server answers the file dialog with
 *  a copy of that maps-src file, so the map keeps its own name). */
async function editorWith(page, layout = "single") {
  await page.addInitScript((l) => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.editor.panes", JSON.stringify(l === "quad" ? { layout: "quad", views: ["3d", "top", "front", "side"] } : { layout: "single", views: ["3d"] }));
  }, layout);
  step("open the editor");
  await page.goto(`http://127.0.0.1:${VITE_PORT}/editor-dev.html`);
  step("import the map");
  const r = await api(page, "/api/tools/editor/import-map", {});
  step("imported");
  if (r.error) throw new Error(`import: ${r.error}`);
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 8000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await settle(page, 3000);
}

/** A menu command, as a person picks it (Select › Hide triggers and zones). */
async function menu(page, top, item) {
  await page.getByRole("menubar", { name: "Editor menu" }).getByRole("menuitem", { name: top, exact: true }).click();
  await page.getByRole("menuitem").filter({ hasText: item }).first().click();
  await settle(page, 600);
}

/** Flies the 3D view: the wheel moves forward (positive steps) or back, over the pane's middle. */
async function fly(page, steps, pane = 0) {
  const b = await page.locator(`.ed-pane[data-pane="${pane}"]`).boundingBox();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  for (let i = 0; i < Math.abs(steps); i++) {
    await page.mouse.wheel(0, steps > 0 ? -120 : 120);
    await page.waitForTimeout(60);
  }
  await settle(page, 800);
}

/** A dock tab by its name (Content, Checks, Budget, Compile...). */
async function dockTab(page, name) {
  const tab = page.locator("[data-dock-strip] [role=tab]", { hasText: new RegExp(`^${name}$`) }).first();
  if (await tab.count()) await tab.click();
  else await page.getByRole("tab", { name, exact: true }).first().click();
  await settle(page, 800);
}

/** Tools pages as someone who's used them a while sees them: in Focus (the window is the tool),
 *  the Focus hint and the Studio's tour offer already seen. */
async function toolsPrefs(page) {
  await page.addInitScript(() => {
    const tools = ["studio", "creatures", "plugins", "editor", "navfix", "callouts", "radar", "map", "doctor", "spray", "playercfg", "people", "models", "replay", "sim", "community", "overview"];
    localStorage.setItem("ls.tools.focus", JSON.stringify(Object.fromEntries(tools.map((t) => [t, true]))));
    localStorage.setItem("ls.tools.focusHintSeen", JSON.stringify(tools));
    localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: true }));
  });
}

/** King of the Hill's round start and scoring chain, framed at a readable zoom. */
async function frameHill(page) {
  // the scoring part of the graph, where a capture is handled (as a click on its event does)
  await page.getByText("Round Started (2)", { exact: true }).first().click().catch(() => page.locator("text=/Round Started\\s+\\(2\\)/").first().click());
  await settle(page, 1500);
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  // a step back: the capture's whole chain in view, still readable
  for (let i = 0; i < 3; i++) await page.locator(".react-flow__controls-zoomout").click();
  await settle(page, 600);
  // the chain runs to the right: slide it into view (a drag on empty canvas)
  const c = await page.locator(".react-flow__pane").boundingBox();
  await page.mouse.move(c.x + c.width * 0.35, c.y + c.height * 0.85);
  await page.mouse.down();
  await page.mouse.move(c.x + c.width * 0.35 - 260, c.y + c.height * 0.85 + 40, { steps: 10 });
  await page.mouse.up();
  await settle(page, 800);
}

/** The Studio on an example mode (opened as a person would: Make it mine keeps a copy). */
async function studioWith(page, file, id, change) {
  const tpl = JSON.parse(readFileSync(join(ui, "src/views/tools/studio/templates", file), "utf8"));
  if (change) change(tpl);
  await toolsPrefs(page);
  await page.goto(`http://127.0.0.1:${VITE_PORT}/tools-dev.html?open=overview`);
  tpl.mode.id = id;
  const r = await api(page, `/api/tools/studio/graph/${id}`, { graph: tpl });
  if (!r.ok) throw new Error(`save ${id}: ${JSON.stringify(r)}`);
  await page.goto(`http://127.0.0.1:${VITE_PORT}/tools-dev.html?open=studio:${id}`);
  await page.locator(".react-flow__node").first().waitFor({ timeout: 30000 });
  await settle(page, 1500);
  return tpl;
}

/** A Tools page by its address (tools-dev.html?open=...), as the app opens it, in Focus. */
async function tool(page, addr, wait = 4000) {
  await toolsPrefs(page);
  // (tools-dev.html's left list is the dev page's own, never the app's: left out for pages
  // that have no Focus)
  await page.addInitScript(() => document.addEventListener("DOMContentLoaded", () => {
    const st = document.createElement("style");
    st.textContent = "aside.sidebar{display:none!important}.shell{grid-template-columns:1fr!important}";
    document.head.appendChild(st);
  }));
  await page.goto(`http://127.0.0.1:${VITE_PORT}/tools-dev.html?open=${addr}`);
  await settle(page, wait);
}
const plain = (group, addr, about, then) => ({ group, about, async take(page) { await tool(page, addr); if (then) await then(page); } });

/** Community's browser with sample cards: the repo's own maps and example modes, credited to
 *  Loner-Strike (the shipped examples, not anyone's upload; nothing made up about people). The
 *  list comes from the page's request, answered here; the pictures are the site's map renders. */
function sampleCommunity() {
  const day = 86400, now = Math.floor(Date.now() / 1000);
  const tpl = (f) => JSON.parse(readFileSync(join(ui, "src/views/tools/studio/templates", f), "utf8")).mode;
  const content = { level: "content", label: "Content", allowed: true, confirm: null, note: "Only map content is installed (maps, textures, models, sounds); nothing that runs code." };
  const data = { level: "data", label: "Settings only", allowed: true, confirm: null, note: "Settings and rules only: no plugins, nothing that runs code." };
  const base = { model: "Mod", category: "", likes: 0, listed: true, have: false, canTry: true, url: "https://lonerstrike.com", games: ["cstrike"] };
  let id = 1;
  const map = (name, summary, modes, img, ago) => ({ ...base, id: id++, key: `mod-${id}`, kind: "map", name, author: "Loner-Strike", summary, image: `https://site-shots.invalid/${img}`, added: now - ago * day, updated: now - ago * day, map: name, modes, badges: ["featured", "bots", "nav", "checked"], bots: "", trust: content });
  const mode = (file, bots, ago) => {
    const m = tpl(file);
    return { ...base, id: id++, key: `mod-${id}`, kind: "mode", name: m.name, author: "Loner-Strike", summary: m.description, image: null, added: now - ago * day, updated: now - ago * day, map: null, modes: [], badges: ["checked"], bots, trust: data };
  };
  return {
    items: [
      map("ls_outpost", "A walled courtyard with a keep and an armory, made for Last Stand", ["Last Stand", "Classic"], "render-outpost.webp", 2),
      map("br_valley", "A river valley with a town, a factory, farms and bridges, made for Battle Royale", ["Battle Royale"], "render-valley.webp", 3),
      mode("king-of-the-hill.graph.json", "ok", 1),
      mode("infection.graph.json", "great", 1),
      mode("gun-game-lite.graph.json", "great", 4),
    ],
    index: "live", gamebanana: "live", indexUpdated: now - 3600, tag: "Software Used: LonerStrike",
  };
}

/** Textures & models on the AK-47 view model; `own`: the list shows only the game's own models. */
async function ak47(page, own) {
  await tool(page, "assets");
  await page.getByRole("button", { name: "Models", exact: true }).first().click();
  await settle(page, 1500);
  await page.getByPlaceholder(/Search models/i).first().fill("v_ak47");
  await settle(page, 1500);
  // the game's own (not a downloaded skin): the one listed under "cstrike"
  const mine = page.locator("button, li").filter({ hasText: /^v_ak47\s*cstrike$/ }).first();
  if (await mine.count()) await mine.click();
  else await page.getByText(/^v_ak47$/).last().click();
  await settle(page, 4000);
}

const SHOTS = {
  creatures: plain("held", "creatures", "Creature Studio: creature types, one's settings, its model playing"),
  community: {
    group: "held",
    about: "Community: sample cards from Loner-Strike's own maps and example modes",
    async take(page) {
      await page.route(/\/api\/tools\/community\/browse/, (r) => r.fulfill({ json: sampleCommunity() }));
      await page.route(/^https:\/\/site-shots\.invalid\//, (r) => r.fulfill({ path: join(repo, "site/img/1.0", r.request().url().split("/").pop()), contentType: "image/webp" }));
      await tool(page, "community", 5000);
    },
  },
  tools: plain("tools", "overview", "The Tools page: every tool, grouped"),
  plugins: plain("tools", "plugins", "Plugin Studio: a game plugin's code (read only), compiled, with Problems below", async (page) => {
    await page.getByText("antiflood.sma", { exact: true }).first().click();
    await page.locator(".monaco-editor").first().waitFor({ timeout: 30000 });
    await settle(page, 1500);
    await page.getByRole("button", { name: "Compile", exact: true }).click().catch(() => {});
    await page.getByText(/No problems|compiled|Compiled/).first().waitFor({ timeout: 120000 }).catch(() => {});
    await settle(page, 1000);
  }),
  navfix: plain("tools", "navfix:cs_rats2", "Nav fixer on cs_rats2: where bots can go and get back from"),
  doctor: plain("tools", "doctor:de_dust2", "Map doctor on de_dust2: every check, grouped"),
  callouts: plain("tools", "callouts:de_dust2", "Callouts on de_dust2: places painted by name"),
  radar: plain("tools", "radar:de_dust2", "Radar maker on de_dust2"),
  "viewer-nav": plain("tools", "map:de_dust2", "The map viewer on de_dust2 with the bots' nav drawn", async (page) => {
    const sw = page.getByRole("switch", { name: /Bots' nav/ }).or(page.getByRole("checkbox", { name: /Bots' nav/ })).first();
    if (await sw.count()) await sw.click();
    else await page.getByText("Bots' nav", { exact: true }).click();
    await settle(page, 2500);
  }),
  spray: plain("tools", "spray", "Spray maker"),
  playercfg: plain("tools", "playercfg", "Crosshair & config with the crosshair live"),
  people: plain("tools", "people", "Bot people: who the bots are"),
  models: { group: "tools", about: "Textures and models: the AK-47's view model in 3D (the list shows the downloaded skins too)", take: (page) => ak47(page, false) },
  "models-game": {
    group: "tools",
    about: "The same, with only the game's own models (the clone's downloaded models set aside for it)",
    async take(page) {
      // (only in the throwaway clone, and put back after: the real game is never touched)
      const dl = join(started.game, "cstrike_downloads", "models");
      const away = `${dl}.site-shots-aside`;
      const moved = existsSync(dl) && !existsSync(away);
      if (moved) renameSync(dl, away);
      try {
        await ak47(page, true);
      } finally {
        if (moved) renameSync(away, dl);
      }
    },
  },

  editor: {
    group: "editor",
    about: "The map editor on br_valley in 3D, with the outline, Inspect and the Content browser",
    pick: "maps-src/br_valley/br_valley.map",
    async take(page) {
      await editorWith(page);
      await menu(page, "Select", "Hide triggers and zones");
      await fly(page, 14);
      await dockTab(page, "Content");
    },
  },
  "editor-quad": {
    group: "editor",
    about: "Quad layout on ls_outpost: 3D, Top, Front and Side, a brush picked with its handles",
    pick: "maps-src/ls_outpost/ls_outpost.map",
    async take(page) {
      await editorWith(page, "quad");
      const top = page.locator('.ed-pane[data-pane="1"]');
      const b = await top.boundingBox();
      await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
      await settle(page, 1500);
    },
  },
  "editor-checks": {
    group: "editor",
    about: "Checks: the map's problems as you build, each with its fix",
    pick: "maps-src/br_valley/br_valley.map",
    async take(page) {
      await editorWith(page);
      await menu(page, "Select", "Hide triggers and zones");
      await fly(page, 14);
      await dockTab(page, "Checks");
      await page.getByText("Checking the map").waitFor({ state: "hidden", timeout: 120000 }).catch(() => {});
      await settle(page, 800);
    },
  },
  "editor-budget": {
    group: "editor",
    about: "Budget: how close br_valley is to GoldSrc's limits",
    pick: "maps-src/br_valley/br_valley.map",
    async take(page) {
      // the Budget where Inspect is, tall (a layout people make by dragging the tab over)
      await page.addInitScript(() =>
        localStorage.setItem("ls.editor.dock2", JSON.stringify({
          areas: { left: { tabs: [], active: null }, right: { tabs: ["outline"], active: "outline" }, right2: { tabs: ["budget", "inspect", "history"], active: "budget" }, bottom: { tabs: ["content", "library", "textures", "checks", "io", "compile", "project"], active: "content" } },
          sizes: { left: 300, right: 400, bottom: 240, rightSplit: 0.22 }, floats: [], popped: [],
        })),
      );
      await editorWith(page);
      await menu(page, "Select", "Hide triggers and zones");
      await fly(page, 14);
      await settle(page, 2500);
    },
  },
  studio: {
    group: "studio",
    about: "Game Mode Studio on King of the Hill: My mode, the graph at a readable zoom, Details",
    async take(page) {
      await studioWith(page, "king-of-the-hill.graph.json", "site_hill");
      await frameHill(page);
    },
  },
  "studio-timeline": {
    group: "studio",
    about: "A Timeline: the hill's glow pulsing (a color track) with a beat key, in its editor",
    async take(page) {
      await studioWith(page, "king-of-the-hill.graph.json", "site_hill_glow", (g) => {
        g.timelines = [{
          id: "glow", name: "Hill Glow", length: 2, loop: true, autoplay: false, tick: 0.05,
          tracks: [
            { id: "color", name: "Glow", kind: "color", keys: [{ t: 0, v: [255, 200, 40] }, { t: 1, v: [255, 60, 20], ease: "ease" }, { t: 2, v: [255, 200, 40] }] },
            { id: "size", name: "Radius", kind: "float", keys: [{ t: 0, v: 160 }, { t: 1, v: 220, ease: "ease" }, { t: 2, v: 160 }] },
            { id: "beat", name: "Beat", kind: "event", keys: [{ t: 0 }, { t: 1 }] },
          ],
        }];
        // played from the round's start (the hill glows all round)
        g.nodes.push({ id: "glow_start", type: "event.round_started", pos: { x: 0, y: 2600 }, params: {} });
        g.nodes.push({ id: "glow_play", type: "timeline.play", pos: { x: 400, y: 2600 }, params: { timeline: "glow" } });
        g.edges.push({ id: "glow_w", from: { node: "glow_start", pin: "then" }, to: { node: "glow_play", pin: "play_start" } });
      });
      await frameHill(page);
      const item = page.locator("text=\"Hill Glow\" >> visible=true").first();
      await item.scrollIntoViewIfNeeded();
      await item.dblclick({ force: true });
      await settle(page, 1500);
    },
  },
  "studio-help": {
    group: "studio",
    about: "Node help: F1 on a node, with what it does, its pins and an example",
    async take(page) {
      await studioWith(page, "king-of-the-hill.graph.json", "site_hill_help");
      await frameHill(page);
      await page.locator(".react-flow__node", { hasText: "Find Marker" }).first().click();
      await page.keyboard.press("F1");
      await settle(page, 1500);
    },
  },
  "studio-start": {
    group: "studio",
    about: "The Studio's start: templates and the five example modes",
    async take(page) {
      await toolsPrefs(page);
      await page.goto(`http://127.0.0.1:${VITE_PORT}/tools-dev.html?open=studio`);
      await page.getByText("Gun Game Lite").first().waitFor({ timeout: 30000 });
      await settle(page);
    },
  },
  "studio-tables": {
    group: "studio",
    about: "A data table: Gun Game Lite's guns, level by level",
    async take(page) {
      await studioWith(page, "gun-game-lite.graph.json", "site_gungame");
      await page.getByText(/^Data tables/).first().click().catch(() => {});
      await page.locator(".st-sidebar, aside").getByText(/Guns|Levels/).first().dblclick();
      await settle(page, 1500);
    },
  },
};

// ---- servers ---------------------------------------------------------------------------------

const listening = (p) =>
  new Promise((ok) => {
    const s = connect(p, "127.0.0.1", () => (s.end(), ok(true)));
    s.on("error", () => ok(false));
  });
const kids = [];
function stopAll() {
  for (const k of kids) if (k.exitCode === null) k.kill("SIGINT");
}
process.on("SIGINT", () => (stopAll(), process.exit(1)));
process.on("SIGTERM", () => (stopAll(), process.exit(1)));

async function start() {
  const real = process.env.LS_TEST_GAME || GUESSES.find((p) => existsSync(join(p, "cstrike")));
  if (!real) throw new Error("Couldn't find Half-Life. Set LS_TEST_GAME.");
  if ((await listening(RUST_PORT)) || (await listening(VITE_PORT))) throw new Error(`Port ${RUST_PORT} or ${VITE_PORT} is in use: another site-shots run?`);
  // (inside the game's CrossOver bottle when there is one, so the app can run its compilers on it:
  // a copy-on-write clone there costs no space either; the app is told about it as a game folder)
  const bottle = real.includes("/drive_c/") ? join(real.slice(0, real.indexOf("/drive_c/") + 8), "Loner-Strike", "SiteShots") : tmp;
  const game = join(bottle, "Half-Life");
  const app = join(tmp, "app");
  if (!flag("--keep")) rmSync(app, { recursive: true, force: true });
  mkdirSync(app, { recursive: true });
  mkdirSync(dirname(game), { recursive: true });
  if (!existsSync(join(game, "cstrike"))) {
    // copy-on-write clone: the real folder is never written to
    const r = spawnSync("cp", ["-cR", real, game]);
    if (r.status !== 0) throw new Error("Couldn't clone the game folder (needs APFS): " + r.stderr);
  }
  // the app's settings: the clone as a game folder (Plugin Studio's and the editor's compilers run there)
  const settingsFile = join(app, "settings.json");
  let settings = {};
  try {
    settings = JSON.parse(readFileSync(settingsFile, "utf8"));
  } catch {
    /* a new app folder */
  }
  writeFileSync(settingsFile, JSON.stringify({ ...settings, game_folders: [game] }));
  // SDHLT for the editor's Compile, from an earlier dev run when there is one (never downloaded here)
  const sdhlt = join(tmpdir(), "ls-tools-dev", "tools", "sdhlt");
  if (existsSync(sdhlt) && !existsSync(join(app, "tools", "sdhlt"))) {
    mkdirSync(join(app, "tools"), { recursive: true });
    spawnSync("cp", ["-cR", sdhlt, join(app, "tools", "sdhlt")]);
  }
  console.log(`game clone ${game}\napp data ${app}`);
  started = { game, app };
  const cfg = join(tmp, "vite.site-shots.mts");
  writeFileSync(
    cfg,
    `import base from ${JSON.stringify(join(ui, "vite.config.ts"))};
export default { ...base, root: ${JSON.stringify(ui)}, cacheDir: ${JSON.stringify(join(tmp, "vite-cache"))},
  server: { ...base.server, host: "127.0.0.1", port: ${VITE_PORT}, strictPort: true, hmr: false, proxy: { "/api": "http://127.0.0.1:${RUST_PORT}", "/thumb": "http://127.0.0.1:${RUST_PORT}" } } };\n`,
  );
  // (node itself, not npx: npx's child would outlive a stop)
  const vite = spawn(process.execPath, [join(ui, "node_modules/vite/bin/vite.js"), "--config", cfg], { cwd: ui, env: process.env, stdio: ["ignore", "ignore", "inherit"] });
  kids.push(vite);
}

/** The app's test build (built once per run; the dev server is one of its tests). */
let built = null;
function testBinary() {
  // (LS_TEST_BIN: one built already, e.g. from a clean checkout while the working tree is mid-edit)
  if (process.env.LS_TEST_BIN) return process.env.LS_TEST_BIN;
  if (built) return built;
  console.log("building the Rust side (cargo test --no-run)…");
  const r = spawnSync("cargo", ["test", "--no-run", "--message-format=json"], { cwd: tauri, encoding: "utf8", maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "inherit"] });
  for (const l of r.stdout.split("\n")) {
    try {
      const m = JSON.parse(l);
      if (m.reason === "compiler-artifact" && m.executable && m.target?.name === "loner-strike" && m.profile?.test) built = m.executable;
    } catch {
      /* not a message */
    }
  }
  if (!built) throw new Error("Couldn't build the app's tests.");
  return built;
}

/** The Rust Tools server, (re)started when the shot needs another map in the file dialog. */
let started = null, rust = null, rustPick = undefined;
async function rustFor(pick) {
  if (rust && rustPick === pick) return;
  if (rust) {
    rust.kill("SIGINT");
    for (let i = 0; i < 40 && (await listening(RUST_PORT)); i++) await new Promise((r) => setTimeout(r, 250));
  }
  const env = { ...process.env, LS_TEST_GAME: started.game, LS_DEV_PORT: String(RUST_PORT), LS_APP_DIR: started.app };
  if (pick) {
    // a copy, named as the map: the editor names the document after the file
    const copy = join(tmp, "maps", pick.split("/").pop());
    mkdirSync(dirname(copy), { recursive: true });
    writeFileSync(copy, readFileSync(join(repo, pick)));
    env.LS_DEV_PICK = copy;
  }
  rust = spawn(testBinary(), ["tools::devserve::tests::serve_tools_dev", "--exact", "--ignored", "--nocapture"], { cwd: tauri, env, stdio: ["ignore", "ignore", "inherit"] });
  kids.push(rust);
  rustPick = pick;
  for (let i = 0; i < 1800 && !((await listening(RUST_PORT)) && (await listening(VITE_PORT))); i++) {
    if (rust.exitCode !== null) throw new Error(`The Rust Tools server stopped (${rust.exitCode}); its error is above.`);
    await new Promise((r) => setTimeout(r, 500));
  }
  if (!(await listening(RUST_PORT))) throw new Error("The Rust Tools server didn't start.");
}

// ---- images ----------------------------------------------------------------------------------

function save(name, shot) {
  mkdirSync(join(OUT, "thumb"), { recursive: true });
  const raw = join(tmp, `${name}.raw.png`);
  writeFileSync(raw, shot);
  const run = (cmd, a) => {
    const r = spawnSync(cmd, a, { encoding: "utf8" });
    if (r.status !== 0) throw new Error(`${cmd} ${a.join(" ")}: ${r.stderr}`);
  };
  run("cwebp", ["-quiet", "-q", "80", "-m", "6", raw, "-o", join(OUT, `${name}.webp`)]);
  run("cwebp", ["-quiet", "-q", "75", "-resize", "320", "0", raw, "-o", join(OUT, "thumb", `${name}.webp`)]);
  const png = join(OUT, `${name}.png`);
  run("sips", ["-s", "format", "png", "--resampleWidth", "1200", raw, "--out", png]);
  if (spawnSync("pngquant", ["--version"]).status === 0) run("pngquant", ["--force", "--skip-if-larger", "--ext", ".png", "256", png]);
  rmSync(raw, { force: true });
}

// ---- run -------------------------------------------------------------------------------------

if (flag("--list")) {
  for (const [n, s] of Object.entries(SHOTS)) console.log(`${n.padEnd(16)} @${s.group.padEnd(8)} ${s.about}`);
  process.exit(0);
}
const asked = args.filter((a) => !a.startsWith("--"));
// (by map: the server restarts only when the next shot needs another one)
const names = Object.keys(SHOTS).filter((n) => !asked.length || asked.includes(n) || asked.includes(`@${SHOTS[n].group}`)).sort((a, b) => (SHOTS[a].pick ?? "").localeCompare(SHOTS[b].pick ?? ""));
if (!names.length) throw new Error(`No such shot. --list shows them.`);
if (spawnSync("cwebp", ["-version"]).status !== 0) throw new Error("Needs cwebp: brew install webp");

const { chromium } = await import("@playwright/test");
await start();
const browser = await chromium.launch({ headless: !flag("--headed") });
let failed = 0;
try {
  for (const n of names) {
    const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1, colorScheme: "dark", reducedMotion: "reduce" });
    const page = await ctx.newPage();
    try {
      await rustFor(SHOTS[n].pick);
      await SHOTS[n].take(page);
      // no hover leftovers or a blinking caret in the picture
      await page.mouse.move(W - 2, H - 2);
      await page.evaluate(() => (document.activeElement instanceof HTMLElement ? document.activeElement.blur() : null));
      await settle(page, 600);
      save(n, await page.screenshot({ type: "png" }));
      console.log(`✓ ${n}`);
    } catch (e) {
      failed++;
      console.error(`✗ ${n}: ${e.message.split("\n")[0]}`);
      writeFileSync(join(tmp, `${n}.failed.png`), await page.screenshot({ type: "png" }).catch(() => Buffer.alloc(0)));
    }
    await ctx.close();
  }
} finally {
  await browser.close();
  stopAll();
  if (!flag("--keep")) rmSync(join(tmp, "app"), { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
