// Map editor, TrenchBroom-style brush editing (Parity A): Shift-drag a face of a picked brush
// in 3D pulls out a new brush (one undo step, the new brush picked, the source unchanged);
// in the Vertex tool, ⌘/Ctrl-drag an edge's middle splits it (a new corner, still convex) and
// Merge corners joins picked corners. Runs on editor-dev.html against the tools dev server
// (npm run test:tools -- tests/editor-extrude.spec.ts).
import { expect, test, type Page } from "@playwright/test";

// (pixel drags on the panes: at the "small" project's 1024x680 the Content dock leaves 2D panes about
// 120 px tall, too small for these gestures; layouts at that size are covered by editor-batch/tools)
test.use({ screenshot: "only-on-failure", viewport: { width: 1440, height: 900 } });

const MOD = process.platform === "darwin" ? "Meta" : "Control";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

type B = { id: number; min: number[]; max: number[] };
const doc = (page: Page) => api(page, "/api/tools/editor/doc") as Promise<{ brushes: B[]; undo: string | null }>;

async function openWithBox(page: Page, layout: "single" | "quad") {
  const errors: string[] = [];
  page.on("pageerror", (e) => !/access control checks/.test(String(e)) && errors.push(String(e)));
  await page.addInitScript((l) => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: l, views: ["3d", "top", "front", "side"] }));
  }, layout);
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const d0 = await doc(page);
  await api(page, "/api/tools/editor/cmd", { op: "delete", brushes: d0.brushes.map((x) => x.id), entities: ((await api(page, "/api/tools/editor/doc")).entities as { id: number }[]).map((x) => x.id) });
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-64, -64, 0], max: [64, 64, 128], texture: "CRATE01" });
  const id = r.created.brushes[0] as number;
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(2500);
  return { id, errors };
}

test("Shift-drag a face in 3D pulls out a new brush", async ({ page }) => {
  const { id, errors } = await openWithBox(page, "single");
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  const mid = { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  await page.mouse.move(mid.x, mid.y, { steps: 3 });
  await page.mouse.click(mid.x, mid.y);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked · 128 × 128 × 128");
  const n0 = (await doc(page)).brushes.length;
  // on the box's left face (the middle of the view is its front corner edge), pulled out to the left
  const on = { x: mid.x - 70, y: mid.y };
  await page.mouse.move(on.x, on.y, { steps: 3 });
  await page.keyboard.down("Shift");
  await page.mouse.down();
  await page.mouse.move(on.x - 20, on.y, { steps: 5 });
  await page.mouse.move(on.x - 45, on.y + 5, { steps: 8 });
  await page.mouse.up();
  await page.keyboard.up("Shift");
  await expect.poll(async () => (await doc(page)).brushes.length).toBe(n0 + 1);
  const d = await doc(page);
  expect(d.undo).toBe("Extrude");
  const src = d.brushes.find((b) => b.id === id)!;
  expect([src.min, src.max]).toEqual([
    [-64, -64, 0],
    [64, 64, 128],
  ]);
  const made = d.brushes.find((b) => b.id !== id)!;
  // the new brush sits against one face of the box, on the grid, as wide as that face
  const touching = [0, 1, 2].filter((i) => made.min[i] === src.max[i] || made.max[i] === src.min[i]);
  expect(touching.length).toBe(1);
  const ax = touching[0];
  for (const i of [0, 1, 2].filter((i) => i !== ax)) expect([made.min[i], made.max[i]]).toEqual([src.min[i], src.max[i]]);
  for (const x of [...made.min, ...made.max]) expect(Math.abs(x % 16), `${x} on the grid`).toBe(0);
  // the new brush is the one picked
  const size = [0, 1, 2].map((i) => made.max[i] - made.min[i]).join(" × ");
  expect(size).not.toBe("128 × 128 × 128");
  await expect(page.locator(".ed-selsize")).toContainText(`1 picked · ${size}`);
  // one undo takes it away
  await page.keyboard.press(process.platform === "darwin" ? "Meta+z" : "Control+z");
  await expect.poll(async () => (await doc(page)).brushes.length).toBe(n0);
  expect(errors).toEqual([]);
});

test("Vertex tool: split an edge by dragging its middle, merge picked corners", async ({ page }) => {
  const { id, errors } = await openWithBox(page, "quad");
  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  await page.mouse.move(top.x + top.width / 2, top.y + top.height / 2, { steps: 3 });
  await page.mouse.click(top.x + top.width / 2, top.y + top.height / 2);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked");
  // where the box's corners and edge middles are on screen in the Top view (the handles' hook)
  const h = (await page.evaluate(() => (window as unknown as { __lsFlatHandles?: Record<string, () => { x: number; y: number }[]> }).__lsFlatHandles?.top?.() ?? null))!;
  expect(h).not.toBeNull();
  await page.locator('.editor-tool[data-cmd="tool.vertex"]').click();
  await page.waitForTimeout(300);

  // ⌘/Ctrl-drag the north edge's middle (the top edge is nearest the camera) 40 px north
  const n = h[5];
  await page.mouse.move(n.x, n.y, { steps: 3 });
  await page.keyboard.down(MOD);
  await page.mouse.down();
  await page.mouse.move(n.x, n.y - 20, { steps: 4 });
  await page.mouse.move(n.x, n.y - 40, { steps: 4 });
  await page.mouse.up();
  await page.keyboard.up(MOD);
  await expect.poll(async () => (await doc(page)).undo).toBe("Split");
  const b = (await doc(page)).brushes.find((x) => x.id === id)!;
  expect(b.max[1]).toBeGreaterThan(64);
  expect(b.max[1] % 16).toBe(0);
  expect(b.min).toEqual([-64, -64, 0]);

  // pick two corners on the south side (Shift adds) and merge them
  await page.mouse.click(h[0].x, h[0].y);
  await page.keyboard.down("Shift");
  await page.mouse.click(h[2].x, h[2].y);
  await page.keyboard.up("Shift");
  await expect(page.getByText(/2 corners picked/)).toBeVisible();
  await page.getByRole("button", { name: "Merge corners" }).click();
  await expect.poll(async () => (await doc(page)).undo).toBe("Merge corners");
  const m = (await doc(page)).brushes.find((x) => x.id === id)!;
  // the two top south corners became one at x = 0: the box's south top edge is gone
  expect(m.min[1]).toBe(-64);
  expect(errors).toEqual([]);
});
