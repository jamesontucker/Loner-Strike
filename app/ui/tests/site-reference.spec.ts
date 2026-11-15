// The generated reference (scripts/gen-reference.mjs; Parity A): every node and entity documents
// itself (this fails when one has no help text); the pages: a search with filters on every page
// (instant, keyboard, "/"), a sidebar of categories with counts, cards drawn like the Studio's
// nodes with details on demand, deep links that open a card, copy-link, related chips, a phone
// layout; the docs search finds reference entries; Reference in the top bar. The pages say what the
// Studio's node help and the AI connection's studio_node_help say (all three read nodehelp.json).
//   npx playwright test -c tests/site.config.ts site-reference
import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
// @ts-expect-error (a plain .mjs script, no types)
import { entities, missingHelp, nodes, typeProblems } from "../../../scripts/gen-reference.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const HELP = JSON.parse(readFileSync(join(here, "../src/views/tools/studio/nodehelp.json"), "utf8")).help;

test("every node and entity has help text (add it when you add one)", () => {
  expect(missingHelp(), "nodes or entities without help").toEqual([]);
});

test("node pages: a card per category on the index, every node once, the node help's text", async ({ page, request }) => {
  const all = nodes().nodes as { type: string; category: string }[];
  await page.goto("/reference/nodes.html");
  const cats = await page.locator(".grid a.mini[href^='nodes/']").evaluateAll((as) => as.map((a) => a.getAttribute("href")!));
  expect(cats.length).toBe(new Set(all.map((n) => n.category)).size);
  let count = 0;
  for (const href of cats) count += ((await (await request.get(`/reference/${href}`)).text()).match(/class="card" id="n-/g) ?? []).length;
  expect(count).toBe(all.length);
  // a category page: its overview grid, then the cards; the sidebar shows counts and this one's nodes
  await page.goto("/reference/nodes/messages.html");
  await expect(page.locator(".grid .mini").first()).toBeVisible();
  await expect(page.locator(".ref-side details.here summary .n")).toHaveText(/^\d+$/);
  const kill = page.locator("#n-msg-killfeed");
  await expect(kill.locator("h3")).toHaveText("Kill Feed Line");
  await expect(kill.locator(".what")).toHaveText(HELP["msg.killfeed"].what);
  // drawn like the Studio's node: pins on both sides, exec pins too
  await expect(kill.locator(".gfx .in .pin")).toHaveCount(4);
  await expect(kill.locator(".gfx .pin.exec")).toHaveCount(2);
  // details on demand: closed until asked (a deep link opens them)
  await expect(kill.locator("details").first()).not.toHaveAttribute("open", "");
  await page.goto("/reference/nodes/messages.html#n-msg-killfeed");
  await expect(page.locator("#n-msg-killfeed details").first()).toHaveAttribute("open", "");
  await expect(page.locator("#n-msg-killfeed")).toBeInViewport();
  await expect(page.locator("#n-msg-killfeed .copy-link")).toBeVisible();
  await expect(page.locator("#n-msg-killfeed .chips .chip").first()).toBeVisible();
});

test("search on a reference page: instant, filters, keyboard, '/'", async ({ page }) => {
  await page.goto("/reference/entities.html");
  await page.keyboard.press("/");
  await expect(page.locator("#ref-q")).toBeFocused();
  await page.keyboard.type("kill feed");
  const opts = page.locator("#ref-results [role=option]");
  await expect(opts.first()).toContainText("Kill Feed Line");
  await expect(opts.first()).toContainText("Studio node · Messages");
  await expect(page.locator("#ref-count")).toHaveText(/\d+ results?/);
  // filters: events that give a Player
  await page.locator("#ref-q").fill("");
  await page.locator(".ref-filter-box > summary").click();
  await page.locator("#ref-f-kind").selectOption("event");
  await page.locator("#ref-f-type").selectOption("Player");
  await expect(opts.first()).toBeVisible();
  const kinds = await opts.locator(".badge").allTextContents();
  expect(kinds.every((k) => k === "event")).toBe(true);
  await page.locator("#ref-f-kind").selectOption("");
  await page.locator("#ref-f-type").selectOption("");
  // the keyboard: ↓ moves, Enter opens it
  await page.locator("#ref-q").fill("ls_npc");
  await expect(opts.first()).toContainText("ls_npc");
  await page.locator("#ref-q").press("ArrowDown");
  await page.locator("#ref-q").press("ArrowUp");
  await page.locator("#ref-q").press("Enter");
  await expect(page).toHaveURL(/reference\/entities\/loner-strike-people\.html#e-ls-npc$/);
  await expect(page.locator("#e-ls-npc")).toBeInViewport();
});

test("phones: the sidebar is a menu, the search stays at the top, nothing sideways", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const url of ["/reference/", "/reference/nodes.html", "/reference/nodes/messages.html#n-msg-killfeed", "/reference/entities/loner-strike-people.html", "/reference/editor.html"]) {
    await page.goto(url);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), url).toBe(true);
    await expect(page.locator("#ref-q")).toBeInViewport();
  }
  await page.goto("/reference/nodes/messages.html");
  await expect(page.locator(".ref-side")).toBeHidden();
  await page.locator(".ref-menu-btn").click();
  await expect(page.locator(".ref-side")).toBeVisible();
});

