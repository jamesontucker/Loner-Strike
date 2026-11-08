//! What a helper's role lets them do on the server they were given: every request from
//! the sharing listener is checked here before anything else sees it. Nothing is allowed
//! unless a rule below names it (method and path), and some requests are also checked or
//! trimmed by what they carry.
//!
//! Never for helpers, whatever their role: this computer's own things (its game, phone
//! access, updates, folders, other servers), the server's keys (console, HLTV, webhooks,
//! tunnel, the join password's setting; their app is given the join password to join), the machine itself (SSH, files, start options,
//! deleting or rebuilding the server), backups, and programs (their own plugins).

use super::Role;
use serde_json::{Map, Value};

/// What happens to a request.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    /// let it through, with this body in place of the one sent (Some) or as it is (None)
    Allow(Option<Value>),
    /// refused, saying why
    Deny(String),
}

/// One request, as the listener saw it.
pub struct Ask<'a> {
    pub role: Role,
    /// "GET" or "POST" (anything else is refused)
    pub method: &'a str,
    pub path: &'a str,
    /// the server this helper was given
    pub server: &'a str,
    /// a POST's JSON body (None: not JSON or none)
    pub body: Option<&'a Value>,
}

/// The console commands of Loner-Strike's own plugin (/api/cmd, game.rs ALLOWED) a
/// Moderator may send: change or restart the map, and say something to everyone.
const MOD_CMDS: &[&str] = &["gm_map", "gm_restartmap", "gm_announce"];
/// ...and the ones no helper sends (a test command that crashes the game)
const NEVER_CMDS: &[&str] = &["gm_crash"];

/// Server settings (POST /api/servers/:id) an Admin may change; the rest (who can join,
/// the join password, port, listen address, public address, start options, the
/// Loner-Strike list) are left out of what reaches the server. Who can join stays with the
/// owner: listing it on Steam shows their address to everyone, and This network shuts out
/// everyone outside it.
const ADMIN_SERVER_FIELDS: &[&str] = &["name", "map", "maxplayers", "bots_when_empty"];

/// The longest ban a Moderator gives (7 days); only Admins and the owner ban for good.
pub const MOD_BAN_MINUTES: u64 = 7 * 24 * 60;

/// Paths under /api/servers/ that are the app's own, never a server's id
const RESERVED: &[&str] = &["select", "import", "remote", "pick-folder", "pick-key"];

fn deny(why: &str) -> Verdict {
    Verdict::Deny(why.to_string())
}

const NOT_YOURS: &str = "Only the server's owner can do that.";
const NOT_ROLE: &str = "Your role on this server doesn't include that.";

