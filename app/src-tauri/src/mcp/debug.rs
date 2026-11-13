//! Debugging and tracing for agents, the way we debug ourselves: the game's console (a
//! read-only allowlist with changes off), console and AMX Mod X log tails with filters and a
//! cursor, the bot brain's trace and report switches, its trace file, recordings, plus the
//! whole app through its own routes (`app_call`, under routes.rs's rules).

use super::catalog::{ask, err, get, id_arg, map_arg, obj, own_server, people_on, plural, s, Done, Fail, Kind, Spec};
use super::routes::{self, Policy};
use super::Ctx;
use axum::http::Method;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

fn server_prop() -> Value {
    json!({"type": "string", "description": "\"game\" for the game the person hosts from Play, or one of their own server ids (servers_list)"})
}
fn server_only() -> Value {
    obj(json!({"server": server_prop()}), &["server"])
}

/// The bot brain's switches (ReGameDLL fork, cs_bot_ls.cpp / cs_bot_surf.cpp / creature.cpp).
pub const BRAIN_CVARS: &[(&str, &str)] = &[
    ("bot_ls_trace", "0/1. Testing: twice a second every bot's state (task, enemy, goal, why, speed, aim, position, hp, weapon) goes to lstrace.csv in the server's folder; ls_mark adds notes. Read it with brain_trace_read."),
    ("bot_ls_surf", "0 off, 1 the brain steers surf bots, 2 also reports every 10 s to the console (LSSURF <map>: floor, starts, lines kept, steering, landings...). Read with console_log filter LSSURF."),
    ("bot_ls_surf_land", "0/1. Experimental landing-aware air control for surf bots."),
    ("bot_ls_fight", "0/1 (1). Human-like fighting; 0 lets a mode's plugin drive the bots."),
    ("bot_ls_match", "0/1 (0). Disciplined match habits (5v5 match mode sets it)."),
    ("bot_ls_solo", "0/1 (1). Bots with little teamwork in their profile play on their own."),
    ("bot_ls_own_memory", "0/1 (1). Each bot keeps its own memory of the map."),
    ("bot_ls_routes", "Number (1.0). How much bots vary their routes."),
    ("bot_ls_lines", "Number (1.0). How much bots hold lines/angles."),
    ("bot_ls_pauses", "Number (1.0). How often bots pause like people do."),
    ("ls_creature_debug", "0/1. Creature debugging output."),
    ("ls_creature_max", "Most creatures at once (48)."),
    ("ls_creature_budget", "Creature think budget (40)."),
];

/// Read-only console lines: exact commands, and reading cvars by name.
const READ_COMMANDS: &[&str] = &[
    "status", "version", "stats", "meta list", "meta version", "amxx list", "amxx plugins", "amxx modules", "amxx version", "amxx cvars",
    "ls_mode status", "ls_creature list", "ls_creature status", "ls_creature types", "listmaps",
];
/// bot_ names that are commands, not settings (never "read" by typing them).
const BOT_COMMANDS: &[&str] = &[
    "bot_add", "bot_add_t", "bot_add_ct", "bot_kick", "bot_kill", "bot_knives_only", "bot_pistols_only", "bot_snipers_only", "bot_all_weapons",
    "bot_goto_mark", "bot_goto_selected", "bot_zoom", "bot_about", "bot_debug_target",
];

/// How a console line may go: with changes off only reads pass; with them on, a few still ask.
#[derive(Debug, PartialEq)]
pub enum Console {
    Read,
    Change,
    Ask(String),
}

pub fn console_policy(line: &str, changes: bool) -> Result<Console, String> {
    let line = line.trim();
    if line.is_empty() || line.len() > 200 || line.chars().any(|c| c.is_control()) {
        return Err("That command can't be sent.".into());
    }
    if line.contains(';') {
        return Err("One command at a time (no ;).".into());
    }
    let norm = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = norm.to_ascii_lowercase();
    let first = lower.split(' ').next().unwrap_or("");
    if first.trim_matches('"').starts_with("rcon_password") || first == "rcon" {
        return Err("The console password is the person's to set.".into());
    }
    let words = lower.split(' ').count();
    let read = READ_COMMANDS.contains(&lower.as_str())
        || (first == "cvarlist" && words <= 2 && lower.chars().all(|c| c.is_ascii_alphanumeric() || " _".contains(c)))
        || (words == 1 && reads_cvar(first));
    if read {
        return Ok(Console::Read);
    }
    if !changes {
        return Err(format!("With changes off only reading works here: {}, or a setting's name to read it (bot_ls_*, sv_*, mp_*, bot_quota...). The person can turn on \"Allow changes\" in Settings > AI agents.", READ_COMMANDS.join(", ")));
    }
    let asks: &[(&str, &str)] = &[
        ("quit", "shut the game or server down"), ("exit", "shut the game or server down"), ("killserver", "shut the server down"),
        ("kick", "kick a player"), ("banid", "ban a player"), ("addip", "ban an address"), ("amx_kick", "kick a player"), ("amx_ban", "ban a player"),
        ("amx_banip", "ban a player"), ("amx_slay", "slay a player"), ("writeid", "write the ban list"), ("writeip", "write the ban list"),
        ("sv_password", "change the join password"), ("changelevel", "change the map"), ("map", "change the map"), ("restart", "restart the map"),
        ("sv_downloadurl", "change where players download from"), ("exec", "run a config file"),
    ];
    if let Some((_, what)) = asks.iter().find(|(c, _)| *c == first) {
        return Ok(Console::Ask(what.to_string()));
    }
    Ok(Console::Change)
}

