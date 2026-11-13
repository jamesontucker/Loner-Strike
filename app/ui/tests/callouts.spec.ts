// Callouts, end to end like a user: pick areas and name them, take names from the map's bomb
// sites and spawns, rename a place, save, and see the names come back after a reload. Runs on
// tools-dev.html against the tools dev server (docs/TOOLS-DEV.md), in Chromium and WebKit:
//   npx playwright test tests/callouts.spec.ts
//
// Saving writes the map's .nav in LS_TEST_GAME, so by default the save is caught in the page
// (the request is checked, nothing reaches the disk). With LS_CALLOUTS_SAVE=1 (LS_TEST_GAME a
// COPY of the game) it really saves, reads the nav back through the API and puts the old names
// back after; LS_CALLOUTS_KEEP=1 leaves the new names on disk (for a game test).
// LS_CALLOUTS_MAP picks the map (cs_office: any map with a nav, bomb sites or hostages, spawns).
import { expect, test, type Page } from "@playwright/test";

const MAP = process.env.LS_CALLOUTS_MAP || "cs_office";
const SAVE = process.env.LS_CALLOUTS_SAVE === "1";
const KEEP = process.env.LS_CALLOUTS_KEEP === "1";
const API = `/api/tools/callouts/${MAP}`;

type Nav = { places: string[]; areas: [number, number, number, number, number, number, number][]; stamp: string; voice: string[]; inUse: boolean };
const names = (n: Nav) => Object.fromEntries(n.areas.map((a) => [a[0], a[6] ? n.places[a[6] - 1] : ""]));

/** The places list: name -> how many areas */
async function placeCounts(page: Page): Promise<Record<string, number>> {
  const out: Record<string, number> = {};
  for (const li of await page.locator(".co-places li").all()) {
    const n = (await li.locator(".co-place-name").innerText()).trim();
    out[n] = Number((await li.locator(".co-place .small-note").innerText()).trim());
  }
  return out;
}

