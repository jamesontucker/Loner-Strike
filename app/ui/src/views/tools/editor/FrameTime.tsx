// MCP: a frame-time readout for the map editor (View > Show frame time, or Ctrl+Alt+Shift+F /
// ⌘⌥⇧F): frames a second and the frame time's median (p50) and p95 over the last 5 seconds,
// measured from the browser's animation frames, so slow frames from the views, React or the
// store all count. For judging big maps (br_valley) in the real app. Off by default; when
// off nothing is mounted and no frame loop runs.

import { useEffect, useState } from "react";
import { create } from "zustand";

const KEY = "ls.editor.frameTime";
const WINDOW_MS = 5000;

function saved(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export const useFrameTime = create<{ on: boolean; set: (on: boolean) => void }>((set) => ({
  on: saved(),
  set: (on) => {
    try {
      localStorage.setItem(KEY, on ? "1" : "0");
    } catch {
      /* this session only */
    }
    set({ on });
  },
}));

export const toggleFrameTime = () => useFrameTime.getState().set(!useFrameTime.getState().on);

/** The p-th percentile (0-100) of sorted numbers. */
export function percentile(sorted: number[], p: number): number {
  if (!sorted.length) return 0;
  const i = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
  return sorted[i];
}

/** What the readout shows for these frame times (ms) over `spanMs`. */
export function summarize(frames: number[], spanMs: number) {
  const sorted = [...frames].sort((a, b) => a - b);
  return { fps: spanMs > 0 ? (frames.length * 1000) / spanMs : 0, p50: percentile(sorted, 50), p95: percentile(sorted, 95), worst: sorted[sorted.length - 1] ?? 0 };
}

function Readout() {
  const [shown, setShown] = useState<ReturnType<typeof summarize> | null>(null);
  useEffect(() => {
    // (times and deltas of the frames in the last 5 s)
    const times: number[] = [];
    const deltas: number[] = [];
    let last = 0;
    let raf = 0;
    let lastShown = 0;
    const tick = (t: number) => {
      if (last) {
        times.push(t);
        deltas.push(t - last);
        while (times.length && t - times[0] > WINDOW_MS) {
          times.shift();
          deltas.shift();
        }
        if (t - lastShown > 500) {
          lastShown = t;
          setShown(summarize(deltas, Math.min(WINDOW_MS, t - (times[0] ?? t) + (deltas[0] ?? 0))));
        }
      }
      last = t;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  const tone = !shown ? "" : shown.p95 > 50 ? " bad" : shown.p95 > 25 ? " meh" : "";
  return (
    <div
      className={`ed-frametime${tone}`}
      role="status"
      aria-label="Frame time"
      title="Frames a second and frame time over the last 5 seconds (View > Show frame time)"
      style={{ position: "fixed", right: 12, bottom: 34, zIndex: 40, pointerEvents: "none", font: "12px/1.35 ui-monospace, Menlo, Consolas, monospace", padding: "4px 8px", borderRadius: 2, background: "rgba(0,0,0,.72)", color: tone === " bad" ? "#f0b9a6" : tone === " meh" ? "#f1d58a" : "#cfe8c6", whiteSpace: "pre" }}
    >
      {shown ? `${shown.fps.toFixed(0)} fps\np50 ${shown.p50.toFixed(1)} ms · p95 ${shown.p95.toFixed(1)} ms\nworst ${shown.worst.toFixed(0)} ms` : "measuring…"}
    </div>
  );
}

/** Mounted once on the editor page: the readout when it's on, and its shortcut. */
export function FrameTimeReadout() {
  const on = useFrameTime((s) => s.on);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.shiftKey && e.altKey && (e.ctrlKey || e.metaKey) && e.code === "KeyF") {
        e.preventDefault();
        toggleFrameTime();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  return on ? <Readout /> : null;
}
