//! Bot personality editor: who the bots are, edited with real controls instead of in ini files.
//!
//! The bots' people come from three files (docs/bot-people.md):
//! - `configs/gamemodes/people.ini` (gm_people): the kinds of people ("archetypes": a name, how
//!   common, the old chat letter it acts as, a play style, a few fixed traits), how common each
//!   trait's values are when rolled ("weights") and what never goes together ("conflicts").
//!   Every bot's person is worked out from its name (so a regular is the same person every
//!   time): an archetype is picked by weight, the rest rolled. gm_people_reload re-reads it.
//! - `configs/gamemodes/regular_names.ini` (gm_pubserver): the names the bot regulars get.
//! - `data/gamemodes/regulars.ini` (gm_pubserver): which bot profile got which name, for good.
//!
//! The page edits people.ini as a model (archetypes keep the comment heading they're under);
//! the file's header and any section we don't know are kept as they are. regular_names.ini is
//! edited line by line (rename in place, remove, add at the end), so its comments stay.
//! Every save keeps the old file in the app's folder (people/backups) first.
//!
//! "Who is this name": `who` repeats gm_people's working out (its random sequence, archetype
//! roll, trait rolls and conflicts) for a plain lobby (Normal mood, no home region, no
//! gm_humanbots aim style); the live game may lean a bot a little from there, and a name the
//! game has already met keeps the archetype it first got (gm_people's memory).
//!
//! Setup treats people.ini and regular_names.ini as the player's once changed (setup::core
//! EDITABLE): an update leaves them and puts its own version beside them as <name>.new.
//!
//! App window only (merged into tools::local_routes). Errors are `{error}` with status 200;
//! POSTs need the panel header.

use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const NO_GAME: &str = "Set up the game on this computer first.";
const NOT_SET_UP: &str = "Loner-Strike isn't set up in this game yet, so there are no bot people to edit. Set it up on the Setup page first.";
const INTERNAL: &str = "Loner-Strike hit an internal error with the bot people. Try again, and copy the diagnostics from Settings if it keeps happening.";
const KEEP: usize = 10;
const MAX_FILE: u64 = 4 << 20;

/// gm_people's traits and their values, in its order (TRAIT_NAME / TRAIT_VALUES).
pub const TRAITS: [(&str, &[&str]); 10] = [
    ("warmth", &["cold", "neutral", "warm"]),
    ("ego", &["humble", "normal", "showoff"]),
    ("temper", &["chill", "normal", "tilty"]),
    ("talk", &["silent", "quiet", "normal", "chatty", "spammer"]),
    ("humor", &["none", "dry", "goofy", "troll"]),
    ("age", &["kid", "teen", "adult", "old"]),
    ("skill", &["newbie", "average", "good", "pro"]),
    ("role", &["regular", "drifter", "admin", "clan", "afk"]),
    ("region", &["us", "uk", "de", "se", "pl", "ru", "br", "fr", "nl", "es", "au", "fi", "dk"]),
    ("style", &["rusher", "roamer", "lurker", "careful", "camper", "awper", "knifer", "nader", "ninja", "objective", "weirdgun"]),
];
const T_STYLE: usize = 9;
pub const HOBBIES: [&str; 16] = ["wc3", "wow", "halo", "school", "work", "cars", "music", "lan", "cz", "source", "awp", "sleep", "food", "girlfriend", "footy", "metal"];
/// gm_botchat's personality letters (PERSONAS) and what each one is (PERSONA_DESC).
pub const LETTERS: &str = "ftsjqncokxhzpbiduwrmABCDEFGHLMNOPSTYIJKQRUVWXZ0123aeglvy456789";
pub const LETTER_DESC: [&str; 62] = [
    "friendly", "tryhard", "salty", "joker", "quiet", "new player", "chill", "old-timer", "kid", "English isn't their first language", "hype", "zoomer", "streamer wannabe", "backseat gamer",
    "the team's caller", "half asleep", "blames themselves", "wholesome", "military roleplay", "meme lord", "anime fan", "Brit", "conspiracy theorist", "dad who games after the kids are in bed",
    "esports wannabe", "foodie", "grammar nerd", "sees hackers everywhere", "lurker", "music fan", "stats nerd", "overconfident", "pacifist", "movement nerd", "map tourist", "yapper", "works in IT",
    "gym regular", "wants the admin", "trivia nerd", "patient coach", "pirate", "villain roleplay", "philosopher", "extreme 2000s gamer", "clutch hero", "jokes about being a bot", "cowboy", "detective",
    "leetspeak", "awper", "rusher", "nade spammer", "lagger", "map voter", "weird guns", "camper", "knifer", "ninja", "clan recruiter", "objective player", "spray artist",
];
/// gm_people's limits (MAX_ARCH, MAX_CONFLICTS, its 256-character line and field sizes).
const MAX_ARCH: usize = 200;
const MAX_CONFLICTS: usize = 64;
const MAX_LINE: usize = 250;
const MAX_FIXED: usize = 159;

fn trait_index(k: &str) -> Option<usize> {
    TRAITS.iter().position(|(n, _)| n.eq_ignore_ascii_case(k))
}

/// A value's index in a trait (or hobby, `t` None) by name or number, like value_index.
fn value_index(t: Option<usize>, v: &str) -> Option<usize> {
    let list: &[&str] = match t {
        Some(t) => TRAITS[t].1,
        None => &HOBBIES,
    };
    if v.starts_with(|c: char| c.is_ascii_digit()) {
        return v.parse::<usize>().ok().filter(|&i| i < list.len());
    }
    list.iter().position(|x| x.eq_ignore_ascii_case(v))
}

// ---- the model --------------------------------------------------------------------------

/// `key=value`: a trait (or `hobby`) and one of its values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub key: String,
    pub value: String,
}

