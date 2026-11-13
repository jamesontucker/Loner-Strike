// Game Mode Studio, graph version 3 in the page: an address opens a Function's tab, Function
// Entry/Return show the Inputs/Outputs editor, the keyboard shortcuts (never while typing),
// a snippet with events goes onto the Event Graph, the library's Export all / Import, and
// leaving with unsaved edits through each way out (asked once; Leave goes and stays gone).
// Run in Chromium and WebKit: `npx playwright test studio-v3 [--browser webkit]`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_v3";

async function seed(page: Page) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "V3 check";
  tpl.functions = [{ id: "fa", name: "Reward", inputs: [{ id: "amt", name: "Amount", type: "number" }], outputs: [] }];
  tpl.nodes.push({ id: "fa_e", type: "fn.entry", pos: { x: 0, y: 0 }, params: {}, graph: "fa" }, { id: "fa_r", type: "fn.return", pos: { x: 420, y: 0 }, params: {}, graph: "fa" });
  tpl.edges.push({ id: "fa_w", from: { node: "fa_e", pin: "then" }, to: { node: "fa_r", pin: "exec" } });
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}
const saved = async (page: Page) => (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
const tabs = (page: Page) => page.getByRole("tablist", { name: "Graphs" });
const selectedTab = (page: Page) => tabs(page).getByRole("tab", { selected: true });
const node = (page: Page, id: string) => page.locator(`.react-flow__node[data-id="${id}"]`);
const canvasKey = async (page: Page, key: string) => {
  // keys go to the canvas: focus its pane first (not a field)
  await page.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press(key);
};

test.beforeEach(async ({ page }) => {
  await seed(page);
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("address opens the tab; Entry/Return details; shortcuts", async ({ page }) => {
  await page.route("**/api/tools/studio/playtest", async (r) => {
    await new Promise((res) => setTimeout(res, 1500));
    await r.fulfill({ json: { ok: true, problems: [], output: "Done." } });
  });
  await page.route("**/api/state", (r) => r.fulfill({ json: { game_open: false } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
  await page.goto(`/tools-dev.html?open=studio:${ID}/fa`);
  // the address's tab is open straight away
  await expect(selectedTab(page)).toContainText("Reward", { timeout: 20_000 });
  await expect(node(page, "fa_e")).toBeVisible();

  // Function Entry: its Inputs editor in Details
  await node(page, "fa_e").click();
  const details = page.getByRole("complementary", { name: "Details" });
  await expect(details.getByRole("heading", { name: "Inputs" })).toBeVisible();
  await details.getByRole("button", { name: "Add input" }).click();
  await expect(details.locator('input[value="Input 2"]')).toHaveCount(1);
  // Return: the Outputs editor
  await node(page, "fa_r").click();
  await expect(details.getByRole("heading", { name: "Outputs" })).toBeVisible();
  await details.getByRole("button", { name: "Add output" }).click();

  // ⌘S saves
  await canvasKey(page, "ControlOrMeta+s");
  await expect.poll(async () => (await saved(page)).functions[0].inputs.length).toBe(2);
  expect((await saved(page)).functions[0].outputs.length).toBe(1);

  // ⌘D duplicates the selection (the Return)
  const before = await page.locator(".react-flow__node").count();
  await node(page, "fa_r").click();
  await page.keyboard.press("ControlOrMeta+d");
  await expect(page.locator(".react-flow__node")).toHaveCount(before + 1);
  await page.keyboard.press("ControlOrMeta+z");
  await expect(page.locator(".react-flow__node")).toHaveCount(before);

  // Alt-click a wire breaks it
  const wires = page.locator(".react-flow__edge");
  await expect(wires).not.toHaveCount(0);
  const wireCount = await wires.count();
  // (the middle of the wire, on screen)
  const mid = await page.getByRole("button", { name: "Edge from fa_e to fa_r" }).locator("path.react-flow__edge-path").evaluate((el) => {
    const p = el as SVGPathElement;
    const pt = p.getPointAtLength(p.getTotalLength() / 2).matrixTransform(p.getScreenCTM()!);
    return { x: pt.x, y: pt.y };
  });
  await page.keyboard.down("Alt");
  await page.mouse.click(mid.x, mid.y);
  await page.keyboard.up("Alt");
  await expect(wires).toHaveCount(wireCount - 1);
  await canvasKey(page, "ControlOrMeta+z");
  await expect(wires).toHaveCount(wireCount);

  // ? opens the cheat sheet (and not while typing)
  await canvasKey(page, "?");
  const sheet = page.getByRole("dialog", { name: "Keys" });
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText("Alt+click a wire");
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);
  // ⌘F: the find box; typing "?" and "f" there stays typing
  await canvasKey(page, "ControlOrMeta+f");
  const findBox = page.locator(".st-find input");
  await expect(findBox).toBeFocused();
  await page.keyboard.type("f?");
  await expect(findBox).toHaveValue("f?");
  await expect(sheet).toHaveCount(0);
  await findBox.fill("");

  // Ctrl+Tab: Event Graph and back; ⌘W closes the Function's tab
  await canvasKey(page, "Control+Tab");
  await expect(selectedTab(page)).toContainText("Event Graph");
  await canvasKey(page, "Control+Tab");
  await expect(selectedTab(page)).toContainText("Reward");
  await canvasKey(page, "ControlOrMeta+w");
  await expect(selectedTab(page)).toContainText("Event Graph");
  await expect(tabs(page).getByRole("tab")).toHaveCount(1);

  // F and Home frame (the view moves, nothing breaks)
  await canvasKey(page, "Home");
  await canvasKey(page, "f");

  // F5: Playtest
  await canvasKey(page, "F5");
  await expect(page.getByRole("status").filter({ hasText: "Compiling" })).toBeVisible();
});

test("a snippet with events goes onto the Event Graph", async ({ page }) => {
  await page.goto(`/tools-dev.html?open=studio:${ID}/fa`);
  await expect(selectedTab(page)).toContainText("Reward", { timeout: 20_000 });
  const evBefore = (await saved(page)).nodes.filter((n: { graph?: string }) => !n.graph).length;
  await page.getByRole("button", { name: /^Kill reward/ }).click();
  await expect(page.getByText("went onto the Event Graph")).toBeVisible();
  await expect(selectedTab(page)).toContainText("Event Graph");
  await canvasKey(page, "ControlOrMeta+s");
  await expect.poll(async () => (await saved(page)).nodes.filter((n: { graph?: string }) => !n.graph).length).toBe(evBefore + 6);
  expect((await saved(page)).nodes.filter((n: { graph?: string }) => n.graph === "fa").length).toBe(2);
});

test("library: Export all, Import a whole library", async ({ page }, info) => {
  const items = async () => (await (await page.request.get("/api/tools/studio/library")).json()).items as { id: string }[];
  const had = new Set((await items()).map((x) => x.id));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
  // keep the Function twice under two names (right-click → Save to my library)
  // (a second Function for the second item: saving one twice is a new version, not an item)
  await page.getByRole("button", { name: "Add a Function (its own graph)" }).click();
  for (const [name, part] of [["PW one", "Reward"], ["PW two", "New Function"]]) {
    await page.locator(".st-mb-item").filter({ hasText: part }).first().click({ button: "right" });
    await page.getByRole("menuitem", { name: /Save to my library/ }).click();
    const form = page.getByRole("dialog", { name: "Save to my library" });
    await form.getByLabel("Name").fill(name);
    await form.getByRole("button", { name: "Save", exact: true }).click();
    await expect(form).toHaveCount(0);
  }
  await expect.poll(async () => (await items()).filter((x) => !had.has(x.id)).length).toBe(2);
  const ours = (await items()).filter((x) => !had.has(x.id)).map((x) => x.id);

  const dl = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export all" }).click();
  const path = info.outputPath("library.lib.json");
  await (await dl).saveAs(path);
  const data = JSON.parse(await (await import("node:fs/promises")).readFile(path, "utf8"));
  expect(data.items.length).toBe(had.size + 2);

  // remove ours, import the whole file: ours come back (the others as renamed copies)
  for (const id of ours) await page.request.post(`/api/tools/studio/library/${id}/delete`, { headers: H });
  await page.locator('.st-mb input[type="file"]').setInputFiles(path);
  await expect(page.getByText(`Imported ${data.items.length} item${data.items.length === 1 ? "" : "s"} into your library`)).toBeVisible();
  const after = (await items()).map((x) => x.id);
  for (const id of ours) expect(after).toContain(id);
  expect(after.length).toBe(had.size + data.items.length);
  for (const id of after) if (!had.has(id)) await page.request.post(`/api/tools/studio/library/${id}/delete`, { headers: H });
});

test.describe("leaving with unsaved edits", () => {
  // in the page, as a person gets there: Overview → Game Mode Studio → the mode → its Function
  const dirty = async (page: Page) => {
    await page.goto(`/tools-dev.html?open=overview`);
    await page.getByRole("navigation", { name: "Tools" }).getByRole("button", { name: "Game Mode Studio" }).click();
    await page.getByRole("button", { name: /V3 check/ }).first().click();
    await expect(selectedTab(page)).toContainText("Event Graph", { timeout: 20_000 });
    await page.locator(".st-mb-item").filter({ hasText: "Reward" }).first().dblclick();
    await expect(selectedTab(page)).toContainText("Reward");
    await node(page, "fa_e").click();
    await page.getByRole("complementary", { name: "Details" }).getByRole("button", { name: "Add input" }).click();
  };
  const leaveAsked = (page: Page) => page.getByRole("alertdialog").filter({ hasText: "Leave without saving" });
  const noQuestion = async (page: Page) => {
    // a moment later: no (second) question, and nothing stepped back
    await page.waitForTimeout(700);
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
  };
  const stillEdited = async (page: Page) => {
    await tabs(page).getByRole("tab", { name: /Reward/ }).click();
    await node(page, "fa_e").click();
    await expect(page.getByRole("complementary", { name: "Details" }).locator('input[value="Input 2"]')).toHaveCount(1);
  };
  const atStudioList = (page: Page) => expect(page).toHaveURL(/open=studio(&|$)/);

  test("between a mode's tabs: nothing to ask, the edits stay", async ({ page }) => {
    await dirty(page);
    await page.locator(".tools-crumb").filter({ hasText: "V3 check" }).click();
    await noQuestion(page);
    await expect(selectedTab(page)).toContainText("Event Graph");
    await page.evaluate(() => history.back());
    await noQuestion(page);
    await expect(selectedTab(page)).toContainText("Reward");
    await stillEdited(page);
  });

  test("a crumb: Cancel stays, Leave goes once", async ({ page }) => {
    await dirty(page);
    const crumb = page.locator(".tools-crumb").filter({ hasText: "Game Mode Studio" });
    await crumb.click();
    await expect(leaveAsked(page)).toBeVisible();
    await leaveAsked(page).getByRole("button", { name: "Cancel" }).click();
    await noQuestion(page);
    await stillEdited(page);
    await crumb.click();
    await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
    await noQuestion(page);
    await atStudioList(page);
  });

  test("the Tools crumb", async ({ page }) => {
    await dirty(page);
    await page.locator(".tools-crumb").filter({ hasText: /^Tools$/ }).click();
    await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
    await noQuestion(page);
    await expect(page).toHaveURL(/open=overview/);
  });

  for (const [how, back] of [
    ["the Back button", (p: Page) => p.getByRole("button", { name: "Back", exact: true }).click()],
    ["Alt+←", async (p: Page) => {
      await p.locator(".react-flow__pane").click({ position: { x: 5, y: 5 } });
      await p.keyboard.press("Alt+ArrowLeft");
    }],
    ["the browser's back", (p: Page) => p.evaluate(() => history.back())],
  ] as [string, (p: Page) => Promise<unknown>][]) {
    test(how, async ({ page }) => {
      await dirty(page);
      // the Function's tab → the Event Graph: the same mode, no question
      await back(page);
      await noQuestion(page);
      await expect(selectedTab(page)).toContainText("Event Graph");
      // → the Studio's list: asked; Cancel stays (edits too), Leave goes and stays gone
      await back(page);
      await expect(leaveAsked(page)).toBeVisible();
      await leaveAsked(page).getByRole("button", { name: "Cancel" }).click();
      await noQuestion(page);
      await expect(selectedTab(page)).toContainText("Event Graph");
      await expect(page.locator(".st-name")).toHaveText("V3 check •");
      await back(page);
      await expect(leaveAsked(page)).toBeVisible();
      await leaveAsked(page).getByRole("button", { name: "Leave" }).click();
      await noQuestion(page);
      await atStudioList(page);
    });
  }
});
