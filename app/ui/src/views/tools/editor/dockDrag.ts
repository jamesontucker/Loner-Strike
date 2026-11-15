// P5: where a dragged panel would land, worked out from the pointer and the boxes on screen.
// Pure (no DOM), so it's unit-tested (scripts/dock.test.ts). Order of preference:
//   1. a tab strip (reorder or join): always wins, so moving a tab along its own strip works
//      even next to a dock target
//   2. a dock target (the small squares near the editor's edges)
//   3. an area's panel body (join that area at the end)
//   4. the view: float the panel there
//   5. anywhere else: nothing (the drag is cancelled)

export type AreaKey = "left" | "right" | "right2" | "bottom";
export type Box = { left: number; top: number; right: number; bottom: number };
export type Strip = { area: AreaKey; box: Box; tabs: { id: string; box: Box }[] };
export type Geometry = { strips: Strip[]; targets: { area: AreaKey; box: Box }[]; bodies: { area: AreaKey; box: Box }[]; view: Box | null };
export type Drop =
  | { kind: "tab"; area: AreaKey; before: string | null; markerX: number; markerTop: number; markerBottom: number }
  | { kind: "area"; area: AreaKey }
  | { kind: "float"; x: number; y: number }
  | null;

const inside = (b: Box, x: number, y: number, pad = 0) => x >= b.left - pad && x <= b.right + pad && y >= b.top - pad && y <= b.bottom + pad;

/** Where `dragged` would go if let go at (x, y). */
export function resolveDrop(g: Geometry, x: number, y: number, dragged: string, canFloat = true): Drop {
  // 1. tab strips (a little slack above and below so it's easy to hit)
  for (const s of g.strips) {
    if (!inside(s.box, x, y, 6)) continue;
    const tabs = s.tabs.filter((t) => t.id !== dragged);
    const next = tabs.find((t) => x < (t.box.left + t.box.right) / 2);
    const markerX = next ? next.box.left - 1 : tabs.length ? tabs[tabs.length - 1].box.right + 1 : s.box.left + 4;
    return { kind: "tab", area: s.area, before: next?.id ?? null, markerX, markerTop: s.box.top + 2, markerBottom: s.box.bottom - 2 };
  }
  // 2. dock targets
  for (const t of g.targets) if (inside(t.box, x, y)) return { kind: "area", area: t.area };
  // 3. a panel's body
  for (const b of g.bodies) if (inside(b.box, x, y)) return { kind: "area", area: b.area };
  // 4. the view
  if (canFloat && g.view && inside(g.view, x, y)) return { kind: "float", x, y };
  return null;
}

/** Whether a drop changes anything (dropping a tab back where it is doesn't). */
export function dropChanges(drop: Drop, dragged: string, where: { area: AreaKey | null; tabs: string[] }): boolean {
  if (!drop) return false;
  if (drop.kind === "float") return true;
  if (drop.area !== where.area) return true;
  if (drop.kind === "area") return false;
  const i = where.tabs.indexOf(dragged);
  const after = drop.before == null ? where.tabs.length : where.tabs.indexOf(drop.before);
  // before itself or before the one right after it: the same place
  return !(after === i || after === i + 1);
}

/** The dock targets' boxes for the editor's main box (small squares near each edge). */
export function targetBoxes(main: Box, size = 44): { area: AreaKey; box: Box }[] {
  const h = size / 2;
  const w = main.right - main.left;
  const ht = main.bottom - main.top;
  const at = (cx: number, cy: number): Box => ({ left: cx - h, top: cy - h, right: cx + h, bottom: cy + h });
  return [
    { area: "left", box: at(main.left + 90 + h, main.top + ht * 0.32) },
    { area: "right", box: at(main.right - 16 - h, main.top + ht * 0.3) },
    { area: "right2", box: at(main.right - 16 - h, main.top + ht * 0.62) },
    { area: "bottom", box: at(main.left + w * 0.5, main.bottom - 16 - h) },
  ];
}
