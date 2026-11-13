// The Game Mode Studio's graph file, version 2 (version 1, without variables, still opens): our own format and the source of truth. React
// Flow only shows and edits it (graphToFlow / flowToGraph in Editor.tsx), so the editor can
// change without breaking saved modes. The app checks the same shape (tools/studio/graph.rs);
// files from a newer app are refused with a readable message instead of half-opening.

import { z } from "zod";

export const SCHEMA = "loner-strike/graph";
export const VERSION = 2;

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
  params: z.record(z.union([label(100), z.number().finite(), z.boolean()])).default({}),
  breakpoint: z.boolean().optional(),
});

export const edgeSchema = z.object({ id, from: pinRef, to: pinRef });

export const commentSchema = z.object({
  id,
  text: z.string().max(2000).regex(NOTE_CHARS, "letters, digits, spaces and plain punctuation only (no * / \\ \" ^)").default(""),
  pos: pos.default({ x: 0, y: 0 }),
  size: z.object({ w: z.number().positive(), h: z.number().positive() }),
  color: z.string().regex(/^#[0-9a-fA-F]{6}$/).optional(),
});

export const VAR_TYPES = ["number", "bool", "text"] as const;
export const variableSchema = z
  .object({
    id,
    name: label(30).refine((s) => s.trim().length > 0, "a name"),
    type: z.enum(VAR_TYPES),
    scope: z.enum(["mode", "player"]),
    default: z.union([z.number().finite().min(-100000).max(100000), z.boolean(), label(60)]).optional(),
  })
  .refine((v) => v.default === undefined || (v.type === "number" ? typeof v.default === "number" : v.type === "bool" ? typeof v.default === "boolean" : typeof v.default === "string"), "the start value doesn't fit the type");

export const graphSchema = z.object({
  schema: z.literal(SCHEMA),
  version: z.union([z.literal(1), z.literal(VERSION)]).transform(() => VERSION),
  mode: z.object({
    id: modeId,
    name: label(40),
    description: label(60).default(""),
  }),
  variables: z.array(variableSchema).max(64).default([]),
  nodes: z.array(nodeSchema).max(400).default([]),
  edges: z.array(edgeSchema).max(1200).default([]),
  comments: z.array(commentSchema).max(100).default([]),
});

export type Graph = z.infer<typeof graphSchema>;
export type GNode = z.infer<typeof nodeSchema>;
export type GEdge = z.infer<typeof edgeSchema>;
export type GComment = z.infer<typeof commentSchema>;
export type GVariable = z.infer<typeof variableSchema>;
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
  return { graph: r.data };
}

export function emptyGraph(id: string, name: string): Graph {
  return { schema: SCHEMA, version: VERSION, mode: { id, name, description: "" }, variables: [], nodes: [], edges: [], comments: [] };
}
