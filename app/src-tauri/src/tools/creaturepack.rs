//! Shared creatures: the custom creature types a map, mode or prefab uses travel with it.
//!
//! - Detection: which creature types a Studio graph, a map's (or prefab's) entities and a
//!   waves.ini name, and the `base =` chain behind each.
//! - Packing: on publish each custom type gets a name that says who made it
//!   (`<author>.<type>`), its creatures.ini sections are written to the package's
//!   `creatures/creatures.ini` (the community list checks it: services/serverlist formats.py
//!   `creatures_ini_problem`) with its custom models and sounds at their game paths, and the
//!   package's references are renamed to match. Built-in types are never renamed or packed.
//! - Installing: each package's types go in creatures.ini inside a block the app manages
//!   (`; >>> from <package id>` ... `; <<< end <package id>`); the player's own sections are
//!   never touched. A type two packages share is in both blocks (the game merges sections by
//!   name), so it stays until both are gone; a model or sound one block wrote passes to another
//!   block that needs it before it's removed. The bot brain reads `;` lines as comments.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::creatures;

/// The pack file in a package (map, mode, prefab or a standalone creature).
pub const PACK_FILE: &str = "creatures/creatures.ini";
const BLOCK_START: &str = "; >>> from ";
const BLOCK_END: &str = "; <<< end ";
const TOOL: &str = "Shared creatures";

// ---- names ------------------------------------------------------------------------------------

pub fn is_builtin(name: &str) -> bool {
    creatures::builtin(name).is_some()
}

/// Names a mode or map may use that aren't types to pack: the wave spawner's "all", and Last
/// Stand's round-table names a stand-in plays until the game has them.
fn not_a_type(name: &str) -> bool {
    name.is_empty() || name == "all" || ["controller", "stukabat"].contains(&name)
}

/// The type part: small letters, digits and _ (1-29), not only digits.
fn plain_part(s: &str) -> bool {
    !s.is_empty() && s.len() <= 29 && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') && !s.bytes().all(|c| c.is_ascii_digit())
}

/// The author part, as the community list checks it: `[a-z0-9][a-z0-9_-]{0,23}`.
fn author_part(s: &str) -> bool {
    !s.is_empty() && s.len() <= 24 && s.as_bytes()[0].is_ascii_alphanumeric() && !s.as_bytes()[0].is_ascii_uppercase() && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// A shared type's name: `<author>.<type>`, one dot, small letters, digits and _, 31 at most.
pub fn is_shared_name(n: &str) -> bool {
    shared_name_problem(n).is_none()
}

pub fn shared_name_problem(n: &str) -> Option<String> {
    let Some((a, t)) = n.split_once('.') else { return Some(format!("{n} isn't named for who made it (author.type).")) };
    if n.len() > 31 || !author_part(a) || !plain_part(t) {
        return Some(format!("{n} isn't a shared creature name (author.type: small letters, digits and _, 31 at most)."));
    }
    None
}

/// The author part of shared names, from the author's name as the package gives it.
pub fn author_slug(author: &str) -> String {
    let mut s = String::new();
    for c in author.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('_') && !s.is_empty() {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() || s.bytes().all(|c| c.is_ascii_digit()) {
        format!("a{s}").chars().take(20).collect()
    } else {
        s.chars().take(20).collect::<String>().trim_end_matches('_').to_string()
    }
}

/// The namespace for someone's shared creatures when the community list hasn't assigned one (no
/// member id to ask with, or an unlisted creation): their name's slug, the list's first choice.
pub fn namespace_for(author: &str) -> String {
    author_slug(author)
}

/// The namespace the community list assigned to a GameBanana member (services/serverlist:
/// `GET /community/namespace?member=<id>&name=<name>`; the first member to claim a slug keeps it,
/// another gets it with a suffix from their id).
pub fn fetch_namespace(member: i64, name: &str) -> Result<String, String> {
    if member <= 0 {
        return Ok(namespace_for(name));
    }
    let r = crate::sys::http()
        .get(NAMESPACE_URL)
        .query(&[("member", member.to_string()), ("name", name.to_string())])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .map_err(|_| "Couldn't reach the community list to get your creature names. Check your internet connection.".to_string())?;
    if r.status().as_u16() == 429 {
        return Err("The community list is busy. Try again in a minute.".into());
    }
    let v: Value = r.json().map_err(|_| "The community list's answer couldn't be read.".to_string())?;
    let ns = v["namespace"].as_str().unwrap_or("").to_string();
    if !author_part(&ns) {
        return Err("The community list didn't give a name for your creatures. Try again later.".into());
    }
    Ok(ns)
}

pub const NAMESPACE_URL: &str = "https://list.lonerstrike.com/community/namespace";

/// An index entry's namespace, cleaned ("" when it isn't one).
pub fn clean_namespace(ns: &str) -> String {
    if author_part(ns) { ns.to_string() } else { String::new() }
}

/// `<author>.<type>`. The author part is always the whole namespace (installs check it against
/// who uploaded the package); a type name too long to fit is cut instead.
pub fn shared_name(slug: &str, name: &str) -> Result<String, String> {
    if is_shared_name(name) {
        return Ok(name.to_string());
    }
    let room = 31usize.saturating_sub(slug.len() + 1);
    let short: String = name.chars().take(room).collect::<String>().trim_end_matches('_').to_string();
    if short.is_empty() || short.bytes().all(|c| c.is_ascii_digit()) {
        return Err(format!("{name} can't be shared under your name: rename it in the Creature Studio."));
    }
    let n = format!("{slug}.{short}");
    match shared_name_problem(&n) {
        None => Ok(n),
        Some(e) => Err(e),
    }
}

// ---- detection ----------------------------------------------------------------------------------

fn add(out: &mut BTreeSet<String>, raw: &str) {
    let t = raw.trim().to_ascii_lowercase();
    if !t.is_empty() && t.len() <= 31 && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'-') && !t.bytes().all(|c| c.is_ascii_digit()) {
        out.insert(t);
    }
}

/// The Studio params that name creature types: (node type, param, default when unset).
const GRAPH_PARAMS: &[(&str, &str, &str)] = &[
    ("creature.spawn", "kind", "zombie"),
    ("turret.setup", "type", "turret"),
    ("creature.for_each", "type", ""),
    ("creature.of_type", "type", ""),
    ("creature.nearest", "type", ""),
];

/// comp.create's `keys` ("type=x" for a creature, "types=a b" for a wave spawner): the type
/// words, with where each sits in the text.
fn comp_key_types(kind: &str, keys: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (class, key, list) in fgd_type_keys() {
        if class != kind {
            continue;
        }
        let pat = format!("{key}=");
        let mut from = 0;
        while let Some(i) = keys[from..].find(&pat) {
            let at = from + i;
            from = at + pat.len();
            if at > 0 && !matches!(keys.as_bytes()[at - 1], b' ' | b';' | b',' | b'|') {
                continue;
            }
            // a list runs to the next key (a word with '=') or a separator
            let rest = &keys[from..];
            let end = rest.find([';', ',', '|']).unwrap_or(rest.len());
            let mut pos = from;
            for (n, w) in rest[..end].split(' ').enumerate() {
                if w.contains('=') || (!list && n > 0) {
                    break;
                }
                if !w.is_empty() {
                    out.push((pos, pos + w.len()));
                }
                pos += w.len() + 1;
            }
        }
    }
    out
}

fn node_types(kind: &str, params: &BTreeMap<String, Value>, out: &mut BTreeSet<String>) {
    for (k, p, def) in GRAPH_PARAMS {
        if *k == kind {
            add(out, params.get(*p).and_then(Value::as_str).unwrap_or(def));
        }
    }
    if kind == "comp.create" {
        let comp = params.get("kind").and_then(Value::as_str).unwrap_or("");
        let keys = params.get("keys").and_then(Value::as_str).unwrap_or("");
        for (a, b) in comp_key_types(comp, keys) {
            add(out, &keys[a..b]);
        }
    }
}

/// Every creature type a Studio graph names (its Functions' and Macros' nodes too).
pub fn types_in_graph(g: &super::studio::graph::Graph) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in &g.nodes {
        node_types(&n.kind, &n.params, &mut out);
    }
    out
}

