// Empty and loading states for the Tools pages, in the app's plain style. Errors use the
// app's own LoadError (ui.tsx).

import type { ReactNode } from "react";
import "./common.css";

/** Nothing to show yet: what's missing, how to get some, and maybe a button for it.
 *  `plain` drops the panel (inside a panel already, or under a search). */
export function Empty({ title, text, children, plain }: { title?: string; text?: ReactNode; children?: ReactNode; plain?: boolean }) {
  return (
    <div className={`tl-empty${plain ? " plain" : ""}`}>
      {title && <h3>{title}</h3>}
      {text && <p>{text}</p>}
      {children && <div className="row">{children}</div>}
    </div>
  );
}

/** A spinner and what's happening ("Reading the map…"). */
export function Loading({ text, className = "" }: { text: string; className?: string }) {
  return (
    <p className={`tl-loading ${className}`} role="status">
      <span className="spinner" aria-hidden="true" />
      {text}
    </p>
  );
}
