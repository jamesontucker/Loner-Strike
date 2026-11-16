// Browse: a creation in the Loner-Strike list (its package passed the automatic checks) shows
// "✓ Checked" on its card and its page; one that's only tagged on GameBanana doesn't. "Checked
// only" hides the tag-only ones (the app already lists checked ones first in every sort). A
// tag-only game mode can't be tried, and its page says why; a tag-only map still can.
//   npx playwright test community-checked --project=large --project=small
//   (LS_SHOT_DIR=<folder>: also screenshots of the grid and the pages, at the project's width)
import { expect, test } from "@playwright/test";

const HELP = "Its package passed Loner-Strike's automatic checks.";
const MODE_NOTE = "This mode isn't in the Loner-Strike list yet, so it can't be tried. Its creator can add it with Add to Community.";

const content = { level: "content", label: "Content", allowed: true, confirm: null, note: "Only map content is installed (maps, textures, models, sounds); nothing that runs code." };
const data = { level: "data", label: "Settings only", allowed: true, confirm: null, note: "Settings and rules only: no plugins, nothing that runs code." };
const unlisted = { level: "unreviewed", label: "Not checked", allowed: false, confirm: null, note: MODE_NOTE };

const entry = (id: number, kind: string, name: string, listed: boolean, trust: object, extra: object = {}) => ({
  model: "Mod", id, key: `mod-${id}`, kind, name, author: "nanoshrine", summary: "", category: "", image: null, likes: 3,
  added: 1_700_000_000, updated: 1_700_000_000, listed, map: null, bots: "", modes: [], have: false, badges: [], games: [],
  trust, canTry: (trust as { allowed: boolean }).allowed, ...extra,
});
// (the app's order: checked ones first within the chosen sort)
const ITEMS = [
  entry(741494, "map", "br_valley - Battle Royale", true, content, { badges: ["nav"], modes: ["Battle Royale"] }),
  entry(741497, "map", "ls_outpost - Last Stand", true, content),
  entry(800002, "mode", "Gun Juggle", true, data, { badges: ["data"], bots: "great" }),
  entry(800003, "map", "de_tagonly", false, content),
  entry(800004, "mode", "Tag-only Mode", false, unlisted),
];

const page_ = (id: number, kind: string, name: string, item: object | null, trust: object, canTry: boolean) => ({
  model: "Mod", id, kind, name, author: "nanoshrine", category: "", text: "A test page.", images: [], files: [], url: `https://gamebanana.com/mods/${id}`,
  tagged: true, likes: 3, downloads: 10, item, trust, canTry, why: null, have: false, installed: null,
});
const listedItem = {
  kind: "map", model: "Mod", gb: 741497, file: 1, md5: "", name: "ls_outpost - Last Stand", author: "nanoshrine", summary: "A walled outpost for Last Stand.",
  games: [], map: null, mode: null, health: null, screenshots: [], tags: [], featured: false, added: 0, updated: 0, status: "listed",
};
const PAGES: Record<number, object> = {
  741497: page_(741497, "map", "ls_outpost - Last Stand", listedItem, content, true),
  800003: page_(800003, "map", "de_tagonly", null, content, true),
  800004: page_(800004, "mode", "Tag-only Mode", null, unlisted, false),
};

test("Checked badge on listed creations, Checked only, and a tag-only mode can't be tried", async ({ page }) => {
  const dir = process.env.LS_SHOT_DIR;
  const w = page.viewportSize()?.width;
  await page.route("**/api/tools/community/browse*", (r) => r.fulfill({ json: { items: ITEMS, index: "live", gamebanana: "live", indexUpdated: 0 } }));
  await page.route(/\/api\/tools\/community\/item\/mod\/(\d+)$/, (r) => r.fulfill({ json: PAGES[Number(r.request().url().split("/").pop())] }));
  await page.goto("/tools-dev.html?open=community");
  const grid = page.getByRole("list", { name: "Creations" });
  await expect(grid.getByRole("listitem")).toHaveCount(5, { timeout: 30_000 });

  // the badge on the three listed ones only, with its tooltip, in the app's order
  const card = (name: string) => grid.getByRole("listitem").filter({ hasText: name });
  for (const n of ["br_valley", "ls_outpost", "Gun Juggle"]) {
    await expect(card(n).getByTestId("cm-checked")).toHaveText("✓ Checked");
    await expect(card(n).getByTestId("cm-checked")).toHaveAttribute("title", HELP);
  }
  for (const n of ["de_tagonly", "Tag-only Mode"]) await expect(card(n).getByTestId("cm-checked")).toHaveCount(0);
  await expect(grid.getByRole("listitem").first()).toContainText("br_valley");
  if (dir) await page.screenshot({ path: `${dir}/browse-all-${w}.png`, fullPage: true });

  // Checked only, next to Kind and Sort
  const only = page.getByRole("checkbox", { name: "Checked only" });
  await expect(only).not.toBeChecked();
  await only.check();
  await expect(grid.getByRole("listitem")).toHaveCount(3);
  await expect(grid).not.toContainText("de_tagonly");
  if (dir) await page.screenshot({ path: `${dir}/browse-checked-only-${w}.png`, fullPage: true });
  await only.uncheck();
  await expect(grid.getByRole("listitem")).toHaveCount(5);

  // a listed creation's page: the badge too
  await page.goto("/tools-dev.html?open=community");
  await card("ls_outpost").getByRole("button").click();
  await expect(page.getByRole("heading", { name: "ls_outpost - Last Stand" })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("cm-checked")).toHaveText("✓ Checked");
  await expect(page.getByRole("button", { name: "Try it" })).toBeEnabled();
  if (dir) await page.screenshot({ path: `${dir}/detail-checked-${w}.png`, fullPage: true });

  // a tag-only map: no badge, still tryable
  await page.getByRole("button", { name: "Back to the list" }).click();
  await card("de_tagonly").getByRole("button").click();
  await expect(page.getByRole("heading", { name: "de_tagonly" })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("cm-checked")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Try it" })).toBeEnabled();

  // a tag-only mode: no badge, can't be tried, and says why
  await page.getByRole("button", { name: "Back to the list" }).click();
  await card("Tag-only Mode").getByRole("button").click();
  await expect(page.getByRole("heading", { name: "Tag-only Mode" })).toBeVisible({ timeout: 20_000 });
  await expect(page.getByTestId("cm-checked")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Try it" })).toBeDisabled();
  await expect(page.getByText(MODE_NOTE)).toBeVisible();
  if (dir) await page.screenshot({ path: `${dir}/detail-tag-only-mode-${w}.png`, fullPage: true });
});
