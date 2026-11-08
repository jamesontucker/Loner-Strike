//! Bot personalities: the original CS 1.6 bot profile, and the bot style files
//! made from a profile (one per style and way of playing together, for
//! gm_humanbots.amxx). Both use Python's random with fixed seeds, so the same
//! profile always gives the same files.

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

/// (name, aggression lo/hi, teamwork cap lo/hi, share of team players, skill scale;
/// None: every bot at Skill 100)
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub name: String,
    pub aggression: (f64, f64),
    pub teamwork: (f64, f64),
    /// how many team players the style has, against the flavour's usual share
    pub team_players: f64,
    pub skill_scale: Option<f64>,
}

/// The styles: aggression range, the bounds teamwork is kept in, how many team
/// players, and how skill is scaled.
pub fn builtin_styles() -> Vec<Style> {
    let s = |name: &str, a: (f64, f64), t: (f64, f64), tp: f64, k: Option<f64>| Style { name: name.into(), aggression: a, teamwork: t, team_players: tp, skill_scale: k };
    vec![
        s("Pub", (25.0, 100.0), (0.0, 75.0), 1.0, Some(1.0)),
        s("Matchmaking", (30.0, 90.0), (0.0, 85.0), 1.4, Some(1.0)),
        s("Veterans", (35.0, 95.0), (0.0, 85.0), 1.2, Some(1.0)),
        s("Newbie", (30.0, 95.0), (0.0, 70.0), 0.6, Some(0.6)),
        s("LockedIn", (40.0, 100.0), (0.0, 90.0), 1.4, Some(1.0)),
        s("Hackers", (60.0, 100.0), (0.0, 70.0), 0.6, Some(1.0)),
        s("Gods", (100.0, 100.0), (0.0, 100.0), 1.0, None),
    ]
}

/// The bot styles.
pub fn styles(_res: &Path) -> Vec<Style> {
    builtin_styles()
}

/// How bots play together, picked per mode or map in configs/gamemodes/botplay.ini.
/// Each style has one file per flavour; CZ reads it when a map loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// a team (Match, and the Matchmaking style on Classic): calls followed, sites split
    Team,
    /// individuals, like a real pub server (the default): hardly any grouping, a wide
    /// mix of playstyles
    Solo,
    /// individuals who mostly play it safe and hold angles (aim and duel maps)
    Hold,
}

impl Flavour {
    pub const ALL: [Flavour; 3] = [Flavour::Team, Flavour::Solo, Flavour::Hold];

    /// What goes after the style's name in the file name (Team keeps the plain name).
    pub fn suffix(self) -> &'static str {
        match self {
            Flavour::Team => "",
            Flavour::Solo => "_Solo",
            Flavour::Hold => "_Hold",
        }
    }

    pub fn parse(word: &str) -> Option<Flavour> {
        match word.to_ascii_lowercase().as_str() {
            "team" => Some(Flavour::Team),
            "solo" => Some(Flavour::Solo),
            "hold" => Some(Flavour::Hold),
            _ => None,
        }
    }

    /// Chance weights of each playstyle (the order of PLAYSTYLES).
    fn weights(self, team_players: f64) -> [f64; 5] {
        let [r, ro, l, a, t] = match self {
            Flavour::Team => [22.0, 22.0, 18.0, 22.0, 16.0],
            Flavour::Solo => [25.0, 30.0, 20.0, 20.0, 5.0],
            Flavour::Hold => [8.0, 15.0, 22.0, 50.0, 5.0],
        };
        [r, ro, l, a, t * team_players]
    }
}

/// File name of a style's personalities for a flavour: BotProfile_Pub_Solo.db.
pub fn style_file(style: &str, flavour: Flavour) -> String {
    format!("BotProfile_{style}{}.db", flavour.suffix())
}

