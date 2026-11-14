// The Studio tour: a short walk round the Studio on one of the example modes (Infection):
// the canvas, adding nodes, the notes, My mode and Details, the three checks, Playtest and
// Install, and the help. Offered once on the first visit ("Not now", "Don't show again"); the
// "?" menu's "Take the Studio tour" starts it any time. Each step rings the real control and
// says what it's for; Next, Back and Skip, or the arrow keys and Escape.

import { useEffect, useLayoutEffect, useState } from "react";
import { create } from "zustand";

const KEY = "ls.studio.tour";

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

type TourState = Saved & {
  /** the step shown (-1: none) */
  step: number;
  set: (p: Partial<TourState>) => void;
};

export const useStudioTour = create<TourState>((set) => ({ ...read(), step: -1, set }));

/** A step: what it points at (a CSS selector; none: the middle of the screen), and what it says. */
export type TourStep = { at?: string; title: string; text: string };

export const TOUR_STEPS: TourStep[] = [
  { title: "Welcome to the Game Mode Studio", text: "This is Infection, one of the example modes. A mode is made of nodes: when something happens (a round starts, a player dies), check things and do things. Let's look round. It takes about a minute." },
  { at: ".st-canvas", title: "The canvas", text: "Each box is a node and the wires carry the flow (white) and values (colored) between them. Drag to move, scroll to zoom, drag empty space to pan. The colored boxes are notes that explain each part." },
  { at: ".st-canvas", title: "Adding nodes", text: "Right-click empty space, press Space, or drag a wire out of a pin and let go: the node list opens, and you can search it. Dropping a wire only lists nodes that fit. Shortcuts: hold B (Branch), S (Sequence), D (Delay) or F (For Each Player) and click empty space." },
  { at: ".st-mb", title: "My mode", text: "The mode's own pieces: its variables (Infection Started, here), Functions (Infect: what makes a zombie), graphs and snippets. Drag any of them onto the canvas: a variable with Ctrl (⌘) held drops a Get, with Alt (⌥) a Set. Double-click a Function to open its graph." },
  { at: ".st-side", title: "Details", text: "Pick a node to see and change its settings here. The mode's name, description and game settings are here when nothing is picked." },
  { at: ".st-results", title: "Problems", text: "The quick check runs as you edit and lists anything that won't work, like a node nothing runs or a missing wire. Click a problem to jump to its node." },
  { at: "[data-testid=studio-compile]", title: "Compile", text: "Runs the real compiler on the mode, without installing it or starting the game. The icon beside it turns green when it's good." },
  { at: "[data-testid=studio-test]", title: "Test run", text: "Plays the mode on a hidden server with bots for about two minutes and reports what happened: errors tied to their nodes, how heavy it is, and which nodes never ran. Your game isn't touched." },
  { at: "[data-testid=studio-playtest]", title: "Playtest", text: "Starts the game in this mode with bots, with the debugger on: nodes light up as they run, and you can set breakpoints. Pick the map and the number of bots beside it." },
  { at: ".tl-actionbar", title: "Save and Install", text: "Save keeps the graph. Install compiles it and adds it to the game's modes, so it's on the Play page and in the in-game menu." },
  { at: "[data-testid=studio-help-menu]", title: "Help", text: "Node help explains every node, with an example you can drop in (or press F1 on a picked node). This tour is in here too. Have fun making modes!" },
];

/** "?" › Take the Studio tour, and the offer's Start. */
export function startStudioTour() {
  useStudioTour.getState().set({ step: 0 });
}

function finish(never = false) {
  const t = useStudioTour.getState();
  const saved = { offered: true, never: never || t.never };
  write(saved);
  t.set({ ...saved, step: -1 });
}

export function TourOffer({ onStart }: { onStart: () => void }) {
  const t = useStudioTour();
  if (t.offered || t.never) return null;
  return (
    <aside className="banner st-tour-offer" role="dialog" aria-label="Studio tour" aria-modal="false" data-testid="studio-tour-offer">
      <span>
        <strong>New to the Studio?</strong> A one-minute tour on an example mode: the canvas, adding nodes, the checks, Playtest and Install.
      </span>
      <span className="st-tour-actions">
        <button type="button" className="btn small primary" onClick={() => (write({ offered: true, never: false }), t.set({ offered: true }), onStart())}>
          Take the tour
        </button>
        <button type="button" className="btn small" onClick={() => finish()}>
          Not now
        </button>
        <button type="button" className="btn small" onClick={() => finish(true)}>
          Don't show again
        </button>
      </span>
      <span className="st-muted">It's in the ? menu any time.</span>
    </aside>
  );
}

