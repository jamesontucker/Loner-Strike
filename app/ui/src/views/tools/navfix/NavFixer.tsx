// Nav fixer (Beta): fix a map's bot nav by hand, the way cs_rats2's beams were fixed. Draw the
// areas the learner missed, link them (both ways, or one way for a drop), mark how bots move
// on them, name them; the problems list says what's still wrong. Save, then rebuild the bots'
// map knowledge, which is made for one nav.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useDirty, useNav } from "../../../store";
import { deadEnd } from "../common/api";
import { useTools, useToolArg } from "../../../stores/tools";
import { Banner, BusyButton, Info, LoadError, Segmented, Skeleton, useAskText, useConfirm, useToast } from "../../../ui";
import { JobProgress } from "../../JobProgress";
import { Check, Loading } from "../common";
import { MapBrowser, rememberMap, type MapBadge } from "../common/MapBrowser";
import { ActionBar } from "../common/ActionBar";
import type { NavArea } from "../overlays";
import type { ToolProps } from "../registry";
import { useReportAddress } from "../common/address";
import { useMesh, useEntities } from "../toolsApi";
import { fmtVec } from "../viewer/coords";
import { canDraw3d, SceneBoundary } from "../viewer/Scene";
import { useModelMeshes } from "../viewer/World";
import { FixScene, LINK, type ColourBy, type Mode } from "./FixScene";
import { saveLayout, saveLinks, savedLayout, savedLinks, useFix, type Layout, type LinkShow, type ViewKind } from "./fixStore";
import { ATTRS, DIRS, editNav, openFix, rebuildKnowledge, redoNav, reloadNav, saveNav, undoNav, REACH_FALLS, REACH_NEVER, REACH_OK, REACH_TRAP, type FixState, type Op, type Problem } from "./navfixApi";
import "./navfix.css";

