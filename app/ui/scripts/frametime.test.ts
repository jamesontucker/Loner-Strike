// MCP: the map editor's frame-time readout math (src/views/tools/editor/FrameTime.tsx).
//   node scripts/test-frametime.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { percentile, summarize } from "../src/views/tools/editor/FrameTime";

test("percentiles of frame times", () => {
  const s = Array.from({ length: 100 }, (_, i) => i + 1);
  assert.equal(percentile(s, 50), 50);
  assert.equal(percentile(s, 95), 95);
  assert.equal(percentile([], 95), 0);
});

test("60 smooth frames and one long one over a second", () => {
  const frames = [...Array(59).fill(16.7), 120];
  const r = summarize(frames, 1000);
  assert.equal(Math.round(r.fps), 60);
  assert.equal(r.p50, 16.7);
  assert.equal(r.p95, 16.7);
  assert.equal(r.worst, 120);
});
