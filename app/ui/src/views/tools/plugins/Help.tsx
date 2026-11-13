// The Plugin Studio's Help: the guides worth reading (links out), a search over every word the
// include files define (with its docs, where it comes from, and its AMX Mod X page), and
// Loner-Strike's own docs (loner.inc, the game mode components) shown right here.

import { useMemo, useState, type ReactNode } from "react";
import { Sheet } from "../../../ui";
import { docsUrl, GUIDES } from "./docs";
import type { Word, Words } from "./pluginsApi";

const OWN_DOCS: { id: string; title: string; what: string; load: () => Promise<{ default: string }> }[] = [
  { id: "loner", title: "loner.inc: Loner-Strike's helpers", what: "Modes for the Game Mode Menu, shared HUD slots, weapons with ammo, timers, bot orders.", load: () => import("../../../../../../docs/LONER-INC.md?raw") },
  { id: "components", title: "Game mode components", what: "The ls_* pieces modes are built from, their keys, and the natives and forwards in ls_components.inc.", load: () => import("../../../../../../docs/COMPONENTS.md?raw") },
];

const KIND: Record<Word["kind"], string> = { native: "native", forward: "forward", stock: "stock", define: "define", macro: "macro", constant: "constant", enum: "enum" };

/** Every word, any case, "_" ignored: "user name" finds get_user_name. */
function matches(s: Word, words: string[]) {
  const hay = `${s.name} ${s.file}`.toLowerCase().replace(/_/g, " ");
  const name = s.name.toLowerCase();
  return words.every((w) => hay.includes(w) || name.includes(w));
}

