//! Version history: the last saves of each mode (up to 50), kept small as differences. A mode's
//! history is one file beside its graph, `<id>.history.json`:
//! - `head`: the newest save, whole, so a save only diffs against it (no replay: StudioPerfAudit #7);
//! - `back`: the older saves, oldest first, each the change that takes the save after it back to
//!   it (nodes, wires, comments, variables... by id; other keys whole), and every tenth one whole,
//!   so opening an old save applies at most a few changes;
//! - each save's counts (nodes; added, removed and changed since the one before), so the list
//!   reads them and rebuilds nothing.
//! The oldest are dropped past 50 saves, or while the file is over 4 MB. The Studio's History
//! panel lists them, shows one against the graph, and restores it (an undo step in the page).
//! (A version 1 file, the oldest save whole and then forward steps, is read and turned into this
//! at the next save.)

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const KEEP: usize = 50;
/// The history file's most: past it the oldest saves go (a 200 KB graph moved whole each save
/// keeps about 20).
const MAX_BYTES: usize = 4 << 20;
/// Every this many kept saves, one is whole.
const FULL_EVERY: usize = 10;

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
        // (by id: a HashMap, not a search per item)
        let by_id_a: HashMap<&str, &Value> = la.iter().filter_map(|x| id_of(x).map(|i| (i, x))).collect();
        let ids_b: HashSet<&str> = lb.iter().filter_map(id_of).collect();
        let set: Vec<&Value> = lb
            .iter()
            .filter(|x| match id_of(x) {
                Some(i) => by_id_a.get(i) != Some(x),
                None => !la.contains(x),
            })
            .collect();
        let del: Vec<&str> = la.iter().filter_map(id_of).filter(|i| !ids_b.contains(i)).collect();
        let del_set: HashSet<&str> = del.iter().copied().collect();
        let order_a: Vec<&str> = la.iter().filter_map(id_of).filter(|i| !del_set.contains(i)).collect();
        let in_a: HashSet<&str> = order_a.iter().copied().collect();
        let order_b: Vec<&str> = lb.iter().filter_map(id_of).collect();
        let mut d = json!({"set": set, "del": del});
        // (the order only when it isn't what applying set/del gives)
        let applied: Vec<&str> = order_a.iter().copied().chain(order_b.iter().copied().filter(|i| !in_a.contains(i))).collect();
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
        let mut list: Vec<Value> = g.remove(k).and_then(|v| if let Value::Array(a) = v { Some(a) } else { None }).unwrap_or_default();
        let del: HashSet<&str> = ch["del"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !del.is_empty() {
            list.retain(|x| !id_of(x).is_some_and(|i| del.contains(i)));
        }
        let mut at: HashMap<String, usize> = list.iter().enumerate().filter_map(|(n, x)| id_of(x).map(|i| (i.to_string(), n))).collect();
        for item in ch["set"].as_array().into_iter().flatten() {
            match id_of(item).and_then(|i| at.get(i)) {
                Some(&n) => list[n] = item.clone(),
                None => {
                    if let Some(i) = id_of(item) {
                        at.insert(i.to_string(), list.len());
                    }
                    list.push(item.clone());
                }
            }
        }
        if let Some(order) = ch.get("order").and_then(Value::as_array) {
            let rank: HashMap<&str, usize> = order.iter().enumerate().filter_map(|(n, o)| o.as_str().map(|s| (s, n))).collect();
            list.sort_by_key(|x| id_of(x).and_then(|i| rank.get(i).copied()).unwrap_or(usize::MAX));
        }
        g.insert(k.clone(), Value::Array(list));
    }
    Value::Object(g)
}

/// Nodes added, removed and changed from `a` to `b` (for the list).
fn counts(a: &Value, b: &Value) -> (usize, usize, usize) {
    let nodes = |g: &Value| -> HashMap<String, Value> { g["nodes"].as_array().into_iter().flatten().filter_map(|n| id_of(n).map(|i| (i.to_string(), n.clone()))).collect() };
    let (na, nb) = (nodes(a), nodes(b));
    let added = nb.keys().filter(|i| !na.contains_key(*i)).count();
    let removed = na.keys().filter(|i| !nb.contains_key(*i)).count();
    let changed = nb.iter().filter(|(i, n)| na.get(*i).is_some_and(|m| m != *n)).count();
    (added, removed, changed)
}

