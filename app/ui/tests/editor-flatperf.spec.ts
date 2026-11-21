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

/** Page frames per second while `act` runs, with draw calls and pane renders per frame drawn. */
async function framesWhile(page: Page, act: () => Promise<void>) {
  await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean; __renders: number; __drawFrames: number; __lsEditorGl: { info: { autoReset: boolean; reset: () => void; render: { calls: number } }; render: (...a: unknown[]) => void; __paWrapped?: boolean } };
    const gl = w.__lsEditorGl;
    if (!gl.__paWrapped) {
      const orig = gl.render.bind(gl);
      gl.render = (...a: unknown[]) => {
        const t = performance.now();
        w.__renders++;
        orig(...a);
        (w as unknown as { __ms: number }).__ms += performance.now() - t;
      };
      gl.__paWrapped = true;
    }
    gl.info.autoReset = false;
    gl.info.reset();
    w.__renders = 0;
    (w as unknown as { __ms: number }).__ms = 0;
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
  const r = await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean; __renders: number; __lsEditorGl: { info: { autoReset: boolean; render: { calls: number } } } };
    w.__stop = true;
    const gl = w.__lsEditorGl;
    gl.info.autoReset = true;
    return { frames: w.__f, calls: gl.info.render.calls, renders: w.__renders, ms: (w as unknown as { __ms: number }).__ms };
  });
  const per = (x: number) => (r.renders ? x / r.renders : 0);
  return { fps: r.frames / seconds, renders: r.renders, calls: r.calls, drawsPerRender: Math.round(per(r.calls)), msPerRender: Math.round(per(r.ms) * 10) / 10 };
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
  const pan = await framesWhile(page, async () => {
    await page.mouse.down({ button: "middle" });
    for (let i = 0; i < 20; i++) {
      await page.mouse.move(cx + Math.sin(i / 3) * 80, cy + Math.cos(i / 3) * 50, { steps: 2 });
      await page.waitForTimeout(60);
    }
    await page.mouse.up({ button: "middle" });
  });
  // zoom in and back out at the cursor
  const zoom = await framesWhile(page, async () => {
    for (let i = 0; i < 16; i++) {
      await page.mouse.wheel(0, i < 8 ? -120 : 120);
      await page.waitForTimeout(80);
    }
  });
  test.info().annotations.push({
    type: "perf",
    description: `br_valley Quad, Top view: ${Math.round(pan.fps)} fps panning (${pan.renders} pane renders, ${pan.drawsPerRender} draws and ${pan.msPerRender} ms each), ${Math.round(zoom.fps)} fps zooming (${zoom.renders} pane renders, ${zoom.drawsPerRender} draws and ${zoom.msPerRender} ms each)`,
  });
  // panning one flat view draws that pane, not all four (a catch-up redraw of all may follow)
  expect(pan.drawsPerRender, "draws per pane").toBeLessThan(150);

  // pick a brush in the Top view and stretch it with a handle: one Scale step (try a few spots
  // near the middle for a brush big enough on screen that its handles don't overlap)
  const handles = () => page.evaluate(() => (window as unknown as { __lsFlatHandles?: Record<string, () => { x: number; y: number }[]> }).__lsFlatHandles?.top?.() ?? null);
  const wide = (h: { x: number; y: number }[] | null) => !!h && Math.max(...h.map((p) => p.x)) - Math.min(...h.map((p) => p.x)) > 24 && Math.max(...h.map((p) => p.y)) - Math.min(...h.map((p) => p.y)) > 24;
  // (zoomed in first: the whole map fits the pane, where brushes are a few pixels wide)
  await page.mouse.move(cx, cy);
  for (let i = 0; i < 6; i++) {
    await page.mouse.wheel(0, -120);
    await page.waitForTimeout(80);
  }
  let picked = false;
  for (const [dx, dy] of [[0, 0], [30, 0], [-30, 0], [0, 30], [0, -30], [50, 40], [-50, -40], [60, -20], [-60, 20]]) {
    await page.keyboard.press("Escape");
    await page.mouse.click(cx + dx, cy + dy);
    await page.waitForTimeout(400);
    if (wide(await handles())) {
      picked = true;
      break;
    }
  }
  expect(picked, "a brush big enough to stretch near the middle").toBe(true);
  {
    const h = (await handles())!;
    await page.mouse.move(h[3].x, h[3].y, { steps: 3 });
    await page.mouse.down();
    await page.mouse.move(h[3].x + 30, h[3].y, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).undo, { timeout: 15_000 }).toBe("Scale");
  }
  // software drawing in headless Chromium sets a low bar; WebKit draws on the GPU
  expect(pan.fps).toBeGreaterThan(10);
  expect(zoom.fps).toBeGreaterThan(10);
});
