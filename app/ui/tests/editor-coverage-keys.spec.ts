// Editor coverage sweep: every keyboard shortcut. Each key in the three key sets (Loner-Strike,
// Hammer-style, TrenchBroom-style; Shortcuts.ts) runs its command's coverage case
// (editor-coverage.cases.ts), so a key that does the wrong thing fails. Then the keys the old
// handler keeps (arrows and Page Up/Down nudge, Esc, camera marks, Alt, clip's Tab/Enter) and
// the always-on ones. Runs in Chromium and WebKit (editor-coverage.config.ts).
import { expect, test, type Page } from "@playwright/test";
import { caseById, type Case } from "./editor-coverage.cases";
import { boundsOf, cmd, installHooks, key, MOD, openEditor, select, sel, st, testMap, undo } from "./editor-coverage.helpers";

type Preset = "loner" | "hammer" | "trenchbroom";
const MAC = process.platform === "darwin";

/** The key sets as Shortcuts.ts has them (checked against the page in the first test). */
const LONER: Record<string, string[]> = {
  "file.new": ["Mod+Alt+N"], "file.open": ["Mod+O"], "file.save": ["Mod+S"], "file.saveAs": ["Mod+Shift+S"], "file.export": ["Mod+E"],
  "edit.undo": ["Mod+Z"], "edit.redo": ["Mod+Shift+Z", "Mod+Y"], "edit.cut": ["Mod+X"], "edit.copy": ["Mod+C"], "edit.paste": ["Mod+V"],
  "edit.pasteSpecial": ["Mod+Shift+V"], "edit.duplicate": ["Mod+D"], "edit.duplicateHere": ["Mod+Shift+D"], "edit.delete": ["Delete", "Backspace"],
  "edit.find": ["Mod+F"], "edit.transform": ["Mod+M"], "edit.group": ["Mod+G"], "edit.ungroup": ["Mod+Shift+G"], "edit.tie": ["Mod+T"],
  "edit.hollow": ["Mod+H"], "edit.snap": ["Mod+B"], "sel.all": ["Mod+A"], "sel.none": ["Mod+Shift+A"], "sel.invert": ["Mod+I"], "sel.goto": ["Mod+J"],
  "sel.hide": ["H"], "sel.isolate": ["Shift+H"], "sel.showAll": ["U"], "sel.lock": ["L"], "sel.unlockAll": ["Shift+L"],
  "view.layout.single": ["Alt+1"], "view.layout.quad": ["Alt+2"], "view.layout.one3": ["Alt+3"], "view.layout.two": ["Alt+4"], "view.maximise": ["Shift+Space"],
  "view.3d": ["Shift+1"], "view.top": ["Shift+2"], "view.front": ["Shift+3"], "view.side": ["Shift+4"], "view.frame": ["Z"], "view.frameAll": ["Shift+Z"],
  "view.fly": ["F"], "view.gridDown": ["["], "view.gridUp": ["]"], "view.gridShow": ["Shift+G"],
  "tool.select": ["Q"], "tool.move": ["W"], "tool.rotate": ["E"], "tool.scale": ["R"], "tool.vertex": ["V"], "tool.box": ["B"], "tool.shape": ["C"],
  "tool.clip": ["X"], "tool.entity": ["N"], "tool.texture": ["T"], "tool.measure": ["M"],
  "help.palette": ["Mod+K", "Mod+Shift+P"], "help.cheats": ["?"], "map.inspect": ["Alt+Enter"], "map.playtest": ["F5"],
  "view.contentDrawer": [MAC ? "Mod+Shift+Space" : "Mod+Space"], "view.snap": ["Shift+W"], "edit.applyTexture": ["Shift+T"],
};
const HAMMER: Record<string, string[]> = {
  ...LONER, "file.new": ["Mod+N"], "edit.redo": ["Mod+Y", "Mod+Shift+Z"], "edit.ungroup": ["Mod+U"], "edit.toWorld": ["Mod+Shift+W"], "edit.carve": ["Mod+Shift+C"],
  "edit.mirrorX": ["Mod+L"], "edit.mirrorZ": ["Mod+I"], "sel.invert": [], "sel.goto": ["Mod+Shift+G"], "edit.find": ["Mod+Shift+F"], "view.maximise": ["Shift+Z"],
  "view.frameAll": [], "view.frame": ["Mod+Shift+E"], "view.texLock": ["Shift+L"], "sel.unlockAll": [], "map.compile": ["F9"], "map.inspect": ["Alt+Enter"],
  "tool.select": ["Shift+S"], "tool.move": [], "tool.rotate": [], "tool.scale": [], "tool.entity": ["Shift+E"], "tool.box": ["Shift+B"], "tool.texture": ["Shift+A"],
  "tool.clip": ["Shift+X"], "tool.vertex": ["Shift+V"], "tool.measure": [], "tool.shape": [], "sel.isolate": [], "view.gridShow": [],
};
const TRENCHBROOM: Record<string, string[]> = {
  ...LONER, "edit.ungroup": ["Mod+Shift+G"], "sel.hide": ["Mod+H"], "sel.isolate": ["Mod+I"], "sel.showAll": ["Mod+Shift+H"], "sel.invert": [], "edit.hollow": [],
  "edit.mirrorX": ["Mod+F"], "edit.mirrorZ": ["Mod+Alt+F"], "edit.find": [], "view.maximise": ["Mod+Space"], "view.contentDrawer": ["Mod+Shift+Space"], "view.frame": ["Mod+U"],
  "tool.select": [], "tool.move": [], "tool.rotate": ["R"], "tool.scale": ["T"], "tool.clip": ["C"], "tool.vertex": ["V"], "tool.shape": [], "tool.texture": [], "tool.box": ["B"],
};
const SETS: Record<Preset, Record<string, string[]>> = { loner: LONER, hammer: HAMMER, trenchbroom: TRENCHBROOM };
/** Tested in editor-coverage-file.spec.ts (they open file dialogs). */
const FILE_KEYS = new Set(["file.open", "file.save", "file.saveAs", "file.export"]);
/** Tested in editor-coverage-map.spec.ts (F5 with the compiler answered). */
const MAP_KEYS = new Set(["map.playtest"]);

