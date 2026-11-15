#!/usr/bin/env node
// The combined re-check (docs/notes/Recheck.md): one build from one commit, every automatic check,
// one summary table. Mac (CrossOver) only for the Playwright game clone and the sims.
//
//   node scripts/recheck.mjs                    # HEAD, everything
//   node scripts/recheck.mjs --commit 1aeec7f1  # another commit
//   node scripts/recheck.mjs --only rust,ui     # some steps (rust, node, ui, list, modfiles, english, gitleaks, sims)
//   node scripts/recheck.mjs --dry-run          # say what would run, run nothing
//
// What it does, in order (each step's output is kept in <out>/<step>.log):
//   1. a detached worktree of the commit (node_modules linked from this checkout), so nothing
//      in your working tree is used or changed
//   2. rust: the whole Rust suite (cargo test --bin loner-strike), one build in --target
//   3. node: the UI type check and every app/ui/scripts/test-*.mjs
//   4. ui: every Playwright spec in Chromium (large and small windows) and WebKit, against the
//      tools dev server over a FULL APFS clone of the game (Sim/RECHECK-HL; never the real game
//      folder, never the small test game) with the app's data in a scratch folder (LS_APP_DIR)
//   5. list: the list service's tests (services/serverlist, unittest)
//   6. modfiles: scripts/mod-files-check.py   7. english: scripts/us-english-check.py
//   8. gitleaks over the commit (GITLEAKS=path, default the audit's copy in the scratchpad)
//   9. sims: ONE CrossOver server at a time (-insecure), a clone of --sim-base with this commit's
//      plugins compiled fresh (amxxpc), its mods' data and configs; each sim step switches the
//      mode like the app (gm_mode), waits, and reads the server log (kills, rounds, errors)
// It stops (marking the rest SKIPPED) whenever free disk is under 8 GB, and cleans up its own
// clones, servers (by their own hlds.exe PID) and worktree at the end, keeping the logs.
//
// RECHECK_BRAIN_ZIP=path: the pinned bot brain's zip when it isn't released yet (checked against
// the pin's SHA-256 either way); the sims run the brain the commit pins, unless RECHECK_BRAIN_DLL
// names another mp.dll (the final check: the next pin's candidate). The log says its ls_mode_ver.
// GITLEAKS=path picks gitleaks (else the one on the PATH). Run it outside a sandbox: a sandboxed
// shell can't stop CrossOver processes (the sim's hlds.exe), and says nothing when it fails.
// RECHECK_WORKERS=n: Playwright workers (default 1, at most 3; low priority either way).
// Options: --out DIR (default $TMPDIR/ls-recheck-<stamp>), --target DIR (cargo target, default
// ~/Library/Caches/loner-strike-recheck-target), --game DIR (the Half-Life folder to clone,
// default CrossOver Steam's), --sim-base NAME (default LSPLAY, under Loner-Strike/Sim), --keep
// (keep the clones), --vite-port 5390, --dev-port 8390, --sim-port 27490.
// Sim steps whose map isn't installed are SKIPPED with why.

import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createSocket } from "node:dgram";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync, appendFileSync, copyFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (k, d) => (args.includes(k) ? args[args.indexOf(k) + 1] : d);
const flag = (k) => args.includes(k);
const stamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 13);
const OUT = resolve(opt("--out", join(tmpdir(), `ls-recheck-${stamp}`)));
const TARGET = resolve(opt("--target", join(homedir(), "Library/Caches/loner-strike-recheck-target")));
const BOTTLE = join(homedir(), "Library/Application Support/CrossOver/Bottles/Steam/drive_c");
const GAME = resolve(opt("--game", join(BOTTLE, "Program Files (x86)/Steam/steamapps/common/Half-Life")));
const SIMS = join(BOTTLE, "Loner-Strike/Sim");
const SIM_BASE = opt("--sim-base", "LSPLAY");
const WINE = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine";
const VITE_PORT = opt("--vite-port", "5390");
const DEV_PORT = opt("--dev-port", "8390");
const SIM_PORT = Number(opt("--sim-port", "27490"));
const DRY = flag("--dry-run");
const ALL = ["rust", "node", "ui", "list", "modfiles", "english", "gitleaks", "sims"];
const only = opt("--only", "") ? opt("--only", "").split(",") : ALL;
const MIN_FREE_GB = 8;

mkdirSync(OUT, { recursive: true });
const commit = execFileSync("git", ["-C", repo, "rev-parse", opt("--commit", "HEAD")], { encoding: "utf8" }).trim();
const TREE = join(OUT, "tree");
const APP_DIR = join(OUT, "app-data");
const GAME_CLONE = join(SIMS, "RECHECK-HL");
// (RECHECK_SIM_NAME: a second process running the sims beside the first's other steps)
const SIM_NAME = (process.env.RECHECK_SIM_NAME || "RECHECK").replace(/[^A-Za-z0-9_-]/g, "");
const SIM = join(SIMS, SIM_NAME);
const rows = []; // {step, result, secs, detail}
const simRows = [];
let stopped = "";

