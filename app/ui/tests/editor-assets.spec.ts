// Map editor: a map whose entity names a sprite that isn't on the computer (like de_kobbl's
// sprites/de_kobbl/blin.spr, which made the server stop at "Mod_LoadModel: … not found"): the
// live checks and the Project panel say so, and Playtest refuses with a plain message.
// Runs on editor-dev.html against the tools dev server (npm run test:tools -- tests/editor-assets.spec.ts).
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

const SPRITE = "sprites/p5_test/not_here.spr";

test("a missing sprite: live checks, Project panel, and Playtest refuses", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  const r = await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "env_sprite", origin: [0, 0, 64], keys: [["model", SPRITE]] });
  expect(r.error ?? null, JSON.stringify(r).slice(0, 300)).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();

  // the route
  const a = await api(page, "/api/tools/editor/assets");
  expect(a.missing).toBe(1);
  expect(a.assets.find((x: { path: string }) => x.path === SPRITE)?.status).toBe("missing");
  // the live checks
  const checks = await api(page, "/api/tools/editor/checks");
  const issue = checks.issues.find((i: { code: string }) => i.code === "missing-file");
  expect(issue?.message).toContain(SPRITE);
  expect(issue?.level).toBe("error");
  await page.getByRole("button", { name: "Checks", exact: true }).or(page.getByRole("tab", { name: "Checks" })).first().click();
  await expect(page.getByText(`${SPRITE} isn't on this computer`).first()).toBeVisible();
  // the Project panel
  await page.getByRole("button", { name: "Project", exact: true }).or(page.getByRole("tab", { name: "Project" })).first().click();
  await expect(page.getByRole("group", { name: "The map's models, sprites and sounds" })).toContainText("isn't on this computer");
  await expect(page.locator(".ed-project-file", { hasText: SPRITE })).toContainText("Missing");
  // Playtest (compile and put in the game) refuses before anything else. (It can't reach the real
  // game anyway: under test the compile, map install and .res routes refuse a real Half-Life
  // install, studio::guard_real_game; a spec that really installs needs the dev server's
  // LS_TEST_GAME on a copy, e.g. a sim server's folder.)
  const pt = await api(page, "/api/tools/compile/start", { preset: "fast", install: true });
  expect(pt.error).toContain("Playtest stopped");
  expect(pt.error).toContain(SPRITE);
});
