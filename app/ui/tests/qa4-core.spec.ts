// QA4-CORE fixes in the app's own pages (docs/notes/QA4-CORE.md), against the tools dev server
// (npm run test:tools) with the live game, setup and pictures stubbed:
// - F1: the top strip's map picture: the name tile under it, the radar or screenshot over it once
//   it loads, asking again while GameBanana is still looked up; a custom map keeps a clean tile
// - F2: reads send X-Panel (the supporters' VIP settings GET needs it)
// - F4: setup's done screen says plainly when a game kept another ReGameDLL, with "Use
//   Loner-Strike's"; the review's time estimate
// - UX: Tools folded on a first run, Find servers keeps Map and Latency in a small window, the
//   quick bar's More menu and a one-line update bar in a small window, "Update all" says why
//   it's off
import { expect, test, type Page, type Route } from "@playwright/test";

const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==", "base64");

function live(map: string) {
  return {
    online: true,
    state: { mode: "classic", map, nav: 1, career: 0, navbatch: 0, navleft: 0, bots: 5, quota: 5, humans: 1, maxplayers: 16, timeleft: 600, presets: 0, addons_pending: 0, version: "1.0.0", crashmap: "", preset: "", peaceful: 0, paused: 0, recording: 0, timelimit: 20, rounds_t: 0, rounds_ct: 0 },
    modes: [{ id: "classic", name: "Classic" }],
    settings: [],
    addons: [],
    addonsets: [],
    players: [],
    presets: [],
    events: [],
    me: {},
  };
}

/** A live game on `map`, and the app's info with the game set up (so no setup screen). */
async function liveGame(page: Page, map: string) {
  await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: live(map) }));
  await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
    r.fulfill({ json: { installs: [{ hl: "/fake/Half-Life", kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false }] }], catalog: [], native_mac: false } }),
  );
}

