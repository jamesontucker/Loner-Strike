// Game Mode Studio, graph version 3 in the page: an address opens a Function's tab, Function
// Entry/Return show the Inputs/Outputs editor, the keyboard shortcuts (never while typing),
// a snippet with events goes onto the Event Graph, the library's Export all / Import, and
// leaving with unsaved edits through each way out (asked once; Leave goes and stays gone).
// Run in Chromium and WebKit: `npx playwright test studio-v3 [--browser webkit]`.
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

const H = { "X-Panel": "1" };
const ID = "pw_v3";

async function seed(page: Page) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "V3 check";
  tpl.functions = [{ id: "fa", name: "Reward", inputs: [{ id: "amt", name: "Amount", type: "number" }], outputs: [] }];
  tpl.nodes.push({ id: "fa_e", type: "fn.entry", pos: { x: 0, y: 0 }, params: {}, graph: "fa" }, { id: "fa_r", type: "fn.return", pos: { x: 420, y: 0 }, params: {}, graph: "fa" });
  tpl.edges.push({ id: "fa_w", from: { node: "fa_e", pin: "then" }, to: { node: "fa_r", pin: "exec" } });
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}
const saved = async (page: Page) => (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
const tabs = (page: Page) => page.getByRole("tablist", { name: "Graphs" });
const selectedTab = (page: Page) => tabs(page).getByRole("tab", { selected: true });
const node = (page: Page, id: string) => page.locator(`.react-flow__node[data-id="${id}"]`);
const canvasKey = async (page: Page, key: string) => {
  // keys go to the canvas: focus its pane first (not a field)
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press(key);
};

// Nothing here installs, compiles or plays in a game: those routes are answered in the page
// (a test that wants a Playtest answer routes it itself, later routes win). The dev server also
// refuses to write into a real Half-Life install while under test (studio::guard_real_game).
const WRITES = ["compile", "check", "playtest", "switch", "source", "uninstall/**", "debug/command"];
test.beforeEach(async ({ page }) => {
  for (const r of WRITES) await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test: not installed)", installedAs: ID } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
  await seed(page);
});
// and whatever a test made is taken away again, installed or not
test.afterAll(async ({ playwright }, info) => {
  const api = await playwright.request.newContext({ baseURL: String(info.project.use.baseURL) });
  for (const id of [ID, "pw_tall", "pw_shots"]) {
    for (const short of [id, `${id}_2`, `${id}_3`, `${id}_4`]) await api.post(`/api/tools/studio/uninstall/${short}`, { headers: H }).catch(() => {});
    await api.post(`/api/tools/studio/graph/${id}/delete`, { headers: H }).catch(() => {});
  }
  await api.dispose();
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("address opens the tab; Entry/Return details; shortcuts", async ({ page }) => {
  await page.route("**/api/tools/studio/playtest", async (r) => {
    await new Promise((res) => setTimeout(res, 1500));
    await r.fulfill({ json: { ok: true, problems: [], output: "Done." } });
  });
  await page.route("**/api/state", (r) => r.fulfill({ json: { game_open: false } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
  await page.goto(`/tools-dev.html?open=studio:${ID}/fa`);
  // the address's tab is open straight away
  await expect(selectedTab(page)).toContainText("Reward", { timeout: 20_000 });
  await expect(node(page, "fa_e")).toBeVisible();

  // Function Entry: its Inputs editor in Details
  await node(page, "fa_e").click();
  const details = page.getByRole("complementary", { name: "Details" });
  await expect(details.getByRole("heading", { name: "Inputs" })).toBeVisible();
  await details.getByRole("button", { name: "Add input" }).click();
  await expect(details.locator('input[value="Input 2"]')).toHaveCount(1);
  // Return: the Outputs editor
  await node(page, "fa_r").click();
  await expect(details.getByRole("heading", { name: "Outputs" })).toBeVisible();
  await details.getByRole("button", { name: "Add output" }).click();

  // ⌘S saves
  await canvasKey(page, "ControlOrMeta+s");
  await expect.poll(async () => (await saved(page)).functions[0].inputs.length).toBe(2);
  expect((await saved(page)).functions[0].outputs.length).toBe(1);

  // ⌘D duplicates the selection (the Return)
  const before = await page.locator(".react-flow__node").count();
  await node(page, "fa_r").click();
  await page.keyboard.press("ControlOrMeta+d");
  await expect(page.locator(".react-flow__node")).toHaveCount(before + 1);
  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.locator(".react-flow__node")).toHaveCount(before);

  // Alt-click a wire breaks it
  const wires = page.locator(".react-flow__edge");
  await expect(wires).not.toHaveCount(0);
  const wireCount = await wires.count();
  // (the middle of the wire, on screen)
  const mid = await page.getByRole("button", { name: "Edge from fa_e to fa_r" }).locator("path.react-flow__edge-path").evaluate((el) => {
    const p = el as SVGPathElement;
    const pt = p.getPointAtLength(p.getTotalLength() / 2).matrixTransform(p.getScreenCTM()!);
    return { x: pt.x, y: pt.y };
  });
  await page.keyboard.down("Alt");
  await page.mouse.click(mid.x, mid.y);
  await page.keyboard.up("Alt");
  await expect(wires).toHaveCount(wireCount - 1);
  await canvasKey(page, "ControlOrMeta+z");
  await expect(wires).toHaveCount(wireCount);

  // ? opens the cheat sheet (and not while typing)
  await canvasKey(page, "?");
  const sheet = page.getByRole("dialog", { name: "Keys" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText("Alt+click a wire");
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);
  // ⌘F: the find box; typing "?" and "f" there stays typing
  await canvasKey(page, "ControlOrMeta+f");
  const findBox = page.locator(".st-find input");
  await expect(findBox).toBeFocused();
  await page.keyboard.type("f?");
  await expect(findBox).toHaveValue("f?");
  await expect(sheet).toHaveCount(0);
  await findBox.fill("");

  // Ctrl+Tab: Event Graph and back; ⌘W closes the Function's tab
  await canvasKey(page, "Control+Tab");
  await expect(selectedTab(page)).toContainText("Event Graph");
  await canvasKey(page, "Control+Tab");
  await expect(selectedTab(page)).toContainText("Reward");
  await canvasKey(page, "ControlOrMeta+w");
  await expect(selectedTab(page)).toContainText("Event Graph");
  await expect(tabs(page).getByRole("tab")).toHaveCount(1);

  // F and Home frame (the view moves, nothing breaks)
  await canvasKey(page, "Home");
  await canvasKey(page, "f");

  // F5: Playtest
  await canvasKey(page, "F5");
  await expect(page.getByRole("status").filter({ hasText: "Compiling" })).toBeVisible();
});

test("a snippet with events goes onto the Event Graph", async ({ page }) => {
  await page.goto(`/tools-dev.html?open=studio:${ID}/fa`);
  await expect(selectedTab(page)).toContainText("Reward", { timeout: 20_000 });
  const evBefore = (await saved(page)).nodes.filter((n: { graph?: string }) => !n.graph).length;
  await page.getByRole("button", { name: /^Kill reward/ }).click();
  await expect(page.getByText("went onto the Event Graph")).toBeVisible();
  await expect(selectedTab(page)).toContainText("Event Graph");
  await canvasKey(page, "ControlOrMeta+s");
  await expect.poll(async () => (await saved(page)).nodes.filter((n: { graph?: string }) => !n.graph).length).toBe(evBefore + 6);
  expect((await saved(page)).nodes.filter((n: { graph?: string }) => n.graph === "fa").length).toBe(2);
});

test("library: Export all, Import a whole library", async ({ page }, info) => {
  const items = async () => (await (await page.request.get("/api/tools/studio/library")).json()).items as { id: string }[];
  const had = new Set((await items()).map((x) => x.id));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
  // keep the Function twice under two names (right-click → Save to my library)
  // (a second Function for the second item: saving one twice is a new version, not an item)
  await page.getByRole("button", { name: "Add a Function (its own graph)" }).click();
  for (const [name, part] of [["PW one", "Reward"], ["PW two", "New Function"]]) {
    await page.locator(".st-mb-item").filter({ hasText: part }).first().click({ button: "right" });
    await page.getByRole("menuitem", { name: /Save to my library/ }).click();
    const form = page.getByRole("dialog", { name: "Save to my library" });
    await form.getByLabel("Name").fill(name);
    await form.getByRole("button", { name: "Save", exact: true }).click();
    await expect(form).toHaveCount(0);
  }
  await expect.poll(async () => (await items()).filter((x) => !had.has(x.id)).length).toBe(2);
  const ours = (await items()).filter((x) => !had.has(x.id)).map((x) => x.id);

  const dl = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export all" }).click();
  const path = info.outputPath("library.lib.json");
  await (await dl).saveAs(path);
  const data = JSON.parse(await (await import("node:fs/promises")).readFile(path, "utf8"));
  expect(data.items.length).toBe(had.size + 2);

  // remove ours, import the whole file: ours come back (the others as renamed copies)
  for (const id of ours) await page.request.post(`/api/tools/studio/library/${id}/delete`, { headers: H });
  await page.locator('.st-mb input[type="file"]').setInputFiles(path);
  await expect(page.getByText(`Imported ${data.items.length} item${data.items.length === 1 ? "" : "s"} into your library`)).toBeVisible();
  const after = (await items()).map((x) => x.id);
  for (const id of ours) expect(after).toContain(id);
  expect(after.length).toBe(had.size + data.items.length);
  for (const id of after) if (!had.has(id)) await page.request.post(`/api/tools/studio/library/${id}/delete`, { headers: H });
});

test("map markers make the mode's kit", async ({ page }) => {
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
  await page.getByRole("button", { name: "Add a map marker" }).click();
  const details = page.getByRole("complementary", { name: "Details" });
  await expect(details).toContainText("Map marker");
  await details.getByRole("textbox").first().fill("Creature spawn");
  await details.locator("select").first().selectOption("path");
  await details.getByRole("button", { name: "Add key" }).click();
  await page.locator(".st-mb-item").filter({ hasText: "Map advice" }).click();
  await details.locator("select").first().selectOption("medium");
  await canvasKey(page, "ControlOrMeta+s");
  await expect.poll(async () => ((await saved(page)).markers ?? []).length).toBe(1);
  const g = await saved(page);
  expect(g.mode.uid).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4/);
  const kit = (await (await page.request.get(`/api/tools/studio/kit/${ID}`)).json()).kit;
  expect(kit.mode.uid).toBe(g.mode.uid);
  expect(kit.markers[0]).toMatchObject({ id: "marker_1", label: "Creature spawn", type: "path", draw: { shape: "line" } });
  expect(kit.markers[0].keys[0]).toMatchObject({ name: "key_1", type: "number" });
  expect(kit.map.size).toBe("medium");
  // drag it onto the canvas: Find Marker
  const before = await page.locator(".react-flow__node").count();
  await page.locator(".st-mb-item").filter({ hasText: "Creature spawn" }).dragTo(page.locator(".react-flow__pane"), { targetPosition: { x: 300, y: 200 } });
  await expect(page.locator(".react-flow__node")).toHaveCount(before + 1);
  await expect(page.locator(".react-flow__node").filter({ hasText: "Find Marker" })).toHaveCount(1);
});

test("our right-click menus: sidebar items, nodes; delete; dismiss", async ({ page }) => {
  // a variable, and a call to Reward on the Event Graph
  const g = await saved(page);
  g.variables.push({ id: "vsq", name: "Squads left", type: "number", scope: "mode", default: 0 });
  g.nodes.push({ id: "call1", type: "fn.call", pos: { x: 600, y: 400 }, params: { func: "fa" } });
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json()).ok).toBeTruthy();
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
  // what the browser would have done with each right-click: shown its own menu or not
  await page.evaluate(() => {
    const w = window as unknown as { __native: { prevented: boolean }[] };
    w.__native = [];
    // (each right-click: was its default, the browser's menu, stopped by anything)
    document.addEventListener(
      "contextmenu",
      (e) => {
        const rec = { prevented: false };
        w.__native.push(rec);
        const stop = e.preventDefault.bind(e);
        e.preventDefault = () => {
          rec.prevented = true;
          stop();
        };
      },
      true,
    );
  });
  const native = () => page.evaluate(() => !(window as unknown as { __native: { prevented: boolean }[] }).__native.at(-1)?.prevented);
  const menu = page.getByRole("menu");
  const sq = page.locator(".st-mb").getByRole("button", { name: /^Squads left/ });

  // a variable: our menu (not the browser's), keyboard on the first item, Escape closes
  await sq.click({ button: "right" });
  await expect(menu).toBeVisible();
  expect(await native()).toBe(false);
  await expect(menu.getByRole("menuitem")).toHaveText([/Rename/, /Duplicate/, /Find uses/, /Save to my library/, /Delete/]);
  await expect(menu.getByRole("menuitem", { name: /Save to my library/ })).toHaveAttribute("aria-disabled", "true");
  await expect(menu.getByRole("menuitem").first()).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(menu.getByRole("menuitem", { name: "Duplicate" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  // a click elsewhere closes it too
  await sq.click({ button: "right" });
  await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
  await expect(menu).toHaveCount(0);
  // Duplicate, then select the copy and press Delete (asks first)
  await sq.click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Duplicate" }).click();
  const copy = page.locator(".st-mb").getByRole("button", { name: /^Squads left 2/ });
  await expect(copy).toHaveCount(1);
  await copy.click();
  await page.keyboard.press("Delete");
  const askDel = page.getByRole("alertdialog");
  await expect(askDel).toContainText("Delete the variable “Squads left 2”?");
  await askDel.getByRole("button", { name: "Delete" }).click();
  await expect(copy).toHaveCount(0);
  await expect(sq).toHaveCount(1);

  // a Function in use: Delete says where it's used; Cancel keeps it; Find uses selects the call
  const fn = page.locator(".st-mb").getByRole("button", { name: "Reward", exact: true });
  await fn.click({ button: "right" });
  await menu.getByRole("menuitem", { name: /Delete/ }).click();
  await expect(askDel).toContainText("used by 1 node on the Event Graph");
  await askDel.getByRole("button", { name: "Cancel" }).click();
  await expect(fn).toHaveCount(1);
  await fn.click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Find uses" }).click();
  await expect(page.locator('.react-flow__node[data-id="call1"]')).toHaveClass(/selected/);
  // Rename
  await fn.click({ button: "right" });
  await menu.getByRole("menuitem", { name: /Rename/ }).click();
  await page.getByRole("alertdialog").getByRole("textbox").fill("Payout");
  await page.getByRole("alertdialog").getByRole("button", { name: "Rename" }).click();
  await expect(page.locator(".st-mb").getByRole("button", { name: "Payout", exact: true })).toHaveCount(1);
  // Save to my library opens its sheet; Escape closes it
  await page.locator(".st-mb").getByRole("button", { name: "Payout", exact: true }).click({ button: "right" });
  await menu.getByRole("menuitem", { name: /Save to my library/ }).click();
  const sheet = page.getByRole("dialog", { name: "Save to my library" });
  await expect(sheet).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);

  // a node on the canvas: its own menu; Delete takes it away
  const call = page.locator('.react-flow__node[data-id="call1"]');
  await call.click({ button: "right" });
  expect(await native()).toBe(false);
  await expect(menu.getByRole("menuitem", { name: "Duplicate" })).toBeVisible();
  await menu.getByRole("menuitem", { name: "Delete" }).click();
  await expect(call).toHaveCount(0);
  // anywhere else in the Studio: no browser menu; a text box keeps the system's (copy, paste)
  await page.getByRole("complementary", { name: "Details" }).locator("p").first().click({ button: "right" });
  expect(await native()).toBe(false);
  await expect(menu).toHaveCount(0);
  await page.locator(".st-find input").click({ button: "right" });
  expect(await native()).toBe(true);
  await page.keyboard.press("Escape");

  // the Find list: closes on a click elsewhere and on Escape
  const findBox = page.locator(".st-find input");
  await findBox.fill("round");
  const list = page.getByRole("listbox");
  await expect(list).toBeVisible();
  await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
  await expect(list).toHaveCount(0);
  await findBox.click();
  await expect(list).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(list).toHaveCount(0);
});

test("the Studio never scrolls as a whole (every-node graph: long panels scroll inside)", async ({ page }) => {
  const g = JSON.parse(readFileSync("../src-tauri/src/tools/studio/testdata/every_node.graph.json", "utf8"));
  g.mode.id = "pw_tall";
  expect((await (await page.request.post("/api/tools/studio/graph/pw_tall", { data: { graph: g }, headers: H })).json()).ok).toBeTruthy();
  try {
    for (const size of [{ width: 1440, height: 900 }, { width: 1024, height: 680 }]) {
      await page.setViewportSize(size);
      await page.goto("/tools-dev.html?open=studio:pw_tall");
      await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
      const still = async (after: string) => {
        const m = await page.evaluate(() => {
          const c = document.querySelector<HTMLElement>(".content")!;
          const st = document.querySelector<HTMLElement>(".st-page")!.getBoundingClientRect();
          const mb = document.querySelector<HTMLElement>(".st-mb")!;
          return { top: c.scrollTop, over: c.scrollHeight - c.clientHeight, win: window.scrollY, bottom: st.bottom, inner: window.innerHeight, mbScrolls: mb.scrollHeight > mb.clientHeight };
        });
        expect(m.top, `${after}: the page scrolled`).toBe(0);
        expect(m.win, after).toBe(0);
        expect(m.over, `${after}: the page is taller than the window`).toBeLessThanOrEqual(1);
        expect(m.bottom, `${after}: the Studio runs past the window`).toBeLessThanOrEqual(m.inner + 1);
        expect(m.mbScrolls, "the long My mode list scrolls inside itself").toBe(true);
      };
      await still("open");
      for (const t of [/^Reward$/, /^Twice$/, /^Loadout/, /^Class/, /^Kills/, /^Spot/]) {
        const item = page.locator(".st-mb").getByRole("button", { name: t }).first();
        if (!(await item.count())) continue;
        await item.scrollIntoViewIfNeeded();
        await item.click();
        await still(`picking ${t}`);
      }
      await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
      await still("clicking the canvas");
      await page.keyboard.press("Home");
      await page.keyboard.press("f");
      await still("Home and F");
      // a node far away, picked from Find (the view goes to it)
      await page.locator(".st-find input").fill("marker");
      await page.keyboard.press("Enter");
      await still("going to a node");
      const shown = page.locator(".react-flow__node.selected").first();
      await expect(shown).toBeVisible();
      const box = (await shown.boundingBox({ timeout: 5000 }))!;
      await page.mouse.click(box.x + box.width / 2, box.y + 8);
      await still("clicking a node");
      await page.locator(".st-mb").getByRole("button", { name: /^Kills/ }).first().click({ button: "right" });
      await page.keyboard.press("Escape");
      await still("a menu");
      await page.locator(".st-find input").fill("print");
      await page.locator(".react-flow__pane").click({ position: { x: 20, y: 20 } });
      await still("the Find list");
      // the last items of the long list: scrolled to inside the panel
      await page.locator(".st-mb").getByRole("button", { name: "Export all" }).scrollIntoViewIfNeeded();
      await still("the end of My mode");
    }
  } finally {
    await page.request.post("/api/tools/studio/graph/pw_tall/delete", { headers: H });
  }
});

test("Compile: the full check's status, its problems tied to nodes, Compile on save, keys", async ({ page }) => {
  let calls = 0;
  let answer: object = { ok: true, problems: [], compiler: [], output: "Done." };
  await page.route("**/api/tools/studio/check", async (r) => {
    calls++;
    await new Promise((res) => setTimeout(res, 300));
    await r.fulfill({ json: answer });
  });
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const icon = page.getByTestId("studio-check-icon");
  const status = page.getByTestId("studio-check-status");
  await expect(icon).toHaveAttribute("data-state", "none");
  await expect(status).toContainText("Quick check: no problems (updated as you edit). Full check: needs compiling.");
  // the button: a spinner while it runs, then green
  await page.getByTestId("studio-compile").click();
  await expect(page.getByTestId("studio-compiling")).toContainText("Compiling…");
  await expect(icon).toHaveAttribute("data-state", "ok");
  await expect(icon).toHaveAttribute("title", /compiled, no problems/);
  await expect(status).toContainText("Full check: compiled, no problems");
  // an edit makes it stale (gray); a move doesn't count as an edit to the compiler
  await canvasKey(page, "Escape");
  await node(page, "give_m4").click();
  await page.keyboard.press("Delete");
  await expect(icon).toHaveAttribute("data-state", "none");
  await expect(status).toContainText("needs compiling (edited since the last compile)");
  // F7: a warning tied to a node (click focuses it) and a line with no node
  answer = { ok: true, problems: [], compiler: [{ level: "warning", node: "is_t", message: "The compiler said (line 40): loose indentation" }, { level: "warning", message: "The compiler said (line 9 of the plugin, not tied to a node; Show code in the menu shows that line): x" }], output: "2 Warnings." };
  await canvasKey(page, "F7");
  await expect(icon).toHaveAttribute("data-state", "warn");
  const list = page.locator(".st-results-list");
  await expect(list.locator("li.warning")).toHaveCount(2);
  await expect(list.locator("li.warning").first()).toContainText("Full check");
  await expect(list.locator("li.warning").nth(1)).toContainText("not tied to a node");
  await list.getByRole("button", { name: /Is Team/ }).first().click();
  await expect(node(page, "is_t")).toHaveClass(/selected/);
  // errors: red, and clicking the icon opens Problems
  answer = { ok: false, problems: [], compiler: [{ level: "error", node: "is_t", message: "The compiler said (line 41): undefined symbol" }], output: "1 Error." };
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+b");
  await expect(icon).toHaveAttribute("data-state", "error");
  await page.locator(".st-results-tabs").getByRole("tab", { name: "Compiler output" }).click();
  await icon.click();
  await expect(page.locator(".st-results-tabs").getByRole("tab", { name: /Problems/ })).toHaveAttribute("aria-selected", "true");
  // Compile on save (on by default): a save compiles in the background
  answer = { ok: true, problems: [], compiler: [], output: "Done." };
  const before = calls;
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+s");
  await expect(icon).toHaveAttribute("data-state", "ok");
  expect(calls).toBe(before + 1);
  // off: a save doesn't
  await page.getByLabel("Compile on save").uncheck();
  // (an edit: Undo brings the deleted node back)
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+s");
  await page.waitForTimeout(800);
  expect(calls).toBe(before + 1);
  await expect(icon).toHaveAttribute("data-state", "none");
  await page.getByLabel("Compile on save").check();
});

test.describe("leaving with unsaved edits", () => {
  // in the page, as a person gets there: Overview → Game Mode Studio → the mode → its Function
  const dirty = async (page: Page) => {
    await page.goto(`/tools-dev.html?open=overview`);
    await page.getByRole("navigation", { name: "Tools" }).getByRole("button", { name: "Game Mode Studio" }).click();
    await page.getByRole("button", { name: /V3 check/ }).first().click();
    await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
    await page.locator(".st-mb-item").filter({ hasText: "Reward" }).first().dblclick();
    await expect(selectedTab(page)).toContainText("Reward");
    await node(page, "fa_e").click();
    await page.getByRole("complementary", { name: "Details" }).getByRole("button", { name: "Add input" }).click();
  };
  const leaveAsked = (page: Page) => page.getByRole("alertdialog").filter({ hasText: "Leave without saving" });
  const noQuestion = async (page: Page) => {
    // a moment later: no (second) question, and nothing stepped back
    await page.waitForTimeout(700);
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
  };
  const stillEdited = async (page: Page) => {
    await tabs(page).getByRole("tab", { name: /Reward/ }).click();
    await node(page, "fa_e").click();
    await expect(page.getByRole("complementary", { name: "Details" }).locator('input[value="Input 2"]')).toHaveCount(1);
  };
  const atStudioList = (page: Page) => expect(page).toHaveURL(/open=studio(&|$)/);

  test("between a mode's tabs: nothing to ask, the edits stay", async ({ page }) => {
    await dirty(page);
    await page.locator(".tools-crumb").filter({ hasText: "V3 check" }).click();
    await noQuestion(page);
    await expect(selectedTab(page)).toContainText("Event Graph");
    await page.evaluate(() => history.back());
    await noQuestion(page);
    await expect(selectedTab(page)).toContainText("Reward");
    await stillEdited(page);
  });

  test("a crumb: Cancel stays, Leave goes once", async ({ page }) => {
    await dirty(page);
    const crumb = page.locator(".tools-crumb").filter({ hasText: "Game Mode Studio" });
    await crumb.click();
    await expect(leaveAsked(page)).toBeVisible();
    await leaveAsked(page).getByRole("button", { name: "Cancel" }).click();
    await noQuestion(page);
    await stillEdited(page);
    await crumb.click();
    await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
    await noQuestion(page);
    await atStudioList(page);
  });

  test("the Tools crumb", async ({ page }) => {
    await dirty(page);
    await page.locator(".tools-crumb").filter({ hasText: /^Tools$/ }).click();
    await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
    await noQuestion(page);
    await expect(page).toHaveURL(/open=overview/);
  });

  for (const [how, back] of [
    ["the Back button", (p: Page) => p.getByRole("button", { name: "Back", exact: true }).click()],
    ["Alt+←", async (p: Page) => {
      await p.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
      await p.keyboard.press("Alt+ArrowLeft");
    }],
    ["the browser's back", (p: Page) => p.evaluate(() => history.back())],
  ] as [string, (p: Page) => Promise<unknown>][]) {
    test(how, async ({ page }) => {
      await dirty(page);
      // the Function's tab → the Event Graph: the same mode, no question
      await back(page);
      await noQuestion(page);
      await expect(selectedTab(page)).toContainText("Event Graph");
      // → the Studio's list: asked; Cancel stays (edits too), Leave goes and stays gone
      await back(page);
      await expect(leaveAsked(page)).toBeVisible();
      await leaveAsked(page).getByRole("button", { name: "Cancel" }).click();
      await noQuestion(page);
      await expect(selectedTab(page)).toContainText("Event Graph");
      await expect(page.locator(".st-name")).toHaveText("V3 check •");
      await back(page);
      await expect(leaveAsked(page)).toBeVisible();
      await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
      await noQuestion(page);
      await atStudioList(page);
    });
  }
});
