import { create } from "zustand";
import type { Vec3 } from "../views/tools/viewer/coords";
import { useToolPage } from "./toolPage";

// The Tools page's view state (plan 3.1): which tool and map are open, the layers, the
// camera, what's picked. Documents (the mesh, entities) live in TanStack Query.

/** a tool's id in views/tools/registry.tsx; "" is the Tools overview (kept in toolPage.ts) */
export type Tool = string;
export type CameraMode = "orbit" | "fly" | "top";

export type Layers = {
  /** the map's own textures (off: a flat colour per texture) */
  textures: boolean;
  /** baked lighting from the map */
  lighting: boolean;
  /** spawns, lights, items and other point entities */
  entities: boolean;
  /** doors, breakables, ladders, platforms (func_* brush entities) */
  moving: boolean;
  /** triggers, buy zones, bomb spots (invisible in the game) */
  triggers: boolean;
  /** the bots' nav: where they can walk */
  nav: boolean;
};

export type NavColour = "place" | "island" | "moves";
export type KnowMode = "off" | "cover" | "sight" | "bangs" | "corners" | "angles";
export type HeatMode = "off" | "time" | "deaths" | "stuck";

export type Selection =
  | { kind: "entity"; index: number }
  | { kind: "face"; texture: string; model: number; face: number | null; point: Vec3 }
  /** a nav area, by its place in the nav file */
  | { kind: "area"; index: number }
  /** a player in the open recording */
  | { kind: "bot"; id: number }
  | null;

export type CameraStatus = { pos: Vec3; yaw: number; pitch: number };

type ToolsState = {
  map: string | null;
  layers: Layers;
  camera: CameraMode;
  selection: Selection;
  /** what's under the mouse: a texture or entity, and where (GoldSrc) */
  hover: { label: string; point: Vec3 | null } | null;
  status: CameraStatus | null;
  /** fly mode has the mouse (pointer lock or a drag) */
  looking: boolean;
  /** a request to move the camera to a point; n changes on every request. The viewer
   *  clears it once it has looked, so one made before the map is up still happens. */
  lookAt: { point: Vec3; size: number; n: number } | null;
  /** what the nav focus asked a tool to open ("<tool>:<arg>"), for tools that take one */
  arg: string | null;
  navColour: NavColour;
  know: KnowMode;
  /** the open recording (Replay) and where it's at */
  replay: string | null;
  time: number;
  playing: boolean;
  speed: number;
  /** the camera follows this player */
  follow: number | null;
  heat: HeatMode;
  setTool: (t: Tool) => void;
  setNavColour: (c: NavColour) => void;
  setKnow: (k: KnowMode) => void;
  openReplay: (id: string | null, map?: string | null) => void;
  setTime: (t: number) => void;
  setPlaying: (v: boolean) => void;
  setSpeed: (v: number) => void;
  setFollow: (id: number | null) => void;
  setHeat: (h: HeatMode) => void;
  openMap: (m: string | null) => void;
  setLayer: (k: keyof Layers, on: boolean) => void;
  setCamera: (c: CameraMode) => void;
  select: (s: Selection) => void;
  setHover: (label: string | null, point?: Vec3 | null) => void;
  setStatus: (s: CameraStatus) => void;
  setLooking: (v: boolean) => void;
  look: (point: Vec3, size?: number) => void;
};

const DEFAULT_LAYERS: Layers = { textures: true, lighting: true, entities: true, moving: true, triggers: false, nav: false };

function savedLayers(): Layers {
  try {
    const v = JSON.parse(localStorage.getItem("ls.tools.layers") ?? "null");
    return v && typeof v === "object" ? { ...DEFAULT_LAYERS, ...v } : DEFAULT_LAYERS;
  } catch {
    return DEFAULT_LAYERS;
  }
}
function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}
const savedCamera = (): CameraMode => {
  try {
    const v = localStorage.getItem("ls.tools.camera");
    return v === "fly" || v === "top" ? v : "orbit";
  } catch {
    return "orbit";
  }
};

export const useTools = create<ToolsState>((set, get) => ({
  map: null,
  layers: savedLayers(),
  camera: savedCamera(),
  selection: null,
  hover: null,
  status: null,
  looking: false,
  lookAt: null,
  arg: null,
  navColour: "island",
  know: "off",
  replay: null,
  time: 0,
  playing: false,
  speed: 4,
  follow: null,
  heat: "off",
  // a look-at left waiting by the last tool (no 3D view up) mustn't move the next one's
  setTool: (tool) => {
    useToolPage.setState({ tool });
    set({ lookAt: null });
  },
  setNavColour: (navColour) => set({ navColour }),
  setKnow: (know) => set({ know }),
  // a recording opens on its own map
  openReplay: (replay, map = null) => set({ replay, map: replay ? map : get().map, time: 0, playing: false, follow: null, selection: null }),
  setTime: (time) => set({ time }),
  setPlaying: (playing) => set({ playing }),
  setSpeed: (speed) => set({ speed }),
  setFollow: (follow) => set({ follow }),
  setHeat: (heat) => set({ heat }),
  openMap: (map) => set({ map, selection: null, hover: null, status: null, lookAt: null }),
  setLayer: (k, on) => {
    const layers = { ...get().layers, [k]: on };
    keep("ls.tools.layers", JSON.stringify(layers));
    set({ layers });
  },
  setCamera: (camera) => {
    keep("ls.tools.camera", camera);
    set({ camera });
  },
  select: (selection) => set({ selection }),
  setHover: (label, point = null) => {
    const h = get().hover;
    if (!label) return h && set({ hover: null });
    // the label alone changes rarely; the point on every move (rounded, so it settles)
    const p = point ? (point.map(Math.round) as Vec3) : null;
    if (h?.label === label && String(h.point) === String(p)) return;
    set({ hover: { label, point: p } });
  },
  setStatus: (status) => set({ status }),
  setLooking: (looking) => set({ looking }),
  look: (point, size = 64) => set((s) => ({ lookAt: { point, size, n: (s.lookAt?.n ?? 0) + 1 } })),
}));

/** The argument the nav focus gave the open tool (e.g. a file or map name), or null. */
export const useToolArg = () => useTools((s) => s.arg);
