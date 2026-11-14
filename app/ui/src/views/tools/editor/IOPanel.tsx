// P5: the entity I/O view (Hammer parity): what triggers what, for deathrun traps, jail cells and
// fun maps. A panel lists every entity that sets something off, kills it, waits on a master or
// points at it, with a multi_manager's names and delays; broken links (a name no entity has) are
// flagged with a one-click remove. Picking an entity shows its chain here and as colored arrows
// in the views (IOArrows): yellow fires, red kills, purple waits on (master), cyan points at.

import { Line } from "@react-three/drei";
import { useMemo, useState } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity } from "./editorApi";
import { chainOf, ioLinks, KIND_COLOR, KIND_WORD, type IoLink } from "./ioGraph";
import { keyOf, originOf, useEditor, type BrushGeo } from "./store";

type Filter = "all" | "broken" | "chain";

const label = (e: DocEntity | undefined) => (e ? `${e.classname}${keyOf(e.keys, "targetname") ? ` “${keyOf(e.keys, "targetname")}”` : ` #${e.id}`}` : "?");

function centre(e: DocEntity, brushes: Map<number, BrushGeo>): Vec3 | null {
  if (!e.brushes.length) return originOf(e.keys);
  const lo: Vec3 = [Infinity, Infinity, Infinity];
  const hi: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const id of e.brushes) {
    const b = brushes.get(id);
    if (!b) continue;
    for (let i = 0; i < 3; i++) (lo[i] = Math.min(lo[i], b.min[i])), (hi[i] = Math.max(hi[i], b.max[i]));
  }
  return Number.isFinite(lo[0]) ? (lo.map((x, i) => (x + hi[i]) / 2) as Vec3) : null;
}

const pick = (id: number) => useEditor.getState().select({ brushes: [], entities: [id], face: null });
function look(e: DocEntity | undefined) {
  if (!e) return;
  const c = centre(e, useEditor.getState().brushes);
  if (c) useTools.getState().look(c, 128);
}

