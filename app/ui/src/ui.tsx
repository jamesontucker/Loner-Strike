import { Component, createContext, lazy, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ComponentType, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cmd, options, sameValue } from "./api";
import { DocsLink } from "./views/DocsLink";

// ---- dialogs -------------------------------------------------------------------------

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
let openSheets = 0;

// ---- the back button (phones, and the browser) ----------------------------------------
// Each open sheet has its own history entry, so Back closes the top one. App.tsx's
// popstate handler asks closeTopSheet() first, then goes back a page.

type OpenSheet = { id: number; close: () => void };
const sheetStack: OpenSheet[] = [];
let nextSheet = 1;
/** Backs a sheet started itself (taking its entry off) that haven't landed yet: they close
 *  nothing. History moves later than the code runs, so a sheet opened meanwhile (React's
 *  dev double mount, or one sheet closing as another opens) would otherwise close at once. */
let ownBacks = 0;
/** History moved to an entry over sheet `landed` (0: no sheet): every sheet opened after
 *  it closes (true), or there was none (false). Works from the entry alone, so a Back we
 *  caused ourselves (a sheet taking its entry off again) closes nothing extra. */
/** Waits until a dialog that just closed has taken its history entry off (its Back has
 *  landed), or a moment: open a new tab or page after that, or the late Back (WebKit) is
 *  taken for a Back from it to the page under the dialog. */
export function afterSheetBack(): Promise<void> {
  return new Promise((done) => {
    let t = 0;
    const end = () => {
      window.removeEventListener("popstate", end);
      clearTimeout(t);
      setTimeout(done, 0);
    };
    window.addEventListener("popstate", end);
    t = window.setTimeout(end, 300);
  });
}
/** Whether a sheet (dialog) is open: keys meant for the page behind it are left alone (the map
 *  editor's Esc, Delete and the rest; EditorCoverage F-1). */
export function sheetOpen() {
  return sheetStack.length > 0;
}

