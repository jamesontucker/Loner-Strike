import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { cmd, options, sameValue } from "./api";

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
