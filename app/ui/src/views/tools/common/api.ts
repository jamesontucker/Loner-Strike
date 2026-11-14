// One way for every Tools page to talk to the app (app/src-tauri/src/tools/*). Every tools
// route is on this computer only and answers in plain words when something goes wrong:
// JSON routes say {error} with 200 (400 for a bad request), binary routes (meshes, pictures)
// say {error} with an error status. These helpers turn all of that, and a request that
// never arrived, into one of two shapes:
//
// - get / getBytes throw a ToolsError whose message can go straight on the page;
// - post / tryGet never throw: the answer carries `error` instead (what a button handler wants).
//
// busy() runs slow work with the app's spinner and label in the top bar.

import { useCallback, useRef, useState } from "react";
import { isAbort } from "../../../api";
import { useActivityStore } from "../../../store";

/** A request the app answered with an error (its own words when it gave some). */
export class ToolsError extends Error {
  /** status: the HTTP status (0: never arrived); json: the app answered in its own words */
  constructor(message: string, readonly status = 0, readonly json = false) {
    super(message);
  }
}

export type Err = { error?: string };

export const NOT_REACHED = "Couldn't reach the app. Try again.";
const WENT_WRONG = "Something went wrong. Try again.";

/** The words to show for anything thrown by a request (or by our own code). */
export function errorText(e: unknown, fallback = WENT_WRONG): string {
  if (e instanceof ToolsError) return e.message;
  if (e instanceof Error && e.message && !/fetch|network|json|unexpected token/i.test(e.message)) return e.message;
  return fallback;
}

/** The app answered with its own words (a damaged map, a missing file): trying the same again
 *  won't help, so the page offers a way on instead of "Try again". */
export function deadEnd(e: unknown): boolean {
  return e instanceof ToolsError && e.json;
}

/** JSON routes answer {error} with 200 (like the rest of the app): that's a failure too. */
export function checked<T>(v: unknown): T {
  if (v && typeof v === "object" && !Array.isArray(v) && typeof (v as Err).error === "string") throw new ToolsError((v as { error: string }).error, 200, true);
  return v as T;
}

/** fetch that throws ToolsError (with the app's words when it gave some) unless the answer is ok. */
export async function fetchOk(path: string, init: RequestInit = {}, notFound = "That isn't there any more."): Promise<Response> {
  let r: Response;
  try {
    r = await fetch(path, { credentials: "same-origin", ...init });
  } catch (e) {
    if (isAbort(e)) throw e;
    throw new ToolsError(NOT_REACHED);
  }
  if (r.ok) return r;
  const json = (r.headers.get("content-type") ?? "").includes("json");
  let msg = "";
  if (json) {
    try {
      msg = String((await r.json()).error ?? "");
    } catch {
      /* no message */
    }
  }
  throw new ToolsError(msg || (r.status === 404 ? notFound : WENT_WRONG), r.status, json && !!msg);
}

/** GET JSON; throws ToolsError, including for a 200 {error}. */
export async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  const r = await fetchOk(path, { signal });
  let v: unknown;
  try {
    v = await r.json();
  } catch {
    throw new ToolsError(WENT_WRONG, r.status);
  }
  return checked<T>(v);
}

/** GET a binary answer (a mesh bundle, a picture); throws ToolsError. */
export async function getBytes(path: string, signal?: AbortSignal, notFound?: string): Promise<ArrayBuffer> {
  return (await fetchOk(path, { signal }, notFound)).arrayBuffer();
}

/** GET JSON that never throws: a failure comes back as {error}. Aborts still throw. */
export async function tryGet<T>(path: string, signal?: AbortSignal): Promise<T & Err> {
  try {
    const r = await fetch(path, { credentials: "same-origin", signal });
    if (!r.ok && r.status !== 400) {
      const v = await r.json().catch(() => null);
      return { error: (v && typeof v.error === "string" && v.error) || (r.status === 404 ? "That isn't there any more." : WENT_WRONG) } as T & Err;
    }
    return (await r.json()) as T & Err;
  } catch (e) {
    if (isAbort(e)) throw e;
    return { error: NOT_REACHED } as T & Err;
  }
}

/** POST JSON from our own page (X-Panel); never throws: a failure comes back as {error}. */
export async function post<T>(path: string, body: unknown = {}): Promise<T & Err> {
  try {
    const r = await fetch(path, {
      method: "POST",
      credentials: "same-origin",
      // only our own pages can send this header without a CORS preflight
      headers: { "Content-Type": "application/json", "X-Panel": "1" },
      body: JSON.stringify(body),
    });
    const v = await r.json().catch(() => null);
    // the save kept backups (safety.rs): offer "Undo" for a few seconds
    const undo = r.headers.get("x-ls-undo");
    if (undo) offerUndo(undo);
    if (v && typeof v === "object") return v as T & Err;
    return { error: r.ok ? WENT_WRONG : r.status === 404 ? "This version of the app can't do that yet." : WENT_WRONG } as T & Err;
  } catch {
    return { error: NOT_REACHED } as T & Err;
  }
}

/** After a save that kept backups: a toast with "Undo" for a few seconds (ui.tsx ToastProvider
 *  listens for ls-undoable). Undo puts every file that save changed back, then the open tool
 *  starts over on what's on disk. `range` is the X-LS-Undo header: "<from>-<to>". */
export function offerUndo(range: string) {
  const m = /^(\d+)-(\d+)$/.exec(range);
  if (!m) return;
  const run = async (): Promise<{ error?: string; message?: string }> => {
    const r = await post<{ message?: string }>("/api/safety/undo", { from: Number(m[1]), to: Number(m[2]) });
    if (!r.error) {
      // the open tool reads its files again (common/shell.tsx starts its page over)
      const { useShell } = await import("./shell");
      useShell.setState((s) => ({ n: s.n + 1 }));
      window.dispatchEvent(new CustomEvent("ls-files-restored"));
    }
    return r;
  };
  window.dispatchEvent(new CustomEvent("ls-undoable", { detail: { text: "Undo that save?", run } }));
}

/** POST that throws ToolsError on {error} (for code that already uses try/catch). */
export async function postOrThrow<T>(path: string, body: unknown = {}): Promise<T> {
  const r = await post<T>(path, body);
  if (r.error) throw new ToolsError(r.error, 200, true);
  return r as T;
}

/** Runs slow work with the app's spinner and label in the top bar (kept across pages).
 *  The same key twice at once shows one label. */
export async function busy<T>(key: string, label: string, fn: () => Promise<T>): Promise<T> {
  const { start, end } = useActivityStore.getState();
  const mine = start(key, label);
  try {
    return await fn();
  } finally {
    if (mine) end(key);
  }
}

/** A button's slow action: `working` is the label to show while it runs ("Saving…"), or null.
 *  run() ignores a second press while the first is going and returns its result. */
export function useBusy() {
  const [working, setWorking] = useState<string | null>(null);
  const going = useRef(false);
  const run = useCallback(async <T,>(label: string, fn: () => Promise<T>): Promise<T | undefined> => {
    if (going.current) return undefined;
    going.current = true;
    setWorking(label);
    try {
      return await fn();
    } finally {
      going.current = false;
      setWorking(null);
    }
  }, []);
  return { working, run };
}
