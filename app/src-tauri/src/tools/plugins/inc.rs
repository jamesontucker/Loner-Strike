//! Reads AMX Mod X include files (`*.inc`) into the Plugin Studio's word list: natives,
//! forwards, stocks, #defines and constants, each with its signature and doc comment.
//! It's a forgiving reader, not a compiler: anything it doesn't understand is skipped,
//! and it never panics on odd input.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Param {
    /// as written ("const name[]", "&Float:x", "len = 0")
    pub label: String,
    /// just the name ("name", "x", "len", "...")
    pub name: String,
    pub doc: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    /// native, forward, stock, define, macro, constant, enum
    pub kind: &'static str,
    /// the declaration, on one line ("native get_user_name(index, name[], len)")
    pub detail: String,
    pub params: Vec<Param>,
    /// the return tag ("Float", "bool"), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// the description from the doc comment
    pub doc: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub returns: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// constants and defines: the value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// the include's file name ("amxmodx.inc")
    pub file: String,
    pub line: usize,
}

/// A doc comment, split into its parts.
#[derive(Debug, Default, Clone)]
struct Doc {
    text: String,
    params: HashMap<String, String>,
    returns: String,
    notes: Vec<String>,
    error: String,
    deprecated: Option<String>,
}

fn parse_doc(raw: &str) -> Doc {
    let body = raw.trim();
    let body = body.strip_prefix("/**<").or_else(|| body.strip_prefix("/**")).or_else(|| body.strip_prefix("/*")).or_else(|| body.strip_prefix("//")).unwrap_or(body);
    let body = body.strip_suffix("*/").unwrap_or(body);
    let mut doc = Doc::default();
    // (tag, name, text) of the section being read
    let mut cur: Option<(String, String, String)> = None;
    let mut desc: Vec<String> = Vec::new();
    let flush = |doc: &mut Doc, cur: &mut Option<(String, String, String)>| {
        if let Some((tag, name, text)) = cur.take() {
            let text = text.trim().to_string();
            match tag.as_str() {
                "param" => {
                    doc.params.insert(name, text);
                }
                "return" | "returns" => doc.returns = text,
                "note" => doc.notes.push(text),
                "error" => doc.error = text,
                "deprecated" => doc.deprecated = Some(text),
                _ => {}
            }
        }
    };
    for line in body.lines() {
        let l = line.trim();
        let l = l.strip_prefix("//").unwrap_or(l);
        let l = l.trim_start_matches('*');
        let l = l.strip_prefix(' ').unwrap_or(l);
        let t = l.trim();
        if let Some(rest) = t.strip_prefix('@') {
            flush(&mut doc, &mut cur);
            let (tag, rest) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            let tag = tag.to_lowercase();
            let rest = rest.trim();
            if tag == "param" {
                let (name, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                cur = Some((tag, name.trim().to_string(), text.trim().to_string()));
            } else {
                cur = Some((tag, String::new(), rest.to_string()));
            }
        } else if let Some((_, _, text)) = cur.as_mut() {
            if !t.is_empty() {
                text.push(' ');
                text.push_str(t);
            }
        } else {
            desc.push(l.trim_end().to_string());
        }
    }
    flush(&mut doc, &mut cur);
    // paragraphs: blank lines kept (once), the rest joined
    let mut out = String::new();
    let mut blank = false;
    for l in desc {
        if l.trim().is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if blank {
            out.push_str("\n\n");
        } else if !out.is_empty() {
            out.push(' ');
        }
        blank = false;
        out.push_str(l.trim());
    }
    doc.text = out;
    doc
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '@'
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '@'
}

/// Collapses whitespace and drops comments, on one line.
fn one_line(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i < c.len() && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        if c[i] == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c[i] == '"' || c[i] == '\'' {
            let q = c[i];
            out.push(q);
            i += 1;
            while i < c.len() && c[i] != q {
                if (c[i] == '^' || c[i] == '\\') && i + 1 < c.len() {
                    out.push(c[i]);
                    i += 1;
                }
                out.push(c[i]);
                i += 1;
            }
            if i < c.len() {
                out.push(q);
            }
            i += 1;
            continue;
        }
        if c[i].is_whitespace() {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
        } else {
            out.push(c[i]);
        }
        i += 1;
    }
    let out = out.trim().replace("( ", "(").replace(" )", ")").replace(" ,", ",");
    out
}

