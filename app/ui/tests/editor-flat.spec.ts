// Map editor, the Top/Front/Side views (Parity A): the selection box's handles stretch the
// picked brush with its edge on the grid; dragging a picked brush moves it in the view's
// plane; a box from empty space picks; the Box tool draws a brush; the wheel zooms to the
// cursor. Runs on editor-dev.html against the tools dev server
// (npm run test:tools -- tests/editor-flat.spec.ts; WebKit: -c tests/webkit.config.ts after adding it there).
import { expect, test, type Page } from "@playwright/test";

// (pixel drags on the panes: at the "small" project's 1024x680 the Content dock leaves 2D panes about
// 120 px tall, too small for these gestures; layouts at that size are covered by editor-batch/tools)
test.use({ screenshot: "only-on-failure", viewport: { width: 1440, height: 900 } });

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

type Pt = { x: number; y: number };
const handles = (page: Page, view: string) =>
  page.evaluate((v) => (window as unknown as { __lsFlatHandles?: Record<string, () => Pt[]> }).__lsFlatHandles?.[v]?.() ?? null, view);
/** The handles once the view has put them up (the view registers them in an effect, which runs
 *  just after the commit that shows the selection: reading at once can come too early). */
async function ready(page: Page, view: string): Promise<Pt[]> {
  await expect.poll(() => handles(page, view), { timeout: 10_000 }).not.toBeNull();
  return (await handles(page, view))!;
}

async function openQuadWithBox(page: Page) {
  const errors: string[] = [];
  // (WebKit reports fetches cut off by the reload below as "access control checks": not errors)
  page.on("pageerror", (e) => !/access control checks/.test(String(e)) && errors.push(String(e)));
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  // just one box (the new map's room would be under every click in the flat views)
  const d0 = await api(page, "/api/tools/editor/doc");
  await api(page, "/api/tools/editor/cmd", { op: "delete", brushes: d0.brushes.map((x: { id: number }) => x.id), entities: d0.entities.map((x: { id: number }) => x.id) });
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-64, -64, 0], max: [64, 64, 128], texture: "CRATE01" });
  const id = r.created?.brushes?.[0] ?? r.doc?.brushes?.at(-1)?.id;
  expect(id, JSON.stringify(r).slice(0, 300)).toBeTruthy();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(2500);
  return { id: id as number, errors };
}

const brush = async (page: Page, id: number) => {
  const b = (await api(page, "/api/tools/editor/doc")).brushes.find((x: { id: number }) => x.id === id);
  return { min: b?.min as number[], max: b?.max as number[] };
};

const pane = (page: Page, slot: number) => page.locator(`.ed-pane[data-pane="${slot}"]`);

test("Top view: handles stretch on the grid, a drag moves, a box picks, Box draws, the wheel zooms to the cursor", async ({ page }) => {
  const { id, errors } = await openQuadWithBox(page);
  const top = (await pane(page, 1).boundingBox())!;
  const mid = { x: top.x + top.width / 2, y: top.y + top.height / 2 };
  // the map is fitted to the pane: the box is in the middle; a click picks it
  await page.mouse.click(mid.x, mid.y);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked · 128 × 128 × 128");
  await expect.poll(() => handles(page, "top")).not.toBeNull();
  const h = await ready(page, "top");
  expect(h.length).toBe(8);

  // the right-middle handle, 40 px to the right: the east side lands on the grid, the west side stays
  await page.mouse.move(h[3].x, h[3].y);
  await page.mouse.down();
  await page.mouse.move(h[3].x + 20, h[3].y, { steps: 4 });
  await page.mouse.move(h[3].x + 40, h[3].y, { steps: 4 });
  await page.mouse.up();
  await expect.poll(async () => (await brush(page, id)).max[0]).toBeGreaterThan(64);
  let b = await brush(page, id);
  expect(b.min).toEqual([-64, -64, 0]);
  expect(b.max[0] % 16, `east side ${b.max[0]} on the 16 grid`).toBe(0);
  expect(b.max.slice(1)).toEqual([64, 128]);
  expect((await api(page, "/api/tools/editor/doc")).undo).toBe("Scale");

  // a corner: both sides at once
  // (the view shows the stretched box a moment after the document has it)
  await expect.poll(async () => JSON.stringify(await handles(page, "top"))).not.toBe(JSON.stringify(h));
  const h2 = await ready(page, "top");
  await page.mouse.move(h2[0].x, h2[0].y);
  await page.mouse.down();
  await page.mouse.move(h2[0].x - 30, h2[0].y + 30, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await brush(page, id)).min[0]).toBeLessThan(-64);
  b = await brush(page, id);
  expect(b.min[1]).toBeLessThan(-64);
  expect([b.min[0] % 16, b.min[1] % 16].map(Math.abs)).toEqual([0, 0]);

  // dragging the picked brush moves it in the view's plane, on the grid; height untouched
  const before = await brush(page, id);
  await expect.poll(async () => JSON.stringify(await handles(page, "top"))).not.toBe(JSON.stringify(h2));
  const c = await ready(page, "top");
  const inside = { x: (c[0].x + c[4].x) / 2, y: (c[0].y + c[4].y) / 2 };
  await page.mouse.move(inside.x, inside.y);
  await page.mouse.down();
  await page.mouse.move(inside.x + 30, inside.y - 30, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await brush(page, id)).min[0]).not.toBe(before.min[0]);
  b = await brush(page, id);
  expect(b.min[0] - before.min[0]).toBeGreaterThan(0);
  expect(b.min[1] - before.min[1]).toBeGreaterThan(0); // up on screen is north (+y)
  expect([b.min[2], b.max[2]]).toEqual([0, 128]);
  expect((b.min[0] - before.min[0]) % 16).toBe(0);
  expect((await api(page, "/api/tools/editor/doc")).undo).toBe("Move");

  // a click in empty space drops the pick; a box round the brush picks it again
  await page.mouse.click(top.x + 12, top.y + top.height - 12);
  await expect(page.locator(".ed-selsize")).toHaveCount(0);
  // (from the bottom right up to the top left: a box that picks what it touches, clear of the
  // pane's own buttons and the axes triad in its corner)
  await page.mouse.move(top.x + top.width - 100, top.y + top.height - 12);
  await page.mouse.down();
  await page.mouse.move(top.x + 20, top.y + 50, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator(".ed-selsize")).toContainText("1 picked");

  // the wheel zooms to the cursor: the handle under the mouse stays put
  const z0 = await ready(page, "top");
  await page.mouse.move(z0[4].x, z0[4].y);
  await page.mouse.wheel(0, -300);
  await page.waitForTimeout(300);
  const z1 = await ready(page, "top");
  expect(Math.abs(z1[4].x - z0[4].x)).toBeLessThan(3);
  expect(Math.abs(z1[4].y - z0[4].y)).toBeLessThan(3);
  expect(Math.abs(z1[0].x - z1[4].x)).toBeGreaterThan(Math.abs(z0[0].x - z0[4].x)); // and it did zoom in

  // Box tool: draw a brush in empty space of the Front view
  const n0 = (await api(page, "/api/tools/editor/doc")).brushes.length;
  await page.locator('.editor-tool[data-cmd="tool.box"]').click();
  const fr = (await pane(page, 2).boundingBox())!;
  // (the bottom left: clear of the box, the pane's buttons and the axes triad)
  await expect(page.locator('.editor-tool[data-cmd="tool.box"]')).toHaveAttribute("aria-pressed", "true");
  await page.mouse.move(fr.x + 30, fr.y + fr.height - 30, { steps: 5 });
  await page.waitForTimeout(100);
  await page.mouse.down();
  await page.mouse.move(fr.x + 80, fr.y + fr.height - 80, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).brushes.length).toBe(n0 + 1);
  const made = (await api(page, "/api/tools/editor/doc")).brushes.at(-1);
  for (const x of [...made.min, ...made.max]) expect(Number.isInteger(x / 16), `${x} on the grid`).toBe(true);
  await page.screenshot({ path: "test-results/editor-flat-quad.png" });
  expect(errors).toEqual([]);
});

