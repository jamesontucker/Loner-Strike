// Editor coverage sweep: TrenchBroom .map files (app/src-tauri/src/editor/mapfile.rs).
// Fixtures in tests/fixtures/trenchbroom (make_fixtures.py writes them): Valve 220 with layers
// (one hidden and locked, one left out of exports), nested groups, linked groups, entities in
// layers and groups; the Quake "standard" format; Quake 2 extras on face lines.
// Import, what comes over, Export, and Export read back. The clean compile of each export is in
// docs/notes/EditorCoverage.md (it runs SDHLT under CrossOver, outside the browser).
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { clearSaved, counts, fixture, menu, openEditor, PICK, pickFile, readSaved, SAVE, st } from "./editor-coverage.helpers";

test.skip(!PICK || !SAVE || !PICK.endsWith(".map"), "needs LS_COV_PICK (ending .map) and LS_COV_SAVE (the dev server's LS_DEV_PICK / LS_DEV_PICK_SAVE)");

const TB = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "trenchbroom");
type Group = { id: number; name: string; parent: number | null; hidden: boolean; locked: boolean };

async function importMap(page: Page, file: string) {
  pickFile(file);
  const before = await st(page, (s) => s.doc.docId);
  await menu(page, "File", "Import a .map…");
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  // a new document (named after the picked path, the dev server's symlink)
  await expect.poll(() => st(page, (s) => s.doc.docId), { timeout: 20_000 }).not.toBe(before);
  await expect.poll(() => st(page, (s) => !s.fileBusy && s.working === 0)).toBe(true);
}
async function exportMap(page: Page) {
  clearSaved();
  await menu(page, "File", "Export a .map…");
  await expect.poll(() => readSaved()?.length ?? 0).toBeGreaterThan(100);
  return readSaved()!.toString("latin1");
}
const groups = (page: Page) => st(page, (s) => s.doc.groups as Group[]);
const brushBounds = (page: Page) => st(page, (s) => s.doc.brushes.map((b: { min: number[]; max: number[] }) => [...b.min, ...b.max].join(" ")).sort());

