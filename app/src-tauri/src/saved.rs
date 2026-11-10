//! Settings saved for the next game: without a game running, the Bots, Rules, Fun and
//! Add-ons pages read and change the files the Game Mode Menu applies when a map starts
//! (data/gamemodes/global.cfg, mode_<id>.cfg, addons.txt, addon_<id>.cfg), written the way
//! the menu writes them. A running game holds these in memory and writes them back over
//! anything changed here, so while one runs the pages change it live and these refuse.
//!
//! What settings there are (names, choices, which page, saved for every mode or per mode)
//! comes from the plugin's own SETTINGS table, read from its source at build time, so the
//! pages work before a game has ever run.

use crate::app_api::{cfg_values, current_mode, effective, known_mode, save_bots, set_cfg_values};
use crate::game::Game;
use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

const PLUGIN: &str = include_str!("../../../plugins/scripting/gamemodes.sma");

/// The plugin's page numbers (CAT_*) and where a setting is saved (SC_*).
const CAT_HIDDEN: u8 = 0;
const CAT_BOT: u8 = 1;
const CAT_FUN: u8 = 3;
const SC_NONE: u8 = 0;
const SC_GLOBAL: u8 = 1;
const SC_MODE: u8 = 2;

/// One row of gamemodes.sma's SETTINGS table.
#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    pub cvar: String,
    pub name: String,
    pub values: String,
    pub labels: String,
    pub cat: u8,
    pub scope: u8,
}

/// The SETTINGS table's rows, from `{ "cvar", "Name", "values", "labels", CAT_X, SC_Y, "flag" }`.
pub fn parse_defs(src: &str) -> Vec<Def> {
    let Some(start) = src.find("new const SETTINGS[][SettingData] =") else { return vec![] };
    let mut out = Vec::new();
    for line in src[start..].lines().skip(2) {
        let l = line.trim();
        if l == "}" {
            break;
        }
        if !l.starts_with('{') {
            continue;
        }
        let quoted: Vec<&str> = l.split('"').skip(1).step_by(2).collect();
        let cat = ["CAT_HIDDEN", "CAT_BOT", "CAT_MATCH", "CAT_FUN"].iter().position(|c| l.contains(c));
        let scope = ["SC_NONE", "SC_GLOBAL", "SC_MODE"].iter().position(|c| l.contains(c));
        if let (true, Some(cat), Some(scope)) = (quoted.len() >= 4, cat, scope) {
            out.push(Def { cvar: quoted[0].into(), name: quoted[1].into(), values: quoted[2].into(), labels: quoted[3].into(), cat: cat as u8, scope: scope as u8 });
        }
    }
    out
}

fn defs() -> &'static [Def] {
    static DEFS: OnceLock<Vec<Def>> = OnceLock::new();
    DEFS.get_or_init(|| parse_defs(PLUGIN))
}

/// The bot and fun settings every mode shares: what Reset bot and fun settings clears.
fn shared_bot_fun(d: &Def) -> bool {
    d.scope == SC_GLOBAL && (d.cat == CAT_BOT || d.cat == CAT_FUN)
}

/// Pawn's parse(): words, or "quoted strings".
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap_or(r.len());
            out.push(r[..end].to_string());
            rest = r.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
        rest = rest.trim_start();
    }
    out
}

/// Lines of a config file that aren't blank or comments.
fn entries(text: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with('/')).map(words)
}

fn read(p: &Path) -> String {
    crate::sys::read_latin1(p).unwrap_or_default()
}

/// The last value a list of config files gives a cvar, in order.
fn last_value(files: &[std::path::PathBuf], cvar: &str) -> Option<String> {
    files.iter().flat_map(|p| cfg_values(p)).filter(|(k, _)| k.eq_ignore_ascii_case(cvar)).last().map(|(_, v)| v)
}

/// What the last game said (status.json stays after it closes): setting and add-on setting values.
fn last_status(g: &Game) -> HashMap<String, String> {
    read(&g.data.join("status.json"))
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["t"] == "setting" || v["t"] == "addonset")
        .filter_map(|v| Some((v["cvar"].as_str()?.to_lowercase(), v["value"].as_str()?.to_string())))
        .collect()
}

/// A setting's choices here: the Enhanced bot roster only where its file is (as setting_choices).
fn choices(g: &Game, d: &Def) -> (String, String) {
    if d.cvar == "bot_profile_db" && !g.dir.join("BotProfileEnhanced.db").is_file() {
        return ("BotProfile.db".into(), "Classic".into());
    }
    (d.values.clone(), d.labels.clone())
}

fn cfg_dir(g: &Game) -> std::path::PathBuf {
    g.dir.join("addons/amxmodx/configs/gamemodes")
}

