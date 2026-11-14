// The "?" menu next to Find: node help, the keyboard shortcuts and the Studio tour.

import { useEffect, useRef, useState } from "react";

export function HelpMenu({ onNodeHelp, onTour }: { onNodeHelp: () => void; onTour?: () => void }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    box.current?.querySelector<HTMLButtonElement>(".st-help-menu-list button")?.focus();
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  const pick = (f: () => void) => () => {
    setOpen(false);
    f();
  };
  return (
    <div className="st-help-menu" ref={box}>
      <button type="button" className="btn" data-testid="studio-help-menu" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)} title="Help: the nodes, keyboard shortcuts and a tour" aria-label="Help">
        ?
      </button>
      {open && (
        <div className="st-help-menu-list" role="menu">
          <button type="button" role="menuitem" onClick={pick(onNodeHelp)}>
            Node help <kbd>F1</kbd>
          </button>
          <button type="button" role="menuitem" onClick={pick(() => document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "?", bubbles: true })))}>
            Keyboard shortcuts <kbd>?</kbd>
          </button>
          <a role="menuitem" className="st-help-menu-link" href="https://lonerstrike.com/docs#game-mode-studio" target="_blank" rel="noreferrer" onClick={() => setOpen(false)}>
            Studio guide (online)
          </a>
          <a role="menuitem" className="st-help-menu-link" href="https://lonerstrike.com/docs#studio-why-not" target="_blank" rel="noreferrer" onClick={() => setOpen(false)}>
            Why won't it compile?
          </a>
          {onTour && (
            <button type="button" role="menuitem" onClick={pick(onTour)}>
              Take the Studio tour
            </button>
          )}
        </div>
      )}
    </div>
  );
}
