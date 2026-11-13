//! Creature Studio (Tools > Make): the brain's creatures.ini (docs/CREATURES.md §2) edited with
//! real controls, the wave mixes a mode can read (waves.ini), the models and sounds a type can
//! use, and "apply live" on the player's own running game (`ls_creature reload` / `spawn`).
//!
//! The file is read the way the fork reads it (`LsLoadTypes` / `LsSetTypeKey` in
//! regamedll/dlls/creature/creature.cpp): built-in types first, then each `[section]` adds a
//! type or overrides one by name, keys in order, `base =` copies a type above, numbers clamped,
//! comments cut at `;`, `#` or `//`. What the form shows is what the game would use.
//!
//! Saving edits the file in place: a changed key keeps its line (and its comment), a new key
//! goes after the section's last key, a new type is a new section with only what differs from
//! its base, and everything else (comments, unknown keys like `mm =`) stays as it was. After
//! writing, the result is read back as the game would and must give exactly the types asked
//! for, or nothing is written.

use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TOOL: &str = "Creature Studio";
pub const FILE_REL: &str = "addons/amxmodx/configs/gamemodes/creatures.ini";
pub const WAVES_REL: &str = "addons/amxmodx/configs/gamemodes/waves.ini";
const MAX_FILE: u64 = 1 << 20;
/// LS_CREATURE_MAX_TYPES
pub const MAX_TYPES: usize = 48;
/// the fork reads a line into char[512]
const MAX_LINE: usize = 511;
const NO_GAME: &str = "Set up the game on this computer first.";
const INTERNAL: &str = "Something went wrong on Loner-Strike's side.";

pub const CATEGORIES: [&str; 6] = ["infected", "alien", "wildlife", "mechanical", "boss", "special"];
pub const BEHAVIOURS: [&str; 6] = ["idle", "wander", "patrol", "guard", "chase", "flee"];
pub const ATTACKS: [&str; 3] = ["none", "melee", "ranged"];
pub const TARGETS: [&str; 4] = ["nearest", "weakest", "attacker", "random"];
pub const ANIMS: [&str; 6] = ["anim_idle", "anim_walk", "anim_run", "anim_attack", "anim_flinch", "anim_die"];
pub const SOUNDS: [&str; 7] = ["snd_idle", "snd_alert", "snd_attack", "snd_hit", "snd_miss", "snd_pain", "snd_die"];

/// The keys a type is written with, in the order a new section lists them.
pub const KEYS: &[&str] = &[
    "name", "category", "hp", "speed", "walk_speed", "aggression", "detect_radius", "detect_fov", "hearing", "attack", "damage", "attack_range",
    "attack_rate", "attack_delay", "spread", "flee_hp", "behaviour", "radius", "target", "hates", "respawn", "corpse_time", "moves", "frags", "money",
    "model", "skin", "body", "scale", "hull_min", "hull_max", "glow", "fallback_glow", "blood", "anim_idle", "anim_walk", "anim_run", "anim_attack",
    "anim_flinch", "anim_die", "snd_idle", "snd_alert", "snd_attack", "snd_hit", "snd_miss", "snd_pain", "snd_die",
];

/// Numbers and the range the fork keeps them in (LsSetTypeKey).
pub const RANGES: &[(&str, f32, f32)] = &[
    ("hp", 1.0, 100000.0),
    ("speed", 0.0, 2000.0),
    ("walk_speed", 0.0, 2000.0),
    ("aggression", 0.0, 1.0),
    ("detect_radius", 0.0, 8192.0),
    ("detect_fov", 10.0, 360.0),
    ("hearing", 0.0, 10.0),
    ("damage", 0.0, 10000.0),
    ("attack_range", 16.0, 8192.0),
    ("attack_rate", 0.1, 20.0),
    ("attack_delay", 0.0, 3.0),
    ("spread", 0.0, 45.0),
    ("flee_hp", 0.0, 1.0),
    ("radius", 32.0, 8192.0),
    ("respawn", 0.0, 3600.0),
    ("corpse_time", 0.0, 120.0),
    ("frags", -100.0, 100.0),
    ("money", -16000.0, 16000.0),
    ("skin", 0.0, 255.0),
    ("body", 0.0, 255.0),
    ("scale", 0.1, 4.0),
];

/// Text sizes in the fork's LsCreatureType (one less than the array: the terminator).
const MAX_NAME: usize = 31;
const MAX_DISPLAY: usize = 47;
const MAX_MODELS: usize = 255;
const MAX_ANIM: usize = 31;
const MAX_SOUND: usize = 159;

/// One creature type, as the game would use it. Field names are the file's keys.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Creature {
    /// the section name (the type, as commands and plugins name it)
    pub name: String,
    /// `base =`: the type this one starts from ("" = none, or the built-in of the same name)
    pub base: String,
    /// `name =` in the file: what players see
    pub display: String,
    pub category: String,
    pub hp: f32,
    pub speed: f32,
    pub walk_speed: f32,
    pub aggression: f32,
    pub detect_radius: f32,
    pub detect_fov: f32,
    pub hearing: f32,
    pub attack: String,
    pub damage: f32,
    pub attack_range: f32,
    pub attack_rate: f32,
    pub attack_delay: f32,
    pub spread: f32,
    pub flee_hp: f32,
    pub behaviour: String,
    pub radius: f32,
    pub target: String,
    pub hates: String,
    pub respawn: f32,
    pub corpse_time: f32,
    pub moves: bool,
    pub frags: i32,
    pub money: i32,
    /// candidates, space separated: the first file that exists wins
    pub model: String,
    pub skin: i32,
    pub body: i32,
    pub scale: f32,
    pub hull_min: [f32; 3],
    pub hull_max: [f32; 3],
    /// 0 0 0 = none
    pub glow: [f32; 3],
    pub fallback_glow: [f32; 3],
    pub blood: String,
    pub anim: [String; 6],
    pub snd: [String; 7],
}

impl Default for Creature {
    fn default() -> Self {
        defaults("")
    }
}

/// LsTypeDefaults: a type the file names without a base.
pub fn defaults(name: &str) -> Creature {
    Creature {
        name: name.into(),
        base: String::new(),
        display: name.into(),
        category: "infected".into(),
        hp: 100.0,
        speed: 230.0,
        walk_speed: 80.0,
        aggression: 1.0,
        detect_radius: 1200.0,
        detect_fov: 160.0,
        hearing: 1.0,
        attack: "melee".into(),
        damage: 20.0,
        attack_range: 64.0,
        attack_rate: 1.0,
        attack_delay: 0.4,
        spread: 3.0,
        flee_hp: 0.0,
        behaviour: "wander".into(),
        radius: 400.0,
        target: "nearest".into(),
        hates: "all".into(),
        respawn: 0.0,
        corpse_time: 8.0,
        moves: true,
        frags: 1,
        money: 0,
        model: "models/zombie.mdl models/hostage.mdl".into(),
        skin: 0,
        body: 0,
        scale: 1.0,
        hull_min: [-16.0, -16.0, 0.0],
        hull_max: [16.0, 16.0, 72.0],
        glow: [0.0; 3],
        fallback_glow: [40.0, 140.0, 20.0],
        blood: "red".into(),
        anim: Default::default(),
        snd: [
            "zombie/zo_idle1.wav zombie/zo_idle2.wav zombie/zo_idle3.wav zombie/zo_idle4.wav".into(),
            "zombie/zo_alert10.wav zombie/zo_alert20.wav zombie/zo_alert30.wav".into(),
            "zombie/zo_attack1.wav zombie/zo_attack2.wav".into(),
            "zombie/claw_strike1.wav zombie/claw_strike2.wav zombie/claw_strike3.wav | weapons/knife_hit1.wav weapons/knife_hit2.wav".into(),
            "zombie/claw_miss1.wav zombie/claw_miss2.wav | weapons/knife_slash1.wav weapons/knife_slash2.wav".into(),
            "zombie/zo_pain1.wav zombie/zo_pain2.wav | player/pl_pain2.wav player/pl_pain4.wav".into(),
            "zombie/zo_pain2.wav | player/die1.wav player/die3.wav".into(),
        ],
    }
}

