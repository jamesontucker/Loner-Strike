//! Suggestions for the server console as you type: every command and setting the running
//! server knows, asked from the server itself (so it's right for 1.6 and CZ and for
//! whichever modes and plugins are loaded), with the syntax where something describes it.
//!
//! - `cmdlist`: the engine's and the game's commands (names cut off at 16 characters)
//! - `cvarlist`: every setting (its values are printed as numbers, so a setting's value is
//!   asked for on its own when it's wanted: `value`)
//! - `amxx cmds` / `amxx cvars`: plugin commands and settings, full names and their plugin
//! - `amx_help`: syntax and a line for the admin commands, a page at a time
//!
//! Kept per server for a few minutes, and dropped when the server is started again.

use crate::rcon::Rcon;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Item {
    /// the command or setting
    pub n: String,
    /// "cmd" or "cvar"
    pub k: &'static str,
    /// what it takes, like `<map>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// what it does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<String>,
    /// the plugin it comes from (none: the engine or the game)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<String>,
}

/// How long a server's list is kept (plugins only change with a restart or a map change)
const KEEP: Duration = Duration::from_secs(5 * 60);

static CACHE: Mutex<Option<HashMap<String, (Instant, Vec<Item>)>>> = Mutex::new(None);

/// Forgets a server's list (it's starting again, maybe with other plugins).
pub fn forget(id: &str) {
    if let Some(m) = CACHE.lock().unwrap().as_mut() {
        m.remove(id);
    }
}

/// The server's commands and settings: kept ones, or asked for now. Blocking.
pub fn for_server(id: &str, rcon: &Rcon, refresh: bool) -> Result<Vec<Item>, String> {
    if !refresh {
        if let Some((at, items)) = CACHE.lock().unwrap().as_ref().and_then(|m| m.get(id)) {
            if at.elapsed() < KEEP {
                return Ok(items.clone());
            }
        }
    }
    let ask = |cmd: &str| rcon.send(cmd).map_err(|_| "The server didn't answer. Check that it's running.".to_string());
    let cmdlist = ask("cmdlist")?;
    let cvarlist = ask("cvarlist")?;
    // without AMX Mod X these just say "Unknown command": nothing to add
    let amxx_cmds = ask("amxx cmds").unwrap_or_default();
    let amxx_cvars = ask("amxx cvars").unwrap_or_default();
    let mut help = String::new();
    let mut start = 1;
    for _ in 0..30 {
        let page = ask(&format!("amx_help {start}")).unwrap_or_default();
        help.push_str(&page);
        match next_help_page(&page) {
            Some(n) if n > start => start = n,
            _ => break,
        }
    }
    let items = merge(&cmdlist, &cvarlist, &amxx_cmds, &amxx_cvars, &help);
    if items.is_empty() {
        return Err("The server didn't list its commands. Try again in a moment.".into());
    }
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(id.to_string(), (Instant::now(), items.clone()));
    Ok(items)
}

/// The lines between cmdlist's / cvarlist's dashed rules.
fn listed(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut inside = false;
    for l in text.lines() {
        let t = l.trim_start_matches('\u{feff}').trim();
        if t.starts_with("-----") {
            if inside {
                break;
            }
            inside = true;
            continue;
        }
        if inside && !t.is_empty() {
            out.push(t);
        }
    }
    out
}

/// A row of `amxx cmds` / `amxx cvars` without its "[ 12]" number, split into words.
fn amxx_row(l: &str) -> Option<Vec<&str>> {
    let t = l.trim_start();
    if !t.starts_with('[') {
        return None;
    }
    let (_, rest) = t.split_once(']')?;
    Some(rest.split_whitespace().collect())
}

/// Where `amx_help` goes on: "Use 'amx_help 11' for more".
fn next_help_page(page: &str) -> Option<u32> {
    let i = page.find("Use 'amx_help ")?;
    page[i + 14..].split('\'').next()?.trim().parse().ok()
}

