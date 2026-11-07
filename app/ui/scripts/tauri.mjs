// Cross-platform wrapper for `cargo tauri` that keeps the Rust build output
// outside the repo (in the OS cache dir) on macOS, Windows and Linux.
import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const home = homedir();
const cacheRoot =
  process.platform === "darwin" ? join(home, "Library", "Caches")
  : process.platform === "win32" ? (process.env.LOCALAPPDATA ?? join(home, "AppData", "Local"))
  : (process.env.XDG_CACHE_HOME ?? join(home, ".cache"));

const env = {
  ...process.env,
  CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(cacheRoot, "loner-strike-target"),
};

const srcTauri = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "src-tauri");
const r = spawnSync("cargo", ["tauri", ...process.argv.slice(2)], {
  cwd: srcTauri,
  env,
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(r.status ?? 1);