/** Planes and textures of every brush face in a .map, normalized (for comparing two files). */
function faces(text: string): string[] {
  const out: string[] = [];
  const re = /^\s*\(\s*([-\d.e]+) ([-\d.e]+) ([-\d.e]+)\s*\)\s*\(\s*([-\d.e]+) ([-\d.e]+) ([-\d.e]+)\s*\)\s*\(\s*([-\d.e]+) ([-\d.e]+) ([-\d.e]+)\s*\)\s*(\S+)/gm;
  for (const m of text.matchAll(re)) {
    const p = m.slice(1, 10).map(Number);
    const a = [p[0] - p[3], p[1] - p[4], p[2] - p[5]];
    const b = [p[6] - p[3], p[7] - p[4], p[8] - p[5]];
    const n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    const len = Math.hypot(...n);
    const u = n.map((x) => Math.round((x / len) * 1000) / 1000 + 0);
    const d = Math.round((u[0] * p[3] + u[1] * p[4] + u[2] * p[5]) * 100) / 100 + 0;
    out.push(`${u.join(",")}|${d}|${m[10].toUpperCase()}`);
  }
  return out.sort();
}
/** The entities of a .map: classname plus keys (TrenchBroom's own keys left out). */
function entities(text: string): string[] {
  const out: string[] = [];
  for (const block of text.split(/\n\{\n/).slice(1)) {
    const keys = [...block.matchAll(/^"([^"]+)" "([^"]*)"$/gm)].map((m) => [m[1], m[2]]).filter(([k]) => !k.startsWith("_tb_") && k !== "mapversion" && k !== "wad");
    const cls = keys.find(([k]) => k === "classname")?.[1];
    if (!cls || cls === "worldspawn" || cls === "func_group") continue;
    out.push(keys.map(([k, v]) => `${k}=${v}`).sort().join(";"));
  }
  return out.sort();
}

test("Valve 220 with layers and nested groups: everything comes over, in its groups", async ({ page }) => {
  await openEditor(page);
  await importMap(page, join(TB, "tb_valve_layers.map"));
  // 6 room brushes, the crate in the Details layer, Crates + Inner, Notes, two Pillars, the door
  await expect.poll(() => counts(page)).toEqual({ brushes: 13, entities: 4, groups: 6 });
  const g = await groups(page);
  const by = (n: string) => g.filter((x) => x.name === n);
  const details = by("Details")[0];
  const crates = by("Crates")[0];
  const inner = by("Inner")[0];
  expect(crates.parent).toBe(details.id);
  expect(inner.parent).toBe(crates.id);
  expect(by("Pillar")).toHaveLength(2);
  const ents = await st(page, (s) => s.doc.entities.map((e: { classname: string; group: number | null; keys: [string, string][] }) => ({ c: e.classname, g: e.group, tb: e.keys.filter(([k]) => k.startsWith("_tb_")).length })));
  expect(ents.find((e) => e.c === "light")!.g).toBe(details.id);
  expect(ents.find((e) => e.c === "info_player_deathmatch")!.g).toBe(inner.id);
  expect(ents.find((e) => e.c === "func_door")!.g).toBe(crates.id);
  expect(ents.every((e) => e.tb === 0)).toBe(true);
  // geometry: the crate and the nested one where TrenchBroom had them
  const b = await brushBounds(page);
  expect(b).toContain("100 100 0 164 164 64");
  expect(b).toContain("100 100 64 132 132 96");
  expect(b).toContain("180 -64 0 196 64 128");
});

test("a hidden, locked TrenchBroom layer comes in hidden and locked", async ({ page }) => {
  // TrenchBroom writes "_tb_layer_hidden" "1" and "_tb_layer_locked" "1" on the layer
  await openEditor(page);
  await importMap(page, join(TB, "tb_valve_layers.map"));
  const details = (await groups(page)).find((x) => x.name === "Details")!;
  expect({ hidden: details.hidden, locked: details.locked }).toEqual({ hidden: true, locked: true });
});

test("a layer TrenchBroom leaves out of exports isn't exported or compiled", async ({ page }) => {
  // "_tb_layer_omit_from_export" "1": TrenchBroom's own export and compile leave that layer out
  await openEditor(page);
  await importMap(page, join(TB, "tb_valve_layers.map"));
  const text = await exportMap(page);
  expect(faces(text).filter((f) => f.endsWith("|AAATRIGGER"))).toEqual([]);
});

test("Export keeps every brush face and entity; groups survive a trip back to TrenchBroom", async ({ page }) => {
  await openEditor(page);
  const src = readFileSync(join(TB, "tb_valve_layers.map"), "latin1");
  await importMap(page, join(TB, "tb_valve_layers.map"));
  const out = await exportMap(page);
  expect(out).toContain('"mapversion" "220"');
  // every face, plane for plane and texture for texture (the omit-from-export layer aside)
  const notOmitted = (f: string[]) => f.filter((x) => !x.endsWith("|AAATRIGGER"));
  expect(notOmitted(faces(out))).toEqual(notOmitted(faces(src)));
  expect(entities(out)).toEqual(entities(src));
  // and read back: the same brushes and entities
  await importMap(page, fixture("tb_valve_layers_roundtrip.map", out));
  await expect.poll(() => counts(page).then((c) => c.entities)).toBe(4);
  expect((await counts(page)).brushes).toBeGreaterThanOrEqual(12);
  // the groups and layers too (so a map can go back to TrenchBroom without losing them)
  expect((await counts(page)).groups).toBe(6);
});

test("TrenchBroom's worldspawn keys (_tb_def) aren't passed on to the compiler", async ({ page }) => {
  await openEditor(page);
  await importMap(page, join(TB, "tb_valve_layers.map"));
  const out = await exportMap(page);
  expect(out).not.toMatch(/"_tb_/);
});

for (const [file, n] of [["tb_standard.map", 7], ["tb_standard_q2.map", 7], ["tb_valve_q2extras.map", 6]] as const) {
  test(`${file}: imports, and exports as the same planes in Valve 220`, async ({ page }) => {
    await openEditor(page);
    const src = readFileSync(join(TB, file), "latin1");
    await importMap(page, join(TB, file));
    await expect.poll(() => counts(page).then((c) => c.brushes)).toBe(n);
    expect(await brushBounds(page)).toContain("-272 -272 -16 272 272 0");
    const out = await exportMap(page);
    expect(faces(out)).toEqual(faces(src));
    expect(entities(out)).toEqual(entities(src));
    // standard format faces get Valve axes: the floor's top face maps u to x, v to -y
    expect(out).toMatch(/\( -272 272 0 \) \( 272 272 0 \) \( 272 -272 0 \) OUT_WALL5 \[ 1 0 0 0 \] \[ 0 -1 0 0 \] 0 1 1|OUT_WALL5 \[ 1 0 0 -?0 \] \[ 0 -1 0 -?0 \]/);
  });
}

test("the standard-format and Q2-extras imports match the plain standard one", async ({ page }) => {
  await openEditor(page);
  await importMap(page, join(TB, "tb_standard.map"));
  const plain = await exportMap(page);
  await importMap(page, join(TB, "tb_standard_q2.map"));
  const q2 = await exportMap(page);
  expect(faces(q2)).toEqual(faces(plain));
  void (await st(page, (s) => s.doc.name));
});
