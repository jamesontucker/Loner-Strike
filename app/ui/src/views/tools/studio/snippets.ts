// The snippet library: pieces of a mode to drop into a graph. Built-in ones ship in snippets/
// (made by scripts/make-studio-snippets.py); the player's own are kept by the app
// (/api/tools/studio/snippets) and shared like any community package: graph data only.

import { readSnippet, type Snippet } from "./graphOps";
import { get, post } from "../common/api";

const BUILT_IN = import.meta.glob("./snippets/*.snippet.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

export const builtInSnippets: Snippet[] = Object.entries(BUILT_IN)
  .map(([path, raw]) => {
    const id = path.replace(/^.*\//, "").replace(".snippet.json", "");
    try {
      return readSnippet(id, JSON.parse(raw));
    } catch {
      return null;
    }
  })
  .filter((s): s is Snippet => !!s)
  .sort((a, b) => a.name.localeCompare(b.name));

export async function mySnippets(): Promise<Snippet[]> {
  const r = await get<{ snippets: { id: string; snippet: unknown }[] }>("/api/tools/studio/snippets");
  return r.snippets.map((x) => readSnippet(`my_${x.id}`, x.snippet, true)).filter((s): s is Snippet => !!s);
}

export const saveSnippet = (id: string, snippet: object) => post<{ ok?: boolean }>(`/api/tools/studio/snippet/${encodeURIComponent(id)}`, { snippet });
export const deleteSnippet = (id: string) => post<{ ok?: boolean }>(`/api/tools/studio/snippet/${encodeURIComponent(id.replace(/^my_/, ""))}/delete`);
