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
    t.is_empty()
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
    fn drops_junk() {
        assert!(junk(" [BLANK_AUDIO]"));
        assert!(junk("Thank you."));
        assert!(!junk("Rush B."));
    }
}
