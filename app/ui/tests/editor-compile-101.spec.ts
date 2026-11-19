// 1.0.1 map building: the Compile panel's stages with their times (and that each can take minutes),
// a playtest's steps with Stop through all of them and "Skip teaching, play now", teaching skipped
// when bots know this version and offered again when the layout changed, and the map's last
// compile and playtest logs (errors shown, Open the full log). Every app route is answered here:
// the long steps (compile, bots learning) are held or let go by the test.
//   LS_VITE_PORT=5334 npx playwright test editor-compile-101 [--project=large|small]
import { expect, test, type Page, type Route } from "@playwright/test";

const SHOTS = "/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/p101/build";
const STATUS = { installed: true, version: "1.3.1", bytes: 1, url: "", licence: "", folder: "", canRun: true, cantRun: null, suggestName: "qa_room", busy: false, cores: 12, threads: 10 };

type Stage = { id: string; name: string; label: string; state: string; seconds: number | null };
const STAGES = (vis: "running" | "done"): Stage[] => [
  { id: "csg", name: "Brushes (CSG)", label: "Compiling brushes…", state: "done", seconds: 3 },
  { id: "bsp", name: "Map (BSP)", label: "Building the map…", state: "done", seconds: 12 },
  { id: "vis", name: "Visibility (VIS)", label: "Working out what can be seen from where…", state: vis, seconds: 95 },
  { id: "rad", name: "Lighting (RAD)", label: "Lighting…", state: vis === "done" ? "done" : "waiting", seconds: vis === "done" ? 40 : null },
];

type Opts = { nav?: "none" | "current" | "changed" | "unknown"; hasNav?: boolean; teach?: boolean; holdCompile?: boolean; last?: unknown };

/** The panel with everything answered; `ctl` lets the compile and the bots' learning go. */
async function setUp(page: Page, o: Opts = {}) {
  const calls: { name: string; body?: unknown }[] = [];
  const ctl = { compileDone: !o.holdCompile, learnDone: false, learnSeen: 0 };
  const note = (name: string, json: unknown = { ok: true }) => (r: Route) => {
    calls.push({ name, body: r.request().method() === "POST" ? r.request().postDataJSON() : undefined });
    return r.fulfill({ json });
  };
  await page.addInitScript((teach) => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.compile.teach", teach ? "1" : "0");
    localStorage.setItem("ls.compile.bots", "8");
  }, o.teach ?? true);
  await page.route(/\/api\/tools\/compile\/status/, (r) => r.fulfill({ json: STATUS }));
  await page.route(/\/api\/tools\/compile\/start/, note("start", { ok: true, map: "qa_room" }));
  await page.route(/\/api\/tools\/compile\/cancel/, (r) => {
    calls.push({ name: "cancel" });
    ctl.compileDone = true;
    return r.fulfill({ json: { ok: true } });
  });
  let stopped = false;
  page.on("request", (q) => {
    if (q.url().includes("/compile/cancel")) stopped = true;
  });
  await page.route(/\/api\/tools\/compile\/progress/, (r) => {
    const running = !ctl.compileDone;
    const job = running
      ? { id: 1, kind: "compile", running: true, stage: "vis", label: "Working out what can be seen from where…", fraction: 0.42, seconds: 110, problems: [], leak: null, result: null, error: null, stopped: false, stages: STAGES("running") }
      : stopped
        ? { id: 1, kind: "compile", running: false, stage: "stopped", label: "Stopped", fraction: 0.42, seconds: 112, problems: [], leak: null, result: null, error: null, stopped: true, stages: STAGES("running").map((s) => (s.state === "running" ? { ...s, state: "stopped" } : s)) }
        : { id: 1, kind: "compile", running: false, stage: "done", label: "Done", fraction: 1, seconds: 150, problems: [], leak: null, result: { map: "qa_room", bsp: "/x/qa_room.bsp", bytes: 2_400_000, preset: "normal", installed: true, backup: null, hasNav: o.hasNav ?? o.nav !== "none", nav: { state: o.nav ?? "none" } }, error: null, stopped: false, stages: STAGES("done") };
    return r.fulfill({ json: { job, lines: ["> sdHLVIS qa_room"], next: 1, partial: "" } });
  });
  await page.route(/\/api\/tools\/compile\/logs\/open/, note("open"));
  await page.route(/\/api\/tools\/compile\/logs\?/, (r) => r.fulfill({ json: o.last ?? { compile: null, playtest: null } }));
  await page.route(/\/api\/tools\/compile\/playtest-log/, note("playtest-log"));
  await page.route(/\/api\/tools\/doctor\/qa_room\/res/, note("res"));
  await page.route(/\/api\/navs\/learn\/stop/, (r) => {
    calls.push({ name: "learn-stop" });
    ctl.learnDone = true;
    return r.fulfill({ json: { ok: true } });
  });
  await page.route(/\/api\/navs\/learn$/, (r) => {
    if (r.request().method() === "POST") return note("learn")(r);
    return r.fulfill({ json: { result: { message: "Bots learned qa_room.", learned: ["qa_room"] } } });
  });
  await page.route(/\/api\/setup\/job/, (r) => {
    ctl.learnSeen++;
    const state = ctl.learnDone ? "done" : "running";
    return r.fulfill({ json: { state, kind: "navs", steps: [{ id: "cstrike/nav-prep", label: "Getting a hidden server ready", state: "done", detail: "" }, { id: "cstrike/nav-0", label: "Learning qa_room (1 of 1)", state: ctl.learnDone ? "done" : "running", detail: "A bot is walking the map (3 min so far)" }] } });
  });
  await page.route(/\/api\/state$/, (r) => r.fulfill({ json: { online: false, game_open: false } }));
  await page.route(/\/api\/app\/launch/, note("launch"));

  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Compile$/ }).first().click();
  await expect(page.locator(".compile-panel").getByRole("textbox", { name: /^Map name/ })).toHaveValue("qa_room");
  return { calls, ctl };
}

