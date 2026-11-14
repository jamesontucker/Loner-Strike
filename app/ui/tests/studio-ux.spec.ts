// Game Mode Studio, fixes from the UX review (docs/notes/StudioUXReview.md), X's share:
// H2 the mode's own Details come back (empty canvas, Escape, the Mode settings button),
// H5 the node search places its best match on Enter, H7 an empty event graph says where to start.
// Run in Chromium and WebKit: `npx playwright test studio-ux --project large --project webkit`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
// UX_SHOTS=<folder>: screenshots of what each test checks
const shot = async (page: Page, name: string) => {
  if (process.env.UX_SHOTS) await page.screenshot({ path: `${process.env.UX_SHOTS}/${name}.png` });
};
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
  // both side panels shown: a small window starts with Details folded (by design, UX review H4)
  await page.addInitScript(() => localStorage.setItem("ls.studio.panels", JSON.stringify({ left: true, right: true, minimap: false })));
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
  await shot(page, "h7-empty");
  await card.getByRole("button", { name: "Round Started" }).click();
  await expect(card).toHaveCount(0);
  await expect(page.locator(".react-flow__node").filter({ hasText: "Round Started" })).toHaveCount(1);
  // just placed: picked, so Details shows it
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
  await expect(page.getByText(/none of the events has anything wired after it/).first()).toBeVisible();
});

test("M5, M7, L9: new things are picked and named at once; a Custom Event puts its node down; no Save to library on a variable", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("heading", { name: "This mode" })).toBeVisible({ timeout: 20_000 });
  // a node from the search is picked: Details shows it
  const pb = (await page.locator(".react-flow__pane").boundingBox())!;
  await page.locator(".react-flow__pane").click({ button: "right", position: { x: pb.width - 60, y: pb.height - 60 } });
  const pal = page.getByRole("dialog", { name: "Add a node" });
  await pal.getByRole("textbox").fill("give money");
  await page.keyboard.press("Enter");
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
  await expect(page.locator(".react-flow__node.selected")).toContainText("Give Money");
  // a new variable: its name field has the focus, all selected
  await page.getByRole("button", { name: "Add a variable" }).click();
  const focused = page.locator("input:focus");
  await expect(focused).toHaveValue(/^Variable \d+$/);
  await page.keyboard.type("Lives");
  await expect(page.locator(".st-mb-section").getByText("Lives", { exact: true })).toBeVisible();
  // its menu has no "Save to my library" (it can't be saved there)
  await page.locator(".st-mb-section").getByText("Lives", { exact: true }).click({ button: "right" });
  await expect(page.getByRole("menuitem", { name: /Rename/ })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /Save to my library/ })).toHaveCount(0);
  await page.keyboard.press("Escape");
  // a Custom Event: its node on the graph, its name ready
  const before = await page.locator(".react-flow__node").filter({ hasText: "Custom Event" }).count();
  await page.getByRole("button", { name: /^Add a Custom Event/ }).click();
  await expect(page.locator("input:focus")).toBeVisible();
  await expect(page.locator(".react-flow__node").filter({ hasText: "Custom Event" })).toHaveCount(before + 1);
});

test("M4, L4, L5, L8: a mode variable's Get is a pill without Player and Team; pin menu; breakpoint dot on hover; labels", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("heading", { name: "This mode" })).toBeVisible({ timeout: 20_000 });
  await page.getByRole("button", { name: "Add a variable" }).click();
  await page.keyboard.type("Score");
  // drop a Get (Ctrl drag) onto empty canvas
  const item = page.locator(".st-mb-section").getByText("Score", { exact: true });
  const pb = (await page.locator(".react-flow__pane").boundingBox())!;
  await page.keyboard.down(process.platform === "darwin" ? "Meta" : "Control");
  await item.dragTo(page.locator(".react-flow__pane"), { targetPosition: { x: pb.width - 120, y: pb.height - 80 } });
  await page.keyboard.up(process.platform === "darwin" ? "Meta" : "Control");
  const choice = page.getByTestId("studio-drop-choice");
  if (await choice.isVisible().catch(() => false)) await choice.getByRole("menuitem", { name: /Get/ }).first().click();
  const pill = page.locator(".st-node-pill").filter({ hasText: "Score" });
  await expect(pill).toHaveCount(1);
  await expect(pill.locator(".react-flow__handle")).toHaveCount(1);
  await shot(page, "m4-pill");
  // nodes and pins have names for screen readers
  await expect(page.locator('.react-flow__node[aria-label="Get Variable node"]')).toHaveCount(1);
  await expect(pill.locator(".react-flow__handle")).toHaveAttribute("aria-label", /output/);
  // the breakpoint dot shows on hover only (unless set)
  // (the whole graph in view: in a small window the node can sit under the side panel)
  await page.locator(".react-flow__controls-fitview").click();
  const n = page.locator(".react-flow__node").filter({ hasText: "Player Killed" }).first();
  const dot = n.locator(".st-bp");
  await page.mouse.move(5, 5);
  await expect(dot).toHaveCSS("opacity", "0");
  await n.hover({ force: true });
  await expect(dot).toHaveCSS("opacity", "1");
  // a wired pin's menu: Break link, Select the linked node
  const wired = page.locator(".st-pin:has(.react-flow__handle.wired) .st-pin-label").first();
  await wired.click({ button: "right" });
  await expect(page.getByRole("menuitem", { name: /^Break/ })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /^Select the/ })).toBeVisible();
  await page.getByRole("menuitem", { name: /^Select the/ }).click();
  await expect(page.locator(".react-flow__node.selected")).not.toHaveCount(0);
});

