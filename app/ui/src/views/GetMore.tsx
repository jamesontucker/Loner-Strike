import { useCallback, useEffect, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import type { GbDetails, GbItem, GbJob, GbPage } from "../types";
import { SectionHead, useToast } from "../ui";

const SORTS: [string, string][] = [["popular", "Most downloaded"], ["new", "Newest"], ["liked", "Most liked"]];
const size = (n: number) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

/** Maps from GameBanana's Counter-Strike 1.6 section, installed through the app's map store. */
export function GetMore({ onInstalled }: { onInstalled: () => void }) {
  const toast = useToast();
  const [cat, setCat] = useState(5474);
  const [sort, setSort] = useState("popular");
  const [q, setQ] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<GbItem[]>([]);
  const [cats, setCats] = useState<[number, string][]>([]);
  const [page, setPage] = useState(0);
  const [done, setDone] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<GbDetails | null>(null);
  const [jobs, setJobs] = useState<GbJob[]>([]);
  const seenDone = useRef(new Set<string>());

  const load = useCallback(
    async (reset: boolean) => {
      setLoading(true);
      setError("");
      const next = reset ? 1 : page + 1;
      try {
        const r = await getJson<GbPage & { error?: string }>(
          `/api/gb/browse?cat=${cat}&sort=${sort}&q=${encodeURIComponent(query)}&page=${next}`,
        );
        if (r.error) throw new Error(r.error);
        setItems((old) => (reset ? r.items : [...old, ...r.items]));
        setCats(r.categories || []);
        setPage(next);
        setDone(r.done || !r.items.length);
      } catch (e) {
        setError((e as Error).message.includes("501") ? "Map downloads aren't in this version of the app yet." : "Couldn't reach GameBanana. Check your internet connection.");
      } finally {
        setLoading(false);
      }
    },
    [cat, sort, query, page],
  );

  useEffect(() => {
    load(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cat, sort, query]);

  useEffect(() => {
    const t = setTimeout(() => setQuery(q.trim()), 450);
    return () => clearTimeout(t);
  }, [q]);

  // install progress
  useEffect(() => {
    const tick = async () => {
      const list = await getJson<GbJob[]>("/api/gb/jobs").catch(() => [] as GbJob[]);
      if (!Array.isArray(list)) return;
      setJobs(list);
      for (const j of list) {
        if ((j.state === "done" || j.state === "error") && !seenDone.current.has(j.id)) {
          seenDone.current.add(j.id);
          toast(j.message || (j.state === "done" ? "Installed" : "Install failed"), j.state === "error");
          if (j.state === "done") onInstalled();
        }
      }
    };
    tick();
    const t = setInterval(tick, 1500);
    return () => clearInterval(t);
  }, [toast, onInstalled]);

  const details = async (id: number) => {
    const d = await getJson<GbDetails>(`/api/gb/mod/${id}`).catch(() => null);
    if (d) setOpen(d);
    else toast("Couldn't load that map's page.", true);
  };
  const install = async (mod: number, file: number) => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/install", { mod, file }).catch(() => ({ error: "Couldn't start the download." }));
    if (r.error) toast(r.error, true);
    else toast("Downloading…");
  };

  const active = jobs.filter((j) => !["done", "error"].includes(j.state));

  return (
    <div className="stack">
      <div>
        <SectionHead title="Get maps" note="From GameBanana's Counter-Strike 1.6 section. Only map files are copied in, and nothing you have is replaced." />
        {active.map((j) => (
          <div className="job" key={j.id}>
            <span>{j.message || j.state}</span>
            {j.size ? <progress max={j.size} value={j.got ?? 0} /> : null}
          </div>
        ))}
        <div className="toolbar">
          <input className="search" type="search" placeholder="Search GameBanana" value={q} onChange={(e) => setQ(e.target.value)} />
          <select value={cat} onChange={(e) => setCat(+e.target.value)} aria-label="Map type">
            {(cats.length ? cats : [[5474, "All maps"] as [number, string]]).map(([id, name]) => (
              <option key={id} value={id}>{name}</option>
            ))}
          </select>
          <div className="seg" role="group" aria-label="Sort">
            {SORTS.map(([id, label]) => (
              <button key={id} aria-pressed={sort === id} onClick={() => setSort(id)}>{label}</button>
            ))}
          </div>
        </div>
      </div>
      {error && <p className="muted">{error}</p>}
      <div className="maps grid">
        {items.map((m) => (
          <button key={m.id} className="map-card" onClick={() => details(m.id)} title={m.name}>
            <div className="map-pic">{m.image && <img src={m.image} alt="" loading="lazy" />}{m.installed && <span className="badge">Installed</span>}</div>
            <span className="map-label">{m.name}</span>
            <span className="map-sub">{m.author} · ♥ {m.likes}</span>
          </button>
        ))}
      </div>
      {!done && !error && (
        <div className="row center">
          <button className="btn" disabled={loading} onClick={() => load(false)}>{loading ? "Loading…" : "Show more"}</button>
        </div>
      )}

      {open && (
        <div className="sheet-backdrop" onClick={() => setOpen(null)}>
          <div className="sheet wide" role="dialog" aria-label={open.name} onClick={(e) => e.stopPropagation()}>
            <div className="sheet-pic">{open.images[0] && <img src={open.images[0]} alt="" />}</div>
            <div className="sheet-body">
              <h2>{open.name}</h2>
              <p className="muted">by {open.author}{open.type ? ` · ${open.type}` : ""}</p>
              <p className="gb-text">{open.text}</p>
              <ul className="files">
                {open.files.map((f) => (
                  <li key={f.id}>
                    <span>{f.file} <small>{size(f.size)}</small></span>
                    {f.virus ? <span className="muted">Flagged by GameBanana's virus scan</span> : f.supported ? (
                      <button className="btn primary" onClick={() => install(open.id, f.id)}>Install</button>
                    ) : <span className="muted">Can't open this kind of file</span>}
                  </li>
                ))}
              </ul>
              <div className="row">
                <a className="btn" href={open.url} target="_blank" rel="noreferrer">Open on GameBanana</a>
                <button className="btn" onClick={() => setOpen(null)}>Close</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
