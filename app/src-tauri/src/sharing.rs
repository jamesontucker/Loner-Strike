//! Play with friends: shares the game over a playit.gg tunnel or Tailscale.
//!
//! playit.gg gives this computer a public address and relays friends' game traffic
//! to it. The app runs playit's own program (playitd) only while sharing is on, as
//! its child: it is stopped with the app and never starts by itself. Setup is a
//! one-time approval on playit.gg; the agent key is kept in <app dir>/playit/secret.txt.
//!
//! Tailscale is the other choice; the app only reads its status for this computer's address.

use crate::rcon::Rcon;
use crate::sys;
use rand::Rng;
use serde_json::{json, Map, Value};
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const API: &str = "https://api.playit.gg";
const VERSION: &str = "playit 1.2.0";
pub const GAME_PORT: u16 = 27015;
const TUNNEL_NAME: &str = "cz-server";
const PLAYITD: &str = if cfg!(windows) { "playitd.exe" } else { "playitd" };

fn error_words(code: &str) -> Option<&'static str> {
    Some(match code {
        "RequiresPlayitPremium" | "PublicPortRequiresPlayitPremium" | "RegionRequiresPlayitPremium" => {
            "playit.gg wants a Premium account for this tunnel."
        }
        "RequiresVerifiedAccount" => "Verify your email on playit.gg (check your inbox), then press Start sharing again.",
        "AgentVersionTooOld" => "playit.gg says this copy of playit is too old. Update Loner-Strike to update it.",
        "AgentNotFound" => "playit.gg doesn't know this computer any more. Disconnect and set it up again.",
        "CodeExpired" => "The setup link expired. Start again.",
        "UserRejected" => "Setup was cancelled on playit.gg.",
        _ => return None,
    })
}

/// Server names: printable ASCII the game's config files and console take inside quotes: everything but
/// `"` (ends the name), `;` (starts another command) and `\\`. `|`, `[ ]`, `=` and `~` stay
/// for names like "Loner-Strike | Alone Together" or "[LS] Pub".
pub fn clean_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| (' '..='~').contains(c) && !"\";\\".contains(*c)).take(48).collect();
    kept.trim().to_string()
}

