#!/usr/bin/env node
// Generated reference docs (Parity A): the Game Mode Studio node reference, the map editor's
// entity reference and its tools and keys, written as static pages in site/reference/, plus a
// search index the docs page loads (site/reference/index.js), the Reference group in docs.html
// (between REFERENCE markers) and a compact form in llms-full.txt (between its markers).
//
//   node scripts/gen-reference.mjs           write everything
//   node scripts/gen-reference.mjs --check   only check: exit 1 if a node or entity has no help, or a
//                                            pin type has no look (color, mark) or no Types entry
//
// Sources: app/ui/src/views/tools/studio/nodes.json and nodehelp.json (the same help the Studio's
// node help panel and the AI connection's studio_node_help read), limits.ts, schema.ts's built-in
// enums and structs, graph.rs's upcast() (what a type goes into), the starter library items
// (library/*.lib.json); app/src-tauri/src/editor/cstrike.fgd,
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

/** schema.ts's BUILTIN_STRUCTS (the app's own structs: Transform). */
function builtinStructs() {
  const text = read(join(studio, "schema.ts"));
  const block = text.slice(text.indexOf("export const BUILTIN_STRUCTS"), text.indexOf("];\n", text.indexOf("export const BUILTIN_STRUCTS")));
  return [...block.matchAll(/id:\s*"([^"]+)",\s*name:\s*"([^"]+)",\s*fields:\s*\[([\s\S]*?)\]\s*\}/g)].map((m) => ({ id: m[1], name: m[2], fields: [...m[3].matchAll(/name:\s*"([^"]+)",\s*type:\s*"([^"]+)"/g)].map((f) => ({ name: f[1], type: f[2] })) }));
}

