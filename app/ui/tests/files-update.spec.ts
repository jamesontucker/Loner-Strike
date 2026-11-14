// The "updates waiting" bar (Parity A, QA4 area A): with CS 1.6 picked and Condition Zero set up
// in the same Half-Life folder, both behind, there is one bar naming both; Update all updates
// CS 1.6 and then CZ, Details has a per-game Update, × hides it until new updates appear. With
// only one game behind it is the short single-game bar. The app's /api/app answer, the setup scan
// and the setup job are stubbed; install requests are caught, not run. Runs against the tools dev
// server (npm run test:tools).
import { expect, test, type Page } from "@playwright/test";

const hl = "/fake/Half-Life";

async function stub(page: Page, game: { outdated: string[]; others: string[] }) {
  const state = { ...game };
  await page.route(/\/api\/app(\?|$)/, async (route) => {
    const r = await route.fetch().catch(() => null);
    const base = r ? await r.json().catch(() => ({})) : {};
    await route.fulfill({
      json: {
        ...base,
        version: base.version ?? "1.0.0",
        game: {
          id: "cstrike",
          dir: `${hl}/cstrike`,
          hl,
          dedicated: false,
          installed: ["core", "foundation"],
          outdated: state.outdated,
          unmanaged: [],
          unbuilt: [],
          others: state.others.length ? [{ id: "czero", dir: `${hl}/czero`, hl, outdated: state.others }] : [],
        },
      },
    });
  });
  // each install starts a job that is running on its first look and done after
  const installs: { games: string[]; hl: string; repair: boolean }[] = [];
  let looks = 0;
  await page.route(/\/api\/setup\/install(\?|$)/, async (route) => {
    installs.push(route.request().postDataJSON());
    looks = 0;
    await route.fulfill({ json: { ok: true } });
  });
  await page.route(/\/api\/setup\/job(\?|$)/, async (route) => {
    const n = installs.length;
    if (!n) return route.fulfill({ json: { state: "idle", steps: [], progress: 0 } });
    looks += 1;
    const running = looks < 3;
    await route.fulfill({ json: { id: `job${n}`, state: running ? "running" : "done", steps: [], progress: running ? 0.5 : 1 } });
  });
  // the scan: both games set up in this Half-Life folder (the tools dev server has no setup routes)
  await page.route(/\/api\/setup\/scan(\?|$)/, (route) =>
    route.fulfill({
      json: {
        installs: [
          {
            hl,
            kind: "crossover",
            bottle: "Steam",
            games: [
              { id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation", "mode-deathrun"], foreign: false },
              { id: "czero", name: "Counter-Strike: Condition Zero", installed: true, components: ["core", "foundation"], foreign: false },
            ],
          },
        ],
        catalog: [{ id: "mode-deathrun", name: "Deathrun" }],
        native_mac: true,
      },
    }),
  );
  return { installs, state };
}

test("both games behind: one bar, Update all does CS 1.6 then CZ", async ({ page }) => {
  const { installs } = await stub(page, { outdated: ["foundation", "mode-deathrun"], others: ["foundation"] });
  await page.goto("/");
  const bar = page.locator(".files-bar");
  await expect(bar).toBeVisible({ timeout: 20_000 });
  await expect(bar).toHaveCount(1);
  await expect(bar).toContainText("Updates waiting: Counter-Strike 1.6 (2 parts) and Condition Zero (1 part).");
  await expect(bar).toContainText("Your servers update on their own.");
  await bar.getByRole("button", { name: "Update all" }).click();
  await expect(page.locator(".files-update")).toContainText(/Updating (Counter-Strike 1\.6… \(1 of 2\)|Condition Zero… \(2 of 2\))/);
  await expect.poll(() => installs.length, { timeout: 20_000 }).toBe(2);
  expect(installs[0]).toMatchObject({ hl, games: ["cstrike"], repair: true, components: ["mode-deathrun"] });
  expect(installs[1]).toMatchObject({ hl, games: ["czero"], repair: true });
  // both done: the bar is back (the stub still says they're behind), no progress left
  await expect(page.locator(".files-update")).toHaveCount(0, { timeout: 20_000 });
  await expect(bar).toBeVisible();
});

test("Details lists the parts per game, each with its own Update", async ({ page }) => {
  const { installs } = await stub(page, { outdated: ["foundation", "mode-deathrun"], others: ["foundation"] });
  await page.goto("/");
  const bar = page.locator(".files-bar");
  await expect(bar).toBeVisible({ timeout: 20_000 });
  await bar.getByRole("button", { name: "Details" }).click();
  const cz = page.locator(".files-update-game", { hasText: "Condition Zero" });
  const cs = page.locator(".files-update-game", { hasText: "Counter-Strike 1.6" });
  await expect(cs).toContainText("Deathrun");
  await expect(cs).toContainText("The bot brain (ReGameDLL)");
  await expect(cz).toContainText("The bot brain (ReGameDLL)");
  await cz.getByRole("button", { name: "Update" }).click();
  await expect.poll(() => installs.length).toBe(1);
  expect(installs[0]).toMatchObject({ hl, games: ["czero"], repair: true });
  await expect(page.locator(".files-update")).toContainText("Updating Condition Zero…");
  await expect(page.locator(".files-update")).not.toContainText("of 2");
});

test("one game behind: the short bar; × hides it until new updates appear", async ({ page }) => {
  const { installs, state } = await stub(page, { outdated: [], others: ["foundation"] });
  await page.goto("/");
  const bar = page.locator(".files-bar");
  await expect(bar).toBeVisible({ timeout: 20_000 });
  await expect(bar).toHaveCount(1);
  await expect(bar).toContainText("Updates waiting: Condition Zero (1 part).");
  await expect(bar).not.toContainText("Counter-Strike 1.6");
  await bar.getByRole("button", { name: "Hide until new updates" }).click();
  await expect(bar).toHaveCount(0);
  // the same updates after a restart: still hidden
  await page.reload();
  await expect(page.locator(".content")).toBeVisible({ timeout: 20_000 });
  await page.waitForTimeout(1500);
  await expect(bar).toHaveCount(0);
  // something new: it comes back
  state.outdated = ["mode-deathrun"];
  await page.reload();
  await expect(bar).toContainText("Updates waiting: Counter-Strike 1.6 (1 part) and Condition Zero (1 part).", { timeout: 20_000 });
  expect(installs.length).toBe(0);
});
