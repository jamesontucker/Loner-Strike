// Game Mode Studio: node help (the ? menu, F1 on a picked node, search, an example dropped in),
// the example modes on the start screen and the tour (offered once, every step, Skip, from the
// ? menu again). Nothing is installed or played: those routes are answered in the page.
// Chromium and WebKit: `npx playwright test studio-help [--browser webkit]`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_help";
const WRITES = ["compile", "check", "testrun", "testrun/cancel", "playtest", "switch", "source", "uninstall/**", "debug/command"];

async function seed(page: Page) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "Help check";
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok).toBeTruthy();
}

test.beforeEach(async ({ page }) => {
  for (const r of WRITES) await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test)", installedAs: ID } }));
  await seed(page);
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("node help: the ? menu, search, F1 on a node, an example dropped in", async ({ page }) => {
  // (the tour isn't offered: it has been seen)
  await page.addInitScript(() => localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false })));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await page.getByTestId("studio-help-menu").click();
  await page.getByRole("menuitem", { name: /Node help/ }).click();
  const help = page.getByTestId("studio-help");
  await expect(help.getByRole("option").first()).toBeVisible();
  await help.getByRole("searchbox").fill("gravity");
  await expect(help.getByRole("option").first()).toContainText("Gravity");
  await help.getByRole("searchbox").press("Enter");
  await expect(help.getByTestId("studio-help-entry")).toContainText("Pins");
  await help.getByRole("button", { name: "Close node help" }).click();
  await expect(help).toBeHidden();

  // F1 on a picked node: its own help
  await page.locator('.react-flow__node[data-id="give_ak"]').click();
  await page.keyboard.press("F1");
  await expect(help.getByTestId("studio-help-entry")).toContainText("Give Weapon");
  const before = await page.locator(".react-flow__node").count();
  await help.getByTestId("studio-help-example").click();
  await expect.poll(() => page.locator(".react-flow__node").count()).toBeGreaterThan(before);
  // Escape closes it
  await help.getByRole("button", { name: /all nodes/i }).focus();
  await page.keyboard.press("Escape");
  await expect(help).toBeHidden();
});

test("example modes and the tour: offered once, every step, skip, again from the ? menu", async ({ page }) => {
  await page.addInitScript(() => {
    if (!sessionStorage.getItem("pw-cleared")) {
      localStorage.removeItem("ls.studio.tour");
      sessionStorage.setItem("pw-cleared", "1");
    }
  });
  await page.goto("/tools-dev.html?open=studio");
  await expect(page.getByTestId("studio-examples").getByRole("button")).toHaveCount(5);
  const offer = page.getByTestId("studio-tour-offer");
  await expect(offer).toBeVisible();
  await offer.getByRole("button", { name: "Take the tour" }).click();
  const tour = page.getByTestId("studio-tour");
  await expect(tour).toContainText("Welcome");
  await expect(page.locator(".st-name")).toContainText("Infection (tour)");
  // every step, each pointing at its control
  let steps = 1;
  while ((await tour.getByRole("button", { name: "Next" }).count()) > 0) {
    await tour.getByRole("button", { name: "Next" }).click();
    steps++;
    if (steps > 2) await expect(page.locator(".st-tour-ring")).toBeVisible();
  }
  expect(steps).toBeGreaterThanOrEqual(8);
  await tour.getByRole("button", { name: "Done" }).click();
  await expect(tour).toBeHidden();
  // not offered again; the ? menu starts it, and Skip ends it
  await page.getByTestId("studio-help-menu").click();
  await page.getByRole("menuitem", { name: "Take the Studio tour" }).click();
  await expect(tour).toContainText("Welcome");
  await tour.getByRole("button", { name: "Skip the tour" }).click();
  await expect(tour).toBeHidden();
  await page.goto("/tools-dev.html?open=studio");
  await expect(page.getByTestId("studio-examples")).toBeVisible();
  await expect(page.getByTestId("studio-tour-offer")).toBeHidden();
});
