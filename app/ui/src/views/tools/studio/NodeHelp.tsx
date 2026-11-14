// Node help: a searchable panel beside the canvas (the "?" menu next to Find, or F1 on a picked
// node). Everything in it comes from nodehelp.json, which node_help.py makes from the node
// registry each time the registry is made, so it can't fall out of step with the nodes.

import { useEffect, useMemo, useRef, useState } from "react";
import { CATEGORIES, NODE_DEFS, nodeDef, typeColor, PIN_TYPES } from "./registry";
import type { Snippet } from "./graphOps";
import type { CanvasApi } from "./Canvas";
import type { Graph } from "./schema";
import { QUICK_ADD, quickAddList } from "./tidyOps";
import { dragHint } from "./dropOps";

/** Ways to add a node without the list: a quick-add key, or a drag from My mode. */
export function addTip(type: string): string | null {
  const k = Object.entries(QUICK_ADD).find(([, q]) => q.type === type)?.[0];
  if (k) return `Quick add: hold ${k.toUpperCase()} and click empty space.`;
  const from: Record<string, string> = { "var.get": "var", "var.set": "var", "event.call": "cevent", "event.custom": "cevent", "timer.by_event": "cevent", "table.get_row": "table", "table.for_each": "table", "flow.switch_enum": "enum", "enum.value": "enum", "struct.make": "struct", "struct.break": "struct", "timeline.play": "tline" };
  return from[type] ? `From My mode: ${dragHint(from[type]).replace(/^Drag onto the canvas/, "drag the item onto the canvas")}.` : null;
}

export type HelpPin = { id: string; dir: "in" | "out"; help: string };
export type HelpEntry = {
  title: string;
  category: string;
  what: string;
  more?: string;
  needs?: string;
  pins: HelpPin[];
  settings: { id: string; help: string }[];
  related: string[];
  cost?: string;
  risk?: string;
  example?: { caption: string; nodes: Graph["nodes"]; edges: Graph["edges"] };
};

let cache: Promise<Record<string, HelpEntry>> | null = null;
/** The help text, loaded the first time it's asked for (it's big, and most visits never open it). */
export function loadHelp(): Promise<Record<string, HelpEntry>> {
  cache ??= import("./nodehelp.json?raw").then((m) => (JSON.parse(m.default) as { help: Record<string, HelpEntry> }).help);
  return cache;
}

/** Help entries that match the words: title first, then category, pins and text. */
export function searchHelp(help: Record<string, HelpEntry>, query: string): string[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const all = NODE_DEFS.map((d) => d.type).filter((t) => help[t]);
  if (!words.length) return all;
  const scored: [string, number][] = [];
  for (const t of all) {
    const h = help[t];
    const d = nodeDef(t);
    const title = h.title.toLowerCase();
    const body = `${h.category} ${h.what} ${h.more ?? ""} ${d?.keywords ?? ""} ${d?.cvar ?? ""} ${h.pins.map((p) => p.help).join(" ")} ${[...(d?.inputs ?? []), ...(d?.outputs ?? [])].map((p) => p.label).join(" ")} ${t}`.toLowerCase();
    let score = 0;
    for (const w of words) {
      if (title.startsWith(w)) score += 4;
      else if (title.split(/\s+/).some((x) => x.startsWith(w))) score += 3;
      else if (title.includes(w)) score += 2;
      else if (body.includes(w)) score += 1;
      else {
        score = -1;
        break;
      }
    }
    if (score > 0) scored.push([t, score]);
  }
  return scored.sort((a, b) => b[1] - a[1]).map(([t]) => t);
}

/** An example as a snippet that drops into the graph. */
export function exampleSnippet(type: string, h: HelpEntry): Snippet | null {
  if (!h.example) return null;
  return {
    id: `help_${type}`,
    name: `${h.title} example`,
    description: h.example.caption,
    part: { schema: "loner-strike/graph", version: 3, mode: { id: "help_example", name: "Example" }, variables: [], nodes: h.example.nodes, edges: h.example.edges, comments: [] } as unknown as Graph,
  };
}

function pinLabel(type: string, id: string, dir: "in" | "out"): { label: string; color: string; kind: string } {
  const d = nodeDef(type);
  const p = (dir === "in" ? d?.inputs : d?.outputs)?.find((x) => x.id === id);
  const t = p?.type ?? "any";
  const kind = t === "exec" ? "Flow" : (PIN_TYPES[t]?.label ?? t.replace(/^\$\w$/, "Matches its setting").replace(/<\$\w(,\$\w)?>/, ""));
  return { label: p?.label || (t === "exec" ? (dir === "in" ? "Run" : "Then") : id), color: typeColor(t), kind };
}