/// Splits on commas that aren't inside brackets or quotes.
fn split_top(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut esc = false;
    for c in s.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if esc {
                esc = false;
            } else if c == '^' || c == '\\' {
                esc = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut cur).trim().to_string());
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// A parameter's name: "const Float:origin[3] = {0.0, 0.0, 0.0}" -> "origin".
fn param_name(label: &str) -> String {
    if label.contains("...") {
        return "...".into();
    }
    let before_default = split_eq(label);
    // drop array sizes
    let mut s = String::new();
    let mut depth = 0;
    for c in before_default.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => s.push(c),
            _ => {}
        }
    }
    // the last identifier that isn't followed by ':' (a tag)
    let s = s.trim();
    let after_tag = s.rsplit(':').next().unwrap_or(s);
    after_tag.split(|c: char| !is_ident(c)).filter(|w| !w.is_empty()).last().unwrap_or("").to_string()
}

/// The part before a top-level '=' (a default value).
fn split_eq(s: &str) -> &str {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '=' if depth == 0 => return s[..i].trim(),
            _ => {}
        }
    }
    s.trim()
}

/// A function-like declaration ("native Float:foo(a, b[])"), parsed.
fn parse_function(decl: &str, kind: &'static str, doc: &Doc, file: &str, line: usize) -> Option<Symbol> {
    let decl = one_line(decl);
    let open = decl.find('(')?;
    let mut head = decl[..open].trim().to_string();
    for kw in ["native", "forward", "stock", "static", "public", "stock"] {
        if let Some(rest) = head.strip_prefix(kw) {
            if rest.starts_with(' ') {
                head = rest.trim().to_string();
            }
        }
    }
    let (tag, name) = match head.rsplit_once(':') {
        Some((t, n)) => (Some(t.trim().to_string()), n.trim().to_string()),
        None => (None, head.clone()),
    };
    if name.is_empty() || !name.chars().next().is_some_and(is_ident_start) || !name.chars().all(is_ident) {
        return None;
    }
    // the matching ')'
    let mut depth = 0;
    let mut close = None;
    for (i, c) in decl[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let params: Vec<Param> = split_top(&decl[open + 1..close])
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|label| {
            let name = param_name(&label);
            let doc = doc.params.get(&name).cloned().unwrap_or_default();
            Param { label, name, doc }
        })
        .collect();
    let tag = tag.filter(|t| t != "_" && !t.is_empty());
    let detail = format!("{kind} {}", &decl[decl.find(&name).unwrap_or(0)..=close]);
    let detail = match &tag {
        Some(t) => format!("{kind} {t}:{}", detail.strip_prefix(&format!("{kind} ")).unwrap_or(&detail)),
        None => detail,
    };
    Some(Symbol {
        name,
        kind,
        detail,
        params,
        tag,
        doc: doc.text.clone(),
        returns: doc.returns.clone(),
        notes: doc.notes.clone(),
        error: doc.error.clone(),
        deprecated: doc.deprecated.clone(),
        value: None,
        file: file.into(),
        line,
    })
}

fn constant(name: &str, value: Option<String>, kind: &'static str, doc: &Doc, file: &str, line: usize) -> Symbol {
    let detail = match &value {
        Some(v) if kind == "define" => format!("#define {name} {v}"),
        Some(v) => format!("{name} = {v}"),
        None => name.to_string(),
    };
    Symbol {
        name: name.into(),
        kind,
        detail,
        params: vec![],
        tag: None,
        doc: doc.text.clone(),
        returns: String::new(),
        notes: doc.notes.clone(),
        error: String::new(),
        deprecated: doc.deprecated.clone(),
        value,
        file: file.into(),
        line,
    }
}

struct Scanner<'a> {
    c: &'a [char],
    i: usize,
    line: usize,
}

