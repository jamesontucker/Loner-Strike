// Publish a map that may be someone else's: a downloaded map (or one whose own text names a
// maker) asks "Did you make this map?", and No needs the maker's permission ticked before the
// package can be made, with the maker named in Credits. The Files and textures check goes by the
// warning's kind, so the bot nav warning (which also names the map doctor) doesn't show there.
//   npx playwright test community-made-it --project=large --project=small
//   (LS_SHOT_DIR=<folder>: also a screenshot of the checks and credits, at the project's width)
import { expect, test, type Page } from "@playwright/test";

const MAP = process.env.LS_TOOLS_MAP || "de_dust2";

async function open(page: Page, origin: object, extra: object = {}) {
  await page.route(`**/api/tools/community/publish/${MAP}`, async (r) => {
    if (r.request().method() === "POST") return r.fulfill({ json: { error: "stop here" } });
    const res = await r.fetch();
    const j = await res.json();
    j.ready.blockers = [];
    j.ready.map.nav = false;
    j.ready.warnings = ["Bots don't know this map yet (no nav that fits it). Teach them in the map doctor so it gets the \"Works with bots\" badge."];
    j.ready.fileWarnings = [];
    j.ready.assets = [];
    j.projects = [];
    j.origin = origin;
    Object.assign(j.ready, extra);
    await r.fulfill({ json: j });
  });
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`community:publish:${MAP}`)}`);
  await expect(page.locator(".cm-checks")).toBeVisible({ timeout: 30_000 });
}

const check = (page: Page, title: string) => page.locator(".cm-check", { hasText: title });

test("a downloaded map asks who made it; No needs permission and names the maker in Credits", async ({ page }) => {
  await open(page, { kind: "download", author: "Dewolt", named: "Dewolt" });
  // the bot nav warning isn't a files problem
  await expect(check(page, "Files and textures")).toHaveClass(/\bok\b/);
  const ask = page.getByTestId("cm-made-it");
  await expect(ask).toContainText("Did you make this map?");
  await expect(ask).toContainText("downloaded from GameBanana, where it's by Dewolt");
  await page.getByRole("textbox", { name: "Made by" }).fill("qa");
  const make = page.getByRole("button", { name: "Make the package" });
  await expect(make).toBeDisabled();
  await ask.getByRole("button", { name: "No, someone else did" }).click();
  await expect(check(page, "Credits and permissions")).toContainText("The map is by Dewolt");
  await expect(page.getByTestId("cm-map-maker")).toContainText("The map itself is by Dewolt");
  await expect(make).toBeDisabled();
  await ask.getByRole("checkbox", { name: "I have Dewolt's permission to share it" }).check();
  await expect(make).toBeEnabled();
  const dir = process.env.LS_SHOT_DIR;
  if (dir) {
    await page.locator(".cm-step").first().screenshot({ path: `${dir}/publish-made-it-${page.viewportSize()?.width}.png` });
    await page.locator("#cm-credits").screenshot({ path: `${dir}/publish-credits-${page.viewportSize()?.width}.png` });
  }
  // Yes: nothing more to tick
  await ask.getByRole("button", { name: "Yes, I made it" }).click();
  await expect(make).toBeEnabled();
  await expect(check(page, "Credits and permissions")).toHaveClass(/\bok\b/);
});

test("a map made in the editor isn't asked about; a files warning shows under Files and textures", async ({ page }) => {
  await open(page, { kind: "editor", author: null, named: null }, { fileWarnings: ["1 texture isn't in any texture file here; players would see a checkerboard. Fix it in the map doctor first."] });
  await expect(page.getByTestId("cm-made-it")).toHaveCount(0);
  await expect(check(page, "Files and textures")).toContainText("1 texture isn't in any texture file here");
});