const log = (m) => {
  console.log(m);
  appendFileSync(join(OUT, "recheck.log"), m + "\n");
};
function freeGb() {
  const out = execFileSync("df", ["-k", "/"], { encoding: "utf8" }).trim().split("\n").pop().split(/\s+/);
  return Math.floor(Number(out[3]) / 1024 / 1024);
}
function diskOk(where) {
  const gb = freeGb();
  if (gb < MIN_FREE_GB) {
    stopped = `free disk ${gb} GB (under ${MIN_FREE_GB} GB) before ${where}`;
    log(`STOP: ${stopped}`);
    return false;
  }
  return true;
}
/** Runs a command, its output to <out>/<name>.log; {code, out}. */
function run(name, cmd, argv, opts = {}) {
  const file = join(OUT, `${name}.log`);
  appendFileSync(file, `$ ${cmd} ${argv.join(" ")}\n`);
  if (DRY) return { code: 0, out: "" };
  const r = spawnSync(cmd, argv, { cwd: opts.cwd ?? TREE, env: { ...process.env, ...(opts.env ?? {}) }, encoding: "utf8", maxBuffer: 1 << 30, timeout: opts.timeout ?? 4 * 3600_000 });
  const out = (r.stdout ?? "") + (r.stderr ?? "");
  appendFileSync(file, out);
  return { code: r.status ?? 1, out };
}
async function step(name, f) {
  if (!only.includes(name) && name !== "build") return;
  if (stopped) return rows.push({ step: name, result: "SKIPPED", secs: 0, detail: stopped });
  if (!diskOk(name)) return rows.push({ step: name, result: "SKIPPED", secs: 0, detail: stopped });
  const t = Date.now();
  log(`== ${name}`);
  try {
    const { ok, detail } = await f();
    rows.push({ step: name, result: DRY ? "DRY RUN" : ok ? "PASS" : "FAIL", secs: Math.round((Date.now() - t) / 1000), detail });
  } catch (e) {
    rows.push({ step: name, result: "FAIL", secs: Math.round((Date.now() - t) / 1000), detail: `error: ${e.message}` });
  }
  log(`   ${rows.at(-1).result} ${rows.at(-1).detail}`);
}

// ---- the commit -------------------------------------------------------------------------------
/** rm -rf that also takes read-only folders (the game has some: valve_hd/gfx/env). */
function removeAll(p) {
  if (!existsSync(p)) return;
  spawnSync("chmod", ["-R", "u+w", p]);
  try {
    rmSync(p, { recursive: true, force: true });
  } catch (e) {
    log(`couldn't remove ${p}: ${e.message}`);
  }
}
function makeTree() {
  if (DRY) return;
  rmSync(TREE, { recursive: true, force: true });
  execFileSync("git", ["-C", repo, "worktree", "add", "--detach", TREE, commit], { stdio: "ignore" });
  // (the Linux voice and intent agents: CI downloads them here; git keeps no empty folder, and
  // the app's build script wants the resource folders to exist)
  for (const d of ["ls-voice", "ls-intent"]) mkdirSync(join(TREE, "app/src-tauri/resources/linux", d), { recursive: true });
  const nm = join(repo, "app/ui/node_modules");
  if (existsSync(nm)) symlinkSync(nm, join(TREE, "app/ui/node_modules"));
}
function dropTree() {
  if (DRY || flag("--keep")) return;
  spawnSync("git", ["-C", repo, "worktree", "remove", "--force", TREE]);
}

// ---- the UI build (the app embeds app/ui/dist) ---------------------------------------------------
function buildUi() {
  const r = run("build", "npx", ["vite", "build"], { cwd: join(TREE, "app/ui") });
  return { ok: r.code === 0, detail: r.code === 0 ? "app/ui/dist built" : "vite build failed (build.log)" };
}

// ---- 2. Rust ------------------------------------------------------------------------------------
const rustEnv = { CARGO_TARGET_DIR: TARGET };
function rust() {
  const r = run("rust", "nice", ["-n", "10", "cargo", "test", "-j", "6", "--bin", "loner-strike"], { cwd: join(TREE, "app/src-tauri"), env: rustEnv });
  const res = [...r.out.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/g)].map((m) => m.slice(1).map(Number));
  const failed = [...r.out.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map((m) => m[1]);
  const [p, f, i] = res.reduce((a, b) => a.map((x, k) => x + b[k]), [0, 0, 0]);
  const compileError = /could not compile|failed to run custom build command/.test(r.out) || !res.length;
  return { ok: r.code === 0, detail: compileError ? "did not compile (see rust.log)" : `${p} passed, ${f} failed, ${i} ignored${failed.length ? `: ${failed.slice(0, 12).join(", ")}${failed.length > 12 ? "…" : ""}` : ""}` };
}

// ---- 3. node checks ----------------------------------------------------------------------------
function node() {
  const ui = join(TREE, "app/ui");
  const bad = [];
  if (run("node", "npx", ["tsc", "--noEmit", "-p", "."], { cwd: ui }).code !== 0) bad.push("tsc");
  const tests = DRY ? [] : readdirSync(join(ui, "scripts")).filter((f) => /^test-.*\.mjs$/.test(f));
  for (const t of tests) if (run("node", "node", [join("scripts", t)], { cwd: ui }).code !== 0) bad.push(t);
  return { ok: !bad.length, detail: bad.length ? `failed: ${bad.join(", ")}` : `tsc and ${tests.length} test scripts pass` };
}

