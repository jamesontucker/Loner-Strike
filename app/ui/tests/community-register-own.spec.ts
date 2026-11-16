// Publish's "Already uploaded? Register a GameBanana page": always there above step 1, the same
// form as step 3 (the link, the Terms tick, Add to Community) with no package step; the app sends
// the link with no package file, and the answers show as in step 3: a refusal at the form's full
// width, its reason once (with a fix only when the reason doesn't already say what to do).
//   npx playwright test community-register-own --project=large --project=small
//   (LS_SHOT_DIR=<folder>: also screenshots of the box in each state, at the project's width)
import { expect, test } from "@playwright/test";

test("a GameBanana page registers without a package", async ({ page }, info) => {
  const sent: { url?: string; file?: string }[] = [];
  const answers = [
    // a refusal with a fix: the reason doesn't say what to do
    { error: "That page isn't in GameBanana's Counter-Strike 1.6 section." },
    // a page whose file GameBanana hasn't scanned: the reason says what to do (said once)
    { error: "GameBanana is still checking the page's file. Try again in a few minutes." },
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
  const shot = async (state: string) => {
    const dir = process.env.LS_SHOT_DIR;
    if (dir) await box.screenshot({ path: `${dir}/register-box-${state}-${page.viewportSize()?.width}.png` });
  };
  // the refusal spans the form's width, and its text wraps normally (not word by word)
  const fullWidth = async () => {
    const why = box.getByTestId("refusal-why");
    const [w, f] = [await why.boundingBox(), await box.locator("form.cm-register").boundingBox()];
    expect(w && f && w.width >= f.width * 0.95, `the reason is ${w?.width}px wide, the form ${f?.width}px`).toBeTruthy();
    const lines = Math.round((w?.height ?? 0) / parseFloat(await why.evaluate((e) => getComputedStyle(e).lineHeight)));
    expect(lines, "wraps like a sentence, not a column").toBeLessThanOrEqual(2);
  };

  await add.click();
  await expect(box.getByTestId("refusal-why")).toHaveText("Not added: That page isn't in GameBanana's Counter-Strike 1.6 section.");
  await expect(box.getByTestId("refusal-fix")).toContainText("Move the page to GameBanana's Counter-Strike 1.6 section");
  await fullWidth();
  await shot("refusal-with-fix");

  await add.click();
  await expect(box.getByTestId("refusal-why")).toHaveText("Not added: GameBanana is still checking the page's file. Try again in a few minutes.");
  await expect(box.getByTestId("refusal-fix")).toHaveCount(0);
  await expect(box.getByText(/Try again in a few minutes/)).toHaveCount(1);
  await fullWidth();
  await shot("pending-scan");

  // then it goes in
  await add.click();
  await expect(box.getByText(/^Listed\./)).toBeVisible();
  expect(sent).toEqual(Array(3).fill({ url: "https://gamebanana.com/mods/741494", file: "" }));
  void info;
});