fn reads_cvar(name: &str) -> bool {
    let ok_chars = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok_chars
        && (name.starts_with("bot_ls_")
            || name.starts_with("ls_creature_")
            || name == "ls_mode_ver"
            || name == "ls_sites"
            || name.starts_with("sv_") && name != "sv_password"
            || name.starts_with("mp_")
            || name.starts_with("gm_") && name.ends_with("_ver")
            || name.starts_with("bot_") && !BOT_COMMANDS.contains(&name) && !name.starts_with("bot_nav_")
            || ["hostname", "maxplayers", "deathmatch", "coop", "pausable", "developer", "fps_max", "sys_ticrate", "amxmodx_version", "metamod_version"].contains(&name))
}

/// Where a game keeps its files: (Half-Life folder or server folder, mod folder).
fn places(ctx: &Ctx, server: Option<&str>) -> Result<(PathBuf, PathBuf), Fail> {
    match server {
        None => {
            let st = ctx.st.as_ref().ok_or_else(|| err("No game here."))?;
            let g = st.local_game().ok_or_else(|| err("Set up the game on this computer first."))?;
            Ok((g.hl.clone(), g.dir.clone()))
        }
        Some(id) => {
            let e = crate::servers::get(id).ok_or_else(|| err("That server isn't there any more."))?;
            Ok((e.dir.clone(), e.dir.join(&e.game)))
        }
    }
}

/// "game" or one of the person's own servers on this computer (None = the game).
async fn target(ctx: &Ctx, a: &Value) -> Result<Option<String>, Fail> {
    match s(a, "server") {
        None | Some("game") | Some("local") => Ok(None),
        Some(_) => own_server(ctx, a).await.map(Some),
    }
}

/// Sends one console line to the game or a server, and returns its answer.
async fn rcon(ctx: &Ctx, server: Option<&str>, line: &str) -> Result<String, Fail> {
    match server {
        Some(id) => {
            let v = super::catalog::call(ctx, Method::POST, &format!("/api/servers/{id}/console/send"), None, Some(json!({"command": line}))).await?;
            Ok(v["reply"].as_str().unwrap_or("").to_string())
        }
        None => {
            let st = ctx.st.clone().ok_or_else(|| err("No game here."))?;
            let line = line.to_string();
            tokio::task::spawn_blocking(move || {
                let g = st.local_game().ok_or("Set up the game on this computer first.")?;
                g.rcon.send(&line).map_err(|_| "The game isn't running, or it didn't take Loner-Strike's console password. Start it from Play first.".to_string())
            })
            .await
            .map_err(|_| err("The game didn't answer."))?
            .map_err(Fail::Error)
        }
    }
}

fn clean_reply(r: &str) -> Vec<String> {
    r.lines().map(|l| crate::control::common::hide_passwords(l.trim_end())).filter(|l| !l.trim().is_empty()).collect()
}

/// `"name" is "value"` from a cvar read.
pub fn cvar_value(reply: &str) -> Option<String> {
    let l = reply.lines().find(|l| l.contains("\" is \""))?;
    let start = l.find("\" is \"")? + 6;
    let rest = &l[start..];
    Some(rest[..rest.find('"').unwrap_or(rest.len())].to_string())
}

// ---- log tails --------------------------------------------------------------------------

