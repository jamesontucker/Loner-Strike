// Community's creature kind (CP-kind): Share… in Creature Studio opens the creature's Publish page
// (checks, details, a picture of it, Make the package, then the upload steps); a creature card
// says Get, installs it and offers Remix; an installed type in Creature Studio offers Remix….
// The app's answers are routed here (no GameBanana, no game).
import { expect, test, type Page } from "@playwright/test";

const ZERO = [0, 0, 0];
function creature(name: string, extra: Record<string, unknown> = {}) {
  return {
    name, base: "runner", display: name, category: "monster", hp: 77, speed: 300, walk_speed: 110, aggression: 0.5, detect_radius: 1200, detect_fov: 120, hearing: 1,
    attack: "melee", damage: 10, attack_range: 64, attack_rate: 1, attack_delay: 0.3, spread: 0, flee_hp: 0, behaviour: "chase", radius: 0, target: "players", hates: "",
    respawn: 0, corpse_time: 10, moves: true, friendly: false, flee_home: false, frags: 1, money: 0, model: "models/qa/x.mdl", skin: 0, body: 0, scale: 1,
    hull_min: [-16, -16, 0], hull_max: [16, 16, 72], glow: ZERO, fallback_glow: ZERO, blood: "red", anim: ["", "", "", "", "", ""], snd: ["", "", "", "", "", "", ""],
    enrage_below: 0, enrage_speed: 0, enrage_damage: 1, enrage_anim: 1, tier2_skin: -1, tier2_body: -1, tier2_hp: 1, tier3_skin: -1, tier3_body: -1, tier3_hp: 1,
    builtin: false, inFile: true, set: ["hp"], skipped: false, summary: "A fast runner.", ...extra,
  };
}

/** Creature Studio with your type qa_runner and an installed gb77.crawler. */
async function fakeCreatures(page: Page, after?: () => unknown) {
  let remixed = false;
  const status = () => ({
    game: "Counter-Strike 1.6", server: false, file: "addons/amxmodx/configs/gamemodes/creatures.ini", exists: true,
    types: [creature("zombie", { builtin: true, base: "", inFile: false, set: [] }), creature("qa_runner"), ...(remixed ? [creature("crawler")] : [])],
    templates: [creature("zombie", { builtin: true, base: "" })], problems: [],
    limits: { ranges: [], categories: ["monster"], behaviours: ["chase"], attacks: ["melee", "ranged", "none"], targets: ["players"], maxTypes: 48, maxDisplay: 40, maxName: 31 },
    installed: [{ name: "gb77.crawler", display: "Crawler", base: "runner", summary: "It crawls.", from: ["mod-77"], model: "models/bob/crawler.mdl" }],
  });
  await page.route("**/api/tools/creatures**", (r) => {
    const u = r.request().url();
    if (/\/creatures\/check/.test(u)) return r.fulfill({ json: { problems: [], summaries: [], models: null } });
    if (/\/creatures\/live/.test(u)) return r.fulfill({ json: { running: false, message: "Your game isn't running a map." } });
    if (/\/creatures\/used-by/.test(u)) return r.fulfill({ json: { usedBy: {} } });
    if (/\/creatures\/names/.test(u)) return r.fulfill({ json: { builtin: ["zombie"], own: ["qa_runner"], installed: [] } });
    if (/\/creatures\/model/.test(u)) return r.fulfill({ json: { report: { candidates: [{ path: "models/qa/x.mdl", found: true, kind: "custom", note: "" }], picked: 0, standIn: false }, model: { folder: "cstrike", path: "models/qa/x.mdl" }, picks: [], sequences: [] } });
    if (/\/creatures$|\/creatures\?/.test(u)) return r.fulfill({ json: status() });
    return r.fulfill({ json: {} });
  });
  await page.route("**/api/tools/community/remix-creature", (r) => {
    remixed = true;
    after?.();
    return r.fulfill({ json: { ok: true, name: "crawler", message: "Made crawler, your own copy of gb77.crawler. Share it when you've made it yours." } });
  });
}

