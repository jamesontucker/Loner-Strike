//! Admins, VIPs and bans, like a real server: the host gives people powers by SteamID.
//!
//! - People live in <data>/admins.json ({id, name, role, expires, note}); each change
//!   rewrites a marked Loner-Strike block in addons/amxmodx/configs/users.ini (anything
//!   else in that file is kept) and asks the running game to reload admins.
//! - Roles are AMX Mod X access flags; "t" is the VIP flag gm_vip.amxx looks for.
//! - VIP perks, the ability grid (which roles get the hook, noclip...) and the donate
//!   link go to <data>/vip.cfg, which gm_vip.amxx runs.
//!   Donations happen outside the app (Ko-fi, PayPal...); the host grants VIP here.
//! - Bans are the engine's own: banned.cfg (banid) and listip.cfg (addip) in the game folder.
//!
//! Everything sent to the game is built from validated SteamIDs / IPs and fixed words.

use crate::game::Game;
use crate::panel_api::from_panel;
use crate::rcon::RconError;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Mutex;

pub(crate) const BEGIN: &str = "; ---- Loner-Strike: people from the app's Admins page. This block is rewritten automatically; edit it there. ----";
pub(crate) const END: &str = "; ---- end of Loner-Strike ----";
/// One change at a time to admins.json, users.ini and the ban files
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Admin,
    Moderator,
    Vip,
}

