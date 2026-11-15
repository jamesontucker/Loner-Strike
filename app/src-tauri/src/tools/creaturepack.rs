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

/// Whether any of `names` is a custom type that would be packed.
pub fn has_custom(names: &[String]) -> bool {
    names.iter().any(|n| !not_a_type(n) && !is_builtin(n))
}

/// Names a mode or map may use that aren't types to pack: the wave spawner's "all", and Last
/// Stand's round-table names a stand-in plays until the game has them.
fn not_a_type(name: &str) -> bool {
    name.is_empty() || name == "all" || ["controller", "stukabat"].contains(&name)
}

/// The type part: small letters, digits and _ (1-29).
fn plain_part(s: &str) -> bool {
    (1..=29).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

/// The namespace part: `gb` and a member id (no leading 0, 9 digits at most).
fn author_part(s: &str) -> bool {
    let Some(d) = s.strip_prefix("gb") else { return false };
    (1..=9).contains(&d.len()) && d.bytes().all(|c| c.is_ascii_digit()) && !d.starts_with('0')
}

/// A shared type's name, exactly as the community list checks it (formats.py):
/// `^gb[1-9][0-9]{0,8}\.[a-z0-9_]{1,29}$`, 31 at most.
pub fn is_shared_name(n: &str) -> bool {
    shared_name_problem(n).is_none()
}

pub fn shared_name_problem(n: &str) -> Option<String> {
    let Some((a, t)) = n.split_once('.') else { return Some(format!("{n} isn't named for who made it (gb<member id>.type).")) };
    if n.len() > 31 || !author_part(a) || !plain_part(t) {
        return Some(format!("{n} isn't a shared creature name (gb<member id>.type: small letters, digits and _, 31 at most)."));
    }
    None
}

/// The namespace of someone's shared creatures: their GameBanana member id, `gb<id>` (the
/// community list binds each entry to its uploader's id). People see "runner_x by <name>".
pub fn namespace_of(member: i64) -> Option<String> {
    (member > 0 && member < 1_000_000_000).then(|| format!("gb{member}"))
}

/// A list entry's namespace, cleaned ("" unless it's `gb<id>`).
pub fn clean_namespace(ns: &str) -> String {
    match ns.strip_prefix("gb").and_then(|d| d.parse::<i64>().ok()) {
        Some(m) if namespace_of(m).as_deref() == Some(ns) => ns.to_string(),
        _ => String::new(),
    }
}

/// A shared type's name for people: (the type, its maker's member id). "gb123.runner_x" is
/// ("runner_x", Some(123)); a type of your own is (its name, None).
pub fn display_name(shared: &str) -> (String, Option<i64>) {
    match shared.split_once('.') {
        Some((ns, t)) if !clean_namespace(ns).is_empty() => (t.to_string(), ns[2..].parse().ok()),
        Some((_, t)) => (t.to_string(), None),
        None => (shared.to_string(), None),
    }
}

/// A GameBanana member id from what someone typed: the id, or their profile link
/// (gamebanana.com/members/123456).
pub fn member_from_link(s: &str) -> Option<i64> {
    let t = s.trim().trim_end_matches('/');
    let last = t.rsplit('/').next().unwrap_or(t);
    let id: i64 = last.parse().ok()?;
    let ok = t == last || t.contains("gamebanana.com/members/");
    (ok && namespace_of(id).is_some()).then_some(id)
}

/// `<author>.<type>`. The author part is always the whole namespace (installs check it against
/// who uploaded the package); a type name too long to fit is cut instead.
pub fn shared_name(slug: &str, name: &str) -> Result<String, String> {
    if is_shared_name(name) {
        return Ok(name.to_string());
    }
    let room = 31usize.saturating_sub(slug.len() + 1);
    let short: String = name.chars().take(room).collect::<String>().trim_end_matches('_').to_string();
    if short.is_empty() {
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

/// The waves.ini sections a graph's wave tables read (`wave.table`'s section; "" is the mode's
/// built-in mix).
pub fn wave_sections_in_graph(g: &super::studio::graph::Graph) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in g.nodes.iter().filter(|n| n.kind == "wave.table") {
        let s = n.params.get("section").and_then(Value::as_str).unwrap_or("").trim().to_ascii_lowercase();
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Every type the named sections of a waves.ini use.
pub fn types_in_wave_sections(text: &str, sections: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for w in creatures::parse_waves(text).into_iter().filter(|w| sections.iter().any(|s| s.eq_ignore_ascii_case(&w.mode))) {
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

/// What a mode's graph needs packed: the types its nodes name and those of the waves.ini sections
/// its wave tables read (in the game at `game_dir`).
pub fn types_for_mode(g: &super::studio::graph::Graph, game_dir: Option<&Path>) -> Vec<String> {
    let mut t = types_in_graph(g);
    let secs = wave_sections_in_graph(g);
    if let (false, Some(d)) = (secs.is_empty(), game_dir) {
        let text = crate::sys::read_latin1(&d.join(creatures::WAVES_REL)).unwrap_or_default();
        t.extend(types_in_wave_sections(&text, &secs));
    }
    t.into_iter().collect()
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
    /// NPC-editor: an NPC's or interactable's custom model and sounds (game path in the package,
    /// file on disk), listed on the pack's `; npc:` line
    pub npc: Vec<(String, PathBuf)>,
    /// NPC-editor: their old paths -> new ones (under the namespace's folders), for the package's
    /// map entities and graphs
    pub npc_renames: BTreeMap<String, String>,
}

impl Pack {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.npc.is_empty()
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
        for (rel, p) in self.files.iter().chain(self.npc.iter().filter(|(r, _)| !self.files.iter().any(|(x, _)| x.eq_ignore_ascii_case(r)))) {
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
            .chain((!self.npc.is_empty()).then(|| format!("NPC files: {}", self.npc.iter().map(|(r, _)| r.rsplit('/').next().unwrap_or(r)).collect::<Vec<_>>().join(", "))))
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
    // (sprites: an Objective Marker's own)
    ((low.starts_with("models/") && low.ends_with(".mdl")) || (low.starts_with("sound/") && low.ends_with(".wav")) || (low.starts_with("sprites/") && low.ends_with(".spr")))
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

/// A custom file's game path in a package: under the namespace's own folder (models/<ns>/...,
/// sound/<ns>/...), so no download can land on another creature's or the game's files.
fn relocate(rel: &str, ns: &str) -> String {
    let low = rel.to_ascii_lowercase();
    for top in ["models/", "sound/", "sprites/"] {
        if let Some(rest) = low.strip_prefix(top) {
            if rest.starts_with(&format!("{ns}/")) {
                return low;
            }
            return format!("{top}{ns}/{rest}");
        }
    }
    low
}

/// A custom file (not the game's own) that's in the game's folders.
fn find_custom(hl: &Path, game: &str, rel: &str) -> bool {
    !super::doctor::is_stock(rel, game) && find_file(hl, game, rel).is_some()
}

/// A model or snd_ value with its custom files' paths moved under the namespace.
fn relocate_value(v: &str, model: bool, ns: &str, custom: &dyn Fn(&str) -> bool) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            let rel = if model { word.clone() } else { format!("sound/{word}") };
            if custom(&rel) {
                let moved = relocate(&rel, ns);
                out.push_str(if model { &moved } else { moved.strip_prefix("sound/").unwrap_or(&moved) });
            } else {
                out.push_str(word);
            }
            word.clear();
        }
    };
    for c in v.chars() {
        if c == ' ' || c == '|' {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// The Publish step's line for the custom creatures `names` use: {"includes": ["runner_x (with
/// its model)", ...]} or {"problem": why it can't be shared}.
pub fn listing(hl: &Path, game: &str, names: &[String]) -> Value {
    // (named under a typical member id here: the real one is the publisher's, on Make the package)
    match pack_types(hl, game, names, "gb1234567") {
        Ok(p) => {
            let carried: Vec<String> = p
                .types
                .iter()
                .filter(|t| is_shared_name(&t.from))
                .map(|t| {
                    let (ty, member) = display_name(&t.name);
                    let who = member.and_then(maker_name).unwrap_or_else(|| "someone else".into());
                    format!("{ty} is {who}'s: it can be listed once their own item is, and only unchanged.")
                })
                .collect();
            let mut notes = p.notes.clone();
            notes.extend(carried);
            serde_json::json!({"includes": p.summary(), "notes": notes})
        }
        Err(e) => serde_json::json!({"includes": [], "problem": e}),
    }
}

/// Who a member is, by the community list's entries under their namespace (the UI shows
/// "runner_x by <name>", never the id).
pub fn maker_name(member: i64) -> Option<String> {
    let ns = namespace_of(member)?;
    let _ = ns;
    super::community::index_cached().items.into_iter().find(|i| i.owner_id == member && !i.author.is_empty()).map(|i| i.author)
}

/// Packs the custom types `names` use (with their custom bases), named under `namespace` (from
/// `namespace_of` the publisher's member id). Built-ins,
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
            // (its files are its maker's: they come with the maker's own item, never in this one)
            types.push(PackedType { name: to.clone(), from: n.clone(), files: vec![] });
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
            // (file names in lower case: the community list and Linux servers match them so; a
            // custom file goes under the namespace's own folder, models/<ns>/ and sound/<ns>/)
            let v = if *k == "model" || k.starts_with("snd_") { relocate_value(&v.replace('\\', "/").to_ascii_lowercase(), *k == "model", &slug, &|r| find_custom(hl, game, r)) } else { v };
            body.push_str(&format!("{k} = {v}\n"));
        }
        let t = t.settled();
        let own: Vec<(String, PathBuf)> = custom_files(hl, game, &t).into_iter().map(|(r, p)| (relocate(&r, &slug), p)).collect();
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
    Ok(Pack { types, renames, text, files, notes, ..Default::default() })
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
    /// each type's own section text
    pub sections: Vec<(String, String)>,
    /// the files each type's model and snd_ values name (game paths)
    pub refs: BTreeMap<String, Vec<String>>,
    /// NPC-editor: NPC and interactable files (`; npc:`), named by the package's maps and graphs
    pub npc: Vec<String>,
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
        // NPC-editor: an NPC's or interactable's files (in its uploader's folders: checked on install)
        if let Some(f) = t.strip_prefix("; npc:") {
            for w in f.split_whitespace() {
                if !safe_rel(w) {
                    return Err(format!("The download names an NPC file it can't use ({w})."));
                }
                if !p.npc.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                    p.npc.push(w.to_ascii_lowercase());
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
                p.sections.push((n.clone(), format!("[{n}]\n")));
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
                    let refs = p.refs.entry(cur.clone().unwrap_or_default()).or_default();
                    for w in v.split([' ', '|']).filter(|w| !w.is_empty()) {
                        let w = w.to_ascii_lowercase();
                        refs.push(if model { w } else { format!("sound/{w}") });
                    }
                }
                if low == "base" && !is_builtin(&v) && !p.types.contains(&v.to_ascii_lowercase()) {
                    return Err(format!("A creature in the download is based on {v}, which it doesn't bring."));
                }
                p.body.push_str(&format!("{k} = {v}\n"));
                if let Some(last) = p.sections.last_mut() {
                    last.1.push_str(&format!("{k} = {v}\n"));
                }
            }
        }
    }
    // every listed file is one of its creatures' own, in that creature's maker's folder
    for f in &p.files {
        let low = f.to_ascii_lowercase();
        let users: Vec<&String> = p.refs.iter().filter(|(_, r)| r.iter().any(|x| *x == low)).map(|(t, _)| t).collect();
        if users.is_empty() {
            return Err(format!("The download lists a file none of its creatures use ({f})."));
        }
        let at_home = users.iter().any(|t| {
            let ns = t.split('.').next().unwrap_or("");
            low.starts_with(&format!("models/{ns}/")) || low.starts_with(&format!("sound/{ns}/"))
        });
        if !at_home {
            return Err(format!("The download's file {f} isn't in its creature maker's own folder."));
        }
    }
    if p.types.is_empty() && p.npc.is_empty() {
        return Err("The download's creature list is empty.".into());
    }
    if p.npc.len() > 64 {
        return Err("The download brings more than 64 NPC files.".into());
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
    /// the namespace of who uploaded it (gb<id>): only its own types are in the block
    pub owner: String,
    /// NPC-editor: the NPC and interactable files it needs (`; npc:`)
    pub npc: Vec<String>,
}

impl Block {
    fn render(&self) -> String {
        let mut s = format!("{BLOCK_START}{}{} (shared creatures: Loner-Strike adds and removes this block)\n", self.id, if self.joined { " +nl" } else { "" });
        if !self.owner.is_empty() {
            s.push_str(&format!("; owner: {}\n", self.owner));
        }
        s.push_str(&format!("; types: {}\n", self.types.join(" ")));
        if !self.files.is_empty() {
            s.push_str(&format!("; files: {}\n", self.files.join(" ")));
        }
        if !self.npc.is_empty() {
            s.push_str(&format!("; npc: {}\n", self.npc.join(" ")));
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
            } else if let Some(r) = t.strip_prefix("; owner:") {
                b.owner = r.trim().to_string();
            } else if let Some(r) = t.strip_prefix("; types:") {
                b.types = r.split_whitespace().map(String::from).collect();
            } else if let Some(r) = t.strip_prefix("; files:") {
                b.files = r.split_whitespace().map(String::from).collect();
            } else if let Some(r) = t.strip_prefix("; npc:") {
                b.npc = r.split_whitespace().map(String::from).collect();
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
/// `namespace`: the uploader's, as the community list's entry says (`gb<member id>`; never one the
/// GameBanana name): its types must be named under it (see `namespace_problem`).
pub fn check_pack(game_dir: &Path, text: &str, namespace: &str) -> Result<Parsed, String> {
    check_for(game_dir, text, namespace, "")
}

/// `replacing`: the package being updated (its own older block doesn't vouch for anything).
fn check_for(game_dir: &Path, text: &str, namespace: &str, replacing: &str) -> Result<Parsed, String> {
    let p = parse_pack(text)?;
    let (user, blocks) = split_blocks(&read_ini(game_dir)?);
    let own: BTreeSet<String> = sections_in(&user).into_iter().collect();
    if let Some(n) = p.types.iter().find(|t| own.contains(*t)) {
        return Err(format!("You already have a creature type called {n}, so this download's isn't added. Rename yours in the Creature Studio to use it."));
    }
    let others: Vec<Block> = blocks.into_iter().filter(|b| b.id != replacing).collect();
    if let Some(e) = namespace_problem(&p, &others, namespace) {
        return Err(e);
    }
    // never the game's own files
    let game = game_dir.file_name().map(|g| g.to_string_lossy().to_string()).unwrap_or_default();
    if let Some(f) = p.files.iter().chain(p.npc.iter()).find(|f| super::doctor::is_stock(f, &game)) {
        return Err(format!("The download brings {f}, one of the game's own files, so it wasn't installed."));
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
        // (only its maker's own block vouches for it)
        let mine = keys_of(&p.body, t);
        if installed.iter().any(|b| b.owner == author && b.types.iter().any(|x| x == t) && keys_of(&b.body, t) == mine) {
            continue;
        }
        let _ = author;
        return Some(format!("{t} isn't the uploader's (theirs are named {slug}.…) and isn't the same as that creator's installed one, so it wasn't added. Install that creator's own item first."));
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
    let (p, owner) = match publisher {
        Some(who) => (check_for(game_dir, text, who, id)?, who.to_string()),
        None => {
            // (a block already checked here: its own owner line says whose it is)
            let owner = split_blocks(text).1.first().map(|b| b.owner.clone()).unwrap_or_default();
            let p = parse_pack(text)?;
            let (user, _) = split_blocks(&read_ini(game_dir)?);
            if let Some(n) = p.types.iter().find(|t| sections_in(&user).contains(t)) {
                return Err(format!("There's already a creature type called {n} there, so it wasn't added."));
            }
            (p, owner)
        }
    };
    // only the uploader's own types go in its block: a carried copy of someone else's stays theirs
    // (their own block, which the check above found unchanged)
    let is_own = |t: &str| owner.is_empty() || t.split('.').next() == Some(owner.as_str());
    let own_types: Vec<String> = p.types.iter().filter(|t| is_own(t)).cloned().collect();
    let body: String = p.sections.iter().filter(|(t, _)| is_own(t)).map(|(_, s)| s.as_str()).collect();
    // every file it brings is in its uploader's own folders (models/gb<id>/, sound/gb<id>/)
    let in_owner = |f: &str| {
        let low = f.to_ascii_lowercase();
        owner.is_empty() || low.starts_with(&format!("models/{owner}/")) || low.starts_with(&format!("sound/{owner}/"))
    };
    if let Some(f) = p.files.iter().find(|f| !in_owner(f)) {
        return Err(format!("The download brings {f}, which isn't in its uploader's own folder, so it wasn't installed."));
    }
    let own_files: Vec<String> = p.files.iter().filter(|f| in_owner(f) && p.refs.iter().any(|(t, r)| is_own(t) && r.iter().any(|x| x.eq_ignore_ascii_case(f)))).cloned().collect();
    // NPC-editor: NPC files, each in the uploader's own folders and named by the package's own
    // maps or graphs (a downloaded package: its unpacked files say)
    if owner.is_empty() && !p.npc.is_empty() {
        return Err("Couldn't tell who uploaded this download, so its NPC files weren't added.".into());
    }
    if let Some(f) = p.npc.iter().find(|f| !in_owner(f)) {
        return Err(format!("The download brings {f}, which isn't in its uploader's own folder, so it wasn't installed."));
    }
    if let (Some(s), Some(_)) = (src, publisher) {
        let named = npc_refs_in(s);
        if let Some(f) = p.npc.iter().find(|f| !named.contains(&f.to_ascii_lowercase())) {
            return Err(format!("The download brings {f}, which none of its NPCs use, so it wasn't installed."));
        }
    }
    let own_npc: Vec<String> = p.npc.iter().filter(|f| !own_files.iter().any(|x| x.eq_ignore_ascii_case(f))).cloned().collect();
    let old = read_ini(game_dir)?;
    let (user, mut blocks) = split_blocks(&old);
    let prev = blocks.iter().position(|b| b.id == id).map(|i| blocks.remove(i));
    let mut done = Done { types: p.types.clone(), ..Default::default() };
    let mut wrote: Vec<String> = Vec::new();
    // (a map's files came in with the map: the store keeps them; nothing to copy or hand over)
    let copying: Vec<String> = if src.is_some() { own_files.iter().chain(own_npc.iter()).cloned().collect() } else { vec![] };
    for rel in &copying {
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
    if src.is_none() {
        done.missing = own_files.iter().chain(own_npc.iter()).filter(|f| !game_dir.join(f).is_file()).cloned().collect();
    }
    // (what it needs, so a file another block wrote passes to it before it'd go: only files in its
    // uploader's own folder, so no download can claim another maker's files)
    let block_files = own_files.clone();
    // the previous version's files: the new one keeps those it still needs (as its own), the rest
    // pass to another block that needs them, else go
    if let Some(b) = &prev {
        for w in &b.wrote {
            if block_files.iter().chain(own_npc.iter()).any(|x| x.eq_ignore_ascii_case(w)) && !wrote.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                wrote.push(w.clone());
            }
        }
        let gone: Vec<String> = b.wrote.iter().filter(|w| !wrote.iter().any(|x| x.eq_ignore_ascii_case(w))).cloned().collect();
        drop_files(game_dir, &gone, &mut blocks);
    }
    done.wrote = wrote.clone();
    blocks.push(Block { id: id.to_string(), types: own_types, files: block_files, wrote, joined: false, body, owner, npc: own_npc });
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
        if let Some(o) = others.iter_mut().find(|o| o.files.iter().chain(o.npc.iter()).any(|x| x.eq_ignore_ascii_case(f))) {
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
        let Some(mut b) = back.pop().filter(|b| b.id == id) else { return Err("The kept creature list couldn't be read.".into()) };
        // files the newer block wrote that the old one doesn't list go
        if let Some(n) = &now {
            for w in &n.wrote {
                if b.files.iter().any(|x| x.eq_ignore_ascii_case(w)) && !b.wrote.iter().any(|x| x.eq_ignore_ascii_case(w)) {
                    b.wrote.push(w.clone());
                }
            }
            let gone: Vec<String> = n.wrote.iter().filter(|w| !b.files.iter().any(|x| x.eq_ignore_ascii_case(w))).cloned().collect();
            drop_files(game_dir, &gone, &mut blocks);
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
    // (the credit: which shared type, so its maker is known; the copy is yours, named by you)
    let mut s = format!("; Remixed from {shared}\n[{new_name}]\nbase = {root}\n");
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
                b = pack.rewrite_entities(&String::from_utf8_lossy(&b)).into_bytes();
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

// ---- NPC-editor: NPCs' and interactables' own models and sounds --------------------------------
// They travel like a creature's files: a custom (not the game's) model or sound an ls_npc or an
// ls_interactable names, or a Game Mode Studio NPC node, goes in the package under the uploader's
// own folders (models/gb<id>/..., sound/gb<id>/...), listed on the pack's `; npc:` line; the
// package's maps and graphs are rewritten to match. On install each such file must be in its
// uploader's folders and named by the package's own maps or graphs; it's copied, recorded as the
// package's, and removed with it.

/// The entity classes whose models and sounds travel with a map.
pub const NPC_CLASSES: &[&str] = &["ls_npc", "ls_interactable", "ls_interactable_brush"];

/// A value that names a model or a sound: (game path, it's a sound named under sound/).
fn npc_file(v: &str) -> Option<(String, bool)> {
    let w = v.trim().replace('\\', "/").to_ascii_lowercase();
    if (w.starts_with("models/") && w.ends_with(".mdl")) || (w.starts_with("sprites/") && w.ends_with(".spr")) {
        return safe_rel(&w).then_some((w, false));
    }
    if w.ends_with(".wav") {
        let rel = if w.starts_with("sound/") { w.clone() } else { format!("sound/{w}") };
        return safe_rel(&rel).then_some((rel, !w.starts_with("sound/")));
    }
    None
}

/// The models and sounds the NPCs and interactables in an entity text name (game paths).
pub fn npc_files_in_entity_text(text: &str) -> BTreeSet<String> {
    let pairs = entity_pairs(text);
    let mut classes: BTreeMap<usize, String> = BTreeMap::new();
    for (e, k, v, _) in &pairs {
        if k == "classname" {
            classes.insert(*e, v.to_ascii_lowercase());
        }
    }
    let mut out = BTreeSet::new();
    for (e, k, v, _) in &pairs {
        if k == "classname" || !classes.get(e).is_some_and(|c| NPC_CLASSES.contains(&c.as_str())) {
            continue;
        }
        if let Some((rel, _)) = npc_file(v) {
            out.insert(rel);
        }
    }
    out
}

/// Graph nodes whose params can name an NPC's model or sound: NPC and interactable nodes, and a
/// component made at run time as one (`comp.create` of ls_npc / ls_interactable: its keys).
/// A Studio node whose settings may name a file that travels with the mode: every node (an
/// NPC's model, an Objective Marker's sprite, a prop's model, a sound a node plays: whatever the
/// asset pickers put in the game travels with the mode; only custom files are packed).
fn npc_node(n: &Value) -> bool {
    n["type"].is_string()
}

/// Words of a param value that may be paths (a comp.create's `k=v;k=v` keys, or a plain value).
fn value_words(v: &str) -> Vec<&str> {
    v.split([';', ' ', '|', '=', '"', ',']).filter(|w| !w.is_empty()).collect()
}

/// The models and sounds a Studio graph's NPC nodes name (game paths).
pub fn npc_files_in_graph_json(v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in v["nodes"].as_array().into_iter().flatten().filter(|n| npc_node(n)) {
        for (_, pv) in n["params"].as_object().into_iter().flatten() {
            for w in pv.as_str().map(value_words).unwrap_or_default() {
                if let Some((rel, _)) = npc_file(w) {
                    out.insert(rel);
                }
            }
        }
    }
    out
}

/// A text value with each path of `renames` (old game path -> new) put in place. Sounds named
/// without `sound/` stay so.
fn rename_paths(v: &str, renames: &BTreeMap<String, String>) -> String {
    let mut out = v.to_string();
    for (old, new) in renames {
        let (o, n) = if v.to_ascii_lowercase().contains(old.as_str()) {
            (old.as_str(), new.as_str())
        } else if let (Some(o), Some(n)) = (old.strip_prefix("sound/"), new.strip_prefix("sound/")) {
            (o, n)
        } else {
            continue;
        };
        // (whole words only: a path is never part of a longer one)
        let low = out.to_ascii_lowercase();
        let mut at = 0;
        let mut res = String::new();
        while let Some(i) = low[at..].find(o).map(|i| i + at) {
            let before = low[..i].chars().last();
            let after = low[i + o.len()..].chars().next();
            let edge = |c: Option<char>| c.is_none_or(|c| !(c.is_ascii_alphanumeric() || "_./-".contains(c)));
            res.push_str(&out[at..i]);
            res.push_str(if edge(before) && edge(after) { n } else { &out[i..i + o.len()] });
            at = i + o.len();
        }
        res.push_str(&out[at..]);
        out = res;
    }
    out
}

impl Pack {
    /// Adds the custom (not the game's own, found in its folders) models and sounds of `rels` as
    /// NPC files under the namespace's folders. A file a creature already brings goes once.
    pub fn with_npc(mut self, hl: &Path, game: &str, rels: &BTreeSet<String>, namespace: &str) -> Result<Pack, String> {
        let custom: Vec<(String, PathBuf)> = rels.iter().filter(|r| !super::doctor::is_stock(r, game)).filter_map(|r| find_file(hl, game, r).map(|p| (r.clone(), p))).collect();
        if custom.is_empty() {
            return Ok(self);
        }
        if !author_part(namespace) {
            return Err("That isn't a name files can be shared under.".into());
        }
        for (r, p) in custom {
            let to = relocate(&r, namespace);
            if to != r {
                self.npc_renames.insert(r.clone(), to.clone());
            }
            if !self.npc.iter().any(|(x, _)| x.eq_ignore_ascii_case(&to)) {
                self.npc.push((to, p));
            }
        }
        if self.text.is_empty() {
            self.text = String::from("; Loner-Strike shared creatures: the app adds these to creatures.ini when this is installed.\n; types:\n");
        }
        self.text.push_str(&format!("; npc: {}\n", self.npc.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>().join(" ")));
        Ok(self)
    }
    /// The package has something to rewrite in its maps or graphs.
    pub fn rewrites(&self) -> bool {
        !self.renames.is_empty() || !self.npc_renames.is_empty()
    }
    /// A map's entity text with creature types and NPC files renamed.
    pub fn rewrite_entities(&self, text: &str) -> String {
        let t = rewrite_entity_text(text, &self.renames);
        if self.npc_renames.is_empty() {
            return t;
        }
        let spans: Vec<(usize, usize)> = {
            let pairs = entity_pairs(&t);
            let mut classes: BTreeMap<usize, String> = BTreeMap::new();
            for (e, k, v, _) in &pairs {
                if k == "classname" {
                    classes.insert(*e, v.to_ascii_lowercase());
                }
            }
            pairs.into_iter().filter(|(e, k, _, _)| k != "classname" && classes.get(e).is_some_and(|c| NPC_CLASSES.contains(&c.as_str()))).map(|(_, _, _, sp)| sp).collect()
        };
        let mut out = t.clone();
        for (a, b) in spans.into_iter().rev() {
            let v = rename_paths(&t[a..b], &self.npc_renames);
            out.replace_range(a..b, &v);
        }
        out
    }
    /// A graph's creature types and NPC files renamed.
    pub fn rewrite_graph(&self, v: &mut Value) {
        rewrite_graph_json(v, &self.renames);
        if self.npc_renames.is_empty() {
            return;
        }
        for n in v["nodes"].as_array_mut().into_iter().flatten() {
            if !npc_node(n) {
                continue;
            }
            if let Some(o) = n["params"].as_object_mut() {
                for (_, pv) in o.iter_mut() {
                    if let Some(sv) = pv.as_str() {
                        *pv = Value::String(rename_paths(sv, &self.npc_renames));
                    }
                }
            }
        }
    }
}

/// Whether any of `rels` is a custom file (not the game's own) in the game's folders.
pub fn any_custom(hl: &Path, game: &str, rels: &BTreeSet<String>) -> bool {
    rels.iter().any(|r| find_custom(hl, game, r))
}

/// The models and sounds an unpacked package's own maps (.bsp entity lumps, prefab .map files)
/// and graphs name for NPCs: an NPC file it brings must be one of these.
pub fn npc_refs_in(src: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for f in crate::archive::walk(src).into_iter().take(4000) {
        let name = f.to_string_lossy().to_ascii_lowercase();
        let Ok(meta) = std::fs::symlink_metadata(&f) else { continue };
        if !meta.is_file() || meta.len() > 256 << 20 {
            continue;
        }
        if name.ends_with(".bsp") {
            if let Some(t) = std::fs::read(&f).ok().and_then(|b| bsp_entities(&b)) {
                out.extend(npc_files_in_entity_text(&t));
            }
        } else if name.ends_with(".map") && meta.len() <= 64 << 20 {
            if let Ok(b) = std::fs::read(&f) {
                out.extend(npc_files_in_entity_text(&String::from_utf8_lossy(&b)));
            }
        } else if name.ends_with(".graph.json") && meta.len() <= 2 << 20 {
            if let Some(v) = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                out.extend(npc_files_in_graph_json(&v));
            }
        }
    }
    out
}

/// The NPC models and sounds in a zip's .map files (a prefab package before it's repacked).
pub fn npc_files_in_zip_maps(zip_path: &Path) -> Result<BTreeSet<String>, String> {
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
        out.extend(npc_files_in_entity_text(&String::from_utf8_lossy(&b)));
    }
    Ok(out)
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
        assert_eq!(namespace_of(123456).as_deref(), Some("gb123456"));
        assert_eq!(namespace_of(0), None);
        assert_eq!(clean_namespace("gb123456"), "gb123456");
        for bad in ["gb0", "gb", "gb012", "jam", "gb12x", "GB12"] {
            assert_eq!(clean_namespace(bad), "", "{bad}");
        }
        assert_eq!(display_name("gb123456.runner_x"), ("runner_x".to_string(), Some(123456)));
        assert_eq!(display_name("runner_x"), ("runner_x".to_string(), None));
        assert_eq!(member_from_link("https://gamebanana.com/members/123456/"), Some(123456));
        assert_eq!(member_from_link(" 98765 "), Some(98765));
        assert_eq!(member_from_link("https://evil.example/members/1"), None);
        assert_eq!(member_from_link("abc"), None);
        assert!(is_shared_name("gb123456.runner_x"));
        assert_eq!(shared_name("gb11", "runner_x").unwrap(), "gb11.runner_x");
        assert_eq!(shared_name("gb123456789", "a_rather_long_name_with_more").unwrap(), "gb123456789.a_rather_long_name", "the type is cut, never the id");
        assert_eq!(shared_name("gb123456789", "short").unwrap(), "gb123456789.short");
        assert_eq!(shared_name("gb11", "gb22.x").unwrap(), "gb22.x", "an installed shared type keeps its name");
        assert_eq!(shared_name("gb11", &"x".repeat(29)).unwrap().len(), 31);
        // exactly the list's rule
        for ok in ["gb1.a", "gb123456789.x", "gb5.123"] {
            assert!(is_shared_name(ok), "{ok}");
        }
        for bad in ["gb0.x", "gb1234567890.x", "jam.x", "gb1.X", "gb1.", "gb1.a.b", "gb01.x", "gb1.a-b"] {
            assert!(!is_shared_name(bad), "{bad}");
        }
        assert!(!is_shared_name("runner_x") && !is_shared_name("a.b.c") && !is_shared_name("A.b"));
    }

    #[test]
    fn packs_custom_types_with_bases_files_and_renames() {
        let (hl, gd) = game(INI);
        std::fs::create_dir_all(gd.join("models/qa")).unwrap();
        std::fs::write(gd.join("models/qa/runner_x.mdl"), b"IDST model").unwrap();
        std::fs::create_dir_all(gd.join("sound/qa")).unwrap();
        std::fs::write(gd.join("sound/qa/die.wav"), b"RIFF").unwrap();
        let p = pack_types(&hl, "cstrike", &["runner_x".into(), "zombie".into(), "all".into()], "gb11").unwrap();
        assert_eq!(p.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["gb11.runner_x", "gb11.tough_base"]);
        assert_eq!(p.renames["runner_x"], "gb11.runner_x");
        assert!(p.text.contains("[gb11.tough_base]\nbase = brute\nhp = 900\n[gb11.runner_x]\nbase = gb11.tough_base\n"), "{}", p.text);
        assert!(!p.text.contains("[zombie]") && !p.text.contains("fast"));
        assert_eq!(p.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["models/gb11/qa/runner_x.mdl", "sound/gb11/qa/die.wav"], "the game's own zombie.mdl isn't packed; custom files go under the namespace");
        assert!(p.text.contains("model = models/gb11/qa/runner_x.mdl models/zombie.mdl\n") && p.text.contains("snd_die = gb11/qa/die.wav\n"), "{}", p.text);
        assert!(parse_pack(&p.text).is_ok());
        assert_eq!(p.summary()[0], "runner_x (with its model and 1 sound)");
        assert!(pack_types(&hl, "cstrike", &["ghost".into()], "gb11").unwrap_err().contains("isn't one of your creature types"));
        assert!(pack_types(&hl, "cstrike", &["zombie".into()], "gb11").unwrap().is_empty());

        // the package's references follow the new names
        let mut g = serde_json::json!({"nodes": [{"id": "a", "type": "creature.spawn", "params": {"kind": "runner_x"}}, {"id": "e", "type": "comp.create", "params": {"kind": "ls_wave_spawner", "keys": "types=runner_x brute waves=3"}}]});
        rewrite_graph_json(&mut g, &p.renames);
        assert_eq!(g["nodes"][0]["params"]["kind"], "gb11.runner_x");
        assert_eq!(g["nodes"][1]["params"]["keys"], "types=gb11.runner_x brute waves=3");
        let ents = "{\n\"classname\" \"ls_wave_spawner\"\n\"types\" \"runner_x zombie\"\n\"targetname\" \"runner_x\"\n}\n";
        assert_eq!(rewrite_entity_text(ents, &p.renames), "{\n\"classname\" \"ls_wave_spawner\"\n\"types\" \"gb11.runner_x zombie\"\n\"targetname\" \"runner_x\"\n}\n");
        // the map's entity lump, replaced
        let mut bsp = vec![0u8; 124];
        bsp[0] = 30;
        bsp[4..8].copy_from_slice(&124u32.to_le_bytes());
        bsp[8..12].copy_from_slice(&(ents.len() as u32).to_le_bytes());
        bsp.extend_from_slice(ents.as_bytes());
        let new = bsp_with_entities(&bsp, &rewrite_entity_text(ents, &p.renames)).unwrap();
        assert!(bsp_entities(&new).unwrap().contains("gb11.runner_x zombie"));
        assert!(bsp_with_entities(b"nope", "").is_err());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn refuses_built_ins_and_names_not_for_who_made_them() {
        assert!(parse_pack("[zombie]\nhp = 5\n").unwrap_err().contains("built-in creature zombie"));
        assert!(parse_pack("[runner_x]\nhp = 5\n").unwrap_err().contains("aren't named for who made them"));
        assert!(parse_pack("[gb11.x]\nbase = gb22.y\n").unwrap_err().contains("doesn't bring"));
        assert!(parse_pack("[gb11.x]\nrun_shell = 1\n").unwrap_err().contains("doesn't know"));
        assert!(parse_pack("; files: ../../x.mdl\n[gb11.x]\n").unwrap_err().contains("can't use"));
        assert!(parse_pack("; files: models/con.mdl\n[gb11.x]\n").is_err());
        let p = parse_pack("; types: gb11.x\n; files: models/gb11/x.mdl\n[gb11.x]\nbase = runner\nhp = 5\nmodel = models/gb11/x.mdl\n").unwrap();
        assert_eq!((p.types, p.files), (vec!["gb11.x".to_string()], vec!["models/gb11/x.mdl".to_string()]));
    }

    #[test]
    fn install_and_uninstall_keep_the_players_part_and_count_users() {
        let mine = "; my creatures\r\n[pet]\r\nbase = hound\r\nhp = 5"; // CRLF, no final line break
        let (hl, gd) = game(mine);
        let src = hl.join("pkg");
        std::fs::create_dir_all(src.join("models/gb11")).unwrap();
        std::fs::write(src.join("models/gb11/x.mdl"), b"IDST x").unwrap();
        let pack = "; types: gb11.x\n; files: models/gb11/x.mdl\n[gb11.x]\nbase = runner\nhp = 50\nmodel = models/gb11/x.mdl\n";
        let d = install_pack(&gd, "mod-1", pack, Some(&src), "gb11").unwrap();
        assert_eq!(d.wrote, vec!["models/gb11/x.mdl"]);
        assert!(gd.join("models/gb11/x.mdl").is_file());
        let text = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert!(text.starts_with(mine) && text.contains("; >>> from mod-1 +nl") && text.contains("[gb11.x]\nbase = runner\nhp = 50\nmodel = models/gb11/x.mdl\n; <<< end mod-1\n"), "{text}");
        // the game (and the Studio) read the type
        let r = creatures::resolve(&creatures::Doc::parse(&text));
        assert_eq!(r.iter().find(|x| x.t.name == "gb11.x").unwrap().t.hp, 50.0);
        assert_eq!(r.iter().find(|x| x.t.name == "pet").unwrap().t.hp, 5.0);

        // a second package that shares the type and the model
        install_pack(&gd, "mod-2", pack, None, "gb11").unwrap();
        assert_eq!(installed_types(&gd)["gb11.x"], vec!["mod-1", "mod-2"]);
        // the first goes: the type and the model stay for the second
        assert!(uninstall_pack(&gd, "mod-1").unwrap().is_empty());
        assert!(gd.join("models/gb11/x.mdl").is_file());
        assert_eq!(installed_types(&gd)["gb11.x"], vec!["mod-2"]);
        // the second goes: everything of ours is gone, the player's part is byte for byte as it was
        assert_eq!(uninstall_pack(&gd, "mod-2").unwrap(), vec!["models/gb11/x.mdl"]);
        assert!(!gd.join("models/gb11/x.mdl").exists());
        assert_eq!(std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap(), mine);
        assert!(uninstall_pack(&gd, "mod-9").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn never_overwrites_the_players_files_or_types() {
        let (hl, gd) = game("[gb11.x]\nhp = 1\n");
        let pack = "; files: models/gb11/x.mdl\n[gb11.x]\nbase = runner\nmodel = models/gb11/x.mdl\n";
        assert!(install_pack(&gd, "mod-1", pack, None, "gb11").unwrap_err().contains("already have a creature type called gb11.x"));
        std::fs::write(gd.join(creatures::FILE_REL), "").unwrap();
        std::fs::create_dir_all(gd.join("models/gb11")).unwrap();
        std::fs::write(gd.join("models/gb11/x.mdl"), b"theirs").unwrap();
        let src = hl.join("pkg");
        std::fs::create_dir_all(src.join("models/gb11")).unwrap();
        std::fs::write(src.join("models/gb11/x.mdl"), b"ours").unwrap();
        let d = install_pack(&gd, "mod-1", pack, Some(&src), "gb11").unwrap();
        assert_eq!(d.kept, vec!["models/gb11/x.mdl"]);
        uninstall_pack(&gd, "mod-1").unwrap();
        assert_eq!(std::fs::read(gd.join("models/gb11/x.mdl")).unwrap(), b"theirs");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn updates_replace_the_block_and_go_back_restores_it() {
        let (hl, gd) = game("[pet]\nbase = hound\n");
        let v1 = "[gb11.x]\nbase = runner\nhp = 50\n";
        let v2 = "[gb11.x]\nbase = runner\nhp = 80\n[gb11.y]\nbase = gb11.x\n";
        install_pack(&gd, "mod-1", v1, None, "gb11").unwrap();
        let kept = block_text(&gd, "mod-1");
        install_pack(&gd, "mod-1", v2, None, "gb11").unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert_eq!(t.matches("; >>> from mod-1").count(), 1, "one block per package");
        assert!(t.contains("hp = 80") && t.contains("[gb11.y]"));
        // Go back (or an update that failed): the old block as it was
        restore_block(&gd, "mod-1", kept.as_deref()).unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        assert!(t.contains("hp = 50") && !t.contains("[gb11.y]"));
        assert!(t.starts_with("[pet]\nbase = hound\n"));
        // nothing kept: the block goes
        restore_block(&gd, "mod-1", None).unwrap();
        assert_eq!(std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap(), "[pet]\nbase = hound\n");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn remix_makes_a_shared_type_your_own() {
        let (hl, gd) = game("[pet]\nbase = hound\n");
        install_pack(&gd, "mod-1", "[gb11.base]\nbase = brute\nhp = 900\n[gb11.x]\nbase = gb11.base\nspeed = 320\n", None, "gb11").unwrap();
        remix_type(&gd, "gb11.x", "my_x").unwrap();
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        let (user, blocks) = split_blocks(&t);
        assert!(user.contains("; Remixed from gb11.x\n[my_x]\nbase = brute\nhp = 900\nspeed = 320\n"), "{user}");
        assert_eq!(blocks.len(), 1);
        let r = creatures::resolve(&creatures::Doc::parse(&t));
        let x = r.iter().find(|x| x.t.name == "my_x").unwrap();
        assert_eq!((x.t.hp, x.t.speed), (900.0, 320.0));
        assert!(remix_type(&gd, "gb11.x", "pet").is_err());
        assert!(remix_type(&gd, "gb11.nope", "other").is_err());
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
                let t = format!("[gb11.x]\n{k} = {v}\n");
                assert!(parse_pack(&t).is_err(), "{k} = {v} got through");
            }
        }
        // the aliases and ls = / sounds = aren't taken at all
        for k in ["ls = model=models/gb11/x.mdl", "sounds = jam/a.wav", "models = models/gb11/x.mdl", "health = 5"] {
            assert!(parse_pack(&format!("[gb11.x]\n{k}\n")).is_err(), "{k}");
        }
        assert!(parse_pack("[gb11.x]\nmodel = jam/x.mdl\n").is_err(), "a model outside models/");
        assert!(parse_pack("[gb11.x]\nsnd_die = jam/x.mp3\n").is_err());
        assert!(parse_pack("[gb11.x]\nmodel = models/gb11/x.mdl | models/zombie.mdl\nsnd_die = jam/a.wav jam/b.wav | player/die1.wav\n").is_ok());
    }

    #[test]
    fn listed_files_are_only_the_creatures_own() {
        // a file no creature names, a file in someone else's folder, the game's own files
        assert!(parse_pack("; files: models/gb11/x.mdl\n[gb11.x]\nhp = 1\n").unwrap_err().contains("none of its creatures use"));
        assert!(parse_pack("; files: models/gb22/x.mdl\n[gb11.x]\nmodel = models/gb22/x.mdl\n").unwrap_err().contains("maker's own folder"));
        assert!(parse_pack("; files: models/player/gign/gign.mdl\n[gb11.x]\nmodel = models/player/gign/gign.mdl\n").unwrap_err().contains("maker's own folder"));
        assert!(parse_pack("; files: sound/gb11/a.wav\n[gb11.x]\nsnd_die = gb11/a.wav\n").is_ok());
        let (hl, gd) = game("");
        assert!(check_pack(&gd, "; files: models/gb11/x.mdl\n[gb11.x]\nmodel = models/gb11/x.mdl\n", "gb11").is_ok());
        // a carried copy of someone else's type brings none of their files, and isn't written again
        install_pack(&gd, "mod-1", "[gb22.y]\nbase = runner\nhp = 5\n", None, "gb22").unwrap();
        let src = hl.join("pk");
        std::fs::create_dir_all(src.join("models/gb22")).unwrap();
        std::fs::write(src.join("models/gb22/y.mdl"), b"x").unwrap();
        let d = install_pack(&gd, "mod-2", "[gb22.y]\nbase = runner\nhp = 5\n[gb11.z]\nbase = gb22.y\n", Some(&src), "gb11").unwrap();
        assert!(d.wrote.is_empty());
        let (_, blocks) = split_blocks(&std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap());
        let b2 = blocks.iter().find(|b| b.id == "mod-2").unwrap();
        assert_eq!((b2.types.clone(), b2.owner.as_str()), (vec!["gb11.z".to_string()], "gb11"));
        assert!(!b2.body.contains("[gb22.y]"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn a_pack_only_brings_files_in_its_uploaders_folders() {
        let (hl, gd) = game("");
        install_pack(&gd, "mod-1", "[gb22.y]\nbase = runner\nmodel = models/gb22/y.mdl\n", None, "gb22").unwrap();
        // gb11's pack carrying gb22's type and listing gb22's file: refused (it's gb22's to bring)
        let t = "; files: models/gb22/y.mdl\n[gb22.y]\nbase = runner\nmodel = models/gb22/y.mdl\n[gb11.z]\nbase = gb22.y\n";
        assert!(parse_pack(t).is_ok(), "the list rule alone lets a carried type's file through");
        assert!(install_pack(&gd, "mod-2", t, None, "gb11").unwrap_err().contains("isn't in its uploader's own folder"));
        // its own folder: fine
        install_pack(&gd, "mod-3", "; files: models/gb11/z.mdl\n[gb11.z]\nmodel = models/gb11/z.mdl\n", None, "gb11").unwrap();
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn a_handed_over_file_stays_with_the_block_that_needs_it() {
        let (hl, gd) = game("");
        let src = hl.join("pk");
        std::fs::create_dir_all(src.join("models/gb11")).unwrap();
        std::fs::write(src.join("models/gb11/x.mdl"), b"m").unwrap();
        let with = "; files: models/gb11/x.mdl\n[gb11.x]\nmodel = models/gb11/x.mdl\n";
        install_pack(&gd, "mod-1", with, Some(&src), "gb11").unwrap();
        install_pack(&gd, "mod-2", with, Some(&src), "gb11").unwrap();
        // mod-1's update no longer uses the model: mod-2 (which needs it) now owns it, on disk
        install_pack(&gd, "mod-1", "[gb11.x]\nhp = 5\n", None, "gb11").unwrap();
        let (_, blocks) = split_blocks(&std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap());
        assert_eq!(blocks.iter().find(|b| b.id == "mod-2").unwrap().wrote, vec!["models/gb11/x.mdl"]);
        assert!(gd.join("models/gb11/x.mdl").is_file());
        // so when mod-2 goes, so does the model
        assert_eq!(uninstall_pack(&gd, "mod-2").unwrap(), vec!["models/gb11/x.mdl"]);
        // Go back: the kept block takes back the file it lists from the newer one
        let kept = block_text(&gd, "mod-1");
        install_pack(&gd, "mod-1", with, Some(&src), "gb11").unwrap();
        restore_block(&gd, "mod-1", kept.as_deref()).unwrap();
        assert!(!gd.join("models/gb11/x.mdl").exists(), "the kept version doesn't need it: gone");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn no_download_plants_files_in_a_makers_folder() {
        assert!(crate::archive::creature_folder("models/gb123/x.mdl") && crate::archive::creature_folder("sound/gb9/a/b.wav") && crate::archive::creature_folder("MODELS\\GB1\\x.mdl"));
        assert!(!crate::archive::creature_folder("models/gbx/x.mdl") && !crate::archive::creature_folder("models/gb/x.mdl") && !crate::archive::creature_folder("models/x.mdl"));
        let rels: Vec<Vec<String>> = ["maps/m.bsp", "models/gb123/runner.mdl", "sound/gb123/die.wav", "models/mine/x.mdl"].iter().map(|r| r.split('/').map(String::from).collect()).collect();
        let (plan, skipped) = crate::archive::plan_rels(&rels);
        let dests: Vec<&str> = plan.iter().map(|(_, d)| d.as_str()).collect();
        assert!(dests.contains(&"maps/m.bsp") && dests.contains(&"models/mine/x.mdl"), "{dests:?}");
        assert!(!dests.iter().any(|d| d.contains("gb123")), "{dests:?}");
        assert_eq!(skipped.iter().filter(|x| x.contains("creature maker's own folder")).count(), 2, "{skipped:?}");
        // a marker zip from any page is checked strictly (plain ASCII names)
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("ls-plant-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("o")).unwrap();
        let zp = dir.join("m.zip");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for n in ["creatures/creatures.ini", "models/caf\u{e9}.mdl"] {
                z.start_file(n, o).unwrap();
                z.write_all(b"x").unwrap();
            }
            z.finish().unwrap();
        }
        assert!(crate::archive::extract(&zp, &dir.join("o"), 1 << 20).unwrap_err().contains("only letters, digits"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_update_cant_vouch_for_its_own_stale_copy() {
        let (hl, gd) = game("");
        // the owner's v2, and a package that carried v1 of it
        install_pack(&gd, "mod-1", "[gb22.y]\nbase = runner\nhp = 5\n", None, "gb22").unwrap();
        install_pack(&gd, "mod-2", "[gb22.y]\nbase = runner\nhp = 5\n[gb11.z]\nbase = gb22.y\n", None, "gb11").unwrap();
        install_pack(&gd, "mod-1", "[gb22.y]\nbase = runner\nhp = 9\n", None, "gb22").unwrap();
        // re-installing the old carried copy doesn't pin it again
        assert!(install_pack(&gd, "mod-2", "[gb22.y]\nbase = runner\nhp = 5\n[gb11.z]\nbase = gb22.y\n", None, "gb11").unwrap_err().contains("isn't the uploader's"));
        // and a block that isn't the owner's never vouches (one written by hand, say)
        let t = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        let (u, mut b) = split_blocks(&t);
        b.push(Block { id: "mod-3".into(), owner: "gb33".into(), types: vec!["gb44.w".into()], body: "[gb44.w]\nbase = runner\n".into(), ..Default::default() });
        std::fs::write(gd.join(creatures::FILE_REL), join_blocks(&u, &b)).unwrap();
        assert!(install_pack(&gd, "mod-4", "[gb44.w]\nbase = runner\n", None, "gb55").unwrap_err().contains("isn't the uploader's"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn one_package_never_replaces_anothers_file() {
        let (hl, gd) = game("");
        let mk = |dir: &str, body: &[u8]| {
            let src = hl.join(dir);
            std::fs::create_dir_all(src.join("models/gb11")).unwrap();
            std::fs::write(src.join("models/gb11/x.mdl"), body).unwrap();
            src
        };
        let pack = "; files: models/gb11/x.mdl\n[gb11.x]\nmodel = models/gb11/x.mdl\n";
        install_pack(&gd, "mod-1", pack, Some(&mk("a", b"from A")), "gb11").unwrap();
        let d = install_pack(&gd, "mod-2", pack, Some(&mk("b", b"from B")), "gb11").unwrap();
        assert_eq!(d.kept, vec!["models/gb11/x.mdl"]);
        assert_eq!(std::fs::read(gd.join("models/gb11/x.mdl")).unwrap(), b"from A");
        // A's own update may replace its file
        install_pack(&gd, "mod-1", pack, Some(&mk("c", b"A v2")), "gb11").unwrap();
        assert_eq!(std::fs::read(gd.join("models/gb11/x.mdl")).unwrap(), b"A v2");
        // A goes: B still needs the file, so it stays (now B's to remove)
        uninstall_pack(&gd, "mod-1").unwrap();
        assert!(gd.join("models/gb11/x.mdl").is_file());
        assert_eq!(uninstall_pack(&gd, "mod-2").unwrap(), vec!["models/gb11/x.mdl"]);
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn types_are_bound_to_who_uploaded_them() {
        let (hl, gd) = game("");
        let pack = "[gb11.x]\nbase = runner\nhp = 50\n";
        assert!(install_pack(&gd, "mod-1", pack, None, "gb22").unwrap_err().contains("isn't the uploader's"));
        assert!(install_pack(&gd, "mod-1", pack, None, "").is_err() && install_pack(&gd, "mod-1", pack, None, "Jam").is_err());
        install_pack(&gd, "mod-1", pack, None, "gb11").unwrap();
        // Bob's mode carries gb11.x: fine when it's the same as the one installed from Jam
        install_pack(&gd, "mod-2", "[gb11.x]\nbase = runner\nhp = 50\n[gb22.y]\nbase = gb11.x\n", None, "gb22").unwrap();
        // ... but not a changed copy (that would replace Jam's creature)
        assert!(install_pack(&gd, "mod-3", "[gb11.x]\nbase = runner\nhp = 9999\n", None, "gb22").unwrap_err().contains("isn't the uploader's"));
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn someone_elses_type_is_carried_unchanged() {
        let (hl, gd) = game("[mine]\nbase = gb22.y\nhp = 5\n");
        let bob = "[gb22.y]\nbase = runner\nhp = 44\nspeed = 300\n";
        install_pack(&gd, "mod-7", bob, None, "gb22").unwrap();
        let p = pack_types(&hl, "cstrike", &["mine".into()], "gb11").unwrap();
        assert_eq!(p.types.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["gb11.mine", "gb22.y"]);
        assert!(p.text.contains("[gb22.y]\nbase = runner\nhp = 44\nspeed = 300\n[gb11.mine]\nbase = gb22.y\n"), "{}", p.text);
        let l = listing(&hl, "cstrike", &["mine".into()]);
        assert_eq!(l["notes"][0], "y is someone else's: it can be listed once their own item is, and only unchanged.");
        // installing Jam's package elsewhere: only where Bob's own is in, unchanged
        let (hl2, other) = game("");
        assert!(install_pack(&other, "mod-8", &p.text, None, "gb11").unwrap_err().contains("isn't the uploader's"));
        install_pack(&other, "mod-7", bob, None, "gb22").unwrap();
        install_pack(&other, "mod-8", &p.text, None, "gb11").unwrap();
        let r = creatures::resolve(&creatures::Doc::parse(&std::fs::read_to_string(other.join(creatures::FILE_REL)).unwrap()));
        assert_eq!(r.iter().find(|x| x.t.name == "gb11.mine").unwrap().t.hp, 5.0);
        let _ = std::fs::remove_dir_all(&hl);
        let _ = std::fs::remove_dir_all(&hl2);
    }

    #[test]
    fn community_downloads_unpack_strictly() {
        use std::io::Write;
        let names = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let problem = |l: &[&str]| crate::archive::strict_problem(&names(l));
        assert!(problem(&["loner-strike.json", "creatures/", "creatures/creatures.ini", "models/gb11/x.mdl", "maps/My Map 2.bsp"]).is_none());
        // twins (case, back slashes, ./ and //, Unicode case and NFC/NFD forms) are all refused
        for twin in [
            ["creatures/creatures.ini", "Creatures/creatures.INI"],
            ["creatures/creatures.ini", "creatures\\creatures.ini"],
            ["creatures/creatures.ini", "./creatures//creatures.ini"],
            ["models/\u{e9}.mdl", "models/\u{c9}.mdl"],
            ["models/caf\u{e9}.mdl", "models/cafe\u{301}.mdl"],
        ] {
            assert!(problem(&twin).is_some(), "{twin:?}");
        }
        // names that could land somewhere else on some system
        for bad in ["a/../x.mdl", "../x.mdl", "/etc/x", "C:\\x.mdl", "models/a:b.mdl", "models/x.mdl.", "models/x.mdl ", "models/PROGRA~1/x.mdl", "models/con.mdl", "models/LPT1", "sound/aux/x.wav", "models//x.mdl", "models/./x.mdl"] {
            assert!(problem(&[bad]).is_some(), "{bad}");
        }
        // markers: an archive carrying a file an installer acts on is checked the same way
        assert!(crate::archive::has_marker(&names(&["x/loner-strike.json"])) && crate::archive::has_marker(&names(&["creatures/creatures.ini"])) && crate::archive::has_marker(&names(&["source/m.map"])));
        assert!(!crate::archive::has_marker(&names(&["maps/x.bsp"])));

        let dir = std::env::temp_dir().join(format!("ls-strict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip_of = |name: &str, files: &[(&str, &str)]| {
            let zp = dir.join(name);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for (n, b) in files {
                z.start_file(*n, o).unwrap();
                z.write_all(b.as_bytes()).unwrap();
            }
            z.finish().unwrap();
            zp
        };
        let fresh = |n: &str| {
            let d = dir.join(n);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        // a twin is refused before anything unpacks: without loner-strike.json too (the marker)
        let zp = zip_of("twin.zip", &[("creatures/creatures.ini", "[gb11.x]\n"), ("CREATURES/creatures.ini", "[gb11.x]\nhp = 1\n")]);
        assert!(crate::archive::extract(&zp, &fresh("o1"), 1 << 20).unwrap_err().contains("two files"));
        assert!(crate::archive::extract_strict(&zp, &fresh("o2"), 1 << 20).unwrap_err().contains("two files"));
        // Community downloads: strict even with no marker, and never over a file already written
        let zp = zip_of("plain.zip", &[("maps/x.bsp", "a"), ("maps/x.bsp.", "b")]);
        assert!(crate::archive::extract_strict(&zp, &fresh("o3"), 1 << 20).is_err());
        assert!(crate::archive::extract(&zp, &fresh("o4"), 1 << 20).is_err(), "aliasing names: refused in every download");
        let zp = zip_of("ok.zip", &[("loner-strike.json", "{}"), ("creatures/creatures.ini", "[gb11.x]\n")]);
        let o = fresh("o5");
        crate::archive::extract_strict(&zp, &o, 1 << 20).unwrap();
        assert!(o.join("creatures/creatures.ini").is_file());
        // a tarball carrying a marker (no loner-strike.json) is refused and cleared; as a
        // Community download, any tarball is
        if Path::new("/usr/bin/tar").exists() {
            let pk = dir.join("pk");
            std::fs::create_dir_all(pk.join("creatures")).unwrap();
            std::fs::write(pk.join("creatures/creatures.ini"), "[gb11.x]\n").unwrap();
            let tp = dir.join("p.tar");
            assert!(std::process::Command::new("/usr/bin/tar").arg("-cf").arg(&tp).arg("-C").arg(&pk).arg(".").status().unwrap().success());
            let out = fresh("o6");
            assert!(crate::archive::extract(&tp, &out, 1 << 20).unwrap_err().contains("come as .zip"));
            assert!(!out.join("creatures/creatures.ini").exists());
            assert!(crate::archive::extract_strict(&tp, &fresh("o7"), 1 << 20).unwrap_err().contains("come as .zip"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_archive_is_checked_on_the_paths_it_writes() {
        use std::io::Write;
        fn crc32(b: &[u8]) -> u32 {
            let mut c = !0u32;
            for x in b {
                c ^= *x as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
                }
            }
            !c
        }
        let dir = std::env::temp_dir().join(format!("ls-every-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fresh = |n: &str| {
            let d = dir.join(n);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        // a Unicode name field that renames the entry: refused, any download
        // (built by hand: the zip crate won't write a field that renames its entry)
        let zp = dir.join("uni.zip");
        {
            let name: &[u8] = b"maps/a.bsp";
            let mut extra = vec![0x75u8, 0x70];
            let mut data = vec![1u8];
            data.extend_from_slice(&crc32(name).to_le_bytes());
            data.extend_from_slice(b"creatures/creatures.ini");
            extra.extend_from_slice(&(data.len() as u16).to_le_bytes());
            extra.extend_from_slice(&data);
            let body: &[u8] = b"x";
            let crc = crc32(body);
            let le16 = |v: u16| v.to_le_bytes().to_vec();
            let le32 = |v: u32| v.to_le_bytes().to_vec();
            let mut b: Vec<u8> = Vec::new();
            for x in [le32(0x0403_4b50), le16(20), le16(0), le16(0), le16(0), le16(0), le32(crc), le32(1), le32(1), le16(name.len() as u16), le16(extra.len() as u16)] {
                b.extend(x);
            }
            b.extend_from_slice(name);
            b.extend_from_slice(&extra);
            b.extend_from_slice(body);
            let cd = b.len() as u32;
            for x in [le32(0x0201_4b50), le16(20), le16(20), le16(0), le16(0), le16(0), le16(0), le32(crc), le32(1), le32(1), le16(name.len() as u16), le16(extra.len() as u16), le16(0), le16(0), le16(0), le32(0), le32(0)] {
                b.extend(x);
            }
            b.extend_from_slice(name);
            b.extend_from_slice(&extra);
            let cd_len = b.len() as u32 - cd;
            for x in [le32(0x0605_4b50), le16(0), le16(0), le16(1), le16(1), le32(cd_len), le32(cd), le16(0)] {
                b.extend(x);
            }
            std::fs::write(&zp, &b).unwrap();
            let mut z = zip::ZipArchive::new(std::fs::File::open(&zp).unwrap()).unwrap();
            assert_eq!(z.by_index(0).unwrap().name().unwrap(), "creatures/creatures.ini", "the reader takes the Unicode name");
        }
        assert!(crate::archive::extract(&zp, &fresh("u"), 1 << 20).unwrap_err().contains("doesn't add up"));
        // aliasing names in a plain GameBanana-style zip (no marker): refused too
        for bad in ["maps/x.bsp.", "maps/x.bsp ", "models/PROGRA~1/x.mdl", "models/con.mdl", "creatures/creatures.ini."] {
            let zp = dir.join("alias.zip");
            {
                let mut z = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
                z.start_file(bad, zip::write::SimpleFileOptions::default()).unwrap();
                z.write_all(b"x").unwrap();
                z.finish().unwrap();
            }
            let out = fresh(&format!("a{}", bad.len()));
            assert!(crate::archive::extract(&zp, &out, 1 << 20).is_err(), "{bad}");
        }
        assert!(crate::archive::written_name_problem("maps\\x.bsp".as_bytes(), "maps/x.bsp").is_none(), "slash style only");
        assert!(crate::archive::written_name_problem(b"caf\x82/x.bsp", "caf\u{e9}/x.bsp").is_none(), "an old code page");
        assert!(crate::archive::written_name_problem(b"caf\x82/x.bsp", "caf\u{e9}/../x.bsp").is_some());
        // a tarball whose twin creatures.ini would land on the first (case): refused, cleared
        if Path::new("/usr/bin/tar").exists() {
            let pk = fresh("pk");
            std::fs::create_dir_all(pk.join("a")).unwrap();
            std::fs::write(pk.join("a/creatures.ini"), "[gb11.x]\n").unwrap();
            std::fs::write(pk.join("creatures.ini.bak"), "[gb11.x]\nhp = 1\n").unwrap();
            let tp = dir.join("twin.tar");
            // the same entry twice under two spellings of one path
            assert!(std::process::Command::new("/usr/bin/tar")
                .arg("-cf").arg(&tp).arg("-C").arg(&pk).arg("-s").arg(",^creatures.ini.bak,a/CREATURES.INI,").arg("a/creatures.ini").arg("creatures.ini.bak")
                .status().unwrap().success());
            let out = fresh("t");
            let r = crate::archive::extract(&tp, &out, 1 << 20);
            // (a case-insensitive disk keeps one file for two entries: refused; a case-sensitive
            // one keeps both, which the lower-case twin check refuses)
            assert!(r.is_err(), "{r:?}");
            assert!(!out.join("a/creatures.ini").exists());
        }
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

    // ---- NPC-editor ------------------------------------------------------------------------

    const NPC_MAP: &str = "{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"ls_npc\"\n\"name\" \"Joe\"\n\"model\" \"models/qa/joe.mdl\"\n\"greet_snd\" \"qa/hello.wav\"\n}\n{\n\"classname\" \"ls_interactable\"\n\"model\" \"models/w_battery.mdl\"\n}\n{\n\"classname\" \"cycler\"\n\"model\" \"models/qa/other.mdl\"\n}\n";

    #[test]
    fn marker_sprites_travel_like_npc_files() {
        // an Objective Marker's own sprite goes with the mode; the game's arrow stays as it is
        let g = serde_json::json!({"nodes": [
            {"type": "fx.marker", "params": {"sprite": "sprites/qa/flag.spr"}},
            {"type": "fx.marker", "params": {"sprite": "sprites/arrow1.spr"}},
            {"type": "fx.beam", "params": {"sprite": "sprites/qa/not_this.spr"}}
        ]});
        let found = npc_files_in_graph_json(&g);
        // (every node's sprite counts now, a beam's too: the asset pickers' files go with a mode)
        assert_eq!(found.iter().map(String::as_str).collect::<Vec<_>>(), vec!["sprites/arrow1.spr", "sprites/qa/flag.spr", "sprites/qa/not_this.spr"]);
        let (hl, gd) = game("");
        std::fs::create_dir_all(gd.join("sprites/qa")).unwrap();
        std::fs::write(gd.join("sprites/qa/flag.spr"), b"spr").unwrap();
        let pack = Pack::default().with_npc(&hl, "cstrike", &found, "gb7").unwrap();
        let rels: Vec<&str> = pack.npc.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(rels, vec!["sprites/gb7/qa/flag.spr"]);
        let mut gv = g.clone();
        pack.rewrite_graph(&mut gv);
        assert_eq!(gv["nodes"][0]["params"]["sprite"], "sprites/gb7/qa/flag.spr");
        assert_eq!(gv["nodes"][1]["params"]["sprite"], "sprites/arrow1.spr");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn npc_files_are_found_packed_under_the_owner_and_renamed() {
        // found: an NPC's and an interactable's files, never another class's
        let found = npc_files_in_entity_text(NPC_MAP);
        assert_eq!(found.iter().map(String::as_str).collect::<Vec<_>>(), vec!["models/qa/joe.mdl", "models/w_battery.mdl", "sound/qa/hello.wav"]);
        let g = serde_json::json!({"nodes": [
            {"type": "npc.spawn", "params": {"model": "models/qa/joe.mdl", "name": "Joe"}},
            {"type": "comp.create", "params": {"kind": "ls_interactable", "keys": "model=models/qa/lever.mdl;prompt=Pull"}},
            {"type": "prop.spawn", "params": {"model": "models/qa/not_npc.mdl"}}
        ]});
        // (any node's file travels with a mode: a prop's model too)
        assert_eq!(npc_files_in_graph_json(&g).into_iter().collect::<Vec<_>>(), vec!["models/qa/joe.mdl", "models/qa/lever.mdl", "models/qa/not_npc.mdl"]);
        // packed: the custom ones (not the game's own battery) under gb7's folders
        let (hl, gd) = game("");
        std::fs::create_dir_all(gd.join("models/qa")).unwrap();
        std::fs::create_dir_all(gd.join("sound/qa")).unwrap();
        std::fs::write(gd.join("models/qa/joe.mdl"), b"joe").unwrap();
        std::fs::write(gd.join("sound/qa/hello.wav"), b"hi").unwrap();
        assert!(any_custom(&hl, "cstrike", &found));
        let pack = Pack::default().with_npc(&hl, "cstrike", &found, "gb7").unwrap();
        let rels: Vec<&str> = pack.npc.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(rels, vec!["models/gb7/qa/joe.mdl", "sound/gb7/qa/hello.wav"]);
        assert!(pack.text.contains("; npc: models/gb7/qa/joe.mdl sound/gb7/qa/hello.wav\n"), "{}", pack.text);
        assert!(parse_pack(&pack.text).is_ok(), "a pack with only NPC files reads");
        assert!(pack.summary().iter().any(|l| l == "NPC files: joe.mdl, hello.wav"), "{:?}", pack.summary());
        let names: Vec<String> = pack.entries(1 << 20).unwrap().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec![PACK_FILE, "models/gb7/qa/joe.mdl", "sound/gb7/qa/hello.wav"]);
        // the packaged copy's entities and graphs follow; other classes and the game's files don't
        let ents = pack.rewrite_entities(NPC_MAP);
        assert!(ents.contains("\"model\" \"models/gb7/qa/joe.mdl\"") && ents.contains("\"greet_snd\" \"gb7/qa/hello.wav\""), "{ents}");
        assert!(ents.contains("\"models/w_battery.mdl\"") && ents.contains("\"models/qa/other.mdl\""));
        let mut gv = g.clone();
        Pack::default().with_npc(&hl, "cstrike", &npc_files_in_graph_json(&g), "gb7").unwrap().rewrite_graph(&mut gv);
        assert_eq!(gv["nodes"][0]["params"]["model"], "models/gb7/qa/joe.mdl");
        // (a file that isn't in the game stays as it was)
        assert_eq!(gv["nodes"][2]["params"]["model"], "models/qa/not_npc.mdl");
        let _ = std::fs::remove_dir_all(&hl);
    }

    #[test]
    fn npc_files_install_from_the_owners_folder_only_and_go_with_the_package() {
        let (hl, gd) = game("; mine\n[qa_mine]\nbase = zombie\n");
        let before = std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap();
        // an unpacked package: its map names the NPC's model
        let src = hl.join("pk");
        std::fs::create_dir_all(src.join("models/gb7/qa")).unwrap();
        std::fs::create_dir_all(src.join("maps")).unwrap();
        std::fs::write(src.join("models/gb7/qa/joe.mdl"), b"joe").unwrap();
        std::fs::write(src.join("models/gb7/qa/extra.mdl"), b"x").unwrap();
        let ents = NPC_MAP.replace("models/qa/joe.mdl", "models/gb7/qa/joe.mdl");
        let mut bsp = vec![0u8; 124];
        bsp[0] = 30;
        std::fs::write(src.join("maps/m.bsp"), bsp_with_entities(&bsp, &ents).unwrap()).unwrap();
        // not named by the package's maps: refused
        let stray = "; types:\n; npc: models/gb7/qa/extra.mdl\n";
        assert!(install_pack(&gd, "map-5", stray, Some(&src), "gb7").unwrap_err().contains("none of its NPCs use"));
        // another maker's folder: refused
        let theirs = "; types:\n; npc: models/gb8/qa/joe.mdl\n";
        assert!(install_pack(&gd, "map-5", theirs, Some(&src), "gb7").unwrap_err().contains("own folder"));
        // the game's own: refused
        assert!(parse_pack("; npc: models/../x.mdl\n").is_err());
        // its own, named by its map: copied, recorded, the player's sections untouched
        let ok = "; types:\n; npc: models/gb7/qa/joe.mdl\n";
        let done = install_pack(&gd, "map-5", ok, Some(&src), "gb7").unwrap();
        assert_eq!(done.wrote, vec!["models/gb7/qa/joe.mdl"]);
        assert!(gd.join("models/gb7/qa/joe.mdl").is_file());
        let (user, blocks) = split_blocks(&std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap());
        assert_eq!(user, before);
        assert_eq!(blocks[0].npc, vec!["models/gb7/qa/joe.mdl"]);
        // a second package needing it keeps it when the first goes
        install_pack(&gd, "map-6", ok, Some(&src), "gb7").unwrap();
        assert_eq!(uninstall_pack(&gd, "map-5").unwrap(), Vec::<String>::new());
        assert!(gd.join("models/gb7/qa/joe.mdl").is_file());
        assert_eq!(uninstall_pack(&gd, "map-6").unwrap(), vec!["models/gb7/qa/joe.mdl"]);
        assert!(!gd.join("models/gb7/qa/joe.mdl").exists());
        assert_eq!(std::fs::read_to_string(gd.join(creatures::FILE_REL)).unwrap(), before);
        let _ = std::fs::remove_dir_all(&hl);
    }
}