const shot = async (page: Page, name: string, el = ".compile-panel") => {
  const size = page.viewportSize()!.width >= 1440 ? "1440" : "1024";
  await page.locator(el).first().scrollIntoViewIfNeeded();
  await page.screenshot({ path: `${SHOTS}/${name}-${size}.png` });
};
const playtest = (page: Page) => page.locator(".compile-playtest").getByRole("button", { name: "Playtest", exact: true }).click();
const step = (page: Page, label: string) => page.locator(".compile-step", { hasText: label });

test("compile stages show their times, and that each can take minutes", async ({ page }) => {
  const { ctl } = await setUp(page, { holdCompile: true });
  await page.locator(".compile-panel").getByRole("button", { name: "Compile", exact: true }).click();
  const stages = page.getByRole("list", { name: "Compile stages" });
  await expect(stages.locator("li")).toHaveCount(4);
  await expect(stages.locator("li").nth(2)).toContainText("Visibility (VIS)");
  await expect(stages.locator("li").nth(2)).toContainText("Running");
  await expect(stages.locator("li").nth(2)).toContainText("1 min 35 s");
  await expect(stages.locator("li").nth(3)).toContainText("Waiting");
  await expect(page.locator(".compile-panel")).toContainText("Each stage can take from a few seconds to several minutes");
  await expect(page.getByRole("progressbar")).toContainText("Working out what can be seen from where…");
  await expect(page.locator(".compile-panel").getByRole("button", { name: "Stop", exact: true })).toBeVisible();
  await shot(page, "01-compile-stages", ".compile-stages-box");
  ctl.compileDone = true;
  // done: each stage's time stays, the note about minutes goes
  await expect(stages.locator("li").nth(3)).toContainText("40 s");
  await expect(page.locator(".compile-panel")).not.toContainText("Each stage can take from a few seconds");
  await shot(page, "02-compile-done", ".compile-stages-box");
});