fn node_count(g: &Value) -> usize {
    g["nodes"].as_array().map_or(0, Vec::len)
}

/// One kept save's facts, as the file holds them.
fn meta(at: u64, g: &Value, c: (usize, usize, usize)) -> Value {
    json!({"at": at, "nodes": node_count(g), "added": c.0, "removed": c.1, "changed": c.2})
}

/// The history as it's kept: the newest whole, then the older ones newest last.
struct History {
    head: Value,
    head_meta: Value,
    back: Vec<Value>,
}

/// A version 1 file (the oldest whole, then forward steps) as this one.
fn from_v1(h: &Value) -> Option<History> {
    let mut cur = h["base"].clone();
    if !cur.is_object() {
        return None;
    }
    let mut vs = vec![(h["at"].as_u64().unwrap_or(0), cur.clone())];
    for s in h["steps"].as_array().into_iter().flatten() {
        cur = apply(&cur, &s["diff"]);
        vs.push((s["at"].as_u64().unwrap_or(0), cur.clone()));
    }
    let mut back = vec![];
    for i in 0..vs.len() - 1 {
        let c = if i == 0 { (0, 0, 0) } else { counts(&vs[i - 1].1, &vs[i].1) };
        let mut e = meta(vs[i].0, &vs[i].1, c);
        if (vs.len() - 1 - i) % FULL_EVERY == 0 {
            e["full"] = vs[i].1.clone();
        } else {
            e["diff"] = diff(&vs[i + 1].1, &vs[i].1);
        }
        back.push(e);
    }
    let n = vs.len();
    let c = if n > 1 { counts(&vs[n - 2].1, &vs[n - 1].1) } else { (0, 0, 0) };
    let (at, head) = vs.pop()?;
    Some(History { head_meta: meta(at, &head, c), head, back })
}

fn read(dir: &Path, id: &str) -> Option<History> {
    let t = std::fs::read_to_string(file(dir, id)).ok()?;
    let mut v: Value = serde_json::from_str(&t).ok()?;
    if v["version"] == 2 && v["head"].is_object() {
        let head = v["head"].take();
        let back = v["back"].as_array_mut().map(std::mem::take).unwrap_or_default();
        let head_meta = v["meta"].take();
        return Some(History { head, head_meta, back });
    }
    from_v1(&v)
}

/// Adds a save (nothing when it's the same as the last); keeps the last `KEEP`.
pub fn record(dir: &Path, id: &str, g: &Value) -> Result<(), String> {
    record_at(dir, id, g, now_ms())
}

pub fn record_at(dir: &Path, id: &str, g: &Value, at: u64) -> Result<(), String> {
    let h = match read(dir, id) {
        None => History { head: g.clone(), head_meta: meta(at, g, (0, 0, 0)), back: vec![] },
        Some(mut h) => {
            if &h.head == g {
                return Ok(());
            }
            // the save before becomes the newest kept older one: whole every tenth, else the
            // change that takes this save back to it
            let since_full = h.back.iter().rev().take_while(|e| e.get("full").is_none()).count();
            let mut e = h.head_meta.clone();
            if since_full + 1 >= FULL_EVERY {
                e["full"] = h.head.clone();
            } else {
                e["diff"] = diff(g, &h.head);
            }
            let c = counts(&h.head, g);
            h.back.push(e);
            h.head_meta = meta(at, g, c);
            h.head = g.clone();
            // past 50, the oldest go (nothing to fold: each kept one is reached from a newer one)
            while h.back.len() + 1 > KEEP {
                h.back.remove(0);
            }
            h
        }
    };
    let mut back_text: Vec<String> = h.back.iter().map(|e| serde_json::to_string(e).unwrap_or_default()).collect();
    let head_text = serde_json::to_string(&h.head).map_err(|e| e.to_string())?;
    let meta_text = h.head_meta.to_string();
    // past the size cap, the oldest go too (always the newest few)
    let mut total: usize = head_text.len() + meta_text.len() + back_text.iter().map(|t| t.len() + 1).sum::<usize>();
    while total > MAX_BYTES && back_text.len() > 1 {
        total -= back_text.remove(0).len() + 1;
    }
    // (an oldest one kept as a change needs a newer whole one or the head: it always has one)
    let text = format!("{{\"version\":2,\"meta\":{meta_text},\"head\":{head_text},\"back\":[{}]}}", back_text.join(","));
    crate::sys::write_atomic(&file(dir, id), text.as_bytes()).map_err(|e| format!("Couldn't keep the version history ({e})."))
}

