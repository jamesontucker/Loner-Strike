// P5: how the 3D view moves. Three presets:
//   Unreal  (default)  hold right mouse: look, WASD/QE fly, scroll changes speed; Alt+left orbit,
//                      Alt+right dolly, middle pan, left-drag on empty space moves forward and turns,
//                      scroll moves forward/back, F focuses what's picked
//   Blender            orbit with left-drag, right-drag pans, scroll zooms, F for fly (the old way)
//   Hammer             Z turns mouse-look on/off (WASD while on), left-drag on empty space looks
//                      (WASD/QE fly while the left button is held, like J.A.C.K.),
//                      right-drag moves forward/back, middle pan, scroll forward/back
// Kept on this computer; chosen once on first open (View menu later).

import { create } from "zustand";

export type NavPreset = "unreal" | "blender" | "hammer";
export const NAV_PRESETS: { id: NavPreset; label: string; tip: string }[] = [
  { id: "unreal", label: "Unreal", tip: "Hold right mouse to look and WASD to fly, scroll for speed; Alt+drag to orbit" },
  { id: "blender", label: "Blender", tip: "Drag to orbit, right-drag to pan, scroll to zoom; F to fly" },
  { id: "hammer", label: "Hammer", tip: "Hold left mouse and WASD to fly, or Z for mouse-look; right-drag to move" },
];

/** The camera speed steps (Unreal's 1-8), units per second at each. */
export const SPEEDS = [64, 128, 256, 512, 1024, 2048, 4096, 8192];

type NavState = {
  preset: NavPreset;
  /** chosen by the person (else the first-run question shows) */
  chosen: boolean;
  speed: number;
  /** a short "Speed 4" note in the view, until this time */
  hudUntil: number;
  /** right mouse (Unreal) or Z (Hammer) look is on: keys go to the camera */
  looking: boolean;
  /** Hammer: that look is a left-mouse hold (it ends when the button comes up, not with Z) */
  held: boolean;
  /** big maps fly faster: 1 up to 4 (a map's size / 3000 units), times the speed step */
  mapScale: number;
  /** the "how to move" hint has done its job (flown once, or closed) */
  hintDone: boolean;
  set: (p: Partial<Omit<NavState, "set">>) => void;
};

const KEY = "ls.editor.nav";
function load(): Pick<NavState, "preset" | "chosen" | "speed"> {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    const preset = NAV_PRESETS.some((p) => p.id === v?.preset) ? (v.preset as NavPreset) : "unreal";
    const speed = Number.isInteger(v?.speed) && v.speed >= 0 && v.speed < SPEEDS.length ? v.speed : 3;
    return { preset, chosen: !!v?.preset, speed };
  } catch {
    return { preset: "unreal", chosen: false, speed: 3 };
  }
}

export const useNav = create<NavState>((set, get) => ({
  ...load(),
  hudUntil: 0,
  looking: false,
  held: false,
  mapScale: 1,
  hintDone: (() => {
    try {
      return localStorage.getItem("ls.editor.navHint") === "1";
    } catch {
      return false;
    }
  })(),
  set: (p) => {
    set(p);
    if ("hintDone" in p) {
      try {
        if (p.hintDone) localStorage.setItem("ls.editor.navHint", "1");
        else localStorage.removeItem("ls.editor.navHint");
      } catch {
        /* private window */
      }
    }
    if ("preset" in p || "speed" in p) {
      const s = get();
      try {
        localStorage.setItem(KEY, JSON.stringify({ preset: s.preset, speed: s.speed }));
      } catch {
        /* private window */
      }
    }
  },
}));

export function setNavPreset(preset: NavPreset) {
  useNav.getState().set({ preset, chosen: true, looking: false });
}

/** Units per second now: the step's speed times the map's size factor. */
export const flySpeed = () => SPEEDS[useNav.getState().speed] * useNav.getState().mapScale;

/** One speed step up or down, with the note in the view. */
export function stepSpeed(d: number) {
  const s = useNav.getState();
  s.set({ speed: Math.max(0, Math.min(SPEEDS.length - 1, s.speed + d)), hudUntil: performance.now() + 1200 });
}

/** The keys the presets add to the key map (Shortcuts.ts puts them under your own keys). */
export const NAV_KEYS: Record<NavPreset, Record<string, string[]>> = {
  unreal: { "view.frame": ["F"], "view.fly": [] },
  blender: {},
  hammer: { "view.fly": [] },
};

/** What the cheat sheet says for each preset. */
export const NAV_HELP: Record<NavPreset, [string, string][]> = {
  unreal: [
    ["Right mouse (hold) + mouse", "Look around"],
    ["Right mouse + W A S D", "Fly (Q down, E up, Shift faster)"],
    ["Right mouse + scroll", "Camera speed (shown in the view)"],
    ["Left-drag on empty space", "Move forward/back and turn"],
    ["Alt + left-drag", "Orbit round what's picked (or the point ahead)"],
    ["Alt + right-drag", "Move closer / further"],
    ["Middle-drag", "Pan"],
    ["Scroll", "Move forward / back"],
    ["F", "Frame selection"],
  ],
  blender: [
    ["Left-drag", "Orbit"],
    ["Right-drag", "Pan"],
    ["Scroll", "Zoom"],
    ["F", "Fly camera (WASD, Esc lets go)"],
  ],
  hammer: [
    ["Z", "Mouse-look on / off (W A S D to move while on)"],
    ["Left-drag on empty space", "Look around"],
    ["Right-drag", "Move forward / back"],
    ["Middle-drag", "Pan"],
    ["Scroll", "Move forward / back"],
  ],
};
