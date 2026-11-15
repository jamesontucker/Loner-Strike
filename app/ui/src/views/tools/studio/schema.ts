// The Game Mode Studio's graph file, version 3 (versions 1 and 2 still open: what's new is all
// optional): our own format and the source of truth. React
// Flow only shows and edits it (graphToFlow / flowToGraph in Editor.tsx), so the editor can
// change without breaking saved modes. The app checks the same shape (tools/studio/graph.rs);
// files from a newer app are refused with a readable message instead of half-opening.

import { z } from "zod";

export const SCHEMA = "loner-strike/graph";
export const VERSION = 4;

const id = z.string().regex(/^[A-Za-z0-9_]{1,24}$/, "ids are 1 to 24 letters, digits or _");
export const modeId = z.string().regex(/^[a-z][a-z0-9_]{1,23}$/, "2 to 24 small letters, digits or _, starting with a letter");
/** Names, labels and text settings: letters, digits, space and plain punctuation only (the app
 *  checks the same: graph.rs label_char). Never * / \\ " ^ or new lines, so graph text can't
 *  turn into code in the plugin made from it. */
export const LABEL_CHARS = /^[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]*$/u;
const NOTE_CHARS = /^[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$\n]*$/u;
export const label = (max: number) => z.string().max(max).regex(LABEL_CHARS, "letters, digits, spaces and plain punctuation only (no * / \\ \" ^)");
const pos = z.object({ x: z.number().finite(), y: z.number().finite() });
const pinRef = z.object({ node: id, pin: z.string().min(1).max(32) });

export const nodeSchema = z.object({
  id,
  type: z.string().min(1).max(64),
  pos: pos.default({ x: 0, y: 0 }),
  // (choices from the registry may hold a /, like radio/go; the app checks each setting by its
  // kind, and a / in text is refused there)
  params: z.record(z.union([z.string().max(400).regex(/^[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$/|]*$/u, "letters, digits, spaces and plain punctuation only"), z.number().finite(), z.boolean()])).default({}),
  breakpoint: z.boolean().optional(),
  /** "Don't warn for this one": no Problems note about bots for it (graph.rs bot_awareness) */
  quiet: z.boolean().optional(),
  /** the Function or Macro whose graph (tab) it's on; none: the Event Graph */
  graph: id.optional(),
});

export const edgeSchema = z.object({ id, from: pinRef, to: pinRef });

export const commentSchema = z.object({
  id,
  text: z.string().max(2000).regex(NOTE_CHARS, "letters, digits, spaces and plain punctuation only (no * / \\ \" ^)").default(""),
  pos: pos.default({ x: 0, y: 0 }),
  size: z.object({ w: z.number().positive(), h: z.number().positive() }),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/).optional(),
  graph: id.optional(),
});

/** A type: number, bool, text, player, entity, team, vector, timer, enum:<id>, struct:<id>,
 *  array<item> or map<key,item> (the app checks the enums and structs exist: graph.rs type_ok). */
export const TYPE = /^(?:[a-z]+|enum:[A-Za-z0-9_]{1,24}|struct:[A-Za-z0-9_]{1,24}|array<(?:[a-z]+|enum:[A-Za-z0-9_]{1,24}|struct:[A-Za-z0-9_]{1,24})>|map<(?:text|number|player),(?:[a-z]+|enum:[A-Za-z0-9_]{1,24})>)$/;
const typeString = z.string().max(64).regex(TYPE, "a type");
export const VAR_TYPES = ["number", "bool", "text"] as const;
export const variableSchema = z
  .object({
    id,
    name: label(30).refine((s) => s.trim().length > 0, "a name"),
    type: typeString,
    scope: z.enum(["mode", "player", "team"]),
    reset: z.enum(["never", "round"]).optional(),
    /** a mode setting: hosts change its start value (Play page, server page, admin menu, MCP) */
    setting: z
      .object({
        label: label(30),
        help: label(120).default(""),
        min: z.number().finite().min(-100000).max(100000).default(0),
        max: z.number().finite().min(-100000).max(100000).default(100),
        step: z.number().finite().min(0).max(100000).default(0),
        choices: z.array(label(30)).max(12).optional(),
        group: label(20).default(""),
      })
      .optional(),
    // (null: no start value, as an older app wrote it)
    default: z
      .union([z.number().finite().min(-100000).max(100000), z.boolean(), label(60), z.null()])
      .optional()
      .transform((d) => d ?? undefined),
  })
  .refine(
    (v) =>
      v.default === undefined ||
      (v.type === "number" || v.type.startsWith("enum:") ? typeof v.default === "number" : v.type === "bool" ? typeof v.default === "boolean" : v.type === "text" ? typeof v.default === "string" : true),
    "the start value doesn't fit the type",
  );

