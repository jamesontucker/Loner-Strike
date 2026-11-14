// Game Mode Studio on a big graph (every_node: 374 nodes, 509 wires): frame times while dragging a
// node, dragging a new wire and panning, at zoom 1 and zoomed out. Run in WebKit (the app's engine):
//   npx playwright test studio-perf -c <a config with browserName webkit>
// It prints p50/p95 per case and checks the targets from docs/notes/PerfAudit.md (#4): 60 fps when
// dragging at zoom 1 (p50 under 20 ms), smooth zoomed out. Also every node picked and dragged
// together (StudioPerfAudit #3). Each drag checks the node really moved: the frame loop's flags
// have their own names (`window.__on` is d3's, inside React Flow: using it broke every drag,
// StudioPerfAudit #4).
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const H = { "X-Panel": "1" };
const ID = "pw_perf";

/** Frame times (ms) while `work` runs: requestAnimationFrame deltas in the page. */
async function frames(page: Page, work: () => Promise<void>) {
  await page.evaluate(() => {
    const w = window as unknown as { __lsPerfF: number[]; __lsPerfOn: boolean };
    w.__lsPerfF = [];
    w.__lsPerfOn = true;
    let last = performance.now();
    const tick = (t: number) => {
      w.__lsPerfF.push(t - last);
      last = t;
      if (w.__lsPerfOn) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const t0 = Date.now();
  await work();
  // (the last frame painted: what the user sees once the drag ends)
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const ms = Date.now() - t0;
  const f = await page.evaluate(() => {
    const w = window as unknown as { __lsPerfF: number[]; __lsPerfOn: boolean };
    w.__lsPerfOn = false;
    return w.__lsPerfF.slice(2);
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
  const g = JSON.parse(readFileSync(fileURLToPath(new URL("../../src-tauri/src/tools/studio/testdata/every_node.graph.json", import.meta.url)), "utf8"));
  g.mode.id = ID;
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json()).ok).toBeTruthy();
  await page.setViewportSize({ width: 1440, height: 900 });
  // (STUDIO_ALL=1: everything drawn, to compare with drawing only what's in view)
  if (process.env.STUDIO_ALL) await page.addInitScript(() => ((window as unknown as { __studioAll: boolean }).__studioAll = true));
  // (opening: to the first node drawn and two frames after; StudioPerfAudit #1)
  const t0 = Date.now();
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("tablist", { name: "Graphs" })).toBeVisible({ timeout: 30_000 });
  await page.locator(".react-flow__node-logic").first().waitFor({ timeout: 30_000 });
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const openMs = Date.now() - t0;
  const pane = page.locator(".react-flow__pane");
  const box = (await pane.boundingBox())!;
  const mid = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const results: Record<string, { n: number; p50: number; p95: number; perStep: number }> = {};
  const undoMs: number[] = [];

  for (const [zoom, label] of [[1, "zoom 1"], [0.25, "zoomed out"]] as const) {
    // the view: the first event at this zoom
    await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
    // the view: the graph's first node in the middle, at this zoom (the Studio's development hook)
    await page.evaluate((z) => (window as unknown as { __studioZoom: (z: number) => void }).__studioZoom(z), zoom);
    await expect.poll(() => page.locator(".react-flow__viewport").evaluate((el) => new DOMMatrixReadOnly(getComputedStyle(el).transform).a)).toBeCloseTo(zoom, 2);
    await page.waitForTimeout(300);
    // a node in view, near the middle
    const nodeBox = await page.evaluate(({ x, y, pane }) => {
      let best: DOMRect | null = null, d = 1e9, id = "";
      for (const el of document.querySelectorAll<HTMLElement>(".react-flow__node-logic")) {
        const r = el.getBoundingClientRect();
        // (its title inside the canvas, with room to drag)
        if (r.y < pane.y + 4 || r.y > pane.y + pane.height - 40 || r.x < pane.x + 4 || r.x + 40 > pane.x + pane.width) continue;
        const dd = Math.hypot(r.x + r.width / 2 - x, r.y + 10 - y);
        if (r.width > 4 && dd < d) ((d = dd), (best = r), (id = el.dataset.id ?? ""));
      }
      return best ? { x: best.x, y: best.y, w: best.width, h: best.height, id } : null;
    }, { ...mid, pane: box });
    expect(nodeBox).not.toBeNull();
    // (the drag starts on the node's title text: its corner can be a breakpoint button or a pin)
    const tb = (await page.locator(`.react-flow__node[data-id="${nodeBox!.id}"] .st-node-title`).first().boundingBox())!;
    const head = { x: tb.x + Math.min(tb.width / 2, 30), y: tb.y + tb.height / 2 };
    // (the node found above: where it is now)
    const at = () => page.locator(`.react-flow__node[data-id="${nodeBox!.id}"]`).evaluate((el) => ({ id: (el as HTMLElement).dataset.id!, t: (el as HTMLElement).style.transform }));
    const before = await at();
    expect(before, "a node under the mouse").not.toBeNull();
    // (a node moved out of view isn't drawn any more: that counts as moved)
    const moved = async (b: { id: string; t: string }) => page.locator(`.react-flow__node[data-id="${b.id}"]`).evaluateAll((els) => (els[0] as HTMLElement | undefined)?.style.transform ?? "out of view");
    results[`drag node, ${label}`] = await frames(page, () => drag(page, head, { x: 160, y: 90 }));
    expect(await moved(before!), `the node moved (${label})`).not.toBe(before!.t);
    // an undo, to its paint
    const u0 = Date.now();
    await page.keyboard.press("ControlOrMeta+z");
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    undoMs.push(Date.now() - u0);
    // every node picked, dragged together
    // (the keys to the canvas itself: a click at a corner can land on a comment box's text)
    await page.locator(".st-canvas").focus();
    await page.keyboard.press("ControlOrMeta+a");
    // (every node of the graph is picked; those in view are drawn)
    await expect.poll(async () => (await page.locator(".react-flow__node.selected").count()) === (await page.locator(".react-flow__node").count()), { message: "everything in view picked" }).toBe(true);
    const others = await page.locator(".react-flow__node.selected").evaluateAll((els) => els.slice(0, 6).map((el) => [(el as HTMLElement).dataset.id!, (el as HTMLElement).style.transform]));
    const all = await at();
    expect(all).not.toBeNull();
    results[`drag all, ${label}`] = await frames(page, () => drag(page, head, { x: 160, y: 90 }));
    expect(await moved(all!), `the picked nodes moved (${label})`).not.toBe(all!.t);
    for (const [id, t] of others) expect(await moved({ id, t }), `${id} moved with them`).not.toBe(t);
    await page.keyboard.press("ControlOrMeta+z");
    await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
    // a new wire from an output pin into empty space (then Escape the node search)
    const handle = page.locator(".react-flow__handle.source").filter({ visible: true }).first();
    const hb = await handle.boundingBox();
    if (hb) {
      results[`drag wire, ${label}`] = await frames(page, () => drag(page, { x: hb.x + hb.width / 2, y: hb.y + hb.height / 2 }, { x: 140, y: 120 }));
      await page.keyboard.press("Escape");
    }
    results[`pan, ${label}`] = await frames(page, () => drag(page, { x: box.x + 8, y: box.y + box.height - 8 }, { x: 200, y: -120 }));
  }
  console.log("STUDIO-PERF " + JSON.stringify({ openMs, undoMs, ...results }));
  // the targets: 60 fps dragging at zoom 1 (p50 under 20 ms); smooth zoomed out (p95 under 50 ms)
  if (process.env.STUDIO_PERF_ASSERT) {
    expect(results["drag node, zoom 1"].perStep).toBeLessThan(20);
    expect(results["drag node, zoomed out"].perStep).toBeLessThan(34);
  }
});

test("every_node: frame times while a Playtest streams node runs (the debugger's flash)", async ({ page }) => {
  test.setTimeout(180_000);
  const g = JSON.parse(readFileSync(fileURLToPath(new URL("../../src-tauri/src/tools/studio/testdata/every_node.graph.json", import.meta.url)), "utf8"));
  g.mode.id = ID;
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json()).ok).toBeTruthy();
  const ids: string[] = g.nodes.filter((n: { graph?: string }) => !n.graph).map((n: { id: string }) => n.id);
  // a Playtest that starts at once, then the game's events: about 1,500 node runs a second, spread
  // over the Event Graph's nodes (StudioPerfAudit #2)
  for (const r of ["compile", "check", "playtest", "debug/command"]) await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test)", installedAs: ID } }));
  await page.route("**/api/state", (r) => r.fulfill({ json: { game_open: false } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
  let seq = 0;
  let last = Date.now();
  await page.route("**/api/tools/studio/debug/events**", (route) => {
    const now = Date.now();
    const n = Math.min(600, Math.round(((now - last) / 1000) * 1500));
    last = now;
    const events = Array.from({ length: n }, () => ({ s: ++seq, t: now, e: "node_enter", node: ids[Math.floor(Math.random() * ids.length)] }));
    return route.fulfill({ json: { events, next: seq } });
  });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("tablist", { name: "Graphs" })).toBeVisible({ timeout: 30_000 });
  // everything in view (the flashes are drawn only for nodes in view), once the opening view is set
  await page.locator(".react-flow__node").first().waitFor();
  await page.waitForTimeout(300);
  await page.locator(".react-flow__controls-fitview").click();
  await page.getByTestId("studio-playtest").click();
  await expect(page.locator(".st-flash").first()).toBeAttached({ timeout: 20_000 });
  const r = await frames(page, () => page.waitForTimeout(4000));
  console.log("STUDIO-PERF-DEBUG " + JSON.stringify({ frames: r.n, p50: r.p50, p95: r.p95 }));
  if (process.env.STUDIO_PERF_ASSERT) expect(r.p95).toBeLessThan(40);
});
