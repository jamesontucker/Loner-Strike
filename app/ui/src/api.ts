import { useCallback, useEffect, useMemo, useRef } from "react";
import { currentTarget, isAbort, targetSignal, useActivityStore } from "./store";

export { currentTarget, isAbort };

// Commands only take plain words; the server checks the same rule.
export const SAFE = /^[A-Za-z0-9_.\-]{1,64}$/;

/** The app window on this computer (not a phone over Wi-Fi). Some things only work here. */
export const ON_THIS_COMPUTER = ["127.0.0.1", "localhost"].includes(location.hostname);

/** The open target as part of an action key ("local" for the player's own game), so the
 *  same action on two servers doesn't share a spinner. */
export const targetKey = () => currentTarget() ?? "local";
/** For URLs the browser loads by itself (images). */
export function withTarget(url: string) {
  const t = currentTarget();
  return t ? `${url}${url.includes("?") ? "&" : "?"}target=${encodeURIComponent(t)}` : url;
}
const targetHeader = (): Record<string, string> => {
  const t = currentTarget();
  return t ? { "X-Target": t } : {};
};

/** GET as JSON for the open target. Opening another server cancels it (an AbortError),
 *  so a reply meant for the previous one can't land on the new one's page. */
export async function getJson<T>(path: string): Promise<T> {
  const { signal, done } = targetSignal();
  try {
    const r = await fetch(path, { credentials: "same-origin", headers: targetHeader(), signal });
    if (!r.ok && r.status !== 400) throw new Error(`${path}: ${r.status}`);
    return (await r.json()) as T;
  } finally {
    done();
  }
}

/** GET for a set target (the one an action started on), without the switch check: for
 *  following a job to its end, which must finish even if another server is opened. */
export async function getJsonAt<T>(path: string, target: string | null, signal?: AbortSignal): Promise<T> {
  const r = await fetch(path, { credentials: "same-origin", headers: target ? { "X-Target": target } : {}, signal });
  if (!r.ok && r.status !== 400) throw new Error(`${path}: ${r.status}`);
  return (await r.json()) as T;
}

/** The setup job (one for the whole app), or null if the app didn't answer. */
export const setupJob = <T>() => getJsonAt<T>("/api/setup/job", null).catch(() => null);

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
  if (r.error || !r.job) return { ok: false, message: r.error || "Couldn't add that file." };
  const target = currentTarget();
  for (let i = 0; i < 600; i++) {
    await new Promise((res) => setTimeout(res, 800));
    const jobs = await getJsonAt<{ id: string; state: string; message?: string }[]>("/api/gb/jobs", target).catch(() => []);
    const j = Array.isArray(jobs) ? jobs.find((x) => x.id === r.job) : undefined;
    if (j && (j.state === "done" || j.state === "error")) return { ok: j.state === "done", message: j.message || (j.state === "done" ? "Added" : "Couldn't add that file.") };
  }
  return { ok: false, message: "Still adding. Check Get more for progress." };
}

/** Saves of one settings object, quickly one after another: each builds on the last one
 *  sent rather than on what's on screen, so two quick changes don't undo each other. */
export function useQueuedSave<S>() {
  const want = useRef<S | null>(null);
  // one save at a time, in order, so the last reply is the last change
  const chain = useRef<Promise<unknown>>(Promise.resolve());
  return useCallback(async <T,>(current: S, change: (s: S) => S, send: (s: S) => Promise<T>): Promise<T> => {
    const next = change(want.current ?? current);
    want.current = next;
    const mine = chain.current.catch(() => null).then(() => send(next));
    chain.current = mine;
    try {
      return await mine;
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

/** Calls `fn` now, then again `ms` after each call has finished, so a slow answer never
 *  stacks up a second request. A hidden window stops asking and asks again as soon as it's
 *  shown. Changing `deps` (or `on`) starts it over. */
export function usePoll(fn: () => unknown, ms: number, deps: unknown[] = [], on = true) {
  const call = useStable(fn);
  useEffect(() => {
    if (!on) return;
    let timer = 0;
    let busy = false;
    let stopped = false;
    const step = async () => {
      timer = 0;
      if (stopped || busy || document.hidden) return;
      busy = true;
      try {
        await call();
      } catch {
        /* the next one tries again */
      } finally {
        busy = false;
      }
      if (!stopped && !document.hidden) timer = window.setTimeout(step, ms);
    };
    const shown = () => {
      if (!document.hidden && !timer) step();
    };
    step();
    document.addEventListener("visibilitychange", shown);
    return () => {
      stopped = true;
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", shown);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [call, ms, on, ...deps]);
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

// ---- what's in progress, for the whole app (store.ts) -------------------------------

/** Everything in progress: [key, label] */
export function useActivity(): [string, string][] {
  const running = useActivityStore((s) => s.running);
  return useMemo(() => Object.entries(running), [running]);
}

/** Runs an action unless the same one is still going; `busy(key)` says whether it is, on
 *  any page. `label` is what the top bar says while it runs ("Saving…"). */
export function useActions() {
  const running = useActivityStore((s) => s.running);
  const run = useCallback(async (key: string, fn: () => Promise<unknown>, label = "") => {
    const { start, end } = useActivityStore.getState();
    if (!start(key, label)) return;
    try {
      await fn();
    } catch (e) {
      // a request cancelled by opening another server isn't a failure; anything else
      // gets a word from the toast provider: a button never fails silently
      if (!isAbort(e)) window.dispatchEvent(new CustomEvent("ls-error", { detail: "Couldn't reach the app. Try again." }));
    } finally {
      end(key);
    }
  }, []);
  return { run, busy: (key: string) => key in running };
}
