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

// ---- QA4-B (docs/notes/QA4-B.md): F5 CSV refusals, F6.1 Mode settings on a Timeline tab, F6.5 Shift-click,
// F6.7 an overshooting curve inside its lane

test("QA4-B F5: a CSV import refuses bad rows with their line and why, the good ones go in", async ({ page }) => {
  await seed(page);
  await openIt(page);
  await page.getByRole("button", { name: /^Add a data table/ }).click();
  await page.getByRole("button", { name: "Paste or import CSV…" }).click();
  await page.getByRole("textbox", { name: "CSV to import" }).fill('Key,Value\n1,10\n2,abc\n3,30\n4,"x""y"\n5,50');
  await page.getByTestId("studio-table-editor").getByRole("button", { name: "Import", exact: true }).click();
  const refused = page.getByTestId("studio-csv-refused");
  await expect(refused).toContainText("2 rows weren't imported");
  await expect(refused).toContainText("Line 3: Value “abc” isn't a number");
  await expect(refused).toContainText('Line 5: Value “x"y” isn\'t a number');
  // the refused rows are back in the box to fix
  await expect(page.getByRole("textbox", { name: "CSV to import" })).toHaveValue('Key,Value\n2,abc\n4,"x""y"');
  // the good ones are in the table (the first row there plus 3), nothing as 0
  await expect(page.getByText(/4 of 256 rows/)).toBeVisible();
  await page.getByRole("textbox", { name: "CSV to import" }).fill("Key,Value\n2,20\n4,40");
  await page.getByTestId("studio-table-editor").getByRole("button", { name: "Import", exact: true }).click();
  await expect(refused).toHaveCount(0);
  await expect(page.getByText(/6 of 256 rows/)).toBeVisible();
});

test("QA4-B F6.1, F6.7: Mode settings shows the mode on a Timeline tab; an overshooting curve stays in its lane", async ({ page }) => {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.timelines = [{ id: "tl1", name: "Lift", length: 4, tick: 0.05, tracks: [{ id: "tr1", name: "Height", kind: "float", keys: [{ t: 0, v: 0, ease: "custom", tout: 400 }, { t: 4, v: 100 }] }] }];
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H })).json()).ok).toBeTruthy();
  await page.goto(`/tools-dev.html?open=studio:${ID}/tl1`);
  await expect(page.getByRole("button", { name: "Mode settings" })).toBeVisible({ timeout: 20_000 });
  // (a small window starts with Details folded)
  if (await page.getByTestId("studio-rail-right").isVisible()) await page.getByTestId("studio-rail-right").click();
  await expect(page.getByText("Details · Timeline")).toBeVisible();
  const modeBtn = page.getByTestId("studio-mode-settings");
  await expect(modeBtn).toHaveAttribute("aria-pressed", "false");
  await modeBtn.click();
  await expect(page.getByRole("heading", { name: "This mode" })).toBeVisible();
  await expect(modeBtn).toHaveAttribute("aria-pressed", "true");
  // the curve's highest point is inside the lane (it overshoots well past 100)
  const ys = await page.locator(".st-tl-lane polyline").first().evaluate((el) => (el.getAttribute("points") ?? "").split(" ").map((p) => Number(p.split(",")[1])));
  expect(Math.min(...ys)).toBeGreaterThanOrEqual(0);
});

test("QA4-B F6.5: Shift-click adds a node to the selection", async ({ page }) => {
  await seed(page);
  await openIt(page);
  await page.locator(".react-flow__controls-fitview").click();
  const a = page.locator('.react-flow__node[data-id="is_ct"] .st-node-title');
  const b = page.locator('.react-flow__node[data-id="give_m4"] .st-node-title');
  await a.click();
  await b.click({ modifiers: ["Shift"] });
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(2);
  // and again takes it out
  await b.click({ modifiers: ["Shift"] });
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
});