/// The flavour for a mode on a map, from botplay.ini's text: a map rule (ending in *,
/// or the exact map) wins over the mode's rule; anything not listed plays solo.
pub fn flavour_for(botplay: &str, mode: &str, map: &str) -> Flavour {
    let (mut by_map, mut by_mode) = (None, None);
    for line in botplay.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        let mut words = line.split_whitespace();
        let (Some(key), Some(f)) = (words.next(), words.next().and_then(Flavour::parse)) else { continue };
        let key = key.to_ascii_lowercase();
        let map = map.to_ascii_lowercase();
        if let Some(prefix) = key.strip_suffix('*') {
            if by_map.is_none() && map.starts_with(prefix) {
                by_map = Some(f);
            }
        } else if by_map.is_none() && key == map {
            by_map = Some(f);
        } else if by_mode.is_none() && key == mode.to_ascii_lowercase() {
            by_mode = Some(f);
        }
    }
    by_map.or(by_mode).unwrap_or(Flavour::Solo)
}

/// Playstyles: (name, aggression lo/hi as a share of the style's range, teamwork lo/hi).
/// Teamwork above 40 is what lets a CZ bot tag along behind a human teammate (and the
/// higher it is, the less often it goes rogue), so only team players get there.
const PLAYSTYLES: [(&str, (f64, f64), (f64, f64)); 5] = [
    ("rusher", (0.80, 1.00), (20.0, 38.0)),
    ("roamer", (0.50, 0.80), (10.0, 32.0)),
    ("lurker", (0.25, 0.50), (0.0, 15.0)),
    ("careful", (0.00, 0.30), (25.0, 40.0)),
    ("team player", (0.35, 0.75), (50.0, 80.0)),
];

