//! What Whisper heard, tidied for the bots: site letters it writes as words ("rush be"), bot
//! names it hears a little wrong ("Derrick" for Derek), fillers ("uh", "yo bro"), the same call
//! said three times. The bot chat (gm_botchat) does the understanding.

use regex::Regex;
use std::sync::OnceLock;

struct Fixes {
    rules: Vec<(Regex, &'static str)>,
    filler: Regex,
    lead: Regex,
    tail: Regex,
}

fn fixes() -> &'static Fixes {
    static F: OnceLock<Fixes> = OnceLock::new();
    F.get_or_init(|| {
        let r = |p: &str| Regex::new(p).unwrap();
        let verbs = "to|rush|go|at|on|in|hit|take|rotate|site|bombsite|stack|push|retake|plant|smoke|flash|fake";
        Fixes {
            rules: vec![
                (r(r"(?i)\brush(?:ay|a)\b"), "rush A"),
                (r(&format!(r"(?i)\b({verbs})\s+(?:be|bee)\b")), "$1 B"),
                (r(&format!(r"(?i)\b({verbs})\s+(?:a|ay|eh)(\s*(?:[.,!?]|$))")), "$1 A$2"),
                (r(r"(?i)\bbomb ?sight\b"), "bombsite"),
                (r(r"(?i)\bbon ?sight\b"), "bombsite"),
            ],
            filler: r(r"(?i)\b(?:uh+|um+|erm+|uhm+|hmm+)\b,?\s*|\b(?:like|you know|i mean),\s*"),
            lead: r(r"(?i)^(?:(?:yo|bro|dude|ok so|okay so|so|hey|guys|man)\b[\s,]*)+"),
            tail: r(r"(?i)(?:[\s,]+(?:bro|dude|man|guys|yo))+$"),
        }
    })
}

/// Whisper's empty or made-up lines on silence and noise: dropped.
pub fn junk(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| ".!?,".contains(c)).to_lowercase();
    // (a sound, not words: "Mm-.", "Hmm", "Uh")
    let letters: String = t.chars().filter(|c| c.is_alphabetic()).collect();
    let mumble = !letters.is_empty() && letters.len() <= 4 && ["m", "h", "u", "a", "e", "o"].iter().any(|s| letters.starts_with(s))
        && letters.chars().all(|c| "mhuaeo".contains(c));
    mumble
        || t.is_empty()
        || t.starts_with('[')
        || t.starts_with('(')
        || t.starts_with('*')
        || ["you", "thank you", "thanks for watching", "bye", "blank_audio", "silence", "music"].contains(&t.as_str())
}

fn callouts(text: &str) -> String {
    let mut t = text.to_string();
    for (re, to) in &fixes().rules {
        t = re.replace_all(&t, *to).into_owned();
    }
    t
}

/// A rough sound of a name: Derek, Derrick and Deric all "drk".
fn sound_key(w: &str) -> String {
    let mut w = w.to_lowercase();
    for (a, b) in [("ph", "f"), ("ck", "k"), ("q", "k"), ("c", "k"), ("z", "s"), ("y", "i"), ("wh", "w")] {
        w = w.replace(a, b);
    }
    let mut out = String::new();
    for (i, ch) in w.chars().enumerate() {
        if i > 0 && "aeiouh".contains(ch) {
            continue;
        }
        if out.chars().last() != Some(ch) {
            out.push(ch);
        }
    }
    out
}

fn edits(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb)));
        }
        prev = cur;
    }
    prev[b.len()]
}

const COMMON: &[&str] = &[
    "a", "an", "the", "to", "go", "me", "my", "you", "your", "we", "us", "they", "them", "he", "she", "it", "is", "are", "was", "be",
    "b", "on", "in", "at", "of", "for", "and", "or", "but", "so", "no", "yes", "ok", "okay", "hey", "yo", "bro", "dude", "guys", "man",
    "come", "rush", "push", "wait", "hold", "stop", "follow", "cover", "drop", "give", "bomb", "money", "gun", "nice", "good", "bad",
    "one", "two", "three", "four", "five", "long", "short", "mid", "site", "spawn", "left", "right", "back", "front", "here", "there",
    "that", "this", "what", "where", "how", "who", "why", "when", "lets", "let", "get", "got", "have", "has", "just", "now", "all",
    "some", "any", "every", "everyone", "team", "please", "thanks", "low", "high", "up", "down", "out", "over", "under", "behind",
    "check", "watch", "look", "see", "saw", "shot", "dead", "kill", "killed", "flash", "smoke", "nade", "eco", "force", "buy", "awp",
    "fake", "plant", "defuse", "rotate", "trade", "split", "save", "fall",
];

