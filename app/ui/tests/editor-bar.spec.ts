// Map editor: one bar (menus on the left; Save, Playtest, Publish on the right; everything the
// old button row had is in a menu with its keys), the bottom panels fold and stay folded, and
// the first-map tour (offered once on the first New map, Not now / Don't show again, Help >
// First map tour, steps tick off as you do them). Runs on editor-dev.html against the tools dev
// server; P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

async function newMap(page: Page, tour: "fresh" | "seen" = "seen") {
  await page.addInitScript((t) => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    if (t === "seen" && !sessionStorage.getItem("p5-tour-kept")) localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: false }));
  }, tour);
  await page.goto("/editor-dev.html");
  if (tour === "fresh") await page.evaluate(() => (localStorage.removeItem("ls.editor.tour"), sessionStorage.setItem("p5-tour-kept", "1")));
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(1200);
}

async function menu(page: Page, name: string) {
  await page.locator(".ed-menubar .ed-menu-title", { hasText: new RegExp(`^${name}$`) }).click();
  return page.locator(`.ed-menu-list[aria-label="${name}"]`);
}

test("one bar: menus left, Save / Playtest / Publish right, the old row's actions in menus with keys", async ({ page }) => {
  await newMap(page);
  await expect(page.locator(".editor-bar")).toHaveCount(0);
  const bar = page.getByRole("menubar", { name: "Editor menu" });
  for (const m of ["File", "Edit", "View", "Select", "Map", "Tools", "Help"]) await expect(bar.locator(".ed-menu-title", { hasText: new RegExp(`^${m}$`) })).toBeVisible();
  for (const b of ["Save", "Playtest", "Publish…"]) await expect(bar.getByRole("button", { name: b, exact: true })).toBeVisible();
  await expect(bar.getByRole("group", { name: "View", exact: true })).toBeVisible();
  // everything the old row had
  const file = await menu(page, "File");
  for (const item of ["New map", "Open a project…", "Import a .map…", "Save", "Save as…", "Export a .map…", "Map properties…"]) await expect(file.getByRole("menuitem").filter({ has: page.locator("span", { hasText: new RegExp(`^${item.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) }) })).toHaveCount(1);
  await expect(file.getByRole("menuitem", { name: /^Save/ }).first().locator("kbd")).toBeVisible();
  await page.keyboard.press("Escape");
  const edit = await menu(page, "Edit");
  await expect(edit.getByRole("menuitem", { name: /^Undo/ }).locator("kbd")).toBeVisible();
  await expect(edit.getByRole("menuitem", { name: /^Redo/ }).locator("kbd")).toBeVisible();
  await page.keyboard.press("Escape");
  const view = await menu(page, "View");
  await expect(view.getByRole("menuitem", { name: /^Top view/ }).locator("kbd")).toBeVisible();
  await page.keyboard.press("Escape");
  const map = await menu(page, "Map");
  await expect(map.getByRole("menuitem", { name: /^Playtest/ }).locator("kbd")).toHaveText("F5");
  await expect(map.getByRole("menuitem", { name: /^Publish…/ })).toBeVisible();
  await page.keyboard.press("Escape");
  // the view buttons work from the bar
  await bar.getByRole("group", { name: "View", exact: true }).getByRole("button", { name: "Top" }).click();
  await expect(bar.getByRole("group", { name: "View", exact: true }).getByRole("button", { name: "Top" })).toHaveAttribute("aria-pressed", "true");
});

test("the bottom panels fold to their tabs and stay folded", async ({ page }) => {
  await newMap(page);
  const fold = page.getByRole("button", { name: "Fold the bottom panels" });
  await fold.click();
  await expect(page.locator('[data-dock-body="bottom"]')).toHaveCount(0);
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 4000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await expect(page.locator('[data-dock-body="bottom"]')).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Open the bottom panels" })).toBeVisible();
  // a tab opens it again
  await page.locator('[data-dock-strip="bottom"] [role=tab]').first().click();
  await expect(page.locator('[data-dock-body="bottom"]')).toHaveCount(1);
});

test("first-map tour: offered once, Not now, replay from Help, steps tick off", async ({ page }) => {
  await newMap(page, "fresh");
  const offer = page.getByRole("dialog", { name: "First map tour" });
  await expect(offer).toBeVisible();
  // it never blocks: it's above the views, not on them
  const ob = (await offer.boundingBox())!;
  const pb = (await page.locator(".editor-main").boundingBox())!;
  expect(ob.y + ob.height).toBeLessThanOrEqual(pb.y + 1);
  await offer.getByRole("button", { name: "Not now" }).click();
  await expect(offer).toHaveCount(0);
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("ls.editor.tour") ?? "{}"))).toMatchObject({ offered: true, never: false });
  // Help > First map tour
  await (await menu(page, "Help")).getByRole("menuitem", { name: /^First map tour/ }).click();
  const tour = page.getByRole("complementary", { name: "First map tour" });
  await expect(tour).toBeVisible();
  await expect(tour).toContainText("1 of 5"); // (a new map has spawns for both teams)
  const step = (i: number) => tour.locator(".p5-tour-steps li").nth(i);
  // draw a room (the Box tool in Top)
  await page.locator(".editor-tool", { hasText: "Box" }).first().click();
  expect((await tour.boundingBox())!.y + (await tour.boundingBox())!.height).toBeLessThanOrEqual((await page.locator(".editor-main").boundingBox())!.y + 1);
  await page.locator('.ed-menubar [aria-label="View"] button', { hasText: /^Top$/ }).click();
  await page.waitForTimeout(500);
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  const c = [r.x + r.width / 2, r.y + r.height / 2];
  await page.mouse.move(c[0] - 60, c[1] - 40);
  await page.mouse.down();
  await page.mouse.move(c[0] + 60, c[1] + 40, { steps: 6 });
  await page.mouse.up();
  await expect(step(0)).toHaveAttribute("aria-checked", "true");
  // hollow it (the box just drawn is picked: Inspect's Hollow)
  await page.locator(".editor-tool", { hasText: "Select" }).first().click();
  await page.getByRole("button", { name: "Hollow", exact: true }).click();
  await expect(step(1)).toHaveAttribute("aria-checked", "true", { timeout: 8000 });
  // a light
  await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "light", origin: [0, 0, 120] });
  // (picked up from outside within a moment)
  await expect(step(2)).toHaveAttribute("aria-checked", "true", { timeout: 8000 });
  await expect(step(3)).toHaveAttribute("aria-checked", "true");
  // F5
  await page.locator(".ed-pane").first().hover();
  await page.keyboard.press("F5");
  await expect(step(4)).toHaveAttribute("aria-checked", "true");
  await expect(tour).toContainText("That's a map.");
  await tour.getByRole("button", { name: "Close the tour" }).click();
  await expect(tour).toHaveCount(0);
});

test("first-map tour: Don't show again", async ({ page }) => {
  await newMap(page, "fresh");
  const offer = page.getByRole("dialog", { name: "First map tour" });
  await offer.getByRole("button", { name: "Don't show again" }).click();
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("ls.editor.tour") ?? "{}"))).toMatchObject({ never: true });
  // a second New map doesn't offer it
  await page.locator(".ed-menubar .ed-menu-title", { hasText: /^File$/ }).click();
  await page.getByRole("menuitem", { name: /^New map/ }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.waitForTimeout(1500);
  await expect(offer).toHaveCount(0);
});

test("Content and Gameplay panels: names, subtitles, the View menu; an old saved layout keeps Gameplay", async ({ page }) => {
  // a layout saved when the panel was "Library" (same id)
  await page.addInitScript(() => {
    if (sessionStorage.getItem("p5-layout-set")) return;
    sessionStorage.setItem("p5-layout-set", "1");
    localStorage.setItem("ls.editor.dock2", JSON.stringify({ areas: { left: { tabs: [], active: null }, right: { tabs: ["outline", "library"], active: "library" }, right2: { tabs: ["inspect"], active: "inspect" }, bottom: { tabs: ["checks"], active: "checks" } }, sizes: { left: 300, right: 320, bottom: 240, rightSplit: 0.45 }, floats: [], popped: [] }));
  });
  await newMap(page);
  const right = page.locator('[data-dock-strip="right"]');
  await expect(right.getByRole("tab", { name: "Gameplay" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator('[data-dock-body="right"] .ed-dock-sub')).toHaveText("Components and mode kits for game modes");
  await expect(right.getByRole("tab", { name: "Gameplay" })).toHaveAttribute("title", /^Components and mode kits for game modes\./);
  // Content came into the saved layout too, with its subtitle
  const bottom = page.locator('[data-dock-strip="bottom"]');
  await bottom.getByRole("tab", { name: "Content" }).click();
  await expect(page.locator('[data-dock-body="bottom"] .ed-dock-sub')).toHaveText("Models, sprites, sounds, textures and prefabs");
  const view = await menu(page, "View");
  await expect(view.getByRole("menuitem", { name: /^Panel: Gameplay/ })).toHaveAttribute("title", /^Components and mode kits for game modes\./);
  await expect(view.getByRole("menuitem", { name: /^Panel: Content/ })).toBeVisible();
  await expect(view.getByRole("menuitem", { name: /^Panel: Library/ })).toHaveCount(0);
});
