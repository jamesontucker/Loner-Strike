// P4's editor helpers: the prefab library's page state, what the 3D view(s) can be asked from
// outside the canvas (what's under a point for drag and drop, the camera, a framed picture
// for a prefab thumbnail, the box in view for the cordon), the settings kept in the project
// (visibility sets, the cordon), and picking up changes made from outside (MCP agents).

import { kitPlacement } from "./kits"; // P5
import type { RootState } from "@react-three/fiber";
import { useEffect } from "react";
import * as THREE from "three";
import { create } from "zustand";
import { gsToThree, threeToGs, type Vec3 } from "../viewer/coords";
import * as api from "./editorApi";
import type { Cordon, CordonBox, Doc, VisSet } from "./editorApi";
import { useEditor } from "./store";

// ---- the views, from outside the canvas ---------------------------------------------------

type SceneReg = { el: () => HTMLElement; state: () => RootState };
const scenes = new Set<SceneReg>();

/** A 3D view announces itself (one per pane). */
export function registerScene(r: SceneReg) {
  scenes.add(r);
  return () => void scenes.delete(r);
}

const rectOf = (r: SceneReg) => r.el().getBoundingClientRect();
const isPersp = (r: SceneReg) => r.state().camera instanceof THREE.PerspectiveCamera;

/** The view under a point of the window. */
export function sceneAt(x: number, y: number): SceneReg | null {
  for (const r of scenes) {
    const b = rectOf(r);
    if (x >= b.left && x <= b.right && y >= b.top && y <= b.bottom && b.width > 0) return r;
  }
  return null;
}

/** The 3D (perspective) view, else any. */
function mainScene(): SceneReg | null {
  let any: SceneReg | null = null;
  for (const r of scenes) {
    if (rectOf(r).width <= 0) continue;
    if (isPersp(r)) return r;
    any ??= r;
  }
  return any;
}

export type PointHit = { point: Vec3; normal: Vec3; kind: "brush" | "entity" | "plane"; id: number | null };

/** What's under a point of the window: a brush face (and which way it faces), an entity, else
 *  the ground plane (z 0 in the 3D view, the view's own plane in flat ones). */
export function pickAt(x: number, y: number): PointHit | null {
  const r = sceneAt(x, y);
  if (!r) return null;
  const st = r.state();
  const b = rectOf(r);
  const ndc = new THREE.Vector2(((x - b.left) / b.width) * 2 - 1, -((y - b.top) / b.height) * 2 + 1);
  const rc = new THREE.Raycaster();
  rc.setFromCamera(ndc, st.camera);
  let plane: PointHit | null = null;
  for (const i of rc.intersectObjects(st.scene.children, true)) {
    const ud = i.object.userData as { kind?: string; id?: number; sky?: boolean };
    if (ud.kind === "brush" && ud.id != null && !ud.sky && i.object.visible) {
      const n = i.face?.normal;
      return { point: threeToGs(i.point), normal: n ? [n.x, n.y, n.z] : [0, 0, 1], kind: "brush", id: ud.id };
    }
    if (ud.kind === "entity" && ud.id != null) return { point: threeToGs(i.point), normal: [0, 0, 1], kind: "entity", id: ud.id };
    if (ud.kind === "plane" && !plane) plane = { point: threeToGs(i.point), normal: [0, 0, 1], kind: "plane", id: null };
  }
  if (plane) return plane;
  // nothing at all: the ground (z 0) under the ray
  const o = threeToGs(rc.ray.origin);
  const d = threeToGs(rc.ray.direction);
  if (Math.abs(d[2]) < 1e-6) return null;
  const t = -o[2] / d[2];
  return t > 0 ? { point: [o[0] + d[0] * t, o[1] + d[1] * t, 0], normal: [0, 0, 1], kind: "plane", id: null } : null;
}

/** Where the 3D camera is (GoldSrc), for a playtest spawn. */
export function cameraPos(): Vec3 | null {
  const r = [...scenes].find(isPersp);
  return r ? threeToGs(r.state().camera.position) : null;
}

/** The part of the map in view, as a box for the cordon: a flat view's rectangle (full
 *  height of the map), or a 1024-wide box where the 3D camera looks. */
