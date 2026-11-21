// Map editor 2D views: zooming toward the mouse or panning, then anything that redraws (a click),
// must keep the view looking straight on and where you left it. It used to snap back to its first
// spot while still aiming at the zoomed-to point, so the view tilted (seen in a tester's video).
//   npx playwright test tests/editor-flat-camera.spec.ts --project large
import { expect, test, type Page } from "@playwright/test";

const cam = (page: Page, view: string) =>
  page.evaluate((v) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const c = (window as any).__lsFlatCams?.[v];
    const d = c.getWorldDirection(c.position.clone());
    return { dir: [d.x, d.y, d.z].map((x: number) => Math.round(x * 1e4) / 1e4), pos: [c.position.x, c.position.y].map(Math.round) };
  }, view);

test("2D views stay straight and put after zoom, pan and clicks", async ({ page }) => {
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator(".ed-layouts button").nth(1).click(); // four views
  await page.waitForTimeout(2500);
  const f = (await page.locator(".ed-pane").nth(2).boundingBox())!;
  const straight = (await cam(page, "front")).dir;
  expect(straight.map(Math.abs)).toEqual([0, 0, 1]);

  // zoom toward a corner, then click: same place, still straight
  await page.mouse.move(f.x + f.width * 0.15, f.y + f.height * 0.8);
  for (let i = 0; i < 8; i++) await page.mouse.wheel(0, -200);
  await page.waitForTimeout(300);
  const zoomed = await cam(page, "front");
  await page.mouse.click(f.x + f.width * 0.5, f.y + f.height * 0.5);
  await page.waitForTimeout(400);
  expect(await cam(page, "front")).toEqual(zoomed);

  // pan, then deselect: still straight
  await page.mouse.move(f.x + f.width * 0.5, f.y + f.height * 0.5);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(f.x + f.width * 0.5 + 120, f.y + f.height * 0.5 - 40, { steps: 6 });
  await page.mouse.up({ button: "right" });
  await page.keyboard.press("Escape");
  await page.waitForTimeout(400);
  expect((await cam(page, "front")).dir).toEqual(straight);
  await page.screenshot({ path: test.info().outputPath("front-after.png") });

  // switching the pane to Side still shows Side, straight on
  await page.locator(".ed-pane").nth(2).locator("select").first().selectOption("side");
  await page.waitForTimeout(500);
  expect((await cam(page, "side")).dir.map(Math.abs)).toEqual([1, 0, 0]);
  await page.screenshot({ path: test.info().outputPath("side-after.png") });
});
