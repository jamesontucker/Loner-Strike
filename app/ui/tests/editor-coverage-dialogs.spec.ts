// Editor coverage sweep: the dialogs' own controls (Paste special, Find and replace, Transform,
// Go to, Grid and snapping, Replace with, Map properties, Entity report, Map information,
// Coordinates, Find a command, the keys sheet, Change keys) and Escape on each.
import { expect, test, type Page } from "@playwright/test";
import { boundsOf, cmd, counts, entity, fixture, key, menu, MOD, openEditor, PICK, pickFile, select, sel, st, testMap } from "./editor-coverage.helpers";

const dialog = (p: Page, name: string | RegExp) => p.getByRole("dialog", { name });
const ls = (p: Page, k: string) => p.evaluate((k) => localStorage.getItem(k), k);

test("Escape closes every dialog (focus inside it) and leaves the selection alone", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  for (const [mn, item, name] of [
    ["Edit", "Paste special…", "Paste special"], ["Edit", "Find and replace…", "Find and replace"], ["Edit", "Transform…", "Transform"],
    ["Edit", "Replace with entity or prefab…", "Replace with"], ["Select", "Go to brush or entity number…", "Go to a number"], ["View", "Grid and snapping…", "Grid and snapping"],
    ["Map", "Map properties…", "Map properties"], ["Map", "Entity report…", "Entity report"], ["Map", "Map information…", "Map information"],
    ["Help", "Find a command…", "Find a command"], ["Help", "Keys cheat sheet", "Keys"], ["Help", "Coordinates and axes", "Coordinates and axes"], ["Help", "Change keys…", "Change keys"],
  ] as const) {
    await select(page, { brushes: [m.a, m.b] });
    await menu(page, mn, item);
    await expect(dialog(page, name)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog(page, name), name).toHaveCount(0);
    expect((await sel(page)).brushes, `${name}: the selection stays`).toEqual([m.a, m.b].sort((x, y) => x - y));
  }
});

test("Change keys: after removing a key, Escape still closes the sheet", async ({ page }) => {
  // the × that had focus is gone, so focus is on the page and Esc reaches the editor (F-1)
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  await menu(page, "Help", "Change keys…");
  const d = dialog(page, "Change keys");
  await d.getByRole("searchbox", { name: "Find a command or key" }).fill("Bigger grid");
  await d.getByRole("button", { name: /^Remove / }).first().click();
  await page.keyboard.press("Escape");
  await expect(d).toHaveCount(0);
  expect((await sel(page)).brushes).toEqual([m.a]);
});

test("Transform: after Apply, Escape closes the dialog and keeps the selection", async ({ page }) => {
  // Apply greys itself out while it works, so focus leaves the dialog; Esc then reaches the
  // editor (which drops the selection) instead of the dialog (EditorCoverage.md F-1)
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  await menu(page, "Edit", "Transform…");
  const d = dialog(page, "Transform");
  await d.getByRole("spinbutton").first().fill("32");
  await d.getByRole("button", { name: "Apply" }).click();
  await expect.poll(async () => (await boundsOf(page, m.a))![0][0]).toBe(32);
  await page.keyboard.press("Escape");
  await expect(d).toHaveCount(0);
  expect((await sel(page)).brushes).toEqual([m.a]);
});

