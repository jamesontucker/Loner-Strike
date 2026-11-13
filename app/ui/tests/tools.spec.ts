// Every Tools page, opened by its address on tools-dev.html (see playwright.config.ts).
// LS_TOOLS_MAP / LS_TOOLS_NAV_MAP pick the maps (default de_dust2 / cs_rats2: any map with a
// nav works for the nav pages).
import { expect, test, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const MAP = process.env.LS_TOOLS_MAP || "de_dust2";
const NAV_MAP = process.env.LS_TOOLS_NAV_MAP || "cs_rats2";

// every tool in the registry, plus the ones that take a map
const registry = readFileSync(fileURLToPath(new URL("../src/views/tools/registry.tsx", import.meta.url)), "utf8");
const ids = [...registry.matchAll(/^\s*\{ id: "([a-z0-9]+)"/gm)].map((m) => m[1]);
const pages = [...new Set(["overview", ...ids, `map:${MAP}`, `doctor:${MAP}`, `navfix:${NAV_MAP}`, `callouts:${NAV_MAP}`, `radar:${MAP}`])];

/** Console noise that isn't ours to fix here. */
const IGNORED = [/status of 404/, /Download the React DevTools/, /GPU stall due to ReadPixels/, /WebGL: too many errors/];

async function settle(page: Page) {
  // the page's own loading lines and skeletons gone (or 20 s, whichever first)
  await page
    .waitForFunction(() => !document.querySelector(".skeleton, .tools-loading, [aria-busy='true']") && !/Reading the map|Reading the nav/.test(document.body.innerText), null, { timeout: 20_000 })
    .catch(() => {});
  await page.waitForTimeout(600);
}

for (const address of pages) {
  test(`Tools page ${address}`, async ({ page }, info) => {
    const problems: string[] = [];
    page.on("console", (m) => {
      if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
    });
    page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));

    await page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);
    await settle(page);

    // the page's crash screen ("Something went wrong") isn't up
    await expect(page.locator(".page-error, .load-error h3:has-text('Something went wrong')")).toHaveCount(0);
    // nothing wider than the window: no sideways scrolling of the whole page
    const over = await page.evaluate(() => {
      const w = document.documentElement.clientWidth;
      const wide = [...document.querySelectorAll<HTMLElement>(".content *")]
        .filter((el) => el.offsetParent && el.getBoundingClientRect().right > w + 1 && getComputedStyle(el).position !== "fixed")
        .slice(0, 3)
        .map((el) => `${el.tagName.toLowerCase()}.${[...el.classList].join(".")} ends at ${Math.round(el.getBoundingClientRect().right)} (window ${w})`);
      return { page: document.documentElement.scrollWidth - w, wide };
    });
    await page.screenshot({ path: info.outputPath(`${address.replace(/[^a-z0-9_]+/gi, "-")}.png`) });
    expect(over.page, `the page scrolls sideways: ${over.wide.join("; ")}`).toBeLessThanOrEqual(1);
    expect(over.wide, "something sticks out past the window").toEqual([]);
    expect(problems).toEqual([]);
  });
}

test("? shows the keys, and Go to moves the viewer", async ({ page }) => {
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`map:${MAP}`)}`);
  await settle(page);
  await page.keyboard.press("?");
  await expect(page.getByRole("dialog", { name: "Keys" })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.keyboard.press("g");
  await page.keyboard.type("0 0 0");
  await page.keyboard.press("Enter");
  await expect(page.locator(".tools-status")).not.toHaveText(/—/);
});

test("the crumbs and Back step out of a map and back to the start", async ({ page }, info) => {
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`doctor:${MAP}`)}`);
  await settle(page);
  const crumbs = page.getByRole("navigation", { name: "Where you are in Tools" });
  await expect(crumbs.locator("[aria-current=page]")).toHaveText(MAP);
  await page.screenshot({ path: info.outputPath("crumbs-doctor-map.png") });
  // the tool's name: its start page (the map list), in one click
  await crumbs.getByRole("button", { name: "Map doctor" }).click();
  await expect(page.getByText("Pick a map to check", { exact: true })).toBeVisible();
  await expect(page).toHaveURL(/open=doctor(&|$)/);
  // Back: the map again (the browser's history, like the mouse's back button)
  await crumbs.getByRole("button", { name: "Back" }).click();
  await expect(crumbs.locator("[aria-current=page]")).toHaveText(MAP);
  // Forward with Alt+→, then Tools: the overview
  await page.locator("body").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Alt+ArrowRight");
  await expect(page.getByText("Pick a map to check", { exact: true })).toBeVisible();
  await crumbs.getByRole("button", { name: "Tools" }).click();
  await expect(page.locator(".tools-cards").first()).toBeVisible();
  await page.goBack();
  await expect(page.getByText("Pick a map to check", { exact: true })).toBeVisible();
});

