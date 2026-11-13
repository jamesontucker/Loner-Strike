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
const pages = [...new Set(["overview", ...ids, `map:${MAP}`, `doctor:${MAP}`, `navfix:${NAV_MAP}`, `callouts:${NAV_MAP}`])];

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
  await expect(page.getByPlaceholder("Pick a map to check")).toBeVisible();
  await expect(page).toHaveURL(/open=doctor(&|$)/);
  // Back: the map again (the browser's history, like the mouse's back button)
  await crumbs.getByRole("button", { name: "Back" }).click();
  await expect(crumbs.locator("[aria-current=page]")).toHaveText(MAP);
  // Forward with Alt+→, then Tools: the overview
  await page.locator("body").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("Alt+ArrowRight");
  await expect(page.getByPlaceholder("Pick a map to check")).toBeVisible();
  await crumbs.getByRole("button", { name: "Tools" }).click();
  await expect(page.locator(".tools-cards").first()).toBeVisible();
  await page.goBack();
  await expect(page.getByPlaceholder("Pick a map to check")).toBeVisible();
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
