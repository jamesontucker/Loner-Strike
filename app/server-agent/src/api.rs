//! The HTTP side: every request through `handle` (the guard, then what it asked for), and
//! the listener on 127.0.0.1 that Caddy or cloudflared passes https://<domain>/control/…
//! to. Paths, request bodies and answers are the owner's app's own (control/mod.rs and the
//! panel's routes), so a helper's app talks to this the same way, at another address.

use crate::game::{self, GameIo, RconError};
use crate::rules;
use crate::store::{self, Files, Member};
use crate::{common, Config, Paths, Role, VERSION};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The biggest request body taken
const MAX_BODY: usize = 64 * 1024;
/// Tries at accepting invites per address, per window (no cap for everyone together: a
/// flood from many addresses would then keep everyone out; invite secrets are 128-bit)
const TRIES_MAX: u32 = 10;
/// Wrong keys from one address before it waits
const BAD_KEYS_MAX: u32 = 30;
const TRIES_WINDOW: u64 = 600;
/// Addresses counted at once
const MAX_TRACKED: usize = 10_000;

pub const OWNER_APP: &str = "That's done from the owner's Loner-Strike for now. Ask the owner.";
const NOT_SET_UP: &str = "That server isn't there any more.";
const GONE: &str = "The owner took your access away, or it never worked. Ask them for a new invite.";

/// What a helper's app may know about the server (the app's control::HELPER_VIEW)
const HELPER_VIEW: &[&str] = &["name", "game", "port", "visibility", "map", "maxplayers", "ready", "loner", "installed", "connect", "bots_when_empty", "bots_when_empty_set", "pending_restart"];

pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    /// where it came from (behind Caddy or cloudflared: the address they passed on)
    pub peer: Option<IpAddr>,
}

#[derive(Debug)]
pub struct Resp {
    pub status: u16,
    pub body: Value,
}

fn ok(v: Value) -> Resp {
    Resp { status: 200, body: v }
}

/// A refusal or a failure, said the way the app says it (200 with an error, as the page expects)
fn err(e: impl Into<String>) -> Resp {
    Resp { status: 200, body: json!({"error": e.into()}) }
}

fn refuse(status: u16, why: &str) -> Resp {
    Resp { status, body: json!({"error": why}) }
}

#[derive(Default)]
struct Tries {
    /// per address: (count, window start)
    by_ip: HashMap<IpAddr, (u32, u64)>,
    all: (u32, u64),
}

pub struct Ctx {
    pub paths: Paths,
    /// the game, for tests (otherwise the one config.json describes)
    pub fake: Option<Arc<dyn GameIo>>,
    redeems: Mutex<Tries>,
    bad_keys: Mutex<Tries>,
    /// the last state, shared by everyone asking within a second
    state: Mutex<Option<(Instant, Value)>>,
}

impl Ctx {
    pub fn new(paths: Paths, fake: Option<Arc<dyn GameIo>>) -> Ctx {
        Ctx { paths, fake, redeems: Mutex::default(), bad_keys: Mutex::default(), state: Mutex::new(None) }
    }

    fn io(&self, c: &Config) -> Arc<dyn GameIo> {
        match &self.fake {
            Some(f) => f.clone(),
            None => Arc::new(game::Real { game_dir: self.paths.server_dir().join(&c.game), port: c.port, unit: format!("lonerstrike-{}", c.id) }),
        }
    }
}

/// Counts one more try from `ip` (and in all, with `all_max`); whether that's too many.
fn too_many(t: &Mutex<Tries>, ip: Option<IpAddr>, max: u32, all_max: Option<u32>) -> bool {
    let now = store::now();
    let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
    t.by_ip.retain(|_, (_, start)| now <= *start + TRIES_WINDOW);
    if now > t.all.1 + TRIES_WINDOW {
        t.all = (0, now);
    }
    t.all.0 += 1;
    let over_all = all_max.is_some_and(|m| t.all.0 > m);
    let Some(ip) = ip else { return over_all };
    // (a flood from very many addresses: new ones wait, the table doesn't grow without end)
    if t.by_ip.len() >= MAX_TRACKED && !t.by_ip.contains_key(&ip) {
        return true;
    }
    let e = t.by_ip.entry(ip).or_insert((0, now));
    e.0 += 1;
    e.0 > max || over_all
}

/// Whether `ip` has already had too many wrong keys (without counting this one).
fn locked_out(t: &Mutex<Tries>, ip: Option<IpAddr>) -> bool {
    let now = store::now();
    let t = t.lock().unwrap_or_else(|e| e.into_inner());
    ip.and_then(|ip| t.by_ip.get(&ip)).is_some_and(|(n, start)| *n >= BAD_KEYS_MAX && now <= start + TRIES_WINDOW)
}

/// The path as the app knows it: /control/api/… (Caddy's handle_path takes /control off,
/// a Cloudflare Tunnel doesn't) becomes /api/…. None: not a path anything here answers.
fn clean_path(p: &str) -> Option<String> {
    let p = p.split(['?', '#']).next().unwrap_or("");
    let p = p.strip_prefix("/control").filter(|r| r.is_empty() || r.starts_with('/')).unwrap_or(p);
    let ok = p.starts_with("/api/") && p.len() <= 256 && p.bytes().all(|c| c.is_ascii_alphanumeric() || b"/_-.".contains(&c)) && !p.split('/').any(|s| s == ".." || s == ".");
    ok.then(|| p.to_string())
}

fn went(r: &Resp) -> bool {
    (200..300).contains(&r.status) && r.body.get("error").map_or(true, |e| e.is_null())
}

/// Requests that only read or look something up, though sent as POST: never in the log
fn only_looks(path: &str) -> bool {
    path.ends_with("/console/hints") || path == "/api/control/join"
}

