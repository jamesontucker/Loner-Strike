"""Builds the bot personality files for the Bot styles (gm_humanbots.amxx).

Each style is a copy of czero/BotProfileEnhanced.db (or BotProfile.db) with
aggression and teamwork reshaped, and CZ's own reaction delay cut down so the
plugin's human reaction times aren't added on top of it.

    python3 botstyles.py            writes czero/BotProfile_<Style>.db
"""
import os
import random
import re
import sys

STYLES = {
    # name: (aggression lo, hi), (teamwork lo, hi), skill scale
    "Pub":         ((55, 100), (20, 80), 1.0),
    "Matchmaking": ((50, 90),  (75, 100), 1.0),
    "Veterans":    ((60, 95),  (70, 100), 1.0),
    "Newbie":      ((45, 85),  (30, 70), 0.6),
    "LockedIn":    ((65, 100), (80, 100), 1.0),
    "Hackers":     ((80, 100), (50, 100), 1.0),
    "Gods":        ((100, 100), (100, 100), None),   # None: every bot at Skill 100
}
FIELD = re.compile(r"^(\s*)(Aggression|Teamwork|ReactionTime|AttackDelay|Skill)(\s*=\s*)([0-9.]+)(.*)$")


def reshape(text, style, seed=27015):
    (alo, ahi), (tlo, thi), skill_scale = STYLES[style]
    rnd = random.Random(seed + len(style))
    out = []
    for line in text.splitlines():
        m = FIELD.match(line)
        if not m:
            out.append(line)
            continue
        indent, key, eq, val, rest = m.groups()
        v = float(val)
        if key == "Aggression":
            # keep each bot's relative temperament, squeezed into the style's range
            v = alo + (ahi - alo) * v / 100.0
            if ahi > alo:
                v += rnd.uniform(-5, 5)
            val = str(int(max(0, min(100, v))))
        elif key == "Teamwork":
            val = str(int(tlo + (thi - tlo) * v / 100.0))
        elif key == "ReactionTime":
            val = "0.1"                  # the plugin adds human reaction times
        elif key == "AttackDelay":
            val = "0"
        elif key == "Skill":
            val = "100" if skill_scale is None else str(int(v * skill_scale))
        out.append(indent + key + eq + val + rest)
    return "\r\n".join(out) + "\r\n"


def build(czero):
    for name in ("BotProfileEnhanced.db", "BotProfile.db"):
        src = os.path.join(czero, name)
        if os.path.exists(src):
            break
    else:
        raise SystemExit("no BotProfile.db found in " + czero)
    with open(src, encoding="latin-1") as f:
        text = f.read()
    made = []
    for style in STYLES:
        dst = os.path.join(czero, "BotProfile_%s.db" % style)
        with open(dst, "w", encoding="latin-1", newline="") as f:
            f.write("// Made by CZ Control Panel botstyles.py from %s for the %s bot style\r\n" % (name, style))
            f.write(reshape(text, style))
        made.append(dst)
    return made


if __name__ == "__main__":
    hl = os.path.expanduser("~/Library/Application Support/CrossOver/Bottles/Steam/drive_c/"
                            "Program Files (x86)/Steam/steamapps/common/Half-Life")
    for path in build(sys.argv[1] if len(sys.argv) > 1 else os.path.join(hl, "czero")):
        print("wrote", path)