impl Pair {
    fn parse(tok: &str) -> Option<Pair> {
        let (k, v) = tok.split_once('=')?;
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        if k.is_empty() {
            return None;
        }
        // numbers become names (gm_people reads both)
        let t = if k == "hobby" { None } else { Some(trait_index(&k)?) };
        let value = match value_index(t, v) {
            Some(i) if v.starts_with(|c: char| c.is_ascii_digit()) => t.map(|t| TRAITS[t].1[i]).unwrap_or(HOBBIES[i]).to_string(),
            _ => v.to_string(),
        };
        Some(Pair { key: k, value })
    }
    fn index(&self) -> Option<(Option<usize>, usize)> {
        let t = if self.key == "hobby" { None } else { Some(trait_index(&self.key)?) };
        Some((t, value_index(t, &self.value)?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arch {
    pub name: String,
    pub weight: u32,
    pub letter: String,
    /// a play style, or "-" (rolled)
    pub style: String,
    pub fixed: Vec<Pair>,
    /// the comment heading it's under ("; ---- jokers and trolls"), as written
    #[serde(default)]
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weight {
    #[serde(rename = "trait")]
    pub key: String,
    pub weights: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub a: Pair,
    pub b: Pair,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct People {
    pub archetypes: Vec<Arch>,
    pub weights: Vec<Weight>,
    pub conflicts: Vec<Conflict>,
}

/// people.ini as read: the model, plus what we keep as it was.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub people: People,
    /// lines before the first section
    head: Vec<String>,
    /// the sections in file order: (name, comment lines in it we keep, blank lines after it,
    /// the lines of a section we don't know)
    sections: Vec<Section>,
    crlf: bool,
}

#[derive(Debug, Clone, Default)]
struct Section {
    name: String,
    notes: Vec<String>,
    gap: usize,
    raw: Vec<String>,
}

const KNOWN: [&str; 3] = ["weights", "conflicts", "archetypes"];

fn arch_line(a: &Arch) -> String {
    let fixed: Vec<String> = a.fixed.iter().map(|p| format!("{}={}", p.key, p.value)).collect();
    format!("{}|{}|{}|{}|{}", a.name, a.weight, a.letter, if a.style.is_empty() { "-" } else { &a.style }, fixed.join(" "))
}

fn field(s: &str) -> String {
    s.trim().to_string()
}

pub fn parse(text: &str) -> Doc {
    let mut d = Doc { crlf: text.contains("\r\n"), ..Default::default() };
    let mut cur: Option<Section> = None;
    let mut group = String::new();
    let mut blanks = 0;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if let Some(mut s) = cur.take() {
                s.gap = blanks;
                d.sections.push(s);
            }
            blanks = 0;
            let name = t[1..].split(']').next().unwrap_or("").to_string();
            cur = Some(Section { name, ..Default::default() });
            group.clear();
            continue;
        }
        let Some(s) = cur.as_mut() else {
            d.head.push(line.to_string());
            continue;
        };
        let known = KNOWN.contains(&s.name.as_str());
        if !known {
            s.raw.push(line.to_string());
            continue;
        }
        if t.is_empty() {
            blanks += 1;
            // (a blank line ends a heading's group: a kind after one has no heading)
            group.clear();
            continue;
        }
        let pending = std::mem::take(&mut blanks);
        if t.starts_with(';') {
            if s.name == "archetypes" {
                group = line.to_string();
            } else {
                s.notes.push(line.to_string());
            }
            continue;
        }
        // a blank line inside a section that isn't before a heading: nothing to keep
        let _ = pending;
        match s.name.as_str() {
            "weights" => {
                let (k, rest) = t.split_once('|').unwrap_or((t, ""));
                d.people.weights.push(Weight { key: field(k).to_ascii_lowercase(), weights: rest.split_whitespace().map(|w| w.parse::<i64>().unwrap_or(0).max(0) as u32).collect() });
            }
            "conflicts" => {
                let mut w = t.split_whitespace();
                if let (Some(a), Some(b)) = (w.next().and_then(Pair::parse), w.next().and_then(Pair::parse)) {
                    d.people.conflicts.push(Conflict { a, b });
                }
            }
            _ => {
                let f: Vec<&str> = t.splitn(5, '|').collect();
                let get = |i: usize| f.get(i).map(|s| field(s)).unwrap_or_default();
                let style = get(3);
                d.people.archetypes.push(Arch {
                    name: get(0),
                    weight: get(1).parse::<i64>().unwrap_or(0).max(0) as u32,
                    letter: get(2).chars().next().map(String::from).unwrap_or_default(),
                    style: if style.is_empty() { "-".into() } else { style },
                    fixed: get(4).split_whitespace().filter_map(Pair::parse).collect(),
                    group: group.clone(),
                });
            }
        }
    }
    if let Some(mut s) = cur.take() {
        s.gap = blanks;
        d.sections.push(s);
    }
    d
}

impl Doc {
    /// The file for `people`, keeping this file's header, section order and other sections.
    pub fn write(&self, people: &People) -> String {
        let mut out: Vec<String> = self.head.clone();
        let mut sections = self.sections.clone();
        for k in KNOWN {
            if !sections.iter().any(|s| s.name == k) {
                if let Some(last) = sections.last_mut() {
                    last.gap = last.gap.max(1);
                }
                sections.push(Section { name: k.into(), ..Default::default() });
            }
        }
        for s in &sections {
            out.push(format!("[{}]", s.name));
            out.extend(s.notes.iter().cloned());
            match s.name.as_str() {
                "weights" => out.extend(people.weights.iter().map(|w| format!("{}|{}", w.key, w.weights.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" ")))),
                "conflicts" => out.extend(people.conflicts.iter().map(|c| format!("{}={} {}={}", c.a.key, c.a.value, c.b.key, c.b.value))),
                "archetypes" => {
                    let mut group: Option<&str> = None;
                    for (i, a) in people.archetypes.iter().enumerate() {
                        if group != Some(a.group.as_str()) {
                            if !a.group.is_empty() {
                                if i > 0 {
                                    out.push(String::new());
                                }
                                out.push(a.group.clone());
                            } else if i > 0 {
                                out.push(String::new());
                            }
                            group = Some(a.group.as_str());
                        }
                        out.push(arch_line(a));
                    }
                }
                _ => out.extend(s.raw.iter().cloned()),
            }
            out.extend(std::iter::repeat(String::new()).take(s.gap));
        }
        let nl = if self.crlf { "\r\n" } else { "\n" };
        out.join(nl) + nl
    }
}

// ---- checks -----------------------------------------------------------------------------

/// Something wrong with the people: `error` stops a save, otherwise it's a warning.
#[derive(Debug, Serialize, PartialEq)]
pub struct Problem {
    /// "arch:<index>", "weight:<trait>", "conflict:<index>", "names", or "" for the whole file
    #[serde(rename = "where")]
    pub at: String,
    pub text: String,
    pub error: bool,
}

fn problem(at: impl Into<String>, text: impl Into<String>, error: bool) -> Problem {
    Problem { at: at.into(), text: text.into(), error }
}

fn pair_problem(p: &Pair) -> Option<String> {
    if p.key != "hobby" && trait_index(&p.key).is_none() {
        return Some(format!("\"{}\" isn't a trait.", p.key));
    }
    if p.index().is_none() {
        return Some(format!("\"{}\" isn't a value for {}.", p.value, p.key));
    }
    None
}

pub fn check(p: &People) -> Vec<Problem> {
    let mut out = Vec::new();
    if p.archetypes.is_empty() {
        out.push(problem("", "There are no kinds of people: every bot would be a plain player.", true));
    }
    if p.archetypes.len() > MAX_ARCH {
        out.push(problem("", format!("The game reads {MAX_ARCH} kinds of people at most; there are {}.", p.archetypes.len()), true));
    }
    if !p.archetypes.is_empty() && p.archetypes.iter().all(|a| a.weight == 0) {
        out.push(problem("", "Every kind of person has \"how common\" at 0, so none would ever be picked.", true));
    }
    let mut seen = std::collections::HashSet::new();
    for (i, a) in p.archetypes.iter().enumerate() {
        let at = format!("arch:{i}");
        let label = if a.name.is_empty() { format!("Kind {}", i + 1) } else { a.name.clone() };
        if a.name.is_empty() {
            out.push(problem(&at, "Give it a name.", true));
        } else if a.name.len() > 31 || !a.name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            out.push(problem(&at, format!("{label}: a name is up to 31 letters, digits and _ (no spaces)."), true));
        } else if !seen.insert(a.name.to_ascii_lowercase()) {
            out.push(problem(&at, format!("{label}: another kind has this name. Bots remember their kind by name, so each needs its own."), true));
        }
        if a.weight > 1000 {
            out.push(problem(&at, format!("{label}: \"how common\" goes up to 1000."), true));
        } else if a.weight == 0 {
            out.push(problem(&at, format!("{label}: \"how common\" is 0, so it's never picked for a new bot."), false));
        }
        if a.letter.chars().count() != 1 || !LETTERS.contains(a.letter.as_str()) {
            out.push(problem(&at, format!("{label}: pick how it chats (its chat personality)."), true));
        }
        if a.style != "-" && value_index(Some(T_STYLE), &a.style).is_none() {
            out.push(problem(&at, format!("{label}: \"{}\" isn't a play style.", a.style), true));
        }
        let mut keys = std::collections::HashSet::new();
        for f in &a.fixed {
            if let Some(e) = pair_problem(f) {
                out.push(problem(&at, format!("{label}: {e}"), true));
            } else if f.key == "style" {
                out.push(problem(&at, format!("{label}: set the play style in its own field, not as a fixed trait."), true));
            } else if !keys.insert(f.key.clone()) {
                out.push(problem(&at, format!("{label}: {} is fixed twice.", f.key), true));
            }
        }
        let line = arch_line(a);
        let fixed_len = line.splitn(5, '|').nth(4).map(str::len).unwrap_or(0);
        if line.len() > MAX_LINE || fixed_len > MAX_FIXED {
            out.push(problem(&at, format!("{label}: too many fixed traits for the game to read. Take some out."), true));
        }
        // fixed on both sides of a never-together pair: the game can't fix that
        for c in &p.conflicts {
            let has = |q: &Pair| a.fixed.iter().any(|f| f.key == q.key && f.index() == q.index() && f.index().is_some()) || (q.key == "style" && a.style == q.value);
            if has(&c.a) && has(&c.b) {
                out.push(problem(&at, format!("{label}: fixes {}={} and {}={}, which never go together.", c.a.key, c.a.value, c.b.key, c.b.value), false));
            }
        }
    }
    for (t, (name, values)) in TRAITS.iter().enumerate() {
        let at = format!("weight:{name}");
        match p.weights.iter().find(|w| w.key == *name) {
            None => out.push(problem(&at, format!("{name}: no weights, so every rolled {name} is \"{}\".", values[0]), false)),
            Some(w) => {
                if w.weights.len() != values.len() {
                    out.push(problem(&at, format!("{name}: needs {} weights, one per value; it has {}.", values.len(), w.weights.len()), true));
                }
                if w.weights.iter().all(|&n| n == 0) {
                    out.push(problem(&at, format!("{name}: every weight is 0."), true));
                }
                if w.weights.iter().any(|&n| n > 1000) {
                    out.push(problem(&at, format!("{name}: a weight goes up to 1000."), true));
                }
            }
        }
        let _ = t;
    }
    for w in &p.weights {
        if trait_index(&w.key).is_none() {
            out.push(problem(format!("weight:{}", w.key), format!("\"{}\" isn't a trait.", w.key), true));
        }
    }
    if p.conflicts.len() > MAX_CONFLICTS {
        out.push(problem("", format!("The game reads {MAX_CONFLICTS} never-together pairs at most; there are {}.", p.conflicts.len()), true));
    }
    for (i, c) in p.conflicts.iter().enumerate() {
        let at = format!("conflict:{i}");
        for e in [pair_problem(&c.a), pair_problem(&c.b)].into_iter().flatten() {
            out.push(problem(&at, e, true));
        }
        if c.a.key == c.b.key && c.a.key != "hobby" {
            out.push(problem(&at, format!("{0} can only have one value, so {0}={1} and {0}={2} are never together anyway.", c.a.key, c.a.value, c.b.value), false));
        }
    }
    out
}

// ---- regular names ----------------------------------------------------------------------

/// A name gm_pubserver hands out (load_names): no quote, not a comment, under 32 characters,
/// no year in it.
pub fn name_problem(n: &str) -> Option<&'static str> {
    if n.trim().is_empty() {
        return Some("is empty");
    }
    if n.trim() != n {
        return Some("starts or ends with a space");
    }
    if n.contains('"') {
        return Some("has a quote in it (the game couldn't kick that bot)");
    }
    if n.starts_with(';') || n.starts_with('#') {
        return Some("starts with ; or # (read as a note)");
    }
    if n.len() >= 32 {
        return Some("is 32 characters or more");
    }
    if n.chars().any(|c| c.is_control()) {
        return Some("has a control character in it");
    }
    if has_year(n) {
        return Some("has a year in it (1950 to 2039), which the game skips");
    }
    None
}

