//! Data tables: a mode's own grids of values (My mode > Data tables): typed columns (number, text,
//! yes/no, an enum, a weapon or an entity name), rows, and a key column. Nodes read them (Get Row,
//! Get Cell, For Each Row, Find Rows, Row Count, Row Keys); the plugin holds them as constant
//! arrays, so a table costs nothing to read. The limits keep the plugin inside AMX Mod X's size.

use super::graph::{label_char, valid_id, Problem};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_TABLES: usize = 16;
pub const MAX_COLUMNS: usize = 12;
pub const MAX_ROWS: usize = 256;
pub const MAX_TEXT: usize = 48;
/// all the tables' cells together (the plugin's data stays small)
pub const MAX_CELLS: usize = 6000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DataTable {
    pub id: String,
    pub name: String,
    /// the key column's id (Get Row finds a row by it)
    pub key: String,
    pub columns: Vec<Column>,
    #[serde(default)]
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Column {
    pub id: String,
    pub name: String,
    /// number | text | bool | enum:<id> | weapon | entname
    #[serde(rename = "type")]
    pub ty: String,
}

impl Column {
    /// The pin type its values have.
    pub fn pin_type(&self) -> String {
        match self.ty.as_str() {
            "weapon" | "entname" => "text".into(),
            t => t.to_string(),
        }
    }
    fn is_text(&self) -> bool {
        matches!(self.ty.as_str(), "text" | "weapon" | "entname")
    }
}

impl DataTable {
    pub fn key_col(&self) -> Option<(usize, &Column)> {
        self.columns.iter().enumerate().find(|(_, c)| c.id == self.key)
    }
    pub fn col(&self, id: &str) -> Option<(usize, &Column)> {
        self.columns.iter().enumerate().find(|(_, c)| c.id == id)
    }
}

/// The weapons a weapon column takes (Give Weapon's names, and knife and c4).
pub const WEAPONS: [&str; 27] = [
    "ak47", "m4a1", "awp", "scout", "famas", "galil", "aug", "sg552", "mp5navy", "p90", "ump45", "mac10", "tmp", "m3", "xm1014", "m249", "deagle", "usp", "glock18", "p228", "fiveseven", "elite", "hegrenade", "flashbang", "smokegrenade", "knife", "c4",
];

fn err(msg: String) -> Problem {
    Problem { level: "error", node: None, pin: None, edge: None, message: msg }
}

/// The tables' own checks (names, types, cells, keys, the size limits).
pub fn validate(tables: &[DataTable], enums: &[(String, usize)]) -> Vec<Problem> {
    let mut p = Vec::new();
    if tables.len() > MAX_TABLES {
        p.push(err(format!("A mode can have at most {MAX_TABLES} data tables.")));
    }
    let mut cells = 0usize;
    for t in tables {
        let name = &t.name;
        if !valid_id(&t.id) || t.name.trim().is_empty() || t.name.chars().count() > 30 || !t.name.chars().all(label_char) {
            p.push(err(format!("The data table \u{201c}{name}\u{201d} needs a name of up to 30 letters, digits, spaces or plain punctuation.")));
        }
        if t.columns.is_empty() || t.columns.len() > MAX_COLUMNS {
            p.push(err(format!("The data table \u{201c}{name}\u{201d} needs 1 to {MAX_COLUMNS} columns.")));
        }
        if t.rows.len() > MAX_ROWS {
            p.push(err(format!("The data table \u{201c}{name}\u{201d} has {} rows; the most is {MAX_ROWS}.", t.rows.len())));
        }
        for c in &t.columns {
            let enum_ok = c.ty.strip_prefix("enum:").map(|e| enums.iter().any(|(id, _)| id == e));
            if !valid_id(&c.id) || c.name.trim().is_empty() || c.name.chars().count() > 30 || !c.name.chars().all(label_char) {
                p.push(err(format!("A column of \u{201c}{name}\u{201d} needs a name of up to 30 letters, digits, spaces or plain punctuation.")));
            }
            if !(matches!(c.ty.as_str(), "number" | "text" | "bool" | "weapon" | "entname") || enum_ok == Some(true)) {
                p.push(err(format!("The column \u{201c}{}\u{201d} of \u{201c}{name}\u{201d} has a type that isn't there (an enum that was removed?).", c.name)));
            }
        }
        if t.key_col().is_none() {
            p.push(err(format!("Pick the key column of \u{201c}{name}\u{201d} (the one Get Row finds rows by).")));
        }
        let mut keys = std::collections::HashSet::new();
        for (r, row) in t.rows.iter().enumerate() {
            if row.len() != t.columns.len() {
                p.push(err(format!("Row {} of \u{201c}{name}\u{201d} has {} cells; the table has {} columns.", r + 1, row.len(), t.columns.len())));
                continue;
            }
            for (c, cell) in t.columns.iter().zip(row) {
                if let Some(m) = cell_problem(c, cell, enums) {
                    p.push(err(format!("Row {}, {} of \u{201c}{name}\u{201d}: {m}", r + 1, c.name)));
                }
            }
            if let Some((k, _)) = t.key_col() {
                let key = row[k].to_string();
                if !keys.insert(key) {
                    p.push(err(format!("Row {} of \u{201c}{name}\u{201d} has the same key as another row (keys must differ).", r + 1)));
                }
            }
        }
        cells += t.rows.len() * t.columns.len();
    }
    if cells > MAX_CELLS {
        p.push(err(format!("The data tables have {cells} cells together; the most a mode can hold is {MAX_CELLS}. Split the data, or keep fewer rows.")));
    }
    p
}

fn cell_problem(c: &Column, v: &Value, enums: &[(String, usize)]) -> Option<String> {
    match c.ty.as_str() {
        "number" => v.as_f64().filter(|n| n.is_finite() && n.abs() <= 1_000_000.0).map(|_| None).unwrap_or(Some("a number (up to a million) goes here.".into())),
        "bool" => v.as_bool().map(|_| None).unwrap_or(Some("yes or no goes here.".into())),
        "weapon" => match v.as_str() {
            Some(w) if WEAPONS.contains(&w) => None,
            _ => Some("a weapon name (ak47, deagle, knife...) goes here.".into()),
        },
        t if t.starts_with("enum:") => {
            let n = enums.iter().find(|(id, _)| Some(id.as_str()) == t.strip_prefix("enum:")).map(|(_, n)| *n).unwrap_or(0);
            v.as_u64().filter(|i| (*i as usize) < n).map(|_| None).unwrap_or(Some("one of the enum's values goes here.".into()))
        }
        _ => match v.as_str() {
            Some(s) if s.chars().count() <= MAX_TEXT && s.chars().all(|ch| label_char(ch) || (c.ty == "entname" && ch == '-')) => None,
            _ => Some(format!("text of up to {MAX_TEXT} letters, digits, spaces and plain punctuation goes here.")),
        },
    }
}

/// Pawn text for a cell (the checks above have passed).
fn lit(c: &Column, v: &Value) -> String {
    match c.ty.as_str() {
        "number" => super::gen::float_lit(v.as_f64().unwrap_or(0.0)),
        "bool" => if v.as_bool().unwrap_or(false) { "1".into() } else { "0".into() },
        t if t.starts_with("enum:") => v.as_u64().unwrap_or(0).to_string(),
        _ => super::gen::pawn_str(v.as_str().unwrap_or("")),
    }
}

/// The tables as constant arrays, with an accessor per column (a row out of range gives 0,
/// no, or empty text) and a key search per table. `used` says which tables any node reads.
pub fn pawn(tables: &[DataTable], used: &dyn Fn(&str) -> bool) -> String {
    let mut o = String::new();
    for (t, tb) in tables.iter().enumerate() {
        if !used(&tb.id) {
            continue;
        }
        let n = tb.rows.len();
        o.push_str(&format!("// Data table: {}\n#define LSG_DT{t}_N {n}\n", super::gen::comment_text(&tb.name)));
        for (c, col) in tb.columns.iter().enumerate() {
            let cells: Vec<String> = tb.rows.iter().map(|r| lit(col, &r[c])).collect();
            let size = n.max(1);
            let body = if cells.is_empty() { if col.is_text() { "\"\"".to_string() } else { "0".to_string() } } else { cells.join(", ") };
            if col.is_text() {
                o.push_str(&format!("stock const g_lsgDt{t}_{c}[{size}][{}] = {{ {body} }}\n", MAX_TEXT + 1));
                o.push_str(&format!("stock lsg_dt{t}_{c}(Float:row)\n{{\n\tnew s[{}], r = floatround(row, floatround_floor)\n\tif (r >= 0 && r < LSG_DT{t}_N)\n\t\tcopy(s, charsmax(s), g_lsgDt{t}_{c}[r])\n\treturn s\n}}\n", MAX_TEXT + 1));
            } else if col.ty == "number" {
                o.push_str(&format!("stock const Float:g_lsgDt{t}_{c}[{size}] = {{ {body} }}\n"));
                o.push_str(&format!("stock Float:lsg_dt{t}_{c}(Float:row)\n{{\n\tnew r = floatround(row, floatround_floor)\n\treturn r >= 0 && r < LSG_DT{t}_N ? g_lsgDt{t}_{c}[r] : 0.0\n}}\n"));
            } else {
                o.push_str(&format!("stock const g_lsgDt{t}_{c}[{size}] = {{ {body} }}\n"));
                o.push_str(&format!("stock lsg_dt{t}_{c}(Float:row)\n{{\n\tnew r = floatround(row, floatround_floor)\n\treturn r >= 0 && r < LSG_DT{t}_N ? g_lsgDt{t}_{c}[r] : 0\n}}\n"));
            }
        }
        // the key search: the row (as a number) with this key, or -1
        if let Some((k, kc)) = tb.key_col() {
            let (arg, test) = if kc.is_text() {
                ("const key[]".to_string(), format!("equal(g_lsgDt{t}_{k}[r], key)"))
            } else if kc.ty == "number" {
                ("Float:key".to_string(), format!("g_lsgDt{t}_{k}[r] == key"))
            } else {
                ("key".to_string(), format!("g_lsgDt{t}_{k}[r] == key"))
            };
            o.push_str(&format!("stock Float:lsg_dt{t}_find({arg})\n{{\n\tfor (new r = 0; r < LSG_DT{t}_N; r++)\n\t\tif ({test})\n\t\t\treturn float(r)\n\treturn -1.0\n}}\n"));
            // the keys, as an array
            let push = if kc.is_text() { format!("ArrayPushString((Array:a), g_lsgDt{t}_{k}[r])") } else { format!("ArrayPushCell((Array:a), g_lsgDt{t}_{k}[r])") };
            o.push_str(&format!("stock lsg_dt{t}_keys()\n{{\n\tnew a = lsg_atmp()\n\tfor (new r = 0; r < LSG_DT{t}_N; r++)\n\t\t{push}\n\treturn a\n}}\n"));
        }
        // Find Rows: the rows where a column has a value (one search per column it's used on)
        for (c, col) in tb.columns.iter().enumerate() {
            let (arg, test) = if col.is_text() {
                ("const v[]".to_string(), format!("equal(g_lsgDt{t}_{c}[r], v)"))
            } else if col.ty == "number" {
                ("Float:v".to_string(), format!("g_lsgDt{t}_{c}[r] == v"))
            } else {
                ("v".to_string(), format!("!g_lsgDt{t}_{c}[r] == !v"))
            };
            let test = if col.ty.starts_with("enum:") { format!("g_lsgDt{t}_{c}[r] == v") } else { test };
            o.push_str(&format!("stock lsg_dt{t}_where{c}({arg})\n{{\n\tnew a = lsg_atmp()\n\tfor (new r = 0; r < LSG_DT{t}_N; r++)\n\t\tif ({test})\n\t\t\tArrayPushCell((Array:a), _:float(r))\n\treturn a\n}}\n"));
        }
        o.push('\n');
    }
    o
}

/// A table's place in the list and the table a node names (its `table` param).
pub fn table_of<'a>(tables: &'a [DataTable], n: &super::graph::Node) -> Option<(usize, &'a DataTable)> {
    let id = n.params.get("table").and_then(Value::as_str)?;
    tables.iter().enumerate().find(|(_, t)| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn guns() -> DataTable {
        serde_json::from_value(json!({
            "id": "guns", "name": "Guns", "key": "lvl",
            "columns": [{"id": "lvl", "name": "Level", "type": "number"}, {"id": "gun", "name": "Gun", "type": "weapon"}, {"id": "ammo", "name": "Ammo", "type": "number"}, {"id": "pistol", "name": "Pistol", "type": "bool"}],
            "rows": [[0, "glock18", 120, true], [1, "mp5navy", 120, false], [2, "ak47", 90, false]]
        }))
        .unwrap()
    }

    #[test]
    fn a_clean_table_checks_clean_and_bad_cells_are_named() {
        assert!(validate(&[guns()], &[]).is_empty());
        let mut t = guns();
        t.rows[1][1] = json!("raygun");
        t.rows[2][0] = json!(0);
        t.rows.push(vec![json!(3), json!("awp")]);
        let p: Vec<String> = validate(&[t], &[]).into_iter().map(|p| p.message).collect();
        assert!(p.iter().any(|m| m.contains("Row 2, Gun") && m.contains("weapon")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("same key")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("Row 4") && m.contains("cells")), "{p:?}");
    }

    #[test]
    fn the_size_limits_are_clear_errors() {
        let mut t = guns();
        t.rows = (0..300).map(|i| vec![json!(i), json!("ak47"), json!(1), json!(false)]).collect();
        let p = validate(&[t.clone()], &[]);
        assert!(p.iter().any(|x| x.message.contains("300 rows; the most is 256")), "{p:?}");
        let many: Vec<DataTable> = (0..8).map(|i| DataTable { id: format!("t{i}"), rows: (0..250).map(|r| vec![json!(r), json!("ak47"), json!(1), json!(false)]).collect(), ..guns() }).collect();
        assert!(validate(&many, &[]).iter().any(|x| x.message.contains("cells together")));
    }

    #[test]
    fn text_with_code_in_it_is_refused() {
        let mut t = guns();
        t.columns[1].ty = "text".into();
        t.rows[0][1] = json!("x\"); server_cmd(\"quit");
        assert!(!validate(&[t], &[]).is_empty());
    }

    #[test]
    fn tables_become_constant_arrays() {
        let src = pawn(&[guns()], &|_| true);
        assert!(src.contains("stock const g_lsgDt0_1[3][49] = { \"glock18\", \"mp5navy\", \"ak47\" }"), "{src}");
        assert!(src.contains("stock const Float:g_lsgDt0_2[3] = { 120.0, 120.0, 90.0 }"), "{src}");
        assert!(src.contains("stock Float:lsg_dt0_find(Float:key)"));
        assert!(pawn(&[guns()], &|_| false).is_empty());
    }
}

#[cfg(test)]
mod graph_tests {
    use super::super::{gen, graph, parse_compiler, run_amxxpc, write_includes};
    use serde_json::json;
    use std::path::{Path, PathBuf};

    /// Every Data table node on a table, wired as a mode would.
    pub fn table_graph() -> graph::Graph {
        let v = json!({
            "schema": "loner-strike/graph", "version": 3,
            "mode": {"id": "tables_test", "name": "Tables"},
            "dataTables": [{"id": "guns", "name": "Guns", "key": "lvl",
                "columns": [{"id": "lvl", "name": "Level", "type": "number"}, {"id": "gun", "name": "Gun", "type": "weapon"}, {"id": "ammo", "name": "Ammo", "type": "number"}, {"id": "pistol", "name": "Pistol", "type": "bool"}],
                "rows": [[0, "glock18", 120, true], [1, "mp5navy", 120, false], [2, "ak47", 90, false]]},
                {"id": "maps", "name": "Map Names", "key": "map", "columns": [{"id": "map", "name": "Map", "type": "text"}, {"id": "nice", "name": "Nice Name", "type": "text"}], "rows": [["de_dust2", "Dust II"]]}],
            "nodes": [
                {"id": "rs", "type": "event.round_started", "params": {}},
                {"id": "each", "type": "table.for_each", "params": {"table": "guns"}},
                {"id": "p1", "type": "debug.print", "params": {"text": "row gun", "chat": false}},
                {"id": "p1b", "type": "debug.print", "params": {"text": "row ammo", "chat": false}},
                {"id": "row", "type": "table.get_row", "params": {"table": "guns"}},
                {"id": "two", "type": "math.number", "params": {"value": 2}},
                {"id": "p2", "type": "debug.print", "params": {"text": "level 2 gun", "chat": false}},
                {"id": "cell", "type": "table.get_cell", "params": {"table": "guns", "column": "gun", "row": 1}},
                {"id": "p3", "type": "debug.print", "params": {"text": "cell", "chat": false}},
                {"id": "find", "type": "table.find", "params": {"table": "guns", "column": "pistol"}},
                {"id": "len", "type": "array.length", "params": {"of": "number"}},
                {"id": "p4", "type": "debug.print", "params": {"text": "pistols", "chat": false}},
                {"id": "cnt", "type": "table.count", "params": {"table": "guns"}},
                {"id": "p5", "type": "debug.print", "params": {"text": "rows", "chat": false}},
                {"id": "keys", "type": "table.keys", "params": {"table": "maps"}},
                {"id": "klen", "type": "array.length", "params": {"of": "text"}},
                {"id": "p6", "type": "debug.print", "params": {"text": "map keys", "chat": false}},
                {"id": "mrow", "type": "table.get_row", "params": {"table": "maps"}},
                {"id": "mname", "type": "game.map_name", "params": {}},
                {"id": "p7", "type": "debug.print", "params": {"text": "nice name", "chat": false}}
            ],
            "edges": [
                {"id": "a1", "from": {"node": "rs", "pin": "then"}, "to": {"node": "each", "pin": "exec"}},
                {"id": "a2", "from": {"node": "each", "pin": "each"}, "to": {"node": "p1", "pin": "exec"}},
                {"id": "a3", "from": {"node": "each", "pin": "gun"}, "to": {"node": "p1", "pin": "value"}},
                {"id": "a3b", "from": {"node": "p1", "pin": "then"}, "to": {"node": "p1b", "pin": "exec"}},
                {"id": "a3c", "from": {"node": "each", "pin": "ammo"}, "to": {"node": "p1b", "pin": "value"}},
                {"id": "a4", "from": {"node": "each", "pin": "then"}, "to": {"node": "p2", "pin": "exec"}},
                {"id": "a5", "from": {"node": "two", "pin": "out"}, "to": {"node": "row", "pin": "key"}},
                {"id": "a6", "from": {"node": "row", "pin": "gun"}, "to": {"node": "p2", "pin": "value"}},
                {"id": "a7", "from": {"node": "p2", "pin": "then"}, "to": {"node": "p3", "pin": "exec"}},
                {"id": "a8", "from": {"node": "cell", "pin": "out"}, "to": {"node": "p3", "pin": "value"}},
                {"id": "a9", "from": {"node": "p3", "pin": "then"}, "to": {"node": "p4", "pin": "exec"}},
                {"id": "a10", "from": {"node": "row", "pin": "pistol"}, "to": {"node": "find", "pin": "value"}},
                {"id": "a11", "from": {"node": "find", "pin": "out"}, "to": {"node": "len", "pin": "array"}},
                {"id": "a12", "from": {"node": "len", "pin": "out"}, "to": {"node": "p4", "pin": "value"}},
                {"id": "a13", "from": {"node": "p4", "pin": "then"}, "to": {"node": "p5", "pin": "exec"}},
                {"id": "a14", "from": {"node": "cnt", "pin": "out"}, "to": {"node": "p5", "pin": "value"}},
                {"id": "a15", "from": {"node": "p5", "pin": "then"}, "to": {"node": "p6", "pin": "exec"}},
                {"id": "a16", "from": {"node": "keys", "pin": "out"}, "to": {"node": "klen", "pin": "array"}},
                {"id": "a17", "from": {"node": "klen", "pin": "out"}, "to": {"node": "p6", "pin": "value"}},
                {"id": "a18", "from": {"node": "p6", "pin": "then"}, "to": {"node": "p7", "pin": "exec"}},
                {"id": "a19", "from": {"node": "mname", "pin": "out"}, "to": {"node": "mrow", "pin": "key"}},
                {"id": "a20", "from": {"node": "mrow", "pin": "nice"}, "to": {"node": "p7", "pin": "value"}}
            ]
        });
        graph::from_value(v).unwrap()
    }

    #[test]
    fn table_nodes_check_clean_and_generate() {
        let g = table_graph();
        if let Ok(d) = std::env::var("X2_DUMP") {
            std::fs::write(d, gen::generate(&g, false).source).unwrap();
        }
        let p = graph::validate(&g);
        assert!(!graph::has_errors(&p), "{p:?}");
        for debug in [false, true] {
            let out = gen::generate(&g, debug);
            assert!(out.source.contains("stock const g_lsgDt0_1[3][49]"), "the table as an array");
            assert!(out.source.contains("lsg_dt0_find(2.0)") || out.source.contains("lsg_dt0_find("), "Get Row searches");
        }
        // a node on a table that isn't there, a column that isn't
        let mut bad = g.clone();
        bad.nodes.iter_mut().find(|n| n.id == "cell").unwrap().params.insert("column".into(), json!("nope"));
        bad.nodes.iter_mut().find(|n| n.id == "cnt").unwrap().params.insert("table".into(), json!("gone"));
        let msgs: Vec<String> = graph::validate(&bad).into_iter().filter(|p| p.level == "error").map(|p| p.message).collect();
        assert!(msgs.iter().any(|m| m.contains("Pick a column")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("Pick a data table")), "{msgs:?}");
    }

    #[test]
    fn table_plugin_compiles_with_amxxpc() {
        let scripting = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/addons/amxmodx/scripting");
        if !scripting.join("amxxpc.exe").is_file() || !Path::new(crate::servers::CROSSOVER_WINE).is_file() {
            eprintln!("skipped: no amxxpc.exe in the CrossOver game, or no CrossOver");
            return;
        }
        let install = crate::steam::Install { hl: PathBuf::new(), kind: "crossover".into(), bottle: Some("Steam".into()), games: vec![], dedicated: false };
        let work = std::env::temp_dir().join(format!("ls-studio-tables-{}", std::process::id()));
        write_includes(&work.join("include")).unwrap();
        let g = table_graph();
        for debug in [false, true] {
            let file = format!("studio_tables{}.sma", if debug { "_dbg" } else { "" });
            std::fs::write(work.join(&file), gen::generate(&g, debug).source).unwrap();
            let (ok, output) = run_amxxpc(&install, &scripting, &work.join(&file), &work.join("include"), &work.join(file.replace(".sma", ".amxx"))).unwrap();
            assert!(ok, "{file}:\n{output}");
            assert!(parse_compiler(&output, &file).is_empty(), "{file} has warnings:\n{output}");
        }
        let _ = std::fs::remove_dir_all(&work);
    }
}
