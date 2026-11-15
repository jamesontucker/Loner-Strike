//! Files as values (Studio types, the asset track, X-fx): Model, Sprite and Sound.
//!
//! A file value is an index into one constant table of every file the mode's values can name
//! (`g_lsgAsset`, 0 = none). The set is closed: no text ever becomes a file (graph.rs has no
//! text -> file conversion), so every file a value can hold is a literal somewhere in the graph:
//! a Model / Sprite / Sound node, a variable's or a Function local's start value, a Set Variable's
//! value. All of them are loaded in `plugin_precache` (a late load would stop the server), each
//! through the guarded loaders (`lsg_pre_model` / `lsg_pre_sound`: a missing file is skipped and
//! logged), and `lsg_asset_ok(i)` says whether one loaded.
//!
//! Kept out of gen.rs like fx.rs: gen.rs calls `Scope::new` (once per plugin), `lit` (a start
//! value), `is_pure`/`pure`, `blocks`, `used`/`precache`; graph.rs calls `problems`,
//! `valid_asset`.

use super::{pawn_str, Gen, Out};
use crate::tools::studio::graph::{valid_path, Graph, Problem, View};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::BTreeSet;

/// The file types (graph.rs BASE_TYPES): a value of one is an index into `g_lsgAsset`.
pub(crate) const TYPES: [&str; 3] = ["model", "sprite", "sound"];
/// The nodes: a constant file of each kind, and a file's name as text.
pub(crate) const PURE: [&str; 4] = ["asset.model", "asset.sprite", "asset.sound", "asset.file_name"];

/// Past these many files of its own a mode gets a warning (the game's 512 models and 512 sounds
/// are shared with the map, its weapons and players and every other plugin), past `CAP` of a
/// kind an error.
pub(crate) const WARN_MODELS: usize = 64;
pub(crate) const WARN_SOUNDS: usize = 96;
pub(crate) const CAP: usize = 128;

pub(crate) fn is_type(t: &str) -> bool {
    TYPES.contains(&t)
}

/// Is `s` a file of this kind, as values hold it: models/….mdl, sprites/….spr, a sound from
/// inside sound/ (….wav); "visual" is a model or a sprite. Empty is "none".
pub fn valid_asset(kind: &str, s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if !valid_path(s) {
        return false;
    }
    let l = s.to_ascii_lowercase();
    match kind {
        "model" => l.starts_with("models/") && l.ends_with(".mdl") && l.len() > 11,
        "sprite" => l.starts_with("sprites/") && l.ends_with(".spr") && l.len() > 12,
        "sound" => !l.starts_with("sound/") && l.ends_with(".wav") && l.len() > 4,
        "visual" => valid_asset("model", s) || valid_asset("sprite", s),
        _ => false,
    }
}

/// What the Problems message says a file of this kind must be.
pub fn asset_rule(kind: &str) -> &'static str {
    match kind {
        "model" => "a model file (models/….mdl)",
        "sprite" => "a sprite file (sprites/….spr)",
        "sound" => "a sound file from inside sound/ (….wav, without sound/)",
        _ => "a model (models/….mdl) or a sprite (sprites/….spr)",
    }
}

/// The type a node param of an asset kind holds.
fn param_type(kind: &str) -> Option<&'static str> {
    match kind {
        "model" => Some("model"),
        "sprite" => Some("sprite"),
        "sound" => Some("sound"),
        _ => None,
    }
}