export function viewBox(mapBounds: [Vec3, Vec3]): CordonBox | null {
  const r = mainScene();
  if (!r) return null;
  const st = r.state();
  const cam = st.camera;
  const round = (v: number, up: boolean) => (up ? Math.ceil(v / 16) * 16 : Math.floor(v / 16) * 16);
  if (cam instanceof THREE.OrthographicCamera) {
    const corners: Vec3[] = [];
    for (const [nx, ny] of [[-1, -1], [1, 1], [-1, 1], [1, -1]]) corners.push(threeToGs(new THREE.Vector3(nx, ny, 0).unproject(cam)));
    const min: Vec3 = [0, 0, 0];
    const max: Vec3 = [0, 0, 0];
    for (let i = 0; i < 3; i++) {
      const vals = corners.map((c) => c[i]);
      const spread = Math.max(...vals) - Math.min(...vals);
      // the axis the view looks along takes the map's whole depth
      if (spread < 1) {
        min[i] = round(mapBounds[0][i] - 16, false);
        max[i] = round(mapBounds[1][i] + 16, true);
      } else {
        min[i] = round(Math.min(...vals), false);
        max[i] = round(Math.max(...vals), true);
      }
    }
    return { min, max };
  }
  const dir = new THREE.Vector3();
  cam.getWorldDirection(dir);
  const at = threeToGs(cam.position.clone().add(dir.multiplyScalar(384)));
  return { min: [round(at[0] - 512, false), round(at[1] - 512, false), round(at[2] - 192, false)], max: [round(at[0] + 512, true), round(at[1] + 512, true), round(at[2] + 320, true)] };
}

/** A picture of part of the map (a prefab's thumbnail): rendered off screen from the 3D view's
 *  direction, framed on the box. A PNG data URL, or null without a view. */
export function snapshot(box: [Vec3, Vec3], w = 192, h = 144): string | null {
  const r = mainScene();
  if (!r) return null;
  const st = r.state();
  const mid = box[0].map((x, i) => (x + box[1][i]) / 2) as Vec3;
  const radius = Math.max(24, Math.hypot(box[1][0] - box[0][0], box[1][1] - box[0][1], box[1][2] - box[0][2]) / 2);
  const cam = new THREE.PerspectiveCamera(40, w / h, 1, 40000);
  const target = gsToThree(mid);
  const dir = new THREE.Vector3();
  if (isPersp(r)) st.camera.getWorldDirection(dir);
  else dir.copy(gsToThree([-1, 1.3, -0.9]).normalize());
  const dist = radius / Math.sin(((40 / 2) * Math.PI) / 180) * 1.05;
  cam.position.copy(target).sub(dir.normalize().multiplyScalar(dist));
  cam.lookAt(target);
  cam.updateProjectionMatrix();
  const gl = st.gl;
  const rt = new THREE.WebGLRenderTarget(w * 2, h * 2, { samples: 4 });
  const prev = gl.getRenderTarget();
  try {
    gl.setRenderTarget(rt);
    gl.clear();
    gl.render(st.scene, cam);
    const px = new Uint8Array(w * 2 * h * 2 * 4);
    gl.readRenderTargetPixels(rt, 0, 0, w * 2, h * 2, px);
    const big = document.createElement("canvas");
    big.width = w * 2;
    big.height = h * 2;
    const bctx = big.getContext("2d");
    if (!bctx) return null;
    const img = bctx.createImageData(w * 2, h * 2);
    // the GPU's rows run bottom up
    for (let y = 0; y < h * 2; y++) img.data.set(px.subarray((h * 2 - 1 - y) * w * 2 * 4, (h * 2 - y) * w * 2 * 4), y * w * 2 * 4);
    bctx.putImageData(img, 0, 0);
    const out = document.createElement("canvas");
    out.width = w;
    out.height = h;
    const ctx = out.getContext("2d");
    if (!ctx) return null;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(big, 0, 0, w, h);
    return out.toDataURL("image/png");
  } catch {
    return null;
  } finally {
    gl.setRenderTarget(prev);
    rt.dispose();
    st.invalidate();
  }
}

// ---- page state -------------------------------------------------------------------------

type P4State = {
  /** a prefab (or a component) waiting for a click in the view to be placed */
  placing: { id: string; name: string; component?: boolean; size?: Vec3 } | null;
  /** where it would land (a see-through box in the views), while dragging a card or placing */
  ghost: { min: Vec3; max: Vec3; yaw: number } | null;
  /** extra quarter turns for the next prefab (R while placing) */
  turn: number;
  /** the prefab library changed (saved, imported): panels refetch */
  prefabRev: number;
  /** the editor tab to show (another page asked: "editor:prefabs") */
  wantTab: string | null;
  set: (p: Partial<Omit<P4State, "set">>) => void;
};

export const useP4 = create<P4State>((set) => ({
  placing: null,
  ghost: null,
  turn: 0,
  prefabRev: 0,
  wantTab: null,
  set: (p) => set(p),
}));

