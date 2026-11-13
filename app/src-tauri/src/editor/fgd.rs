//! Entity definitions (the FGD format Hammer, J.A.C.K. and TrenchBroom read): which keys an
//! entity has, their types, choices, flags and help, so the inspector can show real controls.
//! The editor ships its own Counter-Strike definitions (`cstrike.fgd`, written for
//! Loner-Strike: no Valve or third-party FGD text is copied); the parser reads any standard
//! FGD, so a mapper's own file can be used too. Never panics; errors name the line.

use serde::Serialize;
use std::collections::HashMap;

/// Our Counter-Strike 1.6 entity definitions.
pub const CSTRIKE_FGD: &str = include_str!("cstrike.fgd");
/// More definition files read after ours (they can use our base classes), e.g. the game mode
/// components: add `include_str!("lonerstrike.fgd")` here when it lands.
pub const EXTRA_FGDS: &[&str] = &[include_str!("lonerstrike.fgd")];

/// Every definition the editor ships, as one FGD text.
pub fn shipped() -> String {
    let mut t = CSTRIKE_FGD.to_string();
    for x in EXTRA_FGDS {
        t.push('\n');
        t.push_str(x);
    }
    t
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Flag {
    pub bit: u32,
    pub label: String,
    pub on: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Key {
    pub name: String,
    /// string, integer, float, choices, flags, color255, color1, target_source,
    /// target_destination, studio, sprite, sound, decal, angle, ...
    #[serde(rename = "type")]
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub help: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<Flag>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Class {
    pub name: String,
    /// "point", "solid" or "base"
    pub kind: String,
    pub description: String,
    /// Box size for point entities (min, max), when given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 6]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    /// Keys, with every base class's keys first.
    pub keys: Vec<Key>,
}

// ---- lexer ------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Punct(char),
}

fn lex(text: &str) -> Result<Vec<(Tok, usize)>, String> {
    let mut out = Vec::new();
    let b: Vec<char> = text.chars().collect();
    let (mut i, mut line) = (0usize, 1usize);
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '"' {
            let start = line;
            let mut s = String::new();
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\n' {
                    line += 1;
                }
                s.push(b[i]);
                i += 1;
            }
            if i >= b.len() {
                return Err(format!("Line {start}: a quote is never closed."));
            }
            i += 1;
            out.push((Tok::Str(s), start));
        } else if "[](),:=+@".contains(c) {
            out.push((Tok::Punct(c), line));
            i += 1;
        } else {
            let mut s = String::new();
            while i < b.len() && !b[i].is_whitespace() && !"[](),:=+\"".contains(b[i]) {
                s.push(b[i]);
                i += 1;
            }
            out.push((Tok::Word(s), line));
        }
        if out.len() > 2_000_000 {
            return Err("That entity definition file is too big.".into());
        }
    }
    Ok(out)
}

struct P {
    t: Vec<(Tok, usize)>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i).map(|x| &x.0)
    }
    fn line(&self) -> usize {
        self.t.get(self.i).or(self.t.last()).map_or(0, |x| x.1)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).map(|x| x.0.clone());
        self.i += 1;
        t
    }
    fn is(&self, c: char) -> bool {
        self.peek() == Some(&Tok::Punct(c))
    }
    fn eat(&mut self, c: char) -> bool {
        let y = self.is(c);
        if y {
            self.i += 1;
        }
        y
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!("Line {}: expected \"{c}\".", self.line()))
        }
    }
    /// A string, with `"a" + "b"` joined.
    fn string(&mut self) -> Result<String, String> {
        let mut s = match self.next() {
            Some(Tok::Str(s)) => s,
            _ => return Err(format!("Line {}: expected some text in quotes.", self.line())),
        };
        while self.is('+') {
            self.i += 1;
            match self.next() {
                Some(Tok::Str(x)) => s.push_str(&x),
                _ => return Err(format!("Line {}: expected text after \"+\".", self.line())),
            }
        }
        Ok(s)
    }
    /// A default value: a word, a number (maybe negative) or a string.
    fn value(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Tok::Str(s)) => Ok(s),
            Some(Tok::Word(w)) => Ok(w),
            _ => Err(format!("Line {}: expected a value.", self.line())),
        }
    }
    /// Skip a balanced `( ... )`.
    fn skip_parens(&mut self) -> Result<Vec<String>, String> {
        self.expect('(')?;
        let mut depth = 1;
        let mut words = Vec::new();
        while depth > 0 {
            match self.next() {
                Some(Tok::Punct('(')) => depth += 1,
                Some(Tok::Punct(')')) => depth -= 1,
                Some(Tok::Word(w)) | Some(Tok::Str(w)) => words.push(w),
                Some(_) => {}
                None => return Err("A bracket is never closed.".into()),
            }
        }
        Ok(words)
    }
}

// ---- parser -----------------------------------------------------------------------------

struct Raw {
    class: Class,
    bases: Vec<String>,
}

