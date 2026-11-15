// Map editor: the face edit sheet like Hammer's (P5, parity item 2). Shift / scale / turn
// numbers, Justify and Fit, To the world / face / view, the Pick up eyedropper, and with the
// Texture tool in 3D: right-click paints a face, Alt+right-click paints it and carries the
// picked face's alignment onto it. Runs on editor-dev.html against the tools dev server.
// P5 also runs it in WebKit.
import { expect, test, type Page } from "@playwright/test";

const api = (page: Page, path: string, body?: unknown) =>
  page.evaluate(
    async ([p, b]) => (await fetch(p as string, b === undefined ? {} : { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(b) })).json(),
    [path, body] as const,
  );
const undoLabel = async (page: Page) => (await api(page, "/api/tools/editor/doc")).undo as string | null;

/** A floor (CRATE01) under the whole view and a box (ROCK) in the middle of it. */
async function open(page: Page, box = false) {
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
  await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-4096, -4096, -512], max: [4096, 4096, 64], texture: "CRATE01" });
  if (box) await api(page, "/api/tools/editor/cmd", { op: "add_box", min: [-1536, -1536, 64], max: [1536, 1536, 128], texture: "ROCK" });
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  if (await back.waitFor({ timeout: 5000 }).then(() => true, () => false)) await back.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator(".ed-layouts button").nth(0).click(); // Single
  await page.waitForTimeout(1500);
  await page.keyboard.press("t"); // Texture tool
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  return { mid: [r.x + r.width / 2, r.y + r.height / 2 + 40] as [number, number], near: [r.x + r.width / 2, r.y + r.height - 60] as [number, number] };
}

test("numbers, justify, fit and the three aligns", async ({ page }) => {
  const { mid } = await open(page);
  await page.mouse.click(...mid);
  const sheet = page.locator(".editor-facesheet");
  await expect(sheet).toBeVisible();
  await expect(sheet).toContainText("CRATE01");
  await expect(sheet).toContainText("up (+Z)");
  // shift across
  const shift = sheet.getByLabel("Shift across");
  await shift.fill("16");
  await shift.press("Enter");
  await expect.poll(() => undoLabel(page)).toBe("Align texture");
  await expect(shift).toHaveValue("16");
  // justify left and fit
  await sheet.getByRole("group", { name: "Justify" }).getByRole("button", { name: "Left" }).click();
  await expect(shift).not.toHaveValue("16");
  await sheet.getByLabel("Repeats across").fill("2");
  await sheet.getByRole("button", { name: "Fit repeats" }).click();
  await expect(sheet.getByLabel("Scale across")).not.toHaveValue("1");
  // to the world, the face, the view
  await sheet.getByRole("button", { name: "To the world" }).click();
  await expect(sheet).toContainText("World (and face");
  await sheet.getByRole("button", { name: "To the view" }).click();
  await expect.poll(() => undoLabel(page)).toBe("Align texture");
  await expect(sheet.getByLabel("Rotation (degrees)")).toHaveValue("0");
});

test("Pick up takes a face's texture; right-click paints; Alt+right-click carries the alignment", async ({ page }) => {
  const { mid, near } = await open(page, true);
  await page.mouse.click(...near); // the floor (CRATE01)
  const sheet = page.locator(".editor-facesheet");
  await expect(sheet).toContainText("CRATE01");
  // the current texture is something else; Pick up, then click the floor: it's CRATE01
  await page.mouse.click(...mid);
  await expect(sheet.locator(".editor-face-tex .mono")).toHaveText("ROCK");
  const pick = sheet.getByRole("button", { name: /Pick up|Click a face/ });
  await pick.click();
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await page.mouse.click(...near);
  await expect(pick).toHaveAttribute("aria-pressed", "false");
  await expect(sheet.getByRole("button", { name: /^Paint/ })).toContainText("CRATE01");
  // right-click the middle face: painted CRATE01
  await page.mouse.click(...mid, { button: "right" });
  await expect.poll(() => undoLabel(page)).toBe("Apply texture");
  // pick the floor and give it a shift; Alt+right-click the middle: texture and alignment carried
  await page.mouse.click(...near);
  const shift = sheet.getByLabel("Shift across");
  await shift.fill("24");
  await shift.press("Enter");
  await expect.poll(() => undoLabel(page)).toBe("Align texture");
  await page.keyboard.down("Alt");
  await page.mouse.click(...mid, { button: "right" });
  await page.keyboard.up("Alt");
  await expect.poll(() => undoLabel(page)).toBe("Apply texture and alignment");
  // the middle face now has the floor's shift (same plane direction: the texture runs on)
  await page.mouse.click(...mid);
  await expect(sheet.getByLabel("Shift across")).toHaveValue("24");
  // the hint says how
  await expect(sheet.locator(".p5-face-mouse")).toContainText("Alt+right-click");
});
