// QA4 T1 (M5, M2): the map simulator keeps its picks after Stop and after leaving the page;
// Replay's cards say where each recording came from, how long it is, and "1 player".
// Runs on mocked routes (no game or Rust server needed; Vite alone is fine):
//   npx playwright test tests/sim-replay-qa4.spec.ts --project large
//   LS_BROWSER=webkit npx playwright test tests/sim-replay-qa4.spec.ts --project large
import { expect, test, type Page } from "@playwright/test";

test.use({ browserName: (process.env.LS_BROWSER as "chromium" | "webkit" | undefined) ?? "chromium" });

const json = (body: unknown) => ({ status: 200, contentType: "application/json", body: JSON.stringify(body) });

async function mockSim(page: Page) {
  const state: { run: Record<string, unknown> | null; active: boolean; started: Record<string, unknown>[] } = { run: null, active: false, started: [] };
  await page.route("**/api/tools/maps", (r) => r.fulfill(json({ maps: [{ name: "de_dust2", title: "Dust II", nav: true }, { name: "de_nuke", title: "Nuke", nav: true }] })));
  await page.route("**/api/tools/health/options?**", (r) => {
    const map = new URL(r.request().url()).searchParams.get("map");
    r.fulfill(
      json({
        map, game: "cstrike", hasNav: true, hasKnowledge: true, servers: [{ id: "s1", name: "First" }, { id: "s2", name: "Second" }], server: "s1",
        modes: [{ id: "classic", name: "Classic" }, { id: "dm", name: "Deathmatch" }], mode: "classic", bots: 12, minutes: 5,
        limits: { minutes: [2, 20], bots: [2, 32] }, why: null, busy: null, running: false,
      }),
    );
  });
  await page.route("**/api/tools/health/progress", (r) => r.fulfill(json(state.active ? { run: state.run, active: true, elapsed: 5, left: 100, fraction: 0.1 } : { run: state.run, active: false })));
  await page.route("**/api/tools/health/reports", (r) => r.fulfill(json({ reports: [] })));
  await page.route("**/api/tools/health/start", async (r) => {
    const req = r.request().postDataJSON();
    state.started.push(req);
    state.run = { id: "r1", ...req, modeName: req.mode, phase: "playing", detail: "", started: 1, playingSince: 1, error: null, report: null };
    state.active = true;
    r.fulfill(json({ ok: true, id: "r1" }));
  });
  await page.route("**/api/tools/health/cancel", (r) => {
    state.active = false;
    state.run = { ...state.run!, phase: "stopped" };
    r.fulfill(json({ ok: true }));
  });
  return state;
}

test("the simulator keeps its picks after Stop and after leaving", async ({ page }) => {
  const state = await mockSim(page);
  await page.goto("/tools-dev.html?open=health");
  await page.getByLabel("Map", { exact: true }).selectOption("de_nuke");
  await page.getByLabel("Game mode").selectOption("dm");
  await page.getByRole("radio", { name: "2 min" }).or(page.locator(".seg button", { hasText: "2 min" })).first().click();
  await page.getByRole("radio", { name: "6", exact: true }).or(page.locator(".seg button", { hasText: /^6$/ })).first().click();
  await page.getByLabel("Server to copy").selectOption("s2");
  await page.getByRole("button", { name: "Run the simulation" }).click();
  await expect(page.getByRole("button", { name: "Stop" })).toBeVisible();
  expect(state.started[0]).toMatchObject({ map: "de_nuke", mode: "dm", minutes: 2, bots: 6, server: "s2" });
  await page.getByRole("button", { name: "Stop" }).click();
  await expect(page.getByText("The simulation of de_nuke was stopped.")).toBeVisible();
  const checkForm = async () => {
    await expect(page.getByLabel("Map", { exact: true })).toHaveValue("de_nuke");
    await expect(page.getByLabel("Game mode")).toHaveValue("dm");
    await expect(page.getByLabel("Server to copy")).toHaveValue("s2");
    await page.getByRole("button", { name: "Run the simulation" }).click();
    await expect(page.getByRole("button", { name: "Stop" })).toBeVisible();
    expect(state.started[state.started.length - 1]).toMatchObject({ map: "de_nuke", mode: "dm", minutes: 2, bots: 6, server: "s2" });
    await page.getByRole("button", { name: "Stop" }).click();
  };
  await checkForm();
  // (Setup unmounts while a run goes on, so coming back to the form is the same path as leaving the page)
  await checkForm();
});

