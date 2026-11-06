//! One game folder (czero or cstrike) and the Game Mode Menu plugin inside it.
//! Mirrors panel/panel.py's Game: live state over RCON, and the commands the panel may send.

use crate::rcon::{latin1, Rcon, RconError};
use rand::Rng;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

/// Commands the panel may send. Everything else is refused.
const ALLOWED: &[&str] = &[
    "gm_mode", "gm_map", "gm_set", "gm_addon", "gm_addonset", "gm_restartmap", "gm_bots", "gm_match",
    "gm_player", "gm_nav", "gm_pause", "gm_team", "gm_preset", "gm_presetsave", "gm_presetdel", "gm_pos",
    "gm_crash", "gm_do", "gm_announce",
];

pub struct Game {
    pub hl: PathBuf,
    pub name: String,
    pub dir: PathBuf,
    data: PathBuf,
    rcon: Rcon,
}

fn safe_arg(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && a.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
}

impl Game {
    pub fn new(hl: &PathBuf, name: &str) -> Game {
        let dir = hl.join(name);
        let data = dir.join("addons/amxmodx/data/gamemodes");
        let _ = std::fs::create_dir_all(&data);
        // The plugin reads the same file, so both sides share the RCON password
        let pw_file = data.join("panel_rcon.txt");
        let mut pw = std::fs::read_to_string(&pw_file).unwrap_or_default().trim().to_string();
        if pw.is_empty() {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut rng = rand::thread_rng();
            pw = (0..24).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
            let _ = std::fs::write(&pw_file, format!("{pw}\n"));
        }
        Game { hl: hl.clone(), name: name.into(), dir, data, rcon: Rcon::new(pw, 27015) }
    }

    pub fn state(&self) -> Value {
        let reply = match self.rcon.send("gm_status") {
            Ok(r) => r,
            Err(RconError::BadPassword) => {
                return json!({"online": false, "reason": "the game rejected the panel password - restart the map once"})
            }
            Err(RconError::Offline) => {
                return json!({"online": false, "reason": "Condition Zero isn't running, or no match is loaded."})
            }
        };
        let path = self.data.join("status.json");
        let lines: Vec<String> = if reply.contains("GM status written") && path.exists() {
            latin1(&std::fs::read(&path).unwrap_or_default()).lines().map(String::from).collect()
        } else {
            reply.lines().filter_map(|l| l.strip_prefix("GM ").map(String::from)).collect()
        };
        if lines.is_empty() {
            return json!({"online": false, "reason": "The game is running but the Game Mode Menu plugin didn't answer."});
        }
        let mut out = json!({"online": true, "modes": [], "settings": [], "addons": [], "addonsets": [],
                             "players": [], "presets": [], "events": [], "me": {}});
        for line in lines {
            let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(&line) else { continue };
            let kind = obj.remove("t").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
            match kind.as_str() {
                "state" => out["state"] = Value::Object(obj),
                "me" => out["me"] = Value::Object(obj),
                "mode" | "setting" | "addon" | "addonset" | "player" | "preset" | "event" => {
                    if let Some(list) = out[format!("{kind}s")].as_array_mut() {
                        list.push(Value::Object(obj));
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn command(&self, cmd: &str, args: &[String]) -> Result<Vec<String>, String> {
        if !ALLOWED.contains(&cmd) {
            return Err("unknown command".into());
        }
        let line = if cmd == "gm_announce" {
            // free text: keep only characters that can't end or extend the console command
            let text: String = args
                .join(" ")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || " _.,!?'()+-:#@&%/".contains(*c))
                .take(120)
                .collect();
            let text = text.trim().to_string();
            if text.is_empty() {
                return Err("empty message".into());
            }
            format!("gm_announce \"{text}\"")
        } else {
            if !args.iter().all(|a| safe_arg(a)) {
                return Err("invalid argument".into());
            }
            std::iter::once(cmd.to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(" ")
        };
        match self.rcon.send(&line) {
            Ok(reply) => Ok(reply.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()),
            Err(RconError::BadPassword) => Err("the game rejected the panel password - restart the map once".into()),
            Err(RconError::Offline) => Err("Condition Zero isn't running, or no match is loaded.".into()),
        }
    }

    /// The parts of panel.py's health report the page needs to draw its status chip.
    pub fn health(&self) -> Value {
        let mut checks = Map::new();
        let am = self.dir.join("addons/amxmodx");
        let lib = std::fs::read_to_string(self.dir.join("liblist.gam")).unwrap_or_default().to_lowercase();
        checks.insert("Game loads Metamod".into(), json!(lib.contains("addons\\metamod\\dlls\\metamod.dll")));
        checks.insert("AMX Mod X installed".into(), json!(am.join("dlls/amxmodx_mm.dll").exists()));
        checks.insert("Game Mode Menu installed".into(), json!(am.join("plugins/gamemodes.amxx").exists()));
        let list: Vec<Value> = checks.iter().map(|(k, v)| json!({"name": k, "ok": v, "detail": ""})).collect();
        let ok = list.iter().all(|c| c["ok"] == json!(true));
        json!({"ok": ok, "checks": list, "fixed": []})
    }
}
