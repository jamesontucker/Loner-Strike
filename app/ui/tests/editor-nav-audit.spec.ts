// Map editor fixes from the 1.0.2 camera and 2D audits: a redraw doesn't stop flying, a second
// button during a look doesn't leave the camera stuck, Blender fly keys don't also pick tools,
// Z look turns without pointer lock, Esc stops in one press, Frame all stays within the 2D zoom
// limits, sky brushes can be picked in 2D; then (second audit) text boxes while the mouse is free,
// two 3D panes, dialogs, a leftover fly setting, layout changes keeping 2D views, Wireframe
// entities, and a press right after picking the Box tool. (npm run test:tools -- tests/editor-nav-audit.spec.ts)
import { expect, test, type Page } from "@playwright/test";

type Preset = "hammer" | "unreal" | "blender";

/** Test browsers never grant a real pointer lock: a stand-in that behaves like one, or says no. */
async function fakeLock(page: Page, deny = false) {
  await page.addInitScript((deny) => {
    let el: Element | null = null;
    Object.defineProperty(Document.prototype, "pointerLockElement", { get: () => el, configurable: true });
    const fire = (name: string) => setTimeout(() => document.dispatchEvent(new Event(name)), 0);
    Element.prototype.requestPointerLock = function (this: Element) {
      if (deny) {
        fire("pointerlockerror");
        return Promise.reject(new DOMException("denied", "NotSupportedError"));
      }
      el = this;
      fire("pointerlockchange");
      return Promise.resolve();
    } as typeof Element.prototype.requestPointerLock;
    Document.prototype.exitPointerLock = function () {
      if (!el) return;
      el = null;
      fire("pointerlockchange");
    };
  }, deny);
}

async function openEditor(page: Page, preset: Preset, layout: "single" | "quad" | "two", camera = "orbit") {
  await page.addInitScript(
    ([preset, camera]) => {
      localStorage.setItem("ls.editor.nav", JSON.stringify({ preset, speed: 3 }));
      localStorage.setItem("ls.editor.navHint", "1");
      localStorage.setItem("ls.tools.camera", camera);
    },
    [preset, camera],
  );
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator(".ed-layouts button").nth({ single: 0, quad: 1, two: 3 }[layout]).click();
  await page.waitForTimeout(2500);
}

const nav = (page: Page) =>
  page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const s = (window as any).__lsNav.getState();
    return { looking: s.looking, released: s.released, locked: !!document.pointerLockElement };
  });
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const cmd = (page: Page, body: object) => page.evaluate((b) => (window as any).__lsEditor.getState().cmd(b), body);
const picked = (page: Page) =>
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  page.evaluate(() => [...(window as any).__lsEditor.getState().sel.brushes] as number[]);
const tool = (page: Page) => page.locator(".editor-tool[aria-pressed='true']");

async function pane(page: Page, i: number) {
  const r = (await page.locator(".ed-pane").nth(i).boundingBox())!;
  return { r, shot: () => page.screenshot({ clip: r }), mid: { x: r.x + r.width / 2, y: r.y + r.height * 0.75 } };
}

test("Hammer: Z look keeps going through editor redraws (an edit, a resize)", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "single");
  const { mid, shot } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: false, locked: true });
  // an edit (the map, the unsaved marker, the outliner all redraw)
  await cmd(page, { op: "add_box", min: [-64, -64, 256], max: [64, 64, 320], texture: "CRATE01" });
  await page.waitForTimeout(500);
  expect(await nav(page)).toEqual({ looking: true, released: false, locked: true });
  // the window changes size
  const vp = page.viewportSize()!;
  await page.setViewportSize({ width: vp.width - 120, height: vp.height - 60 });
  await page.waitForTimeout(500);
  expect(await nav(page)).toEqual({ looking: true, released: false, locked: true });
  const before = await shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  expect(Buffer.compare(before, await shot()), "W still flew").not.toBe(0);
  await expect(tool(page)).toContainText("Select");
  await page.keyboard.press("Escape");
});

