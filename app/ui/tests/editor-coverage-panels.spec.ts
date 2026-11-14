// Editor coverage sweep: the panels and their controls (Inspect for a brush, an entity, two
// entities and nothing; Outline; Textures; Gameplay; Budget; I/O; Checks; History; Content;
// Prefabs; Project is in -file), the dock (tabs, ⋯ menu, float, own window, fold), the tool
// rail and each tool's options, the status bar, and the MCP selection route.
// Runs in Chromium and WebKit (editor-coverage.config.ts).
import { expect, test, type Page } from "@playwright/test";
import { boundsOf, cmd, counts, entity, menu, openEditor, select, sel, st, testMap, undo } from "./editor-coverage.helpers";

const panel = (p: Page, sel: string) => p.locator(sel).first();
const tab = (p: Page, title: string) => p.getByRole("tab", { name: title, exact: true }).first();
const ls = (p: Page, k: string) => p.evaluate((k) => localStorage.getItem(k), k);

test("Inspect, a brush: Transform fields, textures, wall thickness and every button", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  await menu(page, "Map", "Properties of what's picked");
  const tr = page.getByRole("group", { name: "Transform" });
  // Location X: type a position
  await tr.getByRole("textbox", { name: /^X \(east\)/ }).fill("64");
  await tr.getByRole("textbox", { name: /^X \(east\)/ }).press("Enter");
  await expect.poll(async () => (await boundsOf(page, m.a))![0][0]).toBe(32);
  // Turn by Z 90 (a cube stays the same size; the undo label says it turned)
  await tr.getByRole("textbox", { name: "Turn round Z, degrees" }).fill("90");
  await tr.getByRole("textbox", { name: "Turn round Z, degrees" }).press("Enter");
  await expect.poll(() => st(page, (s) => s.doc.undo)).toMatch(/turn|rotat/i);
  // Size along Z: stretch to 128
  await tr.getByRole("textbox", { name: /^Size along Z/ }).fill("128");
  await tr.getByRole("textbox", { name: /^Size along Z/ }).press("Enter");
  await expect.poll(async () => { const b = (await boundsOf(page, m.a))!; return b[1][2] - b[0][2]; }).toBe(128);
  // Scale X ×0.5
  await tr.getByRole("textbox", { name: "Stretch along X by this much" }).fill("0.5");
  await tr.getByRole("textbox", { name: "Stretch along X by this much" }).press("Enter");
  await expect.poll(async () => { const b = (await boundsOf(page, m.a))!; return b[1][0] - b[0][0]; }).toBe(32);
  // a texture in the list becomes the current one
  await page.getByRole("listitem", { name: new RegExp(`^${m.tex}: `) }).click();
  await expect.poll(() => st(page, (s) => s.texture)).toBe(m.tex);
  // wall thickness then Hollow
  await page.getByRole("spinbutton", { name: "Wall thickness" }).fill("4");
  await expect.poll(() => st(page, (s) => s.thickness)).toBe(4);
  const n = (await counts(page)).brushes;
  await page.getByRole("button", { name: "Hollow", exact: true }).click();
  await expect.poll(async () => (await counts(page)).brushes).toBe(n + 5);
  await undo(page);
  await select(page, { brushes: [m.a] });
  for (const [name, check] of [
    ["Snap to grid", async () => expect.poll(() => st(page, (s) => s.doc.undo)).toMatch(/snap/i)],
    ["Duplicate", async () => expect.poll(async () => (await counts(page)).brushes).toBe(n + 1)],
  ] as const) {
    await select(page, { brushes: [m.a] });
    await page.getByRole("button", { name, exact: true }).first().click();
    await check();
  }
  await select(page, { brushes: [m.a] });
  // Flip X / Y / Z and Turn 90° buttons
  for (const ax of ["X", "Y", "Z"]) {
    const before = await st(page, (s) => s.doc.revision);
    await page.locator(".tools-panel").getByRole("button", { name: ax, exact: true }).first().click();
    await expect.poll(() => st(page, (s) => s.doc.revision)).not.toBe(before);
  }
  // Paste special… opens its dialog
  await page.getByRole("button", { name: "Paste special…" }).first().click();
  await expect(page.getByRole("dialog", { name: "Paste special" })).toBeVisible();
  await page.getByRole("dialog", { name: "Paste special" }).getByRole("button", { name: "Cancel" }).click();
  // Tie to entity with a type picked
  await select(page, { brushes: [m.b] });
  await page.getByRole("combobox", { name: "Entity type" }).selectOption({ index: 1 });
  const e0 = (await counts(page)).entities;
  await page.getByRole("button", { name: "Tie to entity" }).click();
  await expect.poll(async () => (await counts(page)).entities).toBe(e0 + 1);
  await undo(page);
  // Group, Look at, Hide, Isolate, Delete
  await select(page, { brushes: [m.a, m.b] });
  await page.getByRole("button", { name: "Group", exact: true }).first().click();
  await expect.poll(async () => (await counts(page)).groups).toBe(1);
  await undo(page);
  await select(page, { brushes: [m.b] });
  await page.getByRole("button", { name: "Look at" }).first().click();
  await page.getByRole("button", { name: "Isolate" }).first().click();
  await expect.poll(() => st(page, (s) => s.hidden.brushes.size)).toBeGreaterThan(3);
  await page.evaluate(() => (window as any).__covRun("sel.showAll")); // eslint-disable-line @typescript-eslint/no-explicit-any
  await select(page, { brushes: [m.b] });
  const inspect = page.locator(".tools-panel").filter({ has: page.getByRole("heading", { name: "1 brush" }) }).first();
  await inspect.getByRole("button", { name: "Hide", exact: true }).click();
  await expect.poll(() => st(page, (s) => s.hidden.brushes.size)).toBe(1);
  await page.evaluate(() => (window as any).__covRun("sel.showAll")); // eslint-disable-line @typescript-eslint/no-explicit-any
  await select(page, { brushes: [m.b] });
  const b0 = (await counts(page)).brushes;
  await page.locator(".tools-panel").getByRole("button", { name: "Delete", exact: true }).first().click();
  await expect.poll(async () => (await counts(page)).brushes).toBe(b0 - 1);
  // Merge and Carve with two picked
  await undo(page);
  await select(page, { brushes: [m.a, m.b] });
  await expect(page.getByRole("button", { name: "Merge", exact: true })).toBeEnabled();
});