/// Everything from the listener goes through here (see the notes at the top of main.rs).
pub fn handle(ctx: &Ctx, req: Req, count_redeem: bool) -> Resp {
    let method = req.method.to_ascii_uppercase();
    if method != "GET" && method != "POST" {
        return refuse(405, "Not allowed.");
    }
    let Some(path) = clean_path(&req.path) else { return refuse(404, "Not found.") };
    if req.body.len() > MAX_BODY {
        return refuse(413, "That's too big.");
    }
    let config = match Config::read(&ctx.paths) {
        Ok(c) => c,
        Err(e) => return refuse(503, &e),
    };
    let files = Files::new(&ctx.paths);
    // whether it's there and which version (the owner's app checks its address with this)
    if method == "GET" && path == "/api/control/ping" {
        return ok(json!({"ok": true, "version": VERSION}));
    }
    if method == "POST" && path == "/api/control/redeem" {
        if count_redeem && too_many(&ctx.redeems, req.peer, TRIES_MAX, None) {
            return refuse(429, "Too many tries. Wait a few minutes.");
        }
        return redeem(&files, &config, &req.body);
    }
    if locked_out(&ctx.bad_keys, req.peer) {
        return refuse(429, "Too many tries. Wait a few minutes.");
    }
    // 1. their key
    let key = req.headers.get("authorization").and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("").trim().to_string();
    let m = match files.member_for(&key) {
        Ok(Some(m)) => m,
        Ok(None) => {
            too_many(&ctx.bad_keys, req.peer, BAD_KEYS_MAX, None);
            return refuse(401, GONE);
        }
        // the file busy or damaged: not a reason to say access is gone
        Err(_) => return refuse(503, "The server is busy. Try again in a moment."),
    };
    // 2. the same version
    let theirs = req.headers.get("x-ls-version").map(String::as_str).unwrap_or("");
    if theirs != VERSION {
        let you = if theirs.is_empty() { "an older one" } else { theirs };
        return Resp { status: 426, body: json!({"error": format!("Both of you need the same Loner-Strike version to share a server. The server has {VERSION}, you have {you}."), "version": VERSION}) };
    }
    // 3. pinned to this server
    if m.server != config.id {
        return refuse(403, NOT_SET_UP);
    }
    let body: Option<Value> = if req.body.is_empty() { None } else { serde_json::from_slice(&req.body).ok() };
    if method == "POST" && !req.body.is_empty() && body.is_none() {
        return refuse(400, "That isn't something Loner-Strike sends.");
    }
    // 4. their role (the app's own rules; mode settings aren't known here, so never)
    let verdict = rules::check(&rules::Ask { role: m.role, method: &method, path: &path, server: &config.id, body: body.as_ref() }, &|_, _| false);
    let body = match verdict {
        rules::Verdict::Deny(why) => return refuse(403, &why),
        rules::Verdict::Allow(Some(v)) => Some(v),
        rules::Verdict::Allow(None) => body,
    };
    let io = ctx.io(&config);
    if m.role == Role::Moderator && (path == "/api/admins/ban" || path == "/api/community/gag") {
        let who = body.as_ref().and_then(|b| b["id"].as_str()).unwrap_or("").trim().to_uppercase();
        if outranks(&files, io.as_ref(), &who) {
            return refuse(403, common::OUTRANKED);
        }
    }
    // 5. what they asked for
    let res = act(ctx, &config, &files, io.as_ref(), &m, &method, &path, body.as_ref());
    // 6. a change that went through goes in the log
    if method == "POST" && went(&res) && !only_looks(&path) {
        files.log_change(&m, &path, body.as_ref());
    }
    files.seen(&m.id);
    res
}

/// Whether a Moderator may not ban or mute this SteamID (see game::outranks_moderator).
fn outranks(files: &Files, io: &dyn GameIo, steamid: &str) -> bool {
    let helpers: Vec<String> = match files.read() {
        Ok(s) => s.members.into_iter().map(|m| m.steam).filter(|s| !s.is_empty()).collect(),
        Err(_) => return true,
    };
    game::outranks_moderator(&io.game_dir(), steamid, &helpers)
}

/// A friend's app accepting an invite (no key yet).
fn redeem(files: &Files, config: &Config, body: &[u8]) -> Resp {
    let Ok(b) = serde_json::from_slice::<Value>(body) else { return refuse(400, "That isn't something Loner-Strike sends.") };
    let version = b["version"].as_str().unwrap_or("");
    if version != VERSION {
        return err(format!("Both of you need the same Loner-Strike version to share a server. The server has {VERSION}."));
    }
    let invite = b["invite"].as_str().unwrap_or("");
    let name = b["name"].as_str().unwrap_or("");
    match files.redeem(invite, name) {
        Ok((m, key)) => {
            files.log_change(&m, "/api/control/redeem", Some(&json!({"invited_by": m.invited_by})));
            ok(json!({"ok": true, "key": key, "member": m.id, "role": m.role, "server": {"id": config.id, "name": config.name(), "game": config.game}}))
        }
        Err(e) => err(e),
    }
}

/// The server as a helper sees it: what the owner's app wrote (only the keys helpers get),
/// and whether it's running now.
fn server_view(config: &Config, io: &dyn GameIo) -> Value {
    let mut out = serde_json::Map::new();
    for k in HELPER_VIEW {
        if let Some(v) = config.view.get(*k) {
            out.insert((*k).to_string(), v.clone());
        }
    }
    let (on, secs) = io.active();
    out.insert("id".into(), json!(config.id));
    out.insert("game".into(), json!(config.game));
    out.insert("port".into(), json!(config.port));
    out.insert("ready".into(), json!(true));
    out.insert("state".into(), json!(if on { "running" } else { "stopped" }));
    out.insert("uptime".into(), json!(secs));
    out.insert("error".into(), Value::Null);
    Value::Object(out)
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    }
}

