//! The plugin compiler's (amxxpc) errors and warnings, read from its output and put in
//! plain words, with the line they're on.

use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diag {
    /// the file's name ("myplugin.sma", "loner.inc")
    pub file: String,
    /// in the file that was compiled (not one of its includes)
    pub in_source: bool,
    pub line: u32,
    pub end_line: u32,
    /// error, warning or fatal
    pub severity: &'static str,
    pub code: u32,
    /// the compiler's words
    pub message: String,
    /// what it means, in plain words
    pub plain: String,
}

fn line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?)\((\d+)(?:\s*--\s*(\d+))?\)\s*:\s*(fatal error|error|warning)\s+(\d+)\s*:\s*(.*)$").unwrap())
}

/// The file name at the end of a path, whichever slashes it uses (under Wine the compiler
/// prints `Z:\Users\...\x.sma` or mixes them: `/Users/.../include\loner.inc`).
pub fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Every error and warning in the compiler's output, in order. `source` is the compiled
/// file's name.
pub fn parse(output: &str, source: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for raw in output.lines() {
        let Some(c) = line_re().captures(raw.trim()) else { continue };
        let file = base_name(c[1].trim()).to_string();
        let line: u32 = c[2].parse().unwrap_or(0);
        let end_line: u32 = c.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(line);
        let severity = match &c[4] {
            "fatal error" => "fatal",
            "error" => "error",
            _ => "warning",
        };
        let code: u32 = c[5].parse().unwrap_or(0);
        let message = c[6].trim().to_string();
        let plain = plain(code, &message);
        // a range "(10 -- 12)" points at the end of a statement; its start is more useful
        let (line, end_line) = (line.min(end_line), line.max(end_line));
        out.push(Diag { in_source: file.eq_ignore_ascii_case(source), file, line, end_line, severity, code, message, plain });
    }
    out
}

/// The first "quoted" word in a message.
fn quoted(msg: &str) -> Option<&str> {
    let a = msg.find('"')?;
    let b = msg[a + 1..].find('"')?;
    Some(&msg[a + 1..a + 1 + b])
}

/// What an error or warning means, in plain words.
pub fn plain(code: u32, msg: &str) -> String {
    let q = quoted(msg).map(|s| format!("\u{201c}{s}\u{201d}"));
    let name = q.clone().unwrap_or_else(|| "this".into());
    match code {
        1 => {
            // expected token: "X", but found "Y"
            let exp = quoted(msg).unwrap_or("something");
            let found = msg.rsplit_once("found").map(|(_, f)| f.trim().trim_matches('"').to_string()).unwrap_or_default();
            if found.is_empty() {
                format!("Something is missing here: the compiler expected \u{201c}{exp}\u{201d}. Often it's a ; ) or }} on this line or the one before.")
            } else {
                format!("Something is missing here: the compiler expected \u{201c}{exp}\u{201d} but found \u{201c}{found}\u{201d}. Often it's a ; ) or }} on this line or the one before.")
            }
        }
        2 => "Only a variable or a value can go after \u{201c}case\u{201d}; this one is something else.".into(),
        4 => format!("The function {name} is declared but has no body. Add one, or remove the line."),
        8 => "This must be a fixed value (a number or a #define), not a variable.".into(),
        9 => "An array needs a size here: put a number between the [ ].".into(),
        10 => "The compiler didn't understand this line as a function or a declaration. Look for a missing ( or a stray word, often just above.".into(),
        12 => "This needs to be inside a function's body ({ ... }).".into(),
        17 => format!("{name} isn't defined anywhere. Check the spelling (names are case-sensitive), or add the #include that has it."),
        20 => format!("{name} isn't a valid name. Names start with a letter or _ and use only letters, numbers and _."),
        21 => format!("{name} is already defined. Give one of them another name."),
        22 => "The left side of = must be a variable you can change.".into(),
        25 => format!("The function {name} is defined differently from its declaration (forward or native). Make the parameters match."),
        27 => "That character constant is invalid: use one character between ' ', like 'a'.".into(),
        28 => format!("{name} isn't an array (or has fewer dimensions), so it can't take [ ] here."),
        29 => "The compiler couldn't read this expression. Look for a missing value, operator or bracket.".into(),
        32 => "An array index is out of bounds: the number between [ ] is too big for the array's size.".into(),
        33 => format!("{name} is an array: use one element ({}[0]), or pass the whole array to a function that takes one.", quoted(msg).unwrap_or("x")),
        34 => "Only the last parameters of a function can have default values.".into(),
        35 => "A value passed to this function has the wrong type, e.g. a number where text (an array) is expected, or the other way round.".into(),
        36 => "There's an empty statement here (a stray ;).".into(),
        37 => "A text (string) isn't closed: add the closing \".".into(),
        39 => "This constant symbol has no value given.".into(),
        40 => "Two case labels in this switch have the same value.".into(),
        44 => "A switch can have only one default.".into(),
        46 => "This array's size is unknown here: give it a size.".into(),
        47 => "The arrays' sizes don't match: the one being copied into is too small, or they're different lengths.".into(),
        48 => "The arrays have different numbers of dimensions.".into(),
        52 => "This array's size can't be worked out from its values here.".into(),
        54 => "A } doesn't match: there's an extra one, or one is missing above.".into(),
        55 => "A function's body starts without its header (the line before is probably missing its name or ( ).".into(),
        67 => format!("{name} needs a value: a variable with & or an array can't be left empty."),
        75 => "The text is longer than the array can hold.".into(),
        76 => "A preprocessor line (#...) has a syntax error.".into(),
        77 => "A #define here is malformed.".into(),
        88 => "The number of values passed doesn't match what the function takes. Check its parameters (hover the name to see them).".into(),
        90 => "A public function can't return an array.".into(),
        92 | 100 => match quoted(msg) {
            Some(f) => format!("Couldn't find the include \u{201c}{f}\u{201d}. Check the name, or put {f}.inc next to your plugin or in the game's include folder."),
            None => "Couldn't read a file the plugin includes.".into(),
        },
        101 | 102 => "There's a stray character the compiler doesn't understand here.".into(),
        103 => "Too much nesting or recursion: simplify this part.".into(),
        104 => "An #if has no matching #endif.".into(),
        200 => format!("{name} is longer than 31 characters and gets cut. Use a shorter name."),
        201 => "This #if section contains a redefinition.".into(),
        202 => "The number of values passed doesn't match what the function takes.".into(),
        203 => format!("{name} is never used. Remove it, or use it."),
        204 => format!("{name} gets a value that's never used. Remove it, or use it."),
        205 => "This code can never run (its condition is always false).".into(),
        206 => "This test is always true or always false.".into(),
        208 => format!("{name} is used before it's defined, and it returns a tagged value. Move it above where it's used."),
        209 => format!("The function {name} should return a value on every path: add a return at the end."),
        211 => "Possibly an unintended assignment: a single = in a test sets a value. Use == to compare.".into(),
        213 => "The value's type (tag) doesn't match: a Float where a whole number is expected, or the other way round. Use float() or floatround(), or the right tag.".into(),
        214 => format!("{name} could be const: it's an array the function never changes."),
        215 => "This expression has no effect: it calculates something and throws it away.".into(),
        216 => "A comment is nested inside another comment.".into(),
        217 => "Loose indentation: this line's indentation is mixed or off. It works; tidy the tabs and spaces to silence it.".into(),
        218 => "The old-style prototype here is missing a ;.".into(),
        219 => format!("A local variable {name} has the same name as one outside it, and hides it. Rename one."),
        220 => "This value has a tag but goes into a parameter without one.".into(),
        225 => "This code can never be reached (it's after a return, break or continue).".into(),
        226 => "A variable is assigned to itself.".into(),
        232 => "A float value is used where a whole number is expected; it's cut to a whole number.".into(),
        233 => format!("{name} is deprecated: it still works but may go away. Hover it to see what to use instead."),
        234 => format!("{name} is deprecated: hover it to see what to use instead."),
        235 => format!("The public function {name} has no forward declaration. Check the spelling of the forward's name (e.g. plugin_init, client_putinserver)."),
        _ => String::new(),
    }
}

