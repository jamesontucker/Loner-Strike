//! Applying unified diffs at install time (the bot fixes for the community mods).
//!
//! Used for mods whose upstream code has no license: the app ships only
//! Loner-Strike's changes and applies them to the original files it downloads.
//! Line endings are kept exactly (CZ files often use CRLF).

use super::splitlines;
use regex::Regex;
use std::sync::OnceLock;

const NO_EOL: &str = "\\ No newline at end of file";

/// One hunk: where it starts in the old file, and its lines as (op, text).
pub type Hunk = (usize, Vec<(char, String)>);

/// [(path, hunks)] for each file in the diff, in order.
///
/// Each hunk's header says how many old and new lines it has, so hunks are read
/// by count: a removed line that happens to start with "--" can't be mistaken for
/// the next file's header.
pub fn parse(diff: &str) -> Result<Vec<(String, Vec<Hunk>)>, String> {
    static HEAD: OnceLock<Regex> = OnceLock::new();
    let head = HEAD.get_or_init(|| Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+\d+(?:,(\d+))? @@").unwrap());
    let mut files: Vec<(String, Vec<Hunk>)> = Vec::new();
    let mut path: Option<usize> = None;
    let lines = splitlines(diff, true);
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(p) = line.strip_prefix("+++ ") {
            let p = p.trim();
            let p = p.strip_prefix("b/").unwrap_or(p).to_string();
            path = Some(match files.iter().position(|(f, _)| *f == p) {
                Some(n) => n,
                None => {
                    files.push((p, Vec::new()));
                    files.len() - 1
                }
            });
            i += 1;
            continue;
        }
        let Some(m) = head.captures(line) else {
            i += 1;
            continue;
        };
        let num = |g: usize| m.get(g).map(|v| v.as_str().parse::<i64>().unwrap_or(0)).unwrap_or(1);
        let start: usize = m[1].parse().unwrap_or(0);
        let (mut old_left, mut new_left) = (num(2), num(3));
        let mut body = Vec::new();
        i += 1;
        while i < lines.len() && (old_left > 0 || new_left > 0) {
            let l = lines[i];
            let op = l.chars().next().unwrap_or('\0');
            if !matches!(op, ' ' | '-' | '+') {
                return Err(format!("malformed patch near: {:?}", l.chars().take(60).collect::<String>()));
            }
            let mut text = l[op.len_utf8()..].to_string();
            if i + 1 < lines.len() && lines[i + 1].trim_end_matches(['\r', '\n']) == NO_EOL {
                if text.ends_with('\n') {
                    text.pop();
                }
                i += 1;
            }
            body.push((op, text));
            if op != '+' {
                old_left -= 1;
            }
            if op != '-' {
                new_left -= 1;
            }
            i += 1;
        }
        let Some(n) = path else { return Err("malformed patch: a change comes before its file name".into()) };
        files[n].1.push((start, body));
    }
    Ok(files)
}

/// Applies one file's hunks; each must match exactly (searching nearby if lines moved).
pub fn apply(text: &str, hunks: &[Hunk]) -> Result<String, String> {
    let src = splitlines(text, true);
    let mut out: Vec<&str> = Vec::new();
    let mut pos = 0usize;
    for (start, body) in hunks {
        let old: Vec<&str> = body.iter().filter(|(op, _)| *op != '+').map(|(_, l)| l.as_str()).collect();
        let new: Vec<&str> = body.iter().filter(|(op, _)| *op != '-').map(|(_, l)| l.as_str()).collect();
        let want = start.saturating_sub(1) as i64;
        let fits = |cand: i64| -> bool {
            cand >= pos as i64
                && cand <= src.len() as i64 - old.len() as i64
                && src[cand as usize..cand as usize + old.len()] == old[..]
        };
        let mut at = None;
        'search: for delta in 0..=src.len() as i64 {
            for cand in [want + delta, want - delta] {
                if fits(cand) {
                    at = Some(cand as usize);
                    break 'search;
                }
            }
        }
        let Some(at) = at else {
            return Err(format!("a change near line {start} doesn't fit the downloaded file"));
        };
        out.extend_from_slice(&src[pos..at]);
        out.extend_from_slice(&new);
        pos = at + old.len();
    }
    out.extend_from_slice(&src[pos..]);
    Ok(out.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_moved_lines() {
        let old = "a\r\nb\r\nc\r\nd\r\ne\r\n";
        let diff = "--- a/x.txt\n+++ b/x.txt\n@@ -3,2 +3,2 @@\n c\r\n-d\r\n+D!\r\n";
        let files = parse(diff).unwrap();
        assert_eq!(files[0].0, "x.txt");
        assert_eq!(apply(old, &files[0].1).unwrap(), "a\r\nb\r\nc\r\nD!\r\ne\r\n");
        // the same change still fits when two lines were added above it
        assert_eq!(apply(&format!("z\r\ny\r\n{old}"), &files[0].1).unwrap(), "z\r\ny\r\na\r\nb\r\nc\r\nD!\r\ne\r\n");
        assert!(apply("nothing here\n", &files[0].1).is_err());
    }

    #[test]
    fn no_newline_at_end() {
        let diff = "--- a/f\n+++ b/f\n@@ -1,2 +1,2 @@\n x\n-y\n\\ No newline at end of file\n+z\n\\ No newline at end of file\n";
        let files = parse(diff).unwrap();
        assert_eq!(apply("x\ny", &files[0].1).unwrap(), "x\nz");
    }
}