test.describe("QA4-CORE", () => {
  test("F1: the strip shows the map's name, then the radar, and asks again while a screenshot is looked up", async ({ page }) => {
    await liveGame(page, "de_qa_strip");
    const asked: string[] = [];
    await page.route(/\/thumb\/de_qa_strip\.png/, async (r: Route) => {
      asked.push(r.request().url());
      expect(r.request().headers()["x-panel"]).toBe("1");
      // the first ask: nothing yet (still looking); then the radar while still looking; then done
      const v = Number(new URL(r.request().url()).searchParams.get("v"));
      if (v === 0) return r.fulfill({ status: 404, headers: { "x-thumb-looking": "1", "cache-control": "no-store" }, body: "no picture" });
      return r.fulfill({ status: 200, contentType: "image/png", headers: { "x-thumb-kind": "radar", "x-thumb-looking": v < 2 ? "1" : "0", "cache-control": "no-cache" }, body: PNG });
    });
    await page.goto("/");
    const thumb = page.locator(".strip-thumb");
    await expect(thumb).toBeVisible({ timeout: 20_000 });
    // never an empty box: the name tile is there from the start
    await expect(thumb.locator(".no-pic")).toContainText("qa_strip");
    await expect(thumb.locator(".no-pic-kind")).toHaveText("de");
    await expect(thumb.locator("img")).toBeVisible({ timeout: 15_000 });
    // it asked with ?now=1 and a new ?v= each time, and stopped once the app said it was done
    // (counted by ?v=: the dev build's strict mode asks twice for each)
    const versions = () => new Set(asked.map((u) => new URL(u).searchParams.get("v"))).size;
    await expect.poll(versions, { timeout: 15_000 }).toBe(3);
    expect(asked.every((u) => u.includes("now=1"))).toBe(true);
    await page.waitForTimeout(5000);
    expect(versions()).toBe(3);
  });

  test("F1: a custom map with no picture keeps a clean name tile and isn't asked about again", async ({ page }) => {
    await liveGame(page, "my_custom_map");
    const asked = new Set<string | null>();
    await page.route(/\/thumb\/my_custom_map\.png/, (r) => {
      asked.add(new URL(r.request().url()).searchParams.get("v"));
      return r.fulfill({ status: 404, headers: { "x-thumb-looking": "0" }, body: "no picture" });
    });
    await page.goto("/");
    const thumb = page.locator(".strip-thumb");
    await expect(thumb.locator(".no-pic-name")).toHaveText("custom_map", { timeout: 20_000 });
    await expect(thumb.locator(".no-pic-kind")).toHaveText("my");
    await expect(thumb.locator("img")).toHaveCount(0);
    await page.waitForTimeout(5000);
    expect([...asked]).toEqual(["0"]);
    // the tile isn't wiped by an inline background
    expect(await thumb.evaluate((e) => (e as HTMLElement).style.backgroundImage)).toBe("");
  });

  test("F2: reads from the page send X-Panel", async ({ page }) => {
    await liveGame(page, "de_dust2");
    const headers: Record<string, string>[] = [];
    await page.route(/\/api\/app(\?|$)/, async (r) => {
      headers.push(r.request().headers());
      await r.fallback();
    });
    await page.goto("/");
    await expect.poll(() => headers.length, { timeout: 20_000 }).toBeGreaterThan(0);
    expect(headers.every((h) => h["x-panel"] === "1")).toBe(true);
  });

  test("F4: the done screen says a game kept another ReGameDLL, and Use Loner-Strike's replaces it", async ({ page }) => {
    const hl = "/fake/Half-Life";
    await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: { online: false, reason: "not running" } }));
    await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
      r.fulfill({ json: { installs: [{ hl, kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: false, components: [], foreign: false }] }], catalog: [], native_mac: false } }),
    );
    let started = false;
    await page.route(/\/api\/setup\/install(\?|$)/, (r) => ((started = true), r.fulfill({ json: { ok: true, job: "j1" } })));
    await page.route(/\/api\/setup\/job(\?|$)/, (r) =>
      r.fulfill({
        json: started
          ? { id: "j1", state: "done", kind: "install", steps: [], progress: 1, notes: ["Counter-Strike 1.6: Kept the ReGameDLL that was already there. It isn't Loner-Strike's, so bots won't have Loner-Strike's brain until it's replaced."], other_brain: ["cstrike"] }
          : { state: "idle", steps: [], progress: 0 },
      }),
    );
    const replaced: unknown[] = [];
    await page.route(/\/api\/setup\/use-our-brain(\?|$)/, async (r) => {
      replaced.push(r.request().postDataJSON());
      expect(r.request().headers()["x-panel"]).toBe("1");
      await new Promise((res) => setTimeout(res, 600));
      await r.fulfill({ json: { ok: true, message: "Counter-Strike 1.6 now has Loner-Strike's bot brain. The old ReGameDLL is backed up." } });
    });
    await page.goto("/");
    await page.getByRole("button", { name: "Set it up" }).click({ timeout: 20_000 });
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("button", { name: "Only the basics" }).click();
    // the basics name Last Stand and Battle Royale too; the estimate counts the maps
    await expect(page.getByText(/The basics are always included: .*Last Stand and Battle Royale/)).toBeVisible();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.locator(".estimate")).toHaveText("About 15 MB, plus the popular maps you don't have yet. Usually 2 to 4 minutes.");
    await page.getByRole("button", { name: "Install" }).click();
    const box = page.locator(".other-brain");
    // (the page keeps "Loner-Strike" on one line with a word joiner)
    await expect(box).toContainText(/Counter-Strike 1\.6: the bots won't have Loner-\u2060?Strike's brain\./, { timeout: 20_000 });
    // the same thing isn't said twice in the notes under it
    await expect(page.getByText("Kept the ReGameDLL that was already there")).toHaveCount(0);
    await box.getByRole("button", { name: /^Use Loner-\u2060?Strike's$/ }).click();
    await expect(box.getByRole("button", { name: /Replacing/ })).toBeVisible();
    await expect(page.locator(".other-brain.ok")).toContainText(/now has Loner-\u2060?Strike's bot brain/, { timeout: 10_000 });
    expect(replaced).toEqual([{ hl, game: "cstrike" }]);
  });

  test("UX 12: Tools starts folded and stays open once opened", async ({ page }) => {
    await liveGame(page, "de_dust2");
    await page.goto("/");
    await page.evaluate(() => localStorage.removeItem("ls.toolsOpen"));
    await page.reload();
    const fold = page.locator(".nav-group-fold");
    await expect(fold).toHaveAttribute("aria-expanded", "false", { timeout: 20_000 });
    await expect(page.locator("#side-tools")).toHaveCount(0);
    await fold.click();
    await expect(fold).toHaveAttribute("aria-expanded", "true");
    await page.reload();
    await expect(page.locator(".nav-group-fold")).toHaveAttribute("aria-expanded", "true", { timeout: 20_000 });
  });

  test("UX 3, 4, 5: a small window keeps Map and Latency, folds the quick bar, and the update bar is one line", async ({ page }) => {
    await page.setViewportSize({ width: 820, height: 600 });
    await liveGame(page, "de_dust2");
    await page.route(/\/api\/app(\?|$)/, async (r) => {
      const res = await r.fetch().catch(() => null);
      const base = res ? await res.json().catch(() => ({})) : {};
      await r.fulfill({ json: { ...base, version: base.version ?? "1.0.0", game: { id: "cstrike", dir: "/fake/Half-Life/cstrike", hl: "/fake/Half-Life", dedicated: false, installed: ["core", "foundation"], outdated: ["foundation"], unmanaged: [], unbuilt: [], others: [] } } });
    });
    await page.goto("/");
    // the quick bar: More holds Announce and Record demo
    const more = page.locator(".quick").getByRole("button", { name: "More", exact: true });
    await expect(more).toBeVisible({ timeout: 20_000 });
    await expect(page.locator(".quick .qb-full")).toHaveCount(2);
    await expect(page.locator(".quick .qb-full").first()).toBeHidden();
    await more.click();
    await expect(page.getByRole("menuitemcheckbox", { name: "Announce" }).or(page.getByRole("menuitem", { name: "Announce" }))).toBeVisible();
    await page.keyboard.press("Escape");
    // the update bar: one line, and Update all says why it's off while the game runs
    const bar = page.locator(".files-bar");
    await expect(bar).toBeVisible();
    const h = await bar.evaluate((e) => e.getBoundingClientRect().height);
    expect(h).toBeLessThan(48);
    const all = bar.getByRole("button", { name: "Update all" });
    if (await all.isDisabled()) await expect(all).toHaveAttribute("title", /^Close .* first$/);
    // Find servers: Map and Latency inside the table's width, no sideways scrolling needed
    await page.getByRole("button", { name: "Find servers" }).click();
    const table = page.locator(".browser-table").first();
    await expect(table).toBeVisible({ timeout: 20_000 });
    const box = await table.boundingBox();
    for (const name of ["c-map", "c-ping"]) {
      const b = await table.locator(`.browser-row.head .${name}`).boundingBox();
      expect(b, name).not.toBeNull();
      expect(b!.x + b!.width).toBeLessThanOrEqual(box!.x + box!.width + 1);
    }
    await expect(table.locator(".browser-row.head .c-bots")).toBeHidden();
  });
});
