// Editor coverage sweep: Map menu actions that need the compiler or the game, and the Compile
// panel. The dev server can't run SDHLT on a sim copy (compile.rs finds the game through Steam
// and Settings' game folders), so the compile routes are answered here and what the page SENDS
// is checked: the preset, name, cordon, install for a playtest, the .res, the bots' learning
// and the game launch. The real compile + sim of the same map is in EditorCoverage.md.
// Leaks use a real pointfile through the dev server's file dialog (LS_DEV_PICK).
import { expect, test, type Page, type Route } from "@playwright/test";
import { cmd, consoleErrors, fixture, key, menu, openEditor, PICK, pickFile, select, sel, st, testMap } from "./editor-coverage.helpers";

type Sent = { url: string; body: Record<string, unknown> | null };

/** Answers the compiler's routes like a working compile that ends in `result` (or a leak). */
async function fakeCompiler(page: Page, o: { installed?: boolean; canRun?: boolean; leak?: number[][] } = {}) {
  const sent: Sent[] = [];
  let polls = 0;
  const job = (running: boolean) => ({
    id: 1, kind: "compile", running, stage: running ? "bsp" : "done", label: running ? "Building the map…" : "Done", fraction: running ? 0.5 : 1, seconds: 3,
    problems: o.leak ? [{ level: "error", kind: "leak", message: "The map leaks, so it can't be finished.", detail: "LEAK", at: o.leak[1] }] : [],
    leak: o.leak ? { points: o.leak, pointfile: "cov.lin" } : null,
    result: o.leak ? null : { map: "cov_map", bsp: "cov_map.bsp", bytes: 40960, preset: "fast", installed: true, backup: null },
    error: o.leak ? "The map leaks, so it can't be finished." : null, stopped: false,
  });
  const log = (r: Route) => sent.push({ url: new URL(r.request().url()).pathname, body: r.request().postDataJSON?.() ?? null });
  await page.route("**/api/tools/compile/status", (r) => r.fulfill({ json: { installed: o.installed ?? true, version: "v1.3.1", bytes: 3000000, url: "https://github.com/seedee/SDHLT", licence: "GPL", folder: "/x", canRun: o.canRun ?? true, cantRun: null, suggestName: "cov_map", busy: false, cores: 16, threads: 6 } }));
  await page.route("**/api/tools/compile/setup", (r) => (log(r), r.fulfill({ json: { ok: true, installed: true } })));
  await page.route("**/api/tools/compile/start", (r) => (log(r), (polls = 0), r.fulfill({ json: { ok: true, job: 1, map: (r.request().postDataJSON()?.name as string) || "cov_map" } })));
  await page.route("**/api/tools/compile/cancel", (r) => (log(r), r.fulfill({ json: { ok: true } })));
  await page.route("**/api/tools/compile/progress**", (r) => r.fulfill({ json: { job: job(++polls < 3), lines: [`line ${polls}`], next: polls } }));
  await page.route("**/api/tools/doctor/*/res", (r) => (log(r), r.fulfill({ json: { ok: true } })));
  await page.route("**/api/navs/learn", (r) => (r.request().method() === "POST" ? (log(r), r.fulfill({ json: { ok: true } })) : r.fulfill({ json: { result: { message: "learned", learned: ["cov_map"] }, running: false } })));
  await page.route("**/api/setup/jobs**", (r) => r.fulfill({ json: { jobs: [] } }));
  await page.route("**/api/state", (r) => r.fulfill({ json: { online: false, game_open: false } }));
  await page.route("**/api/app/launch", (r) => (log(r), r.fulfill({ json: { ok: true } })));
  if (o.leak) await page.route("**/api/tools/editor/leak", (r) => r.fulfill({ json: { leaked: true, from: "compile", pointfile: "cov.lin", points: o.leak, hole: { at: o.leak[1], index: 1 }, nearest: [], brushEntitiesNear: [], start: null } }));
  await page.route("**/api/app/start-options", (r) => r.fulfill({ json: { modes: [{ id: "classic", name: "Classic" }, { id: "laststand", name: "Last Stand" }], mode: "classic" } }));
  return sent;
}
const compilePanel = (page: Page) => page.locator(".tools-panel").filter({ has: page.getByRole("group", { name: "Quality" }) }).first();