/// LsBuiltinTypes: the types the fork always has (the templates).
pub fn builtins() -> Vec<Creature> {
    let mut out = Vec::new();
    let mut t = defaults("zombie");
    t.display = "Zombie".into();
    out.push(t);

    let mut t = defaults("runner");
    t.display = "Runner".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_rate, t.attack_delay) = (60.0, 300.0, 110.0, 10.0, 1.6, 0.25);
    t.behaviour = "chase".into();
    t.blood = "yellow".into();
    t.model = "models/houndeye.mdl models/hostage.mdl".into();
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 36.0]);
    t.fallback_glow = [160.0, 160.0, 20.0];
    t.snd[0] = "houndeye/he_idle1.wav houndeye/he_idle2.wav houndeye/he_idle3.wav".into();
    t.snd[1] = "houndeye/he_alert1.wav houndeye/he_alert2.wav houndeye/he_alert3.wav".into();
    t.snd[2] = "houndeye/he_attack1.wav houndeye/he_attack3.wav".into();
    t.snd[5] = "houndeye/he_pain1.wav houndeye/he_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "houndeye/he_die1.wav houndeye/he_die2.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("brute");
    t.display = "Brute".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay) = (450.0, 170.0, 70.0, 40.0, 72.0, 0.7, 0.6);
    t.category = "alien".into();
    t.frags = 3;
    t.blood = "yellow".into();
    t.model = "models/agrunt.mdl models/hostage.mdl".into();
    t.fallback_glow = [120.0, 20.0, 20.0];
    t.snd[0] = "agrunt/ag_idle1.wav agrunt/ag_idle2.wav agrunt/ag_idle3.wav agrunt/ag_idle4.wav".into();
    t.snd[1] = "agrunt/ag_alert1.wav agrunt/ag_alert3.wav agrunt/ag_alert4.wav".into();
    t.snd[2] = "agrunt/ag_attack1.wav agrunt/ag_attack2.wav agrunt/ag_attack3.wav".into();
    t.snd[5] = "agrunt/ag_pain1.wav agrunt/ag_pain2.wav agrunt/ag_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "agrunt/ag_die1.wav agrunt/ag_die4.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("spitter");
    t.display = "Spitter".into();
    t.category = "alien".into();
    (t.hp, t.speed) = (80.0, 220.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread, t.flee_hp) = (8.0, 900.0, 0.8, 0.5, 4.0, 0.3);
    t.blood = "yellow".into();
    t.model = "models/bullsquid.mdl models/hostage.mdl".into();
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 36.0]);
    t.fallback_glow = [120.0, 200.0, 40.0];
    t.snd = [
        "bullchicken/bc_idle1.wav bullchicken/bc_idle2.wav bullchicken/bc_idle3.wav".into(),
        "bullchicken/bc_idle4.wav bullchicken/bc_idle5.wav".into(),
        "bullchicken/bc_attack2.wav bullchicken/bc_attack3.wav | weapons/sg550-1.wav".into(),
        "bullchicken/bc_spithit1.wav bullchicken/bc_spithit2.wav".into(),
        String::new(),
        "bullchicken/bc_pain1.wav bullchicken/bc_pain2.wav | player/pl_pain2.wav".into(),
        "bullchicken/bc_die1.wav bullchicken/bc_die2.wav | player/die1.wav".into(),
    ];
    out.push(t);

    let mut t = defaults("hound");
    t.display = "Hound".into();
    t.category = "wildlife".into();
    (t.hp, t.speed, t.walk_speed, t.aggression) = (70.0, 280.0, 90.0, 0.35);
    t.behaviour = "guard".into();
    (t.radius, t.damage, t.flee_hp) = (350.0, 12.0, 0.35);
    t.blood = "yellow".into();
    t.model = "models/houndeye.mdl models/hostage.mdl".into();
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 36.0]);
    t.fallback_glow = [200.0, 120.0, 20.0];
    t.snd[0] = "houndeye/he_idle1.wav houndeye/he_idle2.wav houndeye/he_idle3.wav".into();
    t.snd[1] = "houndeye/he_alert1.wav houndeye/he_alert2.wav houndeye/he_alert3.wav".into();
    t.snd[2] = "houndeye/he_attack1.wav houndeye/he_attack3.wav".into();
    t.snd[5] = "houndeye/he_pain1.wav houndeye/he_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "houndeye/he_die1.wav houndeye/he_die2.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("turret");
    t.display = "Sentry".into();
    t.category = "mechanical".into();
    t.hp = 150.0;
    t.moves = false;
    (t.speed, t.walk_speed) = (0.0, 0.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread, t.detect_fov, t.hearing) = (6.0, 1200.0, 4.0, 0.05, 3.0, 360.0, 0.0);
    t.behaviour = "guard".into();
    t.blood = "none".into();
    t.corpse_time = 15.0;
    t.model = "models/sentry.mdl models/hostage.mdl".into();
    // (sentry.mdl has no activities: its sequences by name)
    (t.anim[0], t.anim[3], t.anim[5]) = ("spin".into(), "fire".into(), "die".into());
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 48.0]);
    t.fallback_glow = [40.0, 80.0, 200.0];
    t.snd = Default::default();
    t.snd[1] = "turret/tu_deploy.wav | weapons/zoom.wav".into();
    t.snd[2] = "turret/tu_fire1.wav | weapons/m249-1.wav".into();
    t.snd[6] = "turret/tu_die.wav turret/tu_die2.wav | weapons/debris1.wav".into();
    out.push(t);

    let mut t = defaults("boss");
    t.display = "Abomination".into();
    t.category = "boss".into();
    (t.hp, t.speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.detect_radius) = (3000.0, 200.0, 60.0, 80.0, 0.6, 0.7, 2000.0);
    t.behaviour = "chase".into();
    (t.scale, t.frags) = (0.7, 10);
    t.blood = "yellow".into();
    t.model = "models/garg.mdl models/hostage.mdl".into();
    t.fallback_glow = [160.0, 0.0, 160.0];
    t.snd[0] = "garg/gar_idle1.wav garg/gar_idle2.wav garg/gar_idle3.wav garg/gar_breathe1.wav".into();
    t.snd[1] = "garg/gar_alert1.wav garg/gar_alert2.wav garg/gar_alert3.wav".into();
    t.snd[2] = "garg/gar_attack1.wav garg/gar_attack2.wav garg/gar_attack3.wav".into();
    t.snd[3] = "garg/gar_stomp1.wav | weapons/knife_hit1.wav".into();
    t.snd[5] = "garg/gar_pain1.wav garg/gar_pain2.wav garg/gar_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "garg/gar_die1.wav garg/gar_die2.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("headcrab");
    t.display = "Headcrab".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_delay, t.detect_radius) = (25.0, 200.0, 60.0, 8.0, 72.0, 0.3, 800.0);
    t.blood = "yellow".into();
    t.model = "models/headcrab.mdl models/hostage.mdl".into();
    t.anim[3] = "jump".into(); // (its leap is a range attack in the model; MELEE_ATTACK2 is a looping turn)
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 36.0]);
    t.fallback_glow = [200.0, 200.0, 120.0];
    t.snd[0] = "headcrab/hc_idle1.wav headcrab/hc_idle2.wav headcrab/hc_idle3.wav headcrab/hc_idle4.wav headcrab/hc_idle5.wav".into();
    t.snd[1] = "headcrab/hc_alert1.wav headcrab/hc_alert2.wav".into();
    t.snd[2] = "headcrab/hc_attack1.wav headcrab/hc_attack2.wav headcrab/hc_attack3.wav".into();
    t.snd[3] = "headcrab/hc_headbite.wav | weapons/knife_hit1.wav".into();
    t.snd[4] = String::new();
    t.snd[5] = "headcrab/hc_pain1.wav headcrab/hc_pain2.wav headcrab/hc_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "headcrab/hc_die1.wav headcrab/hc_die2.wav | player/die1.wav".into();
    out.push(t);

    // Half-Life's other monsters (valve/models, shared depot 1: CS-only and CZ-only installs have them).
    // shot_fx / hops / flank (the vortigaunt's zap, the assassin's hops, flanking) aren't form fields yet.
    let mut t = defaults("vortigaunt");
    t.display = "Vortigaunt".into();
    t.category = "alien".into();
    (t.hp, t.speed, t.walk_speed) = (70.0, 190.0, 60.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread) = (12.0, 750.0, 0.6, 0.8, 2.0);
    t.blood = "yellow".into();
    t.model = "models/islave.mdl models/hostage.mdl".into();
    t.fallback_glow = [60.0, 200.0, 60.0];
    t.snd = [
        "aslave/slv_word1.wav aslave/slv_word2.wav aslave/slv_word3.wav aslave/slv_word4.wav".into(),
        "aslave/slv_alert1.wav aslave/slv_alert3.wav aslave/slv_alert4.wav".into(),
        "weapons/electro4.wav weapons/electro5.wav weapons/electro6.wav".into(),
        String::new(),
        String::new(),
        "aslave/slv_pain1.wav aslave/slv_pain2.wav | player/pl_pain2.wav".into(),
        "aslave/slv_die1.wav aslave/slv_die2.wav | player/die1.wav".into(),
    ];
    out.push(t);

    let mut t = defaults("babygarg");
    t.display = "Baby gargantua".into();
    t.category = "boss".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.detect_radius) = (900.0, 210.0, 80.0, 35.0, 72.0, 0.8, 0.5, 1600.0);
    t.behaviour = "chase".into();
    (t.scale, t.frags) = (0.45, 5);
    t.blood = "yellow".into();
    t.model = "models/babygarg.mdl models/garg.mdl models/hostage.mdl".into();
    t.fallback_glow = [160.0, 0.0, 160.0];
    t.snd[0] = "garg/gar_idle1.wav garg/gar_idle2.wav garg/gar_breathe1.wav".into();
    t.snd[1] = "garg/gar_alert1.wav garg/gar_alert2.wav".into();
    t.snd[2] = "garg/gar_attack1.wav garg/gar_attack2.wav".into();
    t.snd[3] = "garg/gar_stomp1.wav | weapons/knife_hit1.wav".into();
    t.snd[5] = "garg/gar_pain1.wav garg/gar_pain2.wav | player/pl_pain2.wav".into();
    t.snd[6] = "garg/gar_die1.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("snark");
    t.display = "Snark".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.detect_radius, t.detect_fov) =
        (6.0, 320.0, 160.0, 4.0, 48.0, 2.5, 0.1, 1000.0, 360.0);
    t.behaviour = "chase".into();
    (t.corpse_time, t.frags) = (0.5, 0);
    t.blood = "yellow".into();
    t.model = "models/w_squeak.mdl models/headcrab.mdl".into();
    (t.anim[0], t.anim[1], t.anim[2], t.anim[3]) = ("idle".into(), "run".into(), "run".into(), "jump".into());
    (t.hull_min, t.hull_max) = ([-8.0, -8.0, 0.0], [8.0, 8.0, 30.0]);
    t.fallback_glow = [200.0, 60.0, 60.0];
    t.snd = Default::default();
    t.snd[0] = "squeek/sqk_hunt1.wav squeek/sqk_hunt2.wav squeek/sqk_hunt3.wav".into();
    t.snd[1] = "squeek/sqk_deploy1.wav".into();
    t.snd[2] = "squeek/sqk_hunt2.wav".into();
    t.snd[6] = "squeek/sqk_blast1.wav squeek/sqk_die1.wav".into();
    out.push(t);

    let mut t = defaults("babycrab");
    t.display = "Baby headcrab".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.detect_radius) = (8.0, 230.0, 80.0, 3.0, 56.0, 2.0, 0.2, 900.0);
    t.behaviour = "chase".into();
    (t.corpse_time, t.frags) = (1.5, 0);
    t.blood = "yellow".into();
    t.model = "models/baby_headcrab.mdl models/headcrab.mdl".into();
    (t.hull_min, t.hull_max) = ([-8.0, -8.0, 0.0], [8.0, 8.0, 30.0]);
    t.fallback_glow = [200.0, 200.0, 120.0];
    t.snd = Default::default();
    t.snd[2] = "headcrab/hc_attack1.wav headcrab/hc_attack2.wav".into();
    t.snd[3] = "headcrab/hc_headbite.wav".into();
    t.snd[6] = "headcrab/hc_die1.wav headcrab/hc_die2.wav".into();
    out.push(t);

    let mut t = defaults("miniturret");
    t.display = "Mini turret".into();
    t.category = "mechanical".into();
    t.hp = 80.0;
    t.moves = false;
    (t.speed, t.walk_speed) = (0.0, 0.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread, t.detect_fov, t.hearing) = (4.0, 900.0, 6.0, 0.05, 4.0, 360.0, 0.0);
    t.behaviour = "guard".into();
    t.blood = "none".into();
    t.corpse_time = 15.0;
    t.model = "models/miniturret.mdl models/sentry.mdl".into();
    (t.anim[0], t.anim[3], t.anim[5]) = ("spin".into(), "fire".into(), "die".into());
    (t.hull_min, t.hull_max) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 32.0]);
    t.snd = Default::default();
    t.snd[1] = "turret/tu_alert.wav turret/tu_spinup.wav".into();
    t.snd[2] = "turret/tu_fire1.wav".into();
    t.snd[6] = "turret/tu_die.wav turret/tu_die2.wav turret/tu_die3.wav".into();
    out.push(t);

    let mut t = defaults("assassin");
    t.display = "Assassin".into();
    t.category = "special".into();
    (t.hp, t.speed, t.walk_speed) = (60.0, 320.0, 90.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread, t.flee_hp, t.detect_fov) = (8.0, 650.0, 2.0, 0.15, 4.0, 0.3, 200.0);
    t.model = "models/hassassin.mdl models/hostage.mdl".into();
    t.fallback_glow = [20.0, 20.0, 20.0];
    t.snd = Default::default();
    t.snd[2] = "weapons/pl_gun1.wav weapons/pl_gun2.wav | weapons/usp1.wav".into();
    t.snd[5] = "player/pl_pain2.wav player/pl_pain4.wav".into();
    t.snd[6] = "player/die1.wav player/die3.wav".into();
    out.push(t);

    let mut t = defaults("soldier");
    t.display = "Soldier".into();
    t.category = "special".into();
    (t.hp, t.speed, t.walk_speed) = (110.0, 250.0, 70.0);
    t.attack = "ranged".into();
    (t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.spread, t.flee_hp) = (6.0, 1200.0, 4.0, 0.1, 5.0, 0.25);
    t.model = "models/hgrunt.mdl models/player/gign/gign.mdl".into();
    t.anim[3] = "standing_mp5".into();
    t.fallback_glow = [0.0; 3];
    t.snd = Default::default();
    t.snd[0] = "hgrunt/gr_idle1.wav hgrunt/gr_idle2.wav hgrunt/gr_idle3.wav".into();
    t.snd[1] = "hgrunt/gr_alert1.wav".into();
    t.snd[2] = "hgrunt/gr_mgun1.wav hgrunt/gr_mgun2.wav hgrunt/gr_mgun3.wav | weapons/mp5-1.wav".into();
    t.snd[5] = "hgrunt/gr_pain1.wav hgrunt/gr_pain2.wav hgrunt/gr_pain3.wav | player/pl_pain2.wav".into();
    t.snd[6] = "hgrunt/gr_die1.wav hgrunt/gr_die2.wav hgrunt/gr_die3.wav | player/die1.wav".into();
    out.push(t);

    let mut t = defaults("gonarch");
    t.display = "Gonarch".into();
    t.category = "boss".into();
    (t.hp, t.speed, t.walk_speed, t.damage, t.attack_range, t.attack_rate, t.attack_delay, t.detect_radius) = (3500.0, 190.0, 90.0, 50.0, 96.0, 0.7, 0.6, 2000.0);
    t.behaviour = "chase".into();
    (t.scale, t.frags) = (0.55, 10);
    t.blood = "yellow".into();
    t.model = "models/big_mom.mdl models/hostage.mdl".into();
    t.fallback_glow = [200.0, 160.0, 40.0];
    t.snd = Default::default();
    t.snd[0] = "gonarch/gon_sack1.wav gonarch/gon_sack2.wav gonarch/gon_sack3.wav".into();
    t.snd[1] = "gonarch/gon_alert1.wav gonarch/gon_alert2.wav gonarch/gon_alert3.wav".into();
    t.snd[2] = "gonarch/gon_attack1.wav gonarch/gon_attack2.wav gonarch/gon_attack3.wav".into();
    t.snd[3] = "gonarch/gon_step1.wav | weapons/knife_hit1.wav".into();
    t.snd[5] = "gonarch/gon_pain2.wav gonarch/gon_pain4.wav gonarch/gon_pain5.wav | player/pl_pain2.wav".into();
    t.snd[6] = "gonarch/gon_die1.wav | player/die1.wav".into();
    out.push(t);
    out
}

pub fn builtin(name: &str) -> Option<Creature> {
    builtins().into_iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

// ---- reading values the way the fork does ---------------------------------------------------

/// atof: the longest number at the start ("12abc" = 12, "abc" = 0).
fn atof(s: &str) -> f32 {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = false;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits = true;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits = true;
        }
    }
    if digits && i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            i = j;
        }
    }
    if !digits {
        return 0.0;
    }
    s[..i].parse::<f64>().map(|v| v as f32).unwrap_or(0.0)
}

