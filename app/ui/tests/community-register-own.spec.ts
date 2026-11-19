// Publish's "Already uploaded? Add a GameBanana page to Community": always there above step 1, the same
// form as step 3 (the link, the Terms tick, Add to Community) with no package step; the app sends
// the link with no package file. Every answer shows as one full-width line, its mark (✓ ! ✕) on
// the same line as the text, the reason said once (a fix only when the reason doesn't already say
// what to do).
//   npx playwright test community-register-own --project=large --project=small
//   (LS_SHOT_DIR=<folder>: also a screenshot of the box in each state, at the project's width)
import { expect, test, type Locator, type Page } from "@playwright/test";

// every answer the register form can show, as the app's /register answers it
const STATES: { name: string; answer: object; line: string; mark: "✓" | "!" | "✕"; fix?: string }[] = [
  { name: "listed", answer: { ok: true, state: "listed", error: null, copy: "{}", item: {} }, line: "Listed. It shows in Community within a few minutes.", mark: "✓" },
  { name: "sent-to-moderators", answer: { ok: true, state: "pending", error: null, copy: "{}", item: {} }, line: "Sent. A Loner-Strike moderator looks at it first; it shows under Yours once it's listed.", mark: "✓" },
  { name: "refusal-with-fix", answer: { error: "That page isn't in GameBanana's Counter-Strike 1.6 section." }, line: "Not added: That page isn't in GameBanana's Counter-Strike 1.6 section.", mark: "✕", fix: "Move the page to GameBanana's Counter-Strike 1.6 section" },
  { name: "pending-scan", answer: { error: "GameBanana is still checking the page's file. Try again in a few minutes." }, line: "Not added: GameBanana is still checking the page's file. Try again in a few minutes.", mark: "✕" },
  { name: "list-refused", answer: { ok: true, state: "refused", error: "maps/br_valley.lsv didn't pass the checks: it says 12000 areas, more than the bot brain reads (10000).", copy: "" }, line: "Not added: maps/br_valley.lsv didn't pass the checks: it says 12000 areas, more than the bot brain reads (10000).", mark: "✕" },
  { name: "list-didnt-answer", answer: { ok: true, state: "waiting", error: null, copy: '{"schema": 1, "item": {"gb": 741494}}', item: {} }, line: "The page checks out, but the Loner-Strike list didn't answer. It's kept here and sent automatically later; you can also post this in the Loner-Strike Discord:", mark: "!" },
  { name: "plugin-mode", answer: { error: "Shared creations can't carry plugins.", studio: true }, line: "Plugin modes can't be shared yet. A mode made in Game Mode Studio can: the app builds its code on each player's computer.", mark: "!" },
];

/** The line spans the form, its mark sits on its first line (left of the text), and it wraps
 *  like a sentence (one or two lines at these widths, never a narrow column). */
async function fullWidthLine(page: Page, box: Locator, line: Locator, mark: string) {
  const [l, f] = [await line.boundingBox(), await box.locator("form.cm-register").boundingBox()];
  expect(l && f && l.width >= f.width * 0.95, `the line is ${l?.width}px wide, the form ${f?.width}px`).toBeTruthy();
  const lh = parseFloat(await line.evaluate((e) => getComputedStyle(e).lineHeight));
  expect(Math.round((l?.height ?? 0) / lh), "wraps like a sentence, not a column").toBeLessThanOrEqual(2);
  const before = await line.evaluate((e) => {
    const s = getComputedStyle(e, "::before");
    return { content: s.content, top: parseFloat(s.top) || 0, position: s.position };
  });
  expect(before.content).toBe(`"${mark}"`);
  expect(before.position, "the mark beside the text, on its first line").toBe("absolute");
  void page;
}

test("a GameBanana page registers without a package, and every answer reads as one full-width line", async ({ page }) => {
  const sent: { url?: string; file?: string }[] = [];
  const answers = STATES.map((s) => s.answer);
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
  await box.getByText("Already uploaded? Add a GameBanana page to Community").click();
  await box.getByRole("textbox", { name: "GameBanana link" }).fill("https://gamebanana.com/mods/741494");
  const add = box.getByRole("button", { name: "Add to Community" });
  await expect(add).toBeDisabled();
  await box.getByRole("checkbox", { name: /I have the right to share this/ }).check();

  for (const s of STATES) {
    await add.click();
    const line = s.mark === "✕" ? box.getByTestId("refusal-why") : box.getByTestId("register-result");
    await expect(line, s.name).toHaveText(s.line);
    if (s.fix) await expect(box.getByTestId("refusal-fix"), s.name).toContainText(s.fix);
    else await expect(box.getByTestId("refusal-fix"), `${s.name}: said once`).toHaveCount(0);
    await fullWidthLine(page, box, line, s.mark);
    if (s.name === "list-didnt-answer") {
      // its copy row spans the result too
      const row = box.locator(".cm-result .cm-copy");
      const [r, f] = [await row.boundingBox(), await box.locator("form.cm-register").boundingBox()];
      expect(r && f && r.width >= f.width * 0.95, `the copy row is ${r?.width}px wide`).toBeTruthy();
    }
    const dir = process.env.LS_SHOT_DIR;
    if (dir) await box.screenshot({ path: `${dir}/register-box-${s.name}-${page.viewportSize()?.width}.png` });
  }
  await expect(box.getByText(/Try again in a few minutes/)).toHaveCount(0); // (the last state replaced it)
  expect(sent).toEqual(Array(STATES.length).fill({ url: "https://gamebanana.com/mods/741494", file: "" }));
});
