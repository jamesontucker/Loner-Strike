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
import { useP5 } from "./p5store";
import { usePanes } from "./panes";
import { flySpeed, stepSpeed, useNav, type NavPreset } from "./navigation";

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

/** Hammer: the left button is down on empty 3D space (a camera hold if a move key comes) */
let hammerLeftDown = false;
/** Hammer: this left press became a camera hold (so letting go doesn't also pick what's under it) */
let hammerHeld = false;
export const navTookPress = () => hammerHeld;
/** The pane whose camera has the look now (with two 3D panes, only its keys fly) */
let lookOwner: HTMLElement | null = null;

/** True when the key belongs to the camera right now (Shortcuts.ts leaves it alone). */
/** Z can start a look: the active pane is 3D, or the mouse is over a 3D pane (Quad with a 2D
 *  pane active: no click needed first) */
function zLookHere() {
  if (useEditor.getState().view === "3d") return true;
  const ps = usePanes.getState();
  return ps.hover != null && ps.views[ps.hover] === "3d";
}

export function navWantsKey(e: KeyboardEvent): boolean {
  const n = useNav.getState();
  if (hammerLeftDown && n.preset === "hammer" && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) return true;
  if (n.looking && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) return true;
  if (n.preset === "hammer" && e.code === "KeyZ" && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && (n.looking || zLookHere())) return true;
  return false;
}