test("M8, M9: the Timeline editor (name, fitted length, the key row's place, labeled Remove, Details) and yes/no settings", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("heading", { name: "This mode" })).toBeVisible({ timeout: 20_000 });
  // M9: Free for all is a switch, Round time a number with its unit
  const ffa = page.getByRole("switch", { name: "Free for all" });
  if (await ffa.count()) await expect(ffa).toBeVisible();
  await page.getByRole("button", { name: "Add a setting" }).click();
  const pick = page.getByRole("combobox", { name: "Setting" }).last();
  await pick.selectOption("mp_friendlyfire");
  const sw = page.getByRole("switch", { name: "Friendly fire" });
  await expect(sw).toBeChecked();
  await sw.uncheck();
  await expect(sw).not.toBeChecked();
  await pick.selectOption("mp_roundtime");
  await expect(page.locator(".st-var").last()).toContainText("minutes, next round");
  await shot(page, "m9-settings");
  // M8
  await page.getByRole("button", { name: /^Add a Timeline/ }).click();
  const name = page.getByRole("textbox", { name: "Timeline name" });
  await expect(name).toBeFocused();
  await page.keyboard.type("Lift");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Lift" })).toBeVisible();
  await expect(page.getByText("Details · Timeline")).toBeVisible();
  await page.getByRole("button", { name: "+ Number track" }).click();
  // the ruler shows the whole length at once, its first label whole
  const ruler = page.locator(".st-tl-ruler text");
  await expect(ruler.first()).toHaveText("0s");
  const last = await ruler.last().textContent();
  const len = await page.getByRole("spinbutton", { name: "Length" }).inputValue();
  expect(Number(last!.replace("s", ""))).toBeGreaterThanOrEqual(Number(len) - 0.01);
  // a key added: the lanes don't move
  const lane = page.locator(".st-tl-lane").first();
  // (in view first: in a small window the click would scroll the lanes to it, which isn't the row moving)
  await lane.scrollIntoViewIfNeeded();
  const y0 = (await lane.boundingBox())!.y;
  await lane.click({ position: { x: 80, y: 12 } });
  await expect(page.getByTestId("timeline-key")).toBeVisible();
  expect(Math.abs((await lane.boundingBox())!.y - y0)).toBeLessThan(2);
  // (b14771d7: a track's remove is the × in its one-line header, named for the track; the toolbar's
  // own says Delete timeline)
  await expect(page.getByRole("button", { name: /^Remove track/ }).first()).toHaveText("×");
  await expect(page.getByRole("button", { name: "Delete timeline" })).toBeVisible();
  await shot(page, "m8-timeline");
});

test("H6: a pin's value is typed on the node, the same as in Details", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.getByRole("heading", { name: "This mode" })).toBeVisible({ timeout: 20_000 });
  const pb = (await page.locator(".react-flow__pane").boundingBox())!;
  await page.locator(".react-flow__pane").click({ button: "right", position: { x: pb.width - 260, y: pb.height - 200 } });
  const pal = page.getByRole("dialog", { name: "Add a node" });
  await pal.getByRole("textbox").fill("give money");
  await page.keyboard.press("Enter");
  const money = page.locator(".react-flow__node.selected");
  await expect(money).toContainText("Give Money");
  const amount = money.locator("input.st-pin-value").first();
  await expect(amount).toBeVisible();
  // typed on the node: Details has it too (and the line under the pins doesn't repeat it)
  await amount.fill("750");
  await amount.press("Enter");
  const details = page.getByRole("complementary", { name: "Details" });
  await expect(details.locator('input[type="number"]').first()).toHaveValue("750");
  await expect(money.locator(".st-node-params")).toHaveCount(0);
  // changed in Details: the node shows it
  await details.locator('input[type="number"]').first().fill("300");
  await expect(amount).toHaveValue("300");
  // Escape leaves it as it was
  await amount.fill("999");
  await amount.press("Escape");
  await expect(amount).toHaveValue("300");
  // a yes/no and a choice on a Give Weapon: right there too
  await page.locator(".react-flow__pane").click({ button: "right", position: { x: pb.width - 260, y: 60 } });
  await pal.getByRole("textbox").fill("give weapon");
  await page.keyboard.press("Enter");
  const gw = page.locator(".react-flow__node.selected");
  await expect(gw.locator("select.st-pin-value")).toHaveValue("ak47");
  await gw.locator("select.st-pin-value").selectOption("awp");
  await expect(details.locator("select").filter({ has: page.locator('option[value="awp"]') }).first()).toHaveValue("awp");
  await shot(page, "h6-inline");
});