fn newest(dir: &Path, prefix: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix) && e.file_name().to_string_lossy().ends_with(".log"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

pub fn log_file(base: &Path, moddir: &Path, source: &str) -> Option<PathBuf> {
    let logs = moddir.join("addons").join("amxmodx").join("logs");
    match source {
        "console" => [base.join("qconsole.log"), moddir.join("qconsole.log"), base.join("loner-strike-server.log")].into_iter().find(|p| p.is_file()),
        "amxx" => newest(&logs, "L"),
        "errors" => newest(&logs, "error_"),
        "trace" => [base.join("lstrace.csv"), moddir.join("lstrace.csv")].into_iter().find(|p| p.is_file()),
        _ => None,
    }
}

/// Lines from a file after a cursor ("<file name>:<byte>") or its last `n`, filtered.
pub fn tail(path: &Path, cursor: Option<&str>, n: usize, keep: &dyn Fn(&str) -> bool) -> Result<Value, String> {
    use std::io::{Read, Seek, SeekFrom};
    const MAX: u64 = 256 * 1024;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut f = std::fs::File::open(path).map_err(|e| format!("Couldn't read {name}: {e}"))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let asked = cursor.and_then(|c| c.rsplit_once(':')).filter(|(file, _)| *file == name).and_then(|(_, at)| at.parse::<u64>().ok());
    let (start, reset, following) = match asked {
        Some(at) if at <= len => (at, false, true),
        Some(_) => (0, true, true), // shorter than before: a new file
        None if cursor.is_some() => (0, true, true),
        None => (len.saturating_sub(MAX), false, false),
    };
    let end = if following { (start + MAX).min(len) } else { len };
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; (end - start) as usize];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    // only whole lines (the rest comes next time)
    let whole = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1).unwrap_or(if end == len { buf.len() } else { 0 });
    let text: String = buf[..whole].iter().map(|&b| b as char).collect();
    let mut lines: Vec<&str> = text.lines().collect();
    if !following && start > 0 && !lines.is_empty() {
        lines.remove(0); // (cut in the middle)
    }
    let picked: Vec<String> = lines.into_iter().filter(|l| keep(l)).map(|l| crate::control::common::hide_passwords(l.trim_end_matches('\r'))).collect();
    let total = picked.len();
    let shown: Vec<String> = if following { picked.into_iter().take(n).collect() } else { picked.into_iter().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect() };
    // following and more matched than shown: carry on after the last shown line next time
    let next = if following && total > n {
        let last = shown.last().cloned().unwrap_or_default();
        let at = text.find(last.as_str()).map(|i| i + last.len() + 1).unwrap_or(whole);
        start + at as u64
    } else {
        start + whole as u64
    };
    Ok(json!({"file": name, "lines": shown, "more": following && (total > n || end < len), "reset": reset, "cursor": format!("{name}:{next}")}))
}

/// The filter a log tool was given.
fn filter(a: &Value) -> Box<dyn Fn(&str) -> bool + Send + Sync> {
    let contains: Vec<String> = s(a, "contains").map(|c| c.to_lowercase().split_whitespace().map(String::from).collect()).unwrap_or_default();
    let prefixes: Vec<String> = a.get("prefixes").and_then(Value::as_array).map(|p| p.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
    let errors = a.get("errors").and_then(Value::as_bool).unwrap_or(false);
    Box::new(move |l: &str| {
        let low = l.to_lowercase();
        // (AMXX and qconsole lines start with a date: the prefix is looked for after it)
        let body = l.split_once(": ").filter(|(d, _)| d.starts_with('L') && d.len() < 30).map(|(_, b)| b).unwrap_or(l);
        (contains.iter().all(|w| low.contains(w.as_str())))
            && (prefixes.is_empty() || prefixes.iter().any(|p| body.starts_with(p.as_str()) || l.starts_with(p.as_str())))
            && (!errors || ["error", "fail", "warning", "run time error", "can't", "couldn't", "unknown command", "bad"].iter().any(|w| low.contains(w)))
    })
}

// ---- tools ------------------------------------------------------------------------------

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        Spec { name: "console_log", title: "Console: read the log", kind: Read, schema: console_log_schema,
            description: "The end of the game's or a server's console log or AMX Mod X logs, filtered (words, line prefixes such as LSSURF or [gm_, errors only), with a cursor to follow new lines." },
    ]
    .into_iter()
    .chain(more_specs())
    .collect()
}

fn console_log_schema() -> Value {
    obj(json!({
            "server": server_prop(),
            "source": {"type": "string", "enum": ["console", "amxx", "errors"], "description": "console: qconsole.log (the brain's LSSURF reports, gm_* plugin lines, engine messages); amxx: today's AMX Mod X log; errors: AMX Mod X's error log"},
            "contains": {"type": "string", "description": "Every word must be in the line (any case)"},
            "prefixes": {"type": "array", "items": {"type": "string"}, "description": "Lines starting with one of these (after the log's date), e.g. LSSURF, [gm_"},
            "errors": {"type": "boolean", "description": "Only lines that look like errors or warnings"},
            "cursor": {"type": "string", "description": "The last answer's cursor: only what came after (follow the log)"},
            "lines": {"type": "integer", "minimum": 1, "maximum": 500, "description": "At most this many (default 100)"},
        }), &["server"])
}

