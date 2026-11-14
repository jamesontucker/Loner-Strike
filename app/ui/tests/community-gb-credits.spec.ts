// Publish and GameBanana's rules (its credits and AI Generated Content Policy, wiki 2175):
// - "AI agents helped make this": off unless an agent changed the creation through the AI
//   connection this run (then ticked, with the AI's name from that connection); when ticked the
//   request names the AI and the step points at GameBanana's AI Usage section
// - after packing, the credits block for GameBanana's Authors module, one line per person
// The app's answers are routed here (no GameBanana, no game).
import { expect, test, type Page } from "@playwright/test";

const MODE = "zombie_rush";

async function stubMode(page: Page, aiUse: { touched: boolean; agent: string }) {
  const sent: { ai?: string }[] = [];
  await page.route(`**/api/tools/community/ai-use?*`, (r) => r.fulfill({ json: aiUse }));
  await page.route(`**/api/tools/community/publish-mode/${MODE}`, async (r) => {
    if (r.request().method() === "POST") {
      sent.push(r.request().postDataJSON());
      await new Promise((res) => setTimeout(res, 300));
      const ai = sent.at(-1)?.ai ?? "";
      return r.fulfill({
        json: {
          package: { file: "zombie_rush-loner-strike.zip", size: 1000, md5: "x", sha256: "y", contents: ["a"], screenshots: [], item: {}, warnings: [] },
          form: {
            submitUrl: "https://gamebanana.com/mods/add", game: "Counter-Strike 1.6", category: "Resources > AMXX Plugins & Mods", title: "Zombie Rush",
            text: `Run.${ai ? `\n\nAI disclosure: AI agents helped make this (${ai}).` : ""}`, tag: { title: "Loner-Strike", value: "mode" }, file: "zombie_rush-loner-strike.zip", md5: "x",
            authors: ["qa — Game mode", "someone — Original mode (Zombie Run)", "Zed — Creature gb77.runner"],
            ...(ai ? { ai } : {}),
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

test("AI use: off by default; the Authors block after packing", async ({ page }) => {
  const sent = await stubMode(page, { touched: false, agent: "" });
  const box = page.getByRole("checkbox", { name: "AI agents helped make this" });
  await expect(box).not.toBeChecked();
  await expect(page.getByRole("textbox", { name: "Which AI" })).toHaveCount(0);
  await page.getByRole("button", { name: "Make the package" }).click();
  await expect(page.getByText("Credits for GameBanana's Authors module (one per person)")).toBeVisible({ timeout: 10_000 });
  await expect(page.locator(".cm-copies")).toContainText("someone — Original mode (Zombie Run)");
  expect(sent[0].ai).toBe("");
  // nothing commercial in what goes on GameBanana
  await expect(page.locator(".cm-copies")).not.toContainText(/Support Pack|Polar|donat/i);
});

test("AI use: ticked when an agent changed it, named from the connection, editable", async ({ page }) => {
  const sent = await stubMode(page, { touched: true, agent: "Claude (Anthropic)" });
  await expect(page.getByRole("checkbox", { name: "AI agents helped make this" })).toBeChecked();
  const name = page.getByRole("textbox", { name: "Which AI" });
  await expect(name).toHaveValue("Claude (Anthropic)");
  await expect(page.getByRole("link", { name: "its AI policy" })).toHaveAttribute("href", "https://gamebanana.com/wikis/2175");
  await expect(page.getByText(/AI Usage section on GameBanana/)).toBeVisible();
  await name.fill("Claude (Anthropic), Codex");
  await page.getByRole("button", { name: "Make the package" }).click();
  await expect(page.locator(".cm-copies")).toContainText("AI disclosure: AI agents helped make this (Claude (Anthropic), Codex).", { timeout: 10_000 });
  expect(sent[0].ai).toBe("Claude (Anthropic), Codex");
});