pub fn password_ok(p: &str) -> bool {
    p.len() <= 24 && p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

// ---- Tailscale -----------------------------------------------------------------

fn tailscale_status() -> Value {
    let paths = ["/Applications/Tailscale.app/Contents/MacOS/Tailscale", r"C:\Program Files\Tailscale\tailscale.exe"];
    let exe = sys::find_tool("tailscale").or_else(|| paths.iter().map(PathBuf::from).find(|p| p.is_file()));
    let Some(exe) = exe else { return json!({"installed": false}) };
    let mut c = sys::command(exe);
    c.args(["status", "--json"]);
    let parsed = sys::run_output(c, Duration::from_secs(4)).and_then(|(_, out, _)| serde_json::from_str::<Value>(&out).ok());
    let Some(d) = parsed else { return json!({"installed": true, "state": "unknown"}) };
    let me = &d["Self"];
    let ip = me["TailscaleIPs"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).find(|ip| ip.contains('.'));
    let name = me["DNSName"].as_str().unwrap_or("").trim_end_matches('.').to_string();
    json!({"installed": true, "state": d["BackendState"].as_str().unwrap_or(""),   // Running, Stopped, NeedsLogin...
           "ip": ip, "name": if name.is_empty() { Value::Null } else { json!(name) }})
}

// ---- playit.gg API -------------------------------------------------------------

fn call(path: &str, body: Value, key: Option<&str>) -> Result<Value, String> {
    let mut req = sys::http()
        .post(format!("{API}{path}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "loner-strike")
        .timeout(Duration::from_secs(15))
        .body(body.to_string());
    if let Some(k) = key {
        req = req.header("Authorization", format!("Agent-Key {k}"));
    }
    let unreachable = || "Couldn't reach playit.gg. Check your internet connection.".to_string();
    let r = req.send().map_err(|_| unreachable())?;
    let code = r.status();
    let text = r.text().map_err(|_| unreachable())?;
    let res: Value = match serde_json::from_str(if text.is_empty() { "{}" } else { &text }) {
        Ok(v) => v,
        Err(_) if !code.is_success() => return Err(format!("playit.gg answered with an error ({}).", code.as_u16())),
        Err(_) => return Err(unreachable()),
    };
    if res["status"] == "success" {
        return Ok(res["data"].clone());
    }
    let data = &res["data"];
    let code = match data {
        Value::Object(o) => o
            .get("message")
            .and_then(|v| v.as_str().filter(|s| !s.is_empty()))
            .or_else(|| o.get("type").and_then(|v| v.as_str().filter(|s| !s.is_empty())))
            .map(String::from)
            .unwrap_or_else(|| data.to_string()),
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        v => v.to_string(),
    };
    Err(error_words(&code).map(String::from).unwrap_or_else(|| format!("playit.gg said: {code}")))
}

// ---- state -------------------------------------------------------------------------

#[derive(Default)]
struct Run {
    phase: String, // off, starting, online, error
    error: String,
    address: Value,
    account: String,
    /// Bumped on every start and stop, so an old watcher knows to quit
    gen: u64,
    proc: Option<Child>,
}

/// The game being shared: dedicated servers each have their own port and settings.
struct Target {
    /// the game's data folder: friends.cfg (run by the plugin) and friends.json
    data: PathBuf,
    port: u16,
}

pub struct Sharing {
    target: Mutex<Target>,
    dir: PathBuf,
    secret_file: PathBuf,
    state_file: PathBuf,
    pid_file: PathBuf,
    log_file: PathBuf,
    run: Mutex<Run>,
    /// {"code", "url", "step", "error"} while setting up
    claim: Mutex<Option<Map<String, Value>>>,
    saved: Mutex<Map<String, Value>>,
    ts: Mutex<(Value, Option<Instant>)>,
}

/// playitd next to the app (bundled resources), or the old panel's copy.
fn find_playitd() -> Option<PathBuf> {
    let mut places = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            places.push(d.join(PLAYITD));
            places.push(d.join("resources").join(PLAYITD));
            places.push(d.join("../Resources").join(PLAYITD)); // inside a Mac .app
            places.push(d.join("../Resources/resources").join(PLAYITD));
        }
    }
    if let Some(old) = sys::old_panel_dir() {
        places.push(old.join("tools/playit").join(PLAYITD));
    }
    places.into_iter().find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    p.is_file()
}

impl Sharing {
    pub fn new(app_dir: &Path, data: &Path, port: u16) -> Sharing {
        let dir = app_dir.join("playit");
        let _ = std::fs::create_dir_all(&dir);
        let sh = Sharing {
            target: Mutex::new(Target { data: data.to_path_buf(), port }),
            secret_file: dir.join("secret.txt"),
            state_file: dir.join("sharing.json"),
            pid_file: dir.join("playitd.pid"),
            log_file: dir.join("playitd.log"),
            dir,
            run: Mutex::new(Run { phase: "off".into(), address: Value::Null, ..Default::default() }),
            claim: Mutex::new(None),
            saved: Mutex::new(Map::new()),
            ts: Mutex::new((Value::Null, None)),
        };
        *sh.saved.lock().unwrap() = sh.load();
        sh
    }

    /// The app's one playit.gg agent, shared by every game and server it controls.
    pub fn global(app_dir: &Path, data: &Path, port: u16) -> Arc<Sharing> {
        static ONE: std::sync::OnceLock<Arc<Sharing>> = std::sync::OnceLock::new();
        ONE.get_or_init(|| {
            let sh = Arc::new(Sharing::new(app_dir, data, port));
            // only here, once: a Sharing is built with every Game, and the pid file
            // belongs to the live agent once sharing is on
            sh.stop_leftover();
            sh.adopt_old_panel();
            sh
        })
        .clone()
    }

    /// Points sharing at another game or server. A tunnel for another port stops.
    pub fn retarget(&self, data: &Path, port: u16) {
        let old = {
            let mut t = self.target.lock().unwrap();
            let old = t.port;
            *t = Target { data: data.to_path_buf(), port };
            old
        };
        if old != port && self.run.lock().unwrap().phase != "off" {
            self.stop();
        }
    }

    /// status() as one game or server sees it: its own name, password and port, and
    /// shared_port, the port the playit tunnel is for (another game's when they differ).
    pub fn status_for(&self, data: &Path, port: u16) -> Value {
        let mut v = self.status();
        let (name, password) = self.server_settings(data);
        v["name"] = name;
        v["password"] = password;
        v["shared_port"] = json!(self.port());
        v["port"] = json!(port);
        v
    }

    pub fn port(&self) -> u16 {
        self.target.lock().unwrap().port
    }

    /// The server name and password: per game (friends.json), or what the
    /// Friends tab saved before servers had their own.
    fn server_settings(&self, data: &Path) -> (Value, Value) {
        if let Some(v) = std::fs::read_to_string(data.join("friends.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            return (v["name"].clone(), v["password"].clone());
        }
        let saved = self.saved.lock().unwrap();
        (saved.get("name").cloned().unwrap_or(json!("")), saved.get("password").cloned().unwrap_or(json!("")))
    }

    /// The playit tunnel's key in sharing.json: one tunnel per game port.
    fn tunnel_key(&self) -> String {
        match self.port() {
            GAME_PORT => "tunnel_id".into(),
            p => format!("tunnel_id_{p}"),
        }
    }

    fn tunnel_name(&self) -> String {
        match self.port() {
            GAME_PORT => TUNNEL_NAME.into(),
            p => format!("{TUNNEL_NAME}-{p}"),
        }
    }

    /// Brings over the old panel's playit key and settings the first time.
    pub fn adopt_old_panel(&self) {
        if let Some(old) = sys::old_panel_dir() {
            let o = old.join("tools/playit");
            sys::adopt(&o.join("secret.txt"), &self.secret_file);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&self.secret_file, std::fs::Permissions::from_mode(0o600));
            }
            if !self.state_file.exists() {
                sys::adopt(&o.join("sharing.json"), &self.state_file);
                *self.saved.lock().unwrap() = self.load();
            }
        }
    }

    fn binary(&self) -> Option<PathBuf> {
        find_playitd()
    }

    // ---- saved state ----------------------------------------------------
    fn load(&self) -> Map<String, Value> {
        crate::sys::read_json_map(&self.state_file)
    }

    fn save(&self, saved: &Map<String, Value>) {
        let _ = crate::sys::write_atomic(&self.state_file, serde_json::to_string_pretty(saved).unwrap_or_default().as_bytes());
    }

    fn edit_saved(&self, f: impl FnOnce(&mut Map<String, Value>)) {
        let mut s = self.saved.lock().unwrap();
        f(&mut s);
        self.save(&s);
    }

    fn key(&self) -> Option<String> {
        std::fs::read_to_string(&self.secret_file).ok().map(|t| t.trim().to_string()).filter(|k| !k.is_empty())
    }

    pub fn status(&self) -> Value {
        let saved = self.saved.lock().unwrap().clone();
        let data = self.target.lock().unwrap().data.clone();
        let (name, password) = self.server_settings(&data);
        // the slow parts (a tailscale subprocess, file checks) before taking `run`,
        // which starting and stopping the tunnel wait on
        let (installed, connected, tailscale, port) = (self.binary().is_some(), self.key().is_some(), self.tailscale(), self.port());
        let claim = self.claim.lock().unwrap().clone();
        let run = self.run.lock().unwrap();
        json!({
            "installed": installed,
            "connected": connected,
            "claim": claim,
            "phase": run.phase,
            "error": run.error,
            "address": run.address,
            "account": run.account,
            "name": name,
            "password": password,
            "method": saved.get("method").cloned().unwrap_or(json!("playit")),
            "tailscale": tailscale,
            "port": port,
        })
    }

    fn tailscale(&self) -> Value {
        let mut ts = self.ts.lock().unwrap();
        if ts.1.map(|t| t.elapsed() > Duration::from_secs(5)).unwrap_or(true) {
            // the page asks every 2 seconds
            *ts = (tailscale_status(), Some(Instant::now()));
        }
        ts.0.clone()
    }

    pub fn set_method(&self, method: &str) -> Result<(), String> {
        if method != "playit" && method != "tailscale" {
            return Err("unknown method".into());
        }
        if method == "tailscale" {
            self.stop(); // one way at a time
        }
        self.edit_saved(|s| {
            s.insert("method".into(), json!(method));
        });
        Ok(())
    }

    // ---- one-time setup -------------------------------------------------
    pub fn connect(self: &Arc<Self>) {
        let code = {
            let mut claim = self.claim.lock().unwrap();
            if let Some(c) = claim.as_ref() {
                if c["step"] == "waiting" || c["step"] == "approve" {
                    return;
                }
            }
            let code = format!("{:010x}", rand::thread_rng().gen::<u64>() & 0xff_ffff_ffff);
            let mut c = Map::new();
            c.insert("code".into(), json!(code));
            c.insert("url".into(), json!(format!("https://playit.gg/claim/{code}")));
            c.insert("step".into(), json!("waiting"));
            c.insert("error".into(), json!(""));
            *claim = Some(c);
            code
        };
        let me = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = me.claim_flow(&code) {
                me.with_claim(&code, |c| {
                    c.insert("step".into(), json!("failed"));
                    c.insert("error".into(), json!(e));
                });
            }
        });
    }

    pub fn cancel(&self) {
        *self.claim.lock().unwrap() = None;
    }

    /// Runs f on the current claim if it's still the one with this code. False if it's gone.
    fn with_claim(&self, code: &str, f: impl FnOnce(&mut Map<String, Value>)) -> bool {
        let mut claim = self.claim.lock().unwrap();
        match claim.as_mut() {
            Some(c) if c["code"] == code => {
                f(c);
                true
            }
            _ => false,
        }
    }

    fn claim_flow(&self, code: &str) -> Result<(), String> {
        let give_up = Instant::now() + Duration::from_secs(15 * 60);
        loop {
            if !self.with_claim(code, |_| {}) {
                return Ok(());
            }
            if Instant::now() > give_up {
                return Err("The setup link expired. Start again.".into());
            }
            let step = call("/claim/setup", json!({"code": code, "agent_type": "self-managed", "version": VERSION}), None)?;
            let step = step.as_str().unwrap_or("");
            if step == "UserAccepted" {
                break;
            }
            if step == "UserRejected" {
                return Err(error_words("UserRejected").unwrap().into());
            }
            let next = if step == "WaitingForUser" { "approve" } else { "waiting" };
            self.with_claim(code, |c| {
                c.insert("step".into(), json!(next));
            });
            std::thread::sleep(Duration::from_millis(1500));
        }
        let key = loop {
            if !self.with_claim(code, |_| {}) {
                return Ok(());
            }
            if Instant::now() > give_up {
                return Err("The setup link expired. Start again.".into());
            }
            match call("/claim/exchange", json!({"code": code}), None) {
                Ok(d) => break d["secret_key"].as_str().map(String::from).ok_or("playit.gg sent no key.")?,
                Err(e) if e.contains("NotAccepted") || e.contains("NotSetup") => std::thread::sleep(Duration::from_secs(2)),
                Err(e) => return Err(e),
            }
        };
        write_secret(&self.secret_file, &key).map_err(|e| e.to_string())?;
        self.edit_saved(|s| {
            s.retain(|k, _| !k.starts_with("tunnel_id"));
        });
        let mut claim = self.claim.lock().unwrap();
        if claim.as_ref().map(|c| c["code"] == code).unwrap_or(false) {
            *claim = None; // done
        }
        Ok(())
    }

    pub fn disconnect(&self) {
        self.stop();
        let _ = std::fs::remove_file(&self.secret_file);
        self.edit_saved(|s| {
            s.retain(|k, _| !k.starts_with("tunnel_id"));
        });
        self.run.lock().unwrap().account.clear();
    }

    // ---- server name and password ----------------------------------------
    /// The server name and password of one game or server (its data folder and RCON).
    pub fn set_server(&self, rcon: &Rcon, data: &Path, name: &str, password: &str) -> Result<(), String> {
        let name = clean_name(name);
        let password = password.trim().to_string();
        if !password_ok(&password) {
            return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
        }
        let _ = std::fs::write(data.join("friends.json"), json!({"name": name, "password": password}).to_string());
        let mut lines = Vec::new();
        if !name.is_empty() {
            lines.push(format!("hostname \"{name}\""));
        }
        lines.push(format!("sv_password \"{password}\""));
        // a dedicated server's "bots play on an empty server" line stays (servers.rs write_join)
        let kept: Vec<String> = std::fs::read_to_string(data.join("friends.cfg"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.trim_start().starts_with("bot_join_after_player"))
            .map(String::from)
            .collect();
        // The plugin runs this after every map change (see task_reapply_saved)
        let mut file = lines.clone();
        file.extend(kept);
        std::fs::write(data.join("friends.cfg"), format!("// Written by Loner-Strike's Friends tab\n{}\n", file.join("\n"))).map_err(|e| e.to_string())?;
        for line in &lines {
            if rcon.send(line).is_err() {
                break; // applied when the game next loads a map
            }
        }
        // a dedicated server's name is also its name in the server list (and what Server
        // settings writes back to friends.cfg), so it's kept there too
        if !name.is_empty() {
            crate::servers::rename_by_data(data, &name);
        }
        Ok(())
    }

    // ---- sharing --------------------------------------------------------
    pub fn start(self: &Arc<Self>) -> Result<(), String> {
        let key = self.key().ok_or("Set up playit.gg first.")?;
        let Some(binary) = self.binary() else {
            return Err("playit isn't installed with the app.".into());
        };
        let gen = {
            let mut run = self.run.lock().unwrap();
            if run.phase == "starting" || run.phase == "online" {
                return Ok(());
            }
            run.phase = "starting".into();
            run.error.clear();
            run.address = Value::Null;
            run.gen += 1;
            run.gen
        };
        let me = self.clone();
        std::thread::spawn(move || me.run_tunnel(&key, &binary, gen));
        Ok(())
    }

    pub fn stop(&self) {
        let proc = {
            let mut run = self.run.lock().unwrap();
            run.phase = "off".into();
            run.error.clear();
            run.address = Value::Null;
            run.gen += 1;
            run.proc.take()
        };
        if let Some(p) = proc {
            sys::stop_child(p, Duration::from_secs(5));
        }
        let _ = std::fs::remove_file(&self.pid_file);
    }

    fn fail(&self, gen: u64, message: String) {
        if self.run.lock().unwrap().gen != gen {
            return; // sharing was stopped or restarted meanwhile
        }
        self.stop();
        let mut run = self.run.lock().unwrap();
        run.phase = "error".into();
        run.error = message;
    }

    fn run_tunnel(&self, key: &str, binary: &Path, gen: u64) {
        let setup = (|| -> Result<(), String> {
            let data = call("/v1/agents/rundata", json!({}), Some(key))?;
            self.run.lock().unwrap().account = data["permissions"]["account_status"].as_str().unwrap_or("").to_string();
            if self.find_tunnel(key)?.is_none() {
                let created = call(
                    "/v1/tunnels/create",
                    json!({
                        "ports": {"type": "custom-udp", "details": 1},
                        "origin": {"type": "agent", "data": {"agent_id": data["agent_id"], "config": {"fields": [
                            {"name": "local_ip", "value": "127.0.0.1"},
                            {"name": "local_port", "value": self.port().to_string()},
                        ]}}},
                        "enabled": true, "alloc": null, "name": self.tunnel_name(), "firewall_id": null,
                    }),
                    Some(key),
                )?;
                let id = if created.is_object() { created["id"].clone() } else { created };
                let key = self.tunnel_key();
                self.edit_saved(|s| {
                    s.insert(key, id);
                });
            }
            Ok(())
        })();
        if let Err(e) = setup {
            return self.fail(gen, e);
        }

        let child_id = {
            let mut run = self.run.lock().unwrap();
            if run.phase != "starting" || run.gen != gen {
                return;
            }
            let mut c = sys::command(binary);
            c.arg("--secret-path")
                .arg(&self.secret_file)
                .arg("--socket-path")
                .arg(self.dir.join("playitd.sock"))
                .arg("--log-path")
                .arg(&self.log_file)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            match c.spawn() {
                Ok(child) => {
                    sys::tie_to_app(&child);
                    let id = child.id();
                    run.proc = Some(child);
                    id
                }
                Err(e) => {
                    drop(run);
                    return self.fail(gen, format!("Couldn't start playit ({e})."));
                }
            }
        };
        let _ = std::fs::write(&self.pid_file, child_id.to_string());

        // Watch the tunnel until sharing stops: quickly until it has an address, then now and then
        loop {
            {
                let mut run = self.run.lock().unwrap();
                if run.gen != gen {
                    return;
                }
                let exited = run.proc.as_mut().map(|p| !matches!(p.try_wait(), Ok(None))).unwrap_or(true);
                if exited {
                    drop(run);
                    return self.fail(gen, format!("playit stopped unexpectedly. Details are in {}.", self.log_file.display()));
                }
            }
            match self.find_tunnel(key) {
                Ok(Some(t)) => self.read_tunnel(&t, gen),
                Ok(None) => {}
                Err(e) => {
                    let mut run = self.run.lock().unwrap();
                    if run.gen == gen {
                        run.error = e;
                    }
                }
            }
            let wait = if self.run.lock().unwrap().phase == "starting" { 3 } else { 20 };
            let until = Instant::now() + Duration::from_secs(wait);
            while Instant::now() < until {
                if self.run.lock().unwrap().gen != gen {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }

    fn find_tunnel(&self, key: &str) -> Result<Option<Value>, String> {
        let list = call("/v1/tunnels/list", json!({}), Some(key))?;
        let key = self.tunnel_key();
        let name = self.tunnel_name();
        let want = self.saved.lock().unwrap().get(&key).cloned().unwrap_or(Value::Null);
        for t in list["tunnels"].as_array().into_iter().flatten() {
            if t["id"] == want || (want.is_null() && t["name"] == name.as_str()) {
                if t["id"] != want {
                    let id = t["id"].clone();
                    self.edit_saved(|s| {
                        s.insert(key, id);
                    });
                }
                return Ok(Some(t.clone()));
            }
        }
        Ok(None)
    }

    fn read_tunnel(&self, t: &Value, gen: u64) {
        let (address, reasons) = tunnel_address(t);
        let mut run = self.run.lock().unwrap();
        if run.gen != gen {
            return;
        }
        if let Some(a) = address {
            run.address = a;
        }
        if !run.address.is_null() && reasons.is_empty() {
            run.phase = "online".into();
            run.error.clear();
        } else if let Some(r) = reasons.first() {
            run.error = match r.as_str() {
                "PublicAllocationPending" => "playit.gg is still picking an address...".into(),
                "AgentOverLimit" => "Your playit.gg account has too many agents. Remove an old one on playit.gg.".into(),
                "AgentDisabled" => "This computer is turned off on playit.gg. Turn it back on there.".into(),
                "TunnelDisabled" => "The tunnel is turned off on playit.gg. Turn it back on there.".into(),
                other => format!("playit.gg: {other}"),
            };
        }
    }

    /// Stops a playitd the app started before it was closed without stopping it.
    fn stop_leftover(&self) {
        let Ok(text) = std::fs::read_to_string(&self.pid_file) else { return };
        #[cfg(unix)]
        if let (Ok(pid), Some(bin)) = (text.trim().parse::<i32>(), self.binary()) {
            let mut c = sys::command("ps");
            c.args(["-p", &pid.to_string(), "-o", "command="]);
            if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
                if out.contains(&*bin.to_string_lossy()) {
                    unsafe { libc::kill(pid, libc::SIGTERM) };
                }
            }
        }
        #[cfg(windows)]
        if let Ok(pid) = text.trim().parse::<u32>() {
            let mut c = sys::command("tasklist");
            c.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
            if let Some((_, out, _)) = sys::run_output(c, Duration::from_secs(3)) {
                if out.to_lowercase().contains("playitd") {
                    let mut k = sys::command("taskkill");
                    k.args(["/PID", &pid.to_string(), "/F"]);
                    let _ = sys::run_output(k, Duration::from_secs(3));
                }
            }
        }
        let _ = text;
        let _ = std::fs::remove_file(&self.pid_file);
    }
}

impl Drop for Sharing {
    fn drop(&mut self) {
        if let Some(p) = self.run.get_mut().ok().and_then(|r| r.proc.take()) {
            sys::stop_child(p, Duration::from_secs(2));
        }
    }
}

fn write_secret(path: &Path, key: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(path)?;
    writeln!(f, "{key}")
}

/// ({"host", "ip", "port"} if the tunnel has an address, offline reasons).
pub fn tunnel_address(t: &Value) -> (Option<Value>, Vec<String>) {
    let (mut host, mut port, mut ip): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    for a in t["connect_addresses"].as_array().into_iter().flatten() {
        let v = &a["value"];
        match a["type"].as_str() {
            Some("addr4") if ip.is_none() => {
                if let Some((i, p)) = v["address"].as_str().and_then(|s| s.rsplit_once(':')) {
                    ip = Some(i.into());
                    port = Some(p.into());
                }
            }
            Some("ip4") if ip.is_none() => {
                ip = v["address"].as_str().map(String::from);
                port = Some(v["default_port"].to_string());
            }
            Some("auto") | Some("domain") if host.is_none() => host = v["address"].as_str().map(String::from),
            _ => {}
        }
    }
    if let Some((h, hp)) = host.clone().as_deref().and_then(|h| h.rsplit_once(':')) {
        host = Some(h.to_string());
        if port.is_none() {
            port = Some(hp.to_string());
        }
    }
    if let (Some(h), None) = (&host, &ip) {
        ip = (h.as_str(), 0).to_socket_addrs().ok().and_then(|mut a| a.find(|x| x.is_ipv4())).map(|a| a.ip().to_string());
    }
    let reasons: Vec<String> = t["offline_reasons"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(String::from)).collect();
    let addr = match (port.and_then(|p| p.trim_matches('"').parse::<u16>().ok()), ip.is_some() || host.is_some()) {
        (Some(p), true) => Some(json!({"host": host, "ip": ip, "port": p})),
        _ => None,
    };
    (addr, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_passwords() {
        assert_eq!(clean_name("  My \"server\"; quit  "), "My server quit");
        assert_eq!(clean_name(&"x".repeat(80)).len(), 48);
        assert!(password_ok(""));
        assert!(password_ok("abc-DEF_123"));
        assert!(!password_ok("has space"));
        assert!(!password_ok("quote\""));
        assert!(!password_ok(&"a".repeat(25)));
    }

    #[test]
    fn tunnel_addresses() {
        let t = json!({"connect_addresses": [
            {"type": "domain", "value": {"address": "abc.gl.joinmc.link"}},
            {"type": "addr4", "value": {"address": "1.2.3.4:5678"}},
        ], "offline_reasons": []});
        let (a, r) = tunnel_address(&t);
        assert_eq!(a.unwrap(), json!({"host": "abc.gl.joinmc.link", "ip": "1.2.3.4", "port": 5678}));
        assert!(r.is_empty());
        let t = json!({"connect_addresses": [{"type": "ip4", "value": {"address": "5.6.7.8", "default_port": 27015}}],
                       "offline_reasons": ["PublicAllocationPending"]});
        let (a, r) = tunnel_address(&t);
        assert_eq!(a.unwrap()["port"], 27015);
        assert_eq!(r, vec!["PublicAllocationPending".to_string()]);
        assert!(tunnel_address(&json!({})).0.is_none());
    }
}

#[cfg(test)]
mod name_tests {
    #[test]
    fn keeps_server_name_punctuation() {
        assert_eq!(super::clean_name("Loner-Strike | Alone Together"), "Loner-Strike | Alone Together");
        assert_eq!(super::clean_name("[LS] =Pub= ~24/7~"), "[LS] =Pub= ~24/7~");
        assert_eq!(super::clean_name("bad\";quit\\é"), "badquit");
    }
}
