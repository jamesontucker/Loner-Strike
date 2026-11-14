// Game Mode Studio, fixes from the UX review (docs/notes/StudioUXReview.md), X2's share:
// H1 a readable first view, H4 folding side panels and a node list that isn't cut off, M1 the
// minimap off unless asked for, M2 the drop choice closes with Escape or a click away, M6 the
// toolbar groups, M10 examples open without saving, H3 recommended maps as chips with a search.
// `npx playwright test studio-ux-x2 --project large --project small --project webkit`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
// (one mode per project: the projects can run side by side)
let ID = "pw_ux2";

async function seed(page: Page, big = false) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "UX check two";
  if (big) {
    // a long row of Print nodes: fitting all of it would be far too small to read
    for (let k = 0; k < 40; k++) tpl.nodes.push({ id: `p${k}`, type: "debug.print", pos: { x: 800 + k * 320, y: 0 }, params: { text: `${k}` } });
  }
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}

async function openIt(page: Page) {
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await expect(page.locator(".react-flow__node").first()).toBeVisible({ timeout: 20_000 });
}

test.beforeEach(async ({ page }, info) => {
  ID = `pw_ux2_${info.project.name}`;
  for (const r of ["compile", "check", "testrun", "playtest", "switch", "source", "uninstall/**", "debug/command"])
    await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test)", installedAs: ID } }));
  // (each test starts with both panels shown; a reload inside a test keeps what it set)
  await page.addInitScript(() => {
    try {
      if (!sessionStorage.getItem("pw.panels")) localStorage.removeItem("ls.studio.panels");
      sessionStorage.setItem("pw.panels", "1");
    } catch {
      /* fine */
    }
  });
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("H1: a big graph opens readable, not zoomed out to fit", async ({ page }) => {
  await seed(page, true);
  await openIt(page);
  await expect
    .poll(async () => page.locator(".react-flow__viewport").evaluate((el) => new DOMMatrixReadOnly(getComputedStyle(el).transform).a), { timeout: 5_000 })
    .toBeGreaterThanOrEqual(0.6);
});

test("H4, M1: side panels fold to rails (kept after a reload), the minimap is off until asked for", async ({ page }) => {
  await seed(page);
  await openIt(page);
  await expect(page.locator(".st-minimap")).toHaveCount(0);
  await page.getByRole("button", { name: "Fold My mode" }).click();
  await expect(page.getByTestId("studio-rail-left")).toBeVisible();
  const wide = await page.locator(".st-center").evaluate((el) => el.getBoundingClientRect().width);
  await page.reload();
  await expect(page.getByTestId("studio-rail-left")).toBeVisible({ timeout: 20_000 });
  await page.getByTestId("studio-rail-left").click();
  await expect(page.getByRole("button", { name: "Fold My mode" })).toBeVisible();
  expect(await page.locator(".st-center").evaluate((el) => el.getBoundingClientRect().width)).toBeLessThan(wide);
  // the minimap from the View menu
  await page.getByTestId("studio-view-menu").click();
  await page.getByRole("menuitemcheckbox", { name: /Show the minimap/ }).click();
  await expect(page.locator(".st-minimap")).toBeVisible();
});

test("H4: the node list stays inside the window near the canvas's edge", async ({ page }) => {
  await seed(page);
  await openIt(page);
  const pane = page.locator(".react-flow__pane");
  const b = (await pane.boundingBox())!;
  await pane.click({ button: "right", position: { x: b.width - 70, y: b.height - 30 } });
  const pal = page.getByRole("dialog", { name: "Add a node" });
  await expect(pal).toBeVisible();
  const p = (await pal.boundingBox())!;
  const vp = page.viewportSize()!;
  expect(p.x + p.width).toBeLessThanOrEqual(vp.width);
  expect(p.y + p.height).toBeLessThanOrEqual(vp.height);
  await page.keyboard.press("Escape");
  await expect(pal).toHaveCount(0);
});

test("M2: the variable drop choice closes with Escape and with a click away", async ({ page }) => {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.variables = [{ id: "v1", name: "Kills Made", type: "number", scope: "mode", default: 0 }];
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H })).json()).ok).toBeTruthy();
  await openIt(page);
  // a variable dropped on the canvas, as the browser sends it (no key held: Get or Set?)
  const drop = () =>
    page.evaluate(() => {
      const dt = new DataTransfer();
      dt.setData("application/x-studio-variable", "v1");
      const el = document.querySelector(".st-canvas")!;
      const r = el.getBoundingClientRect();
      for (const type of ["dragover", "drop"]) el.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: dt, clientX: r.left + 320, clientY: r.top + 150 }));
    });
  const choice = page.getByTestId("studio-drop-choice");
  await drop();
  await expect(choice).toBeVisible();
  // the keyboard has it: arrows move, Escape closes
  await expect(choice.getByRole("menuitem").first()).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(choice.getByRole("menuitem").nth(1)).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(choice).toHaveCount(0);
  await drop();
  await expect(choice).toBeVisible();
  await page.locator(".st-toolbar").click({ position: { x: 4, y: 4 } });
  await expect(choice).toHaveCount(0);
});

