import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PageBoundary } from "./ui";
import "./styles.css";

// "Loner-Strike" never splits at its hyphen at the end of a line: an invisible word
// joiner after the hyphen, added to the text wherever the name appears (toasts and
// dialogs included, which live outside #root)
const JOINED = "Loner-\u2060Strike";
const join = (n: Node) => {
  if (n.nodeType === Node.TEXT_NODE) {
    const t = n.nodeValue;
    if (t && t.includes("Loner-Strike")) n.nodeValue = t.split("Loner-Strike").join(JOINED);
  } else {
    const walk = document.createTreeWalker(n, NodeFilter.SHOW_TEXT);
    for (let t = walk.nextNode(); t; t = walk.nextNode()) join(t);
  }
};
new MutationObserver((changes) => {
  for (const c of changes) {
    if (c.type === "characterData") join(c.target);
    else c.addedNodes.forEach(join);
  }
}).observe(document.body, { subtree: true, childList: true, characterData: true });

// Outside links (target=_blank, window.open) do nothing in the app's window, which is a web
// view, not a browser: on this computer they go to the app, which opens them in the
// default browser. (On a phone the panel is a real browser, which opens them itself.)
if (["127.0.0.1", "localhost"].includes(location.hostname)) {
  const outside = (href: string) => {
    try {
      const u = new URL(href, location.href);
      return (u.protocol === "https:" || u.protocol === "http:") && u.origin !== location.origin ? u.href : null;
    } catch {
      return null;
    }
  };
  const openOutside = (url: string) =>
    fetch("/api/app/open-url", { method: "POST", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify({ url }) }).catch(() => {});
  document.addEventListener(
    "click",
    (e) => {
      if (e.defaultPrevented || e.button !== 0) return;
      const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      const url = a && outside(a.getAttribute("href") ?? "");
      if (!url) return;
      e.preventDefault();
      openOutside(url);
    },
    true,
  );
  const open = window.open.bind(window);
  window.open = (target?: string | URL, ...rest) => {
    const url = target ? outside(String(target)) : null;
    if (!url) return open(target, ...rest);
    openOutside(url);
    return null;
  };
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* anything that crashes outside a page (the top bar, setup, a dialog) still
        leaves a way out instead of a blank window */}
    <PageBoundary>
      <App />
    </PageBoundary>
  </React.StrictMode>,
);