fn more_specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        Spec { name: "app_routes", title: "The app's routes", kind: Read,
            schema: || obj(json!({"search": {"type": "string", "description": "Words in the path or what it's for"}}), &[]),
            description: "Every route of the app agents can use with app_call: method, path, what it's for, and whether it reads, changes, asks the person first or is the person's alone. Anything the app window can do goes through these same routes and rules." },
        Spec { name: "app_call", title: "Use the app", kind: Read,
            schema: || obj(json!({
                "method": {"type": "string", "enum": ["GET", "POST"]},
                "path": {"type": "string", "description": "A route from app_routes with its parts filled in, e.g. /api/servers/abc/start"},
                "query": {"type": "object", "additionalProperties": {"type": ["string", "number", "boolean"]}},
                "body": {"type": "object", "description": "The JSON body for a POST, as the app's page sends it"},
                "target": {"type": "string", "description": "For routes about \"the game\" (/api/state, /api/cmd, ...): one of the person's server ids to aim them at that server instead"},
            }), &["method", "path"]),
            description: "Calls one of the app's own routes (see app_routes), exactly as the app window does: local play (launch, the web panel's controls via /api/cmd), dedicated servers (make, settings, start/stop, rotation, HLTV, admins), GetMore / GameBanana installs, add-ons, nav learning, sounds, preferences... GET reads (works with changes off); POST needs \"Allow changes\"; deleting, kicking/banning, stopping what people play on, closing the game and anything public ask the person first; the person's own settings (AI agents, updates, setup repair, phone, sharing, passwords) are refused. Secrets are never shown." },
        Spec { name: "console_send", title: "Console: run a command", kind: Read,
            schema: || obj(json!({"server": server_prop(), "command": {"type": "string", "description": "One console line, e.g. status, bot_ls_trace, ls_mode status, ls_creature list"}}), &["server", "command"]),
            description: "Runs one console command on the person's game or one of their servers on this computer and returns its answer. With changes off only reads work (status, amxx list, meta list, ls_mode status, ls_creature list, cvarlist <prefix>, or a setting's name to read it); anything else needs \"Allow changes\", and kicking, banning, changing the map, quitting or exec ask first. Logged." },
        Spec { name: "brain_settings", title: "Bot brain: settings", kind: Read, schema: server_only,
            description: "The bot brain's switches on a running game or server (bot_ls_trace, bot_ls_surf, bot_ls_fight...) with their values and what each does." },
        Spec { name: "brain_set", title: "Bot brain: set a switch", kind: Change,
            schema: || obj(json!({"server": server_prop(), "cvar": {"type": "string", "enum": BRAIN_CVARS.iter().map(|c| c.0).collect::<Vec<_>>(), "description": BRAIN_CVARS.iter().map(|(n, d)| format!("{n}: {d}")).collect::<Vec<_>>().join(" | ")}, "value": {"type": "string"}}), &["server", "cvar", "value"]),
            description: "Sets one of the bot brain's switches on a running game or server, e.g. bot_ls_trace 1 to trace every bot, bot_ls_surf 2 for surf reports in the console." },
        Spec { name: "brain_trace", title: "Bot brain: trace on/off", kind: Change,
            schema: || obj(json!({"server": server_prop(), "on": {"type": "boolean"}, "note": {"type": "string", "description": "A mark to put in the trace now (ls_mark), e.g. 'before the fix'"}}), &["server"]),
            description: "Turns the brain's trace on or off (all bots, twice a second, to lstrace.csv) and/or puts a mark in it. Read it with brain_trace_read." },
        Spec { name: "brain_trace_read", title: "Bot brain: read the trace", kind: Read,
            schema: || obj(json!({"server": server_prop(), "bot": {"type": "integer", "description": "Only this bot id"}, "from": {"type": "number", "description": "Only rows from this game time on"}, "to": {"type": "number"}, "cursor": {"type": "string", "description": "Follow: only rows written after the last answer"}, "lines": {"type": "integer", "minimum": 1, "maximum": 500}}), &["server"]),
            description: "Rows of the brain's live trace (lstrace.csv) as named columns: time, bot id, team, state, task, enemy, leader, goal, noise, why, speed, aim, attacking, look, position, hp, weapon, crouched; plus #why / #attack / #seen / #mark notes." },
        Spec { name: "mode_status", title: "Mode brain status", kind: Read, schema: server_only,
            description: "What the brain was told by the running game mode (ls_mode status): goals, zones, roles, friends and foes, enemy classes." },
        Spec { name: "creatures_list", title: "Creatures", kind: Read, schema: server_only,
            description: "Creatures alive in the running game (ls_creature list), when the brain has them." },
        Spec { name: "recording_status", title: "Recorder status", kind: Read, schema: server_only,
            description: "Whether the bot recorder (gm_simrec) is loaded on the game or server and which recordings it has written (they show up in replay_list for the game). Map simulations always record." },
        Spec { name: "recording_set", title: "Recorder on/off", kind: Change,
            schema: || obj(json!({"server": server_prop(), "on": {"type": "boolean"}}), &["server", "on"]),
            description: "Pauses or resumes the bot recorder where it's loaded (amxx pause/unpause gm_simrec)." },
        Spec { name: "replay_trace", title: "Recording: a bot's trace", kind: Read,
            schema: || obj(json!({"id": {"type": "string"}, "bot": {"type": "integer"}, "from": {"type": "number", "description": "Game time from"}, "to": {"type": "number"}, "every": {"type": "integer", "minimum": 1, "description": "Every Nth row (a path at lower detail)"}, "lines": {"type": "integer", "minimum": 1, "maximum": 500}}), &["id"]),
            description: "A recording's brain trace for one bot (or all) in a time window, as named columns, with the why / attack / seen notes; with every: N it doubles as the bot's path." },
        Spec { name: "nav_reach", title: "Nav: where bots can't get", kind: Read,
            schema: || obj(json!({"map": {"type": "string"}}), &["map"]),
            description: "The nav fixer's reach check: area ids bots can never reach from a spawn, traps they can't leave, and areas reached only by falling in." },
        Spec { name: "docs_read", title: "Read the docs", kind: Read,
            schema: || obj(json!({"doc": {"type": "string", "enum": DOCS.iter().map(|d| d.0).collect::<Vec<_>>()}, "offset": {"type": "integer", "minimum": 0}}), &["doc"]),
            description: "Loner-Strike's docs for agents in pages: components, loner-inc (the AMX Mod X include), creatures, bot-people, mcp (this connector), gamemodes-plan, studio-nodes." },
    ]
}

