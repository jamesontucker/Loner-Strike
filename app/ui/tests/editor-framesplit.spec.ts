// Map editor on br_valley (Parity A): where a slow frame's time goes while nudging a brush (arrow
// key + undo) and drag-moving one in the Top view. Instruments the page from outside (works on a
// production build, in WebKit and Chromium) and splits each frame into:
//   raf      the render loop (R3F + three.js), of which gl* is time inside WebGL calls:
//            glDraw (draw submission), glUpload (buffer data), glTex (textures), glShader (compile/link)
//   react    React's scheduled work (its scheduler's MessageChannel tasks: render + commit)
//   ipc      fetch: from the request to its body read and parsed (elapsed, mostly waiting)
//   store    the app's own work right after a response (its promise continuations: meshes, the
//            document patch, store listeners), timed up to the next task
//   input    keyboard / pointer handlers;  timers  setTimeout callbacks
//   other    the rest of the frame: GC, style/layout/paint, compositing, waiting on the GPU, idle
// A second pass (GPU) calls gl.finish() after each render-loop callback and times it: the GPU's
// own time to finish what the frame drew (WebKit has no EXT_disjoint_timer_query). Chromium also
// reports its Long Animation Frames (script attribution, style/layout).
// Env: LS_SPLIT_GPU=1 for the gl.finish pass. Prints SPLIT lines; also an annotation.
import { expect, test, type Page } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const MAP = process.env.LS_TEST_BRVALLEY ?? resolve(process.cwd(), "../../maps-src/br_valley/br_valley.map");
const MOD = process.platform === "darwin" ? "Meta" : "Control";
const GPU = process.env.LS_SPLIT_GPU === "1";
/** LS_SPLIT_MAP=tiny: the same steps on a one-box map (what an edit costs whatever the map's size). */
const TINY = process.env.LS_SPLIT_MAP === "tiny";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

/** Runs before the page's own code: wraps what the frame split needs. */
function instrument(gpu: boolean) {
  type F = Record<string, number>;
  const w = window as unknown as { __split: { on: boolean; frames: F[]; cur: F; loaf: unknown[] } };
  const S = (w.__split = { on: false, frames: [], cur: {}, loaf: [] });
  const now = () => performance.now();
  const add = (k: string, t: number) => {
    if (S.on) S.cur[k] = (S.cur[k] ?? 0) + t;
  };
  let depth = 0; // nested timing: only the outermost counts
  const timed = <T>(k: string, f: () => T): T => {
    if (depth) return f();
    depth++;
    const t = now();
    try {
      return f();
    } finally {
      depth--;
      add(k, now() - t);
    }
  };
  // WebGL calls by kind (inside the render loop: counted under raf too)
  const kinds: Record<string, string> = {
    drawArrays: "glDraw", drawElements: "glDraw", drawArraysInstanced: "glDraw", drawElementsInstanced: "glDraw", multiDrawArraysWEBGL: "glDraw",
    bufferData: "glUpload", bufferSubData: "glUpload",
    texImage2D: "glTex", texSubImage2D: "glTex", texImage3D: "glTex", texStorage2D: "glTex",
    compileShader: "glShader", linkProgram: "glShader", getProgramParameter: "glShader", getShaderParameter: "glShader", getProgramInfoLog: "glShader", getShaderInfoLog: "glShader",
  };
  let lastGl: WebGL2RenderingContext | WebGLRenderingContext | null = null;
  for (const P of [window.WebGL2RenderingContext?.prototype, window.WebGLRenderingContext?.prototype]) {
    if (!P) continue;
    for (const [m, k] of Object.entries(kinds)) {
      const orig = (P as unknown as Record<string, (...a: unknown[]) => unknown>)[m];
      if (typeof orig !== "function") continue;
      (P as unknown as Record<string, unknown>)[m] = function (this: WebGL2RenderingContext, ...a: unknown[]) {
        lastGl = this;
        if (!S.on) return orig.apply(this, a);
        const t = now();
        try {
          return orig.apply(this, a);
        } finally {
          S.cur[k] = (S.cur[k] ?? 0) + (now() - t);
        }
      };
    }
  }
  // the render loop and anything else on requestAnimationFrame
  const raf = window.requestAnimationFrame.bind(window);
  window.requestAnimationFrame = (cb: FrameRequestCallback) =>
    raf((ts) => {
      timed("raf", () => cb(ts));
      if (gpu && S.on && lastGl && (S.cur.glDraw ?? 0) > 0) {
        const t = now();
        lastGl.finish();
        S.cur.gpuFinish = (S.cur.gpuFinish ?? 0) + (now() - t);
      }
    });
  // fetch: the request through its parsed body; then the app's continuations ("store"), timed by a
  // task posted on a channel of our own (it runs once every microtask after the response is done)
  const probe = new MessageChannel();
  const pending: number[] = [];
  probe.port1.onmessage = () => {
    const t = pending.shift();
    if (t != null) add("store", now() - t);
  };
  const f0 = window.fetch.bind(window);
  window.fetch = async (...a: Parameters<typeof fetch>) => {
    const t = now();
    const r = await f0(...a);
    const json = r.json.bind(r);
    r.json = async () => {
      const v = await json();
      add("ipc", now() - t);
      pending.push(now());
      probe.port2.postMessage(0);
      return v;
    };
    return r;
  };
  // React's scheduler posts its work through a MessageChannel
  const MC = window.MessageChannel;
  window.MessageChannel = class extends MC {
    constructor() {
      super();
      const port = this.port1;
      let handler: ((e: MessageEvent) => void) | null = null;
      Object.defineProperty(port, "onmessage", {
        get: () => handler,
        set: (h: ((e: MessageEvent) => void) | null) => {
          handler = h;
          Object.getOwnPropertyDescriptor(MessagePort.prototype, "onmessage")?.set?.call(port, h ? (e: MessageEvent) => timed("react", () => h(e)) : null);
        },
      });
    }
  } as typeof MessageChannel;
  // input handlers and timers
  const ael = EventTarget.prototype.addEventListener;
  const wrapped = new WeakMap<object, EventListener>();
  EventTarget.prototype.addEventListener = function (type: string, l: EventListenerOrEventListenerObject | null, o?: boolean | AddEventListenerOptions) {
    if (l && /^(key|pointer|mouse|wheel|click)/.test(type) && typeof l === "function") {
      let w2 = wrapped.get(l);
      if (!w2) wrapped.set(l, (w2 = function (this: unknown, e: Event) { return timed("input", () => (l as EventListener).call(this, e)); } as EventListener));
      return ael.call(this, type, w2, o);
    }
    return ael.call(this, type, l, o);
  };
  const rel = EventTarget.prototype.removeEventListener;
  EventTarget.prototype.removeEventListener = function (type: string, l: EventListenerOrEventListenerObject | null, o?: boolean | EventListenerOptions) {
    return rel.call(this, type, (l && wrapped.get(l as object)) || l, o);
  };
  const st = window.setTimeout.bind(window);
  window.setTimeout = ((cb: TimerHandler, ms?: number, ...rest: unknown[]) => (typeof cb === "function" ? st(() => timed("timers", () => (cb as (...x: unknown[]) => void)(...rest)), ms) : st(cb, ms))) as typeof setTimeout;
  // Chromium: Long Animation Frames (script attribution, style and layout)
  try {
    new PerformanceObserver((l) => {
      if (S.on) for (const e of l.getEntries()) S.loaf.push(JSON.parse(JSON.stringify(e)));
    }).observe({ type: "long-animation-frame", buffered: false });
  } catch {
    /* WebKit: no LoAF */
  }
  // frame boundaries
  let last = now();
  const tick = (ts: number) => {
    if (S.on) {
      S.cur.gap = ts - last;
      S.frames.push(S.cur);
    }
    S.cur = {};
    last = ts;
    raf(tick);
  };
  raf(tick);
}