test("focus mode: the tool fills the window, the key and the pill bring the app back", async ({ page }, info) => {
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`map:${MAP}`)}`);
  await settle(page);
  // (each test starts with nothing remembered)
  await page.evaluate(() => localStorage.removeItem("ls.tools.focus"));
  const canvas = page.locator(".tools-canvas canvas").first();
  const before = await canvas.boundingBox();
  await page.getByRole("button", { name: "Focus", exact: true }).click();
  await expect(page.locator(".sidebar")).toBeHidden();
  await expect(page.locator(".tools-head")).toBeHidden();
  const pill = page.getByRole("button", { name: /Exit focus/ });
  await expect(pill).toBeVisible();
  // the 3D view takes the room (its canvas follows its box)
  await expect.poll(async () => (await canvas.boundingBox())?.width ?? 0).toBeGreaterThan((before?.width ?? 0) + 100);
  await page.screenshot({ path: info.outputPath("focus-map.png") });
  // remembered for this tool
  await page.reload();
  await settle(page);
  await expect(page.locator(".sidebar")).toBeHidden();
  // the key turns it off; the pill too
  await page.locator(".tools-canvas").click({ position: { x: 10, y: 10 } });
  await page.keyboard.press("Escape");
  await page.keyboard.press("ControlOrMeta+Shift+Enter");
  await expect(page.locator(".sidebar")).toBeVisible();
  await page.keyboard.press("ControlOrMeta+Shift+Enter");
  await expect(page.locator(".sidebar")).toBeHidden();
  await pill.click();
  await expect(page.locator(".sidebar")).toBeVisible();
  await expect(pill).toBeHidden();
});

// the editors in focus mode (turned on the way it's remembered): they fill the window, nothing sticks out
for (const address of ["editor", "studio", "plugins", `navfix:${NAV_MAP}`, "assets:models"]) {
  test(`focus mode ${address}`, async ({ page }, info) => {
    const tool = address.split(":")[0];
    await page.addInitScript((t) => localStorage.setItem("ls.tools.focus", JSON.stringify({ [t]: true })), tool);
    await page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);
    await settle(page);
    await expect(page.locator(".sidebar")).toBeHidden();
    await expect(page.getByRole("button", { name: /Exit focus/ })).toBeVisible();
    // the map editor with a map open (a new one, unless one is open from before): its 3D view
    // takes the window's width. (Game Mode Studio isn't opened: a new mode is saved to the game.)
    if (address === "editor") {
      const fresh = page.getByRole("button", { name: "New map" });
      if (await fresh.isVisible()) await fresh.click();
      await settle(page);
      const box = await page.locator(".content canvas").first().boundingBox();
      expect(box?.width ?? 0).toBeGreaterThan(page.viewportSize()!.width * 0.5);
    }
    const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    await page.screenshot({ path: info.outputPath(`focus-${address.replace(/[^a-z0-9_]+/gi, "-")}.png`) });
    expect(over).toBeLessThanOrEqual(1);
  });
}

// History & restore (common/History.tsx, safety.rs): the button on a tool that writes files and
// on Settings' sheet; read-only unless LS_TOOLS_WRITE=1 (that one saves and undoes in your game).
test("History opens on a tool that writes files and says what's kept", async ({ page }, info) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  await page.goto(`/tools-dev.html?open=people`);
  await settle(page);
  await page.getByRole("button", { name: "History", exact: true }).click();
  const sheet = page.getByRole("dialog", { name: "History & restore" });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByText(/Reading the history/)).toHaveCount(0, { timeout: 15_000 });
  await page.screenshot({ path: info.outputPath("history-people.png") });
  // Compare shows what restoring would change (when something's been saved before)
  const compare = sheet.getByRole("button", { name: /^Compare/ });
  if (await compare.count()) {
    await compare.first().click();
    const diff = page.getByRole("dialog", { name: /^Compare / });
    await expect(diff).toBeVisible();
    await expect(diff.getByText(/Reading both versions/)).toHaveCount(0, { timeout: 15_000 });
    await page.screenshot({ path: info.outputPath("history-compare.png") });
    await diff.getByRole("button", { name: "Close" }).click();
  }
  await page.keyboard.press("Escape");
  await expect(sheet).toHaveCount(0);
  // a tool that only reads has no History button
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`map:${MAP}`)}`);
  await settle(page);
  await expect(page.locator(".tools-head").getByRole("button", { name: "History", exact: true })).toHaveCount(0);
  expect(problems).toEqual([]);
});

