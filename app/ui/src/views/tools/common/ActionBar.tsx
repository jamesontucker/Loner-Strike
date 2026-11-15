// The one save bar for every tool that saves something. Pinned to the bottom of the tool's
// area (sticky: it sits after the content at the end of the page, so it never hides the last
// field, and a field focused behind it scrolls clear). One compact row: what's
// going on at the left (unsaved changes / saved ✓ / things to fix / working…), the actions at
// the right, Save last, everything else in a "More" menu, with "Reset to Loner-Strike's
// default…" at the end of it (the safety net's restore, with a sheet listing the files).
//
//   <ActionBar
//     dirty={dirty} errors={errors.length} working={working}
//     where="people.ini for cstrike" info="Saved into the game's people.ini…"
//     save={{ onClick: save }} undo={() => reset(st)}
//     actions={[{ label: "Apply to game", busyLabel: "Applying…", onClick: apply }]}
//     more={[{ label: "Import…", onClick: doImport }, { label: "Export…", onClick: doExport }]}
//     reset={{ tool: "people" }}
//   >
//     {errors.length > 0 && <ProblemList list={errors} />}   (shown under the row)
//   </ActionBar>

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Info } from "../../../ui";
import { MenuButton, type MenuItem } from "./Menu";
import { useResetDefaults } from "./resetDefaults";
import "./ux.css";

export type BarAction = {
  label: ReactNode;
  onClick: () => unknown;
  /** shown with a spinner while `working` equals it */
  busyLabel?: string;
  disabled?: boolean;
  danger?: boolean;
  title?: string;
  hidden?: boolean;
  /** (menu only) a line under the label */
  note?: ReactNode;
};

export type ActionBarProps = {
  /** there are changes not saved yet */
  dirty: boolean;
  /** how many things must be fixed before saving (Save is off while > 0) */
  errors?: number;
  /** the tool's useBusy() label while something runs: shown with a spinner, buttons wait */
  working?: string | null;
  /** where it saves, after the status ("people.ini for cstrike") */
  where?: ReactNode;
  /** a "?" with more about saving */
  info?: ReactNode;
  /** replaces the status words */
  status?: ReactNode;
  save?: { label?: ReactNode; busyLabel?: string; onClick: () => unknown; disabled?: boolean; title?: string } | null;
  /** "Undo changes", shown while dirty */
  undo?: (() => unknown) | null;
  /** buttons before Save (keep it to one or two) */
  actions?: BarAction[];
  /** the "More" menu */
  more?: BarAction[];
  /** "Reset to Loner-Strike's default…" at the end of More. `match`: the files (default: History's list for the tool) */
  reset?: { tool: string; match?: string[]; label?: string } | false;
  /** shown under the row (a list of problems) */
  children?: ReactNode;
  className?: string;
};

