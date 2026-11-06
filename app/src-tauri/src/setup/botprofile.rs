//! Bot personalities: the original CS 1.6 bot profile (installer/botprofile.py)
//! and the bot style files made from a profile (panel/botstyles.py).
//! Both use Python's random with fixed seeds, so the output matches the Python
//! tools byte for byte.

use super::pyrandom::PyRandom;
use super::splitlines;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

// ---- the CS 1.6 bot profile ----------------------------------------------------

const WEAPONS: &[(&str, &[&str])] = &[
    ("Rifle", &["m4a1", "ak47", "famas", "galil", "mp5"]),
    ("RifleT", &["ak47", "m4a1", "galil", "famas", "mp5"]),
    ("Scoped", &["aug", "sg552", "famas", "galil", "mp5"]),
    ("Sniper", &["awp", "scout", "sg550", "g3sg1", "famas", "galil"]),
    ("Heavy", &["m249", "xm1014", "m3", "famas", "galil"]),
    ("Shotgun", &["xm1014", "m3", "famas", "galil", "mp5"]),
    ("Spray", &["p90", "mp5", "tmp", "mac10", "ump45"]),
];
// name: (skill, aggression, reaction time, buy cost tier, difficulty)
const TIERS: &[(&str, i64, i64, f64, i64, &str)] = &[
    ("Ace", 95, 90, 0.20, 4, "EXPERT"),
    ("Skilled", 80, 75, 0.25, 3, "HARD+EXPERT"),
    ("Solid", 65, 60, 0.30, 2, "NORMAL+HARD"),
    ("Casual", 45, 50, 0.40, 2, "EASY+NORMAL"),
    ("Rookie", 25, 40, 0.50, 1, "EASY"),
];
const HANDLES: &str = "Ash Axel Bandit Bishop Blaze Bolt Boone Brick Bronco Buck Bullet Cash Chase Chief Cinder Clutch Cobra
Colt Comet Cosmo Crash Crow Dash Delta Diesel Dingo Dodge Drake Duke Echo Ember Falcon Fang Finch Flint Fox Frost
Fury Ghost Gizmo Goose Grim Gunner Hawk Havoc Hex Hornet Hunter Iron Jinx Jolt Kilo Knox Lance Lobo Lynx Mako Mamba
Maverick Moose Nitro Nova Onyx Orbit Ozzy Patch Pike Pixel Puma Quill Radar Rafter Raven Razor Rebel Rex Rhino Ridge
Rocket Rogue Ronin Rook Rusty Sabre Scout Shade Shark Skid Slate Smoke Snipe Spike Spud Static Storm Stray Tank Thorn
Torch Tracer Trigger Tusk Viper Volt Wolf Wren Yeti Zed Ziggy Zulu";

/// An original bot profile (BotProfile.db) for Counter-Strike 1.6, which has the
/// same bots as Condition Zero but doesn't ship a profile file.
pub fn generate() -> String {
    let mut rnd = PyRandom::new(2004);
    let mut out: Vec<String> = [
        "// BotProfile.db written by Loner-Strike (original content, GPL-3.0).",
        "// Bot personalities for Counter-Strike 1.6. Edit freely.",
        "",
        "Default",
        "\tSkill = 50", "\tAggression = 50", "\tReactionTime = 0.3", "\tAttackDelay = 0", "\tTeamwork = 75",
        "\tWeaponPreference = none", "\tCost = 0", "\tDifficulty = NORMAL", "\tVoicePitch = 100", "\tSkin = 0",
        "End", "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for (name, guns) in WEAPONS {
        out.push(format!("Template {name}"));
        out.extend(guns.iter().map(|g| format!("\tWeaponPreference = {g}")));
        out.extend(["End".to_string(), String::new()]);
    }
    for (name, skill, aggr, react, cost, diff) in TIERS {
        out.extend([
            format!("Template {name}"),
            format!("\tSkill = {skill}"),
            format!("\tAggression = {aggr}"),
            format!("\tReactionTime = {react:.2}"),
            format!("\tCost = {cost}"),
            format!("\tDifficulty = {diff}"),
            "End".into(),
            String::new(),
        ]);
    }
    let tier_weights = [1.0, 2.0, 3.0, 3.0, 2.0];
    let gun_weights = [5.0, 4.0, 2.0, 2.0, 1.0, 1.0, 2.0];
    let pitches = [85, 90, 95, 100, 105, 110];
    for handle in HANDLES.split_whitespace() {
        let tier = TIERS[rnd.choices_weighted(&tier_weights)];
        let guns = WEAPONS[rnd.choices_weighted(&gun_weights)].0;
        let aggression = (tier.2 + rnd.randint(-20, 20)).clamp(0, 100);
        let teamwork = rnd.randint(40, 100);
        let pitch = pitches[rnd.choice(pitches.len())];
        out.extend([
            format!("{}+{} {}", tier.0, guns, handle),
            format!("\tAggression = {aggression}"),
            format!("\tTeamwork = {teamwork}"),
            format!("\tVoicePitch = {pitch}"),
            "End".into(),
            String::new(),
        ]);
    }
    out.join("\r\n") + "\r\n"
}

// ---- bot styles ------------------------------------------------------------------

/// (name, aggression lo/hi, teamwork lo/hi, skill scale; None: every bot at Skill 100)
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub name: String,
    pub aggression: (f64, f64),
    pub teamwork: (f64, f64),
    pub skill_scale: Option<f64>,
}

