// The editor coverage sweep (docs/notes/EditorCoverage.md), before a release:
//
//   1. a Tools dev server over a COPY of the game, with a throwaway app folder and the file
//      dialogs answered by two paths (the spec re-points the first, a symlink):
//        LS_TEST_GAME=<sim copy> LS_APP_DIR=<scratch>/appdir LS_DEV_PORT=<port> \
//        LS_DEV_PICK=<scratch>/pick/current LS_DEV_PICK_SAVE=<scratch>/out/qa.lsmap \
//        cargo test serve_tools_dev -- --ignored --nocapture
//   2. Vite with its own cache (docs/TOOLS-CREW.md), LS_API at that server
//   3. LS_VITE_PORT=<vite port> LS_COV_PICK=<scratch>/pick/current LS_COV_SAVE=<scratch>/out/qa.lsmap \
//        npx playwright test -c tests/editor-coverage.config.ts
//
// The dev server tells a project from a .map or a leak line by the picked path's extension: run
// again with LS_DEV_PICK / LS_COV_PICK ending .lsmap (Open a project) and .lin (Load pointfile);
// tests that need another server skip. docs/notes/EditorCoverage.md has the details.
//
// Chromium runs everything; WebKit (the Mac app's engine) runs the input- and drawing-
// sensitive specs (keys, tools in the views, panels).
import { defineConfig } from "@playwright/test";
import base from "../playwright.config";

export default defineConfig({
  ...base,
  testDir: ".",
  testMatch: ["editor-coverage-*.spec.ts"],
  timeout: 90_000,
  retries: 0,
  // its own folder (other runs empty test-results/ when they start); LS_COV_OUT moves it
  outputDir: process.env.LS_COV_OUT || "../test-results/editor-coverage",
  use: { ...base.use, actionTimeout: 10_000 },
  projects: [
    { name: "chromium", use: { browserName: "chromium", viewport: { width: 1440, height: 900 } } },
    {
      name: "webkit",
      testMatch: ["editor-coverage-keys.spec.ts", "editor-coverage-panels.spec.ts"],
      use: { browserName: "webkit", viewport: { width: 1440, height: 900 } },
    },
  ],
});
