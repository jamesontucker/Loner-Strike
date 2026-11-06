import { Fragment, useMemo, useState } from "react";
import { addFromFile, ON_THIS_COMPUTER, postJson, targetKey, useActions, withTarget } from "../api";
import type { MapInfo, Online } from "../types";
import { BusyButton, Info, SectionHead, useCommand, useConfirm, useToast } from "../ui";

const CATS: [string, string][] = [
  ["all", "All"], ["fav", "Favourites"], ["recent", "Recently played"], ["defuse", "Bomb defusal"],
  ["hostage", "Hostage rescue"], ["objective", "Objectives"], ["surf", "Surf"], ["arena", "Arena & fun"],
  ["other", "Other"], ["nonav", "Needs bot nav"],
  ["stock", "Came with the game"], ["download", "Downloaded"], ["added", "Added by hand"], ["hidden", "Hidden"],
];
/** A "?" after the filters whose names don't say enough. */
const CAT_INFO: Record<string, string> = {
  nonav: "Maps bots haven't learned yet. The first time you play one, the bots spend a few minutes learning it before they can move around.",
  hidden: "Maps you hid. They stay installed but are left out of the other lists, the in-game map menu and map votes.",
};
const SORTS: [string, string][] = [["name", "A to Z"], ["name-desc", "Z to A"], ["played", "Recently played"], ["favs", "Favourites first"], ["type", "By type"], ["source", "Downloaded first"]];
const SOURCE_ORDER = ["download", "added", "stock"];
const SOURCE = { stock: "Came with the game", download: "Downloaded from GameBanana", added: "Added by hand" };

/** What a map without a screenshot shows: its type and name, like a map-loading screen. */
function NoPicture({ name }: { name: string }) {
  const i = name.indexOf("_");
  const kind = i > 0 && i <= 5 ? name.slice(0, i) : "";
  return (
    <div className="no-pic" aria-hidden="true">
      {kind && <span className="no-pic-kind">{kind}</span>}
      <span className="no-pic-name">{kind ? name.slice(i + 1) : name}</span>
    </div>
  );
}

export function MapCard({ map, onClick, current, onFav, selected }: { map: MapInfo; onClick: () => void; current?: boolean; onFav?: () => void; selected?: boolean }) {
  const [broken, setBroken] = useState(false);
  return (
    <button className={`map-card${selected ? " selected" : ""}`} onClick={onClick} aria-current={current ? "true" : undefined} aria-pressed={selected === undefined ? undefined : selected} title={map.title || map.name}>
      <div className={`map-pic${map.thumb === "radar" ? " radar" : ""}`}>
        {(map.thumb !== "radar" || broken) && <NoPicture name={map.name} />}
        {map.thumb && !broken && <img src={withTarget(`/thumb/${encodeURIComponent(map.name)}.png`)} alt="" loading="lazy" onError={() => setBroken(true)} />}
        {onFav ? (
          <span
            role="button"
            tabIndex={0}
            className={`fav-toggle${map.fav ? " on" : ""}`}
            aria-label={map.fav ? `Remove ${map.name} from favourites` : `Add ${map.name} to favourites`}
            title={map.fav ? "Remove from favourites" : "Add to favourites"}
            onClick={(e) => { e.stopPropagation(); onFav(); }}
            onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); onFav(); } }}
          >
            {map.fav ? "★" : "☆"}
          </span>
        ) : (
          map.fav && <span className="fav-star" aria-label="Favourite">★</span>
        )}
      </div>
      <span className="map-label">{map.name}</span>
    </button>
  );
}