fn has_year(n: &str) -> bool {
    let b = n.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_digit()) {
            let mut j = i;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j - i == 4 {
                let v: u32 = n[i..j].parse().unwrap_or(0);
                if (1950..=2039).contains(&v) {
                    return true;
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    false
}

/// The names in regular_names.ini, in order.
pub fn names(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with('#')).map(String::from).collect()
}

/// Changes to the names: renamed in place, removed, added at the end.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct NameEdits {
    pub renamed: Vec<(String, String)>,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

impl NameEdits {
    fn is_empty(&self) -> bool {
        self.renamed.is_empty() && self.removed.is_empty() && self.added.is_empty()
    }
}

/// regular_names.ini with the edits made; its comments and order stay.
pub fn edit_names(text: &str, e: &NameEdits) -> Result<String, String> {
    for n in e.renamed.iter().map(|(_, n)| n).chain(e.added.iter()) {
        if let Some(why) = name_problem(n) {
            return Err(format!("The name {n:?} {why}."));
        }
    }
    let crlf = text.contains("\r\n");
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut existing: std::collections::HashSet<String> = names(text).into_iter().collect();
    for (old, new) in &e.renamed {
        let Some(l) = lines.iter_mut().find(|l| l.trim() == old) else { return Err(format!("{old} isn't in the list any more.")) };
        if old != new && existing.contains(new) {
            return Err(format!("{new} is already in the list."));
        }
        *l = new.clone();
        existing.remove(old);
        existing.insert(new.clone());
    }
    for r in &e.removed {
        if let Some(i) = lines.iter().position(|l| l.trim() == r) {
            lines.remove(i);
            existing.remove(r);
        }
    }
    for a in &e.added {
        if existing.insert(a.clone()) {
            lines.push(a.clone());
        }
    }
    let nl = if crlf { "\r\n" } else { "\n" };
    Ok(lines.join(nl) + nl)
}

/// regulars.ini: `profile = name`, the names given out for good.
pub fn regulars(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(';'))
        .filter_map(|l| l.split_once('=').map(|(p, n)| (p.trim().to_string(), n.trim().to_string())))
        .filter(|(p, n)| !p.is_empty() && !n.is_empty())
        .collect()
}

