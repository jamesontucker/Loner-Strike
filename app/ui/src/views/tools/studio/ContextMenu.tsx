// The Studio's own right-click menu (the browser's never shows in the Studio, except in text
// boxes). One look and one behaviour everywhere: it opens at the pointer (kept on screen), the
// first item has the keyboard, ↑ ↓ Home End move, Enter or Space picks, Escape, a click
// elsewhere, scrolling or leaving the window closes it, and the keyboard goes back to where it was.

import { useEffect, useLayoutEffect, useRef, useState } from "react";

export type MenuItem = {
  label: string;
  help?: string;
  /** a key that does the same, shown on the right */
  keys?: string;
  danger?: boolean;
  /** greyed out, with why in `help` */
  disabled?: boolean;
  run: () => void;
};
export type MenuAt = { x: number; y: number; items: (MenuItem | "-")[]; label: string };

/** Opens with `open(e, label, items)` from an onContextMenu; render `menu` once. */
export function useContextMenu() {
  const [at, setAt] = useState<MenuAt | null>(null);
  const open = (e: { clientX: number; clientY: number; preventDefault: () => void; stopPropagation: () => void }, label: string, items: (MenuItem | "-")[]) => {
    e.preventDefault();
    e.stopPropagation();
    setAt({ x: e.clientX, y: e.clientY, items, label });
  };
  /** from the keyboard (Shift+F10 or the menu key): at the element */
  const openAt = (el: HTMLElement, label: string, items: (MenuItem | "-")[]) => {
    const r = el.getBoundingClientRect();
    setAt({ x: r.left + 12, y: r.bottom - 4, items, label });
  };
  return { open, openAt, close: () => setAt(null), menu: at && <ContextMenu at={at} onClose={() => setAt(null)} /> };
}

/** Shift+F10 or the context-menu key: the keyboard's right-click. */
export const isMenuKey = (e: { key: string; shiftKey: boolean }) => e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey);

export function ContextMenu({ at, onClose }: { at: MenuAt; onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: at.x, top: at.y });
  const back = useRef<HTMLElement | null>(typeof document !== "undefined" ? (document.activeElement as HTMLElement | null) : null);
  const close = (refocus = true) => {
    onClose();
    if (refocus) back.current?.focus?.();
  };
  // kept inside the window
  useLayoutEffect(() => {
    const b = box.current?.getBoundingClientRect();
    if (!b) return;
    setPos({ left: Math.max(4, Math.min(at.x, window.innerWidth - b.width - 4)), top: Math.max(4, Math.min(at.y, window.innerHeight - b.height - 4)) });
    box.current?.querySelector<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])')?.focus();
  }, [at]);
  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) close(false);
    };
    const gone = () => close(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("blur", gone);
    window.addEventListener("resize", gone);
    window.addEventListener("wheel", gone, { passive: true });
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("blur", gone);
      window.removeEventListener("resize", gone);
      window.removeEventListener("wheel", gone);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const items = () => [...(box.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])') ?? [])];
  const onKey = (e: React.KeyboardEvent) => {
    const list = items();
    const i = list.indexOf(document.activeElement as HTMLElement);
    const go = (n: number) => {
      e.preventDefault();
      list[(n + list.length) % list.length]?.focus();
    };
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(list.length - 1);
    else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  };
  return (
    <div
      ref={box}
      className="st-ctx"
      style={pos}
      role="menu"
      aria-label={at.label}
      onKeyDown={onKey}
      onContextMenu={(e) => e.preventDefault()}
    >
      {at.items.map((it, k) =>
        it === "-" ? (
          <div key={k} className="st-ctx-sep" role="separator" />
        ) : (
          <button
            key={k}
            type="button"
            role="menuitem"
            aria-disabled={it.disabled || undefined}
            tabIndex={-1}
            className={`st-ctx-item${it.danger ? " danger" : ""}`}
            onClick={() => {
              if (it.disabled) return;
              close();
              it.run();
            }}
          >
            <span className="st-ctx-label">{it.label}</span>
            {it.keys && <kbd className="st-ctx-keys">{it.keys}</kbd>}
            {it.help && <span className="st-ctx-help">{it.help}</span>}
          </button>
        ),
      )}
    </div>
  );
}

/** Text boxes keep the system's menu (copy, paste); everything else in the Studio gets ours or none. */
export const nativeMenuOk = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || t.tagName === "TEXTAREA" || (t.tagName === "INPUT" && !["checkbox", "radio", "range", "color", "button", "submit", "file"].includes((t as HTMLInputElement).type)));
