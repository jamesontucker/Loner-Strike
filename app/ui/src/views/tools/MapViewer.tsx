// The map viewer: pick a map, then walk around it in 3D, switch what's shown, and click
// anything to see what it is.

import { useEffect, useMemo, useState } from "react";
import { useTools, type CameraMode } from "../../stores/tools";
import type { MapInfo } from "../../types";
import { Banner, LoadError, Segmented, Skeleton } from "../../ui";
import { COLLATOR, MapCard } from "../Maps";
import { Empty, Loading } from "./common";
import { Inspector } from "./panels/Inspector";
import { Layers } from "./panels/Layers";
import type { Know, Nav, Replay, Trace } from "./overlays";
import { Timeline } from "./panels/Timeline";
import { useEntities, useKnow, useMesh, useNavData, useToolsInfo, type MapEntity } from "./toolsApi";
import type { MeshBundle } from "./viewer/bundle";
import { fmtVec } from "./viewer/coords";
import { canDraw3d, Scene, SceneBoundary } from "./viewer/Scene";
import { useModelMeshes } from "./viewer/World";
import { ViewerControl, ViewerTools } from "./common/ViewerTools";
import { matches, searchWords } from "./common/search";

const NO_3D = (
  <LoadError
    title="Your system can't show 3D"
    text="Loner-Strike couldn't start 3D drawing on this computer. Updating your system or graphics drivers usually fixes this."
  />
);

export default function MapViewer({ maps }: { maps: MapInfo[] }) {
  const map = useTools((s) => s.map);
  return map ? <Viewer map={map} /> : <MapPicker maps={maps} />;
}

/** Cards drawn at first; the rest come with Show more. */
const PAGE = 120;

