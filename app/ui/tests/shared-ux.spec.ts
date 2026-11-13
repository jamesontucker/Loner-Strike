// The shared Tools pieces (views/tools/common, ui.tsx): the save bar, its More menu and the
// reset sheet, tooltips above everything, segmented controls, tool icons and the map browser.
// Nothing here saves or resets a file (the reset sheet is cancelled).
import { expect, test, type Page } from "@playwright/test";

const open = async (page: Page, address: string) => {
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);
  await page
    .waitForFunction(() => !document.querySelector(".skeleton, .tools-loading") && !/Reading the map|Reading the nav/.test(document.body.innerText), null, { timeout: 20_000 })
    .catch(() => {});
  await page.waitForTimeout(500);
};

for (const tool of ["people", "playercfg"]) {
  test(`save bar on ${tool}: pinned at the bottom, after the content, More and Reset`, async ({ page }, info) => {
    await open(page, tool);
    const bar = page.locator(".tl-actionbar");
    await expect(bar).toHaveCount(1);
    await expect(bar.getByRole("button", { name: "Save" })).toBeVisible();
    // pinned at the bottom of the window while there's more page below
    const vh = page.viewportSize()!.height;
    const top = await bar.evaluate((el) => el.getBoundingClientRect());
    expect(Math.abs(top.bottom - vh)).toBeLessThanOrEqual(2);
    expect(top.height).toBeLessThan(80);
    await page.screenshot({ path: info.outputPath(`${tool}-bar.png`) });
    // at the end of the page it sits after the content: nothing under it
    await page.evaluate(() => document.querySelector(".content")!.scrollTo(0, 1e7));
    await page.waitForTimeout(200);
    const covered = await bar.evaluate((b) => {
      // a field that shows at its middle point without the bar, and is under the bar with it
      const page = b.parentElement!;
      const fields = [...page.querySelectorAll<HTMLElement>("input, select, button, textarea, summary")].filter((el) => !b.contains(el) && el.offsetParent);
      const mid = (el: HTMLElement) => {
        const e = el.getBoundingClientRect();
        return [e.left + e.width / 2, e.top + e.height / 2] as const;
      };
      b.style.visibility = "hidden";
      const shown = fields.filter((el) => {
        const hit = document.elementFromPoint(...mid(el));
        return !!hit && (el === hit || el.contains(hit));
      });
      b.style.visibility = "";
      return shown.filter((el) => {
        const hit = document.elementFromPoint(...mid(el));
        return !!hit && b.contains(hit);
      }).length;
    });
    expect(covered, "fields under the bar at the end of the page").toBe(0);
    await page.screenshot({ path: info.outputPath(`${tool}-bar-end.png`) });
    // More: a menu in the page body, on screen, with Reset at the end
    await bar.getByRole("button", { name: "More" }).click();
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();
    expect(await menu.evaluate((m) => m.parentElement === document.body)).toBe(true);
    const box = (await menu.boundingBox())!;
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.y + box.height).toBeLessThanOrEqual(vh);
    await page.screenshot({ path: info.outputPath(`${tool}-more.png`) });
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    await bar.getByRole("button", { name: "More" }).click();
    await page.getByRole("menuitem", { name: /Reset to Loner-Strike's default/ }).click();
    // either the sheet naming the files, or a note that everything is already the default
    const sheet = page.getByRole("dialog", { name: "Reset to Loner-Strike's default" });
    const said = page.locator(".toast", { hasText: /already Loner-Strike's default/ });
    await expect(sheet.or(said)).toBeVisible();
    if (await sheet.isVisible()) {
      await expect(sheet.locator(".tl-reset-files li").first()).toBeVisible();
      await page.screenshot({ path: info.outputPath(`${tool}-reset.png`) });
      await sheet.getByRole("button", { name: "Cancel" }).click();
      await expect(sheet).toHaveCount(0);
    }
  });
}

test("save bar in focus mode stays at the bottom", async ({ page }, info) => {
  await open(page, "people");
  await page.keyboard.press(process.platform === "darwin" ? "Shift+Meta+Enter" : "Shift+Control+Enter");
  await page.waitForTimeout(300);
  const r = await page.locator(".tl-actionbar").evaluate((el) => el.getBoundingClientRect());
  expect(Math.abs(r.bottom - page.viewportSize()!.height)).toBeLessThanOrEqual(2);
  await page.screenshot({ path: info.outputPath("people-focus-bar.png") });
  await page.keyboard.press(process.platform === "darwin" ? "Shift+Meta+Enter" : "Shift+Control+Enter");
});

test("tooltips go above everything and stay on screen", async ({ page }, info) => {
  await open(page, "playercfg");
  const btn = page.locator(".info-btn").first();
  await btn.hover();
  const pop = page.locator(".info-pop");
  await expect(pop).toBeVisible();
  const where = await pop.evaluate((el) => ({ body: el.parentElement === document.body, z: Number(getComputedStyle(el).zIndex), r: el.getBoundingClientRect().toJSON() }));
  expect(where.body).toBe(true);
  expect(where.z).toBeGreaterThanOrEqual(1000);
  const vp = page.viewportSize()!;
  expect(where.r.left).toBeGreaterThanOrEqual(0);
  expect(where.r.right).toBeLessThanOrEqual(vp.width);
  expect(where.r.top).toBeGreaterThanOrEqual(0);
  expect(where.r.bottom).toBeLessThanOrEqual(vp.height);
  await page.screenshot({ path: info.outputPath("tooltip.png") });
});

test("segmented choices are separate buttons (Community's kinds)", async ({ page }) => {
  await open(page, "community");
  const kinds = page.getByRole("group", { name: "Kind" }).getByRole("button");
  expect(await kinds.count()).toBeGreaterThanOrEqual(5);
  await expect(kinds.first()).toHaveText("All");
  // no pill anywhere in Tools is a whole list run together
  const joined = await page.locator(".seg button").evaluateAll((bs) => bs.map((b) => b.textContent ?? "").filter((t) => /\w,\w/.test(t)));
  expect(joined).toEqual([]);
});

test("every tool has an icon of its own", async ({ page }, info) => {
  await open(page, "overview");
  const icons = await page.locator(".tools-card").evaluateAll((cards) => cards.map((c) => c.querySelector("svg")?.innerHTML ?? ""));
  expect(icons.length).toBeGreaterThan(10);
  expect(icons.every(Boolean)).toBe(true);
  expect(new Set(icons).size).toBe(icons.length);
  await page.screenshot({ path: info.outputPath("overview-icons.png") });
});

// the map browser, on every start page that uses it
for (const tool of ["doctor", "navfix", "callouts", "radar"]) {
  test(`map browser on ${tool}: search, filters, keys, only on-screen cards drawn`, async ({ page }, info) => {
    await open(page, tool);
    const mb = page.locator(".tl-mb");
    test.skip((await mb.count()) === 0, `${tool} doesn't use the map browser yet`);
    const total = Number((await page.locator(".tl-mb-count").innerText()).match(/\d+/)![0]);
    const drawn = await page.locator(".tl-mb-card").count();
    if (total > 60) expect(drawn).toBeLessThan(total);
    await page.screenshot({ path: info.outputPath(`${tool}-browser.png`) });
    // search narrows as you type; ↓ goes to the first card, → the next, Enter opens it
    await page.locator(".tl-mb-search").fill("de_");
    await expect(page.locator(".tl-mb-count")).toContainText(" of ");
    await page.locator(".tl-mb-search").press("ArrowDown");
    await expect(page.locator(".tl-mb-card").first()).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(page.locator(".tl-mb-card").nth(1)).toBeFocused();
    // the list view, remembered
    await page.getByRole("button", { name: "List" }).click();
    await expect(page.locator(".tl-mb-card.list").first()).toBeVisible();
    await page.screenshot({ path: info.outputPath(`${tool}-browser-list.png`) });
    await page.getByRole("button", { name: "Cards" }).click();
  });
}
