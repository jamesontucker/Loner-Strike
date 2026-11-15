#!/usr/bin/env node
// Generated reference docs (Parity A): the Game Mode Studio node reference, the map editor's
// entity reference and its tools and keys, written as static pages in site/reference/, plus a
// search index the docs page loads (site/reference/index.js), the Reference group in docs.html
// (between REFERENCE markers) and a compact form in llms-full.txt (between its markers).
//
//   node scripts/gen-reference.mjs           write everything
//   node scripts/gen-reference.mjs --check   only check: exit 1 if a node or entity has no help
//
// Sources: app/ui/src/views/tools/studio/nodes.json and nodehelp.json (the same help the Studio's
// node help panel and the AI connection's studio_node_help read), limits.ts, schema.ts's built-in
// enums, the starter library items (library/*.lib.json); app/src-tauri/src/editor/cstrike.fgd,
// lonerstrike.fgd and components.json; docs/notes/EditorCoverage.md's inventory.

import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync, unlinkSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const studio = join(root, "app/ui/src/views/tools/studio");
const editor = join(root, "app/src-tauri/src/editor");
const site = join(root, "site");
const read = (p) => readFileSync(p, "utf8");
const json = (p) => JSON.parse(read(p));

// ---- reading -------------------------------------------------------------------------------

/** limits.ts's LIMITS, by node type. */
function limits() {
  const text = read(join(studio, "limits.ts"));
  const out = {};
  for (const m of text.matchAll(/\{\s*id:\s*"([^"]+)",\s*label:\s*"([^"]+)",\s*default:\s*([\d.]+),\s*lagAbove:\s*([\d.]+),\s*max:\s*([\d.]+),\s*kinds:\s*\[([^\]]*)\]/g)) {
    const lim = { id: m[1], label: m[2], default: Number(m[3]), lagAbove: Number(m[4]), max: Number(m[5]) };
    for (const k of m[6].matchAll(/"([^"]+)"/g)) (out[k[1]] ??= []).push(lim);
  }
  return out;
}

