// Map editor (P5, perf audit #1d, #10): edits patch the page's copy of the document from the
// command's own changes (no whole /doc per edit), changes made from outside (an agent, another
// window) arrive through the long-poll within moments, and an idle page doesn't poll.
// P5 also runs it in WebKit.
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
  await page.waitForTimeout(1500);
}

const things = (page: Page) => page.locator(".ed-ol-foot").first();

test("edits patch the document; outside changes arrive by themselves; idle means quiet", async ({ page }) => {
  await open(page);
  const doc = await api(page, "/api/tools/editor/doc");
  const urls: string[] = [];
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (u.pathname.startsWith("/api/tools/editor/")) urls.push(u.pathname + u.search);
  });
  // five nudges from the page (Ctrl/⌘ arrows move the pick a grid step): no whole document asked for
  const b = doc.brushes[0].id as number;
  await page.locator(".ed-pane").first().click({ position: { x: 5, y: 5 } }).catch(() => {});
  for (let i = 0; i < 5; i++) {
    await page.evaluate(async (id) => {
      const s = (window as unknown as { __lsEditor?: { getState: () => { cmd: (b: Record<string, unknown>) => Promise<unknown> } } }).__lsEditor;
      if (s) await s.getState().cmd({ op: "transform", brushes: [id], entities: [], translate: [16, 0, 0], lock: true });
    }, b);
  }
  const usedStore = await page.evaluate(() => !!(window as unknown as { __lsEditor?: unknown }).__lsEditor);
  test.skip(!usedStore, "the page's store isn't exposed in this build");
  await expect.poll(() => urls.filter((u) => u.startsWith("/api/tools/editor/cmd")).length).toBe(5);
  expect(urls.filter((u) => u === "/api/tools/editor/doc"), urls.join("\n")).toEqual([]);
  // the page's copy follows: the brush's bounds moved 80 units
  const moved = await page.evaluate((id) => {
    const s = (window as unknown as { __lsEditor: { getState: () => { doc: { brushes: { id: number; min: number[] }[] } } } }).__lsEditor;
    return s.getState().doc.brushes.find((x) => x.id === id)!.min[0];
  }, b);
  expect(moved).toBe(doc.brushes[0].min[0] + 80);
  // a change from outside (as an agent makes it): it shows without a reload
  const before = await things(page).innerText();
  const n = Number(before.match(/(\d+) things/)![1]);
  await api(page, "/api/tools/editor/cmd", { op: "add_entity", classname: "light", origin: [0, 0, 64] });
  await expect(things(page)).toContainText(`${n + 1} things`, { timeout: 4000 });
  // (the checks run once, 1.2 s after that change)
  await expect.poll(() => urls.filter((u) => u.startsWith("/api/tools/editor/checks")).length, { timeout: 5000 }).toBeGreaterThan(0);
  await page.waitForTimeout(1500);
  // idle for 6 s: only waiting asks (state?wait= and MCP's selection?after=, each answers after 25 s)
  const at = urls.length;
  await page.waitForTimeout(6000);
  const waiting = (u: string) => u.startsWith("/api/tools/editor/state?wait=") || u.startsWith("/api/tools/editor/selection?after=");
  expect(urls.slice(at).filter((u) => !waiting(u)), urls.slice(at).join("\n")).toEqual([]);
  expect(urls.slice(at).length, urls.slice(at).join("\n")).toBeLessThanOrEqual(2);
});
