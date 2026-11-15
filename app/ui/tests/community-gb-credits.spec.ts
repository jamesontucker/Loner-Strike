// Publish and GameBanana's rules: after packing, the credits block for GameBanana's Authors module,
// one line per person, and nothing commercial. There's no AI question in Publish (people fill in
// GameBanana's own AI Usage section themselves), so the request carries no AI name.
// The app's answers are routed here (no GameBanana, no game).
import { expect, test, type Page } from "@playwright/test";

const MODE = "zombie_rush";

async function stubMode(page: Page) {
  const sent: Record<string, unknown>[] = [];
  await page.route(`**/api/tools/community/publish-mode/${MODE}`, async (r) => {
    if (r.request().method() === "POST") {
      sent.push(r.request().postDataJSON());
      await new Promise((res) => setTimeout(res, 300));
      return r.fulfill({
        json: {
          package: { file: "zombie_rush-loner-strike.zip", size: 1000, md5: "x", sha256: "y", contents: ["a"], screenshots: [], item: {}, warnings: [] },
          form: {
            submitUrl: "https://gamebanana.com/mods/add", game: "Counter-Strike 1.6", category: "Resources > AMXX Plugins & Mods", title: "Zombie Rush",
            text: "Run.", tag: { title: "Loner-Strike", value: "mode" }, file: "zombie_rush-loner-strike.zip", md5: "x",
            authors: ["qa — Game mode", "someone — Original mode (Zombie Run)", "Zed — Creature gb77.runner"],
          },
        },
      });
    }
    return r.fulfill({ json: { id: MODE, name: "Zombie Rush", description: "", nodes: 12, blockers: [], remixOf: null, botsSuggested: "", creatures: [], member: 0 } });
  });
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`community:publish:mode:${MODE}`)}`);
  await expect(page.getByRole("textbox", { name: "Made by" })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("textbox", { name: "Made by" }).fill("qa");
  await page.getByRole("textbox", { name: "Name" }).fill("Zombie Rush");
  return sent;
}

test("Publish: no AI question; the Authors block after packing, with its copy button", async ({ page }) => {
  const sent = await stubMode(page);
  await expect(page.getByRole("checkbox", { name: /AI agents helped/ })).toHaveCount(0);
  await expect(page.getByRole("textbox", { name: "Which AI" })).toHaveCount(0);
  await page.getByRole("button", { name: "Make the package" }).click();
  await expect(page.getByText("Credits for GameBanana's Authors module (one per person)")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".cm-copies")).toContainText("someone — Original mode (Zombie Run)");
  await expect(page.locator(".cm-copies").getByRole("button", { name: /Copy/ }).first()).toBeVisible();
  expect(sent[0]).not.toHaveProperty("ai");
  // nothing commercial, and no AI line, in what goes on GameBanana
  await expect(page.locator(".cm-copies")).not.toContainText(/Support Pack|Polar|donat|AI disclosure/i);
});
