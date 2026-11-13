//! Every route the app window can call, and what an agent may do with each: the same routes
//! and the same handlers the person's clicks reach, under the connector's rules.
//!
//! - GET: reading (works with changes off). Answers are cleaned of secrets (`redact`).
//! - POST: a change (needs "Allow changes"); some ask the person first (deleting, removing,
//!   kicking and banning, stopping or restarting what people play on, closing the game,
//!   anything public or outward), some are the person's alone (`USER_ONLY`): refused even with
//!   changes allowed.
//! - Anything not in `ROUTES` is refused: a route added later stays out until it's listed here
//!   (default deny).

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Policy {
    Read,
    Change,
    /// a change the person is asked about first, with why
    Ask(String),
    /// never for an agent: the person's call
    UserOnly(String),
}

/// (method, path pattern) of every route on the app window's listener (`:x` is one part).
pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/safety/files"),
    ("GET", "/api/safety/version"),
    ("GET", "/api/safety/changed"),
    ("POST", "/api/safety/restore"),
    ("POST", "/api/safety/defaults"),
    ("POST", "/api/safety/undo"),
    ("POST", "/api/safety/clear"),
    ("GET", "/api/admins"),
    ("POST", "/api/admins/ban"),
    ("GET", "/api/admins/bans"),
    ("POST", "/api/admins/kick"),
    ("POST", "/api/admins/person"),
    ("POST", "/api/admins/remove"),
    ("POST", "/api/admins/settings"),
    ("POST", "/api/admins/unban"),
    ("GET", "/api/app"),
    ("GET", "/api/app/diagnostics"),
    ("GET", "/api/app/direct"),
    ("POST", "/api/app/direct"),
    ("POST", "/api/app/game"),
    ("POST", "/api/app/game-folder"),
    ("POST", "/api/app/game-folder/remove"),
    ("GET", "/api/app/lan"),
    ("POST", "/api/app/launch"),
    ("GET", "/api/app/mode-settings"),
    ("POST", "/api/app/mode-settings"),
    ("POST", "/api/app/open-folder"),
    ("POST", "/api/app/open-url"),
    ("GET", "/api/app/phone"),
    ("POST", "/api/app/phone"),
    ("POST", "/api/app/phone/reset"),
    ("GET", "/api/app/prefs"),
    ("POST", "/api/app/prefs"),
    ("POST", "/api/app/remove-everything"),
    ("POST", "/api/app/remove-everything/quit"),
    ("POST", "/api/app/restart"),
    ("GET", "/api/app/saved-settings"),
    ("POST", "/api/app/saved-settings"),
    ("POST", "/api/app/saved-settings/addon"),
    ("POST", "/api/app/saved-settings/bots"),
    ("POST", "/api/app/saved-settings/reset"),
    ("GET", "/api/app/servers-folder"),
    ("POST", "/api/app/servers-folder"),
    ("GET", "/api/app/start-options"),
    ("GET", "/api/app/storage"),
    ("POST", "/api/app/storage/clear"),
    ("POST", "/api/app/update/check"),
    ("POST", "/api/app/update/install"),
    ("GET", "/api/app/vlans"),
    ("POST", "/api/backup/app-prefs"),
    ("GET", "/api/backup/auto"),
    ("POST", "/api/backup/estimate"),
    ("POST", "/api/backup/export"),
    ("GET", "/api/backup/info"),
    ("POST", "/api/backup/open"),
    ("POST", "/api/backup/restore"),
    ("POST", "/api/botchat/reply"),
    ("POST", "/api/browser/connect"),
    ("POST", "/api/browser/favorite"),
    ("GET", "/api/browser/list"),
    ("GET", "/api/browser/server"),
    ("POST", "/api/cmd"),
    ("GET", "/api/community"),
    ("GET", "/api/community/chat"),
    ("POST", "/api/community/gag"),
    ("GET", "/api/community/reports"),
    ("POST", "/api/community/reports"),
    ("POST", "/api/community/settings"),
    ("GET", "/api/community/stats"),
    ("POST", "/api/community/stats/reset"),
    ("POST", "/api/community/ungag"),
    ("POST", "/api/control/accept"),
    ("POST", "/api/control/invite"),
    ("POST", "/api/control/invite/:iid/cancel"),
    ("POST", "/api/control/join"),
    ("GET", "/api/control/me"),
    ("POST", "/api/control/member/:mid"),
    ("POST", "/api/control/member/:mid/remove"),
    ("POST", "/api/control/open-port"),
    ("POST", "/api/control/port"),
    ("POST", "/api/control/redeem"),
    ("GET", "/api/control/server/:id"),
    ("POST", "/api/control/server/:id/invite"),
    ("POST", "/api/control/server/:id/undo"),
    ("POST", "/api/control/shared/:id/join"),
    ("POST", "/api/control/shared/:id/leave"),
    ("GET", "/api/control/whoami"),
    ("GET", "/api/extras"),
    ("POST", "/api/extras/export"),
    ("POST", "/api/extras/rotation"),
    ("POST", "/api/extras/schedule"),
    ("POST", "/api/extras/settings"),
    ("POST", "/api/gb/add-file"),
    ("GET", "/api/gb/browse"),
    ("POST", "/api/gb/install"),
    ("GET", "/api/gb/installed"),
    ("GET", "/api/gb/jobs"),
    ("GET", "/api/gb/mod/:id"),
    ("POST", "/api/gb/uninstall"),
    ("GET", "/api/health"),
    ("GET", "/api/lists"),
    ("POST", "/api/lists"),
    ("GET", "/api/mapcopy"),
    ("POST", "/api/mapcopy"),
    ("GET", "/api/mapcopy/local"),
    ("GET", "/api/mapcopy/servers"),
    ("POST", "/api/mapcopy/stop"),
    ("POST", "/api/mapflag"),
    ("GET", "/api/maps"),
    ("GET", "/api/mcp"),
    ("POST", "/api/mcp"),
    ("POST", "/api/mcp/log/clear"),
    ("POST", "/api/mcp/token"),
    ("GET", "/api/modes/:id/maps"),
    ("GET", "/api/modevote"),
    ("POST", "/api/modevote"),
    ("GET", "/api/mods/plugins"),
    ("POST", "/api/mods/plugins/add"),
    ("POST", "/api/mods/plugins/remove"),
    ("GET", "/api/navs/learn"),
    ("POST", "/api/navs/learn"),
    ("POST", "/api/navs/learn/stop"),
    ("POST", "/api/navs/send"),
    ("GET", "/api/scrim"),
    ("POST", "/api/scrim/action"),
    ("POST", "/api/scrim/settings"),
    ("GET", "/api/servercontent"),
    ("POST", "/api/servercontent/set"),
    ("GET", "/api/servers"),
    ("POST", "/api/servers"),
    ("POST", "/api/servers/:id"),
    ("POST", "/api/servers/:id/:action"),
    ("GET", "/api/servers/:id/console"),
    ("POST", "/api/servers/:id/console/hints"),
    ("POST", "/api/servers/:id/console/send"),
    ("GET", "/api/servers/:id/hltv"),
    ("POST", "/api/servers/:id/hltv"),
    ("POST", "/api/servers/:id/hltv-folder"),
    ("POST", "/api/servers/:id/hltv/:action"),
    ("GET", "/api/servers/:id/hltv/demos/:name"),
    ("GET", "/api/servers/:id/modes"),
    ("POST", "/api/servers/:id/rcon-password"),
    ("POST", "/api/servers/:id/rcon-password/show"),
    ("GET", "/api/servers/:id/security"),
    ("POST", "/api/servers/:id/security/fix"),
    ("GET", "/api/servers/:id/voice"),
    ("POST", "/api/servers/:id/voice"),
    ("GET", "/api/servers/:id/webstatus"),
    ("POST", "/api/servers/:id/webstatus"),
    ("GET", "/api/servers/:id/webstatus/health"),
    ("POST", "/api/servers/:id/webstatus/secret"),
    ("POST", "/api/servers/import"),
    ("POST", "/api/servers/pick-folder"),
    ("POST", "/api/servers/pick-key"),
    ("POST", "/api/servers/remote"),
    ("POST", "/api/servers/remote/probe"),
    ("POST", "/api/servers/select"),
    ("POST", "/api/setup/install"),
    ("GET", "/api/setup/job"),
    ("GET", "/api/setup/running"),
    ("GET", "/api/setup/scan"),
    ("POST", "/api/setup/uninstall"),
    ("GET", "/api/share"),
    ("POST", "/api/share"),
    ("GET", "/api/smartchat"),
    ("POST", "/api/smartchat/download"),
    ("POST", "/api/smartchat/remove"),
    ("POST", "/api/smartchat/set"),
    ("POST", "/api/smartchat/try"),
    ("POST", "/api/smartchat/try/start"),
    ("POST", "/api/smartchat/try/stop"),
    ("GET", "/api/sounds"),
    ("POST", "/api/sounds/add"),
    ("POST", "/api/sounds/addon"),
    ("POST", "/api/sounds/announcer"),
    ("GET", "/api/sounds/file"),
    ("POST", "/api/sounds/pack"),
    ("GET", "/api/sounds/packs"),
    ("POST", "/api/sounds/track"),
    ("GET", "/api/state"),
    ("POST", "/api/tools/assets/add-file"),
    ("POST", "/api/tools/assets/export"),
    ("GET", "/api/tools/assets/model"),
    ("GET", "/api/tools/assets/model/anim"),
    ("GET", "/api/tools/assets/model/info"),
    ("POST", "/api/tools/assets/model/remove"),
    ("GET", "/api/tools/assets/model/tex"),
    ("GET", "/api/tools/assets/models"),
    ("POST", "/api/tools/assets/texture/pick"),
    ("POST", "/api/tools/assets/texture/preview"),
    ("POST", "/api/tools/assets/texture/save"),
    ("GET", "/api/tools/assets/usage"),
    ("GET", "/api/tools/assets/wad/:folder/:wad"),
    ("GET", "/api/tools/assets/wad/:folder/:wad/tex/:file"),
    ("GET", "/api/tools/assets/wads"),
    ("GET", "/api/tools/callouts/:name"),
    ("POST", "/api/tools/callouts/:name"),
    ("GET", "/api/tools/callouts/:name/suggest"),
    ("GET", "/api/tools/community/browse"),
    ("GET", "/api/tools/community/item/:model/:id"),
    ("GET", "/api/tools/community/mine"),
    ("GET", "/api/tools/community/publish/:map"),
    ("POST", "/api/tools/community/publish/:map"),
    ("POST", "/api/tools/community/register"),
    ("POST", "/api/tools/community/report"),
    ("POST", "/api/tools/community/reveal"),
    ("POST", "/api/tools/community/try"),
    ("POST", "/api/tools/compile/cancel"),
    ("GET", "/api/tools/compile/progress"),
    ("POST", "/api/tools/compile/setup"),
    ("POST", "/api/tools/compile/start"),
    ("GET", "/api/tools/compile/status"),
    ("GET", "/api/tools/demos"),
    ("POST", "/api/tools/demos/delete"),
    ("GET", "/api/tools/demos/detail"),
    ("POST", "/api/tools/demos/play"),
    ("POST", "/api/tools/demos/rename"),
    ("POST", "/api/tools/demos/replay"),
    ("GET", "/api/tools/doctor/:name"),
    ("POST", "/api/tools/doctor/:name/copy"),
    ("POST", "/api/tools/doctor/:name/res"),
    ("POST", "/api/tools/editor/autosave"),
    ("GET", "/api/tools/editor/checks"),
    ("POST", "/api/tools/editor/cmd"),
    ("POST", "/api/tools/editor/cmd2"),
    ("GET", "/api/tools/editor/components"),
    ("POST", "/api/tools/editor/components/load-pack"),
    ("GET", "/api/tools/editor/components/pack"),
    ("POST", "/api/tools/editor/components/save-pack"),
    ("POST", "/api/tools/editor/copy"),
    ("POST", "/api/tools/editor/discard-recovery"),
    ("GET", "/api/tools/editor/doc"),
    ("POST", "/api/tools/editor/export-map"),
    ("GET", "/api/tools/editor/fgd"),
    ("POST", "/api/tools/editor/fgd/clear"),
    ("POST", "/api/tools/editor/fgd/pick"),
    ("GET", "/api/tools/editor/history"),
    ("POST", "/api/tools/editor/history/jump"),
    ("POST", "/api/tools/editor/import-map"),
    ("POST", "/api/tools/editor/import-other"),
    ("GET", "/api/tools/editor/meshes"),
    ("POST", "/api/tools/editor/new"),
    ("POST", "/api/tools/editor/open"),
    ("POST", "/api/tools/editor/open-recent"),
    ("GET", "/api/tools/editor/prefab/:id/thumb.png"),
    ("GET", "/api/tools/editor/prefabs"),
    ("POST", "/api/tools/editor/prefabs/delete"),
    ("POST", "/api/tools/editor/prefabs/edit"),
    ("POST", "/api/tools/editor/prefabs/export"),
    ("POST", "/api/tools/editor/prefabs/import"),
    ("POST", "/api/tools/editor/prefabs/save"),
    ("GET", "/api/tools/editor/project"),
    ("POST", "/api/tools/editor/project/action"),
    ("GET", "/api/tools/editor/recent"),
    ("POST", "/api/tools/editor/recover"),
    ("GET", "/api/tools/editor/recovery"),
    ("POST", "/api/tools/editor/redo"),
    ("POST", "/api/tools/editor/save"),
    ("POST", "/api/tools/editor/settings"),
    ("GET", "/api/tools/editor/sound"),
    ("GET", "/api/tools/editor/state"),
    ("GET", "/api/tools/editor/tex/:file"),
    ("GET", "/api/tools/editor/textures"),
    ("POST", "/api/tools/editor/undo"),
    ("GET", "/api/tools/editor/wads"),
    ("POST", "/api/tools/health/cancel"),
    ("GET", "/api/tools/health/options"),
    ("GET", "/api/tools/health/progress"),
    ("GET", "/api/tools/health/report/:id"),
    ("GET", "/api/tools/health/reports"),
    ("POST", "/api/tools/health/start"),
    ("GET", "/api/tools/map/:name/entities"),
    ("GET", "/api/tools/map/:name/info"),
    ("GET", "/api/tools/map/:name/know"),
    ("GET", "/api/tools/map/:name/lightmap.png"),
    ("GET", "/api/tools/map/:name/look"),
    ("GET", "/api/tools/map/:name/mesh"),
    ("GET", "/api/tools/map/:name/nav"),
    ("GET", "/api/tools/map/:name/probe"),
    ("GET", "/api/tools/map/:name/sky/:side"),
    ("GET", "/api/tools/map/:name/tex/:file"),
    ("GET", "/api/tools/maps"),
    ("GET", "/api/tools/navfix/:name"),
    ("POST", "/api/tools/navfix/:name/edit"),
    ("POST", "/api/tools/navfix/:name/rebuild"),
    ("POST", "/api/tools/navfix/:name/redo"),
    ("POST", "/api/tools/navfix/:name/reload"),
    ("POST", "/api/tools/navfix/:name/save"),
    ("POST", "/api/tools/navfix/:name/undo"),
    ("GET", "/api/tools/overview/:name"),
    ("GET", "/api/tools/overview/:name/current.png"),
    ("POST", "/api/tools/overview/:name/install"),
    ("GET", "/api/tools/overview/:name/preview.png"),
    ("POST", "/api/tools/overview/:name/restore"),
    ("GET", "/api/tools/creatures"),
    ("POST", "/api/tools/creatures/check"),
    ("GET", "/api/tools/creatures/live"),
    ("GET", "/api/tools/creatures/model"),
    ("GET", "/api/tools/creatures/models"),
    ("POST", "/api/tools/creatures/reload"),
    ("POST", "/api/tools/creatures/save"),
    ("GET", "/api/tools/creatures/sounds"),
    ("POST", "/api/tools/creatures/spawn"),
    ("GET", "/api/tools/creatures/waves"),
    ("POST", "/api/tools/creatures/waves/save"),
    ("GET", "/api/tools/people"),
    ("POST", "/api/tools/people/check"),
    ("POST", "/api/tools/people/export"),
    ("POST", "/api/tools/people/import"),
    ("POST", "/api/tools/people/restore"),
    ("POST", "/api/tools/people/save"),
    ("POST", "/api/tools/people/shipped"),
    ("GET", "/api/tools/playercfg"),
    ("POST", "/api/tools/playercfg/preview"),
    ("POST", "/api/tools/playercfg/remove"),
    ("POST", "/api/tools/playercfg/restore"),
    ("POST", "/api/tools/playercfg/save"),
    ("GET", "/api/tools/plugins"),
    ("POST", "/api/tools/plugins/compile"),
    ("POST", "/api/tools/plugins/delete"),
    ("GET", "/api/tools/plugins/file"),
    ("POST", "/api/tools/plugins/file"),
    ("GET", "/api/tools/plugins/log"),
    ("POST", "/api/tools/plugins/try"),
    ("POST", "/api/tools/plugins/untry"),
    ("GET", "/api/tools/plugins/words"),
    ("GET", "/api/tools/recording/:id"),
    ("GET", "/api/tools/recording/:id/trace"),
    ("GET", "/api/tools/recordings"),
    ("POST", "/api/tools/recordings/open"),
    ("POST", "/api/tools/screenshot"),
    ("GET", "/api/tools/spray"),
    ("POST", "/api/tools/spray/install"),
    ("POST", "/api/tools/spray/pick"),
    ("POST", "/api/tools/spray/preview"),
    ("POST", "/api/tools/spray/restore"),
    ("POST", "/api/tools/studio/compile"),
    ("POST", "/api/tools/studio/debug/command"),
    ("GET", "/api/tools/studio/debug/events"),
    ("POST", "/api/tools/studio/generate"),
    ("POST", "/api/tools/studio/source"),
    ("GET", "/api/tools/studio/graph/:id"),
    ("POST", "/api/tools/studio/graph/:id"),
    ("POST", "/api/tools/studio/graph/:id/delete"),
    ("GET", "/api/tools/studio/graphs"),
    ("POST", "/api/tools/studio/playtest"),
    ("GET", "/api/tools/studio/status"),
    ("POST", "/api/tools/studio/uninstall/:id"),
    ("POST", "/api/tools/studio/validate"),
    ("POST", "/api/voice/:action"),
    ("GET", "/api/welcome"),
    ("POST", "/api/welcome"),
    ("GET", "/thumb/:file"),
];

