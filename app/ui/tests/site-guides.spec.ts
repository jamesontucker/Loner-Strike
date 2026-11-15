// The hand-written guides (site/reference/guides/*.html, framed by scripts/gen-reference.mjs, which
// keeps the text between their GUIDE markers): every node a guide names exists and its link opens
// that node's card; the reference's front page and the docs list them; the search finds them; the
// pages read well on phones.
//   npx playwright test -c tests/site.config.ts site-guides
import { expect, test } from "@playwright/test";
// @ts-expect-error (a plain .mjs script, no types)
import { guideProblems } from "../../../scripts/gen-reference.mjs";

const GUIDES = ["events-flow", "variables", "spawning", "limits", "bots", "sharing"];

test("every node a guide names is in the catalog", () => {
  expect(guideProblems(), "guide links to nodes that don't exist").toEqual([]);
});

test("each guide: its text between the markers, node links that open the node's card, On this page", async ({ request }) => {
  const pages = new Map<string, string>();
  for (const g of GUIDES) {
    const res = await request.get(`/reference/guides/${g}.html`);
    expect(res.ok(), g).toBe(true);
    const html = await res.text();
    expect(html).toContain("<!-- GUIDE:BEGIN");
    expect(html).toContain("<!-- GUIDE:END -->");
    expect(html).toContain('class="ref-toc"');
    expect(html).toMatch(/<h2 id="see-also"/);
    const links = [...html.matchAll(/<a class="node" data-node="([^"]+)" href="\.\.\/(nodes\/[a-z0-9-]+\.html)#(n-[a-z0-9-]+)">/g)];
    expect(links.length, `${g} names nodes`).toBeGreaterThan(3);
    expect((html.match(/class="node"/g) ?? []).length, `${g}: every node link has its href`).toBe(links.length);
    for (const [, , file, id] of links) {
      if (!pages.has(file)) pages.set(file, await (await request.get(`/reference/${file}`)).text());
      expect(pages.get(file), `${g} → ${file}#${id}`).toContain(`id="${id}"`);
    }
  }
});

test("the front page and the docs list the written guides; Types and casting is still a stub", async ({ page }) => {
  await page.goto("/reference/");
  for (const g of GUIDES) await expect(page.locator(`#guides ~ .grid a[href="guides/${g}.html"]`)).toBeVisible();
  await expect(page.locator('#guides ~ .grid a[href="../docs.html#guide-types"]')).toContainText(/soon/i);
  await page.goto("/docs.html#guide-spawning");
  await expect(page.locator("#guide-spawning a[href='reference/guides/spawning.html']")).toBeVisible();
  await expect(page.locator("#guide-types")).toHaveAttribute("data-stub", "1");
});

test("the search finds guides and their sections", async ({ page }) => {
  await page.goto("/reference/guides/limits.html");
  // (the index loads once the page is idle; typing while it loads is the reference search's own case)
  await page.waitForFunction(() => "LS_REFERENCE" in window);
  await page.locator("#ref-q").fill("double jump");
  const opts = page.locator("#ref-results [role=option]");
  await expect(page.locator("#ref-results [data-u='reference/guides/events-flow.html#examples']")).toBeVisible();
  await page.locator("#ref-q").fill("bots won't know");
  await expect(opts.first()).toHaveAttribute("data-u", "reference/guides/bots.html#warning");
  await page.locator("#ref-q").press("Enter");
  await expect(page).toHaveURL(/guides\/bots\.html#warning$/);
  await expect(page.locator("#warning")).toBeInViewport();
});

test("phones: nothing sideways, the menu lists the guides", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const g of GUIDES) {
    await page.goto(`/reference/guides/${g}.html`);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), g).toBe(true);
  }
  await expect(page.locator(".ref-side")).toBeHidden();
  await page.locator(".ref-menu-btn").click();
  await expect(page.locator(".ref-side a[aria-current='page']")).toHaveText("Sharing");
});