test("Inspect, an entity: name, colour, brightness, flicker, falloff, flags, raw keys, new key, reset, links, Look at, Duplicate, Delete", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { entities: [m.light] });
  await menu(page, "Map", "Properties of what's picked");
  const name = page.getByRole("textbox", { name: /^Name/ }).first();
  await name.fill("lamp9");
  await name.press("Enter");
  await expect.poll(async () => (await entity(page, m.light))!.keys.targetname).toBe("lamp9");
  // (a number field: it saves when it loses focus; Enter alone doesn't)
  await page.getByRole("spinbutton", { name: "Brightness" }).first().fill("150");
  await page.getByRole("spinbutton", { name: "Brightness" }).first().blur();
  await expect.poll(async () => (await entity(page, m.light))!.keys._light ?? "").toMatch(/ 150$/);
  await page.getByRole("combobox", { name: /^Flicker style/ }).selectOption({ index: 2 });
  await expect.poll(async () => (await entity(page, m.light))!.keys.style ?? "").not.toBe("");
  await page.getByRole("combobox", { name: "Falloff" }).selectOption({ index: 1 });
  await expect.poll(async () => (await entity(page, m.light))!.keys.delay ?? (await entity(page, m.light))!.keys._fade ?? "x").not.toBe("");
  // (it saves through the app, then shows checked)
  await page.getByRole("checkbox", { name: /Starts dark/ }).click();
  await expect.poll(async () => (await entity(page, m.light))!.keys.spawnflags).toBe("1");
  await page.getByRole("button", { name: "Show the raw keys" }).click();
  await page.getByRole("combobox", { name: "New key" }).first().fill("cov_key");
  await page.getByRole("textbox", { name: "New value" }).first().fill("42");
  await page.getByRole("button", { name: "Add", exact: true }).first().click();
  await expect.poll(async () => (await entity(page, m.light))!.keys.cov_key).toBe("42");
  await page.getByRole("button", { name: "Reset Name" }).click();
  await expect.poll(async () => (await entity(page, m.light))!.keys.targetname ?? "").toBe("");
  // the trigger: Sets off its target, Set off by goes back
  await select(page, { entities: [m.trig] });
  await cmd(page, { op: "set_key", entity: m.light, key: "targetname", value: "lamp1" });
  await page.getByRole("button", { name: /^Sets off light/ }).click();
  await expect.poll(async () => (await sel(page)).entities).toEqual([m.light]);
  await page.getByRole("button", { name: /^Set off by trigger_multiple/ }).click();
  await expect.poll(async () => (await sel(page)).entities).toEqual([m.trig]);
  // Move to world (the trigger), then Look at / Duplicate / Delete for the light
  await page.getByRole("button", { name: "Move to world" }).click();
  await expect.poll(async () => (await entity(page, m.trig)) === null).toBe(true);
  await undo(page);
  await select(page, { entities: [m.light] });
  await page.getByRole("button", { name: "Look at" }).first().click();
  const e0 = (await counts(page)).entities;
  await page.locator(".tools-panel").getByRole("button", { name: "Duplicate", exact: true }).first().click();
  await expect.poll(async () => (await counts(page)).entities).toBe(e0 + 1);
  await page.locator(".tools-panel").getByRole("button", { name: "Delete", exact: true }).first().click();
  await expect.poll(async () => (await counts(page)).entities).toBe(e0);
});

