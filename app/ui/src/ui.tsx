import { Component, createContext, lazy, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ComponentType, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cmd, options, sameValue } from "./api";

// ---- dialogs -------------------------------------------------------------------------

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
let openSheets = 0;

// ---- the back button (phones, and the browser) ----------------------------------------
// Each open sheet has its own history entry, so Back closes the top one. App.tsx's
// popstate handler asks closeTopSheet() first, then goes back a page.

type OpenSheet = { id: number; close: () => void };
const sheetStack: OpenSheet[] = [];
let nextSheet = 1;
/** History moved to an entry over sheet `landed` (0: no sheet): every sheet opened after
 *  it closes (true), or there was none (false). Works from the entry alone, so a Back we
 *  caused ourselves (a sheet taking its entry off again) closes nothing extra. */
export function closeSheetsAbove(landed: number) {
  const above = sheetStack.filter((s) => s.id > landed);
  above.reverse().forEach((s) => s.close());
  return above.length > 0;
}
/** A sheet's history entry carries the page it's over, so going back to it later (once the
 *  sheet is gone) just lands on that page. */
function pushSheetEntry(id: number) {
  try {
    history.pushState({ ...(history.state ?? {}), ls: 1, sheet: id }, "");
  } catch {
    /* history unavailable: Back just won't close sheets */
  }
}

/** A dialog over the app: Escape and the backdrop close it (unless `onClose` is left
 *  out), focus moves into it and back when it closes, Tab stays inside, and the app
 *  behind can't be clicked or reached by screen readers. */
export function Sheet({
  label,
  onClose,
  children,
  className = "sheet",
  bottom,
  keepApp,
  role = "dialog",
  describedBy,
}: {
  label: string;
  onClose?: () => void;
  children: ReactNode;
  className?: string;
  /** a sheet that slides up from the bottom (the phone's More menu) */
  bottom?: boolean;
  /** the app behind stays usable (the phone's More menu: the tab bar still works) */
  keepApp?: boolean;
  role?: "dialog" | "alertdialog";
  describedBy?: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  // where focus was before the sheet: read while rendering, because React focuses an
  // autoFocus field inside the sheet before any effect runs
  const [back] = useState(() => document.activeElement as HTMLElement | null);
  // a backdrop click closes only if the press started there (not a drag out of a field)
  const downOnBackdrop = useRef(false);
  useEffect(() => {
    const root = document.getElementById("root");
    if (!keepApp) {
      openSheets++;
      root?.setAttribute("inert", "");
    }
    // Back closes it: its own history entry, and a place on the stack Back looks at
    const id = nextSheet++;
    const entry: OpenSheet = {
      id,
      close: () => {
        if (close.current) close.current();
        // can't be closed: it keeps its entry
        else pushSheetEntry(id);
      },
    };
    sheetStack.push(entry);
    pushSheetEntry(id);
    // focus the first field (or button) unless something inside already asked for it
    const el = box.current;
    if (el && !el.contains(document.activeElement)) (el.querySelector<HTMLElement>(FOCUSABLE) ?? el).focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && close.current) {
        e.stopPropagation();
        close.current();
      }
      if (e.key === "Tab" && el) {
        const all = [...el.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => x.offsetParent !== null);
        if (!all.length) return;
        const first = all[0];
        const last = all[all.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    el?.addEventListener("keydown", key);
    return () => {
      el?.removeEventListener("keydown", key);
      if (!keepApp && --openSheets === 0) root?.removeAttribute("inert");
      if (back?.isConnected) back.focus();
      const at = sheetStack.indexOf(entry);
      if (at >= 0) sheetStack.splice(at, 1);
      // closed by its own button (its entry is still the current one): take the entry off
      // again, so Back afterwards goes where it should
      if ((history.state as { sheet?: number } | null)?.sheet === id) history.back();
    };
  }, []);
  return createPortal(
    <div
      className={`sheet-backdrop${bottom ? " bottom" : ""}`}
      onPointerDown={(e) => (downOnBackdrop.current = e.target === e.currentTarget)}
      onClick={(e) => e.target === e.currentTarget && downOnBackdrop.current && close.current?.()}
    >
      <div ref={box} className={className} role={role} aria-modal="true" aria-label={label} aria-describedby={describedBy} tabIndex={-1} onClick={(e) => e.stopPropagation()}>
        {children}
      </div>
    </div>,
    document.body,
  );
}

/** A load that failed: what went wrong and a way to try again. */
export function LoadError({ text, onRetry, title }: { text: string; onRetry?: () => void; title?: string }) {
  return (
    <div className="load-error" role="alert">
      {title && <h3>{title}</h3>}
      <p>{text}</p>
      {onRetry && <button className="btn" onClick={onRetry}>Try again</button>}
    </div>
  );
}

// ---- pages loaded on first open ---------------------------------------------------

/** React.lazy for a page, which reloads the app once if its file can't be loaded (a
 *  phone left open across an app update asks for files that are gone). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function lazyPage<T extends ComponentType<any>>(load: () => Promise<{ default: T }>) {
  return lazy(() =>
    load().then((m) => {
      // loaded fine: a later update may reload again
      try {
        sessionStorage.removeItem("ls.reloaded");
      } catch {
        /* private window */
      }
      return m;
    }).catch((e) => {
      let tried = false;
      try {
        tried = sessionStorage.getItem("ls.reloaded") === "1";
        sessionStorage.setItem("ls.reloaded", "1");
      } catch {
        /* private window: no reload loop guard, so don't reload */
        tried = true;
      }
      if (!tried) location.reload();
      throw e;
    }),
  );
}

