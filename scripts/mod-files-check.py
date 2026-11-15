#!/usr/bin/env python3
"""Checks the files Loner-Strike installs for the classic modes (mods/, plugins/) for mistakes
that only show up on a running server:

  * Base Builder's SQL: every query in its plugin sources must prepare on SQLite (the driver
    the install uses). Pawn joins a string continued with a backslash to the next line without
    the next line's leading spaces, so "CURRENT_TIMESTAMP\\" + "WHERE ..." becomes
    "CURRENT_TIMESTAMPWHERE" and the query fails on every map.
  * AMX Mod X language files (data/lang/*.txt): before the first [xx] section only blank lines
    and ; comments are allowed, and "//" is never a comment ("Invalid multi-lingual line").

    python3 scripts/mod-files-check.py        # exit 1 on any problem
"""
import pathlib
import re
import sqlite3
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASEBUILDER = ROOT / "mods/basebuilder/addons/amxmodx/scripting"


def pawn_queries(folder):
    """(file, query) for every string literal in .sma/.inl files that starts with SQL."""
    out = []
    for f in sorted(folder.rglob("*")):
        if f.suffix not in (".sma", ".inl"):
            continue
        src = f.read_text(encoding="latin-1")
        src = re.sub(r"(?m)^\s*//.*$", "", src)          # whole-line comments
        src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)  # block comments
        joined = re.sub(r"\\\r?\n[ \t]*", "", src)       # Pawn line continuation
        for m in re.finditer(r'"((?:[^"\\^\n]|\^.)*)"', joined):
            s = m.group(1)
            if re.match(r"\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|DROP|REPLACE) ", s):
                out.append((f.relative_to(ROOT), s))
    return out


def fill(q):
    q = q.replace('^"', '"')
    q = re.sub(r"%\d*\.?\d*[dif]", "1", q)
    return re.sub(r"%\d*s", "x", q)


def check_sql():
    problems = []
    queries = pawn_queries(BASEBUILDER)
    db = sqlite3.connect(":memory:")
    creates = [(f, q) for f, q in queries if q.lstrip().startswith("CREATE")]
    others = [(f, q) for f, q in queries if not q.lstrip().startswith("CREATE")]
    for f, q in creates + others:
        # CREATE TABLE ... ) %s  takes the statement end (";") as its argument
        sql = fill(q.replace(") %s", ")")) if q.lstrip().startswith("CREATE") else fill(q)
        try:
            db.execute(sql if sql.lstrip().startswith("CREATE") else "EXPLAIN " + sql)
        except sqlite3.Error as e:
            problems.append(f"{f}: SQLite can't run this query ({e}): {sql[:160]}")
    if len(queries) < 40:
        problems.append(f"{BASEBUILDER}: found only {len(queries)} queries; the check can't see Base Builder's SQL")
    return problems


def check_lang():
    problems = []
    for f in sorted(list(ROOT.glob("mods/*/addons/amxmodx/data/lang/*.txt")) + list(ROOT.glob("plugins/**/lang/*.txt"))):
        in_section = False
        for n, line in enumerate(f.read_text(encoding="latin-1").splitlines(), 1):
            s = line.strip()
            if s.startswith("["):
                in_section = True
            elif s.startswith("//"):
                problems.append(f"{f.relative_to(ROOT)}:{n}: '//' isn't a comment in a language file (use ;)")
            elif s and not s.startswith(";") and not in_section:
                problems.append(f"{f.relative_to(ROOT)}:{n}: a line before the first [xx] section")
    return problems


def main():
    problems = check_sql() + check_lang()
    for p in problems:
        print(p)
    print(f"{len(problems)} problem(s)" if problems else "Mod files OK")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