/// The same on a graph's JSON (a package's graph file).
pub fn types_in_graph_json(v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in v["nodes"].as_array().into_iter().flatten() {
        let params: BTreeMap<String, Value> = n["params"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
        node_types(n["type"].as_str().unwrap_or(""), &params, &mut out);
    }
    out
}

/// Renames creature types in a graph's JSON (node params and comp.create keys).
pub fn rewrite_graph_json(v: &mut Value, renames: &BTreeMap<String, String>) {
    for n in v["nodes"].as_array_mut().into_iter().flatten() {
        let kind = n["type"].as_str().unwrap_or("").to_string();
        for (k, p, def) in GRAPH_PARAMS {
            if *k != kind {
                continue;
            }
            let cur = n["params"][*p].as_str().unwrap_or(def).trim().to_ascii_lowercase();
            if let Some(to) = renames.get(&cur) {
                n["params"][*p] = Value::String(to.clone());
            }
        }
        if kind == "comp.create" {
            let comp = n["params"]["kind"].as_str().unwrap_or("").to_string();
            let keys = n["params"]["keys"].as_str().unwrap_or("").to_string();
            let spans = comp_key_types(&comp, &keys);
            if spans.is_empty() {
                continue;
            }
            let mut s = keys.clone();
            for (a, b) in spans.into_iter().rev() {
                if let Some(to) = renames.get(&keys[a..b].to_ascii_lowercase()) {
                    s.replace_range(a..b, to);
                }
            }
            n["params"]["keys"] = Value::String(s);
        }
    }
}

/// Entity keys that take a creature type, from lonerstrike.fgd (a key whose help mentions
/// creatures.ini): (class, key, a list of types).
fn fgd_type_keys() -> &'static Vec<(String, String, bool)> {
    static K: std::sync::OnceLock<Vec<(String, String, bool)>> = std::sync::OnceLock::new();
    K.get_or_init(|| {
        let mut out: Vec<(String, String, bool)> = Vec::new();
        for fgd in crate::editor::fgd::EXTRA_FGDS {
            let mut class = String::new();
            for line in fgd.lines() {
                let t = line.trim();
                if t.starts_with('@') {
                    class = t.split('=').nth(1).and_then(|r| r.split(':').next()).map(|c| c.trim().to_string()).unwrap_or_default();
                    continue;
                }
                if class.is_empty() || !t.contains("creatures.ini") {
                    continue;
                }
                let Some(open) = t.find('(') else { continue };
                let key = t[..open].trim().to_string();
                if !key.is_empty() && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
                    let list = key.ends_with('s');
                    out.push((class.clone(), key, list));
                }
            }
        }
        for (c, k, l) in [("ls_creature", "type", false), ("ls_wave_spawner", "types", true)] {
            if !out.iter().any(|(a, b, _)| a == c && b == k) {
                out.push((c.into(), k.into(), l));
            }
        }
        out
    })
}