/// The decision for one request. `known_setting(mode, cvar)`: whether the server's own
/// game reports that setting for that mode (asked only for mode settings).
pub fn check(a: &Ask, known_setting: &dyn Fn(&str, &str) -> bool) -> Verdict {
    let admin = a.role == Role::Admin;
    let seg: Vec<&str> = a.path.trim_start_matches('/').split('/').collect();
    // a path naming a server names theirs, or nothing happens
    if seg.len() >= 3 && seg[0] == "api" && seg[1] == "servers" && (seg[2] != a.server || RESERVED.contains(&seg[2])) {
        return deny(NOT_YOURS);
    }
    let both = Verdict::Allow(None);
    let admins = || if admin { Verdict::Allow(None) } else { deny(NOT_ROLE) };
    match (a.method, seg.as_slice()) {
        // ---- reading ---------------------------------------------------------------
        ("GET", ["api", "state"] | ["api", "health"] | ["api", "maps"] | ["api", "modes", _, "maps"] | ["thumb", _]) => both,
        ("GET", ["api", "control", "me"]) => both,
        ("GET", ["api", "servers", _, "console"]) => both,
        // typing in the console: their role's commands (console_line), and suggestions for those
        ("POST", ["api", "servers", _, "console", "send"]) => {
            let line = a.body.and_then(|b| b["command"].as_str()).unwrap_or("");
            match console_line(a.role, line) {
                Ok(()) => both,
                Err(why) => Verdict::Deny(why),
            }
        }
        ("POST", ["api", "servers", _, "console", "hints"]) => both,
        ("GET", ["api", "community"] | ["api", "community", "chat"] | ["api", "community", "reports"] | ["api", "community", "stats"]) => both,
        (
            "GET",
            ["api", "extras"]
            | ["api", "welcome"]
            | ["api", "modevote"]
            | ["api", "scrim"]
            | ["api", "servercontent"]
            | ["api", "gb", "browse"]
            | ["api", "gb", "jobs"]
            | ["api", "gb", "installed"]
            | ["api", "gb", "mod", _]
            | ["api", "sounds"]
            | ["api", "sounds", "file"]
            | ["api", "mods", "plugins"]
            | ["api", "app", "mode-settings"],
        ) => admins(),

        // ---- the game, live --------------------------------------------------------
        ("POST", ["api", "cmd"]) => {
            let cmd = a.body.and_then(|b| b["cmd"].as_str()).unwrap_or("");
            if NEVER_CMDS.contains(&cmd) {
                deny(NOT_ROLE)
            } else if admin || MOD_CMDS.contains(&cmd) {
                both
            } else {
                deny(NOT_ROLE)
            }
        }

        // ---- players ---------------------------------------------------------------
        ("POST", ["api", "admins", "kick"]) => both,
        // the ban file only holds bans for good: lifting one is an Admin's call
        ("POST", ["api", "admins", "unban"]) => admins(),
        ("POST", ["api", "admins", "ban"]) => {
            let minutes = a.body.and_then(|b| b["minutes"].as_u64()).unwrap_or(0);
            if admin || (1..=MOD_BAN_MINUTES).contains(&minutes) {
                both
            } else {
                deny("Moderators ban for up to 7 days. Ask an Admin or the owner for a longer ban.")
            }
        }
        ("POST", ["api", "community", "gag"]) => {
            let minutes = a.body.and_then(|b| b["minutes"].as_u64()).unwrap_or(0);
            if admin || (1..=MOD_BAN_MINUTES).contains(&minutes) {
                both
            } else {
                deny("Moderators mute for up to 7 days. Ask an Admin or the owner for longer.")
            }
        }
        ("POST", ["api", "community", "ungag"] | ["api", "community", "reports"]) => both,
        ("POST", ["api", "community", "settings"] | ["api", "community", "stats", "reset"]) => admins(),

        // ---- the server's setup ----------------------------------------------------
        ("POST", ["api", "extras", "rotation"] | ["api", "extras", "schedule"] | ["api", "extras", "settings"]) => admins(),
        ("POST", ["api", "welcome"] | ["api", "modevote"] | ["api", "scrim", "settings"] | ["api", "scrim", "action"]) => admins(),
        // maps only: other downloads (skins, sounds...) go into the owner's own game
        ("POST", ["api", "gb", "install"] | ["api", "gb", "uninstall"]) => {
            // (no section is maps; anything else, null included, isn't)
            let section = a.body.and_then(|b| b.get("section"));
            if !admin {
                deny(NOT_ROLE)
            } else if section.is_none() || section.is_some_and(|s| s.as_str() == Some("maps")) {
                both
            } else {
                deny("Only maps can be added on a shared server. Other downloads are the owner's.")
            }
        }
        ("POST", ["api", "mapflag"]) => admins(),
        ("POST", ["api", "sounds", "track"] | ["api", "sounds", "announcer"] | ["api", "sounds", "addon"]) => admins(),
        ("POST", ["api", "app", "mode-settings"]) => {
            if !admin {
                return deny(NOT_ROLE);
            }
            // only a setting the server's own game has for that mode (the endpoint takes any
            // name, so a password or rcon_password could otherwise go into the mode's config)
            let mode = a.body.and_then(|b| b["mode"].as_str()).unwrap_or("");
            let cvar = a.body.and_then(|b| b["cvar"].as_str()).unwrap_or("").to_ascii_lowercase();
            // and nothing the console keeps from them (config files the server runs, logs)
            if secret_setting(&cvar) || !known_setting(mode, &cvar) || console_line(Role::Admin, &cvar).is_err() {
                deny(NOT_YOURS)
            } else {
                both
            }
        }
        ("POST", ["api", "servers", _]) => {
            if !admin {
                return deny(NOT_ROLE);
            }
            // only the settings an Admin changes; the owner's ones never reach the server
            let Some(Value::Object(body)) = a.body else { return deny("That isn't a settings change.") };
            let kept: Map<String, Value> = body.iter().filter(|(k, _)| ADMIN_SERVER_FIELDS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
            Verdict::Allow(Some(Value::Object(kept)))
        }
        ("POST", ["api", "servers", _, action]) => {
            if admin && matches!(*action, "start" | "stop" | "restart") {
                both
            } else if matches!(*action, "start" | "stop" | "restart") {
                deny(NOT_ROLE)
            } else {
                deny(NOT_YOURS)
            }
        }

        // ---- sharing itself ----------------------------------------------------------
        ("POST", ["api", "control", "invite"]) => admins(),
        // joining it from their own game (the address and join password, used by their app)
        ("POST", ["api", "control", "join"]) => both,

        _ => deny(NOT_YOURS),
    }
}

// ---- typing in the console ------------------------------------------------------------

/// Console commands no helper types: keys and passwords, running other files or new
/// commands, what players download, logs, admins and plugins, and shutting it down.
const CONSOLE_NEVER: &[&str] = &[
    "exec", "alias", "writecfg", "cfg", "unbind", "bind", "quit", "exit", "sv_downloadurl", "sv_allowupload", "sv_allowdownload", "log", "developer",
    "meta", "amxx", "amx_rcon", "amx_cvar", "amx_cfg", "amx_addadmin", "amx_reloadadmins", "amx_execall", "amx_exec", "amx_execteam", "amx_pausecfg", "amx_on", "amx_off",
    "amx_plugincmd", "gm_crash", "gm_fastdl", "gm_ai_url", "mapchangecfgfile", "motd_write", "ls_list", "setinfo", "localinfo", "motdfile", "mapcyclefile", "servercfgfile", "lservercfgfile", "bannedcfgfile", "listipcfgfile",
];
/// What a Moderator types: talk, look, kick, ban for up to 7 days, change the map.
const CONSOLE_MOD: &[&str] = &[
    "say", "status", "users", "kick", "banid", "changelevel", "amx_kick", "amx_ban", "amx_banip", "amx_map", "amx_say", "amx_csay",
    "amx_tsay", "amx_chat", "amx_who", "amx_gag", "amx_ungag", "amx_slap", "amx_slay", "amx_last",
];

/// Whether a helper may send this console line. One command per line (no `;` or line
/// breaks, which the game would run as more commands), nothing on CONSOLE_NEVER or naming
/// a key or password, and for a Moderator only CONSOLE_MOD, with bans of 1 to 7 days.
pub fn console_line(role: Role, line: &str) -> Result<(), String> {
    let line = line.trim();
    if line.is_empty() || line.len() > 200 || line.chars().any(|c| c.is_control()) {
        return Err("That command can't be sent.".into());
    }
    if line.contains(';') {
        return Err("One command at a time (no ;).".into());
    }
    // the game reads a quoted name ("exec") as the command itself, and may read letters
    // outside plain ASCII as spaces: the command has to be a plain word, the line plain text
    if !line.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        return Err("Only plain letters, numbers and symbols can be sent.".into());
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    let cmd = words[0].to_ascii_lowercase();
    if !cmd.chars().all(|c| c.is_ascii_alphanumeric() || "_+-.".contains(c)) {
        return Err("Start with the command's plain name (no quotes).".into());
    }
    // quotes could make the game see other words than these (a ban's length)
    if role == Role::Moderator && line.contains('"') {
        return Err("Moderators send commands without quotes.".into());
    }
    let starts = ["log", "logaddress", "sv_log", "sv_rcon", "rcon", "amx_plugin", "amx_pause", "amx_unpause"];
    if CONSOLE_NEVER.contains(&cmd.as_str()) || starts.iter().any(|p| cmd.starts_with(p)) || secret_setting(&cmd) {
        return Err(format!("{cmd} stays with the server's owner."));
    }
    // a setting named in the line (amx_cvar is out already): no keys or passwords either
    if words.iter().skip(1).any(|w| secret_setting(w.trim_matches('"'))) {
        return Err("Passwords and keys stay with the server's owner.".into());
    }
    if role == Role::Admin {
        return Ok(());
    }
    if !CONSOLE_MOD.contains(&cmd.as_str()) {
        return Err(format!("{cmd} isn't in a Moderator's commands."));
    }
    // bans: banid <minutes> <id>; amx_ban / amx_banip <player> <minutes> [reason]
    let minutes = match cmd.as_str() {
        "banid" => Some(words.get(1)),
        // amx_gag <player> <minutes>: 0 would be until removed
        "amx_ban" | "amx_banip" | "amx_gag" => Some(words.get(2)),
        _ => None,
    };
    if let Some(m) = minutes {
        let ok = m.and_then(|m| m.parse::<u64>().ok()).is_some_and(|m| (1..=MOD_BAN_MINUTES).contains(&m));
        if !ok {
            return Err("Moderators ban for 1 minute to 7 days (10080 minutes).".into());
        }
        // by SteamID, so the owner's app can tell it isn't the owner, an Admin or a helper
        if mod_target(line).is_none() {
            return Err("Moderators ban and mute by SteamID here (STEAM_0:1:12345). For a player in the game, use the Players page.".into());
        }
    }
    Ok(())
}

/// The SteamID a Moderator's console ban or mute is for: banid <minutes> <id>,
/// amx_ban / amx_banip / amx_gag <player> <minutes>.
pub fn mod_target(line: &str) -> Option<String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let who = match words.first()?.to_ascii_lowercase().as_str() {
        "banid" => words.get(2)?,
        "amx_ban" | "amx_banip" | "amx_gag" => words.get(1)?,
        _ => return None,
    };
    let id = who.to_ascii_uppercase();
    crate::admins::valid_steamid(&id).then_some(id)
}

