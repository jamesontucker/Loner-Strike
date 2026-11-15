// Game Mode Studio, NPCs and interactables (T1fix): the NPC nodes come from the Add a node
// palette (Talked To NPC, Used Interactable, Say and Choose, Say, Set NPC Behavior, NPC Look At,
// Spawn NPC, Spawn Interactable), Say and Choose shows its reply outputs, and The Quest Giver
// opens from the start screen's examples. Nothing is installed or played.
// Run against a dev server over a scratch game (never the real one).
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_npc";
const WRITES = ["compile", "check", "testrun", "testrun/cancel", "playtest", "switch", "source", "uninstall/**", "debug/command"];

async function seed(page: Page) {
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "NPC check";
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
  expect((await r.json()).ok).toBeTruthy();
}

test.beforeEach(async ({ page }) => {
  // (in a small window both side panels leave the canvas about 320 px wide: too little room to
  // right-click empty canvas for seven nodes, so they start closed there)
  const roomy = (page.viewportSize()?.width ?? 1440) >= 1280;
  await page.addInitScript((open) => {
    localStorage.setItem("ls.studio.panels", JSON.stringify({ left: open, right: open, minimap: false }));
    localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false }));
  }, roomy);
  for (const r of WRITES) await page.route(`**/api/tools/studio/${r}`, (route) => route.fulfill({ json: { ok: true, problems: [], output: "(a test)", installedAs: ID } }));
});

test("the NPC nodes come from the palette, and Say and Choose shows its replies", async ({ page }) => {
  await seed(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  await page.locator(".react-flow__node").first().waitFor({ timeout: 20_000 });
  const pane = page.locator(".react-flow__pane");
  const pb = (await pane.boundingBox())!;
  // (small ones first, each on empty canvas at the top right, away from the graph and the controls)
  const nodes: [string, string][] = [
    ["talked to npc", "Talked To NPC"],
    ["used interactable", "Used Interactable"],
    ["set npc behavior", "Set NPC Behavior"],
    ["npc look at", "NPC Look At"],
    ["spawn npc", "Spawn NPC"],
    ["spawn interactable", "Spawn Interactable"],
    ["say and choose", "Say and Choose"],
  ];
  await page.locator(".react-flow__controls-fitview").click();
  await page.mouse.move(pb.x + pb.width / 2, pb.y + pb.height / 2);
  for (let k = 0; k < 4; k++) await page.mouse.wheel(0, 400);
  for (const [i, [search, title]] of nodes.entries()) {
    // (a spot of empty canvas, whatever the window size: the first point on a grid, from the
    // top right, where nothing covers the canvas)
    const spot = await page.evaluate((b) => {
      // (every point of a node-sized box around the spot must be canvas: the palette opens only
      // on the canvas itself)
      const free = (x: number, y: number) => [[0, 0], [-60, -30], [60, -30], [-60, 30], [60, 30]].every(([dx, dy]) => document.elementFromPoint(b.x + x + dx, b.y + y + dy)?.classList.contains("react-flow__pane"));
      for (let y = 50; y < b.height - 40; y += 50)
        for (let x = b.width - 140; x > 80; x -= 60) if (free(x, y)) return { x, y };
      return null;
    }, pb);
    expect(spot, `empty canvas for ${title}`).not.toBeNull();
    await pane.click({ button: "right", position: spot! });
    const pal = page.getByRole("dialog", { name: "Add a node" });
    await pal.getByRole("textbox").fill(search);
    await expect(pal.locator(".st-palette-item").first()).toContainText(title);
    await page.keyboard.press("Enter");
    await expect(pal).toHaveCount(0);
    await expect(page.locator(`.react-flow__node[data-id]`, { hasText: title }).first()).toBeAttached();
  }
  // Say and Choose: one output per reply, Timed out, Walked away and Picked
  const choose = page.locator(".react-flow__node", { hasText: "Say and Choose" }).first();
  for (const out of ["Reply 1", "Reply 6", "Timed out", "Walked away", "Picked"]) await expect(choose).toContainText(out);
});

test("The Quest Giver opens from the examples", async ({ page }) => {
  await page.goto("/tools-dev.html?open=studio");
  const ex = page.getByTestId("studio-examples");
  await expect(ex.getByRole("button")).toHaveCount(6);
  await ex.getByRole("button", { name: /The Quest Giver/ }).click();
  await page.locator(".react-flow__node").first().waitFor({ timeout: 20_000 });
  await expect(page.locator(".st-name")).toContainText("The Quest Giver");
  // (the canvas draws only what's in view, and the whole example fits a large window's view)
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.locator(".react-flow__controls-fitview").click();
  for (const t of ["Talked To NPC", "Say and Choose", "Used Interactable", "Spawn NPC", "Spawn Interactable"]) await expect(page.locator(".react-flow__node", { hasText: t }).first()).toBeAttached();
});
