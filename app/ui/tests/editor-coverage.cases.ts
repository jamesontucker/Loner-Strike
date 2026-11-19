// The editor coverage sweep's command cases (docs/notes/EditorCoverage.md): one entry per
// command in views/tools/editor/commands.ts. Each says how to set up the test map for it, what
// running it must change, and (for document edits) what Undo must put back. The same cases
// run from the menus (editor-coverage-commands.spec.ts) and from their keys
// (editor-coverage-keys.spec.ts), so a key that runs the wrong thing fails too.
import { expect, type Page } from "@playwright/test";
import { boundsOf, cmd, counts, entity, select, sel, st, testMap } from "./editor-coverage.helpers";

export type TestMap = Awaited<ReturnType<typeof testMap>>;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Ctx = any;
export type Case = {
  id: string;
  menu: "File" | "Edit" | "View" | "Select" | "Map" | "Tools" | "Help";
  label: string;
  /** palette and keys only (not in the menu bar) */
  hidden?: boolean;
  setup?: (p: Page, m: TestMap) => Promise<Ctx>;
  check: (p: Page, m: TestMap, c: Ctx) => Promise<void>;
  /** after Edit > Undo: what must be back */
  undo?: (p: Page, m: TestMap, c: Ctx) => Promise<void>;
  /** runs on the start screen's map only (no test map) */
  bare?: boolean;
};

const poll = <T>(fn: () => Promise<T>, timeout = 10_000) => expect.poll(fn, { timeout });
const nBrushes = async (p: Page) => (await counts(p)).brushes;
const nEnts = async (p: Page) => (await counts(p)).entities;
const ls = (p: Page, k: string) => p.evaluate((k) => localStorage.getItem(k), k);
const dialog = (p: Page, name: string | RegExp) => p.getByRole("dialog", { name });
async function closeDialog(p: Page, name: string | RegExp) {
  await expect(dialog(p, name)).toBeVisible();
  await p.keyboard.press("Escape");
  await expect(dialog(p, name)).toHaveCount(0);
}
/** A tab (panel) is the shown one wherever it's docked. */
const tabShown = (p: Page, title: string) => expect(p.getByRole("tab", { name: title, exact: true }).first()).toHaveAttribute("aria-selected", "true");
const texturesOf = (p: Page, id: number) => p.evaluate((id) => [...((window as any).__lsEditor.getState().brushes.get(id)?.textures ?? [])], id); // eslint-disable-line @typescript-eslint/no-explicit-any
const hidden = (p: Page) => st(p, (s) => ({ brushes: s.hidden.brushes.size, entities: s.hidden.entities.size }));
const paneCount = (p: Page) => p.locator(".ed-pane:visible").count();
/** A second box above the first (for flips, turns, line-ups). */
async function addC(p: Page, m: TestMap) {
  return (await cmd(p, { op: "add_box", min: [128, 128, 128], max: [160, 192, 144], texture: m.tex }))!.brushes[0];
}
async function same<T>(p: Page, fn: () => Promise<T>, before: T) {
  await poll(fn).toEqual(before);
}