/// regulars.ini without the given profiles (they get a fresh name next time).
pub fn forget_regulars(text: &str, profiles: &[String]) -> String {
    let crlf = text.contains("\r\n");
    let keep: Vec<&str> = text.lines().filter(|l| !l.split_once('=').is_some_and(|(p, _)| profiles.iter().any(|f| f == p.trim()))).collect();
    if keep.is_empty() {
        return String::new();
    }
    let nl = if crlf { "\r\n" } else { "\n" };
    keep.join(nl) + nl
}

// ---- who a name is (gm_people's working out) ---------------------------------------------

/// A name without its clan tag (ls_bare_name).
pub fn bare_name(name: &str) -> String {
    const OPENS: &[u8] = b"[({<|";
    const CLOSES: &[u8] = b"])}>|";
    let s = name.as_bytes();
    let (mut a, mut b) = (0usize, s.len());
    for _ in 0..2 {
        while a < b && s[a] == b' ' {
            a += 1;
        }
        if let Some(o) = (a < b).then(|| OPENS.iter().position(|&c| c == s[a])).flatten() {
            if let Some(k) = (a + 1..b).find(|&k| s[k] == CLOSES[o]) {
                a = k + 1;
            }
        } else if b - a > 4 && s[a] == b'-' && s[a + 1] == b'=' {
            if let Some(k) = (a + 2..b.saturating_sub(1)).find(|&k| s[k] == b'=' && s[k + 1] == b'-') {
                a = k + 2;
            }
        }
        while b > a && s[b - 1] == b' ' {
            b -= 1;
        }
        if let Some(c) = (b > a).then(|| CLOSES.iter().position(|&c| c == s[b - 1])).flatten() {
            if let Some(k) = (a..b - 1).rev().find(|&k| s[k] == OPENS[c]) {
                b = k;
            }
        }
    }
    if b <= a {
        a = 0;
        b = s.len();
    }
    let m = (b - a).min(31);
    String::from_utf8_lossy(&s[a..a + m]).trim().to_string()
}

fn name_seed(name: &str) -> i32 {
    let mut h: i32 = 2166136261u32 as i32;
    // (Pawn cells from a C string: bytes over 127 come in negative)
    for &c in name.as_bytes() {
        h = (h ^ (c as i8 as i32)).wrapping_mul(16777619);
    }
    h & 0x7FFF_FFFF
}

fn next_rand(rnd: &mut i32) -> i32 {
    *rnd = rnd.wrapping_mul(1103515245).wrapping_add(12345) & 0x7FFF_FFFF;
    *rnd >> 8
}

/// The model in gm_people's numbers: archetypes' fixed traits (-1 rolled) and the weights.
struct Rules {
    weight: Vec<i32>,
    total: i32,
    fixed: Vec<[i32; 10]>,
    hobby: Vec<i32>,
    weights: Vec<Vec<i32>>,
    conflicts: Vec<(usize, usize, usize, usize)>,
}

const HOBBY_T: usize = 10;

impl Rules {
    fn new(p: &People) -> Rules {
        let take = p.archetypes.len().min(MAX_ARCH);
        let mut r = Rules { weight: Vec::new(), total: 0, fixed: Vec::new(), hobby: Vec::new(), weights: vec![Vec::new(); 10], conflicts: Vec::new() };
        for a in &p.archetypes[..take] {
            let mut fx = [-1i32; 10];
            let mut hobby = -1;
            if a.style != "-" {
                fx[T_STYLE] = value_index(Some(T_STYLE), &a.style).map(|v| v as i32).unwrap_or(-1);
            }
            for f in &a.fixed {
                match f.index() {
                    Some((None, v)) => hobby = v as i32,
                    Some((Some(t), v)) => fx[t] = v as i32,
                    None => {}
                }
            }
            r.weight.push(a.weight.min(i32::MAX as u32) as i32);
            r.total += a.weight as i32;
            r.fixed.push(fx);
            r.hobby.push(hobby);
        }
        for w in &p.weights {
            if let Some(t) = trait_index(&w.key) {
                r.weights[t] = w.weights.iter().take(16).map(|&n| n as i32).collect();
            }
        }
        for c in p.conflicts.iter().take(MAX_CONFLICTS) {
            if let (Some((ta, va)), Some((tb, vb))) = (c.a.index(), c.b.index()) {
                r.conflicts.push((ta.unwrap_or(HOBBY_T), va, tb.unwrap_or(HOBBY_T), vb));
            }
        }
        r
    }

