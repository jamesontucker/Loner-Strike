//! The Studio's debug link with a Playtest build (see gen.rs for the plugin's side): the app
//! reads `<data>/studio/<mode>.events` from where it last stopped and writes commands into
//! `<data>/studio/<mode>.cmd`, which the plugin reads and deletes four times a second.
//!
//! Events are JSON objects, one per line: `{"s":seq,"t":gametime,"e":kind,...}` where kind is
//! `session`, `node_enter {node, inputs}`, `node_exit {node, result}`, `node_error {node,
//! message}`, `breakpoint_hit {node, inputs}`, `world_paused {node}`, `world_resumed {paused}`,
//! `vars {round, phase, timeLeft, aliveT, aliveCT, paused}`, `var_player {p}` and `commands`.

use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The most read in one go; the page asks again for the rest.
const MAX_READ: u64 = 512 * 1024;
/// The most events handed back in one go.
const MAX_EVENTS: usize = 3000;

pub fn dir(game_dir: &Path) -> PathBuf {
    game_dir.join("addons/amxmodx/data/studio")
}

pub fn events_file(game_dir: &Path, mode: &str) -> PathBuf {
    dir(game_dir).join(format!("{mode}.events"))
}

pub fn cmd_file(game_dir: &Path, mode: &str) -> PathBuf {
    dir(game_dir).join(format!("{mode}.cmd"))
}

/// The breakpoints that are on, one line of node ids: read by the plugin when a map loads
/// (a `bp` command only lasts until the next map).
pub fn bp_file(game_dir: &Path, mode: &str) -> PathBuf {
    dir(game_dir).join(format!("{mode}.bp"))
}

fn bp_line(ids: &[String]) -> String {
    ids.iter().map(String::as_str).filter(|i| super::graph::valid_id(i)).take(200).collect::<Vec<_>>().join(" ")
}

/// Saves the breakpoints for the next map load and tells the running plugin.
pub fn set_breakpoints(game_dir: &Path, mode: &str, ids: Vec<String>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(game_dir))?;
    crate::sys::write_atomic(&bp_file(game_dir, mode), format!("{}\n", bp_line(&ids)).as_bytes())?;
    send(&cmd_file(game_dir, mode), &[Command::Breakpoints(ids)])
}

/// What a read found: the events, where to read from next time, and whether the file was
/// started again (a new map: everything before is gone).
#[derive(Debug, PartialEq)]
pub struct Batch {
    pub events: Vec<Value>,
    pub next: u64,
    pub reset: bool,
}

/// Reads complete lines from `from` on. A half-written last line waits for the next read.
pub fn read_events(path: &Path, from: u64) -> std::io::Result<Batch> {
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Batch { events: vec![], next: 0, reset: from > 0 }),
        Err(e) => return Err(e),
    };
    let len = f.metadata()?.len();
    let (start, reset) = if from > len { (0, true) } else { (from, false) };
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.take(MAX_READ).read_to_end(&mut buf)?;
    Ok(parse_lines(&buf, start, reset))
}

pub fn parse_lines(buf: &[u8], start: u64, reset: bool) -> Batch {
    let mut events = Vec::new();
    let mut used = 0usize;
    let mut pos = 0usize;
    while let Some(nl) = buf[pos..].iter().position(|&b| b == b'\n') {
        let line = &buf[pos..pos + nl];
        pos += nl + 1;
        used = pos;
        // names can be any bytes; a line that isn't JSON is skipped, not fatal
        let text = String::from_utf8_lossy(line);
        if let Ok(v @ Value::Object(_)) = serde_json::from_str::<Value>(text.trim()) {
            events.push(v);
            if events.len() >= MAX_EVENTS {
                break;
            }
        }
    }
    Batch { events, next: start + used as u64, reset }
}

/// A command for the plugin: `resume`, `vars`, or `bp <node ids>` (the breakpoints that are on;
/// none turns them all off).
pub enum Command {
    Resume,
    Vars,
    Breakpoints(Vec<String>),
}

impl Command {
    fn line(&self) -> String {
        match self {
            Command::Resume => "resume".into(),
            Command::Vars => "vars".into(),
            Command::Breakpoints(ids) => {
                let ids = bp_line(ids);
                if ids.is_empty() { "bp".into() } else { format!("bp {ids}") }
            }
        }
    }
}

/// Adds commands to the file (after any the plugin hasn't read yet).
pub fn send(path: &Path, cmds: &[Command]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    if text.len() > 16 * 1024 {
        // nobody is reading: start over rather than grow forever
        text.clear();
    }
    for c in cmds {
        text.push_str(&c.line());
        text.push('\n');
    }
    crate::sys::write_atomic(path, text.as_bytes())
}

/// A fresh start for a Playtest: no old events, no old commands, the breakpoints as set now.
pub fn start(game_dir: &Path, mode: &str, breakpoints: Vec<String>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(game_dir))?;
    let _ = std::fs::remove_file(events_file(game_dir, mode));
    let _ = std::fs::remove_file(cmd_file(game_dir, mode));
    set_breakpoints(game_dir, mode, breakpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_whole_lines_only() {
        let b = parse_lines(b"{\"e\":\"session\"}\nnot json\n{\"e\":\"node_enter\",\"node\":\"a\"}\n{\"e\":\"half", 10, false);
        assert_eq!(b.events.len(), 2);
        assert_eq!(b.events[1]["node"], "a");
        assert_eq!(b.next, 10 + 55);
    }

    #[test]
    fn file_round_trip() {
        let d = std::env::temp_dir().join(format!("ls-studio-debug-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        start(&d, "mymode", vec!["n1".into(), "bad id!".into()]).unwrap();
        let cmd = cmd_file(&d, "mymode");
        assert_eq!(std::fs::read_to_string(&cmd).unwrap(), "bp n1\n");
        assert_eq!(std::fs::read_to_string(bp_file(&d, "mymode")).unwrap(), "n1\n");
        send(&cmd, &[Command::Resume, Command::Breakpoints(vec![])]).unwrap();
        assert_eq!(std::fs::read_to_string(&cmd).unwrap(), "bp n1\nresume\nbp\n");

        let ev = events_file(&d, "mymode");
        assert_eq!(read_events(&ev, 0).unwrap(), Batch { events: vec![], next: 0, reset: false });
        std::fs::write(&ev, "{\"s\":1,\"e\":\"session\"}\n{\"s\":2,\"e\":\"node_enter\"}\n").unwrap();
        let b = read_events(&ev, 0).unwrap();
        assert_eq!(b.events.len(), 2);
        let again = read_events(&ev, b.next).unwrap();
        assert!(again.events.is_empty() && !again.reset);
        // the map changed: the file started again, shorter
        std::fs::write(&ev, "{\"s\":1,\"e\":\"session\"}\n").unwrap();
        let r = read_events(&ev, b.next).unwrap();
        assert!(r.reset && r.events.len() == 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