/** Where a part came from: a library item and its version ("Update from library"). */
const libFrom = z.object({ lib: modeId, version: z.number().int().min(0).max(100000) }).optional();

/** A struct's field, a Function's or Macro's input or output. */
export const memberSchema = z.object({ id, name: label(30), type: z.union([typeString, z.literal("exec")]) });
export const enumSchema = z.object({ id, name: label(30), values: z.array(label(30)).max(32), from: libFrom });
export const structSchema = z.object({ id, name: label(30), fields: z.array(memberSchema).max(12), from: libFrom });
export const functionSchema = z.object({
  id,
  name: label(30),
  pure: z.boolean().optional(),
  inputs: z.array(memberSchema).max(12).default([]),
  outputs: z.array(memberSchema).max(12).default([]),
  locals: z.array(variableSchema).max(16).optional(),
  from: libFrom,
});
/** A map marker (the mode map kit, docs/notes/X.md): what a map for this mode places. */
const markerId = z.string().regex(/^[a-z0-9_]{1,24}$/);
export const markerKeySchema = z.object({
  name: markerId,
  type: z.enum(["number", "text", "bool", "choice"]),
  default: z.union([z.number().finite(), z.string().max(60), z.boolean(), z.null()]).optional(),
  help: z.string().max(120).default(""),
  choices: z.array(z.tuple([markerId, z.string().max(30)])).max(16).optional(),
  min: z.number().finite().optional(),
  max: z.number().finite().optional(),
});
export const markerSchema = z.object({
  id: markerId,
  label: label(30),
  description: label(200).default(""),
  type: z.enum(["point", "area", "path"]),
  min: z.number().int().min(0).max(64).default(0),
  max: z.number().int().min(1).max(64).optional(),
  keys: z.array(markerKeySchema).max(8).optional(),
  colour: z.string().regex(/^(#[0-9a-fA-F]{6})?$/).default(""),
  icon: z.string().max(12).default(""),
  size: z.tuple([z.number(), z.number(), z.number()]).optional(),
  radius: z.number().optional(),
});
export const mapKitSchema = z.object({
  size: z.enum(["", "small", "medium", "large"]).default(""),
  playersMin: z.number().int().min(1).max(32).optional(),
  playersMax: z.number().int().min(1).max(32).optional(),
  notes: label(600).default(""),
});
export const macroSchema = z.object({ id, name: label(30), inputs: z.array(memberSchema).max(12).default([]), outputs: z.array(memberSchema).max(12).default([]), from: libFrom });

export const graphSchema = z.object({
  schema: z.literal(SCHEMA),
  version: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(VERSION)]).transform(() => VERSION),
  mode: z.object({
    id: modeId,
    /** the mode's permanent identity (UUID v4): made with the mode, kept through renames */
    uid: z.string().regex(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/).optional(),
    name: label(40),
    description: label(60).default(""),
    /** game settings for the mode's cfg (Set Setting's choices; the app checks them) */
    settings: z.array(z.object({ cvar: z.string().regex(/^[a-z_][a-z0-9_]{1,31}$/), value: z.number().finite().min(0).max(100000) })).max(32).optional(),
  }),
  variables: z.array(variableSchema).max(64).default([]),
  nodes: z.array(nodeSchema).max(400).default([]),
  edges: z.array(edgeSchema).max(1200).default([]),
  comments: z.array(commentSchema).max(100).default([]),
  enums: z.array(enumSchema).max(32).optional(),
  structs: z.array(structSchema).max(32).optional(),
  functions: z.array(functionSchema).max(64).optional(),
  macros: z.array(macroSchema).max(32).optional(),
  /** more event graphs after the first, to organize (a node's graph names one); compiled as one */
  eventGraphs: z.array(z.object({ id, name: label(30) })).max(16).optional(),
  /** the mode's own events: Custom Event nodes run when called (Call, Set Timer by Event, the map) */
  timelines: z
    .array(
      z.object({
        id,
        name: label(30),
        length: z.number().min(0.1).max(600),
        loop: z.boolean().optional(),
        autoplay: z.boolean().optional(),
        useLastKey: z.boolean().optional(),
        tick: z.number().min(0.02).max(0.5).default(0.05),
        tracks: z
          .array(
            z.object({
              id,
              name: label(30),
              kind: z.enum(["float", "vector", "color", "event"]),
              keys: z
                .array(
                  z.object({
                    t: z.number().min(0).max(600),
                    v: z.union([z.number(), z.array(z.number()).length(3), z.null()]).optional(),
                    /** to the next key: linear, a step, eased in/out/both, or a curve by the tangents (curves.ts) */
                    ease: z.enum(["linear", "constant", "in", "out", "ease", "custom"]).optional(),
                    /** Custom: the key's own tangents (value per second), in and out; none: Auto */
                    tin: z.union([z.number().finite(), z.array(z.number().finite()).length(3)]).optional(),
                    tout: z.union([z.number().finite(), z.array(z.number().finite()).length(3)]).optional(),
                  }),
                )
                .max(64)
                .default([]),
            }),
          )
          .max(8)
          .default([]),
      }),
    )
    .max(16)
    .optional(),
  customEvents: z.array(z.object({ id, name: label(26), inputs: z.array(memberSchema).max(8).default([]), map: z.boolean().optional() })).max(32).optional(),
  /** the mode's data tables (tables.ts; the app checks the cells and the size limits: tables.rs) */
  dataTables: z
    .array(
      z.object({
        id,
        name: label(30),
        key: id,
        columns: z.array(z.object({ id, name: label(30), type: z.string().regex(/^(number|text|bool|weapon|entname|enum:[A-Za-z0-9_]{1,24})$/) })).min(1).max(16),
        rows: z.array(z.array(z.union([z.number().finite(), z.string().max(48), z.boolean()])).max(16)).max(256).default([]),
      }),
    )
    .max(16)
    .optional(),
  markers: z.array(markerSchema).max(32).optional(),
  mapKit: mapKitSchema.optional(),
});

