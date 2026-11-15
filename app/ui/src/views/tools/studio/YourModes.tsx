// The start page's Your modes: a grid of cards like the example modes, each with its name, what it
// is, when it was last edited and whether it's installed. Open is the card itself; Remove from
// game and Delete are in its small menu (Delete asks first, in StudioPage's removeGraph). With
// more than a handful, a search box and a sort (last edited, newest first; or name).
import { useEffect, useMemo, useRef, useState } from "react";
import type { GraphInfo } from "./studioApi";

/** Above this many, the search box and the sort show. */
export const FIND_ABOVE = 6;

/** "just now", "5 minutes ago", "3 hours ago", "yesterday", "4 days ago", else the date. */
export function edited(secs: number, now = Date.now()): string {
  if (!secs) return "";
  const s = Math.max(0, Math.round(now / 1000 - secs));
  if (s < 60) return "Edited just now";
  if (s < 3600) return `Edited ${Math.round(s / 60)} minute${Math.round(s / 60) === 1 ? "" : "s"} ago`;
  if (s < 86400) return `Edited ${Math.round(s / 3600)} hour${Math.round(s / 3600) === 1 ? "" : "s"} ago`;
  const d = Math.floor(s / 86400);
  if (d === 1) return "Edited yesterday";
  if (d < 14) return `Edited ${d} days ago`;
  return `Edited ${new Date(secs * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}`;
}

type Sort = "edited" | "name";

/** What the list shows: matching the search (name, description or id), sorted. */
export function shownModes(list: GraphInfo[], find: string, sort: Sort): GraphInfo[] {
  const q = find.trim().toLowerCase();
  const hit = (g: GraphInfo) => !q || [g.name, g.description ?? "", g.id].some((x) => x.toLowerCase().includes(q));
  const by = sort === "name" ? (a: GraphInfo, b: GraphInfo) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }) || b.modified - a.modified : (a: GraphInfo, b: GraphInfo) => b.modified - a.modified || a.name.localeCompare(b.name);
  return list.filter(hit).sort(by);
}

export function YourModes({ list, onOpen, onUninstall, onDelete }: { list: GraphInfo[]; onOpen: (g: GraphInfo) => void; onUninstall: (g: GraphInfo) => void; onDelete: (g: GraphInfo) => void }) {
  const [find, setFind] = useState("");
  const [sort, setSort] = useState<Sort>("edited");
  const [menu, setMenu] = useState("");
  const many = list.length > FIND_ABOVE;
  const shown = useMemo(() => shownModes(list, many ? find : "", sort), [list, find, sort, many]);
  return (
    <div className="st-your-modes" data-testid="studio-your-modes">
      {many && (
        <div className="st-your-tools">
          <input className="search" type="search" placeholder="Find a mode" aria-label="Find a mode" value={find} onChange={(e) => setFind(e.target.value)} />
          <label className="st-your-sort">
            <span className="st-muted">Sort</span>
            <select aria-label="Sort your modes" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
              <option value="edited">Last edited</option>
              <option value="name">Name</option>
            </select>
          </label>
          <span className="st-muted" aria-live="polite">
            {shown.length === list.length ? `${list.length} modes` : `${shown.length} of ${list.length}`}
          </span>
        </div>
      )}
      {shown.length === 0 ? (
        <p className="st-muted">No mode matches “{find.trim()}”.</p>
      ) : (
        <ul className="st-examples st-mode-cards">
          {shown.map((g) => (
            <li key={g.id} className="st-mode-card">
              <button type="button" className="st-graph-open" disabled={g.damaged} onClick={() => onOpen(g)} aria-label={`Open ${g.name}`}>
                <strong>{g.name}</strong>
                <span className="st-muted st-mode-what">{g.damaged ? "This file is damaged." : g.description || `${g.nodes ?? 0} nodes`}</span>
                <span className="st-mode-foot">
                  <span className="st-muted">{edited(g.modified)}</span>
                  {g.installed && <span className="st-tag">Installed</span>}
                </span>
              </button>
              <ModeMenu g={g} open={menu === g.id} setOpen={(on) => setMenu(on ? g.id : "")} onUninstall={onUninstall} onDelete={onDelete} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ModeMenu({ g, open, setOpen, onUninstall, onDelete }: { g: GraphInfo; open: boolean; setOpen: (on: boolean) => void; onUninstall: (g: GraphInfo) => void; onDelete: (g: GraphInfo) => void }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const key = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("pointerdown", away);
      window.removeEventListener("keydown", key);
    };
  }, [open, setOpen]);
  const pick = (f: (g: GraphInfo) => void) => () => {
    setOpen(false);
    f(g);
  };
  return (
    <div className="st-mode-more" ref={box}>
      <button type="button" className="btn small st-mode-more-btn" aria-haspopup="menu" aria-expanded={open} aria-label={`More for ${g.name}`} title="More" onClick={() => setOpen(!open)}>
        ⋯
      </button>
      {open && (
        <div className="st-mode-menu" role="menu" aria-label={`${g.name}`}>
          {g.installed && (
            <button type="button" role="menuitem" onClick={pick(onUninstall)}>
              Remove from game
            </button>
          )}
          <button type="button" role="menuitem" className="danger" onClick={pick(onDelete)}>
            Delete…
          </button>
        </div>
      )}
    </div>
  );
}
