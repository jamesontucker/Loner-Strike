// The map simulator (tools/health): its name on the overview and the report's map, rounds
// and fights. Needs a report in the dev server's app folder: make one from a sim recording
// with `cargo test report_from_recording -- --ignored` (see health.rs) and LS_APP_DIR.
//   npx playwright test tests/sim-report.spec.ts --project large
//   LS_BROWSER=webkit npx playwright test tests/sim-report.spec.ts --project large
import { expect, test } from "@playwright/test";

test.use({ browserName: (process.env.LS_BROWSER as "chromium" | "webkit" | undefined) ?? "chromium" });

test("the simulator and the doctor have names that say what they do", async ({ page }) => {
  await page.goto("/tools-dev.html?open=overview");
  await expect(page.getByText("Map simulator", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("Map health check")).toHaveCount(0);
  await page.goto("/tools-dev.html?open=health");
  await expect(page.getByRole("heading", { name: "Simulate a match" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Run the simulation" })).toBeVisible();
});

test("a report shows the map with layers, rounds and fights", async ({ page, request }, info) => {
  const list = await (await request.get("/api/tools/health/reports")).json();
  const id = list.reports?.[0]?.id;
  test.skip(!id, "no simulation report in the dev app folder");
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`/tools-dev.html?open=health:report:${id}`);
  await expect(page.getByRole("heading", { name: "The map", exact: true })).toBeVisible();
  const svg = page.locator(".sim-map-svg");
  await expect(svg).toBeVisible({ timeout: 30_000 });
  await expect(svg.locator("image")).toHaveCount(1, { timeout: 30_000 });
  // the default layer has cells, and the pointer readout says what's there
  await expect(svg.locator(".sim-heat rect").first()).toBeAttached();
  const n = await svg.locator(".sim-heat rect").count();
  expect(n).toBeGreaterThan(20);
  const cell = svg.locator(".sim-heat rect").nth(Math.floor(n / 2));
  await cell.hover({ force: true });
  await expect(page.locator(".sim-map-readout")).toContainText(/bot time|bot-minutes|Falls|Stuck|Fights|Map kills/);
  for (const name of ["Where they died", "Where kills came from", "Where nobody went", "Where bots went"]) {
    await page.getByRole("radio", { name }).or(page.getByRole("button", { name })).first().click();
    await expect(page.locator(".sim-map-legend")).toBeVisible();
  }
  await expect(page.locator(".sim-pin").first()).toBeAttached();
  await expect(page.getByRole("heading", { name: /Where fights happen/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Rounds and sides" })).toBeVisible();
  // nothing sticks out sideways
  const wide = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(wide).toBeLessThanOrEqual(1);
  // (LS_SHOTS: a folder of your own; test-results is shared with every other run)
  const dir = process.env.LS_SHOTS ?? info.outputPath();
  const b = process.env.LS_BROWSER ?? "chromium";
  await page.screenshot({ path: `${dir}/sim-report-${b}.png`, fullPage: true });
  for (const name of ["Where they died", "Where nobody went", "Where bots went"]) {
    await page.locator(".seg button", { hasText: name }).click();
    await page.locator(".sim-map").screenshot({ path: `${dir}/sim-map-${b}-${name.split(" ").pop()}.png` });
  }
  expect(errors).toEqual([]);
});

test("the map picture is still there after leaving the report and coming back", async ({ page, request }) => {
  const list = await (await request.get("/api/tools/health/reports")).json();
  const id = list.reports?.[0]?.id;
  test.skip(!id, "no simulation report in the dev app folder");
  const pictureLoads = async () => {
    const href = await page.locator(".sim-map-svg image").getAttribute("href", { timeout: 30_000 });
    expect(href).toMatch(/^blob:/);
    // (a revoked object URL fails to fetch: that was the blank map)
    return page.evaluate(async (u) => (await fetch(u!)).ok, href).catch(() => false);
  };
  await page.goto(`/tools-dev.html?open=health:report:${id}`);
  await expect(page.locator(".sim-map-svg image")).toHaveCount(1, { timeout: 30_000 });
  expect(await pictureLoads()).toBe(true);
  for (let i = 0; i < 2; i++) {
    await page.getByRole("button", { name: "Back" }).first().click();
    await expect(page.getByRole("heading", { name: "Earlier simulations" })).toBeVisible();
    await page.locator(".health-row").first().click();
    await expect(page.locator(".sim-map-svg image")).toHaveCount(1, { timeout: 30_000 });
    expect(await pictureLoads()).toBe(true);
  }
  // and from another tool and back
  await page.getByRole("link", { name: "Map doctor" }).or(page.getByRole("button", { name: "Map doctor" })).first().click();
  await page.goBack();
  await expect(page.locator(".sim-map-svg image")).toHaveCount(1, { timeout: 30_000 });
  expect(await pictureLoads()).toBe(true);
});

test("worth-a-look findings with a spot open Replay at the moment", async ({ page, request }) => {
  const list = await (await request.get("/api/tools/health/reports")).json();
  const ids: string[] = (list.reports ?? []).map((r: { id: string }) => r.id);
  let found: { id: string; t: number } | null = null;
  for (const id of ids) {
    const r = (await (await request.get(`/api/tools/health/report/${id}`)).json()).report;
    const f = r?.findings?.find((x: { spot?: { t?: number } }) => x.spot?.t != null);
    if (r?.recording && f) {
      found = { id, t: f.spot.t };
      break;
    }
  }
  test.skip(!found, "no report with a finding at a moment in the dev app folder");
  await page.goto(`/tools-dev.html?open=health:report:${found!.id}`);
  await page.locator(".health-finding").getByRole("button", { name: "Watch in Replay" }).first().click();
  await expect(page).toHaveURL(/open=replay(%3A|:)/, { timeout: 15_000 });
  const slider = page.getByRole("slider", { name: "Time" });
  await expect(slider).toBeVisible({ timeout: 30_000 });
  const v = Number(await slider.inputValue());
  expect(Math.abs(v - Math.max(0, found!.t - 3))).toBeLessThan(1.5);
});