/// Every file a value can name, sorted (index 1 up; 0 is none).
pub(crate) fn table(g: &Graph) -> Vec<String> {
    let mut out = BTreeSet::new();
    let mut add = |ty: &str, v: &Value| {
        if let Some(s) = v.as_str() {
            if is_type(ty) && !s.is_empty() && valid_asset(ty, s) {
                out.insert(s.to_string());
            }
        }
    };
    // (start values: the mode's variables and every Function's locals)
    for x in g.variables.iter().chain(g.functions.iter().flat_map(|f| f.locals.iter())) {
        add(&x.ty, &x.default);
    }
    for n in &g.nodes {
        match n.kind.as_str() {
            "asset.model" => add("model", n.params.get("file").unwrap_or(&Value::Null)),
            "asset.sprite" => add("sprite", n.params.get("file").unwrap_or(&Value::Null)),
            "asset.sound" => add("sound", n.params.get("file").unwrap_or(&Value::Null)),
            // (Set Variable's value, for a file variable)
            "var.set" => {
                let ty = n.params.get("var").and_then(Value::as_str).and_then(|id| g.variables.iter().chain(g.functions.iter().flat_map(|f| f.locals.iter())).find(|x| x.id == id)).map(|x| x.ty.clone());
                if let (Some(ty), Some(v)) = (ty, n.params.get("value")) {
                    add(&ty, v);
                }
            }
            _ => {}
        }
    }
    out.into_iter().collect()
}

