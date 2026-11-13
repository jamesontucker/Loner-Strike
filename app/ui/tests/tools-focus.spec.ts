// Focus mode on every tool page: the way out ("Exit focus") is in a bar of its own above the
// tool and never sits on top of a tool's buttons or fields (it used to float in the bottom-right
// corner over toolbars: "Install", "Enter"). Runs on tools-dev.html against the tools dev server
// (npm run test:tools -- tests/tools-focus.spec.ts); P5 also runs it in WebKit.
import { expect, test } from "@playwright/test";
import * as fs from "node:fs";
import * as path from "node:path";

const reg = fs.readFileSync(path.join(__dirname, "../src/views/tools/registry.tsx"), "utf8");
const ids = [...reg.matchAll(/id: "([a-z0-9-]+)", label/g)].map((m) => m[1]);

test("Exit focus never covers a control, on every tool", async ({ page }) => {
  test.setTimeout(240_000);
  expect(ids.length).toBeGreaterThan(10);
  const bad: string[] = [];
  for (const id of ids) {
    await page.goto(`/tools-dev.html?open=${id}`);
    const focusBtn = page.locator(".tools-focus-btn");
    if (!(await focusBtn.waitFor({ timeout: 8000 }).then(() => true, () => false))) continue; // (a page without focus mode)
    await page.waitForTimeout(800);
    await focusBtn.click();
    const bar = page.locator(".tools-focus-bar");
    await expect(bar, id).toBeVisible();
    await page.waitForTimeout(600);
    // anything clickable that overlaps the bar, other than the bar's own button
    const hits = await page.evaluate(() => {
      const b = document.querySelector(".tools-focus-bar")!.getBoundingClientRect();
      const out: string[] = [];
      for (const el of document.querySelectorAll<HTMLElement>("button, a[href], input, select, textarea, [role=button], [role=tab], canvas")) {
        if (el.closest(".tools-focus-bar")) continue;
        const r = el.getBoundingClientRect();
        if (r.width < 2 || r.height < 2 || getComputedStyle(el).visibility === "hidden") continue;
        if (r.left < b.right && r.right > b.left && r.top < b.bottom && r.bottom > b.top) out.push(`${el.tagName.toLowerCase()} “${(el.textContent || el.getAttribute("aria-label") || "").trim().slice(0, 30)}”`);
      }
      // and the bar's button is the thing on top where it is
      const p = document.querySelector(".tools-focus-pill")!.getBoundingClientRect();
      const top = document.elementFromPoint(p.left + p.width / 2, p.top + p.height / 2);
      if (!top?.closest(".tools-focus-pill")) out.push(`covered by ${top?.tagName.toLowerCase()}.${(top as HTMLElement)?.className}`);
      return out;
    });
    if (hits.length) bad.push(`${id}: ${hits.join(", ")}`);
    await page.locator(".tools-focus-pill").click();
    await expect(bar, id).toHaveCount(0);
  }
  expect(bad, bad.join("\n")).toEqual([]);
});

test("the one-time Focus hint sits beside the button, over nothing", async ({ page }) => {
  for (const id of ["editor", "studio", "plugins", "creatures"]) {
    await page.goto(`/tools-dev.html?open=${id}`);
    await page.evaluate(() => localStorage.removeItem("ls.tools.focusHintSeen"));
    await page.waitForTimeout(300);
    await page.goto(`/tools-dev.html?open=${id}`);
    const hint = page.locator(".tools-focus-hint");
    if (!(await hint.waitFor({ timeout: 6000 }).then(() => true, () => false))) continue;
    await page.waitForTimeout(1200);
    const hits = await page.evaluate(() => {
      const b = document.querySelector(".tools-focus-hint")!.getBoundingClientRect();
      const out: string[] = [];
      for (const el of document.querySelectorAll<HTMLElement>("button, a[href], input, select, textarea, [role=button], [role=tab], canvas")) {
        if (el.closest(".tools-focus-hint")) continue;
        const r = el.getBoundingClientRect();
        if (r.width < 2 || r.height < 2) continue;
        if (r.left < b.right && r.right > b.left && r.top < b.bottom && r.bottom > b.top) out.push(`${el.tagName.toLowerCase()} “${(el.textContent || el.getAttribute("aria-label") || "").trim().slice(0, 30)}”`);
      }
      return out;
    });
    expect(hits, `${id}: ${hits.join(", ")}`).toEqual([]);
  }
});
