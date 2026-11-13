// Nav fixer: the things that make it usable up close (docs/TOOLS-DEV.md):
//   npx playwright test tests/navfix-ux.spec.ts --project large     (Chromium)
//   npx playwright test -c tests/webkit.config.ts                    (WebKit)
// The mode on the view while drawing or linking, F looking at what's picked, 3D <-> top-down
// keeping the same spot, the split view with hover and picks in both, and Undo after drawing.
// Nothing is saved: every change is undone before the test ends.
import { expect, test, type Page } from "@playwright/test";

const MAP = process.env.LS_TOOLS_NAV_MAP || "de_dust2";

type Probe = {
  hoverArea: number | null;
  hoverPoint: [number, number, number] | null;
  hoverView: string | null;
  target: [number, number] | null;
  dist: number;
};

/** What the page knows: the hovered area and where the camera looks (GoldSrc x, y). */
const probe = (page: Page) =>
  page.evaluate((): Probe => {
    const w = (window as unknown as { __lsNavfix: { useFix: { getState: () => { hoverArea: number | null; hoverPoint: [number, number, number] | null; hoverView: string | null } }; viewMemory: { target: { x: number; z: number } | null; dist: number } } }).__lsNavfix;
    const s = w.useFix.getState();
    const t = w.viewMemory.target;
    return { hoverArea: s.hoverArea, hoverPoint: s.hoverPoint, hoverView: s.hoverView, target: t ? [t.x, -t.z] : null, dist: w.viewMemory.dist };
  });

async function open(page: Page, layout: "orbit" | "top" | "split") {
  await page.addInitScript((l) => {
    localStorage.setItem("ls.navfix.layout", l);
    localStorage.setItem("ls.navfix.links", "picked");
  }, layout);
  await page.goto(`/tools-dev.html?open=navfix:${MAP}`);
  await page.locator(".navfix-view canvas").first().waitFor({ timeout: 60_000 });
  await expect(page.locator(".tools-overlay")).toHaveCount(0, { timeout: 60_000 });
  await page.waitForTimeout(1500);
  // a clean start: nothing left over from another test
  await expect(page.getByText("No unsaved changes")).toBeVisible();
}

/** A picture for looking over by eye: in test-results, and in LS_SHOTS (a folder) when set. */
async function shot(page: Page, name: string) {
  const dir = process.env.LS_SHOTS;
  const file = dir ? `${dir}/${test.info().project.name}-${name}` : test.info().outputPath(name);
  await page.screenshot({ path: file });
}

const box = async (page: Page, view: "orbit" | "top") => (await page.locator(`.navfix-view[data-view="${view}"]`).boundingBox())!;

/** Moves over a view until the mouse is on a nav area; returns where. */
async function findArea(page: Page, view: "orbit" | "top") {
  const r = await box(page, view);
  for (const fy of [0.5, 0.4, 0.6, 0.3, 0.7]) {
    for (const fx of [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8]) {
      const x = r.x + r.width * fx, y = r.y + r.height * fy;
      await page.mouse.move(x, y, { steps: 2 });
      await page.waitForTimeout(60);
      const p = await probe(page);
      if (p.hoverArea !== null) return { x, y, id: p.hoverArea };
    }
  }
  throw new Error("no nav area under the mouse anywhere in the view");
}

test("drawing and linking say so on the view; Esc steps back", async ({ page }) => {
  await open(page, "top");
  await expect(page.locator(".navfix-chip")).toHaveCount(0);
  await page.keyboard.press("d");
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toContainText("Drawing areas");
  await expect(page.locator(".navfix-canvas")).toHaveClass(/navfix-draw/);
  const r = await box(page, "top");
  await page.mouse.move(r.x + r.width / 2, r.y + r.height / 2);
  await expect(page.locator(".navfix-hint")).toContainText("Drag to draw");
  await page.keyboard.press("l");
  await expect(page.locator('.navfix-chip[data-mode="link"]')).toContainText("Click the area to link from");
  const a = await findArea(page, "top");
  await expect(page.locator(".navfix-hint")).toContainText(`Link from area ${a.id}`);
  await page.mouse.click(a.x, a.y);
  await expect(page.locator(".navfix-chip")).toContainText(`From ${a.id}`);
  await shot(page, "linking.png");
  await page.keyboard.press("Escape");
  await expect(page.locator(".navfix-chip")).toContainText("Click the area to link from");
  await page.keyboard.press("Escape");
  await expect(page.locator(".navfix-chip")).toHaveCount(0);
  await expect(page.locator(".navfix-tools [aria-pressed='true']")).toContainText("Select");
});

