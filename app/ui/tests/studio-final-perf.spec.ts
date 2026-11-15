// The final check's Studio half (docs/notes/FinalCheck.md): the Studio on the biggest graph it opens,
// 400 nodes (graph.rs MAX_NODES): the round's every-node graphs side by side, cut to the cap
// (scripts/finalcheck/graphs/big.graph.json, a version 3 graph so it also goes through the migration). Open, pan, drag a node, drag a wire, connect one,
// undo; frame times printed as FINAL-PERF {...}. Run in WebKit (the app's engine):
//   LS_WEBKIT_ALL=1 npx playwright test studio-final-perf --project webkit
// FINAL_PERF_ASSERT=1 also checks the targets: opens in under 5 s, dragging at zoom 1 p50 under
// 20 ms a step, zoomed out under 34 ms, undo painted in under 300 ms, no page errors.
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const H = { "X-Panel": "1" };
const ID = "pw_fcbig";
const BIG = fileURLToPath(new URL("../../../scripts/finalcheck/graphs/big.graph.json", import.meta.url));

async function frames(page: Page, work: () => Promise<void>, steps = 40) {
  await page.evaluate(() => {
    const w = window as unknown as { __lsFcF: number[]; __lsFcOn: boolean };
    w.__lsFcF = [];
    w.__lsFcOn = true;
    let last = performance.now();
    const tick = (t: number) => {
      w.__lsFcF.push(t - last);
      last = t;
      if (w.__lsFcOn) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const t0 = Date.now();
  await work();
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  const ms = Date.now() - t0;
  const f = await page.evaluate(() => {
    const w = window as unknown as { __lsFcF: number[]; __lsFcOn: boolean };
    w.__lsFcOn = false;
    return w.__lsFcF.slice(2);
  });
  const s = [...f].sort((a, b) => a - b);
  const q = (p: number) => Math.round(s[Math.min(s.length - 1, Math.floor(s.length * p))] ?? 0);
  return { n: s.length, p50: q(0.5), p95: q(0.95), perStep: Math.round(ms / steps) };
}
async function drag(page: Page, from: { x: number; y: number }, by: { x: number; y: number }, steps = 40) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (let i = 1; i <= steps; i++) await page.mouse.move(from.x + (by.x * i) / steps, from.y + (by.y * i) / steps);
  await page.mouse.up();
}
const painted = (page: Page) => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
const savedEdges = async (page: Page) => ((await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph?.edges ?? []).length;

// nothing installs or compiles
const WRITES = ["compile", "check", "testrun", "testrun/cancel", "playtest", "switch", "source", "uninstall/**", "debug/command"];
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("ls.studio.panels", JSON.stringify({ left: true, right: true, minimap: false })));
  for (const r of WRITES) await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test: not installed)", installedAs: ID } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
});
test.afterAll(async ({ playwright }, info) => {
  const api = await playwright.request.newContext({ baseURL: String(info.project.use.baseURL) });
  await api.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H }).catch(() => {});
  await api.dispose();
});

