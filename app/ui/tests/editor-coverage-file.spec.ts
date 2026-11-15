// Editor coverage sweep: files. The start screen, New / Open / Recent / Import (.map, Hammer
// .rmf) / Save / Save as / Export, autosave recovery, and the Project panel's file actions.
// The dev server answers file dialogs with LS_DEV_PICK (a symlink re-pointed here, LS_COV_PICK)
// and LS_DEV_PICK_SAVE (LS_COV_SAVE). See editor-coverage.helpers.ts.
import { expect, test, type Page } from "@playwright/test";
import { copyFileSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { clearSaved, cmd, counts, FIX_DIR, fixture, installHooks, key, menu, MOD, openEditor, PICK, pickFile, readSaved, SAVE, st, testMap } from "./editor-coverage.helpers";
import { sampleRmf } from "./editor-coverage.rmf";

test.skip(!PICK || !SAVE, "needs LS_COV_PICK and LS_COV_SAVE (the dev server's LS_DEV_PICK / LS_DEV_PICK_SAVE)");

// The dev server tells a project from a .map by the picked path's extension, so one server
// can't do both: run this file once with LS_DEV_PICK ending .map (imports) and once ending
// .lsmap (the Open a project tests). Each test skips on the other.
const OPEN = PICK.endsWith(".lsmap");
const needsMapPick = () => test.skip(OPEN, "this run's LS_DEV_PICK ends .lsmap (Open a project run)");
const needsProjectPick = () => test.skip(!OPEN, "needs LS_DEV_PICK ending .lsmap");

const TB = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "trenchbroom");

async function toStart(page: Page) {
  // the dev page opens on the start screen each time (the map stays open in the server)
  await page.reload();
  await installHooks(page);
  await expect(page.getByRole("button", { name: "New map" })).toBeVisible();
}

test("Save, Save as and Export write their files; Open and Recent open the project", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  const n = await counts(page);
  clearSaved();
  // Save (the bar's button): an unsaved map asks where (the dialog answers LS_DEV_PICK_SAVE)
  await page.locator(".ed-bar-actions").getByRole("button", { name: "Save" }).click();
  await expect.poll(() => st(page, (s) => s.doc.dirty)).toBe(false);
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  expect(await st(page, (s) => s.doc.saved)).toBe(true);
  // File > Save after a change: same file, clean again
  await cmd(page, { op: "set_key", entity: m.light, key: "_light", value: "255 0 0 300" });
  await expect.poll(() => st(page, (s) => s.doc.dirty)).toBe(true);
  await menu(page, "File", "Save");
  await expect.poll(() => st(page, (s) => s.doc.dirty)).toBe(false);
  // File > Save as… (asks again, same answer here)
  clearSaved();
  await menu(page, "File", "Save as…");
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  const project = join(FIX_DIR, "cov-project.lsmap");
  copyFileSync(SAVE, project);
  // File > Export a .map…: Valve 220 text
  clearSaved();
  await menu(page, "File", "Export a .map…");
  await expect.poll(() => readSaved()?.toString("latin1") ?? "").toContain('"mapversion" "220"');
  const text = readSaved()!.toString("latin1");
  expect(text).toContain('"classname" "trigger_multiple"');
  expect(text).toContain('"_light" "255 0 0 300"');
  // Recent files: in the File menu and on the start screen
  await menu(page, "File", "New map");
  await page.locator(".ed-menubar .ed-menu-title", { hasText: /^File$/ }).click();
  const recent = page.locator('.ed-menu-list[aria-label="File"] .ed-menu-recent button').first();
  await expect(recent).toBeVisible();
  await recent.click();
  await expect.poll(() => counts(page)).toEqual(n);
  await toStart(page);
  await expect(page.locator(".editor-recent-files li").first()).toBeVisible();
  await page.locator(".editor-recent-files button").first().click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => counts(page)).toEqual(n);
});

