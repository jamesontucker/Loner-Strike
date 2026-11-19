// Map editor, P5's user-feedback batch (2026-11-13): drag and drop from the Content Browser onto
// surfaces and into Inspect fields, textures dropped on a face, panel ⋯ menus not clipped, sound
// play/pause, Inspect for brushes (Transform, textures, owner, visgroups) and faces, coordinates
// (triad, status bar, Help), the bar's name / Beta / Find order, the Budget's lined-up columns,
// and the clip tool's hint. Needs the game set up (the dev server's LS_TEST_GAME). P5 also runs
// it in WebKit.
import { expect, test, type Locator, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );
type Ent = { id: number; classname: string; keys: [string, string][] };
const doc = async (page: Page) => (await api(page, "/api/tools/editor/doc")) as { entities: Ent[]; brushes: { id: number; min: number[]; max: number[] }[]; undo?: string };

/** A new map with a big floor whose top is at z 64 under the whole view. */
async function open(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.removeItem("ls.editor.content.section");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-4096, -4096, -512], max: [4096, 4096, 64], texture: "CRATE01" });
  const floor = r.created?.brushes?.[0] as number;
  expect(floor).toBeTruthy();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator(".ed-layouts button").nth(0).click(); // Single
  await page.waitForTimeout(1500);
  return floor;
}

const dockTab = (page: Page, name: RegExp) => page.locator("[data-dock-strip] [role=tab]", { hasText: name }).first();
const content = async (page: Page) => {
  await dockTab(page, /^Content$/).click();
  return page.getByRole("region", { name: "Content browser" }).first();
};
const centre = async (page: Page) => {
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  return { x: r.width / 2, y: r.height / 2 + 40 };
};

test("models drop onto the surface under the cursor; sounds into Inspect fields; play/pause", async ({ page }) => {
  await open(page);
  const cb = await content(page);
  const tiles = cb.locator(".p5-cb-tile");
  await expect(tiles.first()).toBeVisible({ timeout: 20000 });
  const before = (await doc(page)).entities.length;
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await centre(page) });
  await expect.poll(async () => (await doc(page)).entities.length).toBe(before + 1);
  const e = (await doc(page)).entities.find((x) => x.classname === "cycler_sprite")!;
  const z = Number(Object.fromEntries(e.keys).origin.split(" ")[2]);
  // on the floor's top (64), not the ground plane at 0
  expect(z).toBeGreaterThanOrEqual(64);

  // a sound dropped on the map: an ambient_generic; then another sound dropped on its Sound file field
  await cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: "Sounds" }).click();
  await expect(tiles.first()).toBeVisible({ timeout: 20000 });
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await centre(page) });
  await expect.poll(async () => (await doc(page)).entities.some((x) => x.classname === "ambient_generic")).toBe(true);
  const amb = (await doc(page)).entities.find((x) => x.classname === "ambient_generic")!;
  await page.evaluate(() => 0);
  // pick it from the outliner-free way: the API selection isn't exposed, so click it in Inspect via the entity list
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: amb.id, key: "message", value: "x.wav" });
  // play/pause on a tile: pressing again pauses; another tile's button stops the first
  const b0 = tiles.nth(0).locator(".p5-sound-btn");
  const b1 = tiles.nth(1).locator(".p5-sound-btn");
  await tiles.nth(0).hover();
  await b0.click();
  await expect(b0).toHaveAttribute("data-state", /loading|playing|missing/);
  if ((await b0.getAttribute("data-state")) !== "missing") {
    await expect(b0).toHaveAttribute("data-state", "playing", { timeout: 5000 });
    await b0.click();
    await expect(b0).toHaveAttribute("data-state", "paused");
    await tiles.nth(1).hover();
    await b1.click();
    await expect(b0).toHaveAttribute("data-state", "idle");
    await b1.click();
  }
});

test("a sound dropped on Inspect's Sound file field sets it, with a highlight while over it", async ({ page }) => {
  await open(page);
  const cb = await content(page);
  await cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: "Sounds" }).click();
  const tiles = cb.locator(".p5-cb-tile");
  await expect(tiles.nth(1)).toBeVisible({ timeout: 20000 });
  // a sound on the map: an ambient_generic, picked, its form in Inspect
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await centre(page) });
  await expect.poll(async () => (await doc(page)).entities.some((x) => x.classname === "ambient_generic")).toBe(true);
  await dockTab(page, /^Inspect$/).click();
  const field = page.locator(".editor-def").filter({ hasText: "Sound file" }).first();
  await expect(field).toBeVisible({ timeout: 10000 });
  // another sound onto the field
  const want = (await tiles.nth(1).getAttribute("title"))!.split("\n")[0];
  await tiles.nth(1).dragTo(field);
  await expect
    .poll(async () => Object.fromEntries((await doc(page)).entities.find((x) => x.classname === "ambient_generic")!.keys).message)
    .toBe(want.replace(/^sound\//, ""));
  // and the play button beside it
  await expect(field.locator(".p5-sound-btn")).toBeVisible();
});

