// Parity A: redraw only the panes that changed. Every pane has its own scene (a portal), and
// each pane's portal gets its own `invalidate`, which marks just that pane: panning or zooming
// one view (its controls call it), a hover cross-hair, a ghost while drawing. Anything that can
// change what every pane shows marks them all: the editor's stores changing (the map, the
// selection, a drag's preview, hidden things, layout...), the canvas's own invalidate (outside
// the panes), and a pane's box moving. A pane-only change is followed shortly by one redraw of
// every pane, so nothing can stay stale (React skips an object's invalidate while a frame is
// already on its way). The canvas keeps its picture between frames (preserveDrawingBuffer), so
// a pane that isn't redrawn keeps what it showed.

import { useThree, type RootState } from "@react-three/fiber";
import { useEffect, useRef } from "react";
import { useMarquee } from "./Marquee";
import { useP5 } from "./p5store";
import { usePanes } from "./panes";
import { useEditor } from "./store";
import { useTools } from "../../../stores/tools";
import { useKits } from "./kits";

let gen = 1;
const drawnGen = new Map<number, number>();
const dirty = new Set<number>();
let rootInvalidate: ((frames?: number) => void) | null = null;
let settle: ReturnType<typeof setTimeout> | null = null;

/** Every pane draws on the next frame. */
export function markAll() {
  gen++;
}

/** Only pane `i` changed; every pane catches up shortly after the last such change. */
function markPane(i: number) {
  dirty.add(i);
  if (settle) clearTimeout(settle);
  settle = setTimeout(() => {
    settle = null;
    markAll();
    rootInvalidate?.();
  }, 250);
}

/** The canvas's invalidate marks every pane (call once from the canvas's onCreated). */
export function wrapRootInvalidate(state: RootState) {
  const orig = state.invalidate;
  rootInvalidate = orig;
  state.set({
    invalidate: (frames?: number) => {
      markAll();
      orig(frames);
    },
  });
}

/** A pane's own invalidate (for its portal's state). */
export function paneInvalidate(i: number, fallback: (frames?: number) => void) {
  return (frames?: number) => {
    markPane(i);
    (rootInvalidate ?? fallback)(frames);
  };
}

/** Should pane `i` draw this frame? `pos`: its box on the canvas (a moved box always draws). */
const lastPos = new Map<number, string>();
export function paneNeedsDraw(i: number, pos: { left: number; bottom: number; width: number; height: number }): boolean {
  const key = `${pos.left},${pos.bottom},${pos.width},${pos.height}`;
  const moved = lastPos.get(i) !== key;
  return moved || dirty.has(i) || (drawnGen.get(i) ?? 0) < gen;
}
export function paneDrawn(i: number, pos: { left: number; bottom: number; width: number; height: number }) {
  lastPos.set(i, `${pos.left},${pos.bottom},${pos.width},${pos.height}`);
  drawnGen.set(i, gen);
  dirty.delete(i);
}

// What changes everything: the stores' changes, but not the ones that only move a status line
// (the cursor's place is drawn by each flat pane's own cross-hair, which marks its pane).
const QUIET: Record<string, true> = { cursor: true, working: true, fileBusy: true, hover: true };
function watch<T extends object>(store: { subscribe: (f: (s: T, p: T) => void) => () => void }) {
  store.subscribe((s, p) => {
    for (const k of Object.keys(s) as (keyof T)[]) {
      if (s[k] !== p[k] && !QUIET[k as string]) {
        markAll();
        return;
      }
    }
  });
}
watch(useEditor);
watch(useP5);
watch(usePanes);
watch(useMarquee);
watch(useTools);
watch(useKits);

/** While the camera is dragged with the middle or right button in this pane (pan, look, fly),
 *  the pane stops casting rays for hover (on a big map every mouse move tested every brush);
 *  presses still reach the tools, and it picks again when the button comes up. Mount in a pane. */
export function QuietWhileDragging() {
  const st = useThree();
  const live = useRef(st);
  live.current = st;
  useEffect(() => {
    let off = false;
    const move = (e: PointerEvent) => {
      if (off || !(e.buttons & 6)) return;
      const el = live.current.events.connected as HTMLElement | undefined;
      if (!el || !(e.target instanceof Node) || !el.contains(e.target)) return;
      off = true;
      live.current.setEvents({ enabled: false });
    };
    const up = () => {
      if (!off) return;
      off = false;
      live.current.setEvents({ enabled: true });
    };
    window.addEventListener("pointermove", move, true);
    window.addEventListener("pointerup", up, true);
    window.addEventListener("blur", up);
    return () => {
      window.removeEventListener("pointermove", move, true);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("blur", up);
      up();
    };
  }, []);
  return null;
}

/** Every pane draws on the next frame (shared buffers changed in place). */
export function redrawAll() {
  markAll();
  rootInvalidate?.();
}
