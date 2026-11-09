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
    /// who the bot is, in a few words ("warmth warm, temper tilty, humor goofy, into wc3")
    pub traits: String,
    /// how it knows the person: "friend", "regular", "rival", "clanmate", "stranger"...
    pub relation: String,
    /// the last few kills and deaths involving either of them ("bob killed Gordon with ak")
    pub events: Vec<String>,
    /// what the bot may be asked to do (from ACTIONS): the model may pick one
    pub actions: Vec<String>,
    /// "Bots can be rude" is on: without it the bot teases at most (and anything insulting is
    /// dropped, so the written line is used)
    pub rude: bool,
    /// the lobby's mood: "toxic", "normal" or "friendly"
    pub lobby: String,
}

/// Everything a bot can be asked to do in chat; the plugin decides yes or no as it would
/// for a typed request.
pub const ACTIONS: &[&str] = &["follow", "hold", "come", "goto_a", "goto_b", "spray", "jump", "crouch", "drop"];

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
        self.traits = plain(&self.traits, 160);
        self.relation = plain(&self.relation, 24).to_lowercase();
        self.lobby = plain(&self.lobby, 12).to_lowercase();
        let n = self.events.len();
        self.events = self.events.split_off(n.saturating_sub(3)).iter().map(|l| plain(l, 80)).filter(|l| !l.is_empty()).collect();
        let mut acts: Vec<String> = Vec::new();
        for a in &self.actions {
            let a = a.trim().to_lowercase();
            if ACTIONS.contains(&a.as_str()) && !acts.contains(&a) {
                acts.push(a);
            }
        }
        self.actions = acts;
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
            if parts.len() >= 15 {
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
        if !self.traits.is_empty() {
            s.push_str(&format!(" ({})", self.traits));
        }
        match self.relation.as_str() {
            "" | "stranger" => {
                if self.knows_human() {
                    s.push_str(&format!(", has played with {} before", self.human));
                }
            }
            "friend" => s.push_str(&format!(", friends with {}", self.human)),
            "regular" => s.push_str(&format!(", knows {} from this server", self.human)),
            "rival" => s.push_str(&format!(", {} is their rival", self.human)),
            "clanmate" => s.push_str(&format!(", in a clan with {}", self.human)),
            "dislikes them" => s.push_str(&format!(", doesn't like {}", self.human)),
            r => s.push_str(&format!(", {r} of {}", self.human)),
        }
        match self.lobby.as_str() {
            "toxic" => s.push_str(", the server is toxic tonight"),
            "friendly" => s.push_str(", the server is friendly tonight"),
            _ => {}
        }
        if !self.rude {
            s.push_str(", never insults anyone");
        }
        if let Some(t) = self.topic() {
            s.push_str(&format!(", talking about {t}"));
        }
        s
    }
}