test("playtest on a map bots don't know: teaching shows its time, Skip teaching plays now", async ({ page }) => {
  const { calls, ctl } = await setUp(page, { nav: "none", hasNav: false });
  await playtest(page);
  const teach = step(page, "Teach bots the map");
  await expect(teach).toContainText("Running", { timeout: 15_000 });
  await expect(teach).toContainText("A bot is walking the map. Usually a few minutes; bigger maps take longer.");
  await expect(teach).not.toContainText("so far)");
  await expect(step(page, "Compile")).toContainText("Done");
  await expect(step(page, "Start the game")).toContainText("Waiting");
  const box = page.locator(".compile-steps-box");
  await expect(box.getByRole("button", { name: "Stop", exact: true })).toBeVisible();
  await expect(box.getByRole("button", { name: "Skip teaching, play now" })).toBeVisible();
  await expect(box).toContainText("Skip starts the game now. Bots will wander until they learn the map.");
  // the time ticks
  await expect(teach.locator(".compile-step-time")).toHaveText(/^[1-9]\d* s$/, { timeout: 4000 });
  await shot(page, "03-teaching", ".compile-steps-box");
  await box.getByRole("button", { name: "Skip teaching, play now" }).click();
  await expect.poll(() => calls.some((c) => c.name === "learn-stop")).toBe(true);
  await expect(teach).toContainText("Skipped", { timeout: 20_000 });
  await expect(teach).toContainText("Skipped: bots will wander until they learn the map.");
  await expect.poll(() => calls.some((c) => c.name === "launch"), { timeout: 15_000 }).toBe(true);
  await expect(step(page, "Start the game")).toContainText("Done");
  await expect(page.locator(".compile-steps-box")).toContainText("Playtest steps", { timeout: 10_000 });
  await expect(box.getByRole("button", { name: "Stop", exact: true })).toHaveCount(0);
  const log = calls.find((c) => c.name === "playtest-log")?.body as { map: string; outcome: string; lines: string[]; learner: boolean };
  expect(log.map).toBe("qa_room");
  expect(log.outcome).toBe("ok");
  expect(log.learner).toBe(true);
  expect(log.lines.find((l) => l.startsWith("Teach bots the map"))).toContain("skipped");
  expect(ctl.learnSeen).toBeGreaterThan(0);
  await shot(page, "04-skipped-played", ".compile-steps-box");
});

test("Stop while bots learn ends the playtest: no game started", async ({ page }) => {
  const { calls } = await setUp(page, { nav: "changed" });
  await playtest(page);
  const teach = step(page, "Teach bots the map");
  await expect(teach).toContainText("Running", { timeout: 15_000 });
  await page.locator(".compile-steps-box").getByRole("button", { name: "Stop", exact: true }).click();
  await expect(teach).toContainText("Stopped while bots were learning the map.", { timeout: 20_000 });
  await expect(step(page, "Start the game")).toContainText("Not run");
  expect(calls.some((c) => c.name === "launch")).toBe(false);
  await expect.poll(() => (calls.find((c) => c.name === "playtest-log")?.body as { outcome?: string } | undefined)?.outcome).toBe("stopped");
  await shot(page, "05-stopped-teaching", ".compile-steps-box");
});

test("Stop during the playtest's compile stops the compile (Stop stays through the steps)", async ({ page }) => {
  const { calls } = await setUp(page, { holdCompile: true });
  await playtest(page);
  const box = page.locator(".compile-steps-box");
  await expect(step(page, "Compile")).toContainText("Running");
  // one Stop only: the playtest's (the Compile row's is for a plain compile)
  await expect(page.locator(".compile-panel").getByRole("button", { name: "Stop", exact: true })).toHaveCount(1);
  await shot(page, "06-playtest-compiling");
  await box.getByRole("button", { name: "Stop", exact: true }).click();
  await expect.poll(() => calls.some((c) => c.name === "cancel")).toBe(true);
  await expect(step(page, "Compile")).toContainText("Stopped", { timeout: 10_000 });
  await expect(step(page, "Teach bots the map")).toContainText("Not run");
  expect(calls.some((c) => c.name === "learn" || c.name === "launch")).toBe(false);
});

