// P5: a pane's viewport inside the one canvas: drei's <View> (9.117, MIT) for a tracked box,
// with one fix. drei decides a view is "off screen" by comparing the box's place on the page
// with the canvas's size, which is only right when the canvas sits at the page's top-left: with
// a dock on the left or a short view the 3D pane counted as off screen and stayed blank. Here
// the box is compared with the canvas's own box. Only the tracked-element form is kept.

import { createPortal, useFrame, useThree, type RootState } from "@react-three/fiber";
import { useCallback, useEffect, useLayoutEffect, useReducer, useRef, useState, type MutableRefObject, type ReactNode } from "react";
import * as THREE from "three";
import { paneDrawn, paneInvalidate, paneNeedsDraw } from "./paneRedraw"; // Parity A

type Size = { width: number; height: number; top: number; left: number };

function position(canvas: Size, r: DOMRect) {
  const left = r.left - canvas.left;
  const top = r.top - canvas.top;
  const bottom = canvas.height - (top + r.height);
  const offscreen = top + r.height < 0 || top > canvas.height || left + r.width < 0 || left > canvas.width;
  return { pos: { width: r.width, height: r.height, left, bottom }, offscreen };
}

function scissor(state: RootState, p: { left: number; bottom: number; width: number; height: number }) {
  const cam = state.camera as THREE.PerspectiveCamera | THREE.OrthographicCamera;
  if ((cam as THREE.OrthographicCamera).isOrthographicCamera) {
    const o = cam as THREE.OrthographicCamera;
    if (o.left !== p.width / -2 || o.right !== p.width / 2 || o.top !== p.height / 2 || o.bottom !== p.height / -2) {
      Object.assign(o, { left: p.width / -2, right: p.width / 2, top: p.height / 2, bottom: p.height / -2 });
      o.updateProjectionMatrix();
    }
  } else {
    const pc = cam as THREE.PerspectiveCamera;
    const aspect = p.width / Math.max(1, p.height);
    if (pc.aspect !== aspect) {
      pc.aspect = aspect;
      pc.updateProjectionMatrix();
    }
  }
  const auto = state.gl.autoClear;
  state.gl.autoClear = false;
  state.gl.setViewport(p.left, p.bottom, p.width, p.height);
  state.gl.setScissor(p.left, p.bottom, p.width, p.height);
  state.gl.setScissorTest(true);
  return auto;
}

function Container({ visible, canvasSize, track, rect, index, children }: { visible: boolean; canvasSize: Size; track: MutableRefObject<HTMLElement | null>; rect: MutableRefObject<DOMRect | null>; index: number; children: ReactNode }) {
  const root = useThree();
  const [off, setOff] = useState(false);
  useFrame((state) => {
    if (track.current) rect.current = track.current.getBoundingClientRect();
    if (!rect.current) return;
    const canvas = state.gl.domElement.getBoundingClientRect();
    const { pos, offscreen } = position({ width: canvas.width, height: canvas.height, top: canvas.top, left: canvas.left }, rect.current);
    if (offscreen !== off) setOff(offscreen);
    // Parity A: only a pane that changed draws (paneRedraw.ts)
    if (visible && !offscreen && pos.width > 0 && pos.height > 0 && paneNeedsDraw(index, pos)) {
      paneDrawn(index, pos);
      const auto = scissor(state, pos);
      state.gl.render(state.scene, state.camera);
      state.gl.setScissorTest(false);
      state.gl.autoClear = auto;
    }
  }, index);
  // a hidden view is cleared once and then left alone
  useLayoutEffect(() => {
    const r = rect.current;
    if (!r || (visible && !off)) return;
    const canvas = root.gl.domElement.getBoundingClientRect();
    const { pos } = position({ width: canvas.width, height: canvas.height, top: canvas.top, left: canvas.left }, r);
    const auto = scissor(root, pos);
    root.gl.clear(true, true);
    root.gl.setScissorTest(false);
    root.gl.autoClear = auto;
  }, [visible, off]); // eslint-disable-line react-hooks/exhaustive-deps
  // the view's events come from its box
  useEffect(() => {
    if (!track.current) return;
    const old = root.get().events.connected;
    root.setEvents({ connected: track.current });
    return () => root.setEvents({ connected: old });
  }, [track]); // eslint-disable-line react-hooks/exhaustive-deps
  void canvasSize;
  return (
    <>
      {children}
      <group onPointerOver={() => null} />
    </>
  );
}

/** A view drawn into `track`'s box on the shared canvas, with its own scene and camera. */
export function PaneView({ track, index = 1, visible = true, children }: { track: MutableRefObject<HTMLElement | null>; index?: number; visible?: boolean; children: ReactNode }) {
  const rect = useRef<DOMRect | null>(null);
  const size = useThree((s) => s.size);
  const rootInvalidate = useThree((s) => s.invalidate);
  const [scene] = useState(() => new THREE.Scene());
  const [ready, done] = useReducer(() => true, false);
  const compute = useCallback(
    (event: Event, state: RootState) => {
      const e = event as PointerEvent;
      if (rect.current && track.current && e.target === track.current) {
        const { width, height, left, top } = rect.current;
        state.pointer.set(((e.clientX - left) / width) * 2 - 1, -((e.clientY - top) / height) * 2 + 1);
        state.raycaster.setFromCamera(state.pointer, state.camera);
      }
    },
    [track],
  );
  useEffect(() => {
    if (track.current) rect.current = track.current.getBoundingClientRect();
    done();
  }, [track]);
  if (!ready) return null;
  const r = rect.current;
  return createPortal(
    <Container visible={visible} canvasSize={size as Size} track={track} rect={rect} index={index}>
      {children}
    </Container>,
    scene,
    // (R3F's types leave invalidate out of a portal's state, but the portal takes it: Parity A)
    { invalidate: paneInvalidate(index, rootInvalidate), events: { compute, priority: index }, size: { width: r?.width ?? size.width, height: r?.height ?? size.height, top: r?.top ?? 0, left: r?.left ?? 0 } } as never,
  );
}