test("the docs: Reference in the top bar (current in the Reference group), the docs search opens reference entries", async ({ page }) => {
  await page.goto("/docs.html#reference");
  await expect(page.locator("#nav-reference")).toHaveAttribute("aria-current", "page");
  await page.goto("/docs.html#playing");
  await expect(page.locator("#nav-reference")).not.toHaveAttribute("aria-current", "page");
  await page.locator("#q").fill("kill feed line");
  const first = page.locator("#results [role=option]").first();
  await expect(first).toHaveAttribute("data-id", "reference/nodes/messages.html#n-msg-killfeed");
  await page.locator("#q").press("Enter");
  await expect(page).toHaveURL(/reference\/nodes\/messages\.html#n-msg-killfeed$/);
  await expect(page.locator("#n-msg-killfeed")).toBeInViewport();
});

test("the entity pages", async ({ request }) => {
  const ents = entities() as { name: string }[];
  let count = 0;
  const index = await (await request.get("/reference/entities.html")).text();
  for (const m of index.matchAll(/href="(entities\/[^"]+\.html)"/g)) count += ((await (await request.get(`/reference/${m[1]}`)).text()).match(/class="card" id="e-/g) ?? []).length;
  expect(count).toBe(ents.length);
});

test("the switcher stays one line at 1024; the phone menu keeps only the current category; loops are Flow", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 800 });
  await page.goto("/reference/nodes/arrays.html");
  const tops = await page.$$eval(".ref-switch a", (as) => new Set(as.map((a) => Math.round(a.getBoundingClientRect().top))).size);
  expect(tops).toBe(1);
  const card = page.locator("#n-flow-for-each-array");
  await expect(card.locator(".badge")).toHaveText(/flow/i);
  await expect(card.locator("summary", { hasText: "Inputs" })).toContainText("(3)");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.click(".ref-menu-btn");
  const side = page.locator(".ref-side");
  await expect(side.locator("details.here > summary")).toContainText("Arrays");
  await expect(side.locator("details:not(.here)").first()).toBeHidden();
  await side.locator(".ref-all").click();
  await expect(side.locator("details:not(.here)").first()).toBeVisible();
  // (the landing's group header: the marker beside the label, like every other)
  await page.goto("/reference/");
  await page.setViewportSize({ width: 1440, height: 900 });
  const gap = await page.$eval(".ref-side details.here > summary", (s) => s.querySelector("a")!.getBoundingClientRect().left - s.getBoundingClientRect().left);
  expect(gap).toBeLessThan(40);
});

test("a search typed while the index is still loading runs as soon as it arrives", async ({ page }) => {
  // (the index answers late, so the typing surely lands while it loads)
  await page.route("**/reference/index.js*", async (route) => {
    await new Promise((r) => setTimeout(r, 2500));
    await route.continue();
  });
  // (not waiting for the load event: that waits for the index too)
  await page.goto("/reference/nodes/messages.html", { waitUntil: "domcontentloaded" });
  await page.locator("#ref-q").click();
  await page.locator("#ref-q").fill("double jump");
  await expect(page.locator("#ref-count")).toHaveText(/Loading the index/);
  await expect(page.locator("#ref-results li").first()).toBeVisible({ timeout: 8000 });
  await expect(page.locator("#ref-count")).not.toHaveText(/Loading/);
  // (and the latest text wins: typed again while loading, only the last query runs)
  await page.goto("/reference/entities.html", { waitUntil: "domcontentloaded" });
  await page.locator("#ref-q").fill("door");
  await page.locator("#ref-q").fill("ladder");
  await expect(page.locator("#ref-results li").first()).toContainText(/ladder/i, { timeout: 8000 });
});

test("every pin type has a look and a Types entry (add one when you add a type)", () => {
  expect(typeProblems(), "pin types without a color, a styled mark or a Types entry").toEqual([]);
});