/// One console command for the game; the reply with the secrets blanked.
fn tell(io: &dyn GameIo, line: &str) -> Result<String, RconError> {
    io.rcon(line).map(|r| game::redact(&r, &game::secrets(&io.game_dir())))
}

/// The game's own ALLOWED list (the app's game.rs): the commands /api/cmd passes on
const ALLOWED: &[&str] = &[
    "gm_mode", "gm_map", "gm_set", "gm_addon", "gm_addonset", "gm_restartmap", "gm_bots", "gm_match", "gm_player", "gm_nav", "gm_pause", "gm_team", "gm_preset", "gm_presetsave", "gm_presetdel", "gm_pos", "gm_crash",
    "gm_do", "gm_announce", "gm_votemode",
];

/// Doing what was asked (it already got past the guard).
#[allow(clippy::too_many_arguments)]
fn act(ctx: &Ctx, config: &Config, files: &Files, io: &dyn GameIo, m: &Member, method: &str, path: &str, body: Option<&Value>) -> Resp {
    let empty = json!({});
    let b = body.unwrap_or(&empty);
    let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match (method, seg.as_slice()) {
        ("GET", ["api", "control", "me"]) => ok(json!({"role": m.role, "role_name": m.role.name(), "name": m.name, "server": server_view(config, io), "version": VERSION})),
        // an Admin inviting a Moderator to this server
        ("POST", ["api", "control", "invite"]) => match files.invite(config, Role::Moderator, b["steam"].as_str().unwrap_or(""), &m.id) {
            Ok((iid, text)) => ok(json!({"ok": true, "invite": text, "id": iid})),
            Err(e) => err(e),
        },
        // where to join, with the join password (helpers are people the owner trusts; their
        // app joins with it and doesn't show it)
        ("POST", ["api", "control", "join"]) => {
            let address = config.view["connect"].as_str().filter(|a| !a.is_empty()).map(String::from);
            ok(json!({"address": address, "port": config.port, "password": game::join_password(&io.game_dir()), "game": config.game, "unlisted": config.view["visibility"] == "unlisted"}))
        }
        ("GET", ["api", "state"]) => {
            let mut cached = ctx.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((at, v)) = cached.as_ref().filter(|(at, _)| at.elapsed() < Duration::from_secs(1)) {
                let _ = at;
                return ok(v.clone());
            }
            let v = game::state(io);
            *cached = Some((Instant::now(), v.clone()));
            ok(v)
        }
        ("GET", ["api", "health"]) => ok(json!({"ok": true, "checks": [], "fixed": []})),
        ("GET", ["api", "maps"]) => ok(json!(game::maps(&io.game_dir()))),
        ("GET", ["api", "modes", mode, "maps"]) => {
            if !mode.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || mode.len() > 32 {
                return refuse(400, "unknown mode");
            }
            ok(json!(game::mode_maps(&io.game_dir(), mode)))
        }
        ("POST", ["api", "cmd"]) => cmd(ctx, io, b),
        ("GET", ["api", "servers", _, "console"]) => ok(json!({"lines": game::console(&io.game_dir(), &game::secrets(&io.game_dir()))})),
        ("POST", ["api", "servers", _, "console", "send"]) => {
            let line = b["command"].as_str().unwrap_or("").trim().to_string();
            // (checked by the guard, and again right before it goes to the game)
            if let Err(why) = rules::console_line(m.role, &line) {
                return err(why);
            }
            if m.role == Role::Moderator {
                if let Some(id) = rules::mod_target(&line) {
                    if outranks(files, io, &id) {
                        return err(common::OUTRANKED);
                    }
                }
                // a mute typed in: only ever adding to one that's there, as on the Community
                // page (amx_gag <SteamID> <minutes> [chat|voice|both])
                let w: Vec<&str> = line.split_whitespace().collect();
                if w[0].eq_ignore_ascii_case("amx_gag") {
                    let kind = w.get(3).map(|k| k.to_ascii_lowercase()).unwrap_or_else(|| "both".into());
                    let (chat, voice) = match kind.as_str() {
                        "chat" => (true, false),
                        "voice" => (false, true),
                        _ => (true, true),
                    };
                    let minutes: u64 = w.get(2).and_then(|n| n.parse().ok()).unwrap_or(0);
                    let r = gag(io, m, &json!({"id": w.get(1).copied().unwrap_or(""), "minutes": minutes, "chat": chat, "voice": voice}));
                    return if went(&r) { ok(json!({"reply": "Muted."})) } else { r };
                }
            }
            match tell(io, &line) {
                // (keys and passwords blanked, as in the console: cvarlist prints them all)
                Ok(out) => ok(json!({"reply": out.lines().map(common::hide_passwords).collect::<Vec<_>>().join("\n")})),
                Err(_) => err("The server didn't answer. Check that it's running."),
            }
        }
        // suggestions while typing: not from here yet
        ("POST", ["api", "servers", _, "console", "hints"]) => ok(if b.get("value").is_some() { json!({"ok": true, "value": null}) } else { json!({"ok": true, "items": []}) }),
        // a restart: the game is told to quit, and its service starts it again
        ("POST", ["api", "servers", _, "restart"]) => {
            if !io.active().0 {
                return err("The server isn't running. Starting it is done from the owner's Loner-Strike for now.");
            }
            // (it quits before it can answer: no answer is the expected one)
            match io.rcon("quit") {
                Ok(_) | Err(RconError::Offline) => {
                    *ctx.state.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    ok(json!({"ok": true}))
                }
                Err(e) => err(e.say()),
            }
        }
        ("POST", ["api", "admins", "kick"]) => {
            let uid = b["uid"].as_u64().filter(|u| *u > 0 && *u < u32::MAX as u64);
            let Some(uid) = uid else { return err("That player isn't in the game any more.") };
            // a Moderator: not the owner, an Admin or a helper (who it is, from the game)
            if m.role == Role::Moderator {
                let Ok(status) = io.rcon("status") else { return err("The server didn't answer. Check that it's running.") };
                let Some(who) = common::steamid_for_uid(&status, uid as u32) else { return err("That player isn't in the game any more.") };
                if outranks(files, io, &who.to_uppercase()) {
                    return err(common::OUTRANKED);
                }
            }
            match io.rcon(&common::kick_line(uid as u32, b["reason"].as_str().unwrap_or(""))) {
                Ok(_) => ok(json!({"ok": true})),
                Err(e) => err(e.say()),
            }
        }
        ("POST", ["api", "admins", "ban"]) => ban(io, b),
        ("POST", ["api", "admins", "unban"]) => unban(io, b),
        ("POST", ["api", "community", "gag"]) => gag(io, m, b),
        ("POST", ["api", "community", "ungag"]) => {
            let id = b["id"].as_str().unwrap_or("").trim().to_uppercase();
            if !common::valid_steamid(&id) {
                return err("That isn't a SteamID. It looks like STEAM_0:1:12345.");
            }
            match io.rcon(&format!("amx_ungag {id}")) {
                Ok(_) => ok(json!({"ok": true})),
                Err(e) => err(e.say()),
            }
        }
        // their role allows it, but it needs the owner's files or downloads: settings, the
        // rotation and schedule, the welcome page, sounds, maps and other downloads, mode
        // settings, the mode vote, 5v5 settings, stats and reports, starting and stopping
        _ => err(OWNER_APP),
    }
}