fn atoi(s: &str) -> i32 {
    let v = atof(s.split(['.', 'e', 'E']).next().unwrap_or(""));
    v.clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn finite(v: f32) -> bool {
    v.is_finite() && v.abs() < 1e30
}

/// LsNum: not finite keeps the old value, the rest clamped.
fn num(value: &str, lo: f32, hi: f32, keep: f32) -> f32 {
    let v = atof(value);
    if !finite(v) {
        return keep;
    }
    v.clamp(lo, hi)
}

fn range(key: &str) -> (f32, f32) {
    RANGES.iter().find(|r| r.0 == key).map(|r| (r.1, r.2)).unwrap_or((f32::MIN, f32::MAX))
}

/// LsNameIndex: a name from the list (any case), or its number, else keep.
fn name_index(value: &str, names: &[&str], keep: &str) -> String {
    if let Some(n) = names.iter().find(|n| n.eq_ignore_ascii_case(value)) {
        return n.to_string();
    }
    if value.starts_with(|c: char| c.is_ascii_digit()) {
        let n = atoi(value);
        if n >= 0 && (n as usize) < names.len() {
            return names[n as usize].to_string();
        }
    }
    keep.to_string()
}

/// LsParseVector: three numbers within +-8192, else keep.
fn vector(value: &str, keep: [f32; 3]) -> [f32; 3] {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() < 3 {
        return keep;
    }
    let mut v = [0.0; 3];
    for i in 0..3 {
        let Ok(x) = parts[i].parse::<f32>() else { return keep };
        if !finite(x) || !(-8192.0..=8192.0).contains(&x) {
            return keep;
        }
        v[i] = x;
    }
    v
}

fn color(value: &str, keep: [f32; 3]) -> [f32; 3] {
    vector(value, keep).map(|c| c.clamp(0.0, 255.0))
}

/// Q_strlcpy into char[max + 1]: at most `max` bytes.
fn cut(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// LsSetTypeKey for everything but `base` (which needs the types above: `resolve`).
fn set_key(t: &mut Creature, key: &str, value: &str) {
    let k = key.to_ascii_lowercase();
    let n = |keep: f32, key: &str| {
        let (lo, hi) = range(key);
        num(value, lo, hi, keep)
    };
    match k.as_str() {
        "name" | "display" => t.display = cut(value, MAX_DISPLAY),
        "category" => t.category = name_index(value, &CATEGORIES, &t.category),
        "hp" | "health" => t.hp = n(t.hp, "hp"),
        "speed" => t.speed = n(t.speed, "speed"),
        "walk_speed" => t.walk_speed = n(t.walk_speed, "walk_speed"),
        "aggression" => t.aggression = n(t.aggression, "aggression"),
        "detect_radius" | "detect" => t.detect_radius = n(t.detect_radius, "detect_radius"),
        "detect_fov" | "fov" => t.detect_fov = n(t.detect_fov, "detect_fov"),
        "hearing" => t.hearing = n(t.hearing, "hearing"),
        "attack" => t.attack = name_index(value, &ATTACKS, &t.attack),
        "damage" => t.damage = n(t.damage, "damage"),
        "attack_range" => t.attack_range = n(t.attack_range, "attack_range"),
        "attack_rate" => t.attack_rate = n(t.attack_rate, "attack_rate"),
        "attack_delay" => t.attack_delay = n(t.attack_delay, "attack_delay"),
        "spread" => t.spread = n(t.spread, "spread"),
        "flee_hp" => t.flee_hp = n(t.flee_hp, "flee_hp"),
        "behaviour" | "behavior" => t.behaviour = name_index(value, &BEHAVIOURS, &t.behaviour),
        "radius" | "guard_radius" => t.radius = n(t.radius, "radius"),
        "target" => t.target = name_index(value, &TARGETS, &t.target),
        "hates" => {
            let v = value.to_ascii_lowercase();
            t.hates = if ["t", "terrorist", "1"].contains(&v.as_str()) { "t" } else if ["ct", "2"].contains(&v.as_str()) { "ct" } else { "all" }.into();
        }
        "respawn" => t.respawn = n(t.respawn, "respawn"),
        "corpse_time" => t.corpse_time = n(t.corpse_time, "corpse_time"),
        "moves" => t.moves = atoi(value) != 0,
        "frags" => t.frags = atoi(value).clamp(-100, 100),
        "money" => t.money = atoi(value).clamp(-16000, 16000),
        "model" | "models" => t.model = cut(value, MAX_MODELS),
        "skin" => t.skin = atoi(value).clamp(0, 255),
        "body" => t.body = atoi(value).clamp(0, 255),
        "scale" => t.scale = n(t.scale, "scale"),
        "hull_min" => t.hull_min = vector(value, t.hull_min),
        "hull_max" => t.hull_max = vector(value, t.hull_max),
        "glow" => t.glow = color(value, t.glow),
        "fallback_glow" => t.fallback_glow = color(value, t.fallback_glow),
        "blood" => {
            let v = value.to_ascii_lowercase();
            t.blood = if v == "yellow" { "yellow" } else if v == "none" { "none" } else { "red" }.into();
        }
        "sounds" => {
            t.snd = Default::default();
            t.snd[0] = cut(value, MAX_SOUND);
        }
        "ls" => {
            // ls = model=<path> anims=<walk> <run> <attack> <die>
            const ORDER: [usize; 4] = [1, 2, 3, 5];
            let mut next: Option<usize> = None;
            for tok in value.split_whitespace() {
                if tok.len() >= 6 && tok[..6].eq_ignore_ascii_case("model=") {
                    t.model = cut(&tok[6..], MAX_MODELS);
                    next = None;
                } else if tok.len() >= 6 && tok[..6].eq_ignore_ascii_case("anims=") {
                    next = Some(0);
                    if tok.len() > 6 {
                        t.anim[ORDER[0]] = cut(&tok[6..], MAX_ANIM);
                        next = Some(1);
                    }
                } else if let Some(i) = next.filter(|i| *i < ORDER.len()) {
                    t.anim[ORDER[i]] = cut(tok, MAX_ANIM);
                    next = Some(i + 1);
                }
            }
        }
        other => {
            if let Some(i) = ANIMS.iter().position(|a| *a == other) {
                t.anim[i] = cut(value, MAX_ANIM);
            } else if let Some(i) = SOUNDS.iter().position(|s| *s == other) {
                t.snd[i] = cut(value, MAX_SOUND);
            }
            // (mm, bot: the kit's backends; anything else: ignored, as the fork does)
        }
    }
}

/// What the fork does to every type once read: walk no faster than run, a bad hull back to
/// the default (LsLoadTypes / LsCheckHull).
pub fn hull_ok(min: [f32; 3], max: [f32; 3]) -> bool {
    min[2] <= 0.0 && min[2] >= -64.0 && max[2] >= 30.0 && (0..3).all(|i| finite(min[i]) && finite(max[i]) && min[i] < max[i] && max[i] - min[i] <= 256.0)
}

fn settle(t: &mut Creature) {
    if t.walk_speed > t.speed {
        t.walk_speed = t.speed;
    }
    if !hull_ok(t.hull_min, t.hull_max) {
        t.hull_min = [-16.0, -16.0, 0.0];
        t.hull_max = [16.0, 16.0, 72.0];
    }
    // never wider than a player: the engine moves anything wider with the 64-unit hull
    if t.hull_min[0] < -16.0 || t.hull_min[1] < -16.0 || t.hull_max[0] > 16.0 || t.hull_max[1] > 16.0 {
        (t.hull_min[0], t.hull_min[1], t.hull_max[0], t.hull_max[1]) = (-16.0, -16.0, 16.0, 16.0);
    }
}

// ---- writing values ---------------------------------------------------------------------

/// A number as the file writes it: whole numbers plain, else up to 3 decimals.
pub fn fmt_num(v: f32) -> String {
    let r = (v as f64 * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn fmt_vec(v: [f32; 3]) -> String {
    v.iter().map(|x| fmt_num(*x)).collect::<Vec<_>>().join(" ")
}

impl Creature {
    /// One key's value as the file writes it (canonical key names, see KEYS).
    pub fn get(&self, key: &str) -> String {
        match key {
            "name" => self.display.clone(),
            "category" => self.category.clone(),
            "hp" => fmt_num(self.hp),
            "speed" => fmt_num(self.speed),
            "walk_speed" => fmt_num(self.walk_speed),
            "aggression" => fmt_num(self.aggression),
            "detect_radius" => fmt_num(self.detect_radius),
            "detect_fov" => fmt_num(self.detect_fov),
            "hearing" => fmt_num(self.hearing),
            "attack" => self.attack.clone(),
            "damage" => fmt_num(self.damage),
            "attack_range" => fmt_num(self.attack_range),
            "attack_rate" => fmt_num(self.attack_rate),
            "attack_delay" => fmt_num(self.attack_delay),
            "spread" => fmt_num(self.spread),
            "flee_hp" => fmt_num(self.flee_hp),
            "behaviour" => self.behaviour.clone(),
            "radius" => fmt_num(self.radius),
            "target" => self.target.clone(),
            "hates" => self.hates.clone(),
            "respawn" => fmt_num(self.respawn),
            "corpse_time" => fmt_num(self.corpse_time),
            "moves" => (if self.moves { "1" } else { "0" }).into(),
            "frags" => self.frags.to_string(),
            "money" => self.money.to_string(),
            "model" => self.model.clone(),
            "skin" => self.skin.to_string(),
            "body" => self.body.to_string(),
            "scale" => fmt_num(self.scale),
            "hull_min" => fmt_vec(self.hull_min),
            "hull_max" => fmt_vec(self.hull_max),
            "glow" => fmt_vec(self.glow),
            "fallback_glow" => fmt_vec(self.fallback_glow),
            "blood" => self.blood.clone(),
            k => {
                if let Some(i) = ANIMS.iter().position(|a| *a == k) {
                    self.anim[i].clone()
                } else if let Some(i) = SOUNDS.iter().position(|s| *s == k) {
                    self.snd[i].clone()
                } else {
                    String::new()
                }
            }
        }
    }

    /// Everything the game would use, as one comparable string.
    fn fingerprint(&self) -> String {
        let mut s = self.name.to_ascii_lowercase();
        for k in KEYS {
            s.push('\u{1}');
            s.push_str(&self.get(k));
        }
        s
    }

    /// As the game would hold it after reading these values back (clamped, cut, settled).
    pub fn settled(&self) -> Creature {
        let mut t = defaults(&self.name);
        for k in KEYS {
            set_key(&mut t, k, &self.get(k));
        }
        t.base = self.base.clone();
        settle(&mut t);
        t
    }
}

/// The canonical key for a key the file uses (aliases folded), or None for keys the Studio
/// doesn't edit (`ls`, `sounds`, `mm`, `bot`, unknown ones).
fn canonical(key: &str) -> Option<&'static str> {
    let k = key.to_ascii_lowercase();
    let k = match k.as_str() {
        "display" => "name",
        "health" => "hp",
        "detect" => "detect_radius",
        "fov" => "detect_fov",
        "behavior" => "behaviour",
        "guard_radius" => "radius",
        "models" => "model",
        other => other,
    };
    KEYS.iter().find(|x| **x == k).copied()
}

// ---- the file: lines, sections, keys --------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Line {
    Other,
    Section(String),
    /// key, value, and where the value sits in the raw line (start, end: before the comment)
    Key { key: String, value: String, at: (usize, usize) },
}

/// Where a line's comment starts (the fork cuts at the first ; # or //).
fn comment_at(line: &str) -> usize {
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] == b';' || b[i] == b'#' || (b[i] == b'/' && b.get(i + 1) == Some(&b'/')) {
            return i;
        }
    }
    b.len()
}

/// One line as the fork reads it.
fn classify(raw: &str) -> Line {
    // (the fork reads 511 bytes of a line; the rest is skipped)
    let mut end = raw.len().min(MAX_LINE);
    while !raw.is_char_boundary(end) {
        end -= 1;
    }
    let line = &raw[..end];
    let line = line.trim_end_matches(['\r', '\n']);
    let c = comment_at(line);
    let body = &line[..c];
    let t = body.trim_matches([' ', '\t']);
    if t.is_empty() {
        return Line::Other;
    }
    if let Some(rest) = t.strip_prefix('[') {
        let name = rest.split(']').next().unwrap_or("");
        return Line::Section(name.trim_matches([' ', '\t']).to_string());
    }
    let Some(eq) = body.find('=') else { return Line::Other };
    let key = body[..eq].trim_matches([' ', '\t']).to_string();
    let after = &body[eq + 1..];
    let lead = after.len() - after.trim_start_matches([' ', '\t']).len();
    let value = after.trim_matches([' ', '\t']).to_string();
    let start = eq + 1 + lead;
    Line::Key { key, value: value.clone(), at: (start, start + value.len()) }
}

/// The file's text in lines, with its line ends kept.
#[derive(Debug, Clone)]
pub struct Doc {
    lines: Vec<String>,
    crlf: bool,
    /// the last line ended with a line break
    final_nl: bool,
}

impl Doc {
    pub fn parse(text: &str) -> Doc {
        let crlf = text.contains("\r\n");
        let final_nl = text.is_empty() || text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() { vec![] } else { body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect() };
        Doc { lines, crlf, final_nl }
    }

    pub fn text(&self) -> String {
        let nl = if self.crlf { "\r\n" } else { "\n" };
        let mut s = self.lines.join(nl);
        if self.final_nl && !self.lines.is_empty() {
            s.push_str(nl);
        }
        s
    }

    fn kinds(&self) -> Vec<Line> {
        self.lines.iter().map(|l| classify(l)).collect()
    }

    /// Sections: (name, header line, end line exclusive), in file order.
    fn sections(&self) -> Vec<(String, usize, usize)> {
        let kinds = self.kinds();
        let mut out: Vec<(String, usize, usize)> = Vec::new();
        for (i, k) in kinds.iter().enumerate() {
            if let Line::Section(n) = k {
                if let Some(last) = out.last_mut() {
                    last.2 = i;
                }
                out.push((n.clone(), i, kinds.len()));
            }
        }
        out
    }
}

/// A type as read from the file: what the game uses, and where it came from.
#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    #[serde(flatten)]
    pub t: Creature,
    pub builtin: bool,
    #[serde(rename = "inFile")]
    pub in_file: bool,
    /// keys the file sets for it (canonical where the Studio knows them)
    pub set: Vec<String>,
    /// past the fork's 32 types: skipped
    pub skipped: bool,
}

/// The types the game would have from this file, in the fork's order (built-ins first).
pub fn resolve(doc: &Doc) -> Vec<Resolved> {
    let mut list: Vec<Resolved> = builtins().into_iter().map(|t| Resolved { t, builtin: true, in_file: false, set: vec![], skipped: false }).collect();
    let mut cur: Option<usize> = None;
    for k in doc.kinds() {
        match k {
            Line::Section(name) => {
                cur = match list.iter().position(|r| !r.skipped && r.t.name.eq_ignore_ascii_case(&name)) {
                    Some(i) => Some(i),
                    None if name.is_empty() => None,
                    None if list.iter().filter(|r| !r.skipped).count() >= MAX_TYPES => {
                        if !list.iter().any(|r| r.t.name.eq_ignore_ascii_case(&name)) {
                            list.push(Resolved { t: defaults(&cut(&name, MAX_NAME)), builtin: false, in_file: true, set: vec![], skipped: true });
                        }
                        None
                    }
                    None => {
                        list.push(Resolved { t: defaults(&cut(&name, MAX_NAME)), builtin: false, in_file: true, set: vec![], skipped: false });
                        Some(list.len() - 1)
                    }
                };
                if let Some(i) = cur {
                    list[i].in_file = true;
                }
            }
            Line::Key { key, value, .. } => {
                let Some(i) = cur else { continue };
                let low = key.to_ascii_lowercase();
                if low == "base" {
                    // copies a type above (any but itself), keeps the name
                    if let Some(b) = list.iter().position(|r| !r.skipped && r.t.name.eq_ignore_ascii_case(&value)).filter(|b| *b != i) {
                        let name = list[i].t.name.clone();
                        let mut copy = list[b].t.clone();
                        copy.name = name;
                        list[i].t = copy;
                    }
                    list[i].t.base = value.clone();
                } else {
                    set_key(&mut list[i].t, &key, &value);
                }
                let shown = canonical(&low).map(String::from).unwrap_or(low);
                if !list[i].set.contains(&shown) {
                    list[i].set.push(shown);
                }
            }
            Line::Other => {}
        }
    }
    for r in &mut list {
        settle(&mut r.t);
    }
    list
}

// ---- checking -------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Problem {
    /// the type's name ("" = the whole file)
    pub at: String,
    /// the key it's about ("" = the type)
    pub field: String,
    pub text: String,
    pub error: bool,
}

fn problem(at: &str, field: &str, text: impl Into<String>, error: bool) -> Problem {
    Problem { at: at.into(), field: field.into(), text: text.into(), error }
}

/// A type name commands and plugins can use: letters, digits and _, not a number (an entity
/// index) and not "all".
pub fn name_problem(n: &str) -> Option<String> {
    if n.is_empty() {
        return Some("Give it a name.".into());
    }
    if n.len() > MAX_NAME {
        return Some(format!("Keep the name to {MAX_NAME} characters."));
    }
    if !n.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
        return Some("Use small letters, digits and _ only (commands and plugins name it).".into());
    }
    if n.bytes().all(|c| c.is_ascii_digit()) {
        return Some("A name can't be only digits: commands would read it as an entity number.".into());
    }
    if n == "all" {
        return Some("\"all\" means every creature in commands: pick another name.".into());
    }
    None
}

/// A model candidate path: models/....mdl, plain parts.
pub fn safe_model(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    (2..=5).contains(&parts.len())
        && parts[0].eq_ignore_ascii_case("models")
        && parts.iter().all(|x| crate::content::safe_part(x) && !x.contains(' '))
        && p.to_ascii_lowercase().ends_with(".mdl")
        && p.len() < 64
}

/// A sound path under sound/: plain parts, .wav.
pub fn safe_sound(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    (1..=4).contains(&parts.len()) && parts.iter().all(|x| crate::content::safe_part(x) && !x.contains(' ')) && p.to_ascii_lowercase().ends_with(".wav") && p.len() < 64
}