impl Role {
    pub fn word(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Moderator => "moderator",
            Role::Vip => "vip",
        }
    }

    /// AMX Mod X access flags (a immunity, b reserved slot, c kick, d ban, e slay, f map,
    /// g cvar, h cfg, i chat, j vote, k sv_password, l rcon, m-t custom, u menu).
    /// The role's bit in gm_vip's ability cvars.
    pub fn bit(self) -> u8 {
        match self {
            Role::Vip => 1,
            Role::Moderator => 2,
            Role::Admin => 4,
            Role::Owner => 8,
        }
    }

    pub fn flags(self) -> &'static str {
        match self {
            Role::Owner => "abcdefghijklmnopqrstu",
            Role::Admin => "bcdefijtu",
            Role::Moderator => "ceiju",
            Role::Vip => "bt",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Person {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub role: Role,
    /// Last day it counts (YYYY-MM-DD), none for good
    #[serde(default)]
    pub expires: Option<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Perks {
    pub armor: bool,
    pub defuse: bool,
    pub nades: bool,
    /// extra money each round, 0 = off
    pub money: u32,
    pub tag: bool,
    pub chat: bool,
    /// the host (who has every flag anyway) gets the perks too
    pub host: bool,
}

impl Default for Perks {
    fn default() -> Self {
        Perks { armor: true, defuse: true, nades: true, money: 500, tag: true, chat: true, host: false }
    }
}

/// Which roles get each ability (the Admins page grid). Missing ones get the defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Abilities {
    /// the spawn perks and money (Perks)
    pub gear: Vec<Role>,
    /// the grappling hook add-on
    pub hook: Vec<Role>,
    pub jump: Vec<Role>,
    pub respawn: Vec<Role>,
    pub noclip: Vec<Role>,
    pub god: Vec<Role>,
    /// say /save, /tp
    pub tp: Vec<Role>,
    /// double jump, respawn, noclip and god in modes they'd break too
    pub every_mode: bool,
}

const VIP_UP: [Role; 4] = [Role::Vip, Role::Moderator, Role::Admin, Role::Owner];

impl Default for Abilities {
    fn default() -> Self {
        Abilities {
            gear: VIP_UP.to_vec(),
            hook: VIP_UP.to_vec(),
            jump: vec![],
            respawn: vec![],
            noclip: vec![Role::Admin, Role::Owner],
            god: vec![Role::Owner],
            tp: vec![Role::Admin, Role::Owner],
            every_mode: false,
        }
    }
}

/// Bitmask of roles for a gm_vip cvar.
fn role_mask(roles: &[Role]) -> u8 {
    roles.iter().fold(0, |m, r| m | r.bit())
}

impl Abilities {
    /// (gm_vip cvar, roles) for each ability
    fn cvars(&self) -> [(&'static str, &Vec<Role>); 7] {
        [
            ("gm_vip_gear", &self.gear),
            ("gm_vip_hook", &self.hook),
            ("gm_vip_jump", &self.jump),
            ("gm_vip_respawn", &self.respawn),
            ("gm_vip_noclip", &self.noclip),
            ("gm_vip_god", &self.god),
            ("gm_vip_tp", &self.tp),
        ]
    }

    /// Each list in VIP..Owner order without repeats.
    fn tidy(mut self) -> Self {
        for list in [&mut self.gear, &mut self.hook, &mut self.jump, &mut self.respawn, &mut self.noclip, &mut self.god, &mut self.tp] {
            let m = role_mask(list);
            *list = VIP_UP.iter().copied().filter(|r| m & r.bit() != 0).collect();
        }
        self
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Admins {
    #[serde(default)]
    pub people: Vec<Person>,
    #[serde(default)]
    pub donate: String,
    #[serde(default)]
    pub perks: Perks,
    #[serde(default)]
    pub abilities: Abilities,
}

// ---- checks ------------------------------------------------------------------

/// STEAM_X:Y:Z only (no STEAM_ID_LAN, BOT, VALVE_ID_...).
pub fn valid_steamid(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("STEAM_") else { return false };
    let mut parts = rest.split(':');
    let (Some(x), Some(y), Some(z), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
    let digits = |p: &str, max: usize| !p.is_empty() && p.len() <= max && p.bytes().all(|c| c.is_ascii_digit());
    digits(x, 1) && x.as_bytes()[0] <= b'5' && (y == "0" || y == "1") && digits(z, 10) && z.parse::<u32>().is_ok() && !(z.len() > 1 && z.starts_with('0'))
}

/// YYYY-MM-DD with a real month and day.
pub fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s.get(r).filter(|p| p.bytes().all(|c| c.is_ascii_digit())).and_then(|p| p.parse::<u32>().ok());
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else { return false };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (2000..=2999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= days[m as usize - 1]
}

/// Today's date (UTC) as YYYY-MM-DD.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    civil((secs / 86400) as i64)
}

/// Days since 1970-01-01 to YYYY-MM-DD (Howard Hinnant's civil_from_days).
fn civil(days: i64) -> String {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The last day counts; ISO dates compare as text.
/// Whether someone has one of these roles on this game or server (and it hasn't run out).
pub fn has_role(g: &Game, steamid: &str, roles: &[Role]) -> bool {
    let today = today();
    load(&g.data).people.iter().any(|p| p.id.eq_ignore_ascii_case(steamid) && roles.contains(&p.role) && !expired(p, &today))
}

pub fn expired(p: &Person, today: &str) -> bool {
    p.expires.as_deref().map(|e| e < today).unwrap_or(false)
}

/// Labels and notes go into comments: one line, no control characters.
fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// http(s) links only, nothing that could end the cfg line or its quotes.
fn valid_url(s: &str) -> bool {
    (s.starts_with("https://") || s.starts_with("http://"))
        && s.len() > 10
        && s.len() <= 120
        && s.bytes().all(|c| c.is_ascii_graphic() && !b"\"';\\<>`".contains(&c))
}

// ---- files ---------------------------------------------------------------------

fn load(data: &Path) -> Admins {
    let file = data.join("admins.json");
    match std::fs::read_to_string(&file) {
        Ok(t) => serde_json::from_str(&t).unwrap_or_else(|_| {
            crate::sys::set_aside(&file);
            Admins::default()
        }),
        Err(_) => Admins::default(),
    }
}

fn save(data: &Path, a: &Admins) -> Result<(), String> {
    let text = serde_json::to_string_pretty(a).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&data.join("admins.json"), text.as_bytes()).map_err(|e| format!("Couldn't save the admin list ({e})."))
}

/// The Loner-Strike block for users.ini (expired people left out).
fn users_block(people: &[Person], today: &str, nl: &str) -> String {
    let mut out = vec![BEGIN.to_string()];
    for p in people.iter().filter(|p| valid_steamid(&p.id) && !expired(p, today)) {
        let label = clean_text(if p.name.is_empty() { &p.id } else { &p.name }, 40);
        let until = p.expires.as_deref().map(|e| format!(", until {e}")).unwrap_or_default();
        out.push(format!("; {label} ({}{until})", p.role.word()));
        // "ce": the auth is a SteamID, no password
        out.push(format!("\"{}\" \"\" \"{}\" \"ce\"", p.id, p.role.flags()));
    }
    out.push(END.to_string());
    out.join(nl)
}

/// users.ini with our block replaced (or added at the end); everything else is kept.
/// users.ini without Loner-Strike's block (removing Loner-Strike from a game whose
/// AMX Mod X was there before it: the admins it added stop being admins).
pub fn strip_users(text: &str) -> String {
    if let (Some(s), Some(e)) = (text.find(BEGIN), text.find(END)) {
        if e > s {
            let rest = format!("{}{}", &text[..s], &text[e + END.len()..]);
            let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
            return format!("{}{nl}", rest.trim_end_matches(['\r', '\n']));
        }
    }
    text.to_string()
}

pub fn rewrite_users(text: &str, people: &[Person], today: &str) -> String {
    let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
    let block = users_block(people, today, nl);
    if let (Some(s), Some(e)) = (text.find(BEGIN), text.find(END)) {
        if e > s {
            return format!("{}{block}{}", &text[..s], &text[e + END.len()..]);
        }
    }
    let base = text.trim_end_matches(['\r', '\n']);
    if base.is_empty() {
        format!("{block}{nl}")
    } else {
        format!("{base}{nl}{nl}{block}{nl}")
    }
}

fn vip_cfg(a: &Admins) -> String {
    let p = &a.perks;
    let b = |v: bool| if v { 1 } else { 0 };
    let donate = if valid_url(&a.donate) { a.donate.as_str() } else { "" };
    let mut out = format!(
        "// Written by Loner-Strike's Admins page. Change it there.\r\ngm_vip_armor {}\r\ngm_vip_defuse {}\r\ngm_vip_nades {}\r\ngm_vip_money {}\r\ngm_vip_tag {}\r\ngm_vip_chat {}\r\ngm_vip_host {}\r\ngm_vip_donate \"{donate}\"\r\n",
        b(p.armor), b(p.defuse), b(p.nades), p.money.min(16000), b(p.tag), b(p.chat), b(p.host)
    );
    // abilities: which roles get each one (1 vip, 2 moderator, 4 admin, 8 owner)
    for (cvar, roles) in a.abilities.cvars() {
        out.push_str(&format!("{cvar} {}\r\n", role_mask(roles)));
    }
    out.push_str(&format!("gm_vip_anymode {}\r\n", b(a.abilities.every_mode)));
    out
}

/// Writes users.ini's block and vip.cfg. Ok(true) when users.ini changed.
fn write_files(g: &Game, a: &Admins) -> Result<bool, String> {
    const USERS: &str = "addons/amxmodx/configs/users.ini";
    let users = g.dir.join(USERS);
    let mut changed = false;
    if users.parent().map(|d| d.is_dir()).unwrap_or(false) {
        // on a Linux server, the machine's users.ini: admins added in the game (amx_addadmin)
        // are only there, and must not be written over by this computer's copy. A failed
        // read is an error, never an empty file
        let old = match &g.remote {
            Some(_) => crate::remote::game_file(g, USERS).map_err(|e| format!("Couldn't read the server's users.ini ({e})."))?.map(|b| crate::rcon::latin1(&b)).unwrap_or_default(),
            None => crate::sys::read_latin1(&users).unwrap_or_default(),
        };
        let new = rewrite_users(&old, &a.people, &today());
        if new != old {
            if g.remote.is_some() {
                let bytes: Vec<u8> = new.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
                crate::remote::put_game_file(g, USERS, &bytes).map_err(|e| format!("Couldn't write users.ini ({e})."))?;
            } else {
                crate::sys::write_latin1(&users, &new).map_err(|e| format!("Couldn't write users.ini ({e})."))?;
            }
            changed = true;
        }
    }
    let cfg = g.data.join("vip.cfg");
    let text = vip_cfg(a);
    // only into a game (or server copy) that has Loner-Strike
    if g.data.is_dir() && std::fs::read_to_string(&cfg).ok().as_deref() != Some(text.as_str()) {
        let _ = std::fs::write(&cfg, text);
    }
    Ok(changed)
}

/// The closest role for an existing users.ini entry's access flags.
fn role_for(access: &str) -> Option<Role> {
    let has = |c: char| access.contains(c);
    if has('l') || has('h') {
        Some(Role::Owner)
    } else if has('d') || has('f') {
        Some(Role::Admin)
    } else if has('c') || has('e') {
        Some(Role::Moderator)
    } else if has('b') || has('t') {
        Some(Role::Vip)
    } else {
        None
    }
}

/// The four quoted fields of a users.ini line.
fn users_fields(line: &str) -> Option<[String; 4]> {
    let parts: Vec<&str> = line.split('"').collect();
    // "a" "b" "c" "d" -> ["", a, " ", b, " ", c, " ", d, ...]
    if parts.len() < 9 {
        return None;
    }
    Some([parts[1].into(), parts[3].into(), parts[5].into(), parts[7].into()])
}

/// users.ini text with SteamID entries moved out (left as comments), and the people moved.
pub fn take_users(text: &str) -> (String, Vec<Person>) {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut people = Vec::new();
    let mut out = Vec::new();
    let mut in_block = false;
    let mut label = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t == BEGIN {
            in_block = true;
        }
        if in_block {
            if t == END {
                in_block = false;
            }
            out.push(line.to_string());
            continue;
        }
        if let Some(c) = t.strip_prefix(';') {
            label = c.trim().to_string();
            out.push(line.to_string());
            continue;
        }
        match users_fields(t) {
            Some([auth, _pw, access, flags]) if valid_steamid(&auth) && flags.contains('c') => match role_for(&access) {
                Some(role) => {
                    let name = if label.len() <= 40 && !label.is_empty() { label.clone() } else { String::new() };
                    people.push(Person { id: auth, name, role, expires: None, note: format!("Imported (was \"{access}\")") });
                    out.push(format!("; moved to Loner-Strike's Admins page: {line}"));
                }
                None => out.push(line.to_string()),
            },
            _ => out.push(line.to_string()),
        }
        label.clear();
    }
    let mut text = out.join(nl);
    text.push_str(nl);
    (text, people)
}

/// Brings an existing server's SteamID admins into the Admins page. Returns how many.
pub fn import_users(game_dir: &Path, data: &Path) -> usize {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let users = game_dir.join("addons/amxmodx/configs/users.ini");
    let Ok(old) = crate::sys::read_latin1(&users) else { return 0 };
    let (rest, found) = take_users(&old);
    let mut a = load(data);
    let mut n = 0;
    for p in found {
        if !a.people.iter().any(|x| x.id == p.id) {
            a.people.push(p);
            n += 1;
        }
    }
    if save(data, &a).is_err() {
        return 0;
    }
    let new = rewrite_users(&rest, &a.people, &today());
    if crate::sys::write_latin1(&users, &new).is_err() {
        return 0;
    }
    n
}

/// At app start: bring users.ini and vip.cfg up to date (drops anyone whose time ran out).
pub fn sync(g: &Game) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _ = write_files(g, &load(&g.data));
}

/// A fixed command for the running game (never free text). Offline is fine.
fn tell_game(g: &Game, line: &str) -> Result<bool, String> {
    match g.rcon.send(line) {
        Ok(_) => Ok(true),
        Err(RconError::Offline) => Ok(false),
        Err(RconError::BadPassword) => Err("The game didn't accept Loner-Strike's console password. Restart the map once.".into()),
    }
}

fn reload(g: &Game) {
    let _ = tell_game(g, "amx_reloadadmins; gm_vipreload");
}

// ---- bans ------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct Ban {
    /// "id" (SteamID, banned.cfg) or "ip" (listip.cfg)
    pub kind: &'static str,
    pub value: String,
}

fn ban_file(kind: &str) -> &'static str {
    if kind == "ip" { "listip.cfg" } else { "banned.cfg" }
}

