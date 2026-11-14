// QA4-T2 fixes (docs/notes/QA4-T2.md, "Fixed"): toasts above the save bar, the spray maker in a
// small window, texture search over every texture file, a damaged map in the doctor, the radar's
// zoom buttons and words, and a way on instead of "Try again" after a dead end.
// Same harness as tools.spec.ts (tools-dev.html against a game folder). Nothing here writes to
// the game: saves and damaged maps are faked with page.route.
//   npx playwright test tests/qa4-t2-fixes.spec.ts --project large --project small --project webkit
import { expect, test, type Page } from "@playwright/test";

const MAP = process.env.LS_TOOLS_MAP || "de_aztec";
const IGNORED = [/status of 404/, /Download the React DevTools/, /GPU stall due to ReadPixels/, /WebGL: too many errors/];

function watch(page: Page) {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  return problems;
}

const open = (page: Page, address: string) => page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);

test("UX-M1: a save's message and its Undo are one toast, above the save bar", async ({ page }) => {
  const problems = watch(page);
  // a save that kept backups (as the app answers one), without saving anything
  await page.route("**/api/tools/spray/design/save", (r) =>
    r.fulfill({ status: 200, contentType: "application/json", headers: { "x-ls-undo": "1-2" }, body: JSON.stringify({ ok: true, id: "qa-t2", name: "QA spray" }) }),
  );
  await open(page, "spray");
  await expect(page.locator(".sc-stage svg")).toBeVisible({ timeout: 20_000 });
  const bar = page.locator(".tl-actionbar");
  await bar.getByRole("button", { name: "Save design" }).click();
  const toasts = page.locator(".toasts .toast");
  await expect(toasts).toHaveCount(1);
  await expect(toasts.first()).toContainText("Saved “QA spray”.");
  await expect(toasts.first().getByRole("button", { name: "Undo" })).toBeVisible();
  const t = (await toasts.first().boundingBox())!;
  const b = (await bar.boundingBox())!;
  expect(t.y + t.height, "the toast sits above the bar").toBeLessThanOrEqual(b.y + 1);
  expect(problems).toEqual([]);
});

test("UX-M2: the spray canvas is on the first screen of a small window; a new design says Not saved yet", async ({ page }) => {
  const problems = watch(page);
  await page.setViewportSize({ width: 800, height: 600 });
  await open(page, "spray");
  const stage = page.locator(".sc-stage");
  await expect(stage).toBeVisible({ timeout: 20_000 });
  const s = (await stage.boundingBox())!;
  const bar = (await page.locator(".tl-actionbar").boundingBox())!;
  expect(s.y, "the canvas starts on the first screen").toBeLessThan(bar.y - 120);
  const add = (await page.getByRole("button", { name: "Words", exact: true }).boundingBox())!;
  expect(add.y, "the canvas comes before the Add panel").toBeGreaterThan(s.y);
  await expect(page.locator(".tl-ab-status")).toContainText("Not saved yet");
  await expect(page.locator(".tl-ab-status")).not.toContainText("No unsaved changes");
  expect(problems).toEqual([]);
});

test("UX-M3: texture search looks in every texture file and says where each hit is", async ({ page }) => {
  const problems = watch(page);
  await open(page, "assets");
  const where = page.getByRole("combobox", { name: "Where from" });
  await expect(where).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".assets-count")).not.toContainText("Reading", { timeout: 30_000 });
  const picked = await where.inputValue();
  expect(picked, "starts on one texture file").toMatch(/\.wad$/i);
  // a texture that isn't in the file the page starts on
  const name = await page.evaluate(async (start) => {
    const j = (u: string) => fetch(u).then((r) => r.json());
    const wads: { name: string; folder: string; error?: string }[] = (await j("/api/tools/assets/wads")).wads;
    const list = async (w: { name: string; folder: string }) => ((await j(`/api/tools/assets/wad/${encodeURIComponent(w.folder)}/${encodeURIComponent(w.name)}`)).textures ?? []) as { name: string }[];
    const here = new Set((await list(wads.find((w) => `${w.folder}/${w.name}` === start)!)).map((t) => t.name.toLowerCase()));
    for (const w of wads.filter((w) => !w.error && `${w.folder}/${w.name}` !== start)) {
      const t = (await list(w)).find((t) => !here.has(t.name.toLowerCase()) && /^[a-z]/i.test(t.name));
      if (t) return t.name;
    }
    return null;
  }, picked);
  test.skip(!name, "only one texture file here");
  await page.getByRole("searchbox", { name: "Search textures" }).fill(name!);
  await expect(page.locator(".assets-count")).toContainText(`in every texture file`, { timeout: 30_000 });
  await expect(page.locator(".assets-count")).not.toContainText(/^0 textures/);
  await expect(page.locator(".assets-tile-where").first()).toContainText(/\.wad$/i);
  // picked by hand: only that file, with a way back to all of them
  await where.selectOption(picked);
  await expect(page.locator(".assets-count")).toContainText(/in .+\.wad/i);
  await page.getByRole("button", { name: "Search every texture file" }).click();
  await expect(page.locator(".assets-count")).toContainText("in every texture file");
  expect(problems).toEqual([]);
});