test("Inspect, two entities: a key added to all, and removed from all", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { entities: [m.light, m.spawn] });
  await menu(page, "Map", "Properties of what's picked");
  await page.getByRole("combobox", { name: "New key" }).or(page.getByRole("textbox", { name: "New key" })).first().fill("cov_all");
  await page.getByRole("textbox", { name: "Value" }).first().fill("1");
  await page.getByRole("button", { name: "Add to all" }).click();
  await expect.poll(async () => [(await entity(page, m.light))!.keys.cov_all, (await entity(page, m.spawn))!.keys.cov_all]).toEqual(["1", "1"]);
  await page.getByRole("button", { name: "Remove cov_all from all" }).click();
  await expect.poll(async () => (await entity(page, m.light))!.keys.cov_all ?? null).toBeNull();
});

test("Inspect, nothing picked: map settings (sky, WADs), Map properties… and Entity report…", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Properties of what's picked");
  const sky = page.locator(".tools-panel").getByRole("textbox").filter({ hasText: "" }).first();
  await page.getByText("Map settings").waitFor();
  const skyField = page.locator(".tools-panel input[placeholder=desert]").first();
  await skyField.fill("night");
  await skyField.blur();
  await expect.poll(() => page.evaluate(async () => Object.fromEntries((await (await fetch("/api/tools/editor/doc")).json()).world.keys).skyname)).toBe("night");
  void sky;
  await page.locator(".tools-panel").getByRole("button", { name: "Remove cstrike.wad" }).click();
  await expect.poll(() => page.evaluate(async () => (await (await fetch("/api/tools/editor/doc")).json()).world.wads.join(" "))).not.toContain("cstrike");
  await page.locator(".tools-panel").getByRole("button", { name: "Map properties…" }).click();
  await expect(page.getByRole("dialog", { name: "Map properties" })).toBeVisible();
  await page.getByRole("dialog", { name: "Map properties" }).getByRole("button", { name: /Close|Cancel/ }).first().click();
  await page.locator(".tools-panel").getByRole("button", { name: "Entity report…" }).click();
  await expect(page.getByRole("dialog", { name: "Entity report" })).toBeVisible();
});