test("a save offers Undo, and Undo puts the file back", async ({ page }, info) => {
  test.skip(!process.env.LS_TOOLS_WRITE, "saves in the game folder: set LS_TOOLS_WRITE=1 (point LS_TEST_GAME at a copy)");
  await page.goto(`/tools-dev.html?open=people`);
  await settle(page);
  const name = `Zz Undo Test ${Date.now() % 100000}`;
  const names = () => page.evaluate(async () => (await (await fetch("/api/tools/people")).json()).names as string[]);
  // a save through the tools' own post(), as the page does it
  await page.evaluate(async (n) => {
    const api = await import("/src/views/tools/common/api.ts");
    await api.post("/api/tools/people/save", { names: { added: [n] } });
  }, name);
  expect(await names()).toContain(name);
  const toast = page.locator(".toast", { hasText: "Undo that save?" });
  await expect(toast).toBeVisible();
  await page.screenshot({ path: info.outputPath("undo-toast.png") });
  await toast.getByRole("button", { name: "Undo" }).click();
  await expect(page.locator(".toast", { hasText: /Undone/ })).toBeVisible();
  expect(await names()).not.toContain(name);
});

test("Creature Studio: a type's form, the preview drawn as the game would, the pickers and the waves", async ({ page }, info) => {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  await page.goto(`/tools-dev.html?open=creatures`);
  await settle(page);
  const list = page.getByRole("listbox", { name: "Creature types" });
  // the zombie's model has no run: the preview says so when Run is played
  await list.getByRole("option", { name: /Zombie/ }).click();
  await expect(page.locator(".cs-preview-view canvas")).toBeVisible({ timeout: 15_000 });
  await page.locator(".cs-moves").getByRole("button", { name: "Run" }).click();
  await expect(page.locator(".cs-preview .cs-note")).toContainText(/no run of its own/);
  // a tint shows in the preview (glow shell), as the game draws it
  await page.getByRole("switch", { name: "Tinted" }).first().click();
  await expect(page.locator(".cs-preview .cs-note")).toContainText(/Tinted as in the game/);
  await page.waitForTimeout(1200);
  await page.screenshot({ path: info.outputPath("creatures-zombie-tinted.png") });
  // the model picker and the sound picker open as popovers
  await list.getByRole("option", { name: /Spitter/ }).click();
  await expect(page.locator(".cs-summary")).toContainText(/shoots/);
  await page.locator(".cs-model-btn").click();
  await expect(page.getByRole("dialog", { name: "Pick a model" })).toBeVisible();
  await page.keyboard.press("Escape");
  const sounds = page.locator("details.cs-section").filter({ hasText: "Sounds" });
  if (!(await sounds.evaluate((d) => (d as HTMLDetailsElement).open))) await sounds.locator("summary").click();
  await sounds.getByRole("button", { name: "+ Add" }).first().click();
  await expect(page.getByRole("dialog", { name: /Add a .* sound/ })).toBeVisible();
  await page.screenshot({ path: info.outputPath("creatures-spitter-sounds.png") });
  await page.keyboard.press("Escape");
  // a value past the game's limit says so under its control
  await page.getByRole("spinbutton", { name: "Health (number)" }).fill("200000");
  await expect(page.locator(".cs-error").first()).toContainText(/between 1 and 100000/, { timeout: 5_000 });
  await page.getByRole("button", { name: "Undo changes" }).click();
  await page.getByRole("button", { name: "Waves & spawn groups" }).click();
  await expect(page.locator(".cs-wave-row").first()).toBeVisible();
  await page.screenshot({ path: info.outputPath("creatures-waves.png"), fullPage: true });
  const over = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(over).toBeLessThanOrEqual(1);
  expect(problems).toEqual([]);
});

