// MCP (Windows audit, docs/notes/Windows.md): WebView2 on Windows acts like a browser where
// WKWebView on the Mac doesn't. Unhandled F5 / Ctrl+R reload the whole app, Ctrl+P prints,
// Ctrl+F / F3 open a find bar, F7 asks about caret browsing; a file dropped from Explorer
// replaces the app with that file; right-click shows Back, Refresh, Save as and Print.
// These guards run last (window, bubbling), so every key, drop and menu the app handles itself
// still works; they only stop what nothing in the app took.

/** A key WebView2 would act on as a browser, when the app didn't handle it. */
export function browserKey(e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }): boolean {
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (["F5", "F3", "F7", "BrowserRefresh", "BrowserSearch"].includes(k) && !e.altKey) return true;
  // (Ctrl only: on a Mac ⌘H, ⌘Q and the like belong to the system)
  return e.ctrlKey && !e.metaKey && !e.altKey && ["r", "p", "f", "g", "s", "o", "u", "j", "h"].includes(k);
}

/** A drag of files from outside the app (Explorer, Finder): never dropped on the page itself. */
export function outsideFiles(types: readonly string[] | DOMStringList | null | undefined): boolean {
  if (!types) return false;
  for (let i = 0; i < types.length; i++) {
    const t = "item" in types && typeof types.item === "function" ? types.item(i) : (types as readonly string[])[i];
    if (t === "Files") return true;
  }
  return false;
}

/** Where the browser's own right-click menu helps (cut, copy, paste, spelling): text being typed or selected. */
export function nativeMenuHelps(target: { closest?: (s: string) => unknown } | null, hasSelection: boolean): boolean {
  if (hasSelection) return true;
  return !!target?.closest?.('input, textarea, [contenteditable=""], [contenteditable="true"], .monaco-editor');
}

/** Installs the guards once (main.tsx). */
export function installWindowsGuards() {
  window.addEventListener("keydown", (e) => {
    if (!e.defaultPrevented && browserKey(e)) e.preventDefault();
  });
  // (the page's own drags carry their own types; only plain files from outside are stopped)
  window.addEventListener("dragover", (e) => {
    if (!e.defaultPrevented && outsideFiles(e.dataTransfer?.types)) {
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
    }
  });
  window.addEventListener("drop", (e) => {
    if (!e.defaultPrevented && outsideFiles(e.dataTransfer?.types)) e.preventDefault();
  });
  window.addEventListener("contextmenu", (e) => {
    if (e.defaultPrevented) return;
    const sel = window.getSelection?.();
    if (!nativeMenuHelps(e.target as Element | null, !!sel && !sel.isCollapsed)) e.preventDefault();
  });
}