test("Outline: find, show only, sort, hide and lock per row, unfold, double-click, right-click menu", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Map", "Outline");
  const ol = panel(page, ".editor-outline");
  await ol.getByRole("searchbox").first().fill("lamp1");
  // (the light, and the trigger that targets it)
  await expect(ol.getByRole("treeitem")).toHaveCount(2);
  await ol.getByRole("searchbox").first().fill("");
  await ol.getByRole("combobox", { name: "Show only" }).selectOption({ index: 1 });
  await ol.getByRole("combobox", { name: "Show only" }).selectOption({ index: 0 });
  for (const s of ["Name", "Type", "Texture"]) await ol.getByRole("button", { name: s, exact: true }).click();
  const row = ol.getByRole("treeitem", { name: /lamp1/ });
  await row.click();
  await expect.poll(async () => (await sel(page)).entities).toEqual([m.light]);
  await row.dblclick();
  await row.getByRole("button", { name: /Hide/ }).click();
  await expect.poll(() => st(page, (s) => s.hidden.entities.size)).toBe(1);
  await row.getByRole("button", { name: /Show|Hide/ }).click();
  await row.getByRole("button", { name: /Lock/ }).click();
  await expect.poll(() => page.evaluate(async () => { const p = await import(/* @vite-ignore */ "/src/views/tools/editor/p5store.ts"); return p.useP5.getState().locked.entities.size; })).toBe(1);
  await row.getByRole("button", { name: /Lock|Unlock/ }).click();
  // right-click: the menu (Rename, Duplicate, a new folder…)
  await row.click({ button: "right" });
  const ctx = page.getByRole("menu").filter({ has: page.getByRole("menuitem", { name: /^Rename/ }) });
  await expect(ctx).toBeVisible();
  const e0 = (await counts(page)).entities;
  await ctx.getByRole("menuitem", { name: "Duplicate" }).click();
  await expect.poll(async () => (await counts(page)).entities).toBe(e0 + 1);
  await select(page, { entities: [m.light] });
  await row.click({ button: "right" });
  await ctx.getByRole("menuitem", { name: "New folder from what's picked" }).click();
  await expect.poll(async () => (await counts(page)).groups).toBe(1);
  // the folder unfolds
  const folder = ol.getByRole("treeitem").filter({ hasText: /Group|Folder/ }).first();
  await folder.click({ button: "right" });
  await page.getByRole("menuitem", { name: /^Ungroup/ }).click();
  await expect.poll(async () => (await counts(page)).groups).toBe(0);
});

test("Textures panel: find, one WAD, click chooses, double-click paints what's picked", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Map", "Textures");
  const tx = panel(page, ".editor-textures");
  await tx.getByRole("searchbox", { name: "Find a texture" }).fill("crate01");
  // halflife.wad when the list names it so, whatever else the label says ("halflife.wad (3116)"
  // on a full game), else the first WAD after "All"; never waiting on a label that isn't there
  const which = tx.getByRole("combobox", { name: "Which WAD" });
  await expect(which.locator("option").nth(1)).toBeAttached({ timeout: 20_000 });
  const labels = await which.locator("option").allTextContents();
  const hl = labels.findIndex((l) => /^halflife\.wad\b/i.test(l.trim()));
  await which.selectOption({ index: hl > 0 ? hl : 1 }, { timeout: 5_000 });
  const t = tx.getByRole("button", { name: /^crate01$/i }).first();
  await t.click();
  await expect.poll(() => st(page, (s) => s.texture.toUpperCase())).toBe("CRATE01");
  await select(page, { brushes: [m.a] });
  await t.dblclick();
  await expect.poll(() => page.evaluate((id) => [...(window as any).__lsEditor.getState().brushes.get(id).textures].map((t: string) => t.toUpperCase()), m.a)).toEqual(["CRATE01"]); // eslint-disable-line @typescript-eslint/no-explicit-any
});

test("History: click a step goes back to it; an undone step redoes", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "Edit", "Undo history");
  await page.getByRole("button", { name: "Add light" }).click();
  await expect.poll(async () => (await entity(page, m.spawn)) === null).toBe(true);
  await page.getByRole("button", { name: "Set target" }).click();
  await expect.poll(async () => (await entity(page, m.trig))?.keys.target).toBe("lamp1");
  await page.getByRole("button", { name: "Opened the map" }).click();
  await expect.poll(async () => (await counts(page)).brushes).toBeLessThan(10);
});

test("Checks: filters, Show in 3D, settings, Pick, Look and a fix with undo", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "Map", "Check the map");
  const ck = page.getByRole("region", { name: "Checks" });
  await expect(ck.getByRole("button", { name: /^Notes \d+/ })).toBeVisible({ timeout: 15_000 });
  for (const f of ["Problems", "Warnings", "Notes", "All"]) await ck.getByRole("group", { name: "Show" }).getByRole("button", { name: new RegExp(`^${f}`) }).click();
  await ck.getByRole("checkbox", { name: "Show in 3D" }).uncheck();
  await expect.poll(() => ls(page, "ls.editor.checkMarks")).toBe("0");
  await ck.getByRole("checkbox", { name: "Show in 3D" }).check();
  await ck.getByRole("button", { name: "Settings" }).click();
  await ck.getByRole("spinbutton", { name: "Largest texture scale" }).fill("8");
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.checkLimits")) ?? "{}").maxScale).toBe(8);
  await ck.getByRole("button", { name: "Pick" }).first().click();
  await expect.poll(async () => (await sel(page)).brushes.length).toBeGreaterThan(0);
  await ck.getByRole("button", { name: "Look" }).first().click();
  const fix = ck.getByRole("button", { name: /NULL them all|^Fix/ }).first();
  const rev = await st(page, (s) => s.doc.revision);
  await fix.click();
  await expect.poll(() => st(page, (s) => s.doc.revision)).not.toBe(rev);
  await undo(page);
});

