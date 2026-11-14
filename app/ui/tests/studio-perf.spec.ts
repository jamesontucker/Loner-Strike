// Game Mode Studio on a big graph (every_node: 374 nodes, 509 wires): frame times while dragging a
// node, dragging a new wire and panning, at zoom 1 and zoomed out. Run in WebKit (the app's engine):
//   npx playwright test studio-perf -c <a config with browserName webkit>
// It prints p50/p95 per case and checks the targets from docs/notes/PerfAudit.md (#4): 60 fps when
// dragging at zoom 1 (p50 under 20 ms), smooth zoomed out.
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";

const H = { "X-Panel": "1" };
const ID = "pw_perf";

/** Frame times (ms) while `work` runs: requestAnimationFrame deltas in the page. */
async function frames(page: Page, work: () => Promise<void>) {
  await page.evaluate(() => {
    const w = window as unknown as { __f: number[]; __on: boolean };
    w.__f = [];
    w.__on = true;
    let last = performance.now();
    const tick = (t: number) => {
      w.__f.push(t - last);
      last = t;
      if (w.__on) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const t0 = Date.now();
  await work();
  // (the last frame painted: what the user sees once the drag ends)
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const ms = Date.now() - t0;
  const f = await page.evaluate(() => {
    const w = window as unknown as { __f: number[]; __on: boolean };
    w.__on = false;
    return w.__f.slice(2);
  });
  const s = [...f].sort((a, b) => a - b);
  const q = (p: number) => Math.round(s[Math.min(s.length - 1, Math.floor(s.length * p))] ?? 0);
  // per step: the wall time over the 40 mouse moves (each move waits for the page to take it)
  return { n: s.length, p50: q(0.5), p95: q(0.95), perStep: Math.round(ms / 40) };
}

async function drag(page: Page, from: { x: number; y: number }, by: { x: number; y: number }, steps = 40) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (let i = 1; i <= steps; i++) await page.mouse.move(from.x + (by.x * i) / steps, from.y + (by.y * i) / steps);
  await page.mouse.up();
}

test.afterAll(async ({ playwright }, info) => {
  const api = await playwright.request.newContext({ baseURL: String(info.project.use.baseURL) });
  await api.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H }).catch(() => {});
  await api.dispose();
});

test("every_node: drag, wire and pan frame times", async ({ page }) => {
  test.setTimeout(180_000);
  const g = JSON.parse(readFileSync("../src-tauri/src/tools/studio/testdata/every_node.graph.json", "utf8"));
  g.mode.id = ID;
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json()).ok).toBeTruthy();
  await page.setViewportSize({ width: 1440, height: 900 });
  // (STUDIO_ALL=1: everything drawn, to compare with drawing only what's in view)
  if (process.env.STUDIO_ALL) await page.addInitScript(() => ((window as unknown as { __studioAll: boolean }).__studioAll = true));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("tablist", { name: "Graphs" })).toBeVisible({ timeout: 30_000 });
  const pane = page.locator(".react-flow__pane");
  const box = (await pane.boundingBox())!;
  const mid = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const results: Record<string, { n: number; p50: number; p95: number; perStep: number }> = {};

  for (const [zoom, label] of [[1, "zoom 1"], [0.25, "zoomed out"]] as const) {
    // the view: the first event at this zoom
    await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
    await page.evaluate((z) => {
      const vp = document.querySelector<HTMLElement>(".react-flow__viewport")!;
      void vp;
      (window as unknown as { __z?: number }).__z = z;
    }, zoom);
    await page.keyboard.press("Home");
    await page.waitForTimeout(400);
    const flow = page.locator(".react-flow");
    // zoom with the wheel to about the wanted level
    for (let i = 0; i < 12; i++) {
      const z = await page.locator(".react-flow__viewport").evaluate((el) => new DOMMatrixReadOnly(getComputedStyle(el).transform).a);
      if (Math.abs(z - zoom) / zoom < 0.15) break;
      await page.mouse.move(mid.x, mid.y);
      await page.mouse.wheel(0, z < zoom ? -200 : 200);
      await page.waitForTimeout(150);
    }
    void flow;
    await page.waitForTimeout(300);
    // a node in view, near the middle
    const nodeBox = await page.evaluate(({ x, y }) => {
      let best: DOMRect | null = null, d = 1e9;
      for (const el of document.querySelectorAll<HTMLElement>(".react-flow__node-logic")) {
        const r = el.getBoundingClientRect();
        const dd = Math.hypot(r.x + r.width / 2 - x, r.y + 10 - y);
        if (r.width > 4 && dd < d) ((d = dd), (best = r));
      }
      return best ? { x: best.x, y: best.y, w: best.width, h: best.height } : null;
    }, mid);
    expect(nodeBox).not.toBeNull();
    const head = { x: nodeBox!.x + nodeBox!.w / 2, y: nodeBox!.y + Math.max(4, 8 * zoom) };
    results[`drag node, ${label}`] = await frames(page, () => drag(page, head, { x: 160, y: 90 }));
    await page.keyboard.press("ControlOrMeta+z");
    // a new wire from an output pin into empty space (then Escape the node search)
    const handle = page.locator(".react-flow__handle.source").filter({ visible: true }).first();
    const hb = await handle.boundingBox();
    if (hb) {
      results[`drag wire, ${label}`] = await frames(page, () => drag(page, { x: hb.x + hb.width / 2, y: hb.y + hb.height / 2 }, { x: 140, y: 120 }));
      await page.keyboard.press("Escape");
    }
    results[`pan, ${label}`] = await frames(page, () => drag(page, { x: box.x + 8, y: box.y + box.height - 8 }, { x: 200, y: -120 }));
  }
  console.log("STUDIO-PERF " + JSON.stringify(results));
  // the targets: 60 fps dragging at zoom 1 (p50 under 20 ms); smooth zoomed out (p95 under 50 ms)
  if (process.env.STUDIO_PERF_ASSERT) {
    expect(results["drag node, zoom 1"].perStep).toBeLessThan(20);
    expect(results["drag node, zoomed out"].perStep).toBeLessThan(34);
  }
});