export default function NavFixer({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("navfix", map ? `navfix:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  useEffect(() => {
    if (map) rememberMap("navfix", map);
  }, [map]);
  if (!map)
    return (
      <MapBrowser
        tool="navfix"
        maps={maps}
        onPick={setMap}
        title="Pick a map to fix its nav"
        note={<>Only maps with a bot nav. <Info text="Add the walkable spots the bots' learning missed (narrow beams, ledges), join them up, and mark drops. The reach check shows where bots can't get to. Only maps with a bot nav are listed." /></>}
        action="Fix nav"
        filter={(m, f) => !!(f ? f.nav : m.nav)}
        none="None of your maps has a bot nav yet. The map doctor can teach bots a map; then it shows up here."
        badges={(_m, f) => {
          if (!f?.nav) return [];
          const b: MapBadge[] = [];
          if (f.nav.matchesMap === false) b.push({ text: "Nav for another version", tone: "bad" });
          if (f.lsv?.done) b.push({ text: "Bots know it", tone: "ok" });
          return b;
        }}
      />
    );
  return <Fixer map={map} onBack={() => setMap(null)} />;
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

const KINDS: [string, string][] = [
  ["unreachable", "Bots can't get there"],
  ["trap", "Bots can't get back"],
  ["falls", "Only by falling in"],
  ["ladder", "Ladders"],
  ["cramped", "No room to stand"],
  ["dangling", "Broken links"],
  ["self", "Broken links"],
  ["duplicate", "Damaged areas"],
  ["flat", "Damaged areas"],
];
const kindLabel = (k: string) => KINDS.find((x) => x[0] === k)?.[1] ?? "Other";

const HINTS: Record<Mode, string> = {
  select: "Click an area to pick it · Shift-click adds more · F looks at it · scroll zooms to the mouse",
  draw: "Drag on the floor to draw an area · right-drag pans · Alt: no snapping · Esc stops",
  link: "Click an area, then the one it leads to · Shift-click for a one-way drop · Esc stops",
};
const MODE_KEY: Record<Mode, string> = { select: "V", draw: "D", link: "L" };
const MODE_NAME: Record<Mode, string> = { select: "Select", draw: "Draw area", link: "Link" };

/** The middle and size of some areas (for F and Look at). */
function areasBox(st: FixState, ids: Iterable<number>): { at: [number, number, number]; size: number } | null {
  let lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const id of ids) {
    const a = st.nav.areas[st.nav.index.get(id) ?? -1];
    if (!a) continue;
    for (const p of [a.nw, a.se]) for (let k = 0; k < 3; k++) (lo[k] = Math.min(lo[k], p[k])), (hi[k] = Math.max(hi[k], p[k]));
  }
  if (!Number.isFinite(lo[0])) return null;
  return { at: [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2], size: Math.max(48, hi[0] - lo[0], hi[1] - lo[1]) };
}

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

  const fly = useFix((s) => s.fly);
  const [layout, setLayoutState] = useState<Layout>(savedLayout);
  const setLayout = (l: Layout) => (setLayoutState(l), saveLayout(l));
  const [links, setLinksState] = useState<LinkShow>(savedLinks);
  const setLinks = (l: LinkShow) => (setLinksState(l), saveLinks(l));
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
  const [saved, setSaved] = useState<string | null>(null);

  useDirty("navfix", !!st?.dirty);
  useLayoutEffect(() => useFix.setState({ mode, busy: !!busy }), [mode, busy]);
  // "look at" asked for through Tools (a link to a spot, another tool): this page's cameras
  useEffect(() => {
    const take = (r: ReturnType<typeof useTools.getState>["lookAt"]) => {
      if (!r) return;
      useTools.setState({ lookAt: null });
      useFix.getState().look(r.point, r.size);
    };
    take(useTools.getState().lookAt);
    return useTools.subscribe((s) => take(s.lookAt));
  }, []);
  // a focus waiting for a view doesn't carry over to the next visit or map
  useEffect(() => () => useFix.setState({ focus: null }), [map]);
  // what was under the mouse goes with the view it was in
  useEffect(() => useFix.setState({ hoverArea: null, hoverPoint: null, hoverView: null, mouse: null, draft: null }), [map, layout]);

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
  const undoRef = useRef(undo);
  undoRef.current = undo;
  /** "Added area 812 · Undo" after a change you might not have meant */
  const said = useCallback((text: string) => toast(text, false, { label: "Undo", run: () => void undoRef.current() }, 6000), [toast]);
  const redo = () => st?.redo && act("Redoing…", () => redoNav(map));
  const save = () =>
    act("Saving…", () => saveNav(map), (s) => {
      setSaved(s.message ?? "Saved.");
      // the map viewer's nav layer reads the file again
      void qc.invalidateQueries({ queryKey: ["tools", "nav", map] });
      void qc.invalidateQueries({ queryKey: ["tools", "know", map] });
    });
  const startOver = async () => {
    if (st?.dirty && !(await confirm("Throw away your changes and read the nav file again?", { ok: "Start over", danger: true }))) return;
    setSaved(null);
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
  // one click, one Undo step (QA4-T2 L1)
  const fixAll = async (list: Problem[]) => {
    const ops = list.flatMap((p) => (p.fix ? [p.fix.op] : []));
    if (ops.length) await edit("Fixing…", ops.length === 1 ? ops[0] : { op: "batch", ops, label: `Fix ${ops.length} problems` });
  };
  const showReach = (r: number) => {
    if (!st) return;
    const ids = st.nav.areas.filter((_, i) => st.reach[i] === r).map((a) => a.id);
    setColourBy("reach");
    setShown(new Set(ids));
    setSelected(new Set());
  };
  const del = (ids: number[]) =>
    ids.length && edit(ids.length === 1 ? "Deleting the area…" : "Deleting the areas…", { op: "delete", ids }, () => (setSelected(new Set()), said(ids.length === 1 ? `Deleted area ${ids[0]}` : `Deleted ${ids.length} areas`)));

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
    (x: [number, number], y: [number, number], z: number) =>
      edit("Adding the area…", { op: "add", x, y, z, snap: true }, (s) => {
        if (s.newId == null) return;
        setSelected(new Set([s.newId]));
        said(`Added area ${s.newId} (${Math.round(x[1] - x[0])} × ${Math.round(y[1] - y[0])}). Link it to its neighbors next.`);
      }),
    [edit, said],
  );

  // the views get handlers that never change and call the latest ones (their props can be a
  // frame behind: a click straight after pressing L must already link)
  const handlers = useRef({ onPick, onDraw });
  handlers.current = { onPick, onDraw };
  const pickNow = useCallback((id: number | null, shift: boolean) => handlers.current.onPick(id, shift), []);
  const drawNow = useCallback((x: [number, number], y: [number, number], z: number) => handlers.current.onDraw(x, y, z), []);

  // keys: undo/redo, delete, Esc, V/D/L for the modes, O/T/S for the view, F to look at the pick
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
        // one step back at a time: the link's start, then the tool, then the pick
        if (linkFrom !== null) setLinkFrom(null);
        else if (mode !== "select") setMode("select");
        else {
          setSelected(new Set());
          setShown(new Set());
        }
      } else if (e.code === "KeyF" && st) {
        const box = areasBox(st, selected.size ? selected : shown.size ? shown : linkFrom !== null ? [linkFrom] : []);
        const h = useFix.getState();
        if (box) h.look(box.at, box.size);
        else if (h.hoverPoint) h.look(h.hoverPoint, 128);
        else toast("Pick an area first, then F looks at it.");
      } else if ((e.key === "Delete" || e.key === "Backspace") && selected.size && mode === "select") {
        e.preventDefault();
        void del([...selected]);
      } else if (e.code === "KeyV") setMode("select");
      else if (e.code === "KeyD") setMode("draw");
      else if (e.code === "KeyL") setMode("link");
      else if (e.code === "KeyO") setLayout("orbit");
      else if (e.code === "KeyT") setLayout("top");
      else if (e.code === "KeyS") setLayout(layout === "split" ? "orbit" : "split");
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
    const box = p.areas.length ? areasBox(st!, p.areas) : null;
    // (1.0.1: at least 256 wide, so a one-area problem is seen from above the room, not from inside a wall)
    useFix.getState().look(p.at, Math.max(256, Math.min(box?.size ?? 256, 1200)));
  };

  const failed = mesh.error instanceof Error ? mesh.error.message : q.error instanceof Error ? q.error.message : null;
  // the app said what's wrong (a damaged map or nav): the Map doctor, not "Try again" (QA4-T2 UX-M6)
  const stuck = deadEnd(mesh.error ?? q.error);
  const ready = mesh.data && st && !ents.isLoading;

  return (
    <div className="tools-page navfix-page">
      <div className="tools-bar">
        <span className="map-title">{map}</span>
        {st?.file && <span className="muted small-note">{st.file}</span>}
        <button className="btn small" onClick={async () => (!st?.dirty || (await confirm("Leave without saving your changes to the nav?", { ok: "Leave", danger: true }))) && onBack()}>
          Other map
        </button>
        <span className="push" />
        <div className="navfix-tools" role="group" aria-label="What clicks do">
          {(["select", "draw", "link"] as Mode[]).map((m) => (
            <button key={m} type="button" aria-pressed={mode === m} disabled={m !== "select" && !ready} title={`${HINTS[m]} (${MODE_KEY[m]})`} onClick={() => setMode(mode === m && m !== "select" ? "select" : m)}>
              {MODE_NAME[m]}<kbd>{MODE_KEY[m]}</kbd>
            </button>
          ))}
        </div>
        <button className="btn" disabled={!st?.undo || !!busy} title={st?.undo ? `Undo: ${st.undo} (Mod+Z)` : "Nothing to undo"} onClick={undo}>Undo</button>
        <button className="btn" disabled={!st?.redo || !!busy} title={st?.redo ? `Redo: ${st.redo} (Shift+Mod+Z)` : "Nothing to redo"} onClick={redo}>Redo</button>
      </div>
      {st?.nav.warning && <Banner>{st.nav.warning}</Banner>}
      {st?.nav.matchesMap === false && <Banner>This nav was made for a different version of the map. Fixing it is fine, but the game may learn a new one when it loads the map.</Banner>}
      {!webgl ? (
        <LoadError title="Your system can't show 3D" text="Loner-Strike couldn't start 3D drawing on this computer. Updating your system or graphics drivers usually fixes this." />
      ) : failed ? (
        <LoadError
          title={mesh.error ? `Couldn't open ${map}` : `Couldn't open ${map}'s nav`}
          text={failed}
          action={stuck ? { label: "Check it in the Map doctor", run: () => useNav.getState().go("tools", `doctor:${map}`) } : undefined}
          onRetry={stuck ? undefined : () => (mesh.error ? void mesh.refetch() : void q.refetch())}
        />
      ) : (
        <div className="tools-viewer">
          <div className="tools-stage">
            <div className={`tools-canvas navfix-canvas navfix-${mode} navfix-layout-${layout}`}>
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
                  <div className="navfix-views">
                    {(layout === "split" ? (["top", "orbit"] as ViewKind[]) : [layout]).map((v) => (
                      // (one view keeps its canvas when it turns between 3D and top-down)
                      <div key={layout === "split" && v === "top" ? "two" : "one"} className="navfix-view" data-view={v}>
                        <FixScene
                          bundle={mesh.data!}
                          meshes={meshes}
                          map={map}
                          view={v}
                          nav={st!.nav}
                          reach={st!.reach}
                          colourBy={colourBy}
                          links={links}
                          mode={mode}
                          oneWay={oneWay}
                          selected={selected}
                          linkFrom={linkFrom}
                          shown={shown}
                          busy={!!busy}
                          onPick={pickNow}
                          onDraw={drawNow}
                          onLost={() => setLost(true)}
                        />
                        {layout === "split" && <span className="navfix-view-label">{v === "top" ? "Top-down" : "3D"}</span>}
                      </div>
                    ))}
                  </div>
                </SceneBoundary>
              )}
              {ready && !lost && <ModeChip mode={mode} linkFrom={linkFrom} oneWay={oneWay} setOneWay={setOneWay} onDone={() => setMode("select")} fly={fly && layout !== "top"} />}
              {ready && (
                <div className="navfix-cam">
                  <Segmented
                    options={[
                      ["orbit", <span key="o" title="Turn round the map (O)">3D</span>],
                      ["top", <span key="t" title="Straight down, north up (T)">Top-down</span>],
                      ["split", <span key="s" title="Top-down and 3D side by side (S)">Split</span>],
                    ]}
                    current={layout}
                    onPick={(v) => setLayout(v as Layout)}
                    label="View"
                  />
                  {layout !== "top" && <FlySwitch />}
                </div>
              )}
              {ready && st && <CursorHint st={st} mode={mode} linkFrom={linkFrom} oneWay={oneWay} />}
              {busy && (
                <div className="navfix-busy" role="status">
                  <span className="spinner" aria-hidden="true" />
                  {busy}
                </div>
              )}
            </div>
            <Status mode={mode} linkFrom={linkFrom} oneWay={oneWay} fly={fly && layout !== "top"} />
          </div>
          <aside className="tools-side">
            {!st ? (
              <section className="tools-panel"><Skeleton rows={4} /></section>
            ) : (
              <>
                <ReachPanel st={st} colourBy={colourBy} setColourBy={setColourBy} links={links} setLinks={setLinks} onShow={showReach} />
                {mode !== "select" && (
                  <section className="tools-panel navfix-mode">
                    <h3>{mode === "link" ? "Link areas" : "Draw an area"} <Info text={mode === "link" ? "Click the area to link from, then the one it leads to. Each link carries on from the last area, so you can click along a beam. One way is for a drop bots can't climb back up (or hold Shift)." : "Drag a box on the floor where players can walk. It's laid at the height you start on and each corner is dropped to the floor. Corners snap to an 8-unit grid and to the edges of areas next to it (the edge turns blue); hold Alt for no snapping. Top-down (T) or Split (S) makes narrow spots easiest. Then link it to its neighbors."} /></h3>
                    {mode === "link" ? (
                      <>
                        <p className="small-note">{linkFrom === null ? "Click the area to link from." : `From area ${linkFrom}: click the area it leads to.`}</p>
                        <Check checked={oneWay} onChange={setOneWay} label="One way (a drop)" title="A drop bots can't climb back up" />
                      </>
                    ) : (
                      <p className="small-note">Drag on the floor. Corners snap to the grid and to the edges next door (Alt: no snapping). {fly && layout !== "top" ? "Middle" : "Right"}-drag moves the view.</p>
                    )}
                  </section>
                )}
                <Selection st={st} ids={[...selected]} busy={!!busy} edit={edit} del={del} />
                <Problems st={st} busy={!!busy} onShow={showProblem} edit={edit} fixAll={fixAll} />
                <Knowledge st={st} busy={busy} rebuilding={rebuilding} onRebuild={rebuild} onDone={() => (setRebuilding(false), void q.refetch())} />
              </>
            )}
          </aside>
        </div>
      )}
      {st && (
        <ActionBar
          dirty={st.dirty}
          working={busy}
          where={saved ?? (st.dirty ? `${st.steps} change${st.steps === 1 ? "" : "s"}${st.did ? ` · last: ${st.did}` : ""}` : st.file)}
          info="Saving keeps the nav as it first was (a .bak file) and every save in History. Start over throws away what isn't saved and reads the file again."
          save={{ label: "Save the nav", onClick: save }}
          undo={startOver}
        />
      )}
    </div>
  );
}

