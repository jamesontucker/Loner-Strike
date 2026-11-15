// Map editor at rest (QA4 area A, "idle traffic"): with br_valley open and nobody touching it,
// the page sends at most a few requests a minute (long polls counted). Counts every request the
// page makes over 60 s after it settles. Reads maps-src/br_valley/br_valley.map (or LS_TEST_BRVALLEY).
import { expect, test, type Page } from "@playwright/test";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const MAP = process.env.LS_TEST_BRVALLEY ?? resolve(process.cwd(), "../../maps-src/br_valley/br_valley.map");

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );

test("br_valley open and idle: a few requests a minute at most", async ({ page }) => {
  test.skip(!existsSync(MAP), `needs ${MAP}`);
  test.setTimeout(300_000);
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 4 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "quad", views: ["3d", "top", "front", "side"] }));
  });
  await page.goto("/editor-dev.html");
  expect((await api(page, "/api/tools/editor/new", {})).error ?? null).toBeNull();
  expect((await api(page, "/api/tools/editor/cmd", { op: "paste", text: readFileSync(MAP, "utf8"), offset: [0, 0, 0] })).error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  await back.waitFor({ timeout: 30_000 });
  await back.click();
  await page.waitForFunction(() => /\d{4} brushes/.test(document.querySelector(".tools-status")?.textContent || ""), null, { timeout: 120_000 });
  // let loading, the first checks and thumbnails finish
  await page.waitForTimeout(15_000);
  const seen: string[] = [];
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (u.pathname.startsWith("/api/") || u.pathname.startsWith("/thumb")) seen.push(`${r.method()} ${u.pathname}`);
  });
  await page.waitForTimeout(60_000);
  const by = new Map<string, number>();
  for (const s of seen) by.set(s, (by.get(s) ?? 0) + 1);
  const list = [...by.entries()].map(([k, n]) => `${n}x ${k}`).join(", ") || "none";
  test.info().annotations.push({ type: "idle", description: `${seen.length} requests in 60 s: ${list}` });
  console.log(`IDLE ${seen.length} requests in 60 s: ${list}`);
  expect(seen.length, list).toBeLessThanOrEqual(3);
});
