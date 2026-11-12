import { create } from "zustand";
import type { Vec3 } from "../views/tools/viewer/coords";

// The Tools page's view state (plan 3.1): which tool and map are open, the layers, the
// camera, what's picked. Documents (the mesh, entities) live in TanStack Query.

export type Tool = "map" | "replay";
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
};

export type Selection =
  | { kind: "entity"; index: number }
  | { kind: "face"; texture: string; model: number; face: number | null; point: Vec3 }
  | null;

export type CameraStatus = { pos: Vec3; yaw: number; pitch: number };

type ToolsState = {
  tool: Tool;
  map: string | null;
  layers: Layers;
  camera: CameraMode;
  selection: Selection;
  /** what's under the mouse: a texture or entity, and where (GoldSrc) */
  hover: { label: string; point: Vec3 | null } | null;
  status: CameraStatus | null;
  /** fly mode has the mouse (pointer lock or a drag) */
  looking: boolean;
  /** a request to move the camera to a point; n changes on every request */
  lookAt: { point: Vec3; size: number; n: number } | null;
  setTool: (t: Tool) => void;
  openMap: (m: string | null) => void;
  setLayer: (k: keyof Layers, on: boolean) => void;
  setCamera: (c: CameraMode) => void;
  select: (s: Selection) => void;
  setHover: (label: string | null, point?: Vec3 | null) => void;
  setStatus: (s: CameraStatus) => void;
  setLooking: (v: boolean) => void;
  look: (point: Vec3, size?: number) => void;
};

const DEFAULT_LAYERS: Layers = { textures: true, lighting: true, entities: true, moving: true, triggers: false };

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
  tool: "map",
  map: null,
  layers: savedLayers(),
  camera: savedCamera(),
  selection: null,
  hover: null,
  status: null,
  looking: false,
  lookAt: null,
  setTool: (tool) => set({ tool }),
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
