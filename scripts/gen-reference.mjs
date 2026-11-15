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

import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync } from "node:fs";
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

/** docs.html's own look: its <style> block, so the reference pages match. */
function docsStyle() {
  const d = read(join(site, "docs.html"));
  return d.slice(d.indexOf("<style>"), d.indexOf("</style>") + 8);
}

function page({ title, lead, nav, body, file }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)} · Loner-Strike docs</title>
<meta name="description" content="${esc(lead)}">
<link rel="icon" href="../favicon.ico">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Barlow+Semi+Condensed:wght@500;600;700&display=swap" rel="stylesheet">
${docsStyle().replace(/url\("fonts\//g, 'url("../fonts/')}
<style>
.ref .topic { display: block; }
.ref .pins { display: grid; grid-template-columns: minmax(0, 12em) minmax(0, 1fr); gap: 2px 14px; margin: 6px 0 10px; font-size: 16px; }
.ref .pins dt { color: var(--bone); font-weight: 600; }
.ref .pins dd { margin: 0; color: var(--dim); }
.ref .ty { color: var(--faint); font-weight: 500; font-size: 14px; }
.ref .node { padding: 14px 0; box-shadow: inset 0 -1px 0 rgba(0,0,0,.2); }
.ref .node h3 { margin: 0 0 4px; }
.ref h3 .anchor { color: var(--faint); text-decoration: none; margin-left: 6px; font-weight: 500; opacity: 0; }
.ref h3:hover .anchor, .ref h3 .anchor:focus { opacity: .8; }
.ref .meta { color: var(--faint); font-size: 15px; }
.ref .side { position: static; max-height: none; }
@media (min-width: 901px) { .ref .side { position: sticky; top: calc(var(--top) + 20px); max-height: calc(100vh - var(--top) - 40px); } }
@media (max-width: 520px) { .ref .pins { grid-template-columns: 1fr; } .ref .pins dd { margin-bottom: 6px; } }
</style>
</head>
<body class="ref">
<a class="sr-only skip" href="#content">Skip to the reference</a>
<header class="top"><div class="wrap"><div class="brand"><a class="wordmark" href="/">Loner-Strike</a><span class="here">Reference</span></div>
<nav aria-label="Site"><a href="../docs.html">Docs</a><a href="../docs.html#reference">Reference</a><a class="btn" href="/#get">Get it</a></nav></div></header>
<div class="docs">
  <div class="rail"><nav class="side" aria-label="${esc(title)}">${nav}</nav></div>
  <article id="content" tabindex="-1">
    <p class="crumb"><a href="../docs.html#reference">Reference</a></p>
    <h1>${esc(title)}</h1>
    <p class="lead">${esc(lead)}</p>
    <p class="meta">Made from the app's own catalog (${esc(file)}), so it always matches the app.</p>
${body}
  </article>
</div>
<footer><a href="/">lonerstrike.com</a> · <a href="../docs.html">Docs</a> · <a href="https://github.com/jamesontucker/Loner-Strike">GitHub</a></footer>
</body>
</html>
`;
}

const typeLabel = (types, t) => types[t]?.label ?? t.replace(/^(struct|enum|array):/, "");

function nodesPage(data, enums, lib) {
  const cats = [...new Set([...data.categories, ...data.nodes.map((n) => n.category)])].filter((c) => data.nodes.some((n) => n.category === c));
  const nav = `<div class="group"><b>Categories</b><ul>${cats.map((c) => `<li><a href="#c-${slug(c)}">${esc(c)}</a></li>`).join("")}<li><a href="#enums">Built-in lists</a></li><li><a href="#library">Starter library</a></li></ul></div>`;
  const pins = (list, dir) =>
    list.length
      ? `<p class="meta">${dir}</p><dl class="pins">${list.map((p) => `<dt>${esc(p.label || (p.type === "exec" ? (dir === "Inputs" ? "Run" : "Then") : p.id))} <span class="ty">${esc(typeLabel(data.pinTypes, p.type))}</span></dt><dd>${esc(p.help || (p.type === "exec" ? (dir === "Inputs" ? "Runs it." : "Goes on when it's done.") : ""))}</dd>`).join("")}</dl>`
      : "";
  const setting = (s) => {
    const range = [s.min != null ? `from ${s.min}` : "", s.max != null ? `up to ${s.max}` : ""].filter(Boolean).join(" ");
    const choices = Array.isArray(s.choices) ? ` Choices: ${s.choices.map((c) => (Array.isArray(c) ? c[1] ?? c[0] : typeof c === "object" ? c.label ?? c.value : c)).join(", ")}.` : "";
    const def = s.default === "" || s.default == null ? "" : ` Default: ${typeof s.default === "object" ? JSON.stringify(s.default) : s.default}.`;
    return `<dt>${esc(s.label)}</dt><dd>${esc(s.help)}${esc(def)}${range ? esc(` ${range[0].toUpperCase()}${range.slice(1)}.`) : ""}${esc(choices)}</dd>`;
  };
  const byType = Object.fromEntries(data.nodes.map((n) => [n.type, n]));
  const body = cats
    .map((c) => {
      const list = data.nodes.filter((n) => n.category === c);
      return `<section class="topic" id="c-${slug(c)}"><h2>${esc(c)}</h2>${list
        .map(
          (n) => `<div class="node" id="${nodeId(n.type)}"><h3>${esc(n.title)} <a class="anchor" href="#${nodeId(n.type)}" aria-label="Link to ${esc(n.title)}">#</a></h3>
<p>${esc(n.what)}</p>${n.more ? `<p>${esc(n.more)}</p>` : ""}${pins(n.inputs, "Inputs")}${pins(n.outputs, "Outputs")}${n.settings.length ? `<p class="meta">Settings</p><dl class="pins">${n.settings.map(setting).join("")}</dl>` : ""}${n.limits.length ? `<p class="meta">Limits: ${n.limits.map((l) => esc(`${l.label} (${l.default} by default, hosts can raise it to ${l.max}; slow above ${l.lagAbove})`)).join("; ")}.</p>` : ""}${n.needs ? `<p class="meta">${esc(n.needs)}</p>` : ""}${n.risk ? `<p class="meta">Careful: ${esc(n.risk)}</p>` : ""}${n.cost ? `<p class="meta">Cost: ${esc(n.cost)}</p>` : ""}${n.related.length ? `<p class="meta">Related: ${n.related.map((r) => `<a href="#${nodeId(r)}">${esc(byType[r]?.title ?? r)}</a>`).join(", ")}</p>` : ""}</div>`,
        )
        .join("\n")}</section>`;
    })
    .join("\n");
  const enumsHtml = `<section class="topic" id="enums"><h2>Built-in lists</h2><p>Lists every mode has (What Kind Of, Component Kind Of give them; Switch on Enum fills its cases from them).</p>${enums.map((e) => `<h3 id="enum-${slug(e.id)}">${esc(e.name)}</h3><p>${esc(e.values.join(", "))}</p>`).join("")}</section>`;
  const libHtml = `<section class="topic" id="library"><h2>Starter library</h2><p>Ready-made pieces to add from Game Mode Studio's My library.</p>${lib.map((l) => `<div class="node" id="lib-${slug(l.name)}"><h3>${esc(l.name)}</h3><p class="meta">${esc(l.category)}</p><p>${esc(l.description)}</p>${l.inputs.length ? `<p class="meta">In: ${esc(l.inputs.map((p) => p.name).join(", "))}. Out: ${esc(l.outputs.map((p) => p.name).join(", "))}.</p>` : ""}${l.tables.length ? `<p class="meta">Brings the table${l.tables.length > 1 ? "s" : ""} ${esc(l.tables.join(", "))}.</p>` : ""}</div>`).join("")}</section>`;
  return page({ title: "Game Mode Studio nodes", lead: `Every node (${data.nodes.length}): what it does, its inputs and outputs, settings, limits and related nodes.`, nav, body: body + enumsHtml + libHtml, file: "nodes.json, nodehelp.json, limits.ts" });
}

function entitiesPage(ents) {
  const groups = [...new Set(ents.map((e) => `${e.source}: ${e.group}`))];
  const nav = groups.map((g) => `<div class="group"><b>${esc(g)}</b><ul>${ents.filter((e) => `${e.source}: ${e.group}` === g).map((e) => `<li><a href="#${entId(e.name)}">${esc(e.name)}</a></li>`).join("")}</ul></div>`).join("");
  const body = groups
    .map(
      (g) => `<section class="topic" id="g-${slug(g)}"><h2>${esc(g)}</h2>${ents
        .filter((e) => `${e.source}: ${e.group}` === g)
        .map((e) => {
          const keys = e.keys.filter((k) => k.type !== "flags");
          const flags = e.keys.find((k) => k.type === "flags");
          return `<div class="node" id="${entId(e.name)}"><h3>${esc(e.name)}${e.title && e.title !== e.name ? ` <span class="ty">${esc(e.title)}</span>` : ""} <a class="anchor" href="#${entId(e.name)}" aria-label="Link to ${esc(e.name)}">#</a></h3>
<p>${esc(e.help)}</p>${e.blurb && e.blurb !== e.help ? `<p>${esc(e.blurb)}</p>` : ""}<p class="meta">${e.kind === "SolidClass" ? "A brush entity (tie brushes to it)." : "A point entity."}${e.source.startsWith("Loner") ? " Needs Loner-Strike's components plugin (it comes with the app)." : ""}</p>${keys.length ? `<p class="meta">Keys</p><dl class="pins">${keys.map((k) => `<dt>${esc(k.label || k.name)} <span class="ty">${esc(k.name)}</span></dt><dd>${esc(k.help)}${k.default ? esc(` Default: ${k.default}.`) : ""}${k.options.length ? esc(` Choices: ${k.options.map((o) => `${o.value} ${o.label}`).join(", ")}.`) : ""}</dd>`).join("")}</dl>` : ""}${flags?.options.length ? `<p class="meta">Flags</p><dl class="pins">${flags.options.map((o) => `<dt>${esc(o.value)}</dt><dd>${esc(o.label)}${o.on ? " (on by default)" : ""}</dd>`).join("")}</dl>` : ""}</div>`;
        })
        .join("\n")}</section>`,
    )
    .join("\n");
  return page({ title: "Map editor entities", lead: `Every entity the map editor places (${ents.length}): what it does in the game, its keys and flags.`, nav, body, file: "cstrike.fgd, lonerstrike.fgd, components.json" });
}

function editorPage(inv) {
  const nav = `<div class="group"><b>Where</b><ul>${inv.map((s) => `<li><a href="#t-${slug(s.name)}">${esc(s.name)}</a></li>`).join("")}</ul></div>`;
  const body = inv.map((s) => `<section class="topic" id="t-${slug(s.name)}"><h2>${esc(s.name)}</h2><dl class="pins">${s.rows.map((r) => `<dt>${esc(r.item)}</dt><dd>${esc(r.where)}</dd>`).join("")}</dl></section>`).join("\n");
  return page({ title: "Map editor tools and keys", lead: "Everything in the map editor's menus, panels and dialogs, and where to find it, with the keys.", nav, body, file: "the editor's coverage inventory" });
}

/** The docs search's extra entries: one per node, entity and editor section. */
function searchIndex(data, ents, inv) {
  const cut = (s, n = 160) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);
  return [
    ...data.nodes.map((n) => ({ u: `reference/nodes.html#${nodeId(n.type)}`, n: n.title, t: `Studio node · ${n.category}`, k: n.type, x: cut([n.what, ...n.inputs.map((p) => p.label), ...n.outputs.map((p) => p.label)].filter(Boolean).join(" ")) })),
    ...ents.map((e) => ({ u: `reference/entities.html#${entId(e.name)}`, n: e.name, t: `Entity · ${e.group}`, k: `${e.title} ${e.keys.map((k) => k.name).join(" ")}`, x: cut(e.help) })),
    ...inv.map((s) => ({ u: `reference/editor.html#t-${slug(s.name)}`, n: `Editor: ${s.name}`, t: "Map editor tools and keys", k: "", x: cut(s.rows.map((r) => r.item).join(", ")) })),
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
  const cats = [...new Set([...data.categories, ...data.nodes.map((n) => n.category)])].filter((c) => data.nodes.some((n) => n.category === c));
  const ref = `<section class="topic" id="reference" data-group="Reference" data-keywords="reference nodes entities keys shortcuts catalog">
  <p class="crumb">Reference</p>
  <h1>Reference</h1>
  <p class="lead">Made from the app's own catalogs, so it always matches the app. The docs search finds every entry.</p>
  <ul>
    <li><a href="reference/nodes.html"><b>Game Mode Studio nodes</b></a>: all ${data.nodes.length}, by category: ${cats.map((c) => `<a href="reference/nodes.html#c-${slug(c)}">${esc(c)}</a>`).join(", ")}.</li>
    <li><a href="reference/entities.html"><b>Map editor entities</b></a>: all ${ents.length}, with their keys and flags.</li>
    <li><a href="reference/editor.html"><b>Map editor tools and keys</b></a>: ${inv.map((s) => esc(s.name)).join(", ")}.</li>
  </ul>
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
  writeFileSync(join(out, "nodes.html"), nodesPage(data, builtinEnums(), library()));
  writeFileSync(join(out, "entities.html"), entitiesPage(ents));
  writeFileSync(join(out, "editor.html"), editorPage(inv));
  writeFileSync(join(out, "index.js"), `// Made by scripts/gen-reference.mjs: the docs search's reference entries (u url, n name, t where, k keywords, x text)\nwindow.LS_REFERENCE = ${JSON.stringify(searchIndex(data, ents, inv))};\n`);
  const docs = join(site, "docs.html");
  writeFileSync(docs, replaceBlock(read(docs), docsBlock(data, ents, inv), '<section class="topic" id="something-wrong"'));
  const llms = join(site, "llms-full.txt");
  if (existsSync(llms)) writeFileSync(llms, replaceBlock(read(llms), llmsBlock(data, ents), null));
  console.log(`Wrote ${data.nodes.length} nodes, ${ents.length} entities, ${inv.length} editor sections${missing.length ? `; ${missing.length} without help (see --check)` : ""}. Keys without a description: ${keysWithoutHelp().length}.`);
}
