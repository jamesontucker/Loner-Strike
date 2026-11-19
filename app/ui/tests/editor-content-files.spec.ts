// Your own files in the Content Browser (Parity A): Mine's folders and New folder, a tile dragged
// onto a folder, Rename, Delete with the map's uses counted (Show uses), Undo bringing it back,
// and Copy to Mine for the game's files. The map's references follow a move or rename.
// The dev server saves projects to LS_DEV_PICK_SAVE, so run it with one:
//   LS_DEV_PICK_SAVE=<scratch>/qa_files/qa_files.lsmap npx playwright test editor-content-files
import { expect, test, type Page } from "@playwright/test";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";

const SAVE = process.env.LS_DEV_PICK_SAVE ?? "";
const DIR = SAVE ? dirname(SAVE) : "";
const MDL = Buffer.from("IDST a little model");

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );
const modelKey = async (page: Page) => {
  const doc = await api(page, "/api/tools/editor/doc");
  const e = (doc.entities as { classname: string; keys: [string, string][] }[]).find((x) => x.classname === "cycler_sprite");
  return e?.keys.find((k) => k[0] === "model")?.[1];
};

async function setUp(page: Page) {
  rmSync(DIR, { recursive: true, force: true });
  mkdirSync(DIR, { recursive: true });
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.editor.content.section", "model");
  });
  await page.goto("/editor-dev.html");
  // (the start screen, or a map left open by an earlier run: File › New map then)
  const start = page.getByRole("button", { name: "New map", exact: true });
  const bar = page.getByRole("menubar", { name: "Editor menu" });
  await expect(start.or(bar).first()).toBeVisible({ timeout: 30_000 });
  if (await start.isVisible()) await start.click();
  else {
    await bar.getByRole("menuitem", { name: "File", exact: true }).click();
    await page.getByRole("menuitem", { name: /^New map/ }).first().click();
  }
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const menu = page.getByRole("menubar", { name: "Editor menu" });
  await menu.getByRole("menuitem", { name: "File", exact: true }).click();
  await page.getByRole("menuitem", { name: /^Save( ⌘S| Ctrl\+S)?$/ }).first().click();
  await expect.poll(() => existsSync(SAVE), { timeout: 15_000 }).toBe(true);
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Content$/ }).first().click();
  const cb = page.locator('section[aria-label="Content browser"]').first();
  await cb.getByTestId("content-import-input").setInputFiles([{ name: "qa_box.mdl", mimeType: "application/octet-stream", buffer: MDL }]);
  await expect(cb.locator(".p5-cb-import")).toContainText("Imported models/qa_box.mdl");
  await cb.locator(".p5-cb-import").getByRole("button", { name: "Close" }).click();
  // the map uses it
  await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "cycler_sprite", origin: [0, 0, 64], keys: [["model", "models/qa_box.mdl"]] });
  await cb.getByRole("button", { name: /^Mine/ }).dispatchEvent("click");
  await expect(cb.locator(".p5-cb-tile", { hasText: "qa_box" })).toHaveCount(1);
  return cb;
}

/** A right-click on a tile (as an event: in a short dock panel the status bar can sit over it). */
const rightClick = async (tile: import("@playwright/test").Locator) => {
  await tile.scrollIntoViewIfNeeded();
  await tile.evaluate((el) => {
    const r = el.getBoundingClientRect();
    el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 10, clientY: r.top + 10, button: 2 }));
  });
};
/** An HTML5 drag of a tile onto a target (one DataTransfer from dragstart to drop, as a browser does). */
const dragTile = async (page: Page, tile: import("@playwright/test").Locator, target: import("@playwright/test").Locator) => {
  const dt = await page.evaluateHandle(() => new DataTransfer());
  await tile.dispatchEvent("dragstart", { dataTransfer: dt });
  await target.dispatchEvent("dragenter", { dataTransfer: dt });
  await target.dispatchEvent("dragover", { dataTransfer: dt });
  await target.dispatchEvent("drop", { dataTransfer: dt });
  await tile.dispatchEvent("dragend", { dataTransfer: dt });
};

const answer = async (page: Page, text: string, ok: string) => {
  const dlg = page.getByRole("alertdialog");
  await dlg.locator("input").fill(text);
  await dlg.getByRole("button", { name: ok }).click();
};

