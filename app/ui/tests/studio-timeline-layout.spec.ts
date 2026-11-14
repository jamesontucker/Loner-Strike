// Game Mode Studio's Timeline tab, laid out (Parity A, from the site screenshots): each track's header is
// exactly its lane's height and lined up with it, on one line, nothing in it touches its edges; the
// lane's value range sits beside the lane (in the header), never under a key; the toolbar's
// destructive button says what it deletes. `npx playwright test studio-timeline-layout`.
import { expect, test } from "@playwright/test";

const H = { "X-Panel": "1" };

test("Timeline tab: headers are their lanes' height and aligned; values beside the lanes; Delete timeline", async ({ page }, info) => {
  const id = `pw_tl_layout_${info.project.name}`;
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = id;
  tpl.timelines = [
    {
      id: "tl1", name: "Hill Glow", length: 2, loop: true, tick: 0.1,
      tracks: [
        { id: "c", name: "Glow", kind: "color", keys: [{ t: 0, v: [255, 200, 40] }, { t: 1, v: [255, 60, 20] }, { t: 2, v: [255, 200, 40] }] },
        { id: "r", name: "A long track name here", kind: "float", keys: [{ t: 0, v: 160 }, { t: 1, v: 227 }, { t: 2, v: 153 }] },
        { id: "p", name: "Spot", kind: "vector", keys: [{ t: 0, v: [0, 0, 0] }, { t: 2, v: [64, 32, 16] }] },
        { id: "b", name: "Beat", kind: "event", keys: [{ t: 0 }, { t: 1 }] },
      ],
    },
  ];
  expect((await (await page.request.post(`/api/tools/studio/graph/${id}`, { data: { graph: tpl }, headers: H })).json()).ok).toBeTruthy();
  await page.goto(`/tools-dev.html?open=studio:${id}/tl1`);
  const heads = page.getByTestId("timeline-head");
  await expect(heads).toHaveCount(4, { timeout: 20_000 });
  for (let i = 0; i < 4; i++) {
    const head = heads.nth(i);
    const lane = page.locator("svg.st-tl-lane").nth(i);
    const hb = (await head.boundingBox())!;
    const lb = (await lane.boundingBox())!;
    // the same height, side by side
    expect(Math.abs(hb.height - lb.height), `track ${i}: header ${hb.height} vs lane ${lb.height}`).toBeLessThanOrEqual(1);
    expect(Math.abs(hb.y - lb.y), `track ${i}: header and lane start together`).toBeLessThanOrEqual(1);
    // what's in the header stays inside it, off its top and bottom edges
    for (const el of await head.locator("input, button").all()) {
      const b = (await el.boundingBox())!;
      expect(b.y, `track ${i}: ${await el.getAttribute("aria-label")} off the top`).toBeGreaterThanOrEqual(hb.y + 2);
      expect(b.y + b.height, `track ${i}: off the bottom`).toBeLessThanOrEqual(hb.y + hb.height - 2);
      expect(b.x + b.width).toBeLessThanOrEqual(hb.x + hb.width);
    }
    // the name isn't cut: the input is wide enough for something
    expect((await head.locator("input").boundingBox())!.width).toBeGreaterThan(40);
  }
  // the value range of the number track: in its header, not in the lane (where keys go)
  const ax = heads.nth(1).locator(".st-tl-ax");
  // (the lane's top and bottom: the keys' range with a little room)
  await expect(ax).toHaveCount(2);
  await expect(ax.first()).toHaveText(/^\d+$/);
  const laneBox = (await page.locator("svg.st-tl-lane").nth(1).boundingBox())!;
  for (const el of await ax.all()) {
    const b = (await el.boundingBox())!;
    expect(b.x + b.width, "the value labels end before the lane").toBeLessThanOrEqual(laneBox.x);
  }
  await expect(page.locator("svg.st-tl-lane text")).toHaveCount(0);
  // a track is removed by its × (named), the Timeline by "Delete timeline"
  await expect(page.getByRole("button", { name: "Remove track Glow" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete timeline" })).toBeVisible();
  await page.request.post(`/api/tools/studio/graph/${id}/delete`, { headers: H });
});
