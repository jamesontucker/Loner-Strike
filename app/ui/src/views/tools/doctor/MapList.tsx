// A searchable list of the game's maps to pick one from (the map doctor and the callout
// editor). Rows, not cards: these tools are about files, not pictures.

import { useMemo, useState } from "react";
import type { MapInfo } from "../../../types";
import { COLLATOR } from "../../Maps";

const PAGE = 150;

export function MapList({ maps, onPick, placeholder, note, filter }: {
  maps: MapInfo[];
  onPick: (name: string) => void;
  placeholder: string;
  /** a word or two after a map's name ("no bot nav yet") */
  note?: (m: MapInfo) => string | null;
  filter?: (m: MapInfo) => boolean;
}) {
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const shown = useMemo(() => {
    const s = q.trim().toLowerCase();
    return maps
      .filter((m) => !m.other && (!filter || filter(m)) && (!s || m.name.toLowerCase().includes(s) || (m.title ?? "").toLowerCase().includes(s)))
      .sort((a, b) => COLLATOR.compare(a.name, b.name));
  }, [maps, q, filter]);
  return (
    <div className="stack-tight">
      <div className="toolbar">
        <input className="search" type="search" aria-label="Search maps" placeholder={placeholder} value={q} onChange={(e) => setQ(e.target.value)} />
        <span className="muted small-note">{shown.length} map{shown.length === 1 ? "" : "s"}</span>
      </div>
      <ul className="doc-maps">
        {shown.slice(0, limit).map((m) => {
          const n = note?.(m);
          return (
            <li key={m.name}>
              <button className="doc-map" onClick={() => onPick(m.name)}>
                <span className="doc-map-name">{m.name}</span>
                {m.title && m.title !== m.name && <span className="muted doc-map-title">{m.title}</span>}
                {m.source === "download" && <span className="tag">Downloaded</span>}
                {n && <span className="muted small-note push">{n}</span>}
              </button>
            </li>
          );
        })}
      </ul>
      {!shown.length && <p className="muted">{maps.length ? "No maps match." : "Loner-Strike can't find any maps in your game yet."}</p>}
      {shown.length > limit && (
        <div className="row center">
          <button className="btn" onClick={() => setLimit(limit + PAGE * 2)}>Show more ({shown.length - limit} left)</button>
        </div>
      )}
    </div>
  );
}