/** The 3D view's camera: orbit round a point, or fly (hold right mouse + WASD) like the Map Viewer. */
function FlySwitch() {
  const fly = useFix((s) => s.fly);
  const setFly = useFix((s) => s.setFly);
  return (
    <Segmented
      options={[
        ["orbit", <span key="o" title="Drag to turn round a point">Orbit</span>],
        ["fly", <span key="f" title="Hold right mouse to look, WASD to fly, Q/E down and up, Shift faster">Fly</span>],
      ]}
      current={fly ? "fly" : "orbit"}
      onPick={(v) => setFly(v === "fly")}
      label="3D camera"
    />
  );
}

function Status({ mode, linkFrom, oneWay, fly }: { mode: Mode; linkFrom: number | null; oneWay: boolean; fly: boolean }) {
  const hover = useTools((s) => s.hover);
  // Fly (a 3D view on screen): the right button looks, so drawing pans with the middle button or Shift+drag
  const hint = fly
    ? `${HINTS[mode].replace("right-drag pans", "middle-drag pans")}\u00a0· Hold right mouse + WASD to fly`
    : HINTS[mode];
  return (
    <div className="tools-status" aria-live="off">
      <span className="mono">{mode === "link" && linkFrom !== null ? `Linking from ${linkFrom}${oneWay ? " (one way)" : ""}` : ""}</span>
      <span className="tools-hover mono">{hover ? <>{hover.label}{hover.point && <span className="muted"> at {fmtVec(hover.point)}</span>}</> : ""}</span>
      <span className="tools-hint">{hint}</span>
    </div>
  );
}

