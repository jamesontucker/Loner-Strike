//! One game folder (czero or cstrike) and the Game Mode Menu plugin inside it.
//! Live state over RCON, the commands the panel may send, and the health check. Maps and pictures are in maps.rs, the map store in
//! gamebanana.rs, playing with friends in sharing.rs.

use crate::gamebanana::MapStore;
use crate::rcon::{latin1, Rcon, RconError};
use crate::sharing::Sharing;
use crate::sys;
use rand::Rng;
use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Commands the panel may send. Everything else is refused.
const ALLOWED: &[&str] = &[
    "gm_mode", "gm_map", "gm_set", "gm_addon", "gm_addonset", "gm_restartmap", "gm_bots", "gm_match",
    "gm_player", "gm_nav", "gm_pause", "gm_team", "gm_preset", "gm_presetsave", "gm_presetdel", "gm_pos",
    "gm_crash", "gm_do", "gm_announce",
];
const METAMOD_DLL: &str = r"addons\metamod\dlls\metamod.dll";
const OFFLINE: &str = "No game is loaded. Start one from Play.";
const BAD_PASSWORD: &str = "The game turned down the panel's password. Restart the map once.";

/// Why a command failed: the request was wrong (HTTP 400) or the game didn't take it (503).
#[derive(Debug)]
pub enum CmdError {
    Bad(String),
    Game(String),
}

/// A value worked out at most once per `ttl`: callers arriving while it's being worked
/// out wait for that answer instead of asking again (the app window, every phone and
/// the join notifier all poll the same game).
pub struct Recent<T> {
    ttl: Duration,
    value: Mutex<Option<(Instant, u64, T)>>,
    /// held while the value is worked out
    turn: Mutex<()>,
    /// bumped by clear(), so an answer begun before a change isn't kept after it
    generation: AtomicU64,
}

impl<T: Clone> Recent<T> {
    pub const fn new(ttl: Duration) -> Self {
        Recent { ttl, value: Mutex::new(None), turn: Mutex::new(()), generation: AtomicU64::new(0) }
    }

    fn fresh(&self) -> Option<T> {
        let gen = self.generation.load(Ordering::SeqCst);
        let v = self.value.lock().unwrap_or_else(|e| e.into_inner());
        v.as_ref().filter(|(at, g, _)| *g == gen && at.elapsed() < self.ttl).map(|(_, _, t)| t.clone())
    }

    pub fn get_or(&self, f: impl FnOnce() -> T) -> T {
        if let Some(v) = self.fresh() {
            return v;
        }
        let _turn = self.turn.lock().unwrap_or_else(|e| e.into_inner());
        // someone else may have just worked it out while this one waited
        if let Some(v) = self.fresh() {
            return v;
        }
        let gen = self.generation.load(Ordering::SeqCst);
        let v = f();
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), gen, v.clone()));
        v
    }

    /// Forgets the value (something changed it): the next caller asks again.
    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Is the game open on this computer? (a process search: not on every state check)
fn game_open() -> bool {
    static OPEN: Recent<bool> = Recent::new(Duration::from_secs(5));
    OPEN.get_or(sys::game_running)
}

/// Splits lsrcon's reply from the status file sent after it in the same command.
const FILE_MARK: &str = "\n@@LS-STATUS-FILE@@\n";

pub struct Game {
    pub hl: PathBuf,
    pub name: String,
    pub dir: PathBuf,
    pub(crate) data: PathBuf,
    /// The app's own folder (records, caches)
    pub(crate) app: PathBuf,
    pub(crate) rcon: Arc<Rcon>,
    pub(crate) maps_cache: Arc<Mutex<Option<Vec<Value>>>>,
    pub store: Arc<MapStore>,
    pub sharing: Arc<Sharing>,
    /// The game's UDP port: 27015, or another for a second dedicated server
    pub port: u16,
    /// A dedicated server (not the game's own listen server)
    pub dedicated: bool,
    /// On another machine: how to reach it, and its game folder there (files the plugin
    /// writes, like status.json, are fetched from it)
    pub remote: Option<(crate::remote::RemoteInfo, String)>,
    /// The last state(), shared by everyone asking within a second
    status: Recent<Value>,
}