impl Scanner<'_> {
    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }
    fn starts(&self, s: &str) -> bool {
        s.chars().enumerate().all(|(k, ch)| self.peek(k) == Some(ch))
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek(0)?;
        if ch == '\n' {
            self.line += 1;
        }
        self.i += 1;
        Some(ch)
    }
    /// Skips a "/* */" comment and returns its text.
    fn block_comment(&mut self) -> String {
        let start = self.i;
        self.i += 2;
        while self.i < self.c.len() && !self.starts("*/") {
            self.bump();
        }
        self.i = (self.i + 2).min(self.c.len());
        self.c[start..self.i].iter().collect()
    }
    fn line_comment(&mut self) -> String {
        let start = self.i;
        while self.i < self.c.len() && self.c[self.i] != '\n' {
            self.i += 1;
        }
        self.c[start..self.i].iter().collect()
    }
    fn string(&mut self) {
        let q = self.c[self.i];
        self.i += 1;
        while let Some(ch) = self.peek(0) {
            if ch == '\n' {
                return; // unterminated: stop at the line's end
            }
            self.i += 1;
            if ch == '^' || ch == '\\' {
                self.i += 1;
            } else if ch == q {
                return;
            }
        }
    }
    fn word(&mut self) -> String {
        let start = self.i;
        while self.peek(0).is_some_and(is_ident) {
            self.i += 1;
        }
        self.c[start..self.i].iter().collect()
    }
    /// A preprocessor line, with backslash continuations.
    fn directive(&mut self) -> String {
        let mut s = String::new();
        while let Some(ch) = self.peek(0) {
            if ch == '\n' {
                if s.trim_end().ends_with('\\') {
                    let t = s.trim_end().trim_end_matches('\\').to_string();
                    s = t;
                    s.push(' ');
                    self.bump();
                    continue;
                }
                break;
            }
            if ch == '/' && self.peek(1) == Some('*') {
                // a comment that may run past the line end belongs to the directive
                s.push_str(&self.block_comment());
                continue;
            }
            s.push(ch);
            self.i += 1;
        }
        s
    }
    /// Reads a declaration up to `;` (or `{` when `brace_ends`) at bracket depth 0, without
    /// consuming the `{`. A declaration with no `;` ends at the line after its `)`.
    fn declaration(&mut self, brace_ends: bool) -> String {
        let start = self.i;
        let mut depth = 0i32;
        let mut closed = false;
        while let Some(ch) = self.peek(0) {
            match ch {
                '/' if self.peek(1) == Some('*') => {
                    self.block_comment();
                    continue;
                }
                '/' if self.peek(1) == Some('/') => {
                    self.line_comment();
                    continue;
                }
                '"' | '\'' => {
                    self.string();
                    continue;
                }
                '(' | '[' => depth += 1,
                ')' | ']' => {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                    }
                }
                '{' if depth <= 0 && brace_ends => break,
                '{' => depth += 1,
                '}' => depth -= 1,
                ';' if depth <= 0 => {
                    let s: String = self.c[start..self.i].iter().collect();
                    self.i += 1;
                    return s;
                }
                '\n' if depth <= 0 && closed => {
                    // old includes leave the ';' off; the next line starting with '{' is a body
                    let mut k = 1;
                    while self.peek(k).is_some_and(|c| c.is_whitespace()) {
                        k += 1;
                    }
                    if !(brace_ends && self.peek(k) == Some('{')) {
                        break;
                    }
                }
                _ => {}
            }
            self.bump();
        }
        self.c[start..self.i].iter().collect()
    }
}

/// The doc comment waiting for the next declaration, and where it ended.
struct Pending {
    doc: Doc,
    end_line: usize,
}

