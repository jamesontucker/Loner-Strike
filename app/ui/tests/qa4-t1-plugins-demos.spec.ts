// QA4-T1 fixes for Plugin Studio and Demos (docs/notes/QA4-T1.md, "Fixed"): names Windows keeps
// for devices and unsafe characters refused, dialogs that keep what was typed on an error, the
// code editor on the first screen of a small window, a cut-short demo, and the small items
// (hidden files, slow-function notes, 0-byte size, the HLTV empty state, a faded Watch).
// Same harness as tools.spec.ts (tools-dev.html); every plugin and demo answer is faked with
// page.route, so nothing here reads or writes a game.
//   npx playwright test tests/qa4-t1-plugins-demos.spec.ts --project large --project small --project webkit
import { expect, test, type Page, type Route } from "@playwright/test";

const IGNORED = [/status of 404/, /Download the React DevTools/];

function watch(page: Page) {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  return problems;
}

const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });
const open = (page: Page, address: string) => page.goto(`/tools-dev.html?open=${encodeURIComponent(address)}`);

const SRC = '#include <amxmodx>\n\npublic plugin_init()\n{\n\tregister_plugin("QA", "1.0", "me");\n}\n';

/** A Plugin Studio with My plugins = qa.sma, 2 hidden files; renames and saves recorded. */
async function fakePlugins(page: Page) {
  const calls: { url: string; body: Record<string, unknown> }[] = [];
  const files = [{ name: "qa.sma", size: SRC.length, modified: 1763100000, readOnly: false }];
  await page.route("**/api/tools/plugins**", async (r) => {
    const url = r.request().url();
    const body = r.request().method() === "POST" ? (r.request().postDataJSON() as Record<string, unknown>) : {};
    if (r.request().method() === "POST") calls.push({ url, body });
    if (/\/plugins\/words/.test(url)) return json(r, { includes: [], symbols: [], unread: [] });
    if (/\/plugins\/log/.test(url))
      return json(r, {
        cursor: "x",
        lines: [
          { at: "10:00:00", text: '[AMXX] Plugin ("qa.amxx") has a performance issue. Function cmd_x executed more than 1.1ms.', error: false, slow: true },
          { at: "10:00:01", text: "[qa.amxx] hello", error: false, slow: false },
        ],
      });
    if (/\/plugins\/file\?/.test(url)) return json(r, { name: "qa.sma", folder: "mine", text: SRC, encoding: "utf8", readOnly: null, reset: null, resetHint: null, history: "" });
    if (/\/plugins\/rename/.test(url)) return json(r, body.to === "taken.sma" ? { error: "There's already a file called taken.sma. Pick another name." } : { ok: true, name: body.to });
    if (/\/plugins\/file/.test(url)) return json(r, { ok: true });
    if (/\/plugins$|\/plugins\?/.test(url))
      return json(r, {
        game: "cstrike",
        compiler: true,
        compilerProblem: null,
        running: false,
        trying: [],
        folders: [
          { id: "mine", label: "My plugins", path: "/x/mine", files, hidden: 2 },
          { id: "game", label: "Game's plugins", path: "/x/scripting", files: [], hidden: 0 },
          { id: "include", label: "Include files", path: "/x/include", files: [], hidden: 0 },
        ],
      });
    return json(r, { ok: true });
  });
  return calls;
}

test("Plugin Studio: CON and ; refused in a new plugin's name, the form keeps the name and says why", async ({ page }) => {
  const problems = watch(page);
  const calls = await fakePlugins(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "plugins");
  await page.getByRole("button", { name: "New plugin" }).click();
  const name = page.locator(".ps-new input").first();
  for (const [typed, why] of [
    ["con", /Windows keeps that name/],
    ["LPT1", /Windows keeps that name/],
    ["a;quit", /only letters, numbers/],
  ] as const) {
    await name.fill(typed);
    await page.getByRole("button", { name: "Make it" }).click();
    await expect(page.locator(".ps-new-error")).toHaveText(why);
    await expect(name).toHaveValue(typed);
    await expect(name).toHaveAttribute("aria-invalid", "true");
  }
  expect(calls.filter((c) => /\/plugins\/file/.test(c.url))).toEqual([]);
  // the hidden files are said
  await expect(page.locator(".ps-files")).toContainText("2 files hidden: rename them to use letters, numbers, - and _.");
  expect(problems).toEqual([]);
});

