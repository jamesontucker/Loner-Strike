// The start page of a tool that works on one map (radar maker, callouts, nav fixer, map doctor,
// ...): search as you type, filters (has nav, has overview, has callouts, problems, recently
// used, map type de_/cs_/surf_/...), a grid of cards with the map's picture (or a neat
// placeholder) or a compact list, the tool's own badges per map, recent maps at the top, the
// keyboard (arrows, Page Up/Down, Home/End, Enter; ↓ from the search box), and only the rows on
// screen are drawn, so 400+ maps stay quick.
//
//   <MapBrowser tool="navfix" maps={maps} onPick={setMap}
//     title="Pick a map to fix its nav" action="Fix nav"
//     filter={(m, f) => !!f?.nav}                       // only these maps at all (optional)
//     badges={(m, f) => f?.nav ? [{ text: `nav v${f.nav.version}` }] : [{ text: "no nav yet", tone: "warn" }]}
//     none="None of your maps has a bot nav yet." />
//
// Facts per map come from /api/tools/maps (nav version and places, knowledge, missing WADs);
// pass `facts` to use your own.

import { useQuery } from "@tanstack/react-query";
import { forwardRef, memo, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MutableRefObject, type ReactNode } from "react";
import { withTarget } from "../../../api";
import type { MapInfo } from "../../../types";
import { COLLATOR } from "../../Maps";
import { get } from "./api";
import { Empty } from "./index";
import { matches, searchWords } from "./search";
import "./ux.css";

/** What /api/tools/maps knows about a map. */
export type MapFacts = {
  name: string;
  bspSize?: number;
  nav?: { version: number; places?: number; bspSize?: number; matchesMap?: boolean } | null;
  lsv?: { stage?: number; done?: boolean } | null;
  missingWads?: string[];
};
export type MapBadge = { text: ReactNode; tone?: "ok" | "warn" | "bad" | "info"; title?: string };
export type MapFilter = { id: string; label: string; test: (m: MapInfo, f: MapFacts | undefined, ctx: { recent: Set<string> }) => boolean };

/** The filters every map browser offers (pass `filters` to change them). */
export const MAP_FILTERS: MapFilter[] = [
  { id: "recent", label: "Recently used", test: (m, _f, c) => c.recent.has(m.name) },
  { id: "nav", label: "Has bot nav", test: (m, f) => !!(f ? f.nav : m.nav) },
  { id: "nonav", label: "No nav yet", test: (m, f) => !(f ? f.nav : m.nav) },
  { id: "overview", label: "Has overview", test: (m) => m.thumb === "radar" },
  { id: "callouts", label: "Has callouts", test: (_m, f) => (f?.nav?.places ?? 0) > 0 },
  { id: "problems", label: "Problems found", test: (m, f) => !!defaultProblem(m, f) },
];

/** Missing textures or a nav made for another version of the map. */
export function defaultProblem(_m: MapInfo, f: MapFacts | undefined): string | null {
  if (!f) return null;
  if (f.missingWads?.length) return `${f.missingWads.length} texture file${f.missingWads.length === 1 ? "" : "s"} missing`;
  if (f.nav && f.nav.matchesMap === false) return "nav made for another version";
  return null;
}

// ---- recently used, per tool ----

const RECENT = (tool: string) => `ls.tools.recentMaps.${tool}`;
export function recentMaps(tool: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT(tool)) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, 12) : [];
  } catch {
    return [];
  }
}
/** Puts a map first in the tool's recent maps (MapBrowser does it on a pick). */
export function rememberMap(tool: string, name: string) {
  try {
    localStorage.setItem(RECENT(tool), JSON.stringify([name, ...recentMaps(tool).filter((x) => x !== name)].slice(0, 12)));
  } catch {
    /* private window */
  }
}

