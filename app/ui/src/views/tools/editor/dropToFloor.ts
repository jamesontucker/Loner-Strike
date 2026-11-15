// End drops what's picked to the floor, like Unreal: straight down until its bottom rests on the
// top of a brush under it (the highest one below whose footprint it overlaps). Triggers and
// other see-through brush entities aren't a floor. One undo step.

import type { Vec3 } from "../viewer/coords";
import { selectionIds } from "./EditorScene";
import { fgdNow } from "./fgd";
import { originOf, useEditor } from "./store";

const NOT_FLOOR = /^(trigger_|func_ladder|func_buyzone|func_bomb_target|func_hostage_rescue|func_vip_safetyzone|func_escapezone|env_bubbles|func_illusionary)/;

export async function dropToFloor() {
  const s = useEditor.getState();
  const ids = selectionIds();
  const picked = new Set(ids.brushes);
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  const add = (a: Vec3, b: Vec3) => {
    for (let i = 0; i < 3; i++) {
      min[i] = Math.min(min[i], a[i]);
      max[i] = Math.max(max[i], b[i]);
    }
  };
  for (const id of ids.brushes) {
    const b = s.brushes.get(id);
    if (b) add(b.min, b.max);
  }
  // a point entity: its box from the definition (16 units each way when it has none)
  for (const id of ids.entities) {
    const e = s.doc?.entities.find((x) => x.id === id);
    if (!e || e.brushes.length) continue;
    const o = originOf(e.keys);
    if (!o) continue;
    const sz = fgdNow()?.get(e.classname)?.size;
    const lo: Vec3 = sz?.length === 6 ? [o[0] + sz[0], o[1] + sz[1], o[2] + sz[2]] : [o[0] - 16, o[1] - 16, o[2] - 16];
    const hi: Vec3 = sz?.length === 6 ? [o[0] + sz[3], o[1] + sz[4], o[2] + sz[5]] : [o[0] + 16, o[1] + 16, o[2] + 16];
    add(lo, hi);
  }
  if (!Number.isFinite(min[2])) return s.notify("Pick something to drop.", true);
  const owner = new Map((s.doc?.entities ?? []).flatMap((e) => e.brushes.map((b) => [b, e.classname] as const)));
  let floor = -Infinity;
  for (const [id, b] of s.brushes) {
    if (picked.has(id) || s.hidden.brushes.has(id)) continue;
    if (NOT_FLOOR.test(owner.get(id) ?? "")) continue;
    // (overlapping footprint, and its top at or under the bottom)
    if (b.max[0] <= min[0] || b.min[0] >= max[0] || b.max[1] <= min[1] || b.min[1] >= max[1]) continue;
    if (b.max[2] <= min[2] + 0.01 && b.max[2] > floor) floor = b.max[2];
  }
  if (!Number.isFinite(floor)) return s.notify("There's nothing under it to drop onto.", true);
  const d = Math.round((floor - min[2]) * 1000) / 1000;
  if (Math.abs(d) < 0.001) return s.notify("It's on the floor already.");
  await s.cmd({ op: "transform", ...ids, translate: [0, 0, d], lock: s.textureLock });
}