    fn roll_arch(&self, rnd: &mut i32) -> usize {
        let mut r = next_rand(rnd) % self.total.max(1);
        let n = self.weight.len();
        let mut a = 0;
        while a + 1 < n {
            if r < self.weight[a] {
                break;
            }
            r -= self.weight[a];
            a += 1;
        }
        a
    }

    fn roll(&self, t: usize, rnd: &mut i32) -> usize {
        let w = &self.weights[t];
        let total: i32 = w.iter().sum();
        if w.is_empty() || total <= 0 {
            return 0;
        }
        let mut r = next_rand(rnd) % total;
        for (i, &x) in w.iter().enumerate() {
            if r < x {
                return i;
            }
            r -= x;
        }
        w.len() - 1
    }
}

/// Who a name is in a plain lobby: (archetype index, traits, hobbies).
pub fn who(p: &People, name: &str) -> Option<(usize, [usize; 10], [usize; 2])> {
    let r = Rules::new(p);
    if r.weight.is_empty() {
        return None;
    }
    let mut bare = bare_name(name);
    // (a second bot with the same name, "(1)Dave", is someone else: "Dave#2")
    let nb = name.as_bytes();
    if nb.len() > 1 && nb[0] == b'(' && nb[1].is_ascii_digit() {
        let n: u32 = name[1..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
        bare = format!("{bare}#{}", n + 1);
    }
    let seed = name_seed(&bare);
    let mut rnd = seed;
    let a = r.roll_arch(&mut rnd);
    let mut rnd = seed ^ 0x2545F491;
    let mut traits = [0usize; 10];
    for t in 0..10 {
        traits[t] = if r.fixed[a][t] >= 0 { r.fixed[a][t] as usize } else { r.roll(t, &mut rnd) };
    }
    let hc = HOBBIES.len() as i32;
    let mut hob = [0i32; 2];
    hob[0] = if r.hobby[a] >= 0 { r.hobby[a] } else { next_rand(&mut rnd) % hc };
    hob[1] = next_rand(&mut rnd) % hc;
    if hob[1] == hob[0] {
        hob[1] = (hob[1] + 1 + next_rand(&mut rnd) % (hc - 1)) % hc;
    }
    // conflicts: roll again whichever side the archetype didn't fix
    for tries in 0..12 {
        let has = |traits: &[usize; 10], hob: &[i32; 2], t: usize, v: usize| if t == HOBBY_T { hob[0] == v as i32 || hob[1] == v as i32 } else { traits[t] == v };
        let Some(&(ta, _, tb, _)) = r.conflicts.iter().find(|&&(ta, va, tb, vb)| has(&traits, &hob, ta, va) && has(&traits, &hob, tb, vb)) else { break };
        let mut t = tb;
        if t == HOBBY_T || r.fixed[a][t] >= 0 {
            t = ta;
            if t == HOBBY_T || r.fixed[a][t] >= 0 {
                let h = if (tries & 1) == 1 && r.hobby[a] < 0 { 0 } else { 1 };
                hob[h] = (hob[h] + 1) % hc;
                if hob[h] == hob[1 - h] {
                    hob[h] = (hob[h] + 1) % hc;
                }
                continue;
            }
        }
        traits[t] = r.roll(t, &mut rnd);
    }
    // a clan tag: in a clan, most of the time; no tag: not a clan player
    if r.fixed[a][7] < 0 {
        // (the bare name as the plugin has it here, "#2" and all: "(1)Dave" counts as tagged there too)
        let tagged = name != bare && {
            let pos = name.find(bare.as_str());
            name.char_indices().any(|(i, c)| c.is_ascii_alphabetic() && !pos.is_some_and(|p| i >= p && i < p + bare.len()))
        };
        if tagged && (seed / 17) % 100 < 75 {
            traits[7] = 3;
        } else if !tagged && traits[7] == 3 {
            traits[7] = 0;
        }
    }
    Some((a, traits, [hob[0] as usize, hob[1] as usize]))
}

fn who_json(p: &People, name: &str) -> Value {
    match who(p, name) {
        None => json!({"name": name}),
        Some((a, t, h)) => {
            let mut traits = serde_json::Map::new();
            for (i, (k, vals)) in TRAITS.iter().enumerate() {
                traits.insert(k.to_string(), json!(vals[t[i]]));
            }
            json!({"name": name, "arch": p.archetypes[a].name, "archIndex": a, "traits": traits, "hobbies": [HOBBIES[h[0]], HOBBIES[h[1]]]})
        }
    }
}

// ---- files ------------------------------------------------------------------------------

/// Where the game keeps the files.
#[derive(Clone)]
pub struct Files {
    pub cfg: PathBuf,
    pub data: PathBuf,
    pub app: PathBuf,
    pub label: String,
}

impl Files {
    fn people(&self) -> PathBuf {
        self.cfg.join("people.ini")
    }
    fn names(&self) -> PathBuf {
        self.cfg.join("regular_names.ini")
    }
    fn regulars(&self) -> PathBuf {
        self.data.join("regulars.ini")
    }
}

fn read_text(p: &Path) -> Result<Option<String>, String> {
    match std::fs::metadata(p) {
        Err(_) => Ok(None),
        Ok(m) if m.len() > MAX_FILE => Err(format!("{} is too big to be a settings file.", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
        Ok(_) => crate::sys::read_latin1(p).map(Some).map_err(|e| format!("Couldn't read {}: {e}", p.display())),
    }
}

fn backup_dir(app: &Path) -> PathBuf {
    app.join("people").join("backups")
}

fn backups(app: &Path, file: &str) -> Vec<PathBuf> {
    let prefix = format!("{file}-");
    let mut v: Vec<PathBuf> = std::fs::read_dir(backup_dir(app))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)))
        .collect();
    v.sort();
    v.reverse();
    v
}

