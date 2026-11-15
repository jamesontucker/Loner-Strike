// Map editor: the entity I/O view (P5, Hammer parity item 6). A deathrun trap: a button fires a
// multi_manager (with a master), which fires a door and a hurt twice (delays) and a name nothing
// has. The I/O panel lists it all with kinds and delays, flags the broken name with Remove,
// filters to broken and to the picked entity's chain, picks from the list; the app's
// /api/tools/editor/io agrees, and the Checks panel counts the broken link. P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
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
  const add = async (classname: string, origin: number[], keys: [string, string][]) => (await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname, origin, keys })).created.entities[0] as number;
  const ids = {
    button: await add("trigger_relay", [0, 0, 64], [["targetname", "trap1_btn"], ["target", "trap1_mm"], ["master", "trap_lock"]]),
    mm: await add("multi_manager", [64, 0, 64], [["targetname", "trap1_mm"], ["trap1_door", "0"], ["trap1_hurt", "0.5"], ["trap1_hurt#1", "3"], ["trap1_sound", "0"]]),
    door: await add("env_sprite", [128, 0, 64], [["targetname", "trap1_door"]]),
    hurt: await add("env_explosion", [192, 0, 64], [["targetname", "trap1_hurt"], ["killtarget", "trap1_door"]]),
    lock: await add("multisource", [0, 64, 64], [["targetname", "trap_lock"]]),
  };
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^I\/O$/ }).first().click();
  return { ids, panel: page.getByRole("region", { name: "What triggers what" }) };
}

test("the trap chain: kinds, delays, the broken name, filters, picking, Remove", async ({ page }) => {
  const { ids, panel } = await open(page);
  const group = (name: string) => panel.locator(".p5-io-group").filter({ has: page.locator(".p5-io-from", { hasText: name }) });
  await expect(panel).toBeVisible();
  // 1 (button target) + 1 (master) + 4 (manager names) + 1 (killtarget)
  await expect(panel).toContainText("7 links");
  await expect(panel).toContainText("1 broken");
  await expect(panel.locator(".p5-io-link.broken")).toContainText("“trap1_sound”: nothing has this name");
  await expect(group("multi_manager “trap1_mm”")).toContainText("after 3 s");
  await expect(group("trigger_relay “trap1_btn”")).toContainText("waits on");
  await expect(group("env_explosion “trap1_hurt”")).toContainText("kills");
  // the app agrees (the same rules), and Checks counts the broken one
  const io = await api(page, "/api/tools/editor/io");
  expect(io.counts).toMatchObject({ links: 7, broken: 1 });
  const checks = await api(page, "/api/tools/editor/checks");
  expect(checks.issues.filter((i: { code: string }) => i.code === "dangling-target")).toHaveLength(1);
  // Broken only
  await panel.getByRole("button", { name: "Broken", exact: true }).click();
  await expect(panel.locator(".p5-io-link")).toHaveCount(1);
  await panel.getByRole("button", { name: "All", exact: true }).click();
  // pick the button from the list: its chain (manager, door, hurt, lock) is lit; the chain filter works
  await panel.getByRole("button", { name: "trigger_relay “trap1_btn”", exact: true }).click();
  await expect(panel.locator(".p5-io-group.picked")).toContainText("trap1_btn");
  await panel.getByRole("button", { name: "Picked chain" }).click();
  await expect(panel.locator(".p5-io-group.hot")).toHaveCount(3); // button, manager, hurt (the door and the lock fire nothing)
  const chain = await api(page, `/api/tools/editor/io?entity=${ids.button}`);
  expect(new Set(chain.chain.down)).toEqual(new Set([ids.mm, ids.door, ids.hurt, ids.lock]));
  await panel.getByRole("button", { name: "All", exact: true }).click();
  // a target in the list picks it
  await panel.locator(".p5-io-link", { hasText: "env_sprite" }).first().getByRole("button", { name: /env_sprite/ }).click();
  await expect(panel.locator(".p5-io-group.picked")).toHaveCount(0);
  // search
  await panel.getByRole("searchbox", { name: "Find in the links" }).fill("explosion");
  await expect(panel.locator(".p5-io-group")).toHaveCount(2); // the manager fires it; it kills the door
  await panel.getByRole("searchbox", { name: "Find in the links" }).fill("");
  // Remove the broken name: undoable, gone from the manager
  await panel.locator(".p5-io-link.broken").getByRole("button", { name: "Remove" }).click();
  await expect(panel).toContainText("none broken");
  const mm = (await api(page, "/api/tools/editor/doc")).entities.find((e: { id: number }) => e.id === ids.mm);
  expect(mm.keys.some(([k]: [string]) => k === "trap1_sound")).toBe(false);
});

test("an empty map says how links work", async ({ page }) => {
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^I\/O$/ }).first().click();
  await expect(page.getByRole("region", { name: "What triggers what" })).toContainText("Nothing triggers anything yet");
});