test("Budget: Refresh, Pick from a precache row", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "View", "Budget");
  const b = page.getByRole("region", { name: "Budget" });
  await b.getByRole("button", { name: "Refresh" }).click();
  await expect(b.getByRole("meter").first()).toBeVisible();
  await b.getByRole("button", { name: /^Entities / }).click();
  const pick = b.getByRole("button", { name: "Pick" }).first();
  if (await pick.count()) {
    await pick.click();
    await expect.poll(async () => { const s = await sel(page); return s.brushes.length + s.entities.length; }).toBeGreaterThan(0);
  }
});

test("I/O: All, Broken, Picked chain; find; pick; look; a broken link's Remove", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await cmd(page, { op: "set_key", entity: m.spawn, key: "target", value: "nothing_here" });
  await menu(page, "View", "I/O: what triggers what");
  const io = page.getByRole("region", { name: "What triggers what" });
  await io.getByRole("button", { name: "Broken" }).click();
  await expect(io.getByText(/nothing_here/)).toBeVisible();
  await io.getByRole("button", { name: "All" }).click();
  await io.getByRole("searchbox", { name: "Find in the links" }).fill("lamp1");
  await io.getByRole("button", { name: /^trigger_multiple #/ }).first().click();
  await expect.poll(async () => (await sel(page)).entities).toEqual([m.trig]);
  await io.getByRole("button", { name: "Picked chain" }).click();
  await io.getByRole("button", { name: /^Look at trigger_multiple/ }).first().click();
  await io.getByRole("searchbox", { name: "Find in the links" }).fill("");
  await io.getByRole("button", { name: "Broken" }).click();
  await io.getByRole("button", { name: "Remove" }).first().click();
  await expect.poll(async () => (await entity(page, m.spawn))!.keys.target ?? null).toBeNull();
});

test("Gameplay: Components / Mode kits, search, Place, component file save and load", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "View", "Panel: Gameplay");
  const g = page.getByRole("group", { name: "Gameplay" });
  await g.getByRole("button", { name: "Mode kits" }).click();
  // (no kits in a fresh app folder; placing a kit's markers: editor-kits.spec.ts)
  await expect(page.getByRole("group", { name: "Made for" })).toBeVisible();
  await g.getByRole("button", { name: "Components" }).click();
  await page.getByRole("searchbox", { name: "Search components" }).fill("checkpoint");
  await expect(page.getByText("Race gates passed in order.")).toBeVisible();
  await page.getByRole("button", { name: "Place" }).first().click();
  // placing waits for a click in the view: one click on the floor
  const pane = (await page.locator(".ed-pane").first().boundingBox())!;
  const e0 = (await counts(page)).entities;
  await page.mouse.click(pane.x + pane.width / 2, pane.y + pane.height * 0.7);
  await expect.poll(async () => (await counts(page)).entities).toBe(e0 + 1);
  // component file: save (into the game copy's configs/components) and load
  await page.getByRole("textbox", { name: "Map" }).fill("cov_components");
  await page.getByRole("button", { name: "Save as component file" }).click();
  await expect(page.locator(".toast, [role=status]").filter({ hasText: /cov_components/ }).first()).toBeVisible();
  await page.getByRole("button", { name: "Load component file" }).click();
  await expect(page.locator(".toast, [role=status]").filter({ hasText: /component|loaded|Loaded/ }).first()).toBeVisible();
});