test("folders, drag onto a folder, rename: the map's reference follows", async ({ page }) => {
  test.skip(!SAVE, "needs LS_DEV_PICK_SAVE (see the top of this file)");
  const cb = await setUp(page);
  await cb.getByRole("button", { name: "+ New folder" }).dispatchEvent("click");
  await answer(page, "props", "Make it");
  const props = cb.getByRole("button", { name: "props/" });
  await expect(props).toBeVisible();
  expect(existsSync(join(DIR, "models/props"))).toBe(true);
  // back to all of Mine, then drag the tile onto the folder
  await cb.getByRole("button", { name: /^Mine/ }).dispatchEvent("click");
  await dragTile(page, cb.locator(".p5-cb-tile", { hasText: "qa_box" }), props);
  await expect(cb.locator(".p5-cb-import")).toContainText("Moved to models/props/qa_box.mdl.");
  await expect(cb.locator(".p5-cb-import")).toContainText("1 place in the map updated");
  expect(existsSync(join(DIR, "models/props/qa_box.mdl")) && !existsSync(join(DIR, "models/qa_box.mdl"))).toBe(true);
  expect(await modelKey(page)).toBe("models/props/qa_box.mdl");
  // rename it
  await cb.getByRole("button", { name: /^Mine/ }).dispatchEvent("click");
  await rightClick(cb.locator(".p5-cb-tile", { hasText: "qa_box" }));
  await expect(page.getByRole("menu"), "the tile's menu").toBeVisible({ timeout: 5000 });
  await page.getByRole("menuitem", { name: "Rename…" }).click({ timeout: 5000 });
  await expect(page.getByRole("alertdialog"), "the rename question").toBeVisible({ timeout: 5000 });
  await answer(page, "crate", "Rename");
  await expect(cb.locator(".p5-cb-import")).toContainText("Moved to models/props/crate.mdl.");
  expect(await modelKey(page)).toBe("models/props/crate.mdl");
  // Undo: the file and the reference back together
  await api(page, "/api/tools/editor/undo", {});
  expect(await modelKey(page)).toBe("models/props/qa_box.mdl");
  expect(existsSync(join(DIR, "models/props/qa_box.mdl")) && !existsSync(join(DIR, "models/props/crate.mdl"))).toBe(true);
});

test("delete: the map's uses counted first (Show uses), then into the deleted files; Undo brings it back", async ({ page }) => {
  test.skip(!SAVE, "needs LS_DEV_PICK_SAVE (see the top of this file)");
  const cb = await setUp(page);
  await rightClick(cb.locator(".p5-cb-tile", { hasText: "qa_box" }));
  await page.getByRole("menuitem", { name: "Delete…" }).click();
  const box = page.getByRole("alertdialog", { name: /^Delete qa_box/ });
  await expect(box).toContainText("The map still uses models/qa_box.mdl (1 place)");
  await box.getByRole("button", { name: "Show uses" }).click();
  await expect(page.getByRole("dialog", { name: /^Uses of qa_box/ })).toContainText("1 entity uses");
  await rightClick(cb.locator(".p5-cb-tile", { hasText: "qa_box" }));
  await page.getByRole("menuitem", { name: "Delete…" }).click();
  await page.getByRole("alertdialog", { name: /^Delete qa_box/ }).getByRole("button", { name: "Delete anyway" }).click();
  await expect(cb.locator(".p5-cb-import")).toContainText("Deleted models/qa_box.mdl. Undo brings it back.");
  expect(existsSync(join(DIR, "models/qa_box.mdl"))).toBe(false);
  await api(page, "/api/tools/editor/undo", {});
  expect(existsSync(join(DIR, "models/qa_box.mdl"))).toBe(true);
});

test("the game's own files: read-only, Copy to Mine makes a copy of yours", async ({ page }) => {
  test.skip(!SAVE, "needs LS_DEV_PICK_SAVE (see the top of this file)");
  const cb = await setUp(page);
  await cb.getByRole("button", { name: /^Game/ }).click();
  const tile = cb.locator(".p5-cb-tile").first();
  await tile.waitFor({ timeout: 30_000 });
  await rightClick(tile);
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem", { name: "Rename…" })).toHaveCount(0);
  await expect(menu.getByRole("menuitem", { name: "Delete…" })).toHaveCount(0);
  await menu.getByRole("menuitem", { name: "Copy to Mine" }).click();
  await expect(cb.locator(".p5-cb-import")).toContainText(/Imported models\/\S+_mine\.mdl|Imported models\//);
});
