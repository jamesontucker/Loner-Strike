// P5 (QA4-A): a file action clicked while another runs (Import during "Starting a new map…")
// waits and then runs, with a word about it, instead of doing nothing. Chromium and WebKit.
import { expect, test } from "@playwright/test";

test("Import while a new map is starting: it waits, says so, then imports", async ({ page }) => {
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
  // a slow new map, and an import that answers "cancelled" (no file dialog in a test)
  const order: string[] = [];
  await page.route("**/api/tools/editor/new", async (r) => {
    order.push("new:start");
    await new Promise((x) => setTimeout(x, 1500));
    const res = await r.fetch();
    order.push("new:done");
    await r.fulfill({ response: res });
  });
  await page.route("**/api/tools/editor/import-map", async (r) => {
    order.push("import");
    await r.fulfill({ json: { cancelled: true } });
  });
  await page.evaluate(() => {
    const st = (window as unknown as { __lsEditor: { getState: () => { file: (w: string) => Promise<void> } } }).__lsEditor.getState();
    void st.file("new");
    setTimeout(() => void st.file("import"), 300);
  });
  await expect(page.getByText(/Starting a new map first\. Importing right after\./)).toBeVisible();
  await expect.poll(() => order.join(","), { timeout: 10_000 }).toBe("new:start,new:done,import");
});