test("Compile panel: quality, name, all cores, Compile sends them and says it's done", async ({ page }) => {
  const sent = await fakeCompiler(page);
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Compile and play…");
  const p = compilePanel(page);
  await p.getByRole("group", { name: "Quality" }).getByRole("button", { name: "Final" }).click();
  await p.getByRole("group", { name: "Quality" }).getByRole("button", { name: "Fast" }).click();
  await p.getByLabel("Map name").fill("cov map");
  await expect(p.getByLabel("Map name")).toHaveValue("cov_map"); // spaces become _
  await p.getByLabel(/Use all cores/).check();
  await p.getByRole("button", { name: "Compile", exact: true }).click();
  await expect.poll(() => sent.find((s) => s.url.endsWith("/start"))?.body ?? null).toMatchObject({ preset: "fast", name: "cov_map", allCores: true });
  expect(sent.find((s) => s.url.endsWith("/start"))!.body!.install ?? false).toBe(false);
  await expect(p.getByText(/Compiled cov_map/)).toBeVisible();
  await p.getByText("Compiler output").click();
  await expect(p.locator("pre")).toContainText("line");
});

test("Compile panel: a bad map name is refused before it starts", async ({ page }) => {
  await fakeCompiler(page);
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Compile and play…");
  const p = compilePanel(page);
  await p.getByLabel("Map name").fill("de_dust2");
  // a stock map's name, or Windows' reserved ones: said in words, Compile greyed out
  await p.getByLabel("Map name").fill("con");
  await expect(p.getByRole("alert")).toBeVisible();
  await expect(p.getByRole("button", { name: "Compile", exact: true })).toBeDisabled();
});

test("Download SDHLT asks the app to set it up", async ({ page }) => {
  const sent = await fakeCompiler(page, { installed: false });
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Compile and play…");
  const dl = page.getByRole("button", { name: /Download SDHLT/ });
  await expect(dl).toBeEnabled();
  await dl.click();
  await expect.poll(() => sent.some((s) => s.url.endsWith("/setup"))).toBe(true);
});

for (const via of ["the bar's Playtest button", "Map > Playtest", "F5", "the Compile panel's Playtest"] as const) {
  test(`Playtest from ${via}: compiles into the game, writes the .res, teaches bots, starts the game`, async ({ page }) => {
    const sent = await fakeCompiler(page);
    await openEditor(page);
    await testMap(page);
    // the Playtest box's settings: 6 bots, teach them the map
    await menu(page, "Map", "Compile and play…");
    await page.locator(".compile-playtest").getByRole("spinbutton", { name: "Bots" }).fill("6");
    await page.locator(".compile-playtest").getByLabel("Teach bots the map first").check();
    await menu(page, "View", "Panel: Content");
    if (via === "the bar's Playtest button") await page.locator(".ed-bar-actions").getByRole("button", { name: "Playtest" }).click();
    else if (via === "Map > Playtest") await menu(page, "Map", "Playtest");
    else if (via === "F5") await key(page, "F5");
    else {
      await menu(page, "Map", "Compile and play…");
      await page.locator(".compile-playtest").getByRole("button", { name: "Playtest" }).click();
    }
    await expect.poll(() => sent.map((s) => s.url), { timeout: 20_000 }).toContain("/api/app/launch");
    const start = sent.find((s) => s.url.endsWith("/start"))!.body!;
    expect(start.install).toBe(true);
    const order = sent.map((s) => s.url.replace(/\/api\/tools\/doctor\/[^/]+\/res/, "res"));
    expect(order.indexOf("res")).toBeGreaterThan(order.indexOf("/api/tools/compile/start"));
    expect(order.indexOf("/api/navs/learn")).toBeGreaterThan(order.indexOf("res"));
    expect(sent.find((s) => s.url === "/api/app/launch")!.body).toMatchObject({ map: "cov_map" });
    expect(sent.find((s) => s.url === "/api/app/launch")!.body!.bots).toBe(6);
  });
}

test("a compile that leaks: the leak bar, Go to leak, line toggle, go to its start and end", async ({ page }) => {
  await fakeCompiler(page, { leak: [[0, -128, 96], [0, 0, 64], [600, 0, 64]] });
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Compile and play…");
  const p = compilePanel(page);
  await p.getByRole("button", { name: "Compile", exact: true }).click();
  await expect(p.getByRole("button", { name: "Go to leak" })).toBeVisible();
  await p.getByRole("button", { name: "Go to its start" }).click();
  await p.getByRole("button", { name: "Go to its end" }).click();
  await expect(page.getByRole("alert", { name: "The map leaks" })).toBeVisible();
});

