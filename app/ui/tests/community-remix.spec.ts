// P5 (share and remix): Publish › A map offers "Include the editable source" (off by default)
// with the map's project, and sends them; a community map with source has "Remix this map", which
// asks the app to remix it and opens the editor. The app's answers are routed here.
import { expect, test } from "@playwright/test";

const MAP = "de_dust2";

test("Publish a map: the editable source option, off by default, sends the project", async ({ page }) => {
  await page.route(`**/api/tools/community/publish/${MAP}`, async (r) => {
    if (r.request().method() === "POST") {
      const body = r.request().postDataJSON();
      (globalThis as unknown as { sent: unknown }).sent = body;
      return r.fulfill({ json: { error: `sent:${JSON.stringify({ project: body.project, source: body.source })}` } });
    }
    const res = await r.fetch();
    const j = await res.json();
    j.ready.blockers = [];
    j.projects = [
      { path: "/tmp/other.lsmap", name: "other", match: false, remixOf: null },
      { path: "/tmp/de_dust2.lsmap", name: "de_dust2", match: true, remixOf: { gb: 4242, name: "Original", author: "someone" } },
    ];
    await r.fulfill({ json: j });
  });
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(`community:publish:${MAP}`)}`);
  const step = page.getByTestId("cm-source");
  await expect(step).toBeVisible({ timeout: 30_000 });
  const box = step.getByRole("checkbox", { name: "Include the editable source (others can remix it)" });
  await expect(box).not.toBeChecked();
  await expect(step.getByRole("combobox", { name: "The map's project" })).toHaveValue("/tmp/de_dust2.lsmap");
  await expect(step).toContainText("A remix of Original by someone");
  await box.check();
  await expect(step).toContainText("Adds source/ to the package");
  await page.getByRole("textbox", { name: "Made by" }).fill("qa");
  await page.getByRole("button", { name: "Make the package" }).click();
  await expect(page.getByText(/sent:\{"project":"\/tmp\/de_dust2\.lsmap","source":true\}/)).toBeVisible();
});

test("a community map with source: Remix this map asks the app and opens the editor", async ({ page }) => {
  const item = {
    kind: "map", model: "Mod", gb: 777, file: 1, md5: "", name: "Market", author: "someone", summary: "", games: ["cstrike"],
    map: { name: "de_market", modes: [], nav: true, lsv: false, places: false, res: true, overview: false, bspSize: 1, bspSha256: "" },
    mode: null, health: null, screenshots: [], tags: [], featured: false, added: 0, updated: 0, status: "listed", source: true,
  };
  await page.route("**/api/tools/community/item/mod/777", (r) =>
    r.fulfill({
      json: {
        model: "Mod", id: 777, kind: "map", name: "Market", author: "someone", category: "", text: "A map.", images: [], files: [], url: "https://gamebanana.com/mods/777",
        tagged: true, likes: 0, downloads: 0, item, trust: { level: "content", label: "Content", allowed: true, confirm: null, note: "" }, canTry: true, why: null, have: false, installed: null,
      },
    }),
  );
  let asked: unknown = null;
  await page.route("**/api/tools/community/remix-map", async (r) => {
    asked = r.request().postDataJSON();
    await new Promise((x) => setTimeout(x, 400));
    await r.fulfill({ json: { ok: true, path: "/tmp/remixes/de_market_remix.lsmap", name: "de_market_remix" } });
  });
  await page.goto("/tools-dev.html?open=community:mod:777");
  const btn = page.getByRole("button", { name: "Remix this map" });
  await expect(btn).toBeVisible({ timeout: 20_000 });
  await btn.click();
  await expect(page.getByText("Remixing…").first()).toBeVisible();
  await expect.poll(() => asked).toEqual({ model: "Mod", id: 777 });
  await expect(page.getByText(/de_market_remix is yours to change: a remix of Market/)).toBeVisible();
  await expect(page).toHaveURL(/open=editor|editor/);
});