/** What clicks do now, on the view itself: easy to miss in the bar when you're looking at the map. */
function ModeChip({ mode, linkFrom, oneWay, setOneWay, onDone, fly }: { mode: Mode; linkFrom: number | null; oneWay: boolean; setOneWay: (v: boolean) => void; onDone: () => void; fly: boolean }) {
  if (mode === "select") return null;
  return (
    <div className={`navfix-chip navfix-chip-${mode}`} role="status" data-mode={mode}>
      <strong>{mode === "draw" ? "Drawing areas" : "Linking"}</strong>
      <span className="navfix-chip-text">
        {mode === "draw"
          ? `Drag on the floor · ${fly ? "middle" : "right"}-drag pans · Alt: no snapping`
          : linkFrom === null
            ? "Click the area to link from"
            : `From ${linkFrom}: click the area it leads to`}
      </span>
      {mode === "link" && (
        <span className="seg navfix-chip-seg" role="group" aria-label="Link direction">
          <button type="button" aria-pressed={!oneWay} onClick={() => setOneWay(false)} title="Bots walk both ways">Both ways</button>
          <button type="button" aria-pressed={oneWay} onClick={() => setOneWay(true)} title="A drop bots can't climb back up (or hold Shift as you click)">One way</button>
        </span>
      )}
      <button type="button" className="btn small" onClick={onDone} title="Back to Select (Esc)">Done <kbd>Esc</kbd></button>
    </div>
  );
}