/// The modes modes.ini lists, as the plugin reports them.
pub(crate) fn modes(g: &Game) -> Vec<Value> {
    entries(&read(&cfg_dir(g).join("modes.ini")))
        .filter(|w| w.len() >= 2 && crate::maps::safe_arg(&w[0]))
        .map(|w| {
            let get = |i: usize| w.get(i).cloned().unwrap_or_default();
            json!({"id": w[0], "name": w[1], "desc": get(2), "flags": get(4), "options": i32::from(!get(3).is_empty())})
        })
        .collect()
}

/// The add-ons addons.ini lists: [id, name, plugins, description, command].
fn addon_rows(g: &Game) -> Vec<Vec<String>> {
    entries(&read(&cfg_dir(g).join("addons.ini"))).filter(|w| w.len() >= 3 && crate::maps::safe_arg(&w[0])).collect()
}

fn addons_on(g: &Game) -> Vec<String> {
    read(&g.data.join("addons.txt")).lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect()
}

/// An add-on's settings (configs/gamemodes/addons/<id>.ini): [cvar, name, values, labels].
fn addon_sets(g: &Game, id: &str) -> Vec<Vec<String>> {
    entries(&read(&cfg_dir(g).join(format!("addons/{id}.ini")))).filter(|w| w.len() >= 4).collect()
}

/// Everything the pages show for the next game in `mode`.
fn saved_view(g: &Game, mode: &str) -> Value {
    let status = last_status(g);
    let settings: Vec<Value> = defs()
        .iter()
        // (live-only ones, like freezing the bots, aren't saved: nothing to set before a game)
        .filter(|d| d.cat != CAT_HIDDEN && d.scope != SC_NONE)
        .map(|d| {
            let (values, labels) = choices(g, d);
            let lower = d.cvar.to_lowercase();
            let value = effective(g, mode, &lower).or_else(|| status.get(&lower).cloned()).unwrap_or_default();
            json!({"cvar": d.cvar, "name": d.name, "cat": d.cat, "scope": d.scope, "value": value, "values": values, "labels": labels, "visible": 1})
        })
        .collect();
    let on = addons_on(g);
    let mut addons = Vec::new();
    let mut addonsets = Vec::new();
    for w in addon_rows(g) {
        let id = &w[0];
        let is_on = on.contains(&id.to_lowercase());
        // (offline nothing is loaded yet: what's on is what the next game loads)
        addons.push(json!({"id": id, "name": w[1], "desc": w.get(3).cloned().unwrap_or_default(), "on": i32::from(is_on), "loaded": i32::from(is_on)}));
        if !is_on {
            continue;
        }
        let files = [cfg_dir(g).join(format!("addons/{id}.cfg")), g.data.join(format!("addon_{id}.cfg"))];
        for s in addon_sets(g, id) {
            let value = last_value(&files, &s[0]).or_else(|| status.get(&s[0].to_lowercase()).cloned()).unwrap_or_default();
            addonsets.push(json!({"addon": id, "cvar": s[0], "name": s[1], "value": value, "values": s[2], "labels": s[3]}));
        }
    }
    let bots = cfg_values(&g.data.join("global.cfg")).into_iter().filter(|(k, _)| k == "bot_quota").last().and_then(|(_, v)| v.parse::<u32>().ok());
    let map = read(&g.data.join("lastmap.txt")).trim().to_string();
    let foundation = crate::setup::record::Record::open(&g.dir).component_ids().iter().any(|c| c == "foundation");
    json!({"mode": mode, "map": map, "bots": bots, "modes": modes(g), "settings": settings, "addons": addons, "addonsets": addonsets, "foundation": foundation})
}

/// Why the saved settings can't be changed from here now, if they can't.
fn blocked(g: &Game) -> Option<&'static str> {
    if g.remote.is_some() {
        return Some("Changing settings before a game isn't possible for a server on another machine yet. Start the server to change them.");
    }
    if crate::backup::restoring_game(&g.hl) {
        return Some(crate::backup::RESTORING_GAME);
    }
    // (starting counts too: the console doesn't answer for the first 10-30 seconds, and the
    // plugin writes what it holds back over a change made meanwhile once the map starts)
    let starting = if g.dedicated { crate::servers::running_at(&g.hl) } else { crate::game::game_open() };
    if starting || g.state()["online"] == json!(true) {
        return Some("The game is running now: change it live instead.");
    }
    None
}

type Reply = (StatusCode, Json<Value>);

