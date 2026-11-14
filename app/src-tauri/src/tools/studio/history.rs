//! Version history: the last saves of each mode (up to 50), kept small as differences. A mode's
//! history is one file beside its graph, `<id>.history.json`: the oldest kept save in full, then
//! one step per later save holding only what changed (nodes, wires, comments, variables... by id;
//! other keys whole). The Studio's History panel lists them, shows one against the graph, and
//! restores it (an undo step in the page).

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const KEEP: usize = 50;

/// The graph's lists of things with ids: changed item by item.
const LISTS: [&str; 13] = ["nodes", "edges", "comments", "variables", "functions", "macros", "structs", "enums", "markers", "eventGraphs", "customEvents", "timelines", "dataTables"];

pub fn file(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.history.json"))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn id_of(v: &Value) -> Option<&str> {
    v.get("id").and_then(Value::as_str)
}

/// What changed from `a` to `b` (both whole graphs as JSON).
pub fn diff(a: &Value, b: &Value) -> Value {
    let (ea, eb) = (Map::new(), Map::new());
    let (a, b) = (a.as_object().unwrap_or(&ea), b.as_object().unwrap_or(&eb));
    let mut out = Map::new();
    let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        let (va, vb) = (a.get(k), b.get(k));
        if va == vb {
            continue;
        }
        let listy = LISTS.contains(&k.as_str()) && va.map_or(true, Value::is_array) && vb.map_or(true, Value::is_array);
        if !listy {
            out.insert(k.clone(), match vb {
                Some(v) => json!({"val": v}),
                None => json!({"gone": true}),
            });
            continue;
        }
        let empty = vec![];
        let la = va.and_then(Value::as_array).unwrap_or(&empty);
        let lb = vb.and_then(Value::as_array).unwrap_or(&empty);
        let set: Vec<&Value> = lb.iter().filter(|x| !la.contains(x)).collect();
        let del: Vec<&str> = la.iter().filter_map(id_of).filter(|i| !lb.iter().any(|x| id_of(x) == Some(*i))).collect();
        let order_a: Vec<&str> = la.iter().filter_map(id_of).filter(|i| !del.contains(i)).collect();
        let order_b: Vec<&str> = lb.iter().filter_map(id_of).collect();
        let mut d = json!({"set": set, "del": del});
        // (the order only when it isn't what applying set/del gives)
        let applied: Vec<&str> = order_a.iter().copied().chain(order_b.iter().copied().filter(|i| !order_a.contains(i))).collect();
        if applied != order_b {
            d["order"] = json!(order_b);
        }
        if vb.is_none() {
            d["gone"] = json!(true);
        }
        out.insert(k.clone(), d);
    }
    Value::Object(out)
}

/// `a` with a diff applied.
pub fn apply(a: &Value, d: &Value) -> Value {
    let mut g = a.as_object().cloned().unwrap_or_default();
    for (k, ch) in d.as_object().into_iter().flatten() {
        if ch.get("gone").and_then(Value::as_bool) == Some(true) {
            g.remove(k);
            continue;
        }
        if let Some(v) = ch.get("val") {
            g.insert(k.clone(), v.clone());
            continue;
        }
        let mut list: Vec<Value> = g.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
        let del: Vec<&str> = ch["del"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        list.retain(|x| !id_of(x).is_some_and(|i| del.contains(&i)));
        for item in ch["set"].as_array().into_iter().flatten() {
            match list.iter_mut().find(|x| id_of(x).is_some() && id_of(x) == id_of(item)) {
                Some(slot) => *slot = item.clone(),
                None => list.push(item.clone()),
            }
        }
        if let Some(order) = ch.get("order").and_then(Value::as_array) {
            let pos = |x: &Value| order.iter().position(|o| o.as_str() == id_of(x)).unwrap_or(usize::MAX);
            list.sort_by_key(pos);
        }
        g.insert(k.clone(), Value::Array(list));
    }
    Value::Object(g)
}

fn read(dir: &Path, id: &str) -> Value {
    std::fs::read_to_string(file(dir, id)).ok().and_then(|t| serde_json::from_str(&t).ok()).filter(|v: &Value| v["base"].is_object()).unwrap_or(Value::Null)
}

/// The kept versions, oldest first: (when, the graph).
pub fn versions(dir: &Path, id: &str) -> Vec<(u64, Value)> {
    let h = read(dir, id);
    if h.is_null() {
        return vec![];
    }
    let mut cur = h["base"].clone();
    let mut out = vec![(h["at"].as_u64().unwrap_or(0), cur.clone())];
    for s in h["steps"].as_array().into_iter().flatten() {
        cur = apply(&cur, &s["diff"]);
        out.push((s["at"].as_u64().unwrap_or(0), cur.clone()));
    }
    out
}

/// Adds a save (nothing when it's the same as the last); keeps the last `KEEP`.
pub fn record(dir: &Path, id: &str, g: &Value) -> Result<(), String> {
    record_at(dir, id, g, now_ms())
}

pub fn record_at(dir: &Path, id: &str, g: &Value, at: u64) -> Result<(), String> {
    let h = read(dir, id);
    let mut base = h["base"].clone();
    let mut base_at = h["at"].as_u64().unwrap_or(at);
    let mut steps: Vec<Value> = h["steps"].as_array().cloned().unwrap_or_default();
    if base.is_null() {
        base = g.clone();
    } else {
        let mut head = base.clone();
        for s in &steps {
            head = apply(&head, &s["diff"]);
        }
        if &head == g {
            return Ok(());
        }
        steps.push(json!({"at": at, "diff": diff(&head, g)}));
        // the oldest folds into the base
        while steps.len() + 1 > KEEP {
            let s = steps.remove(0);
            base = apply(&base, &s["diff"]);
            base_at = s["at"].as_u64().unwrap_or(base_at);
        }
    }
    let text = serde_json::to_string(&json!({"version": 1, "at": base_at, "base": base, "steps": steps})).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&file(dir, id), text.as_bytes()).map_err(|e| format!("Couldn't keep the version history ({e})."))
}