test("Hammer: losing the mouse with no move key down stops (Chromium's Esc); with W down it keeps flying", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "single");
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toMatchObject({ looking: true, locked: true });
  // the browser keeps the Esc and only lets go of the mouse
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toEqual({ looking: false, released: false, locked: false });
  // W afterwards is the Move tool again
  await page.keyboard.press("w");
  await expect(tool(page)).toContainText("Move");
  await page.keyboard.press("q");
  // mid-flight it keeps flying (the editor-nav-hammer test covers getting the mouse back)
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toMatchObject({ looking: true, locked: true });
  await page.keyboard.down("w");
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: true, locked: false });
  await page.keyboard.up("w");
  await page.keyboard.press("Escape");
  await expect.poll(() => nav(page)).toMatchObject({ looking: false });
});

test("Hammer: Z look turns with the mouse when the system won't hide it", async ({ page }) => {
  await fakeLock(page, true);
  await openEditor(page, "hammer", "single");
  const { mid, shot } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toMatchObject({ looking: true, locked: false });
  await page.waitForTimeout(300);
  const before = await shot();
  await page.mouse.move(mid.x + 120, mid.y - 40, { steps: 8 });
  await page.waitForTimeout(300);
  expect(Buffer.compare(before, await shot()), "the mouse turned the view").not.toBe(0);
  await page.keyboard.press("Escape");
  await expect.poll(() => nav(page)).toMatchObject({ looking: false });
});

test("Unreal: a second button during a right-drag look doesn't leave the camera stuck", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "unreal", "single");
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(mid.x + 20, mid.y, { steps: 3 });
  await expect.poll(() => nav(page)).toMatchObject({ looking: true });
  await page.mouse.down({ button: "left" });
  await page.mouse.up({ button: "left" });
  await page.mouse.up({ button: "right" });
  await expect.poll(() => nav(page)).toEqual({ looking: false, released: false, locked: false });
  await page.keyboard.press("w");
  await expect(tool(page)).toContainText("Move");
});

test("Blender fly, Quad with a 2D view active: W picks the Move tool and doesn't also fly", async ({ page }) => {
  await openEditor(page, "blender", "quad", "fly");
  const top = await pane(page, 1);
  const three = await pane(page, 0);
  // make the Top view the active one (a click on empty space there)
  await page.mouse.click(top.r.x + 12, top.r.y + top.r.height - 12);
  await expect(page.locator(".ed-pane").nth(1)).toHaveClass(/active/);
  await page.waitForTimeout(600);
  const before = await three.shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  await page.waitForTimeout(600);
  await expect(tool(page)).toContainText("Move");
  expect(Buffer.compare(before, await three.shot()), "the 3D view stayed put").toBe(0);
});

test("2D views: Frame all on a big map stays within the zoom limits (the next wheel notch doesn't jump)", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  await cmd(page, { op: "add_box", min: [-16000, -16000, -64], max: [16000, 16000, 0], texture: "CRATE01" });
  await page.waitForTimeout(800);
  const top = await pane(page, 1);
  await page.mouse.move(top.r.x + top.r.width / 2, top.r.y + top.r.height / 2);
  await page.keyboard.press("Shift+z");
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const zoom = () => page.evaluate(() => (window as any).__lsFlatCams.top.zoom as number);
  await page.waitForTimeout(600);
  const z1 = await zoom();
  // whole map in a quad pane: well under the first fit's zoom
  expect(z1).toBeLessThan(0.05);
  await page.mouse.wheel(0, 120);
  await page.waitForTimeout(400);
  const z2 = await zoom();
  expect(z2, "a notch out zooms out a little").toBeLessThan(z1);
  expect(z2).toBeGreaterThan(z1 * 0.6);
  await page.mouse.wheel(0, -120);
  await page.waitForTimeout(400);
  expect(await zoom()).toBeGreaterThan(z2);
});

