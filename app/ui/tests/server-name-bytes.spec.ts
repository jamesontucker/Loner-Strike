// Server names take 63 bytes of UTF-8 (Steam's server browser limit, 64 with the NUL), counted in
// bytes and never cut inside a character, with "n left" in the last 10. New server's Name field,
// against the tools dev server with the servers list and the setup scan stubbed.
import { expect, test } from "@playwright/test";

test("New server: a name of 63 bytes, cut between characters, with n left", async ({ page }) => {
  await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: { online: false, reason: "not running" } }));
  await page.route(/\/api\/servers(\?|$)/, (r) => r.fulfill({ json: { servers: [] } }));
  await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
    r.fulfill({ json: { installs: [{ hl: "/fake/Half-Life", kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false }] }], catalog: [], native_mac: false } }),
  );
  await page.route(/\/api\/app(\?|$)/, async (r) => {
    const res = await r.fetch().catch(() => null);
    const base = res ? await res.json().catch(() => ({})) : {};
    await r.fulfill({ json: { ...base, version: base.version ?? "1.0.0", game: { id: "cstrike", dir: "/fake/Half-Life/cstrike", hl: "/fake/Half-Life", dedicated: false, installed: ["core", "foundation"], outdated: [], unmanaged: [], unbuilt: [], others: [] } } });
  });
  await page.goto("/");
  // the sidebar's New server (under the Servers header, which only folds the list)
  await page.locator(".side-server.add").click({ timeout: 30_000 });
  const sheet = page.locator(".new-server");
  const name = sheet.getByPlaceholder("Like Friday night frags");
  await expect(name).toBeVisible();
  // short: no counter
  await name.fill("Friday frags");
  await expect(sheet.locator(".name-left")).toHaveCount(0);
  // 49 ASCII: 14 left, still no counter; 55: "8 left"
  await name.fill("x".repeat(55));
  await expect(sheet.locator(".name-left")).toHaveText("8 left");
  // é is 2 bytes: 40 of them (80 bytes) keep 31 (62 bytes), never half of one
  await name.fill("é".repeat(40));
  await expect(name).toHaveValue("é".repeat(31));
  await expect(sheet.locator(".name-left")).toHaveText("1 left");
  // an emoji (4 bytes) that would pass 63 isn't kept; one that ends exactly at 63 is
  await name.fill(`${"a".repeat(60)}🙂`);
  await expect(name).toHaveValue("a".repeat(60));
  await name.fill(`${"a".repeat(59)}🙂`);
  await expect(name).toHaveValue(`${"a".repeat(59)}🙂`);
  await expect(sheet.locator(".name-left")).toHaveText("0 left");
});
