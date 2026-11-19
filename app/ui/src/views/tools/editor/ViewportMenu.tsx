// 1.0.1: right-click in a 3D or 2D view opens a menu of the common commands (the Inspector's
// menu, InspectMenu.tsx, with the editor's own labels and keys). Right-drag stays the camera's
// (Unreal look, Hammer and Blender dolly/pan): the menu only opens for a click, when the pointer
// didn't move and the press was short. The Texture tool keeps Hammer's right-click paint. What
// the press landed on gets picked first (EditorScene reports it), unless it's locked.

import { useEffect } from "react";
import { commands } from "./commands";
import { openInspectMenuAt, type MenuItem } from "./InspectMenu";
import { useP5 } from "./p5store";
import { keysOf } from "./Shortcuts";
import { useEditor } from "./store";

type RightHit = { kind: "brush" | "entity"; id: number } | null;
let lastHit: { at: number; hit: RightHit } = { at: 0, hit: null };

/** EditorScene: what a right press landed on (nothing: empty space or the sky). */
export function noteRightHit(hit: RightHit) {
  lastHit = { at: performance.now(), hit };
}

/** The commands in the menu, by id; null is a line. */
const ITEMS: (string | null)[] = [
  "edit.cut", "edit.copy", "edit.paste", "edit.duplicate", "edit.delete",
  null, "view.frame", "map.inspect",
  null, "sel.hide", "sel.isolate", "sel.showAll",
  null, "edit.group", "edit.ungroup", "edit.tie", "edit.toWorld",
  null, "edit.hollow", "edit.carve", "edit.snap",
];

function items(): MenuItem[] {
  const all = commands();
  return ITEMS.map((id) => {
    if (id === null) return null;
    const c = all.find((x) => x.id === id);
    if (!c) return null;
    return { label: c.label, run: () => void c.run(), disabled: !!c.when && !c.when(), keys: keysOf(c.id)[0], tip: c.hint };
  });
}

/** Picks what the press hit, the way a left click would (a brush of an entity picks the entity), unless it's picked or locked already. */
function pickHit(hit: RightHit) {
  if (!hit) return;
  const s = useEditor.getState();
  const locked = useP5.getState().locked;
  if (hit.kind === "entity") {
    if (locked.entities.has(hit.id) || s.sel.entities.includes(hit.id)) return;
    return s.select({ brushes: [], entities: [hit.id], face: null });
  }
  const owner = s.brushes.get(hit.id)?.entity;
  if (owner != null) {
    if (locked.entities.has(owner) || s.sel.entities.includes(owner)) return;
    return s.select({ brushes: [], entities: [owner], face: null });
  }
  if (locked.brushes.has(hit.id) || s.sel.brushes.includes(hit.id)) return;
  s.select({ brushes: [hit.id], entities: [], face: null });
}

const MAX_MOVE = 4; // px (summed: under pointer lock clientX stands still)
const MAX_HOLD = 450; // ms

/** Mounted once by the menu bar. */
export function useViewportMenu() {
  useEffect(() => {
    let press: { t: number; x: number; y: number; moved: number } | null = null;
    const move = (m: PointerEvent) => {
      if (press) press.moved += Math.abs(m.movementX) + Math.abs(m.movementY);
    };
    const down = (e: PointerEvent) => {
      if (e.button !== 2) return;
      const t = e.target as HTMLElement | null;
      if (!t?.closest?.(".ed-pane") || t.closest(".ed-pane-head")) return;
      press = { t: performance.now(), x: e.clientX, y: e.clientY, moved: 0 };
      window.addEventListener("pointermove", move, true);
    };
    const up = (u: PointerEvent) => {
      const p = press;
      if (!p) return;
      if (u.button !== 2) return;
      press = null;
      window.removeEventListener("pointermove", move, true);
      if (p.moved > MAX_MOVE || performance.now() - p.t > MAX_HOLD) return;
      const s = useEditor.getState();
      if (s.tool === "texture" || s.dialog || useP5.getState().dialog) return;
      pickHit(lastHit.at >= p.t ? lastHit.hit : null);
      openInspectMenuAt(p.x, p.y, items(), "Edit what's picked");
    };
    const cancel = () => {
      press = null;
      window.removeEventListener("pointermove", move, true);
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("pointerup", up, true);
    window.addEventListener("pointercancel", cancel, true);
    window.addEventListener("blur", cancel);
    return () => {
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("pointercancel", cancel, true);
      window.removeEventListener("blur", cancel);
      window.removeEventListener("pointermove", move, true);
    };
  }, []);
}
