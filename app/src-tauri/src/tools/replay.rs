//! Recordings of games (gm_simrec's CSV, see loner-strike-sim/gm_simrec.sma) turned into what
//! the Replay tool draws: per-player column arrays, rounds, kills with their cause (falls
//! found), hits, shots, events, chat, "stuck" spans and 128-unit heat grids. And the brain's
//! trace (lstrace.csv / trace_*.csv: what each bot was doing and why), matched to the
//! recording's clock.
//!
//! Both are read a line at a time (a recording can be big), capped at [`MAX_FILE`].
//!
//! Recording rows:
//!   S,t,id,team,x,y,z,vx,vy,vz,yaw,pitch,duck,ground,weapon,clip,keys,hp,venemy,enemydist,matedist
//!   F,t,id,weapon,speed,duck,ground   H,t,attacker,victim,damage,hitgroup
//!   K,t,killer,victim,headshot,weapon R,t,start|end   P,id,name,bot
//!   C,t,id,team,text                  E,t,id,event,detail

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;

/// The biggest recording or trace we open.
pub const MAX_FILE: u64 = 200 << 20;
/// The game's safe fall speed (PLAYER_MAX_SAFE_FALL_SPEED): falling faster than this hurts.
pub const SAFE_FALL_SPEED: f32 = 580.0;
/// Heat grid cell size, as sweepreport.py.
pub const CELL: f32 = 128.0;
/// Stuck: slower than this...
pub const STUCK_SPEED: f32 = 20.0;
/// ...for at least this long, with no enemy in view.
pub const STUCK_SECS: f32 = 8.0;
/// Samples further apart than this aren't one stretch of living (a death, a disconnect).
const GAP: f32 = 0.25;
/// Coordinates beyond this aren't real (GoldSrc's world is +-32768 at most).
const WORLD: f32 = 32768.0;
/// Lines longer than this are skipped (chat is the longest, ~200 bytes).
const MAX_LINE: usize = 4096;
/// At most this many players (ids are 1-32; a few more slots for odd files).
const MAX_ID: i64 = 64;