test("400-node graph: open, pan, drag, wire, connect, undo", async ({ page }) => {
  test.setTimeout(300_000);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const g = JSON.parse(readFileSync(BIG, "utf8"));
  g.mode.id = ID;
  expect(g.nodes.length, "the big graph: at the cap").toBeGreaterThanOrEqual(390);
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json()).ok, "saved").toBeTruthy();
  await page.setViewportSize({ width: 1440, height: 900 });
  const t0 = Date.now();
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("tablist", { name: "Graphs" })).toBeVisible({ timeout: 60_000 });
  await page.locator(".react-flow__node-logic").first().waitFor({ timeout: 60_000 });
  await painted(page);
  const openMs = Date.now() - t0;
  const pane = page.locator(".react-flow__pane");
  const box = (await pane.boundingBox())!;
  const mid = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const out: Record<string, unknown> = { nodes: g.nodes.length, edges: g.edges.length, openMs };
  const undoMs: number[] = [];

  for (const [zoom, label] of [[1, "zoom 1"], [0.25, "zoomed out"]] as const) {
    await pane.click({ position: { x: 5, y: 5 } });
    await page.evaluate((z) => (window as unknown as { __studioZoom: (z: number) => void }).__studioZoom(z), zoom);
    await expect.poll(() => page.locator(".react-flow__viewport").evaluate((el) => new DOMMatrixReadOnly(getComputedStyle(el).transform).a)).toBeCloseTo(zoom, 2);
    await page.waitForTimeout(300);
    out[`pan, ${label}`] = await frames(page, () => drag(page, { x: box.x + 8, y: box.y + box.height - 8 }, { x: 240, y: -140 }));
    // a node near the middle, by its title
    const nodeId = await page.evaluate(({ x, y, pane }) => {
      let id = "", d = 1e9;
      for (const el of document.querySelectorAll<HTMLElement>(".react-flow__node-logic")) {
        const r = el.getBoundingClientRect();
        if (r.y < pane.y + 4 || r.y > pane.y + pane.height - 40 || r.x < pane.x + 4 || r.x + 40 > pane.x + pane.width || r.width < 4) continue;
        const dd = Math.hypot(r.x + r.width / 2 - x, r.y + 10 - y);
        if (dd < d) ((d = dd), (id = el.dataset.id ?? ""));
      }
      return id;
    }, { ...mid, pane: box });
    expect(nodeId, `a node in view (${label})`).not.toBe("");
    const tb = (await page.locator(`.react-flow__node[data-id="${nodeId}"] .st-node-title`).first().boundingBox())!;
    const head = { x: tb.x + Math.min(tb.width / 2, 30), y: tb.y + tb.height / 2 };
    const where = () => page.locator(`.react-flow__node[data-id="${nodeId}"]`).evaluateAll((els) => (els[0] as HTMLElement | undefined)?.style.transform ?? "out of view");
    const before = await where();
    out[`drag node, ${label}`] = await frames(page, () => drag(page, head, { x: 160, y: 90 }));
    expect(await where(), `the node moved (${label})`).not.toBe(before);
    const u0 = Date.now();
    await page.keyboard.press("ControlOrMeta+z");
    await painted(page);
    undoMs.push(Date.now() - u0);
    await expect.poll(where, { message: `undo put it back (${label})` }).toBe(before);
    // a wire dragged into empty space (the node search opens: Escape)
    await pane.click({ position: { x: 5, y: 5 } });
    const handle = page.locator(".react-flow__handle.source").filter({ visible: true }).first();
    const hb = await handle.boundingBox();
    if (hb) {
      out[`drag wire, ${label}`] = await frames(page, () => drag(page, { x: hb.x + hb.width / 2, y: hb.y + hb.height / 2 }, { x: 140, y: 120 }));
      await page.keyboard.press("Escape");
    }
  }

  // connecting a wire at zoom 1: an exec output to an exec input that has nothing in it, then undo
  await page.evaluate(() => (window as unknown as { __studioZoom: (z: number) => void }).__studioZoom(1));
  await page.waitForTimeout(400);
  const pair = await page.evaluate(() => {
    const vis = (el: Element) => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.y > 80 && r.y < innerHeight - 40 && r.x > 300 && r.x < innerWidth - 340 ? r : null;
    };
    const src = [...document.querySelectorAll(".react-flow__handle.source")].map((e) => [e, vis(e)] as const).find(([, r]) => r);
    if (!src) return null;
    const from = src[0].closest<HTMLElement>(".react-flow__node")?.dataset.id;
    const tgt = [...document.querySelectorAll(".react-flow__handle.target")].map((e) => [e, vis(e)] as const).find(([e, r]) => r && e.closest<HTMLElement>(".react-flow__node")?.dataset.id !== from && !e.classList.contains("connected"));
    if (!tgt) return null;
    const c = (r: DOMRect) => ({ x: r.x + r.width / 2, y: r.y + r.height / 2 });
    return { a: c(src[1]!), b: c(tgt[1]!) };
  });
  if (pair) {
    const n0 = await page.locator(".react-flow__edge").count();
    const c0 = Date.now();
    await drag(page, pair.a, { x: pair.b.x - pair.a.x, y: pair.b.y - pair.a.y }, 20);
    await painted(page);
    out.connectMs = Date.now() - c0;
    const n1 = await page.locator(".react-flow__edge").count();
    out.connected = n1 > n0;
    if (n1 > n0) {
      const u0 = Date.now();
      await page.keyboard.press("ControlOrMeta+z");
      await painted(page);
      undoMs.push(Date.now() - u0);
      await expect.poll(() => page.locator(".react-flow__edge").count(), { message: "undo took the wire away" }).toBe(n0);
    } else await page.keyboard.press("Escape");
  }
  out.undoMs = undoMs;
  out.pageErrors = errors;
  out.savedEdges = await savedEdges(page);
  console.log("FINAL-PERF " + JSON.stringify(out));
  if (process.env.FINAL_PERF_ASSERT) {
    expect(openMs).toBeLessThan(5000);
    expect((out["drag node, zoom 1"] as { perStep: number }).perStep).toBeLessThan(20);
    expect((out["drag node, zoomed out"] as { perStep: number }).perStep).toBeLessThan(34);
    expect(Math.max(...undoMs)).toBeLessThan(300);
    expect(errors).toEqual([]);
  }
});
