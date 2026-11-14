// The Data table editor (My mode > Data tables): a grid of typed columns and rows with a key
// column, CSV paste and import, Copy as CSV. Each finished cell edit is one undo step.

import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { Sheet, useConfirm } from "../../../ui";
import { useStudio } from "./store";
import { COLUMN_TYPES, LIMITS, WEAPONS, addColumn, blank, importCsv, readCell, removeColumn, removeTable, retypeColumn, tableProblems, toCsv, updateTable, type Cell, type DataTable, type TableColumn } from "./tables";
import { LABEL_CHARS } from "./schema";

const useOpen = create<{ id: string | null; set: (id: string | null) => void }>((set) => ({ id: null, set: (id) => set({ id }) }));
export const openTableEditor = (id: string) => useOpen.getState().set(id);

/** A cell: typed in place, kept when you leave it (Enter or Tab), Escape puts it back. */
function CellField({ col, value, onChange, label }: { col: TableColumn; value: Cell; onChange: (v: Cell) => void; label: string }) {
  const graph = useStudio((s) => s.graph);
  if (col.type === "bool")
    return <input type="checkbox" aria-label={label} checked={value === true} onChange={(e) => onChange(e.target.checked)} />;
  if (col.type === "weapon")
    return (
      <select className="st-field" aria-label={label} value={String(value)} onChange={(e) => onChange(e.target.value)}>
        {WEAPONS.map((w) => (
          <option key={w}>{w}</option>
        ))}
      </select>
    );
  if (col.type.startsWith("enum:")) {
    const en = graph?.enums?.find((e) => `enum:${e.id}` === col.type);
    return (
      <select className="st-field" aria-label={label} value={String(value)} onChange={(e) => onChange(Number(e.target.value))}>
        {(en?.values ?? []).map((v, i) => (
          <option key={v} value={i}>
            {v}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      key={String(value)}
      className="st-field"
      aria-label={label}
      type={col.type === "number" ? "number" : "text"}
      maxLength={LIMITS.text}
      defaultValue={String(value)}
      onBlur={(e) => {
        const v = readCell(col, e.target.value);
        if (v !== value) onChange(v);
        e.target.value = String(v);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") (e.target as HTMLInputElement).value = String(value);
      }}
    />
  );
}

export function TableEditorHost({ toast }: { toast: (m: string, bad?: boolean) => void }) {
  const id = useOpen((s) => s.id);
  const graph = useStudio((s) => s.graph);
  const edit = useStudio((s) => s.edit);
  const [csv, setCsv] = useState<string | null>(null);
  const [replace, setReplace] = useState(false);
  const area = useRef<HTMLTextAreaElement>(null);
  const ask = useConfirm();
  useEffect(() => setCsv(null), [id]);
  const t = graph?.dataTables?.find((x) => x.id === id);
  if (!id || !graph) return null;
  const close = () => useOpen.getState().set(null);
  if (!t) {
    // removed (or undone): nothing to show
    setTimeout(close, 0);
    return null;
  }
  const up = (f: (t: DataTable) => DataTable) => edit((g) => updateTable(g, t.id, f));
  const uses = graph.nodes.filter((n) => n.type.startsWith("table.") && n.params.table === t.id).length;
  const problems = tableProblems(graph, t);
  const enums = (graph.enums ?? []).map((e) => [`enum:${e.id}`, `Enum: ${e.name}`] as [string, string]);
  return (
    <Sheet label={`Data table ${t.name}`} onClose={close} className="sheet st-table-sheet">
      <div className="st-table-ed" data-testid="studio-table-editor">
        <div className="st-table-bar">
          <label>
            Name{" "}
            <input
              className="st-field"
              key={t.name}
              defaultValue={t.name}
              maxLength={30}
              aria-label="Table name"
              onBlur={(e) => {
                const name = [...e.target.value].filter((c) => LABEL_CHARS.test(c)).join("").trim().slice(0, 30);
                if (name && name !== t.name) up((x) => ({ ...x, name }));
              }}
            />
          </label>
          <label>
            Key column{" "}
            <select className="st-field" value={t.key} onChange={(e) => up((x) => ({ ...x, key: e.target.value }))} aria-label="Key column">
              {t.columns.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <span className="st-muted">
            {t.rows.length} of {LIMITS.rows} rows · used by {uses} node{uses === 1 ? "" : "s"}
          </span>
          <span className="st-grow" />
          <button type="button" className="btn small" onClick={() => setCsv("")}>
            Paste or import CSV…
          </button>
          <button type="button" className="btn small" onClick={() => void navigator.clipboard.writeText(toCsv(t)).then(() => toast("Copied as CSV."), () => toast("Couldn't copy.", true))}>
            Copy as CSV
          </button>
        </div>
        {problems.length > 0 && (
          <ul className="st-problems">
            {problems.map((m) => (
              <li key={m} className="error">
                {m}
              </li>
            ))}
          </ul>
        )}
        {csv !== null && (
          <div className="st-table-csv">
            <p className="st-muted">
              Paste rows from a spreadsheet or a CSV file (commas or tabs), in the columns' order. A first row with the column names is skipped. Or{" "}
              <label className="st-link">
                open a .csv file
                <input
                  type="file"
                  accept=".csv,.txt,text/csv"
                  hidden
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f && f.size < 256 * 1024) void f.text().then(setCsv);
                    else if (f) toast("That file is too big for a table.", true);
                  }}
                />
              </label>
              .
            </p>
            <textarea ref={area} className="st-field" rows={6} value={csv} onChange={(e) => setCsv(e.target.value)} aria-label="CSV to import" autoFocus />
            <div className="row">
              <label className="st-side-check st-inline">
                <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} />
                <span>Replace the rows there are</span>
              </label>
              <span className="st-grow" />
              <button type="button" className="btn small" onClick={() => setCsv(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn small primary"
                disabled={!csv.trim()}
                onClick={() => {
                  const r = importCsv(t, csv, replace);
                  up(() => r.table);
                  toast(r.message);
                  setCsv(null);
                }}
              >
                Import
              </button>
            </div>
          </div>
        )}
        <div className="st-table-scroll">
          <table className="st-table-grid">
            <thead>
              <tr>
                <th aria-label="Row" />
                {t.columns.map((c) => (
                  <th key={c.id} className={c.id === t.key ? "key" : ""}>
                    <input
                      className="st-field"
                      key={c.name}
                      defaultValue={c.name}
                      maxLength={30}
                      aria-label={`Column name ${c.name}`}
                      onBlur={(e) => {
                        const name = [...e.target.value].filter((ch) => LABEL_CHARS.test(ch)).join("").trim().slice(0, 30);
                        if (name && name !== c.name) up((x) => ({ ...x, columns: x.columns.map((y) => (y.id === c.id ? { ...y, name } : y)) }));
                      }}
                    />
                    <div className="st-table-colbar">
                      <select className="st-field" value={c.type} aria-label={`Type of ${c.name}`} onChange={(e) => up((x) => retypeColumn(x, c.id, e.target.value))}>
                        {[...COLUMN_TYPES, ...enums].map(([k, l]) => (
                          <option key={k} value={k}>
                            {l}
                          </option>
                        ))}
                      </select>
                      {t.columns.length > 1 && (
                        <button type="button" className="btn small icon" aria-label={`Remove the column ${c.name}`} title="Remove this column" onClick={() => up((x) => removeColumn(x, c.id))}>
                          ×
                        </button>
                      )}
                    </div>
                    {c.id === t.key && <span className="st-muted st-table-keytag">key</span>}
                  </th>
                ))}
                <th>
                  {t.columns.length < LIMITS.columns && (
                    <button type="button" className="btn small" onClick={() => up((x) => addColumn(x))} title="Add a column">
                      + Column
                    </button>
                  )}
                </th>
              </tr>
            </thead>
            <tbody>
              {t.rows.map((r, i) => (
                <tr key={i}>
                  <td className="st-muted">{i}</td>
                  {t.columns.map((c, k) => (
                    <td key={c.id}>
                      <CellField col={c} value={r[k]} label={`Row ${i}, ${c.name}`} onChange={(v) => up((x) => ({ ...x, rows: x.rows.map((rr, ri) => (ri === i ? rr.map((cv, ci) => (ci === k ? v : cv)) : rr)) }))} />
                    </td>
                  ))}
                  <td>
                    <button type="button" className="btn small icon" aria-label={`Remove row ${i}`} title="Remove this row" onClick={() => up((x) => ({ ...x, rows: x.rows.filter((_, ri) => ri !== i) }))}>
                      ×
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="row st-table-foot">
          <button type="button" className="btn small" disabled={t.rows.length >= LIMITS.rows} onClick={() => up((x) => ({ ...x, rows: [...x.rows, x.columns.map(blank)] }))}>
            + Row
          </button>
          <span className="st-muted">Rows count from 0 (Get Cell's Row). Get Row finds a row by its key.</span>
          <span className="st-grow" />
          <button
            type="button"
            className="btn small danger"
            onClick={async () => {
              if (uses && !(await ask(`Delete “${t.name}”? ${uses} node${uses === 1 ? " uses" : "s use"} it and will show a problem until changed.`, { ok: "Delete", danger: true }))) return;
              edit((g) => removeTable(g, t.id));
              close();
            }}
          >
            Delete table
          </button>
          <button type="button" className="btn small primary" onClick={close}>
            Done
          </button>
        </div>
      </div>
    </Sheet>
  );
}
