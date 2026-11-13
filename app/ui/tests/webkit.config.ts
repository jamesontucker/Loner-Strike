// The nav fixer's UX test in WebKit (Safari's engine), against the same dev server as the
// Chromium runs (playwright.config.ts):
//   npx playwright test -c tests/webkit.config.ts
// (npx playwright install webkit once, if it isn't there)
import { defineConfig } from "@playwright/test";
import base from "../playwright.config";

export default defineConfig({
  ...base,
  testDir: ".",
  testMatch: ["navfix-ux.spec.ts"],
  use: { ...base.use, browserName: "webkit" },
  projects: [{ name: "webkit", use: { browserName: "webkit", viewport: { width: 1440, height: 900 } } }],
});
