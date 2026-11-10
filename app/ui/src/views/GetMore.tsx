import { useCallback, useEffect, useRef, useState } from "react";
import { addFromFile, getJson, isAbort, ON_THIS_COMPUTER, isSharedId, postJson, targetKey, useActions, usePoll, useStable } from "../api";
import { useAppInfo, useGameState, useMaps } from "../queries";
import { useNav, type Section } from "../store";
import { usePlayDrafts } from "../stores/playDraft";
import type { GbDetails, GbItem, GbJob, GbPage, GbSection } from "../types";
import { BusyButton, LoadError, SectionHead, Sheet, Skeleton, Switch, useConfirm, useToast } from "../ui";

const SORTS: [string, string][] = [["popular", "Most downloaded"], ["new", "Newest"], ["liked", "Most liked"]];
// Shown until the server sends its own list
const FALLBACK_SECTIONS: GbSection[] = [["maps", "Maps", [[5474, "All maps"]]]];
const NOTES: Record<string, string> = {
  maps: "Only map files are copied in, and nothing you have is replaced.",
  default: "These replace the game's own files. Everything they replace is backed up, and removing one puts the originals back.",
};
/** A dedicated server can only send players what has a name of its own: maps, and weapon
 *  skins and player models, which go in under new names (servercontent.rs). */
const SERVER_SECTIONS = ["maps", "skins", "players"];
const SERVER_NOTE = "Players download what this server uses when they join. Skins and player models go in under their own names, so everyone sees them, not just you.";
type Slots = { map?: string; models?: number; sounds?: number; generic?: number };
type ServerContent = { items: { mod: number; section: string }[]; slots: Slots | null; limit: number; ours: number };
/** Shared by the browser and the lists of what you have, so removing shows on both. */
const removeKey = (section: string, mod: number) => `getmore:${targetKey()}:remove:${section}:${mod}`;
/** Every kind GameBanana's browser has: a page elsewhere opens Get more on one with
 *  go("more", kind). */
const KINDS = ["maps", "skins", "players", "sprays", "sprites", "gui", "sounds"];
/** Where what you have of each kind lives in My library (removing is there too): page, its
 *  focus (Sounds' own tab) and name. */
