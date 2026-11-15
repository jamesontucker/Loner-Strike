// Map editor: the Checks panel's deeper checks (agent Checks). A small room built through the
// editor's commands with a func_wall z-fighting a world block and a T spawn sunk into the floor:
// the panel lists both with where they are, the suggested fix, Pick and Look; "Move it to free
// space" fixes the spawn in one click (one undo step), "Nudge 1 unit" the z-fighting; the
// stretched-texture limits in Settings change what's listed. Chromium here, WebKit in
// editor-checks-webkit.spec.ts (the Mac app's engine). Both run the tests in this file.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

async function open(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.removeItem("ls.editor.checkLimits");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
}

const cmd = async (page: Page, body: Record<string, unknown>) => {
  const r = await api(page, "/api/tools/editor/cmd", body);
  expect(r.error ?? null).toBeNull();
  return r;
};

/** A sealed room with a lamp, a func_wall on a world block (z-fighting on top) and a stuck T spawn. */
async function build(page: Page) {
  await api(page, "/api/tools/editor/new", {});
  const box = (min: number[], max: number[]) => cmd(page, { op: "add_box", min, max, texture: "CRATE" });
  await box([-272, -272, -16], [272, 272, 0]);
  await box([-272, -272, 256], [272, 272, 272]);
  await box([-272, -272, 0], [-256, 272, 256]);
  await box([256, -272, 0], [272, 272, 256]);
  await box([-256, -272, 0], [256, -256, 256]);
  await box([-256, 256, 0], [256, 272, 256]);
  await box([0, 0, 0], [64, 64, 64]);
  const wall = await box([16, 16, 32], [48, 48, 64]);
  await cmd(page, { op: "tie", brushes: wall.created.brushes, classname: "func_wall" });
  await cmd(page, { op: "add_entity", classname: "light", origin: [0, 0, 200] });
  await cmd(page, { op: "add_entity", classname: "info_player_start", origin: [-128, 0, 36] });
  const t = await cmd(page, { op: "add_entity", classname: "info_player_deathmatch", origin: [128, -128, 30] });
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  return { spawn: t.created.entities[0] as number };
}

async function showChecks(page: Page) {
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Checks/ }).first().click();
  const c = page.getByRole("region", { name: "Checks" });
  await expect(c.locator(".ck-bar")).toBeVisible({ timeout: 20000 });
  return c;
}

export function checksTests() {
  test("deeper checks list where, what to do, and fix in one click", async ({ page }) => {
    test.setTimeout(120_000);
    await open(page);
    const { spawn } = await build(page);
    const c = await showChecks(page);
    const stuck = c.locator('li[data-code="stuck-in-solid"]');
    await expect(stuck).toHaveCount(1, { timeout: 20000 });
    await expect(stuck).toContainText("info_player_deathmatch overlaps a solid brush");
    await expect(stuck).toContainText("at 128 -128 30");
    await expect(stuck).toContainText("moves it to 128 -128 36");
    const zf = c.locator('li[data-code="z-fighting"]');
    await expect(zf).toHaveCount(1);
    await expect(zf).toContainText("func_wall");
    await expect(zf.getByRole("button", { name: "NULL the hidden face" })).toBeVisible();
    // grouped, with counts
    await expect(c.locator(".ck-group h4", { hasText: "Entities" })).toBeVisible();
    await expect(c.locator(".ck-group h4", { hasText: "Geometry" })).toBeVisible();

    // Pick selects the spawn; Look rings it in 3D
    await stuck.getByRole("button", { name: "Pick" }).click();
    await expect(page.locator(".ed-selsize")).toContainText("1 picked");
    await stuck.getByRole("button", { name: "Look" }).click();
    await page.waitForTimeout(600);
    await page.screenshot({ path: test.info().outputPath("checks-look.png") });

    // one click: moved to free space, and one undo puts it back
    await stuck.getByRole("button", { name: "Move it to free space" }).click();
    await expect(c.locator('li[data-code="stuck-in-solid"]')).toHaveCount(0, { timeout: 15000 });
    let doc = await api(page, "/api/tools/editor/doc");
    expect(doc.entities.find((e: { id: number }) => e.id === spawn).keys).toContainEqual(["origin", "128 -128 36"]);
    expect(doc.undo).toBe("Fix a problem");
    expect((await api(page, "/api/tools/editor/undo", {})).error ?? null).toBeNull();
    doc = await api(page, "/api/tools/editor/doc");
    expect(doc.entities.find((e: { id: number }) => e.id === spawn).keys).toContainEqual(["origin", "128 -128 30"]);

    // the z-fighting's nudge
    await page.reload();
    const back = page.getByRole("button", { name: /^Back to/ });
    if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
    const c2 = await showChecks(page);
    await c2.locator('li[data-code="z-fighting"]').getByRole("button", { name: "Nudge 1 unit" }).click();
    await expect(c2.locator('li[data-code="z-fighting"]')).toHaveCount(0, { timeout: 15000 });
    await expect(c2.locator('li[data-code="stuck-in-solid"]')).toHaveCount(1);

    // filters
    await c2.getByRole("button", { name: /^Problems/ }).click();
    await expect(c2.locator('li[data-code="stuck-in-solid"]')).toHaveCount(1);
    await expect(c2.locator('li[data-code="hidden-faces"]')).toHaveCount(0);
    await c2.getByRole("button", { name: /^All/ }).click();

    // stretched textures: tighten the limits in Settings and a scale-1 face is listed
    await expect(c2.locator('li[data-code="stretched-texture"]')).toHaveCount(0);
    await c2.getByRole("button", { name: "Settings" }).click();
    await c2.getByRole("spinbutton", { name: "Largest texture scale" }).fill("0.5");
    await expect(c2.locator('li[data-code="stretched-texture"]').first()).toBeVisible({ timeout: 15000 });
    await c2.getByRole("spinbutton", { name: "Largest texture scale" }).fill("4");
    await expect(c2.locator('li[data-code="stretched-texture"]')).toHaveCount(0, { timeout: 15000 });

    // no horizontal overflow in the panel
    const wide = await c2.evaluate((el) => el.scrollWidth - el.clientWidth);
    expect(wide).toBeLessThanOrEqual(1);
  });
}