test("keys: Mod+S saves, Mod+Shift+S saves as, Mod+E exports, Mod+Alt+N starts a new map", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  const n = await counts(page);
  clearSaved();
  await key(page, `${MOD}+KeyS`);
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  await expect.poll(() => st(page, (s) => s.doc.dirty)).toBe(false);
  clearSaved();
  await key(page, `${MOD}+Shift+KeyS`);
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  const project = join(FIX_DIR, "cov-project-keys.lsmap");
  copyFileSync(SAVE, project);
  clearSaved();
  await key(page, `${MOD}+KeyE`);
  await expect.poll(() => readSaved()?.toString("latin1") ?? "").toContain('"mapversion" "220"');
  await key(page, `${MOD}+Alt+KeyN`);
  await expect.poll(() => st(page, (s) => s.doc.name)).toBe("Untitled");
  void project;
  void n;
});

test("start screen: New map, Import a .map, Import from Hammer, Back to the open map", async ({ page }) => {
  needsMapPick();
  await openEditor(page);
  await testMap(page);
  await toStart(page);
  // the open map is offered back
  await expect(page.getByRole("button", { name: /^Back to / })).toBeVisible();
  await page.getByRole("button", { name: /^Back to / }).click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  // Import a .map (TrenchBroom's standard format, the tooltip names TrenchBroom)
  await toStart(page);
  const imp = page.getByRole("button", { name: "Import a .map" });
  await expect(imp).toHaveAttribute("title", /TrenchBroom/);
  pickFile(join(TB, "tb_standard.map"));
  await imp.click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => counts(page)).toEqual({ brushes: 7, entities: 2, groups: 1 });
  // Import from Hammer / J.A.C.K.: an .rmf
  await toStart(page);
  pickFile(fixture("cov.rmf", sampleRmf()));
  await page.getByRole("button", { name: "Import from Hammer / J.A.C.K." }).click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => counts(page)).toEqual({ brushes: 3, entities: 4, groups: 1 });
  expect(await st(page, (s) => s.doc.entities.map((e: { classname: string }) => e.classname).sort())).toEqual(["func_door", "light", "path_corner", "path_corner"]);
  // New map from the start screen
  await toStart(page);
  await page.getByRole("button", { name: "New map" }).click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => st(page, (s) => s.doc.name)).toBe("Untitled");
});

test("File menu imports: Import a .map… and Import a Hammer .rmf or J.A.C.K. .jmf…", async ({ page }) => {
  needsMapPick();
  await openEditor(page);
  pickFile(join(TB, "tb_valve_q2extras.map"));
  await menu(page, "File", "Import a .map…");
  await expect.poll(() => counts(page)).toEqual({ brushes: 6, entities: 2, groups: 0 });
  pickFile(fixture("cov.rmf", sampleRmf()));
  await menu(page, "File", "Import a Hammer .rmf or J.A.C.K. .jmf…");
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect.poll(() => counts(page)).toEqual({ brushes: 3, entities: 4, groups: 1 });
  // what didn't come over is said (the path and its nodes come in as entities? the import says)
  await expect(page.locator(".toast, [role=status]").filter({ hasText: /Imported/ }).first()).toBeVisible();
});