/// Writes a file whole, the old one kept in the app's folder first (when `backup`).
fn put(app: &Path, path: &Path, text: &str, backup: bool) -> Result<(), String> {
    let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::safety::before("Bot people", &if backup { format!("Saved {file}") } else { format!("Put back the previous {file}") }, path);
    if backup && path.is_file() {
        let b = backup_dir(app);
        std::fs::create_dir_all(&b).map_err(|e| format!("Couldn't keep a copy of {file}: {e}"))?;
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        std::fs::copy(path, b.join(format!("{file}-{ms:015}"))).map_err(|e| format!("Couldn't keep a copy of {file}: {e}"))?;
        for old in backups(app, &file).into_iter().skip(KEEP) {
            let _ = std::fs::remove_file(old);
        }
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save {file}: {e}"))?;
    }
    crate::sys::make_writable(path);
    crate::sys::write_latin1_atomic(path, text).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Loner-Strike isn't allowed to change {file}."),
        _ => format!("Couldn't save {file}: {e}"),
    })
}

fn catalog() -> Value {
    json!({
        "traits": TRAITS.iter().map(|(k, v)| json!({"key": k, "values": v})).collect::<Vec<_>>(),
        "hobbies": HOBBIES,
        "letters": LETTERS.chars().zip(LETTER_DESC.iter()).map(|(l, d)| json!({"letter": l.to_string(), "desc": d})).collect::<Vec<_>>(),
    })
}

pub fn status(f: &Files) -> Result<Value, String> {
    let Some(text) = read_text(&f.people())? else { return Err(NOT_SET_UP.into()) };
    let doc = parse(&text);
    let names_text = read_text(&f.names())?.unwrap_or_default();
    let regs = read_text(&f.regulars())?.unwrap_or_default();
    Ok(json!({
        "game": f.label,
        "people": doc.people,
        "problems": check(&doc.people),
        "names": names(&names_text),
        "regulars": regulars(&regs).into_iter().map(|(p, n)| json!({"profile": p, "name": n})).collect::<Vec<_>>(),
        "newBeside": f.cfg.join("people.ini.new").is_file(),
        "canRestore": !backups(&f.app, "people.ini").is_empty(),
        "catalog": catalog(),
    }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SaveReq {
    pub people: Option<People>,
    pub names: NameEdits,
    /// bot profiles whose regular name is forgotten (a fresh one next time)
    pub forget: Vec<String>,
}

/// Saves what changed. Returns which files were written.
pub fn save(f: &Files, req: &SaveReq) -> Result<Vec<&'static str>, String> {
    let mut wrote = Vec::new();
    // check everything before writing anything
    let people_text = match &req.people {
        Some(p) => {
            let problems: Vec<String> = check(p).into_iter().filter(|x| x.error).map(|x| x.text).collect();
            if let Some(first) = problems.first() {
                return Err(if problems.len() == 1 { format!("Not saved: {first}") } else { format!("Not saved: {first} (and {} more to fix)", problems.len() - 1) });
            }
            let Some(old) = read_text(&f.people())? else { return Err(NOT_SET_UP.into()) };
            let new = parse(&old).write(p);
            (new != old).then_some(new)
        }
        None => None,
    };
    let names_text = if req.names.is_empty() {
        None
    } else {
        let old = read_text(&f.names())?.unwrap_or_default();
        Some(edit_names(&old, &req.names)?)
    };
    if let Some(t) = people_text {
        put(&f.app, &f.people(), &t, true)?;
        wrote.push("people");
    }
    if let Some(t) = names_text {
        put(&f.app, &f.names(), &t, true)?;
        wrote.push("names");
    }
    if !req.forget.is_empty() {
        let old = read_text(&f.regulars())?.unwrap_or_default();
        let new = forget_regulars(&old, &req.forget);
        if new != old {
            put(&f.app, &f.regulars(), &new, true)?;
            wrote.push("regulars");
        }
    }
    Ok(wrote)
}

pub fn restore(f: &Files) -> Result<(), String> {
    let Some(newest) = backups(&f.app, "people.ini").into_iter().next() else { return Err("There's no earlier people.ini kept.".into()) };
    let text = crate::sys::read_latin1(&newest).map_err(|e| format!("Couldn't read the kept copy: {e}"))?;
    put(&f.app, &f.people(), &text, false)?;
    let _ = std::fs::remove_file(&newest);
    Ok(())
}

// ---- routes ----------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into())))
}

fn target(st: &Shared) -> Result<(std::sync::Arc<crate::game::Game>, Files), String> {
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.remote.is_some() {
        return Err("This server is on another computer: edit its bots' people there.".into());
    }
    let label = if g.dedicated { format!("the server on port {}", g.port) } else if g.name == "czero" { "Condition Zero".into() } else { "Counter-Strike 1.6".into() };
    let f = Files { cfg: g.dir.join("addons/amxmodx/configs/gamemodes"), data: g.data.clone(), app: g.app.clone(), label };
    Ok((g, f))
}

/// Asks the running game to read people.ini again; false when it isn't running.
fn reload(g: &crate::game::Game) -> bool {
    g.rcon.send("gm_people_reload").is_ok()
}

async fn api_status(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((_, f)) => blocking(move || status(&f)).await,
        Err(e) => answer(Err(e)),
    }
}

#[derive(Deserialize)]
struct CheckReq {
    people: People,
    #[serde(default)]
    names: Vec<String>,
}

/// The problems with a model, and who the given names are with it.
async fn api_check(h: HeaderMap, Json(r): Json<CheckReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let who: Vec<Value> = r.names.iter().take(400).map(|n| who_json(&r.people, n)).collect();
        Ok(json!({"problems": check(&r.people), "who": who}))
    })
    .await
}

async fn api_save(State(st): State<Shared>, h: HeaderMap, Json(r): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (g, f) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let wrote = save(&f, &r)?;
        if wrote.is_empty() {
            return Ok(json!({"ok": true, "message": "Nothing changed.", "status": status(&f)?}));
        }
        let live = wrote.contains(&"people") && reload(&g);
        let mut msg = String::from("Saved.");
        if wrote.contains(&"people") {
            msg.push_str(if live { " The bots in the game are who you made them now (a regular the game already knows keeps its kind)." } else { " Used from the next game." });
        }
        if wrote.contains(&"names") || wrote.contains(&"regulars") {
            msg.push_str(" Name changes show from the next map.");
        }
        Ok(json!({"ok": true, "message": msg, "live": live, "status": status(&f)?}))
    })
    .await
}

