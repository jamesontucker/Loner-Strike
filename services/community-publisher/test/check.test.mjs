// Runs the Worker's checks and scripts/community-index-check.py on the same cases: both must agree.
// node --test services/community-publisher/test/
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { check, massDrop, Refused } from "../src/check.js";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const py = join(repo, "scripts", "community-index-check.py");
const item = (n, status = "listed") => ({ kind: "map", model: "Mod", gb: 1000 + n, status, name: `map ${n}` });
const idx = (items, extra = {}) => ({ schema: 1, updated: 1, revoked: [], verifiedAuthors: [], items, ...extra });
const enc = (o) => new TextEncoder().encode(typeof o === "string" ? o : JSON.stringify(o));
const sha = (b) => createHash("sha256").update(b).digest("hex");

async function js(bytes, shaText, header, live) {
  try {
    const { doc, sha: s } = await check(bytes, shaText, header);
    if (live) {
      if (sha(live) === s) return 3;
      let l = null; try { l = JSON.parse(new TextDecoder().decode(live)); } catch {}
      if (l && typeof l === "object" && massDrop(l, doc).refused) return 1;
    }
    return 0;
  } catch (e) { if (e instanceof Refused) return 1; throw e; }
}
function python(bytes, shaText, header, live) {
  const d = mkdtempSync(join(tmpdir(), "lsidx-"));
  writeFileSync(join(d, "n.json"), bytes); writeFileSync(join(d, "n.sha"), shaText);
  const args = [py, join(d, "n.json"), join(d, "n.sha"), header];
  if (live) { writeFileSync(join(d, "l.json"), live); args.push(join(d, "l.json")); }
  try { execFileSync("python3", ["-I", ...args], { stdio: "pipe" }); return 0; } catch (e) { return e.status; }
}

const good = enc(idx([item(1), item(2)]));
const cases = {
  "good, nothing live": [good, `${sha(good)}  index.json\n`, sha(good), null, 0],
  "same as live": [good, sha(good), sha(good), good, 3],
  "sha mismatch": [good, "0".repeat(64), sha(good), null, 1],
  "header mismatch": [good, sha(good), "0".repeat(64), null, 1],
  "not json": [enc("nope"), sha(enc("nope")), sha(enc("nope")), null, 1],
  "schema 2": (() => { const b = enc(idx([], { schema: 2 })); return [b, sha(b), sha(b), null, 1]; })(),
  "bad entry": (() => { const b = enc(idx([{ ...item(1), gb: 0 }])); return [b, sha(b), sha(b), null, 1]; })(),
  "bad revoked": (() => { const b = enc(idx([], { revoked: ["mod:0"] })); return [b, sha(b), sha(b), null, 1]; })(),
  "float gb": (() => { const b = enc('{"schema":1,"revoked":[],"items":[{"kind":"map","model":"Mod","gb":5.5,"status":"listed","name":"x"}]}'); return [b, sha(b), sha(b), null, 1]; })(),
  "4 of 10 drop": (() => { const live = enc(idx([...Array(10)].map((_, i) => item(i)))); const b = enc(idx([...Array(6)].map((_, i) => item(i)))); return [b, sha(b), sha(b), live, 1]; })(),
  "1 of 3 drop ok": (() => { const live = enc(idx([item(1), item(2), item(3)])); const b = enc(idx([item(1), item(2)])); return [b, sha(b), sha(b), live, 0]; })(),
  "2 of 4 drop": (() => { const live = enc(idx([1, 2, 3, 4].map((n) => item(n)))); const b = enc(idx([item(1), item(2)])); return [b, sha(b), sha(b), live, 1]; })(),
};
for (const [name, [b, s, h, live, want]] of Object.entries(cases)) {
  test(name, async () => {
    assert.equal(await js(b, s, h, live), want, "worker");
    assert.equal(python(b, s, h, live), want, "python");
  });
}