export type Graph = z.infer<typeof graphSchema>;
export type GNode = z.infer<typeof nodeSchema>;
export type GEdge = z.infer<typeof edgeSchema>;
export type GComment = z.infer<typeof commentSchema>;
export type GVariable = z.infer<typeof variableSchema>;
export type GMember = z.infer<typeof memberSchema>;
export type GEnum = z.infer<typeof enumSchema>;
export type GStruct = z.infer<typeof structSchema>;
export type GMarker = z.infer<typeof markerSchema>;
export type GMarkerKey = z.infer<typeof markerKeySchema>;
export type GFunction = z.infer<typeof functionSchema>;
export type GMacro = z.infer<typeof macroSchema>;
export type ParamValue = GNode["params"][string];

/** Reads a graph (a file, the clipboard): the graph, or the words to show. */
export function parseGraph(v: unknown): { graph: Graph } | { error: string } {
  if (!v || typeof v !== "object") return { error: "That isn't a Game Mode Studio graph." };
  const o = v as Record<string, unknown>;
  if (o.schema !== SCHEMA) return { error: "That isn't a Game Mode Studio graph." };
  if (typeof o.version === "number" && o.version > VERSION) return { error: `That graph was made by a newer Loner-Strike (graph version ${o.version}). Update the app to open it.` };
  const r = graphSchema.safeParse(v);
  if (!r.success) {
    const i = r.error.issues[0];
    return { error: `That graph file is damaged (${i.path.join(".") || "file"}: ${i.message}).` };
  }
  return { graph: volSpawnPins(projSpawnPins(r.data)) };
}

/** Spawn Trigger Volume's outputs before the spawn outcome standard (graph.rs vol_spawn_pins, the
 *  same rule): Volume reads as Entity (`out`), Success's first id `success` as `ok`. */
export function volSpawnPins(g: Graph): Graph {
  const vols = new Set(g.nodes.filter((n) => n.type === "vol.spawn").map((n) => n.id));
  const old: Record<string, string> = { volume: "out", success: "ok" };
  if (!g.edges.some((e) => vols.has(e.from.node) && e.from.pin in old)) return g;
  return { ...g, edges: g.edges.map((e) => (vols.has(e.from.node) && e.from.pin in old ? { ...e, from: { ...e.from, pin: old[e.from.pin] } } : e)) };
}

