// Community: a Studio mode's page says what it needs beyond the game (QA4-G F6): Loner-Strike's
// components, its creatures and the bot-brain features, each "You have it" or "Update Loner-Strike"
// against this computer's brain. The page's answer is routed here (no GameBanana needed).
import { expect, test } from "@playwright/test";

test("a mode's page lists its needs with You have it / Update Loner-Strike", async ({ page }) => {
  const item = {
    kind: "mode", model: "Mod", gb: 555, file: 1, md5: "", name: "Needy", author: "me", summary: "", games: [], map: null,
    mode: { id: "needy", plugins: [], review: "none", maps: [] }, health: null, screenshots: [], tags: [], featured: false, added: 0, updated: 0, status: "listed",
  };
  await page.route("**/api/tools/community/item/mod/555", (r) =>
    r.fulfill({
      json: {
        model: "Mod", id: 555, kind: "mode", name: "Needy", author: "me", category: "", text: "A test mode.", images: [], files: [], url: "https://gamebanana.com/mods/555",
        tagged: true, likes: 0, downloads: 0, item, trust: { level: "data", label: "Data only", allowed: true, confirm: null, note: "" }, canTry: true, why: null, have: false, installed: null,
        needs: {
          defaultMap: null, maps: [], content: [],
          brain: {
            componentsPlugin: true, components: ["ls_wave_spawner"], creatures: ["zombie"], creaturesHave: true, modeVer: 2, creatureVer: 1,
            features: [
              { id: "creatures", name: "Creatures (zombies, the wave spawner's types)", brain: "creature", ver: 1, have: true },
              { id: "influence", name: "Personality influence on bot orders", brain: "mode", ver: 2, have: false },
            ],
            here: { modeVer: 1, creatureVer: 1 },
          },
        },
      },
    }),
  );
  await page.goto("/tools-dev.html?open=community:mod:555");
  const list = page.getByTestId("cm-brain-needs");
  await expect(list).toBeVisible({ timeout: 20_000 });
  await expect(list.getByRole("listitem").filter({ hasText: "Loner-Strike's components: ls_wave_spawner" })).toContainText("You have it");
  await expect(list.getByRole("listitem").filter({ hasText: "Creatures: zombie" })).toContainText("You have it");
  await expect(list.getByRole("listitem").filter({ hasText: "Personality influence" })).toContainText("bots version 2 or newer");
  await expect(list.getByRole("listitem").filter({ hasText: "Personality influence" })).toContainText("Update Loner-Strike");
});
