// Lays out every Game Mode Studio template and snippet the way Tidy up does (dagre, comment boxes fitted
// round their nodes), with node sizes estimated as drawn. Run after changing a template:
//   npm run tidy:studio

import { build } from "esbuild";
import { readdirSync, readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(root, "src/views/tools/studio/templates");
const out = mkdtempSync(join(tmpdir(), "studio-tidy-"));
const raw = {
  name: "raw",
  setup(b) {
    b.onResolve({ filter: /\?raw$/ }, (a) => ({ path: resolve(a.resolveDir, a.path.slice(0, -4)), namespace: "raw" }));
    b.onLoad({ filter: /.*/, namespace: "raw" }, (a) => ({ contents: `export default ${JSON.stringify(readFileSync(a.path, "utf8"))};`, loader: "js" }));
  },
};
try {
  await build({ entryPoints: [join(root, "src/views/tools/studio/layout.ts")], outfile: join(out, "layout.mjs"), bundle: true, platform: "node", format: "esm", plugins: [raw], logLevel: "warning" });
  const { tidyEstimated } = await import(pathToFileURL(join(out, "layout.mjs")).href);
  const snippets = join(root, "src/views/tools/studio/snippets");
  for (const [d, ext] of [[dir, ".graph.json"], [snippets, ".snippet.json"]]) {
    for (const f of readdirSync(d).filter((f) => f.endsWith(ext))) {
      const g = JSON.parse(readFileSync(join(d, f), "utf8"));
      writeFileSync(join(d, f), JSON.stringify(tidyEstimated(g), null, 1) + "\n");
      console.log("tidied", f);
    }
  }
} finally {
  rmSync(out, { recursive: true, force: true });
}