/// A setting's current value, asked for on its own (`"hostname" is "My Server"`), or None
/// for a password or token, a name that isn't one, or no answer. Blocking.
pub fn value(rcon: &Rcon, name: &str) -> Option<String> {
    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || secret(name) || NEVER.contains(&name) {
        return None;
    }
    parse_value(&rcon.send(name).ok()?, name)
}

fn parse_value(reply: &str, name: &str) -> Option<String> {
    let start = format!("\"{}\" is \"", name.to_ascii_lowercase());
    reply.lines().find_map(|l| {
        let t = l.trim();
        let rest = t.get(..start.len()).filter(|s| s.eq_ignore_ascii_case(&start)).map(|_| &t[start.len()..])?;
        // up to the closing quote (some builds add a default after it)
        Some(rest.find('"').map_or(rest, |i| &rest[..i]).to_string())
    })
}

/// A setting whose value isn't shown.
fn secret(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["password", "passwd", "token", "secret", "webhook", "_key", "apikey"].iter().any(|w| n.contains(w))
}

/// Never offered: the app refuses to send it (the app and the plugin share it).
const NEVER: &[&str] = &["rcon_password"];

/// Everything merged into one list, sorted by name.
pub fn merge(cmdlist: &str, cvarlist: &str, amxx_cmds: &str, amxx_cvars: &str, help: &str) -> Vec<Item> {
    let mut all: BTreeMap<String, Item> = BTreeMap::new();

    // plugin commands first: they have their full names. Only those the server console can
    // run (console, server): "client" ones are for players (say votekick and so on)
    for l in amxx_cmds.lines() {
        let Some(w) = amxx_row(l) else { continue };
        // name, [access], type, plugin: a name with spaces in it (say ...) is a client one
        if !(3..=4).contains(&w.len()) || !matches!(w[w.len() - 2], "console" | "server") {
            continue;
        }
        all.insert(w[0].to_ascii_lowercase(), Item { n: w[0].into(), k: "cmd", s: Some(w[w.len() - 1].into()), ..Default::default() });
    }

    // engine and game commands: cmdlist pads names to 16 and cuts longer ones off, so a
    // 16-character one is either a plugin command already listed in full or not trusted
    for name in listed(cmdlist) {
        if name.contains(' ') || name.len() >= 16 {
            continue;
        }
        all.entry(name.to_ascii_lowercase()).or_insert_with(|| Item { n: name.into(), k: "cmd", ..Default::default() });
    }

    // settings: "name : value, flags" (the value as a number, even for text: not used)
    for l in listed(cvarlist) {
        let Some((name, _)) = l.split_once(" :").or_else(|| l.split_once(':')) else { continue };
        let name = name.trim();
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        all.insert(name.to_ascii_lowercase(), Item { n: name.into(), k: "cvar", ..Default::default() });
    }

    // which plugin a setting comes from: NAME VALUE PLUGIN HOOKED MIN MAX
    for l in amxx_cvars.lines() {
        let Some(w) = amxx_row(l) else { continue };
        if w.len() < 5 {
            continue;
        }
        if let Some(it) = all.get_mut(&w[0].to_ascii_lowercase()) {
            it.s = Some(w[w.len() - 4].into());
        }
    }

    // the admin commands' syntax: "  5: amx_ban <name or #userid> <minutes> [reason]"
    for l in help.lines() {
        let Some((num, rest)) = l.trim().split_once(": ") else { continue };
        if num.parse::<u32>().is_err() {
            continue;
        }
        let rest = rest.trim();
        let (name, args) = rest.split_once(' ').unwrap_or((rest, ""));
        let (a, d) = describe(args);
        if let Some(it) = all.get_mut(&name.to_ascii_lowercase()) {
            it.a = it.a.take().or(a);
            it.d = it.d.take().or(d);
        }
    }

    // the engine's and the game's own, which nothing on the server describes
    for (name, text) in ENGINE {
        let (a, d) = describe(text);
        let key = name.to_ascii_lowercase();
        if let Some(it) = all.get_mut(&key) {
            it.a = it.a.take().or(a);
            it.d = it.d.take().or(d);
        } else if let Some((base, _)) = name.split_once(' ') {
            // a subcommand (meta list, amxx plugins): offered when its command is there
            if all.contains_key(base) {
                all.insert(key, Item { n: name.to_string(), k: "cmd", a, d, ..Default::default() });
            }
        }
    }

    for n in NEVER {
        all.remove(*n);
    }
    all.into_values().collect()
}

