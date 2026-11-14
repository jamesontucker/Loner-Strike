// Editor coverage sweep: every command in the menus (and the palette-only ones), run once
// from where a person finds it, its effect checked, and Undo checked for document edits.
// Cases: editor-coverage.cases.ts. Notes and results: docs/notes/EditorCoverage.md.
import { expect, test } from "@playwright/test";
import { CASES } from "./editor-coverage.cases";
import { consoleErrors, menu, openEditor, palette, st, testMap, undo } from "./editor-coverage.helpers";

/** Commands tested in other coverage specs (files, compile, publish, leaks). */
const ELSEWHERE: Record<string, string> = {
  "file.open": "editor-coverage-file", "file.import": "editor-coverage-file", "file.importOther": "editor-coverage-file",
  "file.save": "editor-coverage-file", "file.saveAs": "editor-coverage-file", "file.export": "editor-coverage-file",
  "map.publish": "editor-coverage-map", "map.playtest": "editor-coverage-map", "map.goToLeak": "editor-coverage-map",
  "map.loadPointfile": "editor-coverage-map", "map.leakLine": "editor-coverage-map",
};


test("every command in the registry has a coverage case", async ({ page }) => {
  await openEditor(page);
  const all = await page.evaluate(() => (window as unknown as { __covCommands: () => { id: string }[] }).__covCommands());
  const have = new Set([...CASES.map((c) => c.id), ...Object.keys(ELSEWHERE)]);
  const missing = all.map((c) => c.id).filter((id) => !have.has(id));
  expect(missing, "commands with no coverage case: add one to editor-coverage.cases.ts").toEqual([]);
  // and every case is still a command (a renamed id would test nothing)
  const ids = new Set(all.map((c) => c.id));
  expect(CASES.map((c) => c.id).filter((id) => !ids.has(id))).toEqual([]);
  // labels and menus as the cases have them
  for (const c of CASES) {
    const real = all.find((x) => x.id === c.id) as { label: string; menu: string; hidden: boolean } | undefined;
    expect(real && { label: real.label, menu: real.menu, hidden: real.hidden }, c.id).toEqual({ label: c.label, menu: c.menu, hidden: !!c.hidden });
  }
});

for (const c of CASES) {
  test(`${c.hidden ? "palette" : c.menu} › ${c.label} (${c.id})`, async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]).catch(() => {});
    const errors = consoleErrors(page);
    await openEditor(page);
    const m = await testMap(page);
    const ctx = c.setup ? await c.setup(page, m) : undefined;
    if (c.hidden) await palette(page, c.label);
    else await menu(page, c.menu, c.label);
    await c.check(page, m, ctx);
    if (c.undo) {
      await expect.poll(() => st(page, (s) => !!s.doc.undo)).toBe(true);
      await undo(page);
      await c.undo(page, m, ctx);
    }
    expect(errors, "console errors").toEqual([]);
  });
}