/// For the panel: each version (newest first) with what changed from the one before. Read from
/// the counts kept with each; nothing is rebuilt.
pub fn list(dir: &Path, id: &str) -> Value {
    let Some(h) = read(dir, id) else {
        return json!({"versions": [], "keep": KEEP});
    };
    let mut out = vec![];
    for (i, e) in h.back.iter().chain(std::iter::once(&h.head_meta)).enumerate() {
        let n = |k: &str| e[k].as_u64().unwrap_or(0);
        // (the oldest kept has nothing before it)
        let (added, removed, changed) = if i == 0 { (0, 0, 0) } else { (n("added"), n("removed"), n("changed")) };
        out.push(json!({"n": i, "at": n("at"), "nodes": n("nodes"), "added": added, "removed": removed, "changed": changed}));
    }
    out.reverse();
    json!({"versions": out, "keep": KEEP})
}

/// One kept save, whole (0 the oldest): from the nearest whole one after it, back.
pub fn get(dir: &Path, id: &str, n: usize) -> Option<Value> {
    let h = read(dir, id)?;
    let len = h.back.len() + 1;
    if n + 1 == len {
        return Some(h.head);
    }
    let back = h.back.get(n..)?;
    let start = back.iter().position(|e| e.get("full").is_some());
    let (mut cur, from) = match start {
        Some(j) => (back[j]["full"].clone(), j),
        None => (h.head, back.len()),
    };
    for e in back[..from].iter().rev() {
        cur = apply(&cur, e.get("diff")?);
    }
    Some(cur)
}