type Frame = Record<string, number>;
const KEYS = ["raf", "glDraw", "glUpload", "glTex", "glShader", "react", "store", "ipc", "input", "timers", "gpuFinish"];

function summarize(label: string, frames: Frame[]) {
  const fs = frames.filter((f) => f.gap > 0);
  const sorted = [...fs].sort((a, b) => a.gap - b.gap);
  const at = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))];
  const slow = fs.filter((f) => f.gap > 17.5);
  const avg: Frame = {};
  for (const f of slow) for (const k of [...KEYS, "gap"]) avg[k] = (avg[k] ?? 0) + (f[k] ?? 0) / Math.max(1, slow.length);
  const row = (name: string, f: Frame | undefined) => {
    if (!f) return `${name}: -`;
    const known = (f.raf ?? 0) + (f.react ?? 0) + (f.store ?? 0) + (f.input ?? 0) + (f.timers ?? 0);
    const parts = KEYS.filter((k) => (f[k] ?? 0) >= 0.05).map((k) => `${k} ${(f[k] ?? 0).toFixed(1)}`);
    return `${name}: frame ${f.gap.toFixed(1)} ms = ${parts.join(", ")}, other ${Math.max(0, f.gap - known).toFixed(1)}`;
  };
  const lines = [`${label}: ${fs.length} frames, ${slow.length} over 17.5 ms`, row("  p50", at(0.5)), row("  p95", at(0.95)), row("  worst", sorted[sorted.length - 1]), row("  slow avg", slow.length ? avg : undefined)];
  return lines.join("\n");
}

