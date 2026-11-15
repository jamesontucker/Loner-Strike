// MCP: the page's pacing when following an agent's selection (src/views/tools/editor/selectionFollow.ts).
//   node scripts/test-selection.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { BACKOFF_MS, MIN_GAP_MS, QUICK_EMPTY_MS, follow, waitBefore, type Answer } from "../src/views/tools/editor/selectionFollow";

/** A fake clock and an app that answers each look at once with the next scripted answer. */
function run(answers: Answer[], maxLooks = answers.length) {
  let t = 0;
  let seen = 5;
  const looks: number[] = [];
  const applied: number[] = [];
  return follow({
    look: async () => {
      looks.push(t);
      return looks.length <= answers.length ? answers[looks.length - 1] : { seq: seen };
    },
    pause: async (ms) => {
      t += ms;
    },
    now: () => t,
    going: () => looks.length < maxLooks,
    seen: () => seen,
    setSeen: (n) => {
      seen = n;
    },
    apply: (a) => applied.push(a.seq!),
  }).then(() => ({ looks, applied }));
}

const gaps = (l: number[]) => l.slice(1).map((x, i) => x - l[i]);

test("an app that answers at once with nothing new is asked at most once a second", async () => {
  const { looks } = await run(Array(40).fill({ seq: 5 }));
  assert.equal(looks.length, 40);
  for (const g of gaps(looks)) assert.ok(g >= QUICK_EMPTY_MS, `gap ${g}`);
});

test("never two looks closer than the minimum gap, even with news each time", async () => {
  const { looks, applied } = await run([6, 7, 8, 9, 10].map((n) => ({ seq: n, by: "agent" })));
  for (const g of gaps(looks)) assert.ok(g >= MIN_GAP_MS, `gap ${g}`);
  assert.deepEqual(applied, [6, 7, 8, 9, 10]);
});

test("errors back off 1, 2, 4, 8, then 10 s, and a good answer resets it", async () => {
  const err = { error: "The app didn't answer." };
  const { looks } = await run([err, err, err, err, err, err, { seq: 6, by: "page" }, err, { seq: 7, by: "agent" }]);
  // six failures (the sixth waits 10 s again), a good answer, the next look right after the gap, one more failure
  assert.deepEqual(gaps(looks), [...BACKOFF_MS, 10000, MIN_GAP_MS, 1000]);
});

test("the page's own selections are seen but not applied", async () => {
  const { applied } = await run([{ seq: 6, by: "page" }, { seq: 7, by: "agent" }, { seq: 7, by: "agent" }]);
  assert.deepEqual(applied, [7]);
});

test("an aborted look stops following", async () => {
  const { looks } = await run([{ seq: 6 }, null, { seq: 8 }], 10);
  assert.equal(looks.length, 2);
});

test("waitBefore", () => {
  assert.equal(waitBefore(-Infinity, 0, 0), 0);
  assert.equal(waitBefore(0, 100, 0), MIN_GAP_MS - 100);
  assert.equal(waitBefore(0, 100, 0, true), QUICK_EMPTY_MS - 100);
  assert.equal(waitBefore(0, 30000, 0, true), 0);
  assert.equal(waitBefore(0, 0, 3), 4000);
  assert.equal(waitBefore(0, 0, 99), 10000);
});
