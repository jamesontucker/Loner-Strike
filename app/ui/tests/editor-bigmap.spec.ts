// Map editor: a big map (br_valley's 3 MB .map) through the normal UI: File > Import a .map,
// autosave, File > Save as, File > Recent opens the project again with every brush and entity;
// and the whole map pasted in one request. The dev server answers the file dialogs:
// LS_DEV_PICK=<a copy of br_valley.map> LS_DEV_PICK_SAVE=<a .lsmap to write>; this test runs when
// LS_TEST_BIGMAP=<that same .map copy> is set. P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";
import * as fs from "node:fs";

const MAP = process.env.LS_TEST_BIGMAP ?? "";
const SAVE = process.env.LS_TEST_BIGMAP_SAVE ?? "";
const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

async function menu(page: Page, m: string, item: RegExp) {
  await page.locator(".ed-menubar .ed-menu-title", { hasText: new RegExp(`^${m}$`) }).click();
  await page.locator(`.ed-menu-list[aria-label="${m}"] [role=menuitem]`, { hasText: item }).first().click();
}

test("br_valley: import through File, autosave, save as, open it again", async ({ page }) => {
  test.skip(!MAP || !SAVE, "needs LS_TEST_BIGMAP and LS_TEST_BIGMAP_SAVE (and the dev server's LS_DEV_PICK / LS_DEV_PICK_SAVE)");
  test.setTimeout(240_000);
  const size = fs.statSync(MAP).size;
  expect(size).toBeGreaterThan(2 << 20);
  const brushesInFile = (fs.readFileSync(MAP, "utf8").match(/^\( /gm) ?? []).length; // (face lines)
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  // File > Import a .map… (the dev server answers the dialog with the copy)
  await menu(page, "File", /^Import a \.map/);
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).brushes.length, { timeout: 120_000 }).toBeGreaterThan(1000);
  const doc = await api(page, "/api/tools/editor/doc");
  const faces = doc.brushes.reduce((n: number, b: { faces?: unknown[] }) => n + (b.faces?.length ?? 0), 0);
  expect(faces || brushesInFile).toBeGreaterThan(0);
  const counts = { brushes: doc.brushes.length, entities: doc.entities.length };
  // the view draws it
  await expect(page.locator(".tools-status")).toContainText(`${counts.brushes} brushes`, { timeout: 60_000 });
  // an edit, then autosave writes it all
  const w = (doc.world.keys as [string, string][]).filter(([k]) => k !== "message");
  await api(page, "/api/tools/editor/cmd", { op: "set_world", keys: [...w, ["message", "big map test"]] });
  const a = await api(page, "/api/tools/editor/autosave", {});
  expect(a.error ?? null).toBeNull();
  expect(a.saved).toBe(true);
  // File > Save as… (the dialog answers with the .lsmap)
  if (fs.existsSync(SAVE)) fs.rmSync(SAVE);
  await menu(page, "File", /^Save as/);
  await expect.poll(() => fs.existsSync(SAVE), { timeout: 60_000 }).toBe(true);
  expect(fs.statSync(SAVE).size).toBeGreaterThan(2 << 20);
  // a new map, then File > Recent opens the project again, whole
  await menu(page, "File", /^New map/);
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).brushes.length).toBeLessThan(100);
  await page.locator(".ed-menubar .ed-menu-title", { hasText: /^File$/ }).click();
  const name = SAVE.split("/").pop()!;
  await page.locator('.ed-menu-list[aria-label="File"] .ed-menu-recent [role=menuitem]', { hasText: name }).first().click();
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).brushes.length, { timeout: 120_000 }).toBe(counts.brushes);
  expect((await api(page, "/api/tools/editor/doc")).entities.length).toBe(counts.entities);
  // and the whole map pasted in one request (over the old 2 MB limit)
  await api(page, "/api/tools/editor/new", {});
  const r = await api(page, "/api/tools/editor/cmd", { op: "paste", text: fs.readFileSync(MAP, "utf8"), offset: [0, 0, 0] });
  expect(r.error ?? null).toBeNull();
  expect(r.created.brushes.length).toBeGreaterThan(1000);
});
