import { useCallback, useEffect, useRef, useState } from "react";
import type { GameState } from "./types";

// Commands only take plain words; the server checks the same rule.
export const SAFE = /^[A-Za-z0-9_.\-]{1,64}$/;

// The dedicated server the Servers area has open; null is the player's own game.
// Every request names it (X-Target), so nothing is switched behind the player's back.
let TARGET: string | null = null;
export function setTarget(id: string | null) {
  TARGET = id;
}
/** For URLs the browser loads by itself (images). */
export function withTarget(url: string) {
  return TARGET ? `${url}${url.includes("?") ? "&" : "?"}target=${encodeURIComponent(TARGET)}` : url;
}
const targetHeader = (): Record<string, string> => (TARGET ? { "X-Target": TARGET } : {});

export async function getJson<T>(path: string): Promise<T> {
  const r = await fetch(path, { credentials: "same-origin", headers: targetHeader() });
  if (!r.ok && r.status !== 400) throw new Error(`${path}: ${r.status}`);
  return r.json() as Promise<T>;
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
  const busy = useRef(false);

  const refresh = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    try {
      setState(await getJson<GameState>("/api/state"));
    } catch {
      setState({ online: false, reason: "Lost contact with the app's server. Restart Loner-Strike." });
    } finally {
      busy.current = false;
    }
  }, []);

  useEffect(() => {
    refresh();
    const t = setInterval(() => !document.hidden && refresh(), 2000);
    const vis = () => !document.hidden && refresh();
    document.addEventListener("visibilitychange", vis);
    return () => {
      clearInterval(t);
      document.removeEventListener("visibilitychange", vis);
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
