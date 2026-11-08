#!/usr/bin/env python3
"""Checks the bots' chat lines (plugins/configs/gamemodes/botchat*.ini) against what
gm_botchat and gm_people understand (docs/bot-people.md).

    python3 scripts/botchat-lint.py [file ...]      (all botchat*.ini without arguments)

Errors: unknown sections, conditions, values or options, broken placeholders, a | or > in
the wrong place, lines over 100 characters, non-ASCII. Warnings: later slang (2008+), lines
said in two places, long lines (most pub chat is a few words).
"""

import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIR = os.path.join(ROOT, "plugins", "configs", "gamemodes")

TRAITS = {
    "warmth": "cold neutral warm",
    "ego": "humble normal showoff",
    "temper": "chill normal tilty",
    "talk": "silent quiet normal chatty spammer",
    "humor": "none dry goofy troll",
    "age": "kid teen adult old",
    "skill": "newbie average good pro",
    "role": "regular drifter admin clan afk",
    "region": "us uk de se pl ru br fr nl es au fi dk",
    "style": "rusher roamer lurker careful camper awper knifer nader ninja objective weirdgun",
}
HOBBIES = "wc3 wow halo school work cars music lan cz source awp sleep food girlfriend footy metal".split()
WORDS = {"phase": "dead freeze live end", "other": "human bot none", "team": "t ct spec",
         "time": "night morning day evening"}
NUMERIC = {"kills", "deaths", "frags", "kd", "alive", "diff", "vs", "vsby", "joined",
           # memory (gm_people, stage 3): how it feels about the other one, sessions together,
           # days since it last saw them, its rival
           "rel", "fam", "days", "rival"}
FREE = {"map", "flag"}  # any value
OPTIONS = {"odds", "cd", "once", "set"}
PLACEHOLDERS = {"name", "other", "map", "place", "weapon", "hs", "carrier", "us", "them", "diff",
                "timeleft", "nextmap", "tag", "hobby"}
LETTERS = set("ftsjqncokxhzpbiduwrmABCDEFGHLMNOPSTYIJKQRUVWXZ0123aeglvy456789")
MOODS = set("-+^=")
# later than the era (about 2008 on), whole words
LATER = ["gg ez", "rekt", "rush b no stop", "cyka", "blyat", "huehue", "kek", "poggers", "pog", "yolo",
         "sus", "ratio", "noice", "lit", "u mad", "epic fail", "baited", "salty af", "af", "lowkey",
         "highkey", "goat", "based", "cringe", "simp", "ez"]
ALLOW_EZ_SECTIONS = {"r_ez", "t_ez"}  # answering someone who typed it


def sections_in_code():
    src = open(os.path.join(ROOT, "plugins", "scripting", "gm_botchat.sma"), encoding="latin-1").read()
    found = set(re.findall(r'"([a-z_]+)"', src))
    found |= {m for m in re.findall(r'fmt\("r_%s", tag\)', src)}
    return found


def check(files):
    errors = warnings = lines = 0
    seen = {}
    code = sections_in_code()
    reply_tags = set()
    defined = set()
    for path in files:
        section = None
        for n, raw in enumerate(open(path, encoding="latin-1"), 1):
            line = raw.rstrip("\r\n")
            where = f"{os.path.basename(path)}:{n}"
            s = line.strip()
            if not s or s.startswith(";"):
                continue
            if s.startswith("["):
                section = s[1:s.index("]")].lower() if "]" in s else s
                defined.add(section)
                continue
            if "|" not in s:
                print(f"{where}: error: no | between conditions and text: {s}")
                errors += 1
                continue
            lines += 1
            cond, text = s.split("|", 1)
            text = text.strip()

            def err(msg):
                nonlocal errors
                print(f"{where}: error: {msg}: {s}")
                errors += 1

            def warn(msg):
                nonlocal warnings
                print(f"{where}: warning: {msg}: {s}")
                warnings += 1

            if section is None:
                err("line before any [section]")
            if not text:
                err("no text")
            if any(ord(c) > 126 for c in s):
                err("not plain ASCII")
            body = text.split(">")[0].rstrip()
            if len(body) > 100:
                err("over 100 characters")
            elif len(body) > 60:
                warn("long for pub chat")
            if text.count(">") > 1:
                err("more than one >")
            if ">" in text:
                tag = text.split(">", 1)[1].strip()
                if not re.fullmatch(r"[a-z_]+", tag):
                    err(f"bad reply tag '{tag}'")
                reply_tags.add("r_" + tag)
            # placeholders and inline choices
            for m in re.finditer(r"\{([^{}]*)\}", body):
                inner = m.group(1)
                if "|" in inner:
                    if any(not opt.strip() and len(inner.split("|")) < 2 for opt in inner.split("|")):
                        err("empty inline choice")
                elif inner not in PLACEHOLDERS:
                    err(f"unknown placeholder {{{inner}}}")
            if body.count("{") != body.count("}"):
                err("unbalanced { }")
            if "|" in re.sub(r"\{[^{}]*\}", "", body):
                err("| in the text outside {a|b} choices")
            # conditions
            for tok in cond.split():
                if tok.startswith("!"):
                    k = tok[1:].split("=", 1)[0]
                    if k not in OPTIONS or "=" not in tok:
                        err(f"unknown option {tok}")
                    elif k in ("odds", "cd") and not re.fullmatch(r"\d+(\.\d+)?", tok.split("=", 1)[1]):
                        err(f"{tok} needs a number")
                    elif k == "once" and tok.split("=", 1)[1] not in ("map", "session"):
                        err(f"{tok}: once=map or once=session")
                    continue
                m = re.fullmatch(r"([a-z0-9]+)(>=|<=|!=|=|<|>)(.+)", tok)
                if m:
                    k, op, v = m.groups()
                    if k in TRAITS:
                        vals = TRAITS[k].split()
                        if v not in vals and not v.isdigit():
                            err(f"{k} has no value '{v}' ({TRAITS[k]})")
                    elif k == "hobby":
                        if v not in HOBBIES or op not in ("=", "!="):
                            err(f"hobby=one of {' '.join(HOBBIES)}")
                    elif k == "arch":
                        pass
                    elif k in WORDS:
                        if v not in WORDS[k].split() or op not in ("=", "!="):
                            err(f"{k}= one of {WORDS[k]}")
                    elif k in NUMERIC:
                        if not re.fullmatch(r"-?\d+(\.\d+)?", v):
                            err(f"{k} needs a number")
                    elif k in FREE:
                        pass
                    else:
                        err(f"unknown condition '{k}'")
                    continue
                if tok == "*":
                    continue
                bad = [c for c in tok if c not in LETTERS and c not in MOODS]
                if bad:
                    err(f"unknown personality letters or condition '{tok}'")
            # era
            low = " " + re.sub(r"[^a-z0-9 ]", " ", body.lower()) + " "
            for w in LATER:
                if f" {w} " in low and not (w == "ez" and section in ALLOW_EZ_SECTIONS):
                    warn(f"'{w}' is later than the era")
            key = (section, body.lower())
            if key in seen:
                warn(f"same line as {seen[key]}")
            else:
                seen[key] = where
    for tag in sorted(reply_tags - defined):
        print(f"error: a line asks for [{tag}], which no file has")
        errors += 1
    print(f"{lines} lines in {len(files)} file(s): {errors} errors, {warnings} warnings")
    return errors


if __name__ == "__main__":
    files = sys.argv[1:] or sorted(glob.glob(os.path.join(DIR, "botchat*.ini")))
    sys.exit(1 if check(files) else 0)