/// "<args> - what it does" into its two halves (either can be missing).
fn describe(text: &str) -> (Option<String>, Option<String>) {
    let text = text.trim();
    let (a, d) = match text.find(" - ") {
        Some(i) => (&text[..i], &text[i + 3..]),
        None if text.starts_with("- ") => ("", &text[2..]),
        None if text.starts_with(['<', '[', '"']) => (text, ""),
        None => ("", text),
    };
    let some = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    (some(a), some(d))
}

/// The engine's and Counter-Strike's own commands and settings, described: "<args> - what".
const ENGINE: &[(&str, &str)] = &[
    ("status", "- players on the server, their IDs, addresses and pings"),
    ("users", "- players' user IDs and Steam IDs"),
    ("changelevel", "<map> - change the map now, keeping players connected"),
    ("map", "<map> - start a map from scratch (players reconnect)"),
    ("restart", "- restart the current map"),
    ("kick", "<name or #userid> [reason] - remove a player from the server"),
    ("say", "<message> - message everyone as the server"),
    ("banid", "<minutes> <#userid or Steam ID> [kick] - ban a Steam ID (0 minutes: for good)"),
    ("removeid", "<Steam ID> - lift a Steam ID ban"),
    ("listid", "- the Steam ID bans"),
    ("writeid", "- save the Steam ID bans to banned.cfg"),
    ("addip", "<minutes> <address> - ban an IP address (0 minutes: for good)"),
    ("removeip", "<address> - lift an IP address ban"),
    ("listip", "- the IP address bans"),
    ("writeip", "- save the IP address bans to listip.cfg"),
    ("exec", "<file.cfg> - run a config file from the game folder"),
    ("echo", "<text> - print text to the console"),
    ("stats", "- the server's CPU, traffic and uptime"),
    ("version", "- the engine's version and build"),
    ("cmdlist", "- every command the server knows"),
    ("cvarlist", "- every setting and its value"),
    ("maps", "<filter or *> - maps in the maps folder"),
    ("quit", "- shut the server down"),
    ("exit", "- shut the server down"),
    ("hostname", "<name> - the name players see in the server list"),
    ("maxplayers", "- how many players fit (set when the server starts)"),
    ("sv_password", "<password> - the join password (empty: none)"),
    ("sv_cheats", "<0|1> - allow cheat commands"),
    ("sv_gravity", "<number> - gravity (800 is normal)"),
    ("sv_maxspeed", "<number> - the fastest players can run (320 is normal)"),
    ("sv_airaccelerate", "<number> - control in the air (10 is normal; 100 for surf and bhop)"),
    ("sv_restart", "<seconds> - restart the round after that many seconds"),
    ("sv_restartround", "<seconds> - restart the round after that many seconds"),
    ("sv_alltalk", "<0|1> - both teams hear each other's voice"),
    ("sv_voiceenable", "<0|1> - voice chat on or off"),
    ("sv_lan", "<0|1> - only players on this network (set by the app)"),
    ("sv_region", "<number> - the region shown in the server list (255: world)"),
    ("sv_timeout", "<seconds> - how long before a silent player is dropped"),
    ("mp_timelimit", "<minutes> - how long each map lasts (0: no limit)"),
    ("mp_roundtime", "<minutes> - how long each round lasts"),
    ("mp_freezetime", "<seconds> - freeze at the start of a round"),
    ("mp_buytime", "<minutes> - how long buying is open after a round starts"),
    ("mp_startmoney", "<amount> - money at the start (800 is normal)"),
    ("mp_c4timer", "<seconds> - the bomb's countdown"),
    ("mp_friendlyfire", "<0|1> - teammates can hurt each other"),
    ("mp_autoteambalance", "<0|1> - even out the teams"),
    ("mp_limitteams", "<number> - the most one team can have over the other (0: no limit)"),
    ("mp_autokick", "<0|1> - kick idle players and team killers"),
    ("mp_tkpunish", "<0|1> - team killers sit out the next round"),
    ("mp_maxrounds", "<number> - change map after this many rounds (0: no limit)"),
    ("mp_winlimit", "<number> - change map once a team wins this many (0: no limit)"),
    ("mp_footsteps", "<0|1> - footstep sounds"),
    ("mp_flashlight", "<0|1> - flashlights allowed"),
    ("mp_forcecamera", "<0|1|2> - what dead players can watch"),
    ("mp_fadetoblack", "<0|1> - dead players' screens go black"),
    ("mp_hostagepenalty", "<number> - hostage kills before a kick (0: never)"),
    ("mp_chattime", "<seconds> - time at the scoreboard before the next map"),
    ("mp_logmessages", "<0|1> - write chat to the server's logs"),
    ("bot_add", "- add a bot to the team that needs one"),
    ("bot_add_t", "- add a Terrorist bot"),
    ("bot_add_ct", "- add a Counter-Terrorist bot"),
    ("bot_kick", "[name or all] - remove bots"),
    ("bot_kill", "[name or all] - kill bots"),
    ("bot_quota", "<number> - how many bots to keep on the server"),
    ("bot_quota_mode", "<normal|fill|match> - how the bot count follows the players"),
    ("bot_difficulty", "<0-3> - easy, normal, hard, expert (new bots)"),
    ("bot_join_team", "<any|T|CT> - the team bots join"),
    ("bot_join_after_player", "<0|1> - bots wait for a person before joining"),
    ("bot_stop", "<0|1> - freeze all bots"),
    ("bot_knives_only", "- bots only use knives"),
    ("bot_pistols_only", "- bots only use pistols"),
    ("bot_all_weapons", "- bots use every weapon again"),
    ("bot_chatter", "<off|radio|minimal|normal> - how much bots talk"),
    ("meta list", "- the Metamod plugins loaded"),
    ("meta version", "- Metamod's version"),
    ("amxx plugins", "- the AMX Mod X plugins loaded and running"),
    ("amxx modules", "- the AMX Mod X modules loaded"),
    ("amxx cvars", "- plugin settings and which plugin made them"),
    ("amxx cmds", "- plugin commands and which plugin made them"),
    ("amxx version", "- AMX Mod X's version"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const CMDLIST: &str = "\u{feff}Command List\n--------------\namx_restrict    \ngm_botchat_reloa\nstatus          \nchangelevel     \nrcon_password   \nmeta            \namxx            \n--------------\n7 Total Commands\nCmdList ? for syntax\n";
    const CVARLIST: &str = "CVar List\n--------------\nmp_roundtime    :      2.5, sv\nhostname        : My \"Server\", a\nsv_password     : hunter2, sv\ngm_webhook_url  : https://x\nrcon_password   : abc\namx_mode        :        1\n--------------\n6 Total CVars\n";
    const AMXX_CMDS: &str = "Registered commands:\n       name                     access           type     plugin          \n [  1] amx_ban                  dv               console  admincmd.amxx   \n [  2] amx_addmenuitem                           server   menufront.amxx  \n [  3] gm_botchat_reload        l                console  gm_botchat.amxx \n [  4] amx_banmenu              dv               client   plmenu.amxx     \n [  5] say votekick                              client   gm_extras.amxx  \n5 commands\n";
    const AMXX_CVARS: &str = "\nManaged cvars:\n       NAME   VALUE   PLUGIN   HOOKED   MIN   MAX\n [  1] amx_mode                 1                        admin.amxx         no       -        -        \n";
    const HELP: &str = "\n----- AMX Mod X Help: Commands -----\n  1: amx_ban <name or #userid> <minutes> [reason]\n  2: amx_addmenuitem <menu text> <menu command> - Add a menu item\n----- Entries 1 - 2 of 12 -----\n----- Use 'amx_help 11' for more -----\n";

    fn get<'a>(items: &'a [Item], n: &str) -> Option<&'a Item> {
        items.iter().find(|i| i.n == n)
    }

    #[test]
    fn merges_the_server_lists() {
        let items = merge(CMDLIST, CVARLIST, AMXX_CMDS, AMXX_CVARS, HELP);
        // full plugin names instead of cmdlist's cut-off ones; players' commands left out
        assert!(get(&items, "gm_botchat_reload").is_some_and(|i| i.s.as_deref() == Some("gm_botchat.amxx")));
        assert!(get(&items, "gm_botchat_reloa").is_none());
        assert!(get(&items, "amx_banmenu").is_none() && !items.iter().any(|i| i.n.starts_with("say")));
        // syntax from amx_help, and its description after " - "
        let ban = get(&items, "amx_ban").unwrap();
        assert_eq!(ban.a.as_deref(), Some("<name or #userid> <minutes> [reason]"));
        let menu = get(&items, "amx_addmenuitem").unwrap();
        assert_eq!((menu.a.as_deref(), menu.d.as_deref()), (Some("<menu text> <menu command>"), Some("Add a menu item")));
        // settings, and the plugin they come from
        assert_eq!(get(&items, "mp_roundtime").unwrap().k, "cvar");
        assert!(get(&items, "hostname").is_some() && get(&items, "gm_webhook_url").is_some());
        assert_eq!(get(&items, "amx_mode").unwrap().s.as_deref(), Some("admin.amxx"));
        // never offered
        assert!(get(&items, "rcon_password").is_none());
        // the engine's own described, subcommands only when their command is there
        assert!(get(&items, "changelevel").unwrap().a.as_deref() == Some("<map>"));
        assert!(get(&items, "meta list").is_some() && get(&items, "amxx plugins").is_some());
        assert!(get(&items, "bot_add").is_none(), "only what the server has");
        assert!(items.windows(2).all(|w| w[0].n.to_ascii_lowercase() <= w[1].n.to_ascii_lowercase()));
    }

    #[test]
    fn without_amx_mod_x() {
        let items = merge(CMDLIST, CVARLIST, "Unknown command: amxx\n", "Unknown command: amxx\n", "Unknown command: amx_help\n");
        assert!(get(&items, "status").is_some() && get(&items, "amxx plugins").is_some());
        assert!(get(&items, "amx_ban").is_none());
    }

    #[test]
    fn setting_values() {
        assert_eq!(parse_value("\"hostname\" is \"My Server\"
", "hostname").as_deref(), Some("My Server"));
        assert_eq!(parse_value("\"mp_roundtime\" is \"2.5\" ( def. \"5\" )
", "MP_roundtime").as_deref(), Some("2.5"));
        assert_eq!(parse_value("\"sv_gravity\" is \"\"
", "sv_gravity").as_deref(), Some(""));
        assert_eq!(parse_value("Unknown command: nope
", "nope"), None);
        assert!(secret("sv_password") && secret("gm_discord_webhook") && !secret("hostname"));
    }

    #[test]
    fn help_pages() {
        assert_eq!(next_help_page(HELP), Some(11));
        assert_eq!(next_help_page("----- Entries 41 - 45 of 45 -----\n"), None);
    }

    #[test]
    fn descriptions() {
        assert_eq!(describe("<map> - change the map"), (Some("<map>".into()), Some("change the map".into())));
        assert_eq!(describe("- players"), (None, Some("players".into())));
        assert_eq!(describe("<a> [b]"), (Some("<a> [b]".into()), None));
        assert_eq!(describe(""), (None, None));
    }
}