/// A key/value pair at depth 1 of an entity text (a BSP's entity lump, or a .map with brushes):
/// (entity index, key, value, where the value sits).
fn entity_pairs(text: &str) -> Vec<(usize, String, String, (usize, usize))> {
    let b = text.as_bytes();
    let (mut depth, mut ent, mut i) = (0i32, 0usize, 0usize);
    let mut out = Vec::new();
    let mut pending: Option<(String, usize)> = None;
    while i < b.len() {
        match b[i] {
            b'{' => {
                depth += 1;
                pending = None;
                i += 1;
            }
            b'}' => {
                if depth == 1 {
                    ent += 1;
                }
                depth -= 1;
                pending = None;
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                let start = i + 1;
                let end = text[start..].find('"').map(|e| start + e).unwrap_or(b.len());
                let s = text[start..end].to_string();
                if depth == 1 {
                    match pending.take() {
                        None => pending = Some((s, start)),
                        Some((k, _)) => out.push((ent, k, s, (start, end))),
                    }
                }
                i = end + 1;
            }
            b'\n' => {
                pending = None;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// For each type-bearing value in an entity text: (where each type word sits).
fn entity_type_spans(text: &str) -> Vec<(usize, usize)> {
    let pairs = entity_pairs(text);
    let mut classes: BTreeMap<usize, String> = BTreeMap::new();
    for (e, k, v, _) in &pairs {
        if k == "classname" {
            classes.insert(*e, v.clone());
        }
    }
    let keys = fgd_type_keys();
    let mut out = Vec::new();
    for (e, k, v, (a, _)) in &pairs {
        let Some(class) = classes.get(e) else { continue };
        let Some((_, _, list)) = keys.iter().find(|(c, key, _)| c == class && key == k) else { continue };
        let mut pos = *a;
        for (n, w) in v.split(' ').enumerate() {
            if !w.is_empty() && (*list || n == 0) {
                out.push((pos, pos + w.len()));
            }
            pos += w.len() + 1;
        }
    }
    out
}

/// Every creature type a map's (or a prefab's .map) entities name.
pub fn types_in_entity_text(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (a, b) in entity_type_spans(text) {
        add(&mut out, &text[a..b]);
    }
    out
}

/// The same on parsed entities.
pub fn types_in_entities<'a>(ents: impl IntoIterator<Item = &'a crate::goldsrc::entities::Entity>) -> BTreeSet<String> {
    let keys = fgd_type_keys();
    let mut out = BTreeSet::new();
    for e in ents {
        let class = e.classname();
        for (c, k, list) in keys.iter().filter(|(c, _, _)| c == class) {
            let _ = c;
            if let Some(v) = e.get(k) {
                for (n, w) in v.split_whitespace().enumerate() {
                    if *list || n == 0 {
                        add(&mut out, w);
                    }
                }
            }
        }
    }
    out
}

/// Renames creature types in an entity text (only the type-bearing keys).
pub fn rewrite_entity_text(text: &str, renames: &BTreeMap<String, String>) -> String {
    let mut s = text.to_string();
    for (a, b) in entity_type_spans(text).into_iter().rev() {
        if let Some(to) = renames.get(&text[a..b].to_ascii_lowercase()) {
            s.replace_range(a..b, to);
        }
    }
    s
}

/// Every type a waves.ini's rows and bosses name.
pub fn types_in_waves(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for w in creatures::parse_waves(text) {
        add(&mut out, &w.boss);
        for r in &w.rows {
            for (t, n) in &r.mix {
                if *n > 0 {
                    add(&mut out, t);
                }
            }
        }
    }
    out
}

/// The `base =` chain behind a type in a creatures.ini: its base, that one's base, ... (the last
/// is a built-in, or a name the file doesn't have). Stops at a loop.
pub fn base_chain(ini: &str, name: &str) -> Vec<String> {
    let bases = bases_of(ini);
    let mut out: Vec<String> = Vec::new();
    let mut cur = name.to_ascii_lowercase();
    while let Some(b) = bases.get(&cur) {
        if b == name || out.contains(b) || is_builtin(&cur) {
            break;
        }
        out.push(b.clone());
        cur = b.clone();
    }
    out
}

/// Each section's last `base =` (lower case).
fn bases_of(ini: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut cur: Option<String> = None;
    for raw in ini.lines() {
        match line_of(raw) {
            L::Section(n) => cur = Some(n.to_ascii_lowercase()),
            L::Key(k, v) if k.eq_ignore_ascii_case("base") => {
                if let Some(c) = &cur {
                    out.insert(c.clone(), v.to_ascii_lowercase());
                }
            }
            _ => {}
        }
    }
    out
}

enum L {
    Other,
    Section(String),
    Key(String, String),
}

/// One creatures.ini line as the fork reads it (comments cut at ; # //).
fn line_of(raw: &str) -> L {
    let mut end = raw.len();
    let b = raw.as_bytes();
    for i in 0..b.len() {
        if b[i] == b';' || b[i] == b'#' || (b[i] == b'/' && b.get(i + 1) == Some(&b'/')) {
            end = i;
            break;
        }
    }
    let t = raw[..end].trim_matches([' ', '\t', '\r']);
    if t.is_empty() {
        return L::Other;
    }
    if let Some(r) = t.strip_prefix('[') {
        return L::Section(r.split(']').next().unwrap_or("").trim().to_string());
    }
    match t.split_once('=') {
        Some((k, v)) => L::Key(k.trim().to_string(), v.trim().to_string()),
        None => L::Other,
    }
}

// ---- packing --------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct PackedType {
    /// the shared name (`author.type`)
    pub name: String,
    /// its name on the publisher's computer
    pub from: String,
    /// its custom models and sounds (game paths)
    pub files: Vec<String>,
}

/// A shared creature as the community list shows it (`creatures` on an entry).
#[derive(Debug, Clone, Serialize, serde::Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct ItemCreature {
    pub name: String,
    /// it brings its own model
    pub model: bool,
    /// how many of its own sounds it brings
    pub sounds: u32,
}

/// An entry's list, cleaned: shared names only, 32 at most.
pub fn clean_item_creatures(list: Vec<ItemCreature>) -> Vec<ItemCreature> {
    let mut out: Vec<ItemCreature> = Vec::new();
    for c in list {
        if is_shared_name(&c.name) && !out.iter().any(|x| x.name == c.name) {
            out.push(ItemCreature { sounds: c.sounds.min(999), ..c });
        }
    }
    out.truncate(32);
    out
}

#[derive(Debug, Clone, Default)]
pub struct Pack {
    pub types: Vec<PackedType>,
    /// publisher's name -> shared name, for the package's references
    pub renames: BTreeMap<String, String>,
    /// the pack file's text (empty: no custom creatures)
    pub text: String,
    /// (game path, file on disk) for the custom models and sounds
    pub files: Vec<(String, PathBuf)>,
    /// what the Publish step should say: a name cut to fit
    pub notes: Vec<String>,
}

impl Pack {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
    /// The entry's `creatures` list.
    pub fn item_creatures(&self) -> Vec<ItemCreature> {
        self.types.iter().map(|t| ItemCreature { name: t.name.clone(), model: t.files.iter().any(|f| f.ends_with(".mdl")), sounds: t.files.iter().filter(|f| f.ends_with(".wav")).count() as u32 }).collect()
    }
    /// What goes in the package for its creatures: (zip path, bytes), checked against `limit`.
    pub fn entries(&self, limit: u64) -> Result<Vec<(String, Vec<u8>)>, String> {
        if self.is_empty() {
            return Ok(vec![]);
        }
        let mut out = vec![(PACK_FILE.to_string(), self.text.clone().into_bytes())];
        let mut total = 0u64;
        for (rel, p) in &self.files {
            let b = std::fs::read(p).map_err(|e| format!("Couldn't read {rel} ({e})."))?;
            total += b.len() as u64;
            if total > limit {
                return Err(format!("The creatures' models and sounds are over {} MB.", limit >> 20));
            }
            out.push((rel.clone(), b));
        }
        Ok(out)
    }
    /// "runner_x (with its model)" for each type the person named (bases follow along).
    pub fn summary(&self) -> Vec<String> {
        self.types
            .iter()
            .map(|t| {
                let m = t.files.iter().filter(|f| f.ends_with(".mdl")).count();
                let s = t.files.len() - m;
                let with = match (m, s) {
                    (0, 0) => String::new(),
                    (1, 0) => " (with its model)".into(),
                    (0, 1) => " (with its sound)".into(),
                    (0, n) => format!(" (with its {n} sounds)"),
                    (1, n) => format!(" (with its model and {n} sound{})", if n == 1 { "" } else { "s" }),
                    (a, 0) => format!(" (with its {a} models)"),
                    (a, n) => format!(" (with its {a} models and {n} sound{})", if n == 1 { "" } else { "s" }),
                };
                format!("{}{with}", t.from)
            })
            .collect()
    }
}

/// A file a type uses, if it's in the game's folders: (game path, file).
fn find_file(hl: &Path, game: &str, rel: &str) -> Option<PathBuf> {
    if !safe_rel(rel) {
        return None;
    }
    let root = std::fs::canonicalize(hl).ok()?;
    super::doctor::game_path(hl, game).into_iter().map(|d| d.join(rel)).find(|p| p.is_file()).and_then(|p| std::fs::canonicalize(p).ok()).filter(|p| p.starts_with(&root))
}

/// A model or sound path a pack may carry: models/....mdl or sound/....wav, plain parts.
fn safe_rel(rel: &str) -> bool {
    let low = rel.to_ascii_lowercase();
    let parts: Vec<&str> = rel.split('/').collect();
    ((low.starts_with("models/") && low.ends_with(".mdl")) || (low.starts_with("sound/") && low.ends_with(".wav")))
        && parts.len() >= 2
        && parts.len() <= 6
        && rel.len() <= 120
        && parts.iter().all(|p| crate::content::safe_part(p) && !crate::setup::record::windows_unsafe(p))
}

/// The custom files (not the game's own) a type's settled keys name.
fn custom_files(hl: &Path, game: &str, t: &creatures::Creature) -> Vec<(String, PathBuf)> {
    let mut rels: Vec<String> = Vec::new();
    for w in t.model.split([' ', '|']).filter(|w| !w.is_empty()) {
        rels.push(w.replace('\\', "/"));
    }
    for s in t.snd.iter() {
        for w in s.split([' ', '|']).filter(|w| !w.is_empty()) {
            rels.push(format!("sound/{}", w.replace('\\', "/")));
        }
    }
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for r in rels {
        if super::doctor::is_stock(&r, game) || out.iter().any(|(x, _)| x.eq_ignore_ascii_case(&r)) {
            continue;
        }
        if let Some(p) = find_file(hl, game, &r) {
            out.push((r.to_ascii_lowercase(), p));
        }
    }
    out
}

/// The Publish step's line for the custom creatures `names` use: {"includes": ["runner_x (with
/// its model)", ...]} or {"problem": why it can't be shared}.
pub fn listing(hl: &Path, game: &str, names: &[String]) -> Value {
    match pack_types(hl, game, names, "you") {
        Ok(p) => {
            let carried: Vec<String> = p
                .types
                .iter()
                .filter(|t| is_shared_name(&t.from))
                .map(|t| {
                    let a = t.name.split('.').next().unwrap_or("");
                    format!("{} is {a}'s: it can be listed once {a}'s own item is, and only unchanged.", t.name)
                })
                .collect();
            let mut notes = p.notes.clone();
            notes.extend(carried);
            serde_json::json!({"includes": p.summary(), "notes": notes})
        }
        Err(e) => serde_json::json!({"includes": [], "problem": e}),
    }
}

/// Packs the custom types `names` use (with their custom bases), named under `namespace` (from
/// `fetch_namespace`, or `namespace_for`). Built-ins,
/// "all" and Last Stand's planned names are left out; a shared type installed from someone
/// else keeps its name.
pub fn pack_types(hl: &Path, game: &str, names: &[String], namespace: &str) -> Result<Pack, String> {
    let wanted: Vec<String> = names.iter().map(|n| n.trim().to_ascii_lowercase()).filter(|n| !not_a_type(n) && !is_builtin(n)).collect();
    if wanted.is_empty() {
        return Ok(Pack::default());
    }
    let ini = crate::sys::read_latin1(&hl.join(game).join(creatures::FILE_REL)).unwrap_or_default();
    let have: BTreeSet<String> = sections_in(&ini).into_iter().collect();
    // each wanted type and the custom types behind it, bases first
    let mut order: Vec<String> = Vec::new();
    for n in &wanted {
        if !have.contains(n) {
            return Err(format!("{n} isn't one of your creature types. Make it in the Creature Studio, or pick another type."));
        }
        let mut chain: Vec<String> = base_chain(&ini, n).into_iter().filter(|b| !is_builtin(b) && have.contains(b)).collect();
        chain.reverse();
        chain.push(n.clone());
        for c in chain {
            if !order.contains(&c) {
                order.push(c);
            }
        }
    }
    let slug = namespace.to_string();
    if !author_part(&slug) {
        return Err("That isn't a name creatures can be shared under.".into());
    }
    let mut renames: BTreeMap<String, String> = BTreeMap::new();
    let mut notes: Vec<String> = Vec::new();
    for n in &order {
        let to = shared_name(&slug, n)?;
        if let Some((other, _)) = renames.iter().find(|(_, v)| **v == to) {
            return Err(format!("{other} and {n} would both be shared as {to}: rename one in the Creature Studio."));
        }
        let part = to.split_once('.').map(|x| x.1).unwrap_or("");
        if !is_shared_name(n) && part != n {
            notes.push(format!("{n} is shared as {part}: the full name is too long."));
        }
        renames.insert(n.clone(), to);
    }
    let resolved = creatures::resolve(&creatures::Doc::parse(&ini));
    let get = |n: &str| resolved.iter().find(|r| r.t.name.eq_ignore_ascii_case(n)).map(|r| r.t.clone());
    let mut types = Vec::new();
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut body = String::new();
    for n in &order {
        let to = &renames[n];
        let t = get(n).unwrap_or_default();
        // its base as the player who installs it has it: a built-in as the game ships it (never
        // the publisher's own changes to it), or a type packed above
        let base_name = bases_of(&ini).get(n).cloned().unwrap_or_default();
        let base_name = if is_builtin(&base_name) || renames.contains_key(&base_name) { base_name } else { String::new() };
        let base = if base_name.is_empty() { Some(creatures::defaults(to)) } else if is_builtin(&base_name) { creatures::builtin(&base_name) } else { get(&base_name) };
        let base = base.unwrap_or_default();
        body.push_str(&format!("[{to}]\n"));
        // a shared type installed from someone else goes as it came, line for line (the community
        // list takes it only unchanged, once its author's own item is listed)
        if is_shared_name(n) {
            for (k, v) in first_section_keys(&ini, n) {
                body.push_str(&format!("{k} = {v}\n"));
            }
            let own = custom_files(hl, game, &t.settled());
            for (r, p) in &own {
                if !files.iter().any(|(x, _)| x.eq_ignore_ascii_case(r)) {
                    files.push((r.clone(), p.clone()));
                }
            }
            types.push(PackedType { name: to.clone(), from: n.clone(), files: own.into_iter().map(|(r, _)| r).collect() });
            continue;
        }
        if !base_name.is_empty() {
            body.push_str(&format!("base = {}\n", renames.get(&base_name).cloned().unwrap_or(base_name.clone())));
        }
        // every key that differs from the base, by its own name (no aliases, no ls = / sounds =)
        for k in creatures::KEYS {
            let v = t.get(k);
            if v == base.get(k) {
                continue;
            }
            // (file names in lower case: the community list and Linux servers match them so)
            let v = if *k == "model" || k.starts_with("snd_") { v.replace('\\', "/").to_ascii_lowercase() } else { v };
            body.push_str(&format!("{k} = {v}\n"));
        }
        let t = t.settled();
        let own = custom_files(hl, game, &t);
        for (r, p) in &own {
            if !files.iter().any(|(x, _)| x.eq_ignore_ascii_case(r)) {
                files.push((r.clone(), p.clone()));
            }
        }
        types.push(PackedType { name: to.clone(), from: n.clone(), files: own.into_iter().map(|(r, _)| r).collect() });
    }
    let mut text = String::from("; Loner-Strike shared creatures: the app adds these to creatures.ini when this is installed.\n");
    text.push_str(&format!("; types: {}\n", types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(" ")));
    if !files.is_empty() {
        text.push_str(&format!("; files: {}\n", files.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>().join(" ")));
    }
    text.push_str(&body);
    // the types the person named come first in the listing; bases after
    types.sort_by_key(|t| !wanted.contains(&t.from));
    Ok(Pack { types, renames, text, files, notes })
}

/// A type's keys in its first section only (an installed type two blocks bring is in both).
fn first_section_keys(ini: &str, name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut state = 0; // 0 before, 1 in it, 2 after
    for raw in ini.lines() {
        match line_of(raw) {
            L::Section(n) => {
                if state == 1 {
                    state = 2;
                } else if state == 0 && n.eq_ignore_ascii_case(name) {
                    state = 1;
                }
            }
            L::Key(k, v) if state == 1 => out.push((k, v)),
            _ => {}
        }
    }
    out
}

/// The section names in a creatures.ini (lower case, once each).
fn sections_in(ini: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in ini.lines() {
        if let L::Section(n) = line_of(raw) {
            let n = n.to_ascii_lowercase();
            if !n.is_empty() && !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// A type's keys over all its sections, in file order (comments left out).
fn keys_of(ini: &str, name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut mine = false;
    for raw in ini.lines() {
        match line_of(raw) {
            L::Section(n) => mine = n.eq_ignore_ascii_case(name),
            L::Key(k, v) if mine => out.push((k, v)),
            _ => {}
        }
    }
    out
}

// ---- reading a pack -----------------------------------------------------------------------------

/// A pack file, checked: its types (shared names, none a built-in), its keys and its files.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    pub types: Vec<String>,
    pub files: Vec<String>,
    /// the sections as they go into creatures.ini
    pub body: String,
}

pub fn parse_pack(text: &str) -> Result<Parsed, String> {
    if text.len() > 64 << 10 || text.lines().any(|l| l.len() > 511) {
        return Err("The creature list in this download is too big.".into());
    }
    let mut p = Parsed::default();
    let mut cur: Option<String> = None;
    for raw in text.lines() {
        let t = raw.trim();
        if let Some(f) = t.strip_prefix("; files:") {
            for w in f.split_whitespace() {
                if !safe_rel(w) {
                    return Err(format!("The download names a creature file it can't use ({w})."));
                }
                if !p.files.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                    p.files.push(w.to_string());
                }
            }
            continue;
        }
        match line_of(raw) {
            L::Other => {}
            L::Section(n) => {
                let n = n.to_ascii_lowercase();
                if is_builtin(&n) || !n.contains('.') && creatures::builtin(&n).is_some() {
                    return Err(format!("The download changes the built-in creature {n}, so it wasn't installed."));
                }
                if let Some(e) = shared_name_problem(&n) {
                    return Err(format!("The download's creatures aren't named for who made them: {e}"));
                }
                if !p.types.contains(&n) {
                    p.types.push(n.clone());
                }
                p.body.push_str(&format!("[{n}]\n"));
                cur = Some(n);
            }
            L::Key(k, v) => {
                if cur.is_none() {
                    continue;
                }
                let low = k.to_ascii_lowercase();
                // (only the keys by their own names: no aliases, and no `ls =` or `sounds =`, which
                // carry paths in their own formats)
                if !(low == "base" || creatures::KEYS.contains(&low.as_str())) {
                    return Err(format!("The download's creature list has a setting the game doesn't know ({k})."));
                }
                if v.chars().any(|c| c.is_control()) || v.len() > 480 {
                    return Err("The download's creature list has a value the game can't read.".into());
                }
                // no value may hold .. \\ or : (whatever the key); file values are models/....mdl
                // and sounds under sound/
                let file_key = low == "model" || low.starts_with("snd_");
                if v.contains("..") || v.contains('\\') || v.contains(':') {
                    return Err(format!("A creature in the download names a file it can't use ({v})."));
                }
                if !file_key && low != "name" && v.contains('/') {
                    return Err(format!("The download's creature list has a value the game can't read ({k})."));
                }
                if file_key {
                    let model = low == "model";
                    let ok = v.split([' ', '|']).filter(|w| !w.is_empty()).all(|w| file_token(&w.to_ascii_lowercase(), model));
                    if !ok {
                        return Err(format!("A creature in the download names a file it can't use ({v})."));
                    }
                }
                if low == "base" && !is_builtin(&v) && !p.types.contains(&v.to_ascii_lowercase()) {
                    return Err(format!("A creature in the download is based on {v}, which it doesn't bring."));
                }
                p.body.push_str(&format!("{k} = {v}\n"));
            }
        }
    }
    if p.types.is_empty() {
        return Err("The download's creature list is empty.".into());
    }
    if p.types.len() > 32 {
        return Err("The download brings more than 32 creature types.".into());
    }
    Ok(p)
}

/// The pack file in an unpacked package, if it has one.
pub fn find_pack(src: &Path) -> Option<(PathBuf, String)> {
    let p = src.join(PACK_FILE);
    let meta = std::fs::symlink_metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > 64 << 10 {
        return None;
    }
    crate::sys::read_latin1(&p).ok().map(|t| (p, t))
}

/// A model or sound value's word, as the community list checks it (formats.py):
/// `models/[a-z0-9_./-]{1,150}.mdl`, or `[a-z0-9_-][a-z0-9_./-]{0,150}.wav` under sound/.
fn file_token(w: &str, model: bool) -> bool {
    let ok = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_./-".contains(&c);
    if model {
        let Some(mid) = w.strip_prefix("models/").and_then(|r| r.strip_suffix(".mdl")) else { return false };
        (1..=150).contains(&mid.len()) && mid.bytes().all(ok)
    } else {
        let Some(mid) = w.strip_suffix(".wav") else { return false };
        let b = mid.as_bytes();
        !b.is_empty() && b.len() <= 151 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit() || b"_-".contains(&b[0])) && b.iter().all(|c| ok(*c))
    }
}

// ---- the blocks in creatures.ini ------------------------------------------------------------------

/// A package id the block markers can carry: letters, digits, - _ : .
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_:.".contains(&c))
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub id: String,
    pub types: Vec<String>,
    /// files it needs
    pub files: Vec<String>,
    /// files the app put in the game for it (removed when no block needs them)
    pub wrote: Vec<String>,
    /// the app added a line break before the block (the file didn't end with one)
    joined: bool,
    pub body: String,
}

