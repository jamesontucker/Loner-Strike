#!/usr/bin/env node
// npm run tools:dev — every Tools page in a browser against your real game files, without
// the app (docs/TOOLS-DEV.md):
//   1. the Rust Tools routes over LS_TEST_GAME (cargo test serve_tools_dev, on LS_DEV_PORT 8037)
//   2. the Vite dev server, proxying /api to it
// then open http://localhost:5173/tools-dev.html?open=map:de_dust2 (any tool address).
//
//   --no-rust    only Vite (point LS_API at a server you started, e.g. the app's 8027)
//   --mod czero  Condition Zero instead of CS 1.6
//
// Ctrl+C stops both.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { connect } from "node:net";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const ui = resolve(here, "..");
const tauri = resolve(ui, "../src-tauri");
const args = process.argv.slice(2);
const flag = (f) => args.includes(f);
const opt = (f) => (args.includes(f) ? args[args.indexOf(f) + 1] : undefined);

const GUESSES = [
  join(homedir(), "Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life"),
  join(homedir(), "Library/Application Support/Steam/steamapps/common/Half-Life"),
  "C:/Program Files (x86)/Steam/steamapps/common/Half-Life",
  join(homedir(), ".steam/steam/steamapps/common/Half-Life"),
];
const game = process.env.LS_TEST_GAME || GUESSES.find((p) => existsSync(join(p, "cstrike")));
const port = Number(process.env.LS_DEV_PORT) || 8037;
const api = process.env.LS_API || `http://127.0.0.1:${port}`;
const kids = [];

function stop(code = 0) {
  for (const k of kids) if (k.exitCode === null) k.kill("SIGINT");
  setTimeout(() => process.exit(code), 300);
}
process.on("SIGINT", () => stop(0));
process.on("SIGTERM", () => stop(0));

const listening = (p) =>
  new Promise((ok) => {
    const s = connect(p, "127.0.0.1", () => (s.end(), ok(true)));
    s.on("error", () => ok(false));
  });

async function rust() {
  if (!game) {
    console.error("Couldn't find Half-Life. Set LS_TEST_GAME to the folder with cstrike/ in it.");
    process.exit(1);
  }
  if (await listening(port)) {
    console.log(`Something is already on port ${port}; using it. (Set LS_DEV_PORT for another.)`);
    return;
  }
  console.log(`Tools routes over ${game}\n(building the Rust side first can take a few minutes)`);
  const env = { ...process.env, LS_TEST_GAME: game, LS_DEV_PORT: String(port) };
  if (opt("--mod")) env.LS_TEST_MOD = opt("--mod");
  const k = spawn("cargo", ["test", "serve_tools_dev", "--", "--ignored", "--nocapture"], { cwd: tauri, env, stdio: ["ignore", "inherit", "inherit"] });
  kids.push(k);
  k.on("exit", (c) => {
    console.error(`The Rust Tools server stopped (${c}).`);
    stop(c ?? 1);
  });
  for (let i = 0; i < 1200 && !(await listening(port)); i++) await new Promise((r) => setTimeout(r, 500));
}

if (!flag("--no-rust")) await rust();
const vite = spawn(process.platform === "win32" ? "npx.cmd" : "npx", ["vite"], { cwd: ui, env: { ...process.env, LS_API: api }, stdio: "inherit" });
kids.push(vite);
vite.on("exit", (c) => stop(c ?? 0));
const vport = Number(process.env.LS_VITE_PORT) || 5173;
setTimeout(() => console.log(`\n  Tools: http://localhost:${vport}/tools-dev.html?open=overview\n  e.g.   http://localhost:${vport}/tools-dev.html?open=map:de_dust2\n`), 1500);
