// Nav fixer: the 3D view's Fly camera (Orbit | Fly switch). Hold right mouse to look, WASD flies,
// and the tools' single keys (D draw, S split) stay off while flying.
//   npx playwright test tests/navfix-fly.spec.ts --project large
import { expect, test, type Page } from "@playwright/test";

const MAP = process.env.LS_TOOLS_NAV_MAP || "de_dust2";

const camAt = (page: Page) =>
  page.evaluate(() => {
    const t = (window as unknown as { __lsNavfix: { viewMemory: { target: { x: number; y: number; z: number } | null } } }).__lsNavfix.viewMemory.target;
    return t ? [t.x, t.y, t.z] : null;
  });

test("Fly: right mouse + W/D flies, D doesn't start drawing, the choice is kept", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.navfix.layout", "orbit");
    // only on the first load: the reload below checks the choice was kept
    if (!sessionStorage.getItem("fly-test")) localStorage.removeItem("ls.navfix.fly");
    sessionStorage.setItem("fly-test", "1");
  });
  await page.goto(`/tools-dev.html?open=navfix:${MAP}`);
  await page.locator(".navfix-view canvas").first().waitFor({ timeout: 60_000 });
  await expect(page.locator(".tools-overlay")).toHaveCount(0, { timeout: 60_000 });
  await page.waitForTimeout(1500);

  await page.getByRole("button", { name: "Fly" }).click();
  await expect(page.locator(".tools-hint")).toContainText("Hold right mouse + WASD to fly");
  await page.screenshot({ path: test.info().outputPath("navfix-fly-switch.png") });

  const r = (await page.locator('.navfix-view[data-view="orbit"]').boundingBox())!;
  const before = await camAt(page);
  await page.mouse.move(r.x + r.width / 2, r.y + r.height / 2);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(r.x + r.width / 2 + 20, r.y + r.height / 2, { steps: 4 });
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  await page.keyboard.down("d");
  await page.waitForTimeout(300);
  await page.keyboard.up("d");
  await page.mouse.up({ button: "right" });
  await page.waitForTimeout(300);
  const after = await camAt(page);
  expect(before && after && Math.hypot(after[0] - before[0], after[1] - before[1], after[2] - before[2])).toBeGreaterThan(50);
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath("navfix-fly-moved.png") });

  // after letting go, D is the draw tool again
  await page.keyboard.press("d");
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toBeVisible();
  await page.keyboard.press("Escape");

  // kept on this computer
  await page.reload();
  await page.locator(".navfix-view canvas").first().waitFor({ timeout: 60_000 });
  await expect(page.getByRole("button", { name: "Fly" })).toHaveAttribute("aria-pressed", "true");
});

/** Moves over the 3D view until the mouse is on a nav area; returns where. */
async function findArea(page: Page) {
  const r = (await page.locator('.navfix-view[data-view="orbit"]').boundingBox())!;
  for (const fy of [0.5, 0.4, 0.6, 0.3, 0.7]) {
    for (const fx of [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8]) {
      const x = r.x + r.width * fx, y = r.y + r.height * fy;
      await page.mouse.move(x, y, { steps: 2 });
      await page.waitForTimeout(60);
      const id = await page.evaluate(() => (window as unknown as { __lsNavfix: { useFix: { getState: () => { hoverArea: number | null } } } }).__lsNavfix.useFix.getState().hoverArea);
      if (id !== null) return { x, y, id };
    }
  }
  throw new Error("no nav area under the mouse");
}

test("Fly: clicks still pick, D + drag still draws, the hints follow, Top-down hides the switch", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.navfix.layout", "orbit");
    localStorage.setItem("ls.navfix.fly", "1");
  });
  await page.goto(`/tools-dev.html?open=navfix:${MAP}`);
  await page.locator(".navfix-view canvas").first().waitFor({ timeout: 60_000 });
  await expect(page.locator(".tools-overlay")).toHaveCount(0, { timeout: 60_000 });
  await page.waitForTimeout(1500);
  await expect(page.getByText("No unsaved changes")).toBeVisible();
  await expect(page.getByRole("button", { name: "Fly" })).toHaveAttribute("aria-pressed", "true");

  const a = await findArea(page);
  await page.mouse.click(a.x, a.y);
  await expect(page.locator(".tools-side h3", { hasText: `Area ${a.id}` })).toBeVisible();

  await page.keyboard.press("d");
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toContainText("middle-drag pans");
  await expect(page.locator(".tools-hint")).toContainText("middle-drag pans");
  await expect(page.locator(".tools-hint")).not.toContainText("right-drag");
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(a.x + 40, a.y + 30, { steps: 4 });
  await page.mouse.up();
  await expect(page.getByText(/^Added area \d+/)).toBeVisible({ timeout: 15_000 });
  await page.keyboard.press(process.platform === "darwin" ? "Meta+z" : "Control+z");
  await expect(page.getByText("No unsaved changes")).toBeVisible({ timeout: 15_000 });
  await page.keyboard.press("Escape");

  // Top-down: no 3D camera switch, and no fly hint
  await page.keyboard.press("t");
  await expect(page.getByRole("button", { name: "Fly" })).toHaveCount(0);
  await expect(page.locator(".tools-hint")).not.toContainText("fly");
  await page.keyboard.press("d");
  await expect(page.locator(".tools-hint")).toContainText("right-drag pans");
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toContainText("right-drag pans");
  await page.keyboard.press("Escape");
});