test("2D views: a sky brush is picked where it's the only brush; a floor under it in Top still wins", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  const floor = (await cmd(page, { op: "add_box", min: [-512, -512, -64], max: [512, 512, 0], texture: "CRATE01" })) as { created: { brushes: number[] } };
  const sky = (await cmd(page, { op: "add_box", min: [-512, -512, 512], max: [512, 512, 544], texture: "sky" })) as { created: { brushes: number[] } };
  await page.waitForTimeout(800);
  // frame everything, then click the ceiling's middle in Front, and the map's middle in Top
  const front = await pane(page, 2);
  await page.mouse.move(front.r.x + front.r.width / 2, front.r.y + front.r.height / 2);
  await page.keyboard.press("Shift+z");
  await page.waitForTimeout(600);
  const at = (view: string, p: number[]) =>
    page.evaluate(
      ([view, p]) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const cam = (window as any).__lsFlatCams[view as string];
        const v = cam.position.clone().set(p[0], p[2], -p[1]).project(cam);
        return [(v.x + 1) / 2, (1 - v.y) / 2];
      },
      [view, p] as const,
    );
  const [fx, fy] = await at("front", [0, 0, 528]);
  await page.mouse.click(front.r.x + fx * front.r.width, front.r.y + fy * front.r.height);
  await expect.poll(() => picked(page)).toEqual(sky.created.brushes);
  const top = await pane(page, 1);
  const [tx, ty] = await at("top", [100, 100, 0]);
  await page.mouse.click(top.r.x + tx * top.r.width, top.r.y + ty * top.r.height);
  // (a floor under the ceiling: mine or the new map's own room)
  await expect.poll(async () => (await picked(page)).length).toBe(1);
  expect(await picked(page)).not.toEqual(sky.created.brushes);
  void floor;
  await page.screenshot({ path: test.info().outputPath("sky-2d.png") });
});

// ---- second audit ----

const search = (page: Page) => page.getByPlaceholder("Find a name, type or texture");

test("Hammer: with the mouse free (no lock, or let go mid-flight) a text box gets its keys", async ({ page }) => {
  await fakeLock(page, true);
  await openEditor(page, "hammer", "single");
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toMatchObject({ looking: true, locked: false });
  // a click outside the view ends the look; typing then goes to the box
  await search(page).click();
  await expect.poll(() => nav(page)).toMatchObject({ looking: false });
  await page.keyboard.type("wasd");
  await expect(search(page)).toHaveValue("wasd");
  await expect(tool(page)).toContainText("Select");
});

test("Hammer: released mid-flight, then a click in a text box: it types", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "single");
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await page.keyboard.down("w");
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: true, locked: false });
  await page.keyboard.up("w");
  await search(page).click();
  await page.keyboard.type("wasd");
  await expect(search(page)).toHaveValue("wasd");
  expect(await nav(page)).toEqual({ looking: false, released: false, locked: false });
});

test("Hammer: Z doesn't start a look while a dialog is open; a hold clears held", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "single");
  await page.getByRole("button", { name: "Map properties…" }).click();
  await expect(page.getByRole("dialog", { name: "Map properties" })).toBeVisible();
  await page.keyboard.press("z");
  await page.waitForTimeout(300);
  expect(await nav(page)).toMatchObject({ looking: false, locked: false });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Map properties" })).toHaveCount(0);
  // a left-hold flight, then letting go: held is off again
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down();
  await page.keyboard.down("w");
  await page.waitForTimeout(200);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const held = () => page.evaluate(() => (window as any).__lsNav.getState().held as boolean);
  expect(await held()).toBe(true);
  await page.keyboard.up("w");
  await page.mouse.up();
  await expect.poll(held).toBe(false);
});