/// A Game Mode Menu command (/api/cmd), checked as the app's game.rs checks it.
fn cmd(ctx: &Ctx, io: &dyn GameIo, b: &Value) -> Resp {
    let name = text(b.get("cmd").unwrap_or(&Value::Null));
    let args: Vec<String> = b["args"].as_array().into_iter().flatten().map(text).collect();
    if !ALLOWED.contains(&name.as_str()) {
        return Resp { status: 400, body: json!({"ok": false, "error": "unknown command"}) };
    }
    let line = if name == "gm_announce" {
        let t = common::announce_text(&args);
        if t.is_empty() {
            return Resp { status: 400, body: json!({"ok": false, "error": "empty message"}) };
        }
        format!("gm_announce \"{t}\"")
    } else {
        if !args.iter().all(|a| common::safe_arg(a)) {
            return Resp { status: 400, body: json!({"ok": false, "error": "invalid argument"}) };
        }
        std::iter::once(name.clone()).chain(args).collect::<Vec<_>>().join(" ")
    };
    let sent = tell(io, &line);
    // the next state shows what it did
    *ctx.state.lock().unwrap_or_else(|e| e.into_inner()) = None;
    match sent {
        Ok(_) if name == "gm_announce" => ok(json!({"ok": true, "output": []})),
        Ok(reply) => ok(json!({"ok": true, "output": reply.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>()})),
        Err(e) => Resp { status: 503, body: json!({"ok": false, "error": e.say()}) },
    }
}

/// A ban by SteamID: for a while through the game (it has to be running), for good also
/// written to banned.cfg so it outlasts a restart (as the app's admins.rs does it).
fn ban(io: &dyn GameIo, b: &Value) -> Resp {
    let id = b["id"].as_str().unwrap_or("").trim().to_uppercase();
    if !common::valid_steamid(&id) {
        return err("That isn't a SteamID. It looks like STEAM_0:1:12345. LAN and bot IDs can't be banned.");
    }
    let minutes = b["minutes"].as_u64().unwrap_or(0).min(525_600);
    let online = match io.rcon(&format!("banid {minutes} {id} kick")) {
        Ok(_) => true,
        Err(RconError::Offline) => false,
        Err(e) => return err(e.say()),
    };
    if minutes > 0 {
        if !online {
            return err("Bans for a set time need the game running. A permanent ban works any time.");
        }
        return ok(json!({"ok": true}));
    }
    match edit_file(&io.game_dir().join(game::ban_file("id")), |t| game::with_ban(t, &id)) {
        Ok(()) => ok(json!({"ok": true})),
        Err(e) => err(e),
    }
}

fn unban(io: &dyn GameIo, b: &Value) -> Resp {
    let value = b["value"].as_str().unwrap_or("").trim().to_string();
    let kind = match b["kind"].as_str() {
        Some("id") if common::valid_steamid(&value) => "id",
        Some("ip") if value.parse::<std::net::Ipv4Addr>().is_ok() => "ip",
        _ => return err("That isn't a SteamID or IP address."),
    };
    match io.rcon(&format!("{} {value}", if kind == "ip" { "removeip" } else { "removeid" })) {
        Ok(_) | Err(RconError::Offline) => {}
        Err(e) => return err(e.say()),
    }
    match edit_file(&io.game_dir().join(game::ban_file(kind)), |t| game::without_ban(t, &value)) {
        Ok(()) => ok(json!({"ok": true})),
        Err(e) => err(e),
    }
}