/** schema.ts's BUILTIN_ENUMS. */
function builtinEnums() {
  const text = read(join(studio, "schema.ts"));
  const block = text.slice(text.indexOf("export const BUILTIN_ENUMS"), text.indexOf("];", text.indexOf("export const BUILTIN_ENUMS")));
  return [...block.matchAll(/id:\s*"([^"]+)",\s*name:\s*"([^"]+)",\s*values:\s*\[([^\]]*)\]/g)].map((m) => ({ id: m[1], name: m[2], values: [...m[3].matchAll(/"([^"]+)"/g)].map((v) => v[1]) }));
}

/** The starter library items (name, category, what it is, its inputs and outputs). */
function library() {
  const dir = join(studio, "library");
  return readdirSync(dir)
    .filter((f) => f.endsWith(".lib.json"))
    .sort()
    .map((f) => {
      const d = json(join(dir, f));
      const item = [...(d.macros ?? []), ...(d.functions ?? [])].find((x) => x.id === d.root) ?? {};
      return { file: f, name: d.name, category: d.category ?? "", kind: d.kind ?? "", description: d.description ?? "", inputs: item.inputs ?? [], outputs: item.outputs ?? [], tables: (d.tables ?? []).map((t) => t.name) };
    });
}

/** Every node with its help. */
export function nodes() {
  const cat = json(join(studio, "nodes.json"));
  const help = json(join(studio, "nodehelp.json")).help;
  const lim = limits();
  return {
    pinTypes: cat.pinTypes,
    categories: cat.categories,
    nodes: cat.nodes
      .filter((n) => !n.superseded)
      .map((n) => {
        const h = help[n.type] ?? {};
        const pinHelp = Object.fromEntries((h.pins ?? []).map((p) => [`${p.dir}:${p.id}`, p.help]));
        const setHelp = Object.fromEntries((h.settings ?? []).map((p) => [p.id, p.help]));
        return {
          type: n.type,
          title: n.title,
          category: n.category,
          what: h.what || n.help || "",
          more: h.more || "",
          needs: h.needs || n.needs || "",
          risk: h.risk || "",
          cost: h.cost || "",
          inputs: n.inputs.map((p) => ({ id: p.id, label: p.label || "", type: p.type, help: pinHelp[`in:${p.id}`] || "" })),
          outputs: n.outputs.map((p) => ({ id: p.id, label: p.label || "", type: p.type, help: pinHelp[`out:${p.id}`] || "" })),
          settings: (n.params ?? []).map((p) => ({ id: p.id, label: p.label, kind: p.kind, default: p.default, min: p.min, max: p.max, choices: p.choices, help: setHelp[p.id] || "" })),
          limits: lim[n.type] ?? [],
          related: (h.related ?? []).filter((r) => cat.nodes.some((x) => x.type === r && !x.superseded)),
          event: !!n.event,
          pure: !!n.pure,
        };
      }),
  };
}

/** A minimal FGD reader: classes (with their base classes' keys), keys, choices and flags. */
function parseFgd(text) {
  // comments out, string concatenation joined
  const src = text.replace(/\/\/[^\n]*/g, "").replace(/"\s*\+\s*"/g, "");
  const classes = {};
  const re = /@(\w+)\s*([^=]*?)=\s*([\w]+)\s*(?::\s*"([^"]*)")?\s*(?::\s*"([^"]*)")?\s*\[/g;
  let m;
  while ((m = re.exec(src))) {
    // the body: up to the matching ]
    let depth = 1, i = re.lastIndex;
    for (; i < src.length && depth; i++) if (src[i] === "[") depth++; else if (src[i] === "]") depth--;
    const body = src.slice(re.lastIndex, i - 1);
    re.lastIndex = i;
    const bases = (/base\(([^)]*)\)/.exec(m[2])?.[1] ?? "").split(",").map((s) => s.trim()).filter(Boolean);
    const keys = [];
    const kre = /^\s*(\w+)\s*\(\s*(\w+)\s*\)\s*(?::\s*"([^"]*)")?\s*(?::\s*("[^"]*"|[^:=\n]*))?\s*(?::\s*"([^"]*)")?\s*(=\s*\[)?/gm;
    let k;
    while ((k = kre.exec(body))) {
      const key = { name: k[1], type: k[2].toLowerCase(), label: k[3] ?? "", default: (k[4] ?? "").trim().replace(/^"|"$/g, ""), help: k[5] ?? "", options: [] };
      if (k[6]) {
        let d = 1, j = kre.lastIndex;
        for (; j < body.length && d; j++) if (body[j] === "[") d++; else if (body[j] === "]") d--;
        const opts = body.slice(kre.lastIndex, j - 1);
        key.options = [...opts.matchAll(/^\s*("?[^:\n"]+"?)\s*:\s*"([^"]*)"(?:\s*:\s*(\d+))?/gm)].map((o) => ({ value: o[1].replace(/"/g, "").trim(), label: o[2], on: o[3] === "1" }));
        kre.lastIndex = j;
      }
      keys.push(key);
    }
    classes[m[3]] = { kind: m[1], name: m[3], title: m[4] && m[5] ? m[4] : "", help: m[5] ?? m[4] ?? "", bases, keys };
  }
  // base classes' keys in, the class's own last (they win)
  const resolve = (c, seen = new Set()) => {
    let keys = [];
    for (const b of c.bases) if (classes[b] && !seen.has(b)) keys = [...keys, ...resolve(classes[b], new Set([...seen, b]))];
    const mine = new Set(c.keys.map((k) => k.name));
    return [...keys.filter((k) => !mine.has(k.name)), ...c.keys];
  };
  return Object.values(classes)
    .filter((c) => c.kind !== "BaseClass")
    .map((c) => ({ ...c, keys: resolve(c) }));
}

/** Every entity class the editor knows, with its group (components.json) and where it's from. */
export function entities() {
  const comps = json(join(editor, "components.json"));
  const byClass = Object.fromEntries((comps.components ?? []).map((c) => [c.class, c]));
  const out = [];
  for (const [file, source] of [["cstrike.fgd", "Counter-Strike"], ["lonerstrike.fgd", "Loner-Strike components"]]) {
    for (const c of parseFgd(read(join(editor, file)))) {
      const comp = byClass[c.name];
      out.push({ ...c, source, group: comp?.group ?? (c.kind === "SolidClass" ? "Brush entities" : "Point entities"), title: comp?.title ?? c.title, blurb: comp?.blurb ?? "" });
    }
  }
  return out.sort((a, b) => a.source.localeCompare(b.source) || a.group.localeCompare(b.group) || a.name.localeCompare(b.name));
}

/** EditorCoverage.md's inventory: what the editor does and where to find it (no test status). */
export function editorInventory() {
  const text = read(join(root, "docs/notes/EditorCoverage.md"));
  const inv = text.slice(text.indexOf("## Inventory"));
  const sections = [];
  let cur = null;
  for (const line of inv.split("\n")) {
    const h = /^### (.+)/.exec(line);
    if (h) {
      cur = { name: h[1].trim(), rows: [] };
      if (!/^WebKit|TrenchBroom/.test(cur.name)) sections.push(cur);
      continue;
    }
    if (!cur || !line.startsWith("|") || /^\|\s*-/.test(line) || /^\|\s*(Item|Key|Panel)\s*\|/i.test(line)) continue;
    const cells = line.split("|").slice(1, -1).map((c) => c.trim());
    if (cells.length >= 2 && cells[0]) cur.rows.push({ item: cells[0].replace(/`/g, ""), where: cells[1].replace(/`/g, "") });
  }
  return sections.filter((s) => s.rows.length);
}

// ---- checks ------------------------------------------------------------------------------

/** What has no help text (a node's what, its pins and settings; an entity's description). */
export function missingHelp() {
  const out = [];
  for (const n of nodes().nodes) {
    if (!n.what.trim()) out.push(`node ${n.type}: no "what it does"`);
    for (const p of [...n.inputs, ...n.outputs]) if (!p.help.trim() && p.type !== "exec") out.push(`node ${n.type}: pin ${p.id} has no help`);
    for (const p of n.settings) if (!p.help.trim()) out.push(`node ${n.type}: setting ${p.id} has no help`);
  }
  for (const e of entities()) if (!e.help.trim()) out.push(`entity ${e.name}: no description`);
  return out;
}

/** Entity keys without a description (reported, not failed: many are Valve's own). */
export function keysWithoutHelp() {
  return entities().flatMap((e) => e.keys.filter((k) => !k.help.trim() && !["spawnflags", "targetname", "target", "origin", "angles"].includes(k.name)).map((k) => `${e.name}.${k.name}`));
}

// ---- writing -------------------------------------------------------------------------------

const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const slug = (s) => String(s).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
const nodeId = (t) => `n-${slug(t)}`;
const entId = (n) => `e-${slug(n)}`;
const typeLabel = (types, t) => types[t]?.label ?? t.replace(/^(struct|enum|array):/, "");
const catsOf = (data) => [...new Set([...data.categories, ...data.nodes.map((n) => n.category)])].filter((c) => data.nodes.some((n) => n.category === c));
/** A node's page and anchor, from reference/ (`nodes/events.html#n-…`). */
const nodeHref = (n) => `nodes/${slug(n.category)}.html#${nodeId(n.type)}`;
const entGroup = (e) => `${e.source === "Counter-Strike" ? "" : "Loner-Strike: "}${e.group}`;
const entHref = (e) => `entities/${slug(entGroup(e))}.html#${entId(e.name)}`;
/** The first sentence (for one-line summaries). */
const firstSentence = (t) => {
  const m = /^(.+?[.!?])(\s|$)/.exec(String(t).trim());
  return m ? m[1] : String(t).trim();
};
const norm = (t) => String(t).toLowerCase().replace(/[^a-z0-9]+/g, " ").trim();
/** Whether `b` mostly says what `a` says already (most of its words are in it). */
const saysSame = (a, b) => {
  const have = new Set(norm(a).split(" "));
  const words = norm(b).split(" ").filter((w) => w.length > 2);
  return !words.length || words.filter((w) => have.has(w)).length / words.length >= 0.6;
};
/** The node's kind: an event, a value (pure), flow, or an action. */
// Flow: the Flow category, every flow.* node, loops (an exec out called Each or
// Loop Body) and plain True/False branches.
const execOuts = (n) => (n.outputs || []).filter((p) => p.type === "exec").map((p) => (p.label || p.id || "").toLowerCase());
const isFlow = (n) => n.category === "Flow" || n.type.startsWith("flow.") || /(^|\.)for_each/.test(n.type) || execOuts(n).some((l) => l === "each" || l === "loop body") || (execOuts(n).length === 2 && execOuts(n).every((l) => l === "true" || l === "false"));
const kindOf = (n) => (n.event ? "event" : n.pure ? "value" : isFlow(n) ? "flow" : "action");
/** What the help says about bots, sentence by sentence. */
const botLines = (n) => [n.what, n.more, n.needs].join(" ").split(/(?<=[.!?])\s+/).filter((x) => /\bbots?\b/i.test(x));
const hasBots = (n) => n.category === "Bots" || botLines(n).length > 0;

/** docs.html's look: its <style> block as a stylesheet the reference pages share. */
function baseCss() {
  const d = read(join(site, "docs.html"));
  return `/* Made by scripts/gen-reference.mjs from docs.html's own styles: the reference looks like the docs. */\n${d.slice(d.indexOf("<style>") + 7, d.indexOf("</style>")).replace(/url\("fonts\//g, 'url("../fonts/')}`;
}

/** A reference page: the docs' top bar, the search with filters, the sidebar, the main column, On this page. */
function page({ title, lead, side, main, toc = "", file, up = "../", kinds = true, section = "" }) {
  // (the three references and the guides are always one click away)
  const sw = [["nodes", "reference/nodes.html", "Nodes", "Studio nodes"], ["entities", "reference/entities.html", "Entities", "Map entities"], ["editor", "reference/editor.html", "Editor", "Editor tools"], ["guides", "reference/#guides", "Guides", "Guides"]]
    .map(([id, href, label, full]) => `<a href="${up}${href}" title="${full}"${id === section ? ' aria-current="true"' : ""}>${label}</a>`).join("");
  // (phones: only the current category stays in the menu, the rest behind one button)
  if ((side.match(/<details/g) || []).length > 8 && side.includes('class="here"')) side += `<button class="ref-all" type="button" aria-expanded="false">All ${section === "entities" ? "groups" : "categories"}</button>`;
  side = `<div class="ref-switch" role="navigation" aria-label="Reference sections">${sw}</div>${side}`;
  const filters = kinds
    ? `<label>Category <select id="ref-f-cat"><option value="">Any</option></select></label>
      <label>Kind <select id="ref-f-kind"><option value="">Any</option><option value="event">Event</option><option value="action">Action</option><option value="value">Value</option><option value="flow">Flow</option><option value="point">Point entity</option><option value="brush">Brush entity</option></select></label>
      <label>Type <select id="ref-f-type"><option value="">Any</option></select></label>
      <label><input type="checkbox" id="ref-f-bots"> Works with bots</label>
      <label><input type="checkbox" id="ref-f-limits"> Has limits</label>`
    : "";
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)} · Loner-Strike reference</title>
<meta name="description" content="${esc(lead)}">
<link rel="icon" href="${up}favicon.ico">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Barlow+Semi+Condensed:wght@500;600;700&display=swap" rel="stylesheet">
<link rel="stylesheet" href="${up}reference/base.css">
<link rel="stylesheet" href="${up}reference/ref.css">
</head>
<body data-root="${up}">
<a class="sr-only skip" href="#content">Skip to the reference</a>
<header class="top"><div class="wrap"><div class="brand"><a class="wordmark" href="/">Loner-Strike</a><span class="here">Reference</span></div>
<nav aria-label="Site"><a href="/">Home</a><a href="${up}docs.html#whats-new">What's new</a><a href="${up}docs.html#tools">Tools</a><a href="${up}docs.html#vps">Run a server</a><a href="${up}reference/" aria-current="page">Reference</a><a class="btn" href="/#get">Get it</a></nav></div></header>
<div class="ref-searchbar" role="search"><div class="wrap">
  <label class="sr-only" for="ref-q">Search the reference</label>
  <input id="ref-q" class="ref-q" type="search" placeholder="Search nodes, entities and editor tools (press /)" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="ref-results">
  ${kinds ? `<details class="ref-filter-box"><summary>Filters</summary><div class="ref-filters" id="ref-filters">${filters}</div></details>` : ""}
  <p class="ref-count" id="ref-count" aria-live="polite"></p>
  <div class="ref-results-box" id="ref-results-box" hidden><ul id="ref-results" role="listbox" aria-label="Results"></ul></div>
</div></div>
<div class="ref-layout">
  <div class="ref-rail"><button class="ref-menu-btn" type="button" aria-expanded="false" aria-controls="ref-side">${esc(title)} <span aria-hidden="true">▾</span></button>
  <nav class="ref-side" id="ref-side" aria-label="Reference">${side}</nav></div>
  <main class="ref-main" id="content" tabindex="-1">
    <p class="crumb"><a href="${up}docs.html#reference">Reference</a></p>
    <h1>${esc(title)}</h1>
    <p class="lead">${esc(lead)}</p>
${main}
    <p class="meta ty">Made from the app's own catalog (${esc(file)}), so it matches the app.</p>
  </main>
  ${toc ? `<nav class="ref-toc" aria-label="On this page"><p>On this page</p><ul>${toc}</ul></nav>` : "<div></div>"}
</div>
<footer><a href="/">lonerstrike.com</a> · <a href="${up}docs.html">Docs</a> · <a href="https://github.com/jamesontucker/Loner-Strike">GitHub</a></footer>
<script src="${up}reference/ref.js" defer></script>
</body>
</html>
`;
}

/** The sidebar for nodes: categories (folding, with counts), the open one's nodes listed. */
function nodeSide(data, here, up) {
  return catsOf(data)
    .map((c) => {
      const list = data.nodes.filter((n) => n.category === c);
      const open = c === here;
      return `<details${open ? " open" : ""} class="${open ? "here" : ""}"><summary><a href="${up}reference/nodes/${slug(c)}.html"${open ? ' aria-current="page"' : ""}>${esc(c)}</a><span class="n">${list.length}</span></summary>${open ? `<ul>${list.map((n) => `<li><a href="#${nodeId(n.type)}">${esc(n.title)}</a></li>`).join("")}</ul>` : ""}</details>`;
    })
    .join("");
}

const pinColor = (types, t) => types[t]?.color ?? (t.startsWith("enum:") ? "#ff9ad5" : t.startsWith("struct:") ? "#7fb2ff" : t.startsWith("array:") ? "#b0b0b0" : "#cccccc");
const CAT_COLORS = { Events: "#8b2e2e", Flow: "#4b4b4b", Conditions: "#3e6a3e", Numbers: "#3e6a3e", Text: "#5a3e6a", Variables: "#3e5a6a" };

/** Nodes whose pins are counted get "+ Add pin" on the canvas (X2, pinCountOps.ts): how many show
 *  at first and how many they can have. */
function addPinNote(n) {
  const count = (n.settings || []).find((s) => s.id === "count");
  const cases = (n.settings || []).find((s) => s.id === "cases");
  let text = "";
  if (["flow.sequence", "flow.multi_gate", "array.make"].includes(n.type) && count) text = `${count.default} shown; add up to ${count.max} with + Add pin.`;
  else if (["flow.switch_num", "flow.switch_text"].includes(n.type) && cases) text = `${String(cases.default).split(",").filter((x) => x.trim()).length} shown; add up to 16 with + Add pin.`;
  return text ? `<p class="ty">${esc(text)}</p>` : "";
}

/** The node drawn like the Studio's: its title bar, inputs on the left, outputs on the right. */
function nodeGfx(data, n) {
  const pin = (p, side) => {
    const label = esc(p.label || (p.type === "exec" ? "" : p.id));
    const dot = p.type === "exec" ? `<span class="pin exec"></span>` : `<span class="pin" style="--c:${pinColor(data.pinTypes, p.type)}"></span>`;
    return `<li>${side === "in" ? `${dot}${label}` : `${label}${dot}`}</li>`;
  };
  return `<div class="gfx" aria-hidden="true"><div class="gfx-title" style="--hc:${CAT_COLORS[n.category] ?? (n.event ? "#8b2e2e" : "#3f4f63")}">${esc(n.title)}</div><div class="gfx-pins"><ul class="in">${n.inputs.map((p) => pin(p, "in")).join("")}</ul><ul class="out">${n.outputs.map((p) => pin(p, "out")).join("")}</ul></div></div>`;
}

/** One node's card: the graphic, what it does, then details on demand. */
function nodeCard(data, n, byType) {
  const k = kindOf(n);
  const pinDl = (list, dir) => list.map((p) => `<dt>${esc(p.label || (p.type === "exec" ? (dir === "in" ? "Run" : "Then") : p.id))} <span class="ty">${esc(typeLabel(data.pinTypes, p.type))}</span></dt><dd>${esc(p.help || (p.type === "exec" ? (dir === "in" ? "Runs it." : "Goes on when it's done.") : ""))}</dd>`).join("");
  const setting = (s) => {
    const range = [s.min != null ? `from ${s.min}` : "", s.max != null ? `up to ${s.max}` : ""].filter(Boolean).join(" ");
    // (the help often lists the choices or the range already: not twice)
    const said = norm(s.help);
    const choices = Array.isArray(s.choices) && !/pick one|choices|one of/.test(said) ? ` Choices: ${s.choices.map((c) => (Array.isArray(c) ? c[1] ?? c[0] : typeof c === "object" ? c.label ?? c.value : c)).join(", ")}.` : "";
    const def = s.default === "" || s.default == null ? "" : ` Default: ${typeof s.default === "object" ? JSON.stringify(s.default) : s.default}.`;
    const showRange = range && !(s.min != null && said.includes(norm(String(s.min))) && s.max != null && said.includes(norm(String(s.max))));
    return `<dt>${esc(s.label)}</dt><dd>${esc(s.help)}${esc(def)}${showRange ? esc(` ${range[0].toUpperCase()}${range.slice(1)}.`) : ""}${esc(choices)}</dd>`;
  };
  // (the longer text only when it says more than the summary)
  const more = n.more && !saysSame(n.what, n.more) ? n.more : "";
  const bots = botLines(n).filter((x) => !norm(n.what).includes(norm(x)));
  const notes = [more, n.needs, n.risk ? `Careful: ${n.risk}` : "", n.cost ? `Cost: ${n.cost}` : ""].filter(Boolean);
  const sec = (label, count, body) => (count ? `<details><summary>${label} <span class="n">(${count})</span></summary>${body}</details>` : "");
  return `<article class="card" id="${nodeId(n.type)}" aria-labelledby="${nodeId(n.type)}-h">
<header><h3 id="${nodeId(n.type)}-h"><a href="#${nodeId(n.type)}">${esc(n.title)}</a></h3><span class="badge k-${k}">${k}</span><span class="id">${esc(n.type)}</span><button class="copy-link" type="button" data-id="${nodeId(n.type)}" aria-label="Copy a link to ${esc(n.title)}">Copy link</button></header>
${nodeGfx(data, n)}
<p class="what">${esc(n.what)}</p>${addPinNote(n)}
${sec("Inputs", n.inputs.length, `<dl>${pinDl(n.inputs, "in")}</dl>`)}${sec("Outputs", n.outputs.length, `<dl>${pinDl(n.outputs, "out")}</dl>`)}${sec("Settings", n.settings.length, `<dl>${n.settings.map(setting).join("")}</dl>`)}${sec("Limits", n.limits.length, `<dl>${n.limits.map((l) => `<dt>${esc(l.label)}</dt><dd>${esc(`${l.default} by default; hosts can raise it to ${l.max}. Above ${l.lagAbove} the server may slow down.`)}</dd>`).join("")}</dl>`)}${sec("Bots", bots.length, `<p>${esc(bots.join(" "))}</p>`)}${sec("Notes", notes.length, notes.map((t) => `<p>${esc(t)}</p>`).join(""))}
${n.related.filter((r) => byType[r]).length ? `<div class="chips"><span class="lbl">Related:</span>${n.related.filter((r) => byType[r]).map((r) => `<a class="chip" href="${byType[r].category === n.category ? "" : `${slug(byType[r].category)}.html`}#${nodeId(r)}">${esc(byType[r].title)}</a>`).join("")}</div>` : ""}
</article>`;
}

/** The node pages: an index (a card per category), and per category an overview grid, then the cards. */
function nodePages(data, enums, lib) {
  const cats = catsOf(data);
  const byType = Object.fromEntries(data.nodes.map((n) => [n.type, n]));
  const pages = {};
  for (const c of cats) {
    const list = data.nodes.filter((n) => n.category === c);
    const grid = `<div class="grid" aria-label="${esc(c)} nodes">${list.map((n) => `<a class="mini" href="#${nodeId(n.type)}"><span class="badge k-${kindOf(n)}">${kindOf(n)}</span><b>${esc(n.title)}</b><span class="s">${esc(firstSentence(n.what))}</span></a>`).join("")}</div>`;
    pages[`${slug(c)}.html`] = page({ section: "nodes",
      title: c,
      lead: `${list.length} Game Mode Studio node${list.length === 1 ? "" : "s"}. Pick one for its pins, settings and limits.`,
      side: nodeSide(data, c, "../../"),
      main: `${grid}\n${list.map((n) => nodeCard(data, n, byType)).join("\n")}`,
      toc: list.map((n) => `<li><a href="#${nodeId(n.type)}">${esc(n.title)}</a></li>`).join(""),
      file: "nodes.json, nodehelp.json, limits.ts",
      up: "../../",
    });
  }
  const catGrid = `<div class="grid">${cats.map((c) => {
    const list = data.nodes.filter((n) => n.category === c);
    return `<a class="mini" href="nodes/${slug(c)}.html"><span class="badge">${list.length}</span><b>${esc(c)}</b><span class="s">${esc(list.slice(0, 4).map((n) => n.title).join(", "))}${list.length > 4 ? "…" : ""}</span></a>`;
  }).join("")}</div>`;
  const enumsHtml = `<h2 id="enums">Built-in lists</h2><p>Lists every mode has (What Kind Of and Component Kind Of give them; Switch on Enum fills its cases from them).</p>${enums.map((e) => `<article class="card" id="enum-${slug(e.id)}"><header><h3>${esc(e.name)}</h3></header><p class="what">${esc(e.values.join(", "))}</p></article>`).join("")}`;
  const libHtml = `<h2 id="library">Starter library</h2><p>Ready-made pieces to add from Game Mode Studio's My library.</p>${lib.map((l) => `<article class="card" id="lib-${slug(l.name)}"><header><h3>${esc(l.name)}</h3><span class="badge">${esc(l.kind || "item")}</span><span class="id">${esc(l.category)}</span></header><p class="what">${esc(l.description)}</p>${l.inputs.length ? `<details><summary>Inputs and outputs</summary><p>In: ${esc(l.inputs.map((p) => p.name).join(", "))}. Out: ${esc(l.outputs.map((p) => p.name).join(", "))}.</p></details>` : ""}${l.tables.length ? `<p class="ty">Brings the table${l.tables.length > 1 ? "s" : ""} ${esc(l.tables.join(", "))}.</p>` : ""}</article>`).join("")}`;
  const index = page({ section: "nodes",
    title: "Game Mode Studio nodes",
    lead: `All ${data.nodes.length} nodes, by category. Search above, or open a category.`,
    side: nodeSide(data, null, "../"),
    main: `${catGrid}${enumsHtml}${libHtml}`,
    toc: `<li><a href="#enums">Built-in lists</a></li><li><a href="#library">Starter library</a></li>`,
    file: "nodes.json, nodehelp.json, limits.ts",
  });
  return { index, pages };
}

/** The entity pages: an index (a card per group), and per group an overview grid, then the cards. */
function entityPages(ents) {
  const groups = [...new Set(ents.map(entGroup))];
  const side = (here, up) => groups.map((g) => {
    const list = ents.filter((e) => entGroup(e) === g);
    const open = g === here;
    return `<details${open ? " open" : ""} class="${open ? "here" : ""}"><summary><a href="${up}reference/entities/${slug(g)}.html"${open ? ' aria-current="page"' : ""}>${esc(g)}</a><span class="n">${list.length}</span></summary>${open ? `<ul>${list.map((e) => `<li><a href="#${entId(e.name)}">${esc(e.name)}</a></li>`).join("")}</ul>` : ""}</details>`;
  }).join("");
  const kind = (e) => (e.source.startsWith("Loner") ? "component" : e.kind === "SolidClass" ? "brush" : "point");
  const card = (e) => {
    const keys = e.keys.filter((k) => k.type !== "flags");
    const flags = e.keys.find((k) => k.type === "flags");
    const blurb = e.blurb && !saysSame(e.help, e.blurb) ? e.blurb : "";
    return `<article class="card" id="${entId(e.name)}"><header><h3><a href="#${entId(e.name)}">${esc(e.name)}</a></h3><span class="badge k-${kind(e)}">${kind(e)}</span>${e.title && e.title !== e.name ? `<span class="id">${esc(e.title)}</span>` : ""}<button class="copy-link" type="button" data-id="${entId(e.name)}" aria-label="Copy a link to ${esc(e.name)}">Copy link</button></header>
<p class="what">${esc(e.help)}</p>${blurb ? `<p>${esc(blurb)}</p>` : ""}<p class="ty">${e.kind === "SolidClass" ? "A brush entity: tie brushes to it." : "A point entity."}${e.source.startsWith("Loner") ? " Needs Loner-Strike's components plugin, which comes with the app." : ""}</p>
${keys.length ? `<details><summary>Keys <span class="n">(${keys.length})</span></summary><dl>${keys.map((k) => `<dt>${esc(k.label || k.name)} <span class="ty">${esc(k.name)}</span></dt><dd>${esc(k.help)}${k.default ? esc(` Default: ${k.default}.`) : ""}${k.options.length ? esc(` Choices: ${k.options.map((o) => `${o.value} ${o.label}`).join(", ")}.`) : ""}</dd>`).join("")}</dl></details>` : ""}${flags?.options.length ? `<details><summary>Flags <span class="n">(${flags.options.length})</span></summary><dl>${flags.options.map((o) => `<dt>${esc(o.value)}</dt><dd>${esc(o.label)}${o.on ? " (on by default)" : ""}</dd>`).join("")}</dl></details>` : ""}
</article>`;
  };
  const pages = {};
  for (const g of groups) {
    const list = ents.filter((e) => entGroup(e) === g);
    pages[`${slug(g)}.html`] = page({ section: "entities",
      title: g,
      lead: `${list.length} entit${list.length === 1 ? "y" : "ies"} the map editor places. Pick one for its keys and flags.`,
      side: side(g, "../../"),
      main: `<div class="grid">${list.map((e) => `<a class="mini" href="#${entId(e.name)}"><span class="badge k-${kind(e)}">${kind(e)}</span><b>${esc(e.name)}</b><span class="s">${esc(firstSentence(e.help))}</span></a>`).join("")}</div>\n${list.map(card).join("\n")}`,
      toc: list.map((e) => `<li><a href="#${entId(e.name)}">${esc(e.name)}</a></li>`).join(""),
      file: "cstrike.fgd, lonerstrike.fgd, components.json",
      up: "../../",
    });
  }
  const index = page({ section: "entities",
    title: "Map editor entities",
    lead: `All ${ents.length} entities the map editor places, by group. Search above, or open a group.`,
    side: side(null, "../"),
    main: `<div class="grid">${groups.map((g) => {
      const list = ents.filter((e) => entGroup(e) === g);
      return `<a class="mini" href="entities/${slug(g)}.html"><span class="badge">${list.length}</span><b>${esc(g)}</b><span class="s">${esc(list.slice(0, 4).map((e) => e.name).join(", "))}${list.length > 4 ? "…" : ""}</span></a>`;
    }).join("")}</div>`,
    file: "cstrike.fgd, lonerstrike.fgd, components.json",
  });
  return { index, pages };
}

/** The editor's tools and keys: a card per menu, panel or dialog group. */
function editorPage(inv) {
  const side = inv.map((s) => `<details><summary><a href="#t-${slug(s.name)}">${esc(s.name)}</a><span class="n">${s.rows.length}</span></summary></details>`).join("");
  const main = inv.map((s) => `<article class="card" id="t-${slug(s.name)}"><header><h3><a href="#t-${slug(s.name)}">${esc(s.name)}</a></h3><span class="badge">${s.rows.length}</span><button class="copy-link" type="button" data-id="t-${slug(s.name)}" aria-label="Copy a link to ${esc(s.name)}">Copy link</button></header><dl>${s.rows.map((r) => `<dt>${esc(r.item)}</dt><dd>${esc(r.where)}</dd>`).join("")}</dl></article>`).join("\n");
  return page({ section: "editor", title: "Map editor tools and keys", lead: "Everything in the map editor's menus, panels and dialogs, where to find it, and its keys.", side, main, toc: inv.map((s) => `<li><a href="#t-${slug(s.name)}">${esc(s.name)}</a></li>`).join(""), file: "the editor's coverage inventory", kinds: false });
}

/** The reference's front page: the three references and the guides. */
function landingPage(data, ents, inv) {
  const card = (href, title, n, about) => `<a class="mini" href="${href}"><span class="badge">${n}</span><b>${esc(title)}</b><span class="s">${esc(about)}</span></a>`;
  const main = `<div class="grid">${card("nodes.html", "Game Mode Studio nodes", data.nodes.length, "Every node: its pins, settings, limits and what bots do with it.")}${card("entities.html", "Map editor entities", ents.length, "Every entity: what it does in the game, its keys and flags.")}${card("editor.html", "Map editor tools and keys", inv.length, "The editor's menus, panels and dialogs, and their keys.")}</div>
<h2 id="guides">Guides</h2><div class="grid">${GUIDES.map(([id, title, about]) => card(`../docs.html#${id}`, title, "soon", about)).join("")}</div>`;
  return page({ title: "Reference", lead: "Made from the app's own catalogs, so it always matches the app. Search everything from the box above.", side: `<details open class="here"><summary><a href="./" aria-current="page">Reference</a></summary><ul><li><a href="nodes.html">Studio nodes</a></li><li><a href="entities.html">Map entities</a></li><li><a href="editor.html">Editor tools and keys</a></li></ul></details>`, main, file: "nodes.json, the FGDs, the editor's inventory" });
}

/** The search entries (the reference pages' search, and the docs search): one per node, entity
 *  and editor section. u url from the site root, n name, t where, k keywords, x one-line summary,
 *  cat category, c kind, b works with bots, l has limits, ty pin types. */
function searchIndex(data, ents, inv) {
  return [
    ...data.nodes.map((n) => ({ u: `reference/${nodeHref(n)}`, n: n.title, t: `Studio node · ${n.category}`, k: `${n.type} ${[...n.inputs, ...n.outputs].map((p) => p.label || "").join(" ")}`.trim(), x: firstSentence(n.what), cat: n.category, c: kindOf(n), b: hasBots(n) ? 1 : 0, l: n.limits.length ? 1 : 0, ty: [...new Set([...n.inputs, ...n.outputs].filter((p) => p.type !== "exec").map((p) => typeLabel(data.pinTypes, p.type)))] })),
    ...ents.map((e) => ({ u: `reference/${entHref(e)}`, n: e.name, t: `Entity · ${entGroup(e)}`, k: `${e.title} ${e.keys.map((k) => k.name).join(" ")}`, x: firstSentence(e.help), cat: entGroup(e), c: e.kind === "SolidClass" ? "brush" : "point", b: /\bbots?\b/i.test(e.help) ? 1 : 0, l: 0, ty: [] })),
    ...inv.map((s) => ({ u: `reference/editor.html#t-${slug(s.name)}`, n: `Editor: ${s.name}`, t: "Map editor tools and keys", k: s.rows.map((r) => r.where).join(" ").slice(0, 300), x: s.rows.map((r) => r.item).slice(0, 8).join(", "), cat: "Map editor", c: "", b: 0, l: 0, ty: [] })),
  ];
}

/** The hand-written guides to come (stubs in docs.html's Reference group). */
const GUIDES = [
  ["guide-types", "Types and casting", "Numbers, text, players, entities, vectors and lists, and turning one into another."],
  ["guide-events-flow", "Events and flow", "What starts a graph running, and Branch, Sequence, loops and timers."],
  ["guide-variables", "Variables and saving", "Mode, round and player variables, and what's kept between maps."],
  ["guide-spawning", "Spawning and transforms", "Making props, creatures and effects, and moving, turning and attaching them."],
  ["guide-limits", "Limits", "The comfort limits on effects and searches, and when the game itself runs out."],
  ["guide-bots", "Steering bots", "Goals, offers, friends and avoid areas: telling bots what matters in your mode."],
  ["guide-sharing", "Sharing", "What travels with a mode, what players need, and how credits work."],
];

function docsBlock(data, ents, inv) {
  const cats = catsOf(data);
  const ref = `<section class="topic" id="reference" data-group="Reference" data-keywords="reference nodes entities keys shortcuts catalog">
  <p class="crumb">Reference</p>
  <h1>Reference</h1>
  <p class="lead">Made from the app's own catalogs, so it always matches the app. The docs search finds every entry.</p>
  <ul>
    <li><a href="reference/nodes.html"><b>Game Mode Studio nodes</b></a>: all ${data.nodes.length}, by category (${cats.length}), each with its pins, settings and limits.</li>
    <li><a href="reference/entities.html"><b>Map editor entities</b></a>: all ${ents.length}, with their keys and flags.</li>
    <li><a href="reference/editor.html"><b>Map editor tools and keys</b></a>: the menus, panels and dialogs, and their keys.</li>
  </ul>
  <p><a href="reference/">Open the reference</a>: it has its own search with filters.</p>
</section>`;
  // (stubs for the hand-written guides: an agent writes them once the type system lands)
  const stubs = GUIDES.map(([id, title, about]) => `<section class="topic" id="${id}" data-group="Reference" data-stub="1">
  <p class="crumb">Reference</p>
  <h1>${esc(title)}</h1>
  <p class="lead">${esc(about)}</p>
  <p>This guide is being written. Until then, the <a href="reference/nodes.html">node reference</a> has every node's own help.</p>
</section>`).join("\n\n");
  return `<!-- REFERENCE:BEGIN (made by scripts/gen-reference.mjs; edits here are replaced) -->\n${ref}\n\n${stubs}\n<!-- REFERENCE:END -->`;
}

function llmsBlock(data, ents) {
  const lines = ["<!-- REFERENCE:BEGIN (made by scripts/gen-reference.mjs) -->", "## Game Mode Studio nodes (compact)", "", "One line each: Title (type, category): what it does. In: inputs. Out: outputs.", ""];
  for (const n of data.nodes) {
    const ins = n.inputs.filter((p) => p.type !== "exec").map((p) => `${p.label || p.id} ${typeLabel(data.pinTypes, p.type)}`);
    const outs = n.outputs.filter((p) => p.type !== "exec").map((p) => `${p.label || p.id} ${typeLabel(data.pinTypes, p.type)}`);
    lines.push(`- ${n.title} (${n.type}, ${n.category}): ${n.what}${ins.length ? ` In: ${ins.join(", ")}.` : ""}${outs.length ? ` Out: ${outs.join(", ")}.` : ""}`);
  }
  lines.push("", "## Map editor entities (compact)", "");
  for (const e of ents) lines.push(`- ${e.name}${e.title && e.title !== e.name ? ` (${e.title})` : ""}: ${e.help} Keys: ${e.keys.map((k) => k.name).join(", ") || "none"}.`);
  lines.push("<!-- REFERENCE:END -->");
  return lines.join("\n");
}

function replaceBlock(text, block, before) {
  const a = text.indexOf("<!-- REFERENCE:BEGIN");
  const b = text.indexOf("<!-- REFERENCE:END -->");
  if (a >= 0 && b > a) return text.slice(0, a) + block + text.slice(b + "<!-- REFERENCE:END -->".length);
  if (!before) return `${text.replace(/\s*$/, "")}\n\n${block}\n`;
  const at = text.indexOf(before);
  if (at < 0) throw new Error(`no place for the reference block (${before})`);
  return `${text.slice(0, at)}${block}\n\n${text.slice(at)}`;
}

// ---- main ----------------------------------------------------------------------------------

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const missing = missingHelp();
  if (process.argv.includes("--check")) {
    if (missing.length) {
      console.error(`${missing.length} without help:\n${missing.slice(0, 50).join("\n")}`);
      process.exit(1);
    }
    console.log("Every node and entity has help.");
    process.exit(0);
  }
  const data = nodes();
  const ents = entities();
  const inv = editorInventory();
  const out = join(site, "reference");
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "base.css"), baseCss());
  const np = nodePages(data, builtinEnums(), library());
  writeFileSync(join(out, "nodes.html"), np.index);
  const ep = entityPages(ents);
  writeFileSync(join(out, "entities.html"), ep.index);
  writeFileSync(join(out, "editor.html"), editorPage(inv));
  writeFileSync(join(out, "index.html"), landingPage(data, ents, inv));
  for (const [dir, pages] of [["nodes", np.pages], ["entities", ep.pages]]) {
    mkdirSync(join(out, dir), { recursive: true });
    for (const f of readdirSync(join(out, dir))) if (f.endsWith(".html") && !pages[f]) unlinkSync(join(out, dir, f));
    for (const [f, html] of Object.entries(pages)) writeFileSync(join(out, dir, f), html);
  }
  writeFileSync(join(out, "index.js"), `// Made by scripts/gen-reference.mjs: the docs search's reference entries (u url, n name, t where, k keywords, x text)\nwindow.LS_REFERENCE = ${JSON.stringify(searchIndex(data, ents, inv))};\n`);
  const docs = join(site, "docs.html");
  writeFileSync(docs, replaceBlock(read(docs), docsBlock(data, ents, inv), '<section class="topic" id="something-wrong"'));
  const llms = join(site, "llms-full.txt");
  if (existsSync(llms)) writeFileSync(llms, replaceBlock(read(llms), llmsBlock(data, ents), null));
  console.log(`Wrote ${data.nodes.length} nodes, ${ents.length} entities, ${inv.length} editor sections${missing.length ? `; ${missing.length} without help (see --check)` : ""}. Keys without a description: ${keysWithoutHelp().length}.`);
}
