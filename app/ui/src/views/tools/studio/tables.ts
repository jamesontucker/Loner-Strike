// Data tables in the page: the pins the Data table nodes get from their table (the same as
// tables.rs / graph.rs), the edits the grid editor makes, and CSV in and out.

import { freshPartId, uniqueName } from "./graphOps";
import type { PinDef } from "./registry";
import type { GNode, Graph } from "./schema";

export type DataTable = NonNullable<Graph["dataTables"]>[number];
export type TableColumn = DataTable["columns"][number];
export type Cell = DataTable["rows"][number][number];

export const COLUMN_TYPES: [string, string][] = [
  ["number", "Number"],
  ["text", "Text"],
  ["bool", "Yes/No"],
  ["weapon", "Weapon"],
  ["entname", "Entity name"],
];
export const WEAPONS = ["ak47", "m4a1", "awp", "scout", "famas", "galil", "aug", "sg552", "mp5navy", "p90", "ump45", "mac10", "tmp", "m3", "xm1014", "m249", "deagle", "usp", "glock18", "p228", "fiveseven", "elite", "hegrenade", "flashbang", "smokegrenade", "knife", "c4"];
export const LIMITS = { tables: 16, columns: 16, rows: 256, text: 48, cells: 6000 };

/** A column's values as a pin type. */
export const pinType = (c: TableColumn) => (c.type === "weapon" || c.type === "entname" ? "text" : c.type);

/** The pins a Data table node gets from its table. */
export function tablePins(g: Graph, n: GNode): { inType: (id: string) => string | undefined; outType: (id: string) => string | undefined; extra: PinDef[] } | null {
  const t = (g.dataTables ?? []).find((x) => x.id === n.params.table);
  if (!t) return null;
  const key = t.columns.find((c) => c.id === t.key);
  const keyTy = key ? pinType(key) : "any";
  const col = t.columns.find((c) => c.id === n.params.column);
  const colTy = col ? pinType(col) : "any";
  const extra: PinDef[] = n.type === "table.get_row" || n.type === "table.for_each" ? t.columns.map((c) => ({ id: c.id, type: pinType(c), label: c.name, optional: true })) : [];
  return {
    inType: (id) => (n.type === "table.get_row" && id === "key" ? keyTy : n.type === "table.find" && id === "value" ? colTy : undefined),
    outType: (id) => (n.type === "table.get_cell" && id === "out" ? colTy : n.type === "table.keys" && id === "out" ? `array<${keyTy}>` : n.type === "table.find" && id === "out" ? "array<number>" : n.type === "table.for_each" && id === "key" ? keyTy : undefined),
    extra,
  };
}

/** A blank cell of a column's type. */
export function blank(c: TableColumn): Cell {
  if (c.type === "number" || c.type.startsWith("enum:")) return 0;
  if (c.type === "bool") return false;
  if (c.type === "weapon") return "knife";
  return "";
}

export function addTable(g: Graph, name?: string): { graph: Graph; id: string } {
  const id = freshPartId(g, "dt");
  const t: DataTable = { id, name: uniqueName((g.dataTables ?? []).map((x) => x.name), name ?? "New Table"), key: "c1", columns: [{ id: "c1", name: "Key", type: "text" }, { id: "c2", name: "Value", type: "number" }], rows: [["first", 0]] };
  return { graph: { ...g, dataTables: [...(g.dataTables ?? []), t] }, id };
}

export function updateTable(g: Graph, id: string, f: (t: DataTable) => DataTable): Graph {
  return { ...g, dataTables: (g.dataTables ?? []).map((t) => (t.id === id ? f(t) : t)) };
}

export function removeTable(g: Graph, id: string): Graph {
  const left = (g.dataTables ?? []).filter((t) => t.id !== id);
  return { ...g, dataTables: left.length ? left : undefined };
}

/** A fresh column id. */
export function freshColumn(t: DataTable): string {
  for (let i = 1; ; i++) if (!t.columns.some((c) => c.id === `c${i}`)) return `c${i}`;
}

export function addColumn(t: DataTable, type = "number"): DataTable {
  if (t.columns.length >= LIMITS.columns) return t;
  const c: TableColumn = { id: freshColumn(t), name: uniqueName(t.columns.map((x) => x.name), "Column"), type };
  return { ...t, columns: [...t.columns, c], rows: t.rows.map((r) => [...r, blank(c)]) };
}