const VIEW = "ls.tools.mapView";
const savedView = (): "grid" | "list" => {
  try {
    return localStorage.getItem(VIEW) === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
};

/** "de_dust2" -> "de" (the map's kind), "" when it has none. */
export function mapPrefix(name: string): string {
  const i = name.indexOf("_");
  return i > 0 && i <= 6 ? name.slice(0, i).toLowerCase() : "";
}

export function useMapFacts(enabled = true) {
  return useQuery({
    queryKey: ["tools-map-facts"],
    queryFn: ({ signal }) => get<{ maps: MapFacts[] }>("/api/tools/maps", signal),
    staleTime: 30_000,
    enabled,
    select: (d) => new Map(d.maps.map((m) => [m.name.toLowerCase(), m])),
  });
}

export type MapBrowserProps = {
  /** the tool's id (recent maps are per tool) */
  tool: string;
  maps: MapInfo[];
  onPick: (name: string) => void;
  title?: ReactNode;
  note?: ReactNode;
  placeholder?: string;
  /** the words on a card's button ("Open", "Fix nav", "Make radar") */
  action?: string | ((m: MapInfo) => string);
  /** only these maps at all */
  filter?: (m: MapInfo, f: MapFacts | undefined) => boolean;
  /** what to say when `filter` leaves no maps */
  none?: string;
  /** the tool's own badges on each map */
  badges?: (m: MapInfo, f: MapFacts | undefined) => MapBadge[];
  /** a problem line (default: missing textures, nav out of date); return null for none */
  problem?: (m: MapInfo, f: MapFacts | undefined) => string | null;
  /** the filter chips (default MAP_FILTERS) */
  filters?: MapFilter[];
  /** facts of your own instead of /api/tools/maps */
  facts?: Map<string, MapFacts>;
  /** buttons at the right of the search row */
  extra?: ReactNode;
  /** the map open last (marked on its card) */
  current?: string | null;
};

const CARD_H = 206; // grid row (card + gap), px
const LIST_H = 44; // list row, px
const MIN_CARD = 210;
const GAP = 12;

export function MapBrowser({ tool, maps, onPick, title, note, placeholder = "Search maps", action = "Open", filter, none, badges, problem = defaultProblem, filters = MAP_FILTERS, facts: ownFacts, extra, current }: MapBrowserProps) {
  const factsQ = useMapFacts(!ownFacts);
  const facts = ownFacts ?? factsQ.data;
  const factOf = useCallback((m: MapInfo) => facts?.get(m.name.toLowerCase()), [facts]);
  const [q, setQ] = useState("");
  const [on, setOn] = useState<string[]>([]);
  const [kind, setKind] = useState("");
  const [view, setView] = useState(savedView);
  const [recent] = useState(() => recentMaps(tool));
  const recentSet = useMemo(() => new Set(recent), [recent]);

  const all = useMemo(() => maps.filter((m) => !m.other && (!filter || filter(m, factOf(m)))), [maps, filter, factOf]);
  const kinds = useMemo(() => {
    const n = new Map<string, number>();
    for (const m of all) n.set(mapPrefix(m.name), (n.get(mapPrefix(m.name)) ?? 0) + 1);
    const big = [...n].filter(([k, c]) => k && c >= 3).sort((a, b) => b[1] - a[1]).slice(0, 9);
    const other = all.length - big.reduce((s, [, c]) => s + c, 0);
    return { big, other };
  }, [all]);
  const bigKinds = useMemo(() => new Set(kinds.big.map(([k]) => k)), [kinds]);

  const shown = useMemo(() => {
    const w = searchWords(q);
    const tests = filters.filter((f) => on.includes(f.id));
    return all
      .filter((m) => {
        if (!matches(w, m.name, m.title)) return false;
        const p = mapPrefix(m.name);
        if (kind === "~" ? bigKinds.has(p) : kind && p !== kind) return false;
        const f = factOf(m);
        return tests.every((t) => t.test(m, f, { recent: recentSet }));
      })
      .sort((a, b) => COLLATOR.compare(a.name, b.name));
  }, [all, q, on, kind, filters, factOf, recentSet, bigKinds]);

  const recentRow = useMemo(() => {
    const byName = new Map(all.map((m) => [m.name, m]));
    let list = recent.map((n) => byName.get(n)).filter((m): m is MapInfo => !!m);
    let label = "Recently used";
    if (!list.length) {
      list = all.filter((m) => m.recent > 0).sort((a, b) => b.recent - a.recent);
      label = "Played lately";
    }
    return { list: list.slice(0, 6), label };
  }, [all, recent]);

  const pick = (name: string) => {
    rememberMap(tool, name);
    onPick(name);
  };
  const toggle = (id: string) => setOn((o) => (o.includes(id) ? o.filter((x) => x !== id) : [...o, id]));
  const setViewSaved = (v: "grid" | "list") => {
    setView(v);
    try {
      localStorage.setItem(VIEW, v);
    } catch {
      /* private window */
    }
  };

  const grid = useRef<HTMLDivElement>(null);
  const actionOf = (m: MapInfo) => (typeof action === "function" ? action(m) : action);
  const searching = !!q.trim() || on.length > 0 || !!kind;

  if (!all.length)
    return (
      <div className="tl-mb">
        {title && <h3 className="tl-mb-title">{title}</h3>}
        <Empty title="No maps here yet" text={maps.length && none ? none : "Loner-Strike can't find any maps in your game yet. Add some from Get more or the Maps page."} />
      </div>
    );

  return (
    <div className="tl-mb">
      {(title || note) && (
        <div className="tl-mb-head">
          {title && <h3 className="tl-mb-title">{title}</h3>}
          {note && <p className="muted tl-mb-note">{note}</p>}
        </div>
      )}

      <div className="tl-mb-bar">
        <input
          className="search tl-mb-search"
          type="search"
          aria-label="Search maps"
          placeholder={placeholder}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown" && shown.length) {
              e.preventDefault();
              grid.current?.dispatchEvent(new CustomEvent("tl-mb-focus", { detail: 0 }));
            } else if (e.key === "Enter" && shown.length && q.trim()) {
              e.preventDefault();
              pick(shown[0].name);
            }
          }}
        />
        <span className="muted tl-mb-count" aria-live="polite">
          {shown.length === all.length ? `${all.length} maps` : `${shown.length} of ${all.length}`}
        </span>
        <div className="seg tl-mb-view" role="group" aria-label="Show as">
          <button type="button" aria-pressed={view === "grid"} onClick={() => setViewSaved("grid")} title="Cards">
            <GridIcon /> <span className="tl-mb-view-word">Cards</span>
          </button>
          <button type="button" aria-pressed={view === "list"} onClick={() => setViewSaved("list")} title="List">
            <ListIcon /> <span className="tl-mb-view-word">List</span>
          </button>
        </div>
        {extra}
      </div>

      <div className="tl-mb-filters" role="group" aria-label="Filters">
        {filters.map((f) => (
          <button key={f.id} type="button" className="tl-chip" aria-pressed={on.includes(f.id)} onClick={() => toggle(f.id)}>
            {f.label}
          </button>
        ))}
        {kinds.big.length > 1 && (
          <>
            <span className="tl-mb-sep" aria-hidden="true" />
            <button type="button" className="tl-chip" aria-pressed={kind === ""} onClick={() => setKind("")}>
              All types
            </button>
            {kinds.big.map(([k, c]) => (
              <button key={k} type="button" className="tl-chip" aria-pressed={kind === k} onClick={() => setKind(kind === k ? "" : k)} title={`${c} maps`}>
                {k}_
              </button>
            ))}
            {kinds.other > 0 && (
              <button type="button" className="tl-chip" aria-pressed={kind === "~"} onClick={() => setKind(kind === "~" ? "" : "~")} title={`${kinds.other} maps`}>
                Other
              </button>
            )}
          </>
        )}
        {searching && (
          <button type="button" className="tl-chip clear" onClick={() => (setQ(""), setOn([]), setKind(""))}>
            Clear
          </button>
        )}
      </div>

      {!searching && recentRow.list.length > 0 && (
        <section className="tl-mb-recent" aria-label={recentRow.label}>
          <h4>{recentRow.label}</h4>
          <div className="tl-mb-recent-row">
            {recentRow.list.map((m) => (
              <button key={m.name} type="button" className="tl-mb-recent-card" onClick={() => pick(m.name)} title={`${actionOf(m)}: ${m.name}`}>
                <Thumb m={m} small />
                <span className="tl-mb-name">{m.name}</span>
              </button>
            ))}
          </div>
        </section>
      )}

      {shown.length ? (
        <Windowed ref={grid} items={shown} view={view} current={current} onPick={pick} render={(m) => ({ facts: factOf(m), badges: badges?.(m, factOf(m)) ?? [], problem: problem(m, factOf(m)), action: actionOf(m) })} />
      ) : (
        <Empty plain text={q.trim() ? `No maps match “${q.trim()}”${on.length || kind ? " with these filters" : ""}.` : "No maps with these filters."}>
          <button type="button" className="btn" onClick={() => (setQ(""), setOn([]), setKind(""))}>
            Show all maps
          </button>
        </Empty>
      )}
    </div>
  );
}