/// Reads one include file's symbols.
pub fn parse(text: &str, file: &str) -> Vec<Symbol> {
    let chars: Vec<char> = text.chars().collect();
    let mut s = Scanner { c: &chars, i: 0, line: 1 };
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut pending: Option<Pending> = None;
    let mut deprecated_next: Option<String> = None;
    let mut at_line_start = true;
    // the doc for a declaration starting on `line`: one that ended on the line before (or
    // two before, for a /** block)
    let take_doc = |pending: &mut Option<Pending>, line: usize| -> Doc {
        match pending.take() {
            Some(p) if line <= p.end_line + 2 => p.doc,
            _ => Doc::default(),
        }
    };
    while let Some(ch) = s.peek(0) {
        if ch == '\n' {
            at_line_start = true;
            s.bump();
            continue;
        }
        if ch.is_whitespace() {
            s.bump();
            continue;
        }
        if s.starts("/*") {
            let doc_like = s.starts("/**") && !s.starts("/**/");
            let text = s.block_comment();
            if depth == 0 {
                // plain /* */ comments count too (older includes), but never a licence header
                let lower = text.to_lowercase();
                if doc_like || !(lower.contains("copyright") || lower.contains("license") || lower.contains("licence")) {
                    pending = Some(Pending { doc: parse_doc(&text), end_line: s.line });
                }
            }
            continue;
        }
        if s.starts("//") {
            s.line_comment();
            continue;
        }
        if ch == '"' || ch == '\'' {
            s.string();
            at_line_start = false;
            continue;
        }
        if ch == '{' {
            depth += 1;
            s.bump();
            continue;
        }
        if ch == '}' {
            depth = (depth - 1).max(0);
            s.bump();
            continue;
        }
        if depth > 0 {
            s.bump();
            continue;
        }
        if ch == '#' && at_line_start {
            let line = s.line;
            let d = s.directive();
            let d = d.trim_start_matches('#').trim_start();
            if let Some(rest) = d.strip_prefix("define") {
                if rest.starts_with(char::is_whitespace) {
                    let doc = take_doc(&mut pending, line);
                    if let Some(sym) = parse_define(rest.trim(), doc, file, line) {
                        out.push(sym);
                    }
                }
            } else if let Some(rest) = d.strip_prefix("pragma") {
                let rest = rest.trim();
                if let Some(why) = rest.strip_prefix("deprecated") {
                    deprecated_next = Some(why.trim().to_string());
                    // keep the doc for the declaration after it
                    if let Some(p) = pending.as_mut() {
                        p.end_line = s.line;
                    }
                }
            }
            continue;
        }
        at_line_start = false;
        if is_ident_start(ch) {
            let start = s.i;
            let line = s.line;
            let w = s.word();
            match w.as_str() {
                "native" | "forward" => {
                    s.i = start;
                    let decl = s.declaration(false);
                    let mut doc = take_doc(&mut pending, line);
                    if let Some(d) = deprecated_next.take() {
                        doc.deprecated.get_or_insert(d);
                    }
                    let kind = if w == "native" { "native" } else { "forward" };
                    if let Some(sym) = parse_function(&decl, kind, &doc, file, line) {
                        out.push(sym);
                    }
                }
                "stock" | "static" => {
                    s.i = start;
                    let decl = s.declaration(true);
                    let mut doc = take_doc(&mut pending, line);
                    if let Some(d) = deprecated_next.take() {
                        doc.deprecated.get_or_insert(d);
                    }
                    // a function when its '(' comes before any '=' (stock const X[] = "(" isn't one)
                    if split_eq(&decl).contains('(') {
                        if let Some(sym) = parse_function(&decl, "stock", &doc, file, line) {
                            out.push(sym);
                        }
                    } else if decl.contains("const") {
                        // stock const NAME[] = "..." / stock const Float:X = 1.0
                        let one = one_line(&decl);
                        let head = split_eq(&one);
                        let name = param_name(head);
                        let value = one.split_once('=').map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
                        if !name.is_empty() {
                            out.push(constant(&name, value, "constant", &doc, file, line));
                        }
                    }
                }
                "const" => {
                    s.i = start;
                    let decl = s.declaration(false);
                    let doc = take_doc(&mut pending, line);
                    let one = one_line(&decl);
                    let one = one.strip_prefix("const").unwrap_or(&one).trim().to_string();
                    let head = split_eq(&one);
                    let name = param_name(head);
                    let value = one.split_once('=').map(|(_, v)| v.trim().to_string());
                    if !name.is_empty() {
                        out.push(constant(&name, value, "constant", &doc, file, line));
                    }
                }
                "enum" => {
                    let doc = take_doc(&mut pending, line);
                    parse_enum(&mut s, &doc, file, line, &mut out);
                }
                _ => {
                    pending = None;
                }
            }
            continue;
        }
        s.bump();
    }
    out
}