thread_local! {
    /// The table of the plugin being written (start values are written where there's no Gen).
    static TABLE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// While a plugin is written: its file table, for `lit`.
pub(crate) struct Scope;
impl Scope {
    pub(crate) fn new(g: &Graph) -> Scope {
        TABLE.with(|t| *t.borrow_mut() = table(g));
        Scope
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        TABLE.with(|t| t.borrow_mut().clear());
    }
}

/// A file's index in the table (0: none, or not a file of the table).
pub(crate) fn index_of(s: &str) -> usize {
    TABLE.with(|t| t.borrow().iter().position(|x| x == s).map_or(0, |i| i + 1))
}

/// A start value or Set Variable value of a file type: its index, a constant.
pub(crate) fn lit(v: Option<&Value>) -> String {
    index_of(v.and_then(Value::as_str).unwrap_or("")).to_string()
}

pub(crate) fn used(g: &Graph) -> bool {
    !table(g).is_empty() || g.nodes.iter().any(|n| n.kind == "asset.file_name")
}

/// A pure node's output.
pub(super) fn pure(gen: &Gen, node: &str, _pin: &str) -> String {
    let (n, def) = gen.node(node);
    match def.kind.as_str() {
        "asset.file_name" => format!("g_lsgAsset[lsg_asset_i({})]", gen.need(node, "file", "model")),
        _ => index_of(gen.text(n, def, "file")).to_string(),
    }
}

pub(crate) fn is_pure(k: &str) -> bool {
    PURE.contains(&k)
}

/// The table and its helpers (with the other globals, before plugin_precache).
pub(super) fn blocks(g: &Graph, o: &mut Out) {
    if !used(g) {
        return;
    }
    let t = table(g);
    let mut lines = vec!["\"\"".to_string()];
    lines.extend(t.iter().map(|s| pawn_str(s)));
    o.line(format!(
        "// The files this mode's Model, Sprite and Sound values can name (0: none), all loaded with the map\nstock const g_lsgAsset[][] = {{ {} }}\nstock bool:g_lsgAssetOk[{}]\n\n// a value as an index of the table (0 when it isn't one)\nstock lsg_asset_i(i)\n\treturn i > 0 && i < sizeof g_lsgAsset ? i : 0\n\n// was that file loaded with the map?\nstock bool:lsg_asset_ok(i)\n\treturn i > 0 && i < sizeof g_lsgAsset && g_lsgAssetOk[i]\n\nstock bool:lsg_pre_asset(const p[])\n{{\n\tnew n = strlen(p)\n\tif (n > 4 && equali(p[n - 4], \".wav\"))\n\t\tlsg_pre_sound(p)\n\telse\n\t\tlsg_pre_model(p)\n\treturn lsg_have(p)\n}}\n",
        lines.join(", "),
        t.len() + 1
    ));
}

/// plugin_precache's lines: every file of the table, through the guarded loaders.
pub(super) fn precache(g: &Graph) -> Vec<String> {
    if table(g).is_empty() {
        return vec![];
    }
    vec!["\t// the files the mode's Model, Sprite and Sound values name\n\tfor (new i = 1; i < sizeof g_lsgAsset; i++)\n\t\tg_lsgAssetOk[i] = lsg_pre_asset(g_lsgAsset[i])".into()]
}

/// Problems: the budget (files of its own a mode loads), a File Name Of fed something that
/// isn't a file, a file variable nothing sets and that starts empty.
pub(crate) fn problems(g: &Graph, v: &View, p: &mut Vec<Problem>) {
    let mut models = BTreeSet::new();
    let mut sounds = BTreeSet::new();
    let mut put = |s: &str| {
        let l = s.to_ascii_lowercase();
        if l.ends_with(".wav") {
            sounds.insert(l);
        } else if l.ends_with(".mdl") || l.ends_with(".spr") {
            models.insert(l);
        }
    };
    for s in table(g) {
        put(&s);
    }
    // (the files node settings name: path settings and the typed file settings)
    for n in &g.nodes {
        let Some(def) = v.defs.get(n.id.as_str()) else { continue };
        for prm in &def.params {
            if matches!(prm.kind.as_str(), "path" | "model" | "sprite" | "sound" | "visual") {
                if let Some(s) = n.params.get(&prm.id).and_then(Value::as_str).or(prm.default.as_str()) {
                    put(s);
                }
            }
        }
    }
    for (what, n, warn) in [("models and sprites", models.len(), WARN_MODELS), ("sounds", sounds.len(), WARN_SOUNDS)] {
        if n > CAP {
            p.push(prob("error", None, None, format!("Too many different {what} ({n}): the game would stop loading the map. Use at most {CAP}, or reuse them.")));
        } else if n > warn {
            p.push(prob("warning", None, None, format!("This mode loads {n} {what}. The game stops at 512 with the map's own and other plugins'; keep a mode under {warn}.")));
        }
    }
    for n in &g.nodes {
        if n.kind != "asset.file_name" {
            continue;
        }
        if let Some((src, pin)) = v.data_source(&n.id, "file") {
            let t = v.out_type(src, pin).unwrap_or_default();
            if !is_type(&t) {
                p.push(prob("error", Some(&n.id), Some("file"), format!("File Name Of takes a Model, Sprite or Sound; this is {}.", v.type_label(&t))));
            }
        }
    }
    // a file variable that starts empty and that nothing sets: whatever uses it does nothing
    for x in g.variables.iter().chain(g.functions.iter().flat_map(|f| f.locals.iter())) {
        if !is_type(&x.ty) || x.default.as_str().is_some_and(|s| !s.is_empty()) {
            continue;
        }
        let set = g.nodes.iter().any(|n| n.params.get("var").and_then(Value::as_str) == Some(x.id.as_str()) && n.kind != "var.get");
        if !set {
            p.push(prob("warning", None, None, format!("\u{201c}{}\u{201d} is empty and nothing sets it: anything that uses it does nothing.", x.name)));
        }
    }
}

fn prob(level: &'static str, node: Option<&str>, pin: Option<&str>, message: String) -> Problem {
    Problem { level, node: node.map(String::from), pin: pin.map(String::from), edge: None, message }
}

/// A node param of a file kind: its value check (graph.rs's param match calls this).
pub(crate) fn check_param(kind: &str, label: &str, v: &Value) -> Option<Result<(), String>> {
    let ty = param_type(kind).or(if kind == "visual" { Some("visual") } else { None })?;
    Some(match v.as_str() {
        Some(s) if !s.is_empty() && valid_asset(ty, s) => Ok(()),
        _ => Err(format!("{label} must be {}, up to 64 letters, digits, _ - . or /.", asset_rule(ty))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::studio::graph::{parse, validate};
    use crate::tools::studio::gen::generate;

    const EVERY: &str = include_str!("../testdata/every_node_assets.graph.json");

    fn with(f: impl Fn(&mut Value)) -> crate::tools::studio::graph::Graph {
        let mut v: Value = serde_json::from_str(EVERY).unwrap();
        f(&mut v);
        parse(&v.to_string()).unwrap()
    }

    /// Every file a value can hold is in the table and loaded with the map: start values (a
    /// Function's local only it holds too), the Model / Sprite / Sound nodes, Set Variable.
    #[test]
    fn every_file_a_value_holds_is_loaded() {
        let g = parse(EVERY).unwrap();
        let p = validate(&g);
        assert!(!p.iter().any(|x| x.level == "error"), "{p:#?}");
        let src = generate(&g, false).source;
        assert!(src.contains("stock const g_lsgAsset[][] = { \"\", \"ambience/thunder_clap.wav\", \"items/gunpickup2.wav\", \"models/w_c4.mdl\", \"models/w_weaponbox.mdl\", \"sprites/arrow1.spr\" }"), "{src}");
        let pre = &src[src.find("public plugin_precache()").unwrap()..];
        let pre = &pre[..pre.find("\n}").unwrap()];
        assert!(pre.contains("g_lsgAssetOk[i] = lsg_pre_asset(g_lsgAsset[i])"), "{pre}");
        // start values are indexes: Taunt is 2, the Function's Box 4, Set Variable's sprite 5
        assert!(src.contains("stock g_v0 = 2 "), "{src}");
        assert!(src.contains(" = 4"), "{src}");
        assert!(src.contains("g_v2 = 5"), "{src}");
        // File Name Of: the table's row (0 when it isn't one)
        assert!(src.contains("g_lsgAsset[lsg_asset_i("), "{src}");
    }

    /// A mode with no file values gets none of it (old plugins stay the same).
    #[test]
    fn no_file_values_no_table() {
        let g = parse(include_str!("../testdata/every_node_fx.graph.json")).unwrap();
        let src = generate(&g, false).source;
        assert!(!src.contains("g_lsgAsset"), "{src}");
    }

    #[test]
    fn bad_start_values_and_budgets_are_problems() {
        let bad = |v: &str| {
            let g = with(|x| x["variables"][0]["default"] = Value::String(v.into()));
            validate(&g).iter().any(|p| p.level == "error")
        };
        assert!(bad("models/w_c4.mdl"), "a model in a Sound");
        assert!(bad("sound/items/a.wav"), "sound/ is left out");
        assert!(bad("../x.wav"));
        assert!(!bad("items/ammopickup1.wav"));
        // 65 different models: a warning; 129: an error
        let many = |n: usize| {
            with(|x| {
                let vars = x["variables"].as_array_mut().unwrap();
                for i in 0..n {
                    vars.push(serde_json::json!({"id": format!("m{i}"), "name": format!("M{i}"), "type": "model", "scope": "mode", "default": format!("models/m{i}.mdl")}));
                }
            })
        };
        let p = validate(&many(65));
        assert!(p.iter().any(|x| x.level == "warning" && x.message.contains("models and sprites")), "{p:#?}");
        let p = validate(&many(130));
        assert!(p.iter().any(|x| x.level == "error" && x.message.contains("Too many different models")), "{p:#?}");
        // File Name Of fed a number
        let g = with(|x| {
            x["nodes"].as_array_mut().unwrap().push(serde_json::json!({"id": "num", "type": "math.number", "pos": {"x": 0, "y": 0}, "params": {}}));
            for e in x["edges"].as_array_mut().unwrap() {
                if e["to"]["node"] == "nm" {
                    e["from"] = serde_json::json!({"node": "num", "pin": "out"});
                }
            }
        });
        assert!(validate(&g).iter().any(|p| p.level == "error" && p.message.contains("File Name Of takes")), "{:#?}", validate(&g));
        // an empty file variable nothing sets
        let g = with(|x| x["variables"][0]["default"] = Value::String(String::new()));
        assert!(validate(&g).iter().any(|p| p.message.contains("\u{201c}Taunt\u{201d} is empty")));
    }

    /// The same list the Studio checks (scripts/studio.test.ts).
    #[test]
    fn asset_values_are_checked_by_kind() {
        let v: Value = serde_json::from_str(include_str!("../testdata/asset_values.json")).unwrap();
        for c in v["cases"].as_array().unwrap() {
            let (k, s, fits) = (c[0].as_str().unwrap(), c[1].as_str().unwrap(), c[2].as_bool().unwrap());
            assert_eq!(valid_asset(k, s), fits, "{k} {s}");
        }
    }
}