export function HelpPanel({ words, query, setQuery, onOpen, onInsert }: {
  words: Words | undefined;
  query: string;
  setQuery: (q: string) => void;
  /** opens the word's include (read only) at its line */
  onOpen: (s: Word) => void;
  /** puts the word at the cursor */
  onInsert: (s: Word) => void;
}) {
  const [reading, setReading] = useState<{ title: string; text: string } | null>(null);
  const [picked, setPicked] = useState<string | null>(null);
  const found = useMemo(() => {
    const q = query.trim().toLowerCase().replace(/_/g, " ");
    if (!q || !words) return [];
    const parts = q.split(/\s+/);
    const hits = words.symbols.filter((s) => matches(s, parts));
    // exact names first, then names starting with it, then functions before constants
    const rank = (s: Word) => (s.name.toLowerCase() === query.trim().toLowerCase() ? 0 : s.name.toLowerCase().startsWith(parts[0]) ? 1 : 2) * 10 + (s.kind === "native" || s.kind === "forward" || s.kind === "stock" ? 0 : 1);
    return hits.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name)).slice(0, 60);
  }, [query, words]);
  const read = async (d: (typeof OWN_DOCS)[number]) => {
    try {
      setReading({ title: d.title, text: (await d.load()).default });
    } catch {
      setReading({ title: d.title, text: "This document couldn't be loaded." });
    }
  };
  return (
    <div className="ps-help">
      <section className="ps-help-search" aria-label="Search the docs">
        <input
          type="search"
          className="ps-help-box"
          placeholder={words ? `Search ${words.symbols.length.toLocaleString()} functions, forwards and constants (e.g. "user health")` : "Reading the include files…"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Search the docs"
          spellCheck={false}
        />
        {query.trim() && found.length === 0 && <p className="muted">Nothing called that. Try fewer words, or part of a name.</p>}
        <ul className="ps-help-hits">
          {found.map((s) => {
            const open = picked === `${s.file}:${s.name}`;
            const url = docsUrl(s);
            return (
              <li key={`${s.file}:${s.name}`} className={open ? "on" : undefined}>
                <button className="ps-help-hit" aria-expanded={open} onClick={() => setPicked(open ? null : `${s.file}:${s.name}`)}>
                  <code>{s.name}</code>
                  <span className="muted">
                    {KIND[s.kind]} · {s.file}
                  </span>
                  {s.doc && !open && <span className="ps-help-line">{s.doc.split(/(?<=\.)\s/)[0]}</span>}
                </button>
                {open && (
                  <div className="ps-help-doc">
                    <pre>{s.detail}</pre>
                    {s.deprecated !== undefined && <p className="ps-help-dep">Deprecated. {s.deprecated}</p>}
                    {s.doc && <p>{s.doc}</p>}
                    {s.params.some((p) => p.doc) && (
                      <dl>
                        {s.params.map((p) => (
                          <div key={p.name}>
                            <dt><code>{p.name}</code></dt>
                            <dd>{p.doc || <span className="muted">(no description)</span>}</dd>
                          </div>
                        ))}
                      </dl>
                    )}
                    {s.returns && <p><strong>Returns</strong> {s.returns}</p>}
                    {(s.notes ?? []).map((n, i) => (
                      <p key={i} className="muted">Note: {n}</p>
                    ))}
                    {s.error && <p className="muted">Errors: {s.error}</p>}
                    <div className="row">
                      <button className="btn small" onClick={() => onInsert(s)}>Put it in my code</button>
                      <button className="btn small" onClick={() => onOpen(s)}>Open {s.file.split("/").pop()} at it</button>
                      {url && <a className="btn small" href={url} target="_blank" rel="noreferrer">AMX Mod X docs ↗</a>}
                    </div>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
        {words && (
          <p className="muted small-note">
            From {words.files ?? "your"} include files in your game, loner.inc and ls_components.inc.
            {words.empty?.length ? ` (${words.empty.join(", ")}: nothing in ${words.empty.length === 1 ? "it" : "them"} to list.)` : ""}
          </p>
        )}
      </section>
      <section className="ps-help-guides" aria-label="Guides">
        <h4>Loner-Strike's docs</h4>
        <ul>
          {OWN_DOCS.map((d) => (
            <li key={d.id}>
              <button className="ps-help-link" onClick={() => read(d)}>{d.title}</button>
              <span className="muted"> {d.what}</span>
            </li>
          ))}
        </ul>
        <h4>Guides on the web</h4>
        <ul>
          {GUIDES.map((g) => (
            <li key={g.url}>
              <a href={g.url} target="_blank" rel="noreferrer">{g.title} ↗</a>
              <span className="muted"> {g.what}</span>
            </li>
          ))}
        </ul>
      </section>
      {reading && (
        <Sheet className="sheet wide ps-doc-sheet" label={reading.title} onClose={() => setReading(null)}>
          <div className="sheet-body">
            <Markdown text={reading.text} />
            <div className="row">
              <button className="btn" onClick={() => setReading(null)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}
    </div>
  );
}

// ---- a small Markdown reader (our docs: headings, lists, tables, code, links) -----------------

function inline(text: string, key: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(`[^`]+`)|\*\*([^*]+)\*\*|\[([^\]]+)\]\(([^)\s]+)\)|\*([^*\s][^*]*)\*/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let n = 0;
  while ((m = re.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const k = `${key}-${n++}`;
    if (m[1]) out.push(<code key={k}>{m[1].slice(1, -1)}</code>);
    else if (m[2]) out.push(<strong key={k}>{m[2]}</strong>);
    else if (m[3]) out.push(/^https:\/\//.test(m[4]) ? <a key={k} href={m[4]} target="_blank" rel="noreferrer">{m[3]}</a> : <em key={k}>{m[3]}</em>);
    else if (m[5]) out.push(<em key={k}>{m[5]}</em>);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({ text }: { text: string }) {
  const lines = text.replace(/\r/g, "").split("\n");
  const blocks: ReactNode[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const k = `b${i}`;
    if (line.startsWith("```")) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) body.push(lines[i++]);
      i++;
      blocks.push(<pre key={k}><code>{body.join("\n")}</code></pre>);
      continue;
    }
    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) {
      const level = Math.min(h[1].length + 1, 5);
      const Tag = `h${level}` as "h2" | "h3" | "h4" | "h5";
      blocks.push(<Tag key={k}>{inline(h[2], k)}</Tag>);
      i++;
      continue;
    }
    if (line.startsWith("|")) {
      const rows: string[][] = [];
      while (i < lines.length && lines[i].startsWith("|")) {
        const cells = lines[i].replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
        if (!cells.every((c) => /^:?-+:?$/.test(c))) rows.push(cells);
        i++;
      }
      const [head, ...body] = rows;
      blocks.push(
        <div key={k} className="ps-md-table">
          <table>
            <thead><tr>{head.map((c, j) => <th key={j}>{inline(c, `${k}h${j}`)}</th>)}</tr></thead>
            <tbody>{body.map((r, ri) => <tr key={ri}>{r.map((c, j) => <td key={j}>{inline(c, `${k}${ri}-${j}`)}</td>)}</tr>)}</tbody>
          </table>
        </div>,
      );
      continue;
    }
    if (/^\s*([-*]|\d+\.)\s+/.test(line)) {
      const ordered = /^\s*\d+\./.test(line);
      const items: string[] = [];
      while (i < lines.length && (/^\s*([-*]|\d+\.)\s+/.test(lines[i]) || (/^\s{2,}\S/.test(lines[i]) && items.length))) {
        if (/^\s*([-*]|\d+\.)\s+/.test(lines[i])) items.push(lines[i].replace(/^\s*([-*]|\d+\.)\s+/, ""));
        else items[items.length - 1] += " " + lines[i].trim();
        i++;
      }
      const List = ordered ? "ol" : "ul";
      blocks.push(<List key={k}>{items.map((t, j) => <li key={j}>{inline(t, `${k}${j}`)}</li>)}</List>);
      continue;
    }
    if (!line.trim()) {
      i++;
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && lines[i].trim() && !/^(```|#{1,4}\s|\||\s*([-*]|\d+\.)\s)/.test(lines[i])) para.push(lines[i++].trim());
    blocks.push(<p key={k}>{inline(para.join(" "), k)}</p>);
  }
  return <div className="ps-md">{blocks}</div>;
}
