// Screenshot tests for the Tools pages (tests/tools.spec.ts, docs/TOOLS-DEV.md):
//   npm run test:tools          (starts `npm run tools:dev` if it isn't running)
// They load every tool page on tools-dev.html against your real game files and fail on
// console errors, crashes, or a page wider than the window. Screenshots land in test-results/.
import { defineConfig } from "@playwright/test";

const port = Number(process.env.LS_VITE_PORT) || 5173;

export default defineConfig({
  testDir: "tests",
  timeout: 90_000,
  workers: 1,
  reporter: [["list"]],
  use: { baseURL: `http://localhost:${port}`, browserName: "chromium", headless: true },
  projects: [
    { name: "large", use: { viewport: { width: 1440, height: 900 } } },
    { name: "small", use: { viewport: { width: 1024, height: 680 } } },
    // WebKit (the Mac app's engine) for the specs listed here (`--project webkit`;
    // `npx playwright install webkit` once)
    // (LS_WEBKIT_ALL=1, the combined re-check: every spec in WebKit too)
    { name: "webkit", testMatch: process.env.LS_WEBKIT_ALL ? undefined : ["callouts.spec.ts", "qa4-t2-fixes.spec.ts", "studio-help.spec.ts", "studio-ux.spec.ts", "editor-filequeue.spec.ts", "community-remix.spec.ts", "qa4-t1-plugins-demos.spec.ts", "qa4-core.spec.ts", "community-gb-credits.spec.ts", "server-name-bytes.spec.ts"], use: { browserName: "webkit", viewport: { width: 1440, height: 900 } } },
  ],
  webServer: {
    command: "node scripts/tools-dev.mjs",
    url: `http://localhost:${port}/api/tools/dev`,
    reuseExistingServer: true,
    // (the first run builds the Rust side)
    timeout: 900_000,
    stdout: "ignore",
  },
});