/// Context keys the plugin sends, in the order they matter, with a few words for each.
const KNOWN_CONTEXT: &[(&str, &str)] = &[
    ("mode", "mode"),
    ("phase", "round:"),
    ("alive_us", "teammates alive"),
    ("alive_them", "enemies alive"),
    ("hp", "health"),
    ("gun", "holding"),
    ("money", "money"),
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
        (&["u", "selfblame"], "quick to blame themselves"),
        (&["B", "brit"], "british"),
        (&["D", "dad", "parent"], "a parent who plays after the kids are in bed"),
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
    if !r.events.is_empty() {
        s.push_str(&format!("* just now: {}\n", r.events.join("; ")));
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

/// SYSTEM with "Bots can be rude" off: teasing at most.
pub const SYSTEM_NICE: &str = "Enter Roleplay Mode. You are a player on a Counter-Strike 1.6 public server in 2004, typing in the game's chat.\n\
Roleplaying Instructions:\n\
- answer with one short chat line, usually 1 to 6 words\n\
- lowercase, little punctuation, pub slang like nt, ns, gg, lol, rush b, ez, says u\n\
- you can be bored or joking, and tease at most: never insult or swear at anyone\n\
- never explain, never offer help, never say you are an ai\n\
- only write the line itself";

/// Example turns: the shape every request takes, answered the way a pub player would.
const EXAMPLES: &[(&str, &str)] = &[
    ("you are Wolf, chill. map de_dust2, score 3-5.\nkRaZy: nice try", "nt"),
    ("you are Gordon, salty, tilted. map de_dust2.\nbob: are u a bot", "lol no"),
    ("you are ZeRo, a tryhard. map de_inferno.\nbob: where they going", "b i think"),
    ("you are Mike, overconfident, cocky. map cs_office.\nfragger: ur bad", "says u"),
    ("you are dave, a parent, polite. map de_aztec.\nkid: sorry for tk", "np"),
];

fn chat_user(r: &ReplyReq) -> String {
    let mut s = format!("you are {}.", r.who().replacen(&format!("{} is ", r.bot), &format!("{}, ", r.bot), 1));
    let sit = r.situation();
    if !sit.is_empty() {
        s.push_str(&format!(" {sit}."));
    }
    if !r.events.is_empty() {
        s.push_str(&format!(" just now: {}.", r.events.join("; ")));
    }
    if !r.actions.is_empty() {
        s.push_str(&format!(" you can do: {}.", r.actions.join(", ")));
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

/// The system prompt when the bot may also do something: the answer is a little JSON object.
pub const SYSTEM_ACT: &str = "Enter Roleplay Mode. You are a player on a Counter-Strike 1.6 public server in 2004, typing in the game's chat.\n\
Roleplaying Instructions:\n\
- answer with one short chat line, usually 1 to 6 words\n\
- lowercase, little punctuation, pub slang like nt, ns, gg, lol, rush b, ez, says u\n\
- you can be rude, bored or joking, like real players\n\
- never explain, never offer help, never say you are an ai\n\
- if someone asks you to do something you can do, you may do it or refuse, as your character would\n\
- reply only with JSON: {\"say\": \"<your chat line>\", \"do\": \"<one thing you can do, or none>\"}";

/// SYSTEM_ACT with "Bots can be rude" off.
pub const SYSTEM_ACT_NICE: &str = "Enter Roleplay Mode. You are a player on a Counter-Strike 1.6 public server in 2004, typing in the game's chat.\n\
Roleplaying Instructions:\n\
- answer with one short chat line, usually 1 to 6 words\n\
- lowercase, little punctuation, pub slang like nt, ns, gg, lol, rush b, ez, says u\n\
- you can be bored or joking, and tease at most: never insult or swear at anyone\n\
- never explain, never offer help, never say you are an ai\n\
- if someone asks you to do something you can do, you may do it or refuse, as your character would\n\
- reply only with JSON: {\"say\": \"<your chat line>\", \"do\": \"<one thing you can do, or none>\"}";

/// The system prompt for a request: with or without actions, rude bots on or off.
pub fn system_prompt(act: bool, rude: bool) -> &'static str {
    match (act, rude) {
        (true, true) => SYSTEM_ACT,
        (true, false) => SYSTEM_ACT_NICE,
        (false, true) => SYSTEM,
        (false, false) => SYSTEM_NICE,
    }
}

/// Example turns for SYSTEM_ACT.
const EXAMPLES_ACT: &[(&str, &str)] = &[
    ("you are Wolf, chill. map de_dust2, score 3-5. you can do: follow, hold, come, goto_a, goto_b.\nkRaZy: wolf come with me", r#"{"say": "ok omw", "do": "follow"}"#),
    ("you are Gordon, salty, tilted. map de_dust2. you can do: follow, hold, come, goto_a, goto_b.\nbob: gordon stay here pls", r#"{"say": "no, we always lose waiting", "do": "none"}"#),
    ("you are ZeRo, a tryhard. map de_inferno. you can do: follow, hold, come, goto_a, goto_b.\nbob: what do u think", r#"{"say": "b split, go banana", "do": "goto_b"}"#),
    ("you are Mike, overconfident, cocky. map cs_office. you can do: follow, hold, come, jump.\nfragger: ur bad", r#"{"say": "says u", "do": "none"}"#),
];

pub fn chat_messages(r: &ReplyReq) -> Vec<Value> {
    let act = !r.actions.is_empty();
    let mut m = vec![json!({"role": "system", "content": system_prompt(act, r.rude)})];
    let examples: &[(&str, &str)] = if act { EXAMPLES_ACT } else { EXAMPLES };
    for (u, a) in examples {
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
        PromptMode::Chat => json!({"messages": chat_messages(r), "max_tokens": if r.actions.is_empty() { 24 } else { 48 },
                                   "stop": if r.actions.is_empty() { json!(["\n"]) } else { json!(["\n\n"]) },
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

// ---- what to do ---------------------------------------------------------------------

/// The chat line and the action in the model's answer. Asked for JSON ({"say", "do"}) it
/// may write that, JSON in a code fence, broken JSON, or just the line: anything that
/// isn't clearly an offered action is no action, and the text is whatever reads as the line.
pub fn split_action(raw: &str, offered: &[String]) -> (String, Option<String>) {
    // (a code fence around it: its lines go)
    let t: String = raw.lines().filter(|l| !l.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n");
    let t = t.trim();
    let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) else {
        return (t.to_string(), None);
    };
    if b <= a {
        return (t.to_string(), None);
    }
    let obj = &t[a..=b];
    let (say, act) = match serde_json::from_str::<Value>(obj) {
        Ok(Value::Object(m)) => {
            let get = |keys: &[&str]| keys.iter().find_map(|k| m.get(*k).and_then(|v| v.as_str()).map(String::from));
            (get(&["say", "text", "reply", "line"]), get(&["do", "action"]))
        }
        _ => (loose_field(obj, &["say", "text", "reply"]), loose_field(obj, &["do", "action"])),
    };
    let act = act.map(|a| a.trim().to_lowercase()).filter(|a| a != "none" && offered.iter().any(|o| o == a));
    match say {
        Some(s) => (s, act),
        // JSON without a line in it: nothing to say, so nothing to do either
        None => (String::new(), None),
    }
}

/// "key": "value" out of something that's nearly JSON (a missing brace, a trailing comma).
fn loose_field(s: &str, keys: &[&str]) -> Option<String> {
    for k in keys {
        let pat = format!("\"{k}\"");
        let Some(at) = s.find(&pat) else { continue };
        let rest = s[at + pat.len()..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    None
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

/// Insults and swearing: with "Bots can be rude" off a reply with one is dropped (the written
/// line is used). Stems match the start of a word.
const INSULT_STEMS: &[&str] = &["fuck", "fuk", "fck", "shit", "bitch", "cunt", "bastard", "wank", "twat", "prick", "piss", "idiot", "moron", "loser", "noob", "n00b", "kurw", "scheis"];
const INSULT_WORDS: &[&str] = &["ass", "asshole", "dumbass", "dick", "stfu", "gtfo", "stupid", "dumb", "nub", "newb", "trash", "garbage", "suck", "sucks", "sux", "suka", "blyat", "cyka", "merda", "l2p", "loser", "crap"];
const INSULT_PHRASES: &[&str] = &["ur bad", "you are bad", "ur so bad", "you suck", "u suck", "learn to play", "learn to aim", "shut up"];

pub fn has_insult(text: &str) -> bool {
    let t: String = text.to_lowercase().chars().map(unleet).collect();
    let words: Vec<&str> = t.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    let joined = format!(" {} ", words.join(" "));
    words.iter().any(|w| INSULT_WORDS.contains(w) || INSULT_STEMS.iter().any(|s| w.starts_with(s)))
        || INSULT_PHRASES.iter().any(|p| joined.contains(&format!(" {p} ")))
}

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
    fn actions() {
        let offered: Vec<String> = ["follow", "hold", "goto_b"].iter().map(|s| s.to_string()).collect();
        assert_eq!(split_action(r#"{"say": "ok omw", "do": "follow"}"#, &offered), ("ok omw".into(), Some("follow".into())));
        assert_eq!(split_action("```json\n{\"say\": \"k\", \"do\": \"HOLD\"}\n```", &offered), ("k".into(), Some("hold".into())));
        // not offered, made up, or none: no action
        assert_eq!(split_action(r#"{"say": "lol", "do": "jump"}"#, &offered).1, None);
        assert_eq!(split_action(r#"{"say": "lol", "do": "rm -rf"}"#, &offered).1, None);
        assert_eq!(split_action(r#"{"say": "nah", "do": "none"}"#, &offered), ("nah".into(), None));
        // broken JSON still gives what it can
        assert_eq!(split_action(r#"{"say": "b go", "do": "goto_b",}"#, &offered), ("b go".into(), Some("goto_b".into())));
        // plain text: the line, nothing to do
        assert_eq!(split_action("ok omw", &offered), ("ok omw".into(), None));
        assert_eq!(split_action("```\nok omw\n```", &offered), ("ok omw".into(), None));
        // JSON with no line: nothing at all
        assert_eq!(split_action(r#"{"do": "follow"}"#, &offered), (String::new(), None));
        // the line still goes through the filter
        let (say, _) = split_action(r#"{"say": "As an AI I cannot follow", "do": "follow"}"#, &offered);
        assert_eq!(clean_reply(&say, "Gordon", false), None);
    }

    #[test]
    fn insults() {
        for t in ["stfu", "ur bad lol", "u suck", "noob", "n00b lol", "fuck off", "kurwa", "shut up kid", "ur trash", "learn to aim"] {
            assert!(has_insult(t), "{t}");
        }
        for t in ["nt", "lol no", "says u", "nice try", "rush b", "u wish", "class act", "passing by", "gg", "bad luck", "im so bad today"] {
            assert!(!has_insult(t), "{t}");
        }
        assert!(!SYSTEM_NICE.contains("rude") && SYSTEM_NICE.contains("never insult"));
        assert!(SYSTEM_ACT_NICE.contains("never insult") && SYSTEM_ACT_NICE.contains("JSON"));
    }

    #[test]
    fn rich_context() {
        let r = ReplyReq {
            bot: "Gordon".into(),
            persona: "s".into(),
            message: "gordon come with me".into(),
            human: "Player".into(),
            traits: "warmth cold, humor dry, into wc3".into(),
            relation: "Rival".into(),
            events: (0..5).map(|i| format!("Player killed Gordon {i}")).collect(),
            actions: vec!["follow".into(), "Hold".into(), "fly".into(), "follow".into()],
            context: serde_json::from_value(json!({"map": "de_dust2", "mode": "classic", "phase": "live", "alive_us": 3, "alive_them": 2, "hp": 64, "gun": "ak", "money": 3200})).unwrap(),
            ..Default::default()
        }
        .tidy();
        assert_eq!(r.actions, vec!["follow".to_string(), "hold".to_string()]);
        assert_eq!(r.events.len(), 3);
        assert_eq!(r.relation, "rival");
        let m = chat_messages(&r);
        assert_eq!(m[0]["content"], SYSTEM_ACT_NICE);
        let u = m.last().unwrap()["content"].as_str().unwrap().to_string();
        assert!(u.contains("Player is their rival"), "{u}");
        assert!(u.contains("warmth cold"), "{u}");
        assert!(u.contains("health 64") && u.contains("holding ak") && u.contains("enemies alive 2"), "{u}");
        assert!(u.contains("just now: Player killed Gordon 2; Player killed Gordon 3; Player killed Gordon 4."), "{u}");
        assert!(u.contains("you can do: follow, hold"), "{u}");
        let (_, body) = request_body(PromptMode::Chat, &r);
        assert_eq!(body["max_tokens"], 48);
        // no actions: the plain prompt
        let plain_req = ReplyReq { actions: vec![], ..r.clone() };
        assert_eq!(chat_messages(&plain_req)[0]["content"], SYSTEM_NICE);
        let rude_req = ReplyReq { rude: true, ..r.clone() };
        assert_eq!(chat_messages(&rude_req)[0]["content"], SYSTEM_ACT);
        assert!(u.contains("never insults anyone"), "{u}");
        assert!(!chat_messages(&rude_req).last().unwrap()["content"].as_str().unwrap().contains("never insults"));
        assert!(continue_prompt(&r).contains("* just now: "));
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
            ..Default::default()
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
        assert_eq!(m[0]["content"], SYSTEM_NICE);
        assert!(m.last().unwrap()["content"].as_str().unwrap().ends_with("Player: are u a bot"));
        let (path, body) = request_body(PromptMode::Chat, &r);
        assert_eq!(path, "/v1/chat/completions");
        assert_eq!(body["max_tokens"], 24);
        assert_eq!(body["cache_prompt"], true);
        assert!(is_loud("k") && is_loud("a loud kid") && !is_loud("salty"));
    }
}