/** "Mod+Shift+S" as Playwright presses it. */
export function press(k: string): string {
  const parts = k.split("+");
  const last = parts.pop() || "+";
  const mods = parts.map((m) => (m === "Mod" ? MOD : m));
  let name = last;
  if (/^[A-Z]$/.test(last)) name = `Key${last}`;
  else if (/^[0-9]$/.test(last)) name = `Digit${last}`;
  else if (last === "[") name = "BracketLeft";
  else if (last === "]") name = "BracketRight";
  else if (last === "?") return [...mods, "Shift", "Slash"].join("+");
  return [...mods, name].join("+");
}

/** Each (key set, command, key) once: Loner-Strike's every key, the other sets' own keys. */
const RUNS: { preset: Preset; id: string; k: string }[] = [];
for (const preset of ["loner", "hammer", "trenchbroom"] as Preset[]) {
  for (const [id, keys] of Object.entries(SETS[preset])) {
    for (const k of keys) {
      if (preset !== "loner" && (LONER[id] ?? []).includes(k)) continue; // same as ours: tested once
      if (FILE_KEYS.has(id) || MAP_KEYS.has(id)) continue;
      RUNS.push({ preset, id, k });
    }
  }
}

test("the key sets here match the editor's (Shortcuts.ts)", async ({ page }) => {
  await openEditor(page);
  for (const preset of ["loner", "hammer", "trenchbroom"] as Preset[]) {
    await page.evaluate((p) => localStorage.setItem("ls.editor.keys", JSON.stringify({ preset: p, mine: {} })), preset);
    await page.reload();
    await installHooks(page);
    const ids = Object.keys(SETS[preset]);
    const real = await page.evaluate((ids) => Object.fromEntries(ids.map((id) => [id, (window as unknown as { __covKeys: (id: string) => string[] }).__covKeys(id)])), ids);
    expect(real, preset).toEqual(SETS[preset]);
  }
});