/// The parts of the bots' names worth matching ("Derek S." -> "Derek"; "::CoD:: Gill" -> "CoD", "Gill").
pub fn name_parts(bots: &[String]) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for b in bots {
        for p in b.split(|c: char| !c.is_alphanumeric()) {
            if p.len() >= 3 && !COMMON.contains(&p.to_lowercase().as_str()) && !parts.iter().any(|x| x.eq_ignore_ascii_case(p)) {
                parts.push(p.to_string());
            }
        }
    }
    parts
}

/// A name in the first two words of each part of the line, heard a little off, put right.
fn fix_names(text: &str, parts: &[String]) -> String {
    let fix_clause = |clause: &str| -> String {
        let mut words: Vec<String> = clause.split(' ').map(str::to_string).collect();
        let mut seen = 0;
        for w in words.iter_mut() {
            let core: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
            if core.is_empty() {
                continue;
            }
            seen += 1;
            if seen > 2 {
                break;
            }
            let lw = core.to_lowercase();
            if COMMON.contains(&lw.as_str()) || core.len() < 3 || parts.iter().any(|p| p.eq_ignore_ascii_case(&core)) {
                continue;
            }
            let hit = parts.iter().find(|p| {
                let pl = p.to_lowercase();
                pl.chars().next() == lw.chars().next()
                    && (sound_key(&core) == sound_key(p) || edits(&lw, &pl) <= if p.len() <= 5 { 1 } else { 2 })
            });
            if let Some(p) = hit {
                *w = w.replace(&core, p);
            }
        }
        words.join(" ")
    };
    let mut out = String::new();
    let mut clause = String::new();
    for ch in text.chars() {
        if ",.!?".contains(ch) {
            out.push_str(&fix_clause(&clause));
            out.push(ch);
            clause.clear();
        } else {
            clause.push(ch);
        }
    }
    out.push_str(&fix_clause(&clause));
    out
}

fn key(c: &str) -> String {
    c.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// Fillers out, repeats said once: "Uh, rush be, rush be, rush be, guys." -> "Rush B."
fn tidy(text: &str) -> String {
    let f = fixes();
    let text = f.filler.replace_all(text, "").into_owned();
    let mut sentences: Vec<String> = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let cut = rest.find(['.', '!', '?']).map(|i| i + 1).unwrap_or(rest.len());
        let s = &rest[..cut];
        rest = rest[cut..].trim_start();
        let body = s.trim_end_matches(['.', '!', '?', ' ']);
        let end = s[body.len()..].trim().chars().next().map(String::from).unwrap_or_default();
        let mut clauses: Vec<String> = Vec::new();
        for c in body.split(',') {
            let c = f.lead.replace(c.trim(), "").into_owned();
            let c = f.tail.replace(c.trim(), "").trim().to_string();
            if !c.is_empty() && clauses.last().map(|l| key(l) != key(&c)).unwrap_or(true) {
                clauses.push(c);
            }
        }
        if clauses.is_empty() {
            continue;
        }
        let sent = format!("{}{}", clauses.join(", "), end);
        if sentences.last().map(|l| key(l) != key(&sent)).unwrap_or(true) {
            sentences.push(sent);
        }
    }
    let t = sentences.join(" ");
    let mut c = t.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => t,
    }
}

/// The whole tidy-up, and safe to put in quotes on a console line.
pub fn clean(text: &str, name_parts: &[String]) -> String {
    let t = tidy(&fix_names(&callouts(text.trim()), name_parts));
    t.replace('"', "'").replace(';', ",").chars().filter(|c| !c.is_control()).take(120).collect()
}

// ---- the understanding model's answers ---------------------------------------------------------

fn command_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        let place = r"[a-z0-9][a-z0-9 ]{0,24}";
        let one = [
            format!(r"go {place}( slow)?"),
            r"fake (a|b)".into(),
            r"fall back|watch flank|follow me|come here|wait here|release|eco|force|full buy|go defuse|give me the bomb".into(),
            r"buy me [a-z0-9]{2,8}|drop me ([a-z0-9]{2,8}|your gun)".into(),
            format!(r"plant( (a|b|here))?|flash for me|flash {place}|smoke {place}|nade (them|{place})"),
            format!(r"[1-5] {place}|they are {place}|awp {place}"),
            r"money\?|what gun\?|hp\?|how many left\?|where are they\?|where are you\?|wheres the bomb\?|what do they have\?|who has [a-z0-9]{2,8}\?".into(),
            r"jump|crouch|spray that wall".into(),
        ]
        .join("|");
        let part = format!(r"([a-z0-9]{{2,24}}, )*({one})");
        Regex::new(&format!(r"^{part}( and {part})*$")).unwrap()
    })
}

