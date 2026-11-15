// Asset pickers bring files in (T1fix): Creature Studio's model picker and Game Mode Studio's
// sprite input get Import… (the system dialog), a file dropped on them, a taken name asking for
// another, "From your map projects" (copied into the game, then picked), a refresh button and a
// rescan when the window comes back. The assets routes (tools/assets.rs) are faked here; the rest
// runs against a dev server over a scratch game copy (never the real one).
import { expect, test, type Page, type Route } from "@playwright/test";

const H = { "X-Panel": "1" };
const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });

/** The faked assets routes: what was asked, and how many times each list was read. */
async function fakeAssets(page: Page, kind: string, path: string) {
  const calls: { url: string; body: Record<string, unknown> }[] = [];
  const reads = { project: 0 };
  let taken = true;
  await page.route("**/api/tools/assets/**", async (r) => {
    const url = r.request().url();
    const body = r.request().method() === "POST" ? (r.request().postDataJSON() as Record<string, unknown>) : {};
    calls.push({ url, body });
    if (url.includes("/project-files")) {
      reads.project++;
      return json(r, { files: [{ project: "ls_lab", path: path.replace(/(\.\w+)$/, "_lab$1"), name: "x", size: 10 }] });
    }
    if (url.includes("/adopt")) return json(r, { ok: true, path: path.replace(/(\.\w+)$/, "_lab$1"), files: [] });
    if (url.includes("/import")) {
      // the first dropped file's name is taken: it asks for another
      if (body.data && taken && !body.rename) {
        taken = false;
        return json(r, { taken: { name: path.replace(/^.*\//, ""), suggestion: path.replace(/^.*\//, "").replace(/(\.\w+)$/, "_2$1") } });
      }
      const name = (body.rename as string) || path.replace(/^.*\//, "");
      return json(r, { ok: true, path: path.replace(/[^/]*$/, name), files: [], credit: { verdict: "ok", authors: [{ name: "Ann" }] } });
    }
    return json(r, { ok: true });
  });
  return { calls, reads };
}

async function drop(page: Page, target: string, name: string) {
  const dt = await page.evaluateHandle((n) => {
    const d = new DataTransfer();
    d.items.add(new File([new Uint8Array([73, 68, 83, 84, 0, 0, 0, 0])], n));
    return d;
  }, name);
  await page.dispatchEvent(target, "dragover", { dataTransfer: dt });
  await page.dispatchEvent(target, "drop", { dataTransfer: dt });
}

test("Creature Studio's model picker: Import…, a dropped file with a taken name, your map projects, refresh", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const { calls, reads } = await fakeAssets(page, "model", "models/qa_new.mdl");
  await page.goto("/tools-dev.html?open=creatures");
  const list = page.getByRole("listbox", { name: "Creature types" });
  await list.getByRole("option", { name: /Zombie/ }).click();
  await page.locator(".cs-model-btn").click();
  const pop = page.getByRole("dialog", { name: "Pick a model" });
  await expect(pop).toContainText("Files you add here go into your game, so creatures and modes can use them. Sharing packages them for you.");

  // Import… through the system dialog: picked at once, with its credit
  await pop.getByRole("button", { name: "Import…" }).click();
  await expect(page.locator(".cs-model-btn")).toContainText("qa_new.mdl");
  await expect(page.locator(".ai-credit")).toContainText("OK to share");
  expect(calls.find((c) => c.url.endsWith("/import"))!.body).toEqual({ kind: "model", pick: true });

  // a file dropped on the picker: its name is taken, so it asks for another, then picks it
  await drop(page, ".cs-model", "qa_new.mdl");
  const ask = page.getByRole("alertdialog");
  await expect(ask).toContainText("There's already a qa_new.mdl in your game");
  await ask.getByRole("textbox").fill("qa_new");
  await ask.getByRole("button", { name: "Use this name" }).click();
  await expect(ask).toContainText("is taken");
  await ask.getByRole("textbox").fill("qa_newer");
  await ask.getByRole("button", { name: "Use this name" }).click();
  await expect(page.locator(".cs-model-btn")).toContainText("qa_newer.mdl");
  expect(calls.filter((c) => c.url.endsWith("/import")).at(-1)!.body).toMatchObject({ kind: "model", name: "qa_new.mdl", rename: "qa_newer.mdl" });

  // from a map project: copied into the game, then picked
  await page.locator(".cs-model-btn").click();
  const group = pop.getByRole("group", { name: "From your map projects" });
  await group.getByRole("button", { name: /qa_new_lab\.mdl/ }).click();
  await expect(page.locator(".cs-model-btn")).toContainText("qa_new_lab.mdl");
  expect(calls.find((c) => c.url.includes("/adopt"))!.body).toEqual({ kind: "model", project: "ls_lab", path: "models/qa_new_lab.mdl" });

  // the refresh button and the window coming back both look again
  const models = () => page.waitForRequest((r) => r.url().includes("/api/tools/creatures/models"));
  await page.locator(".cs-model-btn").click();
  let next = models();
  await pop.getByRole("button", { name: "Refresh the list" }).click();
  await next;
  next = models();
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await next;
  expect(reads.project).toBeGreaterThan(0);
  await page.getByRole("button", { name: "Undo changes" }).click();
  expect(problems).toEqual([]);
});

// (its own id each run: a run that stopped halfway leaves its graph behind)
const ID = `pw_assets_${Date.now() % 100000}`;
test("Game Mode Studio's sprite input: Import…, a dropped file, your map projects", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  // (the left panel stays closed in a small window: the canvas keeps room for the graph; the
  // right one shows the picked node's inputs)
  const roomy = (page.viewportSize()?.width ?? 1440) >= 1280;
  await page.addInitScript((open) => {
    localStorage.setItem("ls.studio.panels", JSON.stringify({ left: open, right: true, minimap: false }));
    localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false }));
  }, roomy);
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "Asset check";
  tpl.nodes.push({ id: "mk", type: "fx.marker", pos: { x: 0, y: -260 }, params: { sprite: "sprites/arrow1.spr" } });
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H })).json()).ok).toBeTruthy();
  const { calls } = await fakeAssets(page, "sprite", "sprites/qa_mark.spr");
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  // (picked the way the Problems list does it: it selects the node and brings it into view, at
  // any window size; the canvas only draws what's in view)
  await page.locator(".react-flow__node").first().waitFor({ timeout: 20_000 });
  await page.getByRole("button", { name: "Objective Marker", exact: true }).first().click();
  const field = page.locator(".ai-field").first();
  await expect(field.locator("input").first()).toHaveValue("sprites/arrow1.spr");
  await field.getByRole("button", { name: "Choose…" }).click();
  const pop = page.getByRole("dialog", { name: "Choose a sprite" });
  await expect(pop).toContainText("Files you add here go into your game");
  await pop.getByRole("button", { name: "Import…" }).click();
  await expect(field.locator("input").first()).toHaveValue("sprites/qa_mark.spr");
  await expect(field.locator(".ai-credit")).toContainText("by Ann");
  // dropped on the field: taken, renamed
  await drop(page, ".ai-field", "qa_mark.spr");
  const ask = page.getByRole("alertdialog");
  await ask.getByRole("textbox").fill("qa_mark2");
  await ask.getByRole("button", { name: "Use this name" }).click();
  await expect(field.locator("input").first()).toHaveValue("sprites/qa_mark2.spr");
  // from a map project
  await field.getByRole("button", { name: "Choose…" }).click();
  await pop.getByRole("group", { name: "From your map projects" }).getByRole("button", { name: /qa_mark_lab\.spr/ }).click();
  await expect(field.locator("input").first()).toHaveValue("sprites/qa_mark_lab.spr");
  expect(calls.filter((c) => /\/(import|adopt)$/.test(c.url)).map((c) => c.body.kind)).toEqual(["sprite", "sprite", "sprite", "sprite"]);
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H }).catch(() => undefined);
  expect(problems).toEqual([]);
});