test("the Types page: a card per type with its pin, upcasts and casts; pins link to it; enum values; # marks; search", async ({ page }) => {
  await page.goto("/reference/nodes/types.html");
  const data = nodes();
  for (const k of Object.keys(data.pinTypes)) await expect(page.locator(`#t-${k.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`), k).toHaveCount(1);
  const npc = page.locator("#t-npc");
  await expect(npc.locator(".pbadge")).toHaveText("N");
  await expect(npc).toContainText("Goes into");
  await expect(npc.locator("a", { hasText: "Entity" }).first()).toHaveAttribute("href", "#t-entity");
  await expect(npc.locator(".chip", { hasText: "As NPC" }).first()).toBeVisible();
  await expect(page.locator("#t-entity")).toContainText("Creature");
  await expect(page.locator("#t-enum-ls-hitzone")).toContainText("left leg");
  await expect(page.locator("#t-enum-ls-weapon")).toContainText("Text");
  await expect(page.locator("#t-color .pin.rainbow")).toHaveCount(1);
  await expect(page.locator("#t-model .pin.file")).toHaveCount(1);
  await expect(page.locator(".ref-main a", { hasText: "Types and casting guide" })).toHaveCount(1);
  // a node card: its pins' types link to the page, an enum pin lists its values
  await page.goto("/reference/nodes/events.html");
  const hurt = page.locator("#n-event-player-hurt");
  await hurt.locator("summary", { hasText: "Outputs" }).click();
  await expect(hurt.locator("dd", { hasText: "Values: generic, head" })).toBeVisible();
  await expect(hurt.locator("a.ty", { hasText: "Hit Zone" })).toHaveAttribute("href", "types.html#t-enum-ls-hitzone");
  await page.goto("/reference/nodes/enums.html");
  await expect(page.locator("#n-flow-switch-enum p.note")).toContainText("one output per value");
  expect(await page.request.get("/reference/nodes/arrays.html").then((r) => r.text())).toContain('class="pwhole"');
  // search: types are in the index
  await page.locator("#ref-q").fill("hit zone");
  await expect(page.locator("#ref-results li", { hasText: "Pin type" }).first()).toBeVisible();
});

test("a sidebar group's triangle opens its items (mouse and keyboard); rows with nothing to open have none", async ({ page }) => {
  for (const [path, group] of [["/reference/nodes/players.html", "Events"], ["/reference/nodes/types.html", "Flow"], ["/reference/entities/loner-strike-creatures.html", "Point entities"], ["/reference/nodes.html", "Pin types"]] as const) {
    await page.goto(path);
    const d = page.locator(".ref-side details", { has: page.locator("summary a", { hasText: new RegExp(`^${group}$`) }) });
    const sum = d.locator("summary");
    await expect(sum, `${path} ${group}`).toHaveAttribute("aria-expanded", "false");
    const box = (await sum.boundingBox())!;
    await page.mouse.click(box.x + 16, box.y + box.height / 2);
    await expect(d.locator("ul li a").first(), `${path} ${group}`).toBeVisible();
    await expect(sum).toHaveAttribute("aria-expanded", "true");
    await sum.focus();
    await page.keyboard.press("Enter");
    await expect(d.locator("ul li a").first()).toBeHidden();
    await page.keyboard.press(" ");
    await expect(d.locator("ul li a").first()).toBeVisible();
  }
  // (the current group's own name opens and closes it, without leaving the page)
  await page.goto("/reference/nodes/players.html");
  const here = page.locator(".ref-side details.here");
  await here.locator("summary a").click();
  await expect(here.locator("ul li a").first()).toBeHidden();
  await expect(page).toHaveURL(/players\.html$/);
  // phones: All categories, then a triangle
  await page.setViewportSize({ width: 390, height: 844 });
  await page.click(".ref-menu-btn");
  await page.click(".ref-all");
  const ev = page.locator(".ref-side details", { has: page.locator("summary a", { hasText: /^Events$/ }) });
  const b2 = (await ev.locator("summary").boundingBox())!;
  await page.mouse.click(b2.x + 16, b2.y + b2.height / 2);
  await expect(ev.locator("ul li a").first()).toBeVisible();
  // the editor's rows have no items: no triangle
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/reference/editor.html");
  await expect(page.locator(".ref-side details.leaf").first()).toBeVisible();
  expect(await page.locator(".ref-side details.leaf > summary").first().evaluate((s) => getComputedStyle(s, "::before").visibility)).toBe("hidden");
  // the docs sidebar: a group's button opens its topics
  await page.goto("/docs.html");
  const g = page.locator('.group[data-group="Create"]');
  await g.locator(".chev").click();
  await expect(g.locator("ul li a").first()).toBeVisible();
  await expect(g.locator(".group-btn")).toHaveAttribute("aria-expanded", "true");
});

test("search: nothing found says so, without an empty box", async ({ page }) => {
  await page.goto("/reference/nodes/effects.html");
  await page.locator("#ref-q").fill("zzzzqq");
  await expect(page.locator("#ref-count")).toHaveText(/No results\. Try fewer/);
  await expect(page.locator("#ref-results-box")).toBeHidden();
  await expect(page.locator("#ref-q")).toHaveAttribute("aria-expanded", "false");
});