/// What the person keeps for themselves, in the words agents get back.
pub const USER_ONLY_LIST: &[&str] = &[
    "The AI agents settings themselves (turning agents on, Allow changes, the token, the port).",
    "App updates, uninstalling Loner-Strike, and setting up, repairing or removing the game's setup.",
    "Phone access, shared control (inviting friends, joining theirs, opening ports) and sharing links.",
    "Showing or changing server console passwords and website-status secrets.",
    "A server's extra start options, putting back all of Loner-Strike's default files, and clearing old backups.",
];

/// The parts of a path, matched against a pattern's (":x" matches any one part, but never an
/// empty one, "." or "..").
pub fn matches(pattern: &str, path: &str) -> bool {
    let (p, q): (Vec<&str>, Vec<&str>) = (pattern.split('/').collect(), path.split('/').collect());
    p.len() == q.len() && p.iter().zip(&q).all(|(a, b)| a.starts_with(':') && !b.is_empty() && *b != "." && *b != ".." || a == b)
}

/// A path exactly as it's checked and sent: "/" and plain ASCII parts, nothing a router or
/// the game might read differently from how it's classified here. Refused: anything not
/// starting with "/", empty parts ("//", a trailing "/"), "." or ".." parts, any "%"
/// (so no %2e, %2f, %5c in any case), backslashes, "?" and "#" (a query goes separately,
/// only where the route takes one), spaces, control and non-ASCII characters.
pub fn canonical(path: &str) -> Result<&str, String> {
    let bad = || Err("path is a route's plain path, e.g. /api/servers/<id>/start (no query, %, \\, . or .. parts).".to_string());
    if !path.starts_with('/') || path.len() > 512 || !path.chars().all(|c| c.is_ascii_graphic()) || path.contains(['%', '\\', '?', '#']) {
        return bad();
    }
    if path[1..].split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return bad();
    }
    Ok(path)
}

