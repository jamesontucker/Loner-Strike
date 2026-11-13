//! Crosshair & config builder: the player's own Counter-Strike 1.6 / Condition Zero settings
//! (crosshair, binds, rates, mouse, a few HUD and video basics), written to `userconfig.cfg`.
//!
//! Why userconfig.cfg: the game rewrites config.cfg every time it quits (whatever the player
//! changed in Options), and ends it with `exec userconfig.cfg` ("Add custom configurations to
//! the file userconfig.cfg", in the engine's own words). So userconfig.cfg runs after it on
//! every start and is never touched by the game. The app's own F3/F4 binds stay in config.cfg
//! (setup::core::binds); the builder never binds those keys.
//!
//! Only our block is ours: the lines between START and END. Everything else in the file is
//! the player's and stays as it is, in its place. The block is rewritten whole on each save
//! (at the end of the file, so it runs after the player's other lines... except those they
//! put after it later, which win, and the page says so). The file is backed up in the app's
//! folder (playercfg/backups) before each save; "Put back" restores the newest copy.
//!
//! A setting goes in the block only when it differs from what the game would use without
//! the block (config.cfg plus the player's own userconfig lines), so saving doesn't freeze
//! every Options setting. Binds always go in: they're what the player asked for.
//!
//! Writing while the game is open is harmless (it reads the file only when it starts), so
//! the save goes ahead and the answer says to restart the game (or `exec userconfig.cfg`).
//!
//! App window only (merged into tools::local_routes). Errors are `{error}` with status 200,
//! like the rest of the app; POSTs need the panel header.

use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const NO_GAME: &str = "Set up the game on this computer first.";
const INTERNAL: &str = "Loner-Strike hit an internal error with your settings. Try again, and copy the diagnostics from Settings if it keeps happening.";
pub const FILE: &str = "userconfig.cfg";
pub const START: &str = "// >>> Loner-Strike: Crosshair & config builder (start)";
pub const END: &str = "// <<< Loner-Strike: Crosshair & config builder (end)";
const KEEP: usize = 10;
/// The biggest config file we read (a real one is a few KB).
const MAX_FILE: u64 = 1 << 20;
const GAMES: [(&str, &str); 2] = [("cstrike", "Counter-Strike 1.6"), ("czero", "Condition Zero")];

// ---- what can be set --------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Bool,
    Num { min: f64, max: f64, int: bool },
    Choice(&'static [&'static str]),
    /// "R G B", each 0..255
    Colour,
}

/// The settings the builder writes, checked against the 2013+ client and engine (strings in
/// client.dll and hw.dll), with the game's defaults.
pub const CVARS: &[(&str, Kind, &str)] = &[
    // crosshair (client.dll)
    ("crosshair", Kind::Bool, "1"),
    ("cl_crosshair_color", Kind::Colour, "50 250 50"),
    ("cl_crosshair_size", Kind::Choice(&["auto", "small", "medium", "large"]), "auto"),
    ("cl_crosshair_translucent", Kind::Bool, "1"),
    ("cl_dynamiccrosshair", Kind::Bool, "1"),
    ("cl_observercrosshair", Kind::Bool, "1"),
    // net (hw.dll)
    ("rate", Kind::Num { min: 2500.0, max: 100000.0, int: true }, "25000"),
    ("cl_updaterate", Kind::Num { min: 10.0, max: 128.0, int: true }, "60"),
    ("cl_cmdrate", Kind::Num { min: 10.0, max: 128.0, int: true }, "60"),
    ("ex_interp", Kind::Num { min: 0.0, max: 0.1, int: false }, "0.1"),
    // mouse
    ("sensitivity", Kind::Num { min: 0.1, max: 20.0, int: false }, "3"),
    ("m_rawinput", Kind::Bool, "1"),
    ("zoom_sensitivity_ratio", Kind::Num { min: 0.1, max: 4.0, int: false }, "1.2"),
    ("m_filter", Kind::Bool, "0"),
    // video, HUD
    ("fps_max", Kind::Num { min: 30.0, max: 1000.0, int: true }, "100"),
    ("hud_fastswitch", Kind::Bool, "0"),
    ("hud_centerid", Kind::Bool, "0"),
    ("cl_righthand", Kind::Bool, "1"),
    ("_cl_autowepswitch", Kind::Bool, "1"),
    ("cl_radartype", Kind::Bool, "0"),
    ("cl_minmodels", Kind::Bool, "0"),
    ("cl_showfps", Kind::Bool, "0"),
    ("net_graph", Kind::Choice(&["0", "1", "2", "3"]), "0"),
    // sound, voice
    ("volume", Kind::Num { min: 0.0, max: 1.0, int: false }, "0.8"),
    ("voice_enable", Kind::Bool, "1"),
    ("voice_scale", Kind::Num { min: 0.0, max: 2.0, int: false }, "1"),
];

