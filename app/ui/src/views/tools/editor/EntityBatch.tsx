// Parity A: point entities drawn in a handful of draw calls instead of four or five each (br_valley:
// ~1200 draws per pane down to ~10). One instanced box mesh (a colour per entity), one set of
// outlines, one set of facing arrows, one point cloud for sprite glows, and one instanced mesh
// per studio model (model + body + skin). Picking still finds each entity: the instanced boxes
// hand back a small stand-in object per instance carrying `{ kind: "entity", id }`, as the
// one-mesh-per-entity boxes did, so the tools, the box select and the drop targets need no change.
// Entities that are picked are still drawn one by one (EditorScene's EntityBox), above these.

import { useThree } from "@react-three/fiber";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity } from "./editorApi";
import { loadModel, modelGeometry, modelMaterials, modelOf, type Got } from "./EntityModels";
import { useKits } from "./kits";
import { useP5 } from "./p5store";
import { keyOf, originOf } from "./store";

type Look = { size: Vec3; colour: string; up: number };

const UNIT_BOX = new THREE.BoxGeometry(1, 1, 1);
const BOX_EDGES: number[] = (() => {
  const e = new THREE.EdgesGeometry(UNIT_BOX).getAttribute("position");
  return Array.from(e.array as Float32Array);
})();

/** An instanced mesh whose hits name the entity behind each instance. */
function pickable(mesh: THREE.InstancedMesh, ids: number[]) {
  const stand = ids.map((id) => {
    const o = new THREE.Object3D();
    o.userData = { kind: "entity", id };
    // events bubble from the stand-in through the real mesh to the tool layer's handlers
    (o as { parent: THREE.Object3D | null }).parent = mesh;
    return o;
  });
  mesh.raycast = (rc, out) => {
    const hits: THREE.Intersection[] = [];
    THREE.InstancedMesh.prototype.raycast.call(mesh, rc, hits);
    for (const h of hits) if (h.instanceId != null && stand[h.instanceId]) out.push({ ...h, object: stand[h.instanceId] });
  };
}

const GLOW_VERT = /* glsl */ `
  attribute float psize;
  attribute vec3 tint;
  uniform float uViewH;
  varying vec3 vTint;
  void main() {
    vec4 mv = modelViewMatrix * vec4(position, 1.0);
    gl_Position = projectionMatrix * mv;
    float s = psize * projectionMatrix[1][1] * uViewH * 0.5;
    // perspective: smaller with distance (an orthographic projection has [3][3] = 1)
    if (projectionMatrix[3][3] < 0.5) s /= max(1.0, -mv.z);
    gl_PointSize = clamp(s, 1.0, 1024.0);
    vTint = tint;
  }
`;
const GLOW_FRAG = /* glsl */ `
  varying vec3 vTint;
  void main() {
    float d = length(gl_PointCoord - 0.5) * 2.0;
    if (d > 1.0) discard;
    // the old canvas gradient: 1 at the middle, 0.5 at 40%, 0 at the edge
    float a = d < 0.4 ? mix(1.0, 0.5, d / 0.4) : mix(0.5, 0.0, (d - 0.4) / 0.6);
    gl_FragColor = vec4(vTint * a, a);
  }
`;

type ModelGroup = { key: string; got: NonNullable<Got>; body: number; skin: number; mats: THREE.Matrix4[] };