/** Drops a prefab where a point of the window is (on the face under it, or the ground). */
export async function placePrefabAt(id: string, x: number, y: number) {
  const s = useEditor.getState();
  const hit = pickAt(x, y);
  if (!hit) return s.notify("Drop it on the map in the view.", true);
  const r = await s.cmd({ op: "place_prefab", prefab: id, point: hit.point, normal: hit.normal, grid: s.alt ? 0 : s.grid, yaw: useP4.getState().turn });
  if (r.created) {
    const owned = new Set(r.created.entities);
    s.select({ brushes: r.created.brushes.filter((b) => !owned.has(useEditor.getState().brushes.get(b)?.entity ?? -1)), entities: r.created.entities, face: null });
  }
}

// ---- where a dropped prefab lands (the same rules as editor/prefab.rs) ----------------------

/** What's being dragged from the Library (drag data can't be read until the drop). */
export const dragging: { current: { id: string; size: Vec3; component?: boolean } | null } = { current: null };

/** prefab.rs wall_yaw: on a wall, its front (+y) faces out. */
export function wallYaw(n: Vec3): number {
  const l = Math.hypot(n[0], n[1], n[2]);
  if (!(l > 0.5) || Math.abs(n[2] / l) >= 0.7) return 0;
  let yaw = (Math.atan2(n[1], n[0]) * 180) / Math.PI - 90;
  const q = Math.round(yaw / 90) * 90;
  if (Math.abs(yaw - q) < 1) yaw = q;
  return ((yaw % 360) + 360) % 360;
}

/** prefab.rs place_transform for a centred box of `size`: the box where it would land. */
export function landingBox(size: Vec3, point: Vec3, normal: Vec3, grid: number, extraYaw: number) {
  const yaw = (wallYaw(normal) + extraYaw + 360) % 360;
  const r = (yaw * Math.PI) / 180;
  const c = Math.abs(Math.cos(r));
  const sn = Math.abs(Math.sin(r));
  // turned about the middle of its footprint (its origin)
  const w = size[0] * c + size[1] * sn;
  const d = size[0] * sn + size[1] * c;
  const lo: Vec3 = [-w / 2, -d / 2, 0];
  const hi: Vec3 = [w / 2, d / 2, size[2]];
  const l = Math.hypot(...normal);
  const n: Vec3 = l > 0.5 ? normal : [0, 0, 1];
  const a = Math.abs(n[0]) >= Math.abs(n[1]) && Math.abs(n[0]) >= Math.abs(n[2]) ? 0 : Math.abs(n[1]) >= Math.abs(n[2]) ? 1 : 2;
  const snapG = (v: number) => (grid > 0 ? Math.round(v / grid) * grid : Math.round(v));
  const off: Vec3 = [0, 0, 0];
  for (let i = 0; i < 3; i++) {
    if (i === a) off[i] = n[i] >= 0 ? point[i] - lo[i] : point[i] - hi[i];
    else off[i] = snapG(point[i] - (hi[i] - lo[i]) / 2) - lo[i];
  }
  return { min: lo.map((v, i) => v + off[i]) as Vec3, max: hi.map((v, i) => v + off[i]) as Vec3, yaw };
}

/** The ghost for what's dragged or being placed, under a point of the window. */
export function updateGhost(x: number, y: number, what: { size?: Vec3; component?: boolean } | null) {
  const p4 = useP4.getState();
  if (!what) return p4.ghost && p4.set({ ghost: null });
  const hit = pickAt(x, y);
  if (!hit) return p4.ghost && p4.set({ ghost: null });
  const s = useEditor.getState();
  const grid = s.alt ? 0 : s.grid;
  let ghost;
  if (what.component || !what.size) {
    const p = hit.point.map((v, i) => Math.round(v + hit.normal[i] * 16)) as Vec3;
    ghost = { min: p.map((v) => v - 16) as Vec3, max: p.map((v) => v + 16) as Vec3, yaw: 0 };
  } else ghost = landingBox(what.size, hit.point, hit.normal, grid, p4.turn);
  const g = p4.ghost;
  if (!g || g.min.some((v, i) => v !== ghost.min[i]) || g.max.some((v, i) => v !== ghost.max[i])) p4.set({ ghost });
}

// ---- game mode components ----------------------------------------------------------------

let compCache: Promise<{ groups: string[]; components: api.ComponentDef[] }> | null = null;
/** The component library (fetched once). */
export function loadComponents() {
  compCache ??= api.getComponents().then((r) => (r.error ? ((compCache = null), { groups: [], components: [] }) : r));
  return compCache;
}

const snapTo = (v: number, g: number) => (g > 0 ? Math.round(v / g) * g : Math.round(v));