test("Creature Studio: Share… packs a type with its picture, then the upload steps", async ({ page }) => {
  await fakeCreatures(page);
  let sent: Record<string, unknown> | null = null;
  await page.route("**/api/tools/community/publish-creature/qa_runner", (r) => {
    if (r.request().method() === "GET") return r.fulfill({ json: { name: "qa_runner", includes: ["qa_runner (with its model)"], blockers: [], remixOf: null, member: 0 } });
    sent = r.request().postDataJSON();
    return r.fulfill({
      json: {
        package: { file: "jo.qa_runner-creature-loner-strike.zip", size: 2048, md5: "abc", sha256: "", contents: ["creatures/creatures.ini"], screenshots: ["x-1.png"], warnings: [], item: { kind: "creature", name: "QA Runner", creatures: [{ name: "jo.qa_runner", model: true, sounds: 0 }] } },
        form: { submitUrl: "https://gamebanana.com/mods/add", game: "Counter-Strike 1.6", category: "Skins > Monsters", title: "QA Runner", text: "A creature", tag: { title: "Software Used", value: "LonerStrike" }, file: "jo.qa_runner-creature-loner-strike.zip", md5: "abc" },
      },
    });
  });
  // (no model bundle here: the 3D view says so, and a drawn canvas stands in for the picture)
  await page.route("**/api/tools/assets/**", (r) => r.fulfill({ status: 404, body: "" }));
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /qa_runner/ }).click();
  await page.getByRole("button", { name: "Share…" }).click();
  const pub = page.getByTestId("cm-pub-creature");
  await expect(pub).toContainText("qa_runner (with its model)", { timeout: 30_000 });
  await expect(pub).toContainText("Shared under your GameBanana account");
  // first time: the profile names the creatures
  await page.getByLabel("Your GameBanana profile").fill("https://gamebanana.com/members/31337");
  const make = page.getByRole("button", { name: "Make the package" });
  await expect(make).toBeDisabled();
  await page.locator(".cm-creature-view").evaluate((el) => {
    const c = document.createElement("canvas");
    c.width = 64;
    c.height = 64;
    const g = c.getContext("2d")!;
    g.fillStyle = "#c33";
    g.fillRect(0, 0, 64, 64);
    el.appendChild(c);
  });
  await page.getByRole("button", { name: "Take the picture" }).click();
  await expect(page.getByRole("img", { name: "Picture of qa_runner" })).toBeVisible();
  const author = page.getByLabel(/GameBanana name|Made by|Author/i).first();
  if (!(await author.inputValue())) await author.fill("Jo");
  await make.click();
  await expect(page.getByText("jo.qa_runner-creature-loner-strike.zip").first()).toBeVisible();
  expect(sent).not.toBeNull();
  expect(String((sent as unknown as { preview: { data: string } }).preview.data)).toMatch(/^data:image\/png;base64,/);
  expect((sent as unknown as { member: number }).member).toBe(31337);
});

test("Community: a creature card, Get installs it, then Remix", async ({ page }) => {
  await fakeCreatures(page);
  const entry = {
    model: "Mod", id: 77, key: "mod-77", kind: "creature", name: "Crawler", author: "bob", summary: "", category: "Monsters", image: null, likes: 0,
    added: 0, updated: 0, listed: true, map: null, bots: "", modes: [], have: false, badges: [], games: [], creatures: 1,
    trust: { level: "data", label: "Data only", allowed: true, confirm: null, note: "" }, canTry: true,
  };
  await page.route("**/api/tools/community/browse*", (r) => r.fulfill({ json: { items: [entry], index: "live", gamebanana: "live", indexUpdated: 0 } }));
  const item = {
    kind: "creature", model: "Mod", gb: 77, file: 1, md5: "", name: "Crawler", author: "bob", summary: "It crawls.", games: [], map: null, health: null,
    screenshots: [], tags: [], featured: false, added: 0, updated: 0, status: "listed", creatures: [{ name: "gb77.crawler", model: true, sounds: 0 }],
  };
  await page.route("**/api/tools/community/item/mod/77", (r) =>
    r.fulfill({
      json: {
        model: "Mod", id: 77, kind: "creature", name: "Crawler", author: "bob", category: "Monsters", text: "", images: [], files: [], url: "https://gamebanana.com/mods/77",
        tagged: true, likes: 0, downloads: 0, item, trust: { level: "data", label: "Data only", allowed: true, confirm: null, note: "" }, canTry: true, why: null, have: false, installed: null,
      },
    }),
  );
  await page.route("**/api/tools/community/library*", (r) => r.fulfill({ json: { items: [], updates: 0 } }));
  let tried = false;
  await page.route("**/api/tools/community/try", (r) => {
    tried = true;
    return r.fulfill({ json: { ok: true, kind: "creature", types: ["gb77.crawler"], message: "Added gb77.crawler. Modes and maps can spawn it from the next map." } });
  });
  await page.goto("/tools-dev.html?open=community:browse:creature");
  await expect(page.getByRole("radio", { name: "Creatures" }).or(page.getByRole("button", { name: "Creatures" })).first()).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: /Crawler/ }).first().click();
  await page.getByRole("button", { name: "Get", exact: true }).click();
  const done = page.getByTestId("cm-creature-done");
  await expect(done).toContainText("Added gb77.crawler.");
  expect(tried).toBe(true);
  await done.getByRole("button", { name: "Remix" }).click();
  await expect(page.locator(".toast", { hasText: "Made crawler, your own copy of gb77.crawler." })).toBeVisible();
  await expect(page.getByRole("listbox", { name: "Creature types" })).toBeVisible();
});

test("Creature Studio: an installed type offers Remix…, and the copy opens", async ({ page }) => {
  await fakeCreatures(page);
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "From Community" }).getByRole("option", { name: /Crawler/ }).click();
  await page.getByRole("button", { name: "Remix…" }).click();
  await expect(page.locator(".toast", { hasText: "Made crawler" })).toBeVisible();
  await expect(page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /crawler/, selected: true })).toBeVisible();
  // your own types can be shared; the installed one can't
  await expect(page.getByRole("button", { name: "Share…" })).toBeVisible();
});
