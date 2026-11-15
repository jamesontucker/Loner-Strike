// Community maps and the map editor (Parity A): a compile into a Community map's name is refused
// with "Save as <name>_edit" and "Remix it"; a remix whose original was updated says so in the
// project info; after a Final compile, "Teach bots and send to my servers" runs its steps (all
// mocked here: the long steps answer at once) with labels, Cancel and a summary.
//   npx playwright test editor-community-maps --project=large [--project=webkit with LS_WEBKIT_ALL=1]
import { expect, test, type Page, type Route } from "@playwright/test";

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

const dock = (page: Page, name: string) => page.locator("[data-dock-strip] [role=tab]", { hasText: new RegExp(`^${name}$`) }).first().click();

const STATUS = { installed: true, version: "1.3.1", bytes: 1, url: "", licence: "", folder: "", canRun: true, cantRun: null, suggestName: "de_lonely", busy: false };

test("a Community map's name: refused with Save as <name>_edit and Remix it", async ({ page }) => {
  const starts: { name?: string; install?: boolean }[] = [];
  const remixes: unknown[] = [];
  await page.route(/\/api\/tools\/compile\/status/, (r) => r.fulfill({ json: STATUS }));
  await page.route(/\/api\/tools\/compile\/start/, (r: Route) => {
    const b = r.request().postDataJSON();
    starts.push(b);
    if ((b.name ?? "de_lonely") === "de_lonely")
      return r.fulfill({ json: { error: "de_lonely came from Community. Its updates would replace your changes.", community: { map: "de_lonely", name: "Lonely", author: "jam", model: "Mod", gb: 4242, editName: "de_lonely_edit", remixable: true } } });
    return r.fulfill({ json: { error: "(stopped here by the test)" } });
  });
  await page.route(/\/api\/tools\/community\/remix-map/, (r) => {
    remixes.push(r.request().postDataJSON());
    return r.fulfill({ json: { error: "(remix asked: stopped here by the test)" } });
  });
  await open(page);
  await dock(page, "Compile");
  await page.locator(".compile-playtest").getByRole("button", { name: "Playtest", exact: true }).click();
  const box = page.getByRole("alertdialog");
  await expect(box).toContainText("de_lonely came from Community.");
  await expect(box).toContainText("Its updates would replace your changes.");
  await expect(box.getByRole("button", { name: "Save as de_lonely_edit" })).toBeVisible();
  // Remix it: the Community remix, with the page's id (credit comes from there)
  await box.getByRole("button", { name: "Remix it" }).click();
  await expect.poll(() => remixes.length).toBe(1);
  expect(remixes[0]).toMatchObject({ id: 4242 });
  // Save as: the name set, then the compile again into the game under it
  await box.getByRole("button", { name: "Save as de_lonely_edit" }).click();
  await expect.poll(() => starts.length).toBe(2);
  expect(starts[1]).toMatchObject({ name: "de_lonely_edit", install: true });
  await expect(page.locator(".compile-panel").getByRole("textbox", { name: /^Map name/ })).toHaveValue("de_lonely_edit");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

test("a remix whose original was updated says so in the project info", async ({ page }) => {
  await page.route(/\/api\/tools\/editor\/project$/, async (r) => {
    const real = await (await r.fetch()).json();
    real.project = { ...(real.project ?? {}), project: "/qa/de_lonely_remix.lsmap", name: "de_lonely_remix", map: "de_lonely_remix", folder: "/qa", game: false };
    return r.fulfill({ json: real });
  });
  await page.route(/\/api\/tools\/community\/remix-updates/, (r) =>
    r.fulfill({ json: { updates: [{ project: "/qa/de_lonely_remix.lsmap", map: "de_lonely_remix", original: "Lonely", author: "jam", gb: 4242, url: "https://gamebanana.com/mods/4242" }] } }),
  );
  await open(page);
  await dock(page, "Project");
  const note = page.getByTestId("remix-updated");
  await expect(note).toContainText("The original was updated.");
  await expect(note.getByRole("link", { name: "See what changed." })).toHaveAttribute("href", "https://gamebanana.com/mods/4242");
});

/** A finished Final compile of qa_final, and every step after it answered by the test. */
async function finalCompiled(page: Page, opts: { holdLearning?: boolean } = {}) {
  const calls: string[] = [];
  let jobPolls = 0;
  await page.route(/\/api\/tools\/compile\/status/, (r) => r.fulfill({ json: { ...STATUS, busy: true } }));
  await page.route(/\/api\/tools\/compile\/progress/, (r) =>
    r.fulfill({ json: { job: { id: 1, kind: "compile", running: false, stage: "done", label: "Done", fraction: 1, seconds: 42, problems: [], result: { map: "qa_final", bsp: "/x/qa_final.bsp", bytes: 1000, preset: "final", installed: false }, error: null, stopped: false }, lines: [], next: 0, partial: "" } }),
  );
  const note = (name: string) => (r: Route) => {
    calls.push(name);
    return r.fulfill({ json: { ok: true } });
  };
  await page.route(/\/api\/tools\/compile\/install/, note("install"));
  await page.route(/\/api\/tools\/doctor\/qa_final\/res/, note("res"));
  await page.route(/\/api\/navs\/learn\/stop/, note("learn-stop"));
  await page.route(/\/api\/mapcopy\/stop/, note("copy-stop"));
  await page.route(/\/api\/navs\/learn$/, (r) => {
    if (r.request().method() === "POST") return note("learn")(r);
    return r.fulfill({ json: { result: { message: "Bots learned qa_final.", learned: ["qa_final"] } } });
  });
  await page.route(/\/api\/setup\/job/, (r) => {
    jobPolls++;
    const hold = opts.holdLearning || jobPolls % 3 !== 0;
    return r.fulfill({ json: { state: hold ? "running" : "done", label: "Working" } });
  });
  await page.route(/\/api\/mapcopy\/servers/, (r) =>
    r.fulfill({ json: { servers: [{ id: "a", name: "Alone Together", game: "cstrike" }, { id: "b", name: "Box", game: "cstrike", has: true }, { id: "c", name: "Far Away", game: "cstrike" }, { id: "d", name: "CZ only", game: "czero", missing: true }] } }),
  );
  await page.route(/\/api\/mapcopy$/, (r) => {
    if (r.request().method() === "POST") {
      calls.push(`copy:${JSON.stringify(r.request().postDataJSON())}`);
      return r.fulfill({ json: { ok: true, job: "j" } });
    }
    return r.fulfill({ json: { result: { message: "Copied qa_final to Alone Together and Box.", copied: 2, failed: [{ map: "qa_final", server: "Far Away", why: "couldn't reach Far Away" }] } } });
  });
  await open(page);
  await dock(page, "Compile");
  return calls;
}

test("after a Final compile: Teach bots and send to my servers, step by step, with a summary", async ({ page }) => {
  const calls = await finalCompiled(page);
  const go = page.getByRole("button", { name: "Teach bots and send to my servers" });
  await expect(go).toBeVisible({ timeout: 15_000 });
  await go.click();
  await expect(page.locator(".compile-sendout")).toContainText("Teaching bots…");
  await expect(page.locator(".compile-sendout")).toContainText("Sending to 3 servers…", { timeout: 15_000 });
  const summary = page.locator(".compile-sendout-summary");
  await expect(summary).toContainText("Bots learned qa_final.", { timeout: 15_000 });
  await expect(summary).toContainText("Map knowledge: the bots build it on each server");
  await expect(summary).toContainText("Copied qa_final to Alone Together and Box.");
  await expect(summary).toContainText("Far Away: couldn't reach Far Away.");
  // in order; the copy brings your own map up to date where this app put it before
  expect(calls.slice(0, 3)).toEqual(["install", "res", "learn"]);
  expect(calls.find((c) => c.startsWith("copy:"))).toBe(`copy:${JSON.stringify({ maps: ["qa_final"], servers: ["a", "b", "c"], update: ["qa_final"] })}`);
});

test("Teach bots and send to my servers can be cancelled", async ({ page }) => {
  const calls = await finalCompiled(page, { holdLearning: true });
  await page.getByRole("button", { name: "Teach bots and send to my servers" }).click({ timeout: 15_000 });
  await expect(page.locator(".compile-sendout")).toContainText("Teaching bots…");
  await page.locator(".compile-sendout").getByRole("button", { name: "Cancel" }).click();
  await expect.poll(() => calls.includes("learn-stop")).toBe(true);
  await expect(page.locator(".compile-sendout-summary")).toContainText("Teaching bots was stopped.", { timeout: 20_000 });
  expect(calls.some((c) => c.startsWith("copy:")), "nothing sent after a cancel").toBe(false);
});

test("Save as <name>_edit is kept in the project: opened again, it compiles to the new name", async ({ page }) => {
  let starts = 0;
  await page.route(/\/api\/tools\/compile\/start/, (r: Route) => {
    starts++;
    if (starts === 1)
      return r.fulfill({ json: { error: "ls_newmap came from Community. Its updates would replace your changes.", community: { map: "ls_newmap", name: "New", author: "jam", model: "Mod", gb: 4243, editName: "ls_newmap_edit", remixable: false } } });
    return r.fulfill({ json: { error: "(stopped here by the test)" } });
  });
  await open(page);
  await dock(page, "Compile");
  const nameBox = page.locator(".compile-panel").getByRole("textbox", { name: /^Map name/ });
  await expect(nameBox).toHaveValue("ls_newmap");
  await page.locator(".compile-playtest").getByRole("button", { name: "Playtest", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "Save as ls_newmap_edit" }).click();
  await expect.poll(() => starts).toBe(2);
  // the project holds it (and counts as changed until saved)
  const doc = await page.evaluate(async () => (await fetch("/api/tools/editor/doc")).json());
  expect(doc.editor.mapName).toBe("ls_newmap_edit");
  expect(doc.dirty).toBe(true);
  // opened again: the compile name is the new one, and no refusal box
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await dock(page, "Compile");
  await expect(page.locator(".compile-panel").getByRole("textbox", { name: /^Map name/ })).toHaveValue("ls_newmap_edit");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});