/** Spawn Projectile's older pins (graph.rs proj_spawn_pins, the same rule): `rot_off` reads as
 *  `angles_offset`, and the off_f/off_r/off_u settings as a Local offset wired from a Make Vector
 *  beside the node (none for the old default 16, 0, 0: an unwired Local offset is that). */
export function projSpawnPins(g: Graph): Graph {
  const olds = g.nodes.filter((n) => n.type === "proj.spawn" && (g.edges.some((e) => e.to.node === n.id && e.to.pin === "rot_off") || ["off_f", "off_r", "off_u"].some((k) => k in n.params)));
  if (!olds.length) return g;
  let nodes = g.nodes;
  let edges = g.edges.map((e) => (e.to.pin === "rot_off" && olds.some((n) => n.id === e.to.node) ? { ...e, to: { ...e.to, pin: "angles_offset" } } : e));
  for (const n of olds) {
    const num = (k: string, d: number) => Math.max(-512, Math.min(512, typeof n.params[k] === "number" ? (n.params[k] as number) : d));
    const [f, r, u] = [num("off_f", 16), num("off_r", 0), num("off_u", 0)];
    const params = { ...n.params };
    delete params.off_f;
    delete params.off_r;
    delete params.off_u;
    nodes = nodes.map((x) => (x.id === n.id ? { ...x, params } : x));
    if ((f === 16 && r === 0 && u === 0) || edges.some((e) => e.to.node === n.id && e.to.pin === "offset")) continue;
    let vid = `${n.id}_offset`;
    while (nodes.some((x) => x.id === vid)) vid += "_";
    let eid = `${vid}_w`;
    while (edges.some((x) => x.id === eid)) eid += "_";
    nodes = [...nodes, { ...n, id: vid, type: "vec.make", pos: { x: n.pos.x - 260, y: n.pos.y + 160 }, params: { x: f, y: r, z: u } } as GNode];
    edges = [...edges, { id: eid, from: { node: vid, pin: "out" }, to: { node: n.id, pin: "offset" } } as GEdge];
  }
  return { ...g, nodes, edges };
}

/** A new mode's uid (UUID v4). */
export const newUid = (): string =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
        const r = (Math.random() * 16) | 0;
        return (c === "x" ? r : (r & 3) | 8).toString(16);
      });

export function emptyGraph(id: string, name: string): Graph {
  return { schema: SCHEMA, version: VERSION, mode: { id, uid: newUid(), name, description: "" }, variables: [], nodes: [], edges: [], comments: [] };
}

/** The app's own enums (graph.rs BUILTIN_ENUMS): in every mode, never saved with it. What Kind Of
 *  and Component Kind Of give, so they plug into Switch on Enum (its cases fill from them) and
 *  Select like any enum. */
export const BUILTIN_ENUMS: GEnum[] = [
  { id: "ls_kind", name: "Entity Kind", values: ["Player", "Bot", "Creature", "NPC", "Interactable", "Component", "Prop", "Map entity", "None"] },
  {
    id: "ls_comp_kind",
    name: "Component Kind",
    values: ["Capture point", "Extraction zone", "Checkpoint", "Shop", "Barricade", "Trap", "Loot container", "Supply drop", "Supply crate", "Perk machine", "Upgrade bench", "Mystery box", "Meeting point", "Moving platform", "Timed door", "Wave spawner", "Safe zone", "Shrinking zone", "Damage zone", "Weapon spawner", "Respawn point", "Player class", "Pickup", "NPC", "Interactable", "None"],
  },
  { id: "ls_spawn_fail", name: "Spawn Failure", values: ["None", "Blocked", "Out of entity slots", "Limit reached", "Missing file", "Retries used up", "Bad input"] },
  // (For Each Player / Pick Random Player's Who, in the order of their setting: X2)
  { id: "ls_who", name: "Who", values: ["Everyone in the game", "Everyone alive", "Everyone dead", "Terrorists", "Counter-Terrorists", "Living Terrorists", "Living Counter-Terrorists", "Bots", "People (not bots)"] },
];

/** The mode's enums and then the app's own (a mode's own of the same id wins). */
export const allEnums = (g: Pick<Graph, "enums"> | null | undefined): GEnum[] => {
  const mine = g?.enums ?? [];
  return [...mine, ...BUILTIN_ENUMS.filter((b) => !mine.some((e) => e.id === b.id))];
};
