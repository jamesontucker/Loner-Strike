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
export const LIMITS = { tables: 16, columns: 12, rows: 256, text: 48, cells: 6000 };

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
export function importCsv(t: DataTable, text: string, replace: boolean): { table: DataTable; message: string } {
  let rows = parseCsv(text);
  const header = rows[0] && rows[0].length >= t.columns.length && t.columns.every((c, i) => rows[0][i]?.trim().toLowerCase() === c.name.toLowerCase());
  if (header) rows = rows.slice(1);
  const room = LIMITS.rows - (replace ? 0 : t.rows.length);
  const took = rows.slice(0, Math.max(0, room));
  const read = took.map((r) => t.columns.map((c, i) => readCell(c, r[i] ?? "")));
  const table = { ...t, rows: [...(replace ? [] : t.rows), ...read] };
  const cut = rows.length - took.length;
  return { table, message: `Added ${read.length} row${read.length === 1 ? "" : "s"}${header ? " (the first row was the column names)" : ""}${cut ? `; ${cut} didn't fit (${LIMITS.rows} rows at most)` : ""}.` };
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
