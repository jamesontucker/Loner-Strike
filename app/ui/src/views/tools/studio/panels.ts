// Which side panels show (My mode on the left, Details on the right): each can be folded to a thin
// rail, remembered in this browser. In a window under 1200 px wide, Details starts folded (UX
// review H4).

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
  const wide = typeof window === "undefined" || window.innerWidth >= 1200;
  return { left: true, right: wide, minimap: false };
}

export const usePanels = create<Panels & { set: (p: Partial<Panels>) => void }>(
  (set, get) => ({
    ...read(),
    set: (p) => {
      set(p);
      try {
        const { left, right, minimap } = get();
        localStorage.setItem(KEY, JSON.stringify({ left, right, minimap }));
      } catch {
        /* private window: for this visit */
      }
    },
  }),
);