export function Maps({ data, maps, loading, reload, refresh }: { data: Online | null; maps: MapInfo[]; loading?: boolean; reload: () => void; refresh: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const toast = useToast();
  const [cat, setCat] = useState("all");
  const [q, setQ] = useState("");
  const [sort, setSortRaw] = useState(() => {
    try {
      return localStorage.getItem("ls.mapSort") || "name";
    } catch {
      return "name";
    }
  });
  const setSort = (v: string) => {
    setSortRaw(v);
    try {
      localStorage.setItem("ls.mapSort", v);
    } catch {
      /* private window */
    }
  };
  const [picked, setPicked] = useState<MapInfo | null>(null);
  const { run: once, busy } = useActions();
  const key = (what: string) => `maps:${targetKey()}:${what}`;
  const changeMap = (name: string) => once(key("change"), () => run("gm_map", [name]), `Changing the map to ${name}…`);
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
    if (cat !== "recent") {
      const byName = (a: MapInfo, b: MapInfo) => a.name.localeCompare(b.name);
      const played = (m: MapInfo) => (m.recent >= 0 ? m.recent : 1e9);
      const order: Record<string, (a: MapInfo, b: MapInfo) => number> = {
        name: byName,
        "name-desc": (a, b) => byName(b, a),
        played: (a, b) => played(a) - played(b) || byName(a, b),
        favs: (a, b) => Number(b.fav) - Number(a.fav) || byName(a, b),
        type: (a, b) => a.cat.localeCompare(b.cat) || byName(a, b),
        source: (a, b) => SOURCE_ORDER.indexOf(a.source) - SOURCE_ORDER.indexOf(b.source) || byName(a, b),
      };
      list = [...list].sort(order[sort] ?? byName);
    }
    return list;
  }, [maps, cat, q, sort]);

  const remove = async (map: MapInfo) => {
    if (!map.mod) return;
    const others = (map.pack ?? []).filter((n) => n.toLowerCase() !== map.name.toLowerCase());
    const msg = others.length
      ? `${map.name} came in "${map.modName ?? "a pack"}" with ${others.length} other map${others.length === 1 ? "" : "s"} (${others.slice(0, 6).join(", ")}${others.length > 6 ? "…" : ""}). Removing it removes the whole download. Continue?`
      : `Remove ${map.name}? Its files are deleted.`;
    if (!(await ask(msg, { ok: "Remove", danger: true }))) return;
    await once(key(`remove:${map.mod}`), async () => {
      const r = await postJson<{ ok?: boolean; error?: string; maps?: string[] }>("/api/gb/uninstall", { mod: map.mod, section: "maps" }).catch((): { ok?: boolean; error?: string; maps?: string[] } => ({ error: "Couldn't remove it." }));
      if (r.error) return toast(r.error, true);
      toast(`Removed ${(r.maps ?? [map.name]).join(", ")}`);
      setPicked(null);
      reload();
    }, `Removing ${map.name}…`);
  };

  const flag = (map: MapInfo, f: "fav" | "hidden", on: boolean) => once(key(`flag:${f}:${map.name}`), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/mapflag", { flag: f, map: map.name, on }).catch(() => ({ error: "Couldn't save that." }));
    if (r.error) return toast(r.error, true);
    if (f === "hidden") toast(on ? `${map.name} is hidden. Find it under Hidden.` : `${map.name} is back in the list.`);
    if (f === "fav") toast(on ? `${map.name} added to favourites` : `${map.name} removed from favourites`);
    setPicked(null);
    reload();
  }, f === "fav" ? (on ? `Adding ${map.name} to favourites…` : `Removing ${map.name} from favourites…`) : on ? `Hiding ${map.name}…` : `Showing ${map.name} again…`);

  return (
    <div className="stack">
      <div>
        <SectionHead title="Maps" note={data ? "Pick a map to play it in the current mode." : "Start the game to change maps."}>
          {ON_THIS_COMPUTER && (
            <BusyButton busy={busy(key("add-file"))} label="Add a map file…" busyLabel="Adding…" onClick={() => once(key("add-file"), async () => {
              const r = await addFromFile("maps");
              if (r) toast(r.message, !r.ok);
              if (r?.ok) reload();
            }, "Adding a map file…")} />
          )}
          <BusyButton busy={busy(key("change"))} label="Random map" busyLabel="Changing…" onClick={() => {
            const pool = shown.filter((m) => m.name.toLowerCase() !== current);
            if (!pool.length || !data) return;
            changeMap(pool[Math.floor(Math.random() * pool.length)].name);
          }} />
        </SectionHead>
        <div className="toolbar">
          <input className="search" type="search" placeholder="Search maps, authors or names like “cobble”" value={q} onChange={(e) => setQ(e.target.value)} />
          <label className="sort">
            <span>Sort</span>
            <select value={sort} onChange={(e) => setSort(e.target.value)} disabled={cat === "recent"}>
              {SORTS.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select>
          </label>
          <div className="chips">
            {CATS.map(([id, label]) => (
              <Fragment key={id}>
                <button className="chip" aria-pressed={cat === id} onClick={() => setCat(id)}>{label}</button>
                {CAT_INFO[id] && <Info text={CAT_INFO[id]} />}
              </Fragment>
            ))}
          </div>
        </div>
      </div>
      <div className="maps grid">
        {shown.map((m) => (
          <MapCard key={m.name} map={m} current={m.name.toLowerCase() === current} onClick={() => setPicked(m)} onFav={() => flag(m, "fav", !m.fav)} />
        ))}
        {!shown.length && (maps.length ? <p className="muted">No maps match.</p> : Array.from({ length: 10 }, (_, i) => <div key={i} className="map-card ghost" aria-hidden="true"><div className="map-pic" /><span className="ghost-label" /></div>))}
      </div>

      {picked && (
        <div className="sheet-backdrop" onClick={() => setPicked(null)}>
          <div className="sheet" role="dialog" aria-label={picked.name} onClick={(e) => e.stopPropagation()}>
            <div className={`sheet-pic${picked.thumb === "radar" ? " radar" : ""}`}>
              {picked.thumb !== "radar" && <NoPicture name={picked.name} />}
              {picked.thumb && <img src={withTarget(`/thumb/${encodeURIComponent(picked.name)}.png`)} alt="" />}
            </div>
            <div className="sheet-body">
              <h2 className="map-title">{picked.name}</h2>
              <p className="muted">
                {picked.title && picked.title !== picked.name ? `${picked.title} · ` : ""}
                {picked.by !== "Other" ? `by ${picked.by}` : ""}
                {!picked.nav && " · Bots will learn this map first (a few minutes)"}
              </p>
              <p className="muted">
                {picked.source === "download" && (picked.mod ?? 0) < 0 ? "Added from a file" : SOURCE[picked.source]}
                {picked.source === "download" && (picked.mod ?? 0) > 0 && picked.modName ? `: ${picked.modName}` : ""}
                {picked.pack && picked.pack.length > 1 ? ` (a pack of ${picked.pack.length} maps)` : ""}
              </p>
              <div className="row">
                {data && (
                  <BusyButton className="btn primary" busy={busy(key("change"))} label={`Play ${picked.name}`} busyLabel="Changing…" onClick={() => { setPicked(null); changeMap(picked.name); }} />
                )}
                <BusyButton busy={busy(key(`flag:fav:${picked.name}`))} label={picked.fav ? "Unfavourite" : "Favourite"} busyLabel="Saving…" onClick={() => flag(picked, "fav", !picked.fav)} />
                <BusyButton busy={busy(key(`flag:hidden:${picked.name}`))} label={picked.hidden ? "Show again" : "Hide"} busyLabel={picked.hidden ? "Showing…" : "Hiding…"} onClick={() => flag(picked, "hidden", !picked.hidden)} />
                {picked.source === "download" && picked.mod && (
                  <BusyButton className="btn danger" busy={busy(key(`remove:${picked.mod}`))} label="Remove" busyLabel="Removing…" onClick={() => remove(picked)} />
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