/// The styles as panel/botstyles.py defines them (used when that file isn't shipped).
pub fn builtin_styles() -> Vec<Style> {
    let s = |name: &str, a: (f64, f64), t: (f64, f64), k: Option<f64>| Style { name: name.into(), aggression: a, teamwork: t, skill_scale: k };
    vec![
        s("Pub", (55.0, 100.0), (20.0, 80.0), Some(1.0)),
        s("Matchmaking", (50.0, 90.0), (75.0, 100.0), Some(1.0)),
        s("Veterans", (60.0, 95.0), (70.0, 100.0), Some(1.0)),
        s("Newbie", (45.0, 85.0), (30.0, 70.0), Some(0.6)),
        s("LockedIn", (65.0, 100.0), (80.0, 100.0), Some(1.0)),
        s("Hackers", (80.0, 100.0), (50.0, 100.0), Some(1.0)),
        s("Gods", (100.0, 100.0), (100.0, 100.0), None),
    ]
}

/// The STYLES table read from a shipped botstyles.py, in its order.
pub fn parse_styles(py: &str) -> Vec<Style> {
    let re = Regex::new(
        r#""(\w+)"\s*:\s*\(\s*\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)\s*,\s*\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)\s*,\s*(None|[0-9.]+)\s*\)"#,
    )
    .unwrap();
    let Some(start) = py.find("STYLES = {") else { return vec![] };
    let body = &py[start..];
    let body = &body[..body.find("\n}").unwrap_or(body.len())];
    re.captures_iter(body)
        .filter_map(|c| {
            let f = |i: usize| c[i].parse::<f64>().ok();
            Some(Style {
                name: c[1].to_string(),
                aggression: (f(2)?, f(3)?),
                teamwork: (f(4)?, f(5)?),
                skill_scale: if &c[6] == "None" { None } else { Some(f(6)?) },
            })
        })
        .collect()
}

/// The styles from <resources>/panel/botstyles.py, or the built-in copy.
pub fn styles(res: &Path) -> Vec<Style> {
    let parsed = std::fs::read_to_string(res.join("panel/botstyles.py")).map(|t| parse_styles(&t)).unwrap_or_default();
    if parsed.is_empty() {
        builtin_styles()
    } else {
        parsed
    }
}

/// botstyles.reshape: a profile with aggression and teamwork squeezed into the
/// style's ranges, CZ's own reaction delay cut down (the plugin adds human
/// reaction times) and skill scaled.
pub fn reshape(text: &str, style: &Style, seed: u64) -> String {
    static FIELD: OnceLock<Regex> = OnceLock::new();
    let field = FIELD.get_or_init(|| {
        Regex::new(r"^(\s*)(Aggression|Teamwork|ReactionTime|AttackDelay|Skill)(\s*=\s*)([0-9.]+)(.*)$").unwrap()
    });
    let (alo, ahi) = style.aggression;
    let (tlo, thi) = style.teamwork;
    let mut rnd = PyRandom::new(seed + style.name.chars().count() as u64);
    let mut out: Vec<String> = Vec::new();
    for line in splitlines(text, false) {
        let Some(m) = field.captures(line) else {
            out.push(line.to_string());
            continue;
        };
        let (indent, key, eq, val, rest) = (&m[1], &m[2], &m[3], &m[4], &m[5]);
        let Ok(v) = val.parse::<f64>() else {
            out.push(line.to_string());
            continue;
        };
        let new = match key {
            "Aggression" => {
                // keep each bot's relative temperament, squeezed into the style's range
                let mut v = alo + (ahi - alo) * v / 100.0;
                if ahi > alo {
                    v += rnd.uniform(-5.0, 5.0);
                }
                format!("{}", v.min(100.0).max(0.0).trunc() as i64)
            }
            "Teamwork" => format!("{}", (tlo + (thi - tlo) * v / 100.0).trunc() as i64),
            "ReactionTime" => "0.1".into(),
            "AttackDelay" => "0".into(),
            _ => match style.skill_scale {
                None => "100".into(),
                Some(k) => format!("{}", (v * k).trunc() as i64),
            },
        };
        out.push(format!("{indent}{key}{eq}{new}{rest}"));
    }
    out.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_match_botstyles_py() {
        let root = super::super::repo_root();
        let py = std::fs::read_to_string(root.join("panel/botstyles.py")).unwrap();
        assert_eq!(parse_styles(&py), builtin_styles());
    }

    #[test]
    fn generate_matches_python() {
        let root = super::super::repo_root();
        let script = format!(
            "import sys; sys.path.insert(0, {:?}); from installer import botprofile; sys.stdout.buffer.write(botprofile.generate().encode('latin-1'))",
            root.to_string_lossy()
        );
        let Some(out) = super::super::test_python(&script) else { return };
        assert_eq!(generate(), out);
    }

    #[test]
    fn reshape_matches_python() {
        let root = super::super::repo_root();
        let base = generate();
        let tmp = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "bp-").unwrap();
        let src = tmp.join("BotProfile.db");
        std::fs::write(&src, &base).unwrap();
        for st in builtin_styles() {
            let script = format!(
                "import sys; sys.path.insert(0, {:?}); import botstyles; t=open({:?}, encoding='latin-1').read(); sys.stdout.buffer.write(botstyles.reshape(t, {:?}).encode('latin-1'))",
                root.join("panel").to_string_lossy(),
                src.to_string_lossy(),
                st.name
            );
            let Some(out) = super::super::test_python(&script) else { return };
            // Python reads with universal newlines here; the text has CRLF which becomes \n, same lines
            assert_eq!(reshape(&base, &st, 27015), out, "{}", st.name);
        }
        let _ = std::fs::remove_dir_all(tmp);
    }
}
