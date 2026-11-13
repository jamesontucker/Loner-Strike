// Map editor: push/pull a face in 3D in Chromium (Windows' WebView2). A box whose corner sits on
// the 3D camera's line puts that edge under the exact middle pixel; Chromium's ray there slipped
// between the two faces' triangles and the drag did nothing (QA round 2). Also: one pane still
// has its drawing style picker, and saving a prefab under a name you already use asks. Runs on editor-dev.html against the tools dev server
// (npm run test:tools -- tests/editor-push.spec.ts).
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

async function openWithBox(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  // the box QA drew: its corner (-64, 64) is on the starting camera's line
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-192, -64, 0], max: [-64, 64, 128], texture: "CRATE01" });
  const id = r.created?.brushes?.[0] ?? r.doc?.brushes?.at(-1)?.id;
  expect(id, JSON.stringify(r).slice(0, 300)).toBeTruthy();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(2500);
  return id as number;
}

const bounds = async (page: Page, id: number) => {
  const b = (await api(page, "/api/tools/editor/doc")).brushes.find((x: { id: number }) => x.id === id);
  return JSON.stringify([b?.min, b?.max]);
};

test("push/pull a face from the exact middle of the 3D view (on the box's edge)", async ({ page }) => {
  const id = await openWithBox(page);
  await page.locator(".ed-layouts button").nth(0).click(); // Single
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  const mid: [number, number] = [r.x + r.width / 2, r.y + r.height / 2];
  // a click there picks the box (the ray runs along its corner edge)
  await page.mouse.click(...mid);
  await expect(page.locator(".ed-selsize")).toContainText("1 picked · 128 × 128 × 128");
  const before = await bounds(page, id);
  await page.mouse.move(...mid);
  await page.mouse.down();
  await page.mouse.move(mid[0], mid[1] - 35, { steps: 5 });
  await page.mouse.move(mid[0], mid[1] - 70, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => bounds(page, id)).not.toBe(before);
  expect((await api(page, "/api/tools/editor/doc")).undo).toBe("Move face");
  // the face lands on the grid (the picked point is on the face, not just near it)
  for (const v of JSON.parse(await bounds(page, id)).flat()) expect(Number.isInteger(v), String(v)).toBe(true);
});

test("one pane has the drawing style picker", async ({ page }) => {
  await openWithBox(page);
  await page.locator(".ed-layouts button").nth(0).click(); // Single
  const pick = page.getByLabel("How pane 1 draws");
  await expect(pick).toBeVisible();
  await expect(page.getByLabel("What pane 1 shows")).toHaveCount(0); // (the top bar picks the view)
  await pick.selectOption({ index: 2 });
  await expect(pick).not.toHaveValue("textured");
});

test("saving a prefab under a name you already use: keep both or replace", async ({ page }) => {
  await openWithBox(page);
  const name = `P5 test crate ${Date.now()}`;
  const mine = async () => ((await api(page, "/api/tools/editor/prefabs")).prefabs as { id: string; uid: string; name: string; builtin?: boolean }[]).filter((p) => !p.builtin && p.name.startsWith(name));
  const save = async (choice?: RegExp) => {
    await page.getByRole("button", { name: "Save what's picked…" }).click();
    await page.getByRole("dialog").getByLabel("Name", { exact: true }).fill(name);
    if (choice) await page.getByRole("button", { name: choice }).click();
    await page.getByRole("dialog").getByRole("button", { name: /^(Save|Replace) prefab$/ }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
  };
  await page.locator(".ed-layouts button").nth(0).click();
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(r.x + r.width / 2, r.y + r.height / 2);
  await expect(page.locator(".ed-selsize")).toContainText("128 × 128 × 128");
  // (prefabs are in the Content Browser: Content › Prefabs)
  await page.locator('[data-dock-strip] [role=tab]', { hasText: /^Content$/ }).first().click();
  await page.getByRole("tablist", { name: "Content" }).getByRole("tab", { name: "Prefabs" }).click();
  await save();
  await save(/^Keep both/);
  const before = await mine();
  expect(before.map((p) => p.name).sort()).toEqual([name, `${name} 2`]);
  // each has its own permanent uid
  expect(new Set(before.map((p) => p.uid)).size).toBe(2);
  for (const p of before) expect(p.uid).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  await save(/^Replace it$/);
  const after = await mine();
  expect(after.map((p) => p.name).sort()).toEqual([name, `${name} 2`]);
  // Replace kept the replaced prefab's uid and id (no new prefab, nothing deleted)
  const old = before.find((p) => p.name === name)!;
  expect(after.find((p) => p.name === name)).toMatchObject({ id: old.id, uid: old.uid });
  for (const p of after) await api(page, "/api/tools/editor/prefabs/delete", { id: p.id });
});