impl Block {
    fn render(&self) -> String {
        let mut s = format!("{BLOCK_START}{}{} (shared creatures: Loner-Strike adds and removes this block)\n", self.id, if self.joined { " +nl" } else { "" });
        s.push_str(&format!("; types: {}\n", self.types.join(" ")));
        if !self.files.is_empty() {
            s.push_str(&format!("; files: {}\n", self.files.join(" ")));
        }
        if !self.wrote.is_empty() {
            s.push_str(&format!("; wrote: {}\n", self.wrote.join(" ")));
        }
        s.push_str(&self.body);
        s.push_str(&format!("{BLOCK_END}{}\n", self.id));
        s
    }
}

/// The player's own text and the app's blocks, apart. Joining them back (`join_blocks`) gives
/// the same file when the blocks are at its end (where the app puts them).
pub fn split_blocks(text: &str) -> (String, Vec<Block>) {
    let mut user = String::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut cur: Option<Block> = None;
    for line in text.split_inclusive('\n') {
        let t = line.trim_end_matches(['\r', '\n']);
        if let Some(b) = cur.as_mut() {
            if t == format!("{BLOCK_END}{}", b.id) {
                blocks.push(cur.take().unwrap());
            } else if let Some(r) = t.strip_prefix("; types:") {
                b.types = r.split_whitespace().map(String::from).collect();
            } else if let Some(r) = t.strip_prefix("; files:") {
                b.files = r.split_whitespace().map(String::from).collect();
            } else if let Some(r) = t.strip_prefix("; wrote:") {
                b.wrote = r.split_whitespace().map(String::from).collect();
            } else {
                b.body.push_str(t);
                b.body.push('\n');
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix(BLOCK_START) {
            let mut words = rest.split_whitespace();
            let id = words.next().unwrap_or("").to_string();
            if valid_id(&id) {
                let joined = words.next() == Some("+nl");
                if joined && user.ends_with('\n') {
                    user.pop();
                    if user.ends_with('\r') {
                        user.pop();
                    }
                }
                cur = Some(Block { id, joined, ..Default::default() });
                continue;
            }
        }
        user.push_str(line);
    }
    // an unfinished block (cut by hand): its lines stay the player's
    if let Some(b) = cur {
        user.push_str(&b.render());
    }
    (user, blocks)
}

pub fn join_blocks(user: &str, blocks: &[Block]) -> String {
    let mut s = user.to_string();
    for b in blocks {
        let mut b = b.clone();
        b.joined = !s.is_empty() && !s.ends_with('\n');
        if b.joined {
            s.push('\n');
        }
        s.push_str(&b.render());
    }
    s
}

fn ini_path(game_dir: &Path) -> PathBuf {
    game_dir.join(creatures::FILE_REL)
}

fn read_ini(game_dir: &Path) -> Result<String, String> {
    let p = ini_path(game_dir);
    if !p.exists() {
        return Ok(String::new());
    }
    crate::sys::read_latin1(&p).map_err(|e| format!("Couldn't read creatures.ini ({e})."))
}

fn write_ini(game_dir: &Path, text: &str, why: &str) -> Result<(), String> {
    let p = ini_path(game_dir);
    crate::safety::before(TOOL, why, &p);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save creatures.ini ({e})."))?;
    }
    crate::sys::make_writable(&p);
    crate::sys::write_latin1_atomic(&p, text).map_err(|e| format!("Couldn't save creatures.ini ({e})."))
}

/// Whether a pack would go in (before anything else of the download is installed): it reads,
/// and none of its types is one of the player's own.
/// `namespace`: the uploader's, as the community list's entry says (else `namespace_for` their
/// GameBanana name): its types must be named under it (see `namespace_problem`).
pub fn check_pack(game_dir: &Path, text: &str, namespace: &str) -> Result<Parsed, String> {
    let p = parse_pack(text)?;
    let (user, blocks) = split_blocks(&read_ini(game_dir)?);
    let own: BTreeSet<String> = sections_in(&user).into_iter().collect();
    if let Some(n) = p.types.iter().find(|t| own.contains(*t)) {
        return Err(format!("You already have a creature type called {n}, so this download's isn't added. Rename yours in the Creature Studio to use it."));
    }
    if let Some(e) = namespace_problem(&p, &blocks, namespace) {
        return Err(e);
    }
    Ok(p)
}

/// A package may only define types named for whoever uploaded it. A type named for someone else
/// is accepted only as a carried copy of one already installed from them, key for key.
fn namespace_problem(p: &Parsed, installed: &[Block], namespace: &str) -> Option<String> {
    let slug = namespace.to_string();
    if !author_part(&slug) {
        return Some("Couldn't tell who uploaded this download, so its creatures weren't added.".into());
    }
    for t in &p.types {
        let author = t.split('.').next().unwrap_or("");
        if author == slug {
            continue;
        }
        let mine = keys_of(&p.body, t);
        if installed.iter().any(|b| b.types.iter().any(|x| x == t) && keys_of(&b.body, t) == mine) {
            continue;
        }
        return Some(format!("This download brings {t}, which is named for {author}, not for who uploaded it. Install {author}'s creation with that creature first."));
    }
    None
}

/// What an install did.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Done {
    pub types: Vec<String>,
    /// files the app put in the game
    pub wrote: Vec<String>,
    /// files the player already had (different from the download's): theirs were kept
    pub kept: Vec<String>,
    /// files it lists that neither the download nor the game had
    pub missing: Vec<String>,
}