type Box = { x: number; y: number; w: number; h: number } | null;

/** Where a step's control is on screen (null: none, or not shown). */
function boxOf(sel?: string): Box {
  if (!sel) return null;
  const el = document.querySelector(sel);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return r.width && r.height ? { x: r.left, y: r.top, w: r.width, h: r.height } : null;
}

export function StudioTour() {
  const step = useStudioTour((t) => t.step);
  const set = useStudioTour((t) => t.set);
  const [box, setBox] = useState<Box>(null);
  const [size, setSize] = useState({ w: window.innerWidth, h: window.innerHeight });
  const s = step >= 0 ? TOUR_STEPS[step] : null;
  // follow the control (the window resized, a panel opened)
  useLayoutEffect(() => {
    if (!s) return;
    const update = () => {
      setBox(boxOf(s.at));
      setSize({ w: window.innerWidth, h: window.innerHeight });
    };
    update();
    const t = setInterval(update, 400);
    window.addEventListener("resize", update);
    return () => {
      clearInterval(t);
      window.removeEventListener("resize", update);
    };
  }, [s]);
  useEffect(() => {
    if (!s) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish();
      else if (e.key === "ArrowRight" || e.key === "Enter") next();
      else if (e.key === "ArrowLeft") back();
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }); // eslint-disable-line react-hooks/exhaustive-deps
  if (!s) return null;
  const last = step === TOUR_STEPS.length - 1;
  const next = () => (last ? finish() : set({ step: step + 1 }));
  const back = () => step > 0 && set({ step: step - 1 });
  // the card: beside the control where there's room (below, above, then the left), else the middle
  const W = Math.min(360, size.w - 32);
  const H = 190;
  let left = (size.w - W) / 2;
  let top = (size.h - H) / 2;
  if (box) {
    const big = box.w > size.w * 0.5 && box.h > size.h * 0.4;
    if (big) {
      left = box.x + 24;
      top = box.y + 24;
    } else if (box.y + box.h + 12 + H < size.h) {
      top = box.y + box.h + 12;
      left = box.x + box.w / 2 - W / 2;
    } else if (box.y - 12 - H > 0) {
      top = box.y - 12 - H;
      left = box.x + box.w / 2 - W / 2;
    } else if (box.x + box.w + 12 + W < size.w) {
      top = Math.max(16, box.y);
      left = box.x + box.w + 12;
    } else {
      top = Math.max(16, box.y);
      left = box.x - W - 12;
    }
    left = Math.max(16, Math.min(left, size.w - W - 16));
    top = Math.max(16, Math.min(top, size.h - H - 16));
  }
  return (
    <div className="st-tour" data-testid="studio-tour">
      {box ? <div className="st-tour-ring" style={{ left: box.x - 4, top: box.y - 4, width: box.w + 8, height: box.h + 8 }} aria-hidden /> : <div className="st-tour-dim" aria-hidden />}
      <div className="st-tour-card" role="dialog" aria-modal="false" aria-labelledby="st-tour-title" style={{ left, top, width: W }}>
        <p className="st-muted st-tour-count">
          {step + 1} of {TOUR_STEPS.length}
        </p>
        <h3 id="st-tour-title">{s.title}</h3>
        <p>{s.text}</p>
        <div className="st-tour-actions">
          <button type="button" className="btn small" onClick={() => finish()}>
            {last ? "Close" : "Skip the tour"}
          </button>
          <span className="st-grow" />
          {step > 0 && (
            <button type="button" className="btn small" onClick={back}>
              Back
            </button>
          )}
          <button type="button" className="btn small primary" onClick={next} autoFocus>
            {last ? "Done" : "Next"}
          </button>
        </div>
      </div>
    </div>
  );
}
