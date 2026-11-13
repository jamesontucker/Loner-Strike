// The callout editor: name places on a map's bot nav ("Catwalk", "Apartments"), so bots say
// where they are on the radio and bot chat and voice commands understand the names.
// Pick areas on the top-down map, give them a name, save.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNav } from "../../../store";
import { useToolArg } from "../../../stores/tools";
import { Banner, BusyButton, Info, LoadError, Skeleton, useAskText, useConfirm, useToast } from "../../../ui";
import { MapList } from "../common";
import type { ToolProps } from "../registry";
import { loadCallouts, placeColours, saveCallouts, spoken, suggest, validPlace, type Area, type CalloutNav, type Change } from "./calloutsApi";
import { TopDown } from "./TopDown";
import "./callouts.css";

export default function Callouts({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  if (!map)
    return (
      <div className="stack-tight">
        <p className="muted measure">
          Pick a map to name its places. Bots say these names on the radio ("enemy spotted at Long") and bot chat and voice commands understand them. Only maps with a bot nav are listed: the map doctor can teach bots the others.
        </p>
        <MapList maps={maps} onPick={setMap} placeholder="Pick a map to name its places" filter={(m) => m.nav} none="None of your maps has a bot nav yet. The map doctor can teach bots a map; then you can name its places here." />
      </div>
    );
  return <Editor map={map} onBack={() => setMap(null)} />;
}

/** names by area id */
type Names = Record<number, string>;

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
  const [history, setHistory] = useState<Names[]>([{}]);
  const [at, setAt] = useState(0);
  const edits = history[at];
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [suggesting, setSuggesting] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [stamp, setStamp] = useState<string | null>(null);

  useEffect(() => {
    setHistory([{}]);
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
  const colours = useMemo(() => placeColours(counts.map((c) => c[0])), [counts]);
  const voice = useMemo(() => new Set(nav?.voice ?? []), [nav]);
  const changes: Change[] = useMemo(() => Object.entries(edits).filter(([id, p]) => (original[+id] ?? "") !== p).map(([id, p]) => ({ id: +id, place: p })), [edits, original]);
  const unnamed = areas.length - counts.reduce((s, c) => s + c[1], 0);

  const push = (next: Names) => {
    setHistory([...history.slice(0, at + 1), next]);
    setAt(at + 1);
    setSaved(null);
  };
  const setNames = (ids: number[], place: string) => {
    const next = { ...edits };
    for (const id of ids) next[id] = place;
    push(next);
  };
  const undo = () => at > 0 && setAt(at - 1);
  const redo = () => at < history.length - 1 && setAt(at + 1);

  // keys: Escape clears the pick, Cmd/Ctrl+Z undoes, with Shift redoes
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.key === "Escape") setSelected(new Set());
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const picked = [...selected].map((i) => areas[i]).filter(Boolean);
  const trimmed = name.trim();
  const nameOk = validPlace(trimmed);
  const applyName = () => {
    if (!nameOk || !picked.length) return;
    setNames(picked.map((a) => a.id), trimmed);
  };
  const clearName = () => picked.length && setNames(picked.map((a) => a.id), "");
  const pickPlace = (place: string, add: boolean) => {
    const next = add ? new Set(selected) : new Set<number>();
    areas.forEach((a, i) => a.place === place && next.add(i));
    setSelected(next);
    setName(place);
  };
  const rename = async (place: string) => {
    const to = await askText(`Rename ${place} on every area that has it`, { label: "New name", placeholder: place, max: 31 }, { ok: "Rename" });
    if (to === null || !to || to === place) return;
    if (!validPlace(to)) return toast("Use letters and numbers (and _), starting with a letter, up to 31.", true);
    setNames(areas.filter((a) => a.place === place).map((a) => a.id), to);
  };

  const fetchSuggest = async (from: "usual" | "other") => {
    setSuggesting(from);
    const r = await suggest(map, from);
    setSuggesting(null);
    if (r.error) return toast(r.error, true);
    const c = r.changes ?? [];
    if (c.length) {
      const next = { ...edits };
      for (const x of c) next[x.id] = x.place;
      push(next);
    }
    toast(r.message || "Done.");
  };

  const save = async () => {
    if (!stamp || !changes.length) return;
    setSaving(true);
    const r = await saveCallouts(map, stamp, changes);
    setSaving(false);
    if (r.error || !r.ok) return toast(r.error || "Couldn't save the names.", true);
    setSaved(r.message || "Saved.");
    toast(r.message || "Saved.");
    await qc.invalidateQueries({ queryKey: ["tools", "callouts", map] });
  };

  const back = async () => {
    if (changes.length && !(await confirm(`You have ${changes.length} unsaved name change${changes.length === 1 ? "" : "s"}. Leave without saving?`, { ok: "Leave", danger: true }))) return;
    onBack();
  };

  const head = (
    <div className="row between co-head">
      <div>
        <h3 className="co-title">Places on {map}</h3>
        {nav && <span className="muted small-note">{nav.file} · {areas.length} areas · {counts.length} places{unnamed ? ` · ${unnamed} areas without a name` : ""}</span>}
      </div>
      <div className="row">
        <button className="btn" onClick={back}>Pick another map</button>
      </div>
    </div>
  );

  if (q.isPending)
    return (
      <div className="panel">
        {head}
        <Skeleton rows={5} />
      </div>
    );
  if (q.isError || !nav)
    return (
      <div className="panel">
        {head}
        <LoadError text={(q.error as Error)?.message || "Couldn't load the nav."} onRetry={() => q.refetch()} />
      </div>
    );

  const locked = nav.inUse;

  return (
    <div className="co stack-tight">
      <div className="panel">
        {head}
        {locked && <Banner>The game has {map} loaded right now. You can name places, but saving waits until you change to another map (or close the game).</Banner>}
        {nav.matchesMap === false && <Banner>This nav was made for a different version of the map. Names still work, but the map doctor can teach bots the map again.</Banner>}
      </div>

      <div className="co-body">
        <div className="panel co-map">
          <TopDown areas={areas} colours={colours} selected={selected} onSelect={setSelected} zRange={zRange} fitKey={map + (nav.areas.length || 0)} />
          <div className="co-floors row">
            <span className="muted small-note">Floors shown</span>
            <label className="co-range">
              <span className="small-note">from</span>
              <input type="range" min={zLimits[0]} max={zLimits[1]} value={zRange[0]} aria-label="Lowest floor shown" onChange={(e) => setZRange([Math.min(+e.target.value, zRange[1]), zRange[1]])} />
            </label>
            <label className="co-range">
              <span className="small-note">to</span>
              <input type="range" min={zLimits[0]} max={zLimits[1]} value={zRange[1]} aria-label="Highest floor shown" onChange={(e) => setZRange([zRange[0], Math.max(+e.target.value, zRange[0])])} />
            </label>
            <span className="muted small-note">{zRange[0]} to {zRange[1]} units</span>
            {(zRange[0] !== zLimits[0] || zRange[1] !== zLimits[1]) && <button className="btn small" onClick={() => setZRange(zLimits)}>All floors</button>}
          </div>
          <p className="muted small-note co-help">
            Click an area to pick it, Shift-click to add more, drag a box to pick many (Alt-drag takes them away). Double-click picks a whole place. Right-drag or Space-drag moves the map, scroll zooms.
          </p>
        </div>

        <div className="co-side stack-tight">
          <div className="panel">
            <h3>{picked.length ? `${picked.length} area${picked.length === 1 ? "" : "s"} picked` : "Pick areas on the map"}</h3>
            {picked.length > 0 && (
              <p className="muted small-note">
                Now: {[...new Set(picked.map((a) => a.place || "no name"))].slice(0, 4).join(", ")}
                {new Set(picked.map((a) => a.place)).size > 4 ? "…" : ""}
              </p>
            )}
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
              <div className="row">
                <button className="btn primary" type="submit" disabled={!picked.length || !nameOk}>Name {picked.length > 1 ? "them" : "it"}</button>
                <button className="btn" type="button" disabled={!picked.length} onClick={clearName}>Remove the name</button>
              </div>
            </form>
            {trimmed && !nameOk && <p className="co-bad small-note">Use letters and numbers (and _), starting with a letter, up to 31.</p>}
            {nameOk && (
              <p className="muted small-note">
                {voice.has(trimmed) ? "Bots say this one on the radio, and write it in chat" : "Bots write this in chat (no radio voice for it)"}: "{spoken(trimmed)}".
              </p>
            )}
          </div>

          <div className="panel">
            <div className="row between">
              <h3>Places</h3>
              <Info text="Names from the game's list (marked with a speaker) have a radio voice line. Any other name works in bot chat and voice commands: capitals and _ become spaces, so UpperTunnel is 'upper tunnel'." />
            </div>
            {counts.length === 0 ? (
              <p className="muted small-note">No places named yet.</p>
            ) : (
              <ul className="co-places">
                {counts.map(([p, n]) => (
                  <li key={p}>
                    <button className="co-place" onClick={(e) => pickPlace(p, e.shiftKey)} title="Pick its areas (Shift adds)">
                      <span className="co-swatch" style={{ background: colours.get(p) }} />
                      <span className="co-place-name">{p}</span>
                      {voice.has(p) && <span className="co-voice" role="img" aria-label="has a radio voice line" title="Bots say it on the radio">🔊</span>}
                      <span className="muted small-note push">{n}</span>
                    </button>
                    <button className="btn small" onClick={() => rename(p)} aria-label={`Rename ${p}`}>Rename</button>
                  </li>
                ))}
              </ul>
            )}
            {(nav.usual || nav.other) && (
              <div className="co-suggest stack-tight">
                {nav.usual && (
                  <BusyButton className="btn" busy={suggesting === "usual"} label="Use the usual callouts" busyLabel="Working…" disabled={suggesting !== null} onClick={() => fetchSuggest("usual")} />
                )}
                {nav.other && (
                  <BusyButton
                    className="btn"
                    busy={suggesting === "other"}
                    label={`Fill gaps from ${nav.other === "czero" ? "Condition Zero's" : "CS 1.6's"} nav`}
                    busyLabel="Working…"
                    disabled={suggesting !== null}
                    onClick={() => fetchSuggest("other")}
                  />
                )}
                <p className="muted small-note">Suggestions show on the map first; nothing is saved until you save.</p>
              </div>
            )}
          </div>

          <div className="panel">
            <h3>Save</h3>
            <p className="muted small-note">
              {changes.length ? `${changes.length} area${changes.length === 1 ? "" : "s"} changed, not saved yet.` : "No changes."}{" "}
              {nav.knowledge === true && "Names don't move the areas, so the bots' map knowledge still fits after saving."}
              {nav.knowledge === false && "The bots' map knowledge was made for a different nav; they work it out again when they next play the map."}
            </p>
            <div className="row">
              <button className="btn" onClick={undo} disabled={at === 0}>Undo</button>
              <button className="btn" onClick={redo} disabled={at >= history.length - 1}>Redo</button>
              <button className="btn" disabled={!changes.length} onClick={() => push({})}>Put back</button>
            </div>
            <div className="row co-save">
              <BusyButton className="btn primary" busy={saving} label="Save names" busyLabel="Saving…" disabled={!changes.length || locked} onClick={save} />
            </div>
            {saved && <p className="co-saved small-note">{saved}</p>}
            <p className="muted small-note">
              The old nav is kept as {nav.file}.bak the first time{nav.backup ? " (there's one already)" : ""}. <button className="linkish" onClick={() => useNav.getState().go("tools", `doctor:${map}`)}>Check the map in the map doctor</button>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
