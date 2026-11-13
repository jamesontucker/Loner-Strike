// Map editor on a big map: a generated 3000-brush map stays light to draw (brushes merged by
// texture: a few hundred draws, not thousands) and the camera keeps moving smoothly while
// flying. Runs on editor-dev.html against the tools dev server (npm run test:tools --
// tests/editor-perf.spec.ts); in Chromium and, with --browser webkit, WebKit (the Mac app).
import { expect, test } from "@playwright/test";

/** A .map with `n` boxes in a grid (a few textures), as the paste command takes it. */
function bigMap(n: number): string {
  const tex = ["CRATE01", "OUT_WALL5", "-0CRETE1", "DUST_FLOOR"];
  const side = Math.ceil(Math.sqrt(n));
  const lines: string[] = ['{\n"classname" "worldspawn"\n"mapversion" "220"'];
  for (let i = 0; i < n; i++) {
    const x = (i % side) * 96 - side * 48;
    const y = Math.floor(i / side) * 96 - side * 48;
    const h = 32 + (i % 7) * 16;
    const t = tex[i % tex.length];
    const f = (a: string, b: string, c: string, u: string, v: string) => `( ${a} ) ( ${b} ) ( ${c} ) ${t} [ ${u} 0 ] [ ${v} 0 ] 0 1 1`;
    const [x0, x1, y0, y1, z0, z1] = [x, x + 64, y, y + 64, 0, h];
    lines.push(
      [
        "{",
        f(`${x0} ${y0} ${z1}`, `${x0} ${y1} ${z1}`, `${x1} ${y1} ${z1}`, "1 0 0", "0 -1 0"),
        f(`${x0} ${y1} ${z0}`, `${x0} ${y0} ${z0}`, `${x1} ${y0} ${z0}`, "1 0 0", "0 -1 0"),
        f(`${x0} ${y0} ${z0}`, `${x0} ${y1} ${z0}`, `${x0} ${y1} ${z1}`, "0 1 0", "0 0 -1"),
        f(`${x1} ${y1} ${z0}`, `${x1} ${y0} ${z0}`, `${x1} ${y0} ${z1}`, "0 1 0", "0 0 -1"),
        f(`${x1} ${y0} ${z0}`, `${x0} ${y0} ${z0}`, `${x0} ${y0} ${z1}`, "1 0 0", "0 0 -1"),
        f(`${x0} ${y1} ${z0}`, `${x1} ${y1} ${z0}`, `${x1} ${y1} ${z1}`, "1 0 0", "0 0 -1"),
        "}",
      ].join("\n"),
    );
  }
  lines.push("}");
  return lines.join("\n");
}

test("a 3000-brush map: few draws, and flying stays smooth", async ({ page }) => {
  test.setTimeout(180_000);
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 4 }));
    localStorage.setItem("ls.editor.navHint", "1");
  });
  await page.goto("/editor-dev.html");
  // a fresh map, then the boxes pasted into it through the app
  const r0 = await page.evaluate(async () => (await fetch("/api/tools/editor/new", { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: "{}" })).json());
  expect(r0.error ?? null).toBeNull();
  const text = bigMap(3000);
  const r1 = await page.evaluate(async (t) => (await fetch("/api/tools/editor/cmd", { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify({ op: "paste", text: t, offset: [0, 0, 0] }) })).json(), text);
  expect(r1.error ?? null).toBeNull();
  await page.reload();
  const back = page.getByRole("button", { name: /^Back to/ });
  await back.waitFor({ timeout: 15_000 });
  await back.click();
  await page.waitForFunction(() => /3\d{3} brushes/.test(document.querySelector(".tools-status")?.textContent || ""), null, { timeout: 120_000 });
  await page.waitForTimeout(2000);

  const pane = (await page.locator(".ed-pane").first().boundingBox())!;
  const cx = pane.x + pane.width / 2;
  const cy = pane.y + pane.height / 2;
  await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean };
    w.__f = 0;
    w.__stop = false;
    const f = () => {
      w.__f++;
      if (!w.__stop) requestAnimationFrame(f);
    };
    requestAnimationFrame(f);
  });
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(cx + 3, cy, { steps: 2 });
  await page.keyboard.down("w");
  const t0 = Date.now();
  for (let i = 0; i < 20; i++) {
    await page.mouse.move(cx + 3 + (i % 5) * 5, cy, { steps: 1 });
    await page.waitForTimeout(100);
  }
  const seconds = (Date.now() - t0) / 1000;
  await page.keyboard.up("w");
  await page.mouse.up({ button: "right" });
  const frames = await page.evaluate(() => {
    const w = window as unknown as { __f: number; __stop: boolean };
    w.__stop = true;
    return w.__f;
  });
  const calls = await page.evaluate(() => (window as unknown as { __lsEditorGl?: { info: { render: { calls: number } } } }).__lsEditorGl?.info.render.calls ?? -1);
  const fps = frames / seconds;
  test.info().annotations.push({ type: "perf", description: `${Math.round(fps)} fps while flying, ${calls} draws` });
  expect(calls, "draw calls for 3000 brushes").toBeGreaterThan(0);
  expect(calls, "draw calls for 3000 brushes").toBeLessThan(600);
  // headless Chromium draws in software: the bar is low, but one-draw-per-brush fell far under it
  expect(fps).toBeGreaterThan(12);
});
