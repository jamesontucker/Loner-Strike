// docs.html's navigation (Parity A): search with one result per heading, a snippet with the match
// marked, the keyboard, deep links and the match flashed; On this page; folding groups; Something's
// wrong?. `npx playwright test -c tests/site.config.ts`
import { expect, test, type Page } from "@playwright/test";

const errors = (page: Page) => {
  const out: string[] = [];
  page.on("pageerror", (e) => out.push(String(e)));
  page.on("console", (m) => {
    // (the page's own calls to lonerstrike.com's services fail from a local copy)
    if (m.type() === "error" && !/access control|Failed to load resource|ERR_|net::/i.test(m.text())) out.push(m.text());
  });
  return out;
};

test("search: one result per heading, a marked snippet, the keyboard, opening it", async ({ page }) => {
  const errs = errors(page);
  await page.goto("/docs.html");
  await page.keyboard.press("/");
  await expect(page.locator("#q")).toBeFocused();
  await page.keyboard.type("leak");
  const opts = page.locator("#results [role=option]");
  await expect(opts.first()).toBeVisible();
  // no place twice
  const ids = await opts.evaluateAll((os) => os.map((o) => o.getAttribute("data-id")));
  expect(new Set(ids).size).toBe(ids.length);
  // each with its topic and a snippet; the match marked
  await expect(opts.first().locator("small")).not.toBeEmpty();
  await expect(page.locator("#results mark").first()).toHaveText(/leak/i);
  // a title match first: "The map leaks" (Help) or the editor's leaks, before body-only hits
  await expect(opts.first().locator("b")).toHaveText(/leak/i);
  // the count is read out
  await expect(page.locator("#rcount")).toHaveText(/\d+ results?/);
  // ↓ ↑ move, the box says which (aria-activedescendant), Enter opens
  await expect(page.locator("#q")).toHaveAttribute("aria-activedescendant", "r0");
  await page.keyboard.press("ArrowDown");
  await expect(page.locator("#q")).toHaveAttribute("aria-activedescendant", "r1");
  await expect(opts.nth(1)).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowUp");
  const first = await opts.first().getAttribute("data-id");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(new RegExp(`#${first}$`));
  await expect(page.locator(`#${first}`)).toBeInViewport();
  await expect(page.locator(".flash")).toHaveCount(1);
  // the search box is cleared, the topics are back
  await expect(page.locator("#q")).toHaveValue("");
  await expect(page.locator("#tree")).toBeVisible();
  expect(errs).toEqual([]);
});

test("search: Esc clears; nothing found says so with two ways on", async ({ page }) => {
  await page.goto("/docs.html");
  await page.locator("#q").fill("zzqqxx");
  await expect(page.locator("#none")).toContainText("No results for “zzqqxx”");
  await expect(page.locator("#none a")).toHaveText(["Something's wrong?", "Getting started"]);
  await page.keyboard.press("Escape");
  await expect(page.locator("#q")).toHaveValue("");
  await expect(page.locator("#results-box")).toBeHidden();
});

test("a deep link opens its topic at the heading; On this page follows the reading", async ({ page }) => {
  await page.goto("/docs.html#compile-leak");
  await expect(page.locator("#tools-help")).toBeVisible();
  await expect(page.locator("#compile-leak")).toBeInViewport();
  const toc = page.locator("#toc");
  await expect(toc).toBeVisible();
  await expect(toc.locator("a")).toHaveCount(await page.locator("#tools-help").locator("h2[id], h3[id]").count());
  // scrolling marks the heading being read
  await page.locator("#tools-report").scrollIntoViewIfNeeded();
  await page.evaluate(() => document.getElementById("tools-report")!.scrollIntoView());
  await expect(toc.locator("a.on")).toHaveAttribute("href", "#tools-report");
  // a topic with fewer than 3 headings has none
  await page.goto("/docs.html#demos");
  await expect(toc).toBeHidden();
});

test("On this page is off on narrow screens; groups fold and remember; Something's wrong?", async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 800 });
  await page.goto("/docs.html#compile-leak");
  await expect(page.locator("#toc")).toBeHidden();
  await page.setViewportSize({ width: 1440, height: 900 });
  // a first visit: only Basics and the open topic's group are open
  await page.goto("/docs.html#demos");
  const open = () => page.locator('.group-btn[aria-expanded="true"] > span:first-child').allTextContents();
  expect((await open()).map((t) => t.trim())).toEqual(["Basics", "Watch & inspect"]);
  await expect(page.locator("#tree a[href='#bot-styles']")).toBeHidden();
  // a viewer's own choices win next time: The bots opened, Basics closed
  await page.locator(".group-btn", { hasText: "The bots" }).click();
  await page.locator(".group-btn", { hasText: "Basics" }).click();
  await page.goto("/docs.html#map-editor");
  await page.reload();
  expect((await open()).map((t) => t.trim())).toEqual(["The bots", "Create"]);
  const watch = page.locator(".group-btn", { hasText: "Watch & inspect" });
  await expect(watch).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator("#tree a[href='#demos']")).toBeHidden();
  // search still finds what's in a closed group
  await page.locator("#q").fill("demos");
  await expect(page.locator("#results [role=option]").first()).toBeVisible();
  await page.locator("#q").fill("");
  // opening a topic opens its group
  await page.goto("/docs.html#demos");
  await expect(page.locator(".group-btn", { hasText: "Watch & inspect" })).toHaveAttribute("aria-expanded", "true");
  // Something's wrong? from the sidebar's top, every link going somewhere
  await page.locator(".wrong-link").click();
  await expect(page.locator("#something-wrong")).toBeVisible();
  const hrefs = await page.locator("#something-wrong a").evaluateAll((as) => as.map((a) => a.getAttribute("href")!));
  for (const h of hrefs) expect(await page.locator(h).count(), h).toBe(1);
});

test("Something's wrong? covers an upload that doesn't show in Community", async ({ page }) => {
  await page.goto("/docs.html#wrong-sharing");
  const item = page.locator("#something-wrong li", { hasText: "My upload doesn't show in Community" });
  await expect(item).toBeVisible();
  for (const need of ["Counter-Strike 1.6", "Custom Tag with the value LonerStrike", "file check", "moderator", "daily re-check"]) await expect(item).toContainText(need);
  await item.getByRole("link", { name: "the publishing steps" }).click();
  await expect(page.locator("#publish-steps")).toBeInViewport();
  await expect(page.locator("#publishing")).toContainText("On GameBanana's Technical tab, under Tags, add a Custom Tag with the value LonerStrike (the title can stay empty), then press + and save");
  // and search finds it
  await page.locator("#q").fill("upload not showing community");
  await expect(page.locator("#results [role=option]").first()).toHaveAttribute("data-id", "wrong-sharing");
});