test("Plugin Studio: Rename stays open on an error with the name kept", async ({ page }) => {
  const problems = watch(page);
  const calls = await fakePlugins(page);
  await page.setViewportSize({ width: 1440, height: 900 });
  await open(page, "plugins:mine/qa.sma");
  await expect(page.locator(".ps-title")).toContainText("qa.sma", { timeout: 20_000 });
  await page.getByRole("button", { name: "More" }).click();
  await page.getByRole("menuitem", { name: /Rename/ }).click();
  const dialog = page.getByRole("alertdialog");
  const field = dialog.getByRole("textbox");
  await expect(field).toHaveValue("qa");
  await field.fill("Nul");
  await dialog.getByRole("button", { name: "Rename" }).click();
  await expect(dialog.locator(".ask-error")).toHaveText(/Windows keeps that name for a device/);
  await expect(field).toHaveValue("Nul");
  // the server's answer shows the same way
  await field.fill("taken");
  await expect(dialog.locator(".ask-error")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Rename" }).click();
  await expect(dialog.locator(".ask-error")).toHaveText("There's already a file called taken.sma. Pick another name.");
  await expect(field).toHaveValue("taken");
  await field.fill("qa_two");
  await dialog.getByRole("button", { name: "Rename" }).click();
  await expect(dialog).toHaveCount(0);
  expect(calls.filter((c) => /rename/.test(c.url)).map((c) => c.body.to)).toEqual(["taken.sma", "qa_two.sma"]);
  expect(problems).toEqual([]);
});

test("Plugin Studio: the Game log folds debug mode's slow-function notes", async ({ page }) => {
  const problems = watch(page);
  await fakePlugins(page);
  await open(page, "plugins:mine/qa.sma");
  await expect(page.locator(".ps-title")).toContainText("qa.sma", { timeout: 20_000 });
  await page.getByRole("tab", { name: "Game log" }).click();
  const log = page.getByRole("log");
  await expect(log).toContainText("[qa.amxx] hello");
  await expect(log).not.toContainText("performance issue");
  await expect(log).toContainText("1 note that a function took over 1 ms is hidden");
  expect(problems).toEqual([]);
});

for (const [w, h] of [
  [800, 600],
  [1024, 680],
]) {
  test(`Plugin Studio at ${w}x${h}: the code editor is on the first screen, the files a drawer`, async ({ page }) => {
    const problems = watch(page);
    await fakePlugins(page);
    await page.setViewportSize({ width: w, height: h });
    await open(page, "plugins:mine/qa.sma");
    await expect(page.locator(".ps-title")).toContainText("qa.sma", { timeout: 20_000 });
    const ed = (await page.locator(".ps-editor-wrap").boundingBox())!;
    expect(ed.y, "the editor starts on the first screen").toBeLessThan(h * 0.45);
    expect(ed.y + Math.min(ed.height, 200), "and 200 px of it shows").toBeLessThan(h);
    await expect(page.locator(".ps-files")).toBeHidden();
    await page.getByRole("button", { name: "Files" }).click();
    await expect(page.locator(".ps-files")).toBeVisible();
    await page.locator(".ps-file", { hasText: "qa.sma" }).click();
    await expect(page.locator(".ps-files")).toBeHidden();
    const sideways = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    expect(sideways).toBeLessThanOrEqual(0);
    expect(problems).toEqual([]);
  });
}

// ---- Demos ----

const DEMO = { at: "game:cstrike", where: "Your game (cstrike)", game: "cstrike", hltv: false, modified: 1763100000, playable: true, safeName: "x.dem", protocol: 48, demoGame: "cstrike" };

async function fakeDemos(page: Page, demos: Record<string, unknown>[]) {
  const calls: Record<string, unknown>[] = [];
  await page.route("**/api/tools/demos**", async (r) => {
    const url = r.request().url();
    if (/\/demos\/rename/.test(url)) {
      const b = r.request().postDataJSON() as Record<string, unknown>;
      calls.push(b);
      return json(r, b.to === "taken" ? { error: "There's already a demo called taken.dem." } : { ok: true, name: `${b.to}.dem`, message: `Renamed to ${b.to}.dem.` });
    }
    if (/\/demos\/detail/.test(url)) {
      const name = new URL(url).searchParams.get("name");
      const cut = name === "cut.dem";
      return json(r, { map: "de_dust2", game: "cstrike", protocol: 48, demoProtocol: 5, seconds: 0, parts: [], names: [], server: null, maxPlayers: null, frames: 0, problem: cut ? "This demo looks cut short (still being recorded, or damaged)." : null, cutShort: cut, canReplay: false });
    }
    return json(r, { demos, places: [{ at: "game:cstrike", label: "Your game (cstrike)", folder: "/x" }], gameOpen: false });
  });
  return calls;
}

test("Demos: a cut-short demo says so and Watch is off; 0 bytes reads Empty", async ({ page }) => {
  const problems = watch(page);
  await fakeDemos(page, [
    { ...DEMO, name: "cut.dem", size: 1_500_000, map: "de_dust2", seconds: 0, problem: "This demo looks cut short (still being recorded, or damaged)." },
    { ...DEMO, name: "empty.dem", size: 0, error: "It's too short to be a demo." },
    { ...DEMO, name: "good.dem", size: 3_700_000, map: "de_dust2", seconds: 65 },
  ]);
  await open(page, "demos");
  const cut = page.locator(".dm-row", { hasText: "cut" });
  await expect(cut).toContainText("Cut short", { timeout: 20_000 });
  await expect(cut.getByRole("button", { name: "Watch cut.dem" })).toBeDisabled();
  await expect(page.locator(".dm-row", { hasText: "good" }).getByRole("button", { name: "Watch good.dem" })).toBeEnabled();
  await expect(page.locator(".dm-row", { hasText: "empty" }).locator(".dm-c-size")).toHaveText("Empty");
  await cut.click();
  const panel = page.locator(".dm-panel");
  await expect(panel.locator(".dm-facts")).toContainText("This demo looks cut short (still being recorded, or damaged).");
  const watchBtn = panel.getByRole("button", { name: "Watch" });
  await expect(watchBtn).toBeDisabled();
  expect(Number(await watchBtn.evaluate((e) => getComputedStyle(e).opacity))).toBeLessThanOrEqual(0.6);
  expect(problems).toEqual([]);
});

test("Demos: rename refuses CON and ;, stays open on an error, keeps what was typed", async ({ page }) => {
  const problems = watch(page);
  const calls = await fakeDemos(page, [{ ...DEMO, name: "good.dem", size: 3_700_000, map: "de_dust2", seconds: 65 }]);
  await open(page, "demos");
  const row = page.locator(".dm-row", { hasText: "good" });
  await expect(row).toBeVisible({ timeout: 20_000 });
  await row.hover();
  await row.getByRole("button", { name: "Rename good.dem" }).click();
  const dialog = page.getByRole("alertdialog");
  const field = dialog.getByRole("textbox");
  await expect(field).toHaveValue("good");
  for (const [typed, why] of [
    ["CON", /Windows keeps that name/],
    ["com3.dem", /Windows keeps that name/],
    ["a;quit", /letters, numbers, spaces and - _ \. \( \) \[ \]/],
  ] as const) {
    await field.fill(typed);
    await dialog.getByRole("button", { name: "Rename" }).click();
    await expect(dialog.locator(".ask-error")).toHaveText(why);
    await expect(field).toHaveValue(typed);
  }
  await field.fill("taken");
  await dialog.getByRole("button", { name: "Rename" }).click();
  await expect(dialog.locator(".ask-error")).toHaveText("There's already a demo called taken.dem.");
  await expect(field).toHaveValue("taken");
  await field.fill("better");
  await dialog.getByRole("button", { name: "Rename" }).click();
  await expect(dialog).toHaveCount(0);
  expect(calls.map((c) => c.to)).toEqual(["taken", "better"]);
  expect(problems).toEqual([]);
});

test("Demos: no HLTV demos says where they come from; the count and its ? stay on one line at 800 px", async ({ page }) => {
  const problems = watch(page);
  await fakeDemos(page, [{ ...DEMO, name: "good.dem", size: 3_700_000, map: "de_dust2", seconds: 65 }]);
  await page.setViewportSize({ width: 800, height: 600 });
  await open(page, "demos");
  await expect(page.locator(".dm-row")).toHaveCount(1, { timeout: 20_000 });
  const count = (await page.locator(".dm-count .tl-count").boundingBox())!;
  const help = (await page.locator(".dm-count").locator("button, [role=button], .info").last().boundingBox())!;
  expect(Math.abs(count.y + count.height / 2 - (help.y + help.height / 2))).toBeLessThan(12);
  await page.getByRole("group", { name: "Which demos" }).getByRole("button", { name: "HLTV" }).click();
  await expect(page.locator(".tl-empty")).toContainText("HLTV on one of your servers records whole matches");
  await expect(page.getByRole("button", { name: "Go to Servers" })).toBeVisible();
  expect(problems).toEqual([]);
});