/// Whether a query string may go with this request: GETs, and the one POST that takes one.
pub fn query_allowed(method: &str, route: &str) -> bool {
    method == "GET" || route == "/api/tools/screenshot"
}

/// What a raw console line may do, for an agent: the same rules as console_send
/// (mcp::debug::console_policy), the one classifier for every route that hands the game a
/// command, a setting's name, or a start option.
pub fn console_line(line: &str) -> Policy {
    use super::debug::{console_policy, Console};
    // (with changes on, so a refusal means never, not "turn changes on")
    match console_policy(line, true) {
        Err(why) => user_only(&why),
        Ok(Console::Ask(what)) => Policy::Ask(format!("{what} (console: {})", line.trim())),
        Ok(Console::Change) => Policy::Change,
        Ok(Console::Read) => Policy::Read,
    }
}

/// The listed route a request is for.
pub fn route_for(method: &str, path: &str) -> Option<&'static str> {
    // (a literal pattern beats one with a :part, e.g. /api/servers/import over /api/servers/:id)
    let mut hits: Vec<&'static str> = ROUTES.iter().filter(|(m, p)| *m == method && matches(p, path)).map(|(_, p)| *p).collect();
    hits.sort_by_key(|p| p.matches(':').count());
    hits.first().copied()
}

fn user_only(why: &str) -> Policy {
    Policy::UserOnly(format!("{why} That's the person's call: ask them to do it in Loner-Strike."))
}