/// A mute, through the plugin's amx_gag (it keeps gags.txt). A Moderator only adds to a
/// mute that's there: never shorter, never less of it (lifting any of it is an Admin's call).
fn gag(io: &dyn GameIo, m: &Member, b: &Value) -> Resp {
    let id = b["id"].as_str().unwrap_or("").trim().to_uppercase();
    if !common::valid_steamid(&id) {
        return err("That isn't a SteamID. It looks like STEAM_0:1:12345. Bots and LAN players can't be muted from here.");
    }
    let (mut chat, mut voice) = (b["chat"].as_bool().unwrap_or(true), b["voice"].as_bool().unwrap_or(false));
    if !chat && !voice {
        return err("Pick chat, voice or both.");
    }
    let minutes = b["minutes"].as_u64().unwrap_or(0);
    if minutes > 525_600 {
        return err("A mute can last a year at most. Pick \"until removed\" for longer.");
    }
    let mut minutes = minutes;
    if m.role == Role::Moderator {
        if let Some((until, c, v)) = game::current_gag(&io.game_dir(), &id) {
            chat |= c;
            voice |= v;
            let now = store::now();
            let wanted = now + minutes * 60;
            if until == 0 {
                minutes = 0;
            } else if until > wanted {
                minutes = (until - now).div_ceil(60);
            }
        }
    }
    let kind = match (chat, voice) {
        (true, true) => "both",
        (true, false) => "chat",
        _ => "voice",
    };
    match io.rcon(&format!("amx_gag {id} {minutes} {kind}")) {
        Ok(_) => ok(json!({"ok": true})),
        Err(RconError::Offline) => err("Mutes from here need the game running."),
        Err(e) => err(e.say()),
    }
}

/// One ban file change at a time (read, changed, written)
static BAN_FILES: Mutex<()> = Mutex::new(());

/// A file of the game folder changed by `f` (Latin-1), written whole.
fn edit_file(path: &std::path::Path, f: impl FnOnce(&str) -> String) -> Result<(), String> {
    let _one = BAN_FILES.lock().unwrap_or_else(|e| e.into_inner());
    let old: String = match std::fs::read(path) {
        Ok(b) => b.iter().map(|&c| c as char).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("Couldn't read the ban list ({e}).")),
    };
    let new = f(&old);
    if new == old {
        return Ok(());
    }
    let bytes: Vec<u8> = new.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
    // written in place: the service may write these two files and nothing else in the game
    // folder (its unit's ReadWritePaths), so there's no temporary file to rename over them
    std::fs::write(path, bytes).map_err(|e| format!("Couldn't save the ban list ({e})."))
}

/// The answer as sent: JSON with the RCON password blanked out, whatever it came from.
pub fn finish(resp: &Resp, game_dir: &std::path::Path) -> String {
    let text = resp.body.to_string();
    // (only the long, random RCON password: the join password could be an ordinary word)
    let rcon: Vec<String> = game::rcon_password(game_dir).into_iter().filter(|s| s.len() >= 8).collect();
    game::redact(&text, &rcon)
}

// ---- the listener ------------------------------------------------------------------------

/// The address a request came from: behind cloudflared (all from 127.0.0.1) the visitor's
/// address Cloudflare gives (CF-Connecting-IP, which its edge always sets itself; Caddy takes
/// any such header off), else the last X-Forwarded-For entry, which the front added itself.
fn peer_of(headers: &HashMap<String, String>, socket: Option<IpAddr>) -> Option<IpAddr> {
    if socket.is_some_and(|ip| ip.is_loopback()) {
        let cf = headers.get("cf-connecting-ip").and_then(|v| v.trim().parse().ok());
        let xff = || headers.get("x-forwarded-for").and_then(|v| v.rsplit(',').next()).and_then(|v| v.trim().parse().ok());
        if let Some(ip) = cf.or_else(xff) {
            return Some(ip);
        }
    }
    socket
}

/// Connections handled at once (more are turned away until one ends)
const MAX_CONNS: usize = 64;
/// How long a connection may take to send its request, and to take the answer
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// The most a request's line and headers may be
const MAX_HEAD: usize = 16 * 1024;

pub fn serve(paths: Paths, port: u16) -> Result<(), String> {
    // 127.0.0.1 only: the front (Caddy or cloudflared) is the way in
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("Couldn't listen on 127.0.0.1:{port} ({e})."))?;
    let ctx = Arc::new(Ctx::new(paths, None));
    eprintln!("ls-control {VERSION} for {} on 127.0.0.1:{port}", ctx.paths.id);
    serve_on(listener, ctx);
    Ok(())
}

/// One thread per connection, at most MAX_CONNS of them, each with time limits: a slow or
/// silent connection is dropped instead of holding a place for long.
fn serve_on(listener: std::net::TcpListener, ctx: Arc<Ctx>) {
    let open = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        use std::sync::atomic::Ordering::SeqCst;
        if open.fetch_add(1, SeqCst) >= MAX_CONNS {
            open.fetch_sub(1, SeqCst);
            continue;
        }
        let (ctx, open2) = (ctx.clone(), open.clone());
        let spawned = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || {
            connection(&ctx, stream);
            open2.fetch_sub(1, SeqCst);
        });
        if spawned.is_err() {
            open.fetch_sub(1, SeqCst);
        }
    }
}

/// What came in on a connection, up to its body.
struct Head {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    /// bytes of the body already read with the head
    rest: Vec<u8>,
}

/// The request line and headers (httparse), or the status to answer with.
fn read_head(s: &mut std::net::TcpStream, started: Instant) -> Result<Head, u16> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        if started.elapsed() > IO_TIMEOUT {
            return Err(408);
        }
        let n = s.read(&mut chunk).map_err(|_| 408u16)?;
        if n == 0 {
            return Err(400);
        }
        buf.extend_from_slice(&chunk[..n]);
        let mut hs = [httparse::EMPTY_HEADER; 48];
        let mut req = httparse::Request::new(&mut hs);
        match req.parse(&buf) {
            Ok(httparse::Status::Complete(len)) => {
                let mut headers: HashMap<String, String> = HashMap::new();
                for h in req.headers.iter() {
                    let k = h.name.to_ascii_lowercase();
                    let v = String::from_utf8_lossy(h.value).to_string();
                    // repeated headers: the values in order, as one (X-Forwarded-For)
                    headers.entry(k).and_modify(|x| *x = format!("{x}, {v}")).or_insert(v);
                }
                return Ok(Head { method: req.method.unwrap_or("").to_string(), path: req.path.unwrap_or("").to_string(), headers, rest: buf[len..].to_vec() });
            }
            Ok(httparse::Status::Partial) if buf.len() < MAX_HEAD => continue,
            Ok(httparse::Status::Partial) => return Err(431),
            Err(_) => return Err(400),
        }
    }
}

