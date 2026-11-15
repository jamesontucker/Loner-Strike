#!/usr/bin/env node
// The final Studio check's game half (docs/notes/FinalCheck.md): Studio modes in a sim server with
// bots. One CrossOver server (a clone of --sim-base, this checkout's plugins compiled fresh, the
// pinned bot brain or --brain), the graphs in scripts/finalcheck/graphs installed by the Studio's
// own code (cargo test finalcheck_install) and compiled with amxxpc. Modes always switch with
// gm_mode (writing current.txt alone leaves the old mode's plugins: QA notes 2026-11-14), and each
// switch is checked: the mode's plugin running, the bot count reached.
//
//   node scripts/finalcheck.mjs                        # every step, full length
//   node scripts/finalcheck.mjs --dry                  # every step, short (proves the harness)
//   node scripts/finalcheck.mjs --steps cover,stress   # some: install cover perf stress br
//
// Steps:
//   install  clone, compile, install the graphs (always runs first unless --reuse)
//   cover    each cover_* mode (debugger build) with 16 bots: which of this round's node kinds ran,
//            run-time errors, node errors
//   perf     Classic, then each perf_*/cover mode (plain build), at each --bots count: server frame
//            time (from stats' FPS), the server's CPU (stats) and the hlds process's CPU (ps);
//            the mode's cost is its numbers against Classic's
//   stress   each stress_* mode with 16 bots, every limit at its max: the server must keep
//            answering (longest silence), no crash or Host_Error, no bot dropped, no overflow; the
//            caps' log lines counted. stress_precache170 must be refused at install.
//            --human: waits for a person to join first (the overflow check needs a real client:
//            bots get no network messages), then checks they're still there after each run
//   br       Battle Royale squads with 16 bots on this sim, as the re-check's br-squads16 step
//
// Everything is the latest of each piece (the user's rule), checked at start and listed in the
// summary's version table; the run refuses one older than HEAD (--allow-old: a harness test only):
// the source is a worktree of HEAD (never the working copy), every plugin with its source there is
// compiled from it, the Studio plugins come from its generator, the navs and map knowledge match
// the live nav pack, the brain is the newest green ls-next3 build and says ls_mode_ver 4+. The sim
// is a fresh APFS clone of --sim-base with all of that put in (never the base as it is).
//
// Options: --sim-base NAME (default LSPLAY), --sim-name NAME (default FINALCHECK), --brain newest |
// pinned | <mp.dll> (default newest: the newest green ls-next3 CI build, the next pin's candidate;
// the sim's ls_mode_ver is read at start and must be --min-mode-ver, default 4, or more), --bots 16,24, --secs N (play time per run),
// --port 27520, --reuse (the clone an earlier --keep run left), --keep, --out DIR, --graphs a,b
// (only these graph files). Needs: outside the sandbox (CrossOver), load under 20, 8 GB free disk.
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createSocket } from "node:dgram";
import { appendFileSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { homedir, loadavg, tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const flag = (k) => args.includes(k);
const DRY = flag("--dry");
const stamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 13);
const OUT = resolve(opt("--out", join(tmpdir(), `ls-finalcheck-${stamp}`)));
const BOTTLE = join(homedir(), "Library/Application Support/CrossOver/Bottles/Steam/drive_c");
const SIMS = join(BOTTLE, "Loner-Strike/Sim");
const SIM_BASE = opt("--sim-base", "LSPLAY");
const SIM_NAME = opt("--sim-name", "FINALCHECK").replace(/[^A-Za-z0-9_-]/g, "");
const SIM = join(SIMS, SIM_NAME);
const GAMEDIR = join(SIM, "cstrike");
const AM = join(GAMEDIR, "addons/amxmodx");
const WINE = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";
const PORT = Number(opt("--port", "27520"));
const TREE = join(OUT, "tree"); // a worktree of HEAD: everything built comes from it, never the working copy
const GRAPHS = join(TREE, "scripts/finalcheck/graphs");
const TARGET = join(repo, "app/src-tauri/target"); // (the shared build folder: only the crate rebuilds)
const NAV_LIST = "https://dl.lonerstrike.com/navs/list.json";
const NAV_BASE = "https://dl.lonerstrike.com/navs/";
const START = Date.now();
const STEPS = opt("--steps", "install,cover,perf,stress,br").split(",");
const BOTS = opt("--bots", DRY ? "16" : "16,24").split(",").map(Number);
const SECS = Number(opt("--secs", DRY ? "40" : "180"));
const MAXPLAYERS = 32;
const PW = `fc${Math.random().toString(36).slice(2, 12)}`;
const qlog = join(SIM, "qconsole.log"); // (-condebug writes it beside hlds.exe)
const results = { versions: [], commit: "", sim: SIM_NAME, base: SIM_BASE, dry: DRY, install: [], cover: [], perf: [], stress: [], br: null, notes: [] };

mkdirSync(OUT, { recursive: true });
const log = (m) => {
  console.log(m);
  appendFileSync(join(OUT, "finalcheck.log"), m + "\n");
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const winPath = (p) => (p.startsWith(BOTTLE) ? "C:" + p.slice(BOTTLE.length) : "Z:" + p).replace(/\//g, "\\");
function freeGb() {
  const out = execFileSync("df", ["-k", "/"], { encoding: "utf8" }).trim().split("\n").pop().split(/\s+/);
  return Math.floor(Number(out[3]) / 1024 / 1024);
}
/** The machine's rules: wait while the load is over 20; stop under 8 GB free disk. */
async function roomToRun(where) {
  if (freeGb() < 8) throw new Error(`free disk ${freeGb()} GB (under 8 GB) before ${where}`);
  for (let i = 0; loadavg()[0] > 20 && i < 60; i++) {
    if (i === 0) log(`   (load ${loadavg()[0].toFixed(1)} over 20: waiting before ${where})`);
    await sleep(15_000);
  }
}
function run(name, cmd, argv, opts = {}) {
  const r = spawnSync(cmd, argv, { cwd: opts.cwd ?? OUT, env: { ...process.env, ...(opts.env ?? {}) }, encoding: "utf8", maxBuffer: 1 << 30, timeout: opts.timeout ?? 3600_000 });
  const out = (r.stdout ?? "") + (r.stderr ?? "");
  appendFileSync(join(OUT, `${name}.log`), `$ ${cmd} ${argv.join(" ")}\n${out}\n`);
  return { code: r.status ?? 1, out };
}
function removeAll(p) {
  if (!existsSync(p)) return;
  spawnSync("chmod", ["-R", "u+w", p]);
  rmSync(p, { recursive: true, force: true });
}
function copyOver(from, to) {
  if (!existsSync(from)) return;
  for (const f of readdirSync(from)) {
    const a = join(from, f);
    const b = join(to, f);
    if (statSync(a).isDirectory()) {
      mkdirSync(b, { recursive: true });
      copyOver(a, b);
    } else copyFileSync(a, b);
  }
}

// ---- the server ------------------------------------------------------------------------------
function hldsPids() {
  const r = spawnSync("pgrep", ["-f", `^C:\\\\Loner-Strike\\\\Sim\\\\${SIM_NAME}\\\\hlds\\.exe`], { encoding: "utf8" });
  return (r.stdout ?? "").split("\n").filter(Boolean);
}
function stopServer() {
  for (const p of hldsPids()) spawnSync("kill", [p]);
  for (let i = 0; i < 10 && hldsPids().length; i++) spawnSync("sleep", ["1"]);
  for (const p of hldsPids()) spawnSync("kill", ["-9", p]);
}
function rcon(cmd, timeout = 3000) {
  return new Promise((ok) => {
    const s = createSocket("udp4");
    let parts = "";
    let closed = false;
    let t = null;
    const done = (v) => {
      if (closed) return;
      closed = true;
      clearTimeout(t);
      s.close();
      ok(v);
    };
    t = setTimeout(() => done(parts || null), timeout);
    const pre = Buffer.from([255, 255, 255, 255]);
    let challenge = null;
    s.on("message", (m) => {
      const text = m.subarray(4).toString("latin1");
      if (!challenge) {
        challenge = /challenge rcon (\d+)/.exec(text)?.[1];
        if (!challenge) return done(null);
        s.send(Buffer.concat([pre, Buffer.from(`rcon ${challenge} "${PW}" ${cmd}\n`, "latin1")]), PORT, "127.0.0.1");
      } else {
        if (/Bad rcon/i.test(text)) return done(null);
        parts += text.slice(1);
        // (long answers come in several packets: a moment for the rest)
        clearTimeout(t);
        t = setTimeout(() => done(parts), 250);
      }
    });
    s.on("error", () => done(null));
    s.send(Buffer.concat([pre, Buffer.from("challenge rcon\n")]), PORT, "127.0.0.1");
  });
}
const logText = (from = 0) => (existsSync(qlog) ? readFileSync(qlog).subarray(from).toString("latin1") : "");
const logSize = () => (existsSync(qlog) ? statSync(qlog).size : 0);
function startServer() {
  const srv = spawn("nice", ["-n", "15", WINE, "--bottle", "Steam", "--workdir", SIM, join(SIM, "hlds.exe"), "-console", "-game", "cstrike", "-insecure", "-nomaster", "-condebug", "-port", String(PORT), "+maxplayers", String(MAXPLAYERS), "+exec", "finalcheck.cfg", "+map", "de_dust2"], { cwd: OUT, stdio: "ignore" });
  // (cwd anywhere but the server's folder: there wine turns "-game cstrike" into a Windows path)
  srv.on("error", () => {});
}
async function serverUp() {
  for (let i = 0; i < 60; i++) {
    if (await rcon("echo up")) {
      for (let j = 0; j < 30 && !/Started map "/.test(logText()); j++) await sleep(2000);
      await sleep(8000);
      return true;
    }
    await sleep(3000);
  }
  return false;
}
async function restartServer() {
  stopServer();
  startServer();
  if (!(await serverUp())) throw new Error("the sim server didn't come back");
}
async function botCount() {
  const s = (await rcon("status", 4000)) ?? "";
  return { bots: (s.match(/ BOT /g) ?? []).length, humans: s.split("\n").filter((l) => /^#\s*\d+ /.test(l) && !l.includes(" BOT ")).length };
}
/** A map the sim doesn't have, from the game (APFS clones of the .bsp and what its .res lists). */
function borrowMap(map) {
  const game = join(BOTTLE, "Program Files (x86)/Steam/steamapps/common/Half-Life");
  const roots = ["cstrike", "cstrike_downloads"].map((g) => join(game, g)).filter(existsSync);
  const find = (rel) => roots.map((r) => join(r, rel)).find(existsSync);
  if (existsSync(join(GAMEDIR, "maps", `${map}.bsp`)) || !find(`maps/${map}.bsp`)) return;
  const files = [`maps/${map}.bsp`, `maps/${map}.nav`, `maps/${map}.lsv`, `maps/${map}.res`];
  const res = find(`maps/${map}.res`);
  if (res) for (const l of readFileSync(res, "latin1").split(/\r?\n/)) {
    const f = l.trim().replace(/\\/g, "/");
    if (f && !f.startsWith("//") && !f.includes("..")) files.push(f);
  }
  for (const f of files) {
    const src = find(f);
    if (!src || existsSync(join(GAMEDIR, f))) continue;
    mkdirSync(dirname(join(GAMEDIR, f)), { recursive: true });
    spawnSync("cp", ["-c", src, join(GAMEDIR, f)]);
  }
  log(`   borrowed ${map} from the game`);
}
/** A mode switch the way the app does it, checked: gm_mode, its map up, the plugin running, bots. */
async function useMode(mode, map, bots, plugin) {
  const from = logSize();
  await rcon(`gm_mode ${mode} ${map}`);
  let up = false;
  for (let w = 0; w < 45 && !up; w++) {
    await sleep(2000);
    up = logText(from).includes(`Started map "${map}"`);
  }
  if (!up) return { ok: false, why: `${map} didn't start after gm_mode ${mode}` };
  await sleep(4000);
  // the mode's saved bot count is applied as its map loads: ours after it, a few times (one cvar
  // per rcon: "a 0;b 5" in one goes to the server as a's value)
  await rcon(`gm_bots count ${bots}`);
  for (let i = 0; i < 4; i++) {
    await sleep(DRY ? 4000 : 8000);
    await rcon("bot_join_after_player 0");
    await rcon(`bot_quota ${bots}`);
  }
  const cur = readFileSync(join(AM, "data/gamemodes/current.txt"), "latin1").trim();
  const plugins = (await rcon("amxx plugins", 5000)) ?? "";
  // (amxx plugins cuts the file name to 11 letters)
  const short = plugin ? `${plugin}.amxx`.slice(0, 11) : null;
  const running = !short || plugins.split("\n").some((l) => l.includes(short) && /running|debug/.test(l));
  const n = await botCount();
  const why = [cur !== mode && `current.txt says ${cur}`, !running && `${plugin} isn't running`, Math.abs(n.bots - bots) > 1 && `${n.bots} bots (wanted ${bots})`].filter(Boolean);
  return { ok: why.length === 0, why: why.join("; "), bots: n.bots, from };
}
/** Samples every 2 s: stats (CPU, FPS), the hlds process's CPU, the longest time RCON went unanswered. */
async function sample(secs) {
  const rows = [];
  let lastAnswer = Date.now();
  let longest = 0;
  const end = Date.now() + secs * 1000;
  while (Date.now() < end) {
    await sleep(2000);
    const t = await rcon("stats", 4000);
    const now = Date.now();
    if (!t) {
      longest = Math.max(longest, now - lastAnswer);
      if (!hldsPids().length) return { rows, longest, crashed: true };
      continue;
    }
    longest = Math.max(longest, now - lastAnswer);
    lastAnswer = now;
    const lines = t.split("\n");
    const i = lines.findIndex((l) => l.trim().startsWith("CPU"));
    const nums = (lines[i + 1] ?? "").trim().split(/\s+/).map(Number);
    const pid = hldsPids()[0];
    const ps = pid ? Number(spawnSync("ps", ["-o", "%cpu=", "-p", pid], { encoding: "utf8" }).stdout.trim()) : NaN;
    if (nums.length >= 7) rows.push({ cpu: nums[0], fps: nums[5], proc: ps });
  }
  return { rows, longest, crashed: false };
}
const avg = (a) => (a.length ? a.reduce((x, y) => x + y, 0) / a.length : NaN);
const pct = (a, p) => {
  const s = [...a].sort((x, y) => x - y);
  return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * p))] : NaN;
};
function costOf(rows) {
  const ft = rows.map((r) => (r.fps > 0 ? 1000 / r.fps : NaN)).filter((x) => !Number.isNaN(x));
  return { n: rows.length, fps: +avg(rows.map((r) => r.fps)).toFixed(1), frameMs: +avg(ft).toFixed(2), frameP95: +pct(ft, 0.95).toFixed(2), cpu: +avg(rows.map((r) => r.cpu)).toFixed(1), proc: +avg(rows.map((r) => r.proc).filter((x) => !Number.isNaN(x))).toFixed(1) };
}

// ---- install ---------------------------------------------------------------------------------
function pinnedBrain() {
  const src = readFileSync(join(repo, "app/src-tauri/src/setup/foundation.rs"), "utf8");
  const m = /pub const REGAMEDLL: Pinned = Pinned \{\s*url: "([^"]+)",\s*sha256: "([0-9a-f]{64})"/.exec(src);
  if (!m) throw new Error("couldn't read the REGAMEDLL pin in setup/foundation.rs");
  const [, url, sha] = m;
  const name = basename(url);
  const cache = join(homedir(), "Library/Caches/loner-strike/downloads");
  const cached = existsSync(cache) ? readdirSync(cache).find((f) => f.endsWith(`-${name}`)) : undefined;
  const zip = cached ? join(cache, cached) : join(OUT, name);
  if (!existsSync(zip) && spawnSync("curl", ["-fsSL", "-o", zip, url]).status !== 0) throw new Error(`couldn't download the pinned brain ${url} (or pass --brain)`);
  const got = execFileSync("shasum", ["-a", "256", zip], { encoding: "utf8" }).split(" ")[0];
  if (got !== sha) throw new Error(`the brain zip ${zip} isn't the pinned one`);
  const dir = join(OUT, "brain");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  execFileSync("unzip", ["-q", "-o", zip, "bin/win32/cstrike/dlls/mp.dll", "-d", dir]);
  return { name, dll: join(dir, "bin/win32/cstrike/dlls/mp.dll") };
}
/** The newest green ls-next3 build of the bot brain (the next pin's candidate), from CI, cached
 *  where install-build.sh keeps them (~/loner-strike-sim/cache/r<run>). */
function newestBrain() {
  const repoName = "jamesontucker/ReGameDLL_CS";
  const r = spawnSync("gh", ["run", "list", "-R", repoName, "-b", "ls-next3", "-s", "success", "-L", "1", "--json", "databaseId,headSha"], { encoding: "utf8" });
  const run = JSON.parse(r.stdout || "[]")[0];
  if (!run) throw new Error("couldn't find a green ls-next3 build (gh run list); pass --brain <mp.dll>");
  const dir = join(homedir(), "loner-strike-sim/cache", `r${run.databaseId}`);
  const dll = join(dir, "bin/win32/cstrike/dlls/mp.dll");
  if (!existsSync(dll) && spawnSync("gh", ["run", "download", String(run.databaseId), "-R", repoName, "-n", "win32", "-D", dir]).status !== 0) throw new Error(`couldn't download CI run ${run.databaseId}'s win32 build`);
  return { name: `ls-next3 ${run.headSha.slice(0, 7)} (CI ${run.databaseId})`, dll };
}
function amxxpc(src, includes, out) {
  const inc = [...includes, join(AM, "scripting/include")].filter(existsSync);
  const r = run("compile", WINE, ["--bottle", "Steam", "--workdir", dirname(src), join(AM, "scripting/amxxpc.exe"), winPath(src), ...inc.map((i) => `-i${winPath(i)}`), `-o${winPath(out)}`], { timeout: 300_000 });
  return r.code === 0 && /Done\./.test(r.out);
}
function graphFiles(prefix) {
  const only = opt("--graphs", "") ? opt("--graphs", "").split(",") : null;
  return readdirSync(GRAPHS).filter((f) => f.startsWith(prefix) && f.endsWith(".graph.json") && (!only || only.some((o) => f.startsWith(o)))).sort();
}
/** The Studio's own code writes each graph's plugin and mode entry (finalcheck_tests.rs); amxxpc builds it. */
function installGraphs(files, debugIds) {
  const r = run("install", "cargo", ["test", "-j", "6", "--bin", "loner-strike", "finalcheck_install", "--", "--ignored", "--nocapture"], {
    cwd: join(TREE, "app/src-tauri"),
    env: { CARGO_TARGET_DIR: TARGET, LS_FC_GAME: GAMEDIR, LS_FC_GRAPHS: files.map((f) => join(GRAPHS, f)).join(","), LS_FC_DEBUG: debugIds },
    timeout: 3600_000,
  });
  const lines = r.out.split("\n").filter((l) => l.startsWith("FC ")).map((l) => JSON.parse(l.slice(3)));
  if (!lines.length) throw new Error("the installer printed nothing (install.log)");
  for (const l of lines.filter((x) => x.installed)) {
    l.compiled = amxxpc(join(AM, `scripting/${l.plugin}.sma`), [join(AM, "scripting/studio/include")], join(AM, `plugins/${l.plugin}.amxx`));
  }
  return lines;
}
const headTime = () => Number(execFileSync("git", ["-C", repo, "log", "-1", "--format=%ct", results.commit], { encoding: "utf8" }).trim()) * 1000;
function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}
/** HEAD as a worktree (the generator, the plugins' sources, the graphs), with the UI built: the
 *  app embeds app/ui/dist, and the Rust build wants it there. */
function makeTree() {
  dropTree();
  execFileSync("git", ["-C", repo, "worktree", "add", "--detach", TREE, results.commit], { stdio: "ignore" });
  for (const d of ["ls-voice", "ls-intent"]) mkdirSync(join(TREE, "app/src-tauri/resources/linux", d), { recursive: true });
  const nm = join(repo, "app/ui/node_modules");
  if (existsSync(nm)) symlinkSync(nm, join(TREE, "app/ui/node_modules"));
  const r = run("build", "npx", ["vite", "build"], { cwd: join(TREE, "app/ui") });
  if (r.code !== 0) throw new Error("vite build failed in the HEAD worktree (build.log)");
}
function dropTree() {
  if (!existsSync(TREE)) return;
  spawnSync("git", ["-C", repo, "worktree", "remove", "--force", TREE]);
  removeAll(TREE);
  spawnSync("git", ["-C", repo, "worktree", "prune"]);
}
/** The live nav pack's navs and map knowledge for every map the sim has in the pack's version. */
function refreshNavs() {
  const list = join(OUT, "navs-list.json");
  if (spawnSync("curl", ["-fsSL", "-o", list, NAV_LIST]).status !== 0) throw new Error(`couldn't read ${NAV_LIST}`);
  const maps = JSON.parse(readFileSync(list, "utf8")).maps;
  let fetched = 0;
  const bad = [];
  let matched = 0;
  for (const m of maps) {
    const bsp = join(GAMEDIR, "maps", `${m.map}.bsp`);
    if (!existsSync(bsp) || statSync(bsp).size !== m.bsp) continue;
    for (const [ext, want] of [["nav", m.nav_sha256], ["lsv", m.lsv_sha256]]) {
      const f = join(GAMEDIR, "maps", `${m.map}.${ext}`);
      if (existsSync(f) && sha256(f) === want) continue;
      spawnSync("curl", ["-fsSL", "-o", f, `${NAV_BASE}${m.map}.${ext}`]);
      fetched++;
      if (!existsSync(f) || sha256(f) !== want) bad.push(`${m.map}.${ext}`);
    }
    matched++;
  }
  return { list: sha256(list).slice(0, 12), matched, fetched, bad };
}
const version = (component, value, fresh, detail = "") => {
  results.versions.push({ component, value, fresh, detail });
  log(`   ${fresh ? "ok  " : "OLD "} ${component}: ${value}${detail ? ` (${detail})` : ""}`);
};

async function install() {
  if (flag("--reuse") && existsSync(join(GAMEDIR, "finalcheck.cfg"))) {
    log("   reusing the clone");
    results.notes.push("--reuse: the plugins are an earlier run's (refused below when older than HEAD's sources)");
  } else {
    const base = join(SIMS, SIM_BASE);
    if (!existsSync(join(base, "hlds.exe"))) throw new Error(`no sim base ${base}`);
    removeAll(SIM);
    execFileSync("cp", ["-c", "-R", base, SIM]);
    // this checkout's mode lists and plugins (every plugin it has the source of, as setup builds them)
    const tree = TREE;
    for (const mod of existsSync(join(tree, "mods")) ? readdirSync(join(tree, "mods")) : []) {
      for (const sub of ["data", "configs"]) copyOver(join(tree, "mods", mod, "addons/amxmodx", sub), join(AM, sub));
    }
    copyOver(join(tree, "plugins/configs/gamemodes"), join(AM, "configs/gamemodes"));
    const jobs = new Map();
    for (const f of readdirSync(join(tree, "plugins/scripting")).filter((x) => x.endsWith(".sma"))) jobs.set(f.slice(0, -4).toLowerCase(), { src: join(tree, "plugins/scripting", f), include: join(tree, "plugins/scripting/include"), ours: true });
    for (const f of readdirSync(join(tree, "catalog/parts")).filter((x) => x.endsWith(".json"))) {
      const part = JSON.parse(readFileSync(join(tree, "catalog/parts", f), "utf8"));
      if (!part.vendor) continue;
      const scripting = join(tree, part.vendor, "addons/amxmodx/scripting");
      for (const c of part.compile ?? []) if (c.src && c.out) jobs.set(String(c.out).toLowerCase(), { src: join(scripting, ...String(c.src).split("/")), include: c.include ? join(scripting, ...String(c.include).split("/")) : null });
    }
    const have = new Set(readdirSync(join(AM, "plugins")).filter((x) => x.endsWith(".amxx")).map((x) => x.slice(0, -5).toLowerCase()));
    let built = 0;
    const bad = [];
    for (const [name, job] of jobs) {
      // ours always (a Studio mode may list gm_components or gm_botnav); a mod's when the server has it
      if (!job.ours && !have.has(name)) continue;
      if (!existsSync(job.src)) continue;
      if (amxxpc(job.src, [job.include].filter(Boolean), join(AM, "plugins", `${name}.amxx`))) built++;
      else bad.push(name);
    }
    log(`   plugins compiled: ${built}${bad.length ? `, FAILED: ${bad.join(" ")}` : ""}`);
    results.notes.push(`plugins compiled ${built}${bad.length ? `; failed ${bad.join(" ")}` : ""}`);
    version("plugins (gm_*, gm_components, BR, Last Stand, Deathrun, mods from catalog parts)", `${built} compiled from ${results.commit}`, bad.length === 0, bad.length ? `failed: ${bad.join(" ")}` : "");
  }
  for (const m of ["de_dust2", "cs_italy", "br_valley"]) borrowMap(m);
  const navs = refreshNavs();
  version("navs and map knowledge", `live pack list ${navs.list}: ${navs.matched} maps match`, navs.bad.length === 0, `${navs.fetched} files fetched${navs.bad.length ? `; still wrong: ${navs.bad.join(" ")}` : ""}`);
  // every plugin the sim has whose source is in HEAD: built by this run (older ones refuse the run)
  const ours = new Set(readdirSync(join(TREE, "plugins/scripting")).filter((x) => x.endsWith(".sma")).map((x) => x.slice(0, -4).toLowerCase()));
  const stale = readdirSync(join(AM, "plugins")).filter((f) => f.endsWith(".amxx") && ours.has(f.slice(0, -5).toLowerCase()) && statSync(join(AM, "plugins", f)).mtimeMs < (flag("--reuse") ? headTime() : START));
  if (stale.length) version("our plugins in the sim", `${stale.length} not built from ${results.commit}`, false, stale.join(" "));
  const want = opt("--brain", "newest");
  const brain = want === "newest" ? newestBrain() : want === "pinned" ? pinnedBrain() : { name: want, dll: want };
  copyFileSync(brain.dll, join(GAMEDIR, "dlls/mp.dll"));
  results.brainDll = brain.dll;
  if (want === "newest") {
    // the newest green build is the branch's head, or the head is still building / red (said so)
    const head = spawnSync("gh", ["api", "repos/jamesontucker/ReGameDLL_CS/branches/ls-next3", "-q", ".commit.sha"], { encoding: "utf8" }).stdout.trim();
    const same = head && brain.name.includes(head.slice(0, 7));
    version("bot brain", brain.name, true, same ? "the branch head" : `the branch head ${head.slice(0, 7)} has no green build yet`);
  } else version("bot brain", brain.name, false, "not the newest green ls-next3 (--brain)");
  version("bot brain mp.dll", sha256(brain.dll).slice(0, 16), true);
  log(`   bot brain ${brain.name}`);
  results.notes.push(`brain ${brain.name}`);
  // bots only, no time or round limits, our rcon, logs to the console
  const limits = "mp_timelimit 0\nmp_fraglimit 0\nmp_maxrounds 0\nmp_winlimit 0\n";
  writeFileSync(join(AM, "data/gamemodes/friends.cfg"), `// finalcheck\nsv_lan 1\nbot_join_after_player 0\nbot_auto_vacate 0\nrcon_password "${PW}"\nlog on\nmp_logecho 1\nbr_wait_human 0\n${limits}`);
  writeFileSync(join(AM, "data/gamemodes/panel_rcon.txt"), `${PW}\n`);
  writeFileSync(join(GAMEDIR, "finalcheck.cfg"), `rcon_password "${PW}"\nsv_lan 1\nlog on\nmp_logecho 1\n${limits}`);
  for (const f of ["lastpreset.txt", "premode.txt", "crashmap.txt"]) rmSync(join(AM, "data/gamemodes", f), { force: true });
  // the graphs: cover with the debugger (node runs), the rest plain
  const files = [...graphFiles("cover_"), ...graphFiles("perf_"), ...graphFiles("stress_")];
  const lines = installGraphs(files, graphFiles("cover_").length ? "all" : "");
  results.install = lines.map((l) => ({ graph: basename(l.graph), id: l.id, version: l.version, installed: l.installed, compiled: l.compiled ?? false, errors: l.errors, warnings: (l.warnings ?? []).length, precache: l.precache, nodes: l.nodes }));
  version("Studio plugins (the generator)", `${results.install.filter((l) => l.compiled).length} made by ${results.commit}'s generator`, true);
  const broken = results.install.filter((l) => !l.graph.startsWith("stress_precache170") && !(l.installed && l.compiled));
  for (const l of results.install) log(`   ${l.graph}: ${l.installed ? (l.compiled ? "installed, compiled" : "installed, COMPILE FAILED") : `NOT installed: ${l.errors.join(" / ")}`}`);
  return broken.length === 0;
}

// ---- the steps -------------------------------------------------------------------------------
const FORBID = ["Host_Error", "Segmentation fault", "FATAL ERROR", "no free edicts", "ED_Alloc"];
const runtimeErrors = (text, plugin) => text.split("\n").filter((l) => /Run time error|\[AMXX\] Displaying debug trace/.test(l) || (plugin && l.includes(plugin) && /error/i.test(l) && !/0 errors/.test(l)));
const newKinds = () => JSON.parse(readFileSync(join(GRAPHS, "new_kinds.json"), "utf8")).kinds;

async function cover() {
  const kinds = new Set(newKinds());
  const ran = new Set();
  const present = new Set();
  for (const f of graphFiles("cover_")) {
    await roomToRun(f);
    const g = JSON.parse(readFileSync(join(GRAPHS, f), "utf8"));
    const id = g.mode.id;
    const plugin = `studio_${id}`;
    const events = join(AM, `data/studio/${id}.events`);
    rmSync(events, { force: true });
    const sw = await useMode(id, "de_dust2", 16, plugin);
    const t = Date.now();
    const s = await sample(SECS);
    await rcon("sv_restartround 1");
    const s2 = await sample(DRY ? 10 : 30);
    const text = logText(sw.from ?? 0);
    const evText = existsSync(events) ? readFileSync(events, "latin1") : "";
    const byId = new Map(g.nodes.map((n) => [n.id, n.type]));
    const nodeRan = new Set();
    const nodeErrors = new Set();
    for (const l of evText.split("\n")) {
      try {
        const e = JSON.parse(l);
        if (e.e === "node_enter" && e.node) nodeRan.add(e.node);
        if (e.e === "node_error" && e.node) nodeErrors.add(`${byId.get(e.node) ?? e.node}: ${e.message}`);
      } catch {}
    }
    for (const n of g.nodes) if (kinds.has(n.type)) present.add(n.type);
    for (const n of nodeRan) if (kinds.has(byId.get(n))) ran.add(byId.get(n));
    const errs = runtimeErrors(text, plugin);
    const bad = FORBID.filter((x) => text.includes(x));
    const row = { mode: id, switched: sw.ok, switchWhy: sw.why, secs: Math.round((Date.now() - t) / 1000), nodesRan: nodeRan.size, nodes: g.nodes.length, runtimeErrors: errs.slice(0, 10), nodeErrors: [...nodeErrors].slice(0, 20), forbidden: bad, crashed: s.crashed || s2.crashed, kills: (text.match(/" killed "/g) ?? []).length };
    row.ok = sw.ok && !row.crashed && !errs.length && !bad.length;
    results.cover.push(row);
    writeFileSync(join(OUT, `cover-${id}.log`), text);
    log(`   ${id}: ${row.ok ? "ok" : "PROBLEM"} - ${row.nodesRan}/${row.nodes} nodes ran, ${errs.length} run-time errors, ${nodeErrors.size} node errors${sw.ok ? "" : `; switch: ${sw.why}`}`);
    if (row.crashed) await restartServer();
  }
  const never = [...present].filter((k) => !ran.has(k)).sort();
  results.coverKinds = { added: kinds.size, inModes: present.size, ran: ran.size, neverRan: never };
  log(`   this round's node kinds: ${ran.size} of ${present.size} ran in a game (${kinds.size} added); never ran: ${never.join(" ") || "none"}`);
  return results.cover.every((r) => r.ok);
}

async function perf() {
  // the cost runs use the plain build of every mode (the cover modes were built with the debugger)
  const files = [...graphFiles("perf_"), ...graphFiles("cover_")];
  const lines = installGraphs(files, "");
  const modes = [["classic", null], ...lines.filter((l) => l.installed && l.compiled).map((l) => [l.id, l.plugin])];
  for (const bots of BOTS) {
    let base = null;
    for (const [mode, plugin] of DRY ? modes.slice(0, 2) : modes) {
      await roomToRun(`perf ${mode} ${bots}`);
      const sw = await useMode(mode, "de_dust2", bots, plugin);
      // (the first seconds after a map change are loading, not playing)
      await sleep(DRY ? 3000 : 15_000);
      const s = await sample(DRY ? 20 : Math.min(SECS, 120));
      const c = costOf(s.rows);
      if (mode === "classic") base = c;
      const row = { mode, bots, botsSeen: sw.bots, switched: sw.ok, switchWhy: sw.why, ...c, crashed: s.crashed, longestSilenceMs: s.longest, vsClassic: base && mode !== "classic" ? { frameMs: +(c.frameMs - base.frameMs).toFixed(2), cpu: +(c.cpu - base.cpu).toFixed(1), proc: +(c.proc - base.proc).toFixed(1) } : null };
      results.perf.push(row);
      log(`   ${mode} @${bots}: ${c.fps} fps (frame ${c.frameMs} ms, p95 ${c.frameP95}), stats CPU ${c.cpu}%, process ${c.proc}%${row.vsClassic ? ` | vs Classic: frame ${row.vsClassic.frameMs >= 0 ? "+" : ""}${row.vsClassic.frameMs} ms, process ${row.vsClassic.proc >= 0 ? "+" : ""}${row.vsClassic.proc}%` : ""}${sw.ok ? "" : ` (switch: ${sw.why})`}`);
      if (s.crashed) await restartServer();
    }
  }
  return results.perf.every((r) => r.switched && !r.crashed);
}

const CAP_LINES = /skipped|not loaded: the mode has loaded|sent over the budget|waited for room|at once allows|limit\)/;
async function stress() {
  const refused = results.install.find((l) => l.graph.startsWith("stress_precache170"));
  if (refused) {
    const ok = !refused.installed && refused.errors.some((e) => /the most is 160/.test(e));
    results.stress.push({ mode: refused.id, ok, what: "refused at install", detail: refused.errors.join(" / ") });
    log(`   ${refused.id}: ${ok ? "refused as it should be" : "NOT refused"}: ${refused.errors.join(" / ")}`);
  }
  let humans = 0;
  if (flag("--human")) {
    log(`   waiting for a person to join 127.0.0.1:${PORT} (connect from the game's console)`);
    for (let i = 0; i < 120 && humans < 1; i++, await sleep(5000)) humans = (await botCount()).humans;
    if (!humans) results.notes.push("--human: nobody joined in 10 minutes; overflow not checked");
  }
  for (const f of graphFiles("stress_").filter((x) => !x.startsWith("stress_precache170"))) {
    const g = JSON.parse(readFileSync(join(GRAPHS, f), "utf8"));
    const id = g.mode.id;
    const maps = id === "fc_st_pre160" ? ["de_dust2", "cs_italy"] : ["de_dust2"];
    for (const map of maps) {
      await roomToRun(`stress ${id}`);
      if (!existsSync(join(GAMEDIR, "maps", `${map}.bsp`))) {
        results.stress.push({ mode: id, map, ok: null, what: "skipped", detail: `${map} isn't in the sim` });
        continue;
      }
      const sw = await useMode(id, map, 16, `studio_${id}`);
      const before = await botCount();
      const s = await sample(DRY ? 30 : 120);
      const alive = !!(await rcon("echo alive", 8000));
      const after = alive ? await botCount() : { bots: 0, humans: 0 };
      const text = logText(sw.from ?? 0);
      const caps = text.split("\n").filter((l) => l.includes(`[${id}]`) && CAP_LINES.test(l));
      const capKinds = [...new Set(caps.map((l) => l.replace(/.*\] \[[^\]]+\] /, "").replace(/\d+/g, "N").trim()))];
      const bad = FORBID.filter((x) => text.includes(x));
      const overflow = text.split("\n").filter((l) => /overflow/i.test(l));
      const dropped = text.split("\n").filter((l) => /dropped|kicked|timed out/i.test(l) && !/skipped/.test(l));
      const problems = [
        !sw.ok && `switch: ${sw.why}`,
        s.crashed && "the server crashed",
        !alive && "the server stopped answering",
        s.longest > 10_000 && `RCON unanswered for ${Math.round(s.longest / 1000)} s (a hang)`,
        ...bad.map((b) => `"${b}" in the log`),
        overflow.length && `${overflow.length} overflow lines`,
        humans && after.humans < humans && "the person was dropped",
        before.bots - after.bots > 1 && `bots ${before.bots} -> ${after.bots}`,
      ].filter(Boolean);
      const row = { mode: id, map, ok: problems.length === 0, problems, longestSilenceMs: s.longest, cost: costOf(s.rows), capLines: caps.length, capKinds: capKinds.slice(0, 12), dropped: dropped.slice(0, 5) };
      results.stress.push(row);
      writeFileSync(join(OUT, `stress-${id}-${map}.log`), text);
      log(`   ${id} on ${map}: ${row.ok ? "ok" : "PROBLEM: " + problems.join("; ")} - ${caps.length} cap lines (${capKinds.slice(0, 4).join(" | ")}), longest silence ${(s.longest / 1000).toFixed(1)} s, ${row.cost.fps} fps`);
      if (!alive || s.crashed || s.longest > 10_000) await restartServer();
    }
  }
  return results.stress.every((r) => r.ok !== false);
}

