// The reference pages' behavior (site/reference/*, made by scripts/gen-reference.mjs): the search
// at the top of every page (instant, with filters, from the static index: no network), the
// sidebar's menu on phones, copy-link buttons, and On this page following the reading.
(function () {
  var root = document.body.getAttribute("data-root") || "../";
  var q = document.getElementById("ref-q");
  var list = document.getElementById("ref-results");
  var count = document.getElementById("ref-count");
  var box = document.getElementById("ref-results-box");
  var filters = document.getElementById("ref-filters");
  var data = null, loading = false, hits = [], at = -1;

  // the index loads the first time the search is used (or once the page is idle)
  function load(then) {
    if (data) return then && then();
    if (window.LS_REFERENCE) {
      data = window.LS_REFERENCE;
      return then && then();
    }
    if (loading) return;
    loading = true;
    var s = document.createElement("script");
    s.src = root + "reference/index.js";
    s.onload = function () { data = window.LS_REFERENCE || []; fillFilters(); if (then) then(); };
    document.head.appendChild(s);
  }
  if ("requestIdleCallback" in window) requestIdleCallback(function () { load(); }, { timeout: 4000 });
  else setTimeout(function () { load(); }, 1500);

  var esc = function (s) { return String(s || "").replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); };
  var get = function (id) { return document.getElementById(id); };

  // the filters' choices come from the index itself
  function fillFilters() {
    var cats = {}, types = {};
    data.forEach(function (r) { if (r.cat) cats[r.cat] = 1; (r.ty || []).forEach(function (t) { types[t] = 1; }); });
    var opt = function (sel, values) {
      if (!sel || sel.options.length > 1) return;
      Object.keys(values).sort().forEach(function (v) { var o = document.createElement("option"); o.value = v; o.textContent = v; sel.appendChild(o); });
    };
    opt(get("ref-f-cat"), cats);
    opt(get("ref-f-type"), types);
  }

  function words(v) { return v.toLowerCase().split(/[^a-z0-9_.]+/).filter(Boolean); }

  function search() {
    var w = words(q.value);
    var fc = get("ref-f-cat") && get("ref-f-cat").value, fk = get("ref-f-kind") && get("ref-f-kind").value;
    var ft = get("ref-f-type") && get("ref-f-type").value, fb = get("ref-f-bots") && get("ref-f-bots").checked, fl = get("ref-f-limits") && get("ref-f-limits").checked;
    var filtering = fc || fk || ft || fb || fl;
    if (!w.length && !filtering) { box.hidden = true; q.setAttribute("aria-expanded", "false"); count.textContent = ""; hits = []; return; }
    if (!data) { count.textContent = "Loading the index…"; return load(search); }
    hits = [];
    for (var i = 0; i < data.length; i++) {
      var r = data[i];
      if (fc && r.cat !== fc) continue;
      if (fk && r.c !== fk) continue;
      if (ft && (r.ty || []).indexOf(ft) < 0) continue;
      if (fb && !r.b) continue;
      if (fl && !r.l) continue;
      var name = r.n.toLowerCase(), hay = (r.n + " " + (r.k || "") + " " + (r.t || "") + " " + (r.x || "")).toLowerCase();
      var score = 0, ok = true;
      for (var j = 0; j < w.length; j++) {
        if (name.indexOf(w[j]) === 0) score += 6;
        else if (name.indexOf(w[j]) > 0) score += 4;
        else if ((r.k || "").toLowerCase().indexOf(w[j]) >= 0) score += 2;
        else if (hay.indexOf(w[j]) >= 0) score += 1;
        else { ok = false; break; }
      }
      if (ok) hits.push({ r: r, s: score });
    }
    hits.sort(function (a, b) { return b.s - a.s || a.r.n.localeCompare(b.r.n); });
    var shown = hits.slice(0, 40);
    list.innerHTML = shown.map(function (h, i) {
      return '<li role="option" id="rr' + i + '" aria-selected="false" data-u="' + esc(h.r.u) + '"><span class="r-name">' + esc(h.r.n) + "</span>" + (h.r.c ? '<span class="badge k-' + esc(h.r.c) + '">' + esc(h.r.c) + "</span>" : "") +
        '<span class="r-where">' + esc(h.r.t) + "</span><span class=\"r-sum\">" + esc(h.r.x) + "</span></li>";
    }).join("");
    count.textContent = hits.length ? hits.length + (hits.length === 1 ? " result" : " results") + (hits.length > 40 ? " (the first 40 shown)" : "") : "No results";
    box.hidden = false;
    q.setAttribute("aria-expanded", "true");
    select(shown.length ? 0 : -1);
  }
  function select(i) {
    var opts = list.children;
    at = opts.length ? (i + opts.length) % opts.length : -1;
    for (var k = 0; k < opts.length; k++) opts[k].setAttribute("aria-selected", k === at ? "true" : "false");
    if (at >= 0) { q.setAttribute("aria-activedescendant", opts[at].id); opts[at].scrollIntoView({ block: "nearest" }); }
    else q.removeAttribute("aria-activedescendant");
  }
  function open(li) { if (li) location.href = root + li.getAttribute("data-u"); }

  q.addEventListener("input", search);
  q.addEventListener("focus", function () { load(); });
  q.addEventListener("keydown", function (e) {
    if (e.key === "ArrowDown") { e.preventDefault(); select(at + 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); select(at - 1); }
    else if (e.key === "Enter") { e.preventDefault(); open(list.children[at < 0 ? 0 : at]); }
    else if (e.key === "Escape") { q.value = ""; search(); }
  });
  if (filters) filters.addEventListener("change", search);
  list.addEventListener("mousedown", function (e) { if (e.target.closest("[role=option]")) e.preventDefault(); });
  list.addEventListener("click", function (e) { open(e.target.closest("[role=option]")); });
  document.addEventListener("keydown", function (e) {
    if (e.key === "/" && document.activeElement !== q && !/INPUT|TEXTAREA|SELECT/.test(document.activeElement.tagName)) { e.preventDefault(); q.focus(); }
  });

  // copy a card's link
  document.addEventListener("click", function (e) {
    var b = e.target.closest(".copy-link");
    if (!b) return;
    var url = location.origin + location.pathname + "#" + b.getAttribute("data-id");
    var done = function () { var t = b.textContent; b.textContent = "Copied"; setTimeout(function () { b.textContent = t; }, 1400); };
    if (navigator.clipboard) navigator.clipboard.writeText(url).then(done, done); else done();
  });

  // phones: the sidebar is a menu
  var menu = document.querySelector(".ref-menu-btn"), side = document.querySelector(".ref-side");
  if (menu && side) menu.addEventListener("click", function () {
    var open = side.classList.toggle("open");
    menu.setAttribute("aria-expanded", open ? "true" : "false");
  });
  var all = document.querySelector(".ref-all");
  if (all && side) all.addEventListener("click", function () {
    var on = side.classList.toggle("all");
    all.setAttribute("aria-expanded", on ? "true" : "false");
  });

  // a link to a card opens its details (a deep link shows everything about that one)
  function openTarget() {
    var el = location.hash && document.getElementById(decodeURIComponent(location.hash.slice(1)));
    if (el && el.classList.contains("card")) [].forEach.call(el.querySelectorAll("details"), function (d) { d.open = true; });
  }
  window.addEventListener("hashchange", openTarget);
  openTarget();

  // On this page: the card being read
  var toc = document.querySelectorAll(".ref-toc a");
  if (toc.length && "IntersectionObserver" in window) {
    var links = {};
    [].forEach.call(toc, function (a) { links[a.getAttribute("href").slice(1)] = a; });
    var mark = function (id) { [].forEach.call(toc, function (a) { a.classList.toggle("on", a === links[id]); }); };
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (en) { if (en.isIntersecting) mark(en.target.id); });
    }, { rootMargin: "-20% 0px -70% 0px" });
    Object.keys(links).forEach(function (id) { var el = document.getElementById(id); if (el) io.observe(el); });
  }
})();
