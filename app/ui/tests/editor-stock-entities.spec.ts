// Map editor: the stock entities checked against the game DLL (EngineCoverage item 9). The new
// point entities are in the Entity tool's picker with their help, the new brush entities in
// Tie to entity's list, the inspector shows their keys' help, a func_tank's warnings and a
// path_track's dangling stop reach the Checks route, and the track and turret previews draw.
// Runs in Chromium and WebKit (`--project webkit` picks it up from webkit's testMatch, or
// LS_WEBKIT_ALL=1).
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

const POINT = ["trigger_random", "trigger_random_time", "trigger_random_unique", "trigger_setorigin", "env_global", "game_score", "game_counter", "game_counter_set", "game_team_master", "game_team_set", "game_player_team", "game_player_hurt", "path_track", "item_healthkit", "item_battery", "item_longjump", "ammo_9mm"];
const SOLID = ["game_zone_player", "momentary_door", "momentary_rot_button", "button_target", "func_tracktrain", "func_traincontrols", "func_trackchange", "func_platrot", "func_guntarget", "func_tank", "func_tankmortar", "func_tanklaser", "func_tankcontrols", "func_healthcharger", "func_recharge"];

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

test("the new stock entities are in the pickers with their help", async ({ page }) => {
  await open(page);
  // the definitions route has them all, each with help
  const fgd = await api(page, "/api/tools/editor/fgd");
  const byName = new Map((fgd.classes as { name: string; kind: string; description: string }[]).map((c) => [c.name, c]));
  for (const n of [...POINT, ...SOLID]) {
    expect(byName.get(n)?.description?.length ?? 0, n).toBeGreaterThan(20);
  }
  expect(byName.get("game_end")!.description).toMatch(/^Warning: ends the map/);
  expect(byName.has("func_tankrocket")).toBe(false);
  // the Entity tool's picker lists the point ones with their help
  await page.locator('[data-cmd="tool.entity"]').click();
  const options = page.locator("#editor-point-classes option");
  for (const n of POINT) await expect(options.and(page.locator(`[value="${n}"]`)), n).toHaveCount(1);
  expect(await page.locator('#editor-point-classes option[value="trigger_random"]').textContent()).toContain("Sets off one of its targets at random");
  const place = page.getByLabel("Entity to place");
  await place.fill("trigger_setorigin");
  await expect(page.locator(".editor-option-note")).toContainText("Moves entities to another entity");
  // a brush picked: Tie to entity lists the brush ones, with their help as the tooltip
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [0, 0, 0], max: [64, 64, 64], texture: "OUT_WALL5" });
  await page.evaluate(`window.__lsEditor.getState().select({ brushes: [${r.created.brushes[0]}], entities: [], face: null })`);
  await page.locator('[data-cmd="tool.select"]').click();
  const tie = page.getByLabel("Entity type");
  await expect(tie).toBeVisible();
  for (const n of SOLID) await expect(tie.locator(`option[value="${n}"]`), n).toHaveCount(1);
  await expect(tie.locator('option[value="func_tank"]')).toHaveAttribute("title", /gun turret/);
});

test("the inspector, the checks and the previews for a turret and a track", async ({ page }) => {
  await open(page);
  const add = async (classname: string, origin: number[], keys: [string, string][]) => (await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname, origin, keys })).created.entities[0] as number;
  const t1 = await add("path_track", [0, 0, 16], [["targetname", "t1"], ["target", "t2"]]);
  await add("path_track", [256, 0, 16], [["targetname", "t2"], ["target", "nowhere"]]);
  const rnd = await add("trigger_random", [0, 128, 32], [["targetname", "r"], ["target_count", "2"], ["target0", "t1"]]);
  // a turret from a brush: tied, then turned toward north with a 45 degree aim either way
  const b = await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-64, -64, 0], max: [-32, -48, 16], texture: "OUT_WALL5" });
  const tie = await api(page, "/api/tools/editor/cmd", { op: "tie", brushes: b.created.brushes, classname: "func_tank" });
  const tank = tie.created.entities[0] as number;
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: tank, key: "yawrange", value: "45" });
  await api(page, "/api/tools/editor/cmd", { op: "set_key", entity: tank, key: "angles", value: "0 90 0" });
  // the checks: the turret never turns or fires, a stop leads nowhere, a random pick is empty
  const checks = (await api(page, "/api/tools/editor/checks")).issues as { code: string; entity?: number; message: string }[];
  const codes = (id: number) => checks.filter((i) => i.entity === id).map((i) => i.code);
  expect(codes(tank)).toEqual(expect.arrayContaining(["tank-no-turn", "tank-no-bullets", "needs-origin"]));
  expect(codes(rnd)).toContain("random-empty-target");
  expect(checks.some((i) => i.code === "dangling-target" && i.message.includes("nowhere"))).toBe(true);
  expect(codes(t1)).not.toContain("empty-brush-entity");
  expect(codes(rnd)).not.toContain("empty-brush-entity");
  // the inspector shows the keys' help
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.evaluate(`window.__lsEditor.getState().select({ brushes: [], entities: [${rnd}], face: null })`);
  const panel = page.locator(".tools-panel", { has: page.locator("h3", { hasText: /^trigger_random$/ }) });
  await expect(panel).toContainText("Sets off one of its targets at random");
  await expect(panel.locator('label[title^="target_count: Picks among the first"]')).toHaveCount(1);
  await expect(panel.getByText("Target 16", { exact: true })).toHaveCount(1);
  // the previews: the track's arrow and the turret's arc are drawn
  const segs = (await page.evaluate(`(async () => {
    const m = await import("/src/views/tools/editor/StockPreviews.tsx");
    const s = window.__lsEditor.getState();
    return m.stockSegments(s.doc.entities, s.brushes).map((x) => [x.a, x.b, x.c]);
  })()`)) as number[][][];
  // t1 -> t2 with a head (3 lines), and the turret: the middle (3) + the arc + its two edges
  const track = segs.filter(([, , c]) => c[0] === 1 && c[2] < 0.2 && c[1] > 0.5);
  expect(track.length).toBeGreaterThanOrEqual(3);
  const aim = segs.filter(([, , c]) => c[0] === 1 && c[1] < 0.3);
  expect(aim.length).toBeGreaterThan(8);
  // the middle of its aim points north (yaw 90)
  const mid = aim[0];
  expect(Math.abs(mid[1][0] - mid[0][0])).toBeLessThan(1);
  expect(mid[1][1] - mid[0][1]).toBeGreaterThan(100);
});