export function removeColumn(t: DataTable, id: string): DataTable {
  const k = t.columns.findIndex((c) => c.id === id);
  if (k < 0 || t.columns.length <= 1) return t;
  const columns = t.columns.filter((c) => c.id !== id);
  return { ...t, columns, key: t.key === id ? columns[0].id : t.key, rows: t.rows.map((r) => r.filter((_, i) => i !== k)) };
}

/** A column's new type: its cells are read again as that type (what can't be is blanked). */
export function retypeColumn(t: DataTable, id: string, type: string): DataTable {
  const k = t.columns.findIndex((c) => c.id === id);
  if (k < 0) return t;
  const c = { ...t.columns[k], type };
  return { ...t, columns: t.columns.map((x, i) => (i === k ? c : x)), rows: t.rows.map((r) => r.map((v, i) => (i === k ? readCell(c, String(v)) : v))) };
}

/** A cell from typed text: the value, or a blank when it doesn't fit the type. */
export function readCell(c: TableColumn, text: string): Cell {
  const s = text.trim();
  if (c.type === "number") {
    const n = Number(s.replace(",", "."));
    return Number.isFinite(n) && Math.abs(n) <= 1e6 ? n : 0;
  }
  if (c.type === "bool") return /^(1|yes|true|y|on)$/i.test(s);
  if (c.type.startsWith("enum:")) {
    const n = Number(s);
    return Number.isInteger(n) && n >= 0 ? n : 0;
  }
  if (c.type === "weapon") {
    const w = s.toLowerCase().replace(/^weapon_/, "");
    return WEAPONS.includes(w) ? w : "knife";
  }
  // text: the label characters only, at most 48
  return [...s].filter((ch) => /[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/u.test(ch)).join("").slice(0, LIMITS.text);
}

/** Splits CSV (commas, or tabs as a spreadsheet copies) into rows of cells; quotes are honored. */
export function parseCsv(text: string): string[][] {
  const tab = text.includes("\t") && !text.split("\n")[0].includes(",");
  const sep = tab ? "\t" : ",";
  const rows: string[][] = [];
  let row: string[] = [];
  let cur = "";
  let q = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (q) {
      if (ch === '"' && text[i + 1] === '"') {
        cur += '"';
        i++;
      } else if (ch === '"') q = false;
      else cur += ch;
    } else if (ch === '"') q = true;
    else if (ch === sep) {
      row.push(cur);
      cur = "";
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && text[i + 1] === "\n") i++;
      row.push(cur);
      cur = "";
      if (row.some((x) => x.trim())) rows.push(row);
      row = [];
    } else cur += ch;
  }
  row.push(cur);
  if (row.some((x) => x.trim())) rows.push(row);
  return rows;
}

/** CSV into a table: a first row that matches the column names is a header and is skipped;
 *  `replace` drops the rows there were. Says what happened. */
/** Why a CSV cell can't go into a column as it is (null: it can). Import never changes a value to
 *  something else without saying so (QA4-B F5: "abc" in a number column went in as 0). `values`:
 *  an enum column's value names (a cell may be the name or its number). */
