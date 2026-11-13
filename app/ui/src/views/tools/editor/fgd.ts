// The entity definitions (our Counter-Strike FGD plus the map's own FGD file, read by the
// app): fetched once, and again when the map's FGD changes (reloadFgd).

import { useEffect, useState } from "react";
import { getFgd, type FgdClass } from "./editorApi";

let cache: Map<string, FgdClass> | null = null;
let pending: Promise<Map<string, FgdClass> | null> | null = null;
const listeners = new Set<(m: Map<string, FgdClass> | null) => void>();

function load() {
  pending ??= getFgd().then((r) => {
    if (r.error || !Array.isArray(r.classes)) {
      pending = null;
      return null;
    }
    cache = new Map(r.classes.map((c) => [c.name, c]));
    return cache;
  });
  return pending;
}

/** Reads the definitions again (after the map's own FGD file changed). */
export async function reloadFgd() {
  cache = null;
  pending = null;
  const m = await load();
  for (const l of listeners) l(m);
}

/** The definitions by class name (null until they arrive, or if they couldn't be read). */
export function useFgd(): Map<string, FgdClass> | null {
  const [m, setM] = useState(cache);
  useEffect(() => {
    listeners.add(setM);
    if (!cache) {
      let live = true;
      void load().then((x) => live && setM(x));
      return () => {
        live = false;
        listeners.delete(setM);
      };
    }
    return () => void listeners.delete(setM);
  }, []);
  return m;
}