/// What an agent may do with one request.
pub fn policy(method: &str, path: &str, body: &Value) -> Policy {
    let Ok(path) = canonical(path) else {
        return Policy::UserOnly("That path isn't a plain route path (no query, %, \\, . or .. parts).".into());
    };
    let Some(route) = route_for(method, path) else {
        return Policy::UserOnly("That isn't one of the app's routes agents can use (app_routes lists them).".into());
    };
    let parts: Vec<&str> = path.split('/').collect();
    let b = |k: &str| body.get(k);
    // the person's alone, whatever the method
    if route.starts_with("/api/mcp") {
        return user_only("The AI agents settings are off limits to agents.");
    }
    if route.starts_with("/api/app/update") || route.starts_with("/api/app/remove-everything") || route == "/api/app/restart" {
        return user_only("Updating, restarting or removing Loner-Strike is off limits to agents.");
    }
    if route.starts_with("/api/app/phone") || route.starts_with("/api/control/") && method == "POST" || route == "/api/share" && method == "POST" {
        return user_only("Phone access, shared control and sharing links are off limits to agents.");
    }
    if route.ends_with("/rcon-password") || route.ends_with("/rcon-password/show") || route.ends_with("/webstatus/secret") {
        return user_only("Console passwords and website-status secrets are off limits to agents.");
    }
    // the safety net: putting every default back or throwing old versions away undoes many
    // changes at once (one file, or the last save, is asked about below)
    if route == "/api/safety/defaults" || route == "/api/safety/clear" {
        return user_only("Putting back all of Loner-Strike's defaults or clearing old backups is off limits to agents.");
    }
    if route == "/api/setup/install" {
        let comps = b("components").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        if b("repair").and_then(Value::as_bool) == Some(true) || comps == 0 {
            return user_only("Setting up or repairing the game's whole setup is off limits to agents (installing named add-ons is fine).");
        }
        return Policy::Ask(format!("install {comps} add-on(s) into the game"));
    }
    if route == "/api/setup/uninstall" {
        let comps = b("components").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        if comps == 0 {
            return user_only("Removing Loner-Strike from the game is off limits to agents (removing named add-ons is fine).");
        }
        return Policy::Ask(format!("remove {comps} add-on(s) from the game"));
    }
    if method == "GET" {
        return Policy::Read;
    }
    // a raw console line, a setting written into a config the game runs, start options:
    // through the console's own rules (never weaker than console_send)
    if route == "/api/servers/:id/console/send" {
        return console_line(b("command").and_then(Value::as_str).unwrap_or(""));
    }
    if route == "/api/app/mode-settings" {
        let cvar = b("cvar").and_then(Value::as_str).unwrap_or("");
        let value = b("value").and_then(Value::as_str).unwrap_or("");
        if cvar.is_empty() || cvar.contains(char::is_whitespace) {
            return Policy::UserOnly("That isn't a setting's name.".into());
        }
        // (the mode's config is run as console lines, so "cvar value" is one)
        return match console_line(&format!("{cvar} {value}")) {
            Policy::Read => Policy::Change,
            p => p,
        };
    }
    if route == "/api/servers/:id" || route == "/api/servers" {
        // start options can run anything on the machine (the phone can't set them either)
        if b("extra_args").is_some_and(|v| !v.is_null()) {
            return user_only("A server's extra start options are off limits to agents.");
        }
        if b("password").is_some_and(|v| !v.is_null()) && route == "/api/servers/:id" {
            return Policy::Ask("change the server's join password".into());
        }
        if b("visibility").and_then(Value::as_str).is_some_and(|v| v == "listed") || b("ls_listed").and_then(Value::as_bool) == Some(true) {
            return Policy::Ask("make the server public / list it".into());
        }
    }
    // outward or public: asked every time
    let outward = [
        ("/api/app/direct", "change how friends connect to your game (direct connect / port forwarding)"),
        ("/api/app/open-url", "open a web page on your computer"),
        ("/api/app/open-folder", "open a folder on your computer"),
        ("/api/servers/:id/webstatus", "change the server's public website status / Discord posting"),
        ("/api/tools/community/register", "register a creation on the community list (public)"),
        ("/api/tools/community/report", "report a community creation to the Loner-Strike moderators"),
        ("/api/tools/community/publish/:map", "build a package of this map for publishing"),
        ("/api/tools/community/try", "download and install something from GameBanana"),
        ("/api/gb/install", "download and install something from GameBanana"),
        ("/api/smartchat/download", "download a chat model (large)"),
        ("/api/browser/connect", "start the game and join a server on the internet"),
        ("/api/browser/favorite", "change your server favourites"),
        ("/api/servers/remote", "add a server on another machine (SSH)"),
        ("/api/servers/remote/probe", "connect to another machine over SSH"),
        ("/api/navs/send", "send a nav to your servers"),
        ("/api/backup/restore", "put a backup back over your current settings"),
        ("/api/backup/app-prefs", "put back the app's preferences from a backup"),
        ("/api/tools/demos/play", "start the game to watch a demo"),
        ("/api/tools/studio/playtest", "build a Studio mode with the debugger into your game"),
        ("/api/safety/restore", "put an earlier version of a game file back"),
        ("/api/safety/undo", "undo the last change to game files"),
    ];
    if let Some((_, what)) = outward.iter().find(|(p, _)| *p == route) {
        return Policy::Ask(what.to_string());
    }
    // what the typed tools ask about (overwriting, replacing unsaved work, the game): asked
    // here too, so app_call is never the way round (test typed_tools_are_never_stronger_than_app_call)
    let overwrites = [
        ("/api/tools/editor/new", "start a new map in the editor (unsaved changes are lost)"),
        ("/api/tools/editor/open", "open another map in the editor (unsaved changes are lost)"),
        ("/api/tools/editor/import-map", "import a map into the editor (unsaved changes are lost)"),
        ("/api/tools/editor/import-other", "import into the editor (unsaved changes are lost)"),
        ("/api/tools/editor/save", "save the editor's map (overwrites its file)"),
        ("/api/tools/compile/setup", "download and set up the map compilers"),
        ("/api/tools/navfix/:name/save", "save the map's nav (overwrites the bots' nav)"),
        ("/api/tools/studio/graph/:id", "save a Game Mode Studio graph (overwrites it)"),
        ("/api/tools/studio/uninstall/:id", "take a Studio mode out of the game"),
        ("/api/tools/plugins/file", "save a plugin's source file (overwrites it)"),
        ("/api/tools/plugins/try", "put a plugin into your game to try"),
        ("/api/tools/editor/components/save-pack", "save a map's components (overwrites them)"),
    ];
    if let Some((_, what)) = overwrites.iter().find(|(p, _)| *p == route) {
        return Policy::Ask(what.to_string());
    }
    if route == "/api/tools/compile/start" && b("install").and_then(Value::as_bool) == Some(true) {
        return Policy::Ask("compile a map and install it into the game".into());
    }
    if route == "/api/app/launch" {
        return if b("restart").and_then(Value::as_bool) == Some(true) { Policy::Ask("start the game (closing it first if it's open)".into()) } else { Policy::Ask("start the game".into()) };
    }
    if route == "/api/servers" {
        return Policy::Change; // a new server
    }
    if route == "/api/servers/:id/:action" {
        let action = parts.last().copied().unwrap_or("");
        return match action {
            "start" | "join" if b("restart").and_then(Value::as_bool) != Some(true) => Policy::Change,
            "delete" => Policy::Ask("delete this server and its files".into()),
            "rebuild" | "retry" => Policy::Ask(format!("{action} this server from scratch")),
            "stop" | "restart" | "update" | "apply-updates" | "join" => Policy::Ask(format!("{action} this server (people playing on it would be dropped)")),
            other => Policy::Ask(format!("{other} this server")),
        };
    }
    if route.starts_with("/api/servers/:id/hltv/") {
        let action = parts.last().copied().unwrap_or("");
        if action.contains("delete") {
            return Policy::Ask("delete a recorded demo".into());
        }
    }
    if route == "/api/gb/uninstall" || route == "/api/mods/plugins/remove" || route == "/api/smartchat/remove" {
        return Policy::Ask("remove something installed".into());
    }
    if route == "/api/sounds/pack" && b("action").and_then(Value::as_str) == Some("remove") {
        return Policy::Ask("remove a sound pack".into());
    }
    if route == "/api/admins/kick" || route == "/api/admins/ban" {
        return Policy::Ask(format!("{} a player", parts.last().unwrap_or(&"")));
    }
    if route == "/api/cmd" {
        let cmd = b("cmd").and_then(Value::as_str).unwrap_or("");
        let args: Vec<&str> = b("args").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if cmd == "gm_player" && args.iter().any(|a| ["kick", "ban", "slay"].contains(a)) {
            return Policy::Ask(format!("{} a player", args.iter().find(|a| ["kick", "ban", "slay"].contains(*a)).unwrap()));
        }
        if ["gm_map", "gm_mode", "gm_restartmap", "gm_votemode"].contains(&cmd) {
            return Policy::Ask(format!("change the map or mode ({cmd}; people playing would be moved)"));
        }
        if cmd == "gm_presetdel" {
            return Policy::Ask("delete a preset".into());
        }
        return Policy::Change;
    }
    let last = parts.last().copied().unwrap_or("");
    let risky = ["delete", "remove", "uninstall", "clear", "reset", "restore", "rebuild", "untry", "unban"];
    if risky.iter().any(|w| last.contains(w)) && last != "unban" {
        return Policy::Ask(format!("{} ({route})", last.replace('-', " ")));
    }
    if route.contains("/delete") || route.contains("/remove") {
        return Policy::Ask(format!("delete or remove something ({route})"));
    }
    Policy::Change
}