impl Game {
    pub fn new(hl: &Path, name: &str) -> Game {
        Game::on_port(hl, name, crate::sharing::GAME_PORT, false)
    }

    /// A game or dedicated server answering on `port`. They all share the app's
    /// one playit.gg agent.
    pub fn on_port(hl: &Path, name: &str, port: u16, dedicated: bool) -> Game {
        let app = sys::app_dir();
        // first run: bring over what the old Python panel knew
        if let Some(old) = sys::old_panel_dir() {
            for f in ["installed_maps.json", "map_sources.json"] {
                sys::adopt(&old.join(f), &app.join(f));
            }
        }
        let mut g = Game::with_app_dir(hl, name, &app);
        g.port = port;
        g.dedicated = dedicated;
        if dedicated {
            g.store = Arc::new(MapStore::new_server(hl, &g.dir, &app, g.maps_cache.clone()));
        }
        g.rcon = Arc::new(Rcon::new(g.rcon.password().to_string(), port));
        g.sharing = Sharing::global(&app, &g.data, port);
        g
    }

    pub fn with_app_dir(hl: &Path, name: &str, app: &Path) -> Game {
        let dir = hl.join(name);
        let data = dir.join("addons/amxmodx/data/gamemodes");
        let _ = std::fs::create_dir_all(&data);
        let _ = std::fs::create_dir_all(app.join("cache"));
        // The plugin reads the same file, so both sides share the RCON password
        let pw_file = data.join("panel_rcon.txt");
        let mut pw = std::fs::read_to_string(&pw_file).unwrap_or_default().trim().to_string();
        if pw.is_empty() {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut rng = rand::thread_rng();
            pw = (0..24).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
            let _ = std::fs::write(&pw_file, format!("{pw}\n"));
        }
        let rcon = Arc::new(Rcon::new(pw, 27015));
        let maps_cache = Arc::new(Mutex::new(None));
        let store = Arc::new(MapStore::new(hl, &dir, app, maps_cache.clone()));
        let sharing = Arc::new(Sharing::new(app, &data, crate::sharing::GAME_PORT));
        Game { hl: hl.to_path_buf(), name: name.into(), dir, data, app: app.to_path_buf(), rcon, maps_cache, store, sharing, port: crate::sharing::GAME_PORT, dedicated: false, remote: None, status: Recent::new(Duration::from_secs(1)) }
    }

    /// The game's live state, asked at most once a second however many pages poll it.
    pub fn state(&self) -> Value {
        self.status.get_or(|| self.state_now())
    }

    fn state_now(&self) -> Value {
        // a dedicated server on the game's port isn't the game: don't knock with the wrong password
        if !self.dedicated {
            if let Some(name) = crate::servers::running_on(self.port) {
                return json!({"online": false, "reason": format!("The server \"{name}\" is using your game's port ({}). Stop it to host from your game.", self.port)});
            }
        }
        // on another machine the plugin writes status.json there: it comes back with the
        // reply, in the same SSH command
        let remote_file = self.remote.as_ref().map(|(_, dir)| format!("{dir}/addons/amxmodx/data/gamemodes/status.json"));
        let sent = match &remote_file {
            Some(f) => self.rcon.send_then_cat("gm_status", f, FILE_MARK),
            None => self.rcon.send("gm_status").map(|r| (r, None)),
        };
        let (reply, file) = match sent {
            Ok(r) => r,
            Err(RconError::BadPassword) => return json!({"online": false, "reason": BAD_PASSWORD}),
            // game_open: the game is open on this computer but not hosting a map (main menu,
            // or playing on someone else's server)
            Err(RconError::Offline) => return json!({"online": false, "reason": OFFLINE, "game_open": !self.dedicated && game_open()}),
        };
        let path = self.data.join("status.json");
        if let (Some(bytes), true) = (file, reply.contains("GM status written")) {
            let _ = std::fs::write(&path, bytes);
        }
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

    pub fn command(&self, cmd: &str, args: &[String]) -> Result<Vec<String>, CmdError> {
        if !ALLOWED.contains(&cmd) {
            return Err(CmdError::Bad("unknown command".into()));
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
                return Err(CmdError::Bad("empty message".into()));
            }
            format!("gm_announce \"{text}\"")
        } else {
            if !args.iter().all(|a| crate::maps::safe_arg(a)) {
                return Err(CmdError::Bad("invalid argument".into()));
            }
            std::iter::once(cmd.to_string()).chain(args.iter().cloned()).collect::<Vec<_>>().join(" ")
        };
        // the next state() shows what the command did
        let sent = self.rcon.send(&line);
        self.status.clear();
        match sent {
            Ok(reply) if cmd == "gm_announce" => {
                let _ = reply;
                Ok(vec![])
            }
            Ok(reply) => Ok(reply.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()),
            Err(RconError::BadPassword) => Err(CmdError::Game(BAD_PASSWORD.into())),
            Err(RconError::Offline) => Err(CmdError::Game(OFFLINE.into())),
        }
    }

