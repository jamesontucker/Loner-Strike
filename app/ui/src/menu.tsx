// A button that opens a short list of actions under it: the quick bar's menus and each
// player's actions. Escape closes it and focus goes back to the button; arrow keys move
// through the items. The list is placed against the window and scrolls by itself when
// it's taller than the room there.

import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { zoom } from "./ui";

export type MenuItem = {
  label: string;
  on: () => void;
  /** a checkbox item (ticked or not) */
  pressed?: boolean;
  disabled?: boolean;
  show?: boolean;
  title?: string;
  /** its action is running: a spinner and `busyLabel`, and it can't be picked again */
  busy?: boolean;
  busyLabel?: string;
  danger?: boolean;
};
export type MenuGroup = { title?: string; items: MenuItem[]; note?: ReactNode };

export function Menu({
  label,
  items,
  groups,
  active,
  ariaLabel,
  className = "qb",
  caret = true,
  wide,
}: {
  label: ReactNode;
  items?: MenuItem[];
  /** items under headings, in place of `items` */
  groups?: MenuGroup[];
  active?: boolean;
  ariaLabel?: string;
  /** the button's classes */
  className?: string;
  caret?: boolean;
  /** a longer list (wider, with headings) */
  wide?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const id = useId();
  const all: MenuGroup[] = (groups ?? [{ items: items ?? [] }]).map((g) => ({ ...g, items: g.items.filter((i) => i.show !== false) })).filter((g) => g.items.length || g.note);
  const shown = all.flatMap((g) => g.items);
  const running = shown.find((i) => i.busy);

  // keep the list on screen: under the button (over it when there's more room there),
  // moved left if it would run off the edge, and no taller than the window. It's drawn
  // over the whole page (a portal), so nothing around the button can cut it off or cover it.
  const place = () => {
    const l = list.current;
    const b = button.current;
    if (!l || !b) return;
    const z = zoom();
    const r = b.getBoundingClientRect();
    const height = window.innerHeight / z;
    const width = window.innerWidth / z;
    const btn = { left: r.left / z, top: r.top / z, bottom: r.bottom / z };
    const below = height - btn.bottom - 12;
    const above = btn.top - 12;
    l.style.maxHeight = "";
    const up = l.scrollHeight > below && above > below;
    l.style.maxHeight = `${Math.max(160, up ? above : below)}px`;
    l.style.left = `${Math.max(8, Math.min(btn.left, width - 8 - l.offsetWidth))}px`;
    l.style.top = `${up ? Math.max(8, btn.top - 4 - l.offsetHeight) : btn.bottom + 4}px`;
  };
  useLayoutEffect(() => {
    if (!open) return;
    place();
    list.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [open]);
  const inside = (t: EventTarget | null) => !!t && (!!box.current?.contains(t as Node) || !!list.current?.contains(t as Node));
  const close = () => {
    setOpen(false);
    button.current?.focus();
  };
  useEffect(() => {
    if (!open) return;
    const away = (e: Event) => !inside(e.target) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close();
    // the page (or the quick bar) scrolling: the list follows its button
    const moved = (e: Event) => !list.current?.contains(e.target as Node) && place();
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    document.addEventListener("scroll", moved, true);
    window.addEventListener("resize", place);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
      document.removeEventListener("scroll", moved, true);
      window.removeEventListener("resize", place);
    };
  }, [open]);
  // up and down (and Home, End) move between the items; Tab out closes it
  const keys = (e: ReactKeyboardEvent) => {
    const buttons = [...(list.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const to = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: buttons.length - 1 }[e.key];
    if (to === undefined || !buttons.length) return;
    e.preventDefault();
    buttons[(to + buttons.length) % buttons.length].focus();
  };
  if (!all.length) return null;
  return (
    <div
      className="qmenu"
      ref={box}
      // Tab out of the list closes it (the list is in a portal, but React still passes its events here)
      onBlur={(e) => open && e.relatedTarget && !inside(e.relatedTarget) && setOpen(false)}
    >
      <button
        ref={button}
        type="button"
        className={className}
        aria-label={ariaLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-pressed={active || undefined}
        aria-busy={running ? true : undefined}
        onClick={() => setOpen((o) => !o)}
      >
        {running ? (
          <>
            <span className="spinner" aria-hidden="true" />
            {running.busyLabel ?? "Sending…"}
          </>
        ) : (
          <>
            {label}
            {caret && <> <span className="caret" aria-hidden="true">▾</span></>}
          </>
        )}
      </button>
      {open && createPortal(
        <div className={`qmenu-list${wide ? " wide" : ""}`} role="menu" aria-label={ariaLabel} ref={list} onKeyDown={keys}>
          {all.map((g, n) => (
            <div key={g.title ?? n} role="group" aria-labelledby={g.title ? `${id}-g${n}` : undefined} aria-describedby={g.note ? `${id}-n${n}` : undefined} className="qmenu-group">
              {g.title && <div className="qmenu-head" id={`${id}-g${n}`} role="presentation">{g.title}</div>}
              {g.items.map((i) => (
                <button
                  key={i.label}
                  type="button"
                  role={i.pressed === undefined ? "menuitem" : "menuitemcheckbox"}
                  aria-checked={i.pressed}
                  aria-busy={i.busy || undefined}
                  className={i.danger ? "danger" : undefined}
                  disabled={i.disabled || i.busy}
                  title={i.title}
                  onClick={() => {
                    close();
                    i.on();
                  }}
                >
                  <span className="tick" aria-hidden="true">{i.busy ? <span className="spinner" /> : i.pressed ? "✓" : ""}</span>
                  {i.busy ? i.busyLabel ?? "Sending…" : i.label}
                </button>
              ))}
              {g.note && <div className="qmenu-note" id={`${id}-n${n}`} role="presentation">{g.note}</div>}
            </div>
          ))}
        </div>,
        document.body,
      )}
    </div>
  );
}