/// botstyles.reshape: a profile where every bot gets its own playstyle (rusher, roamer,
/// lurker, careful player who holds angles, or the odd team player) within the style's
/// aggression range, CZ's own reaction delay cut down (the plugin adds human reaction
/// times) and skill scaled.
///
/// CZ's profile gives most bots their aggression through a skill template and every
/// bot the same teamwork from Default, so each bot's block gets its own lines here.
pub fn reshape(text: &str, style: &Style, flavour: Flavour, seed: u64) -> String {
    static FIELD: OnceLock<Regex> = OnceLock::new();
    let field = FIELD.get_or_init(|| {
        Regex::new(r"^(\s*)(Aggression|Teamwork|ReactionTime|AttackDelay|Skill)(\s*=\s*)([0-9.]+)(.*)$").unwrap()
    });
    let (mut alo, ahi) = style.aggression;
    if flavour == Flavour::Hold {
        alo *= 0.6; // more room for the careful ones
    }
    let (tlo, thi) = style.teamwork;
    let weights = flavour.weights(style.team_players);
    let mut rnd = PyRandom::new(seed + style.name.chars().count() as u64 + flavour as u64 * 100);

    // aggression each template (and Default) gives, for bots without their own line
    let mut inherited: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut out: Vec<String> = Vec::new();
    // the block being read: its header's templates, whether it's a bot, its own aggression
    let mut block: Option<(Vec<String>, bool, Option<f64>, String)> = None;
    for line in splitlines(text, false) {
        let t = line.trim();
        let comment = t.is_empty() || t.starts_with("//");
        if block.is_none() && !comment && !line.starts_with(char::is_whitespace) {
            let mut words = t.split_whitespace();
            let first = words.next().unwrap_or("");
            let rest = words.next();
            let (templates, bot, name) = match first {
                "Default" => (vec![], false, "Default".to_string()),
                "Template" => (vec![], false, rest.unwrap_or("").to_string()),
                _ if rest.is_some() => (first.split('+').map(String::from).collect(), true, String::new()),
                _ => (vec![], true, String::new()),
            };
            block = Some((templates, bot, None, name));
            out.push(line.to_string());
            continue;
        }
        if t == "End" {
            if let Some((templates, bot, own, name)) = block.take() {
                if bot {
                    let base = own
                        .or_else(|| templates.iter().rev().find_map(|n| inherited.get(n).copied()))
                        .or_else(|| inherited.get("Default").copied())
                        .unwrap_or(50.0);
                    let (_, (plo, phi), (wlo, whi)) = PLAYSTYLES[rnd.choices_weighted(&weights)];
                    // mostly the playstyle, a little of the bot's own temperament
                    let pos = 0.75 * rnd.uniform(plo, phi) + 0.25 * base / 100.0;
                    let aggression = (alo + (ahi - alo) * pos).clamp(0.0, 100.0).trunc() as i64;
                    let mut teamwork = rnd.uniform(wlo, whi);
                    if flavour == Flavour::Team {
                        // a team: everyone at least willing (CZ's own bots sit around 75)
                        teamwork = 45.0 + teamwork * 0.6;
                    } else {
                        // individuals: half as much, and team players only just over the line
                        teamwork = if wlo >= 50.0 { rnd.uniform(41.0, 55.0) } else { teamwork * 0.5 };
                    }
                    let teamwork = teamwork.clamp(tlo, thi).trunc() as i64;
                    out.push(format!("\tAggression = {aggression}"));
                    out.push(format!("\tTeamwork = {teamwork}"));
                } else if let Some(a) = own {
                    inherited.insert(name, a);
                }
            }
            out.push(line.to_string());
            continue;
        }
        let Some(m) = field.captures(line) else {
            out.push(line.to_string());
            continue;
        };
        let (indent, key, eq, val, rest) = (&m[1], &m[2], &m[3], &m[4], &m[5]);
        let Ok(v) = val.parse::<f64>() else {
            out.push(line.to_string());
            continue;
        };
        let is_bot = block.as_ref().is_some_and(|b| b.1);
        let new = match key {
            "Aggression" => {
                if let Some(b) = block.as_mut() {
                    b.2 = Some(v);
                }
                if is_bot {
                    continue; // written at the end of the block
                }
                format!("{}", (alo + (ahi - alo) * v / 100.0).clamp(0.0, 100.0).trunc() as i64)
            }
            "Teamwork" => {
                if is_bot {
                    continue;
                }
                format!("{}", (30.0f64).clamp(tlo, thi).trunc() as i64)
            }
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

    const PROFILE: &str = "Default\r\n\tSkill = 50\r\n\tAggression = 50\r\n\tTeamwork = 75\r\nEnd\r\n\r\nTemplate Elite\r\n\tSkill = 100\r\n\tAggression = 100\r\nEnd\r\n\r\nTemplate Fair\r\n\tAggression = 30\r\nEnd\r\n";

    fn profile(bots: usize) -> String {
        let mut p = PROFILE.to_string();
        for i in 0..bots {
            p += &format!("\r\n{}+Rifle Bot{i}\r\n\tVoicePitch = 100\r\n{}End\r\n", if i % 2 == 0 { "Elite" } else { "Fair" }, if i % 7 == 0 { "\tTeamwork = 100\r\n" } else { "" });
        }
        p
    }

    /// (aggression, teamwork) of every bot in a reshaped profile
    fn bots(text: &str) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        let mut cur = (None, None);
        let mut in_bot = false;
        for l in text.lines() {
            let l = l.trim();
            if l.starts_with("Elite+") || l.starts_with("Fair+") {
                in_bot = true;
            } else if l == "End" {
                if in_bot {
                    out.push((cur.0.unwrap(), cur.1.unwrap()));
                }
                in_bot = false;
                cur = (None, None);
            } else if in_bot {
                if let Some(v) = l.strip_prefix("Aggression = ") {
                    assert!(cur.0.is_none(), "one aggression line per bot");
                    cur.0 = v.parse().ok();
                } else if let Some(v) = l.strip_prefix("Teamwork = ") {
                    assert!(cur.1.is_none(), "one teamwork line per bot");
                    cur.1 = v.parse().ok();
                }
            }
        }
        out
    }

    #[test]
    fn every_bot_gets_its_own_playstyle() {
        let pub_style = builtin_styles().into_iter().find(|s| s.name == "Pub").unwrap();
        for flavour in Flavour::ALL {
            let b = bots(&reshape(&profile(80), &pub_style, flavour, 27015));
            assert_eq!(b.len(), 80);
            let aggr: std::collections::BTreeSet<i64> = b.iter().map(|x| x.0).collect();
            assert!(aggr.len() > 25, "{flavour:?}: aggression is spread out ({aggr:?})");
            assert!(b.iter().any(|x| x.0 >= 85) && b.iter().any(|x| x.0 <= 45), "{flavour:?}: rushers and careful players");
            let followers = b.iter().filter(|x| x.1 > 40).count();
            if flavour == Flavour::Team {
                // a team: nearly everyone willing to play together
                assert!(followers >= 80 * 3 / 4, "{flavour:?}: {followers} team players");
            } else {
                // individuals: most can't tag along behind a human (teamwork 40 or less), a few can
                assert!(followers <= 80 / 4, "{flavour:?}: {followers} team players");
                assert!(followers >= 1, "{flavour:?}: the odd team player");
            }
            assert!(b.iter().all(|x| (0..=75).contains(&x.1)));
        }
        let team = bots(&reshape(&profile(80), &pub_style, Flavour::Team, 27015));
        let solo = bots(&reshape(&profile(80), &pub_style, Flavour::Solo, 27015));
        let hold = bots(&reshape(&profile(80), &pub_style, Flavour::Hold, 27015));
        let mean = |v: &[(i64, i64)], f: fn(&(i64, i64)) -> i64| v.iter().map(f).sum::<i64>() as f64 / v.len() as f64;
        assert!(mean(&solo, |x| x.1) < mean(&team, |x| x.1) * 0.7, "individuals team up much less");
        assert!(mean(&hold, |x| x.0) < mean(&solo, |x| x.0), "holding is less aggressive");
        assert!(team.iter().filter(|x| x.1 > 40).count() > hold.iter().filter(|x| x.1 > 40).count());
    }

    #[test]
    fn templates_keep_their_lines_and_skill_scales() {
        let newbie = builtin_styles().into_iter().find(|s| s.name == "Newbie").unwrap();
        let out = reshape(&profile(2), &newbie, Flavour::Solo, 1);
        assert!(out.contains("Template Elite\r\n\tSkill = 60\r\n\tAggression = 95\r\nEnd"), "{out}");
        assert!(out.contains("Default\r\n\tSkill = 30\r\n"));
        assert_eq!(out, reshape(&profile(2), &newbie, Flavour::Solo, 1), "same input, same file");
    }

    #[test]
    fn botplay_rules() {
        let ini = "; comment\r\nclassic team\r\nmatch team\r\nscoutz hold\r\naim_* hold\r\nfy_* hold\r\nde_dust2 solo\r\n";
        assert_eq!(flavour_for(ini, "classic", "de_inferno"), Flavour::Team);
        assert_eq!(flavour_for(ini, "classic", "aim_map"), Flavour::Hold);
        assert_eq!(flavour_for(ini, "classic", "de_dust2"), Flavour::Solo);
        assert_eq!(flavour_for(ini, "csdm", "de_inferno"), Flavour::Solo);
        assert_eq!(flavour_for(ini, "scoutz", "de_inferno"), Flavour::Hold);
        assert_eq!(flavour_for(ini, "CSDM", "FY_iceworld"), Flavour::Hold);
        assert_eq!(flavour_for("", "classic", "de_dust"), Flavour::Solo);
        assert_eq!(style_file("Pub", Flavour::Hold), "BotProfile_Pub_Hold.db");
        assert_eq!(style_file("Pub", Flavour::Team), "BotProfile_Pub.db");
    }
}
