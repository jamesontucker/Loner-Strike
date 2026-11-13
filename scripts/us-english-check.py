#!/usr/bin/env python3
"""Flag UK spellings in user-facing text (Loner-Strike is written in US English).

Looks at string literals and JSX text in app/ui/src, the website copy in site/, and the
string literals in plugin/mod .sma files. Words that are part of an identifier, key, CSS
class or attribute (colour-swatch, colourBy, aria-labelledby) are not matched, and a literal
that looks like a key or class list ("colour", "tools-colour active") is skipped.

  python3 scripts/us-english-check.py            # check, exit 1 on any hit
  python3 scripts/us-english-check.py --fix      # rewrite hits to US spelling
  python3 scripts/us-english-check.py PATH...    # only these files/dirs (also .rs)

Allowlist: add "path:needle" lines to ALLOW below for text that must stay as it is.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_PATHS = ["app/ui/src", "site", "plugins", "mods"]
EXTS = {".ts", ".tsx", ".js", ".jsx", ".mjs", ".html", ".txt", ".sma", ".rs", ".json", ".fgd", ".py"}
SKIP_DIRS = {"node_modules", "dist", "fonts", "img", ".git"}

# UK -> US. Matched as whole words (not next to letters, digits, '_' or '-'), any case.
WORDS = {
    "colour": "color", "colours": "colors", "coloured": "colored", "colouring": "coloring",
    "behaviour": "behavior", "behaviours": "behaviors",
    "favourite": "favorite", "favourites": "favorites", "favour": "favor", "favouring": "favoring",
    "centre": "center", "centres": "centers", "centred": "centered", "centring": "centering",
    "grey": "gray", "greyed": "grayed", "greys": "grays",
    "organise": "organize", "organised": "organized", "organising": "organizing",
    "recognise": "recognize", "recognised": "recognized", "recognising": "recognizing",
    "customise": "customize", "customised": "customized", "customising": "customizing",
    "optimise": "optimize", "optimised": "optimized", "optimising": "optimizing",
    "minimise": "minimize", "minimised": "minimized", "maximise": "maximize", "maximised": "maximized",
    "synchronise": "synchronize", "synchronised": "synchronized",
    "normalise": "normalize", "normalised": "normalized", "visualise": "visualize",
    "modernise": "modernize", "modernised": "modernized", "prioritise": "prioritize",
    "summarise": "summarize", "finalise": "finalize", "initialise": "initialize", "initialised": "initialized",
    "licence": "license", "licences": "licenses", "defence": "defense", "defences": "defenses",
    "offence": "offense", "armour": "armor", "armoury": "armory", "metre": "meter", "metres": "meters",
    "cancelled": "canceled", "cancelling": "canceling", "travelled": "traveled", "travelling": "traveling",
    "modelling": "modeling", "modelled": "modeled", "labelled": "labeled", "labelling": "labeling",
    "catalogue": "catalog", "catalogues": "catalogs", "dialogue": "dialog", "programme": "program",
    "analyse": "analyze", "analysed": "analyzed", "analysing": "analyzing",
    "judgement": "judgment", "whilst": "while", "towards": "toward", "amongst": "among", "learnt": "learned",
    "neighbour": "neighbor", "neighbours": "neighbors", "neighbouring": "neighboring",
}
WORD_RE = re.compile(r"(?<![A-Za-z0-9_\-])(" + "|".join(sorted(WORDS, key=len, reverse=True)) + r")(?![A-Za-z0-9_\-])", re.I)

# "path-suffix:needle" -- a literal in that file containing needle is left alone.
ALLOW = [
    "viewer/Models.tsx:uniform vec3 colour",  # GLSL source: the uniform name is set from JS
]

KEYLIKE = re.compile(r"^[a-z0-9_.,:/#\-]+( [a-z0-9_\-]+)*$")  # keys, class lists, routes


def us(word):
    r = WORDS[word.lower()]
    if word.isupper():
        return r.upper()
    if word[0].isupper():
        return r[0].upper() + r[1:]
    return r


def prose(text):
    t = text.strip()
    return bool(t) and not KEYLIKE.match(t)


def code_spans(src, ext):
    """(start, end) of string-literal contents (and JSX text) in a C-like source."""
    esc = "^" if ext == ".sma" else "\\"
    quotes = '"' if ext in (".rs", ".sma", ".fgd", ".json") else "\"'`"
    jsx = ext in (".tsx", ".jsx")
    n = len(src)
    out = []

    def jsx_text(a, b):
        if jsx:
            for m in re.finditer(r">([^<>{}=;\"|]*[A-Za-z][^<>{}=;\"|]*)<", src[a:b]):
                if "&&" not in m.group(1):
                    out.append((a + m.start(1), a + m.end(1)))

    def template(i):
        """i is just past a backtick; returns the index past the closing backtick."""
        k = i
        chunks = []
        while i < n:
            d = src[i]
            if d == "\\":
                i += 2
            elif d == "`":
                chunks.append((k, i))
                force = prose("X".join(src[x:y] for x, y in chunks))
                out.extend((x, y, force) for x, y in chunks)
                return i + 1
            elif src.startswith("${", i):
                chunks.append((k, i))
                i = code(i + 2, "}")
                k = i
            else:
                i += 1
        return n

    def code(i, stop):
        """Scan code from i until an unmatched `stop` (or the end); returns index past it."""
        depth = 0
        start = i
        while i < n:
            c = src[i]
            if c == "/" and ext not in (".rs", ".sma") and re.search(r"(^|[=(,:\[!&|?{};]|return)\s*$", src[max(0, i - 12):i]) and not src.startswith(("//", "/*"), i):
                j, cls = i + 1, False  # regex literal
                while j < n and src[j] != "\n" and (src[j] != "/" or cls):
                    if src[j] == "\\":
                        j += 1
                    elif src[j] == "[":
                        cls = True
                    elif src[j] == "]":
                        cls = False
                    j += 1
                i = j + 1
                continue
            if src.startswith("//", i):
                jsx_text(start, i)
                j = src.find("\n", i)
                i = start = n if j < 0 else j
            elif src.startswith("/*", i):
                jsx_text(start, i)
                j = src.find("*/", i + 2)
                i = start = n if j < 0 else j + 2
            elif c == "`" and "`" in quotes:
                jsx_text(start, i)
                i = start = template(i + 1)
            elif c in quotes and not (jsx and c == "'" and src[i - 1:i].isalpha() and src[i + 1:i + 2].isalpha()):
                j = i + 1
                while j < n and src[j] != c and not (src[j] == "\n" and ext != ".rs"):
                    j += 2 if src[j] == esc else 1
                if j >= n or src[j] != c:
                    i += 1
                    continue
                jsx_text(start, i)
                out.append((i + 1, j))
                i = start = j + 1
            else:
                if stop and c == "{":
                    depth += 1
                elif stop and c == "}":
                    if depth == 0:
                        jsx_text(start, i)
                        return i + 1
                    depth -= 1
                i += 1
        jsx_text(start, n)
        return n

    code(0, None)
    return out


def html_spans(src):
    """Text between tags plus title/alt/placeholder/aria-label/content attribute values."""
    out = []
    blank = lambda m: "\0" * len(m.group(0))
    body = re.sub(r"<style\b.*?</style>", blank, src, flags=re.S | re.I)
    for m in re.finditer(r"(<script\b[^>]*>)(.*?)</script>", body, flags=re.S | re.I):
        if "json" not in m.group(1):
            a = m.start(2)
            out += [(a + sp[0], a + sp[1]) + tuple(sp[2:]) for sp in code_spans(m.group(2), ".js")]
    body = re.sub(r"<script\b.*?</script>", blank, body, flags=re.S | re.I)
    body = re.sub(r"<!--.*?-->", blank, body, flags=re.S)
    for m in re.finditer(r">([^<>\0]+)<", body):
        out.append((m.start(1), m.end(1)))
    for m in re.finditer(r"\b(?:title|alt|placeholder|aria-label|content)=\"([^\"]*)\"", body):
        out.append((m.start(1), m.end(1)))
    return out


def spans(path, src):
    ext = os.path.splitext(path)[1]
    if ext == ".html":
        return html_spans(src)
    if ext == ".txt":
        return [(0, len(src))]
    return code_spans(src, ext)


def files(paths):
    for p in paths:
        p = os.path.join(ROOT, p)
        if os.path.isfile(p):
            yield p
            continue
        for d, dirs, fs in os.walk(p):
            dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
            for f in fs:
                if os.path.splitext(f)[1] in EXTS:
                    yield os.path.join(d, f)


def main(argv):
    fix = "--fix" in argv
    paths = [a for a in argv if not a.startswith("--")] or DEFAULT_PATHS
    hits = 0
    for path in files(paths):
        rel = os.path.relpath(path, ROOT)
        if not rel.endswith((".sma", ".rs")) and rel.startswith(("plugins", "mods")):
            continue  # only plugin source text there
        try:
            src = open(path, encoding="utf-8").read()
        except UnicodeDecodeError:
            src = open(path, encoding="latin-1").read()
        edits = []
        for sp in spans(path, src):
            a, b = sp[0], sp[1]
            text = src[a:b]
            if path.endswith(".json") and re.match(r'"\s*:', src[b:b + 8]):
                continue  # a JSON key
            if not (sp[2] if len(sp) > 2 else prose(text)) or any(rel.endswith(f) and nd in text for f, nd in (x.split(":", 1) for x in ALLOW)):
                continue
            for m in WORD_RE.finditer(text):
                hits += 1
                line = src.count("\n", 0, a + m.start()) + 1
                if not fix:
                    print(f"{rel}:{line}: {m.group(0)} -> {us(m.group(0))}   {text.strip()[:90]!r}")
                edits.append((a + m.start(), a + m.end(), us(m.group(0))))
        if fix and edits:
            for s, e, r in sorted(set(edits), reverse=True):
                src = src[:s] + r + src[e:]
            open(path, "w", encoding="utf-8").write(src)
            print(f"fixed {len(set(edits))} in {rel}")
    if hits and not fix:
        print(f"\n{hits} UK spelling(s) in user-facing text. Use US English (python3 scripts/us-english-check.py --fix).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
