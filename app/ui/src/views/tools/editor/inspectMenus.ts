// The Inspector's right-click menus' items (InspectMenu draws them).

import { canPasteTransform, clipNow, copyProps, copyTransform, copyValue, pasteProps, pasteTransform, pasteValue, rowWord, type XfRow } from "./inspectClip";
import type { MenuItem } from "./InspectMenu";
import { keysOf } from "./Shortcuts";
import { useEditor } from "./store";

const k = (id: string) => keysOf(id)[0];

/** Copy / Paste properties (onto the picked entities). */
export function propItems(): MenuItem[] {
  const ents = useEditor.getState().sel.entities.length;
  return [
    { label: "Copy properties", run: () => copyProps(), disabled: !ents, keys: k("edit.copyProps"), tip: "Its keys, but not its class, place, brush model or name" },
    { label: "Paste properties", run: () => void pasteProps(), disabled: !ents || clipNow()?.kind !== "props", keys: k("edit.pasteProps"), tip: "Onto every picked entity; keys a class doesn't have are skipped" },
  ];
}

/** A Transform row's menu (or the whole Transform's, from its header). */
export function transformItems(row?: XfRow): MenuItem[] {
  const what = row ? rowWord(row) : "Transform";
  return [
    { label: `Copy ${what}`, run: () => copyTransform(row), keys: row ? undefined : k("edit.copyTransform") },
    { label: `Paste ${what}`, run: () => void pasteTransform(row), disabled: !canPasteTransform(row), keys: row ? undefined : k("edit.pasteTransform") },
    ...(row ? [{ label: "Copy the whole Transform", run: () => copyTransform() }, { label: "Paste the whole Transform", run: () => void pasteTransform(), disabled: !canPasteTransform() }] : []),
    null,
    ...propItems(),
  ];
}

/** A key's menu: its value, then the properties. */
export function keyItems(key: string, value: string): MenuItem[] {
  const c = clipNow();
  const n = useEditor.getState().sel.entities.length;
  return [
    { label: `Copy ${key}`, run: () => copyValue(key, value), tip: value ? `"${value}"` : "(empty)" },
    { label: n > 1 ? `Paste into ${key} (${n} entities)` : `Paste into ${key}`, run: () => void pasteValue(key), disabled: c?.kind !== "value", tip: c?.kind === "value" ? `"${c.value}" (from ${c.key})` : "Copy a value first" },
    null,
    ...propItems(),
  ];
}
