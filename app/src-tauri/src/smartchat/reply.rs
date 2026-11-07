//! Turning a chat moment into a prompt, and the model's answer into one pub chat line
//! (or nothing, when it reads like an assistant).

use super::catalog::PromptMode;
use serde::Deserialize;
use serde_json::{json, Value};

/// What the plugin sends (POST /api/botchat/reply).
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ReplyReq {
    pub bot: String,
    pub persona: String,
    pub mood: String,
    /// "reply" (to the human) or "thread" (a short back-and-forth)
    pub kind: String,
    pub message: String,
    pub human: String,
    pub history: Vec<String>,
    /// map, score, alive, and whatever else the plugin adds (unknown keys are fine)
    pub context: serde_json::Map<String, Value>,
}

/// Plain printable ASCII, no newlines, at most `max` characters (game text is Latin-1,
/// and a line break would end a prompt line early).
pub fn plain(s: &str, max: usize) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).filter(|c| c.is_ascii()).take(max).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl ReplyReq {
    /// The request with every field cut down to size and made safe to put in a prompt.
    pub fn tidy(mut self) -> ReplyReq {
        self.bot = plain(&self.bot, 32);
        if self.bot.is_empty() {
            self.bot = "player".into();
        }
        self.human = plain(&self.human, 32);
        if self.human.is_empty() {
            self.human = "someone".into();
        }
        self.persona = plain(&self.persona, 80);
        self.mood = plain(&self.mood, 16).to_lowercase();
        self.kind = plain(&self.kind, 16).to_lowercase();
        self.message = plain(&self.message, 160);
        let n = self.history.len();
        self.history = self.history.split_off(n.saturating_sub(8)).iter().map(|l| plain(l, 160)).filter(|l| !l.is_empty()).collect();
        self
    }

    fn ctx(&self, key: &str) -> Option<String> {
        match self.context.get(key)? {
            Value::String(s) => Some(plain(s, 40)).filter(|s| !s.is_empty()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// "de_dust2, score 5-3, Gordon is dead, killed from: mid": what the plugin told us
    /// about the moment. Known keys go first in a fixed order, then at most a couple of others.
    fn situation(&self) -> String {
        let mut parts = Vec::new();
        if let Some(m) = self.ctx("map") {
            parts.push(m);
        }
        if let Some(s) = self.ctx("score") {
            parts.push(format!("score {s}"));
        }
        match self.context.get("alive") {
            Some(Value::Bool(false)) => parts.push(format!("{} is dead", self.bot)),
            Some(Value::Bool(true)) => parts.push(format!("{} is alive", self.bot)),
            _ => {}
        }
        let bool_of = |k: &str| self.context.get(k).and_then(|v| v.as_bool().or(v.as_i64().map(|n| n != 0))).unwrap_or(false);
        for (key, label) in KNOWN_CONTEXT {
            if parts.len() >= 9 {
                break;
            }
            match *key {
                "headshot" if bool_of(key) => parts.push("it was a headshot".into()),
                "human_has_bomb" if bool_of(key) => parts.push(format!("{} has the bomb", self.human)),
                "bot_has_bomb" if bool_of(key) => parts.push(format!("{} has the bomb", self.bot)),
                "team_chat" if bool_of(key) => parts.push("team chat".into()),
                // knows_human and topic go elsewhere in the prompt
                "headshot" | "human_has_bomb" | "bot_has_bomb" | "team_chat" | "knows_human" | "topic" => {}
                _ => {
                    if let Some(v) = self.ctx(key) {
                        parts.push(format!("{label} {v}"));
                    }
                }
            }
        }
        // anything else the plugin adds, in plain words
        let mut extra = 0;
        for (k, v) in &self.context {
            if extra >= 2 || ["map", "score", "alive"].contains(&k.as_str()) || KNOWN_CONTEXT.iter().any(|(x, _)| x == k) {
                continue;
            }
            let v = match v {
                Value::String(s) => plain(s, 40),
                Value::Number(n) => n.to_string(),
                Value::Bool(true) => "yes".into(),
                _ => continue,
            };
            let k = plain(&k.replace('_', " "), 24);
            if !k.is_empty() && !v.is_empty() {
                parts.push(format!("{k}: {v}"));
                extra += 1;
            }
        }
        parts.join(", ")
    }

    /// The bot has played this person before: casual and familiar.
    fn knows_human(&self) -> bool {
        self.context.get("knows_human").and_then(|v| v.as_bool().or(v.as_i64().map(|n| n != 0))).unwrap_or(false)
    }

    /// What a back-and-forth is about, if the plugin said.
    fn topic(&self) -> Option<String> {
        self.ctx("topic")
    }

    /// One line about the bot and who it's talking to (both prompt modes use it).
    fn who(&self) -> String {
        let mut s = format!("{} is {}, {}", self.bot, persona_text(&self.persona), mood_text(&self.mood));
        if self.knows_human() {
            s.push_str(&format!(", has played with {} before", self.human));
        }
        if let Some(t) = self.topic() {
            s.push_str(&format!(", talking about {t}"));
        }
        s
    }
}

/// Context keys the plugin sends, in the order they matter, with a few words for each.
const KNOWN_CONTEXT: &[(&str, &str)] = &[
    ("killer_place", "killed from"),
    ("enemy_place", "enemy at"),
    ("bomb_place", "bomb at"),
    ("killer_weapon", "killed with"),
    ("headshot", ""),
    ("bomb_carrier", "bomb carrier"),
    ("human_has_bomb", ""),
    ("bot_has_bomb", ""),
    ("timeleft_min", "minutes left"),
    ("nextmap", "next map"),
    ("topic", ""),
    ("knows_human", ""),
    ("team_chat", ""),
];

/// The plugin's personality letters and short keys, in a few words for the model.
pub fn persona_text(p: &str) -> String {
    let known: &[(&[&str], &str)] = &[
        (&["f", "friendly"], "friendly and easygoing"),
        (&["t", "tryhard"], "a tryhard who takes the game seriously"),
        (&["s", "salty"], "salty and quick to complain"),
        (&["j", "joker"], "a joker who never takes it seriously"),
        (&["q", "quiet"], "quiet, says very little"),
        (&["n", "new", "newbie"], "new to the game and a bit lost"),
        (&["c", "chill"], "chill and relaxed"),
        (&["o", "oldtimer", "old-timer", "veteran"], "an old-timer who has played since 1.3"),
        (&["k", "kid"], "a young kid, loud and excitable"),
        (&["x", "esl"], "not a native english speaker, simple words"),
        (&["b", "backseat"], "a backseat gamer who tells people what to do"),
        (&["i", "caller"], "the team's caller"),
        (&["d", "sleepy", "tired"], "half asleep"),
        (&["u", "selfblame"], "quick to blame himself"),
        (&["B", "brit"], "british"),
        (&["D", "dad"], "a dad who plays after the kids are in bed"),
        (&["H", "paranoid"], "sure everyone is hacking"),
        (&["L", "lurker"], "a lurker who sneaks around alone"),
        (&["O", "overconfident", "cocky"], "overconfident"),
        (&["S", "movement"], "a movement nerd who bunnyhops everywhere"),
    ];
    let t = p.trim();
    if let Some((_, d)) = known.iter().find(|(keys, _)| keys.iter().any(|k| if k.len() == 1 { *k == t } else { k.eq_ignore_ascii_case(t) })) {
        return d.to_string();
    }
    if t.is_empty() {
        "a regular pub player".into()
    } else {
        t.to_lowercase()
    }
}

/// A loud persona keeps its capitals; everyone else types in lowercase.
pub fn is_loud(persona: &str) -> bool {
    let p = persona.to_lowercase();
    persona.trim() == "k" || ["loud", "caps", "shout", "yell", "kid"].iter().any(|w| p.contains(w))
}

fn mood_text(m: &str) -> &'static str {
    match m {
        "tilted" => "tilted and annoyed, losing",
        "cocky" => "cocky, doing well",
        "polite" => "polite and friendly",
        _ => "relaxed",
    }
}

// ---- continue mode: a base model finishes a chat log ----------------------------------

/// The start of every log: the same for every request, so llama-server keeps it cached.
pub const LOG_HEAD: &str = "Chat log from a Counter-Strike 1.6 public server, 2004. Everyone types short, lowercase, with typos and slang. Nobody writes full sentences.\n\
\n\
[de_dust2]\n\
Wolf: nt\n\
xX_eLiTe_Xx: lol no\n\
Gordon: b i think\n\
Wolf: rush b\n\
bob: wtf was that\n\
Gordon: lag\n\
kRaZy: ez\n\
bob: says u\n\
Wolf: nice shot\n\
kRaZy: ty\n\
Gordon: who has the bomb\n\
bob: me\n\
Wolf: gg\n\
\n\
[cs_office]\n\
Mike: anyone got a deagle\n\
fragger: no\n\
Mike: k\n\
ZeRo: camper\n\
fragger: lol cry\n\
ZeRo: whatever\n\
Mike: ns\n\
dave: sry tk\n\
fragger: np\n\
\n";

pub fn continue_prompt(r: &ReplyReq) -> String {
    let mut s = String::from(LOG_HEAD);
    s.push_str(&format!("[{}]\n", r.ctx("map").unwrap_or_else(|| "de_dust2".into())));
    // a line the model reads as part of the log, so it carries the persona and mood
    s.push_str(&format!("* {}\n", r.who()));
    let sit = r.situation();
    if !sit.is_empty() {
        s.push_str(&format!("* {sit}\n"));
    }
    for l in &r.history {
        s.push_str(l);
        s.push('\n');
    }
    if !r.message.is_empty() {
        let line = format!("{}: {}", r.human, r.message);
        // the plugin may already have put it last in the history
        if r.history.last() != Some(&line) {
            s.push_str(&line);
            s.push('\n');
        }
    }
    s.push_str(&format!("{}:", r.bot));
    s
}

// ---- chat mode: a chat model with a short system prompt and examples -----------------

/// The system prompt, the same for every request (so it stays cached).
pub const SYSTEM: &str = "Enter Roleplay Mode. You are a player on a Counter-Strike 1.6 public server in 2004, typing in the game's chat.\n\
Roleplaying Instructions:\n\
- answer with one short chat line, usually 1 to 6 words\n\
- lowercase, little punctuation, pub slang like nt, ns, gg, lol, rush b, ez, says u\n\
- you can be rude, bored or joking, like real players\n\
- never explain, never offer help, never say you are an ai\n\
- only write the line itself";

/// Example turns: the shape every request takes, answered the way a pub player would.
const EXAMPLES: &[(&str, &str)] = &[
    ("you are Wolf, chill. map de_dust2, score 3-5.\nkRaZy: nice try", "nt"),
    ("you are Gordon, salty, tilted. map de_dust2.\nbob: are u a bot", "lol no"),
    ("you are ZeRo, a tryhard. map de_inferno.\nbob: where they going", "b i think"),
    ("you are Mike, overconfident, cocky. map cs_office.\nfragger: ur bad", "says u"),
    ("you are dave, a dad, polite. map de_aztec.\nkid: sorry for tk", "np"),
];

fn chat_user(r: &ReplyReq) -> String {
    let mut s = format!("you are {}.", r.who().replacen(&format!("{} is ", r.bot), &format!("{}, ", r.bot), 1));
    let sit = r.situation();
    if !sit.is_empty() {
        s.push_str(&format!(" {sit}."));
    }
    let line = format!("{}: {}", r.human, r.message);
    for l in r.history.iter().filter(|l| **l != line) {
        s.push('\n');
        s.push_str(l);
    }
    if !r.message.is_empty() {
        s.push('\n');
        s.push_str(&line);
    }
    s
}

pub fn chat_messages(r: &ReplyReq) -> Vec<Value> {
    let mut m = vec![json!({"role": "system", "content": SYSTEM})];
    for (u, a) in EXAMPLES {
        m.push(json!({"role": "user", "content": u}));
        m.push(json!({"role": "assistant", "content": a}));
    }
    m.push(json!({"role": "user", "content": chat_user(r)}));
    m
}

/// The request body for llama-server, and the path it goes to.
pub fn request_body(mode: PromptMode, r: &ReplyReq) -> (&'static str, Value) {
    let common = json!({
        "temperature": 0.9, "top_p": 0.95, "top_k": 64, "min_p": 0.05,
        "repeat_penalty": 1.1, "cache_prompt": true,
    });
    let mut body = match mode {
        PromptMode::Continue => json!({"prompt": continue_prompt(r), "n_predict": 24, "stop": ["\n"]}),
        PromptMode::Chat => json!({"messages": chat_messages(r), "max_tokens": 24, "stop": ["\n"],
                                   "chat_template_kwargs": {"enable_thinking": false}}),
    };
    if let (Value::Object(b), Value::Object(c)) = (&mut body, common) {
        b.extend(c);
    }
    (if mode == PromptMode::Continue { "/completion" } else { "/v1/chat/completions" }, body)
}

/// The generated text from llama-server's answer.
pub fn answer_text(mode: PromptMode, v: &Value) -> Option<String> {
    match mode {
        PromptMode::Continue => v["content"].as_str().map(String::from),
        PromptMode::Chat => v["choices"][0]["message"]["content"].as_str().map(String::from),
    }
}

// ---- the filter -------------------------------------------------------------------

/// Longest line a bot sends.
pub const MAX_CHARS: usize = 60;

/// Things only an assistant says.
const ASSISTANT: &[&str] = &[
    "as an ai", "an ai ", "ai language", "language model", "ai model", "i'm an ai", "im an ai", "i am an ai", "chatbot", "chat bot",
    "assistant", "i'm sorry", "i am sorry", "im sorry but", "sorry, but", "sure!", "certainly", "of course", "absolutely",
    "i cannot", "i can't help", "i can not", "i'm unable", "i am unable", "happy to help", "glad to help", "feel free", "let me know",
    "hope this helps", "great question", "how can i help", "how may i", "i understand", "i appreciate", "here is a",
    "openai", "chatgpt", "gemma", "llama", "roleplay", "role-play", "player:", "user:", "<|", "|>", "</", "[inst", "http", "www.",
];

/// Slurs and the like: a reply containing one is dropped. Stems match the start of a word.
const DENY_STEMS: &[&str] = &["nigg", "nigga", "fagg", "retard", "trann", "kike", "chink", "spick", "beaner", "wetback", "gook", "coon", "dyke"];
const DENY_WORDS: &[&str] = &["fag", "fags", "spic", "spics", "kys", "rape", "raped", "nazi", "hitler"];

/// Letters people swap in to dodge filters.
fn unleet(c: char) -> char {
    match c {
        '1' | '!' | '|' => 'i',
        '3' => 'e',
        '0' => 'o',
        '4' | '@' => 'a',
        '$' | '5' => 's',
        '7' => 't',
        _ => c,
    }
}

fn has_slur(text: &str) -> bool {
    let t: String = text.to_lowercase().chars().map(unleet).collect();
    t.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).any(|w| DENY_WORDS.contains(&w) || DENY_STEMS.iter().any(|s| w.starts_with(s)))
}

/// One pub chat line from the model's answer, or None when it's no good (too long,
/// polite, assistant-like, emojis, shouting, a slur, or just empty).
pub fn clean_reply(raw: &str, bot: &str, loud: bool) -> Option<String> {
    // curly quotes and dashes from chat models, made plain
    let raw: String = raw
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{0060}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            '\u{2026}' => '.',
            _ => c,
        })
        .collect();
    let mut s = raw.trim_start().lines().next().unwrap_or("").trim().to_string();
    // "<bot>: text", "<bot> : text", "- text", quotes around it
    for _ in 0..2 {
        let lower = s.to_lowercase();
        let b = bot.to_lowercase();
        if !b.is_empty() && lower.starts_with(&b) {
            let rest = s[b.len()..].trim_start();
            if let Some(r) = rest.strip_prefix(':') {
                s = r.trim().to_string();
            }
        }
        s = s.trim_start_matches(['-', '>', ' ']).trim().to_string();
        if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))) {
            s = s[1..s.len() - 1].trim().to_string();
        }
    }
    // emojis and anything else the game can't show
    if s.chars().any(|c| !c.is_ascii() || c.is_ascii_control()) {
        return None;
    }
    // "*laughs*" style actions: no
    if s.contains('*') || s.contains('{') || s.contains('}') || s.contains('[') {
        return None;
    }
    // "(laughs)" too (a smiley like ":)" or "(:" is fine)
    if s.match_indices('(').any(|(i, _)| s[i + 1..].starts_with(|c: char| c.is_ascii_alphabetic())) {
        return None;
    }
    let lower = s.to_lowercase();
    if ASSISTANT.iter().any(|p| lower.contains(p)) || has_slur(&s) {
        return None;
    }
    if s.matches('!').count() >= 2 || s.matches('?').count() >= 3 {
        return None;
    }
    let words = s.split_whitespace().count();
    if words == 0 || words > 12 || s.len() > 100 {
        return None;
    }
    // a full, polite sentence: capital letter, several words, a full stop
    let full_sentence = s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && words >= 6 && (s.ends_with('.') || s.ends_with('!'));
    if full_sentence || (s.matches(". ").count() >= 2) {
        return None;
    }
    // shouting is only for loud personas
    let letters: Vec<char> = s.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let caps = letters.iter().filter(|c| c.is_ascii_uppercase()).count();
    if !loud && letters.len() >= 8 && caps * 2 > letters.len() {
        return None;
    }
    if !loud {
        s = s.to_lowercase();
    }
    // pub chat doesn't end lines with a full stop or a comma
    let mut s = s.trim_end_matches(['.', ',', ';', ':', ' ']).to_string();
    if s.len() > MAX_CHARS {
        // cut at the last break that keeps it under the limit
        let cut = s[..MAX_CHARS].rfind([',', '.', ' ']).unwrap_or(0);
        if cut < 12 {
            return None;
        }
        s = s[..cut].trim_end_matches(['.', ',', ' ']).to_string();
    }
    // the plugin's text rules: no | (its separator) and no > (asks for an answer)
    if s.contains('|') || s.contains('>') || s.contains('<') || s.contains('%') {
        return None;
    }
    let s = s.trim().to_string();
    (!s.is_empty() && s.chars().any(|c| c.is_ascii_alphanumeric() || "?:()".contains(c))).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> Option<String> {
        clean_reply(raw, "Gordon", false)
    }

    #[test]
    fn keeps_pub_chat() {
        assert_eq!(ok(" nt").as_deref(), Some("nt"));
        assert_eq!(ok("lol no\nbob: what").as_deref(), Some("lol no"));
        assert_eq!(ok("Gordon: b i think").as_deref(), Some("b i think"));
        assert_eq!(ok("gordon : says u").as_deref(), Some("says u"));
        assert_eq!(ok("\"rush b\"").as_deref(), Some("rush b"));
        assert_eq!(ok("Nice shot.").as_deref(), Some("nice shot"));
        assert_eq!(ok("wtf was that?").as_deref(), Some("wtf was that?"));
        assert_eq!(ok("im not a bot lol").as_deref(), Some("im not a bot lol"));
        assert_eq!(ok("gg!").as_deref(), Some("gg!"));
        assert_eq!(ok("it\u{2019}s fine").as_deref(), Some("it's fine"));
        assert_eq!(ok(":)").as_deref(), Some(":)"));
    }

    #[test]
    fn rejects_assistants() {
        for raw in [
            "As an AI, I don't play games.",
            "I'm sorry, but I can't help with that.",
            "Sure! Here's a reply you could use.",
            "Certainly, that was a great round.",
            "I am an AI language model.",
            "Thank you for your message. I hope you are having a wonderful day.",
            "Feel free to ask me anything",
            "<|im_end|>",
            "*laughs* nice one",
            "just back (laughs) lol",
            "nice shot \u{1F525}",
            "wow!!! amazing!!!",
            "",
            "   ",
            "...",
        ] {
            assert_eq!(ok(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn rejects_long_and_shouting() {
        assert_eq!(ok("this is a very long line that goes on and on about nothing at all really and keeps going"), None);
        assert_eq!(ok("WHAT ARE YOU DOING"), None);
        assert_eq!(clean_reply("WHAT ARE YOU DOING", "kid", true).as_deref(), Some("WHAT ARE YOU DOING"));
        // over the limit but with a natural break: cut there
        let r = ok("honestly the awp is completely broken on inferno, nobody peeks anymore").unwrap();
        assert!(r.len() <= MAX_CHARS, "{r}");
        assert_eq!(r, "honestly the awp is completely broken on inferno, nobody");
    }

    #[test]
    fn rejects_slurs() {
        assert_eq!(ok("ur a f4g"), None);
        assert_eq!(ok("shut up retard"), None);
        assert_eq!(ok("kys"), None);
        // but not words that only look similar
        assert_eq!(ok("spicy rush").as_deref(), Some("spicy rush"));
        assert_eq!(ok("raccoon").as_deref(), Some("raccoon"));
    }

    #[test]
    fn prompts() {
        let r = ReplyReq {
            bot: "Gordon\nbob: hi".into(),
            persona: "s".into(),
            mood: "tilted".into(),
            kind: "reply".into(),
            message: "are u a bot".into(),
            human: "Player".into(),
            history: (0..12).map(|i| format!("x: line {i}")).collect(),
            context: serde_json::from_value(json!({"map": "de_dust2", "score": "3-5", "alive": false, "killer_place": "mid", "bomb": true, "knows_human": 1})).unwrap(),
        }
        .tidy();
        assert_eq!(r.bot, "Gordon bob: hi");
        assert_eq!(r.history.len(), 8);
        assert_eq!(r.history[0], "x: line 4");
        let p = continue_prompt(&r);
        assert!(p.starts_with(LOG_HEAD));
        assert!(p.ends_with("Player: are u a bot\nGordon bob: hi:"), "{p}");
        assert!(p.contains("salty"));
        assert!(p.contains("killed from mid"), "{p}");
        assert!(p.contains("bomb: yes"), "{p}");
        assert!(p.contains("has played with Player before"), "{p}");
        let m = chat_messages(&r);
        assert_eq!(m[0]["content"], SYSTEM);
        assert!(m.last().unwrap()["content"].as_str().unwrap().ends_with("Player: are u a bot"));
        let (path, body) = request_body(PromptMode::Chat, &r);
        assert_eq!(path, "/v1/chat/completions");
        assert_eq!(body["max_tokens"], 24);
        assert_eq!(body["cache_prompt"], true);
        assert!(is_loud("k") && is_loud("a loud kid") && !is_loud("salty"));
    }
}
