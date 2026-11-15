// One dialog for the whole map editor: a header with the title and a close button, a body
// that scrolls on its own, and a footer that always stays in view with the main action on the
// right. Enter does the main action (not in a text area), Esc closes, focus starts on the
// first field and comes back where it was (the app's Sheet). Sizes fit a 1024 × 700 window.
// Fields: <Field> gives every control a label, an optional hint and an inline error.

import { useId, type FormEvent, type ReactNode } from "react";
import { BusyButton, Info, Sheet } from "../../../ui";
import "./dialog.css";

export type DialogAction = { label: string; onClick: () => void; busy?: boolean; busyLabel?: string; disabled?: boolean; danger?: boolean };

export function EditorDialog({
  title,
  subtitle,
  size = "md",
  onClose,
  primary,
  secondary,
  footerNote,
  error,
  children,
  closeLabel = "Cancel",
  bodyClass,
}: {
  title: string;
  subtitle?: ReactNode;
  /** sm 420, md 600, lg 820 wide (never wider than the window) */
  size?: "sm" | "md" | "lg";
  onClose: () => void;
  /** the main action (Enter); without one the footer has only Close */
  primary?: DialogAction;
  /** more actions, on the left of the footer */
  secondary?: ReactNode;
  /** a short line in the footer (what the main action will do) */
  footerNote?: ReactNode;
  /** a problem with the whole form, shown above the footer */
  error?: string | null;
  children: ReactNode;
  closeLabel?: string;
  bodyClass?: string;
}) {
  const titleId = useId();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (primary && !primary.disabled && !primary.busy) primary.onClick();
  };
  return (
    <Sheet label={title} onClose={onClose} className={`sheet ed-dlg ed-dlg-${size}`}>
      <form className="ed-dlg-form" onSubmit={submit} noValidate aria-labelledby={titleId}>
        <header className="ed-dlg-head">
          <div className="ed-dlg-titles">
            <h2 id={titleId}>{title}</h2>
            {subtitle && <p className="ed-dlg-sub">{subtitle}</p>}
          </div>
          <button type="button" className="btn small icon ed-dlg-x" aria-label="Close" title="Close (Esc)" onClick={onClose}>
            ×
          </button>
        </header>
        <div className={`ed-dlg-body${bodyClass ? ` ${bodyClass}` : ""}`}>{children}</div>
        {error && (
          <p className="ed-dlg-error" role="alert">
            {error}
          </p>
        )}
        <footer className="ed-dlg-foot">
          <div className="ed-dlg-foot-left">
            {secondary}
            {footerNote && <span className="ed-dlg-note">{footerNote}</span>}
          </div>
          <div className="ed-dlg-foot-right">
            <button type="button" className="btn" onClick={onClose}>
              {primary ? closeLabel : "Close"}
            </button>
            {primary && (
              <BusyButton
                type="submit"
                className={`btn ${primary.danger ? "danger" : "primary"}`}
                busy={!!primary.busy}
                label={primary.label}
                busyLabel={primary.busyLabel ?? "Working…"}
                disabled={primary.disabled}
              />
            )}
          </div>
        </footer>
      </form>
    </Sheet>
  );
}

/** A labelled control with an optional hint (?) and an inline error under it. */
export function Field({ label, hint, error, children, wide, note }: { label: ReactNode; hint?: string; error?: string | null; children: (id: string) => ReactNode; wide?: boolean; note?: ReactNode }) {
  const id = useId();
  return (
    <div className={`ed-field${wide ? " wide" : ""}${error ? " bad" : ""}`}>
      <label htmlFor={id} className="ed-field-label">
        <span>{label}</span>
        {hint && <Info text={hint} />}
      </label>
      {children(id)}
      {note && !error && <span className="ed-field-note">{note}</span>}
      {error && (
        <span className="ed-field-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}

/** A titled group of fields inside a dialog (two columns when there's room). */
export function DialogSection({ title, children, cols = 1, aside }: { title?: ReactNode; children: ReactNode; cols?: 1 | 2; aside?: ReactNode }) {
  return (
    <section className={`ed-dlg-section${cols === 2 ? " two" : ""}`}>
      {(title || aside) && (
        <div className="ed-dlg-section-head">
          {title && <h3>{title}</h3>}
          {aside}
        </div>
      )}
      <div className="ed-dlg-grid">{children}</div>
    </section>
  );
}