/** graph.rs upcast(): what a value of a type goes into as it is ("player" -> "entity"). */
function upcasts() {
  const text = read(join(root, "app/src-tauri/src/tools/studio/graph.rs"));
  const body = text.slice(text.indexOf("pub fn upcast"), text.indexOf("pub const KEY_TYPES"));
  const out = {};
  for (const m of body.matchAll(/^\s*((?:"[^"]+"\s*\|?\s*)+)=>\s*Some\("([^"]+)"\)/gm)) for (const t of m[1].matchAll(/"([^"]+)"/g)) out[t[1]] = m[2];
  return out;
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
    enums: builtinEnums(),
    structs: builtinStructs(),
    upcast: upcasts(),
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
          inputs: n.inputs.map((p) => ({ id: p.id, label: p.label || "", type: p.type, whole: !!p.whole, help: pinHelp[`in:${p.id}`] || "" })),
          outputs: n.outputs.map((p) => ({ id: p.id, label: p.label || "", type: p.type, whole: !!p.whole, help: pinHelp[`out:${p.id}`] || "" })),
          settings: (n.params ?? []).map((p) => ({ id: p.id, label: p.label, kind: p.kind, whole: !!p.whole, default: p.default, min: p.min, max: p.max, choices: p.choices, help: setHelp[p.id] || "" })),
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
/** `array<T>` -> { kind: "array", item }; `map<K,T>` -> { kind: "map", key, item } (registry.ts container). */
function container(t) {
  let m = /^array<(.+)>$/.exec(t);
  if (m) return { kind: "array", item: m[1] };
  m = /^map<([^,]+),(.+)>$/.exec(t);
  return m ? { kind: "map", key: m[1], item: m[2] } : null;
}
/** The generic letters: the node's own type, set by a setting or by the first wire. */
const GENERIC = { $T: "The node's type", $K: "The node's key type", $E: "The picked enum", $S: "The picked struct", $F: "The picked field's type" };
/** A type's name for people, as the Studio says it (pins.ts typeLabel): Number, Array of Players, Hit Zone. */
function typeLabel(data, t) {
  const c = container(t);
  const inner = (x) => (GENERIC[x] ? GENERIC[x].replace(/^The/, "the") : typeLabel(data, x));
  if (c) return c.kind === "array" ? `Array of ${inner(c.item)}` : `Map of ${inner(c.key)} to ${inner(c.item)}`;
  if (GENERIC[t]) return GENERIC[t];
  if (t.startsWith("enum:")) return data.enums.find((e) => `enum:${e.id}` === t)?.name ?? "Enum";
  if (t.startsWith("struct:")) return data.structs.find((x) => `struct:${x.id}` === t)?.name ?? "Struct";
  return data.pinTypes[t]?.label ?? t;
}
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
function page({ title, lead, side, main, toc = "", file, up = "../", kinds = true, section = "", head = "", meta = "" }) {
  // (the three references and the guides are always one click away)
  const sw = [["nodes", "reference/nodes.html", "Nodes", "Studio nodes"], ["entities", "reference/entities.html", "Entities", "Map entities"], ["editor", "reference/editor.html", "Editor", "Editor tools"], ["guides", "reference/#guides", "Guides", "Guides"]]
    .map(([id, href, label, full]) => `<a href="${up}${href}" title="${full}"${id === section ? ' aria-current="true"' : ""}>${label}</a>`).join("");
  // (phones: only the current category stays in the menu, the rest behind one button)
  if ((side.match(/<details/g) || []).length > 8 && side.includes('class="here"')) side += `<button class="ref-all" type="button" aria-expanded="false">All ${section === "entities" ? "groups" : "categories"}</button>`;
  side = `<div class="ref-switch" role="navigation" aria-label="Reference sections">${sw}</div>${side}`;
  const filters = kinds
    ? `<label>Category <select id="ref-f-cat"><option value="">Any</option></select></label>
      <label>Kind <select id="ref-f-kind"><option value="">Any</option><option value="event">Event</option><option value="action">Action</option><option value="value">Value</option><option value="flow">Flow</option><option value="type">Pin type</option><option value="point">Point entity</option><option value="brush">Brush entity</option></select></label>
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
<link rel="stylesheet" href="${up}reference/ref.css">${head}
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
    <p class="meta ty">${meta || `Made from the app's own catalog (${esc(file)}), so it matches the app.`}</p>
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
  const t = here === "__types";
  return `<details${t ? " open" : ""} class="${t ? "here" : ""}"><summary><a href="${up}reference/nodes/types.html"${t ? ' aria-current="page"' : ""}>Pin types</a><span class="n">${typeEntries(data).length}</span></summary>${t ? `<ul>${TYPE_GROUPS.map((g) => `<li><a href="#g-${slug(g)}">${esc(g)}</a></li>`).join("")}</ul>` : ""}</details>` + catsOf(data)
    .map((c) => {
      const list = data.nodes.filter((n) => n.category === c);
      const open = c === here;
      return `<details${open ? " open" : ""} class="${open ? "here" : ""}"><summary><a href="${up}reference/nodes/${slug(c)}.html"${open ? ' aria-current="page"' : ""}>${esc(c)}</a><span class="n">${list.length}</span></summary>${open ? `<ul>${list.map((n) => `<li><a href="#${nodeId(n.type)}">${esc(n.title)}</a></li>`).join("")}</ul>` : ""}</details>`;
    })
    .join("");
}

/** A pin type's color (registry.ts typeColor): containers take their item's, enums and structs their own. */
function pinColor(types, t) {
  const c = container(t);
  if (c) return pinColor(types, c.item);
  if (t.startsWith("enum:")) return types.enum.color;
  if (t.startsWith("struct:")) return types.struct.color;
  return (types[t] ?? types.any).color;
}
/** The pin's dot: its color, and the shape of an array, a map, a file or a color. */
function pinDot(data, t) {
  if (t === "exec") return `<span class="pin exec"></span>`;
  const c = container(t), mark = data.pinTypes[c ? c.item : t]?.mark;
  const cls = c ? ` ${c.kind}` : mark === "file" ? " file" : mark === "rainbow" ? " rainbow" : "";
  return `<span class="pin${cls}" style="--c:${pinColor(data.pinTypes, t)}"></span>`;
}
/** The marks beside a pin's name: # for a whole number, the handle's letter. */
function pinMarks(data, p) {
  const c = container(p.type), meta = data.pinTypes[c ? c.item : p.type];
  return `${p.whole ? `<span class="pwhole" title="A whole number">#</span>` : ""}${meta?.badge ? `<span class="pbadge" title="${esc(meta.label)}">${esc(meta.badge)}</span>` : ""}`;
}
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
    const label = esc(p.label || (p.type === "exec" ? "" : p.id)) + pinMarks(data, p);
    const dot = pinDot(data, p.type);
    return `<li>${side === "in" ? `${dot}${label}` : `${label}${dot}`}</li>`;
  };
  return `<div class="gfx" aria-hidden="true"><div class="gfx-title" style="--hc:${CAT_COLORS[n.category] ?? (n.event ? "#8b2e2e" : "#3f4f63")}">${esc(n.title)}</div><div class="gfx-pins"><ul class="in">${n.inputs.map((p) => pin(p, "in")).join("")}</ul><ul class="out">${n.outputs.map((p) => pin(p, "out")).join("")}</ul></div></div>`;
}

/** One node's card: the graphic, what it does, then details on demand. */
function nodeCard(data, n, byType) {
  const k = kindOf(n);
  const pinDl = (list, dir) => list.map((p) => `<dt>${esc(p.label || (p.type === "exec" ? (dir === "in" ? "Run" : "Then") : p.id))}${pinMarks(data, p)} <a class="ty" href="types.html#${typeAnchor(data, p.type)}">${esc(typeLabel(data, p.type))}</a></dt><dd>${esc(p.help || (p.type === "exec" ? (dir === "in" ? "Runs it." : "Goes on when it's done.") : ""))}${esc(pinExtra(data, p))}</dd>`).join("");
  const setting = (s) => {
    const range = [s.min != null ? `from ${s.min}` : "", s.max != null ? `up to ${s.max}` : ""].filter(Boolean).join(" ");
    // (the help often lists the choices or the range already: not twice)
    const said = norm(s.help);
    const choices = Array.isArray(s.choices) && !/pick one|choices|one of/.test(said) ? ` Choices: ${s.choices.map((c) => (Array.isArray(c) ? c[1] ?? c[0] : typeof c === "object" ? c.label ?? c.value : c)).join(", ")}.` : "";
    const def = s.default === "" || s.default == null ? "" : ` Default: ${typeof s.default === "object" ? JSON.stringify(s.default) : s.default}.`;
    const showRange = range && !(s.min != null && said.includes(norm(String(s.min))) && s.max != null && said.includes(norm(String(s.max))));
    return `<dt>${esc(s.label)}${s.whole ? `<span class="pwhole" title="A whole number">#</span>` : ""}</dt><dd>${esc(s.help)}${esc(def)}${showRange ? esc(` ${range[0].toUpperCase()}${range.slice(1)}.`) : ""}${esc(choices)}</dd>`;
  };
  // (the longer text only when it says more than the summary)
  const more = n.more && !saysSame(n.what, n.more) ? n.more : "";
  const bots = botLines(n).filter((x) => !norm(n.what).includes(norm(x)));
  const notes = [more, n.needs, n.risk ? `Careful: ${n.risk}` : "", n.cost ? `Cost: ${n.cost}` : ""].filter(Boolean);
  const sec = (label, count, body) => (count ? `<details><summary>${label} <span class="n">(${count})</span></summary>${body}</details>` : "");
  return `<article class="card" id="${nodeId(n.type)}" aria-labelledby="${nodeId(n.type)}-h">
<header><h3 id="${nodeId(n.type)}-h"><a href="#${nodeId(n.type)}">${esc(n.title)}</a></h3><span class="badge k-${k}">${k}</span><span class="id">${esc(n.type)}</span><button class="copy-link" type="button" data-id="${nodeId(n.type)}" aria-label="Copy a link to ${esc(n.title)}">Copy link</button></header>
${nodeGfx(data, n)}
<p class="what">${esc(n.what)}</p>${addPinNote(n)}${switchEnumNote(data, n)}
${sec("Inputs", n.inputs.length, `<dl>${pinDl(n.inputs, "in")}</dl>`)}${sec("Outputs", n.outputs.length, `<dl>${pinDl(n.outputs, "out")}</dl>`)}${sec("Settings", n.settings.length, `<dl>${n.settings.map(setting).join("")}</dl>`)}${sec("Limits", n.limits.length, `<dl>${n.limits.map((l) => `<dt>${esc(l.label)}</dt><dd>${esc(`${l.default} by default; hosts can raise it to ${l.max}. Above ${l.lagAbove} the server may slow down.`)}</dd>`).join("")}</dl>`)}${sec("Bots", bots.length, `<p>${esc(bots.join(" "))}</p>`)}${sec("Notes", notes.length, notes.map((t) => `<p>${esc(t)}</p>`).join(""))}
${n.related.filter((r) => byType[r]).length ? `<div class="chips"><span class="lbl">Related:</span>${n.related.filter((r) => byType[r]).map((r) => `<a class="chip" href="${byType[r].category === n.category ? "" : `${slug(byType[r].category)}.html`}#${nodeId(r)}">${esc(byType[r].title)}</a>`).join("")}</div>` : ""}
</article>`;
}

// ---- types ---------------------------------------------------------------------------------

/** One short entry per type: what it is. What it plugs into, the casts, the values and the nodes
 *  come from the catalog (graph.rs upcast, the As … nodes, schema.ts's built-in enums). A pin type
 *  in nodes.json without an entry here fails --check. [group, what] */
const TYPE_DOCS = {
  exec: ["Wiring", "The white arrows: the order things run in. It carries no value."],
  number: ["Basics", "A number, with or without a fraction. A pin marked # takes a whole number: a fraction is rounded (and Problems says so for one you typed in)."],
  bool: ["Basics", "Yes or no. Branch, the conditions and the gates use it."],
  text: ["Basics", "Words: names, messages, map names. It doesn't last through a Delay unless it's kept in a variable."],
  vector: ["Basics", "Three numbers, X, Y and Z: a place in the map, a direction or a speed. It doesn't last through a Delay unless it's kept in a variable."],
  team: ["Basics", "A team: Terrorists or Counter-Terrorists (no team for a draw)."],
  timer: ["Basics", "A timer a Delay, Repeat or Countdown started, to stop or check later."],
  color: ["Basics", "A color, picked with the color picker (red, green and blue)."],
  any: ["Basics", "A pin that takes a value of any type, like the value shown by Show HUD Text."],
  player: ["Players and entities", "A player in the game: a person or a bot."],
  entity: ["Players and entities", "Anything in the world: a player, a prop, a creature, an NPC, a component, a map entity. The handles below are entities known to be one kind."],
  npc: ["Players and entities", "An NPC from Spawn NPC: a person who talks, walks paths and follows."],
  creature: ["Players and entities", "A creature from Spawn Creature: it moves, attacks and can be hurt."],
  projectile: ["Players and entities", "A projectile from Spawn Projectile, flying until it hits or expires."],
  volume: ["Players and entities", "A trigger volume from Spawn Trigger Volume: an invisible box that notices who goes in and out."],
  objmarker: ["Players and entities", "An objective marker: a sprite floating over an entity or a spot, until Remove Marker."],
  component: ["Players and entities", "A component the mode made or the map placed: a capture point, a shop, a barricade, a door…"],
  soundloop: ["Players and entities", "A sound that keeps playing (Loop Sound) until Stop Sound gets it."],
  model: ["Files", "A model file (.mdl) the mode loads: props, creatures, attachments. Your own go with the mode."],
  sprite: ["Files", "A sprite file (.spr): effects, glows, markers. Your own go with the mode."],
  sound: ["Files", "A sound file (.wav) the mode plays. Your own go with the mode."],
  decal: ["Files", "A kind of decal from the game's decals.wad (a bullet hole, a scorch, blood)."],
  visual: ["Files", "A pin that takes a Model or a Sprite."],
  enum: ["Lists and enums", "A value from a fixed list of named choices: the app's own lists below, or your mode's own enums."],
  struct: ["Lists and enums", "A bundle of named values kept together: Transform, or your mode's own structs. It doesn't last through a Delay unless it's kept in a variable."],
  array: ["Lists and enums", "A list of values of one type, in order. An array doesn't go into an array of a wider type (an Array of Creatures isn't an Array of Entities): a value written through the wider type could be the wrong kind."],
  map: ["Lists and enums", "Values looked up by a key: text, a number or a player."],
  var: ["Wiring", "A variable node's pin: it takes the type of the variable picked."],
  generic: ["Wiring", "A pin whose type the node takes from a setting or from the first wire: an array node set to Players has Player pins."],
  "enum:ls_kind": ["Lists and enums", "What an entity is."],
  "enum:ls_comp_kind": ["Lists and enums", "Which kind of component it is."],
  "enum:ls_spawn_fail": ["Lists and enums", "Why a spawn failed: a spawn node's Failure reason."],
  "enum:ls_who": ["Lists and enums", "Which players a For Each Player or Pick Random Player goes through."],
  "enum:ls_weapon": ["Lists and enums", "A weapon, by its game name."],
  "enum:ls_hitzone": ["Lists and enums", "Where a hit landed on the body."],
  "enum:ls_dmgkind": ["Lists and enums", "What kind of damage it was."],
  "struct:ls_transform": ["Lists and enums", "Where something is and how it moves, in one value."],
};
const TYPE_GROUPS = ["Basics", "Players and entities", "Files", "Lists and enums", "Wiring"];
const TYPE_MARKS = ["file", "rainbow"];

/** The types-page entry a pin type is explained by. */
function typeKey(data, t) {
  const c = container(t);
  if (c) return c.kind;
  if (GENERIC[t]) return "generic";
  if (t.startsWith("enum:")) return TYPE_DOCS[t] ? t : "enum";
  if (t.startsWith("struct:")) return TYPE_DOCS[t] ? t : "struct";
  return t;
}
const typeAnchor = (data, t) => `t-${slug(typeKey(data, t))}`;

/** The entries, in page order, with what the catalog says about each. */
function typeEntries(data) {
  const keys = [...Object.keys(data.pinTypes), ...data.enums.map((e) => `enum:${e.id}`), ...data.structs.map((x) => `struct:${x.id}`), "array", "map", "generic"];
  const label = (k) => (k === "array" ? "Array" : k === "map" ? "Map" : k === "generic" ? "Generic pins" : typeLabel(data, k));
  return keys.map((k) => {
    const [group, what] = TYPE_DOCS[k] ?? ["Basics", ""];
    // (the nodes with a pin of it: in, out)
    const has = (dir) => data.nodes.filter((n) => n[dir].some((p) => p.type !== "exec" && typeKey(data, p.type) === k));
    // what it goes into as it is (graph.rs upcast, up the chain), and what goes into it
    const up = [];
    for (let t = data.upcast[k]; t; t = data.upcast[t]) up.push(t);
    const from = Object.keys(data.upcast).filter((t) => { for (let u = data.upcast[t]; u; u = data.upcast[u]) if (u === k) return true; return false; });
    const casts = data.nodes.filter((n) => n.type.startsWith("cast.") && n.outputs.some((p) => p.type === k));
    const castsFrom = data.nodes.filter((n) => n.type.startsWith("cast.") && n.inputs.some((p) => p.type === k));
    const en = k.startsWith("enum:") ? data.enums.find((e) => `enum:${e.id}` === k) : null;
    const st = k.startsWith("struct:") ? data.structs.find((x) => `struct:${x.id}` === k) : null;
    return { key: k, label: label(k), group, what, meta: data.pinTypes[k], ins: has("inputs"), outs: has("outputs"), up, from, casts, castsFrom, values: en?.values ?? null, fields: st?.fields ?? null };
  });
}

/** What a pin's details add: an enum's values, the generic letter's meaning. */
function pinExtra(data, p) {
  if (p.type.startsWith("enum:")) {
    const e = data.enums.find((x) => `enum:${x.id}` === p.type);
    if (e) return ` Values: ${e.values.join(", ")}.`;
  }
  return "";
}

/** Switch on Enum: one output per value of the enum picked. */
function switchEnumNote(data, n) {
  if (n.type !== "flow.switch_enum") return "";
  const ex = data.enums.find((e) => e.id === "ls_hitzone") ?? data.enums[0];
  return `<p class="note">It has one output per value of the enum picked in its Enum setting (and the enum's value goes into Selection). ${ex ? `A ${esc(ex.name)} gives ${ex.values.length} outputs: ${esc(ex.values.join(", "))}. ` : ""}The app's own enums are on the <a href="types.html#t-enum">types page</a>; your mode's own enums work the same way.</p>`;
}

/** The Types page: one card per type, grouped, with its pin as the Studio draws it. */
function typesPage(data) {
  const all = typeEntries(data);
  const byType = Object.fromEntries(data.nodes.map((n) => [n.type, n]));
  const chip = (n) => `<a class="chip" href="${slug(n.category)}.html#${nodeId(n.type)}">${esc(n.title)}</a>`;
  const tLink = (t) => `<a href="#${typeAnchor(data, t)}">${esc(typeLabel(data, t))}</a>`;
  const guide = guideBody("types") ? "../guides/types.html" : "../../docs.html#guide-types";
  const card = (e) => {
    const dot = e.key === "array" ? pinDot(data, "array<any>") : e.key === "map" ? pinDot(data, "map<text,any>") : e.key === "generic" || e.key === "var" ? pinDot(data, "any") : pinDot(data, e.key);
    const badge = e.meta?.badge ? `<span class="pbadge" title="${esc(e.label)}">${esc(e.meta.badge)}</span>` : "";
    const lines = [];
    if (e.up.length) lines.push(`<dt>Goes into</dt><dd>${e.up.map(tLink).join(", ")} pins as it is${e.key === "enum:ls_weapon" ? " (its name)" : ""}.</dd>`);
    if (e.from.length) lines.push(`<dt>Takes</dt><dd>${e.from.map(tLink).join(", ")} as they are.</dd>`);
    if (e.casts.length) lines.push(`<dt>Cast to it</dt><dd>${e.casts.map(chip).join(" ")} takes an Entity and goes on by Success (it is one) or Failed.</dd>`);
    if (e.castsFrom.length) lines.push(`<dt>Casts</dt><dd>${e.castsFrom.map(chip).join(" ")}</dd>`);
    if (e.values) lines.push(`<dt>Values</dt><dd>${esc(e.values.join(", "))}</dd>`);
    if (e.fields) lines.push(`<dt>Fields</dt><dd>${e.fields.map((f) => `${esc(f.name)} (${tLink(f.type)})`).join(", ")}</dd>`);
    const given = e.outs.length ? `<dt>Given by</dt><dd>${e.outs.slice(0, 8).map(chip).join(" ")}${e.outs.length > 8 ? ` and ${e.outs.length - 8} more` : ""}</dd>` : "";
    const taken = e.ins.length ? `<dt>Taken by</dt><dd>${e.ins.slice(0, 8).map(chip).join(" ")}${e.ins.length > 8 ? ` and ${e.ins.length - 8} more` : ""}</dd>` : "";
    return `<article class="card type-card" id="t-${slug(e.key)}"><header><h3><a href="#t-${slug(e.key)}">${dot}${esc(e.label)}</a>${badge}</h3><span class="id">${esc(e.key)}</span><button class="copy-link" type="button" data-id="t-${slug(e.key)}" aria-label="Copy a link to ${esc(e.label)}">Copy link</button></header><p class="what">${esc(e.what)}</p>${lines.length + (given ? 1 : 0) + (taken ? 1 : 0) ? `<dl>${lines.join("")}${given}${taken}</dl>` : ""}</article>`;
  };
  const main = `<p>Every pin has a type, shown by its color and shape: a square for a file, a grid for an array, a diamond for a map, a letter for an entity handle, <span class="pwhole">#</span> for a whole number. A wire joins pins of the same type, or a narrower type into a wider one (a Creature into an Entity pin); going the other way takes a cast (As Creature). The <a href="${guide}">Types and casting guide</a> walks through it.</p>
${TYPE_GROUPS.map((g) => `<h2 id="g-${slug(g)}">${esc(g)}</h2>\n${all.filter((e) => e.group === g).map(card).join("\n")}`).join("\n")}`;
  return page({ section: "nodes",
    title: "Pin types",
    lead: `The ${all.length} types a Game Mode Studio pin can have: what each is, what it plugs into and how to cast to it.`,
    side: nodeSide(data, "__types", "../../"),
    main,
    toc: TYPE_GROUPS.map((g) => `<li><a href="#g-${slug(g)}">${esc(g)}</a></li>`).join(""),
    file: "nodes.json, nodehelp.json, schema.ts's built-in enums, graph.rs's upcasts",
    up: "../../",
  });
}

/** Pin types without a look (a color, a known mark) or without an entry on the Types page. */
export function typeProblems() {
  const data = nodes();
  const out = [];
  for (const [k, v] of Object.entries(data.pinTypes)) {
    if (!/^#[0-9a-f]{6}$/i.test(v.color ?? "")) out.push(`pin type ${k}: no color`);
    if (v.mark && !TYPE_MARKS.includes(v.mark)) out.push(`pin type ${k}: mark "${v.mark}" has no style`);
    if (!TYPE_DOCS[k]) out.push(`pin type ${k}: no entry on the Types page (TYPE_DOCS)`);
  }
  for (const e of data.enums) if (!TYPE_DOCS[`enum:${e.id}`]) out.push(`enum ${e.id}: no entry on the Types page (TYPE_DOCS)`);
  for (const x of data.structs) if (!TYPE_DOCS[`struct:${x.id}`]) out.push(`struct ${x.id}: no entry on the Types page (TYPE_DOCS)`);
  for (const n of data.nodes) for (const p of [...n.inputs, ...n.outputs]) {
    const c = container(p.type);
    for (const t of c ? [c.key, c.item].filter(Boolean) : [p.type]) {
      const k = typeKey(data, t);
      if (!TYPE_DOCS[k]) out.push(`node ${n.type}: pin ${p.id} has type ${t}, which isn't on the Types page`);
      if (t.startsWith("enum:") && !data.enums.some((e) => `enum:${e.id}` === t)) out.push(`node ${n.type}: pin ${p.id}: enum ${t} isn't a built-in enum`);
    }
  }
  return [...new Set(out)];
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
  pages["types.html"] = typesPage(data);
  const catGrid = `<div class="grid">${cats.map((c) => {
    const list = data.nodes.filter((n) => n.category === c);
    return `<a class="mini" href="nodes/${slug(c)}.html"><span class="badge">${list.length}</span><b>${esc(c)}</b><span class="s">${esc(list.slice(0, 4).map((n) => n.title).join(", "))}${list.length > 4 ? "…" : ""}</span></a>`;
  }).join("")}</div>`;
  const enumsHtml = `<h2 id="types">Pin types</h2><p>Every pin's type, what it plugs into and how to cast to it, and the app's own enums (${esc(enums.map((e) => e.name).join(", "))}): <a href="nodes/types.html">Pin types</a>.</p>`;
  const libHtml = `<h2 id="library">Starter library</h2><p>Ready-made pieces to add from Game Mode Studio's My library.</p>${lib.map((l) => `<article class="card" id="lib-${slug(l.name)}"><header><h3>${esc(l.name)}</h3><span class="badge">${esc(l.kind || "item")}</span><span class="id">${esc(l.category)}</span></header><p class="what">${esc(l.description)}</p>${l.inputs.length ? `<details><summary>Inputs and outputs</summary><p>In: ${esc(l.inputs.map((p) => p.name).join(", "))}. Out: ${esc(l.outputs.map((p) => p.name).join(", "))}.</p></details>` : ""}${l.tables.length ? `<p class="ty">Brings the table${l.tables.length > 1 ? "s" : ""} ${esc(l.tables.join(", "))}.</p>` : ""}</article>`).join("")}`;
  const index = page({ section: "nodes",
    title: "Game Mode Studio nodes",
    lead: `All ${data.nodes.length} nodes, by category. Search above, or open a category.`,
    side: nodeSide(data, null, "../"),
    main: `${catGrid}${enumsHtml}${libHtml}`,
    toc: `<li><a href="#types">Pin types</a></li><li><a href="#library">Starter library</a></li>`,
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
  const main = `<div class="grid">${card("nodes.html", "Game Mode Studio nodes", data.nodes.length, "Every node: its pins, settings, limits and what bots do with it.")}${card("nodes/types.html", "Pin types", typeEntries(data).length, "What each pin type is, what it plugs into, and the casts.")}${card("entities.html", "Map editor entities", ents.length, "Every entity: what it does in the game, its keys and flags.")}${card("editor.html", "Map editor tools and keys", inv.length, "The editor's menus, panels and dialogs, and their keys.")}</div>
<h2 id="guides">Guides</h2><div class="grid">${GUIDES.map(([id, title, about, file]) => (guideBody(file) ? card(`guides/${file}.html`, title, "guide", about) : card(`../docs.html#${id}`, title, "soon", about))).join("")}</div>`;
  return page({ title: "Reference", lead: "Made from the app's own catalogs, so it always matches the app. Search everything from the box above.", side: `<details open class="here"><summary><a href="./" aria-current="page">Reference</a></summary><ul><li><a href="nodes.html">Studio nodes</a></li><li><a href="entities.html">Map entities</a></li><li><a href="editor.html">Editor tools and keys</a></li></ul></details>`, main, file: "nodes.json, the FGDs, the editor's inventory" });
}

/** The search entries (the reference pages' search, and the docs search): one per node, entity
 *  and editor section. u url from the site root, n name, t where, k keywords, x one-line summary,
 *  cat category, c kind, b works with bots, l has limits, ty pin types. */
function searchIndex(data, ents, inv) {
  return [
    ...data.nodes.map((n) => ({ u: `reference/${nodeHref(n)}`, n: n.title, t: `Studio node · ${n.category}`, k: `${n.type} ${[...n.inputs, ...n.outputs].map((p) => p.label || "").join(" ")}`.trim(), x: firstSentence(n.what), cat: n.category, c: kindOf(n), b: hasBots(n) ? 1 : 0, l: n.limits.length ? 1 : 0, ty: [...new Set([...n.inputs, ...n.outputs].filter((p) => p.type !== "exec").map((p) => typeLabel(data, p.type)))] })),
    ...typeEntries(data).map((e) => ({ u: `reference/nodes/types.html#t-${slug(e.key)}`, n: e.label, t: "Pin type", k: `${e.key} type ${e.values ? e.values.join(" ") : ""} ${e.casts.map((n) => n.title).join(" ")}`.trim(), x: e.what, cat: "Pin types", c: "type", b: 0, l: 0, ty: [e.label] })),
    ...ents.map((e) => ({ u: `reference/${entHref(e)}`, n: e.name, t: `Entity · ${entGroup(e)}`, k: `${e.title} ${e.keys.map((k) => k.name).join(" ")}`, x: firstSentence(e.help), cat: entGroup(e), c: e.kind === "SolidClass" ? "brush" : "point", b: /\bbots?\b/i.test(e.help) ? 1 : 0, l: 0, ty: [] })),
    ...inv.map((s) => ({ u: `reference/editor.html#t-${slug(s.name)}`, n: `Editor: ${s.name}`, t: "Map editor tools and keys", k: s.rows.map((r) => r.where).join(" ").slice(0, 300), x: s.rows.map((r) => r.item).slice(0, 8).join(", "), cat: "Map editor", c: "", b: 0, l: 0, ty: [] })),
    ...guideIndex(),
  ];
}

/** The hand-written guides: [docs.html id, title, about, page in reference/guides/]. A guide is
 *  written when its page has text between its GUIDE markers; until then it's a stub in docs.html. */
const GUIDES = [
  ["guide-types", "Types and casting", "Numbers, text, players, entities, vectors and lists, and turning one into another.", "types"],
  ["guide-events-flow", "Events and flow", "What starts a graph running, and Branch, Sequence, loops and timers.", "events-flow"],
  ["guide-variables", "Variables and saving", "Mode, round and player variables, and what's kept between maps.", "variables"],
  ["guide-spawning", "Spawning and transforms", "Making props, creatures and effects, and moving, turning and attaching them.", "spawning"],
  ["guide-limits", "Limits", "The comfort limits on effects and searches, and when the game itself runs out.", "limits"],
  ["guide-bots", "Steering bots", "Goals, offers, friends and avoid areas: telling bots what matters in your mode.", "bots"],
  ["guide-sharing", "Sharing", "What travels with a mode, what players need, and how credits work.", "sharing"],
];

// ---- the guides (hand-written; this script only frames them) --------------------------------
// Each guide's text lives in its own page, site/reference/guides/<file>.html, between
// <!-- GUIDE:BEGIN --> and <!-- GUIDE:END -->. Running this script keeps that text as it is and
// rebuilds everything round it (the top bar, search, sidebar, On this page, the docs' entry and
// the search index). Node links are written <a class="node" data-node="flow.branch" href="…">;
// their href is set again from the catalog each run, so a node moving category never breaks one.

const GUIDE_BEGIN = "<!-- GUIDE:BEGIN (hand-written: scripts/gen-reference.mjs keeps everything between these markers) -->";
const GUIDE_END = "<!-- GUIDE:END -->";
const guideDir = join(site, "reference", "guides");
const guidePath = (file) => join(guideDir, `${file}.html`);

/** A guide's hand-written text (between its markers), or "" when it isn't written yet. */
function guideBody(file) {
  if (!existsSync(guidePath(file))) return "";
  const t = read(guidePath(file));
  const a = t.indexOf("<!-- GUIDE:BEGIN");
  const b = t.indexOf(GUIDE_END);
  if (a < 0 || b < a) return "";
  return t.slice(t.indexOf("-->", a) + 3, b).trim();
}

const strip = (h) => String(h).replace(/<[^>]+>/g, "").replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();
/** The guide's sections: each <h2 id="…">, with the text up to the next one. */
function guideSections(body) {
  const out = [];
  const re = /<h2 id="([^"]+)"[^>]*>([\s\S]*?)<\/h2>/g;
  let m, last = null;
  while ((m = re.exec(body))) {
    if (last) last.html = body.slice(last.end, m.index);
    last = { id: m[1], title: strip(m[2]), end: re.lastIndex };
    out.push(last);
  }
  if (last) last.html = body.slice(last.end);
  for (const x of out) {
    x.text = strip(x.html);
    // (its search words: the nodes it names and its own headings)
    x.words = [...new Set([...x.html.matchAll(/<a class="node"[^>]*>([^<]+)<\/a>|<h3[^>]*>([\s\S]*?)<\/h3>/g)].map((m) => strip(m[1] || m[2])))].join(" ");
  }
  return out;
}
/** The sections worth listing outside the page (not See also). */
const guideTopics = (body) => guideSections(body).filter((s) => s.id !== "see-also");

/** Node links: href from the catalog (the node's category page); unknown nodes listed. */
function linkNodes(body, byType, bad) {
  return body.replace(/<a class="node" data-node="([^"]+)"( href="[^"]*")?>/g, (_, t) => {
    const n = byType[t];
    if (!n) {
      bad.push(t);
      return `<a class="node" data-node="${t}" href="../nodes.html">`;
    }
    return `<a class="node" data-node="${t}" href="../${nodeHref(n)}">`;
  });
}