test("bots who know this version aren't taught again", async ({ page }) => {
  const { calls } = await setUp(page, { nav: "current" });
  await playtest(page);
  await expect(step(page, "Teach bots the map")).toContainText("Bots already know this version of the map.", { timeout: 15_000 });
  await expect.poll(() => calls.some((c) => c.name === "launch"), { timeout: 15_000 }).toBe(true);
  expect(calls.some((c) => c.name === "learn")).toBe(false);
});

test("a changed map with teaching off: offered to teach bots again (Play now, or Teach bots)", async ({ page }) => {
  const { calls, ctl } = await setUp(page, { nav: "changed", teach: false });
  await playtest(page);
  const ask = page.getByRole("alertdialog");
  await expect(ask).toContainText("You changed the map's layout since bots last learned it", { timeout: 15_000 });
  await shot(page, "07-changed-ask", ".compile-steps-box");
  await ask.getByRole("button", { name: "Teach bots" }).click();
  await expect(step(page, "Teach bots the map")).toContainText("Running");
  expect(calls.some((c) => c.name === "learn")).toBe(true);
  ctl.learnDone = true;
  await expect(step(page, "Teach bots the map")).toContainText("Bots learned the map.", { timeout: 15_000 });
  await expect.poll(() => calls.some((c) => c.name === "launch"), { timeout: 15_000 }).toBe(true);
});

test("a changed map, Play now: the game starts on the old knowledge, and says so", async ({ page }) => {
  const { calls } = await setUp(page, { nav: "changed", teach: false });
  await playtest(page);
  await page.getByRole("alertdialog").getByRole("button", { name: "Play now" }).click({ timeout: 15_000 });
  await expect(step(page, "Teach bots the map")).toContainText("they may get lost where you changed it");
  await expect.poll(() => calls.some((c) => c.name === "launch"), { timeout: 15_000 }).toBe(true);
  expect(calls.some((c) => c.name === "learn")).toBe(false);
});

test("the map's last compile and playtest: errors shown, and the full logs open", async ({ page }) => {
  const when = Math.round(Date.now() / 1000) - 300;
  const last = {
    compile: { map: "qa_room", when, outcome: "failed", error: "The map leaks, so it can't be finished. Close the gap the red line goes through, then compile again.", seconds: 74, preset: "normal", errors: 2, warnings: 1, problems: [{ level: "error", kind: "leak", message: "The map leaks: there's a gap to the outside" }, { level: "error", kind: "brush", message: "Brush 12 is inside out" }, { level: "warning", kind: "texture", message: "A texture is missing" }], log: "/x/compile.txt" },
    playtest: { map: "qa_room", when: when + 60, outcome: "stopped", seconds: 30, log: "/x/playtest.txt" },
  };
  const { calls } = await setUp(page, { last });
  const box = page.getByTestId("compile-last");
  await expect(box).toContainText("Last compile of qa_room: didn't finish 5 min ago, took 1 min 14 s, 2 problems, 1 warning.");
  await expect(box).toContainText("The map leaks: there's a gap to the outside");
  await expect(box).toContainText("Brush 12 is inside out");
  await expect(box).toContainText("The map leaks, so it can't be finished.");
  await expect(box).not.toContainText("A texture is missing");
  await expect(box).toContainText("Last playtest: stopped 4 min ago.");
  await shot(page, "08-last-logs", "[data-testid=compile-last]");
  await box.getByRole("button", { name: "Open the full log" }).click();
  await box.getByRole("button", { name: "Open the playtest log" }).click();
  await expect.poll(() => calls.filter((c) => c.name === "open").map((c) => c.body)).toEqual([{ map: "qa_room", which: "compile" }, { map: "qa_room", which: "playtest" }]);
});