test("Transform: Turn, Stretch and Line up (with Spread out)", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  const c = (await cmd(page, { op: "add_box", min: [300, 0, 0], max: [332, 64, 16], texture: m.tex }))!.brushes[0];
  await select(page, { brushes: [c] });
  await menu(page, "Edit", "Transform…");
  const d = dialog(page, "Transform");
  await d.getByRole("group", { name: "Kind" }).getByRole("button", { name: "Turn" }).click();
  await d.getByRole("group", { name: "Axis" }).getByRole("button", { name: "Z" }).click();
  await d.getByLabel(/Degrees/).fill("90");
  await d.getByRole("button", { name: "Apply" }).click();
  await expect.poll(async () => { const b = (await boundsOf(page, c))!; return [b[1][0] - b[0][0], b[1][1] - b[0][1]]; }).toEqual([64, 32]);
  await d.getByRole("group", { name: "Kind" }).getByRole("button", { name: "Stretch" }).click();
  await d.getByRole("spinbutton").first().fill("200");
  await d.getByRole("button", { name: "Apply" }).click();
  await expect.poll(async () => { const b = (await boundsOf(page, c))!; return b[1][0] - b[0][0]; }).toBe(128);
  await d.getByRole("button", { name: "Close" }).click();
  // Line up and spread out (Edit > Line up and spread out… opens the same dialog on it)
  await select(page, { brushes: [m.a, m.b, c] });
  await menu(page, "Edit", "Line up and spread out…");
  await d.getByRole("group", { name: "Kind" }).getByRole("button", { name: "Line up" }).click();
  await d.getByRole("button", { name: "Bottoms" }).click();
  await expect.poll(async () => (await boundsOf(page, c))![0][2]).toBe(0);
  await d.getByRole("button", { name: "Spread out" }).first().click();
  await expect.poll(() => st(page, (s) => s.doc.undo)).toMatch(/spread|distribute|even/i);
  await d.getByRole("button", { name: "Close" }).click();
});

test("Paste special: Row, Stairs, Ring, Spiral fill the form; Cancel makes nothing", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  const n = (await counts(page)).brushes;
  await menu(page, "Edit", "Paste special…");
  const d = dialog(page, "Paste special");
  await d.getByRole("button", { name: "Stairs" }).click();
  await expect(d.getByRole("spinbutton", { name: "Moved by Z" })).not.toHaveValue("0");
  await d.getByRole("button", { name: "Ring" }).click();
  await expect(d.getByRole("spinbutton", { name: "Turned by (°) Z" })).not.toHaveValue("0");
  await d.getByRole("button", { name: "Spiral" }).click();
  await d.getByRole("spinbutton", { name: "Copies" }).fill("3");
  await expect(d.getByRole("button", { name: /Make 3 copies/ })).toBeVisible();
  await d.getByRole("button", { name: "Cancel" }).click();
  await expect(d).toHaveCount(0);
  expect((await counts(page)).brushes).toBe(n);
});

test("Find and replace: entity types, values, key names; Pick them; case and whole-name options", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Edit", "Find and replace…");
  const d = dialog(page, "Find and replace");
  // Pick them (textures)
  await d.getByRole("textbox", { name: "Find" }).fill(m.tex.toLowerCase());
  await d.getByRole("checkbox", { name: "Match upper/lower case" }).check();
  await d.getByRole("button", { name: "Find", exact: true }).click();
  await expect(d.getByText("Nothing matches.")).toBeVisible();
  await d.getByRole("checkbox", { name: "Match upper/lower case" }).uncheck();
  await d.getByRole("checkbox", { name: "Whole name only" }).uncheck();
  await d.getByRole("textbox", { name: "Find" }).fill("WALL");
  await d.getByRole("button", { name: "Find", exact: true }).click();
  await d.getByRole("button", { name: "Pick them" }).click();
  await expect.poll(async () => (await sel(page)).brushes.length).toBeGreaterThan(2);
  if (await d.isVisible()) await d.getByRole("button", { name: "Close" }).click();
  // values: lamp1 -> lamp2 renames the light and the trigger's target
  await menu(page, "Edit", "Find and replace…");
  await d.getByRole("group", { name: "What to look in" }).getByRole("button", { name: "Values" }).click();
  await d.getByRole("textbox", { name: "Find" }).fill("lamp1");
  await d.getByRole("textbox", { name: /Replace with/ }).fill("lamp2");
  await d.getByRole("button", { name: "Find", exact: true }).click();
  await d.getByRole("button", { name: /^Replace (all|\d+)$/ }).click();
  await expect.poll(async () => (await entity(page, m.trig))!.keys.target).toBe("lamp2");
  expect((await entity(page, m.light))!.keys.targetname).toBe("lamp2");
  // entity types
  await d.getByRole("group", { name: "What to look in" }).getByRole("button", { name: "Entity types" }).click();
  await d.getByRole("textbox", { name: "Find" }).fill("light");
  await d.getByRole("textbox", { name: /Replace with/ }).fill("light_spot");
  await d.getByRole("button", { name: "Find", exact: true }).click();
  await d.getByRole("button", { name: /^Replace (all|\d+)$/ }).click();
  await expect.poll(async () => (await entity(page, m.light))!.classname).toBe("light_spot");
  // key names
  await d.getByRole("group", { name: "What to look in" }).getByRole("button", { name: "Key names" }).click();
  await d.getByRole("textbox", { name: "Find" }).fill("targetname");
  await d.getByRole("button", { name: "Find", exact: true }).click();
  await expect(d.getByText(/Found in/)).toBeVisible();
  await d.getByRole("button", { name: "Close" }).click();
});

