import { useCallback, useEffect, useRef, useState } from "react";
import type { GameState } from "./types";

// Commands only take plain words; the server checks the same rule.
export const SAFE = /^[A-Za-z0-9_.\-]{1,64}$/;

/** The app window on this computer (not a phone over Wi-Fi). Some things only work here. */
export const ON_THIS_COMPUTER = ["127.0.0.1", "localhost"].includes(location.hostname);

// The dedicated server the Servers area has open; null is the player's own game.
// Every request names it (X-Target), so nothing is switched behind the player's back.
let TARGET: string | null = null;
// Bumped on every switch, so replies meant for the previous target are dropped.
let GEN = 0;
const stale = () => new Promise<never>(() => {});
export function setTarget(id: string | null) {
  if (id !== TARGET) GEN++;
  TARGET = id;
  window.dispatchEvent(new Event("ls-target"));
}
export const targetGen = () => GEN;
/** For URLs the browser loads by itself (images). */
export function withTarget(url: string) {
  return TARGET ? `${url}${url.includes("?") ? "&" : "?"}target=${encodeURIComponent(TARGET)}` : url;
}
const targetHeader = (): Record<string, string> => (TARGET ? { "X-Target": TARGET } : {});

/** GET as JSON. A reply that arrives after the target changed never resolves, so it can't
 *  land in the newly opened server's page. */
export async function getJson<T>(path: string): Promise<T> {
  const g = GEN;
  try {
    const r = await fetch(path, { credentials: "same-origin", headers: targetHeader() });
    if (!r.ok && r.status !== 400) throw new Error(`${path}: ${r.status}`);
    const j = (await r.json()) as T;
    return g === GEN ? j : stale();
  } catch (e) {
    if (g !== GEN) return stale();
    throw e;
  }
}

export async function postJson<T>(path: string, body: unknown = {}): Promise<T> {
  const r = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    // Only our own pages can send this header without a CORS preflight
    headers: { "Content-Type": "application/json", "X-Panel": "1", ...targetHeader() },
    body: JSON.stringify(body),
  });
  return r.json() as Promise<T>;
}

type CmdResult = { ok: boolean; output?: string[]; error?: string };

/** Sends a Game Mode Menu command; returns the game's reply as one line, or throws its error. */
export async function cmd(command: string, args: string[] = []): Promise<string> {
  if (command !== "gm_announce" && !args.every((a) => SAFE.test(a))) throw new Error("That value can't be sent to the game.");
  let res: CmdResult;
  try {
    res = await postJson<CmdResult>("/api/cmd", { cmd: command, args });
  } catch {
    throw new Error("Couldn't reach the game.");
  }
  if (!res.ok) throw new Error(res.error || "The game didn't accept that.");
  return (res.output || []).map((l) => l.replace(/^\[Game Mode\]\s*/, "")).filter(Boolean).join(" ");
}

/** Live game state, polled every 2 seconds while the window is visible. */
export function useGameState() {
  const [state, setState] = useState<GameState | null>(null);
  // The target generation a request is in flight for; a switch lets the next one through.
  const busy = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    const g = targetGen();
    if (busy.current === g) return;
    busy.current = g;
    try {
      setState(await getJson<GameState>("/api/state"));
    } catch {
      setState({ online: false, reason: "Lost contact with the app's server. Restart Loner-Strike." });
    } finally {
      if (busy.current === g) busy.current = null;
    }
  }, []);

  useEffect(() => {
    refresh();
    const t = setInterval(() => !document.hidden && refresh(), 2000);
    const vis = () => !document.hidden && refresh();
    document.addEventListener("visibilitychange", vis);
    window.addEventListener("ls-target", vis);
    return () => {
      clearInterval(t);
      document.removeEventListener("visibilitychange", vis);
      window.removeEventListener("ls-target", vis);
    };
  }, [refresh]);

  return { state, refresh };
}

/** "values"/"labels" as the plugin sends them: labels split on | when present, else on spaces. */
export function options(values: string, labels: string): [string, string][] {
  const v = values.split(/\s+/).filter(Boolean);
  const l = labels.includes("|") ? labels.split("|") : labels.split(/\s+/).filter(Boolean);
  return v.map((x, i) => [x, l[i] ?? x]);
}

export function sameValue(a: string, b: string) {
  if (String(a).toLowerCase() === String(b).toLowerCase()) return true;
  const fa = parseFloat(a);
  const fb = parseFloat(b);
  return !isNaN(fa) && !isNaN(fb) && /^[\d.]/.test(a) && /^[\d.]/.test(b) && Math.abs(fa - fb) < 0.01;
}

export function clock(s: number) {
  if (s < 0) return "-";
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Picks a file on this computer and installs it into a section (maps, skins, sounds...),
 *  then waits for it. Resolves with the result message, or null if nothing was picked. */
export async function addFromFile(section: string): Promise<{ ok: boolean; message: string } | null> {
  const r = await postJson<{ ok?: boolean; job?: string; error?: string; cancelled?: boolean }>("/api/gb/add-file", { section }).catch(() => ({ error: "Couldn't reach the app." }) as { error: string; job?: string; cancelled?: boolean });
  if (r.cancelled) return null;
  if (r.error || !r.job) return { ok: false, message: r.error || "That didn't work." };
  for (let i = 0; i < 600; i++) {
    await new Promise((res) => setTimeout(res, 800));
    const jobs = await getJson<{ id: string; state: string; message?: string }[]>("/api/gb/jobs").catch(() => []);
    const j = Array.isArray(jobs) ? jobs.find((x) => x.id === r.job) : undefined;
    if (j && (j.state === "done" || j.state === "error")) return { ok: j.state === "done", message: j.message || (j.state === "done" ? "Added" : "That didn't work.") };
  }
  return { ok: false, message: "Still adding. Check Get more for progress." };
}

/** Saves of one settings object, quickly one after another: each builds on the last one
 *  sent rather than on what's on screen, so two quick changes don't undo each other. */
export function useQueuedSave<S>() {
  const want = useRef<S | null>(null);
  return useCallback(async <T,>(current: S, change: (s: S) => S, send: (s: S) => Promise<T>): Promise<T> => {
    const next = change(want.current ?? current);
    want.current = next;
    try {
      return await send(next);
    } finally {
      if (want.current === next) want.current = null;
    }
  }, []);
}

/** A function that keeps the same identity across renders but always calls the latest
 *  version, so an inline callback prop doesn't restart a polling effect. */
export function useStable<A extends unknown[], R>(fn: (...a: A) => R) {
  const latest = useRef(fn);
  latest.current = fn;
  return useCallback((...a: A) => latest.current(...a), []);
}

/** Runs an action unless the same one is still going, so a double click sends it once. */
export function useOnce() {
  const going = useRef(new Set<string>());
  return useCallback(async (key: string, fn: () => Promise<unknown>) => {
    if (going.current.has(key)) return;
    going.current.add(key);
    try {
      await fn();
    } finally {
      going.current.delete(key);
    }
  }, []);
}