/// "banid 0.0 STEAM_0:1:2" / "addip 0.0 1.2.3.4" lines (only permanent bans are saved there).
pub fn parse_bans(text: &str, kind: &'static str) -> Vec<Ban> {
    let word = if kind == "ip" { "addip" } else { "banid" };
    text.lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            (t.len() >= 3 && t[0].eq_ignore_ascii_case(word)).then(|| t[2].to_string())
        })
        .filter(|v| if kind == "ip" { v.parse::<Ipv4Addr>().is_ok() } else { valid_steamid(v) })
        .map(|value| Ban { kind, value })
        .collect()
}

fn read_bans(g: &Game) -> Result<Vec<Ban>, String> {
    let mut out = Vec::new();
    for kind in ["id", "ip"] {
        out.extend(parse_bans(&ban_text(g, kind)?, kind));
    }
    Ok(out)
}

/// A ban file as the server has it: on a Linux server, the one on the machine (the game
/// adds to it itself), not this computer's copy.
fn ban_text(g: &Game, kind: &str) -> Result<String, String> {
    match &g.remote {
        // only a file that isn't there counts as no bans: a failed read must never look
        // like an empty list (it would be written back, or lift every ban when shared)
        Some((r, dir)) => Ok(crate::remote::read_file_opt(r, &format!("{dir}/{}", ban_file(kind)))?.map(|b| crate::rcon::latin1(&b)).unwrap_or_default()),
        None => match crate::sys::read_latin1(&g.dir.join(ban_file(kind))) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("Couldn't read {} ({e}).", ban_file(kind))),
        },
    }
}