fn parse_key(p: &mut P, name: String) -> Result<Key, String> {
    p.expect('(')?;
    let kind = match p.next() {
        Some(Tok::Word(w)) => w.to_ascii_lowercase(),
        _ => return Err(format!("Line {}: expected the key's type.", p.line())),
    };
    p.expect(')')?;
    // some FGDs mark keys `readonly` or `report` after the type
    while let Some(Tok::Word(w)) = p.peek() {
        if matches!(w.as_str(), "readonly" | "report") {
            p.i += 1;
        } else {
            break;
        }
    }
    let mut k = Key { name, kind, label: String::new(), default: String::new(), help: String::new(), choices: vec![], flags: vec![] };
    if p.eat(':') {
        if matches!(p.peek(), Some(Tok::Str(_))) {
            k.label = p.string()?;
        }
        if p.eat(':') {
            if !p.is(':') && !p.is('=') && !matches!(p.peek(), Some(Tok::Punct(_))) {
                k.default = p.value()?;
            }
            if p.eat(':') && matches!(p.peek(), Some(Tok::Str(_))) {
                k.help = p.string()?;
            }
        }
    }
    if p.eat('=') {
        p.expect('[')?;
        while !p.eat(']') {
            let v = p.value()?;
            p.expect(':')?;
            let label = p.string()?;
            if k.kind == "flags" {
                let mut on = false;
                if p.eat(':') {
                    on = p.value()? != "0";
                }
                if p.eat(':') {
                    let _ = p.string();
                }
                let bit = v.parse::<u32>().map_err(|_| format!("Line {}: a flag must be a number.", p.line()))?;
                k.flags.push(Flag { bit, label, on });
            } else {
                if p.eat(':') {
                    let _ = p.string();
                }
                k.choices.push(Choice { value: v, label });
            }
            if p.i > p.t.len() {
                return Err("A list of choices is never closed.".into());
            }
        }
    }
    if k.label.is_empty() {
        k.label = k.name.clone();
    }
    Ok(k)
}

fn nums(words: &[String]) -> Vec<f64> {
    words.iter().filter_map(|w| w.parse::<f64>().ok()).collect()
}