/// For the panel: each version (newest first) with what changed from the one before.
pub fn list(dir: &Path, id: &str) -> Value {
    let v = versions(dir, id);
    let count = |g: &Value, k: &str| g[k].as_array().map_or(0, Vec::len);
    let mut out = vec![];
    for (i, (at, g)) in v.iter().enumerate() {
        let (added, removed, changed) = match i.checked_sub(1).map(|p| &v[p].1) {
            None => (0, 0, 0),
            Some(prev) => {
                let ids = |x: &Value| x["nodes"].as_array().into_iter().flatten().filter_map(|n| id_of(n).map(String::from)).collect::<Vec<_>>();
                let (a, b) = (ids(prev), ids(g));
                let changed = g["nodes"].as_array().into_iter().flatten().filter(|n| prev["nodes"].as_array().into_iter().flatten().any(|m| id_of(m) == id_of(n) && m != *n)).count();
                (b.iter().filter(|x| !a.contains(x)).count(), a.iter().filter(|x| !b.contains(x)).count(), changed)
            }
        };
        out.push(json!({"n": i, "at": at, "nodes": count(g, "nodes"), "added": added, "removed": removed, "changed": changed}));
    }
    out.reverse();
    json!({"versions": out, "keep": KEEP})
}

pub fn get(dir: &Path, id: &str, n: usize) -> Option<Value> {
    versions(dir, id).into_iter().nth(n).map(|x| x.1)
}

pub fn delete(dir: &Path, id: &str) {
    let _ = std::fs::remove_file(file(dir, id));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(nodes: Value) -> Value {
        json!({"schema": "loner-strike/graph", "version": 3, "mode": {"id": "h", "name": "H"}, "variables": [], "nodes": nodes, "edges": [], "comments": []})
    }

    #[test]
    fn diffs_apply_back_to_the_same_graph() {
        let a = g(json!([{"id": "a", "type": "x"}, {"id": "b", "type": "y"}, {"id": "c", "type": "z"}]));
        let mut b = g(json!([{"id": "c", "type": "z2"}, {"id": "a", "type": "x"}, {"id": "d", "type": "w"}]));
        b["mode"]["name"] = json!("H2");
        b["timelines"] = json!([{"id": "t1"}]);
        let d = diff(&a, &b);
        assert_eq!(apply(&a, &d), b);
        // only what changed is kept
        let text = d.to_string();
        assert!(!text.contains("\"type\":\"x\""), "unchanged nodes aren't in it: {text}");
        assert_eq!(apply(&b, &diff(&b, &a)), a);
    }

    #[test]
    fn history_keeps_the_last_fifty_and_skips_the_same() {
        let dir = std::env::temp_dir().join(format!("ls-studio-history-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        delete(&dir, "h");
        for i in 0..60u64 {
            let v = g(json!([{"id": "a", "type": "x", "params": {"n": i}}]));
            record_at(&dir, "h", &v, 1000 + i).unwrap();
            record_at(&dir, "h", &v, 2000 + i).unwrap();
        }
        let v = versions(&dir, "h");
        assert_eq!(v.len(), KEEP);
        assert_eq!(v[0].1["nodes"][0]["params"]["n"], 10);
        assert_eq!(v[KEEP - 1].1["nodes"][0]["params"]["n"], 59);
        assert_eq!(v[KEEP - 1].0, 1059);
        let l = list(&dir, "h");
        assert_eq!(l["versions"][0]["n"], (KEEP - 1) as u64);
        assert_eq!(l["versions"][0]["changed"], 1);
        assert_eq!(get(&dir, "h", 3).unwrap()["nodes"][0]["params"]["n"], 13);
        // small: one changed node per step, not the whole graph
        let size = std::fs::metadata(file(&dir, "h")).unwrap().len();
        assert!(size < 12_000, "{size} bytes");
        delete(&dir, "h");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