/// A short line for a compile that didn't finish, from the output when nothing parsed.
pub fn summary(output: &str) -> String {
    let t = output.trim();
    if t.is_empty() {
        return "The compiler gave no reason.".into();
    }
    let from = t.char_indices().rev().nth(299).map(|(i, _)| i).unwrap_or(0);
    t[from..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAD: &str = include_str!("../testdata/amxxpc/bad.txt");
    const OK: &str = include_str!("../testdata/amxxpc/ok.txt");
    const FATAL: &str = include_str!("../testdata/amxxpc/fatal.txt");
    const INCLUDE: &str = include_str!("../testdata/amxxpc/include.txt");

    #[test]
    fn errors_and_warnings() {
        let d = parse(BAD, "bad.sma");
        assert_eq!(d.len(), 3, "{d:?}");
        assert_eq!((d[0].severity, d[0].code, d[0].line, d[0].in_source), ("error", 17, 6, true));
        assert_eq!(d[0].file, "bad.sma");
        assert!(d[0].plain.contains("\u{201c}undefined_func\u{201d} isn't defined"), "{}", d[0].plain);
        assert_eq!((d[1].severity, d[1].code), ("warning", 204));
        assert!(d[1].plain.contains("\u{201c}x\u{201d}"));
        assert_eq!((d[2].code, d[2].line), (203, 3));
    }

    #[test]
    fn warnings_only_and_done() {
        let d = parse(OK, "ok.sma");
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].severity, d[0].line), ("warning", 5));
        assert!(OK.contains("Done."));
    }

    #[test]
    fn fatal_missing_include() {
        let d = parse(FATAL, "fatal.sma");
        assert_eq!((d[0].severity, d[0].code), ("fatal", 100));
        assert!(d[0].plain.contains("\u{201c}nothere\u{201d}"), "{}", d[0].plain);
    }

    #[test]
    fn error_in_an_include_and_ranges() {
        let d = parse(INCLUDE, "min.sma");
        assert_eq!(d[0].file, "loner.inc");
        assert!(!d[0].in_source);
        assert_eq!(d[0].code, 88);
        assert_eq!((d[1].line, d[1].end_line, d[1].in_source), (10, 12, true));
        assert_eq!(d[1].code, 1);
        assert!(d[1].plain.contains("\u{201c};\u{201d} but found \u{201c}}\u{201d}") || d[1].plain.contains("expected"), "{}", d[1].plain);
    }

    #[test]
    fn names_from_any_path() {
        assert_eq!(base_name(r"Z:\a\b\x.sma"), "x.sma");
        assert_eq!(base_name("/a/b/include\\loner.inc"), "loner.inc");
        assert_eq!(base_name("x.sma"), "x.sma");
        // every code we explain says something
        for code in [1, 17, 21, 29, 33, 35, 88, 100, 203, 204, 209, 211, 213, 217, 225, 235] {
            assert!(!plain(code, "symbol \"a\"").is_empty(), "{code}");
        }
    }
}