    /// Checks that Metamod, AMX Mod X and the menu are wired up, fixing what a
    /// Steam update or file verification tends to undo.
    pub fn health(&self) -> Value {
        health_check(&self.dir, true)
    }
}

/// The health check: {"ok", "checks": [{"name", "ok", "detail"}], "fixed": [...]}.
pub fn health_check(cz: &Path, repair: bool) -> Value {
    let mut checks: Vec<Value> = Vec::new();
    let mut fixed: Vec<&str> = Vec::new();
    let am = cz.join("addons").join("amxmodx");
    let mut check = |name: &str, ok: bool, detail: &str| checks.push(json!({"name": name, "ok": ok, "detail": detail}));

    let liblist = cz.join("liblist.gam");
    match sys::read_latin1(&liblist) {
        Ok(text) => {
            let re = Regex::new(r#"(?m)^gamedll\s+"([^"]*)""#).unwrap();
            let m = re.captures(&text).and_then(|c| c.get(1));
            match m {
                Some(m) if m.as_str().to_lowercase() == METAMOD_DLL.to_lowercase() => check("Game loads Metamod", true, ""),
                Some(m) if repair => {
                    let backup = cz.join(format!("liblist.gam.steam-{}", sys::stamp()));
                    let new = format!("{}{}{}", &text[..m.start()], METAMOD_DLL, &text[m.end()..]);
                    if sys::write_latin1(&backup, &text).is_ok() && sys::write_latin1(&liblist, &new).is_ok() {
                        fixed.push("Steam had reset liblist.gam, so the mods weren't loading. Fixed it. Restart Condition Zero.");
                        check("Game loads Metamod", true, "repaired");
                    } else {
                        check("Game loads Metamod", false, "liblist.gam doesn't point at Metamod");
                    }
                }
                _ => check("Game loads Metamod", false, "liblist.gam doesn't point at Metamod"),
            }
        }
        Err(_) => check("Game loads Metamod", false, "liblist.gam is missing"),
    }

    check("Metamod installed", cz.join("addons/metamod/dlls/metamod.dll").exists(), "");
    let mm_ini = cz.join("addons/metamod/plugins.ini");
    let mm_text = sys::read_latin1(&mm_ini).unwrap_or_default();
    if mm_text.to_lowercase().contains("amxmodx_mm.dll") {
        check("Metamod loads AMX Mod X", true, "");
    } else if repair && mm_ini.parent().map(|d| d.is_dir()).unwrap_or(false) {
        if std::fs::write(&mm_ini, "win32 addons\\amxmodx\\dlls\\amxmodx_mm.dll\r\n").is_ok() {
            fixed.push("Metamod's plugin list was missing AMX Mod X. Fixed it. Restart Condition Zero.");
            check("Metamod loads AMX Mod X", true, "repaired");
        } else {
            check("Metamod loads AMX Mod X", false, "");
        }
    } else {
        check("Metamod loads AMX Mod X", false, "");
    }
    check("AMX Mod X installed", am.join("dlls/amxmodx_mm.dll").exists(), "");
    check("Game Mode Menu installed", am.join("plugins/gamemodes.amxx").exists(), "");

    let pl_ini = am.join("configs/plugins.ini");
    let pl_text = sys::read_latin1(&pl_ini).unwrap_or_default();
    let enabled = Regex::new(r"(?m)^\s*gamemodes\.amxx").unwrap().is_match(&pl_text);
    if enabled {
        check("Game Mode Menu enabled", true, "");
    } else if repair && !pl_text.is_empty() {
        use std::io::Write;
        let res = std::fs::OpenOptions::new()
            .append(true)
            .open(&pl_ini)
            .and_then(|mut f| f.write_all(b"\r\ngamemodes.amxx\t\t; Game Mode Menu\r\n"));
        if res.is_ok() {
            fixed.push("The Game Mode Menu had been removed from plugins.ini. Added it back. Restart the map.");
            check("Game Mode Menu enabled", true, "repaired");
        } else {
            check("Game Mode Menu enabled", false, "");
        }
    } else {
        check("Game Mode Menu enabled", false, "");
    }

    let binds = sys::read_latin1(&cz.join("config.cfg")).unwrap_or_default();
    let menu = binds.contains("\"gamemenu\"");
    check(
        "F3 and F4 keys bound",
        menu && binds.contains("\"gamequick\""),
        if menu { "" } else { "type  bind F3 gamemenu  in the console" },
    );
    let ok = checks.iter().all(|c| c["ok"] == json!(true));
    json!({"ok": ok, "checks": checks, "fixed": fixed})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_shared() {
        use std::sync::atomic::AtomicUsize;
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let r: Arc<Recent<usize>> = Arc::new(Recent::new(Duration::from_millis(300)));
        let slow = || {
            std::thread::sleep(Duration::from_millis(50));
            CALLS.fetch_add(1, Ordering::SeqCst) + 1
        };
        // callers arriving together wait for the one answer
        let threads: Vec<_> = (0..8).map(|_| { let r = r.clone(); std::thread::spawn(move || r.get_or(slow)) }).collect();
        let got: Vec<usize> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(got.iter().all(|&v| v == 1), "{got:?}");
        assert_eq!(r.get_or(slow), 1, "still fresh");
        r.clear();
        assert_eq!(r.get_or(slow), 2, "asked again after a change");
        std::thread::sleep(Duration::from_millis(350));
        assert_eq!(r.get_or(slow), 3, "asked again once stale");
    }

    #[test]
    fn health_repairs() {
        let root = sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "health-").unwrap();
        let cz = root.join("czero");
        for d in ["addons/metamod/dlls", "addons/amxmodx/dlls", "addons/amxmodx/plugins", "addons/amxmodx/configs"] {
            std::fs::create_dir_all(cz.join(d)).unwrap();
        }
        std::fs::write(cz.join("liblist.gam"), "game \"CZ\"\r\ngamedll \"dlls\\czero.dll\"\r\n").unwrap();
        std::fs::write(cz.join("addons/metamod/dlls/metamod.dll"), "").unwrap();
        std::fs::write(cz.join("addons/amxmodx/dlls/amxmodx_mm.dll"), "").unwrap();
        std::fs::write(cz.join("addons/amxmodx/plugins/gamemodes.amxx"), "").unwrap();
        std::fs::write(cz.join("addons/amxmodx/configs/plugins.ini"), "admin.amxx\r\n").unwrap();
        std::fs::write(cz.join("config.cfg"), "bind \"F3\" \"gamemenu\"\nbind \"F4\" \"gamequick\"\n").unwrap();

        let h = health_check(&cz, false);
        assert_eq!(h["ok"], false);
        assert_eq!(h["fixed"].as_array().unwrap().len(), 0);

        let h = health_check(&cz, true);
        assert_eq!(h["ok"], true, "{h}");
        assert_eq!(h["fixed"].as_array().unwrap().len(), 3);
        let lib = std::fs::read_to_string(cz.join("liblist.gam")).unwrap();
        assert_eq!(lib, "game \"CZ\"\r\ngamedll \"addons\\metamod\\dlls\\metamod.dll\"\r\n");
        assert!(std::fs::read_dir(&cz).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("liblist.gam.steam-")));
        assert!(std::fs::read_to_string(cz.join("addons/metamod/plugins.ini")).unwrap().contains("amxmodx_mm.dll"));
        assert!(std::fs::read_to_string(cz.join("addons/amxmodx/configs/plugins.ini")).unwrap().contains("\r\ngamemodes.amxx"));

        let h = health_check(&cz, true);
        assert_eq!(h["ok"], true);
        assert_eq!(h["fixed"].as_array().unwrap().len(), 0);
        let names: Vec<&str> = h["checks"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 7);
        let _ = std::fs::remove_dir_all(&root);
    }
}