test("Front and Side views have handles on the right axes", async ({ page }) => {
  const { id, errors } = await openQuadWithBox(page);
  const top = (await pane(page, 1).boundingBox())!;
  await page.mouse.click(top.x + top.width / 2, top.y + top.height / 2);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked");
  // Front: the top-middle handle pulls the top (z) up
  const side0 = JSON.stringify(await ready(page, "side"));
  const f = await ready(page, "front");
  await page.mouse.move(f[5].x, f[5].y);
  await page.mouse.down();
  await page.mouse.move(f[5].x, f[5].y - 40, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await brush(page, id)).max[2]).toBeGreaterThan(128);
  // Side: the right-middle handle pulls north (+y), once the side view shows the taller box
  await expect.poll(async () => JSON.stringify(await handles(page, "side"))).not.toBe(side0);
  const s = await ready(page, "side");
  await page.mouse.move(s[3].x, s[3].y);
  await page.mouse.down();
  await page.mouse.move(s[3].x + 40, s[3].y, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await brush(page, id)).max[1]).toBeGreaterThan(64);
  const b = await brush(page, id);
  expect(b.min).toEqual([-64, -64, 0]);
  expect(b.max[0]).toBe(64);
  expect(errors).toEqual([]);
});

test("⌘/Ctrl-drag an edge's middle in the Front view shears the box into a slanted wall", async ({ page }) => {
  const MOD = process.platform === "darwin" ? "Meta" : "Control";
  const { id, errors } = await openQuadWithBox(page);
  const top = (await pane(page, 1).boundingBox())!;
  await page.mouse.move(top.x + top.width / 2, top.y + top.height / 2, { steps: 3 });
  await page.mouse.click(top.x + top.width / 2, top.y + top.height / 2);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked · 128 × 128 × 128");
  // Front view: the top edge's middle, 40 px east with ⌘/Ctrl: the top slides, the floor stays
  const f = await ready(page, "front");
  await page.mouse.move(f[5].x, f[5].y, { steps: 3 });
  await page.keyboard.down(MOD);
  await page.mouse.down();
  await page.mouse.move(f[5].x + 20, f[5].y, { steps: 4 });
  await page.mouse.move(f[5].x + 40, f[5].y, { steps: 4 });
  await page.mouse.up();
  await page.keyboard.up(MOD);
  await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).undo).toBe("Shear");
  const b = await brush(page, id);
  expect(b.min).toEqual([-64, -64, 0]);
  expect(b.max[0]).toBeGreaterThan(64);
  expect(b.max[0] % 16).toBe(0);
  expect(b.max.slice(1)).toEqual([64, 128]);
  // the box's volume is unchanged: it leans, it doesn't grow (corners at the floor stay)
  const d = await api(page, "/api/tools/editor/doc");
  expect(d.brushes.length).toBe(1);
  expect(errors).toEqual([]);
});
