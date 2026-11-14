// Map editor: leak tracing (P5, Hammer parity item 1). A room whose east wall stops short of the
// ceiling, a leak line through the gap (as SDHLT writes it, loaded like Map > Load pointfile…):
// the bar over the views says where the gap is, Go to leak flies there and picks the brush beside
// the hole, Pick brushes at the hole picks them all, the Map menu has the commands, and Forget it
// clears it. Runs on editor-dev.html against the tools dev server (test builds take the
// pointfile's path in the request instead of the file dialog). P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

const ROOM: [number[], number[]][] = [
  [[0, 0, 0], [512, 512, 16]], // floor
  [[0, 0, 240], [512, 512, 256]], // ceiling
  [[0, 0, 16], [16, 512, 240]], // west
  [[496, 0, 16], [512, 512, 200]], // east, short: the gap
  [[16, 0, 16], [496, 16, 240]], // south
  [[16, 496, 16], [496, 512, 240]], // north
];

async function open(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const doc = await api(page, "/api/tools/editor/doc");
  await api(page, "/api/tools/editor/cmd", { op: "delete", brushes: doc.brushes.map((b: { id: number }) => b.id), entities: doc.entities.map((e: { id: number }) => e.id) });
  const ids: number[] = [];
  for (const [min, max] of ROOM) ids.push((await api(page, "/api/tools/editor/cmd", { op: "add_box", min, max, texture: "CRATE01" })).created.brushes[0]);
  await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "info_player_start", origin: [256, 256, 64] });
  // the compiler's .lin: segments "x y z - x y z", from the spawn out through the gap
  const dir = mkdtempSync(join(tmpdir(), "ls-leak-"));
  const lin = join(dir, "ls_leak_room.lin");
  writeFileSync(lin, "256 256 64 - 400 256 210\n400 256 210 - 504 256 220\n504 256 220 - 600 256 220\n");
  const r = await api(page, "/api/tools/editor/leak/load", { path: lin });
  expect(r.leaked, JSON.stringify(r)).toBe(true);
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  return { ceiling: ids[1], east: ids[3] };
}

test.afterEach(async ({ page }) => {
  await api(page, "/api/tools/editor/leak/clear", {}).catch(() => {});
});

test("the leak bar, Go to leak, the brushes at the hole, the Map menu and Forget it", async ({ page }) => {
  const { ceiling, east } = await open(page);
  const bar = page.getByRole("alert", { name: "The map leaks" });
  await expect(bar).toBeVisible({ timeout: 10000 });
  await expect(bar).toContainText("from info_player_start to the gap");
  await expect(bar).toContainText(new RegExp(`next to brush #(${ceiling}|${east})`));
  await expect(bar).toContainText("ls_leak_room.lin");
  // Go to leak: the brush beside the hole is picked (the flight runs on its own)
  await bar.getByRole("button", { name: "Go to leak" }).click();
  await expect(page.locator(".ed-selsize")).toContainText("1 picked");
  // all of them
  await bar.getByRole("button", { name: "Pick brushes at the hole" }).click();
  await expect(page.locator(".ed-selsize")).toContainText(/[2-4] picked/);
  // hide and show the line
  await bar.getByRole("button", { name: "Hide line" }).click();
  await expect(bar.getByRole("button", { name: "Show line" })).toBeVisible();
  await bar.getByRole("button", { name: "Show line" }).click();
  // the Map menu has the commands
  await page.getByRole("menuitem", { name: "Map" }).click();
  await expect(page.getByRole("menuitem", { name: /Go to leak/ })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Load pointfile/ })).toBeVisible();
  await page.keyboard.press("Escape");
  // the app's answer for agents and the page: the hole and the brushes round it
  const l = await api(page, "/api/tools/editor/leak");
  expect(l.hole.at).toEqual([504, 256, 220]);
  expect(l.nearest.slice(0, 2).map((n: { brush: number }) => n.brush).sort()).toEqual([ceiling, east].sort());
  // Forget it
  await bar.getByRole("button", { name: "Forget it" }).click();
  await expect(bar).toHaveCount(0);
  expect((await api(page, "/api/tools/editor/leak")).leaked).toBe(false);
});

test("a file that isn't a leak line is refused", async ({ page }) => {
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const dir = mkdtempSync(join(tmpdir(), "ls-leak-"));
  const bad = join(dir, "notes.txt");
  writeFileSync(bad, "hello");
  expect((await api(page, "/api/tools/editor/leak/load", { path: bad })).error).toContain(".lin or .pts");
  const empty = join(dir, "empty.lin");
  writeFileSync(empty, "nothing here\n");
  expect((await api(page, "/api/tools/editor/leak/load", { path: empty })).error).toContain("no leak line");
});
