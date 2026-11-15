// MCP: the Windows (WebView2) guards' rules (src/windowsGuards.ts).
//   node scripts/test-windows.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { browserKey, nativeMenuHelps, outsideFiles } from "../src/windowsGuards";

const k = (key: string, m: Partial<{ ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }> = {}) => ({ key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...m });

test("browser keys WebView2 would act on", () => {
  for (const e of [k("F5"), k("F5", { ctrlKey: true }), k("F3"), k("F7"), k("r", { ctrlKey: true }), k("R", { ctrlKey: true, shiftKey: true }), k("p", { ctrlKey: true }), k("f", { ctrlKey: true })]) assert.ok(browserKey(e), JSON.stringify(e));
  for (const e of [k("r"), k("F2"), k("z", { ctrlKey: true }), k("c", { ctrlKey: true }), k("v", { ctrlKey: true }), k("h", { metaKey: true }), k("r", { ctrlKey: true, altKey: true }), k("F5", { altKey: true })]) assert.ok(!browserKey(e), JSON.stringify(e));
});

test("files from Explorer are stopped; the app's own drags aren't", () => {
  assert.ok(outsideFiles(["Files"]));
  assert.ok(outsideFiles(["text/uri-list", "Files"]));
  assert.ok(!outsideFiles(["application/x-ls-asset"]));
  assert.ok(!outsideFiles(null));
});

test("the native right-click menu only where it helps", () => {
  const at = (inField: boolean) => ({ closest: (s: string) => (inField && s.includes("input") ? {} : null) });
  assert.ok(nativeMenuHelps(at(true), false));
  assert.ok(nativeMenuHelps(at(false), true));
  assert.ok(!nativeMenuHelps(at(false), false));
  assert.ok(!nativeMenuHelps(null, false));
});
