// What's picked in the map: an entity's class and keys (target/targetname links go to the
// other end), or a surface's texture. With nothing picked, a searchable list of entities.

import { useMemo, useState } from "react";
import { copyText } from "../../../api";
import { useTools } from "../../../stores/tools";
import { useToast } from "../../../ui";
import type { MapEntity } from "../toolsApi";
import { fmtVec, type Vec3 } from "../viewer/coords";
import { entityCentre } from "../viewer/Entities";
import type { ModelMesh } from "../viewer/World";

/** Keys whose value names another entity's targetname. */
const LINK_KEYS = new Set(["target", "killtarget", "master", "netname"]);

const nameOf = (e: MapEntity) => e.keys.targetname || (e.origin ? fmtVec(e.origin) : e.model ?? `#${e.index}`);

export function Inspector({ entities, meshes }: { entities: MapEntity[]; meshes: ModelMesh[] }) {
  const sel = useTools((s) => s.selection);
  const { select, look } = useTools.getState();
  const toast = useToast();
  const byName = useMemo(() => {
    const m = new Map<string, MapEntity[]>();
    for (const e of entities) if (e.keys.targetname) m.set(e.keys.targetname, [...(m.get(e.keys.targetname) ?? []), e]);
    return m;
  }, [entities]);

  const copy = async (what: string, v: Vec3) => toast((await copyText(fmtVec(v))) ? `${what} copied: ${fmtVec(v)}` : "Couldn't copy.", false);
  const goTo = (e: MapEntity) => {
    select({ kind: "entity", index: e.index });
    const c = entityCentre(e, meshes);
    if (c) look(c.point, c.size);
  };

  if (sel?.kind === "face") {
    const brush = sel.model ? meshes.find((m) => m.model === sel.model)?.entity : null;
    return (
      <section className="tools-panel" aria-label="Picked">
        <h3>Surface</h3>
        <dl className="tools-keys">
          <dt>Texture</dt>
          <dd className="mono">{sel.texture}</dd>
          <dt>Part of</dt>
          <dd>{brush ? <button type="button" className="link inline" onClick={() => goTo(brush)}>{brush.classname}</button> : "the world"}</dd>
          <dt>Point</dt>
          <dd className="mono">{fmtVec(sel.point)}</dd>
        </dl>
        <div className="row">
          <button className="btn small" onClick={() => copy("Point", sel.point)}>Copy point</button>
          <button className="btn small" onClick={() => select(null)}>Clear</button>
        </div>
      </section>
    );
  }

  const ent = sel?.kind === "entity" ? entities.find((e) => e.index === sel.index) : null;
  if (!ent) return <EntityList entities={entities} onPick={goTo} />;

  const centre = entityCentre(ent, meshes);
  const usedBy = ent.keys.targetname ? entities.filter((o) => o !== ent && Object.entries(o.keys).some(([k, v]) => LINK_KEYS.has(k) && v === ent.keys.targetname)) : [];
  const keys = Object.entries(ent.keys).filter(([k]) => k !== "classname");
  return (
    <section className="tools-panel" aria-label="Picked">
      <h3 className="tools-class">{ent.classname || "(no class)"}</h3>
      {ent.keys.targetname && <p className="small-note muted">Named “{ent.keys.targetname}”</p>}
      <div className="row">
        {centre && <button className="btn small" onClick={() => look(centre.point, centre.size)}>Look at</button>}
        {centre && <button className="btn small" onClick={() => copy(ent.origin ? "Origin" : "Centre", ent.origin ?? centre.point)}>{ent.origin ? "Copy origin" : "Copy centre"}</button>}
        <button className="btn small" onClick={() => select(null)}>Clear</button>
      </div>
      {keys.length > 0 && (
        <dl className="tools-keys">
          {keys.map(([k, v]) => {
            const to = LINK_KEYS.has(k) ? byName.get(v) : undefined;
            return (
              <div key={k} className="tools-key">
                <dt>{k}</dt>
                <dd className="mono">
                  {to?.length ? (
                    <button type="button" className="link inline" title={`Go to ${to[0].classname}`} onClick={() => goTo(to[0])}>{v}</button>
                  ) : (
                    v
                  )}
                  {to && to.length > 1 && <span className="muted"> (+{to.length - 1} more)</span>}
                </dd>
              </div>
            );
          })}
        </dl>
      )}
      {usedBy.length > 0 && (
        <>
          <h4>Set off by</h4>
          <ul className="tools-links">
            {usedBy.map((o) => (
              <li key={o.index}>
                <button type="button" className="link inline" onClick={() => goTo(o)}>{o.classname}</button> <span className="muted mono">{nameOf(o)}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

/** Every entity, grouped by class, with a search. */
function EntityList({ entities, onPick }: { entities: MapEntity[]; onPick: (e: MapEntity) => void }) {
  const [q, setQ] = useState("");
  const groups = useMemo(() => {
    const s = q.trim().toLowerCase();
    const m = new Map<string, MapEntity[]>();
    for (const e of entities) {
      if (e.classname === "worldspawn") continue;
      if (s && !e.classname.toLowerCase().includes(s) && !(e.keys.targetname ?? "").toLowerCase().includes(s)) continue;
      m.set(e.classname, [...(m.get(e.classname) ?? []), e]);
    }
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [entities, q]);
  return (
    <section className="tools-panel" aria-label="Entities">
      <h3>In this map</h3>
      <p className="small-note muted">Click something in the view to see what it is, or find it here.</p>
      <input className="search" type="search" aria-label="Find entities" placeholder="Find, like “spawn” or “door”" value={q} onChange={(e) => setQ(e.target.value)} />
      <div className="tools-groups">
        {groups.map(([cls, list]) => (
          <details key={cls} open={!!q.trim() && groups.length <= 3}>
            <summary>
              <span className="mono">{cls}</span> <span className="muted">{list.length}</span>
            </summary>
            <ul className="tools-links">
              {list.slice(0, 200).map((e) => (
                <li key={e.index}>
                  <button type="button" className="link inline" onClick={() => onPick(e)}>{nameOf(e)}</button>
                </li>
              ))}
              {list.length > 200 && <li className="muted">and {list.length - 200} more</li>}
            </ul>
          </details>
        ))}
        {!groups.length && <p className="small-note muted">{entities.length ? "Nothing matches." : "No entities in this map."}</p>}
      </div>
    </section>
  );
}
