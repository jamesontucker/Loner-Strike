// Rotation & extras > Map rotation: the main rotation is the server's own map cycle, read
// where the server keeps it (for a server on another machine, the machine's mapcycle.txt).
// The page opens on the mode being played when that mode has its own list (the list in use),
// and a main rotation that couldn't be read says so instead of showing an empty list, and
// can't be saved over.
//   npx playwright test extras-rotation-main --project=large --project=small
import { expect, test, type Page } from "@playwright/test";

const MAIN = ["de_airstrip", "cs_havana", "de_chateau"];
const CLASSIC = ["de_dust2", "de_inferno", "de_nuke"];
const settings = {
  rotation_random: true, rtv_cycle: true,
  ads: { on: false, minutes: 5, random: false, prefix: "", messages: [], credit: true },
  afk: { on: false, spec: 120, kick: 180 }, ping: { on: false, max: 250, checks: 5 }, fastdl: "",
  joins: { on: false, country: true, skip_bots: true },
  votes: { kick: false, ban: false, percent: 60, min_players: 3, ban_minutes: 30, cooldown: 3, kick_bots: false },
  empty: { on: false, minutes: 10, map: "", mode: "" },
};
const extras = (over: object) => ({
  settings, modes: [{ id: "classic", name: "Classic" }, { id: "gungame", name: "Gun Game" }], current_mode: "classic", plugin: true, dedicated: true, geoip: false, schedule: null,
  rotation: { file: "mapcycle.txt", main: MAIN, main_error: null, modes: { classic: CLASSIC } }, ...over,
});

async function open(page: Page, reply: object) {
  const state = {
    online: true,
    state: { mode: "classic", map: "de_dust2", nav: 1, career: 0, navbatch: 0, navleft: 0, bots: 5, quota: 5, humans: 1, maxplayers: 16, timeleft: 600, presets: 0, addons_pending: 0, version: "1.0.0", crashmap: "", preset: "", peaceful: 0, paused: 0, recording: 0, timelimit: 20, rounds_t: 0, rounds_ct: 0 },
    modes: [{ id: "classic", name: "Classic" }], settings: [], addons: [], addonsets: [], players: [], presets: [], events: [], me: {},
  };
  await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: state }));
  await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
    r.fulfill({ json: { installs: [{ hl: "/fake/Half-Life", kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false }] }], catalog: [], native_mac: false } }),
  );
  await page.route(/\/api\/app(\?|$)/, (r) =>
    r.fulfill({ json: { version: "1.0.0", game: { id: "cstrike", dir: "/fake/Half-Life/cstrike", hl: "/fake/Half-Life", dedicated: false, installed: ["core", "foundation"], outdated: [], unmanaged: [], unbuilt: [], others: [] } } }),
  );
  await page.route(/\/api\/extras(\?|$)/, (r) => r.fulfill({ json: reply }));
  await page.route(/\/api\/maps(\?|$)/, (r) => r.fulfill({ json: [...MAIN, ...CLASSIC].map((name) => ({ name })) }));
  await page.addInitScript(() => {
    localStorage.setItem("ls.section", "extras");
    localStorage.setItem("ls.extrasTab", "rotation");
  });
  await page.goto("/");
  const pick = page.getByRole("combobox", { name: /Which rotation/ });
  await pick.waitFor({ timeout: 30_000 });
  return pick;
}

const rows = (page: Page) => page.getByRole("list", { name: "Maps in the rotation" }).getByRole("listitem");

test("opens on the playing mode's own list; the main rotation shows the server's map cycle", async ({ page }) => {
  const pick = await open(page, extras({}));
  await expect(pick).toHaveValue("classic");
  await expect(rows(page)).toHaveCount(3);
  await expect(rows(page).first()).toContainText("de_dust2");
  await pick.selectOption("");
  await expect(rows(page)).toHaveCount(3);
  await expect(rows(page).first()).toContainText("de_airstrip");
});

test("without its own list, the playing mode opens on the main rotation", async ({ page }) => {
  const pick = await open(page, extras({ current_mode: "gungame" }));
  await expect(pick).toHaveValue("");
  await expect(rows(page).first()).toContainText("de_airstrip");
});

test("a main rotation that couldn't be read says so, and can't be saved over", async ({ page }) => {
  const why = "Couldn't read the main rotation from the server's machine (timed out).";
  const pick = await open(page, extras({ current_mode: "gungame", rotation: { file: "mapcycle.txt", main: [], main_error: why, modes: {} } }));
  await expect(pick).toHaveValue("");
  await expect(page.getByTestId("rot-main-error")).toContainText(why);
  await expect(page.getByText("No maps yet")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Save rotation" })).toBeDisabled();
});