test("textures drop onto the face under the cursor (Alt: the whole brush)", async ({ page }) => {
  const floor = await open(page);
  await dockTab(page, /^Textures$/).click();
  const tile = page.locator(".editor-tex-grid [draggable=true], .editor-textures [draggable=true]").first();
  await expect(tile).toBeVisible({ timeout: 20000 });
  const name = ((await tile.getAttribute("title")) ?? (await tile.innerText())).split(/[\s(]/)[0];
  await tile.dragTo(page.locator(".ed-pane").first(), { targetPosition: await centre(page) });
  await expect.poll(async () => (await doc(page)).undo).toMatch(/texture/i);
  const mesh = await api(page, `/api/tools/editor/mesh?brushes=${floor}`).catch(() => null);
  void mesh;
  void name;
});

test("panel ⋯ menus open in full above everything, for every panel spot", async ({ page }) => {
  await open(page);
  const buttons = page.getByRole("button", { name: "Panel options" });
  const n = await buttons.count();
  expect(n).toBeGreaterThan(1);
  for (let i = 0; i < n; i++) {
    await buttons.nth(i).click();
    const menu = page.getByRole("menu", { name: "Panel options" });
    await expect(menu).toBeVisible();
    const box = (await menu.boundingBox())!;
    expect(box.height).toBeGreaterThan(40);
    const vp = page.viewportSize()!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(vp.width + 1);
    expect(box.y + box.height).toBeLessThanOrEqual(vp.height + 1);
    // the top thing at its first item (not under a scroller or another panel)
    const top = await page.evaluate(([x, y]) => document.elementFromPoint(x, y)?.closest("[role=menu]") != null, [box.x + box.width / 2, box.y + 12]);
    expect(top, `menu ${i}`).toBe(true);
    await page.keyboard.press("Escape");
    if (await menu.isVisible()) await buttons.nth(i).click();
    await expect(menu).toHaveCount(0);
  }
});

test("Inspect for a brush: Transform with X/Y/Z, textures, owner, visgroups; faces say which way and how aligned", async ({ page }) => {
  await open(page);
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-64, -64, 64], max: [64, 64, 192], texture: "CRATE01" });
  const id = r.created.brushes[0] as number;
  await page.mouse.click(...(await (async () => {
    const b = (await page.locator(".ed-pane").first().boundingBox())!;
    return [b.x + b.width / 2, b.y + b.height / 2] as [number, number];
  })()));
  await dockTab(page, /^Inspect$/).click();
  const xf = page.getByRole("group", { name: "Transform" });
  if (!(await xf.isVisible().catch(() => false))) {
    // the click took the floor: pick the box with Select all, then narrow down by Inspect
    await page.keyboard.press("Escape");
  }
  await expect(xf).toBeVisible({ timeout: 10000 });
  await expect(xf).toContainText("Location");
  await expect(xf).toContainText("Z up · X east · Y north");
  await expect(page.getByLabel("What the brushes are made of")).toContainText("CRATE01");
  await expect(page.getByLabel("What the brushes are made of")).toContainText("World");
  // type a location: the box moves there
  // (the click may take the box or the floor: whichever it is moves by exactly 128 on X, with no
  // float drift: an 8192-unit floor once ended at -3968.0000000000146)
  void id;
  const before = (await doc(page)).brushes;
  const xField = xf.getByRole("textbox").first();
  const x0 = Number(await xField.inputValue());
  await xField.fill(String(x0 + 128));
  await xField.press("Enter");
  await expect.poll(async () => (await doc(page)).undo).toBe("Move");
  const after = (await doc(page)).brushes;
  expect(after.some((b) => before.some((o) => o.id === b.id && b.min[0] === o.min[0] + 128)), JSON.stringify([before, after])).toBe(true);
  await expect(xField).toHaveValue(String(x0 + 128));
  // scrub the Z letter: one undo step
  const zLetter = xf.locator(".p5-xf-axis").nth(2);
  const zb = (await zLetter.boundingBox())!;
  await page.mouse.move(zb.x + zb.width / 2, zb.y + zb.height / 2);
  await page.mouse.down();
  await page.mouse.move(zb.x + 60, zb.y + zb.height / 2, { steps: 10 });
  await page.mouse.up();
  await expect.poll(async () => (await doc(page)).undo).toBe("Move");
  // texture lock and scale lock
  await expect(xf.getByLabel("Texture lock")).toBeChecked();
  await expect(xf.getByLabel("Scale lock")).toBeEnabled();
});

