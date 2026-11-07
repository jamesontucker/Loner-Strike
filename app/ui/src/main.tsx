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

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {/* anything that crashes outside a page (the top bar, setup, a dialog) still
        leaves a way out instead of a blank window */}
    <PageBoundary>
      <App />
    </PageBoundary>
  </React.StrictMode>,
);