/** A short line next to the mouse while drawing or linking: the box's size and whether it
 *  snapped, or what the next click links. */
function CursorHint({ st, mode, linkFrom, oneWay }: { st: FixState; mode: Mode; linkFrom: number | null; oneWay: boolean }) {
  const mouse = useFix((s) => s.mouse);
  const view = useFix((s) => s.hoverView);
  const area = useFix((s) => s.hoverArea);
  const draft = useFix((s) => s.draft);
  const shift = useShift();
  if (mode === "select" || !mouse || !view) return null;
  let text: string;
  let tone = "";
  if (mode === "draw") {
    if (draft && (draft.w || draft.h)) {
      text = `${Math.round(draft.w)} × ${Math.round(draft.h)}${draft.snapped ? " · lined up with the area next to it" : ""}`;
      tone = draft.snapped ? "snap" : "";
    } else text = "Drag to draw an area";
  } else if (linkFrom === null) text = area !== null ? `Link from area ${area}` : "Click an area to start";
  else if (area === null || area === linkFrom) text = area === linkFrom ? "Now click the area it leads to" : "Click the area it leads to";
  else {
    const a = st.nav.areas[st.nav.index.get(linkFrom) ?? -1];
    const there = !!a?.conns.flat().includes(area);
    const back = !!st.nav.areas[st.nav.index.get(area) ?? -1]?.conns.flat().includes(linkFrom);
    const one = oneWay || shift;
    text = there && (one ? !back : back) ? `${linkFrom} → ${area}: already linked` : `${linkFrom} ${one ? "→" : "↔"} ${area}${one ? " · one way (a drop)" : " · both ways"}`;
    tone = one ? "out" : "link";
  }
  const host = document.querySelector(".navfix-canvas")?.getBoundingClientRect();
  const left = mouse.x - (host?.left ?? 0) + 16;
  const top = mouse.y - (host?.top ?? 0) + 18;
  return (
    <div className={`navfix-hint ${tone ? `navfix-hint-${tone}` : ""}`} style={{ left, top }} aria-hidden="true">
      {text}
    </div>
  );
}