/** Battle Royale squads, 16 bots, up to 8 minutes, on this sim (the re-check's br-squads16 step):
 *  a game starts, first contact, then someone knocked down, out or killed. */
async function br() {
  await roomToRun("br");
  const sw = await useMode("br_squads", "br_valley", 16, null);
  const t = Date.now();
  let text = "";
  const done = () => text.includes("game starts") && text.includes("first contact at") && ["knocked down at", " out at ", '" killed "'].some((x) => text.includes(x));
  while (Date.now() - t < (DRY ? 60 : 480) * 1000) {
    await sleep(10_000);
    text = logText(sw.from ?? 0);
    if (done()) break;
  }
  const alive = !!(await rcon("echo alive", 8000));
  writeFileSync(join(OUT, "br-squads16.log"), text);
  const ok = sw.ok && alive && done();
  results.br = { ok, bots: sw.bots, switched: sw.ok, switchWhy: sw.why, secs: Math.round((Date.now() - t) / 1000), line: `${ok ? "PASS" : "FAIL"}: ${sw.bots} bots, ${["game starts", "first contact at", "knocked down at", " out at "].filter((x) => text.includes(x)).map((x) => x.trim()).join(", ")}, ${(text.match(/" killed "/g) ?? []).length} kills in ${Math.round((Date.now() - t) / 1000)} s${sw.ok ? "" : `; switch: ${sw.why}`}${alive ? "" : "; the server stopped answering"}` };
  log(`   br-squads16: ${results.br.line}`);
  return ok;
}