/// The ban file without lines for value.
pub fn remove_ban_line(text: &str, value: &str) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = text.lines().filter(|l| l.split_whitespace().nth(2) != Some(value)).collect();
    if kept.is_empty() { String::new() } else { kept.join(nl) + nl }
}

fn edit_ban_file(g: &Game, kind: &str, f: impl FnOnce(&str) -> String) -> Result<(), String> {
    let path = g.dir.join(ban_file(kind));
    let old = ban_text(g, kind)?;
    let new = f(&old);
    if new != old {
        if let Some((r, dir)) = &g.remote {
            let bytes: Vec<u8> = new.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
            crate::remote::write_file(r, &format!("{dir}/{}", ban_file(kind)), &bytes)?;
        }
        // (on a Linux server this computer's copy is only kept for sharing the list)
        crate::sys::write_latin1(&path, &new).map_err(|e| format!("Couldn't write {} ({e}).", ban_file(kind)))?;
    }
    Ok(())
}

// ---- API ------------------------------------------------------------------------------

/// Runs f with the game on a blocking thread (file and RCON work). The game accessor
/// lives only here.
async fn with_game<T: Send + 'static>(st: &Shared, f: impl FnOnce(&Game) -> T + Send + 'static) -> Option<T> {
    let g = st.game()?;
    tokio::task::spawn_blocking(move || f(&g)).await.ok()
}

// ---- everyone who's played here ---------------------------------------------------

/// Someone who joined (from the plugin's seen.log, kept in seen.json).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Seen {
    pub id: String,
    pub name: String,
    /// other names they've used, newest first
    #[serde(default)]
    pub aka: Vec<String>,
    pub first: i64,
    pub last: i64,
    pub visits: u32,
}

const MAX_SEEN: usize = 5000;

/// Folds new seen.log lines into the list (newest first).
pub fn fold_seen(mut list: Vec<Seen>, log: &str) -> Vec<Seen> {
    for line in log.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(t), Some(id), Some(name)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let (Ok(t), id, name) = (t.trim().parse::<i64>(), id.trim(), clean_text(name, 32)) else { continue };
        if !valid_steamid(id) {
            continue;
        }
        match list.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                if s.name != name && !name.is_empty() {
                    let old = std::mem::replace(&mut s.name, name.clone());
                    s.aka.retain(|n| *n != old && *n != name);
                    s.aka.insert(0, old);
                    s.aka.truncate(5);
                }
                s.last = s.last.max(t);
                s.first = s.first.min(t);
                s.visits += 1;
            }
            None => list.push(Seen { id: id.into(), name, aka: vec![], first: t, last: t, visits: 1 }),
        }
    }
    list.sort_by(|a, b| b.last.cmp(&a.last));
    list.truncate(MAX_SEEN);
    list
}

/// seen() for a game or server: on another machine the plugin's new lines are fetched
/// from it first (moved aside and printed there in one command).
fn seen_of(g: &Game) -> Vec<Seen> {
    let _ = crate::remote::take_log(g, "addons/amxmodx/data/gamemodes/seen.log");
    seen(&g.data)
}

/// The list, after taking in whatever the plugin wrote since last time.
fn seen(data: &Path) -> Vec<Seen> {
    let file = data.join("seen.json");
    let list: Vec<Seen> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // move the log aside first, so lines the plugin writes meanwhile start a new one
    let log = data.join("seen.log");
    let taking = data.join("seen.log.reading");
    if !taking.exists() && std::fs::rename(&log, &taking).is_err() {
        return list;
    }
    let text = crate::sys::read_latin1(&taking).unwrap_or_default();
    let list = fold_seen(list, &text);
    if std::fs::write(&file, serde_json::to_string(&list).unwrap_or_default()).is_ok() {
        let _ = std::fs::remove_file(&taking);
    }
    list
}

fn listing(g: &Game) -> Value {
    let a = load(&g.data);
    let today = today();
    let people: Vec<Value> = a
        .people
        .iter()
        .map(|p| {
            let mut v = serde_json::to_value(p).unwrap_or_default();
            v["expired"] = json!(expired(p, &today));
            v
        })
        .collect();
    json!({"people": people, "donate": a.donate, "perks": a.perks, "abilities": a.abilities, "bans": read_bans(g).unwrap_or_default(), "today": today,
           "users_ini": g.dir.join("addons/amxmodx/configs").is_dir(), "seen": seen_of(g)})
}

type Reply = (StatusCode, Json<Value>);

fn bad(msg: impl Into<String>) -> Reply {
    (StatusCode::BAD_REQUEST, Json(json!({"ok": false, "error": msg.into()})))
}

fn answer(r: Option<Result<Value, String>>) -> Reply {
    match r {
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"ok": false, "error": "No game found."}))),
        Some(Ok(v)) => (StatusCode::OK, Json(v)),
        Some(Err(e)) => bad(e),
    }
}

