// The nav fixer's cameras. Made for getting up close to a beam or a ledge:
//  3D        drag to turn round the point you're looking at, right- or middle-drag (or
//            Shift+drag) to pan, scroll to zoom toward what's under the mouse (it never
//            flies through it), double-click a spot to go there, F for what's picked
//  Top-down  north up; drag (right- or middle-drag while drawing) to pan, scroll zooms
//            toward the mouse
//  Fly       (the 3D view's other camera) hold right mouse to look round and WASD to fly, Q/E down
//            and up, Shift faster; left-drag looks too, middle- or Shift+drag pans, scroll as above
// Moves glide to where they're going (a short ease), so pans, zooms and focus don't jump.
// Both views write where they are to viewMemory, so switching keeps the same spot in view.

import { OrthographicCamera } from "@react-three/drei";
import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { gsToThree, type Vec3 } from "../viewer/coords";
import { remember, useFix, viewMemory, type ViewKind } from "./fixStore";

const FOV = 70;
const TAN = Math.tan((FOV * Math.PI) / 360);
/** the closest the 3D camera comes to what it zooms toward */
const MIN_DIST = 12;
const EASE = 16;
const DEFAULT_DIR = new THREE.Vector3(0.55, 0.62, 0.55).normalize();

/** Whether a hit is something you can see (a shown mesh, a shown texture, not a line). */
export function visibleHit(h: THREE.Intersection): boolean {
  const o = h.object as THREE.Mesh;
  if (!o.isMesh) return false;
  for (let p: THREE.Object3D | null = o; p; p = p.parent) if (!p.visible) return false;
  const m = o.material;
  if (Array.isArray(m)) return h.face ? !!m[h.face.materialIndex]?.visible : false;
  return m.visible;
}

/** read on each press: the page's mode is up to date there before this view's props are */
const drawingNow = () => useFix.getState().mode === "draw";
const flyNow = () => useFix.getState().fly;
const FLY_KEYS = ["KeyW", "KeyA", "KeyS", "KeyD", "KeyQ", "KeyE", "ShiftLeft", "ShiftRight"];

type Home = { centre: THREE.Vector3; radius: number; lo: THREE.Vector3; hi: THREE.Vector3 };
type Common = { map: string; bounds: [Vec3, Vec3] };

export function FixCamera({ view, ...p }: Common & { view: ViewKind }) {
  const home = useMemo<Home>(() => {
    const a = gsToThree(p.bounds[0]);
    const b = gsToThree(p.bounds[1]);
    const lo = a.clone().min(b);
    const hi = a.clone().max(b);
    return { centre: lo.clone().add(hi).multiplyScalar(0.5), radius: Math.max(256, a.distanceTo(b) / 2), lo, hi };
  }, [p.bounds]);
  return view === "orbit" ? <Orbit {...p} home={home} /> : <Top {...p} home={home} />;
}

/** Shared by both: the canvas, the mouse in -1..1, and what's under it. */
function usePointer() {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const raycaster = useThree((s) => s.raycaster);
  return useMemo(() => {
    const ndc = (e: { clientX: number; clientY: number }) => {
      const r = gl.domElement.getBoundingClientRect();
      return new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    };
    const surface = (cam: THREE.Camera, at: THREE.Vector2): THREE.Vector3 | null => {
      raycaster.setFromCamera(at, cam);
      for (const h of raycaster.intersectObjects(scene.children, true)) if (visibleHit(h)) return h.point.clone();
      return null;
    };
    /** the floor straight below a point (or null) */
    const floorBelow = (p: THREE.Vector3, top: number): number | null => {
      raycaster.set(new THREE.Vector3(p.x, top, p.z), new THREE.Vector3(0, -1, 0));
      for (const h of raycaster.intersectObjects(scene.children, true)) if (visibleHit(h)) return h.point.y;
      return null;
    };
    return { el: gl.domElement, ndc, surface, floorBelow };
  }, [gl, scene, raycaster]);
}

type Drag = { kind: "turn" | "look" | "pan"; x: number; y: number; moved: boolean; button: number; depth: number };

// ---- 3D ----------------------------------------------------------------------------------------

