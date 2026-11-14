// Game Mode Studio: the Playtest button is there, enabled on a clean graph, and pressing it
// shows each step with its time (Saving…, Compiling…) instead of a silent wait. The game itself
// isn't started here: the launch is answered by a stub.
import { expect, test } from "@playwright/test";

// (nothing is installed in a game: compile and playtest are answered in the page; the dev server
// also refuses to write into a real Half-Life install under test. Anything left is removed after.)
test.afterAll(async ({ playwright }, info) => {
  const api = await playwright.request.newContext({ baseURL: String(info.project.use.baseURL) });
  for (const short of ["pw_playtest", "pw_playtest_2", "pw_playtest_3"]) await api.post(`/api/tools/studio/uninstall/${short}`, { headers: { "X-Panel": "1" } }).catch(() => {});
  await api.post("/api/tools/studio/graph/pw_playtest/delete", { headers: { "X-Panel": "1" } }).catch(() => {});
  await api.dispose();
});

test("Playtest button: shown, enabled, steps with time", async ({ page }) => {
  await page.route("**/api/tools/studio/compile", (r) => r.fulfill({ json: { ok: true, problems: [] } }));
  // a graph of our own to open (made from the example, saved through the app)
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = "pw_playtest";
  tpl.mode.name = "Playtest check";
  const saved = await page.request.post("/api/tools/studio/graph/pw_playtest", { data: { graph: tpl }, headers: { "X-Panel": "1" } });
  expect((await saved.json()).ok).toBeTruthy();
  // compiling and launching are answered here: this checks the page, not the game
  await page.route("**/api/tools/studio/playtest", async (r) => {
    await new Promise((res) => setTimeout(res, 1500));
    await r.fulfill({ json: { ok: true, problems: [], output: "Done." } });
  });
  await page.route("**/api/state", (r) => r.fulfill({ json: { game_open: false } }));
  await page.route("**/api/app/launch", (r) => r.fulfill({ json: { ok: true } }));
  await page.goto("/tools-dev.html?open=studio:pw_playtest");
  const button = page.getByTestId("studio-playtest");
  await expect(button).toBeVisible({ timeout: 20_000 });
  await expect(button).toBeEnabled();
  await button.click();
  await expect(page.getByTestId("studio-step").filter({ hasText: "Compiling" })).toBeVisible();
  await expect(page.getByText(/Waiting for the game… \d+ s/)).toBeVisible({ timeout: 10_000 });
  await page.request.post("/api/tools/studio/graph/pw_playtest/delete", { headers: { "X-Panel": "1" } });
});
