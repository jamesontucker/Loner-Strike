// The three ways to look at a map:
//  Orbit    drag to turn around a point, right-drag or two fingers to pan, scroll to zoom
//  Fly      click to grab the mouse, WASD to move (Shift faster), Esc lets go
//  Top-down a flat map view, north up (like the Python tools' pictures)
// F / O / T switch between them (MapViewer).

import { MapControls, OrbitControls, OrthographicCamera } from "@react-three/drei";
import { useFrame, useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo, useRef, type ElementRef } from "react";
import * as THREE from "three";
import { useTools, type CameraMode } from "../../../stores/tools";
import { anglesOfThreeDir, gsToThree, threeToGs, type Vec3 } from "./coords";

const FLY_SPEED = 420;
const FLY_FAST = 3;
const LOOK = 0.0022;

/** Fly mode: whether the press that's going on only grabbed the mouse (not a pick). */
const fly = { grabbing: false };
/** Whether a click shouldn't pick: it was a drag, or it grabbed the mouse for flying. */
export const notAPick = (e: ThreeEvent<MouseEvent>) => e.delta > 4 || (useTools.getState().camera === "fly" && fly.grabbing);

type Home = { centre: THREE.Vector3; radius: number; bounds: [Vec3, Vec3] };

export function Cameras({ bounds }: { bounds: [Vec3, Vec3] }) {
  const mode = useTools((s) => s.camera);
  const camera = useThree((s) => s.camera);
  const invalidate = useThree((s) => s.invalidate);
  const home = useMemo<Home>(() => {
    const a = gsToThree(bounds[0]);
    const b = gsToThree(bounds[1]);
    const centre = a.clone().add(b).multiplyScalar(0.5);
    return { centre, radius: Math.max(256, a.distanceTo(b) / 2), bounds };
  }, [bounds]);

  // a new map: the perspective camera starts above one corner, looking at the middle
  const persp = useRef<THREE.PerspectiveCamera | null>(null);
  if (!persp.current && camera instanceof THREE.PerspectiveCamera) persp.current = camera;
  const placed = useRef<Home | null>(null);
  if (persp.current && placed.current !== home) {
    placed.current = home;
    const c = persp.current;
    c.near = 2;
    c.far = Math.max(20000, home.radius * 8);
    c.position.copy(home.centre).add(new THREE.Vector3(home.radius * 0.55, home.radius * 0.45, home.radius * 0.55));
    c.lookAt(home.centre);
    c.updateProjectionMatrix();
  }

  useEffect(() => invalidate(), [mode, invalidate]);
  return (
    <>
      {mode === "orbit" && <Orbit home={home} />}
      {mode === "fly" && <Fly />}
      {mode === "top" && <Top home={home} />}
      <LookAt mode={mode} />
      <Status />
    </>
  );
}

/** A point in front of the camera, for orbiting around what you were looking at. */
function aimPoint(cam: THREE.Camera, dist: number) {
  return cam.position.clone().add(cam.getWorldDirection(new THREE.Vector3()).multiplyScalar(dist));
}

function Orbit({ home }: { home: Home }) {
  const ref = useRef<ElementRef<typeof OrbitControls>>(null);
  const camera = useThree((s) => s.camera);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    // around the middle of the map when the camera is looking at it, else what's ahead
    const toCentre = home.centre.clone().sub(camera.position);
    const ahead = camera.getWorldDirection(new THREE.Vector3());
    const d = toCentre.length();
    c.target.copy(toCentre.normalize().dot(ahead) > 0.98 ? home.centre : aimPoint(camera, Math.min(d, 512)));
    c.update();
  }, [home, camera]);
  return <OrbitControls ref={ref} makeDefault enableDamping dampingFactor={0.18} zoomSpeed={1.2} maxDistance={home.radius * 6} />;
}

