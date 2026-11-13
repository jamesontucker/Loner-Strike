// Map editor UX (P5 polish pass): buttons show their keys from the key set in use, slow edits
// show "Working…" with a spinner, visibility groups are "Visgroups" with a header and labelled
// columns. Runs on editor-dev.html against the tools dev server; P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";

async function open(page: Page, preset = "loner") {
  await page.addInitScript((p) => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.keys", JSON.stringify({ preset: p, mine: {} }));
  }, preset);
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.waitForTimeout(1500);
}

const MOD = process.platform === "darwin" ? "⌘" : "Ctrl+";

async function pickABrush(page: Page) {
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  await page.mouse.click(r.x + r.width / 2, r.y + r.height * 0.85); // the floor
  await expect(page.locator(".ed-selsize")).toContainText("1 picked");
}

test("buttons show their keys from the key set in use", async ({ page }) => {
  await open(page);
  await pickABrush(page);
  const hollow = page.getByRole("button", { name: "Hollow" });
  await hollow.hover();
  await expect(hollow).toHaveAttribute("title", new RegExp(`\\(${MOD.replace("+", "\\+")}H\\)$`));
  const del = page.getByRole("button", { name: "Delete", exact: true });
  await del.hover();
  await expect(del).toHaveAttribute("title", /\((Delete|⌦|Backspace)/);
  const top = page.locator('.seg[aria-label="View"] button', { hasText: /^Top$/ });
  await top.hover();
  await expect(top).toHaveAttribute("title", /^Top view \(/);
  const play = page.getByRole("button", { name: "Playtest", exact: true });
  await play.hover();
  await expect(play).toHaveAttribute("title", /\(F5\)$/);
});

test("with Hammer keys, the tool rail shows Hammer's", async ({ page }) => {
  await open(page, "hammer");
  const sel = page.locator(".editor-tool", { hasText: "Select" }).first();
  await sel.hover();
  await expect(sel).toHaveAttribute("title", /\((⇧|Shift\+)S\)$/);
});

test("a slow edit shows Working… with a spinner", async ({ page }) => {
  await open(page);
  await pickABrush(page);
  await page.route("**/api/tools/editor/cmd", async (route) => {
    await new Promise((r) => setTimeout(r, 1200));
    await route.continue();
  });
  await page.getByRole("button", { name: "Hollow" }).click();
  const busy = page.locator(".tools-status [role=status]", { hasText: "Working…" });
  await expect(busy).toBeVisible();
  await expect(busy.locator(".spinner")).toHaveCount(1);
  await expect(busy).toHaveCount(0, { timeout: 8000 });
  await page.unroute("**/api/tools/editor/cmd");
});

test("Visgroups: a header, labelled columns, a named row", async ({ page }) => {
  await open(page);
  await page.getByRole("tab", { name: "Outline" }).or(page.getByRole("button", { name: "Outline", exact: true })).first().click();
  const vis = page.getByRole("group", { name: "Visgroups" });
  await expect(vis.locator("strong", { hasText: "Visgroups" })).toBeVisible();
  await pickABrush(page);
  await vis.getByRole("button", { name: "New visgroup" }).click();
  await expect(vis.getByLabel("Visgroup name")).toHaveValue("Visgroup 1");
  await expect(vis.locator(".editor-vis-cols")).toContainText("Shown");
  await expect(vis.locator(".editor-vis-cols")).toContainText("Name");
  await vis.getByLabel("Show visgroup Visgroup 1").uncheck();
  await expect(vis).toContainText("1 hidden");
  await vis.getByRole("button", { name: "Remove visgroup Visgroup 1" }).click();
  await expect(vis.getByLabel("Visgroup name")).toHaveCount(0);
});