/** Guide links that point at a guide not written yet go to its stub in the docs instead. */
function linkGuides(body) {
  return body.replace(/href="([a-z-]+)\.html(#[^"]*)?"/g, (all, f) => {
    const g = GUIDES.find((x) => x[3] === f);
    return g && !guideBody(f) ? `href="../../docs.html#${g[0]}"` : all;
  });
}

const GUIDE_CSS = `/* Made by scripts/gen-reference.mjs: the guides' own bits (with base.css and ref.css). */
article.guide { padding: 0; max-width: 780px; }
article.guide h2 { scroll-margin-top: calc(var(--top) + 120px); }
article.guide h3 { scroll-margin-top: calc(var(--top) + 120px); }
a.node { color: var(--bone); text-decoration: none; font-weight: 600; box-shadow: inset 0 -1px 0 var(--signal); }
a.node:hover { color: var(--signal); }
.chain { background: #23271c; box-shadow: var(--sunken); border-radius: 2px; padding: 9px 12px; margin: 10px 0 14px; line-height: 1.7; overflow-wrap: anywhere; }
.chain .to { color: var(--signal); font-weight: 700; padding: 0 4px; }
.chain .pin { display: inline; width: auto; height: auto; border: 0; margin: 0; color: var(--faint); font-size: .92em; }
.guide .see { margin-top: 34px; }
.guide .table td, .guide .table th { vertical-align: top; }
.badge.k-guide { background: #6a5a2f; }
.guide-soon { opacity: .7; }
@media (max-width: 860px) { .chain { padding: 8px 10px; } }
`;

