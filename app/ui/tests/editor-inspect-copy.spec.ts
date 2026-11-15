// Map editor: the Inspector's right-click Copy / Paste (a Transform row, a key's value, an
// entity's properties onto others, skipping keys a class lacks), each paste one undo step.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

test("right-click Copy / Paste in the Inspector", async ({ page }) => {
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
  const add = async (classname: string, origin: number[], keys: [string, string][]) => (await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname, origin, keys })).created.entities[0] as number;
  const a = await add("env_sprite", [64, 32, 16], [["model", "sprites/glow01.spr"], ["renderamt", "150"], ["scale", "2"], ["targetname", "a"]]);
  const b = await add("env_sprite", [512, 0, 0], [["model", "sprites/glow01.spr"]]);
  const l = await add("light", [0, 512, 0], []);
  // (the editor reads the map again)
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  const doc = async () => (await api(page, "/api/tools/editor/doc")) as { entities: { id: number; keys: [string, string][] }[] };
  const key = async (id: number, k: string) => (await doc()).entities.find((e) => e.id === id)?.keys.find(([x]) => x === k)?.[1];
  const pick = (ids: number[]) => page.evaluate(`window.__lsEditor.getState().select({ brushes: [], entities: ${JSON.stringify(ids)}, face: null })`);
  const panel = page.locator(".tools-panel", { has: page.locator("h3") }).first();
  const menu = page.getByRole("menu", { name: "Copy and paste" });

  // a Transform row: Location copied from a, pasted onto b
  await pick([a]);
  await panel.locator(".p5-xf-row", { hasText: "Location" }).click({ button: "right" });
  await menu.getByRole("menuitem", { name: /^Copy Location/ }).click();
  await pick([b]);
  await panel.locator(".p5-xf-row", { hasText: "Location" }).click({ button: "right" });
  await menu.getByRole("menuitem", { name: /^Paste Location/ }).click();
  await expect.poll(() => key(b, "origin")).toBe("64 32 16");

  // properties: a's keys (not its name) onto b and the light; the light lacks some, so they're skipped
  await pick([a]);
  await page.getByRole("heading", { name: "env_sprite" }).click({ button: "right" });
  await expect(menu.getByRole("menuitem", { name: /^Paste properties/ })).toBeDisabled();
  await menu.getByRole("menuitem", { name: /^Copy properties/ }).click();
  await pick([b, l]);
  await page.locator(".tools-panel h3").first().click({ button: "right" });
  await menu.getByRole("menuitem", { name: /^Paste properties/ }).click();
  await expect.poll(() => key(b, "renderamt")).toBe("150");
  expect(await key(b, "scale")).toBe("2");
  expect(await key(b, "targetname")).toBeUndefined();
  expect(await key(l, "renderamt")).toBeUndefined();
  await expect(page.getByText(/Skipped \(not keys of that class\)/)).toBeVisible();
  // one undo takes the whole paste back
  await page.evaluate("window.__lsEditor.getState().undo()");
  // (a placed sprite starts fully drawn: 255)
  await expect.poll(() => key(b, "renderamt")).toBe("255");

  // a key's value: copied from a's renderamt, pasted into b's
  await pick([a]);
  await panel.locator(".editor-key", { has: page.locator('label[title^="renderamt"]') }).click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Copy renderamt" }).click();
  await pick([b]);
  await panel.locator(".editor-key", { has: page.locator('label[title^="renderamt"]') }).click({ button: "right" });
  await menu.getByRole("menuitem", { name: "Paste into renderamt" }).click();
  await expect.poll(() => key(b, "renderamt")).toBe("150");
});
