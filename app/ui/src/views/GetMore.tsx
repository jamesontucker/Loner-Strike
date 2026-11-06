import { useCallback, useEffect, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import type { GbDetails, GbItem, GbJob, GbPage, GbSection } from "../types";
import { SectionHead, useToast } from "../ui";

const SORTS: [string, string][] = [["popular", "Most downloaded"], ["new", "Newest"], ["liked", "Most liked"]];
// Shown until the server sends its own list
const FALLBACK_SECTIONS: GbSection[] = [["maps", "Maps", [[5474, "All maps"]]]];
const NOTES: Record<string, string> = {
  maps: "Only map files are copied in, and nothing you have is replaced.",
  default: "These replace the game's own files. Everything they replace is backed up, and removing one puts the originals back.",
};
const size = (n: number) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

/** GameBanana's Counter-Strike 1.6 section: maps, skins, models, sounds and more. */
/** only: show just one section (a server's "Get maps"). */
export function GetMore({ onInstalled, only }: { onInstalled: () => void; only?: string }) {
  const toast = useToast();
  const [section, setSection] = useState("maps");
  const [sections, setSections] = useState<GbSection[]>(FALLBACK_SECTIONS);
  const [cat, setCat] = useState<number | null>(null);
  const [sort, setSort] = useState("popular");
  const [q, setQ] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<GbItem[]>([]);
  const [page, setPage] = useState(0);
  const [done, setDone] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<GbDetails | null>(null);
  const [jobs, setJobs] = useState<GbJob[]>([]);
  const seenDone = useRef(new Set<string>());
  const cats = sections.find((s) => s[0] === section)?.[2] ?? [];

  const load = useCallback(
    async (reset: boolean) => {
      setLoading(true);
      setError("");
      const next = reset ? 1 : page + 1;
      try {
        const r = await getJson<GbPage & { error?: string }>(
          `/api/gb/browse?section=${section}${cat ? `&cat=${cat}` : ""}&sort=${sort}&q=${encodeURIComponent(query)}&page=${next}`,
        );
        if (r.error) throw new Error(r.error);
        setItems((old) => (reset ? r.items : [...old, ...r.items]));
        if (r.sections?.length) setSections(r.sections);
        setPage(next);
        setDone(r.done || !r.items.length);
      } catch (e) {
        setError((e as Error).message.includes("Start") || (e as Error).message.includes("game") ? (e as Error).message : "Couldn't reach GameBanana. Check your internet connection.");
      } finally {
        setLoading(false);
      }
    },
    [section, cat, sort, query, page],
  );

  useEffect(() => {
    load(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section, cat, sort, query]);

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
          if (j.state === "done") {
            onInstalled();
            setItems((old) => old.map((m) => (m.id === j.mod ? { ...m, installed: true } : m)));
          }
        }
      }
    };
    tick();
    const t = setInterval(tick, 1500);
    return () => clearInterval(t);
  }, [toast, onInstalled]);

  const details = async (id: number) => {
    const d = await getJson<GbDetails>(`/api/gb/mod/${id}?section=${section}`).catch(() => null);
    if (d && !(d as unknown as { error?: string }).error) setOpen(d);
    else toast("Couldn't load that page.", true);
  };
  const install = async (mod: number, file: number) => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/install", { mod, file, section }).catch(() => ({ error: "Couldn't start the download." }));
    if (r.error) toast(r.error, true);
    else {
      toast("Downloading…");
      setOpen(null);
    }
  };
  const remove = async (mod: number, name: string) => {
    if (!confirm(`Remove ${name}? ${section === "maps" ? "Its maps are deleted." : "The game's original files are put back."}`)) return;
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/uninstall", { mod, section }).catch(() => ({ error: "Couldn't remove it." }));
    if (r.error) return toast(r.error, true);
    toast(`Removed ${name}`);
    setOpen(null);
    setItems((old) => old.map((m) => (m.id === mod ? { ...m, installed: false } : m)));
    onInstalled();
  };

  const active = jobs.filter((j) => !["done", "error"].includes(j.state));
  const label = sections.find((s) => s[0] === section)?.[1] ?? "Maps";

  return (
    <div className="stack">
      <div>
        <SectionHead title={only ? "Get maps" : "Get more"} note={`From GameBanana's Counter-Strike 1.6 section. ${NOTES[section] ?? NOTES.default}`} />
        <div className="chips section-tabs" role="tablist" aria-label="What to get" hidden={!!only}>
          {sections.map(([id, name]) => (
            <button key={id} className="chip" role="tab" aria-pressed={section === id} onClick={() => { setSection(id); setCat(null); setItems([]); }}>
              {name}
            </button>
          ))}
        </div>
        {active.map((j) => (
          <div className="job" key={j.id}>
            <span>{j.message || j.state}</span>
            {j.size ? <progress max={j.size} value={j.got ?? 0} /> : null}
          </div>
        ))}
        <div className="toolbar">
          <input className="search" type="search" placeholder={`Search ${label.toLowerCase()}`} value={q} onChange={(e) => setQ(e.target.value)} />
          {cats.length > 1 && (
            <select value={cat ?? cats[0][0]} onChange={(e) => setCat(+e.target.value)} aria-label="Type">
              {cats.map(([id, name]) => <option key={id} value={id}>{name}</option>)}
            </select>
          )}
          <div className="seg" role="group" aria-label="Sort">
            {SORTS.map(([id, l]) => (
              <button key={id} aria-pressed={sort === id} onClick={() => setSort(id)}>{l}</button>
            ))}
          </div>
        </div>
      </div>
      {error && <p className="muted">{error}</p>}
      <div className="maps grid">
        {items.map((m) => (
          <button key={m.id} className="map-card" onClick={() => details(m.id)} title={m.name}>
            <div className="map-pic">
              {m.image && <img src={m.image} alt="" loading="lazy" />}
              {m.installed && <span className="badge">Installed</span>}
            </div>
            <span className="map-label">{m.name}</span>
            <span className="map-sub">{m.author} · ♥ {m.likes}</span>
          </button>
        ))}
        {!items.length && !loading && !error && <p className="muted">Nothing here.</p>}
      </div>
      {!done && !error && items.length > 0 && (
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
              {open.audio && <audio controls src={open.audio} className="preview" />}
              <p className="gb-text">{open.text}</p>
              {open.installed ? (
                <div className="banner">
                  <div>Installed.</div>
                  <button className="btn danger" onClick={() => remove(open.id, open.name)}>Remove</button>
                </div>
              ) : (
                <ul className="files">
                  {open.files.map((f) => (
                    <li key={f.id}>
                      <div>
                        <span>{f.file} <small>{size(f.size)}</small></span>
                        {f.replaces && f.replaces.length > 0 && (
                          <p className="muted small-note">Replaces {f.replaces.length === 1 ? f.replaces[0] : `${f.replaces.length} game files (${f.replaces.slice(0, 3).join(", ")}${f.replaces.length > 3 ? "…" : ""})`}. Backed up first.</p>
                        )}
                        {f.conflicts && f.conflicts.length > 0 && (
                          <p className="err-text small-note">Also changes files from {[...new Set(f.conflicts.map((c) => c.name))].join(", ")}. Removing either later puts things back in order.</p>
                        )}
                        {f.listed === false && <p className="muted small-note">Its contents are checked after downloading.</p>}
                      </div>
                      {f.virus ? <span className="muted">Flagged by GameBanana's virus scan</span> : f.supported ? (
                        <button className="btn primary" onClick={() => install(open.id, f.id)}>Install</button>
                      ) : <span className="muted">Can't open this kind of file</span>}
                    </li>
                  ))}
                </ul>
              )}
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