test("br_valley: where a nudge's and a drag-move's slow frames go", async ({ page, browserName }) => {
  test.skip(!existsSync(MAP), `needs ${MAP}`);
  test.setTimeout(600_000);
  await page.addInitScript(instrument, GPU);
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 4 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
  });
  await page.goto("/editor-dev.html");
  expect((await api(page, "/api/tools/editor/new", {})).error ?? null).toBeNull();
  if (TINY) {
    const d0 = await api(page, "/api/tools/editor/doc");
    await api(page, "/api/tools/editor/cmd", { op: "delete", brushes: d0.brushes.map((x: { id: number }) => x.id), entities: d0.entities.map((x: { id: number }) => x.id) });
    await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-64, -64, 0], max: [64, 64, 128], texture: "CRATE01" });
  } else expect((await api(page, "/api/tools/editor/cmd", { op: "paste", text: readFileSync(MAP, "utf8"), offset: [0, 0, 0] })).error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  await back.waitFor({ timeout: 30_000 });
  await back.click();
  if (!TINY) await page.waitForFunction(() => /\d{4} brushes/.test(document.querySelector(".tools-status")?.textContent || ""), null, { timeout: 120_000 });
  await page.waitForTimeout(4000);
  const top = (await page.locator('.ed-pane[data-pane="1"]').boundingBox())!;
  const cx = top.x + top.width / 2;
  const cy = top.y + top.height / 2;
  await page.mouse.move(cx, cy, { steps: 4 });
  await page.mouse.click(cx, cy);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked", { timeout: 10_000 });
  await page.waitForTimeout(1500);

  const run = async (act: () => Promise<void>) => {
    await page.evaluate(() => {
      const s = (window as unknown as { __split: { on: boolean; frames: unknown[]; loaf: unknown[]; cur: object } }).__split;
      s.frames = [];
      s.loaf = [];
      s.cur = {};
      s.on = true;
    });
    await act();
    return page.evaluate(() => {
      const s = (window as unknown as { __split: { on: boolean; frames: Frame[]; loaf: unknown[] } }).__split;
      s.on = false;
      return { frames: s.frames, loaf: s.loaf };
    });
  };
  const nudge = await run(async () => {
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press("ArrowRight");
      await page.waitForTimeout(250);
      await page.keyboard.press(`${MOD}+z`);
      await page.waitForTimeout(250);
    }
    await page.waitForTimeout(1000);
  });
  // drag the picked brush in the Top view (the select tool moves it, Hammer-style), then undo
  const drag = await run(async () => {
    for (let i = 0; i < 3; i++) {
      await page.mouse.move(cx, cy, { steps: 2 });
      await page.mouse.down();
      for (let k = 1; k <= 30; k++) await page.mouse.move(cx + k * 3, cy, { steps: 1 });
      await page.mouse.up();
      await page.waitForTimeout(400);
      await page.keyboard.press(`${MOD}+z`);
      await page.waitForTimeout(600);
    }
  });
  const gpuName = await page.evaluate(() => {
    const c = document.createElement("canvas").getContext("webgl2");
    const ext = c?.getExtension("WEBGL_debug_renderer_info");
    return c ? `${ext ? c.getParameter(ext.UNMASKED_RENDERER_WEBGL) : c.getParameter(c.RENDERER)}; timer query: ${c.getExtension("EXT_disjoint_timer_query_webgl2") ? "yes" : "no"}` : "no WebGL2";
  });
  const out = [`${browserName}${GPU ? " (gl.finish pass)" : ""}${TINY ? ", one-box map" : ", br_valley"}, ${gpuName}`, summarize("nudge+undo", nudge.frames), summarize("drag-move", drag.frames)];
  // Chromium's own attribution for the long frames (scripts by function, style/layout)
  for (const [label, r] of [
    ["nudge", nudge],
    ["drag", drag],
  ] as const) {
    const loaf = r.loaf as { duration: number; renderStart: number; styleAndLayoutStart: number; startTime: number; scripts: { invoker: string; sourceFunctionName: string; sourceURL: string; duration: number; forcedStyleAndLayoutDuration: number }[] }[];
    if (!loaf.length) continue;
    const by = new Map<string, number>();
    let style = 0;
    let render = 0;
    for (const e of loaf) {
      for (const sc of e.scripts) {
        const k = `${sc.invoker} ${sc.sourceFunctionName || "?"} ${sc.sourceURL.split("/").pop()?.split("?")[0]}`;
        by.set(k, (by.get(k) ?? 0) + sc.duration);
      }
      if (e.styleAndLayoutStart) style += e.startTime + e.duration - e.styleAndLayoutStart;
      if (e.renderStart) render += (e.styleAndLayoutStart || e.startTime + e.duration) - e.renderStart;
    }
    const top5 = [...by.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6).map(([k, t]) => `${(t / loaf.length).toFixed(1)} ${k}`);
    out.push(`  LoAF ${label}: ${loaf.length} long frames, per frame: rAF/render ${(render / loaf.length).toFixed(1)} ms, style+layout ${(style / loaf.length).toFixed(1)} ms; scripts: ${top5.join(" | ")}`);
  }
  const text = out.join("\n");
  console.log("SPLIT\n" + text);
  test.info().annotations.push({ type: "split", description: text });
});
