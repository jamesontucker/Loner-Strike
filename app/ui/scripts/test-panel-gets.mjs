// Every request our pages make sends what its route requires (QA4-CORE F2: the supporters'
// settings GET refused the page because getJson didn't send X-Panel).
// 1. The app's GET routes that answer only our own pages (their handler checks from_panel) are
//    read from the Rust source.
// 2. Every fetch() in the UI to the app sends "X-Panel" (it costs nothing on a route that
//    doesn't need it, and a new panel-only GET can't be missed); only fetches to other sites are
//    left out.
// 3. No panel-only GET is loaded in a way that can't send a header (<img src>, url(), EventSource,
//    a link).
// Run: node scripts/test-panel-gets.mjs
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ui = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const rust = join(ui, "..", "src-tauri", "src");
const walk = (d, ext) =>
  readdirSync(d).flatMap((n) => {
    const p = join(d, n);
    return statSync(p).isDirectory() ? walk(p, ext) : ext.some((e) => n.endsWith(e)) ? [p] : [];
  });
let failed = 0;
const fail = (m) => {
  failed++;
  console.error("FAIL " + m);
};

// 1. panel-only GETs: .route("/path", get(handler)...) whose handler checks from_panel
const handlers = new Map(); // name -> body
const routes = []; // [path, handler]
for (const f of walk(rust, [".rs"])) {
  const s = readFileSync(f, "utf8");
  for (const m of s.matchAll(/\.route\(\s*"([^"]+)",\s*get\(\s*([A-Za-z0-9_:]+)/g)) routes.push([m[1], m[2].split("::").pop()]);
  const fns = [...s.matchAll(/\n(?:pub(?:\([a-z]+\))? )?async fn ([A-Za-z0-9_]+)\s*[<(]/g)];
  fns.forEach((m, i) => {
    const end = i + 1 < fns.length ? fns[i + 1].index : s.length;
    handlers.set(m[1], (handlers.get(m[1]) ?? "") + s.slice(m.index, end));
  });
}
const panelGets = routes.filter(([, h]) => /from_panel\(/.test((handlers.get(h) ?? "").split(/\n\}\n/)[0])).map(([p]) => p);
if (routes.length < 50) fail(`only ${routes.length} GET routes found in the Rust source: the scan is broken`);
if (!panelGets.includes("/api/admins/supporters")) fail("the supporters' settings GET should be found as panel-only");

// 2. every fetch to the app sends X-Panel
const sources = walk(join(ui, "src"), [".ts", ".tsx"]).filter((f) => !/\.test\.tsx?$/.test(f));
// fetches that never reach the app (another site's address), by file and the first argument
const OTHER_SITES = [["views/Radio.tsx", "s.now"]];
let fetches = 0;
for (const f of sources) {
  const s = readFileSync(f, "utf8");
  const rel = relative(join(ui, "src"), f);
  for (const m of s.matchAll(/(?<![A-Za-z0-9_.])fetch\(/g)) {
    // the call's text up to its closing parenthesis
    let depth = 0;
    let i = m.index + "fetch".length;
    for (; i < s.length; i++) {
      if (s[i] === "(") depth++;
      else if (s[i] === ")" && --depth === 0) break;
    }
    const call = s.slice(m.index, i + 1);
    const first = call.slice(6).split(/[,)]/)[0].trim();
    if (OTHER_SITES.some(([file, arg]) => rel === file && first === arg)) continue;
    fetches++;
    if (!call.includes('"X-Panel"')) {
      const line = s.slice(0, m.index).split("\n").length;
      fail(`${rel}:${line} fetches without X-Panel: ${call.split("\n")[0].slice(0, 120)}`);
    }
  }
}
if (fetches < 20) fail(`only ${fetches} fetches found in the UI: the scan is broken`);

// 3. no panel-only GET where no header can go
const asPattern = (p) => p.replace(/[.*+?^$()|[\]\\]/g, "\\$&").replace(/\/[:*][A-Za-z_]+|\/\{[A-Za-z_*]+\}/g, "/[^\"'`)\\s]+");
for (const f of sources) {
  const s = readFileSync(f, "utf8");
  const rel = relative(join(ui, "src"), f);
  for (const p of panelGets) {
    const re = new RegExp(`(src=|href=|url\\(|EventSource\\(|\\.src\\s*=)\\s*[{"'\`]*${asPattern(p)}(["'\`?]|$)`, "m");
    if (re.test(s)) fail(`${rel} loads the panel-only GET ${p} without a header (src/href/url()/EventSource)`);
  }
}

console.log(`${routes.length} GET routes, ${panelGets.length} answer only our pages; ${fetches} fetches checked`);
if (failed) {
  console.error(`${failed} problem(s)`);
  process.exit(1);
}
console.log("ok");