/** Drops a component where a point of the window is: a point entity a little off the surface
 *  (components find the floor themselves), or for brush components (ls_barricade) a box tied to it. */
export async function placeComponentAt(cls: string, x: number, y: number) {
  const s = useEditor.getState();
  const hit = pickAt(x, y);
  if (!hit) return s.notify("Drop it on the map in the view.", true);
  // P5: a mode map kit's marker or component ("kit:<mode>:<id>", kits.ts)
  let keys: [string, string][] | undefined;
  if (cls.startsWith("kit:")) {
    const k = kitPlacement(cls);
    if (!k) return s.notify("That mode's kit isn't installed any more.", true);
    cls = k.classname;
    keys = k.keys.length ? k.keys : undefined;
  }
  const lib = await loadComponents();
  const def = lib.components.find((c) => c.class === cls);
  const g = s.alt ? 0 : s.grid;
  const p = hit.point.map((v, i) => (Math.abs(hit.normal[i]) > 0.7 ? v : snapTo(v, g))) as Vec3;
  if (def?.brush) {
    const min: Vec3 = [p[0] - 32, p[1] - 8, p[2]];
    const max: Vec3 = [p[0] + 32, p[1] + 8, p[2] + 96];
    const r = await s.cmd({ op: "add_box", min, max, texture: s.texture });
    const b = r.created?.brushes ?? [];
    if (!b.length) return;
    const t = await s.cmd({ op: "tie", brushes: b, classname: cls });
    if (t.created?.entities?.length) s.select({ brushes: [], entities: t.created.entities, face: null });
    return;
  }
  const o = p.map((v, i) => Math.round(v + hit.normal[i] * 16)) as Vec3;
  const r = await s.cmd({ op: "add_entity", classname: cls, origin: o, ...(keys ? { keys } : {}) });
  if (r.created) s.select({ brushes: [], entities: r.created.entities, face: null });
}

// ---- settings kept in the project ----------------------------------------------------------

export const NO_CORDON: Cordon = { boxes: [], on: false, seal: "sky", keepLights: false };
export const cordonOf = (d: Doc | null | undefined): Cordon => d?.editor?.cordon ?? NO_CORDON;

async function saveSettings(body: { visSets?: VisSet[]; cordon?: Cordon }) {
  const s = useEditor.getState();
  const d = s.doc;
  if (!d) return;
  // shown at once; the app's answer settles it
  useEditor.setState({ doc: { ...d, editor: { fgd: d.editor?.fgd ?? null, visSets: d.editor?.visSets ?? [], cordon: cordonOf(d), ...body } } });
  const r = await api.saveSettings(body);
  if (r.error) return s.notify(r.error, true);
  const now = useEditor.getState().doc;
  if (now) useEditor.setState({ doc: { ...now, dirty: r.dirty, revision: r.revision, editor: r.editor } });
}

export const setCordon = (patch: Partial<Cordon>) => saveSettings({ cordon: { ...cordonOf(useEditor.getState().doc), ...patch } });
export const setVisSets = (visSets: VisSet[]) => saveSettings({ visSets });

/** Whether a brush is outside every cordon box (drawn dimmed while the cordon is on). */
export function useCordonDim(): ((b: { min: Vec3; max: Vec3 }) => boolean) | null {
  const c = useEditor((s) => s.doc?.editor?.cordon);
  if (!c?.on || !c.boxes.length) return null;
  return (b) => !c.boxes.some((x) => b.min.every((v, i) => v < x.max[i]) && b.max.every((v, i) => v > x.min[i]));
}

// ---- changes made from outside -----------------------------------------------------------

const POLL_MS = 1500;

/** Picks up changes made to the document from outside the page (AI agents over MCP, another
 *  window): the app's revision is checked every moment while the page is visible. */
export function useOutsideChanges() {
  useEffect(() => {
    let busy = false;
    const tick = async () => {
      const s = useEditor.getState();
      if (busy || document.hidden || !s.doc || s.working > 0 || s.fileBusy || s.preview) return;
      busy = true;
      try {
        const st = await api.getState();
        const now = useEditor.getState();
        if (st.error || !now.doc || now.working > 0) return;
        if (st.docId !== now.doc.docId) {
          await now.load();
          return;
        }
        if (st.revision !== now.doc.revision || st.undo !== now.doc.undo || st.dirty !== now.doc.dirty) {
          await now.syncMeshes();
          await now.refreshDoc();
          now.refreshChecks();
        }
      } finally {
        busy = false;
      }
    };
    const id = setInterval(tick, POLL_MS);
    window.addEventListener("focus", tick);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", tick);
    };
  }, []);
}