// ---- the windowed grid/list -----------------------------------------------------------------

type CardBits = { facts: MapFacts | undefined; badges: MapBadge[]; problem: string | null; action: string };

function scrollParent(el: HTMLElement | null): HTMLElement | null {
  for (let p = el?.parentElement; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if ((o === "auto" || o === "scroll") && p.scrollHeight > p.clientHeight) return p;
  }
  return null;
}

const Windowed = forwardRef<HTMLDivElement, { items: MapInfo[]; view: "grid" | "list"; current?: string | null; onPick: (name: string) => void; render: (m: MapInfo) => CardBits }>(function Windowed({ items, view, current, onPick, render }, outer) {
  const box = useRef<HTMLDivElement>(null);
  const uid = useId();
  const [cols, setCols] = useState(4);
  const [range, setRange] = useState<[number, number]>([0, 12]);
  const [cur, setCur] = useState(-1);
  const rowH = view === "grid" ? CARD_H : LIST_H;
  const perRow = view === "grid" ? cols : 1;
  const rows = Math.ceil(items.length / perRow);

  const setRefs = (el: HTMLDivElement | null) => {
    (box as MutableRefObject<HTMLDivElement | null>).current = el;
    if (typeof outer === "function") outer(el);
    else if (outer) outer.current = el;
  };

  // which rows are on screen (in the nearest scrolling box, or the window)
  const measure = useCallback(() => {
    const el = box.current;
    if (!el) return;
    const w = el.clientWidth;
    const c = Math.max(1, Math.floor((w + GAP) / (MIN_CARD + GAP)));
    setCols(c);
    const r = el.getBoundingClientRect();
    const scale = el.offsetHeight ? r.height / el.offsetHeight : 1;
    const sc = scrollParent(el);
    const vr = sc ? sc.getBoundingClientRect() : { top: 0, bottom: window.innerHeight };
    const h = rowH * scale;
    const first = Math.max(0, Math.floor((vr.top - r.top) / h) - 3);
    const last = Math.max(first + 1, Math.ceil((vr.bottom - r.top) / h) + 3);
    setRange((old) => (old[0] === first && old[1] === last ? old : [first, last]));
  }, [rowH]);

  useLayoutEffect(() => {
    measure();
    const el = box.current;
    if (!el) return;
    const sc = scrollParent(el) ?? window;
    const on = () => measure();
    sc.addEventListener("scroll", on, { passive: true });
    window.addEventListener("resize", on);
    const ro = new ResizeObserver(on);
    ro.observe(el);
    return () => {
      sc.removeEventListener("scroll", on);
      window.removeEventListener("resize", on);
      ro.disconnect();
    };
  }, [measure, items.length]);

  // keyboard: move between cards; scroll the one moved to into view, then focus it
  const reveal = useCallback(
    (i: number) => {
      const el = box.current;
      if (!el) return;
      const n = Math.max(0, Math.min(items.length - 1, i));
      setCur(n);
      const r = el.getBoundingClientRect();
      const scale = el.offsetHeight ? r.height / el.offsetHeight : 1;
      const sc = scrollParent(el);
      const vr = sc ? sc.getBoundingClientRect() : { top: 0, bottom: window.innerHeight };
      const top = r.top + Math.floor(n / perRow) * rowH * scale;
      const bottom = top + rowH * scale;
      const by = top < vr.top + 8 ? top - vr.top - 8 : bottom > vr.bottom - 80 ? bottom - vr.bottom + 80 : 0;
      if (by) (sc ?? window).scrollBy({ top: by });
      requestAnimationFrame(() => requestAnimationFrame(() => el.querySelector<HTMLButtonElement>(`[data-i="${n}"]`)?.focus({ preventScroll: true })));
    },
    [items.length, perRow, rowH],
  );
  useEffect(() => {
    const el = box.current;
    const f = (e: Event) => reveal((e as CustomEvent<number>).detail);
    el?.addEventListener("tl-mb-focus", f);
    return () => el?.removeEventListener("tl-mb-focus", f);
  }, [reveal]);
  useEffect(() => setCur(-1), [items]);

  const keys = (e: KeyboardEvent) => {
    const i = Number((e.target as HTMLElement).dataset?.i ?? -1);
    if (i < 0) return;
    const page = Math.max(1, Math.floor((range[1] - range[0] - 6) * perRow));
    const to: Record<string, number> = { ArrowRight: i + 1, ArrowLeft: i - 1, ArrowDown: i + perRow, ArrowUp: i - perRow, PageDown: i + page, PageUp: i - page, Home: 0, End: items.length - 1 };
    if (e.key in to) {
      e.preventDefault();
      if (e.key === "ArrowUp" && i - perRow < 0) return document.querySelector<HTMLInputElement>(".tl-mb-search")?.focus();
      reveal(to[e.key]);
    }
  };

  const from = Math.min(range[0], rows) * perRow;
  const to = Math.min(items.length, range[1] * perRow);
  const slice = items.slice(from, to);
  return (
    <div
      ref={setRefs}
      className={`tl-mb-win ${view}`}
      role="listbox"
      aria-label="Maps"
      style={{ height: rows * rowH }}
      onKeyDown={keys}
    >
      <div className={`tl-mb-items ${view}`} style={{ transform: `translateY(${Math.floor(from / perRow) * rowH}px)`, gridTemplateColumns: view === "grid" ? `repeat(${cols}, minmax(0, 1fr))` : undefined }}>
        {slice.map((m, k) => {
          const i = from + k;
          return <Card key={m.name} m={m} i={i} id={`${uid}-${i}`} view={view} tab={(cur < 0 ? i === 0 : i === cur) ? 0 : -1} here={m.name === current} bits={render(m)} onPick={onPick} />;
        })}
      </div>
    </div>
  );
});

