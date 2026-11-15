// Unit tests for the map editor's dock drops (src/views/tools/editor/dockDrag.ts).
//   npm run test:dock

import { test } from "node:test";
import assert from "node:assert/strict";
import { dropChanges, resolveDrop, targetBoxes, type Geometry } from "../src/views/tools/editor/dockDrag";

const box = (left: number, top: number, right: number, bottom: number) => ({ left, top, right, bottom });
// a 1024-wide editor: bottom strip with three tabs at the left edge, right strip, the view
const main = box(70, 100, 1024, 760);
const g: Geometry = {
  strips: [
    { area: "bottom", box: box(90, 540, 700, 566), tabs: [{ id: "library", box: box(92, 542, 150, 564) }, { id: "textures", box: box(152, 542, 220, 564) }, { id: "checks", box: box(222, 542, 280, 564) }] },
    { area: "right", box: box(720, 100, 1024, 126), tabs: [{ id: "outline", box: box(722, 102, 790, 124) }] },
  ],
  targets: targetBoxes(main),
  bodies: [{ area: "right", box: box(720, 126, 1024, 400) }],
  view: box(90, 130, 700, 530),
};

test("reordering in a strip wins over the left dock target", () => {
  // the first tab's left half: right where the old left drop zone sat
  const d = resolveDrop(g, 100, 553, "checks");
  assert.deepEqual(d && d.kind === "tab" ? [d.area, d.before] : d, ["bottom", "library"]);
  assert.ok(d?.kind === "tab" && d.markerX === 91);
});

test("dropping past the last tab goes to the end", () => {
  const d = resolveDrop(g, 600, 553, "library");
  assert.ok(d?.kind === "tab" && d.before === null);
  assert.equal(dropChanges(d, "library", { area: "bottom", tabs: ["library", "textures", "checks"] }), true);
});

test("dropping a tab where it already is changes nothing", () => {
  // checks, dropped just before itself / just after textures
  const d = resolveDrop(g, 230, 553, "checks");
  assert.ok(d?.kind === "tab");
  assert.equal(dropChanges(d, "checks", { area: "bottom", tabs: ["library", "textures", "checks"] }), false);
  const e = resolveDrop(g, 160, 553, "textures");
  assert.equal(dropChanges(e, "textures", { area: "bottom", tabs: ["library", "textures", "checks"] }), false);
});

test("dock targets, bodies, the view and nowhere", () => {
  const left = g.targets.find((t) => t.area === "left")!.box;
  assert.deepEqual(resolveDrop(g, (left.left + left.right) / 2, (left.top + left.bottom) / 2, "outline"), { kind: "area", area: "left" });
  assert.deepEqual(resolveDrop(g, 800, 300, "library"), { kind: "area", area: "right" });
  assert.deepEqual(resolveDrop(g, 400, 300, "library"), { kind: "float", x: 400, y: 300 });
  assert.equal(resolveDrop(g, 400, 300, "library", false), null);
  assert.equal(resolveDrop(g, 5, 5, "library"), null);
});

test("targets sit near the edges and don't touch a bottom strip at the left", () => {
  const t = targetBoxes(main);
  const strip = g.strips[0].box;
  for (const x of t) {
    const overlap = x.box.left < strip.right && x.box.right > strip.left && x.box.top < strip.bottom && x.box.bottom > strip.top;
    assert.equal(overlap, false, x.area);
  }
});

test("at 1024×700 the left target stays clear of a tall bottom area's tabs", () => {
  // measured in the editor at 1024×700 with the default layout (bottom area 250 high)
  const m = box(88, 130, 1024, 688);
  const strip = box(90, 416, 670, 441);
  const left = targetBoxes(m).find((t) => t.area === "left")!.box;
  assert.ok(left.bottom < strip.top - 6, `left target ${left.top}-${left.bottom} vs strip ${strip.top}`);
});