export function NavCamera({ bounds, preset }: { bounds: [Vec3, Vec3]; preset: Exclude<NavPreset, "blender"> }) {
  const camera = useThree((s) => s.camera) as THREE.PerspectiveCamera;
  const gl = useThree((s) => s.gl);
  const set = useThree((s) => s.set);
  const invalidateNow = useThree((s) => s.invalidate);
  // (through a ref: the mouse/key setup below must not start over when the editor redraws)
  const inval = useRef(invalidateNow);
  inval.current = invalidateNow;
  const invalidate = useMemo(() => (frames?: number) => inval.current(frames), []);
  const connected = useThree((s) => s.events.connected) as HTMLElement | undefined;
  const el = connected?.dataset?.pane != null ? connected : gl.domElement;
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
    // big maps: faster flying, and a near plane that keeps depth sharp far away
    const extent = Math.max(...bounds[1].map((x, i) => x - bounds[0][i]));
    useNav.getState().set({ mapScale: Math.max(1, Math.min(4, extent / 3000)) });
    camera.near = Math.max(2, Math.min(8, r / 1500));
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
    const stepScale = () => flySpeed() / 512;

    // held: a Hammer left-drag look that hands WASD to the camera until the button comes up (like J.A.C.K.)
    type Drag = { kind: "look" | "move" | "orbit" | "dolly" | "pan" | "pending-left"; x: number; y: number; moved: boolean; pivot?: THREE.Vector3; button: number; held?: boolean };
    let drag: Drag | null = null;
    const locked = () => document.pointerLockElement === el;
    // the system said no to hiding the mouse (or has no pointer lock): Z look turns with the
    // visible mouse instead
    let lockDenied = false;
    const lock = () => {
      try {
        const p = el.requestPointerLock() as unknown as Promise<void> | undefined;
        p?.catch?.(() => (lockDenied = true));
      } catch {
        lockDenied = true; // drag without hiding the pointer
      }
    };
    const lockError = () => (lockDenied = true);
    const unlock = () => locked() && document.exitPointerLock();
    /** this pane has the look (or nobody is looking) */
    const mine = () => !nav().looking || lookOwner === el;
    const startLook = (p: Parameters<ReturnType<typeof useNav.getState>["set"]>[0]) => {
      lookOwner = el;
      const slot = el.dataset.pane;
      useNav.getState().set({ looking: true, pane: slot == null ? null : Number(slot), ...p });
    };
    /** Z goes to the 3D pane under the mouse, else the active one */
    const zPane = () => {
      const ps = usePanes.getState();
      const slot = el.dataset.pane;
      if (slot == null) return true;
      const want = ps.hover != null && ps.views[ps.hover] === "3d" ? ps.hover : ps.active;
      return Number(slot) === want;
    };
    const pivotNow = () => {
      const b = selectionBounds();
      if (b) return gsToThree(b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3);
      return ctl.target.clone();
    };

    // Hammer: while the left button turns the camera, WASD/QE fly too (Z mouse-look stays as it was)
    const holdLook = (d: Drag) => {
      if (nav().looking) return;
      d.held = true;
      hammerHeld = true;
      startLook({ held: true });
      (document.activeElement as HTMLElement | null)?.blur?.();
    };
    const down = (e: PointerEvent) => {
      if ((e.target as HTMLElement).closest?.(".ed-pane-head")) return;
      // another button while one drags: the drag keeps going (and ends on its own button)
      if (drag) {
        if (e.buttons & ([1, 4, 2][drag.button] ?? 0)) return;
        drag = null; // its button came up without us seeing it
      }
      if (e.button === 0) hammerHeld = false;
      const p = nav().preset;
      if (e.button === 1) {
        e.preventDefault();
        drag = { kind: "pan", x: e.clientX, y: e.clientY, moved: false, button: 1 };
      } else if (e.button === 2) {
        if (p === "unreal") {
          drag = { kind: e.altKey ? "dolly" : "look", x: e.clientX, y: e.clientY, moved: false, button: 2 };
          if (!e.altKey) {
            startLook({});
            // the keys come here now, wherever focus was
            (document.activeElement as HTMLElement | null)?.blur?.();
            lock();
          }
        } else drag = { kind: "dolly", x: e.clientX, y: e.clientY, moved: false, button: 2 };
      } else if (e.button === 0) {
        if (p === "unreal" && e.altKey) drag = { kind: "orbit", x: e.clientX, y: e.clientY, moved: false, pivot: pivotNow(), button: 0 };
        // a plain left-drag is the tools' first; it moves the camera only if they let it go
        else {
          drag = { kind: "pending-left", x: e.clientX, y: e.clientY, moved: false, button: 0 };
          hammerLeftDown = p === "hammer";
        }
      }
    };
    const move = (e: PointerEvent) => {
      // Hammer mouse-look (Z): the mouse turns the camera with no button down
      if (!drag) {
        if (nav().looking && lookOwner === el && nav().preset === "hammer" && (locked() || (lockDenied && !nav().released))) turn(e.movementX, e.movementY);
        return;
      }
      const dx = locked() ? e.movementX : e.clientX - drag.x;
      const dy = locked() ? e.movementY : e.clientY - drag.y;
      if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 3 && !locked()) return;
      if (drag.kind === "pending-left") {
        if (!ctl.enabled) {
          hammerLeftDown = false;
          drag = null; // a tool took this drag
          return;
        }
        drag.kind = nav().preset === "hammer" ? "look" : "move";
        if (drag.kind === "look") holdLook(drag);
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
      if (e.button === 0) hammerLeftDown = false;
      if (!drag || e.button !== drag.button) return;
      if (drag.held) {
        useNav.getState().set({ looking: false, held: false });
        keys.current.clear();
      }
      if (drag.kind === "look" && nav().preset === "unreal") {
        useNav.getState().set({ looking: false });
        keys.current.clear();
        unlock();
      }
      drag = null;
    };
    const cancel = () => {
      if (drag?.held || (drag?.kind === "look" && nav().preset === "unreal")) {
        useNav.getState().set({ looking: false, held: false });
        keys.current.clear();
        unlock();
      }
      drag = null;
      hammerLeftDown = false;
    };
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      if (nav().looking && nav().preset === "unreal") return stepSpeed(e.deltaY < 0 ? 1 : -1);
      forward(-Math.sign(e.deltaY) * Math.min(4, Math.abs(e.deltaY) / 50 + 0.5) * 48 * stepScale());
    };
    const menu = (e: MouseEvent) => e.preventDefault();
    const keyDown = (e: KeyboardEvent) => {
      // another 3D pane has the look: its camera takes the keys
      if (!mine()) return;
      const t = e.target as HTMLElement | null;
      const field = !!t && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));
      // a text box keeps its keys unless the mouse is grabbed (a released or no-lock look leaves
      // the mouse free to click into one)
      if (field && (!nav().looking || !locked())) return;
      // a dialog or sheet is up: no new look
      if (!nav().looking && (useEditor.getState().dialog || useP5.getState().dialog || document.querySelector(".sheet-backdrop"))) return;
      if (nav().preset === "hammer" && e.code === "KeyZ" && navWantsKey(e)) {
        const on = !nav().looking;
        if (on && !zPane()) return; // (the other 3D pane's camera takes it)
        e.preventDefault();
        e.stopImmediatePropagation();
        if (on) startLook({ held: false, released: false });
        else useNav.getState().set({ looking: false, held: false, released: false });
        if (on) lock();
        else {
          unlock();
          keys.current.clear();
        }
        return;
      }
      if (nav().looking && e.key === "Escape") {
        // (only stops the look: it doesn't also clear the selection)
        e.preventDefault();
        e.stopImmediatePropagation();
        useNav.getState().set({ looking: false, released: false });
        keys.current.clear();
        unlock();
        return;
      }
      // Hammer: left button held still on empty space, then a move key: that's a camera hold too
      if (drag?.kind === "pending-left" && hammerLeftDown && ctl.enabled && nav().preset === "hammer" && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) {
        drag.kind = "look";
        holdLook(drag);
      }
      if (nav().looking && MOVE_KEYS.includes(e.code) && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        e.stopImmediatePropagation();
        // (a held key's auto-repeat doesn't count: Esc with W down let go of the mouse on purpose)
        if (nav().released && !e.repeat) relock();
        keys.current.add(e.code);
        if (!nav().hintDone && /^Key[WASDQE]$/.test(e.code)) useNav.getState().set({ hintDone: true });
        invalidate();
      }
    };
    const keyUp = (e: KeyboardEvent) => keys.current.delete(e.code);
    // Z look lost the mouse without Z or Esc. With a move key down (Windows does this now and
    // then, mid-flight) it keeps flying and takes the mouse back on the next move key or click.
    // With none down it was Esc (Chromium keeps that Esc for itself): stop.
    const lockChange = () => {
      if (!mine()) return;
      if (locked()) {
        lockDenied = false;
        if (nav().released) useNav.getState().set({ released: false });
      } else if (nav().looking && nav().preset === "hammer" && !nav().held) {
        if (keys.current.size > 0) useNav.getState().set({ released: true });
        else useNav.getState().set({ looking: false, released: false });
      }
    };
    const relock = () => {
      useNav.getState().set({ released: false });
      lock();
    };
    // a click while released only takes the mouse back (it doesn't also pick)
    const downFirst = (e: PointerEvent) => {
      if (!nav().looking || lookOwner !== el || !nav().released || nav().preset !== "hammer") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      hammerHeld = true; // and letting go doesn't pick either
      relock();
    };
    // a press outside the pane while the mouse is free (released, or no pointer lock) ends the look
    const downElsewhere = (e: PointerEvent) => {
      if (!nav().looking || lookOwner !== el || locked() || drag) return;
      if (e.target instanceof Node && el.contains(e.target)) return;
      useNav.getState().set({ looking: false, held: false, released: false });
      keys.current.clear();
    };
    const blur = () => {
      // (a button let go in another window never comes back as a pointerup here)
      drag = null;
      hammerLeftDown = false;
      keys.current.clear();
      if (nav().looking) useNav.getState().set({ looking: false, held: false, released: false });
    };
    el.addEventListener("pointerdown", downFirst, true);
    el.addEventListener("pointerdown", down);
    window.addEventListener("pointerdown", downElsewhere, true);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    el.addEventListener("wheel", wheel, { passive: false });
    el.addEventListener("contextmenu", menu);
    window.addEventListener("keydown", keyDown, true);
    window.addEventListener("keyup", keyUp, true);
    document.addEventListener("pointerlockchange", lockChange);
    document.addEventListener("pointerlockerror", lockError);
    window.addEventListener("blur", blur);
    return () => {
      el.removeEventListener("pointerdown", downFirst, true);
      el.removeEventListener("pointerdown", down);
      window.removeEventListener("pointerdown", downElsewhere, true);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      el.removeEventListener("wheel", wheel);
      el.removeEventListener("contextmenu", menu);
      window.removeEventListener("keydown", keyDown, true);
      window.removeEventListener("keyup", keyUp, true);
      document.removeEventListener("pointerlockchange", lockChange);
      document.removeEventListener("pointerlockerror", lockError);
      window.removeEventListener("blur", blur);
      unlock();
      if (useNav.getState().looking && lookOwner === el) useNav.getState().set({ looking: false, held: false, released: false });
    };
  }, [camera, el, ctl, invalidate, preset]);

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
    if (want.lengthSq() > 0) want.normalize().multiplyScalar(flySpeed() * fast);
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