/// Docs compiled in for agents (also resources loner://docs/<name>).
pub const DOCS: &[(&str, &str, &str)] = &[
    ("components", "Game mode components: every component's keys, events, natives, what bots do", include_str!("../../../../docs/COMPONENTS.md")),
    ("loner-inc", "loner.inc: the AMX Mod X include for modes and plugins", include_str!("../../../../docs/LONER-INC.md")),
    ("creatures", "Creatures: the brain's monsters, commands and cvars", include_str!("../../../../docs/CREATURES.md")),
    ("bot-people", "Bot people: archetypes, traits, names", include_str!("../../../../docs/bot-people.md")),
    ("mcp", "This connector: tools, safety, debugging walkthrough", include_str!("../../../../docs/MCP.md")),
    ("gamemodes-plan", "The game modes plan (long: read in pages)", include_str!("../../../../docs/GAMEMODES-PLAN.md")),
    ("studio-nodes", "Game Mode Studio node catalog (JSON)", super::more::STUDIO_NODES),
];

pub fn doc_page(name: &str, offset: usize) -> Option<Value> {
    let (_, what, text) = DOCS.iter().find(|d| d.0 == name)?;
    const PAGE: usize = 40_000;
    let start = offset.min(text.len());
    let mut s0 = start;
    while !text.is_char_boundary(s0) {
        s0 -= 1;
    }
    let mut end = (s0 + PAGE).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Some(json!({"doc": name, "about": what, "text": &text[s0..end], "offset": s0, "next": (end < text.len()).then_some(end), "length": text.len()}))
}

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "app_routes" => Ok(app_routes(s(a, "search").unwrap_or(""))),
        "app_call" => app_call(ctx, a).await,
        "console_send" => async {
            let t = target(ctx, a).await?;
            let line = s(a, "command").ok_or_else(|| err("Say the command."))?.trim().to_string();
            match console_policy(&line, ctx.changes) {
                Err(why) => Err(if ctx.changes { err(why) } else { Fail::Refused(why) }),
                Ok(p) => {
                    if let Console::Ask(what) = &p {
                        let people = people_on(ctx, t.as_deref()).await;
                        let who = match people {
                            Some(0) => "Nobody's playing.".to_string(),
                            Some(n) => format!("{} playing.", plural(n)),
                            None => "Loner-Strike couldn't tell who's playing.".into(),
                        };
                        ask(ctx, format!("An AI agent wants to {what} with the console line \"{line}\". {who} Allow it?")).await?;
                    }
                    let reply = rcon(ctx, t.as_deref(), &line).await?;
                    Ok(json!({"command": line, "reply": clean_reply(&reply)}))
                }
            }
        }
        .await,
        "console_log" => async {
            let t = target(ctx, a).await?;
            let (base, moddir) = places(ctx, t.as_deref())?;
            let source = s(a, "source").unwrap_or("console");
            if !["console", "amxx", "errors"].contains(&source) {
                return Err(err("source must be console, amxx or errors."));
            }
            let file = log_file(&base, &moddir, source).ok_or_else(|| err(format!("No {source} log there yet (it starts when the game or server runs).")))?;
            let keep = filter(a);
            let n = limit_lines(a, 100);
            let cursor = s(a, "cursor").map(String::from);
            tokio::task::spawn_blocking(move || tail(&file, cursor.as_deref(), n, &*keep)).await.map_err(|_| err("Couldn't read the log."))?.map_err(Fail::Error)
        }
        .await,
        "brain_settings" => async {
            let t = target(ctx, a).await?;
            let mut out = vec![];
            for (cvar, what) in BRAIN_CVARS {
                let reply = rcon(ctx, t.as_deref(), cvar).await?;
                out.push(json!({"cvar": cvar, "value": cvar_value(&reply), "what": what}));
            }
            Ok(json!({"settings": out}))
        }
        .await,
        "brain_set" => async {
            let t = target(ctx, a).await?;
            let cvar = s(a, "cvar").filter(|c| BRAIN_CVARS.iter().any(|b| b.0 == *c)).ok_or_else(|| err("cvar must be one of the brain's switches (brain_settings lists them)."))?;
            let value = s(a, "value").map(str::trim).filter(|v| !v.is_empty() && v.len() <= 16 && v.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')).ok_or_else(|| err("value must be a number."))?;
            rcon(ctx, t.as_deref(), &format!("{cvar} {value}")).await?;
            let now = cvar_value(&rcon(ctx, t.as_deref(), cvar).await?);
            Ok(json!({"cvar": cvar, "value": now}))
        }
        .await,
        "brain_trace" => async {
            let t = target(ctx, a).await?;
            let mut did = vec![];
            if let Some(on) = a.get("on").and_then(Value::as_bool) {
                rcon(ctx, t.as_deref(), &format!("bot_ls_trace {}", on as u8)).await?;
                did.push(format!("trace {}", if on { "on" } else { "off" }));
            }
            if let Some(note) = s(a, "note") {
                let note: String = note.chars().filter(|c| c.is_ascii_alphanumeric() || " _-.,:".contains(*c)).take(80).collect();
                let reply = rcon(ctx, t.as_deref(), &format!("ls_mark {note}")).await?;
                did.push(format!("mark: {}", clean_reply(&reply).join(" ")));
            }
            let now = cvar_value(&rcon(ctx, t.as_deref(), "bot_ls_trace").await?);
            Ok(json!({"did": did, "bot_ls_trace": now}))
        }
        .await,
        "brain_trace_read" => async {
            let t = target(ctx, a).await?;
            let (base, moddir) = places(ctx, t.as_deref())?;
            let file = log_file(&base, &moddir, "trace").ok_or_else(|| err("No trace yet: turn it on with brain_trace (on: true) while the game runs."))?;
            let (bot, from, to) = (a.get("bot").and_then(Value::as_i64), a.get("from").and_then(Value::as_f64), a.get("to").and_then(Value::as_f64));
            let n = limit_lines(a, 200);
            let cursor = s(a, "cursor").map(String::from);
            tokio::task::spawn_blocking(move || trace_rows(&file, cursor.as_deref(), n, bot, from, to)).await.map_err(|_| err("Couldn't read the trace."))?.map_err(Fail::Error)
        }
        .await,
        "mode_status" | "creatures_list" => async {
            let t = target(ctx, a).await?;
            let line = if name == "mode_status" { "ls_mode status" } else { "ls_creature list" };
            Ok(json!({"command": line, "reply": clean_reply(&rcon(ctx, t.as_deref(), line).await?)}))
        }
        .await,
        "recording_status" => async {
            let t = target(ctx, a).await?;
            let (_, moddir) = places(ctx, t.as_deref())?;
            let loaded = rcon(ctx, t.as_deref(), "amxx list").await.map(|r| r.contains("gm_simrec")).ok();
            let dir = moddir.join("addons").join("amxmodx").join("data").join("simrec");
            let mut files: Vec<(String, u64)> = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter_map(|e| Some((e.file_name().to_string_lossy().to_string(), e.metadata().ok()?.len()))).collect();
            files.sort();
            Ok(json!({"loaded": loaded, "files": files.iter().rev().take(30).map(|(n, l)| json!({"file": n, "size": l})).collect::<Vec<_>>(),
                      "note": if loaded == Some(true) { "Recording while the map runs; replay_list shows the game's recordings." } else { "The recorder isn't loaded here. A map simulation (health_start) records bots and keeps the recording with its trace for replay_summary / replay_trace." }}))
        }
        .await,
        "recording_set" => async {
            let t = target(ctx, a).await?;
            let on = a["on"].as_bool().unwrap_or(true);
            let reply = rcon(ctx, t.as_deref(), &format!("amxx {} gm_simrec.amxx", if on { "unpause" } else { "pause" })).await?;
            Ok(json!({"reply": clean_reply(&reply)}))
        }
        .await,
        "replay_trace" => async {
            let id = id_arg(a, "id")?;
            let v = get(ctx, &format!("/api/tools/recording/{id}/trace")).await?;
            Ok(trace_window(&v, a))
        }
        .await,
        "nav_reach" => async {
            let m = map_arg(a)?;
            let v = get(ctx, &format!("/api/tools/navfix/{m}")).await?;
            let ids: Vec<Value> = v["nav"]["areas"].as_array().map(|l| l.iter().map(|x| x["id"].clone()).collect()).unwrap_or_default();
            let by = |n: u64| -> Vec<Value> { v["reach"].as_array().into_iter().flatten().enumerate().filter(|(_, r)| r.as_u64() == Some(n)).filter_map(|(i, _)| ids.get(i).cloned()).collect() };
            Ok(json!({"areas": ids.len(), "unreachable": by(1), "traps": by(2), "onlyByFalling": by(3), "spawnAreas": v["spawnAreas"], "note": v["checkNote"]}))
        }
        .await,
        "docs_read" => doc_page(s(a, "doc").unwrap_or(""), a["offset"].as_u64().unwrap_or(0) as usize).ok_or_else(|| err("Unknown doc.")),
        _ => return None,
    })
}

