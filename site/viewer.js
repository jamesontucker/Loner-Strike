// The big screenshot viewer, shared by the home page's tour and the docs' pictures: one dialog over
// the page. Esc, × or a click beside the picture closes it and focus goes back to what opened it;
// the arrow keys, the buttons and a swipe step through the list. The full-size picture (WebP, PNG
// for older browsers) loads only when it's shown.
//   LSViewer.open(items, at, { onStep(i), returnTo })   items: [{ webp, src, cap }]
(() => {
  let dlg = document.getElementById("viewer");
  if (!dlg) {
    dlg = document.createElement("dialog");
    dlg.className = "viewer";
    dlg.id = "viewer";
    dlg.setAttribute("aria-label", "Screenshot");
    document.body.append(dlg);
  }
  dlg.innerHTML =
    '<figure><picture><source type="image/webp"><img alt=""></picture><figcaption></figcaption></figure>' +
    '<button type="button" class="nav prev" aria-label="Previous screenshot">‹</button>' +
    '<button type="button" class="nav next" aria-label="Next screenshot">›</button>' +
    '<button type="button" class="close" aria-label="Close">×</button><span class="count"></span>';
  const $ = (s) => dlg.querySelector(s);
  let list = [], at = 0, back = null, onStep = null;
  const show = () => {
    const s = list[at];
    if (!s) return;
    const source = $("source");
    if (s.webp) source.srcset = s.webp;
    else source.removeAttribute("srcset");
    $("img").src = s.src || s.webp;
    $("img").alt = s.cap || "";
    $("figcaption").textContent = s.cap || "";
    $(".count").textContent = list.length > 1 ? `${at + 1} / ${list.length}` : "";
    $(".prev").hidden = $(".next").hidden = list.length < 2;
  };
  const step = (d) => {
    if (list.length < 2) return;
    at = (at + d + list.length) % list.length;
    if (onStep) onStep(at);
    show();
  };
  $(".prev").addEventListener("click", () => step(-1));
  $(".next").addEventListener("click", () => step(1));
  $(".close").addEventListener("click", () => dlg.close());
  dlg.addEventListener("click", (e) => { if (e.target === dlg || e.target.tagName === "FIGURE") dlg.close(); });
  dlg.addEventListener("keydown", (e) => {
    if (e.key === "ArrowLeft") step(-1);
    if (e.key === "ArrowRight") step(1);
  });
  let x0 = null;
  dlg.addEventListener("touchstart", (e) => { x0 = e.touches[0].clientX; }, { passive: true });
  dlg.addEventListener("touchend", (e) => {
    if (x0 === null) return;
    const dx = e.changedTouches[0].clientX - x0;
    if (Math.abs(dx) > 50) step(dx < 0 ? 1 : -1);
    x0 = null;
  });
  dlg.addEventListener("close", () => {
    const b = back;
    back = onStep = null;
    if (b && b.isConnected) b.focus({ preventScroll: true });
  });
  window.LSViewer = {
    open(items, i, opts = {}) {
      list = items;
      at = i;
      onStep = opts.onStep || null;
      back = opts.returnTo || document.activeElement;
      show();
      if (!dlg.open) dlg.showModal();
      $(".close").focus();
    },
  };
})();