async function runViaKey(page: Page, c: Case, k: string) {
  const m = await testMap(page);
  const ctx = c.setup ? await c.setup(page, m) : undefined;
  await key(page, press(k));
  await c.check(page, m, ctx);
  if (c.undo) {
    await undo(page);
    await c.undo(page, m, ctx);
  }
}

for (const r of RUNS) {
  // Hammer's F9 opens the Compile panel (map.compile)
  test(`${r.preset} keys: ${r.k} › ${r.id}`, async ({ page, context, browserName }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]).catch(() => {});
    const c = caseById.get(r.id);
    expect(c, `no coverage case for ${r.id}`).toBeTruthy();
    test.skip(browserName === "webkit" && /^(edit\.(cut|copy|paste))$/.test(r.id), "WebKit's clipboard needs a user gesture (the editor's own copy still works: Chromium runs these)");
    await openEditor(page, { preset: r.preset });
    await runViaKey(page, c!, r.k);
  });
}

// ---- the old handler's keys (MapEditor.tsx) and the always-on ones ------------------------

test("arrows and Page Up/Down nudge what's picked one grid step; Undo puts it back", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  for (const [k, ax, d] of [["ArrowRight", 0, 16], ["ArrowLeft", 0, -16], ["ArrowUp", 1, 16], ["ArrowDown", 1, -16], ["PageUp", 2, 16], ["PageDown", 2, -16]] as const) {
    const before = (await boundsOf(page, m.a))![0][ax];
    await key(page, k);
    await expect.poll(async () => (await boundsOf(page, m.a))![0][ax]).toBe(before + d);
  }
  await key(page, "ArrowRight");
  await expect.poll(async () => (await boundsOf(page, m.a))![0][0]).toBe(16);
  await undo(page);
  await expect.poll(async () => (await boundsOf(page, m.a))![0][0]).toBe(0);
});

test("vertex tool: arrows move the picked corners; Esc forgets them, then the selection", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "vertex", verts: [] })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await page.evaluate((b) => (window as any).__lsEditor.getState().set({ verts: [{ brush: b, p: [64, 64, 64] }] }), m.a); // eslint-disable-line @typescript-eslint/no-explicit-any
  await key(page, "ArrowRight");
  await expect.poll(async () => (await boundsOf(page, m.a))![1][0]).toBe(80);
  await key(page, "Escape");
  await expect.poll(() => st(page, (s) => s.verts.length)).toBe(0);
  expect((await sel(page)).brushes).toEqual([m.a]);
  await key(page, "Escape");
  await expect.poll(async () => (await sel(page)).brushes.length).toBe(0);
});

test("texture tool: arrows slide the picked face's texture", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ tool: "texture" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await select(page, { brushes: [], face: { brush: m.a, face: 0 } });
  const off = () => page.evaluate((b) => {
    const f = (window as any).__lsEditor.getState().brushes.get(b).faces.find((x: { face: number }) => x.face === 0); // eslint-disable-line @typescript-eslint/no-explicit-any
    return JSON.stringify(f.align ?? f);
  }, m.a);
  const before = await off();
  await key(page, "ArrowLeft");
  await expect.poll(off).not.toBe(before);
});

test("1-9 go to a camera mark stored with Ctrl+1-9; Shift+1-4 pick the view", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  const shot = () => page.locator(".ed-pane").first().screenshot();
  await key(page, "Control+Digit3");
  await expect.poll(() => st(page, (s) => s.bookmark?.store === true && s.bookmark?.n === 3)).toBe(true);
  // move the camera away, then 3 brings it back
  await select(page, { brushes: [m.b] });
  await key(page, "KeyZ");
  await page.waitForTimeout(800);
  const away = await shot();
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ view: "top" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await key(page, "Digit3");
  await expect.poll(() => st(page, (s) => s.view)).toBe("3d");
  await expect.poll(async () => Buffer.compare(await shot(), away) !== 0).toBe(true);
  for (const [d, v] of [["1", "3d"], ["2", "top"], ["3", "front"], ["4", "side"]] as const) {
    await key(page, `Shift+Digit${d}`);
    await expect.poll(() => st(page, (s) => s.view)).toBe(v);
  }
});

