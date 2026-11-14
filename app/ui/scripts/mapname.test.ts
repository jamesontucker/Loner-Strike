// P5 (QA4-A): map names the Compile panel refuses before a compile (src/views/tools/editor/mapName.ts).
//   node scripts/test-mapname.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { mapNameProblem } from "../src/views/tools/editor/mapName";

test("Windows device names, names of only _ and -, and the usual rules", () => {
  for (const n of ["con", "CON", "nul", "Aux", "prn", "com1", "COM9", "lpt3"]) assert.match(mapNameProblem(n), /Windows keeps that name/, n);
  for (const n of ["_", "-", "__-"]) assert.match(mapNameProblem(n), /at least one letter or number/, n);
  for (const n of ["a b", "../x", "x".repeat(32)]) assert.match(mapNameProblem(n), /letters, numbers, _ and -/, n);
  for (const n of ["", "con_rats", "com10", "aux_1", "nul2x", "de_dust2", "a"]) assert.equal(mapNameProblem(n), "", n);
});
