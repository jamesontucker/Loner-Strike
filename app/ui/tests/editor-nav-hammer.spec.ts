// Map editor, Hammer navigation: holding the left mouse on empty 3D space hands WASD to the
// camera (like J.A.C.K.), held still or dragging, and W/A/S/D don't pick tools meanwhile.
// Z mouse-look still works. (npm run test:tools -- tests/editor-nav-hammer.spec.ts)
import { expect, test, type Page } from "@playwright/test";

async function openEditor(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "hammer", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator(".ed-layouts button").nth(0).click();
  await page.waitForTimeout(2500);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const ed = () => (window as any).__lsEditor.getState();
const picked = (page: Page) =>
  page.evaluate(`(() => { const s = (${ed.toString()})(); return { b: [...s.sel.brushes], e: [...s.sel.entities] }; })()`) as Promise<{ b: number[]; e: number[] }>;

async function pane(page: Page) {
  const r = (await page.locator(".ed-pane").first().boundingBox())!;
  const shot = () => page.screenshot({ clip: { x: r.x, y: r.y, width: r.width, height: r.height } });
  return { r, shot, mid: { x: r.x + r.width / 2, y: r.y + r.height * 0.75 } };
}

test("Hammer: hold left mouse still + W flies, and no tool is picked", async ({ page }) => {
  await openEditor(page);
  const { shot, mid } = await pane(page);
  const before = await shot();
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down();
  await page.keyboard.down("w");
  await page.waitForTimeout(600);
  await page.keyboard.up("w");
  await page.keyboard.down("d");
  await page.waitForTimeout(300);
  await page.keyboard.up("d");
  await page.mouse.up();
  await page.waitForTimeout(300);
  expect(Buffer.compare(before, await shot()), "the view moved").not.toBe(0);
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
  // letting go after flying doesn't also pick what was under the mouse
  expect(await picked(page)).toEqual({ b: [], e: [] });
});

test("Hammer: left-drag look + W flies; after letting go W is a shortcut again", async ({ page }) => {
  await openEditor(page);
  const { shot, mid } = await pane(page);
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down();
  await page.mouse.move(mid.x + 30, mid.y, { steps: 4 });
  const turned = await shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(600);
  await page.keyboard.up("w");
  expect(Buffer.compare(turned, await shot()), "W moved the camera during the drag").not.toBe(0);
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
  await page.mouse.up();
  await page.waitForTimeout(200);
  expect(await picked(page)).toEqual({ b: [], e: [] });
  // the button is up: W is the Move tool again
  await page.keyboard.press("w");
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Move");
  await page.keyboard.press("q");
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
});

test("Hammer: a click still picks, and dragging a picked face still pushes it (no camera)", async ({ page }) => {
  await openEditor(page);
  const { shot, mid } = await pane(page);
  await page.mouse.click(mid.x, mid.y);
  await expect.poll(() => picked(page)).not.toEqual({ b: [], e: [] });
  const id = (await picked(page)).b[0];
  const bounds = () => page.evaluate(`(() => { const b = (${ed.toString()})().doc.brushes.find((x) => x.id === ${id}); return JSON.stringify([b.min, b.max]); })()`);
  const was = await bounds();
  await page.waitForTimeout(300);
  const before = await shot();
  // push/pull: the tool takes the drag (the camera doesn't turn)
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down();
  await page.mouse.move(mid.x, mid.y - 60, { steps: 6 });
  await page.mouse.up();
  await expect.poll(bounds).not.toBe(was);
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
  // undo the push, then the view is where it was (the camera didn't move)
  await page.keyboard.press(process.platform === "darwin" ? "Meta+z" : "Control+z");
  await expect.poll(bounds).toBe(was);
  await page.waitForTimeout(400);
  expect(Buffer.compare(before, await shot()), "the camera stayed put").toBe(0);
});

test("Hammer: Z mouse-look + W still flies", async ({ page }) => {
  await openEditor(page);
  const { shot, mid } = await pane(page);
  await page.mouse.move(mid.x, mid.y);
  const before = await shot();
  await page.keyboard.press("z");
  await page.keyboard.down("w");
  await page.waitForTimeout(600);
  await page.keyboard.up("w");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  expect(Buffer.compare(before, await shot()), "the view moved").not.toBe(0);
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
  await expect(page.locator(".ed-pane").first()).toBeVisible();
  await page.screenshot({ path: "test-results/hammer-hint.png" });
});

// Windows can let go of the mouse mid-flight (seen in a tester's video): Z look keeps flying, says
// how to get the mouse back, and the next move key takes it back. Z and Esc still stop it. (With
// no move key down a lost mouse stops the look: editor-nav-audit.spec.ts.)
const nav = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).__lsNav.getState();
    return { looking: s.looking, released: s.released, locked: !!document.pointerLockElement };
  });

test("Hammer: Z look survives the system letting go of the mouse", async ({ page }) => {
  // test browsers never grant a real pointer lock: a stand-in that behaves like one (change events)
  await page.addInitScript(() => {
    let el: Element | null = null;
    Object.defineProperty(Document.prototype, "pointerLockElement", { get: () => el, configurable: true });
    const change = () => setTimeout(() => document.dispatchEvent(new Event("pointerlockchange")), 0);
    Element.prototype.requestPointerLock = function (this: Element) {
      el = this;
      change();
      return Promise.resolve();
    } as typeof Element.prototype.requestPointerLock;
    Document.prototype.exitPointerLock = function () {
      if (!el) return;
      el = null;
      change();
    };
  });
  await openEditor(page);
  const { shot, mid } = await pane(page);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: false, locked: true });
  // the system drops the lock mid-flight (not Z, not Esc)
  await page.keyboard.down("w");
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: true, locked: false });
  await page.keyboard.up("w");
  await expect(page.locator(".ed-nav-hud")).toContainText("click or press a move key");
  await page.screenshot({ path: test.info().outputPath("released-hint.png") });
  // still flying, and W takes the mouse back
  const before = await shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: false, locked: true });
  expect(Buffer.compare(before, await shot()), "W still flew").not.toBe(0);
  await expect(page.locator(".editor-tool[aria-pressed='true']")).toContainText("Select");
  // dropped again: a click takes it back without picking anything
  await page.keyboard.down("d");
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toMatchObject({ released: true });
  await page.keyboard.up("d");
  await page.mouse.click(mid.x, mid.y);
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: false, locked: true });
  expect(await picked(page)).toEqual({ b: [], e: [] });
  // Esc still stops
  await page.keyboard.press("Escape");
  await expect.poll(() => nav(page)).toEqual({ looking: false, released: false, locked: false });
  // and Z after a drop stops too
  await page.keyboard.press("z");
  await page.keyboard.down("s");
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toMatchObject({ released: true });
  await page.keyboard.up("s");
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toEqual({ looking: false, released: false, locked: false });
});
