// The Inspector's right-click menu (Copy / Paste on a Transform row, a key's field or the
// properties), drawn like the Outline's: one at a time, Esc or a click away closes it.

import { useEffect, type MouseEvent as RMouseEvent } from "react";
import { create } from "zustand";
import { showKey } from "./Shortcuts";

export type MenuItem = { label: string; run: () => void; disabled?: boolean; keys?: string; tip?: string } | null;
const useMenu = create<{ at: { x: number; y: number; items: MenuItem[] } | null; set: (m: { x: number; y: number; items: MenuItem[] } | null) => void }>((set) => ({ at: null, set: (at) => set({ at }) }));

/** Opens the menu at the pointer (a right-click handler). */
export function openInspectMenu(ev: RMouseEvent, items: MenuItem[]) {
  ev.preventDefault();
  ev.stopPropagation();
  useMenu.getState().set({ x: ev.clientX, y: ev.clientY, items });
}

export function InspectMenu() {
  const at = useMenu((m) => m.at);
  const close = () => useMenu.getState().set(null);
  useEffect(() => {
    if (!at) return;
    const k = (e: KeyboardEvent) => e.key === "Escape" && (e.stopPropagation(), close());
    window.addEventListener("keydown", k, true);
    return () => window.removeEventListener("keydown", k, true);
  }, [at]);
  if (!at) return null;
  return (
    <div className="ed-ctx-back" onClick={close} onContextMenu={(e) => (e.preventDefault(), close())}>
      <ul className="ed-ctx" role="menu" aria-label="Copy and paste" style={{ left: Math.min(at.x, window.innerWidth - 260), top: Math.min(at.y, window.innerHeight - 40 - at.items.length * 28) }} onClick={(e) => e.stopPropagation()}>
        {at.items.map((it, i) =>
          it === null ? (
            <li key={`s${i}`} className="sep" role="separator" />
          ) : (
            <li key={it.label} role="none">
              <button
                type="button"
                role="menuitem"
                disabled={it.disabled}
                title={it.tip}
                onClick={() => {
                  close();
                  it.run();
                }}
              >
                {it.label}
                {it.keys && <kbd className="ed-ctx-keys">{showKey(it.keys)}</kbd>}
              </button>
            </li>
          ),
        )}
      </ul>
    </div>
  );
}