/// The body (Content-Length only; the fronts never send anything else to here).
fn read_body(s: &mut std::net::TcpStream, head: &mut Head, started: Instant) -> Result<Vec<u8>, u16> {
    use std::io::Read;
    if head.headers.contains_key("transfer-encoding") {
        return Err(411);
    }
    let len = match head.headers.get("content-length") {
        None => 0,
        Some(v) => v.trim().parse::<usize>().map_err(|_| 400u16)?,
    };
    if len > MAX_BODY {
        return Err(413);
    }
    let mut body = std::mem::take(&mut head.rest);
    body.truncate(len);
    let mut chunk = [0u8; 4096];
    while body.len() < len {
        if started.elapsed() > IO_TIMEOUT {
            return Err(408);
        }
        let n = s.read(&mut chunk[..(len - body.len()).min(4096)]).map_err(|_| 408u16)?;
        if n == 0 {
            return Err(400);
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Ok(body)
}

/// Whether a request's body is worth reading: a redeem or a ping, or someone with a key
/// that works (a body from anyone else is never waited for).
fn body_wanted(ctx: &Ctx, head: &Head) -> bool {
    match clean_path(&head.path).as_deref() {
        Some("/api/control/redeem") | Some("/api/control/ping") => true,
        Some(_) => {
            let key = head.headers.get("authorization").and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("").trim();
            matches!(Files::new(&ctx.paths).member_for(key), Ok(Some(_)))
        }
        None => false,
    }
}

fn connection(ctx: &Ctx, mut s: std::net::TcpStream) {
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    let started = Instant::now();
    let socket = s.peer_addr().ok().map(|a| a.ip());
    let resp = match read_head(&mut s, started) {
        Err(code) => refuse(code, "That isn't something Loner-Strike sends."),
        Ok(mut head) => {
            let peer = peer_of(&head.headers, socket);
            // a redeem over its tries: no body read, no file touched
            let redeem = clean_path(&head.path).as_deref() == Some("/api/control/redeem");
            if redeem && too_many(&ctx.redeems, peer, TRIES_MAX, None) {
                refuse(429, "Too many tries. Wait a few minutes.")
            } else {
                let body = if body_wanted(ctx, &head) { read_body(&mut s, &mut head, started) } else { Ok(Vec::new()) };
                match body {
                    Err(413) => refuse(413, "That's too big."),
                    Err(code) => refuse(code, "That isn't something Loner-Strike sends."),
                    Ok(body) => {
                        let req = Req { method: head.method, path: head.path, headers: head.headers, body, peer };
                        // one request going wrong never takes the service down
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(ctx, req, !redeem))).unwrap_or_else(|_| refuse(500, "Something went wrong on the server. Try again."))
                    }
                }
            }
        }
    };
    let game_dir = Config::read(&ctx.paths).map(|c| ctx.paths.server_dir().join(c.game)).unwrap_or_default();
    let text = finish(&resp, &game_dir);
    let reason = match resp.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        426 => "Upgrade Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let out = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        resp.status,
        text.len()
    );
    use std::io::Write;
    let _ = s.write_all(out.as_bytes());
    let _ = s.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{secret_of, temp};

    /// A game that answers every command, and remembers what it was sent
    struct Fake {
        dir: std::path::PathBuf,
        sent: Mutex<Vec<String>>,
        reply: String,
    }

    impl GameIo for Fake {
        fn rcon(&self, cmd: &str) -> Result<String, RconError> {
            self.sent.lock().unwrap().push(cmd.to_string());
            Ok(self.reply.clone())
        }
        fn active(&self) -> (bool, u64) {
            (true, 60)
        }
        fn game_dir(&self) -> std::path::PathBuf {
            self.dir.clone()
        }
    }

    struct Setup {
        ctx: Ctx,
        fake: Arc<Fake>,
        files: Files,
        config: Config,
    }

    fn setup(reply: &str) -> Setup {
        let (p, files, config) = temp("friday");
        let dir = p.server_dir().join("cstrike");
        std::fs::create_dir_all(game::data_dir(&dir)).unwrap();
        std::fs::write(game::data_dir(&dir).join("panel_rcon.txt"), "Rcon0123456789Secret\n").unwrap();
        let fake = Arc::new(Fake { dir, sent: Mutex::new(vec![]), reply: reply.into() });
        Setup { ctx: Ctx::new(p, Some(fake.clone())), fake, files, config }
    }

    impl Setup {
        /// someone with this role: their key
        fn person(&self, role: Role, steam: &str) -> String {
            let (_, text) = self.files.invite(&self.config, role, steam, "owner").unwrap();
            self.files.redeem(&secret_of(&text), "Pat").unwrap().1
        }
        fn ask(&self, key: &str, method: &str, path: &str, body: Value) -> Resp {
            let headers = HashMap::from([("authorization".to_string(), format!("Bearer {key}")), ("x-ls-version".to_string(), VERSION.to_string())]);
            let body = if body.is_null() { vec![] } else { body.to_string().into_bytes() };
            handle(&self.ctx, Req { method: method.into(), path: path.into(), headers, body, peer: Some("203.0.113.9".parse().unwrap()) }, true)
        }
        fn sent(&self) -> Vec<String> {
            self.fake.sent.lock().unwrap().clone()
        }
    }

    #[test]
    fn keys_and_versions() {
        let s = setup("ok");
        let key = s.person(Role::Admin, "");
        assert_eq!(s.ask("", "GET", "/api/state", Value::Null).status, 401);
        assert_eq!(s.ask(&"a".repeat(64), "GET", "/api/state", Value::Null).status, 401);
        assert!(went(&s.ask(&key, "GET", "/control/api/control/me", Value::Null)), "behind a tunnel the /control prefix stays on");
        let mut r = Req { method: "GET".into(), path: "/api/control/me".into(), headers: HashMap::from([("authorization".to_string(), format!("Bearer {key}"))]), body: vec![], peer: None };
        assert_eq!(handle(&s.ctx, r, true).status, 426, "same version only");
        r = Req { method: "DELETE".into(), path: "/api/state".into(), headers: HashMap::new(), body: vec![], peer: None };
        assert_eq!(handle(&s.ctx, r, true).status, 405);
        assert_eq!(s.ask(&key, "GET", "/api/../etc/passwd", Value::Null).status, 404);
        let me = s.ask(&key, "GET", "/api/control/me", Value::Null).body;
        assert_eq!((me["role"].as_str(), me["server"]["id"].as_str(), me["server"]["state"].as_str()), (Some("admin"), Some("friday"), Some("running")));
    }

    #[test]
    fn roles_by_the_shared_rules() {
        let s = setup("ok");
        let m = s.person(Role::Moderator, "");
        let a = s.person(Role::Admin, "");
        assert!(went(&s.ask(&m, "POST", "/api/cmd", json!({"cmd": "gm_map", "args": ["de_dust2"]}))));
        assert_eq!(s.ask(&m, "POST", "/api/cmd", json!({"cmd": "gm_set", "args": ["mp_timelimit", "5"]})).status, 403);
        assert!(went(&s.ask(&a, "POST", "/api/cmd", json!({"cmd": "gm_set", "args": ["mp_timelimit", "5"]}))));
        assert_eq!(s.ask(&a, "POST", "/api/cmd", json!({"cmd": "gm_set", "args": ["rcon_password", "x"]})).status, 403);
        assert_eq!(s.ask(&a, "POST", "/api/cmd", json!({"cmd": "gm_map", "args": ["de_dust2;quit"]})).status, 400);
        assert_eq!(s.ask(&m, "POST", "/api/servers/friday/console/send", json!({"command": "mp_timelimit 5"})).status, 403);
        assert_eq!(s.ask(&a, "POST", "/api/servers/friday/console/send", json!({"command": "rcon_password x"})).status, 403);
        assert!(went(&s.ask(&a, "POST", "/api/servers/friday/console/send", json!({"command": "mp_timelimit 5"}))));
        assert_eq!(s.ask(&m, "POST", "/api/servers/friday/restart", Value::Null).status, 403);
        assert_eq!(s.ask(&m, "POST", "/api/control/invite", json!({})).status, 403);
        assert!(s.ask(&a, "POST", "/api/control/invite", json!({})).body["invite"].as_str().unwrap().starts_with("LS1."));
        // nothing that isn't named, whatever the role
        for (method, path) in [("GET", "/api/servers"), ("POST", "/api/servers/friday/delete"), ("POST", "/api/servers/friday/rcon-password/show"), ("GET", "/api/admins"), ("POST", "/api/control/redeem2")] {
            assert_eq!(s.ask(&a, method, path, json!({})).status, 403, "{method} {path}");
        }
        assert_eq!(s.sent(), vec!["gm_map de_dust2", "gm_set mp_timelimit 5", "mp_timelimit 5"]);
        // a change that needs the owner's files is said so
        let r = s.ask(&a, "POST", "/api/welcome", json!({"text": "hi"}));
        assert_eq!(r.body["error"], OWNER_APP);
        // what went through is in the log, what was refused isn't
        let log = s.files.activity(50);
        assert!(log.iter().any(|e| e["path"] == "/api/cmd") && !log.iter().any(|e| e["path"] == "/api/welcome"));
    }

    #[test]
    fn pinned_to_this_server() {
        let s = setup("ok");
        let a = s.person(Role::Admin, "");
        assert_eq!(s.ask(&a, "GET", "/api/servers/other/console", Value::Null).status, 403);
        assert_eq!(s.ask(&a, "POST", "/api/servers/other/restart", Value::Null).status, 403);
        assert!(went(&s.ask(&a, "GET", "/api/servers/friday/console", Value::Null)));
        // someone given another server (a store copied from elsewhere) gets nothing here
        s.files.edit(|st| {
            st.members.iter_mut().for_each(|m| m.server = "other".into());
            Ok(())
        })
        .unwrap();
        assert_eq!(s.ask(&a, "GET", "/api/state", Value::Null).status, 403);
    }

    #[test]
    fn moderators_and_who_they_ban() {
        let s = setup("ok");
        let m = s.person(Role::Moderator, "STEAM_0:1:100");
        s.person(Role::Admin, "STEAM_0:1:200");
        assert_eq!(s.ask(&m, "POST", "/api/admins/ban", json!({"id": "STEAM_0:1:200", "minutes": 60})).status, 403, "someone helping run it");
        assert_eq!(s.ask(&m, "POST", "/api/admins/ban", json!({"id": "STEAM_0:1:5", "minutes": 0})).status, 403, "for good: admins only");
        assert!(went(&s.ask(&m, "POST", "/api/admins/ban", json!({"id": "steam_0:1:5", "minutes": 60}))));
        assert!(went(&s.ask(&m, "POST", "/api/community/gag", json!({"id": "STEAM_0:1:6", "minutes": 30, "chat": true}))));
        assert_eq!(s.ask(&m, "POST", "/api/servers/friday/console/send", json!({"command": "amx_ban STEAM_0:1:200 60 x"})).status, 200);
        assert!(s.ask(&m, "POST", "/api/servers/friday/console/send", json!({"command": "amx_ban STEAM_0:1:200 60 x"})).body["error"].is_string());
        assert_eq!(s.sent(), vec!["banid 60 STEAM_0:1:5 kick", "amx_gag STEAM_0:1:6 30 chat"]);
        // typed in the console, a mute never shortens one that's there
        let data = game::data_dir(&s.fake.dir);
        std::fs::write(data.join("gags.txt"), "STEAM_0:1:7\t0\tcv\tEve\n").unwrap();
        assert!(went(&s.ask(&m, "POST", "/api/servers/friday/console/send", json!({"command": "amx_gag STEAM_0:1:7 1 voice"}))));
        assert_eq!(s.sent().last().map(String::as_str), Some("amx_gag STEAM_0:1:7 0 both"), "kept until removed, chat and voice");
        assert_eq!(s.ask(&m, "POST", "/api/servers/friday/console/send", json!({"command": "amx_ungag STEAM_0:1:7"})).status, 403);
    }

    #[test]
    fn redeem_tries_are_limited() {
        let s = setup("ok");
        let (_, text) = s.files.invite(&s.config, Role::Admin, "", "owner").unwrap();
        let try_it = |secret: &str, ip: &str| {
            let body = json!({"invite": secret, "name": "Bob", "version": VERSION}).to_string().into_bytes();
            handle(&s.ctx, Req { method: "POST".into(), path: "/api/control/redeem".into(), headers: HashMap::new(), body, peer: Some(ip.parse().unwrap()) }, true)
        };
        for _ in 0..TRIES_MAX {
            assert!(try_it(&"0".repeat(32), "198.51.100.1").body["error"].is_string());
        }
        assert_eq!(try_it(&secret_of(&text), "198.51.100.1").status, 429, "even the right one, once over");
        let r = try_it(&secret_of(&text), "198.51.100.2");
        assert_eq!(r.body["key"].as_str().map(str::len), Some(64));
        assert_eq!(r.body["server"]["id"], "friday");
        assert!(try_it(&secret_of(&text), "198.51.100.3").body["error"].is_string(), "single use");
    }

    #[test]
    fn wrong_keys_wait() {
        let s = setup("ok");
        let key = s.person(Role::Admin, "");
        for _ in 0..BAD_KEYS_MAX {
            assert_eq!(s.ask(&"b".repeat(64), "GET", "/api/state", Value::Null).status, 401);
        }
        assert_eq!(s.ask(&key, "GET", "/api/state", Value::Null).status, 429);
    }

    #[test]
    fn secrets_never_leave() {
        let s = setup("rcon_password is Rcon0123456789Secret");
        let a = s.person(Role::Admin, "");
        let r = s.ask(&a, "POST", "/api/servers/friday/console/send", json!({"command": "status"}));
        assert!(!r.body.to_string().contains("Rcon0123456789Secret"));
        let out = finish(&Resp { status: 200, body: json!({"x": "Rcon0123456789Secret"}) }, &s.fake.dir);
        assert!(!out.contains("Rcon0123456789Secret"));
        // no join password set: none to give
        assert_eq!(s.ask(&a, "POST", "/api/control/join", json!({})).body["password"], Value::Null);
    }

    #[test]
    fn over_a_socket() {
        use std::io::{Read, Write};
        let s = setup("ok");
        let key = s.person(Role::Admin, "");
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let ctx = Arc::new(Ctx::new(s.ctx.paths.clone(), Some(s.fake.clone())));
        std::thread::spawn(move || serve_on(l, ctx));
        let ask = |raw: String| {
            let mut c = std::net::TcpStream::connect(addr).unwrap();
            c.write_all(raw.as_bytes()).unwrap();
            let mut out = String::new();
            let _ = c.read_to_string(&mut out);
            out
        };
        let me = ask(format!("GET /control/api/control/me HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {key}\r\nX-LS-Version: {VERSION}\r\n\r\n"));
        assert!(me.starts_with("HTTP/1.1 200 OK") && me.contains("\"role\":\"admin\""), "{me}");
        // no key: answered at once, the promised body never waited for
        let t = Instant::now();
        let no = ask("POST /api/cmd HTTP/1.1\r\nHost: x\r\nContent-Length: 500\r\n\r\n".into());
        assert!(no.starts_with("HTTP/1.1 401") && t.elapsed() < Duration::from_secs(3), "{no}");
        let chunked = ask(format!("POST /api/cmd HTTP/1.1\r\nAuthorization: Bearer {key}\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"));
        assert!(chunked.starts_with("HTTP/1.1 411"), "{chunked}");
        let big = ask(format!("POST /api/cmd HTTP/1.1\r\nAuthorization: Bearer {key}\r\nContent-Length: {}\r\n\r\n", MAX_BODY + 1));
        assert!(big.starts_with("HTTP/1.1 413"), "{big}");
        assert!(ask("nonsense\r\n\r\n".into()).starts_with("HTTP/1.1 400"));
    }

    #[test]
    fn forwarded_addresses() {
        let h = HashMap::from([("x-forwarded-for".to_string(), "1.2.3.4, 198.51.100.7".to_string())]);
        assert_eq!(peer_of(&h, Some("127.0.0.1".parse().unwrap())), Some("198.51.100.7".parse().unwrap()), "the front's own entry");
        assert_eq!(peer_of(&h, Some("192.0.2.1".parse().unwrap())), Some("192.0.2.1".parse().unwrap()), "only trusted from the front");
        let cf = HashMap::from([("x-forwarded-for".to_string(), "1.2.3.4, 198.51.100.7".to_string()), ("cf-connecting-ip".to_string(), "203.0.113.5".to_string())]);
        assert_eq!(peer_of(&cf, Some("127.0.0.1".parse().unwrap())), Some("203.0.113.5".parse().unwrap()), "Cloudflare's own header first");
        assert_eq!(clean_path("/control/api/state?x=1").as_deref(), Some("/api/state"));
        assert_eq!(clean_path("/controlx/api/state"), None);
    }
}
