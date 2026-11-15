// Runs the map editor selection pacing tests (scripts/selection.test.ts) without a test
// framework: esbuild (already here for Vite) bundles them, Node's own test runner runs them.
//   node scripts/test-selection.mjs

import { build } from "esbuild";
import { readdirSync, readFileSync, mkdtempSync, rmSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tests = [join(root, "scripts/selection.test.ts")];
const out = mkdtempSync(join(tmpdir(), "selection-tests-"));
// Vite's "?raw" imports: the file's text
const raw = {
  name: "raw",
  setup(b) {
    b.onResolve({ filter: /\?raw$/ }, (a) => ({ path: resolve(a.resolveDir, a.path.slice(0, -4)), namespace: "raw" }));
    b.onLoad({ filter: /.*/, namespace: "raw" }, (a) => ({ contents: `export default ${JSON.stringify(readFileSync(a.path, "utf8"))};`, loader: "js" }));
  },
};
try {
  await build({ entryPoints: tests, outdir: out, bundle: true, platform: "node", format: "esm", outExtension: { ".js": ".mjs" }, plugins: [raw], logLevel: "warning" });
  const files = readdirSync(out).map((f) => join(out, f));
  const r = spawnSync(process.execPath, ["--test", ...files], { stdio: "inherit" });
  process.exitCode = r.status ?? 1;
} finally {
  rmSync(out, { recursive: true, force: true });
}