// P9: the four map workspaces (radar maker, callouts, map doctor, nav fixer). Nothing is saved.
test("map workspaces: radar compare and zoom, callouts paint and undo, doctor checklist, nav fixer reach", async ({ page }, info) => {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  const open = async (address: string) => {
    await page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);
    await settle(page);
  };

  // start page: the map browser, with search
  await open("radar");
  await expect(page.locator(".tl-actionbar")).toHaveCount(0);
  await page.getByRole("searchbox").first().fill(MAP);
  await page.screenshot({ path: info.outputPath("radar-start.png") });

  // radar: the picture, the save bar's Install, side by side when there's an overview, zoom
  await open(`radar:${MAP}`);
  await expect(page.locator(".ov-pane img").first()).toBeVisible({ timeout: 20_000 });
  await expect(page.locator(".tl-actionbar").getByRole("button", { name: /Install/ })).toBeVisible();
  const split = page.getByRole("button", { name: "Side by side" });
  if (await split.count()) {
    await split.click();
    await expect(page.locator(".ov-pane")).toHaveCount(2);
  }
  await page.keyboard.press("+");
  await expect(page.locator(".ov-pz.zoomed").first()).toBeVisible();
  await page.keyboard.press("0");
  await page.screenshot({ path: info.outputPath("radar-map.png") });

  // callouts: paint a name with the brush, the bar says unsaved, Undo takes it back
  await open(`callouts:${NAV_MAP}`);
  await page.keyboard.press("p");
  await expect(page.getByRole("button", { name: "Paint", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.getByLabel("Place name").fill("TestPaint");
  const box = (await page.locator(".co-canvas canvas").boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.5);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.5, { steps: 12 });
  await page.mouse.up();
  await expect(page.locator(".tl-actionbar")).toContainText("Unsaved changes");
  await page.screenshot({ path: info.outputPath("callouts-paint.png") });
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(page.locator(".tl-actionbar")).not.toContainText("Unsaved changes");

  // doctor: the verdict, groups, all checks
  await open(`doctor:${MAP}`);
  await expect(page.locator(".doc-verdict")).toBeVisible();
  const all = page.getByRole("button", { name: "All checks" });
  if (await all.count()) await all.click();
  expect(await page.locator(".doc-group").count()).toBeGreaterThanOrEqual(4);
  await page.screenshot({ path: info.outputPath("doctor-checklist.png") });

  // nav fixer: the reach check counts add up to the areas, a kind can be shown
  await open(`navfix:${NAV_MAP}`);
  await expect(page.locator(".navfix-reach-row").first()).toBeVisible({ timeout: 30_000 });
  const row = page.locator(".navfix-reach-row:not([disabled])").nth(1);
  if (await row.count()) await row.click();
  await page.screenshot({ path: info.outputPath("navfix-reach.png") });

  expect(problems).toEqual([]);
});
