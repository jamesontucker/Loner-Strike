// The map editor's Checks panel (agent Checks; it was in Panels.tsx). Live checks after every
// edit: GoldSrc limits, broken brushes, textures and spawns (the basic checks), and the deeper
// ones: z-fighting, things stuck in solid, micro-gaps and off-grid brushes, CS gameplay by map
// prefix, faces nobody can see, texture seams, stretched textures, ladders, lighting basics and
// heavy views. Each finding says where it is (Pick selects it, Look flies there), what to do
// about it, and has a one-click fix where that's safe (one undo step). The findings are drawn in
// the 3D views too (CheckMarkers.tsx).

import { useState } from "react";
import { BusyButton } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { CHECK_LIMITS_KEY, checkLimits, type FixData, type Issue } from "./editorApi";
import { originOf, useEditor } from "./store";
import { placeKit } from "./KitPanel"; // P5
import { useCheckMarks } from "./CheckMarkers";
import "./checks.css";

const LEVEL_LABEL: Record<Issue["level"], string> = { error: "Problem", warning: "Warning", info: "Note" };

/** Which heading a finding goes under. */
const GROUPS: [string, string[]][] = [
  ["Geometry", ["bad-brush", "outside-world", "z-fighting", "micro-gap", "off-grid", "hidden-faces", "too-many-brushes", "many-brushes"]],
  ["Textures", ["missing-texture", "long-texture-name", "too-many-textures", "no-wads", "texture-seam", "stretched-texture"]],
  ["Entities", ["no-classname", "extra-worldspawn", "empty-brush-entity", "no-origin", "long-key", "dangling-target", "stuck-in-solid", "ladder-gap", "too-many-entities", "many-entities", "too-many-models", "many-models", "entstring"]],
  ["Gameplay", ["no-spawns", "no-ct-spawns", "no-t-spawns", "few-spawns", "spawns-overlap", "spawn-no-floor", "no-buyzone", "spawn-no-buyzone", "no-bombsite", "no-hostages", "no-rescue", "no-vip", "no-vip-zone", "no-escape", "map-parameters"]],
  ["Lighting", ["no-sun", "dark-room", "light-in-solid"]],
  ["Performance", ["heavy-view"]],
];
const GROUP_OF = new Map(GROUPS.flatMap(([g, codes]) => codes.map((c) => [c, g] as const)));
const groupOf = (i: Issue) => GROUP_OF.get(i.code) ?? (i.code.startsWith("kit-") ? "Gameplay" : "Files and modes");

export const FIX_LABEL: Record<string, string> = {
  "delete-brush": "Delete the brush",
  "to-world": "Make its brushes world brushes",
  "delete-entity": "Delete it",
  "clear-target": "Clear the target",
  "add-wads": "Add halflife.wad and cstrike.wad",
  "replace-texture": "Replace with the current texture",
  "place:info_player_start": "Place a CT spawn",
  "place:info_player_deathmatch": "Place a T spawn",
  "nudge-face": "Nudge 1 unit",
  "null-face": "NULL the hidden face",
  "move-entity": "Move it to free space",
  "close-gap": "Close the gap",
  "snap-brush": "Snap to whole units",
  "null-hidden": "NULL them all",
  "align-neighbor": "Align with neighbor",
  "reset-scale": "Reset the scale to 1",
  "place:light_environment": "Place a light_environment",
  "place:light": "Place a light",
  "place:hostage_entity": "Place a hostage",
  "place:info_vip_start": "Place the VIP start",
};

type Filter = "all" | Issue["level"];

const fmtAt = (p: Vec3) => p.map((x) => Math.round(x)).join(" ");

/** Where a finding is: its spot, else its brush's middle, else its entity's origin. */
function whereOf(i: Issue): { point: Vec3; size: number } | null {
  const s = useEditor.getState();
  if (i.at) return { point: i.at, size: i.code === "heavy-view" ? 400 : i.code === "dark-room" ? 300 : 96 };
  if (i.brush != null) {
    const b = s.brushes.get(i.brush);
    if (b) return { point: b.min.map((x, k) => (x + b.max[k]) / 2) as Vec3, size: Math.max(64, ...b.max.map((x, k) => x - b.min[k])) };
  }
  if (i.entity != null) {
    const e = s.doc?.entities.find((x) => x.id === i.entity);
    const o = e && originOf(e.keys);
    if (o) return { point: o, size: 64 };
  }
  return null;
}

