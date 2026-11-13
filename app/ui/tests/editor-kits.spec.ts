// Map editor: mode map kits (docs/notes/X.md "Mode map kit format"). A kit put in the app's kits
// folder shows in Gameplay > Mode kits; "Made for" saves ls_modes; the checks list what the mode
// still needs with Place; a placed marker gets the kit's keys and a typed form; ls_creature's
// Type lists the creature types. Runs on editor-dev.html against the tools dev server
// (npm run test:tools -- tests/editor-kits.spec.ts); P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";
import * as fs from "node:fs";
import * as path from "node:path";

const api = (page: Page, p: string, body?: unknown) =>
  page.evaluate(
    async ([u, b]) => (await fetch(u as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [p, body] as const,
  );

const KIT = {
  schema: "loner-strike/mode-kit",
  version: 1,
  mode: { id: "p5_test_stand", uid: "5e0c1f7a-3b2d-4c11-9a8e-1f2e3d4c5b6a", name: "P5 Test Stand", description: "Hold out (test kit)" },
  source: "hand",
  markers: [
    {
      id: "creature_spawn",
      label: "Creature spawn",
      description: "Where creatures come in.",
      type: "point",
      required: { min: 2, max: 8 },
      keys: [
        { name: "wave", type: "number", default: 1, min: 1, max: 99, help: "First wave that uses it" },
        { name: "side", type: "choice", default: "north", choices: [["north", "North"], ["south", "South"]] },
      ],
      draw: { colour: "#c0392b", icon: "skull", shape: "diamond" },
    },
    { id: "base", label: "Base", type: "area", required: { min: 1, max: 1 }, draw: { colour: "#2980b9", shape: "box", size: [256, 256, 128] } },
  ],
  components: [{ classname: "ls_wave_spawner", min: 1, max: 1, note: "One wave spawner" }],
  creatures: ["zombie"],
};

async function open(page: Page, withKits = true) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const r = await api(page, "/api/tools/editor/kits");
  if (!withKits) return r.folder as string;
  fs.mkdirSync(r.folder, { recursive: true });
  fs.writeFileSync(path.join(r.folder, "p5_test_stand.kit.json"), JSON.stringify(KIT));
  fs.writeFileSync(path.join(r.folder, "p5_broken.kit.json"), "{ not json");
  return r.folder as string;
}

test("Mode kits: made for, what it needs, place a marker with its keys", async ({ page }) => {
  const folder = await open(page);
  try {
    await page.getByRole("button", { name: "Gameplay", exact: true }).or(page.getByRole("tab", { name: "Gameplay" })).first().click();
    await page.getByRole("group", { name: "Gameplay" }).getByRole("button", { name: "Mode kits" }).click();
    const kit = page.getByRole("group", { name: "P5 Test Stand kit" });
    await expect(kit).toBeVisible();
    await expect(page.getByText(/p5_broken\.kit\.json \(It isn't valid JSON/)).toBeVisible();
    // made for: saved in the map as ls_modes, and the needs show
    await page.getByRole("group", { name: "Made for" }).getByRole("button", { name: "P5 Test Stand" }).click();
    await expect.poll(async () => ((await api(page, "/api/tools/editor/doc")).world.keys as [string, string][]).find(([k]) => k === "ls_modes")?.[1]).toBe("5e0c1f7a-3b2d-4c11-9a8e-1f2e3d4c5b6a:p5_test_stand");
    const needs = page.getByRole("list", { name: "What P5 Test Stand still needs" });
    await expect(needs).toContainText("2–8 (has 0)");
    await expect(needs.locator("li")).toHaveCount(3);
    // the live checks say so too, with Place
    const checks = await api(page, "/api/tools/editor/checks");
    expect(checks.issues.filter((i: { code: string }) => i.code === "kit-needs").map((i: { place: string }) => i.place)).toEqual(["p5_test_stand:creature_spawn", "p5_test_stand:base", "p5_test_stand:ls_wave_spawner"]);

    // Place a creature spawn: click Place, then the map
    await needs.locator("li", { hasText: "Creature spawn" }).getByRole("button", { name: "Place" }).click();
    const r = (await page.locator(".ed-pane").first().boundingBox())!;
    await page.mouse.click(r.x + r.width / 2, r.y + r.height * 0.75);
    await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).entities.filter((e: { classname: string }) => e.classname === "ls_marker").length).toBe(1);
    const m = (await api(page, "/api/tools/editor/doc")).entities.find((e: { classname: string }) => e.classname === "ls_marker");
    const keys = Object.fromEntries(m.keys);
    expect([keys.kind, keys.mode, keys.wave, keys.side]).toEqual(["creature_spawn", "p5_test_stand", "1", "north"]);
    await expect(needs).toContainText("(has 1)");
    // its form comes from the kit: a typed Side list, and a wrong value is a check warning
    await expect(page.getByText("P5 Test Stand: Creature spawn.").first()).toBeVisible();
    // a mode the map is made for that isn't installed: a plain note (and an old plain id still finds its kit)
    const doc0 = await api(page, "/api/tools/editor/doc");
    const otherKeys = (doc0.world.keys as [string, string][]).filter(([k]) => k !== "ls_modes");
    await api(page, "/api/tools/editor/cmd", { op: "set_world", keys: [...otherKeys, ["ls_modes", "5e0c1f7a-3b2d-4c11-9a8e-1f2e3d4c5b6a:p5_test_stand,11111111-2222-4333-8444-555555555555:zombie_escape"]] });
    await expect(page.getByRole("note").filter({ hasText: "“zombie_escape”, which isn't installed here" })).toBeVisible();
    await api(page, "/api/tools/editor/cmd", { op: "set_world", keys: [...otherKeys, ["ls_modes", "p5_test_stand"]] });
    await expect(page.getByRole("group", { name: "Made for" }).getByRole("button", { name: "P5 Test Stand" })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("note")).toHaveCount(0);
    await page.getByLabel("Side", { exact: true }).selectOption("south");
    await expect.poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((e: { id: number }) => e.id === m.id).keys).side).toBe("south");
    await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: m.id, key: "wave", value: "500" });
    await expect.poll(async () => (await api(page, "/api/tools/editor/checks")).issues.some((i: { code: string; message: string }) => i.code === "kit-key" && i.message.includes("from 1 to 99"))).toBe(true);
    // an area marker is drawn as its box (the overlay mesh) and gets the kit's size
    await page.getByRole("group", { name: "P5 Test Stand kit" }).getByRole("button", { name: "Place Base" }).click();
    await page.mouse.click(r.x + r.width * 0.4, r.y + r.height * 0.75);
    await expect.poll(async () => (await api(page, "/api/tools/editor/doc")).entities.filter((e: { classname: string; keys: [string, string][] }) => e.classname === "ls_marker" && e.keys.some(([k, v]) => k === "size" && v === "256 256 128")).length).toBe(1);
  } finally {
    fs.rmSync(path.join(folder, "p5_test_stand.kit.json"), { force: true });
    fs.rmSync(path.join(folder, "p5_broken.kit.json"), { force: true });
  }
});

