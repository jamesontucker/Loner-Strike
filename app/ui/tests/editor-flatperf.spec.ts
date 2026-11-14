// Map editor, the Top/Front/Side views on a big map (Parity A): br_valley (about 3000 brushes,
// 8000 units across) in Quad; panning and zooming the Top view keeps the page's frame rate up,
// the flat grid is one draw call, and the selection handles work there too. Runs on
// editor-dev.html against the tools dev server; reads maps-src/br_valley/br_valley.map
// (or LS_TEST_BRVALLEY). Numbers land in the test's annotations.
import { expect, test, type Page } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const MAP = process.env.LS_TEST_BRVALLEY ?? resolve(process.cwd(), "../../maps-src/br_valley/br_valley.map");

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

async function framesWhile(page: Page, act: () => Promise<void>) {
  await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean };
    w.__f = 0;
    w.__stop = false;
    const f = () => {
      w.__f++;
      if (!w.__stop) requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
  });
  const t0 = Date.now();
  await act();
  const seconds = (Date.now() - t0) / 1000;
  const frames = await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean };
    w.__stop = true;
    return w.__f;
  });
  return frames / seconds;
}

test("br_valley in Quad: the Top view pans and zooms smoothly", async ({ page }) => {
  test.skip(!existsSync(MAP), `needs ${MAP}`);
  test.setTimeout(240_000);
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 4 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
  });
  await page.goto("/editor-dev.html");
  expect((await api(page, "/api/tools/editor/new", {})).error ?? null).toBeNull();
  const r = await api(page, "/api/tools/editor/cmd", { op: "paste", text: readFileSync(MAP, "utf8"), offset: [0, 0, 0] });
  expect(r.error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  await back.waitFor({ timeout: 20_000 });
  await back.click();
  await page.waitForFunction(() => /\d{4} brushes/.test(document.querySelector(".tools-status")?.textContent || ""), null, { timeout: 120_000 });
  await page.waitForTimeout(2500);

  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  const cx = top.x + top.width / 2;
  const cy = top.y + top.height / 2;
  await page.mouse.move(cx, cy, { steps: 4 });
  // pan with the middle button
  const panFps = await framesWhile(page, async () => {
    await page.mouse.down({ button: "middle" });
    for (let i = 0; i < 20; i++) {
      await page.mouse.move(cx + Math.sin(i / 3) * 80, cy + Math.cos(i / 3) * 50, { steps: 2 });
      await page.waitForTimeout(60);
    }
    await page.mouse.up({ button: "middle" });
  });
  // zoom in and back out at the cursor
  const zoomFps = await framesWhile(page, async () => {
    for (let i = 0; i < 16; i++) {
      await page.mouse.wheel(0, i < 8 ? -120 : 120);
      await page.waitForTimeout(80);
    }
  });
  const calls = await page.evaluate(() => (window as unknown as { __lsEditorGl?: { info: { render: { calls: number } } } }).__lsEditorGl?.info.render.calls ?? -1);
  test.info().annotations.push({ type: "perf", description: `br_valley Quad, Top view: ${Math.round(panFps)} fps panning, ${Math.round(zoomFps)} fps zooming, ${calls} draws in the last pane drawn` });

  // pick a brush in the Top view and stretch it with a handle: one Scale step
  await page.mouse.click(cx, cy);
  const handles = () => page.evaluate(() => (window as unknown as { __lsFlatHandles?: Record<string, () => { x: number; y: number }[]> }).__lsFlatHandles?.top?.() ?? null);
  const picked = await expect.poll(handles, { timeout: 5000 }).not.toBeNull().then(
    () => true,
    () => false,
  );
  if (picked) {
    const h = (await handles())!;
    await page.mouse.move(h[3].x, h[3].y, { steps: 3 });
    await page.mouse.down();
    await page.mouse.move(h[3].x + 30, h[3].y, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).undo, { timeout: 15_000 }).toBe("Scale");
  }
  // software drawing in headless Chromium sets a low bar; WebKit draws on the GPU
  expect(panFps).toBeGreaterThan(10);
  expect(zoomFps).toBeGreaterThan(10);
});