// ---- "+ Add pin" (pinCountOps.ts) and a variable wired into For Each Player's Who
async function seedNodes(page: Page, nodes: object[], edges: object[], variables: object[] = []) {
  const tpl = { schema: "loner-strike/graph", version: 3, mode: { id: ID, name: "Pins check" }, variables, nodes, edges, comments: [] };
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}
const savedGraph = async (page: Page) => (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;

test("+ Add pin on a Sequence adds an output, − takes it off (not while wired), Undo puts it back", async ({ page }) => {
  await seedNodes(page, [
    { id: "rs", type: "event.round_started", pos: { x: 0, y: 0 }, params: {} },
    { id: "seq", type: "flow.sequence", pos: { x: 300, y: 0 }, params: { count: 2 } },
    { id: "p", type: "debug.print", pos: { x: 600, y: 0 }, params: { text: "x", chat: false } },
  ], [{ id: "w1", from: { node: "rs", pin: "then" }, to: { node: "seq", pin: "exec" } }, { id: "w2", from: { node: "seq", pin: "then1" }, to: { node: "p", pin: "exec" } }]);
  await openIt(page);
  await page.locator(".react-flow__controls-fitview").click();
  const seq = page.locator('.react-flow__node[data-id="seq"]');
  const add = seq.getByRole("button", { name: "+ Add pin" });
  // the last pin (Then 1) is wired: − is refused, with why
  await seq.getByRole("button", { name: "Remove last pin" }).click();
  await expect(seq.getByRole("status")).toContainText("has a wire");
  await add.click();
  await expect(seq.locator(".react-flow__handle[data-handleid='then2']")).toHaveCount(1);
  await page.keyboard.press("ControlOrMeta+s");
  await expect.poll(async () => (await savedGraph(page)).nodes.find((n: { id: string }) => n.id === "seq").params.count).toBe(3);
  // the new last pin isn't wired: − takes it off; Undo brings it back
  await seq.getByRole("button", { name: "Remove last pin" }).click();
  await expect(seq.locator(".react-flow__handle[data-handleid='then2']")).toHaveCount(0);
  await page.locator(".st-canvas").focus();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(seq.locator(".react-flow__handle[data-handleid='then2']")).toHaveCount(1);
});

test("a variable wired into For Each Player's Who: saved, and the quick check is clean", async ({ page }) => {
  await seedNodes(page, [
    { id: "rs", type: "event.round_started", pos: { x: 0, y: 0 }, params: {} },
    { id: "who", type: "var.get", pos: { x: 0, y: 200 }, params: { var: "v_who" } },
    { id: "fe", type: "flow.for_each_player", pos: { x: 320, y: 0 }, params: { which: "alive" } },
    { id: "p", type: "debug.print", pos: { x: 640, y: 0 }, params: { text: "x", chat: false } },
  ], [{ id: "w1", from: { node: "rs", pin: "then" }, to: { node: "fe", pin: "exec" } }, { id: "w2", from: { node: "fe", pin: "each" }, to: { node: "p", pin: "exec" } }],
  [{ id: "v_who", name: "Who to count", type: "enum:ls_who", scope: "mode", default: 7 }]);
  await openIt(page);
  await page.locator(".react-flow__controls-fitview").click();
  const from = page.locator('.react-flow__node[data-id="who"] .react-flow__handle.source').first();
  const to = page.locator('.react-flow__node[data-id="fe"] .react-flow__handle[data-handleid="who"]');
  await expect(to).toHaveCount(1);
  await from.dragTo(to);
  await page.keyboard.press("ControlOrMeta+s");
  await expect.poll(async () => (await savedGraph(page)).edges.some((e: { from: { node: string }; to: { node: string; pin: string } }) => e.from.node === "who" && e.to.node === "fe" && e.to.pin === "who")).toBe(true);
  await expect(page.getByTestId("studio-check-status")).toContainText("Quick check:");
  await expect(page.locator(".st-results-list li.error")).toHaveCount(0);
});