/// Installs a package's creatures: its pack text goes in creatures.ini as the package's block
/// (an update replaces it in one write), and with `src` (the unpacked package) its models and
/// sounds are copied into the game where the player doesn't already have a file there. A map's
/// files come in with the map (no `src`).
pub fn install_pack(game_dir: &Path, id: &str, text: &str, src: Option<&Path>, namespace: &str) -> Result<Done, String> {
    install_inner(game_dir, id, text, src, Some(namespace))
}

fn install_inner(game_dir: &Path, id: &str, text: &str, src: Option<&Path>, publisher: Option<&str>) -> Result<Done, String> {
    if !valid_id(id) {
        return Err("That isn't a package id.".into());
    }
    let p = match publisher {
        Some(who) => check_pack(game_dir, text, who)?,
        None => {
            let p = parse_pack(text)?;
            let (user, _) = split_blocks(&read_ini(game_dir)?);
            if let Some(n) = p.types.iter().find(|t| sections_in(&user).contains(t)) {
                return Err(format!("There's already a creature type called {n} there, so it wasn't added."));
            }
            p
        }
    };
    let old = read_ini(game_dir)?;
    let (user, mut blocks) = split_blocks(&old);
    let prev = blocks.iter().position(|b| b.id == id).map(|i| blocks.remove(i));
    let mut done = Done { types: p.types.clone(), ..Default::default() };
    let mut wrote: Vec<String> = Vec::new();
    for rel in &p.files {
        let dest = game_dir.join(rel);
        let from = src.map(|s| s.join(rel)).filter(|f| f.is_file());
        // (only a file this package's previous version wrote is replaced; another package's or
        // the player's stays as it is)
        let ours = prev.as_ref().is_some_and(|b| b.wrote.iter().any(|w| w.eq_ignore_ascii_case(rel)));
        match (&from, dest.is_file()) {
            (Some(f), true) if crate::archive::same_file(f, &dest) => {
                if ours {
                    wrote.push(rel.clone());
                }
            }
            (Some(f), true) if ours => {
                let real = std::fs::canonicalize(f).map_err(|e| e.to_string())?;
                if !src.and_then(|s| std::fs::canonicalize(s).ok()).is_some_and(|s| real.starts_with(s)) {
                    return Err(format!("{rel} in the download isn't inside it."));
                }
                crate::safety::before(TOOL, "update a shared creature's file", &dest);
                std::fs::copy(&real, &dest).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
                wrote.push(rel.clone());
            }
            (Some(_), true) => done.kept.push(rel.clone()),
            (Some(f), false) => {
                let real = std::fs::canonicalize(f).map_err(|e| e.to_string())?;
                if !src.and_then(|s| std::fs::canonicalize(s).ok()).is_some_and(|s| real.starts_with(s)) {
                    return Err(format!("{rel} in the download isn't inside it."));
                }
                if let Some(d) = dest.parent() {
                    std::fs::create_dir_all(d).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
                }
                std::fs::copy(&real, &dest).map_err(|e| format!("Couldn't copy {rel} ({e})."))?;
                wrote.push(rel.clone());
            }
            (None, true) => {}
            (None, false) => done.missing.push(rel.clone()),
        }
    }
    // the previous version's files nobody needs now
    if let Some(b) = &prev {
        let mut others: Vec<Block> = blocks.clone();
        others.push(Block { files: p.files.clone(), ..Default::default() });
        let gone: Vec<String> = b.wrote.iter().filter(|w| !wrote.iter().any(|x| x.eq_ignore_ascii_case(w))).cloned().collect();
        drop_files(game_dir, &gone, &mut others);
    }
    done.wrote = wrote.clone();
    blocks.push(Block { id: id.to_string(), types: p.types.clone(), files: p.files.clone(), wrote, joined: false, body: p.body.clone() });
    let new = join_blocks(&user, &blocks);
    if new != old {
        write_ini(game_dir, &new, &format!("Add shared creatures from {id}"))?;
    }
    Ok(done)
}

/// Copies a block this app already installed and checked on this computer to another game folder
/// (a server here, or a server's copy before it's sent): no publisher check, since the block
/// passed it when it came in; everything else is checked again.
pub fn install_trusted(game_dir: &Path, id: &str, block: &str, src: Option<&Path>) -> Result<Done, String> {
    install_inner(game_dir, id, block, src, None)
}

/// Files a block wrote that it no longer needs: passed to another block that needs them, else
/// removed from the game.
fn drop_files(game_dir: &Path, gone: &[String], others: &mut [Block]) -> Vec<String> {
    let mut removed = Vec::new();
    for f in gone {
        if let Some(o) = others.iter_mut().find(|o| o.files.iter().any(|x| x.eq_ignore_ascii_case(f))) {
            if !o.wrote.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                o.wrote.push(f.clone());
            }
            continue;
        }
        let p = game_dir.join(f);
        if safe_rel(f) && p.is_file() {
            // (kept by the safety net first)
            crate::safety::before(TOOL, "remove a shared creature's file", &p);
            if std::fs::remove_file(&p).is_ok() {
                removed.push(f.clone());
            }
        }
    }
    removed
}

/// Takes a package's block out of creatures.ini (and the files only it needed). The types it
/// shares with another installed package stay (in that one's block).
pub fn uninstall_pack(game_dir: &Path, id: &str) -> Result<Vec<String>, String> {
    let old = read_ini(game_dir)?;
    let (user, mut blocks) = split_blocks(&old);
    let Some(i) = blocks.iter().position(|b| b.id == id) else { return Ok(vec![]) };
    let b = blocks.remove(i);
    let removed = drop_files(game_dir, &b.wrote, &mut blocks);
    write_ini(game_dir, &join_blocks(&user, &blocks), &format!("Remove shared creatures from {id}"))?;
    Ok(removed)
}

/// A package's block as it is now (kept before an update, for Go back).
pub fn block_text(game_dir: &Path, id: &str) -> Option<String> {
    let (_, blocks) = split_blocks(&read_ini(game_dir).ok()?);
    blocks.into_iter().find(|b| b.id == id).map(|b| b.render())
}

/// Puts a kept block back as it was (Go back, or an update that failed), or takes the block
/// out when there was none before.
pub fn restore_block(game_dir: &Path, id: &str, kept: Option<&str>) -> Result<(), String> {
    let old = read_ini(game_dir)?;
    let (user, mut blocks) = split_blocks(&old);
    let now = blocks.iter().position(|b| b.id == id).map(|i| blocks.remove(i));
    if let Some(k) = kept {
        let (_, mut back) = split_blocks(k);
        let Some(b) = back.pop().filter(|b| b.id == id) else { return Err("The kept creature list couldn't be read.".into()) };
        // files the newer block wrote that the old one doesn't list go
        if let Some(n) = &now {
            let gone: Vec<String> = n.wrote.iter().filter(|w| !b.files.iter().any(|x| x.eq_ignore_ascii_case(w))).cloned().collect();
            let mut others = blocks.clone();
            others.push(b.clone());
            drop_files(game_dir, &gone, &mut others);
        }
        blocks.push(b);
    } else if let Some(n) = &now {
        drop_files(game_dir, &n.wrote, &mut blocks);
    }
    let new = join_blocks(&user, &blocks);
    if new != old {
        write_ini(game_dir, &new, &format!("Put back shared creatures from {id}"))?;
    }
    Ok(())
}