test("Go to: compiler numbers (Entity 0 is the world)", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "Select", "Go to brush or entity number…");
  const d = dialog(page, "Go to a number");
  await d.getByRole("button", { name: "Compiler numbers" }).click();
  await d.getByRole("textbox").fill("Entity 0, Brush 2");
  await d.getByRole("button", { name: "Go" }).click();
  await expect.poll(async () => (await sel(page)).brushes.length).toBe(1);
});

test("Grid and snapping: every setting", async ({ page }) => {
  await openEditor(page);
  await menu(page, "View", "Grid and snapping…");
  const d = dialog(page, "Grid and snapping");
  await d.getByRole("checkbox", { name: "Show the grid" }).uncheck();
  await expect.poll(() => ls(page, "ls.editor.showGrid")).toBe("0");
  await d.getByRole("combobox", { name: /Turning snaps to/ }).selectOption({ index: 0 });
  await expect.poll(() => ls(page, "ls.editor.rotSnap")).not.toBeNull();
  await d.getByRole("checkbox", { name: /Brighter lines/ }).uncheck();
  await expect.poll(() => ls(page, "ls.editor.gridMajor")).toBe("0");
  await d.getByRole("checkbox", { name: "Snap to the grid" }).uncheck();
  await expect.poll(() => st(page, (s) => s.snapOff)).toBe(true);
  await d.getByRole("checkbox", { name: "Snap to brush corners and edges" }).check();
  await expect.poll(() => ls(page, "ls.editor.geoSnap")).toBe("1");
  const lock = d.getByRole("checkbox", { name: /Texture lock/ });
  if (await lock.count()) {
    await lock.uncheck();
    await expect.poll(() => st(page, (s) => s.textureLock)).toBe(false);
  }
  await d.getByRole("button", { name: "Done" }).click();
});

test("Replace with: a prefab", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  // a prefab of the light
  const r = await page.evaluate(async (e) => (await fetch("/api/tools/editor/prefabs/save", { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify({ name: "cov lamp", tags: [], author: "qa", description: "", brushes: [], entities: [e] }) })).json(), m.light);
  expect(r.error ?? null).toBeNull();
  await select(page, { entities: [m.spawn] });
  await menu(page, "Edit", "Replace with entity or prefab…");
  const d = dialog(page, "Replace with");
  await d.getByRole("button", { name: "A prefab" }).click();
  await d.getByRole("button", { name: /cov lamp/ }).first().click();
  await d.getByRole("button", { name: "Replace", exact: true }).click();
  await expect.poll(async () => (await entity(page, m.spawn)) === null).toBe(true);
});