test("UX-M4: a map that can't be read shows only that problem in the doctor", async ({ page }) => {
  const problems = watch(page);
  await page.route(`**/api/tools/doctor/${MAP}`, async (r) => {
    const real = await (await r.fetch()).json();
    real.bsp = { ok: false, error: "This map file is damaged (it's shorter than a map header)." };
    await r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(real) });
  });
  await open(page, `doctor:${MAP}`);
  await expect(page.locator(".doc-verdict")).toContainText("1 problem", { timeout: 30_000 });
  await expect(page.locator(".doc-verdict")).toContainText("Other checks need a readable map.");
  await expect(page.getByText("All passed.")).toHaveCount(0);
  await expect(page.getByText(/checks passed/)).toHaveCount(0);
  await expect(page.getByText("header)..")).toHaveCount(0);
  const side = page.locator(".tools-side");
  for (const b of ["Map viewer", "Radar maker", "Simulate a match"]) await expect(side.getByRole("button", { name: b })).toHaveCount(0);
  expect(problems).toEqual([]);
});

test("UX-M5: the radar's zoom buttons stand out, and the status says what Install writes", async ({ page }) => {
  const problems = watch(page);
  await open(page, `radar:${MAP}`);
  const zoom = page.getByRole("group", { name: "Zoom" });
  await expect(zoom).toBeVisible({ timeout: 30_000 });
  const bg = await zoom.evaluate((el) => getComputedStyle(el).backgroundColor);
  const [r, g, b, a = 1] = bg.match(/[\d.]+/g)!.map(Number);
  expect(a, "a solid backing").toBeGreaterThan(0.6);
  expect(Math.max(r, g, b), "a dark backing").toBeLessThan(60);
  const status = page.locator(".tools-status .tools-hover");
  await expect(status).toContainText(/Writes (north up|sideways) \(Auto\)/);
  await page.getByRole("group", { name: "Turn" }).getByRole("button", { name: "Sideways" }).click();
  await expect(status).toContainText(/^Writes sideways(?! \(Auto\))/);
  expect(problems).toEqual([]);
});

test("UX-M6: a damaged map's radar and nav fixer point to the Map doctor, not Try again", async ({ page }) => {
  const problems = watch(page);
  const damaged = { error: "This map file is damaged (it's shorter than a map header)." };
  await page.route(`**/api/tools/overview/${MAP}`, (r) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(damaged) }));
  await page.route(`**/api/tools/navfix/${MAP}`, (r) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(damaged) }));
  for (const tool of ["radar", "navfix"]) {
    await open(page, `${tool}:${MAP}`);
    const err = page.locator(".load-error");
    await expect(err).toBeVisible({ timeout: 30_000 });
    await expect(err.getByRole("button", { name: "Try again" })).toHaveCount(0);
    await expect(err.getByRole("button", { name: "Check it in the Map doctor" })).toBeVisible();
  }
  await page.locator(".load-error").getByRole("button", { name: "Check it in the Map doctor" }).click();
  await expect(page).toHaveURL(/open=doctor(%3A|:)/);
  expect(problems).toEqual([]);
});

test("UX-M1: toasts sit above the save bar in Creature Studio and Plugin Studio too", async ({ page }) => {
  const problems = watch(page);
  for (const address of ["creatures", "plugins"]) {
    await open(page, address);
    const bar = page.locator(".tl-actionbar").first();
    await expect(bar).toBeVisible({ timeout: 30_000 });
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("ls-error", { detail: "A QA toast" })));
    const toast = page.locator(".toasts .toast", { hasText: "A QA toast" });
    await expect(toast).toBeVisible();
    await page.waitForTimeout(300);
    const t = (await toast.boundingBox())!;
    const b = (await bar.boundingBox())!;
    expect(t.y + t.height, `${address}: the toast sits above the bar`).toBeLessThanOrEqual(b.y + 1);
  }
  expect(problems.filter((p) => !p.includes("A QA toast"))).toEqual([]);
});