/// The kept versions, oldest first: (when, the graph). (Tests; the panel uses `list` and `get`.)
#[cfg(test)]
pub fn versions(dir: &Path, id: &str) -> Vec<(u64, Value)> {
    let Some(h) = read(dir, id) else { return vec![] };
    let len = h.back.len() + 1;
    let ats: Vec<u64> = h.back.iter().chain(std::iter::once(&h.head_meta)).map(|e| e["at"].as_u64().unwrap_or(0)).collect();
    (0..len).filter_map(|n| get(dir, id, n).map(|g| (ats[n], g))).collect()
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

    #[test]
    fn old_saves_come_back_through_whole_ones_and_changes() {
        let dir = std::env::temp_dir().join(format!("ls-studio-history3-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        delete(&dir, "h");
        let gs: Vec<Value> = (0..37u64).map(|i| g(json!((0..=i % 5).map(|k| json!({"id": format!("n{k}"), "type": "x", "params": {"i": i}})).collect::<Vec<_>>()))).collect();
        for (i, v) in gs.iter().enumerate() {
            record_at(&dir, "h", v, i as u64).unwrap();
        }
        for (n, v) in gs.iter().enumerate() {
            assert_eq!(&get(&dir, "h", n).unwrap(), v, "version {n}");
        }
        let l = list(&dir, "h");
        // version 6 (i % 5 == 1) has one node more than version 5 and both of them changed... counted
        let v6 = &l["versions"].as_array().unwrap().iter().find(|x| x["n"] == 6).unwrap().clone();
        assert_eq!((v6["added"].as_u64(), v6["removed"].as_u64(), v6["changed"].as_u64(), v6["nodes"].as_u64()), (Some(1), Some(0), Some(1), Some(2)));
        let v5 = &l["versions"].as_array().unwrap().iter().find(|x| x["n"] == 5).unwrap().clone();
        assert_eq!((v5["added"].as_u64(), v5["removed"].as_u64()), (Some(0), Some(4)));
        delete(&dir, "h");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_version_one_file_is_read_and_kept_going() {
        let dir = std::env::temp_dir().join(format!("ls-studio-history4-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let gs: Vec<Value> = (0..4u64).map(|i| g(json!([{"id": "a", "type": "x", "params": {"n": i}}]))).collect();
        let steps: Vec<Value> = (1..4).map(|i| json!({"at": i, "diff": diff(&gs[i - 1], &gs[i])})).collect();
        std::fs::write(file(&dir, "h"), json!({"version": 1, "at": 0, "base": gs[0], "steps": steps}).to_string()).unwrap();
        assert_eq!(versions(&dir, "h").iter().map(|x| x.1.clone()).collect::<Vec<_>>(), gs);
        let next = g(json!([{"id": "a", "type": "x", "params": {"n": 9}}]));
        record_at(&dir, "h", &next, 9).unwrap();
        let v = versions(&dir, "h");
        assert_eq!(v.len(), 5);
        assert_eq!(v[0].1, gs[0]);
        assert_eq!(v[4], (9, next));
        delete(&dir, "h");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_big_history_drops_its_oldest_past_the_size_cap() {
        let dir = std::env::temp_dir().join(format!("ls-studio-history5-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        delete(&dir, "h");
        // ~400 KB a save, all of it changed each time
        let big = |k: u64| g(json!((0..2000).map(|i| json!({"id": format!("n{i}"), "type": "x", "pos": {"x": k, "y": i}, "params": {"t": "a little text so the node has some size to it"}})).collect::<Vec<_>>()));
        for k in 0..30 {
            record_at(&dir, "h", &big(k), k).unwrap();
        }
        let size = std::fs::metadata(file(&dir, "h")).unwrap().len() as usize;
        assert!(size <= MAX_BYTES, "{size}");
        let l = list(&dir, "h");
        let kept = l["versions"].as_array().unwrap().len();
        assert!(kept > 2 && kept < 30, "{kept}");
        assert_eq!(get(&dir, "h", 0).unwrap(), big(30 - kept as u64));
        delete(&dir, "h");
        let _ = std::fs::remove_dir_all(&dir);
    }

}

#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;

    /// `cargo test -j 6 history_bench -- --ignored --nocapture`: a 367-node graph saved 60 times with
    /// every node moved each time (Tidy up), then the list and the oldest version (StudioPerfAudit #7).
    #[test]
    #[ignore]
    fn history_bench() {
        for n in [367usize, 952] {
            let dir = std::env::temp_dir().join(format!("ls-history-bench-{}-{n}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            delete(&dir, "b");
            let graph = |k: usize| {
                let nodes: Vec<Value> = (0..n).map(|i| json!({"id": format!("n{i}"), "type": "player.give_weapon", "pos": {"x": i * 300 + k, "y": (i % 7) * 200}, "params": {"weapon": "weapon_ak47", "ammo": 90, "note": "a little text so a node is node-sized"}})).collect();
                let edges: Vec<Value> = (1..n).map(|i| json!({"id": format!("e{i}"), "from": {"node": format!("n{}", i - 1), "pin": "then"}, "to": {"node": format!("n{i}"), "pin": "exec"}})).collect();
                json!({"schema": "loner-strike/graph", "version": 3, "mode": {"id": "b", "name": "B"}, "variables": [], "nodes": nodes, "edges": edges, "comments": []})
            };
            let mut saves = vec![];
            for k in 0..60 {
                let g = graph(k);
                let t = Instant::now();
                record(&dir, "b", &g).unwrap();
                saves.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            let t = Instant::now();
            let l = list(&dir, "b");
            let list_ms = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let old = get(&dir, "b", 0).unwrap();
            let get_ms = t.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(old["nodes"][0]["pos"]["x"], json!(60 - l["versions"].as_array().unwrap().len()));
            let size = std::fs::metadata(file(&dir, "b")).unwrap().len();
            println!("{n} nodes: first save {:.1} ms, save 50+ {:.1} ms (last), list {list_ms:.1} ms, oldest {get_ms:.1} ms, kept {}, file {} KB", saves[0], saves[59], l["versions"].as_array().unwrap().len(), size / 1024);
            delete(&dir, "b");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