/** A guide's page: the reference frame round the hand-written text. */
function guidePage(g, body, byType, bad) {
  const [, title, about, file] = g;
  const secs = guideSections(body);
  const side = `<details open class="here"><summary><a href="../#guides">Guides</a><span class="n">${GUIDES.filter((x) => guideBody(x[3])).length}</span></summary><ul>${GUIDES.map(([id, t, , f]) => (guideBody(f) ? `<li><a href="${f}.html"${f === file ? ' aria-current="page"' : ""}>${esc(t)}</a></li>` : `<li class="guide-soon"><a href="../../docs.html#${id}">${esc(t)} (soon)</a></li>`)).join("")}</ul></details>`;
  const main = `<article class="guide">\n${GUIDE_BEGIN}\n${linkGuides(linkNodes(body, byType, bad))}\n${GUIDE_END}\n</article>`;
  return page({ section: "guides", title, lead: about, side, main, toc: secs.map((s) => `<li><a href="#${s.id}">${esc(s.title)}</a></li>`).join(""), file: "", up: "../../", kinds: false, head: `\n<link rel="stylesheet" href="guides.css">`, meta: "A hand-written guide. Every node it names links to the node reference, which is made from the app's own catalog." });
}

/** The guides in the search index: one entry per guide and one per section. */
function guideIndex() {
  return GUIDES.flatMap(([, title, about, file]) => {
    const body = guideBody(file);
    if (!body) return [];
    const secs = guideTopics(body);
    return [
      { u: `reference/guides/${file}.html`, n: title, t: "Guide", k: secs.map((s) => s.title).join(" "), x: about, cat: "Guides", c: "guide", b: /\bbots?\b/i.test(body) ? 1 : 0, l: 0, ty: [] },
      ...secs.map((s) => ({ u: `reference/guides/${file}.html#${s.id}`, n: s.title, t: `Guide · ${title}`, k: s.words.slice(0, 400), x: firstSentence(s.text).slice(0, 200), cat: "Guides", c: "guide", b: /\bbots?\b/i.test(s.text) ? 1 : 0, l: 0, ty: [] })),
    ];
  });
}