/// What's wrong with these types, before anything is written. Errors stop a save; notes don't.
pub fn check(types: &[Creature]) -> Vec<Problem> {
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let builtin_names: Vec<String> = builtins().into_iter().map(|t| t.name).collect();
    let extra = types.iter().filter(|t| !builtin_names.contains(&t.name)).count();
    if builtin_names.len() + extra > MAX_TYPES {
        out.push(problem("", "", format!("The game holds {MAX_TYPES} types at most ({} built in + {} of yours): remove {}.", builtin_names.len(), MAX_TYPES - builtin_names.len(), builtin_names.len() + extra - MAX_TYPES), true));
    }
    for (i, t) in types.iter().enumerate() {
        let at = t.name.as_str();
        if let Some(p) = name_problem(&t.name) {
            out.push(problem(at, "name", p, true));
        }
        if seen.contains(&t.name) {
            out.push(problem(at, "name", format!("There's already a type called {}.", t.name), true));
        }
        seen.push(t.name.clone());
        if !t.base.is_empty() {
            let above = builtin_names.iter().any(|b| b.eq_ignore_ascii_case(&t.base)) || types[..i].iter().any(|x| x.name.eq_ignore_ascii_case(&t.base));
            if t.base.eq_ignore_ascii_case(&t.name) && !builtin_names.contains(&t.name) {
                out.push(problem(at, "base", "A type can't start from itself.", true));
            } else if !above {
                out.push(problem(at, "base", format!("It starts from \"{}\", which isn't a built-in type or one listed above it.", t.base), true));
            }
        }
        // text: the file cuts a value at ; # or //, and each has a size
        for key in KEYS {
            let v = t.get(key);
            // (the file's `name =` is the shown name: reported as "display", apart from the type's name)
            let k: &str = if *key == "name" { "display" } else { key };
            if comment_at(&v) < v.len() {
                out.push(problem(at, k, "No ; # or // in a value: the game reads them as the start of a note and cuts the rest.", true));
            }
            if v.contains(['\r', '\n', '[', ']']) && k != "display" {
                out.push(problem(at, k, "No line breaks or [ ] in a value.", true));
            }
            if v.len() + key.len() + 3 > MAX_LINE {
                out.push(problem(at, k, "Too long for one line of the file (511 characters).", true));
            }
        }
        if t.display.len() > MAX_DISPLAY {
            out.push(problem(at, "display", format!("The shown name is {} characters; the game keeps {MAX_DISPLAY}.", t.display.len()), true));
        }
        if t.display.contains(['\r', '\n']) {
            out.push(problem(at, "display", "No line breaks in the shown name.", true));
        }
        // numbers: the game clamps, so out of range is a mistake to fix here
        let nums: [(&str, f32); 21] = [
            ("hp", t.hp), ("speed", t.speed), ("walk_speed", t.walk_speed), ("aggression", t.aggression), ("detect_radius", t.detect_radius),
            ("detect_fov", t.detect_fov), ("hearing", t.hearing), ("damage", t.damage), ("attack_range", t.attack_range), ("attack_rate", t.attack_rate),
            ("attack_delay", t.attack_delay), ("spread", t.spread), ("flee_hp", t.flee_hp), ("radius", t.radius), ("respawn", t.respawn),
            ("corpse_time", t.corpse_time), ("frags", t.frags as f32), ("money", t.money as f32), ("skin", t.skin as f32), ("body", t.body as f32), ("scale", t.scale),
        ];
        for (k, v) in nums {
            let (lo, hi) = range(k);
            if !finite(v) {
                out.push(problem(at, k, "That isn't a number.", true));
            } else if v < lo || v > hi {
                out.push(problem(at, k, format!("Keep it between {} and {}: the game would use {}.", fmt_num(lo), fmt_num(hi), fmt_num(v.clamp(lo, hi))), true));
            }
        }
        for (k, v) in [("glow", t.glow), ("fallback_glow", t.fallback_glow)] {
            if v.iter().any(|c| !finite(*c) || *c < 0.0 || *c > 255.0) {
                out.push(problem(at, k, "Colors are three numbers from 0 to 255.", true));
            }
        }
        let picks: [(&str, &str, &[&str]); 4] = [("category", &t.category, &CATEGORIES), ("attack", &t.attack, &ATTACKS), ("behaviour", &t.behaviour, &BEHAVIOURS), ("target", &t.target, &TARGETS)];
        for (k, v, list) in picks {
            if !list.contains(&v) {
                out.push(problem(at, k, format!("Pick one of: {}.", list.join(", ")), true));
            }
        }
        if !["all", "t", "ct"].contains(&t.hates.as_str()) {
            out.push(problem(at, "hates", "Pick all, t or ct.", true));
        }
        if !["red", "yellow", "none"].contains(&t.blood.as_str()) {
            out.push(problem(at, "blood", "Pick red, yellow or none.", true));
        }
        // hull: a bad one falls back to the default, quietly in the form: say so
        let hv = t.hull_min.iter().chain(t.hull_max.iter()).all(|v| finite(*v) && v.abs() <= 8192.0);
        if !hv || !hull_ok(t.hull_min, t.hull_max) {
            out.push(problem(at, "hull_min", "Its size doesn't work: the bottom must be at 0 or a little below (to -64), the top at least 30 up, each side smaller than the other and at most 256 across. The game would use a person's size instead.", true));
        }
        if hv && hull_ok(t.hull_min, t.hull_max) && (t.hull_min[0] < -16.0 || t.hull_min[1] < -16.0 || t.hull_max[0] > 16.0 || t.hull_max[1] > 16.0) {
            out.push(problem(at, "hull_min", "Wider than a player: the game narrows it to 32 so it fits through the doors the bots' nav leads through. Make the model look bigger with Size instead.", false));
        }
        // models
        let models: Vec<&str> = t.model.split_whitespace().collect();
        if models.is_empty() {
            out.push(problem(at, "model", "Pick a model.", true));
        }
        for m in &models {
            if !safe_model(m) {
                out.push(problem(at, "model", format!("\"{m}\" isn't a model path like models/zombie.mdl."), true));
            }
        }
        for (i, a) in t.anim.iter().enumerate() {
            if a.contains(char::is_whitespace) {
                out.push(problem(at, ANIMS[i], "One sequence name, no spaces.", true));
            }
            if a.len() > MAX_ANIM {
                out.push(problem(at, ANIMS[i], format!("Sequence names are {MAX_ANIM} characters at most."), true));
            }
        }
        for (i, s) in t.snd.iter().enumerate() {
            for f in s.split(|c: char| c.is_whitespace() || c == '|').filter(|x| !x.is_empty()) {
                if !safe_sound(f) {
                    out.push(problem(at, SOUNDS[i], format!("\"{f}\" isn't a sound path like zombie/zo_idle1.wav (inside sound/)."), true));
                }
            }
            if s.len() > MAX_SOUND {
                out.push(problem(at, SOUNDS[i], format!("Too many sounds: the game keeps {MAX_SOUND} characters of the list."), true));
            }
        }
        // notes: allowed, but probably not what was meant
        if t.walk_speed > t.speed && t.moves {
            out.push(problem(at, "walk_speed", "Walking is faster than running: the game walks it at its running speed.", false));
        }
        if !t.moves && t.behaviour != "guard" && t.behaviour != "idle" {
            out.push(problem(at, "behaviour", "It doesn't move, so it can only stand guard or stand still.", false));
        }
        if t.attack == "melee" && t.attack_range > 200.0 {
            out.push(problem(at, "attack_range", "A melee attack that reaches this far looks odd: ranged, maybe?", false));
        }
        if t.attack != "none" && t.damage == 0.0 {
            out.push(problem(at, "damage", "It attacks but does no damage.", false));
        }
        if t.aggression == 0.0 && t.flee_hp == 0.0 && t.attack == "none" {
            out.push(problem(at, "aggression", "It never fights and never runs: it'll just wander about.", false));
        }
        if t.detect_radius < 100.0 {
            out.push(problem(at, "detect_radius", "It'll only notice someone almost touching it.", false));
        }
    }
    out
}

fn rate_words(r: f32) -> String {
    if (r - 1.0).abs() < 0.05 {
        "once a second".into()
    } else if r < 1.0 {
        format!("every {} s", fmt_num((1.0 / r * 10.0).round() / 10.0))
    } else {
        format!("{} times a second", fmt_num(r))
    }
}

/// One line in plain words about how a type plays.
pub fn summary(t: &Creature) -> String {
    let speed = |v: f32| if v >= 280.0 { "fast" } else if v >= 200.0 { "quick" } else if v >= 120.0 { "slow" } else { "very slow" };
    let mut s = String::new();
    let tough = if t.hp >= 1500.0 { "a boss-tough" } else if t.hp >= 300.0 { "a tough" } else if t.hp <= 60.0 { "a fragile" } else { "an average" };
    let what = if !t.moves { "turret" } else { t.category.as_str() };
    s.push_str(&format!("{tough} {what}"));
    if t.moves {
        s.push_str(&format!(", {} on its feet", speed(t.speed)));
    }
    let mood = if t.aggression >= 0.8 {
        "attacks anyone it notices"
    } else if t.aggression > 0.0 {
        "attacks whoever comes close"
    } else if t.flee_hp > 0.0 || t.attack == "none" {
        "never starts a fight and runs from people"
    } else {
        "only fights back"
    };
    s.push_str(&format!(", {mood}"));
    match t.attack.as_str() {
        "melee" => s.push_str(&format!(" (hits for {} {})", fmt_num(t.damage), rate_words(t.attack_rate))),
        "ranged" => s.push_str(&format!(" (shoots for {} {} from up to {} units)", fmt_num(t.damage), rate_words(t.attack_rate), fmt_num(t.attack_range))),
        _ => {}
    }
    let home = match t.behaviour.as_str() {
        "chase" => "hunts the nearest enemy down".to_string(),
        "guard" => format!("guards its spot ({} units)", fmt_num(t.radius)),
        "patrol" => "patrols a route".to_string(),
        "wander" => "wanders around until it notices someone".to_string(),
        "flee" => "keeps away from people".to_string(),
        _ => "stands still until it notices someone".to_string(),
    };
    s.push_str(&format!("; {home}"));
    if t.flee_hp > 0.0 && t.aggression > 0.0 {
        s.push_str(&format!(", runs when below {}% health", (t.flee_hp * 100.0).round()));
    }
    if t.respawn > 0.0 {
        s.push_str(&format!(", comes back {} s after dying", fmt_num(t.respawn)));
    }
    let mut out = s;
    if let Some(c) = out.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    out.push('.');
    out
}

// ---- saving ---------------------------------------------------------------------------------

/// What a type would be with none of its own keys: its base, or the built-in of the same
/// name, or the neutral defaults. `before`: the types above it (as asked for).
fn inherited(t: &Creature, before: &[Creature]) -> Creature {
    let from = if !t.base.is_empty() {
        before.iter().rev().find(|x| x.name.eq_ignore_ascii_case(&t.base)).cloned().or_else(|| builtin(&t.base))
    } else {
        builtin(&t.name)
    };
    let mut c = from.unwrap_or_else(|| defaults(&t.name));
    c.name = t.name.clone();
    c.base = t.base.clone();
    c
}

fn replace_value(raw: &str, at: (usize, usize), value: &str) -> String {
    let (a, b) = at;
    let mut s = String::new();
    s.push_str(&raw[..a]);
    if a > 0 && !raw[..a].ends_with([' ', '\t']) && !value.is_empty() {
        s.push(' ');
    }
    s.push_str(value);
    let rest = &raw[b..];
    // (a value that got longer or shorter: one space before a note that follows)
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) && !value.is_empty() {
        s.push(' ');
    }
    s.push_str(rest);
    s
}

/// Writes these types into the file's text (see the module notes). `types`: every type the
/// player has, built-ins included; a section the list doesn't name is taken out (a built-in
/// goes back to how the game has it).
pub fn write(old: &str, types: &[Creature]) -> Result<String, String> {
    let mut doc = Doc::parse(old);
    let want: Vec<Creature> = types.iter().map(|t| t.settled()).collect();

    // 1. sections nobody wants any more go (header to the next header)
    let keep = |n: &str| want.iter().any(|t| t.name.eq_ignore_ascii_case(n));
    let gone: Vec<(usize, usize)> = doc.sections().into_iter().filter(|(n, _, _)| !keep(n)).map(|(_, a, b)| (a, b)).collect();
    for (a, b) in gone.into_iter().rev() {
        // (keep notes that belong to what follows: trailing comment lines just above the next section)
        let mut end = b;
        while end > a + 1 && end < doc.lines.len() && !matches!(classify(&doc.lines[end - 1]), Line::Key { .. }) {
            end -= 1;
        }
        doc.lines.drain(a..end);
        // (no double blank line where it was)
        if a > 0 && a < doc.lines.len() && doc.lines[a - 1].trim().is_empty() && doc.lines[a].trim().is_empty() {
            doc.lines.remove(a);
        }
        // (nor a trailing one at the end)
        while end >= b && doc.lines.last().is_some_and(|l| l.trim().is_empty()) && doc.lines.len() > 1 && a >= doc.lines.len() {
            doc.lines.pop();
        }
    }

    // 2. each wanted type: edit its section in place, or add one
    for (i, t) in want.iter().enumerate() {
        let base_of = inherited(t, &want[..i]);
        let in_file = doc.sections().iter().any(|(n, _, _)| n.eq_ignore_ascii_case(&t.name));
        if !in_file {
            let diff: Vec<&str> = KEYS.iter().copied().filter(|k| t.get(k) != base_of.get(k)).collect();
            if diff.is_empty() && t.base.is_empty() && builtin(&t.name).is_some() {
                continue; // a built-in as the game has it: nothing to write
            }
            let mut add = Vec::new();
            if doc.lines.last().is_some_and(|l| !l.trim().is_empty()) {
                add.push(String::new());
            }
            add.push(format!("[{}]", t.name));
            if !t.base.is_empty() {
                add.push(format!("base = {}", t.base));
            }
            for k in diff {
                add.push(format!("{k} = {}", t.get(k)));
            }
            if doc.lines.is_empty() {
                doc.final_nl = true;
            }
            doc.lines.extend(add);
            continue;
        }
        edit_section(&mut doc, t);
    }

    // 3. read back as the game would: it must be exactly what was asked for
    let text = doc.text();
    let back = resolve(&Doc::parse(&text));
    for t in &want {
        match back.iter().find(|r| !r.skipped && r.t.name.eq_ignore_ascii_case(&t.name)) {
            Some(r) if r.t.fingerprint() == t.fingerprint() => {}
            Some(r) => {
                let k = KEYS.iter().find(|k| r.t.get(k) != t.get(k)).copied().unwrap_or("?");
                return Err(format!("Not saved: {} would come out different in the game ({k}: {} instead of {}). The file may have something the Studio doesn't understand there.", t.name, r.t.get(k), t.get(k)));
            }
            None => return Err(format!("Not saved: {} wouldn't be in the game (too many types?).", t.name)),
        }
    }
    Ok(text)
}