pub fn too_big(size: u64) -> String {
    format!(
        "This recording is too big to open here ({} MB; the limit is {} MB). Record shorter games, or split the file.",
        size >> 20,
        MAX_FILE >> 20
    )
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Player {
    pub id: u8,
    pub name: String,
    pub bot: bool,
    /// The last team seen (1 T, 2 CT, 0 none).
    pub team: u8,
    /// Where this player's samples are in the columns.
    pub start: usize,
    pub count: usize,
}

/// Every player's samples, one after another (each player's in time order).
#[derive(Debug, Clone, Default)]
pub struct Columns {
    pub t: Vec<f32>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub vz: Vec<f32>,
    pub yaw: Vec<f32>,
    pub pitch: Vec<f32>,
    pub hp: Vec<u8>,
    pub duck: Vec<u8>,
    pub ground: Vec<u8>,
    pub weapon: Vec<u8>,
    pub venemy: Vec<u8>,
    pub team: Vec<u8>,
    pub keys: Vec<u32>,
}

impl Columns {
    fn len(&self) -> usize {
        self.t.len()
    }
    fn push(&mut self, s: &Sample) {
        self.t.push(s.t);
        self.x.push(s.pos[0]);
        self.y.push(s.pos[1]);
        self.z.push(s.pos[2]);
        self.vx.push(s.vel[0]);
        self.vy.push(s.vel[1]);
        self.vz.push(s.vel[2]);
        self.yaw.push(s.yaw);
        self.pitch.push(s.pitch);
        self.hp.push(s.hp);
        self.duck.push(s.duck);
        self.ground.push(s.ground);
        self.weapon.push(s.weapon);
        self.venemy.push(s.venemy);
        self.team.push(s.team);
        self.keys.push(s.keys);
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    t: f32,
    team: u8,
    pos: [f32; 3],
    vel: [f32; 3],
    yaw: f32,
    pitch: f32,
    duck: u8,
    ground: u8,
    weapon: u8,
    keys: u32,
    hp: u8,
    venemy: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fall {
    /// Where it last stood before falling.
    pub from: [f32; 3],
    pub from_t: f32,
    /// How far it dropped, and its speed down at the end.
    pub drop: f32,
    pub speed: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Kill {
    pub t: f32,
    pub killer: u8,
    pub victim: u8,
    pub headshot: bool,
    pub weapon: String,
    /// "enemy", "world" (falls and the like), "trigger_hurt" or "suicide" (own grenade, kill).
    pub cause: &'static str,
    pub team_kill: bool,
    /// Where the victim was last seen alive.
    pub at: Option<[f32; 3]>,
    pub fall: Option<Fall>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub t: f32,
    pub attacker: u8,
    pub victim: u8,
    pub damage: f32,
    pub hitgroup: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct Shot {
    pub t: f32,
    pub id: u8,
    pub weapon: u8,
    pub speed: f32,
    pub duck: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub t: f32,
    pub id: u8,
    pub event: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Chat {
    pub t: f32,
    pub id: u8,
    pub team: bool,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Round {
    pub start: f32,
    pub end: Option<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stuck {
    pub id: u8,
    pub from: f32,
    pub to: f32,
    pub at: [f32; 3],
}

/// 128-unit grids over the area played in: seconds spent, deaths, seconds stuck. Row-major,
/// cell (cx, cy) covers x0 + cx*128 .. and y0 + cy*128 ..
#[derive(Debug, Clone, Default, Serialize)]
pub struct Heat {
    pub cell: f32,
    pub x0: f32,
    pub y0: f32,
    pub w: usize,
    pub h: usize,
    #[serde(skip)]
    pub time: Vec<f32>,
    #[serde(skip)]
    pub deaths: Vec<u32>,
    #[serde(skip)]
    pub stuck: Vec<f32>,
}

impl Heat {
    fn cell_of(&self, x: f32, y: f32) -> Option<usize> {
        let cx = ((x - self.x0) / self.cell).floor();
        let cy = ((y - self.y0) / self.cell).floor();
        if !(cx >= 0.0 && cy >= 0.0 && (cx as usize) < self.w && (cy as usize) < self.h) {
            return None;
        }
        Some(cy as usize * self.w + cx as usize)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub players: Vec<Player>,
    pub cols: Columns,
    pub rounds: Vec<Round>,
    pub kills: Vec<Kill>,
    pub hits: Vec<Hit>,
    pub shots: Vec<Shot>,
    pub events: Vec<Event>,
    pub chat: Vec<Chat>,
    pub stuck: Vec<Stuck>,
    pub heat: Heat,
    /// Lines that weren't any row we know (or were cut off).
    pub skipped: usize,
}

impl Recording {
    pub fn first_time(&self) -> Option<f32> {
        self.cols.t.iter().copied().reduce(f32::min)
    }
    pub fn last_time(&self) -> Option<f32> {
        self.cols.t.iter().copied().reduce(f32::max)
    }
    pub fn seconds(&self) -> f32 {
        match (self.first_time(), self.last_time()) {
            (Some(a), Some(b)) => b - a,
            _ => 0.0,
        }
    }
    pub fn bots(&self) -> usize {
        self.players.iter().filter(|p| p.bot).count()
    }

    fn player(&self, id: u8) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    /// The sample index for player `id` closest to time `t` (by binary search).
    pub fn sample_at(&self, id: u8, t: f32) -> Option<usize> {
        let p = self.player(id)?;
        if p.count == 0 {
            return None;
        }
        let ts = &self.cols.t[p.start..p.start + p.count];
        let k = ts.partition_point(|&v| v < t);
        let best = [k.saturating_sub(1), k.min(p.count - 1)].into_iter().min_by(|&a, &b| (ts[a] - t).abs().total_cmp(&(ts[b] - t).abs()))?;
        Some(p.start + best)
    }

    /// The summary JSON (everything but the columns and grids).
    pub fn summary(&self) -> Value {
        json!({
            "players": self.players, "rounds": self.rounds, "kills": self.kills, "hits": self.hits,
            "shots": self.shots, "events": self.events, "chat": self.chat, "stuck": self.stuck,
            "heat": self.heat, "skipped": self.skipped,
            "start": self.first_time(), "end": self.last_time(),
            "falls": self.kills.iter().filter(|k| k.fall.is_some()).count(),
        })
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// Reads lines (Latin-1, any line ending) and hands each to `f`, refusing more than MAX_FILE
/// bytes in all.
fn each_line(mut r: impl BufRead, mut f: impl FnMut(&str)) -> Result<(), String> {
    let mut buf = Vec::with_capacity(256);
    let mut total: u64 = 0;
    loop {
        buf.clear();
        // (read a capped amount per line: a file with no newlines can't grow the buffer forever)
        let n = std::io::Read::take(&mut r, MAX_LINE as u64 + 1).read_until(b'\n', &mut buf).map_err(|e| format!("Couldn't read the file: {e}"))?;
        if n == 0 {
            return Ok(());
        }
        total += n as u64;
        if total > MAX_FILE {
            return Err(too_big(total));
        }
        if buf.len() > MAX_LINE && buf.last() != Some(&b'\n') {
            // skip the rest of an overlong line
            loop {
                buf.clear();
                let m = std::io::Read::take(&mut r, MAX_LINE as u64).read_until(b'\n', &mut buf).map_err(|e| format!("Couldn't read the file: {e}"))?;
                total += m as u64;
                if total > MAX_FILE {
                    return Err(too_big(total));
                }
                if m == 0 || buf.last() == Some(&b'\n') {
                    break;
                }
            }
            f("");
            continue;
        }
        let line = latin1(&buf);
        f(line.trim_end_matches(['\n', '\r']));
    }
}

fn num(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}
fn id_of(s: &str) -> Option<u8> {
    s.trim().parse::<i64>().ok().filter(|&v| (0..=MAX_ID).contains(&v)).map(|v| v as u8)
}
fn small(s: &str) -> Option<u8> {
    s.trim().parse::<i64>().ok().map(|v| v.clamp(0, 255) as u8)
}
fn coord(s: &str) -> Option<f32> {
    num(s).filter(|v| v.abs() <= WORLD * 2.0)
}

/// Reads a recording.
pub fn parse_recording(r: impl BufRead) -> Result<Recording, String> {
    let mut rec = Recording::default();
    let mut samples: BTreeMap<u8, Vec<Sample>> = BTreeMap::new();
    let mut names: HashMap<u8, (String, bool)> = HashMap::new();
    let mut kills_raw: Vec<(f32, u8, u8, bool, String)> = Vec::new();
    let mut marks: Vec<(f32, bool)> = Vec::new();
    each_line(r, |line| {
        let f: Vec<&str> = line.split(',').collect();
        let ok = (|| -> Option<()> {
            match *f.first()? {
                "S" if f.len() >= 21 => {
                    let id = id_of(f[2])?;
                    let s = Sample {
                        t: num(f[1])?,
                        team: small(f[3])?,
                        pos: [coord(f[4])?, coord(f[5])?, coord(f[6])?],
                        vel: [num(f[7])?, num(f[8])?, num(f[9])?],
                        yaw: num(f[10])?,
                        pitch: num(f[11])?,
                        duck: small(f[12])?,
                        ground: small(f[13])?,
                        weapon: small(f[14])?,
                        keys: f[16].trim().parse::<i64>().ok()?.clamp(0, u32::MAX as i64) as u32,
                        hp: small(f[17])?,
                        venemy: small(f[18])?,
                    };
                    samples.entry(id).or_default().push(s);
                }
                "F" if f.len() >= 6 => rec.shots.push(Shot { t: num(f[1])?, id: id_of(f[2])?, weapon: small(f[3])?, speed: num(f[4])?, duck: f[5].trim() == "1" }),
                "H" if f.len() >= 6 => {
                    rec.hits.push(Hit { t: num(f[1])?, attacker: id_of(f[2])?, victim: id_of(f[3])?, damage: num(f[4])?, hitgroup: small(f[5])? })
                }
                "K" if f.len() >= 6 => kills_raw.push((num(f[1])?, id_of(f[2])?, id_of(f[3])?, f[4].trim() == "1", f[5].trim().to_string())),
                "R" if f.len() >= 3 => marks.push((num(f[1])?, f[2].trim() == "start")),
                "P" if f.len() >= 4 => {
                    // (a name can't hold a comma: gm_simrec swaps them for spaces)
                    names.insert(id_of(f[1])?, (f[2].to_string(), f[3].trim() == "1"));
                }
                "C" if f.len() >= 5 => rec.chat.push(Chat { t: num(f[1])?, id: id_of(f[2])?, team: f[3].trim() == "1", text: f[4..].join(",") }),
                "E" if f.len() >= 4 => rec.events.push(Event {
                    t: num(f[1])?,
                    id: id_of(f[2])?,
                    event: f[3].to_string(),
                    detail: f.get(4..).map(|d| d.join(",")).unwrap_or_default(),
                }),
                _ => return None,
            }
            Some(())
        })();
        if ok.is_none() && !line.is_empty() {
            rec.skipped += 1;
        }
    })?;

    // players, in id order; anyone named but never alive still listed
    for id in names.keys() {
        samples.entry(*id).or_default();
    }
    for (id, list) in samples.iter_mut() {
        list.sort_by(|a, b| a.t.total_cmp(&b.t));
        let start = rec.cols.len();
        for s in list.iter() {
            rec.cols.push(s);
        }
        let (name, bot) = names.get(id).cloned().unwrap_or_else(|| (format!("Player {id}"), false));
        let team = list.last().map(|s| s.team).unwrap_or(0);
        rec.players.push(Player { id: *id, name, bot, team, start, count: list.len() });
    }

    // rounds: each start with the end after it
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (t, start) in &marks {
        if *start {
            rec.rounds.push(Round { start: *t, end: None });
        } else if let Some(r) = rec.rounds.last_mut().filter(|r| r.end.is_none()) {
            r.end = Some(*t);
        }
    }
    // play time: not between a round's end and the next start (round-end pause, freeze time)
    let ends: Vec<f32> = marks.iter().filter(|m| !m.1).map(|m| m.0).collect();
    let starts: Vec<f32> = marks.iter().filter(|m| m.1).map(|m| m.0).collect();
    let in_play = |t: f32| match ends.iter().rev().find(|&&e| e <= t) {
        None => true,
        Some(&e) => starts.iter().any(|&s| s > e && s <= t),
    };

    // kills, with their cause, where, and falls
    for (t, killer, victim, headshot, weapon) in kills_raw {
        let cause = if killer == victim {
            if weapon == "world" {
                "world"
            } else {
                "suicide"
            }
        } else if killer == 0 {
            if weapon == "trigger_hurt" {
                "trigger_hurt"
            } else {
                "world"
            }
        } else {
            "enemy"
        };
        let vs = samples.get(&victim).map(|v| v.as_slice()).unwrap_or(&[]);
        let k = vs.partition_point(|s| s.t <= t + 0.05);
        let last = k.checked_sub(1).map(|i| vs[i]).filter(|s| t - s.t < 1.0);
        let team_kill = cause == "enemy"
            && match (samples.get(&killer).and_then(|v| v.last()), last) {
                (Some(a), Some(b)) => a.team == b.team && a.team != 0,
                _ => false,
            };
        let mut fall = None;
        if cause == "world" {
            if let Some(end) = last {
                let recent = &vs[..k];
                let down = recent.iter().rev().take_while(|s| t - s.t < 1.0).map(|s| -s.vel[2]).fold(0.0f32, f32::max);
                if down > SAFE_FALL_SPEED {
                    // where it last stood, looking back up to 15 s
                    let from = recent.iter().rev().take_while(|s| t - s.t < 15.0).find(|s| s.ground == 1);
                    let (fp, ft) = from.map(|s| (s.pos, s.t)).unwrap_or((recent[0].pos, recent[0].t));
                    fall = Some(Fall { from: fp, from_t: ft, drop: fp[2] - end.pos[2], speed: down });
                }
            }
        }
        rec.kills.push(Kill { t, killer, victim, headshot, weapon, cause, team_kill, at: last.map(|s| s.pos), fall });
    }

    // stuck spans
    for (id, list) in &samples {
        let mut since: Option<usize> = None;
        let close = |since: &mut Option<usize>, end: usize, out: &mut Vec<Stuck>| {
            if let Some(s) = since.take() {
                if list[end].t - list[s].t >= STUCK_SECS {
                    out.push(Stuck { id: *id, from: list[s].t, to: list[end].t, at: list[s].pos });
                }
            }
        };
        for i in 0..list.len() {
            let s = &list[i];
            if i > 0 && s.t - list[i - 1].t > GAP {
                close(&mut since, i - 1, &mut rec.stuck);
            }
            let speed = (s.vel[0] * s.vel[0] + s.vel[1] * s.vel[1]).sqrt();
            if speed < STUCK_SPEED && s.venemy == 0 && in_play(s.t) {
                since.get_or_insert(i);
            } else {
                close(&mut since, i.saturating_sub(1), &mut rec.stuck);
            }
        }
        if !list.is_empty() {
            close(&mut since, list.len() - 1, &mut rec.stuck);
        }
    }
    rec.stuck.sort_by(|a, b| a.from.total_cmp(&b.from));

    // heat grids
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for i in 0..rec.cols.len() {
        let (x, y) = (rec.cols.x[i], rec.cols.y[i]);
        if x.abs() <= WORLD && y.abs() <= WORLD {
            lo = [lo[0].min(x), lo[1].min(y)];
            hi = [hi[0].max(x), hi[1].max(y)];
        }
    }
    if lo[0] <= hi[0] {
        let x0 = (lo[0] / CELL).floor() * CELL;
        let y0 = (lo[1] / CELL).floor() * CELL;
        let w = ((hi[0] - x0) / CELL).floor() as usize + 1;
        let h = ((hi[1] - y0) / CELL).floor() as usize + 1;
        let mut heat = Heat { cell: CELL, x0, y0, w, h, time: vec![0.0; w * h], deaths: vec![0; w * h], stuck: vec![0.0; w * h] };
        for p in &rec.players {
            for i in p.start..p.start + p.count {
                let dt = if i + 1 < p.start + p.count { (rec.cols.t[i + 1] - rec.cols.t[i]).clamp(0.0, GAP) } else { 0.1 };
                if let Some(c) = heat.cell_of(rec.cols.x[i], rec.cols.y[i]) {
                    heat.time[c] += dt;
                }
            }
        }
        for k in &rec.kills {
            if let Some(c) = k.at.and_then(|a| heat.cell_of(a[0], a[1])) {
                heat.deaths[c] += 1;
            }
        }
        for s in &rec.stuck {
            if let Some(c) = heat.cell_of(s.at[0], s.at[1]) {
                heat.stuck[c] += s.to - s.from;
            }
        }
        rec.heat = heat;
    }
    if rec.cols.len() == 0 && rec.kills.is_empty() && rec.rounds.is_empty() {
        return Err("This file isn't a recording (no players, kills or rounds in it). Recordings are the CSV files gm_simrec writes.".into());
    }
    Ok(rec)
}

/// What the recordings list shows, from one quick pass over a file.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Scan {
    pub start: Option<f32>,
    pub end: Option<f32>,
    pub players: usize,
    pub bots: usize,
    pub kills: usize,
    pub rounds: usize,
}

pub fn scan_recording(r: impl BufRead) -> Result<Scan, String> {
    let mut s = Scan::default();
    let mut who: HashMap<u8, bool> = HashMap::new();
    each_line(r, |line| {
        let mut f = line.split(',');
        match f.next() {
            Some("S") => {
                if let Some(t) = f.next().and_then(num) {
                    s.start = Some(s.start.map_or(t, |v| v.min(t)));
                    s.end = Some(s.end.map_or(t, |v| v.max(t)));
                }
            }
            Some("P") => {
                if let (Some(id), _, Some(bot)) = (f.next().and_then(id_of), f.next(), f.next()) {
                    who.insert(id, bot.trim() == "1");
                }
            }
            Some("K") => s.kills += 1,
            Some("R") if line.ends_with("start") => s.rounds += 1,
            _ => {}
        }
    })?;
    s.players = who.len();
    s.bots = who.values().filter(|&&b| b).count();
    Ok(s)
}

// ---- the brain's trace ---------------------------------------------------------------------

/// The per-bot row's columns, in order (the file's own #cols line names them too).
pub const TRACE_COLS: [&str; 24] = [
    "time", "id", "team", "state", "task", "enemy", "leader", "goal_x", "goal_y", "noise_x", "noise_y", "why", "why_ago", "speed",
    "view_off", "attacking", "look", "x", "y", "z", "hp", "weapon", "crouched", "pitch",
];
/// The text columns (the rest are numbers).
const TEXT_COLS: [usize; 3] = [3, 11, 16];

/// The bot's task numbers (CCSBot::TaskType).
pub const TASKS: [&str; 20] = [
    "seek and destroy",
    "plant the bomb",
    "find the ticking bomb",
    "defuse the bomb",
    "guard the ticking bomb",
    "guard the bomb defuser",
    "guard the loose bomb",
    "guard a bomb site",
    "escape from the bomb",
    "hold position",
    "follow",
    "VIP escape",
    "guard the VIP escape zone",
    "collect hostages",
    "rescue hostages",
    "guard hostages",
    "guard the hostage rescue zone",
    "move to the enemy's last known position",
    "move to a sniper spot",
    "sniping",
];

#[derive(Debug, Default)]
struct Segment {
    map: String,
    last_t: f32,
    rows: BTreeMap<u8, Vec<Value>>,
    why: Vec<Value>,
    attack: Vec<Value>,
    seen: Vec<Value>,
    marks: Vec<Value>,
    score: usize,
    count: usize,
}

/// Splits a per-bot row into its 24 fields when a text field (why, look) held commas: the 11
/// fields before `why` and the 7 after `look` are fixed, and `why_ago`..`attacking` are numbers.
fn trace_fields(f: Vec<&str>) -> Option<Vec<String>> {
    if f.len() == 24 {
        return Some(f.into_iter().map(String::from).collect());
    }
    if f.len() < 24 {
        return None;
    }
    let tail = f.len() - 7;
    // why_ago is the first index from 12 where four numbers in a row start, leaving look
    let mut k = 12;
    loop {
        if k + 4 > tail {
            return None;
        }
        if (0..4).all(|j| num(f[k + j]).is_some()) {
            break;
        }
        k += 1;
    }
    let mut out: Vec<String> = f[..11].iter().map(|s| s.to_string()).collect();
    out.push(f[11..k].join(","));
    out.extend(f[k..k + 4].iter().map(|s| s.to_string()));
    out.push(f[k + 4..tail].join(","));
    out.extend(f[tail..].iter().map(|s| s.to_string()));
    Some(out)
}

fn val(s: &str) -> Value {
    match num(s) {
        Some(v) => json!(v),
        None => json!(s),
    }
}

/// Reads a trace and keeps the stretch (one map visit) that matches the recording best: the one
/// whose bots stand where the recording has them at the same times. Times in it are the same
/// game clock as the recording's (both come from the server), so nothing is shifted.
pub fn parse_trace(r: impl BufRead, rec: &Recording) -> Result<Value, String> {
    let mut best: Option<Segment> = None;
    let mut cur = Segment::default();
    let mut lines = 0usize;
    let finish = |seg: Segment, best: &mut Option<Segment>| {
        if seg.count == 0 && seg.why.is_empty() {
            return;
        }
        if best.as_ref().map(|b| seg.score > b.score).unwrap_or(true) {
            *best = Some(seg);
        }
    };
    each_line(r, |line| {
        lines += 1;
        let f: Vec<&str> = line.split(',').collect();
        let tag = f[0];
        if tag == "#map" {
            let map = f.get(1).unwrap_or(&"").to_string();
            let old = std::mem::take(&mut cur);
            finish(old, &mut best);
            cur.map = map;
            return;
        }
        if tag.starts_with("#cols") || line.is_empty() {
            return;
        }
        let t = if tag.starts_with('#') { f.get(1).and_then(|s| num(s)) } else { num(tag) };
        let Some(t) = t else { return };
        // the clock went back: a new map visit even without a #map line
        if t + 1.0 < cur.last_t {
            let map = cur.map.clone();
            let old = std::mem::take(&mut cur);
            finish(old, &mut best);
            cur.map = map;
        }
        cur.last_t = t;
        match tag {
            "#why" if f.len() >= 8 => cur.why.push(json!({"t": t, "id": val(f[2]), "team": val(f[3]), "at": [val(f[4]), val(f[5]), val(f[6])], "why": f[7..].join(",")})),
            "#attack" if f.len() >= 7 => cur.attack.push(json!({"t": t, "id": val(f[2]), "team": val(f[3]), "enemy": val(f[4]), "sinceFirstSaw": val(f[5]), "range": val(f[6])})),
            "#seen" if f.len() >= 6 => cur.seen.push(json!({"t": t, "id": val(f[2]), "team": val(f[3]), "enemy": val(f[4]), "range": val(f[5])})),
            "#mark" if f.len() >= 3 => cur.marks.push(json!({"t": t, "map": f[2], "note": f.get(3..).map(|n| n.join(",")).unwrap_or_default()})),
            _ if !tag.starts_with('#') => {
                let Some(fields) = trace_fields(f) else { return };
                let Some(id) = id_of(&fields[1]) else { return };
                let row: Vec<Value> = fields.iter().enumerate().map(|(i, s)| if TEXT_COLS.contains(&i) { json!(s) } else { val(s) }).collect();
                // does the recording have this bot here at this time?
                if let (Some(x), Some(y)) = (num(&fields[17]), num(&fields[18])) {
                    if let Some(i) = rec.sample_at(id, t) {
                        let (dx, dy) = (rec.cols.x[i] - x, rec.cols.y[i] - y);
                        if (rec.cols.t[i] - t).abs() < 0.3 && dx * dx + dy * dy < 64.0 * 64.0 {
                            cur.score += 1;
                        }
                    }
                }
                cur.count += 1;
                cur.rows.entry(id).or_default().push(Value::Array(row));
            }
            _ => {}
        }
    })?;
    finish(cur, &mut best);
    let Some(seg) = best else {
        return Err("This trace file has no bot lines in it.".into());
    };
    let bots: Vec<Value> = seg.rows.into_iter().map(|(id, rows)| json!({"id": id, "rows": rows})).collect();
    Ok(json!({
        "map": seg.map, "offset": 0.0, "matched": seg.score, "rows": seg.count,
        "matches": seg.count > 0 && seg.score * 2 >= seg.count,
        "columns": TRACE_COLS, "tasks": TASKS, "bots": bots,
        "why": seg.why, "attack": seg.attack, "seen": seg.seen, "marks": seg.marks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REC: &str = include_str!("testdata/rec_snippet.csv");
    const TRACE: &str = include_str!("testdata/trace_snippet.csv");

    #[test]
    fn reads_a_recording() {
        let r = parse_recording(REC.as_bytes()).unwrap();
        assert_eq!(r.skipped, 0);
        assert!(r.players.len() >= 2);
        let p1 = r.players.iter().find(|p| p.id == 1).unwrap();
        assert!(p1.bot && p1.count > 10);
        // columns line up and each player's are in time order
        assert_eq!(r.cols.x.len(), r.cols.t.len());
        for p in &r.players {
            assert!(r.cols.t[p.start..p.start + p.count].windows(2).all(|w| w[0] <= w[1]));
        }
        // the fall: bot 1 dropping at -900 units/s when "world" got it
        let k = r.kills.iter().find(|k| k.victim == 1 && k.killer == 1).unwrap();
        assert_eq!(k.cause, "world");
        let f = k.fall.as_ref().expect("a fall");
        assert!(f.speed > 800.0 && f.drop > 300.0, "{f:?}");
        // an enemy kill and a round
        assert!(r.kills.iter().any(|k| k.cause == "enemy"));
        assert!(r.rounds.iter().any(|x| x.start > 9.0));
        assert!(r.heat.w > 0 && r.heat.time.iter().sum::<f32>() > 1.0);
        assert!(r.heat.deaths.iter().sum::<u32>() >= 1);
        let at = r.sample_at(1, 76.3).unwrap();
        assert!((r.cols.t[at] - 76.25).abs() < 0.06);
        assert!(r.summary()["players"].is_array());
    }

    #[test]
    fn quick_scan() {
        let s = scan_recording(REC.as_bytes()).unwrap();
        assert!(s.bots >= 2 && s.kills == 8 && s.rounds >= 1, "{s:?}");
        assert!(s.start.unwrap() < 3.0 && s.end.unwrap() > 70.0);
    }

    #[test]
    fn causes_and_stuck() {
        let mut csv = String::new();
        csv += "P,3,Bob,1\nP,4,Al,1\nR,0.00,start\n";
        // bot 3 stands still for 10 s with nobody in view, then a trigger_hurt gets it
        for i in 0..=100 {
            csv += &format!("S,{:.2},3,1,100,200,0,0,0,0,0.0,0.0,0,1,28,30,0,100,0,99999,99999\n", i as f32 * 0.1);
            csv += &format!("S,{:.2},4,2,500,200,0,250,0,0,0.0,0.0,0,1,28,30,0,100,0,99999,99999\n", i as f32 * 0.1);
        }
        csv += "K,10.05,0,3,0,trigger_hurt\nK,10.06,4,4,0,grenade\nK,10.07,0,4,0,worldspawn\n";
        let r = parse_recording(csv.as_bytes()).unwrap();
        let causes: Vec<&str> = r.kills.iter().map(|k| k.cause).collect();
        assert_eq!(causes, ["trigger_hurt", "suicide", "world"]);
        assert!(r.kills.iter().all(|k| k.fall.is_none()));
        assert_eq!(r.stuck.len(), 1);
        assert_eq!(r.stuck[0].id, 3);
        assert!((r.stuck[0].to - r.stuck[0].from - 10.0).abs() < 0.01);
        let c = r.heat.cell_of(100.0, 200.0).unwrap();
        assert!((r.heat.stuck[c] - 10.0).abs() < 0.01);
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(parse_recording("hello\nworld\n".as_bytes()).is_err());
        assert!(parse_recording("".as_bytes()).is_err());
        // a line with no end, longer than any real one, and numbers out of range
        let mut s = "S,1,1,1,".to_string() + &"9".repeat(100_000) + "\nK,1,1,1,0,world\nS,1e30,999,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1\n";
        s += "S,NaN,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1\n";
        let r = parse_recording(s.as_bytes()).unwrap();
        assert_eq!(r.cols.len(), 0);
        assert!(r.skipped >= 2);
    }

    #[test]
    fn trace_matches_the_recording() {
        let r = parse_recording(REC.as_bytes()).unwrap();
        let t = parse_trace(TRACE.as_bytes(), &r).unwrap();
        assert_eq!(t["map"], "cs_rats2");
        assert_eq!(t["matches"], true, "{} of {}", t["matched"], t["rows"]);
        let bots = t["bots"].as_array().unwrap();
        assert!(!bots.is_empty());
        let row = &bots[0]["rows"][0];
        assert_eq!(row.as_array().unwrap().len(), 24);
        assert!(row[3].is_string());
        assert!(!t["why"].as_array().unwrap().is_empty());
    }

    #[test]
    fn trace_rows_with_commas() {
        let f: Vec<&str> = "2.27,1,2,Buy,0,0,0,0,0,0,0,a, b,0.3,0,0,0,look, here,-272,656,-596,100,16,0,0".split(',').collect();
        let v = trace_fields(f).unwrap();
        assert_eq!(v.len(), 24);
        assert_eq!(v[11], "a, b");
        assert_eq!(v[16], "look, here");
        assert_eq!(v[17], "-272");
    }

    /// LS_TEST_SIM: a whole rats2_v5 recording and its trace.
    #[test]
    fn real_recording() {
        let Ok(sim) = std::env::var("LS_TEST_SIM") else { return };
        let dir = std::path::Path::new(&sim).join("results/rats2_v5");
        let f = std::fs::File::open(dir.join("W1-1_classic_cs_rats2_150726_cs_rats2_1794524249.csv")).unwrap();
        let r = parse_recording(std::io::BufReader::new(f)).unwrap();
        assert_eq!(r.kills.len(), 63);
        assert!(r.kills.iter().filter(|k| k.fall.is_some()).count() >= 3);
        assert!(r.seconds() > 500.0);
        let f = std::fs::File::open(dir.join("trace_W1-1_classic_cs_rats2_150726.csv")).unwrap();
        let t = parse_trace(std::io::BufReader::new(f), &r).unwrap();
        assert_eq!(t["matches"], true, "{} of {}", t["matched"], t["rows"]);
    }
}
