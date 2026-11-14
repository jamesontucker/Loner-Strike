// Game Mode Studio: node help (the ? menu, F1 on a picked node, search, an example dropped in),
// the example modes on the start screen and the tour (offered once, every step, Skip, from the
// ? menu again). Nothing is installed or played: those routes are answered in the page.
// Chromium and WebKit: `npx playwright test studio-help [--browser webkit]`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_help";
const WRITES = ["compile", "check", "testrun", "testrun/cancel", "playtest", "switch", "source", "uninstall/**", "debug/command"];

/** The whole graph in view: a big graph opens on its first part, readable (UX review H1). */
async function fitAll(page: Page) {
  await page.locator(".react-flow__node").first().waitFor({ timeout: 20_000 });
  await page.waitForTimeout(250); // (after the opening view)
  await page.locator(".react-flow__controls-fitview").click();
}

async function seed(page: Page) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "Help check";
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok).toBeTruthy();
}

test.beforeEach(async ({ page }) => {
  // both side panels shown, whatever the window's width (a small one starts with Details folded)
  await page.addInitScript(() => localStorage.setItem("ls.studio.panels", JSON.stringify({ left: true, right: true, minimap: false })));
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
  await fitAll(page);
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

test("deleting a variable in use: the choices, Find uses, one undo step", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false })));
  const g = await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json();
  const tpl = g.graph;
  tpl.variables = [
    { id: "v1", name: "Kills Made", type: "number", scope: "player", default: 0 },
    { id: "v2", name: "Other Count", type: "number", scope: "player", default: 0 },
  ];
  // Player Killed -> Add to Kills Made -> Is CT ... (the Add sits in the flow)
  tpl.nodes.push({ id: "addk", type: "var.add", pos: { x: 300, y: 600 }, params: { var: "v1", amount: 1 } }, { id: "getk", type: "var.get", pos: { x: 300, y: 800 }, params: { var: "v1" } });
  const w = tpl.edges.find((e: { from: { node: string }; to: { node: string } }) => e.from.node === "killed" && e.to.node === "is_ct");
  w.to = { node: "addk", pin: "exec" };
  tpl.edges.push({ id: "wa", from: { node: "addk", pin: "then" }, to: { node: "is_ct", pin: "exec" } }, { id: "wb", from: { node: "killed", pin: "attacker" }, to: { node: "addk", pin: "player" } }, { id: "wc", from: { node: "killed", pin: "attacker" }, to: { node: "getk", pin: "player" } });
  await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await fitAll(page);
  await page.getByRole("complementary", { name: "My mode" }).getByText("Kills Made").click();
  await page.getByRole("button", { name: "Remove this variable" }).click();
  const dlg = page.getByTestId("studio-delete-dialog");
  await expect(dlg).toContainText("used by 2 nodes");
  await expect(dlg.getByRole("combobox")).toHaveValue("v2");
  // Find uses closes it and picks them
  await dlg.getByRole("button", { name: "Find uses" }).click();
  await expect(dlg).toBeHidden();
  // (both uses picked: Add and Get)
  await expect(page.locator(".react-flow__node.selected")).toHaveCount(2);
  await expect(page.locator('.react-flow__node[data-id="addk"].selected')).toHaveCount(1);
  // again: remove it and its nodes
  await page.getByRole("complementary", { name: "My mode" }).getByText("Kills Made").click();
  await page.getByRole("button", { name: "Remove this variable" }).click();
  await dlg.getByLabel(/Remove it and its nodes/).check();
  await dlg.getByTestId("studio-delete-go").click();
  await expect(page.locator('.react-flow__node[data-id="addk"]')).toHaveCount(0);
  await expect(page.locator('.react-flow__node[data-id="getk"]')).toHaveCount(0);
  // the flow was joined: Player Killed runs Is CT again, so no "nothing runs" for it
  await expect(page.locator('.react-flow__node[data-id="is_ct"] .st-node-badge')).toHaveCount(0);
  // one undo brings it all back
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.locator('.react-flow__node[data-id="addk"]')).toHaveCount(1);
  await expect(page.locator('.react-flow__node[data-id="getk"]')).toHaveCount(1);
  await expect(page.getByRole("complementary", { name: "My mode" }).getByText("Kills Made")).toBeVisible();
});