test("Content: sections, search, size, sources, favorites, recent, right-click menu, Refresh", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await menu(page, "View", "Content Browser");
  const cb = page.getByRole("region", { name: "Content Browser" });
  for (const s of ["Sprites", "Sounds", "Textures", "Prefabs", "GameBanana", "Models"]) {
    await cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: s }).click();
    await expect(cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: s })).toHaveAttribute("aria-selected", "true");
  }
  await cb.getByRole("searchbox", { name: "Search" }).fill("p_knife");
  await expect(cb.getByRole("option", { name: /p_knife/ }).first()).toBeVisible();
  await cb.getByRole("slider", { name: "Thumbnail size" }).fill("128");
  await expect.poll(() => ls(page, "ls.editor.content.size")).toBe("128");
  await cb.getByRole("navigation", { name: "Where from" }).getByRole("button", { name: /^Game/ }).click();
  await cb.getByRole("option", { name: /p_knife/ }).first().click({ button: "right" });
  await page.getByRole("menuitem", { name: /Favorite|favorite/ }).click();
  await cb.getByRole("navigation", { name: "Where from" }).getByRole("button", { name: /Favorites 1/ }).click();
  await expect(cb.getByRole("option", { name: /p_knife/ })).toBeVisible();
  await cb.getByRole("option", { name: /p_knife/ }).first().click({ button: "right" });
  await page.getByRole("menuitem", { name: "Find uses in the map" }).click();
  await cb.getByRole("navigation", { name: "Where from" }).getByRole("button", { name: /^Recent/ }).click();
  await cb.getByRole("button", { name: "Refresh" }).click();
  // the drawer (View > Content drawer) and its Close
  await menu(page, "View", "Content drawer");
  const drawer = page.getByRole("dialog", { name: "Content drawer" });
  await expect(drawer).toBeVisible();
  // a click away closes it
  await page.locator(".ed-bar-name").click();
  await expect(drawer).toHaveCount(0);
});

test("Prefabs: save what's picked, find it, place it, rename and tags, share as a package, delete", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await menu(page, "View", "Content Browser");
  const cb = page.getByRole("region", { name: "Content Browser" });
  await cb.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: "Prefabs" }).click();
  await select(page, { brushes: [m.a], entities: [m.light] });
  await page.getByRole("button", { name: "Save what's picked…" }).click();
  const save = page.getByRole("dialog").last();
  await save.getByRole("textbox").first().fill("cov crate");
  await save.getByRole("button", { name: "Save prefab" }).click();
  await page.getByRole("searchbox", { name: "Search prefabs" }).fill("cov crate");
  const card = page.locator("li, .p4-prefab, [role=option]").filter({ hasText: "cov crate" }).first();
  await expect(card).toBeVisible();
  const n = (await counts(page)).brushes;
  await card.getByRole("button", { name: "Place" }).click();
  const pane = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(pane.x + pane.width / 2, pane.y + pane.height * 0.7);
  await expect.poll(async () => (await counts(page)).brushes).toBe(n + 1);
  await card.getByRole("button", { name: "More for cov crate" }).click();
  await page.getByRole("menuitem", { name: "Rename and tags…" }).click();
  const ren = page.getByRole("dialog", { name: "Rename prefab" });
  await ren.getByRole("textbox").first().fill("cov crate 2");
  await ren.getByRole("button", { name: "Save" }).click();
  await page.getByRole("searchbox", { name: "Search prefabs" }).fill("cov crate 2");
  const card2 = page.locator("li, .p4-prefab, [role=option]").filter({ hasText: "cov crate 2" }).first();
  await expect(card2).toBeVisible();
  await card2.getByRole("button", { name: "More for cov crate 2" }).click();
  await page.getByRole("menuitem", { name: "Delete…" }).click();
  await page.getByRole("button", { name: /^Delete/ }).last().click();
  await expect(card2).toHaveCount(0);
});