test("ls_creature's Type lists every creature type; a wave spawner picks them", async ({ page }) => {
  await open(page, false);
  const c = await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "ls_creature", origin: [0, 0, 16] });
  const w = await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "ls_wave_spawner", origin: [64, 0, 16] });
  expect(c.created?.entities?.length && w.created?.entities?.length).toBeTruthy();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  const creatures = await api(page, "/api/tools/creatures");
  const names: string[] = [...new Set([...(creatures.templates ?? []), ...(creatures.types ?? [])].map((t: { name: string }) => t.name))];
  expect(names.length).toBeGreaterThan(3);
  await page.getByRole("button", { name: "Outline", exact: true }).or(page.getByRole("tab", { name: "Outline" })).first().click();
  await page.getByLabel(/Find in the map|Find by name/).first().fill("ls_creature");
  await page.locator(".ed-ol-row", { hasText: "ls_creature" }).first().click();
  const type = page.locator(`#editor-f-${c.created.entities[0]}-type`);
  await expect(type).toBeVisible();
  for (const n of names) await expect(type.locator(`option[value="${n}"]`)).toHaveCount(1);
  await expect(page.getByText("Its model")).toBeVisible();
  // the wave spawner: creature types as toggles, written to its types key
  await page.getByLabel(/Find in the map|Find by name/).first().fill("ls_wave_spawner");
  await page.locator(".ed-ol-row", { hasText: "ls_wave_spawner" }).first().click();
  const chips = page.getByRole("group", { name: "Creature types" });
  await chips.getByRole("button").nth(1).click();
  await chips.getByRole("button").nth(0).click();
  await expect
    .poll(async () => Object.fromEntries((await api(page, "/api/tools/editor/doc")).entities.find((e: { id: number }) => e.id === w.created.entities[0]).keys).types)
    .toBe(`${names[1]} ${names[0]}`);
});
