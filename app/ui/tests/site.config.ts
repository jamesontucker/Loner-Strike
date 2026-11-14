// The website's own pages (site/), served as plain files: no app, no dev server.
//   npx playwright test -c tests/site.config.ts
import { defineConfig } from "@playwright/test";

const port = Number(process.env.LS_SITE_PORT) || 8763;

export default defineConfig({
  testDir: ".",
  testMatch: ["site-*.spec.ts"],
  timeout: 60_000,
  workers: 1,
  reporter: [["list"]],
  use: { baseURL: `http://127.0.0.1:${port}`, headless: true },
  projects: [
    { name: "chromium", use: { browserName: "chromium", viewport: { width: 1440, height: 900 } } },
    { name: "webkit", use: { browserName: "webkit", viewport: { width: 1440, height: 900 } } },
  ],
  webServer: {
    command: `python3 -m http.server ${port} --bind 127.0.0.1 --directory ../../../site`,
    url: `http://127.0.0.1:${port}/docs.html`,
    reuseExistingServer: true,
    timeout: 30_000,
    stdout: "ignore",
  },
});