test("Map properties: world, sun, rules, WADs, entity definitions; one undo step per change", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Map properties…");
  const d = dialog(page, "Map properties");
  await d.getByRole("textbox", { name: /^Title/ }).fill("Coverage");
  await d.getByRole("combobox", { name: "Sky" }).fill("night");
  await d.getByRole("spinbutton", { name: /Draw distance/ }).fill("4096");
  await d.getByRole("spinbutton", { name: /Base light/ }).fill("20");
  await d.getByRole("textbox", { name: "Color" }).fill("#ff8800");
  await d.getByRole("spinbutton", { name: "Brightness" }).fill("180");
  await d.getByRole("spinbutton", { name: /Height \(pitch\)/ }).fill("-45");
  await d.getByRole("spinbutton", { name: /From \(angle\)/ }).fill("90");
  await d.getByRole("combobox", { name: "Buying" }).selectOption({ index: 1 });
  await d.getByRole("spinbutton", { name: /Bomb blast radius/ }).fill("400");
  await d.getByRole("button", { name: "Remove cstrike.wad" }).click();
  await d.getByRole("button", { name: "Save properties" }).click();
  await expect(d).toHaveCount(0);
  const world = await page.evaluate(async () => (await (await fetch("/api/tools/editor/doc")).json()).world);
  const keys = Object.fromEntries(world.keys);
  expect(keys.message).toBe("Coverage");
  expect(keys.skyname).toBe("night");
  expect(world.wads.join(" ")).not.toContain("cstrike.wad");
  // the sun is the light_environment's
  const sun = await st(page, (s) => Object.fromEntries(s.doc.entities.find((e: { classname: string }) => e.classname === "light_environment")?.keys ?? []));
  expect(sun._light ?? "").toMatch(/^255 136 0/);
  // Add a WAD back
  await menu(page, "Map", "Map properties…");
  const add = d.getByRole("combobox", { name: "Add a WAD" });
  const opt = (await add.locator("option").allTextContents()).find((o) => /cstrike\.wad/i.test(o));
  await add.selectOption({ label: opt! });
  await d.getByRole("button", { name: "Add", exact: true }).click();
  await d.getByRole("button", { name: "Save properties" }).click();
  await expect.poll(async () => (await page.evaluate(async () => (await (await fetch("/api/tools/editor/doc")).json()).world.wads)).join(" ")).toContain("cstrike.wad");
});

test.describe("entity definitions", () => {
  test.skip(!PICK, "needs LS_COV_PICK");
  test("Add your own FGD… then Use ours only", async ({ page }) => {
    await openEditor(page);
    pickFile(fixture("cov.fgd", '@PointClass size(-8 -8 -8, 8 8 8) = cov_marker : "A coverage marker" [ note(string) : "Note" ]\n'));
    await menu(page, "Map", "Map properties…");
    const d = dialog(page, "Map properties");
    await d.getByRole("button", { name: "Add your own FGD…" }).click();
    // (read by content; the dev server keeps the picked path, its symlink)
    await expect(d.getByRole("status")).toContainText("Read 1 entity class");
    await expect(d.getByRole("button", { name: "Choose another FGD…" })).toBeVisible();
    await d.getByRole("button", { name: "Use ours only" }).click();
    await expect(d.getByRole("status")).toContainText("Back to ours only.");
  });
});

test("Entity report: find, class filter, tick all, Pick, Delete, double-click goes there", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Map", "Entity report…");
  const d = dialog(page, "Entity report");
  await d.getByRole("combobox", { name: "Class" }).selectOption({ label: /info_player_start/ } as never).catch(async () => {
    const opts = await d.getByRole("combobox", { name: "Class" }).locator("option").allTextContents();
    await d.getByRole("combobox", { name: "Class" }).selectOption({ label: opts.find((o) => o.startsWith("info_player_start"))! });
  });
  await d.getByRole("checkbox", { name: "Tick all shown" }).check();
  await d.getByRole("button", { name: /^Pick( \d+)?$/ }).click();
  await expect.poll(async () => (await sel(page)).entities.length).toBe(3);
  // (Pick closes it)
  await menu(page, "Map", "Entity report…");
  await d.getByRole("searchbox", { name: "Find" }).fill("lamp1");
  await d.getByRole("row").nth(1).dblclick();
  await expect.poll(async () => (await sel(page)).entities).toEqual([m.light]);
  await menu(page, "Map", "Entity report…");
  await d.getByRole("searchbox", { name: "Find" }).fill("lamp1");
  await d.getByRole("checkbox", { name: "Pick light" }).check();
  await d.getByRole("button", { name: /^Delete/ }).click();
  await expect.poll(async () => (await entity(page, m.light)) === null).toBe(true);
});