test("dock: tabs, ⋯ Float, Dock back, Dock: Left, Close, Open in its own window, fold the bottom", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await tab(page, "Budget").click();
  await expect(tab(page, "Budget")).toHaveAttribute("aria-selected", "true");
  const more = page.getByRole("button", { name: "Panel options" }).first();
  await more.click();
  await page.getByRole("menu", { name: "Panel options" }).getByRole("menuitem", { name: "Float" }).click();
  await expect(page.locator(".ed-float-title").first()).toBeVisible();
  await page.getByRole("button", { name: /^Dock / }).first().click();
  await expect(page.locator(".ed-float-title")).toHaveCount(0);
  // the ⋯ of the area a tab is in
  const optionsBy = (title: string) => page.locator("[data-dock-strip]").filter({ has: page.getByRole("tab", { name: title, exact: true }) }).getByRole("button", { name: "Panel options" });
  await tab(page, "Checks").click();
  await optionsBy("Checks").click();
  await page.getByRole("menu", { name: "Panel options" }).getByRole("menuitem", { name: "Dock: Left" }).click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.dock2"))!).areas.left.tabs).toContain("checks");
  await optionsBy("Checks").click();
  await page.getByRole("menu", { name: "Panel options" }).getByRole("menuitem", { name: /^Close/ }).click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.dock2"))!).areas.left.tabs).not.toContain("checks");
  // own window
  await tab(page, "History").click();
  const popup = page.waitForEvent("popup", { timeout: 10_000 }).catch(() => null);
  await optionsBy("History").click();
  await page.getByRole("menu", { name: "Panel options" }).getByRole("menuitem", { name: "Open in its own window" }).click();
  expect(await popup, "a window opens for the panel").not.toBeNull();
  // fold the bottom panels and unfold
  const fold = page.getByRole("button", { name: /Fold the bottom panels|Unfold/ }).first();
  await fold.click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.dock2"))!).bottomFolded).toBe(true);
  await page.getByRole("button", { name: "Open the bottom panels" }).first().click();
  await expect.poll(async () => JSON.parse((await ls(page, "ls.editor.dock2"))!).bottomFolded).toBe(false);
});

test("the tool rail and each tool's options", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  const rail = page.getByRole("navigation", { name: "Editing tools" });
  for (const t of ["Select", "Move", "Rotate", "Scale", "Vertex", "Box", "Shape", "Clip", "Entity", "Texture", "Measure"]) {
    await rail.getByRole("button", { name: new RegExp(`^${t}`) }).click();
    await expect.poll(() => st(page, (s) => s.tool)).toBe(t.toLowerCase());
  }
  // shared options
  await page.getByRole("combobox", { name: "Grid" }).selectOption("32");
  await expect.poll(() => st(page, (s) => s.grid)).toBe(32);
  await page.getByRole("button", { name: /^Snap (on|off)$/ }).click();
  await expect.poll(() => st(page, (s) => !!s.snapOff)).toBe(true);
  // (scale lock works with texture lock: greyed out when that's off)
  await page.getByRole("checkbox", { name: "Scale lock" }).uncheck();
  await expect.poll(() => st(page, (s) => s.scaleLock)).toBe(false);
  await page.getByRole("checkbox", { name: "Texture lock" }).uncheck();
  await expect.poll(() => st(page, (s) => s.textureLock)).toBe(false);
  await expect(page.getByRole("checkbox", { name: "Scale lock" })).toBeDisabled();
  // select's pick mode and box picking
  await rail.getByRole("button", { name: /^Select/ }).click();
  for (const [b, v] of [["Pick objects", "objects"], ["Pick brushes", "solids"], ["Pick groups", "groups"]] as const) {
    await page.getByRole("group", { name: "What a click picks" }).getByRole("button", { name: b }).click();
    await expect.poll(() => st(page, (s) => s.selectMode)).toBe(v);
  }
  await page.getByRole("combobox", { name: "Box" }).selectOption("Touching");
  await expect.poll(() => ls(page, "ls.editor.marquee")).not.toBeNull();
  await page.getByRole("checkbox", { name: "Only what you can see" }).check();
  await expect.poll(() => ls(page, "ls.editor.marqueeVisible")).toBe("1");
  await page.getByRole("checkbox", { name: "Box anywhere in 3D" }).check();
  // Box: height
  await rail.getByRole("button", { name: /^Box/ }).click();
  await page.getByRole("spinbutton", { name: "Height" }).fill("96");
  await expect.poll(() => st(page, (s) => s.boxHeight)).toBe(96);
  // Shape: each kind, sides, preview
  await rail.getByRole("button", { name: /^Shape/ }).click();
  for (const k of ["Wedge", "Spike", "Sphere", "Arch", "Cone", "Torus", "Cylinder"]) {
    await page.getByRole("group", { name: "Shape" }).getByRole("button", { name: k }).click();
    await expect.poll(() => st(page, (s) => s.shape)).toBe(k.toLowerCase());
  }
  await page.getByRole("spinbutton", { name: "Sides" }).fill("8");
  await expect.poll(() => st(page, (s) => s.sides)).toBe(8);
  await page.getByRole("button", { name: "Preview…" }).click();
  await expect(page.locator('[aria-label="Preview of the shape"]').first()).toBeVisible();
  // Clip: which side stays
  await rail.getByRole("button", { name: /^Clip/ }).click();
  await page.getByRole("group", { name: "Which side stays (Tab)" }).getByRole("button", { name: "Keep behind" }).click();
  await expect(page.getByRole("button", { name: "Cut" })).toBeDisabled();
  // Entity: the class to place
  await rail.getByRole("button", { name: /^Entity/ }).click();
  await page.getByRole("combobox", { name: "Entity to place" }).fill("light");
  await expect.poll(() => st(page, (s) => s.entityClass)).toBe("light");
  // Texture: what a click does
  await rail.getByRole("button", { name: /^Texture/ }).click();
  await page.getByRole("group", { name: "What a click does" }).getByRole("button", { name: "Click paints" }).click();
  await expect.poll(() => st(page, (s) => s.texClick)).toBe("paint");
  // Vertex: Merge corners greyed out with nothing picked
  await rail.getByRole("button", { name: /^Vertex/ }).click();
  await expect(page.getByRole("button", { name: "Merge corners" })).toBeDisabled();
  // the pane's drawing style
  await page.getByRole("combobox", { name: /How pane 1 draws/ }).selectOption("wire");
  await expect(page.getByRole("combobox", { name: /How pane 1 draws/ })).toHaveValue("wire");
});