test("a file that isn't a map says so in words", async ({ page }) => {
  needsMapPick();
  await openEditor(page);
  pickFile(fixture("not-a-map.map", "hello, this is not a map"));
  await menu(page, "File", "Import a .map…");
  await expect(page.locator(".toast, [role=status], .banner").filter({ hasText: /Line 1|expected|isn't/ }).first()).toBeVisible();
  pickFile(fixture("not-an-rmf.rmf", Buffer.from("nope")));
  await menu(page, "File", "Import a Hammer .rmf or J.A.C.K. .jmf…");
  await expect(page.locator(".toast, [role=status], .banner").filter({ hasText: /isn't a Hammer .rmf or J.A.C.K. .jmf/ }).first()).toBeVisible();
});

test("autosave recovery: Throw it away and Recover it", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  const n = await counts(page);
  const autosave = () => page.evaluate(() => fetch("/api/tools/editor/autosave", { method: "POST", headers: { "X-Panel": "1", "Content-Type": "application/json" }, body: "{}" }).then((r) => r.json()));
  expect((await autosave()).saved).toBe(true);
  await toStart(page);
  await expect(page.getByRole("button", { name: "Recover it" })).toBeVisible();
  await page.getByRole("button", { name: "Throw it away" }).click();
  await expect(page.getByRole("button", { name: "Recover it" })).toHaveCount(0);
  await page.getByRole("button", { name: /^Back to / }).click();
  expect((await autosave()).saved).toBe(true);
  await toStart(page);
  await page.getByRole("button", { name: "Recover it" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => counts(page)).toEqual(n);
});

test("Project panel: Save as a project, Look again, Rename, Delete; Open and Show ask the app", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "File", "Project files");
  const panel = page.locator(".tools-panel").filter({ has: page.getByRole("button", { name: "Look again" }) }).first();
  clearSaved();
  await panel.getByRole("button", { name: "Save as a project…" }).click();
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  await panel.getByRole("button", { name: "Look again" }).click();
  // Open and Show hand the file to the system (a Finder window): caught here, the request checked
  const asked: { action: string; file?: string }[] = [];
  await page.route("**/api/tools/editor/project/action", async (r) => {
    const b = r.request().postDataJSON();
    if (b.action === "open" || b.action === "reveal") {
      asked.push(b);
      return r.fulfill({ json: { ok: true } });
    }
    return r.continue();
  });
  const row = panel.locator("li").filter({ hasText: /\.lsmap/ }).first();
  await row.getByRole("button", { name: "Show" }).click();
  await expect.poll(() => asked.map((a) => a.action)).toContain("reveal");
  await row.getByRole("button", { name: "Open" }).click();
  await expect.poll(() => asked.map((a) => a.action)).toContain("open");
  // Rename and Delete on a compiled file of the game copy (ls_newmap.log)
  const log = panel.locator("li").filter({ hasText: /\.log/ }).first();
  if (await log.count()) {
    page.once("dialog", (d) => void d.accept("ls_newmap_cov.log"));
    await log.getByRole("button", { name: "Rename" }).click();
    const name = page.getByRole("textbox", { name: "New name" });
    if (await name.waitFor({ timeout: 2000 }).then(() => true, () => false)) {
      await name.fill("cov_renamed.log");
      await page.getByRole("button", { name: /^Rename$/ }).last().click();
    }
    await expect(panel.locator("li").filter({ hasText: "cov_renamed.log" })).toBeVisible();
    const renamed = panel.locator("li").filter({ hasText: "cov_renamed.log" });
    await renamed.getByRole("button", { name: "Delete" }).click();
    const ok = page.getByRole("button", { name: /^Delete/ }).last();
    if (await ok.isVisible()) await ok.click();
    await expect(panel.locator("li").filter({ hasText: "cov_renamed.log" })).toHaveCount(0);
  }
});

// keep the imported text reader honest: what Export wrote reads back the same
test("an exported .map imports back with the same counts", async ({ page }) => {
  needsMapPick();
  await openEditor(page);
  await testMap(page);
  const n = await counts(page);
  clearSaved();
  await menu(page, "File", "Export a .map…");
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  const p = fixture("cov-export.map", readFileSync(SAVE));
  pickFile(p);
  await menu(page, "File", "Import a .map…");
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await expect.poll(() => counts(page)).toEqual(n);
});

test("Open a project: File menu, Mod+O and the start screen", async ({ page }) => {
  needsProjectPick();
  await openEditor(page);
  await testMap(page);
  const n = await counts(page);
  clearSaved();
  await menu(page, "File", "Save as…");
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  const project = join(FIX_DIR, "cov-open.lsmap");
  copyFileSync(SAVE, project);
  pickFile(project);
  await menu(page, "File", "New map");
  await menu(page, "File", "Open a project…");
  await expect.poll(() => counts(page)).toEqual(n);
  await menu(page, "File", "New map");
  await key(page, `${MOD}+KeyO`);
  await expect.poll(() => counts(page)).toEqual(n);
  await menu(page, "File", "New map");
  await toStart(page);
  await page.getByRole("button", { name: "Open a project" }).click();
  await expect(page.locator(".ed-menubar")).toBeVisible();
  await expect.poll(() => counts(page)).toEqual(n);
  // a dirty map asks first
  await cmd(page, { op: "add_box", min: [0, 0, 300], max: [16, 16, 316], texture: "CRATE01" });
  await menu(page, "File", "Open a project…");
  await expect(page.getByRole("button", { name: "Leave them" })).toBeVisible();
  await page.getByRole("button", { name: "Leave them" }).click();
  await expect.poll(() => counts(page)).toEqual(n);
});