/// Each installed shared type and the packages that bring it.
pub fn installed_types(game_dir: &Path) -> BTreeMap<String, Vec<String>> {
    let (_, blocks) = split_blocks(&read_ini(game_dir).unwrap_or_default());
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for b in blocks {
        for t in b.types {
            out.entry(t).or_default().push(b.id.clone());
        }
    }
    out
}

/// Makes an installed shared type your own: a copy under `new_name` in your part of
/// creatures.ini (its keys flattened onto its built-in base), with a line saying where it came from.
pub fn remix_type(game_dir: &Path, shared: &str, new_name: &str) -> Result<(), String> {
    if let Some(e) = creatures::name_problem(new_name) {
        return Err(e);
    }
    let old = read_ini(game_dir)?;
    let (user, blocks) = split_blocks(&old);
    if sections_in(&user).iter().any(|n| n.eq_ignore_ascii_case(new_name)) || is_builtin(new_name) {
        return Err(format!("There's already a type called {new_name}."));
    }
    let Some(b) = blocks.iter().find(|b| b.types.iter().any(|t| t.eq_ignore_ascii_case(shared))) else { return Err(format!("{shared} isn't installed.")) };
    // the chain inside the block, root first
    let mut chain = vec![shared.to_ascii_lowercase()];
    chain.extend(base_chain(&b.body, shared));
    let root = chain.iter().rev().find(|c| is_builtin(c)).cloned().unwrap_or_else(|| "zombie".into());
    let mut s = format!("; Remixed from {shared} (shared by {})\n[{new_name}]\nbase = {root}\n", shared.split('.').next().unwrap_or(""));
    for n in chain.iter().rev().filter(|c| !is_builtin(c)) {
        for (k, v) in keys_of(&b.body, n) {
            if !k.eq_ignore_ascii_case("base") {
                s.push_str(&format!("{k} = {v}\n"));
            }
        }
    }
    let mut user = user;
    if !user.is_empty() && !user.ends_with('\n') {
        user.push('\n');
    }
    user.push_str(&s);
    write_ini(game_dir, &join_blocks(&user, &blocks), &format!("Remix {shared} as {new_name}"))
}

// ---- prefabs: the package's .map files ------------------------------------------------------------

/// The creature types the .map files in a zip (a prefab package) name.
pub fn types_in_zip_maps(zip_path: &Path) -> Result<BTreeSet<String>, String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut out = BTreeSet::new();
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        if !e.name().map(|n| n.to_ascii_lowercase()).unwrap_or_default().ends_with(".map") || e.size() > 64 << 20 {
            continue;
        }
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut e, &mut b).map_err(|e| e.to_string())?;
        out.extend(types_in_entity_text(&String::from_utf8_lossy(&b)));
    }
    Ok(out)
}

/// Writes a zip again with its .map files' creature types renamed and the pack's entries added.
pub fn repack_zip(zip_path: &Path, pack: &Pack, limit: u64) -> Result<(), String> {
    use std::io::{Read, Write};
    if pack.is_empty() {
        return Ok(());
    }
    let extra = pack.entries(limit)?;
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for i in 0..z.len() {
            let mut e = z.by_index(i).map_err(|e| e.to_string())?;
            let name = e.name().map(|n| n.to_string()).map_err(|e| e.to_string())?;
            if e.is_dir() || extra.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let mut b = Vec::new();
            e.read_to_end(&mut b).map_err(|e| e.to_string())?;
            if name.to_ascii_lowercase().ends_with(".map") {
                b = rewrite_entity_text(&String::from_utf8_lossy(&b), &pack.renames).into_bytes();
            }
            w.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            w.write_all(&b).map_err(|e| e.to_string())?;
        }
        for (n, b) in &extra {
            w.start_file(n.as_str(), opts).map_err(|e| e.to_string())?;
            w.write_all(b).map_err(|e| e.to_string())?;
        }
        w.finish().map_err(|e| e.to_string())?;
    }
    crate::sys::write_atomic(zip_path, &buf.into_inner()).map_err(|e| format!("Couldn't save the package ({e})."))
}

// ---- maps: the entity lump ----------------------------------------------------------------------

/// A BSP (version 30) with its entity lump replaced: the new lump goes at the end (4-byte
/// aligned) and the header points at it. The engine's map checksum skips the entity lump.
pub fn bsp_with_entities(bsp: &[u8], ents: &str) -> Result<Vec<u8>, String> {
    if bsp.len() < 124 || u32::from_le_bytes([bsp[0], bsp[1], bsp[2], bsp[3]]) != 30 {
        return Err("That map file isn't a Half-Life map.".into());
    }
    let mut out = bsp.to_vec();
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let off = out.len() as u32;
    out.extend_from_slice(ents.as_bytes());
    out.push(0);
    let len = ents.len() as u32 + 1;
    out[4..8].copy_from_slice(&off.to_le_bytes());
    out[8..12].copy_from_slice(&len.to_le_bytes());
    Ok(out)
}

