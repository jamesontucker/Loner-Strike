// Undo and redo for the composer: a list of earlier documents. A drag calls `begin` once (on
// its first move) and `live` while it goes, so the whole drag is one step; sliders and typing
// pass a key, and changes with the same key close together merge into one step.

import { useCallback, useRef, useState } from "react";
import type { Doc } from "./doc";

type H = { past: Doc[]; doc: Doc; future: Doc[] };
const LIMIT = 100;
const MERGE_MS = 900;

export function useHistory(first: Doc) {
  const [h, setH] = useState<H>({ past: [], doc: first, future: [] });
  const last = useRef<{ key: string; at: number } | null>(null);

  const commit = useCallback((next: Doc | ((d: Doc) => Doc), key?: string) => {
    const now = Date.now();
    const merge = !!key && last.current?.key === key && now - last.current.at < MERGE_MS;
    last.current = key ? { key, at: now } : null;
    setH((s) => {
      const doc = typeof next === "function" ? next(s.doc) : next;
      if (doc === s.doc) return s;
      return merge ? { ...s, doc, future: [] } : { past: [...s.past, s.doc].slice(-LIMIT), doc, future: [] };
    });
  }, []);
  const begin = useCallback(() => {
    last.current = null;
    setH((s) => ({ past: [...s.past, s.doc].slice(-LIMIT), doc: s.doc, future: [] }));
  }, []);
  const live = useCallback((next: Doc | ((d: Doc) => Doc)) => setH((s) => ({ ...s, doc: typeof next === "function" ? next(s.doc) : next })), []);
  const undo = useCallback(() => {
    last.current = null;
    setH((s) => (s.past.length ? { past: s.past.slice(0, -1), doc: s.past[s.past.length - 1], future: [s.doc, ...s.future] } : s));
  }, []);
  const redo = useCallback(() => {
    last.current = null;
    setH((s) => (s.future.length ? { past: [...s.past, s.doc], doc: s.future[0], future: s.future.slice(1) } : s));
  }, []);
  const reset = useCallback((doc: Doc) => {
    last.current = null;
    setH({ past: [], doc, future: [] });
  }, []);
  return { doc: h.doc, canUndo: h.past.length > 0, canRedo: h.future.length > 0, commit, begin, live, undo, redo, reset };
}
