// P5: the first-map tour: five steps that tick off as you do them (draw a room, Hollow, add a
// light, spawns for both teams, F5 Playtest). Offered once after the first New map ("Not now",
// "Don't show again"); Help > First map tour starts it any time. A slim strip under the bar, in
// the page's flow, so it never covers the views or a control: the steps as ticks, the next one
// spelled out.

import { useEffect, useMemo } from "react";
import { create } from "zustand";
import { useP5 } from "./p5store";
import { keyHint } from "./Shortcuts";
import { useEditor } from "./store";

const KEY = "ls.editor.tour";

type Saved = { offered: boolean; never: boolean };
function read(): Saved {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { offered: !!v.offered, never: !!v.never };
  } catch {
    return { offered: false, never: false };
  }
}
function write(v: Saved) {
  try {
    localStorage.setItem(KEY, JSON.stringify(v));
  } catch {
    /* private window: offered again next time */
  }
}

/** Where the map was when the tour started (steps compare against it). */
type Start = { docId: number; brushes: number; lights: number; playtests: number };

type TourState = Saved & {
  /** "offer": the one-time question; "on": the steps; null: nothing shown */
  mode: "offer" | "on" | null;
  folded: boolean;
  start: Start | null;
  hollowed: boolean;
  set: (p: Partial<TourState>) => void;
};

export const useTour = create<TourState>((set) => ({ ...read(), mode: null, folded: false, start: null, hollowed: false, set }));

function baseline(): Start | null {
  const s = useEditor.getState();
  const d = s.doc;
  if (!d) return null;
  return { docId: d.docId, brushes: s.brushes.size, lights: d.entities.filter((e) => e.classname.startsWith("light")).length, playtests: useP5.getState().playtestAsked };
}

/** Help > First map tour (and the offer's Start). */
export function startTour() {
  useTour.getState().set({ mode: "on", folded: false, start: baseline(), hollowed: false });
}

function finish(never = false) {
  const t = useTour.getState();
  const saved = { offered: true, never: never || t.never };
  write(saved);
  t.set({ ...saved, mode: null });
}

/** Mounted with the menu bar: offers the tour after the first New map, and shows it. */
// The first New map: "Starting a new map…" has just finished (watched from the start, as the
// start screen's New map runs before the editor and its bar are there).
useEditor.subscribe((now, before) => {
  if (before.fileBusy === "Starting a new map…" && !now.fileBusy && now.doc) {
    const t = useTour.getState();
    if (!t.offered && !t.never && t.mode === null) t.set({ mode: "offer" });
  }
});

export function FirstMapTour() {
  const mode = useTour((t) => t.mode);
  if (!mode) return null;
  return mode === "offer" ? <Offer /> : <Steps />;
}

function Offer() {
  return (
    <aside className="banner p5-tour" role="dialog" aria-label="First map tour" aria-modal="false">
      <span>
        <strong>New to making maps?</strong> A short tour: draw a room, hollow it, add a light and spawns, then play it. Each step ticks off as you do it.
      </span>
      <div className="p5-tour-actions">
        <button type="button" className="btn small primary" onClick={() => (write({ offered: true, never: false }), startTour())}>
          Start the tour
        </button>
        <button type="button" className="btn small" onClick={() => finish()}>
          Not now
        </button>
        <button type="button" className="btn small" onClick={() => finish(true)}>
          Don't show again
        </button>
      </div>
      <span className="small-note muted">Help &gt; First map tour, any time.</span>
    </aside>
  );
}

function Steps() {
  const t = useTour();
  const doc = useEditor((s) => s.doc);
  const brushes = useEditor((s) => s.brushes);
  const asked = useP5((p) => p.playtestAsked);
  // Hollow: seen as the last undo step at some point
  useEffect(() => {
    if (doc?.undo === "Hollow" && !t.hollowed) t.set({ hollowed: true });
  }, [doc?.undo]); // eslint-disable-line react-hooks/exhaustive-deps
  // another map opened: the steps start over from it
  useEffect(() => {
    if (doc && t.start && doc.docId !== t.start.docId) t.set({ start: baseline(), hollowed: false });
  }, [doc?.docId]); // eslint-disable-line react-hooks/exhaustive-deps
  const st = t.start;
  const ents = doc?.entities ?? [];
  const has = (c: string) => ents.some((e) => e.classname === c);
  const steps = useMemo(
    () => [
      { short: "Room", done: !!st && brushes.size > st.brushes, text: `Draw a room: the Box tool${keyHint("tool.box")}, drag on the floor in the 3D view or in Top` },
      { short: "Hollow", done: t.hollowed, text: `Hollow it${keyHint("edit.hollow")}: with the box picked, Hollow in Inspect makes walls round an empty middle` },
      { short: "Light", done: !!st && ents.filter((e) => e.classname.startsWith("light")).length > st.lights, text: `Add a light: the Entity tool${keyHint("tool.entity")}, pick light, click inside the room` },
      { short: "Spawns", done: has("info_player_start") && has("info_player_deathmatch"), text: "Spawns for both teams: info_player_start (CT) and info_player_deathmatch (T) inside the room (a new map has two of each: move them in)" },
      { short: "Play", done: !!st && asked > st.playtests, text: `Play it${keyHint("map.playtest")}: Playtest compiles it and starts the game with bots` },
    ],
    [st, brushes, ents, t.hollowed, asked], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const n = steps.filter((s) => s.done).length;
  const all = n === steps.length;
  const next = steps.find((s) => !s.done);
  return (
    <aside className="banner p5-tour p5-tour-on" role="complementary" aria-label="First map tour">
      <strong>First map tour</strong>
      <span className="muted">
        {n} of {steps.length}
      </span>
      <ol className="p5-tour-steps">
        {steps.map((s, i) => (
          <li key={i} className={s.done ? "done" : s === next ? "next" : undefined} aria-checked={s.done} role="checkbox" aria-readonly="true" title={s.text}>
            <span className="p5-tour-tick" aria-hidden="true">
              {s.done ? "✓" : i + 1}
            </span>
            <span className="p5-tour-short">{s.short}</span>
          </li>
        ))}
      </ol>
      {!t.folded && <span className="p5-tour-next">{all ? "That's a map. Save it (File > Save) to keep it as a project." : next?.text}</span>}
      <span className="p5-tour-actions">
        <button type="button" className="btn small icon" aria-label={t.folded ? "Show the next step" : "Hide the next step"} title={t.folded ? "Show what to do next" : "Just the ticks"} onClick={() => t.set({ folded: !t.folded })}>
          {t.folded ? "▸" : "◂"}
        </button>
        <button type="button" className="btn small icon" aria-label="Close the tour" title="Close (Help > First map tour brings it back)" onClick={() => finish()}>
          ×
        </button>
      </span>
    </aside>
  );
}
