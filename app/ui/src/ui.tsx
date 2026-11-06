import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cmd, options, sameValue } from "./api";

// ---- asking before something can't be undone --------------------------------------
type Ask = { text: string; ok: string; danger: boolean; done: (yes: boolean) => void };
const AskCtx = createContext<(text: string, opts?: { ok?: string; danger?: boolean }) => Promise<boolean>>(async () => false);
/** In-app replacement for window.confirm: `if (!(await ask("Delete it?", { ok: "Delete", danger: true }))) return;` */
export const useConfirm = () => useContext(AskCtx);

export function ConfirmProvider({ children }: { children: ReactNode }) {
  const [q, setQ] = useState<Ask | null>(null);
  const ask = useCallback(
    (text: string, opts?: { ok?: string; danger?: boolean }) =>
      new Promise<boolean>((resolve) =>
        setQ((old) => {
          old?.done(false);
          return { text, ok: opts?.ok ?? "OK", danger: !!opts?.danger, done: resolve };
        }),
      ),
    [],
  );
  const close = (yes: boolean) => {
    q?.done(yes);
    setQ(null);
  };
  useEffect(() => {
    if (!q) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
    };
    // a question about one page or server never carries over to the next
    const away = () => close(false);
    const back = document.activeElement as HTMLElement | null;
    window.addEventListener("keydown", key);
    window.addEventListener("ls-target", away);
    window.addEventListener("ls-nav", away);
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("ls-target", away);
      window.removeEventListener("ls-nav", away);
      if (back?.isConnected) back.focus();
    };
  }, [q]);
  return (
    <AskCtx.Provider value={ask}>
      {children}
      {q && (
        <div className="sheet-backdrop" onClick={() => close(false)}>
          <div className="sheet ask" role="alertdialog" aria-modal="true" aria-label={q.text} onClick={(e) => e.stopPropagation()}>
            <p>{q.text}</p>
            <div className="row ask-buttons">
              <button className="btn" autoFocus={q.danger} onClick={() => close(false)}>Cancel</button>
              <button className={`btn ${q.danger ? "danger strong" : "primary"}`} autoFocus={!q.danger} onClick={() => close(true)}>{q.ok}</button>
            </div>
          </div>
        </div>
      )}
    </AskCtx.Provider>
  );
}

// ---- toasts --------------------------------------------------------------
type Toast = { id: number; text: string; error?: boolean };
const ToastCtx = createContext<(text: string, error?: boolean) => void>(() => {});
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const push = useCallback((text: string, error?: boolean) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, error }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), error ? 5200 : 2600);
  }, []);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast${t.error ? " err" : ""}`}>
            {t.text}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

/** Runs a game command and shows its reply (or error) as a toast. */
export function useCommand(after?: () => void) {
  const toast = useToast();
  return useCallback(
    async (command: string, args: string[] = [], quiet = false) => {
      try {
        const msg = await cmd(command, args);
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
}: {
  values: string;
  labels: string;
  current: string;
  onPick: (v: string) => void;
  label: string;
}) {
  return (
    <div className="seg" role="group" aria-label={label}>
      {options(values, labels).map(([v, l]) => (
        <button key={v} type="button" aria-pressed={sameValue(current, v)} onClick={() => onPick(v)}>
          {l}
        </button>
      ))}
    </div>
  );
}

export function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return <button type="button" role="switch" aria-checked={on} aria-label={label} className="switch" onClick={() => onChange(!on)} />;
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
    const b = box.current?.querySelector("button")?.getBoundingClientRect();
    const p = pop.current;
    if (!open || !b || !p) return;
    const w = p.offsetWidth, h = p.offsetHeight, m = 8;
    const left = Math.min(Math.max(b.left + b.width / 2 - w / 2, m), window.innerWidth - w - m);
    const above = b.top - h - m;
    p.style.left = `${left}px`;
    p.style.top = `${above >= m ? above : b.bottom + m}px`;
    p.style.visibility = "visible";
  }, [open]);
  useEffect(() => {
    if (!pinned) return;
    const away = (e: Event) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [pinned]);
  return (
    <span className="info" ref={box} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
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
export function BusyButton({ busy, label, busyLabel, className = "btn", disabled, onClick }: { busy: boolean; label: ReactNode; busyLabel: string; className?: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button className={className} disabled={busy || disabled} aria-busy={busy} onClick={onClick}>
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
export function SteamProgress({ value, title, text, error }: { value: number; title: string; text?: string; error?: boolean }) {
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
        <span>This may take a few minutes.</span>
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
export function applyScale(v: number) {
  (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = String(v);
  try {
    localStorage.setItem("ls.scale", String(v));
  } catch {
    /* private window */
  }
}
