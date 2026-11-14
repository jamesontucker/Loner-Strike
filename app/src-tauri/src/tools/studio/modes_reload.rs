//! After a mode is added to or taken off a game, the Game Mode Menu reads modes.ini again
//! (`gm_modes_reload`), so `gm_mode <new>` works on the map that's playing. A server that's
//! off reads it when it starts; one whose menu is older than the command lists the change
//! after its next map.

use crate::rcon::{Rcon, RconError};
use serde_json::{json, Value};

/// The page's words when the server couldn't take the change on the spot.
pub const NEXT_MAP: &str = "Available after the next map change";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reload {
    /// the menu read modes.ini again: the mode can be switched to now
    Now,
    /// the server runs, but didn't take the command (an older menu, a password it turned away)
    NextMap,
    /// not running: it reads modes.ini when it starts
    Off,
}

impl Reload {
    pub fn as_str(self) -> &'static str {
        match self {
            Reload::Now => "now",
            Reload::NextMap => "next_map",
            Reload::Off => "off",
        }
    }

    /// `{"modes": "now"|"next_map"|"off", "modesNote": <words or null>}` for an API answer.
    pub fn json(self) -> Value {
        json!({"modes": self.as_str(), "modesNote": if self == Reload::NextMap { Some(NEXT_MAP) } else { None }})
    }
}

/// What a running game said to `gm_modes_reload`.
pub fn read_reply(reply: Result<String, RconError>) -> Reload {
    match reply {
        Ok(r) if r.contains("Modes reloaded") => Reload::Now,
        Err(RconError::Offline) => Reload::Off,
        _ => Reload::NextMap,
    }
}

pub fn send(rcon: &Rcon) -> Reload {
    read_reply(rcon.send("gm_modes_reload"))
}

/// A dedicated server (this computer's or another machine's), once its files are in place
/// there (for one on another machine: after the upload).
pub fn for_server(e: &crate::servers::Entry) -> Reload {
    if !crate::servers::running(&e.id) {
        return Reload::Off;
    }
    match e.rcon() {
        Some(r) => send(&r),
        None => Reload::NextMap,
    }
}

/// The game on this computer the Studio installs into, when it's open (or a server runs from
/// its folder).
pub fn for_local_game(g: &crate::game::Game) -> Reload {
    if !crate::sys::game_running() && !crate::servers::running_at(&g.dir) {
        return Reload::Off;
    }
    send(&g.rcon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies() {
        assert_eq!(read_reply(Ok("[Game Mode] Modes reloaded: 25\n".into())), Reload::Now);
        assert_eq!(read_reply(Ok("Unknown command: gm_modes_reload\n".into())), Reload::NextMap);
        assert_eq!(read_reply(Ok(String::new())), Reload::NextMap);
        assert_eq!(read_reply(Err(RconError::BadPassword)), Reload::NextMap);
        assert_eq!(read_reply(Err(RconError::Offline)), Reload::Off);
        assert_eq!(Reload::NextMap.json()["modesNote"], NEXT_MAP);
        assert!(Reload::Now.json()["modesNote"].is_null());
    }
}