fn parse_define(rest: &str, doc: Doc, file: &str, line: usize) -> Option<Symbol> {
    let name: String = rest.chars().take_while(|&c| is_ident(c)).collect();
    if name.is_empty() || name.starts_with('_') {
        return None;
    }
    let after = &rest[name.len()..];
    // a trailing comment documents it
    let (value, comment) = split_comment(after);
    let mut doc = doc;
    if doc.text.is_empty() {
        if let Some(c) = comment {
            doc = parse_doc(&c);
        }
    }
    if let Some(args) = after.strip_prefix('(') {
        let close = args.find(')')?;
        let params: Vec<Param> = args[..close].split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).map(|p| Param { label: p.into(), name: p.into(), doc: String::new() }).collect();
        let body = one_line(value.strip_prefix('(').and_then(|v| v.split_once(')')).map(|(_, b)| b).unwrap_or(""));
        return Some(Symbol {
            detail: format!("#define {name}({}) {body}", args[..close].trim()),
            name,
            kind: "macro",
            params,
            tag: None,
            doc: doc.text,
            returns: doc.returns,
            notes: doc.notes,
            error: String::new(),
            deprecated: doc.deprecated,
            value: Some(body),
            file: file.into(),
            line,
        });
    }
    let value = one_line(&value);
    Some(constant(&name, (!value.is_empty()).then_some(value), "define", &doc, file, line))
}

