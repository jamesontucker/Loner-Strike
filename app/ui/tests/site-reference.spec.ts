// The generated reference (scripts/gen-reference.mjs; Parity A): every node and entity documents
// itself (this fails when one has no help text), the pages work at 390 wide, the docs search finds
// reference entries and opens them, and the pages say what the Studio's node help and the AI
// connection's studio_node_help say (all three read nodehelp.json).
//   npx playwright test -c tests/site.config.ts site-reference
import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
// @ts-expect-error (a plain .mjs script, no types)
import { entities, missingHelp, nodes } from "../../../scripts/gen-reference.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const HELP = JSON.parse(readFileSync(join(here, "../src/views/tools/studio/nodehelp.json"), "utf8")).help;

test("every node and entity has help text (add it when you add one)", () => {
  expect(missingHelp(), "nodes or entities without help").toEqual([]);
});

test("node pages: an index, one page per category, every node with an anchor, the node help's text, fine at 390 wide", async ({ page, request }) => {
  const all = nodes().nodes as { type: string; title: string; category: string }[];
  await page.goto("/reference/nodes.html");
  const cats = await page.locator("#categories h3 a").evaluateAll((as) => as.map((a) => a.getAttribute("href")!));
  expect(cats.length).toBe(new Set(all.map((n) => n.category)).size);
  // every node on its category's page, once
  let count = 0;
  for (const href of cats) count += ((await (await request.get(`/reference/${href}`)).text()).match(/class="node" id="n-/g) ?? []).length;
  expect(count).toBe(all.length);
  await page.goto("/reference/nodes/messages.html#n-msg-killfeed");
  const kill = page.locator("#n-msg-killfeed");
  await expect(kill.locator("h3")).toContainText("Kill Feed Line");
  await expect(kill).toContainText(HELP["msg.killfeed"].what);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/reference/nodes/messages.html#n-msg-killfeed");
  await expect(page.locator("#n-msg-killfeed")).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
});

test("the entity and editor pages", async ({ page }) => {
  const ents = entities() as { name: string }[];
  await page.goto("/reference/entities.html");
  await expect(page.locator(".node[id^='e-']")).toHaveCount(ents.length);
  await expect(page.locator("#e-ls-npc")).toContainText("Keys");
  await page.goto("/reference/editor.html");
  await expect(page.locator("h2").first()).toHaveText("File");
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
});

test("the docs search finds a node and opens its reference entry; Reference in the sidebar", async ({ page }) => {
  await page.goto("/docs.html#reference");
  await expect(page.locator("#reference")).toBeVisible();
  await expect(page.locator(".group-btn", { hasText: "Reference" })).toBeVisible();
  await page.locator("#q").fill("kill feed line");
  const first = page.locator("#results [role=option]").first();
  await expect(first).toHaveAttribute("data-id", "reference/nodes/messages.html#n-msg-killfeed");
  await page.locator("#q").press("Enter");
  await expect(page).toHaveURL(/reference\/nodes\/messages\.html#n-msg-killfeed$/);
  await expect(page.locator("#n-msg-killfeed")).toBeInViewport();
});
