// Game Mode Studio, fixes from the UX review (docs/notes/StudioUXReview.md), X's share:
// H2 the mode's own Details come back (empty canvas, Escape, the Mode settings button),
// H5 the node search places its best match on Enter, H7 an empty event graph says where to start.
// Run in Chromium and WebKit: `npx playwright test studio-ux --project large --project webkit`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_ux";

async function seed(page: Page, blank = false) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "UX check";
  tpl.functions = [{ id: "fa", name: "Reward", inputs: [], outputs: [] }];
  if (blank) {
    tpl.nodes = [];
    tpl.edges = [];
    tpl.comments = [];
    tpl.mode.settings = [];
  }
  tpl.nodes.push({ id: "fa_e", type: "fn.entry", pos: { x: 0, y: 0 }, params: {}, graph: "fa" });
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}

test.beforeEach(async ({ page }) => {
  for (const r of ["compile", "check", "testrun", "playtest", "switch", "source", "uninstall/**", "debug/command"])
    await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test)", installedAs: ID } }));
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("H2: the mode's Details come back after a My mode item (empty canvas, Escape, Mode settings)", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const modeTitle = page.getByRole("heading", { name: "This mode" });
  await expect(modeTitle).toBeVisible({ timeout: 20_000 });
  const fn = page.locator(".st-mb-section").getByText("Reward", { exact: true }).first();
  const back = async (how: () => Promise<void>) => {
    await fn.click();
    await expect(modeTitle).toHaveCount(0);
    await how();
    await expect(modeTitle).toBeVisible();
  };
  await back(() => page.locator(".react-flow__pane").click({ position: { x: 8, y: 8 } }));
  await back(async () => {
    await page.locator(".react-flow__pane").click({ position: { x: 8, y: 8 } });
    await fn.click();
    await page.locator(".st-canvas").focus();
    await page.keyboard.press("Escape");
  });
  await back(() => page.getByTestId("studio-mode-settings").click());
  await expect(page.getByTestId("studio-mode-settings")).toHaveAttribute("aria-pressed", "true");
  // a node picked, then none: the mode again
  await page.locator(".react-flow__node").filter({ hasText: "Player Killed" }).first().click({ force: true });
  await expect(modeTitle).toHaveCount(0);
  await page.locator(".react-flow__pane").click({ position: { x: 8, y: 8 } });
  await expect(modeTitle).toBeVisible();
});

test("H5: search ranks title matches first and Enter places the best one", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.locator(".react-flow__node").first()).toBeVisible({ timeout: 20_000 });
  const killedSaved = async () => {
    const save = page.getByRole("button", { name: "Save", exact: true });
    if (await save.isEnabled()) await save.click();
    await expect(save).toBeDisabled();
    const g = (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
    return g.nodes.filter((n: { type: string }) => n.type === "event.player_killed").length;
  };
  const before = await killedSaved();
  await page.locator(".react-flow__pane").click({ button: "right", position: { x: 40, y: 40 } });
  const pal = page.getByRole("dialog", { name: "Add a node" });
  await pal.getByRole("textbox").fill("killed");
  const first = pal.locator(".st-palette-item").first();
  await expect(first).toContainText("Player Killed");
  // the cursor is on it (not its folder): Enter places it
  await expect(pal.locator(".cur")).toContainText("Player Killed");
  await page.keyboard.press("Enter");
  await expect(pal).toHaveCount(0);
  expect(await killedSaved()).toBe(before + 1);
  // a title that starts with the words beats one that only has them in its help
  const pb = (await page.locator(".react-flow__pane").boundingBox())!;
  await page.locator(".react-flow__pane").click({ button: "right", position: { x: pb.width - 40, y: pb.height - 40 } });
  await pal.getByRole("textbox").fill("give w");
  await expect(pal.locator(".st-palette-item").first()).toContainText("Give Weapon");
  await page.keyboard.press("Escape");
});

test("H7: an empty Event Graph shows where to start, and the Quick check says nothing happens", async ({ page }) => {
  await seed(page, true);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const card = page.getByTestId("studio-empty");
  await expect(card).toBeVisible({ timeout: 20_000 });
  await expect(page.getByText(/Nothing happens yet: add an event/)).toBeVisible();
  await card.getByRole("button", { name: "Round Started" }).click();
  await expect(card).toHaveCount(0);
  await expect(page.locator(".react-flow__node").filter({ hasText: "Round Started" })).toHaveCount(1);
  // just placed: picked, so Details shows it
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
  await expect(page.getByText(/none of the events has anything wired after it/).first()).toBeVisible();
});
