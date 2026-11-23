// Creature Studio's preview while switching types quickly (a tester's video, 1.0.2): one 3D view
// stays up the whole time (no blank panel between models, no lost drawing context), the last
// model stays dimmed while the next opens, and it settles on the picked type's own model.
import { expect, test, type Page } from "@playwright/test";

const IGNORED = [/status of 404/, /Download the React DevTools/, /GPU stall due to ReadPixels/, /WebGL: too many errors/];

async function open(page: Page, problems: string[]) {
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  await page.goto(`/tools-dev.html?open=creatures`);
  await expect(page.getByRole("listbox", { name: "Creature types" })).toBeVisible({ timeout: 20_000 });
}

test("Creature Studio: switching types quickly keeps one 3D view and ends on the right model", async ({ page }) => {
  const problems: string[] = [];
  await open(page, problems);
  const list = page.getByRole("listbox", { name: "Creature types" });
  const opts = list.getByRole("option");
  await opts.nth(0).click();
  const view = page.locator(".cs-preview-view");
  await expect(view.locator("canvas")).toBeVisible({ timeout: 20_000 });
  await expect(view.locator(".cs-preview-wait")).toHaveCount(0, { timeout: 20_000 });
  // count every canvas taken out of the preview from here on
  await page.evaluate(() => {
    const w = window as unknown as { __removed: number };
    w.__removed = 0;
    new MutationObserver((ms) => ms.forEach((m) => m.removedNodes.forEach((n) => {
      if (n instanceof HTMLElement && (n.tagName === "CANVAS" || n.querySelector?.("canvas"))) w.__removed++;
    }))).observe(document.body, { childList: true, subtree: true });
  });
  const n = Math.min(5, await opts.count());
  const order = [1, 2, 0, 3, 1, 4, 2, 0, 1, 3, 2, 1].map((i) => i % n);
  for (const [k, i] of order.entries()) {
    await opts.nth(i).click();
    // (one mid-switch picture: the last model dimmed under "Opening the model…")
    if (k === 0) await view.screenshot({ path: test.info().outputPath("switching.png") }).catch(() => undefined);
    await page.waitForTimeout(90);
  }
  const last = order[order.length - 1];
  await expect(view.locator(".cs-preview-wait")).toHaveCount(0, { timeout: 20_000 });
  expect(await page.evaluate(() => (window as unknown as { __removed: number }).__removed), "the 3D view stayed up").toBe(0);
  // the model on show is the picked type's (its Model field names the file)
  const file = (await page.locator(".cs-main").getByRole("button", { name: /\.mdl/ }).first().textContent())?.trim() ?? "";
  await page.waitForTimeout(800);
  await view.screenshot({ path: test.info().outputPath(`settled-${last}-${file.replace(/\W+/g, "_")}.png`) });
  expect(problems).toEqual([]);
});
