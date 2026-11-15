// Game Mode Studio types (graph version 4, docs/notes/StudioTypes.md and StudioCompat.md):
// orphaned pins and settings are kept and shown, Refresh nodes lists them, the variable type menu
// is grouped with the new types, a Color start value has a picker, and an Entity dropped on a
// Creature pin offers As Creature. Run: `npx playwright test studio-types --project large --project small --project webkit`.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const ID = "pw_types";

async function seed(page: Page, graph: object) {
  const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph }, headers: H });
  expect((await r.json()).ok, "seed saved").toBeTruthy();
}

const base = (extra: object = {}) => ({
  schema: "loner-strike/graph",
  version: 3,
  mode: { id: ID, name: "Types check" },
  variables: [],
  comments: [],
  ...extra,
});

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("ls.studio.panels", JSON.stringify({ left: true, right: true, minimap: false })));
});
test.afterEach(async ({ page }) => {
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
});

test("orphans: a wire to a removed pin and a removed setting stay, shown red and greyed, named in Problems, listed by Refresh nodes", async ({ page }) => {
  // (the save route refuses nothing it can read back: an orphan is kept, never dropped)
  await seed(page, base({
    nodes: [
      { id: "rs", type: "event.round_started", pos: { x: 0, y: 0 }, params: {} },
      { id: "p", type: "debug.print", pos: { x: 320, y: 0 }, params: { text: "hi", chat: false, gone_setting: 5 } },
      { id: "n", type: "math.number", pos: { x: 0, y: 200 }, params: { value: 2 } },
    ],
    edges: [
      { id: "w1", from: { node: "rs", pin: "then" }, to: { node: "p", pin: "exec" } },
      { id: "w2", from: { node: "n", pin: "out" }, to: { node: "p", pin: "removed_pin" } },
    ],
  }));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const orphan = page.locator(".st-pin-orphan").filter({ hasText: "Orphaned: removed_pin" });
  await expect(orphan).toBeVisible({ timeout: 20_000 });
  // (Problems names it too: the app's check, compat_tests.rs orphaned_pins_and_values_are_kept_and_named)
  // Details shows the removed setting greyed, with Clear
  await page.locator(".react-flow__node").filter({ hasText: "Print String" }).first().click();
  const removed = page.getByRole("group", { name: "Removed settings" }).or(page.locator('[aria-label="Removed settings"]'));
  await expect(removed).toContainText("Orphaned: gone_setting = 5");
  // Refresh nodes lists both
  await page.getByTestId("studio-view-menu").click();
  await page.getByRole("menuitem", { name: /Refresh nodes/ }).click();
  await expect(page.getByText(/1 orphaned wire, 1 orphaned setting/)).toBeVisible();
  // saved again, the orphan is still there
  const g = (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
  expect(g.edges.some((e: { to: { pin: string } }) => e.to.pin === "removed_pin")).toBeTruthy();
  // Clear removes the setting
  await removed.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByText("Orphaned: gone_setting = 5")).toHaveCount(0);
});

test("types: the variable type menu is grouped with the new types, and a Color start value has a picker", async ({ page }) => {
  await seed(page, base({ variables: [{ id: "c", name: "Team color", type: "color", scope: "mode", default: "#ffcc00ff" }], nodes: [{ id: "rs", type: "event.round_started", pos: { x: 0, y: 0 }, params: {} }], edges: [] }));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const item = page.locator(".st-mb-section").getByText("Team color", { exact: true });
  await expect(item).toBeVisible({ timeout: 20_000 });
  await item.click();
  const type = page.getByRole("combobox", { name: "Type", exact: true }).first();
  await expect(type).toHaveValue("color");
  for (const g of ["Basic", "Game", "Files (loaded with the map)", "Handles"]) await expect(type.locator(`optgroup[label="${g}"]`)).toHaveCount(1);
  for (const t of ["Transform", "Creature", "Weapon", "Model", "Sound Loop"]) await expect(type.locator("option", { hasText: t }).first()).toBeAttached();
  const picker = page.getByLabel("Start value", { exact: true });
  await expect(picker).toHaveValue("#ffcc00");
  await page.getByRole("button", { name: "Counter-Terrorist blue" }).click();
  await expect(picker).toHaveValue("#3c78e0");
});

test("casts: an Entity dropped on a Creature pin offers Insert As Creature", async ({ page }) => {
  await seed(page, base({
    variables: [{ id: "boss", name: "Boss", type: "creature", scope: "mode" }],
    nodes: [
      { id: "rs", type: "event.round_started", pos: { x: 0, y: 0 }, params: {} },
      { id: "pe", type: "flow.pick_entity", pos: { x: 0, y: 160 }, params: { by: "class", name: "info_player_start" } },
      { id: "set", type: "var.set", pos: { x: 260, y: 160 }, params: { var: "boss" } },
    ],
    edges: [{ id: "w1", from: { node: "rs", pin: "then" }, to: { node: "pe", pin: "exec" } }],
  }));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const from = page.locator('.react-flow__node[data-id="pe"] .react-flow__handle[data-handleid="entity"]');
  const to = page.locator('.react-flow__node[data-id="set"] .react-flow__handle[data-handleid="value"]');
  await expect(from).toBeVisible({ timeout: 20_000 });
  await from.dragTo(to);
  const insert = page.getByRole("button", { name: "Insert As Creature" });
  await expect(insert).toBeVisible();
  await insert.click();
  await expect(page.locator(".react-flow__node").filter({ hasText: "As Creature" })).toHaveCount(1);
});

test("a struct, enum, data table or Function picker in Details makes a new one in one click", async ({ page }) => {
  await seed(page, base({ nodes: [{ id: "mk", type: "struct.make", pos: { x: 0, y: 0 }, params: { struct: "" } }], edges: [] }));
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const node = page.locator('.react-flow__node[data-id="mk"]');
  await expect(node).toBeVisible({ timeout: 20_000 });
  await node.click();
  const details = page.getByRole("complementary", { name: "Details" });
  // (the app's own Transform is always there to pick; the mode's own come from New struct…)
  await expect(details.getByRole("combobox").first()).toContainText("Transform");
  await expect(details.getByRole("button", { name: "New struct…" })).toHaveAttribute("title", /next to Structs in the My mode panel on the left/);
  await details.getByRole("button", { name: "New struct…" }).click();
  // a struct made, picked on the node, and its editor open to fill in
  const g = async () => (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph;
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect.poll(async () => { const x = await g(); return x.structs?.length === 1 && x.nodes.find((n: { id: string }) => n.id === "mk").params.struct === x.structs[0].id; }).toBeTruthy();
  // the left panel's sections are labeled, each with its +
  for (const [title, add] of [["Structs", "Add a struct"], ["Enums", "Add an enum"], ["Data tables", /Add a data table/], ["Functions", /Add a Function/], ["Variables", /Add a variable/]] as const)
    await expect(page.locator(".st-mb-section").filter({ hasText: title }).getByRole("button", { name: add }).first()).toBeAttached();
});
