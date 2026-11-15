// NPCs and interactables in the map editor (NPC-editor): their own Outline icons, an NPC drawn as
// its model (Barney when it has none of its own) with "Its model" in the form, the Checks entries
// (an NPC with no name, an interactable with nothing drawn), and plain map I/O (an interactable
// sets off its target; an NPC's patrol stops). Runs against a tools dev server over a scratch game
// (valve/models/barney.mdl copied in): never the real game.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

const issues = async (page: Page) => ((await api(page, "/api/tools/editor/checks")).issues ?? []) as { code: string; entity?: number; message: string }[];

test("map editor: NPCs and interactables get icons, a model preview, Checks and I/O", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const add = async (classname: string, origin: number[]) => (await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname, origin })).created.entities[0] as number;
  const npc = await add("ls_npc", [0, 0, 16]);
  const use = await add("ls_interactable", [64, 0, 16]);
  const door = await add("info_target", [128, 0, 16]);
  const stop = await add("info_target", [0, 128, 16]);
  for (const [entity, key, value] of [
    [use, "target", "gate"],
    [door, "targetname", "gate"],
    [stop, "targetname", "p1"],
    [npc, "targets", "p1"],
  ] as const)
    await api(page, "/api/tools/editor/cmd", { op: "set_key", entity, key, value });

  // Checks: the NPC has no name, the interactable draws nothing
  await expect.poll(async () => (await issues(page)).filter((i) => ["npc-no-name", "interactable-no-model"].includes(i.code)).map((i) => [i.code, i.entity]).sort(), { timeout: 20_000 }).toEqual([
    ["interactable-no-model", use],
    ["npc-no-name", npc],
  ]);
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: npc, key: "name", value: "Old Joe" });
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: use, key: "model", value: "models/barney.mdl" });
  await expect.poll(async () => (await issues(page)).filter((i) => ["npc-no-name", "interactable-no-model"].includes(i.code)).length, { timeout: 20_000 }).toBe(0);
  // a model that isn't in the game: the asset check names it, on the NPC
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: npc, key: "model", value: "models/qa_nobody.mdl" });
  await expect.poll(async () => (await issues(page)).some((i) => i.code === "missing-file" && i.entity === npc && i.message.includes("qa_nobody.mdl")), { timeout: 20_000 }).toBe(true);
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: npc, key: "model", value: "" });

  // plain map I/O: the interactable sets off the door; the NPC walks between its stops
  const io = await api(page, "/api/tools/editor/io");
  const links = (io.links ?? []) as { from: number; key: string; to: number[] }[];
  expect(links.some((l) => l.from === use && l.key === "target" && l.to.includes(door))).toBe(true);
  expect(links.some((l) => l.from === npc && l.key === "targets" && l.to.includes(stop))).toBe(true);

  // the Outline: their own icons; the NPC's form shows Barney as its model
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.getByRole("button", { name: "Outline", exact: true }).or(page.getByRole("tab", { name: "Outline" })).first().click();
  const find = page.getByLabel(/Find in the map|Find by name/).first();
  await find.fill("ls_");
  const npcRow = page.locator(".ed-ol-row", { hasText: "ls_npc" }).first();
  const useRow = page.locator(".ed-ol-row", { hasText: "ls_interactable" }).first();
  await expect(npcRow.locator("svg path").first()).toHaveAttribute("d", /^M5\.5 4a2 2/);
  await expect(useRow.locator("svg path").first()).toHaveAttribute("d", /^M6 1\.5a1 1/);
  await npcRow.click();
  await expect(page.getByText("Its model")).toBeVisible();
  await expect(page.locator(".editor-def", { hasText: "Its model" })).toContainText("models/barney.mdl");
});
