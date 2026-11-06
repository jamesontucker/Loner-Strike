import { useCallback, useEffect, useRef, useState } from "react";
import { addFromFile, getJson, isAbort, ON_THIS_COMPUTER, postJson, targetKey, useActions, useStable } from "../api";
import type { GbDetails, GbItem, GbJob, GbPage, GbSection } from "../types";
import { BusyButton, LoadError, SectionHead, Sheet, Skeleton, useConfirm, useToast } from "../ui";

const SORTS: [string, string][] = [["popular", "Most downloaded"], ["new", "Newest"], ["liked", "Most liked"]];
// Shown until the server sends its own list
const FALLBACK_SECTIONS: GbSection[] = [["maps", "Maps", [[5474, "All maps"]]]];
const NOTES: Record<string, string> = {
  maps: "Only map files are copied in, and nothing you have is replaced.",
  default: "These replace the game's own files. Everything they replace is backed up, and removing one puts the originals back.",
};
/** Shared by the browser and the Installed list, so removing shows on both. */
const removeKey = (section: string, mod: number) => `getmore:${targetKey()}:remove:${section}:${mod}`;
const size = (n: number) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

/** GameBanana's Counter-Strike 1.6 section: maps, skins, models, sounds and more. */
/** only: show just one section (a server's "Get maps"). */
export function GetMore({ onInstalled: onInstalledProp, only }: { onInstalled: () => void; only?: string }) {
  const onInstalled = useStable(onInstalledProp);
  const ask = useConfirm();
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
  const { run, busy } = useActions();
  const key = (what: string) => `getmore:${targetKey()}:${what}`;
  const seenDone = useRef(new Set<string>());
  const cats = sections.find((s) => s[0] === section)?.[2] ?? [];

  // only the newest browse request may fill the grid (searches and tabs change fast)
  const latest = useRef(0);
  // what the last request was (a fresh page or the next one), for Try again
  const lastReset = useRef(true);
  const load = useCallback(
    async (reset: boolean) => {
      const mine = ++latest.current;
      lastReset.current = reset;
      setLoading(true);
      setError("");
      const next = reset ? 1 : page + 1;
      try {
        const r = await getJson<GbPage & { error?: string }>(
          `/api/gb/browse?section=${section}${cat ? `&cat=${cat}` : ""}&sort=${sort}&q=${encodeURIComponent(query)}&page=${next}`,
        );
        if (mine !== latest.current) return;
        if (r.error) throw new Error(r.error);
        setItems((old) => (reset ? r.items : [...old, ...r.items]));
        if (r.sections?.length) setSections(r.sections);
        setPage(next);
        setDone(r.done || !r.items.length);
      } catch (e) {
        if (mine !== latest.current) return;
        setError((e as Error).message.includes("Start") || (e as Error).message.includes("game") ? (e as Error).message : "Couldn't reach GameBanana. Check your internet connection.");
      } finally {
        if (mine === latest.current) setLoading(false);
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
    let first = true;
    const tick = async () => {
      const list = await getJson<GbJob[]>("/api/gb/jobs").catch(() => [] as GbJob[]);
      if (!Array.isArray(list)) return;
      setJobs(list);
      // jobs that had already ended before this page opened were announced then
      if (first) {
        first = false;
        list.filter((j) => j.state === "done" || j.state === "error").forEach((j) => seenDone.current.add(j.id));
        return;
      }
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

  const details = (id: number, name: string) => run(key(`details:${id}`), async () => {
    // (opening another server meanwhile cancels it: nothing to say then)
    const d = await getJson<GbDetails>(`/api/gb/mod/${id}?section=${section}`).catch((e) => (isAbort(e) ? undefined : null));
    if (d === undefined) return;
    if (d && !(d as unknown as { error?: string }).error) setOpen(d);
    else toast("Couldn't load that page.", true);
  }, `Opening ${name}…`);
  const install = (mod: number, file: number, name: string) => run(key(`install:${mod}:${file}`), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/install", { mod, file, section }).catch(() => ({ error: "Couldn't start the download." }));
    if (r.error) toast(r.error, true);
    else {
      toast("Downloading…");
      setOpen(null);
    }
  }, `Installing ${name}…`);
  const remove = async (mod: number, name: string) => {
    if (!(await ask(`Remove ${name}? ${section === "maps" ? "Its maps are deleted." : "The game's original files are put back."}`, { ok: "Remove", danger: true }))) return;
    await run(removeKey(section, mod), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/uninstall", { mod, section }).catch(() => ({ error: "Couldn't remove it." }));
      if (r.error) return toast(r.error, true);
      toast(`Removed ${name}`);
      setOpen(null);
      setItems((old) => old.map((m) => (m.id === mod ? { ...m, installed: false } : m)));
      onInstalled();
    }, `Removing ${name}…`);
  };

  // one sound preview at a time, straight from the grid
  const player = useRef<HTMLAudioElement | null>(null);
  const [playing, setPlaying] = useState<number | null>(null);
  const preview = (id: number, url: string) => {
    player.current?.pause();
    if (playing === id) return setPlaying(null);
    const a = new Audio(url);
    a.onended = () => setPlaying(null);
    // a pause from the next click rejects play() with AbortError: that's not a failure
    a.play().then(() => setPlaying(id)).catch((e: Error) => e.name !== "AbortError" && toast("Couldn't play the preview.", true));
    player.current = a;
  };
  useEffect(() => () => player.current?.pause(), []);
  useEffect(() => {
    player.current?.pause();
    setPlaying(null);
  }, [section]);

  const [showInstalled, setShowInstalled] = useState(false);
  const active = jobs.filter((j) => !["done", "error"].includes(j.state));
  const jobFor = (mod: number) => active.find((j) => j.mod === mod);
  const jobText = (j: GbJob) => (j.size && (j.got ?? 0) < j.size ? `Downloading ${Math.round(((j.got ?? 0) / j.size) * 100)}%` : j.size ? "Installing…" : "Downloading…");
  const label = sections.find((s) => s[0] === section)?.[1] ?? "Maps";

  return (
    <div className="stack">
      <div>
        <SectionHead title={only ? "Get maps" : "Get more"} note={`From GameBanana's Counter-Strike 1.6 section. ${NOTES[section] ?? NOTES.default}`}>
          {ON_THIS_COMPUTER && !showInstalled && (
            <BusyButton busy={busy(key(`add-file:${section}`))} label="Add from a file…" busyLabel="Adding…" onClick={() => run(key(`add-file:${section}`), async () => {
              const r = await addFromFile(section);
              if (!r) return;
              toast(r.message, !r.ok);
              if (r.ok) onInstalled();
            }, `Adding ${label.toLowerCase()} from a file…`)} />
          )}
        </SectionHead>
        <div className="chips section-tabs" role="tablist" aria-label="What to get" hidden={!!only}>
          {sections.map(([id, name]) => (
            <button key={id} className="chip" role="tab" aria-selected={section === id && !showInstalled} onClick={() => {
              setShowInstalled(false);
              // the same tab again (or back from Installed) keeps what's shown and checks for new
              if (id === section) return void load(true);
              setSection(id);
              setCat(null);
              setItems([]);
            }}>
              {name}
            </button>
          ))}
          <button className="chip" role="tab" aria-selected={showInstalled} onClick={() => setShowInstalled(true)}>Installed</button>
        </div>
        {active.map((j) => (
          <div className="job" key={j.id}>
            <span>{j.message || j.state}</span>
            {j.size ? <progress max={j.size} value={j.got ?? 0} /> : null}
          </div>
        ))}
        {!showInstalled && (
          <div className="toolbar">
            <input className="search" type="search" aria-label={`Search ${label.toLowerCase()}`} placeholder={`Search ${label.toLowerCase()}`} value={q} onChange={(e) => setQ(e.target.value)} />
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
        )}
      </div>
      {showInstalled ? (
        <InstalledList onChanged={onInstalled} />
      ) : (
        <>
          {error && !items.length && !loading && <LoadError text={error} onRetry={() => load(lastReset.current)} />}
          <div className="maps grid">
            {items.map((m) => {
              const job = jobFor(m.id);
              return (
                <div key={m.id} className="card-wrap">
                  <button className="map-card" aria-busy={busy(key(`details:${m.id}`))} onClick={() => details(m.id, m.name)} title={m.name}>
                    <div className="map-pic">
                      {m.image ? (
                        <img src={m.image} alt="" loading="lazy" />
                      ) : (
                        <div className="no-pic" aria-hidden="true">
                          <span className="no-pic-kind">{label.replace(/s$/, "")}</span>
                          <span className="no-pic-name small-name">{m.name}</span>
                        </div>
                      )}
                      {m.installed && !job && <span className="badge">Installed</span>}
                      {job && <span className="badge busy" role="status">{jobText(job)}</span>}
                    </div>
                    <span className="map-label">{m.name}</span>
                    <span className="map-sub">{m.author} · ♥ {m.likes}</span>
                  </button>
                  {m.audio && (
                    <div className="card-pic-overlay">
                      <button
                        type="button"
                        className={`play-preview${playing === m.id ? " on" : ""}`}
                        aria-label={playing === m.id ? `Stop ${m.name}` : `Play ${m.name}`}
                        onClick={() => preview(m.id, m.audio!)}
                      >
                        {playing === m.id ? "■" : "▶"}
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
            {!items.length && !loading && !error && <p className="muted">Nothing here.</p>}
            {!items.length && loading && Array.from({ length: 10 }, (_, i) => <div key={i} className="map-card ghost" aria-hidden="true"><div className="map-pic" /><span className="ghost-label" /></div>)}
          </div>
          {!done && items.length > 0 && (
            <div className="row center">
              {error && !loading && <p className="err-text" role="alert">{error}</p>}
              <BusyButton busy={loading} label={error ? "Try again" : "Show more"} busyLabel="Loading…" onClick={() => load(error ? lastReset.current : false)} />
            </div>
          )}
        </>
      )}

      {open && (
        <Sheet className="sheet wide" label={open.name} onClose={() => setOpen(null)}>
          <div className="sheet-pic">{open.images[0] && <img src={open.images[0]} alt="" />}</div>
          <div className="sheet-body">
            <h2>{open.name}</h2>
            <p className="muted">by {open.author}{open.type ? ` · ${open.type}` : ""}</p>
            {open.audio && <audio controls src={open.audio} className="preview" />}
            <p className="gb-text">{open.text}</p>
            {open.installed ? (
              <div className="banner">
                <div>Installed.</div>
                <BusyButton className="btn danger" busy={busy(removeKey(section, open.id))} label="Remove" busyLabel="Removing…" onClick={() => remove(open.id, open.name)} />
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
                      <BusyButton className="btn primary" busy={busy(key(`install:${open.id}:${f.id}`))} disabled={!!jobFor(open.id)} label={jobFor(open.id) ? jobText(jobFor(open.id)!) : "Install"} busyLabel="Installing…" onClick={() => install(open.id, f.id, open.name)} />
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
        </Sheet>
      )}
    </div>
  );
}

type Installed = { mod: number; section: string; name: string; author?: string | null; url?: string | null; date?: number; maps?: string[]; files?: number; local?: boolean };
const SECTION_NAME: Record<string, string> = { maps: "Map", skins: "Weapon skin", players: "Player model", sounds: "Sound", sprays: "Spray", sprites: "Effect", gui: "HUD & menu" };

/** Everything downloaded into this game, in one place, to remove what you don't want. */
function InstalledList({ onChanged }: { onChanged: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [list, setList] = useState<Installed[] | null>(null);
  const [failed, setFailed] = useState(false);
  const load = () => {
    setFailed(false);
    return getJson<Installed[]>("/api/gb/installed")
      .then((l) => setList(Array.isArray(l) ? l : []))
      .catch((e) => !isAbort(e) && setFailed(true));
  };
  useEffect(() => {
    load();
  }, []);
  const remove = async (it: Installed) => {
    const what = it.section === "maps" ? `Its map${(it.maps?.length ?? 0) === 1 ? " is" : "s are"} deleted.` : "The game's original files are put back.";
    if (!(await ask(`Remove ${it.name}? ${what}`, { ok: "Remove", danger: true }))) return;
    await run(removeKey(it.section, it.mod), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/uninstall", { mod: it.mod, section: it.section }).catch(() => ({ error: "Couldn't remove it." }));
      if (r.error) return toast(r.error, true);
      toast(`Removed ${it.name}`);
      load();
      onChanged();
    }, `Removing ${it.name}…`);
  };
  if (failed && !list) return <LoadError text="Couldn't load what's installed." onRetry={load} />;
  if (!list) return <Skeleton />;
  if (!list.length) return <p className="muted">Nothing downloaded yet. Anything you install from here shows up in this list.</p>;
  return (
    <ul className="people installed-list">
      {list.map((it) => (
        <li key={`${it.section}:${it.mod}`}>
          <div className="person">
            <span className="person-name">
              {it.name}
              <i className="role-badge">{SECTION_NAME[it.section] ?? it.section}</i>
            </span>
            <span className="person-sub">
              {it.local ? "Added from a file" : it.author ? `by ${it.author}` : ""}
              {it.maps?.length ? ` · ${it.maps.length > 3 ? `${it.maps.length} maps` : it.maps.join(", ")}` : ""}
              {it.date ? ` · added ${new Date(it.date * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })}` : ""}
            </span>
          </div>
          <div className="row">
            {it.url && <a className="btn" href={it.url} target="_blank" rel="noreferrer">Page</a>}
            <BusyButton className="btn danger" busy={busy(removeKey(it.section, it.mod))} label="Remove" busyLabel="Removing…" onClick={() => remove(it)} />
          </div>
        </li>
      ))}
    </ul>
  );
}
