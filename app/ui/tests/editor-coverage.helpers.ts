// Shared helpers for the editor coverage sweep (tests/editor-coverage-*.spec.ts,
// docs/notes/EditorCoverage.md). The specs run on editor-dev.html against a Tools dev server
// (docs/TOOLS-DEV.md) over a COPY of the game, with a throwaway LS_APP_DIR:
//
//   LS_TEST_GAME=<sim copy> LS_APP_DIR=<scratch> LS_DEV_PICK=<scratch>/pick/current \
//   LS_DEV_PICK_SAVE=<scratch>/out/qa.lsmap cargo test serve_tools_dev -- --ignored
//   LS_VITE_PORT=<port> npx playwright test editor-coverage
//
// File dialogs answer with LS_DEV_PICK (a symlink these helpers re-point: set LS_COV_PICK to
// it) and LS_DEV_PICK_SAVE (LS_COV_SAVE). Without them the file specs skip.
//
// The editor's session lives in the dev server (one per server), so these run one at a time.
import { expect, type Page } from "@playwright/test";
import { existsSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export const PICK = process.env.LS_COV_PICK ?? "";
export const SAVE = process.env.LS_COV_SAVE ?? "";
export const FIX_DIR = process.env.LS_COV_FIX ?? (PICK ? join(dirname(dirname(PICK)), "fix") : "");

/** Point the dev server's file dialog at this file (LS_DEV_PICK is a symlink). */
export function pickFile(path: string) {
  if (!PICK) throw new Error("LS_COV_PICK isn't set");
  rmSync(PICK, { force: true });
  symlinkSync(path, PICK);
}

/** Write a fixture into the fixtures folder and return its path. */
export function fixture(name: string, text: string | Buffer): string {
  mkdirSync(FIX_DIR, { recursive: true });
  const p = join(FIX_DIR, name);
  writeFileSync(p, text);
  return p;
}

export const readSaved = () => (SAVE && existsSync(SAVE) ? readFileSync(SAVE) : null);
export const clearSaved = () => SAVE && rmSync(SAVE, { force: true });

type Opts = { preset?: "loner" | "hammer" | "trenchbroom"; nav?: "unreal" | "blender" | "hammer"; chosen?: boolean; tour?: boolean; layout?: string };

/** The editor on a fresh new map (the dev page, nothing remembered but the given settings). */
export async function openEditor(page: Page, o: Opts = {}) {
  await page.addInitScript((o: Opts) => {
    if ((window as unknown as { __covInit?: boolean }).__covInit) return;
    (window as unknown as { __covInit?: boolean }).__covInit = true;
    // only on the first load of the page: a reload keeps what the test changed
    if (sessionStorage.getItem("cov.init")) return;
    sessionStorage.setItem("cov.init", "1");
    localStorage.clear();
    if (o.chosen !== false) localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: o.nav ?? "blender", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    if (!o.tour) localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.editor.keys", JSON.stringify({ preset: o.preset ?? "loner", mine: {} }));
  }, o);
  await page.goto("/editor-dev.html");
  await installHooks(page);
  // either the start screen or an open map (the server keeps its session between tests)
  const start = page.getByRole("button", { name: "New map" });
  const bar = page.locator(".ed-menubar");
  await expect(start.or(bar).first()).toBeVisible({ timeout: 30_000 });
  await newMap(page);
}

/** Test hooks from the dev server's own modules (Vite serves them, so these are the same
 *  instances the page uses): run a command by id, and whether it can run now. */
export async function installHooks(page: Page) {
  await page.evaluate(async () => {
    const sc = await import(/* @vite-ignore */ "/src/views/tools/editor/Shortcuts.ts");
    const cm = await import(/* @vite-ignore */ "/src/views/tools/editor/commands.ts");
    const w = window as unknown as Record<string, unknown>;
    w.__covRun = (id: string) => sc.runCommand(id);
    w.__covEnabled = (id: string) => {
      const c = cm.commands().find((x: { id: string }) => x.id === id);
      return c ? !c.when || c.when() : null;
    };
    w.__covKeys = (id: string) => sc.keysOf(id);
    w.__covCommands = () => cm.commands().map((c: { id: string; label: string; menu: string; hidden?: boolean }) => ({ id: c.id, label: c.label, menu: c.menu, hidden: !!c.hidden }));
  });
}

/** File > New map (answering "Leave them"), then waits for the views. */
export async function newMap(page: Page) {
  if (await page.locator(".ed-menubar").isVisible()) {
    await menu(page, "File", "New map");
  } else {
    await page.getByRole("button", { name: "New map" }).first().click();
  }
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor({ timeout: 30_000 });
  await expect.poll(() => st(page, (s) => s.doc?.revision != null && (s.doc?.undo ?? null) === null)).toBe(true);
}

/** Opens a menu and clicks an item (exact label). */
export async function closeMenus(page: Page) {
  // (not Escape: in the editor Escape also drops the selection)
  const open = page.locator(".ed-menubar .ed-menu-title[aria-expanded=true]");
  if (await open.count()) await open.first().click();
}

export async function menu(page: Page, title: string, item: string | RegExp) {
  await closeMenus(page);
  await page.locator(".ed-menubar .ed-menu-title", { hasText: new RegExp(`^${title}$`) }).click();
  const list = page.locator(`.ed-menu-list[aria-label="${title}"]`);
  await expect(list).toBeVisible();
  const btn = menuButton(page, title, item);
  await expect(btn.first()).toBeEnabled();
  await btn.first().click();
}

/** A menu item by its label (its keys aside). */
export function menuButton(page: Page, title: string, item: string | RegExp) {
  const re = typeof item === "string" ? new RegExp(`^${escapeRe(item)}$`) : item;
  return page.locator(`.ed-menu-list[aria-label="${title}"] button[role=menuitem]`).filter({ has: page.locator(":scope > span:first-child", { hasText: re }) });
}

/** Is a menu item there and enabled? (null: not in the menu) */
export async function menuItemEnabled(page: Page, title: string, item: string): Promise<boolean | null> {
  await closeMenus(page);
  await page.locator(".ed-menubar .ed-menu-title", { hasText: new RegExp(`^${title}$`) }).click();
  const btn = menuButton(page, title, item);
  const n = await btn.count();
  const r = n ? await btn.first().isEnabled() : null;
  await closeMenus(page);
  return r;
}

/** Runs a command through the palette (Find a command), by its label. */
export async function palette(page: Page, label: string) {
  await page.locator(".ed-menu-find").click();
  const box = page.getByRole("dialog").locator("input").first();
  await box.fill(label);
  await page.getByRole("option", { name: new RegExp(`^${escapeRe(label)}`) }).first().click();
}

export const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// ---- the store (dev builds put it on window.__lsEditor) -----------------------------------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type S = any;
/** Reads something from the editor's store. */
export async function st<T>(page: Page, fn: (s: S) => T): Promise<T> {
  return page.evaluate(`(${fn.toString()})(window.__lsEditor.getState())`) as Promise<T>;
}
/** Runs a store command (the same as a button would) and waits for it. */
export async function cmd(page: Page, body: Record<string, unknown>) {
  const r = await page.evaluate(async (b) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).__lsEditor.getState();
    const r = await s.cmd(b);
    return { error: r.error ?? null, created: r.created ?? null };
  }, body);
  expect(r.error, JSON.stringify(body)).toBeNull();
  return r.created as { brushes: number[]; entities: number[] } | null;
}
export async function select(page: Page, sel: { brushes?: number[]; entities?: number[]; face?: { brush: number; face: number } | null; faces?: { brush: number; face: number }[] }) {
  await page.evaluate((x) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__lsEditor.getState().select({ brushes: x.brushes ?? [], entities: x.entities ?? [], face: x.face ?? null, faces: x.faces ?? (x.face ? [x.face] : []) });
  }, sel);
}
export const counts = (page: Page) => st(page, (s) => ({ brushes: s.doc?.brushes.length ?? 0, entities: s.doc?.entities.length ?? 0, groups: s.doc?.groups.length ?? 0 }));
export const undoLabel = (page: Page) => st(page, (s) => s.doc?.undo ?? null);
export const sel = (page: Page) => st(page, (s) => ({ brushes: [...s.sel.brushes].sort((a: number, b: number) => a - b), entities: [...s.sel.entities].sort((a: number, b: number) => a - b), face: s.sel.face }));

