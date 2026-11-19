// The Content Browser's Import and Export (Parity A): Import… and files dropped from Finder or
// Explorer go into the project's folder (checked by their real type, never over a file: a new
// name is offered), with a summary; right-click a tile for Show in Finder/Explorer and Export…,
// which shows whose work it is first.
// The dev server saves projects to LS_DEV_PICK_SAVE, so run it with one:
//   LS_DEV_PICK_SAVE=<scratch>/qa_import/qa_import.lsmap npx playwright test editor-content-import
import { expect, test, type Page } from "@playwright/test";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname } from "node:path";

const SAVE = process.env.LS_DEV_PICK_SAVE ?? "";
const MDL = Buffer.from("IDST a little model");
const WAV = Buffer.concat([Buffer.from("RIFF"), Buffer.alloc(4), Buffer.from("WAVEfmt qa")]);

async function savedProject(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.editor.content.section", "model");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  if (SAVE) {
    const menu = page.getByRole("menubar", { name: "Editor menu" });
    await menu.getByRole("menuitem", { name: "File", exact: true }).click();
    await page.getByRole("menuitem", { name: /^Save( ⌘S| Ctrl\+S)?$/ }).first().click();
    await expect.poll(() => existsSync(SAVE), { timeout: 15_000 }).toBe(true);
  }
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Content$/ }).first().click();
  return page.locator('section[aria-label="Content browser"]').first();
}

test.beforeEach(() => {
  if (!SAVE) return;
  rmSync(dirname(SAVE), { recursive: true, force: true });
  mkdirSync(dirname(SAVE), { recursive: true });
});

test("Import…: checked, into the project, a summary; a taken name gets a new one", async ({ page }) => {
  test.skip(!SAVE, "needs LS_DEV_PICK_SAVE (see the top of this file)");
  const cb = await savedProject(page);
  await cb.getByTestId("content-import-input").setInputFiles([
    { name: "qa_box.mdl", mimeType: "application/octet-stream", buffer: MDL },
    { name: "notreally.wav", mimeType: "audio/wav", buffer: Buffer.from("PK\x03\x04 a zip") },
    { name: "readme.txt", mimeType: "text/plain", buffer: Buffer.from("hi") },
  ]);
  const box = cb.locator(".p5-cb-import");
  await expect(box).toContainText("Imported models/qa_box.mdl");
  await expect(box).toContainText("Refused notreally.wav: it isn't really a .wav file");
  await expect(box).toContainText("Refused readme.txt");
  expect(existsSync(`${dirname(SAVE)}/models/qa_box.mdl`)).toBe(true);
  // it's under Mine
  const mine = cb.getByRole("button", { name: /^Mine/ });
  await expect(mine).toContainText("1", { timeout: 20_000 });
  // (the dock panel is short in this window: the tree may be scrolled)
  await mine.dispatchEvent("click");
  await expect(cb.locator(".p5-cb-tile", { hasText: "qa_box" })).toHaveCount(1);
  // again: the name is taken, a new one asked for, nothing replaced
  await cb.getByTestId("content-import-input").setInputFiles([{ name: "qa_box.mdl", mimeType: "application/octet-stream", buffer: Buffer.from("IDST another") }]);
  await page.getByRole("button", { name: "Import under new names" }).click();
  await expect(box).toContainText("models/qa_box.mdl was taken: saved as models/qa_box_2.mdl");
  expect(existsSync(`${dirname(SAVE)}/models/qa_box_2.mdl`)).toBe(true);
});

test("files dropped from Finder or Explorer are imported", async ({ page }) => {
  test.skip(!SAVE, "needs LS_DEV_PICK_SAVE (see the top of this file)");
  const cb = await savedProject(page);
  await cb.evaluate(async (el, wav) => {
    const dt = new DataTransfer();
    dt.items.add(new File([new Uint8Array(wav)], "qa_hum.wav", { type: "audio/wav" }));
    el.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
    el.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, [...WAV]);
  await expect(cb.locator(".p5-cb-import")).toContainText("Imported sound/qa_hum.wav");
  expect(existsSync(`${dirname(SAVE)}/sound/qa_hum.wav`)).toBe(true);
});

test("with no project saved: asks to save it first, or to use Add your own", async ({ page }) => {
  await page.route(/\/api\/tools\/editor\/content\/import/, (r) =>
    r.fulfill({ json: { needProject: true, error: "Save the project first: imported files go in its folder. Or put them in the game with Tools › Add your own." } }),
  );
  await page.addInitScript(() => localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true })));
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Content$/ }).first().click();
  const cb = page.locator('section[aria-label="Content browser"]').first();
  await cb.getByTestId("content-import-input").setInputFiles([{ name: "qa_box.mdl", mimeType: "application/octet-stream", buffer: MDL }]);
  const box = cb.locator(".p5-cb-import");
  await expect(box).toContainText("Save the project first");
  await expect(box.getByRole("button", { name: "Save the project…" })).toBeVisible();
  await expect(box.getByRole("button", { name: "Tools › Add your own" })).toBeVisible();
});

test("right-click a tile: Show in Finder/Explorer, and Export… with whose work it is", async ({ page }) => {
  const exported: unknown[] = [];
  await page.route(/\/api\/tools\/editor\/content\/export/, (r) => {
    exported.push(r.request().postDataJSON());
    return r.fulfill({ json: { ok: true, files: ["qa.mdl", "qaT.mdl"], folder: "/qa/out" } });
  });
  await page.route(/\/api\/tools\/credits\/asset/, (r) =>
    r.fulfill({ json: { source: "gamebanana", verdict: "ask", authors: [{ name: "Mapper Joe" }], page: { url: "https://gamebanana.com/mods/1", name: "Joe's props" } } }),
  );
  await page.addInitScript(() => localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true })));
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Content$/ }).first().click();
  const cb = page.locator('section[aria-label="Content browser"]').first();
  const tile = cb.locator(".p5-cb-tile").first();
  await tile.waitFor({ timeout: 30_000 });
  await tile.click({ button: "right" });
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem", { name: /^Show in (Finder|Explorer)$/ })).toBeVisible();
  await menu.getByRole("menuitem", { name: "Export…" }).click();
  const dlg = page.getByRole("dialog", { name: /^Export / });
  await expect(dlg.getByTestId("export-credits")).toContainText("Made by: Mapper Joe");
  await expect(dlg.getByTestId("export-credits")).toContainText("Ask the author");
  await dlg.getByRole("button", { name: "Choose a folder and export" }).click();
  await expect.poll(() => exported.length).toBe(1);
  await expect(cb.locator(".p5-cb-import")).toContainText("Exported qa.mdl, qaT.mdl to /qa/out.");
});