test("Two 3D panes: Hammer Z and Unreal right-mouse fly the pane you're in, not the first", async ({ page }) => {
  await fakeLock(page);
  await page.addInitScript(() => localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "two", views: ["3d", "3d", "front", "side"] })));
  await openEditor(page, "hammer", "two");
  const a = await pane(page, 0);
  const b = await pane(page, 1);
  await page.mouse.move(b.mid.x, b.mid.y);
  await page.keyboard.press("z");
  await page.waitForTimeout(300);
  expect(await nav(page)).toEqual({ looking: true, released: false, locked: true });
  expect(await page.evaluate(() => (document.pointerLockElement as HTMLElement | null)?.dataset.pane)).toBe("1");
  const a0 = await a.shot();
  const b0 = await b.shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  await page.waitForTimeout(300);
  expect(Buffer.compare(b0, await b.shot()), "the right pane flew").not.toBe(0);
  expect(Buffer.compare(a0, await a.shot()), "the left pane stayed").toBe(0);
  await page.keyboard.press("Escape");
  await expect.poll(() => nav(page)).toMatchObject({ looking: false });
});

test("Two 3D panes, Unreal: right-mouse + W flies the pane it's in", async ({ page }) => {
  await fakeLock(page);
  await page.addInitScript(() => localStorage.setItem("ls.editor.panes", JSON.stringify({ layout: "two", views: ["3d", "3d", "front", "side"] })));
  await openEditor(page, "unreal", "two");
  const a = await pane(page, 0);
  const b = await pane(page, 1);
  await page.mouse.move(b.mid.x, b.mid.y);
  await page.mouse.down({ button: "right" });
  await page.waitForTimeout(400); // (the press makes the right pane the active one: its frame changes)
  const a0 = await a.shot();
  const b0 = await b.shot();
  await page.keyboard.down("w");
  await page.waitForTimeout(500);
  await page.keyboard.up("w");
  await page.mouse.up({ button: "right" });
  await page.waitForTimeout(300);
  expect(Buffer.compare(b0, await b.shot()), "the right pane flew").not.toBe(0);
  expect(Buffer.compare(a0, await a.shot()), "the left pane stayed").toBe(0);
});

test("Hammer with a fly setting left over from Blender: clicks pick and W picks Move", async ({ page }) => {
  await openEditor(page, "hammer", "single", "fly");
  const { mid } = await pane(page, 0);
  await page.mouse.click(mid.x, mid.y);
  await expect.poll(async () => (await picked(page)).length).toBeGreaterThan(0);
  await page.keyboard.press("w");
  await expect(tool(page)).toContainText("Move");
});

test("Layouts: a 2D view keeps its zoom and place through Quad > Single > Quad", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  const front = await pane(page, 2);
  await page.mouse.move(front.r.x + front.r.width * 0.3, front.r.y + front.r.height * 0.4);
  for (let i = 0; i < 4; i++) {
    await page.mouse.wheel(0, -120);
    await page.waitForTimeout(80);
  }
  await page.waitForTimeout(300);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const pose = () => page.evaluate(() => { const c = (window as any).__lsFlatCams.front; return [c.zoom, c.position.x, c.position.y].map((x: number) => Math.round(x * 1000) / 1000); });
  const was = await pose();
  await page.locator(".ed-layouts button").nth(0).click();
  await page.waitForTimeout(800);
  await page.locator(".ed-layouts button").nth(1).click();
  await page.waitForTimeout(1500);
  expect(await pose()).toEqual(was);
});

test("Box tool: a press right after picking it draws a box (not a box-select)", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const count = () => page.evaluate(() => (window as any).__lsEditor.getState().doc.brushes.length as number);
  const top = await pane(page, 1);
  // something picked (a plain press would box-select and drop it)
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await page.evaluate(() => { const s = (window as any).__lsEditor.getState(); s.select({ brushes: [s.doc.brushes[0].id], entities: [], face: null }); });
  await expect.poll(async () => (await picked(page)).length).toBeGreaterThan(0);
  const n = await count();
  const x = top.r.x + top.r.width * 0.15;
  const y = top.r.y + top.r.height * 0.2;
  await page.locator(".editor-tool", { hasText: "Box" }).click();
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 60, y + 40, { steps: 4 });
  await page.mouse.up();
  await expect.poll(count).toBe(n + 1);
});