test("polish: drops with Ctrl and Alt, quick add, Promote to Variable, align, versions", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false })));
  const g = (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
  g.variables = [{ id: "v1", name: "Kills Made", type: "number", scope: "mode", default: 0 }];
  const saved = await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H })).json();
  expect(saved.ok, JSON.stringify(saved)).toBeTruthy();
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await fitAll(page);
  await expect(page.locator(".st-mb").getByText("Kills Made")).toBeVisible();
  const canvas = page.locator(".st-canvas");
  await canvas.waitFor();
  const nodes = () => page.locator(".react-flow__node").count();
  // a drop with a key held, as the browser sends it
  const drop = (payloadType: string, value: string, mods: { ctrl?: boolean; alt?: boolean }, at = { x: 200, y: 150 }) =>
    page.evaluate(
      ({ payloadType, value, mods, at }) => {
        const dt = new DataTransfer();
        dt.setData(payloadType, value);
        const el = document.querySelector(".st-canvas")!;
        const r = el.getBoundingClientRect();
        at = { x: r.left + at.x, y: r.top + at.y };
        for (const type of ["dragover", "drop"]) el.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: dt, clientX: at.x, clientY: at.y, ctrlKey: !!mods.ctrl, altKey: !!mods.alt }));
      },
      { payloadType, value, mods, at },
    );
  const VAR = "application/x-studio-variable";
  const PART = "application/x-studio-part";
  let n = await nodes();
  await drop(VAR, "v1", { ctrl: true });
  await expect.poll(nodes).toBe(n + 1);
  // (a whole-mode variable's Get is a pill with just its name, UX review M4)
  await expect(page.locator(".st-node-pill").filter({ hasText: "Kills Made" })).toHaveCount(1);
  await drop(VAR, "v1", { alt: true }, { x: 200, y: 260 });
  await expect(page.locator(".react-flow__node").filter({ hasText: "Set Kills Made" })).toHaveCount(1);
  // no key: a choice
  await drop(VAR, "v1", {}, { x: 320, y: 150 });
  await expect(page.getByTestId("studio-drop-choice")).toBeVisible();
  await page.getByTestId("studio-drop-choice").getByRole("button", { name: "Cancel" }).click();
  // an event that's already here: jump to it, no new node
  n = await nodes();
  await drop(PART, "event:killed", {});
  await expect(page.getByText("Already in this graph")).toBeVisible();
  expect(await nodes()).toBe(n);
  // quick add: hold B and click empty space
  await canvas.focus();
  await page.keyboard.down("b");
  const spot = await page.evaluate(() => {
    const r = document.querySelector(".react-flow__pane")!.getBoundingClientRect();
    for (let y = r.top + 40; y < r.bottom - 40; y += 30)
      for (let x = r.left + 40; x < r.right - 160; x += 30) if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) return { x, y };
    return null;
  });
  await page.mouse.click(spot!.x, spot!.y);
  await page.keyboard.up("b");
  await expect(page.locator(".react-flow__node").filter({ hasText: "Branch" })).toHaveCount(1);
  // Promote to Variable from a pin
  await page.locator('.react-flow__node[data-id="give_m4"] .st-pin', { hasText: "Spare bullets" }).dispatchEvent("contextmenu", { bubbles: true, clientX: 600, clientY: 400 });
  await page.getByRole("menuitem", { name: "Promote to Variable" }).click();
  await expect(page.locator(".st-mb").getByText("Spare bullets", { exact: true })).toBeVisible();
  // align two nodes' left edges (everything in view first)
  await page.locator(".st-canvas").focus();
  await page.keyboard.press("Home");
  await page.waitForTimeout(400);
  await page.keyboard.press("ControlOrMeta+a");
  await page.locator(".react-flow__node-logic .st-node-head").first().dispatchEvent("contextmenu", { bubbles: true, clientX: 600, clientY: 400 });
  await page.getByRole("menuitem", { name: "Align left edges" }).click();
  // versions: save, change, save; pick the first: marks on the canvas; restore is one undo
  await page.locator(".tl-actionbar").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.locator(".tl-actionbar").getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  const after = (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
  const pos = (id: string) => after.nodes.find((x: { id: string }) => x.id === id).pos.x;
  expect(pos("give_m4")).toBe(pos("give_ak"));
  await page.getByTestId("studio-versions-button").click();
  const panel = page.getByTestId("studio-versions");
  await expect(panel.getByRole("option").first()).toBeVisible();
  const versions = await panel.getByRole("option").count();
  await panel.getByRole("option").last().click();
  await expect(panel).toContainText(/Since then: [1-9]\d* added/);
  // (everything in view: only what's in view is drawn)
  await page.locator(".st-canvas").focus();
  await page.keyboard.press("Home");
  await expect(page.locator(".st-node.st-mark-added").first()).toBeVisible();
  await panel.getByTestId("studio-version-restore").click();
  await expect(page.locator(".react-flow__node").filter({ hasText: "Branch" })).toHaveCount(0);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.locator(".react-flow__node").filter({ hasText: "Branch" })).toHaveCount(1);
  expect(versions).toBeGreaterThanOrEqual(2);
});

test("Timeline curves: pick Custom, drag a tangent handle, Auto puts it back", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false })));
  const g = (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
  g.timelines = [{ id: "tl1", name: "Lift", length: 4, tick: 0.05, tracks: [{ id: "h", name: "Height", kind: "float", keys: [{ t: 0, v: 0 }, { t: 2, v: 50 }, { t: 4, v: 0 }] }] }];
  await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: g }, headers: H });
  await page.goto(`/tools-dev.html?open=studio:${ID}/tl1`);
  const ed = page.getByTestId("timeline-editor");
  await expect(ed).toBeVisible();
  // pick the middle key, then Custom
  await ed.locator('circle.st-tl-k[data-key="1"]').dispatchEvent("pointerdown", { button: 0, clientX: 0, clientY: 0, bubbles: true });
  await page.mouse.up();
  await ed.getByLabel("Key interpolation").selectOption("custom");
  const handle = ed.locator('[data-tangent="out"]');
  await expect(handle).toBeVisible();
  const b = (await handle.boundingBox())!;
  await page.mouse.move(b.x + 4, b.y + 4);
  await handle.dispatchEvent("pointerdown", { button: 0, clientX: b.x + 4, clientY: b.y + 4, bubbles: true });
  await page.mouse.move(b.x + 4, b.y - 30, { steps: 4 });
  await page.mouse.up();
  await expect(ed.getByRole("button", { name: "Auto tangents" })).toBeEnabled();
  await ed.getByRole("button", { name: "Auto tangents" }).click();
  await expect(ed.getByRole("button", { name: "Auto tangents" })).toBeDisabled();
});
