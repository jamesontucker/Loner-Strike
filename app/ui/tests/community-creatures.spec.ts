// Shared creatures: a creation's custom creatures travel with it. The Publish step lists what it
// includes ("Includes: runner_x (with its model)"), a Community card says "Includes N creatures",
// and a creation's page names them. The app's answers are routed here (no GameBanana, no game).
import { expect, test } from "@playwright/test";

const MODE = "zombie_rush";

test("Publish a mode: its custom creatures are listed as included", async ({ page }) => {
  await page.route(`**/api/tools/community/publish-mode/${MODE}`, (r) =>
    r.fulfill({
      json: {
        id: MODE, name: "Zombie Rush", description: "", nodes: 12, blockers: [], remixOf: null, botsSuggested: "",
        creatures: ["runner_x (with its model)", "tough_base"],
      },
    }),
  );
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`community:publish:mode:${MODE}`)}`);
  const row = page.getByText(/Includes: runner_x \(with its model\), tough_base\./);
  await expect(row).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText("Custom creatures", { exact: true })).toBeVisible();
});

test("a card says how many creatures come with it, and its page names them", async ({ page }) => {
  const entry = {
    model: "Mod", id: 901, key: "mod-901", kind: "mode", name: "Crawler Night", author: "jam", summary: "", category: "", image: null, likes: 0,
    added: 0, updated: 0, listed: true, map: null, bots: "", modes: [], have: false, badges: [], games: [], creatures: 2,
    trust: { level: "data", label: "Data only", allowed: true, confirm: null, note: "" }, canTry: true,
  };
  await page.route("**/api/tools/community/browse*", (r) => r.fulfill({ json: { items: [entry], index: "live", gamebanana: "live", indexUpdated: 0 } }));
  const item = {
    kind: "mode", model: "Mod", gb: 901, file: 1, md5: "", name: "Crawler Night", author: "jam", summary: "", games: [], map: null,
    mode: { id: "crawler", plugins: [], review: "none", maps: [] }, health: null, screenshots: [], tags: [], featured: false, added: 0, updated: 0, status: "listed",
    creatures: [{ name: "jam.crawler", model: true, sounds: 2 }, { name: "jam.base", model: false, sounds: 0 }],
  };
  await page.route("**/api/tools/community/item/mod/901", (r) =>
    r.fulfill({
      json: {
        model: "Mod", id: 901, kind: "mode", name: "Crawler Night", author: "jam", category: "", text: "", images: [], files: [], url: "https://gamebanana.com/mods/901",
        tagged: true, likes: 0, downloads: 0, item, trust: { level: "data", label: "Data only", allowed: true, confirm: null, note: "" }, canTry: true, why: null, have: false, installed: null,
      },
    }),
  );
  await page.goto("/tools-dev.html?open=community");
  const card = page.getByTestId("cm-card-creatures");
  await expect(card).toHaveText("Includes 2 creatures", { timeout: 30_000 });
  await page.getByRole("button", { name: /Crawler Night/ }).first().click();
  const inc = page.getByTestId("cm-includes-creatures");
  await expect(inc).toBeVisible({ timeout: 20_000 });
  await expect(inc.getByRole("listitem").filter({ hasText: "jam.crawler" })).toContainText("with its model and 2 sounds");
  await expect(inc.getByRole("listitem").filter({ hasText: "jam.base" })).not.toContainText("with its");
});