async fn api_list(State(st): State<Shared>) -> Reply {
    answer(
        with_game(&st, |g| {
            let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // a VIP whose time ran out while the app was open leaves users.ini now
            if write_files(g, &load(&g.data)).unwrap_or(false) {
                reload(g);
            }
            Ok(listing(g))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct PersonReq {
    id: String,
    #[serde(default)]
    name: String,
    role: Role,
    #[serde(default)]
    expires: Option<String>,
    #[serde(default)]
    note: String,
    /// the SteamID being edited, when it changes
    #[serde(default)]
    was: Option<String>,
}

fn change(g: &Game, f: impl FnOnce(&mut Admins) -> Result<(), String>) -> Result<Value, String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = load(&g.data);
    f(&mut a)?;
    save(&g.data, &a)?;
    write_files(g, &a)?;
    reload(g);
    Ok(listing(g))
}

/// The note on admins shared control adds, so it only ever changes or removes its own
pub const SHARED_NOTE: &str = "Shared control";

/// Shared control (control/): someone who helps run the server is an in-game admin with
/// the matching role (None: no longer). An admin the owner added by hand with that SteamID
/// is left as it is.
pub fn set_helper(g: &Game, steam: &str, name: &str, role: Option<Role>) -> Result<(), String> {
    let steam = steam.trim().to_uppercase();
    if !valid_steamid(&steam) {
        return Ok(());
    }
    change(g, |a| {
        match a.people.iter().position(|p| p.id == steam) {
            Some(i) if a.people[i].note != SHARED_NOTE => {}
            Some(i) => match role {
                Some(r) => {
                    a.people[i].role = r;
                    a.people[i].name = clean_text(name, 32);
                }
                None => {
                    a.people.remove(i);
                }
            },
            None => {
                if let Some(r) = role {
                    a.people.push(Person { id: steam.clone(), name: clean_text(name, 32), role: r, expires: None, note: SHARED_NOTE.into() });
                }
            }
        }
        Ok(())
    })
    .map(|_| ())
}

async fn api_person(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<PersonReq>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let id = req.id.trim().to_uppercase();
    if !valid_steamid(&id) {
        return bad("That isn't a SteamID. It looks like STEAM_0:1:12345. LAN and bot IDs can't be used.");
    }
    let expires = req.expires.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    if expires.as_deref().is_some_and(|e| !valid_date(e)) {
        return bad("That end date isn't a real date.");
    }
    let person = Person { id: id.clone(), name: clean_text(&req.name, 32), role: req.role, expires, note: clean_text(&req.note, 120) };
    let was = req.was.map(|w| w.trim().to_uppercase());
    answer(
        with_game(&st, move |g| {
            change(g, |a| {
                let key = was.unwrap_or_else(|| id.clone());
                if key != id && a.people.iter().any(|p| p.id == id) {
                    return Err("Someone with that SteamID is already on the list.".into());
                }
                match a.people.iter_mut().find(|p| p.id == key) {
                    Some(p) => *p = person,
                    None => a.people.push(person),
                }
                Ok(())
            })
        })
        .await,
    )
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn api_remove(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<IdReq>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    answer(with_game(&st, move |g| change(g, |a| Ok(a.people.retain(|p| p.id != req.id)))).await)
}

#[derive(Deserialize)]
struct SettingsReq {
    donate: String,
    perks: Perks,
    /// left out: unchanged
    #[serde(default)]
    abilities: Option<Abilities>,
}

async fn api_settings(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<SettingsReq>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let donate = req.donate.trim().to_string();
    if !donate.is_empty() && !valid_url(&donate) {
        return bad("The donate link has to be a web address starting with https:// (no quotes or spaces).");
    }
    let mut perks = req.perks;
    perks.money = perks.money.min(16000);
    let abilities = req.abilities.map(Abilities::tidy);
    answer(
        with_game(&st, move |g| {
            change(g, |a| {
                a.donate = donate;
                a.perks = perks;
                if let Some(ab) = abilities {
                    a.abilities = ab;
                }
                Ok(())
            })
        })
        .await,
    )
}

#[derive(Deserialize)]
struct BanReq {
    id: String,
    /// 0 = for good
    #[serde(default)]
    minutes: u32,
}

async fn api_ban(State(st): State<Shared>, headers: HeaderMap, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<BanReq>) -> Reply {
    // someone helping run it (shared control) gets no copy of the owner's Admins list
    let helper = helper.is_some();
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let id = req.id.trim().to_uppercase();
    if !valid_steamid(&id) {
        return bad("That isn't a SteamID. It looks like STEAM_0:1:12345. LAN and bot IDs can't be banned.");
    }
    let minutes = req.minutes.min(525_600);
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let online = tell_game(g, &format!("banid {minutes} {id} kick"))?;
            if minutes > 0 {
                if !online {
                    return Err("Bans for a set time need the game running. A permanent ban works any time.".into());
                }
            } else {
                // saved by hand instead of "writeid", which would overwrite bans the game hasn't loaded
                edit_ban_file(g, "id", |t| {
                    if parse_bans(t, "id").iter().any(|b| b.value == id) {
                        return t.to_string();
                    }
                    let nl = if t.contains("\r\n") || t.is_empty() { "\r\n" } else { "\n" };
                    let base = t.trim_end_matches(['\r', '\n']);
                    format!("{base}{}banid 0.0 {id}{nl}", if base.is_empty() { "" } else { nl })
                })?;
            }
            Ok(if helper { json!({"ok": true}) } else { listing(g) })
        })
        .await,
    )
}

#[derive(Deserialize)]
struct UnbanReq {
    kind: String,
    value: String,
}

async fn api_unban(State(st): State<Shared>, headers: HeaderMap, helper: Option<axum::Extension<crate::control::Helper>>, Json(req): Json<UnbanReq>) -> Reply {
    let helper = helper.is_some();
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let value = req.value.trim().to_string();
    let kind = match req.kind.as_str() {
        "id" if valid_steamid(&value) => "id",
        "ip" if value.parse::<Ipv4Addr>().is_ok() => "ip",
        _ => return bad("That isn't a SteamID or IP address."),
    };
    answer(
        with_game(&st, move |g| {
            let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            tell_game(g, &format!("{} {value}", if kind == "ip" { "removeip" } else { "removeid" }))?;
            edit_ban_file(g, kind, |t| remove_ban_line(t, &value))?;
            Ok(if helper { json!({"ok": true}) } else { listing(g) })
        })
        .await,
    )
}

// ---- kicking with a reason ----------------------------------------------------------

/// A kick reason the player sees: letters, digits and plain punctuation only, so it can't
/// end or extend the console command.
pub fn kick_reason(s: &str) -> String {
    let kept: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || " _.,!?'()+-:#@&%/".contains(*c)).take(64).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The engine's own kick by userid, with the reason shown to them when there is one.
pub fn kick_line(uid: u32, reason: &str) -> String {
    let reason = kick_reason(reason);
    if reason.is_empty() {
        format!("kick #{uid}")
    } else {
        format!("kick #{uid} \"{reason}\"")
    }
}

#[derive(Deserialize)]
struct KickReq {
    uid: u32,
    #[serde(default)]
    reason: String,
}

async fn api_kick(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<KickReq>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    if req.uid == 0 {
        return bad("That player isn't in the game any more.");
    }
    let line = kick_line(req.uid, &req.reason);
    answer(
        with_game(&st, move |g| {
            if !tell_game(g, &line)? {
                return Err("No game is running.".into());
            }
            Ok(json!({"ok": true}))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/admins", get(api_list))
        .route("/api/admins/person", post(api_person))
        .route("/api/admins/remove", post(api_remove))
        .route("/api/admins/settings", post(api_settings))
        .route("/api/admins/ban", post(api_ban))
        .route("/api/admins/unban", post(api_unban))
        .route("/api/admins/kick", post(api_kick))
}

// ---- shared between servers (lists.rs) ----------------------------------------------

/// The admin list as it's stored, to tell whether it changed.
pub(crate) fn shared_admins(g: &Game) -> String {
    // (people from shared control belong to their one server: not part of what's shared)
    let mut a = load(&g.data);
    a.people.retain(|p| p.note != SHARED_NOTE);
    serde_json::to_string(&a).unwrap_or_default()
}

/// Gives `to` the same admins, VIPs and VIP settings as `from`.
pub(crate) fn copy_admins(from: &Game, to: &Game) -> Result<(), String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = load(&from.data);
    // shared control's people stay on their own server: not copied, and kept where they are
    a.people.retain(|p| p.note != SHARED_NOTE);
    a.people.extend(load(&to.data).people.into_iter().filter(|p| p.note == SHARED_NOTE));
    save(&to.data, &a)?;
    write_files(to, &a)?;
    reload(to);
    Ok(())
}

pub(crate) fn shared_bans(g: &Game) -> String {
    // (unreadable: a value no real list has, so nothing is passed on from it)
    let Ok(bans) = read_bans(g) else { return "unreadable".into() };
    let mut v: Vec<String> = bans.into_iter().map(|b| format!("{} {}", b.kind, b.value)).collect();
    v.sort();
    v.join("\n")
}

/// The bans as "id STEAM_..." / "ip 1.2.3.4" lines.
pub(crate) fn ban_set(g: &Game) -> Result<std::collections::BTreeSet<String>, String> {
    Ok(read_bans(g)?.into_iter().map(|b| format!("{} {}", b.kind, b.value)).collect())
}

/// Adds and lifts bans on `to` ("id STEAM_..." / "ip 1.2.3.4" lines), leaving its other
/// bans alone (ones made in the game there stay).
pub(crate) fn apply_bans(to: &Game, add: &[String], lift: &[String]) -> Result<(), String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let parse = |l: &String| {
        l.split_once(' ')
            .map(|(k, v)| (if k == "ip" { "ip" } else { "id" }, v.to_string()))
            .filter(|(k, v)| if *k == "ip" { v.parse::<Ipv4Addr>().is_ok() } else { valid_steamid(v) })
    };
    let mut cmds = Vec::new();
    for kind in ["id", "ip"] {
        let adds: Vec<String> = add.iter().filter_map(parse).filter(|(k, _)| *k == kind).map(|(_, v)| v).collect();
        let lifts: Vec<String> = lift.iter().filter_map(parse).filter(|(k, _)| *k == kind).map(|(_, v)| v).collect();
        if adds.is_empty() && lifts.is_empty() {
            continue;
        }
        let word = if kind == "ip" { "addip" } else { "banid" };
        edit_ban_file(to, kind, |t| {
            let mut text = t.to_string();
            for v in &lifts {
                text = remove_ban_line(&text, v);
            }
            let have: Vec<String> = parse_bans(&text, if kind == "ip" { "ip" } else { "id" }).into_iter().map(|b| b.value).collect();
            let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
            for v in adds.iter().filter(|v| !have.contains(v)) {
                let base = text.trim_end_matches(['\r', '\n']).to_string();
                text = format!("{base}{}{word} 0.0 {v}{nl}", if base.is_empty() { "" } else { nl });
            }
            text
        })?;
        cmds.extend(lifts.iter().map(|v| format!("{} {v}", if kind == "ip" { "removeip" } else { "removeid" })));
        cmds.extend(adds.iter().map(|v| format!("{word} 0 {v}")));
    }
    for chunk in cmds.chunks(20) {
        if !tell_game(to, &chunk.join(";"))? {
            break;
        }
    }
    Ok(())
}

/// Gives `to` the same bans as `from` (the running game is told about each one that
/// came or went).
pub(crate) fn copy_bans(from: &Game, to: &Game) -> Result<(), String> {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = read_bans(to)?;
    let new = read_bans(from)?;
    for kind in ["id", "ip"] {
        let text = ban_text(from, kind)?;
        edit_ban_file(to, kind, |_| text)?;
    }
    let mut cmds = Vec::new();
    for b in old.iter().filter(|b| !new.contains(b)) {
        cmds.push(format!("{} {}", if b.kind == "ip" { "removeip" } else { "removeid" }, b.value));
    }
    for b in new.iter().filter(|b| !old.contains(b)) {
        cmds.push(format!("{} 0 {}", if b.kind == "ip" { "addip" } else { "banid" }, b.value));
    }
    for chunk in cmds.chunks(20) {
        if !tell_game(to, &chunk.join(";"))? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: &str, role: Role, expires: Option<&str>) -> Person {
        Person { id: id.into(), name: "Sam".into(), role, expires: expires.map(String::from), note: String::new() }
    }

    #[test]
    fn admins_steamids() {
        for ok in ["STEAM_0:1:123", "STEAM_0:0:0", "STEAM_1:0:4294967295"] {
            assert!(valid_steamid(ok), "{ok}");
        }
        for bad in [
            "STEAM_ID_LAN", "BOT", "STEAM_ID_PENDING", "VALVE_ID_LAN", "STEAM_0:2:1", "STEAM_6:0:1", "STEAM_0:1:",
            "STEAM_0:1:4294967296", "STEAM_0:1:12a", "STEAM_0:1:1:1", "steam_0:1:1", "STEAM_0:1:01", " STEAM_0:1:1",
            "STEAM_0:1:1\" \"abc", "STEAM_0:1:1;quit",
        ] {
            assert!(!valid_steamid(bad), "{bad}");
        }
    }

    #[test]
    fn admins_dates_and_expiry() {
        assert!(valid_date("2026-12-12") && valid_date("2028-02-29"));
        assert!(!valid_date("2027-02-29") && !valid_date("2026-13-01") && !valid_date("2026-1-01") && !valid_date("12/12/2026"));
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(20_764), "2026-11-07");
        assert!(valid_date(&today()));
        let today = "2026-11-06";
        assert!(!expired(&person("STEAM_0:1:1", Role::Vip, None), today));
        assert!(!expired(&person("STEAM_0:1:1", Role::Vip, Some("2026-11-06")), today), "the last day still counts");
        assert!(expired(&person("STEAM_0:1:1", Role::Vip, Some("2026-11-05")), today));
    }

    #[test]
    fn admins_users_ini_block() {
        let people = vec![
            person("STEAM_0:1:111", Role::Admin, None),
            person("STEAM_0:0:222", Role::Vip, Some("2026-12-12")),
            person("STEAM_0:0:333", Role::Vip, Some("2026-01-01")),
        ];
        let user = "; Users configuration file\r\n\"loopback\" \"\" \"abcdefghijklmnopqrstuv\" \"de\"\r\n";
        let once = rewrite_users(user, &people, "2026-11-06");
        assert!(once.starts_with(user), "the user's own lines stay");
        assert!(once.contains("\"STEAM_0:1:111\" \"\" \"bcdefijtu\" \"ce\"\r\n"));
        assert!(once.contains("\"STEAM_0:0:222\" \"\" \"bt\" \"ce\"\r\n"));
        assert!(once.contains("; Sam (vip, until 2026-12-12)"));
        assert!(!once.contains("STEAM_0:0:333"), "expired people are dropped");
        assert!(!once.replace("\r\n", "").contains('\n'), "keeps CRLF");

        // rewriting replaces the block in place, keeping lines after it
        let edited = format!("{once}\"STEAM_0:1:999\" \"\" \"z\" \"ce\"\r\n");
        let twice = rewrite_users(&edited, &people[..1], "2026-11-06");
        assert_eq!(twice.matches(BEGIN).count(), 1);
        assert!(!twice.contains("STEAM_0:0:222"));
        assert!(twice.contains("STEAM_0:1:999") && twice.starts_with(user));
        assert_eq!(rewrite_users(&twice, &people[..1], "2026-11-06"), twice, "stable");

        // empty list: an empty block; empty file: just the block
        let none = rewrite_users(&once, &[], "2026-11-06");
        assert!(none.contains(&format!("{BEGIN}\r\n{END}")));
        assert_eq!(rewrite_users("", &[], "2026-11-06"), format!("{BEGIN}\r\n{END}\r\n"));
        // LF files stay LF; a bad id never gets in
        let lf = rewrite_users("; x\n", &[person("STEAM_ID_LAN", Role::Owner, None), people[0].clone()], "2026-11-06");
        assert!(!lf.contains('\r') && !lf.contains("STEAM_ID_LAN") && lf.contains("STEAM_0:1:111"));
    }

    #[test]
    fn seen_list() {
        let log = "100\tSTEAM_0:1:5\tBob\n200\tSTEAM_0:1:5\tBobby\n150\tSTEAM_0:0:7\tAnn\n160\tSTEAM_ID_LAN\tLan\nbad line\n";
        let l = fold_seen(vec![], log);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, "STEAM_0:1:5");
        assert_eq!((l[0].name.as_str(), l[0].aka.clone(), l[0].visits, l[0].first, l[0].last), ("Bobby", vec!["Bob".to_string()], 2, 100, 200));
        let l = fold_seen(l, "300\tSTEAM_0:0:7\tAnn\n");
        assert_eq!(l[0].id, "STEAM_0:0:7");
        assert_eq!(l[0].visits, 2);
    }

    #[test]
    fn imports_existing_admins() {
        let text = "; Bob\n\"STEAM_0:1:123\" \"\" \"abcdefghijklmnopqrstu\" \"ce\"\n\"loopback\" \"\" \"abcdefghijklmnopqrstu\" \"de\"\n\"STEAM_0:0:9\" \"\" \"bt\" \"ce\"\n\"STEAM_0:0:8\" \"\" \"z\" \"ce\"\n";
        let (rest, people) = take_users(text);
        assert_eq!(people.len(), 2);
        assert_eq!(people[0].role, Role::Owner);
        assert_eq!(people[0].name, "Bob");
        assert_eq!(people[1].role, Role::Vip);
        assert!(rest.contains("; moved to Loner-Strike's Admins page: \"STEAM_0:1:123\""));
        assert!(rest.contains("\n\"loopback\""), "non-SteamID entries stay");
        assert!(rest.contains("\n\"STEAM_0:0:8\" \"\" \"z\""), "plain users stay");
    }

    #[test]
    fn admins_role_flags() {
        assert!(Role::Owner.flags().contains('l') && Role::Owner.flags().contains('a') && Role::Owner.flags().contains('t'));
        assert!(!Role::Admin.flags().contains('l') && !Role::Admin.flags().contains('g') && !Role::Admin.flags().contains('h'));
        assert!(Role::Admin.flags().contains('t') && Role::Admin.flags().contains('d'));
        assert_eq!(Role::Moderator.flags(), "ceiju");
        assert_eq!(Role::Vip.flags(), "bt");
    }

    #[test]
    fn admins_kick_reason() {
        assert_eq!(kick_line(7, ""), "kick #7");
        assert_eq!(kick_line(7, "  spam  in chat "), "kick #7 \"spam in chat\"");
        assert_eq!(kick_line(7, "x\"; quit; \"y"), "kick #7 \"x quit y\"");
        assert_eq!(kick_line(7, ";\n"), "kick #7");
        assert_eq!(kick_reason(&"a".repeat(100)).len(), 64);
    }

    #[test]
    fn admins_bans_and_settings() {
        let text = "banid 0.0 STEAM_0:1:5\r\nbanid 0.0 STEAM_ID_LAN\r\nbanid 0.0 STEAM_0:0:6\r\n";
        let bans = parse_bans(text, "id");
        assert_eq!(bans.iter().map(|b| b.value.as_str()).collect::<Vec<_>>(), ["STEAM_0:1:5", "STEAM_0:0:6"]);
        assert_eq!(parse_bans("addip 0.0 10.0.0.1\naddip 0.0 nope\n", "ip").len(), 1);
        assert_eq!(remove_ban_line(text, "STEAM_0:1:5"), "banid 0.0 STEAM_ID_LAN\r\nbanid 0.0 STEAM_0:0:6\r\n");
        assert_eq!(remove_ban_line("banid 0.0 STEAM_0:1:5\n", "STEAM_0:1:5"), "");

        assert!(valid_url("https://ko-fi.com/someone") && valid_url("http://paypal.me/x?y=1&z=2"));
        assert!(!valid_url("javascript:alert(1)") && !valid_url("https://x.com/\"; quit") && !valid_url("https://a b.com") && !valid_url("ftp://x.com/aaaa"));
        let a = Admins { donate: "https://ko-fi.com/someone".into(), ..Default::default() };
        let cfg = vip_cfg(&a);
        assert!(cfg.contains("gm_vip_money 500\r\n") && cfg.contains("gm_vip_donate \"https://ko-fi.com/someone\""));
        let a = Admins { donate: "https://x.com/\";quit".into(), ..Default::default() };
        assert!(vip_cfg(&a).contains("gm_vip_donate \"\""));
    }

    #[test]
    fn admins_abilities_defaults_and_json() {
        // an admins.json from before the grid loads with the defaults
        let old: Admins = serde_json::from_str(r#"{"people":[],"donate":"","perks":{"armor":true,"defuse":true,"nades":true,"money":500,"tag":true,"chat":true,"host":false}}"#).unwrap();
        assert_eq!(old.abilities, Abilities::default());
        let d = Abilities::default();
        assert_eq!(role_mask(&d.gear), 15);
        assert_eq!(role_mask(&d.hook), 15);
        assert_eq!(role_mask(&d.jump), 0);
        assert_eq!(role_mask(&d.respawn), 0);
        assert_eq!(role_mask(&d.noclip), 12);
        assert_eq!(role_mask(&d.god), 8);
        assert_eq!(role_mask(&d.tp), 12);
        assert!(!d.every_mode);

        // a half-filled grid keeps what's there and defaults the rest
        let part: Abilities = serde_json::from_str(r#"{"jump":["vip"],"god":[],"every_mode":true}"#).unwrap();
        assert_eq!(part.jump, vec![Role::Vip]);
        assert!(part.god.is_empty() && part.every_mode);
        assert_eq!(part.noclip, vec![Role::Admin, Role::Owner]);

        // written as lowercase role words, and read back the same
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["hook"], json!(["vip", "moderator", "admin", "owner"]));
        assert_eq!(v["god"], json!(["owner"]));
        let back: Abilities = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
        assert!(serde_json::from_str::<Abilities>(r#"{"god":["superuser"]}"#).is_err(), "unknown roles are refused");

        // tidy: VIP..Owner order, no repeats
        let messy = Abilities { noclip: vec![Role::Owner, Role::Vip, Role::Owner], ..Default::default() }.tidy();
        assert_eq!(messy.noclip, vec![Role::Vip, Role::Owner]);
    }

    #[test]
    fn admins_abilities_cfg() {
        let cfg = vip_cfg(&Admins::default());
        for line in ["gm_vip_gear 15\r\n", "gm_vip_hook 15\r\n", "gm_vip_jump 0\r\n", "gm_vip_respawn 0\r\n", "gm_vip_noclip 12\r\n", "gm_vip_god 8\r\n", "gm_vip_tp 12\r\n", "gm_vip_anymode 0\r\n"] {
            assert!(cfg.contains(line), "{line}");
        }
        let a = Admins {
            abilities: Abilities { hook: vec![Role::Moderator, Role::Owner], jump: VIP_UP.to_vec(), god: vec![], every_mode: true, ..Default::default() },
            ..Default::default()
        };
        let cfg = vip_cfg(&a);
        assert!(cfg.contains("gm_vip_hook 10\r\n") && cfg.contains("gm_vip_jump 15\r\n") && cfg.contains("gm_vip_god 0\r\n") && cfg.contains("gm_vip_anymode 1\r\n"));
        assert!(!cfg.replace("\r\n", "").contains('\n'), "CRLF only");
    }
}