/// "5 // five" -> ("5", Some("// five")); comments inside strings don't count.
fn split_comment(s: &str) -> (String, Option<String>) {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < c.len() {
        let ch = c[i];
        if let Some(q) = quote {
            if ch == '^' || ch == '\\' {
                i += 1;
            } else if ch == q {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch == '/' && (c.get(i + 1) == Some(&'/') || c.get(i + 1) == Some(&'*')) {
            return (c[..i].iter().collect::<String>().trim().to_string(), Some(c[i..].iter().collect()));
        }
        i += 1;
    }
    (s.trim().to_string(), None)
}

/// `enum [Tag:][Name] { A = 1, B, /**< doc */ C // doc }`: each entry is a constant.
fn parse_enum(s: &mut Scanner, doc: &Doc, file: &str, line: usize, out: &mut Vec<Symbol>) {
    // the header up to '{'
    let mut header = String::new();
    while let Some(ch) = s.peek(0) {
        if ch == '{' || ch == ';' {
            break;
        }
        header.push(ch);
        s.bump();
    }
    if s.peek(0) != Some('{') {
        return;
    }
    s.bump();
    let enum_name = header.trim().rsplit(':').next().unwrap_or("").trim().to_string();
    if !enum_name.is_empty() && enum_name.chars().all(is_ident) && !enum_name.starts_with('_') {
        out.push(Symbol { detail: format!("enum {}", header.trim()), ..constant(&enum_name, None, "enum", doc, file, line) });
    }
    // entries: (index in out, line)
    let mut last: Option<(usize, usize)> = None;
    let mut waiting: Option<Doc> = None;
    let mut cur = String::new();
    let mut cur_line = s.line;
    let mut depth = 0;
    let finish = |cur: &mut String, cur_line: usize, waiting: &mut Option<Doc>, out: &mut Vec<Symbol>, last: &mut Option<(usize, usize)>| {
        let text = one_line(cur);
        cur.clear();
        if text.is_empty() {
            return;
        }
        let head = split_eq(&text);
        let name = head.rsplit(':').next().unwrap_or("").trim().split('[').next().unwrap_or("").trim().to_string();
        if name.is_empty() || !name.chars().all(is_ident) {
            return;
        }
        let value = text.split_once('=').map(|(_, v)| v.trim().to_string());
        let d = waiting.take().unwrap_or_default();
        out.push(constant(&name, value, "constant", &d, file, cur_line));
        *last = Some((out.len() - 1, cur_line));
    };
    while let Some(ch) = s.peek(0) {
        if s.starts("/*") || s.starts("//") {
            let at = s.line;
            let trailing = s.starts("/**<") || s.starts("//<");
            let text = if s.starts("/*") { s.block_comment() } else { s.line_comment() };
            let d = parse_doc(&text.replacen("//<", "//", 1));
            // a comment on the same line as the entry before it (or marked <) is that entry's
            let pending_entry = !one_line(&cur).is_empty();
            if pending_entry && !trailing {
                // "A = 1 // doc" before the comma: belongs to A
                waiting = Some(d);
            } else if let (Some((idx, l)), true) = (last, trailing || last.is_some_and(|(_, l)| l == at)) {
                let _ = l;
                if out[idx].doc.is_empty() {
                    out[idx].doc = d.text;
                }
            } else {
                waiting = Some(d);
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                let st = s.i;
                s.string();
                cur.extend(s.c[st..s.i].iter());
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth -= 1,
            '}' if depth == 0 => {
                finish(&mut cur, cur_line, &mut waiting, out, &mut last);
                s.bump();
                return;
            }
            '}' => depth -= 1,
            ',' if depth == 0 => {
                finish(&mut cur, cur_line, &mut waiting, out, &mut last);
                s.bump();
                continue;
            }
            _ => {}
        }
        if cur.trim().is_empty() && !ch.is_whitespace() {
            cur_line = s.line;
        }
        cur.push(ch);
        s.bump();
    }
}

/// Every include, read and merged: one entry per name (the first file, in name order, wins).
pub fn parse_all(files: &[(String, String)]) -> Vec<Symbol> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (file, text) in files {
        for sym in parse(text, file) {
            if seen.insert(sym.name.clone()) {
                out.push(sym);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
/* Copyright (C) AMX Mod X team. License: GPL */
#if defined _sample_included
	#endinput
#endif
#define _sample_included

#define PLUGIN_CONTINUE 0 /* returns this to keep going */
#define PLUGIN_HANDLED  1 // stop here
#define charsmax(%1) (sizeof(%1)-1)

/**
 * Returns a client's name.
 *
 * @param index     Client index
 * @param name      Buffer to copy name to
 * @param len       Maximum buffer size
 *
 * @return          Number of cells written to buffer
 * @error           If the index is out of range, an error is thrown.
 */
native get_user_name(index, name[], len);

/**
 * Game time.
 */
native Float:get_gametime();

/**
 * Called when the plugin starts.
 *
 * @note Register things here.
 */
forward plugin_init();

#pragma deprecated Use something else.
native old_thing(const a[] = "x, y", &b = 0, any:...);

enum CsTeams
{
	CS_TEAM_UNASSIGNED = 0,
	CS_TEAM_T,          /**< Terrorists */
	CS_TEAM_CT,         // Counter-Terrorists
	/** Spectators */
	CS_TEAM_SPECTATOR
};

stock const SAMPLE_NAME[] = "sample";
const MAX_THINGS = 32;

/**
 * Clamps a float.
 */
stock Float:fclamp(Float:v, Float:lo, Float:hi)
{
	if (v < lo) { return lo; }
	new s[] = "}";
	return v > hi ? hi : v;
}

native after_stock(id)
"#;

    fn by(syms: &[Symbol], name: &str) -> Symbol {
        syms.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no {name} in {:?}", syms.iter().map(|s| &s.name).collect::<Vec<_>>())).clone()
    }

    #[test]
    fn reads_a_sample_include() {
        let syms = parse(SAMPLE, "sample.inc");
        assert!(!syms.iter().any(|s| s.name.starts_with('_')));

        let n = by(&syms, "get_user_name");
        assert_eq!(n.kind, "native");
        assert_eq!(n.detail, "native get_user_name(index, name[], len)");
        assert_eq!(n.doc, "Returns a client's name.");
        assert_eq!(n.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["index", "name", "len"]);
        assert_eq!(n.params[1].doc, "Buffer to copy name to");
        assert_eq!(n.returns, "Number of cells written to buffer");
        assert!(n.error.contains("out of range"));

        let g = by(&syms, "get_gametime");
        assert_eq!(g.tag.as_deref(), Some("Float"));
        assert_eq!(g.detail, "native Float:get_gametime()");

        let f = by(&syms, "plugin_init");
        assert_eq!((f.kind, f.notes.clone()), ("forward", vec!["Register things here.".to_string()]));

        let o = by(&syms, "old_thing");
        assert_eq!(o.deprecated.as_deref(), Some("Use something else."));
        assert_eq!(o.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["a", "b", "..."]);
        assert_eq!(o.params[0].label, "const a[] = \"x, y\"");

        assert_eq!(by(&syms, "PLUGIN_CONTINUE").value.as_deref(), Some("0"));
        assert_eq!(by(&syms, "PLUGIN_CONTINUE").doc, "returns this to keep going");
        assert_eq!(by(&syms, "PLUGIN_HANDLED").doc, "stop here");
        let m = by(&syms, "charsmax");
        assert_eq!((m.kind, m.params.len()), ("macro", 1));

        assert_eq!(by(&syms, "CsTeams").kind, "enum");
        assert_eq!(by(&syms, "CS_TEAM_UNASSIGNED").value.as_deref(), Some("0"));
        assert_eq!(by(&syms, "CS_TEAM_T").doc, "Terrorists");
        assert_eq!(by(&syms, "CS_TEAM_CT").doc, "Counter-Terrorists");
        assert_eq!(by(&syms, "CS_TEAM_SPECTATOR").doc, "Spectators");

        assert_eq!(by(&syms, "SAMPLE_NAME").kind, "constant");
        assert_eq!(by(&syms, "MAX_THINGS").value.as_deref(), Some("32"));

        let s = by(&syms, "fclamp");
        assert_eq!((s.kind, s.tag.as_deref(), s.doc.as_str()), ("stock", Some("Float"), "Clamps a float."));
        assert_eq!(s.params[2].label, "Float:hi");
        assert_eq!(s.params[2].name, "hi");
        // the body (with a "}" in a string) was skipped, and reading went on after it
        let a = by(&syms, "after_stock");
        assert_eq!(a.detail, "native after_stock(id)");
        assert!(a.doc.is_empty());
        // nothing from inside the body
        assert!(!syms.iter().any(|s| s.name == "s" || s.name == "lo"));
    }

    #[test]
    fn odd_input_never_panics() {
        for text in ["", "native", "native (", "enum {", "enum Foo { A = (1, B", "/** unterminated", "#define", "#define X(", "stock foo(", "\"unterminated\nnative x();", "}}}} native y();", "const = ;", "forward ;"] {
            let _ = parse(text, "odd.inc");
        }
        let syms = parse("}}}} native y();", "odd.inc");
        assert_eq!(syms[0].name, "y");
    }

    #[test]
    fn param_names() {
        assert_eq!(param_name("const Float:origin[3] = {0.0, 0.0, 0.0}"), "origin");
        assert_eq!(param_name("&Float:x"), "x");
        assert_eq!(param_name("{Float,Sql,Result,_}:..."), "...");
        assert_eq!(param_name("any:..."), "...");
        assert_eq!(param_name("const name[]"), "name");
        assert_eq!(param_name("len = sizeof name"), "len");
    }
}

#[cfg(test)]
mod coverage {
    /// Every include on the install, file by file: declarations that look like natives, forwards
    /// and stocks against what the reader found. `LS_TEST_GAME=<Half-Life> cargo test include_coverage -- --nocapture`
    #[test]
    fn include_coverage() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let root = std::path::Path::new(&hl).join("cstrike/addons/amxmodx/scripting");
        let mut files = Vec::new();
        fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>, depth: usize) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() && depth < 3 {
                    walk(&p, out, depth + 1);
                } else if p.extension().is_some_and(|x| x == "inc") {
                    out.push(p);
                }
            }
        }
        walk(&root, &mut files, 0);
        files.sort();
        let (mut want_all, mut got_all) = (0, 0);
        for f in &files {
            let text: String = std::fs::read(f).unwrap().iter().map(|&b| b as char).collect();
            let syms = super::parse(&text, "x.inc");
            let re = regex::Regex::new(r"(?m)^\s*(native|forward|stock|static stock)\s+[A-Za-z_@:]+\s*\(").unwrap();
            let want = re.find_iter(&text).count();
            let got = syms.iter().filter(|s| matches!(s.kind, "native" | "forward" | "stock")).count();
            want_all += want;
            got_all += got;
            if got < want || syms.is_empty() {
                let names: std::collections::HashSet<_> = syms.iter().map(|s| s.name.clone()).collect();
                let missing: Vec<String> = re.find_iter(&text).map(|m| m.as_str().trim_end_matches('(').rsplit([' ', ':']).next().unwrap_or("").trim().to_string()).filter(|n| !names.contains(n)).take(8).collect();
                println!("{}: {} of {} ({} symbols) missing {:?}", f.strip_prefix(&root).unwrap().display(), got, want, syms.len(), missing);
            }
        }
        println!("{} files, {got_all} of {want_all} function declarations read", files.len());
    }
}
