//! The entity lump: `{ "key" "value" ... }` blocks, worldspawn first.

use serde_json::{json, Map, Value};

/// One entity: its keys in file order (a repeated key: the last one counts, as in the game).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entity {
    pub keys: Vec<(String, String)>,
}

impl Entity {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.keys.iter().rev().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
    }

    pub fn classname(&self) -> &str {
        self.get("classname").unwrap_or("")
    }

    /// "x y z" as numbers.
    pub fn vec3(&self, k: &str) -> Option<[f32; 3]> {
        let mut it = self.get(k)?.split_whitespace().map(|s| s.parse::<f32>().ok());
        let v = [it.next()??, it.next()??, it.next()??];
        v.iter().all(|f| f.is_finite()).then_some(v)
    }

    pub fn origin(&self) -> Option<[f32; 3]> {
        self.vec3("origin")
    }

    /// pitch yaw roll, from "angles", else "angle" (the yaw alone; -1 up, -2 down are left
    /// as they are for the page to read).
    pub fn angles(&self) -> Option<[f32; 3]> {
        self.vec3("angles").or_else(|| {
            let a = self.get("angle")?.trim().parse::<f32>().ok().filter(|a| a.is_finite())?;
            Some([0.0, a, 0.0])
        })
    }

    /// The brush model it uses ("*N" → N).
    pub fn brush_model(&self) -> Option<usize> {
        self.get("model")?.strip_prefix('*')?.parse().ok()
    }
}

/// Splits the entity text into entities. Tolerant: stray text between blocks is skipped, an
/// unclosed last block still counts, quotes may hold anything but a quote.
pub fn parse(text: &str) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut cur: Option<Entity> = None;
    let mut pending: Option<String> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' => {
                pending = None;
                if let Some(e) = cur.take() {
                    out.push(e);
                }
                cur = Some(Entity::default());
            }
            '}' => {
                pending = None;
                if let Some(e) = cur.take() {
                    out.push(e);
                }
            }
            '"' => {
                let start = i + 1;
                let mut end = text.len();
                for (j, d) in chars.by_ref() {
                    if d == '"' {
                        end = j;
                        break;
                    }
                }
                let s = text[start..end].to_string();
                match pending.take() {
                    None => pending = Some(s),
                    Some(k) => {
                        if let Some(e) = cur.as_mut() {
                            e.keys.push((k, s));
                        }
                    }
                }
            }
            _ => {}
        }
        if out.len() > 1 << 20 {
            break;
        }
    }
    if let Some(e) = cur {
        out.push(e);
    }
    out
}

/// The texture files worldspawn names ("wad" key, `;`-separated, full Windows paths), as
/// base names only, in order, without repeats.
pub fn wad_list(ents: &[Entity]) -> Vec<String> {
    let Some(w) = ents.first().filter(|e| e.classname() == "worldspawn").and_then(|e| e.get("wad")) else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for part in w.split(';') {
        let base = part.replace('\\', "/").rsplit('/').next().unwrap_or("").trim().to_string();
        if base.to_lowercase().ends_with(".wad") && base.len() <= 64 && !out.iter().any(|o| o.eq_ignore_ascii_case(&base)) {
            out.push(base);
        }
    }
    out
}

/// The entity list the page shows: `[{index, classname, origin?, angles?, model?, keys{}}]`.
pub fn to_json(ents: &[Entity]) -> Value {
    Value::Array(
        ents.iter()
            .enumerate()
            .map(|(i, e)| {
                let keys: Map<String, Value> = e.keys.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
                let mut o = json!({"index": i, "classname": e.classname(), "keys": keys});
                if let Some(p) = e.origin() {
                    o["origin"] = json!(p);
                }
                if let Some(a) = e.angles() {
                    o["angles"] = json!(a);
                }
                if let Some(m) = e.get("model") {
                    o["model"] = json!(m);
                }
                o
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_fixture() {
        let e = parse(crate::goldsrc::bsp::fixture::ENTITIES);
        assert_eq!(e.len(), 4);
        assert_eq!(e[0].classname(), "worldspawn");
        assert_eq!(e[0].get("message"), Some("Test Room"));
        assert_eq!(wad_list(&e), vec!["test.wad".to_string(), "missing.wad".to_string()]);
        assert_eq!(e[1].brush_model(), Some(1));
        assert_eq!(e[2].origin(), Some([0.0, 0.0, -92.0]));
        assert_eq!(e[2].angles(), Some([0.0, 90.0, 0.0]));
        assert_eq!(e[3].angles(), Some([0.0, 180.0, 0.0]));
        let j = to_json(&e);
        assert_eq!(j[1]["model"], "*1");
        assert_eq!(j[2]["origin"], json!([0.0, 0.0, -92.0]));
        assert!(j[0].get("origin").is_none());
        assert_eq!(j[0]["keys"]["skyname"], "desert");
    }

    #[test]
    fn tolerates_junk() {
        let e = parse("garbage { \"a\" \"1\" \"a\" \"2\" \"lonely\" } {\"b\" \"x}y{z\"");
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].get("a"), Some("2"));
        assert_eq!(e[1].get("b"), Some("x}y{z"));
        assert!(parse("\"unterminated").is_empty());
        assert!(parse("").is_empty());
        let e = parse("{\"origin\" \"1 nan 3\" \"angles\" \"1 2\"}");
        assert_eq!(e[0].origin(), None);
        assert_eq!(e[0].angles(), None);
    }

    #[test]
    fn wad_names_are_base_names() {
        let e = parse("{\"classname\" \"worldspawn\" \"wad\" \"c:/a/b/x.WAD;..\\\\..\\\\y.wad; ;notawad;\\\\q\\\\X.wad\"}");
        assert_eq!(wad_list(&e), vec!["x.WAD".to_string(), "y.wad".to_string()]);
    }
}