/// Edits one type's section(s): a key the file sets keeps its line, a key that now differs
/// from what it inherits is added after the last key of its last section; then whatever still
/// comes out wrong (a later `ls =`, `sounds =` or `base =` line) is set again at the end.
fn edit_section(doc: &mut Doc, t: &Creature) {
    let secs: Vec<(usize, usize)> = doc.sections().into_iter().filter(|(n, _, _)| n.eq_ignore_ascii_case(&t.name)).map(|(_, a, b)| (a, b)).collect();
    let Some(&(last_a, _)) = secs.last() else { return };
    // base: set or changed
    let base_line = secs.iter().flat_map(|(a, b)| (*a + 1..*b).collect::<Vec<_>>()).find(|i| matches!(classify(&doc.lines[*i]), Line::Key { ref key, .. } if key.eq_ignore_ascii_case("base")));
    match base_line {
        Some(i) => {
            if let Line::Key { value, at, .. } = classify(&doc.lines[i]) {
                if !value.eq_ignore_ascii_case(&t.base) {
                    if t.base.is_empty() {
                        doc.lines.remove(i);
                        return edit_section(doc, t);
                    }
                    doc.lines[i] = replace_value(&doc.lines[i], at, &t.base);
                }
            }
        }
        None if !t.base.is_empty() => {
            doc.lines.insert(secs[0].0 + 1, format!("base = {}", t.base));
            return edit_section(doc, t);
        }
        None => {}
    }
    let secs: Vec<(usize, usize)> = doc.sections().into_iter().filter(|(n, _, _)| n.eq_ignore_ascii_case(&t.name)).map(|(_, a, b)| (a, b)).collect();
    // what the file gives it now: keys that already come out right are left alone
    let now = resolve(doc).into_iter().find(|r| !r.skipped && r.t.name.eq_ignore_ascii_case(&t.name)).map(|r| r.t);
    let mut adds: Vec<String> = Vec::new();
    for k in KEYS {
        let want = t.get(k);
        if now.as_ref().is_some_and(|n| n.get(k) == want) {
            continue;
        }
        // the last line in its sections that sets this key (aliases count)
        let found = secs.iter().flat_map(|(a, b)| (*a + 1..*b).collect::<Vec<_>>()).filter(|i| matches!(classify(&doc.lines[*i]), Line::Key { ref key, .. } if canonical(key) == Some(k))).last();
        match found {
            Some(i) => {
                if let Line::Key { at, .. } = classify(&doc.lines[i]) {
                    doc.lines[i] = replace_value(&doc.lines[i], at, &want);
                }
            }
            None => adds.push(format!("{k} = {want}")),
        }
    }
    if !adds.is_empty() {
        let (a, b) = doc.sections().into_iter().filter(|(n, _, _)| n.eq_ignore_ascii_case(&t.name)).map(|(_, a, b)| (a, b)).last().unwrap_or((last_a, last_a + 1));
        let after = (a + 1..b).filter(|i| matches!(classify(&doc.lines[*i]), Line::Key { .. })).last().unwrap_or(a);
        for (n, line) in adds.into_iter().enumerate() {
            doc.lines.insert(after + 1 + n, line);
        }
    }
    // anything still different (a later ls/sounds/base line wins over ours): set it again last
    let text = doc.text();
    let back = resolve(&Doc::parse(&text));
    if let Some(r) = back.iter().find(|r| r.t.name.eq_ignore_ascii_case(&t.name)) {
        let wrong: Vec<&str> = KEYS.iter().copied().filter(|k| r.t.get(k) != t.settled().get(k)).collect();
        if !wrong.is_empty() {
            let (a, b) = doc.sections().into_iter().filter(|(n, _, _)| n.eq_ignore_ascii_case(&t.name)).map(|(_, a, b)| (a, b)).last().unwrap_or((last_a, last_a + 1));
            let after = (a + 1..b).filter(|i| matches!(classify(&doc.lines[*i]), Line::Key { .. })).last().unwrap_or(a);
            for (n, k) in wrong.into_iter().enumerate() {
                doc.lines.insert(after + 1 + n, format!("{k} = {}", t.get(k)));
            }
        }
    }
}

// ---- waves (a mix per mode, for modes that read waves.ini) ----------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WaveRow {
    /// first wave this row is for
    pub from: u32,
    /// last wave (None = this one and every wave after)
    pub to: Option<u32>,
    /// (type, weight): how likely each enemy is of that type
    pub mix: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WaveSet {
    pub mode: String,
    /// the boss wave's type ("" = none)
    pub boss: String,
    /// a boss every n waves (0 = never)
    pub boss_every: u32,
    /// times the mode's own number of enemies
    pub count_scale: f32,
    /// times the mode's own health for each enemy
    pub hp_scale: f32,
    /// true: each enemy keeps its type's health (creatures.ini) instead of the mode's table
    pub type_hp: bool,
    pub rows: Vec<WaveRow>,
}

impl Default for WaveSet {
    fn default() -> Self {
        WaveSet { mode: String::new(), boss: String::new(), boss_every: 0, count_scale: 1.0, hp_scale: 1.0, type_hp: false, rows: vec![] }
    }
}

/// Last Stand's built-in mix (gm_laststand.sma pick_type): zombies, runners from wave 2,
/// brutes from 3, spitters from 4, a boss every 5th wave. Waves 10 and after as wave 10.
pub fn laststand_default() -> WaveSet {
    let mut rows = Vec::new();
    for n in 1..=10u32 {
        let mut mix = vec![("zombie".to_string(), 10)];
        if n >= 2 {
            mix.push(("runner".into(), 4 + n));
        }
        if n >= 3 {
            mix.push(("brute".into(), 1 + n / 3));
        }
        if n >= 4 {
            mix.push(("spitter".into(), 2));
        }
        rows.push(WaveRow { from: n, to: if n == 10 { None } else { Some(n) }, mix });
    }
    WaveSet { mode: "laststand".into(), boss: "boss".into(), boss_every: 5, count_scale: 1.0, hp_scale: 1.0, type_hp: false, rows }
}

/// A mode id: what modes.ini and the file's sections use.
pub fn mode_ok(m: &str) -> bool {
    !m.is_empty() && m.len() <= 24 && m.starts_with(|c: char| c.is_ascii_lowercase()) && m.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

fn wave_key(k: &str) -> Option<(u32, Option<u32>)> {
    let r = k.strip_prefix("wave")?.trim();
    if let Some(a) = r.strip_suffix('+') {
        return Some((a.trim().parse().ok()?, None));
    }
    if let Some((a, b)) = r.split_once('-') {
        return Some((a.trim().parse().ok()?, Some(b.trim().parse().ok()?)));
    }
    let n: u32 = r.parse().ok()?;
    Some((n, Some(n)))
}

/// The wave sets in a waves.ini (one per [mode] section).
pub fn parse_waves(text: &str) -> Vec<WaveSet> {
    let mut out: Vec<WaveSet> = Vec::new();
    for raw in text.lines() {
        match classify(raw) {
            Line::Section(m) => out.push(WaveSet { mode: m.to_ascii_lowercase(), ..Default::default() }),
            Line::Key { key, value, .. } => {
                let Some(w) = out.last_mut() else { continue };
                let k = key.to_ascii_lowercase();
                match k.as_str() {
                    "boss" => w.boss = value,
                    "boss_every" => w.boss_every = atoi(&value).clamp(0, 1000) as u32,
                    "count_scale" => w.count_scale = num(&value, 0.1, 10.0, 1.0),
                    "hp_scale" => w.hp_scale = num(&value, 0.1, 10.0, 1.0),
                    "type_hp" => w.type_hp = atoi(&value) != 0,
                    _ => {
                        if let Some((from, to)) = wave_key(&k) {
                            let toks: Vec<&str> = value.split_whitespace().collect();
                            let mix = toks.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].to_ascii_lowercase(), atoi(c[1]).clamp(0, 1000) as u32)).collect();
                            w.rows.push(WaveRow { from, to, mix });
                        }
                    }
                }
            }
            Line::Other => {}
        }
    }
    out
}

fn waves_section(w: &WaveSet) -> Vec<String> {
    let mut v = vec![format!("[{}]", w.mode)];
    v.push(format!("boss = {}", w.boss));
    v.push(format!("boss_every = {}", w.boss_every));
    v.push(format!("count_scale = {}", fmt_num(w.count_scale)));
    v.push(format!("hp_scale = {}", fmt_num(w.hp_scale)));
    v.push(format!("type_hp = {}", if w.type_hp { 1 } else { 0 }));
    let mut rows = w.rows.clone();
    rows.sort_by_key(|r| r.from);
    for r in rows {
        let key = match r.to {
            None => format!("wave {}+", r.from),
            Some(t) if t == r.from => format!("wave {}", r.from),
            Some(t) => format!("wave {}-{t}", r.from),
        };
        let mix: Vec<String> = r.mix.iter().filter(|(_, n)| *n > 0).map(|(t, n)| format!("{t} {n}")).collect();
        v.push(format!("{key} = {}", mix.join(" ")));
    }
    v
}

const WAVES_HEAD: &str = "; waves.ini: which enemies each wave brings, per game mode (made in Tools > Creature Studio).
; [mode]                    the mode's id, as in modes.ini
; boss = boss               the boss wave's creature type (empty = none)
; boss_every = 5            a boss every 5th wave (0 = never)
; count_scale = 1           times the mode's own number of enemies a wave
; hp_scale = 1              times the mode's own health for each enemy
; type_hp = 0               1 = each enemy has its type's health from creatures.ini (times hp_scale)
; wave 3 = zombie 10 runner 7     a wave's mix: type and weight (how likely), as many as you like
; wave 4-6 = ...            waves 4 to 6;  wave 10+ = ...  wave 10 and every one after
; A wave with no line of its own uses the closest line before it.";

/// Puts one mode's set into a waves.ini's text: its section replaced (or added), everything
/// else as it was.
pub fn write_waves(old: &str, w: &WaveSet) -> String {
    let mut doc = Doc::parse(old);
    if doc.lines.is_empty() {
        doc.lines = WAVES_HEAD.lines().map(String::from).collect();
        doc.final_nl = true;
    }
    let new = waves_section(w);
    let secs: Vec<(usize, usize)> = doc.sections().into_iter().filter(|(n, _, _)| n.eq_ignore_ascii_case(&w.mode)).map(|(_, a, b)| (a, b)).collect();
    // (a mode listed twice: the later copies go first, from the end, so the first's lines stay put)
    for (a, b) in secs.iter().skip(1).rev() {
        doc.lines.drain(*a..*b);
    }
    if let Some(&(a, b)) = secs.first() {
        // (notes and blank lines at the end of the section stay: they lead into what follows)
        let mut end = b;
        while end > a + 1 && !matches!(classify(&doc.lines[end - 1]), Line::Key { .. }) {
            end -= 1;
        }
        doc.lines.splice(a..end, new);
    } else {
        if doc.lines.last().is_some_and(|l| !l.trim().is_empty()) {
            doc.lines.push(String::new());
        }
        doc.lines.extend(new);
    }
    doc.text()
}

pub fn check_waves(w: &WaveSet, types: &[String]) -> Vec<Problem> {
    let mut out = Vec::new();
    let at = "waves";
    if !mode_ok(&w.mode) {
        out.push(problem(at, "mode", "A mode id is small letters, digits and _ (starting with a letter), 24 at most.", true));
    }
    let known = |t: &str| types.iter().any(|x| x.eq_ignore_ascii_case(t));
    if !w.boss.is_empty() && !known(&w.boss) {
        out.push(problem(at, "boss", format!("There's no creature type called {}.", w.boss), true));
    }
    if w.rows.is_empty() {
        out.push(problem(at, "rows", "Add at least one wave.", true));
    }
    if w.rows.len() > 100 {
        out.push(problem(at, "rows", "100 lines at most.", true));
    }
    let mut rows = w.rows.clone();
    rows.sort_by_key(|r| r.from);
    for (i, r) in rows.iter().enumerate() {
        let label = match r.to {
            None => format!("Wave {} and after", r.from),
            Some(t) if t == r.from => format!("Wave {}", r.from),
            Some(t) => format!("Waves {}-{t}", r.from),
        };
        if r.from == 0 || r.from > 1000 || r.to.is_some_and(|t| t < r.from || t > 1000) {
            out.push(problem(at, "rows", format!("{label}: waves are numbered from 1, and the last can't come before the first."), true));
        }
        if let Some(next) = rows.get(i + 1) {
            if r.to.is_none_or(|t| t >= next.from) {
                out.push(problem(at, "rows", format!("{label} overlaps the line for wave {}.", next.from), true));
            }
        }
        if r.mix.iter().all(|(_, n)| *n == 0) {
            out.push(problem(at, "rows", format!("{label}: give at least one type a weight."), true));
        }
        for (t, n) in &r.mix {
            if !known(t) && *n > 0 {
                out.push(problem(at, "rows", format!("{label}: there's no creature type called {t}."), true));
            }
            if *n > 1000 {
                out.push(problem(at, "rows", format!("{label}: weights go up to 1000."), true));
            }
        }
    }
    if rows.first().is_some_and(|r| r.from > 1) {
        out.push(problem(at, "rows", "There's no line for wave 1: the mode's own mix plays until the first line.", false));
    }
    if !(0.1..=10.0).contains(&w.count_scale) || !(0.1..=10.0).contains(&w.hp_scale) {
        out.push(problem(at, "count_scale", "Keep the scales between 0.1 and 10.", true));
    }
    out
}

// ---- models and sounds in the game's folders ------------------------------------------------

/// The folders the game reads, in its order (its own, its downloads, CS for CZ, Half-Life).
fn search_dirs(hl: &Path, game: &str) -> Vec<(String, PathBuf)> {
    let mut names = vec![game.to_string(), format!("{game}_downloads")];
    if game != "cstrike" {
        names.push("cstrike".into());
        names.push("cstrike_downloads".into());
    }
    names.push("valve".into());
    names.into_iter().map(|n| (n.clone(), hl.join(n))).filter(|(_, d)| d.is_dir()).collect()
}

