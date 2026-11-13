// A searchable list of the game's maps to pick one from (the map doctor, the nav fixer and
// the callout editor). Rows, not cards: these tools are about files, not pictures.

import { useMemo, useState } from "react";
import type { MapInfo } from "../../../types";
import { COLLATOR } from "../../Maps";
import { Empty } from "./States";

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