test("F looks at the picked area", async ({ page }) => {
  await open(page, "top");
  const a = await findArea(page, "top");
  await page.mouse.click(a.x, a.y);
  await expect(page.locator(".tools-side h3", { hasText: `Area ${a.id}` })).toBeVisible();
  const before = await probe(page);
  await page.keyboard.press("f");
  await page.waitForTimeout(1200);
  const after = await probe(page);
  // the view moved to it (closer for a small area, further out for a big one) ...
  expect(Math.hypot(after.target![0] - before.target![0], after.target![1] - before.target![1]) + Math.abs(after.dist - before.dist)).toBeGreaterThan(1);
  const r = await box(page, "top");
  await page.mouse.move(r.x + r.width / 2, r.y + r.height / 2, { steps: 3 });
  await page.waitForTimeout(150);
  // ... and it's in the middle
  expect((await probe(page)).hoverArea, "the picked area is in the middle").toBe(a.id);
  await shot(page, "focused.png");
});

test("3D and top-down keep the same spot in view", async ({ page }) => {
  await open(page, "orbit");
  // zoom in toward a spot on the nav (a floor, so top-down has it under its middle)
  await findArea(page, "orbit");
  for (let i = 0; i < 8; i++) {
    await page.mouse.wheel(0, -160);
    await page.waitForTimeout(40);
  }
  await page.waitForTimeout(900);
  const orbit = await probe(page);
  expect(orbit.dist).toBeLessThan(1500);
  await page.keyboard.press("t");
  await page.waitForTimeout(900);
  // the top-down view's middle is where the 3D view was looking
  const t = await box(page, "top");
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 3 });
  await page.waitForTimeout(150);
  const top = await probe(page);
  expect(top.hoverPoint, "something under the middle of the top-down view").not.toBeNull();
  const d = Math.hypot(top.hoverPoint![0] - orbit.target![0], top.hoverPoint![1] - orbit.target![1]);
  expect(d, "top-down center vs the 3D view's target").toBeLessThan(48);
  // about as much map in view
  expect(top.dist / orbit.dist).toBeGreaterThan(0.6);
  expect(top.dist / orbit.dist).toBeLessThan(1.6);
  await shot(page, "top-after-3d.png");
  // and back
  await page.keyboard.press("o");
  await page.waitForTimeout(900);
  const back = await probe(page);
  expect(Math.hypot(back.target![0] - orbit.target![0], back.target![1] - orbit.target![1])).toBeLessThan(48);
  await shot(page, "3d-after-top.png");
});

test("split view: both views, hover and picks in both", async ({ page }) => {
  await open(page, "split");
  await expect(page.locator(".navfix-view")).toHaveCount(2);
  await expect(page.locator(".navfix-view canvas")).toHaveCount(2);
  for (const v of ["top", "orbit"] as const) {
    const r = await box(page, v);
    expect(r.width).toBeGreaterThan(200);
  }
  const shot3d = async () => {
    const r = await box(page, "orbit");
    return page.screenshot({ clip: { x: r.x, y: r.y, width: r.width, height: r.height } });
  };
  // move off both views, then hover an area in the top-down view: the 3D view draws it too
  await page.mouse.move(5, 5);
  await page.waitForTimeout(300);
  const quiet = await shot3d();
  const a = await findArea(page, "top");
  const p = await probe(page);
  expect(p.hoverView).toBe("top");
  await page.waitForTimeout(300);
  expect(Buffer.compare(quiet, await shot3d()), "the 3D view shows the hovered area").not.toBe(0);
  await page.mouse.click(a.x, a.y);
  await expect(page.locator(".tools-side h3", { hasText: `Area ${a.id}` })).toBeVisible();
  // F moves both views to it
  await page.keyboard.press("f");
  await page.waitForTimeout(1200);
  const o = await box(page, "orbit");
  await page.mouse.move(o.x + o.width / 2, o.y + o.height / 2, { steps: 3 });
  await page.waitForTimeout(150);
  const in3d = await probe(page);
  expect(in3d.hoverView).toBe("orbit");
  expect(in3d.hoverPoint).not.toBeNull();
  await shot(page, "split.png");
});

test("Undo takes back a drawn area", async ({ page }) => {
  await open(page, "top");
  const a = await findArea(page, "top");
  await page.keyboard.press("d");
  await expect(page.locator('.navfix-chip[data-mode="draw"]')).toBeVisible();
  // (from a spot with floor under it: a press on nothing draws nothing)
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(a.x + 30, a.y + 24, { steps: 4 });
  await expect(page.locator(".navfix-hint")).toContainText("×");
  await shot(page, "drawing.png");
  await page.mouse.up();
  await expect(page.getByText(/^Added area \d+/)).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText(/1 change/)).toBeVisible();
  await page.keyboard.press(process.platform === "darwin" ? "Meta+z" : "Control+z");
  await expect(page.getByText("No unsaved changes")).toBeVisible({ timeout: 15_000 });
  await expect(page.locator(".tools-bar button", { hasText: /^Undo$/ })).toBeDisabled();
});