fn cvar(name: &str) -> Option<&'static (&'static str, Kind, &'static str)> {
    CVARS.iter().find(|(n, ..)| n.eq_ignore_ascii_case(name))
}

/// A value as the game would write it, or why it isn't one.
pub fn check_value(name: &str, v: &str) -> Result<String, String> {
    let Some((n, kind, _)) = cvar(name) else { return Err(format!("{name} isn't a setting the builder knows.")) };
    let v = v.trim();
    let bad = || format!("{v:?} isn't a value for {n}.");
    match kind {
        Kind::Bool => match v {
            "0" | "1" => Ok(v.to_string()),
            _ => Err(bad()),
        },
        Kind::Num { min, max, int } => {
            let f: f64 = v.parse().map_err(|_| bad())?;
            if !f.is_finite() || f < *min - 1e-9 || f > *max + 1e-9 {
                return Err(format!("{n} goes from {min} to {max}."));
            }
            Ok(if *int { format!("{}", f.round() as i64) } else { trim_num(f) })
        }
        Kind::Choice(list) => list.iter().find(|c| c.eq_ignore_ascii_case(v)).map(|c| c.to_string()).ok_or_else(bad),
        Kind::Colour => {
            let parts: Vec<&str> = v.split_whitespace().collect();
            if parts.len() != 3 || !parts.iter().all(|p| p.parse::<u8>().is_ok()) {
                return Err(bad());
            }
            Ok(parts.iter().map(|p| p.parse::<u8>().unwrap().to_string()).collect::<Vec<_>>().join(" "))
        }
    }
}