/** A small test map: a 256x256x16 floor, two 64-unit boxes, a light, a spawn, a trigger. */
export async function testMap(page: Page) {
  await newMap(page);
  const tex = await st(page, (s) => s.texture);
  const floor = (await cmd(page, { op: "add_box", min: [-256, -256, -16], max: [256, 256, 0], texture: tex }))!.brushes[0];
  const a = (await cmd(page, { op: "add_box", min: [0, 0, 0], max: [64, 64, 64], texture: tex }))!.brushes[0];
  const b = (await cmd(page, { op: "add_box", min: [128, 0, 0], max: [192, 64, 64], texture: tex }))!.brushes[0];
  const light = (await cmd(page, { op: "add_entity", classname: "light", origin: [0, -128, 96], keys: [["targetname", "lamp1"]] }))!.entities[0];
  const spawn = (await cmd(page, { op: "add_entity", classname: "info_player_start", origin: [-128, -128, 36] }))!.entities[0];
  const tb = (await cmd(page, { op: "add_box", min: [-192, 64, 0], max: [-128, 128, 64], texture: tex }))!.brushes[0];
  const trig = (await cmd(page, { op: "tie", brushes: [tb], classname: "trigger_multiple" }))!.entities[0];
  await cmd(page, { op: "set_key", entity: trig, key: "target", value: "lamp1" });
  await select(page, {});
  return { floor, a, b, light, spawn, trigBrush: tb, trig, tex };
}