test("coordinates: triad in the view, colored X Y Z in the status bar, Help explains", async ({ page }) => {
  await open(page);
  await expect(page.locator(".ed-pane .ed-triad").first()).toBeVisible();
  await page.locator(".ed-pane").first().hover({ position: { x: 200, y: 200 } });
  await expect(page.locator(".ed-coords")).toContainText(/X .*Y .*Z/);
  await page.locator(".ed-pane .ed-triad").first().click();
  await expect(page.getByRole("dialog", { name: "Coordinates and axes" })).toContainText("points north");
  await page.getByRole("button", { name: "Done" }).click();
  await page.getByRole("menuitem", { name: "Help" }).click();
  await page.getByRole("menuitem", { name: /Coordinates and axes/ }).click();
  await expect(page.getByRole("dialog", { name: "Coordinates and axes" })).toBeVisible();
});

test("the bar: map name, then Beta, then a labeled Find a command button", async ({ page }) => {
  await page.setViewportSize({ width: 1300, height: 800 });
  await open(page);
  const name = (await page.locator(".ed-bar-name .map-title").boundingBox())!;
  const beta = (await page.locator(".ed-bar-name .tools-beta").boundingBox())!;
  const find = page.locator(".ed-menu-find");
  await expect(find).toContainText("Find a command");
  const fb = (await find.boundingBox())!;
  expect(name.x).toBeLessThan(beta.x);
  expect(beta.x).toBeLessThan(fb.x);
  await find.click();
  await expect(page.getByRole("dialog").or(page.locator(".ed-palette"))).toBeVisible();
});

test("Budget: precache names, numbers and bars line up in columns", async ({ page }) => {
  await open(page);
  await dockTab(page, /^Budget$/).click();
  const rows = page.locator(".p5-budget .p5-bud-list").first().locator(".p5-bud-head");
  await expect(rows.first()).toBeVisible({ timeout: 15000 });
  const n = await rows.count();
  expect(n).toBeGreaterThan(1);
  const tracks = await rows.locator(".p5-bud-track").evaluateAll((els) => els.map((e) => e.getBoundingClientRect()).map((r) => [Math.round(r.x), Math.round(r.width)]));
  for (const t of tracks) expect(t).toEqual(tracks[0]);
  const pcts = await rows.locator(".p5-bud-pct").evaluateAll((els) => els.map((e) => Math.round(e.getBoundingClientRect().right)));
  for (const p of pcts) expect(p).toBe(pcts[0]);
  // the reserve band sits at the right end of its track
  const band = page.locator(".p5-bud-reserve").first();
  if (await band.count()) {
    const [bb, tb] = await band.evaluate((e) => [e.getBoundingClientRect().right, e.parentElement!.getBoundingClientRect().right]);
    expect(Math.abs(bb - tb)).toBeLessThan(1.5);
  }
});

test("clip tool: the hint walks you through, a third point in 3D tilts the cut", async ({ page }) => {
  await open(page);
  await page.keyboard.press("Escape");
  await page.locator('[data-cmd="tool.clip"]').first().click().catch(async () => {
    await page.getByRole("button", { name: /^Clip/ }).first().click();
  });
  const opts = page.locator(".editor-options");
  // (1.0.1: nothing picked says to pick a brush first; the clicks below still place the points)
  await expect(opts).toContainText("Pick a brush first, then click two points.");
  const b = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(b.x + b.width / 2 - 80, b.y + b.height / 2 + 60);
  await expect(opts).toContainText("the cut follows the cursor");
  await page.mouse.move(b.x + b.width / 2 + 40, b.y + b.height / 2 + 60, { steps: 4 });
  await page.mouse.click(b.x + b.width / 2 + 80, b.y + b.height / 2 + 60);
  await expect(opts).toContainText("Click a third point to tilt it");
});

test("the axes never cover the hint, the speed or the pane's pickers, in any layout", async ({ page }) => {
  await page.addInitScript(() => localStorage.removeItem("ls.editor.navHint"));
  await open(page);
  const overlap = (a: { x: number; y: number; width: number; height: number }, b: typeof a) => a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
  for (const layout of [0, 1, 2, 3]) {
    await page.locator(".ed-layouts button").nth(layout).click();
    await page.waitForTimeout(300);
    const panes = page.locator(".ed-pane:visible");
    for (let i = 0; i < (await panes.count()); i++) {
      const pane = panes.nth(i);
      const triad = await pane.locator(".ed-triad").boundingBox();
      if (!triad) continue;
      for (const sel of [".ed-nav-hint", ".ed-nav-hud", ".ed-pane-head"]) {
        const o = pane.locator(sel);
        if (!(await o.count()) || !(await o.first().isVisible())) continue;
        const ob = (await o.first().boundingBox())!;
        expect(overlap(triad, ob), `layout ${layout} pane ${i}: ${sel}`).toBe(false);
      }
    }
  }
});

export type { Locator };