/** Catches a page that failed to load or crashed while drawing, so the rest of the app
 *  stays usable: says so, with a way to reload. */
export class PageBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return <LoadError title="This page didn't load" text="Loner-Strike may have been updated since this page was opened, or the connection dropped." onRetry={() => location.reload()} />;
  }
}

// ---- asking before something can't be undone --------------------------------------
type AskOpts = { ok?: string; danger?: boolean };
/** A text box under the question (optional unless `required`). */
type AskInput = { label: string; placeholder?: string; max?: number };
type Ask = { text: string; ok: string; danger: boolean; input?: AskInput; done: (answer: string | null) => void };
type AskFns = {
  ask: (text: string, opts?: AskOpts) => Promise<boolean>;
  askText: (text: string, input: AskInput, opts?: AskOpts) => Promise<string | null>;
};
const AskCtx = createContext<AskFns>({ ask: async () => false, askText: async () => null });
/** In-app replacement for window.confirm: `if (!(await ask("Delete it?", { ok: "Delete", danger: true }))) return;` */
export const useConfirm = () => useContext(AskCtx).ask;
/** The same question with a text box: resolves with what was typed (maybe ""), or null when cancelled. */
export const useAskText = () => useContext(AskCtx).askText;

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [q, setQ] = useState<Ask | null>(null);
  const [typed, setTyped] = useState("");
  const open = useCallback(
    (text: string, opts: AskOpts | undefined, input?: AskInput) =>
      new Promise<string | null>((resolve) => {
        setTyped("");
        setQ((old) => {
          old?.done(null);
          return { text, ok: opts?.ok ?? "OK", danger: !!opts?.danger, input, done: resolve };
        });
      }),
    [],
  );
  const fns = useMemo<AskFns>(
    () => ({
      ask: (text, opts) => open(text, opts).then((a) => a !== null),
      askText: (text, input, opts) => open(text, opts, input),
    }),
    [open],
  );
  const close = (yes: boolean) => {
    q?.done(yes ? typed.trim() : null);
    setQ(null);
  };
  useEffect(() => {
    if (!q) return;
    // a question about one page or server never carries over to the next
    const away = () => close(false);
    window.addEventListener("ls-target", away);
    window.addEventListener("ls-nav", away);
    return () => {
      window.removeEventListener("ls-target", away);
      window.removeEventListener("ls-nav", away);
    };
  }, [q]);
  return (
    <AskCtx.Provider value={fns}>
      {children}
      {q && (
        <Sheet className="sheet ask" role="alertdialog" label={q.ok} describedBy="ask-text" onClose={() => close(false)}>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              close(true);
            }}
          >
            <p id="ask-text">{q.text}</p>
            {q.input && (
              <label className="field ask-field">
                <span>{q.input.label}</span>
                <input className="search" autoFocus placeholder={q.input.placeholder} maxLength={q.input.max ?? 64} value={typed} onChange={(e) => setTyped(e.target.value)} />
              </label>
            )}
            <div className="row ask-buttons">
              <button type="button" className="btn" autoFocus={q.danger && !q.input} onClick={() => close(false)}>Cancel</button>
              <button type="submit" className={`btn ${q.danger ? "danger strong" : "primary"}`} autoFocus={!q.danger && !q.input}>{q.ok}</button>
            </div>
          </form>
        </Sheet>
      )}
    </AskCtx.Provider>
  );
}