const Card = memo(function Card({ m, i, id, view, tab, here, bits, onPick }: { m: MapInfo; i: number; id: string; view: "grid" | "list"; tab: number; here: boolean; bits: CardBits; onPick: (name: string) => void }) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={here}
      id={id}
      data-i={i}
      tabIndex={tab}
      className={`tl-mb-card ${view}${here ? " here" : ""}`}
      onClick={() => onPick(m.name)}
      title={m.title && m.title !== m.name ? `${m.name}: ${m.title}` : m.name}
    >
      <Thumb m={m} small={view === "list"} />
      <span className="tl-mb-body">
        <span className="tl-mb-line">
          <span className="tl-mb-name">{m.name}</span>
          {m.title && m.title !== m.name && <span className="muted tl-mb-sub">{m.title}</span>}
        </span>
        <span className="tl-mb-badges">
          {m.source === "download" && <span className="tl-badge">Downloaded</span>}
          {m.source === "server" && <span className="tl-badge" title="The game got it from a server you joined (in its _downloads folder)">From a server</span>}
          {bits.badges.map((b, k) => (
            <span key={k} className={`tl-badge${b.tone ? ` ${b.tone}` : ""}`} title={b.title}>
              {b.text}
            </span>
          ))}
          {bits.problem && <span className="tl-badge bad">{bits.problem}</span>}
        </span>
      </span>
      <span className="tl-mb-go" aria-hidden="true">
        {bits.action}
      </span>
    </button>
  );
});

