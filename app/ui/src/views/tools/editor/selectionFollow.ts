// MCP: the page following an agent's selection: a long look at the app (`?after=<seq>`), paced so
// that an app that answers at once (an older build, an error path, a proxy) can't make the page
// ask in a tight loop. No imports, so it's tested in Node (scripts/selection.test.ts).

/** At least this long between two looks, whatever the app does. */
export const MIN_GAP_MS = 250;
/** An answer with nothing new that came back faster than this (the app didn't wait): wait this long. */
export const QUICK_EMPTY_MS = 1000;
/** After a failed look: 1 s, then 2 s, 4 s, 8 s, then 10 s each time, until one works. */
export const BACKOFF_MS = [1000, 2000, 4000, 8000, 10000];

export type Answer = { seq?: number; by?: string; error?: string } | null;

export type Follow = {
  /** one look: the app's answer once its selection is newer than `after` (or it gives up); null when aborted */
  look: (after: number) => Promise<Answer>;
  pause: (ms: number) => Promise<void>;
  now: () => number;
  /** keep going? (false: stopped, page hidden, nothing open) */
  going: () => boolean;
  /** the newest selection seen so far (the page's own reports move it too) */
  seen: () => number;
  setSeen: (seq: number) => void;
  /** an agent's selection to show */
  apply: (a: NonNullable<Answer>) => void;
};

/** How long to wait before the next look: at least MIN_GAP_MS after the last one began; a
 *  second when the last brought nothing new right away (`quickEmpty`); after `failures` failed
 *  looks in a row, the backoff. */
export function waitBefore(lastStart: number, now: number, failures: number, quickEmpty = false): number {
  const gap = Math.max(0, MIN_GAP_MS - (now - lastStart), quickEmpty ? QUICK_EMPTY_MS - (now - lastStart) : 0);
  if (failures <= 0) return gap;
  return Math.max(gap, BACKOFF_MS[Math.min(failures, BACKOFF_MS.length) - 1]);
}

/** Follows until `going()` says stop or a look is aborted. */
export async function follow(f: Follow): Promise<void> {
  let failures = 0;
  let lastStart = -Infinity;
  let quickEmpty = false;
  while (f.going()) {
    const w = waitBefore(lastStart, f.now(), failures, quickEmpty);
    if (w > 0) await f.pause(w);
    if (!f.going()) break;
    lastStart = f.now();
    const r = await f.look(f.seen());
    if (r === null) break;
    quickEmpty = false;
    if (r.error || typeof r.seq !== "number") {
      failures++;
      continue;
    }
    failures = 0;
    if (r.seq <= f.seen()) {
      quickEmpty = f.now() - lastStart < QUICK_EMPTY_MS;
      continue;
    }
    f.setSeen(r.seq);
    if (r.by === "agent") f.apply(r);
  }
}