fn imported_models(app: &Path) -> Vec<String> {
    let v: Value = std::fs::read(app.join("tools").join("imported_models.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    v["files"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.to_lowercase())).collect()
}

/// Who has a file: everyone with the game, most players (Half-Life's own), or only those who
/// download it from the server.
fn availability(folder: &str, rel: &str, imported: &[String]) -> (&'static str, &'static str) {
    if folder == "valve" {
        return ("hl", "Half-Life's own file: most players have it (Steam's shared files); anyone without it sees an error model. It's never offered for download.");
    }
    let low = rel.to_lowercase();
    let parts: Vec<&str> = low.split('/').collect();
    // the game's own: models/<file>.mdl, and its own player models; anything in another
    // folder under models/ came with a map or an add-on
    const STOCK_PLAYERS: &[&str] = &["arctic", "gign", "gsg9", "guerilla", "leet", "sas", "terror", "urban", "vip", "spetsnaz", "militia"];
    let stock_place = parts.len() == 2 || (parts.len() == 4 && parts[1] == "player" && STOCK_PLAYERS.contains(&parts[2]));
    if folder.ends_with("_downloads") || imported.contains(&format!("{folder}/{low}")) || !stock_place {
        return ("custom", "Not one of the game's own files: players get it from your server when they join (downloads on, or FastDL), or see an error model.");
    }
    ("game", "Comes with the game: everyone has it.")
}

/// Where a model or sound is (the first folder that has it), or None.
fn find_in(dirs: &[(String, PathBuf)], rel: &str) -> Option<(String, PathBuf)> {
    let f = crate::mapcopy::Finder::default();
    dirs.iter().find_map(|(n, d)| f.find(d, rel).map(|p| (n.clone(), p)))
}

/// Each model candidate of a type: where it is and who has it; and which one the game picks.
pub fn model_report(hl: &Path, game: &str, app: &Path, models: &str) -> Value {
    let dirs = search_dirs(hl, game);
    let imported = imported_models(app);
    let mut picked: Option<usize> = None;
    let list: Vec<Value> = models
        .split_whitespace()
        .enumerate()
        .map(|(i, m)| {
            if !safe_model(m) {
                return json!({"path": m, "found": false, "kind": "bad", "note": "Not a model path."});
            }
            match find_in(&dirs, m) {
                Some((folder, _)) => {
                    picked.get_or_insert(i);
                    let (kind, note) = availability(&folder, m, &imported);
                    json!({"path": m, "found": true, "folder": folder, "kind": kind, "note": note})
                }
                None => json!({"path": m, "found": false, "kind": "missing", "note": "Not in your game's folders."}),
            }
        })
        .collect();
    json!({"candidates": list, "picked": picked, "standIn": picked.is_some_and(|p| p > 0)})
}

/// Sounds: "a b | c" picks the first group with a file there; which group, and what's missing.
pub fn sound_report(hl: &Path, game: &str, list: &str) -> Value {
    let dirs = search_dirs(hl, game);
    let mut used: Option<usize> = None;
    let groups: Vec<Value> = list
        .split('|')
        .enumerate()
        .map(|(gi, g)| {
            let files: Vec<Value> = g
                .split_whitespace()
                .map(|f| {
                    let found = safe_sound(f) && find_in(&dirs, &format!("sound/{f}")).is_some();
                    if found && used.is_none() {
                        used = Some(gi);
                    }
                    json!({"file": f, "found": found})
                })
                .collect();
            json!({"files": files})
        })
        .collect();
    json!({"groups": groups, "used": used})
}

const MAX_LIST: usize = 4000;

/// Models a creature could use (no weapon models), with who has each.
pub fn list_models(hl: &Path, game: &str, app: &Path) -> Vec<Value> {
    let imported = imported_models(app);
    let mut out: Vec<Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (folder, dir) in search_dirs(hl, game) {
        let mut stack = vec![(dir.join("models"), "models".to_string(), 0)];
        while let Some((d, rel, depth)) = stack.pop() {
            let entries: Vec<(String, PathBuf, bool)> = std::fs::read_dir(&d)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path(), e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                .collect();
            let names: std::collections::HashSet<String> = entries.iter().filter(|e| !e.2).map(|e| e.0.to_lowercase()).collect();
            for (name, path, is_dir) in entries {
                if !crate::content::safe_part(&name) || name.contains(' ') {
                    continue;
                }
                if is_dir {
                    if depth < 3 {
                        stack.push((path, format!("{rel}/{name}"), depth + 1));
                    }
                    continue;
                }
                let low = name.to_lowercase();
                let Some(stem) = low.strip_suffix(".mdl") else { continue };
                // weapons, and the texture/animation parts of another model
                if ["v_", "p_", "w_"].iter().any(|p| stem.starts_with(p)) {
                    continue;
                }
                if stem.ends_with('t') && names.contains(&format!("{}.mdl", &stem[..stem.len() - 1])) {
                    continue;
                }
                if stem.len() > 2 && stem[stem.len() - 2..].bytes().all(|c| c.is_ascii_digit()) && names.contains(&format!("{}.mdl", &stem[..stem.len() - 2])) {
                    continue;
                }
                let r = format!("{rel}/{name}");
                if !safe_model(&r) || !seen.insert(r.to_lowercase()) {
                    continue; // the game uses the first folder's
                }
                let (kind, note) = availability(&folder, &r, &imported);
                let group = if r.to_lowercase().starts_with("models/player/") {
                    "Player models"
                } else if low.contains("hostage") || low.contains("scientist") {
                    "People"
                } else {
                    "Creatures and props"
                };
                out.push(json!({"path": r, "folder": folder, "kind": kind, "note": note, "group": group, "size": std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)}));
                if out.len() >= MAX_LIST {
                    return out;
                }
            }
        }
    }
    out.sort_by(|a, b| a["path"].as_str().unwrap_or("").to_lowercase().cmp(&b["path"].as_str().unwrap_or("").to_lowercase()));
    out
}

/// Sounds under sound/ (relative to it), the game's folders in order.
pub fn list_sounds(hl: &Path, game: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (folder, dir) in search_dirs(hl, game) {
        let mut stack = vec![(dir.join("sound"), String::new(), 0)];
        while let Some((d, rel, depth)) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if !crate::content::safe_part(&name) || name.contains(' ') {
                    continue;
                }
                let r = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if depth < 3 {
                        stack.push((e.path(), r, depth + 1));
                    }
                } else if safe_sound(&r) && seen.insert(r.to_lowercase()) {
                    out.push(json!({"file": r, "folder": folder}));
                    if out.len() >= MAX_LIST {
                        return out;
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a["file"].as_str().unwrap_or("").cmp(b["file"].as_str().unwrap_or("")));
    out
}

// GoldSrc activities (activity.h) the fork looks for, in its order
const ACTS: [[i32; 3]; 6] = [[1, 1, 1], [3, 4, 1], [4, 3, 1], [30, 31, 28], [26, 27, 71], [36, 38, 37]];
/// a ranged attacker spits: its range attack before any melee one
const RANGED_ACTS: [i32; 3] = [28, 29, 30];
const PLAYER_NAMES: [&str; 6] = ["idle1", "walk", "run", "ref_shoot_knife", "gutshot", "death1"];

/// The sequences in a model and which one the game would play for idle, walk, run, attack,
/// flinch and death (SetAnim: the type's name, else by activity, else player-model names).
pub fn sequences(path: &Path, anims: &[String; 6], ranged: bool) -> Result<Value, String> {
    let bytes = crate::goldsrc::read_capped(path, "model file", crate::goldsrc::mdl::MAX_MDL).map_err(|e| e.to_string())?;
    let m = crate::goldsrc::mdl::Model::parse(bytes).map_err(|e| e.to_string())?;
    let find_name = |n: &str| m.sequences.iter().position(|s| s.name.eq_ignore_ascii_case(n));
    let picks: Vec<Value> = (0..6)
        .map(|i| {
            let by_name = (!anims[i].is_empty()).then(|| find_name(&anims[i])).flatten();
            let (seq, how) = match by_name {
                Some(s) => (Some(s), "name"),
                None => match (if i == 3 && ranged { &RANGED_ACTS } else { &ACTS[i] }).iter().find_map(|a| m.sequences.iter().position(|s| s.activity == *a)) {
                    Some(s) => (Some(s), "activity"),
                    None => (find_name(PLAYER_NAMES[i]), "player"),
                },
            };
            json!({"seq": seq, "how": if seq.is_some() { how } else { "none" }, "missing": !anims[i].is_empty() && by_name.is_none()})
        })
        .collect();
    let seqs: Vec<Value> = m.sequences.iter().enumerate().map(|(i, s)| json!({"index": i, "name": s.name, "activity": s.activity, "fps": s.fps, "frames": s.frames})).collect();
    // (ResolveAnims: no run of its own when run comes out as the walk; the game plays the walk faster)
    let no_run = picks[2]["seq"].is_null() || picks[2]["seq"] == picks[1]["seq"];
    Ok(json!({"sequences": seqs, "picks": picks, "noRun": no_run}))
}

// ---- the files on disk ----------------------------------------------------------------------

#[derive(Clone)]
pub struct Files {
    pub hl: PathBuf,
    pub game: String,
    pub dir: PathBuf,
    pub app: PathBuf,
    pub label: String,
}

impl Files {
    fn ini(&self) -> PathBuf {
        self.dir.join(FILE_REL)
    }
    fn waves(&self) -> PathBuf {
        self.dir.join(WAVES_REL)
    }
}

fn read_text(p: &Path) -> Result<Option<String>, String> {
    match std::fs::metadata(p) {
        Err(_) => Ok(None),
        Ok(m) if m.len() > MAX_FILE => Err(format!("{} is too big to be a settings file.", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())),
        Ok(_) => crate::sys::read_latin1(p).map(Some).map_err(|e| format!("Couldn't read {}: {e}", p.display())),
    }
}

/// Writes a file whole, through the safety net (History, Undo).
fn put(path: &Path, text: &str, reason: &str) -> Result<(), String> {
    let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::safety::before(TOOL, reason, path);
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save {file}: {e}"))?;
    }
    crate::sys::make_writable(path);
    crate::sys::write_latin1_atomic(path, text).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Loner-Strike isn't allowed to change {file}."),
        _ => format!("Couldn't save {file}: {e}"),
    })
}

fn limits() -> Value {
    json!({
        "ranges": RANGES.iter().map(|(k, lo, hi)| json!({"key": k, "min": lo, "max": hi})).collect::<Vec<_>>(),
        "categories": CATEGORIES, "behaviours": BEHAVIOURS, "attacks": ATTACKS, "targets": TARGETS,
        "maxTypes": MAX_TYPES, "maxDisplay": MAX_DISPLAY, "maxName": MAX_NAME,
    })
}

pub fn status(f: &Files) -> Result<Value, String> {
    let text = read_text(&f.ini())?;
    let doc = Doc::parse(text.as_deref().unwrap_or(""));
    let types = resolve(&doc);
    let plain: Vec<Creature> = types.iter().filter(|r| !r.skipped).map(|r| r.t.clone()).collect();
    let mut problems = check(&plain);
    for r in types.iter().filter(|r| r.skipped) {
        problems.push(problem(&r.t.name, "", format!("{} is past the game's {MAX_TYPES} types: the game skips it.", r.t.name), false));
    }
    Ok(json!({
        "game": f.label,
        "file": FILE_REL,
        "exists": text.is_some(),
        "types": types.iter().map(|r| {
            let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
            v["summary"] = json!(summary(&r.t));
            v
        }).collect::<Vec<_>>(),
        "templates": builtins(),
        "problems": problems,
        "limits": limits(),
        "placed": placed(),
    }))
}

