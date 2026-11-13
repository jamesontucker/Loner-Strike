// Nav fixer (Beta): fix a map's bot nav by hand, the way cs_rats2's beams were fixed. Draw the
// areas the learner missed, link them (both ways, or one way for a drop), mark how bots move
// on them, name them; the problems list says what's still wrong. Save, then rebuild the bots'
// map knowledge, which is made for one nav.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useDirty } from "../../../store";
import { useTools, useToolArg, type CameraMode } from "../../../stores/tools";
import { Banner, BusyButton, Info, LoadError, Segmented, Skeleton, useAskText, useConfirm, useToast } from "../../../ui";
import { JobProgress } from "../../JobProgress";
import { Check, Loading, MapList } from "../common";
import type { NavArea } from "../overlays";
import type { ToolProps } from "../registry";
import { useReportAddress } from "../common/address";
import { useMesh, useEntities } from "../toolsApi";
import { fmtVec } from "../viewer/coords";
import { canDraw3d, SceneBoundary } from "../viewer/Scene";
import { useModelMeshes } from "../viewer/World";
import { FixScene, type ColourBy, type Mode } from "./FixScene";
import { ATTRS, DIRS, editNav, openFix, rebuildKnowledge, redoNav, reloadNav, saveNav, undoNav, REACH_FALLS, REACH_NEVER, REACH_TRAP, type FixState, type Op, type Problem } from "./navfixApi";
import "./navfix.css";