test("M6: the toolbar's groups and plain labels", async ({ page }) => {
  await seed(page);
  await openIt(page);
  await expect(page.locator(".st-toolbar .st-tgroup")).toHaveCount(3);
  await expect(page.getByRole("checkbox", { name: "Compile on save" })).toBeVisible();
  await expect(page.locator(".st-bots")).toContainText("bots");
});

test("H3: recommended maps are chips with a search to add more", async ({ page }) => {
  await seed(page);
  await openIt(page);
  // (a small window starts with Details folded)
  if (await page.getByTestId("studio-rail-right").isVisible()) await page.getByTestId("studio-rail-right").click();
  const req = page.getByRole("region", { name: "Requirements" });
  const add = req.getByRole("searchbox", { name: "Add a recommended map" });
  await expect(add).toBeVisible({ timeout: 20_000 });
  await add.fill("dust");
  const found = req.getByRole("listbox", { name: "Maps found" });
  await expect(found.getByRole("option").first()).toBeVisible();
  const name = ((await found.getByRole("option").first().textContent()) ?? "").split(" ")[0];
  await found.getByRole("option").first().click();
  const chips = req.getByRole("list", { name: "Recommended maps" });
  await expect(chips).toContainText(name);
  await chips.getByRole("button", { name: `Remove ${name}` }).click();
  await expect(req.getByRole("list", { name: "Recommended maps" })).toHaveCount(0);
  // nothing sideways: the panel doesn't scroll left and right
  const over = await page.locator(".st-requirements").evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(over).toBeLessThanOrEqual(1);
});

test("M10: an example opens without saving; Make it mine keeps a copy", async ({ page }) => {
  await page.goto(`/tools-dev.html?open=studio`);
  const ex = page.getByTestId("studio-examples").getByRole("button").first();
  await expect(ex).toBeVisible({ timeout: 20_000 });
  await ex.click();
  const bar = page.getByTestId("studio-example-bar");
  await expect(bar).toBeVisible();
  // nothing saved for it in Your modes
  const names = async () => ((await (await page.request.get("/api/tools/studio/graphs")).json()).graphs as { name: string }[]).map((g) => g.name);
  expect((await names()).filter((n) => n.endsWith("(example)"))).toEqual([]);
  await bar.getByRole("button", { name: "Make it mine" }).click();
  const dlg = page.getByRole("alertdialog", { name: "Make it mine" });
  await dlg.getByRole("textbox").fill(`pw ux2 copy ${test.info().project.name}`);
  await dlg.getByRole("button", { name: "Make it mine" }).click();
  await expect(bar).toHaveCount(0);
  const list = (await (await page.request.get("/api/tools/studio/graphs")).json()).graphs as { id: string; name: string }[];
  const mine = list.find((g) => g.name === `pw ux2 copy ${test.info().project.name}`);
  expect(mine).toBeTruthy();
  await page.request.post(`/api/tools/studio/graph/${mine!.id}/delete`, { headers: H });
});