fn limit_lines(a: &Value, default: usize) -> usize {
    a.get("lines").and_then(Value::as_u64).map(|n| (n as usize).clamp(1, 500)).unwrap_or(default)
}

/// Rows of lstrace.csv as named columns (the #cols line names them).
pub fn trace_rows(file: &Path, cursor: Option<&str>, n: usize, bot: Option<i64>, from: Option<f64>, to: Option<f64>) -> Result<Value, String> {
    // the column names: the first #cols line in the file
    let head: String = {
        use std::io::Read;
        let mut b = vec![0u8; 64 * 1024];
        let got = std::fs::File::open(file).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
        b[..got].iter().map(|&c| c as char).collect()
    };
    let cols: Vec<String> = head.lines().find(|l| l.starts_with("#cols")).map(|l| l.split(',').skip(1).map(|c| c.trim().to_string()).collect()).unwrap_or_default();
    let keep = |l: &str| {
        if l.starts_with("#cols") || l.trim().is_empty() {
            return false;
        }
        let f: Vec<&str> = l.split(',').collect();
        let t = f.first().and_then(|x| x.trim_start_matches('#').parse::<f64>().ok()).or_else(|| f.get(1).and_then(|x| x.parse::<f64>().ok()));
        let id = if l.starts_with('#') { f.get(2) } else { f.get(1) }.and_then(|x| x.trim().parse::<i64>().ok());
        bot.is_none_or(|b| id == Some(b) || l.starts_with("#mark")) && from.is_none_or(|x| t.is_none_or(|t| t >= x)) && to.is_none_or(|x| t.is_none_or(|t| t <= x))
    };
    let mut v = tail(file, cursor, n, &keep)?;
    let rows: Vec<Value> = v["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            if l.starts_with('#') || cols.is_empty() {
                return json!(l);
            }
            let mut o = Map::new();
            for (i, x) in f.iter().enumerate() {
                let k = cols.get(i).cloned().unwrap_or_else(|| format!("c{i}"));
                o.insert(k, x.parse::<f64>().map(|n| json!(n)).unwrap_or_else(|_| json!(x)));
            }
            Value::Object(o)
        })
        .collect();
    v["rows"] = json!(rows);
    v["columns"] = json!(cols);
    if let Some(o) = v.as_object_mut() {
        o.remove("lines");
    }
    Ok(v)
}