test("Looks: Wireframe draws entity models as lines; sky outlines show in a Textured 2D view", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  await cmd(page, { op: "add_box", min: [-512, -512, 512], max: [512, 512, 544], texture: "sky" });
  await page.getByLabel("How pane 1 draws").selectOption("wire");
  await page.getByLabel("How pane 3 draws").selectOption("textured");
  await page.waitForTimeout(1500);
  await page.screenshot({ path: test.info().outputPath("looks.png") });
});

// ---- final audit ----

test("Blender: F switches to the fly camera and back without breaking the editor", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await openEditor(page, "blender", "quad");
  const three = await pane(page, 0);
  await page.mouse.click(three.r.x + 10, three.r.y + three.r.height - 10);
  await page.keyboard.press("f");
  await page.waitForTimeout(500);
  await page.keyboard.press("f");
  await page.waitForTimeout(500);
  expect(errors).toEqual([]);
  await expect(page.locator(".ed-pane")).toHaveCount(4);
});

test("Hammer: Esc with W held stops the look (W's auto-repeat doesn't grab the mouse back)", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "single");
  const { mid } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.keyboard.press("z");
  await page.keyboard.down("w");
  // Chromium: Esc only lets go of the mouse
  await page.evaluate(() => document.exitPointerLock());
  await expect.poll(() => nav(page)).toMatchObject({ released: true, locked: false });
  await page.keyboard.down("w"); // (a repeat)
  await page.keyboard.down("w");
  await page.waitForTimeout(200);
  expect(await nav(page)).toMatchObject({ locked: false });
  await page.keyboard.up("w");
});

test("Unreal: leaving the window mid-look doesn't leave a drag behind", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "unreal", "single");
  const { mid, shot } = await pane(page, 0);
  await page.mouse.move(mid.x, mid.y);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(mid.x + 10, mid.y, { steps: 2 });
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await page.waitForTimeout(300);
  expect(await nav(page)).toMatchObject({ looking: false });
  const before = await shot();
  await page.mouse.move(mid.x + 120, mid.y + 30, { steps: 6 });
  await page.waitForTimeout(300);
  expect(Buffer.compare(before, await shot()), "the free mouse doesn't turn the view").toBe(0);
  await page.mouse.up({ button: "right" });
  // a new right-hold looks again
  await page.mouse.down({ button: "right" });
  await expect.poll(() => nav(page)).toMatchObject({ looking: true });
  await page.mouse.up({ button: "right" });
});

test("Hammer, Quad with a 2D pane active: Z over the 3D pane starts the look", async ({ page }) => {
  await fakeLock(page);
  await openEditor(page, "hammer", "quad");
  const top = await pane(page, 1);
  await page.mouse.click(top.r.x + 12, top.r.y + top.r.height - 12);
  await expect(page.locator(".ed-pane").nth(1)).toHaveClass(/active/);
  const three = await pane(page, 0);
  await page.mouse.move(three.mid.x, three.mid.y);
  await page.keyboard.press("z");
  await expect.poll(() => nav(page)).toEqual({ looking: true, released: false, locked: true });
  await page.keyboard.press("Escape");
  await expect.poll(() => nav(page)).toMatchObject({ looking: false });
});

test("Clip tool: a click right after picking it keeps its first point", async ({ page }) => {
  await openEditor(page, "blender", "quad");
  const top = await pane(page, 1);
  await page.mouse.click(top.r.x + 12, top.r.y + top.r.height - 12); // (Top is the active pane)
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await page.evaluate(() => { const s = (window as any).__lsEditor.getState(); s.select({ brushes: [s.doc.brushes[0].id], entities: [], face: null }); });
  await page.locator(".editor-tool", { hasText: "Clip" }).click();
  await page.mouse.click(top.r.x + top.r.width * 0.3, top.r.y + top.r.height * 0.5);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await expect.poll(() => page.evaluate(() => (window as any).__lsEditor.getState().clipInfo?.points ?? 0)).toBe(1);
});