fn answer(r: Result<Value, String>) -> Reply {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

/// Runs a change to the saved files, off the async threads, once nothing stands in its way.
async fn change(st: Shared, headers: HeaderMap, f: impl FnOnce(&Game) -> Result<(), String> + Send + 'static) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game() else { return answer(Err("Couldn't find the game.".into())) };
    let r = tokio::task::spawn_blocking(move || {
        // (one change at a time: two at once would each write back the file as they read it)
        static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(why) = blocked(&g) {
            return Err(why.to_string());
        }
        std::fs::create_dir_all(&g.data).map_err(|e| format!("Couldn't save it ({e})."))?;
        f(&g)
    })
    .await
    .unwrap_or_else(|_| Err("Couldn't save it.".into()));
    answer(r.map(|()| json!({"ok": true})))
}

#[derive(Deserialize)]
struct ViewQuery {
    #[serde(default)]
    mode: Option<String>,
}

/// The settings the next game starts with, for `mode` (or the mode it starts in).
async fn api_saved(State(st): State<Shared>, Query(q): Query<ViewQuery>) -> Json<Value> {
    let Some(g) = st.game() else { return Json(json!({"error": "Couldn't find the game."})) };
    if g.remote.is_some() {
        return Json(json!({"error": blocked(&g).unwrap_or_default()}));
    }
    let v = tokio::task::spawn_blocking(move || {
        let mode = q.mode.filter(|m| crate::maps::safe_arg(m) && known_mode(&g, m)).unwrap_or_else(|| current_mode(&g));
        saved_view(&g, &mode)
    })
    .await
    .unwrap_or_else(|_| json!({"error": "Couldn't read the settings."}));
    Json(v)
}

#[derive(Deserialize)]
struct SetReq {
    /// the mode a per-mode setting is saved for (the mode it starts in when left out)
    #[serde(default)]
    mode: Option<String>,
    cvar: String,
    value: String,
}

/// Where one setting goes, checked against the plugin's table: (file, cvar, value).
fn place(g: &Game, req: &SetReq) -> Result<std::path::PathBuf, String> {
    let d = defs().iter().find(|d| d.cvar.eq_ignore_ascii_case(&req.cvar) && d.cat != CAT_HIDDEN).ok_or("That isn't a setting here.")?;
    if !choices(g, d).0.split_whitespace().any(|v| v == req.value) {
        return Err(format!("{} isn't a choice for {}.", req.value, d.name));
    }
    match d.scope {
        SC_GLOBAL => Ok(g.data.join("global.cfg")),
        SC_MODE => {
            let mode = req.mode.clone().unwrap_or_else(|| current_mode(g));
            if !crate::maps::safe_arg(&mode) || !known_mode(g, &mode) {
                return Err("That mode isn't installed.".into());
            }
            Ok(g.data.join(format!("mode_{mode}.cfg")))
        }
        _ => Err("That setting isn't saved.".into()),
    }
}

async fn api_saved_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<SetReq>) -> Reply {
    change(st, headers, move |g| {
        let file = place(g, &req)?;
        set_cfg_values(&file, &[(req.cvar.to_lowercase(), req.value)]).map_err(|e| format!("Couldn't save the setting ({e})."))
    })
    .await
}

#[derive(Deserialize)]
struct BotsReq {
    bots: u32,
}

async fn api_saved_bots(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<BotsReq>) -> Reply {
    change(st, headers, move |g| save_bots(&g.dir, req.bots).map_err(|e| format!("Couldn't save the bot count ({e}).")))
        .await
}