/** Brush bounds from the document. */
export async function boundsOf(page: Page, id: number): Promise<[number[], number[]] | null> {
  return page.evaluate((id) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const b = (window as any).__lsEditor.getState().doc.brushes.find((x: any) => x.id === id);
    return b ? [b.min, b.max] : null;
  }, id);
}
export async function entity(page: Page, id: number) {
  return page.evaluate((id) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const e = (window as any).__lsEditor.getState().doc.entities.find((x: any) => x.id === id);
    return e ? { classname: e.classname as string, keys: Object.fromEntries(e.keys) as Record<string, string>, brushes: e.brushes as number[], group: e.group as number | null } : null;
  }, id);
}

/** Undo through the menu and wait for the document to change back. */
export async function undo(page: Page) {
  const before = await st(page, (s) => s.doc.revision);
  await menu(page, "Edit", "Undo");
  await expect.poll(() => st(page, (s) => s.doc.revision)).not.toBe(before);
}
export async function idle(page: Page) {
  await expect.poll(() => st(page, (s) => s.working === 0 && !s.fileBusy), { timeout: 20_000 }).toBe(true);
}

/** Presses keys with focus on the views (not a field). */
export async function key(page: Page, k: string) {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.keyboard.press(k);
}

/** The panel tab (docked anywhere). */
export async function showPanel(page: Page, title: string) {
  await menu(page, "View", `Panel: ${title}`);
}

/** Records console errors (page errors and console.error) for a test. */
export function consoleErrors(page: Page) {
  const errs: string[] = [];
  page.on("pageerror", (e) => errs.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error" && !/404|Failed to load resource/.test(m.text())) errs.push(m.text());
  });
  return errs;
}

export const MOD = process.platform === "darwin" ? "Meta" : "Control";