test("Replay cards say where each recording came from", async ({ page }) => {
  await page.route("**/api/tools/recordings", (r) =>
    r.fulfill(
      json({
        folders: ["/tmp/recordings"],
        recordings: [
          { id: "a", file: "health-1_br_valley_1.csv", map: "br_valley", when: 1794685498, size: 100, hasTrace: true, seconds: 1500, bots: 12, players: 12, kills: 31, rounds: 3, source: "Map simulator, Battle Royale, 12 bots, 25 min" },
          { id: "b", file: "demo-TopFrag_br_valley_1.csv", map: "br_valley", when: 1794685398, size: 100, hasTrace: false, seconds: 1595, bots: 0, players: 1, kills: 0, rounds: 0, source: "Demo TopFrag" },
          { id: "c", file: "br_valley_1.csv", map: "br_valley", when: 1794685298, size: 100, hasTrace: false, seconds: 200, bots: 1, players: 1, kills: 1, rounds: 1, source: "File br_valley_1.csv" },
        ],
      }),
    ),
  );
  await page.goto("/tools-dev.html?open=replay");
  const cards = page.locator(".tools-rec");
  await expect(cards).toHaveCount(3);
  await expect(cards.nth(0)).toContainText("Map simulator, Battle Royale, 12 bots, 25 min");
  await expect(cards.nth(0)).toContainText("25 min · 12 bots · 31 kills");
  await expect(cards.nth(1)).toContainText("Demo TopFrag");
  await expect(cards.nth(1)).toContainText("27 min · 1 player");
  await expect(cards.nth(1)).not.toContainText("0 bots");
  await expect(cards.nth(1)).not.toContainText("kills");
  await expect(cards.nth(2)).toContainText("3 min 20 s · 1 bot · 1 kill");
  await expect(page.getByText("1 players")).toHaveCount(0);
});

test("Run the simulation looks disabled until a map is picked", async ({ page }) => {
  await mockSim(page);
  await page.goto("/tools-dev.html?open=health");
  const run = page.getByRole("button", { name: "Run the simulation" });
  await expect(run).toBeDisabled();
  const look = () => run.evaluate((b) => { const c = getComputedStyle(b); return `${c.backgroundColor} ${c.backgroundImage} ${c.borderStyle}`; });
  const off = await look();
  expect(off).toContain("dashed");
  await page.getByLabel("Map", { exact: true }).selectOption("de_dust2");
  await expect(run).toBeEnabled();
  expect(await look()).not.toContain("dashed");
});

test("Replay errors a retry can't fix offer Back, not Try again", async ({ page }) => {
  await page.route("**/api/tools/recordings", (r) =>
    r.fulfill(json({ folders: [], recordings: [{ id: "bad", file: "x.csv", map: "de_dust2", when: 1, size: 1, hasTrace: false, seconds: 60, bots: 1, players: 1, kills: 0, rounds: 0, source: "File x.csv" }] })),
  );
  await page.route("**/api/tools/recording/bad", (r) => r.fulfill({ status: 422, contentType: "application/json", body: JSON.stringify({ error: "This file isn't a recording (no players, kills or rounds in it)." }) }));
  await page.goto("/tools-dev.html?open=replay:bad");
  await expect(page.getByText("This file isn't a recording")).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toHaveCount(0);
  await page.getByRole("button", { name: "Back to recordings" }).click();
  await expect(page.locator(".tools-rec")).toHaveCount(1);
});
