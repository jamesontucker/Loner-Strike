// Map editor: the Content Browser (docs/notes/Assets.md 3.2). Models with 3D thumbnails, sprites
// decoded (animated in the details), sounds with play and their facts, a source tree, search,
// thumbnail size, Favorites; the details pane with where a file came from and its permission
// badge; drag a tile onto the map (model → cycler_sprite, sprite → env_sprite, sound →
// ambient_generic); Find uses; the content drawer (Ctrl/⌘+Space); the entity form's Choose… /
// Show in Content Browser / Use selected. Needs the game set up (the dev server's LS_TEST_GAME).
// P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

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
  await page.locator('[data-dock-strip] [role=tab]', { hasText: /^Content$/ }).first().click();
  return page.getByRole("region", { name: "Content browser" }).first();
}

// a spot near the middle of the first view, so the drop lands on it at any window size (P5: at
// 1024x680 the Content dock leaves the view about 240 px tall)
const paneSpot = async (page: Page, dx: number, dy: number) => {
  const b = (await page.locator(".ed-pane").first().boundingBox())!;
  return { x: Math.round(b.width / 2 + dx), y: Math.round(b.height / 2 + dy) };
};

const tab = (cb: ReturnType<Page["locator"]>, name: string) => cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name });

test("models: thumbnails, search, size, sources, details, drag onto the map, Find uses", async ({ page }) => {
  const cb = await open(page);
  await expect(tab(cb, "Models")).toHaveAttribute("aria-selected", "true");
  const tiles = cb.locator(".p5-cb-tile");
  await expect(tiles.first()).toBeVisible({ timeout: 20000 });
  // a 3D thumbnail is drawn
  await expect(cb.locator(".p5-cb-tile .p5-cb-thumb img").first()).toBeVisible({ timeout: 20000 });
  // search
  await cb.getByRole("searchbox", { name: "Search" }).fill("hostage");
  await expect(tiles.first()).toContainText(/hostage/i);
  const n = await tiles.count();
  expect(n).toBeGreaterThan(0);
  // size
  const w0 = (await cb.locator(".p5-cb-thumb").first().boundingBox())!.width;
  await cb.getByRole("slider", { name: "Thumbnail size" }).fill("160");
  await expect.poll(async () => (await cb.locator(".p5-cb-thumb").first().boundingBox())!.width).toBeGreaterThan(w0 + 20);
  // sources: the game's own
  await cb.getByRole("navigation", { name: "Where from" }).getByRole("button", { name: /^Game/ }).click();
  await expect(tiles.first()).toBeVisible();
  // details
  await tiles.first().click();
  const details = cb.getByRole("complementary", { name: "Details" });
  await expect(details).toContainText("The game's own");
  await expect(details.locator(".p5-cb-badge")).toHaveText(/Not checked yet|Game's own|OK to share|Ask the author|Not allowed/);
  // drag onto the map: a cycler_sprite with that model
  const path = (await tiles.first().getAttribute("title"))!.split("\n")[0];
  const before = (await api(page, "/api/tools/editor/doc")).entities.length;
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await paneSpot(page, 40, 20) });
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).entities.length).toBe(before + 1);
  const e = (await api(page, "/api/tools/editor/doc")).entities.find((x: { classname: string }) => x.classname === "cycler_sprite");
  expect(Object.fromEntries(e.keys).model).toBe(path);
  // Find uses (right-click)
  await tiles.first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Find uses in the map" }).click();
  await expect(cb.getByRole("dialog", { name: /^Uses of/ })).toContainText("1 entity uses");
  // Favorites
  await tiles.first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Add to favorites" }).click();
  await cb.getByRole("navigation", { name: "Where from" }).getByRole("button", { name: /Favorites/ }).click();
  await expect(tiles).toHaveCount(1);
});

