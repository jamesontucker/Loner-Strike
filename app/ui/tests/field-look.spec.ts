// Every text field, text box and dropdown has the app's field look (Parity A, from the site's
// Publish shot): never the browser's white box, its default font, or a monospace text box, on
// every main page, every tool and the dialogs and steps reachable from them. Code editors and
// log views that are meant to be monospace are left out (MONO_OK).
//   npx playwright test field-look --workers=1 [--project=webkit with LS_WEBKIT_ALL=1]
import { expect, test, type Page } from "@playwright/test";

const H = { "X-Panel": "1" };

/** Where monospace is the point: code, keys, coordinates, consoles and logs. */
const MONO_OK = [
  ".monaco-editor", ".console", ".cm-editor", "pre", "code", ".mono", ".address", ".log", "[data-mono]",
  ".p5-xf-field", ".editor-key", ".ed-keyvalue", ".st-code", ".rcon", ".lsctl",
];

type Bad = { where: string; field: string; why: string };

/** Fields looked at, per test (a sweep that sees none would pass on nothing). */
let seen = 0;

async function sweep(page: Page, where: string): Promise<Bad[]> {
  const r = await page.evaluate(
    ({ where, monoOk }) => {
      const out: { where: string; field: string; why: string }[] = [];
      let n = 0;
      const bodyFont = getComputedStyle(document.body).fontFamily.split(",")[0].trim().replace(/["']/g, "").toLowerCase();
      const text = 'input:not([type]), input[type="text"], input[type="search"], input[type="number"], input[type="url"], input[type="email"], input[type="password"], input[type="tel"], textarea, select';
      for (const el of Array.from(document.querySelectorAll<HTMLElement>(text))) {
        const r = el.getBoundingClientRect();
        const cs = getComputedStyle(el);
        if (r.width < 8 || r.height < 8 || cs.visibility === "hidden" || cs.display === "none" || Number(cs.opacity) === 0) continue;
        if (el.closest(".monaco-editor, [aria-hidden='true']")) continue;
        n++;
        const name = `${el.tagName.toLowerCase()}${el.getAttribute("type") ? `[${el.getAttribute("type")}]` : ""} "${el.getAttribute("aria-label") || el.getAttribute("placeholder") || el.getAttribute("name") || (el as HTMLInputElement).value?.slice(0, 30) || el.className}"`;
        // the background: white or near-white (and drawn: some fields are see-through on purpose)
        const m = cs.backgroundColor.match(/rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)(?:,\s*([\d.]+))?/);
        if (m) {
          const [rr, gg, bb] = [m[1], m[2], m[3]].map(Number);
          const a = m[4] === undefined ? 1 : Number(m[4]);
          const lum = (0.2126 * rr + 0.7152 * gg + 0.0722 * bb) / 255;
          if (a > 0.5 && lum > 0.8) out.push({ where, field: name, why: `white background ${cs.backgroundColor}` });
        }
        const fam = cs.fontFamily.split(",")[0].trim().replace(/["']/g, "").toLowerCase();
        const mono = /mono|menlo|consolas|courier/.test(cs.fontFamily.toLowerCase());
        if (mono) {
          if (!monoOk.some((s) => el.closest(s))) out.push({ where, field: name, why: `monospace ${cs.fontFamily}` });
        } else if (fam !== bodyFont) {
          out.push({ where, field: name, why: `font ${cs.fontFamily} (the app's is ${bodyFont})` });
        }
      }
      return { out, n };
    },
    { where, monoOk: MONO_OK },
  );
  seen += r.n;
  return r.out;
}

async function tools(page: Page, addr: string, wait = 3500) {
  await page.goto(`/tools-dev.html?open=${encodeURIComponent(addr)}`);
  await page.waitForTimeout(wait);
}

/** Clicks a button if it's there (some steps only exist with the right data). */
async function tap(page: Page, name: string | RegExp) {
  const b = page.getByRole("button", { name }).first();
  if (await b.count()) await b.click({ timeout: 3000 }).catch(() => {});
  await page.waitForTimeout(800);
}

test.describe("Field look", () => {
  test.setTimeout(600_000);
  test.beforeEach(() => {
    seen = 0;
  });

  test("main pages", async ({ page }) => {
    const bad: Bad[] = [];
    // a live game and the game set up (as qa4-core.spec does), so each page shows its controls
    const state = {
      online: true,
      state: { mode: "classic", map: "de_dust2", nav: 1, career: 0, navbatch: 0, navleft: 0, bots: 5, quota: 5, humans: 1, maxplayers: 16, timeleft: 600, presets: 0, addons_pending: 0, version: "1.0.0", crashmap: "", preset: "", peaceful: 0, paused: 0, recording: 0, timelimit: 20, rounds_t: 0, rounds_ct: 0 },
      modes: [{ id: "classic", name: "Classic" }], settings: [], addons: [], addonsets: [], players: [], presets: [], events: [], me: {},
    };
    await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: state }));
    await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
      r.fulfill({ json: { installs: [{ hl: "/fake/Half-Life", kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false }] }], catalog: [], native_mac: false } }),
    );
    for (const section of ["play", "maps", "bots", "match", "fun", "players", "addons", "friends", "browser", "servers", "admins", "community", "stats", "extras", "mods", "skins", "sounds", "tools", "settings"]) {
      await page.addInitScript((s) => localStorage.setItem("ls.section", s), section);
      await page.goto("/");
      await page.waitForTimeout(2500);
      bad.push(...(await sweep(page, `page ${section}`)));
    }
    // the top bar's dialogs (Announce: a text field)
    await tap(page, "Announce");
    bad.push(...(await sweep(page, "Announce")));
    await page.keyboard.press("Escape");
    expect(seen, "text fields looked at").toBeGreaterThanOrEqual(3);
    expect(bad, JSON.stringify(bad, null, 1)).toEqual([]);
  });

  test("tools and their steps", async ({ page }) => {
    const bad: Bad[] = [];
    const ids = ["overview", "studio", "creatures", "plugins", "doctor:de_dust2", "health", "navfix:de_dust2", "callouts:de_dust2", "radar:de_dust2", "playercfg", "people", "spray", "map:de_dust2", "replay", "demos", "assets", "community", "community:installed", "community:publish", "community:yours"];
    for (const id of ids) {
      await tools(page, id);
      bad.push(...(await sweep(page, `tool ${id}`)));
    }
    // Community › Publish of a Studio mode: every step
    const tpl = await (await page.request.get("/src/views/tools/studio/templates/king-of-the-hill.graph.json")).json();
    tpl.mode.id = "pw_field_look";
    await page.request.post("/api/tools/studio/graph/pw_field_look", { data: { graph: tpl }, headers: H });
    await tools(page, "community:publish:mode:pw_field_look", 5000);
    bad.push(...(await sweep(page, "publish a mode")));
    // the Studio's panels and dialogs
    await tools(page, "studio:pw_field_look", 5000);
    bad.push(...(await sweep(page, "studio")));
    for (const b of ["Mode settings", "Versions", /^View/]) {
      await tap(page, b);
      bad.push(...(await sweep(page, `studio ${b}`)));
      await page.keyboard.press("Escape");
    }
    await page.request.post("/api/tools/studio/graph/pw_field_look/delete", { headers: H });
    expect(seen, "text fields looked at").toBeGreaterThan(10);
    expect(bad, JSON.stringify(bad, null, 1)).toEqual([]);
  });

  test("map editor and its dialogs", async ({ page }) => {
    const bad: Bad[] = [];
    await page.goto("/editor-dev.html");
    await page.waitForTimeout(3000);
    bad.push(...(await sweep(page, "editor start")));
    await page.getByRole("button", { name: "New map", exact: true }).click({ timeout: 10_000 }).catch(() => {});
    await page.getByRole("menubar", { name: "Editor menu" }).waitFor({ timeout: 30_000 }).catch(() => {});
    await page.waitForTimeout(3000);
    bad.push(...(await sweep(page, "editor")));
    const menu = page.getByRole("menubar", { name: "Editor menu" });
    for (const [top, item] of [["Edit", "Transform…"], ["View", "Grid and snapping…"], ["Select", "Go to brush or entity number…"], ["Map", /Map properties|Map settings/]] as const) {
      if (!(await menu.count())) break;
      await menu.getByRole("menuitem", { name: top, exact: true }).click({ timeout: 3000 }).catch(() => {});
      await page.getByRole("menuitem", { name: item }).first().click({ timeout: 3000 }).catch(() => {});
      await page.waitForTimeout(800);
      bad.push(...(await sweep(page, `editor ${top} › ${item}`)));
      await page.keyboard.press("Escape");
    }
    for (const tab of ["Content", "Gameplay", "Textures", "Checks", "Budget", "I/O", "Compile", "Project"]) {
      await page.locator("[data-dock-strip] [role=tab]", { hasText: new RegExp(`^${tab.replace("/", "\\/")}$`) }).first().click({ timeout: 3000 }).catch(() => {});
      await page.waitForTimeout(700);
      bad.push(...(await sweep(page, `editor dock ${tab}`)));
    }
    expect(seen, "text fields looked at").toBeGreaterThan(10);
    expect(bad, JSON.stringify(bad, null, 1)).toEqual([]);
  });
});