export function closeSheetsAbove(landed: number) {
  if (ownBacks > 0) {
    ownBacks--;
    return true;
  }
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
    // Esc closes the top sheet wherever focus is: a button that had it can grey out (Apply while it
    // works) or go away (a removed row), and focus falls to the page. The page behind never sees
    // that Esc (EditorCoverage F-1: it dropped the editor's selection and left the sheet open).
    const anywhere = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || sheetStack[sheetStack.length - 1] !== entry) return;
      if (el && e.target instanceof Node && el.contains(e.target)) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      close.current?.();
    };
    window.addEventListener("keydown", anywhere, true);
    return () => {
      window.removeEventListener("keydown", anywhere, true);
      el?.removeEventListener("keydown", key);
      if (!keepApp && --openSheets === 0) root?.removeAttribute("inert");
      if (back?.isConnected) back.focus();
      const at = sheetStack.indexOf(entry);
      if (at >= 0) sheetStack.splice(at, 1);
      // closed by its own button (its entry is still the current one): take the entry off
      // again, so Back afterwards goes where it should
      if ((history.state as { sheet?: number } | null)?.sheet === id) {
        ownBacks++;
        history.back();
      }
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
/** `action`: the way on when trying again can't help (shown first, as the main button). */
export function LoadError({ text, onRetry, title, action }: { text: string; onRetry?: () => void; title?: string; action?: { label: string; run: () => void } }) {
  return (
    <div className="load-error" role="alert">
      {title && <h3>{title}</h3>}
      <p>{text}</p>
      {(action || onRetry) && (
        <div className="row">
          {action && <button className="btn primary" onClick={action.run}>{action.label}</button>}
          {onRetry && <button className="btn" onClick={onRetry}>Try again</button>}
        </div>
      )}
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
/** `check` (with a text box): runs on OK with what was typed; a sentence back keeps the dialog
 *  open with that error under the field and the text kept (`busy` is the button's label meanwhile,
 *  e.g. "Renaming…"). */
type AskOpts = { ok?: string; danger?: boolean; check?: (answer: string) => Promise<string | null | undefined> | string | null | undefined; busy?: string };
/** A text box under the question (optional unless `required`). */
type AskInput = { label: string; placeholder?: string; max?: number; value?: string };
type Ask = { text: string; ok: string; danger: boolean; input?: AskInput; check?: AskOpts["check"]; busy?: string; done: (answer: string | null) => void };
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
  const [askError, setAskError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const open = useCallback(
    (text: string, opts: AskOpts | undefined, input?: AskInput) =>
      new Promise<string | null>((resolve) => {
        setTyped(input?.value ?? "");
        setAskError(null);
        setChecking(false);
        setQ((old) => {
          old?.done(null);
          return { text, ok: opts?.ok ?? "OK", danger: !!opts?.danger, input, check: opts?.check, busy: opts?.busy, done: resolve };
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
            onSubmit={async (e) => {
              e.preventDefault();
              if (checking) return;
              if (q.check && q.input) {
                setChecking(true);
                let why: string | null | undefined;
                try {
                  why = await q.check(typed.trim());
                } catch {
                  why = "Something went wrong. Try again.";
                }
                setChecking(false);
                if (why) return setAskError(why);
              }
              close(true);
            }}
          >
            <p id="ask-text">{q.text}</p>
            {q.input && (
              <label className="field ask-field">
                <span>{q.input.label}</span>
                <input
                  className="search"
                  autoFocus
                  placeholder={q.input.placeholder}
                  maxLength={q.input.max ?? 64}
                  value={typed}
                  aria-invalid={askError ? true : undefined}
                  aria-describedby={askError ? "ask-error" : undefined}
                  onChange={(e) => {
                    setTyped(e.target.value);
                    setAskError(null);
                  }}
                />
                {askError && (
                  <small id="ask-error" className="ask-error" role="alert">
                    {askError}
                  </small>
                )}
              </label>
            )}
            <div className="row ask-buttons">
              <button type="button" className="btn" autoFocus={q.danger && !q.input} onClick={() => close(false)}>Cancel</button>
              <button type="submit" className={`btn ${q.danger ? "danger strong" : "primary"}`} autoFocus={!q.danger && !q.input} disabled={checking} aria-busy={checking || undefined}>
                {checking ? (
                  <>
                    <span className="spinner" aria-hidden="true" /> {q.busy ?? "Working…"}
                  </>
                ) : (
                  q.ok
                )}
              </button>
            </div>
          </form>
        </Sheet>
      )}
    </AskCtx.Provider>
  );
}

// ---- toasts --------------------------------------------------------------
type ToastAction = { label: string; run: () => void };
/** `undoAt`: when an "Undo that save?" toast came (the save's own message joins it) */
type Toast = { id: number; text: string; error?: boolean; action?: ToastAction; undoAt?: number };
/** `ms`: how long a toast with a button stays (without: until dismissed) */
type Push = (text: string, error?: boolean, action?: ToastAction, ms?: number) => void;
const ToastCtx = createContext<Push>(() => {});
const UNDO_SAVE = "Undo that save?";
/** `toast("Saved")`, `toast("Couldn't save", true)`, or with a button:
 *  `toast("Couldn't load the maps.", true, { label: "Try again", run: reload })`. */
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);
  const push = useCallback<Push>(
    (text, error, action, ms) => {
      // nothing to say (an empty reply from the game): no empty toast
      if (!String(text ?? "").trim()) return;
      const id = Date.now() + Math.random();
      setToasts((t) => {
        // a save's message right after its "Undo that save?": one toast with the message and
        // Undo (two stacked ones covered the tool's Save bar)
        const undo = !error && !action ? t.find((x) => x.undoAt && Date.now() - x.undoAt < 3000) : undefined;
        if (undo) return t.map((x) => (x === undo ? { ...x, text, undoAt: undefined } : x));
        // the same message twice in a row shows once
        return [...t.filter((x) => x.text !== text), { id, text, error, action, undoAt: action && text === UNDO_SAVE ? Date.now() : undefined }];
      });
      // errors stay long enough to read (and until dismissed when there's something to do)
      if (!action || ms) setTimeout(() => dismiss(id), ms ?? (error ? 9000 : 2600));
    },
    [dismiss],
  );
  // errors from actions (useActions) that nothing else reported
  useEffect(() => {
    const on = (e: Event) => push(String((e as CustomEvent).detail), true);
    window.addEventListener("ls-error", on);
    return () => window.removeEventListener("ls-error", on);
  }, [push]);
  // a save that kept backups (tools/common/api.ts offerUndo): "Undo" for 8 seconds
  useEffect(() => {
    const on = (e: Event) => {
      const d = (e as CustomEvent<{ text: string; run: () => Promise<{ error?: string; message?: string }> }>).detail;
      push(d.text, false, { label: "Undo", run: () => void d.run().then((r) => push(r.error ?? r.message ?? "Undone.", !!r.error)) }, 8000);
    };
    window.addEventListener("ls-undoable", on);
    return () => window.removeEventListener("ls-undoable", on);
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
/** The choices of a Segmented: "a b c" (labels "A|B|C" or "A B C"), "a,b,c" (labels "A,B,C" or
 *  "A|B|C"; empty values kept: ",map" = All, Maps) or arrays. Never one pill for the whole list. */
function choices(values: string | string[], labels?: string | string[]): [string, string][] {
  const commas = typeof values === "string" && values.includes(",") && !/\s/.test(values);
  if (!Array.isArray(values) && !commas) return options(values, Array.isArray(labels) ? labels.join("|") : (labels ?? values));
  const v = Array.isArray(values) ? values.map(String) : values.split(",");
  const l = Array.isArray(labels) ? labels.map(String) : labels ? (labels.includes("|") ? labels.split("|") : labels.split(",")) : [];
  return v.map((x, i) => [x, l[i]?.trim() || x || "All"]);
}

/** A choice of a few words, one pill each. `values`/`labels` take a string ("a b c" with labels
 *  "A|B|C") or arrays; `options` takes [value, label] pairs. */
export function Segmented({
  values,
  labels,
  options: pairs,
  current,
  onPick,
  label,
  disabled,
}: {
  values?: string | string[];
  labels?: string | string[];
  options?: readonly (readonly [string, ReactNode])[];
  current: string;
  onPick: (v: string) => void;
  label: string;
  disabled?: boolean;
}) {
  const list: (readonly [string, ReactNode])[] = pairs ? [...pairs] : choices(values ?? "", labels);
  return (
    <div className="seg" role="group" aria-label={label} aria-busy={disabled || undefined}>
      {list.map(([v, l]) => (
        <button key={v} type="button" aria-pressed={v === "" ? current === "" : sameValue(current, v)} disabled={disabled} onClick={() => onPick(v)}>
          {l}
        </button>
      ))}
    </div>
  );
}

export function Switch({ on, onChange, label, disabled }: { on: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return <button type="button" role="switch" aria-checked={on} aria-label={label} className="switch" disabled={disabled} aria-busy={disabled || undefined} onClick={() => onChange(!on)} />;
}

/** Places a popup (position: fixed, in document.body) next to `anchor`, on screen: on the side
 *  asked for if it fits, else the other side; shifted along so it never leaves the window.
 *  Works in the page's own units (the text size setting zooms the page). */
export function placePopup(anchor: DOMRect, pop: HTMLElement, side: "top" | "bottom" = "top", align: "center" | "start" | "end" = "center", gap = 8) {
  const z = zoom();
  const r = { left: anchor.left / z, right: anchor.right / z, top: anchor.top / z, bottom: anchor.bottom / z };
  const vw = window.innerWidth / z, vh = window.innerHeight / z, m = 8;
  const w = pop.offsetWidth, h = pop.offsetHeight;
  const want = align === "start" ? r.left : align === "end" ? r.right - w : (r.left + r.right) / 2 - w / 2;
  const left = Math.min(Math.max(want, m), Math.max(m, vw - w - m));
  const above = r.top - h - gap, below = r.bottom + gap;
  const fitsAbove = above >= m, fitsBelow = below + h <= vh - m;
  let top = side === "top" ? (fitsAbove || !fitsBelow ? above : below) : fitsBelow || !fitsAbove ? below : above;
  top = Math.min(Math.max(top, m), Math.max(m, vh - h - m));
  pop.style.left = `${left}px`;
  pop.style.top = `${top}px`;
  pop.dataset.side = top < r.top ? "top" : "bottom";
  pop.style.visibility = "visible";
}

/** A small "?" that explains a field: shown on hover, or on click/tap (phones). Safe
 *  inside a <label>: clicking it doesn't open the field. The tip goes in document.body, above
 *  everything (menu bars, panels, sheets), and flips/shifts to stay on screen. */
export function Info({ text }: { text: ReactNode }) {
  // shown while hovered, or pinned open by a click/tap
  const [hover, setHover] = useState(false);
  const [pinned, setOpen] = useState(false);
  const open = hover || pinned;
  const box = useRef<HTMLSpanElement>(null);
  const pop = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const r = box.current?.querySelector("button")?.getBoundingClientRect();
    if (open && r && pop.current) placePopup(r, pop.current, "top");
  }, [open, text]);
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
    window.addEventListener("resize", scrolled);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
      document.removeEventListener("scroll", scrolled, true);
      window.removeEventListener("resize", scrolled);
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
      {open && createPortal(<span className="info-pop" role="tooltip" ref={pop}>{text}</span>, document.body)}
    </span>
  );
}

/** A button that shows a spinner and another label while its action runs. */
export function BusyButton({ busy, label, busyLabel, className = "btn", disabled, onClick, type = "button", title }: { busy: boolean; label: ReactNode; busyLabel: string; className?: string; disabled?: boolean; onClick?: () => void; type?: "button" | "submit"; title?: string }) {
  return (
    <button type={type} className={className} disabled={busy || disabled} aria-busy={busy} onClick={onClick} title={title}>
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

/** A page or section's title row: its buttons at the right, and `docs` (a docs.html section id)
 *  adds the Docs button there last, like the tools' headers. */
export function SectionHead({ title, note, children, docs }: { title: string; note?: ReactNode; children?: ReactNode; docs?: string }) {
  return (
    <div className="section-head">
      <div className="section-title">
        <h2>{title}</h2>
        {(children || docs) && (
          <div className="section-actions">
            {children}
            {docs && <DocsLink to={docs} what={title} small={false} />}
          </div>
        )}
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