test("sprites and sounds: decoded pictures, animated preview, play, facts, placing", async ({ page }) => {
  const cb = await open(page);
  await tab(cb, "Sprites").click();
  const tiles = cb.locator(".p5-cb-tile");
  await expect(tiles.first()).toBeVisible({ timeout: 20000 });
  await cb.getByRole("searchbox", { name: "Search" }).fill("muzzleflash");
  const img = tiles.first().locator("img");
  await tiles.first().scrollIntoViewIfNeeded();
  await expect.poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth)).toBeGreaterThan(0);
  await tiles.first().click();
  const details = cb.getByRole("complementary", { name: "Details" });
  await expect(details).toContainText(/frame/);
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await paneSpot(page, 0, 10) });
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).entities.some((x: { classname: string }) => x.classname === "env_sprite")).toBe(true);
  // sounds
  await tab(cb, "Sounds").click();
  await expect(tiles.first()).toBeVisible({ timeout: 20000 });
  await cb.getByRole("searchbox", { name: "Search" }).fill("ambience");
  await tiles.first().click();
  await expect(details).toContainText(/kHz/);
  await expect(tiles.first().getByRole("button", { name: /^Play / })).toBeVisible();
  await tiles.first().dragTo(page.locator(".ed-pane").first(), { targetPosition: await paneSpot(page, -40, 0) });
  await expect.poll(async () => {
    const a = (await api(page, "/api/tools/editor/doc")).entities.find((x: { classname: string }) => x.classname === "ambient_generic");
    return a ? Object.fromEntries(a.keys).message : null;
  }).toMatch(/^ambience\//);
});

test("the entity form: Choose…, Show in Content Browser, Use selected; the content drawer", async ({ page }) => {
  const cb = await open(page);
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "env_sprite", origin: [0, 0, 64] });
  const id = r.created.entities[0];
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 4000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator('[data-dock-strip] [role=tab]', { hasText: /^Outline$/ }).first().click();
  await page.getByLabel(/Find in the map|Find by name/).first().fill("env_sprite");
  await page.locator(".ed-ol-row", { hasText: "env_sprite" }).first().click();
  // Choose…
  await page.getByRole("button", { name: "Choose…" }).first().click();
  const pop = page.getByRole("dialog", { name: "Choose a sprite" });
  await pop.getByRole("searchbox").fill("muzzleflash");
  await pop.getByRole("option").first().getByRole("button").click();
  await expect.poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((x: { id: number }) => x.id === id).keys).model).toMatch(/^sprites\/muzzleflash/);
  // Show in Content Browser
  await page.getByRole("button", { name: /^Show .* in the Content browser$/ }).first().click();
  const browser = page.getByRole("region", { name: "Content browser" }).first();
  await expect(tab(browser, "Sprites")).toHaveAttribute("aria-selected", "true");
  await expect(browser.locator('.p5-cb-tile[aria-selected="true"]')).toContainText(/muzzleflash/i);
  // Use selected: pick another sprite there
  await browser.getByRole("searchbox", { name: "Search" }).fill("smoke");
  await browser.locator(".p5-cb-tile").first().click();
  await page.locator('[data-dock-strip] [role=tab]', { hasText: /^Inspect$/ }).first().click();
  await page.getByRole("button", { name: "Use selected" }).first().click();
  await expect.poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((x: { id: number }) => x.id === id).keys).model).toMatch(/smoke/);
  void cb;
  // the content drawer: Ctrl/⌘+Space, a click away closes it
  await page.locator(".ed-pane").first().hover();
  // Ctrl+Space as in Unreal; ⌘⇧Space on a Mac (⌘Space is Spotlight, Ctrl+Space the input source)
  const mac = await page.evaluate(() => /Mac/.test(navigator.platform));
  await page.locator(".ed-menubar .ed-menu-title", { hasText: /^View$/ }).click();
  await expect(page.locator('.ed-menu-list[aria-label="View"]').getByRole("menuitem", { name: /^Content drawer/ }).locator("kbd")).toHaveText(mac ? "⌘⇧Space" : "Ctrl+Space");
  await page.keyboard.press("Escape");
  await page.locator(".ed-pane").first().hover();
  await page.keyboard.press(mac ? "Meta+Shift+Space" : "Control+Space");
  const drawer = page.getByRole("dialog", { name: "Content drawer" });
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole("region", { name: "Content browser" })).toBeVisible();
  const pane = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(pane.x + 40, pane.y + 40);
  await expect(drawer).toHaveCount(0);
});