/// A BSP's entity lump as text.
pub fn bsp_entities(bsp: &[u8]) -> Option<String> {
    if bsp.len() < 124 {
        return None;
    }
    let off = u32::from_le_bytes(bsp[4..8].try_into().ok()?) as usize;
    let len = u32::from_le_bytes(bsp[8..12].try_into().ok()?) as usize;
    let raw = bsp.get(off..off.checked_add(len)?)?;
    let raw = raw.split(|b| *b == 0).next().unwrap_or(raw);
    Some(raw.iter().map(|b| *b as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(ini: &str) -> (PathBuf, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let hl = std::env::temp_dir().join(format!("ls-creaturepack-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&hl);
        let gd = hl.join("cstrike");
        std::fs::create_dir_all(gd.join("addons/amxmodx/configs/gamemodes")).unwrap();
        std::fs::write(gd.join(creatures::FILE_REL), ini).unwrap();
        (hl, gd)
    }

    const INI: &str = "; mine\n[tough_base]\nbase = brute\nhp = 900\n\n[runner_x]\nbase = tough_base\nmodel = models/qa/runner_x.mdl models/zombie.mdl\nsnd_die = qa/die.wav\nspeed = 320 ; fast\n\n[loner]\nbase = zombie\n";

    #[test]
    fn detection_in_graphs_entities_waves_and_bases() {
        let g: Value = serde_json::json!({"nodes": [
            {"id": "a", "type": "creature.spawn", "params": {"kind": "runner_x"}},
            {"id": "b", "type": "creature.spawn", "params": {}},
            {"id": "c", "type": "turret.setup", "params": {"type": "my_turret"}},
            {"id": "d", "type": "creature.of_type", "params": {"type": ""}},
            {"id": "e", "type": "comp.create", "params": {"kind": "ls_wave_spawner", "keys": "wave_min=4 types=runner_x brute waves=3"}},
            {"id": "f", "type": "comp.create", "params": {"kind": "ls_creature", "keys": "type=loner behaviour=chase"}},
            {"id": "g", "type": "msg.hud", "params": {"text": "runner_x"}}
        ]});
        let want: BTreeSet<String> = ["runner_x", "zombie", "my_turret", "brute", "loner"].iter().map(|s| s.to_string()).collect();
        assert_eq!(types_in_graph_json(&g), want);
        let graph: super::super::studio::graph::Graph = serde_json::from_value(serde_json::json!({"schema": "x", "version": 1, "mode": {"id": "m", "name": "M"}, "nodes": g["nodes"]})).unwrap();
        assert_eq!(types_in_graph(&graph), want);

        // a map's entities (and a prefab's .map, brushes and all)
        let ents = "{\n\"classname\" \"worldspawn\"\n}\n{\n\"type\" \"runner_x\"\n\"classname\" \"ls_creature\"\n}\n{\n\"classname\" \"ls_wave_spawner\"\n\"types\" \"loner brute\"\n}\n{\n\"classname\" \"info_target\"\n\"type\" \"not_a_creature\"\n}\n";
        let set: Vec<String> = types_in_entity_text(ents).into_iter().collect();
        assert_eq!(set, vec!["brute", "loner", "runner_x"]);
        let parsed = crate::goldsrc::entities::parse(ents);
        assert_eq!(types_in_entities(&parsed).into_iter().collect::<Vec<_>>(), set);
        let map = "// prefab\n{\n\"classname\" \"ls_creature\"\n\"type\" \"loner\"\n{\n( 0 0 0 ) ( 1 0 0 ) ( 0 1 0 ) AAATRIGGER [ 1 0 0 0 ] [ 0 1 0 0 ] 0 1 1\n}\n}\n";
        assert_eq!(types_in_entity_text(map).into_iter().collect::<Vec<_>>(), vec!["loner"]);

        let waves = "[laststand]\nboss = big_one\nwave 1 = zombie 10 runner_x 5 off 0\n";
        assert_eq!(types_in_waves(waves).into_iter().collect::<Vec<_>>(), vec!["big_one", "runner_x", "zombie"]);

        assert_eq!(base_chain(INI, "runner_x"), vec!["tough_base", "brute"]);
        assert_eq!(base_chain(INI, "loner"), vec!["zombie"]);
        assert!(base_chain("[a]\nbase = b\n[b]\nbase = a\n", "a").len() <= 2, "a loop stops");
        assert!(is_builtin("hound") && !is_builtin("runner_x"));
    }

    #[test]
    fn names_say_who_made_them() {
        assert_eq!(author_slug("Jam Tucker!"), "jam_tucker");
        assert_eq!(author_slug("  "), "a");
        assert_eq!(author_slug("1234"), "a1234");
        assert_eq!(shared_name("jam", "runner_x").unwrap(), "jam.runner_x");
        assert_eq!(shared_name("averyveryverylongname", "a_rather_long_name_xx").unwrap(), "averyveryverylongname.a_rather");
        assert_eq!(shared_name("averyveryverylongname", "short").unwrap(), "averyveryverylongname.short");
        assert_eq!(shared_name("jam", "bob.x").unwrap(), "bob.x", "an installed shared type keeps its name");
        assert_eq!(shared_name("jam", &"x".repeat(29)).unwrap().len(), 31);
        assert!(shared_name("averyveryverylongname", "123456789_abc").is_err(), "cut to digits only");
        assert!(!is_shared_name("runner_x") && !is_shared_name("a.b.c") && !is_shared_name("A.b"));
    }

    #[test]
    fn packs_custom_types_with_bases_files_and_renames() {
        let (hl, gd) = game(INI);
        std::fs::create_dir_all(gd.join("models/qa")).unwrap();
        std::fs::write(gd.join("models/qa/runner_x.mdl"), b"IDST model").unwrap();
        std::fs::create_dir_all(gd.join("sound/qa")).unwrap();
        std::fs::write(gd.join("sound/qa/die.wav"), b"RIFF").unwrap();
        let p = pack_types(&hl, "cstrike", &["runner_x".into(), "zombie".into(), "all".into()], "jam").unwrap();
        assert_eq!(p.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["jam.runner_x", "jam.tough_base"]);
        assert_eq!(p.renames["runner_x"], "jam.runner_x");
        assert!(p.text.contains("[jam.tough_base]\nbase = brute\nhp = 900\n[jam.runner_x]\nbase = jam.tough_base\n"), "{}", p.text);
        assert!(!p.text.contains("[zombie]") && !p.text.contains("fast"));
        assert_eq!(p.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["models/qa/runner_x.mdl", "sound/qa/die.wav"], "the game's own zombie.mdl isn't packed");
        assert_eq!(p.summary()[0], "runner_x (with its model and 1 sound)");
        assert!(pack_types(&hl, "cstrike", &["ghost".into()], "jam").unwrap_err().contains("isn't one of your creature types"));
        assert!(pack_types(&hl, "cstrike", &["zombie".into()], "jam").unwrap().is_empty());

        // the package's references follow the new names
        let mut g = serde_json::json!({"nodes": [{"id": "a", "type": "creature.spawn", "params": {"kind": "runner_x"}}, {"id": "e", "type": "comp.create", "params": {"kind": "ls_wave_spawner", "keys": "types=runner_x brute waves=3"}}]});
        rewrite_graph_json(&mut g, &p.renames);
        assert_eq!(g["nodes"][0]["params"]["kind"], "jam.runner_x");
        assert_eq!(g["nodes"][1]["params"]["keys"], "types=jam.runner_x brute waves=3");
        let ents = "{\n\"classname\" \"ls_wave_spawner\"\n\"types\" \"runner_x zombie\"\n\"targetname\" \"runner_x\"\n}\n";
        assert_eq!(rewrite_entity_text(ents, &p.renames), "{\n\"classname\" \"ls_wave_spawner\"\n\"types\" \"jam.runner_x zombie\"\n\"targetname\" \"runner_x\"\n}\n");
        // the map's entity lump, replaced
        let mut bsp = vec![0u8; 124];
        bsp[0] = 30;
        bsp[4..8].copy_from_slice(&124u32.to_le_bytes());
        bsp[8..12].copy_from_slice(&(ents.len() as u32).to_le_bytes());
        bsp.extend_from_slice(ents.as_bytes());
        let new = bsp_with_entities(&bsp, &rewrite_entity_text(ents, &p.renames)).unwrap();
        assert!(bsp_entities(&new).unwrap().contains("jam.runner_x zombie"));
        assert!(bsp_with_entities(b"nope", "").is_err());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn refuses_built_ins_and_names_not_for_who_made_them() {
        assert!(parse_pack("[zombie]\nhp = 5\n").unwrap_err().contains("built-in creature zombie"));
        assert!(parse_pack("[runner_x]\nhp = 5\n").unwrap_err().contains("aren't named for who made them"));
        assert!(parse_pack("[jam.x]\nbase = bob.y\n").unwrap_err().contains("doesn't bring"));
        assert!(parse_pack("[jam.x]\nrun_shell = 1\n").unwrap_err().contains("doesn't know"));
        assert!(parse_pack("; files: ../../x.mdl\n[jam.x]\n").unwrap_err().contains("can't use"));
        assert!(parse_pack("; files: models/con.mdl\n[jam.x]\n").is_err());
        let p = parse_pack("; types: jam.x\n; files: models/jam/x.mdl\n[jam.x]\nbase = runner\nhp = 5\n").unwrap();
        assert_eq!((p.types, p.files), (vec!["jam.x".to_string()], vec!["models/jam/x.mdl".to_string()]));
    }

    #[test]
    fn install_and_uninstall_keep_the_players_part_and_count_users() {
        let mine = "; my creatures\r\n[pet]\r\nbase = hound\r\nhp = 5"; // CRLF, no final line break
        let (hl, gd) = game(mine);
        let src = hl.join("pkg");
        std::fs::create_dir_all(src.join("models/jam")).unwrap();
        std::fs::write(src.join("models/jam/x.mdl"), b"IDST x").unwrap();
        let pack = "; types: jam.x\n; files: models/jam/x.mdl\n[jam.x]\nbase = runner\nhp = 50\n";
        let d = install_pack(&gd, "mod-1", pack, Some(&src), "jam").unwrap();
        assert_eq!(d.wrote, vec!["models/jam/x.mdl"]);
        assert!(gd.join("models/jam/x.mdl").is_file());
        let text = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert!(text.starts_with(mine) && text.contains("; >>> from mod-1 +nl") && text.contains("[jam.x]\nbase = runner\nhp = 50\n; <<< end mod-1\n"), "{text}");
        // the game (and the Studio) read the type
        let r = creatures::resolve(&creatures::Doc::parse(&text));
        assert_eq!(r.iter().find(|x| x.t.name == "jam.x").unwrap().t.hp, 50.0);
        assert_eq!(r.iter().find(|x| x.t.name == "pet").unwrap().t.hp, 5.0);

        // a second package that shares the type and the model
        install_pack(&gd, "mod-2", pack, None, "jam").unwrap();
        assert_eq!(installed_types(&gd)["jam.x"], vec!["mod-1", "mod-2"]);
        // the first goes: the type and the model stay for the second
        assert!(uninstall_pack(&gd, "mod-1").unwrap().is_empty());
        assert!(gd.join("models/jam/x.mdl").is_file());
        assert_eq!(installed_types(&gd)["jam.x"], vec!["mod-2"]);
        // the second goes: everything of ours is gone, the player's part is byte for byte as it was
        assert_eq!(uninstall_pack(&gd, "mod-2").unwrap(), vec!["models/jam/x.mdl"]);
        assert!(!gd.join("models/jam/x.mdl").exists());
        assert_eq!(std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap(), mine);
        assert!(uninstall_pack(&gd, "mod-9").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn never_overwrites_the_players_files_or_types() {
        let (hl, gd) = game("[jam.x]\nhp = 1\n");
        let pack = "; files: models/jam/x.mdl\n[jam.x]\nbase = runner\n";
        assert!(install_pack(&gd, "mod-1", pack, None, "jam").unwrap_err().contains("already have a creature type called jam.x"));
        std::fs::write(gd.join(creatures::FILE_REL), "").unwrap();
        std::fs::create_dir_all(gd.join("models/jam")).unwrap();
        std::fs::write(gd.join("models/jam/x.mdl"), b"theirs").unwrap();
        let src = hl.join("pkg");
        std::fs::create_dir_all(src.join("models/jam")).unwrap();
        std::fs::write(src.join("models/jam/x.mdl"), b"ours").unwrap();
        let d = install_pack(&gd, "mod-1", pack, Some(&src), "jam").unwrap();
        assert_eq!(d.kept, vec!["models/jam/x.mdl"]);
        uninstall_pack(&gd, "mod-1").unwrap();
        assert_eq!(std::fs::read(gd.join("models/jam/x.mdl")).unwrap(), b"theirs");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn updates_replace_the_block_and_go_back_restores_it() {
        let (hl, gd) = game("[pet]\nbase = hound\n");
        let v1 = "[jam.x]\nbase = runner\nhp = 50\n";
        let v2 = "[jam.x]\nbase = runner\nhp = 80\n[jam.y]\nbase = jam.x\n";
        install_pack(&gd, "mod-1", v1, None, "jam").unwrap();
        let kept = block_text(&gd, "mod-1");
        install_pack(&gd, "mod-1", v2, None, "jam").unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert_eq!(t.matches("; >>> from mod-1").count(), 1, "one block per package");
        assert!(t.contains("hp = 80") && t.contains("[jam.y]"));
        // Go back (or an update that failed): the old block as it was
        restore_block(&gd, "mod-1", kept.as_deref()).unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert!(t.contains("hp = 50") && !t.contains("[jam.y]"));
        assert!(t.starts_with("[pet]\nbase = hound\n"));
        // nothing kept: the block goes
        restore_block(&gd, "mod-1", None).unwrap();
        assert_eq!(std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap(), "[pet]\nbase = hound\n");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn remix_makes_a_shared_type_your_own() {
        let (hl, gd) = game("[pet]\nbase = hound\n");
        install_pack(&gd, "mod-1", "[jam.base]\nbase = brute\nhp = 900\n[jam.x]\nbase = jam.base\nspeed = 320\n", None, "jam").unwrap();
        remix_type(&gd, "jam.x", "my_x").unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        let (user, blocks) = split_blocks(&t);
        assert!(user.contains("; Remixed from jam.x (shared by jam)\n[my_x]\nbase = brute\nhp = 900\nspeed = 320\n"), "{user}");
        assert_eq!(blocks.len(), 1);
        let r = creatures::resolve(&creatures::Doc::parse(&t));
        let x = r.iter().find(|x| x.t.name == "my_x").unwrap();
        assert_eq!((x.t.hp, x.t.speed), (900.0, 320.0));
        assert!(remix_type(&gd, "jam.x", "pet").is_err());
        assert!(remix_type(&gd, "jam.nope", "other").is_err());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn a_hostile_pack_cant_name_a_path_through_any_key() {
        let bad = ["../../../x", "..\\x", "C:\\x", "\\\\server\\share\\x", "a:b", "/etc/passwd.wav", "models/../../x.mdl"];
        for k in creatures::KEYS.iter().chain(["ls", "sounds", "models", "health", "mm", "bot"].iter()) {
            for v in bad {
                // (the shown name is text, never a path: it may hold a /)
                if *k == "name" && !v.contains("..") && !v.contains('\\') && !v.contains(':') {
                    continue;
                }
                let t = format!("[jam.x]\n{k} = {v}\n");
                assert!(parse_pack(&t).is_err(), "{k} = {v} got through");
            }
        }
        // the aliases and ls = / sounds = aren't taken at all
        for k in ["ls = model=models/jam/x.mdl", "sounds = jam/a.wav", "models = models/jam/x.mdl", "health = 5"] {
            assert!(parse_pack(&format!("[jam.x]\n{k}\n")).is_err(), "{k}");
        }
        assert!(parse_pack("[jam.x]\nmodel = jam/x.mdl\n").is_err(), "a model outside models/");
        assert!(parse_pack("[jam.x]\nsnd_die = jam/x.mp3\n").is_err());
        assert!(parse_pack("[jam.x]\nmodel = models/jam/x.mdl | models/zombie.mdl\nsnd_die = jam/a.wav jam/b.wav | player/die1.wav\n").is_ok());
    }

    #[test]
    fn one_package_never_replaces_anothers_file() {
        let (hl, gd) = game("");
        let mk = |dir: &str, body: &[u8]| {
            let src = hl.join(dir);
            std::fs::create_dir_all(src.join("models/jam")).unwrap();
            std::fs::write(src.join("models/jam/x.mdl"), body).unwrap();
            src
        };
        let pack = "; files: models/jam/x.mdl\n[jam.x]\nmodel = models/jam/x.mdl\n";
        install_pack(&gd, "mod-1", pack, Some(&mk("a", b"from A")), "jam").unwrap();
        let d = install_pack(&gd, "mod-2", pack, Some(&mk("b", b"from B")), "jam").unwrap();
        assert_eq!(d.kept, vec!["models/jam/x.mdl"]);
        assert_eq!(std::fs::read(gd.join("models/jam/x.mdl")).unwrap(), b"from A");
        // A's own update may replace its file
        install_pack(&gd, "mod-1", pack, Some(&mk("c", b"A v2")), "jam").unwrap();
        assert_eq!(std::fs::read(gd.join("models/jam/x.mdl")).unwrap(), b"A v2");
        // A goes: B still needs the file, so it stays (now B's to remove)
        uninstall_pack(&gd, "mod-1").unwrap();
        assert!(gd.join("models/jam/x.mdl").is_file());
        assert_eq!(uninstall_pack(&gd, "mod-2").unwrap(), vec!["models/jam/x.mdl"]);
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn types_are_bound_to_who_uploaded_them() {
        let (hl, gd) = game("");
        let pack = "[jam.x]\nbase = runner\nhp = 50\n";
        assert!(install_pack(&gd, "mod-1", pack, None, "bob").unwrap_err().contains("named for jam"));
        assert!(install_pack(&gd, "mod-1", pack, None, "").is_err() && install_pack(&gd, "mod-1", pack, None, "Jam").is_err());
        install_pack(&gd, "mod-1", pack, None, "jam").unwrap();
        // Bob's mode carries jam.x: fine when it's the same as the one installed from Jam
        install_pack(&gd, "mod-2", "[jam.x]\nbase = runner\nhp = 50\n[bob.y]\nbase = jam.x\n", None, "bob").unwrap();
        // ... but not a changed copy (that would replace Jam's creature)
        assert!(install_pack(&gd, "mod-3", "[jam.x]\nbase = runner\nhp = 9999\n", None, "bob").unwrap_err().contains("named for jam"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn someone_elses_type_is_carried_unchanged() {
        let (hl, gd) = game("[mine]\nbase = bob.y\nhp = 5\n");
        let bob = "[bob.y]\nbase = runner\nhp = 44\nspeed = 300\n";
        install_pack(&gd, "mod-7", bob, None, "bob").unwrap();
        let p = pack_types(&hl, "cstrike", &["mine".into()], "jam").unwrap();
        assert_eq!(p.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["jam.mine", "bob.y"]);
        assert!(p.text.contains("[bob.y]\nbase = runner\nhp = 44\nspeed = 300\n[jam.mine]\nbase = bob.y\n"), "{}", p.text);
        let l = listing(&hl, "cstrike", &["mine".into()]);
        assert_eq!(l["notes"][0], "bob.y is bob's: it can be listed once bob's own item is, and only unchanged.");
        // installing Jam's package elsewhere: only where Bob's own is in, unchanged
        let (hl2, other) = game("");
        assert!(install_pack(&other, "mod-8", &p.text, None, "jam").unwrap_err().contains("named for bob"));
        install_pack(&other, "mod-7", bob, None, "bob").unwrap();
        install_pack(&other, "mod-8", &p.text, None, "jam").unwrap();
        let r = creatures::resolve(&creatures::Doc::parse(&std::fs::read_to_string(other.join(creatures::FILE_REL)).unwrap()));
        assert_eq!(r.iter().find(|x| x.t.name == "jam.mine").unwrap().t.hp, 5.0);
        let _ = std::fs::remove_dir_all(&hl);
        let _ = std::fs::remove_dir_all(&hl2);
    }

    #[test]
    fn a_package_with_twin_entries_is_refused() {
        use std::io::Write;
        let names = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for twin in [["creatures/creatures.ini", "Creatures/creatures.INI"], ["creatures/creatures.ini", "creatures\\creatures.ini"], ["creatures/creatures.ini", "./creatures//creatures.ini"]] {
            let mut l = names(&["loner-strike.json"]);
            l.extend(names(&twin));
            assert_eq!(crate::archive::package_twin(&l).as_deref(), Some("creatures/creatures.ini"), "{twin:?}");
        }
        assert_eq!(crate::archive::package_twin(&names(&["loner-strike.json", "creatures/", "creatures/creatures.ini"])), None);
        assert_eq!(crate::archive::package_twin(&names(&["a.bsp", "A.bsp"])), None, "only Loner-Strike packages");
        // unpacking one is refused
        let dir = std::env::temp_dir().join(format!("ls-twin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let zp = dir.join("p.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for (n, b) in [("loner-strike.json", "{}"), ("creatures/creatures.ini", "[jam.x]\n"), ("CREATURES/creatures.ini", "[jam.x]\nhp = 1\n")] {
                z.start_file(n, o).unwrap();
                z.write_all(b.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        assert!(crate::archive::extract(&zp, &dir.join("out"), 1 << 20).unwrap_err().contains("two files that unpack as creatures/creatures.ini"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blocks_split_and_join_back_the_same() {
        for t in ["", "[a]\nhp = 1\n", "[a]\r\nhp = 1\r\n", "[a]\nhp = 1"] {
            let (u, b) = split_blocks(t);
            assert_eq!(join_blocks(&u, &b), t);
        }
        let with = join_blocks("[a]\nhp = 1", &[Block { id: "mod-1".into(), types: vec!["j.x".into()], body: "[j.x]\n".into(), ..Default::default() }]);
        let (u, b) = split_blocks(&with);
        assert_eq!(u, "[a]\nhp = 1");
        assert_eq!(join_blocks(&u, &b), with);
    }
}