function Orbit({ map, home }: Common & { home: Home }) {
  const camera = useThree((s) => s.camera) as THREE.PerspectiveCamera;
  const invalidate = useThree((s) => s.invalidate);
  const ptr = usePointer();
  const cur = useRef({ pos: new THREE.Vector3(), target: new THREE.Vector3() });
  const goal = useRef({ pos: new THREE.Vector3(), target: new THREE.Vector3() });
  const applied = useRef(0);
  /** Fly: the right button is held (keys fly the camera) and which keys are down */
  const flying = useRef(false);
  const keys = useRef(new Set<string>());

  const save = (touch = true) => {
    const g = goal.current;
    remember(map, "orbit", g.target, g.pos.distanceTo(g.target), g.pos.clone().sub(g.target), touch);
  };
  /** look at a point from the way you're facing now, `dist` away (a little from above) */
  const aim = (target: THREE.Vector3, dist: number, now = false) => {
    const g = goal.current;
    const dir = g.pos.clone().sub(g.target);
    if (dir.lengthSq() < 1) dir.copy(DEFAULT_DIR);
    dir.normalize();
    if (dir.y < 0.35) {
      dir.y = 0.35;
      dir.normalize();
    }
    g.target.copy(target);
    g.pos.copy(target).addScaledVector(dir, dist);
    if (now) {
      cur.current.pos.copy(g.pos);
      cur.current.target.copy(g.target);
    }
    save(!now);
    invalidate();
  };

  // where to start: where the last view was looking, a focus asked for, or the whole map
  useLayoutEffect(() => {
    camera.fov = FOV;
    camera.far = Math.max(20000, home.radius * 8);
    const g = goal.current;
    const m = viewMemory.map === map && viewMemory.target ? viewMemory : null;
    if (m?.target) {
      const t = m.target.clone();
      if (m.from === "top") t.y = ptr.floorBelow(t, home.hi.y + 64) ?? t.y;
      const dir = (m.dir ?? DEFAULT_DIR).clone();
      if (dir.y < 0.35) (dir.y = 0.35), dir.normalize();
      g.target.copy(t);
      g.pos.copy(t).addScaledVector(dir, Math.max(MIN_DIST * 2, m.dist));
    } else {
      g.target.copy(home.centre);
      g.pos.copy(home.centre).addScaledVector(DEFAULT_DIR, home.radius * 1.15);
    }
    cur.current.pos.copy(g.pos);
    cur.current.target.copy(g.target);
    const f = useFix.getState().focus;
    if (f && f.at > (m?.at ?? 0)) {
      applied.current = f.at;
      aim(gsToThree(f.point), Math.max(96, f.size * 2.5), true);
    }
    camera.position.copy(cur.current.pos);
    camera.lookAt(cur.current.target);
    camera.updateProjectionMatrix();
    save(false);
    invalidate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [camera, map, home]);

  // focus requests (F, Show me, Look at, a link to a spot)
  useEffect(
    () =>
      useFix.subscribe((s) => {
        const f = s.focus;
        if (!f || f.at <= applied.current) return;
        applied.current = f.at;
        aim(gsToThree(f.point), Math.max(96, f.size * 2.5));
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  useEffect(() => {
    const { el } = ptr;
    let drag: Drag | null = null;
    const both = (fn: (s: { pos: THREE.Vector3; target: THREE.Vector3 }) => void) => {
      fn(goal.current);
      fn(cur.current);
    };
    const turn = (dx: number, dy: number) =>
      both((s) => {
        const off = s.pos.clone().sub(s.target);
        const sph = new THREE.Spherical().setFromVector3(off);
        sph.theta -= dx * 0.006;
        sph.phi = Math.max(0.03, Math.min(Math.PI * 0.94, sph.phi - dy * 0.006));
        s.pos.copy(s.target).add(new THREE.Vector3().setFromSpherical(sph));
      });
    // Fly: turn where you're looking, standing still (the point looked at swings round you)
    const look = (dx: number, dy: number) =>
      both((s) => {
        const off = s.target.clone().sub(s.pos);
        const sph = new THREE.Spherical().setFromVector3(off);
        sph.theta -= dx * 0.004;
        sph.phi = Math.max(0.05, Math.min(Math.PI - 0.05, sph.phi + dy * 0.004));
        s.target.copy(s.pos).add(new THREE.Vector3().setFromSpherical(sph));
      });
    const stopFlying = () => {
      flying.current = false;
      keys.current.clear();
    };
    const pan = (dx: number, dy: number, depth: number) => {
      const h = Math.max(1, el.getBoundingClientRect().height);
      const k = (depth * 2 * TAN) / h;
      const right = new THREE.Vector3().setFromMatrixColumn(camera.matrix, 0);
      const up = new THREE.Vector3().setFromMatrixColumn(camera.matrix, 1);
      const mv = right.multiplyScalar(-dx * k).add(up.multiplyScalar(dy * k));
      both((s) => {
        s.pos.add(mv);
        s.target.add(mv);
      });
    };
    const down = (e: PointerEvent) => {
      if (flyNow() && e.button === 2) {
        drag = { kind: "look", x: e.clientX, y: e.clientY, moved: false, button: 2, depth: 0 };
        flying.current = true;
        // the keys come here now, wherever focus was
        (document.activeElement as HTMLElement | null)?.blur?.();
        return;
      }
      const panning = e.button === 1 || e.button === 2 || (e.button === 0 && e.shiftKey);
      if (e.button === 0 && drawingNow() && !e.shiftKey) return; // the left button draws
      if (!panning && e.button !== 0) return;
      if (e.button === 1) e.preventDefault();
      let depth = camera.position.distanceTo(cur.current.target);
      if (panning) {
        const hit = ptr.surface(camera, ptr.ndc(e));
        if (hit) depth = hit.clone().sub(camera.position).dot(camera.getWorldDirection(new THREE.Vector3()));
      }
      drag = { kind: panning ? "pan" : flyNow() ? "look" : "turn", x: e.clientX, y: e.clientY, moved: false, button: e.button, depth: Math.max(MIN_DIST, depth) };
    };
    const move = (e: PointerEvent) => {
      if (!drag) return;
      const dx = e.clientX - drag.x;
      const dy = e.clientY - drag.y;
      if (!drag.moved && Math.hypot(dx, dy) < (drag.kind === "look" ? 2 : 5)) return;
      drag.moved = true;
      drag.x = e.clientX;
      drag.y = e.clientY;
      if (drag.kind === "turn") turn(dx, dy);
      else if (drag.kind === "look") look(dx, dy);
      else pan(dx, dy, drag.depth);
      save();
      invalidate();
    };
    const up = (e: PointerEvent) => {
      if (e.button === 2) stopFlying();
      if (drag && e.button === drag.button) drag = null;
    };
    // while the right button is held in Fly, WASD/QE/Shift are the camera's (not the tools' V/D/S...)
    const keyDown = (e: KeyboardEvent) => {
      if (!flying.current || !FLY_KEYS.includes(e.code) || e.metaKey || e.ctrlKey || e.altKey) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      keys.current.add(e.code);
      invalidate();
    };
    const keyUp = (e: KeyboardEvent) => {
      if (keys.current.delete(e.code)) e.stopImmediatePropagation();
    };
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const dy = Math.max(-240, Math.min(240, e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1)));
      if (!dy) return;
      const f = Math.exp(dy * 0.0016);
      const g = goal.current;
      const at = ptr.ndc(e);
      // toward what's under the mouse; past the map's edge, a point as far as the target
      let p = ptr.surface(camera, at);
      if (!p) {
        const ray = new THREE.Raycaster();
        ray.setFromCamera(at, camera);
        p = ray.ray.at(camera.position.distanceTo(cur.current.target), new THREE.Vector3());
      }
      const d = g.pos.distanceTo(p);
      let k = f;
      if (f < 1 && d * f < MIN_DIST) k = Math.min(1, MIN_DIST / Math.max(d, 1e-3));
      if (f > 1 && g.pos.distanceTo(g.target) * f > home.radius * 5) k = 1;
      if (k === 1) return;
      g.pos.sub(p).multiplyScalar(k).add(p);
      g.target.sub(p).multiplyScalar(k).add(p);
      save();
      invalidate();
    };
    const dbl = (e: MouseEvent) => {
      if (drawingNow()) return;
      const hit = ptr.surface(camera, ptr.ndc(e));
      if (!hit) return;
      aim(hit, Math.min(goal.current.pos.distanceTo(goal.current.target), 320));
    };
    const menu = (e: MouseEvent) => e.preventDefault();
    el.addEventListener("pointerdown", down);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("dblclick", dbl);
    el.addEventListener("contextmenu", menu);
    window.addEventListener("keydown", keyDown, true);
    window.addEventListener("keyup", keyUp, true);
    window.addEventListener("blur", stopFlying);
    return () => {
      window.removeEventListener("keydown", keyDown, true);
      window.removeEventListener("keyup", keyUp, true);
      window.removeEventListener("blur", stopFlying);
      el.removeEventListener("pointerdown", down);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("dblclick", dbl);
      el.removeEventListener("contextmenu", menu);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ptr, camera, home, invalidate]);

  useFrame((_, dt) => {
    const c = cur.current;
    const g = goal.current;
    // Fly: the held keys move the camera and the point it looks at together (no glide)
    const k = keys.current;
    if (k.size) {
      const fwd = g.target.clone().sub(g.pos).normalize();
      const right = new THREE.Vector3().crossVectors(fwd, camera.up).normalize();
      const mv = new THREE.Vector3();
      if (k.has("KeyW")) mv.add(fwd);
      if (k.has("KeyS")) mv.sub(fwd);
      if (k.has("KeyD")) mv.add(right);
      if (k.has("KeyA")) mv.sub(right);
      if (k.has("KeyE")) mv.y += 1;
      if (k.has("KeyQ")) mv.y -= 1;
      if (mv.lengthSq() > 0) {
        const fast = k.has("ShiftLeft") || k.has("ShiftRight") ? 3 : 1;
        // big maps fly faster: about 400 units a second, up to 1200 on the biggest
        const speed = Math.max(400, Math.min(1200, home.radius / 4)) * fast;
        mv.normalize().multiplyScalar(speed * Math.min(dt, 0.1));
        for (const s of [g, c]) {
          s.pos.add(mv);
          s.target.add(mv);
        }
        save();
      }
      invalidate();
    }
    const left = c.pos.distanceTo(g.pos) + c.target.distanceTo(g.target);
    if (left > 0.05) {
      const a = 1 - Math.exp(-Math.min(dt, 0.1) * EASE);
      c.pos.lerp(g.pos, a);
      c.target.lerp(g.target, a);
      invalidate();
    } else if (left > 0) {
      c.pos.copy(g.pos);
      c.target.copy(g.target);
    }
    camera.position.copy(c.pos);
    camera.lookAt(c.target);
    // the near plane follows how close you are: sharp far away, no clipping up close
    const near = Math.max(0.5, Math.min(8, c.pos.distanceTo(c.target) / 120));
    if (Math.abs(near - camera.near) > 0.05 * camera.near) {
      camera.near = near;
      camera.updateProjectionMatrix();
    }
  });
  return null;
}

// ---- top-down ----------------------------------------------------------------------------------

function Top({ map, home }: Common & { home: Home }) {
  const size = useThree((s) => s.size);
  const invalidate = useThree((s) => s.invalidate);
  const ptr = usePointer();
  const ref = useRef<THREE.OrthographicCamera>(null);
  const fit = Math.min(size.width / Math.max(64, home.hi.x - home.lo.x), size.height / Math.max(64, home.hi.z - home.lo.z)) * 0.9;
  const lim = useRef({ min: fit / 4, max: 40 });
  lim.current = { min: Math.min(fit / 4, 0.02), max: 40 };
  const height = home.hi.y + 4096;
  const cur = useRef({ x: 0, z: 0, zoom: 1 });
  const goal = useRef({ x: 0, z: 0, zoom: 1 });
  const latest = useRef({ size });
  latest.current = { size };
  const applied = useRef(0);

  const save = (touch = true) => {
    const g = goal.current;
    const y = viewMemory.map === map && viewMemory.target ? viewMemory.target.y : home.centre.y;
    remember(map, "top", new THREE.Vector3(g.x, y, g.z), latest.current.size.height / 2 / g.zoom / TAN, undefined, touch);
  };
  const focusOn = (p: Vec3, sz: number, now = false) => {
    const t = gsToThree(p);
    const g = goal.current;
    const s = latest.current.size;
    g.x = t.x;
    g.z = t.z;
    g.zoom = Math.max(lim.current.min, Math.min(lim.current.max, Math.min(s.width, s.height) / 2 / Math.max(64, sz * 1.6)));
    if (now) Object.assign(cur.current, g);
    save(!now);
    invalidate();
  };

  useLayoutEffect(() => {
    const m = viewMemory.map === map && viewMemory.target ? viewMemory : null;
    const g = goal.current;
    if (m?.target) {
      g.x = m.target.x;
      g.z = m.target.z;
      g.zoom = Math.max(lim.current.min, Math.min(lim.current.max, size.height / 2 / Math.max(1, m.dist * TAN)));
    } else {
      g.x = home.centre.x;
      g.z = home.centre.z;
      g.zoom = fit;
    }
    Object.assign(cur.current, g);
    const f = useFix.getState().focus;
    if (f && f.at > (m?.at ?? 0)) {
      applied.current = f.at;
      focusOn(f.point, f.size, true);
    }
    save(false);
    invalidate();
    // (only on opening the view: resizing keeps where you are)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [map, home]);

  useEffect(
    () =>
      useFix.subscribe((s) => {
        const f = s.focus;
        if (!f || f.at <= applied.current) return;
        applied.current = f.at;
        focusOn(f.point, f.size);
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  useEffect(() => {
    const { el } = ptr;
    let drag: Drag | null = null;
    /** canvas pixels per pixel on screen (the app's text size zooms the page) */
    const scale = () => latest.current.size.height / Math.max(1, el.getBoundingClientRect().height);
    const down = (e: PointerEvent) => {
      if (e.button === 0 && drawingNow() && !e.shiftKey) return;
      if (e.button > 2) return;
      if (e.button === 1) e.preventDefault();
      drag = { kind: "pan", x: e.clientX, y: e.clientY, moved: false, button: e.button, depth: 0 };
    };
    const move = (e: PointerEvent) => {
      if (!drag) return;
      const dx = e.clientX - drag.x;
      const dy = e.clientY - drag.y;
      if (!drag.moved && Math.hypot(dx, dy) < 5) return;
      drag.moved = true;
      drag.x = e.clientX;
      drag.y = e.clientY;
      const s = scale();
      for (const st of [goal.current, cur.current]) {
        st.x -= (dx * s) / cur.current.zoom;
        st.z -= (dy * s) / cur.current.zoom;
      }
      save();
      invalidate();
    };
    const up = (e: PointerEvent) => {
      if (drag && e.button === drag.button) drag = null;
    };
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const dy = Math.max(-240, Math.min(240, e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1)));
      if (!dy) return;
      const g = goal.current;
      const c = cur.current;
      const sz = latest.current.size;
      const at = ptr.ndc(e);
      // the point under the mouse now stays under it
      const px = c.x + (at.x * sz.width) / 2 / c.zoom;
      const pz = c.z - (at.y * sz.height) / 2 / c.zoom;
      const zoom = Math.max(lim.current.min, Math.min(lim.current.max, g.zoom / Math.exp(dy * 0.0016)));
      if (zoom === g.zoom) return;
      g.zoom = zoom;
      g.x = px - (at.x * sz.width) / 2 / zoom;
      g.z = pz + (at.y * sz.height) / 2 / zoom;
      save();
      invalidate();
    };
    const dbl = (e: MouseEvent) => {
      if (drawingNow()) return;
      const at = ptr.ndc(e);
      const c = cur.current;
      const sz = latest.current.size;
      const g = goal.current;
      g.x = c.x + (at.x * sz.width) / 2 / c.zoom;
      g.z = c.z - (at.y * sz.height) / 2 / c.zoom;
      g.zoom = Math.min(lim.current.max, Math.max(g.zoom, Math.min(sz.width, sz.height) / 2 / 400));
      save();
      invalidate();
    };
    const menu = (e: MouseEvent) => e.preventDefault();
    el.addEventListener("pointerdown", down);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("dblclick", dbl);
    el.addEventListener("contextmenu", menu);
    return () => {
      el.removeEventListener("pointerdown", down);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("dblclick", dbl);
      el.removeEventListener("contextmenu", menu);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ptr, invalidate]);

  useFrame((_, dt) => {
    const cam = ref.current;
    if (!cam) return;
    const c = cur.current;
    const g = goal.current;
    const off = Math.abs(c.x - g.x) * c.zoom + Math.abs(c.z - g.z) * c.zoom + Math.abs(Math.log(c.zoom / g.zoom)) * 100;
    if (off > 0.05) {
      const a = 1 - Math.exp(-Math.min(dt, 0.1) * EASE);
      c.x += (g.x - c.x) * a;
      c.z += (g.z - c.z) * a;
      c.zoom = Math.exp(Math.log(c.zoom) + (Math.log(g.zoom) - Math.log(c.zoom)) * a);
      invalidate();
    } else Object.assign(c, g);
    cam.position.set(c.x, height, c.z);
    cam.up.set(0, 0, -1);
    cam.lookAt(c.x, home.lo.y, c.z);
    if (cam.zoom !== c.zoom) {
      cam.zoom = c.zoom;
      cam.updateProjectionMatrix();
    }
  });

  return (
    <OrthographicCamera ref={ref} makeDefault position={[home.centre.x, height, home.centre.z]} zoom={cur.current.zoom || fit} near={1} far={height - home.lo.y + 8192} />
  );
}