/** Every unpicked point entity in a few draws. `look`/`yaw`: EditorScene's box size, colour and facing. */
export function EntityBatch({ ents, look, yaw }: { ents: DocEntity[]; look: (e: DocEntity) => Look; yaw: (e: DocEntity) => number | null }) {
  const invalidate = useThree((s) => s.invalidate);
  const showModels = useP5((s) => s.showModels);
  const creatures = useKits((k) => k.creatures);
  useKits((k) => k.kits); // markers take their kit's colour when the kits arrive

  // models: load each path once; entities wait as boxes until theirs arrives
  const paths = useMemo(() => {
    const m = new Map<number, string>();
    if (showModels) for (const e of ents) {
      const p = modelOf(e, creatures);
      if (p) m.set(e.id, p);
    }
    return m;
  }, [ents, showModels, creatures]);
  const [loaded, setLoaded] = useState<Map<string, Got>>(() => new Map());
  useEffect(() => {
    let live = true;
    for (const p of new Set(paths.values())) {
      if (loaded.has(p)) continue;
      void loadModel(p).then((g) => {
        if (!live) return;
        setLoaded((old) => (old.has(p) ? old : new Map(old).set(p, g)));
      });
    }
    return () => {
      live = false;
    };
  }, [paths]); // eslint-disable-line react-hooks/exhaustive-deps

  const built = useMemo(() => {
    const boxIds: number[] = [];
    const boxMats: THREE.Matrix4[] = [];
    const boxCols: THREE.Color[] = [];
    const pickIds: number[] = []; // entities shown as a model: their box only picks
    const pickMats: THREE.Matrix4[] = [];
    const edges: number[] = [];
    const arrows: number[] = [];
    const arrowCols: number[] = [];
    const glowPos: number[] = [];
    const glowSize: number[] = [];
    const glowTint: number[] = [];
    const models = new Map<string, ModelGroup>();
    const c = new THREE.Color();
    for (const e of ents) {
      const o = originOf(e.keys);
      if (!o) continue;
      const lk = look(e);
      const [sx, sy, sz] = lk.size;
      const m = new THREE.Matrix4().makeScale(sx, sy, sz).setPosition(o[0], o[1], o[2]);
      const path = paths.get(e.id);
      const got = path ? loaded.get(path) : null;
      const y = yaw(e);
      if (got) {
        pickIds.push(e.id);
        pickMats.push(m);
        const body = Number(keyOf(e.keys, "body") ?? 0) || 0;
        const skin = Number(keyOf(e.keys, "skin") ?? 0) || 0;
        const key = `${path}|${body}|${skin}`;
        let g = models.get(key);
        if (!g) models.set(key, (g = { key, got, body, skin, mats: [] }));
        g.mats.push(new THREE.Matrix4().makeRotationZ(((y ?? 0) * Math.PI) / 180).setPosition(o[0], o[1], o[2]));
      } else {
        boxIds.push(e.id);
        boxMats.push(m);
        boxCols.push(new THREE.Color(lk.colour));
      }
      for (let i = 0; i < BOX_EDGES.length; i += 3) edges.push(o[0] + BOX_EDGES[i] * sx, o[1] + BOX_EDGES[i + 1] * sy, o[2] + BOX_EDGES[i + 2] * sz);
      if (y != null) {
        const len = Math.max(sx, 24);
        const t = (y * Math.PI) / 180;
        arrows.push(o[0], o[1], o[2], o[0] + Math.cos(t) * len, o[1] + Math.sin(t) * len, o[2]);
        c.set(lk.colour);
        arrowCols.push(c.r, c.g, c.b, c.r, c.g, c.b);
      }
      if (showModels && /^env_(sprite|glow)$/.test(e.classname)) {
        const rgb = (keyOf(e.keys, "rendercolor") ?? "255 255 255").split(/\s+/).map(Number);
        const ok = rgb.length >= 3 && rgb.slice(0, 3).every(Number.isFinite) && rgb.some((x) => x > 0);
        const scale = Math.max(0.1, Math.min(8, Number(keyOf(e.keys, "scale") ?? 1) || 1));
        glowPos.push(o[0], o[1], o[2]);
        glowSize.push(32 * scale);
        glowTint.push(...(ok ? [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255] : [1, 1, 1]));
      }
    }
    return { boxIds, boxMats, boxCols, pickIds, pickMats, edges, arrows, arrowCols, glowPos, glowSize, glowTint, models: [...models.values()] };
  }, [ents, look, yaw, paths, loaded, showModels]);

  const boxes = useMemo(() => {
    if (!built.boxIds.length) return null;
    const mesh = new THREE.InstancedMesh(UNIT_BOX, new THREE.MeshLambertMaterial({ transparent: true, opacity: 0.85 }), built.boxIds.length);
    built.boxMats.forEach((m, i) => mesh.setMatrixAt(i, m));
    built.boxCols.forEach((col, i) => mesh.setColorAt(i, col));
    mesh.computeBoundingSphere();
    pickable(mesh, built.boxIds);
    return mesh;
  }, [built]);
  const picks = useMemo(() => {
    if (!built.pickIds.length) return null;
    const mesh = new THREE.InstancedMesh(UNIT_BOX, new THREE.MeshBasicMaterial(), built.pickIds.length);
    built.pickMats.forEach((m, i) => mesh.setMatrixAt(i, m));
    mesh.computeBoundingSphere();
    mesh.visible = false; // picks, never drawn (rays still find invisible meshes)
    pickable(mesh, built.pickIds);
    return mesh;
  }, [built]);
  const lines = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(built.edges, 3));
    return g;
  }, [built]);
  const arrows = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(built.arrows, 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(built.arrowCols, 3));
    return g;
  }, [built]);
  const glow = useMemo(() => {
    if (!built.glowSize.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(built.glowPos, 3));
    g.setAttribute("psize", new THREE.Float32BufferAttribute(built.glowSize, 1));
    g.setAttribute("tint", new THREE.Float32BufferAttribute(built.glowTint, 3));
    const mat = new THREE.ShaderMaterial({ vertexShader: GLOW_VERT, fragmentShader: GLOW_FRAG, uniforms: { uViewH: { value: 600 } }, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending });
    const pts = new THREE.Points(g, mat);
    pts.renderOrder = 4;
    pts.raycast = () => null;
    const v = new THREE.Vector4();
    pts.onBeforeRender = (r) => {
      r.getCurrentViewport(v);
      mat.uniforms.uViewH.value = v.w;
    };
    return pts;
  }, [built]);
  const models = useMemo(
    () =>
      built.models.map((g) => {
        const geo = modelGeometry(g.got.bundle, g.body);
        const mats = modelMaterials(g.got.bundle, g.got.item, g.skin, () => invalidate());
        const mesh = new THREE.InstancedMesh(geo, mats, g.mats.length);
        g.mats.forEach((m, i) => mesh.setMatrixAt(i, m));
        mesh.computeBoundingSphere();
        mesh.raycast = () => null; // the entity's (invisible) box picks
        return mesh;
      }),
    [built, invalidate],
  );
  useEffect(
    () => () => {
      for (const m of [boxes, picks]) if (m) (m.material as THREE.Material).dispose();
      lines.dispose();
      arrows.dispose();
      if (glow) {
        glow.geometry.dispose();
        (glow.material as THREE.Material).dispose();
      }
      for (const m of models) {
        m.geometry.dispose();
        for (const mat of m.material as THREE.MeshLambertMaterial[]) {
          mat.map?.dispose();
          mat.dispose();
        }
      }
    },
    [boxes, picks, lines, arrows, glow, models],
  );
  useEffect(() => invalidate(), [built, invalidate]);

  return (
    <group name="entity-batch">
      {boxes && <primitive object={boxes} />}
      {picks && <primitive object={picks} />}
      {models.map((m, i) => (
        <primitive key={i} object={m} />
      ))}
      <lineSegments geometry={lines} renderOrder={1} raycast={() => null}>
        <lineBasicMaterial color="#11140e" />
      </lineSegments>
      <lineSegments geometry={arrows} raycast={() => null}>
        <lineBasicMaterial vertexColors />
      </lineSegments>
      {glow && <primitive object={glow} />}
    </group>
  );
}