test("Find a command: arrows and Enter; greyed-out commands don't run; Mod+K and Mod+Shift+P open it", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await key(page, `${MOD}+KeyK`);
  const d = dialog(page, "Find a command");
  await d.getByRole("searchbox").fill("grid");
  await page.keyboard.press("ArrowDown");
  await expect(d.getByRole("option", { selected: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await key(page, `${MOD}+Shift+KeyP`);
  await d.getByRole("searchbox").fill("Ungroup");
  await expect(d.getByRole("option", { name: /^Ungroup/ })).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Enter");
  // nothing grouped: Ungroup can't run, the palette says so or stays
  expect((await counts(page)).groups).toBe(0);
});

test("Keys sheet: Change keys… and Print", async ({ page }) => {
  await openEditor(page);
  await page.evaluate(() => {
    (window as unknown as { __printed: number }).__printed = 0;
    window.print = () => void ((window as unknown as { __printed: number }).__printed += 1);
  });
  await menu(page, "Help", "Keys cheat sheet");
  const d = dialog(page, "Keys");
  await d.getByRole("button", { name: "Print" }).click();
  expect(await page.evaluate(() => (window as unknown as { __printed: number }).__printed)).toBe(1);
  await d.getByRole("button", { name: "Change keys…" }).click();
  await expect(dialog(page, "Change keys")).toBeVisible();
});

test("Change keys: your own key, remove one, the clash warning, reset, Put back", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "Help", "Change keys…");
  const d = dialog(page, "Change keys");
  await d.getByRole("searchbox", { name: "Find a command or key" }).fill("Bigger grid");
  await d.getByRole("button", { name: "Add key" }).first().click();
  await page.keyboard.press("KeyJ");
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.keys"))!).mine["view.gridUp"]).toContain("J");
  // J was Go to's? (no: Mod+J). A clash: give Smaller grid the same key
  await d.getByRole("searchbox", { name: "Find a command or key" }).fill("Smaller grid");
  await d.getByRole("button", { name: "Add key" }).first().click();
  await page.keyboard.press("KeyJ");
  await expect(d).toContainText(/also|same key|clash|used by/i);
  await d.getByRole("button", { name: "reset" }).first().click();
  await d.getByRole("searchbox", { name: "Find a command or key" }).fill("Bigger grid");
  await d.getByRole("button", { name: /^Remove / }).first().click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.keys"))!).mine["view.gridUp"]).not.toContain("]");
  // (Done, not Esc: the removed key's button had focus, so Esc goes past the sheet; F-1)
  await d.getByRole("button", { name: "Done" }).click();
  await expect(d).toHaveCount(0);
  // the own key works
  await key(page, "KeyJ");
  await expect.poll(() => st(page, (s) => s.grid)).toBe(32);
  await menu(page, "Help", "Change keys…");
  await d.getByRole("button", { name: /^Put back/ }).click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.keys"))!).mine).toEqual({});
});

test("Hammer-style keys also switch to the four views", async ({ page }) => {
  await openEditor(page);
  await menu(page, "Help", "Change keys…");
  await dialog(page, "Change keys").getByRole("button", { name: "Hammer-style" }).click();
  await page.keyboard.press("Escape");
  await expect.poll(() => page.locator(".ed-pane:visible").count()).toBe(4);
});