/// Keys whose values are never shown to agents.
fn secret_key(k: &str) -> bool {
    let k = k.to_ascii_lowercase();
    ["password", "passwd", "rcon", "token", "secret", "apikey", "api_key", "webhook", "cookie", "privatekey", "private_key", "invite", "steamid", "steam_id", "qr"]
        .iter()
        .any(|w| k.contains(w))
        && !k.starts_with("has")
        && !k.ends_with("set")
}

/// Secrets out of an answer: values under secret-sounding keys (strings and numbers; flags
/// like hasPassword stay), and passwords in console text.
pub fn redact(v: &mut Value) {
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                if secret_key(k) && (x.is_string() || x.is_number()) && x.as_str() != Some("") {
                    *x = Value::String("(hidden)".into());
                } else {
                    redact(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact),
        Value::String(s) => {
            let l = s.to_ascii_lowercase();
            if l.contains("password") || l.contains("rcon") || l.contains("token") {
                *s = s.lines().map(crate::control::common::hide_passwords).collect::<Vec<_>>().join("\n");
            }
        }
        _ => {}
    }
}

/// What each part of the app is, for app_routes.
pub const AREAS: &[(&str, &str)] = &[
    ("/api/state", "the game being played or the server named by target: map, mode, players, settings"),
    ("/api/cmd", "the web panel's controls: {cmd, args} with gm_mode, gm_map, gm_bots, gm_set, gm_addon, gm_match, gm_player, gm_preset, gm_restartmap, gm_announce, gm_votemode..."),
    ("/api/maps", "the game's maps"),
    ("/api/modes", "maps for a mode"),
    ("/api/mapflag", "favourite / hidden maps"),
    ("/api/health", "Loner-Strike's setup check in the game"),
    ("/api/app/launch", "start the game: {map, mode, bots, restart, menu}"),
    ("/api/app/mode-settings", "a mode's saved settings"),
    ("/api/app/saved-settings", "saved settings for the next start"),
    ("/api/app", "the app: version, game, preferences, storage, LAN addresses"),
    ("/api/admins", "admins, VIPs, kicks and bans"),
    ("/api/lists", "lists shared between servers (bans, admins, maps)"),
    ("/api/servers", "dedicated servers: list, make, settings, start/stop, console, HLTV, voice, security, modes"),
    ("/api/extras", "server extras: map rotation, daily restart schedule, settings"),
    ("/api/modevote", "the mode vote"),
    ("/api/scrim", "match mode (scrims)"),
    ("/api/community", "chat filter, gags, player reports, stats"),
    ("/api/servercontent", "what a server sends players (downloads)"),
    ("/api/welcome", "the welcome message"),
    ("/api/sounds", "sounds, round music, announcer"),
    ("/api/voice", "voice chat add-on"),
    ("/api/smartchat", "smarter bot chat (local model)"),
    ("/api/botchat", "bot chat replies"),
    ("/api/gb", "GameBanana: browse, details, install jobs, installed, uninstall"),
    ("/api/mods/plugins", "the person's own AMX Mod X plugins in the game"),
    ("/api/navs", "teaching bots maps (nav learning) and sending navs to servers"),
    ("/api/mapcopy", "copying maps between the game and servers"),
    ("/api/browser", "the server browser"),
    ("/api/backup", "backups (export uses a save dialog; restore asks)"),
    ("/api/setup", "setup: scan, job progress; installing or removing named add-ons asks"),
    ("/api/share", "sharing (reading only)"),
    ("/api/control", "shared control (reading only)"),
    ("/api/tools", "Tools: maps, viewer data, editor, nav fixer, doctor, health check, replay, studios, compile, radar, demos, community, people, config"),
    ("/thumb", "map pictures"),
    ("/api/mcp", "the AI agents settings (the person's alone)"),
    ("/api/safety", "the safety net: every game file the app changed, its earlier versions, compare, put back, undo"),
];

pub fn area_of(path: &str) -> &'static str {
    AREAS.iter().filter(|(p, _)| path.starts_with(p)).max_by_key(|(p, _)| p.len()).map(|(_, d)| *d).unwrap_or("")
}
