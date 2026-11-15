// Map editor: the Checks panel's deeper checks in WebKit, the Mac app's engine
// (the tests: editor-checks.shared.ts; `npx playwright install webkit` once).
import { test } from "@playwright/test";
import { checksTests } from "./editor-checks.shared";

test.use({ browserName: "webkit" });
checksTests();
