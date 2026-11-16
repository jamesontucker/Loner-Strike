// Publish's "Already uploaded? Register a GameBanana page": always there above step 1, the same
// form as step 3 (the link, the Terms tick, Add to Community) with no package step; the app sends
// the link with no package file, and the answers show as in step 3.
//   npx playwright test community-register-own --project=large
import { expect, test } from "@playwright/test";

test("a GameBanana page registers without a package", async ({ page }) => {
  const sent: { url?: string; file?: string }[] = [];
  const answers = [
    { error: "On GameBanana's Technical tab, under Tags, add a Custom Tag with the value LonerStrike (the title can stay empty), then press + and save. Then try again." },
    { ok: true, state: "listed", error: null, copy: "{}", item: {} },
  ];
  await page.route(/\/api\/tools\/community\/register/, (r) => {
    sent.push(r.request().postDataJSON());
    return r.fulfill({ json: answers.shift() });
  });
  // (a first visit opens on Browse: the Publish tab)
  await page.goto(`/tools-dev.html?open=community`);
  await page.getByRole("group", { name: "Community" }).getByRole("button", { name: "Publish", exact: true }).click();
  const box = page.getByTestId("register-own");
  await box.waitFor({ timeout: 20_000 });
  // above step 1, closed until asked
  await expect(page.getByRole("heading", { name: "What are you publishing?" })).toBeVisible();
  await box.getByText("Already uploaded? Register a GameBanana page").click();
  const link = box.getByRole("textbox", { name: "GameBanana link" });
  await link.fill("https://gamebanana.com/mods/741494");
  const add = box.getByRole("button", { name: "Add to Community" });
  await expect(add).toBeDisabled();
  await box.getByRole("checkbox", { name: /I have the right to share this/ }).check();
  // a refusal: the reason and what to fix, as in step 3
  await add.click();
  await expect(box.getByTestId("refusal-fix")).toContainText("add a Custom Tag with the value LonerStrike");
  // then it goes in
  await add.click();
  await expect(box.getByText(/^Listed\./)).toBeVisible();
  expect(sent).toEqual([
    { url: "https://gamebanana.com/mods/741494", file: "" },
    { url: "https://gamebanana.com/mods/741494", file: "" },
  ]);
});