const LIBRARY: [Section, string | undefined, string][] = [["maps", undefined, "Maps"], ["skins", undefined, "Skins & models"], ["sounds", "yours", "Sounds"]];
const size = (n: number) => (n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

/** GameBanana's Counter-Strike 1.6 section: maps, skins, models, sounds and more.
 *  server: for a dedicated server (maps, skins and player models it sends players).
 *  only: just these sections, opened from the page that manages them (a server's Maps or
 *  Skins & models), with a way back there instead of the links to My library.
 *  Your own game's Get more opens on the kind in the nav focus ("skins"), from a page's Get button. */
export function GetMore({ onInstalled: onInstalledProp, server, only, title = "Get more", back }: { onInstalled: () => void; server?: boolean; only?: string[]; title?: string; back?: { label: string; go: () => void } }) {
  const onInstalled = useStable(onInstalledProp);
  const ask = useConfirm();
  const toast = useToast();
  const [section, setSection] = useState(() => {
    const f = useNav.getState().focus;
    return only?.[0] ?? (!server && f && KINDS.includes(f) ? f : "maps");
  });
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
  // a downloaded map can go straight into Play's Start a game form (your own game, not running)
  const allMaps = useMaps().data;
  const gameId = useAppInfo().data?.game?.id;
  const live = !!useGameState().state?.online;
  const startMap = open && !server && !live && gameId && section === "maps" && open.installed ? allMaps?.find((m) => m.mod === open.id && !m.hidden)?.name : undefined;
  const startOn = (name: string) => {
    if (gameId) usePlayDrafts.getState().set(gameId, { map: name });
    setOpen(null);
    useNav.getState().go("play");
  };
  const cats = sections.find((s) => s[0] === section)?.[2] ?? [];
  // a Get button elsewhere: that kind, also when this page is already open
  const focus = useNav((s) => s.focus);
  useEffect(() => {
    if (!focus || only || server || !KINDS.includes(focus)) return;
    useNav.getState().clearFocus();
    if (focus === section) return;
    setSection(focus);
    setCat(null);
    setItems([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focus]);

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
        const allowed = only ?? (server ? SERVER_SECTIONS : null);
        if (r.sections?.length) setSections(allowed ? r.sections.filter((s) => allowed.includes(s[0])) : r.sections);
        setPage(next);
        setDone(r.done || !r.items.length);
      } catch (e) {
        if (mine !== latest.current) return;
        // a new search that failed: the old results aren't its results
        if (reset) setItems([]);
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

  // searched on Enter or the Search button only (GameBanana isn't asked on every keystroke);
  // clearing the box goes back to browsing at once
  useEffect(() => {
    if (!q.trim() && query) setQuery("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q]);
  const search = () => {
    const t = q.trim();
    if (t === query) load(true);
    else setQuery(t);
  };
  // a new search (or browse) on its way back
  const searching = loading && lastReset.current;

  // install progress
  const first = useRef(true);
  usePoll(async () => {
    const list = await getJson<GbJob[]>("/api/gb/jobs").catch(() => [] as GbJob[]);
    if (!Array.isArray(list)) return;
    setJobs(list);
    // jobs that had already ended before this page opened were announced then
    if (first.current) {
      first.current = false;
      list.filter((j) => j.state === "done" || j.state === "error").forEach((j) => seenDone.current.add(j.id));
      return;
    }
    for (const j of list) {
      if ((j.state === "done" || j.state === "error") && !seenDone.current.has(j.id)) {
        seenDone.current.add(j.id);
        toast(j.message || (j.state === "done" ? "Installed" : "Couldn't install that."), j.state === "error");
        if (j.state === "done") {
          onInstalled();
          setItems((old) => old.map((m) => (m.id === j.mod ? { ...m, installed: true } : m)));
        }
      }
    }
  }, 1500);

  const details = (id: number, name: string) => run(key(`details:${id}`), async () => {
    // (opening another server meanwhile cancels it: nothing to say then)
    const d = await getJson<GbDetails>(`/api/gb/mod/${id}?section=${section}`).catch((e) => (isAbort(e) ? undefined : null));
    if (d === undefined) return;
    if (d && !(d as unknown as { error?: string }).error) setOpen(d);
    else toast("Couldn't load the details from GameBanana. Try again.", true);
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

  const active = jobs.filter((j) => !["done", "error"].includes(j.state));
  const jobFor = (mod: number) => active.find((j) => j.mod === mod);
  const jobText = (j: GbJob) => (j.size && (j.got ?? 0) < j.size ? `Downloading ${Math.round(((j.got ?? 0) / j.size) * 100)}%` : j.size ? "Installing…" : "Downloading…");
  const label = sections.find((s) => s[0] === section)?.[1] ?? "Maps";

  return (
    <div className="stack">
      <div>
        <SectionHead title={title} note={`From GameBanana's Counter-Strike 1.6 section. ${server ? (section === "maps" ? NOTES.maps : SERVER_NOTE) : NOTES[section] ?? NOTES.default}`}>
          {back && <button className="btn" onClick={back.go}>{back.label}</button>}
          {ON_THIS_COMPUTER && !isSharedId(targetKey()) && (
            <BusyButton busy={busy(key(`add-file:${section}`))} label="Add from a file" busyLabel="Adding…" onClick={() => run(key(`add-file:${section}`), async () => {
              const r = await addFromFile(section);
              if (!r) return;
              toast(r.message, !r.ok);
              if (r.ok) onInstalled();
            }, `Adding ${label.toLowerCase()} from a file…`)} />
          )}
        </SectionHead>
        {server && !only && <SlotsBar />}
        {/* where what's downloaded is: under the heading, so the tabs sit on the search they go with */}
        {!only && !server && (
          <p className="muted small-note">
            What you've downloaded is in My library, to remove what you don't want:{" "}
            {LIBRARY.map(([id, f, name], i) => (
              <span key={id}>
                {i ? (i === LIBRARY.length - 1 ? " and " : ", ") : ""}
                <button type="button" className="link inline" onClick={() => useNav.getState().go(id, f)}>{name}</button>
              </span>
            ))}
            .
          </p>
        )}
        {(!only || sections.length > 1) && <div className="chips section-tabs" role="tablist" aria-label="What to get">
          {sections.map(([id, name]) => (
            <button key={id} className="chip" role="tab" aria-selected={section === id} onClick={() => {
              // the same tab again keeps what's shown and checks for new
              if (id === section) return void load(true);
              setSection(id);
              setCat(null);
              setItems([]);
            }}>
              {name}
            </button>
          ))}
        </div>}
        {active.map((j) => (
          <div className="job" key={j.id}>
            <span>{j.message || j.state}</span>
            {j.size ? <progress max={j.size} value={j.got ?? 0} /> : null}
          </div>
        ))}
        <div className="toolbar">
          <form className="gb-search" role="search" onSubmit={(e) => { e.preventDefault(); search(); }}>
            <input className="search" type="search" aria-label={`Search ${label.toLowerCase()}`} placeholder={`Search ${label.toLowerCase()}`} value={q} onChange={(e) => setQ(e.target.value)} />
            <BusyButton type="submit" busy={searching && !!query} label="Search" busyLabel="Searching…" />
          </form>
          {searching && !query && <span className="search-status" role="status"><span className="spinner" aria-hidden="true" />Loading…</span>}
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
      {error && !items.length && !loading && <LoadError text={error} onRetry={() => load(lastReset.current)} />}
      <div className={`maps grid${searching && items.length ? " stale" : ""}`} aria-busy={searching}>
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
                  {m.installed && !job && !busy(key(`details:${m.id}`)) && <span className="badge">Installed</span>}
                  {job && <span className="badge busy" role="status">{jobText(job)}</span>}
                  {!job && busy(key(`details:${m.id}`)) && <span className="badge busy" role="status"><span className="spinner" aria-hidden="true" />Opening…</span>}
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
        {!items.length && !loading && !error && <p className="muted">Nothing matches. Try another search or type.</p>}
        {!items.length && loading && Array.from({ length: 10 }, (_, i) => <div key={i} className="map-card ghost" aria-hidden="true"><div className="map-pic" /><span className="ghost-label" /></div>)}
      </div>
      {!done && items.length > 0 && (
        <div className="row center">
          {error && !loading && <p className="err-text" role="alert">{error}</p>}
          <BusyButton busy={loading} label={error ? "Try again" : "Show more"} busyLabel="Loading…" onClick={() => load(error ? lastReset.current : false)} />
        </div>
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
                <div>Installed.{ON_THIS_COMPUTER ? "" : " Remove it on the computer."}</div>
                {startMap && <button className="btn primary" onClick={() => startOn(startMap)}>Start a game on {startMap}</button>}
                {ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={busy(removeKey(section, open.id))} label="Remove" busyLabel="Removing…" onClick={() => remove(open.id, open.name)} />}
              </div>
            ) : (
              <ul className="files">
                {open.files.map((f) => (
                  <li key={f.id}>
                    <div>
                      <span>{f.file} <small>{size(f.size)}</small></span>
                      {f.replaces && f.replaces.length > 0 && (
                        <p className="muted small-note">Replaces {f.replaces.length === 1 ? f.replaces[0] : `${f.replaces.length} game files (${f.replaces.slice(0, 3).join(", ")}${f.replaces.length > 3 ? " and more" : ""})`}. Backed up first.</p>
                      )}
                      {f.conflicts && f.conflicts.length > 0 && (
                        <p className="err-text small-note">Also changes files from {[...new Set(f.conflicts.map((c) => c.name))].join(", ")}. Removing either later puts things back in order.</p>
                      )}
                      {f.listed === false && <p className="muted small-note">Its contents are checked after downloading.</p>}
                    </div>
                    {f.virus ? <span className="muted">Flagged by GameBanana's virus scan</span> : !ON_THIS_COMPUTER ? <span className="muted small-note">Install it on the computer.</span> : f.supported ? (
                      <BusyButton className="btn primary" busy={busy(key(`install:${open.id}:${f.id}`))} disabled={!!jobFor(open.id)} label={jobFor(open.id) ? jobText(jobFor(open.id)!) : server && section !== "maps" ? "Use on this server" : "Install"} busyLabel="Installing…" onClick={() => install(open.id, f.id, open.name)} />
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

type Installed = { mod: number; section: string; name: string; author?: string | null; url?: string | null; date?: number; maps?: string[]; files?: number; local?: boolean; server?: boolean; who?: string; on?: boolean };
const WHO: [string, string][] = [["all", "Everyone"], ["ct", "Counter-Terrorists"], ["t", "Terrorists"], ["admin", "Admins only"]];

/** What the last map loaded, against the engine's 512-a-kind limit (over it the server stops). */
function SlotsBar() {
  const [d, setD] = useState<ServerContent | null>(null);
  useEffect(() => {
    getJson<ServerContent>("/api/servercontent").then(setD).catch(() => setD(null));
  }, []);
  if (!d) return null;
  const s = d.slots;
  if (!s || s.models === undefined) {
    return <p className="muted small-note">The server counts what each map loads (models and sounds, 512 of each at most) when it next starts a map.</p>;
  }
  const near = (n?: number) => (n ?? 0) >= d.limit - 32;
  return (
    <p className={`small-note slots-bar${near(s.models) || near(s.sounds) ? " err-text" : " muted"}`}>
      {s.map ? `${s.map}: ` : ""}models {s.models} of {d.limit}, sounds {s.sounds} of {d.limit}.
      {near(s.models) || near(s.sounds) ? " That's close to the limit: a map that loads more stops the server. Take something off before adding more." : " Each skin uses one to three model slots, a player model one."}
    </p>
  );
}
const SECTION_NAME: Record<string, string> = { maps: "Map", skins: "Weapon skin", players: "Player model", sounds: "Sound", sprays: "Spray", sprites: "Effect", gui: "HUD & menus" };

/** Everything downloaded into this game, in one place, to remove what you don't want. */
function InstalledList({ onChanged, server, only, empty = "Nothing downloaded yet. Anything you install from here shows up in this list." }: { onChanged: () => void; server?: boolean; only?: string[]; empty?: string }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [list, setList] = useState<Installed[] | null>(null);
  const [failed, setFailed] = useState(false);
  const load = () => {
    setFailed(false);
    return getJson<Installed[]>("/api/gb/installed")
      .then((l) => setList(Array.isArray(l) ? l.filter((it) => !only || only.includes(it.section)) : []))
      .catch((e) => !isAbort(e) && setFailed(true));
  };
  useEffect(() => {
    load();
  }, []);
  const remove = async (it: Installed) => {
    const what = it.section === "maps" ? `Its map${(it.maps?.length ?? 0) === 1 ? " is" : "s are"} deleted.` : it.server ? "It's taken off this server from the next map." : "The game's original files are put back.";
    if (!(await ask(`Remove ${it.name}? ${what}`, { ok: "Remove", danger: true }))) return;
    await run(removeKey(it.section, it.mod), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/gb/uninstall", { mod: it.mod, section: it.section }).catch(() => ({ error: "Couldn't remove it." }));
      if (r.error) return toast(r.error, true);
      toast(`Removed ${it.name}`);
      load();
      onChanged();
    }, `Removing ${it.name}…`);
  };
  const setKey = (it: Installed) => `getmore:${targetKey()}:set:${it.section}:${it.mod}`;
  const setItem = (it: Installed, change: { who?: string; on?: boolean }) =>
    run(setKey(it), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/servercontent/set", { mod: it.mod, section: it.section, ...change }).catch(() => ({ error: "Couldn't save it." }));
      if (r.error) return toast(r.error, true);
      toast(`${it.name} changed. It applies from the next map.`);
      load();
    }, `Saving ${it.name}…`);
  if (failed && !list) return <LoadError text="Couldn't load what's installed." onRetry={load} />;
  if (!list) return <Skeleton />;
  if (!list.length) return <p className="muted">{empty}</p>;
  return (
    <>
    {!ON_THIS_COMPUTER && <p className="muted small-note">To remove one, use the computer.</p>}
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
            {server && it.server && ON_THIS_COMPUTER && (
              <>
                <select aria-label={`Who sees ${it.name}`} value={it.who ?? "all"} disabled={busy(setKey(it))} onChange={(e) => setItem(it, { who: e.target.value })}>
                  {WHO.map(([w, l]) => <option key={w} value={w}>{l}</option>)}
                </select>
                <Switch on={it.on !== false} label={`${it.name} on`} disabled={busy(setKey(it))} onChange={(v) => setItem(it, { on: v })} />
              </>
            )}
            {it.url && <a className="btn" href={it.url} target="_blank" rel="noreferrer">Open on GameBanana</a>}
            {ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={busy(removeKey(it.section, it.mod))} label="Remove" busyLabel="Removing…" onClick={() => remove(it)} />}
          </div>
        </li>
      ))}
    </ul>
    </>
  );
}

/** A server's weapon skins and player models: what it uses, who sees each, and the map's
 *  model and sound count against the engine's limit. Getting more opens the browser. */
/** What you have of some kinds of GameBanana content in your own game, with Remove on each
 *  and a way to get more of that kind (Get more, opened on it): a chip per kind when there's
 *  more than one. */
const KIND_NAMES: Record<string, string> = { skins: "Weapon skins", players: "Player models", sprays: "Sprays", sprites: "Effects", gui: "HUD & menus", sounds: "Sounds" };
export function MyContent({ kinds, title, note, empty }: { kinds: string[]; title: string; note: string; empty: string }) {
  const [kind, setKind] = useState<string>(kinds.length > 1 ? "all" : kinds[0]);
  const these = kind === "all" ? kinds : [kind];
  const name = kind === "all" ? title.toLowerCase() : KIND_NAMES[kind]?.toLowerCase() ?? kind;
  // Get more, on this kind (Back comes back here)
  const shop = () => useNav.getState().go("more", these[0]);
  return (
    <div className="stack">
      {/* the heading, its filter and note together (as Get more and Maps have them), then the list */}
      <div>
        <SectionHead title={title} note={note}>
          {ON_THIS_COMPUTER && <button className="btn" onClick={shop}>{`Get ${name}`}</button>}
        </SectionHead>
        {kinds.length > 1 && (
          <div className="chips" role="group" aria-label="Show">
            {["all", ...kinds].map((k) => (
              <button key={k} className="chip" aria-pressed={kind === k} onClick={() => setKind(k)}>{k === "all" ? "All" : KIND_NAMES[k] ?? k}</button>
            ))}
          </div>
        )}
        {!ON_THIS_COMPUTER && <p className="muted small-note">To add or remove them, use the computer.</p>}
      </div>
      <InstalledList key={kind} only={these} onChanged={() => {}} empty={empty} />
    </div>
  );
}

export function MySkins() {
  return (
    <MyContent
      kinds={["skins", "players", "sprays", "sprites", "gui"]}
      title="Skins & models"
      note="Weapon skins, player models, sprays, effects and HUD in your game. Only you see them. Removing one puts the game's own back."
      empty="Nothing here yet: your game uses its own. Get some from GameBanana with the button above."
    />
  );
}

export function ServerSkins() {
  const [shop, setShop] = useState<string | null>(null);
  // a new list after the browser, for what was added there
  const [seen, setSeen] = useState(0);
  if (shop) {
    return (
      <GetMore
        server
        only={[shop]}
        title={shop === "skins" ? "Get weapon skins" : "Get player models"}
        back={{ label: "Back to skins & models", go: () => { setShop(null); setSeen((n) => n + 1); } }}
        onInstalled={() => {}}
      />
    );
  }
  return (
    <div className="stack">
      <div>
        <SectionHead title="Skins & models" note="Weapon skins and player models this server uses. Players download them when they join. Changes apply from the next map.">
          {ON_THIS_COMPUTER && (
            <span className="row">
              <button className="btn" onClick={() => setShop("skins")}>Get weapon skins</button>
              <button className="btn" onClick={() => setShop("players")}>Get player models</button>
            </span>
          )}
        </SectionHead>
        <SlotsBar />
        <p className="muted small-note">Modes with their own models keep them: nothing changes in Paintball, Hide and Seek, Base Builder or the zombie modes, and Superhero and Warcraft 3 keep their own player models.</p>
      </div>
      <InstalledList
        key={seen}
        server
        only={["skins", "players"]}
        onChanged={() => {}}
        empty="This server uses the game's own skins and models. Get some above: they go in under their own names, so the game's files stay as they are."
      />
    </div>
  );
}