/// What goes to the bots for a line, given the understanding model's answer: its command
/// when it's one the bot chat knows (names put right, "release" said as "nvm"); otherwise,
/// and for "nothing" (chatter, trash talk), the words as heard, so the bots still react.
pub fn with_intent(heard: &str, answer: Option<&str>, name_parts: &[String]) -> String {
    let Some(a) = answer.map(|a| a.trim().to_lowercase()) else { return heard.to_string() };
    if a == "nothing" || a.is_empty() || !command_re().is_match(&a) {
        return heard.to_string();
    }
    let a = Regex::new(r"\brelease\b").unwrap().replace_all(&a, "nvm").into_owned();
    // ("gil, go mid": the bot's name as the server has it)
    let fixed = fix_names(&a, name_parts);
    fixed.replace('"', "'").replace(';', ",")
}

/// The places to tell the model about, for the maps people play most (the map's callouts);
/// anything else gets the usual few.
pub fn places(map: &str) -> &'static str {
    match map {
        "de_dust2" => "long, short, mid, catwalk, tunnels, upper tunnels, lower tunnels, ct spawn, t spawn, pit, car, goose, doors, outside long",
        "de_inferno" => "banana, apartments, mid, pit, library, arch, graveyard, boiler, second mid, ct spawn, t spawn, construction",
        "de_nuke" => "outside, ramp, hut, lobby, heaven, hell, secret, vents, squeaky, silo",
        "de_aztec" => "bridge, water, ramp, double doors, cave, palace, ct spawn, t spawn, jungle",
        "de_train" => "ivy, pop dog, z connector, ladder room, upper, lower, brown halls, sandwich",
        "cs_office" => "front door, back hall, kitchen, snowy, conference, garage, fire exit, long hall",
        "de_cbble" => "long a, drop, fountain, hut, connector, rock, b long, sewers",
        "cs_italy" => "market, wine cellar, long hall, apartments, ct spawn, t spawn",
        _ => "long, short, mid, ct spawn, t spawn, a site, b site",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        name_parts(&["Derek S.".into(), "::CoD:: Gill".into(), "Kale".into(), "Andre F.".into()])
    }

    #[test]
    fn cleans_what_whisper_hears() {
        let n = names();
        for (heard, want) in [
            ("Derrick, how much money do you have?", "Derek, how much money do you have?"),
            ("Uh, rush be, rush be, rush be.", "Rush B."),
            ("Come to be, guys. Come to be.", "Come to B."),
            ("Yo bro, two coming long.", "Two coming long."),
            ("Um, Rushay, rushay.", "Rush A."),
            ("Gil, follow me, man.", "Gill, follow me."),
            ("Okay so, Andray, drop me your gun.", "Andre, drop me your gun."),
            ("Bomb sight A.", "Bombsite A."),
        ] {
            assert_eq!(clean(heard, &n), want, "{heard}");
        }
    }

    #[test]
    fn uses_the_models_command_only_when_it_is_one() {
        let n = names();
        assert_eq!(with_intent("Can somebody get over to B real quick", Some("go b"), &n), "go b");
        assert_eq!(with_intent("Yo Gil hold mid for me", Some("gil, go mid"), &n), "Gill, go mid");
        assert_eq!(with_intent("Never mind", Some("release"), &n), "nvm");
        assert_eq!(with_intent("You're trash", Some("nothing"), &n), "You're trash");
        assert_eq!(with_intent("Anyone play Warcraft 3", Some("fake warcraft"), &n), "Anyone play Warcraft 3");
        assert_eq!(with_intent("Money and HP?", Some("money? and hp?"), &n), "money? and hp?");
        assert_eq!(with_intent("Two long", Some("2 long"), &n), "2 long");
        assert_eq!(with_intent("Rush B", None, &n), "Rush B");
    }

    #[test]
    fn drops_junk() {
        assert!(junk(" [BLANK_AUDIO]"));
        assert!(junk("Thank you."));
        assert!(junk("Mm-.") && junk("Hmm") && junk("Uh."));
        assert!(!junk("Ok") && !junk("Go") && !junk("Eco"));
        assert!(!junk("Rush B."));
    }
}