function summary() {
  const L = [`# Final check ${DRY ? "(dry run) " : ""}${new Date().toISOString().slice(0, 16)}`, "", `Commit ${results.commit}, sim ${SIM_NAME} from ${SIM_BASE}. ${results.notes.join("; ")}; ls_mode_ver ${results.brainModeVer ?? "not read"}`, ""];
  L.push("## Versions", "", "| Component | Version | Fresh | Detail |", "|---|---|---|---|");
  for (const v of results.versions) L.push(`| ${v.component} | ${v.value} | ${v.fresh ? "yes" : "NO"} | ${v.detail} |`);
  L.push("");
  L.push("## Install", "", "| Graph | v | Nodes | Installed | Compiled | Precache | Errors |", "|---|---|---|---|---|---|---|");
  for (const r of results.install) L.push(`| ${r.graph} | ${r.version} | ${r.nodes ?? ""} | ${r.installed ? "yes" : "no"} | ${r.compiled ? "yes" : "no"} | ${r.precache ?? ""} | ${(r.errors ?? []).join("; ")} |`);
  if (results.cover.length) {
    L.push("", "## Every new node in a game", "", "| Mode | OK | Nodes ran | Run-time errors | Node errors | Kills |", "|---|---|---|---|---|---|");
    for (const r of results.cover) L.push(`| ${r.mode} | ${r.ok ? "yes" : "NO"} | ${r.nodesRan}/${r.nodes} | ${r.runtimeErrors.length} | ${r.nodeErrors.length} | ${r.kills} |`);
    if (results.coverKinds) L.push("", `This round's kinds: ${results.coverKinds.ran} of ${results.coverKinds.inModes} ran. Never ran: ${results.coverKinds.neverRan.join(", ") || "none"}.`);
  }
  if (results.perf.length) {
    L.push("", "## Cost against Classic", "", "| Mode | Bots | FPS | Frame ms (p95) | stats CPU | Process CPU | Frame vs Classic | Process vs Classic |", "|---|---|---|---|---|---|---|---|");
    for (const r of results.perf) L.push(`| ${r.mode} | ${r.botsSeen}/${r.bots} | ${r.fps} | ${r.frameMs} (${r.frameP95}) | ${r.cpu}% | ${r.proc}% | ${r.vsClassic ? r.vsClassic.frameMs : ""} | ${r.vsClassic ? r.vsClassic.proc + "%" : ""} |`);
  }
  if (results.stress.length) {
    L.push("", "## Janky graphs (every limit at max)", "", "| Mode | Map | OK | Problems | Cap lines | Longest silence |", "|---|---|---|---|---|---|");
    for (const r of results.stress) L.push(`| ${r.mode} | ${r.map ?? ""} | ${r.ok === null ? "skipped" : r.ok ? "yes" : "NO"} | ${(r.problems ?? [r.detail]).join("; ")} | ${r.capLines ?? ""} ${(r.capKinds ?? []).slice(0, 3).join(" / ")} | ${r.longestSilenceMs != null ? (r.longestSilenceMs / 1000).toFixed(1) + " s" : ""} |`);
  }
  if (results.br) L.push("", "## Battle Royale squads, 16 bots", "", results.br.line || `exit ${results.br.code}`);
  writeFileSync(join(OUT, "summary.md"), L.join("\n") + "\n");
  writeFileSync(join(OUT, "results.json"), JSON.stringify(results, null, 1));
}

