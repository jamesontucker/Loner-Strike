// QA4-T1 fixes in Creature Studio: what it edits (target line), a blank number field says so
// and blocks Save, arrow keys in the types list, the new-type dialog keeps a taken name open,
// the Waves tab starts from Last Stand's own lineup (own rounds) with a reset, and the form
// stays in view at 800x600. Runs against a dev server over a sim copy of the game (it saves
// waves.ini there): never the real game.
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

test("Creature Studio: target line, blank numbers, keyboard list, new-type dialog", async ({ page }) => {
  const problems: string[] = [];
  await open(page, problems);
  await expect(page.locator(".cs-target")).toContainText(/Editing creatures for .* on this computer/);
  await expect(page.locator(".cs-target a")).toHaveAttribute("href", /#creatures-how$/);
  // a talking character or a usable thing is made elsewhere: links to both
  const hint = page.locator(".cs-npc-hint");
  await expect(hint).toHaveText("Making a talking character or something to use? Place an NPC or interactable in the map editor or Game Mode Studio.");
  await expect(hint.getByRole("button", { name: "map editor" })).toBeVisible();
  await expect(hint.getByRole("button", { name: "Game Mode Studio" })).toBeVisible();

  // arrow keys move through the list
  const list = page.getByRole("listbox", { name: "Creature types" });
  await list.getByRole("option").first().click();
  const before = await page.locator(".cs-main h3").textContent();
  await page.keyboard.press("ArrowDown");
  await expect(page.locator(".cs-main h3")).not.toHaveText(before ?? "");
  await expect(list.getByRole("option", { selected: true })).toBeFocused();
  await page.keyboard.press("Home");
  await expect(page.locator(".cs-main h3")).toHaveText(before ?? "");

  // a blank number: "Type a number." under it, and Save is blocked
  const hp = page.getByRole("spinbutton", { name: "Health (number)" });
  await hp.fill("");
  await hp.press("Tab");
  await expect(page.locator(".cs-field", { has: hp }).locator(".cs-error")).toHaveText("Type a number.");
  await expect(page.locator(".tl-actionbar").getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await expect(page.locator(".tl-actionbar")).toContainText(/to fix before saving/);
  await page.getByRole("button", { name: "Undo changes" }).click();

  // the new-type dialog: a taken name keeps it open with what was typed
  await page.getByRole("button", { name: "Duplicate" }).click();
  const dialog = page.getByRole("alertdialog");
  const box = dialog.getByRole("textbox", { name: "Type name" });
  const taken = (await list.getByRole("option").first().locator(".muted").textContent())!.split(" · ")[0];
  await box.fill(taken);
  await dialog.getByRole("button", { name: "Make it" }).click();
  await expect(dialog).toContainText(`There's already a type called ${taken}.`);
  await expect(box).toHaveValue(taken);
  // a name with other characters is cleaned, and it says so
  await box.fill("QA Copy!");
  await dialog.getByRole("button", { name: "Make it" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".toast", { hasText: "Named it qa_copy" })).toBeVisible();
  await page.getByRole("button", { name: "Undo changes" }).click();
  expect(problems).toEqual([]);
});

test("Creature Studio: Waves start from Last Stand's own lineup, with a reset", async ({ page }) => {
  const problems: string[] = [];
  await open(page, problems);
  await page.getByRole("button", { name: "Waves & spawn groups" }).click();
  await expect(page.locator(".cs-wave-row").first()).toBeVisible();
  // every type of the round table is a column, and boss/pack rounds are the mode's own
  for (const t of ["zombie", "babycrab", "vortigaunt", "soldier", "controller", "stukabat"]) await expect(page.getByRole("columnheader", { name: t, exact: true })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Boss type" })).toHaveValue("\u0000own");
  // start from the mode's own, add one cell, save: the boss stays the mode's own
  await page.getByRole("button", { name: "Start from the mode's own mix" }).click();
  await page.getByLabel("Wave 1: zombie", { exact: true }).fill("90");
  await page.locator(".tl-actionbar").getByRole("button", { name: "Save waves" }).click();
  await expect(page.locator(".toast", { hasText: "Saved Last Stand's waves." })).toBeVisible();
  await expect(page.locator(".cs-waves")).toContainText("This game's Last Stand uses your list");
  // reset: the section goes, the lineup is the mode's own again
  await page.getByRole("button", { name: "Reset to Last Stand's own" }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Reset" }).click();
  await expect(page.locator(".toast", { hasText: "Back to Last Stand's own waves." })).toBeVisible();
  await expect(page.locator(".cs-waves")).toContainText("Saving makes this game's Last Stand use your list");
  await expect(page.getByRole("button", { name: "Reset to Last Stand's own" })).toHaveCount(0);
  expect(problems).toEqual([]);
});

test("Creature Studio: at 800x600 the list and the form come before the preview", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  const problems: string[] = [];
  await open(page, problems);
  const main = await page.locator(".cs-main").boundingBox();
  const side = await page.locator(".cs-side").boundingBox();
  expect(main!.y).toBeLessThan(600);
  expect(side!.y).toBeGreaterThan(main!.y);
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(over).toBeLessThanOrEqual(1);
  expect(problems).toEqual([]);
});
