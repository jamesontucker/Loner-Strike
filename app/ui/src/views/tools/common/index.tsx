// Shared pieces for the Tools pages: setting rows, checkboxes, empty and loading states, the
// map list. Reuse these (and ui.tsx's Segmented, Switch, Info, BusyButton, LoadError) instead
// of a tool's own copy, so every tool looks and behaves the same. Their CSS is common.css,
// loaded with the Tools page. One file on purpose: one shared chunk for every tool.

import { useMemo, useState, type ReactNode } from "react";
import type { MapInfo } from "../../../types";
import { Info } from "../../../ui";
import { COLLATOR } from "../../Maps";

// ---- setting rows, like the app's Settings page: the name (and a line of
// help, or a "?" for more) on the left, the control on the right at its natural width. When
// there isn't room for both, the control drops onto its own line instead of squeezing.


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

// ---- empty and loading states, in the app's plain style. Errors use the
// app's own LoadError (ui.tsx).


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

// ---- a searchable list of the game's maps to pick one from (the map doctor, the nav fixer and
// the callout editor). Rows, not cards: these tools are about files, not pictures.


const PAGE = 150;

export function MapList({ maps, onPick, placeholder, note, filter, none }: {
  maps: MapInfo[];
  onPick: (name: string) => void;
  placeholder: string;
  /** a word or two after a map's name ("no bot nav yet") */
  note?: (m: MapInfo) => string | null;
  filter?: (m: MapInfo) => boolean;
  /** what to say when the filter leaves no maps at all (say, none has a bot nav yet) */
  none?: string;
}) {
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const all = useMemo(() => maps.filter((m) => !m.other && (!filter || filter(m))), [maps, filter]);
  const shown = useMemo(() => {
    const s = q.trim().toLowerCase();
    return all
      .filter((m) => !s || m.name.toLowerCase().includes(s) || (m.title ?? "").toLowerCase().includes(s))
      .sort((a, b) => COLLATOR.compare(a.name, b.name));
  }, [all, q]);
  if (!all.length)
    return <Empty title="No maps here yet" text={maps.length && none ? none : "Loner-Strike can't find any maps in your game yet. Add some from Get more or the Maps page."} />;
  return (
    <div className="stack-tight">
      <div className="toolbar">
        <input className="search" type="search" aria-label="Search maps" placeholder={placeholder} value={q} onChange={(e) => setQ(e.target.value)} />
        <span className="muted small-note tl-count" aria-live="polite">{shown.length} map{shown.length === 1 ? "" : "s"}</span>
      </div>
      {shown.length ? (
        <ul className="tl-maps">
          {shown.slice(0, limit).map((m) => {
            const n = note?.(m);
            return (
              <li key={m.name}>
                <button className="tl-map" onClick={() => onPick(m.name)}>
                  <span className="tl-map-name">{m.name}</span>
                  {m.title && m.title !== m.name && <span className="muted tl-map-title">{m.title}</span>}
                  {m.source === "download" && <span className="tag">Downloaded</span>}
                  {n && <span className="muted small-note push">{n}</span>}
                </button>
              </li>
            );
          })}
        </ul>
      ) : (
        <Empty plain text={`No maps match “${q.trim()}”.`} />
      )}
      {shown.length > limit && (
        <div className="row center">
          <button className="btn" onClick={() => setLimit(limit + PAGE * 2)}>Show more ({shown.length - limit} left)</button>
        </div>
      )}
    </div>
  );
}