async function main() {
  results.commit = execFileSync("git", ["-C", repo, "rev-parse", "--short", "HEAD"], { encoding: "utf8" }).trim();
  log(`finalcheck ${results.commit} -> ${OUT}${DRY ? " (dry run)" : ""}`);
  const verdict = {};
  const step = async (name, f) => {
    if (!STEPS.includes(name)) return;
    log(`== ${name}`);
    try {
      verdict[name] = (await f()) ? "PASS" : "FAIL";
    } catch (e) {
      verdict[name] = `FAIL (${e.message})`;
    }
    log(`   ${verdict[name]}`);
  };
  await roomToRun("install");
  makeTree();
  version("source", `tools ${results.commit}`, true, "a worktree of HEAD: the working copy's edits aren't in it");
  if (STEPS.includes("install") || !flag("--reuse")) await step("install", install);
  const old = results.versions.filter((v) => !v.fresh);
  if (old.length && !flag("--allow-old")) throw new Error(`older than HEAD, not running: ${old.map((v) => v.component).join(", ")} (--allow-old for a harness test)`);
  if (["cover", "perf", "stress", "br"].some((s) => STEPS.includes(s))) {
    rmSync(qlog, { force: true });
    startServer();
    if (!(await serverUp())) throw new Error("the sim server didn't answer RCON in 3 minutes");
    // the brain the server really runs: its ls_mode_ver (an older one prints its usage list for
    // ls_mode commands the plugins send)
    const ver = Number(/"ls_mode_ver" is "(\d+)"/.exec((await rcon("ls_mode_ver")) ?? "")?.[1] ?? 0);
    results.brainModeVer = ver;
    version("brain's ls_mode_ver in the sim", String(ver || "missing"), ver >= Number(opt("--min-mode-ver", "4")));
    if (ver < Number(opt("--min-mode-ver", "4"))) throw new Error(`the sim's brain says ls_mode_ver ${ver || "nothing"} (wanted ${opt("--min-mode-ver", "4")}+): not the newest brain`);
    await step("cover", cover);
    await step("perf", perf);
    await step("stress", stress);
    await step("br", br);
  }
  results.verdict = verdict;
  summary();
  log(`\n${Object.entries(verdict).map(([k, v]) => `${k}: ${v}`).join("\n")}\nsummary: ${join(OUT, "summary.md")}`);
}
const cleanUp = () => {
  stopServer();
  if (!flag("--keep")) dropTree();
  if (!flag("--keep")) removeAll(SIM);
};
process.on("SIGINT", () => (cleanUp(), process.exit(130)));
process.on("SIGTERM", () => (cleanUp(), process.exit(143)));
process.on("uncaughtException", (e) => (log(`FAILED: ${e.stack ?? e}`), cleanUp(), process.exit(1)));
main()
  .catch((e) => log(`FAILED: ${e.message}`))
  .finally(cleanUp);