export function IOPanel() {
  const ents = useEditor((s) => s.doc?.entities);
  const sel = useEditor((s) => s.sel.entities);
  const [filter, setFilter] = useState<Filter>("all");
  const [q, setQ] = useState("");
  const byId = useMemo(() => new Map((ents ?? []).map((e) => [e.id, e])), [ents]);
  const links = useMemo(() => ioLinks(ents ?? []), [ents]);
  const picked = sel.length === 1 ? sel[0] : null;
  const chain = useMemo(() => (picked != null ? chainOf(links, picked) : null), [links, picked]);
  const broken = links.filter((l) => !l.to.length);
  const inChain = (l: IoLink) => !chain || picked == null || ((l.from === picked || chain.down.has(l.from) || chain.up.has(l.from)) && (!l.to.length || l.to.some((t) => t === picked || chain.down.has(t) || chain.up.has(t))));
  const text = q.trim().toLowerCase();
  const shown = links.filter(
    (l) =>
      (filter !== "broken" || !l.to.length) &&
      (filter !== "chain" || inChain(l)) &&
      (!text || l.name.toLowerCase().includes(text) || label(byId.get(l.from)).toLowerCase().includes(text) || l.to.some((t) => label(byId.get(t)).toLowerCase().includes(text))),
  );
  const groups = new Map<number, IoLink[]>();
  for (const l of shown) groups.set(l.from, [...(groups.get(l.from) ?? []), l]);
  const s = useEditor.getState();
  const remove = (l: IoLink) => void s.cmd({ op: "set_key", entity: l.from, key: l.key, value: "" });

  return (
    <section className="tools-panel p5-io" aria-label="What triggers what">
      <div className="row p5-io-head">
        <span className="small-note">
          {links.length} link{links.length === 1 ? "" : "s"}
          {broken.length ? (
            <>
              {" · "}
              <b className="p5-io-bad">{broken.length} broken</b>
            </>
          ) : (
            " · none broken"
          )}
        </span>
        <span className="push" />
        <div className="seg" role="group" aria-label="Show">
          {(
            [
              ["all", "All"],
              ["broken", "Broken"],
              ["chain", "Picked chain"],
            ] as const
          ).map(([id, l]) => (
            <button key={id} type="button" aria-pressed={filter === id} disabled={id === "chain" && picked == null} title={id === "chain" ? "What the picked entity sets off down the line, and what leads to it" : undefined} onClick={() => setFilter(id)}>
              {l}
            </button>
          ))}
        </div>
      </div>
      <input type="search" className="p5-io-search" placeholder="Find a name or a class" aria-label="Find in the links" value={q} onChange={(e) => setQ(e.target.value)} />
      <p className="small-note muted p5-io-legend">
        {(["fires", "kills", "master", "names"] as const).map((k) => (
          <span key={k}>
            <i style={{ background: KIND_COLOR[k] }} /> {KIND_WORD[k]}
          </span>
        ))}
        <span>Pick an entity to see its chain as arrows.</span>
      </p>
      {!links.length ? (
        <p className="small-note muted">Nothing triggers anything yet. Give an entity a name (targetname) and point another's target at it: a button at a door, a trigger at a multi_manager.</p>
      ) : !groups.size ? (
        <p className="small-note muted">Nothing matches.</p>
      ) : (
        <ul className="p5-io-list" role="list">
          {[...groups.entries()].slice(0, 400).map(([from, ls]) => {
            const e = byId.get(from);
            const hot = picked != null && (from === picked || chain?.down.has(from) || chain?.up.has(from));
            return (
              <li key={from} className={`p5-io-group${hot ? " hot" : ""}${from === picked ? " picked" : ""}`}>
                <div className="p5-io-from">
                  <button type="button" className="linklike" onClick={() => pick(from)} onDoubleClick={() => look(e)} title="Pick it (double-click: look at it)">
                    {label(e)}
                  </button>
                  <button type="button" className="btn small icon" aria-label={`Look at ${label(e)}`} title="Look at it" onClick={() => look(e)}>
                    ◎
                  </button>
                </div>
                <ul role="list">
                  {ls.map((l, i) => (
                    <li key={i} className={`p5-io-link${l.to.length ? "" : " broken"}`}>
                      <i className="p5-io-kind" style={{ background: KIND_COLOR[l.kind] }} aria-hidden="true" />
                      <span className="p5-io-word">{KIND_WORD[l.kind]}</span>
                      {l.to.length ? (
                        l.to.map((t) => (
                          <button key={t} type="button" className="linklike" onClick={() => pick(t)} onDoubleClick={() => look(byId.get(t))}>
                            {label(byId.get(t))}
                          </button>
                        ))
                      ) : (
                        <span className="p5-io-bad" title="No entity has this name (targetname), so nothing happens">
                          “{l.name}”: nothing has this name
                        </span>
                      )}
                      {l.delay != null && l.delay > 0 && <span className="muted">after {l.delay} s</span>}
                      {l.key !== l.kind && l.key !== "target" && <span className="muted mono p5-io-key">{l.key}</span>}
                      {!l.to.length && (
                        <button type="button" className="btn small" title={`Remove the key “${l.key}” from ${label(e)} (undoable)`} onClick={() => remove(l)}>
                          Remove
                        </button>
                      )}
                    </li>
                  ))}
                </ul>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** In the views: the picked entity's chain as colored arrows, and a red mark on every broken link. */
export function IOArrows() {
  const ents = useEditor((s) => s.doc?.entities);
  const brushes = useEditor((s) => s.brushes);
  const sel = useEditor((s) => s.sel.entities);
  const picked = sel.length === 1 ? sel[0] : null;
  const { arrows, broken } = useMemo(() => {
    const arrows: { a: Vec3; b: Vec3; color: string }[] = [];
    const broken: Vec3[] = [];
    if (!ents) return { arrows, broken };
    const byId = new Map(ents.map((e) => [e.id, e]));
    const links = ioLinks(ents);
    const at = new Map<number, Vec3 | null>();
    const where = (id: number) => {
      if (!at.has(id)) at.set(id, byId.get(id) ? centre(byId.get(id)!, brushes) : null);
      return at.get(id) ?? null;
    };
    for (const l of links) if (!l.to.length && broken.length < 300) {
      const p = where(l.from);
      if (p) broken.push(p);
    }
    if (picked != null) {
      const c = chainOf(links, picked);
      const inc = (x: number) => x === picked || c.down.has(x) || c.up.has(x);
      for (const l of links) {
        if (!inc(l.from)) continue;
        const a = where(l.from);
        for (const t of l.to) {
          const b = where(t);
          if (a && b && inc(t) && arrows.length < 500) arrows.push({ a, b, color: KIND_COLOR[l.kind] });
        }
      }
    }
    return { arrows, broken };
  }, [ents, brushes, picked]);
  return (
    <>
      {arrows.map((r, i) => (
        <Arrow key={i} {...r} />
      ))}
      {broken.map((p, i) => (
        <mesh key={`b${i}`} position={[p[0], p[1], p[2] + 20]} renderOrder={9}>
          <octahedronGeometry args={[7]} />
          <meshBasicMaterial color="#ff5f56" depthTest={false} transparent opacity={0.9} />
        </mesh>
      ))}
    </>
  );
}

const UP = new THREE.Vector3(0, 1, 0);
function Arrow({ a, b, color }: { a: Vec3; b: Vec3; color: string }) {
  const va = new THREE.Vector3(...a);
  const vb = new THREE.Vector3(...b);
  const dir = vb.clone().sub(va);
  const len = dir.length();
  if (len < 1) return null;
  dir.normalize();
  // the head two thirds of the way, pointing at the one set off
  const head = va.clone().addScaledVector(dir, len * 0.66);
  const q = new THREE.Quaternion().setFromUnitVectors(UP, dir);
  return (
    <>
      <Line points={[a, b]} color={color} lineWidth={2.5} depthTest={false} transparent opacity={0.95} renderOrder={9} />
      <mesh position={head} quaternion={q} renderOrder={10}>
        <coneGeometry args={[6, 18, 10]} />
        <meshBasicMaterial color={color} depthTest={false} />
      </mesh>
    </>
  );
}