/** Writes every written guide's page again (its text kept), and guides.css. Unknown node links are returned. */
export function writeGuides(data) {
  const byType = Object.fromEntries(data.nodes.map((n) => [n.type, n]));
  const bad = [];
  mkdirSync(guideDir, { recursive: true });
  writeFileSync(join(guideDir, "guides.css"), GUIDE_CSS);
  for (const g of GUIDES) {
    const body = guideBody(g[3]);
    if (body) writeFileSync(guidePath(g[3]), guidePage(g, body, byType, bad));
  }
  return bad;
}

/** Node types a guide links to that the catalog doesn't have (the spec checks it's none). */
export function guideProblems() {
  const types = new Set(nodes().nodes.map((n) => n.type));
  const bad = [];
  for (const g of GUIDES) for (const m of guideBody(g[3]).matchAll(/data-node="([^"]+)"/g)) if (!types.has(m[1])) bad.push(`${g[3]}: ${m[1]}`);
  return bad;
}

function docsBlock(data, ents, inv) {
  const cats = catsOf(data);
  const ref = `<section class="topic" id="reference" data-group="Reference" data-keywords="reference nodes entities keys shortcuts catalog">
  <p class="crumb">Reference</p>
  <h1>Reference</h1>
  <p class="lead">Made from the app's own catalogs, so it always matches the app. The docs search finds every entry.</p>
  <ul>
    <li><a href="reference/nodes.html"><b>Game Mode Studio nodes</b></a>: all ${data.nodes.length}, by category (${cats.length}), each with its pins, settings and limits.</li>
    <li><a href="reference/nodes/types.html"><b>Pin types</b></a>: what each type is, what it plugs into, and the casts.</li>
    <li><a href="reference/entities.html"><b>Map editor entities</b></a>: all ${ents.length}, with their keys and flags.</li>
    <li><a href="reference/editor.html"><b>Map editor tools and keys</b></a>: the menus, panels and dialogs, and their keys.</li>
  </ul>
  <p><a href="reference/">Open the reference</a>: it has its own search with filters.</p>
</section>`;
  // (the hand-written guides: a written one is an entry that opens its page; the rest are stubs)
  const stubs = GUIDES.map(([id, title, about, file]) => {
    const body = guideBody(file);
    if (!body) return `<section class="topic" id="${id}" data-group="Reference" data-stub="1">
  <p class="crumb">Reference</p>
  <h1>${esc(title)}</h1>
  <p class="lead">${esc(about)}</p>
  <p>This guide is being written. Until then, the <a href="reference/nodes.html">node reference</a> has every node's own help.</p>
</section>`;
    const secs = guideTopics(body);
    return `<section class="topic" id="${id}" data-group="Reference" data-guide="${file}" data-keywords="guide ${esc(secs.map((s) => s.title).join(" ").toLowerCase())}">
  <p class="crumb">Reference · Guide</p>
  <h1>${esc(title)}</h1>
  <p class="lead">${esc(about)}</p>
  <p><a href="reference/guides/${file}.html"><b>Read the guide</b></a>. In it:</p>
  <ul>
${secs.map((s) => `    <li><a href="reference/guides/${file}.html#${s.id}">${esc(s.title)}</a></li>`).join("\n")}
  </ul>
</section>`;
  }).join("\n\n");
  return `<!-- REFERENCE:BEGIN (made by scripts/gen-reference.mjs; edits here are replaced) -->\n${ref}\n\n${stubs}\n<!-- REFERENCE:END -->`;
}

