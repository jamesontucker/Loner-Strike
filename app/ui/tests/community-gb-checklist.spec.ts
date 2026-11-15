// Publish's GameBanana step (Parity A): the numbered checklist that gets an upload into Community,
// the one-line "Needs" hint under the link box, and a refusal said in plain words with what to fix.
//   npx playwright test community-gb-checklist --project=large [--project=webkit with LS_WEBKIT_ALL=1]
import { expect, test, type Route } from "@playwright/test";

const H = { "X-Panel": "1" };

test("the GameBanana step: checklist, the Needs hint, and refusals with what to fix", async ({ page }, info) => {
  const id = `pw_gb_check_${info.project.name}`;
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/king-of-the-hill.graph.json")).json();
  tpl.mode.id = id;
  await page.request.post(`/api/tools/studio/graph/${id}`, { data: { graph: tpl }, headers: H });
  // the package and GameBanana's form, as Publish makes them (made here: no zip needed)
  await page.route(/\/api\/tools\/community\/publish-mode\//, (r: Route) => {
    if (r.request().method() !== "POST") return r.fallback();
    return r.fulfill({
      json: {
        package: { file: "king_of_the_hill.zip", size: 12345, md5: "x", sha256: "y", contents: ["loner-strike.json"], screenshots: [], item: {}, warnings: [] },
        form: { submitUrl: "https://gamebanana.com/mods/add", game: "Counter-Strike 1.6", category: "Game Modes", title: "King of the Hill", text: "Hold the hill.", tag: { title: "Software Used", value: "LonerStrike" }, file: "king_of_the_hill.zip", md5: "x", authors: ["Loner-Strike — Creator"] },
      },
    });
  });
  const answers = [
    { error: 'Add the tag "Software Used: LonerStrike" to the page (Edit > Attributes), then try again.' },
    { ok: true, state: "refused", error: "GameBanana is still checking the file. Try again in a few minutes.", copy: "" },
    { ok: true, state: "refused", error: "That page isn't in GameBanana's Counter-Strike 1.6 section.", copy: "" },
  ];
  await page.route(/\/api\/tools\/community\/register/, (r) => r.fulfill({ json: answers.shift() }));
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`community:publish:mode:${id}`)}`);
  const make = page.getByRole("button", { name: "Make the package" });
  await make.waitFor({ timeout: 20_000 });
  const madeBy = page.getByRole("textbox", { name: "Made by" });
  if (await madeBy.count()) await madeBy.fill("Loner-Strike");
  await make.click();

  // the checklist: four short steps, the attribute named exactly
  const list = page.getByRole("list", { name: "Getting it into Community" });
  await expect(list.getByRole("listitem")).toHaveCount(4);
  await expect(list.getByRole("listitem").nth(0)).toContainText("Upload king_of_the_hill.zip unchanged in the Counter-Strike 1.6 section");
  await expect(list.getByRole("listitem").nth(1)).toContainText("set the attribute Software Used to LonerStrike");
  await expect(list.getByRole("listitem").nth(2)).toContainText("AI Usage");
  await expect(list.getByRole("listitem").nth(3)).toContainText("Add to Community");
  // the value to copy is there with its Copy button
  await expect(page.locator(".cm-copy", { hasText: "Attribute: Software Used" }).getByRole("button", { name: "Copy" })).toBeVisible();

  // the link box says what the page needs
  const link = page.getByRole("textbox", { name: "GameBanana link" });
  await expect(link).toHaveAccessibleDescription("Needs: Counter-Strike 1.6 section, Software Used: LonerStrike, and GameBanana's file check done.");

  // refusals: the reason, then what to do
  await link.fill("https://gamebanana.com/mods/123456");
  await page.getByRole("checkbox", { name: /I have the right to share this/ }).check();
  const add = page.getByRole("button", { name: "Add to Community" });
  await add.click();
  await expect(page.getByTestId("refusal-fix")).toContainText("set Software Used to LonerStrike");
  await add.click();
  await expect(page.getByTestId("refusal-fix")).toContainText("Try again in a few minutes.");
  await add.click();
  await expect(page.getByTestId("refusal-fix")).toContainText("Counter-Strike 1.6 section");
  await page.request.post(`/api/tools/studio/graph/${id}/delete`, { headers: H });
});