/// Settings that are keys or passwords, never set by a helper anywhere.
pub fn secret_setting(cvar: &str) -> bool {
    let c = cvar.to_ascii_lowercase();
    ["password", "passwd", "rcon", "token", "secret", "webhook", "downloadurl", "fastdl", "ai_url", "_key", "apikey", "sql_"].iter().any(|w| c.contains(w)) || c.ends_with("_pass") || c.contains("_pass_") || c == "sv_lan"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ask<'a>(role: Role, method: &'a str, path: &'a str, body: Option<&'a Value>) -> Verdict {
        check(&Ask { role, method, path, server: "friday", body }, &|mode, cvar| mode == "zp5" && cvar == "zp_lighting")
    }
    fn ok(v: &Verdict) -> bool {
        matches!(v, Verdict::Allow(_))
    }

    #[test]
    fn nothing_unless_named() {
        for r in [Role::Moderator, Role::Admin] {
            for (m, p) in [
                ("GET", "/api/servers"),
                ("POST", "/api/servers/friday/delete"),
                ("POST", "/api/servers/friday/rebuild"),
                ("POST", "/api/servers/friday/join"),
                ("POST", "/api/servers/friday/rcon-password"),
                ("POST", "/api/servers/friday/rcon-password/show"),
                ("POST", "/api/servers/friday/webstatus/secret"),
                ("GET", "/api/servers/friday/webstatus"),
                ("GET", "/api/servers/friday/hltv"),
                ("POST", "/api/servers/friday/console/send"),
                ("POST", "/api/servers/select"),
                ("GET", "/api/app"),
                ("GET", "/api/app/phone"),
                ("POST", "/api/app/update/check"),
                ("GET", "/api/app/diagnostics"),
                ("POST", "/api/share"),
                ("GET", "/api/admins"),
                ("POST", "/api/admins/person"),
                ("POST", "/api/admins/settings"),
                ("POST", "/api/lists"),
                ("POST", "/api/setup/install"),
                ("POST", "/api/backup/export"),
                ("POST", "/api/mods/plugins/add"),
                ("DELETE", "/api/state"),
                ("GET", "/api/nothing"),
            ] {
                assert!(!ok(&ask(r, m, p, None)), "{r:?} {m} {p}");
            }
        }
    }

    #[test]
    fn only_their_server() {
        assert!(!ok(&ask(Role::Admin, "GET", "/api/servers/other/console", None)));
        assert!(!ok(&ask(Role::Admin, "POST", "/api/servers/other/restart", None)));
        assert!(ok(&ask(Role::Admin, "POST", "/api/servers/friday/restart", None)));
        assert!(ok(&ask(Role::Moderator, "GET", "/api/servers/friday/console", None)));
    }

    #[test]
    fn moderators() {
        let m = Role::Moderator;
        assert!(ok(&ask(m, "GET", "/api/state", None)));
        assert!(ok(&ask(m, "POST", "/api/admins/kick", Some(&json!({"id": "STEAM_0:1:1"})))));
        assert!(ok(&ask(m, "POST", "/api/admins/ban", Some(&json!({"id": "STEAM_0:1:1", "minutes": 60})))));
        assert!(!ok(&ask(m, "POST", "/api/admins/ban", Some(&json!({"id": "STEAM_0:1:1", "minutes": 0})))), "for good: admins only");
        assert!(!ok(&ask(m, "POST", "/api/admins/ban", Some(&json!({"id": "STEAM_0:1:1", "minutes": MOD_BAN_MINUTES + 1})))));
        assert!(ok(&ask(m, "POST", "/api/cmd", Some(&json!({"cmd": "gm_map", "args": ["de_dust2"]})))));
        assert!(!ok(&ask(m, "POST", "/api/cmd", Some(&json!({"cmd": "gm_set", "args": ["mp_timelimit", "5"]})))));
        assert!(!ok(&ask(m, "POST", "/api/servers/friday/restart", None)));
        assert!(!ok(&ask(m, "POST", "/api/servers/friday", Some(&json!({"name": "x"})))));
        assert!(!ok(&ask(m, "POST", "/api/gb/install", None)));
        assert!(!ok(&ask(m, "POST", "/api/control/invite", None)));
        assert!(!ok(&ask(m, "GET", "/api/extras", None)));
    }

    #[test]
    fn console() {
        let (a, m) = (Role::Admin, Role::Moderator);
        for ok in ["mp_timelimit 30", "changelevel de_inferno", "bot_add", "say \"hello there\"", "amx_ban bob 0 cheating"] {
            assert!(console_line(a, ok).is_ok(), "admin: {ok}");
        }
        for bad in ["\"exec\" x.cfg", "\"logaddress_add\" 1.2.3.4 27500", "\"alias\" x quit", "gm_fastdl http://x", "gm_ai_url 1.2.3.4:80", "\u{e9}exec x", "rcon_password x", "sv_password abc", "say hi; rcon_password x", "exec evil.cfg", "alias x quit", "amx_cvar sv_password x", "amx_rcon status", "amx_addadmin me abcdefghijklmnopqrstu", "sv_downloadurl http://x", "logaddress_add 1.2.3.4 1", "meta unload amxx", "amxx pause admin.amxx", "quit", "echo hi
quit", "SV_PASSWORD x", "sv_rcon_banpenalty 0", "amx_pluginsdisable x"] {
            assert!(console_line(a, bad).is_err(), "admin: {bad}");
        }
        assert!(console_line(m, "kick bob").is_ok() && console_line(m, "say hi").is_ok() && console_line(m, "changelevel de_dust2").is_ok());
        assert!(console_line(m, "banid 60 STEAM_0:1:1 kick").is_ok() && console_line(m, "amx_ban STEAM_0:1:2 10080 spam").is_ok());
        assert!(console_line(m, "amx_ban bob 60 spam").is_err() && console_line(m, "banid 60 #5").is_err(), "by SteamID only");
        assert_eq!(mod_target("amx_gag steam_0:0:7 60"), Some("STEAM_0:0:7".into()));
        assert!(secret_setting("amx_sql_pass") && !secret_setting("wc3_cham_passive"));
        assert!(console_line(m, "banid 0 STEAM_0:1:1").is_err(), "for good: admins only");
        assert!(console_line(m, "amx_ban bob 99999").is_err() && console_line(m, "amx_ban bob").is_err());
        assert!(console_line(m, "amx_gag STEAM_0:1:3 60 chat").is_ok() && console_line(m, "amx_gag STEAM_0:1:3 0").is_err(), "mutes until removed: admins only");
        assert!(console_line(a, "sv_cheats 1").is_ok() && console_line(m, "sv_cheats 1").is_err());
        assert!(console_line(m, "amx_ban \"#5 60 x\" 0 spam").is_err(), "quotes could hide a ban for good");
        assert!(console_line(a, "say \"hello there\"").is_ok(), "admins may quote what they say");
        assert!(console_line(m, "mp_timelimit 30").is_err() && console_line(m, "bot_add").is_err());
        assert!(ok(&ask(m, "POST", "/api/servers/friday/console/send", Some(&json!({"command": "kick bob"})))));
        assert!(!ok(&ask(a, "POST", "/api/servers/friday/console/send", Some(&json!({"command": "rcon_password x"})))));
        assert!(!ok(&ask(a, "POST", "/api/servers/other/console/send", Some(&json!({"command": "status"})))));
    }

    #[test]
    fn admins() {
        let a = Role::Admin;
        assert!(ok(&ask(a, "POST", "/api/cmd", Some(&json!({"cmd": "gm_set", "args": ["mp_timelimit", "5"]})))));
        assert!(!ok(&ask(a, "POST", "/api/cmd", Some(&json!({"cmd": "gm_crash"})))));
        assert!(ok(&ask(a, "POST", "/api/admins/ban", Some(&json!({"id": "STEAM_0:1:1", "minutes": 0})))));
        assert!(ok(&ask(a, "POST", "/api/gb/install", None)));
        assert!(ok(&ask(a, "POST", "/api/gb/install", Some(&json!({"mod": 1, "file": 2, "section": "maps"})))));
        assert!(!ok(&ask(a, "POST", "/api/gb/install", Some(&json!({"mod": 1, "file": 2, "section": "skins"})))), "the owner's own game");
        assert!(!ok(&ask(a, "POST", "/api/gb/uninstall", Some(&json!({"mod": 1, "section": "sounds"})))));
        assert!(!ok(&check(&Ask { role: a, method: "POST", path: "/api/servers/select", server: "select", body: Some(&json!({})) }, &|_, _| false)));
        assert!(!ok(&ask(Role::Moderator, "POST", "/api/community/gag", Some(&json!({"id": "x", "minutes": 0})))));
        assert!(ok(&ask(Role::Moderator, "POST", "/api/community/gag", Some(&json!({"id": "x", "minutes": 60})))));
        assert!(ok(&ask(a, "POST", "/api/control/invite", None)));
        // the owner's settings never get through: only theirs are kept
        let body = json!({"name": "New", "maxplayers": 16, "visibility": "listed", "password": "x", "port": 27099, "listen_ip": "1.2.3.4", "extra_args": "+exec evil.cfg", "public_address": "a.b", "ls_listed": true});
        assert_eq!(ask(a, "POST", "/api/servers/friday", Some(&body)), Verdict::Allow(Some(json!({"name": "New", "maxplayers": 16}))));
        // mode settings: only what the game reports, never passwords or rcon
        assert!(ok(&ask(a, "POST", "/api/app/mode-settings", Some(&json!({"mode": "zp5", "cvar": "zp_lighting", "value": "a"})))));
        assert!(!ok(&ask(a, "POST", "/api/app/mode-settings", Some(&json!({"mode": "zp5", "cvar": "rcon_password", "value": "a"})))));
        assert!(!ok(&ask(a, "POST", "/api/app/mode-settings", Some(&json!({"mode": "zp5", "cvar": "servercfgfile", "value": "x.cfg"})))));
        assert!(!ok(&ask(a, "POST", "/api/app/mode-settings", Some(&json!({"mode": "zp5", "cvar": "mapchangecfgfile", "value": "x.cfg"})))));
    }
}
