// The entity definitions (our Counter-Strike FGD, read by the app): fetched once per run.

import { useEffect, useState } from "react";
import { getFgd, type FgdClass } from "./editorApi";

let cache: Map<string, FgdClass> | null = null;
let pending: Promise<Map<string, FgdClass> | null> | null = null;

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

/** The definitions by class name (null until they arrive, or if they couldn't be read). */
export function useFgd(): Map<string, FgdClass> | null {
  const [m, setM] = useState(cache);
  useEffect(() => {
    if (cache) return;
    let live = true;
    void load().then((x) => live && setM(x));
    return () => {
      live = false;
    };
  }, []);
  return m;
}