/// Creatures a mapper places in the map editor instead of a mode spawning them in waves: the
/// `ls_barnacle` entity and the types that don't move (turrets, placed as `ls_creature`), with
/// their map settings from lonerstrike.fgd. Flyers join here when the brain has them.
pub fn placed() -> Vec<Value> {
    let classes = crate::editor::fgd::parse(&crate::editor::fgd::shipped()).unwrap_or_default();
    let keys_of = |class: &str, skip: &[&str]| -> Vec<Value> {
        classes
            .iter()
            .find(|c| c.name == class)
            .map(|c| c.keys.iter().filter(|k| !skip.contains(&k.name.as_str())).map(|k| json!(k)).collect())
            .unwrap_or_default()
    };
    let mut out = vec![json!({
        "id": "ls_barnacle",
        "entity": "ls_barnacle",
        "type": "",
        "title": "Barnacle",
        "model": "models/barnacle.mdl",
        "about": "Hangs under a ceiling with its tongue down to the floor. A player who walks into the tongue is hauled up and bitten until someone shoots it. It comes back every round.",
        "how": "Map editor: Gameplay > Components > Creatures > Barnacle, or an ls_barnacle entity. Place it right under the ceiling.",
        "keys": keys_of("ls_barnacle", &[]),
    })];
    for t in builtins().into_iter().filter(|t| !t.moves) {
        out.push(json!({
            "id": format!("ls_creature:{}", t.name),
            "entity": "ls_creature",
            "type": t.name,
            "title": t.display,
            "model": t.model,
            "about": summary(&t),
            "how": format!("Map editor: Gameplay > Components > Creatures > Creature with Type \"{}\", or an ls_creature entity with type {}. Empty settings use the type's own (edit the type above).", t.display, t.name),
            "keys": keys_of("ls_creature", &["type", "behaviour", "path", "radius", "speed"]),
        }));
    }
    out
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SaveReq {
    pub types: Vec<Creature>,
}

/// Saves the types; Ok(false) when nothing changed.
pub fn save(f: &Files, req: &SaveReq) -> Result<bool, String> {
    let errors: Vec<Problem> = check(&req.types).into_iter().filter(|p| p.error).collect();
    if let Some(first) = errors.first() {
        let who = if first.at.is_empty() { String::new() } else { format!("{}: ", first.at) };
        return Err(if errors.len() == 1 { format!("Not saved: {who}{}", first.text) } else { format!("Not saved: {who}{} (and {} more to fix)", first.text, errors.len() - 1) });
    }
    let old = read_text(&f.ini())?.unwrap_or_default();
    let mut new = write(&old, &req.types)?;
    if old.is_empty() && !new.is_empty() {
        new = format!("; creatures.ini: creature types for the game (docs/CREATURES.md). Made in Tools > Creature Studio; edit freely.\n; A [type] adds a type or changes a built-in one (zombie runner brute spitter hound turret boss) by name.\n\n{new}");
    }
    if new == old {
        return Ok(false);
    }
    put(&f.ini(), &new, "Saved creature types")?;
    Ok(true)
}

pub fn waves_status(f: &Files) -> Result<Value, String> {
    let text = read_text(&f.waves())?;
    let sets = parse_waves(text.as_deref().unwrap_or(""));
    let types: Vec<String> = resolve(&Doc::parse(&read_text(&f.ini())?.unwrap_or_default())).into_iter().filter(|r| !r.skipped).map(|r| r.t.name).collect();
    Ok(json!({"file": WAVES_REL, "exists": text.is_some(), "sets": sets, "defaults": {"laststand": laststand_default()}, "types": types,
              "modes": [{"id": "laststand", "name": "Last Stand", "reads": true, "note": "Last Stand reads [laststand] at map start (types it names are loaded for that map)."}]}))
}

pub fn save_waves(f: &Files, w: &WaveSet) -> Result<bool, String> {
    let types: Vec<String> = resolve(&Doc::parse(&read_text(&f.ini())?.unwrap_or_default())).into_iter().filter(|r| !r.skipped).map(|r| r.t.name).collect();
    let errors: Vec<Problem> = check_waves(w, &types).into_iter().filter(|p| p.error).collect();
    if let Some(first) = errors.first() {
        return Err(format!("Not saved: {}", first.text));
    }
    let old = read_text(&f.waves())?.unwrap_or_default();
    let new = write_waves(&old, w);
    if new == old {
        return Ok(false);
    }
    put(&f.waves(), &new, &format!("Saved the waves for {}", w.mode))?;
    Ok(true)
}

// ---- live: the player's own running game ----------------------------------------------------

/// The console's answer with the game's padding gone.
fn reply_text(r: &str) -> String {
    r.replace('\0', "").lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

fn live_error(e: crate::rcon::RconError) -> String {
    match e {
        crate::rcon::RconError::Offline => "Your game isn't running a map. Start one (Play), then try again.".into(),
        crate::rcon::RconError::BadPassword => "The game didn't let Loner-Strike in (the console password changed). Restart the map and try again.".into(),
        crate::rcon::RconError::Refused => "That has characters the game's console can't take.".into(),
    }
}

fn no_creatures(reply: &str) -> bool {
    reply.contains("Unknown command") || reply.trim().is_empty()
}

const NO_FORK: &str = "The running game doesn't have creatures: they come with Loner-Strike's bot brain (its own game DLL). Turn the brain on, restart the map, and try again.";

/// `ls_creature reload`: the running game reads creatures.ini again.
pub fn reload(g: &crate::game::Game) -> Result<Value, String> {
    let r = reply_text(&g.rcon.send("ls_creature reload").map_err(live_error)?);
    if no_creatures(&r) {
        return Err(NO_FORK.into());
    }
    Ok(json!({"ok": true, "reply": r, "message": "The game read your creature types again. New models and sounds wait for the next map."}))
}

/// The map the game is on, from `status`.
fn current_map(status: &str) -> Option<String> {
    status.lines().find_map(|l| {
        let t = l.trim();
        let rest = t.strip_prefix("map")?.trim_start().strip_prefix(':')?;
        let name = rest.split_whitespace().next()?;
        crate::maps::safe_arg(name).then(|| name.to_string())
    })
}

/// `GM aim x y z [yaw]` from gamemodes.sma's gm_aim (where the host is looking), if it has it.
fn parse_aim(reply: &str) -> Option<([f32; 3], f32)> {
    reply.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("GM aim ")?;
        let n: Vec<f32> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if n.len() < 3 || n.iter().any(|v| !v.is_finite() || v.abs() > 8192.0) {
            return None;
        }
        Some(([n[0], n[1], n[2]], n.get(3).copied().unwrap_or(0.0)))
    })
}

/// A spawn on the map to put a test creature at (Terrorist spawns first: the mode's
/// enemies' side), as (where, facing).
fn spawn_point(g: &crate::game::Game, map: &str) -> Option<([f32; 3], f32)> {
    let m = crate::tools::mapinfo::locate(g, map)?;
    let lite = crate::tools::mapinfo::read_lite(&m.bsp, false).ok()?;
    for class in ["info_player_deathmatch", "info_player_start"] {
        if let Some(e) = lite.entities.iter().find(|e| e.classname() == class) {
            let o = e.origin()?;
            return Some((o, e.angles().map(|a| a[1]).unwrap_or(0.0)));
        }
    }
    None
}

#[derive(Debug, Deserialize)]
pub struct SpawnReq {
    #[serde(rename = "type")]
    pub kind: String,
    /// spawn it standing still (behaviour idle) to look at it
    #[serde(default)]
    pub calm: bool,
}

/// `ls_creature spawn <type> <x y z> [yaw]`: where the player looks if the game can say,
/// else at a spawn.
pub fn spawn(g: &crate::game::Game, req: &SpawnReq) -> Result<Value, String> {
    if name_problem(&req.kind).is_some() {
        return Err("That isn't a creature type's name.".into());
    }
    let aim = g.rcon.send("gm_aim").ok().and_then(|r| parse_aim(&r));
    let (at, yaw, where_) = match aim {
        Some((o, yaw)) => (o, yaw + 180.0, "where you're looking"),
        None => {
            let st = g.rcon.send("status").map_err(live_error)?;
            let map = current_map(&st).ok_or("Couldn't tell which map the game is on.")?;
            let (o, yaw) = spawn_point(g, &map).ok_or_else(|| format!("Couldn't find a spawn on {map} to put it at."))?;
            (o, yaw, "at a Terrorist spawn")
        }
    };
    let mut line = format!("ls_creature spawn {} {} {} {} {}", req.kind, fmt_num(at[0]), fmt_num(at[1]), fmt_num(at[2]), fmt_num(yaw.rem_euclid(360.0)));
    if req.calm {
        line.push_str(" behaviour=idle");
    }
    let r = reply_text(&g.rcon.send(&line).map_err(live_error)?);
    if no_creatures(&r) {
        return Err(NO_FORK.into());
    }
    if r.contains("isn't precached") || r.contains("not precached") {
        return Err(format!("{} wasn't loaded with this map, so it can't come in now. Types a map can use are loaded as it starts: set ls_creature_precache to all (or name the type) and change map.", req.kind));
    }
    if r.contains("unknown type") {
        return Err(format!("The game doesn't know {} yet: save, then Apply to game first.", req.kind));
    }
    if !r.contains("spawned") {
        return Err(if r.is_empty() { "The game didn't say whether it worked.".into() } else { format!("The game said: {r}") });
    }
    Ok(json!({"ok": true, "reply": r, "at": at, "message": format!("A {} is in the game {where_}.", req.kind)}))
}

/// What the running game has: its types (and stand-ins) and the creatures alive.
pub fn live(g: &crate::game::Game) -> Value {
    match g.rcon.send("ls_creature types") {
        Err(e) => json!({"running": false, "message": live_error(e)}),
        Ok(r) => {
            let r = reply_text(&r);
            if no_creatures(&r) {
                return json!({"running": true, "creatures": false, "message": NO_FORK});
            }
            let types: Vec<Value> = r
                .lines()
                .filter_map(|l| {
                    let name = l.split_whitespace().next()?;
                    (name_problem(name).is_none() || builtin(name).is_some()).then(|| json!({"name": name, "precached": !l.contains("(not precached)"), "standIn": l.contains("(stand-in)")}))
                })
                .collect();
            let list = g.rcon.send("ls_creature list").map(|x| reply_text(&x)).unwrap_or_default();
            let alive = list.lines().filter(|l| l.contains(" alive ")).count();
            json!({"running": true, "creatures": true, "types": types, "alive": alive})
        }
    }
}

// ---- routes ---------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into())))
}

fn target(st: &Shared) -> Result<(Arc<crate::game::Game>, Files), String> {
    let g = st.local_game().ok_or(NO_GAME)?;
    if g.remote.is_some() {
        return Err("This server is on another computer: edit its creatures there.".into());
    }
    let label = if g.dedicated { format!("the server on port {}", g.port) } else if g.name == "czero" { "Condition Zero".into() } else { "Counter-Strike 1.6".into() };
    let f = Files { hl: g.hl.clone(), game: g.name.clone(), dir: g.dir.clone(), app: g.app.clone(), label };
    Ok((g, f))
}

async fn api_status(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((_, f)) => blocking(move || status(&f)).await,
        Err(e) => answer(Err(e)),
    }
}

#[derive(Deserialize)]
struct CheckReq {
    types: Vec<Creature>,
    /// the type to report models and sounds for
    #[serde(default)]
    look: Option<String>,
}

async fn api_check(State(st): State<Shared>, h: HeaderMap, Json(r): Json<CheckReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let f = target(&st).ok().map(|(_, f)| f);
    blocking(move || {
        let types: Vec<Creature> = r.types.into_iter().take(MAX_TYPES * 2).collect();
        let mut out = json!({"problems": check(&types), "summaries": types.iter().map(|t| json!({"name": t.name, "summary": summary(t)})).collect::<Vec<_>>()});
        if let (Some(f), Some(t)) = (f, r.look.and_then(|n| types.iter().find(|t| t.name == n).cloned())) {
            out["models"] = model_report(&f.hl, &f.game, &f.app, &t.model);
            out["sounds"] = json!(t.snd.iter().map(|s| sound_report(&f.hl, &f.game, s)).collect::<Vec<_>>());
        }
        Ok(out)
    })
    .await
}

async fn api_save(State(st): State<Shared>, h: HeaderMap, Json(r): Json<SaveReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (_, f) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let changed = save(&f, &r)?;
        Ok(json!({"ok": true, "changed": changed, "message": if changed { "Saved. Apply to game to use it in the running map." } else { "Nothing changed." }, "status": status(&f)?}))
    })
    .await
}

async fn api_models(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((_, f)) => blocking(move || Ok(json!({"models": list_models(&f.hl, &f.game, &f.app)}))).await,
        Err(e) => answer(Err(e)),
    }
}

#[derive(Deserialize)]
struct ModelQ {
    /// the type's candidates ("models/zombie.mdl models/hostage.mdl")
    models: String,
    /// the type's anim_* names, comma separated (idle,walk,run,attack,flinch,die)
    #[serde(default)]
    anims: String,
    /// a ranged attacker (its attack sequence is picked differently)
    #[serde(default)]
    ranged: bool,
}

async fn api_model(State(st): State<Shared>, Query(q): Query<ModelQ>) -> Response {
    let (_, f) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        if q.models.len() > 400 {
            return Err("Too many models.".into());
        }
        let report = model_report(&f.hl, &f.game, &f.app, &q.models);
        let mut anims: [String; 6] = Default::default();
        for (i, a) in q.anims.split(',').take(6).enumerate() {
            anims[i] = cut(a.trim(), MAX_ANIM);
        }
        let picked = report["picked"].as_u64().map(|i| i as usize);
        let picked_path = picked.map(|i| report["candidates"][i]["path"].as_str().unwrap_or("").to_string());
        let mut out = json!({"report": report});
        if let Some(path) = picked_path {
            if let Some((folder, file)) = find_in(&search_dirs(&f.hl, &f.game), &path) {
                out["model"] = json!({"folder": folder, "path": path});
                match sequences(&file, &anims, q.ranged) {
                    Ok(s) => {
                        out["sequences"] = s["sequences"].clone();
                        out["picks"] = s["picks"].clone();
                        out["noRun"] = s["noRun"].clone();
                    }
                    Err(e) => out["modelError"] = json!(e),
                }
            }
        }
        Ok(out)
    })
    .await
}

async fn api_sounds(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((_, f)) => blocking(move || Ok(json!({"sounds": list_sounds(&f.hl, &f.game)}))).await,
        Err(e) => answer(Err(e)),
    }
}

async fn api_live(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((g, _)) => blocking(move || Ok(live(&g))).await,
        Err(e) => answer(Err(e)),
    }
}

async fn api_reload(State(st): State<Shared>, h: HeaderMap) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    match target(&st) {
        Ok((g, _)) => blocking(move || reload(&g)).await,
        Err(e) => answer(Err(e)),
    }
}

async fn api_spawn(State(st): State<Shared>, h: HeaderMap, Json(r): Json<SpawnReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    match target(&st) {
        Ok((g, _)) => blocking(move || spawn(&g, &r)).await,
        Err(e) => answer(Err(e)),
    }
}

async fn api_waves(State(st): State<Shared>) -> Response {
    match target(&st) {
        Ok((_, f)) => blocking(move || waves_status(&f)).await,
        Err(e) => answer(Err(e)),
    }
}

