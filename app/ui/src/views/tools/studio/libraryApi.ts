// My library on the app's side (tools/studio/mod.rs: /api/tools/studio/library).

import { get, post } from "../common/api";
import { readItem, type LibItem } from "./library";

// the items that ship with the app (library/<name>.lib.json, made by make_library.py): read-only,
// listed with the player's own; their ids start builtin_ (a player's item never does)
const BUILT_IN = import.meta.glob("./library/*.lib.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
export const builtInItems: LibItem[] = Object.entries(BUILT_IN)
  .map(([path, raw]): LibItem | null => {
    const it = readItem(`builtin_${path.replace(/^.*\//, "").replace(".lib.json", "")}`, JSON.parse(raw));
    return it ? { ...it, builtIn: true } : null;
  })
  .filter((x): x is LibItem => !!x);

export async function loadLibrary(): Promise<LibItem[]> {
  const r = await get<{ items: { id: string; item: unknown }[] }>("/api/tools/studio/library");
  const mine = r.items.filter((x) => !x.id.startsWith("builtin_")).map((x) => readItem(x.id, x.item)).filter((x): x is LibItem => !!x);
  return [...builtInItems, ...mine].sort((a, b) => a.category.localeCompare(b.category) || a.name.localeCompare(b.name));
}

export const saveItem = (id: string, item: object) => post<{ ok?: boolean; version?: number }>(`/api/tools/studio/library/${encodeURIComponent(id)}`, { item });
export const deleteItem = (id: string) => post<{ ok?: boolean }>(`/api/tools/studio/library/${encodeURIComponent(id)}/delete`);
