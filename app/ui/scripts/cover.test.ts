// MCP: the cover card's text layout (src/views/tools/community/coverCard.ts).
//   node scripts/test-cover.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { initials, wrap } from "../src/views/tools/community/coverCard";

test("the emblem's initials", () => {
  assert.equal(initials("Last Stand"), "LS");
  assert.equal(initials("juggernaut"), "JU");
  assert.equal(initials("Zoë's Rush"), "ZR");
  assert.equal(initials("  "), "?");
});

test("long names and lines wrap and end with …", () => {
  const fits = (s: string) => s.length <= 12;
  assert.deepEqual(wrap("Floor is lava", fits, 2), ["Floor is", "lava"]);
  const lines = wrap("one two three four five six seven eight nine ten", fits, 2);
  assert.equal(lines.length, 2);
  assert.ok(lines[1].endsWith("…"), lines[1]);
  assert.ok(lines.every((l) => fits(l)), JSON.stringify(lines));
  assert.deepEqual(wrap("", fits, 2), []);
});
