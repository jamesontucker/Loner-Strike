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