export function cellProblem(c: TableColumn, text: string, values?: string[]): string | null {
  const s = text.trim();
  if (c.type === "number") {
    if (!s) return "is empty (a number is needed)";
    const n = Number(s);
    if (!Number.isFinite(n)) return `“${s}” isn't a number`;
    if (Math.abs(n) > 1e6) return `${s} is out of range (-1,000,000 to 1,000,000)`;
    return null;
  }
  if (c.type === "bool") return /^(1|0|yes|no|true|false|y|n|on|off)$/i.test(s) ? null : `“${s}” isn't yes or no`;
  if (c.type.startsWith("enum:")) {
    if (values?.some((v) => v.toLowerCase() === s.toLowerCase())) return null;
    const n = Number(s);
    if (s && Number.isInteger(n) && n >= 0 && (!values || n < values.length)) return null;
    return `“${s}” isn't one of ${values?.length ? values.join(", ") : "its values"}`;
  }
  if (c.type === "weapon") return WEAPONS.includes(s.toLowerCase().replace(/^weapon_/, "")) ? null : `“${s}” isn't a weapon name (ak47, awp, knife...)`;
  if ([...s].length > LIMITS.text) return `is longer than ${LIMITS.text} letters`;
  const bad = [...new Set([...s].filter((ch) => !/[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/u.test(ch)))];
  return bad.length ? `has ${bad.map((ch) => `“${ch}”`).join(" ")}, which a table can't hold` : null;
}

export type RefusedRow = { line: number; cells: string[]; reason: string };

/** Rows from CSV: the good ones added (or replacing the rows there are), every other one refused with
 *  its line and why, none changed into something else. */
export function importCsv(t: DataTable, text: string, replace: boolean, enums?: { id: string; values: string[] }[]): { table: DataTable; message: string; refused: RefusedRow[]; header: boolean } {
  let rows = parseCsv(text);
  const header = !!rows[0] && rows[0].length >= t.columns.length && t.columns.every((c, i) => rows[0][i]?.trim().toLowerCase() === c.name.toLowerCase());
  const first = header ? 2 : 1;
  if (header) rows = rows.slice(1);
  const values = (c: TableColumn) => enums?.find((e) => `enum:${e.id}` === c.type)?.values;
  const refused: RefusedRow[] = [];
  const good: Cell[][] = [];
  rows.forEach((r, i) => {
    const line = i + first;
    if (r.length < t.columns.length) return refused.push({ line, cells: r, reason: `${r.length} value${r.length === 1 ? "" : "s"}, ${t.columns.length} needed (${t.columns.map((c) => c.name).join(", ")})` });
    for (const [k, c] of t.columns.entries()) {
      const why = cellProblem(c, r[k] ?? "", values(c));
      if (why) return refused.push({ line, cells: r, reason: `${c.name} ${why}` });
    }
    good.push(t.columns.map((c, k) => {
      const vs = values(c);
      const named = vs?.findIndex((v) => v.toLowerCase() === r[k].trim().toLowerCase()) ?? -1;
      return named >= 0 ? named : readCell(c, r[k]);
    }));
  });
  const room = Math.max(0, LIMITS.rows - (replace ? 0 : t.rows.length));
  const took = good.slice(0, room);
  const table = { ...t, rows: [...(replace ? [] : t.rows), ...took] };
  const cut = good.length - took.length;
  const parts = [`Added ${took.length} row${took.length === 1 ? "" : "s"}${header ? " (the first row was the column names)" : ""}`];
  if (cut) parts.push(`${cut} didn't fit (${LIMITS.rows} rows at most)`);
  if (refused.length) parts.push(`${refused.length} refused: ${refused.slice(0, 3).map((x) => `line ${x.line}, ${x.reason}`).join("; ")}${refused.length > 3 ? `; and ${refused.length - 3} more` : ""}`);
  return { table, message: parts.join("; ") + ".", refused, header };
}

/** Refused rows back as CSV, to fix and import again. */
export function refusedCsv(t: DataTable, refused: RefusedRow[]): string {
  const q = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  return [t.columns.map((c) => q(c.name)).join(","), ...refused.map((r) => r.cells.map(q).join(","))].join("\n");
}

/** The table as CSV (with the column names first). */
export function toCsv(t: DataTable): string {
  const q = (v: Cell) => {
    const s = typeof v === "boolean" ? (v ? "yes" : "no") : String(v);
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  return [t.columns.map((c) => q(c.name)).join(","), ...t.rows.map((r) => r.map(q).join(","))].join("\n");
}

/** Problems the editor shows at once (the app's check says the same: tables.rs). */
export function tableProblems(g: Graph, t: DataTable): string[] {
  const p: string[] = [];
  const key = t.columns.findIndex((c) => c.id === t.key);
  if (key < 0) p.push("Pick the key column (Get Row finds rows by it).");
  else {
    const seen = new Set<string>();
    t.rows.forEach((r, i) => {
      const k = String(r[key]);
      if (seen.has(k)) p.push(`Row ${i + 1} has the same key as another row.`);
      seen.add(k);
    });
  }
  const cells = (g.dataTables ?? []).reduce((n, x) => n + x.rows.length * x.columns.length, 0);
  if (cells > LIMITS.cells) p.push(`The tables have ${cells} cells together; the most is ${LIMITS.cells}.`);
  return p;
}