fn parse_class(p: &mut P, kind: &str) -> Result<Raw, String> {
    let mut bases = Vec::new();
    let (mut size, mut color) = (None, None);
    // attributes up to "="
    loop {
        match p.peek() {
            Some(Tok::Punct('=')) => {
                p.i += 1;
                break;
            }
            Some(Tok::Word(_)) => {
                let Some(Tok::Word(w)) = p.next() else { unreachable!() };
                if p.is('(') {
                    let args = p.skip_parens()?;
                    match w.to_ascii_lowercase().as_str() {
                        "base" => bases.extend(args),
                        "size" => {
                            let n = nums(&args);
                            size = match n.len() {
                                6 => Some([n[0], n[1], n[2], n[3], n[4], n[5]]),
                                3 => Some([-n[0] / 2.0, -n[1] / 2.0, -n[2] / 2.0, n[0] / 2.0, n[1] / 2.0, n[2] / 2.0]),
                                _ => None,
                            }
                        }
                        "color" => {
                            let n = nums(&args);
                            if n.len() == 3 {
                                color = Some([n[0] as u8, n[1] as u8, n[2] as u8]);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => return Err(format!("Line {}: expected \"=\" and a class name.", p.line())),
        }
    }
    let name = match p.next() {
        Some(Tok::Word(w)) => w,
        _ => return Err(format!("Line {}: expected a class name.", p.line())),
    };
    let mut description = String::new();
    if p.eat(':') {
        description = p.string()?;
    }
    p.expect('[')?;
    let mut keys = Vec::new();
    while !p.eat(']') {
        match p.next() {
            Some(Tok::Word(k)) => keys.push(parse_key(p, k)?),
            None => return Err(format!("{name}: its list of keys is never closed.")),
            _ => return Err(format!("Line {}: expected a key name.", p.line())),
        }
    }
    Ok(Raw { class: Class { name, kind: kind.into(), description, size, color, keys }, bases })
}

/// Read an FGD. Base classes are folded in (their keys first, a class's own key wins);
/// base classes themselves aren't returned. Classes come back sorted by name.
pub fn parse(text: &str) -> Result<Vec<Class>, String> {
    let mut p = P { t: lex(text)?, i: 0 };
    let mut raw: Vec<Raw> = Vec::new();
    while p.peek().is_some() {
        if !p.eat('@') {
            return Err(format!("Line {}: expected an @ class.", p.line()));
        }
        let what = match p.next() {
            Some(Tok::Word(w)) => w.to_ascii_lowercase(),
            _ => return Err(format!("Line {}: expected a class type after @.", p.line())),
        };
        match what.as_str() {
            "baseclass" => raw.push(parse_class(&mut p, "base")?),
            "pointclass" | "npcclass" | "keyframeclass" | "moveclass" | "filterclass" => raw.push(parse_class(&mut p, "point")?),
            "solidclass" => raw.push(parse_class(&mut p, "solid")?),
            // @include, @mapsize and the like: skip their arguments
            _ => {
                if p.is('(') {
                    p.skip_parens()?;
                } else if matches!(p.peek(), Some(Tok::Str(_))) {
                    p.next();
                }
            }
        }
    }
    let by_name: HashMap<String, usize> = raw.iter().enumerate().map(|(i, r)| (r.class.name.to_ascii_lowercase(), i)).collect();
    fn resolve(i: usize, raw: &[Raw], by_name: &HashMap<String, usize>, depth: usize, out: &mut Vec<Key>, size: &mut Option<[f64; 6]>, color: &mut Option<[u8; 3]>) {
        if depth > 16 {
            return;
        }
        for b in &raw[i].bases {
            if let Some(&j) = by_name.get(&b.to_ascii_lowercase()) {
                resolve(j, raw, by_name, depth + 1, out, size, color);
            }
        }
        for k in &raw[i].class.keys {
            match out.iter_mut().find(|x| x.name == k.name) {
                Some(x) => *x = k.clone(),
                None => out.push(k.clone()),
            }
        }
        if raw[i].class.size.is_some() {
            *size = raw[i].class.size;
        }
        if raw[i].class.color.is_some() {
            *color = raw[i].class.color;
        }
    }
    let mut out: Vec<Class> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        if r.class.kind == "base" {
            continue;
        }
        let (mut keys, mut size, mut color) = (Vec::new(), None, None);
        resolve(i, &raw, &by_name, 0, &mut keys, &mut size, &mut color);
        out.push(Class { keys, size, color, ..r.class.clone() });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_counter_strike_definitions_read() {
        let c = parse(&shipped()).unwrap();
        assert!(c.len() > 50, "{}", c.len());
        let find = |n: &str| c.iter().find(|x| x.name == n).unwrap_or_else(|| panic!("{n}"));
        let spawn = find("info_player_start");
        assert_eq!(spawn.kind, "point");
        assert!(spawn.keys.iter().any(|k| k.name == "angle"));
        assert_eq!(spawn.size, Some([-16.0, -16.0, -36.0, 16.0, 16.0, 36.0]));
        let light = find("light");
        let l = light.keys.iter().find(|k| k.name == "_light").unwrap();
        assert_eq!(l.kind, "color255");
        assert!(!l.help.is_empty());
        let door = find("func_door");
        assert_eq!(door.kind, "solid");
        let flags = door.keys.iter().find(|k| k.name == "spawnflags").unwrap();
        assert!(flags.flags.iter().any(|f| f.bit == 1));
        let rm = door.keys.iter().find(|k| k.name == "rendermode").unwrap();
        assert!(rm.choices.len() >= 5);
        assert!(door.keys.iter().any(|k| k.name == "targetname" && k.kind == "target_source"));
        let arm = find("armoury_entity");
        assert!(arm.keys.iter().find(|k| k.name == "item").unwrap().choices.len() >= 20);
        for cls in &c {
            assert!(!cls.description.is_empty(), "{} has no description", cls.name);
        }
        assert!(c.iter().all(|x| x.kind != "base"));
    }

    #[test]
    fn standard_fgd_bits() {
        let text = r#"
            // a comment
            @BaseClass = Targetname [ targetname(target_source) : "Name" ]
            @BaseClass size(-8 -8 -8, 8 8 8) color(255 0 0) = Small []
            @PointClass base(Targetname, Small) studio("models/x.mdl") = thing : "A " + "thing"
            [
                health(integer) : "Health" : 10 : "How tough."
                mode(choices) : "Mode" : 1 =
                [
                    0 : "Off"
                    1 : "On"
                ]
                spawnflags(flags) =
                [
                    1 : "Starts on" : 1
                    4 : "Silent" : 0
                ]
                targetname(target_source) : "Its name (own)"
            ]
            @SolidClass = func_x : "X" []
            @mapsize(-4096, 4096)
        "#;
        let c = parse(text).unwrap();
        assert_eq!(c.len(), 2);
        let t = &c[1];
        assert_eq!(t.name, "thing");
        assert_eq!(t.description, "A thing");
        assert_eq!(t.size, Some([-8.0, -8.0, -8.0, 8.0, 8.0, 8.0]));
        assert_eq!(t.color, Some([255, 0, 0]));
        assert_eq!(t.keys[0].name, "targetname");
        assert_eq!(t.keys[0].label, "Its name (own)");
        let h = t.keys.iter().find(|k| k.name == "health").unwrap();
        assert_eq!((h.default.as_str(), h.help.as_str()), ("10", "How tough."));
        assert_eq!(t.keys.iter().find(|k| k.name == "mode").unwrap().choices.len(), 2);
        let f = &t.keys.iter().find(|k| k.name == "spawnflags").unwrap().flags;
        assert_eq!(f, &vec![Flag { bit: 1, label: "Starts on".into(), on: true }, Flag { bit: 4, label: "Silent".into(), on: false }]);
    }

    #[test]
    fn broken_files_say_where() {
        assert!(parse("@PointClass = x : \"never closed").unwrap_err().contains("Line 1"));
        assert!(parse("@PointClass = x [ a(string) ").is_err());
        assert!(parse("garbage").is_err());
        assert!(parse("@PointClass = x [ spawnflags(flags) = [ a : \"b\" ] ]").is_err());
        assert_eq!(parse("").unwrap().len(), 0);
    }
}