export function NodeHelp({ open, onClose, canvas, toast }: { open: string; onClose: () => void; canvas: CanvasApi | null; toast: (m: string, bad?: boolean) => void }) {
  const [help, setHelp] = useState<Record<string, HelpEntry> | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState("");
  const [cur, setCur] = useState(open);
  const [hi, setHi] = useState(0);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    loadHelp().then(setHelp, () => setFailed(true));
  }, []);
  // a new node asked for (F1 on another node) shows that one
  useEffect(() => {
    setCur(open);
    if (!open) setTimeout(() => search.current?.focus({ preventScroll: true }), 0);
  }, [open]);
  const found = useMemo(() => (help ? searchHelp(help, query) : []), [help, query]);
  useEffect(() => setHi(0), [query]);
  useEffect(() => {
    list.current?.querySelector(".cur")?.scrollIntoView({ block: "nearest" });
  }, [hi]);
  const h = cur && help ? help[cur] : undefined;

  const byCat = useMemo(() => {
    if (query.trim()) return null;
    const m = new Map<string, string[]>();
    for (const t of found) {
      const c = help?.[t]?.category ?? "";
      m.set(c, [...(m.get(c) ?? []), t]);
    }
    return [...m.entries()].sort((a, b) => CATEGORIES.indexOf(a[0]) - CATEGORIES.indexOf(b[0]));
  }, [found, query, help]);

  return (
    <aside className="st-help" role="complementary" aria-label="Node help" data-testid="studio-help" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), cur && !open ? setCur("") : onClose())}>
      <div className="st-help-head">
        {cur ? (
          <button type="button" className="btn small" onClick={() => setCur("")} aria-label="Back to all nodes">
            ‹ All nodes
          </button>
        ) : (
          <strong>Node help</strong>
        )}
        <span className="st-grow" />
        <button type="button" className="btn small" onClick={onClose} aria-label="Close node help" title="Close (Escape)">
          ✕
        </button>
      </div>
      {failed ? (
        <p className="st-bad-text st-help-pad">The help couldn't be loaded. Reload the page to try again.</p>
      ) : !help ? (
        <p className="st-help-pad st-muted" role="status">
          <span className="spinner" /> Loading the help…
        </p>
      ) : h ? (
        <article className="st-help-body" data-testid="studio-help-entry">
          <h3>{h.title}</h3>
          <p className="st-muted">{h.category}</p>
          <p>{h.what}</p>
          {h.more && <p>{h.more}</p>}
          {h.needs && <p className="st-help-note">{h.needs}</p>}
          {h.pins.length > 0 && (
            <>
              <h4>Pins</h4>
              <ul className="st-help-pins">
                {h.pins.map((p) => {
                  const l = pinLabel(cur, p.id, p.dir);
                  return (
                    <li key={`${p.dir}${p.id}`}>
                      <span className="st-help-dot" style={{ background: l.color }} aria-hidden />
                      <strong>{l.label}</strong> <span className="st-muted">({p.dir === "in" ? "in" : "out"}, {l.kind})</span>
                      <div>{p.help}</div>
                    </li>
                  );
                })}
              </ul>
            </>
          )}
          {h.settings.length > 0 && (
            <>
              <h4>Settings</h4>
              <ul className="st-help-pins">
                {h.settings.map((s) => (
                  <li key={s.id}>
                    <strong>{nodeDef(cur)?.params.find((p) => p.id === s.id)?.label ?? s.id}</strong>
                    <div>{s.help}</div>
                  </li>
                ))}
              </ul>
            </>
          )}
          {addTip(cur) && <p className="st-help-note">{addTip(cur)}</p>}
          {h.cost && (
            <p className="st-help-note cost">
              <strong>Cost: </strong>
              {h.cost}
            </p>
          )}
          {h.risk && (
            <p className="st-help-note risk">
              <strong>Take care: </strong>
              {h.risk}
            </p>
          )}
          {h.example && (
            <>
              <h4>Example</h4>
              <p>{h.example.caption}</p>
              <p className="st-muted">{h.example.nodes.map((n) => nodeDef(n.type)?.title ?? n.type).join(" → ")}</p>
            </>
          )}
          <div className="row st-help-actions">
            <button
              type="button"
              className="btn"
              disabled={!canvas}
              onClick={() => {
                canvas?.addAtCenter(cur);
                toast(`Added ${h.title}.`);
              }}
            >
              Add this node
            </button>
            {h.example && (
              <button
                type="button"
                className="btn"
                disabled={!canvas}
                data-testid="studio-help-example"
                onClick={() => {
                  const sn = exampleSnippet(cur, h);
                  if (sn) canvas?.snippetAtCenter(sn);
                }}
              >
                Add the example
              </button>
            )}
          </div>
          {h.related.length > 0 && (
            <>
              <h4>See also</h4>
              <p className="st-help-related">
                {h.related.map((r) => (
                  <button key={r} type="button" className="btn small" onClick={() => setCur(r)}>
                    {help[r]?.title ?? r}
                  </button>
                ))}
              </p>
            </>
          )}
        </article>
      ) : (
        <>
          <input
            ref={search}
            className="st-palette-search"
            type="search"
            placeholder="Search the nodes"
            aria-label="Search the nodes"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                e.preventDefault();
                if (found.length) setHi((k) => (k + (e.key === "ArrowDown" ? 1 : found.length - 1)) % found.length);
              } else if (e.key === "Enter" && found[hi]) {
                e.preventDefault();
                setCur(found[hi]);
              }
            }}
          />
          <p className="st-help-pad st-muted st-help-quick">
            Quick add: hold a key and click empty space: {quickAddList().map((q) => `${q.key} ${q.title}`).join(" · ")}.
          </p>
          <div className="st-help-list" ref={list} role="listbox" aria-label="Nodes">
            {found.length === 0 && <div className="st-palette-none">No node matches that.</div>}
            {(byCat ?? [["", found] as [string, string[]]]).map(([c, ts]) => (
              <div key={c || "found"}>
                {c && <div className="st-palette-cat">{c}</div>}
                {ts.map((t) => {
                  const k = found.indexOf(t);
                  return (
                    <button key={t} type="button" role="option" aria-selected={k === hi} className={`st-palette-item${k === hi ? " cur" : ""}`} onMouseEnter={() => setHi(k)} onClick={() => setCur(t)}>
                      <span className="st-palette-title">
                        {help[t].title}
                        {!byCat && <span className="st-muted"> · {help[t].category}</span>}
                      </span>
                      <span className="st-palette-help">{help[t].what}</span>
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
        </>
      )}
    </aside>
  );
}