function useShift() {
  const [on, setOn] = useState(false);
  useEffect(() => {
    const f = (e: KeyboardEvent) => setOn(e.shiftKey);
    window.addEventListener("keydown", f);
    window.addEventListener("keyup", f);
    return () => {
      window.removeEventListener("keydown", f);
      window.removeEventListener("keyup", f);
    };
  }, []);
  return on;
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
        <button className="btn small" title="F" onClick={() => {
          const box = areasBox(st, ids);
          if (box) useFix.getState().look(box.at, box.size);
        }}>Look at</button>
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
        <p className="small-note">No links: bots can't walk on or off it. Use Link to join it to its neighbors.</p>
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

function Problems({ st, busy, onShow, edit, fixAll }: { st: FixState; busy: boolean; onShow: (p: Problem) => void; edit: Edit; fixAll: (list: Problem[]) => Promise<void> }) {
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const groups = new Map<string, Problem[]>();
  for (const p of st.problems) groups.set(kindLabel(p.kind), [...(groups.get(kindLabel(p.kind)) ?? []), p]);
  return (
    <section className="tools-panel">
      <h3>
        Problems <span className="muted small-note">{st.problems.length + st.moreProblems || ""}</span>
        <Info text="Checked after every change: areas bots can't reach from any spawn (or can't get back from, or only land in by falling off an edge), links to areas that aren't there, spots a standing player doesn't fit, ladders with no area at an end." />
      </h3>
      {st.checkNote && <p className="small-note muted">{st.checkNote}</p>}
      {!st.problems.length ? (
        <p className="small-note navfix-clear">Nothing found. Bots can reach every area from the spawns and get back.</p>
      ) : (
        [...groups.entries()].map(([label, list]) => {
          const isOpen = open[label] ?? groups.size <= 2;
          const fixable = list.filter((p) => p.fix);
          return (
            <div key={label} className={`navfix-group navfix-${list[0].kind}`}>
              <div className="navfix-group-head">
                <button className="navfix-group-btn" aria-expanded={isOpen} onClick={() => setOpen({ ...open, [label]: !isOpen })}>
                  <span className="navfix-caret" aria-hidden="true">{isOpen ? "▾" : "▸"}</span>
                  {label} <span className="muted">{list.length}</span>
                </button>
                {fixable.length > 1 && <button className="btn small" disabled={busy} title="One Undo takes them all back" onClick={() => void fixAll(fixable)}>Fix {fixable.length}</button>}
              </div>
              {isOpen && (
                <ul className="navfix-problems">
                  {list.map((p, i) => (
                    <li key={`${p.kind}-${p.areas[0] ?? i}-${i}`} className={`navfix-problem navfix-${p.kind}`} title={p.hint}>
                      <span>{p.text}</span>
                      <div className="row">
                        <button className="btn small" onClick={() => onShow(p)}>Show me</button>
                        {p.fix && <button className="btn small primary" disabled={busy} onClick={() => void edit("Fixing…", p.fix!.op)}>{p.fix.label}</button>}
                        <Info text={p.hint} />
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          );
        })
      )}
      {st.moreProblems > 0 && <p className="small-note muted">And {st.moreProblems} more not listed.</p>}
    </section>
  );
}

// ---- the reach check -------------------------------------------------------------------------

function ReachPanel({ st, colourBy, setColourBy, links, setLinks, onShow }: { st: FixState; colourBy: ColourBy; setColourBy: (c: ColourBy) => void; links: LinkShow; setLinks: (l: LinkShow) => void; onShow: (r: number) => void }) {
  const n = [0, 0, 0, 0];
  for (const r of st.reach) if (r >= 0 && r <= 3) n[r]++;
  const total = st.reach.length || 1;
  const rows: [number, string, string][] = [
    [REACH_OK, "k-ok", "Bots get there and back"],
    [REACH_NEVER, "k-never", "Can't get there"],
    [REACH_TRAP, "k-trap", "Can't get back"],
    [REACH_FALLS, "k-falls", "Only by falling in"],
  ];
  return (
    <section className="tools-panel">
      <h3>
        Reach check <Info text="From the spawns, along links and ladders (the way bots plan), plus stepping or falling off edges (the way bots really move in fights). Click a line to show those areas." />
      </h3>
      <div className="navfix-bar" aria-hidden="true">
        {rows.map(([r, cls]) => n[r] > 0 && <i key={r} className={cls} style={{ flexGrow: n[r] / total }} />)}
      </div>
      <ul className="navfix-reach">
        {rows.map(([r, cls, label]) => (
          <li key={r}>
            <button className="navfix-reach-row" disabled={!n[r]} onClick={() => onShow(r)}>
              <i className={cls} /> {label} <span className="push">{n[r]}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="tools-field">
        <span>Colors</span>
        <Segmented label="Colors" values="reach place moves" labels="Reach|Places|Moves" current={colourBy} onPick={(v) => setColourBy(v as ColourBy)} />
      </div>
      <div className="tools-field">
        <span>Links <Info text="Picked: the links of the area under the mouse and the picked ones, bright and drawn through walls, plus every one-way drop. All: every link on the map (busy on big maps)." /></span>
        <Segmented label="Links" values="picked all" labels="Picked|All" current={links} onPick={(v) => setLinks(v as LinkShow)} />
      </div>
      <p className="navfix-legend small-note" aria-label="Link colors">
        <span><i style={{ background: LINK.both }} />both ways</span>
        <span><i style={{ background: LINK.out }} />one way out →</span>
        <span><i style={{ background: LINK.into }} />drop into it</span>
      </p>
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
          <p className="small-note muted navfix-row">
            {!st.rebuild.can && st.rebuild.why ? st.rebuild.why : `Takes ${st.rebuild.about}.`}
            <Info text={`A hidden server on this computer loads the map with one bot; it takes ${st.rebuild.about} for this map's ${st.nav.areas.length} areas (longer the first time: it downloads Valve's server, about 1 GB).`} />
          </p>
        </>
      )}
    </section>
  );
}
