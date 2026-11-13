// My library on the app's side (tools/studio/mod.rs: /api/tools/studio/library).

import { get, post } from "../common/api";
import { readItem, type LibItem } from "./library";

export async function loadLibrary(): Promise<LibItem[]> {
  const r = await get<{ items: { id: string; item: unknown }[] }>("/api/tools/studio/library");
  return r.items.map((x) => readItem(x.id, x.item)).filter((x): x is LibItem => !!x).sort((a, b) => a.category.localeCompare(b.category) || a.name.localeCompare(b.name));
}

export const saveItem = (id: string, item: object) => post<{ ok?: boolean; version?: number }>(`/api/tools/studio/library/${encodeURIComponent(id)}`, { item });
export const deleteItem = (id: string) => post<{ ok?: boolean }>(`/api/tools/studio/library/${encodeURIComponent(id)}/delete`);