// ---- 4. Playwright on a full game clone --------------------------------------------------------
function cloneGame() {
  if (DRY) return;
  if (!existsSync(join(GAME, "cstrike"))) throw new Error(`no game at ${GAME} (--game)`);
  removeAll(GAME_CLONE);
  // an APFS clone: the whole game, sharing its blocks until something writes
  execFileSync("cp", ["-c", "-R", GAME, GAME_CLONE]);
}
/** What some specs expect of their game, written into the clone (never the real game):
 *  creature-picker.spec.ts wants your own type my_runner (creature-send.spec.ts qa_crawler), jo.ghoul installed by a package
 *  (mode-77) and Last Stand's waves using my_runner. */
function specFixtures() {
  if (DRY) return;
  const gm = join(GAME_CLONE, "cstrike/addons/amxmodx/configs/gamemodes");
  mkdirSync(gm, { recursive: true });
  const ini = join(gm, "creatures.ini");
  const had = existsSync(ini) ? readFileSync(ini, "latin1") : "; creatures.ini\n";
  if (!/^\[my_runner\]/m.test(had)) {
    // yours first, then the installed block at the end (as the app keeps them)
    const block = "; >>> from mode-77\n; types: jo.ghoul\n[jo.ghoul]\nbase = zombie\nname = Ghoul\nhp = 333\n; <<< end mode-77\n";
    const at = had.indexOf("; >>> from ");
    // (creature-send.spec.ts wants qa_crawler, a type of the player's own, too)
    const mine = "[my_runner]\nbase = runner\nname = My runner\nhp = 70\n[qa_crawler]\nbase = zombie\nname = QA crawler\nhp = 50\n";
    const text = at < 0 ? `${had.trimEnd()}\n${mine}${block}` : `${had.slice(0, at)}${mine}${had.slice(at)}${block}`;
    writeFileSync(ini, text, "latin1");
  }
  const waves = join(gm, "waves.ini");
  const w = existsSync(waves) ? readFileSync(waves, "latin1") : "; waves.ini\n";
  if (!/my_runner/.test(w)) {
    const text = /^\[laststand\]/m.test(w) ? w.replace(/^\[laststand\]\s*\n/m, (m) => `${m}wave 1+ = my_runner 2\n`) : `${w.trimEnd()}\n[laststand]\nwave 1+ = my_runner 2 zombie 3\n`;
    writeFileSync(waves, text, "latin1");
  }
}
// Specs that need their own game state, run after the rest with it written into the clone (it
// would change what other specs see: creatures-qa4 wants Last Stand's own lineup, no waves.ini)
const FIXTURE_SPECS = ["creature-picker.spec.ts", "creature-send.spec.ts"];
function ui() {
  cloneGame();
  rmSync(APP_DIR, { recursive: true, force: true });
  mkdirSync(APP_DIR, { recursive: true });
  const env = { ...rustEnv, LS_APP_DIR: APP_DIR, LS_TEST_GAME: GAME_CLONE, LS_DEV_PORT: DEV_PORT, LS_VITE_PORT: VITE_PORT, LS_WEBKIT_ALL: "1", CI: "1" };
  // a few at a time, at low priority: other agents' game servers share this machine, and a starved
  // one can freeze CrossOver's wineserver for everyone (RECHECK_WORKERS, default 1, at most 3)
  const workers = Math.min(3, Math.max(1, Number(process.env.RECHECK_WORKERS) || 1));
  const specs = DRY ? [] : readdirSync(join(TREE, "app/ui/tests")).filter((f) => f.endsWith(".spec.ts"));
  const pass = (name, files, json) =>
    run(name, "nice", ["-n", "10", "npx", "playwright", "test", ...files.map((f) => `tests/${f}`), "--project", "large", "--project", "small", "--project", "webkit", `--workers=${workers}`, "--reporter=list,json"], { cwd: join(TREE, "app/ui"), env: { ...env, PLAYWRIGHT_JSON_OUTPUT_NAME: json } });
  const jsons = [join(OUT, "playwright.json"), join(OUT, "playwright-fixtures.json")];
  const r1 = pass("ui", specs.filter((f) => !FIXTURE_SPECS.includes(f)), jsons[0]);
  specFixtures();
  const r2 = pass("ui", specs.filter((f) => FIXTURE_SPECS.includes(f)), jsons[1]);
  if (DRY) return { ok: true, detail: "" };
  const r = { code: r1.code || r2.code, out: r1.out + r2.out };
  let detail = `exit ${r.code}: ${r.out.trim().split("\n").filter((l) => /error|Error|failed/.test(l)).slice(-1)[0]?.slice(0, 160) ?? "see ui.log"}`;
  const by = {};
  const failed = [];
  for (const json of jsons) {
    try {
      const rep = JSON.parse(readFileSync(json, "utf8"));
      const walk = (s) => {
        for (const sp of s.specs ?? []) for (const t of sp.tests ?? []) {
          const st = t.results?.at(-1)?.status ?? "skipped";
          by[t.projectName] ??= { passed: 0, failed: 0, skipped: 0, flaky: 0 };
          const k = t.status === "flaky" ? "flaky" : st === "passed" ? "passed" : st === "skipped" ? "skipped" : "failed";
          by[t.projectName][k]++;
          if (k === "failed") failed.push(`${t.projectName}: ${basename(sp.file)} › ${sp.title}`);
        }
        for (const c of s.suites ?? []) walk(c);
      };
      for (const s of rep.suites ?? []) walk(s);
    } catch {
      /* the list reporter's output is in ui.log */
    }
  }
  writeFileSync(join(OUT, "ui-failed.txt"), failed.join("\n") + "\n");
  if (Object.keys(by).length) detail = Object.entries(by).map(([p, c]) => `${p} ${c.passed}✓ ${c.failed}✗ ${c.flaky} flaky ${c.skipped} skipped`).join("; ") + (failed.length ? ` (failed list: ui-failed.txt)` : "");
  return { ok: r.code === 0, detail };
}

