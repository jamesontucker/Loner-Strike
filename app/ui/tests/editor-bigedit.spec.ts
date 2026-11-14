// Map editor on br_valley (Parity A, PerfAudit.md findings 1-3): frame times while nudging one
// brush and undoing, one click-select, a ~3000-brush box select, and memory across 10 box
// select / deselect cycles (the renderer's geometry and texture counts; the JS heap after a GC
// in Chromium). Numbers land in the annotations; the limits are the audit's targets, loosened for
// headless software drawing in Chromium. Reads maps-src/br_valley/br_valley.map (or LS_TEST_BRVALLEY).
import { expect, test, type Page } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const MAP = process.env.LS_TEST_BRVALLEY ?? resolve(process.cwd(), "../../maps-src/br_valley/br_valley.map");
const MOD = process.platform === "darwin" ? "Meta" : "Control";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

/** Start recording the gaps between animation frames. */
const record = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __gaps: number[]; __rec: boolean };
    w.__gaps = [];
    w.__rec = true;
    let last = performance.now();
    const f = (t: number) => {
      w.__gaps.push(t - last);
      last = t;
      if (w.__rec) requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
  });
const stop = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __gaps: number[]; __rec: boolean };
    w.__rec = false;
    const g = [...w.__gaps].sort((a, b) => a - b);
    const q = (p: number) => Math.round(g[Math.min(g.length - 1, Math.floor(g.length * p))] ?? 0);
    return { n: g.length, p50: q(0.5), p95: q(0.95), max: Math.round(g[g.length - 1] ?? 0) };
  });
/** The renderer's live geometries and textures (dev builds expose the renderer; -1 in a production build). */
const memory = (page: Page) =>
  page.evaluate(() => {
    const gl = (window as unknown as { __lsEditorGl?: { info: { memory: { geometries: number; textures: number } } } }).__lsEditorGl;
    return { geometries: gl?.info.memory.geometries ?? -1, textures: gl?.info.memory.textures ?? -1 };
  });

test("br_valley: nudge + undo, click select, box select, memory over 10 cycles", async ({ page, browserName }) => {
  test.skip(!existsSync(MAP), `needs ${MAP}`);
  test.setTimeout(600_000);
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 4 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
  });
  await page.goto("/editor-dev.html");
  expect((await api(page, "/api/tools/editor/new", {})).error ?? null).toBeNull();
  expect((await api(page, "/api/tools/editor/cmd", { op: "paste", text: readFileSync(MAP, "utf8"), offset: [0, 0, 0] })).error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  await back.waitFor({ timeout: 30_000 });
  await back.click();
  await page.waitForFunction(() => /\d{4} brushes/.test(document.querySelector(".tools-status")?.textContent || ""), null, { timeout: 120_000 });
  await page.waitForTimeout(4000);
  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  const cx = top.x + top.width / 2;
  const cy = top.y + top.height / 2;
  const notes: string[] = [];

  // one click picks one brush
  await page.mouse.move(cx, cy, { steps: 4 });
  await page.waitForTimeout(500);
  await record(page);
  await page.mouse.click(cx, cy);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked", { timeout: 10_000 });
  await page.waitForTimeout(600);
  const click = await stop(page);
  notes.push(`click select: worst frame ${click.max} ms`);

  // nudge it and undo, 10 times
  await page.waitForTimeout(800);
  await record(page);
  for (let i = 0; i < 10; i++) {
    await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(250);
    await page.keyboard.press(`${MOD}+z`);
    await page.waitForTimeout(250);
  }
  await page.waitForTimeout(1500);
  const nudge = await stop(page);
  notes.push(`nudge+undo x10: frames p50 ${nudge.p50} / p95 ${nudge.p95} / max ${nudge.max} ms`);

  // a box over most of the Top view's left side (touching): ~3000 brushes
  await page.keyboard.press("Escape");
  await page.waitForTimeout(800);
  const boxSelect = async () => {
    // from 70% across: about 3000 of br_valley's 5400 brushes
    await page.mouse.move(top.x + top.width * 0.7, top.y + top.height - 12, { steps: 2 });
    await page.mouse.down();
    await page.mouse.move(top.x + 12, top.y + 40, { steps: 6 });
    await record(page);
    await page.mouse.up();
    await expect(page.locator(".ed-selsize")).toContainText(/\d{3,} picked/, { timeout: 30_000 });
    await page.waitForTimeout(800);
    return stop(page);
  };
  const box = await boxSelect();
  const picked = (await page.locator(".ed-selsize").textContent())?.match(/(\d+) picked/)?.[1];
  notes.push(`box select (${picked} picked): worst frame ${box.max} ms`);
  await record(page);
  await page.keyboard.press("Escape");
  await expect(page.locator(".ed-selsize")).toHaveCount(0);
  await page.waitForTimeout(800);
  const esc = await stop(page);
  notes.push(`deselect: worst frame ${esc.max} ms`);

  // memory: 10 select / deselect cycles
  const cdp = browserName === "chromium" ? await page.context().newCDPSession(page) : null;
  const heap = async () => {
    if (!cdp) return null;
    await cdp.send("HeapProfiler.collectGarbage");
    const h = (await cdp.send("Runtime.getHeapUsage")) as { usedSize: number };
    return Math.round(h.usedSize / 1e6);
  };
  const m0 = await memory(page);
  const h0 = await heap();
  for (let i = 0; i < 10; i++) {
    await boxSelect();
    await page.keyboard.press("Escape");
    await expect(page.locator(".ed-selsize")).toHaveCount(0);
    await page.waitForTimeout(400);
  }
  await page.waitForTimeout(1000);
  const m1 = await memory(page);
  const h1 = await heap();
  notes.push(`10 cycles: geometries ${m0.geometries} -> ${m1.geometries}, textures ${m0.textures} -> ${m1.textures}${h0 != null ? `, JS heap ${h0} -> ${h1} MB` : ""}`);
  test.info().annotations.push({ type: "perf", description: notes.join("; ") });
  console.log("PERF", notes.join("; "));
});
