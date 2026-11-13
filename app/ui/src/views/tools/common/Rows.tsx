// Setting rows for the Tools pages, like the app's Settings page: the name (and a line of
// help, or a "?" for more) on the left, the control on the right at its natural width. When
// there isn't room for both, the control drops onto its own line instead of squeezing.

import type { ReactNode } from "react";
import { Info } from "../../../ui";
import "./common.css";

/** A list of rows. `bare`: no line above the first or below the last (inside a small panel). */
export function Rows({ children, bare, className = "" }: { children: ReactNode; bare?: boolean; className?: string }) {
  return <div className={`tl-rows${bare ? " bare" : ""} ${className}`}>{children}</div>;
}

/** One setting. `wide`: the control takes the room it's given (a search and a list).
 *  `stacked`: the control always goes under the name (a set of checkboxes). */
export function Row({ name, note, info, children, wide, stacked, className = "" }: {
  name: ReactNode;
  note?: ReactNode;
  info?: ReactNode;
  children: ReactNode;
  wide?: boolean;
  stacked?: boolean;
  className?: string;
}) {
  return (
    <div className={`tl-row${stacked ? " stacked" : ""} ${className}`}>
      <div className="tl-row-label">
        <span className="tl-row-name">
          {name}
          {info && <Info text={info} />}
        </span>
        {note && <span className="tl-row-note">{note}</span>}
      </div>
      <div className={`tl-row-control${wide ? " wide" : ""}`}>{children}</div>
    </div>
  );
}

/** A checkbox and its label on one line. */
export function Check({ checked, onChange, label, disabled, title }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode; disabled?: boolean; title?: string }) {
  return (
    <label className="tl-check" title={title}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}