// ---- toasts --------------------------------------------------------------
type ToastAction = { label: string; run: () => void };
type Toast = { id: number; text: string; error?: boolean; action?: ToastAction };
type Push = (text: string, error?: boolean, action?: ToastAction) => void;
const ToastCtx = createContext<Push>(() => {});
/** `toast("Saved")`, `toast("Couldn't save", true)`, or with a button:
 *  `toast("Couldn't load the maps.", true, { label: "Try again", run: reload })`. */
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);
  const push = useCallback<Push>(
    (text, error, action) => {
      // nothing to say (an empty reply from the game): no empty toast
      if (!String(text ?? "").trim()) return;
      const id = Date.now() + Math.random();
      // the same message twice in a row shows once
      setToasts((t) => [...t.filter((x) => x.text !== text), { id, text, error, action }]);
      // errors stay long enough to read (and until dismissed when there's something to do)
      if (!action) setTimeout(() => dismiss(id), error ? 9000 : 2600);
    },
    [dismiss],
  );
  // errors from actions (useActions) that nothing else reported
  useEffect(() => {
    const on = (e: Event) => push(String((e as CustomEvent).detail), true);
    window.addEventListener("ls-error", on);
    return () => window.removeEventListener("ls-error", on);
  }, [push]);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      {createPortal(
        <div className="toasts">
          {toasts.map((t) => (
            <div key={t.id} className={`toast${t.error ? " err" : ""}`} role={t.error ? "alert" : "status"}>
              <span className="toast-text">{t.text}</span>
              {t.action && (
                <button className="btn small" onClick={() => { dismiss(t.id); t.action!.run(); }}>
                  {t.action.label}
                </button>
              )}
              {(t.error || t.action) && (
                <button className="toast-x" aria-label="Dismiss" onClick={() => dismiss(t.id)}>
                  ×
                </button>
              )}
            </div>
          ))}
        </div>,
        document.body,
      )}
    </ToastCtx.Provider>
  );
}

/** Runs a game command and shows its reply (or error) as a toast. */
export function useCommand(after?: () => void) {
  const toast = useToast();
  return useCallback(
    async (command: string, args: string[] = [], quiet = false) => {
      try {
        const msg = (await cmd(command, args))?.trim();
        if (!quiet && msg) toast(msg);
        setTimeout(() => after?.(), 250);
        return true;
      } catch (e) {
        toast((e as Error).message, true);
        return false;
      }
    },
    [toast, after],
  );
}

// ---- controls ------------------------------------------------------------
export function Segmented({
  values,
  labels,
  current,
  onPick,
  label,
  disabled,
}: {
  values: string;
  labels: string;
  current: string;
  onPick: (v: string) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <div className="seg" role="group" aria-label={label} aria-busy={disabled || undefined}>
      {options(values, labels).map(([v, l]) => (
        <button key={v} type="button" aria-pressed={sameValue(current, v)} disabled={disabled} onClick={() => onPick(v)}>
          {l}
        </button>
      ))}
    </div>
  );
}

export function Switch({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return <button type="button" role="switch" aria-checked={on} aria-label={label} className="switch" disabled={disabled} aria-busy={disabled || undefined} onClick={() => onChange(!on)} />;
}

/** A small "?" that explains a field: shown on hover, or on click/tap (phones). Safe
 *  inside a <label>: clicking it doesn't open the field. */
export function Info({ text }: { text: ReactNode }) {
  // shown while hovered, or pinned open by a click/tap
  const [hover, setHover] = useState(false);
  const [pinned, setOpen] = useState(false);
  const open = hover || pinned;
  const box = useRef<HTMLSpanElement>(null);
  const pop = useRef<HTMLSpanElement>(null);
  // placed against the window, so it's never cut off and never makes the page scroll sideways
  useLayoutEffect(() => {
    const r = box.current?.querySelector("button")?.getBoundingClientRect();
    const p = pop.current;
    if (!open || !r || !p) return;
    // measured on screen, placed in the page's own (text size) units
    const z = zoom();
    const b = { left: r.left / z, top: r.top / z, width: r.width / z, bottom: r.bottom / z };
    const w = p.offsetWidth, h = p.offsetHeight, m = 8;
    const left = Math.min(Math.max(b.left + b.width / 2 - w / 2, m), window.innerWidth / z - w - m);
    const above = b.top - h - m;
    p.style.left = `${left}px`;
    p.style.top = `${above >= m ? above : b.bottom + m}px`;
    p.style.visibility = "visible";
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const away = (e: Event) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    // it's placed against the window: scrolling would leave it behind
    const scrolled = () => {
      setOpen(false);
      setHover(false);
    };
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    document.addEventListener("scroll", scrolled, true);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
      document.removeEventListener("scroll", scrolled, true);
    };
  }, [open]);
  // hover is for a mouse: a tap fires enter (and no leave) on touch screens
  const mouse = (e: ReactPointerEvent) => e.pointerType === "mouse";
  return (
    <span className="info" ref={box} onPointerEnter={(e) => mouse(e) && setHover(true)} onPointerLeave={(e) => mouse(e) && setHover(false)}>
      <button
        type="button"
        className="info-btn"
        aria-label="What's this?"
        aria-expanded={open}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setOpen((o) => !o);
          setHover(false);
        }}
      >
        ?
      </button>
      {open && <span className="info-pop" role="tooltip" ref={pop}>{text}</span>}
    </span>
  );
}

