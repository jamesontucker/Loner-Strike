// The "updates waiting" bar for the other game (Parity A, QA4 area A): with CS 1.6 picked and
// Condition Zero set up in the same Half-Life folder but behind (its bot brain), CZ gets a bar of
// its own, and its Update installs into czero only. The app's /api/app answer is stubbed; the
// install request is caught, not run; the setup scan is stubbed too. Runs against the tools dev
// server (npm run test:tools).
import { expect, test } from "@playwright/test";

test("CZ set up but behind gets its own update bar, which updates CZ only", async ({ page }) => {
  const hl = "/fake/Half-Life";
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
          outdated: [],
          unmanaged: [],
          unbuilt: [],
          others: [{ id: "czero", dir: `${hl}/czero`, hl, outdated: ["foundation"] }],
        },
      },
    });
  });
  const installs: unknown[] = [];
  await page.route(/\/api\/setup\/install(\?|$)/, async (route) => {
    installs.push(route.request().postDataJSON());
    await route.fulfill({ json: { error: "not run in this test" } });
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
              { id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false },
              { id: "czero", name: "Counter-Strike: Condition Zero", installed: true, components: ["core", "foundation"], foreign: false },
            ],
          },
        ],
        catalog: [],
        native_mac: true,
      },
    }),
  );
  await page.goto("/");
  const bar = page.locator(".files-bar", { hasText: "Condition Zero" });
  await expect(bar).toBeVisible({ timeout: 20_000 });
  // (the app puts a word joiner after the hyphen in Loner-Strike)
  await expect(bar).toContainText(/Condition Zero\): 1 part of Loner-\u2060?Strike has updates waiting/);
  await expect(bar).toContainText("updated separately from the game you have picked");
  // the picked game (CS 1.6) has nothing waiting: no bar for it
  await expect(page.locator(".files-bar")).toHaveCount(1);
  await bar.getByRole("button", { name: "Update all" }).click();
  await expect.poll(() => installs.length).toBe(1);
  expect(installs[0]).toMatchObject({ hl, games: ["czero"], repair: true });
});