function MapPicker({ maps }: { maps: MapInfo[] }) {
  const openMap = useTools((s) => s.openMap);
  const [q, setQ] = useState("");
  const [limit, setLimit] = useState(PAGE);
  const shown = useMemo(() => {
    const w = searchWords(q);
    return maps
      .filter((m) => !m.other && matches(w, m.name, m.title))
      .sort((a, b) => COLLATOR.compare(a.name, b.name));
  }, [maps, q]);
  const pick = (m: MapInfo) => openMap(m.name);
  return (
    <div className="stack-tight">
      <div className="toolbar">
        <input className="search" type="search" aria-label="Search maps" placeholder="Pick a map to look inside" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      {!maps.length ? (
        <Empty title="No maps yet" text="Loner-Strike can't find any maps in your game yet. Add some from Get more or the Maps page, then come back to look inside them." />
      ) : (
        <div className="maps grid">
          {shown.slice(0, limit).map((m) => <MapCard key={m.name} map={m} onClick={pick} />)}
          {!shown.length && <Empty plain text={`No maps match “${q.trim()}”.`} />}
        </div>
      )}
      {shown.length > limit && (
        <div className="row center">
          <button className="btn" onClick={() => setLimit(limit + PAGE * 2)}>Show more ({shown.length - limit} left)</button>
        </div>
      )}
    </div>
  );
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** What a replay adds to the viewer: the recording, its trace, and the bar's title/back. */
export type ReplayExtra = { data: Replay; trace: Trace | null; title: string; note?: string; onBack: () => void };

export function Viewer({ map, replay = null }: { map: string; replay?: ReplayExtra | null }) {
  const openMap = useTools((s) => s.openMap);
  const navOn = useTools((s) => s.layers.nav);
  const knowMode = useTools((s) => s.know);
  const camera = useTools((s) => s.camera);
  const setCamera = useTools((s) => s.setCamera);
  const webgl = useMemo(canDraw3d, []);
  const mesh = useMesh(webgl ? map : null);
  const ents = useEntities(map, mesh.isSuccess);
  const info = useToolsInfo(map, mesh.isSuccess);
  const nav = useNavData(map, mesh.isSuccess && navOn);
  const know = useKnow(map, mesh.isSuccess && navOn && knowMode !== "off");
  const overlays: Overlays = {
    nav: navOn ? nav.data ?? null : null,
    navError: navOn && nav.error instanceof Error ? nav.error.message : null,
    navLoading: navOn && nav.isLoading,
    know: navOn && knowMode !== "off" ? know.data ?? null : null,
    knowError: navOn && knowMode !== "off" && know.error instanceof Error ? know.error.message : null,
    knowLoading: navOn && knowMode !== "off" && know.isLoading,
    replay: replay?.data ?? null,
    trace: replay?.trace ?? null,
  };

  // F fly, O orbit, T top-down; Esc clears what's picked (and lets go of the mouse)
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if (isField(e.target) || e.metaKey || e.ctrlKey || e.altKey || document.querySelector(".sheet-backdrop")) return;
      const to: Record<string, CameraMode> = { KeyF: "fly", KeyO: "orbit", KeyT: "top" };
      if (to[e.code]) setCamera(to[e.code]);
      else if (e.key === "Escape" && !document.pointerLockElement) useTools.getState().select(null);
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, [setCamera]);

  const missing = (info.data?.textures ?? []).filter((t) => t.from === "missing").map((t) => t.name);
  const wads = Array.isArray(info.data?.missingWads) ? (info.data.missingWads as string[]) : [];
  const loading = mesh.isLoading || (mesh.isSuccess && ents.isLoading);
  const failed = mesh.error instanceof Error ? mesh.error.message || "Something went wrong reading this map." : null;

  return (
    <div className="tools-page">
      <div className="tools-bar">
        <span className="map-title">{replay ? replay.title : map}</span>
        {replay?.note ? <span className="muted small-note">{replay.note}</span> : null}
        <button className="btn small" onClick={() => (replay ? replay.onBack() : openMap(null))}>{replay ? "Other recording" : "Other map"}</button>
        <span className="push" />
        <Segmented values="orbit fly top" labels="Orbit|Fly|Top-down" current={camera} onPick={(v) => setCamera(v as CameraMode)} label="Camera" />
      </div>
      {missing.length > 0 && (
        <Banner>
          {missing.length === 1 ? "1 texture is missing" : `${missing.length} textures are missing`}: {missing.slice(0, 6).join(", ")}
          {missing.length > 6 ? "…" : ""}. They show as a magenta checker.
          {wads.length > 0 && <> {wads.length === 1 ? `The file ${wads[0]} isn't in your game.` : `These files aren't in your game: ${wads.join(", ")}.`}</>}
        </Banner>
      )}
      {ents.isError && <Banner>This map's entities couldn't be read, so spawns and doors aren't shown.</Banner>}
      {!webgl ? (
        NO_3D
      ) : failed ? (
        <LoadError title={`Couldn't open ${map}`} text={failed} onRetry={() => void mesh.refetch()} />
      ) : (
        <Loaded bundle={!loading ? mesh.data ?? null : null} entities={ents.data ?? []} map={map} overlays={overlays} />
      )}
    </div>
  );
}

/** The view and its side panels; while the map's data is on its way, a spinner and placeholders. */
/** The bots' data shown on the map, and how fetching it went. */
export type Overlays = { nav: Nav | null; navError: string | null; navLoading: boolean; know: Know | null; knowError: string | null; knowLoading: boolean; replay: Replay | null; trace: Trace | null };

function Loaded({ bundle, entities, map, overlays }: { bundle: MeshBundle | null; entities: MapEntity[]; map: string; overlays: Overlays }) {
  const looking = useTools((s) => s.looking);
  const meshes = useModelMeshes(bundle, entities);
  // the 3D view lost its context or crashed: a way to start it again (n remounts it)
  const [lost, setLost] = useState(false);
  const [n, setN] = useState(0);
  const restart = () => {
    setLost(false);
    setN((v) => v + 1);
  };
  const stopped = (
    <div className="tools-overlay">
      <LoadError title="The 3D view stopped" text="Your computer's graphics stopped drawing the map. This can happen after sleep or a driver update." onRetry={restart} />
    </div>
  );
  return (
    <div className="tools-viewer">
      <div className="tools-stage">
        <div className="tools-canvas">
          {bundle && !bundle.draws.length ? (
            <div className="tools-overlay">
              <LoadError title="Nothing to show" text="This map file has no surfaces to draw. It may be unfinished or damaged." />
            </div>
          ) : bundle && lost ? (
            stopped
          ) : bundle ? (
            <SceneBoundary key={n} fallback={stopped}>
              <Scene bundle={bundle} meshes={meshes} entities={entities} map={map} overlays={overlays} onLost={() => setLost(true)}>
                <ViewerControl />
              </Scene>
            </SceneBoundary>
          ) : (
            <div className="tools-overlay">
              <Loading text="Reading the map…" />
            </div>
          )}
          {looking && <div className="tools-crosshair" aria-hidden="true" />}
        </div>
        <StatusBar />
      </div>
      <aside className="tools-side">
        {overlays.replay && <Timeline replay={overlays.replay} />}
        <Layers overlays={overlays} />
        <ViewerTools map={map} />
        {bundle ? <Inspector entities={entities} meshes={meshes} overlays={overlays} /> : <section className="tools-panel"><Skeleton rows={4} /></section>}
      </aside>
    </div>
  );
}

const HINTS: Record<CameraMode, string> = {
  orbit: "Drag to turn · Right-drag to move · Scroll to zoom · Click to pick",
  fly: "Click the view to look around",
  top: "Drag to move · Scroll to zoom · Click to pick",
};

function StatusBar() {
  const status = useTools((s) => s.status);
  const hover = useTools((s) => s.hover);
  const camera = useTools((s) => s.camera);
  const looking = useTools((s) => s.looking);
  const hint = camera === "fly" && looking ? "WASD to move · Shift faster · Click to pick · Esc to let go" : HINTS[camera];
  return (
    <div className="tools-status" aria-live="off">
      <span className="mono" title="Where the camera is, in game units (x y z), and which way it faces">
        {status ? <>{fmtVec(status.pos)} · facing {Math.round(status.yaw)}°</> : "—"}
      </span>
      <span className="tools-hover mono">{hover ? <>{hover.label}{hover.point && <span className="muted"> at {fmtVec(hover.point)}</span>}</> : ""}</span>
      <span className="tools-hint" title={hint}>{hint}</span>
    </div>
  );
}
