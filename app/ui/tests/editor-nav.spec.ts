// Map editor: nothing covers the 3D view, and the camera moves (Unreal: right mouse + W) even
// when a text box had the keys; in Single and Quad. Runs on editor-dev.html against the same
// dev server as tools.spec.ts (npm run test:tools -- tests/editor-nav.spec.ts).
import { expect, test, type Page } from "@playwright/test";

async function openEditor(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  // (the page opens on the start screen: a new map, then into it)
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(2500);
}

/** The element on top at each shown pane's middle (must be the pane's own box). */
async function onTop(page: Page) {
  return page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>(".ed-pane")]
      .filter((p) => p.offsetParent !== null && p.getBoundingClientRect().width > 0)
      .map((p) => {
        const r = p.getBoundingClientRect();
        const el = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) as HTMLElement | null;
        return { pane: p.dataset.pane, top: el?.closest<HTMLElement>("[data-pane]")?.dataset.pane ?? el?.className ?? null };
      }),
  );
}

async function flies(page: Page) {
  const pane = page.locator(".ed-pane").first();
  const r = (await pane.boundingBox())!;
  const shot = () => page.screenshot({ clip: { x: r.x, y: r.y, width: r.width, height: r.height } });
  // a text box has the keys first
  await page.getByLabel("Find in the map").click();
  const before = await shot();
  await page.mouse.move(r.x + r.width / 2, r.y + r.height / 2);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(r.x + r.width / 2 + 4, r.y + r.height / 2, { steps: 2 });
  await page.keyboard.down("w");
  await page.waitForTimeout(600);
  await page.keyboard.up("w");
  await page.mouse.up({ button: "right" });
  await page.waitForTimeout(300);
  expect(Buffer.compare(before, await shot()), "the view moved").not.toBe(0);
  await expect(page.getByLabel("Find in the map")).toHaveValue("");
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
}

test("nothing covers the views, and right mouse + W flies (Single)", async ({ page }) => {
  await openEditor(page);
  await page.locator(".ed-layouts button").nth(0).click();
  for (const p of await onTop(page)) expect(p.top, `pane ${p.pane}`).toBe(p.pane);
  await flies(page);
});

test("nothing covers the views in Quad, and the 3D pane flies", async ({ page }) => {
  await openEditor(page);
  await page.locator(".ed-layouts button").nth(1).click();
  await page.waitForTimeout(800);
  const tops = await onTop(page);
  expect(tops.length).toBe(4);
  for (const p of tops) expect(p.top, `pane ${p.pane}`).toBe(p.pane);
  await flies(page);
  await page.locator(".ed-layouts button").nth(0).click();
});