test("tools in the views: Box draws, Entity places, Texture picks a face, Measure measures, Select clicks", async ({ page }) => {
  await openEditor(page);
  await testMap(page);
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ view: "top" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await page.waitForTimeout(800);
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  const at = (fx: number, fy: number) => [r.x + r.width * fx, r.y + r.height * fy] as const;
  // Box: drag in Top
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "box" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  const n = (await counts(page)).brushes;
  await page.mouse.move(...at(0.2, 0.2));
  await page.mouse.down();
  await page.mouse.move(...at(0.3, 0.3), { steps: 8 });
  await page.mouse.up();
  await expect.poll(async () => (await counts(page)).brushes).toBe(n + 1);
  // Entity: click places one
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "entity", entityClass: "info_player_deathmatch" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  const e = (await counts(page)).entities;
  await page.mouse.click(...at(0.7, 0.7));
  await expect.poll(async () => (await counts(page)).entities).toBe(e + 1);
  // Measure: a drag leaves a line
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "measure" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await page.mouse.move(...at(0.4, 0.4));
  await page.mouse.down();
  await page.mouse.move(...at(0.6, 0.4), { steps: 6 });
  await page.mouse.up();
  await expect.poll(() => st(page, (s) => !!s.measure)).toBe(true);
  // Select: a click in empty space drops the selection, a click on the floor picks it
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "select", view: "3d" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await page.waitForTimeout(500);
  const r3 = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(r3.x + r3.width / 2, r3.y + r3.height * 0.85);
  await expect.poll(async () => (await sel(page)).brushes.length).toBe(1);
  // Texture: a click picks the face under it
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "texture", texClick: "pick" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await select(page, {});
  await page.mouse.click(r3.x + r3.width / 2, r3.y + r3.height * 0.85);
  await expect.poll(() => st(page, (s) => !!s.sel.face)).toBe(true);
});

test("status bar: what's picked and its size, the counts", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a, m.b] });
  await expect(page.locator(".ed-selsize")).toContainText("2 picked · 192 × 64 × 64");
  const c = await counts(page);
  await expect(page.getByText(`${c.brushes} brushes · ${c.entities} entities`)).toBeVisible();
});

test("MCP selection: the page reports its selection; an agent's selection shows in the page", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a], entities: [m.light] });
  await expect.poll(() => page.evaluate(async () => (await (await fetch("/api/tools/editor/selection")).json()))).toMatchObject({ brushes: [m.a], entities: [m.light] });
  const got = await page.evaluate(async () => (await (await fetch("/api/tools/editor/selection")).json()));
  expect(got.tool).toBe("select");
  // an agent picks the far box (POST, as the MCP tool does)
  const r = await page.evaluate(async (b) => (await fetch("/api/tools/editor/selection", { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify({ brushes: [b], entities: [], faces: [], by: "agent" }) })).json(), m.b);
  expect(r.error ?? null).toBeNull();
  await expect.poll(async () => (await sel(page)).brushes, { timeout: 30_000 }).toEqual([m.b]);
});