/** The map's picture (overview or screenshot) when the app has one; its name drawn neatly when not. */
export function Thumb({ m, small }: { m: MapInfo; small?: boolean }) {
  const [broken, setBroken] = useState(false);
  const p = mapPrefix(m.name);
  return (
    <span className={`tl-mb-thumb${small ? " small" : ""}${m.thumb === "radar" ? " radar" : ""}`} aria-hidden="true">
      {(!m.thumb || broken || m.thumb === "radar") && (
        <span className="tl-mb-nopic">
          {p && <span className="tl-mb-nopic-kind">{p}</span>}
          {!small && <span className="tl-mb-nopic-name">{p ? m.name.slice(p.length + 1) : m.name}</span>}
        </span>
      )}
      {m.thumb && !broken && <img src={withTarget(`/thumb/${encodeURIComponent(m.name)}.png?v=${m.thumb}`)} alt="" loading="lazy" decoding="async" onError={() => setBroken(true)} />}
    </span>
  );
}

const GridIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
    <rect x="3.5" y="3.5" width="7" height="7" rx="1" />
    <rect x="13.5" y="3.5" width="7" height="7" rx="1" />
    <rect x="3.5" y="13.5" width="7" height="7" rx="1" />
    <rect x="13.5" y="13.5" width="7" height="7" rx="1" />
  </svg>
);
const ListIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
    <path d="M4 6h16M4 12h16M4 18h16" />
  </svg>
);
