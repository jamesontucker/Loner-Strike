import { useMemo, useState } from "react";
import { postJson } from "../api";
import type { MapInfo, Online } from "../types";
import { SectionHead, useCommand, useToast } from "../ui";

const CATS: [string, string][] = [
  ["all", "All"], ["fav", "Favourites"], ["recent", "Recently played"], ["defuse", "Bomb defusal"],
  ["hostage", "Hostage rescue"], ["objective", "Objectives"], ["surf", "Surf"], ["arena", "Arena & fun"],
  ["other", "Other"], ["nonav", "Needs bot nav"],
  ["stock", "Came with the game"], ["download", "Downloaded"], ["added", "Added by hand"], ["hidden", "Hidden"],
];
const SOURCE = { stock: "Came with the game", download: "Downloaded from GameBanana", added: "Added by hand" };

export function MapCard({ map, onClick, current }: { map: MapInfo; onClick: () => void; current?: boolean }) {
  const [broken, setBroken] = useState(false);
  return (
    <button className="map-card" onClick={onClick} aria-current={current ? "true" : undefined} title={map.title || map.name}>
      <div className={`map-pic${map.thumb === "radar" ? " radar" : ""}`}>
        {map.thumb && !broken && <img src={`/thumb/${encodeURIComponent(map.name)}.png`} alt="" loading="lazy" onError={() => setBroken(true)} />}
        {map.fav && <span className="fav-star" aria-label="Favourite">★</span>}
      </div>
      <span className="map-label">{map.name}</span>
    </button>
  );
}

export function Maps({ data, maps, reload, refresh }: { data: Online | null; maps: MapInfo[]; reload: () => void; refresh: () => void }) {
  const run = useCommand(refresh);
  const toast = useToast();
  const [cat, setCat] = useState("all");
  const [q, setQ] = useState("");
  const [picked, setPicked] = useState<MapInfo | null>(null);
  const current = data?.state.map.toLowerCase();

  const shown = useMemo(() => {
    const query = q.trim().toLowerCase();
    let list = maps.filter((m) => (cat === "hidden" ? m.hidden : !m.hidden));
    if (cat === "fav") list = list.filter((m) => m.fav);
    else if (cat === "recent") list = list.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent);
    else if (cat === "nonav") list = list.filter((m) => !m.nav);
    else if (cat === "stock" || cat === "download" || cat === "added") list = list.filter((m) => m.source === cat);
    else if (cat !== "all" && cat !== "hidden") list = list.filter((m) => m.cat === cat);
    if (query) list = list.filter((m) => `${m.name} ${m.title ?? ""} ${m.by}`.toLowerCase().includes(query));
    return list;
  }, [maps, cat, q]);

  const remove = async (map: MapInfo) => {
    if (!map.mod) return;
    const others = (map.pack ?? []).filter((n) => n.toLowerCase() !== map.name.toLowerCase());
    const msg = others.length
      ? `${map.name} came in "${map.modName ?? "a pack"}" with ${others.length} other map${others.length === 1 ? "" : "s"} (${others.slice(0, 6).join(", ")}${others.length > 6 ? "…" : ""}). Removing it removes the whole download. Continue?`
      : `Remove ${map.name}? Its files are deleted.`;
    if (!confirm(msg)) return;
    const r = await postJson<{ ok?: boolean; error?: string; maps?: string[] }>("/api/gb/uninstall", { mod: map.mod, section: "maps" }).catch((): { ok?: boolean; error?: string; maps?: string[] } => ({ error: "Couldn't remove it." }));
    if (r.error) return toast(r.error, true);
    toast(`Removed ${(r.maps ?? [map.name]).join(", ")}`);
    setPicked(null);
    reload();
  };

  const flag = async (map: MapInfo, f: "fav" | "hidden", on: boolean) => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/mapflag", { flag: f, map: map.name, on }).catch(() => ({ error: "Couldn't save that." }));
    if (r.error) return toast(r.error, true);
    if (f === "hidden") toast(on ? `${map.name} is hidden. Find it under Hidden.` : `${map.name} is back in the list.`);
    setPicked(null);
    reload();
  };

  return (
    <div className="stack">
      <div>
        <SectionHead title="Maps" note={data ? "Pick a map to play it in the current mode." : "Start the game to change maps."}>
          <button className="btn" onClick={() => {
            const pool = shown.filter((m) => m.name.toLowerCase() !== current);
            if (!pool.length || !data) return;
            run("gm_map", [pool[Math.floor(Math.random() * pool.length)].name]);
          }}>Random map</button>
        </SectionHead>
        <div className="toolbar">
          <input className="search" type="search" placeholder="Search maps, authors or names like “cobble”" value={q} onChange={(e) => setQ(e.target.value)} />
          <div className="chips">
            {CATS.map(([id, label]) => (
              <button key={id} className="chip" aria-pressed={cat === id} onClick={() => setCat(id)}>{label}</button>
            ))}
          </div>
        </div>
      </div>
      <div className="maps grid">
        {shown.map((m) => (
          <MapCard key={m.name} map={m} current={m.name.toLowerCase() === current} onClick={() => setPicked(m)} />
        ))}
        {!shown.length && <p className="muted">{maps.length ? "No maps match." : "Loading maps…"}</p>}
      </div>

      {picked && (
        <div className="sheet-backdrop" onClick={() => setPicked(null)}>
          <div className="sheet" role="dialog" aria-label={picked.name} onClick={(e) => e.stopPropagation()}>
            <div className={`sheet-pic${picked.thumb === "radar" ? " radar" : ""}`}>
              {picked.thumb && <img src={`/thumb/${encodeURIComponent(picked.name)}.png`} alt="" />}
            </div>
            <div className="sheet-body">
              <h2 className="map-title">{picked.name}</h2>
              <p className="muted">
                {picked.title && picked.title !== picked.name ? `${picked.title} · ` : ""}
                {picked.by !== "Other" ? `by ${picked.by}` : ""}
                {!picked.nav && " · Bots will learn this map first (a few minutes)"}
              </p>
              <p className="muted">
                {SOURCE[picked.source]}
                {picked.source === "download" && picked.modName ? `: ${picked.modName}` : ""}
                {picked.pack && picked.pack.length > 1 ? ` (a pack of ${picked.pack.length} maps)` : ""}
              </p>
              <div className="row">
                {data && (
                  <button className="btn primary" onClick={() => { setPicked(null); run("gm_map", [picked.name]); }}>
                    Play {picked.name}
                  </button>
                )}
                <button className="btn" onClick={() => flag(picked, "fav", !picked.fav)}>{picked.fav ? "Unfavourite" : "Favourite"}</button>
                <button className="btn" onClick={() => flag(picked, "hidden", !picked.hidden)}>{picked.hidden ? "Show again" : "Hide"}</button>
                {picked.source === "download" && picked.mod && (
                  <button className="btn danger" onClick={() => remove(picked)}>Remove</button>
                )}
                <button className="btn" onClick={() => setPicked(null)}>Close</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
