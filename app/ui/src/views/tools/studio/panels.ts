// Which side panels show (My mode on the left, Details on the right): each can be folded to a thin
// rail, remembered in this browser. In a window under 1200 px wide, Details starts folded (UX
// review H4); there the panels are narrower (studio.css), opening one folds the other, and a window
// made narrow with both open folds Details (at 1024 x 680 with both the canvas was about 320 px).

import { create } from "zustand";

const KEY = "ls.studio.panels";
type Panels = {
  left: boolean;
  right: boolean;
  /** the canvas minimap, off unless asked for (UX review M1) */ minimap: boolean;
};

function read(): Panels {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && typeof v.left === "boolean" && typeof v.right === "boolean")
      return { minimap: false, ...v };
  } catch {
    /* private window */
  }
  return { left: true, right: !narrow(), minimap: false };
}

const narrow = () => typeof window !== "undefined" && window.innerWidth < 1200;

/** What showing a panel does in a narrow window: one at a time (both asked for at once stays both: the tour). */
export function narrowed(cur: { left: boolean; right: boolean }, p: Partial<Panels>, isNarrow: boolean): Partial<Panels> {
  if (!isNarrow) return p;
  if (p.left === true && p.right === undefined && cur.right) return { ...p, right: false };
  if (p.right === true && p.left === undefined && cur.left) return { ...p, left: false };
  return p;
}

export const usePanels = create<Panels & { set: (p: Partial<Panels>) => void }>(
  (set, get) => ({
    ...read(),
    set: (p) => {
      set(narrowed(get(), p, narrow()));
      try {
        const { left, right, minimap } = get();
        localStorage.setItem(KEY, JSON.stringify({ left, right, minimap }));
      } catch {
        /* private window: for this visit */
      }
    },
  }),
);

// a window made narrow with both panels open: Details folds (for this visit)
if (typeof window !== "undefined")
  window.addEventListener("resize", () => {
    const st = usePanels.getState();
    if (narrow() && st.left && st.right) usePanels.setState({ right: false });
  });