test("name places, save, and see them after a reload", async ({ page, request, browserName }) => {
  test.setTimeout(120_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && !/404|Not Found/.test(m.text()) && errors.push(m.text()));

  const before: Nav = await (await request.get(API)).json();
  test.skip(!before.areas?.length, `${MAP} has no nav here`);
  test.skip(SAVE && before.inUse, `the game has ${MAP} loaded`);
  const original = names(before);

  await page.goto(`/tools-dev.html?open=callouts:${MAP}`);
  const canvas = page.locator(".co-stage canvas");
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  await expect(page.locator(".tools-status")).toContainText(`${before.areas.length} areas`);

  // 1. drag a box in the middle of the map (Pick), and name what it caught
  const box = (await canvas.boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.44, box.y + box.height * 0.44);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.56, box.y + box.height * 0.56, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator(".tools-status")).toContainText("picked");
  await page.getByLabel("Place name").fill("Catwalk");
  await expect(page.getByText("In chat only")).toBeVisible();
  await page.getByRole("button", { name: /^Name (it|them)$/ }).click();
  const afterBox = await placeCounts(page);
  expect(afterBox.Catwalk).toBeGreaterThan(0);

  // 2. names from the map's own zones and spawns: unnamed areas only, never what was just named
  await page.getByRole("button", { name: "From bomb sites and spawns" }).click();
  await expect(page.locator(".toast").filter({ hasText: /named from the map's bomb sites|Nothing to add/ })).toBeVisible();
  const marked = await placeCounts(page);
  expect(marked.Catwalk, "a suggestion took names the user had just given").toBe(afterBox.Catwalk);
  expect(Object.keys(marked)).toContain("CTSpawn");

  // 3. rename a place to one of our own (no voice line): every area that had it
  await page.getByRole("button", { name: "Rename CTSpawn" }).click();
  await page.getByRole("alertdialog").getByRole("textbox").fill("ZorkBase");
  await page.getByRole("alertdialog").getByRole("button", { name: "Rename" }).click();
  const renamed = await placeCounts(page);
  expect(renamed.ZorkBase).toBe(marked.CTSpawn);
  expect(renamed.CTSpawn).toBeUndefined();
  const changed = Object.values(renamed).reduce((s, n) => s + n, 0) - Object.values(original).filter(Boolean).length;
  await expect(page.locator(".tl-actionbar")).toContainText("Unsaved changes");
  await expect(page.locator(".tl-actionbar")).toContainText(/\d+ areas? renamed/);

  // 4. save
  const save = page.getByRole("button", { name: "Save names" });
  await expect(save).toBeEnabled();
  if (!SAVE) {
    let body: { stamp: string; changes: { id: number; place: string }[] } | null = null;
    await page.route(`**${API}`, async (route) => {
      if (route.request().method() !== "POST") return route.fallback();
      body = route.request().postDataJSON();
      expect(route.request().headers()["x-panel"]).toBe("1");
      await route.fulfill({ json: { ok: true, changed: body!.changes.length, message: "Saved (caught by the test).", stamp: before.stamp } });
    });
    await save.click();
    await expect(page.locator(".toast").filter({ hasText: "caught by the test" })).toBeVisible();
    expect(body!.stamp).toBe(before.stamp);
    const sent = new Map(body!.changes.map((c) => [c.id, c.place]));
    expect([...sent.values()]).toContain("ZorkBase");
    expect([...sent.values()]).toContain("Catwalk");
    expect([...sent.values()]).not.toContain("CTSpawn");
    expect(sent.size).toBeGreaterThanOrEqual(changed);
    expect(errors, errors.join("\n")).toEqual([]);
    return;
  }

  await save.click();
  await expect(page.locator(".toast").filter({ hasText: /^Saved \d+ area name/ })).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".tl-actionbar")).not.toContainText("Unsaved changes");
  const saved: Nav = await (await request.get(API)).json();
  expect(saved.places).toEqual(expect.arrayContaining(["ZorkBase", "Catwalk"]));
  expect(saved.places).not.toContain("CTSpawn");
  expect(saved.stamp).not.toBe(before.stamp);
  // the areas themselves didn't move
  expect(saved.areas.map((a) => a.slice(0, 6))).toEqual(before.areas.map((a) => a.slice(0, 6)));

  // a reload shows the saved names
  await page.reload();
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  expect(await placeCounts(page)).toEqual(renamed);

  if (!KEEP) {
    // the old names back (through the same route the page uses)
    const back = saved.areas.filter((a) => (a[6] ? saved.places[a[6] - 1] : "") !== original[a[0]]).map((a) => ({ id: a[0], place: original[a[0]] }));
    const r = await (await request.post(API, { headers: { "X-Panel": "1" }, data: { stamp: saved.stamp, changes: back } })).json();
    expect(r.ok, JSON.stringify(r)).toBe(true);
    expect(names(await (await request.get(API)).json())).toEqual(original);
  }
  expect(errors, `${browserName}: ${errors.join("\n")}`).toEqual([]);
});

test("a suggestion never takes a name you just gave", async ({ page, request }) => {
  const before: Nav = await (await request.get(API)).json();
  test.skip(!before.areas?.length, `${MAP} has no nav here`);
  await page.goto(`/tools-dev.html?open=callouts:${MAP}`);
  const canvas = page.locator(".co-stage canvas");
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  const box = (await canvas.boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.3);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.7, { steps: 8 });
  await page.mouse.up();
  await page.getByLabel("Place name").fill("Library");
  await page.getByRole("button", { name: /^Name (it|them)$/ }).click();
  const mine = (await placeCounts(page)).Library;
  expect(mine).toBeGreaterThan(0);
  // the server names from the saved file, so it offers every area (even the ones named here)
  await page.route(`**${API}/suggest?from=marks`, (route) =>
    route.fulfill({ json: { changes: before.areas.map((a) => ({ id: a[0], place: "TSpawn" })), message: `${before.areas.length} areas named from the map's bomb sites, spawns and zones.` } }),
  );
  await page.getByRole("button", { name: "From bomb sites and spawns" }).click();
  await expect(page.locator(".toast").filter({ hasText: `${mine} areas you named yourself kept their names` })).toBeVisible();
  const after = await placeCounts(page);
  expect(after.Library).toBe(mine);
  expect(after.TSpawn).toBe(before.areas.length - mine);
});
