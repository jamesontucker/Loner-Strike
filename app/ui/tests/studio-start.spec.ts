// Game Mode Studio's start page with modes of your own (YourModes.tsx): cards in a grid with the
// actions inside, a search and a sort past six, Your modes first for a returning creator (the
// intro and the tour's banner only for someone new). The mode list is stubbed: 0, 1, 8, 30 modes,
// at 1440, 1024 and 390 wide.
//   npx playwright test studio-start --project large --project webkit
import { expect, test, type Page } from "@playwright/test";

const NOW = Math.floor(Date.now() / 1000);
const modes = (n: number) =>
  Array.from({ length: n }, (_, k) => ({
    id: `pw_start_${k}`,
    name: `${["Zombie", "Arena", "Gun", "Hide", "Race", "Tag"][k % 6]} mode ${k + 1}`,
    description: k % 3 === 0 ? "A long description that goes on for a while to see that it stays inside its card and wraps onto two lines at most" : `Mode number ${k + 1}`,
    nodes: 10 + k,
    // the 4th is the newest, then by number
    modified: k === 3 ? NOW - 30 : NOW - 3600 * (k + 1),
    installed: k % 4 === 0,
  }));

async function stub(page: Page, n: number) {
  await page.route("**/api/tools/studio/graphs", (r) => r.fulfill({ json: { graphs: modes(n) } }));
  await page.route("**/api/tools/studio/graph/*/delete", (r) => r.fulfill({ json: { ok: true } }));
  await page.goto("/tools-dev.html?open=studio");
  await expect(page.getByTestId("studio-examples")).toBeVisible({ timeout: 30_000 });
}

/** No sideways scrolling, and every card's actions inside the card. */
async function fits(page: Page) {
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(over, "no sideways scroll").toBeLessThanOrEqual(1);
  const outside = await page.locator(".st-mode-card").evaluateAll((cards) =>
    cards.filter((c) => {
      const b = c.getBoundingClientRect();
      const m = c.querySelector(".st-mode-more-btn")!.getBoundingClientRect();
      return m.left < b.left || m.right > b.right + 0.5 || m.top < b.top || m.bottom > b.bottom + 0.5;
    }).length,
  );
  expect(outside, "the menu button sits inside each card").toBe(0);
}

const SIZES: [number, number][] = [
  [1440, 900],
  [1024, 680],
  [390, 844],
];

test("someone new: the intro and the tour banner, Your modes last", async ({ page }) => {
  await page.addInitScript(() => localStorage.removeItem("ls.studio.tour"));
  await stub(page, 0);
  await expect(page.getByTestId("studio-tour-offer")).toBeVisible();
  await expect(page.getByText("None yet: start a Blank mode")).toBeVisible();
  const yours = await page.getByRole("heading", { name: "Your modes" }).boundingBox();
  const ex = await page.getByRole("heading", { name: "Example modes" }).boundingBox();
  expect(yours!.y).toBeGreaterThan(ex!.y);
});

test("one mode: Your modes first, a card with its actions inside, no search", async ({ page }) => {
  await page.addInitScript(() => localStorage.removeItem("ls.studio.tour"));
  await stub(page, 1);
  const list = page.getByTestId("studio-your-modes");
  await expect(list.locator(".st-mode-card")).toHaveCount(1);
  await expect(page.getByTestId("studio-tour-offer")).toHaveCount(0);
  const yours = await page.getByRole("heading", { name: "Your modes" }).boundingBox();
  const ex = await page.getByRole("heading", { name: "Example modes" }).boundingBox();
  expect(yours!.y).toBeLessThan(ex!.y);
  await expect(page.getByRole("searchbox", { name: "Find a mode" })).toHaveCount(0);
  const card = list.locator(".st-mode-card").first();
  await expect(card).toContainText("Zombie mode 1");
  await expect(card).toContainText("Edited 1 hour ago");
  await expect(card.locator(".st-tag")).toHaveText("Installed");
  // the menu: Remove from game and Delete, which asks first
  await card.getByRole("button", { name: "More for Zombie mode 1" }).click();
  const menu = page.getByRole("menu");
  await expect(menu.getByRole("menuitem")).toHaveText(["Remove from game", "Delete…"]);
  await menu.getByRole("menuitem", { name: "Delete…" }).click();
  const ask = page.getByRole("alertdialog");
  await expect(ask).toContainText("Delete the graph “Zombie mode 1”?");
  await ask.getByRole("button", { name: "Cancel" }).click();
  await expect(card).toBeVisible();
  // Escape closes the menu
  await card.getByRole("button", { name: "More for Zombie mode 1" }).click();
  await expect(page.getByRole("menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  for (const [w, h] of SIZES) {
    await page.setViewportSize({ width: w, height: h });
    await fits(page);
  }
});

test("eight modes: search and sort, most recent first", async ({ page }) => {
  await stub(page, 8);
  const list = page.getByTestId("studio-your-modes");
  const names = () => list.locator(".st-mode-card strong").allTextContents();
  await expect(list.locator(".st-mode-card")).toHaveCount(8);
  expect((await names())[0]).toBe("Hide mode 4");
  expect((await names())[1]).toBe("Zombie mode 1");
  await page.getByRole("combobox", { name: "Sort your modes" }).selectOption("name");
  const byName = await names();
  expect(byName).toEqual([...byName].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" })));
  await page.getByRole("searchbox", { name: "Find a mode" }).fill("arena");
  await expect(list.locator(".st-mode-card")).toHaveCount(2);
  await expect(list).toContainText("2 of 8");
  await page.getByRole("searchbox", { name: "Find a mode" }).fill("nothing like this");
  await expect(list).toContainText("No mode matches");
  await page.getByRole("searchbox", { name: "Find a mode" }).fill("");
  for (const [w, h] of SIZES) {
    await page.setViewportSize({ width: w, height: h });
    await fits(page);
  }
});

test("thirty modes: a grid that holds at every width", async ({ page }) => {
  await stub(page, 30);
  const cards = page.getByTestId("studio-your-modes").locator(".st-mode-card");
  await expect(cards).toHaveCount(30);
  for (const [w, h] of SIZES) {
    await page.setViewportSize({ width: w, height: h });
    await fits(page);
    // columns: several when wide, one on a phone
    const xs = new Set((await cards.evaluateAll((cs) => cs.slice(0, 6).map((c) => Math.round(c.getBoundingClientRect().left)))));
    if (w >= 1024) expect(xs.size).toBeGreaterThan(1);
    else expect(xs.size).toBe(1);
  }
  // a menu near the right edge stays on screen
  await page.setViewportSize({ width: 390, height: 844 });
  await cards.first().getByRole("button", { name: /More for/ }).click();
  const m = await page.getByRole("menu").boundingBox();
  expect(m!.x).toBeGreaterThanOrEqual(0);
  expect(m!.x + m!.width).toBeLessThanOrEqual(391);
});