export const CASES: Case[] = [
  // ---- File ------------------------------------------------------------------------------
  { id: "file.new", menu: "File", label: "New map", check: async (p) => {
    // the test map has unsaved changes: it asks first
    await p.getByRole("button", { name: "Leave them" }).click();
    await poll(() => st(p, (s) => s.doc.undo)).toBeNull();
    await poll(() => st(p, (s) => s.doc.name)).toBe("Untitled");
  } },
  { id: "file.project", menu: "File", label: "Project files", check: async (p) => tabShown(p, "Project") },
  { id: "file.props", menu: "File", label: "Map properties…", check: async (p) => closeDialog(p, "Map properties") },

  // ---- Edit ------------------------------------------------------------------------------
  { id: "edit.undo", menu: "Edit", label: "Undo", setup: async (p) => nBrushes(p), check: async (p, m, n) => {
    // the last step was "Set target" on the trigger
    await poll(async () => (await entity(p, m.trig))?.keys.target ?? null).toBeNull();
    void n;
  } },
  { id: "edit.redo", menu: "Edit", label: "Redo", setup: async (p, m) => {
    await p.evaluate(() => (window as any).__lsEditor.getState().undo()); // eslint-disable-line @typescript-eslint/no-explicit-any
    await poll(async () => (await entity(p, m.trig))?.keys.target ?? null).toBeNull();
  }, check: async (p, m) => poll(async () => (await entity(p, m.trig))?.keys.target).toBe("lamp1") },
  { id: "edit.history", menu: "Edit", label: "Undo history", check: async (p) => {
    await tabShown(p, "History");
    await expect(p.getByRole("button", { name: "Set target" })).toBeVisible();
  } },
  { id: "edit.cut", menu: "Edit", label: "Cut", setup: async (p, m) => (await select(p, { brushes: [m.a] }), nBrushes(p)), check: async (p, m, n) => {
    await poll(() => nBrushes(p)).toBe(n - 1);
    await poll(() => st(p, (s) => (s.clip ?? "").includes("{"))).toBe(true);
  }, undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.copy", menu: "Edit", label: "Copy", setup: async (p, m) => {
    await p.evaluate(() => (window as any).__lsEditor.getState().set({ clip: null })); // eslint-disable-line @typescript-eslint/no-explicit-any
    await select(p, { brushes: [m.a], entities: [m.light] });
  }, check: async (p) => {
    await poll(() => st(p, (s) => s.clip ?? "")).toContain("light");
    await expect(p.locator(".toast, [role=status]").filter({ hasText: /Copied 1 brush and 1 entity/ }).first()).toBeVisible();
  } },
  { id: "edit.paste", menu: "Edit", label: "Paste", setup: async (p, m) => {
    // copied the way a person does (Edit > Copy writes the editor's copy and the clipboard)
    await select(p, { brushes: [m.a] });
    await p.evaluate(() => (window as any).__covRun("edit.copy")); // eslint-disable-line @typescript-eslint/no-explicit-any
    await poll(() => st(p, (s) => (s.clip ?? "").includes("{"))).toBe(true);
    return nBrushes(p);
  }, check: async (p, m, n) => {
    await poll(() => nBrushes(p)).toBe(n + 1);
    // pasted beside the original (one grid step over) and picked
    const picked = (await sel(p)).brushes;
    expect(picked).toHaveLength(1);
    const b = await boundsOf(p, picked[0]);
    expect(b![0][0]).toBe(16);
  }, undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.pasteSpecial", menu: "Edit", label: "Paste special…", setup: async (p, m) => (await select(p, { brushes: [m.a] }), nBrushes(p)), check: async (p, m, n) => {
    const d = dialog(p, "Paste special");
    await d.getByRole("button", { name: "Row" }).click();
    await d.getByRole("button", { name: /Make 4 copies/ }).click();
    await poll(() => nBrushes(p)).toBe(n + 4);
  }, undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.duplicate", menu: "Edit", label: "Duplicate", setup: async (p, m) => (await select(p, { entities: [m.light] }), nEnts(p)), check: async (p, m, n) => {
    await poll(() => nEnts(p)).toBe(n + 1);
    const copy = (await sel(p)).entities[0];
    const e = await entity(p, copy);
    // a grid step over, with a new name
    expect(e!.keys.origin).toBe("16 -112 96");
    expect(e!.keys.targetname).not.toBe("lamp1");
  }, undo: async (p, m, n) => poll(() => nEnts(p)).toBe(n) },
  { id: "edit.applyTexture", menu: "Edit", label: "Apply the current texture", setup: async (p, m) => {
    await p.evaluate(() => (window as any).__lsEditor.getState().set({ texture: "CRATE01" })); // eslint-disable-line @typescript-eslint/no-explicit-any
    await select(p, { brushes: [m.a] });
  }, check: async (p, m) => poll(() => texturesOf(p, m.a)).toEqual(["CRATE01"]), undo: async (p, m) => poll(() => texturesOf(p, m.a)).toEqual([m.tex]) },
  { id: "edit.duplicateHere", menu: "Edit", label: "Duplicate in place", setup: async (p, m) => (await select(p, { brushes: [m.a] }), nBrushes(p)), check: async (p, m, n) => {
    await poll(() => nBrushes(p)).toBe(n + 1);
    const copy = (await sel(p)).brushes[0];
    expect(copy).not.toBe(m.a);
    expect(await boundsOf(p, copy)).toEqual(await boundsOf(p, m.a));
  }, undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.duplicateSame", menu: "Edit", label: "Duplicate (keep names)", hidden: true, setup: async (p, m) => (await select(p, { entities: [m.light] }), nEnts(p)), check: async (p, m, n) => {
    await poll(() => nEnts(p)).toBe(n + 1);
    const copy = (await sel(p)).entities[0];
    expect((await entity(p, copy))!.keys.targetname).toBe("lamp1");
  }, undo: async (p, m, n) => poll(() => nEnts(p)).toBe(n) },
  { id: "edit.delete", menu: "Edit", label: "Delete", setup: async (p, m) => (await select(p, { brushes: [m.a], entities: [m.light] }), counts(p)), check: async (p, m, n) => {
    await poll(() => counts(p)).toEqual({ ...n, brushes: n.brushes - 1, entities: n.entities - 1 });
  }, undo: async (p, m, n) => poll(() => counts(p)).toEqual(n) },
  { id: "edit.find", menu: "Edit", label: "Find and replace…", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p, m) => {
    const d = dialog(p, "Find and replace");
    await d.getByRole("textbox", { name: "Find" }).fill(m.tex);
    await d.getByRole("textbox", { name: /Replace with/ }).fill("CRATE01");
    await d.getByRole("checkbox", { name: "Only in what's picked" }).check();
    // Find first (it lists what it found), then Replace all
    await d.getByRole("button", { name: "Find", exact: true }).click();
    // (the button then says how many: "Replace 6")
    await d.getByRole("button", { name: /^Replace (all|\d+)$/ }).click();
    await poll(() => texturesOf(p, m.a)).toEqual(["CRATE01"]);
    // only in what's picked: the other box keeps its texture
    expect(await texturesOf(p, m.b)).toEqual([m.tex]);
    await d.getByRole("button", { name: "Close" }).click();
  }, undo: async (p, m) => poll(() => texturesOf(p, m.a)).toEqual([m.tex]) },
  { id: "edit.transform", menu: "Brush", label: "Transform…", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p, m) => {
    const d = dialog(p, "Transform");
    await d.getByRole("spinbutton").first().fill("32");
    await d.getByRole("button", { name: "Apply" }).click();
    await poll(async () => (await boundsOf(p, m.a))![0][0]).toBe(32);
    // (Close, not Esc: after Apply, Esc goes past the dialog; see editor-coverage-dialogs)
    await d.getByRole("button", { name: "Close" }).click();
  }, undo: async (p, m) => poll(async () => (await boundsOf(p, m.a))![0][0]).toBe(0) },
  { id: "edit.replaceWith", menu: "Brush", label: "Replace with entity or prefab…", setup: async (p, m) => select(p, { entities: [m.spawn] }), check: async (p, m) => {
    const d = dialog(p, "Replace with");
    await d.getByRole("combobox", { name: "Entity type" }).fill("info_player_deathmatch");
    await d.getByRole("button", { name: "Replace", exact: true }).click();
    await poll(async () => (await st(p, (s) => s.doc.entities.filter((e: { classname: string }) => e.classname === "info_player_deathmatch").length))).toBeGreaterThan(2);
  }, undo: async (p, m) => poll(async () => (await entity(p, m.spawn))?.classname).toBe("info_player_start") },
  { id: "edit.group", menu: "Brush", label: "Group", setup: async (p, m) => (await select(p, { brushes: [m.a, m.b] }), (await counts(p)).groups), check: async (p, m, g) => poll(async () => (await counts(p)).groups).toBe(g + 1), undo: async (p, m, g) => poll(async () => (await counts(p)).groups).toBe(g) },
  { id: "edit.ungroup", menu: "Brush", label: "Ungroup", setup: async (p, m) => {
    await cmd(p, { op: "group", name: "G", brushes: [m.a, m.b], entities: [] });
    await select(p, { brushes: [m.a] });
    return (await counts(p)).groups;
  }, check: async (p, m, g) => poll(async () => (await counts(p)).groups).toBe(g - 1), undo: async (p, m, g) => poll(async () => (await counts(p)).groups).toBe(g) },
  { id: "edit.tie", menu: "Brush", label: "Tie to entity", setup: async (p, m) => (await select(p, { brushes: [m.a] }), nEnts(p)), check: async (p, m, n) => {
    await poll(() => nEnts(p)).toBe(n + 1);
    const e = (await sel(p)).entities[0];
    expect((await entity(p, e))!.classname).toBe("func_wall");
    expect((await entity(p, e))!.brushes).toEqual([m.a]);
  }, undo: async (p, m, n) => poll(() => nEnts(p)).toBe(n) },
  { id: "edit.toWorld", menu: "Brush", label: "Move to world", setup: async (p, m) => (await select(p, { entities: [m.trig] }), nEnts(p)), check: async (p, m, n) => {
    await poll(() => nEnts(p)).toBe(n - 1);
    expect(await p.evaluate((id) => (window as any).__lsEditor.getState().doc.brushes.find((b: { id: number }) => b.id === id)?.entity ?? null, m.trigBrush)).toBeNull(); // eslint-disable-line @typescript-eslint/no-explicit-any
  }, undo: async (p, m, n) => poll(() => nEnts(p)).toBe(n) },
  { id: "edit.hollow", menu: "Brush", label: "Hollow", setup: async (p, m) => (await select(p, { brushes: [m.a] }), nBrushes(p)), check: async (p, m, n) => poll(() => nBrushes(p)).toBe(n + 5), undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.carve", menu: "Brush", label: "Carve", setup: async (p, m) => {
    const cutter = (await cmd(p, { op: "add_box", min: [32, 32, 32], max: [96, 96, 96], texture: m.tex }))!.brushes[0];
    await select(p, { brushes: [cutter] });
    return { n: await nBrushes(p) };
  }, check: async (p, m, c) => {
    // the box it overlaps is cut into pieces round the cutter
    await poll(async () => (await boundsOf(p, m.a)) === null).toBe(true);
    expect(await nBrushes(p)).toBeGreaterThan(c.n);
  }, undo: async (p, m, c) => poll(() => nBrushes(p)).toBe(c.n) },
  { id: "edit.merge", menu: "Brush", label: "Merge", setup: async (p, m) => (await select(p, { brushes: [m.a, m.b] }), nBrushes(p)), check: async (p, m, n) => {
    await poll(() => nBrushes(p)).toBe(n - 1);
    const merged = (await sel(p)).brushes[0] ?? (await st(p, (s) => s.doc.brushes[s.doc.brushes.length - 1].id));
    expect(await boundsOf(p, merged)).toEqual([[0, 0, 0], [192, 64, 64]]);
  }, undo: async (p, m, n) => poll(() => nBrushes(p)).toBe(n) },
  { id: "edit.snap", menu: "Brush", label: "Snap to grid", setup: async (p, m) => {
    const off = (await cmd(p, { op: "add_box", min: [3, 5, 1], max: [67, 69, 65], texture: m.tex }))!.brushes[0];
    await select(p, { brushes: [off] });
    return off;
  }, check: async (p, m, off) => poll(async () => (await boundsOf(p, off))![0]).toEqual([0, 0, 0]), undo: async (p, m, off) => poll(async () => (await boundsOf(p, off))![0]).toEqual([3, 5, 1]) },
  ...[0, 1, 2].map((ax): Case => ({
    id: `edit.mirror${"XYZ"[ax]}`, menu: "Brush", label: `Flip along ${"XYZ"[ax]}`,
    setup: async (p, m) => {
      const c = await addC(p, m);
      await select(p, { brushes: [m.a, c] });
      return { c, a: await boundsOf(p, m.a), cb: await boundsOf(p, c) };
    },
    check: async (p, m, x) => {
      // flipped round their middle: on that axis the two swap ends
      const lo = Math.min(x.a[0][ax], x.cb[0][ax]);
      const hi = Math.max(x.a[1][ax], x.cb[1][ax]);
      await poll(async () => (await boundsOf(p, m.a))![1][ax]).toBe(lo + hi - x.a[0][ax]);
    },
    undo: async (p, m, x) => poll(() => boundsOf(p, m.a)).toEqual(x.a),
  })),
  ...[0, 1, 2].map((ax): Case => ({
    id: `edit.turn${"XYZ"[ax]}`, menu: "Brush", label: `Turn 90° around ${"XYZ"[ax]}`,
    setup: async (p, m) => {
      const c = await addC(p, m); // 32 x 64 x 16
      await select(p, { brushes: [c] });
      return { c, b: await boundsOf(p, c) };
    },
    check: async (p, m, x) => {
      const size = (b: number[][]) => b[1].map((v, i) => v - b[0][i]);
      const before = size(x.b);
      const want = [...before];
      const [i, j] = [[1, 2], [0, 2], [0, 1]][ax];
      [want[i], want[j]] = [before[j], before[i]];
      await poll(async () => size((await boundsOf(p, x.c))!)).toEqual(want);
    },
    undo: async (p, m, x) => poll(() => boundsOf(p, x.c)).toEqual(x.b),
  })),
  ...[0, 1, 2].flatMap((ax) =>
    (["min", "centre", "max"] as const).map((edge): Case => ({
      id: `edit.align${"XYZ"[ax]}${edge}`, menu: "Brush", hidden: true,
      label: `Line up on ${"XYZ"[ax]}: ${edge === "min" ? (ax === 2 ? "bottoms" : "low sides") : edge === "max" ? (ax === 2 ? "tops" : "high sides") : "middles"}`,
      setup: async (p, m) => {
        const c = await addC(p, m);
        await select(p, { brushes: [m.a, c] });
        return { c, a: await boundsOf(p, m.a), cb: await boundsOf(p, c) };
      },
      check: async (p, m, x) => {
        const at = (b: number[][]) => (edge === "min" ? b[0][ax] : edge === "max" ? b[1][ax] : (b[0][ax] + b[1][ax]) / 2);
        // both line up on the selection's edge (or middle), and something moved
        await poll(async () => at((await boundsOf(p, x.c))!) === at((await boundsOf(p, m.a))!)).toBe(true);
        const moved = at((await boundsOf(p, x.c))!) !== at(x.cb) || at((await boundsOf(p, m.a))!) !== at(x.a);
        expect(moved).toBe(true);
      },
      undo: async (p, m, x) => poll(async () => [await boundsOf(p, m.a), await boundsOf(p, x.c)]).toEqual([x.a, x.cb]),
    })),
  ),
  ...[0, 1, 2].map((ax): Case => ({
    id: `edit.distribute${"XYZ"[ax]}`, menu: "Brush", hidden: true, label: `Spread out evenly along ${"XYZ"[ax]}`,
    setup: async (p, m) => {
      // three boxes on the axis: 0..16, 16..32 (the middle one bunched up), 200..216
      const mk = async (lo: number) => {
        const min = [0, 300, 300];
        const max = [16, 316, 316];
        min[ax] = lo;
        max[ax] = lo + 16;
        return (await cmd(p, { op: "add_box", min, max, texture: m.tex }))!.brushes[0];
      };
      const ids = [await mk(0), await mk(16), await mk(200)];
      await select(p, { brushes: ids });
      return { ids, mid: await boundsOf(p, ids[1]) };
    },
    check: async (p, m, x) => poll(async () => (await boundsOf(p, x.ids[1]))![0][ax]).toBe(100),
    undo: async (p, m, x) => poll(() => boundsOf(p, x.ids[1])).toEqual(x.mid),
  })),

  // ---- Select ----------------------------------------------------------------------------
  { id: "sel.all", menu: "Select", label: "Select all", check: async (p) => {
    const n = await st(p, (s) => s.doc.entities.length + s.doc.brushes.filter((b: { entity: number | null }) => b.entity == null).length);
    await poll(async () => { const x = await sel(p); return x.brushes.length + x.entities.length; }).toBe(n);
  } },
  { id: "sel.none", menu: "Select", label: "Select none", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p) => poll(async () => (await sel(p)).brushes.length).toBe(0) },
  { id: "sel.invert", menu: "Select", label: "Invert selection", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p, m) => {
    await poll(async () => (await sel(p)).brushes.includes(m.b)).toBe(true);
    expect((await sel(p)).brushes).not.toContain(m.a);
  } },
  { id: "sel.texture", menu: "Select", label: "Select same texture", setup: async (p, m) => {
    await cmd(p, { op: "set_texture", faces: [[m.a, null], [m.b, null]], texture: "CRATE01" });
    await select(p, { brushes: [m.a] });
  }, check: async (p, m) => poll(async () => (await sel(p)).brushes).toEqual([m.a, m.b].sort((x, y) => x - y)) },
  { id: "sel.class", menu: "Select", label: "Select same type", setup: async (p, m) => select(p, { entities: [m.spawn] }), check: async (p) => {
    const n = await st(p, (s) => s.doc.entities.filter((e: { classname: string }) => e.classname === "info_player_start").length);
    await poll(async () => (await sel(p)).entities.length).toBe(n);
  } },
  { id: "sel.value", menu: "Select", label: "Select same name", setup: async (p, m) => {
    const other = (await cmd(p, { op: "add_entity", classname: "light", origin: [64, -128, 96], keys: [["targetname", "lamp1"]] }))!.entities[0];
    await select(p, { entities: [m.light] });
    return other;
  }, check: async (p, m, other) => poll(async () => (await sel(p)).entities).toEqual([m.light, other].sort((x, y) => x - y)) },
  { id: "sel.touching", menu: "Select", label: "Select touching", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p, m) => {
    // the box sits on the floor: the floor touches it, the far box doesn't
    await poll(async () => (await sel(p)).brushes.includes(m.floor)).toBe(true);
    expect((await sel(p)).brushes).not.toContain(m.b);
  } },
  { id: "sel.inside", menu: "Select", label: "Select inside", setup: async (p, m) => {
    const inner = (await cmd(p, { op: "add_box", min: [16, 16, 16], max: [32, 32, 32], texture: m.tex }))!.brushes[0];
    await select(p, { brushes: [m.a] });
    return inner;
  }, check: async (p, m, inner) => poll(async () => (await sel(p)).brushes).toContain(inner) },
  { id: "sel.goto", menu: "Select", label: "Go to brush or entity number…", check: async (p, m) => {
    const d = dialog(p, "Go to a number");
    await d.getByRole("button", { name: "Editor numbers" }).click();
    await d.getByRole("textbox").fill(`Brush ${m.b}`);
    await d.getByRole("button", { name: "Go" }).click();
    await poll(async () => (await sel(p)).brushes).toEqual([m.b]);
  } },
  { id: "sel.hide", menu: "Select", label: "Hide", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p) => poll(() => hidden(p)).toEqual({ brushes: 1, entities: 0 }) },
  { id: "sel.isolate", menu: "Select", label: "Hide everything else", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p) => {
    const n = await nBrushes(p);
    await poll(async () => (await hidden(p)).brushes).toBe(n - 1);
  } },
  { id: "sel.showAll", menu: "Select", label: "Show everything", setup: async (p, m) => {
    await select(p, { brushes: [m.a] });
    await p.evaluate(() => (window as any).__lsEditor.getState().set({ hidden: { brushes: new Set([1, 2]), entities: new Set([3]) } })); // eslint-disable-line @typescript-eslint/no-explicit-any
  }, check: async (p) => poll(() => hidden(p)).toEqual({ brushes: 0, entities: 0 }) },
  { id: "sel.hideEntities", menu: "Select", label: "Hide all point entities", check: async (p) => {
    const n = await st(p, (s) => s.doc.entities.filter((e: { brushes: number[] }) => !e.brushes.length).length);
    await poll(async () => (await hidden(p)).entities).toBe(n);
  } },
  { id: "sel.hideTriggers", menu: "Select", label: "Hide triggers and zones", check: async (p, m) => {
    await poll(() => st(p, (s) => [...s.hidden.entities])).toEqual([m.trig]);
  } },
  { id: "sel.hideWorld", menu: "Select", label: "Hide world brushes", check: async (p) => {
    const n = await st(p, (s) => s.doc.brushes.filter((b: { entity: number | null }) => b.entity == null).length);
    await poll(async () => (await hidden(p)).brushes).toBe(n);
  } },
  { id: "sel.lock", menu: "Select", label: "Lock", setup: async (p, m) => select(p, { brushes: [m.a] }), check: async (p) => {
    await poll(async () => (await sel(p)).brushes.length).toBe(0);
    await expect(p.locator(".toast, [role=status]").filter({ hasText: /Locked/ }).first()).toBeVisible();
  } },
  { id: "sel.unlockAll", menu: "Select", label: "Unlock everything", setup: async (p, m) => {
    await select(p, { brushes: [m.a] });
    await p.evaluate(() => (window as any).__covRun?.("sel.lock")); // eslint-disable-line @typescript-eslint/no-explicit-any
  }, check: async (p) => {
    // with nothing locked, Unlock everything is greyed out again
    await poll(() => p.evaluate(() => (window as any).__covEnabled?.("sel.unlockAll") ?? null)).toBe(false); // eslint-disable-line @typescript-eslint/no-explicit-any
  } },
  { id: "sel.groups", menu: "Select", label: "Clicks pick whole groups", setup: async (p) => p.evaluate(() => (window as any).__lsEditor.getState().set({ selectMode: "objects" })), check: async (p) => poll(() => st(p, (s) => s.selectMode)).toBe("groups") }, // eslint-disable-line @typescript-eslint/no-explicit-any
  { id: "sel.objects", menu: "Select", label: "Clicks pick single objects", check: async (p) => poll(() => st(p, (s) => s.selectMode)).toBe("objects") },

  // ---- View ------------------------------------------------------------------------------
  ...([["single", "Single", 1], ["quad", "Quad", 4], ["one3", "1+3", 4], ["two", "Two", 2]] as const).map(([id, label, n]): Case => ({
    id: `view.layout.${id}`, menu: "View", label: `Layout: ${label}`,
    setup: async (p) => p.evaluate((id) => {
      // start from another layout
      const other = id === "single" ? "quad" : "single";
      const b = [...document.querySelectorAll<HTMLButtonElement>('.ed-layouts button')];
      b[["single", "quad", "one3", "two"].indexOf(other)].click();
    }, id),
    check: async (p) => poll(() => paneCount(p)).toBe(n),
  })),
  { id: "view.maximise", menu: "View", label: "Maximize the pane / put it back", setup: async (p) => {
    await p.locator(".ed-layouts button").nth(1).click();
    await poll(() => paneCount(p)).toBe(4);
  }, check: async (p) => poll(() => paneCount(p)).toBe(1) },
  ...(["3d", "top", "front", "side"] as const).map((v): Case => ({
    id: `view.${v}`, menu: "View", label: `${v === "3d" ? "3D" : v[0].toUpperCase() + v.slice(1)} view`,
    setup: async (p) => p.evaluate((v) => (window as any).__lsEditor.getState().set({ view: v === "top" ? "side" : "top" }), v), // eslint-disable-line @typescript-eslint/no-explicit-any
    check: async (p) => poll(() => st(p, (s) => s.view)).toBe(v),
  })),
  { id: "view.frame", menu: "View", label: "Frame selection", setup: async (p, m) => {
    await select(p, { brushes: [m.b] });
    return p.locator(".ed-pane").first().screenshot();
  }, check: async (p, m, shot) => poll(async () => Buffer.compare(await p.locator(".ed-pane").first().screenshot(), shot) !== 0).toBe(true) },
  { id: "view.frameAll", menu: "View", label: "Frame the whole map", setup: async (p, m) => {
    await select(p, { brushes: [m.b] });
    await p.evaluate(() => (window as any).__covRun?.("view.frame")); // eslint-disable-line @typescript-eslint/no-explicit-any
    await p.waitForTimeout(800);
    await select(p, {});
    return p.locator(".ed-pane").first().screenshot();
  }, check: async (p, m, shot) => poll(async () => Buffer.compare(await p.locator(".ed-pane").first().screenshot(), shot) !== 0).toBe(true) },
  { id: "view.fly", menu: "View", label: "Fly / orbit camera", setup: async (p) => p.locator(".tools-status, .editor-status").first().textContent(), check: async (p, m, before) => {
    await poll(async () => (await p.locator(".tools-status, .editor-status").first().textContent()) !== before).toBe(true);
  } },
  { id: "view.gridDown", menu: "View", label: "Smaller grid", check: async (p) => poll(() => st(p, (s) => s.grid)).toBe(8) },
  { id: "view.gridUp", menu: "View", label: "Bigger grid", check: async (p) => poll(() => st(p, (s) => s.grid)).toBe(32) },
  { id: "view.gridShow", menu: "View", label: "Show / hide the grid", setup: async (p) => ls(p, "ls.editor.showGrid"), check: async (p, m, before) => poll(() => ls(p, "ls.editor.showGrid")).toBe(before === "0" ? "1" : "0") },
  { id: "view.gridSettings", menu: "View", label: "Grid and snapping…", check: async (p) => {
    const d = dialog(p, "Grid and snapping");
    await d.getByRole("combobox", { name: /Grid size/ }).selectOption("64");
    await poll(() => st(p, (s) => s.grid)).toBe(64);
    await d.getByRole("button", { name: "Done" }).click();
    await expect(d).toHaveCount(0);
  } },
  { id: "view.texLock", menu: "View", label: "Texture lock on / off", setup: async (p) => st(p, (s) => s.textureLock), check: async (p, m, b) => poll(() => st(p, (s) => s.textureLock)).toBe(!b) },
  { id: "view.frameTime", menu: "View", label: "Show frame time", check: async (p) => expect(p.locator(".ed-frametime").first()).toBeVisible() },
  { id: "view.snap", menu: "View", label: "Snap to grid on / off", check: async (p) => {
    await poll(() => st(p, (s) => !!s.snapOff)).toBe(true);
    await expect(p.locator(".editor-snap")).toHaveClass(/off/);
  } },
  { id: "view.geoSnap", menu: "View", label: "Snap to corners and edges on / off", setup: async (p) => ls(p, "ls.editor.geoSnap"), check: async (p, m, b) => poll(() => ls(p, "ls.editor.geoSnap")).toBe(b === "1" ? "0" : "1") },
  ...([["unreal", "Unreal"], ["blender", "Blender"], ["hammer", "Hammer"]] as const).map(([id, label]): Case => ({
    id: `view.nav.${id}`, menu: "View", label: `Move like ${label}`,
    setup: async (p) => p.evaluate((id) => localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: id === "blender" ? "unreal" : "blender", speed: 3 })), id),
    check: async (p) => poll(async () => JSON.parse((await ls(p, "ls.editor.nav")) ?? "{}").preset).toBe(id),
  })),
  { id: "view.speedUp", menu: "View", label: "Camera faster", hidden: true, check: async (p) => poll(async () => JSON.parse((await ls(p, "ls.editor.nav")) ?? "{}").speed).toBe(4) },
  { id: "view.speedDown", menu: "View", label: "Camera slower", hidden: true, check: async (p) => poll(async () => JSON.parse((await ls(p, "ls.editor.nav")) ?? "{}").speed).toBe(2) },
  { id: "view.models", menu: "View", label: "Models in place of entity boxes on / off", setup: async (p) => ls(p, "ls.editor.showModels"), check: async (p, m, b) => poll(() => ls(p, "ls.editor.showModels")).toBe(b === "0" ? "1" : "0") },
  { id: "view.paneMode", menu: "View", label: "Next drawing style for the active pane", check: async (p) => poll(() => p.getByRole("combobox", { name: /How pane 1 draws/ }).inputValue()).toBe("flat") },
  ...[["content", "Content"], ["inspect", "Inspect"], ["outline", "Outline"], ["textures", "Textures"], ["library", "Gameplay"], ["budget", "Budget"], ["io", "I/O"], ["checks", "Checks"], ["compile", "Compile"], ["project", "Project"], ["history", "History"]].map(([id, title]): Case => ({
    id: `panel.${id}`, menu: "View", label: `Panel: ${title}`, check: async (p) => tabShown(p, title),
  })),
  ...([["default", "Default (Unreal-like)"], ["hammer", "Hammer"], ["unreal", "Unreal-like"], ["tabs", "All on the right"]] as const).map(([id, label]): Case => ({
    id: `dock.${id}`, menu: "View", label: `Panel layout: ${label}`,
    setup: async (p) => p.evaluate(() => localStorage.removeItem("ls.editor.dock2")),
    check: async (p) => {
      await poll(async () => (await ls(p, "ls.editor.dock2")) != null).toBe(true);
      const want = { default: ["right", "outline"], hammer: ["right", "inspect"], unreal: null, tabs: null }[id];
      if (want) await poll(async () => JSON.parse((await ls(p, "ls.editor.dock2"))!).areas[want[0]].tabs[0]).toBe(want[1]);
    },
  })),
  { id: "dock.reset", menu: "View", label: "Reset the panel layout", setup: async (p) => {
    await p.evaluate(() => (window as any).__covRun?.("dock.tabs")); // eslint-disable-line @typescript-eslint/no-explicit-any
  }, check: async (p) => poll(async () => JSON.parse((await ls(p, "ls.editor.dock2"))!).areas.right.tabs[0]).toBe("outline") },
  { id: "view.content", menu: "View", label: "Content browser", check: async (p) => tabShown(p, "Content") },
  { id: "view.contentDrawer", menu: "View", label: "Content drawer", check: async (p) => expect(p.getByRole("region", { name: "Content drawer" }).or(p.locator('[aria-label="Content drawer"]')).first()).toBeVisible() },
  { id: "view.budget", menu: "View", label: "Budget", check: async (p) => tabShown(p, "Budget") },
  { id: "view.io", menu: "View", label: "I/O: what triggers what", check: async (p) => tabShown(p, "I/O") },

  // ---- Tools -----------------------------------------------------------------------------
  ...["select", "move", "rotate", "scale", "vertex", "box", "shape", "clip", "entity", "texture", "measure"].map((t): Case => ({
    id: `tool.${t}`, menu: "Tools", label: `${t[0].toUpperCase() + t.slice(1)} tool`,
    setup: async (p) => p.evaluate((t) => (window as any).__lsEditor.getState().set({ tool: t === "select" ? "box" : "select" }), t), // eslint-disable-line @typescript-eslint/no-explicit-any
    check: async (p) => {
      await poll(() => st(p, (s) => s.tool)).toBe(t);
      await expect(p.locator(".editor-tool[aria-pressed=true]")).toContainText(t[0].toUpperCase() + t.slice(1));
    },
  })),

  // ---- Map -------------------------------------------------------------------------------
  { id: "map.props", menu: "Map", label: "Map properties…", check: async (p) => {
    const d = dialog(p, "Map properties");
    await d.getByRole("spinbutton", { name: /Base light/ }).fill("40");
    await d.getByRole("button", { name: "Save properties" }).click();
    await poll(() => st(p, (s) => s.doc.undo)).toBeTruthy();
    await poll(() => p.evaluate(async () => (await fetch("/api/tools/editor/doc").then((r) => r.json())).world?.keys ?? null)).toBeTruthy();
  } },
  { id: "map.report", menu: "Map", label: "Entity report…", check: async (p, m) => {
    const d = dialog(p, "Entity report");
    await d.getByRole("searchbox", { name: "Find" }).fill("lamp1");
    // (the light, and the trigger that targets it)
    await expect(d.getByRole("row")).toHaveCount(3);
    await d.getByRole("checkbox", { name: "Pick light" }).check();
    await d.getByRole("button", { name: /^Pick( \d+)?$/ }).click();
    await poll(async () => (await sel(p)).entities).toEqual([m.light]);
  } },
  { id: "map.info", menu: "Map", label: "Map information…", check: async (p) => {
    const d = dialog(p, "Map information");
    const n = await nBrushes(p);
    await expect(d.getByRole("definition").first()).toContainText(String(n));
    await p.keyboard.press("Escape");
  } },
  { id: "map.checks", menu: "Map", label: "Check the map", check: async (p) => tabShown(p, "Checks") },
  { id: "map.compile", menu: "Map", label: "Compile and play…", check: async (p) => tabShown(p, "Compile") },
  { id: "map.outline", menu: "Map", label: "Outline", check: async (p) => tabShown(p, "Outline") },
  { id: "map.textures", menu: "Map", label: "Textures", check: async (p) => tabShown(p, "Textures") },
  { id: "map.inspect", menu: "Map", label: "Properties of what's picked", setup: async (p, m) => {
    await p.evaluate(() => (window as any).__covRun?.("panel.history")); // eslint-disable-line @typescript-eslint/no-explicit-any
    await select(p, { entities: [m.light] });
  }, check: async (p) => {
    await tabShown(p, "Inspect");
    await expect(p.getByRole("heading", { name: "light", exact: true })).toBeVisible();
  } },

  // ---- Help ------------------------------------------------------------------------------
  { id: "help.palette", menu: "Help", label: "Find a command…", check: async (p) => {
    const d = dialog(p, "Find a command");
    await d.getByRole("searchbox").fill("bigger grid");
    await p.keyboard.press("Enter");
    await poll(() => st(p, (s) => s.grid)).toBe(32);
  } },
  { id: "help.cheats", menu: "Help", label: "Keys cheat sheet", check: async (p) => {
    await expect(dialog(p, "Keys")).toContainText("Key set: Loner-Strike");
    await p.keyboard.press("Escape");
  } },
  { id: "help.coords", menu: "Help", label: "Coordinates and axes", check: async (p) => closeDialog(p, "Coordinates and axes") },
  { id: "help.tour", menu: "Help", label: "First map tour", check: async (p) => expect(p.getByRole("region", { name: "First map tour" }).or(p.locator('[aria-label="First map tour"]')).first()).toBeVisible() },
  { id: "help.keys", menu: "Help", label: "Change keys…", check: async (p) => {
    const d = dialog(p, "Change keys");
    await d.getByRole("button", { name: "TrenchBroom-style" }).click();
    await poll(async () => JSON.parse((await ls(p, "ls.editor.keys")) ?? "{}").preset).toBe("trenchbroom");
    await d.getByRole("button", { name: "Loner-Strike" }).click();
    await poll(async () => JSON.parse((await ls(p, "ls.editor.keys")) ?? "{}").preset).toBe("loner");
    await p.keyboard.press("Escape");
  } },
];

/** Cases by command id. */
export const caseById = new Map(CASES.map((c) => [c.id, c]));