function Fly() {
  const camera = useThree((s) => s.camera);
  const gl = useThree((s) => s.gl);
  const invalidate = useThree((s) => s.invalidate);
  const keys = useRef(new Set<string>());
  const euler = useRef(new THREE.Euler(0, 0, 0, "YXZ"));

  useEffect(() => {
    euler.current.setFromQuaternion(camera.quaternion, "YXZ");
    euler.current.z = 0;
    const el = gl.domElement;
    const { setLooking } = useTools.getState();
    let dragging = false;
    const locked = () => document.pointerLockElement === el;
    const turn = (dx: number, dy: number) => {
      const e = euler.current;
      e.y -= dx * LOOK;
      e.x = Math.max(-Math.PI / 2 + 0.01, Math.min(Math.PI / 2 - 0.01, e.x - dy * LOOK));
      camera.quaternion.setFromEuler(e);
      invalidate();
    };
    const down = (e: PointerEvent) => {
      if (e.button !== 0) return;
      fly.grabbing = !locked();
      if (locked()) return;
      dragging = true;
      // pointer lock where the system allows it; otherwise dragging turns the view
      try {
        const p = el.requestPointerLock() as unknown as Promise<void> | undefined;
        p?.catch?.(() => {});
      } catch {
        /* drag instead */
      }
    };
    const up = () => (dragging = false);
    const move = (e: PointerEvent) => {
      if (locked() || dragging) turn(e.movementX, e.movementY);
    };
    const lockChange = () => {
      setLooking(locked());
      if (!locked()) keys.current.clear();
    };
    const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));
    const keyDown = (e: KeyboardEvent) => {
      if (isField(e.target) || e.metaKey || e.ctrlKey || e.altKey) return;
      if (["KeyW", "KeyA", "KeyS", "KeyD", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "ShiftLeft", "ShiftRight"].includes(e.code)) {
        keys.current.add(e.code);
        if (e.code.startsWith("Arrow")) e.preventDefault();
        invalidate();
      }
    };
    const keyUp = (e: KeyboardEvent) => keys.current.delete(e.code);
    const blur = () => keys.current.clear();
    el.addEventListener("pointerdown", down);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointermove", move);
    document.addEventListener("pointerlockchange", lockChange);
    window.addEventListener("keydown", keyDown);
    window.addEventListener("keyup", keyUp);
    window.addEventListener("blur", blur);
    return () => {
      el.removeEventListener("pointerdown", down);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointermove", move);
      document.removeEventListener("pointerlockchange", lockChange);
      window.removeEventListener("keydown", keyDown);
      window.removeEventListener("keyup", keyUp);
      window.removeEventListener("blur", blur);
      if (locked()) document.exitPointerLock();
      setLooking(false);
      fly.grabbing = false;
    };
  }, [camera, gl, invalidate]);

  const fwd = useMemo(() => new THREE.Vector3(), []);
  const side = useMemo(() => new THREE.Vector3(), []);
  useFrame((_, dt) => {
    const k = keys.current;
    const f = (k.has("KeyW") || k.has("ArrowUp") ? 1 : 0) - (k.has("KeyS") || k.has("ArrowDown") ? 1 : 0);
    const s = (k.has("KeyD") || k.has("ArrowRight") ? 1 : 0) - (k.has("KeyA") || k.has("ArrowLeft") ? 1 : 0);
    if (!f && !s) return;
    const speed = FLY_SPEED * (k.has("ShiftLeft") || k.has("ShiftRight") ? FLY_FAST : 1) * Math.min(dt, 0.1);
    camera.getWorldDirection(fwd);
    side.crossVectors(fwd, camera.up).normalize();
    camera.position.addScaledVector(fwd, f * speed).addScaledVector(side, s * speed);
    invalidate();
  });
  return null;
}

function Top({ home }: { home: Home }) {
  const size = useThree((s) => s.size);
  const camera = useThree((s) => s.camera);
  const ref = useRef<ElementRef<typeof MapControls>>(null);
  const [lo, hi] = home.bounds;
  // fit the whole map in view
  const zoom = Math.min(size.width / Math.max(64, hi[0] - lo[0]), size.height / Math.max(64, hi[1] - lo[1])) * 0.9;
  const height = hi[2] + 4096;
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    c.target.set(home.centre.x, lo[2], home.centre.z);
    c.object.position.set(home.centre.x, height, home.centre.z);
    c.update();
    // (only on opening the view: resizing keeps where you are)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [home, camera]);
  return (
    <>
      <OrthographicCamera makeDefault position={[home.centre.x, height, home.centre.z]} zoom={zoom} near={1} far={height - lo[2] + 8192} />
      {/* (made once the flat camera is the one in use, so the controls move it) */}
      {camera instanceof THREE.OrthographicCamera && <MapControls ref={ref} makeDefault enableRotate={false} screenSpacePanning enableDamping={false} minPolarAngle={0} maxPolarAngle={0} minZoom={zoom / 8} maxZoom={zoom * 40} />}
    </>
  );
}

/** Moves the camera to a point when something asks (Inspector's Look at, or another tool
 *  opening the viewer on a spot before the map was up), then clears the request. */
function LookAt({ mode }: { mode: CameraMode }) {
  const req = useTools((s) => s.lookAt);
  const camera = useThree((s) => s.camera);
  const controls = useThree((s) => s.controls) as unknown as { target: THREE.Vector3; update: () => void } | null;
  const invalidate = useThree((s) => s.invalidate);
  useEffect(() => {
    if (!req) return;
    // Orbit and Top-down move their controls' target too: wait until those are up
    if (mode !== "fly" && !controls) return;
    useTools.setState({ lookAt: null });
    const p = gsToThree(req.point);
    const dist = Math.max(160, req.size * 2.5);
    if (mode === "top") {
      if (controls) {
        const dy = camera.position.y - controls.target.y;
        controls.target.set(p.x, controls.target.y, p.z);
        camera.position.set(p.x, controls.target.y + dy, p.z);
        controls.update();
      }
    } else {
      // keep the direction you're looking from, a little above
      const from = camera.position.clone().sub(controls?.target && mode === "orbit" ? controls.target : p);
      if (from.lengthSq() < 1) from.set(1, 0.6, 1);
      from.normalize();
      from.y = Math.max(from.y, 0.35);
      camera.position.copy(p).addScaledVector(from.normalize(), dist);
      camera.lookAt(p);
      if (mode === "orbit" && controls) {
        controls.target.copy(p);
        controls.update();
      }
    }
    invalidate();
  }, [req, mode, camera, controls, invalidate]);
  return null;
}

/** Puts the camera's place and heading in the status bar (only when the whole numbers change). */
function Status() {
  const dir = useMemo(() => new THREE.Vector3(), []);
  useFrame(({ camera }) => {
    const { yaw, pitch } = anglesOfThreeDir(camera.getWorldDirection(dir));
    const pos = threeToGs(camera.position);
    const s = useTools.getState().status;
    if (s && s.pos.every((v, i) => Math.round(v) === Math.round(pos[i])) && Math.round(s.yaw) === Math.round(yaw) && Math.round(s.pitch) === Math.round(pitch)) return;
    useTools.getState().setStatus({ pos, yaw, pitch });
  });
  return null;
}