/// 0.0100 -> "0.01", 3.0 -> "3"
fn trim_num(f: f64) -> String {
    let s = format!("{f:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Two values of a setting mean the same ("0.10" and "0.1", "auto" and "AUTO").
fn same_value(name: &str, a: &str, b: &str) -> bool {
    match (check_value(name, a), check_value(name, b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// What a key can do: (id, the command bound, aliases it needs).
pub const ACTIONS: &[(&str, &str, &[&str])] = &[
    ("jumpduck", "+ls_jumpduck", &["alias +ls_jumpduck \"+jump; +duck\"", "alias -ls_jumpduck \"-jump; -duck\""]),
    ("jump", "+jump", &[]),
    ("duck", "+duck", &[]),
    ("lastinv", "lastinv", &[]),
    ("qswitch", "ls_qswitch", &["alias ls_qswitch \"slot3; wait; slot1\""]),
    ("voice", "+voicerecord", &[]),
    ("use", "+use", &[]),
    ("drop", "drop", &[]),
    ("chat", "messagemode", &[]),
    ("teamchat", "messagemode2", &[]),
    ("buymenu", "buy", &[]),
    ("buy_rifle", "ak47; m4a1", &[]),
    ("buy_awp", "awp", &[]),
    ("buy_deagle", "deagle", &[]),
    ("buy_armor", "vesthelm", &[]),
    ("buy_defuser", "defuser", &[]),
    ("buy_nades", "hegren; flash; flash; sgren", &[]),
    ("buy_ammo", "primammo; secammo", &[]),
    ("netgraph", "ls_netgraph", &["alias ls_netgraph \"ls_netgraph_on\"", "alias ls_netgraph_on \"net_graph 3; alias ls_netgraph ls_netgraph_off\"", "alias ls_netgraph_off \"net_graph 0; alias ls_netgraph ls_netgraph_on\""]),
];

fn action(id: &str) -> Option<&'static (&'static str, &'static str, &'static [&'static str])> {
    ACTIONS.iter().find(|(a, ..)| *a == id)
}

/// Keys the app's own menus use (setup::core::BINDS): not for the builder.
pub const RESERVED: &[(&str, &str)] = &[("f3", "the Game Mode Menu"), ("f4", "the quick menu")];

/// A key name the game knows, lowercased as config.cfg writes it; None for anything else
/// (and for the console key, which a bind would take away).
pub fn key_name(k: &str) -> Option<String> {
    let k = k.trim();
    let low = k.to_ascii_lowercase();
    if low.len() == 1 {
        let c = low.as_bytes()[0];
        return (c.is_ascii_lowercase() || c.is_ascii_digit() || b";',./[]-=\\".contains(&c)).then_some(low);
    }
    const NAMED: &[&str] = &[
        "space", "tab", "enter", "shift", "ctrl", "alt", "backspace", "capslock", "ins", "del", "home", "end", "pgup", "pgdn", "uparrow", "downarrow", "leftarrow", "rightarrow", "pause", "mouse1", "mouse2", "mouse3",
        "mouse4", "mouse5", "mwheelup", "mwheeldown", "kp_home", "kp_uparrow", "kp_pgup", "kp_leftarrow", "kp_5", "kp_rightarrow", "kp_end", "kp_downarrow", "kp_pgdn", "kp_enter", "kp_ins", "kp_del", "kp_slash",
        "kp_minus", "kp_plus", "semicolon",
    ];
    if NAMED.contains(&low.as_str()) {
        return Some(low);
    }
    if let Some(n) = low.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=12).contains(&n) {
            return Some(low);
        }
    }
    None
}

// ---- reading configs --------------------------------------------------------------------

/// One line's commands, each as its words: `;` splits commands and `//` starts a comment,
/// both only outside quotes; quoted words keep their spaces.
pub fn commands(line: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut quoted, mut had) = (false, false);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let flush_word = |cur: &mut String, had: &mut bool, words: &mut Vec<String>| {
        if *had || !cur.is_empty() {
            words.push(std::mem::take(cur));
        }
        *had = false;
    };
    while i < chars.len() {
        let c = chars[i];
        if quoted {
            if c == '"' {
                quoted = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            quoted = true;
            had = true;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            break;
        } else if c == ';' {
            flush_word(&mut cur, &mut had, &mut words);
            if !words.is_empty() {
                out.push(std::mem::take(&mut words));
            }
        } else if c.is_whitespace() {
            flush_word(&mut cur, &mut had, &mut words);
        } else {
            cur.push(c);
        }
        i += 1;
    }
    flush_word(&mut cur, &mut had, &mut words);
    if !words.is_empty() {
        out.push(words);
    }
    out
}

/// What a config sets: the known settings (by lowercase name) and binds (key -> command).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Sets {
    pub cvars: BTreeMap<String, String>,
    pub binds: BTreeMap<String, String>,
}

impl Sets {
    pub fn read(&mut self, text: &str) {
        for line in text.lines() {
            for cmd in commands(line) {
                let head = cmd[0].to_ascii_lowercase();
                match head.as_str() {
                    "bind" if cmd.len() >= 3 => {
                        self.binds.insert(cmd[1].to_ascii_lowercase(), cmd[2].clone());
                    }
                    "unbind" if cmd.len() >= 2 => {
                        self.binds.remove(&cmd[1].to_ascii_lowercase());
                    }
                    _ if cmd.len() >= 2 && cvar(&head).is_some() => {
                        self.cvars.insert(head, cmd[1].clone());
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A userconfig.cfg split up: the player's own lines (block taken out) and our block's lines.
#[derive(Debug, Default, PartialEq)]
pub struct Split {
    /// the player's lines, in order, with where our block was (None: not there)
    pub own: Vec<String>,
    pub block_at: Option<usize>,
    pub block: Vec<String>,
    /// lines of theirs after our block (they win over it)
    pub after: Vec<String>,
    pub crlf: bool,
}

pub fn split(text: &str) -> Split {
    let crlf = text.contains("\r\n");
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.trim() == START);
    let end = start.and_then(|s| lines[s + 1..].iter().position(|l| l.trim() == END).map(|e| s + 1 + e));
    let mut sp = Split { crlf, ..Default::default() };
    match (start, end) {
        (Some(s), Some(e)) => {
            sp.own.extend(lines[..s].iter().map(|l| l.to_string()));
            sp.block_at = Some(s);
            sp.block = lines[s + 1..e].iter().map(|l| l.to_string()).collect();
            sp.after = lines[e + 1..].iter().map(|l| l.to_string()).collect();
            sp.own.extend(sp.after.iter().cloned());
        }
        // a start with no end (cut by hand): only the marker line goes, nothing of theirs
        (Some(s), None) => {
            sp.own = lines.iter().enumerate().filter(|(i, _)| *i != s).map(|(_, l)| l.to_string()).collect();
        }
        _ => sp.own = lines.iter().map(|l| l.to_string()).collect(),
    }
    sp
}

/// The file with our block replaced by `block` (none: taken out). The player's lines stay
/// as they are and where they are; the block stays where it was, or a new one goes at the
/// end, with a blank line before it.
pub fn with_block(text: &str, block: &[String]) -> String {
    let sp = split(text);
    let nl = if sp.crlf || text.is_empty() { "\r\n" } else { "\n" };
    let mut out: Vec<String>;
    match sp.block_at {
        // the block stays where it is (lines they put after it keep winning over it)
        Some(at) => {
            out = sp.own[..at].to_vec();
            if block.is_empty() {
                // (the blank line we put before it, when nothing of theirs follows)
                if sp.after.is_empty() {
                    while out.last().is_some_and(|l| l.trim().is_empty()) {
                        out.pop();
                    }
                }
            } else {
                out.push(START.into());
                out.extend(block.iter().cloned());
                out.push(END.into());
            }
            out.extend(sp.after.iter().cloned());
        }
        // a new block goes at the end, after a blank line
        None => {
            out = sp.own;
            if !block.is_empty() {
                if out.last().is_some_and(|l| !l.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push(START.into());
                out.extend(block.iter().cloned());
                out.push(END.into());
            }
        }
    }
    if out.is_empty() {
        return String::new();
    }
    out.join(nl) + nl
}

// ---- the player's choice ----------------------------------------------------------------

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Bind {
    pub key: String,
    pub action: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Choice {
    /// setting -> value; a setting left out keeps what the game has
    pub settings: BTreeMap<String, String>,
    pub binds: Vec<Bind>,
}

/// Our block's lines for a choice, given what the game would use without it (`base`).
/// Checks everything: a bad value or key is an error in plain words.
pub fn block_for(choice: &Choice, base: &Sets) -> Result<Vec<String>, String> {
    let mut sets = Vec::new();
    for (name, v) in &choice.settings {
        let v = check_value(name, v)?;
        let name = cvar(name).map(|c| c.0).unwrap_or(name);
        let now = base.cvars.get(&name.to_ascii_lowercase()).map(String::as_str).unwrap_or_else(|| cvar(name).map(|c| c.2).unwrap_or(""));
        if !same_value(name, &v, now) {
            sets.push(format!("{name} \"{v}\""));
        }
    }
    let mut aliases: Vec<&str> = Vec::new();
    let mut binds = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for b in &choice.binds {
        let key = key_name(&b.key).ok_or_else(|| format!("{:?} isn't a key the game knows.", b.key))?;
        if let Some((_, what)) = RESERVED.iter().find(|(k, _)| *k == key) {
            return Err(format!("{} opens {what}. Pick another key.", key.to_uppercase()));
        }
        if !seen.insert(key.clone()) {
            return Err(format!("{} is used twice. Give each key one job.", key.to_uppercase()));
        }
        let (_, cmd, needs) = action(&b.action).ok_or_else(|| format!("{:?} isn't something a key can do here.", b.action))?;
        for a in needs.iter() {
            if !aliases.contains(a) {
                aliases.push(a);
            }
        }
        binds.push(format!("bind \"{key}\" \"{cmd}\""));
    }
    if sets.is_empty() && binds.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = vec!["// Made in Loner-Strike (Tools > Crosshair & config). Saving there replaces these lines;".to_string(), "// put your own lines above this block.".to_string()];
    out.extend(sets);
    out.extend(aliases.iter().map(|a| a.to_string()));
    out.extend(binds);
    Ok(out)
}

/// The choice back from a block we wrote (settings and binds we know; anything else skipped).
pub fn choice_from_block(block: &[String]) -> Choice {
    let mut c = Choice::default();
    for line in block {
        for cmd in commands(line) {
            let head = cmd[0].to_ascii_lowercase();
            if head == "bind" && cmd.len() >= 3 {
                if let Some((id, ..)) = ACTIONS.iter().find(|(_, command, _)| command.eq_ignore_ascii_case(&cmd[2])) {
                    c.binds.push(Bind { key: cmd[1].to_ascii_lowercase(), action: id.to_string() });
                }
            } else if cmd.len() >= 2 {
                if let Some((n, ..)) = cvar(&head) {
                    c.settings.insert(n.to_string(), cmd[1].clone());
                }
            }
        }
    }
    c
}

// ---- files ------------------------------------------------------------------------------

fn read_small(p: &Path) -> Option<String> {
    if std::fs::metadata(p).map(|m| m.len() > MAX_FILE).unwrap_or(true) {
        return None;
    }
    crate::sys::read_latin1(p).ok()
}

/// What the game uses without our block: config.cfg, then the player's userconfig lines.
pub fn base_sets(dir: &Path) -> (Sets, Split) {
    let mut s = Sets::default();
    if let Some(t) = read_small(&dir.join("config.cfg")) {
        s.read(&t);
    }
    let sp = split(&read_small(&dir.join(FILE)).unwrap_or_default());
    s.read(&sp.own.join("\n"));
    (s, sp)
}

fn backup_dir(app: &Path) -> PathBuf {
    app.join("playercfg").join("backups")
}

fn backups(app: &Path, game: &str) -> Vec<PathBuf> {
    let prefix = format!("{game}-");
    let mut v: Vec<PathBuf> = std::fs::read_dir(backup_dir(app))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".cfg")))
        .collect();
    v.sort();
    v.reverse();
    v
}

fn game_dir(hl: &Path, game: &str) -> Result<(&'static str, PathBuf), String> {
    GAMES
        .iter()
        .find(|(g, _)| *g == game)
        .map(|&(g, label)| (label, hl.join(g)))
        .filter(|(_, d)| d.is_dir())
        .map(|(label, d)| (label, d))
        .ok_or_else(|| "That game isn't on this computer.".to_string())
}

/// Writes userconfig.cfg (the old one backed up first when `backup`).
fn put(app: &Path, game: &str, dir: &Path, text: &str, backup: bool) -> Result<(), String> {
    let target = dir.join(FILE);
    crate::safety::before("Crosshair & config", if backup { "Saved userconfig.cfg" } else { "Changed userconfig.cfg" }, &target);
    if backup && target.is_file() {
        let b = backup_dir(app);
        std::fs::create_dir_all(&b).map_err(|e| format!("Couldn't keep a copy of your old {FILE}: {e}"))?;
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        std::fs::copy(&target, b.join(format!("{game}-{ms:015}.cfg"))).map_err(|e| format!("Couldn't keep a copy of your old {FILE}: {e}"))?;
        for old in backups(app, game).into_iter().skip(KEEP) {
            let _ = std::fs::remove_file(old);
        }
    }
    crate::sys::make_writable(&target);
    crate::sys::write_latin1_atomic(&target, text).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Loner-Strike isn't allowed to change files in the {game} folder."),
        _ => format!("Couldn't save {FILE}: {e}"),
    })
}

/// What the page shows for one game.
pub fn game_status(app: &Path, game: &str, label: &str, dir: &Path) -> Value {
    let (base, sp) = base_sets(dir);
    let ours = choice_from_block(&sp.block);
    // the effective values: the game's defaults, then config.cfg and their lines, then ours
    let mut current = Map::new();
    for (n, _, d) in CVARS {
        let v = ours.settings.get(*n).or_else(|| base.cvars.get(*n)).map(String::as_str).unwrap_or(d);
        current.insert(n.to_string(), json!(check_value(n, v).unwrap_or_else(|_| d.to_string())));
    }
    // their own lines after our block that set what we set: those win
    let mut later = Sets::default();
    later.read(&sp.after.join("\n"));
    let overridden: Vec<String> = later.cvars.keys().filter(|k| ours.settings.contains_key(k.as_str())).cloned().chain(later.binds.keys().filter(|k| ours.binds.iter().any(|b| &b.key == *k)).cloned()).collect();
    json!({
        "game": game,
        "label": label,
        "exists": dir.join(FILE).is_file(),
        "current": current,
        "binds": base.binds,
        "ours": {"settings": ours.settings, "binds": ours.binds.iter().map(|b| json!({"key": b.key, "action": b.action})).collect::<Vec<_>>()},
        "hasBlock": sp.block_at.is_some(),
        "ownLines": sp.own.iter().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//")).count(),
        "overridden": overridden,
        "canRestore": !backups(app, game).is_empty(),
    })
}

pub fn status(hl: &Path, app: &Path) -> Value {
    let games: Vec<Value> = GAMES.iter().filter(|(g, _)| hl.join(g).is_dir()).map(|&(g, label)| game_status(app, g, label, &hl.join(g))).collect();
    json!({"games": games, "running": crate::game::game_open()})
}

fn after_save(label: &str) -> String {
    if crate::game::game_open() {
        format!("Saved for {label}. The game is open: restart it to use them (or type exec userconfig.cfg in its console).")
    } else {
        format!("Saved for {label}. They're used from the next time you start the game.")
    }
}

pub fn save(hl: &Path, app: &Path, game: &str, choice: &Choice) -> Result<Value, String> {
    let (label, dir) = game_dir(hl, game)?;
    let (base, _) = base_sets(&dir);
    let block = block_for(choice, &base)?;
    let old = read_small(&dir.join(FILE)).unwrap_or_default();
    let text = with_block(&old, &block);
    if text != old {
        put(app, game, &dir, &text, true)?;
    }
    Ok(json!({"ok": true, "message": after_save(label), "status": game_status(app, game, label, &dir)}))
}

/// The lines a save would write (for the page's "What gets saved").
pub fn preview(hl: &Path, game: &str, choice: &Choice) -> Result<Value, String> {
    let (_, dir) = game_dir(hl, game)?;
    let (base, _) = base_sets(&dir);
    let block = block_for(choice, &base)?;
    Ok(json!({"lines": block}))
}

/// Takes our block out (their own lines stay).
pub fn remove(hl: &Path, app: &Path, game: &str) -> Result<Value, String> {
    let (label, dir) = game_dir(hl, game)?;
    let old = read_small(&dir.join(FILE)).unwrap_or_default();
    let text = with_block(&old, &[]);
    if text != old {
        put(app, game, &dir, &text, true)?;
    }
    Ok(json!({"ok": true, "message": format!("Took the builder's settings out of {label}'s {FILE}. Restart the game to go back to its own."), "status": game_status(app, game, label, &dir)}))
}

/// Puts the newest kept copy back (and forgets it, so again goes one further back).
pub fn restore(hl: &Path, app: &Path, game: &str) -> Result<Value, String> {
    let (label, dir) = game_dir(hl, game)?;
    let Some(newest) = backups(app, game).into_iter().next() else {
        return Err(format!("There's no earlier {FILE} kept for {label}."));
    };
    let text = crate::sys::read_latin1(&newest).map_err(|e| format!("Couldn't read the kept copy: {e}"))?;
    put(app, game, &dir, &text, false)?;
    let _ = std::fs::remove_file(&newest);
    Ok(json!({"ok": true, "message": format!("Your previous {FILE} is back for {label}."), "status": game_status(app, game, label, &dir)}))
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

fn folders(st: &Shared) -> Result<(PathBuf, PathBuf), String> {
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.remote.is_some() {
        return Err("Your own game isn't on this computer.".into());
    }
    Ok((g.hl.clone(), g.app.clone()))
}

#[derive(Deserialize)]
struct Req {
    game: String,
    #[serde(default)]
    choice: Choice,
}

async fn api_status(State(st): State<Shared>) -> Response {
    match folders(&st) {
        Ok((hl, app)) => blocking(move || Ok(status(&hl, &app))).await,
        Err(e) => answer(Err(e)),
    }
}

macro_rules! post_route {
    ($name:ident, |$hl:ident, $app:ident, $r:ident| $body:expr) => {
        async fn $name(State(st): State<Shared>, h: HeaderMap, Json($r): Json<Req>) -> Response {
            if !crate::panel_api::from_panel(&h) {
                return forbidden();
            }
            match folders(&st) {
                Ok(($hl, $app)) => blocking(move || $body).await,
                Err(e) => answer(Err(e)),
            }
        }
    };
}

post_route!(api_preview, |hl, _app, r| preview(&hl, &r.game, &r.choice));
post_route!(api_save, |hl, app, r| save(&hl, &app, &r.game, &r.choice));
post_route!(api_remove, |hl, app, r| remove(&hl, &app, &r.game));
post_route!(api_restore, |hl, app, r| restore(&hl, &app, &r.game));

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/playercfg", get(api_status))
        .route("/api/tools/playercfg/preview", post(api_preview))
        .route("/api/tools/playercfg/save", post(api_save))
        .route("/api/tools/playercfg/remove", post(api_remove))
        .route("/api/tools/playercfg/restore", post(api_restore))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-playercfg-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("hl/cstrike")).unwrap();
        d
    }

    #[test]
    fn commands_split_like_the_console() {
        assert_eq!(commands(r#"bind "F5" "ak47; m4a1" // buy"#), vec![vec!["bind".to_string(), "F5".into(), "ak47; m4a1".into()]]);
        assert_eq!(commands("sensitivity 2.5; fps_max \"101\""), vec![vec!["sensitivity".to_string(), "2.5".into()], vec!["fps_max".to_string(), "101".into()]]);
        assert_eq!(commands(r#"name """#), vec![vec!["name".to_string(), "".into()]]);
        assert!(commands("// only a comment").is_empty());
    }

    #[test]
    fn values_are_checked() {
        assert_eq!(check_value("ex_interp", "0.0100").unwrap(), "0.01");
        assert_eq!(check_value("RATE", "25000.4").unwrap(), "25000");
        assert_eq!(check_value("cl_crosshair_color", " 0  255 255 ").unwrap(), "0 255 255");
        assert_eq!(check_value("cl_crosshair_size", "Small").unwrap(), "small");
        assert!(check_value("cl_crosshair_color", "0 256 0").is_err());
        assert!(check_value("fps_max", "5").is_err());
        assert!(check_value("rcon_password", "x").is_err());
        assert!(check_value("crosshair", "2").is_err());
        assert_eq!(key_name("F5").as_deref(), Some("f5"));
        assert_eq!(key_name("MOUSE4").as_deref(), Some("mouse4"));
        assert_eq!(key_name("`"), None);
        assert_eq!(key_name("\""), None);
        assert_eq!(key_name("f13"), None);
    }

    #[test]
    fn block_round_trip_keeps_the_players_lines() {
        let theirs = "// my stuff\r\nbind \"j\" \"say hi\"\r\nsensitivity 2\r\n";
        let base = {
            let mut s = Sets::default();
            s.read("cl_crosshair_color \"50 250 50\"\r\nex_interp 0.1\r\n");
            s.read(theirs);
            s
        };
        let mut c = Choice::default();
        c.settings.insert("cl_crosshair_color".into(), "0 255 255".into());
        // the same as the game has: not written
        c.settings.insert("ex_interp".into(), "0.10".into());
        c.settings.insert("sensitivity".into(), "2".into());
        c.binds.push(Bind { key: "SPACE".into(), action: "jumpduck".into() });
        c.binds.push(Bind { key: "f5".into(), action: "buy_rifle".into() });
        let block = block_for(&c, &base).unwrap();
        let once = with_block(theirs, &block);
        assert!(once.starts_with(theirs), "{once}");
        assert!(once.contains("cl_crosshair_color \"0 255 255\"\r\n") && !once.contains("ex_interp") && !once.contains("sensitivity \"2\""));
        assert!(once.contains("alias +ls_jumpduck \"+jump; +duck\"\r\n") && once.contains("bind \"space\" \"+ls_jumpduck\"\r\n"));
        // saving again is the same file; the choice reads back
        assert_eq!(with_block(&once, &block), once);
        let back = choice_from_block(&split(&once).block);
        assert_eq!(back.settings.get("cl_crosshair_color").map(String::as_str), Some("0 255 255"));
        assert_eq!(back.binds, vec![Bind { key: "space".into(), action: "jumpduck".into() }, Bind { key: "f5".into(), action: "buy_rifle".into() }]);
        // a line they add after the block stays after it; taking the block out leaves theirs as it was
        let later = format!("{once}fps_max 300\r\n");
        let again = with_block(&later, &block);
        assert!(again.starts_with(theirs) && again.ends_with("fps_max 300\r\n") && again.contains(START));
        assert_eq!(with_block(&once, &[]), theirs);
        assert_eq!(with_block(&again, &[]), format!("{theirs}\r\nfps_max 300\r\n"));
        // LF files stay LF; an empty file gets only the block
        let lf = with_block("sensitivity 2\n", &block);
        assert!(lf.contains("\n// >>>") && !lf.contains('\r'));
        assert!(with_block("", &block).starts_with(START));
        // a start marker with no end: only the marker goes
        let cut = format!("a 1\n{START}\nb 2\n");
        assert_eq!(split(&cut).own, vec!["a 1".to_string(), "b 2".to_string()]);
    }

    #[test]
    fn bad_binds_are_refused() {
        let base = Sets::default();
        let one = |key: &str, action: &str| block_for(&Choice { settings: Default::default(), binds: vec![Bind { key: key.into(), action: action.into() }] }, &base);
        assert!(one("f3", "voice").unwrap_err().contains("Game Mode Menu"));
        assert!(one("`", "voice").is_err());
        assert!(one("k", "quit").is_err());
        let twice = Choice { settings: Default::default(), binds: vec![Bind { key: "k".into(), action: "voice".into() }, Bind { key: "K".into(), action: "drop".into() }] };
        assert!(block_for(&twice, &base).unwrap_err().contains("twice"));
        assert!(block_for(&Choice::default(), &base).unwrap().is_empty());
    }

    #[test]
    fn save_backs_up_and_restores() {
        let root = tmp("save");
        let (hl, app) = (root.join("hl"), root.join("app"));
        let dir = hl.join("cstrike");
        std::fs::write(dir.join("config.cfg"), "bind \"F3\" \"gamemenu\"\r\nsensitivity \"3\"\r\nexec userconfig.cfg\r\n").unwrap();
        std::fs::write(dir.join(FILE), "say_team_hello 1\r\n").unwrap();
        let mut c = Choice::default();
        c.settings.insert("sensitivity".into(), "1.8".into());
        c.binds.push(Bind { key: "mouse4".into(), action: "voice".into() });
        let v = save(&hl, &app, "cstrike", &c).unwrap();
        assert_eq!(v["ok"], true);
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(text.starts_with("say_team_hello 1\r\n") && text.contains("sensitivity \"1.8\"") && text.contains("bind \"mouse4\" \"+voicerecord\""));
        assert_eq!(v["status"]["current"]["sensitivity"], "1.8");
        assert_eq!(v["status"]["ours"]["binds"][0]["action"], "voice");
        assert_eq!(backups(&app, "cstrike").len(), 1);
        // config.cfg is never touched
        assert!(std::fs::read_to_string(dir.join("config.cfg")).unwrap().contains("gamemenu"));
        restore(&hl, &app, "cstrike").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "say_team_hello 1\r\n");
        assert!(restore(&hl, &app, "cstrike").is_err());
        save(&hl, &app, "cstrike", &c).unwrap();
        remove(&hl, &app, "cstrike").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "say_team_hello 1\r\n");
        assert!(save(&hl, &app, "czero", &c).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
