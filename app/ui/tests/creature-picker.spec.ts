// Shared creatures (CP-ui): one creature type picker in the map editor and the Game Mode Studio
// (built-in, yours, installed <author>.<type>), a warning for a name the game doesn't have, the
// editor's Checks entry for it, and Creature Studio's "Used by". Runs against a tools dev server
// over a scratch game whose creatures.ini has my_runner (yours) and jo.ghoul (installed by a
// package), and waves.ini using my_runner: never the real game.
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };
const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

test("the server lists built-in, your own and installed types", async ({ request }) => {
  const n = await (await request.get("/api/tools/creatures/names")).json();
  expect(n.builtin).toContain("zombie");
  expect(n.own).toContain("my_runner");
  expect(n.installed).toContainEqual({ name: "jo.ghoul", label: "ghoul by jo", from: ["mode-77"] });
});

test("map editor: the creature type picker, its warning and the Checks entry", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const c = await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "ls_creature", origin: [0, 0, 16] });
  const id = c.created.entities[0];
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: id, key: "type", value: "nobody_here" });
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  // the Checks flag it
  await expect.poll(async () => ((await api(page, "/api/tools/editor/checks")).issues ?? []).some((i: { code: string; message: string }) => i.code === "unknown-creature" && i.message.includes("nobody_here")), { timeout: 20_000 }).toBe(true);
  await page.getByRole("button", { name: "Outline", exact: true }).or(page.getByRole("tab", { name: "Outline" })).first().click();
  await page.getByLabel(/Find in the map|Find by name/).first().fill("ls_creature");
  await page.locator(".ed-ol-row", { hasText: "ls_creature" }).first().click();
  const input = page.locator(`#editor-f-${id}-type`);
  await expect(input).toHaveValue("nobody_here");
  const field = page.locator(".editor-def", { has: input });
  await expect(field.locator(".creature-picker-warn")).toContainText("no creature type called nobody_here");
  // the picker offers built-in, yours and installed types
  const pick = field.locator(".creature-picker-add");
  for (const n of ["zombie", "my_runner", "jo.ghoul"]) await expect(pick.locator(`option[value="${n}"]`)).toHaveCount(1);
  await pick.selectOption("jo.ghoul");
  await expect.poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((e: { id: number }) => e.id === id).keys).type).toBe("jo.ghoul");
  await expect(field.locator(".creature-picker-warn")).toHaveCount(0);
  // an installed type reads "<type> by <author>"; the key keeps the full name
  await expect(field.locator(".creature-picker-shown")).toHaveText("ghoul by jo");
  await expect(pick.locator('option[value="jo.ghoul"]')).toHaveText("ghoul by jo");
  await expect.poll(async () => ((await api(page, "/api/tools/editor/checks")).issues ?? []).some((i: { code: string }) => i.code === "unknown-creature"), { timeout: 20_000 }).toBe(false);
  // typing still works
  await input.fill("my_runner");
  await input.press("Enter");
  await expect.poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((e: { id: number }) => e.id === id).keys).type).toBe("my_runner");
});

const ID = "pw_creature_picker";
test.describe("Game Mode Studio", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem("ls.studio.panels", JSON.stringify({ left: true, right: true, minimap: false })));
    const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
    tpl.mode.id = ID;
    tpl.mode.name = "Picker check";
    tpl.nodes = [{ id: "sp", type: "creature.spawn", pos: { x: 200, y: 200 }, params: { kind: "nobody_here", at: "spot1" } }];
    tpl.edges = [];
    tpl.comments = [];
    const r = await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H });
    expect((await r.json()).ok, "seed saved").toBeTruthy();
  });
  test.afterEach(async ({ page }) => {
    await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H });
  });

  test("a creature param uses the picker, warns on an unknown name, and Used by sees the mode", async ({ page }) => {
    await page.goto(`/tools-dev.html?open=studio:${ID}`);
    const node = page.locator(".st-node", { hasText: "Spawn Creature" }).first();
    await expect(node).toBeVisible({ timeout: 20_000 });
    await node.click();
    const picker = page.locator(".st-side-field", { hasText: "Type" }).locator(".creature-picker").first();
    await expect(picker.locator("input")).toHaveValue("nobody_here");
    await expect(picker.locator(".creature-picker-warn")).toContainText("no creature type called nobody_here");
    await picker.locator(".creature-picker-add").selectOption("my_runner");
    await expect(picker.locator("input")).toHaveValue("my_runner");
    await expect(picker.locator(".creature-picker-warn")).toHaveCount(0);
    await page.locator(".tl-actionbar").getByRole("button", { name: "Save", exact: true }).click();
    // saved: the Creature Studio's Used by lists this mode for my_runner (and runner, its base)
    await expect
      .poll(async () => (await (await page.request.get(`/api/tools/studio/graph/${ID}`)).json()).graph.nodes.find((n: { id: string }) => n.id === "sp").params.kind, { timeout: 15_000 })
      .toBe("my_runner");
    const used = await (await page.request.get("/api/tools/creatures/used-by")).json();
    expect(used.usedBy.my_runner).toContainEqual({ kind: "mode", name: "Picker check", id: ID });
    expect(used.usedBy.runner).toContainEqual({ kind: "mode", name: "Picker check", id: ID });
    expect(used.usedBy.my_runner).toContainEqual({ kind: "waves", name: "Last Stand's waves", id: "laststand" });
    await page.goto(`/tools-dev.html?open=creatures`);
    await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /My runner/ }).click();
    await expect(page.getByLabel("Used by")).toContainText("Picker check");
    await expect(page.getByLabel("Used by")).toContainText("Last Stand's waves");
    // an installed type: listed apart, read only, with where it came from
    await expect(page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /Ghoul/ })).toHaveCount(0);
    await page.getByRole("listbox", { name: "From Community" }).getByRole("option", { name: /Ghoul/ }).click();
    await expect(page.locator(".cs-main")).toContainText("From Community, read only.");
    await expect(page.locator(".cs-main h3")).toHaveText("ghoul by jo");
    await expect(page.getByLabel("Used by")).toContainText("none of your maps, modes or waves yet");
  });
});
