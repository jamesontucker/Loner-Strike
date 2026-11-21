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
import { loadModel, modelOf, sharedModelGeometry, sharedModelMaterials, type Got } from "./EntityModels";
import { useKits } from "./kits";
import { useP5 } from "./p5store";
import { keyOf, originOf } from "./store";
import { usePaneMode } from "./renderModes";

type Look = { size: Vec3; colour: string; up: number };

const UNIT_BOX = new THREE.BoxGeometry(1, 1, 1);
// shared for the session: a rebuild never disposes a material, so three never drops and
// recompiles its shader program on an edit (PerfAudit #1)
const BOX_MATERIAL = new THREE.MeshLambertMaterial({ transparent: true, opacity: 0.85 });
const PICK_MATERIAL = new THREE.MeshBasicMaterial();
// Wireframe panes: studio models as lines, like the brushes (shared, so no recompiles)
const MODEL_WIRE = new THREE.MeshBasicMaterial({ color: "#aab894", wireframe: true, transparent: true, opacity: 0.55 });
const LINK_MATERIAL = new THREE.LineDashedMaterial({ color: "#7fb7c9", dashSize: 12, gapSize: 8, transparent: true, opacity: 0.6 });
const MARKER_MATERIAL = new THREE.MeshBasicMaterial();
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
export function EntityBatch({ ents, look, yaw, picked = false }: { ents: DocEntity[]; look: (e: DocEntity) => Look; yaw: (e: DocEntity) => number | null; picked?: boolean }) {
  const invalidate = useThree((s) => s.invalidate);
  const showModels = useP5((s) => s.showModels);
  const wire = usePaneMode() === "wire";
  const creatures = useKits((k) => k.creatures);
  useKits((k) => k.kits); // markers take their kit's colour when the kits arrive

  // what's drawn, as text: a refreshed document with the same entities keeps everything
  const sig = useMemo(
    () => ents.map((e) => `${e.id}|${e.classname}|${e.keys.map((k) => k.join("=")).join(";")}|${look(e).colour}`).join("\n"),
    [ents, look],
  );
  // models: load each path once; entities wait as boxes until theirs arrives
  const paths = useMemo(() => {
    const m = new Map<number, string>();
    if (showModels) for (const e of ents) {
      const p = modelOf(e, creatures);
      if (p) m.set(e.id, p);
    }
    return m;
  }, [sig, showModels, creatures]); // eslint-disable-line react-hooks/exhaustive-deps
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
      if (showModels && /^env_(sprite|glow|funnel)$/.test(e.classname)) {
        // (env_funnel: its particles are an orange flare, sprites/flare6.spr)
        const rgb = (keyOf(e.keys, "rendercolor") ?? (e.classname === "env_funnel" ? "255 150 40" : "255 255 255")).split(/\s+/).map(Number);
        const ok = rgb.length >= 3 && rgb.slice(0, 3).every(Number.isFinite) && rgb.some((x) => x > 0);
        const scale = Math.max(0.1, Math.min(8, Number(keyOf(e.keys, "scale") ?? 1) || 1));
        glowPos.push(o[0], o[1], o[2]);
        glowSize.push(32 * scale);
        glowTint.push(...(ok ? [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255] : [1, 1, 1]));
      }
    }
    return { boxIds, boxMats, boxCols, pickIds, pickMats, edges, arrows, arrowCols, glowPos, glowSize, glowTint, models: [...models.values()] };
  }, [sig, paths, loaded, showModels]); // eslint-disable-line react-hooks/exhaustive-deps

  const boxes = useMemo(() => {
    if (!built.boxIds.length) return null;
    const mesh = new THREE.InstancedMesh(UNIT_BOX, BOX_MATERIAL, built.boxIds.length);
    built.boxMats.forEach((m, i) => mesh.setMatrixAt(i, m));
    built.boxCols.forEach((col, i) => mesh.setColorAt(i, col));
    mesh.computeBoundingSphere();
    pickable(mesh, built.boxIds);
    return mesh;
  }, [built]);
  const picks = useMemo(() => {
    if (!built.pickIds.length) return null;
    const mesh = new THREE.InstancedMesh(UNIT_BOX, PICK_MATERIAL, built.pickIds.length);
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
        // shared for the session: textures are uploaded once per model and skin (PerfAudit #1)
        const geo = sharedModelGeometry(g.got.bundle, g.body);
        const mats = sharedModelMaterials(g.got.bundle, g.got.item, g.skin);
        const mesh = new THREE.InstancedMesh(geo, mats, g.mats.length);
        g.mats.forEach((m, i) => mesh.setMatrixAt(i, m));
        mesh.computeBoundingSphere();
        mesh.raycast = () => null; // the entity's (invisible) box picks
        return mesh;
      }),
    [built],
  );
  useEffect(
    () => () => {
      for (const m of [boxes, picks]) m?.dispose(); // per-instance buffers (the materials are shared)
      lines.dispose();
      arrows.dispose();
      if (glow) {
        glow.geometry.dispose();
        (glow.material as THREE.Material).dispose();
      }
      for (const m of models) m.dispose(); // per-instance buffers only: geometry and materials are shared
    },
    [boxes, picks, lines, arrows, glow, models],
  );
  useEffect(() => invalidate(), [built, invalidate]);
  // Wireframe: no filled boxes, models drawn as lines
  for (const m of models) {
    m.userData.solid ??= m.material;
    (m as THREE.Mesh).material = wire ? MODEL_WIRE : (m.userData.solid as THREE.Material | THREE.Material[]);
  }
  if (boxes) boxes.visible = !wire;

  return (
    <group name="entity-batch">
      {boxes && <primitive object={boxes} />}
      {picks && <primitive object={picks} />}
      {models.map((m, i) => (
        <primitive key={i} object={m} />
      ))}
      {/* picked: yellow outlines and arrows drawn through walls, as EntityBox drew them */}
      <lineSegments geometry={lines} renderOrder={picked ? 3 : 1} raycast={() => null}>
        <lineBasicMaterial color={picked ? "#ffd84a" : wire ? "#c6d2ae" : "#11140e"} depthTest={!picked && !wire} />
      </lineSegments>
      <lineSegments geometry={arrows} raycast={() => null}>
        {picked ? <lineBasicMaterial color="#ffd84a" /> : <lineBasicMaterial vertexColors />}
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
    const seg = new THREE.LineSegments(g, LINK_MATERIAL);
    seg.computeLineDistances();
    seg.raycast = () => null;
    return seg;
  }, [lines]);
  useEffect(
    () => () => {
      obj.geometry.dispose();
    },
    [obj],
  );
  return lines.length ? <primitive object={obj} /> : null;
}

const MARKER_GEOMETRY = { octahedron: new THREE.OctahedronGeometry(1), sphere: new THREE.SphereGeometry(1, 10, 8), box: new THREE.BoxGeometry(2, 2, 2) };

/** Many small solid markers (one colour each) in one draw: component points, check dots. Not pickable. */
export function InstancedMarkers({ items, shape, size, renderOrder = 0, material }: { items: { at: Vec3; color: string }[]; shape: keyof typeof MARKER_GEOMETRY; size: number; renderOrder?: number; material?: THREE.Material }) {
  const mesh = useMemo(() => {
    if (!items.length) return null;
    const m = new THREE.InstancedMesh(MARKER_GEOMETRY[shape], material ?? MARKER_MATERIAL, items.length);
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
  useEffect(
    () => () => {
      if (!mesh) return;
      mesh.dispose(); // per-instance buffers (materials are shared or the caller's)
    },
    [mesh, material],
  );
  return mesh ? <primitive object={mesh} /> : null;
}
