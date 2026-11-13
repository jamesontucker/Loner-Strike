// A button that opens a small menu of actions (the action bar's "More", or any tool's own).
// The menu goes in document.body above everything, flips above/below and shifts to stay on
// screen (ui.tsx placePopup), and works from the keyboard: arrows, Home/End, Enter, Esc.
//
//   <MenuButton label="More" items={[{ label: "Export…", onClick: doExport }, { label: "Reset…", danger: true, onClick }]} />

import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { placePopup } from "../../../ui";
import "./ux.css";

export type MenuItem = {
  label: ReactNode;
  onClick: () => unknown;
  disabled?: boolean;
  danger?: boolean;
  /** a line under the label */
  note?: ReactNode;
  title?: string;
  /** a line above this item */
  separator?: boolean;
  hidden?: boolean;
};

export function MenuButton({ label, items, icon = true, side = "bottom", align = "end", className = "btn", disabled, title }: {
  label: ReactNode;
  items: MenuItem[];
  /** the "⋯" dots before the label */
  icon?: boolean;
  side?: "top" | "bottom";
  align?: "start" | "end";
  className?: string;
  disabled?: boolean;
  title?: string;
}) {
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const shown = items.filter((i) => !i.hidden);

  useLayoutEffect(() => {
    if (!open || !btn.current || !list.current) return;
    placePopup(btn.current.getBoundingClientRect(), list.current, side, align, 4);
    list.current.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus();
  }, [open, side, align]);

  useEffect(() => {
    if (!open) return;
    const away = (e: Event) => {
      const t = e.target as Node;
      if (!list.current?.contains(t) && !btn.current?.contains(t)) setOpen(false);
    };
    const close = () => setOpen(false);
    document.addEventListener("pointerdown", away);
    window.addEventListener("resize", close);
    document.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("pointerdown", away);
      window.removeEventListener("resize", close);
      document.removeEventListener("scroll", close, true);
    };
  }, [open]);

  const keys = (e: KeyboardEvent) => {
    const all = [...(list.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? [])];
    const at = all.indexOf(document.activeElement as HTMLButtonElement);
    const to = (i: number) => all[(i + all.length) % all.length]?.focus();
    if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      btn.current?.focus();
    } else if (e.key === "ArrowDown") (e.preventDefault(), to(at + 1));
    else if (e.key === "ArrowUp") (e.preventDefault(), to(at - 1));
    else if (e.key === "Home") (e.preventDefault(), to(0));
    else if (e.key === "End") (e.preventDefault(), to(all.length - 1));
  };

  return (
    <>
      <button
        ref={btn}
        type="button"
        className={`${className} tl-menu-btn`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        disabled={disabled || !shown.length}
        title={title}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {icon && <Dots />}
        {label}
      </button>
      {open &&
        createPortal(
          <div className="tl-menu" role="menu" id={id} ref={list} onKeyDown={keys}>
            {shown.map((it, i) => (
              <div key={i} role="none" className={it.separator && i > 0 ? "tl-menu-sep" : undefined}>
                <button
                  type="button"
                  role="menuitem"
                  className={`tl-menu-item${it.danger ? " danger" : ""}`}
                  disabled={it.disabled}
                  title={it.title}
                  onClick={() => {
                    setOpen(false);
                    btn.current?.focus();
                    it.onClick();
                  }}
                >
                  <span>{it.label}</span>
                  {it.note && <span className="tl-menu-note">{it.note}</span>}
                </button>
              </div>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}

const Dots = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <circle cx="5" cy="12" r="1.8" />
    <circle cx="12" cy="12" r="1.8" />
    <circle cx="19" cy="12" r="1.8" />
  </svg>
);
