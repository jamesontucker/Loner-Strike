// Map editor, the Shape tool's primitives (Parity A): torus and cone are in the shape list, the
// Preview popover shows the shape and follows its settings, and dragging one out in the Top view
// makes the brushes the settings ask for (a torus's pieces, a cut-off cone), one undo step each.
// Runs on editor-dev.html against the tools dev server (npm run test:tools -- tests/editor-shapes.spec.ts).
import { expect, test, type Page } from "@playwright/test";

// (pixel drags on the panes: at the "small" project's 1024x680 the Content dock leaves 2D panes about
// 120 px tall, too small for these gestures; layouts at that size are covered by editor-batch/tools)
test.use({ screenshot: "only-on-failure", viewport: { width: 1440, height: 900 } });

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

async function openEmptyQuad(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (e) => !/access control checks/.test(String(e)) && errors.push(String(e)));
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
    localStorage.setItem("ls.editor.shapeParams", JSON.stringify({ coneTop: 0, tubeSides: 8, addHeight: 0 }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  // a floor only, so the Top view is fitted to it and shapes stand on it
  const d0 = await api(page, "/api/tools/editor/doc");
  await api(page, "/api/tools/editor/cmd", { op: "delete", brushes: d0.brushes.map((x: { id: number }) => x.id), entities: d0.entities.map((x: { id: number }) => x.id) });
  await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-512, -512, -16], max: [512, 512, 0], texture: "CRATE01" });
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(2000);
  return errors;
}

async function dragInTop(page: Page, from: [number, number], to: [number, number]) {
  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  const at = (f: [number, number]) => [top.x + top.width * f[0], top.y + top.height * f[1]] as const;
  await page.mouse.move(...at(from), { steps: 5 });
  await page.waitForTimeout(100);
  await page.mouse.down();
  await page.mouse.move(...at([(from[0] + to[0]) / 2, (from[1] + to[1]) / 2]), { steps: 4 });
  await page.mouse.move(...at(to), { steps: 4 });
  await page.mouse.up();
}

const count = async (page: Page) => (await api(page, "/api/tools/editor/doc")).brushes.length as number;

test("torus and cone: settings, preview, and drawing them out", async ({ page }) => {
  const errors = await openEmptyQuad(page);
  await page.locator('.editor-tool[data-cmd="tool.shape"]').click();
  const shapes = page.getByRole("group", { name: "Shape" });
  await shapes.getByRole("button", { name: "Torus" }).click();
  // the torus's own settings in the tool bar
  await page.getByLabel("Pieces").fill("12");
  await page.getByLabel("Tube sides").fill("6");
  await page.getByLabel("Tube width").fill("64");

  // Preview: every setting with a wireframe of the shape; changing one redraws it
  await page.getByRole("button", { name: "Preview…" }).click();
  const pop = page.getByRole("dialog", { name: "Torus settings" });
  await expect(pop).toBeVisible();
  const wire = pop.getByRole("img", { name: "Preview of the shape" });
  // 13 cross-sections of 6 edges, joined by 12 × 6 lines
  await expect(wire).toHaveAttribute("data-segments", String(13 * 6 + 12 * 6));
  await pop.getByLabel("Tube sides").fill("8");
  await expect(wire).toHaveAttribute("data-segments", String(13 * 8 + 12 * 8));
  await pop.getByLabel("Add height").fill("8");
  await expect(pop.getByText("12 brushes.")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(pop).toHaveCount(0);
  // the tool bar shows what the popover set
  await expect(page.getByLabel("Tube sides")).toHaveValue("8");

  // drag it out on the floor in the Top view: 12 pieces, climbing 8 each, one undo step
  const n0 = await count(page);
  await dragInTop(page, [0.3, 0.3], [0.7, 0.7]);
  await expect.poll(() => count(page)).toBe(n0 + 12);
  const doc = await api(page, "/api/tools/editor/doc");
  expect(doc.undo).toBe("Add torus");
  const made = doc.brushes.slice(-12) as { min: number[]; max: number[] }[];
  expect(made[11].min[2] - made[0].min[2]).toBeGreaterThan(80);

  // a cone cut off at half its base: one brush, sides + 2 faces
  await shapes.getByRole("button", { name: "Cone" }).click();
  await page.getByLabel("Sides", { exact: true }).fill("10");
  await page.getByLabel("Top %").fill("50");
  await page.getByRole("button", { name: "Preview…" }).click();
  await expect(page.getByRole("dialog", { name: "Cone settings" }).getByRole("img", { name: "Preview of the shape" })).toHaveAttribute("data-segments", String(10 * 3));
  await page.keyboard.press("Escape");
  const n1 = await count(page);
  await dragInTop(page, [0.15, 0.15], [0.3, 0.3]);
  await expect.poll(() => count(page)).toBe(n1 + 1);
  expect((await api(page, "/api/tools/editor/doc")).undo).toBe("Add cone");
  await page.screenshot({ path: "test-results/editor-shapes.png" });
  expect(errors).toEqual([]);
});

test("the drag ghost follows the picked shape (not always a cylinder)", async ({ page }) => {
  const errors = await openEmptyQuad(page);
  await page.locator('.editor-tool[data-cmd="tool.shape"]').click();
  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  const shots: Buffer[] = [];
  for (const name of ["Cylinder", "Torus", "Cone"]) {
    await page.getByRole("group", { name: "Shape" }).getByRole("button", { name }).click();
    await page.mouse.move(top.x + top.width * 0.3, top.y + top.height * 0.3, { steps: 4 });
    await page.mouse.down();
    await page.mouse.move(top.x + top.width * 0.7, top.y + top.height * 0.7, { steps: 6 });
    await page.waitForTimeout(150);
    shots.push(await page.screenshot({ clip: top }));
    // Esc isn't a cancel for this drag: move back to the start so nothing is made
    await page.mouse.move(top.x + top.width * 0.3, top.y + top.height * 0.3, { steps: 4 });
    await page.mouse.up();
  }
  expect(Buffer.compare(shots[0], shots[1])).not.toBe(0);
  expect(Buffer.compare(shots[1], shots[2])).not.toBe(0);
  expect(errors).toEqual([]);
});