async fn api_restore(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (g, f) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        restore(&f)?;
        let live = reload(&g);
        Ok(json!({"ok": true, "message": if live { "Your previous people.ini is back, and the game is using it." } else { "Your previous people.ini is back." }, "status": status(&f)?}))
    })
    .await
}

/// Loner-Strike's own people.ini (to start over from), parsed but not saved.
async fn api_shipped(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let res = crate::setup_api::resources_for(&st);
    blocking(move || {
        let p = res?.join("plugins/configs/gamemodes/people.ini");
        let text = read_text(&p)?.ok_or("Couldn't find Loner-Strike's own people.ini.")?;
        let people = parse(&text).people;
        Ok(json!({"problems": check(&people), "people": people}))
    })
    .await
}

/// Saves the model as a people.ini file the player picks (for sharing).
async fn api_export(State(st): State<Shared>, h: HeaderMap, Json(r): Json<CheckReq>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err(INTERNAL.into())) };
    let base = target(&st).ok().and_then(|(_, f)| read_text(&f.people()).ok().flatten()).unwrap_or_default();
    blocking(move || {
        let Some(path) = app.dialog().file().set_title("Save the bots' people").set_file_name("people.ini").add_filter("Settings file", &["ini"]).blocking_save_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        crate::sys::write_latin1_atomic(&path, &parse(&base).write(&r.people)).map_err(|e| format!("Couldn't save the file: {e}"))?;
        Ok(json!({"ok": true, "message": format!("Saved as {}.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())}))
    })
    .await
}

/// Reads a people.ini the player picks into the editor (not saved until they save).
async fn api_import(State(st): State<Shared>, h: HeaderMap) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err(INTERNAL.into())) };
    blocking(move || {
        let Some(path) = app.dialog().file().set_title("Open a bots' people file").add_filter("Settings file", &["ini", "txt"]).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let text = read_text(&path)?.ok_or("Couldn't open that file.")?;
        let people = parse(&text).people;
        if people.archetypes.is_empty() {
            return Err("That file has no kinds of people in it (an [archetypes] section). Is it a people.ini?".into());
        }
        Ok(json!({"problems": check(&people), "people": people}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/people", get(api_status))
        .route("/api/tools/people/check", post(api_check))
        .route("/api/tools/people/save", post(api_save))
        .route("/api/tools/people/restore", post(api_restore))
        .route("/api/tools/people/shipped", post(api_shipped))
        .route("/api/tools/people/export", post(api_export))
        .route("/api/tools/people/import", post(api_import))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(file: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/configs/gamemodes").join(file)).unwrap()
    }

    #[test]
    fn real_people_ini_round_trips() {
        let text = real("people.ini");
        let doc = parse(&text);
        assert!(doc.people.archetypes.len() > 100, "{}", doc.people.archetypes.len());
        assert_eq!(doc.people.weights.len(), 10);
        assert!(doc.people.conflicts.len() >= 10);
        assert_eq!(doc.write(&doc.people), text);
        // CRLF stays CRLF
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(parse(&crlf).write(&parse(&crlf).people), crlf);
        // the shipped file has no errors
        let errors: Vec<_> = check(&doc.people).into_iter().filter(|p| p.error).collect();
        assert!(errors.is_empty(), "{errors:?}");
        let a = &doc.people.archetypes[0];
        assert_eq!((a.name.as_str(), a.weight, a.letter.as_str(), a.style.as_str()), ("just_plays", 60, "q", "-"));
        assert_eq!(a.fixed, vec![Pair { key: "talk".into(), value: "quiet".into() }, Pair { key: "humor".into(), value: "none".into() }]);
        assert!(a.group.contains("everyday majority"));
    }

    #[test]
    fn edits_write_cleanly() {
        let text = real("people.ini");
        let doc = parse(&text);
        let mut p = doc.people.clone();
        // a copy at the end of its group, one removed, a weight changed
        let mut copy = p.archetypes[0].clone();
        copy.name = "just_plays_2".into();
        p.archetypes.insert(1, copy);
        p.archetypes.retain(|a| a.name != "troll");
        p.weights[0].weights = vec![10, 50, 40];
        let out = doc.write(&p);
        assert!(out.starts_with("; Who the bots are"));
        assert!(out.contains("just_plays|60|q|-|talk=quiet humor=none\njust_plays_2|60|q|-|talk=quiet humor=none\n"));
        assert!(!out.contains("\ntroll|"));
        assert!(out.contains("warmth|10 50 40\n"));
        let again = parse(&out);
        assert_eq!(again.people, p);
        // a new kind with no heading goes after a blank line
        let mut q = p.clone();
        q.archetypes.push(Arch { name: "mine".into(), weight: 5, letter: "f".into(), style: "awper".into(), fixed: vec![], group: String::new() });
        let out = doc.write(&q);
        assert!(out.ends_with("temper=tilty\n\nmine|5|f|awper|\n"), "{}", &out[out.len() - 80..]);
        assert_eq!(parse(&out).people, q);
    }

    #[test]
    fn checks_catch_what_the_game_would_trip_on() {
        let mut p = parse(&real("people.ini")).people;
        p.archetypes[0].name = "has space".into();
        p.archetypes[1].name = p.archetypes[2].name.clone();
        p.archetypes[3].letter = "!".into();
        p.archetypes[4].style = "sprinter".into();
        p.archetypes[5].fixed.push(Pair { key: "age".into(), value: "ancient".into() });
        p.weights[0].weights.pop();
        p.conflicts.push(Conflict { a: Pair { key: "talk".into(), value: "silent".into() }, b: Pair { key: "colour".into(), value: "red".into() } });
        let errs: Vec<String> = check(&p).into_iter().filter(|x| x.error).map(|x| x.at).collect();
        for want in ["arch:0", "arch:2", "arch:3", "arch:4", "arch:5", "weight:warmth"] {
            assert!(errs.iter().any(|e| e == want), "{want} in {errs:?}");
        }
        assert!(errs.iter().any(|e| e.starts_with("conflict:")));
        let mut empty = People::default();
        assert!(check(&empty).iter().any(|x| x.error));
        empty.archetypes.push(Arch { name: "a".into(), weight: 0, letter: "f".into(), style: "-".into(), fixed: vec![], group: String::new() });
        assert!(check(&empty).iter().any(|x| x.error && x.text.contains("picked")));
    }

    #[test]
    fn names_edit_in_place() {
        let text = real("regular_names.ini");
        let all = names(&text);
        assert!(all.len() > 1000);
        assert_eq!(edit_names(&text, &NameEdits::default()).unwrap(), text);
        let e = NameEdits { renamed: vec![("Crackle".into(), "Crackle2".into())], removed: vec!["Moose".into()], added: vec!["Newbie Ned".into(), "Crackle2".into()] };
        let out = edit_names(&text, &e).unwrap();
        assert!(out.starts_with("; Names for the bot regulars"));
        assert!(out.contains("\nCrackle2\n") && !out.contains("\nMoose\n") && out.ends_with("\nNewbie Ned\n"));
        assert_eq!(names(&out).iter().filter(|n| *n == "Crackle2").count(), 1);
        assert_eq!(names(&out).len(), all.len());
        assert!(edit_names(&text, &NameEdits { added: vec!["nacho2004".into()], ..Default::default() }).is_err());
        assert!(edit_names(&text, &NameEdits { added: vec!["a\"b".into()], ..Default::default() }).is_err());
        assert!(edit_names(&text, &NameEdits { renamed: vec![("Crackle".into(), "Ember".into())], ..Default::default() }).is_err());
        assert_eq!(name_problem("x".repeat(32).as_str()), Some("is 32 characters or more"));
        assert_eq!(name_problem("thunder123"), None);
    }

    #[test]
    fn regulars_forget() {
        let t = "Dash = |=HaZe=| Vapor\nZed = -=WaR=- OldRattle\n";
        assert_eq!(regulars(t), vec![("Dash".into(), "|=HaZe=| Vapor".into()), ("Zed".into(), "-=WaR=- OldRattle".into())]);
        assert_eq!(forget_regulars(t, &["Dash".into()]), "Zed = -=WaR=- OldRattle\n");
        assert_eq!(forget_regulars(t, &["Dash".into(), "Zed".into()]), "");
    }

    #[test]
    fn who_matches_the_plugin() {
        assert_eq!(bare_name("-=[CoC]=- Bob"), "Bob");
        assert_eq!(bare_name("[FX]Dave"), "Dave");
        assert_eq!(bare_name("Dave |AX|"), "Dave");
        assert_eq!(bare_name("GG | Glacier"), "GG | Glacier");
        assert_eq!(bare_name("[TAG]"), "[TAG]");
        // FNV-1a in 32-bit cells, as Pawn does it
        assert_eq!(name_seed(""), (2166136261u32 & 0x7FFF_FFFF) as i32);
        let p = parse(&real("people.ini")).people;
        // the same name is the same person; a tag doesn't change who it is (but its role)
        let a = who(&p, "Moose").unwrap();
        assert_eq!(who(&p, "Moose").unwrap(), a);
        assert_eq!(who(&p, "[XyZ] Moose").unwrap().0, a.0);
        assert_ne!(who(&p, "(1)Moose").unwrap(), a);
        // fixed traits hold, and many names spread over many kinds
        let mut kinds = std::collections::HashSet::new();
        for n in names(&real("regular_names.ini")).iter().take(400) {
            let (k, t, h) = who(&p, n).unwrap();
            kinds.insert(k);
            for f in &p.archetypes[k].fixed {
                match f.index().unwrap() {
                    (Some(ti), v) => assert_eq!(t[ti], v, "{n} {f:?}"),
                    (None, v) => assert!(h.contains(&v)),
                }
            }
            assert_ne!(h[0], h[1]);
        }
        assert!(kinds.len() > 40, "{}", kinds.len());
    }

    #[test]
    fn save_with_backups() {
        let root = std::env::temp_dir().join(format!("ls-people-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let f = Files { cfg: root.join("cfg"), data: root.join("data"), app: root.join("app"), label: "test".into() };
        assert_eq!(status(&f).unwrap_err(), NOT_SET_UP);
        std::fs::create_dir_all(&f.cfg).unwrap();
        std::fs::create_dir_all(&f.data).unwrap();
        let text = real("people.ini");
        std::fs::write(f.people(), &text).unwrap();
        std::fs::write(f.names(), "; names\nAlpha\nBravo\n").unwrap();
        std::fs::write(f.regulars(), "Bot1 = Alpha\nBot2 = Bravo\n").unwrap();
        let v = status(&f).unwrap();
        assert_eq!(v["names"], json!(["Alpha", "Bravo"]));
        assert_eq!(v["regulars"][1]["name"], "Bravo");
        // nothing changed: nothing written
        let mut p: People = serde_json::from_value(v["people"].clone()).unwrap();
        assert!(save(&f, &SaveReq { people: Some(p.clone()), ..Default::default() }).unwrap().is_empty());
        // an error stops everything
        p.archetypes[0].letter = String::new();
        assert!(save(&f, &SaveReq { people: Some(p.clone()), names: NameEdits { added: vec!["Charlie".into()], ..Default::default() }, ..Default::default() }).is_err());
        assert_eq!(std::fs::read_to_string(f.names()).unwrap(), "; names\nAlpha\nBravo\n");
        p.archetypes[0].letter = "f".into();
        let wrote = save(&f, &SaveReq { people: Some(p), names: NameEdits { added: vec!["Charlie".into()], ..Default::default() }, forget: vec!["Bot1".into()] }).unwrap();
        assert_eq!(wrote, vec!["people", "names", "regulars"]);
        assert!(std::fs::read_to_string(f.people()).unwrap().contains("just_plays|60|f|"));
        assert_eq!(std::fs::read_to_string(f.regulars()).unwrap(), "Bot2 = Bravo\n");
        assert_eq!(backups(&f.app, "people.ini").len(), 1);
        restore(&f).unwrap();
        assert_eq!(std::fs::read_to_string(f.people()).unwrap(), text);
        assert!(restore(&f).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
