// Comfort limits (limits.rs, the same ids): defaults, not hard caps. Adding a node that has one
// adds its mode setting (a number variable lim_<id> in the "Limits" group) when the mode doesn't
// have it yet: the creator sets its start value, hosts change it in mode settings.

import type { GVariable, Graph } from "./schema";

export type Limit = { id: string; label: string; default: number; lagAbove: number; max: number; kinds: string[] };

export const LIMITS: Limit[] = [
  // W1's utility nodes
  { id: "search_found", label: "Entities one search keeps", default: 64, lagAbove: 256, max: 2048, kinds: ["spatial.entities_in_radius"] },
  { id: "free_spot_tries", label: "Tries finding a free spot", default: 64, lagAbove: 256, max: 1024, kinds: ["spatial.free_spot"] },
  { id: "aim_trace", label: "Aim Point's longest trace", default: 8192, lagAbove: 8192, max: 32768, kinds: ["aim.point"] },
  { id: "tick_rate", label: "On Tick runs a second at most", default: 100, lagAbove: 100, max: 1000, kinds: ["event.on_tick"] },
  { id: "loop_steps", label: "Loop steps a frame", default: 20000, lagAbove: 200000, max: 2000000, kinds: ["flow.for_loop", "flow.for_loop_break", "flow.while", "flow.for_each_array", "flow.for_each_player", "flow.for_each_entity", "creature.for_each", "table.for_each", "marker.for_each"] },
  { id: "fn_loop_steps", label: "Loop steps in a Function", default: 1000000, lagAbove: 2000000, max: 10000000, kinds: ["flow.for_loop", "flow.for_loop_break", "flow.while", "flow.for_each_array", "flow.for_each_player", "flow.for_each_entity", "creature.for_each", "table.for_each", "marker.for_each"] },
  { id: "markers", label: "Objective markers at once", default: 32, lagAbove: 64, max: 256, kinds: ["fx.marker"] },
  { id: "trails", label: "Trails at once", default: 32, lagAbove: 64, max: 256, kinds: ["fun.trail"] },
  // the effects agent (gen/fx.rs)
  { id: "loop_sounds", label: "Loop sounds at once", default: 16, lagAbove: 64, max: 256, kinds: ["snd.loop"] },
  // EC (gen/coverage.rs)
  { id: "ec_attach", label: "Attached things at once", default: 64, lagAbove: 128, max: 256, kinds: ["ent.attach"] },
  { id: "ec_anim_loops", label: "Looping animations at once", default: 64, lagAbove: 128, max: 256, kinds: ["ent.play_anim"] },
  { id: "ec_npc_moves", label: "NPC moves at once", default: 32, lagAbove: 48, max: 64, kinds: ["npc.move_to", "npc.follow_path"] },
  { id: "ec_map_hooks", label: "Map entity kinds heard", default: 96, lagAbove: 160, max: 256, kinds: ["event.map_fired"] },
  { id: "command_rate", label: "Player Command uses a second", default: 5, lagAbove: 20, max: 1000, kinds: ["in.command"] },
  { id: "top_players", label: "Top Players places", default: 10, lagAbove: 50, max: 100, kinds: ["save.top"] },
  // W2 (collide.rs)
  { id: "trigger_volumes", label: "Trigger volumes at once", default: 32, lagAbove: 64, max: 256, kinds: ["vol.spawn"] },
  { id: "volume_members", label: "Things in one trigger volume", default: 32, lagAbove: 48, max: 64, kinds: ["vol.spawn"] },
  { id: "avoid_areas", label: "Areas bots avoid at once", default: 16, lagAbove: 24, max: 32, kinds: ["bots.avoid"] },
  { id: "visible_things", label: "Visible things at once", default: 128, lagAbove: 200, max: 1000, kinds: ["ent.spawn_prop", "ent.spawn_entity", "ent.spawn_weapon", "creature.spawn", "npc.spawn", "npc.spawn_use", "comp.create", "proj.spawn", "fx.marker"] },
  { id: "projectiles", label: "Projectiles at once", default: 64, lagAbove: 128, max: 512, kinds: ["proj.spawn"] },
  { id: "messages", label: "Effect messages a frame", default: 48, lagAbove: 56, max: 64, kinds: ["debug.draw_line", "debug.draw_box", "debug.draw_point", "fun.burst", "fun.trail", "msg.killfeed", "msg.status_icon", "proj.spawn", "msg.big_hud", "msg.color_chat", "msg.radio", "fx.dlight", "fx.decal", "fx.gibs", "fx.ring", "snd.play_2d", "fx.beam_ents", "fx.icon_over", "fx.sprite_at", "fx.tracer", "hud.radar_blip", "hud.team_score", "world.weather", "world.fog", "eng.score_tag", "eng.score_color", "eng.soundtrack", "eng.flashlight", "eng.nightvision"] },
];

export const limitVar = (l: Limit): GVariable =>
  ({
    id: `lim_${l.id}`,
    name: l.label.slice(0, 30),
    type: "number",
    scope: "mode",
    default: l.default,
    setting: { label: l.label, help: `A comfort limit (default ${l.default}). Higher may lag servers with many players.`, min: 1, max: l.max, step: 1, group: "Limits" },
  }) as GVariable;

/** The graph with the limit settings a node of this type needs (unchanged when it has them). */
export function withLimitVars(g: Graph, type: string): Graph {
  const add = LIMITS.filter((l) => l.kinds.includes(type) && !g.variables.some((v) => v.id === `lim_${l.id}`));
  return add.length ? { ...g, variables: [...g.variables, ...add.map(limitVar)] } : g;
}
