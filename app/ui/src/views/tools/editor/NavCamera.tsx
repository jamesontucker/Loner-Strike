// P5: the 3D pane's camera for the Unreal and Hammer navigation presets (navigation.ts).
// Blender keeps B's orbit/fly cameras (viewer/Cameras.tsx). Registers a small controls object
// as the pane's `controls` so the tools (which switch controls off while they drag), the gizmo,
// camera marks and "look at" requests work the same with either.

import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { gsToThree, type Vec3 } from "../viewer/coords";
import { selectionBounds, useEditor } from "./store";
import { SPEEDS, stepSpeed, useNav, type NavPreset } from "./navigation";

const LOOK = 0.0022;
const MOVE_KEYS = ["KeyW", "KeyA", "KeyS", "KeyD", "KeyQ", "KeyE", "ShiftLeft", "ShiftRight", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"];

/** What the tools and the gizmo see as the pane's controls. */
class NavControls extends THREE.EventDispatcher<{ change: object }> {
  enabled = true;
  target = new THREE.Vector3();
  update() {
    /* the camera moves itself */
  }
}

/** True when the key belongs to the camera right now (Shortcuts.ts leaves it alone). */
export function navWantsKey(e: KeyboardEvent): boolean {
  const n = useNav.getState();
  if (n.looking && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) return true;
  if (n.preset === "hammer" && e.code === "KeyZ" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && useEditor.getState().view === "3d") return true;
  return false;
}

export function NavCamera({ bounds, preset }: { bounds: [Vec3, Vec3]; preset: Exclude<NavPreset, "blender"> }) {
  const camera = useThree((s) => s.camera) as THREE.PerspectiveCamera;
  const gl = useThree((s) => s.gl);
  const set = useThree((s) => s.set);
  const invalidate = useThree((s) => s.invalidate);
  const connected = useThree((s) => s.events.connected) as HTMLElement | undefined;
  const ctl = useMemo(() => new NavControls(), []);
  const keys = useRef(new Set<string>());
  const vel = useRef(new THREE.Vector3());
  const euler = useRef(new THREE.Euler(0, 0, 0, "YXZ"));

  // the camera starts above a corner looking at the middle (like the orbit camera)
  const placed = useRef(false);
  useEffect(() => {
    if (placed.current) return;
    placed.current = true;
    const a = gsToThree(bounds[0]);
    const b = gsToThree(bounds[1]);
    const centre = a.clone().add(b).multiplyScalar(0.5);
    const r = Math.max(256, a.distanceTo(b) / 2);
    camera.near = 2;
    camera.far = Math.max(20000, r * 8);
    camera.position.copy(centre).add(new THREE.Vector3(r * 0.55, r * 0.45, r * 0.55));
    camera.lookAt(centre);
    camera.updateProjectionMatrix();
    ctl.target.copy(centre);
    euler.current.setFromQuaternion(camera.quaternion, "YXZ");
    invalidate();
  }, [bounds, camera, ctl, invalidate]);

  // be the pane's controls while mounted
  useEffect(() => {
    set({ controls: ctl as unknown as THREE.EventDispatcher });
    return () => set({ controls: null });
  }, [ctl, set]);

  // "look at" requests (outliner double-click, checks, F / zoom to selection)
  const req = useTools((s) => s.lookAt);
  useEffect(() => {
    if (!req) return;
    useTools.setState({ lookAt: null });
    const p = gsToThree(req.point);
    const dist = Math.max(160, req.size * 2.5);
    const dir = camera.getWorldDirection(new THREE.Vector3());
    camera.position.copy(p).addScaledVector(dir, -dist);
    camera.lookAt(p);
    ctl.target.copy(p);
    euler.current.setFromQuaternion(camera.quaternion, "YXZ");
    invalidate();
  }, [req, camera, ctl, invalidate]);

  useEffect(() => {
    const el = connected?.dataset?.pane != null ? connected : gl.domElement;
    const nav = useNav.getState;
    const turn = (dx: number, dy: number) => {
      const e = euler.current;
      e.setFromQuaternion(camera.quaternion, "YXZ");
      e.z = 0;
      e.y -= dx * LOOK;
      e.x = Math.max(-Math.PI / 2 + 0.01, Math.min(Math.PI / 2 - 0.01, e.x - dy * LOOK));
      camera.quaternion.setFromEuler(e);
      // the orbit point stays ahead at the same distance
      const d = Math.max(64, camera.position.distanceTo(ctl.target));
      ctl.target.copy(camera.position).addScaledVector(camera.getWorldDirection(new THREE.Vector3()), d);
      invalidate();
    };
    const forward = (amount: number) => {
      const dir = camera.getWorldDirection(new THREE.Vector3());
      camera.position.addScaledVector(dir, amount);
      ctl.target.addScaledVector(dir, amount);
      invalidate();
    };
    const pan = (dx: number, dy: number) => {
      const d = Math.max(64, camera.position.distanceTo(ctl.target));
      const k = (d * 2 * Math.tan((camera.fov * Math.PI) / 360)) / Math.max(1, el.clientHeight);
      const right = new THREE.Vector3().setFromMatrixColumn(camera.matrix, 0);
      const up = new THREE.Vector3().setFromMatrixColumn(camera.matrix, 1);
      const move = right.multiplyScalar(-dx * k).add(up.multiplyScalar(dy * k));
      camera.position.add(move);
      ctl.target.add(move);
      invalidate();
    };
    const orbit = (dx: number, dy: number, pivot: THREE.Vector3) => {
      const off = camera.position.clone().sub(pivot);
      const sph = new THREE.Spherical().setFromVector3(off);
      sph.theta -= dx * 0.005;
      sph.phi = Math.max(0.05, Math.min(Math.PI - 0.05, sph.phi - dy * 0.005));
      camera.position.copy(pivot).add(new THREE.Vector3().setFromSpherical(sph));
      camera.lookAt(pivot);
      ctl.target.copy(pivot);
      euler.current.setFromQuaternion(camera.quaternion, "YXZ");
      invalidate();
    };
    const stepScale = () => SPEEDS[nav().speed] / 512;

    type Drag = { kind: "look" | "move" | "orbit" | "dolly" | "pan" | "pending-left"; x: number; y: number; moved: boolean; pivot?: THREE.Vector3; button: number };
    let drag: Drag | null = null;
    const locked = () => document.pointerLockElement === el;
    const lock = () => {
      try {
        const p = el.requestPointerLock() as unknown as Promise<void> | undefined;
        p?.catch?.(() => {});
      } catch {
        /* drag without hiding the pointer */
      }
    };
    const unlock = () => locked() && document.exitPointerLock();
    const pivotNow = () => {
      const b = selectionBounds();
      if (b) return gsToThree(b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3);
      return ctl.target.clone();
    };

    const down = (e: PointerEvent) => {
      if ((e.target as HTMLElement).closest?.(".ed-pane-head")) return;
      const p = nav().preset;
      if (e.button === 1) {
        e.preventDefault();
        drag = { kind: "pan", x: e.clientX, y: e.clientY, moved: false, button: 1 };
      } else if (e.button === 2) {
        if (p === "unreal") {
          drag = { kind: e.altKey ? "dolly" : "look", x: e.clientX, y: e.clientY, moved: false, button: 2 };
          if (!e.altKey) {
            useNav.getState().set({ looking: true });
            lock();
          }
        } else drag = { kind: "dolly", x: e.clientX, y: e.clientY, moved: false, button: 2 };
      } else if (e.button === 0) {
        if (p === "unreal" && e.altKey) drag = { kind: "orbit", x: e.clientX, y: e.clientY, moved: false, pivot: pivotNow(), button: 0 };
        // a plain left-drag is the tools' first; it moves the camera only if they let it go
        else drag = { kind: "pending-left", x: e.clientX, y: e.clientY, moved: false, button: 0 };
      }
    };
    const move = (e: PointerEvent) => {
      // Hammer mouse-look (Z): the mouse turns the camera with no button down
      if (!drag) {
        if (nav().looking && nav().preset === "hammer" && locked()) turn(e.movementX, e.movementY);
        return;
      }
      const dx = locked() ? e.movementX : e.clientX - drag.x;
      const dy = locked() ? e.movementY : e.clientY - drag.y;
      if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 3 && !locked()) return;
      if (drag.kind === "pending-left") {
        if (!ctl.enabled) {
          drag = null; // a tool took this drag
          return;
        }
        drag.kind = nav().preset === "hammer" ? "look" : "move";
      }
      if (!drag.moved && drag.kind !== "look" && !ctl.enabled) {
        drag = null;
        return;
      }
      drag.moved = true;
      if (!locked()) {
        drag.x = e.clientX;
        drag.y = e.clientY;
      }
      switch (drag.kind) {
        case "look":
          turn(dx, dy);
          break;
        case "move":
          turn(dx, 0);
          forward(-dy * 2 * stepScale());
          break;
        case "orbit":
          orbit(dx, dy, drag.pivot ?? ctl.target);
          break;
        case "dolly":
          forward(-dy * 2 * stepScale());
          break;
        case "pan":
          pan(dx, dy);
          break;
      }
    };
    const up = (e: PointerEvent) => {
      if (!drag || e.button !== drag.button) return;
      if (drag.kind === "look" && nav().preset === "unreal") {
        useNav.getState().set({ looking: false });
        keys.current.clear();
        unlock();
      }
      drag = null;
    };
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      if (nav().looking && nav().preset === "unreal") return stepSpeed(e.deltaY < 0 ? 1 : -1);
      forward(-Math.sign(e.deltaY) * Math.min(4, Math.abs(e.deltaY) / 50 + 0.5) * 48 * stepScale());
    };
    const menu = (e: MouseEvent) => e.preventDefault();
    const keyDown = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName))) return;
      if (nav().preset === "hammer" && e.code === "KeyZ" && navWantsKey(e) && useEditor.getState().view === "3d") {
        e.preventDefault();
        e.stopImmediatePropagation();
        const on = !nav().looking;
        useNav.getState().set({ looking: on });
        if (on) lock();
        else {
          unlock();
          keys.current.clear();
        }
        return;
      }
      if (nav().looking && e.key === "Escape") {
        useNav.getState().set({ looking: false });
        keys.current.clear();
        unlock();
        return;
      }
      if (nav().looking && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        e.stopImmediatePropagation();
        keys.current.add(e.code);
        invalidate();
      }
    };
    const keyUp = (e: KeyboardEvent) => keys.current.delete(e.code);
    const lockChange = () => {
      if (!locked() && nav().looking && nav().preset === "hammer") {
        useNav.getState().set({ looking: false });
        keys.current.clear();
      }
    };
    const blur = () => {
      keys.current.clear();
      if (nav().looking) useNav.getState().set({ looking: false });
    };
    el.addEventListener("pointerdown", down);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("contextmenu", menu);
    window.addEventListener("keydown", keyDown, true);
    window.addEventListener("keyup", keyUp, true);
    document.addEventListener("pointerlockchange", lockChange);
    window.addEventListener("blur", blur);
    return () => {
      el.removeEventListener("pointerdown", down);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("contextmenu", menu);
      window.removeEventListener("keydown", keyDown, true);
      window.removeEventListener("keyup", keyUp, true);
      document.removeEventListener("pointerlockchange", lockChange);
      window.removeEventListener("blur", blur);
      unlock();
      if (useNav.getState().looking) useNav.getState().set({ looking: false });
    };
  }, [camera, gl, connected, ctl, invalidate, preset]);

  // flying: velocity eases toward where the keys point (a short ramp up and a glide to stop)
  const fwd = useMemo(() => new THREE.Vector3(), []);
  const side = useMemo(() => new THREE.Vector3(), []);
  const want = useMemo(() => new THREE.Vector3(), []);
  useFrame((_, dt) => {
    const k = keys.current;
    const f = (k.has("KeyW") || k.has("ArrowUp") ? 1 : 0) - (k.has("KeyS") || k.has("ArrowDown") ? 1 : 0);
    const s = (k.has("KeyD") || k.has("ArrowRight") ? 1 : 0) - (k.has("KeyA") || k.has("ArrowLeft") ? 1 : 0);
    const u = (k.has("KeyE") ? 1 : 0) - (k.has("KeyQ") ? 1 : 0);
    const fast = k.has("ShiftLeft") || k.has("ShiftRight") ? 3 : 1;
    camera.getWorldDirection(fwd);
    side.crossVectors(fwd, camera.up).normalize();
    want.set(0, 0, 0).addScaledVector(fwd, f).addScaledVector(side, s).addScaledVector(camera.up, u);
    if (want.lengthSq() > 0) want.normalize().multiplyScalar(SPEEDS[useNav.getState().speed] * fast);
    const step = Math.min(dt, 0.1);
    vel.current.lerp(want, 1 - Math.exp(-step * 9));
    if (vel.current.lengthSq() < 1) {
      vel.current.set(0, 0, 0);
      return;
    }
    camera.position.addScaledVector(vel.current, step);
    ctl.target.addScaledVector(vel.current, step);
    invalidate();
  });
  return null;
}