function llmsBlock(data, ents) {
  const lines = ["<!-- REFERENCE:BEGIN (made by scripts/gen-reference.mjs) -->", "## Game Mode Studio nodes (compact)", "", "One line each: Title (type, category): what it does. In: inputs. Out: outputs.", ""];
  for (const n of data.nodes) {
    const ins = n.inputs.filter((p) => p.type !== "exec").map((p) => `${p.label || p.id} ${typeLabel(data, p.type)}`);
    const outs = n.outputs.filter((p) => p.type !== "exec").map((p) => `${p.label || p.id} ${typeLabel(data, p.type)}`);
    lines.push(`- ${n.title} (${n.type}, ${n.category}): ${n.what}${ins.length ? ` In: ${ins.join(", ")}.` : ""}${outs.length ? ` Out: ${outs.join(", ")}.` : ""}`);
  }
  lines.push("", "## Game Mode Studio pin types (compact)", "", "A wire joins the same type, or a narrower into a wider one (Goes into); the other way takes a cast node.", "");
  for (const e of typeEntries(data)) lines.push(`- ${e.label} (${e.key}): ${e.what}${e.up.length ? ` Goes into: ${e.up.map((t) => typeLabel(data, t)).join(", ")}.` : ""}${e.casts.length ? ` Cast: ${e.casts.map((n) => n.title).join(", ")}.` : ""}${e.values ? ` Values: ${e.values.join(", ")}.` : ""}${e.fields ? ` Fields: ${e.fields.map((f) => f.name).join(", ")}.` : ""}`);
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
  const typeBad = typeProblems();
  if (process.argv.includes("--check")) {
    if (missing.length) console.error(`${missing.length} without help:\n${missing.slice(0, 50).join("\n")}`);
    if (typeBad.length) console.error(`${typeBad.length} pin type problems:\n${typeBad.slice(0, 50).join("\n")}`);
    if (missing.length || typeBad.length) process.exit(1);
    console.log("Every node and entity has help; every pin type has a look and a Types entry.");
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
  const badLinks = writeGuides(data);
  if (badLinks.length) console.warn(`Guides link to nodes the catalog doesn't have: ${[...new Set(badLinks)].join(", ")}`);
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
  console.log(`Wrote ${data.nodes.length} nodes, ${ents.length} entities, ${inv.length} editor sections${missing.length ? `; ${missing.length} without help (see --check)` : ""}. Keys without a description: ${keysWithoutHelp().length}.${typeBad.length ? ` Pin type problems: ${typeBad.length} (see --check).` : ""}`);
}