export function ActionBar({ dirty, errors = 0, working, where, info, status, save, undo, actions = [], more = [], reset, children, className = "" }: ActionBarProps) {
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    if (dirty) setSaved(false);
  }, [dirty]);
  const resetter = useResetDefaults(reset ? reset.tool : "", { match: reset ? reset.match : [], dirty, label: reset ? reset.label : undefined });
  const busy = !!working || !!resetter.working;
  const bar = useRef<HTMLDivElement>(null);
  // a field focused behind the bar (Tab, a search) scrolls up clear of it
  useEffect(() => {
    const onFocus = (e: FocusEvent) => {
      const t = e.target;
      const b = bar.current;
      if (!b || !(t instanceof HTMLElement) || b.contains(t)) return;
      requestAnimationFrame(() => {
        const r = t.getBoundingClientRect();
        const br = b.getBoundingClientRect();
        if (r.bottom > br.top && r.top < br.bottom) {
          let sc: HTMLElement | null = b.parentElement;
          while (sc && !/(auto|scroll)/.test(getComputedStyle(sc).overflowY)) sc = sc.parentElement;
          (sc ?? window).scrollBy({ top: r.bottom - br.top + 12 });
        }
      });
    };
    document.addEventListener("focusin", onFocus);
    return () => document.removeEventListener("focusin", onFocus);
  }, []);
  // toasts sit above the bar, never over Save: --tl-ab-h is how far the bar's top is from the
  // window's bottom (ux.css)
  useEffect(() => {
    const b = bar.current;
    const root = document.documentElement;
    if (!b) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const up = Math.max(0, Math.min(window.innerHeight, window.innerHeight - b.getBoundingClientRect().top));
        root.style.setProperty("--tl-ab-h", `${Math.ceil(up)}px`);
      });
    };
    measure();
    const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    ro?.observe(b);
    window.addEventListener("resize", measure);
    document.addEventListener("scroll", measure, { capture: true, passive: true });
    return () => {
      cancelAnimationFrame(frame);
      ro?.disconnect();
      window.removeEventListener("resize", measure);
      document.removeEventListener("scroll", measure, { capture: true });
      root.style.removeProperty("--tl-ab-h");
    };
  }, []);
  const saving = save?.busyLabel ?? "Saving…";

  const words = status ?? (
    busy ? (
      <span className="tl-ab-state working" role="status">
        <span className="spinner" aria-hidden="true" />
        {working || resetter.working}
      </span>
    ) : errors > 0 ? (
      <span className="tl-ab-state error">
        {errors} thing{errors === 1 ? "" : "s"} to fix before saving
      </span>
    ) : dirty ? (
      <span className="tl-ab-state dirty">
        <span className="tl-ab-dot" aria-hidden="true" />
        Unsaved changes
      </span>
    ) : saved ? (
      <span className="tl-ab-state saved">
        <Tick /> Saved
      </span>
    ) : (
      <span className="tl-ab-state">No unsaved changes</span>
    )
  );

  const menu: MenuItem[] = more
    .filter((a) => !a.hidden)
    .map((a) => ({ label: a.label, onClick: a.onClick, disabled: a.disabled || busy, danger: a.danger, title: a.title, note: a.note }));
  if (reset && resetter.available) menu.push({ label: resetter.label, onClick: resetter.open, disabled: busy, danger: true, separator: menu.length > 0 });

  return (
    <div ref={bar} className={`tl-actionbar ${className}`} role="region" aria-label="Save">
      <div className="tl-ab-row">
        <div className="tl-ab-status" aria-live="polite">
          {words}
          {where && <span className="tl-ab-where muted">{where}</span>}
          {info && <Info text={info} />}
        </div>
        <div className="tl-ab-actions">
          {undo && dirty && (
            <button type="button" className="btn" disabled={busy} onClick={() => undo()}>
              Undo changes
            </button>
          )}
          {actions
            .filter((a) => !a.hidden)
            .map((a, i) => (
              <BarButton key={i} a={a} working={working} busy={busy} />
            ))}
          {menu.length > 0 && <MenuButton label="More" items={menu} side="top" align="end" title="More actions" />}
          {save && (
            <BarButton
              primary
              a={{
                label: save.label ?? "Save",
                busyLabel: saving,
                title: save.title,
                disabled: save.disabled ?? (!dirty || errors > 0),
                onClick: async () => {
                  await save.onClick();
                  setSaved(true);
                },
              }}
              working={working}
              busy={busy}
            />
          )}
        </div>
      </div>
      {children && <div className="tl-ab-more">{children}</div>}
      {resetter.sheet}
    </div>
  );
}

function BarButton({ a, working, busy, primary }: { a: BarAction; working?: string | null; busy: boolean; primary?: boolean }) {
  const mine = !!a.busyLabel && working === a.busyLabel;
  return (
    <button type="button" className={`btn${primary ? " primary" : ""}${a.danger ? " danger" : ""}`} disabled={busy || a.disabled} aria-busy={mine || undefined} title={a.title} onClick={() => a.onClick()}>
      {mine ? (
        <>
          <span className="spinner" aria-hidden="true" />
          {a.busyLabel}
        </>
      ) : (
        a.label
      )}
    </button>
  );
}

const Tick = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M4.5 12.5l5 5 10-11" />
  </svg>
);