#[derive(Deserialize)]
struct AddonReq {
    id: String,
    /// switch it on or off...
    #[serde(default)]
    on: Option<bool>,
    /// ...or change one of its settings
    #[serde(default)]
    cvar: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

async fn api_saved_addon(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<AddonReq>) -> Reply {
    change(st, headers, move |g| {
        let ini = cfg_dir(g).join("addons.ini");
        if let Some(on) = req.on {
            // what the plugin writes when one is switched (sounds.rs does the same for Sounds)
            let (txt, plugins) = crate::sounds::write_addons_offline(&read(&ini), &read(&g.data.join("addons.txt")), &req.id, on)?;
            crate::sys::write_latin1_atomic(&g.data.join("addons.txt"), &txt).map_err(|e| e.to_string())?;
            crate::sys::write_latin1_atomic(&g.dir.join("addons/amxmodx/configs/plugins-gmaddons.ini"), &plugins).map_err(|e| e.to_string())?;
            return Ok(());
        }
        let (Some(cvar), Some(value)) = (req.cvar, req.value) else { return Err("Nothing to change.".into()) };
        let id = addon_rows(g).into_iter().map(|w| w[0].clone()).find(|i| i.eq_ignore_ascii_case(&req.id)).ok_or("That add-on isn't installed.")?;
        let set = addon_sets(g, &id).into_iter().find(|s| s[0].eq_ignore_ascii_case(&cvar)).ok_or("That isn't one of its settings.")?;
        if !set[2].split_whitespace().any(|v| v == value) || !crate::maps::safe_arg(&value) {
            return Err(format!("{value} isn't a choice for {}.", set[1]));
        }
        set_cfg_values(&g.data.join(format!("addon_{id}.cfg")), &[(set[0].clone(), value)]).map_err(|e| format!("Couldn't save the setting ({e})."))
    })
    .await
}

#[derive(Deserialize)]
struct ResetReq {
    /// "shared": every bot and fun setting all modes share; "mode": one mode's own settings
    what: String,
    #[serde(default)]
    mode: Option<String>,
}

/// global.cfg without the shared bot and fun settings (the bot count and the rest stay).
fn without_shared(text: &str) -> String {
    let keep: Vec<&str> = text
        .lines()
        .filter(|l| {
            let cvar = l.split_whitespace().next().unwrap_or("");
            !defs().iter().any(|d| shared_bot_fun(d) && d.cvar.eq_ignore_ascii_case(cvar))
        })
        .collect();
    keep.join("\n") + "\n"
}

async fn api_saved_reset(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ResetReq>) -> Reply {
    change(st, headers, move |g| match req.what.as_str() {
        "shared" => {
            let path = g.data.join("global.cfg");
            if !path.is_file() {
                return Ok(());
            }
            crate::sys::write_latin1_atomic(&path, &without_shared(&read(&path))).map_err(|e| format!("Couldn't reset them ({e})."))
        }
        "mode" => {
            let mode = req.mode.unwrap_or_else(|| current_mode(g));
            if !crate::maps::safe_arg(&mode) || !known_mode(g, &mode) {
                return Err("That mode isn't installed.".into());
            }
            match std::fs::remove_file(g.data.join(format!("mode_{mode}.cfg"))) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Couldn't reset them ({e}).")),
                _ => Ok(()),
            }
        }
        _ => Err("Nothing to reset.".into()),
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/app/saved-settings", get(api_saved).post(api_saved_set))
        .route("/api/app/saved-settings/bots", post(api_saved_bots))
        .route("/api/app/saved-settings/addon", post(api_saved_addon))
        .route("/api/app/saved-settings/reset", post(api_saved_reset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_table_reads() {
        let d = defs();
        assert!(d.len() > 100);
        let style = d.iter().find(|d| d.cvar == "gm_botstyle").unwrap();
        assert_eq!((style.cat, style.scope), (CAT_BOT, SC_GLOBAL));
        assert!(style.labels.starts_with("Original|"));
        let ff = d.iter().find(|d| d.cvar == "mp_friendlyfire").unwrap();
        assert_eq!((ff.values.as_str(), ff.scope), ("0 1", SC_MODE));
        // the last row, without a comma after it
        assert!(d.iter().any(|d| d.cvar == "gm_infmoney"));
        assert_eq!(d.iter().find(|d| d.cvar == "bot_quota").unwrap().cat, CAT_HIDDEN);
    }

    /// Reset brings every shared bot and fun setting back from base.cfg, so each needs its default there.
    #[test]
    fn base_cfg_has_every_shared_default() {
        let base = include_str!("../../../plugins/configs/gamemodes/base.cfg");
        let set: Vec<String> = base.lines().filter_map(|l| l.split_whitespace().next()).map(str::to_lowercase).collect();
        // (the roster isn't: a bot style picks its own, and the plugin resets it by itself)
        let missing: Vec<&str> = defs().iter().filter(|d| shared_bot_fun(d) && d.cvar != "bot_profile_db" && !set.contains(&d.cvar)).map(|d| d.cvar.as_str()).collect();
        assert!(missing.is_empty(), "base.cfg has no default for {missing:?}");
    }

    #[test]
    fn reset_keeps_count_and_rules() {
        let text = "// Saved by the Game Mode Menu\nbot_quota \"9\"\ngm_botstyle \"6\"\nGM_HB_AIM \"0\"\ngm_rtv \"0\"\ngm_tracers \"2\"\ngm_mappresets \"0\"\n";
        assert_eq!(without_shared(text), "// Saved by the Game Mode Menu\nbot_quota \"9\"\ngm_rtv \"0\"\ngm_mappresets \"0\"\n");
    }

    #[test]
    fn pawn_words() {
        assert_eq!(words(r#"hook   "Grappling hook"  "a.amxx b.amxx"  "Bind a key"  """#), vec!["hook", "Grappling hook", "a.amxx b.amxx", "Bind a key", ""]);
        assert_eq!(words("csb_hook_pull \"Pull speed\" \"500 700\" \"Slow|Fast\""), vec!["csb_hook_pull", "Pull speed", "500 700", "Slow|Fast"]);
    }
}
