// MCP: the editor's selection, shared with AI agents. When the selection, the tool or the view
// changes, the page tells the app (after 150 ms of quiet), so an agent asked to "change this"
// knows what "this" is (`editor_selection`). When an agent selects something (`editor_select`),
// the page selects it too (a long look at the app, not a poll: no idle traffic), so the person sees what it changed or found.
// Server: app/src-tauri/src/tools/editor/selection.rs. Only reports and applies the store's
// selection; how it's drawn is the scene's business.

import { useEffect } from "react";
import { post, tryGet } from "../common/api";
import { follow } from "./selectionFollow";
import { facesOf, useEditor, type Selection } from "./store";

const PATH = "/api/tools/editor/selection";
const QUIET_MS = 150;

type Reported = { by?: string; seq?: number; brushes?: number[]; entities?: number[]; faces?: [number, number][]; error?: string };

const keyOf = (sel: Selection, tool: string, view: string) =>
  JSON.stringify([sel.brushes, sel.entities, facesOf(sel).map((f) => [f.brush, f.face]), tool, view]);

/** Keeps the app's copy of the selection current while the editor page is open. */
export function useSelectionSync() {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let sent = "";
    let seen = 0;
    let stopped = false;
    const send = () => {
      timer = null;
      const s = useEditor.getState();
      if (!s.doc) return;
      const key = keyOf(s.sel, s.tool, s.view);
      if (key === sent) return;
      sent = key;
      void post<Reported>(PATH, { brushes: s.sel.brushes, entities: s.sel.entities, faces: facesOf(s.sel).map((f) => [f.brush, f.face]), tool: s.tool, view: s.view }).then((r) => {
        if (typeof r.seq === "number") seen = Math.max(seen, r.seq);
      });
    };
    const unsub = useEditor.subscribe((s, p) => {
      if (s.sel === p.sel && s.tool === p.tool && s.view === p.view && s.doc?.docId === p.doc?.docId) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(send, QUIET_MS);
    });
    // an agent's selection: selected here too. A long look (the app answers when the selection
    // is newer than `seen`, or after 25 s), only while the page is in view; paced and backed off
    // in selectionFollow.ts.
    let ctl: AbortController | null = null;
    let running = false;
    const listen = async () => {
      if (running) return;
      running = true;
      try {
        await follow({
          look: async (after) => {
            // nothing open: answer "no news" so the pacing waits before looking again
            if (!useEditor.getState().doc) return { seq: seen };
            ctl = new AbortController();
            const r = await tryGet<Reported>(`${PATH}?after=${after}`, ctl.signal).catch(() => null);
            ctl = null;
            return stopped ? null : r;
          },
          pause: (ms) => new Promise((r) => setTimeout(r, ms)),
          now: () => Date.now(),
          going: () => !stopped && !document.hidden,
          seen: () => seen,
          setSeen: (n) => {
            seen = n;
          },
          apply: (a) => {
            const r = a as Reported;
            const faces = (r.faces ?? []).map(([brush, face]) => ({ brush, face }));
            const sel: Selection = { brushes: r.brushes ?? [], entities: r.entities ?? [], face: faces[0] ?? null, faces };
            const s = useEditor.getState();
            // (not sent back: it came from there)
            sent = keyOf(sel, s.tool, s.view);
            s.select(sel);
          },
        });
      } finally {
        running = false;
      }
    };
    const wake = () => {
      if (!document.hidden) void listen();
      else ctl?.abort();
    };
    void listen();
    document.addEventListener("visibilitychange", wake);
    window.addEventListener("focus", wake);
    send();
    return () => {
      stopped = true;
      unsub();
      ctl?.abort();
      document.removeEventListener("visibilitychange", wake);
      window.removeEventListener("focus", wake);
      if (timer) clearTimeout(timer);
    };
  }, []);
}