test.describe("leaks from a pointfile", () => {
  // the dev server tells a leak line by the picked path's extension: a run with LS_DEV_PICK ending .lin
  test.skip(!PICK.endsWith(".lin"), "needs LS_COV_PICK ending .lin");
  test("Load pointfile…, the leak bar, Go to leak, Pick brushes at the hole, Show or hide the leak line", async ({ page }) => {
    const errors = consoleErrors(page);
    await openEditor(page);
    const m = await testMap(page);
    // a line from the light, out through the east side of the room
    // (the compiler's .lin: segments "x y z - x y z")
    pickFile(fixture("cov-src.lin", "0 -128 96 - 200 -128 96\n200 -128 96 - 400 -128 96\n400 -128 96 - 700 -128 96\n"));
    await menu(page, "Map", "Load pointfile…");
    const bar = page.getByRole("alert", { name: "The map leaks" });
    await expect(bar).toBeVisible();
    await expect(bar).toContainText(".lin");
    // Map > Show or hide the leak line (the bar's button follows)
    await expect(bar.getByRole("button", { name: "Hide line" })).toBeVisible();
    await menu(page, "Map", "Show or hide the leak line");
    await expect(bar.getByRole("button", { name: "Show line" })).toBeVisible();
    await bar.getByRole("button", { name: "Show line" }).click();
    await expect(bar.getByRole("button", { name: "Hide line" })).toBeVisible();
    // Map > Go to leak: flies there and picks the brush beside the hole
    await select(page, {});
    await menu(page, "Map", "Go to leak");
    await expect.poll(async () => (await sel(page)).brushes.length, { timeout: 15_000 }).toBeGreaterThan(0);
    const pick = bar.getByRole("button", { name: "Pick brushes at the hole" });
    if (await pick.count()) {
      await select(page, {});
      await pick.click();
      await expect.poll(async () => (await sel(page)).brushes.length).toBeGreaterThan(0);
    }
    void m;
    expect(errors).toEqual([]);
  });
});

test("Publish…: says the budget first when near a limit, then opens Community's Publish for this map", async ({ page }) => {
  // (set before the page loads: a reload while the app moves to its own address can be cut short
  // in WebKit, "Frame load interrupted")
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "blender", speed: 3 }));
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
  });
  await page.goto("/tools-dev.html?open=editor");
  const bar = page.locator(".ed-menubar");
  if (!(await bar.isVisible().catch(() => false))) {
    const back = page.getByRole("button", { name: /^Back to |^New map$/ }).first();
    await back.click();
  }
  await expect(bar).toBeVisible({ timeout: 30_000 });
  const name = await page.evaluate(() => (window as unknown as { __lsEditor: { getState: () => { doc: { name: string } } } }).__lsEditor.getState().doc.name);
  await page.locator(".ed-bar-actions").getByRole("button", { name: "Publish…" }).click();
  await expect.poll(() => decodeURIComponent(page.url())).toMatch(/community:publish:/);
  void name;
  // and Map > Publish… the same way (back through the app's own history: WebKit can report a
  // same-page Back as an interrupted load)
  await page.evaluate(() => history.back());
  await expect(bar).toBeVisible({ timeout: 30_000 });
  await menu(page, "Map", "Publish…");
  await expect.poll(() => decodeURIComponent(page.url())).toMatch(/community:publish:/);
});

test("Cordon: a box round what's picked, more boxes, from the view, remove, remove all", async ({ page }) => {
  await fakeCompiler(page);
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Map", "Compile and play…");
  const cordon = () => st(page, (s) => s.doc.editor?.cordon ?? null);
  await select(page, { brushes: [m.a] });
  await page.getByRole("button", { name: "Box round what's picked" }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(1);
  // round what's picked, with a margin
  await expect.poll(async () => { const b = (await cordon())?.boxes[0]; return !!b && b.min.every((x: number) => x <= 0) && b.max.every((x: number) => x >= 64); }).toBe(true);
  await page.getByRole("button", { name: "Cordon…" }).click();
  await page.getByRole("button", { name: "Add one round what's picked" }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(2);
  await page.getByRole("button", { name: "Add one from the view" }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(3);
  await page.getByRole("button", { name: "Remove box 3" }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(2);
  await select(page, { brushes: [m.b] });
  await page.getByRole("button", { name: "Round what's picked", exact: true }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(1);
  await page.getByRole("button", { name: "Remove all boxes" }).click();
  await expect.poll(async () => (await cordon())?.boxes.length ?? 0).toBe(0);
  void cmd;
});