async fn api_waves_save(State(st): State<Shared>, h: HeaderMap, Json(w): Json<WaveSet>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (_, f) = match target(&st) {
        Ok(x) => x,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let changed = save_waves(&f, &w)?;
        Ok(json!({"ok": true, "changed": changed, "message": if changed { format!("Saved the waves for {}.", w.mode) } else { "Nothing changed.".into() }, "status": waves_status(&f)?}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/creatures", get(api_status))
        .route("/api/tools/creatures/check", post(api_check))
        .route("/api/tools/creatures/save", post(api_save))
        .route("/api/tools/creatures/models", get(api_models))
        .route("/api/tools/creatures/model", get(api_model))
        .route("/api/tools/creatures/sounds", get(api_sounds))
        .route("/api/tools/creatures/live", get(api_live))
        .route("/api/tools/creatures/reload", post(api_reload))
        .route("/api/tools/creatures/spawn", post(api_spawn))
        .route("/api/tools/creatures/waves", get(api_waves))
        .route("/api/tools/creatures/waves/save", post(api_waves_save))
}

#[cfg(test)]
mod tests {
    #[test]
    fn placed_lists_the_barnacle_and_turrets_with_their_map_keys() {
        let p = super::placed();
        let barnacle = p.iter().find(|x| x["entity"] == "ls_barnacle").expect("barnacle");
        let keys: Vec<&str> = barnacle["keys"].as_array().unwrap().iter().map(|k| k["name"].as_str().unwrap()).collect();
        for k in ["health", "damage", "length", "speed", "hates"] {
            assert!(keys.contains(&k), "barnacle key {k}");
        }
        assert!(p.iter().any(|x| x["type"] == "miniturret"));
        assert!(p.iter().filter(|x| x["entity"] == "ls_creature").all(|x| !x["keys"].as_array().unwrap().is_empty()));
    }

    use super::*;

    /// The example creatures.ini in docs/CREATURES.md §2 (the brain's own documentation).
    fn doc_example() -> String {
        let md = include_str!("../../../../docs/CREATURES.md");
        let start = md.find("```ini\n").unwrap() + 7;
        let end = start + md[start..].find("```").unwrap();
        md[start..end].to_string()
    }

    fn fixture() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools/testdata/creatures.ini")).unwrap()
    }

    fn types_of(text: &str) -> Vec<Creature> {
        resolve(&Doc::parse(text)).into_iter().filter(|r| !r.skipped).map(|r| r.t).collect()
    }

    #[test]
    fn files_round_trip_untouched() {
        for text in [doc_example(), fixture(), String::new(), "; only a note\n".into(), "[zombie]\nhp=5".into()] {
            let types = types_of(&text);
            assert_eq!(write(&text, &types).unwrap(), text, "{text}");
            let crlf = text.replace('\n', "\r\n");
            assert_eq!(write(&crlf, &types_of(&crlf)).unwrap(), crlf);
        }
    }

    #[test]
    fn reads_like_the_fork() {
        let t = types_of(&doc_example());
        let runner = t.iter().find(|x| x.name == "runner").unwrap();
        // base = zombie, then the keys; `ls =` comes after model = and wins
        assert_eq!(runner.hp, 60.0);
        assert_eq!(runner.speed, 300.0);
        assert_eq!(runner.model, "models/player/zombie_source/zombie_source.mdl");
        assert_eq!(runner.anim, ["".to_string(), "walk".into(), "run".into(), "attack1".into(), "".into(), "die".into()]);
        assert_eq!(runner.snd[0], "zombie/zo_idle1.wav zombie/zo_idle2.wav | player/pl_pain2.wav");
        assert_eq!(runner.fallback_glow, [160.0, 160.0, 20.0]);
        assert_eq!(runner.glow, [0.0; 3]);
        assert_eq!(t.len(), 16);
        // clamps, aliases, comments, a bad hull, walk over run, base copies, numbers for names
        let t = types_of("[a]\nhealth = 500000 ; huge\ndetect = -5\nfov = 1\nwalk_speed = 900\nspeed = 100\nhull_min = 0 0 10\nname = Big // shown\ncategory = 3\nattack = ranged\nhates = terrorist\nmoves = 0\nmoney = 99999\n[b]\nbase = a\nhp = 7\n");
        let a = t.iter().find(|x| x.name == "a").unwrap();
        assert_eq!((a.hp, a.detect_radius, a.detect_fov, a.walk_speed), (100000.0, 0.0, 10.0, 100.0));
        assert_eq!(a.hull_min, [-16.0, -16.0, 0.0]);
        assert_eq!((a.display.as_str(), a.category.as_str(), a.attack.as_str(), a.hates.as_str(), a.moves, a.money), ("Big", "mechanical", "ranged", "t", false, 16000));
        let b = t.iter().find(|x| x.name == "b").unwrap();
        assert_eq!((b.hp, b.display.as_str(), b.base.as_str()), (7.0, "Big", "a"));
        // past 32 types: skipped
        let many: String = (0..MAX_TYPES).map(|i| format!("[t{i}]\n")).collect();
        let r = resolve(&Doc::parse(&many));
        assert_eq!(r.iter().filter(|x| !x.skipped).count(), MAX_TYPES);
        assert_eq!(r.iter().filter(|x| x.skipped).count(), builtins().len());
    }

    #[test]
    fn edits_change_only_what_changed() {
        let text = fixture();
        let mut types = types_of(&text);
        let r = types.iter_mut().find(|x| x.name == "runner").unwrap();
        r.hp = 75.0;
        let new = write(&text, &types).unwrap();
        let changed: Vec<(&str, &str)> = text.lines().zip(new.lines()).filter(|(a, b)| a != b).collect();
        assert_eq!(new.lines().count(), text.lines().count(), "{new}");
        assert_eq!(changed.len(), 1, "{changed:?}");
        assert!(changed[0].1.starts_with("hp = 75") && changed[0].1.contains("; a bit tougher"), "{changed:?}");
        assert_eq!(types_of(&new).iter().find(|x| x.name == "runner").unwrap().hp, 75.0);
        // a key the section doesn't have goes after its last key; aliases are edited in place
        types.iter_mut().find(|x| x.name == "runner").unwrap().behaviour = "wander".into();
        types.iter_mut().find(|x| x.name == "my_hound").unwrap().behaviour = "guard".into();
        let new = write(&text, &types).unwrap();
        assert!(new.contains("mm = monster_zombie ; the kit's MonsterMod mapping\nbehaviour = wander\n# a note in the middle"), "{new}");
        assert!(new.contains("behavior = guard\n"));
        assert_eq!(new.lines().count(), text.lines().count() + 1);
        // a new type from a template: only what differs
        let mut types = types_of(&text);
        let mut fast = builtin("zombie").unwrap();
        fast.name = "fast_zombie".into();
        fast.base = "zombie".into();
        fast.speed = 320.0;
        types.push(fast);
        let new = write(&text, &types).unwrap();
        assert!(new.ends_with("\n[fast_zombie]\nbase = zombie\nspeed = 320\n"), "{new}");
        // taking a section out: a built-in goes back to how the game has it
        let types: Vec<Creature> = types_of(&text).into_iter().filter(|t| t.name != "spitter_big").collect();
        let new = write(&text, &types).unwrap();
        assert!(!new.contains("[spitter_big]"));
        assert!(new.contains("; the end"));
        // a later sounds = line would undo our snd_idle: set again after it
        let t = "[zombie]\nsnd_idle = a.wav\nsounds = b.wav\n";
        let mut types = types_of(t);
        types[0].snd[1] = "zombie/x.wav".into();
        let new = write(t, &types).unwrap();
        assert_eq!(types_of(&new)[0].snd[1], "zombie/x.wav");
        // CRLF stays CRLF on edit
        let crlf = text.replace('\n', "\r\n");
        let mut types = types_of(&crlf);
        types.iter_mut().find(|x| x.name == "brute").unwrap().damage = 45.0;
        let new = write(&crlf, &types).unwrap();
        assert!(!new.replace("\r\n", "").contains('\n'));
        assert!(new.contains("damage = 45\r\n"));
    }

    #[test]
    fn checks_hold_the_brains_limits() {
        let mut t = builtin("zombie").unwrap();
        t.name = "my_zombie".into();
        assert!(check(std::slice::from_ref(&t)).iter().all(|p| !p.error), "{:?}", check(&[t.clone()]));
        let err = |t: &Creature, field: &str| check(std::slice::from_ref(t)).into_iter().any(|p| p.error && p.field == field);
        for (k, v) in [("hp", 0.0), ("hp", 100001.0), ("speed", 2001.0), ("damage", 10001.0), ("detect_radius", 9000.0), ("attack_rate", 0.0), ("scale", 5.0), ("flee_hp", 1.5)] {
            let mut x = t.clone();
            match k {
                "hp" => x.hp = v,
                "speed" => x.speed = v,
                "damage" => x.damage = v,
                "detect_radius" => x.detect_radius = v,
                "attack_rate" => x.attack_rate = v,
                "scale" => x.scale = v,
                _ => x.flee_hp = v,
            }
            assert!(err(&x, k), "{k} {v}");
        }
        let mut x = t.clone();
        x.damage = 20000.0;
        let p = check(&[x]).into_iter().find(|p| p.field == "damage").unwrap();
        assert!(p.text.contains("10000"), "{}", p.text);
        let mut x = t.clone();
        x.hull_min = [-16.0, -16.0, 5.0];
        assert!(err(&x, "hull_min"));
        x.hull_min = [-200.0, -16.0, 0.0];
        x.hull_max = [200.0, 16.0, 72.0];
        assert!(err(&x, "hull_min"));
        for bad in ["", "All", "all", "123", "a b", "x;y", &"a".repeat(32)] {
            let mut x = t.clone();
            x.name = bad.into();
            assert!(err(&x, "name"), "{bad}");
        }
        let mut x = t.clone();
        x.display = "Nice ; name".into();
        assert!(err(&x, "display"));
        let mut x = t.clone();
        x.model = "../../liblist.gam".into();
        assert!(err(&x, "model"));
        x.model = "models/../x.mdl".into();
        assert!(err(&x, "model"));
        let mut x = t.clone();
        x.snd[0] = "../x.wav".into();
        assert!(err(&x, "snd_idle"));
        let mut x = t.clone();
        x.base = "later".into();
        let mut later = t.clone();
        later.name = "later".into();
        assert!(check(&[x.clone(), later.clone()]).iter().any(|p| p.error && p.field == "base"));
        assert!(!check(&[later, x]).iter().any(|p| p.error && p.field == "base"));
        // two of a name; too many types
        assert!(check(&[t.clone(), t.clone()]).iter().any(|p| p.error && p.field == "name"));
        let many: Vec<Creature> = (0..MAX_TYPES + 1).map(|i| Creature { name: format!("t{i}"), ..t.clone() }).collect();
        assert!(check(&many).iter().any(|p| p.error && p.at.is_empty()));
        // notes, not errors
        let mut x = t.clone();
        x.walk_speed = 500.0;
        assert!(check(&[x]).iter().any(|p| !p.error && p.field == "walk_speed"));
        // a save is refused with errors, before anything is written
        let dir = std::env::temp_dir().join(format!("ls-creatures-check-{}", std::process::id()));
        let f = Files { hl: dir.clone(), game: "cstrike".into(), dir: dir.join("cstrike"), app: dir.join("app"), label: "test".into() };
        let mut x = t.clone();
        x.damage = -1.0;
        assert!(save(&f, &SaveReq { types: vec![x] }).unwrap_err().starts_with("Not saved"));
        assert!(!f.ini().exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn safe_paths() {
        for ok in ["models/zombie.mdl", "models/player/gign/gign.mdl", "models/hostage.MDL"] {
            assert!(safe_model(ok), "{ok}");
        }
        for bad in ["zombie.mdl", "models/../x.mdl", "models//x.mdl", "/models/x.mdl", "models/x.wav", "models/a b.mdl", "models\\x.mdl", "models/.x.mdl", "c:/models/x.mdl"] {
            assert!(!safe_model(bad), "{bad}");
        }
        for ok in ["zombie/zo_idle1.wav", "x.wav"] {
            assert!(safe_sound(ok), "{ok}");
        }
        for bad in ["../x.wav", "a/../../x.wav", "/x.wav", "x.mp3", "a b.wav", ""] {
            assert!(!safe_sound(bad), "{bad}");
        }
        for ok in ["laststand", "my_mode2"] {
            assert!(mode_ok(ok));
        }
        for bad in ["", "Last", "../x", "2mode", "a-b", &"a".repeat(25)] {
            assert!(!mode_ok(bad), "{bad}");
        }
        // parse_aim and current_map never take junk
        assert_eq!(parse_aim("GM aim 1 2 3 90"), Some(([1.0, 2.0, 3.0], 90.0)));
        assert_eq!(parse_aim("GM aim 1 2 99999"), None);
        assert_eq!(parse_aim("Unknown command: gm_aim"), None);
        assert_eq!(current_map("hostname: x\nmap     :  de_dust2 at: 0 x, 0 y, 0 z\n").as_deref(), Some("de_dust2"));
        assert_eq!(current_map("map : ../../x at"), None);
    }

    #[test]
    fn saves_through_the_safety_net_in_the_game_folder() {
        let dir = std::env::temp_dir().join(format!("ls-creatures-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = Files { hl: dir.clone(), game: "cstrike".into(), dir: dir.join("cstrike"), app: dir.join("app"), label: "test".into() };
        let mut types = builtins();
        assert!(!save(&f, &SaveReq { types: types.clone() }).unwrap(), "built-ins as they are: nothing to write");
        assert!(!f.ini().exists());
        types[1].hp = 61.0;
        assert!(save(&f, &SaveReq { types: types.clone() }).unwrap());
        let text = std::fs::read_to_string(f.ini()).unwrap();
        assert!(text.starts_with("; creatures.ini"));
        assert!(text.contains("\n\n[runner]\nhp = 61\n"), "{text}");
        assert!(!save(&f, &SaveReq { types }).unwrap());
        let st = status(&f).unwrap();
        assert_eq!(st["types"].as_array().unwrap().len(), 16);
        assert_eq!(st["types"][1]["inFile"], true);
        assert_eq!(st["types"][1]["set"], json!(["hp"]));
        // waves next to it
        let w = laststand_default();
        assert!(save_waves(&f, &w).unwrap());
        assert_eq!(parse_waves(&std::fs::read_to_string(f.waves()).unwrap()), vec![w.clone()]);
        let mut bad = w.clone();
        bad.rows[0].mix = vec![("dragon".into(), 5)];
        assert!(save_waves(&f, &bad).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn waves_round_trip_and_match_last_stand() {
        let w = laststand_default();
        // the plugin's pick_type: wave 2 has runner 6, wave 9 brute 4, wave 10+ the same as 10
        assert_eq!(w.rows[1].mix, vec![("zombie".to_string(), 10), ("runner".into(), 6)]);
        assert_eq!(w.rows[8].mix[2], ("brute".to_string(), 4));
        assert_eq!(w.rows[9].to, None);
        let text = write_waves("", &w);
        assert!(text.starts_with("; waves.ini"));
        assert!(text.contains("wave 10+ = zombie 10 runner 14 brute 4 spitter 2"));
        assert_eq!(parse_waves(&text), vec![w.clone()]);
        // another mode's section and notes stay; this one is replaced
        let other = format!("{text}\n; mine\n[othermode]\nboss =\nwave 1 = zombie 1\n");
        let mut w2 = w.clone();
        w2.boss_every = 0;
        let out = write_waves(&other, &w2);
        assert!(out.contains("; mine\n[othermode]\nboss =\nwave 1 = zombie 1\n"));
        assert_eq!(parse_waves(&out)[0].boss_every, 0);
        assert_eq!(write_waves(&out, &w2), out);
        let types: Vec<String> = builtins().into_iter().map(|t| t.name).collect();
        assert!(check_waves(&w, &types).iter().all(|p| !p.error));
        let mut bad = w.clone();
        bad.rows[3].to = Some(7);
        assert!(check_waves(&bad, &types).iter().any(|p| p.error && p.text.contains("overlaps")));
    }

    #[test]
    fn summaries_read_plainly() {
        let s = summary(&builtin("turret").unwrap());
        assert!(s.starts_with("A") && s.contains("turret") && s.contains("shoots"), "{s}");
        let s = summary(&builtin("hound").unwrap());
        assert!(s.contains("guards its spot") && s.contains("35%"), "{s}");
    }

    #[test]
    #[ignore]
    fn sequences_of_real_models() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let p = Path::new(&hl).join("valve/models/zombie.mdl");
        if !p.exists() {
            return;
        }
        let v = sequences(&p, &Default::default(), false).unwrap();
        println!("{v:#}");
        assert!(v["picks"][1]["seq"].is_u64());
    }
}