export default function NavFixer({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("navfix", map ? `navfix:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  if (!map)
    return (
      <div className="stack-tight">
        <p className="muted measure">
          Pick a map to fix its bot nav: add the walkable spots the bots' learning missed (narrow beams, ledges), join them up, and mark drops. Only maps with a bot nav are listed.
        </p>
        <MapList maps={maps} onPick={setMap} placeholder="Pick a map to fix its nav" filter={(m) => m.nav} none="None of your maps has a bot nav yet. The map doctor can teach bots a map; then it shows up here." />
      </div>
    );
  return <Fixer map={map} onBack={() => setMap(null)} />;
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

const HINTS: Record<Mode, string> = {
  select: "Click an area to pick it · Shift-click adds more · Delete removes them",
  draw: "Drag on the floor to draw an area · Right-drag moves the view · Scroll zooms",
  link: "Click an area, then the one it leads to · Shift-click for a one-way drop · Esc stops",
};

function Fixer({ map, onBack }: { map: string; onBack: () => void }) {
  const qc = useQueryClient();
  const toast = useToast();
  const confirm = useConfirm();
  const webgl = useMemo(canDraw3d, []);
  const mesh = useMesh(webgl ? map : null);
  const ents = useEntities(map, mesh.isSuccess);
  const meshes = useModelMeshes(mesh.data ?? null, ents.data ?? []);
  const key = ["tools", "navfix", map];
  const q = useQuery({ queryKey: key, queryFn: ({ signal }) => openFix(map, signal), retry: false, refetchOnWindowFocus: false, staleTime: Infinity, gcTime: 0, structuralSharing: false });
  const st = q.data;

  const camera = useTools((s) => s.camera);
  const setCamera = useTools((s) => s.setCamera);
  const [mode, setMode] = useState<Mode>("select");
  const [colourBy, setColourBy] = useState<ColourBy>("reach");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [linkFrom, setLinkFrom] = useState<number | null>(null);
  const [oneWay, setOneWay] = useState(false);
  const [shown, setShown] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState<string | null>(null);
  const [lost, setLost] = useState(false);
  const [n, setN] = useState(0);
  const [rebuilding, setRebuilding] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  useDirty("navfix", !!st?.dirty);
  // (flying grabs the mouse: this tool draws and picks, so it's Orbit or Top-down)
  useEffect(() => {
    if (camera === "fly") setCamera("orbit");
  }, [camera, setCamera]);

  // areas that went (an undo, a delete) leave the selection
  useEffect(() => {
    if (!st) return;
    setSelected((s) => {
      const keep = new Set([...s].filter((id) => st.nav.index.has(id)));
      return keep.size === s.size ? s : keep;
    });
    if (linkFrom !== null && !st.nav.index.has(linkFrom)) setLinkFrom(null);
  }, [st, linkFrom]);

  /** Runs a request that answers with the new state; one at a time. */
  const act = useCallback(
    async (label: string, fn: () => Promise<FixState>, after?: (s: FixState) => void) => {
      if (busy) return;
      setBusy(label);
      try {
        const s = await fn();
        qc.setQueryData(key, s);
        if (s.notes?.length) toast(s.notes.join(" "));
        after?.(s);
      } catch (e) {
        toast(e instanceof Error ? e.message : String(e), true);
      } finally {
        setBusy(null);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [busy, map, qc, toast],
  );
  const edit = useCallback((label: string, op: Op, after?: (s: FixState) => void) => act(label, () => editNav(map, op), after), [act, map]);
  const undo = () => st?.undo && act("Undoing…", () => undoNav(map));
  const redo = () => st?.redo && act("Redoing…", () => redoNav(map));
  const save = () =>
    act("Saving…", () => saveNav(map), (s) => {
      setSaid(s.message ?? "Saved.");
      // the map viewer's nav layer reads the file again
      void qc.invalidateQueries({ queryKey: ["tools", "nav", map] });
      void qc.invalidateQueries({ queryKey: ["tools", "know", map] });
    });
  const startOver = async () => {
    if (st?.dirty && !(await confirm("Throw away your changes and read the nav file again?", { ok: "Start over", danger: true }))) return;
    setSaid(null);
    await act("Reading the nav…", () => reloadNav(map));
  };
  const rebuild = async () => {
    if (busy) return;
    setBusy("Starting…");
    try {
      await rebuildKnowledge(map);
      setRebuilding(true);
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), true);
    } finally {
      setBusy(null);
    }
  };
  const del = (ids: number[]) => ids.length && edit(ids.length === 1 ? "Deleting the area…" : "Deleting the areas…", { op: "delete", ids }, () => setSelected(new Set()));

  const onPick = useCallback(
    (id: number | null, shift: boolean) => {
      if (mode === "link") {
        if (id === null) return;
        if (linkFrom === null || linkFrom === id) return setLinkFrom(id === linkFrom ? null : id);
        const one = shift || oneWay;
        const from = linkFrom;
        void edit(one ? "Making the drop…" : "Linking…", { op: "link", a: from, b: id, oneWay: one }, () => setLinkFrom(id));
        return;
      }
      setShown(new Set());
      if (id === null) return setSelected(new Set());
      setSelected((s) => {
        if (!shift) return new Set([id]);
        const next = new Set(s);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
    },
    [mode, linkFrom, oneWay, edit],
  );
  const onDraw = useCallback(
    (x: [number, number], y: [number, number], z: number) => edit("Adding the area…", { op: "add", x, y, z, snap: true }, (s) => s.newId != null && setSelected(new Set([s.newId]))),
    [edit],
  );

  // keys: undo/redo, delete, Esc, V/D/L for the modes, O/T for the camera
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (isField(e.target) || document.querySelector(".sheet-backdrop")) return;
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) void redo();
        else void undo();
        return;
      }
      if (mod && e.key.toLowerCase() === "y") {
        e.preventDefault();
        void redo();
        return;
      }
      if (mod || e.altKey) return;
      if (e.key === "Escape") {
        setLinkFrom(null);
        setSelected(new Set());
        setShown(new Set());
      } else if ((e.key === "Delete" || e.key === "Backspace") && selected.size && mode === "select") {
        e.preventDefault();
        void del([...selected]);
      } else if (e.code === "KeyV") setMode("select");
      else if (e.code === "KeyD") setMode("draw");
      else if (e.code === "KeyL") setMode("link");
      else if (e.code === "KeyO") setCamera("orbit");
      else if (e.code === "KeyT") setCamera("top");
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  });
  useEffect(() => {
    if (mode !== "link") setLinkFrom(null);
  }, [mode]);

  const showProblem = (p: Problem) => {
    setShown(new Set(p.areas));
    if (p.areas.length) setSelected(new Set(p.areas.length <= 20 ? p.areas : []));
    useTools.getState().look(p.at, p.areas.length > 3 ? 256 : 96);
  };

  const failed = mesh.error instanceof Error ? mesh.error.message : q.error instanceof Error ? q.error.message : null;
  const ready = mesh.data && st && !ents.isLoading;

  return (
    <div className="tools-page">
      <div className="tools-bar">
        <span className="map-title">{map}</span>
        {st?.file && <span className="muted small-note">{st.file}</span>}
        <button className="btn small" onClick={async () => (!st?.dirty || (await confirm("Leave without saving your changes to the nav?", { ok: "Leave", danger: true }))) && onBack()}>
          Other map
        </button>
        <span className="push" />
        <Segmented values="select draw link" labels="Select|Draw area|Link" current={mode} onPick={(v) => setMode(v as Mode)} label="What clicks do" />
        <Segmented values="orbit top" labels="Orbit|Top-down" current={camera === "fly" ? "orbit" : camera} onPick={(v) => setCamera(v as CameraMode)} label="Camera" />
      </div>
      {st?.nav.warning && <Banner>{st.nav.warning}</Banner>}
      {st?.nav.matchesMap === false && <Banner>This nav was made for a different version of the map. Fixing it is fine, but the game may learn a new one when it loads the map.</Banner>}
      {!webgl ? (
        <LoadError title="Your system can't show 3D" text="Loner-Strike couldn't start 3D drawing on this computer. Updating your system or graphics drivers usually fixes this." />
      ) : failed ? (
        <LoadError title={`Couldn't open ${map}'s nav`} text={failed} onRetry={() => (mesh.error ? void mesh.refetch() : void q.refetch())} />
      ) : (
        <div className="tools-viewer">
          <div className="tools-stage">
            <div className={`tools-canvas navfix-canvas navfix-${mode}`}>
              {!ready ? (
                <div className="tools-overlay">
                  <Loading text={mesh.data ? "Reading the nav…" : "Reading the map…"} />
                </div>
              ) : lost ? (
                <div className="tools-overlay">
                  <LoadError title="The 3D view stopped" text="Your computer's graphics stopped drawing the map." onRetry={() => (setLost(false), setN((v) => v + 1))} />
                </div>
              ) : (
                <SceneBoundary key={n} fallback={<div className="tools-overlay"><LoadError title="The 3D view stopped" text="Your computer's graphics stopped drawing the map." onRetry={() => setN((v) => v + 1)} /></div>}>
                  <FixScene
                    bundle={mesh.data!}
                    meshes={meshes}
                    map={map}
                    nav={st!.nav}
                    reach={st!.reach}
                    colourBy={colourBy}
                    mode={mode}
                    selected={selected}
                    linkFrom={linkFrom}
                    shown={shown}
                    busy={!!busy}
                    onPick={onPick}
                    onDraw={onDraw}
                    onLost={() => setLost(true)}
                  />
                </SceneBoundary>
              )}
              {busy && (
                <div className="navfix-busy" role="status">
                  <span className="spinner" aria-hidden="true" />
                  {busy}
                </div>
              )}
            </div>
            <Status mode={mode} linkFrom={linkFrom} oneWay={oneWay} />
          </div>
          <aside className="tools-side">
            {!st ? (
              <section className="tools-panel"><Skeleton rows={4} /></section>
            ) : (
              <>
                <section className="tools-panel">
                  <h3>Changes</h3>
                  <p className="small-note muted">
                    {st.dirty ? `Not saved yet${st.steps ? ` · ${st.steps} change${st.steps === 1 ? "" : "s"} you can undo` : ""}.` : st.steps ? "Saved." : "No changes yet."}
                    {st.did ? ` Last: ${st.did}.` : ""}
                  </p>
                  <div className="row">
                    <button className="btn small" disabled={!st.undo || !!busy} title={st.undo ? `Undo: ${st.undo} (Cmd/Ctrl+Z)` : "Nothing to undo"} onClick={undo}>Undo</button>
                    <button className="btn small" disabled={!st.redo || !!busy} title={st.redo ? `Redo: ${st.redo} (Shift+Cmd/Ctrl+Z)` : "Nothing to redo"} onClick={redo}>Redo</button>
                    <span className="push" />
                    <button className="btn small" disabled={!!busy} onClick={startOver}>Start over</button>
                  </div>
                  <BusyButton className="btn primary" busy={busy === "Saving…"} busyLabel="Saving…" label="Save the nav" disabled={!st.dirty || !!busy} onClick={save} />
                  {said && <p className="small-note">{said}</p>}
                  {!said && st.backup && <p className="small-note muted">The nav you first changed is kept next to it as a .bak file.</p>}
                </section>
                {mode === "link" && (
                  <section className="tools-panel">
                    <h3>Link areas</h3>
                    <p className="small-note muted">
                      {linkFrom === null ? "Click the area to link from." : `From area ${linkFrom}: click the area it leads to. Each link carries on from the last area, so you can click along a beam.`}
                    </p>
                    <Check checked={oneWay} onChange={setOneWay} label="One way" title="A drop bots can't climb back up" />
                    <p className="small-note muted">One way is for a drop bots can't climb back up. Holding Shift while clicking makes a one-way link too.</p>
                  </section>
                )}
                {mode === "draw" && (
                  <section className="tools-panel">
                    <h3>Draw an area</h3>
                    <p className="small-note muted">
                      Drag a box on the floor where players can walk. It's laid at the height you start on and each corner is dropped to the floor. Top-down (T) makes narrow spots easiest. Then link it to its neighbours.
                    </p>
                  </section>
                )}
                <Selection st={st} ids={[...selected]} busy={!!busy} edit={edit} del={del} />
                <Problems st={st} busy={!!busy} onShow={showProblem} edit={edit} />
                <Knowledge st={st} busy={busy} rebuilding={rebuilding} onRebuild={rebuild} onDone={() => (setRebuilding(false), void q.refetch())} />
                <section className="tools-panel">
                  <label className="tools-field">
                    Colours
                    <select value={colourBy} onChange={(e) => setColourBy(e.target.value as ColourBy)}>
                      <option value="reach">Where bots can get to</option>
                      <option value="place">Places</option>
                      <option value="moves">How bots move</option>
                    </select>
                  </label>
                  {colourBy === "reach" && (
                    <p className="small-note muted navfix-key">
                      <i className="k-ok" /> reachable <i className="k-never" /> can't get there <i className="k-trap" /> can't get back <i className="k-falls" /> only by falling in <i className="k-sel" /> picked
                    </p>
                  )}
                </section>
              </>
            )}
          </aside>
        </div>
      )}
    </div>
  );
}

function Status({ mode, linkFrom, oneWay }: { mode: Mode; linkFrom: number | null; oneWay: boolean }) {
  const hover = useTools((s) => s.hover);
  return (
    <div className="tools-status" aria-live="off">
      <span className="mono">{mode === "link" && linkFrom !== null ? `Linking from ${linkFrom}${oneWay ? " (one way)" : ""}` : ""}</span>
      <span className="tools-hover mono">{hover ? <>{hover.label}{hover.point && <span className="muted"> at {fmtVec(hover.point)}</span>}</> : ""}</span>
      <span className="tools-hint">{HINTS[mode]}</span>
    </div>
  );
}

// ---- the picked areas ------------------------------------------------------------------------

type Edit = (label: string, op: Op, after?: (s: FixState) => void) => Promise<void> | void;

function Selection({ st, ids, busy, edit, del }: { st: FixState; ids: number[]; busy: boolean; edit: Edit; del: (ids: number[]) => void }) {
  const askText = useAskText();
  const areas = ids.map((id) => st.nav.areas[st.nav.index.get(id) ?? -1]).filter(Boolean) as NavArea[];
  if (!areas.length)
    return (
      <section className="tools-panel">
        <h3>Areas</h3>
        <p className="small-note muted">{st.nav.areas.length} areas. Click one to change it, or switch to Draw area to add one.</p>
      </section>
    );
  const bits = st.nav.attrBits;
  const one = areas.length === 1 ? areas[0] : null;
  const placeOf = (a: NavArea) => (a.place ? st.nav.places[a.place - 1] ?? "" : "");
  const places = new Set(areas.map(placeOf));
  const place = places.size === 1 ? [...places][0] : null;
  const setPlace = async (v: string) => {
    let name = v;
    if (v === "\u0000new") {
      const typed = await askText("Name for these areas (as bots say it on the radio, e.g. BombsiteA, Long, Catwalk):", { label: "Place name", max: 63, placeholder: "Catwalk" }, { ok: "Name them" });
      if (typed === null || !typed.trim()) return;
      name = typed.trim();
    }
    await edit("Naming…", { op: "place", ids, name });
  };
  return (
    <section className="tools-panel">
      <h3>{one ? `Area ${one.id}` : `${areas.length} areas`}</h3>
      {one && <AreaFacts st={st} a={one} busy={busy} edit={edit} />}
      <h4>How bots move here</h4>
      <div className="navfix-attrs">
        {ATTRS.map((t) => {
          const bit = bits[t.bit];
          const all = areas.every((a) => a.attr & bit);
          const some = areas.some((a) => a.attr & bit);
          return (
            <label key={t.bit} className="navfix-attr" title={t.help}>
              <input
                type="checkbox"
                checked={all}
                ref={(el) => el && (el.indeterminate = some && !all)}
                disabled={busy}
                onChange={() => void edit(all ? "Unmarking…" : "Marking…", all ? { op: "attr", ids, clear: bit } : { op: "attr", ids, set: bit })}
              />
              {t.label}
            </label>
          );
        })}
      </div>
      <label className="tools-field">
        Place
        <select value={place ?? "\u0000mixed"} disabled={busy} onChange={(e) => void setPlace(e.target.value)}>
          {place === null && <option value={"\u0000mixed"}>(different)</option>}
          <option value="">None</option>
          {st.nav.places.map((p) => <option key={p} value={p}>{p}</option>)}
          <option value={"\u0000new"}>New name…</option>
        </select>
      </label>
      <div className="row">
        <button className="btn small danger" disabled={busy} onClick={() => del(ids)}>{one ? "Delete area" : `Delete ${areas.length} areas`}</button>
        <button className="btn small" onClick={() => useTools.getState().look(st.nav.centres[st.nav.index.get(areas[0].id)!], 96)}>Look at</button>
      </div>
    </section>
  );
}

function AreaFacts({ st, a, busy, edit }: { st: FixState; a: NavArea; busy: boolean; edit: Edit }) {
  const i = st.nav.index.get(a.id)!;
  const [box, setBox] = useState(() => boxOf(a));
  useEffect(() => setBox(boxOf(a)), [a]);
  const reach = st.reach[i];
  const oneWay = new Set(st.nav.oneWay.map(([x, y]) => `${x}>${y}`));
  const out = a.conns.flatMap((ids, d) => ids.map((id) => ({ id, d, back: !oneWay.has(`${a.id}>${id}`) })));
  // links into this area that don't go back: drops into it
  const into = st.nav.areas.filter((o) => oneWay.has(`${o.id}>${a.id}`)).map((o) => o.id);
  const num = (k: keyof typeof box) => (
    <input type="number" step={1} value={box[k]} aria-label={k} onChange={(e) => setBox({ ...box, [k]: e.target.value })} />
  );
  const apply = () => {
    const v = Object.fromEntries(Object.entries(box).map(([k, s]) => [k, Number(s)])) as Record<keyof typeof box, number>;
    if (Object.values(v).some((x) => !Number.isFinite(x))) return;
    void edit("Moving the area…", { op: "reshape", id: a.id, x: [v.x0, v.x1], y: [v.y0, v.y1], z: v.z, snap: true });
  };
  const changed = JSON.stringify(box) !== JSON.stringify(boxOf(a));
  return (
    <>
      <p className="small-note muted">
        {Math.round(a.se[0] - a.nw[0])} × {Math.round(a.se[1] - a.nw[1])} units, floor at {Math.round(Math.min(a.nw[2], a.se[2], a.neZ, a.swZ))}
        {a.place ? ` · ${st.nav.places[a.place - 1]}` : ""}
        {reach === REACH_NEVER ? " · bots can't get here" : reach === REACH_TRAP ? " · bots can't get back from here" : reach === REACH_FALLS ? " · bots only get here by falling in (no link leads here)" : ""}
        {a.hiding.length ? ` · ${a.hiding.length} hiding spot${a.hiding.length === 1 ? "" : "s"}` : ""}
      </p>
      <h4>
        Links <Info text="Bots walk from an area to the ones it links to. Both ways is normal; one way is a drop they can't climb back up." />
      </h4>
      {out.length || into.length ? (
        <ul className="tools-links navfix-links">
          {out.map((l) => (
            <li key={`${l.id}-${l.d}`}>
              <span>{l.back ? "↔" : "↓"} {l.id} <span className="muted">({DIRS[l.d]}{l.back ? "" : ", one way"})</span></span>
              <button className="btn small" disabled={busy} title="Remove this link (both ways)" onClick={() => void edit("Unlinking…", { op: "unlink", a: a.id, b: l.id, both: true })}>Unlink</button>
            </li>
          ))}
          {into.map((id) => (
            <li key={`in-${id}`}>
              <span>↓ from {id} <span className="muted">(a drop into this one)</span></span>
              <button className="btn small" disabled={busy} onClick={() => void edit("Unlinking…", { op: "unlink", a: id, b: a.id, both: false })}>Unlink</button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="small-note">No links: bots can't walk on or off it. Use Link to join it to its neighbours.</p>
      )}
      <details className="navfix-box">
        <summary>Position and size</summary>
        <div className="navfix-grid">
          <span>x</span>{num("x0")}{num("x1")}
          <span>y</span>{num("y0")}{num("y1")}
          <span>floor</span>{num("z")}<span />
        </div>
        <div className="row">
          <button className="btn small" disabled={busy || !changed} onClick={apply}>Move it</button>
          <button className="btn small" disabled={busy} title="Drop each corner to the floor below" onClick={() => void edit("Snapping…", { op: "reshape", id: a.id, x: [a.nw[0], a.se[0]], y: [a.nw[1], a.se[1]], snap: true })}>Snap to floor</button>
        </div>
      </details>
    </>
  );
}

function boxOf(a: NavArea) {
  const r = (v: number) => String(Math.round(v * 100) / 100);
  return { x0: r(Math.min(a.nw[0], a.se[0])), x1: r(Math.max(a.nw[0], a.se[0])), y0: r(Math.min(a.nw[1], a.se[1])), y1: r(Math.max(a.nw[1], a.se[1])), z: r(Math.min(a.nw[2], a.se[2], a.neZ, a.swZ)) };
}

// ---- problems --------------------------------------------------------------------------------

function Problems({ st, busy, onShow, edit }: { st: FixState; busy: boolean; onShow: (p: Problem) => void; edit: Edit }) {
  const [all, setAll] = useState(false);
  const list = all ? st.problems : st.problems.slice(0, 12);
  return (
    <section className="tools-panel">
      <h3>
        Problems <Info text="Checked after every change: areas bots can't reach from any spawn (or can't get back from, or only land in by falling off an edge), links to areas that aren't there, spots a standing player doesn't fit, ladders with no area at an end." />
      </h3>
      {st.checkNote && <p className="small-note muted">{st.checkNote}</p>}
      {!st.problems.length ? (
        <p className="small-note">Nothing found. Bots can reach every area from the spawns and get back.</p>
      ) : (
        <ul className="navfix-problems">
          {list.map((p, i) => (
            <li key={`${p.kind}-${p.areas[0] ?? i}-${i}`} className={`navfix-problem navfix-${p.kind}`}>
              <strong>{p.text}</strong>
              <span className="small-note muted">{p.hint}</span>
              <div className="row">
                <button className="btn small" onClick={() => onShow(p)}>Show me</button>
                {p.fix && <button className="btn small" disabled={busy} onClick={() => void edit("Fixing…", p.fix!.op)}>{p.fix.label}</button>}
              </div>
            </li>
          ))}
        </ul>
      )}
      {st.problems.length > 12 && (
        <button className="btn small" onClick={() => setAll(!all)}>{all ? "Show fewer" : `Show all ${st.problems.length}`}</button>
      )}
      {st.moreProblems > 0 && <p className="small-note muted">And {st.moreProblems} more not listed.</p>}
    </section>
  );
}

// ---- the bots' map knowledge -----------------------------------------------------------------

function Knowledge({ st, busy, rebuilding, onRebuild, onDone }: { st: FixState; busy: string | null; rebuilding: boolean; onRebuild: () => void; onDone: () => void }) {
  const k = st.knowledge;
  const status = !k
    ? "The bots haven't studied this map yet. They do it when they play it, or rebuild it here."
    : k.fits && k.done
      ? "Up to date: made for this nav."
      : k.fits
        ? "Half done for this nav (the bots carry on next time they play it)."
        : st.dirty
          ? "Made for the nav before your changes. Save, then rebuild it."
          : "Made for the old nav. Rebuild it, or play the map and the bots rebuild it themselves in the first minutes.";
  const needed = !k || !k.fits || !k.done;
  return (
    <section className="tools-panel">
      <h3>
        Bot knowledge <Info text="What the bots work out from the nav: sightlines, cover, angles to watch, wallbangs. It's made for one exact nav, so after areas are added, moved or deleted it has to be built again. Changing links, moves or places alone keeps it." />
      </h3>
      <p className="small-note">{status}</p>
      {rebuilding ? (
        <JobProgress title="Bots are studying the new nav…" onEnd={onDone} onClose={onDone} />
      ) : (
        <>
          <BusyButton
            className="btn"
            busy={busy === "Starting…"}
            busyLabel="Starting…"
            label="Rebuild bot knowledge"
            disabled={!!busy || st.dirty || !st.rebuild.can || !needed}
            onClick={onRebuild}
          />
          <p className="small-note muted">
            {!st.rebuild.can && st.rebuild.why
              ? st.rebuild.why
              : `A hidden server on this computer loads the map with one bot; it takes ${st.rebuild.about} for this map's ${st.nav.areas.length} areas (longer the first time: it downloads Valve's server, about 1 GB).`}
          </p>
        </>
      )}
    </section>
  );
}
