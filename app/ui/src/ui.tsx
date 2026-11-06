import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
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
      <h2>{title}</h2>
      {note && <p>{note}</p>}
      {children && <div className="section-actions">{children}</div>}
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