// ---- 5-8. the rest ------------------------------------------------------------------------------
const list = () => {
  const r = run("list", "python3", ["-m", "unittest", "discover", "-s", "services/serverlist", "-p", "test_*.py"]);
  const m = /Ran (\d+) tests?/.exec(r.out);
  return { ok: r.code === 0, detail: `${m ? m[1] : "?"} tests${r.code === 0 ? ", OK" : `, ${/(FAILED \(.*\))/.exec(r.out)?.[1] ?? "failed"}`}` };
};
const modfiles = () => {
  const r = run("modfiles", "python3", ["scripts/mod-files-check.py"]);
  return { ok: r.code === 0, detail: r.out.trim().split("\n").slice(-1)[0] ?? "" };
};
const english = () => {
  const r = run("english", "python3", ["scripts/us-english-check.py"]);
  return { ok: r.code === 0, detail: r.code === 0 ? "no UK spellings" : `${r.out.trim().split("\n").length} lines (english.log)` };
};
const gitleaks = () => {
  // GITLEAKS, else one on the PATH (never installed by this script)
  const gl = process.env.GITLEAKS || (spawnSync("which", ["gitleaks"], { encoding: "utf8" }).stdout ?? "").trim();
  if (!gl) return { ok: false, detail: "gitleaks not found: set GITLEAKS to its path" };
  if (!DRY && !existsSync(gl)) return { ok: false, detail: `no gitleaks at ${gl}` };
  const r = run("gitleaks", gl, ["git", `--log-opts=-1 ${commit}`, "--no-banner", "--redact", "-v", repo], { cwd: repo });
  const n = /leaks found: (\d+)/.exec(r.out)?.[1];
  return { ok: r.code === 0, detail: r.code === 0 ? "no leaks found (this commit)" : `${n ?? "?"} leaks (gitleaks.log)` };
};

