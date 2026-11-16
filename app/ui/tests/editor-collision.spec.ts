// Map editor: a model's Collision (Inspect › Collision, made into a box when the map compiles) and
// the information drawing styles (Collision, By entity type, Missing textures) with their keys.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

test("a model's Collision and the information drawing styles", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  // what a dropped model becomes: Box, players only
  const ent = await page.evaluate(`import("/src/views/tools/editor/extras.ts").then((m) => m.assetEntity("model", "models/w_c4.mdl"))`);
  expect((ent as { keys: [string, string][] }).keys).toContainEqual(["ls_collision", "box"]);
  const id = (await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "cycler_sprite", origin: [0, 0, 0], keys: (ent as { keys: [string, string][] }).keys })).created.entities[0];
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.evaluate(`window.__lsEditor.getState().select({ brushes: [], entities: [${id}], face: null })`);
  const field = page.locator(".tools-panel").getByRole("combobox", { name: /^Collision/ });
  await expect(field).toHaveValue("box");
  await expect(field.locator("option")).toHaveText(["Default (None (walk through))", "None (walk through)", "Box, players only", "Box, players and bullets", "Custom (your own brushes)"]);
  // the drawing styles: each information style shows its key
  const style = page.locator(".ed-pane-mode").first();
  await style.selectOption("collision");
  await expect(page.getByRole("list", { name: "What the colors mean" }).first()).toContainText("CLIP: players only");
  await style.selectOption("bytype");
  await expect(page.getByRole("list", { name: "What the colors mean" }).first()).toContainText("func_detail");
  await style.selectOption("missing");
  await expect(page.getByRole("list", { name: "What the colors mean" }).first()).toContainText("Missing or unknown texture");
  await style.selectOption("textured");
  await expect(page.getByRole("list", { name: "What the colors mean" })).toHaveCount(0);
});