/** target lines (not the picked entity's): one dashed set in one draw. */
export function LinkBatch({ lines }: { lines: { a: Vec3; b: Vec3 }[] }) {
  const obj = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(lines.flatMap((l) => [...l.a, ...l.b]), 3));
    const seg = new THREE.LineSegments(g, new THREE.LineDashedMaterial({ color: "#7fb7c9", dashSize: 12, gapSize: 8, transparent: true, opacity: 0.6 }));
    seg.computeLineDistances();
    seg.raycast = () => null;
    return seg;
  }, [lines]);
  useEffect(
    () => () => {
      obj.geometry.dispose();
      (obj.material as THREE.Material).dispose();
    },
    [obj],
  );
  return lines.length ? <primitive object={obj} /> : null;
}

const MARKER_GEOMETRY = { octahedron: new THREE.OctahedronGeometry(1), sphere: new THREE.SphereGeometry(1, 10, 8) };

/** Many small solid markers (one colour each) in one draw: component points, check dots. Not pickable. */
export function InstancedMarkers({ items, shape, size, renderOrder = 0 }: { items: { at: Vec3; color: string }[]; shape: keyof typeof MARKER_GEOMETRY; size: number; renderOrder?: number }) {
  const mesh = useMemo(() => {
    if (!items.length) return null;
    const m = new THREE.InstancedMesh(MARKER_GEOMETRY[shape], new THREE.MeshBasicMaterial(), items.length);
    const mat = new THREE.Matrix4();
    const c = new THREE.Color();
    items.forEach((it, i) => {
      m.setMatrixAt(i, mat.makeScale(size, size, size).setPosition(it.at[0], it.at[1], it.at[2]));
      m.setColorAt(i, c.set(it.color));
    });
    m.computeBoundingSphere();
    m.renderOrder = renderOrder;
    m.raycast = () => null;
    return m;
    // a new list from its owner each render: rebuild only when what it says changes
  }, [JSON.stringify(items), shape, size, renderOrder]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => (mesh?.material as THREE.Material | undefined)?.dispose(), [mesh]);
  return mesh ? <primitive object={mesh} /> : null;
}