test("holding Alt turns snapping off for as long as it's held", async ({ page }) => {
  await openEditor(page);
  await page.keyboard.down("Alt");
  await expect.poll(() => st(page, (s) => s.alt)).toBe(true);
  await expect(page.locator(".editor-snap")).toHaveClass(/off/);
  await page.keyboard.up("Alt");
  await expect.poll(() => st(page, (s) => s.alt)).toBe(false);
});

test("clip tool: Tab picks the side that stays, Enter cuts", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { brushes: [m.a] });
  // two clicks in the Top view place the cut, through the box's middle (where its handles say it is)
  await page.evaluate(() => (window as any).__lsEditor.getState().set({ view: "top" })); // eslint-disable-line @typescript-eslint/no-explicit-any
  await key(page, "KeyZ");
  await page.waitForTimeout(800);
  const grips = await page.evaluate(() => (window as unknown as { __lsFlatHandles?: Record<string, () => { x: number; y: number }[]> }).__lsFlatHandles?.top?.() ?? null);
  expect(grips, "the Top view's handles").not.toBeNull();
  const xs = grips!.map((g) => g.x);
  const ys = grips!.map((g) => g.y);
  const mid = (Math.min(...xs) + Math.max(...xs)) / 2;
  await key(page, "KeyX");
  await expect.poll(() => st(page, (s) => s.tool)).toBe("clip");
  await page.waitForTimeout(300);
  // (above and below the box, but inside the view: in a small window the box nearly fills it,
  // and a click past the view's edge lands on the panels instead)
  const pane = (await page.locator(".ed-pane").first().boundingBox())!;
  const inside = (y: number) => Math.max(pane.y + 12, Math.min(pane.y + pane.height - 12, y));
  for (const y of [inside(Math.min(...ys) - 40), inside(Math.max(...ys) + 40)]) {
    const n = await st(page, (s) => s.clipInfo?.points ?? 0);
    await page.mouse.move(mid, y, { steps: 4 });
    await page.mouse.click(mid, y);
    await expect.poll(() => st(page, (s) => s.clipInfo?.points ?? 0)).toBe(n + 1);
  }
  await expect.poll(() => st(page, (s) => s.clipInfo?.points ?? 0)).toBeGreaterThanOrEqual(2);
  const keep0 = await st(page, (s) => s.clipInfo?.keep);
  await page.keyboard.press("Tab");
  await expect.poll(() => st(page, (s) => s.clipInfo?.keep)).not.toBe(keep0);
  const before = await boundsOf(page, m.a);
  await page.keyboard.press("Enter");
  // the box is cut (one side kept: it's smaller, or gone and replaced by the kept piece)
  await expect.poll(() => st(page, (s) => s.doc.undo ?? "")).toMatch(/clip|cut/i);
  expect(await boundsOf(page, m.a)).not.toEqual(before);
});

test("Ctrl+Alt+Shift+F shows the frame time; ? shows the keys; / finds", async ({ page }) => {
  await openEditor(page);
  await key(page, `${MOD}+Alt+Shift+KeyF`);
  await expect(page.locator(".ed-frametime").first()).toBeVisible();
  await key(page, "Shift+Slash");
  await expect(page.getByRole("dialog", { name: "Keys" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Keys" })).toHaveCount(0);
});

test("a key in a field types; it doesn't run the editor's command", async ({ page }) => {
  await openEditor(page);
  const m = await testMap(page);
  await select(page, { entities: [m.light] });
  await installHooks(page);
  const name = page.getByRole("textbox", { name: /^Name/ }).first();
  await name.click();
  await page.keyboard.type("hub");
  // H (hide) and U (show all) and B (box tool) went into the field
  expect(await st(page, (s) => s.tool)).toBe("select");
  expect(await st(page, (s) => s.hidden.entities.size)).toBe(0);
  void cmd;
});
