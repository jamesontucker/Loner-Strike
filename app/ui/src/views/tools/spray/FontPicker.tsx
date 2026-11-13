// The lettering font picker: a button showing the font in its own face, opening a list with a
// search box: "Built-in" (the app's fonts, first) then "On this computer" (installed families,
// each shown in itself). Up/Down move, Enter picks, Esc closes. The list goes in document.body
// so the settings column's scrolling never cuts it off.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { placePopup } from "../../../ui";
import { FONTS, fontCss } from "./doc";
import { FALLBACK, fontState, isBuiltIn, listSystemFonts, loadSystemFont } from "./sysfonts";

type Item = { value: string; label: string; group: "builtin" | "system" };

const BUILT_IN: Item[] = FONTS.map(([value, label]) => ({ value, label, group: "builtin" }));

export function FontPicker({ value, onPick }: { value: string; onPick: (font: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const fonts = useQuery({ queryKey: ["tools", "spray", "fonts"], queryFn: listSystemFonts, staleTime: Infinity, enabled: open || !isBuiltIn(value) });
  const system = fonts.data?.fonts ?? [];

  // an installed family the design uses: load it (the canvas needs the file), or learn it's missing
  const [, redraw] = useState(0);
  useEffect(() => {
    if (!isBuiltIn(value)) void loadSystemFont(value).then(() => redraw((n) => n + 1));
  }, [value]);
  const missing = !isBuiltIn(value) && fontState(value) === "missing";

  const items = useMemo(() => {
    const words = q.trim().toLowerCase();
    const hit = (i: Item) => !words || i.label.toLowerCase().includes(words);
    return [...BUILT_IN.filter(hit), ...system.map((f): Item => ({ value: f.family, label: f.family, group: "system" })).filter(hit)];
  }, [q, system]);

  const label = isBuiltIn(value) ? BUILT_IN.find((b) => b.value === value)!.label : value;

  useLayoutEffect(() => {
    if (!open || !pop.current || !button.current) return;
    const place = () => pop.current && button.current && placePopup(button.current.getBoundingClientRect(), pop.current, "bottom", "start", 4);
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, items.length]);

  // start on the font in use
  useEffect(() => {
    if (!open) return;
    setQ("");
    setAt(Math.max(0, [...BUILT_IN, ...system.map((f) => ({ value: f.family }))].findIndex((i) => i.value === value)));
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    list.current?.querySelector<HTMLElement>(`[data-i="${at}"]`)?.scrollIntoView({ block: "nearest" });
  }, [at, open]);

  // a click outside closes it; Esc closes only the list (caught before the composer's keys,
  // wherever focus is), and focus goes into the search box
  useEffect(() => {
    if (!open) return;
    pop.current?.querySelector("input")?.focus();
    const down = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!pop.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      button.current?.focus();
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("keydown", esc, true);
    };
  }, [open]);

  const pick = (i: Item | undefined) => {
    if (!i) return;
    onPick(i.value);
    setOpen(false);
    button.current?.focus();
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") (e.preventDefault(), setAt((a) => Math.min(items.length - 1, a + 1)));
    else if (e.key === "ArrowUp") (e.preventDefault(), setAt((a) => Math.max(0, a - 1)));
    else if (e.key === "Enter") (e.preventDefault(), pick(items[at]));
  };

  const firstSystem = items.findIndex((i) => i.group === "system");
  return (
    <>
      <button
        ref={button}
        type="button"
        className="search sc-font-button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Font: ${label}`}
        onClick={() => setOpen((o) => !o)}
        style={{ fontFamily: fontCss(value) }}
      >
        <span className="sc-font-name">{label}</span>
        <span className="caret" aria-hidden="true">
          ▾
        </span>
      </button>
      {missing && (
        <small className="sc-font-missing" role="note">
          “{value}” isn't installed on this computer, so the words show in {FALLBACK}. Pick another font, or install it and open the design again.
        </small>
      )}
      {open &&
        createPortal(
          <div ref={pop} className="sc-font-pop" style={{ visibility: "hidden" }} onKeyDown={onKey}>
            <input
              className="search"
              autoFocus
              placeholder="Search fonts"
              aria-label="Search fonts"
              role="combobox"
              aria-expanded="true"
              aria-controls={`${id}-list`}
              aria-activedescendant={items[at] ? `${id}-${at}` : undefined}
              value={q}
              onChange={(e) => (setQ(e.target.value), setAt(0))}
            />
            <div ref={list} id={`${id}-list`} className="sc-font-list" role="listbox" aria-label="Fonts">
              {items.map((i, n) => (
                <div key={i.group + i.value} role="presentation">
                  {(n === 0 && i.group === "builtin") || n === firstSystem ? (
                    <div className="sc-font-group" role="presentation">
                      {i.group === "builtin" ? "Built-in" : "On this computer"}
                    </div>
                  ) : null}
                  <div
                    id={`${id}-${n}`}
                    data-i={n}
                    role="option"
                    aria-selected={i.value === value}
                    className={`sc-font-item${n === at ? " at" : ""}`}
                    style={{ fontFamily: fontCss(i.value) }}
                    onPointerMove={() => setAt(n)}
                    onClick={() => pick(i)}
                  >
                    {i.label}
                  </div>
                </div>
              ))}
              {firstSystem < 0 && (
                <>
                  <div className="sc-font-group" role="presentation">
                    On this computer
                  </div>
                  <p className="muted sc-font-note" role="status">
                    {fonts.isPending ? (
                      <>
                        <span className="spinner" aria-hidden="true" /> Looking for fonts…
                      </>
                    ) : fonts.data?.error || fonts.isError ? (
                      "Couldn't list this computer's fonts."
                    ) : q ? (
                      "No installed font matches."
                    ) : (
                      "No other fonts found."
                    )}
                  </p>
                </>
              )}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