/// A recording trace narrowed to one bot / a time window, as named columns.
pub fn trace_window(v: &Value, a: &Value) -> Value {
    let cols: Vec<String> = v["columns"].as_array().map(|c| c.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let ti = cols.iter().position(|c| c == "t" || c == "time").unwrap_or(0);
    let (bot, from, to) = (a.get("bot").and_then(Value::as_i64), a.get("from").and_then(Value::as_f64), a.get("to").and_then(Value::as_f64));
    let every = a.get("every").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
    let n = a.get("lines").and_then(Value::as_u64).map(|n| (n as usize).clamp(1, 500)).unwrap_or(200);
    let in_window = |t: Option<f64>| t.is_none_or(|t| from.is_none_or(|f| t >= f) && to.is_none_or(|x| t <= x));
    let mut bots = vec![];
    let mut shown = 0;
    for b in v["bots"].as_array().into_iter().flatten() {
        if bot.is_some_and(|id| b["id"].as_i64() != Some(id)) {
            continue;
        }
        let rows: Vec<Value> = b["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| in_window(r.get(ti).and_then(Value::as_f64)))
            .step_by(every)
            .take(n.saturating_sub(shown))
            .map(|r| Value::Object(r.as_array().into_iter().flatten().enumerate().map(|(i, x)| (cols.get(i).cloned().unwrap_or_else(|| format!("c{i}")), x.clone())).collect()))
            .collect();
        shown += rows.len();
        bots.push(json!({"id": b["id"], "rows": rows}));
    }
    let notes = |k: &str| -> Vec<Value> { v[k].as_array().into_iter().flatten().filter(|e| bot.is_none_or(|id| e["id"].as_i64() == Some(id) || k == "marks") && in_window(e["t"].as_f64())).take(100).cloned().collect() };
    json!({"map": v["map"], "matches": v["matches"], "tasks": v["tasks"], "bots": bots, "why": notes("why"), "attack": notes("attack"), "seen": notes("seen"), "marks": notes("marks"), "truncated": shown >= n})
}

fn app_routes(search: &str) -> Value {
    let words: Vec<String> = search.to_lowercase().split_whitespace().map(String::from).collect();
    let list: Vec<Value> = routes::ROUTES
        .iter()
        .filter(|(m, p)| {
            let text = format!("{m} {p} {}", routes::area_of(p)).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|(m, p)| {
            let may = match routes::policy(m, &p.replace(':', "x"), &json!({"components": ["x"]})) {
                Policy::Read => "reads",
                Policy::Change => "changes (needs Allow changes)",
                Policy::Ask(_) => "asks the person first",
                Policy::UserOnly(_) => "the person's alone",
            };
            json!({"method": m, "path": p, "may": may, "area": routes::area_of(p)})
        })
        .collect();
    json!({"routes": list, "userOnly": routes::USER_ONLY_LIST})
}

async fn app_call(ctx: &Ctx, a: &Value) -> Done {
    let method = s(a, "method").unwrap_or("GET").to_ascii_uppercase();
    let path = s(a, "path").ok_or_else(|| err("Say the path, e.g. /api/state."))?;
    // (checked once and sent exactly as checked)
    let path = routes::canonical(path).map_err(|e| err(&e))?;
    let body = a.get("body").cloned().unwrap_or_else(|| json!({}));
    let query = a.get("query").and_then(Value::as_object).filter(|q| !q.is_empty());
    if query.is_some() && !routes::route_for(&method, path).is_some_and(|r| routes::query_allowed(&method, r)) {
        return Err(err("A query only goes with GET requests (and the screenshot route)."));
    }
    let policy = routes::policy(&method, path, &body);
    match &policy {
        Policy::UserOnly(why) => return Err(Fail::Refused(why.clone())),
        Policy::Read => {}
        _ if !ctx.changes => return Err(Fail::Refused("Changes are off. The person can turn on \"Allow changes\" under AI agents in Loner-Strike's Settings; until then only GET works.".into())),
        Policy::Ask(what) => ask(ctx, format!("An AI agent wants to {what} ({method} {path}). Allow it?")).await?,
        Policy::Change => {}
    }
    let target = match s(a, "target") {
        None | Some("") | Some("game") | Some("local") => None,
        Some(_) => Some(own_server(ctx, &json!({"server": a["target"]})).await?),
    };
    let mut full = path.to_string();
    if let Some(q) = query {
        let parts: Vec<String> = q
            .iter()
            .map(|(k, v)| {
                let v = v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
                format!("{}={}", enc(k), enc(&v))
            })
            .collect();
        if !parts.is_empty() {
            full = format!("{full}?{}", parts.join("&"));
        }
    }
    let m = if method == "POST" { Method::POST } else { Method::GET };
    super::catalog::call(ctx, m, &full, target.as_deref(), (method == "POST").then_some(body)).await
}

fn enc(v: &str) -> String {
    v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
