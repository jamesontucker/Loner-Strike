// The callout editor: name places on a map's bot nav ("Catwalk", "Apartments"), so bots say
// where they are on the radio and bot chat and voice commands understand the names.
//
// Layout: the nav seen from above fills the stage; the side has the name palette (the brush),
// the places with their counts and colours, suggestions and checks. Pick areas and name them,
// or pick a name and paint it on (P), erase (E). Undo/Redo and Save are in the bar.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useDirty, useNav } from "../../../store";
import { useToolArg } from "../../../stores/tools";
import { useReportAddress } from "../common/address";
import { Banner, BusyButton, Info, LoadError, Segmented, useAskText, useConfirm, useToast } from "../../../ui";
import { Empty, Loading } from "../common";
import { MapBrowser, rememberMap } from "../common/MapBrowser";
import { ActionBar } from "../common/ActionBar";
import type { ToolProps } from "../registry";
import { loadCallouts, placeColours, saveCallouts, spoken, suggest, validPlace, type Area, type CalloutNav, type Change } from "./calloutsApi";
import { TopDown, type Tool } from "./TopDown";
import "./callouts.css";

export default function Callouts({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("callouts", map ? `callouts:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  useEffect(() => {
    if (map) rememberMap("callouts", map);
  }, [map]);
  if (!map)
    return (
      <MapBrowser
        tool="callouts"
        maps={maps}
        onPick={setMap}
        title="Pick a map to name its places"
        note={<>Only maps with a bot nav. <Info text={'Bots say these names on the radio ("enemy spotted at Long"), and bot chat and voice commands understand them. Only maps with a bot nav are listed: the map doctor can teach bots the others.'} /></>}
        action="Name places"
        filter={(m, f) => !!(f ? f.nav : m.nav)}
        none="None of your maps has a bot nav yet. The map doctor can teach bots a map; then you can name its places here."
        badges={(_m, f) => (!f?.nav ? [] : f.nav.places ? [{ text: `${f.nav.places} place${f.nav.places === 1 ? "" : "s"}`, tone: "ok" }] : [{ text: "No places yet", tone: "warn" }])}
      />
    );
  return <Editor map={map} onBack={() => setMap(null)} />;
}

/** names by area id */
type Names = Record<number, string>;
type Issue = { key: string; tone: "warn" | "bad" | "info"; text: string; areas: number[]; fix?: { label: string; run: () => void } };

const TOOL_HINT: Record<Tool, string> = {
  pick: "Click to pick · Shift adds · drag a box · double-click picks a place · right-drag moves",
  paint: "Drag over areas to give them the name · right-drag or Space moves · scroll zooms",
  erase: "Drag over areas to take their name off · right-drag or Space moves · scroll zooms",
};

const norm = (n: string) => n.toLowerCase().replace(/[_\s]/g, "");

function Editor({ map, onBack }: { map: string; onBack: () => void }) {
  const toast = useToast();
  const confirm = useConfirm();
  const askText = useAskText();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["tools", "callouts", map], queryFn: () => loadCallouts(map), retry: false, refetchOnWindowFocus: false });
  const nav: CalloutNav | undefined = q.data;

  // the names as loaded, and the edits on top (with undo)
  const original = useMemo<Names>(() => {
    const o: Names = {};
    for (const a of nav?.areas ?? []) o[a[0]] = a[6] ? nav!.places[a[6] - 1] ?? "" : "";
    return o;
  }, [nav]);
  const [history, setHistory] = useState<{ names: Names; what: string }[]>([{ names: {}, what: "" }]);
  const [at, setAt] = useState(0);
  const edits = history[at].names;
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [flash, setFlash] = useState<Set<number>>(new Set());
  const [tool, setTool] = useState<Tool>("pick");
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [suggesting, setSuggesting] = useState<string | null>(null);
  const [stamp, setStamp] = useState<string | null>(null);
  const [hover, setHover] = useState<Area | null>(null);

  useEffect(() => {
    setHistory([{ names: {}, what: "" }]);
    setAt(0);
    setSelected(new Set());
    setStamp(nav?.stamp ?? null);
  }, [nav]);

  const areas = useMemo<Area[]>(
    () => (nav?.areas ?? []).map(([id, x0, y0, x1, y1, z]) => ({ id, x0, y0, x1, y1, z, place: edits[id] ?? original[id] ?? "" })),
    [nav, edits, original],
  );
  const zLimits = useMemo<[number, number]>(() => {
    if (!areas.length) return [0, 0];
    let [lo, hi] = [Infinity, -Infinity];
    for (const a of areas) {
      lo = Math.min(lo, a.z);
      hi = Math.max(hi, a.z);
    }
    return [Math.floor(lo), Math.ceil(hi)];
  }, [nav]); // eslint-disable-line react-hooks/exhaustive-deps
  const [zRange, setZRange] = useState<[number, number]>([-99999, 99999]);
  useEffect(() => setZRange(zLimits), [zLimits]);

  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const a of areas) if (a.place) m.set(a.place, (m.get(a.place) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [areas]);
  const trimmed = name.trim();
  const nameOk = validPlace(trimmed);
  // the brush's colour: its own if the place exists, else the one it would get
  const colours = useMemo(() => placeColours([...counts.map((c) => c[0]), ...(nameOk ? [trimmed] : [])]), [counts, nameOk, trimmed]);
  const voice = useMemo(() => new Set(nav?.voice ?? []), [nav]);
  const changes: Change[] = useMemo(() => Object.entries(edits).filter(([id, p]) => (original[+id] ?? "") !== p).map(([id, p]) => ({ id: +id, place: p })), [edits, original]);
  // (asked about before leaving: the crumbs, Back, another section)
  useDirty("callouts", changes.length > 0);
  const unnamed = areas.length - counts.reduce((s, c) => s + c[1], 0);

  const push = (next: Names, what: string) => {
    setHistory([...history.slice(0, at + 1), { names: next, what }]);
    setAt(at + 1);
  };
  const setNames = (ids: number[], place: string, what?: string) => {
    const next = { ...edits };
    for (const id of ids) next[id] = place;
    push(next, what ?? (place ? `name ${ids.length} area${ids.length === 1 ? "" : "s"} ${place}` : `take the name off ${ids.length} area${ids.length === 1 ? "" : "s"}`));
  };
  const undo = () => at > 0 && setAt(at - 1);
  const redo = () => at < history.length - 1 && setAt(at + 1);

  // keys: V/P/E tools, Escape clears the pick, Cmd/Ctrl+Z undoes, with Shift redoes
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName) || document.querySelector(".sheet-backdrop")) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "Escape") {
        setSelected(new Set());
        setFlash(new Set());
      } else if (e.code === "KeyV") setTool("pick");
      else if (e.code === "KeyP") setTool("paint");
      else if (e.code === "KeyE") setTool("erase");
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const picked = [...selected].map((i) => areas[i]).filter(Boolean);
  const applyName = () => {
    if (!nameOk || !picked.length) return;
    setNames(picked.map((a) => a.id), trimmed);
  };
  const clearName = () => picked.length && setNames(picked.map((a) => a.id), "");
  const pickPlace = (place: string, add: boolean) => {
    const next = add ? new Set(selected) : new Set<number>();
    areas.forEach((a, i) => a.place === place && next.add(i));
    setSelected(next);
    setFlash(new Set());
    setName(place);
  };
  const showAreas = (ids: number[]) => {
    const want = new Set(ids);
    const s = new Set<number>();
    areas.forEach((a, i) => want.has(a.id) && s.add(i));
    setFlash(s);
    setSelected(s);
  };
  const rename = async (place: string) => {
    const to = await askText(`Rename ${place} on every area that has it`, { label: "New name", placeholder: place, max: 31 }, { ok: "Rename" });
    if (to === null || !to || to === place) return;
    if (!validPlace(to)) return toast("Use letters and numbers (and _), starting with a letter, up to 31.", true);
    setNames(areas.filter((a) => a.place === place).map((a) => a.id), to, `rename ${place} to ${to}`);
  };
  const onPaint = (list: number[]) => {
    if (tool === "erase") return setNames(list.map((i) => areas[i].id).filter((id, k) => areas[list[k]].place !== ""), "");
    if (!nameOk) return toast("Type or pick a name first: that's what the brush paints.", true);
    const ids = list.filter((i) => areas[i].place !== trimmed).map((i) => areas[i].id);
    if (ids.length) setNames(ids, trimmed);
  };

  const fetchSuggest = async (from: "usual" | "other" | "marks") => {
    setSuggesting(from);
    const r = await suggest(map, from);
    setSuggesting(null);
    if (r.error) return toast(r.error, true);
    // (the server works from the saved names: a name given here since then stays as it is)
    const mine = (id: number) => edits[id] !== undefined && edits[id] !== (original[id] ?? "");
    const c = (r.changes ?? []).filter((x) => !mine(x.id));
    const kept = (r.changes ?? []).length - c.length;
    if (c.length) {
      const next = { ...edits };
      for (const x of c) next[x.id] = x.place;
      push(next, from === "marks" ? "names from bomb sites and spawns" : from === "usual" ? "the usual callouts" : "names from the other game's nav");
      showAreas(c.map((x) => x.id));
    }
    toast((r.message || "Done.") + (kept ? ` ${kept} area${kept === 1 ? "" : "s"} you named yourself kept ${kept === 1 ? "its name" : "their names"}.` : ""));
  };

  const save = async () => {
    if (!stamp || !changes.length) return;
    setSaving(true);
    const r = await saveCallouts(map, stamp, changes);
    setSaving(false);
    if (r.error || !r.ok) return toast(r.error || "Couldn't save the names.", true);
    toast(r.message || "Saved.");
    await qc.invalidateQueries({ queryKey: ["tools", "callouts", map] });
  };

  const back = async () => {
    if (changes.length && !(await confirm(`You have ${changes.length} unsaved name change${changes.length === 1 ? "" : "s"}. Leave without saving?`, { ok: "Leave", danger: true }))) return;
    onBack();
  };

  // ---- checks: two spellings of one place, a place in pieces far apart, one-area places, unnamed areas
  const issues = useMemo<Issue[]>(() => {
    const out: Issue[] = [];
    const byNorm = new Map<string, string[]>();
    for (const [p] of counts) byNorm.set(norm(p), [...(byNorm.get(norm(p)) ?? []), p]);
    const count = new Map(counts);
    for (const names of byNorm.values()) {
      if (names.length < 2) continue;
      const keep = [...names].sort((a, b) => (count.get(b) ?? 0) - (count.get(a) ?? 0))[0];
      const others = names.filter((n) => n !== keep);
      out.push({
        key: `spell-${keep}`,
        tone: "bad",
        text: `${names.join(" and ")} look like one place spelled ${names.length === 2 ? "two" : "different"} ways`,
        areas: areas.filter((a) => names.includes(a.place)).map((a) => a.id),
        fix: { label: `Use ${keep}`, run: () => setNames(areas.filter((a) => others.includes(a.place)).map((a) => a.id), keep, `use ${keep} for ${others.join(", ")}`) },
      });
    }
    for (const [p, n] of counts) {
      const mine = areas.filter((a) => a.place === p);
      const parts = pieces(mine);
      if (parts.length > 1) out.push({ key: `split-${p}`, tone: "warn", text: `${p} is in ${parts.length} spots far apart`, areas: mine.map((a) => a.id) });
      else if (n === 1 && mine[0] && (mine[0].x1 - mine[0].x0) * (mine[0].y1 - mine[0].y0) < 128 * 128) out.push({ key: `one-${p}`, tone: "info", text: `${p} names only one small area`, areas: mine.map((a) => a.id) });
    }
    if (unnamed && counts.length) out.push({ key: "unnamed", tone: "info", text: `${unnamed} area${unnamed === 1 ? "" : "s"} without a name`, areas: areas.filter((a) => !a.place).map((a) => a.id) });
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [areas, counts, unnamed]);

  const title = (
    <>
      <span className="map-title">{map}</span>
      {nav?.file && <span className="muted small-note">{nav.file}</span>}
      <button className="btn small" onClick={back}>Other map</button>
    </>
  );

  if (q.isPending)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <div className="tools-stage co-empty"><Loading text={`Reading ${map}'s nav…`} /></div>
      </div>
    );
  if (q.isError || !nav)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <LoadError title={`Couldn't open ${map}'s nav`} text={(q.error as Error)?.message || "Couldn't load the nav."} onRetry={() => q.refetch()} />
      </div>
    );

  const locked = nav.inUse;
  const brushColour = nameOk ? colours.get(trimmed) ?? null : null;
  const known = [...new Set([...nav.voice, ...nav.chat])].filter((n) => !counts.some((c) => c[0] === n));
  const floorsCut = zRange[0] !== zLimits[0] || zRange[1] !== zLimits[1];

  return (
    <div className="tools-page co-page">
      <div className="tools-bar">
        {title}
        <span className="push" />
        <Segmented label="What dragging does" values="pick paint erase" labels="Pick|Paint|Erase" current={tool} onPick={(v) => setTool(v as Tool)} />
        <button className="btn" onClick={undo} disabled={at === 0} title={at ? `Undo ${history[at].what} (Mod+Z)` : "Nothing to undo"}>Undo</button>
        <button className="btn" onClick={redo} disabled={at >= history.length - 1} title={at < history.length - 1 ? `Redo ${history[at + 1].what}` : "Nothing to redo"}>Redo</button>
      </div>
      {locked && <Banner>The game has {map} loaded. You can name places, but saving waits until you change map (or close the game).</Banner>}
      {nav.matchesMap === false && <Banner>This nav was made for another version of the map. Names still work; the map doctor can teach bots the map again.</Banner>}

      <div className="tools-viewer">
        <div className="tools-stage">
          <div className={`tools-canvas co-stage co-${tool}`}>
            {areas.length ? (
              <TopDown areas={areas} colours={colours} selected={selected} onSelect={(s) => (setSelected(s), setFlash(new Set()))} zRange={zRange} fitKey={map + (nav.areas.length || 0)} tool={tool} brush={brushColour} onPaint={onPaint} flash={flash} onHoverArea={setHover} />
            ) : (
              <div className="tools-overlay"><Empty plain title="The nav has no areas" text="There's nothing to name. Teach bots the map again in the map doctor." /></div>
            )}
            {tool === "paint" && (
              <div className="co-brush" aria-live="polite">
                {nameOk ? <><span className="co-swatch" style={{ background: brushColour ?? undefined }} /> Painting <strong>{trimmed}</strong></> : "Pick a name to paint with"}
              </div>
            )}
          </div>
          <div className="tools-status">
            <span className="mono">{areas.length} areas · {counts.length} places</span>
            <span className="tools-hover">{hover ? <>{hover.place || <span className="muted">no name</span>}<span className="muted"> · area {hover.id} · height {Math.round(hover.z)}</span></> : picked.length ? `${picked.length} picked` : ""}</span>
            <span className="tools-hint">{TOOL_HINT[tool]}</span>
          </div>
        </div>

        <aside className="tools-side">
          <section className="tools-panel">
            <h3>{tool === "pick" ? (picked.length ? `Name ${picked.length} picked area${picked.length === 1 ? "" : "s"}` : "Name") : tool === "paint" ? "Brush" : "Erase"}</h3>
            <form
              className="co-name"
              onSubmit={(e) => {
                e.preventDefault();
                applyName();
              }}
            >
              <input className="search" list="co-names" placeholder="Place name, e.g. Catwalk" aria-label="Place name" maxLength={31} value={name} onChange={(e) => setName(e.target.value.replace(/\s+/g, ""))} />
              <datalist id="co-names">
                {[...new Set([...counts.map((c) => c[0]), ...nav.voice, ...nav.chat])].map((n) => (
                  <option key={n} value={n}>{voice.has(n) ? "bots say it on the radio" : "in chat"}</option>
                ))}
              </datalist>
              {tool === "pick" && (
                <div className="row">
                  <button className="btn primary small" type="submit" disabled={!picked.length || !nameOk}>Name {picked.length > 1 ? "them" : "it"}</button>
                  <button className="btn small" type="button" disabled={!picked.length} onClick={clearName}>Remove the name</button>
                </div>
              )}
            </form>
            {trimmed && !nameOk && <p className="co-bad small-note">Letters and numbers (and _), starting with a letter, up to 31.</p>}
            {nameOk && <p className="muted small-note">{voice.has(trimmed) ? "🔊 Said on the radio" : "In chat only"}: “{spoken(trimmed)}”</p>}
            {known.length > 0 && (
              <details className="co-known">
                <summary>Names bots know</summary>
                <div className="chips">
                  {known.slice(0, 80).map((n) => (
                    <button key={n} className="chip" aria-pressed={trimmed === n} title={voice.has(n) ? "Bots say it on the radio" : "Bot chat knows it"} onClick={() => setName(n)}>{n}</button>
                  ))}
                </div>
              </details>
            )}
          </section>

          <section className="tools-panel">
            <h3>
              Places <span className="muted small-note">{counts.length}</span>
              <Info text="Click a place to pick its areas (Shift adds) and make it the brush. Names with 🔊 have a radio voice line; any other name works in bot chat and voice commands (capitals and _ become spaces: UpperTunnel is 'upper tunnel'). Saving adds new names to the game's place list (BotChatter.db), which it needs to keep them." />
            </h3>
            {counts.length === 0 ? (
              <Empty plain text={nav.usual ? "No places yet. Paint some, or start from a suggestion below." : "No places yet. Pick areas and name them, or paint a name on (P)."} />
            ) : (
              <ul className="co-places">
                {counts.map(([p, n]) => (
                  <li key={p}>
                    <button className="co-place" aria-pressed={trimmed === p} onClick={(e) => pickPlace(p, e.shiftKey)} title="Pick its areas and paint with it (Shift adds)">
                      <span className="co-swatch" style={{ background: colours.get(p) }} />
                      <span className="co-place-name">{p}</span>
                      {voice.has(p) && <span className="co-voice" role="img" aria-label="has a radio voice line" title="Bots say it on the radio">🔊</span>}
                      <span className="muted small-note push">{n}</span>
                    </button>
                    <button className="btn small" onClick={() => rename(p)} aria-label={`Rename ${p}`} title={`Rename ${p}`}>Rename</button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="tools-panel">
            <h3>Suggestions <Info text="Suggestions only fill in names on the map; nothing is saved until you save, and Undo takes them back." /></h3>
            <div className="co-suggest">
              <BusyButton className="btn small" busy={suggesting === "marks"} label="From bomb sites and spawns" busyLabel="Working…" disabled={suggesting !== null} onClick={() => fetchSuggest("marks")} />
              {nav.usual && <BusyButton className="btn small" busy={suggesting === "usual"} label="The usual callouts" busyLabel="Working…" disabled={suggesting !== null} onClick={() => fetchSuggest("usual")} />}
              {nav.other && <BusyButton className="btn small" busy={suggesting === "other"} label={`From ${nav.other === "czero" ? "Condition Zero's" : "CS 1.6's"} nav`} busyLabel="Working…" disabled={suggesting !== null} onClick={() => fetchSuggest("other")} />}
            </div>
          </section>

          {issues.length > 0 && (
            <section className="tools-panel">
              <h3>Checks <span className="muted small-note">{issues.filter((i) => i.tone !== "info").length || ""}</span></h3>
              <ul className="co-issues">
                {issues.map((i) => (
                  <li key={i.key} className={`co-issue ${i.tone}`}>
                    <span>{i.text}</span>
                    <div className="row">
                      <button className="btn small" onClick={() => showAreas(i.areas)}>Show me</button>
                      {i.fix && <button className="btn small" onClick={i.fix.run}>{i.fix.label}</button>}
                    </div>
                  </li>
                ))}
              </ul>
            </section>
          )}

          <section className="tools-panel">
            <h3>
              Floors <Info text="Show only the areas between two heights, to name one floor of a building at a time. The others are drawn faint and can't be picked." />
              {floorsCut && <button className="btn small co-all" onClick={() => setZRange(zLimits)}>All</button>}
            </h3>
            <label className="co-range"><span className="small-note">From</span><input type="range" min={zLimits[0]} max={zLimits[1]} value={zRange[0]} aria-label="Lowest floor shown" onChange={(e) => setZRange([Math.min(+e.target.value, zRange[1]), zRange[1]])} /><span className="co-num">{zRange[0]}</span></label>
            <label className="co-range"><span className="small-note">To</span><input type="range" min={zLimits[0]} max={zLimits[1]} value={zRange[1]} aria-label="Highest floor shown" onChange={(e) => setZRange([zRange[0], Math.max(+e.target.value, zRange[0])])} /><span className="co-num">{zRange[1]}</span></label>
          </section>

          <p className="small-note muted co-foot">
            {nav.knowledge === false ? "The bots' map knowledge is for another nav; they work it out again when they next play the map. " : "Names don't move areas, so the bots' map knowledge still fits. "}
            <button className="linkish" onClick={() => useNav.getState().go("tools", `doctor:${map}`)}>Check the map in the map doctor</button>
          </p>
        </aside>
      </div>
      <ActionBar
        dirty={changes.length > 0}
        working={saving ? "Saving…" : null}
        where={changes.length ? `${changes.length} area${changes.length === 1 ? "" : "s"} renamed · ${nav.file}` : nav.file}
        info={`The old nav is kept as ${nav.file}.bak the first time, and every save is in History. Names don't move areas, so the bots' map knowledge still fits.`}
        save={{ label: "Save names", onClick: save, disabled: !changes.length || locked, title: locked ? "The game has this map loaded" : undefined }}
        undo={() => push({}, "put every name back")}
      />
    </div>
  );
}

/** A place's areas in spots: areas within 600 units of each other (and on about the same floor) are one spot. */
function pieces(list: Area[]): Area[][] {
  const near = (a: Area, b: Area) => a.x0 - 600 <= b.x1 && b.x0 - 600 <= a.x1 && a.y0 - 600 <= b.y1 && b.y0 - 600 <= a.y1 && Math.abs(a.z - b.z) < 200;
  const group = new Array<number>(list.length).fill(-1);
  const groups: Area[][] = [];
  for (let i = 0; i < list.length; i++) {
    if (group[i] >= 0) continue;
    const g: Area[] = [];
    group[i] = groups.length;
    const todo = [i];
    while (todo.length) {
      const k = todo.pop()!;
      g.push(list[k]);
      for (let j = 0; j < list.length; j++) if (group[j] < 0 && near(list[k], list[j])) (group[j] = groups.length), todo.push(j);
    }
    groups.push(g);
  }
  return groups;
}
