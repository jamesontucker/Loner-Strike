// Map editor: the Budget panel (how close the map is to GoldSrc's limits). Live bars for
// ls_outpost and br_valley (pasted from maps-src) and a Hammer import (LS_TEST_RMF: the .rmf
// the dev server's LS_DEV_PICK answers with), precache with a mode's roster, the last compile's
// numbers from its log, contributors with Pick, and the warning before Playtest and Publish when
// a bar is red. P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";
import * as fs from "node:fs";
import * as path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../../..");
const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

async function open(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
}

async function showBudget(page: Page) {
  await page.locator('[data-dock-strip] [role=tab]', { hasText: /^Budget$/ }).first().click();
  const b = page.getByRole("region", { name: "Budget" });
  await expect(b.locator(".p5-bud").first()).toBeVisible({ timeout: 20000 });
  return b;
}

async function pasteMap(page: Page, file: string) {
  const text = fs.readFileSync(path.join(ROOT, file), "utf8");
  await api(page, "/api/tools/editor/new", {});
  // (one request: the editor's command route takes big pastes)
  const r = await api(page, "/api/tools/editor/cmd", { op: "paste", text, offset: [0, 0, 0] });
  expect(r.error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
}

const barNum = async (b: ReturnType<Page["locator"]>, name: string) => {
  const t = await b.locator(".p5-bud", { has: b.page().locator(".p5-bud-name", { hasText: new RegExp(`^${name}`) }) }).first().locator(".p5-bud-num").innerText();
  return Number(t.split("/")[0].replace(/,/g, "").trim());
};

for (const [name, file] of [
  ["ls_outpost", "maps-src/ls_outpost/ls_outpost.map"],
  ["br_valley", "maps-src/br_valley/br_valley.map"],
] as const) {
  test(`live bars and precache for ${name}`, async ({ page }) => {
    test.setTimeout(120_000);
    await open(page);
    await pasteMap(page, file);
    const b = await showBudget(page);
    const doc = await api(page, "/api/tools/editor/doc");
    expect(await barNum(b, "Entities")).toBe(doc.entities.length + 1);
    expect(await barNum(b, "Brush entities")).toBeGreaterThan(0);
    expect(await barNum(b, "Different textures")).toBeGreaterThan(1);
    expect(await barNum(b, "Precached models")).toBeGreaterThanOrEqual(160);
    await expect(b).toContainText(/sounds<\/b>|sounds and/);
    // a bar's contributors, and Pick selects them in the map
    await b.locator(".p5-bud-head", { hasText: /^Entities/ }).click();
    const more = b.locator(".p5-bud-more").first();
    await expect(more.locator("li").first()).toBeVisible();
    await more.getByRole("button", { name: "Pick" }).first().click();
    await expect(page.locator(".ed-selsize")).toContainText("picked");
  });
}

test("a Hammer import, a mode's roster, the last compile, and the warnings", async ({ page }) => {
  test.setTimeout(150_000);
  await open(page);
  if (process.env.LS_TEST_RMF) {
    const r = await api(page, "/api/tools/editor/import-other", {});
    expect(r.error ?? null).toBeNull();
    await page.reload();
    const back = page.getByRole("button", { name: /^Back to/ });
    if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
    await page.locator(".ed-pane").first().waitFor();
  }
  let b = await showBudget(page);
  const doc = await api(page, "/api/tools/editor/doc");
  if (process.env.LS_TEST_RMF) expect(doc.brushes.length).toBeGreaterThan(100);
  expect(await barNum(b, "Entities")).toBe(doc.entities.length + 1);
  // a mode with a creature roster adds its models and sounds
  const kits = await api(page, "/api/tools/editor/kits");
  fs.mkdirSync(kits.folder, { recursive: true });
  const kitFile = path.join(kits.folder, "p5_budget_stand.kit.json");
  fs.writeFileSync(kitFile, JSON.stringify({ schema: "loner-strike/mode-kit", version: 1, mode: { id: "p5_budget_stand", name: "Budget Stand", description: "test" }, markers: [], components: [], creatures: ["zombie", "headcrab"] }));
  const before = await barNum(b, "Precached sounds");
  const keys = (doc.world.keys as [string, string][]).filter(([k]) => k !== "ls_modes");
  await api(page, "/api/tools/editor/cmd", { op: "set_world", keys: [...keys, ["ls_modes", "p5_budget_stand"]] });
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  b = await showBudget(page);
  await expect(b).toContainText("Budget Stand with this roster (zombie, headcrab)");
  await expect.poll(() => barNum(b, "Precached sounds")).toBeGreaterThan(before);
  // the last compile (its log), with a red bar: Playtest and Publish ask first
  const rep = await api(page, "/api/tools/editor/budget");
  fs.mkdirSync(path.dirname(rep.logAt), { recursive: true });
  fs.writeFileSync(rep.logAt, "Object names  Objects/Maxobjs  Memory / Maxmem  Fullness\nmodels            30/512          1920/32768     ( 5.9%)\nclipnodes      31000/32767      248000/262136    (94.6%)\nplanes         20000/32768      400000/655360    (61.0%)\n");
  try {
    await b.getByRole("button", { name: "Refresh" }).click();
    await expect(b.locator(".p5-bud.red", { hasText: "clipnodes" })).toBeVisible();
    await expect(b).toContainText("over 90%");
    await b.locator(".p5-bud-head", { hasText: "clipnodes" }).click();
    await expect(b.locator(".p5-bud-more")).toContainText("CLIP brush");
    // Publish asks (never a silent block); Cancel stays here
    await page.getByRole("menubar", { name: "Editor menu" }).getByRole("button", { name: "Publish…" }).click();
    const sheet = page.getByRole("alertdialog").filter({ hasText: "Near the engine's limits" });
    await expect(sheet).toContainText("clipnodes");
    await sheet.getByRole("button", { name: "Cancel" }).click();
    await expect(page.locator(".ed-pane").first()).toBeVisible();
  } finally {
    fs.rmSync(rep.logAt, { force: true });
    fs.rmSync(kitFile, { force: true });
  }
});