export function ChecksPanel() {
  const checks = useEditor((s) => s.checks);
  const texture = useEditor((s) => s.texture);
  const showMarks = useCheckMarks((s) => s.show);
  const [busy, setBusy] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [settings, setSettings] = useState(false);
  const [limits, setLimits] = useState(checkLimits);
  const s = useEditor.getState();
  if (!checks) {
    return (
      <section className="tools-panel">
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Checking the map…
        </p>
      </section>
    );
  }
  const pick = (i: Issue) => {
    if (i.brush != null) s.select({ brushes: [i.brush], entities: [], face: i.face != null ? { brush: i.brush, face: i.face } : null });
    else if (i.entity != null) s.select({ brushes: [], entities: [i.entity], face: null });
  };
  const look = (i: Issue, n: number) => {
    const w = whereOf(i);
    if (w) useTools.getState().look(w.point, w.size);
    useCheckMarks.getState().set({ focus: n });
  };
  const run = async (list: { fix: string; brush?: number; entity?: number; texture?: string; face?: number; fixData?: FixData }[], key: string) => {
    const first = list[0]?.fix;
    if (!first) return;
    if (first.startsWith("place:")) {
      s.set({ tool: "entity", entityClass: first.slice(6) });
      s.notify("Click a floor in the view to place it.");
      return;
    }
    setBusy(key);
    try {
      await s.cmd({ op: "fix", fixes: list.map((i) => ({ fix: i.fix, brush: i.brush, entity: i.entity, texture: i.texture, face: i.face, fixData: i.fixData })), with: first === "replace-texture" ? texture : undefined });
    } finally {
      setBusy(null);
    }
  };
  const fixOf = (i: Issue) => ({ fix: i.fix!, brush: i.brush, entity: i.entity, texture: i.texture, face: i.face, fixData: i.fixData });
  const saveLimits = (l: typeof limits) => {
    setLimits(l);
    if (!(l.minScale > 0 && l.maxScale > l.minScale)) return;
    try {
      localStorage.setItem(CHECK_LIMITS_KEY, JSON.stringify(l));
    } catch {
      /* this session only */
    }
    s.refreshChecks();
  };

  const counts = { error: 0, warning: 0, info: 0 };
  for (const i of checks.issues) counts[i.level]++;
  const shown = checks.issues.map((i, n) => [i, n] as const).filter(([i]) => filter === "all" || i.level === filter);
  const groups = new Map<string, (readonly [Issue, number])[]>();
  for (const row of shown) groups.set(groupOf(row[0]), [...(groups.get(groupOf(row[0])) ?? []), row]);
  const order = [...GROUPS.map(([g]) => g), "Files and modes"];

  return (
    <section className="tools-panel editor-checks ck" aria-label="Checks">
      <div className="ck-bar">
        <div className="seg" role="group" aria-label="Show">
          {(
            [
              ["all", `All ${checks.issues.length}`],
              ["error", `Problems ${counts.error}`],
              ["warning", `Warnings ${counts.warning}`],
              ["info", `Notes ${counts.info}`],
            ] as const
          ).map(([k, label]) => (
            <button key={k} type="button" aria-pressed={filter === k} onClick={() => setFilter(k)}>
              {label}
            </button>
          ))}
        </div>
        <label className="editor-check" title="Dots where the problems are, and soft balls over heavy views and dark rooms">
          <input type="checkbox" checked={showMarks} onChange={(e) => useCheckMarks.getState().set({ show: e.target.checked })} /> Show in 3D
        </label>
        <button type="button" className="btn small" aria-expanded={settings} onClick={() => setSettings(!settings)}>
          Settings
        </button>
      </div>
      {settings && (
        <div className="ck-settings">
          <span>Stretched textures: a scale under</span>
          <input type="number" aria-label="Smallest texture scale" min={0.01} step={0.05} value={limits.minScale} onChange={(e) => saveLimits({ ...limits, minScale: Number(e.target.value) })} />
          <span>or over</span>
          <input type="number" aria-label="Largest texture scale" min={0.1} step={0.5} value={limits.maxScale} onChange={(e) => saveLimits({ ...limits, maxScale: Number(e.target.value) })} />
          {!(limits.minScale > 0 && limits.maxScale > limits.minScale) && <span className="ck-bad">The second number must be bigger.</span>}
        </div>
      )}
      {checks.issues.length === 0 ? (
        <p className="small-note">No problems found. (Leaks are found when the map is compiled.)</p>
      ) : shown.length === 0 ? (
        <p className="small-note muted">Nothing at this level.</p>
      ) : (
        order
          .filter((g) => groups.has(g))
          .map((g) => {
            const rows = groups.get(g)!;
            // fixes that can be done all at once, by kind
            const kinds = new Map<string, Issue[]>();
            for (const [i] of rows) if (i.fix && !i.fix.startsWith("place:")) kinds.set(i.code, [...(kinds.get(i.code) ?? []), i]);
            return (
              <div key={g} className="ck-group">
                <h4>
                  {g} <span className="muted">{rows.length}</span>
                </h4>
                {[...kinds.entries()]
                  .filter(([, l]) => l.length > 1)
                  .map(([code, l]) => (
                    <BusyButton key={code} busy={busy === `all-${code}`} className="btn small" label={`Fix all ${l.length}: ${FIX_LABEL[l[0].fix!] ?? "Fix"}`} busyLabel="Fixing…" onClick={() => void run(l.map(fixOf), `all-${code}`)} />
                  ))}
                <ul>
                  {rows.map(([i, n]) => {
                    const where = i.brush != null || i.entity != null;
                    return (
                      <li key={n} className={`editor-issue ${i.level}`} data-code={i.code} onMouseEnter={() => useCheckMarks.getState().set({ focus: n })}>
                        <span className="ck-msg">
                          <strong>{LEVEL_LABEL[i.level]}:</strong> {i.message}
                        </span>
                        {i.at && <span className="ck-at muted">at {fmtAt(i.at)}</span>}
                        {i.hint && <span className="ck-hint">{i.hint}</span>}
                        <span className="ck-actions">
                          {where && (
                            <button type="button" className="btn small" onClick={() => pick(i)} title="Select it">
                              Pick
                            </button>
                          )}
                          {(where || i.at) && (
                            <button type="button" className="btn small" onClick={() => look(i, n)} title="Fly the 3D view there">
                              Look
                            </button>
                          )}
                          {/* P5: a mode kit's marker or component to place */}
                          {i.place && (
                            <button type="button" className="btn small editor-fix" onClick={() => placeKit(i.place!)}>
                              Place
                            </button>
                          )}
                          {i.fix && (
                            <BusyButton
                              busy={busy === `one-${n}`}
                              className="btn small editor-fix"
                              label={i.fix === "replace-texture" ? `Replace with ${texture}` : FIX_LABEL[i.fix] ?? "Fix"}
                              busyLabel="Fixing…"
                              onClick={() => void run([fixOf(i)], `one-${n}`)}
                            />
                          )}
                          {i.moreFixes?.map((m, k) => (
                            <BusyButton key={k} busy={busy === `more-${n}-${k}`} className="btn small editor-fix" label={FIX_LABEL[m.fix] ?? "Fix"} busyLabel="Fixing…" onClick={() => void run([{ ...m, face: m.fixData?.face }], `more-${n}-${k}`)} />
                          ))}
                        </span>
                      </li>
                    );
                  })}
                </ul>
              </div>
            );
          })
      )}
      {!checks.texturesChecked && <p className="small-note muted">Textures aren't checked until the game is set up on this computer.</p>}
    </section>
  );
}