/** A button that shows a spinner and another label while its action runs. */
export function BusyButton({ busy, label, busyLabel, className = "btn", disabled, onClick, type = "button" }: { busy: boolean; label: ReactNode; busyLabel: string; className?: string; disabled?: boolean; onClick?: () => void; type?: "button" | "submit" }) {
  return (
    <button type={type} className={className} disabled={busy || disabled} aria-busy={busy} onClick={onClick}>
      {busy ? (
        <>
          <span className="spinner" aria-hidden="true" />
          {busyLabel}
        </>
      ) : (
        label
      )}
    </button>
  );
}

export function SectionHead({ title, note, children }: { title: string; note?: ReactNode; children?: ReactNode }) {
  return (
    <div className="section-head">
      <div className="section-title">
        <h2>{title}</h2>
        {children && <div className="section-actions">{children}</div>}
      </div>
      {note && <p>{note}</p>}
    </div>
  );
}

export function Banner({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="banner">
      <div>{children}</div>
      {action}
    </div>
  );
}

/** The classic Steam loader: a sunken bar that fills up with blocks. */
export function SteamProgress({ value, title, text, error, foot = "This may take a few minutes." }: { value: number; title: string; text?: string; error?: boolean; foot?: string }) {
  const bar = useRef<HTMLDivElement>(null);
  const [slots, setSlots] = useState(40);
  useEffect(() => {
    const el = bar.current;
    if (!el) return;
    const fit = () => setSlots(Math.max(8, Math.floor((el.clientWidth - 6 + 3) / 10)));
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const filled = Math.round(Math.min(1, Math.max(0, value)) * slots);
  return (
    <div className={`steam-box${error ? " error" : ""}`} role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(value * 100)} aria-label={title}>
      <div className="steam-box-title">{title}</div>
      <p className="steam-box-text">{text}</p>
      <div className="steam-bar" ref={bar}>
        {Array.from({ length: filled }, (_, i) => <i key={i} />)}
      </div>
      <div className="steam-box-foot">
        <span>{foot}</span>
        <span>{Math.round(value * 100)}%</span>
      </div>
    </div>
  );
}

/** Grey placeholder bars while something loads. */
export function Skeleton({ rows = 3 }: { rows?: number }) {
  return (
    <div className="skeleton" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => <div key={i} className="skeleton-row" style={{ width: `${92 - ((i * 23) % 40)}%` }} />)}
    </div>
  );
}

// ---- text size (per device: a phone and the computer can differ) --------------------
export const SCALES: [string, number][] = [["Small", 0.9], ["Normal", 1], ["Large", 1.15], ["Larger", 1.3]];
export function savedScale() {
  try {
    return Number(localStorage.getItem("ls.scale")) || 1;
  } catch {
    return 1;
  }
}
/** The text size zoom on the page (1 = normal): sizes measured with getBoundingClientRect
 *  are zoomed, while positions set in CSS get zoomed again, so divide by this first. */
export const zoom = () => Number(getComputedStyle(document.documentElement).getPropertyValue("--zoom")) || 1;
export function applyScale(v: number) {
  (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = String(v);
  document.documentElement.style.setProperty("--zoom", String(v));
  try {
    localStorage.setItem("ls.scale", String(v));
  } catch {
    /* private window */
  }
}
