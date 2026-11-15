//! W1's utility nodes: Problems for bad settings and for heavy work in a fast On Tick.

use super::*;

/// The pure nodes feeding a node's inputs (and theirs), so a flow's real work can be seen.
fn feeders<'a>(g: &'a Graph, v: &View<'a>, start: &BTreeSet<&'a str>) -> BTreeSet<&'a str> {
    let mut seen: BTreeSet<&'a str> = BTreeSet::new();
    let mut todo: Vec<&'a str> = start.iter().copied().collect();
    while let Some(n) = todo.pop() {
        for e in g.edges.iter().filter(|e| e.to.node == n) {
            let from = e.from.node.as_str();
            if v.defs.get(from).is_some_and(|d| d.pure) && seen.insert(from) {
                todo.push(from);
            }
        }
    }
    seen
}

pub(super) fn problems(g: &Graph, v: &View) -> Vec<Problem> {
    let mut p = Vec::new();
    let kind = |id: &str| v.defs.get(id).map(|d| d.kind.as_str()).unwrap_or("");
    let wired = |id: &str, pin: &str| g.edges.iter().any(|e| e.to.node == id && e.to.pin == pin);
    let num = |n: &Node, id: &str| v.defs.get(n.id.as_str()).and_then(|d| param(n, d, id)).and_then(Value::as_f64).unwrap_or(0.0);
    let text = |n: &Node, id: &str| v.defs.get(n.id.as_str()).and_then(|d| param(n, d, id)).and_then(Value::as_str).unwrap_or("").to_string();
    for n in &g.nodes {
        let id = n.id.as_str();
        match kind(id) {
            "spatial.players_in_radius" | "spatial.nearest_player" | "spatial.entities_in_radius" | "spatial.nearest_entity" => {
                match (wired(id, "pos"), wired(id, "to")) {
                    (false, false) => p.push(Problem::err(Some(id), "Wire in a point or an entity to search around.").pin("pos")),
                    (true, true) => p.push(Problem::warn(Some(id), "Both a point and an entity are wired in: the point is used (the entity is still left out of what's found).").pin("pos")),
                    _ => {}
                }
                if kind(id) == "spatial.nearest_entity" && text(n, "class").trim().is_empty() {
                    p.push(Problem::err(Some(id), "Pick the class of entity to look for."));
                }
            }
            "spatial.line_of_sight" => {
                for (e, pos, side) in [("a", "a_pos", "A"), ("b", "b_pos", "B")] {
                    if !wired(id, e) && !wired(id, pos) {
                        p.push(Problem::err(Some(id), format!("Wire in {side}: an entity or a point.")).pin(e));
                    }
                }
            }
            // (comfort limits are settings: raised a lot, a note, never a block)
            "debug.log_line" if num(n, "most") > 20.0 => {
                p.push(Problem::warn(Some(id), format!("Up to {} log lines a second from this node: the log file grows fast. 5 is usually plenty.", num(n, "most").round())));
            }
            "math.bitwise" if !wired(id, "b") && matches!(n.params.get("op").and_then(Value::as_str), Some("shl" | "shr")) && !(0.0..=31.0).contains(&num(n, "b")) => {
                p.push(Problem::warn(Some(id), "A shift of less than 0 or more than 31 bits: it's kept to 0..31.").pin("b"));
            }
            "random.weighted" => {
                if wired(id, "items") && !wired(id, "weights") {
                    p.push(Problem::err(Some(id), "Wire in the weights: Items only says what each index is.").pin("weights"));
                }
            }
            "math.map_range" => {
                let fixed = !wired(id, "in_min") && !wired(id, "in_max");
                if fixed && num(n, "in_min") == num(n, "in_max") {
                    p.push(Problem::warn(Some(id), "In Min and In Max are the same: every value comes out as Out Min.").pin("in_max"));
                }
            }
            "text.split" if !wired(id, "sep") && text(n, "sep").is_empty() => {
                p.push(Problem::warn(Some(id), "The separator is empty: the text comes out whole, as one piece.").pin("sep"));
            }
            "text.replace" if !wired(id, "find") && text(n, "find").is_empty() => {
                p.push(Problem::warn(Some(id), "Find is empty: nothing gets replaced.").pin("find"));
            }
            "text.substring" if !wired(id, "len") && num(n, "len") <= 0.0 => {
                p.push(Problem::warn(Some(id), "Length is 0: the result is always empty.").pin("len"));
            }
            "array.slice" if !wired(id, "count") && num(n, "count") <= 0.0 => {
                p.push(Problem::warn(Some(id), "Count is 0: the new array is always empty.").pin("count"));
            }
            "vec.random_in_radius" if !wired(id, "radius") && num(n, "radius") <= 0.0 => {
                p.push(Problem::warn(Some(id), "Radius is 0: the point is always the center.").pin("radius"));
            }
            "flow.multi_gate" => {
                let count = num(n, "count").round().clamp(1.0, 8.0);
                if num(n, "start") >= count {
                    p.push(Problem::warn(Some(id), format!("Start index is past the last output (Out {}): it starts at Out {}.", count as i64 - 1, count as i64 - 1)));
                }
            }
            "var.toggle" => match v.var_of(id) {
                None => p.push(Problem::err(Some(id), "Pick the yes/no variable to flip.")),
                Some(x) if x.ty != "bool" => p.push(Problem::err(Some(id), format!("\u{201c}{}\u{201d} isn't a yes/no variable: only those can be flipped.", x.name))),
                _ => {}
            },
            "event.on_tick" => {
                let every = num(n, "interval").max(0.0);
                if every >= 0.05 {
                    continue;
                }
                let body = flow_from(v, id, &["then"]);
                let all: BTreeSet<&str> = body.iter().copied().chain(feeders(g, v, &body)).collect();
                let heavy = all.iter().find(|m| {
                    let k = kind(m);
                    k.contains("for_each") || k == "ent.find_all" || k.ends_with("_in_radius") || k == "spatial.line_of_sight" || k == "aim.point"
                });
                if let Some(m) = heavy {
                    let what = v.defs.get(*m).map(|d| d.title.clone()).unwrap_or_default();
                    p.push(Problem::warn(Some(id), format!("This On Tick runs {} with {what} in it: that work every few milliseconds can slow the server. Use 0.1 s (ten times a second) or more if it doesn't need to be that fast.", if every <= 0.0 { "every server frame (100 times a second or more)".to_string() } else { format!("{} times a second", (1.0 / every).round()) })));
                }
            }
            _ => {}
        }
    }
    p
}