// ---- 9. sims ------------------------------------------------------------------------------------
const PW = `rc${Math.random().toString(36).slice(2, 12)}`;
// the sim steps: what each one shows (Recheck.md lists the commits each covers)
const SIM_STEPS = [
  { id: "classic", mode: "classic", map: "de_dust2", secs: 120, kills: 1, why: "the classic modes still play (QA4-CORE)" },
  { id: "csdm", mode: "csdm", map: "de_dust2", secs: 90, kills: 5, why: "respawn modes" },
  { id: "gungame", mode: "gungame", map: "de_dust2", secs: 90, kills: 3, forbid: ["Invalid multi-lingual line"], why: "Gun Game's language file (caca2004)" },
  { id: "basebuilder", mode: "basebuilder", map: "bb_aztec", secs: 120, forbid: ["SQL-LOG", "syntax error"], why: "Base Builder saves on SQLite (ee9818d3)" },
  { id: "bb-precache", mode: "basebuilder", map: "bb_dust_iplay", secs: 90, alive: true, why: "a map over the model limit can't hang the server (56f3ca19)" },
  { id: "laststand", mode: "laststand", map: "ls_outpost", secs: 180, why: "Last Stand waves, creatures, turrets (QA4-C, W1)" },
  { id: "br", mode: "br", map: "br_valley", secs: 180, expect: ["game starts"], why: "Battle Royale starts with bots (5 bots is too few for a fight in 3 minutes)" },
  // W2's way: 16 bots in squads, up to 8 minutes, until someone is knocked down or out
  { id: "br-squads16", mode: "br_squads", map: "br_valley", secs: 480, bots: 16, expect: ["game starts", "first contact at"], anyOf: ["knocked down at", " out at ", '" killed "'], until: true, why: "Battle Royale squads fight: first contact, knocks and kills (QA4-D, W2)" },
  { id: "deathrun", mode: "deathrun", map: "deathrun_temple", secs: 150, rounds: 1, why: "rounds end at time-up (584eb81d)" },
  { id: "zombie", mode: "zombie", map: "zm_deko2", secs: 120, rounds: 1, why: "Zombie Plague" },
  { id: "modevote", mode: "classic", map: "de_dust2", secs: 60, vote: true, expect: ["Map vote for"], why: "mode vote lists (a7ef360f)" },
];
const FORBID = ["Run time error", "Host_Error", "Segmentation fault", "failed to load"];
const winPath = (p) => (p.startsWith(BOTTLE) ? "C:" + p.slice(BOTTLE.length) : "Z:" + p).replace(/\//g, "\\");
function hldsPids() {
  const r = spawnSync("pgrep", ["-f", `^C:\\\\Loner-Strike\\\\Sim\\\\${SIM_NAME}\\\\hlds\\.exe`], { encoding: "utf8" });
  return (r.stdout ?? "").split("\n").filter(Boolean);
}
function stopSim() {
  for (const p of hldsPids()) spawnSync("kill", [p]);
  for (let i = 0; i < 10 && hldsPids().length; i++) spawnSync("sleep", ["1"]);
  for (const p of hldsPids()) spawnSync("kill", ["-9", p]);
}
function rcon(cmd, timeout = 3000) {
  return new Promise((ok) => {
    const s = createSocket("udp4");
    const done = (v) => (clearTimeout(t), s.close(), ok(v));
    const t = setTimeout(() => done(null), timeout);
    const pre = Buffer.from([255, 255, 255, 255]);
    let challenge = null;
    s.on("message", (m) => {
      const text = m.subarray(4).toString("latin1");
      if (!challenge) {
        challenge = /challenge rcon (\d+)/.exec(text)?.[1];
        if (!challenge) return done(null);
        s.send(Buffer.concat([pre, Buffer.from(`rcon ${challenge} "${PW}" ${cmd}\n`, "latin1")]), SIM_PORT, "127.0.0.1");
      } else done(/Bad rcon/i.test(text) ? null : text.slice(1));
    });
    s.send(Buffer.concat([pre, Buffer.from("challenge rcon\n")]), SIM_PORT, "127.0.0.1");
  });
}
const sleep = (ms) => new Promise((r) => setTimeout(r, DRY ? 0 : ms));
/** The sim server: a clone of --sim-base with this commit's plugins, mods' files and configs. */
function prepareSim() {
  const base = join(SIMS, SIM_BASE);
  if (!existsSync(join(base, "hlds.exe"))) throw new Error(`no sim base ${base} (--sim-base)`);
  // --reuse-sim: a clone an earlier run of the same commit left (--keep), plugins compiled
  // already; only the settings below are written again
  if (flag("--reuse-sim") && existsSync(join(SIM, "cstrike/recheck.cfg"))) {
    log("   sim: reusing the clone (plugins compiled by an earlier run)");
    return writeSimSettings(join(SIM, "cstrike/addons/amxmodx"), { built: 0, bad: [], reused: true });
  }
  removeAll(SIM);
  execFileSync("cp", ["-c", "-R", base, SIM]);
  const am = join(SIM, "cstrike/addons/amxmodx");
  // the mods' data and configs (lang files, ini) for mods this server has, then ours
  for (const mod of readdirSync(join(TREE, "mods"))) {
    const src = join(TREE, "mods", mod, "addons/amxmodx");
    if (!existsSync(src)) continue;
    for (const sub of ["data", "configs"]) copyOver(join(src, sub), join(am, sub), true);
  }
  copyOver(join(TREE, "plugins/configs/gamemodes"), join(am, "configs/gamemodes"), true);
  // plugins: every .amxx this server has whose source is in the commit, compiled fresh the way
  // setup does: a mod's from its catalog part (src and include folder under the mod's
  // scripting), ours from plugins/scripting with its include folder
  const jobs = new Map(); // out name (lower case) -> { src, include }
  for (const f of readdirSync(join(TREE, "plugins/scripting")).filter((x) => x.endsWith(".sma"))) {
    jobs.set(f.slice(0, -4).toLowerCase(), { src: join(TREE, "plugins/scripting", f), include: join(TREE, "plugins/scripting/include") });
  }
  for (const f of readdirSync(join(TREE, "catalog/parts")).filter((x) => x.endsWith(".json"))) {
    const part = JSON.parse(readFileSync(join(TREE, "catalog/parts", f), "utf8"));
    if (!part.vendor) continue;
    const scripting = join(TREE, part.vendor, "addons/amxmodx/scripting");
    for (const c of part.compile ?? []) {
      if (!c.src || !c.out) continue;
      jobs.set(String(c.out).toLowerCase(), { src: join(scripting, ...String(c.src).split("/")), include: c.include ? join(scripting, ...String(c.include).split("/")) : null });
    }
  }
  const bad = [];
  let built = 0;
  for (const f of readdirSync(join(am, "plugins")).filter((x) => x.endsWith(".amxx"))) {
    const job = jobs.get(f.slice(0, -5).toLowerCase());
    if (!job || !existsSync(job.src)) continue;
    const inc = [job.include, join(am, "scripting/include")].filter((i) => i && existsSync(i));
    const argv = ["--bottle", "Steam", "--workdir", winPath(dirname(job.src)), winPath(join(am, "scripting/amxxpc.exe")), winPath(job.src), ...inc.map((i) => `-i${winPath(i)}`), `-o${winPath(join(am, "plugins", f))}`];
    const r = run("sims-compile", WINE, argv, { cwd: SIM, timeout: 300_000 });
    if (r.code !== 0 || !/Done\./.test(r.out)) bad.push(f);
    else built++;
  }
  return writeSimSettings(am, { built, bad });
}
function writeSimSettings(am, result) {
  // the bot brain the commit pins (setup/foundation.rs REGAMEDLL), as setup would install it
  // (RECHECK_BRAIN_DLL: another brain's mp.dll, not checked against the pin: the final check
  // runs the next pin's candidate)
  const brain = process.env.RECHECK_BRAIN_DLL ? { name: `${process.env.RECHECK_BRAIN_DLL} (RECHECK_BRAIN_DLL)`, dll: process.env.RECHECK_BRAIN_DLL } : pinnedBrain();
  copyFileSync(brain.dll, join(SIM, "cstrike/dlls/mp.dll"));
  log(`   sim: bot brain ${brain.name} (sha256 checked)`);
  // bots only, logs to the console, our rcon password, after every map
  const friends = join(am, "data/gamemodes/friends.cfg");
  writeFileSync(friends, `// recheck\nsv_lan 1\nbot_join_after_player 0\nbot_auto_vacate 0\nrcon_password "${PW}"\nlog on\nmp_logecho 1\nmp_logdetail 3\n// Battle Royale starts with bots only (it waits for a person by default)\nbr_wait_human 0\n`);
  // gamemodes.amxx sets rcon_password from panel_rcon.txt on every map: ours there too
  writeFileSync(join(am, "data/gamemodes/panel_rcon.txt"), `${PW}\n`);
  writeFileSync(join(SIM, "cstrike/recheck.cfg"), `rcon_password "${PW}"\nsv_lan 1\nbot_quota 5\nlog on\nmp_logecho 1\n`);
  return result;
}
/** The pinned ReGameDLL zip (RECHECK_BRAIN_ZIP, else the app's download cache, else downloaded),
 *  its SHA-256 checked against the pin, and its win32 mp.dll unpacked into <out>/brain. */
function pinnedBrain() {
  const src = readFileSync(join(TREE, "app/src-tauri/src/setup/foundation.rs"), "utf8");
  const m = /pub const REGAMEDLL: Pinned = Pinned \{\s*url: "([^"]+)",\s*sha256: "([0-9a-f]{64})"/.exec(src);
  if (!m) throw new Error("couldn't read the REGAMEDLL pin in setup/foundation.rs");
  const [, url, sha] = m;
  const name = basename(url);
  const cache = join(homedir(), "Library/Caches/loner-strike/downloads");
  const cached = existsSync(cache) ? readdirSync(cache).find((f) => f.endsWith(`-${name}`)) : undefined;
  let zip = process.env.RECHECK_BRAIN_ZIP || (cached ? join(cache, cached) : join(OUT, name));
  if (!existsSync(zip)) {
    const r = spawnSync("curl", ["-fsSL", "-o", zip, url]);
    if (r.status !== 0) throw new Error(`couldn't download the pinned brain ${url} (set RECHECK_BRAIN_ZIP to a local copy)`);
  }
  const got = execFileSync("shasum", ["-a", "256", zip], { encoding: "utf8" }).split(" ")[0];
  if (got !== sha) throw new Error(`the brain zip ${zip} isn't the pinned one (sha256 ${got}, the pin says ${sha})`);
  const dir = join(OUT, "brain");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  execFileSync("unzip", ["-q", "-o", zip, "bin/win32/cstrike/dlls/mp.dll", "-d", dir]);
  return { name, dll: join(dir, "bin/win32/cstrike/dlls/mp.dll") };
}
function borrowMap(map) {
  const dst = join(SIM, "cstrike");
  if (existsSync(join(dst, "maps", `${map}.bsp`))) return;
  const roots = ["cstrike", "cstrike_downloads"].map((g) => join(GAME, g)).filter(existsSync);
  const find = (rel) => roots.map((r) => join(r, rel)).find(existsSync);
  if (!find(`maps/${map}.bsp`)) return;
  const res = find(`maps/${map}.res`);
  const files = [`maps/${map}.bsp`, `maps/${map}.nav`, `maps/${map}.res`, `overviews/${map}.bmp`, `overviews/${map}.txt`];
  if (res) for (const l of readFileSync(res, "latin1").split(/\r?\n/)) {
    const f = l.trim().replace(/\\/g, "/");
    if (f && !f.startsWith("//") && !f.includes("..")) files.push(f);
  }
  for (const f of files) {
    const src = find(f);
    if (!src || existsSync(join(dst, f))) continue;
    mkdirSync(dirname(join(dst, f)), { recursive: true });
    spawnSync("cp", ["-c", src, join(dst, f)]);
  }
  log(`   sim: borrowed ${map} from the game (${files.length} files listed)`);
}
function copyOver(from, to, overwrite) {
  if (!existsSync(from)) return;
  for (const f of readdirSync(from)) {
    const a = join(from, f);
    const b = join(to, f);
    if (statSync(a).isDirectory()) {
      mkdirSync(b, { recursive: true });
      copyOver(a, b, overwrite);
    } else if (overwrite || !existsSync(b)) copyFileSync(a, b);
  }
}
async function sims() {
  if (DRY) {
    for (const s of SIM_STEPS) simRows.push({ ...s, result: "DRY RUN", detail: "" });
    return { ok: true, detail: `${SIM_STEPS.length} steps` };
  }
  const { built, bad } = prepareSim();
  const qlog = join(SIM, "qconsole.log"); // (-condebug writes it beside hlds.exe)
  rmSync(qlog, { force: true });
  const am = join(SIM, "cstrike/addons/amxmodx");
  writeFileSync(join(am, "data/gamemodes/current.txt"), "classic\n");
  const srv = spawn("nice", ["-n", "15", WINE, "--bottle", "Steam", "--workdir", SIM, join(SIM, "hlds.exe"), "-console", "-game", "cstrike", "-insecure", "-nomaster", "-condebug", "-port", String(SIM_PORT), "+maxplayers", "20", "+exec", "recheck.cfg", "+map", "de_dust2"], { cwd: OUT, stdio: "ignore", detached: false });
  // (cwd anywhere but the server's folder: there wine turns "-game cstrike" into a Windows path,
  // and the server starts as Half-Life with no maps)
  srv.on("error", () => {});
  const up = async () => {
    for (let i = 0; i < 40; i++) {
      if (await rcon("echo up")) {
        // the first map loaded and its plugins up (a mode switch before that is lost)
        for (let j = 0; j < 30 && !(existsSync(qlog) && /Started map "/.test(readFileSync(qlog, "latin1"))); j++) await sleep(2000);
        await sleep(8000);
        return true;
      }
      await sleep(3000);
    }
    return false;
  };
  if (!(await up())) {
    stopSim();
    return { ok: false, detail: "the sim server didn't answer RCON in 2 minutes (sims.log)" };
  }
  // the brain the server runs (an older one prints its usage list for newer ls_mode commands)
  const modeVer = /"ls_mode_ver" is "(\d+)"/.exec((await rcon("ls_mode_ver")) ?? "")?.[1] ?? "missing";
  log(`   sim: ls_mode_ver ${modeVer}`);
  // a step's map the sim doesn't have comes from the game (APFS clones of the .bsp, its nav and
  // what its .res lists), read only
  for (const s of SIM_STEPS) borrowMap(s.map);
  const maps = new Set(readdirSync(join(SIM, "cstrike/maps")).filter((f) => f.endsWith(".bsp")).map((f) => f.slice(0, -4).toLowerCase()));
  const pickSteps = opt("--sim-steps", "") ? opt("--sim-steps", "").split(",") : null;
  for (const s of SIM_STEPS.filter((x) => !pickSteps || pickSteps.includes(x.id))) {
    if (!diskOk(`sim ${s.id}`)) {
      simRows.push({ ...s, result: "SKIPPED", detail: stopped });
      continue;
    }
    if (!maps.has(s.map)) {
      simRows.push({ ...s, result: "SKIPPED", detail: `${s.map} isn't installed in the sim` });
      continue;
    }
    const from = existsSync(qlog) ? statSync(qlog).size : 0;
    const t = Date.now();
    if (s.vote) await rcon("gm_votemode");
    else {
      await rcon(`gm_mode ${s.mode} ${s.map}`);
      // the mode's saved bot count, as the app's Bots page sets it (a plain bot_quota is held),
      // once the new map is up (sent during the map change, it's lost)
      for (let w = 0; w < 30; w++) {
        await sleep(2000);
        const sofar = existsSync(qlog) ? readFileSync(qlog).subarray(from).toString("latin1") : "";
        if (sofar.includes(`Started map "${s.map}"`)) break;
      }
      await sleep(3000);
      await rcon(`gm_bots count ${s.bots ?? 5}`);
      for (let i = 0; i < 4; i++) {
        await sleep(10_000);
        // (one cvar per rcon: "a 0;b 5" in one goes to the server as a's value)
        await rcon("bot_join_after_player 0");
        await rcon(`bot_quota ${s.bots ?? 5}`);
      }
    }
    // (until: stop early once everything it looks for is in the log)
    while (Date.now() - t < s.secs * 1000) {
      await sleep(Math.min(15_000, Math.max(0, s.secs * 1000 - (Date.now() - t))));
      if (!s.until) continue;
      const sofar = existsSync(qlog) ? readFileSync(qlog).subarray(from).toString("latin1") : "";
      if ((s.expect ?? []).every((e) => sofar.includes(e)) && (s.anyOf ?? []).some((e) => sofar.includes(e))) break;
    }
    const alive = !!(await rcon("echo alive", 5000));
    // (the switch took: gm_mode writes current.txt with the mode's plugin list; a mode left over
    // from the clone's own saved state played instead once, with 0 kills)
    const cur = s.vote ? s.mode : (existsSync(join(am, "data/gamemodes/current.txt")) ? readFileSync(join(am, "data/gamemodes/current.txt"), "latin1").trim() : "");
    const text = existsSync(qlog) ? readFileSync(qlog).subarray(from).toString("latin1") : "";
    writeFileSync(join(OUT, `sim-${s.id}.log`), text);
    const kills = (text.match(/" killed "/g) ?? []).length;
    const rounds = (text.match(/World triggered "Round_End"/g) ?? []).length;
    const errs = [...FORBID, ...(s.forbid ?? [])].filter((f) => text.includes(f));
    const missing = [...(s.expect ?? []).filter((e) => !text.includes(e)), ...(s.anyOf && !s.anyOf.some((e) => text.includes(e)) ? [`any of ${s.anyOf.join(" / ")}`] : [])];
    const problems = [
      ...errs.map((e) => `"${e}" in the log`),
      ...missing.map((e) => `no "${e}"`),
      ...(s.kills && kills < s.kills ? [`${kills} kills (wanted ${s.kills}+)`] : []),
      ...(s.rounds && rounds < s.rounds ? [`${rounds} rounds ended (wanted ${s.rounds}+)`] : []),
      ...(!alive ? ["the server stopped answering"] : []),
      ...(cur !== s.mode ? [`the mode didn't switch (current.txt says ${cur || "nothing"})`] : []),
    ];
    simRows.push({ ...s, result: problems.length ? "FAIL" : "PASS", detail: `${kills} kills, ${rounds} rounds${problems.length ? `; ${problems.join("; ")}` : ""}` });
    log(`   sim ${s.id}: ${simRows.at(-1).result} ${simRows.at(-1).detail}`);
    if (!alive) {
      // a dead server: start it again for the next step
      stopSim();
      spawn("nice", ["-n", "15", WINE, "--bottle", "Steam", "--workdir", SIM, join(SIM, "hlds.exe"), "-console", "-game", "cstrike", "-insecure", "-nomaster", "-condebug", "-port", String(SIM_PORT), "+maxplayers", "20", "+exec", "recheck.cfg", "+map", "de_dust2"], { cwd: OUT, stdio: "ignore" }).on("error", () => {});
      await up();
    }
  }
  stopSim();
  const failed = simRows.filter((r) => r.result === "FAIL").length;
  return { ok: !failed && !bad.length, detail: `${built ? `${built} plugins compiled` : "plugins from the kept clone"}${bad.length ? `, ${bad.length} didn't: ${bad.join(", ")}` : ""}; ${simRows.filter((r) => r.result === "PASS").length} pass, ${failed} fail, ${simRows.filter((r) => r.result === "SKIPPED").length} skipped` };
}

// ---- the run ------------------------------------------------------------------------------------
function summary() {
  const lines = [
    `# Combined re-check of ${commit.slice(0, 8)}`,
    "",
    `${new Date().toISOString()} · logs in ${OUT}${stopped ? ` · **stopped: ${stopped}**` : ""}`,
    "",
    "| Step | Result | Time | Details |",
    "|---|---|---|---|",
    ...rows.map((r) => `| ${r.step} | ${r.result} | ${r.secs ? `${Math.floor(r.secs / 60)}m${r.secs % 60}s` : "-"} | ${String(r.detail).replace(/\|/g, "/")} |`),
  ];
  if (simRows.length) {
    lines.push("", "| Sim step | Mode / map | Result | What it shows | Details |", "|---|---|---|---|---|");
    for (const s of simRows) lines.push(`| ${s.id} | ${s.mode} / ${s.map} | ${s.result} | ${s.why} | ${s.detail} |`);
  }
  const text = lines.join("\n") + "\n";
  writeFileSync(join(OUT, "summary.md"), text);
  console.log("\n" + text);
}

// Ctrl+C or a kill: clean up as at the end (the sim server, the clones, the worktree)
let cleaning = false;
function cleanUp() {
  if (cleaning) return;
  cleaning = true;
  if (!DRY) {
    stopSim();
    if (!flag("--keep")) for (const p of [...(only.includes("ui") ? [GAME_CLONE] : []), SIM, APP_DIR]) removeAll(p);
  }
  try {
    dropTree();
  } catch (e) {
    log(`couldn't remove the worktree: ${e.message}`);
  }
}
for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(sig, () => (log(`stopped by ${sig}`), cleanUp(), process.exit(130)));

log(`recheck ${commit} → ${OUT}${DRY ? " (dry run)" : ""}; free disk ${freeGb()} GB`);
try {
  if (diskOk("the worktree")) {
    makeTree();
    // the app embeds the built UI (app/ui/dist): the Rust build and the dev server need it
    if (only.some((s) => ["rust", "ui"].includes(s))) await step("build", buildUi);
    await step("rust", rust);
    await step("node", node);
    await step("ui", ui);
    await step("list", list);
    await step("modfiles", modfiles);
    await step("english", english);
    await step("gitleaks", gitleaks);
    await step("sims", sims);
  }
} finally {
  cleanUp();
  summary();
}
process.exit(rows.some((r) => r.result === "FAIL") || stopped ? 1 : 0);
